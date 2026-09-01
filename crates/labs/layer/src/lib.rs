//! Fragment-compute layer and GPU-resident 120-cell prism demonstration.

#![deny(missing_docs)]

pub mod compute;
pub mod geometry;
pub mod kernels;

#[cfg(target_arch = "wasm32")]
mod demo;

#[cfg(target_arch = "wasm32")]
use std::cell::{Cell, RefCell};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static DEMO: RefCell<Option<demo::Demo>> = const { RefCell::new(None) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

/// Runs deterministic object invariants during debug wasm initialization.
///
/// # Panics
///
/// Panics only when deterministic 120-cell construction violates a required invariant.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_init() {
    #[cfg(debug_assertions)]
    geometry::assert_invariants();
}

/// Initializes the WebGL2 layer, golden self-test, pillar kernels, and thin renderer.
///
/// # Errors
///
/// Returns a JavaScript error containing a typed refusal, capability failure, or pipeline diagnostic.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start_layer(canvas: web_sys::HtmlCanvasElement) -> Result<String, JsValue> {
    let generation = GENERATION.get().wrapping_add(1);
    GENERATION.set(generation);
    DEMO.with_borrow_mut(|slot| *slot = None);
    let (demo, facts) = demo::Demo::new(canvas)
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    if GENERATION.get() != generation {
        return Err(JsValue::from_str(
            "layer initialization completed after its generation was replaced",
        ));
    }
    DEMO.with_borrow_mut(|slot| *slot = Some(demo));
    Ok(facts)
}

/// Computes and presents one display-cadence frame.
///
/// # Errors
///
/// Returns a JavaScript error when initialization is absent, the device is lost, or presentation fails.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_layer_frame(time_seconds: f32) -> Result<u64, JsValue> {
    DEMO.with_borrow_mut(|slot| {
        slot.as_mut()
            .ok_or_else(|| JsValue::from_str("layer is not initialized"))?
            .frame(time_seconds)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    })
}
