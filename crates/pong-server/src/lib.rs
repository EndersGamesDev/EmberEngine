//! Arena-shooter matchmaking + match server.
//!
//! Same architecture as ember-server: ONE hub thread owns all state
//! (connections, lobbies, running games) and ticks at 60 Hz; a thread per
//! connection translates WebSocket frames into events. Transport is
//! WebSocket + JSON (`pong_core::proto`) so browsers can join, with TLS
//! terminated by the tunnel in front.
//!
//! A lobby IS a running game: creating one starts the match with the host
//! inside, joiners drop straight in (up to 8), leavers drop out; the lobby
//! dies when the last player leaves.
//!
//! This listener sits behind a PUBLIC tunnel: every input is untrusted.
//! Bounded queues, connection caps, message-size caps, per-tick message
//! budgets, and string sanitization throughout.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use pong_core::proto::{
    BState, C2S, LobbyInfo, MAX_HANDLE_LEN, MAX_LOBBY_LEN, MAX_PASSWORD_LEN, PROTO_VERSION, PState,
    PlayerMeta, S2C, STATE_EVERY_TICKS, color_for, sanitize_text,
};
use pong_core::shooter::{ARENA_HALF, FIXED_DT, MAX_PLAYERS, PlayerIn, Sim};
use tungstenite::Message;
use tungstenite::protocol::WebSocketConfig;

pub struct ServerConfig {
    pub max_conns: usize,
    pub max_lobbies: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_conns: 128,
            max_lobbies: 64,
        }
    }
}

const OUTBOUND_QUEUE: usize = 256;
/// How long a peer may go silent before it is dropped. Generous on purpose:
/// the browser client keeps the connection alive from a timer, and a hidden
/// tab has its timers throttled to roughly one wake-up a minute, so anything
/// tighter evicts a player who merely alt-tabbed - and takes the lobby with
/// them when they were hosting. A genuinely dead peer closes its socket and
/// is reaped by that, not by this.
const CONN_TIMEOUT: Duration = Duration::from_secs(120);
/// A connection must complete its WS handshake within this, total — a
/// watchdog closes the socket otherwise (per-read timeouts alone let a
/// byte-dribbling client hold a slot forever).
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(10);
/// A connection must Hello within this after the handshake.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// A connection that is in no game may idle at most this long (covers
/// lobby browsers generously; blocks slot-parking forever).
const LOBBYLESS_TIMEOUT: Duration = Duration::from_secs(300);
/// Cap per remote IP for DIRECT exposure. Loopback is exempt: behind the
/// Cloudflare tunnel every peer is 127.0.0.1 (the edge applies its own
/// per-source protections there).
const MAX_CONNS_PER_IP: u32 = 6;
/// Hard ceiling on client messages processed per connection per tick;
/// beyond it the connection is dropped as a flooder.
const MSGS_PER_TICK_LIMIT: u32 = 30;
const MAX_WS_MESSAGE: usize = 16 * 1024;

enum Ev {
    Connected {
        id: u64,
        tx: SyncSender<Message>,
        peer: String,
    },
    Msg {
        id: u64,
        msg: C2S,
    },
    /// Measured WS ping round-trip for this connection (bounds its rewind).
    Rtt {
        id: u64,
        rtt_ms: u32,
    },
    Disconnected {
        id: u64,
    },
}

struct Conn {
    tx: SyncSender<Message>,
    peer: String,
    handle: Option<String>, // set by Hello
    /// Client protocol version from Hello. Listing is allowed for any
    /// version (the hub browses without loading the game); entering a
    /// game requires the current one.
    proto: u16,
    lobby: Option<String>,
    created: Instant,
    /// Since when this conn has been outside any game.
    lobbyless_since: Instant,
    last_seen: Instant,
    msgs_this_tick: u32,
    /// Measured transport RTT in sim ticks; floors how far back this
    /// client's `view_tick` claim may reach (anti "free 300 ms rewind").
    /// Generous until the first measurement arrives.
    rtt_ticks: u64,
}

/// A lobby IS a running shooter game.
struct Lobby {
    password: Option<String>,
    sim: Sim,
    seed: u64,
    /// Conn ids; [0] is the listed "host" (purely cosmetic after creation).
    members: Vec<u64>,
    /// conn id -> in-game player id
    pids: HashMap<u64, u8>,
    /// pid -> (latest intent, its client sequence number, the client view
    /// tick it claims, and the sim tick it arrived on).
    inputs: HashMap<u8, (PlayerIn, u32, u64, u64)>,
}

