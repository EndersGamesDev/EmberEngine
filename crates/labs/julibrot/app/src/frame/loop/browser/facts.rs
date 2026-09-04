use super::*;

impl BrowserFrameLoop {
    pub(super) fn main_epoch(&self) -> u64 {
        self.owner_epoch
    }

    /// Returns immutable presentation facts.
    #[must_use]
    pub fn present_facts(&self) -> ember_julibrot_present::PresentFacts {
        self.presenter.facts()
    }

    /// Returns the newest kernel arithmetic receipt.
    #[must_use]
    pub const fn dispatch_facts(&self) -> Option<DispatchFacts> {
        self.last_dispatch
    }

    /// Returns current worker ownership and credit facts.
    #[must_use]
    pub fn worker_facts(&self) -> WorkerFacts {
        self.owner_endpoint.facts()
    }

    /// Returns whether cooperative refresh work remains.
    #[must_use]
    pub fn pending(&self, runtime: &BrowserRuntime, viewer: &ViewerController) -> bool {
        let present = self.presenter.facts();
        self.loop_state.needs_refresh(
            present.in_flight_scene_id.is_some(),
            runtime.has_pending_surface(),
            self.owner_endpoint.pending_request_depth() != 0
                || !self.submitted_references.is_empty()
                || present.scene_fill_due,
            self.presented_view_is_stale(viewer),
        )
    }

    /// Selects automatic or button-driven scene refinement without changing the current pose.
    pub fn set_scene_mode(&mut self, mode: super::SceneMode) {
        let has_scene = self.presenter.facts().completed_scene_id.is_some();
        self.loop_state
            .set_scene_mode(mode, self.main.generation_applied, has_scene);
        self.prepared_level = None;
    }

    /// Returns the current automatic or manual scene policy.
    #[must_use]
    pub const fn scene_mode(&self) -> super::SceneMode {
        self.loop_state.scene_mode()
    }

    /// Reports a manual pose change not yet covered by a completed requested scene.
    #[must_use]
    pub const fn scene_update_pending(&self) -> bool {
        self.loop_state.scene_update_pending()
    }

    /// Returns how many draft levels an accepted better warp made unnecessary.
    #[must_use]
    pub const fn draft_skipped_count(&self) -> u64 {
        self.loop_state.draft_skipped_count()
    }

