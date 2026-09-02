//! `probe <ws or wss url> [--expect-commit <sha>]`: a health check that
//! proves the *hub* is alive and the *lobby path* runs, not just the
//! listener.
//!
//! `fire-server`'s probe exists because an HTTP `101` only says a connection
//! thread completed the WebSocket handshake; `pong-server` was once observed
//! with the listener up, handshakes succeeding and the hub loop dead. So this
//! speaks the protocol: `Hello`, and `Welcome` back, which only the hub can
//! produce. Then it goes one step deeper than fire's, because this server is
//! deployed by a script that rebuilds in place: it creates a lobby, requires
//! `Joined`, and leaves, proving the version gate admits this build and the
//! lobby machinery runs end to end. With `--expect-commit` it also requires
//! `Welcome.commit` to be the stamp of the binary just built, which catches
//! a missed pkill leaving last week's server on the port.
//!
//! Exit codes: 0 healthy, 1 unhealthy (reason on stderr, round trip on
//! stdout).

// This command-line health probe reports success and failure directly to its caller.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::time::{Duration, Instant};

use kings_core::proto::{self, C2S, S2C};
use tungstenite::Message;

const DEADLINE: Duration = Duration::from_secs(12);

type Ws = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// What the probe is waiting for, in order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Joined,
}

fn send(ws: &mut Ws, msg: &C2S) -> Result<(), String> {
    let text = serde_json::to_string(msg).map_err(|e| format!("cannot encode {msg:?}: {e}"))?;
    ws.send(Message::text(text))
        .map_err(|e| format!("send failed: {e}"))
}

// A linear diagnostic script: splitting the probe sequence into helpers would obscure the one path it exists to document.
#[allow(clippy::too_many_lines)]
fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(url) = args.next() else {
        eprintln!("usage: probe <ws-url> [--expect-commit <sha>]");
        return std::process::ExitCode::from(1);
    };
    let mut expect_commit: Option<String> = None;
    while let Some(arg) = args.next() {
        if arg == "--expect-commit" {
            expect_commit = args.next();
            if expect_commit.is_none() {
                eprintln!("probe: --expect-commit needs a sha");
                return std::process::ExitCode::from(1);
            }
        } else {
            eprintln!("probe: unknown argument {arg}");
            return std::process::ExitCode::from(1);
        }
    }

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

    let hello = C2S::Hello {
        proto: proto::PROTO_VERSION,
        handle: "probe".into(),
    };
    if let Err(e) = send(&mut ws, &hello) {
        eprintln!("probe: handshake succeeded but the {e}");
        return std::process::ExitCode::from(1);
    }

    let lobby = format!("probe-{}", std::process::id());
    let mut step = Step::Welcome;
    while t0.elapsed() < DEADLINE {
        match ws.read() {
            Ok(Message::Text(t)) => {
                let Ok(msg) = serde_json::from_str::<S2C>(&t) else {
                    continue;
                };
                match (msg, step) {
                    (
                        S2C::Welcome {
                            proto: server,
                            host,
                            version,
                            commit,
                            players,
                            lobbies,
                        },
                        Step::Welcome,
                    ) => {
                        if server != proto::PROTO_VERSION {
                            drop(ws.close(None));
                            eprintln!(
                                "probe: server is ALIVE but speaks kings protocol v{server}, \
                                 this build speaks v{}",
                                proto::PROTO_VERSION
                            );
                            return std::process::ExitCode::from(1);
                        }
                        if let Some(want) = &expect_commit
                            && want != &commit
                        {
                            drop(ws.close(None));
                            eprintln!(
                                "probe: server is ALIVE but is build '{commit}' (version \
                                 '{version}'), expected commit '{want}': an older server \
                                 is still on the port"
                            );
                            return std::process::ExitCode::from(1);
                        }
                        println!(
                            "probe: Welcome received, kings protocol v{server}, host '{host}', \
                             build '{version}' '{commit}', {players} in game, {lobbies} open, \
                             {} ms",
                            t0.elapsed().as_millis()
                        );
                        // The deep step: the lobby path through the version
                        // gate, which only this build's protocol can pass.
                        if let Err(e) = send(
                            &mut ws,
                            &C2S::CreateLobby {
                                name: lobby.clone(),
                                password: None,
                            },
                        ) {
                            eprintln!("probe: could not create a lobby: {e}");
                            return std::process::ExitCode::from(1);
                        }
                        step = Step::Joined;
                    }
                    (S2C::Joined { lobby: name, .. }, Step::Joined) => {
                        if name != lobby {
                            eprintln!("probe: joined '{name}' but asked for '{lobby}'");
                            return std::process::ExitCode::from(1);
                        }
                        // Leave so the probe's lobby does not sit in the
                        // list; the server removes an empty lobby at once.
                        if let Err(e) = send(&mut ws, &C2S::LeaveLobby) {
                            eprintln!("probe: could not leave the lobby: {e}");
                            return std::process::ExitCode::from(1);
                        }
                        drop(ws.close(None));
                        println!(
                            "probe: healthy, Welcome and Joined received, kings protocol v{}, \
                             {} ms round trip",
                            proto::PROTO_VERSION,
                            t0.elapsed().as_millis()
                        );
                        return std::process::ExitCode::SUCCESS;
                    }
                    (S2C::Rejected { reason }, _) => {
                        drop(ws.close(None));
                        eprintln!("probe: server refused the lobby step: {reason}");
                        return std::process::ExitCode::from(1);
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => {
                if step == Step::Welcome {
                    eprintln!(
                        "probe: server closed the connection without answering Hello, \
                         this is the listener-up/hub-dead signature"
                    );
                } else {
                    eprintln!("probe: server closed the connection before answering CreateLobby");
                }
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
    match step {
        Step::Welcome => eprintln!(
            "probe: no Welcome within {} s, the listener answers but the hub loop is not running",
            DEADLINE.as_secs()
        ),
        Step::Joined => eprintln!(
            "probe: Welcome came but no Joined within {} s, the lobby path is stuck",
            DEADLINE.as_secs()
        ),
    }
    std::process::ExitCode::from(1)
}
