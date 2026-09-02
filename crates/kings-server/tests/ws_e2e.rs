//! End-to-end: start a real server, connect real WebSocket clients, and play
//! the first move of a game.
//!
//! The unit tests drive `Lobby` with synthetic time. This one does not: it
//! speaks the wire, which is the only way to prove the thing is actually
//! *joinable*, and it is the list of section 4.9 of `docs/kings-design.md`.

use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use kings_core::proto::{self, ActionKind, BoardState, C2S, Phase, S2C};
use tungstenite::{Message, WebSocket, connect};

type Client = WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// Non-blocking-ish reads. `MaybeTlsStream` does not forward socket options,
/// so reach through to the plain stream; these tests never use TLS.
fn set_read_timeout(ws: &Client, d: Duration) {
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_ref() {
        drop(s.set_read_timeout(Some(d)));
    }
}

fn start_server(turn_ms: u32) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        drop(kings_server::run(
            listener,
            kings_server::ServerConfig {
                turn_ms,
                max_lobbies: 8,
                host: "test".into(),
            },
        ));
    });
    // Give the accept loop a moment to come up.
    thread::sleep(Duration::from_millis(150));
    port
}

fn raw_client(port: u16) -> Client {
    let (ws, _) = connect(format!("ws://127.0.0.1:{port}")).expect("connect");
    set_read_timeout(&ws, Duration::from_millis(50));
    ws
}

/// Connect, Hello, and wait for Welcome.
fn client(port: u16, handle: &str) -> Client {
    let mut ws = raw_client(port);
    send(
        &mut ws,
        &C2S::Hello {
            proto: proto::PROTO_VERSION,
            handle: handle.into(),
        },
    );
    assert!(
        pump(&mut ws, Duration::from_secs(2), |m| matches!(
            m,
            S2C::Welcome { .. }
        )),
        "no welcome for {handle}"
    );
    ws
}

fn send(ws: &mut Client, msg: &C2S) {
    ws.send(Message::text(serde_json::to_string(msg).unwrap()))
        .expect("send");
}

