//! Descriptor-heap-effect WebGL2 lab and native allocator evidence.

#![recursion_limit = "256"]

#[cfg(target_arch = "wasm32")]
mod browser_error;
mod completion;
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) mod conformance;
mod dialect;
mod executor;
mod heap;
#[cfg(any(test, target_arch = "wasm32"))]
mod kernels;
mod lattice;
mod mode_c;
#[cfg(test)]
mod page_contract;
mod selection;
mod span;
#[cfg(any(test, target_arch = "wasm32"))]
mod spike;

#[cfg(target_arch = "wasm32")]
mod lattice_gpu;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use browser_error::{install_logging_handler, publish_browser_error};
pub use completion::{MAX_COMPLETION_POLLS, PollCounter};
pub use dialect::{
    DialectError, DialectLimits, DispatchError, DispatchPlan, ForbiddenConstruct, KernelDesc,
    PagePass, RegisteredKernel,
};
pub use executor::{
    DispatchSelector, ExecutorCapacity, ExecutorDispatch, ExecutorError, GpuKernel,
    GpuKernelExecutor, GpuKernelExecutorConfig, HeaderSetHandle, HeapPresentResources,
};
pub use heap::{Descriptor, Handle, HeapAllocator, HeapError, HeapKind, PackedDescriptor};
pub use lattice::{
    BOX_INDICES, BoxVertex, FrameUniform, MODE_A_ROTATION_KERNEL, ModeAEndpoint, ModeARecordSet,
    box_vertices, frame_for, mode_a_endpoint, mode_a_records, mode_a_shader,
};
#[cfg(target_arch = "wasm32")]
pub use lattice_gpu::{
    cancel_heap_lattice, conform_heap_lattice_json, measure_heap_lattice_batch_json,
    install_heap_lattice_panic_hook, render_heap_lattice_frame_json, select_heap_lattice_json,
    start_heap_lattice, take_heap_lattice_panic,
};
pub use mode_c::{
    ComparatorWork, EqualWorkSignature, ModeCFrameUniform, layer_comparator_draw_shader,
    layer_comparator_kernel, mode_c_pose, mode_c_register, mode_c_shader,
};
pub use selection::{SelectionEpoch, SurfaceOwnership};
pub use span::{
    DataSpan, DeliveryPlan, DispatchHeader, PackedSpan, SpanArena, SpanDirectory, SpanError,
    SpanPlan, StaticHeaders, WallTerm,
};
#[cfg(target_arch = "wasm32")]
pub use spike::{cancel_heap_spike, run_heap_spike_json};
