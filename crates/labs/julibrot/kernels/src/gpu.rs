use std::cell::RefCell;

use ember_julibrot_math::{CentreSplit, EscapeParams, Plane, ScaleSplit};
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
}

#[derive(Clone, Debug)]
struct AcceptedReference {
    span: DataSpan,
    generation: u32,
    length: u32,
    precision_bits: u32,
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
        level: RefinementLevel,
        plane: &Plane,
        centre: &CentreSplit,
        pixel_scale: f32,
        params: EscapeParams,
    ) -> Result<DispatchFacts, KernelError> {
        let allocation = self.allocation(grid)?;
        ensure_requested_params(&allocation.plan, params)?;
        let selected = allocation.plan.level(level);
        let uniform = ShallowUniform::pack(
            *plane,
            *centre,
            pixel_scale,
            selected.extent,
            delivered_params(params, selected.iteration_cap),
            level,
        )?;
        let dispatch = executor
            .plan_prefix_dispatch(
                &self.shallow,
                &[],
                &[&grid.span],
                pixel_count(selected.extent)?,
                uniform.bytes(),
            )
            .map_err(|_| KernelError::Dispatch)?;
        let facts = checked_facts(
            executor,
            allocation,
            &dispatch,
            level,
            KernelMode::Shallow,
            owner_epoch,
            None,
        )?;
        encode_pages(
            executor,
            encoder,
            &self.shallow,
            &dispatch,
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
        level: RefinementLevel,
        plane: &Plane,
        scale: ScaleSplit,
        params: EscapeParams,
        reference: ReferenceOrbitInput<'_>,
    ) -> Result<DispatchFacts, KernelError> {
        let allocation = self.allocation(grid)?;
        ensure_requested_params(&allocation.plan, params)?;
        self.validate_reference(reference, allocation.plan.requested_max_iter)?;
        let selected = allocation.plan.level(level);
        let used_orbit_length = reference.length.min(selected.iteration_cap);
        let uniform = PerturbUniform::pack(
            *plane,
            scale,
            selected.extent,
            delivered_params(params, selected.iteration_cap),
            used_orbit_length,
            level,
        )?;
        let dispatch = executor
            .plan_prefix_dispatch(
                &self.perturbation,
                &[reference.span],
                &[&grid.span],
                pixel_count(selected.extent)?,
                uniform.bytes(),
            )
            .map_err(|_| KernelError::Dispatch)?;
        let facts = checked_facts(
            executor,
            allocation,
            &dispatch,
            level,
            KernelMode::Perturbation,
            owner_epoch,
            Some((reference.generation, reference.length)),
        )?;
        let resources_changed =
            self.accept_reference(reference, allocation.plan.requested_max_iter)?;
        if resources_changed {
            executor.sync_dispatch_resources(&dispatch);
        }
        encode_pages(
            executor,
            encoder,
            &self.perturbation,
            &dispatch,
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

const fn level_set(level: RefinementLevel) -> u32 {
    match level {
        RefinementLevel::Preview => 0,
        RefinementLevel::Interactive => 1,
        RefinementLevel::Final => 2,
    }
}

fn checked_facts(
    executor: &GpuKernelExecutor,
    allocation: &GridAllocation,
    dispatch: &ExecutorDispatch,
    level: RefinementLevel,
    mode: KernelMode,
    owner_epoch: u64,
    orbit: Option<(u32, u32)>,
) -> Result<DispatchFacts, KernelError> {
    let mut facts = dispatch_facts(
        &allocation.plan,
        level,
        mode,
        owner_epoch,
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
    use ember_lab_heap::SpanArena;

    use super::{AcceptedReference, accept_reference_transition};
    use crate::{KernelError, ReferenceOrbitInput};

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
}
