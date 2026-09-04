use super::*;

impl BrowserFrameLoop {
    pub(super) fn prepare_due_level(&mut self) -> bool {
        if self.active_backdrop_map.is_some() {
            return false;
        }
        let Some(level) = self.loop_state.due() else {
            return false;
        };
        if self.prepared_level == Some(level) {
            return false;
        }
        self.prepared_level = Some(level);
        level == RefinementLevel::Final
    }

    pub(super) fn install_main(
        &mut self,
        viewer: &ViewerController,
        object: ObjectAngles,
        plane: Plane,
        map: PoseMap,
    ) {
        let current_stamp = self.view_stamp(viewer);
        let ready_backdrop = self.backdrop.as_ref().and_then(|backdrop| {
            backdrop.ready.filter(|ready| {
                ready.stamp.render_equivalent(current_stamp) && self.active_backdrop_map.is_none()
            })
        });
        let (grid, source_map, requested_width, delivered_iter_cap, backdrop) =
            if let Some(source_map) = self.active_backdrop_map {
                let backdrop = self
                    .backdrop
                    .as_ref()
                    .expect("active backdrop map always owns a grid");
                let mut grid = backdrop.grid.clone();
                let final_spec = backdrop.plan.level(RefinementLevel::Final);
                grid.width = final_spec.extent.width;
                grid.height = final_spec.extent.height;
                grid.level = super::BACKDROP_PRESENT_LEVEL;
                (
                    grid,
                    source_map,
                    backdrop.plan.requested_extent.width,
                    super::published_iteration_cap(&backdrop.plan),
                    None,
                )
            } else {
                let mut grid = self.grid.clone();
                if let Some(level) = self.prepared_level {
                    let spec = self.plan.level(level);
                    grid.width = spec.extent.width;
                    grid.height = spec.extent.height;
                    grid.level = level;
                }
                let backdrop = ready_backdrop.map(|ready| {
                    let stored = self
                        .backdrop
                        .as_ref()
                        .expect("ready backdrop always owns a grid");
                    PresentBackdrop {
                        grid: stored.grid.clone(),
                        iteration_cap: super::published_iteration_cap(&stored.plan),
                        plane,
                        map: ready.map,
                    }
                });
                (
                    grid,
                    map,
                    self.plan.requested_extent.width,
                    super::published_iteration_cap(&self.plan),
                    backdrop,
                )
            };
        self.main.delivered_iter_cap = delivered_iter_cap;
        let facts_pose = (map, [grid.width, grid.height]);
        if self.facts_pose != facts_pose {
            let horizon = super::horizon_facts(map, facts_pose.1);
            self.horizon_pixels = horizon.pixels;
            self.horizon_fraction = horizon.fraction;
            self.uncertain_pixels = horizon.uncertain_pixels;
            self.uncertain_fraction = horizon.uncertain_fraction;
            self.map_condition_number = horizon.condition_number;
            self.edge_on = horizon.edge_on;
            self.facts_pose = facts_pose;
        }
        self.presenter.set_main(PresentMain {
            epoch: self.owner_epoch,
            state: super::main_for_grid(self.main, grid.width, requested_width),
            grid,
            object,
            plane,
            map: source_map,
            backdrop,
        });
    }

    pub(super) fn observe_scene_selection(&mut self, viewer: &ViewerController) {
        let selection = SceneSelection {
            generation: self.main.generation_applied,
            requested_iter_cap: self.main.requested_iter_cap,
            palette_id: self.main.palette_id,
            plane_origin_f64: self.main.plane_origin_f64,
            precision_mode: self.main.precision_mode,
        };
        if self.scene_ready(viewer.requested().zoom_log2)
            && self.submitted_references.is_empty()
            && viewer.owner().navigation_pending_depth() == 0
            && self
                .scene_selection
                .is_some_and(|previous| previous != selection)
        {
            self.loop_state
                .scene_selection_changed(self.main.generation_applied);
            self.prepared_level = None;
        }
        self.scene_selection = Some(selection);
    }

    /// Stamps the view the next presented image is expected to reproduce.
    pub(super) fn view_stamp(&self, viewer: &ViewerController) -> ViewStamp {
        let requested = viewer.requested();
        ViewStamp {
            generation_applied: self.main.generation_applied,
            centre_revision: self.main.centre_revision,
            requested_iter_cap: self.main.requested_iter_cap,
            palette_id: self.main.palette_id,
            plane_origin_f64: self.main.plane_origin_f64,
            view: requested.view,
            zoom_log2: requested.zoom_log2,
            object_angles: requested.object_angles,
            map: super::stamped_screen_map(viewer, &self.plan),
            precision_mode: self.main.precision_mode,
        }
    }

    /// Reports whether the image on the canvas belongs to an older requested view.
    ///
    /// Nothing has been presented before the first warp completes, so an unstarted page reads
    /// stale, which is the honest answer: it is showing no view at all.
    #[must_use]
    pub fn presented_view_is_stale(&self, viewer: &ViewerController) -> bool {
        let current = self.view_stamp(viewer);
        self.presented_view
            .is_none_or(|presented| !presented.render_equivalent(current))
    }

    pub(super) fn prepared_extent(&self) -> [u32; 2] {
        if self.active_backdrop_map.is_some()
            && let Some(backdrop) = &self.backdrop
        {
            let final_spec = backdrop.plan.level(RefinementLevel::Final);
            return [final_spec.extent.width, final_spec.extent.height];
        }
        self.prepared_level
            .map_or([self.grid.width, self.grid.height], |level| {
                let extent = self.plan.level(level).extent;
                [extent.width, extent.height]
            })
    }

