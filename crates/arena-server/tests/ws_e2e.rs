//! End-to-end over a real `WebSocket`: create a passworded game, reject a bad
//! password, drop a second player in, shoot, and drop out.

use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use arena_core::proto::{C2S, PROTO_VERSION, S2C};
use arena_core::shooter::{GameMode, HILL_FREE, MAP_FREIGHT_YARD, MAP_HARBOR, MAP_TRENCH_CITY};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

type Ws = WebSocket<MaybeTlsStream<TcpStream>>;

#[test]
fn harbor_advertises_real_bounds_and_eight_distinct_team_starts() {
    let port = start_server();
    let mut clients = Vec::new();
    for slot in 0..8 {
        let mut ws = connect(port, &format!("harbor-{slot}"));
        let request = if slot == 0 {
            C2S::CreateLobby {
                name: "harbor-eight".into(),
                password: Some("fixture".into()),
                map: MAP_HARBOR.into(),
                mode: "tdm".into(),
            }
        } else {
            C2S::JoinLobby {
                name: "harbor-eight".into(),
                password: Some("fixture".into()),
            }
        };
        send(&mut ws, &request);
        recv_until(&mut ws, 5, |m| match m {
            S2C::GameJoined {
                arena_half,
                map,
                mode,
                players,
                ..
            } => {
                assert_eq!(
                    arena_half, 48.0,
                    "creator and joiners get the simulation bounds"
                );
                assert_eq!(map, MAP_HARBOR);
                assert_eq!(mode, "tdm");
                assert_eq!(players.len(), slot + 1);
                Some(())
            }
            _ => None,
        });
        clients.push(ws);
    }
    recv_until(clients.last_mut().unwrap(), 5, |m| match m {
        S2C::State { players, .. } if players.len() == 8 => {
            for team in [0, 1] {
                assert_eq!(players.iter().filter(|p| p.team == team).count(), 4);
            }
            for (i, a) in players.iter().enumerate() {
                assert!(
                    a.z.abs() > 24.0,
                    "real starts are beyond the old arena boundary"
                );
                assert!(arena_core::harbor::SPAWNS.contains(&[a.x, a.z]));
                assert_eq!(a.z > 0.0, a.team == 0);
                for b in &players[i + 1..] {
                    assert!(
                        (a.x - b.x).hypot(a.z - b.z) > 1.2,
                        "overlapping starts: {a:?} / {b:?}"
                    );
                }
            }
            Some(())
        }
        _ => None,
    });
    let mut ninth = connect(port, "ninth");
    send(
        &mut ninth,
        &C2S::JoinLobby {
            name: "harbor-eight".into(),
            password: Some("fixture".into()),
        },
    );
    recv_until(&mut ninth, 5, |m| match m {
        S2C::Error { message } => {
            assert!(message.contains("full"), "{message}");
            Some(())
        }
        _ => None,
    });
}

fn start_server() -> u16 {
    start_named_server(String::new())
}

fn start_named_server(host_name: String) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        drop(arena_server::run(
            listener,
            arena_server::ServerConfig {
                host_name,
                ..Default::default()
            },
        ));
    });
    port
}

