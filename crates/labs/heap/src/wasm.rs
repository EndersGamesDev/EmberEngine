//! WebGL2 lowering, browser exports, and completion-fenced benchmark runner.

// Browser GPU limits and fixed instance records deliberately cross usize/u32/f32 boundaries.
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use std::cell::{Cell, RefCell};
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, closure::Closure};
use wgpu::util::DeviceExt;

use crate::heap::{Handle, HeapAllocator, HeapError, HeapKind};
use crate::kernels::{
    DIRECT_FETCH_SHADER, DRAW_STEPS, FETCH_HEIGHT, FETCH_WIDTH, FETCHES_PER_FRAGMENT,
    HEAP_DRAW_SHADER_TEMPLATE, HEAP_FETCH_SHADER_TEMPLATE, PAYLOAD_SIDE, TRADITIONAL_DRAW_SHADER,
    heap_shader, material_color, payload_texel,
};

const HEAP_SIDE: u16 = 256;
const REQUESTED_LAYERS: u16 = 8;
const MAX_DESCRIPTORS: u32 = 4_096;
const FETCH_COMPLETION_DEADLINE_MS: i32 = 4_000;
const DRAW_COMPLETION_DEADLINE_MS: i32 = 30_000;
const MATERIAL_UNIFORM_STRIDE: u64 = 256;

