//! `cargo run -p fire-server --example probe -- wss://host` — a health check
//! that proves the *event loop* is alive, not just the listener.
//!
//! The deploy scripts used to accept an HTTP `101` as proof the server was up.
//! A `101` only says a connection thread completed the WebSocket handshake.
//! `pong-server` was observed on specht in exactly that state: listener up,
//! handshakes succeeding, hub loop dead, every peer getting `101` and then an
//! immediate close — and the deploy would have printed ONLINE.
//!
//! So this speaks the protocol. It sends `Hello` and requires `Welcome` back,
//! which can only be produced by the hub thread that owns the lobbies. It also
//! checks the version the server reports, so a protocol mismatch is named at
//! deploy time rather than by the first player who cannot join.
//!
//! Exit codes: 0 healthy, 1 unhealthy (reason on stderr).

// This command-line health probe reports success and failure directly to its caller.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::time::{Duration, Instant};

use fire_core::proto::{self, C2S, S2C};
use tungstenite::Message;

const DEADLINE: Duration = Duration::from_secs(12);

/// Occupancy check, for the deploy to consult before it restarts anything.
/// A separate exit code from "unhealthy" on purpose: a deploy must refuse when
/// people are mid-race, but must NOT refuse merely because the old server is
/// unreachable — that is precisely when a redeploy is most needed.
const EXIT_OCCUPIED: u8 = 2;

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(url) = args.next() else {
        eprintln!("usage: probe <ws-url> [--require-empty]");
        return std::process::ExitCode::from(1);
    };
    let require_empty = args.any(|a| a == "--require-empty");

    // Needed for wss:// through the tunnel; harmless for plain ws://.
    drop(rustls::crypto::ring::default_provider().install_default());

    let t0 = Instant::now();
    let (mut ws, _) = match tungstenite::connect(&url) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("probe: cannot connect to {url}: {e}");
            return std::process::ExitCode::from(1);
        }
    };
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_ref() {
        drop(s.set_read_timeout(Some(Duration::from_millis(200))));
    }

    let hello = C2S::Hello { proto: proto::PROTO_VERSION, handle: "probe".into() };
    if let Err(e) = ws.send(Message::text(serde_json::to_string(&hello).unwrap())) {
        eprintln!("probe: handshake succeeded but the send failed: {e}");
        return std::process::ExitCode::from(1);
    }

    while t0.elapsed() < DEADLINE {
        match ws.read() {
            Ok(Message::Text(t)) => {
                let Ok(msg) = serde_json::from_str::<S2C>(&t) else { continue };
                match msg {
                    S2C::Welcome { proto: server } => {
                        if server != proto::PROTO_VERSION {
                            drop(ws.close(None));
                            eprintln!(
                                "probe: server is ALIVE but speaks fire protocol v{server}, \
                                 this build speaks v{}",
                                proto::PROTO_VERSION
                            );
                            return std::process::ExitCode::from(1);
                        }
                        println!(
                            "probe: healthy — Welcome received, fire protocol v{server}, \
                             {} ms round trip",
                            t0.elapsed().as_millis()
                        );
                        if !require_empty {
                            drop(ws.close(None));
                            return std::process::ExitCode::SUCCESS;
                        }
                        // Ask who is on the server before anyone restarts it.
                        let list = serde_json::to_string(&C2S::ListLobbies).unwrap();
                        if ws.send(Message::text(list)).is_err() {
                            eprintln!("probe: could not ask for the lobby list");
                            return std::process::ExitCode::from(1);
                        }
                    }
                    S2C::Lobbies { lobbies } => {
                        drop(ws.close(None));
                        let busy: Vec<_> = lobbies.iter().filter(|l| l.players > 0).collect();
                        let total: u32 = busy.iter().map(|l| l.players as u32).sum();
                        if total == 0 {
                            println!("probe: nobody in game ({} lobbies)", lobbies.len());
                            return std::process::ExitCode::SUCCESS;
                        }
                        for l in &busy {
                            println!(
                                "probe: OCCUPIED — lobby '{}' has {}/{} player(s){}",
                                l.name,
                                l.players,
                                l.cap,
                                if l.racing { ", racing" } else { "" }
                            );
                        }
                        eprintln!("probe: {total} player(s) in game");
                        return std::process::ExitCode::from(EXIT_OCCUPIED);
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => {
                eprintln!(
                    "probe: server closed the connection without answering Hello — \
                     this is the listener-up/hub-dead signature"
                );
                return std::process::ExitCode::from(1);
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(e) => {
                eprintln!("probe: read failed: {e}");
                return std::process::ExitCode::from(1);
            }
        }
    }
    eprintln!(
        "probe: no Welcome within {} s — the listener answers but the hub loop is not running",
        DEADLINE.as_secs()
    );
    std::process::ExitCode::from(1)
}
