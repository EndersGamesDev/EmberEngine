//! Narrow capabilities and version-facing contracts for hosted legacy games.
//!
//! This crate is an in-tree compatibility surface, not a stable external API.
//! Hosted versions depend on these neutral values and object-safe traits rather
//! than on the server, wire transport, renderer, or operating-system runtime.

#![deny(missing_docs)]
// Contract names remain explicit when imported beside current runtime types.
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// A permanent game identity paired with one frozen wire and rules version.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct GameKey {
    /// Permanent lowercase ASCII game slug.
    pub game_id: String,
    /// Monotonically allocated network and gameplay contract version.
    pub game_version: u32,
}

/// Host-enforced resource limits for one hosted version.
// A common `max_` prefix makes every charged boundary unambiguous at call sites.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionLimits {
    /// Maximum simultaneously active lobbies for this version.
    pub max_lobbies: u32,
    /// Maximum admitted players in one lobby.
    pub max_players_per_lobby: u16,
    /// Maximum bytes charged for one inner frame before decoding.
    pub max_frame_bytes: u32,
    /// Maximum messages charged to one peer during one second.
    pub max_messages_per_second: u32,
    /// Maximum queued outbound bytes for one peer.
    pub max_outbound_queue_bytes: u32,
    /// Maximum outbound bytes charged to one version during one second.
    pub max_outbound_bytes_per_second: u64,
    /// Maximum host wall-time budget for one deterministic session step.
    pub max_step_duration: MonotonicDuration,
}

/// Why a concrete version-limits value is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersionLimitsError {
    /// At least one capacity or byte limit is zero.
    ZeroCapacity,
    /// The permitted step duration is zero.
    ZeroStepDuration,
}

impl VersionLimits {
    /// Validates that every enforced limit has a usable nonzero value.
    ///
    /// # Errors
    ///
    /// Returns the first invalid category found in the limits.
    pub const fn validate(self) -> Result<(), VersionLimitsError> {
        if self.max_lobbies == 0
            || self.max_players_per_lobby == 0
            || self.max_frame_bytes == 0
            || self.max_messages_per_second == 0
            || self.max_outbound_queue_bytes == 0
            || self.max_outbound_bytes_per_second == 0
        {
            return Err(VersionLimitsError::ZeroCapacity);
        }
        if self.max_step_duration.as_micros() == 0 {
            return Err(VersionLimitsError::ZeroStepDuration);
        }
        Ok(())
    }
}

/// A host-supplied monotonic instant measured in microseconds from an opaque epoch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MonotonicTimestamp(u64);

impl MonotonicTimestamp {
    /// Constructs a monotonic timestamp from host-epoch microseconds.
    #[must_use]
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// Returns the host-epoch microsecond value.
    #[must_use]
    pub const fn as_micros(self) -> u64 {
        self.0
    }

    /// Adds a duration, saturating at the largest representable instant.
    #[must_use]
    pub const fn saturating_add(self, duration: MonotonicDuration) -> Self {
        Self(self.0.saturating_add(duration.0))
    }
}

/// A nonnegative duration measured in microseconds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MonotonicDuration(u64);

impl MonotonicDuration {
    /// A duration containing no elapsed time.
    pub const ZERO: Self = Self(0);

    /// Constructs a duration from microseconds.
    #[must_use]
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// Returns the number of microseconds in the duration.
    #[must_use]
    pub const fn as_micros(self) -> u64 {
        self.0
    }
}

/// A frequency-free request for the host to schedule future session work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulingRequest {
    /// Schedule work at an absolute monotonic timestamp.
    At(MonotonicTimestamp),
    /// Schedule work after a duration relative to the host's current time.
    After(MonotonicDuration),
}

/// An opaque identifier for a scheduling request accepted by the host.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScheduleHandle(u64);

impl ScheduleHandle {
    /// Constructs a handle from a host-owned value.
    #[must_use]
    pub const fn from_host_value(value: u64) -> Self {
        Self(value)
    }

    /// Returns the host-owned value for adapter bookkeeping.
    #[must_use]
    pub const fn host_value(self) -> u64 {
        self.0
    }
}

