use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use ember_lab_heap::{DialectLimits, HeapPresentResources};
use wgpu::util::DeviceExt as _;

use crate::fence::{FenceDecision, FenceLedger};
use crate::state::{ExposureLatch, SceneCompletion, SceneLedger};
use crate::{
    FrameReceipt, FrameState, HOT_PAYLOAD_BYTES, HotSlot, HotUniform, PaletteId, PaletteRecord,
    Pose, PoseMap, PresentConfig, PresentDataError, PresentError, PresentEvent, PresentFacts,
    PresentHot, PresentMain, PresentStatus, RefinementLevel, SCENE_PAYLOAD_BYTES, SampleClass,
    SceneUniform, SubmissionKind, Warp, WarpKind, WarpValidation, camera_rotation,
    camera_rotation_pairs, camera_translation, exterior_zero, glitch_count_shader, hot_ring_bytes,
    pack_homography_rows, palette, scene_indices, scene_shader, view_scale, warp_shader,
};

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const FENCE_BYTES: u64 = 4;
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

const fn scene_draw_order(has_backdrop: bool) -> &'static [SceneLayer] {
    if has_backdrop {
        &BACKDROP_THEN_MAIN
    } else {
        &MAIN_ONLY
    }
}

#[cfg(test)]
const fn composed_layer(main_covered: bool, backdrop_covered: bool) -> Option<SceneLayer> {
    if main_covered {
        Some(SceneLayer::Main)
    } else if backdrop_covered {
        Some(SceneLayer::Backdrop)
    } else {
        None
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
}

impl WarpSourceSlot {
    fn write_hot(&mut self, plan: &crate::WarpPlan) {
        self.planned = plan
            .source_scene_id
            .zip(plan.source_texture_index)
            .filter(|_| plan.source_valid);
        self.held_stale = plan.kind == WarpKind::HoldStale;
        self.relief_redraw = plan.kind == WarpKind::ReliefRedraw;
    }

    fn frame<'a>(&self, retained: Option<&'a crate::SceneFrame>) -> Option<&'a crate::SceneFrame> {
        select_warp_source(self.planned, retained)
    }

    fn relief_frame<'a>(
        &self,
        retained: Option<&'a crate::SceneFrame>,
    ) -> Option<&'a crate::SceneFrame> {
        if self.relief_redraw {
            self.frame(retained)
        } else {
            None
        }
    }

    fn accepted_frame<'a>(
        &self,
        retained: Option<&'a crate::SceneFrame>,
    ) -> Option<&'a crate::SceneFrame> {
        if self.held_stale {
            None
        } else {
            self.frame(retained)
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
    glitch_count_pipeline: wgpu::RenderPipeline,
    relief_redraw_pipeline: wgpu::RenderPipeline,
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
        if selection_replaced {
            self.ledger.mark_replaced();
        }
        if self.ledger.invalidate_incompatible(
            main.state.delivered_iter_cap,
            main.state.plane_origin_f64,
            main.plane,
            precision_mode_name,
        ) || incompatible
            || precision_mode.is_none()
        {
            self.facts.status = PresentStatus::ClearForIncompatibleMain;
            self.clear_retained_facts();
        }
        self.facts.reference_shift_px = main.state.reference_shift_px;
        self.facts.precision_mode = precision_mode_name;
        if let Some((palette_id, _)) = main.selected_palette() {
            self.facts.palette = palette_id;
        }
        self.main = Some(main);
    }

    /// Writes exactly one 288-byte HOT payload into the checked three-slot ring.
    ///
    /// When `hold_refused_warp` is true, a clear-only plan with a retained picture becomes an
    /// identity `HoldStale` plan so manual mode cannot replace that picture with a permanent clear.
    #[allow(
        clippy::too_many_lines,
        reason = "HOT publication keeps pose, source identity, and exposure in one transaction"
    )]
    pub fn write_hot(
        &mut self,
        slot: HotSlot,
        hot: PresentHot,
        validation: WarpValidation,
        hold_refused_warp: bool,
    ) {
        let screen_rows = match hot.map {
            PoseMap::Mapped(map) => pack_homography_rows(map.rows),
            PoseMap::EdgeOn => Some(identity_rows()),
        };
        let pose = self.main.as_ref().and_then(|main| {
            let pose = Pose {
                epoch: hot.epoch,
                orbit_generation: main.state.generation_applied,
                plane: hot.plane,
                object: hot.object,
                plane_origin: main.state.plane_origin_f64,
                zoom_log2: hot.state.zoom_log2,
                view: hot.view,
                grid_width: main.grid.width,
                grid_height: main.grid.height,
                map: hot.map,
                centre_from_reference_px: hot.state.centre_from_reference_px,
            };
            pose_is_finite(&pose).then_some(pose)
        });
        let plan = pose.as_ref().map_or_else(
            || clear_warp_plan(false, true),
            |to_pose| {
                if matches!(to_pose.map, PoseMap::EdgeOn) {
                    clear_warp_plan(true, false)
                } else if let (Some(frame), Some(precision_mode)) = (
                    self.ledger.retained(),
                    self.main.as_ref().and_then(PresentMain::precision_mode),
                ) {
                    Warp::reproject(frame, &frame.pose, to_pose, precision_mode, validation)
                } else {
                    clear_warp_plan(false, true)
                }
            },
        );
        let plan = apply_hold_policy(plan, self.ledger.retained(), hold_refused_warp);
        let selected = selected_or_classic(self.main.as_ref());
        // A refused control falls back to the neutral row rather than to a stale one, so a
        // non-finite value shows the flat chart instead of the last thing that happened to be
        // in the lane.
        let ambient = camera_rotation_pairs(hot.view.camera).unwrap_or([[1.0, 0.0, 1.0, 0.0]; 5]);
        let translation = camera_translation(hot.view.camera_translation).unwrap_or([[0.0; 4]; 2]);
        let observer = camera_rotation(hot.view.camera_yaw, hot.view.camera_pitch)
            .unwrap_or([1.0, 0.0, 1.0, 0.0]);
        let scale = view_scale(
            hot.view.height_scale,
            hot.view.distance_five,
            hot.view.distance_four,
        )
        .unwrap_or([0.0, 8.0, 8.0, 0.0]);
        let screen_rows = screen_rows.unwrap_or_else(identity_rows);
        let epoch = hot.epoch.to_le_bytes();
        let epoch_low = u32::from_le_bytes([epoch[0], epoch[1], epoch[2], epoch[3]]);
        let epoch_high = u32::from_le_bytes([epoch[4], epoch[5], epoch[6], epoch[7]]);
        let uniform = HotUniform {
            camera_rotation_pairs_0: ambient[0],
            camera_rotation_pairs_1: ambient[1],
            camera_rotation_pairs_2: ambient[2],
            camera_rotation_pairs_3: ambient[3],
            camera_rotation_pairs_4: ambient[4],
            camera_translation_0: translation[0],
            camera_translation_1: translation[1],
            observer_rotation: observer,
            view_scale: scale,
            homography_row_0: plan.rows[0],
            homography_row_1: plan.rows[1],
            homography_row_2: plan.rows[2],
            screen_to_plane_row_0: screen_rows[0],
            screen_to_plane_row_1: screen_rows[1],
            screen_to_plane_row_2: screen_rows[2],
            exterior_zero_rgba: exterior_zero(selected.1),
            clear_rgba: selected.1.clear_rgba,
            flags: [
                epoch_low,
                epoch_high,
                u32::from(plan.source_valid),
                u32::from(plan.edge_on),
            ],
        };
        let offset = u64::from(slot.dynamic_offset());
        self.queue
            .write_buffer(&self.gpu.hot_buffer, offset, bytemuck::bytes_of(&uniform));
        self.hot[slot.index() as usize] = pose;
        self.latest_hot_slot = Some(slot);
        self.hot_warp_source[slot.index() as usize].write_hot(&plan);
        // A refusal against the pose the retained scene was rendered at is not a disocclusion.
        // Exposure means the destination shows ground the source cannot cover, and the answer to
        // it is a completed scene; when the source already is the completed scene at this exact
        // pose there is no such ground, and restarting the ladder can only produce the same scene
        // again. Latching it there is how a held pose spends forever refining what it already has.
        let showing_retained_pose = pose.as_ref().is_some_and(|current| {
            self.ledger
                .retained()
                .is_some_and(|frame| crate::renders_same_picture(&frame.pose, current))
        });
        let exposed = plan.exposed && !showing_retained_pose;
        self.hot_exposed[slot.index() as usize] = exposed;
        self.exposure.observe_warp(exposed);
        self.facts.centre_from_reference_px = hot.state.centre_from_reference_px;
        self.facts.view = hot.view;
        let exposed_fraction = pose
            .as_ref()
            .and_then(|to_pose| warp_exposed_fraction(&plan, to_pose, self.ledger.retained()));
        self.facts.record_warp_plan(&plan, exposed_fraction);
        self.facts.warp_exposed = exposed;
        self.facts.scene_fill_due = self.exposure.due();
        if matches!(plan.kind, WarpKind::ClearOnly | WarpKind::ReliefRedraw)
            && self.ledger.retained().is_none()
        {
            self.facts.status = PresentStatus::WaitingForFirstScene;
        } else if showing_retained_pose {
            self.facts.status = PresentStatus::ShowingCompletedScene;
        } else if plan.source_valid {
            self.facts.status = PresentStatus::ShowingStaleApproximation;
        }
    }

    /// Returns the level and exposure state of the accepted retained warp source.
    #[must_use]
    pub fn accepted_warp_source(&self, slot: HotSlot) -> Option<(RefinementLevel, bool)> {
        self.hot_warp_source[slot.index() as usize]
            .accepted_frame(self.ledger.retained())
            .map(|frame| (frame.level, self.hot_exposed[slot.index() as usize]))
    }

    /// Reports whether this HOT slot selects a retained-record relief redraw.
    #[must_use]
    pub fn accepted_relief_redraw(&self, slot: HotSlot) -> bool {
        self.hot_warp_source[slot.index() as usize]
            .relief_frame(self.ledger.retained())
            .is_some()
    }

    /// Submits the one image scene pass, optional Final census, and completion fence.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for absent/invalid state, an occupied target, or checked overflow.
    pub fn submit_scene(&mut self, hot_slot: HotSlot, now_ms: f64) -> Result<u64, PresentError> {
        let result = self.try_submit_scene(hot_slot, now_ms);
        if let Err(error) = &result {
            self.facts.status = PresentStatus::Refused(error.clone());
        }
        result
    }

    #[allow(
        clippy::too_many_lines,
        reason = "scene submission keeps validation and its ordered GPU transaction together"
    )]
    fn try_submit_scene(&mut self, hot_slot: HotSlot, now_ms: f64) -> Result<u64, PresentError> {
        let main = self.main.as_ref().ok_or(PresentError::InvalidGrid {
            width: 0,
            height: 0,
            logical_len: 0,
        })?;
        validate_grid(main, self.gpu.heap_limits)?;
        if let Some(backdrop) = &main.backdrop {
            validate_backdrop(backdrop, self.gpu.heap_limits)?;
        }
        let pose = self.hot[hot_slot.index() as usize].ok_or(PresentError::Device {
            operation: "select unwritten HOT slot",
        })?;
        let (palette_id, palette_record) = main.selected_palette().ok_or(PresentError::Device {
            operation: "decode palette identifier",
        })?;
        let precision_mode = main.precision_mode().ok_or(PresentError::Device {
            operation: "decode precision policy",
        })?;
        let extent = [main.grid.width, main.grid.height];
        validate_extent(&self.device, extent)?;
        let uniform = SceneUniform::new(
            extent,
            main.grid.level as u32,
            main.state.delivered_iter_cap,
            main.grid.span.directory_index,
            main.grid.span.logical_len,
            main.plane,
            main.map,
            palette_record,
        )
        .map_err(|error| match error {
            PresentDataError::InvalidMap => PresentError::Device {
                operation: "pack scene screen map",
            },
            _ => PresentError::InvalidGrid {
                width: extent[0],
                height: extent[1],
                logical_len: main.grid.span.logical_len,
            },
        })?;
        let backdrop_uniform = main
            .backdrop
            .as_ref()
            .map(|backdrop| {
                SceneUniform::new(
                    [backdrop.grid.width, backdrop.grid.height],
                    backdrop.grid.level as u32,
                    backdrop.iteration_cap,
                    backdrop.grid.span.directory_index,
                    backdrop.grid.span.logical_len,
                    backdrop.plane,
                    backdrop.map,
                    palette_record,
                )
            })
            .transpose()
            .map_err(|error| match error {
                PresentDataError::InvalidMap => PresentError::Device {
                    operation: "pack backdrop screen map",
                },
                _ => PresentError::InvalidGrid {
                    width: main
                        .backdrop
                        .as_ref()
                        .map_or(0, |backdrop| backdrop.grid.width),
                    height: main
                        .backdrop
                        .as_ref()
                        .map_or(0, |backdrop| backdrop.grid.height),
                    logical_len: main
                        .backdrop
                        .as_ref()
                        .map_or(0, |backdrop| backdrop.grid.span.logical_len),
                },
            })?;
        let scene_id = self.next_scene_id;
        let next_scene_id = scene_id.checked_add(1).ok_or(PresentError::Device {
            operation: "advance scene identity",
        })?;
        let device = &self.device;
        let gpu = &mut self.gpu;
        let facts = &mut self.facts;
        let scene_samples = &mut self.scene_samples;
        let warp_samples = &mut self.warp_samples;
        let texture_index = self.ledger.begin(|texture_index| {
            let reallocated = ensure_scene_texture(device, gpu, texture_index as usize, extent)?;
            if reallocated {
                facts.texture_reallocations = facts.texture_reallocations.saturating_add(1);
                scene_samples.reset();
                warp_samples.reset();
            }
            ensure_indices(device, gpu, extent)?;
            ensure_backdrop_indices(
                device,
                gpu,
                main.backdrop
                    .as_ref()
                    .map(|backdrop| [backdrop.grid.width, backdrop.grid.height]),
            )?;
            ensure_depth(device, gpu, extent)?;
            if main.grid.level == RefinementLevel::Final {
                ensure_glitch_count_resources(device, gpu, extent);
            }
            Ok(crate::state::PendingScene {
                scene_id,
                pose,
                palette: palette_id,
                iteration_cap: main.state.delivered_iter_cap,
                level: main.grid.level,
                extent,
                texture_index,
                centre_revision: main.state.centre_revision,
                plane_origin_f64: main.state.plane_origin_f64,
                precision_mode: precision_mode.as_str(),
                drop_reason: None,
            })
        })?;
        self.next_scene_id = next_scene_id;
        self.queue
            .write_buffer(&self.gpu.scene_buffers[0], 0, bytemuck::bytes_of(&uniform));
        if let Some(backdrop_uniform) = backdrop_uniform {
            self.queue.write_buffer(
                &self.gpu.scene_buffers[1],
                0,
                bytemuck::bytes_of(&backdrop_uniform),
            );
        }
        let collect_glitch_count = main.grid.level == RefinementLevel::Final;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot scene and fence"),
            });
        encode_scene(
            &mut encoder,
            &self.gpu,
            texture_index as usize,
            hot_slot.dynamic_offset(),
            palette_record,
            main.backdrop.is_some(),
        );
        if collect_glitch_count {
            let readback = &self.gpu.glitch_readback;
            encode_glitch_count(&mut encoder, &self.gpu, hot_slot.dynamic_offset());
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.gpu.glitch_count_target.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback.buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(readback.bytes_per_row),
                        rows_per_image: Some(readback.extent[1]),
                    },
                },
                extent_3d(readback.extent),
            );
        }
        encoder.clear_buffer(&self.gpu.scene_fence, 0, Some(FENCE_BYTES));
        self.queue.submit([encoder.finish()]);
        let glitch_readback =
            collect_glitch_count.then(|| arm_glitch_readback(&self.gpu.glitch_readback.buffer));
        self.scene_fence = Some(arm_fence(
            &self.gpu.scene_fence,
            glitch_readback,
            FenceLedger::new(
                SubmissionKind::Scene,
                scene_id,
                None,
                precision_mode.as_str(),
                self.scene_samples.next(),
                now_ms,
                self.config.fence_deadline_ms,
                self.config.max_fence_polls,
            ),
        ));
        self.facts.in_flight_scene_id = Some(scene_id);
        Ok(scene_id)
    }

    /// Submits the sole warp pass to the borrowed surface view and returns before completion.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for zero surface extent, unwritten HOT state, or a pending warp.
    pub fn frame(
        &mut self,
        state: FrameState<'_>,
        hot_slot: HotSlot,
    ) -> Result<FrameReceipt, PresentError> {
        let result = self.try_frame(state, hot_slot);
        if let Err(error) = &result {
            self.facts.status = PresentStatus::Refused(error.clone());
        }
        result
    }

    #[allow(
        clippy::too_many_lines,
        reason = "warp submission keeps validation and its ordered GPU transaction together"
    )]
    fn try_frame(
        &mut self,
        state: FrameState<'_>,
        hot_slot: HotSlot,
    ) -> Result<FrameReceipt, PresentError> {
        if state.canvas_width == 0 || state.canvas_height == 0 {
            return Err(PresentError::SurfaceTargetZero);
        }
        if self.warp_fence.is_some() {
            return Err(PresentError::Device {
                operation: "submit warp while surface fence is pending",
            });
        }
        if self.hot[hot_slot.index() as usize].is_none() {
            return Err(PresentError::Device {
                operation: "select unwritten HOT slot",
            });
        }
        let precision_mode = self
            .main
            .as_ref()
            .and_then(PresentMain::precision_mode)
            .ok_or(PresentError::Device {
                operation: "decode precision policy",
            })?;
        let warp_id = self.next_warp_id;
        self.next_warp_id = warp_id.checked_add(1).ok_or(PresentError::Device {
            operation: "advance warp identity",
        })?;
        let source_slot = self.hot_warp_source[hot_slot.index() as usize];
        let source = source_slot.frame(self.ledger.retained()).cloned();
        let source_scene_id = source.as_ref().map(|frame| frame.scene_id);
        let texture_index = source
            .as_ref()
            .map_or(0, |frame| frame.texture_index as usize);
        let relief_redraw = source_slot.relief_frame(self.ledger.retained()).is_some();
        if source.is_none() {
            self.clear_hot_source(hot_slot);
        }
        let selected = self
            .main
            .as_ref()
            .and_then(PresentMain::selected_palette)
            .unwrap_or((PaletteId::Classic, palette(PaletteId::Classic)));
        if relief_redraw {
            let source = source.as_ref().ok_or(PresentError::Device {
                operation: "select relief redraw source",
            })?;
            self.prepare_relief_redraw(
                source,
                [state.canvas_width, state.canvas_height],
                selected.1,
            )?;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot warp and fence"),
            });
        if relief_redraw {
            encode_relief_redraw(
                &mut encoder,
                &self.gpu,
                state.surface_view,
                hot_slot.dynamic_offset(),
                selected.1,
            );
            self.facts.record_relief_redraw();
        } else {
            encode_image_warp(
                &mut encoder,
                &self.gpu,
                state.surface_view,
                texture_index,
                hot_slot.dynamic_offset(),
                selected.1,
            );
        }
        encoder.clear_buffer(&self.gpu.warp_fence, 0, Some(FENCE_BYTES));
        self.queue.submit([encoder.finish()]);
        self.warp_fence = Some(arm_fence(
            &self.gpu.warp_fence,
            None,
            FenceLedger::new(
                SubmissionKind::Warp,
                warp_id,
                source_scene_id,
                precision_mode.as_str(),
                self.warp_samples.next(),
                state.now_ms,
                self.config.fence_deadline_ms,
                self.config.max_fence_polls,
            ),
        ));
        if source_scene_id.is_none() {
            self.facts.refreshes_without_scene =
                self.facts.refreshes_without_scene.saturating_add(1);
        }
        Ok(FrameReceipt {
            refresh_id: state.refresh_id,
            warp_id,
            source_scene_id,
            precision_mode: precision_mode.as_str(),
            exposed: self.hot_exposed[hot_slot.index() as usize],
            status: self.facts.status.clone(),
        })
    }

    fn clear_hot_source(&self, hot_slot: HotSlot) {
        self.queue.write_buffer(
            &self.gpu.hot_buffer,
            u64::from(hot_slot.dynamic_offset()) + HOT_SOURCE_VALID_BYTE_OFFSET,
            bytemuck::bytes_of(&0_u32),
        );
    }

    fn prepare_relief_redraw(
        &mut self,
        source: &crate::SceneFrame,
        surface_extent: [u32; 2],
        selected: PaletteRecord,
    ) -> Result<(), PresentError> {
        let main = self.main.as_ref().ok_or(PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: 0,
        })?;
        validate_grid(main, self.gpu.heap_limits)?;
        let uniform = relief_scene_uniform(main, source, selected)?;
        ensure_indices(&self.device, &mut self.gpu, source.extent)?;
        ensure_depth(&self.device, &mut self.gpu, surface_extent)?;
        self.queue
            .write_buffer(&self.gpu.scene_buffers[0], 0, bytemuck::bytes_of(&uniform));
        Ok(())
    }

    /// Observes every pending fence once without waiting and returns terminal events.
    #[must_use]
    pub fn poll(&mut self, now_ms: f64) -> Vec<PresentEvent> {
        if self.scene_fence.is_some() || self.warp_fence.is_some() {
            self.device.poll(wgpu::Maintain::Poll);
        }
        let mut events = Vec::with_capacity(2);
        if let Some(event) = self.poll_scene(now_ms) {
            events.push(event);
        }
        if let Some(event) = self.poll_warp(now_ms) {
            events.push(event);
        }
        events
    }

    /// Returns the latest immutable facts without polling or submitting.
    #[must_use]
    pub fn facts(&self) -> PresentFacts {
        self.facts.clone()
    }

    fn poll_scene(&mut self, now_ms: f64) -> Option<PresentEvent> {
        let pending = self.scene_fence.as_mut()?;
        let decision = observe_fence(pending, now_ms);
        match decision {
            FenceDecision::Pending => None,
            FenceDecision::Complete(measurement) => {
                let readback_result = take_glitch_readback_result(pending);
                let glitch_pixel_count = glitch_count_if_ready(readback_result, || {
                    mapped_glitch_count(&self.gpu.glitch_readback)
                });
                if pending.glitch_readback.is_some() {
                    self.gpu.glitch_readback.buffer.unmap();
                }
                self.gpu.scene_fence.unmap();
                self.scene_fence = None;
                self.scene_samples.completed();
                self.facts.in_flight_scene_id = None;
                self.facts.last_scene = Some(measurement);
                let preserve_accepted_best = self
                    .latest_hot_slot
                    .and_then(|slot| {
                        let index = slot.index() as usize;
                        self.hot_warp_source[index].frame(self.ledger.retained())
                    })
                    .is_some();
                match self
                    .ledger
                    .complete_preserving_accepted_best(measurement, preserve_accepted_best)?
                {
                    SceneCompletion::Promoted(frame) => {
                        self.publish_promoted(&frame, glitch_pixel_count);
                        Some(PresentEvent::SceneCompleted { frame })
                    }
                    SceneCompletion::KeptBest(frame) => {
                        Some(PresentEvent::SceneCompleted { frame })
                    }
                    SceneCompletion::Dropped {
                        pending,
                        reason,
                        measurement,
                    } => Some(PresentEvent::SceneDropped {
                        scene_id: pending.scene_id,
                        orbit_generation: pending.pose.orbit_generation,
                        reason,
                        measurement,
                    }),
                }
            }
            FenceDecision::Refused {
                reason,
                polls,
                wall_ms,
                precision_mode,
            } => {
                let id = pending.ledger.id();
                if pending.glitch_readback.is_some() {
                    self.gpu.glitch_readback.buffer.unmap();
                }
                self.gpu.scene_fence.unmap();
                self.scene_fence = None;
                self.ledger.cancel_pending();
                self.facts.in_flight_scene_id = None;
                Some(PresentEvent::FenceRefused {
                    kind: SubmissionKind::Scene,
                    id,
                    reason,
                    polls,
                    wall_ms,
                    precision_mode,
                })
            }
        }
    }

    fn poll_warp(&mut self, now_ms: f64) -> Option<PresentEvent> {
        let pending = self.warp_fence.as_mut()?;
        let decision = observe_fence(pending, now_ms);
        match decision {
            FenceDecision::Pending => None,
            FenceDecision::Complete(measurement) => {
                self.gpu.warp_fence.unmap();
                self.warp_fence = None;
                self.warp_samples.completed();
                self.facts.last_warp = Some(measurement);
                if let Some(source) = measurement.source_scene_id {
                    if self.active_warp_scene == Some(source) {
                        self.active_warp_count = self.active_warp_count.saturating_add(1);
                    } else if self.replaced_warp_scene == Some(source) {
                        self.facts.reprojected_per_scene = Some(
                            self.facts
                                .reprojected_per_scene
                                .unwrap_or(0)
                                .saturating_add(1),
                        );
                    }
                }
                Some(PresentEvent::WarpCompleted { measurement })
            }
            FenceDecision::Refused {
                reason,
                polls,
                wall_ms,
                precision_mode,
            } => {
                let id = pending.ledger.id();
                self.gpu.warp_fence.unmap();
                self.warp_fence = None;
                Some(PresentEvent::FenceRefused {
                    kind: SubmissionKind::Warp,
                    id,
                    reason,
                    polls,
                    wall_ms,
                    precision_mode,
                })
            }
        }
    }

    fn publish_promoted(&mut self, frame: &crate::SceneFrame, glitch_pixel_count: Option<u32>) {
        let fills_current = self
            .hot
            .iter()
            .flatten()
            .max_by_key(|pose| pose.epoch)
            .is_some_and(|pose| *pose == frame.pose);
        if fills_current {
            self.exposure.scene_completed();
            self.facts.scene_fill_due = false;
            self.facts.warp_exposed = false;
            self.facts.warp_exposed_fraction = Some(0.0);
        }
        self.replaced_warp_scene = self.active_warp_scene;
        self.facts.reprojected_per_scene = self.active_warp_scene.map(|_| self.active_warp_count);
        self.active_warp_scene = Some(frame.scene_id);
        self.active_warp_count = 0;
        self.facts.completed_scene_id = Some(frame.scene_id);
        self.facts.source_generation = Some(frame.pose.orbit_generation);
        self.facts.precision_mode = frame.precision_mode;
        self.facts.delivered_width = frame.extent[0];
        self.facts.delivered_height = frame.extent[1];
        self.facts.delivered_level = Some(frame.level);
        self.facts.iteration_cap = Some(frame.iteration_cap);
        self.facts.glitch_pixel_count = if frame.level == RefinementLevel::Final {
            glitch_pixel_count
        } else {
            None
        };
        self.facts.palette = frame.palette;
        self.facts.view = frame.pose.view;
        self.facts.status = PresentStatus::ShowingCompletedScene;
    }

    const fn clear_retained_facts(&mut self) {
        self.facts.completed_scene_id = None;
        self.facts.source_generation = None;
        self.facts.delivered_width = 0;
        self.facts.delivered_height = 0;
        self.facts.delivered_level = None;
        self.facts.iteration_cap = None;
        self.facts.glitch_pixel_count = None;
        self.active_warp_scene = None;
        self.active_warp_count = 0;
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
        true,
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
        false,
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
        true,
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
        glitch_count_pipeline,
        relief_redraw_pipeline,
        warp_pipeline,
        scene_fence: create_fence(device, "Julibrot scene four-byte fence"),
        warp_fence: create_fence(device, "Julibrot warp four-byte fence"),
        heap_limits,
    })
}

