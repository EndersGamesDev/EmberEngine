//! Headless dedicated server.
//!
//! Architecture: all game state lives on ONE simulation thread that runs a
//! fixed 60 Hz tick (matching the engine's deterministic-simulation pillar).
//! Network IO threads never touch the world — they only translate socket
//! bytes into `Event`s on an mpsc channel, and each connection has a writer
//! thread fed by its own outbound channel. Ordering per client is guaranteed
//! by the channels; the sim thread is the single writer of truth.

use std::collections::HashMap;
use std::io;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use ember_net::{
    color_for, read_msg, sanitize_dir, sanitize_name, write_msg, ClientMsg, PlayerId, PlayerMeta,
    PlayerState, ServerMsg, ARENA_HALF, CLIENT_TIMEOUT_SECS, MOVE_SPEED, PROTOCOL_VERSION, TICK_HZ,
};

pub struct ServerConfig {
    pub max_players: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { max_players: 32 }
    }
}

enum Event {
    Connected {
        conn: u64,
        stream: TcpStream,
        peer: String,
    },
    Msg {
        conn: u64,
        msg: ClientMsg,
    },
    Disconnected {
        conn: u64,
    },
}

struct Player {
    id: PlayerId,
    name: String,
    color: [f32; 3],
    pos: [f32; 2],
    vel: [f32; 2],
    dir: [f32; 2],
}

/// Per-client outbound queue depth. At 60 snapshots/s this is ~4 s of
/// backlog; a client further behind than that is dead or hostile.
const OUTBOUND_QUEUE: usize = 256;
/// Writes to a stalled client error out after this instead of blocking the
/// writer thread forever.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// A joined client silent this long is flagged as lagging (well before the
/// hard CLIENT_TIMEOUT_SECS kick — clients keepalive every ~2 s or less).
const LAG_THRESHOLD: Duration = Duration::from_secs(3);

struct Conn {
    tx: SyncSender<ServerMsg>,
    /// Kept solely so `remove_conn` can unblock the reader thread; the
    /// writer half shuts the socket down fully when it exits.
    sock: TcpStream,
    peer: String,
    player: Option<Player>,
    last_seen: Instant,
    /// True while this client is flagged as lagging; cleared on any message.
    lag_flagged: bool,
}

/// Runs the server on an already-bound listener. Blocks forever.
/// Taking a listener (instead of an address) lets tests bind port 0.
pub fn run(listener: TcpListener, cfg: ServerConfig) -> io::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        "ember-server listening on {local} (protocol v{PROTOCOL_VERSION}, {TICK_HZ} Hz, max {} players)",
        cfg.max_players
    );

    let (events_tx, events_rx) = mpsc::channel::<Event>();

    // Accept thread: hand every connection to the sim thread.
    {
        let events_tx = events_tx.clone();
        thread::spawn(move || {
            let mut next_conn: u64 = 1;
            for stream in listener.incoming() {
                let stream = match stream {
                    Ok(s) => s,
                    Err(e) => {
                        // e.g. fd exhaustion: back off instead of spinning.
                        tracing::warn!("accept error: {e}");
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                };
                let peer = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "?".into());
                let _ = stream.set_nodelay(true);
                let conn = next_conn;
                next_conn += 1;
                if events_tx
                    .send(Event::Connected { conn, stream, peer })
                    .is_err()
                {
                    break; // sim thread gone
                }
            }
        });
    }

    sim_loop(events_tx, events_rx, cfg)
}

fn spawn_reader(conn: u64, stream: TcpStream, events_tx: Sender<Event>) {
    thread::spawn(move || {
        let mut stream = stream;
        // Ends on EOF, reset, or protocol garbage.
        while let Ok(msg) = read_msg::<_, ClientMsg>(&mut stream) {
            let is_bye = matches!(msg, ClientMsg::Bye);
            if events_tx.send(Event::Msg { conn, msg }).is_err() || is_bye {
                break;
            }
        }
        let _ = events_tx.send(Event::Disconnected { conn });
    });
}

