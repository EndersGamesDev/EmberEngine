//! GPU layer for the ATW-first architecture.
//!
//! In docs/atw-first-rendering.md stage A,
//! the SCENE pass renders the world into an offscreen `SceneFrame`
//! (color + depth) and never touches the swapchain; the PRESENTER pass owns
//! presentation and warps the newest `SceneFrame` onto the surface — today an
//! identity blit, later rotation-only / depth-aware reprojection. UI will
//! composite in the presenter, never in the scene. The `SceneFrame` may be a
//! different resolution than the canvas (`scene_scale` — dynamic resolution
//! comes for free because the presenter resamples anyway).

// GPU APIs use u32 dimensions/counts and f32 scales, while Rust collections and winit use wider types.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::OcclusionField;
use crate::environment::{Environment, Particle};

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

impl TextureData {
    /// Decode PNG bytes (e.g. from `include_bytes`!) into RGBA8. Works on all
    /// targets — wasm games embed their texture bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when `bytes` are not a valid PNG image.
    pub fn from_png_bytes(bytes: &[u8]) -> Result<Self, String> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        Ok(Self {
            width: img.width(),
            height: img.height(),
            rgba8: img.into_raw(),
        })
    }
}

impl MeshData {
    /// Axis-aligned unit box, every face UV-tiled `tiles` times.
    #[must_use]
    pub fn textured_box(tiles: f32, texture: Option<TextureData>) -> Self {
        let mut vertices = Vec::with_capacity(36);
        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        for (n, u, v) in CUBE_FACES {
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
            for idx in [0usize, 1, 2, 0, 2, 3] {
                vertices.push(MeshVertex {
                    pos: corners[idx].to_array(),
                    normal: n,
                    uv: [uvs[idx][0] * tiles, uvs[idx][1] * tiles],
                });
            }
        }
        Self { vertices, texture }
    }

    /// Flat unit plane at y = 0 facing +Y, UVs tiled `tiles` times.
    #[must_use]
    pub fn textured_plane(tiles: f32, texture: Option<TextureData>) -> Self {
        let corners = [
            Vec3::new(-0.5, 0.0, -0.5),
            Vec3::new(0.5, 0.0, -0.5),
            Vec3::new(0.5, 0.0, 0.5),
            Vec3::new(-0.5, 0.0, 0.5),
        ];
        let uvs = [[0.0, 0.0], [tiles, 0.0], [tiles, tiles], [0.0, tiles]];
        let mut vertices = Vec::with_capacity(6);
        for idx in [0usize, 1, 2, 0, 2, 3] {
            vertices.push(MeshVertex {
                pos: corners[idx].to_array(),
                normal: [0.0, 1.0, 0.0],
                uv: uvs[idx],
            });
        }
        Self { vertices, texture }
    }
}

/// (normal, tangent u, tangent v) per cube face — shared by the built-in
/// cube and `MeshData::textured_box`.
const CUBE_FACES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
    ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
    ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
    ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
];

/// Scalar outdoor surface response; textures still supply base color only.
/// Coated/painted metal is dielectric (`metallic = 0`), not bare metal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    /// Perceptual roughness; sanitized to 0.08..=1 at GPU upload.
    pub roughness: f32,
    /// Bare-metal fraction; sanitized to 0..=1 at GPU upload.
    pub metallic: f32,
}

/// One draw unit: a colored mesh instance (default: the unit cube),
/// optionally rotated. Vertices are scaled first, then rotated.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub position: Vec3,
    pub scale: Vec3,
    pub color: Vec3,
    /// Full rotation. `with_yaw` covers the common rotate-around-Y case.
    pub rot: Quat,
    /// Mesh id: 0 = built-in cube, 1..=N = EngineConfig.meshes entries.
    pub mesh: u32,
    /// Opaque world geometry casts and receives directional shadows. Disable
    /// for first-person weapons, sky geometry and emissive decoration.
    pub casts_shadow: bool,
    /// Allow the environment's wetness to add sky and sun reflections.
    pub wettable: bool,
    /// None preserves legacy shading. Used only with an outdoor environment.
    pub surface: Option<Surface>,
}

impl Instance {
    #[must_use]
    pub const fn new(position: Vec3, scale: Vec3, color: Vec3) -> Self {
        Self {
            position,
            scale,
            color,
            rot: Quat::IDENTITY,
            mesh: 0,
            casts_shadow: true,
            wettable: false,
            surface: None,
        }
    }

    #[must_use]
    pub fn with_yaw(mut self, yaw: f32) -> Self {
        self.rot = Quat::from_rotation_y(yaw);
        self
    }

    #[must_use]
    pub const fn with_rot(mut self, rot: Quat) -> Self {
        self.rot = rot;
        self
    }

    #[must_use]
    pub const fn with_mesh(mut self, mesh: u32) -> Self {
        self.mesh = mesh;
        self
    }

    #[must_use]
    pub const fn without_shadow(mut self) -> Self {
        self.casts_shadow = false;
        self
    }

    #[must_use]
    pub const fn with_wetness(mut self) -> Self {
        self.wettable = true;
        self
    }

    /// Opt into view-dependent outdoor material lighting. This adds neither
    /// material maps nor scene-geometry reflections. Invalid values are made
    /// finite and clamped on upload (roughness fallback 1, metallic fallback 0).
    #[must_use]
    pub const fn with_surface(mut self, roughness: f32, metallic: f32) -> Self {
        self.surface = Some(Surface {
            roughness,
            metallic,
        });
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
    #[must_use]
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        // glam's perspective_rh targets wgpu's 0..1 clip depth.
        Mat4::perspective_rh(self.fov_y_deg.to_radians(), aspect.max(0.01), 0.1, 500.0)
            * Mat4::look_at_rh(self.eye, self.target, Vec3::Y)
    }
}

/// Distance fog of the scene pass: the lit colour is mixed toward `color`
/// by `1 - exp(-view_depth * density)`.
///
/// The default is the tuning the shader carried as constants before fog
/// became per-frame, so a game that never sets it renders exactly as it
/// did. The mix happens after the ACES tonemap, so `color` is post-tonemap
/// linear light: the horizon the picture actually shows, not a scene
/// radiance that the curve would then compress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub color: [f32; 3],
    pub density: f32,
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            color: [0.012, 0.020, 0.045],
            density: 0.005,
        }
    }
}

/// Everything the game wants drawn this frame.
#[derive(Default)]
pub struct Frame {
    pub camera: Camera,
    pub instances: Vec<Instance>,
    /// Per-frame fog; `Fog::default()` is the pre-v13 look.
    pub fog: Fog,
    /// Opt-in outdoor sky, directional light, shadows and wet reflections.
    pub environment: Environment,
    /// Immutable static indirect-light visibility. Bake once, then share each frame.
    pub occlusion: Option<Arc<OcclusionField>>,
    /// Camera-facing alpha particles drawn after opaque geometry.
    pub particles: Vec<Particle>,
}

