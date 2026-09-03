use std::cell::RefCell;

use ember_julibrot_math::{
    CentreSplit, EscapeParams, Homography, Plane, PrecisionMode, ScaleSplit,
};
use ember_lab_heap::{
    DataSpan, DispatchSelector, ExecutorDispatch, GpuKernel, GpuKernelExecutor, HeaderSetHandle,
    RegisteredKernel, SpanPlan,
};

use crate::{
    DispatchFacts, EscapeGrid, GridExtent, KernelError, KernelMode, PerturbUniform,
    ReferenceOrbitInput, RefinementLevel, RefinementPlan, ShallowUniform, dispatch_facts,
    perturbation_kernel, refinement::validate_plan, shallow_kernel,
};

const LEVEL_COUNT: u32 = 3;

#[derive(Clone, Debug)]
struct GridAllocation {
    span: DataSpan,
    headers: HeaderSetHandle,
    plan: RefinementPlan,
    span_plan: SpanPlan,
    shallow_dispatches: [ExecutorDispatch; LEVEL_COUNT as usize],
    reference_dispatches: RefCell<Vec<ReferenceDispatches>>,
}

#[derive(Clone, Debug)]
struct ReferenceDispatches {
    resource_words: [u32; 4],
    levels: [ExecutorDispatch; LEVEL_COUNT as usize],
}

#[derive(Clone, Debug)]
struct AcceptedReference {
    span: DataSpan,
    generation: u32,
    length: u32,
    precision_bits: u32,
    precision_mode: &'static str,
}

/// The two registered Julibrot pipelines and their private grid-lifetime records.
pub struct JulibrotKernels {
    shallow: GpuKernel,
    perturbation: GpuKernel,
    grids: Vec<GridAllocation>,
    latest_reference: RefCell<Option<AcceptedReference>>,
}

impl JulibrotKernels {
    /// Registers the shallow and scaled-perturbation dialect-v2 pipelines.
    ///
    /// # Errors
    ///
    /// Returns `Register` if either descriptor or immutable pipeline exceeds the executor contract.
    pub fn new(executor: &mut GpuKernelExecutor) -> Result<Self, KernelError> {
        let limits = executor.dialect_limits();
        let shallow = RegisteredKernel::register(&shallow_kernel(), limits)
            .map_err(|_| KernelError::Register)
            .and_then(|kernel| {
                executor
                    .register_kernel(kernel)
                    .map_err(|_| KernelError::Register)
            })?;
        let perturbation = RegisteredKernel::register(&perturbation_kernel(), limits)
            .map_err(|_| KernelError::Register)
            .and_then(|kernel| {
                executor
                    .register_kernel(kernel)
                    .map_err(|_| KernelError::Register)
            })?;
        Ok(Self {
            shallow,
            perturbation,
            grids: Vec::new(),
            latest_reference: RefCell::new(None),
        })
    }

    /// Selects the first power-of-two-degraded plan admitted by exact live heap and header trials.
    ///
    /// # Errors
    ///
    /// Returns a typed input, arithmetic, or live-capacity refusal without mutating the executor.
    pub fn plan(
        executor: &GpuKernelExecutor,
        requested_extent: GridExtent,
        params: EscapeParams,
    ) -> Result<RefinementPlan, KernelError> {
        let capacity = executor.capacity_report();
        let page_records = u32::from(crate::OUTPUT_PAGE_SIDE).pow(2);
        let header_record_capacity = capacity
            .max_header_pages
            .checked_mul(page_records)
            .ok_or(KernelError::ArithmeticOverflow)?;
        let bytes_per_set = u64::from(capacity.header_stride)
            .checked_mul(u64::from(capacity.max_header_pages))
            .ok_or(KernelError::ArithmeticOverflow)?;
        let required_header_bytes = bytes_per_set
            .checked_mul(u64::from(LEVEL_COUNT))
            .ok_or(KernelError::ArithmeticOverflow)?;
        crate::plan_refinement(requested_extent, params, |records| {
            records <= header_record_capacity
                && capacity.free_header_bytes >= required_header_bytes
                && executor.plan_span(records, crate::OUTPUT_PAGE_SIDE).is_ok()
        })
    }

