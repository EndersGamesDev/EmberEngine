//! The whole loop, natively: a real `fire-server`, the real `Net` transport,
//! and the real `Online` client with prediction and reconciliation.
//!
//! The server's own e2e test proves the wire works. This one proves the
//! *client* works against it — specifically that prediction converges rather
//! than drifting, which is the failure you cannot see in a unit test and
//! cannot miss when playing.

use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use fire::net::{Net, Status};
use fire::online::{Online, Screen};
use fire_core::car::CarInput;
use fire_core::proto::{C2S, Phase, S2C};

/// Surface the server's own warnings (dropped sends, rate-limited messages)
/// in test output. Without this a lost control message is invisible from both
/// ends and looks like an unexplained timeout.
fn init_logs() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        drop(
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "warn".into()),
                )
                .with_test_writer()
                .try_init(),
        );
    });
}

fn start_server(laps: u32) -> u16 {
    init_logs();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        drop(fire_server::run(
            listener,
            fire_server::ServerConfig {
                laps,
                max_lobbies: 8,
            },
        ));
    });
    thread::sleep(Duration::from_millis(150));
    port
}

struct Peer {
    net: Net,
    game: Online,
    inbox: std::collections::VecDeque<S2C>,
}

impl Peer {
    fn connect(port: u16, handle: &str) -> Self {
        let net = Net::connect(&format!("ws://127.0.0.1:{port}")).expect("connect");
        let t0 = Instant::now();
        while net.status() == Status::Connecting && t0.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(net.status(), Status::Open, "socket never opened");
        fire::net::hello(&net, handle);
        let mut peer = Self {
            net,
            game: Online::new(),
            inbox: std::collections::VecDeque::default(),
        };
        // Wait for Welcome before returning. Hello must be the first message
        // on a connection and Welcome acknowledges it; anything version-gated
        // sent before then races the server's handling of Hello and is refused
        // with "this build speaks fire protocol v0".
        assert!(
            peer.wait_for(Duration::from_secs(5), |g| g.welcomed),
            "{handle}: no Welcome"
        );
        peer
    }

    fn pump(&mut self) {
        self.net.drain(&mut self.inbox);
        while let Some(m) = self.inbox.pop_front() {
            self.game.apply(m);
        }
    }

    /// Pump until `f` is satisfied or we give up.
    fn wait_for(&mut self, dur: Duration, mut f: impl FnMut(&Online) -> bool) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < dur {
            self.pump();
            if f(&self.game) {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }

    /// Everything that distinguishes the ways this can go wrong: a dead
    /// socket, a refusal we never looked at, or a message that simply never
    /// came. A bare `assert!` cannot tell them apart, which is what made the
    /// last two failures look like the same bug.
    fn dump(&self, who: &str) -> String {
        format!(
            "{who}: socket={:?} screen={:?} slot={:?} phase={:?} notice={:?} roster={} lobby={:?}",
            self.net.status(),
            self.game.screen,
            self.game.my_slot,
            self.game.phase,
            self.game.notice,
            self.game.roster.len(),
            self.game.lobby_name,
        )
    }

    /// One client frame: apply what arrived, send an input, predict forward.
    fn frame(&mut self, input: CarInput) {
        self.pump();
        let msg = self.game.make_input(input);
        self.net.send(&msg);
        self.game.predict_tick(input);
    }
}

#[test]
fn a_client_joins_races_and_its_prediction_converges() {
    let port = start_server(2);
    let mut me = Peer::connect(port, "driver");

    me.net.send(&C2S::CreateLobby {
        name: "castle".into(),
        password: None,
    });
    assert!(
        me.wait_for(Duration::from_secs(5), |g| g.screen == Screen::InLobby),
        "never joined the lobby I created.\n  {}",
        me.dump("me")
    );
    let slot = me.game.my_slot.expect("no grid slot");

    me.net.send(&C2S::Ready { ready: true });
    assert!(
        me.wait_for(Duration::from_secs(4), |g| g.phase == Phase::Countdown
            || g.phase == Phase::Racing),
        "the countdown never started"
    );
    assert!(
        me.wait_for(Duration::from_secs(8), |g| g.phase == Phase::Racing),
        "the race never went green"
    );

    // Drive for a few seconds, tracking how far the prediction sits from the
    // authoritative state at the moment each snapshot lands.
    let throttle = CarInput {
        throttle: 1.0,
        steer: 0.15,
        handbrake: false,
        boost: false,
    };
    let start = me.game.my_car().unwrap().pos;
    let mut worst_error = 0.0f32;

    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        // Measure just before applying: the gap between where we think we are
        // and where the server puts us.
        let predicted = me.game.my_car().unwrap().pos;
        me.net.drain(&mut me.inbox);
        let mut authoritative = None;
        while let Some(m) = me.inbox.pop_front() {
            if let S2C::State { cars, .. } = &m
                && let Some(c) = cars.iter().find(|c| c.id == slot)
            {
                authoritative = Some(ember_engine::glam::Vec2::new(c.x, c.z));
            }
            me.game.apply(m);
        }
        if let Some(a) = authoritative {
            // Only meaningful once we are actually moving.
            if predicted.distance(start) > 5.0 {
                worst_error = worst_error.max(predicted.distance(a));
            }
        }
        let msg = me.game.make_input(throttle);
        me.net.send(&msg);
        me.game.predict_tick(throttle);
        thread::sleep(Duration::from_millis(16));
    }