fn create_heap_layout(
    device: &wgpu::Device,
    limits: DialectLimits,
) -> Result<wgpu::BindGroupLayout, PresentError> {
    let directory_records = limits
        .span_capacity
        .checked_add(limits.handle_capacity.div_ceil(4))
        .ok_or(PresentError::Device {
            operation: "compute heap directory binding size",
        })?;
    let uniform = |binding, bytes| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(bytes),
        },
        count: None,
    };
    Ok(
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Julibrot immutable heap presentation layout"),
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
                uniform(1, u64::from(limits.descriptor_capacity) * 16),
                uniform(2, u64::from(directory_records) * 16),
            ],
        }),
    )
}

fn create_scene_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot scene and HOT layout"),
        entries: &[
            uniform_entry(0, false, u64::from(SCENE_PAYLOAD_BYTES)),
            uniform_entry(1, true, u64::from(HOT_PAYLOAD_BYTES)),
        ],
    })
}

fn create_warp_hot_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot warp HOT layout"),
        entries: &[
            uniform_entry(0, true, u64::from(HOT_PAYLOAD_BYTES)),
            uniform_entry(1, false, u64::from(SCENE_PAYLOAD_BYTES)),
        ],
    })
}

fn uniform_entry(binding: u32, dynamic: bool, bytes: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: NonZeroU64::new(bytes),
        },
        count: None,
    }
}