#[derive(Debug, thiserror::Error)]
enum LabError {
    #[error("WebGL2 capability refused: {0}")]
    Capability(String),
    #[error("heap allocation failed: {0}")]
    Heap(#[from] HeapError),
    #[error("surface failed: {0}")]
    Surface(String),
    #[error("completion mapping failed: {0}")]
    Mapping(String),
    #[error("completion exceeded {0} ms")]
    Deadline(i32),
    #[error("benchmark generation {observed} is stale; current is {current}")]
    StaleGeneration { observed: u64, current: u64 },
    #[error("device lost: {0}")]
    DeviceLost(String),
    #[error("internal benchmark state was already borrowed during {0}")]
    BorrowConflict(&'static str),
    #[error("unknown benchmark mode {0}")]
    UnknownMode(String),
    #[error("draw step {0} is not one of 16, 64, 256, 1024, 4096, 16384, 65536, 262144, 1048576")]
    UnknownStep(u32),
    #[error("repeat count must be in 1..=4096, got {0}")]
    InvalidRepeat(u32),
    #[error("could not serialize browser facts: {0}")]
    Serialization(String),
}

impl From<LabError> for JsValue {
    fn from(error: LabError) -> Self {
        Self::from_str(&error.to_string())
    }
}

#[derive(Clone, Copy, Debug)]
enum FetchMode {
    Direct,
    Heap,
}

impl FetchMode {
    fn parse(mode: &str) -> Result<Self, LabError> {
        match mode {
            "direct" => Ok(Self::Direct),
            "heap" => Ok(Self::Heap),
            _ => Err(LabError::UnknownMode(mode.to_string())),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Direct => "direct-bound-texture",
            Self::Heap => "descriptor-indirect-data-heap",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum DrawMode {
    Traditional,
    Heap,
}

impl DrawMode {
    fn parse(mode: &str) -> Result<Self, LabError> {
        match mode {
            "traditional" => Ok(Self::Traditional),
            "heap" => Ok(Self::Heap),
            _ => Err(LabError::UnknownMode(mode.to_string())),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Traditional => "traditional-per-draw-bind-and-uniform-write",
            Self::Heap => "one-bind-group-static-instance-handle",
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceData {
    placement: [f32; 4],
    handle: u32,
    shape: u32,
    padding: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform16 {
    values: [f32; 4],
}

struct Scene {
    requested_draws: u32,
    delivered_draws: u32,
    instances: wgpu::Buffer,
}

struct TraditionalMaterial {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

#[derive(Serialize)]
struct StepFact {
    requested_draws: u32,
    delivered_draws: u32,
    requested_distinct_resources: u32,
    delivered_distinct_resources: u32,
    repeated_resource_draws: u32,
    traditional_frame_bytes: u64,
    heap_frame_bytes: u64,
    arithmetic: String,
}

#[derive(Serialize)]
struct LabFacts {
    adapter_name: String,
    backend: String,
    max_texture_dimension_2d: u32,
    max_texture_array_layers: u32,
    max_uniform_buffer_binding_size: u32,
    rgba32f_usages: String,
    timestamp_query_exposed: bool,
    timestamp_query_used: bool,
    heap_side: u16,
    heap_layers: u16,
    data_heap_bytes: u64,
    image_heap_bytes: u64,
    descriptor_capacity: u32,
    permanently_missing_descriptors: u32,
    benchmark_a_data_descriptors: u32,
    delivered_material_capacity: u32,
    max_draws_per_instance_buffer: u32,
    free_descriptor_count: usize,
    data_free_buddy_blocks: usize,
    image_free_buddy_blocks: usize,
    scene_setup_cpu_to_gpu_bytes: u64,
    benchmark_a_workload: String,
    benchmark_b_workload: String,
    completion: &'static str,
    generation_validation: &'static str,
    steps: Vec<StepFact>,
}

#[derive(Serialize)]
struct DrawReport {
    generation: u64,
    mode: &'static str,
    requested_draws: u32,
    delivered_draws: u32,
    requested_distinct_resources: u32,
    delivered_distinct_resources: u32,
    repeated_resource_draws: u32,
    per_frame_cpu_to_gpu_bytes: u64,
    scene_setup_cpu_to_gpu_bytes: u64,
    wall_arithmetic: String,
    timing_status: &'static str,
}

#[derive(Serialize)]
struct Measurement {
    generation: u64,
    benchmark: &'static str,
    mode: &'static str,
    repeats: u32,
    elapsed_ms: f64,
    normalized_ms: f64,
    per_draw_microseconds: Option<f64>,
    requested_draws: Option<u32>,
    delivered_draws: Option<u32>,
    per_frame_cpu_to_gpu_bytes: u64,
    timing_method: &'static str,
    gpu_timestamp_ms: Option<f64>,
}

struct PendingFence {
    buffer: wgpu::Buffer,
    deadline_ms: i32,
}

struct Lab {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    lost: Arc<Mutex<Option<String>>>,
    heap_bind_group: wgpu::BindGroup,
    direct_bind_group: wgpu::BindGroup,
    direct_fetch_pipeline: wgpu::RenderPipeline,
    heap_fetch_pipeline: wgpu::RenderPipeline,
    traditional_draw_pipeline: wgpu::RenderPipeline,
    heap_draw_pipeline: wgpu::RenderPipeline,
    fetch_target: wgpu::TextureView,
    scenes: Vec<Scene>,
    traditional_materials: Vec<TraditionalMaterial>,
    material_uniforms: wgpu::Buffer,
    frame_uniform: wgpu::Buffer,
    fence_source: wgpu::Buffer,
    delivered_materials: u32,
    scene_setup_bytes: u64,
    _data_heap: wgpu::Texture,
    _image_heap: wgpu::Texture,
    _direct_payload: wgpu::Texture,
    _descriptor_buffer: wgpu::Buffer,
    _samplers: [wgpu::Sampler; 2],
}

const INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Uint32,
        offset: 16,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Uint32,
        offset: 20,
        shader_location: 2,
    },
];

fn instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<InstanceData>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &INSTANCE_ATTRIBUTES,
    }
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or_else(js_sys::Date::now, |performance| performance.now())
}

async fn yield_to_browser() -> Result<(), LabError> {
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
        .map_err(|error| LabError::Mapping(format!("browser yield failed: {error:?}")))
}

impl Lab {
    #[allow(clippy::too_many_lines)]
    async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<(Self, LabFacts), LabError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| {
                LabError::Capability(format!("could not create canvas surface: {error}"))
            })?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| LabError::Capability("no WebGL2 adapter".to_string()))?;
        let info = adapter.get_info();
        if info.backend != wgpu::Backend::Gl {
            return Err(LabError::Capability(format!(
                "requested GL/WebGL2 but selected {:?}",
                info.backend
            )));
        }
        let adapter_limits = adapter.limits();
        if adapter_limits.max_texture_dimension_2d < u32::from(HEAP_SIDE) {
            return Err(LabError::Capability(format!(
                "max texture dimension {} is below prototype side {HEAP_SIDE}",
                adapter_limits.max_texture_dimension_2d
            )));
        }
        let layers = u16::try_from(
            adapter_limits
                .max_texture_array_layers
                .min(u32::from(REQUESTED_LAYERS)),
        )
        .map_err(|_| LabError::Capability("array layer limit did not fit u16".to_string()))?;
        if layers == 0 {
            return Err(LabError::Capability(
                "texture arrays expose zero layers".to_string(),
            ));
        }
        let descriptor_bytes = adapter_limits
            .max_uniform_buffer_binding_size
            .min(MAX_DESCRIPTORS * 16);
        let descriptor_capacity = descriptor_bytes / 16;
        if descriptor_capacity < 1_024 {
            return Err(LabError::Capability(format!(
                "uniform descriptor table holds {descriptor_capacity} records; 1024 required"
            )));
        }
        let rgba32f = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float);
        let required_rgba32f = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT;
        if !rgba32f.allowed_usages.contains(required_rgba32f) {
            return Err(LabError::Capability(format!(
                "RGBA32Float usages {:?} do not include {:?}",
                rgba32f.allowed_usages, required_rgba32f
            )));
        }
        let mut required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter_limits.clone());
        required_limits.max_texture_array_layers = u32::from(layers);
        required_limits.max_uniform_buffer_binding_size = descriptor_bytes;
        let exposed_features = adapter.features();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("heap lab WebGL2 device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|error| LabError::Capability(format!("device request failed: {error}")))?;
        let lost = Arc::new(Mutex::new(None));
        let lost_callback = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = lost_callback.lock() {
                *slot = Some(format!("{reason:?}: {message}"));
            }
        });
        crate::browser_error::install_logging_handler(&device, "heap benchmark");
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| surface_caps.formats.first().copied())
            .ok_or_else(|| LabError::Capability("surface exposes no format".to_string()))?;
        let present_mode = surface_caps
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            .or_else(|| surface_caps.present_modes.first().copied())
            .ok_or_else(|| LabError::Capability("surface exposes no present mode".to_string()))?;
        let alpha_mode = surface_caps
            .alpha_modes
            .first()
            .copied()
            .ok_or_else(|| LabError::Capability("surface exposes no alpha mode".to_string()))?;
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