fn connect(port: u16, handle: &str) -> Ws {
    let (mut ws, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(
        &mut ws,
        &C2S::Hello {
            proto: PROTO_VERSION,
            handle: handle.into(),
        },
    );
    match recv(&mut ws) {
        S2C::Welcome { .. } => ws,
        other => panic!("expected Welcome, got {other:?}"),
    }
}

fn send(ws: &mut Ws, msg: &C2S) {
    ws.send(Message::text(serde_json::to_string(msg).unwrap()))
        .unwrap();
}

fn recv(ws: &mut Ws) -> S2C {
    loop {
        match ws.read().unwrap() {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected frame: {other:?}"),
        }
    }
}

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
// The scenario stays linear so each protocol transition is asserted in wire order.
#[allow(clippy::too_many_lines)]
fn drop_in_arena_flow_with_password() {
    let port = start_server();

    // Host creates a passworded game and is inside it immediately.
    let mut host = connect(port, "alice");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "arena".into(),
            password: Some("s3cret".into()),
            map: MAP_TRENCH_CITY.into(),
            mode: String::new(),
        },
    );
    let (host_pid, seed) = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined {
            id,
            seed,
            players,
            map,
            ..
        } => {
            assert_eq!(players.len(), 1);
            // The lobby names its level: a client that built the seeded
            // arena from `seed` alone would be predicting against boxes
            // the server does not have.
            assert_eq!(map, MAP_TRENCH_CITY);
            Some((id, seed))
        }
        _ => None,
    });

    // Guest sees it listed with player count and the password flag.
    let mut guest = connect(port, "bob");
    send(&mut guest, &C2S::ListLobbies);
    match recv(&mut guest) {
        S2C::LobbyList { lobbies } => {
            assert_eq!(lobbies.len(), 1);
            assert_eq!(lobbies[0].name, "arena");
            assert_eq!(lobbies[0].host, "alice");
            assert!(lobbies[0].has_password);
            assert_eq!((lobbies[0].players, lobbies[0].cap), (1, 8));
        }
        other => panic!("expected LobbyList, got {other:?}"),
    }

    // Wrong password rejected without disconnecting.
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "arena".into(),
            password: Some("nope".into()),
        },
    );
    match recv(&mut guest) {
        S2C::Error { message } => assert!(message.contains("password"), "{message}"),
        other => panic!("expected Error, got {other:?}"),
    }

    // Correct password drops the guest into the SAME arena (same map, same
    // seed).
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "arena".into(),
            password: Some("s3cret".into()),
        },
    );
    let (guest_pid, guest_seed) = recv_until(&mut guest, 5, |m| match m {
        S2C::GameJoined {
            id,
            seed,
            players,
            map,
            ..
        } => {
            assert_eq!(players.len(), 2, "joiner sees the full roster");
            assert_eq!(map, MAP_TRENCH_CITY, "the joiner is told the same level");
            Some((id, seed))
        }
        _ => None,
    });
    assert_eq!(guest_seed, seed, "all players share the arena seed");
    assert_ne!(guest_pid, host_pid);
    recv_until(&mut host, 5, |m| match m {
        S2C::PlayerJoined { meta } => {
            assert_eq!(meta.handle, "bob");
            Some(())
        }
        _ => None,
    });

    // Guest holds fire: bullets must appear in the state stream.
    send(
        &mut guest,
        &C2S::Input {
            seq: 1,
            view_tick: 0,
            mx: 0.0,
            my: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            fire: true,
            sprint: false,
            crouch: false,
            reload: false,
            jump: false,
            shield: false,
            melee: false,
            ads: false,
        },
    );
    // The next state must echo the input's seq back as this player's ack.
    recv_until(&mut guest, 5, |m| match m {
        S2C::State { players, .. } => players
            .iter()
            .find(|p| p.id == guest_pid)
            .filter(|p| p.ack == 1)
            .map(|_| ()),
        _ => None,
    });
    recv_until(&mut host, 5, |m| match m {
        S2C::State {
            bullets, players, ..
        } => {
            assert_eq!(players.len(), 2);
            (!bullets.is_empty()).then_some(())
        }
        _ => None,
    });

    // Guest leaves; host is told and the game keeps running.
    drop(guest);
    recv_until(&mut host, 10, |m| match m {
        S2C::PlayerLeft { id } => {
            assert_eq!(id, guest_pid);
            Some(())
        }
        _ => None,
    });
    // Host still receives states alone.
    recv_until(&mut host, 5, |m| match m {
        S2C::State { players, .. } => (players.len() == 1).then_some(()),
        _ => None,
    });
}

