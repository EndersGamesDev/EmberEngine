use std::cell::RefCell;

use ember_julibrot_math::{
    CentreSplit, EscapeParams, Homography, Plane, PrecisionMode, ScaleSplit,
};
use ember_lab_heap::{
    DataSpan, ExecutorDispatch, GpuKernel, GpuKernelExecutor, HeaderSetHandle, KernelDesc,
    RegisteredKernel, SpanPlan,
};

use crate::{
    DispatchFacts, EscapeGrid, GridExtent, KernelError, KernelMode, PairedOutputAllocation,
    PerturbUniform, ReferenceOrbitInput, RefinementLevel, RefinementPlan, ShallowUniform,
    SourceReconstructionUniform, TileJobError, dispatch_facts, perturbation_kernel,
    refinement::validate_plan, shallow_kernel,
};

const LEVEL_COUNT: u32 = 3;
const RECONSTRUCTION_ACCESSORS: &[&str] = &["escape"];
const PERTURBATION_RECONSTRUCTION_ACCESSORS: &[&str] = &["reference", "escape"];
const RECONSTRUCTION_OUTPUT_FIELDS: &[&str] = &["reconstruction"];
const RGBA32F_COLOR_ATTACHMENT_BYTES: u32 = 16;
#[cfg(test)]
const PAIRED_TRANSACTION_COLOR_ATTACHMENT_BYTES: u32 = 2 * RGBA32F_COLOR_ATTACHMENT_BYTES;
const PAIRED_PERTURB_UNIFORM_BYTES: u32 = 256;

/// Uniform capacity required when reconstruction follows an existing value pass.
pub const PAIRED_KERNEL_UNIFORM_BYTES: u32 = 288;

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

