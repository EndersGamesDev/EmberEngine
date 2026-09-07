//! The league host: one hub thread owns every lobby and steps them on a
//! fixed 60 Hz clock; one thread per connection owns its socket. Nothing
//! is shared by lock — everything crosses a channel. The authority story
//! is the fire one, made simpler by league having no client-side prediction
//! to reconcile: clients send commands, never positions, and the world
//! they see is exactly what `league_core::sim::Match` said last.
//!
//! The lobby walks `Select → Live → Over → Select`: humans pick champions,
//! summoner spells and rune pages during Select (the host may start early
//! after every human has picked; the clock starts the match otherwise).
//! Unfilled seats are deterministic
//! bots that think with `league_core::ai`, and the result screen holds for
//! `RESULT_SECS` before the same roster re-selects.

use std::collections::HashMap;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use tungstenite::Message;
use tungstenite::protocol::WebSocketConfig;

use league_core::ai;
use league_core::proto::{self, C2S, Cmd, LobbyInfo, Phase, S2C, STATE_EVERY_TICKS, SlotInfo};
use league_core::sim::Match;

const OUTBOUND_QUEUE: usize = 256;
const MAX_WS_MESSAGE: usize = proto::MAX_FRAME_BYTES;
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(10);
const MAX_MSGS_PER_TICK: u32 = 64;
const MAX_PENDING_EVENTS: usize = 1024;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub max_lobbies: usize,
    pub host_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_lobbies: 16,
            host_name: String::new(),
        }
    }
}

#[must_use]
pub fn build_stamp() -> (&'static str, &'static str) {
    (
        option_env!("EMBER_BUILD_VERSION").unwrap_or(""),
        option_env!("EMBER_BUILD_COMMIT").unwrap_or(""),
    )
}

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
    Disconnected {
        id: u64,
    },
}

struct Conn {
    tx: SyncSender<Message>,
    peer: String,
    handle: Option<String>,
    /// From Hello. Listing is allowed at any version — the lobby browser
    /// has no game loaded — but entering a lobby requires an exact match.
    proto: u16,
    lobby: Option<String>,
    slot: Option<u8>,
    last_seen: Instant,
    msgs_this_tick: u32,
}

struct Lobby {
    password: Option<String>,
    /// Team size: 1 for the duel, 3 for the squad.
    mode: u8,
    m: Match,
    /// slot -> conn
    members: HashMap<u8, u64>,
    /// Broadcast the select/result clock at this cadence.
    clock_left: f32,
}

/// Serve League connections accepted from `listener` until it closes.
///
/// # Errors
///
/// Returns an I/O error when the listener's local address cannot be read.
pub fn run(listener: &TcpListener, cfg: ServerConfig) -> io::Result<()> {
    let local = listener.local_addr()?;
    let (version, commit) = build_stamp();
    let host = if cfg.host_name.is_empty() {
        "<unnamed>"
    } else {
        cfg.host_name.as_str()
    };
    if version.is_empty() && commit.is_empty() {
        tracing::info!(
            host,
            "league-server: UNSTAMPED build (no EMBER_BUILD_VERSION/EMBER_BUILD_COMMIT at compile time)"
        );
    } else {
        tracing::info!(host, version, commit, "league-server build");
    }
    tracing::info!(
        addr = %local,
        proto = proto::PROTO_VERSION,
        "league-server listening"
    );

    let (events_tx, events_rx) = mpsc::sync_channel::<Ev>(MAX_PENDING_EVENTS);
    let hub = thread::spawn(move || {
        if let Err(e) = hub_loop(&events_rx, &cfg) {
            tracing::error!("hub loop died: {e}");
        }
    });

    let mut next_id = 1u64;
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let id = next_id;
                next_id += 1;
                let tx = events_tx.clone();
                thread::spawn(move || conn_thread(id, s, &tx));
            }
            Err(e) => tracing::warn!("accept failed: {e}"),
        }
    }
    drop(events_tx);
    drop(hub.join());
    Ok(())
}

