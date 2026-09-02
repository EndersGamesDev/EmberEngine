//! Evergreen hosting adapter for the frozen Fire protocol 1 contract.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use ember_legacy::{
    AdmissionMetadata, AdmissionRefusal, DecodedInput, EncodedEvent, FactoryError, GameFactory,
    GameKey, GameSession, InnerCodec, InnerCodecError, InnerFrame, LeaveReason, LegacyCapabilities,
    LobbySeed, LobbyStatus, MonotonicTimestamp, OutboundEvent, OutboundTarget, PeerId,
    SessionCreationData, SessionInput, SessionUpdate,
};
use serde::{Deserialize, Serialize};

use crate::ai;
use crate::car::{CarInput, DT};
use crate::castle;
use crate::proto::{self, C2S, CarState, MAX_PLAYERS, Phase, PlayerMeta, S2C, STATE_EVERY_TICKS};
use crate::sim::{FixedStep, Race, RaceState};

/// Permanent hosted-game identifier for Fire.
pub const GAME_ID: &str = "fire";
/// Frozen gameplay and inner-wire version hosted by this crate.
pub const GAME_VERSION: u32 = 1;
/// Immutable behavior-gate suite named by `games/hosted.toml`.
pub const FIXTURE_SUITE_ID: &str = "fire-v1-hosted-contract";
/// Lap count used by the deployed Fire server when no rule bytes are supplied.
pub const DEFAULT_LAPS: u32 = 3;

const RESULTS_SECS: f32 = 8.0;

/// Returns the exact registry key implemented by this crate.
#[must_use]
pub fn game_key() -> GameKey {
    GameKey {
        game_id: GAME_ID.to_string(),
        game_version: GAME_VERSION,
    }
}

/// Immutable version-owned rules accepted through `SessionCreationData`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FireRules {
    /// Number of completed laps required to finish.
    pub laps: u32,
}

impl Default for FireRules {
    fn default() -> Self {
        Self { laps: DEFAULT_LAPS }
    }
}

/// Exact protocol-1 JSON text-frame codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct FireCodec;

impl InnerCodec for FireCodec {
    fn decode(&self, frame: &InnerFrame) -> Result<DecodedInput, InnerCodecError> {
        let InnerFrame::Text(text) = frame else {
            return Err(InnerCodecError::WrongFrameKind);
        };
        if text.len() > proto::MAX_FRAME_BYTES {
            return Err(InnerCodecError::InvalidFrame(format!(
                "fire protocol 1 frame is {} bytes; maximum is {}",
                text.len(),
                proto::MAX_FRAME_BYTES
            )));
        }
        serde_json::from_str::<C2S>(text)
            .map_err(|error| InnerCodecError::DecodeFailed(error.to_string()))?;
        Ok(DecodedInput {
            payload: text.as_bytes().to_vec(),
        })
    }

    fn encode(&self, event: &EncodedEvent) -> Result<InnerFrame, InnerCodecError> {
        if event.payload.len() > proto::MAX_FRAME_BYTES {
            return Err(InnerCodecError::InvalidFrame(format!(
                "fire protocol 1 frame is {} bytes; maximum is {}",
                event.payload.len(),
                proto::MAX_FRAME_BYTES
            )));
        }
        serde_json::from_slice::<S2C>(&event.payload)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))?;
        let text = std::str::from_utf8(&event.payload)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))?;
        Ok(InnerFrame::Text(text.to_owned()))
    }
}

/// Registry factory for authoritative Fire protocol 1 lobby sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct FireFactory;

