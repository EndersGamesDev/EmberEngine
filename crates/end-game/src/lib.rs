//! End Game v11: a single-player Ember dungeon, native and WASM.
mod camera;
#[cfg(not(target_arch = "wasm32"))]
mod capture;
mod castle;
mod cell;
mod enemies;
mod hands;
mod marks;
mod quest;
mod scene;
mod sword_motion;
mod warden;

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
    attacks: u8,
    paused: bool,
    guard_reset: bool,
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
    guard_release_required: bool,
    third_person: bool,
    feedback: Feedback,
    wake: f32,
    stair_eye: camera::StairEye,
}

/// Project in vertical-FOV units; the shell applies its current viewport aspect.
fn aim_projection(camera: &ember_engine::Camera, point: glam::Vec3) -> Option<Vec2> {
    let forward = (camera.target - camera.eye).try_normalize()?;
    let right = forward.cross(glam::Vec3::Y).try_normalize()?;
    let up = right.cross(forward);
    let delta = point - camera.eye;
    let depth = delta.dot(forward);
    if depth <= 0.1 {
        return None;
    }
    let scale = depth * (camera.fov_y_deg.to_radians() * 0.5).tan();
    let projected = Vec2::new(delta.dot(right), delta.dot(up)) / scale;
    projected.is_finite().then_some(projected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_engine::PadState;
    use end_game_core::StrikeKind;

    #[test]
    fn aim_marker_matches_renderer_projection_in_both_camera_modes() {
        let mut g = game();
        g.sim.stage = 5;
        g.sim.position = glam::Vec3::new(0.0, 6.0, -70.0);
        g.sim.yaw = 0.4;
        g.sim.pitch = -0.2;
        g.sim.combat.impact_left = 0.15;
        g.sim.combat.impact_strength = 0.7;
        let target = g.sim.sword_aim_point();
        for third_person in [false, true] {
            let frame = g
                .scene
                .frame_with_camera_lift(&g.sim, third_person, 0.0, -0.2);
            let projected = aim_projection(&frame.camera, target).unwrap();
            for aspect in [9.0 / 16.0, 16.0 / 9.0, 32.0 / 9.0] {
                let clip = frame.camera.view_proj(aspect) * target.extend(1.0);
                let ndc = clip.truncate() / clip.w;
                assert!((projected.x / aspect - ndc.x).abs() < 0.0001);
                assert!((projected.y - ndc.y).abs() < 0.0001);
            }
            let behind = frame.camera.eye - (frame.camera.target - frame.camera.eye);
            assert!(aim_projection(&frame.camera, behind).is_none());
        }
    }

    fn game() -> Game {
        UI.with(|u| *u.borrow_mut() = Ui::default());
        let (scene, _) = scene::Scene::build();
        Game {
            sim: Dungeon::default(),
            scene,
            accumulated: 0.0,
            previous: 0,
            guard_release_required: false,
            third_person: false,
            feedback: Feedback::default(),
            wake: 0.0,
            stair_eye: camera::StairEye::default(),
        }
    }

    #[test]
    fn castle_movement_guard_and_hud_continue_after_the_basement_escape() {
        let mut g = game();
        g.sim.stage = 5;
        g.sim.exit_open = 1.0;
        g.sim.warden_health = 0.0;
        g.sim.position = glam::Vec3::new(0.0, 6.0, -70.0);
        let from = g.sim.position;
        let walking = InputState::from_parts(&[KeyCode::KeyW], &[], (0.0, 0.0), None);
        g.update(&walking, STEP * 6.0);
        assert!(g.sim.position.z < from.z);
        assert!((g.sim.position.y - 6.0).abs() < 0.001);
        assert!(g.sim.grounded);
        let guarding = InputState::from_parts(&[KeyCode::KeyF], &[], (0.0, 0.0), None);
        for _ in 0..3 {
            g.update(&guarding, STEP * 6.0);
        }
        assert!(g.sim.guard.ready());
        let state: serde_json::Value = HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
        assert_eq!(state["finished"], false);
        assert_eq!(state["location"], "Backyard garden");
        assert_eq!(state["exploration"]["total"], 7);
        UI.with(|u| u.borrow_mut().paused = true);
        let paused_position = g.sim.position;
        g.update(&walking, STEP * 6.0);
        assert_eq!(g.sim.position, paused_position);
    }

    #[test]
    fn castle_boss_contact_journal_and_ending_reach_the_shell() {
        use end_game_core::enemies::{Enemy, EnemyAttack, EnemyKind};
        let mut g = game();
        g.sim.stage = 5;
        g.sim.exit_open = 1.0;
        g.sim.warden_health = 0.0;
        g.sim.position = glam::Vec3::new(0.0, 0.0, -42.0);
        for (kind, attack, cost) in [
            (EnemyKind::HollowAxeKnight, EnemyAttack::Chop, 32.0),
            (EnemyKind::Cyclops, EnemyAttack::Sweep, 38.0),
        ] {
            let mut enemy = Enemy::new(0, kind, glam::Vec3::new(0.0, 0.0, -44.0));
            enemy.alerted = true;
            enemy.start_attack(g.sim.position);
            enemy.attack = attack;
            g.sim.enemies = vec![enemy];
            g.update(&InputState::default(), STEP);
            let state: serde_json::Value =
                HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
            assert_eq!(state["enemy"]["blockCost"], cost);
        }
        let mut boss = Enemy::new(0, EnemyKind::Cyclops, glam::Vec3::new(0.0, 0.0, -44.0));
        boss.alerted = true;
        boss.start_attack(g.sim.position);
        boss.attack = EnemyAttack::Slam;
        boss.elapsed = EnemyAttack::Slam.contact_time() - STEP;
        g.sim.enemies = vec![boss];
        g.sim.quest.inscription_read = true;
        g.sim.quest.sequence = 2;
        g.sim.quest.shrines_used = 5;
        g.update(&InputState::default(), STEP);
        let state: serde_json::Value = HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
        assert_eq!(state["enemy"]["boss"], true);
        assert_eq!(state["enemy"]["maxHealth"], 360.0);
        assert!(state["enemy"]["blockCost"].is_null());
        assert_eq!(state["quest"]["shrinesUsed"], 2);
        assert!(state["quest"]["journal"].as_array().unwrap().len() >= 3);
        assert!(state["enemyCombat"]["hitEvent"].as_u64().unwrap() > 0);
        assert_eq!(g.sim.health, 66.0);
        assert!(g.feedback().rumbles.iter().any(|r| r.strong > 0.5));
        g.sim.quest.escaped = true;
        g.update(&InputState::default(), STEP);
        let state: serde_json::Value = HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
        assert_eq!(state["finished"], true);
    }

    fn combat_game() -> Game {
        let mut g = game();
        g.sim.stage = 4;
        g.sim.warden_health = 0.0;
        g
    }

    fn observe_strikes(g: &mut Game, input: &InputState, frames: usize) -> Vec<StrikeKind> {
        let mut strikes: Vec<_> = g.sim.combat.active.iter().map(|s| s.kind).collect();
        for _ in 0..frames {
            let previous = g.sim.combat.swing_event;
            // Six fixed ticks per frame keeps asset-backed tests bounded. Even
            // the shortest strike lasts longer, so no start can be skipped.
            g.update(input, STEP * 6.0);
            if g.sim.combat.swing_event != previous {
                assert_eq!(g.sim.combat.swing_event, previous + 1);
                strikes.push(g.sim.combat.active.unwrap().kind);
            }
        }
        strikes
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
    fn dialogue_snapshot_keeps_story_events_across_pause_and_clears_on_respawn() {
        let mut g = game();
        g.sim.stage = 2;
        g.sim.position = end_game_core::InteractionKind::Lock.approach();
        g.sim.interact();
        for _ in 0..23 {
            g.update(&InputState::default(), 0.1);
        }
        let snapshot =
            || HUD.with(|hud| serde_json::from_str::<serde_json::Value>(&hud.borrow()).unwrap());
        let before = snapshot()["dialogue"].clone();
        assert!(
            before["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["kind"] == "key")
        );
        UI.with(|u| u.borrow_mut().paused = true);
        g.update(&InputState::default(), 1.0);
        assert_eq!(snapshot()["dialogue"], before);
        UI.with(|u| u.borrow_mut().paused = false);
        g.sim.health = 0.0;
        g.update(&InputState::default(), STEP);
        let after = snapshot()["dialogue"].clone();
        assert_eq!(
            after["life"].as_u64().unwrap(),
            before["life"].as_u64().unwrap() + 1
        );
        assert!(after["events"].as_array().unwrap().is_empty());
        assert_eq!(after["dead"], false);
    }

    #[test]
    fn pausing_a_knife_windup_freezes_the_enemy_then_resumes_contact_and_feedback() {
        use end_game_core::warden::{KNIFE_CONTACT, WardenPhase};
        let mut g = game();
        g.sim.stage = 3;
        g.sim.position = glam::Vec3::new(-2.9, 0.0, -2.55);
        g.sim.alert = 1.0;
        g.sim.warden_ai.phase = WardenPhase::Attacking;
        g.sim.warden_ai.elapsed = 0.4;
        let paused_time = g.sim.time;
        let phase_time = g.sim.warden_ai.elapsed;
        UI.with(|u| u.borrow_mut().paused = true);
        for _ in 0..12 {
            g.update(&InputState::default(), 0.1);
        }
        assert_eq!(g.sim.time, paused_time);
        assert_eq!(g.sim.warden_ai.phase, WardenPhase::Attacking);
        assert_eq!(g.sim.warden_ai.elapsed, phase_time);
        assert_eq!(g.sim.health, 100.0);
        UI.with(|u| u.borrow_mut().paused = false);
        while g.sim.warden_ai.elapsed + STEP < KNIFE_CONTACT {
            g.update(&InputState::default(), STEP);
            assert_eq!(g.sim.health, 100.0);
        }
        g.update(&InputState::default(), STEP);
        assert_eq!(g.sim.health, 85.0);
        assert_eq!(g.sim.warden_ai.hit_event, 1);
        assert!(!g.feedback().rumbles.is_empty());
        let snapshot: serde_json::Value =
            HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
        assert_eq!(snapshot["warden"]["phase"], "Attacking");
        assert_eq!(snapshot["warden"]["hitEvent"], 1);
        assert!(snapshot["warden"]["hitLeft"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn short_tap_survives_until_tick_without_repeating() {
        let mut g = game();
        g.sim.position = glam::Vec3::new(-1.25, 0.0, 2.1);
        let tap = InputState::from_parts(&[KeyCode::KeyE], &[], (0.0, 0.0), None);
        g.update(&tap, STEP * 0.2);
        assert_eq!(g.sim.stage, 0);
        g.update(&InputState::default(), STEP);
        assert!(g.sim.interaction.is_some());
        g.update(&InputState::default(), STEP * 3.0);
        assert_eq!(g.sim.stage, 0);
        for _ in 0..100 {
            g.update(&InputState::default(), STEP);
        }
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

    #[test]
    fn pausing_freezes_pickup_and_busy_look_and_taps_do_not_queue() {
        let mut g = game();
        g.sim.position = end_game_core::InteractionKind::Key.approach();
        g.sim.stage = 1;
        g.sim.interact();
        g.update(&InputState::default(), STEP * 5.0);
        let elapsed = g.sim.interaction.unwrap().elapsed;
        UI.with(|u| u.borrow_mut().paused = true);
        for _ in 0..20 {
            g.update(&InputState::default(), STEP * 3.0);
        }
        assert_eq!(g.sim.interaction.unwrap().elapsed, elapsed);
        UI.with(|u| u.borrow_mut().paused = false);
        let input = InputState::from_parts(&[KeyCode::KeyE], &[], (90.0, 90.0), None);
        let yaw = g.sim.yaw;
        let pitch = g.sim.pitch;
        g.update(&input, STEP * 0.2);
        assert_eq!((g.sim.yaw, g.sim.pitch), (yaw, pitch));
        for _ in 0..150 {
            g.update(&InputState::default(), STEP);
        }
        assert_eq!(g.sim.stage, 2);
        assert!(g.sim.interaction.is_none());
    }

    #[test]
    fn combat_input_counted_api_taps_before_a_tick_produce_the_exact_quick_chain() {
        let mut g = combat_game();
        // The WASM action API counts each canvas/touch press in this field.
        UI.with(|u| u.borrow_mut().attacks = 3);
        g.update(&InputState::default(), STEP * 0.25);
        assert!(g.sim.combat.active.is_none());
        assert_eq!(UI.with(|u| u.borrow().attacks), 3);

        g.update(&InputState::default(), STEP);
        assert_eq!(g.sim.combat.active.unwrap().kind, StrikeKind::Cut);
        assert_eq!(g.sim.combat.queued_count(), 0);
        assert_eq!(UI.with(|u| u.borrow().attacks), 2);
        g.update(&InputState::default(), STEP);
        assert_eq!(g.sim.combat.queued(), [Some(StrikeKind::Backhand), None]);
        assert_eq!(UI.with(|u| u.borrow().attacks), 1);
        g.update(&InputState::default(), STEP);
        assert_eq!(
            g.sim.combat.queued(),
            [Some(StrikeKind::Backhand), Some(StrikeKind::Finisher)]
        );
        assert_eq!(UI.with(|u| u.borrow().attacks), 0);

        assert_eq!(
            observe_strikes(&mut g, &InputState::default(), 36),
            [StrikeKind::Cut, StrikeKind::Backhand, StrikeKind::Finisher]
        );
        assert!(g.sim.combat.finished());
        assert_eq!(g.sim.combat.swing_event, 3);
    }

    #[test]
    fn combat_input_held_mouse_and_gamepad_attack_once_until_released() {
        for held in [
            InputState::from_parts(&[], &[MouseButton::Left], (0.0, 0.0), None),
            InputState::from_parts(
                &[],
                &[],
                (0.0, 0.0),
                Some(PadState {
                    buttons: PadButton::RB.mask(),
                    ..PadState::default()
                }),
            ),
        ] {
            let mut g = combat_game();
            g.update(&held, STEP * 0.25);
            assert!(g.sim.combat.active.is_none());
            assert_eq!(observe_strikes(&mut g, &held, 24), [StrikeKind::Cut]);
            assert!(g.sim.combat.finished());
            assert_eq!(g.sim.combat.swing_event, 1);
            assert_eq!(UI.with(|u| u.borrow().attacks), 0);

            g.update(&InputState::default(), STEP);
            g.update(&held, STEP);
            assert_eq!(g.sim.combat.swing_event, 2);
            assert_eq!(g.sim.combat.active.unwrap().kind, StrikeKind::Cut);
        }
    }

    #[test]
    fn heavy_input_is_distinct_counted_once_and_preserved_before_the_next_tick() {
        for input in [
            InputState::from_parts(&[KeyCode::KeyR], &[], (0.0, 0.0), None),
            InputState::from_parts(&[], &[MouseButton::Middle], (0.0, 0.0), None),
            InputState::from_parts(
                &[],
                &[],
                (0.0, 0.0),
                Some(PadState {
                    buttons: PadButton::RT.mask(),
                    ..PadState::default()
                }),
            ),
        ] {
            let mut g = combat_game();
            g.update(&input, STEP * 0.2);
            g.update(&InputState::default(), STEP * 0.2);
            assert!(g.sim.combat.active.is_none());
            assert_eq!(UI.with(|u| u.borrow().actions & 64), 64);
            g.update(&InputState::default(), STEP);
            assert_eq!(g.sim.combat.active.unwrap().kind, StrikeKind::Overhead);
            assert_eq!(g.sim.combat.swing_event, 1);
            observe_strikes(&mut g, &input, 30);
            assert_eq!(g.sim.combat.swing_event, 1);
            g.update(&InputState::default(), STEP);
            g.update(&input, STEP);
            assert_eq!(g.sim.combat.swing_event, 2);
        }
    }

    #[test]
    fn heavy_action_wins_light_batch_and_jump_heavy_is_a_separate_animation() {
        let mut g = combat_game();
        UI.with(|u| {
            let mut ui = u.borrow_mut();
            ui.actions = 64;
            ui.attacks = 3;
        });
        g.update(&InputState::default(), STEP);
        assert_eq!(g.sim.combat.active.unwrap().kind, StrikeKind::Overhead);
        assert_eq!(UI.with(|u| u.borrow().attacks), 0);
        assert_eq!(g.sim.combat.queued_count(), 0);
        observe_strikes(&mut g, &InputState::default(), 30);
        assert_eq!(g.sim.combat.swing_event, 1);

        let mut jumping = combat_game();
        UI.with(|u| u.borrow_mut().actions = 4 | 64);
        jumping.update(&InputState::default(), STEP);
        assert!(!jumping.sim.grounded);
        assert_eq!(
            jumping.sim.combat.active.unwrap().kind,
            StrikeKind::JumpHeavy
        );
        let state: serde_json::Value = HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
        assert_eq!(
            state["combat"]["active"]["kind"],
            StrikeKind::JumpHeavy.label()
        );
    }

    #[test]
    fn guard_and_pause_discard_heavy_edges_without_replaying_a_held_trigger() {
        for pause in [false, true] {
            let mut g = combat_game();
            let held = InputState::from_parts(
                &[],
                &[],
                (0.0, 0.0),
                Some(PadState {
                    buttons: PadButton::RT.mask(),
                    ..PadState::default()
                }),
            );
            UI.with(|u| {
                let mut ui = u.borrow_mut();
                ui.actions = 64;
                ui.held = if pause { 0 } else { 4 };
                ui.paused = pause;
            });
            g.update(&held, STEP);
            assert!(g.sim.combat.active.is_none());
            UI.with(|u| {
                let mut ui = u.borrow_mut();
                ui.paused = false;
                ui.held = 0;
            });
            observe_strikes(&mut g, &held, 5);
            assert!(g.sim.combat.active.is_none());
            g.update(&InputState::default(), STEP);
            g.update(&held, STEP);
            assert_eq!(g.sim.combat.active.unwrap().kind, StrikeKind::Overhead);
        }
    }

    #[test]
    fn combat_input_high_refresh_mouse_taps_survive_release_before_the_tick() {
        let mut g = combat_game();
        let pressed = InputState::from_parts(&[], &[MouseButton::Left], (0.0, 0.0), None);
        for count in 1..=3 {
            g.update(&pressed, STEP * 0.1);
            g.update(&InputState::default(), STEP * 0.1);
            assert!(g.sim.combat.active.is_none());
            assert_eq!(UI.with(|u| u.borrow().attacks), count);
        }
        g.update(&InputState::default(), STEP * 3.0);
        assert_eq!(g.sim.combat.swing_event, 1);
        assert_eq!(g.sim.combat.queued_count(), 2);
        assert_eq!(UI.with(|u| u.borrow().attacks), 0);
        assert_eq!(
            observe_strikes(&mut g, &InputState::default(), 36),
            [StrikeKind::Cut, StrikeKind::Backhand, StrikeKind::Finisher]
        );
        assert!(g.sim.combat.finished());
        assert_eq!(g.sim.combat.swing_event, 3);
    }

    #[test]
    fn combat_input_pause_freezes_hitstop_discards_new_taps_and_resumes_existing_queue() {
        let mut g = combat_game();
        g.sim.position = glam::Vec3::new(0.0, 0.0, -3.0);
        g.sim.warden = Vec2::new(0.0, -4.8);
        g.sim.gate_open = 1.0;
        g.sim.warden_health = 100.0;
        UI.with(|u| u.borrow_mut().attacks = 2);
        g.update(&InputState::default(), STEP * 2.0);
        for _ in 0..30 {
            g.update(&InputState::default(), STEP);
            if g.sim.combat.impact_event > 0 {
                break;
            }
        }
        assert_eq!(g.sim.combat.impact_event, 1);
        assert!(g.sim.combat.hitstop_left > 0.0);
        assert_eq!(g.sim.combat.queued(), [Some(StrikeKind::Backhand), None]);
        let frozen = g.sim.clone();
        let held = InputState::from_parts(&[], &[MouseButton::Left], (0.0, 0.0), None);
        for _ in 0..16 {
            UI.with(|u| {
                let mut u = u.borrow_mut();
                u.paused = true;
                u.attacks = 3;
                u.actions = 4 | 8 | 16 | 32;
                u.movement = Vec2::ONE;
                u.look = Vec2::new(90.0, 90.0);
            });
            g.update(&held, STEP * 6.0);
            assert_eq!(g.sim.combat, frozen.combat);
            assert_eq!(g.sim.position, frozen.position);
            assert_eq!(g.sim.velocity_y, frozen.velocity_y);
            assert_eq!(g.sim.warden, frozen.warden);
            assert_eq!(g.sim.body.position, frozen.body.position);
            assert_eq!(g.sim.body.velocity, frozen.body.velocity);
            assert_eq!((g.sim.yaw, g.sim.pitch), (frozen.yaw, frozen.pitch));
            assert_eq!(
                (g.sim.stamina, g.sim.health),
                (frozen.stamina, frozen.health)
            );
            assert_eq!(g.sim.time, frozen.time);
            assert_eq!(g.accumulated, 0.0);
            assert!(!g.third_person);
            assert_eq!(UI.with(|u| u.borrow().attacks), 0);
            assert_eq!(UI.with(|u| u.borrow().actions), 0);
        }

        // Unpausing clears touch state; the mouse remains held, so it cannot
        // create a fresh edge or replay any presses discarded during pause.
        UI.with(|u| *u.borrow_mut() = Ui::default());
        g.update(&held, STEP);
        assert_eq!(g.sim.combat.active, frozen.combat.active);
        assert!(g.sim.combat.hitstop_left < frozen.combat.hitstop_left);
        assert_eq!(g.sim.combat.queued(), [Some(StrikeKind::Backhand), None]);
        assert_eq!(
            observe_strikes(&mut g, &held, 36),
            [StrikeKind::Cut, StrikeKind::Backhand]
        );
        assert!(g.sim.combat.finished());
        assert_eq!(g.sim.combat.swing_event, 2);
        assert_eq!(g.sim.combat.impact_event, 2);
    }

    #[test]
    fn combat_input_hitstop_freezes_look_without_discarding_buffered_edges() {
        let mut g = combat_game();
        g.sim.position = glam::Vec3::new(0.0, 0.0, -3.0);
        g.sim.warden = Vec2::new(0.0, -4.8);
        g.sim.gate_open = 1.0;
        g.sim.warden_health = 100.0;
        UI.with(|u| u.borrow_mut().attacks = 1);
        for _ in 0..30 {
            g.update(&InputState::default(), STEP);
            if g.sim.combat.impact_event > 0 {
                break;
            }
        }
        assert_eq!(g.sim.combat.impact_event, 1);
        assert!(g.sim.combat.hitstop_left > 0.0);
        let angles = (g.sim.yaw, g.sim.pitch);
        let strike = g.sim.combat.active;
        let look_and_attack = InputState::from_parts(
            &[],
            &[],
            (180.0, -90.0),
            Some(PadState {
                right: [0.8, 0.6],
                buttons: PadButton::RB.mask(),
                ..PadState::default()
            }),
        );
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.look = Vec2::new(110.0, -90.0);
            u.attacks = 1;
        });
        // The touch edge and new shoulder edge arrive together. One enters the
        // core this tick; the other must survive until the next frozen tick.
        // This physical contact is past the quick-combo window, so it selects
        // the delayed overhead branch; another tap cannot extend that branch.
        g.update(&look_and_attack, STEP);
        assert_eq!((g.sim.yaw, g.sim.pitch), angles);
        assert_eq!(g.sim.combat.active, strike);
        assert_eq!(g.sim.combat.queued(), [Some(StrikeKind::Overhead), None]);
        assert_eq!(UI.with(|u| u.borrow().attacks), 1);
        g.update(&look_and_attack, STEP);
        assert_eq!((g.sim.yaw, g.sim.pitch), angles);
        assert_eq!(g.sim.combat.active, strike);
        assert_eq!(g.sim.combat.queued(), [Some(StrikeKind::Overhead), None]);
        assert_eq!(UI.with(|u| u.borrow().attacks), 0);
        while g.sim.combat.hitstop_left > 0.0 {
            g.update(&look_and_attack, STEP);
            assert_eq!((g.sim.yaw, g.sim.pitch), angles);
            assert_eq!(g.sim.combat.active, strike);
        }
        g.update(&look_and_attack, STEP);
        assert_ne!((g.sim.yaw, g.sim.pitch), angles);
        assert!(g.sim.combat.active.unwrap().elapsed > strike.unwrap().elapsed);
        assert_eq!(g.sim.combat.queued_count(), 1);
    }

    #[test]
    fn guard_mouse_keyboard_controller_and_touch_raise_and_release_equally() {
        let inputs = [
            InputState::from_parts(&[], &[MouseButton::Right], (0.0, 0.0), None),
            InputState::from_parts(&[KeyCode::KeyF], &[], (0.0, 0.0), None),
            InputState::from_parts(
                &[],
                &[],
                (0.0, 0.0),
                Some(PadState {
                    buttons: PadButton::LT.mask(),
                    ..PadState::default()
                }),
            ),
            InputState::default(),
        ];
        for (i, input) in inputs.iter().enumerate() {
            let mut g = combat_game();
            if i == 3 {
                UI.with(|u| u.borrow_mut().held = 4);
            }
            for _ in 0..12 {
                g.update(input, STEP);
            }
            assert!(g.sim.guard.ready(), "input source {i}");
            assert_eq!(g.sim.combat.swing_event, 0);
            UI.with(|u| u.borrow_mut().held = 0);
            for _ in 0..8 {
                g.update(&InputState::default(), STEP);
            }
            assert_eq!(g.sim.guard.amount, 0.0);
        }
    }

    #[test]
    fn guard_pause_freezes_pose_then_requires_physical_release_before_rearming() {
        let mut g = combat_game();
        let held = InputState::from_parts(
            &[],
            &[],
            (0.0, 0.0),
            Some(PadState {
                buttons: PadButton::LT.mask(),
                ..PadState::default()
            }),
        );
        for _ in 0..12 {
            g.update(&held, STEP);
        }
        let frozen = g.sim.guard.clone();
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.paused = true;
            u.held = 4;
        });
        // The web platform masks the controller snapshot while paused.
        g.update(&InputState::default(), 0.1);
        assert_eq!(g.sim.guard, frozen);
        assert_eq!(UI.with(|u| u.borrow().held), 0);
        UI.with(|u| u.borrow_mut().paused = false);
        for _ in 0..10 {
            g.update(&held, STEP);
        }
        assert_eq!(g.sim.guard.amount, 0.0);
        g.update(&InputState::default(), STEP);
        for _ in 0..12 {
            g.update(&held, STEP);
        }
        assert!(g.sim.guard.ready());
        // A complete pause/resume between updates must also drop held guard.
        UI.with(|u| u.borrow_mut().guard_reset = true);
        g.update(&held, STEP);
        assert!(!g.sim.guard.ready());
    }

    #[test]
    fn guard_discards_new_attacks_but_waits_for_the_committed_combo() {
        let mut g = combat_game();
        UI.with(|u| u.borrow_mut().attacks = 3);
        for _ in 0..3 {
            g.update(&InputState::default(), STEP);
        }
        assert_eq!(g.sim.combat.queued_count(), 2);
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.held = 4;
            u.attacks = 1;
        });
        for _ in 0..48 {
            g.update(&InputState::default(), STEP * 6.0);
        }
        assert_eq!(g.sim.combat.swing_event, 3);
        assert!(g.sim.combat.finished());
        assert!(g.sim.guard.ready());
        UI.with(|u| u.borrow_mut().held = 0);
        for _ in 0..8 {
            g.update(&InputState::default(), STEP);
        }
        assert_eq!(g.sim.combat.swing_event, 3);
    }

    #[test]
    fn guard_contact_hud_and_rumble_distinguish_block_from_break() {
        use end_game_core::warden::{KNIFE_CONTACT, WardenPhase};
        for (stamina, broken) in [(100.0, false), (10.0, true)] {
            let mut g = game();
            g.sim.stage = 4;
            g.sim.position = glam::Vec3::new(-2.9, 0.0, -2.55);
            g.sim.guard.amount = 1.0;
            g.sim.stamina = stamina;
            UI.with(|u| u.borrow_mut().held = 4);
            // Hold past the window first: this test is about a block.
            for _ in 0..11 {
                g.update(&InputState::default(), STEP);
            }
            g.sim.warden_ai.phase = WardenPhase::Attacking;
            g.sim.warden_ai.elapsed = KNIFE_CONTACT - STEP;
            g.update(&InputState::default(), STEP);
            let snapshot: serde_json::Value =
                HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
            assert_eq!(snapshot["guard"]["breakEvent"], u32::from(broken));
            assert_eq!(snapshot["guard"]["blockEvent"], u32::from(!broken));
            assert_eq!(g.sim.health, if broken { 85.0 } else { 100.0 });
            let feedback = g.feedback();
            assert!(!feedback.rumbles.is_empty());
            assert_eq!(feedback.rumbles[0].strong > 0.5, broken);
            assert_eq!(feedback.rumbles[0].ms > 180, broken);
        }
    }

    #[test]
    fn a_guard_pressed_on_the_contact_frame_reaches_the_hud_as_a_parry() {
        use end_game_core::warden::{KNIFE_CONTACT, WardenPhase};
        let mut g = game();
        g.sim.stage = 4;
        g.sim.position = glam::Vec3::new(-2.9, 0.0, -2.55);
        g.sim.warden_ai.phase = WardenPhase::Attacking;
        g.sim.warden_ai.elapsed = KNIFE_CONTACT - STEP;
        // Guard down: the press on the contact frame is the deflection.
        UI.with(|u| u.borrow_mut().held = 4);
        g.update(&InputState::default(), STEP);
        let snapshot: serde_json::Value =
            HUD.with(|hud| serde_json::from_str(&hud.borrow()).unwrap());
        assert_eq!(snapshot["guard"]["parryEvent"], 1);
        assert_eq!(snapshot["guard"]["blockEvent"], 0);
        assert_eq!(snapshot["guard"]["breakEvent"], 0);
        assert_eq!(g.sim.health, 100.0);
        assert_eq!(g.sim.stamina, 78.0);
        let feedback = g.feedback();
        assert!(!feedback.rumbles.is_empty());
        assert_eq!(feedback.rumbles[0].ms, 140);
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
        guard_release_required: false,
        third_person: false,
        feedback: Feedback::default(),
        wake: 1.0,
        stair_eye: camera::StairEye::default(),
    };
    #[allow(unused_mut)]
    let mut passive = false;
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("EMBER_CAPTURE_PATH").is_some() {
        passive = true;
        game.wake = 0.0;
        match std::env::var("END_GAME_SCENE").as_deref() {
            Ok("castle") | Ok("enemy") => {
                let value = |key: &str, fallback: f32| {
                    std::env::var(key)
                        .ok()
                        .and_then(|s| s.parse::<f32>().ok())
                        .filter(|v| v.is_finite())
                        .unwrap_or(fallback)
                };
                game.sim.stage = 5;
                game.sim.exit_open = value("END_GAME_EXIT_OPEN", 1.0).clamp(0.0, 1.0);
                game.sim.warden_health = 0.0;
                game.sim.warden_ai.die();
                game.sim.warden_ai.elapsed = 2.0;
                game.sim.position = glam::Vec3::new(
                    value("END_GAME_X", 0.0),
                    value("END_GAME_Y", 0.0),
                    value("END_GAME_Z", -24.0),
                );
                game.sim.yaw = value("END_GAME_YAW", 0.0);
                game.sim.pitch = value("END_GAME_PITCH", 0.1).clamp(-1.2, 1.15);
                game.third_person = std::env::var("END_GAME_THIRD_PERSON").as_deref() == Ok("1");
                game.sim.quest.gate_open = value("END_GAME_SALLY_OPEN", 0.0).clamp(0.0, 1.0);
                game.sim.quest.gate_unlocked = game.sim.quest.gate_open > 0.0;
                game.sim.quest.sequence = value("END_GAME_SEALS", 0.0).clamp(0.0, 3.0) as u8;
                game.sim.quest.seal = std::env::var("END_GAME_CROWN").as_deref() == Ok("1");
                if std::env::var("END_GAME_SCENE").as_deref() == Ok("enemy") {
                    use end_game_core::enemies::{EnemyAttack, EnemyPhase};
                    let id = value("END_GAME_ENEMY_ID", 8.0).clamp(0.0, 8.0) as usize;
                    let enemy = &mut game.sim.enemies[id];
                    let distance =
                        value("END_GAME_ENEMY_DISTANCE", if id == 8 { 5.0 } else { 3.0 });
                    game.sim.position = enemy.position + glam::Vec3::Z * distance;
                    game.sim.yaw = 0.0;
                    game.sim.pitch = value("END_GAME_PITCH", if id == 8 { 0.08 } else { -0.10 });
                    enemy.start_attack(game.sim.position);
                    match std::env::var("END_GAME_ENEMY_VIEW").as_deref() {
                        Ok("side") => {
                            game.sim.position = enemy.position + glam::Vec3::X * distance;
                            game.sim.yaw = -std::f32::consts::FRAC_PI_2;
                        }
                        Ok("back") => {
                            game.sim.position = enemy.position - glam::Vec3::Z * distance;
                            game.sim.yaw = std::f32::consts::PI;
                        }
                        _ => {}
                    }
                    enemy.attack = match std::env::var("END_GAME_ENEMY_ATTACK").as_deref() {
                        Ok("slam") => EnemyAttack::Slam,
                        Ok("sweep") => EnemyAttack::Sweep,
                        _ => enemy.attack,
                    };
                    enemy.elapsed = value("END_GAME_ACTION_TIME", enemy.attack.contact_time());
                    enemy.walk_phase = value("END_GAME_WALK_PHASE", 0.0);
                    enemy.phase_two = std::env::var("END_GAME_PHASE_TWO").as_deref() == Ok("1");
                    enemy.phase = match std::env::var("END_GAME_ENEMY_PHASE").as_deref() {
                        Ok("walk") => EnemyPhase::Hunting,
                        Ok("idle") => EnemyPhase::Idle,
                        Ok("dead") => {
                            enemy.health = 0.0;
                            EnemyPhase::Dead
                        }
                        Ok("stagger") => EnemyPhase::Staggered,
                        _ => EnemyPhase::Attacking,
                    };
                    enemy.walk_blend = value(
                        "END_GAME_WALK_BLEND",
                        if enemy.phase == EnemyPhase::Hunting {
                            1.0
                        } else {
                            0.0
                        },
                    )
                    .clamp(0.0, 1.0);
                }
                if std::env::var("END_GAME_ENEMIES").as_deref() == Ok("none") {
                    game.sim.enemies.clear();
                }
            }
            Ok("sword-hit") => capture::sword_hit(&mut game),
            Ok("guard") => {
                use end_game_core::warden::{KNIFE_CONTACT, WardenPhase};
                game.sim.stage = 4;
                game.sim.position = glam::Vec3::new(-2.9, 0.0, -2.2);
                game.sim.pitch = -0.10;
                game.sim.warden_ai.phase = WardenPhase::Hunting;
                game.sim.guard.impact_point = game.sim.position + glam::Vec3::new(0.0, 1.3, -0.7);
                let elapsed = std::env::var("END_GAME_ACTION_TIME")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|t| t.is_finite())
                    .unwrap_or(0.06)
                    .max(0.0);
                match std::env::var("END_GAME_GUARD_PHASE").as_deref() {
                    Ok("raise") => game.sim.guard.amount = (elapsed / 0.18).clamp(0.0, 1.0),
                    Ok("lower") => game.sim.guard.amount = (1.0 - elapsed / 0.12).clamp(0.0, 1.0),
                    Ok("recoil") => {
                        game.sim.guard.amount = 1.0;
                        game.sim.guard.impact_left = (0.24 - elapsed).max(0.0);
                        game.sim.guard.block_event = 1;
                        game.sim.stamina = 72.0;
                        game.sim.warden_ai.phase = WardenPhase::Attacking;
                        game.sim.warden_ai.elapsed = KNIFE_CONTACT + elapsed;
                    }
                    Ok("break") => {
                        game.sim.guard.amount = (1.0 - elapsed / 0.12).clamp(0.0, 1.0);
                        game.sim.guard.broken_left = (0.9 - elapsed).max(0.0);
                        game.sim.guard.break_event = 1;
                        game.sim.stamina = 0.0;
                        game.sim.warden_ai.phase = WardenPhase::Attacking;
                        game.sim.warden_ai.elapsed = KNIFE_CONTACT + elapsed;
                    }
                    _ => game.sim.guard.amount = 1.0,
                }
            }
            Ok("warden") => {
                use end_game_core::warden::WardenPhase;
                game.sim.stage = 3;
                game.sim.position = glam::Vec3::new(-2.9, 0.0, -0.9);
                game.sim.pitch = -0.16;
                if let Ok(distance) = std::env::var("END_GAME_WARDEN_DISTANCE") {
                    if let Ok(distance) = distance.parse::<f32>() {
                        game.sim.position.z = game.sim.warden.y + distance.clamp(1.0, 3.0);
                    }
                }
                let distance = game.sim.position.z - game.sim.warden.y;
                match std::env::var("END_GAME_WARDEN_VIEW").as_deref() {
                    Ok("side") => {
                        game.sim.position.x = game.sim.warden.x + distance;
                        game.sim.position.z = game.sim.warden.y;
                        game.sim.yaw = -std::f32::consts::FRAC_PI_2;
                    }
                    Ok("back") => {
                        game.sim.position.z = game.sim.warden.y - distance;
                        game.sim.yaw = std::f32::consts::PI;
                    }
                    _ => {}
                }
                if let Some(pitch) = std::env::var("END_GAME_WARDEN_PITCH")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|pitch| pitch.is_finite())
                {
                    game.sim.pitch = pitch.clamp(-0.6, 0.1);
                }
                game.sim.warden_ai.phase = match std::env::var("END_GAME_WARDEN_PHASE").as_deref() {
                    Ok("waking") => WardenPhase::Waking,
                    Ok("hunting") => WardenPhase::Hunting,
                    Ok("attacking") => WardenPhase::Attacking,
                    Ok("staggered") => WardenPhase::Staggered,
                    Ok("dead") => WardenPhase::Dead,
                    _ => WardenPhase::Sleeping,
                };
                game.sim.warden_ai.elapsed = std::env::var("END_GAME_ACTION_TIME")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                game.sim.warden_ai.walk_phase = std::env::var("END_GAME_WALK_PHASE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                if game.sim.warden_ai.phase == WardenPhase::Hunting {
                    game.sim.warden_ai.walk_blend = 1.0;
                }
                if let Some(blend) = std::env::var("END_GAME_WALK_BLEND")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|blend| blend.is_finite())
                {
                    game.sim.warden_ai.walk_blend = blend.clamp(0.0, 1.0);
                }
                let interrupted_attack = std::env::var("END_GAME_WARDEN_INTERRUPT_TIME")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|t| t.is_finite())
                    .filter(|_| {
                        matches!(
                            game.sim.warden_ai.phase,
                            WardenPhase::Staggered | WardenPhase::Dead
                        )
                    });
                if let Some(attack_time) = interrupted_attack {
                    let killed = game.sim.warden_ai.phase == WardenPhase::Dead;
                    let elapsed = game.sim.warden_ai.elapsed;
                    game.sim.warden_ai.phase = WardenPhase::Attacking;
                    game.sim.warden_ai.elapsed =
                        attack_time.clamp(0.0, end_game_core::warden::KNIFE_DURATION);
                    game.sim.warden_ai.on_sword_hit(true, killed);
                    game.sim.warden_ai.elapsed = elapsed;
                }
                if game.sim.warden_ai.phase == WardenPhase::Staggered {
                    game.sim.warden_ai.flinch_left = (0.22 - game.sim.warden_ai.elapsed).max(0.0);
                }
                if game.sim.warden_ai.phase == WardenPhase::Dead {
                    game.sim.warden_health = 0.0;
                    if interrupted_attack.is_none() {
                        let elapsed = game.sim.warden_ai.elapsed;
                        game.sim.warden_ai.phase = WardenPhase::Hunting;
                        game.sim.warden_ai.die();
                        game.sim.warden_ai.elapsed = elapsed;
                    }
                }
            }
            Ok("combat") => {
                use end_game_core::combat::{ImpactKind, Strike, StrikeKind, StrikeLink};
                let parse_kind = |name: &str| match name {
                    "cut" => Some(StrikeKind::Cut),
                    "backhand" => Some(StrikeKind::Backhand),
                    "finisher" => Some(StrikeKind::Finisher),
                    "overhead" => Some(StrikeKind::Overhead),
                    "rising" => Some(StrikeKind::Rising),
                    "jump-heavy" => Some(StrikeKind::JumpHeavy),
                    _ => None,
                };
                let kind = std::env::var("END_GAME_STRIKE")
                    .ok()
                    .as_deref()
                    .and_then(parse_kind)
                    .unwrap_or(StrikeKind::Cut);
                let elapsed = std::env::var("END_GAME_ACTION_TIME")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(kind.contact_time())
                    .clamp(0.0, kind.duration());
                game.sim.stage = 4;
                game.sim.position = glam::Vec3::new(-2.9, 0.0, -1.4);
                let mut strike = Strike::new(kind);
                strike.elapsed = elapsed;
                strike.contact_done = elapsed >= kind.contact_time();
                strike.previous = std::env::var("END_GAME_PREVIOUS_STRIKE")
                    .ok()
                    .as_deref()
                    .and_then(parse_kind);
                let link_time = std::env::var("END_GAME_LINK_TIME")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|t| t.is_finite())
                    .unwrap_or(0.1)
                    .max(0.0);
                strike.previous_link_at = link_time.min(strike.previous.unwrap_or(kind).duration());
                strike.link = std::env::var("END_GAME_NEXT_STRIKE")
                    .ok()
                    .as_deref()
                    .and_then(parse_kind)
                    .map(|next| StrikeLink {
                        next,
                        at: link_time.min(kind.duration()),
                        cancelled_at: None,
                    });
                game.sim.combat.active = Some(strike);
                if kind == StrikeKind::JumpHeavy {
                    game.sim.position.y = std::env::var("END_GAME_AIR_HEIGHT")
                        .ok()
                        .and_then(|s| s.parse::<f32>().ok())
                        .filter(|v| v.is_finite())
                        .unwrap_or(0.75)
                        .max(0.0);
                    game.sim.grounded = game.sim.position.y < 0.001;
                    game.sim.combat.active.as_mut().unwrap().landing_wait =
                        std::env::var("END_GAME_LANDING_WAIT").as_deref() == Ok("1");
                }
                if std::env::var("END_GAME_IMPACT").as_deref() == Ok("1") {
                    game.sim.combat.impact_left = if kind.heavy() { 0.32 } else { 0.22 };
                    game.sim.combat.impact_strength = if kind.heavy() { 0.85 } else { 0.45 };
                    game.sim.combat.impact_kind = Some(ImpactKind::Warden);
                    game.sim.combat.impact_strike = Some(kind);
                    game.sim.combat.impact_point = glam::Vec3::new(-2.9, 1.1, -3.7);
                }
            }
            Ok(name @ ("lift-board" | "pickup-key" | "unlock" | "draw-sword")) => {
                use end_game_core::{Interaction, InteractionKind};
                let (kind, stage) = match name {
                    "lift-board" => (InteractionKind::Board, 0),
                    "pickup-key" => (InteractionKind::Key, 1),
                    "unlock" => (InteractionKind::Lock, 2),
                    _ => (InteractionKind::Sword, 3),
                };
                let elapsed = std::env::var("END_GAME_ACTION_TIME")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1.0);
                game.sim.stage = stage + u8::from(elapsed >= kind.commit_time());
                game.sim.position = kind.approach();
                game.sim.interaction = Some(Interaction {
                    kind,
                    elapsed,
                    origin: kind.approach(),
                    committed: elapsed >= kind.commit_time(),
                });
                game.sim.board_open = if kind == InteractionKind::Board {
                    game.sim.interaction.unwrap().manipulate()
                } else {
                    1.0
                };
            }
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
        if std::env::var("END_GAME_SCENE").as_deref() != Ok("sword-hit") {
            game.sim.time = std::env::var("END_GAME_TIME")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            game.sim.gate_open = if game.sim.stage >= 3 { 1.0 } else { 0.0 };
        }
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
            value.attacks = 0;
            value.guard_reset = false;
            result
        });
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.10)
        } else {
            0.0
        };
        let pad = input.pad().unwrap_or_default();
        // Browser keys/pointers use the cancellable held-input bridge. Native
        // input and the standard controller share the same simulation flag.
        let guard_held = (cfg!(not(target_arch = "wasm32"))
            && (input.down(KeyCode::KeyF) || input.mouse_down(MouseButton::Right)))
            || pad.down(PadButton::LT)
            || ui.held & 4 != 0;
        if ui.paused || ui.guard_reset {
            self.guard_release_required = true;
        } else if !guard_held {
            self.guard_release_required = false;
        }
        let block = guard_held && !self.guard_release_required;
        let bits = u32::from(input.down(KeyCode::KeyE) || pad.down(PadButton::West))
            | (u32::from(
                (cfg!(not(target_arch = "wasm32")) && input.mouse_down(MouseButton::Left))
                    || pad.down(PadButton::RB),
            ) << 1)
            | (u32::from(input.down(KeyCode::Space) || pad.down(PadButton::South)) << 2)
            | (u32::from(input.down(KeyCode::AltLeft) || pad.down(PadButton::East)) << 3)
            | (u32::from(input.down(KeyCode::KeyQ) || pad.down(PadButton::North)) << 4)
            | (u32::from(input.down(KeyCode::KeyV) || pad.down(PadButton::R3)) << 5)
            | (u32::from(
                (cfg!(not(target_arch = "wasm32"))
                    && (input.down(KeyCode::KeyR) || input.mouse_down(MouseButton::Middle)))
                    || pad.down(PadButton::RT),
            ) << 6);
        let mut pressed = (bits & !self.previous) | ui.actions;
        let mut attack_presses = ui.attacks.saturating_add(u8::from(pressed & 2 != 0)).min(3);
        if pressed & 64 != 0 {
            // A simultaneous heavy press owns this input batch; never replay
            // counted light taps after the heavy animation has started.
            attack_presses = 0;
        }
        if guard_held {
            // Do not replay a strike pressed during guard after it lowers.
            // Already committed core combo swings continue independently.
            attack_presses = 0;
            pressed &= !64;
        }
        pressed &= !2;
        self.previous = bits;
        if !ui.paused {
            if pressed & 32 != 0 || ui.third_person {
                self.third_person = !self.third_person;
                UI.with(|u| u.borrow_mut().third_person = false);
            }
            pressed &= !32;
            let (dx, dy) = input.mouse_delta();
            if self.sim.interaction.is_none() && self.sim.combat.hitstop_left == 0.0 {
                self.sim.yaw += dx * 0.0021 + pad.right[0] * dt * 2.25 + ui.look.x * 0.003;
                self.sim.pitch = (self.sim.pitch - dy * 0.0021 + pad.right[1] * dt * 1.65
                    - ui.look.y * 0.003)
                    .clamp(-1.2, 1.15);
            } else if self.sim.interaction.is_some() {
                pressed = 0;
                attack_presses = 0;
            }
            self.wake = (self.wake - dt * 0.35).max(0.0);
            self.accumulated += dt;
            let old_event = self.sim.event;
            let old_impact = self.sim.combat.impact_event;
            let old_knife_hit = self.sim.warden_ai.hit_event;
            let old_castle_hit = self
                .sim
                .castle_events
                .events()
                .filter(|e| e.kind == end_game_core::enemies::CastleEventKind::PlayerHit)
                .last()
                .map(|e| e.id);
            let old_block = self.sim.guard.block_event;
            let old_break = self.sim.guard.break_event;
            let old_parry = self.sim.guard.parry_event;
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
                    attack: attack_presses > 0,
                    heavy: pressed & 64 != 0,
                    block,
                    jump: pressed & 4 != 0,
                    dodge: pressed & 8 != 0,
                    transform: pressed & 16 != 0,
                });
                pressed = 0;
                attack_presses = attack_presses.saturating_sub(1);
                self.accumulated -= STEP;
            }
            // Preserve a tap when a high-refresh frame has not reached the next sim tick.
            if pressed != 0 {
                UI.with(|u| u.borrow_mut().actions |= pressed);
            }
            if attack_presses > 0 {
                UI.with(|u| u.borrow_mut().attacks = attack_presses);
            }
            if self.sim.guard.parry_event != old_parry {
                // Sharper and shorter than a break, stronger than a block, so
                // the three read apart through the pad alone.
                self.feedback.rumble(0.55, 0.30, 140);
            } else if self.sim.guard.break_event != old_break {
                self.feedback.rumble(0.85, 0.65, 260);
            } else if self.sim.guard.block_event != old_block {
                self.feedback.rumble(0.22, 0.42, 100);
            } else if self.sim.warden_ai.hit_event != old_knife_hit
                && self.sim.warden_ai.hit_left > 0.0
                || self
                    .sim
                    .castle_events
                    .events()
                    .filter(|e| e.kind == end_game_core::enemies::CastleEventKind::PlayerHit)
                    .last()
                    .map(|e| e.id)
                    .is_some_and(|id| Some(id) != old_castle_hit)
            {
                self.feedback.rumble(0.72, 0.55, 160);
            } else if self.sim.combat.impact_event != old_impact {
                self.feedback
                    .rumble(self.sim.combat.impact_strength.min(1.0), 0.42, 190);
            } else if self.sim.event != old_event {
                self.feedback.rumble(
                    if self.sim.attack_time > 0.0 {
                        0.16
                    } else {
                        0.32
                    },
                    0.25,
                    150,
                );
            }
        } else {
            self.accumulated = 0.0;
            UI.with(|u| u.borrow_mut().held = 0);
        }
        let enemy = self.sim.focus_enemy().map(|enemy| {
            use end_game_core::enemies::{EnemyKind, EnemyPhase};
            let telegraph =
                enemy.phase == EnemyPhase::Attacking && enemy.elapsed < enemy.attack.contact_time();
            let cue = if enemy.can_parry() {
                "Blade up — it can turn a cut aside; strike after the swing"
            } else if telegraph {
                if enemy.attack.guardable() {
                    "Weapon raised — face it and guard, or dodge"
                } else {
                    "CRUSHING SLAM — dodge sideways; guard cannot stop it"
                }
            } else if enemy.phase == EnemyPhase::Attacking {
                "Recovering — strike now"
            } else if enemy.phase_two {
                "Enraged — faster pursuit"
            } else {
                "Watch the weapon. Strike after its attack."
            };
            serde_json::json!({"name": enemy.kind.label(), "health": enemy.health,
                "maxHealth": enemy.max_health, "boss": enemy.kind == EnemyKind::Cyclops,
                "cue": cue, "unblockable": telegraph && !enemy.attack.guardable(),
                "blockCost": if telegraph && enemy.attack.guardable() { Some(enemy.attack.block_cost()) } else { None }})
        });
        let castle_hit = self
            .sim
            .castle_events
            .events()
            .filter(|e| e.kind == end_game_core::enemies::CastleEventKind::PlayerHit)
            .last();
        let castle_attack = self
            .sim
            .castle_events
            .events()
            .filter(|e| e.kind == end_game_core::enemies::CastleEventKind::EnemyAttack)
            .last();
        let lift = self.stair_eye.update(
            self.sim.position.y,
            self.sim.grounded,
            if ui.paused { 0.0 } else { dt },
        );
        let frame =
            self.scene
                .frame_with_camera_lift(&self.sim, self.third_person, self.wake, lift);
        let projection = aim_projection(&frame.camera, self.sim.sword_aim_point());
        let combat_state = serde_json::json!({
            "label": self.sim.combat.label(), "queued": self.sim.combat.queued_count(),
            "rhythmLeft": self.sim.combat.rhythm_left(), "quickLeft": self.sim.combat.quick_left(),
            "swingEvent": self.sim.combat.swing_event,"impactEvent":self.sim.combat.impact_event,
            "impactStrength":self.sim.combat.impact_strength,
            "aimedZone":self.sim.aimed_zone().map(|zone|zone.label()),
            "aimProjection":projection.map(|p|serde_json::json!({"x":p.x,"y":p.y})),
            "impactZone":self.sim.combat.impact_zone.map(|zone|zone.label()),
            "impactSurface":self.sim.combat.impact_surface.map(|surface|format!("{surface:?}")),
            "impactKind":self.sim.combat.impact_kind.map(|kind|format!("{kind:?}")),
            "impactLeft":self.sim.combat.impact_left,
            "active":self.sim.combat.active.map(|s|serde_json::json!({"kind":s.kind.label(),"elapsed":s.elapsed,"windup":s.kind.windup_time(),"duration":s.kind.duration(),"landingWait":s.landing_wait && s.elapsed >= s.kind.follow_end()}))
        });
        let state = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"), "stage": self.sim.stage, "objective": self.sim.objective(),
            "location": self.sim.location(),
            "exploration": { "visited": self.sim.explored_count(), "total": end_game_core::TOTAL_EXPLORE_COUNT },
            "prompt": self.sim.prompt(), "health": self.sim.health, "stamina": self.sim.stamina,
            "form": if self.sim.werewolf { "Werewolf" } else { "Wolf" }, "alert": self.sim.alert,
            "event": self.sim.event, "message": self.sim.message, "footsteps": self.sim.footsteps,
            "crouched": self.sim.crouched, "pad": input.pad().is_some(), "transformation": self.sim.transformation,
            "position": self.sim.position.to_array(), "time": self.sim.time,
            "interacting": self.sim.interaction.is_some(),
            "finished": self.sim.finished(),
            "enemy": enemy,
            "enemyCombat": {"hitEvent":castle_hit.map_or(0,|e| e.id),
                "attackEvent":castle_attack.map_or(0,|e| e.id),
                "hitLeft":castle_hit.map_or(0.0,|e| (0.24 - (self.sim.time-e.time)).clamp(0.0,0.24))},
            "quest": {"sequence":self.sim.quest.sequence,"seal":self.sim.quest.seal,
                "journal":self.sim.quest.journal(),"shrinesUsed":self.sim.quest.shrines_used.count_ones(),
                "checkpoint":self.sim.quest.checkpoint,"defeated":self.sim.defeated_count()},
            "castleEvents":self.sim.castle_events.events().map(|e| serde_json::json!({
                "id":e.id,"kind":e.kind.key(),"time":e.time
            })).collect::<Vec<_>>(),
            "guard": {
                "amount": self.sim.guard.amount, "ready": self.sim.guard.ready(),
                "brokenLeft": self.sim.guard.broken_left, "impactLeft": self.sim.guard.impact_left,
                "blockEvent": self.sim.guard.block_event, "breakEvent": self.sim.guard.break_event,
                "parryWindowLeft": self.sim.guard.parry_window_left,
                "parryCooldown": self.sim.guard.parry_cooldown,
                "parryEvent": self.sim.guard.parry_event,
                "parryWindow": end_game_core::guard::PARRY_WINDOW,
                "parryCost": end_game_core::guard::PARRY_STAMINA_COST
            },
            "dialogue": {
                "life": self.sim.dialogue.life, "sequence": self.sim.dialogue.sequence,
                "dead": self.sim.dialogue.dead(),
                "events": self.sim.dialogue.events().map(|event| serde_json::json!({
                    "id": event.id, "kind": event.kind.key(), "time": event.time
                })).collect::<Vec<_>>()
            },
            "warden": {
                "phase": format!("{:?}", self.sim.warden_ai.phase),
                "label": self.sim.warden_ai.label(), "health": self.sim.warden_health,
                "elapsed": self.sim.warden_ai.elapsed,
                "near": self.sim.distance(self.sim.warden.x, self.sim.warden.y) < 7.0,
                "attackEvent": self.sim.warden_ai.attack_event,
                "hitEvent": self.sim.warden_ai.hit_event, "hitLeft": self.sim.warden_ai.hit_left,
                "windup": end_game_core::warden::KNIFE_WINDUP,
                "contact": end_game_core::warden::KNIFE_CONTACT
            },
            "combat": combat_state,
            "physics": { "gravity": end_game_core::GRAVITY, "crateMass": self.sim.body.mass(), "crateWear": self.sim.body.wear }
        });
        HUD.with(|hud| *hud.borrow_mut() = state.to_string());
        frame
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
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.actions |= mask & (127 ^ 2);
            if mask & 2 != 0 {
                u.attacks = u.attacks.saturating_add(1).min(3);
            }
        });
    }
    #[wasm_bindgen]
    pub fn pause(paused: bool) {
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.paused = paused;
            u.guard_reset |= paused;
            u.movement = Vec2::ZERO;
            u.look = Vec2::ZERO;
            u.actions = 0;
            u.attacks = 0;
            u.held = 0;
        });
    }
}
