//! Cross-slice progressive frame scheduling and browser GPU integration.

#[cfg(any(target_arch = "wasm32", test))]
use std::sync::Arc;

#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_kernels::{EscapeGrid, RefinementPlan};
use ember_julibrot_kernels::{RefinementLevel, next_refinement_level};
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_math::PICTURE_FAST_EDIT_BUDGET;
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_math::PoseMap;
use ember_julibrot_math::PrecisionMode;
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_present::{FenceRefusal, SubmissionKind};

#[cfg(any(target_arch = "wasm32", test))]
use crate::{AppError, ViewerController};

#[cfg(test)]
const LEVELS: [RefinementLevel; 3] = [
    RefinementLevel::Preview,
    RefinementLevel::Interactive,
    RefinementLevel::Final,
];

#[cfg(any(target_arch = "wasm32", test))]
const REFERENCE_RECORD_BYTES: usize = 8;
#[cfg(any(target_arch = "wasm32", test))]
const REFERENCE_TEXEL_BYTES: usize = 16;

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
pub(crate) fn backdrop_extent(final_extent: [u32; 2]) -> Option<[u32; 2]> {
    let [width, height] = final_extent;
    let extent = [width / 2, height / 2];
    if extent.contains(&0) {
        return None;
    }
    let final_records = width.checked_mul(height)?;
    let backdrop_records = extent[0].checked_mul(extent[1])?;
    (backdrop_records <= final_records / 4).then_some(extent)
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
/// Tests whether the accepted perturbation reference belongs to the requested scene.
///
/// `ViewerController` writes requested zoom and owner HOT zoom from the same edit result, and its
/// HOT drain refuses any divergence. Their bit equality is therefore an identity invariant rather
/// than an approximate comparison between independently rounded coordinates.
fn perturbation_reference_is_current(
    requested_zoom_log2: f64,
    main_generation: u32,
    reference: Option<(u32, f64)>,
) -> bool {
    reference.is_some_and(|(generation, zoom_log2)| {
        generation == main_generation && zoom_log2.to_bits() == requested_zoom_log2.to_bits()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SceneTicket {
    id: u64,
    generation: u32,
    level: RefinementLevel,
}

/// Whether control changes immediately refine a new scene or wait for an explicit update.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SceneMode {
    /// Preserve the existing behaviour: every material control change restarts refinement.
    #[default]
    Auto,
    /// Reproject control changes and wait for an explicit scene update before refining.
    Manual,
}

impl SceneMode {
    /// Decodes the browser checkbox boundary, where zero is manual and one is automatic.
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Manual),
            1 => Some(Self::Auto),
            _ => None,
        }
    }

    /// Returns the stable facts value shown by the page.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

/// Latest-wins Preview, Interactive, Final scheduler with one scene in flight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefinementSchedule {
    generation: u32,
    precision_mode: PrecisionMode,
    next: Option<RefinementLevel>,
    in_flight: Option<SceneTicket>,
}

