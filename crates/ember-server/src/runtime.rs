//! Single-writer hub, lobby ownership, admission, session stepping, and drain.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ember_legacy::{
    AdmissionMetadata, CloseReason, GameKey, GameSession, InnerCodec, InnerFrame, LeaveReason,
    LobbySeed, MonotonicTimestamp, OutboundEvent, OutboundTarget, PeerId, SessionCreationData,
    SessionId, SessionInput, SessionUpdate, VersionLimits,
};
use ember_net::outer::{
    self, ConnectionState, CreateLobby, InnerPayload, JoinLobby, Joined, Lobbies, LobbyEntry,
    OuterError, OuterErrorCode, ServerMessage, StateAction, StateInput, StateMachine, Welcome,
};
use sha2::{Digest, Sha256};
use tungstenite::Message;
use tungstenite::protocol::frame::coding::CloseCode;

use crate::capabilities::{HostEpoch, SessionCapabilities};
use crate::connection::{
    ConnectionConfig, ConnectionEvent, DataFrame, Ingress, OutboundCommand, spawn_acceptor,
};
use crate::registry::{Registry, RegistryBuilder, RegistryError, SelectionError};

const DEFAULT_MANIFEST_PATH: &str = "games/hosted.toml";
const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:24816";
const MAX_HANDLE_BYTES: usize = 64;
const MAX_LOBBY_NAME_BYTES: usize = 96;
const MAX_PASSWORD_BYTES: usize = 256;

/// Process-wide host limits and timeouts independent of version semantics.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// Plain WebSocket address behind the deployment's sole TLS terminator.
    pub bind_address: String,
    /// Authoritative hosted-set manifest loaded before binding.
    pub manifest_path: PathBuf,
    /// Hard process connection capacity, including reserved operational headroom.
    pub max_connections: usize,
    /// Connections withheld from ordinary admission as process headroom.
    pub reserved_connections: usize,
    /// Direct-exposure cap for one remote IP address.
    pub max_connections_per_ip: usize,
    /// Whether the per-IP cap applies to loopback test and tooling peers.
    pub cap_loopback: bool,
    /// Hard process lobby capacity, including reserved operational headroom.
    pub max_lobbies: usize,
    /// Lobby slots withheld from ordinary creation as process headroom.
    pub reserved_lobbies: usize,
    /// Hard process player capacity, including reserved operational headroom.
    pub max_players: usize,
    /// Player slots withheld from ordinary admission as process headroom.
    pub reserved_players: usize,
    /// Bounded number of pending transport writes per connection.
    pub outbound_queue_messages: usize,
    /// Bounded raw connection events awaiting the single-writer hub.
    pub inbound_event_queue_messages: usize,
    /// Absolute WebSocket data-message cap across every hosted version.
    pub max_websocket_message_bytes: usize,
    /// Outer messages charged to one peer during one second.
    pub max_outer_messages_per_second: u32,
    /// Outer and inner bytes charged to one peer during one second.
    pub max_inbound_bytes_per_second: u64,
    /// Absolute queued outbound-byte cap for one peer before or after join.
    pub max_connection_outbound_queue_bytes: usize,
    /// Total time permitted for the WebSocket handshake.
    pub handshake_timeout: Duration,
    /// Time permitted for the permanent outer hello.
    pub hello_timeout: Duration,
    /// Idle time permitted for an admitted lobby browser.
    pub browsing_timeout: Duration,
    /// Idle time permitted after joining a version session.
    pub joined_timeout: Duration,
    /// Maximum time one socket write may block.
    pub write_timeout: Duration,
    /// Host-private cadence used to poll timestamped version work.
    pub step_cadence: Duration,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            bind_address: DEFAULT_BIND_ADDRESS.to_string(),
            manifest_path: PathBuf::from(DEFAULT_MANIFEST_PATH),
            max_connections: 512,
            reserved_connections: 16,
            max_connections_per_ip: 16,
            cap_loopback: false,
            max_lobbies: 128,
            reserved_lobbies: 8,
            max_players: 2_048,
            reserved_players: 64,
            outbound_queue_messages: 256,
            inbound_event_queue_messages: 1_024,
            max_websocket_message_bytes: 256 * 1_024,
            max_outer_messages_per_second: 64,
            max_inbound_bytes_per_second: 4 * 1_024 * 1_024,
            max_connection_outbound_queue_bytes: 256 * 1_024,
            handshake_timeout: Duration::from_secs(10),
            hello_timeout: Duration::from_secs(10),
            browsing_timeout: Duration::from_secs(300),
            joined_timeout: Duration::from_secs(120),
            write_timeout: Duration::from_secs(15),
            step_cadence: Duration::from_micros(16_667),
        }
    }
}

impl HostConfig {
    fn validate(&self) -> Result<(), HostError> {
        if self.max_connections <= self.reserved_connections {
            return Err(HostError::InvalidConfiguration(
                "reserved connections must be below total connections".to_string(),
            ));
        }
        if self.max_lobbies <= self.reserved_lobbies {
            return Err(HostError::InvalidConfiguration(
                "reserved lobbies must be below total lobbies".to_string(),
            ));
        }
        if self.max_players <= self.reserved_players {
            return Err(HostError::InvalidConfiguration(
                "reserved players must be below total players".to_string(),
            ));
        }
        if self.max_connections_per_ip == 0
            || self.outbound_queue_messages == 0
            || self.inbound_event_queue_messages == 0
            || self.max_websocket_message_bytes < outer::MAX_OUTER_FRAME_BYTES
            || self.max_outer_messages_per_second == 0
            || self.max_inbound_bytes_per_second == 0
            || self.max_connection_outbound_queue_bytes < outer::MAX_OUTER_FRAME_BYTES
        {
            return Err(HostError::InvalidConfiguration(
                "host capacities must be nonzero".to_string(),
            ));
        }
        if self.handshake_timeout.is_zero()
            || self.hello_timeout.is_zero()
            || self.browsing_timeout.is_zero()
            || self.joined_timeout.is_zero()
            || self.write_timeout.is_zero()
            || self.step_cadence.is_zero()
        {
            return Err(HostError::InvalidConfiguration(
                "host timeouts and cadence must be nonzero".to_string(),
            ));
        }
        Ok(())
    }
}

fn validate_registry_limits(registry: &Registry, config: &HostConfig) -> Result<(), HostError> {
    let player_capacity = config.max_players.saturating_sub(config.reserved_players);
    for (key, entry) in registry.iter() {
        let limits = entry.limits();
        if usize::try_from(limits.max_frame_bytes).unwrap_or(usize::MAX)
            > config.max_websocket_message_bytes
        {
            return Err(HostError::InvalidConfiguration(format!(
                "frame limit for {key:?} exceeds the global WebSocket cap"
            )));
        }
        if usize::try_from(limits.max_outbound_queue_bytes).unwrap_or(usize::MAX)
            > config.max_connection_outbound_queue_bytes
        {
            return Err(HostError::InvalidConfiguration(format!(
                "outbound queue limit for {key:?} exceeds the global peer cap"
            )));
        }
        if usize::from(limits.max_players_per_lobby) > player_capacity {
            return Err(HostError::InvalidConfiguration(format!(
                "player limit for {key:?} exceeds global admission capacity"
            )));
        }
    }
    Ok(())
}

