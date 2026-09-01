//! Evergreen hosting adapter for the frozen Pong protocol 1 contract.

use ember_legacy::{
    AdmissionMetadata, AdmissionRefusal, CloseReason, CloseRequest, DecodedInput, EncodedEvent,
    FactoryError, GameFactory, GameKey, GameSession, InnerCodec, InnerCodecError, InnerFrame,
    LeaveReason, LegacyCapabilities, LobbySeed, LobbyStatus, MonotonicDuration, MonotonicTimestamp,
    OutboundEvent, OutboundTarget, PeerId, SchedulingRequest, SessionCreationData, SessionInput,
    SessionUpdate,
};

use crate::proto::{
    C2S, MAX_HANDLE_LEN, PROTO_VERSION, S2C, STATE_EVERY_TICKS, sanitize_axis, sanitize_text,
};
use crate::sim::{Phase, Sim};

/// Permanent hosted-game identifier for Pong.
pub const GAME_ID: &str = "pong";
/// Frozen gameplay and inner-wire version hosted by this crate.
pub const GAME_VERSION: u32 = 1;
/// Immutable behavior-gate suite named by `games/hosted.toml`.
pub const FIXTURE_SUITE_ID: &str = "pong-v1-hosted-contract";

const _: () = assert!(PROTO_VERSION == 1);

const FIXED_STEP_MICROS: u64 = 16_667;
const STALL_GRACE_STEPS: u64 = 10;
const MAX_FRAME_BYTES: usize = 16 * 1_024;

/// Returns the exact registry key implemented by this crate.
#[must_use]
pub fn game_key() -> GameKey {
    GameKey {
        game_id: GAME_ID.to_string(),
        game_version: GAME_VERSION,
    }
}

/// Exact JSON text-frame codec for Pong protocol 1.
#[derive(Clone, Copy, Debug, Default)]
pub struct PongCodec;

impl PongCodec {
    /// Constructs the stateless protocol-1 codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl InnerCodec for PongCodec {
    fn decode(&self, frame: &InnerFrame) -> Result<DecodedInput, InnerCodecError> {
        let InnerFrame::Text(text) = frame else {
            return Err(InnerCodecError::WrongFrameKind);
        };
        if text.len() > MAX_FRAME_BYTES {
            return Err(InnerCodecError::InvalidFrame(format!(
                "pong protocol 1 frame is {} bytes; maximum is {MAX_FRAME_BYTES}",
                text.len()
            )));
        }
        serde_json::from_str::<C2S>(text)
            .map_err(|error| InnerCodecError::DecodeFailed(error.to_string()))?;
        Ok(DecodedInput {
            payload: text.as_bytes().to_vec(),
        })
    }

    fn encode(&self, event: &EncodedEvent) -> Result<InnerFrame, InnerCodecError> {
        if event.payload.len() > MAX_FRAME_BYTES {
            return Err(InnerCodecError::InvalidFrame(format!(
                "pong protocol 1 frame is {} bytes; maximum is {MAX_FRAME_BYTES}",
                event.payload.len()
            )));
        }
        serde_json::from_slice::<S2C>(&event.payload)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))?;
        let text = std::str::from_utf8(&event.payload)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))?;
        Ok(InnerFrame::Text(text.to_owned()))
    }
}

/// Factory for authoritative Pong protocol-1 sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct PongFactory;

impl PongFactory {
    /// Constructs the stateless protocol-1 session factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl GameFactory for PongFactory {
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
        if !creation.configured_rules.is_empty() {
            return Err(FactoryError::InvalidConfiguration(
                "Pong protocol 1 has no configurable rules".to_string(),
            ));
        }
        Ok(Box::new(PongSession::new(
            creation.lobby_name.clone(),
            creation.lobby_seed,
            creation.created_at,
        )))
    }
}

#[derive(Clone, Debug)]
struct Member {
    peer_id: PeerId,
    handle: String,
}

