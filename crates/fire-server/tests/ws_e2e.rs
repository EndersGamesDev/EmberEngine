//! End-to-end: start a real server, connect real WebSocket clients, and race.
//!
//! The unit tests reach inside the hub. This one does not — it speaks the
//! wire, which is the only way to prove the thing is actually *joinable*.

use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use fire_core::proto::{self, C2S, Phase, S2C};
use tungstenite::{Message, WebSocket, connect};

type Client = WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// Non-blocking-ish reads. `MaybeTlsStream` does not forward socket options,
/// so reach through to the plain stream — these tests never use TLS.
fn set_read_timeout(ws: &Client, d: Duration) {
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_ref() {
        let _ = s.set_read_timeout(Some(d));
    }
}

fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let _ = fire_server::run(
            listener,
            fire_server::ServerConfig {
                laps: 1,
                max_lobbies: 8,
            },
        );
    });
    // Give the accept loop a moment to come up.
    thread::sleep(Duration::from_millis(150));
    port
}

fn client(port: u16, handle: &str) -> Client {
    let (mut ws, _) = connect(format!("ws://127.0.0.1:{port}")).expect("connect");
    set_read_timeout(&ws, Duration::from_millis(50));
    send(
        &mut ws,
        &C2S::Hello {
            proto: proto::PROTO_VERSION,
            handle: handle.into(),
        },
    );
    ws
}

fn send(ws: &mut Client, msg: &C2S) {
    ws.send(Message::text(serde_json::to_string(msg).unwrap()))
        .expect("send");
}

