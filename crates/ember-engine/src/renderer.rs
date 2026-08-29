//! GPU layer. ATW-first architecture (docs/atw-first-rendering.md, stage A):
//! the SCENE pass renders the world into an offscreen `SceneFrame`
//! (color + depth) and never touches the swapchain; the PRESENTER pass owns
//! presentation and warps the newest SceneFrame onto the surface — today an
//! identity blit, later rotation-only / depth-aware reprojection. UI will
//! composite in the presenter, never in the scene. The SceneFrame may be a
//! different resolution than the canvas (`scene_scale` — dynamic resolution
//! comes for free because the presenter resamples anyway).

use std::sync::Arc;

use glam::{Mat4, Vec3};
use winit::window::Window;

/// A vertex of a registered mesh (matches the built-in cube's layout).
#[derive(Clone, Copy, Debug)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// Texture coordinate; [0,0] for untextured meshes.
    pub uv: [f32; 2],
}

/// A triangle-list mesh the game registers at startup (EngineConfig.meshes).
/// Mesh id 0 is always the built-in unit cube; registered meshes get ids
/// 1..=N in order.
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<MeshVertex>,
    /// Optional RGBA8 texture sampled via the mesh's UVs. None = a shared
    /// 1x1 white pixel, i.e. the classic instance-color-only look.
    pub texture: Option<TextureData>,
}

/// Raw RGBA8 pixels for a mesh texture.
#[derive(Clone, Debug)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
impl TextureData {
    /// Decode PNG bytes (e.g. from include_bytes!) into RGBA8.
    pub fn from_png_bytes(bytes: &[u8]) -> Result<Self, String> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        Ok(Self { width: img.width(), height: img.height(), rgba8: img.into_raw() })
    }
}

/// One draw unit: a colored mesh instance (default: the unit cube),
/// optionally rotated around Y.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub position: Vec3,
    pub scale: Vec3,
    pub color: Vec3,
    /// Rotation around the Y axis, radians. 0 = axis-aligned.
    pub yaw: f32,
    /// Mesh id: 0 = built-in cube, 1..=N = EngineConfig.meshes entries.
    pub mesh: u32,
}

impl Instance {
    pub fn new(position: Vec3, scale: Vec3, color: Vec3) -> Self {
        Self { position, scale, color, yaw: 0.0, mesh: 0 }
    }

    pub fn with_yaw(mut self, yaw: f32) -> Self {
        self.yaw = yaw;
        self
    }

    pub fn with_mesh(mut self, mesh: u32) -> Self {
        self.mesh = mesh;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov_y_deg: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 32.0, 40.0),
            target: Vec3::ZERO,
            fov_y_deg: 50.0,
        }
    }
}

impl Camera {
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        // glam's perspective_rh targets wgpu's 0..1 clip depth.
        Mat4::perspective_rh(self.fov_y_deg.to_radians(), aspect.max(0.01), 0.1, 500.0)
            * Mat4::look_at_rh(self.eye, self.target, Vec3::Y)
    }
}

/// Everything the game wants drawn this frame.
pub struct Frame {
    pub camera: Camera,
    pub instances: Vec<Instance>,
}

impl Default for Frame {
    fn default() -> Self {
        Self { camera: Camera::default(), instances: Vec::new() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    pos: [f32; 3],
    scale: [f32; 3],
    color: [f32; 3],
    yaw: f32,
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Post-tonemap LDR, as the ATW doc prescribes: the presenter warps LDR.
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The offscreen render target the scene pass draws into and the presenter
/// samples from. Depth is kept as a first-class texture (needed by warp
/// stage C and by SSAO/TAA later).
struct SceneTargets {
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    width: u32,
    height: u32,
}

/// GPU-side mesh: vertex buffer, vertex count, texture bind group.
struct MeshEntry {
    buf: wgpu::Buffer,
    count: u32,
    bind: wgpu::BindGroup,
}

/// Owns the GPU. The only place in the engine that touches wgpu.
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    // Scene pass.
    scene: SceneTargets,
    scene_scale: f32,
    scene_pipeline: wgpu::RenderPipeline,
    camera_buf: wgpu::Buffer,
    camera_bind: wgpu::BindGroup,
    /// Per mesh id (0 = built-in cube): buffer, count, texture bind group.
    meshes: Vec<MeshEntry>,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    // Presenter.
    present_pipeline: wgpu::RenderPipeline,
    present_layout: wgpu::BindGroupLayout,
    present_bind: wgpu::BindGroup,
    present_sampler: wgpu::Sampler,
    /// Set when the surface format itself is non-sRGB (WebGPU canvases):
    /// the present pass renders into an sRGB reinterpreting view.
    surface_view_format: Option<wgpu::TextureFormat>,
    // Pipeline layouts kept so WGSL hot-reload can rebuild pipelines
    // (native-only reader, hence unused on wasm).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    scene_pipeline_layout: wgpu::PipelineLayout,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    present_pipeline_layout: wgpu::PipelineLayout,
    #[cfg(not(target_arch = "wasm32"))]
    shader_reload: ShaderReload,
}

/// Tracks shader sources on disk for native WGSL hot-reload.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct ShaderReload {
    frame: u32,
    scene_mtime: Option<std::time::SystemTime>,
    present_mtime: Option<std::time::SystemTime>,
}

impl Renderer {
    pub async fn new(window: Arc<Window>, extra_meshes: Vec<MeshData>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window)
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no compatible GPU adapter found");
        let info = adapter.get_info();
        tracing::info!(gpu = %info.name, backend = ?info.backend, "GPU adapter selected");

