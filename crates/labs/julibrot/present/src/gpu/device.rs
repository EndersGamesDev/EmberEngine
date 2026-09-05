use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use ember_lab_heap::{DialectLimits, HeapPresentResources};

#[cfg(test)]
use crate::PaletteId;
use crate::fence::FenceLedger;
use crate::state::{ExposureLatch, SceneLedger};
use crate::{
    HOT_PAYLOAD_BYTES, HotSlot, Pose, PoseMap, PresentConfig, PresentError, PresentFacts,
    PresentMain, PresentStatus, SCENE_PAYLOAD_BYTES, SampleClass, WarpKind, glitch_count_shader,
    hot_ring_bytes, scene_shader,
};

#[cfg(test)]
use census::census_bytes;
use census::{
    arm_fence, arm_glitch_readback, create_fence, create_glitch_count_target,
    create_glitch_readback, encode_glitch_count, ensure_glitch_count_resources,
};
use ledger::{
    apply_hold_policy, clear_warp_plan, identity_rows, pose_is_finite, select_warp_source,
    warp_exposed_fraction,
};
use redraw::encode_relief_redraw;
#[cfg(test)]
use redraw::relief_scene_uniform;
use scene::{
    create_depth_target, create_scene_pipeline, create_scene_texture, encode_scene,
    encode_scene_mesh, ensure_backdrop_indices, ensure_depth, ensure_indices, ensure_scene_texture,
    extent_3d, validate_backdrop, validate_extent, validate_grid, validate_grid_parts,
};
use uniforms::{
    create_heap_layout, create_scene_layout, create_warp_hot_layout, create_warp_texture_layout,
};
use warp::{color, create_warp_pipeline, warp_load_color};
#[cfg(test)]
use warp::{planned_exposed_fraction, relief_redraw_clear_fraction};

mod census;
mod ledger;
mod poll;
mod redraw;
mod scene;
mod uniforms;
mod warp;

#[cfg(test)]
pub use scene::scene_load_color;

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const FENCE_BYTES: u64 = 4;
const HOT_HOMOGRAPHY_BYTE_OFFSET: u64 = 144;
const HOT_SOURCE_VALID_BYTE_OFFSET: u64 = 280;
const EXPOSURE_FACT_STEPS: u32 = 9;
const GLITCH_RECORDS_PER_TEXEL: u32 = 255;
const RGBA8_BYTES_PER_TEXEL: u32 = 4;
const SCENE_DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::LessEqual;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SceneLayer {
    Backdrop,
    Main,
}
const BACKDROP_THEN_MAIN: [SceneLayer; 2] = [SceneLayer::Backdrop, SceneLayer::Main];
const MAIN_ONLY: [SceneLayer; 1] = [SceneLayer::Main];

/// The coarse backdrop is drawn first and the main grid over it.
///
/// The depth ranges keep the independently sampled layers ordered while preserving depth ordering
/// inside each mesh; see [`scene_depth_range`].
const fn scene_draw_order(has_backdrop: bool) -> &'static [SceneLayer] {
    if has_backdrop {
        &BACKDROP_THEN_MAIN
    } else {
        &MAIN_ONLY
    }
}

/// Disjoint viewport-depth ranges make every main fragment at least as near as every backdrop
/// fragment without disabling depth ordering inside either mesh. This is same-surface quality
/// ownership rather than a claim that independently sampled chords have comparable physical depth:
/// a backdrop chord that passes in front of a main back-face still loses, matching the previous
/// stencil ownership rule instead of letting a coarse draft displace main content.
const fn scene_depth_range(layer: SceneLayer, has_backdrop: bool) -> [f32; 2] {
    if !has_backdrop {
        return [0.0, 1.0];
    }
    match layer {
        SceneLayer::Backdrop => [0.5, 1.0],
        SceneLayer::Main => [0.0, 0.5],
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "WebGPU viewport dimensions are f32 and device texture extents are below exact f32 integer range"
)]
const fn viewport_dimension(value: u32) -> f32 {
    value as f32
}

/// The main-grid stencil stamp and the pass's clear value.
const MAIN_STENCIL: u32 = 1;
const BACKDROP_STENCIL: u32 = 0;

/// The stencil reference each layer is drawn with.
const fn stencil_reference(layer: SceneLayer) -> u32 {
    match layer {
        SceneLayer::Main => MAIN_STENCIL,
        SceneLayer::Backdrop => BACKDROP_STENCIL,
    }
}

