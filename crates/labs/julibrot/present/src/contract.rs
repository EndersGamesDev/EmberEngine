use std::sync::Arc;

use ember_julibrot_kernels::{EscapeGrid, RefinementLevel};
use ember_julibrot_math::{Plane, Pose, ViewMode};
use ember_julibrot_worker::{HotState, MainState};
use thiserror::Error;

use crate::{PaletteId, PaletteRecord, palette};

/// Refresh-rate owner state adapted with its immutable sampled plane and standing VIEW time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentHot {
    /// Owner observation epoch; attribution only, never a compatibility key.
    pub epoch: u64,
    /// Worker-owned hot state.
    pub state: HotState,
    /// Math-owned sampled plane frozen for this refresh.
    pub plane: Plane,
    /// Monotonic seconds used by the standing VIEW rotation.
    pub view_time_seconds: f64,
}

/// Arrival-rate owner state adapted with a published escape grid and selected view.
#[derive(Clone, Debug, PartialEq)]
pub struct PresentMain {
    /// Owner observation epoch; attribution only.
    pub epoch: u64,
    /// Worker-owned main state.
    pub state: MainState,
    /// Kernels-owned escape grid whose active prefix is ready in DATA.
    pub grid: EscapeGrid,
    /// Selected presentation view.
    pub view: ViewMode,
}

impl PresentMain {
    pub(crate) fn selected_palette(&self) -> Option<(PaletteId, PaletteRecord)> {
        let id = match self.state.palette_id {
            0 => PaletteId::Classic,
            1 => PaletteId::Ember,
            2 => PaletteId::Ice,
            _ => return None,
        };
        Some((id, palette(id)))
    }
}

/// Device and bounded-completion policy supplied by the app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentConfig {
    /// Configured surface colour format.
    pub surface_format: wgpu::TextureFormat,
    /// Live dynamic-uniform alignment limit in bytes.
    pub min_uniform_buffer_offset_alignment: u32,
    /// Monotonic wall deadline for either four-byte fence.
    pub fence_deadline_ms: f64,
    /// Maximum cooperative observations of one fence.
    pub max_fence_polls: u32,
}

impl PresentConfig {
    /// Contracted v1 fence deadline in milliseconds.
    pub const V1_FENCE_DEADLINE_MS: f64 = 30_000.0;
    /// Contracted v1 cooperative poll limit.
    pub const V1_MAX_FENCE_POLLS: u32 = 4_096;
}

/// Borrowed app-owned surface target and refresh attribution.
#[derive(Clone, Copy, Debug)]
pub struct FrameState<'a> {
    /// View of the surface image retained by the app through warp completion.
    pub surface_view: &'a wgpu::TextureView,
    /// Physical canvas width in pixels.
    pub canvas_width: u32,
    /// Physical canvas height in pixels.
    pub canvas_height: u32,
    /// Monotonic app refresh identifier.
    pub refresh_id: u64,
    /// Monotonic submission timestamp in milliseconds.
    pub now_ms: f64,
}

/// One fence-measured submission class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmissionKind {
    /// Selected flat or tumbled scene pass.
    Scene,
    /// Sole fullscreen reprojection pass.
    Warp,
}

/// Honesty label for warm-up and policy selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleClass {
    /// First sample after construction, extent allocation, or view change.
    ColdWarmUp,
    /// Second sample used to decide the displayed 100 ms policy.
    PolicyProbe,
    /// Subsequent comparable sample.
    Measured,
}

/// Completed four-byte-fence measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubmissionMeasurement {
    /// Scene or warp region.
    pub kind: SubmissionKind,
    /// Monotonic identity within that kind.
    pub id: u64,
    /// Retained source scene for a valid warp; absent for clear-only output.
    pub source_scene_id: Option<u64>,
    /// Warm-up and policy label.
    pub sample_class: SampleClass,
    /// Submission-start through callback-observation wall in milliseconds.
    pub wall_ms: f64,
    /// First-poll through callback-observation wall in milliseconds.
    pub fence_wait_ms: f64,
    /// Number of cooperative device polls.
    pub polls: u32,
}

/// Fence-completed scene texture and the semantic state rendered into it.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFrame {
    /// Monotonic scene identity.
    pub scene_id: u64,
    /// Immutable pose captured at submission and rebased on accepted references.
    pub pose: Pose,
    /// Palette captured by the scene pass.
    pub palette: PaletteId,
    /// Delivered iteration cap.
    pub iteration_cap: u32,
    /// Delivered kernels refinement level.
    pub level: RefinementLevel,
    /// Delivered scene-target extent in pixels.
    pub extent: [u32; 2],
    /// Texture index, always zero or one.
    pub texture_index: u32,
    /// Accepted centre revision against which the pose is expressed.
    pub centre_revision: u32,
    /// Defining plane origin including Julia's constant.
    pub plane_origin_f64: [f64; 4],
    /// Fence measurement for this scene.
    pub measurement: SubmissionMeasurement,
}