        let mut allocator = HeapAllocator::new(HEAP_SIDE, layers, descriptor_capacity)?;
        let data_handle =
            allocator.allocate(HeapKind::Data, PAYLOAD_SIDE as u16, PAYLOAD_SIDE as u16)?;
        if data_handle.index() != 1 {
            return Err(LabError::Capability(format!(
                "Benchmark A requires descriptor 1 but allocator returned {}",
                data_handle.index()
            )));
        }
        let mut material_handles = Vec::new();
        for _ in 0..descriptor_capacity {
            match allocator.allocate(HeapKind::Image, 1, 1) {
                Ok(handle) => material_handles.push(handle),
                Err(HeapError::DescriptorTableFull | HeapError::PhysicalHeapFull { .. }) => break,
                Err(error) => return Err(error.into()),
            }
        }
        let delivered_materials = material_handles.len() as u32;
        if delivered_materials == 0 {
            return Err(LabError::Capability(
                "no descriptor remains for Benchmark B materials".to_string(),
            ));
        }
        let data_descriptor = allocator.resolve(data_handle)?;

        let data_heap = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("heap DATA array"),
            size: wgpu::Extent3d {
                width: u32::from(HEAP_SIDE),
                height: u32::from(HEAP_SIDE),
                depth_or_array_layers: u32::from(layers),
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let image_heap = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("heap IMAGE array"),
            size: wgpu::Extent3d {
                width: u32::from(HEAP_SIDE),
                height: u32::from(HEAP_SIDE),
                depth_or_array_layers: u32::from(layers),
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let direct_payload = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Benchmark A direct payload"),
            size: wgpu::Extent3d {
                width: PAYLOAD_SIDE,
                height: PAYLOAD_SIDE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let payload: Vec<_> = (0..PAYLOAD_SIDE)
            .flat_map(|y| (0..PAYLOAD_SIDE).map(move |x| payload_texel(x, y)))
            .collect();
        let payload_layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(PAYLOAD_SIDE * 16),
            rows_per_image: Some(PAYLOAD_SIDE),
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &direct_payload,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&payload),
            payload_layout,
            wgpu::Extent3d {
                width: PAYLOAD_SIDE,
                height: PAYLOAD_SIDE,
                depth_or_array_layers: 1,
            },
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &data_heap,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: u32::from(data_descriptor.x),
                    y: u32::from(data_descriptor.y),
                    z: u32::from(data_descriptor.layer),
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&payload),
            payload_layout,
            wgpu::Extent3d {
                width: PAYLOAD_SIDE,
                height: PAYLOAD_SIDE,
                depth_or_array_layers: 1,
            },
        );

