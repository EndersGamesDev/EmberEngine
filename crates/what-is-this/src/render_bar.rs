//! Surface-backed WebGPU progress rendering owned by the diagnostic page.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use wgpu::util::DeviceExt;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

const SHADER: &str = r#"
struct BarUniform {
    progress: f32,
    aspect: f32,
    padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> bar: BarUniform;

const POSITIONS = array<vec3<f32>, 36>(
    vec3(-1.0, -1.0,  1.0), vec3( 1.0, -1.0,  1.0), vec3( 1.0,  1.0,  1.0),
    vec3(-1.0, -1.0,  1.0), vec3( 1.0,  1.0,  1.0), vec3(-1.0,  1.0,  1.0),
    vec3( 1.0, -1.0, -1.0), vec3(-1.0, -1.0, -1.0), vec3(-1.0,  1.0, -1.0),
    vec3( 1.0, -1.0, -1.0), vec3(-1.0,  1.0, -1.0), vec3( 1.0,  1.0, -1.0),
    vec3( 1.0, -1.0,  1.0), vec3( 1.0, -1.0, -1.0), vec3( 1.0,  1.0, -1.0),
    vec3( 1.0, -1.0,  1.0), vec3( 1.0,  1.0, -1.0), vec3( 1.0,  1.0,  1.0),
    vec3(-1.0, -1.0, -1.0), vec3(-1.0, -1.0,  1.0), vec3(-1.0,  1.0,  1.0),
    vec3(-1.0, -1.0, -1.0), vec3(-1.0,  1.0,  1.0), vec3(-1.0,  1.0, -1.0),
    vec3(-1.0,  1.0,  1.0), vec3( 1.0,  1.0,  1.0), vec3( 1.0,  1.0, -1.0),
    vec3(-1.0,  1.0,  1.0), vec3( 1.0,  1.0, -1.0), vec3(-1.0,  1.0, -1.0),
    vec3(-1.0, -1.0, -1.0), vec3( 1.0, -1.0, -1.0), vec3( 1.0, -1.0,  1.0),
    vec3(-1.0, -1.0, -1.0), vec3( 1.0, -1.0,  1.0), vec3(-1.0, -1.0,  1.0),
);

const NORMALS = array<vec3<f32>, 36>(
    vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 1.0),
    vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 1.0),
    vec3(0.0, 0.0,-1.0), vec3(0.0, 0.0,-1.0), vec3(0.0, 0.0,-1.0),
    vec3(0.0, 0.0,-1.0), vec3(0.0, 0.0,-1.0), vec3(0.0, 0.0,-1.0),
    vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0),
    vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0),
    vec3(-1.0,0.0, 0.0), vec3(-1.0,0.0, 0.0), vec3(-1.0,0.0, 0.0),
    vec3(-1.0,0.0, 0.0), vec3(-1.0,0.0, 0.0), vec3(-1.0,0.0, 0.0),
    vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0),
    vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0),
    vec3(0.0,-1.0, 0.0), vec3(0.0,-1.0, 0.0), vec3(0.0,-1.0, 0.0),
    vec3(0.0,-1.0, 0.0), vec3(0.0,-1.0, 0.0), vec3(0.0,-1.0, 0.0),
);

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
}

