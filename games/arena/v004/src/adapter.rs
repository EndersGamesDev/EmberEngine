//! Evergreen host adapter around the frozen Arena v4 contract.

use std::collections::HashMap;

use ember_legacy::{
    AdmissionMetadata, AdmissionRefusal, CloseReason, CloseRequest, DecodedInput, EncodedEvent,
    FactoryError, GameFactory, GameKey, GameSession, InnerCodec, InnerCodecError, InnerFrame,
    LeaveReason, LegacyCapabilities, LobbyStatus, MonotonicDuration, MonotonicTimestamp,
    OutboundEvent, OutboundTarget, PeerId, SchedulingRequest, SessionCreationData, SessionInput,
    SessionUpdate,
};

use crate::proto::{
    BState, C2S, MAX_HANDLE_LEN, PROTO_VERSION, PState, PlayerMeta, S2C, STATE_EVERY_TICKS,
    color_for, sanitize_text,
};
use crate::shooter::{ARENA_HALF, MAX_PLAYERS, PlayerIn, Sim};

const GAME_ID: &str = "arena";
const FIXED_STEP_MICROS: u64 = 16_667;
const STALL_GRACE_STEPS: u64 = 10;

/// Returns the exact registry key implemented by this crate.
#[must_use]
pub fn game_key() -> GameKey {
    GameKey {
        game_id: GAME_ID.to_string(),
        game_version: u32::from(PROTO_VERSION),
    }
}

/// Exact JSON text-frame codec for Arena protocol 4.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaCodec;

impl ArenaCodec {
    /// Constructs the stateless v4 codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl InnerCodec for ArenaCodec {
    fn decode(&self, frame: &InnerFrame) -> Result<DecodedInput, InnerCodecError> {
        let InnerFrame::Text(text) = frame else {
            return Err(InnerCodecError::WrongFrameKind);
        };
        let message: C2S = serde_json::from_str(text)
            .map_err(|error| InnerCodecError::DecodeFailed(error.to_string()))?;
        let payload = serde_json::to_vec(&message)
            .map_err(|error| InnerCodecError::DecodeFailed(error.to_string()))?;
        Ok(DecodedInput { payload })
    }

    fn encode(&self, event: &EncodedEvent) -> Result<InnerFrame, InnerCodecError> {
        let message: S2C = serde_json::from_slice(&event.payload)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))?;
        serde_json::to_string(&message)
            .map(InnerFrame::Text)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))
    }
}

/// Factory for authoritative Arena protocol-4 sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaFactory;

impl ArenaFactory {
    /// Constructs the stateless v4 session factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl GameFactory for ArenaFactory {
    fn create(
        &self,
        _capabilities: &LegacyCapabilities,
        creation: &SessionCreationData,
    ) -> Result<Box<dyn GameSession>, FactoryError> {
        if creation.game_key.game_id != GAME_ID
            || creation.game_key.game_version != u32::from(PROTO_VERSION)
        {
            return Err(FactoryError::InvalidConfiguration(
                "Arena v4 factory received a different game key".to_string(),
            ));
        }
        if !creation.configured_rules.is_empty() {
            return Err(FactoryError::InvalidConfiguration(
                "Arena v4 has no configurable rules".to_string(),
            ));
        }
        let bytes = creation.lobby_seed.0;
        let seed = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        Ok(Box::new(ArenaSession::new(seed, creation.created_at)))
    }
}

#[derive(Clone, Debug)]
struct Member {
    peer_id: PeerId,
    player_id: u8,
    handle: String,
}

struct ArenaSession {
    seed: u64,
    sim: Sim,
    tick: u64,
    members: Vec<Member>,
    inputs: HashMap<u8, PlayerIn>,
    pending_inputs: Vec<SessionInput>,
    next_tick_at: MonotonicTimestamp,
    schedule_started: bool,
}

impl ArenaSession {
    fn new(seed: u64, created_at: MonotonicTimestamp) -> Self {
        Self {
            seed,
            sim: Sim::new(seed),
            tick: 0,
            members: Vec::new(),
            inputs: HashMap::new(),
            pending_inputs: Vec::new(),
            next_tick_at: created_at
                .saturating_add(MonotonicDuration::from_micros(FIXED_STEP_MICROS)),
            schedule_started: false,
        }
    }

    fn alloc_player_id(&self) -> u8 {
        (0..u8::MAX)
            .find(|candidate| {
                !self
                    .members
                    .iter()
                    .any(|member| member.player_id == *candidate)
            })
            .unwrap_or(0)
    }

    fn roster(&self) -> Vec<PlayerMeta> {
        self.members
            .iter()
            .map(|member| PlayerMeta {
                id: member.player_id,
                handle: member.handle.clone(),
                color: color_for(member.player_id),
            })
            .collect()
    }

    fn peer_ids(&self) -> Vec<PeerId> {
        self.members.iter().map(|member| member.peer_id).collect()
    }

    fn push_message(update: &mut SessionUpdate, target: OutboundTarget, message: &S2C) {
        if let Ok(payload) = serde_json::to_vec(message) {
            update.outbound.push(OutboundEvent {
                target,
                event: EncodedEvent { payload },
            });
        }
    }

