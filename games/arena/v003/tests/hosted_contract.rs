use std::collections::BTreeMap;
use std::sync::Arc;

use ember_game_arena_v3::proto::{C2S, S2C};
use ember_game_arena_v3::{ArenaCodec, ArenaFactory, FIXTURE_SUITE_ID};
use ember_legacy::{
    AdmissionMetadata, BroadcastHandle, GameFactory, GameKey, InnerCodec, InnerFrame,
    LegacyCapabilities, LegacyClock, LegacyTransport, LobbySeed, MetricObservation,
    MonotonicTimestamp, PeerId, ScheduleError, ScheduleHandle, SchedulingRequest,
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

#[derive(Deserialize)]
struct SuiteFixture {
    id: String,
    game_id: String,
    game_version: u32,
    c2s: String,
    s2c: String,
    lobby_refusal: String,
    deterministic_trace: String,
}

#[derive(Deserialize)]
struct LobbyRefusalFixture {
    joined: String,
    duplicate: RefusalFixture,
    full: RefusalFixture,
    status: StatusFixture,
}

#[derive(Deserialize)]
struct RefusalFixture {
    code: String,
    message: String,
}

#[derive(Deserialize)]
struct StatusFixture {
    code: String,
    detail: Option<String>,
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
        random: Arc::new(ember_legacy::FrozenKeyedRandom),
        transport: Arc::new(TestTransport),
        assets: None,
    }
}

fn creation(seed: u64, created_at_micros: u64) -> SessionCreationData {
    let mut seed_bytes = [0_u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    SessionCreationData {
        game_key: GameKey {
            game_id: "arena".to_string(),
            game_version: 3,
        },
        session_id: ember_legacy::SessionId::from_host_value(7),
        lobby_name: "trace".to_string(),
        lobby_seed: LobbySeed(seed_bytes),
        created_at: MonotonicTimestamp::from_micros(created_at_micros),
        configured_rules: Vec::new(),
    }
}

fn admission(peer: u64, handle: &str, at: MonotonicTimestamp) -> AdmissionMetadata {
    AdmissionMetadata {
        peer_id: PeerId::from_host_value(peer),
        handle: handle.to_string(),
        admitted_at: at,
        attributes: BTreeMap::new(),
    }
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
fn suite_identity_and_file_names_are_stable() {
    let fixture: SuiteFixture =
        serde_json::from_str(SUITE_FIXTURE).expect("suite fixture must decode");
    assert_eq!(fixture.id, FIXTURE_SUITE_ID);
    assert_eq!(fixture.game_id, "arena");
    assert_eq!(fixture.game_version, 3);
    assert_eq!(fixture.c2s, "c2s.jsonl");
    assert_eq!(fixture.s2c, "s2c.jsonl");
    assert_eq!(fixture.lobby_refusal, "lobby_refusal.json");
    assert_eq!(fixture.deterministic_trace, "deterministic_trace.json");
}

#[test]
fn every_wire_message_has_an_exact_json_fixture() {
    let c2s_kinds: Vec<_> = C2S_FIXTURES
        .lines()
        .map(|line| {
            let message: C2S = serde_json::from_str(line).expect("C2S fixture must decode");
            assert_eq!(
                serde_json::to_string(&message).expect("C2S fixture must encode"),
                line
            );
            c2s_kind(&message)
        })
        .collect();
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

    let s2c_kinds: Vec<_> = S2C_FIXTURES
        .lines()
        .map(|line| {
            let message: S2C = serde_json::from_str(line).expect("S2C fixture must decode");
            assert_eq!(
                serde_json::to_string(&message).expect("S2C fixture must encode"),
                line
            );
            s2c_kind(&message)
        })
        .collect();
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
        let decoded = codec
            .decode(&InnerFrame::Text(line.to_string()))
            .expect("text C2S fixture must decode");
        let message: C2S =
            serde_json::from_slice(&decoded.payload).expect("decoded C2S must be valid");
        assert_eq!(
            serde_json::to_string(&message).expect("decoded C2S must encode"),
            line
        );
    }
    for line in S2C_FIXTURES.lines() {
        let message: S2C = serde_json::from_str(line).expect("S2C fixture must decode");
        let event = ember_legacy::EncodedEvent {
            payload: serde_json::to_vec(&message).expect("S2C fixture must encode"),
        };
        assert_eq!(
            codec.encode(&event).expect("S2C fixture must pass codec"),
            InnerFrame::Text(line.to_string())
        );
    }
    assert_eq!(
        codec.decode(&InnerFrame::Binary(Vec::new())),
        Err(ember_legacy::InnerCodecError::WrongFrameKind)
    );
}

#[test]
fn lobby_join_status_and_refusal_fixtures_are_frozen() {
    let fixture: LobbyRefusalFixture =
        serde_json::from_str(LOBBY_REFUSAL_FIXTURE).expect("lobby and refusal fixture must decode");
    let creation = creation(42, 1_000_000);
    let mut session = ArenaFactory::new()
        .create(&test_capabilities(), &creation)
        .expect("fixture session must construct");
    let first = admission(0, "ender", creation.created_at);
    let joined = session.join(first.clone()).expect("first player must join");
    let codec = ArenaCodec::new();
    assert_eq!(
        codec
            .encode(&joined.outbound[0].event)
            .expect("join response must encode"),
        InnerFrame::Text(fixture.joined)
    );

    let duplicate = session
        .join(first)
        .expect_err("the same peer cannot join twice");
    assert_eq!(duplicate.code, fixture.duplicate.code);
    assert_eq!(duplicate.message, fixture.duplicate.message);

    for peer in 1..8_u64 {
        session
            .join(admission(peer, "guest", creation.created_at))
            .expect("players up to the frozen capacity must join");
    }
    let full = session
        .join(admission(8, "overflow", creation.created_at))
        .expect_err("the ninth player must be refused");
    assert_eq!(full.code, fixture.full.code);
    assert_eq!(full.message, fixture.full.message);

    let status = session.lobby_status();
    assert_eq!(status.code, fixture.status.code);
    assert_eq!(status.detail, fixture.status.detail);
}

#[test]
fn timestamped_transcript_produces_authoritative_checkpoints() {
    let trace: TraceFixture =
        serde_json::from_str(TRACE_FIXTURE).expect("trace fixture must decode");
    let creation = creation(trace.seed, trace.created_at_micros);
    let mut session = ArenaFactory::new()
        .create(&test_capabilities(), &creation)
        .expect("trace session must construct");
    let peer_id = PeerId::from_host_value(11);
    let codec = ArenaCodec::new();
    let joined = session
        .join(admission(11, "ender", creation.created_at))
        .expect("trace peer must join");
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
                input: codec
                    .decode(&InnerFrame::Text(frame))
                    .expect("trace input must decode"),
            })
            .collect();
        let update = session.step(timestamp, inputs);
        let state = state_from_update(&codec, &update);
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
    }
}
