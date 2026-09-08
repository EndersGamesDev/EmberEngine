//! Integration runtime and page contract for the Julibrot laboratory.

mod error;
#[cfg(target_arch = "wasm32")]
mod facts;
mod frame;
mod measurement;
mod saved;
mod state;
mod surface;
mod timing;

#[cfg(target_arch = "wasm32")]
mod runtime;

pub use error::AppError;
#[cfg(target_arch = "wasm32")]
pub use facts::PageFacts;
#[cfg(target_arch = "wasm32")]
pub use frame::BrowserFrameLoop;
pub use frame::{RefinementSchedule, SceneMode};
pub use measurement::{
    ADAPTIVE_SAMPLES, ADAPTIVE_WARM_UPS, AdaptivePlan, CONTINUOUS_FRAME_THRESHOLD_MS,
    FrameObservation, FramePolicy, FramePolicyTracker, MAX_ADAPTIVE_REPEATS, MAX_BATCH_MS,
    MeasurementError, SUITE_DEADLINE_MS, SampleSummary, TARGET_TIMER_QUANTA,
    TIMER_PROBE_DEADLINE_MS, TIMER_READ_LIMIT, TIMER_TRANSITION_TARGET, TimerProbeFacts,
    probe_timer,
};
#[cfg(target_arch = "wasm32")]
pub use runtime::{BrowserRuntime, DeviceFacts, install_julibrot_panic_hook, take_julibrot_panic};
pub use saved::{SavedCentre, SavedCoordinate, SavedView};
pub use state::{
    BOX_CLICK_THRESHOLD_PX, HotFrame, INITIAL_ITERATION_CAP, IntoGridExtent, NavigationEdit,
    PRESET_ROWS, PresetRow, RequestedControls, SCALE_RANGE_LOG2, ViewerController, anchor_px_up,
    box_zoom_delta_log2, css_from_anchor_px_up, drag_delta_px_down, is_box_selection, preset_row,
};
pub use surface::{PendingSurface, SurfaceAction, SurfaceState};
pub use timing::{LEVEL_TIMING_CAPACITY, LevelTimingLedger, LevelTimingRecord, TimingLevel};

/// Main integration object combining browser ownership and worker-published controls.
#[cfg(target_arch = "wasm32")]
pub struct App {
    runtime: BrowserRuntime,
    viewer: ViewerController,
    frame_loop: BrowserFrameLoop,
    requests: RunRequests,
}

/// Explicit app work requests; no flag claims submission or measurement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunRequests {
    /// A surface refresh was explicitly requested.
    pub frame: bool,
    /// An adaptive measurement suite was explicitly requested.
    pub measurement: bool,
    /// The current pose should restart scene refinement after its controls are drained.
    pub scene_update: bool,
}

#[cfg(target_arch = "wasm32")]
impl App {
    /// Performs version-independent browser startup before sibling runtime integration.
    ///
    /// # Errors
    ///
    /// Returns a typed device, surface, or canonical-viewer failure.
    pub async fn start(canvas_id: &str, status_id: &str) -> Result<Self, AppError> {
        Self::assemble(BrowserRuntime::start(canvas_id, status_id).await?)
    }

    /// Performs the same startup on a canvas element, with no status element in the document.
    ///
    /// # Errors
    ///
    /// Returns a typed device, surface, or canonical-viewer failure.
    pub async fn start_on_canvas(canvas: web_sys::HtmlCanvasElement) -> Result<Self, AppError> {
        Self::assemble(BrowserRuntime::start_on_canvas(canvas).await?)
    }

    fn assemble(runtime: BrowserRuntime) -> Result<Self, AppError> {
        let mut viewer = ViewerController::new([runtime.facts().width, runtime.facts().height])?;
        let frame_loop = BrowserFrameLoop::new(&runtime, &mut viewer)?;
        Ok(Self {
            runtime,
            viewer,
            frame_loop,
            requests: RunRequests::default(),
        })
    }

    /// Returns the initialized browser device and surface owner.
    #[must_use]
    pub const fn runtime(&self) -> &BrowserRuntime {
        &self.runtime
    }

    /// Returns the render-grid extent that pointer input must be expressed in.
    #[must_use]
    pub const fn grid_extent(&self) -> [u32; 2] {
        let facts = self.runtime.facts();
        [facts.width, facts.height]
    }

    /// Returns requested controls and worker owner integration.
    #[must_use]
    pub const fn viewer(&self) -> &ViewerController {
        &self.viewer
    }

