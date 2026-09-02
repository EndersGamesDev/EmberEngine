//! The authoritative Four Kings server: WebSocket lobbies, one board each.
//!
//! Structure is `fire-server`'s, because that shape is already proven
//! against this deployment path (Cloudflare quick tunnel in front of a plain
//! TCP listener). One thread per connection *owns* its socket and alternates
//! a short blocking read with draining a bounded outbound queue; a single hub
//! thread owns every lobby. Nothing is shared by lock; everything crosses a
//! channel.
//!
//! There is no simulation to step. The hub polls every `HUB_POLL`, drains
//! events, then feeds the elapsed milliseconds to the pure `tick_lobby` of
//! every lobby, which is where the 15-second clock lives: it repeats `Clock`
//! once a second and applies `kings_core::timeout` when a turn runs out.
//! Every lobby transition (`join`, `leave`, `set_formation`, `start`,
//! `do_move`, `tick`) is a method on `Lobby` that returns the messages to
//! send instead of sending them, so the unit tests drive a whole game with
//! synthetic time and never open a socket.
//!
//! The authority story: the server runs `kings_core`, the same rules the
//! client uses for highlights. Clients send intents stamped with the turn
//! they were computed against; the server applies or refuses, and every
//! accepted action goes out as a full `State`.
//!
//! Section numbers in the comments refer to `docs/kings-design.md`.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tungstenite::Message;
use tungstenite::protocol::WebSocketConfig;

use kings_core::board::{SEAT_BY_JOIN, SEATS, setup, to_state};
use kings_core::proto::{
    self, C2S, Formation, LobbyInfo, MAX_PLAYERS, MIN_PLAYERS, Phase, PlayerMeta, S2C,
};
use kings_core::{State, apply_move, disconnect, timeout};

/// `r<N>` of this build from the deploy's `EMBER_BUILD_VERSION`, or `""`
/// for an unstamped dev build (`docs/hosts.md`, section 4).
pub const BUILD_VERSION: &str = match option_env!("EMBER_BUILD_VERSION") {
    Some(v) => v,
    None => "",
};
/// Short commit hash of this build from `EMBER_BUILD_COMMIT`, or `""`.
pub const BUILD_COMMIT: &str = match option_env!("EMBER_BUILD_COMMIT") {
    Some(v) => v,
    None => "",
};

/// Outbound queue depth per connection. A board game sends a few kilobytes
/// per turn, so this is generous; it is bounded so a dead peer cannot make
/// the hub buffer on its behalf for ever.
const OUTBOUND_QUEUE: usize = 256;
const MAX_WS_MESSAGE: usize = proto::MAX_FRAME_BYTES;
/// A client that has not finished the WebSocket handshake by now is not a
/// client. Without this, a peer dribbling one byte per read window holds a
/// connection slot indefinitely.
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(10);
/// How often the hub drains events and advances the clocks.
const HUB_POLL: Duration = Duration::from_millis(10);
/// Message allowance per connection per `MSG_WINDOW`. A board-game client
/// sends about one message a second; a peer above this is flooding and is
/// dropped, as `pong-server` does.
const MAX_MSGS_PER_WINDOW: u32 = 32;
const MSG_WINDOW: Duration = Duration::from_millis(100);
/// Hard ceiling on live connections.
const MAX_CONNS: usize = 512;
/// Cap per remote IP for DIRECT exposure. Loopback is exempt: behind the
/// Cloudflare tunnel every peer is 127.0.0.1 (the edge applies its own
/// per-source protections there).
const MAX_CONNS_PER_IP: u32 = 6;
/// How long Finished stays on screen before the lobby returns to Waiting.
const RESULTS_MS: u32 = proto::RESULTS_SECS * 1000;

/// Tunables. Production is the default; the e2e tests shorten the turn.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Length of a turn in server milliseconds before the grace window.
    /// `kings_core::proto::TURN_MS` in production; tests use 1000 so a
    /// silent turn passes in about a second.
    pub turn_ms: u32,
    /// Most lobbies at once; `CreateLobby` is refused past this.
    pub max_lobbies: usize,
    /// The host name sent in every `Welcome`; `""` when unnamed.
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            turn_ms: proto::TURN_MS,
            max_lobbies: 32,
            host: String::new(),
        }
    }
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
    /// Set by Hello, which must be the first message. `None` means no Hello
    /// has been processed yet.
    handle: Option<String>,
    /// From Hello. Listing is allowed at any version (the lobby browser has
    /// no game loaded) but entering a lobby requires an exact match.
    proto: u16,
    lobby: Option<String>,
    last_seen: Instant,
    msgs_this_window: u32,
}

/// Who a lobby message is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Recipient {
    /// One connection.
    One(u64),
    /// Every member of the lobby, as of after the transition.
    Members,
}

type Outbox = Vec<(Recipient, S2C)>;

/// One human in a lobby.
#[derive(Clone, Debug)]
struct Member {
    conn: u64,
    /// Lobby-local id on the wire (`PlayerMeta::id`, `Joined::id`,
    /// `Roster::creator`): the smallest free value in `0..MAX_PLAYERS`,
    /// stable while the member stays.
    id: u8,
    handle: String,
    /// `SEAT_BY_JOIN[position]` while Waiting; frozen at Start.
    seat: u8,
}

/// A lobby IS a table: at most four humans, one board, one phase.
///
/// Seats follow join order through `SEAT_BY_JOIN` and are recomputed on every
/// Waiting roster change (section 1.4). While Waiting the board is the setup
/// for the seats held so far, garrisons in the empty corners, rebuilt on every
/// join, leave and formation change; at Start it becomes the game.
struct Lobby {
    name: String,
    password: Option<String>,
    /// Conn id of the creator; the longest-present member takes over when
    /// the creator leaves.
    creator: u64,
    /// In join order.
    members: Vec<Member>,
    /// Per conn id; the default for anyone who never sent `SetFormation`.
    formations: HashMap<u64, Formation>,
    state: State,
    phase: Phase,
    /// Length of a turn before the grace, from `ServerConfig`.
    turn_ms: u32,
    /// Server milliseconds since the current turn began. The authority on
    /// the clock: `State::left_ms` and `Clock::left_ms` derive from it.
    elapsed_ms: u32,
    /// Accumulator for the once-a-second `Clock`.
    clock_acc_ms: u32,
    /// Milliseconds of Finished left before the lobby returns to Waiting.
    results_left_ms: u32,
}

