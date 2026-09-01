# One-server interface freeze

This document is the verbatim shared interface frozen by the first `interface freeze` commit on lane `found`; implementations may evolve, but post-freeze signature changes are additive only and are recorded in the changelog.

## Dependency law

Version crates may depend on `ember-legacy` and behavior-neutral pure libraries, but never on `ember-net`, `ember-server`, client shells, renderer internals, or another version of themselves; the host alone connects `ember-net` frames to `ember-legacy` codecs and sessions.

`ember-net::outer` owns only canonical bootstrap, browsing, selection, and exact post-join payload handoff; existing cube-protocol exports remain at their current paths during this wave.

## Object safety

`LegacyClock`, `LegacyRandom`, `LegacyTransport`, `LegacyAssets`, `InnerCodec`, `GameSession`, and `GameFactory` are object-safe by design: their methods have no type parameters, do not return `Self`, and use owned neutral values or borrowed slices, so the closed build-time registry can store heterogeneous `Arc<dyn InnerCodec>`, `Arc<dyn GameFactory>`, capability trait objects, and returned `Box<dyn GameSession>` values without monomorphizing the host per game.

`DecodedInput` and `EncodedEvent` are version-private canonical byte envelopes opaque to the host; they avoid a generic protocol-message parameter while keeping decoding and encoding independently fixture-testable.

## `ember-legacy` frozen signatures

All declarations below are at the `ember_legacy` crate root.

### Identity and limits

```rust
pub struct GameKey {
    pub game_id: String,
    pub game_version: u32,
}

pub struct VersionLimits {
    pub max_lobbies: u32,
    pub max_players_per_lobby: u16,
    pub max_frame_bytes: u32,
    pub max_messages_per_second: u32,
    pub max_outbound_queue_bytes: u32,
    pub max_outbound_bytes_per_second: u64,
    pub max_step_duration: MonotonicDuration,
}

pub enum VersionLimitsError {
    ZeroCapacity,
    ZeroStepDuration,
}

impl VersionLimits {
    pub const fn validate(self) -> Result<(), VersionLimitsError>;
}
```

### Time capability

```rust
pub struct MonotonicTimestamp(u64);

impl MonotonicTimestamp {
    pub const fn from_micros(micros: u64) -> Self;
    pub const fn as_micros(self) -> u64;
    pub const fn saturating_add(self, duration: MonotonicDuration) -> Self;
}

pub struct MonotonicDuration(u64);

impl MonotonicDuration {
    pub const ZERO: Self = Self(0);
    pub const fn from_micros(micros: u64) -> Self;
    pub const fn as_micros(self) -> u64;
}

pub enum SchedulingRequest {
    At(MonotonicTimestamp),
    After(MonotonicDuration),
}

pub struct ScheduleHandle(u64);

impl ScheduleHandle {
    pub const fn from_host_value(value: u64) -> Self;
    pub const fn host_value(self) -> u64;
}

pub enum ScheduleError {
    CapacityReached,
    UnknownHandle,
    ShuttingDown,
}

pub trait LegacyClock: Send + Sync {
    fn now(&self) -> MonotonicTimestamp;
    fn request_schedule(
        &self,
        request: SchedulingRequest,
    ) -> Result<ScheduleHandle, ScheduleError>;
    fn cancel_schedule(&self, handle: ScheduleHandle) -> Result<(), ScheduleError>;
}
```

### Deterministic randomness capability

```rust
pub struct LobbySeed(pub [u8; 32]);

pub struct RandomStreamKey(pub String);

pub struct RandomDrawKey {
    pub game_key: GameKey,
    pub lobby_seed: LobbySeed,
    pub stream_key: RandomStreamKey,
    pub event_index: u64,
}

pub trait LegacyRandom: Send + Sync {
    fn draw_u64(&self, key: &RandomDrawKey) -> u64;
    fn fill_bytes(&self, key: &RandomDrawKey, output: &mut [u8]);
}
```

The trait key is frozen at this commit; the exact byte-level derivation is finalized after the freeze and, once recorded in this document, becomes frozen gameplay semantics.

### Session transport capability