/// Pump messages for `dur`, handing each to `f`. Returns when `f` says stop.
/// A timed-out read is "nothing yet", on Windows in any of its three
/// costumes (`kings_core::proto::is_transient_read`).
fn pump(ws: &mut Client, dur: Duration, mut f: impl FnMut(S2C) -> bool) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < dur {
        match ws.read() {
            Ok(Message::Text(t)) => {
                if let Ok(m) = serde_json::from_str::<S2C>(&t)
                    && f(m)
                {
                    return true;
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(_) => break,
        }
    }
    false
}

/// Wait for a `State` that `f` accepts and return it.
fn next_board(ws: &mut Client, dur: Duration, mut f: impl FnMut(&BoardState) -> bool) -> Option<BoardState> {
    let mut found = None;
    pump(ws, dur, |m| {
        if let S2C::State { board } = m
            && f(&board)
        {
            found = Some(board);
            return true;
        }
        false
    });
    found
}

/// Wait for a `Rejected` and return its reason.
fn next_rejection(ws: &mut Client, dur: Duration) -> Option<String> {
    let mut found = None;
    pump(ws, dur, |m| {
        if let S2C::Rejected { reason } = m {
            found = Some(reason);
            return true;
        }
        false
    });
    found
}

fn create(ws: &mut Client, name: &str) -> u8 {
    send(
        ws,
        &C2S::CreateLobby {
            name: name.into(),
            password: None,
        },
    );
    let mut id = None;
    assert!(
        pump(ws, Duration::from_secs(2), |m| {
            if let S2C::Joined { lobby, id: got } = m {
                assert_eq!(lobby, name);
                id = Some(got);
                return true;
            }
            false
        }),
        "creator never got Joined"
    );
    id.unwrap()
}

fn join(ws: &mut Client, name: &str) -> u8 {
    send(
        ws,
        &C2S::JoinLobby {
            name: name.into(),
            password: None,
        },
    );
    let mut id = None;
    assert!(
        pump(ws, Duration::from_secs(2), |m| {
            if let S2C::Joined { id: got, .. } = m {
                id = Some(got);
                return true;
            }
            false
        }),
        "joiner never got Joined"
    );
    id.unwrap()
}

/// The list of section 4.9, in one sitting: create, list, join, the guest's
/// Start refused, the creator's Start, the first move seen by the other
/// side, an out-of-turn move and a stale one refused, the listing marking
/// the lobby as playing, and a third player refused.
#[test]
fn two_players_create_join_start_and_move() {
    let port = start_server(proto::TURN_MS);
    let mut a = client(port, "alice");
    let mut b = client(port, "bob");

    let a_id = create(&mut a, "court");
    // The creator's own view of the empty table: Roster with the creator at
    // seat 0, a full Waiting board, Phase Waiting, no CanStart yet.
    assert!(
        pump(&mut a, Duration::from_secs(2), |m| {
            matches!(m, S2C::Roster { creator, roster } if creator == a_id && roster.len() == 1 && roster[0].seat == 0)
        }),
        "the creator never saw a roster"
    );
    let waiting = next_board(&mut a, Duration::from_secs(2), |_| true).expect("waiting board");
    assert_eq!(waiting.pieces.len(), 64);
    assert!(
        pump(&mut a, Duration::from_secs(2), |m| matches!(
            m,
            S2C::Phase {
                phase: Phase::Waiting,
                ..
            }
        )),
        "no Phase after the Waiting board"
    );

    // It shows up in the listing, not yet playing.
    send(&mut b, &C2S::ListLobbies);
    assert!(
        pump(&mut b, Duration::from_secs(2), |m| {
            matches!(m, S2C::Lobbies { lobbies }
                if lobbies.iter().any(|l| l.name == "court" && !l.playing && l.players == 1 && l.host == "alice"))
        }),
        "the new lobby was not listed as open"
    );

    // Bob joins and sits diagonally.
    let b_id = join(&mut b, "court");
    assert_ne!(a_id, b_id);
    assert!(
        pump(&mut b, Duration::from_secs(2), |m| {
            if let S2C::Roster { creator, roster } = m {
                assert_eq!(creator, a_id);
                assert_eq!(roster.len(), 2, "roster should already hold both");
                let seat = |id: u8| roster.iter().find(|p| p.id == id).map(|p| p.seat);
                assert_eq!(seat(a_id), Some(0));
                assert_eq!(seat(b_id), Some(2));
                return true;
            }
            false
        }),
        "joiner never got the roster"
    );
    // Alice is told, and may start.
    assert!(
        pump(&mut a, Duration::from_secs(2), |m| matches!(
            m,
            S2C::CanStart { players: 2 }
        )),
        "the creator was not told the table can start"
    );

    // The guest may not start.
    send(&mut b, &C2S::Start);
    let reason = next_rejection(&mut b, Duration::from_secs(2)).expect("guest Start refused");
    assert!(reason.contains("creator"), "{reason}");

    // The creator starts: Phase Playing, then the board with two garrisons.
    send(&mut a, &C2S::Start);
    for (ws, who) in [(&mut a, "alice"), (&mut b, "bob")] {
        assert!(
            pump(ws, Duration::from_secs(2), |m| matches!(
                m,
                S2C::Phase {
                    phase: Phase::Playing,
                    ..
                }
            )),
            "{who} never saw Playing"
        );
        let board = next_board(ws, Duration::from_secs(2), |_| true)
            .unwrap_or_else(|| panic!("{who} never saw the starting board"));
        assert_eq!(board.pieces.len(), 64);
        assert_eq!((board.turn, board.seat), (1, 0));
        assert!(board.seats[0].alive && board.seats[2].alive);
        assert!(board.seats[1].garrison && board.seats[3].garrison);
        assert_eq!(board.left_ms, proto::TURN_MS);
    }

    // Out of turn: seat 2 tries to move first.
    send(
        &mut b,
        &C2S::Move {
            turn: 1,
            fx: 6,
            fy: 9,
            tx: 5,
            ty: 9,
        },
    );
    let reason = next_rejection(&mut b, Duration::from_secs(2)).expect("out-of-turn refused");
    assert_eq!(reason, "not your turn");

    // Stale stamp: seat 0 with a turn that is not the current one.
    send(
        &mut a,
        &C2S::Move {
            turn: 7,
            fx: 3,
            fy: 0,
            tx: 4,
            ty: 0,
        },
    );
    let reason = next_rejection(&mut a, Duration::from_secs(2)).expect("stale turn refused");
    assert!(reason.contains("earlier turn"), "{reason}");

    // The real first move: seat 0's pawn (3,0) -> (4,0), seen by bob.
    send(
        &mut a,
        &C2S::Move {
            turn: 1,
            fx: 3,
            fy: 0,
            tx: 4,
            ty: 0,
        },
    );
    let board = next_board(&mut b, Duration::from_secs(2), |b| b.turn == 2)
        .expect("bob never saw the move");
    let last = board.last.expect("the board narrates the move");
    assert_eq!(last.kind, ActionKind::Move);
    assert_eq!((last.seat, last.fx, last.fy, last.tx, last.ty), (0, 3, 0, 4, 0));
    assert_eq!(board.seat, 2, "seat 2 is next");
    assert!(board.pieces.iter().any(|p| p.x == 4 && p.y == 0 && p.owner == 0));
    assert!(!board.pieces.iter().any(|p| p.x == 3 && p.y == 0));

    // The listing now says playing, and a third player is refused.
    let mut c = client(port, "carol");
    send(&mut c, &C2S::ListLobbies);
    assert!(
        pump(&mut c, Duration::from_secs(2), |m| {
            matches!(m, S2C::Lobbies { lobbies }
                if lobbies.iter().any(|l| l.name == "court" && l.playing && l.players == 2))
        }),
        "the running game was not listed as playing"
    );
    send(
        &mut c,
        &C2S::JoinLobby {
            name: "court".into(),
            password: None,
        },
    );
    let reason = next_rejection(&mut c, Duration::from_secs(2)).expect("join refused");
    assert!(reason.contains("started"), "{reason}");
}

/// With a one-second turn, a silent seat is passed by the server with a
/// timeout mark, and the other side sees it as a `State`.
#[test]
fn a_silent_turn_passes_with_a_timeout() {
    let port = start_server(1_000);
    let mut a = client(port, "alice");
    let mut b = client(port, "bob");
    create(&mut a, "quiet");
    join(&mut b, "quiet");
    send(&mut a, &C2S::Start);
    assert!(
        pump(&mut b, Duration::from_secs(2), |m| matches!(
            m,
            S2C::Phase {
                phase: Phase::Playing,
                ..
            }
        )),
        "bob never saw Playing"
    );
    // Nobody moves. A Clock arrives at one second, then the timeout pass.
    assert!(
        pump(&mut b, Duration::from_secs(3), |m| matches!(
            m,
            S2C::Clock { turn: 1, seat: 0, .. }
        )),
        "no Clock during the turn"
    );
    let board = next_board(&mut b, Duration::from_secs(3), |b| b.turn == 2)
        .expect("the silent turn never passed");
    let last = board.last.expect("the pass is narrated");
    assert_eq!(last.kind, ActionKind::Timeout);
    assert_eq!(last.seat, 0);
    assert_eq!(board.seats[0].timeouts, 1);
    assert_eq!(board.seat, 2);
    assert_eq!(board.pieces.len(), 64, "a timeout moves nothing");
}

/// Anything before Hello is a protocol violation and the server closes the
/// connection.
#[test]
fn a_message_before_hello_closes_the_connection() {
    let port = start_server(proto::TURN_MS);
    let mut ws = raw_client(port);
    send(&mut ws, &C2S::ListLobbies);
    let t0 = Instant::now();
    let mut closed = false;
    while t0.elapsed() < Duration::from_secs(3) {
        match ws.read() {
            Ok(Message::Close(_)) => {
                closed = true;
                break;
            }
            Ok(Message::Text(t)) => panic!("the server answered a pre-Hello message: {t}"),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(_) => {
                closed = true;
                break;
            }
        }
    }
    assert!(closed, "the connection stayed open after a pre-Hello message");
}

/// The gate is exact equality, and the refusal has to say what each side
/// speaks: "cannot join" with no version is the message that wastes an hour.
#[test]
fn a_version_mismatch_is_refused_with_both_versions() {
    let port = start_server(proto::TURN_MS);
    let mut ws = raw_client(port);
    send(
        &mut ws,
        &C2S::Hello {
            proto: proto::PROTO_VERSION + 7,
            handle: "stale".into(),
        },
    );
    assert!(pump(&mut ws, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { proto, host, .. } if proto == proto::PROTO_VERSION && host == "test")
    }));
    send(
        &mut ws,
        &C2S::CreateLobby {
            name: "nope".into(),
            password: None,
        },
    );
    let reason = next_rejection(&mut ws, Duration::from_secs(2))
        .expect("a stale client was allowed to create a lobby");
    assert!(
        reason.contains(&format!("v{}", proto::PROTO_VERSION + 7)),
        "{reason}"
    );
    assert!(
        reason.contains(&format!("v{}", proto::PROTO_VERSION)),
        "{reason}"
    );
}

