//! WebGL2-only heap-lattice page runtime and mapped-fence measurement path.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::future_not_send,
    clippy::too_many_lines
)]

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bytemuck::Zeroable as _;
use ember_lab_layer::geometry::{
    Prism, lattice_copy_count, lattice_edge_count, lattice_steps, prism,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt as _;

use crate::completion::{MAX_COMPLETION_POLLS, PollCounter};
use crate::conformance::{
    IMAGE_BYTES, IMAGE_BYTES_PER_ROW, IMAGE_HEIGHT, IMAGE_WIDTH, ImageComparison,
    NumericComparison, RECORD_BYTES, RECORD_STRIDE, compare_images, compare_records,
    deterministic_indices,
};
use crate::selection::SelectionEpoch;
use crate::{
    BOX_INDICES, ComparatorWork, DataSpan, DialectLimits, DispatchPlan, EqualWorkSignature,
    FrameUniform, KernelDesc, ModeCFrameUniform, RegisteredKernel, SpanArena, StaticHeaders,
    box_vertices, frame_for, layer_comparator_draw_shader, layer_comparator_kernel, mode_a_records,
    mode_a_shader, mode_c_register, mode_c_shader,
};

const HEAP_SIDE: u16 = 512;
const HEAP_LAYERS: u16 = 16;
const DESCRIPTOR_CAPACITY: u32 = 64;
const SPAN_CAPACITY: u32 = 16;
const HANDLE_CAPACITY: u32 = 128;
const DIRECTORY_BYTES: u32 = SPAN_CAPACITY * 16 + HANDLE_CAPACITY * 4;
const MODE_A_PAGE: u16 = 64;
const MODE_C_PAGE: u16 = 256;
const MAX_HEADER_PAGES: u32 = 64;
const LAYER_BYTE_BUDGET: u64 = 64 * 1024 * 1024;
const DEFAULT_POLICY: u32 = 2_000_000;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const COMPLETION_DEADLINE_MS: f64 = 30_000.0;
const CONFORMANCE_TIME: f32 = 0.625;
const CONFORMANCE_IMAGE_STEP: u32 = 1;
const CONFORMANCE_IMAGE_POLICY: u32 = 3_000;

#[derive(Debug, thiserror::Error)]
enum LatticeError {
    #[error("WebGL2 capability refused: {0}")]
    Capability(String),
    #[error("heap lattice resource failure: {0}")]
    Resource(String),
    #[error("surface failure: {0}")]
    Surface(String),
    #[error("completion mapping failed: {0}")]
    Mapping(String),
    #[error("completion exceeded 30000 ms")]
    Deadline,
    #[error("completion exceeded the fixed {0}-poll limit")]
    PollLimit(u32),
    #[error("lattice generation {observed} is stale; current is {current}")]
    StaleGeneration { observed: u64, current: u64 },
    #[error("device lost: {0}")]
    DeviceLost(String),
    #[error("internal lattice state was already borrowed during {0}")]
    BorrowConflict(&'static str),
    #[error("unknown lattice mode {0}")]
    UnknownMode(String),
    #[error("lattice step {0} is outside 0..113")]
    UnknownStep(u32),
    #[error("repeat count must be in 1..=4096, got {0}")]
    InvalidRepeat(u32),
    #[error("could not serialize page facts: {0}")]
    Serialization(String),
    #[error("conformance failed: {0}")]
    Conformance(String),
}

impl From<LatticeError> for JsValue {
    fn from(error: LatticeError) -> Self {
        Self::from_str(&error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    A,
    C,
    Layer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectionIntent {
    mode: Mode,
    step: u32,
    policy: u32,
}

impl Mode {
    fn parse(value: &str) -> Result<Self, LatticeError> {
        match value {
            "mode-a" => Ok(Self::A),
            "mode-c" => Ok(Self::C),
            "layer" => Ok(Self::Layer),
            _ => Err(LatticeError::UnknownMode(value.to_string())),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::A => "Mode A · algebraic heap",
            Self::C => "Mode C · layer kernel over heap spans",
            Self::Layer => "layer · square slots",
        }
    }

    fn from_label(value: &str) -> Result<Self, LatticeError> {
        match value {
            "Mode A · algebraic heap" => Ok(Self::A),
            "Mode C · layer kernel over heap spans" => Ok(Self::C),
            "layer · square slots" => Ok(Self::Layer),
            _ => Err(LatticeError::UnknownMode(value.to_string())),
        }
    }
}

#[derive(Clone, Serialize)]
struct SelectionReport {
    generation: u64,
    mode: &'static str,
    requested_step: u32,
    requested_axes: [u32; 5],
    requested_copies: u64,
    requested_edges: u64,
    delivered_copies: u64,
    delivered_edges: u32,
    submitted_indices: u64,
    ideal_vertex_invocations: u64,
    compute_passes: u32,
    copy_commands: u32,
    gpu_copy_bytes: u64,
    per_frame_cpu_to_gpu_bytes: u32,
    logical_output_bytes: u64,
    reserved_output_bytes: u64,
    scratch_bytes: u64,
    layer_slot_bytes: u64,
    layer_allocation_bytes: u64,
    policy: u32,
    limiting_term: String,
    wall_arithmetic: String,
    equal_work_signature: Option<EqualWorkSignature>,
    timing_status: &'static str,
}

#[derive(Serialize)]
struct InitReport {
    adapter: String,
    backend: String,
    webgl_only: bool,
    heap_side: u16,
    heap_layers: u16,
    heap_bytes: u64,
    scratch_layers: u32,
    scratch_bytes: u64,
    descriptor_capacity: u32,
    span_capacity: u32,
    handle_capacity: u32,
    header_stride: u32,
    max_texture_dimension_2d: u32,
    max_texture_array_layers: u32,
    max_uniform_buffer_binding_size: u32,
    configured_layer_byte_budget: u64,
    timestamp_query_exposed: bool,
    timestamp_query_used: bool,
    completion_poll_limit: u32,
    output_path: &'static str,
    completion: &'static str,
    initial: SelectionReport,
}

#[derive(Serialize)]
struct FrameReport {
    generation: u64,
    mode: &'static str,
    delivered_edges: u32,
    per_frame_cpu_to_gpu_bytes: u32,
    compute_passes: u32,
    copy_commands: u32,
    gpu_copy_bytes: u64,
}

#[derive(Serialize)]
struct BatchReport {
    generation: u64,
    mode: &'static str,
    repeats: u32,
    elapsed_ms: f64,
    normalized_ms: f64,
    fence_polls: u32,
    fence_wait_ms: f64,
    microseconds_per_edge: Option<f64>,
    delivered_edges: u32,
    per_frame_cpu_to_gpu_bytes: u32,
    timing_method: &'static str,
    gpu_timestamp_ms: Option<f64>,
}

#[derive(Serialize)]
struct ConformanceReport {
    generation: u64,
    requested_step: u32,
    requested_axes: [u32; 5],
    policy: u32,
    mode_c_delivered_edges: u32,
    layer_delivered_edges: u32,
    counts_match: bool,
    signatures_match: bool,
    numeric: NumericComparison,
    image_step: u32,
    image_edges: u32,
    image: ImageComparison,
    status: &'static str,
    timing_qualified: bool,
}

struct PendingFence {
    buffer: wgpu::Buffer,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct FenceWait {
    polls: u32,
    waited_ms: f64,
}

struct PendingReadback {
    buffer: wgpu::Buffer,
    expected_bytes: usize,
}

struct HeapDispatch {
    plan: DispatchPlan,
    page_side: u16,
    mode: Mode,
}

struct LayerStep {
    compute_pipeline: wgpu::RenderPipeline,
    compute_group: wgpu::BindGroup,
    render_group: wgpu::BindGroup,
    midpoint: wgpu::Texture,
    orientation: wgpu::Texture,
    side: u32,
}

struct LatticeLab {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    lost: Arc<Mutex<Option<String>>>,
    object: Prism,
    arena: SpanArena,
    data: wgpu::Texture,
    scratch: wgpu::Texture,
    heap_group: wgpu::BindGroup,
    descriptor_buffer: wgpu::Buffer,
    directory_buffer: wgpu::Buffer,
    header_buffer: wgpu::Buffer,
    resources_buffer: wgpu::Buffer,
    frame_buffer: wgpu::Buffer,
    header_stride: u32,
    mode_a_kernel: RegisteredKernel,
    mode_c_kernel: RegisteredKernel,
    mode_a_compute: wgpu::RenderPipeline,
    mode_c_compute: wgpu::RenderPipeline,
    mode_a_draw: wgpu::RenderPipeline,
    mode_c_draw: wgpu::RenderPipeline,
    layer_compute_layout: wgpu::BindGroupLayout,
    layer_draw_pipeline: wgpu::RenderPipeline,
    layer_draw_layout: wgpu::BindGroupLayout,
    layer_edge: wgpu::Texture,
    layer_base_four: wgpu::Texture,
    layer_base_fifth: wgpu::Texture,
    box_vertices: wgpu::Buffer,
    box_indices: wgpu::Buffer,
    depth: wgpu::TextureView,
    fence_source: wgpu::Buffer,
    base_four: DataSpan,
    base_fifth: DataSpan,
    edge: DataSpan,
    mode_a_outputs: [DataSpan; 2],
    mode_c_outputs: Option<[DataSpan; 2]>,
    heap_dispatch: Option<HeapDispatch>,
    layer_step: Option<LayerStep>,
    active: SelectionReport,
    layer_max_side: u32,
}

fn serialize<T: Serialize>(value: &T) -> Result<String, LatticeError> {
    serde_json::to_string(value).map_err(|error| LatticeError::Serialization(error.to_string()))
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or_else(js_sys::Date::now, |performance| performance.now())
}

async fn yield_to_browser() -> Result<(), LatticeError> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let Some(window) = web_sys::window() else {
            drop(reject.call1(
                &JsValue::UNDEFINED,
                &JsValue::from_str("window is unavailable"),
            ));
            return;
        };
        if let Err(error) =
            window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
        {
            drop(reject.call1(&JsValue::UNDEFINED, &error));
        }
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|error| LatticeError::Mapping(format!("browser yield failed: {error:?}")))
}

fn texture(
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

fn rectangular_texture(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("heap lattice depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn heap_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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
        label: Some("heap lattice immutable group layout"),
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
            uniform(1, false, u64::from(DESCRIPTOR_CAPACITY) * 16),
            uniform(2, false, u64::from(DIRECTORY_BYTES)),
            uniform(3, true, 16),
            uniform(4, false, 8 * 16),
            uniform(5, false, 192),
        ],
    })
}

fn compute_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    source: &str,
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
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
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

const BOX_ATTRIBUTES: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
    format: wgpu::VertexFormat::Float32x3,
    offset: 0,
    shader_location: 0,
}];

fn draw_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    source: &str,
    vertex_entry: &'static str,
    fragment_entry: &'static str,
    surface_format: wgpu::TextureFormat,
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
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: size_of::<crate::BoxVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &BOX_ATTRIBUTES,
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn layer_compute_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("layer comparator compute layout"),
        entries: &[
            texture_entry(0),
            texture_entry(1),
            texture_entry(2),
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(192),
                },
                count: None,
            },
        ],
    })
}

