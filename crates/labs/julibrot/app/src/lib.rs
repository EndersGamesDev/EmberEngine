//! Integration runtime and page contract for the Julibrot laboratory.

mod error;
#[cfg(target_arch = "wasm32")]
mod facts;
mod measurement;
mod state;
mod surface;

#[cfg(target_arch = "wasm32")]
mod runtime;

pub use error::AppError;
#[cfg(target_arch = "wasm32")]
pub use facts::PageFacts;
pub use measurement::{
    ADAPTIVE_SAMPLES, ADAPTIVE_WARM_UPS, AdaptivePlan, CONTINUOUS_FRAME_THRESHOLD_MS,
    FrameObservation, FramePolicy, FramePolicyTracker, MAX_ADAPTIVE_REPEATS, MAX_BATCH_MS,
    MeasurementError, SUITE_DEADLINE_MS, SampleSummary, TARGET_TIMER_QUANTA,
    TIMER_PROBE_DEADLINE_MS, TIMER_READ_LIMIT, TIMER_TRANSITION_TARGET, TimerProbeFacts,
    probe_timer,
};
#[cfg(target_arch = "wasm32")]
pub use runtime::{BrowserRuntime, DeviceFacts, install_julibrot_panic_hook, take_julibrot_panic};
pub use state::{
    HotFrame, INITIAL_ITERATION_CAP, NavigationEdit, RequestedControls, ViewerController,
};
pub use surface::{PendingSurface, SurfaceAction, SurfaceState};

/// Main integration object combining browser ownership and worker-published controls.
#[cfg(target_arch = "wasm32")]
pub struct App {
    runtime: BrowserRuntime,
    viewer: ViewerController,
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
        let viewer = ViewerController::new()?;
        Ok(Self {
            runtime,
            viewer,
            requests: RunRequests::default(),
        })
    }

    /// Returns the initialized browser device and surface owner.
    #[must_use]
    pub const fn runtime(&self) -> &BrowserRuntime {
        &self.runtime
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

    /// Queues one explicit frame request for the future present integration turn.
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
    /// A typed failure was published.
    FailedTyped,
}

#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    use crate::runtime::publish_start_error;
    use ember_julibrot_math::{PlaneAngles, PlanePreset, ViewMode};
    use ember_julibrot_present::PaletteId;

    use crate::{App, JULIBROT_ABI_VERSION, PageFacts};

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

    /// Stages pointer-anchored zoom and preserves the requested control on delayed work.
    #[wasm_bindgen]
    pub fn app_wheel_zoom(delta_log2: f64, anchor_x: f64, anchor_y_up: f64) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .wheel_zoom(delta_log2, [anchor_x, anchor_y_up])
                .map(|_| ())
                .map_err(app_js_error)
        })
    }

    /// Stages drag pan after converting DOM-down y inside the Rust control boundary.
    #[wasm_bindgen]
    pub fn app_drag_pan(delta_x: f64, delta_y_down: f64) -> Result<(), JsValue> {
        with_app_mut(|app| {
            app.viewer_mut()
                .drag_pan([delta_x, delta_y_down])
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

    /// Selects Mandelbrot or Julia and resets its centre to the defining origin.
    #[wasm_bindgen]
    pub fn app_set_preset(kind: u32, c_re: f64, c_im: f64) -> Result<(), JsValue> {
        let preset = match kind {
            0 => PlanePreset::Mandelbrot,
            1 => PlanePreset::Julia { c0: [c_re, c_im] },
            _ => return Err(JsValue::from_str("preset discriminant is outside 0..1")),
        };
        with_app_mut(|app| app.viewer_mut().set_preset(preset).map_err(app_js_error))
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

    /// Stages one of present's exact palette records.
    #[wasm_bindgen]
    pub fn app_set_palette(palette: u32) -> Result<(), JsValue> {
        let palette = match palette {
            0 => PaletteId::Classic,
            1 => PaletteId::Ember,
            2 => PaletteId::Ice,
            _ => return Err(JsValue::from_str("palette discriminant is outside 0..2")),
        };
        with_app_mut(|app| {
            app.viewer_mut().set_palette(palette);
            Ok(())
        })
    }

    /// Stages the math-owned flat or tumbled view discriminant.
    #[wasm_bindgen]
    pub fn app_set_view(view: u32) -> Result<(), JsValue> {
        let view = match view {
            0 => ViewMode::Flat,
            1 => ViewMode::Tumbled,
            _ => return Err(JsValue::from_str("view discriminant is outside 0..1")),
        };
        with_app_mut(|app| {
            app.viewer_mut().set_view(view);
            Ok(())
        })
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
    app_drag_pan, app_facts_json, app_request_frame, app_request_measurement,
    app_set_iteration_cap, app_set_palette, app_set_plane_angles, app_set_preset, app_set_view,
    app_wheel_zoom, julibrot_abi_version, start_julibrot,
};
