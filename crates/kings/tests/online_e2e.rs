//! The whole loop, natively: a real `kings-server`, the real `Net` transport
//! and the real `Online` client state, with the `Selection` machine making
//! the first move.
//!
//! The server's own e2e test proves the wire works. This one proves the
//! *client* works against it: that `Online` follows a lobby from creation
//! through Start, a move, two refusals and a timeout, and that the two
//! clients' boards agree at every step. It is the list of section 4.9 of
//! `docs/kings-design.md` for the `kings` crate.

use std::collections::VecDeque;
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use kings::game::Screen;
use kings::net::{Net, Status};
use kings::online::Online;
use kings::ui::{Ui, UiOut};
use kings_core::board::Tile;
use kings_core::proto::{self, ActionKind, BoardState, C2S, Phase, S2C};
use tungstenite::Message;

/// A turn length short enough for a silent turn to pass inside a test, long
/// enough for a loopback move to land well inside it.
const TURN_MS: u32 = 1000;

/// Surface the server's own warnings (dropped sends, rate-limited messages)
/// in test output. Without this a lost control message is invisible from both
/// ends and looks like an unexplained timeout.
fn init_logs() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        drop(
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "warn".into()),
                )
                .with_test_writer()
                .try_init(),
        );
    });
}

fn start_server(turn_ms: u32) -> u16 {
    init_logs();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        drop(kings_server::run(
            listener,
            kings_server::ServerConfig {
                turn_ms,
                max_lobbies: 8,
                host: String::new(),
            },
        ));
    });
    // Give the accept loop a moment to come up.
    thread::sleep(Duration::from_millis(150));
    port
}

struct Peer {
    net: Net,
    game: Online,
    inbox: VecDeque<S2C>,
}

impl Peer {
    fn connect(port: u16, handle: &str) -> Self {
        let net = Net::connect(&format!("ws://127.0.0.1:{port}"), handle).expect("connect");
        let t0 = Instant::now();
        while net.status() == Status::Connecting && t0.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(net.status(), Status::Open, "socket never opened");
        let mut peer = Self {
            net,
            game: Online::new(),
            inbox: VecDeque::default(),
        };
        // Wait for Welcome before returning. Hello is the socket's own first
        // message and Welcome acknowledges it; anything version-gated sent
        // before then races the server's handling of Hello.
        assert!(
            peer.wait_for(Duration::from_secs(5), |g| g.welcomed),
            "{handle}: no Welcome"
        );
        peer
    }

    fn pump(&mut self) {
        self.net.drain(&mut self.inbox);
        while let Some(m) = self.inbox.pop_front() {
            self.game.apply(m);
        }
    }

