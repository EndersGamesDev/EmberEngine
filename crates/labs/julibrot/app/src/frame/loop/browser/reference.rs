use super::*;

fn expand_reference_texels_from_array(
    records: &js_sys::Uint8Array,
    length: u32,
    texels: &mut Vec<u8>,
) -> Result<(), AppError> {
    let count = usize::try_from(length)
        .map_err(|_| AppError::Worker("reference length does not fit usize".to_string()))?;
    let expected = count
        .checked_mul(super::super::REFERENCE_RECORD_BYTES)
        .ok_or_else(|| AppError::Worker("reference record byte length overflow".to_string()))?;
    if usize::try_from(records.length()).ok() != Some(expected) {
        return Err(AppError::Worker(format!(
            "reference payload has {} bytes; expected {expected}",
            records.length()
        )));
    }
    let texel_bytes = super::super::reference_texel_bytes(length)?;
    if texels.capacity() < texel_bytes {
        texels
            .try_reserve_exact(texel_bytes.saturating_sub(texels.len()))
            .map_err(|error| {
                AppError::Worker(format!("reference upload reserve failed: {error}"))
            })?;
    }
    texels.resize(texel_bytes, 0);
    records.copy_to(&mut texels[..expected]);
    for index in (0..count).rev() {
        let source = index * super::super::REFERENCE_RECORD_BYTES;
        let destination = index * super::super::REFERENCE_TEXEL_BYTES;
        texels.copy_within(
            source..source + super::super::REFERENCE_RECORD_BYTES,
            destination,
        );
        texels[destination + super::super::REFERENCE_RECORD_BYTES
            ..destination + super::super::REFERENCE_TEXEL_BYTES]
            .fill(0);
    }
    Ok(())
}

fn viewer_precision_mode(value: u32) -> &'static str {
    PrecisionMode::from_u32(value).map_or("unavailable", PrecisionMode::as_str)
}

impl BrowserFrameLoop {
    /// Requests one reference at a completed grid's best census candidate.
    ///
    /// A reference whose own orbit ends before a level's cap turns every longer-lived record of
    /// that level into a glitch, so a short accepted reference is replaced by the record the
    /// census ranked highest: one that never escaped within the completed grid's cap where the
    /// grid holds one, a glitch that exhausted its reference next, then the longest-lived
    /// escaping record, and a glitch from arithmetic failure last, ties broken by the lowest
    /// index. The request is made only when the completed grid's cap is strictly longer than
    /// the accepted orbit, because only then can its top-ranked record name a longer orbit at
    /// all. The navigation centre never moves; only the orbit point does.
    ///
    /// One request is issued per accepted orbit: a ladder whose Interactive and Final caps both
    /// outlast the same short reference would otherwise spend two of the bounded slots to buy
    /// one correction, because the second request only supersedes the first. At most
    /// [`super::super::SAMPLED_REFERENCE_LIMIT`] requests per accepted navigation, so a grid whose
    /// glitches have another cause cannot drive a chase, and the top rank is a heuristic rather
    /// than a proof, which is why the arrival keeps the longest orbit rather than the newest.
    pub(super) fn maybe_request_sampled_reference(
        &mut self,
        viewer: &mut ViewerController,
        frame: &ember_julibrot_present::SceneFrame,
        candidate: Option<u32>,
    ) {
        let Some(index) = candidate else {
            return;
        };
        if !super::super::sampled_reference_due(
            KernelMode::for_zoom(viewer.requested().zoom_log2) == KernelMode::Perturbation,
            frame.iteration_cap,
            self.main.orbit_length,
            self.sampled_references,
            self.sampled_request_at_length,
        ) {
            return;
        }
        if viewer
            .request_reference_for_pixel(index, frame.extent)
            .is_ok()
        {
            self.sampled_references = self.sampled_references.saturating_add(1);
            self.sampled_request_at_length = Some(self.main.orbit_length);
            self.sampled_resume_level = Some(frame.level);
        }
    }

    /// Returns how many reference requests the census candidate has driven for this navigation.
    #[must_use]
    pub const fn sampled_reference_requests(&self) -> u32 {
        self.sampled_references
    }

    /// Returns how many of those requests were accepted, each costing one resumed ladder.
    #[must_use]
    pub const fn sampled_reference_rounds(&self) -> u32 {
        self.sampled_reference_rounds
    }

    /// Returns how many sampled arrivals were discarded for not lengthening the accepted orbit.
    #[must_use]
    pub const fn sampled_reference_discards(&self) -> u32 {
        self.sampled_reference_discards
    }