fn conn_thread(id: u64, stream: TcpStream, events_tx: &SyncSender<Ev>) {
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "?".into(), |a| a.to_string());
    drop(stream.set_nodelay(true));
    drop(stream.set_read_timeout(Some(Duration::from_secs(10))));
    drop(stream.set_write_timeout(Some(Duration::from_secs(15))));

    // Total handshake deadline: a peer dribbling one byte per window must
    // not hold a slot.
    let done = Arc::new(AtomicBool::new(false));
    if let Ok(watch) = stream.try_clone() {
        let flag = Arc::clone(&done);
        thread::spawn(move || {
            let step = Duration::from_millis(250);
            let mut waited = Duration::ZERO;
            while waited < HANDSHAKE_DEADLINE {
                thread::sleep(step);
                waited += step;
                if flag.load(Ordering::Relaxed) {
                    return;
                }
            }
            drop(watch.shutdown(std::net::Shutdown::Both));
        });
    }

    let ws_cfg = WebSocketConfig::default()
        .max_message_size(Some(MAX_WS_MESSAGE))
        .max_frame_size(Some(MAX_WS_MESSAGE));
    let mut ws = match tungstenite::accept_with_config(stream, Some(ws_cfg)) {
        Ok(ws) => ws,
        Err(e) => {
            done.store(true, Ordering::Relaxed);
            tracing::debug!(conn = id, peer = %peer, "handshake failed: {e}");
            return;
        }
    };
    done.store(true, Ordering::Relaxed);
    drop(
        ws.get_ref()
            .set_read_timeout(Some(Duration::from_millis(5))),
    );

    let (tx, rx) = mpsc::sync_channel::<Message>(OUTBOUND_QUEUE);
    if events_tx
        .send(Ev::Connected {
            id,
            tx,
            peer: peer.clone(),
        })
        .is_err()
    {
        return;
    }

    'outer: loop {
        loop {
            match rx.try_recv() {
                Ok(m) => {
                    if let Err(e) = ws.write(m) {
                        tracing::debug!(conn = id, "write failed: {e}");
                        break 'outer;
                    }
                    match ws.flush() {
                        Ok(()) => {}
                        Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
                        Err(e) => {
                            tracing::debug!(conn = id, "flush failed: {e}");
                            break 'outer;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    drop(ws.close(None));
                    drop(ws.flush());
                    break 'outer;
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                match serde_json::from_str::<C2S>(&t) {
                    Ok(msg) => {
                        if events_tx.send(Ev::Msg { id, msg }).is_err() {
                            tracing::warn!("hub channel closed; dropping inbound");
                            break;
                        }
                    }
                    // An unparseable frame is the peer's problem, not
                    // grounds to drop the connection.
                    Err(e) => tracing::warn!(conn = id, "undecodable frame: {e}: {t}"),
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(e) => {
                tracing::debug!(conn = id, peer = %peer, "read ended: {e}");
                break;
            }
        }
    }
    drop(events_tx.send(Ev::Disconnected { id }));
}

fn send_to(conns: &HashMap<u64, Conn>, id: u64, msg: &S2C) -> bool {
    let Ok(text) = serde_json::to_string(msg) else {
        return false;
    };
    let Some(c) = conns.get(&id) else {
        tracing::warn!(conn = id, "send to a connection that is gone: {msg:?}");
        return false;
    };
    match c.tx.try_send(Message::text(text)) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            tracing::warn!(conn = id, "outbound queue FULL, dropping: {msg:?}");
            false
        }
        Err(TrySendError::Disconnected(_)) => {
            tracing::debug!(conn = id, "outbound queue closed, dropping: {msg:?}");
            false
        }
    }
}

fn broadcast(lobby: &Lobby, conns: &HashMap<u64, Conn>, msg: &S2C) {
    for member in lobby.members.values() {
        send_to(conns, *member, msg);
    }
}

fn hub_loop(events_rx: &Receiver<Ev>, cfg: &ServerConfig) -> io::Result<()> {
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut lobbies: HashMap<String, Lobby> = HashMap::new();
    let mut last = Instant::now();
    // a fixed-step accumulator: the fractional 2 ms the hub polls with
    // must carry between iterations, or truncating `elapsed / DT` never
    // reaches one tick and the world stands still (learned from fire's
    // FixedStep, which exists for exactly this)
    let mut acc: f32 = 0.0;

    loop {
        // A busy sender must not prevent the fixed simulation clock from
        // advancing. The bounded channel also limits pending input memory.
        for _ in 0..MAX_PENDING_EVENTS {
            match events_rx.try_recv() {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, cfg),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        let now = Instant::now();
        let elapsed = now.duration_since(last).as_secs_f32().min(0.25);
        last = now;
        acc += elapsed;
        loop {
            if acc < league_core::DT {
                break;
            }
            acc -= league_core::DT;
            tick_lobbies(&mut lobbies, &conns);
            for conn in conns.values_mut() {
                conn.msgs_this_tick = 0;
            }
        }
        drop_silent(&mut conns, &mut lobbies);
        thread::sleep(Duration::from_millis(2));
    }
}

fn tick_lobbies(lobbies: &mut HashMap<String, Lobby>, conns: &HashMap<u64, Conn>) {
    let mut empty: Vec<String> = Vec::new();
    for (name, lobby) in lobbies.iter_mut() {
        if lobby.members.is_empty() {
            empty.push(name.clone());
            continue;
        }
        // Bots think on the sim clock, deterministically: every live tick
        // rolls a fresh decision per bot seat; `ai::think` owns the rest.
        // A disconnected human is a bot seat now (leave() says so).
        if lobby.m.phase == Phase::Live {
            let mut acts: Vec<(u8, Cmd)> = Vec::new();
            for slot in 0..2 * lobby.m.team_size {
                if lobby.m.roster[usize::from(slot)].bot {
                    acts.extend(ai::think(&lobby.m, slot).map(|c| (slot, c)));
                }
            }
            for (slot, c) in acts {
                lobby.m.command(slot, c);
            }
        }
        // Step first, then read every field the broadcasts need into plain
        // locals: the mutable sim borrow must end before the lobby is read
        // again to address its members.
        let before = lobby.m.phase;
        lobby.m.step();
        let phase = lobby.m.phase;
        let left = lobby.m.left;
        let mut out: Vec<S2C> = Vec::new();
        if phase == Phase::Over && left <= 0.0 {
            // the result screen held long enough: same roster, new draft
            reset_for_select(lobby);
            out.push(S2C::Roster {
                roster: lobby.m.roster.clone(),
            });
            out.push(S2C::Phase {
                phase: lobby.m.phase,
                left: lobby.m.left,
            });
        } else if phase != before {
            out.push(S2C::Phase { phase, left });
            if phase == Phase::Live {
                out.push(S2C::Roster {
                    roster: lobby.m.roster.clone(),
                });
            }
            if phase == Phase::Over {
                out.push(S2C::Result {
                    winner: lobby.m.winner,
                    kills: lobby.m.kills,
                    gold: lobby.m.earned,
                });
            }
        } else if phase != Phase::Live && lobby.clock_left <= 0.0 {
            // the draft/result clock, twice a second, for the pages
            lobby.clock_left = 0.5;
            out.push(S2C::Phase { phase, left });
        }
        if lobby.clock_left > 0.0 {
            lobby.clock_left -= league_core::DT;
        }
        if phase == Phase::Live && lobby.m.tick % STATE_EVERY_TICKS == 0 {
            out.push(lobby.m.snapshot());
        }
        for msg in &out {
            broadcast(lobby, conns, msg);
        }
    }
    for name in empty {
        lobbies.remove(&name);
    }
}

fn roster_msg(lobby: &Lobby) -> S2C {
    S2C::Roster {
        roster: lobby.m.roster.clone(),
    }
}

/// The result screen closed: the same roster re-selects. Picks are cleared
/// so the next game is a new draft, and the seed advances so bot picks and
/// rolls differ from the last game.
fn reset_for_select(lobby: &mut Lobby) {
    let mut fresh = Match::new(lobby.mode, lobby.m.seed.wrapping_add(1));
    for r in &lobby.m.roster {
        let slot = &mut fresh.roster[usize::from(r.slot)];
        slot.handle.clone_from(&r.handle);
        slot.bot = r.bot;
        slot.connected = r.connected;
    }
    lobby.m = fresh;
    lobby.clock_left = 0.0;
}

fn handle_event(
    ev: Ev,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    cfg: &ServerConfig,
) {
    match ev {
        Ev::Connected { id, tx, peer } => {
            conns.insert(
                id,
                Conn {
                    tx,
                    peer,
                    handle: None,
                    proto: 0,
                    lobby: None,
                    slot: None,
                    last_seen: Instant::now(),
                    msgs_this_tick: 0,
                },
            );
        }
        Ev::Disconnected { id } => {
            if let Some(c) = conns.get(&id) {
                tracing::debug!(conn = id, peer = %c.peer, "connection gone");
            }
            drop_conn(id, conns, lobbies);
        }
        Ev::Msg { id, msg } => {
            let Some(c) = conns.get_mut(&id) else {
                return;
            };
            c.last_seen = Instant::now();
            if c.msgs_this_tick >= MAX_MSGS_PER_TICK {
                tracing::warn!(
                    conn = id,
                    "over the per-tick message allowance, dropping: {:?}",
                    std::mem::discriminant(&msg)
                );
                return;
            }
            c.msgs_this_tick += 1;
            handle_msg(id, msg, conns, lobbies, cfg);
        }
    }
}

fn handle_msg(
    id: u64,
    msg: C2S,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    cfg: &ServerConfig,
) {
    match msg {
        C2S::Hello { proto: v, handle } => handle_hello(id, v, &handle, conns, lobbies, cfg),
        C2S::ListLobbies => handle_list_lobbies(id, conns, lobbies),
        C2S::CreateLobby {
            name,
            password,
            mode,
        } => create_lobby(id, &name, password.as_deref(), mode, conns, lobbies, cfg),
        C2S::JoinLobby { name, password } => {
            let pw = password.unwrap_or_default();
            join_lobby(id, &name, &pw, conns, lobbies);
        }
        C2S::LeaveLobby => leave_lobby(id, conns, lobbies),
        C2S::Pick { champ, d, f, runes } => {
            handle_pick(id, champ, d, f, runes, conns, lobbies);
        }
        C2S::StartMatch => handle_start_match(id, conns, lobbies),
        C2S::Cmd(cmd) => handle_cmd(id, cmd, conns, lobbies),
        C2S::Ping { nonce } => {
            send_to(conns, id, &S2C::Pong { nonce });
        }
    }
}

fn handle_hello(
    id: u64,
    version: u16,
    handle: &str,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &HashMap<String, Lobby>,
    cfg: &ServerConfig,
) {
    let Some(conn) = conns.get_mut(&id) else {
        return;
    };
    if conn.handle.is_some() {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "Hello was already received".into(),
            },
        );
        return;
    }
    tracing::info!(conn = id, proto = version, handle = %handle, "hello");
    conn.proto = version;
    conn.handle = Some(proto::sanitize_handle(handle));
    let players =
        u32::try_from(conns.values().filter(|c| c.slot.is_some()).count()).unwrap_or(u32::MAX);
    let open = u32::try_from(lobbies.len()).unwrap_or(u32::MAX);
    let (version, commit) = build_stamp();
    send_to(
        conns,
        id,
        &S2C::Welcome {
            proto: proto::PROTO_VERSION,
            host: cfg.host_name.clone(),
            version: version.to_owned(),
            commit: commit.to_owned(),
            players,
            lobbies: open,
        },
    );
}