/// Listing must work at any version: the hub's lobby browser has no game
/// loaded and must not be locked out by a protocol bump.
#[test]
fn listing_is_ungated() {
    let port = start_server(proto::TURN_MS);
    let mut host = client(port, "host");
    create(&mut host, "open");

    let mut browser = raw_client(port);
    send(
        &mut browser,
        &C2S::Hello {
            proto: 0,
            handle: "browser".into(),
        },
    );
    send(&mut browser, &C2S::ListLobbies);
    assert!(
        pump(&mut browser, Duration::from_secs(2), |m| {
            matches!(m, S2C::Lobbies { lobbies } if lobbies.iter().any(|l| l.name == "open"))
        }),
        "a version-0 browser could not list lobbies"
    );
}

#[test]
fn a_wrong_password_is_refused_and_the_right_one_is_not() {
    let port = start_server(proto::TURN_MS);
    let mut host = client(port, "host");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "private".into(),
            password: Some("hunter2".into()),
        },
    );
    assert!(pump(&mut host, Duration::from_secs(2), |m| matches!(
        m,
        S2C::Joined { .. }
    )));

    let mut guest = client(port, "guest");
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "private".into(),
            password: Some("wrong".into()),
        },
    );
    let reason = next_rejection(&mut guest, Duration::from_secs(2)).expect("wrong password refused");
    assert!(reason.contains("password"), "{reason}");
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "private".into(),
            password: Some("hunter2".into()),
        },
    );
    assert!(
        pump(&mut guest, Duration::from_secs(2), |m| matches!(
            m,
            S2C::Joined { .. }
        )),
        "the correct password was refused"
    );
}