    /// Allocates one Final-capacity span and reserves its three immutable dense-prefix header sets.
    ///
    /// # Errors
    ///
    /// Returns a typed plan, allocation, or header-capacity refusal and rolls back a span when
    /// header reservation fails.
    pub fn allocate_grid(
        &mut self,
        executor: &mut GpuKernelExecutor,
        plan: &RefinementPlan,
    ) -> Result<EscapeGrid, KernelError> {
        validate_plan(plan)?;
        let logical_len = pixel_count(plan.delivered_extent)?;
        let span_plan = executor
            .plan_span(logical_len, crate::OUTPUT_PAGE_SIDE)
            .map_err(|_| KernelError::Heap)?;
        let span = executor
            .allocate_span(logical_len, crate::OUTPUT_PAGE_SIDE)
            .map_err(|_| KernelError::Heap)?;
        let headers = plan
            .levels
            .iter()
            .map(|level| {
                pixel_count(level.extent)
                    .map_err(|_| KernelError::Dispatch)
                    .and_then(|active_len| {
                        executor
                            .prefix_headers(&span, active_len)
                            .map_err(|_| KernelError::Dispatch)
                    })
            })
            .collect::<Result<Vec<_>, _>>();
        let headers = match headers {
            Ok(headers) => headers,
            Err(error) => {
                executor.free_span(span).map_err(|_| KernelError::Heap)?;
                return Err(error);
            }
        };
        let Ok(header_sets) = executor.reserve_header_sets(&headers) else {
            executor.free_span(span).map_err(|_| KernelError::Heap)?;
            return Err(KernelError::Dispatch);
        };
        if header_sets.set_count() != LEVEL_COUNT {
            executor.free_span(span).map_err(|_| KernelError::Heap)?;
            return Err(KernelError::Dispatch);
        }
        let shallow_dispatches = match dispatch_templates(
            executor,
            &self.shallow,
            &[],
            &span,
            plan,
            &[0; core::mem::size_of::<ShallowUniform>()],
        ) {
                Ok(dispatches) => dispatches,
                Err(error) => {
                    executor.free_span(span).map_err(|_| KernelError::Heap)?;
                    return Err(error);
                }
            };
        let first = plan.level(RefinementLevel::Preview);
        let grid = EscapeGrid {
            span: span.clone(),
            width: first.extent.width,
            height: first.extent.height,
            level: first.level,
        };
        self.grids.push(GridAllocation {
            span,
            headers: header_sets,
            plan: *plan,
            span_plan,
            shallow_dispatches,
            reference_dispatches: RefCell::new(Vec::with_capacity(2)),
        });
        Ok(grid)
    }