/// A failure to register or cancel scheduled work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleError {
    /// The host has reached the version's scheduling capacity.
    CapacityReached,
    /// The supplied handle is not active for this capability instance.
    UnknownHandle,
    /// The host is shutting down and accepts no new scheduling work.
    ShuttingDown,
}

/// Object-safe access to host-owned monotonic time and scheduling.
pub trait LegacyClock: Send + Sync {
    /// Returns the host's current monotonic timestamp.
    fn now(&self) -> MonotonicTimestamp;

    /// Registers frequency-free future work.
    ///
    /// # Errors
    ///
    /// Returns an error when host capacity or lifecycle state rejects the request.
    fn request_schedule(
        &self,
        request: SchedulingRequest,
    ) -> Result<ScheduleHandle, ScheduleError>;

    /// Cancels previously accepted future work.
    ///
    /// # Errors
    ///
    /// Returns an error when the handle is unknown to this capability instance.
    fn cancel_schedule(&self, handle: ScheduleHandle) -> Result<(), ScheduleError>;
}

/// A deterministic lobby seed supplied as immutable creation data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LobbySeed(pub [u8; 32]);

/// A version-owned stable name separating one deterministic random stream.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RandomStreamKey(pub String);

/// The complete key for a deterministic, call-order-independent random draw.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RandomDrawKey {
    /// Hosted game and version whose rules assign meaning to the draw.
    pub game_key: GameKey,
    /// Immutable seed assigned when the lobby is created.
    pub lobby_seed: LobbySeed,
    /// Version-owned stable stream name.
    pub stream_key: RandomStreamKey,
    /// Explicit event index within the stable stream.
    pub event_index: u64,
}

/// Object-safe deterministic random draws without ambient mutable state.
pub trait LegacyRandom: Send + Sync {
    /// Derives one unsigned 64-bit draw from the complete draw key.
    fn draw_u64(&self, key: &RandomDrawKey) -> u64;

    /// Fills bytes deterministically from the complete draw key.
    fn fill_bytes(&self, key: &RandomDrawKey, output: &mut [u8]);
}

/// An opaque host-owned connection identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PeerId(u64);

impl PeerId {
    /// Constructs a peer identifier from a host-owned value.
    #[must_use]
    pub const fn from_host_value(value: u64) -> Self {
        Self(value)
    }

    /// Returns the host-owned value for adapter bookkeeping.
    #[must_use]
    pub const fn host_value(self) -> u64 {
        self.0
    }
}

/// An opaque host-owned lobby-session identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(u64);

impl SessionId {
    /// Constructs a session identifier from a host-owned value.
    #[must_use]
    pub const fn from_host_value(value: u64) -> Self {
        Self(value)
    }

    /// Returns the host-owned value for adapter bookkeeping.
    #[must_use]
    pub const fn host_value(self) -> u64 {
        self.0
    }
}

/// An opaque bounded target handle for one peer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UnicastHandle(PeerId);

impl UnicastHandle {
    /// Returns the peer selected by this host-issued handle.
    #[must_use]
    pub const fn peer_id(self) -> PeerId {
        self.0
    }
}

/// An opaque bounded target handle for the admitted peers in one session.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BroadcastHandle(SessionId);

impl BroadcastHandle {
    /// Returns the session selected by this host-issued handle.
    #[must_use]
    pub const fn session_id(self) -> SessionId {
        self.0
    }
}

/// Why a peer or session is being closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// The peer explicitly requested a clean departure.
    Requested,
    /// The peer's transport disconnected.
    Disconnected,
    /// The peer violated the selected wire protocol.
    ProtocolViolation,
    /// The peer exceeded its charged frame budget.
    FrameTooLarge,
    /// The peer did not drain its bounded outbound queue.
    SlowConsumer,
    /// The peer exceeded a liveness timeout.
    Timeout,
    /// Version-owned admission logic refused the peer.
    AdmissionRefused,
    /// The host is draining for shutdown.
    ServerShutdown,
    /// An internal boundary failed without exposing implementation details.
    InternalError,
}