        // On the web the GL/WebGL2 fallback path has tighter limits; request
        // only what that path can give so one code path runs everywhere.
        let required_limits = if cfg!(target_arch = "wasm32") {
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
        } else {
            wgpu::Limits::default()
        };
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("main device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("failed to create device");

        let caps = surface.get_capabilities(&adapter);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface not supported by adapter");
        config.present_mode = wgpu::PresentMode::AutoVsync;
        // The presenter writes LINEAR light (sampled from the sRGB
        // SceneFrame), so the swapchain must sRGB-encode on write or the
        // whole image displays gamma-crushed dark. Default format order is
        // driver/browser-defined and often non-sRGB (always, on the web).
        let mut surface_view_format: Option<wgpu::TextureFormat> = None;
        if !config.format.is_srgb() {
            if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
                config.format = srgb;
            } else {
                // WebGPU canvases expose only non-sRGB formats; render the
                // present pass into an sRGB *view* of the surface instead.
                let srgb = config.format.add_srgb_suffix();
                if srgb != config.format {
                    config.view_formats.push(srgb);
                    surface_view_format = Some(srgb);
                }
            }
        }
        surface.configure(&device, &config);

        let scene_scale = 1.0;
        let scene = create_scene_targets(&device, &config, scene_scale);

        // ---- scene pass ----
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        use wgpu::util::DeviceExt;

        // Per-mesh textures: group(1) of the scene pass. Meshes without a
        // texture share a 1x1 white pixel, keeping the instance-color look.
        let mesh_tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let mesh_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mesh sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = TextureData { width: 1, height: 1, rgba8: vec![255, 255, 255, 255] };

        let mut mesh_textures: Vec<Option<TextureData>> = std::iter::once(None)
            .chain(extra_meshes.iter().map(|m| m.texture.clone()))
            .collect();
        // Native debug aid: EMBER_DEBUG_TEXTURE=<path.png> textures the
        // built-in cube (mesh 0) without any game-side changes.
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(path) = std::env::var("EMBER_DEBUG_TEXTURE") {
            match load_png_rgba8(&path) {
                Ok(t) => {
                    tracing::info!(path, w = t.width, h = t.height, "debug texture on mesh 0");
                    mesh_textures[0] = Some(t);
                }
                Err(e) => tracing::error!(path, error = %e, "EMBER_DEBUG_TEXTURE load failed"),
            }
        }

