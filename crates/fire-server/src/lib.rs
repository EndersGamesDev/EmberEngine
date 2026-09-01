//! The authoritative Fire Racer server: WebSocket lobbies, one race each.
//!
//! Structure mirrors `pong-server`, because that shape is already proven
//! against this deployment path (Cloudflare quick tunnel in front of a plain
//! TCP listener). One thread per connection *owns* its socket and alternates
//! a short blocking read with draining a bounded outbound queue; a single hub
//! thread owns every lobby and steps them on a fixed 60 Hz clock. Nothing is
//! shared by lock — everything crosses a channel.
//!
//! The authority story: the server runs `fire_core::sim::Race`, the same code
//! the client predicts with. Clients send intents, never positions.

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tungstenite::protocol::WebSocketConfig;
use tungstenite::Message;

use fire_core::ai;
use fire_core::car::{CarInput, DT};
use fire_core::castle;
use fire_core::proto::{
    self, CarState, LobbyInfo, Phase, PlayerMeta, C2S, MAX_PLAYERS, S2C, STATE_EVERY_TICKS,
};
use fire_core::sim::{Race, RaceState};

/// Outbound queue depth per connection. Deep enough to hold several seconds
/// of 30 Hz state for a peer that briefly stops polling — at 64 a client that
/// looked away for two seconds filled the queue and then silently lost the
/// *control* messages behind it, so a joining player never appeared in
/// anyone's roster. Still bounded, so a dead peer cannot make the hub buffer
/// on its behalf for ever.
const OUTBOUND_QUEUE: usize = 256;
const MAX_WS_MESSAGE: usize = proto::MAX_FRAME_BYTES;
/// A client that has not finished the WebSocket handshake by now is not a
/// client. Without this, a peer dribbling one byte per read window holds a
/// connection slot indefinitely.
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(10);
/// Per-tick message allowance. A peer above this is flooding.
const MAX_MSGS_PER_TICK: u32 = 32;
/// How long a finished race stays on screen before the lobby resets.
const RESULTS_SECS: f32 = 8.0;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub laps: u32,
    pub max_lobbies: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { laps: 3, max_lobbies: 32 }
    }
}

enum Ev {
    Connected { id: u64, tx: SyncSender<Message>, peer: String },
    Msg { id: u64, msg: C2S },
    Disconnected { id: u64 },
}

struct Conn {
    tx: SyncSender<Message>,
    peer: String,
    handle: Option<String>,
    /// From Hello. Listing is allowed at any version — the lobby browser has
    /// no game loaded — but entering a race requires an exact match.
    proto: u16,
    lobby: Option<String>,
    last_seen: Instant,
    msgs_this_tick: u32,
}

/// A lobby IS a race. The grid is always full: unclaimed slots are driven by
/// the same AI the local game uses, so a two-player lobby is still a race
/// against a field rather than a lonely time trial.
struct Lobby {
    password: Option<String>,
    race: Race,
    /// Conn ids; `[0]` is the listed host, cosmetic after creation.
    members: Vec<u64>,
    /// conn id -> grid slot, which is also the player id on the wire.
    slots: HashMap<u64, u8>,
    ready: HashSet<u64>,
    /// slot -> (latest intent, its client sequence number).
    inputs: HashMap<u8, (CarInput, u32)>,
    /// Seconds left on the results screen once everyone has finished.
    results_left: f32,
    last_phase: Phase,
}

impl Lobby {
    fn new(password: Option<String>, laps: u32) -> Self {
        Self {
            password,
            race: Race::new(castle::track(), MAX_PLAYERS as usize, laps),
            members: Vec::new(),
            slots: HashMap::new(),
            ready: HashSet::new(),
            inputs: HashMap::new(),
            results_left: 0.0,
            last_phase: Phase::Waiting,
        }
    }

    const fn phase(&self) -> Phase {
        match self.race.state {
            RaceState::Waiting => Phase::Waiting,
            RaceState::Countdown => Phase::Countdown,
            RaceState::Racing => Phase::Racing,
            RaceState::Finished => Phase::Finished,
        }
    }

    /// Smallest unclaimed grid slot. Scanning beats a wrapping counter: in a
    /// long-lived lobby a counter eventually collides with a sitting player.
    fn alloc_slot(&self) -> Option<u8> {
        (0..MAX_PLAYERS).find(|s| !self.slots.values().any(|v| v == s))
    }
}

