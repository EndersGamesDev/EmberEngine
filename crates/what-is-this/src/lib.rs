//! Browser diagnostic and benchmark client for “what is this?”.

#![deny(missing_docs)]

mod kernels;

pub use kernels::{FloatProbeResult, KernelSpec, KernelSuite, jank_chunk, kernel_specs};

#[cfg(target_arch = "wasm32")]
mod wasm_api;
