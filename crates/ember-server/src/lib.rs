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
use std::io::Read;
use std::net::{IpAddr, Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use ember_net::{
    ARENA_HALF, CLIENT_TIMEOUT_SECS, ClientMsg, MOVE_SPEED, PROTOCOL_VERSION, PlayerId, PlayerMeta,
    PlayerState, ServerMsg, TICK_HZ, color_for, read_msg, sanitize_dir, sanitize_name, write_msg,
};

pub struct ServerConfig {
    pub max_players: usize,
    /// Cap on simultaneous connections from one remote IP. Without it a
    /// single host can occupy the whole global admission cap.
    pub max_conns_per_ip: usize,
    /// Whether the per-IP cap also applies to loopback peers. Off by
    /// default: the deployment binds to the `WireGuard` address, so a
    /// loopback peer is local tooling (netbot, a second dev client) rather
    /// than a stranger. Tests turn it on to exercise the cap, which is
    /// otherwise unreachable in-process.
    pub cap_loopback: bool,
    /// Total time one complete client message may take to arrive (see
    /// `DeadlineReader`). Configurable only so tests can shorten it; the
    /// default is the deployment value.
    pub frame_deadline: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_players: 32,
            max_conns_per_ip: 6,
            cap_loopback: false,
            frame_deadline: FRAME_DEADLINE,
        }
    }
}

enum Event {
    Connected {
        conn: u64,
        stream: TcpStream,
        peer: String,
        /// Resolved once on accept; `None` if the peer address was already
        /// unavailable, in which case the per-IP cap cannot apply.
        ip: Option<IpAddr>,
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
/// hard `CLIENT_TIMEOUT_SECS` kick — clients keepalive every ~2 s or less).
const LAG_THRESHOLD: Duration = Duration::from_secs(3);

/// Sustained ceiling on client messages processed per connection per tick.
/// An honest client sends one Input per frame plus an occasional Ping, so
/// this is a wide margin; beyond it the connection is dropped as a flooder.
/// Without it one post-Hello peer can dominate the shared event channel.
const MSGS_PER_TICK_LIMIT: u32 = 30;

/// Ceiling the per-connection message budget refills to. The budget is a
/// token bucket rather than a per-tick counter for one reason: after a sim
/// stall the catch-up drain delivers several ticks' worth of a client's
/// messages in a single pass, and that backlog is the server's fault, not
/// the client's. Eight ticks of slack absorbs it; the sustained rate above
/// still holds, because refill is what bounds the average.
const MSG_BURST: u32 = MSGS_PER_TICK_LIMIT * 8;

/// Per-`read` syscall timeout on a client socket. Short so the reader
/// thread returns to its own frame deadline promptly instead of parking in
/// the kernel; it is NOT itself the idle bound (see `FRAME_DEADLINE`).
const READ_POLL: Duration = Duration::from_millis(250);

/// Total time one complete message may take, measured from when the reader
/// starts waiting for it. A plain socket read timeout does not bound this:
/// `read_exact` restarts it on every byte that arrives, so a peer dribbling
/// one byte per window holds its reader and writer threads indefinitely.
/// Sits just above `CLIENT_TIMEOUT_SECS` so the sim thread's own sweep
/// normally wins the race and logs the kick — this is the backstop for when
/// the sim thread cannot act.
const FRAME_DEADLINE: Duration = Duration::from_secs(CLIENT_TIMEOUT_SECS + 2);

struct Conn {
    tx: SyncSender<ServerMsg>,
    /// Kept solely so `remove_conn` can unblock the reader thread; the
    /// writer half shuts the socket down fully when it exits.
    sock: TcpStream,
    peer: String,
    /// The peer's IP, for the per-IP admission cap. Counting live entries
    /// of `conns` is what enforces that cap, so no side table can drift out
    /// of sync with reality: a slot is released exactly when the connection
    /// is removed.
    ip: Option<IpAddr>,
    player: Option<Player>,
    last_seen: Instant,
    /// True while this client is flagged as lagging; cleared on any message.
    lag_flagged: bool,
    /// Token bucket for the message-rate cap: spent per message received,
    /// refilled `MSGS_PER_TICK_LIMIT` per tick up to `MSG_BURST`.
    msg_budget: u32,
}

/// Runs the server on an already-bound listener. Blocks forever.
/// Taking a listener (instead of an address) lets tests bind port 0.
///
/// # Errors
///
/// Returns an error if the listener's local address is unavailable or the
/// accept thread disconnects from the simulation loop.
// The public API intentionally retains its established ownership-taking signature.
#[allow(clippy::needless_pass_by_value)]
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
                let (peer, ip) = match stream.peer_addr() {
                    Ok(a) => (a.to_string(), Some(a.ip())),
                    Err(_) => ("?".to_string(), None),
                };
                let _ = stream.set_nodelay(true);
                let conn = next_conn;
                next_conn += 1;
                if events_tx
                    .send(Event::Connected {
                        conn,
                        stream,
                        peer,
                        ip,
                    })
                    .is_err()
                {
                    break; // sim thread gone
                }
            }
        });
    }

    sim_loop(&events_tx, &events_rx, &cfg)
}

