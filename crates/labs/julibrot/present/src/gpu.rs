use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use ember_lab_heap::{DialectLimits, HeapPresentResources};
use wgpu::util::DeviceExt as _;

use crate::fence::{FenceDecision, FenceLedger};
use crate::state::{SceneCompletion, SceneLedger};
use crate::{
    FrameReceipt, FrameState, HotSlot, HotUniform, PaletteId, Pose, PresentConfig, PresentError,
    PresentEvent, PresentFacts, PresentHot, PresentMain, PresentStatus, SampleClass, SceneUniform,
    SubmissionKind, ViewMode, Warp, WarpKind, hot_ring_bytes,
    hot_stride, palette, scene_shaders, tumbled_indices, view_rotation, warp_shader,
};

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const FENCE_BYTES: u64 = 4;

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
    flat_pipeline: wgpu::RenderPipeline,
    tumbled_pipeline: wgpu::RenderPipeline,
    warp_pipeline: wgpu::RenderPipeline,
    scene_fence: wgpu::Buffer,
    warp_fence: wgpu::Buffer,
    hot_stride: u32,
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
        let incompatible = self.main.as_ref().is_some_and(|previous| {
            previous.state.delivered_iter_cap != main.state.delivered_iter_cap
                || previous.state.plane_origin_f64 != main.state.plane_origin_f64
        });
        let revision_advanced = self
            .main
            .as_ref()
            .is_none_or(|previous| previous.state.centre_revision != main.state.centre_revision);
        let view_changed = self
            .main
            .as_ref()
            .is_some_and(|previous| previous.view != main.view);
        let latest_pose = self
            .hot
            .iter()
            .flatten()
            .max_by_key(|pose| pose.epoch)
            .copied();
        if revision_advanced && let Some(mut accepted_pose) = latest_pose {
            accepted_pose.orbit_generation = main.state.generation_applied;
            accepted_pose.grid_width = main.grid.width;
            accepted_pose.grid_height = main.grid.height;
            accepted_pose.view = main.view;
            self.ledger.apply_reference_shift(
                &accepted_pose,
                main.state.generation_applied,
                main.state.centre_revision,
                main.state.reference_shift_px,
            );
        }
        if self
            .ledger
            .invalidate_incompatible(main.state.delivered_iter_cap, main.state.plane_origin_f64)
            || incompatible
        {
            self.facts.status = PresentStatus::ClearForIncompatibleMain;
            self.clear_retained_facts();
        }
        if view_changed {
            self.scene_samples.reset();
            self.warp_samples.reset();
        }
        self.facts.view = main.view;
        self.facts.reference_shift_px = main.state.reference_shift_px;
        if let Some((palette_id, _)) = main.selected_palette() {
            self.facts.palette = palette_id;
        }
        self.main = Some(main);
    }

    /// Writes exactly one 128-byte HOT payload into the checked three-slot ring.
    pub fn write_hot(&mut self, slot: HotSlot, hot: PresentHot) {
        let mut plan = None;
        let pose = self.main.as_ref().and_then(|main| {
            let view_theta_1 = 0.4 * hot.view_time_seconds;
            let pose = Pose {
                epoch: hot.epoch,
                orbit_generation: main.state.generation_applied,
                plane: hot.plane,
                plane_theta_1: hot.state.plane_theta_1,
                plane_theta_2: hot.state.plane_theta_2,
                zoom_log2: hot.state.zoom_log2,
                view_theta_1,
                grid_width: main.grid.width,
                grid_height: main.grid.height,
                view: main.view,
                centre_from_reference_px: hot.state.centre_from_reference_px,
            };
            pose_is_finite(&pose).then_some(pose)
        });
        if let (Some(frame), Some(to_pose)) = (self.ledger.retained(), pose.as_ref()) {
            plan = Some(Warp::reproject(frame, &frame.pose, to_pose));
        }
        let selected = self
            .main
            .as_ref()
            .and_then(PresentMain::selected_palette)
            .unwrap_or((PaletteId::Classic, palette(PaletteId::Classic)));
        let rotation = view_rotation(hot.view_time_seconds).unwrap_or([1.0, 0.0, 1.0, 0.0]);
        let plan = plan.unwrap_or_else(clear_warp_plan);
        let epoch = hot.epoch.to_le_bytes();
        let epoch_low = u32::from_le_bytes([epoch[0], epoch[1], epoch[2], epoch[3]]);
        let epoch_high = u32::from_le_bytes([epoch[4], epoch[5], epoch[6], epoch[7]]);
        let uniform = HotUniform {
            plane_u: hot.plane.basis_u,
            plane_v: hot.plane.basis_v,
            view_rotation: rotation,
            homography_row_0: plan.rows[0],
            homography_row_1: plan.rows[1],
            homography_row_2: plan.rows[2],
            clear_rgba: selected.1.clear_rgba,
            flags: [
                epoch_low,
                epoch_high,
                u32::from(plan.source_valid),
                self.main
                    .as_ref()
                    .map_or(ViewMode::Flat as u32, |main| main.view as u32),
            ],
        };
        let offset = u64::from(slot.index() * self.gpu.hot_stride);
        self.queue
            .write_buffer(&self.gpu.hot_buffer, offset, bytemuck::bytes_of(&uniform));
        self.hot[slot.index() as usize] = pose;
        self.facts.centre_from_reference_px = hot.state.centre_from_reference_px;
        self.facts.chart_residual = plan.source_valid.then_some(plan.chart_residual);
        self.facts.tumbled_max_error_px = plan.approx_max_error_px;
        if plan.kind == WarpKind::ClearOnly && self.ledger.retained().is_none() {
            self.facts.status = PresentStatus::WaitingForFirstScene;
        } else if plan.kind == WarpKind::FlatExact
            && pose.is_some_and(|current| {
                self.ledger
                    .retained()
                    .is_some_and(|frame| frame.pose == current)
            })
        {
            self.facts.status = PresentStatus::ShowingCompletedScene;
        } else if plan.source_valid {
            self.facts.status = PresentStatus::ShowingStaleApproximation;
        }
    }

    /// Submits one flat or tumbled scene pass plus its four-byte completion fence.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for absent/invalid state, an occupied target, or checked overflow.
    pub fn submit_scene(&mut self, hot_slot: HotSlot, now_ms: f64) -> Result<u64, PresentError> {
        if let Some(pending) = self.ledger.pending() {
            return Err(PresentError::SceneBusy {
                scene_id: pending.scene_id,
            });
        }
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
        let scene_id = self.next_scene_id;
        self.next_scene_id = scene_id.checked_add(1).ok_or(PresentError::Device {
            operation: "advance scene identity",
        })?;
        let texture_index = self.ledger.begin(crate::state::PendingScene {
            scene_id,
            pose,
            palette: palette_id,
            iteration_cap: main.state.delivered_iter_cap,
            level: main.grid.level,
            extent: [main.grid.width, main.grid.height],
            texture_index: 0,
            centre_revision: main.state.centre_revision,
            plane_origin_f64: main.state.plane_origin_f64,
            drop_reason: None,
        })?;
        let extent = [main.grid.width, main.grid.height];
        let reallocated =
            ensure_scene_texture(&self.device, &mut self.gpu, texture_index as usize, extent)?;
        if reallocated {
            self.facts.texture_reallocations = self.facts.texture_reallocations.saturating_add(1);
            self.scene_samples.reset();
            self.warp_samples.reset();
        }
        if main.view == ViewMode::Tumbled {
            ensure_indices(&self.device, &mut self.gpu, extent)?;
            ensure_depth(&self.device, &mut self.gpu, extent)?;
        }
        let uniform = SceneUniform::new(
            extent,
            main.grid.level as u32,
            main.state.delivered_iter_cap,
            main.grid.span.directory_index,
            main.grid.span.logical_len,
            palette_record,
        )
        .map_err(|_| PresentError::InvalidGrid {
            width: extent[0],
            height: extent[1],
            logical_len: main.grid.span.logical_len,
        })?;
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
            main.view,
            hot_slot.index() * self.gpu.hot_stride,
            palette_record.clear_rgba,
        );
        encoder.clear_buffer(&self.gpu.scene_fence, 0, Some(FENCE_BYTES));
        self.queue.submit([encoder.finish()]);
        self.scene_fence = Some(arm_fence(
            &self.gpu.scene_fence,
            FenceLedger::new(
                SubmissionKind::Scene,
                scene_id,
                None,
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
        let warp_id = self.next_warp_id;
        self.next_warp_id = warp_id.checked_add(1).ok_or(PresentError::Device {
            operation: "advance warp identity",
        })?;
        let source = self.ledger.retained();
        let source_scene_id = source.map(|frame| frame.scene_id);
        let texture_index = source.map_or(0, |frame| frame.texture_index as usize);
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
            pass.set_bind_group(
                1,
                &self.gpu.warp_hot_group,
                &[hot_slot.index() * self.gpu.hot_stride],
            );
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
                match self.ledger.complete(measurement)? {
                    SceneCompletion::Promoted(frame) => {
                        self.publish_promoted(&frame);
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
                })
            }
        }
    }

    fn publish_promoted(&mut self, frame: &crate::SceneFrame) {
        self.replaced_warp_scene = self.active_warp_scene;
        self.facts.reprojected_per_scene = self.active_warp_scene.map(|_| self.active_warp_count);
        self.active_warp_scene = Some(frame.scene_id);
        self.active_warp_count = 0;
        self.facts.completed_scene_id = Some(frame.scene_id);
        self.facts.source_generation = Some(frame.pose.orbit_generation);
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
    let hot_stride = hot_stride(config.min_uniform_buffer_offset_alignment).map_err(|_| {
        PresentError::Device {
            operation: "compute HOT ring stride",
        }
    })?;
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
        size: 80,
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
                    size: NonZeroU64::new(80),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &hot_buffer,
                    offset: 0,
                    size: NonZeroU64::new(128),
                }),
            },
        ],
    });
    let warp_texture_layout = create_warp_texture_layout(device);
    let warp_hot_layout = create_warp_hot_layout(device);
    let warp_hot_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Julibrot immutable warp HOT group"),
        layout: &warp_hot_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &hot_buffer,
                offset: 0,
                size: NonZeroU64::new(128),
            }),
        }],
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
    let sources = scene_shaders(heap_limits);
    let flat_pipeline = create_scene_pipeline(
        device,
        "Julibrot flat scene pipeline",
        &sources.flat,
        "flat_vertex",
        "flat_fragment",
        &heap_layout,
        &scene_layout,
        false,
    );
    let tumbled_pipeline = create_scene_pipeline(
        device,
        "Julibrot tumbled scene pipeline",
        &sources.tumbled,
        "tumbled_vertex",
        "tumbled_fragment",
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
        flat_pipeline,
        tumbled_pipeline,
        warp_pipeline,
        scene_fence: create_fence(device, "Julibrot scene four-byte fence"),
        warp_fence: create_fence(device, "Julibrot warp four-byte fence"),
        hot_stride,
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
        entries: &[uniform_entry(0, false, 80), uniform_entry(1, true, 128)],
    })
}

