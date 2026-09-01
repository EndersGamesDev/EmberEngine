use std::fmt;
use std::time::Duration;

use crate::WireFrame;

/// A game hook failure at the exact inner-frame boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HookError {
    /// A game message could not be encoded without changing its wire shape.
    Encode(String),
    /// A frame could not be decoded as the game's frozen message type.
    Decode(String),
    /// The WebSocket frame kind is not legal for this game version.
    WrongFrameKind,
}

impl fmt::Display for HookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(detail) => write!(formatter, "inner frame encode failed: {detail}"),
            Self::Decode(detail) => write!(formatter, "inner frame decode failed: {detail}"),
            Self::WrongFrameKind => formatter.write_str("inner frame has the wrong kind"),
        }
    }
}

impl std::error::Error for HookError {}

/// Exact inner-frame conversion owned by one game version.
pub trait InnerFrameCodec {
    /// Client-to-server message type.
    type Outbound;
    /// Server-to-client message type.
    type Inbound;

    /// Encodes one message without outer wrapping or reinterpretation.
    ///
    /// # Errors
    ///
    /// Returns an encoding failure without producing a partial frame.
    fn encode_inner(&self, message: &Self::Outbound) -> Result<WireFrame, HookError>;

    /// Decodes one exact post-handshake frame.
    ///
    /// # Errors
    ///
    /// Returns a frame-kind or game-codec failure.
    fn decode_inner(&self, frame: &WireFrame) -> Result<Self::Inbound, HookError>;
}

/// Which side of an acknowledgement remains available for replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcknowledgementMode {
    /// Drop the acknowledged command and everything before it.
    Through,
    /// Drop only commands before the acknowledged command.
    Before,
}

/// Player-visible treatment of one authoritative correction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorrectionMode {
    /// Apply the corrected state immediately, as for a teleport or Fire car.
    Snap,
    /// Ease presentation toward the corrected simulation state.
    Smooth,
}

/// Cursor data for one library-orchestrated replay callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayContext {
    /// Sequence carried by this retained input.
    pub sequence: u32,
    /// Client monotonic time when this input was emitted.
    pub sent_at: Duration,
    /// Time of the following retained input, when one exists.
    pub next_sent_at: Option<Duration>,
    /// Client monotonic time through which replay may run.
    pub replay_until: Duration,
    /// Authoritative acknowledgement that selected the replay cursor.
    pub acknowledgement: u32,
    /// Emission time of the retained acknowledged command, when present.
    pub acknowledged_sent_at: Option<Duration>,
    /// Opaque game-owned server timestamp extracted from the state.
    pub server_timestamp: u64,
}

/// Game-specific meaning applied by the shared prediction cursor mechanics.
pub trait PredictionHooks {
    /// Input retained until acknowledgement.
    type Input: Clone;
    /// Authoritative state carried by one server snapshot.
    type AuthoritativeState;
    /// Mutable locally predicted state.
    type PredictedState: Clone;

    /// Extracts the last input sequence represented by the server state.
    fn acknowledgement(&self, authoritative: &Self::AuthoritativeState) -> u32;

    /// Extracts the game-owned server timestamp used by diagnostics and replay.
    fn server_timestamp(&self, authoritative: &Self::AuthoritativeState) -> u64;

    /// Chooses whether the acknowledged command itself remains in history.
    fn acknowledgement_mode(&self) -> AcknowledgementMode;

    /// Rebases local prediction on the complete authoritative state.
    fn apply_authoritative(
        &self,
        predicted: &mut Self::PredictedState,
        authoritative: &Self::AuthoritativeState,
    );

    /// Applies the game-specific meaning of one retained input slice.
    fn replay_one_slice(
        &self,
        predicted: &mut Self::PredictedState,
        input: &Self::Input,
        context: ReplayContext,
        authoritative: &Self::AuthoritativeState,
    );

    /// Chooses the player-visible correction treatment after replay.
    fn snap_or_smooth(
        &self,
        before: &Self::PredictedState,
        after: &Self::PredictedState,
        authoritative: &Self::AuthoritativeState,
    ) -> CorrectionMode;
}

/// Game-specific presentation over the shared remote-snapshot cursor.
pub trait RemoteEntityHooks {
    /// One authoritative remote-entity snapshot.
    type Snapshot;
    /// Renderable remote-entity state produced from snapshots.
    type RenderState;

    /// Interpolates between two ordered server snapshots.
    fn interpolate_remote(
        &self,
        from: &Self::Snapshot,
        to: &Self::Snapshot,
        numerator: u64,
        denominator: u64,
    ) -> Self::RenderState;

    /// Extrapolates past the newest snapshot in game-owned timestamp units.
    fn dead_reckon_remote(&self, latest: &Self::Snapshot, elapsed: u64) -> Self::RenderState;

    /// Decides whether a discontinuity snaps rather than interpolates.
    fn snap_or_smooth_remote(&self, from: &Self::Snapshot, to: &Self::Snapshot) -> CorrectionMode;
}