/// Stencil state retained by the depth-stencil pipelines.
///
/// Backdrop now draws first, so its equality test sees the clear zero, while the later main stamp
/// is read by no draw. Both stencil operations are therefore vacuous; the disjoint depth ranges
/// alone enforce main-over-backdrop ownership and preserve ordering within each mesh.
const fn scene_stencil(layer: SceneLayer) -> wgpu::StencilState {
    let face = match layer {
        SceneLayer::Main => wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Replace,
        },
        SceneLayer::Backdrop => wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        },
    };
    wgpu::StencilState {
        front: face,
        back: face,
        read_mask: 0xff,
        write_mask: match layer {
            SceneLayer::Main => 0xff,
            SceneLayer::Backdrop => 0,
        },
    }
}

struct SceneTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    warp_group: wgpu::BindGroup,
    extent: [u32; 2],
}

struct DepthTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: [u32; 2],
}

struct IndexTarget {
    buffer: wgpu::Buffer,
    count: u32,
    extent: [u32; 2],
}

struct GlitchCountTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: [u32; 2],
}

struct GlitchReadback {
    buffer: wgpu::Buffer,
    extent: [u32; 2],
    bytes_per_row: u32,
}

/// One completed grid's census: its exact status-one count and its reference candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SceneCensus {
    glitch_pixel_count: u32,
    reference_sample: Option<u32>,
}

impl SceneCensus {
    const EMPTY: Self = Self {
        glitch_pixel_count: 0,
        reference_sample: None,
    };

    const fn glitch_pixel_count(self) -> u32 {
        self.glitch_pixel_count
    }
    const fn reference_sample(self) -> Option<u32> {
        self.reference_sample
    }
}

type MapSignal = Arc<Mutex<Option<Result<(), ()>>>>;

struct PendingFence {
    ledger: FenceLedger,
    signal: MapSignal,
    signal_result: Option<Result<(), ()>>,
    glitch_readback: Option<PendingGlitchReadback>,
}

struct PendingGlitchReadback {
    signal: MapSignal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SampleTracker {
    completed_since_reset: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WarpSourceSlot {
    planned: Option<(u64, u32)>,
    held_stale: bool,
    relief_redraw: bool,
    hold_on_redraw_refusal: bool,
}

impl WarpSourceSlot {
    fn write_hot(&mut self, plan: &crate::WarpPlan, hold_on_redraw_refusal: bool) {
        self.planned = plan
            .source_scene_id
            .zip(plan.source_texture_index)
            .filter(|_| plan.source_valid);
        self.held_stale = plan.kind == WarpKind::HoldStale;
        self.relief_redraw = plan.kind == WarpKind::ReliefRedraw;
        self.hold_on_redraw_refusal = hold_on_redraw_refusal;
    }
    fn frame<'a>(
        &self,
        retained: Option<&'a crate::SceneFrame>,
        held: Option<&'a crate::state::HeldScene>,
    ) -> Option<&'a crate::SceneFrame> {
        let source = if self.held_stale {
            retained.or_else(|| held.map(|held| &held.frame))
        } else {
            retained
        };
        select_warp_source(self.planned, source)
    }
    fn relief_frame<'a>(
        &self,
        retained: Option<&'a crate::SceneFrame>,
        held: Option<&'a crate::state::HeldScene>,
    ) -> Option<&'a crate::SceneFrame> {
        if self.relief_redraw {
            self.frame(retained, held)
        } else {
            None
        }
    }
    fn accepted_frame<'a>(
        &self,
        retained: Option<&'a crate::SceneFrame>,
        held: Option<&'a crate::state::HeldScene>,
    ) -> Option<&'a crate::SceneFrame> {
        if self.held_stale {
            None
        } else {
            self.frame(retained, held)
        }
    }
}

impl SampleTracker {
    const fn next(self) -> SampleClass {
        match self.completed_since_reset {
            0 => SampleClass::ColdWarmUp,
            1 => SampleClass::PolicyProbe,
            _ => SampleClass::Measured,
        }
    }
    const fn completed(&mut self) {
        self.completed_since_reset = self.completed_since_reset.saturating_add(1);
    }

    const fn reset(&mut self) {
        self.completed_since_reset = 0;
    }
}

