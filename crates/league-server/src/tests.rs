use super::*;

#[derive(Default)]
struct Hub {
    conns: HashMap<u64, Conn>,
    lobbies: HashMap<String, Lobby>,
    inbox: HashMap<u64, Receiver<Message>>,
    cfg: ServerConfig,
}

impl Hub {
    fn connect(&mut self, id: u64, version: Option<u16>) {
        let (tx, rx) = mpsc::sync_channel(OUTBOUND_QUEUE);
        self.inbox.insert(id, rx);
        handle_event(
            Ev::Connected {
                id,
                tx,
                peer: "test".into(),
            },
            &mut self.conns,
            &mut self.lobbies,
            &self.cfg,
        );
        if let Some(proto) = version {
            self.msg(
                id,
                C2S::Hello {
                    proto,
                    handle: format!("player-{id}"),
                },
            );
        }
    }

    fn msg(&mut self, id: u64, msg: C2S) -> Vec<S2C> {
        handle_event(
            Ev::Msg { id, msg },
            &mut self.conns,
            &mut self.lobbies,
            &self.cfg,
        );
        self.drain(id)
    }

    fn drain(&self, id: u64) -> Vec<S2C> {
        self.inbox[&id]
            .try_iter()
            .map(|m| serde_json::from_str(m.to_text().unwrap()).unwrap())
            .collect()
    }

    fn create(&mut self, id: u64, name: &str, mode: u8) -> Vec<S2C> {
        self.msg(
            id,
            C2S::CreateLobby {
                name: name.into(),
                password: None,
                mode,
            },
        )
    }

    fn join(&mut self, id: u64, name: &str) -> Vec<S2C> {
        self.msg(
            id,
            C2S::JoinLobby {
                name: name.into(),
                password: None,
            },
        )
    }

    fn pick(&mut self, id: u64, champ: u8) -> Vec<S2C> {
        self.msg(
            id,
            C2S::Pick {
                champ,
                d: 0,
                f: 1,
                runes: [0, 1, 2],
            },
        )
    }
}

fn rejected(messages: &[S2C]) -> bool {
    messages.iter().any(|m| matches!(m, S2C::Rejected { .. }))
}

#[test]
fn start_match_sends_roster_phase_and_state_in_order() {
    let mut hub = Hub::default();
    hub.connect(1, Some(proto::PROTO_VERSION));
    hub.connect(2, Some(proto::PROTO_VERSION));
    hub.create(1, "duel", 1);
    hub.join(2, "duel");
    hub.pick(1, 0);
    hub.pick(2, 0);
    drop(hub.drain(1));

    let replies = hub.msg(1, C2S::StartMatch);

    assert!(matches!(
        replies.as_slice(),
        [
            S2C::Roster { .. },
            S2C::Phase {
                phase: Phase::Live,
                ..
            },
            S2C::State { .. },
        ]
    ));
}

#[test]
fn protocol_gate_allows_listing_but_requires_one_compatible_hello_to_join() {
    let mut hub = Hub::default();
    hub.connect(1, None);
    assert!(matches!(
        hub.msg(1, C2S::ListLobbies).as_slice(),
        [S2C::Lobbies { .. }]
    ));
    assert!(rejected(&hub.create(1, "duel", 1)));
    hub.connect(2, Some(0));
    assert!(matches!(
        hub.msg(2, C2S::ListLobbies).as_slice(),
        [S2C::Lobbies { .. }]
    ));
    assert!(rejected(&hub.create(2, "duel", 1)));
    assert!(rejected(&hub.msg(
        2,
        C2S::Hello {
            proto: proto::PROTO_VERSION,
            handle: "changed".into()
        }
    )));
    assert_eq!(hub.conns[&2].proto, 0);
    hub.connect(3, Some(proto::PROTO_VERSION));
    assert!(!rejected(&hub.create(3, "duel", 1)));
    assert!(rejected(&hub.join(2, "duel")));
    // Frozen v1/v2 clients may list, but cannot join the attack-move protocol.
    hub.connect(4, Some(1));
    assert!(matches!(
        hub.msg(4, C2S::ListLobbies).as_slice(),
        [S2C::Lobbies { .. }]
    ));
    assert!(rejected(&hub.join(4, "duel")));
}

