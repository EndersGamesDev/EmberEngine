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
pub use frame::RefinementSchedule;
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
    BOX_CLICK_THRESHOLD_PX, HotFrame, INITIAL_ITERATION_CAP, NavigationEdit, PRESET_ROWS,
    PresetRow, RequestedControls, SCALE_RANGE_LOG2, ViewerController, anchor_px_up,
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

/// Explicit app work requests; neither flag claims submission or measurement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunRequests {
    /// A surface refresh was explicitly requested.
    pub frame: bool,
    /// An adaptive measurement suite was explicitly requested.
    pub measurement: bool,
}

#[cfg(target_arch = "wasm32")]
impl App {
    /// Performs version-independent browser startup before sibling runtime integration.
    ///
    /// # Errors
    ///
    /// Returns a typed device, surface, or canonical-viewer failure.
    pub async fn start(canvas_id: &str, status_id: &str) -> Result<Self, AppError> {
        let runtime = BrowserRuntime::start(canvas_id, status_id).await?;
        let mut viewer = ViewerController::new(runtime.facts().width)?;
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
}

/// Version shared by the loader, wasm module, worker entry, and wire protocol.
pub const JULIBROT_ABI_VERSION: u32 = ember_julibrot_worker::JULIBROT_ABI_VERSION;

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
    use ember_julibrot_math::{PlaneAngles, PrecisionMode, ViewControls};
    use ember_julibrot_present::PaletteId;

    use crate::{
        App, JULIBROT_ABI_VERSION, PageFacts, SavedCentre, SavedView, anchor_px_up,
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
                .set_target(anchor, delta_log2)
                .map(|_| ())
                .map_err(app_js_error)
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

    /// Moves the absolute plane origin and resets the centre to it.
    #[wasm_bindgen]
    pub fn app_set_plane_origin(z_re: f64, z_im: f64, c_re: f64, c_im: f64) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .set_plane_origin([z_re, z_im, c_re, c_im])
                .map_err(app_js_error)
        })
    }

    /// Stages both VIEW angles in radians.
    #[wasm_bindgen]
    pub fn app_set_view_angles(theta_1: f64, theta_2: f64) -> Result<(), JsValue> {
        with_view(|view| {
            view.theta_1 = theta_1;
            view.theta_2 = theta_2;
        })
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
        Ok(format!(
            concat!(
                r#"{{"name":"{}","theta_1":{},"theta_2":{},"origin":[{},{},{},{}],"#,
                r#""view_theta_1":{},"view_theta_2":{},"camera_yaw":{},"camera_pitch":{},"#,
                r#""height_scale":{},"distance_five":{},"distance_four":{}}}"#
            ),
            row.name,
            row.plane_angles[0],
            row.plane_angles[1],
            row.plane_origin[0],
            row.plane_origin[1],
            row.plane_origin[2],
            row.plane_origin[3],
            row.view.theta_1,
            row.view.theta_2,
            row.view.camera_yaw,
            row.view.camera_pitch,
            row.view.height_scale,
            row.view.distance_five,
            row.view.distance_four,
        ))
    }

    /// Returns the row the viewer is showing, in the form a view box stores.
    #[wasm_bindgen]
    pub fn app_saved_view_json() -> Result<String, JsValue> {
        with_app(|app| {
            let saved = SavedView::capture(app.viewer()).map_err(app_js_error)?;
            serde_json::to_string(&saved).map_err(|error| JsValue::from_str(&error.to_string()))
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
        let from: SavedView = serde_json::from_str(&from_json)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let to: SavedView = serde_json::from_str(&to_json)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let morphed = SavedView::lerp(&from, &to, t).map_err(app_js_error)?;
        serde_json::to_string(&morphed).map_err(|error| JsValue::from_str(&error.to_string()))
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
    app_crosshair_json, app_facts_json, app_morph_view, app_needs_refresh, app_pan_px, app_preset,
    app_refresh, app_request_frame, app_request_measurement, app_saved_view_json, app_set_camera,
    app_set_centre, app_set_distances, app_set_height, app_set_iteration_cap, app_set_palette,
    app_set_plane_angles, app_set_plane_origin, app_set_precision_mode, app_set_scale,
    app_set_target, app_set_view_angles, app_zoom_box, julibrot_abi_version, start_julibrot,
};