fn create_warp_hot_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot warp HOT layout"),
        entries: &[uniform_entry(0, true, 128)],
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
        label: Some("Julibrot tumbled depth target"),
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
    let values = tumbled_indices(extent).map_err(|_| PresentError::IndexCountOverflow {
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
        label: Some("Julibrot tumbled u32 index buffer"),
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
    view: ViewMode,
    hot_offset: u32,
    clear: [f32; 4],
) {
    let depth_attachment =
        (view == ViewMode::Tumbled).then_some(wgpu::RenderPassDepthStencilAttachment {
            view: &gpu.depth.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Julibrot selected scene pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &gpu.scene_textures[texture_index].view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color(clear)),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth_attachment,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    pass.set_bind_group(1, &gpu.scene_group, &[hot_offset]);
    match view {
        ViewMode::Flat => {
            pass.set_pipeline(&gpu.flat_pipeline);
            pass.draw(0..3, 0..1);
        }
        ViewMode::Tumbled => {
            pass.set_pipeline(&gpu.tumbled_pipeline);
            if let Some(indices) = &gpu.indices {
                pass.set_index_buffer(indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..indices.count, 0, 0..1);
            }
        }
    }
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
        && [
            pose.plane_theta_1,
            pose.plane_theta_2,
            pose.zoom_log2,
            pose.view_theta_1,
            pose.centre_from_reference_px[0],
            pose.centre_from_reference_px[1],
        ]
        .into_iter()
        .all(f64::is_finite)
        && pose
            .plane
            .basis_u
            .into_iter()
            .chain(pose.plane.basis_v)
            .all(f32::is_finite)
}

const fn clear_warp_plan() -> crate::WarpPlan {
    crate::WarpPlan {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        source_valid: false,
        kind: WarpKind::ClearOnly,
        chart_residual: 0.0,
        approx_max_error_px: None,
        approx_p95_error_px: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let plan = clear_warp_plan();
        assert_eq!(plan.kind, WarpKind::ClearOnly);
        assert!(!plan.source_valid);
        assert_eq!(plan.rows[2], [0.0, 0.0, 1.0, 0.0]);
    }
}