fn create_warp_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot retained scene texture layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    })
}

fn create_scene_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    extent: [u32; 2],
) -> SceneTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot scene texture slot"),
        size: extent_3d(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SCENE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let warp_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Julibrot immutable scene warp group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    SceneTexture {
        _texture: texture,
        view,
        warp_group,
        extent,
    }
}

fn create_depth_target(device: &wgpu::Device, extent: [u32; 2]) -> DepthTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot scene depth target"),
        size: extent_3d(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    DepthTarget {
        _texture: texture,
        view,
        extent,
    }
}

fn create_glitch_count_target(device: &wgpu::Device, extent: [u32; 2]) -> GlitchCountTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot status-one census target"),
        size: extent_3d(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SCENE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    GlitchCountTarget {
        texture,
        view,
        extent,
    }
}

#[allow(clippy::too_many_arguments)]
fn create_scene_pipeline(
    device: &wgpu::Device,
    label: &'static str,
    source: &str,
    vertex_entry: &'static str,
    fragment_entry: &'static str,
    heap_layout: &wgpu::BindGroupLayout,
    scene_layout: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    depth: bool,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[heap_layout, scene_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth.then_some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: SCENE_DEPTH_COMPARE,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn create_warp_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    texture_layout: &wgpu::BindGroupLayout,
    hot_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Julibrot sole warp shader"),
        source: wgpu::ShaderSource::Wgsl(warp_shader().into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Julibrot sole warp pipeline"),
        bind_group_layouts: &[texture_layout, hot_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Julibrot sole warp pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("warp_vertex"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("warp_fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
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

fn ensure_scene_texture(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    index: usize,
    extent: [u32; 2],
) -> Result<bool, PresentError> {
    validate_extent(device, extent)?;
    if gpu.scene_textures[index].extent == extent {
        return Ok(false);
    }
    gpu.scene_textures[index] =
        create_scene_texture(device, &gpu.warp_texture_layout, &gpu.scene_sampler, extent);
    Ok(true)
}

fn ensure_glitch_count_resources(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    scene_extent: [u32; 2],
) {
    let extent = [
        scene_extent[0],
        scene_extent[1].div_ceil(GLITCH_RECORDS_PER_TEXEL),
    ];
    if gpu.glitch_count_target.extent != extent {
        gpu.glitch_count_target = create_glitch_count_target(device, extent);
        gpu.glitch_readback = create_glitch_readback(device, extent);
    }
}

fn ensure_depth(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    extent: [u32; 2],
) -> Result<(), PresentError> {
    validate_extent(device, extent)?;
    if gpu.depth.extent != extent {
        gpu.depth = create_depth_target(device, extent);
    }
    Ok(())
}

fn ensure_indices(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    extent: [u32; 2],
) -> Result<(), PresentError> {
    if gpu
        .indices
        .as_ref()
        .is_some_and(|indices| indices.extent == extent)
    {
        return Ok(());
    }
    let values = scene_indices(extent).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let count = u32::try_from(values.len()).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let contents = if values.is_empty() {
        &[0_u32][..]
    } else {
        &values
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Julibrot scene u32 index buffer"),
        contents: bytemuck::cast_slice(contents),
        usage: wgpu::BufferUsages::INDEX,
    });
    gpu.indices = Some(IndexTarget {
        buffer,
        count,
        extent,
    });
    Ok(())
}

fn ensure_backdrop_indices(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    extent: Option<[u32; 2]>,
) -> Result<(), PresentError> {
    let Some(extent) = extent else {
        return Ok(());
    };
    if gpu
        .backdrop_indices
        .as_ref()
        .is_some_and(|indices| indices.extent == extent)
    {
        return Ok(());
    }
    let values = scene_indices(extent).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let count = u32::try_from(values.len()).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let contents = if values.is_empty() {
        &[0_u32][..]
    } else {
        &values
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Julibrot backdrop u32 index buffer"),
        contents: bytemuck::cast_slice(contents),
        usage: wgpu::BufferUsages::INDEX,
    });
    gpu.backdrop_indices = Some(IndexTarget {
        buffer,
        count,
        extent,
    });
    Ok(())
}

fn encode_scene(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    texture_index: usize,
    hot_offset: u32,
    selected: PaletteRecord,
    has_backdrop: bool,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Julibrot backdrop then main scene pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &gpu.scene_textures[texture_index].view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(scene_load_color(selected)),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &gpu.depth.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    pass.set_pipeline(&gpu.scene_pipeline);
    for layer in scene_draw_order(has_backdrop) {
        let (group, indices) = match layer {
            SceneLayer::Backdrop => (&gpu.scene_groups[1], gpu.backdrop_indices.as_ref()),
            SceneLayer::Main => (&gpu.scene_groups[0], gpu.indices.as_ref()),
        };
        draw_scene_mesh(&mut pass, group, indices, hot_offset);
    }
}

fn encode_glitch_count(encoder: &mut wgpu::CommandEncoder, gpu: &GpuState, hot_offset: u32) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Julibrot status-one census pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &gpu.glitch_count_target.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_pipeline(&gpu.glitch_count_pipeline);
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    pass.set_bind_group(1, &gpu.scene_group, &[hot_offset]);
    pass.draw(0..3, 0..1);
}

fn encode_relief_redraw(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    surface_view: &wgpu::TextureView,
    hot_offset: u32,
    selected: PaletteRecord,
) {
    encode_scene_mesh(
        encoder,
        gpu,
        surface_view,
        &gpu.relief_redraw_pipeline,
        hot_offset,
        warp_load_color(selected),
        "Julibrot relief redraw pass",
    );
}

fn encode_image_warp(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    surface_view: &wgpu::TextureView,
    texture_index: usize,
    hot_offset: u32,
    selected: PaletteRecord,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Julibrot sole warp pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: surface_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(warp_load_color(selected)),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_pipeline(&gpu.warp_pipeline);
    pass.set_bind_group(0, &gpu.scene_textures[texture_index].warp_group, &[]);
    pass.set_bind_group(1, &gpu.warp_hot_group, &[hot_offset]);
    pass.draw(0..3, 0..1);
}

#[allow(
    clippy::too_many_arguments,
    reason = "the shared mesh encoder names its target, pipeline, bindings, palette, and label"
)]
fn encode_scene_mesh(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    color_view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    hot_offset: u32,
    load_color: wgpu::Color,
    label: &'static str,
) {
    let depth_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
        view: &gpu.depth.view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Discard,
        }),
        stencil_ops: None,
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(load_color),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth_attachment,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    pass.set_pipeline(pipeline);
    draw_scene_mesh(
        &mut pass,
        &gpu.scene_groups[0],
        gpu.indices.as_ref(),
        hot_offset,
    );
}

