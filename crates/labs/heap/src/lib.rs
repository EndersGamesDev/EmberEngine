//! Descriptor-heap-effect WebGL2 lab and native allocator evidence.

mod heap;
#[cfg(any(test, target_arch = "wasm32"))]
mod kernels;
#[cfg(any(test, target_arch = "wasm32"))]
mod spike;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use heap::{Descriptor, Handle, HeapAllocator, HeapError, HeapKind, PackedDescriptor};
#[cfg(target_arch = "wasm32")]
pub use spike::{cancel_heap_spike, run_heap_spike_json};
