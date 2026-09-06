//! Ultimate League — the client, native and wasm.
//!
//! Top-down MOBA on one lane. The page around the canvas owns everything
//! with text (draft, HUD, shop, minimap) and talks to this crate through
//! `state_json` / `cmd_json`; the canvas shows the world the
//! `league-server` authoritative sim produced, or the local practice
//! match's. Neither this crate nor the page decides combat outcomes —
//! `league-core` does, on the server, once.
//!
//! Layering, per the repo's one-way rule: this crate is game code. It
//! talks to `league-core` for the simulation and to `ember-engine` for
//! the window and the GPU, and neither of those knows this crate exists.

pub mod bindings;
pub mod game;
pub mod online_game;
pub mod world;

mod net;
mod scene;

/// The page polls `state_json` every animation frame; both game modes
/// publish the latest world JSON here. A string, because the page parses
/// it whole and nothing in Rust reads it back.
pub mod hud {
    use std::sync::Mutex;

    static LATEST: Mutex<String> = Mutex::new(String::new());

    pub fn set(json: &str) {
        if let Ok(mut s) = LATEST.lock() {
            *s = json.to_string();
        }
    }

    #[must_use]
    pub fn get() -> String {
        let s = LATEST.lock().map(|s| s.clone()).unwrap_or_default();
        if s.is_empty() {
            r#"{"phase":"select","left":0,"mode":1,"me":0}"#.to_string()
        } else {
            s
        }
    }
}

use ember_engine::EngineConfig;

/// Start the practice match against bots: no server, same sim.
pub fn run_local(mode: u8) {
    let meshes = scene::build_meshes();
    let game = game::LocalGame::new(mode, "you", 0x1ea9_e67b);
    ember_engine::run(
        EngineConfig {
            title: "ember — ultimate league (practice)".to_string(),
            capture_mouse: false,
            activate: cfg!(target_arch = "wasm32"),
            meshes,
        },
        game,
    );
}

/// Join or create a match on a server.
///
/// # Errors
///
/// Returns an error if the networking backend cannot start the connection.
pub fn run_online(cfg: &online_game::Config) -> Result<(), String> {
    let meshes = scene::build_meshes();
    let game = online_game::OnlineGame::connect(cfg.clone())?;
    ember_engine::run(
        EngineConfig {
            title: "ember — ultimate league (online)".to_string(),
            capture_mouse: false,
            activate: cfg!(target_arch = "wasm32"),
            meshes,
        },
        game,
    );
    Ok(())
}

// ---- wasm entry points ----------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use wasm_bindgen::prelude::*;

    /// `ember_engine::run` installs the panic hook itself, so there is
    /// nothing to do here that would not be done twice.
    #[wasm_bindgen(start)]
    pub fn wasm_init() {}

    /// Practice against bots. `mode`: 1 for the duel, 3 for the squad.
    #[wasm_bindgen]
    pub fn start_local(mode: u8) {
        super::run_local(if mode == 1 { 1 } else { 3 });
    }

    /// `{"ws":"wss://...","handle":"...","lobby":"...","password":"","create":false,"mode":3}`
    #[wasm_bindgen]
    pub fn start_online(config_json: &str) -> Result<(), JsValue> {
        let cfg = super::online_game::Config::from_json(config_json)
            .map_err(|e| JsValue::from_str(&e))?;
        super::run_online(&cfg).map_err(|e| JsValue::from_str(&e))
    }

    /// The protocol this bundle speaks; the page shows it beside the
    /// server's so a mismatch names both numbers instead of just failing.
    #[wasm_bindgen]
    pub fn proto_version() -> u16 {
        league_core::proto::PROTO_VERSION
    }

    /// The world, as the page draws it: draft state, HUD, feed, minimap.
    #[wasm_bindgen]
    pub fn state_json() -> String {
        super::hud::get()
    }

    /// The static tables (champions, items, runes, spells) for the draft
    /// screen and the shop, from the same numbers the sim runs on.
    #[wasm_bindgen]
    pub fn data_json() -> String {
        super::world::data_json()
    }

    /// Apply a flat physical-key map over the defaults. Validation is atomic.
    #[wasm_bindgen]
    pub fn set_bindings_json(json: &str) -> Result<(), JsValue> {
        super::bindings::set_json(json).map_err(|error| JsValue::from_str(&error))
    }

    /// Complete physical bindings, including the shop key handled by the page.
    #[wasm_bindgen]
    pub fn bindings_json() -> String {
        super::bindings::json()
    }

    /// Supported physical keys and their short labels for the settings page.
    #[wasm_bindgen]
    pub fn binding_options_json() -> String {
        super::bindings::options_json()
    }

    /// Suppress gameplay input while menus or text fields have focus.
    #[wasm_bindgen]
    pub fn set_input_enabled(enabled: bool) {
        super::bindings::set_input_enabled(enabled);
    }

    /// Queue one page command for the next frame: `{"pick":{...}}`,
    /// `{"start":true}`, `{"buy":id}`, `{"use":slot}`, `{"rank":i}`,
    /// `{"shop":true|false}`.
    #[wasm_bindgen]
    pub fn cmd_json(json: &str) {
        super::game::uiq::push(json.to_string());
    }
}
