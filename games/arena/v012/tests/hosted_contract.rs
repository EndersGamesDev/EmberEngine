use std::collections::BTreeMap;
use std::sync::Arc;

use ember_game_arena_v12::proto::{C2S, S2C};
use ember_game_arena_v12::{
    ArenaCodec, ArenaFactory, ArenaLegacyAction, ArenaLegacyDecoder, ArenaLegacyIngressFactory,
    FIXTURE_SUITE_ID, LegacyLobbyEntry,
};
use ember_legacy::{
    AdmissionMetadata, BroadcastHandle, GameFactory, GameKey, InnerCodec, InnerFrame,
    LegacyCapabilities, LegacyClock, LegacyConnectionState, LegacyIngressAction,
    LegacyIngressFactory, LegacyLobbyProjection, LegacyRandom, LegacyTransport, LobbySeed,
    LobbyStatus, MetricObservation, MonotonicTimestamp, PeerId, RandomDrawKey, ScheduleError,
    ScheduleHandle, SchedulingRequest, SessionCreationData, SessionInput, TransportError,
    UnicastHandle,
};
use serde::Deserialize;

const C2S_FIXTURES: &str = include_str!("fixtures/c2s.jsonl");
const S2C_FIXTURES: &str = include_str!("fixtures/s2c.jsonl");
const LEGACY_LOBBY_FIXTURE: &str = include_str!("fixtures/legacy_lobby.json");
const TRACE_FIXTURE: &str = include_str!("fixtures/deterministic_trace.json");
const SUITE_FIXTURE: &str = include_str!("fixtures/suite.json");

const fn c2s_kind(message: &C2S) -> &'static str {
    match message {
        C2S::Hello { .. } => "hello",
        C2S::ListLobbies => "list_lobbies",
        C2S::CreateLobby { .. } => "create_lobby",
        C2S::JoinLobby { .. } => "join_lobby",
        C2S::LeaveLobby => "leave_lobby",
        C2S::Input { .. } => "input",
        C2S::Ping { .. } => "ping",
    }
}

const fn s2c_kind(message: &S2C) -> &'static str {
    match message {
        S2C::Welcome { .. } => "welcome",
        S2C::Error { .. } => "error",
        S2C::LobbyList { .. } => "lobby_list",
        S2C::GameJoined { .. } => "game_joined",
        S2C::PlayerJoined { .. } => "player_joined",
        S2C::PlayerLeft { .. } => "player_left",
        S2C::State { .. } => "state",
        S2C::Kill { .. } => "kill",
        S2C::Pong { .. } => "pong",
    }
}

#[test]
fn every_v12_wire_message_has_a_canonical_json_fixture() {
    let mut c2s_kinds = Vec::new();
    for line in C2S_FIXTURES.lines() {
        let message: C2S = serde_json::from_str(line).unwrap();
        c2s_kinds.push(c2s_kind(&message));
        assert_eq!(serde_json::to_string(&message).unwrap(), line);
    }
    assert_eq!(
        c2s_kinds,
        [
            "hello",
            "list_lobbies",
            "create_lobby",
            "join_lobby",
            "leave_lobby",
            "input",
            "ping",
        ]
    );

    let mut s2c_kinds = Vec::new();
    for line in S2C_FIXTURES.lines() {
        let message: S2C = serde_json::from_str(line).unwrap();
        s2c_kinds.push(s2c_kind(&message));
        assert_eq!(serde_json::to_string(&message).unwrap(), line);
    }
    assert_eq!(
        s2c_kinds,
        [
            "welcome",
            "error",
            "lobby_list",
            "game_joined",
            "player_joined",
            "player_left",
            "state",
            "kill",
            "pong",
        ]
    );
}