impl Default for RefinementSchedule {
    fn default() -> Self {
        Self {
            generation: 0,
            precision_mode: PrecisionMode::Deterministic,
            next: None,
            in_flight: None,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
trait PresenterPoll {
    type Event;

    fn poll_once(&mut self, now_ms: f64) -> Vec<Self::Event>;
}

/// What one present fence refusal means for the life of the refresh loop.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefusalClass {
    /// A bounded wall or poll budget elapsed with the fence still unobserved.
    Transient,
    /// App or presenter cancelled the submission in favour of newer work.
    Cancelled,
    /// The four-byte fence callback itself failed, which is device-level loss.
    Device,
}

/// Names the outcome of applying the refusal policy to one refused submission.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RefusalOutcome {
    class: RefusalClass,
    retired_scene: bool,
}

/// Classifies a present fence refusal into the two lives it can have.
///
/// A deadline or a poll limit says only that the bounded observation window closed before the
/// fence did, which is exactly what a background-throttled tab produces: the wall keeps running
/// while the callback queue does not. Treating that as terminal kills a page whose GPU is
/// healthy. Only a failed fence callback is evidence that the device is gone.
#[cfg(any(target_arch = "wasm32", test))]
const fn classify_refusal(reason: FenceRefusal) -> RefusalClass {
    match reason {
        FenceRefusal::PollLimit | FenceRefusal::Deadline => RefusalClass::Transient,
        FenceRefusal::Cancelled => RefusalClass::Cancelled,
        FenceRefusal::Device => RefusalClass::Device,
    }
}

/// Renders one present fence refusal as the typed app error the page displays.
#[cfg(any(target_arch = "wasm32", test))]
fn fence_error(kind: SubmissionKind, reason: FenceRefusal, polls: u32, wall_ms: f64) -> AppError {
    let operation = match kind {
        SubmissionKind::Scene => "scene fence",
        SubmissionKind::Warp => "warp fence",
    };
    match reason {
        FenceRefusal::PollLimit => AppError::CompletionPollLimit { operation, polls },
        FenceRefusal::Deadline => AppError::Deadline {
            operation,
            deadline_ms: wall_ms,
        },
        FenceRefusal::Device => AppError::Mapping {
            operation,
            detail: "four-byte fence callback failed".to_string(),
        },
        FenceRefusal::Cancelled => AppError::Present(format!(
            "{operation} was cancelled after {polls} polls and {wall_ms} ms"
        )),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "manual dirtiness, manual rendering, requested run, and completed run are independent scheduler facts"
)]
struct FrameLoop {
    schedule: RefinementSchedule,
    scene_mode: SceneMode,
    scene_update_pending: bool,
    draft_skipped_count: u64,
    last_draft_skip_reason: Option<&'static str>,
    manual_rendering: bool,
    restart_after_scene: Option<u32>,
    requested_run: bool,
    completed_run: bool,
    transient_refusals: u32,
    last_transient: Option<AppError>,
    last_transient_text: Option<Arc<str>>,
    stopped: Option<AppError>,
    stopped_text: Option<Arc<str>>,
}

#[cfg(any(target_arch = "wasm32", test))]
const fn schedule_exposure_fill(
    frame_loop: &mut FrameLoop,
    exposed: bool,
    generation: u32,
) -> bool {
    exposed && frame_loop.exposure_fill(generation)
}

#[cfg(any(target_arch = "wasm32", test))]
fn pose_maps_close(
    first: ember_julibrot_math::PoseMap,
    second: ember_julibrot_math::PoseMap,
) -> bool {
    use ember_julibrot_math::PoseMap;
    match (first, second) {
        (PoseMap::EdgeOn, PoseMap::EdgeOn) => true,
        (PoseMap::Mapped(first), PoseMap::Mapped(second)) => first
            .inverse
            .into_iter()
            .zip(second.inverse)
            .all(|(first, second)| (first - second).abs() <= 1.0e-12),
        _ => false,
    }
}

/// The extent the stamped screen map is built at.
///
/// The ladder renders every level below Final at a fraction of the requested extent, and a screen
/// map's perspective row scales with the grid width, so the same requested view has a different
/// map at each level. Stamping the map at the level currently prepared therefore makes the
/// ladder's own progress read as a control change, and the schedule restarts at Preview forever.
/// The requested extent keeps the stamp a property of the requested view alone, which is what
/// staleness asks about.
#[cfg(any(target_arch = "wasm32", test))]
const fn stamped_extent(plan: &RefinementPlan) -> [u32; 2] {
    [plan.requested_extent.width, plan.requested_extent.height]
}

#[cfg(any(target_arch = "wasm32", test))]
fn stamped_screen_map(viewer: &ViewerController, plan: &RefinementPlan) -> PoseMap {
    viewer
        .screen_map(stamped_extent(plan))
        .unwrap_or(PoseMap::EdgeOn)
}

/// Publishes the level a scene submission represents even when no kernel dispatch runs.
#[cfg(any(target_arch = "wasm32", test))]
const fn stamp_scene_level(grid: &mut EscapeGrid, plan: &RefinementPlan, level: RefinementLevel) {
    let spec = plan.level(level);
    grid.width = spec.extent.width;
    grid.height = spec.extent.height;
    grid.level = level;
}

#[cfg(any(target_arch = "wasm32", test))]
fn view_projection_changed(
    first: ember_julibrot_math::ViewControls,
    first_map: ember_julibrot_math::PoseMap,
    second: ember_julibrot_math::ViewControls,
    second_map: ember_julibrot_math::PoseMap,
) -> bool {
    if first.height_scale == 0.0 && second.height_scale == 0.0 {
        !pose_maps_close(first_map, second_map)
    } else {
        first != second
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn level_rank(level: RefinementLevel) -> u32 {
    match level {
        RefinementLevel::Preview => 0,
        RefinementLevel::Interactive => 1,
        RefinementLevel::Final => 2,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl FrameLoop {
    fn refresh<P: PresenterPoll>(presenter: &mut P, now_ms: f64) -> Vec<P::Event> {
        presenter.poll_once(now_ms)
    }

    const fn accept_request(&mut self, generation: u32, restart_scene: bool) {
        self.requested_run = true;
        self.completed_run = false;
        if restart_scene {
            self.scene_changed(generation);
        }
        if !self.schedule.pending() {
            self.completed_run = true;
        }
    }

    const fn apply_precision_mode(&mut self, precision_mode: PrecisionMode, generation: u32) {
        self.schedule.generation = generation;
        self.schedule.precision_mode = precision_mode;
        match self.scene_mode {
            SceneMode::Auto => {
                self.schedule.next = Some(RefinementLevel::Preview);
                self.schedule.in_flight = None;
                if self.requested_run {
                    self.completed_run = false;
                }
            }
            SceneMode::Manual => self.scene_changed(generation),
        }
    }

    const fn restart(&mut self, generation: u32) {
        self.schedule.restart(generation);
        self.restart_after_scene = None;
        if self.requested_run {
            self.completed_run = false;
        }
    }

    const fn scene_changed(&mut self, generation: u32) {
        match self.scene_mode {
            SceneMode::Auto => {
                if !self.schedule.pending() {
                    self.restart(generation);
                }
            }
            SceneMode::Manual => {
                self.scene_update_pending = true;
                self.manual_rendering = false;
                self.restart_after_scene = None;
                self.schedule.pause();
            }
        }
    }

    const fn scene_input_ready(&mut self, generation: u32) {
        if matches!(self.scene_mode, SceneMode::Auto) || self.manual_rendering {
            self.restart(generation);
        }
    }

    const fn scene_selection_changed(&mut self, generation: u32) {
        if matches!(self.scene_mode, SceneMode::Auto) {
            self.restart(generation);
        } else if !self.manual_rendering {
            self.scene_changed(generation);
        }
    }

    const fn request_scene_update(&mut self, generation: u32) {
        self.requested_run = true;
        self.completed_run = false;
        self.manual_rendering = matches!(self.scene_mode, SceneMode::Manual);
        if self.schedule.scene_in_flight() {
            self.schedule.pause();
            self.restart_after_scene = Some(generation);
        } else {
            self.restart(generation);
        }
    }

    fn set_scene_mode(&mut self, mode: SceneMode, generation: u32, has_scene: bool) {
        if self.scene_mode == mode {
            return;
        }
        self.scene_mode = mode;
        match mode {
            SceneMode::Auto => {
                self.manual_rendering = false;
                if self.scene_update_pending {
                    self.scene_update_pending = false;
                    self.requested_run = true;
                    self.completed_run = false;
                    if self.schedule.scene_in_flight() {
                        self.schedule.pause();
                        self.restart_after_scene = Some(generation);
                    } else {
                        self.restart(generation);
                    }
                }
            }
            SceneMode::Manual => {
                self.manual_rendering = !has_scene;
                if has_scene {
                    self.restart_after_scene = None;
                    self.schedule.pause();
                }
            }
        }
    }

    const fn exposure_fill(&mut self, generation: u32) -> bool {
        if !matches!(self.scene_mode, SceneMode::Auto) {
            if !self.manual_rendering {
                self.scene_update_pending = true;
            }
            return false;
        }
        if self.refinement_pending() {
            return false;
        }
        self.restart(generation);
        true
    }

    fn due(&self) -> Option<RefinementLevel> {
        self.schedule.due()
    }

    fn skip_drafts_for_accepted_warp(&mut self, source: Option<(RefinementLevel, bool)>) -> bool {
        let Some(due) = self.due() else {
            return false;
        };
        let Some((source_level, exposed)) = source else {
            return false;
        };
        if level_rank(source_level) <= level_rank(due) || due == RefinementLevel::Final {
            return false;
        }
        let skipped = match (self.schedule.precision_mode, due) {
            (PrecisionMode::Deterministic, RefinementLevel::Preview) => 2,
            (_, RefinementLevel::Preview | RefinementLevel::Interactive) => 1,
            (_, RefinementLevel::Final) => 0,
        };
        self.schedule.next = Some(RefinementLevel::Final);
        self.draft_skipped_count = self.draft_skipped_count.saturating_add(skipped);
        self.last_draft_skip_reason = Some(if exposed {
            "accepted exposed higher-level retained warp"
        } else {
            "accepted covering higher-level retained warp"
        });
        true
    }

    const fn submitted(&mut self, id: u64, level: RefinementLevel) {
        self.schedule.submitted(id, level);
    }

    fn completed(&mut self, id: u64, generation: u32, level: RefinementLevel) -> bool {
        let observed = self.schedule.matches_scene(id);
        let completed = self.schedule.completed(id, generation, level);
        if observed && let Some(restart_generation) = self.restart_after_scene.take() {
            self.restart(restart_generation);
            return true;
        }
        if completed && self.manual_rendering {
            self.scene_update_pending = false;
        }
        if self.requested_run && !self.schedule.pending() {
            self.completed_run = true;
        }
        completed
    }

    fn retired(&mut self, id: u64) -> bool {
        let retired = self.schedule.retired(id);
        if retired && let Some(restart_generation) = self.restart_after_scene.take() {
            self.restart(restart_generation);
        }
        if self.requested_run && !self.schedule.pending() {
            self.completed_run = true;
        }
        retired
    }

    const fn generation(&self) -> u32 {
        self.schedule.generation()
    }

    const fn refinement_pending(&self) -> bool {
        self.schedule.pending()
    }

    const fn scene_mode(&self) -> SceneMode {
        self.scene_mode
    }

    const fn scene_update_pending(&self) -> bool {
        matches!(self.scene_mode, SceneMode::Manual) && self.scene_update_pending
    }

    const fn hold_refused_warp(&self) -> bool {
        matches!(self.scene_mode, SceneMode::Manual)
    }

    const fn draft_skipped_count(&self) -> u64 {
        self.draft_skipped_count
    }

    const fn last_draft_skip_reason(&self) -> Option<&'static str> {
        self.last_draft_skip_reason
    }

    const fn warp_requested(&self, policy: crate::FramePolicy) -> bool {
        self.requested_run || !matches!(policy, crate::FramePolicy::SingleFrameOnDemand)
    }

    const fn warp_submitted(&mut self) {
        if self.requested_run && self.completed_run {
            self.requested_run = false;
            self.completed_run = false;
        }
    }

    /// Applies the fence-refusal policy to one refused submission of either kind.
    ///
    /// A transient refusal retires the submission, is counted, and re-arms the run: the refused
    /// warp is the only thing that would have asked for the next surface image, so without the
    /// re-arm a single-frame-on-demand page sits on a stale image with nothing pending and never
    /// schedules another refresh. Device loss is latched instead, and stops the loop.
    fn refused(
        &mut self,
        kind: SubmissionKind,
        reason: FenceRefusal,
        id: u64,
        polls: u32,
        wall_ms: f64,
    ) -> RefusalOutcome {
        let class = classify_refusal(reason);
        let error = fence_error(kind, reason, polls, wall_ms);
        let retired_scene = matches!(kind, SubmissionKind::Scene) && self.retired(id);
        match class {
            RefusalClass::Device => self.stop(error),
            RefusalClass::Transient | RefusalClass::Cancelled => self.record_transient(error),
        }
        RefusalOutcome {
            class,
            retired_scene,
        }
    }

    /// Counts one transient refusal and re-arms exactly one more run.
    ///
    /// `completed_run` mirrors the ladder rather than being cleared outright: when no level
    /// remains the re-armed run is already complete, so the very next warp submission retires the
    /// request and the page returns to its on-demand rhythm instead of spinning forever after a
    /// refusal it survived.
    fn record_transient(&mut self, error: AppError) {
        self.transient_refusals = self.transient_refusals.saturating_add(1);
        self.last_transient_text = Some(Arc::from(error.to_string()));
        self.last_transient = Some(error);
        self.requested_run = true;
        self.completed_run = !self.schedule.pending();
    }

    /// Latches the first terminal refusal; a later one never overwrites the cause.
    fn stop(&mut self, error: AppError) {
        if self.stopped.is_none() {
            self.stopped_text = Some(Arc::from(error.to_string()));
            self.stopped = Some(error);
        }
    }

    const fn stopped(&self) -> Option<&AppError> {
        self.stopped.as_ref()
    }

    const fn transient_refusals(&self) -> u32 {
        self.transient_refusals
    }

    const fn last_transient(&self) -> Option<&AppError> {
        self.last_transient.as_ref()
    }

    const fn last_transient_text(&self) -> Option<&Arc<str>> {
        self.last_transient_text.as_ref()
    }

    const fn stopped_text(&self) -> Option<&Arc<str>> {
        self.stopped_text.as_ref()
    }

    /// Reports whether JavaScript must schedule another cooperative turn.
    ///
    /// `view_stale` carries the term the loop had no way to express before: an image was
    /// presented for an older requested view, so work remains even though no submission is
    /// outstanding. A stopped loop answers false, and only then.
    #[allow(
        clippy::fn_params_excessive_bools,
        reason = "four independent outstanding-work terms; grouping them into a record only moves the same four flags behind a second name"
    )]
    const fn needs_refresh(
        &self,
        scene_in_flight: bool,
        warp_in_flight: bool,
        auxiliary_pending: bool,
        view_stale: bool,
    ) -> bool {
        if self.stopped.is_some() {
            return false;
        }
        scene_in_flight
            || warp_in_flight
            || auxiliary_pending
            || self.requested_run
            || self.schedule.pending()
            || view_stale
    }
}

/// Applies one requested precision policy to every CPU owner of that policy.
#[cfg(any(target_arch = "wasm32", test))]
fn apply_precision_mode(
    next: PrecisionMode,
    precision_mode: &mut PrecisionMode,
    loop_state: &mut FrameLoop,
    plan: &mut RefinementPlan,
    viewer: &mut ViewerController,
) -> Result<(), AppError> {
    viewer
        .owner_mut()
        .configure_precision_mode(next, PICTURE_FAST_EDIT_BUDGET)
        .map_err(|error| AppError::Worker(error.to_string()))?;
    let generation = viewer.owner().latest_requested_generation();
    *precision_mode = next;
    loop_state.apply_precision_mode(next, generation);
    *plan = (*plan).with_precision_mode(next);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
impl PresenterPoll for ember_julibrot_present::Presenter {
    type Event = ember_julibrot_present::PresentEvent;

    fn poll_once(&mut self, now_ms: f64) -> Vec<Self::Event> {
        self.poll(now_ms)
    }
}

impl RefinementSchedule {
    /// Restarts ordered refinement for a newly accepted selection.
    pub const fn restart(&mut self, generation: u32) {
        self.generation = generation;
        self.next = Some(RefinementLevel::Preview);
    }

    /// Stops future levels without forgetting a scene whose fence must still be observed.
    #[cfg(any(target_arch = "wasm32", test))]
    const fn pause(&mut self) {
        self.next = None;
    }

    /// Reports whether a submitted scene still owns the present target.
    #[cfg(any(target_arch = "wasm32", test))]
    const fn scene_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Reports whether the named scene is the one whose fence remains outstanding.
    #[cfg(any(target_arch = "wasm32", test))]
    fn matches_scene(&self, id: u64) -> bool {
        self.in_flight.is_some_and(|ticket| ticket.id == id)
    }

    /// Returns the exact next level only when present has no scene target occupied.
    #[must_use]
    pub fn due(&self) -> Option<RefinementLevel> {
        self.in_flight.is_none().then_some(self.next).flatten()
    }

    /// Records a successful present scene submission.
    pub const fn submitted(&mut self, id: u64, level: RefinementLevel) {
        self.in_flight = Some(SceneTicket {
            id,
            generation: self.generation,
            level,
        });
    }

    /// Advances only a completion belonging to the current schedule token.
    #[must_use]
    pub fn completed(&mut self, id: u64, generation: u32, level: RefinementLevel) -> bool {
        let Some(ticket) = self.in_flight.filter(|ticket| ticket.id == id) else {
            return false;
        };
        self.in_flight = None;
        if ticket.generation != self.generation
            || ticket.generation != generation
            || ticket.level != level
        {
            return false;
        }
        let Some(next) = self.next else {
            return false;
        };
        if next != level {
            return false;
        }
        self.next = next_refinement_level(self.precision_mode, level);
        true
    }

    /// Retires a refused or dropped scene while leaving the same current level pending.
    #[must_use]
    pub fn retired(&mut self, id: u64) -> bool {
        if self.in_flight.is_some_and(|ticket| ticket.id == id) {
            self.in_flight = None;
            true
        } else {
            false
        }
    }

    /// Reports either a due or an in-flight level without claiming delivery.
    #[must_use]
    pub const fn pending(&self) -> bool {
        self.next.is_some() || self.in_flight.is_some()
    }

    /// Returns the current accepted generation token.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use ember_julibrot_kernels::{
        DispatchFacts, EscapeGrid, GridExtent, JulibrotKernels, KERNEL_UNIFORM_BYTES, KernelMode,
        OUTPUT_PAGE_SIDE, ReferenceOrbitInput, RefinementLevel, RefinementPlan,
    };
    use ember_julibrot_math::{
        BigCentre, EscapeParams, ObjectAngles, Plane, PoseMap, PrecisionMode, precision_for,
        reference_shift_px, scale_split, shallow_pixel_scale, split_centre,
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

    use super::{FrameLoop, RefusalClass};
    use crate::{
        AppError, BrowserRuntime, FramePolicy, FramePolicyTracker, LevelTimingLedger,
        RefreshOutcome, RefreshStatus, RunRequests, ViewerController,
    };

    const HEAP_SIDE: u16 = 512;
    const HEAP_LAYERS: u16 = 16;
    const DESCRIPTOR_CAPACITY: u32 = 64;
    const SPAN_CAPACITY: u32 = 16;
    const HANDLE_CAPACITY: u32 = 128;
    const DIRECTORY_BYTES: u32 = SPAN_CAPACITY * 16 + HANDLE_CAPACITY * 4;
    const MAX_HEADER_PAGES: u32 = 64;
    const MAX_HEADER_SETS: u32 = 6;

    fn expand_reference_texels_from_array(
        records: &js_sys::Uint8Array,
        length: u32,
        texels: &mut Vec<u8>,
    ) -> Result<(), AppError> {
        let count = usize::try_from(length)
            .map_err(|_| AppError::Worker("reference length does not fit usize".to_string()))?;
        let expected = count
            .checked_mul(super::REFERENCE_RECORD_BYTES)
            .ok_or_else(|| AppError::Worker("reference record byte length overflow".to_string()))?;
        if usize::try_from(records.length()).ok() != Some(expected) {
            return Err(AppError::Worker(format!(
                "reference payload has {} bytes; expected {expected}",
                records.length()
            )));
        }
        let texel_bytes = super::reference_texel_bytes(length)?;
        if texels.capacity() < texel_bytes {
            texels
                .try_reserve_exact(texel_bytes.saturating_sub(texels.len()))
                .map_err(|error| {
                    AppError::Worker(format!("reference upload reserve failed: {error}"))
                })?;
        }
        texels.resize(texel_bytes, 0);
        records.copy_to(&mut texels[..expected]);
        for index in (0..count).rev() {
            let source = index * super::REFERENCE_RECORD_BYTES;
            let destination = index * super::REFERENCE_TEXEL_BYTES;
            texels.copy_within(source..source + super::REFERENCE_RECORD_BYTES, destination);
            texels[destination + super::REFERENCE_RECORD_BYTES
                ..destination + super::REFERENCE_TEXEL_BYTES]
                .fill(0);
        }
        Ok(())
    }

    fn viewer_precision_mode(value: u32) -> &'static str {
        PrecisionMode::from_u32(value).map_or("unavailable", PrecisionMode::as_str)
    }

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
        centre: BigCentre,
        zoom_log2: f64,
        plane: Plane,
        precision_mode: &'static str,
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
        submitted_references: Vec<SubmittedReference>,
        reference_upload: Vec<u8>,
        plan: RefinementPlan,
        grid: EscapeGrid,
        backdrop: Option<BackdropGrid>,
        active_backdrop_map: Option<PoseMap>,
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
                JulibrotKernels::plan(&executor, extent, params).map_err(kernel_error)?;
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
            let grid = kernels
                .allocate_grid(&mut executor, &plan)
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
                    max_iter: requested.iteration_cap,
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
                submitted_references: Vec::with_capacity(2),
                reference_upload,
                plan,
                grid,
                backdrop: None,
                active_backdrop_map: None,
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
            let observed = self.handle_events(runtime, events)?;
            self.synchronize_precision_mode(viewer)?;
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
            self.observe_scene_selection(viewer);
            if !self.loop_state.refinement_pending() && self.presented_view_is_stale(viewer) {
                self.loop_state.scene_changed(self.main.generation_applied);
                self.prepared_level = None;
                self.prepare_due_level();
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
            let hold_refused_warp = self.loop_state.hold_refused_warp();
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
                precision_mode: viewer_precision_mode(self.main.precision_mode),
            }
        }

        fn handle_events(
            &mut self,
            runtime: &mut BrowserRuntime,
            events: Vec<PresentEvent>,
        ) -> Result<ObservedEvents, AppError> {
            let mut observed = ObservedEvents::default();
            let mut refusal = None;
            for event in events {
                match event {
                    PresentEvent::SceneCompleted { frame } => {
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

        fn prepare_due_level(&mut self) -> bool {
            if self.active_backdrop_map.is_some() {
                return false;
            }
            let Some(level) = self.loop_state.due() else {
                return false;
            };
            if self.prepared_level == Some(level) {
                return false;
            }
            self.prepared_level = Some(level);
            level == RefinementLevel::Final
        }

        fn prepare_backdrop(&mut self, viewer: &ViewerController) -> Result<bool, AppError> {
            let final_spec = self.plan.level(RefinementLevel::Final);
            let Some(requested_extent) =
                super::backdrop_extent([final_spec.extent.width, final_spec.extent.height])
            else {
                self.active_backdrop_map = None;
                return Ok(false);
            };
            let Some(mut map) = viewer.backdrop_map(requested_extent)? else {
                self.active_backdrop_map = None;
                return Ok(false);
            };
            if let Some(flight) = self
                .backdrop
                .as_ref()
                .and_then(|backdrop| backdrop.in_flight)
            {
                self.active_backdrop_map = Some(flight.map);
                return Ok(true);
            }
            let stamp = self.view_stamp(viewer);
            if self.backdrop.as_ref().is_some_and(|backdrop| {
                backdrop
                    .ready
                    .is_some_and(|ready| ready.stamp.render_equivalent(stamp))
            }) {
                self.active_backdrop_map = None;
                return Ok(false);
            }
            self.ensure_backdrop_grid(requested_extent, viewer.requested().iteration_cap)?;
            let delivered_extent = self
                .backdrop
                .as_ref()
                .map(|backdrop| {
                    let final_spec = backdrop.plan.level(RefinementLevel::Final);
                    [final_spec.extent.width, final_spec.extent.height]
                })
                .ok_or_else(|| AppError::Kernel("backdrop grid was not allocated".to_string()))?;
            if delivered_extent != requested_extent {
                map = viewer.backdrop_map(delivered_extent)?.ok_or_else(|| {
                    AppError::Math("delivered backdrop unexpectedly has no apron".to_string())
                })?;
            }
            self.active_backdrop_map = Some(map);
            Ok(true)
        }

        fn ensure_backdrop_grid(
            &mut self,
            requested_extent: [u32; 2],
            requested_iter_cap: u32,
        ) -> Result<(), AppError> {
            let requested_extent = GridExtent {
                width: requested_extent[0],
                height: requested_extent[1],
            };
            if self.backdrop.as_ref().is_some_and(|backdrop| {
                backdrop.plan.requested_extent == requested_extent
                    && backdrop.plan.requested_max_iter == requested_iter_cap
                    && backdrop.plan.precision_mode == self.precision_mode
            }) {
                return Ok(());
            }
            self.release_backdrop()?;
            let plan = JulibrotKernels::plan(
                &self.executor,
                requested_extent,
                EscapeParams::new(requested_iter_cap),
            )
            .map_err(kernel_error)?
            .with_precision_mode(self.precision_mode);
            let grid = self
                .kernels
                .allocate_grid(&mut self.executor, &plan)
                .map_err(kernel_error)?;
            self.backdrop = Some(BackdropGrid {
                plan,
                grid,
                ready: None,
                in_flight: None,
            });
            Ok(())
        }

        fn release_backdrop(&mut self) -> Result<(), AppError> {
            self.active_backdrop_map = None;
            if let Some(backdrop) = self.backdrop.take() {
                self.kernels
                    .free_grid(&mut self.executor, backdrop.grid)
                    .map_err(kernel_error)?;
            }
            Ok(())
        }

        fn install_main(
            &mut self,
            viewer: &ViewerController,
            object: ObjectAngles,
            plane: Plane,
            map: PoseMap,
        ) {
            let current_stamp = self.view_stamp(viewer);
            let ready_backdrop = self.backdrop.as_ref().and_then(|backdrop| {
                backdrop.ready.filter(|ready| {
                    ready.stamp.render_equivalent(current_stamp)
                        && self.active_backdrop_map.is_none()
                })
            });
            let (grid, source_map, requested_width, delivered_iter_cap, backdrop) =
                if let Some(source_map) = self.active_backdrop_map {
                    let backdrop = self
                        .backdrop
                        .as_ref()
                        .expect("active backdrop map always owns a grid");
                    let mut grid = backdrop.grid.clone();
                    let final_spec = backdrop.plan.level(RefinementLevel::Final);
                    grid.width = final_spec.extent.width;
                    grid.height = final_spec.extent.height;
                    grid.level = RefinementLevel::Final;
                    (
                        grid,
                        source_map,
                        backdrop.plan.requested_extent.width,
                        super::published_iteration_cap(&backdrop.plan),
                        None,
                    )
                } else {
                    let mut grid = self.grid.clone();
                    if let Some(level) = self.prepared_level {
                        let spec = self.plan.level(level);
                        grid.width = spec.extent.width;
                        grid.height = spec.extent.height;
                        grid.level = level;
                    }
                    let backdrop = ready_backdrop.map(|ready| {
                        let stored = self
                            .backdrop
                            .as_ref()
                            .expect("ready backdrop always owns a grid");
                        PresentBackdrop {
                            grid: stored.grid.clone(),
                            iteration_cap: super::published_iteration_cap(&stored.plan),
                            plane,
                            map: ready.map,
                        }
                    });
                    (
                        grid,
                        map,
                        self.plan.requested_extent.width,
                        super::published_iteration_cap(&self.plan),
                        backdrop,
                    )
                };
            self.main.delivered_iter_cap = delivered_iter_cap;
            let facts_pose = (map, [grid.width, grid.height]);
            if self.facts_pose != facts_pose {
                let horizon = super::horizon_facts(map, facts_pose.1);
                self.horizon_pixels = horizon.pixels;
                self.horizon_fraction = horizon.fraction;
                self.uncertain_pixels = horizon.uncertain_pixels;
                self.uncertain_fraction = horizon.uncertain_fraction;
                self.map_condition_number = horizon.condition_number;
                self.edge_on = horizon.edge_on;
                self.facts_pose = facts_pose;
            }
            self.presenter.set_main(PresentMain {
                epoch: self.owner_epoch,
                state: super::main_for_grid(self.main, grid.width, requested_width),
                grid,
                object,
                plane,
                map: source_map,
                backdrop,
            });
        }

        fn observe_scene_selection(&mut self, viewer: &ViewerController) {
            let selection = SceneSelection {
                generation: self.main.generation_applied,
                requested_iter_cap: self.main.requested_iter_cap,
                palette_id: self.main.palette_id,
                plane_origin_f64: self.main.plane_origin_f64,
                precision_mode: self.main.precision_mode,
            };
            if self.scene_ready(viewer.requested().zoom_log2)
                && self.submitted_references.is_empty()
                && viewer.owner().navigation_pending_depth() == 0
                && self
                    .scene_selection
                    .is_some_and(|previous| previous != selection)
            {
                self.loop_state
                    .scene_selection_changed(self.main.generation_applied);
                self.prepared_level = None;
            }
            self.scene_selection = Some(selection);
        }

        /// Stamps the view the next presented image is expected to reproduce.
        fn view_stamp(&self, viewer: &ViewerController) -> ViewStamp {
            let requested = viewer.requested();
            ViewStamp {
                generation_applied: self.main.generation_applied,
                centre_revision: self.main.centre_revision,
                requested_iter_cap: self.main.requested_iter_cap,
                palette_id: self.main.palette_id,
                plane_origin_f64: self.main.plane_origin_f64,
                view: requested.view,
                zoom_log2: requested.zoom_log2,
                object_angles: requested.object_angles,
                map: super::stamped_screen_map(viewer, &self.plan),
                precision_mode: self.main.precision_mode,
            }
        }

        /// Reports whether the image on the canvas belongs to an older requested view.
        ///
        /// Nothing has been presented before the first warp completes, so an unstarted page reads
        /// stale, which is the honest answer: it is showing no view at all.
        #[must_use]
        pub fn presented_view_is_stale(&self, viewer: &ViewerController) -> bool {
            let current = self.view_stamp(viewer);
            self.presented_view
                .is_none_or(|presented| !presented.render_equivalent(current))
        }

        fn prepared_extent(&self) -> [u32; 2] {
            if self.active_backdrop_map.is_some()
                && let Some(backdrop) = &self.backdrop
            {
                let final_spec = backdrop.plan.level(RefinementLevel::Final);
                return [final_spec.extent.width, final_spec.extent.height];
            }
            self.prepared_level
                .map_or([self.grid.width, self.grid.height], |level| {
                    let extent = self.plan.level(level).extent;
                    [extent.width, extent.height]
                })
        }

        fn service_arrivals(
            &mut self,
            viewer: &mut ViewerController,
            now_ms: f64,
        ) -> Result<bool, AppError> {
            let mut applied = false;
            for _ in 0..2 {
                let Some(mut response) = self.owner_endpoint.next_arrival() else {
                    break;
                };
                let generation = response.generation();
                let _finished = viewer.finish_reference_submission(generation);
                let submitted = self
                    .submitted_references
                    .iter()
                    .position(|item| item.generation == generation)
                    .map(|index| self.submitted_references.swap_remove(index));
                let processed = self.process_arrival(viewer, &response, submitted);
                let disposition = processed
                    .as_ref()
                    .map_or(OrbitDisposition::Stale, |result| result.0);
                let credited = self
                    .owner_endpoint
                    .return_credit(&mut response, disposition, now_us(now_ms))
                    .map_err(worker_error);
                let (_, arrival_applied) = processed?;
                credited?;
                applied |= arrival_applied;
            }
            Ok(applied)
        }

        fn process_arrival(
            &mut self,
            viewer: &mut ViewerController,
            response: &ember_julibrot_worker::OrbitResponseView,
            submitted: Option<SubmittedReference>,
        ) -> Result<(OrbitDisposition, bool), AppError> {
            let Some(submitted) = submitted.filter(|submitted| {
                submitted.precision_mode == viewer.requested().precision_mode.as_str()
                    && super::arrival_is_current(
                        response.cancelled(),
                        response.generation(),
                        self.owner_endpoint.latest_generation(),
                        viewer.owner().navigation_pending_depth(),
                    )
            }) else {
                return Ok((OrbitDisposition::Stale, false));
            };
            let records = response
                .records
                .transfer_record_bytes()
                .map_err(worker_error)?;
            expand_reference_texels_from_array(
                &records,
                response.length(),
                &mut self.reference_upload,
            )?;
            let span = self
                .executor
                .allocate_span(response.length(), OUTPUT_PAGE_SIDE)
                .map_err(heap_error)?;
            if let Err(error) = self.executor.write_span(&span, &self.reference_upload) {
                let _freed = self.executor.free_span(span);
                return Err(heap_error(error));
            }
            let registered = RegisteredOrbit {
                span: span.clone(),
                length: response.length(),
                precision_bits: response.precision_bits(),
                precision_mode: submitted.precision_mode,
            };
            let handle = match self.orbits.insert(response.generation(), registered) {
                Ok(handle) => handle,
                Err(error) => {
                    let _freed = self.executor.free_span(span);
                    return Err(registry_error(error));
                }
            };
            let shift = match self
                .accepted_reference
                .as_ref()
                .map_or(Ok([0.0; 2]), |old| {
                    let old = old.with_precision(submitted.centre.precision_bits)?;
                    reference_shift_px(
                        &old,
                        &submitted.centre,
                        &submitted.plane,
                        submitted.zoom_log2,
                        self.plan.requested_extent.width,
                    )
                }) {
                Ok(shift) => shift,
                Err(error) => {
                    self.remove_orbit(handle)?;
                    return Err(math_error(error));
                }
            };
            if let Err(error) = viewer.configure_navigation_context(
                submitted.centre.clone(),
                submitted.centre.clone(),
                submitted.plane,
            ) {
                self.remove_orbit(handle)?;
                return Err(error);
            }
            let disposition = viewer.owner_mut().accept_orbit(response, handle, shift);
            if disposition == OrbitDisposition::Stale {
                self.remove_orbit(handle)?;
                return Ok((disposition, false));
            }
            self.replace_current_orbit(handle)?;
            self.accepted_reference = Some(submitted.centre);
            self.accepted_reference_zoom_log2 = Some(submitted.zoom_log2);
            self.main = viewer.drain_main()?.main;
            self.level_timings.record_worker(
                response.centre_revision(),
                response.compute_us(),
                None,
            );
            self.rebuild_grid_if_needed(viewer.requested().iteration_cap)?;
            self.loop_state.scene_input_ready(response.generation());
            self.prepared_level = None;
            let requested = viewer.requested();
            let map = viewer.screen_map(self.prepared_extent())?;
            let plane = viewer.checked_plane();
            self.install_main(viewer, requested.object_angles, plane, map);
            Ok((disposition, true))
        }

        /// Releases every reference whose typed worker refusal means no arrival can land.
        ///
        /// Without this the scene path stays blocked on a submission that will never return.
        fn abandon_submitted_references(&mut self, viewer: &mut ViewerController) {
            for submitted in std::mem::take(&mut self.submitted_references) {
                let _finished = viewer.finish_reference_submission(submitted.generation);
            }
        }

        fn submit_pending_reference(
            &mut self,
            viewer: &mut ViewerController,
            plane: Plane,
        ) -> Result<bool, AppError> {
            let Some(submission) = viewer.take_reference_submission() else {
                return Ok(false);
            };
            let navigation = submission.navigation;
            let requested = viewer.requested();
            if KernelMode::for_zoom(navigation.zoom_log2) == KernelMode::Shallow {
                if !viewer.owner_mut().accept_navigation_without_orbit(
                    navigation.generation,
                    navigation.centre_revision,
                ) {
                    return Ok(false);
                }
                self.shallow_centre = Some(navigation.centre);
                self.main = viewer.drain_main()?.main;
                self.rebuild_grid_if_needed(requested.iteration_cap)?;
                self.loop_state.scene_input_ready(navigation.generation);
                self.prepared_level = None;
                let map = viewer.screen_map(self.prepared_extent())?;
                self.install_main(viewer, requested.object_angles, plane, map);
                return Ok(true);
            }
            let precision = precision_for(
                navigation.zoom_log2,
                self.plan.requested_extent.width,
                requested.iteration_cap,
            )
            .map_err(math_error)?;
            let centre = EncodedCentre::encode_math(&navigation.centre, navigation.centre_revision)
                .map_err(worker_error)?;
            let request = OrbitRequest::new(
                navigation.generation,
                centre,
                depth_digits(navigation.zoom_log2),
                precision.requested_bits,
                requested.iteration_cap,
                PrecisionMode::from_u32(navigation.precision_mode).ok_or_else(|| {
                    AppError::Worker(format!(
                        "precision mode {} is outside 0..1",
                        navigation.precision_mode
                    ))
                })?,
                submission.reason,
            )
            .map_err(worker_error)?;
            let required_upload = super::reference_texel_bytes(requested.iteration_cap)?;
            if self.reference_upload.capacity() < required_upload {
                self.reference_upload
                    .try_reserve_exact(required_upload.saturating_sub(self.reference_upload.len()))
                    .map_err(|error| {
                        AppError::Worker(format!("reference upload reserve failed: {error}"))
                    })?;
            }
            if self.owner_endpoint.submit(request) == SubmitOutcome::GenerationExhausted {
                let _finished = viewer.finish_reference_submission(navigation.generation);
                return Err(AppError::GenerationExhausted);
            }
            self.submitted_references.push(SubmittedReference {
                generation: navigation.generation,
                centre: navigation.centre,
                zoom_log2: navigation.zoom_log2,
                plane,
                precision_mode: viewer_precision_mode(navigation.precision_mode),
            });
            Ok(false)
        }

        fn rebuild_grid_if_needed(&mut self, requested_max_iter: u32) -> Result<(), AppError> {
            if self.plan.requested_max_iter == requested_max_iter {
                return Ok(());
            }
            self.release_backdrop()?;
            let requested_extent = self.plan.requested_extent;
            let next = JulibrotKernels::plan(
                &self.executor,
                requested_extent,
                EscapeParams::new(requested_max_iter),
            )
            .map_err(kernel_error)?
            .with_precision_mode(self.precision_mode);
            if next == self.plan {
                return Ok(());
            }
            let next_grid = self
                .kernels
                .allocate_grid(&mut self.executor, &next)
                .map_err(kernel_error)?;
            let old_grid = std::mem::replace(&mut self.grid, next_grid);
            self.kernels
                .free_grid(&mut self.executor, old_grid)
                .map_err(kernel_error)?;
            self.plan = next;
            Ok(())
        }

        fn synchronize_precision_mode(
            &mut self,
            viewer: &mut ViewerController,
        ) -> Result<(), AppError> {
            let next = viewer.requested().precision_mode;
            if next == self.precision_mode {
                return Ok(());
            }
            self.release_backdrop()?;
            let next_plan = self.plan.with_precision_mode(next);
            let next_grid = self
                .kernels
                .allocate_grid(&mut self.executor, &next_plan)
                .map_err(kernel_error)?;
            if let Err(error) = super::apply_precision_mode(
                next,
                &mut self.precision_mode,
                &mut self.loop_state,
                &mut self.plan,
                viewer,
            ) {
                self.kernels
                    .free_grid(&mut self.executor, next_grid)
                    .map_err(kernel_error)?;
                return Err(error);
            }
            let old_grid = std::mem::replace(&mut self.grid, next_grid);
            self.kernels
                .free_grid(&mut self.executor, old_grid)
                .map_err(kernel_error)?;
            self.prepared_level = None;
            self.scene_selection = None;
            Ok(())
        }

        fn submit_due_scene(
            &mut self,
            viewer: &ViewerController,
            object: ObjectAngles,
            plane: Plane,
            map: PoseMap,
            slot: HotSlot,
            owner_epoch: u64,
            now_ms: f64,
        ) -> Result<Option<u64>, AppError> {
            if matches!(map, PoseMap::Mapped(_))
                && (!self.submitted_references.is_empty()
                    || viewer.owner().navigation_pending_depth() != 0)
            {
                return Ok(None);
            }
            if !self.scene_ready(viewer.requested().zoom_log2) {
                return Ok(None);
            }
            if self.active_backdrop_map.is_some() {
                return self.submit_due_backdrop(viewer, plane, slot, owner_epoch, now_ms);
            }
            let Some(level) = self.loop_state.due() else {
                return Ok(None);
            };
            if self.prepared_level != Some(level) {
                return Ok(None);
            }
            if matches!(map, PoseMap::EdgeOn) {
                super::stamp_scene_level(&mut self.grid, &self.plan, level);
            }
            let facts = if let PoseMap::Mapped(screen_to_plane) = map {
                let params = EscapeParams::new(viewer.requested().iteration_cap);
                let mut encoder =
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Julibrot kernels SCRATCH and DATA copy"),
                        });
                let mode = KernelMode::for_zoom(viewer.requested().zoom_log2);
                let facts = match mode {
                    KernelMode::Shallow => {
                        let centre = self.shallow_centre.as_ref().ok_or_else(|| {
                            AppError::Kernel("missing shallow centre".to_string())
                        })?;
                        let split = split_centre(centre).map_err(math_error)?;
                        let scale = shallow_pixel_scale(
                            viewer.requested().zoom_log2,
                            self.plan.level(level).extent.width,
                        )
                        .map_err(math_error)?;
                        self.kernels
                            .encode_shallow(
                                &self.executor,
                                &mut encoder,
                                &mut self.grid,
                                owner_epoch,
                                viewer.requested().precision_mode,
                                level,
                                &plane,
                                &screen_to_plane,
                                &split,
                                scale,
                                params,
                            )
                            .map_err(kernel_error)?
                    }
                    KernelMode::Perturbation => {
                        let handle = self.current_orbit.ok_or_else(|| {
                            AppError::Kernel("missing reference orbit".to_string())
                        })?;
                        let orbit = self.orbits.get(handle).map_err(registry_error)?;
                        let scale = scale_split(
                            viewer.requested().zoom_log2,
                            self.plan.level(level).extent.width,
                        )
                        .map_err(math_error)?;
                        self.kernels
                            .encode_perturbation(
                                &self.executor,
                                &mut encoder,
                                &mut self.grid,
                                owner_epoch,
                                viewer.requested().precision_mode,
                                level,
                                &plane,
                                &screen_to_plane,
                                scale,
                                params,
                                ReferenceOrbitInput {
                                    span: &orbit.span,
                                    generation: handle.generation,
                                    length: orbit.length,
                                    precision_bits: orbit.precision_bits,
                                    precision_mode: orbit.precision_mode,
                                },
                            )
                            .map_err(kernel_error)?
                    }
                };
                self.queue.submit([encoder.finish()]);
                Some(facts)
            } else {
                None
            };
            self.main.delivered_iter_cap = super::published_iteration_cap(&self.plan);
            self.install_main(viewer, object, plane, map);
            match self.presenter.submit_scene(slot, now_ms) {
                Ok(scene_id) => {
                    self.last_dispatch = facts;
                    self.level_timings
                        .begin_scene(self.main.centre_revision, scene_id, level);
                    self.loop_state.submitted(scene_id, level);
                    Ok(Some(scene_id))
                }
                Err(ember_julibrot_present::PresentError::SceneBusy { .. }) => Ok(None),
                Err(error) => Err(present_error(error)),
            }
        }

        fn submit_due_backdrop(
            &mut self,
            viewer: &ViewerController,
            plane: Plane,
            slot: HotSlot,
            owner_epoch: u64,
            now_ms: f64,
        ) -> Result<Option<u64>, AppError> {
            if self
                .backdrop
                .as_ref()
                .is_some_and(|backdrop| backdrop.in_flight.is_some())
            {
                return Ok(None);
            }
            let PoseMap::Mapped(screen_to_plane) = self
                .active_backdrop_map
                .ok_or_else(|| AppError::Math("backdrop dispatch has no map".to_string()))?
            else {
                return Ok(None);
            };
            let stamp = self.view_stamp(viewer);
            let params = EscapeParams::new(viewer.requested().iteration_cap);
            let sampling_zoom = super::sampling_zoom_log2(
                viewer.requested().zoom_log2,
                screen_to_plane.apron_scale,
            )?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Julibrot backdrop kernels SCRATCH and DATA copy"),
                });
            let mode = KernelMode::for_zoom(viewer.requested().zoom_log2);
            let facts = {
                let backdrop = self
                    .backdrop
                    .as_mut()
                    .ok_or_else(|| AppError::Kernel("backdrop dispatch has no grid".to_string()))?;
                let level = RefinementLevel::Final;
                match mode {
                    KernelMode::Shallow => {
                        let centre = self.shallow_centre.as_ref().ok_or_else(|| {
                            AppError::Kernel("missing shallow centre".to_string())
                        })?;
                        let split = split_centre(centre).map_err(math_error)?;
                        let scale = shallow_pixel_scale(
                            sampling_zoom,
                            backdrop.plan.level(level).extent.width,
                        )
                        .map_err(math_error)?;
                        self.kernels
                            .encode_shallow(
                                &self.executor,
                                &mut encoder,
                                &mut backdrop.grid,
                                owner_epoch,
                                viewer.requested().precision_mode,
                                level,
                                &plane,
                                &screen_to_plane,
                                &split,
                                scale,
                                params,
                            )
                            .map_err(kernel_error)?
                    }
                    KernelMode::Perturbation => {
                        let handle = self.current_orbit.ok_or_else(|| {
                            AppError::Kernel("missing reference orbit".to_string())
                        })?;
                        let orbit = self.orbits.get(handle).map_err(registry_error)?;
                        let scale =
                            scale_split(sampling_zoom, backdrop.plan.level(level).extent.width)
                                .map_err(math_error)?;
                        self.kernels
                            .encode_perturbation(
                                &self.executor,
                                &mut encoder,
                                &mut backdrop.grid,
                                owner_epoch,
                                viewer.requested().precision_mode,
                                level,
                                &plane,
                                &screen_to_plane,
                                scale,
                                params,
                                ReferenceOrbitInput {
                                    span: &orbit.span,
                                    generation: handle.generation,
                                    length: orbit.length,
                                    precision_bits: orbit.precision_bits,
                                    precision_mode: orbit.precision_mode,
                                },
                            )
                            .map_err(kernel_error)?
                    }
                }
            };
            self.queue.submit([encoder.finish()]);
            match self.presenter.submit_scene(slot, now_ms) {
                Ok(scene_id) => {
                    let backdrop = self.backdrop.as_mut().ok_or_else(|| {
                        AppError::Kernel("submitted backdrop lost its grid".to_string())
                    })?;
                    backdrop.in_flight = Some(BackdropFlight {
                        scene_id,
                        stamp,
                        map: PoseMap::Mapped(screen_to_plane),
                    });
                    self.last_dispatch = Some(facts);
                    self.level_timings.begin_scene(
                        self.main.centre_revision,
                        scene_id,
                        RefinementLevel::Final,
                    );
                    Ok(Some(scene_id))
                }
                Err(ember_julibrot_present::PresentError::SceneBusy { .. }) => Ok(None),
                Err(error) => Err(present_error(error)),
            }
        }

        fn scene_ready(&self, zoom_log2: f64) -> bool {
            match KernelMode::for_zoom(zoom_log2) {
                KernelMode::Shallow => self.shallow_centre.is_some(),
                KernelMode::Perturbation => super::perturbation_reference_is_current(
                    zoom_log2,
                    self.main.generation_applied,
                    self.current_orbit
                        .zip(self.accepted_reference_zoom_log2)
                        .map(|(handle, reference_zoom_log2)| {
                            (handle.generation, reference_zoom_log2)
                        }),
                ),
            }
        }

        fn replace_current_orbit(&mut self, next: OrbitHandle) -> Result<(), AppError> {
            if let Some(previous) = self.current_orbit.replace(next) {
                self.remove_orbit(previous)?;
            }
            Ok(())
        }

        fn remove_orbit(&mut self, handle: OrbitHandle) -> Result<(), AppError> {
            let orbit = self.orbits.remove(handle).map_err(registry_error)?;
            self.executor.free_span(orbit.span).map_err(heap_error)
        }

        fn main_epoch(&self) -> u64 {
            self.owner_epoch
        }

        /// Returns immutable presentation facts.
        #[must_use]
        pub fn present_facts(&self) -> ember_julibrot_present::PresentFacts {
            self.presenter.facts()
        }

        /// Returns the newest kernel arithmetic receipt.
        #[must_use]
        pub const fn dispatch_facts(&self) -> Option<DispatchFacts> {
            self.last_dispatch
        }

        /// Returns current worker ownership and credit facts.
        #[must_use]
        pub fn worker_facts(&self) -> WorkerFacts {
            self.owner_endpoint.facts()
        }

        /// Returns whether cooperative refresh work remains.
        #[must_use]
        pub fn pending(&self, runtime: &BrowserRuntime, viewer: &ViewerController) -> bool {
            let present = self.presenter.facts();
            self.loop_state.needs_refresh(
                present.in_flight_scene_id.is_some(),
                runtime.has_pending_surface(),
                self.owner_endpoint.pending_request_depth() != 0
                    || !self.submitted_references.is_empty()
                    || present.scene_fill_due,
                self.presented_view_is_stale(viewer),
            )
        }

        /// Selects automatic or button-driven scene refinement without changing the current pose.
        pub fn set_scene_mode(&mut self, mode: super::SceneMode) {
            let has_scene = self.presenter.facts().completed_scene_id.is_some();
            self.loop_state
                .set_scene_mode(mode, self.main.generation_applied, has_scene);
            self.prepared_level = None;
        }

        /// Returns the current automatic or manual scene policy.
        #[must_use]
        pub const fn scene_mode(&self) -> super::SceneMode {
            self.loop_state.scene_mode()
        }

        /// Reports a manual pose change not yet covered by a completed requested scene.
        #[must_use]
        pub const fn scene_update_pending(&self) -> bool {
            self.loop_state.scene_update_pending()
        }

        /// Returns how many draft levels an accepted better warp made unnecessary.
        #[must_use]
        pub const fn draft_skipped_count(&self) -> u64 {
            self.loop_state.draft_skipped_count()
        }

        /// Returns the stable reason for the most recent direct-to-Final decision.
        #[must_use]
        pub const fn last_draft_skip_reason(&self) -> Option<&'static str> {
            self.loop_state.last_draft_skip_reason()
        }

