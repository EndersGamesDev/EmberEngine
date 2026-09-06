//! `wsprobe <ws-url> [lobby] [--mode 1|3] [--expect-commit sha]` exercises a
//! fresh lobby, draft, match start, skill rank, shopping and movement over
//! the real WebSocket protocol. It leaves its lobby and exits unsuccessfully
//! on rejection, incompatible server identity, or a stalled simulation.

// A command-line health check reports its result directly to its caller.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::process::ExitCode;
use std::time::{Duration, Instant};

use league_core::proto::{self, C2S, Cmd, S2C};
use tungstenite::Message;
use tungstenite::stream::MaybeTlsStream;

type Ws = tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn main() -> ExitCode {
    let started = Instant::now();
    match options()
        .and_then(|(url, lobby, mode, commit)| run(&url, &lobby, mode, commit.as_deref()))
    {
        Ok(()) => {
            println!(
                "wsprobe: league protocol v{} healthy in {:.2}s",
                proto::PROTO_VERSION,
                started.elapsed().as_secs_f32()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!(
                "wsprobe FAILED after {:.2}s: {e}",
                started.elapsed().as_secs_f32()
            );
            ExitCode::FAILURE
        }
    }
}

fn options() -> Result<(String, String, u8, Option<String>), String> {
    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .ok_or("usage: wsprobe <ws-url> [lobby] [--mode 1|3] [--expect-commit sha]")?;
    let mut lobby = None;
    let mut mode = 1;
    let mut commit = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => {
                mode = match args.next().as_deref() {
                    Some("1") => 1,
                    Some("3") => 3,
                    _ => return Err("--mode must be 1 or 3".into()),
                };
            }
            "--expect-commit" => commit = Some(args.next().ok_or("--expect-commit needs a SHA")?),
            _ if !arg.starts_with('-') && lobby.is_none() => lobby = Some(arg),
            _ => return Err(format!("unexpected argument: {arg}")),
        }
    }
    Ok((
        url,
        lobby.unwrap_or_else(|| format!("wsprobe-{}", std::process::id())),
        mode,
        commit,
    ))
}

fn run(url: &str, lobby: &str, mode: u8, expected_commit: Option<&str>) -> Result<(), String> {
    drop(rustls::crypto::ring::default_provider().install_default());
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| e.to_string())?;
    let socket = match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_ref(),
        _ => return Err("unsupported WebSocket transport".into()),
    };
    socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(20);

    send(
        &mut ws,
        &C2S::Hello {
            proto: proto::PROTO_VERSION,
            handle: "probe".into(),
        },
    )?;
    let welcome = receive(&mut ws, deadline, "Welcome", |m| {
        matches!(m, S2C::Welcome { .. })
    })?;
    if let S2C::Welcome {
        proto: version,
        commit,
        ..
    } = welcome
    {
        if version != proto::PROTO_VERSION {
            return Err(format!(
                "server uses protocol v{version}, expected v{}",
                proto::PROTO_VERSION
            ));
        }
        if let Some(expected) = expected_commit {
            if expected != commit {
                return Err(format!("server commit {commit:?}, expected {expected:?}"));
            }
        }
    }

    send(
        &mut ws,
        &C2S::CreateLobby {
            name: lobby.into(),
            password: None,
            mode,
        },
    )?;
    receive(
        &mut ws,
        deadline,
        "a new lobby",
        |m| matches!(m, S2C::Joined { id: 0, mode: m, roster, .. } if *m == mode && roster.len() == usize::from(2 * mode)),
    )?;
    send(
        &mut ws,
        &C2S::Pick {
            champ: 1,
            d: 2,
            f: 0,
            runes: [0, 1, 2],
        },
    )?;
    receive(
        &mut ws,
        deadline,
        "confirmed draft",
        |m| matches!(m, S2C::Roster { roster } if roster.first().is_some_and(|r| r.picked && r.champ == 1 && r.runes == [0, 1, 2])),
    )?;
    send(&mut ws, &C2S::StartMatch)?;
    let first = receive(
        &mut ws,
        deadline,
        "live state",
        |m| matches!(m, S2C::State { champs, .. } if champs.len() == usize::from(2 * mode)),
    )?;
    let S2C::State { units, .. } = first else {
        unreachable!()
    };
    let start_x = units
        .iter()
        .find(|u| u.k == 0 && u.slot == 0)
        .ok_or("live state has no local champion")?
        .x;
    for cmd in [
        Cmd::Rank { slot: 0 },
        Cmd::Buy { item: 1 },
        Cmd::Move { x: -40.0, z: 0.0 },
    ] {
        send(&mut ws, &C2S::Cmd(cmd))?;
    }
    let live = receive(
        &mut ws,
        deadline,
        "rank, purchase and movement",
        |m| matches!(m, S2C::State { tick, units, champs, .. } if *tick > 0 && units.iter().any(|u| u.k == 0 && u.slot == 0 && u.x > start_x + 0.5) && champs.iter().any(|c| c.slot == 0 && c.ranks[0] == 1 && c.items[0] == 1)),
    )?;
    if let S2C::State { tick, units, .. } = live {
        println!(
            "on the wire: {mode}v{mode}, tick {tick}, {} units; skill, item and movement confirmed",
            units.len()
        );
    }
    send(&mut ws, &C2S::Ping { nonce: 42 })?;
    receive(&mut ws, deadline, "Pong", |m| {
        matches!(m, S2C::Pong { nonce: 42 })
    })?;
    send(&mut ws, &C2S::LeaveLobby)?;
    ws.close(None).map_err(|e| e.to_string())?;
    Ok(())
}

fn receive(
    ws: &mut Ws,
    deadline: Instant,
    expected: &str,
    matches: impl Fn(&S2C) -> bool,
) -> Result<S2C, String> {
    while Instant::now() < deadline {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let msg: S2C = serde_json::from_str(&text)
                    .map_err(|e| format!("undecodable server response: {e}"))?;
                if let S2C::Rejected { reason } = &msg {
                    return Err(format!("server rejected the probe: {reason}"));
                }
                if matches(&msg) {
                    return Ok(msg);
                }
            }
            Ok(Message::Close(_)) => {
                return Err(format!("server closed while waiting for {expected}"));
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(e) => {
                return Err(format!(
                    "WebSocket failed while waiting for {expected}: {e}"
                ));
            }
        }
    }
    Err(format!("timed out waiting for {expected}"))
}

fn send(ws: &mut Ws, msg: &C2S) -> Result<(), String> {
    let text = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    ws.send(Message::text(text)).map_err(|e| e.to_string())
}