#[test]
fn a_shot_event_reaches_every_member_the_tick_the_round_ends() {
    // v20: the guest holds fire, and both members are told where each
    // round ended as `S2C::Shot`, with the guest as its owner. The round
    // lives a fifth of a second, so the event and not the state stream is
    // how a peer learns a shot happened at all.
    let port = start_server();
    let mut host = connect(port, "alice");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "range".into(),
            password: None,
            map: MAP_FREIGHT_YARD.into(),
            mode: String::new(),
        },
    );
    recv_until(&mut host, 5, |m| {
        matches!(m, S2C::GameJoined { .. }).then_some(())
    });
    let mut guest = connect(port, "bob");
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "range".into(),
            password: None,
        },
    );
    let guest_pid = recv_until(&mut guest, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });
    send(
        &mut guest,
        &C2S::Input {
            seq: 1,
            view_tick: 0,
            mx: 0.0,
            my: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            fire: true,
            sprint: false,
            crouch: false,
            reload: false,
            jump: false,
            shield: false,
            melee: false,
            ads: false,
        },
    );
    let check = |m: S2C| match m {
        S2C::Shot {
            owner,
            weapon,
            x0,
            y0,
            x1,
            y1,
            hit,
            ..
        } if owner == guest_pid => {
            assert_eq!(weapon, arena_core::shooter::SIDEARM);
            assert!(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite());
            assert!(x1 > x0, "fired along +x: {x0} -> {x1}");
            assert!(hit <= 5, "a kind the sim names: {hit}");
            Some(())
        }
        _ => None,
    };
    recv_until(&mut guest, 5, check);
    recv_until(&mut host, 5, check);
}

