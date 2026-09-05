// Preserve established client interpolation and simulation expression ordering.
#![allow(clippy::suboptimal_flops)]
// Angle normalization intentionally advances through floating-point turn boundaries.
#![allow(clippy::while_float)]

//! Arena on the ember engine — the v0 pong classic locally and the shooter online
//! (matchmaking lobbies via arena-server over WebSocket).
//!
//! Local controls: P1 (blue, near) A/D · P2 (red, far) ←/→ · first to 7.
//! Online: either key set steers YOUR paddle; the server is authoritative.

mod ads;
mod feel;
#[cfg(test)]
mod grip_tests;
mod grips;
mod online;
mod props;
mod rounds;
mod script;
mod sound;
mod viewarms;
mod weather;

use ember_engine::glam::Vec3;
use ember_engine::{Camera, EmberGame, EngineConfig, Frame, InputState, Instance, KeyCode};

use arena_core::sim::{
    BALL_R, COURT_END_Z, COURT_HALF_W, FIXED_DT, PADDLE_HALF_W, PADDLE_Z, Phase, Sim,
};

pub use online::OnlineConfig;

const P1_COLOR: Vec3 = Vec3::new(0.25, 0.55, 0.95);
const P2_COLOR: Vec3 = Vec3::new(0.92, 0.32, 0.28);
const BALL_COLOR: Vec3 = Vec3::new(0.95, 0.93, 0.80);

/// Everything the scene builder needs, mode-agnostic.
struct SceneParams {
    p1_x: f32,
    p2_x: f32,
    ball: [f32; 2],
    ball_y: f32,
    scores: [u32; 2],
    /// Flip the camera to the far side (online role 1 sees themselves near).
    flip: bool,
}

fn build_scene(p: &SceneParams) -> Frame {
    let camera = if p.flip {
        Camera {
            eye: Vec3::new(0.0, 24.0, -30.0),
            target: Vec3::new(0.0, 0.0, 1.0),
            fov_y_deg: 50.0,
        }
    } else {
        Camera {
            eye: Vec3::new(0.0, 24.0, 30.0),
            target: Vec3::new(0.0, 0.0, -1.0),
            fov_y_deg: 50.0,
        }
    };
    let mut frame = Frame {
        camera,
        instances: Vec::with_capacity(32),
        fog: ember_engine::Fog::default(),
        ..Frame::default()
    };
    let inst = |frame: &mut Frame, pos: Vec3, scale: Vec3, color: Vec3| {
        frame.instances.push(Instance::new(pos, scale, color));
    };

    // Court: floor, side walls, dashed center line.
    let floor_w = COURT_HALF_W * 2.0 + 3.0;
    let floor_d = COURT_END_Z * 2.0 + 2.0;
    inst(
        &mut frame,
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(floor_w, 1.0, floor_d),
        Vec3::new(0.13, 0.14, 0.18),
    );
    for side in [-1.0f32, 1.0] {
        inst(
            &mut frame,
            Vec3::new((COURT_HALF_W + 0.75) * side, 0.45, 0.0),
            Vec3::new(0.6, 0.9, floor_d),
            Vec3::new(0.32, 0.34, 0.40),
        );
    }
    let dashes: u16 = 9;
    for i in 0..dashes {
        let x = -COURT_HALF_W + (f32::from(i) + 0.5) * (COURT_HALF_W * 2.0 / f32::from(dashes));
        inst(
            &mut frame,
            Vec3::new(x, 0.02, 0.0),
            Vec3::new(1.0, 0.06, 0.18),
            Vec3::new(0.30, 0.32, 0.38),
        );
    }

    // Paddles.
    inst(
        &mut frame,
        Vec3::new(p.p1_x, 0.5, PADDLE_Z),
        Vec3::new(PADDLE_HALF_W * 2.0, 1.0, 0.8),
        P1_COLOR,
    );
    inst(
        &mut frame,
        Vec3::new(p.p2_x, 0.5, -PADDLE_Z),
        Vec3::new(PADDLE_HALF_W * 2.0, 1.0, 0.8),
        P2_COLOR,
    );

    // Ball.
    inst(
        &mut frame,
        Vec3::new(p.ball[0], p.ball_y, p.ball[1]),
        Vec3::splat(BALL_R * 2.0),
        BALL_COLOR,
    );

    // Score pips on top of each wall: your points march toward your side.
    for (idx, (color, sign)) in [(P1_COLOR, 1.0f32), (P2_COLOR, -1.0f32)].iter().enumerate() {
        for i in 0..p.scores[idx] {
            let pip_index = u8::try_from(i).expect("scores are capped below u8::MAX");
            inst(
                &mut frame,
                Vec3::new(
                    (COURT_HALF_W + 0.75) * -sign, // P1 pips on the left wall
                    1.25,
                    (11.0 - f32::from(pip_index) * 1.7) * sign,
                ),
                Vec3::splat(0.55),
                *color,
            );
        }
    }

    frame
}