    /// Returns the stable reason for the most recent direct-to-Final decision.
    #[must_use]
    pub const fn last_draft_skip_reason(&self) -> Option<&'static str> {
        self.loop_state.last_draft_skip_reason()
    }

    /// Returns the count of transient fence refusals this session survived.
    #[must_use]
    pub const fn transient_refusals(&self) -> u32 {
        self.loop_state.transient_refusals()
    }

    /// Returns the newest transient fence refusal, rendered as its typed text.
    #[must_use]
    pub fn last_transient_refusal(&self) -> Option<String> {
        self.loop_state
            .last_transient()
            .map(std::string::ToString::to_string)
    }

    /// Returns the newest transient refusal without allocating display text.
    #[must_use]
    pub const fn last_transient_error(&self) -> Option<&AppError> {
        self.loop_state.last_transient()
    }

    /// Returns the latched stopping refusal without allocating display text.
    #[must_use]
    pub const fn stopped_error(&self) -> Option<&AppError> {
        self.loop_state.stopped()
    }

    /// Returns cached transient-refusal text with no display allocation.
    #[must_use]
    pub fn last_transient_text(&self) -> Option<std::sync::Arc<str>> {
        self.loop_state
            .last_transient_text()
            .map(std::sync::Arc::clone)
    }

    /// Returns cached stopping-refusal text with no display allocation.
    #[must_use]
    pub fn stopped_text(&self) -> Option<std::sync::Arc<str>> {
        self.loop_state.stopped_text().map(std::sync::Arc::clone)
    }

    /// Returns the latched terminal cause once the loop has stopped.
    #[must_use]
    pub fn stopped_reason(&self) -> Option<String> {
        self.loop_state
            .stopped()
            .map(std::string::ToString::to_string)
    }

    /// Returns the status of the most recent completed refresh turn.
    #[must_use]
    pub const fn last_status(&self) -> RefreshStatus {
        self.last_status
    }

    /// Reports whether a progressive level is due or has a scene fence pending.
    #[must_use]
    pub const fn refinement_pending(&self) -> bool {
        self.loop_state.refinement_pending()
    }

    /// Returns the depth of orbit requests main has handed to the producer.
    ///
    /// Without this the page cannot separate a producer that never admits from an app that
    /// never submits: both leave the credit ledger and the worker facts epoch frozen while the
    /// refresh loop keeps turning.
    #[must_use]
    pub fn worker_request_depth(&self) -> u32 {
        self.owner_endpoint.pending_request_depth()
    }

    /// Returns how many submitted references the app is still waiting on.
    #[must_use]
    pub fn outstanding_reference_count(&self) -> u32 {
        u32::try_from(self.submitted_references.len()).unwrap_or(u32::MAX)
    }

    /// Returns the newest generation the app has submitted and not yet seen return.
    #[must_use]
    pub fn outstanding_reference_generation(&self) -> Option<u32> {
        self.submitted_references
            .iter()
            .map(|submitted| submitted.generation)
            .max()
    }

    /// Returns the second-warp policy selected after its labelled warm-up.
    #[must_use]
    pub const fn frame_policy(&self) -> FramePolicy {
        self.frame_policy.policy()
    }

    /// Returns the latest warp source identity.
    #[must_use]
    pub const fn last_warp_source(&self) -> Option<u64> {
        self.last_warp_source
    }

    /// Returns the bounded per-edit timing records in oldest-to-newest order.
    #[must_use]
    pub const fn level_timings(&self) -> &LevelTimingLedger {
        &self.level_timings
    }

    /// Returns the latest post-fence presented zoom, when known.
    #[must_use]
    pub fn last_presented_zoom_log2(&self) -> Option<f64> {
        self.presented_view.map(|stamp| stamp.zoom_log2)
    }

    /// Returns the zoom captured by the currently accepted reference orbit.
    #[must_use]
    pub const fn accepted_reference_zoom_log2(&self) -> Option<f64> {
        self.accepted_reference_zoom_log2
    }

    /// Returns the live capacity-selected progressive plan.
    #[must_use]
    pub const fn plan(&self) -> RefinementPlan {
        self.plan
    }

    /// Returns the applied backdrop scale and its capacity-selected Final extent.
    #[must_use]
    pub fn backdrop_facts(&self, viewer: &ViewerController) -> Option<(f64, [u32; 2])> {
        let backdrop = self.backdrop.as_ref()?;
        let current_stamp = self.view_stamp(viewer);
        let map = self.active_backdrop_map.or_else(|| {
            backdrop
                .ready
                .filter(|ready| ready.stamp.render_equivalent(current_stamp))
                .map(|ready| ready.map)
        })?;
        let PoseMap::Mapped(map) = map else {
            return None;
        };
        let final_spec = backdrop.plan.level(RefinementLevel::Final);
        Some((
            map.apron_scale,
            [final_spec.extent.width, final_spec.extent.height],
        ))
    }

    /// Returns the share of current grid centres beyond the neutral-height horizon.
    #[must_use]
    pub const fn horizon_fraction(&self) -> f64 {
        self.horizon_fraction
    }

    /// Returns the number of current grid centres beyond the neutral-height horizon.
    #[must_use]
    pub const fn horizon_pixels(&self) -> u64 {
        self.horizon_pixels
    }

    /// Returns the share of mapped centres whose f32 position is uncertified.
    #[must_use]
    pub const fn uncertain_fraction(&self) -> f64 {
        self.uncertain_fraction
    }

    /// Returns the number of mapped centres whose f32 position is uncertified.
    #[must_use]
    pub const fn uncertain_pixels(&self) -> u64 {
        self.uncertain_pixels
    }

    /// Reports the physical edge-on all-sky state.
    #[must_use]
    pub const fn edge_on(&self) -> bool {
        self.edge_on
    }

    /// Returns the infinity-norm condition number of the current screen map.
    #[must_use]
    pub const fn map_condition_number(&self) -> f64 {
        self.map_condition_number
    }
}