impl Lobby {
    fn new(name: String, password: Option<String>, turn_ms: u32) -> Self {
        Self {
            name,
            password,
            creator: 0,
            members: Vec::new(),
            formations: HashMap::new(),
            state: setup([false; SEATS], [Formation::DEFAULT; SEATS]),
            phase: Phase::Waiting,
            turn_ms,
            elapsed_ms: 0,
            clock_acc_ms: 0,
            results_left_ms: 0,
        }
    }

    fn member(&self, conn: u64) -> Option<&Member> {
        self.members.iter().find(|m| m.conn == conn)
    }

    fn player_count(&self) -> u8 {
        u8::try_from(self.members.len()).expect("membership is capped at MAX_PLAYERS")
    }

    /// Smallest unused lobby-local id. Scanning beats a wrapping counter: in
    /// a long-lived lobby a counter eventually collides with a sitting player.
    fn alloc_id(&self) -> Option<u8> {
        (0..MAX_PLAYERS).find(|id| !self.members.iter().any(|m| m.id == *id))
    }

    /// Seats by join order (section 1.4). Waiting only; a running game keeps
    /// the seats it started with.
    fn reseat(&mut self) {
        for (i, m) in self.members.iter_mut().enumerate() {
            m.seat = SEAT_BY_JOIN[i];
        }
    }

    /// The Waiting board: the seats held so far, garrisons elsewhere, each
    /// member's own formation.
    fn waiting_board(&self) -> State {
        let mut present = [false; SEATS];
        let mut formations = [Formation::DEFAULT; SEATS];
        for m in &self.members {
            present[usize::from(m.seat)] = true;
            formations[usize::from(m.seat)] =
                self.formations.get(&m.conn).copied().unwrap_or_default();
        }
        setup(present, formations)
    }

    /// Rebuild the Waiting board after a roster or formation change.
    fn rebuild(&mut self) {
        self.state = self.waiting_board();
        self.elapsed_ms = 0;
        self.clock_acc_ms = 0;
    }

    /// What the player sees on the clock: the server's counter, never the
    /// core's (`to_state` would report the core's 15 s clock, which the
    /// tests shorten through `turn_ms`).
    const fn left_ms(&self) -> u32 {
        self.turn_ms.saturating_sub(self.elapsed_ms)
    }

    fn board_msg(&self) -> S2C {
        let mut board = to_state(&self.state);
        board.left_ms = self.left_ms();
        S2C::State { board }
    }

    fn roster_msg(&self) -> S2C {
        S2C::Roster {
            creator: self.member(self.creator).map_or(0, |m| m.id),
            roster: self
                .members
                .iter()
                .map(|m| PlayerMeta {
                    id: m.id,
                    handle: m.handle.clone(),
                    seat: m.seat,
                })
                .collect(),
        }
    }

    fn phase_msg(&self) -> S2C {
        let result = self.state.result.filter(|_| self.phase == Phase::Finished);
        S2C::Phase {
            phase: self.phase,
            winner: result.and_then(|r| r.winner),
            end: result.map(|r| r.end),
        }
    }

    /// The spec's start notification (section 1.4): to the creator whenever
    /// the lobby holds at least `MIN_PLAYERS`, on every roster change.
    fn push_can_start(&self, out: &mut Outbox) {
        if self.phase == Phase::Waiting && self.player_count() >= MIN_PLAYERS {
            out.push((
                Recipient::One(self.creator),
                S2C::CanStart {
                    players: self.player_count(),
                },
            ));
        }
    }

    /// A Waiting roster change: reseat, rebuild, tell everyone.
    fn roster_changed(&mut self, out: &mut Outbox) {
        self.reseat();
        self.rebuild();
        out.push((Recipient::Members, self.roster_msg()));
        out.push((Recipient::Members, self.board_msg()));
        self.push_can_start(out);
    }