/// Smallest player id not held by a current member (never collides, unlike
/// a wrapping counter in a long-lived lobby).
fn alloc_pid(lobby: &Lobby) -> u8 {
    (0..u8::MAX)
        .find(|p| !lobby.pids.values().any(|v| v == p))
        .unwrap_or(0)
}

fn bounded_tick_age(current: u64, earlier: u64) -> u16 {
    u16::try_from(current.saturating_sub(earlier).min(u64::from(u16::MAX)))
        .expect("the tick age is clamped to u16::MAX")
}

// RTT values are finite and nonnegative, and the established float formula always fits u64.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn rtt_ticks(rtt_ms: u32) -> u64 {
    ((rtt_ms as f32 / 1000.0) * 60.0).ceil() as u64
}

/// Runs the arena server until its event channel closes.
///
/// # Errors
///
/// Returns an error when the listener's local address cannot be read or the internal event
/// channel disconnects.
///
/// # Panics
///
/// Panics if an internal connection/lobby bookkeeping invariant is violated or the per-IP
/// connection-count mutex is poisoned.
// This public entry point retains ownership of its configuration for API compatibility.
#[allow(clippy::needless_pass_by_value)]
pub fn run(listener: TcpListener, cfg: ServerConfig) -> io::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        "pong-server (arena shooter) listening on {local} (proto v{PROTO_VERSION}, max {} conns, {} lobbies)",
        cfg.max_conns,
        cfg.max_lobbies
    );

    let (events_tx, events_rx) = mpsc::channel::<Ev>();
    let live_conns = Arc::new(AtomicUsize::new(0));
    let per_ip: Arc<std::sync::Mutex<HashMap<std::net::IpAddr, u32>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));

    {
        let live_conns = Arc::clone(&live_conns);
        let per_ip = Arc::clone(&per_ip);
        let max_conns = cfg.max_conns;
        thread::spawn(move || {
            let mut next_id: u64 = 1;
            for stream in listener.incoming() {
                let stream = match stream {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("accept error: {e}");
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                };
                if live_conns.load(Ordering::Relaxed) >= max_conns {
                    tracing::warn!("connection cap reached, refusing peer");
                    continue; // stream drops -> RST/FIN
                }
                // Per-IP cap for direct exposure (loopback = tunnel, exempt).
                let ip = stream.peer_addr().ok().map(|a| a.ip());
                if let Some(ip) = ip
                    && !ip.is_loopback()
                {
                    let mut map = per_ip.lock().unwrap();
                    let count = map.entry(ip).or_insert(0);
                    if *count >= MAX_CONNS_PER_IP {
                        tracing::warn!(%ip, "per-ip cap reached, refusing peer");
                        continue;
                    }
                    *count += 1;
                }
                let id = next_id;
                next_id += 1;
                live_conns.fetch_add(1, Ordering::Relaxed);
                let events_tx = events_tx.clone();
                let live_conns = Arc::clone(&live_conns);
                let per_ip = Arc::clone(&per_ip);
                thread::spawn(move || {
                    conn_thread(id, stream, &events_tx);
                    live_conns.fetch_sub(1, Ordering::Relaxed);
                    if let Some(ip) = ip
                        && !ip.is_loopback()
                    {
                        let mut map = per_ip.lock().unwrap();
                        if let Some(c) = map.get_mut(&ip) {
                            *c -= 1;
                            if *c == 0 {
                                map.remove(&ip);
                            }
                        }
                    }
                });
            }
        });
    }

    hub_loop(&events_rx, &cfg)
}