/// Scene-pass uniform (group 0, binding 0). Mirrors `SceneUniform` in
/// `shader.wgsl`: WGSL uniform layout wants a 16-byte-multiple struct and
/// a 16-byte-aligned `vec3`, so fog colour and density share one `vec4`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    /// `Fog::color` in xyz, `Fog::density` in w.
    fog: [f32; 4],
    inverse_view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    sun_direction: [f32; 4],
    sun_color: [f32; 4],
    sky_zenith: [f32; 4],
    sky_horizon: [f32; 4],
    wind_time: [f32; 4],
    camera_right: [f32; 4],
    camera_up: [f32; 4],
    occlusion_min_strength: [f32; 4],
    occlusion_cell: [f32; 4],
}

impl SceneUniform {
    const SIZE: u64 = std::mem::size_of::<Self>() as u64;

    fn new(frame: &Frame, aspect: f32) -> Self {
        let environment = &frame.environment;
        let eye = finite_vec3(frame.camera.eye, Vec3::new(0.0, 32.0, 40.0));
        let forward = finite_vec3(frame.camera.target - eye, -Vec3::Z)
            .try_normalize()
            .unwrap_or(-Vec3::Z);
        let up = if forward.cross(Vec3::Y).length_squared() < 1.0e-8 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let right = forward.cross(up).normalize();
        let camera_up = right.cross(forward).normalize();
        let view_proj = Mat4::perspective_rh(
            finite_clamp(frame.camera.fov_y_deg, 50.0, 1.0, 179.0).to_radians(),
            finite_clamp(aspect, 1.0, 0.01, 100.0),
            0.1,
            500.0,
        ) * Mat4::look_at_rh(eye, eye + forward, up);
        let sun_direction = finite_vec3(environment.sun_direction, Vec3::new(0.4, 1.0, 0.3))
            .try_normalize()
            .unwrap_or(Vec3::Y);
        let extent = finite_clamp(environment.shadow_extent, 90.0, 16.0, 180.0);
        // Camera-centred orthographic map: quantize its two lateral coordinates
        // to shadow texels so walking does not make static shadows shimmer.
        let light_up = if sun_direction.dot(Vec3::Y).abs() > 0.995 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let light_right = (-sun_direction).cross(light_up).normalize();
        let light_vertical = light_right.cross(-sun_direction).normalize();
        let texel = 2.0 * extent / SHADOW_SIZE as f32;
        let center = eye
            + light_right
                * (eye.dot(light_right) / texel)
                    .round()
                    .mul_add(texel, -eye.dot(light_right))
            + light_vertical
                * (eye.dot(light_vertical) / texel)
                    .round()
                    .mul_add(texel, -eye.dot(light_vertical));
        let light_view_proj =
            Mat4::orthographic_rh(-extent, extent, -extent, extent, 0.1, extent * 4.0)
                * Mat4::look_at_rh(center + sun_direction * extent * 2.0, center, light_up);
        let fog_color = finite_vec3(
            Vec3::from_array(frame.fog.color),
            Vec3::from_array(Fog::default().color),
        )
        .clamp(Vec3::ZERO, Vec3::ONE);
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            fog: fog_color
                .extend(finite_clamp(frame.fog.density, 0.005, 0.0, 1.0))
                .to_array(),
            inverse_view_proj: view_proj.inverse().to_cols_array_2d(),
            light_view_proj: light_view_proj.to_cols_array_2d(),
            eye: eye.extend(1.0).to_array(),
            sun_direction: sun_direction
                .extend(f32::from(u8::from(environment.enabled)))
                .to_array(),
            sun_color: finite_vec3(environment.sun_color, Vec3::ONE)
                .clamp(Vec3::ZERO, Vec3::splat(4.0))
                .extend(finite_clamp(environment.sun_intensity, 1.15, 0.0, 8.0))
                .to_array(),
            sky_zenith: finite_vec3(environment.sky_zenith, Vec3::new(0.12, 0.3, 0.65))
                .clamp(Vec3::ZERO, Vec3::splat(4.0))
                .extend(finite_clamp(environment.cloud_coverage, 0.45, 0.0, 1.0))
                .to_array(),
            sky_horizon: finite_vec3(environment.sky_horizon, Vec3::new(0.55, 0.65, 0.75))
                .clamp(Vec3::ZERO, Vec3::splat(4.0))
                .extend(finite_clamp(environment.wetness, 0.0, 0.0, 1.0))
                .to_array(),
            wind_time: [
                finite_clamp(environment.wind.x, 0.0, -30.0, 30.0),
                finite_clamp(environment.wind.y, 0.0, -30.0, 30.0),
                finite_clamp(environment.time, 0.0, 0.0, 1.0e7),
                extent,
            ],
            camera_right: right.extend(0.0).to_array(),
            camera_up: camera_up.extend(0.0).to_array(),
            occlusion_min_strength: frame
                .occlusion
                .as_ref()
                .map_or([0.0; 4], |field| field.min().extend(0.65).to_array()),
            occlusion_cell: frame
                .occlusion
                .as_ref()
                .map_or([1.0; 4], |field| field.cell_size().extend(0.0).to_array()),
        }
    }
}