/// The registered Julibrot pipelines and their private grid-lifetime records.
pub struct JulibrotKernels {
    shallow: GpuKernel,
    perturbation: GpuKernel,
    shallow_reconstruction: Option<GpuKernel>,
    perturbation_reconstruction: Option<GpuKernel>,
    grids: Vec<GridAllocation>,
    paired_outputs: Vec<PairedOutputAllocation>,
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
            shallow_reconstruction: None,
            perturbation_reconstruction: None,
            grids: Vec::new(),
            paired_outputs: Vec::new(),
            latest_reference: RefCell::new(None),
        })
    }

    /// Registers one-output reconstruction passes that follow both existing value kernels.
    ///
    /// # Errors
    ///
    /// Returns `Register` when the executor cannot admit both reconstruction descriptors. The
    /// original single-output pipelines remain available after every refusal.
    pub fn enable_paired_outputs(
        &mut self,
        executor: &mut GpuKernelExecutor,
    ) -> Result<(), KernelError> {
        if self.shallow_reconstruction.is_some() && self.perturbation_reconstruction.is_some() {
            return Ok(());
        }
        if self.shallow_reconstruction.is_some() || self.perturbation_reconstruction.is_some() {
            return Err(KernelError::Register);
        }
        let shallow = register_reconstruction_kernel(
            executor,
            "julibrot_shallow_reconstruction",
            RECONSTRUCTION_ACCESSORS,
            "PairedShallowUniform",
            &shallow_kernel(),
            PAIRED_KERNEL_UNIFORM_BYTES,
        )?;
        let perturbation = register_reconstruction_kernel(
            executor,
            "julibrot_perturbation_reconstruction",
            PERTURBATION_RECONSTRUCTION_ACCESSORS,
            "PairedPerturbUniform",
            &perturbation_kernel(),
            PAIRED_PERTURB_UNIFORM_BYTES,
        )?;
        self.shallow_reconstruction = Some(shallow);
        self.perturbation_reconstruction = Some(perturbation);
        Ok(())
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
        Self::plan_for_grid_count(executor, requested_extent, params, 1, |records| {
            executor.plan_span(records, crate::OUTPUT_PAGE_SIDE).is_ok()
        })
    }

    /// Selects a plan whose Final-capacity record span can be allocated twice atomically.
    ///
    /// # Errors
    ///
    /// Returns a typed input, arithmetic, or live-capacity refusal without mutating the executor.
    pub fn plan_grid_pair(
        executor: &GpuKernelExecutor,
        requested_extent: GridExtent,
        params: EscapeParams,
    ) -> Result<RefinementPlan, KernelError> {
        Self::plan_for_grid_count(executor, requested_extent, params, 2, |records| {
            executor.plan_paired_copies(1, records, crate::OUTPUT_PAGE_SIDE) == 1
        })
    }

    fn plan_for_grid_count(
        executor: &GpuKernelExecutor,
        requested_extent: GridExtent,
        params: EscapeParams,
        grid_count: u32,
        mut spans_fit: impl FnMut(u32) -> bool,
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
            .and_then(|bytes| bytes.checked_mul(u64::from(grid_count)))
            .ok_or(KernelError::ArithmeticOverflow)?;
        crate::plan_refinement(requested_extent, params, |records| {
            records <= header_record_capacity
                && capacity.free_header_bytes >= required_header_bytes
                && spans_fit(records)
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

    /// Allocates two Final-capacity spans atomically and installs both grid records together.
    ///
    /// # Errors
    ///
    /// Returns a typed plan, allocation, or header-capacity refusal and rolls back both spans when
    /// either grid record cannot be prepared.
    pub fn allocate_grid_pair(
        &mut self,
        executor: &mut GpuKernelExecutor,
        plan: &RefinementPlan,
    ) -> Result<[EscapeGrid; 2], KernelError> {
        validate_plan(plan)?;
        let logical_len = pixel_count(plan.delivered_extent)?;
        let span_plan = executor
            .plan_span(logical_len, crate::OUTPUT_PAGE_SIDE)
            .map_err(|_| KernelError::Heap)?;
        let spans = executor
            .allocate_pair(logical_len, crate::OUTPUT_PAGE_SIDE)
            .map_err(|_| KernelError::Heap)?;
        let prepared = spans
            .clone()
            .map(|span| prepare_grid_allocation(executor, &self.shallow, span, plan, span_plan));
        let [first, second] = match prepared {
            [Ok(first), Ok(second)] => [first, second],
            [first, second] => {
                let error = first
                    .err()
                    .or_else(|| second.err())
                    .unwrap_or(KernelError::Dispatch);
                for span in spans {
                    executor.free_span(span).map_err(|_| KernelError::Heap)?;
                }
                return Err(error);
            }
        };
        let first_grid = first.0;
        let second_grid = second.0;
        self.grids.push(first.1);
        self.grids.push(second.1);
        Ok([first_grid, second_grid])
    }

    /// Allocates one generation-tagged value/reconstruction pair as an atomic heap operation.
    ///
    /// # Errors
    ///
    /// Returns a typed plan, allocation, header-capacity, arithmetic, or profile refusal. Both
    /// spans are retired if their generation-tagged allocation record cannot be constructed.
    pub fn allocate_output_pair(
        &mut self,
        executor: &mut GpuKernelExecutor,
        plan: &RefinementPlan,
        generation: u32,
    ) -> Result<([EscapeGrid; 2], PairedOutputAllocation), KernelError> {
        let grids = self.allocate_grid_pair(executor, plan)?;
        let [value_grid, reconstruction_grid] = &grids;
        let allocation = PairedOutputAllocation::from_spans(
            generation,
            &value_grid.span,
            &reconstruction_grid.span,
            executor.capacity_report().data_bytes,
        );
        let allocation = match allocation {
            Ok(allocation) => allocation,
            Err(error) => {
                for grid in grids {
                    self.free_grid(executor, grid)?;
                }
                return Err(tile_job_error(error));
            }
        };
        self.paired_outputs.push(allocation);
        Ok((grids, allocation))
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
        centre_from_reference_px: [f64; 2],
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
        let uniform = PerturbUniform::pack_referenced(
            *plane,
            screen_to_plane,
            centre_from_reference_px,
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

    /// Encodes byte-identical shallow values and source reconstruction as two ordered passes.
    ///
    /// # Errors
    ///
    /// Returns a typed pair, uniform, planning, resident-header, or copy-encoding refusal before
    /// publishing either active span extent.
    pub fn encode_shallow_pair(
        &self,
        executor: &GpuKernelExecutor,
        encoder: &mut wgpu::CommandEncoder,
        max_color_attachment_bytes_per_sample: u32,
        pair: (&mut [EscapeGrid; 2], PairedOutputAllocation),
        identity: (u64, PrecisionMode, RefinementLevel),
        uniforms: (&ShallowUniform, &SourceReconstructionUniform),
    ) -> Result<DispatchFacts, KernelError> {
        admit_paired_color_attachment_limit(max_color_attachment_bytes_per_sample)?;
        let reconstruction_kernel = self
            .shallow_reconstruction
            .as_ref()
            .ok_or(KernelError::Register)?;
        let (outputs, allocation) = pair;
        let (owner_epoch, precision_mode, level) = identity;
        let (value_uniform, source_uniform) = uniforms;
        let [value_grid, reconstruction_grid] = outputs;
        let [value_allocation, reconstruction_allocation] =
            self.paired_allocations(value_grid, reconstruction_grid, allocation)?;
        let selected = paired_level(
            value_allocation,
            reconstruction_allocation,
            level,
            value_uniform.width,
            value_uniform.height,
            value_uniform.max_iter,
        )?;
        let uniform_bytes = paired_uniform_bytes(value_uniform.bytes(), source_uniform.bytes());
        let value_dispatch = &value_allocation.shallow_dispatches[level_index(level)];
        let reconstruction_dispatch = executor
            .plan_prefix_dispatch(
                reconstruction_kernel,
                &[&value_allocation.span],
                &[&reconstruction_allocation.span],
                pixel_count(selected.extent)?,
                &uniform_bytes,
            )
            .map_err(|_| KernelError::Dispatch)?;
        let facts = checked_paired_facts(
            executor,
            (value_allocation, allocation),
            [value_dispatch, &reconstruction_dispatch],
            (level, KernelMode::Shallow, owner_epoch, precision_mode),
            None,
        )?;
        encode_pages(
            executor,
            encoder,
            &self.shallow,
            value_dispatch,
            &value_allocation.headers,
            level,
            value_uniform.bytes(),
        )?;
        executor.sync_dispatch_resources(&reconstruction_dispatch);
        encode_pages(
            executor,
            encoder,
            reconstruction_kernel,
            &reconstruction_dispatch,
            &reconstruction_allocation.headers,
            level,
            &uniform_bytes,
        )?;
        publish_level(value_grid, selected.extent, level);
        publish_level(reconstruction_grid, selected.extent, level);
        Ok(facts)
    }

    /// Encodes byte-identical perturbation values and reconstruction as two ordered passes.
    ///
    /// # Errors
    ///
    /// Returns a typed pair, reference, uniform, planning, resident-header, or copy-encoding
    /// refusal before publishing either active span extent.
    pub fn encode_perturbation_pair(
        &self,
        executor: &GpuKernelExecutor,
        encoder: &mut wgpu::CommandEncoder,
        max_color_attachment_bytes_per_sample: u32,
        pair: (&mut [EscapeGrid; 2], PairedOutputAllocation),
        identity: (u64, PrecisionMode, RefinementLevel),
        uniforms: (
            &PerturbUniform,
            &SourceReconstructionUniform,
            ReferenceOrbitInput<'_>,
        ),
    ) -> Result<DispatchFacts, KernelError> {
        admit_paired_color_attachment_limit(max_color_attachment_bytes_per_sample)?;
        let reconstruction_kernel = self
            .perturbation_reconstruction
            .as_ref()
            .ok_or(KernelError::Register)?;
        let (outputs, allocation) = pair;
        let (owner_epoch, precision_mode, level) = identity;
        let (value_uniform, source_uniform, reference) = uniforms;
        let [value_grid, reconstruction_grid] = outputs;
        let [value_allocation, reconstruction_allocation] =
            self.paired_allocations(value_grid, reconstruction_grid, allocation)?;
        self.validate_reference(reference, value_allocation.plan.requested_max_iter)?;
        if reference.precision_mode != precision_mode.as_str() {
            return Err(KernelError::ReferencePrecisionMismatch);
        }
        let selected = paired_level(
            value_allocation,
            reconstruction_allocation,
            level,
            value_uniform.width,
            value_uniform.height,
            value_uniform.max_iter,
        )?;
        let uniform_bytes = paired_uniform_bytes(value_uniform.bytes(), source_uniform.bytes());
        let resource_words = ensure_paired_reference_dispatches(
            executor,
            &self.perturbation,
            value_allocation,
            reference,
        )?;
        let cached = value_allocation.reference_dispatches.borrow();
        let value_dispatch = &cached
            .iter()
            .find(|cached| cached.resource_words == resource_words)
            .ok_or(KernelError::Dispatch)?
            .levels[level_index(level)];
        let reconstruction_dispatch = executor
            .plan_prefix_dispatch(
                reconstruction_kernel,
                &[reference.span, &value_allocation.span],
                &[&reconstruction_allocation.span],
                pixel_count(selected.extent)?,
                &uniform_bytes,
            )
            .map_err(|_| KernelError::Dispatch)?;
        let facts = checked_paired_facts(
            executor,
            (value_allocation, allocation),
            [value_dispatch, &reconstruction_dispatch],
            (level, KernelMode::Perturbation, owner_epoch, precision_mode),
            Some((reference.generation, reference.length)),
        )?;
        let _resources_changed =
            self.accept_reference(reference, value_allocation.plan.requested_max_iter)?;
        encode_pages(
            executor,
            encoder,
            &self.perturbation,
            value_dispatch,
            &value_allocation.headers,
            level,
            value_uniform.bytes(),
        )?;
        executor.sync_dispatch_resources(&reconstruction_dispatch);
        encode_pages(
            executor,
            encoder,
            reconstruction_kernel,
            &reconstruction_dispatch,
            &reconstruction_allocation.headers,
            level,
            &uniform_bytes,
        )?;
        publish_level(value_grid, selected.extent, level);
        publish_level(reconstruction_grid, selected.extent, level);
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
        let directory_index = grid.span.directory_index;
        executor
            .free_span(grid.span)
            .map_err(|_| KernelError::Heap)?;
        self.grids.swap_remove(position);
        self.paired_outputs.retain(|allocation| {
            allocation.spans.value.directory_index != directory_index
                && allocation.spans.reconstruction.directory_index != directory_index
        });
        Ok(())
    }

    fn allocation(&self, grid: &EscapeGrid) -> Result<&GridAllocation, KernelError> {
        self.grids
            .iter()
            .find(|allocation| allocation.span == grid.span)
            .ok_or(KernelError::Heap)
    }

    fn paired_allocations(
        &self,
        value_grid: &EscapeGrid,
        reconstruction_grid: &EscapeGrid,
        allocation: PairedOutputAllocation,
    ) -> Result<[&GridAllocation; 2], KernelError> {
        if !self.paired_outputs.contains(&allocation)
            || allocation.spans.value.directory_index != value_grid.span.directory_index
            || allocation.spans.value.logical_len != value_grid.span.logical_len
            || allocation.spans.reconstruction.directory_index
                != reconstruction_grid.span.directory_index
            || allocation.spans.reconstruction.logical_len != reconstruction_grid.span.logical_len
        {
            return Err(KernelError::Heap);
        }
        Ok([
            self.allocation(value_grid)?,
            self.allocation(reconstruction_grid)?,
        ])
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

const RECONSTRUCTION_BODY: &str = r"

struct SourceReconstructionUniform {
    camera_rotation_pairs: array<vec4<f32>, 5>,
    camera_translation: array<vec4<f32>, 2>,
    observer_rotation: vec4<f32>,
    view_scale: vec4<f32>,
}

struct __PAIRED_UNIFORM__ {
    value: __VALUE_UNIFORM__,
    source: SourceReconstructionUniform,
}

struct ReconstructionResult {
    reconstruction: vec4<f32>,
}

struct PairedAmbient5 {
    low: vec4<f32>,
    fifth: f32,
}

fn paired_finite(value: f32) -> bool {
    return abs(value) <= 3.402823e38;
}

fn paired_binary(value: f32) -> bool {
    return value == 0.0 || value == 1.0;
}

fn paired_terminal_status(value: f32) -> bool {
    return value == 0.0 || value == 1.0 || value == 2.0 || value == 3.0;
}

fn paired_record_height(record: vec4<f32>, max_iter: u32) -> f32 {
    let malformed = !paired_binary(record.y)
        || !paired_terminal_status(record.w)
        || !paired_finite(record.z)
        || record.z < 0.0
        || record.z != floor(record.z);
    if (malformed || record.w == 1.0 || record.w == 2.0) {
        return 0.0;
    }
    if (record.y == 0.0) {
        if (record.x == -1.0) {
            return -2.0;
        }
        return 0.0;
    }
    if (!paired_finite(record.x)) {
        return 0.0;
    }
    return 4.0 * clamp(record.x / max(f32(max_iter), 1.0), 0.0, 1.0) - 2.0;
}

fn paired_rotate(value: PairedAmbient5, axes: vec2<u32>, pair: vec2<f32>) -> PairedAmbient5 {
    var coordinates = array<f32, 5>(
        value.low.x,
        value.low.y,
        value.low.z,
        value.low.w,
        value.fifth,
    );
    let first = coordinates[axes.x];
    let second = coordinates[axes.y];
    coordinates[axes.x] = pair.x * first - pair.y * second;
    coordinates[axes.y] = pair.y * first + pair.x * second;
    var result: PairedAmbient5;
    result.low = vec4<f32>(coordinates[0], coordinates[1], coordinates[2], coordinates[3]);
    result.fifth = coordinates[4];
    return result;
}

fn paired_camera(value: PairedAmbient5, source: SourceReconstructionUniform) -> PairedAmbient5 {
    var rotated = paired_rotate(value, vec2<u32>(3u, 4u), source.camera_rotation_pairs[4].zw);
    rotated = paired_rotate(rotated, vec2<u32>(2u, 4u), source.camera_rotation_pairs[4].xy);
    rotated = paired_rotate(rotated, vec2<u32>(1u, 4u), source.camera_rotation_pairs[3].zw);
    rotated = paired_rotate(rotated, vec2<u32>(0u, 4u), source.camera_rotation_pairs[3].xy);
    rotated = paired_rotate(rotated, vec2<u32>(2u, 3u), source.camera_rotation_pairs[2].zw);
    rotated = paired_rotate(rotated, vec2<u32>(1u, 3u), source.camera_rotation_pairs[2].xy);
    rotated = paired_rotate(rotated, vec2<u32>(1u, 2u), source.camera_rotation_pairs[1].zw);
    rotated = paired_rotate(rotated, vec2<u32>(0u, 3u), source.camera_rotation_pairs[1].xy);
    rotated = paired_rotate(rotated, vec2<u32>(0u, 2u), source.camera_rotation_pairs[0].zw);
    rotated = paired_rotate(rotated, vec2<u32>(0u, 1u), source.camera_rotation_pairs[0].xy);
    rotated.low += source.camera_translation[0];
    rotated.fifth += source.camera_translation[1].x;
    return rotated;
}

fn paired_reconstruction(
    index: u32,
    record: vec4<f32>,
    uniforms: __PAIRED_UNIFORM__,
) -> vec4<f32> {
    if (!paired_binary(record.y) || !paired_terminal_status(record.w) || record.w == 2.0) {
        return vec4<f32>(0.0);
    }
    let column = index % uniforms.value.width;
    let row = index / uniforms.value.width;
    let x = f32(column) + 0.5 - 0.5 * f32(uniforms.value.width);
    let y = f32(row) + 0.5 - 0.5 * f32(uniforms.value.height);
    let screen = vec3<f32>(x, y, 1.0);
    let homogeneous = vec3<f32>(
        dot(uniforms.value.screen_to_plane_row_0.xyz, screen),
        dot(uniforms.value.screen_to_plane_row_1.xyz, screen),
        dot(uniforms.value.screen_to_plane_row_2.xyz, screen),
    );
    if (!all(vec3<bool>(
        paired_finite(homogeneous.x),
        paired_finite(homogeneous.y),
        paired_finite(homogeneous.z),
    )) || homogeneous.z <= 0.0) {
        return vec4<f32>(0.0);
    }
    let chart = uniforms.source.view_scale.w * homogeneous.xy / homogeneous.z;
    if (!all(vec2<bool>(paired_finite(chart.x), paired_finite(chart.y)))) {
        return vec4<f32>(0.0);
    }
    let local = chart.x * uniforms.value.basis_u + chart.y * uniforms.value.basis_v;
    let height = uniforms.source.view_scale.x
        * (paired_record_height(record, uniforms.value.max_iter) + 2.0)
        * 0.5;
    let ambient = paired_camera(PairedAmbient5(local, height), uniforms.source);
    let distance_five = uniforms.source.view_scale.y;
    let denominator_five = distance_five - ambient.fifth;
    if (denominator_five < 0.05 * distance_five || denominator_five <= 1.0e-4) {
        return vec4<f32>(0.0);
    }
    let projected_four = ambient.low * (distance_five / denominator_five);
    let distance_four = uniforms.source.view_scale.z;
    let denominator_four = distance_four - projected_four.w;
    if (denominator_four <= 1.0e-4) {
        return vec4<f32>(0.0);
    }
    let world = projected_four.xyz * (distance_four / denominator_four);
    let observer = uniforms.source.observer_rotation;
    let yawed_z = -observer.y * world.x + observer.x * world.z;
    let linear_depth = distance_four - observer.w * world.y - observer.z * yawed_z;
    if (!paired_finite(linear_depth) || linear_depth <= 1.0e-4) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(chart, linear_depth, 1.0);
}

fn kernel(index: u32, uniforms: __PAIRED_UNIFORM__) -> ReconstructionResult {
    let value = load_escape(index);
    var result: ReconstructionResult;
    result.reconstruction = paired_reconstruction(index, value, uniforms);
    return result;
}
";

fn register_reconstruction_kernel(
    executor: &mut GpuKernelExecutor,
    name: &str,
    accessors: &[&str],
    uniform_type: &str,
    value_descriptor: &KernelDesc<'_>,
    uniform_size: u32,
) -> Result<GpuKernel, KernelError> {
    let body = reconstruction_kernel_body(value_descriptor, uniform_type)?;
    let descriptor = KernelDesc {
        name,
        body: &body,
        accessors,
        output_fields: RECONSTRUCTION_OUTPUT_FIELDS,
        uniform_type,
        uniform_size,
        output_page_side: crate::OUTPUT_PAGE_SIDE,
    };
    let registered = RegisteredKernel::register(&descriptor, executor.dialect_limits())
        .map_err(|_| KernelError::Register)?;
    executor
        .register_kernel(registered)
        .map_err(|_| KernelError::Register)
}

fn reconstruction_kernel_body(
    value_descriptor: &KernelDesc<'_>,
    paired_uniform: &str,
) -> Result<String, KernelError> {
    let value_declaration = value_uniform_declaration(value_descriptor)?;
    let reconstruction = RECONSTRUCTION_BODY
        .replace("__PAIRED_UNIFORM__", paired_uniform)
        .replace("__VALUE_UNIFORM__", value_descriptor.uniform_type);
    let mut body = String::with_capacity(value_declaration.len() + reconstruction.len());
    body.push_str(value_declaration);
    body.push_str(&reconstruction);
    Ok(body)
}

fn value_uniform_declaration<'a>(
    value_descriptor: &KernelDesc<'a>,
) -> Result<&'a str, KernelError> {
    let (declaration, _) = value_descriptor
        .body
        .split_once("\n\n")
        .ok_or(KernelError::Register)?;
    let declaration_start = format!("struct {} {{", value_descriptor.uniform_type);
    if !declaration.starts_with(&declaration_start) || !declaration.ends_with('}') {
        return Err(KernelError::Register);
    }
    Ok(declaration)
}

fn paired_uniform_bytes(value: &[u8], source: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(value.len() + source.len());
    bytes.extend_from_slice(value);
    bytes.extend_from_slice(source);
    bytes
}

fn ensure_paired_reference_dispatches(
    executor: &GpuKernelExecutor,
    kernel: &GpuKernel,
    allocation: &GridAllocation,
    reference: ReferenceOrbitInput<'_>,
) -> Result<[u32; 4], KernelError> {
    let resource_words = [
        reference.span.directory_index,
        reference.span.logical_len,
        0,
        0,
    ];
    let is_cached = allocation
        .reference_dispatches
        .borrow()
        .iter()
        .any(|cached| cached.resource_words == resource_words);
    if !is_cached {
        let levels = dispatch_templates(
            executor,
            kernel,
            &[reference.span],
            &allocation.span,
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
    Ok(resource_words)
}

const fn admit_paired_color_attachment_limit(
    max_color_attachment_bytes_per_sample: u32,
) -> Result<(), KernelError> {
    if max_color_attachment_bytes_per_sample < RGBA32F_COLOR_ATTACHMENT_BYTES {
        Err(KernelError::OutputTransferUnsupported)
    } else {
        Ok(())
    }
}

fn paired_level(
    value: &GridAllocation,
    reconstruction: &GridAllocation,
    level: RefinementLevel,
    width: u32,
    height: u32,
    max_iter: u32,
) -> Result<crate::LevelSpec, KernelError> {
    let selected = value.plan.level(level);
    if value.plan != reconstruction.plan
        || selected.extent != (GridExtent { width, height })
        || selected.iteration_cap != max_iter
    {
        return Err(KernelError::Dispatch);
    }
    Ok(selected)
}

const fn tile_job_error(error: TileJobError) -> KernelError {
    match error {
        TileJobError::ArithmeticOverflow => KernelError::ArithmeticOverflow,
        _ => KernelError::Heap,
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

fn prepare_grid_allocation(
    executor: &mut GpuKernelExecutor,
    shallow: &GpuKernel,
    span: DataSpan,
    plan: &RefinementPlan,
    span_plan: SpanPlan,
) -> Result<(EscapeGrid, GridAllocation), KernelError> {
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
        .collect::<Result<Vec<_>, _>>()?;
    let header_sets = executor
        .reserve_header_sets(&headers)
        .map_err(|_| KernelError::Dispatch)?;
    if header_sets.set_count() != LEVEL_COUNT {
        return Err(KernelError::Dispatch);
    }
    let shallow_dispatches = dispatch_templates(
        executor,
        shallow,
        &[],
        &span,
        plan,
        &[0; core::mem::size_of::<ShallowUniform>()],
    )?;
    let first = plan.level(RefinementLevel::Preview);
    let grid = EscapeGrid {
        span: span.clone(),
        width: first.extent.width,
        height: first.extent.height,
        level: first.level,
    };
    let allocation = GridAllocation {
        span,
        headers: header_sets,
        plan: *plan,
        span_plan,
        shallow_dispatches,
        reference_dispatches: RefCell::new(Vec::with_capacity(2)),
    };
    Ok((grid, allocation))
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

fn checked_paired_facts(
    executor: &GpuKernelExecutor,
    allocation: (&GridAllocation, PairedOutputAllocation),
    dispatches: [&ExecutorDispatch; 2],
    identity: (RefinementLevel, KernelMode, u64, PrecisionMode),
    orbit: Option<(u32, u32)>,
) -> Result<DispatchFacts, KernelError> {
    let (value_allocation, paired_allocation) = allocation;
    let (level, mode, owner_epoch, precision_mode) = identity;
    let mut facts = dispatch_facts(
        &value_allocation.plan,
        level,
        mode,
        owner_epoch,
        precision_mode,
        executor.capacity_report().scratch_bytes,
        orbit,
    )?;
    let page_passes = facts
        .page_passes
        .checked_mul(2)
        .ok_or(KernelError::ArithmeticOverflow)?;
    let paired_copy_commands = facts
        .copy_commands
        .checked_mul(2)
        .ok_or(KernelError::ArithmeticOverflow)?;
    let paired_copy_bytes = facts
        .gpu_copy_bytes
        .checked_mul(2)
        .ok_or(KernelError::ArithmeticOverflow)?;
    for dispatch in dispatches {
        let dispatch_page_passes = u32::try_from(dispatch.plan().passes.len())
            .map_err(|_| KernelError::ArithmeticOverflow)?;
        if dispatch_page_passes != facts.page_passes
            || dispatch.copy_commands() != facts.copy_commands
            || dispatch.plan().gpu_copy_bytes != facts.gpu_copy_bytes
        {
            return Err(KernelError::Dispatch);
        }
    }
    facts.page_passes = page_passes;
    facts.copy_commands = paired_copy_commands;
    facts.gpu_copy_bytes = paired_copy_bytes;
    facts.logical_heap_bytes = facts
        .logical_heap_bytes
        .checked_mul(2)
        .ok_or(KernelError::ArithmeticOverflow)?;
    facts.reserved_heap_bytes = paired_allocation.reserved_bytes;
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
    executor
        .encode_dispatch_selected_set(encoder, kernel, dispatch, headers, level_set(level))
        .map_err(|_| KernelError::Dispatch)
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

    use super::{
        AcceptedReference, PAIRED_KERNEL_UNIFORM_BYTES, PAIRED_PERTURB_UNIFORM_BYTES,
        PAIRED_TRANSACTION_COLOR_ATTACHMENT_BYTES, PERTURBATION_RECONSTRUCTION_ACCESSORS,
        RECONSTRUCTION_ACCESSORS, RECONSTRUCTION_OUTPUT_FIELDS, RGBA32F_COLOR_ATTACHMENT_BYTES,
        accept_reference_transition, admit_paired_color_attachment_limit,
        reconstruction_kernel_body, value_uniform_declaration,
    };
    use crate::{
        EscapeParams, GridExtent, KernelError, ReferenceOrbitInput, RefinementLevel,
        ShallowUniform, SourceReconstructionUniform, perturbation_kernel, plan_refinement,
        shallow_kernel,
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

    fn app_main_pair_plan(requested_extent: GridExtent) -> (SpanArena, crate::RefinementPlan) {
        let arena =
            SpanArena::new(512, 16, 64, 16 * 16 + 128 * 4, 16).expect("browser heap planner arena");
        let plan = plan_refinement(requested_extent, EscapeParams::new(4_096), |records| {
            arena.plan_paired_copies(1, records, crate::OUTPUT_PAGE_SIDE) == 1
        })
        .expect("a degraded paired browser plan fits");
        (arena, plan)
    }

    #[test]
    fn reconstruction_descriptors_read_s0_and_register_one_output_per_pass() {
        let limits = DialectLimits {
            descriptor_capacity: 64,
            span_capacity: 16,
            handle_capacity: 64,
        };
        for (name, accessors, paired_uniform, value_descriptor, uniform_size) in [
            (
                "julibrot_shallow_reconstruction",
                RECONSTRUCTION_ACCESSORS,
                "PairedShallowUniform",
                shallow_kernel(),
                PAIRED_KERNEL_UNIFORM_BYTES,
            ),
            (
                "julibrot_perturbation_reconstruction",
                PERTURBATION_RECONSTRUCTION_ACCESSORS,
                "PairedPerturbUniform",
                perturbation_kernel(),
                PAIRED_PERTURB_UNIFORM_BYTES,
            ),
        ] {
            let declaration = value_uniform_declaration(&value_descriptor)
                .expect("value uniform declaration is present");
            let body = reconstruction_kernel_body(&value_descriptor, paired_uniform)
                .expect("reconstruction body derives from the value descriptor");
            let copied_declaration = body.split_once("\n\n").map(|(block, _)| block);
            assert_eq!(copied_declaration, Some(declaration));
            let descriptor = ember_lab_heap::KernelDesc {
                name,
                body: &body,
                accessors,
                output_fields: RECONSTRUCTION_OUTPUT_FIELDS,
                uniform_type: paired_uniform,
                uniform_size,
                output_page_side: crate::OUTPUT_PAGE_SIDE,
            };
            let registered = RegisteredKernel::register(&descriptor, limits)
                .expect("reconstruction descriptor satisfies dialect v2");
            assert_eq!(registered.output_count(), 1);
            assert_eq!(registered.uniform_size(), uniform_size);
            assert!(
                registered
                    .source()
                    .contains("let value = load_escape(index);")
            );
            assert!(
                registered
                    .source()
                    .contains("return vec4<f32>(chart, linear_depth, 1.0);")
            );
        }
        assert_eq!(
            PAIRED_KERNEL_UNIFORM_BYTES,
            u32::try_from(
                core::mem::size_of::<ShallowUniform>()
                    + core::mem::size_of::<SourceReconstructionUniform>()
            )
            .expect("paired shallow uniform size fits u32")
        );
        assert_eq!(
            PAIRED_PERTURB_UNIFORM_BYTES,
            u32::try_from(
                core::mem::size_of::<crate::PerturbUniform>()
                    + core::mem::size_of::<SourceReconstructionUniform>()
            )
            .expect("paired perturbation uniform size fits u32")
        );
    }

    #[test]
    fn browser_final_pair_admission_rejects_when_equal_spans_need_more_than_63_live_descriptor_slots()
     {
        let floor = wgpu::Limits::downlevel_webgl2_defaults().max_color_attachment_bytes_per_sample;
        assert_eq!(RGBA32F_COLOR_ATTACHMENT_BYTES, 16);
        assert_eq!(PAIRED_TRANSACTION_COLOR_ATTACHMENT_BYTES, 32);
        assert_eq!(floor, PAIRED_TRANSACTION_COLOR_ATTACHMENT_BYTES);
        assert!(RGBA32F_COLOR_ATTACHMENT_BYTES < floor);
        assert_eq!(admit_paired_color_attachment_limit(floor), Ok(()));
        assert_eq!(
            admit_paired_color_attachment_limit(RGBA32F_COLOR_ATTACHMENT_BYTES - 1),
            Err(KernelError::OutputTransferUnsupported)
        );
        for (requested_extent, delivered_extent, divisor) in [
            (
                GridExtent {
                    width: 1_600,
                    height: 900,
                },
                GridExtent {
                    width: 1_600,
                    height: 900,
                },
                1,
            ),
            (
                GridExtent {
                    width: 1_920,
                    height: 1_080,
                },
                GridExtent {
                    width: 960,
                    height: 540,
                },
                2,
            ),
            (
                GridExtent {
                    width: 2_560,
                    height: 1_440,
                },
                GridExtent {
                    width: 1_280,
                    height: 720,
                },
                2,
            ),
        ] {
            let (mut arena, plan) = app_main_pair_plan(requested_extent);
            assert_eq!(plan.delivered_extent, delivered_extent);
            assert_eq!(plan.extent_divisor, divisor);
            let records = delivered_extent.width * delivered_extent.height;
            let pair = arena
                .allocate_pair(records, crate::OUTPUT_PAGE_SIDE)
                .expect("the admitted Final pair allocates atomically");
            assert_eq!(pair[0].page_count, pair[1].page_count);
        }

        let (arena, _) = app_main_pair_plan(GridExtent {
            width: 1_920,
            height: 1_080,
        });
        let page_records = u32::from(crate::OUTPUT_PAGE_SIDE).pow(2);
        let admitted_span_pages = 2_031_616_u32.div_ceil(page_records);
        let refused_span_pages = 2_031_617_u32.div_ceil(page_records);
        assert_eq!(admitted_span_pages, 31);
        assert_eq!(refused_span_pages, 32);
        assert_eq!(admitted_span_pages * 2, 62);
        assert_eq!(refused_span_pages * 2, 64);
        assert_eq!(
            arena.plan_paired_copies(1, 2_031_616, crate::OUTPUT_PAGE_SIDE),
            1
        );
        assert_eq!(
            arena.plan_paired_copies(1, 2_031_617, crate::OUTPUT_PAGE_SIDE),
            0
        );
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
