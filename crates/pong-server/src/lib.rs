//! Pong matchmaking + match server.
//!
//! Same architecture as ember-server: ONE hub thread owns all state
//! (connections, lobbies, running matches) and ticks at 60 Hz; a thread per
//! connection translates WebSocket frames into events. Transport is
//! WebSocket + JSON (`pong_core::proto`) so browsers can join, with TLS
//! terminated by the tunnel in front.
//!
//! This listener sits behind a PUBLIC tunnel: every input is untrusted.
//! Bounded queues, connection caps, message-size caps, per-tick message
//! budgets, and string sanitization throughout.

use std::collections::HashMap;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use pong_core::proto::{
    sanitize_axis, sanitize_text, C2S, LobbyInfo, S2C, MAX_HANDLE_LEN, MAX_LOBBY_LEN,
    MAX_PASSWORD_LEN, PROTO_VERSION, STATE_EVERY_TICKS,
};
use pong_core::sim::{Phase, Sim, FIXED_DT};
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
    last_seen: Instant,
    msgs_this_tick: u32,
}

struct Lobby {
    host: u64,
    guest: Option<u64>,
    password: Option<String>,
    game: Option<Match>,
}

struct Match {
    sim: Sim,
    host_axis: f32,
    guest_axis: f32,
}

pub fn run(listener: TcpListener, cfg: ServerConfig) -> io::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        "pong-server listening on {local} (proto v{PROTO_VERSION}, max {} conns, {} lobbies)",
        cfg.max_conns, cfg.max_lobbies
    );

    let (events_tx, events_rx) = mpsc::channel::<Ev>();
    let live_conns = Arc::new(AtomicUsize::new(0));

    {
        let events_tx = events_tx.clone();
        let live_conns = Arc::clone(&live_conns);
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
                let id = next_id;
                next_id += 1;
                live_conns.fetch_add(1, Ordering::Relaxed);
                let events_tx = events_tx.clone();
                let live_conns = Arc::clone(&live_conns);
                thread::spawn(move || {
                    conn_thread(id, stream, events_tx);
                    live_conns.fetch_sub(1, Ordering::Relaxed);
                });
            }
        });
    }

    hub_loop(events_rx, cfg)
}

/// Per-connection thread: WS handshake, then a single loop that alternates
/// draining the outbound queue and polling for one inbound frame (short
/// read timeout). One thread per conn keeps sync tungstenite simple — no
/// reader/writer split needed.
fn conn_thread(id: u64, stream: TcpStream, events_tx: Sender<Ev>) {
    let peer = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "?".into());
    let _ = stream.set_nodelay(true);
    // Generous timeout for the handshake itself...
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    let ws_cfg = WebSocketConfig::default()
        .max_message_size(Some(MAX_WS_MESSAGE))
        .max_frame_size(Some(MAX_WS_MESSAGE));
    let mut ws = match tungstenite::accept_with_config(stream, Some(ws_cfg)) {
        Ok(ws) => ws,
        Err(e) => {
            tracing::debug!(conn = id, peer = %peer, "handshake failed: {e}");
            return;
        }
    };
    // ...then a short poll interval for the steady-state loop.
    let _ = ws.get_ref().set_read_timeout(Some(Duration::from_millis(20)));

    let (tx, rx) = mpsc::sync_channel::<Message>(OUTBOUND_QUEUE);
    if events_tx
        .send(Ev::Connected { id, tx, peer: peer.clone() })
        .is_err()
    {
        return;
    }

    'outer: loop {
        // Drain pending outbound messages first.
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    if ws.send(msg).is_err() {
                        break 'outer;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Hub dropped us deliberately: polite close.
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    break 'outer;
                }
            }
        }
        // Poll one inbound frame (20 ms max).
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
            Ok(Message::Ping(_) | Message::Pong(_)) => {} // tungstenite auto-pongs
            Ok(_) => {
                tracing::debug!(conn = id, "binary frame from client; dropping");
                break;
            }
            Err(tungstenite::Error::Io(e))
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                // No inbound frame this poll — loop back to the outbox.
            }
            Err(_) => break,
        }
    }
    let _ = events_tx.send(Ev::Disconnected { id });
}

