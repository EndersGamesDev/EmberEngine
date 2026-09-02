//! Descriptor-heap-effect WebGL2 lab and native allocator evidence.

mod heap;
#[cfg(any(test, target_arch = "wasm32"))]
mod kernels;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use heap::{Descriptor, Handle, HeapAllocator, HeapError, HeapKind, PackedDescriptor};
