// Board and camera maths read better as written than as mul_add chains.
#![allow(clippy::suboptimal_flops)]

//! Four Kings: the client, native and wasm.
//!
//! Hotseat (four seats, one keyboard) or online against `kings-server`.
//! The rules are `kings-core`'s and are used here for highlights and for
//! applying the server's echo, never for prediction; the page around the
//! canvas owns everything that needs text (design 4.4, 4.6 and 4.7).
//!
//! Layering, per the repo's one-way rule: this crate is game code. It talks
//! to `kings-core` for the rules and to `ember-engine` for the window and
//! the GPU, and neither of those knows this crate exists.

pub mod game;
pub mod hotseat;
pub mod meshes;
pub mod net;
pub mod online;
pub mod online_game;
pub mod ui;

use ember_engine::EngineConfig;

/// Start the hotseat game: four seats, one keyboard, the default
/// formations, 15 s turns, the camera turning to the seat to move.
pub fn run_local() {
    let (meshes, ids) = game::build_meshes();
    ember_engine::run(
        EngineConfig {
            title: "ember: four kings".to_string(),
            // The board is clicked on the page and steered with the
            // keyboard; grabbing the pointer would only take it away.
            capture_mouse: false,
            meshes,
        },
        hotseat::Hotseat::new(ids),
    );
}

/// Join or create a lobby on a server. `cfg` names the lobby; the page has
/// already let the player pick it from the browser's own listing.
///
/// # Errors
///
/// Returns an error if the networking backend cannot start the connection.
pub fn run_online(cfg: online_game::Config) -> Result<(), String> {
    let (meshes, ids) = game::build_meshes();
    let game = online_game::OnlineGame::connect(cfg, ids)?;
    ember_engine::run(
        EngineConfig {
            title: "ember: four kings (online)".to_string(),
            capture_mouse: false,
            meshes,
        },
        game,
    );
    Ok(())
}

// ---- wasm entry points ----------------------------------------------------
//
// All entry points are thread-local queue/snapshot exchanges, fire's
// hud_json pattern in both directions: wasm is single-threaded and the
// engine loop runs on rAF after `ember_engine::run` returns, so JS calls
// between frames never race the game. There is no pass_turn: the rules have
// no voluntary pass.

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use wasm_bindgen::prelude::*;

    use crate::game::{self, UiCmd};

    /// `ember_engine::run` installs the panic hook itself, so there is
    /// nothing to do here that would not be done twice.
    #[wasm_bindgen(start)]
    pub fn wasm_init() {}

    /// Hotseat: four seats, one keyboard, default formation, 15 s turns,
    /// camera turns to the seat to move.
    #[wasm_bindgen]
    pub fn start_local() {
        super::run_local();
    }

    /// `{"ws":"wss://...","handle":"...","lobby":"...","password":"","create":false}`,
    /// same shape and defaults as fire (handle -> "player", lobby -> "court").
    #[wasm_bindgen]
    pub fn start_online(config_json: &str) -> Result<(), JsValue> {
        let cfg = super::online_game::Config::from_json(config_json)
            .map_err(|e| JsValue::from_str(&e))?;
        super::run_online(cfg).map_err(|e| JsValue::from_str(&e))
    }

    /// The protocol this bundle speaks; the page shows it beside the
    /// server's so a mismatch names both numbers instead of just failing.
    #[wasm_bindgen]
    pub fn proto_version() -> u16 {
        kings_core::proto::PROTO_VERSION
    }

    /// Polled every animation frame: the JSON of `game::HudState`.
    #[wasm_bindgen]
    pub fn state_json() -> String {
        game::state_json()
    }

    /// Queues a click on the absolute tile `(x, y)` for the next update;
    /// the same path as Enter on the keyboard cursor.
    #[wasm_bindgen]
    pub fn click_tile(x: u8, y: u8) {
        game::push_cmd(UiCmd::Click(x, y));
    }

    /// The creator's Start button: queues `C2S::Start`.
    #[wasm_bindgen]
    pub fn start_game() {
        game::push_cmd(UiCmd::Start);
    }

    /// Esc, or a click beside the board.
    #[wasm_bindgen]
    pub fn clear_selection() {
        game::push_cmd(UiCmd::Clear);
    }
}
