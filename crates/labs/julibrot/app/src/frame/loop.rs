//! Cross-slice progressive frame scheduling and browser GPU integration.

#[cfg(any(target_arch = "wasm32", test))]
use super::schedule::{
    FrameLoop, RefusalClass, SceneMode, apply_precision_mode, defer_scene_until_relief_redraw,
    fence_error, hold_redraw_during_scene, schedule_exposure_fill, stamp_scene_level,
    stamped_screen_map, view_projection_changed, warp_submission_due,
};
#[cfg(test)]
use super::schedule::{RefinementSchedule, classify_refusal, stamped_extent};

#[cfg(test)]
use ember_julibrot_kernels::SampleStatus;
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_kernels::{
    DispatchFacts, EscapeGrid, GridExtent, KernelError, KernelMode, RefinementLevel, RefinementPlan,
};
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_math::{
    CentreSplit, EscapeParams, Homography, Plane, PoseMap, PrecisionMode, ScaleSplit,
};
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_present::{FenceRefusal, PresentEvent, PresentEvents, SubmissionKind};
#[cfg(any(target_arch = "wasm32", test))]
use ember_lab_heap::DataSpan;

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
    let reference_verification = match verification {
        ember_julibrot_worker::ReferenceVerification::Deferred => "Deferred",
        ember_julibrot_worker::ReferenceVerification::Stable => "Stable",
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

/// Hands the accepted orbit to the navigation a discarded census correction created.
///
/// The correction asks for a reference at another orbit point on the view already being drawn: its
/// navigation delta is zero, and the owner's exact-edit path still spends one requested generation
/// and one centre revision staging it. Discarding the arrival for not outlasting the accepted orbit
/// therefore leaves that navigation with no reference at all unless the orbit already held is given
/// to it, because a lease still naming the navigation before the correction matches nothing the
/// loop will ask about again. The source generation moves across with the rest: it says the orbit
/// is this navigation's own reference rather than one renewed over a change of view, and no view
/// changed, so the conservative full-cap renewal rule has nothing here to protect.
#[cfg(any(target_arch = "wasm32", test))]
const fn adopt_reference_lease_for_correction(
    lease: &mut ReferenceLeaseIdentity,
    generation: u32,
    centre_revision: u32,
) {
    lease.main_generation = generation;
    lease.source_generation = generation;
    lease.centre_revision = centre_revision;
}

/// Whether an idle ladder has requested-picture work to start.
///
/// Keeps automatic refinement due until the requested Final actually exists.
///
/// An accepted warp may already stamp the requested view while showing retained lower-resolution
/// data. If the scene behind it is retired, view staleness alone cannot restart the ladder. The
/// completed-frame comparison therefore decides automatic recovery independently of what the
/// current surface presents. Manual mode still records only a stale view and waits for Update.
#[cfg(any(target_arch = "wasm32", test))]
const fn stale_view_needs_a_new_scene(
    scene_mode: SceneMode,
    refinement_pending: bool,
    view_stale: bool,
    completed_requested_final: bool,
) -> bool {
    if refinement_pending {
        return false;
    }
    match scene_mode {
        SceneMode::Manual => view_stale,
        SceneMode::Auto => !completed_requested_final,
    }
}

/// Whether a submitted warp puts the requested view on the canvas.
///
/// A held warp draws the last completed picture unmoved, so what reaches the canvas is the view
/// already there and not the view being asked for. Stamping the requested view against a hold
/// makes the loop report the held picture as current, which is the one reading a hold exists to
/// keep honest: the hold is defensible only while the source coverage proof remains valid, and a
/// caller cannot see that the substitution has gone wrong if the stamp says nothing is stale.
#[cfg(any(target_arch = "wasm32", test))]
const fn warp_presents_requested_view(kind: ember_julibrot_present::WarpKind) -> bool {
    !matches!(kind, ember_julibrot_present::WarpKind::HoldStale)
}

/// Names why one census correction bought nothing, for the facts row.
#[cfg(target_arch = "wasm32")]
const DISCARDED_CORRECTION_REASON: &str =
    "census candidate orbit did not outlast the accepted reference";

/// Names a correction whose navigation the gesture moved past before the orbit could return to it.
#[cfg(target_arch = "wasm32")]
const SUPERSEDED_CORRECTION_REASON: &str =
    "census correction was superseded; the newer navigation carries its own reference";

/// Names the one case in which the discarded correction leaves the ladder nothing to wait for.
#[cfg(any(target_arch = "wasm32", test))]
const STRANDED_CORRECTION_REASON: &str =
    "the accepted orbit could not be returned to the census correction's navigation";

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

#[cfg(any(target_arch = "wasm32", test))]
struct CaptureDrained;

#[cfg(any(target_arch = "wasm32", test))]
struct CaptureStaged;

#[cfg(any(target_arch = "wasm32", test))]
struct FencesObserved;

#[cfg(any(target_arch = "wasm32", test))]
struct HotWritten;

#[cfg(any(target_arch = "wasm32", test))]
struct SceneConsidered;

/// Effects performed by one ordered refresh turn.
#[cfg(any(target_arch = "wasm32", test))]
trait OrderedRefresh {
    type Error;
    type Output;

    fn drain_capture(&mut self) -> Result<(), Self::Error>;
    fn stage_capture(&mut self, stage: CaptureDrained) -> Result<(), Self::Error>;
    fn observe_fences(&mut self, stage: CaptureStaged) -> Result<(), Self::Error>;
    fn write_hot(&mut self, stage: FencesObserved) -> Result<(), Self::Error>;
    fn consider_scene(&mut self, stage: HotWritten) -> Result<(), Self::Error>;
    fn consider_warp(&mut self, stage: SceneConsidered) -> Result<Self::Output, Self::Error>;
}

/// Performs the externally visible stages of one refresh in their required order.
#[cfg(any(target_arch = "wasm32", test))]
fn execute_ordered_refresh<R: OrderedRefresh>(refresh: &mut R) -> Result<R::Output, R::Error> {
    refresh.drain_capture()?;
    refresh.stage_capture(CaptureDrained)?;
    refresh.observe_fences(CaptureStaged)?;
    refresh.write_hot(FencesObserved)?;
    refresh.consider_scene(HotWritten)?;
    refresh.consider_warp(SceneConsidered)
}

/// Every value argument to one acquired surface warp submission.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct SurfaceWarpJob {
    generation: u32,
    canvas_extent: [u32; 2],
    refresh_id: u64,
    now_ms: f64,
    slot: ember_julibrot_present::HotSlot,
}

/// Every value argument to one terminal surface resolution.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum SurfaceResolutionEvent {
    WarpCompleted {
        measurement: ember_julibrot_present::SubmissionMeasurement,
        capture: crate::CaptureArming,
    },
    WarpRefused {
        kind: SubmissionKind,
        id: u64,
        reason: FenceRefusal,
        polls: u32,
        wall_ms: f64,
        precision_mode: &'static str,
    },
    DeviceFailed,
}

#[cfg(any(target_arch = "wasm32", test))]
impl SurfaceResolutionEvent {
    const fn surface_event(self) -> crate::surface::SurfaceEvent {
        match self {
            Self::WarpCompleted { measurement, .. } => {
                crate::surface::SurfaceEvent::WarpCompleted {
                    warp_id: measurement.id,
                }
            }
            Self::WarpRefused {
                id,
                reason,
                polls,
                wall_ms,
                precision_mode,
                ..
            } => {
                let _ = (reason, polls, wall_ms.to_bits(), precision_mode);
                crate::surface::SurfaceEvent::WarpRefused { warp_id: id }
            }
            Self::DeviceFailed => crate::surface::SurfaceEvent::DeviceFailed,
        }
    }

    #[cfg(test)]
    const fn warp_id(self) -> Option<u64> {
        match self {
            Self::WarpCompleted { measurement, .. } => Some(measurement.id),
            Self::WarpRefused { id, .. } => Some(id),
            Self::DeviceFailed => None,
        }
    }

    const fn expects_present(self) -> bool {
        matches!(self, Self::WarpCompleted { .. })
    }

    const fn valid(self) -> bool {
        !matches!(self, Self::WarpRefused { kind, .. } if !matches!(kind, SubmissionKind::Warp))
    }
}

/// Result of considering one surface-backed warp submission.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum SurfaceSubmission {
    Occupied,
    Submitted(ember_julibrot_present::FrameReceipt),
}

/// Terminal surface action independent of its concrete frame value.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SurfaceResolutionAction {
    Present,
    Drop,
    #[default]
    Ignore,
}

/// Stable effect of resolving one terminal surface event.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SurfaceResolutionEffect {
    #[cfg(test)]
    action: SurfaceResolutionAction,
    presented: bool,
}

/// App-local lowering used by the replayable surface-resolution transaction owner.
#[cfg(any(target_arch = "wasm32", test))]
trait SurfacePort {
    type Error;
    type Frame;