/// Pump messages for `dur`, handing each to `f`. Returns when `f` says stop.
fn pump(ws: &mut Client, dur: Duration, mut f: impl FnMut(S2C) -> bool) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < dur {
        match ws.read() {
            Ok(Message::Text(t)) => {
                if let Ok(m) = serde_json::from_str::<S2C>(&t)
                    && f(m)
                {
                    return true;
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(_) => break,
        }
    }
    false
}

#[test]
fn two_players_join_a_lobby_and_race() {
    let port = start_server();
    let mut a = client(port, "alice");
    let mut b = client(port, "bob");

    assert!(
        pump(&mut a, Duration::from_secs(2), |m| matches!(
            m,
            S2C::Welcome { .. }
        )),
        "no welcome for the first client"
    );
    assert!(
        pump(&mut b, Duration::from_secs(2), |m| matches!(
            m,
            S2C::Welcome { .. }
        )),
        "no welcome for the second client"
    );

    // Alice opens a lobby.
    send(
        &mut a,
        &C2S::CreateLobby {
            name: "castle".into(),
            password: None,
        },
    );
    let mut a_slot = None;
    assert!(
        pump(&mut a, Duration::from_secs(2), |m| {
            if let S2C::Joined { lobby, slot, .. } = m {
                assert_eq!(lobby, "castle");
                a_slot = Some(slot);
                return true;
            }
            false
        }),
        "creator never got Joined"
    );

    // It shows up in the listing.
    send(&mut b, &C2S::ListLobbies);
    assert!(
        pump(&mut b, Duration::from_secs(2), |m| {
            matches!(m, S2C::Lobbies { lobbies } if lobbies.iter().any(|l| l.name == "castle"))
        }),
        "the new lobby was not listed"
    );

    // Bob joins it.
    send(
        &mut b,
        &C2S::JoinLobby {
            name: "castle".into(),
            password: None,
        },
    );
    let mut b_slot = None;
    assert!(
        pump(&mut b, Duration::from_secs(2), |m| {
            if let S2C::Joined { slot, roster, .. } = m {
                assert_eq!(roster.len(), 2, "roster should already hold both drivers");
                b_slot = Some(slot);
                return true;
            }
            false
        }),
        "joiner never got Joined"
    );
    let (a_slot, b_slot) = (a_slot.unwrap(), b_slot.unwrap());
    assert_ne!(a_slot, b_slot, "both players were given the same grid slot");

    // Alice is told about Bob.
    assert!(
        pump(&mut a, Duration::from_secs(2), |m| matches!(
            m,
            S2C::PlayerJoined { .. }
        )),
        "the host was not told someone joined"
    );

    // Both ready up; the countdown should start.
    send(&mut a, &C2S::Ready { ready: true });
    send(&mut b, &C2S::Ready { ready: true });
    assert!(
        pump(&mut a, Duration::from_secs(3), |m| {
            matches!(
                m,
                S2C::Phase {
                    phase: Phase::Countdown,
                    ..
                }
            )
        }),
        "the countdown never started"
    );

    // Drive. Both hold throttle; the server is authoritative.
    let mut seq = 0u32;
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut moved_a = 0.0f32;
    let mut moved_b = 0.0f32;
    let mut start_a: Option<(f32, f32)> = None;
    let mut saw_racing = false;
    while Instant::now() < deadline {
        seq += 1;
        let input = C2S::Input {
            seq,
            throttle: 1.0,
            steer: 0.0,
            handbrake: false,
            boost: false,
        };
        send(&mut a, &input);
        send(&mut b, &input);

        pump(&mut a, Duration::from_millis(60), |m| {
            match m {
                S2C::Phase {
                    phase: Phase::Racing,
                    ..
                } => saw_racing = true,
                S2C::State { cars, .. } => {
                    let find = |slot: u8| cars.iter().find(|c| c.id == slot).copied();
                    if let Some(c) = find(a_slot) {
                        let p = start_a.get_or_insert((c.x, c.z));
                        moved_a = ((c.x - p.0).powi(2) + (c.z - p.1).powi(2)).sqrt();
                    }
                    if let Some(c) = find(b_slot) {
                        moved_b = moved_b.max(c.progress);
                    }
                }
                _ => {}
            }
            false
        });
        if saw_racing && moved_a > 40.0 && moved_b > 40.0 {
            break;
        }
    }

    assert!(saw_racing, "the race never reached the Racing phase");
    assert!(
        moved_a > 40.0,
        "alice's car only moved {moved_a:.1} m under full throttle"
    );
    assert!(
        moved_b > 40.0,
        "bob's car only covered {moved_b:.1} m of the lap"
    );
}

#[test]
fn a_wrong_password_is_refused_and_the_right_one_is_not() {
    let port = start_server();
    let mut host = client(port, "host");
    pump(&mut host, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "private".into(),
            password: Some("hunter2".into()),
        },
    );
    assert!(pump(&mut host, Duration::from_secs(2), |m| matches!(
        m,
        S2C::Joined { .. }
    )));

    let mut guest = client(port, "guest");
    pump(&mut guest, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });

    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "private".into(),
            password: Some("wrong".into()),
        },
    );
    assert!(
        pump(&mut guest, Duration::from_secs(2), |m| {
            matches!(m, S2C::Rejected { reason } if reason.contains("password"))
        }),
        "a wrong password was accepted"
    );

    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "private".into(),
            password: Some("hunter2".into()),
        },
    );
    assert!(
        pump(&mut guest, Duration::from_secs(2), |m| matches!(
            m,
            S2C::Joined { .. }
        )),
        "the correct password was refused"
    );
}

/// The gate is exact equality, and the refusal has to say what each side
/// speaks — "cannot join" with no version is the message that wastes an hour.
#[test]
fn a_version_mismatch_is_refused_with_both_versions() {
    let port = start_server();
    let (mut ws, _) = connect(format!("ws://127.0.0.1:{port}")).expect("connect");
    set_read_timeout(&ws, Duration::from_millis(50));
    send(
        &mut ws,
        &C2S::Hello {
            proto: proto::PROTO_VERSION + 7,
            handle: "stale".into(),
        },
    );
    pump(&mut ws, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });

    send(
        &mut ws,
        &C2S::CreateLobby {
            name: "nope".into(),
            password: None,
        },
    );
    let mut reason = String::new();
    assert!(
        pump(&mut ws, Duration::from_secs(2), |m| {
            if let S2C::Rejected { reason: r } = m {
                reason = r;
                return true;
            }
            false
        }),
        "a stale client was allowed to create a lobby"
    );
    assert!(
        reason.contains(&format!("v{}", proto::PROTO_VERSION + 7)),
        "{reason}"
    );
    assert!(
        reason.contains(&format!("v{}", proto::PROTO_VERSION)),
        "{reason}"
    );
}