struct GpuState {
    heap_group: wgpu::BindGroup,
    scene_groups: [wgpu::BindGroup; 2],
    warp_hot_group: wgpu::BindGroup,
    warp_texture_layout: wgpu::BindGroupLayout,
    scene_sampler: wgpu::Sampler,
    hot_buffer: wgpu::Buffer,
    scene_buffers: [wgpu::Buffer; 2],
    scene_textures: [SceneTexture; 2],
    depth: DepthTarget,
    indices: Option<IndexTarget>,
    backdrop_indices: Option<IndexTarget>,
    glitch_count_target: GlitchCountTarget,
    glitch_readback: GlitchReadback,
    scene_pipeline: wgpu::RenderPipeline,
    backdrop_pipeline: wgpu::RenderPipeline,
    glitch_count_pipeline: wgpu::RenderPipeline,
    relief_redraw_pipeline: wgpu::RenderPipeline,
    relief_redraw_backdrop_pipeline: wgpu::RenderPipeline,
    warp_pipeline: wgpu::RenderPipeline,
    scene_fence: wgpu::Buffer,
    warp_fence: wgpu::Buffer,
    heap_limits: DialectLimits,
}

/// Two-texture Julibrot scene owner and sole warp-pass runtime.
pub struct Presenter {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    gpu: GpuState,
    config: PresentConfig,
    main: Option<PresentMain>,
    hot: [Option<Pose>; 3],
    latest_hot_slot: Option<HotSlot>,
    hot_warp_source: [WarpSourceSlot; 3],
    hot_exposed: [bool; 3],
    exposure: ExposureLatch,
    ledger: SceneLedger,
    scene_fence: Option<PendingFence>,
    warp_fence: Option<PendingFence>,
    next_scene_id: u64,
    next_warp_id: u64,
    scene_samples: SampleTracker,
    warp_samples: SampleTracker,
    active_warp_scene: Option<u64>,
    active_warp_count: u32,
    replaced_warp_scene: Option<u64>,
    facts: PresentFacts,
}