fn handle_list_lobbies(id: u64, conns: &HashMap<u64, Conn>, lobbies: &HashMap<String, Lobby>) {
    let rows: Vec<LobbyInfo> = lobbies
        .iter()
        .map(|(name, lobby)| LobbyInfo {
            name: name.clone(),
            host: lobby.members.keys().min().map_or_else(
                || "?".to_string(),
                |slot| lobby.m.roster[usize::from(*slot)].handle.clone(),
            ),
            has_password: lobby.password.is_some(),
            players: u8::try_from(lobby.m.roster.iter().filter(|r| !r.bot).count())
                .unwrap_or(u8::MAX),
            cap: 2 * lobby.mode,
            mode: lobby.mode,
            racing: lobby.m.phase == Phase::Live,
        })
        .collect();
    send_to(conns, id, &S2C::Lobbies { lobbies: rows });
}

fn handle_pick(
    id: u64,
    champ: u8,
    d: u8,
    f: u8,
    runes: [u8; 3],
    conns: &HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
) {
    let (Some(lobby_name), Some(slot)) = (
        conns.get(&id).and_then(|c| c.lobby.clone()),
        conns.get(&id).and_then(|c| c.slot),
    ) else {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "not in a lobby".into(),
            },
        );
        return;
    };
    let Some(lobby) = lobbies.get_mut(&lobby_name) else {
        return;
    };
    if lobby.m.phase != Phase::Select {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "the draft is closed".into(),
            },
        );
        return;
    }
    lobby.m.set_pick(slot, champ, d, f, runes);
    let pick = &lobby.m.roster[usize::from(slot)];
    if !pick.picked || (pick.champ, pick.d, pick.f, pick.runes) != (champ, d, f, runes) {
        send_to(conns, id, &S2C::Rejected {
            reason: "choose an available champion for your team, two different spells, and three different runes".into(),
        });
        return;
    }
    // a click on a champion card is cheap to echo; broadcast the
    // whole roster so every screen in the draft agrees
    broadcast(lobby, conns, &roster_msg(lobby));
}

