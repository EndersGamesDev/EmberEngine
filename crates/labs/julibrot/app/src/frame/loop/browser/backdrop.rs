use super::*;

impl BrowserFrameLoop {
    pub(super) fn prepare_backdrop(&mut self, viewer: &ViewerController) -> Result<bool, AppError> {
        let final_spec = self.plan.level(RefinementLevel::Final);
        let Some(requested_extent) =
            super::backdrop_extent([final_spec.extent.width, final_spec.extent.height])
        else {
            self.active_backdrop_map = None;
            return Ok(false);
        };
        let Some(mut map) = viewer.backdrop_map(requested_extent)? else {
            self.active_backdrop_map = None;
            return Ok(false);
        };
        if let Some(flight) = self
            .backdrop
            .as_ref()
            .and_then(|backdrop| backdrop.in_flight)
        {
            self.active_backdrop_map = Some(flight.map);
            return Ok(true);
        }
        let stamp = self.view_stamp(viewer);
        if self.backdrop.as_ref().is_some_and(|backdrop| {
            backdrop
                .ready
                .is_some_and(|ready| ready.stamp.render_equivalent(stamp))
        }) {
            self.coverage_turn = super::CoverageTurn::Backdrop;
            self.active_backdrop_map = None;
            return Ok(false);
        }
        let (pre_empts, next_turn) = super::coverage_pre_empts(
            self.coverage_turn,
            self.presenter.facts().in_flight_scene_id.is_some(),
        );
        self.coverage_turn = next_turn;
        if !pre_empts {
            self.active_backdrop_map = None;
            return Ok(false);
        }
        if !self.ensure_backdrop_grid(requested_extent, viewer.requested().iteration_cap)? {
            self.active_backdrop_map = None;
            return Ok(false);
        }
        let delivered_extent = self
            .backdrop
            .as_ref()
            .map(|backdrop| {
                let final_spec = backdrop.plan.level(RefinementLevel::Final);
                [final_spec.extent.width, final_spec.extent.height]
            })
            .ok_or_else(|| AppError::Kernel("backdrop grid was not allocated".to_string()))?;
        if delivered_extent != requested_extent {
            map = viewer.backdrop_map(delivered_extent)?.ok_or_else(|| {
                AppError::Math("delivered backdrop unexpectedly has no apron".to_string())
            })?;
        }
        self.active_backdrop_map = Some(map);
        Ok(true)
    }

    fn ensure_backdrop_grid(
        &mut self,
        requested_extent: [u32; 2],
        requested_iter_cap: u32,
    ) -> Result<bool, AppError> {
        let requested_extent = GridExtent {
            width: requested_extent[0],
            height: requested_extent[1],
        };
        if self.backdrop.as_ref().is_some_and(|backdrop| {
            backdrop.plan.requested_extent == requested_extent
                && backdrop.plan.requested_max_iter == requested_iter_cap
                && backdrop.plan.precision_mode == self.precision_mode
        }) {
            return Ok(true);
        }
        self.release_backdrop()?;
        let Some(plan) = super::optional_backdrop_plan(JulibrotKernels::plan(
            &self.executor,
            requested_extent,
            EscapeParams::new(requested_iter_cap),
        ))?
        else {
            return Ok(false);
        };
        let plan = plan.with_precision_mode(self.precision_mode);
        let grid = match self.kernels.allocate_grid(&mut self.executor, &plan) {
            Ok(grid) => grid,
            Err(KernelError::Heap) => return Ok(false),
            Err(error) => return Err(kernel_error(error)),
        };
        self.backdrop = Some(BackdropGrid {
            plan,
            grid,
            ready: None,
            in_flight: None,
        });
        Ok(true)
    }

    pub(super) fn release_backdrop(&mut self) -> Result<(), AppError> {
        self.active_backdrop_map = None;
        self.coverage_turn = super::CoverageTurn::Backdrop;
        if let Some(backdrop) = self.backdrop.take() {
            if let Err(error) = self.free_grid(&backdrop.grid) {
                self.backdrop = Some(backdrop);
                return Err(error);
            }
        }
        Ok(())
    }