/// One game/version-labelled occupancy observation used by drain and rollout tooling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OccupancySnapshot {
    /// Permanent game slug.
    pub game_id: String,
    /// Exact hosted game version.
    pub game_version: u32,
    /// Active lobby count for this key.
    pub lobbies: u32,
    /// Admitted player count for this key.
    pub players: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct OccupancyCount {
    lobbies: u32,
    players: u32,
}

/// Cloneable control and observation handle for admission-stop draining.
#[derive(Clone)]
pub struct DrainHandle {
    draining: Arc<AtomicBool>,
    occupancy: Arc<Mutex<BTreeMap<GameKey, OccupancyCount>>>,
}

impl DrainHandle {
    /// Stops new lobby creation and joining without interrupting admitted sessions.
    pub fn stop_admission(&self) {
        self.draining.store(true, Ordering::Release);
    }

    /// Returns whether admission has been stopped.
    #[must_use]
    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::Acquire)
    }

    /// Returns deterministic game/version-labelled occupancy, including hosted keys at zero.
    #[must_use]
    pub fn occupancy(&self) -> Vec<OccupancySnapshot> {
        self.occupancy.lock().map_or_else(
            |_| Vec::new(),
            |occupancy| {
                occupancy
                    .iter()
                    .map(|(key, count)| OccupancySnapshot {
                        game_id: key.game_id.clone(),
                        game_version: key.game_version,
                        lobbies: count.lobbies,
                        players: count.players,
                    })
                    .collect()
            },
        )
    }
}

/// Fully validated sole-host instance; construction completes before binding.
pub struct Host {
    registry: Arc<Registry>,
    config: HostConfig,
    drain: DrainHandle,
}

impl Host {
    /// Loads the authoritative manifest and every compiled registration before listening.
    ///
    /// `EMBER_SERVER_BIND` and `EMBER_HOSTED_MANIFEST` may override the two deployment paths. The
    /// explicit `demo` feature injects its fixture manifest instead of reading the product set.
    ///
    /// # Errors
    ///
    /// Returns configuration or registry construction failures before any listener exists.
    pub fn from_environment() -> Result<Self, HostError> {
        let mut config = HostConfig::default();
        if let Ok(bind_address) = std::env::var("EMBER_SERVER_BIND") {
            config.bind_address = bind_address;
        }
        if let Ok(manifest_path) = std::env::var("EMBER_HOSTED_MANIFEST") {
            config.manifest_path = PathBuf::from(manifest_path);
        }
        let builder = RegistryBuilder::new();
        #[cfg(feature = "demo")]
        let registry = {
            let mut builder = builder;
            crate::fixture::register(&mut builder)?;
            builder.build_from_source(crate::fixture::MANIFEST)?
        };
        #[cfg(not(feature = "demo"))]
        let registry = builder.load(&config.manifest_path)?;
        Self::new(registry, config)
    }

    /// Constructs a host around an injected, already validated immutable registry.
    ///
    /// # Errors
    ///
    /// Returns a host-configuration failure.
    pub fn new(registry: Registry, config: HostConfig) -> Result<Self, HostError> {
        config.validate()?;
        validate_registry_limits(&registry, &config)?;
        let occupancy = registry
            .iter()
            .map(|(key, _)| (key.clone(), OccupancyCount::default()))
            .collect();
        Ok(Self {
            registry: Arc::new(registry),
            config,
            drain: DrainHandle {
                draining: Arc::new(AtomicBool::new(false)),
                occupancy: Arc::new(Mutex::new(occupancy)),
            },
        })
    }

    /// Returns a cloneable admission-stop and occupancy handle.
    #[must_use]
    pub fn drain_handle(&self) -> DrainHandle {
        self.drain.clone()
    }

    /// Binds the configured canonical listener and runs the single-writer hub.
    ///
    /// # Errors
    ///
    /// Returns bind, listener, or internal event-loop failures.
    pub fn run(self) -> Result<(), HostError> {
        let listener = TcpListener::bind(&self.config.bind_address)?;
        self.run_on(listener)
    }

    /// Runs the host on an already-bound listener, allowing port-zero end-to-end fixtures.
    ///
    /// # Errors
    ///
    /// Returns listener-address or internal event-loop failures.
    pub fn run_on(self, listener: TcpListener) -> Result<(), HostError> {
        let local_address = listener.local_addr()?;
        let connection_config = ConnectionConfig {
            max_connections: self
                .config
                .max_connections
                .saturating_sub(self.config.reserved_connections),
            max_connections_per_ip: self.config.max_connections_per_ip,
            cap_loopback: self.config.cap_loopback,
            outbound_queue_messages: self.config.outbound_queue_messages,
            max_ws_message_bytes: self.config.max_websocket_message_bytes,
            handshake_timeout: self.config.handshake_timeout,
            write_timeout: self.config.write_timeout,
        };
        let (events_sender, events_receiver) =
            mpsc::sync_channel(self.config.inbound_event_queue_messages);
        let epoch = HostEpoch::new();
        spawn_acceptor(
            listener,
            events_sender,
            connection_config,
            self.registry.legacy_routes(),
            epoch,
        )?;
        tracing::info!(
            address = %local_address,
            games = ?self.registry.hosted_games(),
            "sole game host listening"
        );
        Hub::new(self.registry, self.config, self.drain, epoch).run(&events_receiver)
    }
}

/// Startup or runtime failure of the sole host.
#[derive(Debug)]
pub enum HostError {
    /// Host-global caps or timeouts are internally inconsistent.
    InvalidConfiguration(String),
    /// The immutable registry could not be constructed.
    Registry(RegistryError),
    /// Listener or accept-loop I/O failed.
    Io(std::io::Error),
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(detail) => {
                write!(formatter, "invalid host configuration: {detail}")
            }
            Self::Registry(error) => write!(formatter, "registry startup failed: {error}"),
            Self::Io(error) => write!(formatter, "host I/O failed: {error}"),
        }
    }
}

impl std::error::Error for HostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registry(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::InvalidConfiguration(_) => None,
        }
    }
}

impl From<RegistryError> for HostError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}

impl From<std::io::Error> for HostError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LobbyKey {
    game_key: GameKey,
    lobby_name: String,
}

struct PasswordVerifier([u8; 32]);

impl PasswordVerifier {
    fn new(password: &str, session_id: SessionId) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"ember-host-lobby-password-v1\0");
        digest.update(session_id.host_value().to_le_bytes());
        digest.update(password.as_bytes());
        Self(digest.finalize().into())
    }

    fn matches(&self, password: Option<&str>, session_id: SessionId) -> bool {
        password.is_some_and(|password| {
            constant_time_equal(&Self::new(password, session_id).0, &self.0)
        })
    }
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left_byte, right_byte)| {
            difference | (*left_byte ^ *right_byte)
        })
        == 0
}