fn draw_scene_mesh<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    scene_group: &'pass wgpu::BindGroup,
    indices: Option<&'pass IndexTarget>,
    hot_offset: u32,
) {
    pass.set_bind_group(1, scene_group, &[hot_offset]);
    if let Some(indices) = indices {
        pass.set_index_buffer(indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.count, 0, 0..1);
    }
}

pub fn scene_load_color(selected: PaletteRecord) -> wgpu::Color {
    color(exterior_zero(selected))
}

fn warp_load_color(selected: PaletteRecord) -> wgpu::Color {
    color(selected.clear_rgba)
}

fn relief_scene_uniform(
    main: &PresentMain,
    source: &crate::SceneFrame,
    selected: PaletteRecord,
) -> Result<SceneUniform, PresentError> {
    if [main.grid.width, main.grid.height] != source.extent {
        return Err(PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: main.grid.span.logical_len,
        });
    }
    SceneUniform::new(
        source.extent,
        source.level as u32,
        source.iteration_cap,
        main.grid.span.directory_index,
        main.grid.span.logical_len,
        source.pose.plane,
        source.pose.map,
        selected,
    )
    .map_err(|error| match error {
        PresentDataError::InvalidMap => PresentError::Device {
            operation: "pack relief redraw source map",
        },
        _ => PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: main.grid.span.logical_len,
        },
    })
}

