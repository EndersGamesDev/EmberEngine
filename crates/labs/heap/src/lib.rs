//! Descriptor-heap-effect WebGL2 lab and native allocator evidence.

mod heap;
#[cfg(any(test, target_arch = "wasm32"))]
mod kernels;
mod span;
#[cfg(any(test, target_arch = "wasm32"))]
mod spike;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use heap::{Descriptor, Handle, HeapAllocator, HeapError, HeapKind, PackedDescriptor};
pub use span::{
    DataSpan, DeliveryPlan, DispatchHeader, PackedSpan, SpanArena, SpanDirectory, SpanError,
    StaticHeaders, WallTerm,
};
#[cfg(target_arch = "wasm32")]
pub use spike::{cancel_heap_spike, run_heap_spike_json};