    fn pending(&self) -> bool;
    fn acquire(&mut self, generation: u32) -> Result<Self::Frame, Self::Error>;
    fn submit(
        &mut self,
        frame: &Self::Frame,
        job: &SurfaceWarpJob,
    ) -> Result<ember_julibrot_present::FrameReceipt, Self::Error>;
    fn retain(
        &mut self,
        frame: Self::Frame,
        generation: u32,
        receipt: &ember_julibrot_present::FrameReceipt,
    ) -> Result<(), Self::Error>;
    fn release_unsubmitted(&mut self, generation: u32) -> bool;
    fn resolve(&mut self, event: crate::surface::SurfaceEvent)
    -> crate::SurfaceAction<Self::Frame>;
    fn present(&mut self, event: &SurfaceResolutionEvent, frame: Self::Frame);
    fn drop_frame(&mut self, frame: Self::Frame);
    fn invalid_receipt(&self, detail: &'static str) -> Self::Error;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceResolutionFailure {
    Acquire,
    Submit,
    Retain,
    InvalidReceipt,
    InvalidAction,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum SurfaceResolutionOutcome {
    Occupied,
    Submitted(Box<ember_julibrot_present::FrameReceipt>),
    Presented { warp_id: u64 },
    Dropped { warp_id: Option<u64> },
    Ignored { warp_id: Option<u64> },
    Failed(SurfaceResolutionFailure),
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
enum SurfaceResolutionTransaction {
    Submit(SurfaceWarpJob),
    Resolve(SurfaceResolutionEvent),
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
struct SurfaceResolutionRecord {
    transaction: SurfaceResolutionTransaction,
    outcome: SurfaceResolutionOutcome,
}

/// Chronological surface-resolution record for one refresh turn.
#[cfg(test)]
#[derive(Clone, Debug, Default, PartialEq)]
struct SurfaceResolutionTurn {
    now_ms_bits: u64,
    records: Vec<SurfaceResolutionRecord>,
}

/// Owns surface acquisition, warp publication, terminal matching, and present/drop order.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Default)]
struct SurfaceResolutionOwner {
    #[cfg(test)]
    turn: SurfaceResolutionTurn,
}

#[cfg(any(target_arch = "wasm32", test))]
impl SurfaceResolutionOwner {
    const fn new(now_ms: f64) -> Self {
        #[cfg(not(test))]
        let _ = now_ms;
        Self {
            #[cfg(test)]
            turn: SurfaceResolutionTurn {
                now_ms_bits: now_ms.to_bits(),
                records: Vec::new(),
            },
        }
    }

    fn submit<P: SurfacePort>(
        &mut self,
        port: &mut P,
        job: SurfaceWarpJob,
    ) -> Result<SurfaceSubmission, P::Error> {
        if port.pending() {
            #[cfg(test)]
            self.record(
                SurfaceResolutionTransaction::Submit(job),
                SurfaceResolutionOutcome::Occupied,
            );
            return Ok(SurfaceSubmission::Occupied);
        }
        let frame = match port.acquire(job.generation) {
            Ok(frame) => frame,
            Err(error) => {
                #[cfg(test)]
                self.record(
                    SurfaceResolutionTransaction::Submit(job),
                    SurfaceResolutionOutcome::Failed(SurfaceResolutionFailure::Acquire),
                );
                return Err(error);
            }
        };
        let receipt = match port.submit(&frame, &job) {
            Ok(receipt) => receipt,
            Err(error) => {
                let released = port.release_unsubmitted(job.generation);
                debug_assert!(released, "failed warp must release surface token");
                #[cfg(test)]
                self.record(
                    SurfaceResolutionTransaction::Submit(job),
                    SurfaceResolutionOutcome::Failed(SurfaceResolutionFailure::Submit),
                );
                return Err(error);
            }
        };
        if receipt.refresh_id != job.refresh_id || receipt.warp_id == 0 {
            let released = port.release_unsubmitted(job.generation);
            debug_assert!(released, "invalid warp receipt must release surface token");
            #[cfg(test)]
            self.record(
                SurfaceResolutionTransaction::Submit(job),
                SurfaceResolutionOutcome::Failed(SurfaceResolutionFailure::InvalidReceipt),
            );
            return Err(port.invalid_receipt(
                "surface warp receipt does not preserve its transaction identity",
            ));
        }
        if let Err(error) = port.retain(frame, job.generation, &receipt) {
            let released = port.release_unsubmitted(job.generation);
            debug_assert!(released, "failed retain must release surface token");
            #[cfg(test)]
            self.record(
                SurfaceResolutionTransaction::Submit(job),
                SurfaceResolutionOutcome::Failed(SurfaceResolutionFailure::Retain),
            );
            return Err(error);
        }
        #[cfg(test)]
        self.record(
            SurfaceResolutionTransaction::Submit(job),
            SurfaceResolutionOutcome::Submitted(Box::new(receipt.clone())),
        );
        Ok(SurfaceSubmission::Submitted(receipt))
    }

    fn resolve<P: SurfacePort>(
        &mut self,
        port: &mut P,
        transaction: SurfaceResolutionEvent,
    ) -> Result<SurfaceResolutionEffect, P::Error> {
        if !transaction.valid() {
            #[cfg(test)]
            self.record(
                SurfaceResolutionTransaction::Resolve(transaction),
                SurfaceResolutionOutcome::Failed(SurfaceResolutionFailure::InvalidReceipt),
            );
            return Err(port.invalid_receipt("surface refusal receipt has a non-warp kind"));
        }
        #[cfg(test)]
        let warp_id = transaction.warp_id();
        let action = port.resolve(transaction.surface_event());
        let effect = match action {
            crate::SurfaceAction::Present(frame) if transaction.expects_present() => {
                port.present(&transaction, frame);
                #[cfg(test)]
                self.record(
                    SurfaceResolutionTransaction::Resolve(transaction),
                    SurfaceResolutionOutcome::Presented {
                        warp_id: warp_id.unwrap_or_default(),
                    },
                );
                SurfaceResolutionEffect {
                    #[cfg(test)]
                    action: SurfaceResolutionAction::Present,
                    presented: true,
                }
            }
            crate::SurfaceAction::Drop(frame) if !transaction.expects_present() => {
                port.drop_frame(frame);
                #[cfg(test)]
                self.record(
                    SurfaceResolutionTransaction::Resolve(transaction),
                    SurfaceResolutionOutcome::Dropped { warp_id },
                );
                SurfaceResolutionEffect {
                    #[cfg(test)]
                    action: SurfaceResolutionAction::Drop,
                    presented: false,
                }
            }
            crate::SurfaceAction::Ignore => {
                #[cfg(test)]
                self.record(
                    SurfaceResolutionTransaction::Resolve(transaction),
                    SurfaceResolutionOutcome::Ignored { warp_id },
                );
                SurfaceResolutionEffect {
                    #[cfg(test)]
                    action: SurfaceResolutionAction::Ignore,
                    presented: false,
                }
            }
            crate::SurfaceAction::Present(frame) | crate::SurfaceAction::Drop(frame) => {
                port.drop_frame(frame);
                #[cfg(test)]
                self.record(
                    SurfaceResolutionTransaction::Resolve(transaction),
                    SurfaceResolutionOutcome::Failed(SurfaceResolutionFailure::InvalidAction),
                );
                return Err(
                    port.invalid_receipt("surface action does not match its terminal transaction")
                );
            }
        };
        Ok(effect)
    }

    #[cfg(test)]
    fn record(
        &mut self,
        transaction: SurfaceResolutionTransaction,
        outcome: SurfaceResolutionOutcome,
    ) {
        self.turn.records.push(SurfaceResolutionRecord {
            transaction,
            outcome,
        });
    }

    #[cfg(test)]
    fn take_turn(&mut self) -> SurfaceResolutionTurn {
        std::mem::take(&mut self.turn)
    }
}

/// Variant tag for one app-owned presenter transaction.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PresentEventKind {
    SceneCompleted,
    SceneDropped,
    WarpCompleted,
    FenceRefused,
}

/// Every semantic argument of one completed scene receipt.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct PresentSceneCompletion {
    frame: ember_julibrot_present::SceneFrame,
    reference_sample: Option<u32>,
}

/// Every semantic argument of one dropped scene receipt.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct PresentSceneDrop {
    scene_id: u64,
    orbit_generation: u32,
    reason: ember_julibrot_present::DropReason,
    measurement: ember_julibrot_present::SubmissionMeasurement,
}

/// Every semantic argument of one completed warp receipt.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct PresentWarpCompletion {
    measurement: ember_julibrot_present::SubmissionMeasurement,
}

/// Every semantic argument of one refused fence receipt.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct PresentFenceRefusal {
    kind: SubmissionKind,
    id: u64,
    reason: FenceRefusal,
    polls: u32,
    wall_ms: f64,
    precision_mode: &'static str,
}

/// One presenter event with every semantic argument owned by the app transaction.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct PresentEventTransaction {
    kind: PresentEventKind,
    scene_completion: Option<PresentSceneCompletion>,
    scene_drop: Option<PresentSceneDrop>,
    warp_completion: Option<PresentWarpCompletion>,
    fence_refusal: Option<PresentFenceRefusal>,
}