fn handle_start_match(id: u64, conns: &HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let (Some(lobby_name), Some(slot)) = (
        conns.get(&id).and_then(|c| c.lobby.clone()),
        conns.get(&id).and_then(|c| c.slot),
    ) else {
        return;
    };
    let Some(lobby) = lobbies.get_mut(&lobby_name) else {
        return;
    };
    if lobby.members.keys().copied().min() != Some(slot) {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "only the host can start the match".into(),
            },
        );
        return;
    }
    if lobby.m.phase != Phase::Select {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "the draft already ran".into(),
            },
        );
        return;
    }
    if lobby.m.roster.iter().any(|r| !r.bot && !r.picked) {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "every player must pick a champion before the host can start".into(),
            },
        );
        return;
    }
    lobby.m.start();
    broadcast(lobby, conns, &roster_msg(lobby));
    broadcast(
        lobby,
        conns,
        &S2C::Phase {
            phase: Phase::Live,
            left: 0.0,
        },
    );
    let state = lobby.m.snapshot();
    broadcast(lobby, conns, &state);
}

fn handle_cmd(id: u64, cmd: Cmd, conns: &HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let (Some(lobby_name), Some(slot)) = (
        conns.get(&id).and_then(|c| c.lobby.clone()),
        conns.get(&id).and_then(|c| c.slot),
    ) else {
        return;
    };
    let Some(lobby) = lobbies.get_mut(&lobby_name) else {
        return;
    };
    if lobby.m.phase == Phase::Live {
        lobby.m.command(slot, cmd.sanitized());
    }
}

