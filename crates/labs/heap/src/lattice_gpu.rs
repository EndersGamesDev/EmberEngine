//! WebGL2-only heap-lattice page runtime and mapped-fence measurement path.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::future_not_send,
    clippy::too_many_lines
)]

use std::cell::{Cell, RefCell};
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bytemuck::Zeroable as _;
use ember_lab_layer::geometry::{
    Prism, lattice_copy_count, lattice_edge_count, lattice_steps, prism,
};
use ember_lab_layer::kernels::LATTICE_EDGE_KERNEL;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt as _;

use crate::{
    BOX_INDICES, ComparatorWork, DataSpan, DialectLimits, DispatchPlan, EqualWorkSignature,
    FrameUniform, HeapKind, KernelDesc, ModeCFrameUniform, RegisteredKernel, SpanArena,
    StaticHeaders, box_vertices, frame_for, mode_a_records, mode_a_shader, mode_c_register,
    mode_c_shader,
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
const DEFAULT_POLICY: u32 = 2_147_483_647;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const COMPLETION_DEADLINE_MS: f64 = 30_000.0;
const LAYER_DRAW_SHADER: &str = include_str!("layer-draw.wgsl");

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
    #[error("lattice generation {observed} is stale; current is {current}")]
    StaleGeneration { observed: u64, current: u64 },
    #[error("device lost: {0}")]
    DeviceLost(String),
    #[error("unknown lattice mode {0}")]
    UnknownMode(String),
    #[error("lattice step {0} is outside 0..113")]
    UnknownStep(u32),
    #[error("repeat count must be in 1..=4096, got {0}")]
    InvalidRepeat(u32),
    #[error("could not serialize page facts: {0}")]
    Serialization(String),
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
    microseconds_per_edge: Option<f64>,
    delivered_edges: u32,
    per_frame_cpu_to_gpu_bytes: u32,
    timing_method: &'static str,
    gpu_timestamp_ms: Option<f64>,
}