fn layer_draw_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("layer comparator indexed draw layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(192),
                },
                count: None,
            },
        ],
    })
}

fn layer_compute_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &str,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("layer exact lattice kernel"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("layer comparator compute pipeline layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("layer comparator exact kernel pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("layer_compute_vertex"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("layer_compute_fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
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

fn square_side(records: usize) -> u32 {
    if records == 0 {
        1
    } else {
        u32::try_from(records.saturating_sub(1).isqrt().saturating_add(1)).unwrap_or(u32::MAX)
    }
}

fn upload_square(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    records: &[[f32; 4]],
) -> wgpu::Texture {
    let side = square_side(records.len());
    let texture = texture(
        device,
        label,
        side,
        1,
        wgpu::TextureFormat::Rgba32Float,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let mut padded = vec![[0.0_f32; 4]; side as usize * side as usize];
    padded[..records.len()].copy_from_slice(records);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&padded),
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
    texture
}

fn upload_span(
    queue: &wgpu::Queue,
    data: &wgpu::Texture,
    arena: &SpanArena,
    span: &DataSpan,
    records: &[[f32; 4]],
) -> Result<(), LatticeError> {
    let side = span.page_records.isqrt();
    for (page, handle) in span.handles().iter().enumerate() {
        let descriptor = arena
            .heap()
            .resolve(*handle)
            .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let start = page * span.page_records as usize;
        let end = records.len().min(start + span.page_records as usize);
        let mut padded = vec![[0.0_f32; 4]; span.page_records as usize];
        padded[..end - start].copy_from_slice(&records[start..end]);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: data,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: u32::from(descriptor.x),
                    y: u32::from(descriptor.y),
                    z: u32::from(descriptor.layer),
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&padded),
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

impl LatticeLab {
    async fn new(
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(Self, String, String), LatticeError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| {
                LatticeError::Capability(format!("surface creation failed: {error}"))
            })?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| LatticeError::Capability("no WebGL2 adapter".to_string()))?;
        let info = adapter.get_info();
        if info.backend != wgpu::Backend::Gl {
            return Err(LatticeError::Capability(format!(
                "requested GL/WebGL2 but selected {:?}",
                info.backend
            )));
        }
        let adapter_limits = adapter.limits();
        if adapter_limits.max_texture_dimension_2d < u32::from(HEAP_SIDE)
            || adapter_limits.max_texture_array_layers < u32::from(HEAP_LAYERS)
            || adapter_limits.max_uniform_buffer_binding_size < u64::from(DIRECTORY_BYTES) as u32
        {
            return Err(LatticeError::Capability(format!(
                "limits dimension={} layers={} uniform={} cannot create heap side={} layers={} directory={}B",
                adapter_limits.max_texture_dimension_2d,
                adapter_limits.max_texture_array_layers,
                adapter_limits.max_uniform_buffer_binding_size,
                HEAP_SIDE,
                HEAP_LAYERS,
                DIRECTORY_BYTES
            )));
        }
        let required_usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST;
        let rgba = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float);
        if !rgba.allowed_usages.contains(required_usage) {
            return Err(LatticeError::Capability(format!(
                "RGBA32Float usages {:?} omit {:?}",
                rgba.allowed_usages, required_usage
            )));
        }
        let mut required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter_limits.clone());
        required_limits.max_texture_array_layers = u32::from(HEAP_LAYERS);
        required_limits.max_uniform_buffer_binding_size = required_limits
            .max_uniform_buffer_binding_size
            .max(DIRECTORY_BYTES);
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("heap lattice GL device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|error| LatticeError::Capability(format!("device request failed: {error}")))?;
        let lost = Arc::new(Mutex::new(None));
        let lost_callback = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = lost_callback.lock() {
                *slot = Some(format!("{reason:?}: {message}"));
            }
        });
        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| LatticeError::Capability("surface exposes no format".to_string()))?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            .or_else(|| capabilities.present_modes.first().copied())
            .ok_or_else(|| {
                LatticeError::Capability("surface exposes no present mode".to_string())
            })?;
        let alpha_mode =
            capabilities.alpha_modes.first().copied().ok_or_else(|| {
                LatticeError::Capability("surface exposes no alpha mode".to_string())
            })?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: 960,
            height: 540,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
        let object = prism();
        let records = mode_a_records(&object);
        let mut arena = SpanArena::new(
            HEAP_SIDE,
            HEAP_LAYERS,
            DESCRIPTOR_CAPACITY,
            DIRECTORY_BYTES,
            SPAN_CAPACITY,
        )
        .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let base_four = arena
            .allocate_span(1_200, MODE_A_PAGE)
            .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let base_fifth = arena
            .allocate_span(1_200, MODE_A_PAGE)
            .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let edge = arena
            .allocate_span(3_000, MODE_A_PAGE)
            .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let mode_a_outputs = arena
            .allocate_pair(1_200, MODE_A_PAGE)
            .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let data = texture(
            &device,
            "heap lattice DATA",
            u32::from(HEAP_SIDE),
            u32::from(HEAP_LAYERS),
            wgpu::TextureFormat::Rgba32Float,
            required_usage,
        );
        let scratch = texture(
            &device,
            "heap lattice SCRATCH",
            u32::from(HEAP_SIDE),
            4,
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        upload_span(&queue, &data, &arena, &base_four, &records.base_four)?;
        upload_span(&queue, &data, &arena, &base_fifth, &records.base_fifth)?;
        upload_span(&queue, &data, &arena, &edge, &records.edges)?;
        let descriptor_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lattice descriptor UBO"),
            contents: bytemuck::cast_slice(&arena.heap().packed_table()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let directory_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lattice span directory UBO"),
            contents: bytemuck::cast_slice(&arena.directory().packed_words()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let header_stride = device.limits().min_uniform_buffer_offset_alignment.max(16);
        let header_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap lattice static dispatch headers"),
            size: u64::from(header_stride) * u64::from(MAX_HEADER_PAGES),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let resources_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lattice step resources"),
            contents: bytemuck::bytes_of(&[[0_u32; 4]; 8]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let frame_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lattice 192-byte frame uniform"),
            contents: bytemuck::bytes_of(&FrameUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = heap_layout(&device);
        let data_view = data.create_view(&wgpu::TextureViewDescriptor {
            label: Some("heap lattice full DATA array view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(u32::from(HEAP_LAYERS)),
            ..Default::default()
        });
        let heap_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heap lattice one immutable bind group"),
            layout: &layout,
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
                    resource: frame_buffer.as_entire_binding(),
                },
            ],
        });
        let limits = DialectLimits {
            descriptor_capacity: DESCRIPTOR_CAPACITY,
            span_capacity: SPAN_CAPACITY,
            handle_capacity: HANDLE_CAPACITY,
        };
        let mode_a_kernel = RegisteredKernel::register(
            &KernelDesc {
                name: "mode_a_rotation",
                body: crate::MODE_A_ROTATION_KERNEL,
                accessors: &["base_four", "base_fifth"],
                output_fields: &["rotated_four", "rotated_fifth"],
                uniform_type: "FrameUniform",
                uniform_size: 192,
                output_page_side: MODE_A_PAGE,
            },
            limits,
        )
        .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let mode_c_kernel = mode_c_register(MODE_C_PAGE, limits)
            .map_err(|error| LatticeError::Resource(error.to_string()))?;
        let mode_a_compute = compute_pipeline(
            &device,
            &layout,
            "heap lattice Mode A rotation pipeline",
            mode_a_kernel.source(),
        );
        let mode_c_compute = compute_pipeline(
            &device,
            &layout,
            "heap lattice Mode C exact layer kernel pipeline",
            mode_c_kernel.source(),
        );
        let mode_a_draw = draw_pipeline(
            &device,
            &layout,
            "heap lattice Mode A indexed draw",
            &mode_a_shader(limits),
            "mode_a_vertex",
            "mode_a_fragment",
            surface_format,
        );
        let mode_c_draw = draw_pipeline(
            &device,
            &layout,
            "heap lattice Mode C indexed draw",
            &mode_c_shader(limits),
            "mode_c_vertex",
            "mode_c_fragment",
            surface_format,
        );
        let layer_compute_layout = layer_compute_layout(&device);
        let layer_draw_layout = layer_draw_layout(&device);
        let layer_draw_pipeline = draw_pipeline(
            &device,
            &layer_draw_layout,
            "layer comparator indexed draw",
            layer_comparator_draw_shader(),
            "layer_vertex",
            "layer_fragment",
            surface_format,
        );
        let layer_edge = upload_square(&device, &queue, "layer static edge slot", &records.edges);
        let layer_base_four = upload_square(
            &device,
            &queue,
            "layer static base-four slot",
            &records.base_four,
        );
        let layer_base_fifth = upload_square(
            &device,
            &queue,
            "layer static base-fifth slot",
            &records.base_fifth,
        );
        let box_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lattice eight unique box vertices"),
            contents: bytemuck::cast_slice(&box_vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let box_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lattice 36 box indices"),
            contents: bytemuck::cast_slice(&BOX_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let depth = depth_view(&device, config.width, config.height);
        let fence_source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lattice four-byte fence source"),
            contents: &[0x68, 0x6c, 0x61, 0x74],
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        let layer_max_by_budget = (LAYER_BYTE_BUDGET / 32).isqrt();
        let layer_max_side = adapter_limits
            .max_texture_dimension_2d
            .min(u32::try_from(layer_max_by_budget).unwrap_or(u32::MAX));
        let placeholder = SelectionReport {
            generation: 0,
            mode: Mode::A.label(),
            requested_step: 0,
            requested_axes: [0; 5],
            requested_copies: 0,
            requested_edges: 0,
            delivered_copies: 0,
            delivered_edges: 0,
            submitted_indices: 0,
            ideal_vertex_invocations: 0,
            compute_passes: 0,
            copy_commands: 0,
            gpu_copy_bytes: 0,
            per_frame_cpu_to_gpu_bytes: 192,
            logical_output_bytes: 0,
            reserved_output_bytes: 0,
            scratch_bytes: u64::from(HEAP_SIDE) * u64::from(HEAP_SIDE) * 4 * 16,
            layer_slot_bytes: 0,
            layer_allocation_bytes: 0,
            policy: DEFAULT_POLICY,
            limiting_term: "requested".to_string(),
            wall_arithmetic: "initialization pending".to_string(),
            equal_work_signature: None,
            timing_status: "requires visible replay",
        };
        Ok((
            Self {
                surface,
                config,
                device,
                queue,
                lost,
                object,
                arena,
                data,
                scratch,
                heap_group,
                descriptor_buffer,
                directory_buffer,
                header_buffer,
                resources_buffer,
                frame_buffer,
                header_stride,
                mode_a_kernel,
                mode_c_kernel,
                mode_a_compute,
                mode_c_compute,
                mode_a_draw,
                mode_c_draw,
                layer_compute_layout,
                layer_draw_pipeline,
                layer_draw_layout,
                layer_edge,
                layer_base_four,
                layer_base_fifth,
                box_vertices,
                box_indices,
                depth,
                fence_source,
                base_four,
                base_fifth,
                edge,
                mode_a_outputs,
                mode_c_outputs: None,
                heap_dispatch: None,
                layer_step: None,
                active: placeholder,
                layer_max_side,
            },
            info.name,
            format!("{:?}", info.backend),
        ))
    }

    fn sync_heap_metadata(&self) {
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

    fn sync_dispatch(&self, plan: &DispatchPlan, headers: &StaticHeaders) {
        self.queue.write_buffer(
            &self.resources_buffer,
            0,
            bytemuck::cast_slice(&plan.resource_words),
        );
        self.queue
            .write_buffer(&self.header_buffer, 0, &headers.bytes);
    }

    fn copy_command_count(plan: &DispatchPlan, page_side: u16) -> u32 {
        let side = u32::from(page_side);
        plan.passes
            .iter()
            .map(|pass| {
                let regions = u32::from(pass.valid_length / side > 0)
                    + u32::from(pass.valid_length % side > 0);
                regions * pass.destinations.len() as u32
            })
            .sum()
    }

    fn select(
        &mut self,
        mode: Mode,
        step: u32,
        policy: u32,
        generation: u64,
    ) -> Result<SelectionReport, LatticeError> {
        let steps = lattice_steps();
        let axes = *steps
            .get(step as usize)
            .ok_or(LatticeError::UnknownStep(step))?;
        let requested_copies = lattice_copy_count(axes);
        let requested_edges = lattice_edge_count(axes);
        let policy = policy.max(1);
        self.heap_dispatch = None;
        self.layer_step = None;
        if let Some(outputs) = self.mode_c_outputs.take() {
            for output in outputs {
                self.arena
                    .free(output)
                    .map_err(|error| LatticeError::Resource(error.to_string()))?;
            }
        }
        let scratch_bytes = u64::from(HEAP_SIDE) * u64::from(HEAP_SIDE) * 4 * 16;
        let report = match mode {
            Mode::A => {
                let delivered_edges_u64 = requested_edges
                    .min(u64::from(u32::MAX))
                    .min(u64::from(policy));
                let delivered_edges = delivered_edges_u64 as u32;
                let frame = frame_for(
                    &self.object,
                    axes,
                    0.0,
                    self.config.width as f32 / self.config.height as f32,
                );
                let headers = StaticHeaders::for_span(&self.mode_a_outputs[0], self.header_stride)
                    .map_err(|error| LatticeError::Resource(error.to_string()))?;
                let mut plan = self
                    .mode_a_kernel
                    .plan_dispatch(
                        &self.arena,
                        &[&self.base_four, &self.base_fifth],
                        &[&self.mode_a_outputs[0], &self.mode_a_outputs[1]],
                        bytemuck::bytes_of(&frame),
                        &headers,
                    )
                    .map_err(|error| LatticeError::Resource(error.to_string()))?;
                plan.resource_words[2] = [self.edge.directory_index, self.edge.logical_len, 0, 0];
                plan.resource_words[3] = [
                    self.mode_a_outputs[0].directory_index,
                    self.mode_a_outputs[0].logical_len,
                    0,
                    0,
                ];
                plan.resource_words[4] = [
                    self.mode_a_outputs[1].directory_index,
                    self.mode_a_outputs[1].logical_len,
                    0,
                    0,
                ];
                self.sync_heap_metadata();
                self.sync_dispatch(&plan, &headers);
                let copy_commands = Self::copy_command_count(&plan, MODE_A_PAGE);
                self.heap_dispatch = Some(HeapDispatch {
                    plan,
                    page_side: MODE_A_PAGE,
                    mode,
                });
                let limiting_term = if requested_edges <= u64::from(u32::MAX)
                    && requested_edges <= u64::from(policy)
                {
                    "requested"
                } else if u64::from(policy) <= u64::from(u32::MAX) {
                    "overridable GLsizei policy"
                } else {
                    "u32 instance WALL"
                };
                SelectionReport {
                    generation,
                    mode: mode.label(),
                    requested_step: step,
                    requested_axes: axes,
                    requested_copies,
                    requested_edges,
                    delivered_copies: delivered_edges_u64 / 3_000,
                    delivered_edges,
                    submitted_indices: delivered_edges_u64 * 36,
                    ideal_vertex_invocations: delivered_edges_u64 * 8,
                    compute_passes: 1,
                    copy_commands,
                    gpu_copy_bytes: 38_400,
                    per_frame_cpu_to_gpu_bytes: 192,
                    logical_output_bytes: 38_400,
                    reserved_output_bytes: self
                        .mode_a_outputs
                        .iter()
                        .map(|span| span.reserved_records() * 16)
                        .sum(),
                    scratch_bytes,
                    layer_slot_bytes: 0,
                    layer_allocation_bytes: 0,
                    policy,
                    limiting_term: limiting_term.to_string(),
                    wall_arithmetic: format!(
                        "min(requested {requested_edges}, u32 {}, POLICY {policy}) = {delivered_edges_u64} submitted edges",
                        u32::MAX
                    ),
                    equal_work_signature: None,
                    timing_status: "requires visible replay",
                }
            }
            Mode::C => {
                let address_copies = u64::from(u32::MAX) / 3_000;
                let draw_copies = u64::from(policy.min(u32::MAX)) / 3_000;
                let heap_copies = self.arena.plan_paired_copies(
                    requested_copies.min(address_copies).min(draw_copies),
                    3_000,
                    MODE_C_PAGE,
                );
                let delivered_copies = requested_copies
                    .min(address_copies)
                    .min(draw_copies)
                    .min(heap_copies);
                let delivered_edges_u64 = delivered_copies * 3_000;
                let delivered_edges = delivered_edges_u64 as u32;
                let mut compute_passes = 0;
                let mut copy_commands = 0;
                let mut gpu_copy_bytes = 0;
                let mut reserved_output_bytes = 0;
                if delivered_edges > 0 {
                    let outputs = self
                        .arena
                        .allocate_pair(delivered_edges, MODE_C_PAGE)
                        .map_err(|error| LatticeError::Resource(error.to_string()))?;
                    let frame = frame_for(
                        &self.object,
                        axes,
                        0.0,
                        self.config.width as f32 / self.config.height as f32,
                    );
                    let uniform = ModeCFrameUniform::from_frame(&frame);
                    let headers = StaticHeaders::for_span(&outputs[0], self.header_stride)
                        .map_err(|error| LatticeError::Resource(error.to_string()))?;
                    let mut plan = self
                        .mode_c_kernel
                        .plan_dispatch(
                            &self.arena,
                            &[&self.edge, &self.base_four, &self.base_fifth],
                            &[&outputs[0], &outputs[1]],
                            bytemuck::bytes_of(&uniform),
                            &headers,
                        )
                        .map_err(|error| LatticeError::Resource(error.to_string()))?;
                    plan.resource_words[3] =
                        [outputs[0].directory_index, outputs[0].logical_len, 0, 0];
                    plan.resource_words[4] =
                        [outputs[1].directory_index, outputs[1].logical_len, 0, 0];
                    compute_passes = plan.passes.len() as u32;
                    copy_commands = Self::copy_command_count(&plan, MODE_C_PAGE);
                    gpu_copy_bytes = plan.gpu_copy_bytes;
                    reserved_output_bytes = outputs
                        .iter()
                        .map(|span| span.reserved_records() * 16)
                        .sum();
                    self.sync_heap_metadata();
                    self.sync_dispatch(&plan, &headers);
                    self.heap_dispatch = Some(HeapDispatch {
                        plan,
                        page_side: MODE_C_PAGE,
                        mode,
                    });
                    self.mode_c_outputs = Some(outputs);
                } else {
                    self.sync_heap_metadata();
                }
                let limiting_term = [
                    ("requested", requested_copies),
                    ("paired heap and directory WALL", heap_copies),
                    ("u32 kernel WALL", address_copies),
                    ("overridable GLsizei policy", draw_copies),
                ]
                .into_iter()
                .min_by_key(|(_, copies)| *copies)
                .map_or("requested", |(name, _)| name);
                SelectionReport {
                    generation,
                    mode: mode.label(),
                    requested_step: step,
                    requested_axes: axes,
                    requested_copies,
                    requested_edges,
                    delivered_copies,
                    delivered_edges,
                    submitted_indices: delivered_edges_u64 * 36,
                    ideal_vertex_invocations: delivered_edges_u64 * 8,
                    compute_passes,
                    copy_commands,
                    gpu_copy_bytes,
                    per_frame_cpu_to_gpu_bytes: 192,
                    logical_output_bytes: delivered_edges_u64 * 32,
                    reserved_output_bytes,
                    scratch_bytes,
                    layer_slot_bytes: ComparatorWork::for_axes(axes).layer_slot_bytes,
                    layer_allocation_bytes: ComparatorWork::for_axes(axes).layer_allocation_bytes,
                    policy,
                    limiting_term: limiting_term.to_string(),
                    wall_arithmetic: format!(
                        "whole copies min(requested {requested_copies}, paired heap {heap_copies}, u32 {address_copies}, POLICY {draw_copies}) = {delivered_copies}; edges = copies * 3000"
                    ),
                    equal_work_signature: Some(EqualWorkSignature::for_work(
                        &self.object,
                        axes,
                        0.0,
                    )),
                    timing_status: "requires visible replay",
                }
            }
            Mode::Layer => {
                let max_edges = u64::from(self.layer_max_side) * u64::from(self.layer_max_side);
                let wall_copies = max_edges / 3_000;
                let address_copies = u64::from(u32::MAX) / 3_000;
                let policy_copies = u64::from(policy) / 3_000;
                let delivered_copies = requested_copies
                    .min(wall_copies)
                    .min(address_copies)
                    .min(policy_copies);
                let delivered_edges_u64 = delivered_copies * 3_000;
                let delivered_edges = delivered_edges_u64 as u32;
                if delivered_edges > 0 {
                    self.layer_step = Some(self.create_layer_step(delivered_edges)?);
                }
                let delivered_work = ComparatorWork::for_axes([
                    u32::try_from(delivered_copies).unwrap_or(u32::MAX),
                    1,
                    1,
                    1,
                    1,
                ]);
                let side = if delivered_edges == 0 {
                    0
                } else {
                    delivered_edges.saturating_sub(1).isqrt().saturating_add(1)
                };
                let slot_bytes = u64::from(side) * u64::from(side) * 16;
                let limiting_term = [
                    ("requested", requested_copies),
                    ("layer slot dimension/budget WALL", wall_copies),
                    ("u32 kernel WALL", address_copies),
                    ("overridable GLsizei policy", policy_copies),
                ]
                .into_iter()
                .min_by_key(|(_, copies)| *copies)
                .map_or("requested", |(name, _)| name);
                SelectionReport {
                    generation,
                    mode: mode.label(),
                    requested_step: step,
                    requested_axes: axes,
                    requested_copies,
                    requested_edges,
                    delivered_copies,
                    delivered_edges,
                    submitted_indices: delivered_edges_u64 * 36,
                    ideal_vertex_invocations: delivered_edges_u64 * 8,
                    compute_passes: u32::from(delivered_edges > 0),
                    copy_commands: 0,
                    gpu_copy_bytes: 0,
                    per_frame_cpu_to_gpu_bytes: 192,
                    logical_output_bytes: delivered_edges_u64 * 32,
                    reserved_output_bytes: delivered_work.logical_bytes,
                    scratch_bytes: 0,
                    layer_slot_bytes: slot_bytes,
                    layer_allocation_bytes: slot_bytes * 2,
                    policy,
                    limiting_term: limiting_term.to_string(),
                    wall_arithmetic: format!(
                        "whole copies min(requested {requested_copies}, layer square-slot {wall_copies}, u32 {address_copies}, POLICY {policy_copies}) = {delivered_copies}; live max side {} from min(device dimension, configured 64 MiB two-slot budget)",
                        self.layer_max_side
                    ),
                    equal_work_signature: Some(EqualWorkSignature::for_work(
                        &self.object,
                        axes,
                        0.0,
                    )),
                    timing_status: "requires visible replay",
                }
            }
        };
        self.active = report.clone();
        Ok(report)
    }

    fn create_layer_step(&self, edges: u32) -> Result<LayerStep, LatticeError> {
        let side = edges.saturating_sub(1).isqrt().saturating_add(1);
        if side > self.layer_max_side {
            return Err(LatticeError::Resource(format!(
                "layer output side {side} exceeds runtime wall {}",
                self.layer_max_side
            )));
        }
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        let midpoint = texture(
            &self.device,
            "layer midpoint-hue output slot",
            side,
            1,
            wgpu::TextureFormat::Rgba32Float,
            usage,
        );
        let orientation = texture(
            &self.device,
            "layer orientation-length output slot",
            side,
            1,
            wgpu::TextureFormat::Rgba32Float,
            usage,
        );
        let edge_view = self
            .layer_edge
            .create_view(&wgpu::TextureViewDescriptor::default());
        let base_four_view = self
            .layer_base_four
            .create_view(&wgpu::TextureViewDescriptor::default());
        let base_fifth_view = self
            .layer_base_fifth
            .create_view(&wgpu::TextureViewDescriptor::default());
        let compute_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("layer comparator frozen step bind group"),
            layout: &self.layer_compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&edge_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&base_four_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&base_fifth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.frame_buffer.as_entire_binding(),
                },
            ],
        });
        let midpoint_view = midpoint.create_view(&wgpu::TextureViewDescriptor::default());
        let orientation_view = orientation.create_view(&wgpu::TextureViewDescriptor::default());
        let render_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("layer comparator indexed presentation group"),
            layout: &self.layer_draw_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&midpoint_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&orientation_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frame_buffer.as_entire_binding(),
                },
            ],
        });
        let source = layer_comparator_kernel(side, edges);
        let compute_pipeline =
            layer_compute_pipeline(&self.device, &self.layer_compute_layout, &source);
        Ok(LayerStep {
            compute_pipeline,
            compute_group,
            render_group,
            midpoint,
            orientation,
            side,
        })
    }

    fn scratch_view(&self, layer: u32) -> wgpu::TextureView {
        self.scratch.create_view(&wgpu::TextureViewDescriptor {
            label: Some("heap lattice one-layer SCRATCH attachment"),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        })
    }

    fn encode_heap_compute(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        dispatch: &HeapDispatch,
    ) -> Result<(), LatticeError> {
        let pipeline = if dispatch.mode == Mode::A {
            &self.mode_a_compute
        } else {
            &self.mode_c_compute
        };
        let side = u32::from(dispatch.page_side);
        for page in &dispatch.plan.passes {
            let views: Vec<_> = page
                .destinations
                .iter()
                .enumerate()
                .map(|(layer, _)| self.scratch_view(layer as u32))
                .collect();
            let attachments: Vec<_> = views
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
                .collect();
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("heap dialect page into SCRATCH"),
                    color_attachments: &attachments,
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.heap_group, &[page.header_offset]);
                pass.set_viewport(0.0, 0.0, side as f32, side as f32, 0.0, 1.0);
                pass.draw(0..3, 0..1);
            }
            for (scratch_layer, destination) in page.destinations.iter().enumerate() {
                let descriptor = self
                    .arena
                    .heap()
                    .resolve(*destination)
                    .map_err(|error| LatticeError::Resource(error.to_string()))?;
                let full_rows = page.valid_length / side;
                let tail = page.valid_length % side;
                if full_rows > 0 {
                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.scratch,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: scratch_layer as u32,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
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
                        wgpu::Extent3d {
                            width: side,
                            height: full_rows,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                if tail > 0 {
                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.scratch,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: full_rows,
                                z: scratch_layer as u32,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.data,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: u32::from(descriptor.x),
                                y: u32::from(descriptor.y) + full_rows,
                                z: u32::from(descriptor.layer),
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
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

    fn encode_layer_compute(&self, encoder: &mut wgpu::CommandEncoder, step: &LayerStep) {
        let midpoint = step
            .midpoint
            .create_view(&wgpu::TextureViewDescriptor::default());
        let orientation = step
            .orientation
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layer comparator exact fragment-compute pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &midpoint,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &orientation,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&step.compute_pipeline);
        pass.set_bind_group(0, &step.compute_group, &[]);
        pass.set_viewport(0.0, 0.0, step.side as f32, step.side as f32, 0.0, 1.0);
        pass.draw(0..3, 0..1);
    }

    fn frame_uniform(&self, time: f32) -> [u8; 192] {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let frame = frame_for(&self.object, self.active.requested_axes, time, aspect);
        if self.active.mode == Mode::A.label() {
            *bytemuck::from_bytes(bytemuck::bytes_of(&frame))
        } else {
            let uniform = ModeCFrameUniform::from_frame(&frame);
            *bytemuck::from_bytes(bytemuck::bytes_of(&uniform))
        }
    }

    fn encode_presentation(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        label: &'static str,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.008,
                        g: 0.012,
                        b: 0.025,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_vertex_buffer(0, self.box_vertices.slice(..));
        pass.set_index_buffer(self.box_indices.slice(..), wgpu::IndexFormat::Uint16);
        match self.heap_dispatch.as_ref().map(|dispatch| dispatch.mode) {
            Some(Mode::A) => {
                pass.set_pipeline(&self.mode_a_draw);
                pass.set_bind_group(0, &self.heap_group, &[0]);
            }
            Some(Mode::C) => {
                pass.set_pipeline(&self.mode_c_draw);
                pass.set_bind_group(0, &self.heap_group, &[0]);
            }
            _ if self.layer_step.is_some() => {
                let step = self.layer_step.as_ref().expect("checked layer step");
                pass.set_pipeline(&self.layer_draw_pipeline);
                pass.set_bind_group(0, &step.render_group, &[]);
            }
            _ => {}
        }
        if self.heap_dispatch.is_some() || self.layer_step.is_some() {
            pass.draw_indexed(0..36, 0, 0..self.active.delivered_edges);
        }
    }

    fn acquire_frame(&mut self) -> Result<wgpu::SurfaceTexture, LatticeError> {
        if let Some(reason) = self.lost.lock().ok().and_then(|slot| slot.clone()) {
            return Err(LatticeError::DeviceLost(reason));
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                self.surface
                    .get_current_texture()
                    .map_err(|error| LatticeError::Surface(error.to_string()))?
            }
            Err(error) => return Err(LatticeError::Surface(error.to_string())),
        };
        Ok(frame)
    }

    fn submit_frame_to_view(
        &mut self,
        time: f32,
        view: &wgpu::TextureView,
    ) -> Result<(), LatticeError> {
        let uniform = self.frame_uniform(time);
        self.queue.write_buffer(&self.frame_buffer, 0, &uniform);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("heap lattice frame"),
            });
        if let Some(dispatch) = &self.heap_dispatch {
            self.encode_heap_compute(&mut encoder, dispatch)?;
        } else if let Some(step) = &self.layer_step {
            self.encode_layer_compute(&mut encoder, step);
        }
        self.encode_presentation(
            &mut encoder,
            &view,
            &self.depth,
            "heap lattice one presentation pass",
        );
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn render_frame(&mut self, time: f32) -> Result<(), LatticeError> {
        let frame = self.acquire_frame()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.submit_frame_to_view(time, &view)?;
        frame.present();
        Ok(())
    }

    fn submit_measured_batch(
        &mut self,
        time: f32,
        repeats: u32,
    ) -> Result<(PendingFence, wgpu::SurfaceTexture), LatticeError> {
        let frame = self.acquire_frame()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        for repeat in 0..repeats {
            self.submit_frame_to_view(time + repeat as f32 * 0.000_001, &view)?;
        }
        let fence = self.pending_fence();
        Ok((fence, frame))
    }

    fn record_readback_buffer(&self) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap lattice conformance record readback"),
            size: RECORD_BYTES as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn copy_record(
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        origin: wgpu::Origin3d,
        buffer: &wgpu::Buffer,
        slot: usize,
    ) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: (slot * RECORD_STRIDE) as u64,
                    bytes_per_row: Some(RECORD_STRIDE as u32),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    fn capture_records(
        &self,
        mode: Mode,
        indices: &[u32],
    ) -> Result<PendingReadback, LatticeError> {
        let buffer = self.record_readback_buffer();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("heap lattice conformance record copies"),
            });
        match mode {
            Mode::C => {
                let outputs = self.mode_c_outputs.as_ref().ok_or_else(|| {
                    LatticeError::Conformance("Mode C has no live output spans".to_string())
                })?;
                for (sample, index) in indices.iter().copied().enumerate() {
                    for (field, span) in outputs.iter().enumerate() {
                        let (handle, local) = span
                            .resolve_record(self.arena.heap(), index)
                            .map_err(|error| LatticeError::Conformance(error.to_string()))?;
                        let descriptor = self
                            .arena
                            .heap()
                            .resolve(handle)
                            .map_err(|error| LatticeError::Conformance(error.to_string()))?;
                        let width = u32::from(descriptor.width);
                        Self::copy_record(
                            &mut encoder,
                            &self.data,
                            wgpu::Origin3d {
                                x: u32::from(descriptor.x) + local % width,
                                y: u32::from(descriptor.y) + local / width,
                                z: u32::from(descriptor.layer),
                            },
                            &buffer,
                            sample * 2 + field,
                        );
                    }
                }
            }
            Mode::Layer => {
                let step = self.layer_step.as_ref().ok_or_else(|| {
                    LatticeError::Conformance("layer has no live output slots".to_string())
                })?;
                for (sample, index) in indices.iter().copied().enumerate() {
                    let origin = wgpu::Origin3d {
                        x: index % step.side,
                        y: index / step.side,
                        z: 0,
                    };
                    Self::copy_record(&mut encoder, &step.midpoint, origin, &buffer, sample * 2);
                    Self::copy_record(
                        &mut encoder,
                        &step.orientation,
                        origin,
                        &buffer,
                        sample * 2 + 1,
                    );
                }
            }
            Mode::A => {
                return Err(LatticeError::Conformance(
                    "Mode A does not produce edge-pose records".to_string(),
                ));
            }
        }
        self.queue.submit([encoder.finish()]);
        Ok(PendingReadback {
            buffer,
            expected_bytes: RECORD_BYTES,
        })
    }

    fn capture_image(&self) -> Result<PendingReadback, LatticeError> {
        if !matches!(
            self.config.format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            return Err(LatticeError::Conformance(format!(
                "surface format {:?} is not a four-byte checksum format",
                self.config.format
            )));
        }
        let target = rectangular_texture(
            &self.device,
            "heap lattice 64x36 conformance image",
            IMAGE_WIDTH,
            IMAGE_HEIGHT,
            self.config.format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = depth_view(&self.device, IMAGE_WIDTH, IMAGE_HEIGHT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap lattice conformance image readback"),
            size: IMAGE_BYTES as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("heap lattice conformance image draw and copy"),
            });
        self.encode_presentation(
            &mut encoder,
            &view,
            &depth,
            "heap lattice 64x36 conformance presentation",
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(IMAGE_BYTES_PER_ROW),
                    rows_per_image: Some(IMAGE_HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: IMAGE_WIDTH,
                height: IMAGE_HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        Ok(PendingReadback {
            buffer,
            expected_bytes: IMAGE_BYTES,
        })
    }

    fn pending_fence(&self) -> PendingFence {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap lattice ordered four-byte MAP_READ fence"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("heap lattice completion fence copy"),
            });
        encoder.copy_buffer_to_buffer(&self.fence_source, 0, &buffer, 0, 4);
        self.queue.submit([encoder.finish()]);
        PendingFence { buffer }
    }

    fn frame_report(&self, generation: u64) -> FrameReport {
        FrameReport {
            generation,
            mode: self.active.mode,
            delivered_edges: self.active.delivered_edges,
            per_frame_cpu_to_gpu_bytes: 192,
            compute_passes: self.active.compute_passes,
            copy_commands: self.active.copy_commands,
            gpu_copy_bytes: self.active.gpu_copy_bytes,
        }
    }
}

