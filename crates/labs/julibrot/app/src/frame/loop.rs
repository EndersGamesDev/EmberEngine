//! Cross-slice progressive frame scheduling and browser GPU integration.

#[cfg(any(target_arch = "wasm32", test))]
use super::schedule::{
    FrameLoop, RefusalClass, SceneMode, apply_precision_mode, fence_error, schedule_exposure_fill,
    stamp_scene_level, stamped_screen_map, view_projection_changed,
};
#[cfg(test)]
use super::schedule::{PresenterPoll, RefinementSchedule, classify_refusal, stamped_extent};
#[cfg(any(target_arch = "wasm32", test))]
use super::warp::{defer_scene_until_relief_redraw, hold_redraw_during_scene};

#[cfg(test)]
use ember_julibrot_kernels::SampleStatus;
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_kernels::{KernelError, RefinementLevel, RefinementPlan};
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_math::{Plane, PoseMap};
#[cfg(test)]
use ember_julibrot_present::{FenceRefusal, SubmissionKind};

#[cfg(any(target_arch = "wasm32", test))]
use crate::AppError;

#[cfg(test)]
const LEVELS: [RefinementLevel; 3] = [
    RefinementLevel::Preview,
    RefinementLevel::Interactive,
    RefinementLevel::Final,
];

/// Presentation rank of the coverage-first backdrop while it is the only completed layer.
///
/// Its records come from the backdrop plan's Final level, but calling the temporary composed
/// frame Final would let accepted-warp policy skip the main grid's still-due Preview.
#[cfg(any(target_arch = "wasm32", test))]
const BACKDROP_PRESENT_LEVEL: RefinementLevel = RefinementLevel::Preview;

/// Bound on sampled reference requests per accepted navigation.
#[cfg(any(target_arch = "wasm32", test))]
const SAMPLED_REFERENCE_LIMIT: u32 = 4;
#[cfg(any(target_arch = "wasm32", test))]
const REFERENCE_RECORD_BYTES: usize = 8;
#[cfg(any(target_arch = "wasm32", test))]
const REFERENCE_TEXEL_BYTES: usize = 16;
#[cfg(target_arch = "wasm32")]
const PAGE_MAX_ITERATION_CAP: u32 = 4_096;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReferenceCandidate {
    index: u32,
    rank: u8,
}

/// Mirrors the present census candidate on CPU records.
///
/// The rank is the same total order the census shader encodes: a record that never escaped within
/// the grid's cap ranks 255, a glitch that exhausted its reference ranks 254, an escaped record
/// ranks by its own count over 1..253, and a glitch from arithmetic failure ranks 0. Equal ranks
/// keep the lowest record index, so the same grid always names the same reference point.
#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a finite smooth count is clamped into the encoded rank range"
)]
fn select_reference_candidate(
    records: &[ember_julibrot_math::EscapeGridRecord],
    iteration_cap: u32,
) -> Option<ReferenceCandidate> {
    let cap = f64::from(iteration_cap.max(1));
    records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            let status = SampleStatus::from_f32(record.status)?;
            if status == SampleStatus::Horizon || status == SampleStatus::MapUncertain {
                return None;
            }
            let rank = if status == SampleStatus::Glitch {
                u8::from(
                    record.smooth_iter.to_bits()
                        == ember_julibrot_kernels::GLITCH_REFERENCE_EXHAUSTED.to_bits(),
                ) * 254
            } else if record.escaped == 0.0 {
                255
            } else if record.smooth_iter.is_finite() {
                let reached = f64::from(record.smooth_iter.ceil().max(0.0)).min(cap);
                1 + (252.0 * reached / cap + 0.5).floor() as u8
            } else {
                return None;
            };
            Some(ReferenceCandidate {
                index: u32::try_from(index).ok()?,
                rank,
            })
        })
        .fold(None, |best, candidate| match best {
            Some(best) if best.rank >= candidate.rank => Some(best),
            _ => Some(candidate),
        })
}

/// Decides whether one completed level should buy a new reference at its census candidate.
///
/// The level's cap must strictly outlast the accepted orbit, because only then can that grid's
/// top-ranked record name a longer one; the bound must not be spent; and no request may already be
/// outstanding for this same orbit length, or a ladder whose Interactive and Final caps both
/// outlast one short reference would spend two slots to buy a single correction.
#[cfg(any(target_arch = "wasm32", test))]
const fn sampled_reference_due(
    perturbation: bool,
    level_cap: u32,
    accepted_orbit_length: u32,
    requests_made: u32,
    request_at_length: Option<u32>,
) -> bool {
    perturbation
        && requests_made < SAMPLED_REFERENCE_LIMIT
        && accepted_orbit_length != 0
        && level_cap > accepted_orbit_length
        && !matches!(request_at_length, Some(length) if length == accepted_orbit_length)
}

#[cfg(any(target_arch = "wasm32", test))]
fn reference_texel_bytes(length: u32) -> Result<usize, AppError> {
    let count = usize::try_from(length)
        .map_err(|_| AppError::Worker("reference length does not fit usize".to_string()))?;
    count
        .checked_mul(REFERENCE_TEXEL_BYTES)
        .ok_or_else(|| AppError::Worker("reference texel byte length overflow".to_string()))
}

/// Chooses the coarse backdrop extent within one quarter of the Final record count.
#[cfg(any(target_arch = "wasm32", test))]
fn backdrop_extent(final_extent: [u32; 2]) -> Option<[u32; 2]> {
    let [width, height] = final_extent;
    let extent = [width / 2, height / 2];
    if extent.contains(&0) {
        return None;
    }
    let final_records = width.checked_mul(height)?;
    let backdrop_records = extent[0].checked_mul(extent[1])?;
    (backdrop_records <= final_records / 4).then_some(extent)
}

/// Which layer takes the next scene turn when both the coverage backdrop and the ladder are due.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CoverageTurn {
    Backdrop,
    Main,
}