const fn finite_clamp(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn finite_vec3(value: Vec3, fallback: Vec3) -> Vec3 {
    if value.is_finite() { value } else { fallback }
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
    rot: [f32; 4],
    /// Wettable, shadow receiver, roughness (-1 = legacy), metallic.
    material: [f32; 4],
}

impl From<&Instance> for InstanceRaw {
    fn from(instance: &Instance) -> Self {
        let (roughness, metallic) = instance.surface.map_or((-1.0, 0.0), |surface| {
            (
                finite_clamp(surface.roughness, 1.0, 0.08, 1.0),
                finite_clamp(surface.metallic, 0.0, 0.0, 1.0),
            )
        });
        Self {
            pos: instance.position.to_array(),
            scale: instance.scale.to_array(),
            color: instance.color.to_array(),
            rot: instance.rot.to_array(),
            material: [
                f32::from(u8::from(instance.wettable)),
                f32::from(u8::from(instance.casts_shadow)),
                roughness,
                metallic,
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ParticleRaw {
    position: [f32; 3],
    size: [f32; 2],
    color: [f32; 3],
    opacity: f32,
}

const SHADOW_SIZE: u32 = 1536;
const MAX_PARTICLES: usize = 4096;
// Packed depth is colour-renderable and textureLoad-readable on WebGL2.
const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct ShadowTargets {
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    bind: wgpu::BindGroup,
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
    /// Size of the last swapchain texture actually presented; logged when it
    /// changes, which is the only evidence a fullscreen switch reached the GPU
    /// (a borderless fullscreen swapchain presents in flip mode, invisible
    /// to screen capture).
    presented_size: [u32; 2],
    scene_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    particle_pipeline: wgpu::RenderPipeline,
    shadow: ShadowTargets,
    occlusion_layout: wgpu::BindGroupLayout,
    occlusion_bind: wgpu::BindGroup,
    occlusion_field: Option<Arc<OcclusionField>>,
    particle_buf: wgpu::Buffer,
    scene_uniform_buf: wgpu::Buffer,
    scene_uniform_bind: wgpu::BindGroup,
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
    // Native hot reload reads this layout; wasm intentionally retains it without reading it.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    scene_pipeline_layout: wgpu::PipelineLayout,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    effect_pipeline_layout: wgpu::PipelineLayout,
    // Native hot reload reads this layout; wasm intentionally retains it without reading it.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    present_pipeline_layout: wgpu::PipelineLayout,
    #[cfg(not(target_arch = "wasm32"))]
    shader_reload: ShaderReload,
    /// Scene-pass Hz cap for the ATW rig; 0 = uncapped (overlay-controlled).
    #[cfg(not(target_arch = "wasm32"))]
    scene_hz_cap: f32,
    #[cfg(not(target_arch = "wasm32"))]
    last_scene_at: std::time::Instant,
    #[cfg(not(target_arch = "wasm32"))]
    egui_painter: Option<egui_wgpu::Renderer>,
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
    /// Create a renderer for `window` and register the supplied meshes.
    ///
    /// # Panics
    ///
    /// Panics when the platform cannot create a compatible surface, adapter,
    /// device, or surface configuration.
    // GPU initialization is a linear descriptor pipeline whose ordering mirrors resource dependencies.
    #[allow(clippy::too_many_lines)]
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
            if let Some(srgb) = caps
                .formats
                .iter()
                .copied()
                .find(wgpu::TextureFormat::is_srgb)
            {
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
        // Trilinear: every mesh texture is uploaded with its full mip chain
        // (see `mip_chain`), so minification blends between levels instead
        // of skipping texels and shimmering at distance.
        let mesh_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mesh sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = TextureData {
            width: 1,
            height: 1,
            rgba8: vec![255, 255, 255, 255],
        };

        let mesh_textures: Vec<Option<TextureData>> = std::iter::once(None)
            .chain(extra_meshes.iter().map(|m| m.texture.clone()))
            .collect();
        #[cfg(not(target_arch = "wasm32"))]
        let mut mesh_textures = mesh_textures;
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
            // The whole chain is uploaded, not just level 0: wgpu's GL
            // backend (the WebGL2 fallback) allocates storage for every
            // declared level and samples garbage from any it never received.
            let (mip_level_count, levels) = mip_chain(data);
            let tex = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: data.width,
                        height: data.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &levels,
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
            bind: make_bind(
                mesh_textures[0].as_ref().unwrap_or(&white),
                "mesh 0 texture",
            ),
        });
        for (i, m) in extra_meshes.iter().enumerate() {
            let verts: Vec<Vertex> = m
                .vertices
                .iter()
                .map(|v| Vertex {
                    pos: v.pos,
                    normal: v.normal,
                    uv: v.uv,
                })
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

        let scene_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene uniform"),
            size: SceneUniform::SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene_uniform_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("scene uniform layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // The vertex stage reads the camera, the fragment stage
                    // reads the fog.
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let scene_uniform_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene uniform bind"),
            layout: &scene_uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_uniform_buf.as_entire_binding(),
            }],
        });

        let shadow_layout = create_shadow_layout(&device);
        let shadow = create_shadow_targets(&device, &shadow_layout);
        let occlusion_layout = create_occlusion_layout(&device);
        let occlusion_bind = create_occlusion_bind(&device, &queue, &occlusion_layout, None);
        let scene_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("scene pipeline layout"),
                bind_group_layouts: &[
                    &scene_uniform_layout,
                    &mesh_tex_layout,
                    &shadow_layout,
                    &occlusion_layout,
                ],
                push_constant_ranges: &[],
            });
        let scene_pipeline = build_scene_pipeline(&device, &scene_pipeline_layout, &shader);
        let effect_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("environment pipeline layout"),
                bind_group_layouts: &[&scene_uniform_layout],
                push_constant_ranges: &[],
            });
        let sky_pipeline =
            build_effect_pipeline(&device, &effect_pipeline_layout, &shader, EffectPass::Sky);
        let shadow_pipeline = build_effect_pipeline(
            &device,
            &effect_pipeline_layout,
            &shader,
            EffectPass::Shadow,
        );
        let particle_pipeline = build_effect_pipeline(
            &device,
            &effect_pipeline_layout,
            &shader,
            EffectPass::Particle,
        );
        let particle_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weather particles"),
            size: (MAX_PARTICLES * std::mem::size_of::<ParticleRaw>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
        let present_bind = create_present_bind(
            &device,
            &present_layout,
            &scene.color_view,
            &present_sampler,
        );
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
            presented_size: [0, 0],
            scene_pipeline,
            sky_pipeline,
            shadow_pipeline,
            particle_pipeline,
            shadow,
            occlusion_layout,
            occlusion_bind,
            occlusion_field: None,
            particle_buf,
            scene_uniform_buf,
            scene_uniform_bind,
            meshes,
            instance_buf,
            instance_cap,
            present_pipeline,
            present_layout,
            present_bind,
            present_sampler,
            surface_view_format,
            scene_pipeline_layout,
            effect_pipeline_layout,
            present_pipeline_layout,
            #[cfg(not(target_arch = "wasm32"))]
            shader_reload: ShaderReload::default(),
            #[cfg(not(target_arch = "wasm32"))]
            scene_hz_cap: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            last_scene_at: std::time::Instant::now(),
            #[cfg(not(target_arch = "wasm32"))]
            egui_painter: None,
        }
    }

    /// ATW rig: cap the scene pass rate (0 = uncapped). Presenter unaffected.
    #[cfg(not(target_arch = "wasm32"))]
    pub const fn set_scene_hz_cap(&mut self, hz: f32) {
        self.scene_hz_cap = hz.max(0.0);
    }

    /// Milliseconds since the last scene pass actually rendered.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn scene_age_ms(&self) -> f32 {
        self.last_scene_at.elapsed().as_secs_f32() * 1000.0
    }

    /// Current surface size in pixels.
    pub const fn surface_size(&self) -> [u32; 2] {
        [self.config.width, self.config.height]
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
        tracing::debug!(width, height, "surface configured");
        self.scene = create_scene_targets(&self.device, &self.config, self.scene_scale);
        self.present_bind = create_present_bind(
            &self.device,
            &self.present_layout,
            &self.scene.color_view,
            &self.present_sampler,
        );
    }

    pub fn render(&mut self, frame: &Frame) {
        self.render_impl(
            frame,
            #[cfg(not(target_arch = "wasm32"))]
            None,
        );
    }

    /// Render plus an optional overlay composited after the present pass
    /// (presenter-side, never into the `SceneFrame` — ATW doc §6).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_with_overlay(
        &mut self,
        frame: &Frame,
        overlay: Option<crate::overlay::OverlayDraw>,
    ) {
        self.render_impl(frame, overlay);
    }

    // The render pass remains linear so GPU resource lifetimes and submission order stay visible.
    #[allow(clippy::too_many_lines)]
    fn render_impl(
        &mut self,
        frame: &Frame,
        #[cfg(not(target_arch = "wasm32"))] overlay: Option<crate::overlay::OverlayDraw>,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        self.maybe_reload_shaders();

        // ATW rig throttle: skip the scene pass while capped — the presenter
        // below keeps re-presenting the last SceneFrame (that staleness is
        // exactly what the rig exists to demonstrate).
        #[cfg(not(target_arch = "wasm32"))]
        let scene_pass_due = self.scene_hz_cap <= 0.0
            || self.last_scene_at.elapsed().as_secs_f32() >= 1.0 / self.scene_hz_cap;
        #[cfg(target_arch = "wasm32")]
        let scene_pass_due = true;

        // The immutable Arc identity is the upload key; normal frames never rebake
        // or upload. Reset to a tiny neutral texture when leaving the map.
        let same_occlusion = match (&self.occlusion_field, &frame.occlusion) {
            (Some(current), Some(next)) => Arc::ptr_eq(current, next),
            (None, None) => true,
            _ => false,
        };
        if scene_pass_due && !same_occlusion {
            self.occlusion_bind = create_occlusion_bind(
                &self.device,
                &self.queue,
                &self.occlusion_layout,
                frame.occlusion.as_deref(),
            );
            self.occlusion_field.clone_from(&frame.occlusion);
        }

        let surface_tex = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                tracing::debug!(
                    width = self.config.width,
                    height = self.config.height,
                    "surface lost or outdated; reconfiguring"
                );
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "dropped frame");
                return;
            }
        };
        if surface_tex.texture.width() != self.presented_size[0]
            || surface_tex.texture.height() != self.presented_size[1]
        {
            self.presented_size = [surface_tex.texture.width(), surface_tex.texture.height()];
            tracing::debug!(
                width = self.presented_size[0],
                height = self.presented_size[1],
                "presenting at a new size"
            );
        }
        let surface_view = surface_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: self.surface_view_format,
                ..Default::default()
            });

        let scene_cmd = if scene_pass_due {
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.last_scene_at = std::time::Instant::now();
            }
            let aspect = self.scene.width as f32 / self.scene.height.max(1) as f32;
            let uniform = SceneUniform::new(frame, aspect);
            self.queue
                .write_buffer(&self.scene_uniform_buf, 0, bytemuck::bytes_of(&uniform));

            // Bucket instances by mesh so each mesh draws with one instanced
            // call over a contiguous range of the shared instance buffer.
            let mut buckets: Vec<Vec<InstanceRaw>> = vec![Vec::new(); self.meshes.len()];
            let mut noncasters: Vec<Vec<InstanceRaw>> = vec![Vec::new(); self.meshes.len()];
            for i in &frame.instances {
                let m = (i.mesh as usize).min(self.meshes.len() - 1);
                let bucket = if i.casts_shadow {
                    &mut buckets[m]
                } else {
                    &mut noncasters[m]
                };
                bucket.push(InstanceRaw::from(i));
            }
            let mut raws: Vec<InstanceRaw> = Vec::with_capacity(frame.instances.len());
            let mut ranges: Vec<(usize, std::ops::Range<u32>)> = Vec::new();
            let mut shadow_ranges: Vec<(usize, std::ops::Range<u32>)> = Vec::new();
            for (mi, b) in buckets.iter().enumerate() {
                if b.is_empty() && noncasters[mi].is_empty() {
                    continue;
                }
                let start = raws.len() as u32;
                raws.extend_from_slice(b);
                if !b.is_empty() {
                    shadow_ranges.push((mi, start..raws.len() as u32));
                }
                raws.extend_from_slice(&noncasters[mi]);
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
            let particles = particle_instances(frame, &uniform);
            if !particles.is_empty() {
                self.queue
                    .write_buffer(&self.particle_buf, 0, bytemuck::cast_slice(&particles));
            }

            // Separate command buffers per pass, per the ATW doc's sliced-
            // submission rule: the presenter must never wait behind scene work
            // in a single monolithic submission.
            let mut scene_enc =
                self.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("scene encoder"),
                    });
            if frame.environment.enabled {
                let mut pass = scene_enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("directional shadow pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.shadow.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.shadow_pipeline);
                pass.set_bind_group(0, &self.scene_uniform_bind, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                for (mi, range) in &shadow_ranges {
                    let mesh = &self.meshes[*mi];
                    pass.set_vertex_buffer(0, mesh.buf.slice(..));
                    pass.draw(0..mesh.count, range.clone());
                }
            }
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
                if frame.environment.enabled {
                    pass.set_pipeline(&self.sky_pipeline);
                    pass.set_bind_group(0, &self.scene_uniform_bind, &[]);
                    pass.draw(0..3, 0..1);
                }
                if !raws.is_empty() {
                    pass.set_pipeline(&self.scene_pipeline);
                    pass.set_bind_group(0, &self.scene_uniform_bind, &[]);
                    pass.set_bind_group(2, &self.shadow.bind, &[]);
                    pass.set_bind_group(3, &self.occlusion_bind, &[]);
                    pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                    for (mi, range) in &ranges {
                        let mesh = &self.meshes[*mi];
                        pass.set_bind_group(1, &mesh.bind, &[]);
                        pass.set_vertex_buffer(0, mesh.buf.slice(..));
                        pass.draw(0..mesh.count, range.clone());
                    }
                }
                if !particles.is_empty() {
                    pass.set_pipeline(&self.particle_pipeline);
                    pass.set_bind_group(0, &self.scene_uniform_bind, &[]);
                    pass.set_vertex_buffer(0, self.particle_buf.slice(..));
                    pass.draw(0..6, 0..particles.len() as u32);
                }
            }
            Some(scene_enc.finish())
        } else {
            None
        };

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

        // Overlay (ATW doc §6): composited AFTER the warp/present pass onto
        // the swapchain — UI never bakes into the reprojectable SceneFrame.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(draw) = overlay {
            let egui_painter = self.egui_painter.get_or_insert_with(|| {
                egui_wgpu::Renderer::new(
                    &self.device,
                    self.surface_view_format.unwrap_or(self.config.format),
                    None,
                    1,
                    false,
                )
            });
            for (id, delta) in &draw.textures_delta.set {
                egui_painter.update_texture(&self.device, &self.queue, *id, delta);
            }
            egui_painter.update_buffers(
                &self.device,
                &self.queue,
                &mut present_enc,
                &draw.primitives,
                &draw.screen,
            );
            {
                let rpass = present_enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("overlay pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                let mut rpass = rpass.forget_lifetime();
                egui_painter.render(&mut rpass, &draw.primitives, &draw.screen);
            }
            for id in &draw.textures_delta.free {
                egui_painter.free_texture(id);
            }
        }

        self.queue
            .submit(scene_cmd.into_iter().chain([present_enc.finish()]));
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
        if !self.shader_reload.frame.is_multiple_of(60) {
            return;
        }

        if check_mtime(SCENE_SRC, &mut self.shader_reload.scene_mtime)
            && let Some(module) = self.try_compile(SCENE_SRC, "scene shader (hot-reload)")
        {
            // Entry-point/layout mismatches are pipeline errors, not module
            // errors. Validate the complete family before replacing any part.
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let scene = build_scene_pipeline(&self.device, &self.scene_pipeline_layout, &module);
            let sky = build_effect_pipeline(
                &self.device,
                &self.effect_pipeline_layout,
                &module,
                EffectPass::Sky,
            );
            let shadow = build_effect_pipeline(
                &self.device,
                &self.effect_pipeline_layout,
                &module,
                EffectPass::Shadow,
            );
            let particle = build_effect_pipeline(
                &self.device,
                &self.effect_pipeline_layout,
                &module,
                EffectPass::Particle,
            );
            if let Some(error) = pollster::block_on(self.device.pop_error_scope()) {
                tracing::error!(path = SCENE_SRC, %error, "scene shader hot-reload rejected; keeping all old pipelines");
            } else {
                self.scene_pipeline = scene;
                self.sky_pipeline = sky;
                self.shadow_pipeline = shadow;
                self.particle_pipeline = particle;
                tracing::info!(
                    path = SCENE_SRC,
                    "scene and environment shaders hot-reloaded"
                );
            }
        }
        if check_mtime(PRESENT_SRC, &mut self.shader_reload.present_mtime)
            && let Some(module) = self.try_compile(PRESENT_SRC, "present shader (hot-reload)")
        {
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
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
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

fn particle_instances(frame: &Frame, uniform: &SceneUniform) -> Vec<ParticleRaw> {
    let eye = Vec3::from_slice(&uniform.eye);
    let forward =
        Vec3::from_slice(&uniform.camera_up).cross(Vec3::from_slice(&uniform.camera_right));
    let mut particles: Vec<&Particle> = frame
        .particles
        .iter()
        .filter(|particle| {
            particle.position.is_finite()
                && particle.size.is_finite()
                && particle.color.is_finite()
                && particle.opacity.is_finite()
                && particle.opacity > 0.0
                && particle.size.min_element() > 0.0
        })
        .take(MAX_PARTICLES)
        .collect();
    particles.sort_by(|a, b| {
        let a_depth = (a.position - eye).dot(forward);
        let b_depth = (b.position - eye).dot(forward);
        b_depth.total_cmp(&a_depth)
    });
    particles
        .into_iter()
        .map(|particle| ParticleRaw {
            position: particle.position.to_array(),
            size: particle.size.min(glam::Vec2::splat(20.0)).to_array(),
            color: particle
                .color
                .clamp(Vec3::ZERO, Vec3::splat(8.0))
                .to_array(),
            opacity: particle.opacity.clamp(0.0, 1.0),
        })
        .collect()
}

fn create_occlusion_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let volume = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D3,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("static indirect visibility layout"),
        entries: &[
            volume(0),
            volume(1),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

// Two core-WebGL2 RGBA8 volumes; no float filtering, mip chain or extra pass.
fn create_occlusion_bind(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    field: Option<&OcclusionField>,
) -> wgpu::BindGroup {
    let [width, height, depth_or_array_layers] = field.map_or([1; 3], OcclusionField::dimensions);
    let make = |label, data: &[u8]| {
        device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D3,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                data,
            )
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let a = make(
        "indirect visibility XY",
        field.map_or(&[255; 4], OcclusionField::texture_a),
    );
    let b = make(
        "indirect visibility Z",
        field.map_or(&[255; 4], OcclusionField::texture_b),
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("indirect visibility trilinear sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("static indirect visibility"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&a),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&b),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    })
}

fn create_shadow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("packed shadow texture layout"),
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
    })
}

fn create_shadow_targets(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> ShadowTargets {
    let make = |label, format, usage| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: SHADOW_SIZE,
                    height: SHADOW_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let color_view = make(
        "directional shadow packed depth",
        SHADOW_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let depth_view = make(
        "directional shadow depth",
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("directional shadow texture"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&color_view),
        }],
    });
    ShadowTargets {
        color_view,
        depth_view,
        bind,
    }
}

