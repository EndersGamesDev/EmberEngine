//! Public owner of the heap lattice's paid DATA/SCRATCH execution path.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::redundant_pub_crate,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use std::num::NonZeroU64;
use std::sync::Arc;

use thiserror::Error;
use wgpu::util::DeviceExt as _;

use crate::span::SpanIdentity;
use crate::{
    DataSpan, DialectLimits, DispatchError, DispatchPlan, RegisteredKernel, SpanArena, SpanError,
    SpanPlan, StaticHeaders,
};

const RESOURCE_SLOTS: usize = 8;
const RECORD_BYTES: usize = 16;

/// Fixed capacities used to create one executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuKernelExecutorConfig {
    pub heap_side: u16,
    pub heap_layers: u16,
    pub descriptor_capacity: u32,
    pub span_capacity: u32,
    pub directory_binding_bytes: u32,
    pub scratch_layers: u32,
    pub max_header_pages: u32,
    pub max_header_sets: u32,
    pub kernel_uniform_bytes: u32,
}

/// Capability-gated DATA upload layout.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpanUploadMode {
    /// Preserve the established one-full-square-staging-page upload.
    #[default]
    PaddedPages,
    /// Upload only complete logical rows plus an optional exact-width tail row.
    ValidRows,
}

/// Created capacity and byte facts; none are inferred measurements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutorCapacity {
    pub heap_side: u16,
    pub heap_layers: u16,
    pub descriptor_capacity: u32,
    pub free_descriptors: u32,
    pub span_capacity: u32,
    pub handle_capacity: u32,
    pub scratch_layers: u32,
    pub header_stride: u32,
    pub max_header_pages: u32,
    pub max_header_sets: u32,
    pub header_buffer_bytes: u64,
    pub free_header_bytes: u64,
    pub kernel_uniform_bytes: u32,
    pub data_bytes: u64,
    pub scratch_bytes: u64,
}

/// One immutable header set and page selected for a fragment dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchSelector {
    /// Zero-based set within the reservation.
    pub set: u32,
    /// Zero-based page within that set.
    pub page: u32,
}

/// Opaque, generation-checked reservation in one executor's static header buffer.
#[derive(Clone, Debug)]
pub struct HeaderSetHandle {
    owner: Arc<()>,
    slot: u32,
    generation: u32,
    set_count: u32,
    base_offset: u32,
    stride: u32,
}

impl HeaderSetHandle {
    /// Number of immutable sets in this reservation.
    #[must_use]
    pub const fn set_count(&self) -> u32 {
        self.set_count
    }

    /// Byte offset of the reservation's first fixed-size set region.
    #[must_use]
    pub const fn base_offset(&self) -> u32 {
        self.base_offset
    }

    /// Dynamic-uniform alignment between page headers.
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }
}

#[derive(Clone, Debug)]
struct HeaderReservation {
    span: SpanIdentity,
    first_set: u32,
    headers: Vec<StaticHeaders>,
}

#[derive(Clone, Debug)]
struct HeaderReservationSlot {
    generation: u32,
    reservation: Option<HeaderReservation>,
    retired: bool,
}

#[derive(Debug)]
struct HeaderReservations {
    occupied: Vec<bool>,
    slots: Vec<HeaderReservationSlot>,
    max_header_pages: u32,
    stride: u32,
}

impl HeaderReservations {
    fn new(max_header_sets: u32, max_header_pages: u32, stride: u32) -> Self {
        Self {
            occupied: vec![false; max_header_sets as usize],
            slots: Vec::new(),
            max_header_pages,
            stride,
        }
    }

    fn free_sets(&self) -> u32 {
        self.occupied.iter().filter(|occupied| !**occupied).count() as u32
    }

    fn longest_free_run(&self) -> u32 {
        let mut current = 0_u32;
        let mut longest = 0_u32;
        for occupied in &self.occupied {
            if *occupied {
                current = 0;
            } else {
                current += 1;
                longest = longest.max(current);
            }
        }
        longest
    }

    fn capacity_error(&self, requested_sets: u32) -> DispatchError {
        DispatchError::HeaderSetCapacity {
            requested_sets,
            total_free_sets: self.free_sets(),
            longest_free_run: self.longest_free_run(),
        }
    }

    fn find(&self, executor: &Arc<()>, headers: &[StaticHeaders]) -> Option<HeaderSetHandle> {
        self.slots.iter().enumerate().find_map(|(slot, entry)| {
            let reservation = entry.reservation.as_ref()?;
            (reservation.headers == headers)
                .then(|| self.make_handle(executor, slot as u32, entry, reservation))
        })
    }

    fn reserve(
        &mut self,
        executor: &Arc<()>,
        headers: &[StaticHeaders],
    ) -> Result<HeaderSetHandle, DispatchError> {
        let requested = headers.len() as u32;
        let run = self
            .occupied
            .windows(headers.len())
            .position(|window| window.iter().all(|occupied| !*occupied));
        let Some(run) = run else {
            return Err(self.capacity_error(requested));
        };
        let run = run as u32;
        let slot = self
            .slots
            .iter()
            .position(|entry| entry.reservation.is_none() && !entry.retired)
            .unwrap_or(self.slots.len());
        if slot == self.slots.len() {
            self.slots.push(HeaderReservationSlot {
                generation: 1,
                reservation: None,
                retired: false,
            });
        }
        self.occupied[run as usize..(run + requested) as usize].fill(true);
        self.slots[slot].reservation = Some(HeaderReservation {
            span: headers[0].owner().clone(),
            first_set: run,
            headers: headers.to_vec(),
        });
        Ok(HeaderSetHandle {
            owner: Arc::clone(executor),
            slot: slot as u32,
            generation: self.slots[slot].generation,
            set_count: requested,
            base_offset: run * self.max_header_pages * self.stride,
            stride: self.stride,
        })
    }