#[test]
fn attack_move_wire_command_controls_only_the_senders_live_champion() {
    let mut hub = Hub::default();
    for id in 1..=3 {
        hub.connect(id, Some(proto::PROTO_VERSION));
    }
    hub.create(1, "duel", 1);
    hub.join(2, "duel");
    hub.pick(1, 0);
    hub.pick(2, 0);
    hub.msg(1, C2S::StartMatch);
    let game = &mut hub.lobbies.get_mut("duel").unwrap().m;
    game.wave_left = 1e9;
    game.units[0].x = 0.0;
    game.units[0].z = 0.0;
    game.units[1].x = 4.0;
    game.units[1].z = 0.0;
    let raw = r#"{"t":"cmd","a":"attack_move","x":-20.0,"z":0.0}"#;
    hub.msg(3, serde_json::from_str(raw).unwrap());
    hub.msg(2, serde_json::from_str(raw).unwrap());
    let game = &mut hub.lobbies.get_mut("duel").unwrap().m;
    game.step();
    assert_eq!(game.units[0].order, league_core::sim::Order::Hold);
    assert_eq!(game.units[1].order, league_core::sim::Order::AttackMove);
    assert_eq!(game.units[1].target, game.units[0].id);
    assert_eq!(game.projs.len(), 1);
    assert_eq!(game.projs[0].owner, game.units[1].id);
}

#[test]
fn creating_a_second_lobby_removes_the_old_membership() {
    let mut hub = Hub::default();
    hub.connect(1, Some(proto::PROTO_VERSION));
    hub.connect(2, Some(proto::PROTO_VERSION));
    hub.create(1, "old", 1);
    hub.join(2, "old");
    assert!(!rejected(&hub.create(1, "new", 3)));
    assert_eq!(hub.conns[&1].lobby.as_deref(), Some("new"));
    assert_eq!(hub.lobbies["old"].members.len(), 1);
    assert!(hub.lobbies["old"].m.roster[0].bot);
    assert_eq!(hub.lobbies["new"].members.get(&0), Some(&1));
    assert!(rejected(&hub.create(1, "old", 1)));
    assert_eq!(hub.conns[&1].lobby.as_deref(), Some("new"));
    hub.msg(1, C2S::LeaveLobby);
    assert!(!hub.lobbies.contains_key("new"));
    assert!(!hub.lobbies["old"].members.values().any(|id| *id == 1));
}

#[test]
fn rejected_lobby_switch_preserves_the_current_seat_and_pick() {
    let mut hub = Hub::default();
    for id in 1..=4 {
        hub.connect(id, Some(proto::PROTO_VERSION));
    }
    hub.create(1, "mine", 1);
    hub.pick(1, 2);
    hub.create(2, "full", 1);
    hub.join(3, "full");
    hub.msg(
        4,
        C2S::CreateLobby {
            name: "private".into(),
            password: Some("secret".into()),
            mode: 3,
        },
    );
    for name in ["missing", "full", "private"] {
        assert!(rejected(&hub.join(1, name)));
        assert_eq!(hub.conns[&1].lobby.as_deref(), Some("mine"));
        assert_eq!(hub.lobbies["mine"].m.roster[0].champ, 2);
    }
    assert!(!rejected(&hub.join(1, "mine")));
    assert_eq!(hub.conns[&1].slot, Some(0));
    assert_eq!(hub.lobbies["mine"].m.roster[0].champ, 2);
    assert_eq!(hub.lobbies["mine"].members.len(), 1);
    assert!(!rejected(&hub.msg(
        1,
        C2S::JoinLobby {
            name: "private".into(),
            password: Some("secret".into())
        }
    )));
    assert!(!hub.lobbies.contains_key("mine"));
}