        /// Returns the count of transient fence refusals this session survived.
        #[must_use]
        pub const fn transient_refusals(&self) -> u32 {
            self.loop_state.transient_refusals()
        }

        /// Returns the newest transient fence refusal, rendered as its typed text.
        #[must_use]
        pub fn last_transient_refusal(&self) -> Option<String> {
            self.loop_state
                .last_transient()
                .map(std::string::ToString::to_string)
        }

        /// Returns the newest transient refusal without allocating display text.
        #[must_use]
        pub const fn last_transient_error(&self) -> Option<&AppError> {
            self.loop_state.last_transient()
        }

        /// Returns the latched stopping refusal without allocating display text.
        #[must_use]
        pub const fn stopped_error(&self) -> Option<&AppError> {
            self.loop_state.stopped()
        }

        /// Returns cached transient-refusal text with no display allocation.
        #[must_use]
        pub fn last_transient_text(&self) -> Option<std::sync::Arc<str>> {
            self.loop_state
                .last_transient_text()
                .map(std::sync::Arc::clone)
        }

        /// Returns cached stopping-refusal text with no display allocation.
        #[must_use]
        pub fn stopped_text(&self) -> Option<std::sync::Arc<str>> {
            self.loop_state.stopped_text().map(std::sync::Arc::clone)
        }

