//! Shared, game-neutral client networking scaffolding.
//!
//! Transport lifecycle and prediction bookkeeping live here. Games retain
//! ownership of wire payloads, simulation meaning, and presentation policy.

#![deny(missing_docs)]
// Public names remain explicit after re-export from private implementation modules.
#![allow(clippy::module_name_repetitions)]

mod connection;
mod frame;
mod handshake;
mod hooks;
mod prediction;
mod snapshot;
mod transport;

pub use connection::{ClientConnection, ClientDiagnostics, ConnectionProgress};
pub use frame::{Keepalive, WireFrame};
pub use handshake::{
    CanonicalHandshake, CanonicalSelection, HandshakeProgress, HandshakeProvider,
    HandshakeUpdate, LegacyHandshake, LegacyJsonTags,
};
pub use hooks::{
    AcknowledgementMode, CorrectionMode, HookError, InnerFrameCodec, PredictionHooks,
    RemoteEntityHooks, ReplayContext,
};
pub use prediction::{InputHistory, Reconciliation, Reconciler, SequenceAllocator, SequencedInput};
pub use snapshot::{RemoteSnapshot, RemoteSnapshotBuffer, SnapshotPush};
pub use transport::{
    CloseKind, ConnectionClose, ConnectionDiagnostics, SendError, TransportConfig,
    TransportStatus, WebSocketTransport,
};
