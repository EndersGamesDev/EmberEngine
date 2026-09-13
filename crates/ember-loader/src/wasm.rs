//! The browser surface: the machine, as an object `web/loader.js` drives.
//!
//! Every call carries the page's own clock rather than reading one here. That
//! is what lets `web/loader.js` start the fetch and discovery in the same tick
//! as the page load and replay the drive calls, in order and with their real
//! timestamps, once this module has finished instantiating — so the loader's
//! own arrival never delays the work it reports on, and the numbers a player
//! reads are measured from the moment the page began rather than from the
//! moment the loader was ready to watch.

use js_sys::{Object, Reflect};
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::event::{Event, js_key};
use crate::phase::{Machine, Phase};
use crate::progress::whole_bytes;

fn put(object: &Object, key: &str, value: &JsValue) {
    // A freshly made object literal has no setter, no proxy and no frozen
    // prototype, so this cannot fail; there is no caller to report to in any
    // case, and a load must not end because a field could not be attached.
    let _set = Reflect::set(object, &JsValue::from_str(key), value);
}

fn put_number(object: &Object, key: &str, value: Option<f64>) {
    if let Some(number) = value {
        put(object, key, &JsValue::from_f64(number));
    }
}

fn put_text(object: &Object, key: &str, value: Option<&str>) {
    if let Some(text) = value {
        put(object, key, &JsValue::from_str(text));
    }
}

/// The event, as the plain object a page's handler receives.
fn to_js(ev: &Event) -> JsValue {
    let object = Object::new();
    put(
        &object,
        js_key::PHASE,
        &JsValue::from_str(ev.phase.as_str()),
    );
    put(
        &object,
        js_key::STATUS,
        &JsValue::from_str(ev.status.as_str()),
    );
    put(
        &object,
        js_key::ELAPSED_MS,
        &JsValue::from_f64(ev.elapsed_ms),
    );
    put(&object, js_key::PHASE_MS, &JsValue::from_f64(ev.phase_ms));
    put_number(
        &object,
        js_key::LOADED,
        ev.loaded.map(crate::progress::as_f64),
    );
    put_number(
        &object,
        js_key::TOTAL,
        ev.total.map(crate::progress::as_f64),
    );
    put_number(&object, js_key::PERCENT, ev.percent.map(f64::from));
    put_number(&object, js_key::RATE_BPS, ev.rate_bps);
    put_number(&object, js_key::ETA_MS, ev.eta_ms);
    put(&object, js_key::STALLED, &JsValue::from_bool(ev.stalled));
    put(
        &object,
        js_key::STALLED_MS,
        &JsValue::from_f64(ev.stalled_ms),
    );
    put_text(&object, js_key::REASON, ev.reason.as_deref());
    put_text(&object, js_key::DETAIL, ev.detail.as_deref());
    put(&object, js_key::TEXT, &JsValue::from_str(&ev.text));
    object.into()
}

/// A byte count from a page's number, ignoring anything that is not one.
fn count(value: f64) -> u64 {
    whole_bytes(value)
}

/// The load's rules, for one page.
#[wasm_bindgen]
pub struct Loader {
    machine: Machine,
}

#[wasm_bindgen]
impl Loader {
    /// A loader whose clock starts at `now_ms`, calling a download stalled
    /// after `stall_ms` of silence.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(now_ms: f64, stall_ms: f64) -> Self {
        Self {
            machine: Machine::new(now_ms, stall_ms),
        }
    }

    /// Whether the load has finished or failed.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn ended(&self) -> bool {
        self.machine.ended()
    }

    /// Open the load.
    pub fn begin(&mut self, now_ms: f64) -> JsValue {
        to_js(&self.machine.begin(now_ms))
    }

    /// Begin a named phase.
    pub fn enter(&mut self, now_ms: f64, phase: &str) -> JsValue {
        match Phase::parse(phase) {
            Some(phase) => to_js(&self.machine.enter(now_ms, phase)),
            None => to_js(&self.machine.unknown(now_ms, phase)),
        }
    }

    /// Reopen host discovery for a later ranking.
    pub fn rerank(&mut self, now_ms: f64) -> JsValue {
        to_js(&self.machine.rerank(now_ms))
    }

    /// Finish a named phase. Finishing `ready` ends the load.
    pub fn done(&mut self, now_ms: f64, phase: &str) -> JsValue {
        match Phase::parse(phase) {
            Some(phase) => to_js(&self.machine.done(now_ms, phase)),
            None => to_js(&self.machine.unknown(now_ms, phase)),
        }
    }

    /// Record the declared length of the download. A value that is not a
    /// positive number means the response declared none.
    pub fn total(&mut self, now_ms: f64, total: f64) -> JsValue {
        let declared = if total.is_finite() && total > 0.0 {
            Some(count(total))
        } else {
            None
        };
        to_js(&self.machine.total(now_ms, declared))
    }

    /// Record one arrived chunk.
    pub fn bytes(&mut self, now_ms: f64, bytes: f64) -> JsValue {
        to_js(&self.machine.bytes(now_ms, count(bytes)))
    }

    /// Let the clock run. Returns `null` unless the stall state flipped.
    pub fn tick(&mut self, now_ms: f64) -> JsValue {
        self.machine
            .tick(now_ms)
            .as_ref()
            .map_or(JsValue::NULL, to_js)
    }

    /// Fail a named phase.
    pub fn fail(&mut self, now_ms: f64, phase: &str, reason: &str, detail: &str) -> JsValue {
        match Phase::parse(phase) {
            Some(phase) => to_js(&self.machine.fail(now_ms, phase, reason, detail)),
            None => to_js(&self.machine.unknown(now_ms, phase)),
        }
    }
}