/// Per-connection thread: WS handshake, then a single loop that alternates
/// draining the outbound queue and polling for one inbound frame (short
/// read timeout). One thread per conn keeps sync tungstenite simple.
// The handshake and I/O phases stay together so socket ownership and event ordering remain clear.
#[allow(clippy::too_many_lines)]
fn conn_thread(id: u64, stream: TcpStream, events_tx: &Sender<Ev>) {
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "?".into(), |a| a.to_string());
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    // A total handshake deadline: the per-read timeout alone lets a client
    // dribble one byte per window and hold the slot forever. The watchdog
    // closes the socket if the handshake hasn't finished in time.
    let handshake_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Ok(watch) = stream.try_clone() {
        let done = Arc::clone(&handshake_done);
        thread::spawn(move || {
            let step = Duration::from_millis(250);
            let mut waited = Duration::ZERO;
            while waited < HANDSHAKE_DEADLINE {
                thread::sleep(step);
                waited += step;
                if done.load(Ordering::Relaxed) {
                    return;
                }
            }
            let _ = watch.shutdown(std::net::Shutdown::Both);
        });
    }

    let ws_cfg = WebSocketConfig::default()
        .max_message_size(Some(MAX_WS_MESSAGE))
        .max_frame_size(Some(MAX_WS_MESSAGE));
    let mut ws = match tungstenite::accept_with_config(stream, Some(ws_cfg)) {
        Ok(ws) => ws,
        Err(e) => {
            handshake_done.store(true, Ordering::Relaxed);
            tracing::debug!(conn = id, peer = %peer, "handshake failed: {e}");
            return;
        }
    };
    handshake_done.store(true, Ordering::Relaxed);
    // This thread owns the socket, so the outbound queue is only drained
    // between reads - a 20 ms timeout meant a broadcast state sat up to 20 ms
    // before it hit the wire, and states measurably arrived 21-53 ms apart
    // instead of the sim's clean 33.3. 5 ms cuts that smear by four at 200
    // idle wake-ups a second per connection, which is nothing next to the
    // 60 Hz sim it is feeding.
    let _ = ws
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(5)));

    let (tx, rx) = mpsc::sync_channel::<Message>(OUTBOUND_QUEUE);
    if events_tx.send(Ev::Connected { id, tx, peer }).is_err() {
        return;
    }

    let mut last_ws_ping = Instant::now();
    let mut ping_sent_at: Option<Instant> = None;
    'outer: loop {
        // Measure transport RTT with WS pings (browsers and tungstenite
        // clients auto-pong); the hub uses it to bound lag-comp rewinds.
        if last_ws_ping.elapsed() >= Duration::from_secs(5) {
            last_ws_ping = Instant::now();
            ping_sent_at = Some(Instant::now());
            if ws
                .send(Message::Ping(tungstenite::Bytes::default()))
                .is_err()
            {
                break;
            }
        }
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    if ws.send(msg).is_err() {
                        break 'outer;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    break 'outer;
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => match serde_json::from_str::<C2S>(text.as_str()) {
                Ok(msg) => {
                    if events_tx.send(Ev::Msg { id, msg }).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::debug!(conn = id, "bad message ({e}); dropping");
                    break;
                }
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) => {}
            Ok(Message::Pong(_)) => {
                if let Some(t) = ping_sent_at.take() {
                    let rtt_ms = u32::try_from(t.elapsed().as_millis()).unwrap_or(u32::MAX);
                    if events_tx.send(Ev::Rtt { id, rtt_ms }).is_err() {
                        break;
                    }
                }
            }
            Ok(_) => {
                tracing::debug!(conn = id, "binary frame from client; dropping");
                break;
            }
            Err(tungstenite::Error::Io(e))
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
            }
            Err(_) => break,
        }
    }
    let _ = events_tx.send(Ev::Disconnected { id });
}

