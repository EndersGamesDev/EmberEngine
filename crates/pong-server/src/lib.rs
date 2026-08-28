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

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use pong_core::proto::{
    color_for, sanitize_text, BState, C2S, LobbyInfo, PState, PlayerMeta, S2C, MAX_HANDLE_LEN,
    MAX_LOBBY_LEN, MAX_PASSWORD_LEN, PROTO_VERSION, STATE_EVERY_TICKS,
};
use pong_core::shooter::{PlayerIn, Sim, ARENA_HALF, FIXED_DT, MAX_PLAYERS};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::Message;

pub struct ServerConfig {
    pub max_conns: usize,
    pub max_lobbies: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { max_conns: 128, max_lobbies: 64 }
    }
}

const OUTBOUND_QUEUE: usize = 256;
const CONN_TIMEOUT: Duration = Duration::from_secs(30);
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
    Connected { id: u64, tx: SyncSender<Message>, peer: String },
    Msg { id: u64, msg: C2S },
    Disconnected { id: u64 },
}

struct Conn {
    tx: SyncSender<Message>,
    peer: String,
    handle: Option<String>, // set by Hello
    lobby: Option<String>,
    created: Instant,
    /// Since when this conn has been outside any game.
    lobbyless_since: Instant,
    last_seen: Instant,
    msgs_this_tick: u32,
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
    inputs: HashMap<u8, PlayerIn>,
    next_pid: u8,
}

pub fn run(listener: TcpListener, cfg: ServerConfig) -> io::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        "pong-server (arena shooter) listening on {local} (proto v{PROTO_VERSION}, max {} conns, {} lobbies)",
        cfg.max_conns, cfg.max_lobbies
    );

    let (events_tx, events_rx) = mpsc::channel::<Ev>();
    let live_conns = Arc::new(AtomicUsize::new(0));
    let per_ip: Arc<std::sync::Mutex<HashMap<std::net::IpAddr, u32>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));

    {
        let events_tx = events_tx.clone();
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
                if let Some(ip) = ip {
                    if !ip.is_loopback() {
                        let mut map = per_ip.lock().unwrap();
                        let count = map.entry(ip).or_insert(0);
                        if *count >= MAX_CONNS_PER_IP {
                            tracing::warn!(%ip, "per-ip cap reached, refusing peer");
                            continue;
                        }
                        *count += 1;
                    }
                }
                let id = next_id;
                next_id += 1;
                live_conns.fetch_add(1, Ordering::Relaxed);
                let events_tx = events_tx.clone();
                let live_conns = Arc::clone(&live_conns);
                let per_ip = Arc::clone(&per_ip);
                thread::spawn(move || {
                    conn_thread(id, stream, events_tx);
                    live_conns.fetch_sub(1, Ordering::Relaxed);
                    if let Some(ip) = ip {
                        if !ip.is_loopback() {
                            let mut map = per_ip.lock().unwrap();
                            if let Some(c) = map.get_mut(&ip) {
                                *c -= 1;
                                if *c == 0 {
                                    map.remove(&ip);
                                }
                            }
                        }
                    }
                });
            }
        });
    }

    hub_loop(events_rx, cfg)
}

/// Per-connection thread: WS handshake, then a single loop that alternates
/// draining the outbound queue and polling for one inbound frame (short
/// read timeout). One thread per conn keeps sync tungstenite simple.
fn conn_thread(id: u64, stream: TcpStream, events_tx: Sender<Ev>) {
    let peer = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "?".into());
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
    let _ = ws.get_ref().set_read_timeout(Some(Duration::from_millis(20)));

    let (tx, rx) = mpsc::sync_channel::<Message>(OUTBOUND_QUEUE);
    if events_tx
        .send(Ev::Connected { id, tx, peer: peer.clone() })
        .is_err()
    {
        return;
    }

    'outer: loop {
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
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            Ok(_) => {
                tracing::debug!(conn = id, "binary frame from client; dropping");
                break;
            }
            Err(tungstenite::Error::Io(e))
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
    let _ = events_tx.send(Ev::Disconnected { id });
}

