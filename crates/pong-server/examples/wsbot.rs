// This health-check CLI intentionally reports status through standard streams and exit codes.
#![allow(clippy::exit, clippy::print_stderr, clippy::print_stdout)]
// Elapsed-time casts intentionally produce bounded protocol sequence and animation counters.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

//! Headless arena bot (works over `ws://` and `wss://`).
//!
//!     cargo run -p pong-server --example wsbot -- <URL> create|join <LOBBY> [PASSWORD|-] [HANDLE] [SECS] [MODES]
//!
//! Creates or joins a game, runs in circles spraying bullets, and reports
//! how many state updates it saw. Exit 0 = the online loop works.
//!
//! MODES is an optional comma-separated list that switches on the parts of
//! the protocol the default spray never touches: `shield` holds Q, `jump`
//! presses Space about once a second, `nofire` keeps the trigger up.
//!
//! `jump` PULSES deliberately. Since v11 the flag is a press the sim consumes
//! on one tick, so a bot that held it set would re-launch off every surface
//! it touched and make a broken build look fine. Two bots, one plain and one
//! `shield,nofire`, are enough to watch a round get reflected - which the
//! default bot can never do, because it never raises the plate.

use std::time::{Duration, Instant};

use pong_core::proto::{C2S, PROTO_VERSION, S2C};
use tungstenite::Message;
use tungstenite::stream::MaybeTlsStream;

// Keeping the scripted session linear makes its health-check sequence auditable.
#[allow(clippy::too_many_lines)]
fn main() {
    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .expect("usage: wsbot URL create|join LOBBY [PASSWORD|-] [HANDLE] [SECS]");
    let action = args.next().expect("create|join");
    let lobby = args.next().expect("lobby name");
    let password = args.next().filter(|p| !p.is_empty() && p != "-");
    let handle = args.next().unwrap_or_else(|| format!("wsbot-{action}"));
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);
    let modes: Vec<String> = args
        .next()
        .map(|m| {
            m.split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let has = |m: &str| modes.iter().any(|s| s == m);
    let (shield, jump, nofire) = (has("shield"), has("jump"), has("nofire"));

    // rustls needs an explicitly installed crypto provider for wss.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (mut ws, _) = tungstenite::connect(&url).unwrap_or_else(|e| {
        eprintln!("WSBOT FAIL: connect {url}: {e}");
        std::process::exit(1);
    });
    match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap(),
        MaybeTlsStream::Rustls(s) => s
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap(),
        _ => {}
    }

    let send = |ws: &mut tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>, m: &C2S| {
        ws.send(Message::text(serde_json::to_string(m).unwrap()))
            .unwrap_or_else(|e| {
                eprintln!("WSBOT FAIL: send: {e}");
                std::process::exit(1);
            });
    };

    send(
        &mut ws,
        &C2S::Hello {
            proto: PROTO_VERSION,
            handle: handle.clone(),
        },
    );
    match action.as_str() {
        "create" => send(
            &mut ws,
            &C2S::CreateLobby {
                name: lobby,
                password,
            },
        ),
        "join" => send(
            &mut ws,
            &C2S::JoinLobby {
                name: lobby,
                password,
            },
        ),
        other => {
            eprintln!("WSBOT FAIL: unknown action {other}");
            std::process::exit(1);
        }
    }

    let started = Instant::now();
    let mut in_game = false;
    let mut my_id: Option<u8> = None;
    let mut states: u64 = 0;
    let mut kills_seen: u64 = 0;
    let mut max_players = 0usize;
    let mut bullets_seen: u64 = 0;
    let now = Instant::now();
    let mut last_input = now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
    let mut last_ping = Instant::now();

    while started.elapsed() < Duration::from_secs(secs) {
        if in_game && last_input.elapsed() >= Duration::from_millis(50) {
            last_input = Instant::now();
            let t = started.elapsed().as_secs_f32();
            send(
                &mut ws,
                &C2S::Input {
                    seq: (t * 20.0) as u32,
                    view_tick: 0,
                    mx: (t * 0.9).cos(),
                    my: (t * 0.9).sin(),
                    ax: (t * 1.7).cos(),
                    az: (t * 1.7).sin(),
                    pitch: (t * 0.6).sin() * 0.7,
                    fire: !nofire,
                    sprint: (t as u64).is_multiple_of(3),
                    crouch: false,
                    reload: false,
                    // A press, not a level - see the module docs.
                    jump: jump && ((t * 20.0) as u32).is_multiple_of(24),
                    shield,
                },
            );
        }
        if last_ping.elapsed() >= Duration::from_secs(4) {
            last_ping = Instant::now();
            send(&mut ws, &C2S::Ping { nonce: 1 });
        }
        match ws.read() {
            Ok(Message::Text(t)) => match serde_json::from_str::<S2C>(t.as_str()) {
                Ok(S2C::Welcome { .. }) => println!("wsbot {handle}: welcomed"),
                Ok(S2C::GameJoined {
                    id, seed, players, ..
                }) => {
                    println!(
                        "wsbot {handle}: in the arena as #{id} (seed {seed}, {} players)",
                        players.len()
                    );
                    my_id = Some(id);
                    in_game = true;
                }
                Ok(S2C::PlayerJoined { meta }) => {
                    println!("wsbot {handle}: {} joined", meta.handle);
                }
                Ok(S2C::PlayerLeft { id }) => println!("wsbot {handle}: #{id} left"),
                Ok(S2C::State {
                    players, bullets, ..
                }) => {
                    states += 1;
                    max_players = max_players.max(players.len());
                    bullets_seen += bullets.len() as u64;
                }
                Ok(S2C::Kill { killer, victim }) => {
                    kills_seen += 1;
                    let me = my_id.unwrap_or(255);
                    if killer == me {
                        println!("wsbot {handle}: fragged #{victim}!");
                    } else if victim == me {
                        println!("wsbot {handle}: fragged by #{killer}");
                    }
                }
                Ok(S2C::Error { message }) => {
                    eprintln!("WSBOT FAIL: server error: {message}");
                    std::process::exit(1);
                }
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
                // os error 997 is Windows ERROR_IO_PENDING: a read that timed
                // out inside an overlapped operation. Rust cannot categorise
                // it, so it matches neither arm below and would end the loop.
                // This example is now deploy-pong-online.sh's health check and
                // runs on Windows, where that would be a spurious deploy
                // failure. (fire_core::proto::is_transient_read is the same
                // predicate; pong-server does not depend on fire-core.)
                if e.raw_os_error() == Some(997)
                    || e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                eprintln!("WSBOT FAIL: read: {e}");
                std::process::exit(1);
            }
        }
    }

    // A shielding bot cannot fire - the server blocks its own trigger while
    // the plate is up - so bullets are only evidence of a working loop when
    // this bot was actually shooting.
    let expects_bullets = !nofire && !shield;
    if !in_game || states < 10 || (expects_bullets && bullets_seen == 0) {
        eprintln!("WSBOT FAIL: in_game={in_game} states={states} bullets_seen={bullets_seen}");
        std::process::exit(1);
    }
    let modes_note = if modes.is_empty() {
        String::new()
    } else {
        format!(" modes={}", modes.join(","))
    };
    println!(
        "WSBOT OK: states={states} max_players={max_players} bullets_seen={bullets_seen} kills_seen={kills_seen}{modes_note}"
    );
}