impl GameFactory for FireFactory {
    fn create(
        &self,
        _capabilities: &LegacyCapabilities,
        creation: &SessionCreationData,
    ) -> Result<Box<dyn GameSession>, FactoryError> {
        if creation.game_key != game_key() {
            return Err(FactoryError::InvalidConfiguration(format!(
                "expected {GAME_ID}/{GAME_VERSION}, got {}/{}",
                creation.game_key.game_id, creation.game_key.game_version
            )));
        }
        let rules = if creation.configured_rules.is_empty() {
            FireRules::default()
        } else {
            serde_json::from_slice(&creation.configured_rules)
                .map_err(|error| FactoryError::InvalidConfiguration(error.to_string()))?
        };
        Ok(Box::new(FireSession::new(
            rules,
            creation.lobby_name.clone(),
            creation.lobby_seed,
            creation.created_at,
        )))
    }
}

/// One authoritative Fire race with the deployed server's input and output mapping.
pub struct FireSession {
    race: Race,
    rules: FireRules,
    lobby_name: String,
    members: Vec<PeerId>,
    slots: BTreeMap<PeerId, u8>,
    handles: BTreeMap<PeerId, String>,
    ready: BTreeSet<PeerId>,
    inputs: BTreeMap<u8, (CarInput, u32)>,
    fixed_step: FixedStep,
    last_step: MonotonicTimestamp,
    results_left: f32,
    last_phase: Phase,
}

impl FireSession {
    /// Constructs a session from immutable host creation data.
    ///
    /// Fire protocol 1 performs no random draws, so the lobby seed is intentionally inert.
    #[must_use]
    pub fn new(
        rules: FireRules,
        lobby_name: String,
        _lobby_seed: LobbySeed,
        created_at: MonotonicTimestamp,
    ) -> Self {
        Self {
            race: Race::new(castle::track(), usize::from(MAX_PLAYERS), rules.laps),
            rules,
            lobby_name,
            members: Vec::new(),
            slots: BTreeMap::new(),
            handles: BTreeMap::new(),
            ready: BTreeSet::new(),
            inputs: BTreeMap::new(),
            fixed_step: FixedStep::default(),
            last_step: created_at,
            results_left: 0.0,
            last_phase: Phase::Waiting,
        }
    }

    /// Returns the frozen simulation state for behavior-gate checkpoints.
    #[must_use]
    pub const fn race(&self) -> &Race {
        &self.race
    }

    const fn phase(&self) -> Phase {
        match self.race.state {
            RaceState::Waiting => Phase::Waiting,
            RaceState::Countdown => Phase::Countdown,
            RaceState::Racing => Phase::Racing,
            RaceState::Finished => Phase::Finished,
        }
    }

    fn alloc_slot(&self) -> Option<u8> {
        (0..MAX_PLAYERS).find(|slot| !self.slots.values().any(|used| used == slot))
    }

    fn roster(&self) -> Vec<PlayerMeta> {
        self.members
            .iter()
            .filter_map(|peer_id| {
                let slot = *self.slots.get(peer_id)?;
                Some(PlayerMeta {
                    id: slot,
                    handle: self
                        .handles
                        .get(peer_id)
                        .cloned()
                        .unwrap_or_else(|| "?".to_string()),
                    slot,
                })
            })
            .collect()
    }

    fn accept_input(&mut self, session_input: &SessionInput, update: &mut SessionUpdate) {
        let Ok(message) = serde_json::from_slice::<C2S>(&session_input.input.payload) else {
            return;
        };
        match message {
            C2S::Ready { ready } => {
                if self.slots.contains_key(&session_input.peer_id) {
                    if ready {
                        self.ready.insert(session_input.peer_id);
                    } else {
                        self.ready.remove(&session_input.peer_id);
                    }
                }
            }
            C2S::Input {
                seq,
                throttle,
                steer,
                handbrake,
                boost,
            } => {
                let incoming = CarInput {
                    throttle,
                    steer,
                    handbrake,
                    boost,
                }
                .sanitized();
                self.record_input(session_input.peer_id, seq, incoming);
            }
            C2S::Ping { nonce } => push_message(
                update,
                OutboundTarget::Peers(vec![session_input.peer_id]),
                &S2C::Pong { nonce },
            ),
            C2S::LeaveLobby => {
                let mut leave = self.remove_peer(session_input.peer_id);
                update.outbound.append(&mut leave.outbound);
                update.scheduling.append(&mut leave.scheduling);
                update.closes.append(&mut leave.closes);
            }
            C2S::Hello { .. }
            | C2S::ListLobbies
            | C2S::CreateLobby { .. }
            | C2S::JoinLobby { .. } => {}
        }
    }