/// Listing must work at any version: the hub's lobby browser has no game
/// loaded and must not be locked out by a protocol bump.
#[test]
fn listing_is_ungated() {
    let port = start_server();
    let mut host = client(port, "host");
    pump(&mut host, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "open".into(),
            password: None,
        },
    );
    assert!(pump(&mut host, Duration::from_secs(2), |m| matches!(
        m,
        S2C::Joined { .. }
    )));

    let (mut browser, _) = connect(format!("ws://127.0.0.1:{port}")).expect("connect");
    set_read_timeout(&browser, Duration::from_millis(50));
    send(
        &mut browser,
        &C2S::Hello {
            proto: 0,
            handle: "browser".into(),
        },
    );
    send(&mut browser, &C2S::ListLobbies);
    assert!(
        pump(&mut browser, Duration::from_secs(2), |m| {
            matches!(m, S2C::Lobbies { lobbies } if lobbies.iter().any(|l| l.name == "open"))
        }),
        "a version-0 browser could not list lobbies"
    );
}

/// A client that stops reading for a few seconds must not lose control
/// messages behind a wall of state broadcasts.
///
/// This is a real bug that shipped: a Waiting lobby broadcast the parked grid
/// at 30 Hz, so a peer that looked away for two seconds filled its 64-deep
/// outbound queue, and `PlayerJoined` — sent after the queue was full — was
/// dropped on the floor. The joiner then never appeared in anyone's roster,
/// permanently. The fix is two-part: do not stream state for a race that has
/// not started, and give the queue real headroom.
#[test]
fn a_slow_peer_still_gets_roster_updates() {
    let port = start_server();
    let mut host = client(port, "host");
    pump(&mut host, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "patient".into(),
            password: None,
        },
    );
    assert!(pump(&mut host, Duration::from_secs(2), |m| matches!(
        m,
        S2C::Joined { .. }
    )));

    // The host now sits in the lobby without reading a single frame.
    thread::sleep(Duration::from_secs(4));

    let mut guest = client(port, "guest");
    pump(&mut guest, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });
    send(
        &mut guest,
        &C2S::JoinLobby {
            name: "patient".into(),
            password: None,
        },
    );
    assert!(pump(&mut guest, Duration::from_secs(2), |m| matches!(
        m,
        S2C::Joined { .. }
    )));

    // Only now does the host start reading again. The join must still be in
    // there somewhere.
    assert!(
        pump(&mut host, Duration::from_secs(4), |m| matches!(
            m,
            S2C::PlayerJoined { .. }
        )),
        "a host who looked away for four seconds never learned someone joined"
    );
}

#[test]
fn a_disconnect_frees_the_slot_for_the_next_player() {
    let port = start_server();
    let mut host = client(port, "host");
    pump(&mut host, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });
    send(
        &mut host,
        &C2S::CreateLobby {
            name: "revolving".into(),
            password: None,
        },
    );
    assert!(pump(&mut host, Duration::from_secs(2), |m| matches!(
        m,
        S2C::Joined { .. }
    )));

    let mut first = client(port, "first");
    pump(&mut first, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });
    send(
        &mut first,
        &C2S::JoinLobby {
            name: "revolving".into(),
            password: None,
        },
    );
    let mut first_slot = None;
    assert!(pump(&mut first, Duration::from_secs(2), |m| {
        if let S2C::Joined { slot, .. } = m {
            first_slot = Some(slot);
            return true;
        }
        false
    }));

    drop(first);
    // Let the hub notice the drop.
    thread::sleep(Duration::from_millis(400));
    assert!(
        pump(&mut host, Duration::from_secs(2), |m| matches!(
            m,
            S2C::PlayerLeft { .. }
        )),
        "nobody was told the player left"
    );

    let mut second = client(port, "second");
    pump(&mut second, Duration::from_secs(2), |m| {
        matches!(m, S2C::Welcome { .. })
    });
    send(
        &mut second,
        &C2S::JoinLobby {
            name: "revolving".into(),
            password: None,
        },
    );
    assert!(
        pump(&mut second, Duration::from_secs(2), |m| {
            matches!(m, S2C::Joined { slot, .. } if Some(slot) == first_slot)
        }),
        "the vacated slot was not reused"
    );
}