/// Immutable host-owned facts supplied when a peer joins a session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionMetadata {
    /// Opaque identity assigned before version-owned admission runs.
    pub peer_id: PeerId,
    /// Sanitized outer handle chosen by the peer.
    pub handle: String,
    /// Monotonic time at which host admission began.
    pub admitted_at: MonotonicTimestamp,
    /// Closed, host-defined metadata copied without transport objects.
    pub attributes: BTreeMap<String, String>,
}

/// A game-neutral metric value labelled by the host with game and version.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricObservation {
    /// Stable version-owned metric name.
    pub name: String,
    /// Numeric observation recorded by the outer metrics implementation.
    pub value: f64,
}

/// A failure to acquire a bounded target handle or request closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    /// The peer is no longer admitted.
    UnknownPeer,
    /// The session is no longer active.
    UnknownSession,
    /// The relevant bounded queue has no remaining capacity.
    QueueFull,
    /// The host is draining and accepts no new transport work.
    ShuttingDown,
}

/// Object-safe access to host-owned bounded targets, closure, and metrics.
pub trait LegacyTransport: Send + Sync {
    /// Acquires a bounded target handle for one admitted peer.
    ///
    /// # Errors
    ///
    /// Returns an error if the peer is unknown or its queue cannot accept work.
    fn unicast(&self, peer_id: PeerId) -> Result<UnicastHandle, TransportError>;

    /// Acquires a bounded target handle for one active session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is unknown or its queue cannot accept work.
    fn broadcast(&self, session_id: SessionId) -> Result<BroadcastHandle, TransportError>;

    /// Requests that the host close one peer with a structured reason.
    ///
    /// # Errors
    ///
    /// Returns an error if the peer is already absent.
    fn close_peer(&self, peer_id: PeerId, reason: CloseReason) -> Result<(), TransportError>;

    /// Records a version-owned observation through outer-owned metrics.
    fn record_metric(&self, observation: MetricObservation);
}

/// A stable logical key used to locate or register a client asset.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetKey(pub String);

/// Neutral decoded triangle-mesh data without renderer-specific objects.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    /// Vertex positions in object-local coordinates.
    pub positions: Vec<[f32; 3]>,
    /// Optional vertex normals parallel to `positions`.
    pub normals: Vec<[f32; 3]>,
    /// Optional texture coordinates parallel to `positions`.
    pub texture_coordinates: Vec<[f32; 2]>,
    /// Triangle indices into `positions`.
    pub indices: Vec<u32>,
}

/// Channel layout for neutral decoded texture pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureFormat {
    /// Eight-bit red, green, blue, and alpha channels.
    Rgba8,
    /// Eight-bit red, green, and blue channels.
    Rgb8,
    /// One eight-bit luminance channel.
    Luma8,
}

/// Neutral decoded texture data without renderer-specific objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureData {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel channel layout.
    pub format: TextureFormat,
    /// Tightly packed row-major pixels in the declared format.
    pub pixels: Vec<u8>,
}

/// An opaque adapter-owned registered mesh handle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MeshHandle(u64);

impl MeshHandle {
    /// Constructs a mesh handle from an adapter-owned value.
    #[must_use]
    pub const fn from_adapter_value(value: u64) -> Self {
        Self(value)
    }

    /// Returns the adapter-owned value for adapter bookkeeping.
    #[must_use]
    pub const fn adapter_value(self) -> u64 {
        self.0
    }
}

/// An opaque adapter-owned registered texture handle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextureHandle(u64);

impl TextureHandle {
    /// Constructs a texture handle from an adapter-owned value.
    #[must_use]
    pub const fn from_adapter_value(value: u64) -> Self {
        Self(value)
    }

    /// Returns the adapter-owned value for adapter bookkeeping.
    #[must_use]
    pub const fn adapter_value(self) -> u64 {
        self.0
    }
}

/// Severity assigned to a client-asset diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticLevel {
    /// Information useful while diagnosing asset selection.
    Info,
    /// Recoverable asset degradation.
    Warning,
    /// An asset failure that prevents the requested operation.
    Error,
}

/// A neutral diagnostic emitted by legacy client-asset code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetDiagnostic {
    /// Diagnostic severity.
    pub level: DiagnosticLevel,
    /// Stable logical key associated with the diagnostic, when known.
    pub asset_key: Option<AssetKey>,
    /// Human-readable detail for current diagnostics.
    pub message: String,
}

