use std::collections::BTreeMap;

use ember_game_fire_v1::hosted::{
    FIXTURE_SUITE_ID, FireCodec, FireRules, FireSession, GAME_ID, GAME_VERSION, game_key,
};
use ember_game_fire_v1::legacy::{
    LegacyFireAction, LegacyFireAdapter, LegacyFireIngressFactory, LegacyLobby, project_lobbies,
};
use ember_game_fire_v1::proto::{C2S, S2C};
use ember_game_fire_v1::sim::RaceState;
use ember_legacy::{
    AdmissionMetadata, GameSession, InnerCodec, InnerFrame, LegacyConnectionState,
    LegacyIngressAction, LegacyIngressFactory, LegacyLobbyProjection, LobbySeed, LobbyStatus,
    MonotonicTimestamp, PeerId, SessionInput,
};
use serde::Deserialize;

const SUITE: &str = include_str!("fixtures/fire-v1-hosted-contract/suite.json");
const CLIENT_TO_SERVER: &str =
    include_str!("fixtures/fire-v1-hosted-contract/client-to-server.jsonl");
const SERVER_TO_CLIENT: &str =
    include_str!("fixtures/fire-v1-hosted-contract/server-to-client.jsonl");
const LEGACY_LOBBIES: &str = include_str!("fixtures/fire-v1-hosted-contract/legacy-lobbies.json");
const LEGACY_REJECTED: &str = include_str!("fixtures/fire-v1-hosted-contract/legacy-rejected.json");
const LEGACY_TRANSCRIPT: &str =
    include_str!("fixtures/fire-v1-hosted-contract/legacy-transcript.jsonl");
const DETERMINISTIC_TRACE: &str =
    include_str!("fixtures/fire-v1-hosted-contract/deterministic-trace.json");

#[derive(Deserialize)]
struct SuiteFixture {
    fixture_suite: String,
    game_id: String,
    game_version: u32,
    client_to_server: String,
    server_to_client: String,
    legacy_lobbies: String,
    legacy_rejected: String,
    legacy_transcript: String,
    deterministic_trace: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Client,
    Server,
}

#[derive(Deserialize)]
struct TranscriptLine {
    connection: String,
    direction: Direction,
    frame: String,
}

#[derive(Deserialize)]
struct TraceFixture {
    fixture_suite: String,
    seed: [u8; 32],
    created_at_micros: u64,
    tick_interval_micros: u64,
    end_at_micros: u64,
    messages: Vec<TraceMessage>,
    checkpoints: Vec<TraceCheckpoint>,
}

#[derive(Deserialize)]
struct TraceMessage {
    at_micros: u64,
    message: C2S,
}

#[derive(Deserialize)]
struct TraceCheckpoint {
    at_micros: u64,
    tick: u64,
    phase: String,
    boost_charges: u8,
    boosting: bool,
    last_state_ack: Option<u32>,
}

#[test]
fn suite_identity_matches_the_hosted_manifest_entry() {
    let suite: SuiteFixture = serde_json::from_str(SUITE).expect("suite fixture must decode");
    assert_eq!(suite.fixture_suite, FIXTURE_SUITE_ID);
    assert_eq!(suite.game_id, GAME_ID);
    assert_eq!(suite.game_version, GAME_VERSION);
    assert_eq!(suite.client_to_server, "client-to-server.jsonl");
    assert_eq!(suite.server_to_client, "server-to-client.jsonl");
    assert_eq!(suite.legacy_lobbies, "legacy-lobbies.json");
    assert_eq!(suite.legacy_rejected, "legacy-rejected.json");
    assert_eq!(suite.legacy_transcript, "legacy-transcript.jsonl");
    assert_eq!(suite.deterministic_trace, "deterministic-trace.json");
}

#[test]
fn client_to_server_frames_are_exact_protocol_1_json_text() {
    let codec = FireCodec;
    for frame in CLIENT_TO_SERVER.lines() {
        let message: C2S = serde_json::from_str(frame).expect("client fixture must decode");
        assert_eq!(
            serde_json::to_string(&message).expect("client fixture must encode"),
            frame
        );
        let decoded = codec
            .decode(&InnerFrame::Text(frame.to_string()))
            .expect("text fixture must pass the Fire codec");
        assert_eq!(decoded.payload, frame.as_bytes());
    }
}