thread_local! {
    static LAB: RefCell<Option<Rc<RefCell<LatticeLab>>>> = const { RefCell::new(None) };
    static SELECTION: Cell<SelectionEpoch<Option<SelectionIntent>>> = const {
        Cell::new(SelectionEpoch::new(None))
    };
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
    static PANIC_HOOK_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

fn current_lab() -> Result<Rc<RefCell<LatticeLab>>, LatticeError> {
    LAB.with(|slot| {
        slot.try_borrow()
            .map_err(|_| LatticeError::BorrowConflict("reading the lab slot"))?
            .as_ref()
            .cloned()
            .ok_or_else(|| LatticeError::Capability("heap lattice is not initialized".to_string()))
    })
}

fn begin_selection(intent: SelectionIntent) -> u64 {
    SELECTION.with(|slot| {
        let mut epoch = slot.get();
        let generation = epoch.select(Some(intent));
        slot.set(epoch);
        generation
    })
}

fn invalidate_selection() -> u64 {
    SELECTION.with(|slot| {
        let mut epoch = slot.get();
        let generation = epoch.invalidate();
        slot.set(epoch);
        generation
    })
}

fn current_generation() -> u64 {
    SELECTION.with(|slot| slot.get().generation())
}

fn generation_is_current(generation: u64) -> bool {
    SELECTION.with(|slot| slot.get().is_current(generation))
}

fn stale_generation(generation: u64) -> LatticeError {
    LatticeError::StaleGeneration {
        observed: generation,
        current: current_generation(),
    }
}

fn borrow_for_generation<'a>(
    lab: &'a Rc<RefCell<LatticeLab>>,
    generation: u64,
    operation: &'static str,
) -> Result<Ref<'a, LatticeLab>, LatticeError> {
    if !generation_is_current(generation) {
        return Err(stale_generation(generation));
    }
    lab.try_borrow()
        .map_err(|_| LatticeError::BorrowConflict(operation))
}