fn turn(point: vec3<f32>) -> vec3<f32> {
    let cy = cos(0.48);
    let sy = sin(0.48);
    let yawed = vec3(point.x * cy + point.z * sy, point.y, -point.x * sy + point.z * cy);
    let cx = cos(-0.34);
    let sx = sin(-0.34);
    return vec3(yawed.x, yawed.y * cx - yawed.z * sx, yawed.y * sx + yawed.z * cx);
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> VertexOut {
    let amount = clamp(bar.progress, 0.0, 1.0);
    var scale = vec3(1.58, 0.34, 0.34);
    var center = vec3(0.0, 0.0, 0.0);
    var color = vec3(0.08, 0.20, 0.14);
    if (instance == 1u) {
        let half_width = max(0.002, 1.50 * amount);
        scale = vec3(half_width, 0.255, 0.255);
        center = vec3(-1.50 + half_width, 0.0, 0.0);
        color = vec3(0.18, 0.88, 0.48);
    } else if (instance == 2u) {
        scale = vec3(0.035, 0.42, 0.42);
        center = vec3(-1.50 + 3.0 * amount, 0.0, 0.0);
        color = vec3(1.0, 0.67, 0.24);
    }
    let world = turn(POSITIONS[vertex] * scale + center);
    let normal = turn(NORMALS[vertex]);
    let perspective = 1.0 + world.z * 0.10;
    var output: VertexOut;
    output.clip = vec4(world.x * 0.57, world.y * 1.28, 0.52 + world.z * 0.08, perspective);
    output.normal = normal;
    output.color = color;
    return output;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let light = normalize(vec3(-0.35, 0.80, 0.48));
    let diffuse = max(dot(normal, light), 0.0);
    let rim = 0.10 * pow(1.0 - abs(normal.z), 2.0);
    let lit = input.color * (0.30 + 0.70 * diffuse) + vec3(rim);
    return vec4(lit, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BarUniform {
    progress: f32,
    aspect: f32,
    padding: [f32; 2],
}

#[derive(Serialize)]
struct RenderFacts {
    adapter_identity: String,
    surface_format: String,
    width: u32,
    height: u32,
    device_policy: &'static str,
}

struct BarRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    lost: Arc<Mutex<Option<String>>>,
    frames_presented: u32,
}

impl BarRenderer {
    #[allow(clippy::too_many_lines)]
    async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<(Self, RenderFacts), String> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| format!("could not bind WebGPU to the progress canvas: {error}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                "the browser exposed WebGPU but refused a canvas-compatible adapter".to_string()
            })?;
        let info = adapter.get_info();
        let required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("what-is-this 3D progress device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|error| format!("progress renderer requestDevice was refused: {error}"))?;
        let lost = Arc::new(Mutex::new(None));
        let lost_callback = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = lost_callback.lock() {
                *slot = Some(format!(
                    "progress renderer WebGPU device lost ({reason:?}): {message}"
                ));
            }
        });
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| "canvas surface exposed no renderable formats".to_string())?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            .or_else(|| capabilities.present_modes.first().copied())
            .ok_or_else(|| "canvas surface exposed no presentation modes".to_string())?;
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .ok_or_else(|| "canvas surface exposed no alpha modes".to_string())?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
        let depth = create_depth(&device, width, height);
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("what-is-this 3D progress shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("what-is-this 3D progress uniform"),
            contents: bytemuck::bytes_of(&BarUniform {
                progress: 0.0,
                aspect: width as f32 / height as f32,
                padding: [0.0; 2],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("what-is-this 3D progress bind layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("what-is-this 3D progress bind group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("what-is-this 3D progress pipeline layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("what-is-this 3D progress pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
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
        });
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!(
                "3D progress shader or pipeline validation failed during warmup: {error}"
            ));
        }
        let adapter_identity = if info.name.is_empty() {
            format!("vendor {:#06x}, {:?}", info.vendor, info.device_type)
        } else {
            format!("{} ({:?})", info.name, info.device_type)
        };
        let facts = RenderFacts {
            adapter_identity,
            surface_format: format!("{format:?}"),
            width,
            height,
            device_policy: "dedicated render device; compute retains its separate surface-free device",
        };
        Ok((
            Self {
                surface,
                device,
                queue,
                config,
                depth,
                pipeline,
                uniform,
                bind_group,
                lost,
                frames_presented: 0,
            },
            facts,
        ))
    }

    fn lost_reason(&self) -> Option<String> {
        self.lost.lock().ok().and_then(|reason| reason.clone())
    }

    fn frame(&mut self, progress: f64) -> Result<u32, String> {
        if let Some(reason) = self.lost_reason() {
            return Err(reason);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(self.frames_presented);
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(self.frames_presented),
            Err(error) => return Err(format!("progress canvas presentation failed: {error}")),
        };
        let uniform = BarUniform {
            progress: progress.clamp(0.0, 1.0) as f32,
            aspect: self.config.width as f32 / self.config.height as f32,
            padding: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("what-is-this 3D progress frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("what-is-this 3D progress pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.018,
                            g: 0.050,
                            b: 0.032,
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
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..36, 0..3);
        }
        self.queue.submit([encoder.finish()]);
        frame.present();
        self.frames_presented = self.frames_presented.saturating_add(1);
        Ok(self.frames_presented)
    }
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("what-is-this 3D progress depth"),
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

thread_local! {
    static RENDERER: RefCell<Option<BarRenderer>> = const { RefCell::new(None) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

pub(crate) fn reset() {
    GENERATION.set(GENERATION.get().wrapping_add(1));
    RENDERER.with_borrow_mut(|slot| *slot = None);
}

pub(crate) async fn initialize(canvas: web_sys::HtmlCanvasElement) -> Result<String, String> {
    let generation = GENERATION.get();
    let (renderer, facts) = BarRenderer::new(canvas).await?;
    if GENERATION.get() != generation {
        return Err("3D progress initialization completed after its run was replaced".to_string());
    }
    let json = serde_json::to_string(&facts)
        .map_err(|error| format!("could not encode progress renderer facts: {error}"))?;
    RENDERER.with_borrow_mut(|slot| *slot = Some(renderer));
    Ok(json)
}

pub(crate) fn frame(progress: f64) -> Result<u32, String> {
    RENDERER.with_borrow_mut(|slot| {
        slot.as_mut()
            .ok_or_else(|| "3D progress renderer is not initialized".to_string())?
            .frame(progress)
    })
}
