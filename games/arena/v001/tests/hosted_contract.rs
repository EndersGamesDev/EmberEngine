// Frozen checkpoint values are intentionally compared bit-for-bit.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;
use std::sync::Arc;

use ember_game_arena_v1::hosted::{
    ArenaCodec, ArenaFactory, ArenaSession, FIXTURE_SUITE_ID, GAME_ID, GAME_VERSION, game_key,
};
use ember_game_arena_v1::proto::{C2S, S2C};
use ember_legacy::{
    AdmissionMetadata, BroadcastHandle, CloseReason, GameFactory, GameSession, InnerCodec,
    InnerFrame, LegacyCapabilities, LegacyClock, LegacyRandom, LegacyTransport, LobbySeed,
    MetricObservation, MonotonicTimestamp, PeerId, RandomDrawKey, ScheduleError, ScheduleHandle,
    SchedulingRequest, SessionCreationData, SessionInput, TransportError, UnicastHandle,
};
use serde::Deserialize;

const SUITE: &str = include_str!("fixtures/arena-v1-hosted-contract/suite.json");
const CLIENT_TO_SERVER: &str =
    include_str!("fixtures/arena-v1-hosted-contract/client-to-server.jsonl");
const SERVER_TO_CLIENT: &str =
    include_str!("fixtures/arena-v1-hosted-contract/server-to-client.jsonl");
const LOBBY: &str = include_str!("fixtures/arena-v1-hosted-contract/lobby.json");
const TRACE: &str = include_str!("fixtures/arena-v1-hosted-contract/deterministic-trace.json");

#[derive(Deserialize)]
struct SuiteFixture {
    fixture_suite: String,
    game_id: String,
    game_version: u32,
    client_to_server: String,
    server_to_client: String,
    lobby: String,
    deterministic_trace: String,
}

#[derive(Deserialize)]
struct LobbyFixture {
    created: String,
    host_start: String,
    guest_start: String,
    full_refusal_code: String,
    full_refusal_message: String,
    waiting_status: String,
    playing_status: String,
    opponent_left: String,
}

#[derive(Deserialize)]
struct TraceFixture {
    fixture_suite: String,
    seed: [u8; 32],
    created_at_micros: u64,
    calls: Vec<TraceCall>,
}

#[derive(Deserialize)]
struct TraceCall {
    timestamp_micros: u64,
    frames: Vec<TraceFrame>,
    checkpoint: Option<TraceCheckpoint>,
}

#[derive(Deserialize)]
struct TraceFrame {
    peer: String,
    frame: String,
}

#[derive(Deserialize)]
struct TraceCheckpoint {
    tick: u64,
    paddles: [f32; 2],
    ball: [f32; 2],
    scores: [u32; 2],
    serving: bool,
}

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
        S2C::LobbyCreated { .. } => "lobby_created",
        S2C::MatchStart { .. } => "match_start",
        S2C::State { .. } => "state",
        S2C::MatchEvent { .. } => "match_event",
        S2C::OpponentLeft => "opponent_left",
        S2C::Pong { .. } => "pong",
    }
}

#[test]
fn suite_identity_matches_the_hosted_manifest_entry() {
    let suite: SuiteFixture = serde_json::from_str(SUITE).unwrap();
    assert_eq!(suite.fixture_suite, FIXTURE_SUITE_ID);
    assert_eq!(suite.game_id, GAME_ID);
    assert_eq!(suite.game_version, GAME_VERSION);
    assert_eq!(suite.client_to_server, "client-to-server.jsonl");
    assert_eq!(suite.server_to_client, "server-to-client.jsonl");
    assert_eq!(suite.lobby, "lobby.json");
    assert_eq!(suite.deterministic_trace, "deterministic-trace.json");
}