        let image_texel_count =
            usize::from(HEAP_SIDE) * usize::from(HEAP_SIDE) * usize::from(layers);
        let mut image_bytes = vec![0_u8; image_texel_count * 4];
        for (material, handle) in material_handles.iter().copied().enumerate() {
            let descriptor = allocator.resolve(handle)?;
            let texel =
                (usize::from(descriptor.layer) * usize::from(HEAP_SIDE) * usize::from(HEAP_SIDE)
                    + usize::from(descriptor.y) * usize::from(HEAP_SIDE)
                    + usize::from(descriptor.x))
                    * 4;
            image_bytes[texel..texel + 4].copy_from_slice(&material_color(material as u32));
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &image_heap,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(u32::from(HEAP_SIDE) * 4),
                rows_per_image: Some(u32::from(HEAP_SIDE)),
            },
            wgpu::Extent3d {
                width: u32::from(HEAP_SIDE),
                height: u32::from(HEAP_SIDE),
                depth_or_array_layers: u32::from(layers),
            },
        );
        let packed_table = allocator.packed_table();
        let descriptor_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap descriptor UBO"),
            contents: bytemuck::cast_slice(&packed_table),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let frame_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap frame uniform"),
            contents: bytemuck::bytes_of(&Uniform16::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let data_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("heap DATA nearest sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("heap IMAGE linear sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let data_view = data_heap.create_view(&wgpu::TextureViewDescriptor {
            label: Some("heap DATA array view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(u32::from(layers)),
            ..Default::default()
        });
        let image_view = image_heap.create_view(&wgpu::TextureViewDescriptor {
            label: Some("heap IMAGE array view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(u32::from(layers)),
            ..Default::default()
        });
        let heap_layout = create_heap_layout(&device);
        let heap_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("immutable heap bind group"),
            layout: &heap_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&data_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&data_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&image_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&image_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: descriptor_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: frame_uniform.as_entire_binding(),
                },
            ],
        });
        let direct_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Benchmark A direct layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let direct_view = direct_payload.create_view(&wgpu::TextureViewDescriptor::default());
        let direct_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Benchmark A direct bind group"),
            layout: &direct_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&direct_view),
            }],
        });
        let traditional_layout = create_traditional_layout(&device);
        let material_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("traditional per-material uniforms"),
            size: u64::from(delivered_materials) * MATERIAL_UNIFORM_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut traditional_materials = Vec::with_capacity(delivered_materials as usize);
        for material in 0..delivered_materials {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("traditional one-texel material"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &material_color(material),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let binding_size = NonZeroU64::new(16)
                .ok_or_else(|| LabError::Capability("uniform size was zero".to_string()))?;
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("traditional material bind group"),
                layout: &traditional_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&image_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &material_uniforms,
                            offset: u64::from(material) * MATERIAL_UNIFORM_STRIDE,
                            size: Some(binding_size),
                        }),
                    },
                ],
            });
            traditional_materials.push(TraditionalMaterial {
                _texture: texture,
                bind_group,
            });
            if material % 64 == 63 {
                yield_to_browser().await?;
            }
        }
        let max_draws_per_instance_buffer =
            u32::try_from(device.limits().max_buffer_size / size_of::<InstanceData>() as u64)
                .unwrap_or(u32::MAX);
        let mut scenes = Vec::new();
        let mut scene_upload_bytes = 0_u64;
        for requested_draws in DRAW_STEPS {
            let delivered_draws = requested_draws.min(max_draws_per_instance_buffer);
            let instances = make_instances(delivered_draws, &material_handles);
            let upload_bytes = (instances.len() * size_of::<InstanceData>()) as u64;
            scene_upload_bytes += upload_bytes;
            scenes.push(Scene {
                requested_draws,
                delivered_draws,
                instances: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("static scene instance handles"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            });
        }

        let direct_fetch_pipeline = create_pipeline(
            &device,
            "Benchmark A direct pipeline",
            &direct_layout,
            DIRECT_FETCH_SHADER,
            wgpu::TextureFormat::Rgba8Unorm,
            false,
        );
        let heap_fetch_source = heap_shader(HEAP_FETCH_SHADER_TEMPLATE, descriptor_capacity);
        let heap_fetch_pipeline = create_pipeline(
            &device,
            "Benchmark A heap pipeline",
            &heap_layout,
            &heap_fetch_source,
            wgpu::TextureFormat::Rgba8Unorm,
            false,
        );
        let traditional_draw_pipeline = create_pipeline(
            &device,
            "Benchmark B traditional pipeline",
            &traditional_layout,
            TRADITIONAL_DRAW_SHADER,
            surface_format,
            true,
        );
        let heap_draw_source = heap_shader(HEAP_DRAW_SHADER_TEMPLATE, descriptor_capacity);
        let heap_draw_pipeline = create_pipeline(
            &device,
            "Benchmark B heap pipeline",
            &heap_layout,
            &heap_draw_source,
            surface_format,
            true,
        );
        let fetch_target_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Benchmark A 512x512 target"),
            size: wgpu::Extent3d {
                width: FETCH_WIDTH,
                height: FETCH_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let fetch_target =
            fetch_target_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let fence_source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heap lab four-byte fence source"),
            contents: &[0x68, 0x65, 0x61, 0x70],
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        let scene_setup_bytes = payload.len() as u64 * 16 * 2
            + image_bytes.len() as u64
            + packed_table.len() as u64 * 16
            + u64::from(delivered_materials) * 4
            + scene_upload_bytes;
        let steps = DRAW_STEPS
            .into_iter()
            .map(|draws| {
                step_fact(
                    draws,
                    draws.min(max_draws_per_instance_buffer),
                    delivered_materials,
                )
            })
            .collect();
        let facts = LabFacts {
            adapter_name: info.name,
            backend: format!("{:?}", info.backend),
            max_texture_dimension_2d: adapter_limits.max_texture_dimension_2d,
            max_texture_array_layers: adapter_limits.max_texture_array_layers,
            max_uniform_buffer_binding_size: adapter_limits.max_uniform_buffer_binding_size,
            rgba32f_usages: format!("{:?}", rgba32f.allowed_usages),
            timestamp_query_exposed: exposed_features.contains(wgpu::Features::TIMESTAMP_QUERY),
            timestamp_query_used: false,
            heap_side: HEAP_SIDE,
            heap_layers: layers,
            data_heap_bytes: u64::from(HEAP_SIDE)
                * u64::from(HEAP_SIDE)
                * u64::from(layers)
                * 16,
            image_heap_bytes: u64::from(HEAP_SIDE)
                * u64::from(HEAP_SIDE)
                * u64::from(layers)
                * 4,
            descriptor_capacity,
            permanently_missing_descriptors: 1,
            benchmark_a_data_descriptors: 1,
            delivered_material_capacity: delivered_materials,
            max_draws_per_instance_buffer,
            free_descriptor_count: allocator.free_descriptor_count(),
            data_free_buddy_blocks: allocator.free_block_count(HeapKind::Data),
            image_free_buddy_blocks: allocator.free_block_count(HeapKind::Image),
            scene_setup_cpu_to_gpu_bytes: scene_setup_bytes,
            benchmark_a_workload: format!(
                "{FETCH_WIDTH}x{FETCH_HEIGHT} fragments x {FETCHES_PER_FRAGMENT} nearest RGBA32F loads = {} loads per base invocation",
                u64::from(FETCH_WIDTH) * u64::from(FETCH_HEIGHT) * u64::from(FETCHES_PER_FRAGMENT)
            ),
            benchmark_b_workload: "same delivered N three-vertex draw calls in both arms; requested ladder 16 through 1048576; traditional switches bind group and writes 16 bytes per draw; heap binds once and reads static instance handles".to_string(),
            completion: "submit to ordered four-byte MAP_READ fence; WebGL device.poll(Poll) in zero-timeout yield loop; 4000 ms Benchmark A deadline; 30000 ms Benchmark B deadline",
            generation_validation: "CPU-side on every debug resolve/free; shader generation fetch omitted to keep Benchmark A's measured indirection to one descriptor record",
            steps,
        };
        Ok((
            Self {
                surface,
                config,
                device,
                queue,
                lost,
                heap_bind_group,
                direct_bind_group,
                direct_fetch_pipeline,
                heap_fetch_pipeline,
                traditional_draw_pipeline,
                heap_draw_pipeline,
                fetch_target,
                scenes,
                traditional_materials,
                material_uniforms,
                frame_uniform,
                fence_source,
                delivered_materials,
                scene_setup_bytes,
                _data_heap: data_heap,
                _image_heap: image_heap,
                _direct_payload: direct_payload,
                _descriptor_buffer: descriptor_buffer,
                _samplers: [data_sampler, image_sampler],
            },
            facts,
        ))
    }

    fn lost_reason(&self) -> Option<String> {
        self.lost.lock().ok().and_then(|reason| reason.clone())
    }

    fn pending_fence(&self, encoder: &mut wgpu::CommandEncoder, deadline_ms: i32) -> PendingFence {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heap lab ordered four-byte completion fence"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&self.fence_source, 0, &buffer, 0, 4);
        PendingFence {
            buffer,
            deadline_ms,
        }
    }

    fn render_fetch(&mut self, mode: FetchMode, repeats: u32) -> Result<PendingFence, LabError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Benchmark A adaptive batch"),
            });
        for _ in 0..repeats {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Benchmark A one base invocation"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.fetch_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            match mode {
                FetchMode::Direct => {
                    pass.set_pipeline(&self.direct_fetch_pipeline);
                    pass.set_bind_group(0, &self.direct_bind_group, &[]);
                }
                FetchMode::Heap => {
                    pass.set_pipeline(&self.heap_fetch_pipeline);
                    pass.set_bind_group(0, &self.heap_bind_group, &[]);
                }
            }
            pass.draw(0..3, 0..1);
        }
        let pending = self.pending_fence(&mut encoder, FETCH_COMPLETION_DEADLINE_MS);
        self.queue.submit([encoder.finish()]);
        Ok(pending)
    }

    fn render_draws(
        &mut self,
        mode: DrawMode,
        draws: u32,
        repeats: u32,
        include_fence: bool,
    ) -> Result<Option<PendingFence>, LabError> {
        let scene_index = self
            .scenes
            .iter()
            .position(|scene| scene.requested_draws == draws)
            .ok_or(LabError::UnknownStep(draws))?;
        let delivered_draws = self.scenes[scene_index].delivered_draws;
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                self.surface
                    .get_current_texture()
                    .map_err(|error| LabError::Surface(error.to_string()))?
            }
            Err(error) => return Err(LabError::Surface(error.to_string())),
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Benchmark B adaptive batch"),
            });
        for repeat in 0..repeats {
            match mode {
                DrawMode::Traditional => {
                    for draw in 0..delivered_draws {
                        let material = draw % self.delivered_materials;
                        let uniform = Uniform16 {
                            values: [
                                self.config.width as f32 / self.config.height as f32,
                                (draw & 3) as f32,
                                repeat as f32,
                                0.0,
                            ],
                        };
                        self.queue.write_buffer(
                            &self.material_uniforms,
                            u64::from(material) * MATERIAL_UNIFORM_STRIDE,
                            bytemuck::bytes_of(&uniform),
                        );
                    }
                }
                DrawMode::Heap => {
                    let frame = Uniform16 {
                        values: [
                            self.config.width as f32 / self.config.height as f32,
                            repeat as f32,
                            0.0,
                            0.0,
                        ],
                    };
                    self.queue
                        .write_buffer(&self.frame_uniform, 0, bytemuck::bytes_of(&frame));
                }
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Benchmark B one frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.015,
                            g: 0.025,
                            b: 0.045,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_vertex_buffer(0, self.scenes[scene_index].instances.slice(..));
            match mode {
                DrawMode::Traditional => {
                    pass.set_pipeline(&self.traditional_draw_pipeline);
                    for draw in 0..delivered_draws {
                        let material = (draw % self.delivered_materials) as usize;
                        pass.set_bind_group(
                            0,
                            &self.traditional_materials[material].bind_group,
                            &[],
                        );
                        pass.draw(0..3, draw..draw + 1);
                    }
                }
                DrawMode::Heap => {
                    pass.set_pipeline(&self.heap_draw_pipeline);
                    pass.set_bind_group(0, &self.heap_bind_group, &[]);
                    for draw in 0..delivered_draws {
                        pass.draw(0..3, draw..draw + 1);
                    }
                }
            }
        }
        let pending =
            include_fence.then(|| self.pending_fence(&mut encoder, DRAW_COMPLETION_DEADLINE_MS));
        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(pending)
    }

    fn draw_report(&self, generation: u64, mode: DrawMode, draws: u32) -> DrawReport {
        let delivered_draws = self
            .scenes
            .iter()
            .find(|scene| scene.requested_draws == draws)
            .map_or(0, |scene| scene.delivered_draws);
        let delivered_distinct = delivered_draws.min(self.delivered_materials);
        DrawReport {
            generation,
            mode: mode.label(),
            requested_draws: draws,
            delivered_draws,
            requested_distinct_resources: draws,
            delivered_distinct_resources: delivered_distinct,
            repeated_resource_draws: delivered_draws - delivered_distinct,
            per_frame_cpu_to_gpu_bytes: match mode {
                DrawMode::Traditional => u64::from(delivered_draws) * 16,
                DrawMode::Heap => 16,
            },
            scene_setup_cpu_to_gpu_bytes: self.scene_setup_bytes,
            wall_arithmetic: format!(
                "min(requested draws {draws}, instance-buffer wall {}) = {delivered_draws}; min(delivered draws {delivered_draws}, descriptor-backed material wall {}) = {delivered_distinct}; repeated draws = {}",
                self.max_draws_per_instance_buffer(),
                self.delivered_materials,
                delivered_draws - delivered_distinct
            ),
            timing_status: "requires visible replay",
        }
    }

    fn max_draws_per_instance_buffer(&self) -> u32 {
        u32::try_from(self.device.limits().max_buffer_size / size_of::<InstanceData>() as u64)
            .unwrap_or(u32::MAX)
    }
}