    /// Seat a new member. The password is the caller's business.
    fn join(&mut self, conn: u64, handle: String) -> Result<Outbox, &'static str> {
        if self.phase != Phase::Waiting {
            return Err("that game has already started");
        }
        let Some(id) = self.alloc_id() else {
            return Err("lobby is full");
        };
        if self.members.is_empty() {
            self.creator = conn;
        }
        self.members.push(Member {
            conn,
            id,
            handle,
            seat: 0,
        });
        let mut out = vec![(
            Recipient::One(conn),
            S2C::Joined {
                lobby: self.name.clone(),
                id,
            },
        )];
        // Joined, then Roster, State, Phase for the joiner (the proto's
        // documented order); the others see the Roster and State.
        self.roster_changed(&mut out);
        let phase = self.phase_msg();
        // The joiner's Phase goes after the broadcast pair so its own queue
        // reads Joined, Roster, State, Phase; CanStart (to the creator) may
        // sit between, which is harmless.
        out.push((Recipient::One(conn), phase));
        Ok(out)
    }

    /// A member left or dropped. While Waiting the seat is freed and the
    /// rest reseated; mid-game the seat is eliminated (section 1.7). The
    /// creator's departure hands the lobby to the longest-present member.
    fn leave(&mut self, conn: u64) -> Outbox {
        let mut out = Outbox::new();
        let Some(pos) = self.members.iter().position(|m| m.conn == conn) else {
            return out;
        };
        let gone = self.members.remove(pos);
        self.formations.remove(&conn);
        if self.creator == conn
            && let Some(first) = self.members.first()
        {
            self.creator = first.conn;
        }
        if self.members.is_empty() {
            return out;
        }
        match self.phase {
            Phase::Waiting => self.roster_changed(&mut out),
            Phase::Playing => {
                let turn = self.state.turn;
                disconnect(&mut self.state, gone.seat);
                if self.state.turn != turn {
                    self.elapsed_ms = 0;
                    self.clock_acc_ms = 0;
                }
                out.push((Recipient::Members, self.roster_msg()));
                out.push((Recipient::Members, self.board_msg()));
                self.settle(&mut out);
            }
            Phase::Finished => {
                disconnect(&mut self.state, gone.seat);
                out.push((Recipient::Members, self.roster_msg()));
                out.push((Recipient::Members, self.board_msg()));
            }
        }
        out
    }

    /// The pre-game card swap (section 2): Waiting only, validated,
    /// rebroadcast as a new Waiting board.
    fn set_formation(&mut self, conn: u64, formation: Formation) -> Result<Outbox, &'static str> {
        if self.phase != Phase::Waiting {
            return Err("formations are frozen once the game has started");
        }
        if self.member(conn).is_none() {
            return Err("you are not at this table");
        }
        formation.validate().map_err(|e| e.reason())?;
        self.formations.insert(conn, formation);
        self.rebuild();
        Ok(vec![(Recipient::Members, self.board_msg())])
    }

    /// Start the game (section 1.4): creator only, Waiting only, at least
    /// `MIN_PLAYERS`. Seat 0's clock starts now.
    fn start(&mut self, conn: u64) -> Result<Outbox, &'static str> {
        if conn != self.creator {
            return Err("only the creator can start the game");
        }
        if self.phase != Phase::Waiting {
            return Err("the game has already started");
        }
        if self.player_count() < MIN_PLAYERS {
            return Err("the game needs at least two players");
        }
        self.state = self.waiting_board();
        self.phase = Phase::Playing;
        self.elapsed_ms = 0;
        self.clock_acc_ms = 0;
        let mut out = vec![
            (Recipient::Members, self.phase_msg()),
            (Recipient::Members, self.board_msg()),
        ];
        // A first seat with no legal move is passed by `setup`'s successor
        // logic only at end_turn; the setup itself always has moves
        // (section 1.3), but a result at turn 1 is still handled uniformly.
        self.settle(&mut out);
        Ok(out)
    }

    /// One action from a member (section 1.5 and 1.6). Refusals name the
    /// first thing wrong; the board is untouched by a refusal.
    fn do_move(
        &mut self,
        conn: u64,
        turn: u32,
        from: (u8, u8),
        to: (u8, u8),
    ) -> Result<Outbox, &'static str> {
        match self.phase {
            Phase::Waiting => return Err("the game has not started"),
            Phase::Finished => return Err("the game is over"),
            Phase::Playing => {}
        }
        let seat = self.member(conn).ok_or("you are not at this table")?.seat;
        // The server's counter is the authority: past the grace the timeout
        // is due and no move lands, whatever the core's own clock says.
        if self.elapsed_ms >= self.turn_ms.saturating_add(proto::GRACE_MS) {
            return Err("the turn has ended");
        }
        let before = self.state.turn;
        apply_move(&mut self.state, seat, turn, from, to).map_err(|e| e.reason())?;
        if self.state.turn != before {
            self.elapsed_ms = 0;
            self.clock_acc_ms = 0;
        }
        let mut out = vec![(Recipient::Members, self.board_msg())];
        self.settle(&mut out);
        Ok(out)
    }

    /// After any change to a Playing board: if the game has a result, move
    /// to Finished, announce it, and start the results countdown.
    fn settle(&mut self, out: &mut Outbox) {
        if self.phase == Phase::Playing && self.state.result.is_some() {
            self.phase = Phase::Finished;
            self.results_left_ms = RESULTS_MS;
            out.push((Recipient::Members, self.phase_msg()));
        }
    }

    /// Advance the lobby by `ms` of server time.
    fn tick(&mut self, ms: u32, out: &mut Outbox) {
        match self.phase {
            Phase::Waiting => {}
            Phase::Playing => {
                self.elapsed_ms = self.elapsed_ms.saturating_add(ms);
                self.clock_acc_ms = self.clock_acc_ms.saturating_add(ms);
                // The core's clock is fed too so `State` stays consistent
                // with a client that reconstructs it; the server's counter
                // decides.
                self.state.clock.tick(ms);
                if self.elapsed_ms >= self.turn_ms.saturating_add(proto::GRACE_MS) {
                    timeout(&mut self.state);
                    self.elapsed_ms = 0;
                    self.clock_acc_ms = 0;
                    out.push((Recipient::Members, self.board_msg()));
                    self.settle(out);
                } else if self.clock_acc_ms >= proto::CLOCK_EVERY_MS {
                    // One Clock per second of accumulated time, never a burst
                    // after a stall: the page only needs the latest value.
                    self.clock_acc_ms %= proto::CLOCK_EVERY_MS;
                    out.push((
                        Recipient::Members,
                        S2C::Clock {
                            turn: self.state.turn,
                            seat: self.state.to_move,
                            left_ms: self.left_ms(),
                        },
                    ));
                }
            }
            Phase::Finished => {
                self.results_left_ms = self.results_left_ms.saturating_sub(ms);
                if self.results_left_ms == 0 {
                    // Back to Waiting with the same members and creator and a
                    // fresh Waiting board (section 1.8). Seats are recomputed
                    // in case someone left mid-game.
                    self.phase = Phase::Waiting;
                    out.push((Recipient::Members, self.phase_msg()));
                    self.roster_changed(out);
                }
            }
        }
    }
}

/// Advance one lobby by `elapsed_ms` of server time, collecting what to send.
///
/// Pure: the hub calls it with wall-clock deltas, the tests with synthetic
/// ones. Every `CLOCK_EVERY_MS` of a running turn it emits `Clock`; when the
/// turn reaches `turn_ms + GRACE_MS` it applies the timeout pass, emits
/// `State` (and `Phase` if that ended the game) and starts the next turn's
/// count; a Finished lobby returns to Waiting after `RESULTS_SECS`.
fn tick_lobby(lobby: &mut Lobby, elapsed_ms: u32, out: &mut Outbox) {
    lobby.tick(elapsed_ms, out);
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
    let build = if BUILD_VERSION.is_empty() && BUILD_COMMIT.is_empty() {
        "unstamped dev build".to_string()
    } else {
        format!("build {BUILD_VERSION} {BUILD_COMMIT}")
    };
    tracing::info!(
        addr = %local,
        proto = proto::PROTO_VERSION,
        host = %cfg.host,
        build = %build,
        turn_ms = cfg.turn_ms,
        "kings-server listening"
    );

    let (events_tx, events_rx) = mpsc::channel::<Ev>();
    let hub = thread::spawn(move || hub_loop(&events_rx, &cfg));

    let live_conns = Arc::new(AtomicUsize::new(0));
    let per_ip: Arc<Mutex<HashMap<IpAddr, u32>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut next_id = 1u64;
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("accept failed: {e}");
                continue;
            }
        };
        if live_conns.load(Ordering::Relaxed) >= MAX_CONNS {
            tracing::warn!("connection cap reached, refusing peer");
            continue; // stream drops -> RST/FIN
        }
        // Per-IP cap for direct exposure (loopback = tunnel, exempt).
        let ip = stream.peer_addr().ok().map(|a| a.ip());
        if let Some(ip) = ip
            && !ip.is_loopback()
        {
            let mut map = per_ip.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let tx = events_tx.clone();
        let live_conns = Arc::clone(&live_conns);
        let per_ip = Arc::clone(&per_ip);
        thread::spawn(move || {
            conn_thread(id, stream, &tx);
            live_conns.fetch_sub(1, Ordering::Relaxed);
            if let Some(ip) = ip
                && !ip.is_loopback()
            {
                let mut map = per_ip.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(c) = map.get_mut(&ip) {
                    *c -= 1;
                    if *c == 0 {
                        map.remove(&ip);
                    }
                }
            }
        });
    }
    drop(events_tx);
    drop(hub.join());
    Ok(())
}

