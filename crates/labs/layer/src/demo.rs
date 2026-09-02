//! WebGL2 surface, pillar resources, and thin consumer renderer.

use std::sync::{Arc, Mutex};

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use wgpu::util::DeviceExt;

use crate::compute::{
    ComputeBuffer, ComputeDevice, DispatchToken, Grid1DLimit, IndexSpace, InputBinding, Kernel,
    KernelDesc, LayerError, OutputBinding,
};
use crate::geometry;
use crate::kernels::{LATTICE_EDGE_KERNEL, LatticeUniform};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

const RENDER_SHADER: &str = r"
struct CameraUniform {
    aspect: f32,
    yaw: f32,
    pitch: f32,
    distance: f32,
}
@group(0) @binding(0) var midpoint_hue_texture: texture_2d<f32>;
@group(0) @binding(1) var orientation_length_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> camera: CameraUniform;

const POSITIONS = array<vec3<f32>, 36>(
    vec3(-1.0,-1.0, 1.0), vec3( 1.0,-1.0, 1.0), vec3( 1.0, 1.0, 1.0),
    vec3(-1.0,-1.0, 1.0), vec3( 1.0, 1.0, 1.0), vec3(-1.0, 1.0, 1.0),
    vec3( 1.0,-1.0,-1.0), vec3(-1.0,-1.0,-1.0), vec3(-1.0, 1.0,-1.0),
    vec3( 1.0,-1.0,-1.0), vec3(-1.0, 1.0,-1.0), vec3( 1.0, 1.0,-1.0),
    vec3( 1.0,-1.0, 1.0), vec3( 1.0,-1.0,-1.0), vec3( 1.0, 1.0,-1.0),
    vec3( 1.0,-1.0, 1.0), vec3( 1.0, 1.0,-1.0), vec3( 1.0, 1.0, 1.0),
    vec3(-1.0,-1.0,-1.0), vec3(-1.0,-1.0, 1.0), vec3(-1.0, 1.0, 1.0),
    vec3(-1.0,-1.0,-1.0), vec3(-1.0, 1.0, 1.0), vec3(-1.0, 1.0,-1.0),
    vec3(-1.0, 1.0, 1.0), vec3( 1.0, 1.0, 1.0), vec3( 1.0, 1.0,-1.0),
    vec3(-1.0, 1.0, 1.0), vec3( 1.0, 1.0,-1.0), vec3(-1.0, 1.0,-1.0),
    vec3(-1.0,-1.0,-1.0), vec3( 1.0,-1.0,-1.0), vec3( 1.0,-1.0, 1.0),
    vec3(-1.0,-1.0,-1.0), vec3( 1.0,-1.0, 1.0), vec3(-1.0,-1.0, 1.0)
);
const NORMALS = array<vec3<f32>, 36>(
    vec3(0.0,0.0,1.0), vec3(0.0,0.0,1.0), vec3(0.0,0.0,1.0),
    vec3(0.0,0.0,1.0), vec3(0.0,0.0,1.0), vec3(0.0,0.0,1.0),
    vec3(0.0,0.0,-1.0), vec3(0.0,0.0,-1.0), vec3(0.0,0.0,-1.0),
    vec3(0.0,0.0,-1.0), vec3(0.0,0.0,-1.0), vec3(0.0,0.0,-1.0),
    vec3(1.0,0.0,0.0), vec3(1.0,0.0,0.0), vec3(1.0,0.0,0.0),
    vec3(1.0,0.0,0.0), vec3(1.0,0.0,0.0), vec3(1.0,0.0,0.0),
    vec3(-1.0,0.0,0.0), vec3(-1.0,0.0,0.0), vec3(-1.0,0.0,0.0),
    vec3(-1.0,0.0,0.0), vec3(-1.0,0.0,0.0), vec3(-1.0,0.0,0.0),
    vec3(0.0,1.0,0.0), vec3(0.0,1.0,0.0), vec3(0.0,1.0,0.0),
    vec3(0.0,1.0,0.0), vec3(0.0,1.0,0.0), vec3(0.0,1.0,0.0),
    vec3(0.0,-1.0,0.0), vec3(0.0,-1.0,0.0), vec3(0.0,-1.0,0.0),
    vec3(0.0,-1.0,0.0), vec3(0.0,-1.0,0.0), vec3(0.0,-1.0,0.0)
);

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) hue: f32,
}