/// One authoritative two-player Pong lobby.
pub struct PongSession {
    lobby_name: String,
    members: Vec<Member>,
    sim: Option<Sim>,
    axes: [f32; 2],
    tick: u64,
    next_tick_at: MonotonicTimestamp,
    schedule_started: bool,
}

impl PongSession {
    /// Constructs a lobby from immutable host data without consuming capabilities.
    #[must_use]
    pub fn new(lobby_name: String, _lobby_seed: LobbySeed, created_at: MonotonicTimestamp) -> Self {
        Self {
            lobby_name,
            members: Vec::new(),
            sim: None,
            axes: [0.0, 0.0],
            tick: 0,
            next_tick_at: created_at
                .saturating_add(MonotonicDuration::from_micros(FIXED_STEP_MICROS)),
            schedule_started: false,
        }
    }

    /// Returns the current simulation when an opponent has joined.
    #[must_use]
    pub fn sim(&self) -> Option<&Sim> {
        self.sim.as_ref()
    }

    /// Returns the adapter's era server tick.
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    fn peer_ids(&self) -> Vec<PeerId> {
        self.members.iter().map(|member| member.peer_id).collect()
    }

    fn accept_input(&mut self, input: SessionInput, update: &mut SessionUpdate) {
        let Ok(message) = serde_json::from_slice::<C2S>(&input.input.payload) else {
            update.closes.push(CloseRequest {
                peer_id: input.peer_id,
                reason: CloseReason::ProtocolViolation,
            });
            return;
        };
        match message {
            C2S::Input { axis } => {
                if let Some(role) = self
                    .members
                    .iter()
                    .position(|member| member.peer_id == input.peer_id)
                {
                    self.axes[role] = sanitize_axis(axis);
                }
            }
            C2S::Ping { nonce } => push_message(
                update,
                OutboundTarget::Peers(vec![input.peer_id]),
                &S2C::Pong { nonce },
            ),
            C2S::LeaveLobby => update.closes.push(CloseRequest {
                peer_id: input.peer_id,
                reason: CloseReason::Requested,
            }),
            C2S::CreateLobby { .. } | C2S::JoinLobby { .. } => push_message(
                update,
                OutboundTarget::Peers(vec![input.peer_id]),
                &S2C::Error {
                    message: "already in a lobby".to_string(),
                },
            ),
            C2S::Hello { .. } => update.closes.push(CloseRequest {
                peer_id: input.peer_id,
                reason: CloseReason::ProtocolViolation,
            }),
            C2S::ListLobbies => push_message(
                update,
                OutboundTarget::Peers(vec![input.peer_id]),
                &S2C::LobbyList {
                    lobbies: Vec::new(),
                },
            ),
        }
    }

    fn run_tick(&mut self, update: &mut SessionUpdate) {
        self.tick = self.tick.saturating_add(1);
        let peers = self.peer_ids();
        let Some(sim) = self.sim.as_mut() else {
            return;
        };
        sim.step(self.axes[0], self.axes[1]);
        if let Some((scorer, won)) = sim.event {
            let scorer = if scorer == 0 { 0 } else { 1 };
            push_message(
                update,
                OutboundTarget::Peers(peers.clone()),
                &S2C::MatchEvent {
                    scorer,
                    won,
                    scores: sim.score,
                },
            );
        }
        if self.tick.is_multiple_of(STATE_EVERY_TICKS) {
            push_message(
                update,
                OutboundTarget::Peers(peers),
                &S2C::State {
                    tick: self.tick,
                    ball: sim.ball_pos,
                    paddles: [sim.p1_x, sim.p2_x],
                    scores: sim.score,
                    serving: matches!(sim.phase, Phase::Serving { .. }),
                },
            );
        }
    }