    fn record_input(&mut self, peer_id: PeerId, seq: u32, incoming: CarInput) {
        let Some(&slot) = self.slots.get(&peer_id) else {
            return;
        };
        match self.inputs.get_mut(&slot) {
            Some((held, last_seq)) => {
                if seq <= *last_seq {
                    return;
                }
                let pending_boost = held.boost;
                *held = incoming;
                held.boost |= pending_boost;
                *last_seq = seq;
            }
            None => {
                self.inputs.insert(slot, (incoming, seq));
            }
        }
    }

    const fn elapsed_seconds(&mut self, timestamp: MonotonicTimestamp) -> f32 {
        let previous = self.last_step.as_micros();
        let current = timestamp.as_micros();
        if current < previous {
            return 0.0;
        }
        self.last_step = timestamp;
        Duration::from_micros(current - previous).as_secs_f32()
    }

    fn tick(&mut self, update: &mut SessionUpdate) {
        if self.race.state == RaceState::Waiting
            && !self.ready.is_empty()
            && self.ready.len() == self.members.len()
        {
            self.race.start_countdown();
        }

        let inputs = self.lobby_inputs();
        self.race.step(&inputs, DT);

        for (input, _) in self.inputs.values_mut() {
            input.boost = false;
        }

        let phase = self.phase();
        if phase != self.last_phase {
            self.last_phase = phase;
            self.broadcast(
                update,
                &S2C::Phase {
                    phase,
                    countdown: self.race.countdown_left(),
                },
            );
            if phase == Phase::Finished {
                let order = self
                    .race
                    .standings()
                    .iter()
                    .filter_map(|index| u8::try_from(*index).ok())
                    .collect();
                self.broadcast(update, &S2C::Results { order });
                self.results_left = RESULTS_SECS;
            }
        } else if phase == Phase::Countdown && self.race.tick.is_multiple_of(STATE_EVERY_TICKS) {
            self.broadcast(
                update,
                &S2C::Phase {
                    phase,
                    countdown: self.race.countdown_left(),
                },
            );
        }

        if self.race.state != RaceState::Waiting && self.race.tick.is_multiple_of(STATE_EVERY_TICKS)
        {
            let cars = self.race_state();
            self.broadcast(
                update,
                &S2C::State {
                    tick: self.race.tick,
                    cars,
                },
            );
        }

        if self.race.state == RaceState::Finished {
            self.results_left -= DT;
            if self.results_left <= 0.0 {
                self.race = Race::new(castle::track(), usize::from(MAX_PLAYERS), self.rules.laps);
                self.ready.clear();
                self.inputs.clear();
                self.last_phase = Phase::Waiting;
                self.broadcast(
                    update,
                    &S2C::Phase {
                        phase: Phase::Waiting,
                        countdown: 0.0,
                    },
                );
            }
        }
    }

    fn lobby_inputs(&self) -> Vec<CarInput> {
        (0..MAX_PLAYERS)
            .zip(&self.race.racers)
            .map(|(slot, racer)| match self.inputs.get(&slot) {
                Some((input, _)) => *input,
                None if self.slots.values().any(|candidate| *candidate == slot) => {
                    CarInput::default()
                }
                None => ai::chase(&self.race.track, &racer.car, ai::DEFAULT_SKILL),
            })
            .collect()
    }