    /// Encodes one shallow logical level through SCRATCH and exact DATA copies.
    ///
    /// # Errors
    ///
    /// Returns a typed grid, uniform, planning, resident-header, or copy-encoding refusal before
    /// publishing a new active extent on `grid`.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_shallow(
        &self,
        executor: &GpuKernelExecutor,
        encoder: &mut wgpu::CommandEncoder,
        grid: &mut EscapeGrid,
        owner_epoch: u64,
        precision_mode: PrecisionMode,
        level: RefinementLevel,
        plane: &Plane,
        screen_to_plane: &Homography,
        centre: &CentreSplit,
        pixel_scale: f32,
        params: EscapeParams,
    ) -> Result<DispatchFacts, KernelError> {
        let allocation = self.allocation(grid)?;
        ensure_requested_params(&allocation.plan, params)?;
        let selected = allocation.plan.level(level);
        let uniform = ShallowUniform::pack(
            *plane,
            screen_to_plane,
            *centre,
            pixel_scale,
            selected.extent,
            delivered_params(params, selected.iteration_cap),
            level,
        )?;
        let dispatch = &allocation.shallow_dispatches[level_index(level)];
        let facts = checked_facts(
            executor,
            allocation,
            dispatch,
            level,
            KernelMode::Shallow,
            owner_epoch,
            precision_mode,
            None,
        )?;
        encode_pages(
            executor,
            encoder,
            &self.shallow,
            dispatch,
            &allocation.headers,
            level,
            uniform.bytes(),
        )?;
        publish_level(grid, selected.extent, level);
        Ok(facts)
    }

    /// Encodes one scaled-perturbation logical level using one generation-tagged orbit span.
    ///
    /// # Errors
    ///
    /// Returns a typed grid, reference, uniform, planning, resident-header, or copy-encoding refusal
    /// before publishing a new active extent on `grid`.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_perturbation(
        &self,
        executor: &GpuKernelExecutor,
        encoder: &mut wgpu::CommandEncoder,
        grid: &mut EscapeGrid,
        owner_epoch: u64,
        precision_mode: PrecisionMode,
        level: RefinementLevel,
        plane: &Plane,
        screen_to_plane: &Homography,
        scale: ScaleSplit,
        params: EscapeParams,
        reference: ReferenceOrbitInput<'_>,
    ) -> Result<DispatchFacts, KernelError> {
        let allocation = self.allocation(grid)?;
        ensure_requested_params(&allocation.plan, params)?;
        self.validate_reference(reference, allocation.plan.requested_max_iter)?;
        if reference.precision_mode != precision_mode.as_str() {
            return Err(KernelError::ReferencePrecisionMismatch);
        }
        let selected = allocation.plan.level(level);
        let used_orbit_length = reference.length.min(selected.iteration_cap);
        let uniform = PerturbUniform::pack(
            *plane,
            screen_to_plane,
            scale,
            selected.extent,
            delivered_params(params, selected.iteration_cap),
            used_orbit_length,
            level,
        )?;
        let resource_words = [
            reference.span.directory_index,
            reference.span.logical_len,
            0,
            0,
        ];
        if !allocation
            .reference_dispatches
            .borrow()
            .iter()
            .any(|cached| cached.resource_words == resource_words)
        {
            let levels = dispatch_templates(
                executor,
                &self.perturbation,
                &[reference.span],
                &grid.span,
                &allocation.plan,
                &[0; core::mem::size_of::<PerturbUniform>()],
            )?;
            let mut cached = allocation.reference_dispatches.borrow_mut();
            if cached.len() == 2 {
                cached.swap_remove(0);
            }
            cached.push(ReferenceDispatches {
                resource_words,
                levels,
            });
        }
        let cached = allocation.reference_dispatches.borrow();
        let dispatch = &cached
            .iter()
            .find(|cached| cached.resource_words == resource_words)
            .ok_or(KernelError::Dispatch)?
            .levels[level_index(level)];
        let facts = checked_facts(
            executor,
            allocation,
            dispatch,
            level,
            KernelMode::Perturbation,
            owner_epoch,
            precision_mode,
            Some((reference.generation, reference.length)),
        )?;
        let resources_changed =
            self.accept_reference(reference, allocation.plan.requested_max_iter)?;
        if resources_changed {
            executor.sync_dispatch_resources(dispatch);
        }
        encode_pages(
            executor,
            encoder,
            &self.perturbation,
            dispatch,
            &allocation.headers,
            level,
            uniform.bytes(),
        )?;
        publish_level(grid, selected.extent, level);
        Ok(facts)
    }

    /// Frees one kernels-owned grid after presentation has relinquished every clone.
    ///
    /// # Errors
    ///
    /// Returns `Heap` if the grid is foreign, stale, or refused by the executor; private lifetime
    /// state is retained when executor release fails.
    pub fn free_grid(
        &mut self,
        executor: &mut GpuKernelExecutor,
        grid: EscapeGrid,
    ) -> Result<(), KernelError> {
        let position = self
            .grids
            .iter()
            .position(|allocation| allocation.span == grid.span)
            .ok_or(KernelError::Heap)?;
        executor
            .free_span(grid.span)
            .map_err(|_| KernelError::Heap)?;
        self.grids.swap_remove(position);
        Ok(())
    }

    fn allocation(&self, grid: &EscapeGrid) -> Result<&GridAllocation, KernelError> {
        self.grids
            .iter()
            .find(|allocation| allocation.span == grid.span)
            .ok_or(KernelError::Heap)
    }

    fn accept_reference(
        &self,
        reference: ReferenceOrbitInput<'_>,
        requested_max_iter: u32,
    ) -> Result<bool, KernelError> {
        accept_reference_transition(
            &mut self.latest_reference.borrow_mut(),
            reference,
            requested_max_iter,
        )
    }

    fn validate_reference(
        &self,
        reference: ReferenceOrbitInput<'_>,
        requested_max_iter: u32,
    ) -> Result<(), KernelError> {
        validate_reference_transition(
            self.latest_reference.borrow().as_ref(),
            reference,
            requested_max_iter,
        )
        .map(|_| ())
    }
}