#[test]
fn host_handoff_starts_the_match_and_live_disconnects_become_bots() {
    let mut hub = Hub::default();
    for id in 1..=3 {
        hub.connect(id, Some(proto::PROTO_VERSION));
    }
    hub.create(1, "squad", 3);
    hub.join(2, "squad");
    assert!(rejected(&hub.msg(2, C2S::StartMatch)));
    hub.msg(1, C2S::LeaveLobby);
    assert!(!rejected(&hub.pick(2, 1)));
    let replies = hub.msg(2, C2S::StartMatch);
    assert!(!rejected(&replies));
    assert_eq!(hub.lobbies["squad"].m.phase, Phase::Live);
    assert!(
        replies
            .iter()
            .any(|m| matches!(m, S2C::State { champs, .. } if champs.len() == 6))
    );
    hub.lobbies.get_mut("squad").unwrap().m.fx.push(proto::Fx {
        source: 0,
        champ: proto::UNKNOWN_PRESENTATION,
        ability: proto::UNKNOWN_PRESENTATION,
        k: 8,
        x: 0.0,
        z: 0.0,
        x2: 1.0,
        z2: 1.0,
        v: 0.0,
    });
    let replies = hub.join(3, "squad");
    assert!(replies.iter().any(|m| matches!(m, S2C::State { .. })));
    assert_eq!(
        hub.lobbies["squad"].m.fx.len(),
        1,
        "a joining client must not consume effects owed to existing players"
    );
    handle_event(
        Ev::Disconnected { id: 2 },
        &mut hub.conns,
        &mut hub.lobbies,
        &hub.cfg,
    );
    assert!(hub.lobbies["squad"].m.roster[1].bot);
    assert!(!hub.lobbies["squad"].m.roster[1].connected);
    assert!(
        hub.drain(3)
            .iter()
            .any(|m| matches!(m, S2C::PlayerLeft { slot: 1 }))
    );
}

#[test]
fn squad_draft_rejects_teammate_duplicates_and_accepts_mirror_picks() {
    let mut hub = Hub::default();
    for id in 1..=4 {
        hub.connect(id, Some(proto::PROTO_VERSION));
    }
    hub.create(1, "squad", 3);
    for id in 2..=4 {
        hub.join(id, "squad");
    }
    assert!(!rejected(&hub.pick(1, 0)));
    assert!(rejected(&hub.pick(2, 0)));
    assert!(!rejected(&hub.pick(4, 0)));
    assert!(rejected(&hub.msg(
        2,
        C2S::Pick {
            champ: 1,
            d: 0,
            f: 0,
            runes: [0, 1, 2]
        }
    )));
    assert!(rejected(&hub.msg(
        2,
        C2S::Pick {
            champ: 1,
            d: 0,
            f: 1,
            runes: [0, 0, 2]
        }
    )));
    assert!(!rejected(&hub.pick(2, 1)));
    assert!(!rejected(&hub.pick(3, 2)));
    assert!(!rejected(&hub.msg(1, C2S::StartMatch)));
    assert!(rejected(&hub.pick(4, 1)));
    for team in 0..2 {
        let mut picks: Vec<u8> = hub.lobbies["squad"]
            .m
            .roster
            .iter()
            .filter(|r| r.team == team)
            .map(|r| r.champ)
            .collect();
        picks.sort_unstable();
        picks.dedup();
        assert_eq!(picks.len(), 3);
    }
}