fn borrow_mut_for_generation<'a>(
    lab: &'a Rc<RefCell<LatticeLab>>,
    generation: u64,
    operation: &'static str,
) -> Result<RefMut<'a, LatticeLab>, LatticeError> {
    if !generation_is_current(generation) {
        return Err(stale_generation(generation));
    }
    lab.try_borrow_mut()
        .map_err(|_| LatticeError::BorrowConflict(operation))
}

fn poll_lab_once(
    lab: &Rc<RefCell<LatticeLab>>,
    counter: &mut PollCounter,
) -> Result<Option<String>, LatticeError> {
    let borrowed = lab
        .try_borrow()
        .map_err(|_| LatticeError::BorrowConflict("one completion poll"))?;
    counter.record().map_err(LatticeError::PollLimit)?;
    borrowed.device.poll(wgpu::Maintain::Poll);
    let reason = borrowed.lost.lock().ok().and_then(|slot| slot.clone());
    Ok(reason)
}

fn try_apply_selection(
    lab: &Rc<RefCell<LatticeLab>>,
    intent: SelectionIntent,
    generation: u64,
) -> Result<Option<SelectionReport>, LatticeError> {
    if !generation_is_current(generation) {
        return Err(stale_generation(generation));
    }
    let Ok(mut borrowed) = lab.try_borrow_mut() else {
        return Ok(None);
    };
    let report = borrowed.select(intent.mode, intent.step, intent.policy, generation)?;
    borrowed.render_frame(0.0)?;
    Ok(Some(report))
}