fn texture_coordinate(index: u32) -> vec2<i32> {
    let width = textureDimensions(midpoint_hue_texture).x;
    return vec2<i32>(i32(index % width), i32(index / width));
}

fn camera_turn(point: vec3<f32>) -> vec3<f32> {
    let cosine_yaw = cos(camera.yaw);
    let sine_yaw = sin(camera.yaw);
    let yawed = vec3<f32>(
        point.x * cosine_yaw + point.z * sine_yaw,
        point.y,
        -point.x * sine_yaw + point.z * cosine_yaw
    );
    let cosine_pitch = cos(camera.pitch);
    let sine_pitch = sin(camera.pitch);
    return vec3<f32>(
        yawed.x,
        yawed.y * cosine_pitch - yawed.z * sine_pitch,
        yawed.y * sine_pitch + yawed.z * cosine_pitch
    );
}

@vertex
fn vertex_main(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> VertexOut {
    let coordinate = texture_coordinate(instance);
    let midpoint_hue = textureLoad(midpoint_hue_texture, coordinate, 0);
    let orientation_length = textureLoad(orientation_length_texture, coordinate, 0);
    if (orientation_length.w < 0.0) {
        var invalid: VertexOut;
        invalid.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        invalid.normal = vec3<f32>(0.0, 0.0, 1.0);
        invalid.hue = midpoint_hue.w;
        return invalid;
    }
    let axis = orientation_length.xyz;
    let reference = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(axis.y) > 0.90);
    let side = normalize(cross(reference, axis));
    let upward = cross(axis, side);
    let local = POSITIONS[vertex];
    let world = midpoint_hue.xyz
        + side * local.x * 0.012
        + upward * local.y * 0.012
        + axis * local.z * orientation_length.w * 0.5;
    let normal = normalize(side * NORMALS[vertex].x + upward * NORMALS[vertex].y + axis * NORMALS[vertex].z);
    let view = camera_turn(world * 0.82);
    let depth = camera.distance - view.z;
    var output: VertexOut;
    output.clip = vec4<f32>(view.x * 1.45 / camera.aspect, view.y * 1.45, depth - 0.1, depth);
    output.normal = camera_turn(normal);
    output.hue = midpoint_hue.w;
    return output;
}

fn hue_rgb(hue: f32) -> vec3<f32> {
    let phases = abs(fract(vec3<f32>(hue, hue + 0.6666667, hue + 0.3333333)) * 6.0 - 3.0);
    return 0.20 + 0.80 * clamp(phases - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(-0.35, 0.75, 0.55));
    let diffuse = max(dot(normalize(input.normal), light), 0.0);
    let color = hue_rgb(input.hue) * (0.28 + 0.72 * diffuse);
    return vec4<f32>(color, 1.0);
}
";

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CameraUniform {
    aspect: f32,
    yaw: f32,
    pitch: f32,
    distance: f32,
}

#[derive(Serialize)]
struct DemoFacts {
    adapter: String,
    backend: String,
    vertices: usize,
    edges: usize,
    derived_edge_length: f64,
    cap_circumradius: f64,
    base_vertex_grid: (u32, u32),
    base_edge_grid: (u32, u32),
    pose_limit: Grid1DLimit,
    steps: Vec<StepCapacity>,
    per_frame_cpu_to_gpu_bytes_when_delivered: u32,
    hue_mapping: &'static str,
    projection_validity: &'static str,
    completion: &'static str,
    capabilities: crate::compute::CapabilityFacts,
}