fn hub_loop(events_rx: Receiver<Ev>, cfg: ServerConfig) -> io::Result<()> {
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
            let Some(wait) = next_tick_at.checked_duration_since(now) else { break };
            match events_rx.recv_timeout(wait) {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, &mut lobby_counter, &cfg),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(io::ErrorKind::Other, "accept thread died"));
                }
            }
        }
        for _ in 0..1024 {
            match events_rx.try_recv() {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, &mut lobby_counter, &cfg),
                Err(_) => break,
            }
        }
        next_tick_at += tick_dt;
        let now = Instant::now();
        if now > next_tick_at + tick_dt * 10 {
            tracing::warn!(
                tick,
                behind_ms = now.duration_since(next_tick_at).as_millis() as u64,
                "hub stall: resyncing tick clock"
            );
            next_tick_at = now + tick_dt;
        }
        tick += 1;

        // Step every running game and broadcast.
        let mut dead_conns: Vec<u64> = Vec::new();
        for lobby in lobbies.values_mut() {
            let inputs = &lobby.inputs;
            lobby.sim.step(&|pid| inputs.get(&pid).copied().unwrap_or_default());

            for &(killer, victim) in &lobby.sim.events {
                let msg = S2C::Kill { killer, victim };
                for &m in &lobby.members {
                    let _ = send_to(&conns, m, &msg);
                }
            }
            if tick % STATE_EVERY_TICKS == 0 {
                let state = S2C::State {
                    tick,
                    players: lobby
                        .sim
                        .players
                        .iter()
                        .map(|p| PState {
                            id: p.id,
                            x: p.pos[0],
                            z: p.pos[1],
                            ax: p.aim[0],
                            az: p.aim[1],
                            hp: p.hp,
                            score: p.score,
                            alive: p.alive,
                        })
                        .collect(),
                    bullets: lobby
                        .sim
                        .bullets
                        .iter()
                        .map(|b| BState { x: b.pos[0], z: b.pos[1], vx: b.vel[0], vz: b.vel[1] })
                        .collect(),
                };
                for &m in &lobby.members {
                    if !send_to(&conns, m, &state) {
                        // Stalled pipe: drop the peer, the game plays on.
                        dead_conns.push(m);
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
                let no_hello =
                    c.handle.is_none() && now.duration_since(c.created) > HELLO_TIMEOUT;
                let parked = c.lobby.is_none()
                    && now.duration_since(c.lobbyless_since) > LOBBYLESS_TIMEOUT;
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
    let Some(c) = conns.get(&id) else { return false };
    let text = match serde_json::to_string(msg) {
        Ok(t) => t,
        Err(_) => return false,
    };
    match c.tx.try_send(Message::text(text)) {
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
                    lobby: None,
                    created: now,
                    lobbyless_since: now,
                    last_seen: now,
                    msgs_this_tick: 0,
                },
            );
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
                    if proto != PROTO_VERSION {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: format!(
                                "protocol mismatch: server v{PROTO_VERSION}, client v{proto} — reload the page"
                            ),
                        });
                        drop_conn(id, conns, lobbies);
                        return;
                    }
                    let handle = {
                        let h = sanitize_text(&handle, MAX_HANDLE_LEN);
                        if h.is_empty() { "player".to_string() } else { h }
                    };
                    let c = conns.get_mut(&id).unwrap();
                    c.handle = Some(handle);
                    let _ = send_to(conns, id, &S2C::Welcome {
                        proto: PROTO_VERSION,
                        motd: "ember arena — cubes with guns".into(),
                    });
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
                            players: l.members.len() as u8,
                            cap: MAX_PLAYERS as u8,
                        })
                        .collect();
                    let _ = send_to(conns, id, &S2C::LobbyList { lobbies: list });
                }
                (C2S::CreateLobby { name, password }, true) => {
                    let name = sanitize_text(&name, MAX_LOBBY_LEN);
                    let password = password
                        .map(|p| sanitize_text(&p, MAX_PASSWORD_LEN))
                        .filter(|p| !p.is_empty());
                    if name.is_empty() {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: "lobby name must not be empty".into(),
                        });
                        return;
                    }
                    if conns.get(&id).unwrap().lobby.is_some() {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: "already in a game".into(),
                        });
                        return;
                    }
                    if lobbies.contains_key(&name) {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: format!("lobby \"{name}\" already exists"),
                        });
                        return;
                    }
                    if lobbies.len() >= cfg.max_lobbies {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: "server is full of lobbies, try later".into(),
                        });
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
                        next_pid: 0,
                    };
                    let pid = lobby.next_pid;
                    lobby.next_pid += 1;
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
                    let name = sanitize_text(&name, MAX_LOBBY_LEN);
                    if conns.get(&id).unwrap().lobby.is_some() {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: "already in a game".into(),
                        });
                        return;
                    }
                    let Some(lobby) = lobbies.get_mut(&name) else {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: format!("no lobby named \"{name}\""),
                        });
                        return;
                    };
                    if lobby.members.len() >= MAX_PLAYERS {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: "game is full".into(),
                        });
                        return;
                    }
                    if let Some(expected) = &lobby.password {
                        let given = password
                            .map(|p| sanitize_text(&p, MAX_PASSWORD_LEN))
                            .unwrap_or_default();
                        if &given != expected {
                            let _ = send_to(conns, id, &S2C::Error {
                                message: "wrong password".into(),
                            });
                            return;
                        }
                    }
                    let pid = lobby.next_pid;
                    lobby.next_pid = lobby.next_pid.wrapping_add(1);
                    lobby.members.push(id);
                    lobby.pids.insert(id, pid);
                    lobby.sim.add_player(pid);
                    conns.get_mut(&id).unwrap().lobby = Some(name.clone());

                    let handle = conns
                        .get(&id)
                        .and_then(|c| c.handle.clone())
                        .unwrap_or_else(|| "?".into());
                    let meta = PlayerMeta { id: pid, handle: handle.clone(), color: color_for(pid) };
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
                (C2S::Input { mx, my, ax, az, fire }, true) => {
                    let Some(lobby_name) = conns.get(&id).unwrap().lobby.clone() else { return };
                    let Some(lobby) = lobbies.get_mut(&lobby_name) else { return };
                    let Some(&pid) = lobby.pids.get(&id) else { return };
                    // The sim sanitizes magnitudes/NaN on use.
                    lobby.inputs.insert(pid, PlayerIn { mv: [mx, my], aim: [ax, az], fire });
                }
            }
        }
    }
}

/// Remove a connection entirely (disconnect/timeout/violation).
fn drop_conn(
    id: u64,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
) {
    leave_lobby(id, conns, lobbies);
    if let Some(c) = conns.remove(&id) {
        if let Some(h) = &c.handle {
            tracing::info!(conn = id, peer = %c.peer, handle = %h, "disconnected");
        }
        // Dropping c.tx makes the conn thread send a Close and exit.
    }
}

/// Take a player out of their game; the game keeps running for the rest,
/// and the lobby dies when the last player leaves.
fn leave_lobby(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let Some(name) = conns.get_mut(&id).and_then(|c| {
        c.lobbyless_since = Instant::now();
        c.lobby.take()
    }) else {
        return;
    };
    let Some(lobby) = lobbies.get_mut(&name) else { return };

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
