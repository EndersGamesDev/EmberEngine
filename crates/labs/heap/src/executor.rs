//! Public owner of the heap lattice's paid DATA/SCRATCH execution path.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use std::num::NonZeroU64;
use std::sync::Arc;

use thiserror::Error;
use wgpu::util::DeviceExt as _;

use crate::{
    DataSpan, DialectLimits, DispatchError, DispatchPlan, RegisteredKernel, SpanArena, SpanError,
    StaticHeaders,
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
    pub kernel_uniform_bytes: u32,
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
    pub kernel_uniform_bytes: u32,
    pub data_bytes: u64,
    pub scratch_bytes: u64,
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
    pub(super) fn set_resource(&mut self, slot: usize, span: &DataSpan) -> Result<(), ExecutorError> {
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
        let header_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap executor static dispatch headers"),
            size: u64::from(header_stride) * u64::from(config.max_header_pages),
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
            next_kernel_id: 1,
        })
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

    /// Frees one DATA span and publishes changed metadata.
    ///
    /// # Errors
    ///
    /// Returns the arena's typed stale-handle or directory failure.
    pub fn free_span(&mut self, span: DataSpan) -> Result<(), SpanError> {
        self.arena.free(span)?;
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
            let mut padded = vec![0_u8; span.page_records as usize * RECORD_BYTES];
            padded[..end - start].copy_from_slice(&bytes[start..end]);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.data,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: u32::from(descriptor.x),
                        y: u32::from(descriptor.y),
                        z: u32::from(descriptor.layer),
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &padded,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(side * 16),
                    rows_per_image: Some(side),
                },
                wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 1,
                },
            );
        }
        Ok(())
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
        let plan = kernel.registered.plan_dispatch(
            &self.arena,
            inputs,
            outputs,
            uniform,
            &headers,
        )?;
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
        let first = outputs
            .first()
            .ok_or(DispatchError::OutputShapeMismatch)?;
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

    /// Publishes current metadata, resources, and headers for a dispatch.
    ///
    /// # Errors
    ///
    /// Returns a capacity error for foreign header bytes.
    pub fn sync_dispatch(&self, dispatch: &ExecutorDispatch) -> Result<(), ExecutorError> {
        if dispatch.headers.offsets.len() > self.config.max_header_pages as usize {
            return Err(ExecutorError::Contract("header buffer capacity exceeded"));
        }
        self.sync_heap_metadata();
        self.queue.write_buffer(
            &self.resources_buffer,
            0,
            bytemuck::cast_slice(&dispatch.plan.resource_words),
        );
        self.queue
            .write_buffer(&self.header_buffer, 0, &dispatch.headers.bytes);
        Ok(())
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
        let side = u32::from(dispatch.page_side);
        for page in &dispatch.plan.passes {
            if page.destinations.len() != kernel.outputs {
                return Err(DispatchError::OutputCount {
                    expected: kernel.outputs,
                    actual: page.destinations.len(),
                }
                .into());
            }
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
                pass.set_bind_group(0, &self.heap_group, &[page.header_offset]);
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
        }
        Ok(())
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
            let regions = u32::from(pass.valid_length / side > 0)
                + u32::from(pass.valid_length % side > 0);
            regions * pass.destinations.len() as u32
        })
        .sum()
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

fn heap_layout(
    device: &wgpu::Device,
    config: GpuKernelExecutorConfig,
) -> wgpu::BindGroupLayout {
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
    use crate::{DispatchPlan, Handle, PagePass};

    use super::copy_command_count;

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
}
