//! Cross-slice progressive frame scheduling and browser GPU integration.

use ember_julibrot_kernels::RefinementLevel;
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_kernels::RefinementPlan;
#[cfg(any(target_arch = "wasm32", test))]
use ember_julibrot_present::{FenceRefusal, SubmissionKind};

#[cfg(any(target_arch = "wasm32", test))]
use crate::AppError;

const LEVELS: [RefinementLevel; 3] = [
    RefinementLevel::Preview,
    RefinementLevel::Interactive,
    RefinementLevel::Final,
];

#[cfg(any(target_arch = "wasm32", test))]
const fn arrival_is_current(
    cancelled: bool,
    response_generation: u32,
    endpoint_generation: u32,
    navigation_pending_depth: u32,
) -> bool {
    !cancelled && response_generation == endpoint_generation && navigation_pending_depth == 0
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

/// Latest-wins Preview, Interactive, Final scheduler with one scene in flight.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RefinementSchedule {
    generation: u32,
    next: Option<usize>,
    in_flight: Option<SceneTicket>,
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
struct FrameLoop {
    schedule: RefinementSchedule,
    requested_run: bool,
    completed_run: bool,
    transient_refusals: u32,
    last_transient: Option<AppError>,
    stopped: Option<AppError>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl FrameLoop {
    fn refresh<P: PresenterPoll>(presenter: &mut P, now_ms: f64) -> Vec<P::Event> {
        presenter.poll_once(now_ms)
    }

    const fn accept_request(&mut self, generation: u32, scene_ready: bool) {
        self.requested_run = true;
        self.completed_run = false;
        if scene_ready && !self.schedule.pending() {
            self.schedule.restart(generation);
        }
    }

    const fn restart(&mut self, generation: u32) {
        self.schedule.restart(generation);
        if self.requested_run {
            self.completed_run = false;
        }
    }

    fn due(&self) -> Option<RefinementLevel> {
        self.schedule.due()
    }

    const fn submitted(&mut self, id: u64, level: RefinementLevel) {
        self.schedule.submitted(id, level);
    }

    fn completed(&mut self, id: u64, generation: u32, level: RefinementLevel) -> bool {
        let completed = self.schedule.completed(id, generation, level);
        if completed && self.requested_run && !self.schedule.pending() {
            self.completed_run = true;
        }
        completed
    }

    fn retired(&mut self, id: u64) -> bool {
        self.schedule.retired(id)
    }

    const fn generation(&self) -> u32 {
        self.schedule.generation()
    }

    const fn refinement_pending(&self) -> bool {
        self.schedule.pending()
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
        self.last_transient = Some(error);
        self.requested_run = true;
        self.completed_run = !self.schedule.pending();
    }

    /// Latches the first terminal refusal; a later one never overwrites the cause.
    fn stop(&mut self, error: AppError) {
        if self.stopped.is_none() {
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
        self.next = Some(0);
    }

    /// Returns the exact next level only when present has no scene target occupied.
    #[must_use]
    pub fn due(&self) -> Option<RefinementLevel> {
        self.in_flight
            .is_none()
            .then(|| self.next.map(|index| LEVELS[index]))
            .flatten()
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
        let Some(index) = self.next else {
            return false;
        };
        if LEVELS[index] != level {
            return false;
        }
        self.next = (index + 1 < LEVELS.len()).then_some(index + 1);
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
        DispatchFacts, EscapeGrid, GridExtent, JulibrotKernels, KernelMode, OUTPUT_PAGE_SIDE,
        ReferenceOrbitInput, RefinementPlan,
    };
    use ember_julibrot_math::{
        BigCentre, EscapeParams, Plane, precision_for, reference_shift_px, scale_split,
        shallow_pixel_scale, split_centre,
    };
    use ember_julibrot_present::{
        FrameState, HotSlot, PresentConfig, PresentEvent, PresentHot, PresentMain, Presenter,
        SubmissionKind, hot_stride,
    };
    use ember_julibrot_worker::{
        EncodedCentre, OrbitDisposition, OrbitHandle, OrbitRegistry, OrbitRequest, OwnerEndpoint,
        ProducerEndpoint, RegistryError, SubmitOutcome, WorkerChannel, WorkerConfig, WorkerFacts,
        WorkerMode,
    };
    use ember_lab_heap::{DataSpan, GpuKernelExecutor, GpuKernelExecutorConfig};

    use super::{FrameLoop, RefusalClass};
    use crate::{
        AppError, BrowserRuntime, FramePolicy, FramePolicyTracker, RefreshOutcome, RefreshStatus,
        RunRequests, ViewerController,
    };

    const HEAP_SIDE: u16 = 512;
    const HEAP_LAYERS: u16 = 16;
    const DESCRIPTOR_CAPACITY: u32 = 64;
    const SPAN_CAPACITY: u32 = 16;
    const HANDLE_CAPACITY: u32 = 128;
    const DIRECTORY_BYTES: u32 = SPAN_CAPACITY * 16 + HANDLE_CAPACITY * 4;
    const MAX_HEADER_PAGES: u32 = 64;
    const MAX_HEADER_SETS: u32 = 6;
    const KERNEL_UNIFORM_BYTES: u32 = 96;

    #[derive(Debug)]
    struct RegisteredOrbit {
        span: DataSpan,
        length: u32,
        precision_bits: u32,
    }

    #[derive(Debug)]
    struct SubmittedReference {
        generation: u32,
        centre: BigCentre,
        zoom_log2: f64,
        plane: Plane,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct SceneSelection {
        generation: u32,
        requested_iter_cap: u32,
        palette_id: u32,
        plane_origin_f64: [f64; 4],
        view: ember_julibrot_math::ViewMode,
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
        plane_angles: [f64; 2],
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
        accepted_reference_zoom_log2: Option<f64>,
        submitted_references: Vec<SubmittedReference>,
        plan: RefinementPlan,
        grid: EscapeGrid,
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
            let plan = JulibrotKernels::plan(&executor, extent, params).map_err(kernel_error)?;
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
            presenter.set_main(PresentMain {
                epoch: 0,
                state: main,
                grid: grid.clone(),
            });
            let (owner_endpoint, producer_endpoint) = WorkerChannel::new(
                WorkerConfig {
                    max_iter: requested.iteration_cap,
                },
                WorkerMode::WebWorker,
            )
            .map_err(worker_error)?;
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
                accepted_reference: None,
                accepted_reference_zoom_log2: None,
                submitted_references: Vec::with_capacity(2),
                plan,
                grid,
                main,
                scene_selection: None,
                loop_state: FrameLoop::default(),
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
            let presented = observed.presented;
            if requests.frame {
                self.loop_state
                    .accept_request(self.main.generation_applied, self.current_orbit.is_some());
                requests.frame = false;
            }
            if let Some(error) = self.owner_endpoint.take_error() {
                self.abandon_submitted_references(viewer);
                return Err(worker_error(error));
            }

            self.prepare_due_level();
            let extent = self.prepared_extent();
            let hot = viewer.drain_hot(extent)?;
            self.owner_epoch = hot.state.epoch;
            self.main = hot.state.main;
            self.observe_scene_selection(viewer);
            self.install_main(viewer);
            let slot = HotSlot::for_refresh(self.refresh_id, self.hot_stride, hot.state.epoch)
                .map_err(|error| AppError::Present(error.to_string()))?;
            self.presenter.write_hot(
                slot,
                PresentHot {
                    epoch: hot.state.epoch,
                    state: hot.state.hot,
                    plane: hot.plane,
                },
            );

            let main_arrived = self.service_arrivals(viewer, now_ms)?;
            self.submit_pending_reference(viewer, hot.plane)?;
            let scene_id = if main_arrived {
                None
            } else {
                self.submit_due_scene(viewer, hot.plane, slot, hot.state.epoch, now_ms)?
            };

            let mut warp_id = None;
            let warp_requested = self.loop_state.warp_requested(self.frame_policy.policy());
            if warp_requested && !runtime.has_pending_surface() {
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
                        if let Err(error) = runtime.retain_for_warp(
                            receipt.warp_id,
                            self.loop_state.generation(),
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
                        if self.loop_state.completed(
                            frame.scene_id,
                            frame.pose.orbit_generation,
                            frame.level,
                        ) {
                            self.prepared_level = None;
                        }
                    }
                    PresentEvent::SceneDropped { scene_id, .. } => {
                        if self.loop_state.retired(scene_id) {
                            self.prepared_level = None;
                        }
                    }
                    PresentEvent::WarpCompleted { measurement } => {
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
                    } => {
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

        fn prepare_due_level(&mut self) {
            let Some(level) = self.loop_state.due() else {
                return;
            };
            if self.prepared_level == Some(level) {
                return;
            }
            self.prepared_level = Some(level);
        }

        fn install_main(&mut self, viewer: &ViewerController) {
            let mut grid = self.grid.clone();
            if let Some(level) = self.prepared_level {
                let spec = self.plan.level(level);
                grid.width = spec.extent.width;
                grid.height = spec.extent.height;
                grid.level = level;
            }
            self.main.delivered_iter_cap = super::published_iteration_cap(&self.plan);
            self.presenter.set_main(PresentMain {
                epoch: self.owner_epoch,
                state: self.main,
                grid,
            });
        }

        fn observe_scene_selection(&mut self, viewer: &ViewerController) {
            let selection = SceneSelection {
                generation: self.main.generation_applied,
                requested_iter_cap: self.main.requested_iter_cap,
                palette_id: self.main.palette_id,
                plane_origin_f64: self.main.plane_origin_f64,
            };
            if self.current_orbit.is_some()
                && self.submitted_references.is_empty()
                && viewer.owner().navigation_pending_depth() == 0
                && self
                    .scene_selection
                    .is_some_and(|previous| previous != selection)
            {
                self.loop_state.restart(self.main.generation_applied);
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
                zoom_log2: requested.zoom_log2,
                plane_angles: [
                    requested.plane_angles.theta_1,
                    requested.plane_angles.theta_2,
                ],
            }
        }

        /// Reports whether the image on the canvas belongs to an older requested view.
        ///
        /// Nothing has been presented before the first warp completes, so an unstarted page reads
        /// stale, which is the honest answer: it is showing no view at all.
        #[must_use]
        pub fn presented_view_is_stale(&self, viewer: &ViewerController) -> bool {
            self.presented_view != Some(self.view_stamp(viewer))
        }

        fn prepared_extent(&self) -> [u32; 2] {
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
            let Some(submitted) = submitted.filter(|_| {
                super::arrival_is_current(
                    response.cancelled(),
                    response.generation(),
                    self.owner_endpoint.latest_generation(),
                    viewer.owner().navigation_pending_depth(),
                )
            }) else {
                return Ok((OrbitDisposition::Stale, false));
            };
            let bytes = response
                .records
                .transfer_record_bytes()
                .map_err(worker_error)?
                .to_vec();
            let span = self
                .executor
                .allocate_span(response.length(), OUTPUT_PAGE_SIDE)
                .map_err(heap_error)?;
            if let Err(error) = self.executor.write_span(&span, &bytes) {
                let _freed = self.executor.free_span(span);
                return Err(heap_error(error));
            }
            let registered = RegisteredOrbit {
                span: span.clone(),
                length: response.length(),
                precision_bits: response.precision_bits(),
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
                    reference_shift_px(
                        old,
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
            self.rebuild_grid_if_needed(viewer.requested().iteration_cap)?;
            self.loop_state.restart(response.generation());
            self.prepared_level = None;
            self.install_main(viewer);
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
        ) -> Result<(), AppError> {
            let Some(submission) = viewer.take_reference_submission() else {
                return Ok(());
            };
            let navigation = submission.navigation;
            let requested = viewer.requested();
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
                submission.reason,
            )
            .map_err(worker_error)?;
            if self.owner_endpoint.submit(request) == SubmitOutcome::GenerationExhausted {
                let _finished = viewer.finish_reference_submission(navigation.generation);
                return Err(AppError::GenerationExhausted);
            }
            self.submitted_references.push(SubmittedReference {
                generation: navigation.generation,
                centre: navigation.centre,
                zoom_log2: navigation.zoom_log2,
                plane,
            });
            Ok(())
        }

        fn rebuild_grid_if_needed(&mut self, requested_max_iter: u32) -> Result<(), AppError> {
            let requested_extent = self.plan.requested_extent;
            let next = JulibrotKernels::plan(
                &self.executor,
                requested_extent,
                EscapeParams::new(requested_max_iter),
            )
            .map_err(kernel_error)?;
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

        fn submit_due_scene(
            &mut self,
            viewer: &ViewerController,
            plane: Plane,
            slot: HotSlot,
            owner_epoch: u64,
            now_ms: f64,
        ) -> Result<Option<u64>, AppError> {
            if !self.submitted_references.is_empty()
                || viewer.owner().navigation_pending_depth() != 0
            {
                return Ok(None);
            }
            let Some(level) = self.loop_state.due() else {
                return Ok(None);
            };
            if self.prepared_level != Some(level) {
                return Ok(None);
            }
            let params = EscapeParams::new(viewer.requested().iteration_cap);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Julibrot kernels SCRATCH and DATA copy"),
                });
            let mode = KernelMode::for_zoom(viewer.requested().zoom_log2);
            let facts = match mode {
                KernelMode::Shallow => {
                    let centre = self
                        .accepted_reference
                        .as_ref()
                        .ok_or_else(|| AppError::Kernel("missing shallow centre".to_string()))?;
                    let split = split_centre(centre).map_err(math_error)?;
                    let scale = shallow_pixel_scale(
                        viewer.requested().zoom_log2,
                        self.plan.requested_extent.width,
                    )
                    .map_err(math_error)?;
                    self.kernels
                        .encode_shallow(
                            &self.executor,
                            &mut encoder,
                            &mut self.grid,
                            owner_epoch,
                            level,
                            &plane,
                            &split,
                            scale,
                            params,
                        )
                        .map_err(kernel_error)?
                }
                KernelMode::Perturbation => {
                    let handle = self
                        .current_orbit
                        .ok_or_else(|| AppError::Kernel("missing reference orbit".to_string()))?;
                    let orbit = self.orbits.get(handle).map_err(registry_error)?;
                    let scale = scale_split(
                        viewer.requested().zoom_log2,
                        self.plan.requested_extent.width,
                    )
                    .map_err(math_error)?;
                    self.kernels
                        .encode_perturbation(
                            &self.executor,
                            &mut encoder,
                            &mut self.grid,
                            owner_epoch,
                            level,
                            &plane,
                            scale,
                            params,
                            ReferenceOrbitInput {
                                span: &orbit.span,
                                generation: handle.generation,
                                length: orbit.length,
                                precision_bits: orbit.precision_bits,
                            },
                        )
                        .map_err(kernel_error)?
                }
            };
            self.queue.submit([encoder.finish()]);
            self.main.delivered_iter_cap = super::published_iteration_cap(&self.plan);
            self.presenter.set_main(PresentMain {
                epoch: owner_epoch,
                state: self.main,
                grid: self.grid.clone(),
            });
            match self.presenter.submit_scene(slot, now_ms) {
                Ok(scene_id) => {
                    self.last_dispatch = Some(facts);
                    self.loop_state.submitted(scene_id, level);
                    Ok(Some(scene_id))
                }
                Err(ember_julibrot_present::PresentError::SceneBusy { .. }) => Ok(None),
                Err(error) => Err(present_error(error)),
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
            self.loop_state.needs_refresh(
                self.presenter.facts().in_flight_scene_id.is_some(),
                runtime.has_pending_surface(),
                self.owner_endpoint.pending_request_depth() != 0
                    || !self.submitted_references.is_empty(),
                self.presented_view_is_stale(viewer),
            )
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
mod tests {
    use ember_julibrot_kernels::{GridExtent, plan_refinement};
    use ember_julibrot_math::EscapeParams;

    use super::{
        FenceRefusal, FrameLoop, LEVELS, PresenterPoll, RefinementLevel, RefinementSchedule,
        RefusalClass, SubmissionKind, arrival_is_current, fence_error, published_iteration_cap,
    };
    use crate::FramePolicy;

    /// Poll budget and wall the version-one present configuration refuses at.
    const SCENE_POLLS: u32 = 4_096;
    const SCENE_DEADLINE_MS: f64 = 30_000.0;

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
        submissions: Vec<RefinementLevel>,
        warp_submissions: Vec<u64>,
        presented_warps: Vec<u64>,
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

        fn submit_warp(&mut self) -> u64 {
            self.next_id += 1;
            self.pending_warp = Some(self.next_id);
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
                    self.pending = None;
                    events.push(event);
                }
            }
            if self.pending_warp.is_some() {
                self.warp_fence_observations += 1;
                if let Some(event) = self.warp_callback.take() {
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