/// Decides whether a fresh coverage backdrop may pre-empt the main refinement ladder this turn.
///
/// Coverage comes first, but only for its own delivery, not for the whole gesture. The backdrop is
/// requested against the current view stamp, and under a continuous drag that stamp moves every
/// frame: a rule that simply prefers the backdrop whenever its stamp is stale dispatches a fresh
/// backdrop before the ladder ever runs, so the whole drag is presented at the backdrop's coarse
/// sampling — at an apron of five that is about one sample per five main samples per axis — and the
/// main grid is never seen until the pose settles. The two alternate instead: one backdrop, then
/// one main level, then the next backdrop.
///
/// A turn is also yielded while a main scene is already submitted. Switching the presented layer
/// replaces the presenter's selection and drops whatever scene is in flight, so pre-empting a
/// running main dispatch would discard the very level this alternation exists to let through.
///
/// Returns whether the backdrop takes this turn, and the turn to carry into the next contest.
#[cfg(any(target_arch = "wasm32", test))]
const fn coverage_pre_empts(turn: CoverageTurn, main_in_flight: bool) -> (bool, CoverageTurn) {
    if main_in_flight || matches!(turn, CoverageTurn::Main) {
        (false, CoverageTurn::Backdrop)
    } else {
        (true, CoverageTurn::Main)
    }
}

/// Converts a map's applied apron into the kernel-only zoom offset.
#[cfg(any(target_arch = "wasm32", test))]
fn sampling_zoom_log2(zoom_log2: f64, apron_scale: f64) -> Result<f64, AppError> {
    if apron_scale.to_bits() == 1.0_f64.to_bits() {
        return Ok(zoom_log2);
    }
    if !zoom_log2.is_finite() || !apron_scale.is_finite() || apron_scale < 1.0 {
        return Err(AppError::Math(
            "sampling apron is not a finite scale at least one".to_string(),
        ));
    }
    let sampling_zoom = zoom_log2 - apron_scale.log2();
    sampling_zoom
        .is_finite()
        .then_some(sampling_zoom)
        .ok_or_else(|| AppError::Math("sampling zoom is not finite".to_string()))
}

#[cfg(test)]
fn expand_reference_texels_into(
    records: &[u8],
    length: u32,
    texels: &mut Vec<u8>,
) -> Result<(), AppError> {
    let count = usize::try_from(length)
        .map_err(|_| AppError::Worker("reference length does not fit usize".to_string()))?;
    let expected = count
        .checked_mul(REFERENCE_RECORD_BYTES)
        .ok_or_else(|| AppError::Worker("reference record byte length overflow".to_string()))?;
    if records.len() != expected {
        return Err(AppError::Worker(format!(
            "reference payload has {} bytes; expected {expected}",
            records.len()
        )));
    }
    let texel_bytes = reference_texel_bytes(length)?;
    if texels.capacity() < texel_bytes {
        texels
            .try_reserve_exact(texel_bytes.saturating_sub(texels.len()))
            .map_err(|error| {
                AppError::Worker(format!("reference upload reserve failed: {error}"))
            })?;
    }
    texels.clear();
    for record in records.as_chunks::<REFERENCE_RECORD_BYTES>().0 {
        texels.extend_from_slice(record);
        texels.extend_from_slice(&[0; REFERENCE_TEXEL_BYTES - REFERENCE_RECORD_BYTES]);
    }
    Ok(())
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HorizonFacts {
    pixels: u64,
    fraction: f64,
    uncertain_pixels: u64,
    uncertain_fraction: f64,
    condition_number: f64,
    edge_on: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    reason = "the facts census mirrors the kernel's f32 operation sequence"
)]
fn horizon_facts(map: PoseMap, extent: [u32; 2]) -> HorizonFacts {
    let [width, height] = extent;
    if width == 0 || height == 0 {
        return HorizonFacts::default();
    }
    let total = u64::from(width) * u64::from(height);
    let PoseMap::Mapped(screen_to_plane) = map else {
        return HorizonFacts {
            pixels: total,
            fraction: 1.0,
            uncertain_pixels: 0,
            uncertain_fraction: 0.0,
            condition_number: 0.0,
            edge_on: true,
        };
    };
    let rows: [[f32; 3]; 3] = core::array::from_fn(|row| {
        core::array::from_fn(|column| screen_to_plane.rows[row * 3 + column] as f32)
    });
    let mut horizon = 0_u64;
    let mut uncertain = 0_u64;
    for row in 0..height {
        for column in 0..width {
            let x = column as f32 + 0.5 - 0.5 * width as f32;
            let y = row as f32 + 0.5 - 0.5 * height as f32;
            let homogeneous = rows.map(|map_row| map_row[0] * x + map_row[1] * y + map_row[2]);
            if homogeneous[2].is_finite() && homogeneous[2] <= 0.0 {
                horizon += 1;
                continue;
            }
            if map_is_uncertain(rows, [x, y], homogeneous) {
                uncertain += 1;
            }
        }
    }
    HorizonFacts {
        pixels: horizon,
        fraction: horizon as f64 / total as f64,
        uncertain_pixels: uncertain,
        uncertain_fraction: uncertain as f64 / total as f64,
        condition_number: screen_to_plane.condition_number,
        edge_on: false,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::suboptimal_flops,
    reason = "the facts census mirrors the kernel's f32 operation sequence"
)]
fn map_is_uncertain(rows: [[f32; 3]; 3], point: [f32; 2], homogeneous: [f32; 3]) -> bool {
    if !homogeneous.iter().all(|value| value.is_finite()) || homogeneous[2] <= 0.0 {
        return true;
    }
    let scales = rows.map(|row| {
        row[0]
            .abs()
            .mul_add(point[0].abs(), row[1].abs() * point[1].abs())
            + row[2].abs()
    });
    let errors = scales.map(|scale| 4.0 * f32::EPSILON * scale);
    let mapped = [
        homogeneous[0] / homogeneous[2],
        homogeneous[1] / homogeneous[2],
    ];
    if !mapped.iter().all(|value| value.is_finite()) || homogeneous[2] <= errors[2] {
        return true;
    }
    let safe_denominator = homogeneous[2] - errors[2];
    let quotient = [
        (errors[0] + mapped[0].abs() * errors[2]) / safe_denominator,
        (errors[1] + mapped[1].abs() * errors[2]) / safe_denominator,
    ];
    quotient[0] * quotient[0] + quotient[1] * quotient[1] > 0.0625
}