struct PendingFence {
    buffer: wgpu::Buffer,
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
            depth_compare: wgpu::CompareFunction::Less,
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

fn layer_compute_source(output_side: u32, logical_len: u32) -> String {
    format!(
        r"
@group(0) @binding(0) var layer_edge: texture_2d<f32>;
@group(0) @binding(1) var layer_base_four: texture_2d<f32>;
@group(0) @binding(2) var layer_base_fifth: texture_2d<f32>;
fn load_layer(texture: texture_2d<f32>, index: u32) -> vec4<f32> {{
    let width = textureDimensions(texture).x;
    return textureLoad(texture, vec2<i32>(i32(index % width), i32(index / width)), 0);
}}
fn load_edge(index: u32) -> vec4<f32> {{ return load_layer(layer_edge, index); }}
fn load_base_four(index: u32) -> vec4<f32> {{ return load_layer(layer_base_four, index); }}
fn load_base_fifth(index: u32) -> vec4<f32> {{ return load_layer(layer_base_fifth, index); }}
{LATTICE_EDGE_KERNEL}
@group(0) @binding(3) var<uniform> layer_uniforms: LatticeUniform;
struct FullscreenOut {{ @builtin(position) position: vec4<f32>, }}
@vertex fn layer_compute_vertex(@builtin(vertex_index) vertex: u32) -> FullscreenOut {{
    var points = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var output: FullscreenOut; output.position = vec4(points[vertex], 0.0, 1.0); return output;
}}
struct LayerOutput {{ @location(0) midpoint_hue: vec4<f32>, @location(1) orientation_length: vec4<f32>, }}
@fragment fn layer_compute_fragment(@builtin(position) position: vec4<f32>) -> LayerOutput {{
    let index = u32(position.y) * {output_side}u + u32(position.x);
    if (index >= {logical_len}u) {{ discard; }}
    let result = kernel(index, layer_uniforms);
    var output: LayerOutput; output.midpoint_hue = result.midpoint_hue; output.orientation_length = result.orientation_length; return output;
}}
"
    )
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
            LAYER_DRAW_SHADER,
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
                let plan = self
                    .mode_a_kernel
                    .plan_dispatch(
                        &self.arena,
                        &[&self.base_four, &self.base_fifth],
                        &[&self.mode_a_outputs[0], &self.mode_a_outputs[1]],
                        bytemuck::bytes_of(&frame),
                        &headers,
                    )
                    .map_err(|error| LatticeError::Resource(error.to_string()))?;
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
                    let plan = self
                        .mode_c_kernel
                        .plan_dispatch(
                            &self.arena,
                            &[&self.edge, &self.base_four, &self.base_fifth],
                            &[&outputs[0], &outputs[1]],
                            bytemuck::bytes_of(&uniform),
                            &headers,
                        )
                        .map_err(|error| LatticeError::Resource(error.to_string()))?;
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
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
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
        let source = layer_compute_source(side, edges);
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

    fn render_frame(&mut self, time: f32) -> Result<(), LatticeError> {
        if let Some(reason) = self.lost.lock().ok().and_then(|slot| slot.clone()) {
            return Err(LatticeError::DeviceLost(reason));
        }
        let uniform = self.frame_uniform(time);
        self.queue.write_buffer(&self.frame_buffer, 0, &uniform);
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
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
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
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("heap lattice one presentation pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
                    view: &self.depth,
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
        self.queue.submit([encoder.finish()]);
        frame.present();
        Ok(())
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
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

fn current_lab() -> Result<Rc<RefCell<LatticeLab>>, LatticeError> {
    LAB.with_borrow(|slot| {
        slot.as_ref()
            .cloned()
            .ok_or_else(|| LatticeError::Capability("heap lattice is not initialized".to_string()))
    })
}

async fn wait_for_fence(
    lab: &Rc<RefCell<LatticeLab>>,
    pending: PendingFence,
    generation: u64,
) -> Result<(), LatticeError> {
    let state = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&state);
    let slice = pending.buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        if let Ok(mut slot) = callback.lock() {
            *slot = Some(result.map_err(|error| error.to_string()));
        }
    });
    let started = performance_now();
    loop {
        let current = GENERATION.get();
        if current != generation {
            pending.buffer.unmap();
            return Err(LatticeError::StaleGeneration {
                observed: generation,
                current,
            });
        }
        {
            let borrowed = lab.borrow();
            borrowed.device.poll(wgpu::Maintain::Poll);
            if let Some(reason) = borrowed.lost.lock().ok().and_then(|slot| slot.clone()) {
                pending.buffer.unmap();
                return Err(LatticeError::DeviceLost(reason));
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
            return Ok(());
        }
        if performance_now() - started >= COMPLETION_DEADLINE_MS {
            pending.buffer.unmap();
            return Err(LatticeError::Deadline);
        }
        yield_to_browser().await?;
    }
}

/// Initializes the explicit GL backend and renders Mode A step one immediately.
///
/// # Errors
///
/// Returns a JavaScript error for a missing WebGL2 capability, allocation, or initial frame.
#[wasm_bindgen]
pub async fn start_heap_lattice(canvas: web_sys::HtmlCanvasElement) -> Result<String, JsValue> {
    let generation = GENERATION.get().wrapping_add(1);
    GENERATION.set(generation);
    LAB.with_borrow_mut(|slot| *slot = None);
    let (mut lab, adapter, backend) = LatticeLab::new(canvas).await.map_err(JsValue::from)?;
    if GENERATION.get() != generation {
        return Err(JsValue::from(LatticeError::StaleGeneration {
            observed: generation,
            current: GENERATION.get(),
        }));
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
        output_path: "paid SCRATCH render plus exact-region copy_texture_to_texture into DATA; ping-pong remains spike-only fallback",
        completion: "four-byte MAP_READ fence; device.poll(Poll) in zero-timeout browser-yield loop; 30000 ms deadline; generation guard",
        initial,
    };
    LAB.with_borrow_mut(|slot| *slot = Some(Rc::new(RefCell::new(lab))));
    serialize(&report).map_err(JsValue::from)
}

/// Invalidates every in-flight page measurement.
#[wasm_bindgen]
pub fn cancel_heap_lattice() {
    GENERATION.set(GENERATION.get().wrapping_add(1));
}

/// Selects a mode and rung, computes live walls, and renders without measurement admission.
///
/// # Errors
///
/// Returns a JavaScript error for an unknown selection, allocation refusal, or frame failure.
#[wasm_bindgen]
pub fn select_heap_lattice_json(mode: &str, step: u32, policy: u32) -> Result<String, JsValue> {
    let mode = Mode::parse(mode).map_err(JsValue::from)?;
    let generation = GENERATION.get().wrapping_add(1);
    GENERATION.set(generation);
    let lab = current_lab().map_err(JsValue::from)?;
    let report = {
        let mut lab = lab.borrow_mut();
        let report = lab
            .select(mode, step, policy, generation)
            .map_err(JsValue::from)?;
        lab.render_frame(0.0).map_err(JsValue::from)?;
        report
    };
    serialize(&report).map_err(JsValue::from)
}

/// Renders one current selection at the supplied animation time.
///
/// # Errors
///
/// Returns a JavaScript error for absent initialization, device loss, or surface failure.
#[wasm_bindgen]
pub fn render_heap_lattice_frame_json(time_seconds: f32) -> Result<String, JsValue> {
    let generation = GENERATION.get();
    let lab = current_lab().map_err(JsValue::from)?;
    let report = {
        let mut lab = lab.borrow_mut();
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
    let generation = GENERATION.get();
    let lab = current_lab().map_err(JsValue::from)?;
    let started = performance_now();
    let (pending, mode, delivered_edges) = {
        let mut borrowed = lab.borrow_mut();
        for repeat in 0..repeats {
            borrowed
                .render_frame(time_seconds + repeat as f32 * 0.000_001)
                .map_err(JsValue::from)?;
        }
        (
            borrowed.pending_fence(),
            borrowed.active.mode,
            borrowed.active.delivered_edges,
        )
    };
    wait_for_fence(&lab, pending, generation)
        .await
        .map_err(JsValue::from)?;
    let elapsed_ms = performance_now() - started;
    let normalized_ms = elapsed_ms / f64::from(repeats);
    serialize(&BatchReport {
        generation,
        mode,
        repeats,
        elapsed_ms,
        normalized_ms,
        microseconds_per_edge: (delivered_edges > 0)
            .then(|| normalized_ms * 1_000.0 / f64::from(delivered_edges)),
        delivered_edges,
        per_frame_cpu_to_gpu_bytes: 192,
        timing_method: "CPU encode plus queue submit through ordered four-byte MAP_READ fence wall clock",
        gpu_timestamp_ms: None,
    })
    .map_err(JsValue::from)
}