    /// Returns mutable requested controls for serialized JavaScript callbacks.
    #[must_use]
    pub const fn viewer_mut(&mut self) -> &mut ViewerController {
        &mut self.viewer
    }

    /// Returns pending user requests without reporting them as submitted.
    #[must_use]
    pub const fn requests(&self) -> RunRequests {
        self.requests
    }

    /// Returns frame-loop facts without polling or submitting work.
    #[must_use]
    pub const fn frame_loop(&self) -> &BrowserFrameLoop {
        &self.frame_loop
    }

    /// Executes one bounded refresh turn and its immediate completion observation.
    ///
    /// # Errors
    ///
    /// Returns a typed sibling, surface, device, deadline, or poll-limit refusal.
    pub fn refresh(&mut self, now_ms: f64) -> Result<RefreshOutcome, AppError> {
        self.frame_loop.refresh(
            &mut self.runtime,
            &mut self.viewer,
            &mut self.requests,
            now_ms,
        )
    }

    /// Reports whether a yielded completion or refinement turn remains pending.
    ///
    /// A stopped loop answers false and stays false; every other answer carries the term for a
    /// presented image belonging to an older requested view, so a transient refusal that retired
    /// the only outstanding submission still leaves a turn scheduled.
    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        if self.frame_loop.stopped_reason().is_some() {
            return false;
        }
        // Nothing animates on its own. The retired term kept the loop turning forever whenever
        // the tumbled mode was selected, because the geometry read a clock; with every angle a
        // control, an untouched page reaches a fixed image and the loop is allowed to go quiet.
        self.requests.frame
            || self.requests.measurement
            || self.requests.scene_update
            // A copy that has been asked for and not yet taken keeps the loop turning, because the
            // copy is made on the turn that presents a frame and read on a later one.
            || self.frame_loop.frame_capture_turning()
            || self.frame_loop.pending(&self.runtime, &self.viewer)
    }

    /// Queues one explicit frame request for the next cooperative refresh turn.
    pub const fn request_frame(&mut self) {
        self.requests.frame = true;
    }

    /// Queues one explicit measurement request for the future measured submission path.
    pub const fn request_measurement(&mut self) {
        self.requests.measurement = true;
    }

    /// Selects automatic or button-driven scene refinement.
    pub fn set_scene_mode(&mut self, mode: SceneMode) {
        self.frame_loop.set_scene_mode(mode);
    }

    /// Restarts the scene ladder for the currently requested controls.
    pub const fn update_scene(&mut self) {
        self.requests.scene_update = true;
    }

    /// Arms one copy of the next presented frame; the copy itself is taken at present time.
    pub const fn request_frame_capture(&mut self) {
        self.frame_loop.arm_frame_capture();
    }

    /// Takes the completed frame copy, leaving nothing behind for a second caller.
    pub fn take_frame_capture(&mut self) -> Option<ember_julibrot_present::FrameReadback> {
        self.frame_loop.take_frame_capture()
    }

    /// Returns what a caller needs to know about frame copying before asking for one.
    #[must_use]
    pub fn frame_capture_facts(&self) -> FrameCaptureFacts<'_> {
        let device = self.runtime.facts();
        FrameCaptureFacts {
            supported: device.frame_copy_supported,
            status: device.frame_copy_status,
            route: device.frame_copy_route.as_str(),
            copy_route: self
                .frame_loop
                .frame_capture_ready_route()
                .map(ember_julibrot_present::FrameReadbackRoute::as_str),
            scene_id: self.frame_loop.frame_capture_scene_id(),
            pending: self.frame_loop.frame_capture_pending(),
            extent: self.frame_loop.frame_capture_extent(),
            refusal: self.frame_loop.frame_capture_refusal(),
        }
    }
}

/// What the page can honestly say about copying the presented frame.
///
/// A device that cannot copy its surface says so rather than answering an empty picture, and a
/// copy that was refused names its refusal rather than staying silently absent.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameCaptureFacts<'a> {
    /// Whether the surface exposes the copy usage a readback needs.
    pub supported: bool,
    /// Which route a copy takes on this device, in the words the page publishes.
    pub status: &'static str,
    /// The same route, named separately from the status sentence.
    pub route: &'static str,
    /// The route that produced the copy waiting to be taken, once one is waiting.
    pub copy_route: Option<&'static str>,
    /// The completed scene the waiting copy was drawn from.
    pub scene_id: Option<u64>,
    /// Whether a copy has been asked for and has not yet been taken.
    pub pending: bool,
    /// The extent of a copy waiting to be taken.
    pub extent: Option<[u32; 2]>,
    /// The typed reason the last copy did not happen.
    pub refusal: Option<&'a str>,
}