/// Run the lobby server until the listener closes.
///
/// # Errors
///
/// Returns an error if the listener's local address cannot be queried.
// The server owns and eventually closes the listener; changing this public signature would break callers.
#[allow(clippy::needless_pass_by_value)]
pub fn run(listener: TcpListener, cfg: ServerConfig) -> io::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        addr = %local,
        proto = proto::PROTO_VERSION,
        laps = cfg.laps,
        "fire-server listening"
    );

    let (events_tx, events_rx) = mpsc::channel::<Ev>();
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
    let _ = hub.join();
    Ok(())
}

fn conn_thread(id: u64, stream: TcpStream, events_tx: &Sender<Ev>) {
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "?".into(), |address| address.to_string());
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    // Total handshake deadline. The per-read timeout alone is not enough: a
    // peer can send one byte per window forever and never finish.
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
            let _ = watch.shutdown(std::net::Shutdown::Both);
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

    // This thread owns the socket, so the outbound queue only drains between
    // reads. A short read timeout keeps a broadcast from sitting on the queue
    // for most of a tick before it reaches the wire.
    let _ = ws.get_ref().set_read_timeout(Some(Duration::from_millis(5)));

    let (tx, rx) = mpsc::sync_channel::<Message>(OUTBOUND_QUEUE);
    if events_tx.send(Ev::Connected { id, tx, peer: peer.clone() }).is_err() {
        return;
    }

    'outer: loop {
        loop {
            match rx.try_recv() {
                Ok(m) => {
                    // write() buffers inside tungstenite; flush() puts it on
                    // the wire. A flush that would block has NOT lost the
                    // message — it is still buffered and the next flush sends
                    // it — so tearing the connection down here would drop a
                    // live peer and everything else queued for them.
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
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    break 'outer;
                }
            }
        }

        match ws.read() {
            Ok(Message::Text(t)) => {
                match serde_json::from_str::<C2S>(&t) {
                    Ok(msg) => {
                        if events_tx.send(Ev::Msg { id, msg }).is_err() {
                            tracing::warn!(conn = id, "hub channel closed; dropping inbound");
                            break;
                        }
                    }
                    // An unparseable frame is the peer's problem, not grounds
                    // to drop them: a newer client may send a variant this
                    // build has never heard of. But it is NOT debug-level —
                    // a frame we cannot read is indistinguishable, from the
                    // other end, from one that was never delivered, and that
                    // is exactly the shape of the residual join failure.
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
    let _ = events_tx.send(Ev::Disconnected { id });
}

fn send_to(conns: &HashMap<u64, Conn>, id: u64, msg: &S2C) -> bool {
    let Ok(text) = serde_json::to_string(msg) else {
        return false;
    };
    let Some(c) = conns.get(&id) else {
        tracing::warn!(conn = id, "send to a connection that is gone: {msg:?}");
        return false;
    };
    // Every caller ignores this bool, so without the logging a dropped
    // control message leaves no trace on either side of the wire.
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

fn broadcast(conns: &HashMap<u64, Conn>, lobby: &Lobby, msg: &S2C) {
    for m in &lobby.members {
        send_to(conns, *m, msg);
    }
}

fn roster(lobby: &Lobby, conns: &HashMap<u64, Conn>) -> Vec<PlayerMeta> {
    lobby
        .members
        .iter()
        .filter_map(|m| {
            let slot = *lobby.slots.get(m)?;
            Some(PlayerMeta {
                id: slot,
                handle: conns
                    .get(m)
                    .and_then(|c| c.handle.clone())
                    .unwrap_or_else(|| "?".into()),
                slot,
            })
        })
        .collect()
}

fn hub_loop(events_rx: &Receiver<Ev>, cfg: &ServerConfig) -> io::Result<()> {
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut lobbies: HashMap<String, Lobby> = HashMap::new();
    // Accumulate real elapsed time and run the whole ticks it owes, rather
    // than assuming one tick per loop. A `sleep` that overshoots under load
    // used to simply lose that time: the countdown is counted in ticks, so a
    // starved server ran the entire race in slow motion — a 3 s countdown
    // taking nine wall-clock seconds. `FixedStep` caps the catch-up too, so a
    // long stall is skipped rather than repaid as a burst.
    let mut clock = fire_core::sim::FixedStep::default();
    let mut last = Instant::now();

    loop {
        // Drain everything that arrived, then tick once. Draining first means
        // an input that lands 1 ms before the tick is applied on that tick
        // rather than the next one.
        loop {
            match events_rx.try_recv() {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, cfg),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        let now = Instant::now();
        let elapsed = now.duration_since(last).as_secs_f32();
        last = now;
        let ticks = clock.ticks(elapsed);
        for _ in 0..ticks {
            tick_lobbies(&mut lobbies, &conns, cfg);
        }
        if ticks > 0 {
            // Count all messages between simulation ticks, not merely between
            // the hub's much faster polling iterations.
            for conn in conns.values_mut() {
                conn.msgs_this_tick = 0;
            }
        }
        drop_silent(&mut conns, &mut lobbies);

        // Short sleep rather than sleeping to the next tick boundary: the
        // accumulator above already owns the timing, and waking a little
        // early only costs an idle loop.
        thread::sleep(Duration::from_millis(2));
    }
}

fn tick_lobbies(lobbies: &mut HashMap<String, Lobby>, conns: &HashMap<u64, Conn>, cfg: &ServerConfig) {
    let mut empty: Vec<String> = Vec::new();
    for (name, lobby) in lobbies.iter_mut() {
        if lobby.members.is_empty() {
            empty.push(name.clone());
            continue;
        }

        // Start when every human present has readied, and at least one has.
        if lobby.race.state == RaceState::Waiting
            && !lobby.ready.is_empty()
            && lobby.ready.len() == lobby.members.len()
        {
            lobby.race.start_countdown();
        }

        // Build the full grid: humans where they sit, AI everywhere else.
        let inputs: Vec<CarInput> = (0..lobby.race.racers.len())
            .map(|i| {
                let slot = u8::try_from(i).expect("race grid fits in a wire player id");
                match lobby.inputs.get(&slot) {
                    Some((input, _)) => *input,
                    None => {
                        if lobby.slots.values().any(|s| *s == slot) {
                            // A human holds this slot but has sent nothing
                            // yet — coast, do not hand their car to the AI.
                            CarInput::default()
                        } else {
                            ai::chase(&lobby.race.track, &lobby.race.racers[i].car, ai::DEFAULT_SKILL)
                        }
                    }
                }
            })
            .collect();

        lobby.race.step(&inputs, DT);

        // A boost press is consumed by exactly one tick. Clearing the flag
        // here is what stops a single packet boosting on every tick until the
        // next one arrives — the same bug the arena hit with jump.
        for (input, _) in lobby.inputs.values_mut() {
            input.boost = false;
        }

        let phase = lobby.phase();
        if phase != lobby.last_phase {
            lobby.last_phase = phase;
            broadcast(conns, lobby, &S2C::Phase { phase, countdown: lobby.race.countdown_left() });
            if phase == Phase::Finished {
                let order: Vec<u8> = lobby
                    .race
                    .standings()
                    .iter()
                    .map(|&i| u8::try_from(i).expect("race grid fits in a wire player id"))
                    .collect();
                broadcast(conns, lobby, &S2C::Results { order });
                lobby.results_left = RESULTS_SECS;
            }
        } else if phase == Phase::Countdown && lobby.race.tick % STATE_EVERY_TICKS == 0 {
            // 30 Hz, not 60: the page only renders `ceil(countdown)`, and the
            // spare frames were queue pressure on anyone slow to poll.
            broadcast(conns, lobby, &S2C::Phase { phase, countdown: lobby.race.countdown_left() });
        }

        // Only broadcast state once there is something to say. A Waiting
        // lobby's cars are parked on a grid the client already computed for
        // itself, so streaming it 30 times a second is pure queue pressure —
        // and it was enough to push control messages out of a slow peer's
        // queue while everyone sat in the lobby.
        if lobby.race.state != RaceState::Waiting && lobby.race.tick % STATE_EVERY_TICKS == 0 {
            let cars: Vec<CarState> = lobby
                .race
                .racers
                .iter()
                .enumerate()
                .map(|(i, racer)| {
                    let id = u8::try_from(i).expect("race grid fits in a wire player id");
                    CarState {
                        id,
                        x: racer.car.pos.x,
                        z: racer.car.pos.y,
                        yaw: racer.car.yaw,
                        vx: racer.car.vel.x,
                        vz: racer.car.vel.y,
                        lap: racer.lap.lap,
                        progress: racer.lap.progress,
                        boost: racer.car.boost_charges,
                        boosting: racer.car.boosting(),
                        drift: racer.car.drift,
                        ack: lobby.inputs.get(&id).map_or(0, |(_, sequence)| *sequence),
                    }
                })
                .collect();
            broadcast(conns, lobby, &S2C::State { tick: lobby.race.tick, cars });
        }

        // Reset for another race once the results have been shown.
        if lobby.race.state == RaceState::Finished {
            lobby.results_left -= DT;
            if lobby.results_left <= 0.0 {
                lobby.race = Race::new(castle::track(), MAX_PLAYERS as usize, cfg.laps);
                lobby.ready.clear();
                lobby.inputs.clear();
                lobby.last_phase = Phase::Waiting;
                broadcast(conns, lobby, &S2C::Phase { phase: Phase::Waiting, countdown: 0.0 });
            }
        }
    }
    for name in empty {
        lobbies.remove(&name);
    }
}

fn drop_silent(conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let cutoff = Duration::from_secs(proto::CLIENT_TIMEOUT_SECS);
    let dead: Vec<u64> = conns
        .iter()
        .filter(|(_, c)| c.last_seen.elapsed() > cutoff)
        .map(|(id, _)| *id)
        .collect();
    for id in dead {
        tracing::debug!(conn = id, "dropping silent peer");
        drop_conn(id, conns, lobbies);
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
            conns.insert(
                id,
                Conn {
                    tx,
                    peer,
                    handle: None,
                    proto: 0,
                    lobby: None,
                    last_seen: Instant::now(),
                    msgs_this_tick: 0,
                },
            );
        }
        Ev::Disconnected { id } => drop_conn(id, conns, lobbies),
        Ev::Msg { id, msg } => {
            let Some(c) = conns.get_mut(&id) else {
                // The last silent drop path in the hub. If this ever fires,
                // an Ev::Msg overtook its own connection's Ev::Connected.
                tracing::warn!(conn = id, "message for an unregistered connection: {msg:?}");
                return;
            };
            c.last_seen = Instant::now();
            c.msgs_this_tick += 1;
            if c.msgs_this_tick > MAX_MSGS_PER_TICK {
                tracing::warn!(
                    conn = id,
                    n = c.msgs_this_tick,
                    "over the per-drain message allowance, dropping: {msg:?}"
                );
                return;
            }
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
        C2S::Hello { proto: v, handle } => {
            // One line per connection, not per frame. This is the datum that
            // splits the remaining "no Welcome" failure in half: if it appears
            // and the client still never sees Welcome, the loss is
            // server->client; if it never appears, the Hello never arrived.
            tracing::info!(conn = id, proto = v, "hello");
            if let Some(c) = conns.get_mut(&id) {
                c.proto = v;
                c.handle = Some(proto::sanitize_handle(&handle));
            }
            // Listing is deliberately ungated so the hub's lobby browser
            // works from a frozen page. Entering a race is not.
            send_to(conns, id, &S2C::Welcome { proto: proto::PROTO_VERSION });
        }

        C2S::ListLobbies => {
            let list: Vec<LobbyInfo> = lobbies
                .iter()
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
                        .expect("lobby membership is bounded by MAX_PLAYERS"),
                    cap: MAX_PLAYERS,
                    racing: l.race.state != RaceState::Waiting,
                })
                .collect();
            send_to(conns, id, &S2C::Lobbies { lobbies: list });
        }

        C2S::CreateLobby { name, password } => {
            if !version_ok(id, conns) {
                return;
            }
            let name = proto::sanitize(&name, proto::MAX_LOBBY_LEN);
            if name.is_empty() {
                send_to(conns, id, &S2C::Rejected { reason: "lobby needs a name".into() });
                return;
            }
            if lobbies.contains_key(&name) {
                send_to(conns, id, &S2C::Rejected { reason: "that name is taken".into() });
                return;
            }
            if lobbies.len() >= cfg.max_lobbies {
                send_to(conns, id, &S2C::Rejected { reason: "server is full".into() });
                return;
            }
            let password = password
                .map(|p| proto::sanitize(&p, proto::MAX_PASSWORD_LEN))
                .filter(|p| !p.is_empty());
            lobbies.insert(name.clone(), Lobby::new(password, cfg.laps));
            join_lobby(id, &name, None, conns, lobbies, cfg, true);
        }

        C2S::JoinLobby { name, password } => {
            if !version_ok(id, conns) {
                return;
            }
            let name = proto::sanitize(&name, proto::MAX_LOBBY_LEN);
            join_lobby(id, &name, password.as_deref(), conns, lobbies, cfg, false);
        }

        C2S::LeaveLobby => leave_lobby(id, conns, lobbies),

        C2S::Ready { ready } => {
            let Some(lobby_name) = conns.get(&id).and_then(|c| c.lobby.clone()) else { return };
            if let Some(l) = lobbies.get_mut(&lobby_name) {
                if ready {
                    l.ready.insert(id);
                } else {
                    l.ready.remove(&id);
                }
            }
        }

        C2S::Input { seq, throttle, steer, handbrake, boost } => {
            let Some(lobby_name) = conns.get(&id).and_then(|c| c.lobby.clone()) else { return };
            let Some(l) = lobbies.get_mut(&lobby_name) else { return };
            let Some(&slot) = l.slots.get(&id) else { return };
            let incoming = CarInput { throttle, steer, handbrake, boost }.sanitized();
            match l.inputs.get_mut(&slot) {
                Some((held, last_seq)) => {
                    // Out-of-order or replayed packets must not rewind the
                    // car's intent. A stale packet is simply dropped.
                    if seq <= *last_seq {
                        return;
                    }
                    // A boost press latches until a tick consumes it, so a
                    // press is never lost between two input packets.
                    let pending = held.boost;
                    *held = incoming;
                    held.boost |= pending;
                    *last_seq = seq;
                }
                None => {
                    l.inputs.insert(slot, (incoming, seq));
                }
            }
        }

        C2S::Ping { nonce } => {
            send_to(conns, id, &S2C::Pong { nonce });
        }
    }
}

/// The join gate: exact version equality, and a reason the player can read.
fn version_ok(id: u64, conns: &HashMap<u64, Conn>) -> bool {
    let v = conns.get(&id).map_or(0, |c| c.proto);
    if v == proto::PROTO_VERSION {
        return true;
    }
    // `handle` is set by Hello alongside `proto`, so a None handle here means
    // no Hello was ever processed for this connection — a very different fault
    // from a genuinely stale client, and worth telling apart in the log.
    let saw_hello = conns.get(&id).is_some_and(|c| c.handle.is_some());
    tracing::warn!(conn = id, proto = v, saw_hello, "refusing on protocol version");
    send_to(
        conns,
        id,
        &S2C::Rejected {
            reason: format!(
                "this build speaks fire protocol v{v}, the live game is v{}",
                proto::PROTO_VERSION
            ),
        },
    );
    false
}

fn join_lobby(
    id: u64,
    name: &str,
    password: Option<&str>,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    cfg: &ServerConfig,
    creating: bool,
) {
    // Leaving first keeps a player from occupying two grids at once.
    leave_lobby(id, conns, lobbies);

    let Some(lobby) = lobbies.get_mut(name) else {
        send_to(conns, id, &S2C::Rejected { reason: "no such lobby".into() });
        return;
    };
    if !creating
        && let Some(want) = &lobby.password
        && password.unwrap_or("") != want
    {
        send_to(conns, id, &S2C::Rejected { reason: "wrong password".into() });
        return;
    }
    let Some(slot) = lobby.alloc_slot() else {
        send_to(conns, id, &S2C::Rejected { reason: "lobby is full".into() });
        return;
    };

    lobby.members.push(id);
    lobby.slots.insert(id, slot);
    if let Some(c) = conns.get_mut(&id) {
        c.lobby = Some(name.to_string());
    }

    let meta = PlayerMeta {
        id: slot,
        handle: conns.get(&id).and_then(|c| c.handle.clone()).unwrap_or_else(|| "?".into()),
        slot,
    };
    let list = roster(lobby, conns);
    let phase = lobby.phase();
    let countdown = lobby.race.countdown_left();

    send_to(
        conns,
        id,
        &S2C::Joined { lobby: name.to_string(), id: slot, slot, laps: cfg.laps, roster: list },
    );
    send_to(conns, id, &S2C::Phase { phase, countdown });
    for m in lobby.members.iter().filter(|m| **m != id) {
        send_to(conns, *m, &S2C::PlayerJoined { meta: meta.clone() });
    }
    tracing::info!(conn = id, lobby = name, slot, "joined");
}

fn leave_lobby(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let Some(name) = conns.get_mut(&id).and_then(|c| c.lobby.take()) else { return };
    let Some(lobby) = lobbies.get_mut(&name) else { return };
    lobby.members.retain(|m| *m != id);
    lobby.ready.remove(&id);
    if let Some(slot) = lobby.slots.remove(&id) {
        // Hand the car back to the AI rather than parking it on the racing
        // line for everyone else to hit.
        lobby.inputs.remove(&slot);
        let msg = S2C::PlayerLeft { id: slot };
        for m in &lobby.members {
            send_to(conns, *m, &msg);
        }
    }
    if lobby.members.is_empty() {
        lobbies.remove(&name);
    }
}

fn drop_conn(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    leave_lobby(id, conns, lobbies);
    if let Some(c) = conns.remove(&id) {
        tracing::debug!(conn = id, peer = %c.peer, "disconnected");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lobby_with(members: &[u64]) -> Lobby {
        let mut l = Lobby::new(None, 3);
        for (i, m) in members.iter().enumerate() {
            l.members.push(*m);
            l.slots
                .insert(*m, u8::try_from(i).expect("test lobby fits in a wire player id"));
        }
        l
    }

    #[test]
    fn slots_are_allocated_without_collision() {
        let mut l = lobby_with(&[10, 11, 12]);
        assert_eq!(l.alloc_slot(), Some(3));
        // Free the middle one: the next join must reuse it, not run off the end.
        l.slots.remove(&11);
        assert_eq!(l.alloc_slot(), Some(1));
    }

    #[test]
    fn a_full_lobby_has_no_slot_left() {
        let members: Vec<u64> = (0..u64::from(MAX_PLAYERS)).collect();
        let l = lobby_with(&members);
        assert_eq!(l.alloc_slot(), None);
    }

    /// The grid is always full: every slot without a human gets an AI, so a
    /// lobby of one is still a race.
    #[test]
    fn unclaimed_slots_are_driven_by_the_ai() {
        let mut lobbies = HashMap::new();
        let mut l = lobby_with(&[42]);
        l.race.start_countdown();
        lobbies.insert("test".to_string(), l);
        let conns = HashMap::new();
        let cfg = ServerConfig::default();
        for _ in 0..60 * 40 {
            tick_lobbies(&mut lobbies, &conns, &cfg);
        }
        let l = &lobbies["test"];
        assert_eq!(l.race.state, RaceState::Racing);
        // Slot 0 is the human, who sent nothing and should be parked.
        assert!(l.race.racers[0].car.speed() < 1.0, "the absent human's car drove itself");
        // Everyone else should be racing.
        let moving = l.race.racers.iter().skip(1).filter(|r| r.car.speed() > 5.0).count();
        assert!(moving >= 6, "only {moving} AI cars got going");
    }

    /// The jump bug, in its racing costume: a single input packet must not
    /// boost on every tick until the next packet arrives.
    #[test]
    fn one_boost_packet_spends_one_charge() {
        let mut lobbies = HashMap::new();
        let mut l = lobby_with(&[7]);
        l.race.start_countdown();
        lobbies.insert("t".to_string(), l);
        let mut conns = HashMap::new();
        let (tx, _rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        conns.insert(
            7u64,
            Conn {
                tx,
                peer: "test".into(),
                handle: Some("h".into()),
                proto: proto::PROTO_VERSION,
                lobby: Some("t".into()),
                last_seen: Instant::now(),
                msgs_this_tick: 0,
            },
        );
        let cfg = ServerConfig::default();
        // Run the countdown out first.
        for _ in 0..60 * 4 {
            tick_lobbies(&mut lobbies, &conns, &cfg);
        }
        // One packet with boost set, then many ticks with no further packets.
        handle_msg(
            7,
            C2S::Input { seq: 1, throttle: 1.0, steer: 0.0, handbrake: false, boost: true },
            &mut conns,
            &mut lobbies,
            &cfg,
        );
        for _ in 0..120 {
            tick_lobbies(&mut lobbies, &conns, &cfg);
        }
        let charges = lobbies["t"].race.racers[0].car.boost_charges;
        assert_eq!(
            charges,
            fire_core::car::BOOST_CHARGES - 1,
            "one packet spent {} charges",
            fire_core::car::BOOST_CHARGES - charges
        );
    }

    /// A replayed or reordered packet must not rewind the car's intent.
    #[test]
    fn stale_input_packets_are_ignored() {
        let mut lobbies = HashMap::new();
        lobbies.insert("t".to_string(), lobby_with(&[7]));
        let mut conns = HashMap::new();
        let (tx, _rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        conns.insert(
            7u64,
            Conn {
                tx, peer: "t".into(), handle: Some("h".into()),
                proto: proto::PROTO_VERSION, lobby: Some("t".into()),
                last_seen: Instant::now(), msgs_this_tick: 0,
            },
        );
        let cfg = ServerConfig::default();
        let send = |seq, throttle, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>| {
            handle_msg(
                7,
                C2S::Input { seq, throttle, steer: 0.0, handbrake: false, boost: false },
                conns, lobbies, &cfg,
            );
        };
        send(10, 1.0, &mut conns, &mut lobbies);
        send(10, -1.0, &mut conns, &mut lobbies); // replay, must also be dropped
        send(5, -1.0, &mut conns, &mut lobbies); // stale, must be dropped
        let (held, seq) = lobbies["t"].inputs[&0];
        assert_eq!(seq, 10);
        assert_eq!(held.throttle, 1.0, "a stale packet overwrote the current intent");
    }

    #[test]
    fn a_mismatched_version_cannot_enter_a_race() {
        let mut conns = HashMap::new();
        let (tx, rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        conns.insert(
            1u64,
            Conn {
                tx, peer: "t".into(), handle: Some("h".into()),
                proto: proto::PROTO_VERSION + 1, lobby: None,
                last_seen: Instant::now(), msgs_this_tick: 0,
            },
        );
        assert!(!version_ok(1, &conns), "a mismatched peer was let in");
        let Ok(Message::Text(t)) = rx.try_recv() else { panic!("no rejection sent") };
        let msg: S2C = serde_json::from_str(&t).unwrap();
        match msg {
            S2C::Rejected { reason } => assert!(reason.contains("fire protocol"), "{reason}"),
            other => panic!("wrong message: {other:?}"),
        }
    }

    /// Leaving must free the slot and hand the car back to the AI, not park
    /// it on the racing line.
    #[test]
    fn leaving_frees_the_slot() {
        let mut lobbies = HashMap::new();
        lobbies.insert("t".to_string(), lobby_with(&[7, 8]));
        let mut conns = HashMap::new();
        for id in [7u64, 8] {
            let (tx, _rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
            conns.insert(
                id,
                Conn {
                    tx, peer: "t".into(), handle: Some("h".into()),
                    proto: proto::PROTO_VERSION, lobby: Some("t".into()),
                    last_seen: Instant::now(), msgs_this_tick: 0,
                },
            );
        }
        lobbies.get_mut("t").unwrap().inputs.insert(0, (CarInput::default(), 3));
        leave_lobby(7, &mut conns, &mut lobbies);
        let l = &lobbies["t"];
        assert!(!l.members.contains(&7));
        assert!(!l.slots.contains_key(&7));
        assert!(!l.inputs.contains_key(&0), "the leaver's intent outlived them");
        assert_eq!(l.alloc_slot(), Some(0), "the freed slot was not reusable");
    }

    /// An empty lobby must not linger and keep its name reserved.
    #[test]
    fn the_last_player_out_removes_the_lobby() {
        let mut lobbies = HashMap::new();
        lobbies.insert("t".to_string(), lobby_with(&[7]));
        let mut conns = HashMap::new();
        let (tx, _rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        conns.insert(
            7u64,
            Conn {
                tx, peer: "t".into(), handle: Some("h".into()),
                proto: proto::PROTO_VERSION, lobby: Some("t".into()),
                last_seen: Instant::now(), msgs_this_tick: 0,
            },
        );
        leave_lobby(7, &mut conns, &mut lobbies);
        assert!(lobbies.is_empty(), "an empty lobby held its name");
    }
}