#[derive(Clone, Serialize)]
struct StepCapacity {
    step: u32,
    axes: [u32; 5],
    copies: u64,
    requested_edges: u64,
    output_side: u64,
    output_bytes: u64,
    fifth_range: f64,
    arithmetically_deliverable: bool,
    refusal: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct StepReport {
    requested_step: u32,
    requested_axes: [u32; 5],
    requested_edges: u64,
    delivered_edges: u64,
    shown_step: u32,
    shown_axes: [u32; 5],
    shown_edges: u64,
    instances: u64,
    draw_calls: u32,
    compute_passes: u32,
    per_frame_cpu_to_gpu_bytes: u32,
    output_grid: Option<(u32, u32)>,
    output_bytes: u64,
    visible: &'static str,
    refusal: Option<String>,
}

struct StepResources {
    step: u32,
    axes: [u32; 5],
    edges: u32,
    fifth_range: f32,
    kernel: Kernel,
    render_bind_group: wgpu::BindGroup,
    _poses: ComputeBuffer,
}

/// Surface-backed pillar renderer using the fragment-compute layer.
pub(crate) struct Demo {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    compute: ComputeDevice,
    depth: wgpu::TextureView,
    render_pipeline: wgpu::RenderPipeline,
    render_layout: wgpu::BindGroupLayout,
    camera: wgpu::Buffer,
    base: ComputeBuffer,
    edge_indices: ComputeBuffer,
    steps: Vec<StepCapacity>,
    active: Option<StepResources>,
    lost: Arc<Mutex<Option<String>>>,
    frames: u64,
}

impl Demo {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn new(
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(Self, String), LayerError> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| {
                LayerError::Capability(format!("could not bind WebGL2 canvas: {error}"))
            })?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| LayerError::Capability("no WebGL2 surface adapter".to_string()))?;
        let info = adapter.get_info();
        if info.backend != wgpu::Backend::Gl {
            return Err(LayerError::Capability(format!(
                "requested GL/WebGL2 but wgpu selected {:?}",
                info.backend
            )));
        }
        let required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("layer WebGL2 device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|error| LayerError::Capability(format!("WebGL2 device refused: {error}")))?;
        let lost = Arc::new(Mutex::new(None));
        let surface_lost = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = surface_lost.lock() {
                *slot = Some(format!("{reason:?}: {message}"));
            }
        });
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| LayerError::Capability("surface exposes no format".to_string()))?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            .or_else(|| capabilities.present_modes.first().copied())
            .ok_or_else(|| LayerError::Capability("surface exposes no present mode".to_string()))?;
        let alpha_mode =
            capabilities.alpha_modes.first().copied().ok_or_else(|| {
                LayerError::Capability("surface exposes no alpha mode".to_string())
            })?;
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
        let mut compute = ComputeDevice::new(&adapter, device, queue)?;
        compute.golden_self_test().await?;

        let object = geometry::prism();
        let first_slot: Vec<_> = object
            .vertices
            .iter()
            .map(|point| {
                [
                    point[0] as f32,
                    point[1] as f32,
                    point[2] as f32,
                    point[3] as f32,
                ]
            })
            .collect();
        let second_slot: Vec<_> = object
            .vertices
            .iter()
            .map(|point| [point[4] as f32, 0.0, 0.0, 0.0])
            .collect();
        let base_initial: [&[[f32; 4]]; 2] = [&first_slot, &second_slot];
        let base = compute.create_buffer(
            "pillar base coordinates",
            IndexSpace::Grid1D(1_200),
            2,
            Some(&base_initial),
        )?;
        let edge_values: Vec<_> = object
            .edges
            .iter()
            .map(|edge| [edge.a as f32, edge.b as f32, 0.0, 0.0])
            .collect();
        let edge_initial: [&[[f32; 4]]; 1] = [&edge_values];
        let edge_indices = compute.create_buffer(
            "pillar edge indices",
            IndexSpace::Grid1D(3_000),
            1,
            Some(&edge_initial),
        )?;
        let pose_limit = compute.grid1d_limit(2)?;
        let steps = step_capacities(&object, &pose_limit);

        compute
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = compute
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("pillar thin box renderer"),
                source: wgpu::ShaderSource::Wgsl(RENDER_SHADER.into()),
            });
        let render_layout =
            compute
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("pillar render texture layout"),
                    entries: &[
                        texture_layout_entry(0),
                        texture_layout_entry(1),
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(16),
                            },
                            count: None,
                        },
                    ],
                });
        let camera = compute
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("pillar camera uniform"),
                contents: bytemuck::bytes_of(&CameraUniform {
                    aspect: width as f32 / height as f32,
                    yaw: 0.42,
                    pitch: -0.28,
                    distance: 10.5,
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let pipeline_layout =
            compute
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("pillar render pipeline layout"),
                    bind_group_layouts: &[&render_layout],
                    push_constant_ranges: &[],
                });
        let render_pipeline =
            compute
                .device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("pillar thin box renderer"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vertex_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fragment_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        cull_mode: Some(wgpu::Face::Back),
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
        if let Some(error) = compute.device().pop_error_scope().await {
            return Err(LayerError::Pipeline(format!(
                "vertex-texture consumer pipeline failed: {error}"
            )));
        }
        let depth = create_depth(compute.device(), width, height);
        let facts = DemoFacts {
            adapter: if info.name.is_empty() {
                format!("vendor {:#06x}", info.vendor)
            } else {
                info.name
            },
            backend: format!("{:?}", info.backend),
            vertices: object.vertices.len(),
            edges: object.edges.len(),
            derived_edge_length: object.edge_length,
            cap_circumradius: object.cap_circumradius,
            base_vertex_grid: IndexSpace::Grid1D(1_200).rect(),
            base_edge_grid: IndexSpace::Grid1D(3_000).rect(),
            pose_limit: pose_limit.clone(),
            steps: steps.clone(),
            per_frame_cpu_to_gpu_bytes_when_delivered: 64,
            hue_mapping: "clamp(midpoint post-rotation x5 / (2 * runtime symmetric lattice range) + 0.5, 0, 1)",
            projection_validity: "per vertex: d5-x5 > 0.05 and d4-projected_x4 > 0.05; an edge with either invalid endpoint remains shown/submitted but its box is clipped",
            completion: "ordered submissions for rendering; 4-byte mapped-copy fence only for explicit complete(), texture mapping for read()",
            capabilities: compute.facts().clone(),
        };
        let facts = serde_json::to_string(&facts)
            .map_err(|error| LayerError::Resource(format!("could not encode facts: {error}")))?;
        Ok((
            Self {
                surface,
                config,
                compute,
                depth,
                render_pipeline,
                render_layout,
                camera,
                base,
                edge_indices,
                steps,
                active: None,
                lost,
                frames: 0,
            },
            facts,
        ))
    }

    pub(crate) async fn set_step(&mut self, step: u32) -> Result<StepReport, LayerError> {
        let capacity = self
            .steps
            .iter()
            .find(|capacity| capacity.step == step)
            .cloned()
            .ok_or_else(|| LayerError::Resource(format!("lattice step {step} is not offered")))?;
        if step == 0 {
            self.active = None;
            return Ok(self.step_report(&capacity, None));
        }
        if !capacity.arithmetically_deliverable {
            return Ok(self.step_report(&capacity, capacity.refusal.clone()));
        }
        match self.build_step(&capacity).await {
            Ok(resources) => {
                self.active = Some(resources);
                Ok(self.step_report(&capacity, None))
            }
            Err(error) => Ok(self.step_report(
                &capacity,
                Some(format!(
                    "runtime allocation or registration refused after arithmetic admission: {error}"
                )),
            )),
        }
    }

    async fn build_step(&self, capacity: &StepCapacity) -> Result<StepResources, LayerError> {
        let edges = u32::try_from(capacity.requested_edges).map_err(|_| {
            LayerError::Resource(format!(
                "{} requested edges exceed the dialect u32 index",
                capacity.requested_edges
            ))
        })?;
        let index_space = IndexSpace::Grid1D(edges);
        let poses = self
            .compute
            .create_buffer("lattice edge poses", index_space, 2, None)?;
        let inputs = [
            InputBinding {
                accessor: "load_edge",
                buffer: &self.edge_indices,
                slot: 0,
            },
            InputBinding {
                accessor: "load_base_four",
                buffer: &self.base,
                slot: 0,
            },
            InputBinding {
                accessor: "load_base_fifth",
                buffer: &self.base,
                slot: 1,
            },
        ];
        let outputs = [
            OutputBinding {
                field: "midpoint_hue",
                buffer: &poses,
                slot: 0,
            },
            OutputBinding {
                field: "orientation_length",
                buffer: &poses,
                slot: 1,
            },
        ];
        let kernel = self
            .compute
            .create_kernel(KernelDesc {
                name: "lattice_edges",
                body: LATTICE_EDGE_KERNEL,
                index_space,
                inputs: &inputs,
                outputs: &outputs,
                uniform_type: "LatticeUniform",
                uniform_size: 48,
            })
            .await?;
        let render_bind_group = create_render_bind_group(
            self.compute.device(),
            &self.render_layout,
            &poses,
            &self.camera,
        )?;
        Ok(StepResources {
            step: capacity.step,
            axes: capacity.axes,
            edges,
            fifth_range: capacity.fifth_range as f32,
            kernel,
            render_bind_group,
            _poses: poses,
        })
    }

    fn step_report(&self, capacity: &StepCapacity, refusal: Option<String>) -> StepReport {
        let delivered = refusal.is_none()
            && (capacity.step == 0
                || self
                    .active
                    .as_ref()
                    .is_some_and(|resources| resources.step == capacity.step));
        let shown = self.active.as_ref();
        let requested_delivered = if delivered {
            capacity.requested_edges
        } else {
            0
        };
        let shown_edges = shown.map_or(0, |resources| u64::from(resources.edges));
        StepReport {
            requested_step: capacity.step,
            requested_axes: capacity.axes,
            requested_edges: capacity.requested_edges,
            delivered_edges: requested_delivered,
            shown_step: shown.map_or(0, |resources| resources.step),
            shown_axes: shown.map_or([0; 5], |resources| resources.axes),
            shown_edges,
            instances: shown_edges,
            draw_calls: u32::from(shown.is_some()),
            compute_passes: u32::from(shown.is_some()),
            per_frame_cpu_to_gpu_bytes: if shown.is_some() { 64 } else { 0 },
            output_grid: shown.map(|resources| IndexSpace::Grid1D(resources.edges).rect()),
            output_bytes: capacity.output_bytes,
            visible: "not counted without readback: projection-valid subset only; shown/submitted includes every pole-discarded edge",
            refusal,
        }
    }

    pub(crate) fn frame(&mut self, time_seconds: f32) -> Result<u64, LayerError> {
        self.submit_frame(time_seconds).map(|(frames, _)| frames)
    }

    pub(crate) async fn probe_frame(&mut self, time_seconds: f32) -> Result<u64, LayerError> {
        let (frames, token) = self.submit_frame(time_seconds)?;
        if let Some(token) = token {
            self.compute.complete(token).await?;
        }
        Ok(frames)
    }

    fn submit_frame(
        &mut self,
        time_seconds: f32,
    ) -> Result<(u64, Option<DispatchToken>), LayerError> {
        if let Some(reason) = self.lost.lock().ok().and_then(|reason| reason.clone()) {
            return Err(LayerError::DeviceLost(reason));
        }
        let theta_one = 0.4 * time_seconds;
        let theta_two = (1.0 + 5.0_f32.sqrt()) * 0.5 * theta_one;
        let token = self
            .active
            .as_ref()
            .map(|active| {
                let uniform = LatticeUniform {
                    theta_one,
                    theta_two,
                    pole_five: 8.0,
                    pole_four: 8.0,
                    axis_counts: [
                        active.axes[0] as f32,
                        active.axes[1] as f32,
                        active.axes[2] as f32,
                        active.axes[3] as f32,
                    ],
                    axis_five: active.axes[4] as f32,
                    spacing: geometry::LATTICE_SPACING as f32,
                    fifth_range: active.fifth_range,
                    pole_epsilon: 0.05,
                };
                self.compute
                    .dispatch(&active.kernel, bytemuck::bytes_of(&uniform))
            })
            .transpose()?;
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(self.compute.device(), &self.config);
                return Ok((self.frames, token));
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok((self.frames, token)),
            Err(error) => {
                return Err(LayerError::DeviceLost(format!(
                    "surface presentation failed: {error}"
                )));
            }
        };
        let camera = CameraUniform {
            aspect: self.config.width as f32 / self.config.height as f32,
            yaw: 0.42,
            pitch: -0.28,
            distance: 10.5,
        };
        if self.active.is_some() {
            self.compute
                .queue()
                .write_buffer(&self.camera, 0, bytemuck::bytes_of(&camera));
        }
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.compute
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pillar render frame"),
                });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pillar thin boxes"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.012,
                            b: 0.028,
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
            pass.set_pipeline(&self.render_pipeline);
            if let Some(active) = &self.active {
                pass.set_bind_group(0, &active.render_bind_group, &[]);
                pass.draw(0..36, 0..active.edges);
            }
        }
        self.compute.queue().submit([encoder.finish()]);
        frame.present();
        self.frames = self.frames.saturating_add(1);
        Ok((self.frames, token))
    }
}