fn conn_thread(id: u64, stream: TcpStream, events_tx: &Sender<Ev>) {
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "?".into(), |address| address.to_string());
    drop(stream.set_nodelay(true));
    drop(stream.set_read_timeout(Some(Duration::from_secs(10))));
    drop(stream.set_write_timeout(Some(Duration::from_secs(15))));

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

    // This thread owns the socket, so the outbound queue only drains between
    // reads. A short read timeout keeps a broadcast from sitting on the queue
    // for most of a poll before it reaches the wire.
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
                    // write() buffers inside tungstenite; flush() puts it on
                    // the wire. A flush that would block has NOT lost the
                    // message: it is still buffered and the next flush sends
                    // it, so tearing the connection down here would drop a
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
                    // The hub dropped this connection (protocol violation,
                    // flood, silence): close politely and stop.
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
                            tracing::warn!(conn = id, "hub channel closed; dropping inbound");
                            break;
                        }
                    }
                    // An unparseable frame is the peer's problem, not grounds
                    // to drop them: a newer client may send a variant this
                    // build has never heard of. But it is NOT debug-level: a
                    // frame we cannot read is indistinguishable, from the
                    // other end, from one that was never delivered.
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

/// Send a lobby transition's outbox. `Members` is resolved against the
/// lobby as it is now, i.e. after the transition: a leaver is not told about
/// their own departure, a joiner is in the broadcast that announces them.
fn deliver(conns: &HashMap<u64, Conn>, lobby: &Lobby, out: Outbox) {
    for (to, msg) in out {
        match to {
            Recipient::One(id) => {
                send_to(conns, id, &msg);
            }
            Recipient::Members => {
                for m in &lobby.members {
                    send_to(conns, m.conn, &msg);
                }
            }
        }
    }
}

/// The hub: owns every connection record and every lobby until the accept
/// loop drops its end of the channel.
fn hub_loop(events_rx: &Receiver<Ev>, cfg: &ServerConfig) {
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut lobbies: HashMap<String, Lobby> = HashMap::new();
    let mut last = Instant::now();
    let mut window_start = last;

    loop {
        // Drain everything that arrived, then advance the clocks by the
        // time that actually passed. Draining first means a move that lands
        // just before a timeout is applied rather than timed out.
        loop {
            match events_rx.try_recv() {
                Ok(ev) => handle_event(ev, &mut conns, &mut lobbies, cfg),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        // Whole milliseconds only; the remainder is carried, not lost.
        let now = Instant::now();
        let ms = u32::try_from(now.duration_since(last).as_millis()).unwrap_or(u32::MAX);
        last += Duration::from_millis(u64::from(ms));
        if ms > 0 {
            tick_lobbies(&mut lobbies, &conns, ms);
        }
        if now.duration_since(window_start) >= MSG_WINDOW {
            window_start = now;
            for conn in conns.values_mut() {
                conn.msgs_this_window = 0;
            }
        }
        drop_silent(&mut conns, &mut lobbies);

        thread::sleep(HUB_POLL);
    }
}

fn tick_lobbies(lobbies: &mut HashMap<String, Lobby>, conns: &HashMap<u64, Conn>, ms: u32) {
    let mut empty: Vec<String> = Vec::new();
    let mut out = Outbox::new();
    for (name, lobby) in lobbies.iter_mut() {
        if lobby.members.is_empty() {
            empty.push(name.clone());
            continue;
        }
        out.clear();
        tick_lobby(lobby, ms, &mut out);
        deliver(conns, lobby, std::mem::take(&mut out));
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
                    msgs_this_window: 0,
                },
            );
        }
        Ev::Disconnected { id } => drop_conn(id, conns, lobbies),
        Ev::Msg { id, msg } => {
            let Some(c) = conns.get_mut(&id) else {
                // If this ever fires, an Ev::Msg overtook its own
                // connection's Ev::Connected.
                tracing::warn!(conn = id, "message for an unregistered connection: {msg:?}");
                return;
            };
            c.last_seen = Instant::now();
            c.msgs_this_window += 1;
            if c.msgs_this_window > MAX_MSGS_PER_WINDOW {
                tracing::warn!(
                    conn = id,
                    n = c.msgs_this_window,
                    "message flood, dropping the peer: {msg:?}"
                );
                drop_conn(id, conns, lobbies);
                return;
            }
            handle_msg(id, msg, conns, lobbies, cfg);
        }
    }
}

/// Humans in games: members of lobbies whose phase is Playing. The
/// `Welcome.players` figure of `docs/hosts.md`.
fn players_in_games(lobbies: &HashMap<String, Lobby>) -> u32 {
    lobbies
        .values()
        .filter(|l| l.phase == Phase::Playing)
        .map(|l| u32::from(l.player_count()))
        .sum()
}