    pub(super) fn rebuild_grid_if_needed(
        &mut self,
        requested_max_iter: u32,
    ) -> Result<(), AppError> {
        if self.plan.requested_max_iter == requested_max_iter {
            return Ok(());
        }
        self.release_backdrop()?;
        let requested_extent = self.plan.requested_extent;
        let next = JulibrotKernels::plan(
            &self.executor,
            requested_extent,
            EscapeParams::new(requested_max_iter),
        )
        .map_err(kernel_error)?
        .with_precision_mode(self.precision_mode);
        if next == self.plan {
            return Ok(());
        }
        let next_grid = self
            .kernels
            .allocate_grid(&mut self.executor, &next)
            .map_err(kernel_error)?;
        let old_grid = std::mem::replace(&mut self.grid, next_grid);
        self.kernels
            .free_grid(&mut self.executor, old_grid)
            .map_err(kernel_error)?;
        self.plan = next;
        Ok(())
    }

    pub(super) fn synchronize_precision_mode(
        &mut self,
        viewer: &mut ViewerController,
    ) -> Result<(), AppError> {
        let next = viewer.requested().precision_mode;
        if next == self.precision_mode {
            return Ok(());
        }
        self.release_backdrop()?;
        let next_plan = self.plan.with_precision_mode(next);
        let next_grid = self
            .kernels
            .allocate_grid(&mut self.executor, &next_plan)
            .map_err(kernel_error)?;
        if let Err(error) = super::super::apply_precision_mode(
            next,
            &mut self.precision_mode,
            &mut self.loop_state,
            &mut self.plan,
            viewer,
        ) {
            self.kernels
                .free_grid(&mut self.executor, next_grid)
                .map_err(kernel_error)?;
            return Err(error);
        }
        let old_grid = std::mem::replace(&mut self.grid, next_grid);
        self.kernels
            .free_grid(&mut self.executor, old_grid)
            .map_err(kernel_error)?;
        self.prepared_level = None;
        self.scene_selection = None;
        Ok(())
    }

    pub(super) fn submit_due_scene(
        &mut self,
        viewer: &ViewerController,
        object: ObjectAngles,
        plane: Plane,
        map: PoseMap,
        centre_from_reference_px: [f64; 2],
        slot: HotSlot,
        owner_epoch: u64,
        now_ms: f64,
    ) -> Result<Option<u64>, AppError> {
        if matches!(map, PoseMap::Mapped(_))
            && (!self.submitted_references.is_empty()
                || viewer.owner().navigation_pending_depth() != 0)
        {
            return Ok(None);
        }
        if !self.scene_ready(viewer.requested().zoom_log2) {
            return Ok(None);
        }
        if self.active_backdrop_map.is_some() {
            return self.submit_due_backdrop(viewer, plane, slot, owner_epoch, now_ms);
        }
        let Some(level) = self.loop_state.due() else {
            return Ok(None);
        };
        if self.prepared_level != Some(level) {
            return Ok(None);
        }
        if matches!(map, PoseMap::EdgeOn) {
            super::stamp_scene_level(&mut self.grid, &self.plan, level);
        }
        let facts = if let PoseMap::Mapped(screen_to_plane) = map {
            let params = EscapeParams::new(viewer.requested().iteration_cap);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Julibrot kernels SCRATCH and DATA copy"),
                });
            let mode = KernelMode::for_zoom(viewer.requested().zoom_log2);
            let facts = match mode {
                KernelMode::Shallow => {
                    let centre = self
                        .shallow_centre
                        .as_ref()
                        .ok_or_else(|| AppError::Kernel("missing shallow centre".to_string()))?;
                    let split = split_centre(centre).map_err(math_error)?;
                    let scale = shallow_pixel_scale(
                        viewer.requested().zoom_log2,
                        self.plan.level(level).extent.width,
                    )
                    .map_err(math_error)?;
                    self.kernels
                        .encode_shallow(
                            &self.executor,
                            &mut encoder,
                            &mut self.grid,
                            owner_epoch,
                            viewer.requested().precision_mode,
                            level,
                            &plane,
                            &screen_to_plane,
                            &split,
                            scale,
                            params,
                        )
                        .map_err(kernel_error)?
                }
                KernelMode::Perturbation => {
                    let handle = self
                        .current_orbit
                        .ok_or_else(|| AppError::Kernel("missing reference orbit".to_string()))?;
                    let orbit = self.orbits.get(handle).map_err(registry_error)?;
                    let scale = scale_split(
                        viewer.requested().zoom_log2,
                        self.plan.level(level).extent.width,
                    )
                    .map_err(math_error)?;
                    self.kernels
                        .encode_perturbation(
                            &self.executor,
                            &mut encoder,
                            &mut self.grid,
                            owner_epoch,
                            viewer.requested().precision_mode,
                            level,
                            &plane,
                            &screen_to_plane,
                            centre_from_reference_px,
                            scale,
                            params,
                            ReferenceOrbitInput {
                                span: &orbit.span,
                                generation: handle.generation,
                                length: orbit.length,
                                precision_bits: orbit.precision_bits,
                                precision_mode: orbit.precision_mode,
                            },
                        )
                        .map_err(kernel_error)?
                }
            };
            self.queue.submit([encoder.finish()]);
            Some(facts)
        } else {
            None
        };
        self.main.delivered_iter_cap = super::published_iteration_cap(&self.plan);
        self.install_main(viewer, object, plane, map);
        match self.presenter.submit_scene(slot, now_ms) {
            Ok(scene_id) => {
                self.last_dispatch = facts;
                self.level_timings
                    .begin_scene(self.main.centre_revision, scene_id, level);
                self.loop_state.submitted(scene_id, level);
                Ok(Some(scene_id))
            }
            Err(ember_julibrot_present::PresentError::SceneBusy { .. }) => Ok(None),
            Err(error) => Err(present_error(error)),
        }
    }
}
