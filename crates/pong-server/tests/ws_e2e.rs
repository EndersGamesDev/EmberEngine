//! End-to-end over real WebSockets: create a passworded lobby, reject a bad
//! password, play a bit of the match, and reopen the lobby on disconnect.

use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use pong_core::proto::{C2S, S2C, PROTO_VERSION};
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
    send(&mut ws, &C2S::Hello { proto: PROTO_VERSION, handle: handle.into() });
    match recv(&mut ws) {
        S2C::Welcome { .. } => ws,
        other => panic!("expected Welcome, got {other:?}"),
    }
}

fn send(ws: &mut Ws, msg: &C2S) {
    ws.send(Message::text(serde_json::to_string(msg).unwrap())).unwrap();
}

/// Next parsed message, skipping WS control frames.
fn recv(ws: &mut Ws) -> S2C {
    loop {
        match ws.read().unwrap() {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected frame: {other:?}"),
        }
    }
}

/// Wait (skipping other messages) until `pred` returns Some.
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
fn full_match_flow_with_password() {
    let port = start_server();

    // Host creates a passworded lobby.
    let mut host = connect(port, "alice");
    send(&mut host, &C2S::CreateLobby { name: "duel".into(), password: Some("s3cret".into()) });
    match recv(&mut host) {
        S2C::LobbyCreated { name } => assert_eq!(name, "duel"),
        other => panic!("expected LobbyCreated, got {other:?}"),
    }

    // Guest sees it in the list, flagged as passworded.
    let mut guest = connect(port, "bob");
    send(&mut guest, &C2S::ListLobbies);
    match recv(&mut guest) {
        S2C::LobbyList { lobbies } => {
            assert_eq!(lobbies.len(), 1);
            assert_eq!(lobbies[0].name, "duel");
            assert_eq!(lobbies[0].host, "alice");
            assert!(lobbies[0].has_password);
        }
        other => panic!("expected LobbyList, got {other:?}"),
    }

    // Wrong password is rejected without disconnecting.
    send(&mut guest, &C2S::JoinLobby { name: "duel".into(), password: Some("nope".into()) });
    match recv(&mut guest) {
        S2C::Error { message } => assert!(message.contains("password"), "{message}"),
        other => panic!("expected Error, got {other:?}"),
    }

    // Correct password starts the match for both, with correct roles.
    send(&mut guest, &C2S::JoinLobby { name: "duel".into(), password: Some("s3cret".into()) });
    let (g_role, g_opp) = recv_until(&mut guest, 5, |m| match m {
        S2C::MatchStart { role, opponent } => Some((role, opponent)),
        _ => None,
    });
    assert_eq!((g_role, g_opp.as_str()), (1, "alice"));
    let (h_role, h_opp) = recv_until(&mut host, 5, |m| match m {
        S2C::MatchStart { role, opponent } => Some((role, opponent)),
        _ => None,
    });
    assert_eq!((h_role, h_opp.as_str()), (0, "bob"));

    // Host holds right; both peers must see paddle 0 move right in states.
    send(&mut host, &C2S::Input { axis: 1.0 });
    let moved = recv_until(&mut guest, 5, |m| match m {
        S2C::State { paddles, .. } if paddles[0] > 1.0 => Some(paddles[0]),
        _ => None,
    });
    assert!(moved > 1.0);

    // Guest disconnects; host gets OpponentLeft and the lobby reopens.
    drop(guest);
    recv_until(&mut host, 5, |m| matches!(m, S2C::OpponentLeft).then_some(()));

    // A new guest can join the reopened lobby (same name, same password).
    let mut guest2 = connect(port, "carol");
    send(&mut guest2, &C2S::JoinLobby { name: "duel".into(), password: Some("s3cret".into()) });
    let role = recv_until(&mut guest2, 5, |m| match m {
        S2C::MatchStart { role, .. } => Some(role),
        _ => None,
    });
    assert_eq!(role, 1);
}

#[test]
fn message_before_hello_disconnects() {
    let port = start_server();
    let (mut ws, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(&mut ws, &C2S::ListLobbies);
    // Server closes on protocol violation: reads must end in error/close.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "server never closed the connection");
        match ws.read() {
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
}