```rust
pub struct PeerId(u64);

impl PeerId {
    pub const fn from_host_value(value: u64) -> Self;
    pub const fn host_value(self) -> u64;
}

pub struct SessionId(u64);

impl SessionId {
    pub const fn from_host_value(value: u64) -> Self;
    pub const fn host_value(self) -> u64;
}

pub struct UnicastHandle(PeerId);

impl UnicastHandle {
    pub const fn peer_id(self) -> PeerId;
}

pub struct BroadcastHandle(SessionId);

impl BroadcastHandle {
    pub const fn session_id(self) -> SessionId;
}

pub enum CloseReason {
    Requested,
    Disconnected,
    ProtocolViolation,
    FrameTooLarge,
    SlowConsumer,
    Timeout,
    AdmissionRefused,
    ServerShutdown,
    InternalError,
}

pub struct AdmissionMetadata {
    pub peer_id: PeerId,
    pub handle: String,
    pub admitted_at: MonotonicTimestamp,
    pub attributes: BTreeMap<String, String>,
}

pub struct MetricObservation {
    pub name: String,
    pub value: f64,
}

pub enum TransportError {
    UnknownPeer,
    UnknownSession,
    QueueFull,
    ShuttingDown,
}

pub trait LegacyTransport: Send + Sync {
    fn unicast(&self, peer_id: PeerId) -> Result<UnicastHandle, TransportError>;
    fn broadcast(&self, session_id: SessionId) -> Result<BroadcastHandle, TransportError>;
    fn close_peer(&self, peer_id: PeerId, reason: CloseReason) -> Result<(), TransportError>;
    fn record_metric(&self, observation: MetricObservation);
}
```

`UnicastHandle` and `BroadcastHandle` are host-issued bounded target capabilities: acquiring them may fail on missing targets or queue pressure, and version code never receives sockets, channels, WebSocket messages, tasks, TLS state, or lobby maps.

### Client asset capability

```rust
pub struct AssetKey(pub String);

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texture_coordinates: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

pub enum TextureFormat {
    Rgba8,
    Rgb8,
    Luma8,
}

pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub pixels: Vec<u8>,
}

pub struct MeshHandle(u64);

impl MeshHandle {
    pub const fn from_adapter_value(value: u64) -> Self;
    pub const fn adapter_value(self) -> u64;
}

pub struct TextureHandle(u64);

impl TextureHandle {
    pub const fn from_adapter_value(value: u64) -> Self;
    pub const fn adapter_value(self) -> u64;
}

pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

pub struct AssetDiagnostic {
    pub level: DiagnosticLevel,
    pub asset_key: Option<AssetKey>,
    pub message: String,
}

pub enum AssetError {
    NotFound(AssetKey),
    DecodeFailed(String),
    RegistrationFailed(String),
}

pub trait LegacyAssets: Send + Sync {
    fn load_mesh(&self, key: &AssetKey) -> Result<MeshData, AssetError>;
    fn load_texture(&self, key: &AssetKey) -> Result<TextureData, AssetError>;
    fn register_mesh(&self, key: &AssetKey, mesh: MeshData) -> Result<MeshHandle, AssetError>;
    fn register_texture(
        &self,
        key: &AssetKey,
        texture: TextureData,
    ) -> Result<TextureHandle, AssetError>;
    fn diagnose(&self, diagnostic: AssetDiagnostic);
}

pub struct LegacyCapabilities {
    pub clock: Arc<dyn LegacyClock>,
    pub random: Arc<dyn LegacyRandom>,
    pub transport: Arc<dyn LegacyTransport>,
    pub assets: Option<Arc<dyn LegacyAssets>>,
}
```

### Inner codec, session, and factory contracts

```rust
pub enum InnerFrame {
    Text(String),
    Binary(Vec<u8>),
}

impl InnerFrame {
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

pub struct DecodedInput {
    pub payload: Vec<u8>,
}

pub struct EncodedEvent {
    pub payload: Vec<u8>,
}

pub enum InnerCodecError {
    WrongFrameKind,
    InvalidFrame(String),
    DecodeFailed(String),
    EncodeFailed(String),
}

pub trait InnerCodec: Send + Sync {
    fn decode(&self, frame: &InnerFrame) -> Result<DecodedInput, InnerCodecError>;
    fn encode(&self, event: &EncodedEvent) -> Result<InnerFrame, InnerCodecError>;
}

pub struct SessionInput {
    pub peer_id: PeerId,
    pub received_at: MonotonicTimestamp,
    pub input: DecodedInput,
}

pub enum OutboundTarget {
    Unicast(UnicastHandle),
    Broadcast(BroadcastHandle),
    Peers(Vec<PeerId>),
}

pub struct OutboundEvent {
    pub target: OutboundTarget,
    pub event: EncodedEvent,
}

pub struct CloseRequest {
    pub peer_id: PeerId,
    pub reason: CloseReason,
}

pub struct SessionUpdate {
    pub outbound: Vec<OutboundEvent>,
    pub scheduling: Vec<SchedulingRequest>,
    pub closes: Vec<CloseRequest>,
}

pub struct LobbyStatus {
    pub code: String,
    pub detail: Option<String>,
}

pub struct AdmissionRefusal {
    pub code: String,
    pub message: String,
}

pub struct LeaveReason {
    pub close_reason: CloseReason,
    pub detail: Option<String>,
}

pub trait GameSession: Send {
    fn step(
        &mut self,
        timestamp: MonotonicTimestamp,
        inputs: Vec<SessionInput>,
    ) -> SessionUpdate;
    fn join(&mut self, admission: AdmissionMetadata) -> Result<SessionUpdate, AdmissionRefusal>;
    fn leave(&mut self, peer_id: PeerId, reason: LeaveReason) -> SessionUpdate;
    fn lobby_status(&self) -> LobbyStatus;
}

pub struct SessionCreationData {
    pub game_key: GameKey,
    pub session_id: SessionId,
    pub lobby_name: String,
    pub lobby_seed: LobbySeed,
    pub created_at: MonotonicTimestamp,
    pub configured_rules: Vec<u8>,
}

pub enum FactoryError {
    InvalidConfiguration(String),
    CapabilityUnavailable(String),
    ConstructionFailed(String),
}

pub trait GameFactory: Send + Sync {
    fn create(
        &self,
        capabilities: &LegacyCapabilities,
        creation: &SessionCreationData,
    ) -> Result<Box<dyn GameSession>, FactoryError>;
}
```

