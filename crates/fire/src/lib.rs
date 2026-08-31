//! Fire Racer — arcade racing through a gothic castle bailey.
//!
//! Controls: W/S throttle and brake, A/D steer, Space handbrake to drift,
//! Shift to spend a boost charge. Three laps of the castle yard.
//!
//! Layering, per the repo's one-way rule: this crate is game code. It talks
//! to `fire-core` for the simulation and to `ember-engine` for the window and
//! the GPU, and neither of those knows this crate exists.

pub mod game;
pub mod meshes;
pub mod texgen;
pub mod trackmesh;

use ember_engine::EngineConfig;

/// Start the local game: one human, seven AI cars, three laps.
pub fn run_local() {
    let track = fire_core::castle::track();
    tracing::info!(
        "fire: castle circuit — {:.0} m lap, {:.0} m wide, tightest corner {:.0} m",
        track.length(),
        track.half_width() * 2.0,
        track.min_curvature_radius(),
    );
    let (meshes, ids) = game::build_meshes(&track);
    let game = game::Game::new(ids);
    ember_engine::run(
        EngineConfig {
            title: "ember — fire racer".to_string(),
            // The car is steered with the keyboard; grabbing the pointer
            // would only take the cursor away for nothing.
            capture_mouse: false,
            meshes,
        },
        game,
    );
}

// ---- wasm entry points ----------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use wasm_bindgen::prelude::*;

    /// `ember_engine::run` installs the panic hook itself, so there is
    /// nothing to do here that would not be done twice.
    #[wasm_bindgen(start)]
    pub fn wasm_init() {}

    #[wasm_bindgen]
    pub fn start_local() {
        super::run_local();
    }

    /// The page draws the HUD: this renderer has one scene pass, no 2D layer
    /// and no text, so speed, lap, place and boost charges are handed to the
    /// surrounding HTML instead of drawn in the world.
    #[wasm_bindgen]
    pub fn hud_json() -> String {
        let h = super::game::hud();
        format!(
            "{{\"speed\":{:.0},\"lap\":{},\"laps\":{},\"place\":{},\"racers\":{},\
             \"boost\":{},\"boosting\":{},\"drifting\":{},\"countdown\":{:.2},\"finished\":{}}}",
            h.speed_kmh,
            h.lap,
            h.laps_total,
            h.place,
            h.racers,
            h.boost_charges,
            h.boosting,
            h.drifting,
            h.countdown,
            h.finished
        )
    }
}
