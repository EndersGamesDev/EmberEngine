//! Headless verification client. Connects, walks in a circle, and checks that
//! snapshots arrive and show it moving. Exit code 0 = multiplayer loop works.
//!
//!     cargo run -p ember-net --example netbot -- [ADDR] [NAME] [SECONDS]

use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ember_net::{read_msg, write_msg, ClientMsg, PlayerId, ServerMsg, PROTOCOL_VERSION};

#[derive(Default)]
struct Stats {
    snapshots: u64,
    first_pos: Option<[f32; 2]>,
    last_pos: Option<[f32; 2]>,
    max_players: usize,
    rtts_ms: Vec<f64>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let addr = args
        .next()
        .unwrap_or_else(|| format!("127.0.0.1:{}", ember_net::DEFAULT_PORT));
    let name = args.next().unwrap_or_else(|| "netbot".into());
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(5);

    let mut stream = TcpStream::connect(&addr).unwrap_or_else(|e| {
        eprintln!("NETBOT FAIL: connect {addr}: {e}");
        std::process::exit(1);
    });
    stream.set_nodelay(true).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

    write_msg(&mut stream, &ClientMsg::Hello { protocol: PROTOCOL_VERSION, name: name.clone() })
        .unwrap();
    let my_id: PlayerId = match read_msg::<_, ServerMsg>(&mut stream) {
        Ok(ServerMsg::Welcome { id, tick_hz, roster, .. }) => {
            println!("netbot {name}: joined as {id:?}, tick {tick_hz} Hz, {} online", roster.len());
            id
        }
        Ok(ServerMsg::Reject { reason }) => {
            eprintln!("NETBOT FAIL: rejected: {reason}");
            std::process::exit(1);
        }
        other => {
            eprintln!("NETBOT FAIL: expected Welcome, got {other:?}");
            std::process::exit(1);
        }
    };

    let stats = Arc::new(Mutex::new(Stats::default()));
    let dead = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    {
        let stats = Arc::clone(&stats);
        let dead = Arc::clone(&dead);
        let mut reader = stream.try_clone().unwrap();
        std::thread::spawn(move || loop {
            match read_msg::<_, ServerMsg>(&mut reader) {
                Ok(ServerMsg::Snapshot { players, .. }) => {
                    let mut s = stats.lock().unwrap();
                    s.snapshots += 1;
                    s.max_players = s.max_players.max(players.len());
                    if let Some(me) = players.iter().find(|p| p.id == my_id) {
                        if s.first_pos.is_none() {
                            s.first_pos = Some(me.pos);
                        }
                        s.last_pos = Some(me.pos);
                    }
                }
                Ok(ServerMsg::Pong { nonce }) => {
                    let sent_ms = nonce as f64;
                    let now_ms = started.elapsed().as_secs_f64() * 1000.0;
                    stats.lock().unwrap().rtts_ms.push(now_ms - sent_ms);
                }
                Ok(_) => {}
                Err(_) => {
                    dead.store(true, Ordering::Relaxed);
                    break;
                }
            }
        });
    }

    let mut last_ping = Instant::now() - Duration::from_secs(1);
    while started.elapsed() < Duration::from_secs(secs) && !dead.load(Ordering::Relaxed) {
        let t = started.elapsed().as_secs_f32();
        let dir = [(t * 1.5).cos(), (t * 1.5).sin()];
        write_msg(&mut stream, &ClientMsg::Input { move_dir: dir }).unwrap_or_else(|e| {
            eprintln!("NETBOT FAIL: send: {e}");
            std::process::exit(1);
        });
        if last_ping.elapsed() >= Duration::from_secs(1) {
            last_ping = Instant::now();
            let nonce = started.elapsed().as_millis() as u32;
            let _ = write_msg(&mut stream, &ClientMsg::Ping { nonce });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Capture liveness BEFORE Bye: the server closing the socket after our
    // Bye is the expected goodbye, not a failure.
    let died_early = dead.load(Ordering::Relaxed);
    let _ = write_msg(&mut stream, &ClientMsg::Bye);
    std::thread::sleep(Duration::from_millis(200));

    let s = stats.lock().unwrap();
    let moved = match (s.first_pos, s.last_pos) {
        (Some(a), Some(b)) => {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            (dx * dx + dy * dy).sqrt()
        }
        _ => 0.0,
    };
    let avg_rtt = if s.rtts_ms.is_empty() {
        f64::NAN
    } else {
        s.rtts_ms.iter().sum::<f64>() / s.rtts_ms.len() as f64
    };
    // Expect roughly TICK_HZ snapshots/sec; accept half to tolerate slow links.
    let min_snapshots = ember_net::TICK_HZ as u64 * secs / 2;
    if died_early {
        eprintln!("NETBOT FAIL: connection died early");
        std::process::exit(1);
    }
    if s.snapshots < min_snapshots || moved < 2.0 {
        eprintln!(
            "NETBOT FAIL: snapshots={} (need {min_snapshots}), moved={moved:.2} (need 2.0)",
            s.snapshots
        );
        std::process::exit(1);
    }
    println!(
        "NETBOT OK: snapshots={} moved={moved:.1} max_players_seen={} avg_rtt={avg_rtt:.1}ms",
        s.snapshots, s.max_players
    );
}