fn create_heap_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("immutable heap layout"),
        entries: &[
            texture_layout(0, wgpu::TextureViewDimension::D2Array, false),
            sampler_layout(1, wgpu::SamplerBindingType::NonFiltering),
            texture_layout(2, wgpu::TextureViewDimension::D2Array, true),
            sampler_layout(3, wgpu::SamplerBindingType::Filtering),
            uniform_layout(4, wgpu::ShaderStages::FRAGMENT),
            uniform_layout(5, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
        ],
    })
}

fn create_traditional_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("traditional material layout"),
        entries: &[
            texture_layout(0, wgpu::TextureViewDimension::D2, true),
            sampler_layout(1, wgpu::SamplerBindingType::Filtering),
            uniform_layout(2, wgpu::ShaderStages::VERTEX),
        ],
    })
}

const fn texture_layout(
    binding: u32,
    view_dimension: wgpu::TextureViewDimension,
    filterable: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension,
            multisampled: false,
        },
        count: None,
    }
}

const fn sampler_layout(
    binding: u32,
    kind: wgpu::SamplerBindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(kind),
        count: None,
    }
}

const fn uniform_layout(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    label: &'static str,
    bind_group_layout: &wgpu::BindGroupLayout,
    shader_source: &str,
    target_format: wgpu::TextureFormat,
    instances: bool,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    let instance_buffers = [instance_layout()];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: if instances { &instance_buffers } else { &[] },
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn make_instances(draws: u32, handles: &[Handle]) -> Vec<InstanceData> {
    let columns = draws.isqrt().max(1);
    let rows = draws.div_ceil(columns);
    let scale_x = 0.88 / columns as f32;
    let scale_y = 0.88 / rows as f32;
    (0..draws)
        .map(|draw| {
            let column = draw % columns;
            let row = draw / columns;
            let x = (column as f32 + 0.5) * 2.0 / columns as f32 - 1.0;
            let y = 1.0 - (row as f32 + 0.5) * 2.0 / rows as f32;
            let handle = handles[draw as usize % handles.len()];
            InstanceData {
                placement: [x, y, scale_x, scale_y],
                handle: handle.raw(),
                shape: draw & 3,
                padding: [0; 2],
            }
        })
        .collect()
}

fn step_fact(draws: u32, delivered_draws: u32, material_wall: u32) -> StepFact {
    let delivered = delivered_draws.min(material_wall);
    StepFact {
        requested_draws: draws,
        delivered_draws,
        requested_distinct_resources: draws,
        delivered_distinct_resources: delivered,
        repeated_resource_draws: delivered_draws - delivered,
        traditional_frame_bytes: u64::from(delivered_draws) * 16,
        heap_frame_bytes: 16,
        arithmetic: format!(
            "min({draws} requested draws, instance-buffer wall) = {delivered_draws}; min({delivered_draws} delivered draws, {material_wall} material descriptors) = {delivered}"
        ),
    }
}

enum MapOutcome {
    Complete(Result<(), wgpu::BufferAsyncError>),
    Deadline,
}

fn finish_map(state: &Arc<Mutex<Option<MapOutcome>>>, outcome: MapOutcome) {
    if let Ok(mut slot) = state.lock() {
        if slot.is_none() {
            *slot = Some(outcome);
        }
    }
}

fn arm_deadline(state: Arc<Mutex<Option<MapOutcome>>>, deadline_ms: i32) -> Result<(), LabError> {
    let callback = Closure::once(move || finish_map(&state, MapOutcome::Deadline));
    web_sys::window()
        .ok_or_else(|| LabError::Mapping("window is unavailable".to_string()))?
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            deadline_ms,
        )
        .map_err(|error| LabError::Mapping(format!("could not arm deadline: {error:?}")))?;
    callback.forget();
    Ok(())
}