fn selected_or_classic(main: Option<&PresentMain>) -> (PaletteId, PaletteRecord) {
    main.and_then(PresentMain::selected_palette)
        .unwrap_or((PaletteId::Classic, palette(PaletteId::Classic)))
}

fn validate_grid(main: &PresentMain, limits: DialectLimits) -> Result<(), PresentError> {
    validate_grid_parts(&main.grid, main.state.delivered_iter_cap, limits)
}

fn validate_grid_parts(
    grid: &ember_julibrot_kernels::EscapeGrid,
    iteration_cap: u32,
    limits: DialectLimits,
) -> Result<(), PresentError> {
    let active_len = grid
        .width
        .checked_mul(grid.height)
        .filter(|length| *length > 0 && *length <= grid.span.logical_len)
        .ok_or(PresentError::InvalidGrid {
            width: grid.width,
            height: grid.height,
            logical_len: grid.span.logical_len,
        })?;
    if grid.span.directory_index >= limits.span_capacity
        || grid.span.page_count > limits.handle_capacity
        || grid
            .span
            .handles()
            .iter()
            .any(|handle| handle.index() >= limits.descriptor_capacity)
    {
        return Err(PresentError::StaleSpan {
            directory_index: grid.span.directory_index,
        });
    }
    if active_len == 0 || iteration_cap == 0 {
        return Err(PresentError::InvalidGrid {
            width: grid.width,
            height: grid.height,
            logical_len: grid.span.logical_len,
        });
    }
    Ok(())
}