    fn race_state(&self) -> Vec<CarState> {
        (0..MAX_PLAYERS)
            .zip(&self.race.racers)
            .map(|(id, racer)| CarState {
                id,
                x: racer.car.pos.x,
                z: racer.car.pos.y,
                yaw: racer.car.yaw,
                vx: racer.car.vel.x,
                vz: racer.car.vel.y,
                lap: racer.lap.lap,
                progress: racer.lap.progress,
                boost: racer.car.boost_charges,
                boosting: racer.car.boosting(),
                drift: racer.car.drift,
                ack: self.inputs.get(&id).map_or(0, |(_, sequence)| *sequence),
            })
            .collect()
    }

    fn broadcast(&self, update: &mut SessionUpdate, message: &S2C) {
        if !self.members.is_empty() {
            push_message(update, OutboundTarget::Peers(self.members.clone()), message);
        }
    }

    fn remove_peer(&mut self, peer_id: PeerId) -> SessionUpdate {
        self.members.retain(|member| *member != peer_id);
        self.ready.remove(&peer_id);
        self.handles.remove(&peer_id);
        let mut update = SessionUpdate::default();
        if let Some(slot) = self.slots.remove(&peer_id) {
            self.inputs.remove(&slot);
            self.broadcast(&mut update, &S2C::PlayerLeft { id: slot });
        }
        update
    }
}

impl GameSession for FireSession {
    fn step(&mut self, timestamp: MonotonicTimestamp, inputs: Vec<SessionInput>) -> SessionUpdate {
        let mut update = SessionUpdate::default();
        for input in inputs {
            self.accept_input(&input, &mut update);
        }
        let elapsed = self.elapsed_seconds(timestamp);
        let ticks = self.fixed_step.ticks(elapsed);
        for _ in 0..ticks {
            self.tick(&mut update);
        }
        update
    }

    fn join(&mut self, admission: AdmissionMetadata) -> Result<SessionUpdate, AdmissionRefusal> {
        if self.slots.contains_key(&admission.peer_id) {
            return Err(AdmissionRefusal {
                code: "already_joined".to_string(),
                message: "peer is already in this Fire lobby".to_string(),
            });
        }
        let Some(slot) = self.alloc_slot() else {
            return Err(AdmissionRefusal {
                code: "lobby_full".to_string(),
                message: "lobby is full".to_string(),
            });
        };

        let existing = self.members.clone();
        let peer_id = admission.peer_id;
        let handle = proto::sanitize_handle(&admission.handle);
        self.members.push(peer_id);
        self.slots.insert(peer_id, slot);
        self.handles.insert(peer_id, handle.clone());

        let mut update = SessionUpdate::default();
        push_message(
            &mut update,
            OutboundTarget::Peers(vec![peer_id]),
            &S2C::Joined {
                lobby: self.lobby_name.clone(),
                id: slot,
                slot,
                laps: self.rules.laps,
                roster: self.roster(),
            },
        );
        push_message(
            &mut update,
            OutboundTarget::Peers(vec![peer_id]),
            &S2C::Phase {
                phase: self.phase(),
                countdown: self.race.countdown_left(),
            },
        );
        if !existing.is_empty() {
            push_message(
                &mut update,
                OutboundTarget::Peers(existing),
                &S2C::PlayerJoined {
                    meta: PlayerMeta {
                        id: slot,
                        handle,
                        slot,
                    },
                },
            );
        }
        Ok(update)
    }

    fn leave(&mut self, peer_id: PeerId, _reason: LeaveReason) -> SessionUpdate {
        self.remove_peer(peer_id)
    }

    fn lobby_status(&self) -> LobbyStatus {
        let code = if self.race.state == RaceState::Waiting {
            "waiting"
        } else {
            "racing"
        };
        LobbyStatus {
            code: code.to_string(),
            detail: None,
        }
    }
}

fn push_message(update: &mut SessionUpdate, target: OutboundTarget, message: &S2C) {
    if let Ok(payload) = serde_json::to_vec(message) {
        update.outbound.push(OutboundEvent {
            target,
            event: EncodedEvent { payload },
        });
    }
}
