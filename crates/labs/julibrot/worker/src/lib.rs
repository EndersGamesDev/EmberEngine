//! Reference-orbit worker, ownership channel, and versioned viewer state.

#![deny(missing_docs)]

mod codec;
mod error;
mod slots;
mod wire;

pub use codec::{
    CoordinateDescriptor, EncodedCentre, OrbitReason, OrbitRequest, RequestBodyView,
};
pub use error::{ChannelError, ErrorCode};
pub use slots::{FourSlotModel, SlotId, SlotOwner};
pub use wire::{
    BUFFER_OVERHEAD_BYTES, ERROR_RECORD_BYTES, ErrorRecord, HEADER_BYTES, JULIBROT_ABI_VERSION,
    MAGIC, MessageHeader, MessageKind, ORBIT_RECORD_BYTES, POOL_TRAILER_BYTES, Pool, PoolTrailer,
    ReferenceOrbitRecord, TRAILER_MAGIC, WireBuffer, buffer_capacity,
};