fn validate_backdrop(
    backdrop: &crate::PresentBackdrop,
    limits: DialectLimits,
) -> Result<(), PresentError> {
    validate_grid_parts(&backdrop.grid, backdrop.iteration_cap, limits)
}

fn validate_extent(device: &wgpu::Device, extent: [u32; 2]) -> Result<(), PresentError> {
    let limit = device.limits().max_texture_dimension_2d;
    if extent[0] == 0 || extent[1] == 0 || extent[0] > limit || extent[1] > limit {
        return Err(PresentError::ExtentAllocation {
            width: extent[0],
            height: extent[1],
        });
    }
    Ok(())
}

const fn extent_3d(extent: [u32; 2]) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: extent[0],
        height: extent[1],
        depth_or_array_layers: 1,
    }
}

fn create_fence(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: FENCE_BYTES,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

fn create_glitch_readback(device: &wgpu::Device, extent: [u32; 2]) -> GlitchReadback {
    let packed_row = extent[0] * RGBA8_BYTES_PER_TEXEL;
    let bytes_per_row = packed_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Julibrot status-one census readback"),
        size: u64::from(bytes_per_row) * u64::from(extent[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    GlitchReadback {
        buffer,
        extent,
        bytes_per_row,
    }
}

fn arm_glitch_readback(buffer: &wgpu::Buffer) -> PendingGlitchReadback {
    let signal = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&signal);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if let Ok(mut slot) = callback.lock() {
                *slot = Some(result.map_err(|_| ()));
            }
        });
    PendingGlitchReadback { signal }
}

fn arm_fence(
    buffer: &wgpu::Buffer,
    glitch_readback: Option<PendingGlitchReadback>,
    ledger: FenceLedger,
) -> PendingFence {
    let signal = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&signal);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if let Ok(mut slot) = callback.lock() {
                *slot = Some(result.map_err(|_| ()));
            }
        });
    PendingFence {
        ledger,
        signal,
        signal_result: None,
        glitch_readback,
    }
}

fn observe_fence(pending: &mut PendingFence, now_ms: f64) -> FenceDecision {
    if pending.signal_result.is_none() {
        pending.signal_result = pending.signal.lock().ok().and_then(|mut slot| slot.take());
    }
    pending.ledger.observe(now_ms, pending.signal_result)
}

fn take_glitch_readback_result(pending: &mut PendingFence) -> Option<Result<(), ()>> {
    pending
        .glitch_readback
        .as_mut()?
        .signal
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

fn glitch_count_if_ready(
    result: Option<Result<(), ()>>,
    count: impl FnOnce() -> u32,
) -> Option<u32> {
    matches!(result, Some(Ok(()))).then(count)
}

fn mapped_glitch_count(readback: &GlitchReadback) -> u32 {
    let bytes = readback.buffer.slice(..).get_mapped_range();
    let count = sum_glitch_count_bytes(&bytes, readback.extent, readback.bytes_per_row);
    drop(bytes);
    count
}

fn sum_glitch_count_bytes(bytes: &[u8], extent: [u32; 2], bytes_per_row: u32) -> u32 {
    let packed_row = extent[0] as usize * RGBA8_BYTES_PER_TEXEL as usize;
    (0..extent[1] as usize)
        .flat_map(|row| {
            let start = row * bytes_per_row as usize;
            bytes[start..start + packed_row]
                .as_chunks::<{ RGBA8_BYTES_PER_TEXEL as usize }>()
                .0
                .iter()
                .map(|rgba| u32::from(rgba[0]))
        })
        .sum()
}

fn color(rgba: [f32; 4]) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(rgba[0]),
        g: f64::from(rgba[1]),
        b: f64::from(rgba[2]),
        a: f64::from(rgba[3]),
    }
}

fn pose_is_finite(pose: &Pose) -> bool {
    pose.grid_width > 0
        && pose.grid_height > 0
        && pose.view.is_valid()
        && pose.object.is_valid()
        && [
            pose.zoom_log2,
            pose.centre_from_reference_px[0],
            pose.centre_from_reference_px[1],
        ]
        .into_iter()
        .chain(pose.plane_origin)
        .all(f64::is_finite)
        && pose
            .plane
            .basis_u
            .into_iter()
            .chain(pose.plane.basis_v)
            .all(f32::is_finite)
        && match pose.map {
            PoseMap::Mapped(map) => {
                map.rows
                    .into_iter()
                    .chain(map.inverse)
                    .chain([map.condition_number, map.apron_scale])
                    .all(f64::is_finite)
                    && map.apron_scale >= 1.0
            }
            PoseMap::EdgeOn => true,
        }
}

fn select_warp_source(
    planned: Option<(u64, u32)>,
    retained: Option<&crate::SceneFrame>,
) -> Option<&crate::SceneFrame> {
    retained.filter(|frame| {
        select_warp_source_identity(planned, Some((frame.scene_id, frame.texture_index))).is_some()
    })
}

/// Measures the share of a fixed destination lattice that the actual warp shader paints clear.
/// Points behind either screen-map denominator are exterior sky rather than exposure.
fn warp_exposed_fraction(
    plan: &crate::WarpPlan,
    to_pose: &Pose,
    retained: Option<&crate::SceneFrame>,
) -> Option<f64> {
    let retained = select_warp_source(
        plan.source_scene_id.zip(plan.source_texture_index),
        retained,
    )?;
    if plan.kind == WarpKind::ReliefRedraw {
        return None;
    }
    if !plan.exposed {
        return Some(0.0);
    }
    let PoseMap::Mapped(screen_to_plane) = to_pose.map else {
        return None;
    };
    let source_half_width = f64::from(retained.extent[0]) * 0.5;
    let source_half_height = f64::from(retained.extent[1]) * 0.5;
    let mut exposed = 0_u32;
    for row in 0..EXPOSURE_FACT_STEPS {
        for column in 0..EXPOSURE_FACT_STEPS {
            let target = [
                (f64::from(column) / f64::from(EXPOSURE_FACT_STEPS - 1) - 0.5)
                    * f64::from(to_pose.grid_width),
                (f64::from(row) / f64::from(EXPOSURE_FACT_STEPS - 1) - 0.5)
                    * f64::from(to_pose.grid_height),
            ];
            let plane_weight = screen_to_plane.rows[6].mul_add(
                target[0],
                screen_to_plane.rows[7].mul_add(target[1], screen_to_plane.rows[8]),
            );
            if !plane_weight.is_finite() || plane_weight <= 0.0 {
                continue;
            }
            let mapped = plan.rows.map(|plan_row| {
                f64::from(plan_row[0]).mul_add(
                    target[0],
                    f64::from(plan_row[1]).mul_add(target[1], f64::from(plan_row[2])),
                )
            });
            if !mapped.iter().all(|value| value.is_finite()) || mapped[2] <= 0.0 {
                continue;
            }
            let source_pixel = [mapped[0] / mapped[2], mapped[1] / mapped[2]];
            if source_pixel[0] < -source_half_width
                || source_pixel[0] > source_half_width
                || source_pixel[1] < -source_half_height
                || source_pixel[1] > source_half_height
            {
                exposed = exposed.saturating_add(1);
            }
        }
    }
    Some(f64::from(exposed) / f64::from(EXPOSURE_FACT_STEPS * EXPOSURE_FACT_STEPS))
}