/// CPU-only result of the f64 reprojection planner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpPlan {
    /// Three padded inverse-sampling homography rows.
    pub rows: [[f32; 4]; 3],
    /// Whether sampling the retained texture is honest.
    pub source_valid: bool,
    /// Exact, approximate, or clear-only plan kind.
    pub kind: WarpKind,
    /// Plane-chart residual in retained-frame pixels.
    pub chart_residual: f64,
    /// Maximum tumbled approximation error in pixels, when applicable.
    pub approx_max_error_px: Option<f64>,
    /// Ninety-fifth-percentile tumbled approximation error in pixels, when applicable.
    pub approx_p95_error_px: Option<f64>,
}

/// Reprojection algorithm selected for one HOT payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WarpKind {
    /// Math-owned exact plane-chart homography.
    FlatExact,
    /// Four-anchor image homography for a tumbled endpoint.
    TumbledHomography,
    /// Honest clear because no compatible source or finite plan exists.
    ClearOnly,
}

/// Why a completed scene was measured but not promoted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropReason {
    /// Iteration cap or defining plane origin changed.
    IncompatibleMain,
    /// A later MAIN selection replaced the captured work.
    ReplacedMain,
    /// Its delivered extent no longer satisfied the span contract.
    InvalidExtent,
}

/// Why bounded completion retired an unfinished fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FenceRefusal {
    /// Cooperative observation count reached its limit.
    PollLimit,
    /// Monotonic elapsed wall reached its deadline.
    Deadline,
    /// Mapping or device completion failed.
    Device,
    /// App or presenter cancelled the submission.
    Cancelled,
}

/// App-visible asynchronous completion event.
#[derive(Clone, Debug, PartialEq)]
pub enum PresentEvent {
    /// A compatible scene became the retained warp source.
    SceneCompleted {
        /// Newly promoted frame.
        frame: SceneFrame,
    },
    /// A scene completed and was measured but was not compatible.
    SceneDropped {
        /// Monotonic scene identity.
        scene_id: u64,
        /// Captured orbit generation.
        orbit_generation: u32,
        /// Exact rejection reason.
        reason: DropReason,
        /// Completed fence measurement.
        measurement: SubmissionMeasurement,
    },
    /// A warp target may now be presented by the app.
    WarpCompleted {
        /// Completed fence measurement carrying the warp identity.
        measurement: SubmissionMeasurement,
    },
    /// A pending fence was boundedly retired.
    FenceRefused {
        /// Scene or warp submission.
        kind: SubmissionKind,
        /// Submission identity.
        id: u64,
        /// Exact refusal reason.
        reason: FenceRefusal,
        /// Poll count at retirement.
        polls: u32,
        /// Submission-start wall at retirement in milliseconds.
        wall_ms: f64,
    },
}

/// Honest current display state.
#[derive(Clone, Debug, PartialEq)]
pub enum PresentStatus {
    /// No scene has completed yet; warp emits the clear colour.
    WaitingForFirstScene,
    /// Current HOT state exactly matches the retained scene.
    ShowingCompletedScene,
    /// Warp is moving a compatible older scene toward current HOT state.
    ShowingStaleApproximation,
    /// MAIN invalidated the retained source.
    ClearForIncompatibleMain,
    /// A typed synchronous refusal prevented submission.
    Refused(PresentError),
}

/// Receipt returned before a warp fence completes.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameReceipt {
    /// App refresh identity.
    pub refresh_id: u64,
    /// Monotonic warp identity matched by the eventual event.
    pub warp_id: u64,
    /// Retained scene sampled by this warp, absent for clear-only output.
    pub source_scene_id: Option<u64>,
    /// Honest display status at submission.
    pub status: PresentStatus,
}

/// Immutable app-facing presentation facts snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct PresentFacts {
    /// Newest retained scene identity.
    pub completed_scene_id: Option<u64>,
    /// Sole pending scene identity.
    pub in_flight_scene_id: Option<u64>,
    /// Orbit generation carried by the retained source.
    pub source_generation: Option<u32>,
    /// Delivered width in pixels.
    pub delivered_width: u32,
    /// Delivered height in pixels.
    pub delivered_height: u32,
    /// Delivered refinement level.
    pub delivered_level: Option<RefinementLevel>,
    /// Delivered iteration cap.
    pub iteration_cap: Option<u32>,
    /// Latest MAIN palette selection.
    pub palette: PaletteId,
    /// Latest MAIN view selection.
    pub view: ViewMode,
    /// Latest HOT displacement in current pixels.
    pub centre_from_reference_px: [f64; 2],
    /// Latest accepted reference shift in current pixels.
    pub reference_shift_px: [f64; 2],
    /// Most recent completed scene measurement.
    pub last_scene: Option<SubmissionMeasurement>,
    /// Most recent completed warp measurement.
    pub last_warp: Option<SubmissionMeasurement>,
    /// Warps completed against the retained scene.
    pub reprojected_per_scene: Option<u32>,
    /// Refresh submissions without a retained scene.
    pub refreshes_without_scene: u64,
    /// Scene-target reallocations after initial construction.
    pub texture_reallocations: u32,
    /// Latest plane-chart residual.
    pub chart_residual: Option<f64>,
    /// Latest tumbled maximum approximation error.
    pub tumbled_max_error_px: Option<f64>,
    /// Honest current status.
    pub status: PresentStatus,
}