#[test]
fn codec_preserves_exact_json_text_frames() {
    let codec = ArenaCodec::new();
    for line in C2S_FIXTURES.lines() {
        let decoded = codec.decode(&InnerFrame::Text(line.to_string())).unwrap();
        let message: C2S = serde_json::from_slice(&decoded.payload).unwrap();
        assert_eq!(serde_json::to_string(&message).unwrap(), line);
    }
    for line in S2C_FIXTURES.lines() {
        let message: S2C = serde_json::from_str(line).unwrap();
        let event = ember_legacy::EncodedEvent {
            payload: serde_json::to_vec(&message).unwrap(),
        };
        assert_eq!(
            codec.encode(&event).unwrap(),
            InnerFrame::Text(line.to_string())
        );
    }
    assert_eq!(
        codec.decode(&InnerFrame::Binary(Vec::new())),
        Err(ember_legacy::InnerCodecError::WrongFrameKind)
    );
}

#[derive(Deserialize)]
struct LegacyFixture {
    hello: String,
    welcome: String,
    list: String,
    list_response: String,
    create: String,
    join: String,
    refusal: String,
}

#[derive(Deserialize)]
struct SuiteFixture {
    id: String,
}

#[test]
fn legacy_lobby_create_join_and_refusal_match_the_deployed_wire() {
    let suite: SuiteFixture = serde_json::from_str(SUITE_FIXTURE).unwrap();
    assert_eq!(FIXTURE_SUITE_ID, suite.id);
    let fixture: LegacyFixture = serde_json::from_str(LEGACY_LOBBY_FIXTURE).unwrap();
    let mut decoder = ArenaLegacyDecoder::new();

    let hello = decoder.decode(&InnerFrame::Text(fixture.hello)).unwrap();
    let ArenaLegacyAction::Hello {
        handle,
        requested_version,
        response,
    } = hello
    else {
        panic!("hello fixture decoded to the wrong action");
    };
    assert_eq!(handle, "ender");
    assert_eq!(requested_version, 12);
    assert_eq!(serde_json::to_string(&response).unwrap(), fixture.welcome);

    assert!(matches!(
        decoder.decode(&InnerFrame::Text(fixture.list)).unwrap(),
        ArenaLegacyAction::ListLobbies {
            requested_version: 12
        }
    ));
    let projected = ArenaLegacyDecoder::project_lobby_list(
        12,
        &[
            LegacyLobbyEntry {
                game_key: GameKey {
                    game_id: "arena".to_string(),
                    game_version: 12,
                },
                name: "alpha".to_string(),
                host: "ender".to_string(),
                has_password: true,
                players: 1,
                cap: 8,
            },
            LegacyLobbyEntry {
                game_key: GameKey {
                    game_id: "fire".to_string(),
                    game_version: 1,
                },
                name: "alpha".to_string(),
                host: "driver".to_string(),
                has_password: false,
                players: 2,
                cap: 8,
            },
        ],
    );
    assert_eq!(
        serde_json::to_string(&projected).unwrap(),
        fixture.list_response
    );

    let create = decoder.decode(&InnerFrame::Text(fixture.create)).unwrap();
    let ArenaLegacyAction::CreateLobby {
        game_key,
        name,
        password,
    } = create
    else {
        panic!("create fixture decoded to the wrong action");
    };
    assert_eq!(game_key.game_id, "arena");
    assert_eq!(game_key.game_version, 12);
    assert_eq!(name, "alpha");
    assert_eq!(password.as_deref(), Some("secret"));

    let join = decoder.decode(&InnerFrame::Text(fixture.join)).unwrap();
    let ArenaLegacyAction::JoinLobby {
        game_key,
        name,
        password,
    } = join
    else {
        panic!("join fixture decoded to the wrong action");
    };
    assert_eq!(game_key.game_version, 12);
    assert_eq!(name, "alpha");
    assert_eq!(password.as_deref(), Some("secret"));

    let refusal = ArenaLegacyDecoder::version_refusal(11, &[12]);
    assert_eq!(serde_json::to_string(&refusal).unwrap(), fixture.refusal);
}