#[test]
fn old_proto_may_list_but_not_join() {
    let port = start_server();
    // A current-proto host opens a lobby.
    let mut host = connect(port, "alice");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "arena".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    recv_until(&mut host, 5, |m| {
        matches!(m, S2C::GameJoined { .. }).then_some(())
    });

    // A stale client (or the hub's proto-0 browser) may list...
    let (mut old, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = old.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(
        &mut old,
        &C2S::Hello {
            proto: 0,
            handle: "browser".into(),
        },
    );
    recv_until(&mut old, 5, |m| {
        matches!(m, S2C::Welcome { .. }).then_some(())
    });
    send(&mut old, &C2S::ListLobbies);
    recv_until(&mut old, 5, |m| match m {
        S2C::LobbyList { lobbies } => (lobbies.len() == 1).then_some(()),
        _ => None,
    });

    // ...but entering a game requires the live protocol.
    send(
        &mut old,
        &C2S::JoinLobby {
            name: "arena".into(),
            password: None,
        },
    );
    recv_until(&mut old, 5, |m| match m {
        S2C::Error { message } => {
            assert!(message.contains("live version"), "{message}");
            // The sentence is the whole compatibility story a frozen build's
            // player ever sees, so it must still NAME both versions after a
            // bump — "play the live version" alone leaves them guessing
            // which build theirs is and which one is live.
            assert!(
                message.contains("v0"),
                "must name the stale version: {message}"
            );
            assert!(
                message.contains(&format!("v{PROTO_VERSION}")),
                "must name the live version: {message}"
            );
            Some(())
        }
        _ => None,
    });
}

/// A lobby names its map, in the listing and in `GameJoined`, and an empty
/// name resolves to the current default rather than to nothing.
#[test]
fn a_lobby_lists_its_map() {
    let port = start_server();

    // No map named: the yard, and the joiner is told so.
    let mut alice = connect(port, "alice");
    send(
        &mut alice,
        &C2S::CreateLobby {
            name: "default".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    recv_until(&mut alice, 5, |m| match m {
        S2C::GameJoined { map, .. } => {
            assert_eq!(map, MAP_FREIGHT_YARD, "an empty map is the yard");
            Some(())
        }
        _ => None,
    });

    // Named: Trench City, exactly as asked.
    let mut bob = connect(port, "bob");
    send(
        &mut bob,
        &C2S::CreateLobby {
            name: "trench".into(),
            password: None,
            map: MAP_TRENCH_CITY.into(),
            mode: String::new(),
        },
    );
    recv_until(&mut bob, 5, |m| match m {
        S2C::GameJoined { map, .. } => {
            assert_eq!(map, MAP_TRENCH_CITY);
            Some(())
        }
        _ => None,
    });

    // The listing carries both, so a browser can show the map before
    // joining.
    let mut carol = connect(port, "carol");
    send(&mut carol, &C2S::ListLobbies);
    match recv(&mut carol) {
        S2C::LobbyList { lobbies } => {
            assert_eq!(lobbies.len(), 2);
            let map_of = |name: &str| {
                lobbies
                    .iter()
                    .find(|l| l.name == name)
                    .unwrap_or_else(|| panic!("no lobby {name}"))
                    .map
                    .clone()
            };
            assert_eq!(map_of("default"), MAP_FREIGHT_YARD);
            assert_eq!(map_of("trench"), MAP_TRENCH_CITY);
        }
        other => panic!("expected LobbyList, got {other:?}"),
    }

    // A joiner is told the level the lobby was created with.
    send(
        &mut carol,
        &C2S::JoinLobby {
            name: "trench".into(),
            password: None,
        },
    );
    recv_until(&mut carol, 5, |m| match m {
        S2C::GameJoined { map, .. } => {
            assert_eq!(map, MAP_TRENCH_CITY, "the joiner rebuilds the same level");
            Some(())
        }
        _ => None,
    });
}

/// A map name that is no level is refused, never silently seeded: a page
/// with a typo must be told, and the connection stays open to try again.
#[test]
fn an_unknown_map_is_refused() {
    let port = start_server();
    let mut alice = connect(port, "alice");
    send(
        &mut alice,
        &C2S::CreateLobby {
            name: "moon".into(),
            password: None,
            map: "moon-base".into(),
            mode: String::new(),
        },
    );
    match recv(&mut alice) {
        S2C::Error { message } => assert!(message.contains("unknown map"), "{message}"),
        other => panic!("expected Error, got {other:?}"),
    }
    // Nothing was created, and the same connection may still create.
    send(&mut alice, &C2S::ListLobbies);
    match recv(&mut alice) {
        S2C::LobbyList { lobbies } => assert!(lobbies.is_empty(), "{lobbies:?}"),
        other => panic!("expected LobbyList, got {other:?}"),
    }
    send(
        &mut alice,
        &C2S::CreateLobby {
            name: "moon".into(),
            password: None,
            map: MAP_FREIGHT_YARD.into(),
            mode: String::new(),
        },
    );
    recv_until(&mut alice, 5, |m| {
        matches!(m, S2C::GameJoined { .. }).then_some(())
    });
}

#[test]
fn message_before_hello_disconnects() {
    let port = start_server();
    let (mut ws, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(&mut ws, &C2S::ListLobbies);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "server never closed the connection"
        );
        match ws.read() {
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

/// The wire half of the jump-reconciliation fix: a state that puts a player
/// in the air must carry the velocity that put them there. `y` alone cannot
/// restart the client's integrator - it replays gravity from `vy`, and a
/// state without it silently seeds that replay with zero.
#[test]
fn an_airborne_state_carries_the_velocity_that_made_it() {
    let port = start_server();
    let mut host = connect(port, "jumper");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "jump-wire".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });

    // Hold Space. One press is enough to leave the ground and the arc lasts
    // ~0.7 s, so a handful of inputs covers it. Paced like a real client:
    // faster than one message a tick and the server drops us as a flooder,
    // which is exactly what the first version of this test did.
    for seq in 1..6 {
        send(
            &mut host,
            &C2S::Input {
                seq,
                view_tick: 0,
                mx: 0.0,
                my: 0.0,
                ax: 1.0,
                az: 0.0,
                pitch: 0.0,
                fire: false,
                sprint: false,
                crouch: false,
                reload: false,
                jump: true,
                shield: false,
                melee: false,
                ads: false,
            },
        );
        std::thread::sleep(Duration::from_millis(40));
    }

    let (y, vy) = recv_until(&mut host, 5, |m| match m {
        S2C::State { players, .. } => players
            .iter()
            .find(|p| p.id == me && p.y > 0.5)
            .map(|p| (p.y, p.vy)),
        _ => None,
    });
    assert!(
        vy != 0.0,
        "airborne at y={y} with vy={vy}: the state dropped the velocity"
    );
}

/// Jump is a press the sim consumes once, not a held key it re-applies. The
/// server keeps re-running the last input it received every tick, so a set
/// jump flag used to re-launch the player off every surface they landed on -
/// including a crate top, from which the second launch clears the containers
/// that are supposed to be unreachable hard cover.
#[test]
fn one_jump_press_launches_once_and_does_not_bunny_hop() {
    let port = start_server();
    let mut host = connect(port, "hopper");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "hop-once".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });

    // Exactly one input, with the press set. Nothing is sent afterwards, so
    // anything that happens next is the server re-applying what it holds.
    send(
        &mut host,
        &C2S::Input {
            seq: 1,
            view_tick: 0,
            mx: 0.0,
            my: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            fire: false,
            sprint: false,
            crouch: false,
            reload: false,
            jump: true,
            shield: false,
            melee: false,
            ads: false,
        },
    );

    let y_of = |m: S2C, me: u8| match m {
        S2C::State { players, .. } => players.iter().find(|p| p.id == me).map(|p| p.y),
        _ => None,
    };
    // The one press must actually leave the ground.
    let peak = recv_until(&mut host, 5, |m| y_of(m, me).filter(|y| *y > 0.5));
    assert!(peak > 0.5, "the press never launched: {peak}");

    // Then land, and stay landed. A full arc is ~0.7 s; watch twice that.
    let mut landed = false;
    let deadline = Instant::now() + Duration::from_millis(1600);
    while Instant::now() < deadline {
        if let Some(y) = y_of(recv(&mut host), me) {
            if landed {
                assert!(y < 0.2, "re-launched without a new press: y={y}");
            } else if y < 0.05 {
                landed = true;
            }
        }
    }
    assert!(landed, "never came back down");
}

/// The state has to say how long the server has been applying the command it
/// acks, or the client cannot place the state on its own clock and guesses
/// the replay window from its send cadence instead.
#[test]
fn a_state_reports_how_long_the_acked_command_has_been_applied() {
    let port = start_server();
    let mut host = connect(port, "acker");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "ack-age".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });
    send(
        &mut host,
        &C2S::Input {
            seq: 9,
            view_tick: 0,
            mx: 1.0,
            my: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            fire: false,
            sprint: false,
            crouch: false,
            reload: false,
            jump: false,
            shield: false,
            melee: false,
            ads: false,
        },
    );

    // Nothing else is sent, so the age of seq 9 must climb tick by tick.
    let mut ages = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while ages.len() < 4 && Instant::now() < deadline {
        if let S2C::State { players, .. } = recv(&mut host)
            && let Some(p) = players.iter().find(|p| p.id == me && p.ack == 9)
        {
            ages.push(p.ack_age_ticks);
        }
    }
    assert!(
        ages.len() >= 4,
        "not enough states carrying the ack: {ages:?}"
    );
    assert!(
        ages.windows(2).all(|w| w[1] > w[0]),
        "ack_age_ticks did not advance: {ages:?}"
    );
}

