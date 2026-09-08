//! End Game v2: a single-player Ember dungeon, native and WASM.
mod cell;
mod scene;

use ember_engine::{
    EmberGame, EngineConfig, Feedback, Frame, InputState, KeyCode, MouseButton, PadButton,
};
use end_game_core::{Controls, Dungeon, STEP};
use glam::Vec2;
use std::cell::RefCell;

#[derive(Default, Clone, Copy)]
struct Ui {
    movement: Vec2,
    look: Vec2,
    held: u32,
    actions: u32,
    paused: bool,
    third_person: bool,
}
thread_local! {
    static UI: RefCell<Ui> = RefCell::new(Ui::default());
    static HUD: RefCell<String> = const { RefCell::new(String::new()) };
}
pub struct Game {
    pub sim: Dungeon,
    scene: scene::Scene,
    accumulated: f32,
    previous: u32,
    third_person: bool,
    feedback: Feedback,
    wake: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_engine::PadState;

    fn game() -> Game {
        UI.with(|u| *u.borrow_mut() = Ui::default());
        let (scene, _) = scene::Scene::build();
        Game {
            sim: Dungeon::default(),
            scene,
            accumulated: 0.0,
            previous: 0,
            third_person: false,
            feedback: Feedback::default(),
            wake: 0.0,
        }
    }

    #[test]
    fn high_refresh_camera_toggle_is_consumed_once() {
        let mut g = game();
        let held = InputState::from_parts(&[KeyCode::KeyV], &[], (0.0, 0.0), None);
        g.update(&held, STEP * 0.3);
        assert!(g.third_person);
        g.update(&InputState::default(), STEP * 0.3);
        assert!(g.third_person);
        g.update(&InputState::default(), STEP);
        assert!(g.third_person);
    }

    #[test]
    fn short_tap_survives_until_tick_without_repeating() {
        let mut g = game();
        g.sim.position = glam::Vec3::new(-1.25, 0.0, 2.1);
        let tap = InputState::from_parts(&[KeyCode::KeyE], &[], (0.0, 0.0), None);
        g.update(&tap, STEP * 0.2);
        assert_eq!(g.sim.stage, 0);
        g.update(&InputState::default(), STEP);
        assert_eq!(g.sim.stage, 1);
        g.update(&InputState::default(), STEP * 3.0);
        assert_eq!(g.sim.stage, 1);
    }

    #[test]
    fn standard_gamepad_and_touch_drive_the_same_motion() {
        let mut pad_game = game();
        let pad = InputState::from_parts(
            &[],
            &[],
            (0.0, 0.0),
            Some(PadState {
                left: [0.5, 0.8],
                ..PadState::default()
            }),
        );
        pad_game.update(&pad, STEP);
        let expected = pad_game.sim.position;
        let mut touch_game = game();
        UI.with(|u| u.borrow_mut().movement = Vec2::new(0.5, 0.8));
        touch_game.update(&InputState::default(), STEP);
        assert!(touch_game.sim.position.abs_diff_eq(expected, 1e-6));
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.paused = true;
            u.actions = 2;
        });
        touch_game.update(&pad, STEP * 3.0);
        assert_eq!(touch_game.sim.position, expected);
        assert_eq!(touch_game.sim.attack_time, 0.0);
    }
}

pub fn run() {
    let (scene, meshes) = scene::Scene::build();
    #[allow(unused_mut)] // Native capture configures a passive snapshot before launch.
    let mut game = Game {
        sim: Dungeon::default(),
        scene,
        accumulated: 0.0,
        previous: 0,
        third_person: false,
        feedback: Feedback::default(),
        wake: 1.0,
    };
    #[allow(unused_mut)]
    let mut passive = false;
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("EMBER_CAPTURE_PATH").is_some() {
        passive = true;
        game.wake = 0.0;
        match std::env::var("END_GAME_SCENE").as_deref() {
            Ok("cell-detail") => {
                game.sim.position = glam::Vec3::new(1.5, 0.0, 1.3);
                game.sim.yaw = -2.35;
                game.sim.pitch = -0.08;
            }
            Ok("cell-wall") => {
                game.sim.position = glam::Vec3::new(-0.5, 0.0, 2.4);
                game.sim.yaw = -1.35;
                game.sim.pitch = -0.08;
            }
            Ok("corridor") => {
                game.sim.stage = 3;
                game.sim.position = glam::Vec3::new(0.0, 0.0, -0.8);
                game.sim.yaw = -0.3;
            }
            Ok("wolf") => {
                game.third_person = true;
                game.sim.stage = 4;
                game.sim.werewolf = true;
                game.sim.position = glam::Vec3::new(2.2, 0.0, -3.6);
                game.sim.yaw = 1.5;
                game.sim.transformation = 2.0;
            }
            Ok("sword") => {
                game.sim.stage = 4;
                game.sim.position = glam::Vec3::new(1.0, 0.0, -3.0);
            }
            _ => {}
        }
        game.sim.time = std::env::var("END_GAME_TIME")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        game.sim.gate_open = if game.sim.stage >= 3 { 1.0 } else { 0.0 };
        UI.with(|u| u.borrow_mut().paused = true);
    }
    ember_engine::run(
        EngineConfig {
            title: format!("End Game — {}", env!("CARGO_PKG_VERSION")),
            capture_mouse: !passive,
            activate: !passive,
            meshes,
        },
        game,
    );
}

