#[cfg(any(target_arch = "wasm32", test))]
use std::sync::Arc;

#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_kernels::{EscapeGrid, RefinementPlan};
use ember_julibrot_kernels::{RefinementLevel, next_refinement_level};
use ember_julibrot_math::PrecisionMode;
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_math::{PICTURE_FAST_EDIT_BUDGET, PoseMap};
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_present::{FenceRefusal, SubmissionKind};

#[cfg(any(target_arch = "wasm32", test))]
use crate::{AppError, ViewerController};

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
    pub(super) generation: u32,
    pub(super) precision_mode: PrecisionMode,
    pub(super) next: Option<RefinementLevel>,
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
pub(super) trait PresenterPoll {
    type Event;

    fn poll_once(&mut self, now_ms: f64) -> Vec<Self::Event>;
}

/// What one present fence refusal means for the life of the refresh loop.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefusalClass {
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
pub(super) struct RefusalOutcome {
    pub(super) class: RefusalClass,
    pub(super) retired_scene: bool,
}

/// Classifies a present fence refusal into the two lives it can have.
///
/// A deadline or a poll limit says only that the bounded observation window closed before the
/// fence did, which is exactly what a background-throttled tab produces: the wall keeps running
/// while the callback queue does not. Treating that as terminal kills a page whose GPU is
/// healthy. Only a failed fence callback is evidence that the device is gone.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn classify_refusal(reason: FenceRefusal) -> RefusalClass {
    match reason {
        FenceRefusal::PollLimit | FenceRefusal::Deadline => RefusalClass::Transient,
        FenceRefusal::Cancelled => RefusalClass::Cancelled,
        FenceRefusal::Device => RefusalClass::Device,
    }
}