/// A failure to find, decode, or register a neutral client asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetError {
    /// The stable logical key has no configured asset.
    NotFound(AssetKey),
    /// The configured asset cannot be decoded into neutral data.
    DecodeFailed(String),
    /// The current adapter cannot register the neutral data.
    RegistrationFailed(String),
}

/// Object-safe access to logical client assets and opaque registration handles.
pub trait LegacyAssets: Send + Sync {
    /// Loads neutral mesh data by stable logical key.
    ///
    /// # Errors
    ///
    /// Returns an error when lookup or neutral decoding fails.
    fn load_mesh(&self, key: &AssetKey) -> Result<MeshData, AssetError>;

    /// Loads neutral texture data by stable logical key.
    ///
    /// # Errors
    ///
    /// Returns an error when lookup or neutral decoding fails.
    fn load_texture(&self, key: &AssetKey) -> Result<TextureData, AssetError>;

    /// Registers neutral mesh data with the current client adapter.
    ///
    /// # Errors
    ///
    /// Returns an error when current renderer plumbing rejects the data.
    fn register_mesh(&self, key: &AssetKey, mesh: MeshData) -> Result<MeshHandle, AssetError>;

    /// Registers neutral texture data with the current client adapter.
    ///
    /// # Errors
    ///
    /// Returns an error when current renderer plumbing rejects the data.
    fn register_texture(
        &self,
        key: &AssetKey,
        texture: TextureData,
    ) -> Result<TextureHandle, AssetError>;

    /// Emits a diagnostic through current client plumbing.
    fn diagnose(&self, diagnostic: AssetDiagnostic);
}

/// Cloneable capability objects available to a legacy session factory.
#[derive(Clone)]
pub struct LegacyCapabilities {
    /// Host-owned monotonic time and scheduling.
    pub clock: Arc<dyn LegacyClock>,
    /// Deterministic keyed random derivation.
    pub random: Arc<dyn LegacyRandom>,
    /// Host-owned bounded targets, closure, and metrics.
    pub transport: Arc<dyn LegacyTransport>,
    /// Optional client-only neutral asset adapter.
    pub assets: Option<Arc<dyn LegacyAssets>>,
}

/// One exact WebSocket data-frame payload at the inner protocol boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InnerFrame {
    /// An exact UTF-8 WebSocket text payload.
    Text(String),
    /// An exact WebSocket binary payload.
    Binary(Vec<u8>),
}

impl InnerFrame {
    /// Returns the charged payload size in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Binary(bytes) => bytes.len(),
        }
    }

    /// Returns whether the exact payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Version-private canonical bytes produced by an inner decoder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInput {
    /// Bytes whose meaning is private to the matching version session.
    pub payload: Vec<u8>,
}

/// Version-private canonical bytes produced by a session for inner encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedEvent {
    /// Bytes whose meaning is private to the matching version codec.
    pub payload: Vec<u8>,
}

/// A stable classification for inner frame decode or encode failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InnerCodecError {
    /// The frame kind is not part of this version's frozen protocol.
    WrongFrameKind,
    /// The frame exceeds version-owned structural constraints after host charging.
    InvalidFrame(String),
    /// The frame cannot be decoded as the selected frozen message schema.
    DecodeFailed(String),
    /// A version-produced event cannot be represented by the frozen schema.
    EncodeFailed(String),
}

/// Object-safe exact inner frame decoder and encoder for one hosted version.
pub trait InnerCodec: Send + Sync {
    /// Decodes one exact transport payload into version-private canonical bytes.
    ///
    /// # Errors
    ///
    /// Returns a stable codec failure without changing or guessing the protocol.
    fn decode(&self, frame: &InnerFrame) -> Result<DecodedInput, InnerCodecError>;

    /// Encodes version-private canonical bytes as one exact transport payload.
    ///
    /// # Errors
    ///
    /// Returns a stable codec failure without emitting a partial frame.
    fn encode(&self, event: &EncodedEvent) -> Result<InnerFrame, InnerCodecError>;
}

