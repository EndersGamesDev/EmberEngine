//! Integration runtime and page contract for the Julibrot laboratory.

mod error;
mod surface;

#[cfg(target_arch = "wasm32")]
mod runtime;

pub use error::AppError;
#[cfg(target_arch = "wasm32")]
pub use runtime::{BrowserRuntime, DeviceFacts, install_julibrot_panic_hook, take_julibrot_panic};
pub use surface::{PendingSurface, SurfaceAction, SurfaceState};

/// Main browser integration object named by the implementation contract.
#[cfg(target_arch = "wasm32")]
pub type App = BrowserRuntime;

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
    use crate::{BrowserRuntime, JULIBROT_ABI_VERSION};

    thread_local! {
        static RUNTIME: RefCell<Option<BrowserRuntime>> = const { RefCell::new(None) };
    }

    /// Returns the module ABI for loader and worker handshakes.
    #[wasm_bindgen]
    pub fn julibrot_abi_version() -> u32 {
        JULIBROT_ABI_VERSION
    }

    /// Starts the GL-only main-thread runtime and stores its single surface owner.
    #[wasm_bindgen]
    pub async fn start_julibrot(canvas_id: String, status_id: String) -> Result<(), JsValue> {
        let runtime = BrowserRuntime::start(&canvas_id, &status_id)
            .await
            .map_err(|error| publish_start_error(&error))?;
        RUNTIME.with(|slot| {
            let mut slot = slot
                .try_borrow_mut()
                .map_err(|_| JsValue::from_str("Julibrot runtime startup is already publishing"))?;
            if slot.is_some() {
                return Err(JsValue::from_str("Julibrot runtime is already started"));
            }
            *slot = Some(runtime);
            Ok(())
        })
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_entry::{julibrot_abi_version, start_julibrot};