/// Installs the wasm panic reporter during module initialization.
#[wasm_bindgen(start)]
pub fn install_heap_lattice_panic_hook() {
    if PANIC_HOOK_INSTALLED.replace(true) {
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let location = info.location().map_or_else(
            || "unknown location".to_string(),
            |location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            },
        );
        let message = format!("heap lattice panic at {location}: {payload}");
        LAST_PANIC.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                *slot = Some(message.clone());
            }
        });
        web_sys::console::error_1(&JsValue::from_str(&message));
        if let Some(status) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("status"))
        {
            status.set_class_name("status failed");
            status.set_text_content(Some(&message));
        }
    }));
}

/// Returns and clears the most recent panic report captured by the initialization hook.
#[wasm_bindgen]
pub fn take_heap_lattice_panic() -> Option<String> {
    LAST_PANIC.with(|slot| {
        slot.try_borrow_mut()
            .ok()
            .and_then(|mut message| message.take())
    })
}

async fn wait_for_fence(
    lab: &Rc<RefCell<LatticeLab>>,
    pending: PendingFence,
    generation: u64,
) -> Result<FenceWait, LatticeError> {
    let state = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&state);
    let slice = pending.buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        if let Ok(mut slot) = callback.lock() {
            *slot = Some(result.map_err(|error| error.to_string()));
        }
    });
    let started = performance_now();
    let mut counter = PollCounter::new();
    loop {
        let current = current_generation();
        if current != generation {
            pending.buffer.unmap();
            return Err(LatticeError::StaleGeneration {
                observed: generation,
                current,
            });
        }
        if counter.polls() > 0 && performance_now() - started >= COMPLETION_DEADLINE_MS {
            pending.buffer.unmap();
            return Err(LatticeError::Deadline);
        }
        match poll_lab_once(lab, &mut counter) {
            Ok(Some(reason)) => {
                pending.buffer.unmap();
                return Err(LatticeError::DeviceLost(reason));
            }
            Ok(None) => {}
            Err(LatticeError::BorrowConflict(_)) => {
                yield_to_browser().await?;
                continue;
            }
            Err(error) => {
                pending.buffer.unmap();
                return Err(error);
            }
        }
        if let Some(result) = state.lock().ok().and_then(|mut slot| slot.take()) {
            result.map_err(LatticeError::Mapping)?;
            let bytes = slice.get_mapped_range();
            if bytes.len() != 4 {
                let length = bytes.len();
                drop(bytes);
                pending.buffer.unmap();
                return Err(LatticeError::Mapping(format!(
                    "completion fence mapped {length} bytes instead of 4"
                )));
            }
            drop(bytes);
            pending.buffer.unmap();
            return Ok(FenceWait {
                polls: counter.polls(),
                waited_ms: performance_now() - started,
            });
        }
        yield_to_browser().await?;
    }
}