fn create_lobby(
    id: u64,
    requested_name: &str,
    password: Option<&str>,
    mode: u8,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    cfg: &ServerConfig,
) {
    if !version_ok(id, conns) {
        return;
    }
    if lobbies.len() >= cfg.max_lobbies {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "server is full".into(),
            },
        );
        return;
    }
    let name = proto::sanitize(requested_name, proto::MAX_LOBBY_LEN);
    if name.is_empty() {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "lobby needs a name".into(),
            },
        );
        return;
    }
    if lobbies.contains_key(&name) {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "that name is taken".into(),
            },
        );
        return;
    }
    let mode = if mode == 1 { 1 } else { 3 };
    let pw = proto::sanitize(password.unwrap_or(""), proto::MAX_PASSWORD_LEN);
    // Creation is a lobby switch too. Only leave after every request
    // validation succeeds, so a rejected create preserves the old seat.
    leave_lobby(id, conns, lobbies);
    let mut lobby = Lobby {
        password: if pw.is_empty() { None } else { Some(pw) },
        mode,
        m: Match::new(mode, hash_name(&name)),
        members: HashMap::new(),
        clock_left: 0.0,
    };
    // the creator takes slot 0 and hosts the draft
    if join_match(&mut lobby, &name, id, conns) {
        let roster = lobby.m.roster.clone();
        let (phase, left) = (lobby.m.phase, lobby.m.left);
        lobbies.insert(name.clone(), lobby);
        send_to(
            conns,
            id,
            &S2C::Joined {
                lobby: name,
                id: 0,
                mode,
                roster,
            },
        );
        send_to(conns, id, &S2C::Phase { phase, left });
    } else {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "could not seat the creator".into(),
            },
        );
    }
}

#[must_use]
fn hash_name(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

fn join_lobby(
    id: u64,
    name: &str,
    password: &str,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
) {
    if !version_ok(id, conns) {
        return;
    }
    let Some(lobby) = lobbies.get(name) else {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "no such lobby".into(),
            },
        );
        return;
    };
    if let Some(want) = &lobby.password
        && proto::sanitize(password, proto::MAX_PASSWORD_LEN) != *want
    {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "wrong password".into(),
            },
        );
        return;
    }
    let already_here = conns
        .get(&id)
        .is_some_and(|c| c.lobby.as_deref() == Some(name));
    if !already_here && !lobby.m.roster.iter().any(|r| r.bot) {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "lobby is full".into(),
            },
        );
        return;
    }
    finish_join(id, name, already_here, conns, lobbies);
}

