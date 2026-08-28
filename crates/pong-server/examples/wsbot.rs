//! Headless online verification client (works over ws:// and wss://).
//!
//!     cargo run -p pong-server --example wsbot -- <URL> create|join <LOBBY> [PASSWORD] [HANDLE] [SECS]
//!
//! Creates or joins a lobby, plays sinusoid inputs once the match starts,
//! and reports how many state updates it saw. Exit 0 = the online loop works.

use std::time::{Duration, Instant};

use pong_core::proto::{C2S, S2C, PROTO_VERSION};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;

fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("usage: wsbot URL create|join LOBBY [PASSWORD] [HANDLE] [SECS]");
    let action = args.next().expect("create|join");
    let lobby = args.next().expect("lobby name");
    let password = args.next().filter(|p| !p.is_empty() && p != "-");
    let handle = args.next().unwrap_or_else(|| format!("wsbot-{action}"));
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);

    // rustls needs an explicitly installed crypto provider for wss.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (mut ws, _) = tungstenite::connect(&url).unwrap_or_else(|e| {
        eprintln!("WSBOT FAIL: connect {url}: {e}");
        std::process::exit(1);
    });
    match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(Some(Duration::from_millis(100))).unwrap(),
        MaybeTlsStream::Rustls(s) => {
            s.get_ref().set_read_timeout(Some(Duration::from_millis(100))).unwrap()
        }
        _ => {}
    }

    let send = |ws: &mut tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>, m: &C2S| {
        ws.send(Message::text(serde_json::to_string(m).unwrap())).unwrap_or_else(|e| {
            eprintln!("WSBOT FAIL: send: {e}");
            std::process::exit(1);
        });
    };

    send(&mut ws, &C2S::Hello { proto: PROTO_VERSION, handle: handle.clone() });
    match action.as_str() {
        "create" => send(&mut ws, &C2S::CreateLobby { name: lobby.clone(), password }),
        "join" => send(&mut ws, &C2S::JoinLobby { name: lobby.clone(), password }),
        other => {
            eprintln!("WSBOT FAIL: unknown action {other}");
            std::process::exit(1);
        }
    }

    let started = Instant::now();
    let mut in_match = false;
    let mut states: u64 = 0;
    let mut last_scores = [0u32; 2];
    let mut last_input = Instant::now() - Duration::from_secs(1);
    let mut last_ping = Instant::now();

    while started.elapsed() < Duration::from_secs(secs) {
        if in_match && last_input.elapsed() >= Duration::from_millis(100) {
            last_input = Instant::now();
            let t = started.elapsed().as_secs_f32();
            send(&mut ws, &C2S::Input { axis: (t * 1.3).sin() });
        }
        if last_ping.elapsed() >= Duration::from_secs(4) {
            last_ping = Instant::now();
            send(&mut ws, &C2S::Ping { nonce: 1 });
        }
        match ws.read() {
            Ok(Message::Text(t)) => match serde_json::from_str::<S2C>(t.as_str()) {
                Ok(S2C::Welcome { .. }) => println!("wsbot {handle}: welcomed"),
                Ok(S2C::LobbyCreated { name }) => println!("wsbot {handle}: created \"{name}\", waiting"),
                Ok(S2C::MatchStart { role, opponent }) => {
                    println!("wsbot {handle}: match started, role {role} vs {opponent}");
                    in_match = true;
                }
                Ok(S2C::State { scores, .. }) => {
                    states += 1;
                    last_scores = scores;
                }
                Ok(S2C::MatchEvent { scorer, won, scores }) => {
                    println!("wsbot {handle}: player {} scored (won={won}, {scores:?})", scorer + 1)
                }
                Ok(S2C::Error { message }) => {
                    eprintln!("WSBOT FAIL: server error: {message}");
                    std::process::exit(1);
                }
                Ok(S2C::OpponentLeft) => println!("wsbot {handle}: opponent left"),
                Ok(_) => {}
                Err(e) => {
                    eprintln!("WSBOT FAIL: bad server message: {e}");
                    std::process::exit(1);
                }
            },
            Ok(Message::Close(_)) => {
                eprintln!("WSBOT FAIL: server closed the connection");
                std::process::exit(1);
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                eprintln!("WSBOT FAIL: read: {e}");
                std::process::exit(1);
            }
        }
    }

    // Expect ~30 states/sec while in match; accept a third to tolerate the
    // waiting period before the opponent arrived.
    if !in_match || states < 10 {
        eprintln!("WSBOT FAIL: in_match={in_match} states={states}");
        std::process::exit(1);
    }
    println!("WSBOT OK: states={states} scores={last_scores:?}");
}