async fn wait_for_fence(
    lab: &Rc<RefCell<Lab>>,
    pending: PendingFence,
    generation: u64,
) -> Result<(), LabError> {
    let slice = pending.buffer.slice(..);
    let state = Arc::new(Mutex::new(None));
    let callback_state = Arc::clone(&state);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        finish_map(&callback_state, MapOutcome::Complete(result));
    });
    arm_deadline(Arc::clone(&state), pending.deadline_ms)?;
    loop {
        let current = GENERATION.get();
        if current != generation {
            pending.buffer.unmap();
            return Err(LabError::StaleGeneration {
                observed: generation,
                current,
            });
        }
        let polled = lab.try_borrow().map(|lab| {
            lab.device.poll(wgpu::Maintain::Poll);
            lab.lost_reason()
        });
        match polled {
            Ok(Some(reason)) => {
                pending.buffer.unmap();
                return Err(LabError::DeviceLost(reason));
            }
            Ok(None) => {}
            Err(_) => {
                yield_to_browser().await?;
                continue;
            }
        }
        let outcome = state.lock().ok().and_then(|mut slot| slot.take());
        match outcome {
            Some(MapOutcome::Complete(Ok(()))) => {
                let bytes = slice.get_mapped_range();
                let mapped_len = bytes.len();
                if mapped_len != 4 {
                    drop(bytes);
                    pending.buffer.unmap();
                    return Err(LabError::Mapping(format!(
                        "completion fence mapped {} bytes instead of 4",
                        mapped_len
                    )));
                }
                drop(bytes);
                pending.buffer.unmap();
                return Ok(());
            }
            Some(MapOutcome::Complete(Err(error))) => {
                return Err(LabError::Mapping(error.to_string()));
            }
            Some(MapOutcome::Deadline) => {
                pending.buffer.unmap();
                return Err(LabError::Deadline(pending.deadline_ms));
            }
            None => yield_to_browser().await?,
        }
    }
}

