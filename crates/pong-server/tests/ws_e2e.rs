//! End-to-end over a real `WebSocket`: create a passworded game, reject a bad
//! password, drop a second player in, shoot, and drop out.

use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use pong_core::proto::{C2S, PROTO_VERSION, S2C};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

type Ws = WebSocket<MaybeTlsStream<TcpStream>>;

fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        drop(pong_server::run(
            listener,
            pong_server::ServerConfig::default(),
        ));
    });
    port
}

fn connect(port: u16, handle: &str) -> Ws {
    let (mut ws, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(
        &mut ws,
        &C2S::Hello {
            proto: PROTO_VERSION,
            handle: handle.into(),
        },
    );
    match recv(&mut ws) {
        S2C::Welcome { .. } => ws,
        other => panic!("expected Welcome, got {other:?}"),
    }
}

fn send(ws: &mut Ws, msg: &C2S) {
    ws.send(Message::text(serde_json::to_string(msg).unwrap()))
        .unwrap();
}

fn recv(ws: &mut Ws) -> S2C {
    loop {
        match ws.read().unwrap() {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected frame: {other:?}"),
        }
    }
}

fn recv_until<T>(ws: &mut Ws, secs: u64, mut pred: impl FnMut(S2C) -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if let Some(v) = pred(recv(ws)) {
            return v;
        }
    }
    panic!("condition not met within {secs}s");
}

#[test]
// The scenario stays linear so each protocol transition is asserted in wire order.
#[allow(clippy::too_many_lines)]
fn drop_in_arena_flow_with_password() {
    let port = start_server();

    // Host creates a passworded game and is inside it immediately.
    let mut host = connect(port, "alice");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "arena".into(),
            password: Some("s3cret".into()),
        },
    );
    let (host_pid, seed) = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined {
            id, seed, players, ..
        } => {
            assert_eq!(players.len(), 1);
            Some((id, seed))
        }
        _ => None,
    });

    // Guest sees it listed with player count and the password flag.
    let mut guest = connect(port, "bob");
    send(&mut guest, &C2S::ListLobbies);
    match recv(&mut guest) {
        S2C::LobbyList { lobbies } => {
            assert_eq!(lobbies.len(), 1);
            assert_eq!(lobbies[0].name, "arena");
            assert_eq!(lobbies[0].host, "alice");
            assert!(lobbies[0].has_password);
            assert_eq!((lobbies[0].players, lobbies[0].cap), (1, 8));
        }
        other => panic!("expected LobbyList, got {other:?}"),
    }

    // Wrong password rejected without disconnecting.
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "arena".into(),
            password: Some("nope".into()),
        },
    );
    match recv(&mut guest) {
        S2C::Error { message } => assert!(message.contains("password"), "{message}"),
        other => panic!("expected Error, got {other:?}"),
    }

    // Correct password drops the guest into the SAME arena (same seed).
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "arena".into(),
            password: Some("s3cret".into()),
        },
    );
    let (guest_pid, guest_seed) = recv_until(&mut guest, 5, |m| match m {
        S2C::GameJoined {
            id, seed, players, ..
        } => {
            assert_eq!(players.len(), 2, "joiner sees the full roster");
            Some((id, seed))
        }
        _ => None,
    });
    assert_eq!(guest_seed, seed, "all players share the arena seed");
    assert_ne!(guest_pid, host_pid);
    recv_until(&mut host, 5, |m| match m {
        S2C::PlayerJoined { meta } => {
            assert_eq!(meta.handle, "bob");
            Some(())
        }
        _ => None,
    });

    // Guest holds fire: bullets must appear in the state stream.
    send(
        &mut guest,
        &C2S::Input {
            seq: 1,
            view_tick: 0,
            mx: 0.0,
            my: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            fire: true,
            sprint: false,
            crouch: false,
            reload: false,
            jump: false,
            shield: false,
            melee: false,
        },
    );
    // The next state must echo the input's seq back as this player's ack.
    recv_until(&mut guest, 5, |m| match m {
        S2C::State { players, .. } => players
            .iter()
            .find(|p| p.id == guest_pid)
            .filter(|p| p.ack == 1)
            .map(|_| ()),
        _ => None,
    });
    recv_until(&mut host, 5, |m| match m {
        S2C::State {
            bullets, players, ..
        } => {
            assert_eq!(players.len(), 2);
            (!bullets.is_empty()).then_some(())
        }
        _ => None,
    });

    // Guest leaves; host is told and the game keeps running.
    drop(guest);
    recv_until(&mut host, 10, |m| match m {
        S2C::PlayerLeft { id } => {
            assert_eq!(id, guest_pid);
            Some(())
        }
        _ => None,
    });
    // Host still receives states alone.
    recv_until(&mut host, 5, |m| match m {
        S2C::State { players, .. } => (players.len() == 1).then_some(()),
        _ => None,
    });
}