/// Validated borrowed view of exactly one transaction variant.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy)]
enum PresentEventView<'a> {
    SceneCompleted(&'a PresentSceneCompletion),
    SceneDropped(&'a PresentSceneDrop),
    WarpCompleted(&'a PresentWarpCompletion),
    FenceRefused(&'a PresentFenceRefusal),
}

/// Identity certified by a completed or refused presenter receipt.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PresentEventReceipt {
    kind: SubmissionKind,
    id: u64,
    completion_sequence: Option<u64>,
    orbit_generation: Option<u32>,
    drop_reason: Option<ember_julibrot_present::DropReason>,
    precision_mode: &'static str,
}

#[cfg(any(target_arch = "wasm32", test))]
impl PresentEventTransaction {
    fn from_presenter(event: &PresentEvent) -> Self {
        match event {
            PresentEvent::SceneCompleted {
                frame,
                reference_sample,
            } => Self {
                kind: PresentEventKind::SceneCompleted,
                scene_completion: Some(PresentSceneCompletion {
                    frame: frame.clone(),
                    reference_sample: *reference_sample,
                }),
                scene_drop: None,
                warp_completion: None,
                fence_refusal: None,
            },
            PresentEvent::SceneDropped {
                scene_id,
                orbit_generation,
                reason,
                measurement,
            } => Self {
                kind: PresentEventKind::SceneDropped,
                scene_completion: None,
                scene_drop: Some(PresentSceneDrop {
                    scene_id: *scene_id,
                    orbit_generation: *orbit_generation,
                    reason: *reason,
                    measurement: *measurement,
                }),
                warp_completion: None,
                fence_refusal: None,
            },
            PresentEvent::WarpCompleted { measurement } => Self {
                kind: PresentEventKind::WarpCompleted,
                scene_completion: None,
                scene_drop: None,
                warp_completion: Some(PresentWarpCompletion {
                    measurement: *measurement,
                }),
                fence_refusal: None,
            },
            PresentEvent::FenceRefused {
                kind,
                id,
                reason,
                polls,
                wall_ms,
                precision_mode,
            } => Self {
                kind: PresentEventKind::FenceRefused,
                scene_completion: None,
                scene_drop: None,
                warp_completion: None,
                fence_refusal: Some(PresentFenceRefusal {
                    kind: *kind,
                    id: *id,
                    reason: *reason,
                    polls: *polls,
                    wall_ms: *wall_ms,
                    precision_mode,
                }),
            },
        }
    }

    const fn view(&self) -> Result<PresentEventView<'_>, &'static str> {
        match (
            self.kind,
            &self.scene_completion,
            &self.scene_drop,
            &self.warp_completion,
            &self.fence_refusal,
        ) {
            (PresentEventKind::SceneCompleted, Some(event), None, None, None) => {
                Ok(PresentEventView::SceneCompleted(event))
            }
            (PresentEventKind::SceneDropped, None, Some(event), None, None) => {
                Ok(PresentEventView::SceneDropped(event))
            }
            (PresentEventKind::WarpCompleted, None, None, Some(event), None) => {
                Ok(PresentEventView::WarpCompleted(event))
            }
            (PresentEventKind::FenceRefused, None, None, None, Some(event)) => {
                Ok(PresentEventView::FenceRefused(event))
            }
            _ => Err("present transaction does not contain exactly its tagged event"),
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl PresentEventView<'_> {
    fn receipt(self) -> Result<PresentEventReceipt, &'static str> {
        match self {
            Self::SceneCompleted(event) => {
                if event.frame.measurement.kind != SubmissionKind::Scene {
                    return Err("completed scene receipt has a non-scene kind");
                }
                if event.frame.measurement.id != event.frame.scene_id {
                    return Err("completed scene receipt has a different scene id");
                }
                Ok(PresentEventReceipt {
                    kind: SubmissionKind::Scene,
                    id: event.frame.scene_id,
                    completion_sequence: Some(event.frame.measurement.completion_sequence),
                    orbit_generation: Some(event.frame.pose.orbit_generation),
                    drop_reason: None,
                    precision_mode: event.frame.precision_mode,
                })
            }
            Self::SceneDropped(event) => {
                if event.measurement.kind != SubmissionKind::Scene {
                    return Err("dropped scene receipt has a non-scene kind");
                }
                if event.measurement.id != event.scene_id {
                    return Err("dropped scene receipt has a different scene id");
                }
                Ok(PresentEventReceipt {
                    kind: SubmissionKind::Scene,
                    id: event.scene_id,
                    completion_sequence: Some(event.measurement.completion_sequence),
                    orbit_generation: Some(event.orbit_generation),
                    drop_reason: Some(event.reason),
                    precision_mode: event.measurement.precision_mode,
                })
            }
            Self::WarpCompleted(event) => {
                if event.measurement.kind != SubmissionKind::Warp {
                    return Err("completed warp receipt has a non-warp kind");
                }
                Ok(PresentEventReceipt {
                    kind: SubmissionKind::Warp,
                    id: event.measurement.id,
                    completion_sequence: Some(event.measurement.completion_sequence),
                    orbit_generation: None,
                    drop_reason: None,
                    precision_mode: event.measurement.precision_mode,
                })
            }
            Self::FenceRefused(event) => Ok(PresentEventReceipt {
                kind: event.kind,
                id: event.id,
                completion_sequence: None,
                orbit_generation: None,
                drop_reason: None,
                precision_mode: event.precision_mode,
            }),
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl PresentEventReceipt {
    fn matches(self, event: PresentEventView<'_>) -> bool {
        match event {
            PresentEventView::SceneCompleted(event) => {
                self.kind == SubmissionKind::Scene
                    && self.id == event.frame.scene_id
                    && self.completion_sequence == Some(event.frame.measurement.completion_sequence)
                    && self.orbit_generation == Some(event.frame.pose.orbit_generation)
                    && self.drop_reason.is_none()
                    && self.precision_mode == event.frame.measurement.precision_mode
            }
            PresentEventView::SceneDropped(event) => {
                self.kind == SubmissionKind::Scene
                    && self.id == event.scene_id
                    && self.completion_sequence == Some(event.measurement.completion_sequence)
                    && self.orbit_generation == Some(event.orbit_generation)
                    && self.drop_reason == Some(event.reason)
                    && self.precision_mode == event.measurement.precision_mode
            }
            PresentEventView::WarpCompleted(event) => {
                self.kind == SubmissionKind::Warp
                    && self.id == event.measurement.id
                    && self.completion_sequence == Some(event.measurement.completion_sequence)
                    && self.orbit_generation.is_none()
                    && self.drop_reason.is_none()
                    && self.precision_mode == event.measurement.precision_mode
            }
            PresentEventView::FenceRefused(event) => {
                self.kind == event.kind
                    && self.id == event.id
                    && self.completion_sequence.is_none()
                    && self.orbit_generation.is_none()
                    && self.drop_reason.is_none()
                    && self.precision_mode == event.precision_mode
            }
        }
    }
}

/// Stable app facts produced while one presenter event is applied.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PresentEventEffect {
    presented: bool,
    refused: bool,
    cancelled: bool,
    #[cfg(test)]
    retained_scene_id: Option<u64>,
    #[cfg(test)]
    presented_scene_id: Option<u64>,
}

/// Stable result of the present-event transaction for one refresh turn.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PresentEventFacts {
    presented: bool,
    refused: bool,
    cancelled: bool,
    #[cfg(test)]
    retained_scene_id: Option<u64>,
    #[cfg(test)]
    presented_scene_id: Option<u64>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl PresentEventFacts {
    const fn absorb(&mut self, effect: PresentEventEffect) {
        self.presented |= effect.presented;
        self.refused |= effect.refused;
        self.cancelled |= effect.cancelled;
        #[cfg(test)]
        {
            self.retained_scene_id = effect.retained_scene_id;
            self.presented_scene_id = effect.presented_scene_id;
        }
    }
}

/// One applied receipt in its exact observation order.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
struct PresentEventRecord {
    transaction: PresentEventTransaction,
    receipt: PresentEventReceipt,
    effect: PresentEventEffect,
}

/// Chronological present-event record and stable result for one refresh turn.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, PartialEq)]
struct PresentEventTurn {
    #[cfg(test)]
    now_ms_bits: u64,
    facts: PresentEventFacts,
    #[cfg(test)]
    events: Vec<PresentEventRecord>,
}

/// A retained turn paired with the outcome observed after its events were applied.
#[cfg(any(target_arch = "wasm32", test))]
struct PresentEventObservation<E> {
    turn: PresentEventTurn,
    finish: Result<(), E>,
}

/// Browser or native lowering used by the replayable present-event transaction owner.
#[cfg(any(target_arch = "wasm32", test))]
trait PresentEventPort {
    type Error;

    fn poll(&mut self, now_ms: f64) -> PresentEvents;
    fn scene_completed(
        &mut self,
        event: &PresentSceneCompletion,
    ) -> Result<PresentEventEffect, Self::Error>;
    fn scene_dropped(
        &mut self,
        event: &PresentSceneDrop,
    ) -> Result<PresentEventEffect, Self::Error>;
    fn warp_completed(
        &mut self,
        event: &PresentWarpCompletion,
    ) -> Result<PresentEventEffect, Self::Error>;
    fn fence_refused(
        &mut self,
        event: &PresentFenceRefusal,
    ) -> Result<PresentEventEffect, Self::Error>;
    fn finish(&mut self) -> Result<(), Self::Error>;
    fn invalid_receipt(&self, detail: &'static str) -> Self::Error;
}

/// Owns presenter receipt validation and chronological application for one refresh stage.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default)]
struct PresentEventOwner;

#[cfg(any(target_arch = "wasm32", test))]
impl PresentEventOwner {
    fn observe<P: PresentEventPort>(
        port: &mut P,
        now_ms: f64,
    ) -> PresentEventObservation<P::Error> {
        let transactions = port.poll(now_ms);
        let mut facts = PresentEventFacts::default();
        let mut last_completion_sequence = None;
        #[cfg(test)]
        let mut events = Vec::with_capacity(transactions.len());
        let applied = transactions.into_iter().try_for_each(|event| {
            let transaction = PresentEventTransaction::from_presenter(&event);
            let view = transaction
                .view()
                .map_err(|detail| port.invalid_receipt(detail))?;
            let receipt = view
                .receipt()
                .map_err(|detail| port.invalid_receipt(detail))?;
            if !receipt.matches(view) {
                return Err(port.invalid_receipt(
                    "present receipt does not preserve its transaction metadata",
                ));
            }
            if let Some(sequence) = receipt.completion_sequence {
                if last_completion_sequence.is_some_and(|last| sequence < last) {
                    return Err(port.invalid_receipt(
                        "present receipts are not in chronological completion order",
                    ));
                }
                last_completion_sequence = Some(sequence);
            }
            let effect = match view {
                PresentEventView::SceneCompleted(event) => port.scene_completed(event)?,
                PresentEventView::SceneDropped(event) => port.scene_dropped(event)?,
                PresentEventView::WarpCompleted(event) => port.warp_completed(event)?,
                PresentEventView::FenceRefused(event) => port.fence_refused(event)?,
            };
            facts.absorb(effect);
            #[cfg(test)]
            events.push(PresentEventRecord {
                transaction,
                receipt,
                effect,
            });
            Ok(())
        });
        let turn = PresentEventTurn {
            #[cfg(test)]
            now_ms_bits: now_ms.to_bits(),
            facts,
            #[cfg(test)]
            events,
        };
        let finish = applied.and_then(|()| port.finish());
        PresentEventObservation { turn, finish }
    }
}

/// Which app-selected whole-grid target one kernel transaction serves.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KernelGridTarget {
    Main,
    Backdrop,
}

/// Plain request for one kernels-owned refinement plan.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelPlan {
    target: KernelGridTarget,
    requested_extent: GridExtent,
    requested_max_iter: u32,
    precision_mode: Option<PrecisionMode>,
}

/// Plain result of one refinement-planning transaction.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelPlanning {
    #[cfg(test)]
    request: KernelPlan,
    plan: RefinementPlan,
}

#[cfg(any(target_arch = "wasm32", test))]
impl KernelPlanning {
    const fn into_plan(self) -> RefinementPlan {
        self.plan
    }
}

/// Stable, allocation-free identity for one generation-tagged heap span.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelSpanGeneration {
    directory_index: u32,
    page_count: u32,
    first_generation: u16,
    handle_fingerprint: u64,
}

#[cfg(any(target_arch = "wasm32", test))]
impl KernelSpanGeneration {
    fn from_span(span: &DataSpan) -> Self {
        const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        let mut handle_fingerprint = OFFSET_BASIS;
        for handle in span.handles() {
            handle_fingerprint ^= u64::from(handle.raw());
            handle_fingerprint = handle_fingerprint.wrapping_mul(PRIME);
        }
        Self {
            directory_index: span.directory_index,
            page_count: span.page_count,
            first_generation: span
                .handles()
                .first()
                .map_or(0, |handle| handle.generation()),
            handle_fingerprint,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
trait KernelGridIdentity {
    fn span_generation(&self) -> KernelSpanGeneration;
}

#[cfg(any(target_arch = "wasm32", test))]
impl KernelGridIdentity for EscapeGrid {
    fn span_generation(&self) -> KernelSpanGeneration {
        KernelSpanGeneration::from_span(&self.span)
    }
}

/// Plain result of allocating one or two grid spans.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelAllocation {
    target: KernelGridTarget,
    spans: [Option<KernelSpanGeneration>; 2],
}

/// One allocated grid together with its replayable span identity.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
struct AllocatedKernelGrid<G> {
    grid: G,
    #[cfg(test)]
    transaction: KernelAllocation,
}

#[cfg(any(target_arch = "wasm32", test))]
impl<G> AllocatedKernelGrid<G> {
    fn into_grid(self) -> G {
        self.grid
    }
}

/// One allocated main-grid pair together with both replayable span identities.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
struct AllocatedKernelGridPair<G> {
    grids: [G; 2],
    #[cfg(test)]
    transaction: KernelAllocation,
}

#[cfg(any(target_arch = "wasm32", test))]
impl<G> AllocatedKernelGridPair<G> {
    fn into_grids(self) -> [G; 2] {
        self.grids
    }
}

/// Plain result of retiring one grid span.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelRetirement {
    target: KernelGridTarget,
    span: KernelSpanGeneration,
}

/// Plain identity bound to one borrowed reference span during a deep dispatch.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelReferenceIdentity {
    span: KernelSpanGeneration,
    generation: u32,
    length: u32,
    precision_bits: u32,
    precision_mode: &'static str,
}

/// Plain whole-grid kernel variant carrying every semantic encoding argument by value.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum WholeGridMode {
    Shallow {
        centre: CentreSplit,
        pixel_scale: f32,
    },
    Perturbation {
        centre_from_reference_px: [f64; 2],
        scale: ScaleSplit,
        reference: KernelReferenceIdentity,
    },
}

#[cfg(any(target_arch = "wasm32", test))]
impl WholeGridMode {
    const fn kernel_mode(&self) -> KernelMode {
        match self {
            Self::Shallow { .. } => KernelMode::Shallow,
            Self::Perturbation { .. } => KernelMode::Perturbation,
        }
    }

    const fn orbit_generation(&self) -> Option<u32> {
        match self {
            Self::Shallow { .. } => None,
            Self::Perturbation { reference, .. } => Some(reference.generation),
        }
    }

    #[cfg(test)]
    const fn orbit_length(&self) -> u32 {
        match self {
            Self::Shallow { .. } => 0,
            Self::Perturbation { reference, .. } => reference.length,
        }
    }
}

/// Every value argument to one mapped whole-grid kernel publication.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct WholeGridJob {
    target: KernelGridTarget,
    owner_epoch: u64,
    precision_mode: PrecisionMode,
    level: RefinementLevel,
    requested_extent: GridExtent,
    plane: Plane,
    screen_to_plane: Homography,
    params: EscapeParams,
    mode: WholeGridMode,
}

