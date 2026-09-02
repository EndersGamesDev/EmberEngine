//! Reference-orbit worker, ownership channel, and versioned viewer state.

#![deny(missing_docs)]

mod channel;
#[cfg(target_arch = "wasm32")]
mod browser;
pub(crate) mod codec;
mod compute;
mod error;
mod owner;
mod registry;
pub(crate) mod slots;
pub(crate) mod wire;

pub use channel::{
    BUFFER_RETURN_DEADLINE_US, JULIBROT_PHASE_IMPLEMENTED, MIN_MAX_ITER,
    ORBIT_BUDGET_US_PER_SECOND, OrbitLease, OrbitResponseView, OwnerEndpoint, ProducerEndpoint,
    RequestLease, SubmitOutcome, WorkerChannel, WorkerConfig, WorkerMode, worker_mode_from_search,
};
#[cfg(target_arch = "wasm32")]
pub use browser::{allocate_transfer_buffer, worker_main};
pub use codec::{CoordinateDescriptor, EncodedCentre, OrbitReason, OrbitRequest};
pub use compute::{
    MathFailureCode, MonotonicClock, ORBIT_CHUNK_MAX_ITERATIONS, ORBIT_CHUNK_MAX_US, OrbitTaskPoll,
    ReferenceOrbitTask,
};
pub use ember_julibrot_math::{ComputedOrbit, ReferenceOrbitRecord};
pub use error::{ChannelError, ErrorCode};
pub use owner::{
    HotDrain, HotState, MainDrain, MainState, OrbitDisposition, OrbitHandle, ViewerOwner,
    ViewerState,
};
pub use registry::{OrbitRegistry, RegistryError};
pub use wire::{
    BUFFER_OVERHEAD_BYTES, ERROR_RECORD_BYTES, ErrorRecord, HEADER_BYTES, JULIBROT_ABI_VERSION,
    MAGIC, MessageHeader, MessageKind, ORBIT_RECORD_BYTES, POOL_TRAILER_BYTES, Pool, PoolTrailer,
    TRAILER_MAGIC, buffer_capacity,
};