    pub(super) fn service_arrivals(
        &mut self,
        viewer: &mut ViewerController,
        now_ms: f64,
    ) -> Result<bool, AppError> {
        let mut applied = false;
        for _ in 0..2 {
            let Some(mut response) = self.owner_endpoint.next_arrival() else {
                break;
            };
            let generation = response.generation();
            let _finished = viewer.finish_reference_submission(generation);
            let submitted = self
                .submitted_references
                .iter()
                .position(|item| item.generation == generation)
                .map(|index| self.submitted_references.swap_remove(index));
            let processed = self.process_arrival(viewer, &response, submitted);
            let disposition = processed
                .as_ref()
                .map_or(OrbitDisposition::Stale, |result| result.0);
            let credited = self
                .owner_endpoint
                .return_credit(&mut response, disposition, now_us(now_ms))
                .map_err(worker_error);
            let (_, arrival_applied) = processed?;
            credited?;
            applied |= arrival_applied;
        }
        Ok(applied)
    }

    fn process_arrival(
        &mut self,
        viewer: &mut ViewerController,
        response: &ember_julibrot_worker::OrbitResponseView,
        submitted: Option<SubmittedReference>,
    ) -> Result<(OrbitDisposition, bool), AppError> {
        let Some(submitted) = submitted.filter(|submitted| {
            submitted.precision_mode == viewer.requested().precision_mode.as_str()
                && super::super::arrival_is_current(
                    response.cancelled(),
                    response.generation(),
                    self.owner_endpoint.latest_generation(),
                    viewer.owner().navigation_pending_depth(),
                )
        }) else {
            return Ok((OrbitDisposition::Stale, false));
        };
        if submitted.sampled && response.length() <= self.main.orbit_length {
            self.sampled_reference_discards = self.sampled_reference_discards.saturating_add(1);
            self.sampled_resume_level = None;
            return Ok((OrbitDisposition::Stale, false));
        }
        let records = response
            .records
            .transfer_record_bytes()
            .map_err(worker_error)?;
        expand_reference_texels_from_array(
            &records,
            response.length(),
            &mut self.reference_upload,
        )?;
        let span = self
            .executor
            .allocate_span(response.length(), OUTPUT_PAGE_SIDE)
            .map_err(heap_error)?;
        if let Err(error) = self.executor.write_span(&span, &self.reference_upload) {
            let _freed = self.executor.free_span(span);
            return Err(heap_error(error));
        }
        let registered = RegisteredOrbit {
            span: span.clone(),
            length: response.length(),
            precision_bits: response.precision_bits(),
            precision_mode: submitted.precision_mode,
        };
        let handle = match self.orbits.insert(response.generation(), registered) {
            Ok(handle) => handle,
            Err(error) => {
                let _freed = self.executor.free_span(span);
                return Err(registry_error(error));
            }
        };
        let shift = match self
            .accepted_reference
            .as_ref()
            .map_or(Ok([0.0; 2]), |old| {
                let old = old.with_precision(submitted.reference_centre.precision_bits)?;
                reference_shift_px(
                    &old,
                    &submitted.reference_centre,
                    &submitted.plane,
                    submitted.zoom_log2,
                    self.plan.requested_extent.width,
                )
            }) {
            Ok(shift) => shift,
            Err(error) => {
                self.remove_orbit(handle)?;
                return Err(math_error(error));
            }
        };
        if let Err(error) = viewer.configure_navigation_context(
            submitted.view_centre,
            submitted.reference_centre.clone(),
            submitted.plane,
        ) {
            self.remove_orbit(handle)?;
            return Err(error);
        }
        let disposition = viewer.owner_mut().accept_orbit(response, handle, shift);
        if disposition == OrbitDisposition::Stale {
            self.remove_orbit(handle)?;
            return Ok((disposition, false));
        }
        self.replace_current_orbit(handle)?;
        self.accepted_reference = Some(submitted.reference_centre);
        self.accepted_reference_zoom_log2 = Some(submitted.zoom_log2);
        self.sampled_request_at_length = None;
        if submitted.sampled {
            self.sampled_reference_rounds = self.sampled_reference_rounds.saturating_add(1);
        } else {
            self.sampled_references = 0;
            self.sampled_reference_rounds = 0;
            self.sampled_reference_discards = 0;
            self.sampled_resume_level = None;
        }
        self.main = viewer.drain_main()?.main;
        self.level_timings
            .record_worker(response.centre_revision(), response.compute_us(), None);
        self.rebuild_grid_if_needed(viewer.requested().iteration_cap)?;
        match self
            .sampled_resume_level
            .take()
            .filter(|_| submitted.sampled)
        {
            Some(level) => self
                .loop_state
                .scene_input_resumed(response.generation(), level),
            None => self.loop_state.scene_input_ready(response.generation()),
        }
        self.prepared_level = None;
        let requested = viewer.requested();
        let map = viewer.screen_map(self.prepared_extent())?;
        let plane = viewer.checked_plane();
        self.install_main(viewer, requested.object_angles, plane, map);
        Ok((disposition, true))
    }