#[test]
fn every_protocol_1_wire_message_has_an_exact_json_fixture() {
    let client_kinds = CLIENT_TO_SERVER
        .lines()
        .map(|frame| {
            let message: C2S = serde_json::from_str(frame).unwrap();
            assert_eq!(serde_json::to_string(&message).unwrap(), frame);
            c2s_kind(&message)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        client_kinds,
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

    let server_kinds = SERVER_TO_CLIENT
        .lines()
        .map(|frame| {
            let message: S2C = serde_json::from_str(frame).unwrap();
            assert_eq!(serde_json::to_string(&message).unwrap(), frame);
            s2c_kind(&message)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        server_kinds,
        [
            "welcome",
            "error",
            "lobby_list",
            "lobby_created",
            "match_start",
            "state",
            "match_event",
            "opponent_left",
            "pong",
        ]
    );
}

#[test]
fn codec_preserves_exact_protocol_1_json_text_frames() {
    let codec = ArenaCodec::new();
    for frame in CLIENT_TO_SERVER.lines() {
        let decoded = codec.decode(&InnerFrame::Text(frame.to_string())).unwrap();
        assert_eq!(decoded.payload, frame.as_bytes());
    }
    for frame in SERVER_TO_CLIENT.lines() {
        let event = ember_legacy::EncodedEvent {
            payload: frame.as_bytes().to_vec(),
        };
        assert_eq!(
            codec.encode(&event).unwrap(),
            InnerFrame::Text(frame.to_string())
        );
    }
    assert_eq!(
        codec.decode(&InnerFrame::Binary(Vec::new())),
        Err(ember_legacy::InnerCodecError::WrongFrameKind)
    );
}

fn creation(seed: [u8; 32], created_at: MonotonicTimestamp) -> SessionCreationData {
    SessionCreationData {
        game_key: game_key(),
        session_id: ember_legacy::SessionId::from_host_value(9),
        lobby_name: "duel".to_string(),
        lobby_seed: LobbySeed(seed),
        created_at,
        configured_rules: Vec::new(),
    }
}

fn admission(peer_id: PeerId, handle: &str, at: MonotonicTimestamp) -> AdmissionMetadata {
    AdmissionMetadata {
        peer_id,
        handle: handle.to_string(),
        admitted_at: at,
        attributes: BTreeMap::new(),
    }
}

#[test]
fn lobby_join_refusal_and_reopen_match_the_era_server() {
    let fixture: LobbyFixture = serde_json::from_str(LOBBY).unwrap();
    let created_at = MonotonicTimestamp::from_micros(1_000_000);
    let mut session = ArenaSession::new("duel".to_string(), LobbySeed([0; 32]), created_at);
    let host = PeerId::from_host_value(1);
    let guest = PeerId::from_host_value(2);
    let extra = PeerId::from_host_value(3);

    let created = session.join(admission(host, "alice", created_at)).unwrap();
    assert_eq!(
        created.outbound[0].event.payload,
        fixture.created.as_bytes()
    );
    assert_eq!(session.lobby_status().code, fixture.waiting_status);
    assert_eq!(
        created.scheduling,
        [SchedulingRequest::At(MonotonicTimestamp::from_micros(
            1_016_667
        ))]
    );

    let started = session.join(admission(guest, "bob", created_at)).unwrap();
    assert_eq!(
        started.outbound[0].event.payload,
        fixture.host_start.as_bytes()
    );
    assert_eq!(
        started.outbound[1].event.payload,
        fixture.guest_start.as_bytes()
    );
    assert_eq!(session.lobby_status().code, fixture.playing_status);

    let refusal = session
        .join(admission(extra, "carol", created_at))
        .unwrap_err();
    assert_eq!(refusal.code, fixture.full_refusal_code);
    assert_eq!(refusal.message, fixture.full_refusal_message);

    let left = session.leave(
        guest,
        ember_legacy::LeaveReason {
            close_reason: CloseReason::Disconnected,
            detail: None,
        },
    );
    assert_eq!(
        left.outbound[0].event.payload,
        fixture.opponent_left.as_bytes()
    );
    assert_eq!(session.lobby_status().code, "waiting");
}

struct PanicClock;

impl LegacyClock for PanicClock {
    fn now(&self) -> MonotonicTimestamp {
        panic!("Arena v1 must not consume the clock capability");
    }

    fn request_schedule(
        &self,
        _request: SchedulingRequest,
    ) -> Result<ScheduleHandle, ScheduleError> {
        panic!("Arena v1 must not consume the clock capability");
    }

    fn cancel_schedule(&self, _handle: ScheduleHandle) -> Result<(), ScheduleError> {
        panic!("Arena v1 must not consume the clock capability");
    }
}

struct PanicRandom;

impl LegacyRandom for PanicRandom {
    fn draw_u64(&self, _key: &RandomDrawKey) -> u64 {
        panic!("Arena v1 must not consume the random capability");
    }

    fn fill_bytes(&self, _key: &RandomDrawKey, _output: &mut [u8]) {
        panic!("Arena v1 must not consume the random capability");
    }
}

struct PanicTransport;

impl LegacyTransport for PanicTransport {
    fn unicast(&self, _peer_id: PeerId) -> Result<UnicastHandle, TransportError> {
        panic!("Arena v1 must not consume the transport capability");
    }

    fn broadcast(
        &self,
        _session_id: ember_legacy::SessionId,
    ) -> Result<BroadcastHandle, TransportError> {
        panic!("Arena v1 must not consume the transport capability");
    }

    fn close_peer(&self, _peer_id: PeerId, _reason: CloseReason) -> Result<(), TransportError> {
        panic!("Arena v1 must not consume the transport capability");
    }

    fn record_metric(&self, _observation: MetricObservation) {
        panic!("Arena v1 must not consume the transport capability");
    }
}

#[test]
fn factory_constructs_without_consuming_any_capability_surface() {
    let capabilities = LegacyCapabilities {
        clock: Arc::new(PanicClock),
        random: Arc::new(PanicRandom),
        transport: Arc::new(PanicTransport),
        assets: None,
    };
    let creation = creation([7; 32], MonotonicTimestamp::from_micros(50));
    ArenaFactory::new().create(&capabilities, &creation).unwrap();
}

#[test]
fn timestamped_inputs_produce_frozen_authoritative_checkpoints() {
    let trace: TraceFixture = serde_json::from_str(TRACE).unwrap();
    assert_eq!(trace.fixture_suite, FIXTURE_SUITE_ID);
    let created_at = MonotonicTimestamp::from_micros(trace.created_at_micros);
    let mut session = ArenaSession::new("duel".to_string(), LobbySeed(trace.seed), created_at);
    let host = PeerId::from_host_value(1);
    let guest = PeerId::from_host_value(2);
    session.join(admission(host, "alice", created_at)).unwrap();
    session.join(admission(guest, "bob", created_at)).unwrap();
    let codec = ArenaCodec::new();

    for call in trace.calls {
        let timestamp = MonotonicTimestamp::from_micros(call.timestamp_micros);
        let inputs = call
            .frames
            .into_iter()
            .map(|entry| SessionInput {
                peer_id: match entry.peer.as_str() {
                    "host" => host,
                    "guest" => guest,
                    other => panic!("unknown fixture peer {other}"),
                },
                received_at: timestamp,
                input: codec.decode(&InnerFrame::Text(entry.frame)).unwrap(),
            })
            .collect();
        let update = session.step(timestamp, inputs);
        let state = update.outbound.iter().find_map(|outbound| {
            serde_json::from_slice::<S2C>(&outbound.event.payload)
                .ok()
                .and_then(|message| match message {
                    state @ S2C::State { .. } => Some(state),
                    _ => None,
                })
        });
        let Some(checkpoint) = call.checkpoint else {
            assert!(state.is_none());
            continue;
        };
        let Some(S2C::State {
            tick,
            ball,
            paddles,
            scores,
            serving,
        }) = state
        else {
            panic!("checkpoint call did not broadcast state");
        };
        assert_eq!(tick, checkpoint.tick);
        assert_eq!(paddles, checkpoint.paddles);
        assert_eq!(ball, checkpoint.ball);
        assert_eq!(scores, checkpoint.scores);
        assert_eq!(serving, checkpoint.serving);
        assert_eq!(session.tick(), checkpoint.tick);
    }
}