/// A press is an event, so it has to survive being overtaken. Two input
/// frames can land in the same inter-tick window - the hub drains every
/// queued event before it steps - and storing the second on top of the first
/// used to destroy the press before the sim ever ran. Under the old held-key
/// meaning that was free, because the next packet still carried the key down.
#[test]
fn a_press_survives_a_second_input_in_the_same_tick() {
    let port = start_server();
    let mut host = connect(port, "coalesced");
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "coalesce".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    let me = recv_until(&mut host, 5, |m| match m {
        S2C::GameJoined { id, .. } => Some(id),
        _ => None,
    });

    let input = |seq: u32, jump: bool| C2S::Input {
        seq,
        view_tick: 0,
        mx: 0.0,
        my: 0.0,
        ax: 1.0,
        az: 0.0,
        pitch: 0.0,
        fire: false,
        sprint: false,
        crouch: false,
        reload: false,
        jump,
        shield: false,
        melee: false,
        ads: false,
    };
    // Back to back, deliberately with no sleep: the press and the packet that
    // overtakes it reach the hub inside one 16.7 ms window.
    send(&mut host, &input(1, true));
    send(&mut host, &input(2, false));

    let y = recv_until(&mut host, 5, |m| match m {
        S2C::State { players, .. } => players
            .iter()
            .find(|p| p.id == me && p.y > 0.5)
            .map(|p| p.y),
        _ => None,
    });
    assert!(
        y > 0.5,
        "the press was overwritten before the sim saw it: {y}"
    );
}

/// Hello, then the RAW `Welcome` frame. The text and not just the decoded
/// message, because the wire key names are the contract `web/hosts.js` and
/// the address book read, and no Rust type checks those.
fn connect_raw_welcome(port: u16, handle: &str) -> (Ws, String) {
    let (mut ws, _) = tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    send(
        &mut ws,
        &C2S::Hello {
            proto: PROTO_VERSION,
            handle: handle.into(),
        },
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match ws.read().unwrap() {
            Message::Text(t) => return (ws, t.as_str().to_owned()),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected frame: {other:?}"),
        }
    }
    panic!("no Welcome within 5s");
}