    /// Releases every reference whose typed worker refusal means no arrival can land.
    ///
    /// Without this the scene path stays blocked on a submission that will never return.
    pub(super) fn abandon_submitted_references(&mut self, viewer: &mut ViewerController) {
        for submitted in std::mem::take(&mut self.submitted_references) {
            let _finished = viewer.finish_reference_submission(submitted.generation);
        }
    }

    pub(super) fn submit_pending_reference(
        &mut self,
        viewer: &mut ViewerController,
        plane: Plane,
    ) -> Result<bool, AppError> {
        let Some(submission) = viewer.take_reference_submission() else {
            return Ok(false);
        };
        let navigation = submission.navigation;
        let sampled = submission.reference_centre != navigation.centre;
        let requested = viewer.requested();
        if KernelMode::for_zoom(navigation.zoom_log2) == KernelMode::Shallow {
            if !viewer
                .owner_mut()
                .accept_navigation_without_orbit(navigation.generation, navigation.centre_revision)
            {
                return Ok(false);
            }
            self.shallow_centre = Some(navigation.centre);
            self.main = viewer.drain_main()?.main;
            self.rebuild_grid_if_needed(requested.iteration_cap)?;
            self.loop_state.scene_input_ready(navigation.generation);
            self.prepared_level = None;
            let map = viewer.screen_map(self.prepared_extent())?;
            self.install_main(viewer, requested.object_angles, plane, map);
            return Ok(true);
        }
        let precision = precision_for(
            navigation.zoom_log2,
            self.plan.requested_extent.width,
            requested.iteration_cap,
        )
        .map_err(math_error)?;
        let centre =
            EncodedCentre::encode_math(&submission.reference_centre, navigation.centre_revision)
                .map_err(worker_error)?;
        let request = OrbitRequest::new(
            navigation.generation,
            centre,
            depth_digits(navigation.zoom_log2),
            precision.requested_bits,
            requested.iteration_cap,
            PrecisionMode::from_u32(navigation.precision_mode).ok_or_else(|| {
                AppError::Worker(format!(
                    "precision mode {} is outside 0..1",
                    navigation.precision_mode
                ))
            })?,
            submission.reason,
        )
        .map_err(worker_error)?;
        let required_upload = super::super::reference_texel_bytes(requested.iteration_cap)?;
        if self.reference_upload.capacity() < required_upload {
            self.reference_upload
                .try_reserve_exact(required_upload.saturating_sub(self.reference_upload.len()))
                .map_err(|error| {
                    AppError::Worker(format!("reference upload reserve failed: {error}"))
                })?;
        }
        if self.owner_endpoint.submit(request) == SubmitOutcome::GenerationExhausted {
            let _finished = viewer.finish_reference_submission(navigation.generation);
            return Err(AppError::GenerationExhausted);
        }
        self.submitted_references.push(SubmittedReference {
            generation: navigation.generation,
            view_centre: navigation.centre,
            reference_centre: submission.reference_centre,
            sampled,
            zoom_log2: navigation.zoom_log2,
            plane,
            precision_mode: viewer_precision_mode(navigation.precision_mode),
        });
        Ok(false)
    }

    pub(super) fn scene_ready(&self, zoom_log2: f64) -> bool {
        match KernelMode::for_zoom(zoom_log2) {
            KernelMode::Shallow => self.shallow_centre.is_some(),
            KernelMode::Perturbation => super::super::perturbation_reference_is_current(
                zoom_log2,
                self.main.generation_applied,
                self.current_orbit
                    .zip(self.accepted_reference_zoom_log2)
                    .map(|(handle, reference_zoom_log2)| (handle.generation, reference_zoom_log2)),
            ),
        }
    }

    fn replace_current_orbit(&mut self, next: OrbitHandle) -> Result<(), AppError> {
        if let Some(previous) = self.current_orbit.replace(next) {
            self.remove_orbit(previous)?;
        }
        Ok(())
    }

    fn remove_orbit(&mut self, handle: OrbitHandle) -> Result<(), AppError> {
        let orbit = self.orbits.remove(handle).map_err(registry_error)?;
        self.executor.free_span(orbit.span).map_err(heap_error)
    }
}