impl EmberGame for Game {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let ui = UI.with(|cell| {
            let mut value = cell.borrow_mut();
            let result = *value;
            value.look = Vec2::ZERO;
            value.actions = 0;
            result
        });
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.10)
        } else {
            0.0
        };
        let pad = input.pad().unwrap_or_default();
        let bits = u32::from(input.down(KeyCode::KeyE) || pad.down(PadButton::West))
            | (u32::from(
                input.mouse_down(MouseButton::Left)
                    || pad.down(PadButton::RT)
                    || pad.down(PadButton::RB),
            ) << 1)
            | (u32::from(input.down(KeyCode::Space) || pad.down(PadButton::South)) << 2)
            | (u32::from(input.down(KeyCode::AltLeft) || pad.down(PadButton::East)) << 3)
            | (u32::from(input.down(KeyCode::KeyQ) || pad.down(PadButton::North)) << 4)
            | (u32::from(input.down(KeyCode::KeyV) || pad.down(PadButton::R3)) << 5);
        let mut pressed = (bits & !self.previous) | ui.actions;
        self.previous = bits;
        if !ui.paused {
            if pressed & 32 != 0 || ui.third_person {
                self.third_person = !self.third_person;
                UI.with(|u| u.borrow_mut().third_person = false);
            }
            pressed &= !32;
            let (dx, dy) = input.mouse_delta();
            self.sim.yaw += dx * 0.0021 + pad.right[0] * dt * 2.25 + ui.look.x * 0.003;
            self.sim.pitch = (self.sim.pitch - dy * 0.0021 + pad.right[1] * dt * 1.65
                - ui.look.y * 0.003)
                .clamp(-1.2, 1.15);
            self.wake = (self.wake - dt * 0.35).max(0.0);
            self.accumulated += dt;
            let old_event = self.sim.event;
            while self.accumulated >= STEP {
                self.sim.tick(Controls {
                    movement: Vec2::new(
                        input.axis(KeyCode::KeyA, KeyCode::KeyD) + pad.left[0],
                        input.axis(KeyCode::KeyS, KeyCode::KeyW) + pad.left[1],
                    ) + ui.movement,
                    sprint: input.down(KeyCode::ShiftLeft)
                        || pad.down(PadButton::L3)
                        || ui.held & 1 != 0,
                    crouch: input.down(KeyCode::KeyC)
                        || pad.down(PadButton::LB)
                        || ui.held & 2 != 0,
                    interact: pressed & 1 != 0,
                    attack: pressed & 2 != 0,
                    jump: pressed & 4 != 0,
                    dodge: pressed & 8 != 0,
                    transform: pressed & 16 != 0,
                });
                pressed = 0;
                self.accumulated -= STEP;
            }
            // Preserve a tap when a high-refresh frame has not reached the next sim tick.
            if pressed != 0 {
                UI.with(|u| u.borrow_mut().actions |= pressed);
            }
            if self.sim.event != old_event {
                self.feedback.rumble(
                    if self.sim.attack_time > 0.0 {
                        0.7
                    } else {
                        0.32
                    },
                    0.25,
                    150,
                );
            }
        } else {
            self.accumulated = 0.0;
        }
        let state = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"), "stage": self.sim.stage, "objective": self.sim.objective(),
            "prompt": self.sim.prompt(), "health": self.sim.health, "stamina": self.sim.stamina,
            "form": if self.sim.werewolf { "Werewolf" } else { "Wolf" }, "alert": self.sim.alert,
            "event": self.sim.event, "message": self.sim.message, "footsteps": self.sim.footsteps,
            "crouched": self.sim.crouched, "pad": input.pad().is_some(), "transformation": self.sim.transformation,
            "position": self.sim.position.to_array(), "time": self.sim.time,
            "physics": { "gravity": end_game_core::GRAVITY, "crateMass": self.sim.body.mass(), "crateWear": self.sim.body.wear }
        });
        HUD.with(|hud| *hud.borrow_mut() = state.to_string());
        self.scene.frame(&self.sim, self.third_person, self.wake)
    }
    fn feedback(&mut self) -> Feedback {
        std::mem::take(&mut self.feedback)
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub fn start() {
        super::run();
    }
    #[wasm_bindgen]
    pub fn state_json() -> String {
        HUD.with(|hud| hud.borrow().clone())
    }
    #[wasm_bindgen]
    pub fn touch_input(x: f32, y: f32, look_x: f32, look_y: f32, held: u32) {
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.movement = Vec2::new(x, y).clamp_length_max(1.0);
            u.look += Vec2::new(look_x, look_y);
            u.held = held;
        });
    }
    #[wasm_bindgen]
    pub fn action(mask: u32) {
        UI.with(|u| u.borrow_mut().actions |= mask & 63);
    }
    #[wasm_bindgen]
    pub fn pause(paused: bool) {
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.paused = paused;
            u.movement = Vec2::ZERO;
            u.look = Vec2::ZERO;
            u.actions = 0;
            u.held = 0;
        });
    }
}