    fn resolve<'a>(
        &'a self,
        executor: &Arc<()>,
        handle: &HeaderSetHandle,
        selector: DispatchSelector,
    ) -> Result<(&'a StaticHeaders, u32), DispatchError> {
        if !Arc::ptr_eq(executor, &handle.owner) {
            return Err(DispatchError::ForeignHeaderSet);
        }
        let entry = self
            .slots
            .get(handle.slot as usize)
            .ok_or(DispatchError::StaleHeaderSet)?;
        let reservation = entry
            .reservation
            .as_ref()
            .filter(|_| entry.generation == handle.generation)
            .ok_or(DispatchError::StaleHeaderSet)?;
        if selector.set >= reservation.headers.len() as u32 {
            return Err(DispatchError::HeaderSetSelection {
                set: selector.set,
                set_count: reservation.headers.len() as u32,
            });
        }
        let headers = &reservation.headers[selector.set as usize];
        if selector.page >= headers.offsets.len() as u32 {
            return Err(DispatchError::HeaderPageSelection {
                set: selector.set,
                page: selector.page,
                page_count: headers.offsets.len() as u32,
            });
        }
        if handle.set_count != reservation.headers.len() as u32
            || handle.base_offset != reservation.first_set * self.max_header_pages * self.stride
            || handle.stride != self.stride
        {
            return Err(DispatchError::StaleHeaderSet);
        }
        let absolute_offset =
            (reservation.first_set + selector.set) * self.max_header_pages * self.stride
                + headers.offsets[selector.page as usize];
        Ok((headers, absolute_offset))
    }

    fn release_span(&mut self, span: &SpanIdentity) {
        for entry in &mut self.slots {
            let Some(reservation) = entry
                .reservation
                .as_ref()
                .filter(|reservation| &reservation.span == span)
            else {
                continue;
            };
            let start = reservation.first_set as usize;
            let end = start + reservation.headers.len();
            self.occupied[start..end].fill(false);
            entry.reservation = None;
            if let Some(next) = entry.generation.checked_add(1) {
                entry.generation = next;
            } else {
                entry.retired = true;
            }
        }
    }

    fn make_handle(
        &self,
        executor: &Arc<()>,
        slot: u32,
        entry: &HeaderReservationSlot,
        reservation: &HeaderReservation,
    ) -> HeaderSetHandle {
        HeaderSetHandle {
            owner: Arc::clone(executor),
            slot,
            generation: entry.generation,
            set_count: reservation.headers.len() as u32,
            base_offset: reservation.first_set * self.max_header_pages * self.stride,
            stride: self.stride,
        }
    }
}

/// Stable identities used to create present's immutable heap bind group.
#[derive(Clone)]
pub struct HeapPresentResources {
    pub data_view: Arc<wgpu::TextureView>,
    pub descriptor_buffer: Arc<wgpu::Buffer>,
    pub span_directory_buffer: Arc<wgpu::Buffer>,
    pub descriptor_capacity: u32,
    pub span_capacity: u32,
    pub handle_capacity: u32,
}

/// A validated dialect kernel and its immutable GPU pipeline.
pub struct GpuKernel {
    id: u64,
    registered: RegisteredKernel,
    pipeline: wgpu::RenderPipeline,
    outputs: usize,
    page_side: u16,
    uniform_size: u32,
}

impl GpuKernel {
    #[must_use]
    pub fn name(&self) -> &str {
        self.registered.name()
    }

    #[must_use]
    pub const fn page_side(&self) -> u16 {
        self.page_side
    }

    #[must_use]
    pub const fn uniform_size(&self) -> u32 {
        self.uniform_size
    }
}

/// A validated dispatch plus its immutable prefix headers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutorDispatch {
    kernel_id: u64,
    plan: DispatchPlan,
    headers: StaticHeaders,
    page_side: u16,
    copy_commands: u32,
}

impl ExecutorDispatch {
    #[must_use]
    pub const fn plan(&self) -> &DispatchPlan {
        &self.plan
    }

    #[must_use]
    pub const fn copy_commands(&self) -> u32 {
        self.copy_commands
    }