thread_local! {
    static LAB: RefCell<Option<Rc<RefCell<Lab>>>> = const { RefCell::new(None) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

fn current_lab() -> Result<Rc<RefCell<Lab>>, LabError> {
    LAB.with(|slot| {
        slot.try_borrow()
            .map_err(|_| LabError::BorrowConflict("reading the benchmark lab slot"))?
            .as_ref()
            .cloned()
            .ok_or_else(|| LabError::Capability("heap lab is not initialized".to_string()))
    })
}

fn serialize<T: Serialize>(value: &T) -> Result<String, LabError> {
    serde_json::to_string(value).map_err(|error| LabError::Serialization(error.to_string()))
}

/// Initializes the WebGL2 heap and renders the 16-draw heap baseline immediately.
///
/// # Errors
///
/// Returns a JavaScript error when WebGL2 capabilities, allocation, shader creation, or initial
/// rendering fail.
#[wasm_bindgen]
pub async fn start_heap(canvas: web_sys::HtmlCanvasElement) -> Result<String, JsValue> {
    let generation = GENERATION.get().wrapping_add(1);
    GENERATION.set(generation);
    let (mut lab, facts) = Lab::new(canvas).await.map_err(JsValue::from)?;
    let current = GENERATION.get();
    if current != generation {
        return Err(JsValue::from(LabError::StaleGeneration {
            observed: generation,
            current,
        }));
    }
    lab.render_draws(DrawMode::Heap, DRAW_STEPS[0], 1, false)
        .map_err(JsValue::from)?;
    LAB.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| LabError::BorrowConflict("publishing the benchmark lab"))?;
        *slot = Some(Rc::new(RefCell::new(lab)));
        Ok::<_, LabError>(())
    })
    .map_err(JsValue::from)?;
    serialize(&facts).map_err(JsValue::from)
}

/// Invalidates every in-flight mapped-fence wait without removing the initialized device.
#[wasm_bindgen]
pub fn cancel_heap_measurement() {
    GENERATION.set(GENERATION.get().wrapping_add(1));
}

