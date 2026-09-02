//! Typed integration failures that remain readable at the page boundary.

use thiserror::Error;

/// Failure categories owned or relayed by the Julibrot application.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AppError {
    /// A loader, worker, or wasm artifact carried another ABI version.
    #[error("{component} ABI version {actual} does not match required version {expected}")]
    VersionSkew {
        /// Artifact whose version differed.
        component: &'static str,
        /// Version required by this application.
        expected: u32,
        /// Version supplied by the artifact.
        actual: u32,
    },
    /// WebGL2 or a required device capability was absent.
    #[error("capability refusal during {operation}: {detail}")]
    Capability {
        /// Operation that discovered the wall.
        operation: &'static str,
        /// Adapter or browser detail.
        detail: String,
    },
    /// The selected device was lost.
    #[error("device lost during {operation}: {detail}")]
    DeviceLost {
        /// Operation active when loss was observed.
        operation: &'static str,
        /// Device callback detail.
        detail: String,
    },
    /// The non-panicking handler observed an uncaptured GPU error.
    #[error("uncaptured GPU error during {operation}: {detail}")]
    UncapturedGpu {
        /// Attributed operation.
        operation: &'static str,
        /// Error text supplied by wgpu.
        detail: String,
    },
    /// A generation-tagged validation scope captured a GPU error.
    #[error("captured GPU error during {operation} generation {generation}: {detail}")]
    CapturedGpu {
        /// Scoped operation.
        operation: &'static str,
        /// Selection or measurement generation.
        generation: u64,
        /// Error text supplied by wgpu.
        detail: String,
    },
    /// Another generation owns the sole acquired surface image.
    #[error("surface generation {owner} is live; generation {requested} cannot acquire")]
    SurfaceBusy {
        /// Current owner.
        owner: u64,
        /// Requesting generation.
        requested: u64,
    },
    /// A recoverable timeout skipped one requested surface frame.
    #[error("surface frame skipped: {detail}")]
    SurfaceSkipped {
        /// Timeout or retry detail.
        detail: String,
    },
    /// Surface creation, acquisition, or configuration failed.
    #[error("surface failure: {detail}")]
    Surface {
        /// Backend detail.
        detail: String,
    },
    /// Delayed work attempted to publish under another generation.
    #[error("stale generation {observed}; current generation is {current}")]
    StaleGeneration {
        /// Delayed generation.
        observed: u64,
        /// Current generation.
        current: u64,
    },
    /// The shared owner epoch could no longer advance.
    #[error("viewer owner epoch exhausted")]
    EpochExhausted,
    /// The session-local request generation could no longer advance.
    #[error("orbit request generation exhausted")]
    GenerationExhausted,
    /// A finite wall elapsed before completion.
    #[error("{operation} exceeded its {deadline_ms} ms deadline")]
    Deadline {
        /// Bounded operation.
        operation: &'static str,
        /// Configured wall in milliseconds.
        deadline_ms: f64,
    },
    /// A completion exhausted the fixed poll budget.
    #[error("{operation} exhausted its {polls} completion polls")]
    CompletionPollLimit {
        /// Bounded operation.
        operation: &'static str,
        /// Poll count at refusal.
        polls: u32,
    },
    /// Four-byte fence mapping failed.
    #[error("fence mapping failed during {operation}: {detail}")]
    Mapping {
        /// Measured operation.
        operation: &'static str,
        /// Mapping detail.
        detail: String,
    },
    /// Worker-owned typed error rendered at the app boundary.
    #[error("worker failure: {0}")]
    Worker(String),
    /// Math-owned typed error rendered at the app boundary.
    #[error("math failure: {0}")]
    Math(String),
    /// Kernels-owned typed error rendered at the app boundary.
    #[error("kernel failure: {0}")]
    Kernel(String),
    /// Present-owned typed error rendered at the app boundary.
    #[error("presentation failure: {0}")]
    Present(String),
    /// Facts or message serialization failed.
    #[error("serialization failure: {0}")]
    Serialization(String),
}