        let make_bind = |data: &TextureData, label: &str| -> wgpu::BindGroup {
            let tex = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: data.width,
                        height: data.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &data.rgba8,
            );
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &mesh_tex_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&mesh_sampler),
                    },
                ],
            })
        };

        let mut meshes: Vec<MeshEntry> = Vec::with_capacity(1 + extra_meshes.len());
        let cube_vertices = cube_vertices();
        meshes.push(MeshEntry {
            buf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh 0 (cube)"),
                contents: bytemuck::cast_slice(&cube_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            count: cube_vertices.len() as u32,
            bind: make_bind(mesh_textures[0].as_ref().unwrap_or(&white), "mesh 0 texture"),
        });
        for (i, m) in extra_meshes.iter().enumerate() {
            let verts: Vec<Vertex> = m
                .vertices
                .iter()
                .map(|v| Vertex { pos: v.pos, normal: v.normal, uv: v.uv })
                .collect();
            meshes.push(MeshEntry {
                buf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("mesh {}", i + 1)),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                count: verts.len() as u32,
                bind: make_bind(
                    mesh_textures[i + 1].as_ref().unwrap_or(&white),
                    &format!("mesh {} texture", i + 1),
                ),
            });
        }

        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera layout"),
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
        let camera_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });

        let scene_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("scene pipeline layout"),
                bind_group_layouts: &[&camera_layout, &mesh_tex_layout],
                push_constant_ranges: &[],
            });
        let scene_pipeline = build_scene_pipeline(&device, &scene_pipeline_layout, &shader);

        let instance_cap = 64;
        let instance_buf = create_instance_buf(&device, instance_cap);

        // ---- presenter ----
        let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
        });
        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let present_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("present sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let present_bind =
            create_present_bind(&device, &present_layout, &scene.color_view, &present_sampler);
        let present_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("present pipeline layout"),
                bind_group_layouts: &[&present_layout],
                push_constant_ranges: &[],
            });
        let present_pipeline = build_present_pipeline(
            &device,
            &present_pipeline_layout,
            &present_shader,
            surface_view_format.unwrap_or(config.format),
        );

        Self {
            surface,
            device,
            queue,
            config,
            scene,
            scene_scale,
            scene_pipeline,
            camera_buf,
            camera_bind,
            meshes,
            instance_buf,
            instance_cap,
            present_pipeline,
            present_layout,
            present_bind,
            present_sampler,
            surface_view_format,
            scene_pipeline_layout,
            present_pipeline_layout,
            #[cfg(not(target_arch = "wasm32"))]
            shader_reload: ShaderReload::default(),
        }
    }

    /// Resize only when the size actually changed (cheap to call per frame).
    pub fn resize_if_changed(&mut self, width: u32, height: u32) {
        if width != self.config.width || height != self.config.height {
            self.resize(width, height);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // minimized
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.scene = create_scene_targets(&self.device, &self.config, self.scene_scale);
        self.present_bind = create_present_bind(
            &self.device,
            &self.present_layout,
            &self.scene.color_view,
            &self.present_sampler,
        );
    }

    pub fn render(&mut self, frame: &Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        self.maybe_reload_shaders();

        let surface_tex = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "dropped frame");
                return;
            }
        };
        let surface_view = surface_tex.texture.create_view(&wgpu::TextureViewDescriptor {
            format: self.surface_view_format,
            ..Default::default()
        });

        let aspect = self.scene.width as f32 / self.scene.height.max(1) as f32;
        let vp = frame.camera.view_proj(aspect);
        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::cast_slice(vp.as_ref()));

        // Bucket instances by mesh so each mesh draws with one instanced
        // call over a contiguous range of the shared instance buffer.
        let mut buckets: Vec<Vec<InstanceRaw>> = vec![Vec::new(); self.meshes.len()];
        for i in &frame.instances {
            let m = (i.mesh as usize).min(self.meshes.len() - 1);
            buckets[m].push(InstanceRaw {
                pos: i.position.to_array(),
                scale: i.scale.to_array(),
                color: i.color.to_array(),
                yaw: i.yaw,
            });
        }
        let mut raws: Vec<InstanceRaw> = Vec::with_capacity(frame.instances.len());
        let mut ranges: Vec<(usize, std::ops::Range<u32>)> = Vec::new();
        for (mi, b) in buckets.iter().enumerate() {
            if b.is_empty() {
                continue;
            }
            let start = raws.len() as u32;
            raws.extend_from_slice(b);
            ranges.push((mi, start..raws.len() as u32));
        }
        if raws.len() > self.instance_cap {
            self.instance_cap = raws.len().next_power_of_two();
            self.instance_buf = create_instance_buf(&self.device, self.instance_cap);
        }
        if !raws.is_empty() {
            self.queue
                .write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&raws));
        }

        // Separate command buffers per pass, per the ATW doc's sliced-
        // submission rule: the presenter must never wait behind scene work
        // in a single monolithic submission.
        let mut scene_enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scene encoder"),
            });
        {
            let mut pass = scene_enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.028,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.scene.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        // Kept (not discarded): warp stage C and later SSAO/
                        // TAA read scene depth.
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if !raws.is_empty() {
                pass.set_pipeline(&self.scene_pipeline);
                pass.set_bind_group(0, &self.camera_bind, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                for (mi, range) in &ranges {
                    let mesh = &self.meshes[*mi];
                    pass.set_bind_group(1, &mesh.bind, &[]);
                    pass.set_vertex_buffer(0, mesh.buf.slice(..));
                    pass.draw(0..mesh.count, range.clone());
                }
            }
        }

        let mut present_enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present encoder"),
            });
        {
            let mut pass = present_enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
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
            pass.set_pipeline(&self.present_pipeline);
            pass.set_bind_group(0, &self.present_bind, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue
            .submit([scene_enc.finish(), present_enc.finish()]);
        surface_tex.present();
    }

    /// Native-only WGSL hot-reload: every 60 rendered frames poll the shader
    /// sources on disk and rebuild the affected pipeline when a file changed.
    /// A shader that fails validation is logged and the old pipeline kept.
    #[cfg(not(target_arch = "wasm32"))]
    fn maybe_reload_shaders(&mut self) {
        const SCENE_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shader.wgsl");
        const PRESENT_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/present.wgsl");

        self.shader_reload.frame = self.shader_reload.frame.wrapping_add(1);
        if self.shader_reload.frame % 60 != 0 {
            return;
        }

        if check_mtime(SCENE_SRC, &mut self.shader_reload.scene_mtime) {
            if let Some(module) = self.try_compile(SCENE_SRC, "scene shader (hot-reload)") {
                self.scene_pipeline =
                    build_scene_pipeline(&self.device, &self.scene_pipeline_layout, &module);
                tracing::info!(path = SCENE_SRC, "scene shader hot-reloaded");
            }
        }
        if check_mtime(PRESENT_SRC, &mut self.shader_reload.present_mtime) {
            if let Some(module) = self.try_compile(PRESENT_SRC, "present shader (hot-reload)") {
                let format = self.surface_view_format.unwrap_or(self.config.format);
                self.present_pipeline = build_present_pipeline(
                    &self.device,
                    &self.present_pipeline_layout,
                    &module,
                    format,
                );
                tracing::info!(path = PRESENT_SRC, "present shader hot-reloaded");
            }
        }
    }

    /// Compile WGSL from `path` under a validation error scope. None (plus an
    /// error log) when the file is unreadable or the shader fails validation.
    #[cfg(not(target_arch = "wasm32"))]
    fn try_compile(&self, path: &str, label: &str) -> Option<wgpu::ShaderModule> {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(path, error = %e, "shader hot-reload: read failed");
                return None;
            }
        };
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        if let Some(err) = pollster::block_on(self.device.pop_error_scope()) {
            tracing::error!(path, %err, "shader hot-reload: validation failed; keeping old pipeline");
            return None;
        }
        Some(module)
    }
}