fn step_capacities(object: &geometry::Prism, limit: &Grid1DLimit) -> Vec<StepCapacity> {
    geometry::lattice_steps()
        .into_iter()
        .enumerate()
        .map(|(step, axes)| {
            let requested_edges = geometry::lattice_edge_count(axes);
            let output_side = if requested_edges == 0 {
                0
            } else {
                requested_edges.saturating_sub(1).isqrt().saturating_add(1)
            };
            let output_bytes = limit.required_bytes(requested_edges).unwrap_or(u64::MAX);
            let mut reasons = Vec::new();
            if requested_edges > limit.dimension_elements {
                reasons.push(format!(
                    "square output side {output_side} exceeds this device texture wall of {} texels per side",
                    limit.dimension_elements.isqrt()
                ));
            }
            if requested_edges > limit.byte_budget_elements {
                reasons.push(format!(
                    "two square-padded RGBA32F slots need {output_bytes} bytes; the layer/device aggregate ceiling is {} bytes",
                    limit.aggregate_byte_ceiling
                ));
            }
            if requested_edges > limit.address_elements {
                reasons.push(format!(
                    "{requested_edges} invocations exceed the dialect u32 address ceiling of {}",
                    limit.address_elements
                ));
            }
            let refusal = (!reasons.is_empty()).then(|| reasons.join("; "));
            StepCapacity {
                step: u32::try_from(step).expect("the finite lattice ladder fits u32"),
                axes,
                copies: geometry::lattice_copy_count(axes),
                requested_edges,
                output_side,
                output_bytes,
                fifth_range: geometry::lattice_fifth_range(object, axes),
                arithmetically_deliverable: step == 0 || refusal.is_none(),
                refusal,
            }
        })
        .collect()
}

fn create_render_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    poses: &ComputeBuffer,
    camera: &wgpu::Buffer,
) -> Result<wgpu::BindGroup, LayerError> {
    Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("lattice render textures"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(poses.as_vertex_texture(0)?),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(poses.as_vertex_texture(1)?),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: camera.as_entire_binding(),
            },
        ],
    }))
}

fn texture_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("pillar depth"),
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