/// Plain input to one kernel publication.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum KernelJob {
    WholeGrid(WholeGridJob),
}

#[cfg(any(target_arch = "wasm32", test))]
impl KernelJob {
    const fn whole_grid(&self) -> &WholeGridJob {
        match self {
            Self::WholeGrid(job) => job,
        }
    }
}

/// Plain result after encoded kernel work has been submitted to its queue.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct KernelPublication {
    #[cfg(test)]
    job: KernelJob,
    #[cfg(test)]
    span: KernelSpanGeneration,
    facts: DispatchFacts,
}

#[cfg(any(target_arch = "wasm32", test))]
impl KernelPublication {
    const fn into_facts(self) -> DispatchFacts {
        self.facts
    }
}

/// App-local lowering used by the replayable kernel-submission transaction owner.
#[cfg(any(target_arch = "wasm32", test))]
trait KernelSubmissionPort {
    type Grid: KernelGridIdentity;
    type Error;
    type PlanContext<'a>
    where
        Self: 'a;
    type AllocationContext<'a>
    where
        Self: 'a;
    type SubmissionContext<'a>
    where
        Self: 'a;

    fn plan(
        &mut self,
        context: Self::PlanContext<'_>,
        request: KernelPlan,
    ) -> Result<RefinementPlan, Self::Error>;
    fn allocate_grid(
        &mut self,
        context: Self::AllocationContext<'_>,
        plan: &RefinementPlan,
    ) -> Result<Self::Grid, Self::Error>;
    fn allocate_grid_pair(
        &mut self,
        context: Self::AllocationContext<'_>,
        plan: &RefinementPlan,
    ) -> Result<[Self::Grid; 2], Self::Error>;
    fn retire(
        &mut self,
        context: Self::AllocationContext<'_>,
        target: KernelGridTarget,
        expected_span: KernelSpanGeneration,
        grid: Self::Grid,
    ) -> Result<(), Self::Error>;
    #[cfg(target_arch = "wasm32")]
    fn reference_span_generation(&self, span: &DataSpan) -> KernelSpanGeneration;
    fn submit(
        &mut self,
        context: Self::SubmissionContext<'_>,
        grid: &mut Self::Grid,
        job: &KernelJob,
    ) -> Result<DispatchFacts, Self::Error>;
}

/// Owns planning, span lifetime, and queue publication across browser and replay lowerings.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Default)]
struct KernelSubmissionOwner<P> {
    port: P,
}

#[cfg(any(target_arch = "wasm32", test))]
impl<P: KernelSubmissionPort> KernelSubmissionOwner<P> {
    const fn new(port: P) -> Self {
        Self { port }
    }

    fn plan(
        &mut self,
        context: P::PlanContext<'_>,
        request: KernelPlan,
    ) -> Result<KernelPlanning, P::Error> {
        let mut plan = self.port.plan(context, request)?;
        if let Some(precision_mode) = request.precision_mode {
            plan = plan.with_precision_mode(precision_mode);
        }
        Ok(KernelPlanning {
            #[cfg(test)]
            request,
            plan,
        })
    }

    fn allocate_grid(
        &mut self,
        context: P::AllocationContext<'_>,
        #[cfg(test)] target: KernelGridTarget,
        #[cfg(target_arch = "wasm32")] _target: KernelGridTarget,
        plan: &RefinementPlan,
    ) -> Result<AllocatedKernelGrid<P::Grid>, P::Error> {
        let grid = self.port.allocate_grid(context, plan)?;
        #[cfg(test)]
        let transaction = KernelAllocation {
            target,
            spans: [Some(grid.span_generation()), None],
        };
        Ok(AllocatedKernelGrid {
            grid,
            #[cfg(test)]
            transaction,
        })
    }

    fn allocate_grid_pair(
        &mut self,
        context: P::AllocationContext<'_>,
        #[cfg(test)] target: KernelGridTarget,
        #[cfg(target_arch = "wasm32")] _target: KernelGridTarget,
        plan: &RefinementPlan,
    ) -> Result<AllocatedKernelGridPair<P::Grid>, P::Error> {
        let grids = self.port.allocate_grid_pair(context, plan)?;
        #[cfg(test)]
        let transaction = KernelAllocation {
            target,
            spans: [
                Some(grids[0].span_generation()),
                Some(grids[1].span_generation()),
            ],
        };
        Ok(AllocatedKernelGridPair {
            grids,
            #[cfg(test)]
            transaction,
        })
    }

    fn retire(
        &mut self,
        context: P::AllocationContext<'_>,
        target: KernelGridTarget,
        grid: P::Grid,
    ) -> Result<KernelRetirement, P::Error> {
        let transaction = KernelRetirement {
            target,
            span: grid.span_generation(),
        };
        self.port
            .retire(context, transaction.target, transaction.span, grid)?;
        Ok(transaction)
    }

    #[cfg(target_arch = "wasm32")]
    fn reference_identity(
        &self,
        span: &DataSpan,
        generation: u32,
        length: u32,
        precision_bits: u32,
        precision_mode: &'static str,
    ) -> KernelReferenceIdentity {
        KernelReferenceIdentity {
            span: self.port.reference_span_generation(span),
            generation,
            length,
            precision_bits,
            precision_mode,
        }
    }

    fn submit(
        &mut self,
        context: P::SubmissionContext<'_>,
        grid: &mut P::Grid,
        job: &KernelJob,
    ) -> Result<KernelPublication, P::Error> {
        #[cfg(test)]
        let span = grid.span_generation();
        let facts = self.port.submit(context, grid, job)?;
        Ok(KernelPublication {
            #[cfg(test)]
            job: *job,
            #[cfg(test)]
            span,
            facts,
        })
    }
}

/// Plain result of handing one orbit request to the worker service.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkerSubmission {
    generation: u32,
    outcome: ember_julibrot_worker::SubmitOutcome,
}

/// Plain metadata observed when one worker arrival is drained.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkerArrival {
    generation: u32,
    centre_revision: u32,
    length: u32,
    compute_us: u32,
    precision_bits: u32,
    admission_credit_us: u32,
    reference_verification: ember_julibrot_worker::ReferenceVerification,
    max_consumed_word_error_ulps: Option<u32>,
    precision_escalations: u32,
    cancelled: bool,
    records: Result<Vec<u8>, String>,
    response_observed_us: Option<u64>,
    upload_started_us: Option<u64>,
}

/// Plain result of applying one drained arrival and returning its credit.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkerApplication {
    generation: u32,
    centre_revision: u32,
    disposition: ember_julibrot_worker::OrbitDisposition,
    reference_applied: bool,
}

/// Applies one plain arrival to app-owned reference state.
#[cfg(any(target_arch = "wasm32", test))]
trait WorkerAcceptance<R> {
    type Submission;

    fn accept(
        &mut self,
        arrival: &WorkerArrival,
        response: &R,
        submitted: Option<Self::Submission>,
        latest_generation: u32,
    ) -> Result<WorkerApplication, AppError>;
    fn finish_reference_submission(&mut self, generation: u32);
}

/// App-local lowering used by the replayable worker-service transaction owner.
#[cfg(any(target_arch = "wasm32", test))]
trait WorkerServicePort {
    type Response;

    fn submit(&mut self, request: ember_julibrot_worker::OrbitRequest) -> WorkerSubmission;
    fn drain(&mut self) -> Option<WorkerArrival>;
    fn response(&self, generation: u32) -> Option<&Self::Response>;
    fn apply(
        &mut self,
        arrival: WorkerArrival,
        application: WorkerApplication,
        owner_now_us: u64,
    ) -> Result<WorkerApplication, AppError>;
    fn facts(&self) -> ember_julibrot_worker::WorkerFacts;
    fn take_error(&mut self) -> Option<AppError>;
    fn latest_generation(&self) -> u32;
    fn pending_request_depth(&self) -> u32;
    #[cfg(target_arch = "wasm32")]
    fn reserve_reference_upload(&mut self, required: usize) -> Result<(), AppError>;
}

/// Owns worker service transactions independently of their browser or replay lowering.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Default)]
struct WorkerServiceOwner<P> {
    port: P,
}

#[cfg(any(target_arch = "wasm32", test))]
impl<P: WorkerServicePort> WorkerServiceOwner<P> {
    const fn new(port: P) -> Self {
        Self { port }
    }

    fn submit(&mut self, request: ember_julibrot_worker::OrbitRequest) -> WorkerSubmission {
        self.port.submit(request)
    }

    fn drain(&mut self) -> Option<WorkerArrival> {
        self.port.drain()
    }

    fn apply<A: WorkerAcceptance<P::Response>>(
        &mut self,
        acceptance: &mut A,
        arrival: WorkerArrival,
        submitted: Option<A::Submission>,
        owner_now_us: u64,
    ) -> Result<WorkerApplication, AppError> {
        let generation = arrival.generation;
        let centre_revision = arrival.centre_revision;
        let latest_generation = self.latest_generation();
        let processed = self.port.response(generation).map_or_else(
            || {
                Err(AppError::Worker(
                    "worker service lost its drained response".to_string(),
                ))
            },
            |response| acceptance.accept(&arrival, response, submitted, latest_generation),
        );
        acceptance.finish_reference_submission(generation);
        let application = processed.as_ref().map_or(
            WorkerApplication {
                generation,
                centre_revision,
                disposition: ember_julibrot_worker::OrbitDisposition::Stale,
                reference_applied: false,
            },
            |application| *application,
        );
        let credited = self.port.apply(arrival, application, owner_now_us);
        let application = processed?;
        credited?;
        Ok(application)
    }

    fn facts(&self) -> ember_julibrot_worker::WorkerFacts {
        self.port.facts()
    }