        /// Returns the latched terminal cause once the loop has stopped.
        #[must_use]
        pub fn stopped_reason(&self) -> Option<String> {
            self.loop_state
                .stopped()
                .map(std::string::ToString::to_string)
        }

        /// Returns the status of the most recent completed refresh turn.
        #[must_use]
        pub const fn last_status(&self) -> RefreshStatus {
            self.last_status
        }

        /// Reports whether a progressive level is due or has a scene fence pending.
        #[must_use]
        pub const fn refinement_pending(&self) -> bool {
            self.loop_state.refinement_pending()
        }

        /// Returns the depth of orbit requests main has handed to the producer.
        ///
        /// Without this the page cannot separate a producer that never admits from an app that
        /// never submits: both leave the credit ledger and the worker facts epoch frozen while the
        /// refresh loop keeps turning.
        #[must_use]
        pub fn worker_request_depth(&self) -> u32 {
            self.owner_endpoint.pending_request_depth()
        }

        /// Returns how many submitted references the app is still waiting on.
        #[must_use]
        pub fn outstanding_reference_count(&self) -> u32 {
            u32::try_from(self.submitted_references.len()).unwrap_or(u32::MAX)
        }

        /// Returns the newest generation the app has submitted and not yet seen return.
        #[must_use]
        pub fn outstanding_reference_generation(&self) -> Option<u32> {
            self.submitted_references
                .iter()
                .map(|submitted| submitted.generation)
                .max()
        }

        /// Returns the second-warp policy selected after its labelled warm-up.
        #[must_use]
        pub const fn frame_policy(&self) -> FramePolicy {
            self.frame_policy.policy()
        }

        /// Returns the latest warp source identity.
        #[must_use]
        pub const fn last_warp_source(&self) -> Option<u64> {
            self.last_warp_source
        }

        /// Returns the bounded per-edit timing records in oldest-to-newest order.
        #[must_use]
        pub const fn level_timings(&self) -> &LevelTimingLedger {
            &self.level_timings
        }

        /// Returns the latest post-fence presented zoom, when known.
        #[must_use]
        pub fn last_presented_zoom_log2(&self) -> Option<f64> {
            self.presented_view.map(|stamp| stamp.zoom_log2)
        }

        /// Returns the zoom captured by the currently accepted reference orbit.
        #[must_use]
        pub const fn accepted_reference_zoom_log2(&self) -> Option<f64> {
            self.accepted_reference_zoom_log2
        }

        /// Returns the live capacity-selected progressive plan.
        #[must_use]
        pub const fn plan(&self) -> RefinementPlan {
            self.plan
        }

        /// Returns the applied backdrop scale and its capacity-selected Final extent.
        #[must_use]
        pub fn backdrop_facts(&self, viewer: &ViewerController) -> Option<(f64, [u32; 2])> {
            let backdrop = self.backdrop.as_ref()?;
            let current_stamp = self.view_stamp(viewer);
            let map = self.active_backdrop_map.or_else(|| {
                backdrop
                    .ready
                    .filter(|ready| ready.stamp.render_equivalent(current_stamp))
                    .map(|ready| ready.map)
            })?;
            let PoseMap::Mapped(map) = map else {
                return None;
            };
            let final_spec = backdrop.plan.level(RefinementLevel::Final);
            Some((
                map.apron_scale,
                [final_spec.extent.width, final_spec.extent.height],
            ))
        }

        /// Returns the share of current grid centres beyond the neutral-height horizon.
        #[must_use]
        pub const fn horizon_fraction(&self) -> f64 {
            self.horizon_fraction
        }

        /// Returns the number of current grid centres beyond the neutral-height horizon.
        #[must_use]
        pub const fn horizon_pixels(&self) -> u64 {
            self.horizon_pixels
        }

        /// Returns the share of mapped centres whose f32 position is uncertified.
        #[must_use]
        pub const fn uncertain_fraction(&self) -> f64 {
            self.uncertain_fraction
        }

        /// Returns the number of mapped centres whose f32 position is uncertified.
        #[must_use]
        pub const fn uncertain_pixels(&self) -> u64 {
            self.uncertain_pixels
        }

        /// Reports the physical edge-on all-sky state.
        #[must_use]
        pub const fn edge_on(&self) -> bool {
            self.edge_on
        }