async fn wait_for_readback(
    lab: &Rc<RefCell<LatticeLab>>,
    pending: PendingReadback,
    generation: u64,
) -> Result<Vec<u8>, LatticeError> {
    let state = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&state);
    let slice = pending.buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        if let Ok(mut slot) = callback.lock() {
            *slot = Some(result.map_err(|error| error.to_string()));
        }
    });
    let started = performance_now();
    let mut counter = PollCounter::new();
    loop {
        let current = current_generation();
        if current != generation {
            pending.buffer.unmap();
            return Err(LatticeError::StaleGeneration {
                observed: generation,
                current,
            });
        }
        if counter.polls() > 0 && performance_now() - started >= COMPLETION_DEADLINE_MS {
            pending.buffer.unmap();
            return Err(LatticeError::Deadline);
        }
        match poll_lab_once(lab, &mut counter) {
            Ok(Some(reason)) => {
                pending.buffer.unmap();
                return Err(LatticeError::DeviceLost(reason));
            }
            Ok(None) => {}
            Err(LatticeError::BorrowConflict(_)) => {
                yield_to_browser().await?;
                continue;
            }
            Err(error) => {
                pending.buffer.unmap();
                return Err(error);
            }
        }
        if let Some(result) = state.lock().ok().and_then(|mut slot| slot.take()) {
            result.map_err(LatticeError::Mapping)?;
            let mapped = slice.get_mapped_range();
            if mapped.len() != pending.expected_bytes {
                let length = mapped.len();
                drop(mapped);
                pending.buffer.unmap();
                return Err(LatticeError::Mapping(format!(
                    "conformance readback mapped {length} bytes instead of {}",
                    pending.expected_bytes
                )));
            }
            let bytes = mapped.to_vec();
            drop(mapped);
            pending.buffer.unmap();
            return Ok(bytes);
        }
        yield_to_browser().await?;
    }
}