/// Renders one present fence refusal as the typed app error the page displays.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn fence_error(
    kind: SubmissionKind,
    reason: FenceRefusal,
    polls: u32,
    wall_ms: f64,
) -> AppError {
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
pub(super) struct FrameLoop {
    pub(super) schedule: RefinementSchedule,
    pub(super) scene_mode: SceneMode,
    pub(super) scene_update_pending: bool,
    pub(super) draft_skipped_count: u64,
    pub(super) last_draft_skip_reason: Option<&'static str>,
    pub(super) ladder_round: u64,
    pub(super) manual_rendering: bool,
    pub(super) restart_after_scene: Option<u32>,
    pub(super) requested_run: bool,
    pub(super) completed_run: bool,
    pub(super) transient_refusals: u32,
    pub(super) last_transient: Option<AppError>,
    pub(super) last_transient_text: Option<Arc<str>>,
    pub(super) stopped: Option<AppError>,
    pub(super) stopped_text: Option<Arc<str>>,
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn schedule_exposure_fill(
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
pub(super) const fn stamped_extent(plan: &RefinementPlan) -> [u32; 2] {
    [plan.requested_extent.width, plan.requested_extent.height]
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn stamped_screen_map(viewer: &ViewerController, plan: &RefinementPlan) -> PoseMap {
    viewer
        .screen_map(stamped_extent(plan))
        .unwrap_or(PoseMap::EdgeOn)
}

/// Publishes the level a scene submission represents even when no kernel dispatch runs.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn stamp_scene_level(
    grid: &mut EscapeGrid,
    plan: &RefinementPlan,
    level: RefinementLevel,
) {
    let spec = plan.level(level);
    grid.width = spec.extent.width;
    grid.height = spec.extent.height;
    grid.level = level;
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn view_projection_changed(
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
    pub(super) fn refresh<P: PresenterPoll>(presenter: &mut P, now_ms: f64) -> Vec<P::Event> {
        presenter.poll_once(now_ms)
    }

    pub(super) const fn accept_request(&mut self, generation: u32, restart_scene: bool) {
        self.requested_run = true;
        self.completed_run = false;
        if restart_scene {
            self.scene_changed(generation);
        }
        if !self.schedule.pending() {
            self.completed_run = true;
        }
    }

    pub(super) const fn apply_precision_mode(
        &mut self,
        precision_mode: PrecisionMode,
        generation: u32,
    ) {
        self.schedule.generation = generation;
        self.schedule.precision_mode = precision_mode;
        match self.scene_mode {
            SceneMode::Auto => {
                self.ladder_round = self.ladder_round.saturating_add(1);
                self.schedule.next = Some(RefinementLevel::Preview);
                self.schedule.in_flight = None;
                if self.requested_run {
                    self.completed_run = false;
                }
            }
            SceneMode::Manual => self.scene_changed(generation),
        }
    }

    pub(super) const fn restart(&mut self, generation: u32) {
        self.ladder_round = self.ladder_round.saturating_add(1);
        self.schedule.restart(generation);
        self.restart_after_scene = None;
        if self.requested_run {
            self.completed_run = false;
        }
    }

    pub(super) const fn scene_changed(&mut self, generation: u32) {
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

    pub(super) const fn scene_input_ready(&mut self, generation: u32) {
        if matches!(self.scene_mode, SceneMode::Auto) || self.manual_rendering {
            self.restart(generation);
        }
    }

    /// Accepts new scene input that changed no pixel's meaning and resumes at one chosen level.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) const fn scene_input_resumed(&mut self, generation: u32, level: RefinementLevel) {
        if matches!(self.scene_mode, SceneMode::Auto) || self.manual_rendering {
            self.ladder_round = self.ladder_round.saturating_add(1);
            self.schedule.resume_at(generation, level);
            self.restart_after_scene = None;
            if self.requested_run {
                self.completed_run = false;
            }
        }
    }

    pub(super) const fn scene_selection_changed(&mut self, generation: u32) {
        if matches!(self.scene_mode, SceneMode::Auto) {
            self.restart(generation);
        } else if !self.manual_rendering {
            self.scene_changed(generation);
        }
    }

    pub(super) const fn request_scene_update(&mut self, generation: u32) {
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

    pub(super) fn set_scene_mode(&mut self, mode: SceneMode, generation: u32, has_scene: bool) {
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

    pub(super) const fn exposure_fill(&mut self, generation: u32) -> bool {
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

    pub(super) fn due(&self) -> Option<RefinementLevel> {
        self.schedule.due()
    }

    pub(super) fn skip_drafts_for_accepted_warp(
        &mut self,
        source: Option<(RefinementLevel, bool)>,
    ) -> bool {
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

    pub(super) const fn submitted(&mut self, id: u64, level: RefinementLevel) {
        self.schedule.submitted(id, level);
    }

    pub(super) fn completed(&mut self, id: u64, generation: u32, level: RefinementLevel) -> bool {
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

    pub(super) fn retired(&mut self, id: u64) -> bool {
        let retired = self.schedule.retired(id);
        if retired && let Some(restart_generation) = self.restart_after_scene.take() {
            self.restart(restart_generation);
        }
        if self.requested_run && !self.schedule.pending() {
            self.completed_run = true;
        }
        retired
    }

    pub(super) const fn generation(&self) -> u32 {
        self.schedule.generation()
    }

    pub(super) const fn refinement_pending(&self) -> bool {
        self.schedule.pending()
    }

    pub(super) const fn scene_mode(&self) -> SceneMode {
        self.scene_mode
    }

    pub(super) const fn scene_update_pending(&self) -> bool {
        matches!(self.scene_mode, SceneMode::Manual) && self.scene_update_pending
    }

    pub(super) const fn hold_refused_warp(&self, has_retained_scene: bool) -> bool {
        if matches!(self.scene_mode, SceneMode::Manual) {
            return true;
        }
        has_retained_scene && self.refinement_pending()
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) const fn ladder_round(&self) -> u64 {
        self.ladder_round
    }

    pub(super) const fn draft_skipped_count(&self) -> u64 {
        self.draft_skipped_count
    }

    pub(super) const fn last_draft_skip_reason(&self) -> Option<&'static str> {
        self.last_draft_skip_reason
    }

    pub(super) const fn warp_requested(&self, policy: crate::FramePolicy) -> bool {
        self.requested_run || !matches!(policy, crate::FramePolicy::SingleFrameOnDemand)
    }

    pub(super) const fn warp_submitted(&mut self) {
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
    pub(super) fn refused(
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
    pub(super) fn record_transient(&mut self, error: AppError) {
        self.transient_refusals = self.transient_refusals.saturating_add(1);
        self.last_transient_text = Some(Arc::from(error.to_string()));
        self.last_transient = Some(error);
        self.requested_run = true;
        self.completed_run = !self.schedule.pending();
    }

    /// Latches the first terminal refusal; a later one never overwrites the cause.
    pub(super) fn stop(&mut self, error: AppError) {
        if self.stopped.is_none() {
            self.stopped_text = Some(Arc::from(error.to_string()));
            self.stopped = Some(error);
        }
    }

    pub(super) const fn stopped(&self) -> Option<&AppError> {
        self.stopped.as_ref()
    }

    pub(super) const fn transient_refusals(&self) -> u32 {
        self.transient_refusals
    }

    pub(super) const fn last_transient(&self) -> Option<&AppError> {
        self.last_transient.as_ref()
    }

    pub(super) const fn last_transient_text(&self) -> Option<&Arc<str>> {
        self.last_transient_text.as_ref()
    }

    pub(super) const fn stopped_text(&self) -> Option<&Arc<str>> {
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
    pub(super) const fn needs_refresh(
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
pub(super) fn apply_precision_mode(
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

    /// Resumes ordered refinement at one chosen level for a selection whose picture did not change.
    ///
    /// A reference exchange replaces the orbit the same view is expanded around, so the levels
    /// below the one already delivered would only repaint the same meaning at lower resolution.
    pub const fn resume_at(&mut self, generation: u32, level: RefinementLevel) {
        self.generation = generation;
        self.next = Some(level);
    }

    /// Stops future levels without forgetting a scene whose fence must still be observed.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) const fn pause(&mut self) {
        self.next = None;
    }

    /// Reports whether a submitted scene still owns the present target.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) const fn scene_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Reports whether the named scene is the one whose fence remains outstanding.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn matches_scene(&self, id: u64) -> bool {
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