#[test]
fn old_proto_may_list_but_not_join() {
    let port = start_server();
    // A current-proto host opens a lobby.
    let mut host = connect(port, "alice");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "arena".into(),
            password: None,
        },
    );
    recv_until(&mut host, 5, |m| {
        matches!(m, S2C::GameJoined { .. }).then_some(())
    });

    // A stale client (or the hub's proto-0 browser) may list...
    let (mut old, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = old.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(
        &mut old,
        &C2S::Hello {
            proto: 0,
            handle: "browser".into(),
        },
    );
    recv_until(&mut old, 5, |m| {
        matches!(m, S2C::Welcome { .. }).then_some(())
    });
    send(&mut old, &C2S::ListLobbies);
    recv_until(&mut old, 5, |m| match m {
        S2C::LobbyList { lobbies } => (lobbies.len() == 1).then_some(()),
        _ => None,
    });

    // ...but entering a game requires the live protocol.
    send(
        &mut old,
        &C2S::JoinLobby {
            name: "arena".into(),
            password: None,
        },
    );
    recv_until(&mut old, 5, |m| match m {
        S2C::Error { message } => {
            assert!(message.contains("live version"), "{message}");
            // The sentence is the whole compatibility story a frozen build's
            // player ever sees, so it must still NAME both versions after a
            // bump — "play the live version" alone leaves them guessing
            // which build theirs is and which one is live.
            assert!(
                message.contains("v0"),
                "must name the stale version: {message}"
            );
            assert!(
                message.contains(&format!("v{PROTO_VERSION}")),
                "must name the live version: {message}"
            );
            Some(())
        }
        _ => None,
    });
}

#[test]
fn message_before_hello_disconnects() {
    let port = start_server();
    let (mut ws, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(&mut ws, &C2S::ListLobbies);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "server never closed the connection"
        );
        match ws.read() {
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

/// The wire half of the jump-reconciliation fix: a state that puts a player
/// in the air must carry the velocity that put them there. `y` alone cannot
/// restart the client's integrator - it replays gravity from `vy`, and a
/// state without it silently seeds that replay with zero.
#[test]
fn an_airborne_state_carries_the_velocity_that_made_it() {
    let port = start_server();
    let mut host = connect(port, "jumper");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "jump-wire".into(),
            password: None,
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });

    // Hold Space. One press is enough to leave the ground and the arc lasts
    // ~0.7 s, so a handful of inputs covers it. Paced like a real client:
    // faster than one message a tick and the server drops us as a flooder,
    // which is exactly what the first version of this test did.
    for seq in 1..6 {
        send(
            &mut host,
            &C2S::Input {
                seq,
                view_tick: 0,
                mx: 0.0,
                my: 0.0,
                ax: 1.0,
                az: 0.0,
                pitch: 0.0,
                fire: false,
                sprint: false,
                crouch: false,
                reload: false,
                jump: true,
                shield: false,
                melee: false,
            },
        );
        std::thread::sleep(Duration::from_millis(40));
    }

    let (y, vy) = recv_until(&mut host, 5, |m| match m {
        S2C::State { players, .. } => players
            .iter()
            .find(|p| p.id == me && p.y > 0.5)
            .map(|p| (p.y, p.vy)),
        _ => None,
    });
    assert!(
        vy != 0.0,
        "airborne at y={y} with vy={vy}: the state dropped the velocity"
    );
}