/// Version shared by the loader, wasm module, worker entry, and wire protocol.
pub const JULIBROT_ABI_VERSION: u32 = ember_julibrot_worker::JULIBROT_ABI_VERSION;

/// Full release version carried by the Julibrot application package.
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long an in-flight frame copy may stay unmapped before it is abandoned.
///
/// A map that never completes is not a slow copy, it is a copy that is never coming: it holds a
/// surface-sized buffer, refuses every later request, and keeps the loop turning for a caller that
/// will wait out its whole timeout learning nothing. Five seconds is far longer than any measured
/// map and short enough that the caller is told why rather than left counting.
pub const FRAME_CAPTURE_DEADLINE_MS: f64 = 5_000.0;

/// The four readings that together say whether the picture on the canvas is finished.
///
/// Four, and three of them is not finished. The first three say the ladder has nothing left to do;
/// the fourth says the image belongs to the view being asked for now rather than to an older one,
/// which is the condition a delivered refinement level cannot supply. A level is a property of the
/// last completed scene and survives a control move, so a caller reading only the level and the
/// pending flags reads the previous picture's finished state and calls it this picture's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one independent reading the loop already holds, and naming all four is the point: a caller that folded them would be back to guessing which condition failed"
)]
pub struct PictureState {
    /// A refinement turn is still due.
    pub refinement_pending: bool,
    /// A manual scene update has been asked for and not run.
    pub scene_update_pending: bool,
    /// A scene submission has not completed.
    pub scene_in_flight: bool,
    /// The presented image belongs to an older requested view.
    pub presented_view_stale: bool,
    /// The image on screen was warped from the completed scene rather than from an earlier one.
    ///
    /// A scene completes on one turn and reaches the canvas on a later present, so between those
    /// two moments every other reading says finished while the eye is still on the previous
    /// picture. A caller that copies the frame at that moment copies the wrong row.
    pub presented_scene_is_completed: bool,
    /// The presenter is holding an unmoved older picture instead of showing this one.
    pub warp_holds_stale: bool,
}

impl PictureState {
    /// Whether the picture on the canvas is finished and is the picture the controls ask for.
    ///
    /// The first three readings say the ladder has nothing left to do. The fourth says the pose on
    /// screen is the pose being asked for. The last two say the pixels on screen are the finished
    /// scene's: a scene completes one turn before its warp is presented, and a warp the presenter
    /// refused to move is an older picture standing in for this one. A caller who is about to copy
    /// the frame needs all six, because "the render finished" and "the finished render is on
    /// screen" are one present apart and the copy lands in between.
    #[must_use]
    pub const fn finished(self) -> bool {
        !self.refinement_pending
            && !self.scene_update_pending
            && !self.scene_in_flight
            && !self.presented_view_stale
            && self.presented_scene_is_completed
            && !self.warp_holds_stale
    }
}

/// What decides whether an armed frame copy is taken at a given moment.
///
/// These two answers are the whole of "on request only, never per frame": without them a
/// diagnostic that costs a surface-sized transfer becomes a cost every frame pays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one independent reading about one armed copy, and the two answers below are exactly which subsets of them mean yes"
)]
pub struct CaptureArming {
    /// A copy has been asked for and not yet handed to the renderer.
    pub armed: bool,
    /// This device takes the route being considered.
    pub route_matches: bool,
    /// A copy is already in flight and has not been taken.
    pub readback_in_flight: bool,
    /// An arming has already been handed to the renderer for a later submission.
    pub renderer_already_armed: bool,
}

impl CaptureArming {
    /// Whether this presentation is the one a direct-route copy is taken on.
    ///
    /// The surface image exists only between its acquisition and its presentation, so the direct
    /// route has exactly one moment; a device on the other route does not take one here, because
    /// its copy is drawn inside the submission instead and taking both would encode two copies of
    /// one frame; and a copy already in flight is not replaced by a second one.
    #[must_use]
    pub const fn surface_due(self) -> bool {
        self.armed && self.route_matches && !self.readback_in_flight
    }

    /// Whether an armed fallback copy may be handed to the next presentation submission.
    ///
    /// Handing it over twice would encode two copies of one frame, so an arming the renderer
    /// already holds blocks the next one until it has been taken.
    #[must_use]
    pub const fn offscreen_due(self) -> bool {
        self.surface_due() && !self.renderer_already_armed
    }
}

