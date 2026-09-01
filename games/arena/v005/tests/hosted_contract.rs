use std::collections::BTreeMap;
use std::sync::Arc;

use ember_game_arena_v5::proto::{C2S, S2C};
use ember_game_arena_v5::{ArenaCodec, ArenaFactory, FIXTURE_SUITE_ID};
use ember_legacy::{
    AdmissionMetadata, BroadcastHandle, GameFactory, GameKey, InnerCodec, InnerFrame,
    LegacyCapabilities, LegacyClock, LegacyRandom, LegacyTransport, LobbySeed, MetricObservation,
    MonotonicTimestamp, PeerId, RandomDrawKey, ScheduleError, ScheduleHandle, SchedulingRequest,
    SessionCreationData, SessionInput, TransportError, UnicastHandle,
};
use serde::Deserialize;

const C2S_FIXTURES: &str = include_str!("fixtures/c2s.jsonl");
const S2C_FIXTURES: &str = include_str!("fixtures/s2c.jsonl");
const LOBBY_REFUSAL_FIXTURE: &str = include_str!("fixtures/lobby_refusal.json");
const TRACE_FIXTURE: &str = include_str!("fixtures/deterministic_trace.json");
const SUITE_FIXTURE: &str = include_str!("fixtures/suite.json");

fn c2s_kind(message: &C2S) -> &'static str {
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

fn s2c_kind(message: &S2C) -> &'static str {
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
fn every_v5_wire_message_has_a_canonical_json_fixture() {
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

fn test_capabilities() -> LegacyCapabilities {
    LegacyCapabilities {
        clock: Arc::new(TestClock),
        random: Arc::new(TestRandom),
        transport: Arc::new(TestTransport),
        assets: None,
    }
}

fn creation(seed: u64, created_at: MonotonicTimestamp) -> SessionCreationData {
    let mut seed_bytes = [0_u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    SessionCreationData {
        game_key: GameKey {
            game_id: "arena".to_string(),
            game_version: 5,
        },
        session_id: ember_legacy::SessionId::from_host_value(7),
        lobby_name: "trace".to_string(),
        lobby_seed: LobbySeed(seed_bytes),
        created_at,
        configured_rules: Vec::new(),
    }
}

fn admission(peer_id: PeerId, admitted_at: MonotonicTimestamp) -> AdmissionMetadata {
    AdmissionMetadata {
        peer_id,
        handle: "ender".to_string(),
        admitted_at,
        attributes: BTreeMap::new(),
    }
}

#[derive(Deserialize)]
struct SuiteFixture {
    id: String,
    game_id: String,
    game_version: u32,
}

#[derive(Deserialize)]
struct LobbyRefusalFixture {
    joined: String,
    duplicate_code: String,
    duplicate_message: String,
    status_code: String,
}

#[test]
fn suite_lobby_and_admission_refusal_fixtures_are_frozen() {
    let suite: SuiteFixture = serde_json::from_str(SUITE_FIXTURE).unwrap();
    assert_eq!(suite.id, FIXTURE_SUITE_ID);
    assert_eq!(suite.game_id, "arena");
    assert_eq!(suite.game_version, 5);

    let fixture: LobbyRefusalFixture =
        serde_json::from_str(LOBBY_REFUSAL_FIXTURE).unwrap();
    let created_at = MonotonicTimestamp::from_micros(1_000_000);
    let creation = creation(42, created_at);
    let mut session = ArenaFactory::new()
        .create(&test_capabilities(), &creation)
        .unwrap();
    let peer_id = PeerId::from_host_value(11);
    let joined = session.join(admission(peer_id, created_at)).unwrap();
    let joined_frame = ArenaCodec::new()
        .encode(&joined.outbound[0].event)
        .unwrap();
    assert_eq!(joined_frame, InnerFrame::Text(fixture.joined));
    assert_eq!(session.lobby_status().code, fixture.status_code);

    let Err(refusal) = session.join(admission(peer_id, created_at)) else {
        panic!("duplicate admission unexpectedly succeeded");
    };
    assert_eq!(refusal.code, fixture.duplicate_code);
    assert_eq!(refusal.message, fixture.duplicate_message);
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
    ack: u32,
}

fn state_from_update(codec: &ArenaCodec, update: &ember_legacy::SessionUpdate) -> Option<S2C> {
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
    let created_at = MonotonicTimestamp::from_micros(trace.created_at_micros);
    let creation = creation(trace.seed, created_at);
    let mut session = ArenaFactory::new()
        .create(&test_capabilities(), &creation)
        .unwrap();
    let peer_id = PeerId::from_host_value(11);
    let codec = ArenaCodec::new();
    let joined = session.join(admission(peer_id, created_at)).unwrap();
    assert_eq!(
        joined.scheduling,
        [SchedulingRequest::At(MonotonicTimestamp::from_micros(
            1_016_667
        ))]
    );

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
        let state = state_from_update(&codec, &update);
        let Some(checkpoint) = call.checkpoint else {
            assert!(state.is_none());
            continue;
        };
        let Some(S2C::State {
            tick,
            mut players,
            ..
        }) = state
        else {
            panic!("checkpoint call did not broadcast state");
        };
        assert_eq!(tick, checkpoint.tick);
        assert_eq!(players.len(), 1);
        let player = players.remove(0);
        assert!(close_enough(player.x, checkpoint.x));
        assert!(close_enough(player.z, checkpoint.z));
        assert_eq!(player.ack, checkpoint.ack);
    }
}