The host supplies timestamped inputs in deterministic admission and receipt order, invokes exactly one mutable authority for each lobby, charges limits outside the session, encodes returned events through the matching codec, and enqueues only through bounded outer transport.

### Hosted manifest

```rust
pub struct HostedManifest {
    pub games: Vec<HostedGame>,
}

pub struct HostedGame {
    pub game_id: String,
    pub game_version: u32,
    pub package: String,
    pub latest: bool,
    pub limits_profile: String,
    pub fixture_suite: String,
    pub legacy_game: Option<String>,
}

impl HostedGame {
    pub fn game_key(&self) -> GameKey;
}

pub struct ManifestParseError {
    pub message: String,
}

pub enum ManifestValidationError {
    DuplicateGameKey(GameKey),
    MultipleLatest {
        game_id: String,
    },
    MissingPackage(GameKey),
    MissingLimitsProfile(GameKey),
    MissingFixtureSuite(GameKey),
    InvalidGameId {
        game_id: String,
    },
    InvalidLegacySelector {
        game_key: GameKey,
        selector: String,
    },
    DuplicateLegacySelector {
        selector: String,
    },
}

pub fn parse_hosted_manifest(source: &str) -> Result<HostedManifest, ManifestParseError>;

pub fn validate_hosted_manifest(
    manifest: &HostedManifest,
) -> Result<(), Vec<ManifestValidationError>>;

pub fn is_valid_game_id(game_id: &str) -> bool;

pub fn is_valid_legacy_selector(selector: &str) -> bool;
```

The TOML representation is `[[games]]` with field names identical to `HostedGame`; `package`, `latest`, `limits_profile`, `fixture_suite`, and `legacy_game` accept serde defaults so pure semantic validation can distinguish a missing required value from malformed TOML.

## `ember-net::outer` frozen signatures

The canonical JSON enums use `#[serde(tag = "type", content = "payload", rename_all = "snake_case")]`; `OuterErrorCode` uses `#[serde(rename_all = "snake_case")]`.

### Messages and codec