    fn accept_input(&mut self, session_input: &SessionInput, update: &mut SessionUpdate) {
        let Some(player_id) = self
            .members
            .iter()
            .find(|member| member.peer_id == session_input.peer_id)
            .map(|member| member.player_id)
        else {
            return;
        };
        let Ok(message) = serde_json::from_slice::<C2S>(&session_input.input.payload) else {
            update.closes.push(CloseRequest {
                peer_id: session_input.peer_id,
                reason: CloseReason::ProtocolViolation,
            });
            return;
        };
        match message {
            C2S::Input {
                mx,
                my,
                ax,
                az,
                fire,
                sprint,
                crouch,
            } => {
                self.inputs.insert(
                    player_id,
                    PlayerIn {
                        mv: [mx, my],
                        aim: [ax, az],
                        fire,
                        sprint,
                        crouch,
                    },
                );
            }
            C2S::Ping { nonce } => Self::push_message(
                update,
                OutboundTarget::Peers(vec![session_input.peer_id]),
                &S2C::Pong { nonce },
            ),
            C2S::LeaveLobby => {
                update.closes.push(CloseRequest {
                    peer_id: session_input.peer_id,
                    reason: CloseReason::Requested,
                });
            }
            C2S::CreateLobby { .. } | C2S::JoinLobby { .. } => Self::push_message(
                update,
                OutboundTarget::Peers(vec![session_input.peer_id]),
                &S2C::Error {
                    message: "already in a game".to_string(),
                },
            ),
            C2S::Hello { .. } => update.closes.push(CloseRequest {
                peer_id: session_input.peer_id,
                reason: CloseReason::ProtocolViolation,
            }),
            C2S::ListLobbies => Self::push_message(
                update,
                OutboundTarget::Peers(vec![session_input.peer_id]),
                &S2C::LobbyList {
                    lobbies: Vec::new(),
                },
            ),
        }
    }

    fn accept_ready_inputs(&mut self, timestamp: MonotonicTimestamp, update: &mut SessionUpdate) {
        let mut future = Vec::new();
        let pending = std::mem::take(&mut self.pending_inputs);
        for input in pending {
            if input.received_at <= timestamp {
                self.accept_input(&input, update);
            } else {
                future.push(input);
            }
        }
        self.pending_inputs = future;
    }

    fn run_tick(&mut self, update: &mut SessionUpdate) {
        self.tick += 1;
        let inputs = &self.inputs;
        self.sim
            .step(&|player_id| inputs.get(&player_id).copied().unwrap_or_default());

        let peers = self.peer_ids();
        for (killer, victim) in self.sim.events.clone() {
            Self::push_message(
                update,
                OutboundTarget::Peers(peers.clone()),
                &S2C::Kill { killer, victim },
            );
        }
        if self.tick.is_multiple_of(STATE_EVERY_TICKS) {
            let state = S2C::State {
                tick: self.tick,
                players: self
                    .sim
                    .players
                    .iter()
                    .map(|player| PState {
                        id: player.id,
                        x: player.pos[0],
                        z: player.pos[1],
                        ax: player.aim[0],
                        az: player.aim[1],
                        hp: player.hp,
                        score: player.score,
                        alive: player.alive,
                        crouch: player.crouch,
                    })
                    .collect(),
                bullets: self
                    .sim
                    .bullets
                    .iter()
                    .map(|bullet| BState {
                        x: bullet.pos[0],
                        z: bullet.pos[1],
                        vx: bullet.vel[0],
                        vz: bullet.vel[1],
                    })
                    .collect(),
            };
            Self::push_message(update, OutboundTarget::Peers(peers), &state);
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

impl GameSession for ArenaSession {
    fn step(&mut self, timestamp: MonotonicTimestamp, inputs: Vec<SessionInput>) -> SessionUpdate {
        self.pending_inputs.extend(inputs);
        let mut update = SessionUpdate::default();
        self.accept_ready_inputs(timestamp, &mut update);
        if self.advance(timestamp, &mut update) && !self.members.is_empty() {
            update
                .scheduling
                .push(SchedulingRequest::At(self.next_tick_at));
        }
        update
    }

    fn join(&mut self, admission: AdmissionMetadata) -> Result<SessionUpdate, AdmissionRefusal> {
        if self.members.len() >= MAX_PLAYERS {
            return Err(AdmissionRefusal {
                code: "game_full".to_string(),
                message: "game is full".to_string(),
            });
        }
        if self
            .members
            .iter()
            .any(|member| member.peer_id == admission.peer_id)
        {
            return Err(AdmissionRefusal {
                code: "already_joined".to_string(),
                message: "already in a game".to_string(),
            });
        }

        let player_id = self.alloc_player_id();
        let handle = {
            let sanitized = sanitize_text(&admission.handle, MAX_HANDLE_LEN);
            if sanitized.is_empty() {
                "player".to_string()
            } else {
                sanitized
            }
        };
        let others = self.peer_ids();
        self.members.push(Member {
            peer_id: admission.peer_id,
            player_id,
            handle: handle.clone(),
        });
        self.sim.add_player(player_id);

        let mut update = SessionUpdate::default();
        Self::push_message(
            &mut update,
            OutboundTarget::Peers(vec![admission.peer_id]),
            &S2C::GameJoined {
                id: player_id,
                seed: self.seed,
                arena_half: ARENA_HALF,
                players: self.roster(),
            },
        );
        if !others.is_empty() {
            Self::push_message(
                &mut update,
                OutboundTarget::Peers(others),
                &S2C::PlayerJoined {
                    meta: PlayerMeta {
                        id: player_id,
                        handle,
                        color: color_for(player_id),
                    },
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
        let member = self.members.remove(index);
        self.inputs.remove(&member.player_id);
        self.sim.remove_player(member.player_id);

        let mut update = SessionUpdate::default();
        if !self.members.is_empty() {
            Self::push_message(
                &mut update,
                OutboundTarget::Peers(self.peer_ids()),
                &S2C::PlayerLeft {
                    id: member.player_id,
                },
            );
        }
        update
    }

    fn lobby_status(&self) -> LobbyStatus {
        LobbyStatus {
            code: "running".to_string(),
            detail: None,
        }
    }
}