    #[must_use]
    pub const fn headers(&self) -> &StaticHeaders {
        &self.headers
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn set_resource(
        &mut self,
        slot: usize,
        span: &DataSpan,
    ) -> Result<(), ExecutorError> {
        let words = self
            .plan
            .resource_words
            .get_mut(slot)
            .ok_or(ExecutorError::Contract("resource slot is outside 0..8"))?;
        *words = [span.directory_index, span.logical_len, 0, 0];
        Ok(())
    }
}

/// Typed executor configuration, upload, or dispatch failure.
#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error(transparent)]
    Span(#[from] SpanError),
    #[error(transparent)]
    Dialect(#[from] crate::DialectError),
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
    #[error("GPU kernel executor contract failed: {0}")]
    Contract(&'static str),
    #[error("DATA upload has {actual} bytes; expected {expected}")]
    UploadSize { actual: usize, expected: usize },
}

/// Opaque owner of the lattice allocator, fixed resources, and copy encoder.
pub struct GpuKernelExecutor {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: GpuKernelExecutorConfig,
    arena: SpanArena,
    data: wgpu::Texture,
    scratch: wgpu::Texture,
    data_view: Arc<wgpu::TextureView>,
    heap_layout: wgpu::BindGroupLayout,
    heap_group: wgpu::BindGroup,
    descriptor_buffer: Arc<wgpu::Buffer>,
    directory_buffer: Arc<wgpu::Buffer>,
    header_buffer: wgpu::Buffer,
    resources_buffer: wgpu::Buffer,
    kernel_uniform_buffer: wgpu::Buffer,
    header_stride: u32,
    header_owner: Arc<()>,
    header_reservations: HeaderReservations,
    compatibility_header: Option<(StaticHeaders, HeaderSetHandle)>,
    span_upload_mode: SpanUploadMode,
    next_kernel_id: u64,
}

impl GpuKernelExecutor {
    /// Creates the fixed allocator, GPU resources, and one immutable bind group.
    ///
    /// # Errors
    ///
    /// Returns a typed configuration or arena-construction failure before publication.
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: GpuKernelExecutorConfig,
    ) -> Result<Self, ExecutorError> {
        if config.heap_side == 0
            || config.heap_layers == 0
            || !(1..=4).contains(&config.scratch_layers)
            || config.max_header_pages == 0
            || config.max_header_sets == 0
            || config.kernel_uniform_bytes == 0
            || !config.kernel_uniform_bytes.is_multiple_of(16)
        {
            return Err(ExecutorError::Contract("invalid fixed capacity"));
        }
        let arena = SpanArena::new(
            config.heap_side,
            config.heap_layers,
            config.descriptor_capacity,
            config.directory_binding_bytes,
            config.span_capacity,
        )?;
        let data = texture(
            &device,
            "heap executor DATA",
            u32::from(config.heap_side),
            u32::from(config.heap_layers),
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        );
        let scratch = texture(
            &device,
            "heap executor four-layer SCRATCH",
            u32::from(config.heap_side),
            config.scratch_layers,
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let descriptor_buffer = Arc::new(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("heap executor descriptor UBO"),
                contents: bytemuck::cast_slice(&arena.heap().packed_table()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        ));
        let directory_buffer = Arc::new(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("heap executor span directory UBO"),
                contents: bytemuck::cast_slice(&arena.directory().packed_words()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        ));
        let header_stride = device.limits().min_uniform_buffer_offset_alignment.max(16);
        let header_buffer_bytes = u64::from(header_stride)
            .checked_mul(u64::from(config.max_header_pages))
            .and_then(|bytes| bytes.checked_mul(u64::from(config.max_header_sets)))
            .filter(|bytes| u32::try_from(*bytes).is_ok())
            .ok_or(ExecutorError::Contract(
                "header buffer byte capacity overflow",
            ))?;
        let header_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap executor static dispatch headers"),
            size: header_buffer_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let resources_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap executor step resources"),
            contents: bytemuck::bytes_of(&[[0_u32; 4]; RESOURCE_SLOTS]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let kernel_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap executor kernel uniform"),
            size: u64::from(config.kernel_uniform_bytes),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let heap_layout = heap_layout(&device, config);
        let data_view = Arc::new(data.create_view(&wgpu::TextureViewDescriptor {
            label: Some("heap executor full DATA array view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(u32::from(config.heap_layers)),
            ..Default::default()
        }));
        let heap_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heap executor one immutable bind group"),
            layout: &heap_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&data_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: descriptor_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: directory_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &header_buffer,
                        offset: 0,
                        size: NonZeroU64::new(16),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: resources_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: kernel_uniform_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            device,
            queue,
            config,
            arena,
            data,
            scratch,
            data_view,
            heap_layout,
            heap_group,
            descriptor_buffer,
            directory_buffer,
            header_buffer,
            resources_buffer,
            kernel_uniform_buffer,
            header_stride,
            header_owner: Arc::new(()),
            header_reservations: HeaderReservations::new(
                config.max_header_sets,
                config.max_header_pages,
                header_stride,
            ),
            compatibility_header: None,
            span_upload_mode: SpanUploadMode::PaddedPages,
            next_kernel_id: 1,
        })
    }

    /// Selects the capability-gated DATA upload layout for later `write_span` calls.
    pub const fn set_span_upload_mode(&mut self, mode: SpanUploadMode) {
        self.span_upload_mode = mode;
    }

    /// Returns the selected DATA upload layout.
    #[must_use]
    pub const fn span_upload_mode(&self) -> SpanUploadMode {
        self.span_upload_mode
    }

    #[must_use]
    pub fn capacity_report(&self) -> ExecutorCapacity {
        let side = u64::from(self.config.heap_side);
        ExecutorCapacity {
            heap_side: self.config.heap_side,
            heap_layers: self.config.heap_layers,
            descriptor_capacity: self.config.descriptor_capacity,
            free_descriptors: self.arena.heap().free_descriptor_count() as u32,
            span_capacity: self.config.span_capacity,
            handle_capacity: self.arena.directory().handle_capacity(),
            scratch_layers: self.config.scratch_layers,
            header_stride: self.header_stride,
            max_header_pages: self.config.max_header_pages,
            max_header_sets: self.config.max_header_sets,
            header_buffer_bytes: u64::from(self.header_stride)
                * u64::from(self.config.max_header_pages)
                * u64::from(self.config.max_header_sets),
            free_header_bytes: u64::from(self.header_stride)
                * u64::from(self.config.max_header_pages)
                * u64::from(self.header_reservations.free_sets()),
            kernel_uniform_bytes: self.config.kernel_uniform_bytes,
            data_bytes: side * side * u64::from(self.config.heap_layers) * 16,
            scratch_bytes: side * side * u64::from(self.config.scratch_layers) * 16,
        }
    }

    #[must_use]
    pub fn present_resources(&self) -> HeapPresentResources {
        HeapPresentResources {
            data_view: Arc::clone(&self.data_view),
            descriptor_buffer: Arc::clone(&self.descriptor_buffer),
            span_directory_buffer: Arc::clone(&self.directory_buffer),
            descriptor_capacity: self.config.descriptor_capacity,
            span_capacity: self.config.span_capacity,
            handle_capacity: self.arena.directory().handle_capacity(),
        }
    }

    #[must_use]
    pub const fn dialect_limits(&self) -> DialectLimits {
        DialectLimits {
            descriptor_capacity: self.config.descriptor_capacity,
            span_capacity: self.config.span_capacity,
            handle_capacity: self.arena.directory().handle_capacity(),
        }
    }

    /// Creates the immutable GPU pipeline for a validated dialect kernel.
    ///
    /// # Errors
    ///
    /// Returns a capacity error before pipeline creation.
    pub fn register_kernel(
        &mut self,
        registered: RegisteredKernel,
    ) -> Result<GpuKernel, ExecutorError> {
        let outputs = registered.output_count();
        if outputs > self.config.scratch_layers as usize
            || registered.uniform_size() > self.config.kernel_uniform_bytes
        {
            return Err(ExecutorError::Contract(
                "kernel exceeds fixed SCRATCH or uniform capacity",
            ));
        }
        let pipeline = compute_pipeline(
            &self.device,
            &self.heap_layout,
            registered.name(),
            registered.source(),
            outputs,
        );
        let id = self.next_kernel_id;
        self.next_kernel_id = id
            .checked_add(1)
            .ok_or(ExecutorError::Contract("kernel id exhausted"))?;
        Ok(GpuKernel {
            id,
            page_side: registered.output_page_side(),
            uniform_size: registered.uniform_size(),
            registered,
            pipeline,
            outputs,
        })
    }

    /// Allocates one DATA span and publishes changed metadata.
    ///
    /// # Errors
    ///
    /// Returns the arena's typed allocation failure.
    pub fn allocate_span(
        &mut self,
        logical_len: u32,
        page_side: u16,
    ) -> Result<DataSpan, SpanError> {
        let span = self.arena.allocate_span(logical_len, page_side)?;
        self.sync_heap_metadata();
        Ok(span)
    }

    /// Allocates two DATA spans atomically and publishes changed metadata.
    ///
    /// # Errors
    ///
    /// Returns the arena's typed allocation failure.
    pub fn allocate_pair(
        &mut self,
        logical_len: u32,
        page_side: u16,
    ) -> Result<[DataSpan; 2], SpanError> {
        let spans = self.arena.allocate_pair(logical_len, page_side)?;
        self.sync_heap_metadata();
        Ok(spans)
    }

    #[must_use]
    pub fn plan_paired_copies(
        &self,
        requested_copies: u64,
        records_per_copy: u32,
        page_side: u16,
    ) -> u64 {
        self.arena
            .plan_paired_copies(requested_copies, records_per_copy, page_side)
    }

    /// Trials one exact DATA-span allocation without mutating live allocator state.
    ///
    /// # Errors
    ///
    /// Returns exactly the typed failure that a real single-span allocation would return for the
    /// current buddy and directory state.
    pub fn plan_span(&self, logical_len: u32, page_side: u16) -> Result<SpanPlan, SpanError> {
        self.arena.plan_span(logical_len, page_side)
    }

    /// Frees one DATA span and publishes changed metadata.
    ///
    /// # Errors
    ///
    /// Returns the arena's typed stale-handle or directory failure.
    pub fn free_span(&mut self, span: DataSpan) -> Result<(), SpanError> {
        let identity = span.identity();
        self.arena.free(span)?;
        self.header_reservations.release_span(&identity);
        if self
            .compatibility_header
            .as_ref()
            .is_some_and(|(headers, _)| headers.owner() == &identity)
        {
            self.compatibility_header = None;
        }
        self.sync_heap_metadata();
        Ok(())
    }

    /// Writes exact 16-byte records into one DATA span.
    ///
    /// # Errors
    ///
    /// Returns a byte-count or stale-handle failure.
    pub fn write_span(&self, span: &DataSpan, bytes: &[u8]) -> Result<(), ExecutorError> {
        let expected = span.logical_len as usize * RECORD_BYTES;
        if bytes.len() != expected {
            return Err(ExecutorError::UploadSize {
                actual: bytes.len(),
                expected,
            });
        }
        let side = span.page_records.isqrt();
        if side == 0 || side * side != span.page_records {
            return Err(ExecutorError::Contract("span page is not square"));
        }
        for (page, handle) in span.handles().iter().enumerate() {
            let descriptor = self
                .arena
                .heap()
                .resolve(*handle)
                .map_err(SpanError::from)?;
            let start = page * span.page_records as usize * RECORD_BYTES;
            let end = bytes
                .len()
                .min(start + span.page_records as usize * RECORD_BYTES);
            let valid_records = u32::try_from((end - start) / RECORD_BYTES)
                .map_err(|_| ExecutorError::Contract("upload record count overflow"))?;
            match self.span_upload_mode {
                SpanUploadMode::PaddedPages => {
                    let mut padded = vec![0_u8; span.page_records as usize * RECORD_BYTES];
                    padded[..end - start].copy_from_slice(&bytes[start..end]);
                    self.write_texture_region(descriptor, &padded, UploadRegion::padded_page(side));
                }
                SpanUploadMode::ValidRows => {
                    for region in valid_row_regions(valid_records, side).into_iter().flatten() {
                        let region_start = start + region.source_offset_bytes;
                        let region_end = region_start + region.source_len_bytes;
                        self.write_texture_region(
                            descriptor,
                            &bytes[region_start..region_end],
                            region,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn write_texture_region(
        &self,
        descriptor: crate::Descriptor,
        bytes: &[u8],
        region: UploadRegion,
    ) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.data,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: u32::from(descriptor.x),
                    y: u32::from(descriptor.y) + region.origin_y,
                    z: u32::from(descriptor.layer),
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: region.bytes_per_row,
                rows_per_image: region.rows_per_image,
            },
            wgpu::Extent3d {
                width: region.width,
                height: region.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Builds immutable headers for exactly one active dense prefix.
    ///
    /// # Errors
    ///
    /// Returns `HeaderMismatch` for an invalid prefix or fixed-capacity overflow.
    pub fn prefix_headers(
        &self,
        span: &DataSpan,
        active_len: u32,
    ) -> Result<StaticHeaders, DispatchError> {
        let headers = StaticHeaders::for_prefix(span, active_len, self.header_stride)
            .map_err(|_| DispatchError::HeaderMismatch)?;
        if headers.offsets.len() > self.config.max_header_pages as usize {
            return Err(DispatchError::HeaderMismatch);
        }
        Ok(headers)
    }

    /// Uploads immutable dispatch-header sets into distinct fixed regions of the header buffer.
    ///
    /// A repeated reservation of byte-identical sets for the same live span returns the existing
    /// reservation without another upload. The reservation is reclaimed when its owning span is
    /// freed; later use of its handle returns a typed stale-handle error.
    ///
    /// # Errors
    ///
    /// Returns a typed empty-set, page-capacity, set-capacity, malformed-header, or stale-span
    /// failure before any partial upload.
    pub fn reserve_header_sets(
        &mut self,
        sets: &[StaticHeaders],
    ) -> Result<HeaderSetHandle, DispatchError> {
        self.validate_header_sets(sets)?;
        if let Some(handle) = self.header_reservations.find(&self.header_owner, sets) {
            return Ok(handle);
        }
        let handle = self.header_reservations.reserve(&self.header_owner, sets)?;
        let region_bytes = self.config.max_header_pages * self.header_stride;
        for (set, headers) in sets.iter().enumerate() {
            self.queue.write_buffer(
                &self.header_buffer,
                u64::from(handle.base_offset + set as u32 * region_bytes),
                &headers.bytes,
            );
        }
        Ok(handle)
    }

    /// Plans against caller-supplied immutable headers without GPU mutation.
    ///
    /// # Errors
    ///
    /// Returns the dialect's typed dispatch failure.
    pub fn plan_dispatch(
        &self,
        kernel: &GpuKernel,
        inputs: &[&DataSpan],
        outputs: &[&DataSpan],
        uniform: &[u8],
        headers: StaticHeaders,
    ) -> Result<ExecutorDispatch, DispatchError> {
        let plan =
            kernel
                .registered
                .plan_dispatch(&self.arena, inputs, outputs, uniform, &headers)?;
        Ok(make_dispatch(kernel, plan, headers))
    }

    /// Plans a dense output prefix while retaining the full allocation.
    ///
    /// # Errors
    ///
    /// Returns the dialect's typed dispatch failure.
    pub fn plan_prefix_dispatch(
        &self,
        kernel: &GpuKernel,
        inputs: &[&DataSpan],
        outputs: &[&DataSpan],
        active_len: u32,
        uniform: &[u8],
    ) -> Result<ExecutorDispatch, DispatchError> {
        let first = outputs.first().ok_or(DispatchError::OutputShapeMismatch)?;
        let headers = self.prefix_headers(first, active_len)?;
        let prefixes = outputs
            .iter()
            .map(|span| span.prefix(active_len))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| DispatchError::OutputShapeMismatch)?;
        self.plan_dispatch(
            kernel,
            inputs,
            &prefixes.iter().collect::<Vec<_>>(),
            uniform,
            headers,
        )
    }

    /// Compatibility wrapper that reserves one immutable header set and publishes resources.
    ///
    /// # Errors
    ///
    /// Returns a typed header-reservation, resource, or stale-span error.
    pub fn sync_dispatch(&mut self, dispatch: &ExecutorDispatch) -> Result<(), ExecutorError> {
        let headers = std::slice::from_ref(&dispatch.headers);
        let handle = self.reserve_header_sets(headers)?;
        self.compatibility_header = Some((dispatch.headers.clone(), handle));
        self.sync_dispatch_resources(dispatch);
        Ok(())
    }

    /// Publishes allocator metadata and resource-directory words without touching resident headers.
    pub fn sync_dispatch_resources(&self, dispatch: &ExecutorDispatch) {
        self.sync_heap_metadata();
        self.queue.write_buffer(
            &self.resources_buffer,
            0,
            bytemuck::cast_slice(&dispatch.plan.resource_words),
        );
    }

    /// Writes exactly one registered kernel uniform.
    ///
    /// # Errors
    ///
    /// Returns the dialect's uniform-size failure without writing.
    pub fn write_kernel_uniform(
        &self,
        kernel: &GpuKernel,
        uniform: &[u8],
    ) -> Result<(), ExecutorError> {
        if uniform.len() != kernel.uniform_size as usize {
            return Err(DispatchError::UniformSize {
                expected: kernel.uniform_size,
                actual: uniform.len(),
            }
            .into());
        }
        self.queue
            .write_buffer(&self.kernel_uniform_buffer, 0, uniform);
        Ok(())
    }

    /// Encodes the paid fragment pass and exact SCRATCH-to-DATA copies.
    ///
    /// # Errors
    ///
    /// Returns a typed kernel mismatch or stale destination failure.
    pub fn encode_dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        kernel: &GpuKernel,
        dispatch: &ExecutorDispatch,
    ) -> Result<(), ExecutorError> {
        if kernel.id != dispatch.kernel_id {
            return Err(ExecutorError::Contract("dispatch kernel mismatch"));
        }
        let (_, handle) = self
            .compatibility_header
            .as_ref()
            .filter(|(headers, _)| headers == &dispatch.headers)
            .ok_or(DispatchError::HeaderMismatch)?;
        for page in 0..dispatch.plan.passes.len() as u32 {
            self.encode_dispatch_selected(
                encoder,
                kernel,
                dispatch,
                handle,
                DispatchSelector { set: 0, page },
            )?;
        }
        Ok(())
    }

    /// Encodes one dispatch page using a selected immutable resident header.
    ///
    /// The caller can issue pages from any reserved dense-prefix set without a header upload; only
    /// the bind group's dynamic offset changes. Call `sync_dispatch_resources` when the dispatch's
    /// resource words differ from those most recently published.
    ///
    /// # Errors
    ///
    /// Returns typed kernel, foreign/stale handle, set/page selection, header mismatch, output, or
    /// destination failures.
    pub fn encode_dispatch_selected(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        kernel: &GpuKernel,
        dispatch: &ExecutorDispatch,
        header_sets: &HeaderSetHandle,
        selector: DispatchSelector,
    ) -> Result<(), ExecutorError> {
        if kernel.id != dispatch.kernel_id {
            return Err(ExecutorError::Contract("dispatch kernel mismatch"));
        }
        let (headers, absolute_offset) =
            self.header_reservations
                .resolve(&self.header_owner, header_sets, selector)?;
        if headers != &dispatch.headers {
            return Err(DispatchError::HeaderMismatch.into());
        }
        let page = dispatch.plan.passes.get(selector.page as usize).ok_or(
            DispatchError::HeaderPageSelection {
                set: selector.set,
                page: selector.page,
                page_count: dispatch.plan.passes.len() as u32,
            },
        )?;
        self.encode_page(encoder, kernel, dispatch.page_side, page, absolute_offset)
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) const fn arena(&self) -> &SpanArena {
        &self.arena
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) const fn data_texture(&self) -> &wgpu::Texture {
        &self.data
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) const fn bind_group(&self) -> &wgpu::BindGroup {
        &self.heap_group
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) const fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.heap_layout
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) const fn kernel_uniform_buffer(&self) -> &wgpu::Buffer {
        &self.kernel_uniform_buffer
    }

    pub(super) fn sync_heap_metadata(&self) {
        self.queue.write_buffer(
            &self.descriptor_buffer,
            0,
            bytemuck::cast_slice(&self.arena.heap().packed_table()),
        );
        self.queue.write_buffer(
            &self.directory_buffer,
            0,
            bytemuck::cast_slice(&self.arena.directory().packed_words()),
        );
    }

    fn validate_header_sets(&self, sets: &[StaticHeaders]) -> Result<(), DispatchError> {
        let first = sets.first().ok_or(DispatchError::EmptyHeaderSets)?;
        self.arena
            .validate_header_owner(first.owner())
            .map_err(|error| DispatchError::InvalidHandle(error.to_string()))?;
        for headers in sets {
            let owner_mismatch = headers.owner() != first.owner();
            let stride_mismatch = headers.stride != self.header_stride;
            let byte_length_mismatch =
                headers.bytes.len() != headers.offsets.len() * self.header_stride as usize;
            let offset_mismatch = headers
                .offsets
                .iter()
                .enumerate()
                .any(|(page, offset)| *offset != page as u32 * self.header_stride);
            if owner_mismatch || stride_mismatch || byte_length_mismatch || offset_mismatch {
                return Err(DispatchError::HeaderMismatch);
            }
            if headers.offsets.len() > self.config.max_header_pages as usize {
                return Err(DispatchError::HeaderPageCapacity {
                    actual: headers.offsets.len() as u32,
                    capacity: self.config.max_header_pages,
                });
            }
        }
        if sets.len() > self.config.max_header_sets as usize {
            return Err(self.header_reservations.capacity_error(sets.len() as u32));
        }
        Ok(())
    }

    fn encode_page(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        kernel: &GpuKernel,
        page_side: u16,
        page: &crate::PagePass,
        header_offset: u32,
    ) -> Result<(), ExecutorError> {
        if page.destinations.len() != kernel.outputs {
            return Err(DispatchError::OutputCount {
                expected: kernel.outputs,
                actual: page.destinations.len(),
            }
            .into());
        }
        let side = u32::from(page_side);
        let views = page
            .destinations
            .iter()
            .enumerate()
            .map(|(layer, _)| self.scratch_view(layer as u32))
            .collect::<Vec<_>>();
        let attachments = views
            .iter()
            .map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect::<Vec<_>>();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("heap dialect page into SCRATCH"),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&kernel.pipeline);
            pass.set_bind_group(0, &self.heap_group, &[header_offset]);
            pass.set_viewport(0.0, 0.0, side as f32, side as f32, 0.0, 1.0);
            pass.draw(0..3, 0..1);
        }
        for (layer, output) in page.destinations.iter().enumerate() {
            let descriptor = self
                .arena
                .heap()
                .resolve(*output)
                .map_err(SpanError::from)?;
            let full_rows = page.valid_length / side;
            let tail = page.valid_length % side;
            let source = |y| wgpu::TexelCopyTextureInfo {
                texture: &self.scratch,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y,
                    z: layer as u32,
                },
                aspect: wgpu::TextureAspect::All,
            };
            let destination = |y| wgpu::TexelCopyTextureInfo {
                texture: &self.data,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: u32::from(descriptor.x),
                    y: u32::from(descriptor.y) + y,
                    z: u32::from(descriptor.layer),
                },
                aspect: wgpu::TextureAspect::All,
            };
            if full_rows > 0 {
                encoder.copy_texture_to_texture(
                    source(0),
                    destination(0),
                    wgpu::Extent3d {
                        width: side,
                        height: full_rows,
                        depth_or_array_layers: 1,
                    },
                );
            }
            if tail > 0 {
                encoder.copy_texture_to_texture(
                    source(full_rows),
                    destination(full_rows),
                    wgpu::Extent3d {
                        width: tail,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        Ok(())
    }

    fn scratch_view(&self, layer: u32) -> wgpu::TextureView {
        self.scratch.create_view(&wgpu::TextureViewDescriptor {
            label: Some("heap executor one-layer SCRATCH attachment"),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        })
    }
}

fn make_dispatch(
    kernel: &GpuKernel,
    plan: DispatchPlan,
    headers: StaticHeaders,
) -> ExecutorDispatch {
    let side = u32::from(kernel.page_side);
    let copy_commands = copy_command_count(&plan, side);
    ExecutorDispatch {
        kernel_id: kernel.id,
        plan,
        headers,
        page_side: kernel.page_side,
        copy_commands,
    }
}

fn copy_command_count(plan: &DispatchPlan, side: u32) -> u32 {
    plan.passes
        .iter()
        .map(|pass| {
            let regions =
                u32::from(pass.valid_length / side > 0) + u32::from(pass.valid_length % side > 0);
            regions * pass.destinations.len() as u32
        })
        .sum()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UploadRegion {
    source_offset_bytes: usize,
    source_len_bytes: usize,
    origin_y: u32,
    width: u32,
    height: u32,
    bytes_per_row: Option<u32>,
    rows_per_image: Option<u32>,
}

impl UploadRegion {
    const fn padded_page(side: u32) -> Self {
        let records = side as usize * side as usize;
        Self {
            source_offset_bytes: 0,
            source_len_bytes: records * RECORD_BYTES,
            origin_y: 0,
            width: side,
            height: side,
            bytes_per_row: Some(side * RECORD_BYTES as u32),
            rows_per_image: Some(side),
        }
    }
}

fn valid_row_regions(valid_records: u32, side: u32) -> [Option<UploadRegion>; 2] {
    let full_rows = valid_records / side;
    let tail = valid_records % side;
    let row_bytes = side as usize * RECORD_BYTES;
    [
        (full_rows > 0).then_some(UploadRegion {
            source_offset_bytes: 0,
            source_len_bytes: full_rows as usize * row_bytes,
            origin_y: 0,
            width: side,
            height: full_rows,
            bytes_per_row: Some(side * RECORD_BYTES as u32),
            rows_per_image: Some(full_rows),
        }),
        (tail > 0).then_some(UploadRegion {
            source_offset_bytes: full_rows as usize * row_bytes,
            source_len_bytes: tail as usize * RECORD_BYTES,
            origin_y: full_rows,
            width: tail,
            height: 1,
            bytes_per_row: None,
            rows_per_image: None,
        }),
    ]
}

#[cfg(test)]
fn planned_upload_bytes(logical_len: u32, page_records: u32, mode: SpanUploadMode) -> u64 {
    match mode {
        SpanUploadMode::PaddedPages => {
            u64::from(logical_len.div_ceil(page_records) * page_records) * RECORD_BYTES as u64
        }
        SpanUploadMode::ValidRows => u64::from(logical_len) * RECORD_BYTES as u64,
    }
}

pub(super) fn texture(
    device: &wgpu::Device,
    label: &'static str,
    side: u32,
    layers: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn heap_layout(device: &wgpu::Device, config: GpuKernelExecutorConfig) -> wgpu::BindGroupLayout {
    let uniform = |binding, dynamic, size| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: NonZeroU64::new(size),
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("heap executor immutable group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            uniform(1, false, u64::from(config.descriptor_capacity) * 16),
            uniform(2, false, u64::from(config.directory_binding_bytes)),
            uniform(3, true, 16),
            uniform(4, false, 8 * 16),
            uniform(5, false, u64::from(config.kernel_uniform_bytes)),
        ],
    })
}

fn compute_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    source: &str,
    outputs: usize,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    let targets = vec![
        Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba32Float,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
        outputs
    ];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("heap_kernel_vertex"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("heap_kernel_fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{DispatchError, DispatchPlan, Handle, PagePass, SpanArena, StaticHeaders};

    use super::{
        DispatchSelector, HeaderReservations, SpanUploadMode, UploadRegion, copy_command_count,
        planned_upload_bytes, valid_row_regions,
    };

    #[test]
    fn row_exact_reference_layout_removes_sixteen_fold_upload_amplification() {
        assert_eq!(
            planned_upload_bytes(4_096, 65_536, SpanUploadMode::PaddedPages),
            1_048_576
        );
        assert_eq!(
            planned_upload_bytes(4_096, 65_536, SpanUploadMode::ValidRows),
            65_536
        );
        assert_eq!(
            valid_row_regions(4_096, 256),
            [
                Some(UploadRegion {
                    source_offset_bytes: 0,
                    source_len_bytes: 65_536,
                    origin_y: 0,
                    width: 256,
                    height: 16,
                    bytes_per_row: Some(4_096),
                    rows_per_image: Some(16),
                }),
                None,
            ]
        );
        assert_eq!(
            valid_row_regions(257, 256),
            [
                Some(UploadRegion {
                    source_offset_bytes: 0,
                    source_len_bytes: 4_096,
                    origin_y: 0,
                    width: 256,
                    height: 1,
                    bytes_per_row: Some(4_096),
                    rows_per_image: Some(1),
                }),
                Some(UploadRegion {
                    source_offset_bytes: 4_096,
                    source_len_bytes: 16,
                    origin_y: 1,
                    width: 1,
                    height: 1,
                    bytes_per_row: None,
                    rows_per_image: None,
                }),
            ]
        );
    }

    #[test]
    fn exact_row_copy_count_keeps_full_and_tail_rows_separate() {
        let plan = DispatchPlan {
            resource_words: [[0; 4]; 8],
            passes: vec![PagePass {
                header_offset: 0,
                global_base: 0,
                valid_length: 300,
                destinations: vec![
                    Handle::encode(1, 1).expect("valid handle"),
                    Handle::encode(2, 1).expect("valid handle"),
                ],
            }],
            logical_len: 300,
            gpu_copy_bytes: 9_600,
        };
        assert_eq!(copy_command_count(&plan, 256), 4);
    }

    #[test]
    fn three_header_sets_are_resident_selected_and_capacity_checked() {
        let mut arena = SpanArena::new(32, 1, 16, 512, 8).expect("arena");
        let span = arena.allocate_span(700, 16).expect("span");
        let sets = [200, 400, 700].map(|length| {
            StaticHeaders::for_prefix(&span, length, 256).expect("valid dense prefix")
        });
        let executor = Arc::new(());
        let mut reservations = HeaderReservations::new(3, 4, 256);
        let handle = reservations
            .reserve(&executor, &sets)
            .expect("three fixed regions fit");
        assert_eq!(handle.set_count(), 3);
        assert_eq!(handle.base_offset(), 0);
        assert_eq!(handle.stride(), 256);
        let offsets = [
            DispatchSelector { set: 0, page: 0 },
            DispatchSelector { set: 1, page: 1 },
            DispatchSelector { set: 2, page: 2 },
        ]
        .map(|selector| {
            reservations
                .resolve(&executor, &handle, selector)
                .expect("resident selection")
                .1
        });
        assert_eq!(offsets, [0, 1_280, 2_560]);
        assert_eq!(reservations.free_sets(), 0);
        assert!(matches!(
            reservations.reserve(&executor, &sets[..1]),
            Err(DispatchError::HeaderSetCapacity {
                requested_sets: 1,
                total_free_sets: 0,
                longest_free_run: 0,
            })
        ));
    }

    #[test]
    fn fragmented_header_capacity_reports_total_free_and_longest_run() {
        let mut arena = SpanArena::new(8, 1, 16, 512, 8).expect("arena");
        let spans = (0..6)
            .map(|_| arena.allocate_span(1, 1).expect("one record span"))
            .collect::<Vec<_>>();
        let headers = spans
            .iter()
            .map(|span| StaticHeaders::for_span(span, 256).expect("one page header"))
            .collect::<Vec<_>>();
        let executor = Arc::new(());
        let mut reservations = HeaderReservations::new(5, 1, 256);
        for header in &headers[..5] {
            reservations
                .reserve(&executor, std::slice::from_ref(header))
                .expect("one set region fits");
        }
        reservations.release_span(headers[1].owner());
        reservations.release_span(headers[3].owner());
        assert_eq!(reservations.free_sets(), 2);
        assert_eq!(reservations.longest_free_run(), 1);
        let error = reservations
            .reserve(&executor, &[headers[5].clone(), headers[5].clone()])
            .expect_err("two total holes do not form a two-set run");
        assert_eq!(
            error,
            DispatchError::HeaderSetCapacity {
                requested_sets: 2,
                total_free_sets: 2,
                longest_free_run: 1,
            }
        );
        assert_eq!(
            error.to_string(),
            "header buffer cannot reserve contiguous run of 2 sets; 2 total set regions are free and the longest free run is 1"
        );
    }

    #[test]
    fn header_set_handles_reject_foreign_and_stale_use() {
        let mut arena = SpanArena::new(16, 2, 8, 256, 4).expect("arena");
        let span = arena.allocate_span(300, 16).expect("span");
        let headers = StaticHeaders::for_span(&span, 256).expect("headers");
        let executor = Arc::new(());
        let foreign = Arc::new(());
        let mut reservations = HeaderReservations::new(2, 2, 256);
        let handle = reservations
            .reserve(&executor, std::slice::from_ref(&headers))
            .expect("one set fits");
        let selector = DispatchSelector { set: 0, page: 0 };
        assert_eq!(
            reservations.resolve(&foreign, &handle, selector),
            Err(DispatchError::ForeignHeaderSet)
        );
        reservations.release_span(headers.owner());
        assert_eq!(
            reservations.resolve(&executor, &handle, selector),
            Err(DispatchError::StaleHeaderSet)
        );
        assert_eq!(reservations.free_sets(), 2);
    }
}