// The tick scheduler and broadcasts stay together to preserve their exact ordering.
#[allow(clippy::too_many_lines)]
fn hub_loop(events_rx: &Receiver<Ev>, cfg: &ServerConfig) -> io::Result<()> {
    let tick_dt = Duration::from_secs_f32(FIXED_DT);
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut lobbies: HashMap<String, Lobby> = HashMap::new();
    let mut lobby_counter: u64 = 0;
    let mut tick: u64 = 0;
    let mut next_tick_at = Instant::now() + tick_dt;
    let mut last_report = Instant::now();

    loop {
        loop {
            let now = Instant::now();
            let Some(wait) = next_tick_at.checked_duration_since(now) else {
                break;
            };
            match events_rx.recv_timeout(wait) {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, &mut lobby_counter, cfg),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::other("accept thread died"));
                }
            }
        }
        for _ in 0..1024 {
            match events_rx.try_recv() {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, &mut lobby_counter, cfg),
                Err(_) => break,
            }
        }
        next_tick_at += tick_dt;
        let now = Instant::now();
        if now > next_tick_at + tick_dt * 10 {
            let behind_ms =
                u64::try_from(now.duration_since(next_tick_at).as_millis()).unwrap_or(u64::MAX);
            tracing::warn!(tick, behind_ms, "hub stall: resyncing tick clock");
            next_tick_at = now + tick_dt;
        }
        tick += 1;

        // Step every running game and broadcast.
        let mut dead_conns: Vec<u64> = Vec::new();
        for lobby in lobbies.values_mut() {
            let inputs = &lobby.inputs;
            // This step runs as tick cur_tick + 1, and history frame
            // len-1-d resolves to tick (cur_tick+1)-d — so the delay must
            // be measured from the POST-step tick or every rewind lands one
            // tick after the view the client reported.
            let apply_tick = lobby.sim.tick + 1;
            lobby.sim.step(&|pid| {
                inputs
                    .get(&pid)
                    .map(|(i, _, view_tick, _)| {
                        let mut i = *i;
                        i.delay_ticks = bounded_tick_age(apply_tick, *view_tick);
                        i
                    })
                    .unwrap_or_default()
            });
            // A jump is a press, and this is where it is consumed. The last
            // input is re-applied every tick, so leaving the flag set makes a
            // held Space re-launch on every grounded tick - which is how you
            // bunny-hopped off a crate top onto a container.
            for (i, ..) in lobby.inputs.values_mut() {
                i.jump = false;
            }

            for &(killer, victim) in &lobby.sim.events {
                if let Ok(text) = serde_json::to_string(&S2C::Kill { killer, victim }) {
                    for &m in &lobby.members {
                        let _ = send_text_to(&conns, m, &text);
                    }
                }
            }
            if tick.is_multiple_of(STATE_EVERY_TICKS) {
                let state = S2C::State {
                    tick: lobby.sim.tick,
                    players: lobby
                        .sim
                        .players
                        .iter()
                        .map(|p| PState {
                            id: p.id,
                            x: p.pos[0],
                            z: p.pos[1],
                            y: p.y,
                            vy: p.vy,
                            ax: p.aim[0],
                            az: p.aim[1],
                            pitch: p.pitch,
                            hp: p.hp,
                            score: p.score,
                            alive: p.alive,
                            crouch: p.crouch,
                            shield: p.shield,
                            weapon: p.weapon,
                            ammo: p.ammo,
                            reloading: p.reload_t > 0.0,
                            deaths: p.death_count,
                            ack: lobby.inputs.get(&p.id).map_or(0, |(_, s, _, _)| *s),
                            ack_age_ticks: lobby
                                .inputs
                                .get(&p.id)
                                .map_or(0, |(_, _, _, recv_tick)| {
                                    bounded_tick_age(lobby.sim.tick, *recv_tick)
                                }),
                        })
                        .collect(),
                    bullets: lobby
                        .sim
                        .bullets
                        .iter()
                        .map(|b| BState {
                            x: b.pos[0],
                            z: b.pos[1],
                            vx: b.vel[0],
                            vz: b.vel[1],
                            y: b.y,
                            vy: b.vy,
                            owner: b.owner,
                        })
                        .collect(),
                    pads: lobby.sim.pads.iter().map(|p| p.respawn_t <= 0.0).collect(),
                };
                // Serialize once per lobby, not once per recipient.
                if let Ok(text) = serde_json::to_string(&state) {
                    for &m in &lobby.members {
                        if !send_text_to(&conns, m, &text) {
                            // Stalled pipe: drop the peer, the game plays on.
                            dead_conns.push(m);
                        }
                    }
                }
            }
        }
        for id in dead_conns {
            tracing::info!(conn = id, "outbound stalled; dropping");
            drop_conn(id, &mut conns, &mut lobbies);
        }

        // Reset flood counters; sweep silent, hello-less, and parked peers.
        let timeout_ids: Vec<u64> = conns
            .iter_mut()
            .filter_map(|(&id, c)| {
                c.msgs_this_tick = 0;
                let silent = now.duration_since(c.last_seen) > CONN_TIMEOUT;
                let no_hello = c.handle.is_none() && now.duration_since(c.created) > HELLO_TIMEOUT;
                let parked =
                    c.lobby.is_none() && now.duration_since(c.lobbyless_since) > LOBBYLESS_TIMEOUT;
                (silent || no_hello || parked).then_some(id)
            })
            .collect();
        for id in timeout_ids {
            tracing::info!(conn = id, "timed out (silent, hello-less, or parked)");
            drop_conn(id, &mut conns, &mut lobbies);
        }

        if last_report.elapsed() > Duration::from_secs(30) {
            last_report = Instant::now();
            let in_game: usize = lobbies.values().map(|l| l.members.len()).sum();
            tracing::info!(
                tick,
                connections = conns.len(),
                lobbies = lobbies.len(),
                players_in_game = in_game,
                "server health"
            );
        }
    }
}