fn create_scene_targets(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    scale: f32,
) -> SceneTargets {
    let width = ((config.width as f32 * scale) as u32).max(1);
    let height = ((config.height as f32 * scale) as u32).max(1);
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene color"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SCENE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene depth"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    SceneTargets {
        color_view: color.create_view(&wgpu::TextureViewDescriptor::default()),
        depth_view: depth.create_view(&wgpu::TextureViewDescriptor::default()),
        width,
        height,
    }
}

fn create_present_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    scene_color: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present bind"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(scene_color),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// Scene render pipeline; shared by startup and WGSL hot-reload.
fn build_scene_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("scene pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<InstanceRaw>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![3 => Float32x3, 4 => Float32x3, 5 => Float32x3, 6 => Float32],
                },
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: SCENE_FORMAT,
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
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

/// Presenter pipeline; shared by startup and WGSL hot-reload.
fn build_present_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("present pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

/// True when the file's mtime differs from `tracked` (which is updated).
/// The first successful stat only records the baseline and reports false.
#[cfg(not(target_arch = "wasm32"))]
fn check_mtime(path: &str, tracked: &mut Option<std::time::SystemTime>) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false };
    let Ok(mtime) = meta.modified() else { return false };
    match tracked {
        Some(prev) if *prev != mtime => {
            *tracked = Some(mtime);
            true
        }
        Some(_) => false,
        None => {
            *tracked = Some(mtime);
            false
        }
    }
}

/// Decode a PNG file into RGBA8 (native debug/tooling path).
#[cfg(not(target_arch = "wasm32"))]
fn load_png_rgba8(path: &str) -> Result<TextureData, String> {
    let img = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    Ok(TextureData { width: img.width(), height: img.height(), rgba8: img.into_raw() })
}

fn create_instance_buf(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instances"),
        size: (cap * std::mem::size_of::<InstanceRaw>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Unit cube centered at the origin, 36 vertices with face normals.
fn cube_vertices() -> Vec<Vertex> {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        // (normal, tangent u, tangent v) per face
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    let mut verts = Vec::with_capacity(36);
    for (n, u, v) in faces {
        let n3 = Vec3::from(n);
        let u3 = Vec3::from(u);
        let v3 = Vec3::from(v);
        let center = n3 * 0.5;
        let corners = [
            center - u3 * 0.5 - v3 * 0.5,
            center + u3 * 0.5 - v3 * 0.5,
            center + u3 * 0.5 + v3 * 0.5,
            center - u3 * 0.5 + v3 * 0.5,
        ];
        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        for idx in [0usize, 1, 2, 0, 2, 3] {
            verts.push(Vertex { pos: corners[idx].to_array(), normal: n, uv: uvs[idx] });
        }
    }
    verts
}