const fn mesh_vertex_layouts() -> [wgpu::VertexBufferLayout<'static>; 2] {
    const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
    const INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![3 => Float32x3, 4 => Float32x3, 5 => Float32x3, 6 => Float32x4, 7 => Float32x4];
    [
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        },
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceRaw>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &INSTANCE_ATTRIBUTES,
        },
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum EffectPass {
    Sky,
    Shadow,
    Particle,
}

fn build_effect_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    effect: EffectPass,
) -> wgpu::RenderPipeline {
    const PARTICLE_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32];
    let (label, vertex, fragment) = match effect {
        EffectPass::Sky => ("sky pipeline", "vs_sky", "fs_sky"),
        EffectPass::Shadow => ("shadow pipeline", "vs_shadow", "fs_shadow"),
        EffectPass::Particle => ("particle pipeline", "vs_particle", "fs_particle"),
    };
    let mesh_buffers = mesh_vertex_layouts();
    let particle_buffers = [wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ParticleRaw>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &PARTICLE_ATTRIBUTES,
    }];
    let buffers = match effect {
        EffectPass::Sky => &[][..],
        EffectPass::Shadow => &mesh_buffers[..],
        EffectPass::Particle => &particle_buffers[..],
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: if effect == EffectPass::Shadow {
                    SHADOW_FORMAT
                } else {
                    SCENE_FORMAT
                },
                blend: if effect == EffectPass::Particle {
                    Some(wgpu::BlendState::ALPHA_BLENDING)
                } else {
                    None
                },
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: effect == EffectPass::Shadow,
            depth_compare: if effect == EffectPass::Sky {
                wgpu::CompareFunction::Always
            } else {
                wgpu::CompareFunction::LessEqual
            },
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn create_scene_targets(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    scale: f32,
) -> SceneTargets {
    let width = ((config.width as f32 * scale) as u32).max(1);
    let height = ((config.height as f32 * scale) as u32).max(1);
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
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
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &mesh_vertex_layouts(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
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
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
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
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// True when the file's mtime differs from `tracked` (which is updated).
/// The first successful stat only records the baseline and reports false.
#[cfg(not(target_arch = "wasm32"))]
fn check_mtime(path: &str, tracked: &mut Option<std::time::SystemTime>) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
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
    Ok(TextureData {
        width: img.width(),
        height: img.height(),
        rgba8: img.into_raw(),
    })
}

/// The full mip chain of an RGBA8 texture, tightly packed level after
/// level, plus its length: exactly what `create_texture_with_data` consumes
/// for a single-layer 2D texture.
///
/// Level sizes follow WebGPU's rule (`max(1, dim >> level)`, chain length
/// `32 - leading_zeros(max(w, h))`), so a 1x1 texture stays one level and
/// an odd dimension drops its last row or column rather than clamping the
/// size up. Each level averages RGB in linear light, then encodes it back
/// to sRGB bytes; alpha is already linear and is averaged directly. The
/// lookup tables avoid doing transfer-function powers for every texel.
///
/// # Panics
///
/// Panics when `tex.rgba8` is not `width * height * 4` bytes; the upload
/// would have rejected it anyway, this just names the mesh's problem.
fn mip_chain(tex: &TextureData) -> (u32, Vec<u8>) {
    let expected = (tex.width * tex.height * 4) as usize;
    assert_eq!(
        tex.rgba8.len(),
        expected,
        "texture {}x{} needs {} RGBA8 bytes, got {}",
        tex.width,
        tex.height,
        expected,
        tex.rgba8.len()
    );
    let levels = 32 - tex.width.max(tex.height).leading_zeros();
    // The chain is bounded by 4/3 of level 0 (geometric series of quarters).
    let mut bytes = Vec::with_capacity(tex.rgba8.len() / 3 * 4 + 64);
    bytes.extend_from_slice(&tex.rgba8);
    let (mut w, mut h) = (tex.width, tex.height);
    let mut cur: std::borrow::Cow<'_, [u8]> = std::borrow::Cow::Borrowed(&tex.rgba8);
    for _ in 1..levels {
        let (nw, nh, next) = downsample_rgba8(&cur, w, h);
        bytes.extend_from_slice(&next);
        (w, h) = (nw, nh);
        cur = std::borrow::Cow::Owned(next);
    }
    (levels, bytes)
}

/// Transfer tables for sRGB albedo mips, initialized once per process.
///
/// Output thresholds are the linear-light equivalents of half-byte sRGB
/// values. Searching them rounds the encoded mean exactly, without a large
/// approximate inverse table or an expensive per-channel power operation.
struct SrgbMipTables {
    decode: [f64; 256],
    thresholds: [f64; 255],
}

impl SrgbMipTables {
    fn get() -> &'static Self {
        static TABLES: std::sync::OnceLock<SrgbMipTables> = std::sync::OnceLock::new();
        TABLES.get_or_init(|| {
            let linear = |encoded: f64| {
                if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                }
            };
            Self {
                decode: std::array::from_fn(|i| {
                    linear(f64::from(u8::try_from(i).expect("sRGB table index fits u8")) / 255.0)
                }),
                thresholds: std::array::from_fn(|i| {
                    linear(
                        (f64::from(u8::try_from(i).expect("sRGB threshold index fits u8")) + 0.5)
                            / 255.0,
                    )
                }),
            }
        })
    }

    fn average(&self, samples: [u8; 4]) -> u8 {
        let mean = samples
            .iter()
            .map(|&sample| self.decode[usize::from(sample)])
            .sum::<f64>()
            * 0.25;
        u8::try_from(
            self.thresholds
                .partition_point(|&threshold| threshold <= mean),
        )
        .expect("there are only 255 encoding thresholds")
    }
}