    pub(super) fn submit_due_backdrop(
        &mut self,
        viewer: &ViewerController,
        plane: Plane,
        slot: HotSlot,
        owner_epoch: u64,
        now_ms: f64,
    ) -> Result<Option<u64>, AppError> {
        if self
            .backdrop
            .as_ref()
            .is_some_and(|backdrop| backdrop.in_flight.is_some())
        {
            return Ok(None);
        }
        let PoseMap::Mapped(screen_to_plane) = self
            .active_backdrop_map
            .ok_or_else(|| AppError::Math("backdrop dispatch has no map".to_string()))?
        else {
            return Ok(None);
        };
        let stamp = self.view_stamp(viewer);
        let params = EscapeParams::new(viewer.requested().iteration_cap);
        let sampling_zoom =
            super::sampling_zoom_log2(viewer.requested().zoom_log2, screen_to_plane.apron_scale)?;
        let record_grid = self
            .backdrop
            .as_ref()
            .ok_or_else(|| AppError::Kernel("backdrop dispatch has no grid".to_string()))?
            .grid
            .clone();
        self.presenter.forget_retained_records(&record_grid);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot backdrop kernels SCRATCH and DATA copy"),
            });
        let mode = KernelMode::for_zoom(viewer.requested().zoom_log2);
        let facts = {
            let backdrop = self
                .backdrop
                .as_mut()
                .ok_or_else(|| AppError::Kernel("backdrop dispatch has no grid".to_string()))?;
            let level = RefinementLevel::Final;
            match mode {
                KernelMode::Shallow => {
                    let centre = self
                        .shallow_centre
                        .as_ref()
                        .ok_or_else(|| AppError::Kernel("missing shallow centre".to_string()))?;
                    let split = split_centre(centre).map_err(math_error)?;
                    let scale =
                        shallow_pixel_scale(sampling_zoom, backdrop.plan.level(level).extent.width)
                            .map_err(math_error)?;
                    self.kernels
                        .encode_shallow(
                            &self.executor,
                            &mut encoder,
                            &mut backdrop.grid,
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
                    let scale = scale_split(sampling_zoom, backdrop.plan.level(level).extent.width)
                        .map_err(math_error)?;
                    // The backdrop samples the same view at its own width and apron-widened
                    // zoom, so the drained displacement is re-expressed in this grid's pixels:
                    // a fixed plane offset costs pixels in inverse proportion to pixel scale.
                    let backdrop_pixel =
                        pixel_scale(sampling_zoom, backdrop.plan.level(level).extent.width)
                            .map_err(math_error)?;
                    let requested_pixel = pixel_scale(
                        viewer.requested().zoom_log2,
                        self.plan.requested_extent.width,
                    )
                    .map_err(math_error)?;
                    let ratio = requested_pixel / backdrop_pixel;
                    let centre_from_reference =
                        self.centre_from_reference_px.map(|value| value * ratio);
                    self.kernels
                        .encode_perturbation(
                            &self.executor,
                            &mut encoder,
                            &mut backdrop.grid,
                            owner_epoch,
                            viewer.requested().precision_mode,
                            level,
                            &plane,
                            &screen_to_plane,
                            centre_from_reference,
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
            }
        };
        self.queue.submit([encoder.finish()]);
        match self.presenter.submit_scene(slot, now_ms) {
            Ok(scene_id) => {
                let backdrop = self.backdrop.as_mut().ok_or_else(|| {
                    AppError::Kernel("submitted backdrop lost its grid".to_string())
                })?;
                backdrop.in_flight = Some(BackdropFlight {
                    scene_id,
                    stamp,
                    map: PoseMap::Mapped(screen_to_plane),
                });
                self.last_dispatch = Some(facts);
                self.level_timings.begin_scene(
                    self.main.centre_revision,
                    scene_id,
                    RefinementLevel::Final,
                );
                Ok(Some(scene_id))
            }
            Err(ember_julibrot_present::PresentError::SceneBusy { .. }) => Ok(None),
            Err(error) => Err(present_error(error)),
        }
    }
}