fn hub_loop(events_rx: Receiver<Ev>, cfg: ServerConfig) -> io::Result<()> {
    let tick_dt = Duration::from_secs_f32(FIXED_DT);
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut lobbies: HashMap<String, Lobby> = HashMap::new();
    let mut tick: u64 = 0;
    let mut next_tick_at = Instant::now() + tick_dt;
    let mut last_report = Instant::now();

    loop {
        loop {
            let now = Instant::now();
            let Some(wait) = next_tick_at.checked_duration_since(now) else { break };
            match events_rx.recv_timeout(wait) {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, &cfg),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(io::ErrorKind::Other, "accept thread died"));
                }
            }
        }
        for _ in 0..1024 {
            match events_rx.try_recv() {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, &cfg),
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

        // Step every running match.
        let mut ended_lobbies: Vec<String> = Vec::new();
        for (name, lobby) in lobbies.iter_mut() {
            let Some(m) = lobby.game.as_mut() else { continue };
            let Some(guest) = lobby.guest else { continue };
            m.sim.step(m.host_axis, m.guest_axis);

            if let Some((scorer, won)) = m.sim.event {
                let ev = S2C::MatchEvent {
                    scorer: scorer as u8,
                    won,
                    scores: m.sim.score,
                };
                send_to(&conns, lobby.host, &ev);
                send_to(&conns, guest, &ev);
            }
            if tick % STATE_EVERY_TICKS == 0 {
                let state = S2C::State {
                    tick,
                    ball: m.sim.ball_pos,
                    paddles: [m.sim.p1_x, m.sim.p2_x],
                    scores: m.sim.score,
                    serving: matches!(m.sim.phase, Phase::Serving { .. }),
                };
                if !send_to(&conns, lobby.host, &state) || !send_to(&conns, guest, &state) {
                    // A stalled pipe ends the match; the sweep/disconnect
                    // path will clean the conns up.
                    ended_lobbies.push(name.clone());
                }
            }
        }
        for name in ended_lobbies {
            lobbies.remove(&name);
        }

        // Reset flood counters; sweep silent peers.
        let timeout_ids: Vec<u64> = conns
            .iter_mut()
            .map(|(&id, c)| {
                c.msgs_this_tick = 0;
                (id, now.duration_since(c.last_seen))
            })
            .filter(|(_, silent)| *silent > CONN_TIMEOUT)
            .map(|(id, _)| id)
            .collect();
        for id in timeout_ids {
            tracing::info!(conn = id, "timed out");
            drop_conn(id, &mut conns, &mut lobbies);
        }

        if last_report.elapsed() > Duration::from_secs(30) {
            last_report = Instant::now();
            let in_match = lobbies.values().filter(|l| l.game.is_some()).count();
            tracing::info!(
                tick,
                connections = conns.len(),
                lobbies = lobbies.len(),
                matches_running = in_match,
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

fn handle_event(
    ev: Ev,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    cfg: &ServerConfig,
) {
    match ev {
        Ev::Connected { id, tx, peer } => {
            tracing::info!(conn = id, peer = %peer, "connected");
            conns.insert(
                id,
                Conn {
                    tx,
                    peer,
                    handle: None,
                    lobby: None,
                    last_seen: Instant::now(),
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
                                "protocol mismatch: server v{PROTO_VERSION}, client v{proto}"
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
                        motd: "ember pong matchmaking".into(),
                    });
                }
                (C2S::Hello { .. }, true) => {
                    tracing::debug!(conn = id, "duplicate Hello; dropping");
                    drop_conn(id, conns, lobbies);
                }
                (C2S::Ping { nonce }, _) => {
                    if !send_to(conns, id, &S2C::Pong { nonce }) {
                        drop_conn(id, conns, lobbies);
                    }
                }
                (_, false) => {
                    tracing::debug!(conn = id, "message before Hello; dropping");
                    drop_conn(id, conns, lobbies);
                }
                (C2S::ListLobbies, true) => {
                    let list: Vec<LobbyInfo> = lobbies
                        .iter()
                        .filter(|(_, l)| l.guest.is_none())
                        .map(|(name, l)| LobbyInfo {
                            name: name.clone(),
                            host: conns
                                .get(&l.host)
                                .and_then(|c| c.handle.clone())
                                .unwrap_or_else(|| "?".into()),
                            has_password: l.password.is_some(),
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
                            message: "already in a lobby".into(),
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
                    lobbies.insert(
                        name.clone(),
                        Lobby { host: id, guest: None, password, game: None },
                    );
                    conns.get_mut(&id).unwrap().lobby = Some(name.clone());
                    tracing::info!(conn = id, lobby = %name, "lobby created");
                    let _ = send_to(conns, id, &S2C::LobbyCreated { name });
                }
                (C2S::JoinLobby { name, password }, true) => {
                    let name = sanitize_text(&name, MAX_LOBBY_LEN);
                    if conns.get(&id).unwrap().lobby.is_some() {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: "already in a lobby".into(),
                        });
                        return;
                    }
                    let Some(lobby) = lobbies.get_mut(&name) else {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: format!("no lobby named \"{name}\""),
                        });
                        return;
                    };
                    if lobby.guest.is_some() {
                        let _ = send_to(conns, id, &S2C::Error {
                            message: "lobby is full".into(),
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
                    lobby.guest = Some(id);
                    lobby.game = Some(Match {
                        sim: Sim::new(),
                        host_axis: 0.0,
                        guest_axis: 0.0,
                    });
                    let host_id = lobby.host;
                    conns.get_mut(&id).unwrap().lobby = Some(name.clone());
                    let host_handle = conns
                        .get(&host_id)
                        .and_then(|c| c.handle.clone())
                        .unwrap_or_else(|| "?".into());
                    let guest_handle = conns
                        .get(&id)
                        .and_then(|c| c.handle.clone())
                        .unwrap_or_else(|| "?".into());
                    tracing::info!(lobby = %name, host = %host_handle, guest = %guest_handle, "match starting");
                    let _ = send_to(conns, host_id, &S2C::MatchStart {
                        role: 0,
                        opponent: guest_handle,
                    });
                    let _ = send_to(conns, id, &S2C::MatchStart {
                        role: 1,
                        opponent: host_handle,
                    });
                }
                (C2S::LeaveLobby, true) => {
                    leave_lobby(id, conns, lobbies);
                }
                (C2S::Input { axis }, true) => {
                    let axis = sanitize_axis(axis);
                    let Some(lobby_name) = conns.get(&id).unwrap().lobby.clone() else {
                        return;
                    };
                    let Some(lobby) = lobbies.get_mut(&lobby_name) else { return };
                    let Some(m) = lobby.game.as_mut() else { return };
                    if lobby.host == id {
                        m.host_axis = axis;
                    } else if lobby.guest == Some(id) {
                        m.guest_axis = axis;
                    }
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

/// Take a player out of their lobby; the remaining player (if any) becomes
/// the waiting host of the still-open lobby.
fn leave_lobby(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let Some(name) = conns.get_mut(&id).and_then(|c| c.lobby.take()) else { return };
    let Some(lobby) = lobbies.get_mut(&name) else { return };

    let other = if lobby.host == id {
        lobby.guest.take()
    } else if lobby.guest == Some(id) {
        lobby.guest = None;
        Some(lobby.host)
    } else {
        None
    };
    lobby.game = None;

    match other {
        Some(remaining) => {
            lobby.host = remaining;
            tracing::info!(lobby = %name, "player left; lobby reopened for the remaining player");
            let _ = send_to(conns, remaining, &S2C::OpponentLeft);
        }
        None => {
            tracing::info!(lobby = %name, "lobby closed");
            lobbies.remove(&name);
        }
    }
}
