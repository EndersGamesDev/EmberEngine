// Preserve established client simulation arithmetic and its rounding behavior.
#![allow(clippy::suboptimal_flops)]

//! Fire Racer V2 — GT circuit racing with tactical power-ups.
//!
//! Controls: W/S throttle and brake, A/D steer, Space handbrake to drift,
//! Shift boosts, E uses the held item, R recovers. Three laps of the castle circuit.
//!
//! Layering, per the repo's one-way rule: this crate is game code. It talks
//! to `fire-core` for the simulation and to `ember-engine` for the window and
//! the GPU, and neither of those knows this crate exists.

pub mod game;
pub mod meshes;
pub mod net;
pub mod online;
pub mod online_game;
pub mod texgen;
pub mod trackmesh;

use ember_engine::EngineConfig;

/// Start the local game: one human, seven AI cars, three laps.
pub fn run_local() {
    run_local_vehicle(0);
}

/// Start practice with a selected GT, lightweight or muscle car.
pub fn run_local_vehicle(vehicle: u8) {
    let track = fire_core::castle::track();
    tracing::info!(
        "fire: castle circuit — {:.0} m lap, {:.0} m wide, tightest corner {:.0} m",
        track.length(),
        track.half_width() * 2.0,
        track.min_curvature_radius(),
    );
    let (meshes, ids) = game::build_meshes(&track);
    let game = game::Game::new_with_vehicle(ids, vehicle.min(2));
    ember_engine::run(
        EngineConfig {
            title: "ember — fire racer".to_string(),
            // The car is steered with the keyboard; grabbing the pointer
            // would only take the cursor away for nothing.
            capture_mouse: false,
            // A player opened this; it takes the foreground like any app.
            activate: true,
            meshes,
        },
        game,
    );
}

/// Join a race on a server. `cfg` names the lobby; the page has already let
/// the player pick it from the browser's own listing.
///
/// # Errors
///
/// Returns an error if the networking backend cannot start the connection.
pub fn run_online(cfg: &online_game::Config) -> Result<(), String> {
    let track = fire_core::castle::track();
    let (meshes, ids) = game::build_meshes(&track);
    let game = online_game::OnlineGame::connect(cfg, ids)?;
    ember_engine::run(
        EngineConfig {
            title: "ember — fire racer (online)".to_string(),
            capture_mouse: false,
            // A player opened this; it takes the foreground like any app.
            activate: true,
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

    #[wasm_bindgen]
    pub fn start_local() {
        super::run_local();
    }

    #[wasm_bindgen]
    pub fn start_local_vehicle(vehicle: u8) {
        super::run_local_vehicle(vehicle);
    }

    /// Reuse the live renderer while resetting a local race and its timers.
    #[wasm_bindgen]
    pub fn restart_local(vehicle: u8) {
        super::game::request_restart(vehicle.min(2));
    }

    /// `{"ws":"wss://...","handle":"...","lobby":"...","password":null,"create":false}`
    #[wasm_bindgen]
    pub fn start_online(config_json: &str) -> Result<(), JsValue> {
        let cfg = super::online_game::Config::from_json(config_json)
            .map_err(|e| JsValue::from_str(&e))?;
        super::run_online(&cfg).map_err(|e| JsValue::from_str(&e))
    }

    /// The protocol this bundle speaks. The page shows it next to the
    /// server's so a mismatch names both numbers instead of just failing.
    #[wasm_bindgen]
    pub fn proto_version() -> u16 {
        fire_core::proto::PROTO_VERSION
    }

    /// Connection and lobby status, kept separate from numeric driving telemetry.
    #[wasm_bindgen]
    pub fn online_status() -> String {
        super::online_game::status()
    }

    /// The page draws the HUD: this renderer has one scene pass, no 2D layer
    /// and no text, so speed, lap, place and boost charges are handed to the
    /// surrounding HTML instead of drawn in the world.
    #[wasm_bindgen]
    pub fn hud_json() -> String {
        let h = super::game::hud();
        format!(
            "{{\"speed\":{:.0},\"lap\":{},\"laps\":{},\"place\":{},\"racers\":{},\
             \"boost\":{},\"boosting\":{},\"drifting\":{},\"countdown\":{:.2},\"finished\":{},\
             \"item\":{},\"vehicle\":{},\"gear\":{},\"race_time\":{:.3},\"drift_charge\":{:.3},\
             \"shield\":{:.2},\"grip\":{:.2},\"hit\":{:.2}}}",
            h.speed_kmh,
            h.lap,
            h.laps_total,
            h.place,
            h.racers,
            h.boost_charges,
            h.boosting,
            h.drifting,
            h.countdown,
            h.finished,
            h.item,
            h.vehicle,
            h.gear,
            h.race_time,
            h.drift_charge,
            h.shield,
            h.grip,
            h.hit
        )
    }
}
