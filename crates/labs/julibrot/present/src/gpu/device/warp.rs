use crate::fence::FenceLedger;
use crate::{
    FrameReceipt, FrameState, HotSlot, HotUniform, PaletteId, PaletteRecord, Pose, PoseMap,
    PresentError, PresentHot, PresentMain, PresentStatus, RefinementLevel, SubmissionKind, Warp,
    WarpKind, WarpValidation, camera_rotation, camera_rotation_pairs, camera_translation,
    exterior_zero, pack_homography_rows, palette, view_scale, warp_shader,
};
use ember_julibrot_math::scene_uncovered_fraction;

use super::{
    FENCE_BYTES, GpuState, HOT_HOMOGRAPHY_BYTE_OFFSET, HOT_SOURCE_VALID_BYTE_OFFSET, Presenter,
    SCENE_GRID_BYTE_OFFSET, apply_hold_policy, arm_fence, clear_warp_plan, encode_relief_redraw,
    identity_rows, pose_is_finite, warp_exposed_fraction,
};

impl Presenter {
    /// Writes exactly one 288-byte HOT payload into the checked three-slot ring.
    ///
    /// When `hold_refused_warp` is true, a clear-only plan with a retained picture becomes a
    /// `HoldStale` plan whose rows carry the ratio between the held picture's extent and the
    /// destination lattice, so the app cannot replace that picture with a disallowed clear.
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
        let selected = selected_or_classic(self.main.as_ref());
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
        let plan = if plan.kind == WarpKind::ReliefRedraw
            && self.ledger.retained().is_none_or(|source| {
                !self.retained_records_support_relief_redraw(source, selected.1)
            }) {
            clear_warp_plan(plan.edge_on, true)
        } else {
            plan
        };
        let hold_source = self
            .ledger
            .retained()
            .or_else(|| self.ledger.held().map(|held| &held.frame));
        // The lattice the plan rows are expressed against: the pose this HOT payload publishes,
        // whose grid is the current MAIN grid. A hold from a picture drawn at another extent is
        // scaled onto this lattice rather than laid on it pixel for pixel.
        let plan = apply_hold_policy(
            plan,
            hold_source,
            hold_refused_warp,
            destination_extent(pose.as_ref(), self.main.as_ref()),
        );
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
        self.hot_warp_source[slot.index() as usize].write_hot(&plan, hold_refused_warp);
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
        let exposed_fraction = planned_exposed_fraction(
            &plan,
            pose.as_ref(),
            self.ledger
                .retained()
                .or_else(|| self.ledger.held().map(|held| &held.frame)),
        );
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
            .accepted_frame(self.ledger.retained(), self.ledger.held())
            .map(|frame| (frame.level, self.hot_exposed[slot.index() as usize]))
    }

    /// Reports whether this HOT slot selects a retained-record relief redraw.
    #[must_use]
    pub fn accepted_relief_redraw(&self, slot: HotSlot) -> bool {
        self.hot_warp_source[slot.index() as usize]
            .relief_frame(self.ledger.retained(), self.ledger.held())
            .is_some()
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
        let warp_destination_extent = destination_extent(
            self.hot[hot_slot.index() as usize].as_ref(),
            self.main.as_ref(),
        );
        let source_slot = self.hot_warp_source[hot_slot.index() as usize];
        let source = source_slot
            .frame(self.ledger.retained(), self.ledger.held())
            .cloned();
        let mut source_scene_id = source.as_ref().map(|frame| frame.scene_id);
        let mut texture_index = source
            .as_ref()
            .map_or(0, |frame| frame.texture_index as usize);
        let planned_relief_redraw = source_slot
            .relief_frame(self.ledger.retained(), self.ledger.held())
            .is_some();
        if source.is_none() {
            self.clear_hot_source(hot_slot);
        }
        let selected = self
            .main
            .as_ref()
            .and_then(PresentMain::selected_palette)
            .unwrap_or((PaletteId::Classic, palette(PaletteId::Classic)));
        let relief_redraw_backdrop = if planned_relief_redraw {
            let source = source.as_ref().ok_or(PresentError::Device {
                operation: "select relief redraw source",
            })?;
            self.prepare_relief_redraw(
                source,
                [state.canvas_width, state.canvas_height],
                selected.1,
            )?
        } else {
            None
        };
        let relief_redraw = relief_redraw_backdrop.is_some();
        let mut held_stale = source_slot.held_stale;
        if planned_relief_redraw && !relief_redraw {
            let fallback = apply_hold_policy(
                clear_warp_plan(false, true),
                source.as_ref(),
                source_slot.hold_on_redraw_refusal,
                warp_destination_extent,
            );
            self.rewrite_hot_warp(hot_slot, &fallback);
            self.hot_exposed[hot_slot.index() as usize] = fallback.exposed;
            self.facts.record_warp_plan(&fallback, None);
            held_stale = fallback.kind == WarpKind::HoldStale;
            if !fallback.source_valid {
                source_scene_id = None;
                texture_index = 0;
            }
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot warp and fence"),
            });
        if relief_redraw {
            let has_backdrop = relief_redraw_backdrop.unwrap_or(false);
            encode_relief_redraw(
                &mut encoder,
                &self.gpu,
                state.surface_view,
                hot_slot.dynamic_offset(),
                selected.1,
                has_backdrop,
            );
            self.facts.record_relief_redraw();
            let exposed_fraction = self.hot[hot_slot.index() as usize]
                .as_ref()
                .and_then(|pose| {
                    relief_redraw_clear_fraction(pose, self.main.as_ref(), has_backdrop)
                });
            self.facts.record_relief_coverage(exposed_fraction);
        } else {
            self.write_warp_destination_extent(warp_destination_extent);
            encode_image_warp(
                &mut encoder,
                &self.gpu,
                state.surface_view,
                texture_index,
                hot_slot.dynamic_offset(),
                selected.1,
            );
            if held_stale {
                self.facts.record_warp_hold();
            }
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

    /// Publishes the destination lattice the plan rows were built against to the warp fragment.
    ///
    /// The fragment builds its destination point from the chart and `scene.grid.xy`
    /// (`warp_shader.rs` line 20), and that buffer is last written by a scene submission or by a
    /// relief redraw, either of which can name a different extent than the pose this warp is
    /// planned against: a refinement-level change between the last completed scene and the
    /// current grid, or a redraw that names the retained records' extent. Writing the pose's own
    /// grid immediately before the warp submission keeps the fragment's destination lattice the
    /// one the rows assume. Both writers restate the whole uniform before their own pass, so this
    /// cannot reach a scene pass.
    fn write_warp_destination_extent(&self, extent: [u32; 2]) {
        self.queue.write_buffer(
            &self.gpu.scene_buffers[0],
            SCENE_GRID_BYTE_OFFSET,
            bytemuck::bytes_of(&extent),
        );
    }

    fn clear_hot_source(&self, hot_slot: HotSlot) {
        self.queue.write_buffer(
            &self.gpu.hot_buffer,
            u64::from(hot_slot.dynamic_offset()) + HOT_SOURCE_VALID_BYTE_OFFSET,
            bytemuck::bytes_of(&0_u32),
        );
    }

    fn rewrite_hot_warp(&self, hot_slot: HotSlot, plan: &crate::WarpPlan) {
        let offset = u64::from(hot_slot.dynamic_offset());
        self.queue.write_buffer(
            &self.gpu.hot_buffer,
            offset + HOT_HOMOGRAPHY_BYTE_OFFSET,
            bytemuck::bytes_of(&plan.rows),
        );
        self.queue.write_buffer(
            &self.gpu.hot_buffer,
            offset + HOT_SOURCE_VALID_BYTE_OFFSET,
            bytemuck::bytes_of(&u32::from(plan.source_valid)),
        );
    }
}