/// Refresh result returned without conflating submission and presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefreshOutcome {
    /// Shared owner epoch observed by this refresh.
    pub epoch: u64,
    /// Orbit generation observed by this refresh.
    pub generation: u32,
    /// Monotonic refresh identifier.
    pub refresh_id: u64,
    /// Warp submission retained with the surface image.
    pub warp_id: Option<u64>,
    /// Scene submission made during this refresh.
    pub scene_id: Option<u64>,
    /// True only after matching warp completion and post-timing present.
    pub presented: bool,
    /// Honest terminal state for this refresh.
    pub status: RefreshStatus,
    /// Precision policy under which this refresh was planned.
    pub precision_mode: &'static str,
}

/// App-owned refresh status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshStatus {
    /// No compatible scene has completed.
    Waiting,
    /// Scene or warp work was submitted.
    Submitted,
    /// The matching surface image was presented.
    Presented,
    /// Surface acquisition timed out and was skipped.
    SkippedTimeout,
    /// Newer work cancelled this refresh.
    Cancelled,
    /// A bounded fence refused; the submission was retired and will be retried.
    Refused,
    /// A typed failure was published.
    FailedTyped,
}

impl RefreshStatus {
    /// Returns the stable name the page overlay displays for this status.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting",
            Self::Submitted => "Submitted",
            Self::Presented => "Presented",
            Self::SkippedTimeout => "SkippedTimeout",
            Self::Cancelled => "Cancelled",
            Self::Refused => "Refused",
            Self::FailedTyped => "FailedTyped",
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    use crate::runtime::publish_start_error;
    use ember_julibrot_math::{ObjectAngles, PlaneAngles, PrecisionMode, ViewControls};
    use ember_julibrot_present::PaletteId;

    use crate::{
        App, JULIBROT_ABI_VERSION, PageFacts, SavedCentre, SavedView, SceneMode, anchor_px_up,
        box_zoom_delta_log2, css_from_anchor_px_up, drag_delta_px_down, is_box_selection,
        preset_row,
    };

    thread_local! {
        static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    }

    /// Returns the module ABI for loader and worker handshakes.
    #[wasm_bindgen]
    pub fn julibrot_abi_version() -> u32 {
        JULIBROT_ABI_VERSION
    }

    /// Returns the full release version carried by this wasm package.
    #[wasm_bindgen]
    pub fn julibrot_package_version() -> String {
        crate::PACKAGE_VERSION.to_string()
    }

    /// Starts the GL-only main-thread runtime and stores its single surface owner.
    #[wasm_bindgen]
    pub async fn start_julibrot(canvas_id: String, status_id: String) -> Result<(), JsValue> {
        let app = App::start(&canvas_id, &status_id)
            .await
            .map_err(|error| publish_start_error(&error))?;
        APP.with(|slot| {
            let mut slot = slot
                .try_borrow_mut()
                .map_err(|_| JsValue::from_str("Julibrot runtime startup is already publishing"))?;
            if slot.is_some() {
                return Err(JsValue::from_str("Julibrot runtime is already started"));
            }
            *slot = Some(app);
            Ok(())
        })
    }

    /// Starts the same runtime on a canvas element, for a page with no controls beside it.
    ///
    /// The control page keeps [`start_julibrot`] and its status paragraph. A driver has a canvas
    /// and nothing else, and inventing a status element for it would put a page contract in the
    /// way of a measurement rather than in the way of a bug.
    #[wasm_bindgen]
    pub async fn start_julibrot_on_canvas(
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), JsValue> {
        let app = App::start_on_canvas(canvas)
            .await
            .map_err(|error| publish_start_error(&error))?;
        APP.with(|slot| {
            let mut slot = slot
                .try_borrow_mut()
                .map_err(|_| JsValue::from_str("Julibrot runtime startup is already publishing"))?;
            if slot.is_some() {
                return Err(JsValue::from_str("Julibrot runtime is already started"));
            }
            *slot = Some(app);
            Ok(())
        })
    }

