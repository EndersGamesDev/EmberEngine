//! Minimal compiled fixture version for host tests and feature-gated demonstrations.

use std::collections::BTreeSet;
use std::sync::Arc;

use ember_legacy::{
    AdmissionMetadata, AdmissionRefusal, DecodedInput, EncodedEvent, FactoryError, GameFactory,
    GameKey, GameSession, InnerCodec, InnerCodecError, InnerFrame, LeaveReason, LegacyTransport,
    LobbyStatus, MetricObservation, MonotonicDuration, MonotonicTimestamp, OutboundEvent,
    OutboundTarget, PeerId, SchedulingRequest, SessionCreationData, SessionInput, SessionUpdate,
    VersionLimits,
};

use crate::{RegistryBuilder, RegistryError, RegistryRegistration};

pub(crate) const MANIFEST: &str = r#"
[[games]]
game_id = "fixture"
game_version = 1
package = "ember-server"
latest = true
limits_profile = "fixture-bounded"
fixture_suite = "fixture-hosted-contract"
"#;

pub(crate) fn register(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    builder.register(RegistryRegistration::new(
        fixture_key(),
        fixture_limits(),
        Arc::new(FixtureCodec),
        Arc::new(FixtureFactory),
    ))
}

pub(crate) fn fixture_key() -> GameKey {
    GameKey {
        game_id: "fixture".to_string(),
        game_version: 1,
    }
}

pub(crate) const fn fixture_limits() -> VersionLimits {
    VersionLimits {
        max_lobbies: 4,
        max_players_per_lobby: 4,
        max_frame_bytes: 1_024,
        max_messages_per_second: 64,
        max_outbound_queue_bytes: 8 * 1_024,
        max_outbound_bytes_per_second: 64 * 1_024,
        max_step_duration: MonotonicDuration::from_micros(25_000),
    }
}

struct FixtureCodec;

impl InnerCodec for FixtureCodec {
    fn decode(&self, frame: &InnerFrame) -> Result<DecodedInput, InnerCodecError> {
        let mut payload = Vec::with_capacity(frame.len().saturating_add(1));
        match frame {
            InnerFrame::Text(text) => {
                payload.push(0);
                payload.extend_from_slice(text.as_bytes());
            }
            InnerFrame::Binary(bytes) => {
                payload.push(1);
                payload.extend_from_slice(bytes);
            }
        }
        Ok(DecodedInput { payload })
    }

    fn encode(&self, event: &EncodedEvent) -> Result<InnerFrame, InnerCodecError> {
        let Some((kind, payload)) = event.payload.split_first() else {
            return Err(InnerCodecError::EncodeFailed(
                "fixture event has no frame-kind byte".to_string(),
            ));
        };
        match kind {
            0 => String::from_utf8(payload.to_vec())
                .map(InnerFrame::Text)
                .map_err(|error| InnerCodecError::EncodeFailed(error.to_string())),
            1 => Ok(InnerFrame::Binary(payload.to_vec())),
            _ => Err(InnerCodecError::EncodeFailed(
                "fixture event has an unknown frame kind".to_string(),
            )),
        }
    }
}

struct FixtureFactory;

impl GameFactory for FixtureFactory {
    fn create(
        &self,
        capabilities: &ember_legacy::LegacyCapabilities,
        _creation: &SessionCreationData,
    ) -> Result<Box<dyn GameSession>, FactoryError> {
        Ok(Box::new(FixtureSession {
            peers: BTreeSet::new(),
            transport: Arc::clone(&capabilities.transport),
        }))
    }
}

struct FixtureSession {
    peers: BTreeSet<PeerId>,
    transport: Arc<dyn LegacyTransport>,
}

impl GameSession for FixtureSession {
    fn step(
        &mut self,
        _timestamp: MonotonicTimestamp,
        inputs: Vec<SessionInput>,
    ) -> SessionUpdate {
        let outbound = inputs
            .into_iter()
            .map(|input| OutboundEvent {
                target: OutboundTarget::Peers(vec![input.peer_id]),
                event: EncodedEvent {
                    payload: input.input.payload,
                },
            })
            .collect();
        SessionUpdate {
            outbound,
            scheduling: Vec::new(),
            closes: Vec::new(),
        }
    }

    fn join(
        &mut self,
        admission: AdmissionMetadata,
    ) -> Result<SessionUpdate, AdmissionRefusal> {
        self.peers.insert(admission.peer_id);
        self.transport.record_metric(MetricObservation {
            name: "fixture_join".to_string(),
            value: 1.0,
        });
        Ok(SessionUpdate {
            outbound: vec![OutboundEvent {
                target: OutboundTarget::Peers(vec![admission.peer_id]),
                event: EncodedEvent {
                    payload: b"\0fixture-ready".to_vec(),
                },
            }],
            scheduling: vec![SchedulingRequest::After(MonotonicDuration::from_micros(
                1_000,
            ))],
            closes: Vec::new(),
        })
    }

    fn leave(&mut self, peer_id: PeerId, _reason: LeaveReason) -> SessionUpdate {
        self.peers.remove(&peer_id);
        SessionUpdate {
            outbound: Vec::new(),
            scheduling: Vec::new(),
            closes: Vec::new(),
        }
    }

    fn lobby_status(&self) -> LobbyStatus {
        let peer_count = self.peers.len();
        LobbyStatus {
            code: "waiting".to_string(),
            detail: Some(format!("{peer_count} fixture peers")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_has_no_legacy_selector() {
        let mut builder = RegistryBuilder::new();
        register(&mut builder).unwrap();
        let registry = builder.build_from_source(MANIFEST).unwrap();
        assert!(registry.legacy_selectors().is_empty());
    }

    #[test]
    fn fixture_codec_preserves_text_and_binary_frame_kinds() {
        let codec = FixtureCodec;
        for frame in [
            InnerFrame::Text("hello".to_string()),
            InnerFrame::Binary(vec![1, 2, 3]),
        ] {
            let decoded = codec.decode(&frame).unwrap();
            let encoded = codec
                .encode(&EncodedEvent {
                    payload: decoded.payload,
                })
                .unwrap();
            assert_eq!(encoded, frame);
        }
    }
}