```rust
pub const OUTER_VERSION: u16 = 1;
pub const SUPPORTED_OUTER_VERSIONS: [u16; 1] = [OUTER_VERSION];
pub const MAX_OUTER_FRAME_BYTES: usize = 64 * 1024;

pub struct Hello {
    pub outer_version: u16,
    pub handle: String,
}

pub struct Welcome {
    pub outer_version: u16,
    pub supported_outer_versions: Vec<u16>,
}

pub struct ListLobbies;

pub struct LobbyEntry {
    pub game_id: String,
    pub game_version: u32,
    pub lobby_name: String,
    pub password_protected: bool,
    pub occupancy: u16,
    pub capacity: u16,
    pub status: LobbyStatus,
}

pub struct LobbyStatus {
    pub code: String,
    pub detail: Option<String>,
}

pub struct Lobbies {
    pub entries: Vec<LobbyEntry>,
}

pub struct CreateLobby {
    pub game_id: String,
    pub game_version: u32,
    pub lobby_name: String,
    pub password: Option<String>,
}

pub struct JoinLobby {
    pub game_id: String,
    pub game_version: u32,
    pub lobby_name: String,
    pub password: Option<String>,
}

pub struct Joined {
    pub game_id: String,
    pub game_version: u32,
    pub lobby_name: String,
}

pub struct VersionNotHosted {
    pub requested_game: String,
    pub requested_version: u32,
    pub hosted_versions_for_game: Vec<u32>,
}

pub struct GameNotHosted {
    pub requested_game: String,
    pub hosted_games: Vec<String>,
}

pub enum OuterErrorCode {
    MalformedMessage,
    MessageTooLarge,
    UnexpectedMessage,
    RepeatedHello,
    PayloadBeforeJoin,
    UnsupportedOuterVersion,
    InvalidRequest,
    InternalError,
}

pub struct OuterError {
    pub code: OuterErrorCode,
    pub message: String,
}

pub enum ClientMessage {
    Hello(Hello),
    ListLobbies(ListLobbies),
    CreateLobby(CreateLobby),
    JoinLobby(JoinLobby),
}

pub enum ServerMessage {
    Welcome(Welcome),
    Lobbies(Lobbies),
    Joined(Joined),
    VersionNotHosted(VersionNotHosted),
    GameNotHosted(GameNotHosted),
    Error(OuterError),
}

pub enum OuterCodecError {
    FrameTooLarge {
        actual: usize,
        maximum: usize,
    },
    MalformedJson {
        detail: String,
    },
    UnsupportedOuterVersion {
        requested: u16,
        supported: Vec<u16>,
    },
    EncodeFailed {
        detail: String,
    },
}

impl OuterCodecError {
    pub fn outer_error(&self) -> OuterError;
    pub const fn closes_connection(&self) -> bool;
}

pub fn decode_client_frame(frame: &[u8]) -> Result<ClientMessage, OuterCodecError>;

pub fn encode_server_frame(message: &ServerMessage) -> Result<Vec<u8>, OuterCodecError>;
```

The permanent bootstrap decoder reads the adjacent-tag `Hello` shape, checks `outer_version` against `SUPPORTED_OUTER_VERSIONS`, and never reinterprets an unsupported version; outer input and encoded output are charged against `MAX_OUTER_FRAME_BYTES` before use.

### State machine

```rust
pub enum InnerPayload {
    Text(String),
    Binary(Vec<u8>),
}

pub enum StateInput {
    Outer(ClientMessage),
    Inner(InnerPayload),
    Control,
    TransportClosed,
}

pub enum ConnectionState {
    AwaitHello,
    Browsing {
        outer_version: u16,
    },
    Joined {
        game_id: String,
        game_version: u32,
        lobby_name: String,
    },
    Closed,
}

pub enum StateAction {
    AcceptHello(Hello),
    ListLobbies,
    CreateLobby(CreateLobby),
    JoinLobby(JoinLobby),
    DispatchInner(InnerPayload),
    HandleControl,
    Reject {
        error: OuterError,
        close_after_error: bool,
    },
    Close,
    Ignore,
}

pub enum StateMutationError {
    NotBrowsing,
}

pub struct StateMachine {
    state: ConnectionState,
}

impl StateMachine {
    pub const fn new() -> Self;
    pub const fn state(&self) -> &ConnectionState;
    pub fn transition(&mut self, input: StateInput) -> StateAction;
    pub fn mark_joined(&mut self, joined: Joined) -> Result<(), StateMutationError>;
    pub fn close(&mut self);
}
```

The host decodes `StateInput::Outer` only in `AwaitHello` or `Browsing`; after `mark_joined`, every WebSocket text or binary data frame becomes `StateInput::Inner` with exact payload bytes and frame kind preserved.

Malformed JSON, oversize input, unsupported outer versions, wrong-state messages, repeated hello, and pre-join inner payloads produce their stable outer error when safe and close because continued interpretation is ambiguous; password failure, full lobbies, version selection refusals, and version-owned admission refusals leave the connection in `Browsing`.

## Hosted manifest freeze

`games/hosted.toml` initially hosts latest `arena/12` from package `ember-game-arena-v12` with fixture suite `arena-v12-hosted-contract` and legacy selector `arena`, and latest `fire/1` from package `ember-game-fire-v1` with fixture suite `fire-v1-hosted-contract` and legacy selector `fire`.

Both limits-profile identifiers contain `estate-measurement-required`; they are product-visible placeholders and are not claims of measured or production-safe budgets.

## Frozen randomness construction

Pending post-freeze implementation; the trait key and required inputs are frozen above, while the exact domain separator, field encoding, hash expansion, endianness, and draw extraction will be added here before implementation is complete and will then be immutable semantics.

## Post-freeze additive changelog