#[test]
fn shared_legacy_ingress_preserves_arena_frames() {
    let fixture: LegacyFixture = serde_json::from_str(LEGACY_LOBBY_FIXTURE).unwrap();
    let mut ingress = ArenaLegacyIngressFactory.create();
    let hello = ingress
        .decode(
            LegacyConnectionState::AwaitHello,
            &InnerFrame::Text(fixture.hello),
        )
        .unwrap();
    let LegacyIngressAction::Hello { response, .. } = hello else {
        panic!("shared Arena hello decoded to the wrong action");
    };
    assert_eq!(response, InnerFrame::Text(fixture.welcome));
    let projected = ingress
        .project_lobbies(&[LegacyLobbyProjection {
            game_key: GameKey {
                game_id: "arena".to_string(),
                game_version: 12,
            },
            lobby_name: "alpha".to_string(),
            host_handle: "ender".to_string(),
            password_protected: true,
            occupancy: 1,
            capacity: 8,
            status: LobbyStatus {
                code: "running".to_string(),
                detail: None,
            },
        }])
        .unwrap();
    assert_eq!(projected, InnerFrame::Text(fixture.list_response));
}

struct TestClock;

impl LegacyClock for TestClock {
    fn now(&self) -> MonotonicTimestamp {
        MonotonicTimestamp::from_micros(0)
    }

    fn request_schedule(
        &self,
        _request: SchedulingRequest,
    ) -> Result<ScheduleHandle, ScheduleError> {
        Ok(ScheduleHandle::from_host_value(1))
    }

    fn cancel_schedule(&self, _handle: ScheduleHandle) -> Result<(), ScheduleError> {
        Ok(())
    }
}

struct TestRandom;

impl LegacyRandom for TestRandom {
    fn draw_u64(&self, _key: &RandomDrawKey) -> u64 {
        0
    }

    fn fill_bytes(&self, _key: &RandomDrawKey, output: &mut [u8]) {
        output.fill(0);
    }
}

struct TestTransport;

impl LegacyTransport for TestTransport {
    fn unicast(&self, _peer_id: PeerId) -> Result<UnicastHandle, TransportError> {
        Err(TransportError::UnknownPeer)
    }

    fn broadcast(
        &self,
        _session_id: ember_legacy::SessionId,
    ) -> Result<BroadcastHandle, TransportError> {
        Err(TransportError::UnknownSession)
    }

    fn close_peer(
        &self,
        _peer_id: PeerId,
        _reason: ember_legacy::CloseReason,
    ) -> Result<(), TransportError> {
        Err(TransportError::UnknownPeer)
    }

    fn record_metric(&self, _observation: MetricObservation) {}
}

#[derive(Deserialize)]
struct TraceFixture {
    seed: u64,
    created_at_micros: u64,
    calls: Vec<TraceCall>,
}

#[derive(Deserialize)]
struct TraceCall {
    timestamp_micros: u64,
    frames: Vec<String>,
    checkpoint: Option<TraceCheckpoint>,
}

#[derive(Deserialize)]
struct TraceCheckpoint {
    tick: u64,
    x: f32,
    z: f32,
    y: f32,
    vy: f32,
    ack: u32,
    ack_age_ticks: u16,
}

fn test_capabilities() -> LegacyCapabilities {
    LegacyCapabilities {
        clock: Arc::new(TestClock),
        random: Arc::new(TestRandom),
        transport: Arc::new(TestTransport),
        assets: None,
    }
}

fn state_from_update(codec: ArenaCodec, update: &ember_legacy::SessionUpdate) -> Option<S2C> {
    update.outbound.iter().find_map(|outbound| {
        let frame = codec.encode(&outbound.event).ok()?;
        let InnerFrame::Text(text) = frame else {
            return None;
        };
        let message: S2C = serde_json::from_str(&text).ok()?;
        match message {
            state @ S2C::State { .. } => Some(state),
            _ => None,
        }
    })
}

