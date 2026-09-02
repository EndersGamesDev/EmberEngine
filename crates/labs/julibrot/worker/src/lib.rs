//! Reference-orbit worker, ownership channel, and versioned viewer state.

#![deny(missing_docs)]

pub(crate) mod codec;
mod channel;
mod error;
mod owner;
mod registry;
pub(crate) mod slots;
pub(crate) mod wire;

pub use codec::{CoordinateDescriptor, EncodedCentre, OrbitReason, OrbitRequest};
pub use channel::{
    OrbitLease, OrbitResponseView, OwnerEndpoint, ProducerEndpoint, RequestLease, SubmitOutcome,
    WorkerChannel, WorkerConfig, WorkerMode,
};
pub use error::{ChannelError, ErrorCode};
pub use owner::{
    HotDrain, HotState, MainDrain, MainState, OrbitDisposition, OrbitHandle, ViewerOwner,
    ViewerState,
};
pub use registry::{OrbitRegistry, RegistryError};
pub use wire::{
    BUFFER_OVERHEAD_BYTES, ERROR_RECORD_BYTES, ErrorRecord, HEADER_BYTES, JULIBROT_ABI_VERSION,
    MAGIC, MessageHeader, MessageKind, ORBIT_RECORD_BYTES, POOL_TRAILER_BYTES, Pool, PoolTrailer,
    ReferenceOrbitRecord, TRAILER_MAGIC, buffer_capacity,
};