fn send_to(conns: &HashMap<u64, Conn>, id: u64, msg: &S2C) -> bool {
    let Ok(text) = serde_json::to_string(msg) else {
        return false;
    };
    send_text_to(conns, id, &text)
}

fn send_text_to(conns: &HashMap<u64, Conn>, id: u64, text: &str) -> bool {
    let Some(c) = conns.get(&id) else {
        return false;
    };
    match c.tx.try_send(Message::text(text.to_owned())) {
        Ok(()) => true,
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
    }
}

fn roster(lobby: &Lobby, conns: &HashMap<u64, Conn>) -> Vec<PlayerMeta> {
    lobby
        .members
        .iter()
        .filter_map(|m| {
            let pid = *lobby.pids.get(m)?;
            Some(PlayerMeta {
                id: pid,
                handle: conns
                    .get(m)
                    .and_then(|c| c.handle.clone())
                    .unwrap_or_else(|| "?".into()),
                color: color_for(pid),
            })
        })
        .collect()
}

// Keeping protocol validation and state mutation in one dispatch preserves event ordering.
#[allow(clippy::too_many_lines)]
fn handle_event(
    ev: Ev,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    lobby_counter: &mut u64,
    cfg: &ServerConfig,
) {
    match ev {
        Ev::Connected { id, tx, peer } => {
            tracing::info!(conn = id, peer = %peer, "connected");
            let now = Instant::now();
            conns.insert(
                id,
                Conn {
                    tx,
                    peer,
                    handle: None,
                    proto: 0,
                    lobby: None,
                    created: now,
                    lobbyless_since: now,
                    last_seen: now,
                    msgs_this_tick: 0,
                    rtt_ticks: 18,
                },
            );
        }
        Ev::Rtt { id, rtt_ms } => {
            if let Some(c) = conns.get_mut(&id) {
                let ticks = rtt_ticks(rtt_ms);
                // Smooth toward the newest measurement, biased downward so a
                // one-off spike doesn't widen the rewind window for long.
                c.rtt_ticks = if ticks < c.rtt_ticks {
                    ticks
                } else {
                    u64::midpoint(c.rtt_ticks, ticks)
                };
            }
        }
        Ev::Disconnected { id } => {
            drop_conn(id, conns, lobbies);
        }
        Ev::Msg { id, msg } => {
            let Some(c) = conns.get_mut(&id) else { return };
            c.last_seen = Instant::now();
            c.msgs_this_tick += 1;
            if c.msgs_this_tick > MSGS_PER_TICK_LIMIT {
                tracing::warn!(conn = id, "message flood; dropping");
                drop_conn(id, conns, lobbies);
                return;
            }
            let has_handle = c.handle.is_some();
            match (msg, has_handle) {
                (C2S::Hello { proto, handle }, false) => {
                    // Any version may Hello and LIST (the hub browses
                    // lobbies without the game loaded); Create/Join below
                    // enforce the current protocol.
                    let handle = {
                        let h = sanitize_text(&handle, MAX_HANDLE_LEN);
                        if h.is_empty() {
                            "player".to_string()
                        } else {
                            h
                        }
                    };
                    let c = conns.get_mut(&id).unwrap();
                    c.handle = Some(handle);
                    c.proto = proto;
                    let _ = send_to(
                        conns,
                        id,
                        &S2C::Welcome {
                            proto: PROTO_VERSION,
                            motd: "ember arena — cubes with guns".into(),
                        },
                    );
                }
                (C2S::Hello { .. }, true) => {
                    tracing::debug!(conn = id, "duplicate Hello; dropping");
                    drop_conn(id, conns, lobbies);
                }
                // Anything else before Hello — Ping included — is a
                // protocol violation (pre-Hello pings could park a slot).
                (_, false) => {
                    tracing::debug!(conn = id, "message before Hello; dropping");
                    drop_conn(id, conns, lobbies);
                }
                (C2S::Ping { nonce }, true) => {
                    if !send_to(conns, id, &S2C::Pong { nonce }) {
                        drop_conn(id, conns, lobbies);
                    }
                }
                (C2S::ListLobbies, true) => {
                    let list: Vec<LobbyInfo> = lobbies
                        .iter()
                        .filter(|(_, l)| l.members.len() < MAX_PLAYERS)
                        .map(|(name, l)| LobbyInfo {
                            name: name.clone(),
                            host: l
                                .members
                                .first()
                                .and_then(|m| conns.get(m))
                                .and_then(|c| c.handle.clone())
                                .unwrap_or_else(|| "?".into()),
                            has_password: l.password.is_some(),
                            players: u8::try_from(l.members.len())
                                .expect("lobby membership is capped below u8::MAX"),
                            cap: u8::try_from(MAX_PLAYERS).expect("MAX_PLAYERS fits in u8"),
                        })
                        .collect();
                    let _ = send_to(conns, id, &S2C::LobbyList { lobbies: list });
                }
                (C2S::CreateLobby { name, password }, true) => {
                    if conns.get(&id).unwrap().proto != PROTO_VERSION {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: format!(
                                    "this build speaks protocol v{}, the live game is v{PROTO_VERSION} — play the live version",
                                    conns.get(&id).unwrap().proto
                                ),
                            },
                        );
                        return;
                    }
                    let name = sanitize_text(&name, MAX_LOBBY_LEN);
                    let password = password
                        .map(|p| sanitize_text(&p, MAX_PASSWORD_LEN))
                        .filter(|p| !p.is_empty());
                    if name.is_empty() {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: "lobby name must not be empty".into(),
                            },
                        );
                        return;
                    }
                    if conns.get(&id).unwrap().lobby.is_some() {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: "already in a game".into(),
                            },
                        );
                        return;
                    }
                    if lobbies.contains_key(&name) {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: format!("lobby \"{name}\" already exists"),
                            },
                        );
                        return;
                    }
                    if lobbies.len() >= cfg.max_lobbies {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: "server is full of lobbies, try later".into(),
                            },
                        );
                        return;
                    }
                    *lobby_counter += 1;
                    let mut hasher = DefaultHasher::new();
                    name.hash(&mut hasher);
                    lobby_counter.hash(&mut hasher);
                    let seed = hasher.finish();

                    let mut lobby = Lobby {
                        password,
                        sim: Sim::new(seed),
                        seed,
                        members: vec![id],
                        pids: HashMap::new(),
                        inputs: HashMap::new(),
                    };
                    let pid = alloc_pid(&lobby);
                    lobby.pids.insert(id, pid);
                    lobby.sim.add_player(pid);
                    conns.get_mut(&id).unwrap().lobby = Some(name.clone());
                    let joined = S2C::GameJoined {
                        id: pid,
                        seed,
                        arena_half: ARENA_HALF,
                        players: roster(&lobby, conns),
                    };
                    lobbies.insert(name.clone(), lobby);
                    tracing::info!(conn = id, lobby = %name, "game created");
                    let _ = send_to(conns, id, &joined);
                }
                (C2S::JoinLobby { name, password }, true) => {
                    if conns.get(&id).unwrap().proto != PROTO_VERSION {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: format!(
                                    "this build speaks protocol v{}, the live game is v{PROTO_VERSION} — play the live version",
                                    conns.get(&id).unwrap().proto
                                ),
                            },
                        );
                        return;
                    }
                    let name = sanitize_text(&name, MAX_LOBBY_LEN);
                    if conns.get(&id).unwrap().lobby.is_some() {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: "already in a game".into(),
                            },
                        );
                        return;
                    }
                    let Some(lobby) = lobbies.get_mut(&name) else {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: format!("no lobby named \"{name}\""),
                            },
                        );
                        return;
                    };
                    if lobby.members.len() >= MAX_PLAYERS {
                        let _ = send_to(
                            conns,
                            id,
                            &S2C::Error {
                                message: "game is full".into(),
                            },
                        );
                        return;
                    }
                    if let Some(expected) = &lobby.password {
                        let given = password
                            .map(|p| sanitize_text(&p, MAX_PASSWORD_LEN))
                            .unwrap_or_default();
                        if &given != expected {
                            let _ = send_to(
                                conns,
                                id,
                                &S2C::Error {
                                    message: "wrong password".into(),
                                },
                            );
                            return;
                        }
                    }
                    let pid = alloc_pid(lobby);
                    lobby.members.push(id);
                    lobby.pids.insert(id, pid);
                    lobby.sim.add_player(pid);
                    conns.get_mut(&id).unwrap().lobby = Some(name.clone());

                    let handle = conns
                        .get(&id)
                        .and_then(|c| c.handle.clone())
                        .unwrap_or_else(|| "?".into());
                    let meta = PlayerMeta {
                        id: pid,
                        handle: handle.clone(),
                        color: color_for(pid),
                    };
                    let joined = S2C::GameJoined {
                        id: pid,
                        seed: lobby.seed,
                        arena_half: ARENA_HALF,
                        players: roster(lobby, conns),
                    };
                    let others: Vec<u64> =
                        lobby.members.iter().copied().filter(|&m| m != id).collect();
                    tracing::info!(conn = id, lobby = %name, player = %handle, "joined game");
                    let _ = send_to(conns, id, &joined);
                    for m in others {
                        let _ = send_to(conns, m, &S2C::PlayerJoined { meta: meta.clone() });
                    }
                }
                (C2S::LeaveLobby, true) => {
                    leave_lobby(id, conns, lobbies);
                }
                (
                    C2S::Input {
                        seq,
                        view_tick,
                        mx,
                        my,
                        ax,
                        az,
                        pitch,
                        fire,
                        sprint,
                        crouch,
                        reload,
                        jump,
                        shield,
                    },
                    true,
                ) => {
                    let conn = conns.get(&id).unwrap();
                    let rtt_ticks = conn.rtt_ticks;
                    let Some(lobby_name) = conn.lobby.clone() else {
                        return;
                    };
                    let Some(lobby) = lobbies.get_mut(&lobby_name) else {
                        return;
                    };
                    let Some(&pid) = lobby.pids.get(&id) else {
                        return;
                    };
                    // The sim sanitizes magnitudes/NaN on use. view_tick is
                    // clamped above by the present and FLOORED by what this
                    // connection's measured latency can justify (one-way +
                    // interpolation + jitter slack) — a zero-ping client
                    // cannot claim a free 300 ms rewind.
                    let allowed_delay = rtt_ticks / 2 + 6;
                    let floor = lobby.sim.tick.saturating_sub(allowed_delay);
                    // A press is an event, not a level. Two input frames can
                    // land in the same inter-tick window - the hub drains
                    // every queued event before it steps - and a plain
                    // overwrite would destroy the earlier one's jump before
                    // the sim ever saw it. Merge; the post-step clear still
                    // makes it fire exactly once.
                    let jump = jump || lobby.inputs.get(&pid).is_some_and(|(i, ..)| i.jump);
                    lobby.inputs.insert(
                        pid,
                        (
                            PlayerIn {
                                mv: [mx, my],
                                aim: [ax, az],
                                pitch,
                                fire,
                                sprint,
                                crouch,
                                reload,
                                jump,
                                shield,
                                delay_ticks: 0,
                            },
                            seq,
                            view_tick.clamp(floor, lobby.sim.tick),
                            lobby.sim.tick,
                        ),
                    );
                }
            }
        }
    }
}