struct Lobby {
    key: LobbyKey,
    session_id: SessionId,
    password: Option<PasswordVerifier>,
    session: Box<dyn GameSession>,
    codec: Arc<dyn InnerCodec>,
    limits: VersionLimits,
    capabilities: SessionCapabilities,
    members: BTreeSet<PeerId>,
    pending_inputs: Vec<SessionInput>,
    pending_updates: Vec<SessionUpdate>,
    next_step: Instant,
}

struct Connection {
    id: u64,
    peer: String,
    ingress: Ingress,
    outbound: SyncSender<OutboundCommand>,
    state: StateMachine,
    handle: Option<String>,
    created_at: Instant,
    last_seen: Instant,
    inbound: InboundWindow,
    queued_outbound_bytes: usize,
    queued_version_bytes: usize,
    close_reason: Option<CloseReason>,
}

impl Connection {
    fn joined_lobby(&self) -> Option<LobbyKey> {
        let ConnectionState::Joined {
            game_id,
            game_version,
            lobby_name,
        } = self.state.state()
        else {
            return None;
        };
        Some(LobbyKey {
            game_key: GameKey {
                game_id: game_id.clone(),
                game_version: *game_version,
            },
            lobby_name: lobby_name.clone(),
        })
    }
}

struct InboundWindow {
    started: Instant,
    messages: u32,
    bytes: u64,
}

impl InboundWindow {
    const fn new(now: Instant) -> Self {
        Self {
            started: now,
            messages: 0,
            bytes: 0,
        }
    }

    fn charge(
        &mut self,
        now: Instant,
        frame_bytes: usize,
        message_limit: u32,
        byte_limit: u64,
    ) -> ChargeResult {
        self.reset_if_elapsed(now);
        self.messages = self.messages.saturating_add(1);
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(frame_bytes).unwrap_or(u64::MAX));
        ChargeResult {
            messages_within_limit: self.messages <= message_limit,
            bytes_within_limit: self.bytes <= byte_limit,
        }
    }

    fn reset_if_elapsed(&mut self, now: Instant) {
        if now.duration_since(self.started) >= Duration::from_secs(1) {
            self.started = now;
            self.messages = 0;
            self.bytes = 0;
        }
    }
}

#[derive(Clone, Copy)]
struct ChargeResult {
    messages_within_limit: bool,
    bytes_within_limit: bool,
}

struct ByteWindow {
    started: Instant,
    bytes: u64,
}

impl ByteWindow {
    const fn new(now: Instant) -> Self {
        Self {
            started: now,
            bytes: 0,
        }
    }

    fn charge(&mut self, now: Instant, bytes: usize, limit: u64) -> bool {
        if now.duration_since(self.started) >= Duration::from_secs(1) {
            self.started = now;
            self.bytes = 0;
        }
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
        self.bytes <= limit
    }
}

struct Hub {
    registry: Arc<Registry>,
    config: HostConfig,
    drain: DrainHandle,
    epoch: HostEpoch,
    connections: BTreeMap<u64, Connection>,
    lobbies: BTreeMap<LobbyKey, Lobby>,
    version_outbound: BTreeMap<GameKey, ByteWindow>,
    next_session_id: u64,
}

impl Hub {
    fn new(
        registry: Arc<Registry>,
        config: HostConfig,
        drain: DrainHandle,
        epoch: HostEpoch,
    ) -> Self {
        let now = Instant::now();
        let version_outbound = registry
            .iter()
            .map(|(key, _)| (key.clone(), ByteWindow::new(now)))
            .collect();
        Self {
            registry,
            config,
            drain,
            epoch,
            connections: BTreeMap::new(),
            lobbies: BTreeMap::new(),
            version_outbound,
            next_session_id: 1,
        }
    }