    /// Arms one copy of the next presented frame.
    ///
    /// The copy is on request only and never per frame: it is a full surface-sized transfer, and a
    /// loop that paid it every turn would spend its budget measuring its own readback.
    #[wasm_bindgen]
    pub fn app_request_frame_capture() -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.request_frame_capture();
            Ok(())
        })
    }

    /// Reports whether the frame can be copied, whether one is pending, and its extent.
    #[wasm_bindgen]
    pub fn app_frame_capture_json() -> Result<String, JsValue> {
        with_app(|app| {
            let facts = app.frame_capture_facts();
            let extent = facts.extent.unwrap_or([0, 0]);
            serde_json::to_string(&serde_json::json!({
                "frame_capture_supported": facts.supported,
                "frame_capture_status": facts.status,
                "frame_capture_route": facts.route,
                "frame_capture_copy_route": facts.copy_route,
                "frame_capture_scene_id": facts.scene_id,
                "frame_capture_pending": facts.pending,
                "frame_capture_ready": facts.extent.is_some(),
                "frame_capture_width": extent[0],
                "frame_capture_height": extent[1],
                "frame_capture_refusal": facts.refusal,
            }))
            .map_err(|error| JsValue::from_str(&error.to_string()))
        })
    }

    /// Returns the copied frame's packed RGBA bytes, top-down, or nothing while none is ready.
    ///
    /// Reading takes the copy: a second call answers nothing until another one is asked for, so
    /// two readers cannot both believe they hold the frame.
    #[wasm_bindgen]
    pub fn app_take_frame_rgba() -> Result<Option<Vec<u8>>, JsValue> {
        with_app_mut(|app| Ok(app.take_frame_capture().map(|frame| frame.rgba)))
    }

    /// Returns the full honest facts snapshot as JSON.
    #[wasm_bindgen]
    pub fn app_facts_json() -> Result<String, JsValue> {
        with_app(|app| {
            serde_json::to_string(&PageFacts::snapshot(app))
                .map_err(|error| JsValue::from_str(&error.to_string()))
        })
    }

    /// Stores the clicked canvas point as a point on the slice, without moving the picture.
    ///
    /// The click arrives as canvas-relative DOM CSS pixels beside the canvas client rectangle, so
    /// centring, the CSS-to-grid scale and the y flip stay on this one boundary. What the app then
    /// keeps is the point, not the pixel: a click changes nothing the eye can see except where the
    /// crosshair is drawn, and the picture is left exactly where it was.
    #[wasm_bindgen]
    pub fn app_set_target(
        pointer_css_x: f64,
        pointer_css_y_down: f64,
        rect_css_width: f64,
        rect_css_height: f64,
    ) -> Result<(), JsValue> {
        with_app_mut(|app| {
            let grid = app.grid_extent();
            let anchor = anchor_px_up(
                [pointer_css_x, pointer_css_y_down],
                [rect_css_width, rect_css_height],
                grid,
            )
            .map_err(app_js_error)?;
            app.viewer_mut().set_crosshair(anchor).map_err(app_js_error)
        })
    }

    /// Translates the picture by a drag displacement in canvas CSS pixels.
    ///
    /// The stored point is untouched, so the crosshair travels with the feature it was set on
    /// rather than with the screen.
    #[wasm_bindgen]
    pub fn app_pan_px(
        delta_css_x: f64,
        delta_css_y_down: f64,
        rect_css_width: f64,
        rect_css_height: f64,
    ) -> Result<(), JsValue> {
        with_app_mut(|app| {
            let grid = app.grid_extent();
            let delta = drag_delta_px_down(
                [delta_css_x, delta_css_y_down],
                [rect_css_width, rect_css_height],
                grid,
            )
            .map_err(app_js_error)?;
            app.viewer_mut()
                .pan_px(delta)
                .map(|_| ())
                .map_err(app_js_error)
        })
    }

    /// Returns where the stored point now falls, in the page's own canvas CSS pixels.
    ///
    /// The page draws the crosshair from this and nothing else, so the marker is a projection of
    /// the point rather than a memory of a pixel. `on_surface` is false when the point has left
    /// the canvas box; the point is kept either way.
    #[wasm_bindgen]
    pub fn app_crosshair_json(
        rect_css_width: f64,
        rect_css_height: f64,
    ) -> Result<String, JsValue> {
        with_app(|app| {
            let grid = app.grid_extent();
            let viewer = app.viewer();
            let Some(plane_px) = viewer.crosshair_plane_px() else {
                return Ok(r#"{"crosshair_present":false}"#.to_string());
            };
            let rect = [rect_css_width, rect_css_height];
            let css = css_from_anchor_px_up(plane_px, rect, grid).map_err(app_js_error)?;
            let on_surface =
                css[0] >= 0.0 && css[0] <= rect[0] && css[1] >= 0.0 && css[1] <= rect[1];
            let precision_bits = viewer.crosshair_precision_bits().unwrap_or_default();
            let mirror = viewer.crosshair_centre_f64().unwrap_or_default();
            Ok(format!(
                concat!(
                    r#"{{"crosshair_present":true,"crosshair_plane_px":[{},{}],"#,
                    r#""crosshair_css":[{},{}],"crosshair_on_surface":{},"#,
                    r#""crosshair_precision_bits":{},"crosshair_point_f64":[{},{},{},{}]}}"#
                ),
                plane_px[0],
                plane_px[1],
                css[0],
                css[1],
                on_surface,
                precision_bits,
                mirror[0],
                mirror[1],
                mirror[2],
                mirror[3],
            ))
        })
    }

    /// Zooms a dragged screen box to fill the screen, or treats a box under four pixels as a click.
    ///
    /// The page reports the rectangle it drew and nothing else; whether that rectangle was a box or
    /// a click, and what zoom change it earns, are decided here so the two gestures cannot drift
    /// apart in the loader.
    #[wasm_bindgen]
    pub fn app_zoom_box(
        start_css_x: f64,
        start_css_y_down: f64,
        end_css_x: f64,
        end_css_y_down: f64,
        rect_css_width: f64,
        rect_css_height: f64,
    ) -> Result<(), JsValue> {
        with_app_mut(|app| {
            let grid = app.grid_extent();
            let rect = [rect_css_width, rect_css_height];
            let extent = [
                (end_css_x - start_css_x).abs(),
                (end_css_y_down - start_css_y_down).abs(),
            ];
            let anchor = anchor_px_up(
                [
                    f64::midpoint(start_css_x, end_css_x),
                    f64::midpoint(start_css_y_down, end_css_y_down),
                ],
                rect,
                grid,
            )
            .map_err(app_js_error)?;
            app.viewer_mut()
                .set_crosshair(anchor)
                .map_err(app_js_error)?;
            if !is_box_selection(extent) {
                return Ok(());
            }
            let delta_log2 = box_zoom_delta_log2(extent, rect).map_err(app_js_error)?;
            app.viewer_mut()
                .zoom_about_crosshair(delta_log2)
                .map(|_| ())
                .map_err(app_js_error)
        })
    }

    /// Clears the stored slice point when a row replaces the current picture.
    #[wasm_bindgen]
    pub fn app_clear_crosshair() -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut().clear_crosshair();
            Ok(())
        })
    }

    /// Moves the absolute `scale` control, zooming about the stored crosshair point.
    #[wasm_bindgen]
    pub fn app_set_scale(zoom_log2: f64) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .set_zoom_log2(zoom_log2)
                .map(|_| ())
                .map_err(app_js_error)
        })
    }

    /// Stages independent plane angles in radians.
    #[wasm_bindgen]
    pub fn app_set_plane_angles(theta_1: f64, theta_2: f64) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .set_plane_angles(PlaneAngles { theta_1, theta_2 })
                .map_err(app_js_error)
        })
    }

    /// Stages all six object rotations in product order.
    #[wasm_bindgen]
    pub fn app_set_object_angles(
        rho_12: f64,
        rho_13: f64,
        rho_14: f64,
        rho_23: f64,
        rho_24: f64,
        rho_34: f64,
    ) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .set_object_angles(ObjectAngles {
                    rho_12,
                    rho_13,
                    rho_14,
                    rho_23,
                    rho_24,
                    rho_34,
                })
                .map_err(app_js_error)
        })
    }

    /// Moves the absolute plane origin and resets the centre to it.
    #[wasm_bindgen]
    pub fn app_set_plane_origin(z_re: f64, z_im: f64, c_re: f64, c_im: f64) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .set_plane_origin([z_re, z_im, c_re, c_im])
                .map_err(app_js_error)
        })
    }

    /// Stages the two retired VIEW aliases as camera factors q12 and q35.
    #[wasm_bindgen]
    pub fn app_set_view_angles(theta_1: f64, theta_2: f64) -> Result<(), JsValue> {
        with_view(|view| {
            view.camera[0] = theta_1;
            view.camera[8] = theta_2;
        })
    }

    /// Stages all ten ambient-camera rotations in product order.
    #[wasm_bindgen]
    #[allow(
        clippy::too_many_arguments,
        reason = "the browser contract exposes one scalar for each named camera plane"
    )]
    pub fn app_set_camera_angles(
        q_12: f64,
        q_13: f64,
        q_14: f64,
        q_23: f64,
        q_24: f64,
        q_34: f64,
        q_15: f64,
        q_25: f64,
        q_35: f64,
        q_45: f64,
    ) -> Result<(), JsValue> {
        with_view(|view| {
            view.camera = [q_12, q_13, q_14, q_23, q_24, q_34, q_15, q_25, q_35, q_45];
        })
    }

    /// Stages the five-dimensional camera translation in chart units.
    #[wasm_bindgen]
    pub fn app_set_camera_translation(
        t_1: f64,
        t_2: f64,
        t_3: f64,
        t_4: f64,
        t_5: f64,
    ) -> Result<(), JsValue> {
        with_view(|view| view.camera_translation = [t_1, t_2, t_3, t_4, t_5])
    }

    /// Stages the observer yaw and pitch in radians.
    #[wasm_bindgen]
    pub fn app_set_camera(yaw: f64, pitch: f64) -> Result<(), JsValue> {
        with_view(|view| {
            view.camera_yaw = yaw;
            view.camera_pitch = pitch;
        })
    }

    /// Stages the escape-height amplitude; zero is exactly the flat chart.
    #[wasm_bindgen]
    pub fn app_set_height(height_scale: f64) -> Result<(), JsValue> {
        with_view(|view| view.height_scale = height_scale)
    }

    /// Stages both perspective distances.
    #[wasm_bindgen]
    pub fn app_set_distances(distance_five: f64, distance_four: f64) -> Result<(), JsValue> {
        with_view(|view| {
            view.distance_five = distance_five;
            view.distance_four = distance_four;
        })
    }

    /// Returns one preset as the JSON row of control values the page writes into its elements.
    ///
    /// The page applies the row through the same handlers a user's own movement reaches, so this
    /// is a source of values and never a second path into the worker.
    #[wasm_bindgen]
    pub fn app_preset(id: u32) -> Result<String, JsValue> {
        let row = preset_row(id)
            .ok_or_else(|| JsValue::from_str("preset identifier is outside its range"))?;
        let saved = SavedView::from_preset(row).map_err(app_js_error)?;
        let mut value: serde_json::Value =
            serde_json::from_str(&saved.to_page_json().map_err(app_js_error)?)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        value
            .as_object_mut()
            .ok_or_else(|| JsValue::from_str("preset row is not a JSON object"))?
            .insert(
                "name".to_string(),
                serde_json::Value::String(row.name.to_string()),
            );
        serde_json::to_string(&value).map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Returns the row the viewer is showing, in the form a view box stores.
    #[wasm_bindgen]
    pub fn app_saved_view_json() -> Result<String, JsValue> {
        with_app(|app| {
            let saved = SavedView::capture(app.viewer()).map_err(app_js_error)?;
            saved.to_page_json().map_err(app_js_error)
        })
    }

    /// Applies every field of one saved or morphed row through one navigation transaction.
    #[wasm_bindgen]
    pub fn app_apply_saved_view(row_json: String) -> Result<(), JsValue> {
        let row = SavedView::from_page_json(&row_json).map_err(app_js_error)?;
        with_app_mut(|app| {
            app.viewer_mut()
                .apply_saved_view(&row)
                .map_err(app_js_error)
        })
    }

    /// Installs the authoritative centre of a stored row, which no control element carries.
    ///
    /// Every other field of a row is a control and reaches the worker through the handler a user's
    /// own movement reaches; the centre has no widget, so loading one is this single explicit call
    /// rather than a second path for the values that do have widgets.
    #[wasm_bindgen]
    pub fn app_set_centre(centre_json: String) -> Result<(), JsValue> {
        let centre: SavedCentre = serde_json::from_str(&centre_json)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let decoded = centre.decode().map_err(app_js_error)?;
        with_app_mut(|app| app.viewer_mut().set_centre(decoded).map_err(app_js_error))
    }

    /// Returns the row `t` of the way from one stored row to another.
    #[wasm_bindgen]
    pub fn app_morph_view(from_json: String, to_json: String, t: f64) -> Result<String, JsValue> {
        let from = SavedView::from_page_json(&from_json).map_err(app_js_error)?;
        let to = SavedView::from_page_json(&to_json).map_err(app_js_error)?;
        let morphed = SavedView::lerp(&from, &to, t).map_err(app_js_error)?;
        morphed.to_page_json().map_err(app_js_error)
    }

    fn with_view(edit: impl FnOnce(&mut ViewControls)) -> Result<(), JsValue> {
        with_app_mut(|app| {
            let mut view = app.viewer().requested().view;
            edit(&mut view);
            app.viewer_mut()
                .set_view_controls(view)
                .map_err(app_js_error)
        })
    }

    /// Stages the requested iteration cap.
    #[wasm_bindgen]
    pub fn app_set_iteration_cap(max_iter: u32) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .set_iteration_cap(max_iter)
                .map_err(app_js_error)
        })
    }

    /// Stages the requested precision policy as incompatible MAIN work.
    #[wasm_bindgen]
    pub fn app_set_precision_mode(mode: u32) -> Result<(), JsValue> {
        let precision_mode = PrecisionMode::from_u32(mode)
            .ok_or_else(|| JsValue::from_str("precision discriminant is outside 0..1"))?;
        with_app_mut(|app| {
            app.viewer_mut()
                .set_precision_mode(precision_mode)
                .map_err(app_js_error)
        })
    }

    /// Stages one of present's exact palette records.
    #[wasm_bindgen]
    pub fn app_set_palette(palette: u32) -> Result<(), JsValue> {
        let palette = match palette {
            0 => PaletteId::Classic,
            1 => PaletteId::Ember,
            2 => PaletteId::Ice,
            _ => return Err(JsValue::from_str("palette discriminant is outside 0..2")),
        };
        with_app_mut(|app| app.viewer_mut().set_palette(palette).map_err(app_js_error))
    }

    /// Queues one explicit surface-frame request without claiming it was submitted.
    #[wasm_bindgen]
    pub fn app_request_frame() -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.request_frame();
            Ok(())
        })
    }

    /// Queues one explicit measurement request without claiming results exist.
    #[wasm_bindgen]
    pub fn app_request_measurement() -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.request_measurement();
            Ok(())
        })
    }

    /// Selects automatic scene updates for one, or manual button-driven updates for zero.
    #[wasm_bindgen]
    pub fn app_set_scene_mode(mode: u32) -> Result<(), JsValue> {
        let mode = SceneMode::from_u32(mode)
            .ok_or_else(|| JsValue::from_str("scene mode discriminant is outside 0..1"))?;
        with_app_mut(|app| {
            app.set_scene_mode(mode);
            Ok(())
        })
    }

    /// Restarts scene refinement at the currently staged pose.
    #[wasm_bindgen]
    pub fn app_update_scene() -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.update_scene();
            Ok(())
        })
    }

    /// Runs one zero-timeout refresh turn at the supplied monotonic browser timestamp.
    #[wasm_bindgen]
    pub fn app_refresh(now_ms: f64) -> Result<String, JsValue> {
        with_app_mut(|app| {
            let outcome = app.refresh(now_ms).map_err(app_js_error)?;
            serde_json::to_string(&(
                outcome.refresh_id,
                outcome.warp_id,
                outcome.scene_id,
                outcome.presented,
            ))
            .map_err(|error| JsValue::from_str(&error.to_string()))
        })
    }

    /// Reports whether JavaScript should schedule another cooperative animation turn.
    #[wasm_bindgen]
    pub fn app_needs_refresh() -> Result<bool, JsValue> {
        with_app(|app| Ok(app.needs_refresh()))
    }

    fn with_app<T>(operation: impl FnOnce(&App) -> Result<T, JsValue>) -> Result<T, JsValue> {
        APP.with(|slot| {
            let slot = slot
                .try_borrow()
                .map_err(|_| JsValue::from_str("Julibrot app is already borrowed"))?;
            let app = slot
                .as_ref()
                .ok_or_else(|| JsValue::from_str("Julibrot app is not started"))?;
            operation(app)
        })
    }

    fn with_app_mut<T>(
        operation: impl FnOnce(&mut App) -> Result<T, JsValue>,
    ) -> Result<T, JsValue> {
        APP.with(|slot| {
            let mut slot = slot
                .try_borrow_mut()
                .map_err(|_| JsValue::from_str("Julibrot app is already borrowed"))?;
            let app = slot
                .as_mut()
                .ok_or_else(|| JsValue::from_str("Julibrot app is not started"))?;
            operation(app)
        })
    }

    fn app_js_error(error: crate::AppError) -> JsValue {
        JsValue::from_str(&error.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_entry::{
    app_apply_saved_view, app_clear_crosshair, app_crosshair_json, app_facts_json,
    app_frame_capture_json, app_morph_view, app_needs_refresh, app_pan_px, app_preset, app_refresh,
    app_request_frame, app_request_frame_capture, app_request_measurement, app_saved_view_json,
    app_set_camera, app_set_camera_angles, app_set_camera_translation, app_set_centre,
    app_set_distances, app_set_height, app_set_iteration_cap, app_set_object_angles,
    app_set_palette, app_set_plane_angles, app_set_plane_origin, app_set_precision_mode,
    app_set_scale, app_set_scene_mode, app_set_target, app_set_view_angles, app_take_frame_rgba,
    app_update_scene, app_zoom_box, julibrot_abi_version, julibrot_package_version, start_julibrot,
    start_julibrot_on_canvas,
};