async fn run_conformance(
    lab: &Rc<RefCell<LatticeLab>>,
    generation: u64,
    step: u32,
    policy: u32,
) -> Result<ConformanceReport, LatticeError> {
    let (mode_c_report, mode_c_pending, indices) = {
        let mut borrowed =
            borrow_mut_for_generation(lab, generation, "starting Mode C conformance")?;
        let report = borrowed.select(Mode::C, step, policy, generation)?;
        borrowed.render_frame(CONFORMANCE_TIME)?;
        let indices = deterministic_indices(report.delivered_edges);
        let pending = (!indices.is_empty())
            .then(|| borrowed.capture_records(Mode::C, &indices))
            .transpose()?;
        (report, pending, indices)
    };
    let mode_c_records = match mode_c_pending {
        Some(pending) => Some(wait_for_readback(lab, pending, generation).await?),
        None => None,
    };
    let (layer_report, layer_pending) = {
        let mut borrowed =
            borrow_mut_for_generation(lab, generation, "starting layer conformance")?;
        let report = borrowed.select(Mode::Layer, step, policy, generation)?;
        borrowed.render_frame(CONFORMANCE_TIME)?;
        let pending = (report.delivered_edges == mode_c_report.delivered_edges
            && !indices.is_empty())
        .then(|| borrowed.capture_records(Mode::Layer, &indices))
        .transpose()?;
        (report, pending)
    };
    let layer_records = match layer_pending {
        Some(pending) => Some(wait_for_readback(lab, pending, generation).await?),
        None => None,
    };
    let counts_match = mode_c_report.delivered_edges == layer_report.delivered_edges;
    let signatures_match = mode_c_report.equal_work_signature == layer_report.equal_work_signature;
    let numeric = match (mode_c_records, layer_records) {
        (Some(mode_c), Some(layer)) => {
            compare_records(&mode_c, &layer, indices).map_err(LatticeError::Conformance)?
        }
        _ => NumericComparison {
            sampled_indices: indices,
            compared_records: 0,
            compared_components: 0,
            exact_components: 0,
            mismatched_components: 0,
            tolerance: crate::conformance::F32_TOLERANCE,
            max_abs_error: 0.0,
            pass: false,
        },
    };

    let mode_c_image = {
        let pending = {
            let mut borrowed =
                borrow_mut_for_generation(lab, generation, "starting Mode C image conformance")?;
            let report = borrowed.select(
                Mode::C,
                CONFORMANCE_IMAGE_STEP,
                CONFORMANCE_IMAGE_POLICY,
                generation,
            )?;
            if report.delivered_edges != CONFORMANCE_IMAGE_POLICY {
                return Err(LatticeError::Conformance(format!(
                    "Mode C small-rung image delivered {} edges instead of {}",
                    report.delivered_edges, CONFORMANCE_IMAGE_POLICY
                )));
            }
            borrowed.render_frame(CONFORMANCE_TIME)?;
            borrowed.capture_image()?
        };
        wait_for_readback(lab, pending, generation).await?
    };
    let layer_image = {
        let pending = {
            let mut borrowed =
                borrow_mut_for_generation(lab, generation, "starting layer image conformance")?;
            let report = borrowed.select(
                Mode::Layer,
                CONFORMANCE_IMAGE_STEP,
                CONFORMANCE_IMAGE_POLICY,
                generation,
            )?;
            if report.delivered_edges != CONFORMANCE_IMAGE_POLICY {
                return Err(LatticeError::Conformance(format!(
                    "layer small-rung image delivered {} edges instead of {}",
                    report.delivered_edges, CONFORMANCE_IMAGE_POLICY
                )));
            }
            borrowed.render_frame(CONFORMANCE_TIME)?;
            borrowed.capture_image()?
        };
        wait_for_readback(lab, pending, generation).await?
    };
    let image = compare_images(&mode_c_image, &layer_image).map_err(LatticeError::Conformance)?;
    let timing_qualified = counts_match && signatures_match && numeric.pass && image.pass;
    Ok(ConformanceReport {
        generation,
        requested_step: step,
        requested_axes: mode_c_report.requested_axes,
        policy,
        mode_c_delivered_edges: mode_c_report.delivered_edges,
        layer_delivered_edges: layer_report.delivered_edges,
        counts_match,
        signatures_match,
        numeric,
        image_step: CONFORMANCE_IMAGE_STEP,
        image_edges: CONFORMANCE_IMAGE_POLICY,
        image,
        status: if timing_qualified { "PASS" } else { "FAIL" },
        timing_qualified,
    })
}