const fn select_warp_source_identity(
    planned: Option<(u64, u32)>,
    retained: Option<(u64, u32)>,
) -> Option<(u64, u32)> {
    match (planned, retained) {
        (Some(planned), Some(retained)) if planned.0 == retained.0 && planned.1 == retained.1 => {
            Some(retained)
        }
        _ => None,
    }
}

const fn identity_rows() -> [[f32; 4]; 3] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]
}

const fn clear_warp_plan(edge_on: bool, exposed: bool) -> crate::WarpPlan {
    crate::WarpPlan {
        rows: identity_rows(),
        source_scene_id: None,
        source_texture_index: None,
        source_valid: false,
        edge_on,
        exposed,
        kind: WarpKind::ClearOnly,
        chart_residual: 0.0,
        approx_max_error_px: None,
        approx_p95_error_px: None,
    }
}

fn apply_hold_policy(
    mut plan: crate::WarpPlan,
    retained: Option<&crate::SceneFrame>,
    hold_refused_warp: bool,
) -> crate::WarpPlan {
    if !hold_refused_warp || plan.kind != WarpKind::ClearOnly {
        return plan;
    }
    let Some(frame) = retained else {
        return plan;
    };
    plan.rows = identity_rows();
    plan.source_scene_id = Some(frame.scene_id);
    plan.source_texture_index = Some(frame.texture_index);
    plan.source_valid = true;
    plan.exposed = false;
    plan.kind = WarpKind::HoldStale;
    plan
}

#[cfg(test)]
mod tests {
    use ember_julibrot_kernels::{EscapeGrid, RefinementLevel};
    use ember_julibrot_math::{PrecisionMode, ViewControls};
    use ember_julibrot_worker::MainState;

    use super::*;
    use crate::SubmissionMeasurement;
    use crate::state::PendingScene;

    #[test]
    fn glitch_census_sums_red_counts_and_ignores_row_padding() {
        let mut bytes = vec![99_u8; 32];
        bytes[..8].copy_from_slice(&[7, 0, 0, 255, 11, 0, 0, 255]);
        bytes[16..24].copy_from_slice(&[13, 0, 0, 255, 17, 0, 0, 255]);
        assert_eq!(sum_glitch_count_bytes(&bytes, [2, 2], 16), 48);
    }

    #[test]
    fn census_failure_or_delay_never_refuses_or_delays_the_scene() {
        for census_result in [None, Some(Err(()))] {
            let mut pending = PendingFence {
                ledger: FenceLedger::new(
                    SubmissionKind::Scene,
                    29,
                    None,
                    PrecisionMode::PictureFast.as_str(),
                    SampleClass::Measured,
                    100.0,
                    30_000.0,
                    4_096,
                ),
                signal: Arc::new(Mutex::new(Some(Ok(())))),
                signal_result: None,
                glitch_readback: Some(PendingGlitchReadback {
                    signal: Arc::new(Mutex::new(census_result)),
                }),
            };

            let FenceDecision::Complete(measurement) = observe_fence(&mut pending, 101.0) else {
                panic!("a successful scene fence must deliver independently of its census");
            };
            assert_eq!(measurement.id, 29);
            let glitch_pixel_count =
                glitch_count_if_ready(take_glitch_readback_result(&mut pending), || {
                    panic!("an unavailable census must not be read")
                });
            assert_eq!(glitch_pixel_count, None);
        }
    }

    fn binding_pose() -> Pose {
        Pose {
            epoch: 1,
            orbit_generation: 1,
            plane: ember_julibrot_math::Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            object: ember_julibrot_math::ObjectAngles::JULIA,
            plane_origin: [0.0; 4],
            zoom_log2: 0.0,
            view: ViewControls::NEUTRAL,
            grid_width: 64,
            grid_height: 36,
            map: PoseMap::Mapped(ember_julibrot_math::Homography::IDENTITY),
            centre_from_reference_px: [0.0; 2],
        }
    }