        /// Returns the infinity-norm condition number of the current screen map.
        #[must_use]
        pub const fn map_condition_number(&self) -> f64 {
            self.map_condition_number
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

/// Keeps the retained DATA grid intact until its relief redraw reaches the surface.
#[cfg(any(target_arch = "wasm32", test))]
const fn defer_scene_until_relief_redraw(relief_redraw: bool, view_stale: bool) -> bool {
    relief_redraw && view_stale
}

/// Holds the last exact redraw while Final overwrites DATA and fills its own retained image.
#[cfg(any(target_arch = "wasm32", test))]
const fn hold_redraw_during_scene(relief_redraw: bool, scene_in_flight: bool) -> bool {
    relief_redraw && scene_in_flight
}

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserFrameLoop;

#[cfg(test)]
mod tests {
    use std::{
        num::NonZeroU32,
        time::{Duration, Instant},
    };

    use ember_julibrot_kernels::{
        EscapeGrid, GridExtent, KernelMode, PerturbUniform, RefinementPlan, SampleStatus,
        perturb_scaled_pixel, plan_refinement,
    };
    use ember_julibrot_math::{
        BigCentre, EscapeParams, Homography, MathError, ObjectAngles, OrbitStep, Plane, PoseMap,
        PrecisionMode, ReferenceOrbitBuilder, ViewControls, precision_for, scale_split,
        screen_to_plane,
    };

    use super::{
        FenceRefusal, FrameLoop, LEVELS, PresenterPoll, REFERENCE_RECORD_BYTES,
        REFERENCE_TEXEL_BYTES, RefinementLevel, RefinementSchedule, RefusalClass, SceneMode,
        SubmissionKind, apply_precision_mode, arrival_is_current, backdrop_extent,
        defer_scene_until_relief_redraw, expand_reference_texels_into, fence_error,
        hold_redraw_during_scene, horizon_facts, main_for_grid,
        perturbation_reference_is_current, published_iteration_cap, sampling_zoom_log2,
        schedule_exposure_fill, stamp_scene_level, stamped_extent, stamped_screen_map,
        view_projection_changed,
    };
    use crate::{AppError, FramePolicy, LevelTimingLedger, ViewerController};
    use ember_julibrot_present::{SampleClass, SubmissionMeasurement, WarpKind};
    use ember_lab_heap::SpanArena;

    /// Poll budget and wall the version-three present configuration refuses at.
    const SCENE_POLLS: u32 = 4_096;
    const SCENE_DEADLINE_MS: f64 = 30_000.0;

    #[test]
    fn zoom_twelve_to_fourteen_waits_for_a_matching_reference_and_finishes_without_glitches() {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 540;
        const CAP: u32 = 512;
        const ZOOM_TWELVE_GENERATION: u32 = 12;
        const ZOOM_FOURTEEN_GENERATION: u32 = 14;
        assert_eq!(KernelMode::for_zoom(12.0), KernelMode::Shallow);
        assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
        assert!(!perturbation_reference_is_current(
            14.0,
            ZOOM_FOURTEEN_GENERATION,
            Some((ZOOM_TWELVE_GENERATION, 12.0))
        ));
        assert!(perturbation_reference_is_current(
            14.0,
            ZOOM_FOURTEEN_GENERATION,
            Some((ZOOM_FOURTEEN_GENERATION, 14.0))
        ));

        let precision = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
        let centre = BigCentre::from_f64(
            [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33],
            precision.requested_bits,
        )
        .expect("finite seahorse centre");
        let mut builder = ReferenceOrbitBuilder::new(&centre, precision, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack(
            Plane {
                basis_u: [0.0, 0.0, 1.0, 0.0],
                basis_v: [0.0, 0.0, 0.0, 1.0],
            },
            &Homography::IDENTITY,
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            orbit.length,
            RefinementLevel::Final,
        )
        .expect("matching-reference uniform");
        let glitch_pixel_count = (0..WIDTH * HEIGHT)
            .filter(|index| {
                SampleStatus::from_f32(
                    perturb_scaled_pixel(&uniforms, &orbit.records, *index)
                        .expect("canonical pixel index")
                        .record
                        .status,
                ) == Some(SampleStatus::Glitch)
            })
            .count();
        assert_eq!(orbit.length, CAP);
        assert_eq!(glitch_pixel_count, 0);
    }

    #[test]
    fn requested_and_owner_hot_zoom_keep_bit_identity_through_every_absolute_reset_path() {
        const EXTENT: [u32; 2] = [960, 540];

        fn assert_zoom_identity(viewer: &mut ViewerController) {
            let requested = viewer.requested().zoom_log2.to_bits();
            assert_eq!(
                viewer
                    .drain_hot(EXTENT)
                    .expect("bit-identical HOT zoom drains")
                    .state
                    .hot
                    .zoom_log2
                    .to_bits(),
                requested
            );
        }

        let mut viewer = ViewerController::new(EXTENT).expect("canonical viewer");
        viewer.set_zoom_log2(12.0).expect("slider zoom");
        assert_zoom_identity(&mut viewer);
        viewer.wheel_zoom(0.375, [37.0, -19.0]).expect("wheel zoom");
        assert_zoom_identity(&mut viewer);
        viewer
            .set_plane_origin([0.0, 0.0, -0.75, 0.1])
            .expect("finite origin reset");
        assert_zoom_identity(&mut viewer);
    }

    #[test]
    fn relief_redraw_precedes_final_and_holds_while_final_overwrites_data() {
        assert!(defer_scene_until_relief_redraw(true, true));
        assert!(!defer_scene_until_relief_redraw(true, false));
        assert!(!defer_scene_until_relief_redraw(false, true));
        assert!(hold_redraw_during_scene(true, true));
        assert!(!hold_redraw_during_scene(true, false));
        assert!(!hold_redraw_during_scene(false, true));
    }

    #[test]
    fn browser_refresh_wires_relief_redraw_before_submission_and_holds_during_scene() {
        let source = include_str!("frame.rs");
        assert!(
            source.contains("let defer_scene_for_redraw = super::defer_scene_until_relief_redraw(")
        );
        assert!(source.contains(
            "let scene_id = if defer_scene_for_redraw && self.active_backdrop_map.is_none() {"
        ));
        assert!(source.contains("let redraw_scene_in_flight = super::hold_redraw_during_scene("));
        assert!(source.contains(
            "if warp_requested && !runtime.has_pending_surface() && !redraw_scene_in_flight {"
        ));
    }

    #[test]
    fn backdrop_extent_spends_at_most_one_quarter_of_final_records() {
        assert_eq!(backdrop_extent([960, 540]), Some([480, 270]));
        assert_eq!(backdrop_extent([961, 541]), Some([480, 270]));
        assert_eq!(backdrop_extent([1, 1]), None);
        for extent in [[960, 540], [961, 541], [2, 2], [4_096, 2_047]] {
            let backdrop = backdrop_extent(extent).expect("fixture admits a backdrop");
            let final_records = u64::from(extent[0]) * u64::from(extent[1]);
            let backdrop_records = u64::from(backdrop[0]) * u64::from(backdrop[1]);
            assert!(backdrop_records <= final_records / 4);
        }
    }

    #[test]
    fn backdrop_sampling_zoom_widens_only_the_coarse_kernel_grid() {
        let zoom = 3.921_825_538_184_839;
        assert_eq!(
            sampling_zoom_log2(zoom, 1.0).expect("identity").to_bits(),
            zoom.to_bits()
        );
        let widened = sampling_zoom_log2(zoom, 5.0).expect("fivefold backdrop");
        assert_eq!(widened, zoom - 5.0_f64.log2());
        assert!(sampling_zoom_log2(zoom, 0.5).is_err());
    }

    #[test]
    fn compact_reference_records_expand_to_zero_padded_rgba_texels() {
        let records = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut texels = Vec::with_capacity(32);
        expand_reference_texels_into(&records, 2, &mut texels).expect("fixture has two records");
        assert_eq!(texels.len(), 32);
        assert_eq!(&texels[..8], &records[..8]);
        assert_eq!(&texels[8..16], &[0; 8]);
        assert_eq!(&texels[16..24], &records[8..]);
        assert_eq!(&texels[24..], &[0; 8]);
        assert!(expand_reference_texels_into(&records, 1, &mut texels).is_err());
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "the requested native performance oracle reports allocations and copied bytes"
    )]
    fn accepted_reference_upload_reuses_scratch_without_copying_the_transfer() {
        let records = vec![7_u8; 4_096 * REFERENCE_RECORD_BYTES];
        let mut scratch = Vec::with_capacity(4_096 * REFERENCE_TEXEL_BYTES);
        let allocation = scratch.as_ptr();
        let capacity = scratch.capacity();
        expand_reference_texels_into(&records, 4_096, &mut scratch)
            .expect("preallocated scratch accepts the maximum policy orbit");
        let after_allocations = usize::from(scratch.as_ptr() != allocation);
        assert_eq!(after_allocations, 0);
        assert_eq!(scratch.capacity(), capacity);
        assert_eq!(scratch.len(), 4_096 * REFERENCE_TEXEL_BYTES);

        let before_allocations = 2;
        let copied_records = std::hint::black_box(records.clone());
        let copied_texels = std::hint::black_box(vec![0_u8; 4_096 * REFERENCE_TEXEL_BYTES]);
        std::hint::black_box((copied_records, copied_texels));
        eprintln!(
            "accepted_reference_upload before_allocations={before_allocations} after_allocations={after_allocations} before_copied_bytes={} after_copied_bytes={}",
            4_096 * REFERENCE_RECORD_BYTES * 2,
            4_096 * REFERENCE_RECORD_BYTES,
        );
    }

    #[test]
    fn inert_distance_five_change_does_not_restart_a_flat_ladder() {
        let before = ViewControls::MANDELBROT_FLAT;
        let after = ViewControls {
            distance_five: 64.0,
            ..before
        };
        let map = |view| {
            PoseMap::Mapped(
                screen_to_plane(&ObjectAngles::IDENTITY, &view, 0.0, 960, 540, 16.0 / 9.0)
                    .expect("faced flat map"),
            )
        };
        assert!(!view_projection_changed(
            before,
            map(before),
            after,
            map(after)
        ));
    }

    #[test]
    fn the_stamped_map_extent_does_not_follow_the_refinement_ladder() {
        let plan = plan_refinement(
            GridExtent {
                width: 960,
                height: 540,
            },
            EscapeParams::new(512),
            |_| true,
        )
        .expect("the ladder fixture has enough capacity")
        .with_precision_mode(PrecisionMode::PictureFast);

        // A near-edge-on Mandelbrot object rotation: well outside the canonical flat pair, so the
        // map is solved at every extent instead of collapsing to the identity.
        let object = ObjectAngles {
            rho_13: 1.5,
            ..ObjectAngles::IDENTITY
        };
        let view = ViewControls::MANDELBROT_FLAT;
        let map = |extent: [u32; 2]| {
            let [width, height] = extent;
            PoseMap::Mapped(
                screen_to_plane(
                    &object,
                    &view,
                    0.0,
                    width,
                    height,
                    f64::from(width) / f64::from(height),
                )
                .expect("the tilted fixture map is invertible"),
            )
        };

        // The ladder's own levels disagree about the map for one unchanged requested view, so a
        // stamp taken at the prepared level reads stale the moment the ladder advances.
        let preview = plan.level(RefinementLevel::Preview).extent;
        let last = plan.level(RefinementLevel::Final).extent;
        assert_ne!([preview.width, preview.height], [last.width, last.height]);
        assert!(view_projection_changed(
            view,
            map([preview.width, preview.height]),
            view,
            map([last.width, last.height])
        ));

        // The stamp is taken at the requested extent, which every level of the plan shares, so an
        // unchanged requested view stays equivalent to itself across the whole ladder.
        assert_eq!(stamped_extent(&plan), [960, 540]);
        assert!(!view_projection_changed(
            view,
            map(stamped_extent(&plan)),
            view,
            map(stamped_extent(&plan))
        ));
        // Nothing the ladder prepares can move it: only Final renders at the stamped extent.
        for level in LEVELS {
            let extent = plan.level(level).extent;
            assert_eq!(
                [extent.width, extent.height] == stamped_extent(&plan),
                level == RefinementLevel::Final,
                "level {level:?}"
            );
        }
    }

    #[test]
    fn browser_refresh_reuses_preview_and_requested_extent_maps() {
        let plan = plan_refinement(
            GridExtent {
                width: 960,
                height: 540,
            },
            EscapeParams::new(512),
            |_| true,
        )
        .expect("the ladder fixture has enough capacity")
        .with_precision_mode(PrecisionMode::PictureFast);
        let preview = plan.level(RefinementLevel::Preview).extent;
        let preview_extent = [preview.width, preview.height];
        let requested_extent = stamped_extent(&plan);
        assert_ne!(preview_extent, requested_extent);

        let mut viewer = ViewerController::new(requested_extent).expect("canonical viewer");
        for _ in 0..120 {
            viewer.drain_hot(preview_extent).expect("Preview HOT map");
            std::hint::black_box(stamped_screen_map(&viewer, &plan));
            assert_eq!(viewer.map_construction_count(), 2);
        }
    }

    #[test]
    fn horizon_fraction_counts_pixel_centres_without_sampling_the_grid() {
        assert_eq!(
            horizon_facts(PoseMap::Mapped(Homography::IDENTITY), [8, 4]).fraction,
            0.0
        );
        let half = Homography {
            rows: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
            ..Homography::IDENTITY
        };
        assert_eq!(
            horizon_facts(PoseMap::Mapped(half), [8, 4]),
            super::HorizonFacts {
                pixels: 16,
                fraction: 0.5,
                uncertain_pixels: 0,
                uncertain_fraction: 0.0,
                condition_number: 1.0,
                edge_on: false,
            }
        );
        let all = Homography {
            rows: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0],
            ..Homography::IDENTITY
        };
        assert_eq!(horizon_facts(PoseMap::Mapped(all), [8, 4]).fraction, 1.0);
        let edge = horizon_facts(PoseMap::EdgeOn, [8, 4]);
        assert_eq!(edge.fraction, 1.0);
        assert!(edge.edge_on);
    }

    #[test]
    fn reference_shift_is_expressed_in_each_level_pixel_scale() {
        let state = ember_julibrot_worker::MainState {
            reference_shift_px: [12.0, -8.0],
            ..ember_julibrot_worker::MainState::default()
        };
        assert_eq!(
            main_for_grid(state, 240, 960).reference_shift_px,
            [3.0, -2.0]
        );
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct PendingFakeScene {
        id: u64,
        generation: u32,
        level: RefinementLevel,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum FakeEvent {
        Completed(PendingFakeScene),
        Deadline(u64),
        WarpCompleted(u64),
        Refused {
            id: u64,
            kind: SubmissionKind,
            reason: FenceRefusal,
            polls: u32,
            wall_ms: f64,
        },
    }

    #[derive(Debug, Default)]
    struct FakePresenter {
        next_id: u64,
        pending: Option<PendingFakeScene>,
        pending_warp: Option<u64>,
        callback: Option<FakeEvent>,
        warp_callback: Option<FakeEvent>,
        fence_observations: u32,
        warp_fence_observations: u32,
        hot_writes: u32,
        submissions: Vec<RefinementLevel>,
        warp_submissions: Vec<u64>,
        presented_warps: Vec<u64>,
        retained_scene: Option<u64>,
        presented_scene: Option<u64>,
        pending_warp_source: Option<u64>,
        refuse_warp: bool,
        warp_kind: Option<WarpKind>,
    }

    impl FakePresenter {
        fn submit(&mut self, generation: u32, level: RefinementLevel) -> u64 {
            self.next_id += 1;
            let scene = PendingFakeScene {
                id: self.next_id,
                generation,
                level,
            };
            self.pending = Some(scene);
            self.submissions.push(level);
            scene.id
        }

        fn write_hot(&mut self, hold_refused_warp: bool) {
            self.hot_writes += 1;
            self.warp_kind = Some(if self.refuse_warp {
                if hold_refused_warp && self.retained_scene.is_some() {
                    WarpKind::HoldStale
                } else {
                    WarpKind::ClearOnly
                }
            } else if self.retained_scene.is_some() {
                WarpKind::AnchorHomography
            } else {
                WarpKind::ClearOnly
            });
        }

        fn submit_warp(&mut self) -> u64 {
            self.next_id += 1;
            self.pending_warp = Some(self.next_id);
            self.pending_warp_source = match self.warp_kind {
                Some(WarpKind::ClearOnly) | None => None,
                Some(_) => self.retained_scene,
            };
            self.warp_submissions.push(self.next_id);
            self.next_id
        }

        fn fire_completed_callback(&mut self) {
            self.callback = self.pending.map(FakeEvent::Completed);
        }

        fn fire_deadline(&mut self) {
            self.callback = self.pending.map(|scene| FakeEvent::Deadline(scene.id));
        }

        fn fire_warp_completed(&mut self) {
            self.warp_callback = self.pending_warp.map(FakeEvent::WarpCompleted);
        }

        fn fire_warp_refusal(&mut self, reason: FenceRefusal, polls: u32, wall_ms: f64) {
            self.warp_callback = self.pending_warp.map(|id| FakeEvent::Refused {
                id,
                kind: SubmissionKind::Warp,
                reason,
                polls,
                wall_ms,
            });
        }
    }

    impl PresenterPoll for FakePresenter {
        type Event = FakeEvent;

        fn poll_once(&mut self, _now_ms: f64) -> Vec<Self::Event> {
            let mut events = Vec::new();
            if self.pending.is_some() {
                self.fence_observations += 1;
                if let Some(event) = self.callback.take() {
                    if let FakeEvent::Completed(scene) = event
                        && (self.retained_scene.is_none() || scene.level == RefinementLevel::Final)
                    {
                        self.retained_scene = Some(scene.id);
                        self.refuse_warp = false;
                    }
                    self.pending = None;
                    events.push(event);
                }
            }
            if self.pending_warp.is_some() {
                self.warp_fence_observations += 1;
                if let Some(event) = self.warp_callback.take() {
                    if matches!(event, FakeEvent::WarpCompleted(_)) {
                        self.presented_scene = self.pending_warp_source.take();
                    }
                    self.pending_warp = None;
                    events.push(event);
                }
            }
            events
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    struct FakeClock {
        now_ms: f64,
    }

    impl FakeClock {
        fn advance(&mut self, elapsed_ms: f64) {
            self.now_ms += elapsed_ms;
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    struct TurnOutcome {
        scene_id: Option<u64>,
        warp_id: Option<u64>,
        presented: bool,
        refused: bool,
    }

    /// Drives one turn in the browser's order: poll, then scene, then surface warp.
    fn drive_turn(
        frame_loop: &mut FrameLoop,
        presenter: &mut FakePresenter,
        clock: FakeClock,
        policy: FramePolicy,
        warps: bool,
    ) -> TurnOutcome {
        let mut outcome = TurnOutcome::default();
        for event in FrameLoop::refresh(presenter, clock.now_ms) {
            match event {
                FakeEvent::Completed(scene) => {
                    frame_loop.completed(scene.id, scene.generation, scene.level);
                }
                FakeEvent::WarpCompleted(id) => {
                    presenter.presented_warps.push(id);
                    outcome.presented = true;
                }
                FakeEvent::Deadline(id) => {
                    let refusal = frame_loop.refused(
                        SubmissionKind::Scene,
                        FenceRefusal::Deadline,
                        id,
                        SCENE_POLLS,
                        SCENE_DEADLINE_MS,
                    );
                    outcome.refused = refusal.class != RefusalClass::Device;
                }
                FakeEvent::Refused {
                    id,
                    kind,
                    reason,
                    polls,
                    wall_ms,
                } => {
                    let refusal = frame_loop.refused(kind, reason, id, polls, wall_ms);
                    outcome.refused = refusal.class != RefusalClass::Device;
                }
            }
        }
        if frame_loop.stopped().is_some() {
            return outcome;
        }
        presenter.write_hot(frame_loop.hold_refused_warp());
        if !outcome.refused
            && let Some(level) = frame_loop.due()
        {
            let id = presenter.submit(frame_loop.generation(), level);
            frame_loop.submitted(id, level);
            outcome.scene_id = Some(id);
        }
        if warps && presenter.pending_warp.is_none() && frame_loop.warp_requested(policy) {
            outcome.warp_id = Some(presenter.submit_warp());
            frame_loop.warp_submitted();
        }
        outcome
    }

    fn drive_refresh(
        frame_loop: &mut FrameLoop,
        presenter: &mut FakePresenter,
        clock: FakeClock,
    ) -> Option<u64> {
        drive_turn(
            frame_loop,
            presenter,
            clock,
            FramePolicy::SingleFrameOnDemand,
            false,
        )
        .scene_id
    }

    fn drive_viewer_harness(
        frame_loop: &mut FrameLoop,
        presenter: &mut FakePresenter,
        clock: FakeClock,
        warps: bool,
    ) -> TurnOutcome {
        drive_turn(
            frame_loop,
            presenter,
            clock,
            FramePolicy::SingleFrameOnDemand,
            warps,
        )
    }

    fn retained_presenter(refuse_warp: bool) -> FakePresenter {
        FakePresenter {
            next_id: 37,
            retained_scene: Some(37),
            presented_scene: Some(37),
            refuse_warp,
            ..FakePresenter::default()
        }
    }

    fn finish_pending_refused_ladder(
        frame_loop: &mut FrameLoop,
        presenter: &mut FakePresenter,
        clock: &mut FakeClock,
    ) -> u64 {
        assert_eq!(
            presenter.pending.map(|scene| scene.level),
            Some(RefinementLevel::Preview)
        );
        presenter.fire_completed_callback();
        clock.advance(1.0);
        let interactive = drive_viewer_harness(frame_loop, presenter, *clock, false);
        assert_eq!(
            presenter.pending.map(|scene| scene.level),
            Some(RefinementLevel::Interactive)
        );
        assert!(interactive.scene_id.is_some());
        presenter.fire_completed_callback();
        clock.advance(1.0);
        let final_turn = drive_viewer_harness(frame_loop, presenter, *clock, false);
        let final_scene = final_turn.scene_id.expect("Final scene is submitted");
        assert_eq!(
            presenter.pending.map(|scene| scene.level),
            Some(RefinementLevel::Final)
        );
        presenter.fire_completed_callback();
        clock.advance(1.0);
        let fill = drive_viewer_harness(frame_loop, presenter, *clock, true);
        assert!(fill.warp_id.is_some());
        assert_eq!(presenter.retained_scene, Some(final_scene));
        assert_eq!(presenter.warp_kind, Some(WarpKind::AnchorHomography));
        presenter.fire_warp_completed();
        clock.advance(1.0);
        assert!(drive_viewer_harness(frame_loop, presenter, *clock, false).presented);
        assert_eq!(presenter.presented_scene, Some(final_scene));
        final_scene
    }

    fn precision_runtime_from_viewer(
        viewer: &mut ViewerController,
    ) -> (PrecisionMode, FrameLoop, RefinementPlan) {
        let mut precision_mode = PrecisionMode::Deterministic;
        let mut frame_loop = FrameLoop::default();
        let mut plan = plan_refinement(
            GridExtent {
                width: 960,
                height: 540,
            },
            EscapeParams::new(4_096),
            |_| true,
        )
        .expect("the native precision fixture has enough capacity");
        apply_precision_mode(
            viewer.requested().precision_mode,
            &mut precision_mode,
            &mut frame_loop,
            &mut plan,
            viewer,
        )
        .expect("the viewer precision request is applicable");
        (precision_mode, frame_loop, plan)
    }

    fn complete_preview(frame_loop: &mut FrameLoop, id: u64) {
        let generation = frame_loop.generation();
        frame_loop.submitted(id, RefinementLevel::Preview);
        assert!(frame_loop.completed(id, generation, RefinementLevel::Preview));
    }

    /// Runs the Preview, Interactive, Final ladder to its end with a warp on every turn.
    fn run_ladder_to_idle(
        frame_loop: &mut FrameLoop,
        presenter: &mut FakePresenter,
        clock: &mut FakeClock,
        policy: FramePolicy,
    ) {
        for _ in 0..=LEVELS.len() {
            drive_turn(frame_loop, presenter, *clock, policy, true);
            presenter.fire_completed_callback();
            presenter.fire_warp_completed();
            clock.advance(16.0);
        }
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "the requested fake-clock oracle reports acceptance-to-scene latency"
    )]
    fn accepted_deep_reference_submits_its_first_scene_in_the_accepting_refresh() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let clock = FakeClock::default();
        let accepted_at = clock.now_ms;

        frame_loop.scene_input_ready(7);
        let scene_id = drive_refresh(&mut frame_loop, &mut presenter, clock)
            .expect("the accepting refresh submits its scheduled scene");
        let after_ms = clock.now_ms - accepted_at;

        assert_eq!(presenter.submissions, [RefinementLevel::Preview]);
        assert_eq!(scene_id, 1);
        assert_eq!(after_ms, 0.0);
        eprintln!(
            "accepted_reference_first_scene before_ms={:.6} after_ms={after_ms:.6}",
            1_000.0 / 60.0,
        );
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "the requested native facts harness reports structural snapshot allocations"
    )]
    fn facts_refresh_reuses_cached_text_and_borrows_the_timing_ledger() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.record_transient(AppError::Deadline {
            operation: "facts allocation harness",
            deadline_ms: 1.0,
        });
        let cached = std::sync::Arc::clone(
            frame_loop
                .last_transient_text()
                .expect("the refusal cached its display text"),
        );
        frame_loop.stop(AppError::Deadline {
            operation: "facts stopping harness",
            deadline_ms: 2.0,
        });
        let stopped = std::sync::Arc::clone(
            frame_loop
                .stopped_text()
                .expect("the stop cached its display text"),
        );
        let timings = LevelTimingLedger::default();
        let timing_address = std::ptr::from_ref(&timings);

        for _ in 0..120 {
            let text = std::sync::Arc::clone(
                frame_loop
                    .last_transient_text()
                    .expect("cached text remains available"),
            );
            assert!(std::sync::Arc::ptr_eq(&cached, &text));
            let stopped_text = std::sync::Arc::clone(
                frame_loop
                    .stopped_text()
                    .expect("cached stop text remains available"),
            );
            assert!(std::sync::Arc::ptr_eq(&stopped, &stopped_text));
            assert_eq!(std::ptr::from_ref(&timings), timing_address);
        }

        let before_allocations = 11;
        let after_allocations = 0;
        eprintln!(
            "page_facts_snapshot before_allocations_at_least={before_allocations} after_allocations={after_allocations} refreshes=120"
        );
    }

    #[test]
    fn headless_frame_loop_populates_a_per_level_timing_record() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        let mut timings = LevelTimingLedger::default();
        frame_loop.accept_request(7, true);
        let scene_id = drive_refresh(&mut frame_loop, &mut presenter, clock)
            .expect("headless Preview submission");
        timings.record_worker(11, 1_250, None);
        timings.begin_scene(11, scene_id, RefinementLevel::Preview);
        presenter.fire_completed_callback();
        clock.advance(2.5);
        let _next_scene = drive_refresh(&mut frame_loop, &mut presenter, clock);
        timings.complete_scene(
            scene_id,
            SubmissionMeasurement {
                kind: SubmissionKind::Scene,
                id: scene_id,
                source_scene_id: None,
                sample_class: SampleClass::Measured,
                precision_mode: PrecisionMode::Deterministic.as_str(),
                wall_ms: 2.5,
                fence_wait_ms: 2.0,
                polls: 2,
            },
        );
        timings.complete_warp(SubmissionMeasurement {
            kind: SubmissionKind::Warp,
            id: 99,
            source_scene_id: Some(scene_id),
            sample_class: SampleClass::Measured,
            precision_mode: PrecisionMode::Deterministic.as_str(),
            wall_ms: 0.75,
            fence_wait_ms: 0.5,
            polls: 1,
        });
        assert_eq!(presenter.submissions[0], RefinementLevel::Preview);
        assert_eq!(timings.records().len(), 1);
        let records = timings.records();
        let record = records.first().expect("one timing record");
        assert_eq!(record.scene_us, Some(2_500));
        assert_eq!(record.warp_us, Some(750));
        assert_eq!(record.worker_reference_us, Some(1_250));
        assert_eq!(record.dispatch_us, None);
        assert_eq!(record.credit_wait_us, None);
        assert!(!record.discarded);
    }

    #[test]
    fn published_main_cap_holds_for_the_whole_ladder() {
        for requested in [64_u32, 512, 4_096, 8_192] {
            let plan = plan_refinement(
                GridExtent {
                    width: 960,
                    height: 540,
                },
                EscapeParams::new(requested),
                |_| true,
            )
            .expect("a 960 by 540 plan with unlimited capacity is representable");
            assert_eq!(published_iteration_cap(&plan), requested.min(4_096));
        }
    }

    #[test]
    fn the_per_level_cap_the_app_must_not_publish_changes_between_levels() {
        let plan = plan_refinement(
            GridExtent {
                width: 960,
                height: 540,
            },
            EscapeParams::new(512),
            |_| true,
        )
        .expect("a 960 by 540 plan with unlimited capacity is representable");
        let level_caps = LEVELS.map(|level| plan.level(level).iteration_cap);
        assert_eq!(level_caps, [64, 256, 512]);
        assert!(
            level_caps
                .iter()
                .any(|cap| *cap != published_iteration_cap(&plan)),
            "a per-level cap would look like a new selection to present"
        );
    }

    #[test]
    fn coalesced_navigation_makes_an_endpoint_current_arrival_stale() {
        assert!(arrival_is_current(false, 7, 7, 0));
        assert!(!arrival_is_current(false, 7, 7, 1));
        assert!(!arrival_is_current(false, 7, 8, 0));
        assert!(!arrival_is_current(true, 7, 7, 0));
    }

    #[test]
    fn refinement_advances_only_after_matching_completed_scenes() {
        let mut schedule = RefinementSchedule::default();
        schedule.restart(7);
        assert_eq!(schedule.due(), Some(RefinementLevel::Preview));
        schedule.submitted(11, RefinementLevel::Preview);
        assert_eq!(schedule.due(), None);
        assert!(!schedule.completed(10, 7, RefinementLevel::Preview));
        assert!(schedule.completed(11, 7, RefinementLevel::Preview));
        assert_eq!(schedule.due(), Some(RefinementLevel::Interactive));
        schedule.submitted(12, RefinementLevel::Interactive);
        assert!(schedule.completed(12, 7, RefinementLevel::Interactive));
        assert_eq!(schedule.due(), Some(RefinementLevel::Final));
        schedule.submitted(13, RefinementLevel::Final);
        assert!(schedule.completed(13, 7, RefinementLevel::Final));
        assert!(!schedule.pending());
    }

    #[test]
    fn picture_fast_advances_directly_from_preview_to_final() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.apply_precision_mode(PrecisionMode::PictureFast, 8);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        frame_loop.submitted(21, RefinementLevel::Preview);
        assert!(frame_loop.completed(21, 8, RefinementLevel::Preview));
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
        frame_loop.submitted(22, RefinementLevel::Final);
        assert!(frame_loop.completed(22, 8, RefinementLevel::Final));
        assert!(!frame_loop.refinement_pending());
    }

    #[test]
    fn picture_fast_viewer_builds_the_fast_ladder_and_centre_policy() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        let (precision_mode, mut frame_loop, plan) = precision_runtime_from_viewer(&mut viewer);
        assert_eq!(precision_mode, PrecisionMode::PictureFast);
        assert_eq!(plan.precision_mode, PrecisionMode::PictureFast);
        assert_eq!(plan.level(RefinementLevel::Preview).iteration_cap, 32);
        assert_eq!(
            viewer
                .owner()
                .navigation_centre()
                .expect("configured centre")
                .precision_bits,
            64
        );
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        complete_preview(&mut frame_loop, 31);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
    }