/// Initializes the explicit GL backend and renders Mode A step one immediately.
///
/// # Errors
///
/// Returns a JavaScript error for a missing WebGL2 capability, allocation, or initial frame.
#[wasm_bindgen]
pub async fn start_heap_lattice(canvas: web_sys::HtmlCanvasElement) -> Result<String, JsValue> {
    let initial_intent = SelectionIntent {
        mode: Mode::A,
        step: 1,
        policy: DEFAULT_POLICY,
    };
    let generation = begin_selection(initial_intent);
    LAB.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| LatticeError::BorrowConflict("clearing the lab slot"))?;
        *slot = None;
        Ok::<_, LatticeError>(())
    })
    .map_err(JsValue::from)?;
    let (mut lab, adapter, backend) = LatticeLab::new(canvas).await.map_err(JsValue::from)?;
    if !generation_is_current(generation) {
        return Err(JsValue::from(stale_generation(generation)));
    }
    let initial = lab
        .select(Mode::A, 1, DEFAULT_POLICY, generation)
        .map_err(JsValue::from)?;
    lab.render_frame(0.0).map_err(JsValue::from)?;
    let report = InitReport {
        adapter,
        backend,
        webgl_only: true,
        heap_side: HEAP_SIDE,
        heap_layers: HEAP_LAYERS,
        heap_bytes: u64::from(HEAP_SIDE) * u64::from(HEAP_SIDE) * u64::from(HEAP_LAYERS) * 16,
        scratch_layers: 4,
        scratch_bytes: u64::from(HEAP_SIDE) * u64::from(HEAP_SIDE) * 4 * 16,
        descriptor_capacity: DESCRIPTOR_CAPACITY,
        span_capacity: SPAN_CAPACITY,
        handle_capacity: HANDLE_CAPACITY,
        header_stride: lab.header_stride,
        max_texture_dimension_2d: lab.device.limits().max_texture_dimension_2d,
        max_texture_array_layers: lab.device.limits().max_texture_array_layers,
        max_uniform_buffer_binding_size: lab.device.limits().max_uniform_buffer_binding_size,
        configured_layer_byte_budget: LAYER_BYTE_BUDGET,
        timestamp_query_exposed: lab
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
        timestamp_query_used: false,
        completion_poll_limit: MAX_COMPLETION_POLLS,
        output_path: "paid SCRATCH render plus exact-region copy_texture_to_texture into DATA; ping-pong remains spike-only fallback",
        completion: "draw submit then four-byte MAP_READ fence; counted device.poll(Poll) before each zero-timeout browser yield; 4096-poll and 30000 ms bounds; surface present after the measured fence timestamp; generation guard",
        initial,
    };
    LAB.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| LatticeError::BorrowConflict("publishing the initialized lab"))?;
        *slot = Some(Rc::new(RefCell::new(lab)));
        Ok::<_, LatticeError>(())
    })
    .map_err(JsValue::from)?;
    serialize(&report).map_err(JsValue::from)
}

/// Invalidates every in-flight page measurement.
#[wasm_bindgen]
pub fn cancel_heap_lattice() {
    invalidate_selection();
}

/// Selects a mode and rung, computes live walls, and renders without measurement admission.
///
/// # Errors
///
/// Returns a JavaScript error for an unknown selection, allocation refusal, or frame failure.
#[wasm_bindgen]
pub async fn select_heap_lattice_json(
    mode: &str,
    step: u32,
    policy: u32,
) -> Result<String, JsValue> {
    let mode = Mode::parse(mode).map_err(JsValue::from)?;
    let intent = SelectionIntent { mode, step, policy };
    let generation = begin_selection(intent);
    let lab = current_lab().map_err(JsValue::from)?;
    loop {
        if let Some(report) =
            try_apply_selection(&lab, intent, generation).map_err(JsValue::from)?
        {
            return serialize(&report).map_err(JsValue::from);
        }
        yield_to_browser().await.map_err(JsValue::from)?;
    }
}

/// Runs the live Mode C versus layer record and small-image equality gate, then restores the page selection.
///
/// # Errors
///
/// Returns a JavaScript error for cancellation, allocation, rendering, mapping, comparison, restoration, or serialization failure.
#[wasm_bindgen]
pub async fn conform_heap_lattice_json() -> Result<String, JsValue> {
    let generation = current_generation();
    let lab = current_lab().map_err(JsValue::from)?;
    let (saved_mode, step, policy) = {
        let borrowed =
            borrow_for_generation(&lab, generation, "reading the selection before conformance")
                .map_err(JsValue::from)?;
        (
            Mode::from_label(borrowed.active.mode).map_err(JsValue::from)?,
            borrowed.active.requested_step,
            borrowed.active.policy,
        )
    };
    let outcome = run_conformance(&lab, generation, step, policy).await;
    let restore = if generation_is_current(generation) {
        let mut borrowed = borrow_mut_for_generation(
            &lab,
            generation,
            "restoring the selection after conformance",
        )
        .map_err(JsValue::from)?;
        borrowed
            .select(saved_mode, step, policy, generation)
            .and_then(|_| borrowed.render_frame(0.0))
    } else {
        Err(stale_generation(generation))
    };
    restore.map_err(JsValue::from)?;
    serialize(&outcome.map_err(JsValue::from)?).map_err(JsValue::from)
}

/// Renders one current selection at the supplied animation time.
///
/// # Errors
///
/// Returns a JavaScript error for absent initialization, device loss, or surface failure.
#[wasm_bindgen]
pub fn render_heap_lattice_frame_json(time_seconds: f32) -> Result<String, JsValue> {
    let generation = current_generation();
    let lab = current_lab().map_err(JsValue::from)?;
    let report = {
        let mut lab = borrow_mut_for_generation(&lab, generation, "rendering an animation frame")
            .map_err(JsValue::from)?;
        lab.render_frame(time_seconds).map_err(JsValue::from)?;
        lab.frame_report(generation)
    };
    serialize(&report).map_err(JsValue::from)
}

/// Measures an adaptive batch through the ordered mapped fence.
///
/// # Errors
///
/// Returns a JavaScript error for invalid repeats, cancellation, device loss, timeout, mapping, or rendering failure.
#[wasm_bindgen]
pub async fn measure_heap_lattice_batch_json(
    time_seconds: f32,
    repeats: u32,
) -> Result<String, JsValue> {
    if !(1..=4_096).contains(&repeats) {
        return Err(JsValue::from(LatticeError::InvalidRepeat(repeats)));
    }
    let generation = current_generation();
    let lab = current_lab().map_err(JsValue::from)?;
    let started = performance_now();
    let (pending, frame, mode, delivered_edges) = {
        let mut borrowed =
            borrow_mut_for_generation(&lab, generation, "submitting a measured batch")
                .map_err(JsValue::from)?;
        let (pending, frame) = borrowed
            .submit_measured_batch(time_seconds, repeats)
            .map_err(JsValue::from)?;
        (
            pending,
            frame,
            borrowed.active.mode,
            borrowed.active.delivered_edges,
        )
    };
    let fence_wait = wait_for_fence(&lab, pending, generation)
        .await
        .map_err(JsValue::from)?;
    let elapsed_ms = performance_now() - started;
    frame.present();
    let normalized_ms = elapsed_ms / f64::from(repeats);
    serialize(&BatchReport {
        generation,
        mode,
        repeats,
        elapsed_ms,
        normalized_ms,
        fence_polls: fence_wait.polls,
        fence_wait_ms: fence_wait.waited_ms,
        microseconds_per_edge: (delivered_edges > 0)
            .then(|| normalized_ms * 1_000.0 / f64::from(delivered_edges)),
        delivered_edges,
        per_frame_cpu_to_gpu_bytes: 192,
        timing_method: "CPU encode plus presentation-draw submit through ordered four-byte MAP_READ fence wall clock; surface present occurs after the recorded end timestamp",
        gpu_timestamp_ms: None,
    })
    .map_err(JsValue::from)
}
