// This health-check CLI intentionally reports status through standard streams and exit codes.
#![allow(clippy::exit, clippy::print_stderr, clippy::print_stdout)]
// Elapsed-time casts intentionally produce bounded protocol sequence and animation counters.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

//! Headless arena bot (works over `ws://` and `wss://`).
//!
//!     cargo run -p arena-server --example wsbot -- [--map NAME] [--mode NAME] [--bonk] <URL> create|join <LOBBY> [PASSWORD|-] [HANDLE] [SECS] [MODES]
//!
//! Creates or joins a game, runs in circles spraying bullets, and reports
//! how many state updates it saw. Exit 0 = the online loop works.
//!
//! `--map NAME` (or `--map=NAME`) is the level a `create` asks for; the
//! default is `freight-yard`, the server's own default. `--mode NAME` (or
//! `--mode=NAME`) is the `GameMode` a `create` asks for: `ffa` (the
//! default), `tdm` or `hill`; a joiner plays whatever the lobby runs and the
//! bot prints the round it sees end. `--bonk` (also
//! spelled `bonk` in MODES) hunts loot blocks: the bot rebuilds the level
//! from `GameJoined` exactly as a client does, walks a straight line to the
//! nearest block a floor jump can reach, and presses jump every 60 ticks
//! once it is under it, so a run on either map counts `Loot` events. There
//! is no pathfinding: a bot that stops moving for two seconds against cover
//! gives up on that block and turns to the next one.
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

use arena_core::proto::{C2S, PROTO_VERSION, S2C};
use arena_core::shooter::{BODY_H_STAND, Cover, GRAVITY, JUMP_VEL, Level, MAP_FREIGHT_YARD};
use tungstenite::Message;
use tungstenite::stream::MaybeTlsStream;

