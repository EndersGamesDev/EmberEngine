//! In-process end-to-end test: real TCP, real threads, two scripted clients.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use ember_net::{ClientMsg, PROTOCOL_VERSION, PlayerId, ServerMsg, read_msg, write_msg};
use ember_server::ServerConfig;

fn start_server() -> u16 {
    start_server_with(ServerConfig {
        max_players: 8,
        ..Default::default()
    })
}

fn start_server_with(cfg: ServerConfig) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let _ = ember_server::run(listener, cfg);
    });
    port
}

/// Like `connect`, but returns `None` when the server refuses the
/// connection instead of panicking — the admission caps are tested by
/// asserting on which side of that line a connection lands.
fn try_join(port: u16, name: &str) -> Option<TcpStream> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_nodelay(true).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write_msg(
        &mut stream,
        &ClientMsg::Hello {
            protocol: PROTOCOL_VERSION,
            name: name.into(),
        },
    )
    .ok()?;
    match read_msg::<_, ServerMsg>(&mut stream) {
        Ok(ServerMsg::Welcome { .. }) => Some(stream),
        _ => None,
    }
}

/// True once the server has ENDED this stream (EOF or reset), false if it
/// is merely quiet. The distinction matters: a read timeout is not evidence
/// of a disconnect, and treating it as one would make every cap test pass
/// against a server that enforces nothing.
fn stream_ended(s: &mut TcpStream, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
    let mut buf = [0u8; 512];
    while Instant::now() < deadline {
        match s.read(&mut buf) {
            Ok(0) => return true, // clean EOF
            Ok(_) => {}           // still being served
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return true, // reset
        }
    }
    false
}

fn connect(port: u16, name: &str) -> (TcpStream, PlayerId, usize) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_nodelay(true).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write_msg(
        &mut stream,
        &ClientMsg::Hello {
            protocol: PROTOCOL_VERSION,
            name: name.into(),
        },
    )
    .unwrap();
    match read_msg::<_, ServerMsg>(&mut stream).unwrap() {
        ServerMsg::Welcome { id, roster, .. } => (stream, id, roster.len()),
        other => panic!("expected Welcome, got {other:?}"),
    }
}

#[test]
fn two_players_see_each_other_move() {
    let port = start_server();

    let (mut a, a_id, a_roster) = connect(port, "alice");
    assert_eq!(a_roster, 1, "first player sees only itself in the roster");
    let (mut b, b_id, b_roster) = connect(port, "bob");
    assert_ne!(a_id, b_id);
    assert_eq!(b_roster, 2, "second player sees both in the roster");

    // A walks +x; B watches A move through snapshots.
    write_msg(
        &mut a,
        &ClientMsg::Input {
            move_dir: [1.0, 0.0],
        },
    )
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut first_x: Option<f32> = None;
    let mut moved = false;
    let mut b_saw_join = false;
    while Instant::now() < deadline && !moved {
        match read_msg::<_, ServerMsg>(&mut b).unwrap() {
            ServerMsg::Snapshot { players, .. } => {
                assert!(players.len() <= 2);
                if let Some(pa) = players.iter().find(|p| p.id == a_id) {
                    let x = pa.pos[0];
                    let fx = *first_x.get_or_insert(x);
                    if x - fx > 2.0 {
                        moved = true;
                    }
                }
            }
            ServerMsg::PlayerJoined { .. } => b_saw_join = true,
            _ => {}
        }
    }
    assert!(moved, "B never saw A move (+x) in snapshots");
    // B joined after A, so B should NOT have gotten a PlayerJoined for A
    // (A was in B's Welcome roster instead). A join event would mean the
    // roster/broadcast split is wrong.
    assert!(
        !b_saw_join,
        "B incorrectly received PlayerJoined for a pre-existing player"
    );

    // A watches B leave.
    write_msg(&mut b, &ClientMsg::Bye).unwrap();
    drop(b);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_left = false;
    while Instant::now() < deadline && !saw_left {
        if let ServerMsg::PlayerLeft { id } = read_msg::<_, ServerMsg>(&mut a).unwrap() {
            assert_eq!(id, b_id);
            saw_left = true;
        }
    }
    assert!(saw_left, "A never received PlayerLeft for B");
}