    let travelled = me.game.my_car().unwrap().pos.distance(start);
    assert!(
        travelled > 60.0,
        "the car only covered {travelled:.1} m in six seconds"
    );

    // Over loopback the round trip is ~0, so the prediction should track the
    // server closely. A large number here means reconciliation is not
    // converging — the client and server disagree about the physics.
    assert!(
        worst_error < 25.0,
        "prediction drifted {worst_error:.1} m from the server — reconciliation is not converging"
    );
}

#[test]
fn two_clients_see_each_other_move() {
    let port = start_server(3);
    let mut a = Peer::connect(port, "alice");
    let mut b = Peer::connect(port, "bob");

    a.net.send(&C2S::CreateLobby {
        name: "shared".into(),
        password: None,
    });
    assert!(
        a.wait_for(Duration::from_secs(5), |g| g.screen == Screen::InLobby),
        "alice never got Joined for the lobby she created.\n  {}",
        a.dump("alice")
    );
    b.net.send(&C2S::JoinLobby {
        name: "shared".into(),
        password: None,
    });
    assert!(
        b.wait_for(Duration::from_secs(5), |g| g.screen == Screen::InLobby),
        "bob never joined.\n  {}\n  {}",
        b.dump("bob"),
        a.dump("alice")
    );

    let a_slot = a.game.my_slot.unwrap();
    let b_slot = b.game.my_slot.unwrap();
    assert_ne!(a_slot, b_slot);
    // Report enough to tell the two failure modes apart. "Never learned" can
    // mean the message was lost, or that alice's socket died and `drain` has
    // been returning nothing ever since — which looks identical from here.
    assert!(
        a.wait_for(Duration::from_secs(5), |g| g.roster.len() == 2),
        "alice never learned bob had joined.\n  \
         alice socket: {:?}\n  bob socket: {:?}\n  \
         alice roster: {:?}\n  alice slot {a_slot}, bob slot {b_slot}",
        a.net.status(),
        b.net.status(),
        a.game.roster,
    );

    a.net.send(&C2S::Ready { ready: true });
    b.net.send(&C2S::Ready { ready: true });
    if !a.wait_for(Duration::from_secs(10), |g| g.phase == Phase::Racing) {
        // Either a Ready never reached the server, or a Phase never came back.
        b.pump();
        panic!(
            "the race never went green.
               alice phase {:?} socket {:?}
  bob phase {:?} socket {:?}
               alice roster {:?}",
            a.game.phase,
            a.net.status(),
            b.game.phase,
            b.net.status(),
            a.game.roster,
        );
    }

    // Only alice drives; bob holds still. Alice must see bob's car where the
    // server puts it, and bob must see alice pull away.
    let throttle = CarInput {
        throttle: 1.0,
        steer: 0.0,
        handbrake: false,
        boost: false,
    };
    let idle = CarInput::default();
    let a_start_seen_by_b = b.game.race.racers[usize::from(a_slot)].car.pos;

    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        a.frame(throttle);
        b.frame(idle);
        thread::sleep(Duration::from_millis(16));
    }

    let a_moved_for_b = b.game.race.racers[usize::from(a_slot)]
        .car
        .pos
        .distance(a_start_seen_by_b);
    assert!(
        a_moved_for_b > 40.0,
        "bob saw alice move only {a_moved_for_b:.1} m — remote cars are not being applied"
    );
}

/// A refusal has to reach the client as something it can show, not a silent
/// failure to join.
#[test]
fn a_refused_join_surfaces_a_reason() {
    let port = start_server(3);
    let mut host = Peer::connect(port, "host");
    host.net.send(&C2S::CreateLobby {
        name: "locked".into(),
        password: Some("secret".into()),
    });
    assert!(host.wait_for(Duration::from_secs(3), |g| g.screen == Screen::InLobby));

    let mut guest = Peer::connect(port, "guest");
    guest.net.send(&C2S::JoinLobby {
        name: "locked".into(),
        password: Some("nope".into()),
    });
    assert!(
        guest.wait_for(Duration::from_secs(3), |g| g.notice.is_some()),
        "a refused join produced no notice"
    );
    assert!(guest.game.notice.as_deref().unwrap().contains("password"));
    assert_eq!(
        guest.game.screen,
        Screen::Browsing,
        "a refused client thinks it is in a lobby"
    );
}

#[test]
fn the_lobby_browser_sees_open_lobbies() {
    let port = start_server(3);
    let mut host = Peer::connect(port, "host");
    host.net.send(&C2S::CreateLobby {
        name: "visible".into(),
        password: None,
    });
    assert!(host.wait_for(Duration::from_secs(3), |g| g.screen == Screen::InLobby));

    let mut browser = Peer::connect(port, "browser");
    browser.net.send(&C2S::ListLobbies);
    assert!(
        browser.wait_for(Duration::from_secs(3), |g| {
            g.lobbies.iter().any(|l| l.name == "visible")
        }),
        "the lobby browser saw nothing"
    );
    let l = browser
        .game
        .lobbies
        .iter()
        .find(|l| l.name == "visible")
        .unwrap();
    assert_eq!(l.players, 1);
    assert!(!l.has_password);
}