/// Jump is a press the sim consumes once, not a held key it re-applies. The
/// server keeps re-running the last input it received every tick, so a set
/// jump flag used to re-launch the player off every surface they landed on -
/// including a crate top, from which the second launch clears the containers
/// that are supposed to be unreachable hard cover.
#[test]
fn one_jump_press_launches_once_and_does_not_bunny_hop() {
    let port = start_server();
    let mut host = connect(port, "hopper");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "hop-once".into(),
            password: None,
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });

    // Exactly one input, with the press set. Nothing is sent afterwards, so
    // anything that happens next is the server re-applying what it holds.
    send(
        &mut host,
        &C2S::Input {
            seq: 1,
            view_tick: 0,
            mx: 0.0,
            my: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            fire: false,
            sprint: false,
            crouch: false,
            reload: false,
            jump: true,
            shield: false,
            melee: false,
        },
    );

    let y_of = |m: S2C, me: u8| match m {
        S2C::State { players, .. } => players.iter().find(|p| p.id == me).map(|p| p.y),
        _ => None,
    };
    // The one press must actually leave the ground.
    let peak = recv_until(&mut host, 5, |m| y_of(m, me).filter(|y| *y > 0.5));
    assert!(peak > 0.5, "the press never launched: {peak}");

    // Then land, and stay landed. A full arc is ~0.7 s; watch twice that.
    let mut landed = false;
    let deadline = Instant::now() + Duration::from_millis(1600);
    while Instant::now() < deadline {
        if let Some(y) = y_of(recv(&mut host), me) {
            if landed {
                assert!(y < 0.2, "re-launched without a new press: y={y}");
            } else if y < 0.05 {
                landed = true;
            }
        }
    }
    assert!(landed, "never came back down");
}

/// The state has to say how long the server has been applying the command it
/// acks, or the client cannot place the state on its own clock and guesses
/// the replay window from its send cadence instead.
#[test]
fn a_state_reports_how_long_the_acked_command_has_been_applied() {
    let port = start_server();
    let mut host = connect(port, "acker");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "ack-age".into(),
            password: None,
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });
    send(
        &mut host,
        &C2S::Input {
            seq: 9,
            view_tick: 0,
            mx: 1.0,
            my: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            fire: false,
            sprint: false,
            crouch: false,
            reload: false,
            jump: false,
            shield: false,
            melee: false,
        },
    );

    // Nothing else is sent, so the age of seq 9 must climb tick by tick.
    let mut ages = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while ages.len() < 4 && Instant::now() < deadline {
        if let S2C::State { players, .. } = recv(&mut host)
            && let Some(p) = players.iter().find(|p| p.id == me && p.ack == 9)
        {
            ages.push(p.ack_age_ticks);
        }
    }
    assert!(
        ages.len() >= 4,
        "not enough states carrying the ack: {ages:?}"
    );
    assert!(
        ages.windows(2).all(|w| w[1] > w[0]),
        "ack_age_ticks did not advance: {ages:?}"
    );
}

/// A press is an event, so it has to survive being overtaken. Two input
/// frames can land in the same inter-tick window - the hub drains every
/// queued event before it steps - and storing the second on top of the first
/// used to destroy the press before the sim ever ran. Under the old held-key
/// meaning that was free, because the next packet still carried the key down.
#[test]
fn a_press_survives_a_second_input_in_the_same_tick() {
    let port = start_server();
    let mut host = connect(port, "coalesced");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "coalesce".into(),
            password: None,
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });

    let input = |seq: u32, jump: bool| C2S::Input {
        seq,
        view_tick: 0,
        mx: 0.0,
        my: 0.0,
        ax: 1.0,
        az: 0.0,
        pitch: 0.0,
        fire: false,
        sprint: false,
        crouch: false,
        reload: false,
        jump,
        shield: false,
        melee: false,
    };
    // Back to back, deliberately with no sleep: the press and the packet that
    // overtakes it reach the hub inside one 16.7 ms window.
    send(&mut host, &input(1, true));
    send(&mut host, &input(2, false));

    let y = recv_until(&mut host, 5, |m| match m {
        S2C::State { players, .. } => players
            .iter()
            .find(|p| p.id == me && p.y > 0.5)
            .map(|p| p.y),
        _ => None,
    });
    assert!(
        y > 0.5,
        "the press was overwritten before the sim saw it: {y}"
    );
}