/// The destination pixel lattice a plan is expressed against: the pose's grid, else MAIN's.
pub(super) fn destination_extent(pose: Option<&Pose>, main: Option<&PresentMain>) -> [u32; 2] {
    pose.map_or_else(
        || main.map_or([0, 0], |main| [main.grid.width, main.grid.height]),
        |pose| [pose.grid_width, pose.grid_height],
    )
}

pub(super) fn planned_exposed_fraction(
    plan: &crate::WarpPlan,
    to_pose: Option<&Pose>,
    source: Option<&crate::SceneFrame>,
) -> Option<f64> {
    if plan.kind == WarpKind::ReliefRedraw {
        return None;
    }
    to_pose.and_then(|to_pose| warp_exposed_fraction(plan, to_pose, source))
}

pub(super) fn relief_redraw_clear_fraction(
    pose: &Pose,
    main: Option<&PresentMain>,
    use_backdrop: bool,
) -> Option<f64> {
    let apron_scale = use_backdrop
        .then(|| {
            main.and_then(|main| main.backdrop.as_ref())
                .and_then(|backdrop| match backdrop.map {
                    PoseMap::Mapped(map) => Some(map.apron_scale),
                    PoseMap::EdgeOn => None,
                })
        })
        .flatten()
        .unwrap_or(1.0);
    scene_uncovered_fraction(
        &pose.object,
        &pose.view,
        pose.grid_width,
        pose.grid_height,
        apron_scale,
    )
    .ok()
}

pub(super) fn create_warp_pipeline(
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

pub(super) fn encode_image_warp(
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

pub(super) fn warp_load_color(selected: PaletteRecord) -> wgpu::Color {
    color(selected.clear_rgba)
}

pub(super) fn selected_or_classic(main: Option<&PresentMain>) -> (PaletteId, PaletteRecord) {
    main.and_then(PresentMain::selected_palette)
        .unwrap_or((PaletteId::Classic, palette(PaletteId::Classic)))
}

pub(super) fn color(rgba: [f32; 4]) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(rgba[0]),
        g: f64::from(rgba[1]),
        b: f64::from(rgba[2]),
        a: f64::from(rgba[3]),
    }
}