#[test]
fn early_start_waits_for_every_human_pick_and_fills_only_bot_seats() {
    for mode in [1, 3] {
        for humans in 1..=mode * 2 {
            let mut hub = Hub::default();
            for human in 1..=humans {
                let id = u64::from(human);
                hub.connect(id, Some(proto::PROTO_VERSION));
                let replies = if human == 1 {
                    hub.create(id, "draft", mode)
                } else {
                    hub.join(id, "draft")
                };
                assert!(!rejected(&replies));
            }
            let draft_left = hub.lobbies["draft"].m.left;
            for human in 1..=humans {
                // The existing protocol-1 request must reject without starting
                // the sim while even the last human remains unpicked.
                let start: C2S = serde_json::from_str(r#"{"t":"start_match"}"#).unwrap();
                let replies = hub.msg(1, start);
                assert!(
                    rejected(&replies),
                    "mode={mode}, humans={humans}, next pick={human}"
                );
                assert!(!replies.iter().any(|m| matches!(m, S2C::State { .. })));
                let lobby = &hub.lobbies["draft"];
                assert_eq!(lobby.m.phase, Phase::Select);
                assert_eq!(lobby.m.left, draft_left);
                assert_eq!(
                    lobby.m.roster.iter().filter(|r| r.picked).count(),
                    usize::from(human - 1)
                );
                // Unique within each team; opponents may mirror the pick.
                assert!(!rejected(&hub.pick(u64::from(human), (human - 1) % mode)));
            }
            let replies = hub.msg(1, C2S::StartMatch);
            assert!(!rejected(&replies), "mode={mode}, humans={humans}");
            assert!(replies.iter().any(|m| matches!(
                m,
                S2C::Phase {
                    phase: Phase::Live,
                    ..
                }
            )));
            assert!(replies.iter().any(|m| matches!(
                m,
                S2C::State { champs, .. } if champs.len() == usize::from(mode * 2)
            )));
            let lobby = &hub.lobbies["draft"];
            assert_eq!(lobby.m.phase, Phase::Live);
            assert!(lobby.m.roster.iter().all(|r| r.picked));
            assert_eq!(
                lobby.m.roster.iter().filter(|r| r.bot).count(),
                usize::from(mode * 2 - humans)
            );
            for human in 1..=humans {
                let slot = &lobby.m.roster[usize::from(human - 1)];
                assert!(!slot.bot);
                assert_eq!(slot.champ, (human - 1) % mode);
            }
        }
    }
}

#[test]
fn draft_clock_result_reset_and_silent_cleanup_follow_the_lifecycle() {
    let mut hub = Hub::default();
    hub.connect(1, Some(proto::PROTO_VERSION));
    hub.create(1, "duel", 1);
    hub.lobbies.get_mut("duel").unwrap().m.left = league_core::DT;
    tick_lobbies(&mut hub.lobbies, &hub.conns);
    assert_eq!(hub.lobbies["duel"].m.phase, Phase::Live);
    assert!(hub.drain(1).iter().any(|m| matches!(
        m,
        S2C::Phase {
            phase: Phase::Live,
            ..
        }
    )));
    let lobby = hub.lobbies.get_mut("duel").unwrap();
    lobby.m.phase = Phase::Over;
    lobby.m.left = 0.0;
    let seed = lobby.m.seed;
    tick_lobbies(&mut hub.lobbies, &hub.conns);
    let lobby = &hub.lobbies["duel"];
    assert_eq!(lobby.m.phase, Phase::Select);
    assert_eq!(lobby.m.seed, seed.wrapping_add(1));
    assert!(lobby.m.roster[0].connected);
    assert!(!lobby.m.roster[0].picked);
    hub.conns.get_mut(&1).unwrap().last_seen = Instant::now()
        .checked_sub(Duration::from_secs(proto::CLIENT_TIMEOUT_SECS + 1))
        .unwrap();
    drop_silent(&mut hub.conns, &mut hub.lobbies);
    assert!(hub.conns.is_empty());
    assert!(hub.lobbies.is_empty());
}

type Wire = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>;

fn wire_send(ws: &mut Wire, msg: &C2S) {
    ws.send(Message::text(serde_json::to_string(msg).unwrap()))
        .unwrap();
}

fn wire_until(ws: &mut Wire, matches: impl Fn(&S2C) -> bool) -> S2C {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let msg: S2C = serde_json::from_str(&text).unwrap();
                if matches(&msg) {
                    return msg;
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if proto::is_transient_read(&e) => {}
            Err(e) => panic!("WebSocket failed: {e}"),
        }
    }
    panic!("timed out waiting for the expected server message");
}

fn connect_wire_clients(url: &str, mode: u8) -> Vec<Wire> {
    let mut clients = Vec::new();
    for slot in 0..=2 * mode {
        let (mut ws, _) = tungstenite::connect(url).unwrap();
        if let tungstenite::stream::MaybeTlsStream::Plain(stream) = ws.get_ref() {
            stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
        }
        wire_send(
            &mut ws,
            &C2S::Hello {
                proto: if slot == 2 * mode {
                    0
                } else {
                    proto::PROTO_VERSION
                },
                handle: format!("wire-{slot}"),
            },
        );
        wire_until(
            &mut ws,
            |m| matches!(m, S2C::Welcome { proto: p, .. } if *p == proto::PROTO_VERSION),
        );
        clients.push(ws);
    }
    clients
}