#[test]
fn server_to_client_frames_are_exact_protocol_1_json_text() {
    let codec = FireCodec;
    for frame in SERVER_TO_CLIENT.lines() {
        let message: S2C = serde_json::from_str(frame).expect("server fixture must decode");
        let payload = serde_json::to_vec(&message).expect("server fixture must encode");
        assert_eq!(payload, frame.as_bytes());
        assert_eq!(
            codec
                .encode(&ember_legacy::EncodedEvent { payload })
                .expect("server fixture must pass the Fire codec"),
            InnerFrame::Text(frame.to_string())
        );
    }
}

#[test]
fn legacy_lobby_and_refusal_projections_are_frozen() {
    let projected = project_lobbies(&[LegacyLobby {
        name: "castle".to_string(),
        host: "driver".to_string(),
        has_password: true,
        players: 2,
        cap: 8,
        racing: true,
    }])
    .expect("legacy lobby projection must encode");
    assert_eq!(projected.payload, LEGACY_LOBBIES.trim_end().as_bytes());

    let mut adapter = LegacyFireAdapter::default();
    let hello = InnerFrame::Text(r#"{"t":"hello","proto":1,"handle":"driver"}"#.to_string());
    match adapter.decode(&hello).expect("legacy hello must decode") {
        LegacyFireAction::Hello {
            selection,
            handle,
            welcome,
        } => {
            assert_eq!(selection, game_key());
            assert_eq!(handle, "driver");
            assert_eq!(welcome.payload, br#"{"t":"welcome","proto":1}"#);
        }
        other => panic!("wrong legacy hello action: {other:?}"),
    }

    let join =
        InnerFrame::Text(r#"{"t":"join_lobby","name":"castle","password":"ember"}"#.to_string());
    match adapter.decode(&join).expect("legacy join must decode") {
        LegacyFireAction::JoinLobby(request) => {
            assert_eq!(request.selection, game_key());
            assert_eq!(request.lobby_name, "castle");
            assert_eq!(request.password.as_deref(), Some("ember"));
        }
        other => panic!("wrong legacy join action: {other:?}"),
    }

    let mut stale = LegacyFireAdapter::default();
    let stale_hello = InnerFrame::Text(r#"{"t":"hello","proto":2,"handle":"driver"}"#.to_string());
    stale
        .decode(&stale_hello)
        .expect("stale legacy hello still receives welcome");
    match stale
        .decode(&join)
        .expect("stale join must become a refusal")
    {
        LegacyFireAction::Reply(event) => {
            assert_eq!(event.payload, LEGACY_REJECTED.trim_end().as_bytes());
        }
        other => panic!("wrong stale-client action: {other:?}"),
    }
}

#[test]
fn shared_legacy_ingress_preserves_fire_frames() {
    let mut ingress = LegacyFireIngressFactory.create();
    let hello = InnerFrame::Text(r#"{"t":"hello","proto":1,"handle":"driver"}"#.to_string());
    let action = ingress
        .decode(LegacyConnectionState::AwaitHello, &hello)
        .unwrap();
    let LegacyIngressAction::Hello { response, .. } = action else {
        panic!("shared Fire hello decoded to the wrong action");
    };
    assert_eq!(
        response,
        InnerFrame::Text(r#"{"t":"welcome","proto":1}"#.to_string())
    );
    let projected = ingress
        .project_lobbies(&[LegacyLobbyProjection {
            game_key: game_key(),
            lobby_name: "castle".to_string(),
            host_handle: "driver".to_string(),
            password_protected: true,
            occupancy: 2,
            capacity: 8,
            status: LobbyStatus {
                code: "racing".to_string(),
                detail: None,
            },
        }])
        .unwrap();
    assert_eq!(
        projected,
        InnerFrame::Text(LEGACY_LOBBIES.trim_end().to_string())
    );
}

#[test]
fn frozen_client_transcript_contains_only_canonical_legacy_frames() {
    let mut browser_frames = 0;
    let mut game_frames = 0;
    for raw in LEGACY_TRANSCRIPT.lines() {
        let line: TranscriptLine =
            serde_json::from_str(raw).expect("legacy transcript line must decode");
        match line.connection.as_str() {
            "browser" => browser_frames += 1,
            "game" => game_frames += 1,
            other => panic!("unknown transcript connection: {other}"),
        }
        match line.direction {
            Direction::Client => {
                let message: C2S =
                    serde_json::from_str(&line.frame).expect("client transcript frame must decode");
                assert_eq!(
                    serde_json::to_string(&message).expect("client transcript frame must encode"),
                    line.frame
                );
            }
            Direction::Server => {
                let message: S2C =
                    serde_json::from_str(&line.frame).expect("server transcript frame must decode");
                assert_eq!(
                    serde_json::to_string(&message).expect("server transcript frame must encode"),
                    line.frame
                );
            }
        }
    }
    assert_eq!(browser_frames, 4);
    assert_eq!(game_frames, 5);
}

#[test]
fn seeded_timestamped_trace_reaches_frozen_authoritative_checkpoints() {
    let trace: TraceFixture =
        serde_json::from_str(DETERMINISTIC_TRACE).expect("trace fixture must decode");
    assert_eq!(trace.fixture_suite, FIXTURE_SUITE_ID);
    let peer_id = PeerId::from_host_value(7);
    let created_at = MonotonicTimestamp::from_micros(trace.created_at_micros);
    let mut session = FireSession::new(
        FireRules::default(),
        "castle".to_string(),
        LobbySeed(trace.seed),
        created_at,
    );
    let joined = session
        .join(AdmissionMetadata {
            peer_id,
            handle: "driver".to_string(),
            admitted_at: created_at,
            attributes: BTreeMap::new(),
        })
        .expect("fixture peer must join");
    assert_eq!(
        joined.outbound[0].event.payload,
        SERVER_TO_CLIENT
            .lines()
            .nth(3)
            .expect("joined fixture")
            .as_bytes()
    );

    let elapsed = trace.end_at_micros - trace.created_at_micros;
    assert_eq!(elapsed % trace.tick_interval_micros, 0);
    let steps = elapsed / trace.tick_interval_micros;
    let mut last_state_ack = None;
    let mut checkpoints_seen = 0;
    for step in 1..=steps {
        let at_micros = trace.created_at_micros + step * trace.tick_interval_micros;
        let inputs = trace
            .messages
            .iter()
            .filter(|entry| entry.at_micros == at_micros)
            .map(|entry| SessionInput {
                peer_id,
                received_at: MonotonicTimestamp::from_micros(entry.at_micros),
                input: ember_legacy::DecodedInput {
                    payload: serde_json::to_vec(&entry.message).expect("trace message must encode"),
                },
            })
            .collect();
        let update = session.step(MonotonicTimestamp::from_micros(at_micros), inputs);
        for outbound in update.outbound {
            if let Ok(S2C::State { cars, .. }) =
                serde_json::from_slice::<S2C>(&outbound.event.payload)
                && let Some(car) = cars.iter().find(|car| car.id == 0)
            {
                last_state_ack = Some(car.ack);
            }
        }

        for checkpoint in trace
            .checkpoints
            .iter()
            .filter(|checkpoint| checkpoint.at_micros == at_micros)
        {
            checkpoints_seen += 1;
            let race = session.race();
            let car = &race.racers[0].car;
            assert_eq!(race.tick, checkpoint.tick);
            assert_eq!(phase_name(race.state), checkpoint.phase);
            assert_eq!(car.boost_charges, checkpoint.boost_charges);
            assert_eq!(car.boosting(), checkpoint.boosting);
            assert_eq!(last_state_ack, checkpoint.last_state_ack);
        }
    }
    assert_eq!(checkpoints_seen, trace.checkpoints.len());
}

const fn phase_name(state: RaceState) -> &'static str {
    match state {
        RaceState::Waiting => "waiting",
        RaceState::Countdown => "countdown",
        RaceState::Racing => "racing",
        RaceState::Finished => "finished",
    }
}
