//! 3D Pong for two players at one keyboard.
//! Player 1 (blue, near): A / D.  Player 2 (red, far): ← / →.
//! First to 7. Runs native and on the web (wasm) on the same engine.

mod sim;

use ember_engine::glam::Vec3;
use ember_engine::{Camera, EmberGame, EngineConfig, Frame, InputState, Instance, KeyCode};

use sim::{Phase, Sim, BALL_R, COURT_END_Z, COURT_HALF_W, FIXED_DT, PADDLE_HALF_W, PADDLE_Z};

const P1_COLOR: Vec3 = Vec3::new(0.25, 0.55, 0.95);
const P2_COLOR: Vec3 = Vec3::new(0.92, 0.32, 0.28);
const BALL_COLOR: Vec3 = Vec3::new(0.95, 0.93, 0.80);

struct Pong {
    sim: Sim,
    /// Render-time accumulator driving fixed sim steps.
    accumulator: f32,
}

impl Pong {
    fn new() -> Self {
        Self { sim: Sim::new(), accumulator: 0.0 }
    }
}

impl EmberGame for Pong {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let p1 = input.axis(KeyCode::KeyA, KeyCode::KeyD);
        let p2 = input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight);

        // Fixed 60 Hz sim regardless of display rate.
        self.accumulator = (self.accumulator + dt).min(0.25);
        while self.accumulator >= FIXED_DT {
            self.accumulator -= FIXED_DT;
            self.sim.step(p1, p2);
            if let Some((scorer, won)) = self.sim.event {
                if won {
                    log::info!("player {} WINS the game!", scorer + 1);
                } else {
                    log::info!(
                        "player {} scores ({} : {})",
                        scorer + 1,
                        self.sim.score[0],
                        self.sim.score[1]
                    );
                }
            }
        }

        let mut frame = Frame {
            camera: Camera {
                eye: Vec3::new(0.0, 24.0, 30.0),
                target: Vec3::new(0.0, 0.0, -1.0),
                fov_y_deg: 50.0,
            },
            instances: Vec::with_capacity(32),
        };
        let inst = |frame: &mut Frame, pos: Vec3, scale: Vec3, color: Vec3| {
            frame.instances.push(Instance { position: pos, scale, color });
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
        let dashes = 9;
        for i in 0..dashes {
            let x = -COURT_HALF_W + (i as f32 + 0.5) * (COURT_HALF_W * 2.0 / dashes as f32);
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
            Vec3::new(self.sim.p1_x, 0.5, PADDLE_Z),
            Vec3::new(PADDLE_HALF_W * 2.0, 1.0, 0.8),
            P1_COLOR,
        );
        inst(
            &mut frame,
            Vec3::new(self.sim.p2_x, 0.5, -PADDLE_Z),
            Vec3::new(PADDLE_HALF_W * 2.0, 1.0, 0.8),
            P2_COLOR,
        );

        // Ball (slightly raised pulse while serving so players see it coming).
        let ball_y = match self.sim.phase {
            Phase::Serving { timer, .. } => 0.5 + (timer * 6.0).sin().abs() * 0.4,
            Phase::Playing => 0.5,
        };
        inst(
            &mut frame,
            Vec3::new(self.sim.ball_pos[0], ball_y, self.sim.ball_pos[1]),
            Vec3::splat(BALL_R * 2.0),
            BALL_COLOR,
        );

        // Score pips on top of each wall: your points march toward your side.
        for (idx, (color, sign)) in [(P1_COLOR, 1.0f32), (P2_COLOR, -1.0f32)].iter().enumerate() {
            for i in 0..self.sim.score[idx] {
                inst(
                    &mut frame,
                    Vec3::new(
                        (COURT_HALF_W + 0.75) * -sign, // P1 pips on the left wall
                        1.25,
                        (11.0 - i as f32 * 1.7) * sign,
                    ),
                    Vec3::splat(0.55),
                    *color,
                );
            }
        }

        frame
    }
}

pub fn run() {
    ember_engine::run(
        EngineConfig { title: "ember pong — P1: A/D, P2: ←/→".to_string() },
        Pong::new(),
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_start() {
    run();
}