/// The probe's deep step (`examples/probe.rs`, design 4.8), replayed over
/// the wire against the test server: Hello, a Welcome that carries this
/// build's stamp, CreateLobby, Joined, LeaveLobby, and the lobby gone from
/// the listing at once so a probe never leaves a table behind.
#[test]
fn the_probes_deep_step_passes_against_the_test_server() {
    let port = start_server(proto::TURN_MS);
    let mut ws = raw_client(port);
    send(
        &mut ws,
        &C2S::Hello {
            proto: proto::PROTO_VERSION,
            handle: "probe".into(),
        },
    );
    assert!(
        pump(&mut ws, Duration::from_secs(2), |m| {
            matches!(m, S2C::Welcome { proto, host, version, commit, players, lobbies }
                if proto == proto::PROTO_VERSION
                    && host == "test"
                    && version == kings_server::BUILD_VERSION
                    && commit == kings_server::BUILD_COMMIT
                    && players == 0
                    && lobbies == 0)
        }),
        "no Welcome with this build's stamp"
    );
    let lobby = format!("probe-{}", std::process::id());
    let id = create(&mut ws, &lobby);
    assert_eq!(id, 0, "the probe is the creator of its own lobby");
    send(&mut ws, &C2S::LeaveLobby);
    send(&mut ws, &C2S::ListLobbies);
    assert!(
        pump(&mut ws, Duration::from_secs(2), |m| {
            matches!(m, S2C::Lobbies { lobbies } if lobbies.is_empty())
        }),
        "the probe's lobby outlived the probe"
    );
}

/// A dropped socket mid-game eliminates the seat and, with two players,
/// ends the game for the one who stayed.
#[test]
fn a_disconnect_while_playing_ends_a_two_player_game() {
    let port = start_server(proto::TURN_MS);
    let mut a = client(port, "alice");
    let mut b = client(port, "bob");
    create(&mut a, "drop");
    join(&mut b, "drop");
    send(&mut a, &C2S::Start);
    assert!(pump(&mut a, Duration::from_secs(2), |m| matches!(
        m,
        S2C::Phase {
            phase: Phase::Playing,
            ..
        }
    )));
    drop(b);
    assert!(
        pump(&mut a, Duration::from_secs(3), |m| matches!(
            m,
            S2C::Phase {
                phase: Phase::Finished,
                winner: Some(0),
                ..
            }
        )),
        "the survivor was not declared the winner"
    );
}