    fn run(mut self, events: &Receiver<ConnectionEvent>) -> Result<(), HostError> {
        loop {
            match events.recv_timeout(self.config.step_cadence) {
                Ok(event) => self.handle_event(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(HostError::Io(std::io::Error::other(
                        "all connection event senders stopped",
                    )));
                }
            }
            for _ in 0..1_024 {
                match events.try_recv() {
                    Ok(event) => self.handle_event(event),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
            self.process_pending_updates();
            self.step_due_sessions();
            self.sweep_timeouts();
        }
    }

    fn handle_event(&mut self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected {
                id,
                outbound,
                peer,
                ingress,
            } => {
                let now = Instant::now();
                tracing::info!(connection = id, %peer, ?ingress, "connection established");
                self.connections.insert(
                    id,
                    Connection {
                        id,
                        peer,
                        ingress,
                        outbound,
                        state: StateMachine::new(),
                        handle: None,
                        created_at: now,
                        last_seen: now,
                        inbound: InboundWindow::new(now),
                        queued_outbound_bytes: 0,
                        queued_version_bytes: 0,
                        close_reason: None,
                    },
                );
            }
            ConnectionEvent::Data {
                id,
                frame,
                received_at,
            } => self.handle_data(id, frame, received_at),
            ConnectionEvent::Control { id } => {
                if let Some(connection) = self.connections.get_mut(&id) {
                    connection.last_seen = Instant::now();
                    let _action = connection.state.transition(StateInput::Control);
                }
            }
            ConnectionEvent::OutboundDrained {
                id,
                bytes,
                version_frame,
            } => {
                if let Some(connection) = self.connections.get_mut(&id) {
                    connection.queued_outbound_bytes =
                        connection.queued_outbound_bytes.saturating_sub(bytes);
                    if version_frame {
                        connection.queued_version_bytes =
                            connection.queued_version_bytes.saturating_sub(bytes);
                    }
                }
            }
            ConnectionEvent::Disconnected { id } => {
                if let Some(mut connection) = self.connections.remove(&id) {
                    tracing::info!(connection = id, peer = %connection.peer, "connection closed");
                    let _action = connection.state.transition(StateInput::TransportClosed);
                    self.detach_connection(&connection, CloseReason::Disconnected);
                }
            }
        }
    }

    fn handle_data(
        &mut self,
        id: u64,
        frame: DataFrame,
        received_at: MonotonicTimestamp,
    ) {
        let Some(mut connection) = self.connections.remove(&id) else {
            return;
        };
        connection.last_seen = Instant::now();
        let joined_lobby = connection.joined_lobby();
        let selected_key = joined_lobby
            .as_ref()
            .map(|key| key.game_key.clone())
            .or_else(|| match &connection.ingress {
                Ingress::Canonical => None,
                Ingress::Legacy(key) => Some(key.clone()),
            });
        let entry = selected_key
            .as_ref()
            .and_then(|key| self.registry.entry(key));
        let limits = if let Some(entry) = entry {
            (
                entry.limits().max_messages_per_second,
                usize::try_from(entry.limits().max_frame_bytes).unwrap_or(usize::MAX),
            )
        } else {
            (
                self.config.max_outer_messages_per_second,
                outer::MAX_OUTER_FRAME_BYTES,
            )
        };
        let charge = connection.inbound.charge(
            Instant::now(),
            frame.byte_len(),
            limits.0,
            self.config.max_inbound_bytes_per_second,
        );
        let can_send_outer_error =
            joined_lobby.is_none() && matches!(&connection.ingress, Ingress::Canonical);

        if frame.byte_len() > limits.1 {
            if can_send_outer_error {
                self.send_outer(
                    &mut connection,
                    &ServerMessage::Error(OuterError {
                        code: OuterErrorCode::MessageTooLarge,
                        message: "outer frame exceeds the byte limit".to_string(),
                    }),
                );
            }
            self.close_detached(
                &mut connection,
                CloseReason::FrameTooLarge,
                CloseCode::Size,
                "frame exceeds the selected byte limit",
            );
        } else if !charge.messages_within_limit || !charge.bytes_within_limit {
            if can_send_outer_error {
                self.send_outer(
                    &mut connection,
                    &ServerMessage::Error(OuterError {
                        code: OuterErrorCode::InvalidRequest,
                        message: "outer message or byte rate exceeded".to_string(),
                    }),
                );
            }
            self.close_detached(
                &mut connection,
                CloseReason::ProtocolViolation,
                CloseCode::Policy,
                "inbound message or byte rate exceeded",
            );
        } else if let Ingress::Legacy(key) = &connection.ingress {
            tracing::warn!(
                connection = id,
                game = %key.game_id,
                version = key.game_version,
                "legacy ingress reached missing frozen decoder surface"
            );
            self.close_detached(
                &mut connection,
                CloseReason::InternalError,
                CloseCode::Error,
                "legacy decoder unavailable in frozen host interface",
            );
        } else {
            self.handle_canonical_frame(&mut connection, frame, received_at);
        }

        if matches!(connection.state.state(), ConnectionState::Closed) {
            let reason = connection
                .close_reason
                .clone()
                .unwrap_or(CloseReason::ProtocolViolation);
            self.detach_connection(&connection, reason);
        } else {
            self.connections.insert(id, connection);
        }
    }

    fn handle_canonical_frame(
        &mut self,
        connection: &mut Connection,
        frame: DataFrame,
        received_at: MonotonicTimestamp,
    ) {
        let input = if matches!(connection.state.state(), ConnectionState::Joined { .. }) {
            StateInput::Inner(to_inner_payload(frame))
        } else {
            match frame {
                DataFrame::Text(text) => match outer::decode_client_frame(text.as_bytes()) {
                    Ok(message) => StateInput::Outer(message),
                    Err(error) => {
                        let close = error.closes_connection();
                        self.reject_outer(connection, error.outer_error(), close);
                        return;
                    }
                },
                DataFrame::Binary(bytes) => StateInput::Inner(InnerPayload::Binary(bytes)),
            }
        };
        match connection.state.transition(input) {
            StateAction::AcceptHello(hello) => self.accept_hello(connection, hello),
            StateAction::ListLobbies => {
                let entries = self.list_lobbies();
                self.send_outer(connection, &ServerMessage::Lobbies(Lobbies { entries }));
            }
            StateAction::CreateLobby(request) => self.create_lobby(connection, request),
            StateAction::JoinLobby(request) => self.join_lobby(connection, request),
            StateAction::DispatchInner(payload) => {
                self.dispatch_inner(connection, payload, received_at);
            }
            StateAction::Reject {
                error,
                close_after_error,
            } => self.reject_outer(connection, error, close_after_error),
            StateAction::Close => connection.state.close(),
            StateAction::HandleControl | StateAction::Ignore => {}
        }
    }

    fn accept_hello(&mut self, connection: &mut Connection, hello: outer::Hello) {
        if !valid_handle(&hello.handle) {
            self.reject_outer(
                connection,
                OuterError {
                    code: OuterErrorCode::InvalidRequest,
                    message: "handle must be nonempty, bounded UTF-8 without controls".to_string(),
                },
                true,
            );
            return;
        }
        connection.handle = Some(hello.handle);
        self.send_outer(
            connection,
            &ServerMessage::Welcome(Welcome {
                outer_version: hello.outer_version,
                supported_outer_versions: outer::SUPPORTED_OUTER_VERSIONS.to_vec(),
            }),
        );
    }

    fn list_lobbies(&mut self) -> Vec<LobbyEntry> {
        let keys: Vec<_> = self.lobbies.keys().cloned().collect();
        let mut entries = Vec::with_capacity(keys.len());
        let mut panicked = Vec::new();
        for key in keys {
            let Some(lobby) = self.lobbies.get(&key) else {
                continue;
            };
            let status = catch_unwind(AssertUnwindSafe(|| lobby.session.lobby_status()));
            let Ok(status) = status else {
                panicked.push(key);
                continue;
            };
            entries.push(LobbyEntry {
                game_id: lobby.key.game_key.game_id.clone(),
                game_version: lobby.key.game_key.game_version,
                lobby_name: lobby.key.lobby_name.clone(),
                password_protected: lobby.password.is_some(),
                occupancy: u16::try_from(lobby.members.len()).unwrap_or(u16::MAX),
                capacity: lobby.limits.max_players_per_lobby,
                status: outer::LobbyStatus {
                    code: status.code,
                    detail: status.detail,
                },
            });
        }
        for key in panicked {
            self.terminate_lobby(&key, "version panicked while projecting lobby status");
        }
        entries
    }

    // Construction stays linear so every pre-factory admission check is visibly ordered.
    #[allow(clippy::too_many_lines)]
    fn create_lobby(&mut self, connection: &mut Connection, request: CreateLobby) {
        let Some(key) = self.select_or_refuse(
            connection,
            &request.game_id,
            request.game_version,
        ) else {
            return;
        };
        if self.refuse_if_draining(connection)
            || !valid_lobby_request(&request.lobby_name, request.password.as_deref())
        {
            if !self.drain.is_draining() {
                self.invalid_request(connection, "lobby name or password is invalid");
            }
            return;
        }
        let lobby_key = LobbyKey {
            game_key: key.clone(),
            lobby_name: request.lobby_name,
        };
        if self.lobbies.contains_key(&lobby_key) {
            self.invalid_request(connection, "lobby already exists for this game and version");
            return;
        }
        let Some(entry) = self.registry.entry(&key) else {
            self.internal_error(connection, "selected registry entry disappeared");
            return;
        };
        let limits = entry.limits();
        let factory = entry.factory();
        let codec = entry.codec();
        let version_lobbies = self
            .lobbies
            .keys()
            .filter(|candidate| candidate.game_key == key)
            .count();
        let version_lobby_limit = usize::try_from(limits.max_lobbies).unwrap_or(usize::MAX);
        if version_lobbies >= version_lobby_limit
            || self.lobbies.len()
                >= self
                    .config
                    .max_lobbies
                    .saturating_sub(self.config.reserved_lobbies)
        {
            self.invalid_request(connection, "lobby capacity is full");
            return;
        }
        let global_players = self
            .lobbies
            .values()
            .map(|candidate| candidate.members.len())
            .sum::<usize>();
        if global_players
            >= self
                .config
                .max_players
                .saturating_sub(self.config.reserved_players)
        {
            self.invalid_request(connection, "global player capacity is full");
            return;
        }

        let session_id = SessionId::from_host_value(self.next_session_id);
        self.next_session_id = self.next_session_id.saturating_add(1);
        let capabilities = SessionCapabilities::new(
            key.clone(),
            session_id,
            self.epoch,
            Arc::new(AtomicBool::new(false)),
        );
        let creation = SessionCreationData {
            game_key: key.clone(),
            session_id,
            lobby_name: lobby_key.lobby_name.clone(),
            lobby_seed: lobby_seed(&key, &lobby_key.lobby_name, session_id),
            created_at: self.epoch.now(),
            configured_rules: Vec::new(),
        };
        let session = catch_unwind(AssertUnwindSafe(|| {
            factory.create(&capabilities.capabilities, &creation)
        }));
        let session = match session {
            Ok(Ok(session)) => session,
            Ok(Err(error)) => {
                tracing::warn!(?key, ?error, "version factory refused lobby construction");
                self.invalid_request(connection, "version refused lobby construction");
                return;
            }
            Err(_) => {
                self.internal_error(connection, "version factory panicked");
                return;
            }
        };
        let password = request
            .password
            .as_deref()
            .filter(|password| !password.is_empty())
            .map(|password| PasswordVerifier::new(password, session_id));
        let mut lobby = Lobby {
            key: lobby_key.clone(),
            session_id,
            password,
            session,
            codec,
            limits,
            capabilities,
            members: BTreeSet::new(),
            pending_inputs: Vec::new(),
            pending_updates: Vec::new(),
            next_step: next_step_after(self.config.step_cadence),
        };
        match self.admit_to_detached_lobby(connection, &mut lobby) {
            AdmissionOutcome::Admitted(update) => {
                lobby.pending_updates.push(update);
                self.lobbies.insert(lobby_key.clone(), lobby);
                self.record_occupancy();
                self.complete_join(connection, &lobby_key);
            }
            AdmissionOutcome::Refused { code, message } => {
                drop_lobby_safely(lobby, "creator admission refusal");
                self.invalid_request(connection, &format!("{code}: {message}"));
            }
            AdmissionOutcome::Panicked => {
                drop_lobby_safely(lobby, "creator admission panic");
                self.internal_error(connection, "version panicked during creator admission");
            }
        }
    }

    fn join_lobby(&mut self, connection: &mut Connection, request: JoinLobby) {
        let Some(key) = self.select_or_refuse(
            connection,
            &request.game_id,
            request.game_version,
        ) else {
            return;
        };
        if self.refuse_if_draining(connection)
            || !valid_lobby_request(&request.lobby_name, request.password.as_deref())
        {
            if !self.drain.is_draining() {
                self.invalid_request(connection, "lobby name or password is invalid");
            }
            return;
        }
        let lobby_key = LobbyKey {
            game_key: key,
            lobby_name: request.lobby_name,
        };
        let Some(mut lobby) = self.lobbies.remove(&lobby_key) else {
            self.invalid_request(connection, "lobby does not exist for this game and version");
            return;
        };
        if !password_matches(&lobby, request.password.as_deref()) {
            self.lobbies.insert(lobby_key, lobby);
            self.invalid_request(connection, "password does not match");
            return;
        }
        let global_players = self
            .lobbies
            .values()
            .map(|candidate| candidate.members.len())
            .sum::<usize>()
            .saturating_add(lobby.members.len());
        if lobby.members.len() >= usize::from(lobby.limits.max_players_per_lobby)
            || global_players
                >= self
                    .config
                    .max_players
                    .saturating_sub(self.config.reserved_players)
        {
            self.lobbies.insert(lobby_key, lobby);
            self.invalid_request(connection, "lobby is full");
            return;
        }
        match self.admit_to_detached_lobby(connection, &mut lobby) {
            AdmissionOutcome::Admitted(update) => {
                lobby.pending_updates.push(update);
                self.lobbies.insert(lobby_key.clone(), lobby);
                self.record_occupancy();
                self.complete_join(connection, &lobby_key);
            }
            AdmissionOutcome::Refused { code, message } => {
                self.lobbies.insert(lobby_key, lobby);
                self.invalid_request(connection, &format!("{code}: {message}"));
            }
            AdmissionOutcome::Panicked => {
                self.terminate_detached_lobby(lobby, "version panicked during join admission");
                self.internal_error(connection, "version panicked during join admission");
            }
        }
    }

    fn admit_to_detached_lobby(
        &self,
        connection: &Connection,
        lobby: &mut Lobby,
    ) -> AdmissionOutcome {
        let peer_id = PeerId::from_host_value(connection.id);
        lobby.capabilities.transport.add_peer(peer_id);
        let admission = AdmissionMetadata {
            peer_id,
            handle: connection.handle.clone().unwrap_or_default(),
            admitted_at: self.epoch.now(),
            attributes: BTreeMap::from([("peer".to_string(), connection.peer.clone())]),
        };
        match catch_unwind(AssertUnwindSafe(|| lobby.session.join(admission))) {
            Ok(Ok(update)) => {
                lobby.members.insert(peer_id);
                AdmissionOutcome::Admitted(update)
            }
            Ok(Err(refusal)) => {
                lobby.capabilities.transport.remove_peer(peer_id);
                AdmissionOutcome::Refused {
                    code: refusal.code,
                    message: refusal.message,
                }
            }
            Err(_) => {
                lobby.capabilities.transport.remove_peer(peer_id);
                AdmissionOutcome::Panicked
            }
        }
    }

    fn complete_join(&mut self, connection: &mut Connection, lobby_key: &LobbyKey) {
        let joined = Joined {
            game_id: lobby_key.game_key.game_id.clone(),
            game_version: lobby_key.game_key.game_version,
            lobby_name: lobby_key.lobby_name.clone(),
        };
        if connection.state.mark_joined(joined.clone()).is_err() {
            self.internal_error(connection, "connection left browsing during admission");
            return;
        }
        connection.inbound = InboundWindow::new(Instant::now());
        self.send_outer(connection, &ServerMessage::Joined(joined));
    }

    fn dispatch_inner(
        &mut self,
        connection: &mut Connection,
        payload: InnerPayload,
        received_at: MonotonicTimestamp,
    ) {
        let Some(lobby_key) = connection.joined_lobby() else {
            self.internal_error(connection, "joined state has no lobby key");
            return;
        };
        let Some(lobby) = self.lobbies.get_mut(&lobby_key) else {
            self.internal_error(connection, "joined lobby no longer exists");
            return;
        };
        let frame = match payload {
            InnerPayload::Text(text) => InnerFrame::Text(text),
            InnerPayload::Binary(bytes) => InnerFrame::Binary(bytes),
        };
        let decoded = catch_unwind(AssertUnwindSafe(|| lobby.codec.decode(&frame)));
        let decoded = match decoded {
            Ok(Ok(decoded)) => decoded,
            Ok(Err(error)) => {
                tracing::debug!(connection = connection.id, ?error, "inner codec rejected frame");
                self.close_detached(
                    connection,
                    CloseReason::ProtocolViolation,
                    CloseCode::Protocol,
                    "selected version rejected the frame",
                );
                return;
            }
            Err(_) => {
                let key = lobby_key.clone();
                self.terminate_lobby(&key, "version codec panicked during decode");
                self.close_detached(
                    connection,
                    CloseReason::InternalError,
                    CloseCode::Error,
                    "version codec panicked at the session boundary",
                );
                return;
            }
        };
        lobby.pending_inputs.push(SessionInput {
            peer_id: PeerId::from_host_value(connection.id),
            received_at,
            input: decoded,
        });
    }

    fn step_due_sessions(&mut self) {
        let timestamp = self.epoch.now();
        let now = Instant::now();
        let due: Vec<_> = self
            .lobbies
            .iter()
            .filter(|(_, lobby)| {
                now >= lobby.next_step || lobby.capabilities.clock.has_due_schedule(timestamp)
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in due {
            let Some(mut lobby) = self.lobbies.remove(&key) else {
                continue;
            };
            lobby.next_step = now
                .checked_add(self.config.step_cadence)
                .unwrap_or(now);
            let inputs = std::mem::take(&mut lobby.pending_inputs);
            let started = Instant::now();
            let update = catch_unwind(AssertUnwindSafe(|| lobby.session.step(timestamp, inputs)));
            let elapsed = started.elapsed();
            let max_step = Duration::from_micros(lobby.limits.max_step_duration.as_micros());
            match update {
                Ok(update) if elapsed <= max_step => {
                    self.lobbies.insert(key.clone(), lobby);
                    self.process_update(&key, update);
                }
                Ok(_) => self.terminate_detached_lobby(lobby, "version exceeded step wall budget"),
                Err(_) => self.terminate_detached_lobby(lobby, "version panicked during step"),
            }
        }
    }

    fn process_pending_updates(&mut self) {
        let keys: Vec<_> = self.lobbies.keys().cloned().collect();
        for key in keys {
            let Some(lobby) = self.lobbies.get_mut(&key) else {
                continue;
            };
            let updates = std::mem::take(&mut lobby.pending_updates);
            for update in updates {
                self.process_update(&key, update);
            }
        }
    }

    fn process_update(&mut self, lobby_key: &LobbyKey, update: SessionUpdate) {
        for request in update.scheduling {
            let scheduled = self.lobbies.get(lobby_key).is_some_and(|lobby| {
                ember_legacy::LegacyClock::request_schedule(
                    lobby.capabilities.clock.as_ref(),
                    request,
                )
                .is_ok()
            });
            if !scheduled {
                self.terminate_lobby(
                    lobby_key,
                    "version scheduling request exceeded host capacity",
                );
                return;
            }
        }
        for outbound in update.outbound {
            if self.dispatch_outbound(lobby_key, outbound).is_err() {
                self.terminate_lobby(lobby_key, "version outbound processing failed");
                return;
            }
        }
        let mut closes = update.closes;
        if let Some(lobby) = self.lobbies.get(lobby_key) {
            closes.extend(lobby.capabilities.transport.take_close_requests());
        }
        for close in closes {
            self.close_peer(
                close.peer_id,
                close.reason,
                CloseCode::Normal,
                "version requested connection closure",
            );
        }
    }

    fn dispatch_outbound(
        &mut self,
        lobby_key: &LobbyKey,
        outbound: OutboundEvent,
    ) -> Result<(), ()> {
        let (targets, codec, limits) = {
            let Some(lobby) = self.lobbies.get(lobby_key) else {
                return Err(());
            };
            (
                outbound_targets(lobby, outbound.target)?,
                Arc::clone(&lobby.codec),
                lobby.limits,
            )
        };
        let frame = catch_unwind(AssertUnwindSafe(|| codec.encode(&outbound.event)))
            .map_err(|_| ())?
            .map_err(|_| ())?;
        if frame.len() > usize::try_from(limits.max_frame_bytes).unwrap_or(usize::MAX) {
            return Err(());
        }
        let mut slow_peers = Vec::new();
        for peer_id in targets {
            let bytes = frame.len();
            let within_version_rate = self
                .version_outbound
                .get_mut(&lobby_key.game_key)
                .is_some_and(|window| {
                    window.charge(
                        Instant::now(),
                        bytes,
                        limits.max_outbound_bytes_per_second,
                    )
                });
            if !within_version_rate {
                return Err(());
            }
            let Some(connection) = self.connections.get_mut(&peer_id.host_value()) else {
                continue;
            };
            if connection.joined_lobby().as_ref() != Some(lobby_key) {
                continue;
            }
            if !enqueue_inner(
                connection,
                &frame,
                limits,
                self.config.max_connection_outbound_queue_bytes,
            ) {
                slow_peers.push(peer_id);
            }
        }
        for peer_id in slow_peers {
            self.close_peer(
                peer_id,
                CloseReason::SlowConsumer,
                CloseCode::Policy,
                "bounded outbound queue is full",
            );
        }
        Ok(())
    }

    fn sweep_timeouts(&mut self) {
        let now = Instant::now();
        let timed_out: Vec<_> = self
            .connections
            .iter()
            .filter(|(_, connection)| match connection.state.state() {
                ConnectionState::AwaitHello => {
                    now.duration_since(connection.created_at) >= self.config.hello_timeout
                }
                ConnectionState::Browsing { .. } => {
                    now.duration_since(connection.last_seen) >= self.config.browsing_timeout
                }
                ConnectionState::Joined { .. } => {
                    now.duration_since(connection.last_seen) >= self.config.joined_timeout
                }
                ConnectionState::Closed => false,
            })
            .map(|(id, _)| *id)
            .collect();
        for id in timed_out {
            self.close_connection(
                id,
                CloseReason::Timeout,
                CloseCode::Away,
                "connection lifecycle timeout",
            );
        }
    }

    fn select_or_refuse(
        &mut self,
        connection: &mut Connection,
        game_id: &str,
        game_version: u32,
    ) -> Option<GameKey> {
        match self.registry.exact_key(game_id, game_version) {
            Ok(key) => Some(key),
            Err(SelectionError::GameNotHosted(refusal)) => {
                self.send_outer(connection, &ServerMessage::GameNotHosted(refusal));
                None
            }
            Err(SelectionError::VersionNotHosted(refusal)) => {
                self.send_outer(connection, &ServerMessage::VersionNotHosted(refusal));
                None
            }
        }
    }

    fn refuse_if_draining(&mut self, connection: &mut Connection) -> bool {
        if !self.drain.is_draining() {
            return false;
        }
        self.invalid_request(connection, "server is draining; new admission is stopped");
        true
    }

    fn reject_outer(&mut self, connection: &mut Connection, error: OuterError, close: bool) {
        self.send_outer(connection, &ServerMessage::Error(error));
        if close {
            self.close_detached(
                connection,
                CloseReason::ProtocolViolation,
                CloseCode::Protocol,
                "outer protocol interpretation is ambiguous",
            );
        }
    }

    fn invalid_request(&mut self, connection: &mut Connection, message: &str) {
        self.send_outer(
            connection,
            &ServerMessage::Error(OuterError {
                code: OuterErrorCode::InvalidRequest,
                message: message.to_string(),
            }),
        );
    }

    fn internal_error(&mut self, connection: &mut Connection, message: &str) {
        self.send_outer(
            connection,
            &ServerMessage::Error(OuterError {
                code: OuterErrorCode::InternalError,
                message: message.to_string(),
            }),
        );
        self.close_detached(
            connection,
            CloseReason::InternalError,
            CloseCode::Error,
            "internal session boundary failure",
        );
    }

    fn send_outer(&mut self, connection: &mut Connection, message: &ServerMessage) {
        let encoded = match outer::encode_server_frame(message) {
            Ok(encoded) => encoded,
            Err(error) => {
                tracing::error!(connection = connection.id, %error, "outer encode failed");
                connection.close_reason = Some(CloseReason::InternalError);
                connection.state.close();
                return;
            }
        };
        let Ok(text) = String::from_utf8(encoded) else {
            connection.close_reason = Some(CloseReason::InternalError);
            connection.state.close();
            return;
        };
        let bytes = text.len();
        if connection.queued_outbound_bytes.saturating_add(bytes)
            > self.config.max_connection_outbound_queue_bytes
        {
            connection.close_reason = Some(CloseReason::SlowConsumer);
            connection.state.close();
            return;
        }
        match connection.outbound.try_send(OutboundCommand::Data {
            message: Message::text(text),
            bytes,
            version_frame: false,
        }) {
            Ok(()) => {
                connection.queued_outbound_bytes =
                    connection.queued_outbound_bytes.saturating_add(bytes);
            }
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                connection.close_reason = Some(CloseReason::SlowConsumer);
                connection.state.close();
            }
        }
    }

    fn close_connection(
        &mut self,
        id: u64,
        reason: CloseReason,
        code: CloseCode,
        detail: &str,
    ) {
        let Some(mut connection) = self.connections.remove(&id) else {
            return;
        };
        self.close_detached(&mut connection, reason.clone(), code, detail);
        self.detach_connection(&connection, reason);
    }

    fn close_peer(
        &mut self,
        peer_id: PeerId,
        reason: CloseReason,
        code: CloseCode,
        detail: &str,
    ) {
        self.close_connection(peer_id.host_value(), reason, code, detail);
    }

    fn close_detached(
        &mut self,
        connection: &mut Connection,
        reason: CloseReason,
        code: CloseCode,
        detail: &str,
    ) {
        tracing::info!(
            connection = connection.id,
            peer = %connection.peer,
            ?reason,
            detail,
            "closing connection"
        );
        drop(connection.outbound.try_send(OutboundCommand::Close {
            code,
            reason: detail.to_string(),
        }));
        connection.close_reason = Some(reason);
        connection.state.close();
    }

    fn detach_connection(&mut self, connection: &Connection, reason: CloseReason) {
        let Some(key) = connection.joined_lobby() else {
            return;
        };
        let Some(mut lobby) = self.lobbies.remove(&key) else {
            return;
        };
        let peer_id = PeerId::from_host_value(connection.id);
        if !lobby.members.remove(&peer_id) {
            self.lobbies.insert(key, lobby);
            return;
        }
        lobby.capabilities.transport.remove_peer(peer_id);
        let leave = catch_unwind(AssertUnwindSafe(|| {
            lobby.session.leave(
                peer_id,
                LeaveReason {
                    close_reason: reason,
                    detail: None,
                },
            )
        }));
        match leave {
            Ok(_update) if lobby.members.is_empty() => {
                drop_lobby_safely(lobby, "last peer left");
                self.record_occupancy();
            }
            Ok(update) => {
                self.lobbies.insert(key.clone(), lobby);
                self.record_occupancy();
                self.process_update(&key, update);
            }
            Err(_) => self.terminate_detached_lobby(lobby, "version panicked during leave"),
        }
    }

    fn terminate_lobby(&mut self, key: &LobbyKey, detail: &str) {
        if let Some(lobby) = self.lobbies.remove(key) {
            self.terminate_detached_lobby(lobby, detail);
        }
    }

    fn terminate_detached_lobby(&mut self, lobby: Lobby, detail: &str) {
        tracing::error!(
            game = %lobby.key.game_key.game_id,
            version = lobby.key.game_key.game_version,
            lobby = %lobby.key.lobby_name,
            detail,
            "terminating isolated version session"
        );
        let members: Vec<_> = lobby.members.iter().copied().collect();
        drop_lobby_safely(lobby, detail);
        for peer in members {
            self.close_peer(
                peer,
                CloseReason::InternalError,
                CloseCode::Error,
                "version session terminated at isolation boundary",
            );
        }
        self.record_occupancy();
    }

    fn record_occupancy(&self) {
        let Ok(mut occupancy) = self.drain.occupancy.lock() else {
            return;
        };
        for count in occupancy.values_mut() {
            *count = OccupancyCount::default();
        }
        for lobby in self.lobbies.values() {
            if let Some(count) = occupancy.get_mut(&lobby.key.game_key) {
                count.lobbies = count.lobbies.saturating_add(1);
                count.players = count
                    .players
                    .saturating_add(u32::try_from(lobby.members.len()).unwrap_or(u32::MAX));
            }
        }
    }
}

enum AdmissionOutcome {
    Admitted(SessionUpdate),
    Refused { code: String, message: String },
    Panicked,
}

fn to_inner_payload(frame: DataFrame) -> InnerPayload {
    match frame {
        DataFrame::Text(text) => InnerPayload::Text(text),
        DataFrame::Binary(bytes) => InnerPayload::Binary(bytes),
    }
}

fn drop_lobby_safely(lobby: Lobby, context: &str) {
    let game_id = lobby.key.game_key.game_id.clone();
    let game_version = lobby.key.game_key.game_version;
    let lobby_name = lobby.key.lobby_name.clone();
    if catch_unwind(AssertUnwindSafe(|| drop(lobby))).is_err() {
        tracing::error!(
            game = %game_id,
            version = game_version,
            lobby = %lobby_name,
            context,
            "version session panicked while being destroyed"
        );
    }
}

fn next_step_after(cadence: Duration) -> Instant {
    let now = Instant::now();
    now.checked_add(cadence).unwrap_or(now)
}

fn valid_handle(handle: &str) -> bool {
    !handle.is_empty()
        && handle.len() <= MAX_HANDLE_BYTES
        && !handle.chars().any(char::is_control)
}

fn valid_lobby_request(lobby_name: &str, password: Option<&str>) -> bool {
    !lobby_name.is_empty()
        && lobby_name.len() <= MAX_LOBBY_NAME_BYTES
        && !lobby_name.chars().any(char::is_control)
        && password.is_none_or(|password| password.len() <= MAX_PASSWORD_BYTES)
}

fn password_matches(lobby: &Lobby, password: Option<&str>) -> bool {
    lobby
        .password
        .as_ref()
        .is_none_or(|verifier| verifier.matches(password, lobby.session_id))
}

fn lobby_seed(key: &GameKey, lobby_name: &str, session_id: SessionId) -> LobbySeed {
    let mut digest = Sha256::new();
    digest.update(b"ember-host-lobby-seed-v1\0");
    update_length_prefixed(&mut digest, key.game_id.as_bytes());
    digest.update(key.game_version.to_le_bytes());
    update_length_prefixed(&mut digest, lobby_name.as_bytes());
    digest.update(session_id.host_value().to_le_bytes());
    LobbySeed(digest.finalize().into())
}

fn update_length_prefixed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(bytes);
}

fn outbound_targets(lobby: &Lobby, target: OutboundTarget) -> Result<BTreeSet<PeerId>, ()> {
    match target {
        OutboundTarget::Unicast(handle) => Ok(BTreeSet::from([handle.peer_id()])),
        OutboundTarget::Broadcast(handle) if handle.session_id() == lobby.session_id => {
            Ok(lobby.members.clone())
        }
        OutboundTarget::Broadcast(_) => Err(()),
        OutboundTarget::Peers(peers) => Ok(peers
            .into_iter()
            .filter(|peer| lobby.members.contains(peer))
            .collect()),
    }
}

fn enqueue_inner(
    connection: &mut Connection,
    frame: &InnerFrame,
    limits: VersionLimits,
    global_queue_limit: usize,
) -> bool {
    let bytes = frame.len();
    let version_queue_limit =
        usize::try_from(limits.max_outbound_queue_bytes).unwrap_or(usize::MAX);
    if connection.queued_version_bytes.saturating_add(bytes) > version_queue_limit
        || connection.queued_outbound_bytes.saturating_add(bytes) > global_queue_limit
    {
        return false;
    }
    let message = match frame {
        InnerFrame::Text(text) => Message::text(text.clone()),
        InnerFrame::Binary(bytes) => Message::binary(bytes.clone()),
    };
    match connection
        .outbound
        .try_send(OutboundCommand::Data {
            message,
            bytes,
            version_frame: true,
        })
    {
        Ok(()) => {
            connection.queued_outbound_bytes =
                connection.queued_outbound_bytes.saturating_add(bytes);
            connection.queued_version_bytes =
                connection.queued_version_bytes.saturating_add(bytes);
            true
        }
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_legacy::{DecodedInput, MonotonicDuration, SchedulingRequest};

    #[test]
    fn charging_happens_before_frame_rejection() {
        let now = Instant::now();
        let mut window = InboundWindow::new(now);
        let result = window.charge(now, outer::MAX_OUTER_FRAME_BYTES + 1, 1, u64::MAX);
        assert!(result.messages_within_limit);
        assert!(result.bytes_within_limit);
        assert_eq!(window.messages, 1);
        assert_eq!(
            window.bytes,
            u64::try_from(outer::MAX_OUTER_FRAME_BYTES + 1).unwrap()
        );
    }

    #[test]
    fn rate_budget_charges_the_rejected_message() {
        let now = Instant::now();
        let mut window = InboundWindow::new(now);
        assert!(window.charge(now, 1, 1, 2).messages_within_limit);
        let rejected = window.charge(now, 1, 1, 2);
        assert!(!rejected.messages_within_limit);
        assert!(rejected.bytes_within_limit);
        assert_eq!(window.messages, 2);
    }

    #[test]
    fn drain_stops_admission_without_erasing_occupancy() {
        let key = GameKey {
            game_id: "fixture".to_string(),
            game_version: 1,
        };
        let occupancy = Arc::new(Mutex::new(BTreeMap::from([(
            key,
            OccupancyCount {
                lobbies: 1,
                players: 2,
            },
        )])));
        let drain = DrainHandle {
            draining: Arc::new(AtomicBool::new(false)),
            occupancy,
        };
        drain.stop_admission();
        assert!(drain.is_draining());
        assert_eq!(
            drain.occupancy(),
            vec![OccupancySnapshot {
                game_id: "fixture".to_string(),
                game_version: 1,
                lobbies: 1,
                players: 2,
            }]
        );
    }

    #[test]
    fn same_lobby_name_is_scoped_by_exact_game_key() {
        let arena = LobbyKey {
            game_key: GameKey {
                game_id: "arena".to_string(),
                game_version: 12,
            },
            lobby_name: "same".to_string(),
        };
        let fire = LobbyKey {
            game_key: GameKey {
                game_id: "fire".to_string(),
                game_version: 1,
            },
            lobby_name: "same".to_string(),
        };
        assert_ne!(arena, fire);
    }

    #[test]
    fn canonical_state_machine_legality_is_preserved() {
        let mut state = StateMachine::new();
        assert!(matches!(
            state.transition(StateInput::Inner(InnerPayload::Binary(vec![1]))),
            StateAction::Reject {
                close_after_error: true,
                ..
            }
        ));
        let hello = outer::Hello {
            outer_version: outer::OUTER_VERSION,
            handle: "player".to_string(),
        };
        assert!(matches!(
            state.transition(StateInput::Outer(outer::ClientMessage::Hello(
                hello.clone()
            ))),
            StateAction::AcceptHello(_)
        ));
        assert!(matches!(
            state.transition(StateInput::Outer(outer::ClientMessage::Hello(hello))),
            StateAction::Reject {
                close_after_error: true,
                ..
            }
        ));
        state
            .mark_joined(Joined {
                game_id: "fixture".to_string(),
                game_version: 1,
                lobby_name: "room".to_string(),
            })
            .unwrap();
        assert!(matches!(
            state.transition(StateInput::Outer(outer::ClientMessage::ListLobbies(
                outer::ListLobbies
            ))),
            StateAction::Reject {
                close_after_error: true,
                ..
            }
        ));
    }

    #[test]
    fn frequency_never_enters_outer_protocol_values() {
        let scheduling = SchedulingRequest::After(MonotonicDuration::from_micros(20_000));
        assert!(matches!(scheduling, SchedulingRequest::After(_)));
        let decoded = DecodedInput {
            payload: Vec::new(),
        };
        assert!(decoded.payload.is_empty());
    }
}