    #[test]
    fn viewer_mode_changes_reapply_the_ladder_plan_and_centre_width() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        let (mut precision_mode, mut frame_loop, mut plan) =
            precision_runtime_from_viewer(&mut viewer);

        viewer
            .set_precision_mode(PrecisionMode::Deterministic)
            .expect("the page mode path accepts deterministic");
        apply_precision_mode(
            viewer.requested().precision_mode,
            &mut precision_mode,
            &mut frame_loop,
            &mut plan,
            &mut viewer,
        )
        .expect("deterministic mode applies everywhere");
        assert_eq!(precision_mode, PrecisionMode::Deterministic);
        assert_eq!(plan.precision_mode, PrecisionMode::Deterministic);
        assert_eq!(
            viewer
                .owner()
                .navigation_centre()
                .expect("configured centre")
                .precision_bits,
            1_024
        );
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        complete_preview(&mut frame_loop, 41);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Interactive));

        viewer
            .set_precision_mode(PrecisionMode::PictureFast)
            .expect("the page mode path accepts picture-fast");
        apply_precision_mode(
            viewer.requested().precision_mode,
            &mut precision_mode,
            &mut frame_loop,
            &mut plan,
            &mut viewer,
        )
        .expect("picture-fast mode applies everywhere");
        assert_eq!(precision_mode, PrecisionMode::PictureFast);
        assert_eq!(plan.precision_mode, PrecisionMode::PictureFast);
        assert_eq!(
            viewer
                .owner()
                .navigation_centre()
                .expect("configured centre")
                .precision_bits,
            64
        );
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        complete_preview(&mut frame_loop, 42);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));

        viewer
            .set_precision_mode(PrecisionMode::Deterministic)
            .expect("the page mode path switches back to deterministic");
        apply_precision_mode(
            viewer.requested().precision_mode,
            &mut precision_mode,
            &mut frame_loop,
            &mut plan,
            &mut viewer,
        )
        .expect("deterministic mode is restored everywhere");
        assert_eq!(precision_mode, PrecisionMode::Deterministic);
        assert_eq!(plan.precision_mode, PrecisionMode::Deterministic);
        assert_eq!(
            viewer
                .owner()
                .navigation_centre()
                .expect("configured centre")
                .precision_bits,
            1_024
        );
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        complete_preview(&mut frame_loop, 43);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Interactive));
    }

    fn wait_for_unused_shallow_orbit() -> Result<(), MathError> {
        let centre = BigCentre::from_f64([0.0, 0.0, -0.5, 0.5], 1_024)?;
        let params = EscapeParams::new(4_096);
        let mut builder =
            ReferenceOrbitBuilder::new(&centre, precision_for(0.0, 960, params.max_iter)?, params)?;
        let chunk = NonZeroU32::new(params.max_iter).ok_or(MathError::InvalidMaxIter)?;
        loop {
            match builder.step(chunk)? {
                OrbitStep::Pending { .. } => {}
                OrbitStep::Complete(orbit) => {
                    std::hint::black_box(orbit);
                    return Ok(());
                }
            }
        }
    }

    fn time_to_first_shallow_scene(wait_for_orbit: bool) -> Result<Duration, MathError> {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let clock = FakeClock::default();
        let started = Instant::now();
        if wait_for_orbit {
            wait_for_unused_shallow_orbit()?;
        }
        frame_loop.accept_request(1, true);
        assert_eq!(KernelMode::for_zoom(0.0), KernelMode::Shallow);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(1)
        );
        Ok(started.elapsed())
    }

    fn median(mut samples: Vec<Duration>) -> Duration {
        samples.sort_unstable();
        samples[samples.len() / 2]
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "the requested native performance oracle reports its before and after walls"
    )]
    fn shallow_frame_loop_no_longer_waits_for_an_unused_orbit() -> Result<(), MathError> {
        wait_for_unused_shallow_orbit()?;
        let before = median(
            (0..5)
                .map(|_| time_to_first_shallow_scene(true))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let after = median(
            (0..5)
                .map(|_| time_to_first_shallow_scene(false))
                .collect::<Result<Vec<_>, _>>()?,
        );
        assert!(before > after);
        eprintln!(
            "first_shallow_scene before_ms={:.6} after_ms={:.6} saved_ms={:.6}",
            before.as_secs_f64() * 1_000.0,
            after.as_secs_f64() * 1_000.0,
            before.saturating_sub(after).as_secs_f64() * 1_000.0,
        );
        Ok(())
    }

    #[test]
    fn a_shallow_selection_is_ready_without_an_orbit_but_a_deep_crossing_waits() {
        let mut shallow = FrameLoop::default();
        shallow.accept_request(7, true);
        assert_eq!(shallow.due(), Some(RefinementLevel::Preview));

        let mut deep = FrameLoop::default();
        deep.accept_request(8, false);
        assert_eq!(deep.due(), None);
        deep.restart(8);
        assert_eq!(deep.due(), Some(RefinementLevel::Preview));
        assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
    }

    #[test]
    fn newer_main_keeps_old_in_flight_work_from_advancing_it() {
        let mut schedule = RefinementSchedule::default();
        schedule.restart(3);
        schedule.submitted(21, RefinementLevel::Preview);
        schedule.restart(4);
        assert_eq!(schedule.due(), None);
        assert!(!schedule.completed(21, 3, RefinementLevel::Preview));
        assert_eq!(schedule.due(), Some(RefinementLevel::Preview));
    }

    #[test]
    fn refusal_retries_the_same_level_without_a_third_target() {
        let mut schedule = RefinementSchedule::default();
        schedule.restart(9);
        schedule.submitted(31, RefinementLevel::Preview);
        assert!(schedule.retired(31));
        assert_eq!(schedule.due(), Some(RefinementLevel::Preview));
    }

    #[test]
    fn pending_fence_is_observed_once_per_refresh_and_completes_after_callback() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        frame_loop.accept_request(7, true);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(1)
        );

        for _ in 0..3 {
            clock.advance(4.0);
            assert_eq!(drive_refresh(&mut frame_loop, &mut presenter, clock), None);
        }
        assert_eq!(presenter.fence_observations, 3);
        assert_eq!(presenter.submissions, [RefinementLevel::Preview]);

        presenter.fire_completed_callback();
        assert_eq!(presenter.submissions, [RefinementLevel::Preview]);
        clock.advance(4.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(2)
        );
        assert_eq!(presenter.fence_observations, 4);
        assert_eq!(
            presenter.submissions,
            [RefinementLevel::Preview, RefinementLevel::Interactive]
        );
    }

    #[test]
    fn deadline_refusal_resubmits_the_same_level_on_the_following_refresh() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        frame_loop.accept_request(11, true);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(1)
        );

        presenter.fire_deadline();
        clock.advance(30_000.0);
        assert_eq!(drive_refresh(&mut frame_loop, &mut presenter, clock), None);
        assert_eq!(presenter.submissions, [RefinementLevel::Preview]);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));

        clock.advance(1.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(2)
        );
        assert_eq!(
            presenter.submissions,
            [RefinementLevel::Preview, RefinementLevel::Preview]
        );
    }

    #[test]
    fn app_needs_refresh_while_either_fence_is_in_flight() {
        let frame_loop = FrameLoop::default();
        assert!(frame_loop.needs_refresh(true, false, false, false));
        assert!(frame_loop.needs_refresh(false, true, false, false));
        assert!(!frame_loop.needs_refresh(false, false, false, false));

        let mut scheduled = FrameLoop::default();
        scheduled.restart(5);
        assert!(scheduled.refinement_pending());
        assert!(scheduled.needs_refresh(false, false, false, false));
    }

    #[test]
    fn viewer_harness_holds_manual_refusal_until_update_scene_presents_final() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.set_scene_mode(SceneMode::Manual, 37, true);
        frame_loop.accept_request(37, true);
        let mut presenter = retained_presenter(true);
        let mut clock = FakeClock::default();

        let held = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
        assert_eq!(held.scene_id, None);
        assert!(held.warp_id.is_some());
        assert_eq!(presenter.warp_kind, Some(WarpKind::HoldStale));
        assert_eq!(presenter.pending_warp_source, Some(37));
        presenter.fire_warp_completed();
        clock.advance(1.0);
        assert!(drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false).presented);
        assert_eq!(presenter.presented_scene, Some(37));
        assert!(frame_loop.scene_update_pending());

        frame_loop.request_scene_update(37);
        let update = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false);
        assert!(update.scene_id.is_some());
        let final_scene =
            finish_pending_refused_ladder(&mut frame_loop, &mut presenter, &mut clock);
        assert_ne!(final_scene, 37);
        assert!(!frame_loop.scene_update_pending());
    }

    #[test]
    fn viewer_harness_keeps_auto_clear_then_final_fill_for_the_same_refusal() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.accept_request(37, true);
        let mut presenter = retained_presenter(true);
        let mut clock = FakeClock::default();

        let cleared = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
        assert!(cleared.scene_id.is_some());
        assert!(cleared.warp_id.is_some());
        assert_eq!(presenter.warp_kind, Some(WarpKind::ClearOnly));
        assert_eq!(presenter.pending_warp_source, None);
        presenter.fire_warp_completed();
        clock.advance(1.0);
        assert!(drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false).presented);
        assert_eq!(presenter.presented_scene, None);

        let final_scene =
            finish_pending_refused_ladder(&mut frame_loop, &mut presenter, &mut clock);
        assert_ne!(final_scene, 37);
    }

    #[test]
    fn viewer_harness_keeps_bounded_manual_warps_moving_the_retained_picture() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.set_scene_mode(SceneMode::Manual, 37, true);
        frame_loop.accept_request(37, true);
        let mut presenter = retained_presenter(false);
        let mut clock = FakeClock::default();

        let bounded = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
        assert!(bounded.warp_id.is_some());
        assert_eq!(presenter.warp_kind, Some(WarpKind::AnchorHomography));
        assert_eq!(presenter.pending_warp_source, Some(37));
        presenter.fire_warp_completed();
        clock.advance(1.0);
        assert!(drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false).presented);
        assert_eq!(presenter.presented_scene, Some(37));
    }

    #[test]
    fn manual_control_change_writes_hot_and_schedules_no_scene() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let clock = FakeClock::default();
        frame_loop.set_scene_mode(SceneMode::Manual, 7, true);
        assert!(frame_loop.hold_refused_warp());
        frame_loop.accept_request(7, true);
        frame_loop.scene_selection_changed(7);
        assert!(!frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, true))));

        let turn = drive_turn(
            &mut frame_loop,
            &mut presenter,
            clock,
            FramePolicy::SingleFrameOnDemand,
            true,
        );
        assert_eq!(presenter.hot_writes, 1);
        assert_eq!(turn.scene_id, None);
        assert!(
            turn.warp_id.is_some(),
            "the changed HOT pose is still reprojected"
        );
        assert!(frame_loop.scene_update_pending());
        assert!(!frame_loop.refinement_pending());

        presenter.fire_warp_completed();
        let completed = drive_turn(
            &mut frame_loop,
            &mut presenter,
            clock,
            FramePolicy::SingleFrameOnDemand,
            false,
        );
        assert!(completed.presented);
        assert!(!frame_loop.needs_refresh(false, false, false, false));
        assert!(frame_loop.scene_update_pending());
    }

    #[test]
    fn manual_update_scene_restarts_the_current_ladder() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        frame_loop.set_scene_mode(SceneMode::Manual, 11, true);
        frame_loop.accept_request(11, true);
        frame_loop.request_scene_update(11);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(1)
        );

        presenter.fire_completed_callback();
        clock.advance(1.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(2)
        );
        assert!(!frame_loop.scene_update_pending());
        presenter.fire_completed_callback();
        clock.advance(1.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(3)
        );
        presenter.fire_completed_callback();
        clock.advance(1.0);
        assert_eq!(drive_refresh(&mut frame_loop, &mut presenter, clock), None);
        assert_eq!(presenter.submissions, LEVELS);
        assert!(!frame_loop.refinement_pending());
    }

    #[test]
    fn manual_main_change_keeps_its_orbit_request_without_a_scene() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        let initial = viewer
            .take_reference_submission()
            .expect("startup requests its first orbit");
        assert!(viewer.finish_reference_submission(initial.navigation.generation));
        viewer
            .set_plane_origin([0.0, 0.0, -0.75, 0.1])
            .expect("finite origin");
        assert!(
            viewer.take_reference_submission().is_some(),
            "manual scene policy does not consume or suppress MAIN orbit work"
        );

        let mut frame_loop = FrameLoop::default();
        frame_loop.set_scene_mode(SceneMode::Manual, 13, true);
        frame_loop.accept_request(13, true);
        frame_loop.scene_input_ready(14);
        assert_eq!(frame_loop.due(), None);
        assert!(frame_loop.needs_refresh(false, false, true, false));
    }

    #[test]
    fn enabling_auto_with_a_manual_change_schedules_preview() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.set_scene_mode(SceneMode::Manual, 17, true);
        frame_loop.accept_request(17, true);
        assert!(frame_loop.scene_update_pending());
        frame_loop.set_scene_mode(SceneMode::Auto, 17, true);
        assert_eq!(frame_loop.scene_mode(), SceneMode::Auto);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        assert!(!frame_loop.scene_update_pending());
    }

    #[test]
    fn auto_mode_and_the_first_manual_scene_keep_automatic_refinement() {
        let mut automatic = FrameLoop::default();
        automatic.accept_request(19, true);
        assert_eq!(automatic.due(), Some(RefinementLevel::Preview));

        let mut first_scene = FrameLoop::default();
        first_scene.restart(23);
        first_scene.set_scene_mode(SceneMode::Manual, 23, false);
        assert_eq!(first_scene.due(), Some(RefinementLevel::Preview));
    }

    #[test]
    fn exposed_accepted_final_warp_submits_final_directly_and_returns_to_idle() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.schedule.precision_mode = PrecisionMode::PictureFast;
        frame_loop.accept_request(31, true);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        assert!(frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, true))));
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
        assert_eq!(frame_loop.draft_skipped_count(), 1);
        assert_eq!(
            frame_loop.last_draft_skip_reason(),
            Some("accepted exposed higher-level retained warp")
        );

        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(1)
        );
        assert_eq!(presenter.submissions, [RefinementLevel::Final]);
        presenter.fire_completed_callback();
        clock.advance(1.0);
        let warp = drive_turn(
            &mut frame_loop,
            &mut presenter,
            clock,
            FramePolicy::SingleFrameOnDemand,
            true,
        );
        assert_eq!(warp.scene_id, None);
        assert!(warp.warp_id.is_some());
        presenter.fire_warp_completed();
        clock.advance(1.0);
        let settled = drive_turn(
            &mut frame_loop,
            &mut presenter,
            clock,
            FramePolicy::SingleFrameOnDemand,
            false,
        );
        assert!(settled.presented);
        assert!(!frame_loop.needs_refresh(false, false, false, false));
    }

    #[test]
    fn refused_warp_and_first_scene_run_the_full_ladder() {
        let mut refused = FrameLoop::default();
        assert!(!refused.hold_refused_warp());
        refused.accept_request(37, true);
        assert!(!refused.skip_drafts_for_accepted_warp(None));
        assert_eq!(refused.due(), Some(RefinementLevel::Preview));

        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        run_ladder_to_idle(
            &mut refused,
            &mut presenter,
            &mut clock,
            FramePolicy::SingleFrameOnDemand,
        );
        assert_eq!(presenter.submissions, LEVELS);
        assert_eq!(refused.draft_skipped_count(), 0);
        assert_eq!(refused.last_draft_skip_reason(), None);
    }

    #[test]
    fn deterministic_accepted_warp_skips_both_draft_levels() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.accept_request(41, true);
        assert!(frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, false))));
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
        assert_eq!(frame_loop.draft_skipped_count(), 2);
        assert_eq!(
            frame_loop.last_draft_skip_reason(),
            Some("accepted covering higher-level retained warp")
        );
    }

    #[test]
    fn edge_on_final_completes_once_then_stays_idle_in_both_scene_modes() {
        const SETTLED_REFRESHES: usize = 16;
        let plan = plan_refinement(
            GridExtent {
                width: 8,
                height: 8,
            },
            EscapeParams::new(64),
            |_| true,
        )
        .expect("the edge-on fixture has enough capacity");
        let mut arena = SpanArena::new(8, 2, 8, 256, 8).expect("fixture arena");
        let span = arena.allocate_span(64, 8).expect("fixture grid span");
        let edge_on_object = ObjectAngles {
            rho_13: std::f64::consts::FRAC_PI_2,
            ..ObjectAngles::IDENTITY
        };
        assert!(
            screen_to_plane(
                &edge_on_object,
                &ViewControls::MANDELBROT_FLAT,
                0.0,
                8,
                8,
                1.0,
            )
            .is_err(),
            "the exact browser fixture must take the all-sky EdgeOn path"
        );

        for mode in [SceneMode::Auto, SceneMode::Manual] {
            let mut frame_loop = FrameLoop::default();
            if mode == SceneMode::Manual {
                frame_loop.set_scene_mode(mode, 29, false);
            }
            frame_loop.schedule.generation = 29;
            frame_loop.schedule.next = Some(RefinementLevel::Final);

            // EdgeOn skips the kernel encoder, so the grid still carries the preceding level until
            // app stamps the scheduled all-sky scene explicitly.
            let mut grid = EscapeGrid {
                span: span.clone(),
                width: plan.level(RefinementLevel::Preview).extent.width,
                height: plan.level(RefinementLevel::Preview).extent.height,
                level: RefinementLevel::Preview,
            };
            let scheduled = frame_loop.due().expect("Final is due");
            assert_ne!(grid.level, scheduled);
            stamp_scene_level(&mut grid, &plan, scheduled);

            let mut presenter = FakePresenter::default();
            let scene_id = presenter.submit(frame_loop.generation(), grid.level);
            frame_loop.submitted(scene_id, scheduled);
            presenter.fire_completed_callback();
            let mut clock = FakeClock::default();
            clock.advance(1.0);

            for _ in 0..SETTLED_REFRESHES {
                let outcome = drive_turn(
                    &mut frame_loop,
                    &mut presenter,
                    clock,
                    FramePolicy::SingleFrameOnDemand,
                    false,
                );
                assert_eq!(outcome.scene_id, None, "mode {mode:?}");
                assert!(!frame_loop.refinement_pending(), "mode {mode:?}");
                assert!(!frame_loop.scene_update_pending(), "mode {mode:?}");
                assert!(
                    !frame_loop.needs_refresh(false, false, false, false),
                    "mode {mode:?}"
                );
                clock.advance(1.0);
            }
            assert_eq!(presenter.submissions, [RefinementLevel::Final]);
        }
    }

    #[test]
    fn a_stale_presented_view_alone_keeps_the_loop_scheduled() {
        let frame_loop = FrameLoop::default();
        assert!(
            !frame_loop.needs_refresh(false, false, false, false),
            "an idle loop showing the requested view has nothing to do"
        );
        assert!(
            frame_loop.needs_refresh(false, false, false, true),
            "an image belonging to an older requested view is unfinished work"
        );
    }

    #[test]
    fn one_frame_request_after_idle_starts_a_new_scene_ladder() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        frame_loop.accept_request(13, true);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(1)
        );

        for expected in [Some(2), Some(3), None] {
            presenter.fire_completed_callback();
            clock.advance(1.0);
            assert_eq!(
                drive_refresh(&mut frame_loop, &mut presenter, clock),
                expected
            );
        }
        frame_loop.warp_submitted();
        assert!(!frame_loop.needs_refresh(false, false, false, false));
        assert!(!frame_loop.warp_requested(FramePolicy::SingleFrameOnDemand));

        frame_loop.accept_request(13, true);
        assert!(frame_loop.needs_refresh(false, false, false, false));
        assert!(frame_loop.warp_requested(FramePolicy::SingleFrameOnDemand));
        clock.advance(1_000.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(4)
        );
        assert_eq!(
            presenter.submissions.last(),
            Some(&RefinementLevel::Preview)
        );
    }

    #[test]
    fn an_exposed_warp_starts_the_scene_that_fills_its_edge() {
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        frame_loop.accept_request(17, true);
        run_ladder_to_idle(
            &mut frame_loop,
            &mut presenter,
            &mut clock,
            FramePolicy::SingleFrameOnDemand,
        );
        assert!(!frame_loop.refinement_pending());

        assert!(schedule_exposure_fill(&mut frame_loop, true, 17));
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        let preview_id = presenter.next_id + 1;
        clock.advance(1.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(preview_id)
        );
        presenter.fire_completed_callback();
        let interactive_id = presenter.next_id + 1;
        clock.advance(1.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            Some(interactive_id)
        );
        assert_eq!(presenter.submissions[3], RefinementLevel::Preview);
        assert_eq!(presenter.submissions[4], RefinementLevel::Interactive);
    }

    #[test]
    fn an_exposed_manual_warp_waits_for_update_without_latching_the_loop() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.set_scene_mode(SceneMode::Manual, 18, true);
        assert!(!schedule_exposure_fill(&mut frame_loop, true, 18));
        assert_eq!(frame_loop.due(), None);
        assert!(frame_loop.scene_update_pending());
        assert!(!frame_loop.needs_refresh(false, false, false, false));
    }

    #[test]
    fn the_page_reads_the_deadline_refusal_that_killed_the_throttled_tab() {
        assert_eq!(
            fence_error(SubmissionKind::Warp, FenceRefusal::Deadline, 7, 60_000.0).to_string(),
            "warp fence exceeded its 60000 ms deadline"
        );
        assert_eq!(
            classified(FenceRefusal::Deadline),
            RefusalClass::Transient,
            "a bounded wall says the observation window closed, not that the device is gone"
        );
        assert_eq!(classified(FenceRefusal::PollLimit), RefusalClass::Transient);
        assert_eq!(classified(FenceRefusal::Cancelled), RefusalClass::Cancelled);
        assert_eq!(classified(FenceRefusal::Device), RefusalClass::Device);
    }

    fn classified(reason: FenceRefusal) -> RefusalClass {
        super::classify_refusal(reason)
    }

    /// Reproduces the throttled-tab failure: a warp fence refuses at its deadline with an empty
    /// ladder, no scene in flight and no surface pending, which is precisely the state whose
    /// `needs_refresh` answer used to be false and left the page dead.
    #[test]
    fn a_throttled_warp_deadline_re_arms_the_loop_and_a_later_warp_presents() {
        let policy = FramePolicy::SingleFrameOnDemand;
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        frame_loop.accept_request(21, true);
        run_ladder_to_idle(&mut frame_loop, &mut presenter, &mut clock, policy);
        assert_eq!(
            presenter.submissions,
            [
                RefinementLevel::Preview,
                RefinementLevel::Interactive,
                RefinementLevel::Final
            ]
        );
        let refused_warp = *presenter
            .warp_submissions
            .last()
            .expect("the ladder submits a warp on every turn");

        // The tab goes to the background, the fence is polled once a second, and the wall runs out.
        presenter.fire_warp_refusal(FenceRefusal::Deadline, 61, 60_000.0);
        clock.advance(60_000.0);
        let refusal_turn = drive_turn(&mut frame_loop, &mut presenter, clock, policy, false);
        assert!(refusal_turn.refused);
        assert!(!refusal_turn.presented);
        assert_eq!(frame_loop.transient_refusals(), 1);
        assert_eq!(
            frame_loop.last_transient().map(ToString::to_string),
            Some("warp fence exceeded its 60000 ms deadline".to_string())
        );
        assert!(frame_loop.stopped().is_none());
        assert!(
            frame_loop.needs_refresh(false, false, false, false),
            "with the ladder empty and the refused warp retired, only the re-arm keeps the page alive"
        );

        // Input resumes: the very next turn asks for the surface image the refusal cost.
        clock.advance(16.0);
        let retry_warp = drive_turn(&mut frame_loop, &mut presenter, clock, policy, true)
            .warp_id
            .expect("a re-armed run requests the next surface image");
        assert_ne!(retry_warp, refused_warp);

        presenter.fire_warp_completed();
        clock.advance(16.0);
        let recovery = drive_turn(&mut frame_loop, &mut presenter, clock, policy, true);
        assert!(recovery.presented);
        assert_eq!(presenter.presented_warps.last(), Some(&retry_warp));
        assert!(
            !frame_loop.needs_refresh(false, false, false, false),
            "one survived refusal must not turn an on-demand page into a permanent spin"
        );
    }

    #[test]
    fn a_warp_poll_limit_refusal_is_survived_the_same_way() {
        let policy = FramePolicy::SingleFrameOnDemand;
        let mut frame_loop = FrameLoop::default();
        let mut presenter = FakePresenter::default();
        let mut clock = FakeClock::default();
        frame_loop.accept_request(31, true);
        run_ladder_to_idle(&mut frame_loop, &mut presenter, &mut clock, policy);

        presenter.fire_warp_refusal(FenceRefusal::PollLimit, 4_096, 812.5);
        clock.advance(812.5);
        let refusal_turn = drive_turn(&mut frame_loop, &mut presenter, clock, policy, false);
        assert!(refusal_turn.refused);
        assert_eq!(frame_loop.transient_refusals(), 1);
        assert_eq!(
            frame_loop.last_transient().map(ToString::to_string),
            Some("warp fence exhausted its 4096 completion polls".to_string())
        );
        assert!(frame_loop.stopped().is_none());
        assert!(frame_loop.needs_refresh(false, false, false, false));

        clock.advance(16.0);
        let retry_warp = drive_turn(&mut frame_loop, &mut presenter, clock, policy, true)
            .warp_id
            .expect("a re-armed run requests the next surface image");
        presenter.fire_warp_completed();
        clock.advance(16.0);
        assert!(drive_turn(&mut frame_loop, &mut presenter, clock, policy, true).presented);
        assert_eq!(presenter.presented_warps.last(), Some(&retry_warp));
    }

    #[test]
    fn a_scene_deadline_refusal_retires_the_scene_and_keeps_the_level_due() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.accept_request(41, true);
        frame_loop.submitted(9, RefinementLevel::Preview);
        assert_eq!(frame_loop.due(), None);

        let outcome = frame_loop.refused(
            SubmissionKind::Scene,
            FenceRefusal::Deadline,
            9,
            61,
            30_000.0,
        );
        assert_eq!(outcome.class, RefusalClass::Transient);
        assert!(outcome.retired_scene);
        assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
        assert_eq!(frame_loop.transient_refusals(), 1);
        assert!(frame_loop.stopped().is_none());
        assert!(frame_loop.needs_refresh(false, false, false, false));
    }

    #[test]
    fn a_device_fence_refusal_stops_the_loop_with_its_typed_status() {
        let mut frame_loop = FrameLoop::default();
        frame_loop.accept_request(51, true);
        let outcome = frame_loop.refused(SubmissionKind::Warp, FenceRefusal::Device, 3, 12, 41.5);
        assert_eq!(outcome.class, RefusalClass::Device);
        assert_eq!(
            frame_loop.transient_refusals(),
            0,
            "device loss is never counted as something the page survived"
        );
        assert_eq!(
            frame_loop.stopped().map(ToString::to_string),
            Some(
                "fence mapping failed during warp fence: four-byte fence callback failed"
                    .to_string()
            )
        );
        assert!(
            !frame_loop.needs_refresh(true, true, true, true),
            "a stopped loop schedules nothing, whatever else is outstanding"
        );

        frame_loop.refused(
            SubmissionKind::Warp,
            FenceRefusal::Deadline,
            4,
            61,
            60_000.0,
        );
        assert_eq!(
            frame_loop.stopped().map(ToString::to_string),
            Some(
                "fence mapping failed during warp fence: four-byte fence callback failed"
                    .to_string()
            ),
            "a later refusal never overwrites the cause the page is reporting"
        );
    }
}