fn close_enough(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 0.000_01
}

#[test]
fn timestamped_transcript_produces_authoritative_checkpoints() {
    let trace: TraceFixture = serde_json::from_str(TRACE_FIXTURE).unwrap();
    let mut seed_bytes = [0_u8; 32];
    seed_bytes[..8].copy_from_slice(&trace.seed.to_le_bytes());
    let creation = SessionCreationData {
        game_key: GameKey {
            game_id: "arena".to_string(),
            game_version: 12,
        },
        session_id: ember_legacy::SessionId::from_host_value(7),
        lobby_name: "trace".to_string(),
        lobby_seed: LobbySeed(seed_bytes),
        created_at: MonotonicTimestamp::from_micros(trace.created_at_micros),
        configured_rules: Vec::new(),
    };
    let factory = ArenaFactory::new();
    let mut session = factory.create(&test_capabilities(), &creation).unwrap();
    let peer_id = PeerId::from_host_value(11);
    let codec = ArenaCodec::new();
    let joined = session
        .join(AdmissionMetadata {
            peer_id,
            handle: "ender".to_string(),
            admitted_at: creation.created_at,
            attributes: BTreeMap::new(),
        })
        .unwrap();
    assert_eq!(
        joined.scheduling,
        [SchedulingRequest::At(MonotonicTimestamp::from_micros(
            1_016_667
        ))]
    );
    let joined_frame = codec.encode(&joined.outbound[0].event).unwrap();
    let InnerFrame::Text(joined_text) = joined_frame else {
        panic!("Arena v12 emitted a binary join frame");
    };
    let joined_message: S2C = serde_json::from_str(&joined_text).unwrap();
    assert!(matches!(joined_message, S2C::GameJoined { seed: 42, .. }));

    for call in trace.calls {
        let timestamp = MonotonicTimestamp::from_micros(call.timestamp_micros);
        let inputs = call
            .frames
            .into_iter()
            .map(|frame| SessionInput {
                peer_id,
                received_at: timestamp,
                input: codec.decode(&InnerFrame::Text(frame)).unwrap(),
            })
            .collect();
        let update = session.step(timestamp, inputs);
        let state = state_from_update(codec, &update);
        let Some(checkpoint) = call.checkpoint else {
            assert!(state.is_none());
            continue;
        };
        let Some(S2C::State {
            tick, mut players, ..
        }) = state
        else {
            panic!("checkpoint call did not broadcast state");
        };
        assert_eq!(tick, checkpoint.tick);
        assert_eq!(players.len(), 1);
        let player = players.remove(0);
        assert!(close_enough(player.x, checkpoint.x));
        assert!(close_enough(player.z, checkpoint.z));
        assert!(close_enough(player.y, checkpoint.y));
        assert!(close_enough(player.vy, checkpoint.vy));
        assert_eq!(player.ack, checkpoint.ack);
        assert_eq!(player.ack_age_ticks, checkpoint.ack_age_ticks);
    }
}

#[test]
fn lobby_projection_follows_the_hello_protocol() {
    let entries = [
        LegacyLobbyEntry {
            game_key: GameKey {
                game_id: "arena".to_string(),
                game_version: 12,
            },
            name: "twelve".to_string(),
            host: "ender".to_string(),
            has_password: false,
            players: 1,
            cap: 8,
        },
        LegacyLobbyEntry {
            game_key: GameKey {
                game_id: "arena".to_string(),
                game_version: 9,
            },
            name: "nine".to_string(),
            host: "bean".to_string(),
            has_password: false,
            players: 2,
            cap: 8,
        },
    ];
    let S2C::LobbyList { lobbies } = ArenaLegacyDecoder::project_lobby_list(9, &entries) else {
        panic!("projection produced the wrong message variant");
    };
    assert_eq!(lobbies.len(), 1);
    assert_eq!(lobbies[0].name, "nine");
}