/// One mip step averages RGB in linear light and alpha directly.
///
/// A dimension already at 1 samples the same texel twice so the average
/// stays exact; the last row or column of an odd dimension is dropped,
/// matching the `>> 1` size rule the GPU applies.
fn downsample_rgba8(src: &[u8], sw: u32, sh: u32) -> (u32, u32, Vec<u8>) {
    let dw = (sw >> 1).max(1);
    let dh = (sh >> 1).max(1);
    let texel = |x: u32, y: u32, c: u32| src[((y * sw + x) * 4 + c) as usize];
    let srgb = SrgbMipTables::get();
    let mut out = Vec::with_capacity((dw * dh * 4) as usize);
    for y in 0..dh {
        let (y0, y1) = (2 * y, (2 * y + 1).min(sh - 1));
        for x in 0..dw {
            let (x0, x1) = (2 * x, (2 * x + 1).min(sw - 1));
            for c in 0..4 {
                let samples = [
                    texel(x0, y0, c),
                    texel(x1, y0, c),
                    texel(x0, y1, c),
                    texel(x1, y1, c),
                ];
                out.push(if c == 3 {
                    let sum: u32 = samples.iter().map(|&sample| u32::from(sample)).sum();
                    ((sum + 2) / 4) as u8
                } else {
                    srgb.average(samples)
                });
            }
        }
    }
    (dw, dh, out)
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
            verts.push(Vertex {
                pos: corners[idx].to_array(),
                normal: n,
                uv: uvs[idx],
            });
        }
    }
    verts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_upload_preserves_legacy_flags_and_finite_material_bounds() {
        let instance = Instance::new(Vec3::ONE, Vec3::splat(2.0), Vec3::splat(0.3));
        assert_eq!(InstanceRaw::from(&instance).material, [0.0, 1.0, -1.0, 0.0]);
        let authored = instance
            .with_surface(0.45, 0.8)
            .without_shadow()
            .with_wetness();
        let raw = InstanceRaw::from(&authored);
        assert_eq!(raw.material, [1.0, 0.0, 0.45, 0.8]);
        assert_eq!(raw.pos, [1.0; 3]);
        assert_eq!(raw.scale, [2.0; 3]);
        assert_eq!(raw.color, [0.3; 3]);
        for (roughness, metallic, expected) in [
            (-1.0, -2.0, [0.08, 0.0]),
            (2.0, 3.0, [1.0, 1.0]),
            (f32::NAN, f32::NAN, [1.0, 0.0]),
            (f32::INFINITY, f32::NEG_INFINITY, [1.0, 0.0]),
        ] {
            let raw = InstanceRaw::from(&instance.with_surface(roughness, metallic));
            assert_eq!(&raw.material[2..], &expected);
        }
        assert_eq!(std::mem::size_of::<InstanceRaw>(), 68);
        let layouts = mesh_vertex_layouts();
        assert_eq!(layouts[1].array_stride, 68);
        assert_eq!(
            layouts[1].attributes[4].format,
            wgpu::VertexFormat::Float32x4
        );
        assert_eq!(layouts[1].attributes[4].offset, 52);
    }

    fn tex(width: u32, height: u32, rgba8: Vec<u8>) -> TextureData {
        TextureData {
            width,
            height,
            rgba8,
        }
    }

    /// Byte count `create_texture_with_data` will read for a single-layer
    /// RGBA8 texture with `levels` mips, computed independently of
    /// `mip_chain`'s own loop.
    fn upload_bytes(w: u32, h: u32, levels: u32) -> usize {
        (0..levels)
            .map(|l| ((w >> l).max(1) * (h >> l).max(1) * 4) as usize)
            .sum()
    }

    #[test]
    fn shared_white_pixel_stays_one_level() {
        let (levels, bytes) = mip_chain(&tex(1, 1, vec![255; 4]));
        assert_eq!(levels, 1);
        assert_eq!(bytes, vec![255; 4]);
    }

    #[test]
    fn chain_length_and_size_follow_the_webgpu_rule() {
        for (w, h, want_levels) in [
            (2, 2, 2),
            (5, 3, 3),
            (300, 7, 9),
            (1024, 1024, 11),
            (1, 8, 4),
        ] {
            let (levels, bytes) = mip_chain(&tex(w, h, vec![7; (w * h * 4) as usize]));
            assert_eq!(levels, want_levels, "{w}x{h}");
            assert_eq!(bytes.len(), upload_bytes(w, h, levels), "{w}x{h}");
            assert!(
                bytes.ends_with(&[7, 7, 7, 7]),
                "last level is 1x1 of the flat colour"
            );
        }
    }

    #[test]
    fn mip_level_one_averages_rgb_in_linear_light_and_alpha_linearly() {
        #[rustfmt::skip]
        let px = vec![
            0, 10, 20, 0,   255, 30, 40, 64,
            100, 50, 60, 128, 200, 70, 80, 255,
        ];
        let (levels, bytes) = mip_chain(&tex(2, 2, px.clone()));
        assert_eq!(levels, 2);
        assert_eq!(&bytes[..16], &px[..], "level 0 is untouched");
        // RGB is decode -> average -> encode, not the old [139, 40, 50].
        // Alpha remains the rounded integer average (0+64+128+255)/4.
        assert_eq!(&bytes[16..], &[175, 46, 55, 112]);
    }

    #[test]
    fn mip_thin_dimension_duplicates_samples_without_gamma_on_alpha() {
        // 1 wide, 2 tall: the second column clamps onto the first, so the
        // result is the exact mean of the two rows.
        for (w, h) in [(1, 2), (2, 1)] {
            let (levels, bytes) = mip_chain(&tex(w, h, vec![10, 20, 30, 0, 30, 40, 50, 255]));
            assert_eq!(levels, 2);
            assert_eq!(&bytes[8..], &[22, 32, 41, 128]);
        }
    }

    #[test]
    fn odd_dimensions_drop_the_last_row_and_column() {
        // 3x1 with a bright third texel: level 1 is 1x1 and must ignore it.
        let (levels, bytes) = mip_chain(&tex(
            3,
            1,
            vec![0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255],
        ));
        assert_eq!(levels, 2);
        assert_eq!(&bytes[12..], &[0, 0, 0, 255]);
    }

    #[test]
    fn mip_black_white_checkerboard_preserves_linear_energy() {
        let (levels, bytes) = mip_chain(&tex(
            2,
            2,
            vec![
                0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0,
            ],
        ));
        assert_eq!(levels, 2);
        assert_eq!(&bytes[16..], &[188, 188, 188, 128]);
    }

    #[test]
    fn mip_all_uniform_byte_values_survive_every_level() {
        for value in 0..=u8::MAX {
            let rgba = [value, 255 - value, value / 2, value];
            let (_, bytes) = mip_chain(&tex(8, 4, rgba.repeat(32)));
            assert!(bytes.as_chunks::<4>().0.iter().all(|pixel| *pixel == rgba));
        }
    }

    #[test]
    fn mip_lookup_rounding_matches_the_transfer_formula() {
        fn reference(samples: [u8; 4]) -> u8 {
            let linear = samples.map(|byte| {
                let encoded = f64::from(byte) / 255.0;
                if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                }
            });
            let mean = linear.into_iter().sum::<f64>() / 4.0;
            let encoded = if mean <= 0.003_130_8 {
                mean * 12.92
            } else {
                1.055_f64.mul_add(mean.powf(1.0 / 2.4), -0.055)
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                (encoded * 255.0).round() as u8
            }
        }
        let tables = SrgbMipTables::get();
        for a in 0..=u8::MAX {
            for b in 0..=u8::MAX {
                for samples in [[a, b, a, b], [a, b, a.wrapping_mul(37), b.wrapping_add(83)]] {
                    assert_eq!(tables.average(samples), reference(samples), "{samples:?}");
                }
            }
        }
    }

    #[test]
    fn mip_odd_non_square_edges_keep_the_existing_sampling_rule() {
        let mut source = [255, 255, 255, 255].repeat(15);
        for (x, y, pixel) in [
            (0, 0, [0, 0, 0, 0]),
            (1, 0, [255, 255, 255, 255]),
            (0, 1, [255, 255, 255, 255]),
            (1, 1, [0, 0, 0, 0]),
        ] {
            let at = (y * 3 + x) * 4;
            source[at..at + 4].copy_from_slice(&pixel);
        }
        let (w, h, first) = downsample_rgba8(&source, 3, 5);
        assert_eq!((w, h), (1, 2));
        assert_eq!(first, [188, 188, 188, 128, 255, 255, 255, 255]);
        // The bright last column/row is still excluded; the second level
        // consumes the encoded first level through the same transfer rule.
        let (_, chain) = mip_chain(&tex(3, 5, source));
        assert_eq!(&chain[60..68], first);
        assert_eq!(&chain[68..], &[225, 225, 225, 192]);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    #[ignore = "CPU texture-upload preparation benchmark; run with --release --nocapture"]
    #[allow(clippy::print_stdout)] // An explicitly requested, opt-in benchmark report.
    fn mip_upload_cpu_cost_1024() {
        // The pre-fix implementation is retained only as the measured
        // baseline. Both paths include allocations and level-zero copies;
        // this measures CPU preparation, not GPU submission or frame time.
        fn legacy_chain(tex: &TextureData) -> (u32, Vec<u8>) {
            let levels = 32 - tex.width.max(tex.height).leading_zeros();
            let mut bytes = Vec::with_capacity(tex.rgba8.len() / 3 * 4 + 64);
            bytes.extend_from_slice(&tex.rgba8);
            let mut source: std::borrow::Cow<'_, [u8]> = std::borrow::Cow::Borrowed(&tex.rgba8);
            let (mut w, mut h) = (tex.width, tex.height);
            for _ in 1..levels {
                let (nw, nh) = ((w >> 1).max(1), (h >> 1).max(1));
                let sample =
                    |x: u32, y: u32, c: u32| u32::from(source[((y * w + x) * 4 + c) as usize]);
                let mut next = Vec::with_capacity((nw * nh * 4) as usize);
                for y in 0..nh {
                    let (y0, y1) = (2 * y, (2 * y + 1).min(h - 1));
                    for x in 0..nw {
                        let (x0, x1) = (2 * x, (2 * x + 1).min(w - 1));
                        for c in 0..4 {
                            let sum = sample(x0, y0, c)
                                + sample(x1, y0, c)
                                + sample(x0, y1, c)
                                + sample(x1, y1, c);
                            next.push(u8::try_from((sum + 2) / 4).unwrap());
                        }
                    }
                }
                bytes.extend_from_slice(&next);
                source = std::borrow::Cow::Owned(next);
                (w, h) = (nw, nh);
            }
            (levels, bytes)
        }
        let mut pixels = Vec::with_capacity(1024 * 1024 * 4);
        for y in 0..1024_u32 {
            for x in 0..1024_u32 {
                let grain = (x * 13 + y * 7) ^ ((x / 32) * 83 + (y / 64) * 41);
                pixels.extend_from_slice(&[
                    u8::try_from(grain & 255).unwrap(),
                    u8::try_from((grain / 3 + 67) & 255).unwrap(),
                    u8::try_from((grain / 7 + 113) & 255).unwrap(),
                    255,
                ]);
            }
        }
        let input = tex(1024, 1024, pixels);
        let measure = |make: fn(&TextureData) -> (u32, Vec<u8>)| {
            std::hint::black_box(make(std::hint::black_box(&input)));
            let started = std::time::Instant::now();
            for _ in 0..16 {
                std::hint::black_box(make(std::hint::black_box(&input)));
            }
            started.elapsed().as_secs_f64() * 1000.0 / 16.0
        };
        let old_ms = measure(legacy_chain);
        let linear_ms = measure(mip_chain);
        println!(
            "1024x1024 RGBA CPU full-chain preparation, 16 warmed runs: \
             old encoded {old_ms:.3} ms/texture; linear-light {linear_ms:.3} ms/texture; \
             ratio {:.2}x (not GPU upload or frame time)",
            linear_ms / old_ms
        );
    }

    #[test]
    fn fog_default_is_the_old_shader_constants() {
        let f = Fog::default();
        assert_eq!(f.color, [0.012, 0.020, 0.045]);
        assert_eq!(f.density, 0.005);
        assert_eq!(Frame::default().fog, f);
    }

    #[test]
    fn scene_uniform_keeps_fog_offsets_and_packs_environment_in_vec4s() {
        assert_eq!(SceneUniform::SIZE, 368, "three mat4 + eleven vec4");
        let frame = Frame {
            fog: Fog {
                color: [0.1, 0.2, 0.3],
                density: 0.4,
            },
            ..Frame::default()
        };
        let u = SceneUniform::new(&frame, 1.0);
        assert_eq!(u.fog, [0.1, 0.2, 0.3, 0.4]);
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 368);
        assert_eq!(&bytes[64..68], &0.1f32.to_le_bytes());
        assert_eq!(&bytes[76..80], &0.4f32.to_le_bytes());
        assert_eq!(&bytes[336..352], bytemuck::bytes_of(&[0.0_f32; 4]));
        assert_eq!(&bytes[352..368], bytemuck::bytes_of(&[1.0_f32; 4]));
    }

    #[test]
    fn uniform_handles_vertical_sun_degenerate_camera_and_invalid_weather() {
        let mut frame = Frame::default();
        frame.camera.eye = Vec3::ZERO;
        frame.camera.target = Vec3::ZERO;
        frame.camera.fov_y_deg = f32::NAN;
        frame.environment.sun_direction = Vec3::Y;
        frame.environment.cloud_coverage = f32::INFINITY;
        frame.environment.shadow_extent = f32::NAN;
        frame.environment.wind.x = f32::NAN;
        frame.environment.sun_intensity = f32::INFINITY;
        let uniform = SceneUniform::new(&frame, f32::NAN);
        let values: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&uniform));
        assert!(values.iter().all(|value| value.is_finite()));
        let light = Mat4::from_cols_array_2d(&uniform.light_view_proj);
        assert!(light.determinant().abs() > 1.0e-10);
    }

    #[test]
    fn uniform_preserves_sniper_scope_projection() {
        let frame = Frame {
            camera: Camera {
                eye: Vec3::new(3.0, 1.7, 4.0),
                target: Vec3::new(0.0, 2.0, -10.0),
                fov_y_deg: 3.5,
            },
            ..Frame::default()
        };
        let uniform = SceneUniform::new(&frame, 16.0 / 9.0);
        let actual = Mat4::from_cols_array_2d(&uniform.view_proj);
        assert!(actual.abs_diff_eq(frame.camera.view_proj(16.0 / 9.0), 0.0001));
    }

    #[test]
    fn near_vertical_camera_preserves_roll_and_projection() {
        let pitch = 1.53_f32;
        let frame = Frame {
            camera: Camera {
                eye: Vec3::ZERO,
                target: Vec3::new(0.0, pitch.sin(), -pitch.cos()),
                fov_y_deg: 60.0,
            },
            ..Frame::default()
        };
        let uniform = SceneUniform::new(&frame, 1.0);
        assert!(
            Mat4::from_cols_array_2d(&uniform.view_proj)
                .abs_diff_eq(frame.camera.view_proj(1.0), 0.0001)
        );
        assert!(Vec3::from_slice(&uniform.camera_right).abs_diff_eq(Vec3::X, 0.0001));
    }

    #[test]
    fn particle_upload_rejects_invalid_values_and_orders_by_view_depth() {
        let mut frame = Frame {
            camera: Camera {
                eye: Vec3::ZERO,
                target: -Vec3::Z,
                fov_y_deg: 60.0,
            },
            ..Frame::default()
        };
        let particle = |position| Particle {
            position,
            size: glam::Vec2::ONE,
            color: Vec3::ONE,
            opacity: 0.5,
        };
        // Distance alone is wrong here: the off-axis near drop is farther
        // from the eye, but the centre far drop must blend first.
        frame.particles = vec![
            particle(Vec3::new(10.0, 0.0, -2.0)),
            particle(Vec3::new(0.0, 0.0, -5.0)),
            particle(Vec3::splat(f32::NAN)),
        ];
        let uniform = SceneUniform::new(&frame, 1.0);
        let raw = particle_instances(&frame, &uniform);
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[0].position, [0.0, 0.0, -5.0]);
        assert_eq!(raw[1].position, [10.0, 0.0, -2.0]);
    }

    #[test]
    fn environment_wgsl_validates_all_entrypoints() {
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("shader.wgsl"))
            .expect("environment shader must parse");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect(
            "all scene/environment shader entrypoints must validate without optional capabilities",
        );
        assert_eq!(module.entry_points.len(), 8);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "renderer_gpu_test.rs"]
mod gpu_tests;
