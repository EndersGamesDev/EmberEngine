//! Evergreen host adapters around the frozen Arena v9 contract.

use std::collections::HashMap;

use ember_legacy::{
    AdmissionMetadata, AdmissionRefusal, CloseReason, CloseRequest, DecodedInput, EncodedEvent,
    FactoryError, GameFactory, GameKey, GameSession, InnerCodec, InnerCodecError, InnerFrame,
    LeaveReason, LegacyCapabilities, LegacyConnectionState, LegacyIngress, LegacyIngressAction,
    LegacyIngressError, LegacyIngressFactory, LegacyIngressRefusal, LegacyLobbyProjection,
    LobbyStatus, MonotonicDuration, MonotonicTimestamp, OutboundEvent, OutboundTarget, PeerId,
    SchedulingRequest, SessionCreationData, SessionInput, SessionInputWithTransport, SessionUpdate,
};

use crate::proto::{
    BState, C2S, LobbyInfo, MAX_HANDLE_LEN, MAX_LOBBY_LEN, MAX_PASSWORD_LEN, PROTO_VERSION, PState,
    PlayerMeta, S2C, STATE_EVERY_TICKS, color_for, sanitize_text,
};
use crate::shooter::{ARENA_HALF, MAX_PLAYERS, PlayerIn, Sim};

const GAME_ID: &str = "arena";
const FIXED_STEP_MICROS: u64 = 16_667;
const STALL_GRACE_STEPS: u64 = 10;
const INITIAL_RTT_TICKS: u64 = 18;

/// Returns the exact registry key implemented by this crate.
#[must_use]
pub fn game_key() -> GameKey {
    GameKey {
        game_id: GAME_ID.to_string(),
        game_version: u32::from(PROTO_VERSION),
    }
}

/// Exact JSON text-frame codec for Arena protocol 9.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaCodec;