// Keeping the scripted session linear makes its health-check sequence auditable.
#[allow(clippy::too_many_lines)]
fn main() {
    // Flags first, wherever they sit; what is left is positional.
    let mut map = MAP_FREIGHT_YARD.to_string();
    let mut mode = String::new();
    let mut bonk_flag = false;
    let mut positional: Vec<String> = Vec::new();
    let mut raw = std::env::args().skip(1);
    while let Some(a) = raw.next() {
        if let Some(m) = a.strip_prefix("--map=") {
            map = m.to_string();
        } else if a == "--map" {
            map = raw.next().unwrap_or_else(|| {
                eprintln!("WSBOT FAIL: --map needs a level name");
                std::process::exit(1);
            });
        } else if let Some(m) = a.strip_prefix("--mode=") {
            mode = m.to_string();
        } else if a == "--mode" {
            mode = raw.next().unwrap_or_else(|| {
                eprintln!("WSBOT FAIL: --mode needs a mode name");
                std::process::exit(1);
            });
        } else if a == "--bonk" {
            bonk_flag = true;
        } else {
            positional.push(a);
        }
    }
    let mut args = positional.into_iter();
    let url = args
        .next()
        .expect("usage: wsbot [--map NAME] [--mode NAME] [--bonk] URL create|join LOBBY [PASSWORD|-] [HANDLE] [SECS] [MODES]");
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
    let bonk = bonk_flag || has("bonk");

    // rustls needs an explicitly installed crypto provider for wss.
    drop(rustls::crypto::ring::default_provider().install_default());
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
                map,
                mode,
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
    let (mut hits_seen, mut blasts_seen, mut loot_seen) = (0u64, 0u64, 0u64);
    let mut rounds_seen: u64 = 0;
    let now = Instant::now();
    let mut last_input = now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
    let mut last_ping = Instant::now();
    // The block hunt: the centres of every block a floor jump can reach,
    // where this bot last stood, which block it is walking to, and when it
    // last made progress toward it.
    let mut blocks: Vec<[f32; 2]> = Vec::new();
    let mut me_pos: Option<[f32; 2]> = None;
    let mut target = 0usize;
    let mut target_since = Instant::now();
    let mut progress = (Instant::now(), [f32::NAN, f32::NAN]);

    while started.elapsed() < Duration::from_secs(secs) {
        if in_game && last_input.elapsed() >= Duration::from_millis(50) {
            last_input = Instant::now();
            let t = started.elapsed().as_secs_f32();
            let frame = (t * 20.0) as u32;
            let circle = [(t * 0.9).cos(), (t * 0.9).sin()];
            // On the way: walk straight at the block, hopping once a second
            // so a crate, a sandbag or the dock in the way is mounted the
            // way a player mounts it. Under it: stand still and keep the
            // once-a-second press, which is the bonk. Stuck for two seconds
            // (moved less than half a metre) or eight seconds on one block
            // means hard cover is in the way, so the next block is tried.
            let hunt = match (bonk, me_pos, blocks.get(target)) {
                (true, Some(me), Some(&goal)) => {
                    let (dx, dz) = (goal[0] - me[0], goal[1] - me[1]);
                    let dist = dx.hypot(dz);
                    let (since, at) = progress;
                    let moved = (me[0] - at[0]).hypot(me[1] - at[1]);
                    if moved.is_nan() || moved > 0.5 {
                        progress = (Instant::now(), me);
                    }
                    if dist > 0.6
                        && (since.elapsed() > Duration::from_secs(2)
                            || target_since.elapsed() > Duration::from_secs(8))
                    {
                        target = (target + 1) % blocks.len();
                        target_since = Instant::now();
                        progress = (Instant::now(), me);
                    }
                    let mv = if dist > 0.3 {
                        [dx / dist, dz / dist]
                    } else {
                        [0.0, 0.0]
                    };
                    Some((mv, frame.is_multiple_of(20)))
                }
                _ => None,
            };
            let (mv, bonk_press) = hunt.unwrap_or((circle, false));
            send(
                &mut ws,
                &C2S::Input {
                    seq: frame,
                    view_tick: 0,
                    mx: mv[0],
                    my: mv[1],
                    ax: (t * 1.7).cos(),
                    az: (t * 1.7).sin(),
                    pitch: (t * 0.6).sin() * 0.7,
                    fire: !nofire,
                    sprint: hunt.is_none() && (t as u64).is_multiple_of(3),
                    crouch: false,
                    reload: false,
                    // A press, not a level - see the module docs. `jump`
                    // presses every 24 frames; `bonk` only under a block.
                    jump: (jump && frame.is_multiple_of(24)) || bonk_press,
                    shield,
                    melee: false,
                    ads: false,
                },
            );
        }
        if last_ping.elapsed() >= Duration::from_secs(4) {
            last_ping = Instant::now();
            send(&mut ws, &C2S::Ping { nonce: 1 });
        }
        match ws.read() {
            Ok(Message::Text(t)) => match serde_json::from_str::<S2C>(t.as_str()) {
                // Naming the host and build here is what makes a bot run
                // against a tunnel address evidence about WHICH machine
                // answered, not just that something did.
                Ok(S2C::Welcome {
                    host,
                    version,
                    commit,
                    ..
                }) => println!(
                    "wsbot {handle}: welcomed by {} ({})",
                    if host.is_empty() { "<unnamed>" } else { &host },
                    if version.is_empty() && commit.is_empty() {
                        "unstamped build".to_string()
                    } else {
                        format!("{version} {commit}")
                    }
                ),
                Ok(S2C::GameJoined {
                    id,
                    seed,
                    players,
                    map,
                    mode,
                    ..
                }) => {
                    println!(
                        "wsbot {handle}: in the arena as #{id} (map \"{map}\", mode \"{mode}\", seed {seed}, {} players)",
                        players.len()
                    );
                    my_id = Some(id);
                    in_game = true;
                    // The level, rebuilt as a client rebuilds it, and the
                    // blocks whose bottom a standing head reaches at the
                    // apex of a floor jump (v^2 / 2g above the floor).
                    let apex = JUMP_VEL * JUMP_VEL / (2.0 * -GRAVITY);
                    blocks = Level::named(&map, seed)
                        .obstacles
                        .iter()
                        .filter(|o| o.kind == Cover::Loot && o.base <= BODY_H_STAND + apex)
                        .map(|o| {
                            [
                                f32::midpoint(o.min[0], o.max[0]),
                                f32::midpoint(o.min[1], o.max[1]),
                            ]
                        })
                        .collect();
                    if bonk {
                        println!(
                            "wsbot {handle}: hunting {} floor-reachable loot blocks",
                            blocks.len()
                        );
                    }
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
                    if let Some(p) = players.iter().find(|p| Some(p.id) == my_id) {
                        me_pos = Some([p.x, p.z]);
                    }
                }
                Ok(S2C::Kill { killer, victim }) => {
                    kills_seen += 1;
                    let me = my_id.unwrap_or(255);
                    // `killer == victim` is a rocket's own splash (v14): a
                    // self-kill, never a frag.
                    if killer == me && victim == me {
                        println!("wsbot {handle}: blew itself up");
                    } else if killer == me {
                        println!("wsbot {handle}: fragged #{victim}!");
                    } else if victim == me {
                        println!("wsbot {handle}: fragged by #{killer}");
                    }
                }
                Ok(S2C::RoundOver {
                    winner,
                    team,
                    scores,
                }) => {
                    rounds_seen += 1;
                    println!(
                        "wsbot {handle}: round over, {} {winner} wins; scores {scores:?}",
                        if team { "team" } else { "player" }
                    );
                }
                Ok(S2C::Hit { .. }) => hits_seen += 1,
                Ok(S2C::Blast { .. }) => blasts_seen += 1,
                Ok(S2C::Loot {
                    player,
                    block,
                    weapon,
                }) => {
                    loot_seen += 1;
                    if Some(player) == my_id {
                        println!("wsbot {handle}: bonked block {block}, got weapon {weapon}");
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
                // predicate; arena-server does not depend on fire-core.)
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
    let bonk_note = if bonk { " bonk" } else { "" };
    println!(
        "WSBOT OK: states={states} max_players={max_players} bullets_seen={bullets_seen} kills_seen={kills_seen} hits_seen={hits_seen} blasts_seen={blasts_seen} loot_seen={loot_seen} rounds_seen={rounds_seen}{modes_note}{bonk_note}"
    );
}