/// Moving state as of the step before the latest one, for render-side
/// interpolation between fixed sim steps.
#[derive(Clone, Copy)]
struct PrevState {
    p1_x: f32,
    p2_x: f32,
    ball: [f32; 2],
}

/// Local mode: both players at one keyboard, sim runs in-process.
struct LocalGame {
    sim: Sim,
    /// Render-time accumulator driving fixed sim steps.
    accumulator: f32,
    prev: PrevState,
}

impl LocalGame {
    const fn new() -> Self {
        Self {
            sim: Sim::new(),
            accumulator: 0.0,
            prev: PrevState {
                p1_x: 0.0,
                p2_x: 0.0,
                ball: [0.0, 0.0],
            },
        }
    }
}

impl EmberGame for LocalGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let p1 = input.axis(KeyCode::KeyA, KeyCode::KeyD);
        let p2 = input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight);

        // Fixed 60 Hz sim regardless of display rate.
        self.accumulator = (self.accumulator + dt).min(0.25);
        while self.accumulator >= FIXED_DT {
            self.accumulator -= FIXED_DT;
            self.prev = PrevState {
                p1_x: self.sim.p1_x,
                p2_x: self.sim.p2_x,
                ball: self.sim.ball_pos,
            };
            self.sim.step(p1, p2);
            if let Some((scorer, won)) = self.sim.event {
                // The ball teleports to center on a point; don't smear the
                // interpolation across that jump.
                self.prev.ball = self.sim.ball_pos;
                if won {
                    tracing::info!("player {} WINS the game!", scorer + 1);
                } else {
                    tracing::info!(
                        "player {} scores ({} : {})",
                        scorer + 1,
                        self.sim.score[0],
                        self.sim.score[1]
                    );
                }
            }
        }

        // Render interpolation between fixed steps.
        let alpha = (self.accumulator / FIXED_DT).clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * alpha;
        let ball_y = match self.sim.phase {
            Phase::Serving { timer, .. } => 0.5 + (timer * 6.0).sin().abs() * 0.4,
            Phase::Playing => 0.5,
        };
        build_scene(&SceneParams {
            p1_x: lerp(self.prev.p1_x, self.sim.p1_x),
            p2_x: lerp(self.prev.p2_x, self.sim.p2_x),
            ball: [
                lerp(self.prev.ball[0], self.sim.ball_pos[0]),
                lerp(self.prev.ball[1], self.sim.ball_pos[1]),
            ],
            ball_y,
            scores: self.sim.score,
            flip: false,
        })
    }
}

pub fn run_local() {
    ember_engine::run(
        EngineConfig {
            title: "ember arena — v0, the pong classic — P1: A/D, P2: ←/→".to_string(),
            ..Default::default()
        },
        LocalGame::new(),
    );
}