impl Default for PresentFacts {
    fn default() -> Self {
        Self {
            completed_scene_id: None,
            in_flight_scene_id: None,
            source_generation: None,
            delivered_width: 0,
            delivered_height: 0,
            delivered_level: None,
            iteration_cap: None,
            palette: PaletteId::Classic,
            view: ViewMode::Flat,
            centre_from_reference_px: [0.0; 2],
            reference_shift_px: [0.0; 2],
            last_scene: None,
            last_warp: None,
            reprojected_per_scene: None,
            refreshes_without_scene: 0,
            texture_reallocations: 0,
            chart_residual: None,
            tumbled_max_error_px: None,
            status: PresentStatus::WaitingForFirstScene,
        }
    }
}

/// Typed synchronous presentation refusal.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum PresentError {
    /// Delivered grid prefix is empty, overflows, or exceeds its span.
    #[error("grid {width}x{height} does not fit logical length {logical_len}")]
    InvalidGrid {
        /// Delivered width.
        width: u32,
        /// Delivered height.
        height: u32,
        /// Span capacity.
        logical_len: u32,
    },
    /// Exactly one scene target is already in flight.
    #[error("scene {scene_id} is already in flight")]
    SceneBusy {
        /// Existing scene identity.
        scene_id: u64,
    },
    /// The published span directory index is outside the immutable capacity.
    #[error("span directory index {directory_index} is stale or out of range")]
    StaleSpan {
        /// Published directory index.
        directory_index: u32,
    },
    /// Fixed LDR scene targets are unsupported.
    #[error("Rgba8Unorm scene targets are unsupported")]
    UnsupportedSceneFormat,
    /// App-selected surface format cannot be rendered.
    #[error("surface format {format:?} is unsupported for warp output")]
    UnsupportedSurfaceFormat {
        /// Rejected surface format.
        format: wgpu::TextureFormat,
    },
    /// Texture or depth extent could not be allocated.
    #[error("could not allocate scene extent {width}x{height}")]
    ExtentAllocation {
        /// Delivered width.
        width: u32,
        /// Delivered height.
        height: u32,
    },
    /// Tumbled index count exceeded the u32 draw contract.
    #[error("tumbled index count overflowed for {width}x{height}")]
    IndexCountOverflow {
        /// Delivered width.
        width: u32,
        /// Delivered height.
        height: u32,
    },
    /// A checked GPU operation failed synchronously.
    #[error("device operation failed: {operation}")]
    Device {
        /// Stable operation name.
        operation: &'static str,
    },
    /// Surface target has a zero physical extent.
    #[error("surface target extent is zero")]
    SurfaceTargetZero,
}

/// Shared device and queue ownership aliases used by the app-facing constructor.
pub(crate) type SharedDevice = Arc<wgpu::Device>;
pub(crate) type SharedQueue = Arc<wgpu::Queue>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_palette_decode_is_exact() {
        let main = |palette_id| PresentMain {
            epoch: 0,
            state: MainState {
                palette_id,
                ..MainState::default()
            },
            grid: EscapeGrid {
                span: test_span(),
                width: 1,
                height: 1,
                level: RefinementLevel::Preview,
            },
            view: ViewMode::Flat,
        };
        assert_eq!(
            main(0).selected_palette().map(|selected| selected.0),
            Some(PaletteId::Classic)
        );
        assert_eq!(main(3).selected_palette(), None);
    }

    #[test]
    fn initial_facts_never_invent_delivered_values() {
        let facts = PresentFacts::default();
        assert_eq!(facts.delivered_width, 0);
        assert_eq!(facts.delivered_level, None);
        assert_eq!(facts.last_scene, None);
        assert_eq!(facts.status, PresentStatus::WaitingForFirstScene);
    }

    fn test_span() -> ember_lab_heap::DataSpan {
        let mut arena = ember_lab_heap::SpanArena::new(8, 1, 8, 256, 8)
            .expect("test arena is valid");
        arena.allocate_span(1, 1).expect("one record fits")
    }
}
