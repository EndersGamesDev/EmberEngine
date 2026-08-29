//! End-to-end over real WebSockets: create a passworded game, reject a bad
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
        let _ = pong_server::run(listener, pong_server::ServerConfig::default());
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
            fire: true,
            sprint: false,
            crouch: false,
            reload: false,
            jump: false,
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
            assert!(players.len() == 2);
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