    fn binding_measurement(id: u64) -> SubmissionMeasurement {
        SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            precision_mode: PrecisionMode::PictureFast.as_str(),
            wall_ms: 1.0,
            fence_wait_ms: 0.5,
            polls: 1,
        }
    }

    fn promote_binding_scene(ledger: &mut SceneLedger, scene_id: u64) -> crate::SceneFrame {
        ledger
            .begin(|texture_index| {
                Ok(PendingScene {
                    scene_id,
                    pose: binding_pose(),
                    palette: PaletteId::Classic,
                    iteration_cap: 64,
                    level: RefinementLevel::Final,
                    extent: [64, 36],
                    texture_index,
                    centre_revision: 1,
                    plane_origin_f64: [0.0; 4],
                    precision_mode: PrecisionMode::PictureFast.as_str(),
                    drop_reason: None,
                })
            })
            .expect("binding scene begins");
        match ledger.complete(binding_measurement(scene_id)) {
            Some(SceneCompletion::Promoted(frame)) => frame,
            other => panic!("binding scene did not promote: {other:?}"),
        }
    }

    fn binding_main() -> PresentMain {
        let mut arena = ember_lab_heap::SpanArena::new(64, 1, 64, 4_096, 64)
            .expect("relief fixture arena is valid");
        let span = arena
            .allocate_span(64 * 36, 64)
            .expect("relief fixture grid fits");
        PresentMain {
            epoch: 1,
            state: MainState {
                delivered_iter_cap: 64,
                ..MainState::default()
            },
            grid: EscapeGrid {
                span,
                width: 64,
                height: 36,
                level: RefinementLevel::Final,
            },
            object: ember_julibrot_math::ObjectAngles::JULIA,
            plane: binding_pose().plane,
            map: PoseMap::EdgeOn,
            backdrop: None,
        }
    }

    #[test]
    fn sample_classes_reset_and_advance_without_hiding_warmup() {
        let mut tracker = SampleTracker::default();
        assert_eq!(tracker.next(), SampleClass::ColdWarmUp);
        tracker.completed();
        assert_eq!(tracker.next(), SampleClass::PolicyProbe);
        tracker.completed();
        assert_eq!(tracker.next(), SampleClass::Measured);
        tracker.reset();
        assert_eq!(tracker.next(), SampleClass::ColdWarmUp);
    }

    #[test]
    fn clear_plan_is_identity_but_never_samples() {
        let plan = clear_warp_plan(false, true);
        assert_eq!(plan.kind, WarpKind::ClearOnly);
        assert!(!plan.source_valid);
        assert!(plan.exposed);
        assert_eq!(plan.rows[2], [0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn manual_hold_keeps_a_refused_warp_on_the_retained_picture() {
        let mut ledger = SceneLedger::default();
        let sampled = promote_binding_scene(&mut ledger, 37);
        let held = apply_hold_policy(clear_warp_plan(false, true), ledger.retained(), true);
        assert_eq!(held.kind, WarpKind::HoldStale);
        assert_eq!(held.source_scene_id, Some(sampled.scene_id));
        assert_eq!(held.source_texture_index, Some(sampled.texture_index));
        assert!(held.source_valid);
        assert!(!held.exposed);
        assert_eq!(held.rows, identity_rows());

        let mut facts = PresentFacts::default();
        facts.record_warp_plan(&held, Some(0.0));
        assert_eq!(facts.warp_kind, WarpKind::HoldStale);
        assert_eq!(facts.warp_kind.as_str(), "HoldStale");

        let mut hot = WarpSourceSlot::default();
        hot.write_hot(&held);
        assert_eq!(
            hot.frame(ledger.retained()).map(|frame| frame.scene_id),
            Some(37)
        );
        assert_eq!(hot.accepted_frame(ledger.retained()), None);
    }

    #[test]
    fn auto_refusal_still_clears_and_manual_bounded_warp_stays_accepted() {
        let mut ledger = SceneLedger::default();
        let sampled = promote_binding_scene(&mut ledger, 41);
        let cleared = apply_hold_policy(clear_warp_plan(false, true), ledger.retained(), false);
        assert_eq!(cleared.kind, WarpKind::ClearOnly);
        assert!(!cleared.source_valid);

        let mut bounded = clear_warp_plan(false, false);
        bounded.kind = WarpKind::AnchorHomography;
        bounded.source_scene_id = Some(sampled.scene_id);
        bounded.source_texture_index = Some(sampled.texture_index);
        bounded.source_valid = true;
        let accepted = apply_hold_policy(bounded, ledger.retained(), true);
        assert_eq!(accepted, bounded);

        let mut facts = PresentFacts::default();
        facts.record_warp_plan(
            &apply_hold_policy(clear_warp_plan(false, true), ledger.retained(), true),
            Some(0.0),
        );
        assert_eq!(facts.warp_kind, WarpKind::HoldStale);
        facts.record_warp_plan(&accepted, Some(0.0));
        assert_eq!(facts.warp_kind, WarpKind::AnchorHomography);
    }

    #[test]
    fn browser_order_clears_a_hot_plan_after_scene_promotion() {
        let mut ledger = SceneLedger::default();
        assert!(!ledger.invalidate_incompatible(
            64,
            [0.0; 4],
            binding_pose().plane,
            PrecisionMode::PictureFast.as_str(),
        ));
        let sampled = promote_binding_scene(&mut ledger, 41);
        let mut plan = clear_warp_plan(false, false);
        plan.kind = WarpKind::AnchorHomography;
        plan.source_scene_id = Some(sampled.scene_id);
        plan.source_texture_index = Some(sampled.texture_index);
        plan.source_valid = true;
        let mut hot = WarpSourceSlot::default();
        hot.write_hot(&plan);
        assert_eq!(
            hot.frame(ledger.retained()).map(|frame| frame.scene_id),
            Some(41)
        );

        let promoted = promote_binding_scene(&mut ledger, 42);
        assert_eq!(promoted.scene_id, 42);
        assert_eq!(
            hot.frame(ledger.retained()).map(|frame| frame.scene_id),
            None
        );
        assert_eq!(HOT_SOURCE_VALID_BYTE_OFFSET, 280);
    }

    #[test]
    fn accepted_exposed_plan_remains_a_source_and_reports_its_clear_share() {
        let mut ledger = SceneLedger::default();
        let sampled = promote_binding_scene(&mut ledger, 51);
        let mut plan = clear_warp_plan(false, true);
        plan.kind = WarpKind::AnchorHomography;
        plan.source_scene_id = Some(sampled.scene_id);
        plan.source_texture_index = Some(sampled.texture_index);
        plan.source_valid = true;
        plan.rows[0][2] = 16.0;
        let mut hot = WarpSourceSlot::default();
        hot.write_hot(&plan);

        assert_eq!(
            hot.frame(ledger.retained()).map(|frame| frame.scene_id),
            Some(51),
            "exposure does not invalidate the accepted source"
        );
        let fraction = warp_exposed_fraction(&plan, &binding_pose(), ledger.retained())
            .expect("the accepted source has an exposure census");
        assert!((fraction - 2.0 / 9.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn relief_redraw_reuses_the_retained_grid_and_scene_uniform_contract() {
        let mut ledger = SceneLedger::default();
        let sampled = promote_binding_scene(&mut ledger, 61);
        let mut plan = clear_warp_plan(false, true);
        plan.kind = WarpKind::ReliefRedraw;
        plan.source_scene_id = Some(sampled.scene_id);
        plan.source_texture_index = Some(sampled.texture_index);
        plan.source_valid = true;
        let mut hot = WarpSourceSlot::default();
        hot.write_hot(&plan);
        assert_eq!(
            hot.relief_frame(ledger.retained())
                .map(|frame| frame.scene_id),
            Some(61)
        );

        let main = binding_main();
        let uniform = relief_scene_uniform(&main, &sampled, crate::CLASSIC_PALETTE)
            .expect("compatible records form a scene uniform");
        assert_eq!(uniform.grid, [64, 36, RefinementLevel::Final as u32, 64]);
        assert_eq!(uniform.span[0], main.grid.span.directory_index);
        assert_eq!(uniform.span[1], 64 * 36);
        assert_eq!(uniform.basis_u, sampled.pose.plane.basis_u);
        assert_eq!(uniform.screen_to_plane_row_0, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(uniform.screen_to_plane_row_2, [0.0, 0.0, 1.0, 1.0]);
        let load = scene_load_color(crate::CLASSIC_PALETTE);
        let sky = crate::exterior_zero(crate::CLASSIC_PALETTE);
        assert_eq!([load.r, load.g, load.b, load.a], sky.map(f64::from));
    }

    #[test]
    fn backdrop_is_drawn_first_and_main_wins_equal_depth() {
        assert_eq!(
            scene_draw_order(true),
            &[SceneLayer::Backdrop, SceneLayer::Main]
        );
        assert_eq!(SCENE_DEPTH_COMPARE, wgpu::CompareFunction::LessEqual);
        assert_eq!(composed_layer(true, true), Some(SceneLayer::Main));
        assert_eq!(composed_layer(false, true), Some(SceneLayer::Backdrop));
        assert_eq!(composed_layer(false, false), None);
    }

    #[test]
    fn relief_redraw_disocclusion_is_clear_and_distinct_from_exterior() {
        let disocclusion = warp_load_color(crate::CLASSIC_PALETTE);
        let clear = crate::CLASSIC_PALETTE.clear_rgba.map(f64::from);
        let exterior = crate::exterior_zero(crate::CLASSIC_PALETTE).map(f64::from);
        assert_eq!(
            [
                disocclusion.r,
                disocclusion.g,
                disocclusion.b,
                disocclusion.a
            ],
            clear
        );
        assert_ne!(clear, exterior);
    }

    #[test]
    fn relief_redraw_refuses_a_retained_grid_from_an_old_extent() {
        let mut ledger = SceneLedger::default();
        let sampled = promote_binding_scene(&mut ledger, 62);
        let mut main = binding_main();
        main.grid.width /= 2;
        main.grid.height /= 2;
        assert!(relief_scene_uniform(&main, &sampled, crate::CLASSIC_PALETTE).is_err());
    }

    #[test]
    fn every_gpu_dynamic_offset_comes_from_the_opaque_slot() {
        let source = include_str!("gpu.rs");
        let accessor = [".dynamic_", "offset()"].concat();
        let bypass = ["index()", " * self.gpu.hot_stride"].concat();
        assert_eq!(source.matches(&accessor).count(), 6);
        assert!(!source.contains(&bypass));
    }
}