impl Presenter {
    /// Allocates the exact HOT ring, two scene slots, fixed pipelines, and immutable heap group.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for invalid limits, formats, or checked resource sizes.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        heap: HeapPresentResources,
        config: PresentConfig,
    ) -> Result<Self, PresentError> {
        validate_config(&device, &heap, config)?;
        let gpu = create_gpu_state(&device, &heap, config)?;
        Ok(Self {
            device,
            queue,
            gpu,
            config,
            main: None,
            hot: [None; 3],
            latest_hot_slot: None,
            hot_warp_source: [WarpSourceSlot::default(); 3],
            hot_exposed: [false; 3],
            exposure: ExposureLatch::default(),
            ledger: SceneLedger::default(),
            scene_fence: None,
            warp_fence: None,
            next_scene_id: 1,
            next_warp_id: 1,
            scene_samples: SampleTracker::default(),
            warp_samples: SampleTracker::default(),
            active_warp_scene: None,
            active_warp_count: 0,
            replaced_warp_scene: None,
            facts: PresentFacts::default(),
        })
    }

    /// Applies latest-wins MAIN state, exactly-once reference rebasing, and incompatibility clear.
    #[allow(clippy::float_cmp)]
    pub fn set_main(&mut self, main: PresentMain) {
        let precision_mode = main.precision_mode();
        let precision_mode_name = precision_mode.map_or("unavailable", |mode| mode.as_str());
        let incompatible = self.main.as_ref().is_some_and(|previous| {
            previous.state.delivered_iter_cap != main.state.delivered_iter_cap
                || previous.state.precision_mode != main.state.precision_mode
        });
        let revision_advanced = self
            .main
            .as_ref()
            .is_none_or(|previous| previous.state.centre_revision != main.state.centre_revision);
        let selection_replaced = self.main.as_ref().is_some_and(|previous| {
            previous.state.palette_id != main.state.palette_id
                || previous.grid != main.grid
                || previous.backdrop != main.backdrop
        });
        if selection_replaced {
            self.ledger.mark_replaced();
        }
        let retained_became_held = self.ledger.invalidate_incompatible(
            main.state.delivered_iter_cap,
            main.state.plane_origin_f64,
            main.plane,
            precision_mode_name,
        );
        if revision_advanced {
            self.ledger.apply_reference_shift(
                main.state.generation_applied,
                main.state.centre_revision,
                main.state.reference_shift_px,
            );
            if let Some(frame) = self.ledger.retained() {
                self.facts.source_generation = Some(frame.pose.orbit_generation);
            }
        }
        if retained_became_held || incompatible || precision_mode.is_none() {
            self.facts.status = PresentStatus::ClearForIncompatibleMain;
            self.publish_held_facts();
        }
        self.facts.reference_shift_px = main.state.reference_shift_px;
        self.facts.precision_mode = precision_mode_name;
        if let Some((palette_id, _)) = main.selected_palette() {
            self.facts.palette = palette_id;
        }
        self.main = Some(main);
    }

    /// Forgets a retained scene whose record span has just left the owning heap allocator.
    pub fn forget_retained_grid(&mut self, grid: &ember_julibrot_kernels::EscapeGrid) {
        if self.ledger.forget_retained_grid(grid) {
            self.clear_retained_facts();
        }
    }

    /// Forgets retained records immediately before their live span is overwritten.
    ///
    /// The completed image remains available for ordinary reprojection and stale-picture holds.
    pub fn forget_retained_records(&mut self, grid: &ember_julibrot_kernels::EscapeGrid) {
        self.ledger.forget_retained_records(grid);
    }

    const fn clear_retained_facts(&mut self) {
        self.facts.completed_scene_id = None;
        self.facts.source_generation = None;
        self.facts.held_frame_partition = None;
        self.facts.held_since_scene_id = None;
        self.facts.delivered_width = 0;
        self.facts.delivered_height = 0;
        self.facts.delivered_level = None;
        self.facts.iteration_cap = None;
        self.facts.glitch_pixel_count = None;
        self.active_warp_scene = None;
        self.active_warp_count = 0;
    }

    const fn publish_held_facts(&mut self) {
        let Some(held) = self.ledger.held() else {
            self.clear_retained_facts();
            return;
        };
        self.facts.completed_scene_id = Some(held.frame.scene_id);
        self.facts.source_generation = Some(held.frame.pose.orbit_generation);
        self.facts.held_frame_partition = Some(held.partition);
        self.facts.held_since_scene_id = Some(held.held_since_scene_id);
        self.facts.delivered_width = held.frame.extent[0];
        self.facts.delivered_height = held.frame.extent[1];
        self.facts.delivered_level = Some(held.frame.level);
        self.facts.iteration_cap = Some(held.frame.iteration_cap);
        self.active_warp_scene = Some(held.frame.scene_id);
    }
}