/// Open lobbies: lobbies whose phase is Waiting, the ones a player could
/// join right now. The `Welcome.lobbies` figure of `docs/hosts.md`.
fn open_lobbies(lobbies: &HashMap<String, Lobby>) -> u32 {
    u32::try_from(
        lobbies
            .values()
            .filter(|l| l.phase == Phase::Waiting)
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn handle_msg(
    id: u64,
    msg: C2S,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    cfg: &ServerConfig,
) {
    let saw_hello = conns.get(&id).is_some_and(|c| c.handle.is_some());
    match (msg, saw_hello) {
        (C2S::Hello { proto: v, handle }, false) => {
            // One line per connection, not per frame: if it appears and the
            // client still never sees Welcome, the loss is server->client;
            // if it never appears, the Hello never arrived.
            tracing::info!(conn = id, proto = v, "hello");
            if let Some(c) = conns.get_mut(&id) {
                c.proto = v;
                c.handle = Some(proto::sanitize_handle(&handle));
            }
            // Listing is deliberately ungated so the hub's lobby browser
            // works from a frozen page. Entering a lobby is not.
            send_to(
                conns,
                id,
                &S2C::Welcome {
                    proto: proto::PROTO_VERSION,
                    host: cfg.host.clone(),
                    version: BUILD_VERSION.into(),
                    commit: BUILD_COMMIT.into(),
                    players: players_in_games(lobbies),
                    lobbies: open_lobbies(lobbies),
                },
            );
        }
        (C2S::Hello { .. }, true) => {
            tracing::debug!(conn = id, "duplicate Hello; dropping");
            drop_conn(id, conns, lobbies);
        }
        // Anything else before Hello, Ping included, is a protocol violation
        // (pre-Hello pings could park a slot): the connection is closed.
        (msg, false) => {
            tracing::debug!(conn = id, "message before Hello; dropping: {msg:?}");
            drop_conn(id, conns, lobbies);
        }

        (C2S::ListLobbies, true) => {
            let list: Vec<LobbyInfo> = lobbies
                .iter()
                .map(|(name, l)| LobbyInfo {
                    name: name.clone(),
                    host: l
                        .member(l.creator)
                        .map_or_else(|| "?".into(), |m| m.handle.clone()),
                    has_password: l.password.is_some(),
                    players: l.player_count(),
                    cap: MAX_PLAYERS,
                    // True from Start until the lobby is back in Waiting: a
                    // Finished table is not joinable either.
                    playing: l.phase != Phase::Waiting,
                })
                .collect();
            send_to(conns, id, &S2C::Lobbies { lobbies: list });
        }

        (C2S::CreateLobby { name, password }, true) => {
            create_lobby(id, &name, password, conns, lobbies, cfg);
        }

        (C2S::JoinLobby { name, password }, true) => {
            if !version_ok(id, conns) {
                return;
            }
            let name = proto::sanitize(&name, proto::MAX_LOBBY_LEN);
            join_lobby(id, &name, password.as_deref(), conns, lobbies, false);
        }

        (C2S::LeaveLobby, true) => leave_lobby(id, conns, lobbies),

        (C2S::SetFormation { formation }, true) => {
            if !version_ok(id, conns) {
                return;
            }
            with_lobby(id, conns, lobbies, |lobby| lobby.set_formation(id, formation));
        }

        (C2S::Start, true) => {
            if !version_ok(id, conns) {
                return;
            }
            with_lobby(id, conns, lobbies, |lobby| lobby.start(id));
        }

        (
            C2S::Move {
                turn,
                fx,
                fy,
                tx,
                ty,
            },
            true,
        ) => {
            if !version_ok(id, conns) {
                return;
            }
            with_lobby(id, conns, lobbies, |lobby| {
                lobby.do_move(id, turn, (fx, fy), (tx, ty))
            });
        }

        (C2S::Ping { nonce }, true) => {
            send_to(conns, id, &S2C::Pong { nonce });
        }
    }
}

/// Run a lobby transition for the sender's lobby: deliver its outbox, or
/// send the refusal to the sender alone.
fn with_lobby(
    id: u64,
    conns: &HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    f: impl FnOnce(&mut Lobby) -> Result<Outbox, &'static str>,
) {
    let lobby = conns
        .get(&id)
        .and_then(|c| c.lobby.as_ref())
        .and_then(|name| lobbies.get_mut(name));
    let result = match lobby {
        Some(lobby) => f(lobby).map(|out| (out, &*lobby)),
        None => Err("you are not in a lobby"),
    };
    match result {
        Ok((out, lobby)) => deliver(conns, lobby, out),
        Err(reason) => {
            send_to(
                conns,
                id,
                &S2C::Rejected {
                    reason: reason.into(),
                },
            );
        }
    }
}

fn create_lobby(
    id: u64,
    requested_name: &str,
    password: Option<String>,
    conns: &mut HashMap<u64, Conn>,
    lobbies: &mut HashMap<String, Lobby>,
    cfg: &ServerConfig,
) {
    if !version_ok(id, conns) {
        return;
    }
    let name = proto::sanitize(requested_name, proto::MAX_LOBBY_LEN);
    let rejection = if name.is_empty() {
        Some("lobby needs a name")
    } else if lobbies.contains_key(&name) {
        Some("that name is taken")
    } else if lobbies.len() >= cfg.max_lobbies {
        Some("server is full")
    } else {
        None
    };
    if let Some(reason) = rejection {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: reason.into(),
            },
        );
        return;
    }

    let password = password
        .map(|value| proto::sanitize(&value, proto::MAX_PASSWORD_LEN))
        .filter(|value| !value.is_empty());
    lobbies.insert(name.clone(), Lobby::new(name.clone(), password, cfg.turn_ms));
    join_lobby(id, &name, None, conns, lobbies, true);
}