/// One timestamped decoded input charged and ordered by the host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionInput {
    /// Admitted peer that supplied the frame.
    pub peer_id: PeerId,
    /// Host monotonic receipt timestamp.
    pub received_at: MonotonicTimestamp,
    /// Version-private canonical payload from the matching codec.
    pub input: DecodedInput,
}

/// A game-neutral target set for a version-produced inner event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutboundTarget {
    /// One host-issued bounded peer target.
    Unicast(UnicastHandle),
    /// All currently admitted peers selected by a host-issued session target.
    Broadcast(BroadcastHandle),
    /// An explicit stable set of admitted peers.
    Peers(Vec<PeerId>),
}

/// One version-produced event awaiting exact inner encoding and bounded enqueue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboundEvent {
    /// Host-owned target set.
    pub target: OutboundTarget,
    /// Version-private canonical event for the matching codec.
    pub event: EncodedEvent,
}

/// One version-requested peer closure returned to the host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseRequest {
    /// Peer to close after the current session operation.
    pub peer_id: PeerId,
    /// Structured close reason.
    pub reason: CloseReason,
}

/// All deterministic effects returned by one session operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionUpdate {
    /// Version-produced inner events in deterministic output order.
    pub outbound: Vec<OutboundEvent>,
    /// Frequency-free future-work requests in deterministic output order.
    pub scheduling: Vec<SchedulingRequest>,
    /// Peer closures in deterministic output order.
    pub closes: Vec<CloseRequest>,
}

/// A small version-owned status projection safe for outer lobby listings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LobbyStatus {
    /// Stable compact status code such as `waiting` or `racing`.
    pub code: String,
    /// Optional short display detail containing no inner state.
    pub detail: Option<String>,
}

/// A version-owned refusal produced after outer admission checks succeed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionRefusal {
    /// Stable version-owned refusal code.
    pub code: String,
    /// Human-readable refusal detail for the selected client contract.
    pub message: String,
}

/// Why a peer left an already-created session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaveReason {
    /// Structured outer close category.
    pub close_reason: CloseReason,
    /// Optional version-neutral diagnostic supplied by the host.
    pub detail: Option<String>,
}

/// Object-safe deterministic simulation session for one hosted lobby.
pub trait GameSession: Send {
    /// Applies an ordered input batch at one host-supplied monotonic timestamp.
    fn step(
        &mut self,
        timestamp: MonotonicTimestamp,
        inputs: Vec<SessionInput>,
    ) -> SessionUpdate;

    /// Admits a peer after outer key, lobby, password, and capacity checks.
    ///
    /// # Errors
    ///
    /// Returns a version-owned refusal while leaving the connection in browsing.
    fn join(&mut self, admission: AdmissionMetadata) -> Result<SessionUpdate, AdmissionRefusal>;

    /// Removes a peer and returns deterministic resulting events.
    fn leave(&mut self, peer_id: PeerId, reason: LeaveReason) -> SessionUpdate;

    /// Projects current version state into the small outer lobby status.
    fn lobby_status(&self) -> LobbyStatus;
}

/// Immutable data supplied while constructing one authoritative lobby session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionCreationData {
    /// Exact hosted game and version selected before construction.
    pub game_key: GameKey,
    /// Opaque host-owned session identity.
    pub session_id: SessionId,
    /// Lobby name scoped by the exact game key.
    pub lobby_name: String,
    /// Immutable deterministic lobby seed.
    pub lobby_seed: LobbySeed,
    /// Host monotonic creation timestamp.
    pub created_at: MonotonicTimestamp,
    /// Immutable version-owned configured rules in version-private canonical bytes.
    pub configured_rules: Vec<u8>,
}

/// A stable classification for failure to construct a version session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactoryError {
    /// Immutable configured rules are invalid for the selected version.
    InvalidConfiguration(String),
    /// Required current plumbing is unavailable.
    CapabilityUnavailable(String),
    /// Construction failed without exposing current implementation types.
    ConstructionFailed(String),
}