fn spawn_writer(mut stream: TcpStream) -> SyncSender<ServerMsg> {
    let (tx, rx) = mpsc::sync_channel::<ServerMsg>(OUTBOUND_QUEUE);
    thread::spawn(move || {
        let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
        // Drains queued messages even after all senders drop, so a final
        // Reject still reaches the peer before the shutdown below.
        for msg in rx {
            if write_msg(&mut stream, &msg).is_err() {
                break;
            }
        }
        // Unblocks the reader thread too: shutdown applies to the socket,
        // not just this clone.
        let _ = stream.shutdown(Shutdown::Both);
    });
    tx
}

fn sim_loop(
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    cfg: ServerConfig,
) -> io::Result<()> {
    let tick_dt = Duration::from_nanos(1_000_000_000 / TICK_HZ as u64);
    let dt = tick_dt.as_secs_f32();

    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut next_player_id: u32 = 1;
    let mut tick: u64 = 0;
    let mut next_tick_at = Instant::now() + tick_dt;
    let mut last_report = Instant::now();
    // Tick-health accounting for the periodic report.
    let mut max_busy = Duration::ZERO;
    let mut overruns: u32 = 0;

    loop {
        // Drain events until the next tick deadline.
        loop {
            let now = Instant::now();
            let Some(wait) = next_tick_at.checked_duration_since(now) else {
                break;
            };
            match events_rx.recv_timeout(wait) {
                Ok(ev) => handle_event(ev, &mut conns, &mut next_player_id, &cfg, &events_tx),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::other("accept thread died"));
                }
            }
        }
        // After a stall the deadline is long past and the loop above drained
        // nothing; process what is already queued (bounded so a flood can't
        // starve the tick) so keepalives count before the timeout sweep.
        for _ in 0..1024 {
            match events_rx.try_recv() {
                Ok(ev) => handle_event(ev, &mut conns, &mut next_player_id, &cfg, &events_tx),
                Err(_) => break,
            }
        }
        next_tick_at += tick_dt;
        // If we fell far behind (debugger pause, host stall), resync instead
        // of running a burst of catch-up ticks.
        let now = Instant::now();
        if now > next_tick_at + tick_dt * 10 {
            let behind = now.duration_since(next_tick_at);
            tracing::warn!(
                tick,
                behind_ms = behind.as_millis() as u64,
                "sim stall: fell behind the tick clock; resyncing"
            );
            next_tick_at = now + tick_dt;
        }
        tick += 1;
        let tick_started = Instant::now();
        let _tick_span = tracing::trace_span!("tick", tick).entered();

        // Integrate.
        for conn in conns.values_mut() {
            if let Some(p) = conn.player.as_mut() {
                p.vel = [p.dir[0] * MOVE_SPEED, p.dir[1] * MOVE_SPEED];
                p.pos[0] = (p.pos[0] + p.vel[0] * dt).clamp(-ARENA_HALF, ARENA_HALF);
                p.pos[1] = (p.pos[1] + p.vel[1] * dt).clamp(-ARENA_HALF, ARENA_HALF);
            }
        }

        // Lag detection: flag joined clients that have gone silent well
        // before the hard timeout kicks them (clients keepalive every ~2 s).
        let now = Instant::now();
        for (&conn_id, c) in conns.iter_mut() {
            if c.player.is_some() && !c.lag_flagged {
                let silent = now.duration_since(c.last_seen);
                if silent > LAG_THRESHOLD {
                    c.lag_flagged = true;
                    tracing::warn!(
                        conn = conn_id,
                        peer = %c.peer,
                        silent_ms = silent.as_millis() as u64,
                        "client lagging: no input or keepalive received"
                    );
                }
            }
        }

        // Timeouts (dead peers whose TCP hasn't reset yet).
        let timeout = Duration::from_secs(CLIENT_TIMEOUT_SECS);
        let stale: Vec<u64> = conns
            .iter()
            .filter(|(_, c)| now.duration_since(c.last_seen) > timeout)
            .map(|(&id, _)| id)
            .collect();
        for conn_id in stale {
            tracing::info!("conn {conn_id}: timed out");
            remove_conn(conn_id, &mut conns);
        }

        // Broadcast snapshot.
        let mut players: Vec<PlayerState> = conns
            .values()
            .filter_map(|c| c.player.as_ref())
            .map(|p| PlayerState {
                id: p.id,
                pos: p.pos,
                vel: p.vel,
            })
            .collect();
        players.sort_by_key(|p| p.id);
        let snapshot = ServerMsg::Snapshot { tick, players };
        let dead: Vec<u64> = conns
            .iter()
            .filter(|(_, c)| {
                c.player.is_some()
                    && match c.tx.try_send(snapshot.clone()) {
                        Ok(()) => false,
                        // Full = client stopped reading ~4 s ago; both cases
                        // mean this connection is done.
                        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => true,
                    }
            })
            .map(|(&id, _)| id)
            .collect();
        for conn_id in dead {
            remove_conn(conn_id, &mut conns);
        }

        // Tick-overrun detection: how long the tick body actually took vs
        // its 16.7 ms budget. Consistent overruns mean the sim can't hold
        // 60 Hz (the stall warn above catches the catastrophic version).
        let busy = tick_started.elapsed();
        if busy > max_busy {
            max_busy = busy;
        }
        if busy > tick_dt {
            overruns += 1;
        }

        if last_report.elapsed() > Duration::from_secs(10) {
            last_report = Instant::now();
            let joined = conns.values().filter(|c| c.player.is_some()).count();
            let lagging = conns.values().filter(|c| c.lag_flagged).count();
            tracing::info!(
                tick,
                players = joined,
                connections = conns.len(),
                lagging,
                max_tick_busy_us = max_busy.as_micros() as u64,
                tick_overruns = overruns,
                "server health"
            );
            max_busy = Duration::ZERO;
            overruns = 0;
        }
    }
}