/// The join gate: exact version equality, and a reason the player can read.
fn version_ok(id: u64, conns: &HashMap<u64, Conn>) -> bool {
    let v = conns.get(&id).map_or(0, |c| c.proto);
    if v == proto::PROTO_VERSION {
        return true;
    }
    tracing::warn!(conn = id, proto = v, "refusing on protocol version");
    send_to(
        conns,
        id,
        &S2C::Rejected {
            reason: format!(
                "this build speaks kings protocol v{v}, the live game is v{}",
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
    creating: bool,
) {
    // Re-joining the table one already sits at would first leave it, which
    // for a lone creator deletes the lobby; refuse instead.
    if conns.get(&id).and_then(|c| c.lobby.as_deref()) == Some(name) {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "you are already at that table".into(),
            },
        );
        return;
    }
    // Leaving first keeps a player from sitting at two tables at once. A
    // mid-game leave is an elimination, and that is the player's choice.
    leave_lobby(id, conns, lobbies);

    let Some(lobby) = lobbies.get_mut(name) else {
        send_to(
            conns,
            id,
            &S2C::Rejected {
                reason: "no such lobby".into(),
            },
        );
        return;
    };
    if !creating
        && let Some(want) = &lobby.password
        && password.unwrap_or("") != want
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
    let handle = conns
        .get(&id)
        .and_then(|c| c.handle.clone())
        .unwrap_or_else(|| "player".into());
    match lobby.join(id, handle) {
        Ok(out) => {
            if let Some(c) = conns.get_mut(&id) {
                c.lobby = Some(name.to_string());
            }
            let seat = lobby.member(id).map_or(0, |m| m.seat);
            tracing::info!(conn = id, lobby = name, seat, "joined");
            deliver(conns, lobby, out);
        }
        Err(reason) => {
            send_to(
                conns,
                id,
                &S2C::Rejected {
                    reason: reason.into(),
                },
            );
        }
    }
}

fn leave_lobby(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    let Some(name) = conns.get_mut(&id).and_then(|c| c.lobby.take()) else {
        return;
    };
    let Some(lobby) = lobbies.get_mut(&name) else {
        return;
    };
    let out = lobby.leave(id);
    deliver(conns, lobby, out);
    if lobby.members.is_empty() {
        lobbies.remove(&name);
    }
}

fn drop_conn(id: u64, conns: &mut HashMap<u64, Conn>, lobbies: &mut HashMap<String, Lobby>) {
    leave_lobby(id, conns, lobbies);
    if let Some(c) = conns.remove(&id) {
        // Dropping `tx` is what closes the socket: the connection thread
        // sees the queue disconnect and sends Close.
        tracing::debug!(conn = id, peer = %c.peer, "disconnected");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kings_core::Kind;
    use kings_core::proto::{ActionKind, BoardState, EndReason, GRACE_MS, TURN_MS};

    const ADA: u64 = 11;
    const BOB: u64 = 22;
    const CY: u64 = 33;

    fn table(turn_ms: u32) -> Lobby {
        let mut l = Lobby::new("court".into(), None, turn_ms);
        l.join(ADA, "ada".into()).expect("creator joins");
        l.join(BOB, "bob".into()).expect("guest joins");
        l
    }

    fn playing(turn_ms: u32) -> Lobby {
        let mut l = table(turn_ms);
        l.start(ADA).expect("creator starts");
        l
    }

    fn boards(out: &Outbox) -> Vec<&BoardState> {
        out.iter()
            .filter_map(|(_, m)| match m {
                S2C::State { board } => Some(board),
                _ => None,
            })
            .collect()
    }

    fn clocks(out: &Outbox) -> Vec<(u32, u8, u32)> {
        out.iter()
            .filter_map(|(_, m)| match m {
                S2C::Clock {
                    turn,
                    seat,
                    left_ms,
                } => Some((*turn, *seat, *left_ms)),
                _ => None,
            })
            .collect()
    }

    fn phases(out: &Outbox) -> Vec<(Phase, Option<u8>, Option<EndReason>)> {
        out.iter()
            .filter_map(|(_, m)| match m {
                S2C::Phase { phase, winner, end } => Some((*phase, *winner, *end)),
                _ => None,
            })
            .collect()
    }

    fn rosters(out: &Outbox) -> Vec<(u8, Vec<(u8, String, u8)>)> {
        out.iter()
            .filter_map(|(_, m)| match m {
                S2C::Roster { creator, roster } => Some((
                    *creator,
                    roster
                        .iter()
                        .map(|p| (p.id, p.handle.clone(), p.seat))
                        .collect(),
                )),
                _ => None,
            })
            .collect()
    }

    fn can_starts(out: &Outbox) -> Vec<(Recipient, u8)> {
        out.iter()
            .filter_map(|(to, m)| match m {
                S2C::CanStart { players } => Some((*to, *players)),
                _ => None,
            })
            .collect()
    }

    fn tick_by(l: &mut Lobby, ms: u32) -> Outbox {
        let mut out = Outbox::new();
        tick_lobby(l, ms, &mut out);
        out
    }

    /// Two players sit diagonally, the creator at seat 0; the Waiting board
    /// is a full 64-piece setup with garrisons in the empty corners; the
    /// joiner reads Joined, Roster, State, Phase in that order.
    #[test]
    fn a_join_seats_by_the_table_and_shows_the_waiting_board() {
        let mut l = Lobby::new("court".into(), None, TURN_MS);
        let out = l.join(ADA, "ada".into()).unwrap();
        assert!(matches!(
            &out[0],
            (Recipient::One(ADA), S2C::Joined { id: 0, lobby }) if lobby == "court"
        ));
        assert!(matches!(out[1], (Recipient::Members, S2C::Roster { .. })));
        assert!(matches!(out[2], (Recipient::Members, S2C::State { .. })));
        assert!(matches!(
            out[3],
            (
                Recipient::One(ADA),
                S2C::Phase {
                    phase: Phase::Waiting,
                    winner: None,
                    end: None
                }
            )
        ));
        assert_eq!(out.len(), 4, "no CanStart for a lone creator");
        assert_eq!(l.creator, ADA);

        let out = l.join(BOB, "bob".into()).unwrap();
        let r = rosters(&out);
        assert_eq!(
            r,
            vec![(0, vec![(0, "ada".into(), 0), (1, "bob".into(), 2)])]
        );
        let b = boards(&out)[0];
        assert_eq!(b.pieces.len(), 64);
        assert!(b.seats[0].present && b.seats[2].present);
        assert!(b.seats[1].garrison && b.seats[3].garrison);
        assert_eq!(b.left_ms, TURN_MS);
        assert_eq!(l.phase, Phase::Waiting);
    }

    #[test]
    fn can_start_at_two_and_on_every_roster_change() {
        let mut l = Lobby::new("court".into(), None, TURN_MS);
        let out = l.join(ADA, "ada".into()).unwrap();
        assert!(can_starts(&out).is_empty());
        let out = l.join(BOB, "bob".into()).unwrap();
        assert_eq!(can_starts(&out), vec![(Recipient::One(ADA), 2)]);
        let out = l.join(CY, "cy".into()).unwrap();
        assert_eq!(can_starts(&out), vec![(Recipient::One(ADA), 3)]);
        let out = l.leave(CY);
        assert_eq!(can_starts(&out), vec![(Recipient::One(ADA), 2)]);
        let out = l.leave(BOB);
        assert!(can_starts(&out).is_empty(), "one player cannot start");
        // A fourth join is the "table is full" cue.
        l.join(BOB, "bob".into()).unwrap();
        l.join(CY, "cy".into()).unwrap();
        let out = l.join(44, "dee".into()).unwrap();
        assert_eq!(can_starts(&out), vec![(Recipient::One(ADA), 4)]);
        assert_eq!(l.join(55, "eve".into()), Err("lobby is full"));
    }

    #[test]
    fn seats_are_recomputed_on_leave() {
        let mut l = table(TURN_MS);
        l.join(CY, "cy".into()).unwrap();
        let seats: Vec<u8> = l.members.iter().map(|m| m.seat).collect();
        assert_eq!(seats, vec![0, 2, 1]);
        let out = l.leave(BOB);
        let seats: Vec<u8> = l.members.iter().map(|m| m.seat).collect();
        assert_eq!(seats, vec![0, 2], "the third joiner moved to the diagonal");
        assert_eq!(
            rosters(&out),
            vec![(0, vec![(0, "ada".into(), 0), (2, "cy".into(), 2)])]
        );
        let b = boards(&out)[0];
        assert!(b.seats[0].present && b.seats[2].present);
        assert!(!b.seats[1].present && b.seats[1].garrison);
        // The freed id is reused by the next joiner.
        l.join(BOB, "bob".into()).unwrap();
        assert_eq!(l.member(BOB).unwrap().id, 1);
    }

    #[test]
    fn the_creator_leaving_hands_over_to_the_longest_present() {
        let mut l = table(TURN_MS);
        l.join(CY, "cy".into()).unwrap();
        let out = l.leave(ADA);
        assert_eq!(l.creator, BOB);
        let r = rosters(&out);
        assert_eq!(r[0].0, 1, "Roster carries bob's id as creator");
        assert_eq!(r[0].1, vec![(1, "bob".into(), 0), (2, "cy".into(), 2)]);
        assert_eq!(can_starts(&out), vec![(Recipient::One(BOB), 2)]);
        assert_eq!(
            l.start(CY),
            Err("only the creator can start the game"),
            "the old guest is still a guest"
        );
        assert!(l.start(BOB).is_ok());
    }

    #[test]
    fn start_is_the_creators_with_two_seats_while_waiting() {
        let mut l = Lobby::new("court".into(), None, TURN_MS);
        l.join(ADA, "ada".into()).unwrap();
        assert_eq!(l.start(ADA), Err("the game needs at least two players"));
        l.join(BOB, "bob".into()).unwrap();
        assert_eq!(l.start(BOB), Err("only the creator can start the game"));
        let out = l.start(ADA).unwrap();
        assert_eq!(phases(&out), vec![(Phase::Playing, None, None)]);
        let b = boards(&out)[0];
        assert_eq!(b.pieces.len(), 64);
        assert_eq!((b.turn, b.seat, b.left_ms), (1, 0, TURN_MS));
        assert_eq!(l.start(ADA), Err("the game has already started"));
        assert_eq!(
            l.join(CY, "cy".into()),
            Err("that game has already started")
        );
    }

    #[test]
    fn clock_every_second_with_the_servers_left_ms() {
        let mut l = playing(TURN_MS);
        let mut seen = Vec::new();
        for _ in 0..350 {
            seen.extend(clocks(&tick_by(&mut l, 10)));
        }
        assert_eq!(
            seen,
            vec![(1, 0, 14_000), (1, 0, 13_000), (1, 0, 12_000)],
            "one Clock per second, no more"
        );
        // A shortened turn reports its own scale.
        let mut l = playing(1_000);
        let out = tick_by(&mut l, 1_000);
        assert_eq!(clocks(&out), vec![(1, 0, 0)]);
        assert!(boards(&out).is_empty(), "the grace is still running");
    }

    #[test]
    fn a_move_inside_the_grace_is_applied() {
        let mut l = playing(TURN_MS);
        let out = tick_by(&mut l, TURN_MS + 100);
        assert!(boards(&out).is_empty(), "no timeout at 15 100");
        let out = l.do_move(ADA, 1, (3, 0), (4, 0)).unwrap();
        let b = boards(&out)[0];
        assert_eq!(b.last.map(|a| a.kind), Some(ActionKind::Move));
        assert_eq!((b.turn, b.seat), (2, 2));
        assert_eq!(b.left_ms, TURN_MS, "the next turn's clock is fresh");
        assert_eq!(l.elapsed_ms, 0);
    }

    #[test]
    fn a_move_after_the_timeout_is_refused() {
        let mut l = playing(TURN_MS);
        let out = tick_by(&mut l, TURN_MS + GRACE_MS);
        let b = boards(&out)[0];
        assert_eq!(b.last.map(|a| a.kind), Some(ActionKind::Timeout));
        assert_eq!((b.turn, b.seat), (2, 2));
        assert_eq!(b.seats[0].timeouts, 1);
        // The mover whose turn passed is refused because the turn is no
        // longer theirs; the seat to move with the old stamp is refused as
        // stale (`apply_move` checks in that order).
        assert_eq!(l.do_move(ADA, 1, (3, 0), (4, 0)), Err("not your turn"));
        assert_eq!(
            l.do_move(BOB, 1, (6, 9), (5, 9)),
            Err("that move was for an earlier turn")
        );
        assert_eq!(
            l.do_move(BOB, 2, (3, 0), (4, 0)),
            Err("that is not your piece")
        );
        assert_eq!(l.state.turn, 2, "refusals leave the board alone");
        // Seat 2's own pawn moves.
        assert!(l.do_move(BOB, 2, (6, 9), (5, 9)).is_ok());
    }

    #[test]
    fn three_silent_turns_eliminate_and_the_survivor_wins() {
        let mut l = playing(1_000);
        let mut all = Outbox::new();
        // Seat 0, seat 2, seat 0, seat 2, then seat 0's third timeout.
        for i in 0..5 {
            let out = tick_by(&mut l, 1_300);
            assert_eq!(boards(&out).len(), 1, "tick {i}");
            all.extend(out);
        }
        assert_eq!(l.phase, Phase::Finished);
        assert_eq!(
            phases(&all),
            vec![(Phase::Finished, Some(2), Some(EndReason::LastKing))]
        );
        let last = boards(&all).last().unwrap().clone();
        assert_eq!(last.last.map(|a| a.eliminated), Some(Some(0)));
        assert!(!last.seats[0].alive);
        assert_eq!(l.do_move(BOB, last.turn, (6, 9), (5, 9)), Err("the game is over"));
    }

    #[test]
    fn finished_returns_to_waiting_after_results() {
        let mut l = playing(1_000);
        for _ in 0..5 {
            tick_by(&mut l, 1_300);
        }
        assert_eq!(l.phase, Phase::Finished);
        let out = tick_by(&mut l, RESULTS_MS - 1);
        assert!(out.is_empty());
        assert_eq!(l.phase, Phase::Finished);
        let out = tick_by(&mut l, 1);
        assert_eq!(l.phase, Phase::Waiting);
        assert_eq!(phases(&out), vec![(Phase::Waiting, None, None)]);
        assert_eq!(
            rosters(&out),
            vec![(0, vec![(0, "ada".into(), 0), (1, "bob".into(), 2)])]
        );
        let b = boards(&out)[0];
        assert_eq!(b.pieces.len(), 64, "a fresh Waiting board");
        assert!(b.seats[0].alive && b.seats[2].alive);
        assert_eq!(b.turn, 1);
        assert_eq!(can_starts(&out), vec![(Recipient::One(ADA), 2)]);
        assert!(l.start(ADA).is_ok(), "a new game needs a new Start");
    }

    #[test]
    fn set_formation_in_waiting_only() {
        let mut l = table(TURN_MS);
        let swapped = Formation {
            legend: [Kind::Joker, Kind::Queen, Kind::Hero, Kind::King],
            epic: Formation::DEFAULT.epic,
        };
        let out = l.set_formation(BOB, swapped).unwrap();
        let b = boards(&out)[0];
        // Bob is seat 2 (NE): its (0,0) is (9,9), its (1,1) is (8,8).
        let at = |x, y| b.pieces.iter().find(|p| p.x == x && p.y == y).unwrap().kind;
        assert_eq!(at(9, 9), Kind::Joker);
        assert_eq!(at(8, 8), Kind::King);
        assert_eq!(at(0, 0), Kind::King, "ada's corner is untouched");
        let bad = Formation {
            legend: [Kind::King, Kind::King, Kind::Hero, Kind::Joker],
            epic: Formation::DEFAULT.epic,
        };
        assert_eq!(
            l.set_formation(BOB, bad),
            Err("the corner tiles must hold the king, queen, hero and joker once each")
        );
        assert_eq!(
            l.set_formation(CY, swapped),
            Err("you are not at this table")
        );
        let out = l.start(ADA).unwrap();
        let b = boards(&out)[0];
        let at = |x, y| b.pieces.iter().find(|p| p.x == x && p.y == y).unwrap().kind;
        assert_eq!(at(9, 9), Kind::Joker, "the swap survives Start");
        assert_eq!(
            l.set_formation(BOB, Formation::DEFAULT),
            Err("formations are frozen once the game has started")
        );
    }

    #[test]
    fn a_leave_while_playing_eliminates_the_seat() {
        let mut l = playing(TURN_MS);
        // The non-mover drops: elimination and an immediate end check.
        let out = l.leave(BOB);
        assert_eq!(l.phase, Phase::Finished);
        assert_eq!(
            phases(&out),
            vec![(Phase::Finished, Some(0), Some(EndReason::LastKing))]
        );
        let r = rosters(&out);
        assert_eq!(r[0].1.len(), 1);
        let b = boards(&out)[0];
        assert!(!b.seats[2].present && !b.seats[2].alive);
        assert_eq!(
            b.pieces.iter().filter(|p| p.owner == 2).count(),
            0,
            "an eliminated seat's pieces leave the board"
        );

        // With three, the mover dropping ends its turn and the game goes on.
        let mut l = table(TURN_MS);
        l.join(CY, "cy".into()).unwrap();
        l.start(ADA).unwrap();
        tick_by(&mut l, 5_000);
        let out = l.leave(ADA);
        assert_eq!(l.phase, Phase::Playing);
        assert_eq!(l.creator, BOB, "the creator's departure hands over");
        let b = boards(&out)[0];
        assert_eq!(b.last.map(|a| (a.kind, a.eliminated)), Some((ActionKind::Pass, Some(0))));
        assert_eq!(b.seat, 1, "seat 1 (cy) is next after seat 0");
        assert_eq!(b.left_ms, TURN_MS, "the next turn's clock is fresh");
        assert_eq!(l.elapsed_ms, 0);
    }

    #[test]
    fn welcome_counts_humans_in_games_and_open_lobbies() {
        let mut lobbies = HashMap::new();
        lobbies.insert("open".to_string(), table(TURN_MS));
        lobbies.insert("busy".to_string(), playing(TURN_MS));
        let mut three = table(TURN_MS);
        three.join(CY, "cy".into()).unwrap();
        three.start(ADA).unwrap();
        lobbies.insert("busier".to_string(), three);
        assert_eq!(players_in_games(&lobbies), 5);
        assert_eq!(open_lobbies(&lobbies), 1);
    }

    #[test]
    fn a_mismatched_version_cannot_enter_a_lobby() {
        let mut conns = HashMap::new();
        let (tx, rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        conns.insert(
            1u64,
            Conn {
                tx,
                peer: "t".into(),
                handle: Some("h".into()),
                proto: proto::PROTO_VERSION + 1,
                lobby: None,
                last_seen: Instant::now(),
                msgs_this_window: 0,
            },
        );
        assert!(!version_ok(1, &conns), "a mismatched peer was let in");
        let Ok(Message::Text(t)) = rx.try_recv() else {
            panic!("no rejection sent")
        };
        let msg: S2C = serde_json::from_str(&t).unwrap();
        match msg {
            S2C::Rejected { reason } => {
                assert!(reason.contains("kings protocol"), "{reason}");
                assert!(
                    reason.contains(&format!("v{}", proto::PROTO_VERSION + 1)),
                    "{reason}"
                );
                assert!(
                    reason.contains(&format!("v{}", proto::PROTO_VERSION)),
                    "{reason}"
                );
            }
            other => panic!("wrong message: {other:?}"),
        }
    }

    /// An empty lobby must not linger and keep its name reserved, and a
    /// message before Hello closes the connection.
    #[test]
    fn hub_paths_that_touch_the_lobby_map() {
        let mut conns = HashMap::new();
        let mut lobbies = HashMap::new();
        let cfg = ServerConfig::default();
        let (tx, rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        conns.insert(
            7u64,
            Conn {
                tx,
                peer: "t".into(),
                handle: None,
                proto: 0,
                lobby: None,
                last_seen: Instant::now(),
                msgs_this_window: 0,
            },
        );
        handle_msg(7, C2S::ListLobbies, &mut conns, &mut lobbies, &cfg);
        assert!(!conns.contains_key(&7), "a message before Hello must drop");
        assert!(
            matches!(rx.try_recv(), Err(mpsc::TryRecvError::Disconnected)),
            "the queue closes, which is what closes the socket"
        );

        let (tx, _rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        conns.insert(
            8u64,
            Conn {
                tx,
                peer: "t".into(),
                handle: Some("h".into()),
                proto: proto::PROTO_VERSION,
                lobby: None,
                last_seen: Instant::now(),
                msgs_this_window: 0,
            },
        );
        handle_msg(
            8,
            C2S::CreateLobby {
                name: "court".into(),
                password: None,
            },
            &mut conns,
            &mut lobbies,
            &cfg,
        );
        assert!(lobbies.contains_key("court"));
        assert_eq!(conns[&8].lobby.as_deref(), Some("court"));
        handle_msg(8, C2S::LeaveLobby, &mut conns, &mut lobbies, &cfg);
        assert!(lobbies.is_empty(), "an empty lobby held its name");
        assert_eq!(conns[&8].lobby, None);
    }
}