#[cfg(any(target_arch = "wasm32", test))]
fn main_for_grid(
    mut state: ember_julibrot_worker::MainState,
    grid_width: u32,
    requested_width: u32,
) -> ember_julibrot_worker::MainState {
    let ratio = f64::from(grid_width) / f64::from(requested_width);
    state.reference_shift_px = state.reference_shift_px.map(|value| value * ratio);
    state
}

#[cfg(any(target_arch = "wasm32", test))]
const fn arrival_is_current(
    cancelled: bool,
    response_generation: u32,
    endpoint_generation: u32,
    navigation_pending_depth: u32,
) -> bool {
    !cancelled && response_generation == endpoint_generation && navigation_pending_depth == 0
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ReferenceLeaseIdentity {
    main_generation: u32,
    source_generation: u32,
    centre_revision: u32,
    plane: Plane,
    precision_mode: u32,
    precision_bits: u32,
    orbit_length: u32,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AcceptedReferenceFacts {
    reference_verification: &'static str,
    consumed_word_error_ulps: Option<u32>,
    precision_escalations: u32,
}

#[cfg(any(target_arch = "wasm32", test))]
const fn accepted_reference_facts(
    verification: ember_julibrot_worker::ReferenceVerification,
    consumed_word_error_ulps: Option<u32>,
    precision_escalations: u32,
) -> AcceptedReferenceFacts {
    let reference_verification = if precision_escalations != 0 {
        "Escalated"
    } else {
        match verification {
            ember_julibrot_worker::ReferenceVerification::Deferred => "Deferred",
            ember_julibrot_worker::ReferenceVerification::Stable => "Stable",
        }
    };
    AcceptedReferenceFacts {
        reference_verification,
        consumed_word_error_ulps,
        precision_escalations,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn renew_reference_lease_identity(
    lease: &mut ReferenceLeaseIdentity,
    main_generation: u32,
    centre_revision: u32,
    precision_mode: u32,
) {
    lease.main_generation = main_generation;
    lease.centre_revision = centre_revision;
    lease.precision_mode = precision_mode;
}

/// Tests whether the accepted perturbation reference lease belongs to the requested scene.
///
/// A freshly accepted short orbit may render once in its source generation so the existing census
/// can choose a better reference. Renewing that orbit into a later MAIN generation requires the
/// conservative full-cap length; zoom itself is dispatch scale and is deliberately absent.
#[cfg(any(target_arch = "wasm32", test))]
fn perturbation_reference_is_current(
    main_generation: u32,
    centre_revision: u32,
    plane: Plane,
    precision_mode: u32,
    requested_precision_bits: u32,
    requested_iteration_cap: u32,
    reference: Option<ReferenceLeaseIdentity>,
) -> bool {
    reference.is_some_and(|lease| {
        let orbit_is_sufficient = lease.orbit_length == requested_iteration_cap
            || lease.source_generation == main_generation;
        lease.main_generation == main_generation
            && lease.centre_revision == centre_revision
            && lease.plane == plane
            && lease.precision_mode == precision_mode
            && lease.precision_bits >= requested_precision_bits
            && orbit_is_sufficient
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn reference_submission_requires_worker(
    sampled: bool,
    centre_unchanged: bool,
    plane: Plane,
    precision_mode: u32,
    requested_precision_bits: u32,
    requested_iteration_cap: u32,
    reference: Option<ReferenceLeaseIdentity>,
) -> bool {
    sampled
        || !centre_unchanged
        || reference.is_none_or(|lease| {
            lease.plane != plane
                || lease.precision_mode != precision_mode
                || lease.precision_bits < requested_precision_bits
                || lease.orbit_length != requested_iteration_cap
        })
}

/// Returns the iteration cap MAIN publishes to present for the current selection.
///
/// Present reads MAIN's delivered cap as the selection identity and drops its retained scene
/// whenever that cap changes, so a per-level cap would annihilate every promotion on the very
/// refresh that advances the ladder. The plan's delivered cap is the one value that holds for
/// the whole Preview, Interactive, Final sequence and changes only when the request does.
#[cfg(any(target_arch = "wasm32", test))]
const fn published_iteration_cap(plan: &RefinementPlan) -> u32 {
    plan.delivered_max_iter
}

#[cfg(any(target_arch = "wasm32", test))]
fn optional_backdrop_plan(
    result: Result<RefinementPlan, KernelError>,
) -> Result<Option<RefinementPlan>, AppError> {
    match result {
        Ok(plan) => Ok(Some(plan)),
        Err(KernelError::Heap) => Ok(None),
        Err(error) => Err(AppError::Kernel(error.to_string())),
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use ember_julibrot_kernels::{
        DispatchFacts, EscapeGrid, GridExtent, JulibrotKernels, KERNEL_UNIFORM_BYTES, KernelError,
        KernelMode, OUTPUT_PAGE_SIDE, ReferenceOrbitInput, RefinementLevel, RefinementPlan,
    };
    use ember_julibrot_math::{
        BigCentre, EscapeParams, ObjectAngles, Plane, PoseMap, PrecisionMode, pixel_scale,
        precision_for, reference_shift_px, scale_split, shallow_pixel_scale, split_centre,
    };
    use ember_julibrot_present::{
        FrameState, HotSlot, PresentBackdrop, PresentConfig, PresentEvent, PresentHot, PresentMain,
        Presenter, SubmissionKind, WarpValidation, hot_stride,
    };
    use ember_julibrot_worker::{
        EncodedCentre, OrbitDisposition, OrbitHandle, OrbitRegistry, OrbitRequest, OwnerEndpoint,
        ProducerEndpoint, RegistryError, SubmitOutcome, WorkerChannel, WorkerConfig, WorkerFacts,
        WorkerMode,
    };
    use ember_lab_heap::{DataSpan, GpuKernelExecutor, GpuKernelExecutorConfig};

    use super::{
        BACKDROP_PRESENT_LEVEL, CoverageTurn, FrameLoop, PAGE_MAX_ITERATION_CAP, RefusalClass,
        SceneMode, backdrop_extent, coverage_pre_empts, horizon_facts, main_for_grid,
        published_iteration_cap, sampling_zoom_log2, stamp_scene_level, stamped_screen_map,
    };
    use crate::timing::ReferenceTimingSample;
    use crate::{
        AppError, BrowserRuntime, FramePolicy, FramePolicyTracker, LevelTimingLedger,
        RefreshOutcome, RefreshStatus, RunRequests, ViewerController,
    };

    mod backdrop;
    mod facts;
    mod reference;
    mod submit;

    const HEAP_SIDE: u16 = 512;
    const HEAP_LAYERS: u16 = 16;
    const DESCRIPTOR_CAPACITY: u32 = 64;
    const SPAN_CAPACITY: u32 = 16;
    const HANDLE_CAPACITY: u32 = 128;
    const DIRECTORY_BYTES: u32 = SPAN_CAPACITY * 16 + HANDLE_CAPACITY * 4;
    const MAX_HEADER_PAGES: u32 = 64;
    const MAX_HEADER_SETS: u32 = 9;

    #[derive(Debug)]
    struct RegisteredOrbit {
        span: DataSpan,
        length: u32,
        precision_bits: u32,
        precision_mode: &'static str,
    }

    #[derive(Debug)]
    struct SubmittedReference {
        generation: u32,
        view_centre: BigCentre,
        reference_centre: BigCentre,
        sampled: bool,
        zoom_log2: f64,
        plane: Plane,
        precision_mode: u32,
        request_transfer_us: Option<u64>,
        transferred_at_us: Option<u64>,
    }

    #[derive(Debug)]
    struct AcceptedReferenceReceipt {
        lease: super::ReferenceLeaseIdentity,
        view_centre: BigCentre,
        verification: ember_julibrot_worker::ReferenceVerification,
        max_consumed_word_error_ulps: Option<u32>,
        precision_escalations: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct SceneSelection {
        generation: u32,
        requested_iter_cap: u32,
        palette_id: u32,
        plane_origin_f64: [f64; 4],
        precision_mode: u32,
    }

    /// Everything the warp reproduces for one surface image, stamped when it is submitted.
    ///
    /// Comparing the stamp carried by the last presented image against the stamp of the current
    /// request is what makes "the page is showing an older view" a fact the loop can read, rather
    /// than something only the eye can see.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct ViewStamp {
        generation_applied: u32,
        centre_revision: u32,
        requested_iter_cap: u32,
        palette_id: u32,
        plane_origin_f64: [f64; 4],
        view: ember_julibrot_math::ViewControls,
        zoom_log2: f64,
        object_angles: ObjectAngles,
        map: PoseMap,
        precision_mode: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct BackdropReady {
        stamp: ViewStamp,
        map: PoseMap,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct BackdropFlight {
        scene_id: u64,
        stamp: ViewStamp,
        map: PoseMap,
    }

    #[derive(Debug)]
    struct BackdropGrid {
        plan: RefinementPlan,
        grid: EscapeGrid,
        ready: Option<BackdropReady>,
        in_flight: Option<BackdropFlight>,
    }

    impl ViewStamp {
        fn render_equivalent(self, other: Self) -> bool {
            let selection_matches = self.generation_applied == other.generation_applied
                && self.centre_revision == other.centre_revision
                && self.requested_iter_cap == other.requested_iter_cap
                && self.palette_id == other.palette_id
                && self.plane_origin_f64 == other.plane_origin_f64
                && self.zoom_log2 == other.zoom_log2
                && self.object_angles == other.object_angles
                && self.precision_mode == other.precision_mode;
            selection_matches
                && !super::view_projection_changed(self.view, self.map, other.view, other.map)
        }
    }

    /// What one poll of present's event queue said about this turn.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    struct ObservedEvents {
        presented: bool,
        refused: bool,
        cancelled: bool,
    }

    /// Browser-only owner of the heap, kernels, worker endpoint, presenter, and frame schedule.
    pub struct BrowserFrameLoop {
        device: std::sync::Arc<wgpu::Device>,
        queue: std::sync::Arc<wgpu::Queue>,
        executor: GpuKernelExecutor,
        kernels: JulibrotKernels,
        presenter: Presenter,
        owner_endpoint: OwnerEndpoint,
        _producer_endpoint: ProducerEndpoint,
        orbits: OrbitRegistry<RegisteredOrbit>,
        current_orbit: Option<OrbitHandle>,
        accepted_reference: Option<BigCentre>,
        shallow_centre: Option<BigCentre>,
        accepted_reference_zoom_log2: Option<f64>,
        accepted_reference_receipt: Option<AcceptedReferenceReceipt>,
        requested_plane: Plane,
        /// Centre-minus-reference displacement of the latest HOT drain, in requested-extent pixels.
        centre_from_reference_px: [f64; 2],
        sampled_references: u32,
        sampled_request_at_length: Option<u32>,
        sampled_reference_rounds: u32,
        sampled_reference_discards: u32,
        sampled_resume_level: Option<RefinementLevel>,
        submitted_references: Vec<SubmittedReference>,
        reference_upload: Vec<u8>,
        plan: RefinementPlan,
        grid: EscapeGrid,
        spare_grid: Option<EscapeGrid>,
        grid_round: u64,
        backdrop: Option<BackdropGrid>,
        active_backdrop_map: Option<PoseMap>,
        coverage_turn: super::CoverageTurn,
        main: ember_julibrot_worker::MainState,
        scene_selection: Option<SceneSelection>,
        loop_state: FrameLoop,
        prepared_level: Option<ember_julibrot_kernels::RefinementLevel>,
        hot_stride: u32,
        refresh_id: u64,
        owner_epoch: u64,
        frame_policy: FramePolicyTracker,
        last_dispatch: Option<DispatchFacts>,
        last_warp_source: Option<u64>,
        pending_warp_view: Option<(u64, ViewStamp)>,
        presented_view: Option<ViewStamp>,
        last_status: RefreshStatus,
        level_timings: LevelTimingLedger,
        precision_mode: PrecisionMode,
        horizon_pixels: u64,
        horizon_fraction: f64,
        uncertain_pixels: u64,
        uncertain_fraction: f64,
        map_condition_number: f64,
        edge_on: bool,
        facts_pose: (PoseMap, [u32; 2]),
    }

    impl BrowserFrameLoop {
        /// Constructs every fixed GPU resource and starts the initial worker request.
        ///
        /// # Errors
        ///
        /// Returns a typed heap, kernel, present, worker, or arithmetic refusal.
        pub fn new(
            runtime: &BrowserRuntime,
            viewer: &mut ViewerController,
        ) -> Result<Self, AppError> {
            let precision_mode = viewer.requested().precision_mode;
            Self::new_with_mode(runtime, viewer, precision_mode)
        }

        /// Constructs the browser loop under one explicit precision policy.
        ///
        /// # Errors
        ///
        /// Returns the same typed initialization refusals as [`Self::new`].
        pub fn new_with_mode(
            runtime: &BrowserRuntime,
            viewer: &mut ViewerController,
            precision_mode: PrecisionMode,
        ) -> Result<Self, AppError> {
            let device = runtime.device();
            let queue = runtime.queue();
            let mut executor = GpuKernelExecutor::new(
                device.clone(),
                queue.clone(),
                GpuKernelExecutorConfig {
                    heap_side: HEAP_SIDE,
                    heap_layers: HEAP_LAYERS,
                    descriptor_capacity: DESCRIPTOR_CAPACITY,
                    span_capacity: SPAN_CAPACITY,
                    directory_binding_bytes: DIRECTORY_BYTES,
                    scratch_layers: 4,
                    max_header_pages: MAX_HEADER_PAGES,
                    max_header_sets: MAX_HEADER_SETS,
                    kernel_uniform_bytes: KERNEL_UNIFORM_BYTES,
                },
            )
            .map_err(heap_error)?;
            let kernels = JulibrotKernels::new(&mut executor).map_err(kernel_error)?;
            let requested = viewer.requested();
            let extent = GridExtent {
                width: runtime.facts().width,
                height: runtime.facts().height,
            };
            let params = EscapeParams::new(requested.iteration_cap);
            let mut plan =
                JulibrotKernels::plan_grid_pair(&executor, extent, params).map_err(kernel_error)?;
            let mut reference_upload = Vec::new();
            reference_upload
                .try_reserve_exact(super::reference_texel_bytes(requested.iteration_cap)?)
                .map_err(|error| {
                    AppError::Worker(format!("reference upload reserve failed: {error}"))
                })?;
            let mut loop_state = FrameLoop::default();
            let mut applied_precision_mode = PrecisionMode::Deterministic;
            super::apply_precision_mode(
                precision_mode,
                &mut applied_precision_mode,
                &mut loop_state,
                &mut plan,
                viewer,
            )?;
            let accepted_reference = viewer
                .owner()
                .reference_centre()
                .ok_or_else(|| AppError::Worker("owner navigation is unconfigured".to_string()))?;
            let mut kernels = kernels;
            let [grid, spare_grid] = kernels
                .allocate_grid_pair(&mut executor, &plan)
                .map_err(kernel_error)?;
            let config = PresentConfig {
                surface_format: runtime.surface_format(),
                min_uniform_buffer_offset_alignment: device
                    .limits()
                    .min_uniform_buffer_offset_alignment,
                fence_deadline_ms: PresentConfig::V1_FENCE_DEADLINE_MS,
                max_fence_polls: PresentConfig::V1_MAX_FENCE_POLLS,
            };
            let hot_stride = hot_stride(config.min_uniform_buffer_offset_alignment)
                .map_err(|error| AppError::Present(error.to_string()))?;
            let mut presenter = Presenter::new(
                device.clone(),
                queue.clone(),
                executor.present_resources(),
                config,
            )
            .map_err(present_error)?;
            let mut main = viewer.owner().snapshot().main;
            main.delivered_iter_cap = super::published_iteration_cap(&plan);
            let map = viewer.screen_map([grid.width, grid.height])?;
            let plane = viewer.checked_plane();
            presenter.set_main(PresentMain {
                epoch: 0,
                state: super::main_for_grid(main, grid.width, plan.requested_extent.width),
                grid: grid.clone(),
                object: requested.object_angles,
                plane,
                map,
                backdrop: None,
            });
            let (owner_endpoint, producer_endpoint) = WorkerChannel::new(
                WorkerConfig {
                    max_iter: PAGE_MAX_ITERATION_CAP,
                },
                WorkerMode::WebWorker,
            )
            .map_err(worker_error)?;
            let grid_extent = [grid.width, grid.height];
            let initial_horizon = super::horizon_facts(map, grid_extent);
            let frame_loop = Self {
                device,
                queue,
                executor,
                kernels,
                presenter,
                owner_endpoint,
                _producer_endpoint: producer_endpoint,
                orbits: OrbitRegistry::new(),
                current_orbit: None,
                accepted_reference: Some(accepted_reference),
                shallow_centre: None,
                accepted_reference_zoom_log2: None,
                accepted_reference_receipt: None,
                requested_plane: plane,
                centre_from_reference_px: [0.0; 2],
                sampled_references: 0,
                sampled_request_at_length: None,
                sampled_reference_rounds: 0,
                sampled_reference_discards: 0,
                sampled_resume_level: None,
                submitted_references: Vec::with_capacity(2),
                reference_upload,
                plan,
                grid,
                spare_grid: Some(spare_grid),
                grid_round: loop_state.ladder_round(),
                backdrop: None,
                active_backdrop_map: None,
                coverage_turn: super::CoverageTurn::Backdrop,
                main,
                scene_selection: None,
                loop_state,
                prepared_level: None,
                hot_stride,
                refresh_id: 0,
                owner_epoch: 0,
                frame_policy: FramePolicyTracker::new(),
                last_dispatch: None,
                last_warp_source: None,
                pending_warp_view: None,
                presented_view: None,
                last_status: RefreshStatus::Waiting,
                level_timings: LevelTimingLedger::default(),
                precision_mode: applied_precision_mode,
                horizon_pixels: initial_horizon.pixels,
                horizon_fraction: initial_horizon.fraction,
                uncertain_pixels: initial_horizon.uncertain_pixels,
                uncertain_fraction: initial_horizon.uncertain_fraction,
                map_condition_number: initial_horizon.condition_number,
                edge_on: initial_horizon.edge_on,
                facts_pose: (map, grid_extent),
            };
            Ok(frame_loop)
        }

        /// Executes one bounded refresh and immediate pre-yield completion observation.
        ///
        /// A typed refusal that escapes one turn is latched: the loop stops and every later call
        /// returns the same cause, so the page reports one honest reason instead of restating a
        /// broken invariant sixty times a second. A transient fence refusal never escapes.
        ///
        /// # Errors
        ///
        /// Returns the first typed cross-slice refusal without looping or presenting an unfinished
        /// surface image.
        pub fn refresh(
            &mut self,
            runtime: &mut BrowserRuntime,
            viewer: &mut ViewerController,
            requests: &mut RunRequests,
            now_ms: f64,
        ) -> Result<RefreshOutcome, AppError> {
            if let Some(stopped) = self.loop_state.stopped() {
                return Err(stopped.clone());
            }
            let result = self.refresh_turn(runtime, viewer, requests, now_ms);
            match &result {
                Ok(outcome) => self.last_status = outcome.status,
                Err(error) => {
                    self.loop_state.stop(error.clone());
                    self.last_status = RefreshStatus::FailedTyped;
                }
            }
            result
        }

        fn refresh_turn(
            &mut self,
            runtime: &mut BrowserRuntime,
            viewer: &mut ViewerController,
            requests: &mut RunRequests,
            now_ms: f64,
        ) -> Result<RefreshOutcome, AppError> {
            if !now_ms.is_finite() {
                return Err(AppError::Deadline {
                    operation: "refresh clock",
                    deadline_ms: PresentConfig::V1_FENCE_DEADLINE_MS,
                });
            }
            if let Err(error) = runtime.check_device("Julibrot refresh") {
                let _dropped = runtime.drop_pending_surface();
                return Err(error);
            }
            self.refresh_id = self
                .refresh_id
                .checked_add(1)
                .ok_or(AppError::GenerationExhausted)?;
            let events = FrameLoop::refresh(&mut self.presenter, now_ms);
            let observed = self.handle_events(runtime, viewer, events)?;
            self.synchronize_precision_mode(viewer)?;
            self.requested_plane = viewer.checked_plane();
            let presented = observed.presented;
            if requests.frame {
                let restart_scene = self.scene_ready(viewer.requested().zoom_log2)
                    && self.presented_view_is_stale(viewer);
                self.loop_state
                    .accept_request(self.main.generation_applied, restart_scene);
                requests.frame = false;
            }
            if requests.scene_update {
                requests.scene_update = false;
                self.loop_state
                    .request_scene_update(self.main.generation_applied);
                self.prepared_level = None;
            }
            if let Some(error) = self.owner_endpoint.take_error() {
                self.abandon_submitted_references(viewer);
                return Err(worker_error(error));
            }

            if KernelMode::for_zoom(viewer.requested().zoom_log2) == KernelMode::Shallow {
                self.abandon_submitted_references(viewer);
            }
            let backdrop_active = self.prepare_backdrop(viewer)?;
            let final_validation = if backdrop_active {
                false
            } else {
                self.prepare_due_level()
            };
            let extent = self.prepared_extent();
            let mut hot = viewer.drain_hot(extent)?;
            self.owner_epoch = hot.state.epoch;
            self.main = hot.state.main;
            self.centre_from_reference_px = hot.state.hot.centre_from_reference_px;
            self.observe_scene_selection(viewer);
            if !self.loop_state.refinement_pending() && self.presented_view_is_stale(viewer) {
                self.loop_state.scene_changed(self.main.generation_applied);
                self.prepared_level = None;
                self.prepare_due_level();
                // Re-selecting the level changes the prepared extent, and the drained pose is
                // expressed in that extent's pixels: the screen map and centre_from_reference_px
                // both rescale with it, so the first drain no longer describes this scene.
                hot = viewer.drain_hot(self.prepared_extent())?;
                self.owner_epoch = hot.state.epoch;
                self.main = hot.state.main;
                self.centre_from_reference_px = hot.state.hot.centre_from_reference_px;
            }
            self.install_main(viewer, hot.pose.object, hot.plane, hot.pose.map);
            let mut slot = HotSlot::for_refresh(self.refresh_id, self.hot_stride, hot.state.epoch)
                .map_err(|error| AppError::Present(error.to_string()))?;
            let measure_validation =
                requests.measurement && self.presenter.facts().completed_scene_id.is_some();
            let validation = if measure_validation {
                requests.measurement = false;
                WarpValidation::Measure
            } else if final_validation {
                WarpValidation::Final
            } else {
                WarpValidation::Ordinary
            };
            let hold_refused_warp = self
                .loop_state
                .hold_refused_warp(self.presenter.facts().completed_scene_id.is_some());
            self.presenter.write_hot(
                slot,
                PresentHot {
                    epoch: hot.state.epoch,
                    state: ember_julibrot_worker::HotState {
                        centre_from_reference_px: hot.pose.centre_from_reference_px,
                        ..hot.state.hot
                    },
                    object: hot.pose.object,
                    plane: hot.plane,
                    view: viewer.requested().view,
                    map: hot.pose.map,
                },
                validation,
                hold_refused_warp,
            );

            let main_arrived = self.service_arrivals(viewer, now_ms)?;
            let shallow_accepted = self.submit_pending_reference(viewer, hot.plane)?;
            if main_arrived || shallow_accepted {
                self.prepare_backdrop(viewer)?;
                self.prepare_due_level();
                hot = viewer.drain_hot(self.prepared_extent())?;
                self.owner_epoch = hot.state.epoch;
                self.main = hot.state.main;
                self.observe_scene_selection(viewer);
                self.install_main(viewer, hot.pose.object, hot.plane, hot.pose.map);
                slot = HotSlot::for_refresh(self.refresh_id, self.hot_stride, hot.state.epoch)
                    .map_err(|error| AppError::Present(error.to_string()))?;
                self.presenter.write_hot(
                    slot,
                    PresentHot {
                        epoch: hot.state.epoch,
                        state: ember_julibrot_worker::HotState {
                            centre_from_reference_px: hot.pose.centre_from_reference_px,
                            ..hot.state.hot
                        },
                        object: hot.pose.object,
                        plane: hot.plane,
                        view: viewer.requested().view,
                        map: hot.pose.map,
                    },
                    WarpValidation::Ordinary,
                    hold_refused_warp,
                );
            }
            if self.active_backdrop_map.is_none()
                && self
                    .loop_state
                    .skip_drafts_for_accepted_warp(self.presenter.accepted_warp_source(slot))
            {
                self.prepared_level = None;
                let final_validation = self.prepare_due_level();
                debug_assert!(final_validation);
                hot = viewer.drain_hot(self.prepared_extent())?;
                self.owner_epoch = hot.state.epoch;
                self.main = hot.state.main;
                self.observe_scene_selection(viewer);
                self.install_main(viewer, hot.pose.object, hot.plane, hot.pose.map);
                slot = HotSlot::for_refresh(self.refresh_id, self.hot_stride, hot.state.epoch)
                    .map_err(|error| AppError::Present(error.to_string()))?;
                self.presenter.write_hot(
                    slot,
                    PresentHot {
                        epoch: hot.state.epoch,
                        state: ember_julibrot_worker::HotState {
                            centre_from_reference_px: hot.pose.centre_from_reference_px,
                            ..hot.state.hot
                        },
                        object: hot.pose.object,
                        plane: hot.plane,
                        view: viewer.requested().view,
                        map: hot.pose.map,
                    },
                    WarpValidation::Final,
                    hold_refused_warp,
                );
            }
            let relief_redraw = self.presenter.accepted_relief_redraw(slot);
            let defer_scene_for_redraw = super::defer_scene_until_relief_redraw(
                relief_redraw,
                self.presented_view_is_stale(viewer),
            );
            let scene_id = if defer_scene_for_redraw && self.active_backdrop_map.is_none() {
                None
            } else {
                self.submit_due_scene(
                    viewer,
                    hot.pose.object,
                    hot.plane,
                    hot.pose.map,
                    hot.pose.centre_from_reference_px,
                    slot,
                    hot.state.epoch,
                    now_ms,
                )?
            };

            let mut warp_id = None;
            let warp_requested = self.loop_state.warp_requested(self.frame_policy.policy());
            let redraw_scene_in_flight = super::hold_redraw_during_scene(
                relief_redraw,
                self.presenter.facts().in_flight_scene_id.is_some(),
            );
            if warp_requested && !runtime.has_pending_surface() && !redraw_scene_in_flight {
                match runtime.acquire_for_warp(self.loop_state.generation()) {
                    Ok(frame) => {
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let receipt = match self.presenter.frame(
                            FrameState {
                                surface_view: &view,
                                canvas_width: runtime.facts().width,
                                canvas_height: runtime.facts().height,
                                refresh_id: self.refresh_id,
                                now_ms,
                            },
                            slot,
                        ) {
                            Ok(receipt) => receipt,
                            Err(error) => {
                                let released =
                                    runtime.release_unsubmitted_warp(self.loop_state.generation());
                                debug_assert!(released, "failed warp must release surface token");
                                return Err(present_error(error));
                            }
                        };
                        warp_id = Some(receipt.warp_id);
                        self.last_warp_source = receipt.source_scene_id;
                        if super::schedule_exposure_fill(
                            &mut self.loop_state,
                            receipt.exposed,
                            self.main.generation_applied,
                        ) {
                            self.prepared_level = None;
                        }
                        if let Err(error) = runtime.retain_for_warp(
                            receipt.warp_id,
                            self.loop_state.generation(),
                            receipt.precision_mode,
                            frame,
                        ) {
                            let _released =
                                runtime.release_unsubmitted_warp(self.loop_state.generation());
                            return Err(error);
                        }
                        self.pending_warp_view = Some((receipt.warp_id, self.view_stamp(viewer)));
                        self.loop_state.warp_submitted();
                    }
                    Err(AppError::SurfaceSkipped { .. }) => {
                        return Ok(self.outcome(
                            None,
                            scene_id,
                            false,
                            RefreshStatus::SkippedTimeout,
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
            let status = if presented {
                RefreshStatus::Presented
            } else if warp_id.is_some() || scene_id.is_some() {
                RefreshStatus::Submitted
            } else if observed.cancelled {
                RefreshStatus::Cancelled
            } else if observed.refused {
                RefreshStatus::Refused
            } else {
                RefreshStatus::Waiting
            };
            Ok(self.outcome(warp_id, scene_id, presented, status))
        }

        fn outcome(
            &self,
            warp_id: Option<u64>,
            scene_id: Option<u64>,
            presented: bool,
            status: RefreshStatus,
        ) -> RefreshOutcome {
            RefreshOutcome {
                epoch: self.main_epoch(),
                generation: self.loop_state.generation(),
                refresh_id: self.refresh_id,
                warp_id,
                scene_id,
                presented,
                status,
                precision_mode: reference::viewer_precision_mode(self.main.precision_mode),
            }
        }

        fn handle_events(
            &mut self,
            runtime: &mut BrowserRuntime,
            viewer: &mut ViewerController,
            events: Vec<PresentEvent>,
        ) -> Result<ObservedEvents, AppError> {
            let mut observed = ObservedEvents::default();
            let mut refusal = None;
            for event in events {
                match event {
                    PresentEvent::SceneCompleted {
                        frame,
                        reference_sample,
                    } => {
                        self.level_timings
                            .complete_scene(frame.scene_id, frame.measurement);
                        let backdrop_completed = self.backdrop.as_mut().is_some_and(|backdrop| {
                            let Some(flight) = backdrop
                                .in_flight
                                .filter(|flight| flight.scene_id == frame.scene_id)
                            else {
                                return false;
                            };
                            backdrop.in_flight = None;
                            backdrop.ready = Some(BackdropReady {
                                stamp: flight.stamp,
                                map: flight.map,
                            });
                            true
                        });
                        if backdrop_completed {
                            self.active_backdrop_map = None;
                        } else if self.loop_state.completed(
                            frame.scene_id,
                            frame.pose.orbit_generation,
                            frame.level,
                        ) {
                            self.prepared_level = None;
                            self.maybe_request_sampled_reference(viewer, &frame, reference_sample);
                        }
                    }
                    PresentEvent::SceneDropped {
                        scene_id,
                        measurement,
                        ..
                    } => {
                        self.level_timings.drop_scene(scene_id, Some(measurement));
                        let backdrop_retired = self.backdrop.as_mut().is_some_and(|backdrop| {
                            if backdrop
                                .in_flight
                                .is_some_and(|flight| flight.scene_id == scene_id)
                            {
                                backdrop.in_flight = None;
                                true
                            } else {
                                false
                            }
                        });
                        if backdrop_retired {
                            self.active_backdrop_map = None;
                        } else if self.loop_state.retired(scene_id) {
                            self.prepared_level = None;
                        }
                    }
                    PresentEvent::WarpCompleted { measurement } => {
                        self.level_timings.complete_warp(measurement);
                        if measurement.sample_class
                            == ember_julibrot_present::SampleClass::ColdWarmUp
                        {
                            self.frame_policy.reset();
                        }
                        self.frame_policy
                            .record(measurement.wall_ms)
                            .map_err(|error| AppError::Present(error.to_string()))?;
                        if runtime.complete_warp(measurement.id) {
                            observed.presented = true;
                            if let Some((warp_id, stamp)) = self.pending_warp_view
                                && warp_id == measurement.id
                            {
                                self.pending_warp_view = None;
                                self.presented_view = Some(stamp);
                            }
                        }
                    }
                    PresentEvent::FenceRefused {
                        kind,
                        id,
                        reason,
                        polls,
                        wall_ms,
                        precision_mode: _,
                    } => {
                        if matches!(kind, SubmissionKind::Scene) {
                            self.level_timings.drop_scene(id, None);
                        }
                        let backdrop_retired = matches!(kind, SubmissionKind::Scene)
                            && self.backdrop.as_mut().is_some_and(|backdrop| {
                                if backdrop
                                    .in_flight
                                    .is_some_and(|flight| flight.scene_id == id)
                                {
                                    backdrop.in_flight = None;
                                    true
                                } else {
                                    false
                                }
                            });
                        if backdrop_retired {
                            self.active_backdrop_map = None;
                        }
                        let outcome = self.loop_state.refused(kind, reason, id, polls, wall_ms);
                        if outcome.retired_scene {
                            self.prepared_level = None;
                        }
                        if matches!(kind, SubmissionKind::Warp) {
                            let _dropped = runtime.refuse_warp(id);
                            if self
                                .pending_warp_view
                                .is_some_and(|pending| pending.0 == id)
                            {
                                self.pending_warp_view = None;
                            }
                        }
                        match outcome.class {
                            RefusalClass::Device => {
                                if refusal.is_none() {
                                    refusal =
                                        Some(super::fence_error(kind, reason, polls, wall_ms));
                                }
                            }
                            RefusalClass::Cancelled => observed.cancelled = true,
                            RefusalClass::Transient => observed.refused = true,
                        }
                    }
                }
            }
            refusal.map_or(Ok(observed), Err)
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn depth_digits(zoom_log2: f64) -> u32 {
        (zoom_log2.max(0.0) * core::f64::consts::LOG10_2)
            .ceil()
            .min(f64::from(u32::MAX)) as u32
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn now_us(now_ms: f64) -> u64 {
        (now_ms.max(0.0) * 1_000.0).floor().min(u64::MAX as f64) as u64
    }

    fn heap_error(error: impl std::fmt::Display) -> AppError {
        AppError::Kernel(error.to_string())
    }

    fn kernel_error(error: ember_julibrot_kernels::KernelError) -> AppError {
        AppError::Kernel(error.to_string())
    }

    fn present_error(error: ember_julibrot_present::PresentError) -> AppError {
        AppError::Present(error.to_string())
    }

    fn worker_error(error: ember_julibrot_worker::ChannelError) -> AppError {
        AppError::Worker(error.to_string())
    }

    fn math_error(error: ember_julibrot_math::MathError) -> AppError {
        AppError::Math(error.to_string())
    }

    fn registry_error(error: RegistryError) -> AppError {
        AppError::Worker(format!("orbit registry refusal: {error:?}"))
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserFrameLoop;

#[cfg(test)]
mod tests;