/// Remove a connection entirely (disconnect/timeout/violation).
fn drop_conn(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    leave_lobby(id, conns, lobbies);
    // Removing the connection drops its sender and makes the socket thread close.
    if let Some(c) = conns.remove(&id)
        && let Some(h) = &c.handle
    {
        tracing::info!(conn = id, peer = %c.peer, handle = %h, "disconnected");
    }
}

/// Take a player out of their game; the game keeps running for the rest,
/// and the lobby dies when the last player leaves.
fn leave_lobby(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let Some(name) = conns.get_mut(&id).and_then(|c| c.lobby.take()) else {
        return;
    };
    // Reset the parked timer only when a lobby was actually left — a bare
    // LeaveLobby spam must not refresh the sweep deadline.
    if let Some(c) = conns.get_mut(&id) {
        c.lobbyless_since = Instant::now();
    }
    let Some(lobby) = lobbies.get_mut(&name) else {
        return;
    };

    lobby.members.retain(|&m| m != id);
    if let Some(pid) = lobby.pids.remove(&id) {
        lobby.inputs.remove(&pid);
        lobby.sim.remove_player(pid);
        for &m in &lobby.members {
            let _ = send_to(conns, m, &S2C::PlayerLeft { id: pid });
        }
    }
    if lobby.members.is_empty() {
        tracing::info!(lobby = %name, "game closed (last player left)");
        lobbies.remove(&name);
    }
}