fn handle_event(
    ev: Event,
    conns: &mut HashMap<u64, Conn>,
    next_player_id: &mut u32,
    cfg: &ServerConfig,
    events_tx: &Sender<Event>,
) {
    match ev {
        Event::Connected { conn, stream, peer } => {
            // Admission cap BEFORE any thread is spawned for this socket.
            let conn_cap = cfg.max_players * 2 + 16;
            if conns.len() >= conn_cap {
                tracing::warn!("conn {conn} ({peer}): connection cap {conn_cap} reached, refusing");
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
            let (reader_stream, sock) = match (stream.try_clone(), stream.try_clone()) {
                (Ok(r), Ok(s)) => (r, s),
                _ => {
                    tracing::warn!("conn {conn} ({peer}): socket clone failed");
                    return;
                }
            };
            let tx = spawn_writer(stream);
            conns.insert(
                conn,
                Conn {
                    tx,
                    sock,
                    peer: peer.clone(),
                    player: None,
                    last_seen: Instant::now(),
                    lag_flagged: false,
                },
            );
            spawn_reader(conn, reader_stream, events_tx.clone());
            tracing::info!("conn {conn}: accepted from {peer}");
        }
        Event::Msg { conn, msg } => {
            let Some(c) = conns.get_mut(&conn) else {
                return;
            };
            if c.lag_flagged {
                c.lag_flagged = false;
                tracing::info!(
                    conn,
                    silent_ms = c.last_seen.elapsed().as_millis() as u64,
                    "client recovered from lag"
                );
            }
            // Only a connection that has completed Hello may refresh its
            // liveness: pre-Hello traffic is rejected below, and refreshing
            // first would let it park an admission slot for as long as it
            // kept sending.
            if c.player.is_some() {
                c.last_seen = Instant::now();
            }
            match (msg, c.player.is_some()) {
                (ClientMsg::Hello { protocol, name }, false) => {
                    if protocol != PROTOCOL_VERSION {
                        let _ = c.tx.try_send(ServerMsg::Reject {
                            reason: format!(
                                "protocol mismatch: server v{PROTOCOL_VERSION}, client v{protocol}"
                            ),
                        });
                        remove_conn(conn, conns);
                        return;
                    }
                    let joined = conns.values().filter(|c| c.player.is_some()).count();
                    if joined >= cfg.max_players {
                        let c = conns.get_mut(&conn).unwrap();
                        let _ = c.tx.try_send(ServerMsg::Reject {
                            reason: "server full".into(),
                        });
                        remove_conn(conn, conns);
                        return;
                    }
                    let id = PlayerId(*next_player_id);
                    *next_player_id += 1;
                    let name = sanitize_name(&name);
                    let color = color_for(id);
                    // Deterministic, spread-out spawn ring.
                    let angle = id.0 as f32 * 2.399963;
                    let radius = 6.0 + (id.0 % 4) as f32 * 2.0;
                    let spawn = [angle.cos() * radius, angle.sin() * radius];
                    let player = Player {
                        id,
                        name: name.clone(),
                        color,
                        pos: spawn,
                        vel: [0.0, 0.0],
                        dir: [0.0, 0.0],
                    };
                    let meta = PlayerMeta {
                        id,
                        name: name.clone(),
                        color,
                        pos: spawn,
                    };

                    let c = conns.get_mut(&conn).unwrap();
                    c.player = Some(player);
                    let roster: Vec<PlayerMeta> = conns
                        .values()
                        .filter_map(|c| c.player.as_ref())
                        .map(|p| PlayerMeta {
                            id: p.id,
                            name: p.name.clone(),
                            color: p.color,
                            pos: p.pos,
                        })
                        .collect();
                    let c = conns.get_mut(&conn).unwrap();
                    let _ = c.tx.try_send(ServerMsg::Welcome {
                        id,
                        tick_hz: TICK_HZ,
                        arena_half: ARENA_HALF,
                        roster,
                    });
                    for (&other_id, other) in conns.iter() {
                        if other_id != conn && other.player.is_some() {
                            let _ = other
                                .tx
                                .try_send(ServerMsg::PlayerJoined { meta: meta.clone() });
                        }
                    }
                    tracing::info!("conn {conn}: joined as {:?} \"{name}\"", id);
                }
                (ClientMsg::Hello { .. }, true) => {
                    tracing::warn!("conn {conn}: duplicate Hello, dropping");
                    remove_conn(conn, conns);
                }
                // Anything else before Hello — Ping included — is a protocol
                // violation: an unauthenticated peer must not be served, and
                // a pre-Hello ping loop could otherwise park an admission
                // slot indefinitely by refreshing `last_seen` forever.
                (_, false) => {
                    tracing::warn!("conn {conn}: message before Hello, dropping");
                    remove_conn(conn, conns);
                }
                (ClientMsg::Input { move_dir }, true) => {
                    if let Some(p) = c.player.as_mut() {
                        p.dir = sanitize_dir(move_dir);
                    }
                }
                (ClientMsg::Ping { nonce }, true) => {
                    // A peer that pings but never drains its socket fills the
                    // queue; treat a full queue as a dead connection.
                    if c.tx.try_send(ServerMsg::Pong { nonce }).is_err() {
                        remove_conn(conn, conns);
                    }
                }
                (ClientMsg::Bye, true) => {
                    remove_conn(conn, conns);
                }
            }
        }
        Event::Disconnected { conn } => {
            remove_conn(conn, conns);
        }
    }
}

fn remove_conn(conn: u64, conns: &mut HashMap<u64, Conn>) {
    let Some(c) = conns.remove(&conn) else { return };
    // Unblock the reader thread immediately; dropping c.tx then ends the
    // writer thread after it drains (any final Reject still goes out), and
    // the writer's own WRITE_TIMEOUT bounds its lifetime even if the peer
    // has stopped reading.
    let _ = c.sock.shutdown(Shutdown::Read);
    if let Some(p) = c.player {
        tracing::info!("conn {conn} ({}): {:?} \"{}\" left", c.peer, p.id, p.name);
        for other in conns.values() {
            if other.player.is_some() {
                let _ = other.tx.try_send(ServerMsg::PlayerLeft { id: p.id });
            }
        }
    }
}
