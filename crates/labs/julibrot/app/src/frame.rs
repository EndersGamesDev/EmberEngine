//! Cross-slice progressive frame scheduling and browser GPU integration.

use ember_julibrot_kernels::RefinementLevel;

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

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FrameLoop {
    schedule: RefinementSchedule,
    requested_run: bool,
    completed_run: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
impl FrameLoop {
    fn refresh<P: PresenterPoll>(&mut self, presenter: &mut P, now_ms: f64) -> Vec<P::Event> {
        presenter.poll_once(now_ms)
    }

    fn accept_request(&mut self, generation: u32, scene_ready: bool) {
        self.requested_run = true;
        self.completed_run = false;
        if scene_ready && !self.schedule.pending() {
            self.schedule.restart(generation);
        }
    }

    fn restart(&mut self, generation: u32) {
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

    const fn needs_refresh(
        &self,
        scene_in_flight: bool,
        warp_in_flight: bool,
        auxiliary_pending: bool,
    ) -> bool {
        scene_in_flight
            || warp_in_flight
            || auxiliary_pending
            || self.requested_run
            || self.schedule.pending()
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

    use super::FrameLoop;
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
        pending_warp_zoom: Option<(u64, f64)>,
        last_presented_zoom_log2: Option<f64>,
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
            main.delivered_iter_cap = plan.level(grid.level).iteration_cap;
            presenter.set_main(PresentMain {
                epoch: 0,
                state: main,
                grid: grid.clone(),
                view: requested.view,
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
                pending_warp_zoom: None,
                last_presented_zoom_log2: None,
            };
            Ok(frame_loop)
        }

        /// Executes one bounded refresh and immediate pre-yield completion observation.
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
            let events = self.loop_state.refresh(&mut self.presenter, now_ms);
            let presented = self.handle_events(runtime, events)?;
            if requests.frame {
                self.loop_state
                    .accept_request(self.main.generation_applied, self.current_orbit.is_some());
                requests.frame = false;
            }
            if let Some(error) = self.owner_endpoint.take_error() {
                return Err(worker_error(error));
            }

            self.prepare_due_level();
            let extent = self.prepared_extent();
            let hot = viewer.drain_hot(extent, now_ms / 1_000.0)?;
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
                    view_time_seconds: now_ms / 1_000.0,
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
                        self.pending_warp_zoom =
                            Some((receipt.warp_id, viewer.requested().zoom_log2));
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
        ) -> Result<bool, AppError> {
            let mut presented = false;
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
                            presented = true;
                            if let Some((warp_id, zoom_log2)) = self.pending_warp_zoom.take()
                                && warp_id == measurement.id
                            {
                                self.last_presented_zoom_log2 = Some(zoom_log2);
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
                        match kind {
                            SubmissionKind::Scene => {
                                if self.loop_state.retired(id) {
                                    self.prepared_level = None;
                                }
                            }
                            SubmissionKind::Warp => {
                                let _dropped = runtime.refuse_warp(id);
                                if self
                                    .pending_warp_zoom
                                    .is_some_and(|pending| pending.0 == id)
                                {
                                    self.pending_warp_zoom = None;
                                }
                            }
                        }
                        if refusal.is_none() {
                            refusal = Some(fence_error(kind, reason, polls, wall_ms));
                        }
                    }
                }
            }
            refusal.map_or(Ok(presented), Err)
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
                self.main.delivered_iter_cap = spec.iteration_cap;
            }
            self.presenter.set_main(PresentMain {
                epoch: self.owner_epoch,
                state: self.main,
                grid,
                view: viewer.requested().view,
            });
        }

        fn observe_scene_selection(&mut self, viewer: &ViewerController) {
            let selection = SceneSelection {
                generation: self.main.generation_applied,
                requested_iter_cap: self.main.requested_iter_cap,
                palette_id: self.main.palette_id,
                plane_origin_f64: self.main.plane_origin_f64,
                view: viewer.requested().view,
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
            self.main.delivered_iter_cap = self.plan.level(level).iteration_cap;
            self.presenter.set_main(PresentMain {
                epoch: owner_epoch,
                state: self.main,
                grid: self.grid.clone(),
                view: viewer.requested().view,
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
        pub fn pending(&self, runtime: &BrowserRuntime) -> bool {
            self.loop_state.needs_refresh(
                self.presenter.facts().in_flight_scene_id.is_some(),
                runtime.has_pending_surface(),
                self.owner_endpoint.pending_request_depth() != 0
                    || !self.submitted_references.is_empty(),
            )
        }

        /// Reports whether a progressive level is due or has a scene fence pending.
        #[must_use]
        pub const fn refinement_pending(&self) -> bool {
            self.loop_state.refinement_pending()
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
        pub const fn last_presented_zoom_log2(&self) -> Option<f64> {
            self.last_presented_zoom_log2
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

    fn fence_error(
        kind: SubmissionKind,
        reason: ember_julibrot_present::FenceRefusal,
        polls: u32,
        wall_ms: f64,
    ) -> AppError {
        use ember_julibrot_present::FenceRefusal;

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
}

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserFrameLoop;

#[cfg(test)]
mod tests {
    use super::{
        FrameLoop, PresenterPoll, RefinementLevel, RefinementSchedule, arrival_is_current,
    };
    use crate::FramePolicy;

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
    }

    #[derive(Debug, Default)]
    struct FakePresenter {
        next_id: u64,
        pending: Option<PendingFakeScene>,
        callback: Option<FakeEvent>,
        fence_observations: u32,
        submissions: Vec<RefinementLevel>,
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

        fn fire_completed_callback(&mut self) {
            self.callback = self.pending.map(FakeEvent::Completed);
        }

        fn fire_deadline(&mut self) {
            self.callback = self.pending.map(|scene| FakeEvent::Deadline(scene.id));
        }
    }

    impl PresenterPoll for FakePresenter {
        type Event = FakeEvent;

        fn poll_once(&mut self, _now_ms: f64) -> Vec<Self::Event> {
            if self.pending.is_none() {
                return Vec::new();
            }
            self.fence_observations += 1;
            let Some(event) = self.callback.take() else {
                return Vec::new();
            };
            self.pending = None;
            vec![event]
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

    fn drive_refresh(
        frame_loop: &mut FrameLoop,
        presenter: &mut FakePresenter,
        clock: FakeClock,
    ) -> Option<u64> {
        let mut refused = false;
        for event in frame_loop.refresh(presenter, clock.now_ms) {
            match event {
                FakeEvent::Completed(scene) => {
                    frame_loop.completed(scene.id, scene.generation, scene.level);
                }
                FakeEvent::Deadline(id) => {
                    frame_loop.retired(id);
                    refused = true;
                }
            }
        }
        if refused {
            return None;
        }
        let level = frame_loop.due()?;
        let id = presenter.submit(frame_loop.generation(), level);
        frame_loop.submitted(id, level);
        Some(id)
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
        assert!(frame_loop.needs_refresh(true, false, false));
        assert!(frame_loop.needs_refresh(false, true, false));
        assert!(!frame_loop.needs_refresh(false, false, false));
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
        assert!(!frame_loop.needs_refresh(false, false, false));
        assert!(!frame_loop.warp_requested(FramePolicy::SingleFrameOnDemand));

        frame_loop.accept_request(13, true);
        assert!(frame_loop.needs_refresh(false, false, false));
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
}
