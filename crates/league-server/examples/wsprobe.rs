//! Headless league client for deploys: `wsprobe <ws-url> [lobby-name]`
//! greets, creates or joins a lobby, picks a champion, starts the match,
//! and prints the first live state tick it sees. The deploy script uses
//! it as a health check before it exposes a fresh tunnel — if this gets
//! an `on the wire` line, the server really speaks the protocol end to
//! end. Exits non-zero on any failure.

use std::time::{Duration, Instant};

use tungstenite::Message;
use tungstenite::stream::MaybeTlsStream;

use league_core::proto::{C2S, Cmd, Phase, S2C};

type Ws = tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(url) = args.next() else {
        eprintln!("usage: wsprobe <ws-url> [lobby]");
        std::process::exit(2);
    };
    let lobby = args.next().unwrap_or_else(|| "wsprobe".to_string());
    match run(&url, &lobby) {
        Ok(()) => println!("wsprobe: {url} speaks league protocol v{}", league_core::proto::PROTO_VERSION),
        Err(e) => {
            eprintln!("wsprobe FAILED: {e}");
            std::process::exit(1);
        }
    }
}

fn run(url: &str, lobby: &str) -> Result<(), String> {
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| e.to_string())?;
    match ws.get_ref() {
        MaybeTlsStream::Plain(s) => drop(s.set_read_timeout(Some(Duration::from_millis(5)))),
        MaybeTlsStream::Rustls(s) => drop(s.get_ref().set_read_timeout(Some(Duration::from_millis(5)))),
        _ => {}
    };
    let deadline = Instant::now() + Duration::from_secs(20);

    let mut welcomed = false;
    let mut joined = false;
    let mut started = false;
    let mut live_seen = false;
    let mut hello_sent = false;
    let mut next_send = Instant::now();

    while Instant::now() < deadline {
        drain(&mut ws, &mut welcomed, &mut joined, &mut started, &mut live_seen);
        if Instant::now() >= next_send {
            next_send = Instant::now() + Duration::from_millis(150);
            if !hello_sent {
                // the ONE hello: the server closes on a second
                send(&mut ws, &C2S::Hello { proto: league_core::proto::PROTO_VERSION, handle: "probe".into() })?;
                hello_sent = true;
            } else if !welcomed {
            } else if !joined {
                send(&mut ws, &C2S::CreateLobby { name: lobby.into(), password: None, mode: 1 })?;
            } else if !started {
                send(&mut ws, &C2S::Pick { champ: 1, d: 2, f: 0, runes: [0, 1, 2] })?;
                send(&mut ws, &C2S::StartMatch)?;
                started = true;
            } else if !live_seen {
                // nudge the world to prove commands flow, then wait for state
                send(&mut ws, &C2S::Cmd(Cmd::Move { x: -40.0, z: 0.0 }))?;
            } else {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Err(format!(
        "timed out: welcomed={welcomed} joined={joined} started={started} live={live_seen}"
    ))
}

fn drain(ws: &mut Ws, welcomed: &mut bool, joined: &mut bool, started: &mut bool, live: &mut bool) {
    loop {
        let Ok(frame) = ws.read() else { break };
        let Message::Text(t) = frame else { continue };
        let Ok(msg) = serde_json::from_str::<S2C>(&t) else { continue };
        match msg {
            S2C::Welcome { players, .. } => {
                println!("welcome: {players} playing");
                *welcomed = true;
            }
            S2C::Joined { id, roster, .. } => {
                println!("joined as slot {id}, roster {}", roster.len());
                *joined = true;
            }
            S2C::Rejected { reason } => {
                eprintln!("rejected: {reason}");
            }
            S2C::Phase { phase, left } => {
                if phase == Phase::Live && !*started {
                    *started = true;
                }
                println!("phase {phase:?} left {left:.1}");
            }
            S2C::Roster { roster } => {
                let picked = roster.iter().filter(|r| r.picked).count();
                println!("roster: {picked} picked");
            }
            S2C::State { tick, units, .. } => {
                if !*live {
                    println!("live: tick {tick}, {} units on the field", units.len());
                    *live = true;
                }
            }
            S2C::Result { winner, .. } => println!("result: team {winner} wins"),
            S2C::Pong { .. } | S2C::Lobbies { .. } | S2C::PlayerJoined { .. } | S2C::PlayerLeft { .. } => {}
        }
    }
}

fn send(ws: &mut Ws, msg: &C2S) -> Result<(), String> {
    // send, not write: write only buffers, and the server would sit on an
    // empty socket until the probe timed out
    ws.send(Message::text(serde_json::to_string(msg).unwrap()))
        .map_err(|e| e.to_string())
}