/// Starts an online arena session.
///
/// # Errors
///
/// Returns an error when configuration is invalid, a connection cannot be established, an
/// opening message cannot be encoded or sent, or the loaded mesh count exceeds the engine ID
/// space.
// This public entry point retains ownership of its configuration for API compatibility.
#[allow(clippy::needless_pass_by_value)]
pub fn run_online(cfg: OnlineConfig) -> Result<(), String> {
    // Tracing first, so asset-loading diagnostics are visible.
    #[cfg(not(target_arch = "wasm32"))]
    ember_engine::init_diagnostics();
    let (mut meshes, assets) = online::load_assets();
    // Textured environment set (arena v8): registered after the GLB parts.
    let env_base = u32::try_from(meshes.len())
        .map_err(|_| "viewmodel mesh count exceeds u32".to_string())?
        + 1; // 0 is the built-in cube
    meshes.extend(online::env_meshes());
    // Articulated character parts (arena v9), after the env set.
    let parts_base = u32::try_from(meshes.len())
        .map_err(|_| "environment mesh count exceeds u32".to_string())?
        + 1;
    let (part_meshes, parts) = online::part_meshes(parts_base);
    meshes.extend(part_meshes);
    // Trench City props (arena v13), after the character: cover by kind,
    // the city, sky and ground. This slot held the factory skyline (arena
    // v10), which the arena no longer draws.
    let props_base = u32::try_from(meshes.len())
        .map_err(|_| "character mesh count exceeds u32".to_string())?
        + 1;
    let prop_meshes = props::prop_meshes();
    let prop_fits = props::measure(&prop_meshes);
    meshes.extend(prop_meshes);
    // The rounds (arena v20): the five bullet meshes, the streak cone, the
    // core frustum, the hole disc and the particle puff, after the props,
    // in `rounds::Round` order with the streak, the core, the disc and then
    // the puff last. A new mesh in this group goes on the END of it, or
    // every id after it shifts.
    let rounds_base =
        u32::try_from(meshes.len()).map_err(|_| "prop mesh count exceeds u32".to_string())? + 1;
    meshes.extend(rounds::round_meshes());
    let mut game = online::ShooterGame::connect(&cfg, assets)?;
    game.set_env_base(env_base);
    game.set_parts(parts);
    game.set_props(props_base, &prop_fits);
    game.set_rounds(rounds_base);
    ember_engine::run(
        EngineConfig {
            title: format!("ember arena — {}", cfg.lobby),
            // A scripted client (`EMBER_SCRIPT`) never grabs the cursor: the
            // operator keeps their pointer while a capture runs, and since
            // the grab is refused here, once, it cannot come back when the
            // script ends. See `script`.
            capture_mouse: !script::scripted(),
            // …and it does not take the foreground when it opens either: the
            // operator keeps the window they were typing into.
            activate: !script::scripted(),
            meshes,
        },
        game,
    );
    Ok(())
}

// ---- wasm entry points (the page menu calls these) ----

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use wasm_bindgen::prelude::*;

    /// Module init: hooks only. The game starts when the page picks a mode.
    #[wasm_bindgen(start)]
    pub fn wasm_init() {
        console_error_panic_hook_lite();
    }

    fn console_error_panic_hook_lite() {
        // ember-engine installs the real hooks inside run(); nothing needed
        // here, but keeping the start fn explicit documents the contract.
    }

    /// The protocol version this build speaks — the page's plain-JS lobby
    /// browser uses it instead of hardcoding a number that can drift.
    #[wasm_bindgen]
    pub fn proto_version() -> u16 {
        arena_core::proto::PROTO_VERSION
    }

    #[wasm_bindgen]
    pub fn start_local() {
        super::run_local();
    }

    /// config JSON: {url, action: "create"|"join", lobby, password?, handle}
    #[wasm_bindgen]
    pub fn start_online(config_json: &str) -> Result<(), JsValue> {
        let cfg: super::OnlineConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("bad config: {e}")))?;
        super::run_online(cfg).map_err(|e| JsValue::from_str(&e))
    }
}