#[test]
fn protocol_mismatch_is_rejected() {
    let port = start_server();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write_msg(
        &mut s,
        &ClientMsg::Hello {
            protocol: 9999,
            name: "x".into(),
        },
    )
    .unwrap();
    match read_msg::<_, ServerMsg>(&mut s).unwrap() {
        ServerMsg::Reject { reason } => assert!(reason.contains("protocol")),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn input_before_hello_disconnects() {
    let port = start_server();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write_msg(
        &mut s,
        &ClientMsg::Input {
            move_dir: [1.0, 0.0],
        },
    )
    .unwrap();
    // Server must close the connection: next read hits EOF (or reset).
    let res = read_msg::<_, ServerMsg>(&mut s);
    assert!(res.is_err(), "server should have dropped the connection");
}

#[test]
fn ping_before_hello_parks_no_slot() {
    let port = start_server();

    // A pre-Hello Ping must not be answered, and the connection must not
    // survive it: were the slot held, this many attempts would exhaust the
    // admission cap (max_players * 2 + 16) and lock out every later client.
    let attempts_needed = 8 * 2 + 16 + 1;
    let mut attempts = Vec::new();
    for _ in 0..attempts_needed {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write_msg(&mut s, &ClientMsg::Ping { nonce: 7 }).unwrap();
        assert!(
            read_msg::<_, ServerMsg>(&mut s).is_err(),
            "server answered a pre-Hello Ping instead of dropping the connection"
        );
        // Held open: a parked slot would still be parked at the check below.
        attempts.push(s);
    }
    assert_eq!(attempts.len(), attempts_needed);

    // Every slot came back, so a real client still gets in — and none of the
    // pingers was ever admitted as a player.
    let (_late, _id, roster) = connect(port, "late");
    assert_eq!(roster, 1, "a pre-Hello pinger was admitted as a player");
}

#[test]
fn message_flood_disconnects_the_client() {
    let port = start_server();
    let mut s = try_join(port, "flood").expect("the flooder must be admitted first");

    // Far more than the burst ceiling can absorb, sent as fast as the socket
    // accepts them. The budget refills at a fixed rate per tick, so no
    // honest client can produce this shape.
    for _ in 0..4000 {
        if write_msg(
            &mut s,
            &ClientMsg::Input {
                move_dir: [0.0, 1.0],
            },
        )
        .is_err()
        {
            break; // the server closed on us mid-flood — that IS the drop
        }
    }

    assert!(
        stream_ended(&mut s, Duration::from_secs(5)),
        "server kept serving a client that sent 4000 messages in one burst"
    );
}

#[test]
fn a_steady_client_is_not_mistaken_for_a_flood() {
    let port = start_server();
    let mut s = try_join(port, "steady").expect("join");

    // ~125 Hz of Input for two seconds: double a 60 Hz client's rate, and
    // several times the burst ceiling in total volume. The cap is a RATE,
    // so this must survive — without this control the test above would
    // pass against a server that drops every client.
    for _ in 0..250 {
        write_msg(
            &mut s,
            &ClientMsg::Input {
                move_dir: [1.0, 0.0],
            },
        )
        .expect("server dropped a client sending at an honest rate");
        std::thread::sleep(Duration::from_millis(8));
    }

    assert!(
        !stream_ended(&mut s, Duration::from_millis(500)),
        "server dropped a client sending at roughly twice the tick rate"
    );
}

#[test]
fn the_per_ip_cap_refuses_the_surplus_connection() {
    let port = start_server_with(ServerConfig {
        max_players: 8,
        max_conns_per_ip: 3,
        cap_loopback: true,
        ..Default::default()
    });

    // Held open for the whole test: the cap counts LIVE connections.
    let held: Vec<TcpStream> = (0..3)
        .map(|i| try_join(port, &format!("p{i}")).expect("a connection below the cap was refused"))
        .collect();

    assert!(
        try_join(port, "surplus").is_none(),
        "a 4th connection from one IP was admitted under a cap of 3"
    );
    drop(held);
}

#[test]
fn the_per_ip_cap_exempts_loopback_by_default() {
    // Identical to the test above except for `cap_loopback`, which is off in
    // the deployment: the server binds to the WireGuard address, so a
    // loopback peer is local tooling rather than a stranger. Without the
    // exemption the 4th join here would be refused.
    let port = start_server_with(ServerConfig {
        max_players: 8,
        max_conns_per_ip: 3,
        ..Default::default()
    });

    let mut held = Vec::new();
    for i in 0..4 {
        held.push(
            try_join(port, &format!("p{i}")).expect("loopback was capped despite the exemption"),
        );
    }
    assert_eq!(held.len(), 4);
}

#[test]
fn the_per_ip_cap_releases_a_slot_when_a_connection_closes() {
    let port = start_server_with(ServerConfig {
        max_players: 8,
        max_conns_per_ip: 2,
        cap_loopback: true,
        ..Default::default()
    });

    let a = try_join(port, "a").expect("first connection");
    let b = try_join(port, "b").expect("second connection");
    assert!(try_join(port, "c").is_none(), "a cap of 2 admitted a third");

    // The count is derived from the live connection map, so closing one must
    // free exactly one slot — there is no side table that could leak.
    drop(b);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut readmitted = None;
    while Instant::now() < deadline && readmitted.is_none() {
        std::thread::sleep(Duration::from_millis(50));
        readmitted = try_join(port, "c2");
    }
    assert!(
        readmitted.is_some(),
        "no per-ip slot was released after a connection closed"
    );
    drop(a);
}

#[test]
fn a_dribbled_frame_hits_the_frame_deadline() {
    let port = start_server_with(ServerConfig {
        max_players: 8,
        frame_deadline: Duration::from_millis(600),
        ..Default::default()
    });

    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_nodelay(true).unwrap();
    s.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    // A length header promising a frame that never completes, then one byte
    // at a time — faster than any per-read socket timeout would fire. This
    // is precisely the shape a read timeout cannot stop, because read_exact
    // restarts it on every byte that arrives.
    s.write_all(&1024u32.to_le_bytes()).unwrap();
    s.flush().unwrap();

    let started = Instant::now();
    let mut ended = false;
    let mut buf = [0u8; 64];
    while started.elapsed() < Duration::from_secs(8) && !ended {
        if s.write_all(&[0u8]).is_err() || s.flush().is_err() {
            ended = true;
            break;
        }
        match s.read(&mut buf) {
            Ok(0) => ended = true,
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => ended = true,
        }
    }

    assert!(ended, "a byte-dribbling peer was never disconnected");
    // Attributes the kill to the frame deadline and not to the sim thread's
    // CLIENT_TIMEOUT_SECS sweep, which is an order of magnitude later and
    // would eventually reap this connection too.
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the dribbler outlived its 600 ms frame deadline by too much to \
         credit the deadline for the kill"
    );
}

#[test]
fn a_client_sending_complete_frames_outlives_the_frame_deadline() {
    let port = start_server_with(ServerConfig {
        max_players: 8,
        frame_deadline: Duration::from_millis(600),
        ..Default::default()
    });

    let mut s = try_join(port, "prompt").expect("join");
    // Six frame deadlines of life, with complete frames arriving throughout:
    // the deadline bounds ONE message in flight, not the connection.
    for _ in 0..24 {
        write_msg(
            &mut s,
            &ClientMsg::Input {
                move_dir: [0.0, 1.0],
            },
        )
        .expect("the frame deadline killed a client sending complete frames");
        std::thread::sleep(Duration::from_millis(150));
    }

    assert!(
        !stream_ended(&mut s, Duration::from_millis(500)),
        "a client sending complete frames was killed by the frame deadline"
    );
}