fn finish_join(
    id: u64,
    name: &str,
    already_here: bool,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
) {
    // Validation finished before this point: a typo, a full lobby or a
    // wrong password must not remove a player from an existing match.
    if !already_here {
        leave_lobby(id, conns, lobbies);
    }
    let Some(lobby) = lobbies.get_mut(name) else {
        return;
    };
    let handle = conns
        .get(&id)
        .and_then(|c| c.handle.clone())
        .unwrap_or_else(|| "summoner".into());
    let slot = if already_here {
        conns.get(&id).and_then(|c| c.slot)
    } else {
        lobby.m.join(&handle)
    };
    let Some(slot) = slot else {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "lobby is full".into(),
            },
        );
        return;
    };
    lobby.members.insert(slot, id);
    if let Some(c) = conns.get_mut(&id) {
        c.lobby = Some(name.to_string());
        c.slot = Some(slot);
    }
    let roster = lobby.m.roster.clone();
    send_to(
        conns,
        id,
        &S2C::Joined {
            lobby: name.to_string(),
            id: slot,
            mode: lobby.mode,
            roster: roster.clone(),
        },
    );
    if lobby.m.phase == Phase::Live {
        // snapshot drains transient effects; preserve them for the normal
        // broadcast to all existing players on the next state tick.
        send_to(conns, id, &lobby.m.clone().snapshot());
    } else if lobby.m.phase == Phase::Over {
        send_to(
            conns,
            id,
            &S2C::Result {
                winner: lobby.m.winner,
                kills: lobby.m.kills,
                gold: lobby.m.earned,
            },
        );
    }
    send_to(
        conns,
        id,
        &S2C::Phase {
            phase: lobby.m.phase,
            left: lobby.m.left,
        },
    );
    let meta: SlotInfo = roster[usize::from(slot)].clone();
    // everyone else sees the new face
    for member in lobby.members.values() {
        if *member != id && !already_here {
            send_to(conns, *member, &S2C::PlayerJoined { slot: meta.clone() });
        }
    }
    tracing::info!(conn = id, lobby = name, slot, "joined");
}

/// Seat the creator on slot 0 of their fresh lobby. `Match::join` claims
/// the lowest free seat, which is 0 in a lobby nobody has joined yet.
fn join_match(lobby: &mut Lobby, name: &str, id: u64, conns: &mut HashMap<u64, Conn>) -> bool {
    let handle = conns
        .get(&id)
        .and_then(|c| c.handle.clone())
        .unwrap_or_else(|| "summoner".into());
    let Some(slot) = lobby.m.join(&handle) else {
        return false;
    };
    lobby.members.insert(slot, id);
    if let Some(c) = conns.get_mut(&id) {
        c.lobby = Some(name.to_string());
        c.slot = Some(slot);
    }
    true
}

fn version_ok(id: u64, conns: &HashMap<u64, Conn>) -> bool {
    let v = conns.get(&id).map_or(0, |c| c.proto);
    if v == proto::PROTO_VERSION {
        return true;
    }
    let saw_hello = conns.get(&id).is_some_and(|c| c.handle.is_some());
    tracing::warn!(
        conn = id,
        proto = v,
        saw_hello,
        "refusing on protocol version"
    );
    send_to(
        conns,
        id,
        &S2C::Rejected {
            reason: format!(
                "this build speaks league protocol v{v}, the live game is v{}",
                proto::PROTO_VERSION
            ),
        },
    );
    false
}

fn leave_lobby(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let Some(lobby_name) = conns.get(&id).and_then(|c| c.lobby.clone()) else {
        return;
    };
    let Some(slot) = conns.get(&id).and_then(|c| c.slot) else {
        return;
    };
    if let Some(c) = conns.get_mut(&id) {
        c.lobby = None;
        c.slot = None;
    }
    if let Some(lobby) = lobbies.get_mut(&lobby_name) {
        lobby.members.remove(&slot);
        lobby.m.leave(slot);
        // Live champions continue under bot control. The remaining lowest
        // occupied slot becomes host without moving anyone's champion.
        broadcast(lobby, conns, &S2C::PlayerLeft { slot });
        broadcast(lobby, conns, &roster_msg(lobby));
        if lobby.members.is_empty() {
            lobbies.remove(&lobby_name);
        }
    }
}

fn drop_conn(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    leave_lobby(id, conns, lobbies);
    conns.remove(&id);
}

fn drop_silent(conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let timeout = Duration::from_secs(proto::CLIENT_TIMEOUT_SECS);
    let gone: Vec<u64> = conns
        .iter()
        .filter(|(_, c)| c.last_seen.elapsed() > timeout)
        .map(|(id, _)| *id)
        .collect();
    for id in gone {
        tracing::info!(conn = id, "dropping a silent connection");
        drop_conn(id, conns, lobbies);
    }
}

#[cfg(test)]
mod tests;