fn accept_reference_transition(
    latest: &mut Option<AcceptedReference>,
    reference: ReferenceOrbitInput<'_>,
    requested_max_iter: u32,
) -> Result<bool, KernelError> {
    let resources_changed =
        validate_reference_transition(latest.as_ref(), reference, requested_max_iter)?;
    if latest
        .as_ref()
        .is_none_or(|accepted| reference.generation > accepted.generation)
    {
        *latest = Some(AcceptedReference {
            span: reference.span.clone(),
            generation: reference.generation,
            length: reference.length,
            precision_bits: reference.precision_bits,
            precision_mode: reference.precision_mode,
        });
    }
    Ok(resources_changed)
}

fn validate_reference_transition(
    latest: Option<&AcceptedReference>,
    reference: ReferenceOrbitInput<'_>,
    requested_max_iter: u32,
) -> Result<bool, KernelError> {
    if reference.length == 0
        || reference.length != reference.span.logical_len
        || reference.length > requested_max_iter
    {
        return Err(KernelError::ReferenceLengthMismatch);
    }
    if reference.precision_bits == 0 {
        return Err(KernelError::ReferencePrecisionMismatch);
    }
    if let Some(accepted) = latest {
        if reference.generation < accepted.generation
            || (reference.generation == accepted.generation && reference.span != &accepted.span)
        {
            return Err(KernelError::StaleReference);
        }
        if reference.generation == accepted.generation && reference.length != accepted.length {
            return Err(KernelError::ReferenceLengthMismatch);
        }
        if reference.generation == accepted.generation
            && reference.precision_bits != accepted.precision_bits
        {
            return Err(KernelError::ReferencePrecisionMismatch);
        }
        if reference.generation == accepted.generation
            && reference.precision_mode != accepted.precision_mode
        {
            return Err(KernelError::ReferencePrecisionMismatch);
        }
    }
    Ok(latest.is_none_or(|accepted| reference.span != &accepted.span))
}

fn ensure_requested_params(plan: &RefinementPlan, params: EscapeParams) -> Result<(), KernelError> {
    crate::shallow::validate_params(params)?;
    if params.max_iter != plan.requested_max_iter {
        return Err(KernelError::Dispatch);
    }
    Ok(())
}

const fn delivered_params(params: EscapeParams, max_iter: u32) -> EscapeParams {
    EscapeParams {
        max_iter,
        bailout: params.bailout,
    }
}

fn pixel_count(extent: GridExtent) -> Result<u32, KernelError> {
    crate::shallow::validate_extent(extent)
}

fn dispatch_templates(
    executor: &GpuKernelExecutor,
    kernel: &GpuKernel,
    inputs: &[&DataSpan],
    output: &DataSpan,
    plan: &RefinementPlan,
    uniform: &[u8],
) -> Result<[ExecutorDispatch; LEVEL_COUNT as usize], KernelError> {
    let plan_level = |level| {
        let selected = plan.level(level);
        executor
            .plan_prefix_dispatch(
                kernel,
                inputs,
                &[output],
                pixel_count(selected.extent)?,
                uniform,
            )
            .map_err(|_| KernelError::Dispatch)
    };
    Ok([
        plan_level(RefinementLevel::Preview)?,
        plan_level(RefinementLevel::Interactive)?,
        plan_level(RefinementLevel::Final)?,
    ])
}