/// Object-safe factory for a heterogeneous build-time registry entry.
pub trait GameFactory: Send + Sync {
    /// Constructs one authoritative session from capabilities and immutable data.
    ///
    /// # Errors
    ///
    /// Returns a structured construction failure before a lobby becomes visible.
    fn create(
        &self,
        capabilities: &LegacyCapabilities,
        creation: &SessionCreationData,
    ) -> Result<Box<dyn GameSession>, FactoryError>;
}

/// Parsed product manifest containing the exact hosted set.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostedManifest {
    /// Explicit hosted entries; directory discovery never adds entries.
    pub games: Vec<HostedGame>,
}

/// One hosted game-version entry in `games/hosted.toml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostedGame {
    /// Permanent lowercase ASCII game slug.
    pub game_id: String,
    /// Monotonically allocated frozen contract version.
    pub game_version: u32,
    /// Unique workspace package compiled for this hosted entry.
    #[serde(default)]
    pub package: String,
    /// Whether current clients select this entry as the game's latest version.
    #[serde(default)]
    pub latest: bool,
    /// Named host limits profile measured outside version code.
    #[serde(default)]
    pub limits_profile: String,
    /// Identifier of this entry's immutable `hosted-contract` fixture suite.
    #[serde(default)]
    pub fixture_suite: String,
    /// Closed legacy query-selector value for an already deployed client.
    #[serde(default)]
    pub legacy_game: Option<String>,
}

impl HostedGame {
    /// Returns this manifest entry's exact registry key.
    #[must_use]
    pub fn game_key(&self) -> GameKey {
        GameKey {
            game_id: self.game_id.clone(),
            game_version: self.game_version,
        }
    }
}

/// A TOML syntax or shape error encountered before semantic validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestParseError {
    /// Human-readable parser detail without runtime host state.
    pub message: String,
}

impl fmt::Display for ManifestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ManifestParseError {}

/// One pure semantic validation failure in a parsed hosted manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestValidationError {
    /// The same exact game key appears more than once.
    DuplicateGameKey(GameKey),
    /// More than one entry for a game is marked latest.
    MultipleLatest {
        /// Game whose latest selector is ambiguous.
        game_id: String,
    },
    /// A game entry has no workspace package name.
    MissingPackage(GameKey),
    /// A game entry has no limits-profile identifier.
    MissingLimitsProfile(GameKey),
    /// A game entry has no fixture-suite identifier.
    MissingFixtureSuite(GameKey),
    /// A game identifier is not a nonempty lowercase ASCII slug.
    InvalidGameId {
        /// Invalid identifier as parsed.
        game_id: String,
    },
    /// A legacy selector is not a valid closed lowercase ASCII slug.
    InvalidLegacySelector {
        /// Exact game key declaring the selector.
        game_key: GameKey,
        /// Invalid selector as parsed.
        selector: String,
    },
    /// More than one entry claims the same legacy selector.
    DuplicateLegacySelector {
        /// Repeated selector value.
        selector: String,
    },
}

/// Parses hosted-manifest TOML without consulting the filesystem or runtime.
///
/// # Errors
///
/// Returns a parser error when TOML syntax or the declared shape is invalid.
pub fn parse_hosted_manifest(source: &str) -> Result<HostedManifest, ManifestParseError> {
    toml::from_str(source).map_err(|error| ManifestParseError {
        message: error.to_string(),
    })
}

/// Validates one parsed hosted manifest without runtime enforcement.
///
/// # Errors
///
/// Returns every semantic manifest error in deterministic entry order.
pub fn validate_hosted_manifest(
    manifest: &HostedManifest,
) -> Result<(), Vec<ManifestValidationError>> {
    let errors: Vec<_> = manifest
        .games
        .iter()
        .filter(|game| !is_valid_game_id(&game.game_id))
        .map(|game| ManifestValidationError::InvalidGameId {
            game_id: game.game_id.clone(),
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Returns whether a game identifier is a nonempty lowercase ASCII slug.
#[must_use]
pub fn is_valid_game_id(game_id: &str) -> bool {
    !game_id.is_empty()
        && game_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Returns whether a legacy selector is a valid closed lowercase ASCII slug.
#[must_use]
pub fn is_valid_legacy_selector(selector: &str) -> bool {
    is_valid_game_id(selector)
}