    fn take_error(&mut self) -> Option<AppError> {
        self.port.take_error()
    }

    fn latest_generation(&self) -> u32 {
        self.port.latest_generation()
    }

    fn pending_request_depth(&self) -> u32 {
        self.port.pending_request_depth()
    }

    #[cfg(target_arch = "wasm32")]
    fn reserve_reference_upload(&mut self, required: usize) -> Result<(), AppError> {
        self.port.reserve_reference_upload(required)
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
        FenceRefusal, FrameState, HotSlot, PresentBackdrop, PresentConfig, PresentEvents,
        PresentHot, PresentMain, Presenter, SubmissionKind, hot_stride,
    };
    use ember_julibrot_worker::{
        EncodedCentre, OrbitDisposition, OrbitHandle, OrbitRegistry, OrbitRequest, OwnerEndpoint,
        ProducerEndpoint, RegistryError, SubmitOutcome, WorkerChannel, WorkerConfig, WorkerFacts,
        WorkerMode,
    };
    use ember_lab_heap::{DataSpan, GpuKernelExecutor, GpuKernelExecutorConfig};

    use super::{
        AllocatedKernelGrid, BACKDROP_PRESENT_LEVEL, CaptureDrained, CaptureStaged, CoverageTurn,
        FencesObserved, FrameLoop, HotWritten, KernelGridIdentity, KernelGridTarget, KernelJob,
        KernelPlan, KernelSpanGeneration, KernelSubmissionOwner, KernelSubmissionPort,
        OrderedRefresh, PAGE_MAX_ITERATION_CAP, PresentEventEffect, PresentEventFacts,
        PresentEventOwner, PresentEventPort, PresentFenceRefusal, PresentSceneCompletion,
        PresentSceneDrop, PresentWarpCompletion, RefusalClass, SceneConsidered, SceneMode,
        SurfacePort, SurfaceResolutionEvent, SurfaceResolutionOwner, SurfaceSubmission,
        SurfaceWarpJob, WholeGridJob, WholeGridMode, WorkerAcceptance, WorkerApplication,
        WorkerArrival, WorkerServiceOwner, WorkerServicePort, WorkerSubmission, backdrop_extent,
        coverage_pre_empts, execute_ordered_refresh, horizon_facts, main_for_grid,
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

    struct BrowserKernelSubmission {
        kernels: JulibrotKernels,
    }

    struct BrowserKernelDispatch<'a> {
        executor: &'a GpuKernelExecutor,
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        reference_span: Option<&'a DataSpan>,
    }

    impl KernelSubmissionPort for BrowserKernelSubmission {
        type Grid = EscapeGrid;
        type Error = KernelError;
        type PlanContext<'a>
            = &'a GpuKernelExecutor
        where
            Self: 'a;
        type AllocationContext<'a>
            = &'a mut GpuKernelExecutor
        where
            Self: 'a;
        type SubmissionContext<'a>
            = BrowserKernelDispatch<'a>
        where
            Self: 'a;

        fn plan(
            &mut self,
            executor: Self::PlanContext<'_>,
            request: KernelPlan,
        ) -> Result<RefinementPlan, Self::Error> {
            let params = EscapeParams::new(request.requested_max_iter);
            match request.target {
                KernelGridTarget::Main => {
                    JulibrotKernels::plan_grid_pair(executor, request.requested_extent, params)
                }
                KernelGridTarget::Backdrop => {
                    JulibrotKernels::plan(executor, request.requested_extent, params)
                }
            }
        }

        fn allocate_grid(
            &mut self,
            executor: Self::AllocationContext<'_>,
            plan: &RefinementPlan,
        ) -> Result<Self::Grid, Self::Error> {
            self.kernels.allocate_grid(executor, plan)
        }

        fn allocate_grid_pair(
            &mut self,
            executor: Self::AllocationContext<'_>,
            plan: &RefinementPlan,
        ) -> Result<[Self::Grid; 2], Self::Error> {
            self.kernels.allocate_grid_pair(executor, plan)
        }

        fn retire(
            &mut self,
            executor: Self::AllocationContext<'_>,
            _target: KernelGridTarget,
            expected_span: KernelSpanGeneration,
            grid: Self::Grid,
        ) -> Result<(), Self::Error> {
            debug_assert_eq!(expected_span, grid.span_generation());
            self.kernels.free_grid(executor, grid)
        }

        fn reference_span_generation(&self, span: &DataSpan) -> KernelSpanGeneration {
            KernelSpanGeneration::from_span(span)
        }

        fn submit(
            &mut self,
            dispatch: Self::SubmissionContext<'_>,
            grid: &mut Self::Grid,
            job: &KernelJob,
        ) -> Result<DispatchFacts, Self::Error> {
            let job = job.whole_grid();
            let label = match job.target {
                KernelGridTarget::Main => "Julibrot kernels SCRATCH and DATA copy",
                KernelGridTarget::Backdrop => "Julibrot backdrop kernels SCRATCH and DATA copy",
            };
            let mut encoder = dispatch
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
            let facts = match job.mode {
                WholeGridMode::Shallow {
                    centre,
                    pixel_scale,
                } => self.kernels.encode_shallow(
                    dispatch.executor,
                    &mut encoder,
                    grid,
                    job.owner_epoch,
                    job.precision_mode,
                    job.level,
                    &job.plane,
                    &job.screen_to_plane,
                    &centre,
                    pixel_scale,
                    job.params,
                )?,
                WholeGridMode::Perturbation {
                    centre_from_reference_px,
                    scale,
                    reference,
                } => {
                    let span = dispatch
                        .reference_span
                        .ok_or(KernelError::MissingReference)?;
                    if reference.span != KernelSpanGeneration::from_span(span) {
                        return Err(KernelError::StaleReference);
                    }
                    self.kernels.encode_perturbation(
                        dispatch.executor,
                        &mut encoder,
                        grid,
                        job.owner_epoch,
                        job.precision_mode,
                        job.level,
                        &job.plane,
                        &job.screen_to_plane,
                        centre_from_reference_px,
                        scale,
                        job.params,
                        ReferenceOrbitInput {
                            span,
                            generation: reference.generation,
                            length: reference.length,
                            precision_bits: reference.precision_bits,
                            precision_mode: reference.precision_mode,
                        },
                    )?
                }
            };
            debug_assert_eq!(facts.mode, job.mode.kernel_mode());
            debug_assert_eq!(facts.requested_extent, job.requested_extent);
            debug_assert_eq!(facts.requested_max_iter, job.params.max_iter);
            debug_assert_eq!(facts.orbit_generation, job.mode.orbit_generation());
            dispatch.queue.submit([encoder.finish()]);
            Ok(facts)
        }
    }

    struct ChannelWorkerService {
        owner_endpoint: OwnerEndpoint,
        _producer_endpoint: ProducerEndpoint,
        pending_response: Option<ember_julibrot_worker::OrbitResponseView>,
        reference_upload: Vec<u8>,
    }

    impl WorkerServicePort for ChannelWorkerService {
        type Response = ember_julibrot_worker::OrbitResponseView;

        fn submit(&mut self, request: OrbitRequest) -> WorkerSubmission {
            let generation = request.generation();
            let outcome = self.owner_endpoint.submit(request);
            WorkerSubmission {
                generation,
                outcome,
            }
        }

        fn drain(&mut self) -> Option<WorkerArrival> {
            debug_assert!(self.pending_response.is_none());
            let response = self.owner_endpoint.next_arrival()?;
            let response_observed_us = reference::monotonic_now_us();
            let upload_started_us = reference::monotonic_now_us();
            let records = if response.cancelled() {
                self.reference_upload.clear();
                Ok(std::mem::take(&mut self.reference_upload))
            } else {
                response
                    .records
                    .transfer_record_bytes()
                    .map_err(|error| error.to_string())
                    .and_then(|records| {
                        reference::expand_reference_texels_from_array(
                            &records,
                            response.length(),
                            &mut self.reference_upload,
                        )
                    })
                    .map(|()| std::mem::take(&mut self.reference_upload))
            };
            let arrival = WorkerArrival {
                generation: response.generation(),
                centre_revision: response.centre_revision(),
                length: response.length(),
                compute_us: response.compute_us(),
                precision_bits: response.precision_bits(),
                admission_credit_us: response.admission_credit_us(),
                reference_verification: response.reference_verification(),
                max_consumed_word_error_ulps: response.max_consumed_word_error_ulps(),
                precision_escalations: response.precision_escalations(),
                cancelled: response.cancelled(),
                records,
                response_observed_us,
                upload_started_us,
            };
            self.pending_response = Some(response);
            Some(arrival)
        }

        fn response(&self, generation: u32) -> Option<&Self::Response> {
            self.pending_response
                .as_ref()
                .filter(|response| response.generation() == generation)
        }

        fn apply(
            &mut self,
            arrival: WorkerArrival,
            application: WorkerApplication,
            owner_now_us: u64,
        ) -> Result<WorkerApplication, AppError> {
            let mut response = self.pending_response.take().ok_or_else(|| {
                AppError::Worker("worker service has no response to apply".to_string())
            })?;
            let generation_matches = response.generation() == application.generation;
            let disposition = if generation_matches {
                application.disposition
            } else {
                OrbitDisposition::Stale
            };
            let credited = self
                .owner_endpoint
                .return_credit(&mut response, disposition, owner_now_us)
                .map_err(worker_error);
            if let Ok(mut records) = arrival.records {
                records.clear();
                self.reference_upload = records;
            }
            credited?;
            if !generation_matches {
                return Err(AppError::Worker(
                    "worker service applied a different generation".to_string(),
                ));
            }
            Ok(application)
        }

        fn facts(&self) -> WorkerFacts {
            self.owner_endpoint.facts()
        }

        fn take_error(&mut self) -> Option<AppError> {
            self.owner_endpoint.take_error().map(worker_error)
        }

        fn latest_generation(&self) -> u32 {
            self.owner_endpoint.latest_generation()
        }

        fn pending_request_depth(&self) -> u32 {
            self.owner_endpoint.pending_request_depth()
        }