fn validate_config(
    device: &wgpu::Device,
    heap: &HeapPresentResources,
    config: PresentConfig,
) -> Result<(), PresentError> {
    if heap.descriptor_capacity == 0 || heap.span_capacity == 0 || heap.handle_capacity == 0 {
        return Err(PresentError::Device {
            operation: "validate heap presentation capacities",
        });
    }
    if config.min_uniform_buffer_offset_alignment
        != device.limits().min_uniform_buffer_offset_alignment
        || !config.fence_deadline_ms.is_finite()
        || config.fence_deadline_ms <= 0.0
        || config.max_fence_polls == 0
    {
        return Err(PresentError::Device {
            operation: "validate presentation configuration",
        });
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn create_gpu_state(
    device: &wgpu::Device,
    heap: &HeapPresentResources,
    config: PresentConfig,
) -> Result<GpuState, PresentError> {
    let hot_bytes = hot_ring_bytes(config.min_uniform_buffer_offset_alignment).map_err(|_| {
        PresentError::Device {
            operation: "compute HOT ring bytes",
        }
    })?;
    let heap_limits = DialectLimits {
        descriptor_capacity: heap.descriptor_capacity,
        span_capacity: heap.span_capacity,
        handle_capacity: heap.handle_capacity,
    };
    let heap_layout = create_heap_layout(device, heap_limits)?;
    let heap_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Julibrot immutable heap presentation group"),
        layout: &heap_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&heap.data_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: heap.descriptor_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: heap.span_directory_buffer.as_entire_binding(),
            },
        ],
    });
    let hot_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Julibrot three-slot HOT ring"),
        size: u64::from(hot_bytes),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let scene_buffers = [
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Julibrot main scene uniform"),
            size: u64::from(SCENE_PAYLOAD_BYTES),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Julibrot backdrop scene uniform"),
            size: u64::from(SCENE_PAYLOAD_BYTES),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
    ];
    let scene_layout = create_scene_layout(device);
    let scene_groups = [
        ("Julibrot main scene and HOT group", &scene_buffers[0]),
        ("Julibrot backdrop scene and HOT group", &scene_buffers[1]),
    ]
    .map(|(label, scene_buffer)| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &scene_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: scene_buffer,
                        offset: 0,
                        size: NonZeroU64::new(u64::from(SCENE_PAYLOAD_BYTES)),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &hot_buffer,
                        offset: 0,
                        size: NonZeroU64::new(u64::from(HOT_PAYLOAD_BYTES)),
                    }),
                },
            ],
        })
    });
    let warp_texture_layout = create_warp_texture_layout(device);
    let warp_hot_layout = create_warp_hot_layout(device);
    let warp_hot_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Julibrot immutable warp HOT group"),
        layout: &warp_hot_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &hot_buffer,
                    offset: 0,
                    size: NonZeroU64::new(u64::from(HOT_PAYLOAD_BYTES)),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &scene_buffers[0],
                    offset: 0,
                    size: NonZeroU64::new(u64::from(SCENE_PAYLOAD_BYTES)),
                }),
            },
        ],
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Julibrot nearest scene sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let scene_textures = [
        create_scene_texture(device, &warp_texture_layout, &sampler, [1, 1]),
        create_scene_texture(device, &warp_texture_layout, &sampler, [1, 1]),
    ];
    let glitch_count_target = create_glitch_count_target(device, [1, 1]);
    let glitch_readback = create_glitch_readback(device, [1, 1]);
    let depth = create_depth_target(device, [1, 1]);
    let source = scene_shader(heap_limits);
    let scene_pipeline = create_scene_pipeline(
        device,
        "Julibrot scene pipeline",
        &source,
        "scene_vertex",
        "scene_fragment",
        &heap_layout,
        &scene_layout,
        SCENE_FORMAT,
        Some(SceneLayer::Main),
    );
    // Same shader, same target, one differing fixed-function state: the backdrop is stencil-tested
    // against the stamp the main pass leaves, so it reaches only the pixels the main grid missed.
    let backdrop_pipeline = create_scene_pipeline(
        device,
        "Julibrot backdrop scene pipeline",
        &source,
        "scene_vertex",
        "scene_fragment",
        &heap_layout,
        &scene_layout,
        SCENE_FORMAT,
        Some(SceneLayer::Backdrop),
    );
    let glitch_count_pipeline = create_scene_pipeline(
        device,
        "Julibrot glitch census pipeline",
        &glitch_count_shader(heap_limits),
        "glitch_count_vertex",
        "glitch_count_fragment",
        &heap_layout,
        &scene_layout,
        SCENE_FORMAT,
        None,
    );
    let relief_redraw_pipeline = create_scene_pipeline(
        device,
        "Julibrot relief redraw pipeline",
        &source,
        "scene_vertex",
        "scene_fragment",
        &heap_layout,
        &scene_layout,
        config.surface_format,
        Some(SceneLayer::Main),
    );
    let relief_redraw_backdrop_pipeline = create_scene_pipeline(
        device,
        "Julibrot relief redraw backdrop pipeline",
        &source,
        "scene_vertex",
        "scene_fragment",
        &heap_layout,
        &scene_layout,
        config.surface_format,
        Some(SceneLayer::Backdrop),
    );
    let warp_pipeline = create_warp_pipeline(
        device,
        config.surface_format,
        &warp_texture_layout,
        &warp_hot_layout,
    );
    Ok(GpuState {
        heap_group,
        scene_groups,
        warp_hot_group,
        warp_texture_layout,
        scene_sampler: sampler,
        hot_buffer,
        scene_buffers,
        scene_textures,
        depth,
        indices: None,
        backdrop_indices: None,
        glitch_count_target,
        glitch_readback,
        scene_pipeline,
        backdrop_pipeline,
        glitch_count_pipeline,
        relief_redraw_pipeline,
        relief_redraw_backdrop_pipeline,
        warp_pipeline,
        scene_fence: create_fence(device, "Julibrot scene four-byte fence"),
        warp_fence: create_fence(device, "Julibrot warp four-byte fence"),
        heap_limits,
    })
}

#[cfg(test)]
mod tests;