impl ArenaCodec {
    /// Constructs the stateless v9 codec.
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

/// Factory for authoritative Arena protocol-9 sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaFactory;

impl ArenaFactory {
    /// Constructs the stateless v9 session factory.
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
                "Arena v9 factory received a different game key".to_string(),
            ));
        }
        if !creation.configured_rules.is_empty() {
            return Err(FactoryError::InvalidConfiguration(
                "Arena v9 has no configurable rules".to_string(),
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

#[derive(Clone, Copy, Debug)]
struct InputRecord {
    input: PlayerIn,
    sequence: u32,
    view_tick: u64,
}

struct ArenaSession {
    seed: u64,
    sim: Sim,
    members: Vec<Member>,
    inputs: HashMap<u8, InputRecord>,
    pending_inputs: Vec<SessionInputWithTransport>,
    next_tick_at: MonotonicTimestamp,
    schedule_started: bool,
}

impl ArenaSession {
    fn new(seed: u64, created_at: MonotonicTimestamp) -> Self {
        Self {
            seed,
            sim: Sim::new(seed),
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

    fn accept_input(
        &mut self,
        session_input: SessionInputWithTransport,
        update: &mut SessionUpdate,
    ) {
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
                seq,
                view_tick,
                mx,
                my,
                ax,
                az,
                pitch,
                fire,
                sprint,
                crouch,
                reload,
                jump,
                shield,
            } => {
                let transport_rtt_ticks = session_input.transport_rtt.map_or(
                    INITIAL_RTT_TICKS,
                    |duration| duration.as_micros().div_ceil(FIXED_STEP_MICROS),
                );
                let allowed_delay = transport_rtt_ticks / 2 + 6;
                let floor = self.sim.tick.saturating_sub(allowed_delay);
                self.inputs.insert(
                    player_id,
                    InputRecord {
                        input: PlayerIn {
                            mv: [mx, my],
                            aim: [ax, az],
                            pitch,
                            fire,
                            sprint,
                            crouch,
                            reload,
                            jump,
                            shield,
                            delay_ticks: 0,
                        },
                        sequence: seq,
                        view_tick: view_tick.clamp(floor, self.sim.tick),
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
                self.accept_input(input, update);
            } else {
                future.push(input);
            }
        }
        self.pending_inputs = future;
    }

    fn run_tick(&mut self, update: &mut SessionUpdate) {
        let apply_tick = self.sim.tick + 1;
        let inputs = &self.inputs;
        self.sim.step(&|player_id| {
            inputs.get(&player_id).map_or_else(PlayerIn::default, |record| {
                let mut input = record.input;
                input.delay_ticks = bounded_tick_age(apply_tick, record.view_tick);
                input
            })
        });
        let peers = self.peer_ids();
        for (killer, victim) in self.sim.events.clone() {
            Self::push_message(
                update,
                OutboundTarget::Peers(peers.clone()),
                &S2C::Kill { killer, victim },
            );
        }
        if self.sim.tick.is_multiple_of(STATE_EVERY_TICKS) {
            let state = S2C::State {
                tick: self.sim.tick,
                players: self
                    .sim
                    .players
                    .iter()
                    .map(|player| PState {
                        id: player.id,
                        x: player.pos[0],
                        z: player.pos[1],
                        y: player.y,
                        ax: player.aim[0],
                        az: player.aim[1],
                        pitch: player.pitch,
                        hp: player.hp,
                        score: player.score,
                        alive: player.alive,
                        crouch: player.crouch,
                        shield: player.shield,
                        weapon: player.weapon,
                        ammo: player.ammo,
                        reloading: player.reload_t > 0.0,
                        deaths: player.death_count,
                        ack: self
                            .inputs
                            .get(&player.id)
                            .map_or(0, |record| record.sequence),
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
                        y: bullet.y,
                        vy: bullet.vy,
                        owner: bullet.owner,
                    })
                    .collect(),
                pads: self
                    .sim
                    .pads
                    .iter()
                    .map(|pad| pad.respawn_t <= 0.0)
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
        let stall_limit = following
            .saturating_add(FIXED_STEP_MICROS.saturating_mul(STALL_GRACE_STEPS));
        if now > stall_limit {
            self.run_tick(update);
            self.next_tick_at = MonotonicTimestamp::from_micros(
                now.saturating_add(FIXED_STEP_MICROS),
            );
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
    fn step(
        &mut self,
        timestamp: MonotonicTimestamp,
        inputs: Vec<SessionInput>,
    ) -> SessionUpdate {
        self.step_with_transport(
            timestamp,
            inputs
                .into_iter()
                .map(|input| SessionInputWithTransport {
                    peer_id: input.peer_id,
                    received_at: input.received_at,
                    transport_rtt: None,
                    input: input.input,
                })
                .collect(),
        )
    }

    fn step_with_transport(
        &mut self,
        timestamp: MonotonicTimestamp,
        inputs: Vec<SessionInputWithTransport>,
    ) -> SessionUpdate {
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

fn bounded_tick_age(current: u64, earlier: u64) -> u16 {
    u16::try_from(current.saturating_sub(earlier).min(u64::from(u16::MAX)))
        .unwrap_or(u16::MAX)
}

/// Canonical lobby data projected into Arena's legacy `LobbyList` message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyLobbyEntry {
    /// Exact hosted key owning the lobby.
    pub game_key: GameKey,
    /// Legacy lobby name.
    pub name: String,
    /// Display handle of the first admitted member.
    pub host: String,
    /// Whether admission requires a password.
    pub has_password: bool,
    /// Current admitted player count.
    pub players: u8,
    /// Maximum admitted player count.
    pub cap: u8,
}

/// Host action decoded from Arena's pre-consolidation browsing wire.
#[derive(Debug)]
pub enum ArenaLegacyAction {
    /// Records the legacy protocol selection and returns the frozen welcome.
    Hello {
        /// Sanitized legacy handle.
        handle: String,
        /// Protocol version supplied by the frozen client.
        requested_version: u16,
        /// Frozen welcome response.
        response: S2C,
    },
    /// Requests a projection of Arena lobbies.
    ListLobbies {
        /// Protocol version recorded by the hello, including zero for the hub browser.
        requested_version: u16,
    },
    /// Requests exact creation under the legacy hello's selector.
    CreateLobby {
        /// Canonical game selection synthesized from the legacy hello.
        game_key: GameKey,
        /// Sanitized lobby name.
        name: String,
        /// Sanitized optional password.
        password: Option<String>,
    },
    /// Requests exact joining under the legacy hello's selector.
    JoinLobby {
        /// Canonical game selection synthesized from the legacy hello.
        game_key: GameKey,
        /// Sanitized lobby name.
        name: String,
        /// Password bytes as supplied by the legacy client.
        password: Option<String>,
    },
    /// Requests departure from the current legacy lobby.
    LeaveLobby,
    /// Returns an immediate legacy response such as Pong.
    Reply(S2C),
    /// Ignores a gameplay input received while still browsing, as the deployed server did.
    Ignore,
}

/// Stateful decoder for the manifest selector `legacy_game=arena`.
#[derive(Debug, Default)]
pub struct ArenaLegacyDecoder {
    protocol: Option<u16>,
}

/// Stateless registry factory for independent Arena legacy connections.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaLegacyIngressFactory;

impl LegacyIngressFactory for ArenaLegacyIngressFactory {
    fn create(&self) -> Box<dyn LegacyIngress> {
        Box::new(ArenaLegacyDecoder::new())
    }
}

impl ArenaLegacyDecoder {
    /// Constructs a decoder awaiting the deployed legacy hello.
    #[must_use]
    pub const fn new() -> Self {
        Self { protocol: None }
    }

    /// Decodes one legacy browsing frame and synthesizes its canonical selection action.
    ///
    /// # Errors
    ///
    /// Returns the frozen codec error category for binary, malformed, repeated-hello, or
    /// pre-hello frames.
    pub fn decode(&mut self, frame: &InnerFrame) -> Result<ArenaLegacyAction, InnerCodecError> {
        let InnerFrame::Text(text) = frame else {
            return Err(InnerCodecError::WrongFrameKind);
        };
        let message: C2S = serde_json::from_str(text)
            .map_err(|error| InnerCodecError::DecodeFailed(error.to_string()))?;
        match (message, self.protocol) {
            (C2S::Hello { proto, handle }, None) => {
                self.protocol = Some(proto);
                let handle = sanitize_text(&handle, MAX_HANDLE_LEN);
                Ok(ArenaLegacyAction::Hello {
                    handle: if handle.is_empty() {
                        "player".to_string()
                    } else {
                        handle
                    },
                    requested_version: proto,
                    response: S2C::Welcome {
                        proto: PROTO_VERSION,
                        motd: "ember arena — cubes with guns".to_string(),
                    },
                })
            }
            (C2S::Hello { .. }, Some(_)) => Err(InnerCodecError::InvalidFrame(
                "duplicate legacy hello".to_string(),
            )),
            (_, None) => Err(InnerCodecError::InvalidFrame(
                "legacy message before hello".to_string(),
            )),
            (C2S::ListLobbies, Some(requested_version)) => {
                Ok(ArenaLegacyAction::ListLobbies { requested_version })
            }
            (C2S::CreateLobby { name, password }, Some(proto)) => {
                let password = password
                    .map(|value| sanitize_text(&value, MAX_PASSWORD_LEN))
                    .filter(|value| !value.is_empty());
                Ok(ArenaLegacyAction::CreateLobby {
                    game_key: GameKey {
                        game_id: GAME_ID.to_string(),
                        game_version: u32::from(proto),
                    },
                    name: sanitize_text(&name, MAX_LOBBY_LEN),
                    password,
                })
            }
            (C2S::JoinLobby { name, password }, Some(proto)) => {
                let password = password.map(|value| sanitize_text(&value, MAX_PASSWORD_LEN));
                Ok(ArenaLegacyAction::JoinLobby {
                    game_key: GameKey {
                        game_id: GAME_ID.to_string(),
                        game_version: u32::from(proto),
                    },
                    name: sanitize_text(&name, MAX_LOBBY_LEN),
                    password,
                })
            }
            (C2S::LeaveLobby, Some(_)) => Ok(ArenaLegacyAction::LeaveLobby),
            (C2S::Ping { nonce }, Some(_)) => {
                Ok(ArenaLegacyAction::Reply(S2C::Pong { nonce }))
            }
            (C2S::Input { .. }, Some(_)) => Ok(ArenaLegacyAction::Ignore),
        }
    }

    /// Projects exact Arena-9 entries into the deployed legacy list tag and fields.
    #[must_use]
    pub fn project_lobby_list(entries: &[LegacyLobbyEntry]) -> S2C {
        let lobbies = entries
            .iter()
            .filter(|entry| {
                entry.game_key.game_id == GAME_ID
                    && entry.game_key.game_version == u32::from(PROTO_VERSION)
            })
            .map(|entry| LobbyInfo {
                name: entry.name.clone(),
                host: entry.host.clone(),
                has_password: entry.has_password,
                players: entry.players,
                cap: entry.cap,
            })
            .collect();
        S2C::LobbyList { lobbies }
    }

    /// Builds the deployed protocol mismatch refusal for the hosted v9 set.
    #[must_use]
    pub fn version_refusal(requested_version: u16, hosted_versions: &[u32]) -> S2C {
        let message = if hosted_versions == [u32::from(PROTO_VERSION)] {
            format!(
                "this build speaks protocol v{requested_version}, the live game is v{PROTO_VERSION} — play the live version"
            )
        } else {
            let hosted = hosted_versions
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "this build speaks protocol v{requested_version}, hosted Arena versions: {hosted}"
            )
        };
        S2C::Error { message }
    }

    /// Wraps a canonical admission refusal in Arena's deployed recoverable error variant.
    #[must_use]
    pub fn admission_refusal(message: impl Into<String>) -> S2C {
        S2C::Error {
            message: message.into(),
        }
    }
}

impl LegacyIngress for ArenaLegacyDecoder {
    fn decode(
        &mut self,
        state: LegacyConnectionState,
        frame: &InnerFrame,
    ) -> Result<LegacyIngressAction, LegacyIngressError> {
        let action = Self::decode(self, frame)?;
        let action = match action {
            ArenaLegacyAction::Hello {
                handle,
                requested_version,
                response,
            } => LegacyIngressAction::Hello {
                selection: GameKey {
                    game_id: GAME_ID.to_string(),
                    game_version: u32::from(requested_version),
                },
                handle,
                response: encode_legacy_message(&response)?,
            },
            ArenaLegacyAction::ListLobbies { .. } => LegacyIngressAction::ListLobbies,
            ArenaLegacyAction::CreateLobby { .. }
                if state == LegacyConnectionState::Joined => {
                LegacyIngressAction::DispatchInner(ArenaCodec.decode(frame)?)
            }
            ArenaLegacyAction::CreateLobby {
                game_key,
                name,
                password,
            } => LegacyIngressAction::CreateLobby {
                selection: game_key,
                lobby_name: name,
                password,
            },
            ArenaLegacyAction::JoinLobby { .. }
                if state == LegacyConnectionState::Joined => {
                LegacyIngressAction::DispatchInner(ArenaCodec.decode(frame)?)
            }
            ArenaLegacyAction::JoinLobby {
                game_key,
                name,
                password,
            } => LegacyIngressAction::JoinLobby {
                selection: game_key,
                lobby_name: name,
                password,
            },
            ArenaLegacyAction::LeaveLobby => LegacyIngressAction::LeaveLobby,
            ArenaLegacyAction::Reply(response) => {
                LegacyIngressAction::Reply(encode_legacy_message(&response)?)
            }
            ArenaLegacyAction::Ignore if state == LegacyConnectionState::Joined => {
                LegacyIngressAction::DispatchInner(ArenaCodec.decode(frame)?)
            }
            ArenaLegacyAction::Ignore => LegacyIngressAction::Ignore,
        };
        Ok(action)
    }

    fn project_lobbies(
        &self,
        entries: &[LegacyLobbyProjection],
    ) -> Result<InnerFrame, LegacyIngressError> {
        let entries = entries
            .iter()
            .map(|entry| LegacyLobbyEntry {
                game_key: entry.game_key.clone(),
                name: entry.lobby_name.clone(),
                host: entry.host_handle.clone(),
                has_password: entry.password_protected,
                players: u8::try_from(entry.occupancy).unwrap_or(u8::MAX),
                cap: u8::try_from(entry.capacity).unwrap_or(u8::MAX),
            })
            .collect::<Vec<_>>();
        encode_legacy_message(&Self::project_lobby_list(&entries))
    }

    fn project_refusal(
        &self,
        refusal: &LegacyIngressRefusal,
    ) -> Result<InnerFrame, LegacyIngressError> {
        let response = match refusal {
            LegacyIngressRefusal::GameNotHosted {
                requested_game,
                hosted_games,
            } => Self::admission_refusal(format!(
                "game {requested_game} is not hosted; hosted games: {}",
                hosted_games.join(", ")
            )),
            LegacyIngressRefusal::VersionNotHosted {
                requested,
                hosted_versions,
            } => Self::version_refusal(
                u16::try_from(requested.game_version).unwrap_or(u16::MAX),
                hosted_versions,
            ),
            LegacyIngressRefusal::InvalidRequest { message } => {
                Self::admission_refusal(message.clone())
            }
            LegacyIngressRefusal::LobbyNotFound { lobby_name } => {
                Self::admission_refusal(format!("lobby \"{lobby_name}\" does not exist"))
            }
            LegacyIngressRefusal::LobbyAlreadyExists { lobby_name } => {
                Self::admission_refusal(format!("lobby \"{lobby_name}\" already exists"))
            }
            LegacyIngressRefusal::PasswordRejected => {
                Self::admission_refusal("password does not match")
            }
            LegacyIngressRefusal::LobbyFull => Self::admission_refusal("lobby is full"),
            LegacyIngressRefusal::ServerAtCapacity => {
                Self::admission_refusal("server capacity is full")
            }
            LegacyIngressRefusal::Draining => {
                Self::admission_refusal("server is draining; new admission is stopped")
            }
            LegacyIngressRefusal::AdmissionRefused { code, message } => {
                Self::admission_refusal(format!("{code}: {message}"))
            }
            LegacyIngressRefusal::InternalError => {
                Self::admission_refusal("internal session boundary failure")
            }
        };
        encode_legacy_message(&response)
    }
}

fn encode_legacy_message(message: &S2C) -> Result<InnerFrame, LegacyIngressError> {
    serde_json::to_string(message)
        .map(InnerFrame::Text)
        .map_err(|error| LegacyIngressError::EncodeFailed(error.to_string()))
}