        #[cfg(target_arch = "wasm32")]
        fn reserve_reference_upload(&mut self, required: usize) -> Result<(), AppError> {
            if self.reference_upload.capacity() < required {
                self.reference_upload
                    .try_reserve_exact(required.saturating_sub(self.reference_upload.len()))
                    .map_err(|error| {
                        AppError::Worker(format!("reference upload reserve failed: {error}"))
                    })?;
            }
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct SceneSelection {
        generation: u32,
        requested_iter_cap: u32,
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

    #[derive(Debug)]
    struct MainGridPair {
        current: EscapeGrid,
        spare: EscapeGrid,
    }

    impl ViewStamp {
        fn render_equivalent(self, other: Self) -> bool {
            let selection_matches = self.generation_applied == other.generation_applied
                && self.centre_revision == other.centre_revision
                && self.requested_iter_cap == other.requested_iter_cap
                && self.plane_origin_f64 == other.plane_origin_f64
                && self.zoom_log2 == other.zoom_log2
                && self.object_angles == other.object_angles
                && self.precision_mode == other.precision_mode;
            selection_matches
                && !super::view_projection_changed(self.view, self.map, other.view, other.map)
        }
    }

    /// One requested copy of the presented frame, and what happened to the request.
    ///
    /// The copy is on request only and never per frame: it costs a full surface-sized transfer,
    /// 2,073,600 bytes at 960 by 540, and a frame loop that paid that every turn would be measuring
    /// its own readback rather than the picture.
    #[derive(Debug)]
    struct FrameCapture {
        armed: bool,
        route: ember_julibrot_present::FrameReadbackRoute,
        ready: Option<ember_julibrot_present::FrameReadback>,
        refusal: Option<String>,
        /// When the copy in flight was handed to the renderer, so a map that never fires can be
        /// abandoned with a reason instead of holding its buffer and the loop forever.
        in_flight_since_ms: Option<f64>,
    }

    /// Browser-only owner of the heap, kernels, worker endpoint, presenter, and frame schedule.
    pub struct BrowserFrameLoop {
        device: std::sync::Arc<wgpu::Device>,
        queue: std::sync::Arc<wgpu::Queue>,
        executor: GpuKernelExecutor,
        kernel_submission: KernelSubmissionOwner<BrowserKernelSubmission>,
        presenter: Presenter,
        worker_service: Option<WorkerServiceOwner<ChannelWorkerService>>,
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
        sampled_reference_refusal: Option<&'static str>,
        sampled_resume_level: Option<RefinementLevel>,
        submitted_references: Vec<SubmittedReference>,
        plan: RefinementPlan,
        main_grid_pair: Option<MainGridPair>,
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
        /// The scene the image now on the canvas was warped from.
        ///
        /// Recorded when a warp is PRESENTED, not when one is submitted: the submitted source says
        /// what is being drawn, and only the presented one says what is being looked at.
        presented_scene_id: Option<u64>,
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
        frame_capture: FrameCapture,
    }

    struct BrowserRefreshTurn<'a> {
        frame_loop: &'a mut BrowserFrameLoop,
        runtime: &'a mut BrowserRuntime,
        viewer: &'a mut ViewerController,
        requests: &'a mut RunRequests,
        now_ms: f64,
        observed: PresentEventFacts,
        hot: Option<crate::HotFrame>,
        slot: Option<HotSlot>,
        relief_redraw: bool,
        defer_scene_for_redraw: bool,
        scene_id: Option<u64>,
        surface_resolution: SurfaceResolutionOwner,
    }

    /// Production lowering from the presenter's event source into app-owned state.
    struct BrowserPresentEvents<'a> {
        frame_loop: &'a mut BrowserFrameLoop,
        runtime: &'a mut BrowserRuntime,
        viewer: &'a mut ViewerController,
        surface_resolution: &'a mut SurfaceResolutionOwner,
        refusal: Option<AppError>,
    }

    /// Browser lowering over the runtime-owned surface and presenter's warp encoder.
    struct BrowserSurfaceResolution<'a> {
        runtime: &'a mut BrowserRuntime,
        presenter: &'a mut Presenter,
        frame_capture: &'a mut FrameCapture,
    }

    impl SurfacePort for BrowserSurfaceResolution<'_> {
        type Error = AppError;
        type Frame = wgpu::SurfaceTexture;

        fn pending(&self) -> bool {
            self.runtime.has_pending_surface()
        }

        fn acquire(&mut self, generation: u32) -> Result<Self::Frame, Self::Error> {
            self.runtime.acquire_for_warp(generation)
        }

        fn submit(
            &mut self,
            frame: &Self::Frame,
            job: &SurfaceWarpJob,
        ) -> Result<ember_julibrot_present::FrameReceipt, Self::Error> {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.presenter
                .frame(
                    FrameState {
                        surface_view: &view,
                        canvas_width: job.canvas_extent[0],
                        canvas_height: job.canvas_extent[1],
                        refresh_id: job.refresh_id,
                        now_ms: job.now_ms,
                    },
                    job.slot,
                )
                .map_err(present_error)
        }

        fn retain(
            &mut self,
            frame: Self::Frame,
            generation: u32,
            receipt: &ember_julibrot_present::FrameReceipt,
        ) -> Result<(), Self::Error> {
            self.runtime
                .retain_for_warp(receipt.warp_id, generation, receipt.precision_mode, frame)
        }

        fn release_unsubmitted(&mut self, generation: u32) -> bool {
            self.runtime.release_unsubmitted_warp(generation)
        }

        fn resolve(
            &mut self,
            event: crate::surface::SurfaceEvent,
        ) -> crate::SurfaceAction<Self::Frame> {
            self.runtime.resolve_surface(event)
        }

        fn present(&mut self, event: &SurfaceResolutionEvent, frame: Self::Frame) {
            let SurfaceResolutionEvent::WarpCompleted {
                measurement,
                capture: arming,
            } = *event
            else {
                return;
            };
            let presenter = &mut *self.presenter;
            let capture = &mut *self.frame_capture;
            BrowserRuntime::present_surface_capturing(frame, |texture| {
                if !arming.surface_due() {
                    return;
                }
                capture.armed = false;
                match presenter.request_frame_readback(texture) {
                    Ok(()) => capture.refusal = None,
                    Err(error) => capture.refusal = Some(error.to_string()),
                }
            });
            presenter.record_presented(measurement.id);
        }

        fn drop_frame(&mut self, _frame: Self::Frame) {}

