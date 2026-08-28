//! In-process end-to-end test: real TCP, real threads, two scripted clients.

use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use ember_net::{read_msg, write_msg, ClientMsg, PlayerId, ServerMsg, PROTOCOL_VERSION};

fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let _ = ember_server::run(listener, ember_server::ServerConfig { max_players: 8 });
    });
    port
}

fn connect(port: u16, name: &str) -> (TcpStream, PlayerId, usize) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_nodelay(true).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write_msg(
        &mut stream,
        &ClientMsg::Hello { protocol: PROTOCOL_VERSION, name: name.into() },
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
    write_msg(&mut a, &ClientMsg::Input { move_dir: [1.0, 0.0] }).unwrap();

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
    assert!(!b_saw_join, "B incorrectly received PlayerJoined for a pre-existing player");

    // A watches B leave.
    write_msg(&mut b, &ClientMsg::Bye).unwrap();
    drop(b);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_left = false;
    while Instant::now() < deadline && !saw_left {
        match read_msg::<_, ServerMsg>(&mut a).unwrap() {
            ServerMsg::PlayerLeft { id } => {
                assert_eq!(id, b_id);
                saw_left = true;
            }
            _ => {}
        }
    }
    assert!(saw_left, "A never received PlayerLeft for B");
}

#[test]
fn protocol_mismatch_is_rejected() {
    let port = start_server();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write_msg(&mut s, &ClientMsg::Hello { protocol: 9999, name: "x".into() }).unwrap();
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
    write_msg(&mut s, &ClientMsg::Input { move_dir: [1.0, 0.0] }).unwrap();
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
    let mut attempts = Vec::new();
    for _ in 0..(8 * 2 + 16 + 1) {
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

    // Every slot came back, so a real client still gets in — and none of the
    // pingers was ever admitted as a player.
    let (_late, _id, roster) = connect(port, "late");
    assert_eq!(roster, 1, "a pre-Hello pinger was admitted as a player");
}