const fn level_index(level: RefinementLevel) -> usize {
    match level {
        RefinementLevel::Preview => 0,
        RefinementLevel::Interactive => 1,
        RefinementLevel::Final => 2,
    }
}

const fn level_set(level: RefinementLevel) -> u32 {
    match level {
        RefinementLevel::Preview => 0,
        RefinementLevel::Interactive => 1,
        RefinementLevel::Final => 2,
    }
}

#[allow(clippy::too_many_arguments)]
fn checked_facts(
    executor: &GpuKernelExecutor,
    allocation: &GridAllocation,
    dispatch: &ExecutorDispatch,
    level: RefinementLevel,
    mode: KernelMode,
    owner_epoch: u64,
    precision_mode: PrecisionMode,
    orbit: Option<(u32, u32)>,
) -> Result<DispatchFacts, KernelError> {
    let mut facts = dispatch_facts(
        &allocation.plan,
        level,
        mode,
        owner_epoch,
        precision_mode,
        executor.capacity_report().scratch_bytes,
        orbit,
    )?;
    let page_passes =
        u32::try_from(dispatch.plan().passes.len()).map_err(|_| KernelError::ArithmeticOverflow)?;
    if facts.page_passes != page_passes
        || facts.copy_commands != dispatch.copy_commands()
        || facts.gpu_copy_bytes != dispatch.plan().gpu_copy_bytes
    {
        return Err(KernelError::Dispatch);
    }
    facts.reserved_heap_bytes = allocation.span_plan.reserved_bytes;
    Ok(facts)
}

fn encode_pages(
    executor: &GpuKernelExecutor,
    encoder: &mut wgpu::CommandEncoder,
    kernel: &GpuKernel,
    dispatch: &ExecutorDispatch,
    headers: &HeaderSetHandle,
    level: RefinementLevel,
    uniform: &[u8],
) -> Result<(), KernelError> {
    executor
        .write_kernel_uniform(kernel, uniform)
        .map_err(|_| KernelError::Dispatch)?;
    let page_count =
        u32::try_from(dispatch.plan().passes.len()).map_err(|_| KernelError::ArithmeticOverflow)?;
    for page in 0..page_count {
        executor
            .encode_dispatch_selected(
                encoder,
                kernel,
                dispatch,
                headers,
                DispatchSelector {
                    set: level_set(level),
                    page,
                },
            )
            .map_err(|_| KernelError::Dispatch)?;
    }
    Ok(())
}