/// Selects and immediately submits one Benchmark B frame.
///
/// # Errors
///
/// Returns a JavaScript error for an unknown step or mode, device loss, or surface failure.
#[wasm_bindgen]
pub fn render_heap_step_json(mode: &str, draws: u32) -> Result<String, JsValue> {
    let mode = DrawMode::parse(mode).map_err(JsValue::from)?;
    if !DRAW_STEPS.contains(&draws) {
        return Err(JsValue::from(LabError::UnknownStep(draws)));
    }
    let generation = GENERATION.get().wrapping_add(1);
    GENERATION.set(generation);
    let lab = current_lab().map_err(JsValue::from)?;
    let report = {
        let mut lab = lab
            .try_borrow_mut()
            .map_err(|_| LabError::BorrowConflict("rendering a benchmark selection"))
            .map_err(JsValue::from)?;
        lab.render_draws(mode, draws, 1, false)
            .map_err(JsValue::from)?;
        lab.draw_report(generation, mode, draws)
    };
    serialize(&report).map_err(JsValue::from)
}

/// Measures one adaptively repeated Benchmark A batch through an ordered mapped fence.
///
/// # Errors
///
/// Returns a JavaScript error for invalid mode/repeats, cancellation, device loss, timeout, or
/// mapping failure.
#[wasm_bindgen]
pub async fn measure_heap_fetch_json(mode: &str, repeats: u32) -> Result<String, JsValue> {
    let mode = FetchMode::parse(mode).map_err(JsValue::from)?;
    if !(1..=4_096).contains(&repeats) {
        return Err(JsValue::from(LabError::InvalidRepeat(repeats)));
    }
    let generation = GENERATION.get();
    let lab = current_lab().map_err(JsValue::from)?;
    let started = performance_now();
    let pending = {
        let mut borrowed = lab
            .try_borrow_mut()
            .map_err(|_| LabError::BorrowConflict("submitting a fetch benchmark"))
            .map_err(JsValue::from)?;
        borrowed
            .render_fetch(mode, repeats)
            .map_err(JsValue::from)?
    };
    wait_for_fence(&lab, pending, generation)
        .await
        .map_err(JsValue::from)?;
    let elapsed_ms = performance_now() - started;
    serialize(&Measurement {
        generation,
        benchmark: "A.heap-fetch-overhead",
        mode: mode.label(),
        repeats,
        elapsed_ms,
        normalized_ms: elapsed_ms / f64::from(repeats),
        per_draw_microseconds: None,
        requested_draws: None,
        delivered_draws: None,
        per_frame_cpu_to_gpu_bytes: 0,
        timing_method: "submit-to-4-byte-map-read-fence-wall-clock",
        gpu_timestamp_ms: None,
    })
    .map_err(JsValue::from)
}

/// Measures one adaptively repeated Benchmark B batch through an ordered mapped fence.
///
/// # Errors
///
/// Returns a JavaScript error for invalid mode/step/repeats, cancellation, device loss, timeout,
/// surface failure, or mapping failure.
#[wasm_bindgen]
pub async fn measure_many_draws_json(
    mode: &str,
    draws: u32,
    repeats: u32,
) -> Result<String, JsValue> {
    let mode = DrawMode::parse(mode).map_err(JsValue::from)?;
    if !DRAW_STEPS.contains(&draws) {
        return Err(JsValue::from(LabError::UnknownStep(draws)));
    }
    if !(1..=4_096).contains(&repeats) {
        return Err(JsValue::from(LabError::InvalidRepeat(repeats)));
    }
    let generation = GENERATION.get();
    let lab = current_lab().map_err(JsValue::from)?;
    let started = performance_now();
    let pending = {
        let mut borrowed = lab
            .try_borrow_mut()
            .map_err(|_| LabError::BorrowConflict("submitting a draw benchmark"))
            .map_err(JsValue::from)?;
        borrowed
            .render_draws(mode, draws, repeats, true)
            .map_err(JsValue::from)?
            .ok_or_else(|| JsValue::from_str("Benchmark B did not create a completion fence"))?
    };
    wait_for_fence(&lab, pending, generation)
        .await
        .map_err(JsValue::from)?;
    let elapsed_ms = performance_now() - started;
    let borrowed = lab
        .try_borrow()
        .map_err(|_| LabError::BorrowConflict("reporting a draw benchmark"))
        .map_err(JsValue::from)?;
    let report = borrowed.draw_report(generation, mode, draws);
    let normalized_ms = elapsed_ms / f64::from(repeats);
    let per_draw_microseconds = (report.delivered_draws > 0)
        .then(|| normalized_ms * 1_000.0 / f64::from(report.delivered_draws));
    serialize(&Measurement {
        generation,
        benchmark: "B.many-small-draws",
        mode: mode.label(),
        repeats,
        elapsed_ms,
        normalized_ms,
        per_draw_microseconds,
        requested_draws: Some(draws),
        delivered_draws: Some(report.delivered_draws),
        per_frame_cpu_to_gpu_bytes: report.per_frame_cpu_to_gpu_bytes,
        timing_method: "CPU encode-and-write plus submit-to-4-byte-map-read-fence wall clock",
        gpu_timestamp_ms: None,
    })
    .map_err(JsValue::from)
}