/// A `Read` adapter that fails the whole read once `deadline` passes, no
/// matter how the peer paces its bytes.
///
/// This is the piece a socket read timeout cannot provide. `read_msg` reads
/// a frame with `read_exact`, which loops over `read` syscalls, and the
/// socket timeout applies per syscall — every byte that arrives restarts it.
/// A peer sending one byte per window therefore keeps a reader thread (and,
/// via the connection, a writer thread and an admission slot) alive forever
/// under a plain timeout. Checking a deadline the peer cannot influence
/// closes that.
struct DeadlineReader<'a> {
    inner: &'a mut TcpStream,
    deadline: Instant,
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if Instant::now() >= self.deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "frame deadline exceeded",
                ));
            }
            match self.inner.read(buf) {
                // The socket's own poll timeout: no bytes yet, so loop back
                // to the deadline check. Unix reports WouldBlock here and
                // Windows TimedOut, hence both.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                other => return other,
            }
        }
    }
}

fn spawn_reader(conn: u64, stream: TcpStream, events_tx: Sender<Event>, frame_deadline: Duration) {
    thread::spawn(move || {
        let mut stream = stream;
        // Makes the deadline below enforceable: without it a read parks in
        // the kernel indefinitely and the deadline is never consulted. If
        // it cannot be set, the sim thread's timeout sweep is still the
        // outer bound, so this is a degradation, not a failure.
        let _ = stream.set_read_timeout(Some(READ_POLL));
        // Ends on EOF, reset, protocol garbage, or the frame deadline.
        loop {
            let mut reader = DeadlineReader {
                inner: &mut stream,
                deadline: Instant::now() + frame_deadline,
            };
            let Ok(msg) = read_msg::<_, ClientMsg>(&mut reader) else {
                break;
            };
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
    events_tx: &Sender<Event>,
    events_rx: &Receiver<Event>,
    cfg: &ServerConfig,
) -> io::Result<()> {
    let tick_dt = Duration::from_nanos(1_000_000_000 / u64::from(TICK_HZ));
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
        drain_events(
            events_tx,
            events_rx,
            &mut conns,
            &mut next_player_id,
            cfg,
            next_tick_at,
        )?;
        next_tick_at += tick_dt;
        // If we fell far behind (debugger pause, host stall), resync instead
        // of running a burst of catch-up ticks.
        let now = Instant::now();
        if now > next_tick_at + tick_dt * 10 {
            let behind = now.duration_since(next_tick_at);
            tracing::warn!(
                tick,
                behind_ms = duration_millis(behind),
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

        let now = Instant::now();
        maintain_connections(&mut conns, now);

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
                max_tick_busy_us = duration_micros(max_busy),
                tick_overruns = overruns,
                "server health"
            );
            max_busy = Duration::ZERO;
            overruns = 0;
        }
    }
}

fn drain_events(
    events_tx: &Sender<Event>,
    events_rx: &Receiver<Event>,
    conns: &mut HashMap<u64, Conn>,
    next_player_id: &mut u32,
    cfg: &ServerConfig,
    next_tick_at: Instant,
) -> io::Result<()> {
    // Drain events until the next tick deadline.
    loop {
        let now = Instant::now();
        let Some(wait) = next_tick_at.checked_duration_since(now) else {
            break;
        };
        match events_rx.recv_timeout(wait) {
            Ok(ev) => handle_event(ev, conns, next_player_id, cfg, events_tx),
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
            Ok(ev) => handle_event(ev, conns, next_player_id, cfg, events_tx),
            Err(_) => break,
        }
    }
    Ok(())
}

fn maintain_connections(conns: &mut HashMap<u64, Conn>, now: Instant) {
    // Flag joined clients before the hard timeout and refill each rate budget.
    for (&conn_id, c) in &mut *conns {
        c.msg_budget = (c.msg_budget + MSGS_PER_TICK_LIMIT).min(MSG_BURST);
        if c.player.is_some() && !c.lag_flagged {
            let silent = now.duration_since(c.last_seen);
            if silent > LAG_THRESHOLD {
                c.lag_flagged = true;
                tracing::warn!(
                    conn = conn_id,
                    peer = %c.peer,
                    silent_ms = duration_millis(silent),
                    "client lagging: no input or keepalive received"
                );
            }
        }
    }

    // Remove dead peers whose TCP connection has not reset yet.
    let timeout = Duration::from_secs(CLIENT_TIMEOUT_SECS);
    let stale: Vec<u64> = conns
        .iter()
        .filter(|(_, c)| now.duration_since(c.last_seen) > timeout)
        .map(|(&id, _)| id)
        .collect();
    for conn_id in stale {
        tracing::info!("conn {conn_id}: timed out");
        remove_conn(conn_id, conns);
    }
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn duration_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn handle_event(
    ev: Event,
    conns: &mut HashMap<u64, Conn>,
    next_player_id: &mut u32,
    cfg: &ServerConfig,
    events_tx: &Sender<Event>,
) {
    match ev {
        Event::Connected {
            conn,
            stream,
            peer,
            ip,
        } => handle_connected(conn, stream, &peer, ip, conns, cfg, events_tx),
        Event::Msg { conn, msg } => handle_message(conn, msg, conns, next_player_id, cfg),
        Event::Disconnected { conn } => {
            remove_conn(conn, conns);
        }
    }
}

fn handle_connected(
    conn: u64,
    stream: TcpStream,
    peer: &str,
    ip: Option<IpAddr>,
    conns: &mut HashMap<u64, Conn>,
    cfg: &ServerConfig,
    events_tx: &Sender<Event>,
) {
    // Admission cap BEFORE any thread is spawned for this socket.
    let conn_cap = cfg.max_players * 2 + 16;
    if conns.len() >= conn_cap {
        tracing::warn!("conn {conn} ({peer}): connection cap {conn_cap} reached, refusing");
        let _ = stream.shutdown(Shutdown::Both);
        return;
    }
    // Per-IP cap, so one host cannot occupy the global cap above. Counted
    // from the live map so crash-drop cleanup cannot leave a stale side table.
    if let Some(ip) = ip
        && (cfg.cap_loopback || !ip.is_loopback())
    {
        let from_ip = conns.values().filter(|c| c.ip == Some(ip)).count();
        if from_ip >= cfg.max_conns_per_ip {
            tracing::warn!(
                "conn {conn} ({peer}): per-ip cap {} reached, refusing",
                cfg.max_conns_per_ip
            );
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
    }
    let (Ok(reader_stream), Ok(sock)) = (stream.try_clone(), stream.try_clone()) else {
        tracing::warn!("conn {conn} ({peer}): socket clone failed");
        return;
    };
    let tx = spawn_writer(stream);
    conns.insert(
        conn,
        Conn {
            tx,
            sock,
            peer: peer.to_owned(),
            ip,
            player: None,
            last_seen: Instant::now(),
            lag_flagged: false,
            // One tick's worth to start: burst slack is earned over time.
            msg_budget: MSGS_PER_TICK_LIMIT,
        },
    );
    spawn_reader(conn, reader_stream, events_tx.clone(), cfg.frame_deadline);
    tracing::info!("conn {conn}: accepted from {peer}");
}

fn handle_message(
    conn: u64,
    msg: ClientMsg,
    conns: &mut HashMap<u64, Conn>,
    next_player_id: &mut u32,
    cfg: &ServerConfig,
) {
    let joined = {
        let Some(c) = conns.get_mut(&conn) else {
            return;
        };
        // Spend the rate budget before granting lag recovery, liveness, or a
        // simulation update, so a flooder gets no benefit from the attempt.
        if c.msg_budget == 0 {
            tracing::warn!(
                "conn {conn}: message flood (over {}/tick sustained), dropping",
                MSGS_PER_TICK_LIMIT
            );
            remove_conn(conn, conns);
            return;
        }
        c.msg_budget -= 1;
        if c.lag_flagged {
            c.lag_flagged = false;
            tracing::info!(
                conn,
                silent_ms = duration_millis(c.last_seen.elapsed()),
                "client recovered from lag"
            );
        }
        let joined = c.player.is_some();
        // Pre-Hello traffic must not refresh an unauthenticated admission slot.
        if joined {
            c.last_seen = Instant::now();
        }
        joined
    };

    match (msg, joined) {
        (ClientMsg::Hello { protocol, name }, false) => {
            handle_hello(conn, protocol, &name, conns, next_player_id, cfg);
        }
        (ClientMsg::Hello { .. }, true) => {
            tracing::warn!("conn {conn}: duplicate Hello, dropping");
            remove_conn(conn, conns);
        }
        // Anything else before Hello — Ping included — is a protocol
        // violation and must not retain an admission slot.
        (_, false) => {
            tracing::warn!("conn {conn}: message before Hello, dropping");
            remove_conn(conn, conns);
        }
        (ClientMsg::Input { move_dir }, true) => {
            if let Some(player) = conns.get_mut(&conn).and_then(|c| c.player.as_mut()) {
                player.dir = sanitize_dir(move_dir);
            }
        }
        (ClientMsg::Ping { nonce }, true) => {
            // A peer that pings but never drains its socket fills the queue;
            // treat a full queue as a dead connection.
            let send_failed = conns
                .get(&conn)
                .is_some_and(|c| c.tx.try_send(ServerMsg::Pong { nonce }).is_err());
            if send_failed {
                remove_conn(conn, conns);
            }
        }
        (ClientMsg::Bye, true) => {
            remove_conn(conn, conns);
        }
    }
}

fn handle_hello(
    conn: u64,
    protocol: u16,
    name: &str,
    conns: &mut HashMap<u64, Conn>,
    next_player_id: &mut u32,
    cfg: &ServerConfig,
) {
    if protocol != PROTOCOL_VERSION {
        if let Some(c) = conns.get(&conn) {
            let _ = c.tx.try_send(ServerMsg::Reject {
                reason: format!(
                    "protocol mismatch: server v{PROTOCOL_VERSION}, client v{protocol}"
                ),
            });
        }
        remove_conn(conn, conns);
        return;
    }
    let joined = conns.values().filter(|c| c.player.is_some()).count();
    if joined >= cfg.max_players {
        if let Some(c) = conns.get(&conn) {
            let _ = c.tx.try_send(ServerMsg::Reject {
                reason: "server full".into(),
            });
        }
        remove_conn(conn, conns);
        return;
    }

    let id = PlayerId(*next_player_id);
    *next_player_id += 1;
    let name = sanitize_name(name);
    let color = color_for(id);
    // Player IDs intentionally seed the frozen f32 wire position format.
    #[allow(clippy::cast_precision_loss)]
    let angle = id.0 as f32 * 2.399_963;
    let ring = f32::from(u8::try_from(id.0 % 4).unwrap_or_default());
    let radius = 6.0 + ring * 2.0;
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

    if let Some(c) = conns.get_mut(&conn) {
        c.player = Some(player);
    }
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
    if let Some(c) = conns.get(&conn) {
        let _ = c.tx.try_send(ServerMsg::Welcome {
            id,
            tick_hz: TICK_HZ,
            arena_half: ARENA_HALF,
            roster,
        });
    }
    for (&other_id, other) in &*conns {
        if other_id != conn && other.player.is_some() {
            let _ = other
                .tx
                .try_send(ServerMsg::PlayerJoined { meta: meta.clone() });
        }
    }
    tracing::info!("conn {conn}: joined as {:?} \"{name}\"", id);
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
