#![cfg(feature = "demo")]

use std::net::TcpListener;
use std::time::Duration;

use ember_net::outer::{OuterErrorCode, ServerMessage};
use ember_server::{Host, HostConfig, demo_registry};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

type Client = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn start_host() -> (u16, ember_server::DrainHandle) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = HostConfig {
        cap_loopback: true,
        ..HostConfig::default()
    };
    let host = Host::new(demo_registry().unwrap(), config).unwrap();
    let drain = host.drain_handle();
    std::thread::spawn(move || {
        host.run_on(listener).unwrap();
    });
    (port, drain)
}

fn connect(port: u16, handle: &str) -> Client {
    let (websocket, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}"))
        .expect("fixture WebSocket connection");
    let mut websocket = websocket;
    if let MaybeTlsStream::Plain(stream) = websocket.get_ref() {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
    }
    websocket
        .send(Message::text(format!(
            r#"{{"type":"hello","payload":{{"outer_version":1,"handle":"{handle}"}}}}"#
        )))
        .unwrap();
    assert!(matches!(
        read_outer(&mut websocket),
        ServerMessage::Welcome(_)
    ));
    websocket
}

fn read_outer(websocket: &mut Client) -> ServerMessage {
    loop {
        match websocket.read().unwrap() {
            Message::Text(text) => return serde_json::from_str(text.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected outer text response, got {other:?}"),
        }
    }
}

#[test]
fn feature_gated_fixture_exercises_canonical_host_and_drain() {
    let (port, drain) = start_host();
    let mut creator = connect(port, "alice");
    creator
        .send(Message::text(concat!(
            r#"{"type":"create_lobby","payload":{"game_id":"fixture","#,
            r#""game_version":1,"lobby_name":"room","password":null}}"#,
        )))
        .unwrap();
    assert!(matches!(read_outer(&mut creator), ServerMessage::Joined(_)));
    assert_eq!(creator.read().unwrap(), Message::text("fixture-ready"));

    creator.send(Message::text("inner-text")).unwrap();
    assert_eq!(creator.read().unwrap(), Message::text("inner-text"));
    creator.send(Message::binary(vec![1_u8, 2, 3])).unwrap();
    assert_eq!(creator.read().unwrap(), Message::binary(vec![1_u8, 2, 3]));

    let mut browser = connect(port, "bob");
    drain.stop_admission();
    browser
        .send(Message::text(concat!(
            r#"{"type":"join_lobby","payload":{"game_id":"fixture","#,
            r#""game_version":1,"lobby_name":"room","password":null}}"#,
        )))
        .unwrap();
    let ServerMessage::Error(error) = read_outer(&mut browser) else {
        panic!("drain must return a structured browsing-state refusal");
    };
    assert_eq!(error.code, OuterErrorCode::InvalidRequest);
    assert!(error.message.contains("draining"));

    browser
        .send(Message::text(r#"{"type":"list_lobbies"}"#))
        .unwrap();
    let ServerMessage::Lobbies(lobbies) = read_outer(&mut browser) else {
        panic!("drain refusal must leave the connection browsing");
    };
    assert_eq!(lobbies.entries.len(), 1);
    assert_eq!(lobbies.entries[0].occupancy, 1);
    assert_eq!(drain.occupancy()[0].players, 1);
}