fn fill_wire_lobby(clients: &mut [Wire], mode: u8) {
    let browser = usize::from(2 * mode);
    wire_send(
        &mut clients[browser],
        &C2S::CreateLobby {
            name: "wrong-version".into(),
            password: None,
            mode,
        },
    );
    wire_until(&mut clients[browser], |m| matches!(m, S2C::Rejected { .. }));
    wire_send(
        &mut clients[0],
        &C2S::CreateLobby {
            name: "wire-game".into(),
            password: None,
            mode,
        },
    );
    wire_until(&mut clients[0], |m| matches!(m, S2C::Joined { id: 0, .. }));
    for slot in 1..2 * mode {
        wire_send(
            &mut clients[usize::from(slot)],
            &C2S::JoinLobby {
                name: "wire-game".into(),
                password: None,
            },
        );
        wire_until(
            &mut clients[usize::from(slot)],
            |m| matches!(m, S2C::Joined { id, .. } if *id == slot),
        );
    }
    wire_send(&mut clients[browser], &C2S::ListLobbies);
    wire_until(
        &mut clients[browser],
        |m| matches!(m, S2C::Lobbies { lobbies } if lobbies.len() == 1 && lobbies[0].players == 2 * mode && lobbies[0].cap == 2 * mode),
    );
    for slot in 0..2 * mode {
        wire_send(
            &mut clients[usize::from(slot)],
            &C2S::Pick {
                champ: slot % mode,
                d: 0,
                f: 1,
                runes: [0, 1, 2],
            },
        );
        wire_until(
            &mut clients[usize::from(slot)],
            |m| matches!(m, S2C::Roster { roster } if roster[usize::from(slot)].picked && roster[usize::from(slot)].champ == slot % mode),
        );
    }
}

fn exercise_live_wire_commands(clients: &mut [Wire], mode: u8) {
    wire_send(&mut clients[0], &C2S::StartMatch);
    let first = wire_until(
        &mut clients[0],
        |m| matches!(m, S2C::State { champs, .. } if champs.len() == usize::from(2 * mode)),
    );
    let S2C::State { units, .. } = first else {
        unreachable!()
    };
    let start_x = units.iter().find(|u| u.k == 0 && u.slot == 0).unwrap().x;
    for cmd in [
        Cmd::Rank { slot: 0 },
        Cmd::Buy { item: 1 },
        Cmd::Move { x: -40.0, z: 0.0 },
    ] {
        wire_send(&mut clients[0], &C2S::Cmd(cmd));
    }
    wire_until(
        &mut clients[0],
        |m| matches!(m, S2C::State { tick, units, champs, .. } if *tick > 0 && units.iter().any(|u| u.k == 0 && u.slot == 0 && u.x > start_x + 0.5) && champs[0].ranks[0] == 1 && champs[0].items[0] == 1),
    );
    // Travel beyond the previous Move destination, so a silently ignored
    // new command cannot pass this actual socket regression.
    wire_send(
        &mut clients[0],
        &C2S::Cmd(Cmd::AttackMove { x: -30.0, z: 0.0 }),
    );
    wire_until(
        &mut clients[0],
        |m| matches!(m, S2C::State { units, .. } if units.iter().any(|u| u.k == 0 && u.slot == 0 && u.x > -38.0)),
    );
    wire_send(&mut clients[0], &C2S::LeaveLobby);
    wire_until(&mut clients[1], |m| {
        matches!(m, S2C::PlayerLeft { slot: 0 })
    });
}

fn exercise_wire_mode(mode: u8) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::sync_channel(MAX_PENDING_EVENTS);
    let server = thread::spawn(move || {
        let hub = thread::spawn(move || hub_loop(&rx, &ServerConfig::default()).unwrap());
        let mut connections = Vec::new();
        for id in 0..=u64::from(2 * mode) {
            let stream = listener.accept().unwrap().0;
            let events = tx.clone();
            connections.push(thread::spawn(move || conn_thread(id, stream, &events)));
        }
        drop(tx);
        for connection in connections {
            connection.join().unwrap();
        }
        hub.join().unwrap();
    });
    let mut clients = connect_wire_clients(&url, mode);
    fill_wire_lobby(&mut clients, mode);
    exercise_live_wire_commands(&mut clients, mode);
    for client in &mut clients {
        drop(client.close(None));
    }
    drop(clients);
    server.join().unwrap();
}

#[test]
fn real_websockets_fill_both_modes_and_apply_live_player_commands() {
    for mode in [1u8, 3] {
        exercise_wire_mode(mode);
    }
}