    /// Pump until `f` is satisfied or we give up.
    fn wait_for(&mut self, dur: Duration, mut f: impl FnMut(&Online) -> bool) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < dur {
            self.pump();
            if f(&self.game) {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }

    /// Pump until a notice appears, and return it.
    fn wait_for_notice(&mut self, who: &str) -> String {
        assert!(
            self.wait_for(Duration::from_secs(3), |g| g.notice.is_some()),
            "{who} got no notice.\n  {}",
            self.dump(who)
        );
        self.game.notice.clone().unwrap()
    }

    /// Everything that distinguishes the ways this can go wrong: a dead
    /// socket, a refusal we never looked at, or a message that simply never
    /// came. A bare `assert!` cannot tell them apart.
    fn dump(&self, who: &str) -> String {
        format!(
            "{who}: socket={:?} screen={:?} id={:?} seat={:?} phase={:?} notice={:?} roster={} turn={:?}",
            self.net.status(),
            self.game.screen,
            self.game.my_id,
            self.game.my_seat,
            self.game.phase,
            self.game.notice,
            self.game.roster.len(),
            self.game.board.as_ref().map(|b| b.turn),
        )
    }

    const fn board(&self) -> &BoardState {
        self.game.board.as_ref().expect("no board yet")
    }
}

/// The parts of a board two clients must agree on. `left_ms` is excluded:
/// each client resyncs it from `Clock` at its own pumping moments.
fn agreed(b: &BoardState) -> BoardState {
    BoardState {
        left_ms: 0,
        ..b.clone()
    }
}

type Raw = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// A page's lobby browser: raw tungstenite, `hello` with `proto: 0`, then
/// `list_lobbies`. Reads treat a timed-out socket as "nothing yet", on
/// Windows in any of its costumes (`kings_core::proto::is_transient_read`).
fn browse(port: u16) -> Vec<proto::LobbyInfo> {
    let (mut ws, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).expect("connect");
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_ref() {
        drop(s.set_read_timeout(Some(Duration::from_millis(50))));
    }
    let send = |ws: &mut Raw, msg: &C2S| {
        ws.send(Message::text(serde_json::to_string(msg).unwrap()))
            .expect("send");
    };
    send(
        &mut ws,
        &C2S::Hello {
            proto: 0,
            handle: "browser".into(),
        },
    );
    send(&mut ws, &C2S::ListLobbies);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(3) {
        match ws.read() {
            Ok(Message::Text(t)) => {
                if let Ok(S2C::Lobbies { lobbies }) = serde_json::from_str::<S2C>(&t) {
                    return lobbies;
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(e) => panic!("browser socket died: {e}"),
        }
    }
    panic!("the browser never got a lobby list");
}

// ---- the phases of the one scenario ---------------------------------------

/// The creator makes the table and sees it (Joined, Roster, a full Waiting
/// board, Phase Waiting); the guest joins and sits diagonally; both see a
/// 64-piece Waiting board and agree on it.
fn create_and_join(a: &mut Peer, b: &mut Peer) {
    a.net.send(&C2S::CreateLobby {
        name: "court".into(),
        password: None,
    });
    assert!(
        a.wait_for(Duration::from_secs(5), |g| g.screen == Screen::Lobby
            && g.my_seat == Some(0)
            && g.board.as_ref().is_some_and(|b| b.pieces.len() == 64)),
        "ada never saw the table she created.\n  {}",
        a.dump("ada")
    );
    assert!(a.game.is_creator, "{}", a.dump("ada"));
    assert!(!a.game.can_start, "alone, she cannot start");
    assert_eq!(a.game.phase, Phase::Waiting);
    assert!(a.game.state.is_some(), "the Waiting board decoded");

    b.net.send(&C2S::JoinLobby {
        name: "court".into(),
        password: None,
    });
    assert!(
        b.wait_for(Duration::from_secs(5), |g| g.screen == Screen::Lobby
            && g.my_seat == Some(2)
            && g.board.as_ref().is_some_and(|b| b.pieces.len() == 64)),
        "bob never joined.\n  {}\n  {}",
        b.dump("bob"),
        a.dump("ada")
    );
    assert!(!b.game.is_creator);
    assert_eq!(b.game.phase, Phase::Waiting);
    assert!(
        a.wait_for(Duration::from_secs(5), |g| g.roster.len() == 2
            && g.can_start),
        "ada never learned bob had joined, or was never told she may start.\n  {}",
        a.dump("ada")
    );
    assert_eq!(a.game.roster.len(), 2);
    assert_eq!(b.game.roster.len(), 2);
    assert_eq!(a.board().pieces.len(), 64);
    assert_eq!(
        agreed(a.board()),
        agreed(b.board()),
        "the Waiting boards agree"
    );
}

/// The guest's Start is refused with a reason the page can show; the
/// creator's Start with two seats gives Playing on both, seat 0 to move on
/// turn 1, the two empty corners garrisons.
fn start_with_two_seats(a: &mut Peer, b: &mut Peer) {
    b.net.send(&C2S::Start);
    let reason = b.wait_for_notice("bob");
    assert!(reason.contains("creator"), "{reason}");
    assert_eq!(b.game.phase, Phase::Waiting, "still waiting");

    a.net.send(&C2S::Start);
    for (peer, who) in [(&mut *a, "ada"), (&mut *b, "bob")] {
        assert!(
            peer.wait_for(Duration::from_secs(5), |g| g.phase == Phase::Playing
                && g.board.as_ref().is_some_and(|b| b.turn == 1 && b.seat == 0)),
            "{who} never saw the game start.\n  {}",
            peer.dump(who)
        );
        let board = peer.board();
        assert_eq!(board.pieces.len(), 64);
        assert!(board.seats[0].alive && board.seats[2].alive);
        assert!(board.seats[1].garrison && board.seats[3].garrison);
    }
    assert!(a.game.my_turn(), "{}", a.dump("ada"));
    assert!(!b.game.my_turn(), "{}", b.dump("bob"));
}

/// Seat 0's pawn (3,0) -> (4,0), through the selection machine: the click
/// pair emits a Move stamped with the current turn, and both boards agree
/// on the result.
fn first_move(a: &mut Peer, b: &mut Peer) {
    let mut ui = Ui::default();
    let state = a.game.state.as_ref().expect("decoded board");
    assert_eq!(
        ui.click(state, a.game.my_seat, a.game.phase, Tile::at(3, 0)),
        None
    );
    let out = ui.click(state, a.game.my_seat, a.game.phase, Tile::at(4, 0));
    let Some(UiOut::Move { turn, from, to }) = out else {
        panic!("the click pair did not emit a move: {out:?}");
    };
    assert_eq!((turn, from, to), (1, Tile::at(3, 0), Tile::at(4, 0)));
    a.net.send(&C2S::Move {
        turn,
        fx: from.x,
        fy: from.y,
        tx: to.x,
        ty: to.y,
    });
    a.game.pending = true;
    for (peer, who) in [(&mut *a, "ada"), (&mut *b, "bob")] {
        assert!(
            peer.wait_for(Duration::from_secs(3), |g| g
                .board
                .as_ref()
                .is_some_and(|b| b.turn == 2)),
            "{who} never saw the move.\n  {}",
            peer.dump(who)
        );
        let board = peer.board();
        assert_eq!(board.seat, 2, "{who}: seat 2 is next");
        assert!(
            board
                .pieces
                .iter()
                .any(|p| p.x == 4 && p.y == 0 && p.owner == 0),
            "{who}: the pawn is on (4,0)"
        );
        assert!(
            !board.pieces.iter().any(|p| p.x == 3 && p.y == 0),
            "{who}: (3,0) is empty"
        );
        let last = board.last.expect("the board narrates the move");
        assert_eq!(last.kind, ActionKind::Move);
        assert_eq!(
            (last.seat, last.fx, last.fy, last.tx, last.ty),
            (0, 3, 0, 4, 0)
        );
        // The decoded state follows the snapshot.
        let state = peer.game.state.as_ref().expect("decoded");
        assert_eq!(state.turn, 2);
        assert_eq!(state.to_move, 2);
        assert_eq!(state.piece(Tile::at(4, 0)).map(|p| p.owner), Some(0));
    }
    assert_eq!(agreed(a.board()), agreed(b.board()), "both boards agree");
    assert!(!a.game.pending, "the State echo clears pending");
    assert!(b.game.my_turn());
    assert!(!a.game.my_turn());
}

/// Seat 0 moving again: "not your turn". Seat 2 aiming a pawn at the middle
/// of the board: "cannot move there", and the board exactly as it was.
fn two_refusals(a: &mut Peer, b: &mut Peer) {
    a.game.notice = None;
    a.net.send(&C2S::Move {
        turn: 2,
        fx: 4,
        fy: 0,
        tx: 5,
        ty: 0,
    });
    assert_eq!(a.wait_for_notice("ada"), "not your turn");

    let before = agreed(b.board());
    b.game.notice = None;
    b.net.send(&C2S::Move {
        turn: 2,
        fx: 6,
        fy: 9,
        tx: 4,
        ty: 4,
    });
    let reason = b.wait_for_notice("bob");
    assert!(reason.starts_with("cannot move there"), "{reason}");
    assert_eq!(agreed(b.board()), before, "a refused move changes nothing");
    assert_eq!(b.board().turn, 2, "the turn did not end");
}

/// Nobody moves: with a one-second turn the server passes for seat 2 and
/// narrates it as a timeout; seat 0 is to move again on turn 3.
fn silent_turn(a: &mut Peer, b: &mut Peer) {
    for (peer, who) in [(&mut *a, "ada"), (&mut *b, "bob")] {
        assert!(
            peer.wait_for(Duration::from_secs(5), |g| g.board.as_ref().is_some_and(
                |b| b.last.is_some_and(|l| l.kind == ActionKind::Timeout)
            )),
            "{who} never saw the silent turn pass.\n  {}",
            peer.dump(who)
        );
        let board = peer.board();
        let last = board.last.unwrap();
        assert_eq!(last.seat, 2, "{who}: seat 2 timed out");
        assert_eq!(last.eliminated, None);
        assert_eq!(board.seats[2].timeouts, 1);
        assert_eq!(board.turn, 3);
        assert_eq!(board.seat, 0);
        assert_eq!(board.pieces.len(), 64, "{who}: a timeout moves nothing");
    }
    assert_eq!(agreed(a.board()), agreed(b.board()));
    assert!(a.game.left_ms <= TURN_MS);
}

/// The list of section 4.9 in one sitting, because every step depends on
/// the one before it: create, join, the guest's Start refused, the creator's
/// Start, the first move agreed on both boards, an out-of-turn move and an
/// illegal target refused with their reasons, a silent turn passing as a
/// timeout, and a browser peer listing the table as playing.
#[test]
fn two_clients_create_join_start_move_and_time_out() {
    let port = start_server(TURN_MS);
    let mut a = Peer::connect(port, "ada");
    let mut b = Peer::connect(port, "bob");

    create_and_join(&mut a, &mut b);
    start_with_two_seats(&mut a, &mut b);
    first_move(&mut a, &mut b);
    two_refusals(&mut a, &mut b);
    silent_turn(&mut a, &mut b);

    // A browser-style peer sees the table, marked playing. Last, because a
    // one-second clock keeps running while it looks.
    let lobbies = browse(port);
    let court = lobbies
        .iter()
        .find(|l| l.name == "court")
        .unwrap_or_else(|| panic!("the lobby is not listed: {lobbies:?}"));
    assert!(court.playing, "{court:?}");
    assert_eq!(court.players, 2);
    assert_eq!(court.host, "ada");
    assert!(!court.has_password);
}

/// Hello and the keepalive are the socket's job, not the game loop's. A web
/// tab that gets no animation frames never calls `update`, so a `Net` that
/// nobody drives has to greet the server on its own and keep the seat alive
/// past `CLIENT_TIMEOUT_SECS`. There is no timeout knob on `ServerConfig`
/// and this test does not add one; two Pongs prove the same thing, because
/// the server answers Ping only after Hello and the client only pings from
/// its reader thread.
#[test]
fn a_bare_net_greets_and_pings_without_update() {
    let port = start_server(proto::TURN_MS);
    let mut net = Net::connect(&format!("ws://127.0.0.1:{port}"), "quiet").expect("connect");
    let mut inbox = VecDeque::new();
    let t0 = Instant::now();
    let mut welcomed_at = None;
    let mut pongs = Vec::new();
    // Hello at open, a Ping every CLIENT_PING_SECS: the second Pong is due
    // at about 2 x CLIENT_PING_SECS, so the window is that plus slack.
    let window = proto::CLIENT_PING_SECS * 2 + 2;
    while t0.elapsed() < Duration::from_secs(window) && pongs.len() < 2 {
        net.drain(&mut inbox);
        while let Some(m) = inbox.pop_front() {
            match m {
                S2C::Welcome { .. } => welcomed_at = Some(t0.elapsed()),
                S2C::Pong { nonce } => pongs.push((nonce, t0.elapsed())),
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let welcomed_at = welcomed_at.unwrap_or_else(|| {
        panic!(
            "no Welcome without update: the socket did not say hello (status {:?})",
            net.status()
        )
    });
    assert!(
        welcomed_at < Duration::from_secs(3),
        "Welcome took {welcomed_at:?}: Hello waited for something"
    );
    assert!(
        pongs.len() >= 2,
        "{} Pong(s) in {:?}: the keepalive is not leaving the reader thread (status {:?})",
        pongs.len(),
        t0.elapsed(),
        net.status()
    );
    assert_eq!(pongs[0].0, 1, "nonces count from one");
    assert_eq!(pongs[1].0, 2, "and increment per ping");
    assert_eq!(
        net.status(),
        Status::Open,
        "still seated after {:?}",
        t0.elapsed()
    );
    // The game never sends Hello: the server closes on a second one, and
    // this connection must outlive an attempt.
    net.send(&C2S::Hello {
        proto: proto::PROTO_VERSION,
        handle: "quiet".into(),
    });
    net.send(&C2S::ListLobbies);
    let t1 = Instant::now();
    let mut listed = false;
    while t1.elapsed() < Duration::from_secs(3) && !listed {
        net.drain(&mut inbox);
        listed = inbox.drain(..).any(|m| matches!(m, S2C::Lobbies { .. }));
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        listed,
        "a Hello handed to send() reached the server and closed the connection (status {:?})",
        net.status()
    );
    eprintln!(
        "bare net: Welcome at {welcomed_at:?}, Pongs at {:?} and {:?}",
        pongs[0].1, pongs[1].1
    );
}

/// A refusal has to reach the client as something it can show, not a silent
/// failure to join.
#[test]
fn a_refused_join_surfaces_a_reason() {
    let port = start_server(proto::TURN_MS);
    let mut host = Peer::connect(port, "host");
    host.net.send(&C2S::CreateLobby {
        name: "locked".into(),
        password: Some("secret".into()),
    });
    assert!(
        host.wait_for(Duration::from_secs(3), |g| g.screen == Screen::Lobby),
        "{}",
        host.dump("host")
    );

    let mut guest = Peer::connect(port, "guest");
    guest.net.send(&C2S::JoinLobby {
        name: "locked".into(),
        password: Some("nope".into()),
    });
    let reason = guest.wait_for_notice("guest");
    assert!(reason.contains("password"), "{reason}");
    assert_eq!(
        guest.game.screen,
        Screen::Browsing,
        "a refused client thinks it is in a lobby"
    );
}