    fn advance(&mut self, timestamp: MonotonicTimestamp, update: &mut SessionUpdate) -> bool {
        let now = timestamp.as_micros();
        let due = self.next_tick_at.as_micros();
        if now < due {
            return false;
        }

        let following = due.saturating_add(FIXED_STEP_MICROS);
        let stall_limit =
            following.saturating_add(FIXED_STEP_MICROS.saturating_mul(STALL_GRACE_STEPS));
        if now > stall_limit {
            self.run_tick(update);
            self.next_tick_at =
                MonotonicTimestamp::from_micros(now.saturating_add(FIXED_STEP_MICROS));
            return true;
        }

        let mut next = due;
        loop {
            self.run_tick(update);
            let advanced = next.saturating_add(FIXED_STEP_MICROS);
            self.next_tick_at = MonotonicTimestamp::from_micros(advanced);
            if advanced == next || now < advanced {
                break;
            }
            next = advanced;
        }
        true
    }
}

impl GameSession for PongSession {
    fn step(&mut self, timestamp: MonotonicTimestamp, inputs: Vec<SessionInput>) -> SessionUpdate {
        let mut update = SessionUpdate::default();
        for input in inputs {
            self.accept_input(input, &mut update);
        }
        if self.advance(timestamp, &mut update) && !self.members.is_empty() {
            update
                .scheduling
                .push(SchedulingRequest::At(self.next_tick_at));
        }
        update
    }

    fn join(&mut self, admission: AdmissionMetadata) -> Result<SessionUpdate, AdmissionRefusal> {
        if self
            .members
            .iter()
            .any(|member| member.peer_id == admission.peer_id)
        {
            return Err(AdmissionRefusal {
                code: "already_joined".to_string(),
                message: "already in this Pong lobby".to_string(),
            });
        }
        if self.members.len() >= 2 {
            return Err(AdmissionRefusal {
                code: "lobby_full".to_string(),
                message: "lobby is full".to_string(),
            });
        }

        let handle = sanitize_text(&admission.handle, MAX_HANDLE_LEN);
        let handle = if handle.is_empty() {
            "player".to_string()
        } else {
            handle
        };
        let peer_id = admission.peer_id;
        let mut update = SessionUpdate::default();
        if self.members.is_empty() {
            self.members.push(Member { peer_id, handle });
            push_message(
                &mut update,
                OutboundTarget::Peers(vec![peer_id]),
                &S2C::LobbyCreated {
                    name: self.lobby_name.clone(),
                },
            );
        } else {
            let host = self.members[0].clone();
            self.members.push(Member {
                peer_id,
                handle: handle.clone(),
            });
            self.axes = [0.0, 0.0];
            self.sim = Some(Sim::new());
            push_message(
                &mut update,
                OutboundTarget::Peers(vec![host.peer_id]),
                &S2C::MatchStart {
                    role: 0,
                    opponent: handle,
                },
            );
            push_message(
                &mut update,
                OutboundTarget::Peers(vec![peer_id]),
                &S2C::MatchStart {
                    role: 1,
                    opponent: host.handle,
                },
            );
        }
        if !self.schedule_started {
            self.schedule_started = true;
            update
                .scheduling
                .push(SchedulingRequest::At(self.next_tick_at));
        }
        Ok(update)
    }

    fn leave(&mut self, peer_id: PeerId, _reason: LeaveReason) -> SessionUpdate {
        let Some(index) = self
            .members
            .iter()
            .position(|member| member.peer_id == peer_id)
        else {
            return SessionUpdate::default();
        };
        let was_running = self.sim.is_some();
        self.members.remove(index);
        self.sim = None;
        self.axes = [0.0, 0.0];
        let mut update = SessionUpdate::default();
        if was_running && let Some(remaining) = self.members.first() {
            push_message(
                &mut update,
                OutboundTarget::Peers(vec![remaining.peer_id]),
                &S2C::OpponentLeft,
            );
        }
        update
    }

    fn lobby_status(&self) -> LobbyStatus {
        LobbyStatus {
            code: if self.sim.is_some() {
                "playing".to_string()
            } else {
                "waiting".to_string()
            },
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
