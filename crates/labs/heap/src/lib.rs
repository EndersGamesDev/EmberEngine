//! Descriptor-heap-effect WebGL2 lab and native allocator evidence.

mod dialect;
mod heap;
#[cfg(any(test, target_arch = "wasm32"))]
mod kernels;
mod lattice;
mod mode_c;
mod span;
#[cfg(any(test, target_arch = "wasm32"))]
mod spike;

#[cfg(target_arch = "wasm32")]
mod lattice_gpu;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use dialect::{
    DialectError, DialectLimits, DispatchError, DispatchPlan, ForbiddenConstruct, KernelDesc,
    PagePass, RegisteredKernel,
};
pub use heap::{Descriptor, Handle, HeapAllocator, HeapError, HeapKind, PackedDescriptor};
pub use lattice::{
    BOX_INDICES, BoxVertex, FrameUniform, MODE_A_ROTATION_KERNEL, ModeAEndpoint, ModeARecordSet,
    box_vertices, frame_for, mode_a_endpoint, mode_a_records, mode_a_shader,
};
#[cfg(target_arch = "wasm32")]
pub use lattice_gpu::{
    cancel_heap_lattice, measure_heap_lattice_batch_json, render_heap_lattice_frame_json,
    select_heap_lattice_json, start_heap_lattice,
};
pub use mode_c::{
    ComparatorWork, EqualWorkSignature, ModeCFrameUniform, mode_c_pose, mode_c_register,
    mode_c_shader,
};
pub use span::{
    DataSpan, DeliveryPlan, DispatchHeader, PackedSpan, SpanArena, SpanDirectory, SpanError,
    StaticHeaders, WallTerm,
};
#[cfg(target_arch = "wasm32")]
pub use spike::{cancel_heap_spike, run_heap_spike_json};