/// A host answers with its own name and its live load, in one round trip —
/// which is the whole reason a page can rank hosts without a second request.
#[test]
fn welcome_names_the_host_and_reports_its_live_load() {
    let port = start_named_server("test-otter".to_string());

    // Nothing running yet: named host, no load.
    let (mut alice, raw) = connect_raw_welcome(port, "alice");
    match serde_json::from_str::<S2C>(&raw).unwrap() {
        S2C::Welcome {
            host,
            players,
            lobbies,
            ..
        } => {
            assert_eq!(host, "test-otter");
            assert_eq!(players, 0, "an idle server has nobody in a game: {raw}");
            assert_eq!(lobbies, 0);
        }
        other => panic!("expected Welcome, got {other:?}"),
    }

    send(
        &mut alice,
        &C2S::CreateLobby {
            name: "arena".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    recv_until(&mut alice, 5, |m| {
        matches!(m, S2C::GameJoined { .. }).then_some(())
    });

    // The next arrival is told what alice is doing. The counts are what
    // "emptiest host wins" is decided on, so a Welcome that kept reporting
    // zero would quietly send every player to the busiest machine.
    let (_bob, raw) = connect_raw_welcome(port, "bob");
    match serde_json::from_str::<S2C>(&raw).unwrap() {
        S2C::Welcome {
            host,
            players,
            lobbies,
            ..
        } => {
            assert_eq!(host, "test-otter");
            assert_eq!(players, 1, "alice is in a game: {raw}");
            assert_eq!(lobbies, 1, "alice's lobby is open: {raw}");
        }
        other => panic!("expected Welcome, got {other:?}"),
    }
}

/// The default configuration is a legal one: an unnamed host whose Welcome a
/// client still decodes, carrying every key by its contract name.
#[test]
fn a_default_server_is_unnamed_and_its_welcome_still_decodes() {
    let port = start_server();
    let (_ws, raw) = connect_raw_welcome(port, "ghost");

    for key in ["host", "version", "commit", "players", "lobbies"] {
        assert!(raw.contains(&format!("\"{key}\"")), "{key} missing: {raw}");
    }
    match serde_json::from_str::<S2C>(&raw).expect("the client must decode this") {
        S2C::Welcome {
            proto,
            host,
            players,
            lobbies,
            ..
        } => {
            assert_eq!(proto, PROTO_VERSION);
            assert_eq!(host, "", "a server started without a name has none");
            assert_eq!((players, lobbies), (0, 0));
        }
        other => panic!("expected Welcome, got {other:?}"),
    }

    // And the other direction: a client of this build against a server that
    // predates the identity fields. Same decode, empty identity.
    let old = format!(r#"{{"t":"welcome","proto":{PROTO_VERSION},"motd":"ember arena"}}"#);
    match serde_json::from_str::<S2C>(&old).expect("an old Welcome must decode") {
        S2C::Welcome { host, players, .. } => {
            assert_eq!(host, "");
            assert_eq!(players, 0);
        }
        other => panic!("expected Welcome, got {other:?}"),
    }
}

/// A lobby names its mode, in the listing and in `GameJoined`, and an empty
/// name resolves to free for all rather than to nothing. A joiner of a
/// team game is put on a team, and the state says which.
#[test]
// Three lobbies, a listing and a join, asserted in wire order like the flow test.
#[allow(clippy::too_many_lines)]
fn a_lobby_lists_its_mode() {
    let port = start_server();

    // No mode named: free for all, and the creator is told so by name.
    let mut alice = connect(port, "alice");
    send(
        &mut alice,
        &C2S::CreateLobby {
            name: "plain".into(),
            password: None,
            map: String::new(),
            mode: String::new(),
        },
    );
    recv_until(&mut alice, 5, |m| match m {
        S2C::GameJoined { mode, .. } => {
            assert_eq!(
                GameMode::from_name(&mode),
                Some(GameMode::Ffa),
                "an empty mode is free for all: {mode:?}"
            );
            Some(())
        }
        _ => None,
    });

    // Named: team deathmatch and king of the hill, exactly as asked.
    let mut bob = connect(port, "bob");
    send(
        &mut bob,
        &C2S::CreateLobby {
            name: "teams".into(),
            password: None,
            map: MAP_TRENCH_CITY.into(),
            mode: GameMode::Tdm.name().into(),
        },
    );
    recv_until(&mut bob, 5, |m| match m {
        S2C::GameJoined { mode, map, .. } => {
            assert_eq!(mode, GameMode::Tdm.name());
            assert_eq!(map, MAP_TRENCH_CITY);
            Some(())
        }
        _ => None,
    });
    let mut carol = connect(port, "carol");
    send(
        &mut carol,
        &C2S::CreateLobby {
            name: "king".into(),
            password: None,
            map: MAP_FREIGHT_YARD.into(),
            mode: GameMode::Hill.name().into(),
        },
    );
    recv_until(&mut carol, 5, |m| match m {
        S2C::GameJoined { mode, .. } => {
            assert_eq!(mode, GameMode::Hill.name());
            Some(())
        }
        _ => None,
    });

    // The listing carries all three, so a browser can show the mode pill
    // beside the map pill before joining.
    let mut dave = connect(port, "dave");
    send(&mut dave, &C2S::ListLobbies);
    match recv(&mut dave) {
        S2C::LobbyList { lobbies } => {
            assert_eq!(lobbies.len(), 3);
            let mode_of = |name: &str| {
                lobbies
                    .iter()
                    .find(|l| l.name == name)
                    .unwrap_or_else(|| panic!("no lobby {name}"))
                    .mode
                    .clone()
            };
            assert_eq!(mode_of("plain"), GameMode::Ffa.name());
            assert_eq!(mode_of("teams"), GameMode::Tdm.name());
            assert_eq!(mode_of("king"), GameMode::Hill.name());
        }
        other => panic!("expected LobbyList, got {other:?}"),
    }

    // A joiner is told the mode the lobby was created with, and in team
    // deathmatch lands on the other side from the creator.
    send(
        &mut dave,
        &C2S::JoinLobby {
            name: "teams".into(),
            password: None,
        },
    );
    let me = recv_until(&mut dave, 5, |m| match m {
        S2C::GameJoined { id, mode, .. } => {
            assert_eq!(
                mode,
                GameMode::Tdm.name(),
                "the joiner builds the same rules"
            );
            Some(id)
        }
        _ => None,
    });
    recv_until(&mut dave, 5, |m| match m {
        S2C::State {
            players,
            team_score,
            hill,
            round_pause,
            ..
        } => {
            assert_eq!(players.len(), 2);
            let mine = players
                .iter()
                .find(|p| p.id == me)
                .expect("I am in the state");
            let theirs = players
                .iter()
                .find(|p| p.id != me)
                .expect("bob is in the state");
            assert_ne!(
                mine.team, theirs.team,
                "two players in a team game face each other"
            );
            assert_eq!(team_score, [0, 0], "nobody has scored");
            assert_eq!(hill, HILL_FREE, "no hill in team deathmatch");
            assert_eq!(round_pause, 0.0, "the round is running");
            Some(())
        }
        _ => None,
    });
}

/// A mode name that is no mode is refused, never silently played as free
/// for all: a page with a typo must be told, and the connection stays open
/// to try again.
#[test]
fn an_unknown_mode_is_refused() {
    let port = start_server();
    let mut alice = connect(port, "alice");
    send(
        &mut alice,
        &C2S::CreateLobby {
            name: "flags".into(),
            password: None,
            map: String::new(),
            mode: "ctf".into(),
        },
    );
    match recv(&mut alice) {
        S2C::Error { message } => assert!(message.contains("unknown mode"), "{message}"),
        other => panic!("expected Error, got {other:?}"),
    }
    // Nothing was created, and the same connection may still create.
    send(&mut alice, &C2S::ListLobbies);
    match recv(&mut alice) {
        S2C::LobbyList { lobbies } => assert!(lobbies.is_empty(), "{lobbies:?}"),
        other => panic!("expected LobbyList, got {other:?}"),
    }
    send(
        &mut alice,
        &C2S::CreateLobby {
            name: "flags".into(),
            password: None,
            map: String::new(),
            mode: GameMode::Ffa.name().into(),
        },
    );
    recv_until(&mut alice, 5, |m| {
        matches!(m, S2C::GameJoined { .. }).then_some(())
    });
}