const fn publish_level(grid: &mut EscapeGrid, extent: GridExtent, level: RefinementLevel) {
    grid.width = extent.width;
    grid.height = extent.height;
    grid.level = level;
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ember_lab_heap::{
        DataSpan, DialectLimits, DispatchPlan, RegisteredKernel, SpanArena, StaticHeaders,
    };

    use super::{AcceptedReference, accept_reference_transition};
    use crate::{
        EscapeParams, GridExtent, KernelError, ReferenceOrbitInput, RefinementLevel,
        perturbation_kernel, plan_refinement,
    };

    struct NativeDispatchHarness {
        arena: SpanArena,
        kernel: RegisteredKernel,
        reference: DataSpan,
        outputs: [DataSpan; 3],
    }

    impl NativeDispatchHarness {
        fn new() -> Self {
            let mut arena = SpanArena::new(1_024, 1, 64, 4_096, 16).expect("planner arena");
            let reference = arena
                .allocate_span(4_096, crate::OUTPUT_PAGE_SIDE)
                .expect("reference span");
            let plan = plan_refinement(
                GridExtent {
                    width: 960,
                    height: 540,
                },
                EscapeParams::new(4_096),
                |_| true,
            )
            .expect("960 by 540 plan");
            let outputs = plan.levels.map(|level| {
                arena
                    .allocate_span(
                        level.extent.width * level.extent.height,
                        crate::OUTPUT_PAGE_SIDE,
                    )
                    .expect("level output")
            });
            let kernel = RegisteredKernel::register(
                &perturbation_kernel(),
                DialectLimits {
                    descriptor_capacity: 64,
                    span_capacity: 16,
                    handle_capacity: 64,
                },
            )
            .expect("perturbation kernel");
            Self {
                arena,
                kernel,
                reference,
                outputs,
            }
        }

        fn plan(&self, level: usize) -> DispatchPlan {
            let output = &self.outputs[level];
            let headers = StaticHeaders::for_span(output, 256).expect("level headers");
            self.kernel
                .plan_dispatch(
                    &self.arena,
                    &[&self.reference],
                    &[output],
                    &[0; 112],
                    &headers,
                )
                .expect("level dispatch")
        }

        fn templates(&self) -> [DispatchPlan; 3] {
            [self.plan(0), self.plan(1), self.plan(2)]
        }
    }

    #[test]
    fn newer_reference_cancels_every_older_or_conflicting_identity() {
        let mut arena = SpanArena::new(8, 2, 8, 256, 8).expect("fixture arena");
        let older = arena.allocate_span(4, 2).expect("older orbit span");
        let newer = arena.allocate_span(4, 2).expect("newer orbit span");
        let input = |span, generation, precision_bits| ReferenceOrbitInput {
            span,
            generation,
            length: 4,
            precision_bits,
            precision_mode: "PictureFast",
        };
        let mut accepted: Option<AcceptedReference> = None;
        assert!(
            accept_reference_transition(&mut accepted, input(&older, 7, 192), 8)
                .expect("first generation is accepted")
        );
        assert!(
            accept_reference_transition(&mut accepted, input(&newer, 8, 224), 8)
                .expect("newer span replaces it")
        );
        assert!(
            !accept_reference_transition(&mut accepted, input(&newer, 8, 224), 8)
                .expect("same generation and span remain current")
        );
        assert_eq!(
            accept_reference_transition(&mut accepted, input(&older, 7, 192), 8),
            Err(KernelError::StaleReference)
        );
        assert_eq!(
            accept_reference_transition(&mut accepted, input(&older, 8, 224), 8),
            Err(KernelError::StaleReference)
        );
        assert_eq!(
            accept_reference_transition(&mut accepted, input(&newer, 8, 192), 8),
            Err(KernelError::ReferencePrecisionMismatch)
        );
    }

    #[test]
    fn cached_dispatch_templates_pin_every_level_plan() {
        let harness = NativeDispatchHarness::new();
        let templates = harness.templates();
        for (index, level) in [
            RefinementLevel::Preview,
            RefinementLevel::Interactive,
            RefinementLevel::Final,
        ]
        .into_iter()
        .enumerate()
        {
            let fresh = harness.plan(index);
            assert_eq!(templates[index], fresh, "level {level:?}");
            assert_eq!(
                templates[index].resource_words[0],
                [
                    harness.reference.directory_index,
                    harness.reference.logical_len,
                    0,
                    0,
                ]
            );
        }
    }

    #[test]
    #[ignore = "native kernels measurement harness"]
    #[allow(
        clippy::print_stderr,
        reason = "the explicitly selected performance harness reports allocations and wall"
    )]
    fn measures_dispatch_planning_allocations_and_wall_per_level() {
        const ROUNDS: u32 = 10_000;
        let harness = NativeDispatchHarness::new();
        let templates = harness.templates();
        for (index, level) in ["Preview", "Interactive", "Final"].into_iter().enumerate() {
            let before_start = Instant::now();
            for _ in 0..ROUNDS {
                std::hint::black_box(harness.plan(index));
            }
            let before_wall = before_start.elapsed();

            let after_start = Instant::now();
            for _ in 0..ROUNDS {
                std::hint::black_box(&templates[index]);
            }
            let after_wall = after_start.elapsed();

            eprintln!(
                "PF-V3 level={level} rounds={ROUNDS} before_allocations_at_least=5 after_allocations=0 before_us={} after_us={}",
                before_wall.as_micros(),
                after_wall.as_micros()
            );
        }
    }
}
