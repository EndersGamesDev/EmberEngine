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
    camera_rotation_pairs, camera_translation, exterior_zero, hot_ring_bytes, pack_homography_rows,
    palette, scene_indices, scene_shader, view_scale, warp_shader,
};

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const FENCE_BYTES: u64 = 4;
const HOT_SOURCE_VALID_BYTE_OFFSET: u64 = 280;
const EXPOSURE_FACT_STEPS: u32 = 9;

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

type MapSignal = Arc<Mutex<Option<Result<(), ()>>>>;

struct PendingFence {
    ledger: FenceLedger,
    signal: MapSignal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SampleTracker {
    completed_since_reset: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WarpSourceSlot {
    planned: Option<(u64, u32)>,
}

impl WarpSourceSlot {
    fn write_hot(&mut self, plan: &crate::WarpPlan) {
        self.planned = plan
            .source_scene_id
            .zip(plan.source_texture_index)
            .filter(|_| plan.source_valid);
    }

    fn frame<'a>(&self, retained: Option<&'a crate::SceneFrame>) -> Option<&'a crate::SceneFrame> {
        select_warp_source(self.planned, retained)
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
    scene_group: wgpu::BindGroup,
    warp_hot_group: wgpu::BindGroup,
    warp_texture_layout: wgpu::BindGroupLayout,
    scene_sampler: wgpu::Sampler,
    hot_buffer: wgpu::Buffer,
    scene_buffer: wgpu::Buffer,
    scene_textures: [SceneTexture; 2],
    depth: DepthTarget,
    indices: Option<IndexTarget>,
    scene_pipeline: wgpu::RenderPipeline,
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
            previous.state.palette_id != main.state.palette_id || previous.grid != main.grid
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
            main.object,
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
    #[allow(
        clippy::too_many_lines,
        reason = "HOT publication keeps pose, source identity, and exposure in one transaction"
    )]
    pub fn write_hot(&mut self, slot: HotSlot, hot: PresentHot, validation: WarpValidation) {
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
        let selected = self
            .main
            .as_ref()
            .and_then(PresentMain::selected_palette)
            .unwrap_or((PaletteId::Classic, palette(PaletteId::Classic)));
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
        self.hot_exposed[slot.index() as usize] = plan.exposed;
        self.exposure.observe_warp(plan.exposed);
        self.facts.centre_from_reference_px = hot.state.centre_from_reference_px;
        self.facts.view = hot.view;
        let exposed_fraction = pose
            .as_ref()
            .and_then(|to_pose| warp_exposed_fraction(&plan, to_pose, self.ledger.retained()));
        self.facts.record_warp_plan(&plan, exposed_fraction);
        self.facts.scene_fill_due = self.exposure.due();
        if matches!(plan.kind, WarpKind::ClearOnly | WarpKind::ReliefRedraw)
            && self.ledger.retained().is_none()
        {
            self.facts.status = PresentStatus::WaitingForFirstScene;
        } else if pose.is_some_and(|current| {
            self.ledger
                .retained()
                .is_some_and(|frame| frame.pose == current)
        }) {
            self.facts.status = PresentStatus::ShowingCompletedScene;
        } else if plan.source_valid {
            self.facts.status = PresentStatus::ShowingStaleApproximation;
        }
    }

    /// Returns the level and exposure state of the accepted retained warp source.
    #[must_use]
    pub fn accepted_warp_source(&self, slot: HotSlot) -> Option<(RefinementLevel, bool)> {
        self.hot_warp_source[slot.index() as usize]
            .frame(self.ledger.retained())
            .map(|frame| (frame.level, self.hot_exposed[slot.index() as usize]))
    }

    /// Submits the one scene pass plus its four-byte completion fence.
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
            ensure_depth(device, gpu, extent)?;
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
            .write_buffer(&self.gpu.scene_buffer, 0, bytemuck::bytes_of(&uniform));
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
        );
        encoder.clear_buffer(&self.gpu.scene_fence, 0, Some(FENCE_BYTES));
        self.queue.submit([encoder.finish()]);
        self.scene_fence = Some(arm_fence(
            &self.gpu.scene_fence,
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
        let source = self.hot_warp_source[hot_slot.index() as usize].frame(self.ledger.retained());
        let source_scene_id = source.map(|frame| frame.scene_id);
        let texture_index = source.map_or(0, |frame| frame.texture_index as usize);
        if source.is_none() {
            self.queue.write_buffer(
                &self.gpu.hot_buffer,
                u64::from(hot_slot.dynamic_offset()) + HOT_SOURCE_VALID_BYTE_OFFSET,
                bytemuck::bytes_of(&0_u32),
            );
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot warp and fence"),
            });
        {
            let clear = self
                .main
                .as_ref()
                .and_then(PresentMain::selected_palette)
                .map_or(palette(PaletteId::Classic).clear_rgba, |selected| {
                    selected.1.clear_rgba
                });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Julibrot sole warp pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: state.surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color(clear)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.gpu.warp_pipeline);
            pass.set_bind_group(0, &self.gpu.scene_textures[texture_index].warp_group, &[]);
            pass.set_bind_group(1, &self.gpu.warp_hot_group, &[hot_slot.dynamic_offset()]);
            pass.draw(0..3, 0..1);
        }
        encoder.clear_buffer(&self.gpu.warp_fence, 0, Some(FENCE_BYTES));
        self.queue.submit([encoder.finish()]);
        self.warp_fence = Some(arm_fence(
            &self.gpu.warp_fence,
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
                        self.publish_promoted(&frame);
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

    fn publish_promoted(&mut self, frame: &crate::SceneFrame) {
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
    let scene_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Julibrot regional scene uniform"),
        size: u64::from(SCENE_PAYLOAD_BYTES),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let scene_layout = create_scene_layout(device);
    let scene_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Julibrot immutable scene and HOT group"),
        layout: &scene_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &scene_buffer,
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
                    buffer: &scene_buffer,
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
        scene_group,
        warp_hot_group,
        warp_texture_layout,
        scene_sampler: sampler,
        hot_buffer,
        scene_buffer,
        scene_textures,
        depth,
        indices: None,
        scene_pipeline,
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

#[allow(clippy::too_many_arguments)]
fn create_scene_pipeline(
    device: &wgpu::Device,
    label: &'static str,
    source: &str,
    vertex_entry: &'static str,
    fragment_entry: &'static str,
    heap_layout: &wgpu::BindGroupLayout,
    scene_layout: &wgpu::BindGroupLayout,
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
                format: SCENE_FORMAT,
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
            depth_compare: wgpu::CompareFunction::LessEqual,
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

fn encode_scene(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    texture_index: usize,
    hot_offset: u32,
    selected: PaletteRecord,
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
        label: Some("Julibrot scene pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &gpu.scene_textures[texture_index].view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(scene_load_color(selected)),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth_attachment,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    pass.set_bind_group(1, &gpu.scene_group, &[hot_offset]);
    pass.set_pipeline(&gpu.scene_pipeline);
    if let Some(indices) = &gpu.indices {
        pass.set_index_buffer(indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.count, 0, 0..1);
    }
}

pub fn scene_load_color(selected: PaletteRecord) -> wgpu::Color {
    color(exterior_zero(selected))
}

fn validate_grid(main: &PresentMain, limits: DialectLimits) -> Result<(), PresentError> {
    let active_len = main
        .grid
        .width
        .checked_mul(main.grid.height)
        .filter(|length| *length > 0 && *length <= main.grid.span.logical_len)
        .ok_or(PresentError::InvalidGrid {
            width: main.grid.width,
            height: main.grid.height,
            logical_len: main.grid.span.logical_len,
        })?;
    if main.grid.span.directory_index >= limits.span_capacity
        || main.grid.span.page_count > limits.handle_capacity
        || main
            .grid
            .span
            .handles()
            .iter()
            .any(|handle| handle.index() >= limits.descriptor_capacity)
    {
        return Err(PresentError::StaleSpan {
            directory_index: main.grid.span.directory_index,
        });
    }
    if active_len == 0 || main.state.delivered_iter_cap == 0 {
        return Err(PresentError::InvalidGrid {
            width: main.grid.width,
            height: main.grid.height,
            logical_len: main.grid.span.logical_len,
        });
    }
    Ok(())
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

fn arm_fence(buffer: &wgpu::Buffer, ledger: FenceLedger) -> PendingFence {
    let signal = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&signal);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if let Ok(mut slot) = callback.lock() {
                *slot = Some(result.map_err(|_| ()));
            }
        });
    PendingFence { ledger, signal }
}

fn observe_fence(pending: &mut PendingFence, now_ms: f64) -> FenceDecision {
    let callback = pending.signal.lock().ok().and_then(|mut slot| slot.take());
    pending.ledger.observe(now_ms, callback)
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
            PoseMap::Mapped(map) => map
                .rows
                .into_iter()
                .chain(map.inverse)
                .chain([map.condition_number])
                .all(f64::is_finite),
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

#[cfg(test)]
mod tests {
    use ember_julibrot_kernels::RefinementLevel;
    use ember_julibrot_math::{PrecisionMode, ViewControls};

    use super::*;
    use crate::SubmissionMeasurement;
    use crate::state::PendingScene;

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
    fn browser_order_clears_a_hot_plan_after_scene_promotion() {
        let mut ledger = SceneLedger::default();
        assert!(!ledger.invalidate_incompatible(
            64,
            [0.0; 4],
            ember_julibrot_math::ObjectAngles::JULIA,
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
    fn every_gpu_dynamic_offset_comes_from_the_opaque_slot() {
        let source = include_str!("gpu.rs");
        let accessor = [".dynamic_", "offset()"].concat();
        let bypass = ["index()", " * self.gpu.hot_stride"].concat();
        assert_eq!(source.matches(&accessor).count(), 4);
        assert!(!source.contains(&bypass));
    }
}