        fn invalid_receipt(&self, detail: &'static str) -> Self::Error {
            AppError::Present(detail.to_string())
        }
    }

    impl OrderedRefresh for BrowserRefreshTurn<'_> {
        type Error = AppError;
        type Output = RefreshOutcome;

        fn drain_capture(&mut self) -> Result<(), Self::Error> {
            if !self.now_ms.is_finite() {
                return Err(AppError::Deadline {
                    operation: "refresh clock",
                    deadline_ms: PresentConfig::V1_FENCE_DEADLINE_MS,
                });
            }
            if let Err(error) = self.runtime.check_device("Julibrot refresh") {
                let mut port = BrowserSurfaceResolution {
                    runtime: self.runtime,
                    presenter: &mut self.frame_loop.presenter,
                    frame_capture: &mut self.frame_loop.frame_capture,
                };
                self.surface_resolution
                    .resolve(&mut port, SurfaceResolutionEvent::DeviceFailed)?;
                return Err(error);
            }
            self.frame_loop.refresh_id = self
                .frame_loop
                .refresh_id
                .checked_add(1)
                .ok_or(AppError::GenerationExhausted)?;
            self.frame_loop.drain_frame_capture(self.now_ms);
            Ok(())
        }

        fn stage_capture(&mut self, stage: CaptureDrained) -> Result<(), Self::Error> {
            let CaptureDrained = stage;
            self.frame_loop.stage_frame_capture();
            Ok(())
        }

        fn observe_fences(&mut self, stage: CaptureStaged) -> Result<(), Self::Error> {
            let CaptureStaged = stage;
            let mut port = BrowserPresentEvents {
                frame_loop: self.frame_loop,
                runtime: self.runtime,
                viewer: self.viewer,
                surface_resolution: &mut self.surface_resolution,
                refusal: None,
            };
            let observation = PresentEventOwner::observe(&mut port, self.now_ms);
            observation.finish?;
            self.observed = observation.turn.facts;
            Ok(())
        }

        fn write_hot(&mut self, stage: FencesObserved) -> Result<(), Self::Error> {
            let FencesObserved = stage;
            let frame_loop = &mut *self.frame_loop;
            let viewer = &mut *self.viewer;
            let requests = &mut *self.requests;

            frame_loop.synchronize_precision_mode(viewer)?;
            frame_loop.requested_plane = viewer.checked_plane();
            if requests.frame {
                let restart_scene = frame_loop.scene_ready(viewer.requested().zoom_log2)
                    && frame_loop.presented_view_is_stale(viewer);
                frame_loop
                    .loop_state
                    .accept_request(frame_loop.main.generation_applied, restart_scene);
                requests.frame = false;
            }
            if requests.scene_update {
                requests.scene_update = false;
                frame_loop
                    .loop_state
                    .request_scene_update(frame_loop.main.generation_applied);
                frame_loop.prepared_level = None;
            }
            if let Some(error) = frame_loop.worker_service_mut().take_error() {
                frame_loop.abandon_submitted_references(viewer);
                return Err(error);
            }

            if KernelMode::for_zoom(viewer.requested().zoom_log2) == KernelMode::Shallow {
                frame_loop.abandon_submitted_references(viewer);
            }
            let backdrop_active = frame_loop.prepare_backdrop(viewer)?;
            if !backdrop_active {
                frame_loop.prepare_due_level();
            }
            let extent = frame_loop.prepared_extent();
            let mut hot = viewer.drain_hot(extent)?;
            frame_loop.owner_epoch = hot.state.epoch;
            frame_loop.main = hot.state.main;
            frame_loop.centre_from_reference_px = hot.state.hot.centre_from_reference_px;
            frame_loop.observe_scene_selection(viewer);
            let completed_requested_final = frame_loop.presenter.has_completed_requested_final(
                &hot.pose,
                published_iteration_cap(&frame_loop.plan),
                viewer.requested().precision_mode,
            );
            if super::stale_view_needs_a_new_scene(
                frame_loop.loop_state.scene_mode(),
                frame_loop.loop_state.refinement_pending(),
                frame_loop.presented_view_is_stale(viewer),
                completed_requested_final,
            ) {
                frame_loop
                    .loop_state
                    .request_missing_final(frame_loop.main.generation_applied);
                frame_loop.prepared_level = None;
                frame_loop.prepare_due_level();
                // Re-selecting the level changes the prepared extent, and the drained pose is
                // expressed in that extent's pixels: the screen map and centre_from_reference_px
                // both rescale with it, so the first drain no longer describes this scene.
                hot = viewer.drain_hot(frame_loop.prepared_extent())?;
                frame_loop.owner_epoch = hot.state.epoch;
                frame_loop.main = hot.state.main;
                frame_loop.centre_from_reference_px = hot.state.hot.centre_from_reference_px;
            }
            frame_loop.install_main(viewer, hot.pose.object, hot.plane, hot.pose.map);
            let mut slot = HotSlot::for_refresh(
                frame_loop.refresh_id,
                frame_loop.hot_stride,
                hot.state.epoch,
            )
            .map_err(|error| AppError::Present(error.to_string()))?;
            if requests.measurement && frame_loop.presenter.facts().completed_scene_id.is_some() {
                requests.measurement = false;
            }
            // Scheduler idleness cannot disarm a covering retained picture. The presenter proves
            // geometric coverage before it turns this authorization into a hold.
            let hold_refused_warp = frame_loop.presenter.facts().completed_scene_id.is_some();
            frame_loop.presenter.write_hot(
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
                hold_refused_warp,
            );

            let main_arrived = frame_loop.service_arrivals(viewer, self.now_ms)?;
            let shallow_accepted = frame_loop.submit_pending_reference(viewer, hot.plane)?;
            if main_arrived || shallow_accepted {
                frame_loop.prepare_backdrop(viewer)?;
                frame_loop.prepare_due_level();
                hot = viewer.drain_hot(frame_loop.prepared_extent())?;
                frame_loop.owner_epoch = hot.state.epoch;
                frame_loop.main = hot.state.main;
                frame_loop.observe_scene_selection(viewer);
                frame_loop.install_main(viewer, hot.pose.object, hot.plane, hot.pose.map);
                slot = HotSlot::for_refresh(
                    frame_loop.refresh_id,
                    frame_loop.hot_stride,
                    hot.state.epoch,
                )
                .map_err(|error| AppError::Present(error.to_string()))?;
                frame_loop.presenter.write_hot(
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
                    hold_refused_warp,
                );
            }
            if frame_loop.active_backdrop_map.is_none()
                && frame_loop
                    .loop_state
                    .skip_drafts_for_accepted_warp(frame_loop.presenter.accepted_warp_source(slot))
            {
                frame_loop.prepared_level = None;
                let prepared_final = frame_loop.prepare_due_level();
                debug_assert!(prepared_final);
                hot = viewer.drain_hot(frame_loop.prepared_extent())?;
                frame_loop.owner_epoch = hot.state.epoch;
                frame_loop.main = hot.state.main;
                frame_loop.observe_scene_selection(viewer);
                frame_loop.install_main(viewer, hot.pose.object, hot.plane, hot.pose.map);
                slot = HotSlot::for_refresh(
                    frame_loop.refresh_id,
                    frame_loop.hot_stride,
                    hot.state.epoch,
                )
                .map_err(|error| AppError::Present(error.to_string()))?;
                frame_loop.presenter.write_hot(
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
                    hold_refused_warp,
                );
            }
            self.hot = Some(hot);
            self.slot = Some(slot);
            Ok(())
        }

        fn consider_scene(&mut self, stage: HotWritten) -> Result<(), Self::Error> {
            let HotWritten = stage;
            let (Some(hot), Some(slot)) = (self.hot, self.slot) else {
                return Err(AppError::Present(
                    "ordered refresh lost its HOT stage".to_string(),
                ));
            };
            let frame_loop = &mut *self.frame_loop;
            let viewer = &*self.viewer;
            let relief_redraw = frame_loop.presenter.accepted_relief_redraw(slot);
            let defer_scene_for_redraw = super::defer_scene_until_relief_redraw(
                relief_redraw,
                frame_loop.presented_view_is_stale(viewer),
            );
            let scene_id = if defer_scene_for_redraw && frame_loop.active_backdrop_map.is_none() {
                None
            } else {
                frame_loop.submit_due_scene(
                    viewer,
                    hot.pose.object,
                    hot.plane,
                    hot.pose.map,
                    hot.pose.centre_from_reference_px,
                    slot,
                    hot.state.epoch,
                    self.now_ms,
                )?
            };
            self.relief_redraw = relief_redraw;
            self.defer_scene_for_redraw = defer_scene_for_redraw;
            self.scene_id = scene_id;
            Ok(())
        }

        fn consider_warp(&mut self, stage: SceneConsidered) -> Result<Self::Output, Self::Error> {
            let SceneConsidered = stage;
            let Some(slot) = self.slot else {
                return Err(AppError::Present(
                    "ordered refresh lost its HOT slot".to_string(),
                ));
            };
            let frame_loop = &mut *self.frame_loop;
            let runtime = &mut *self.runtime;
            let viewer = &*self.viewer;
            let scene_id = self.scene_id;

            let mut warp_id = None;
            // A redraw that defers the replacement scene is itself required progress. Stale-view
            // recovery may restart refinement without arming a one-shot frame request, so making
            // the redraw depend only on scheduler demand would leave both submissions waiting.
            let warp_requested = super::warp_submission_due(
                frame_loop
                    .loop_state
                    .warp_requested(frame_loop.frame_policy.policy()),
                self.defer_scene_for_redraw,
            );
            let redraw_scene_in_flight = super::hold_redraw_during_scene(
                self.relief_redraw,
                frame_loop.presenter.facts().in_flight_scene_id.is_some(),
            );
            if warp_requested && !redraw_scene_in_flight {
                let generation = frame_loop.loop_state.generation();
                let dimensions = runtime.facts();
                let job = SurfaceWarpJob {
                    generation,
                    canvas_extent: [dimensions.width, dimensions.height],
                    refresh_id: frame_loop.refresh_id,
                    now_ms: self.now_ms,
                    slot,
                };
                let submission = {
                    let mut port = BrowserSurfaceResolution {
                        runtime,
                        presenter: &mut frame_loop.presenter,
                        frame_capture: &mut frame_loop.frame_capture,
                    };
                    self.surface_resolution.submit(&mut port, job)
                };
                match submission {
                    Ok(SurfaceSubmission::Submitted(receipt)) => {
                        warp_id = Some(receipt.warp_id);
                        frame_loop.last_warp_source = receipt.source_scene_id;
                        if super::schedule_exposure_fill(
                            &mut frame_loop.loop_state,
                            receipt.exposed,
                            frame_loop.main.generation_applied,
                        ) {
                            frame_loop.prepared_level = None;
                        }
                        if super::warp_presents_requested_view(
                            frame_loop.presenter.facts().warp_kind,
                        ) {
                            frame_loop.pending_warp_view =
                                Some((receipt.warp_id, frame_loop.view_stamp(viewer)));
                        }
                        frame_loop.loop_state.warp_submitted();
                    }
                    Ok(SurfaceSubmission::Occupied) => {}
                    Err(AppError::SurfaceSkipped { .. }) => {
                        return Ok(frame_loop.outcome(
                            None,
                            scene_id,
                            false,
                            RefreshStatus::SkippedTimeout,
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
            let status = if self.observed.presented {
                RefreshStatus::Presented
            } else if warp_id.is_some() || scene_id.is_some() {
                RefreshStatus::Submitted
            } else if self.observed.cancelled {
                RefreshStatus::Cancelled
            } else if self.observed.refused {
                RefreshStatus::Refused
            } else {
                RefreshStatus::Waiting
            };
            Ok(frame_loop.outcome(warp_id, scene_id, self.observed.presented, status))
        }
    }

    impl BrowserFrameLoop {
        fn worker_service(&self) -> &WorkerServiceOwner<ChannelWorkerService> {
            self.worker_service
                .as_ref()
                .unwrap_or_else(|| unreachable!("worker service is present outside arrival apply"))
        }

        fn worker_service_mut(&mut self) -> &mut WorkerServiceOwner<ChannelWorkerService> {
            self.worker_service
                .as_mut()
                .unwrap_or_else(|| unreachable!("worker service is present outside arrival apply"))
        }

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
            let mut kernel_submission =
                KernelSubmissionOwner::new(BrowserKernelSubmission { kernels });
            let requested = viewer.requested();
            let extent = GridExtent {
                width: runtime.facts().width,
                height: runtime.facts().height,
            };
            let mut plan = kernel_submission
                .plan(
                    &executor,
                    KernelPlan {
                        target: KernelGridTarget::Main,
                        requested_extent: extent,
                        requested_max_iter: requested.iteration_cap,
                        precision_mode: None,
                    },
                )
                .map_err(kernel_error)?
                .into_plan();
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
                .reference_centre()
                .ok_or_else(|| AppError::Worker("owner navigation is unconfigured".to_string()))?;
            let [grid, spare_grid] = kernel_submission
                .allocate_grid_pair(&mut executor, KernelGridTarget::Main, &plan)
                .map_err(kernel_error)?
                .into_grids();
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
            let mut main = viewer.published_main();
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
                kernel_submission,
                presenter,
                worker_service: Some(WorkerServiceOwner::new(ChannelWorkerService {
                    owner_endpoint,
                    _producer_endpoint: producer_endpoint,
                    pending_response: None,
                    reference_upload,
                })),
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
                sampled_reference_refusal: None,
                sampled_resume_level: None,
                submitted_references: Vec::with_capacity(2),
                plan,
                main_grid_pair: Some(MainGridPair {
                    current: grid,
                    spare: spare_grid,
                }),
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
                presented_scene_id: None,
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
                frame_capture: FrameCapture {
                    armed: false,
                    route: runtime.facts().frame_copy_route,
                    ready: None,
                    refusal: None,
                    in_flight_since_ms: None,
                },
            };
            Ok(frame_loop)
        }

        /// Executes one bounded refresh and immediate pre-yield completion observation.
        ///
        /// A typed refusal that escapes one turn is latched: the loop stops and every later call
        /// returns the same cause, so the page reports one honest reason instead of restating a
        /// broken invariant sixty times a second. A transient fence refusal never escapes.
        ///
        /// Arms one copy of the next presented frame.
        ///
        /// Arming is not a copy: the surface image exists only between its acquisition and its
        /// presentation, so the request waits for the turn that presents a frame and is taken
        /// there. A second arming while one is already outstanding is the same one request.
        pub const fn arm_frame_capture(&mut self) {
            self.frame_capture.armed = true;
        }

        /// Reports whether an armed or in-flight frame copy still needs turns of the loop.
        #[must_use]
        pub fn frame_capture_turning(&self) -> bool {
            self.frame_capture.armed
                || self.presenter.offscreen_frame_readback_armed()
                || self.presenter.frame_readback_pending()
        }

        /// Reports which route a copy takes on this device.
        #[must_use]
        pub const fn frame_capture_route(&self) -> ember_julibrot_present::FrameReadbackRoute {
            self.frame_capture.route
        }

        /// Reports which route produced the copy waiting to be taken.
        #[must_use]
        pub fn frame_capture_ready_route(
            &self,
        ) -> Option<ember_julibrot_present::FrameReadbackRoute> {
            self.frame_capture.ready.as_ref().map(|frame| frame.route)
        }

        /// Reports the completed scene the copy waiting to be taken was drawn from.
        #[must_use]
        pub fn frame_capture_scene_id(&self) -> Option<u64> {
            self.frame_capture
                .ready
                .as_ref()
                .and_then(|frame| frame.scene_id)
        }

        /// Reports whether a copy is armed or in flight and has not yet been taken.
        #[must_use]
        pub fn frame_capture_pending(&self) -> bool {
            self.frame_capture_turning() && self.frame_capture.ready.is_none()
        }

        /// Returns the extent of the copy that is waiting to be taken.
        #[must_use]
        pub fn frame_capture_extent(&self) -> Option<[u32; 2]> {
            self.frame_capture
                .ready
                .as_ref()
                .map(|frame| [frame.width, frame.height])
        }

        /// Returns the typed reason the last copy did not happen, if one did not.
        #[must_use]
        pub fn frame_capture_refusal(&self) -> Option<&str> {
            self.frame_capture.refusal.as_deref()
        }

        /// Takes the completed copy, leaving nothing behind for a second caller.
        pub fn take_frame_capture(&mut self) -> Option<ember_julibrot_present::FrameReadback> {
            self.frame_capture.ready.take()
        }

        /// Collects a completed copy without waiting on one that is still in flight.
        ///
        /// A capture the presentation pass could not encode left its reason behind; picking that up
        /// here is what stops an armed copy from ending as a silent absence rather than a cause.
        fn drain_frame_capture(&mut self, now_ms: f64) {
            if let Some(refusal) = self.presenter.take_frame_readback_refusal() {
                self.frame_capture.refusal = Some(refusal.to_string());
                self.frame_capture.in_flight_since_ms = None;
            }
            if !self.presenter.frame_readback_pending() {
                self.frame_capture.in_flight_since_ms = None;
                return;
            }
            let since = *self.frame_capture.in_flight_since_ms.get_or_insert(now_ms);
            match self.presenter.take_frame_readback() {
                Ok(None) => {
                    // A map that has not fired inside the deadline is not a slow copy: it is one
                    // that is never coming, and waiting on it costs the caller its whole timeout,
                    // the device a surface-sized buffer, and every later request its refusal.
                    if now_ms - since > crate::FRAME_CAPTURE_DEADLINE_MS
                        && self.presenter.abandon_frame_readback()
                    {
                        self.frame_capture.in_flight_since_ms = None;
                        self.frame_capture.refusal = Some(format!(
                            "the frame copy was abandoned: its map did not complete within {:.0} ms",
                            crate::FRAME_CAPTURE_DEADLINE_MS
                        ));
                    }
                }
                Ok(Some(frame)) => {
                    self.frame_capture.refusal = None;
                    self.frame_capture.in_flight_since_ms = None;
                    self.frame_capture.ready = Some(frame);
                }
                Err(error) => {
                    self.frame_capture.in_flight_since_ms = None;
                    self.frame_capture.refusal = Some(error.to_string());
                }
            }
        }

        /// Hands an armed copy to the presentation pass, on a device whose surface cannot be read.
        ///
        /// The offscreen route has to be armed before the pass is submitted, because the second
        /// draw is appended to that submission's own encoder. The direct route is not armed here:
        /// it copies the surface image, which exists only at present time, so it is taken there.
        fn stage_frame_capture(&mut self) {
            let arming = crate::CaptureArming {
                armed: self.frame_capture.armed,
                route_matches: self.frame_capture.route
                    == ember_julibrot_present::FrameReadbackRoute::OffscreenRerender,
                readback_in_flight: self.presenter.frame_readback_pending(),
                renderer_already_armed: self.presenter.offscreen_frame_readback_armed(),
            };
            if !arming.offscreen_due() {
                return;
            }
            self.presenter.arm_offscreen_frame_readback();
            self.frame_capture.armed = false;
        }

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
            let mut turn = BrowserRefreshTurn {
                frame_loop: self,
                runtime,
                viewer,
                requests,
                now_ms,
                observed: PresentEventFacts::default(),
                hot: None,
                slot: None,
                relief_redraw: false,
                defer_scene_for_redraw: false,
                scene_id: None,
                surface_resolution: SurfaceResolutionOwner::new(now_ms),
            };
            execute_ordered_refresh(&mut turn)
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
    }

    impl BrowserPresentEvents<'_> {
        #[cfg(test)]
        fn event_effect(
            &self,
            presented: bool,
            refused: bool,
            cancelled: bool,
        ) -> PresentEventEffect {
            PresentEventEffect {
                presented,
                refused,
                cancelled,
                #[cfg(test)]
                retained_scene_id: self.frame_loop.presenter.facts_ref().completed_scene_id,
                #[cfg(test)]
                presented_scene_id: self.frame_loop.presented_scene_id,
            }
        }

        #[cfg(not(test))]
        const fn event_effect(
            presented: bool,
            refused: bool,
            cancelled: bool,
        ) -> PresentEventEffect {
            PresentEventEffect {
                presented,
                refused,
                cancelled,
            }
        }

        fn complete_scene(
            &mut self,
            frame: &ember_julibrot_present::SceneFrame,
            reference_sample: Option<u32>,
        ) {
            self.frame_loop
                .level_timings
                .complete_scene(frame.scene_id, frame.measurement);
            let backdrop_completed = self.frame_loop.backdrop.as_mut().is_some_and(|backdrop| {
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
                self.frame_loop.active_backdrop_map = None;
            } else if self.frame_loop.loop_state.completed(
                frame.scene_id,
                frame.pose.orbit_generation,
                frame.level,
            ) {
                self.frame_loop.prepared_level = None;
                self.frame_loop.maybe_request_sampled_reference(
                    self.viewer,
                    frame,
                    reference_sample,
                );
            }
        }

        fn drop_scene(
            &mut self,
            scene_id: u64,
            measurement: ember_julibrot_present::SubmissionMeasurement,
        ) {
            self.frame_loop
                .level_timings
                .drop_scene(scene_id, Some(measurement));
            let backdrop_retired = self.frame_loop.backdrop.as_mut().is_some_and(|backdrop| {
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
                self.frame_loop.active_backdrop_map = None;
            } else if self.frame_loop.loop_state.retired(scene_id) {
                self.frame_loop.prepared_level = None;
            }
        }

        fn complete_warp(
            &mut self,
            measurement: ember_julibrot_present::SubmissionMeasurement,
        ) -> Result<bool, AppError> {
            self.frame_loop.level_timings.complete_warp(measurement);
            if measurement.sample_class == ember_julibrot_present::SampleClass::ColdWarmUp {
                self.frame_loop.frame_policy.reset();
            }
            self.frame_loop
                .frame_policy
                .record(measurement.wall_ms)
                .map_err(|error| AppError::Present(error.to_string()))?;
            let transaction = SurfaceResolutionEvent::WarpCompleted {
                measurement,
                capture: crate::CaptureArming {
                    armed: self.frame_loop.frame_capture.armed,
                    route_matches: self.frame_loop.frame_capture.route
                        == ember_julibrot_present::FrameReadbackRoute::Surface,
                    readback_in_flight: self.frame_loop.presenter.frame_readback_pending(),
                    renderer_already_armed: false,
                },
            };
            let effect = {
                let mut port = BrowserSurfaceResolution {
                    runtime: self.runtime,
                    presenter: &mut self.frame_loop.presenter,
                    frame_capture: &mut self.frame_loop.frame_capture,
                };
                self.surface_resolution.resolve(&mut port, transaction)
            }?;
            let presented = effect.presented;
            if presented {
                // What is now on the canvas, as opposed to what was last submitted.
                self.frame_loop.presented_scene_id = measurement.source_scene_id;
                if let Some((warp_id, stamp)) = self.frame_loop.pending_warp_view
                    && warp_id == measurement.id
                {
                    self.frame_loop.pending_warp_view = None;
                    self.frame_loop.presented_view = Some(stamp);
                }
            }
            Ok(presented)
        }

        fn refuse_fence(
            &mut self,
            kind: SubmissionKind,
            id: u64,
            reason: FenceRefusal,
            polls: u32,
            wall_ms: f64,
            precision_mode: &'static str,
        ) -> Result<(bool, bool), AppError> {
            if matches!(kind, SubmissionKind::Scene) {
                self.frame_loop.level_timings.drop_scene(id, None);
            }
            let backdrop_retired = matches!(kind, SubmissionKind::Scene)
                && self.frame_loop.backdrop.as_mut().is_some_and(|backdrop| {
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
                self.frame_loop.active_backdrop_map = None;
            }
            let outcome = self
                .frame_loop
                .loop_state
                .refused(kind, reason, id, polls, wall_ms);
            if outcome.retired_scene {
                self.frame_loop.prepared_level = None;
            }
            if matches!(kind, SubmissionKind::Warp) {
                let transaction = SurfaceResolutionEvent::WarpRefused {
                    kind,
                    id,
                    reason,
                    polls,
                    wall_ms,
                    precision_mode,
                };
                let mut port = BrowserSurfaceResolution {
                    runtime: self.runtime,
                    presenter: &mut self.frame_loop.presenter,
                    frame_capture: &mut self.frame_loop.frame_capture,
                };
                self.surface_resolution.resolve(&mut port, transaction)?;
                if self
                    .frame_loop
                    .pending_warp_view
                    .is_some_and(|pending| pending.0 == id)
                {
                    self.frame_loop.pending_warp_view = None;
                }
            }
            match outcome.class {
                RefusalClass::Device => {
                    if self.refusal.is_none() {
                        self.refusal = Some(super::fence_error(kind, reason, polls, wall_ms));
                    }
                    Ok((false, false))
                }
                RefusalClass::Cancelled => Ok((false, true)),
                RefusalClass::Transient => Ok((true, false)),
            }
        }
    }

    impl PresentEventPort for BrowserPresentEvents<'_> {
        type Error = AppError;

        fn poll(&mut self, now_ms: f64) -> PresentEvents {
            self.frame_loop.presenter.poll_fixed(now_ms)
        }

        fn scene_completed(
            &mut self,
            event: &PresentSceneCompletion,
        ) -> Result<PresentEventEffect, Self::Error> {
            self.complete_scene(&event.frame, event.reference_sample);
            #[cfg(test)]
            let effect = self.event_effect(false, false, false);
            #[cfg(not(test))]
            let effect = Self::event_effect(false, false, false);
            Ok(effect)
        }

        fn scene_dropped(
            &mut self,
            event: &PresentSceneDrop,
        ) -> Result<PresentEventEffect, Self::Error> {
            self.drop_scene(event.scene_id, event.measurement);
            #[cfg(test)]
            let effect = self.event_effect(false, false, false);
            #[cfg(not(test))]
            let effect = Self::event_effect(false, false, false);
            Ok(effect)
        }

        fn warp_completed(
            &mut self,
            event: &PresentWarpCompletion,
        ) -> Result<PresentEventEffect, Self::Error> {
            let presented = self.complete_warp(event.measurement)?;
            #[cfg(test)]
            let effect = self.event_effect(presented, false, false);
            #[cfg(not(test))]
            let effect = Self::event_effect(presented, false, false);
            Ok(effect)
        }

        fn fence_refused(
            &mut self,
            event: &PresentFenceRefusal,
        ) -> Result<PresentEventEffect, Self::Error> {
            let (refused, cancelled) = self.refuse_fence(
                event.kind,
                event.id,
                event.reason,
                event.polls,
                event.wall_ms,
                event.precision_mode,
            )?;
            #[cfg(test)]
            let effect = self.event_effect(false, refused, cancelled);
            #[cfg(not(test))]
            let effect = Self::event_effect(false, refused, cancelled);
            Ok(effect)
        }

        fn finish(&mut self) -> Result<(), Self::Error> {
            self.refusal.take().map_or(Ok(()), Err)
        }

        fn invalid_receipt(&self, detail: &'static str) -> Self::Error {
            AppError::Present(detail.to_string())
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
