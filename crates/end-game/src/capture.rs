//! Passive native fixtures. These run only inside an EMBER_CAPTURE_PATH session.
use crate::Game;
use end_game_core::{Controls, Enemy, EnemyKind, Strike, StrikeKind};
use glam::{Quat, Vec3};

fn value(key: &str, fallback: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(fallback)
}

pub fn sword_hit(game: &mut Game) {
    let mode = std::env::var("END_GAME_HIT_TARGET").unwrap_or_else(|_| "enemy".into());
    let kind = match std::env::var("END_GAME_STRIKE").as_deref() {
        Ok("backhand") => StrikeKind::Backhand,
        Ok("finisher") => StrikeKind::Finisher,
        Ok("overhead") => StrikeKind::Overhead,
        Ok("rising") => StrikeKind::Rising,
        Ok("jump-heavy") => StrikeKind::JumpHeavy,
        _ => StrikeKind::Cut,
    };
    game.sim.stage = 5;
    game.sim.exit_open = 1.0;
    game.sim.gate_open = 1.0;
    game.sim.warden_health = 0.0;
    game.sim.warden_ai.die();
    game.sim.warden_ai.elapsed = 2.0;
    game.sim.enemies.clear();
    game.sim.crouched = std::env::var("END_GAME_CROUCHED").as_deref() == Ok("1");
    game.third_person = std::env::var("END_GAME_THIRD_PERSON").as_deref() == Ok("1");
    match mode.as_str() {
        "wall" => {
            game.sim.position = Vec3::new(12.35, 0.0, -35.0);
            game.sim.yaw = std::f32::consts::FRAC_PI_2;
            game.sim.pitch = 0.0;
        }
        "floor" => {
            game.sim.position = Vec3::new(0.0, 0.0, 2.0);
            game.sim.yaw = 0.0;
            game.sim.pitch = -0.8;
        }
        "iron" => {
            game.sim.position = Vec3::new(0.0, 0.0, -1.6);
            game.sim.yaw = std::f32::consts::PI;
            game.sim.pitch = 0.0;
            game.sim.gate_open = 0.0;
        }
        _ => {
            let body = match std::env::var("END_GAME_ENEMY_KIND").as_deref() {
                Ok("spear") => EnemyKind::SpearSoldier,
                Ok("hollow") => EnemyKind::HollowAxeKnight,
                Ok("cyclops") => EnemyKind::Cyclops,
                _ => EnemyKind::SwordSoldier,
            };
            let mut enemy = Enemy::new(0, body, Vec3::new(0.0, 0.0, -35.0));
            enemy.yaw = std::f32::consts::PI;
            enemy.cooldown = 999.0;
            let h = body.height();
            let local = match std::env::var("END_GAME_TARGET_ZONE").as_deref() {
                Ok("left-torso") => Vec3::new(-0.10, 0.63, -0.10),
                Ok("right-torso") => Vec3::new(0.10, 0.63, -0.10),
                Ok("left-leg") => Vec3::new(-0.075, 0.22, -0.08),
                Ok("right-leg") => Vec3::new(0.075, 0.22, -0.08),
                _ => Vec3::new(0.0, 0.92, -0.10),
            } * h;
            let target = enemy.position + Quat::from_rotation_y(-enemy.yaw) * local;
            game.sim.position = enemy.position + Vec3::Z * value("END_GAME_ENEMY_DISTANCE", 2.15);
            let eye = game.sim.position + Vec3::Y * if game.sim.crouched { 1.0 } else { 1.65 };
            let mut start = eye - Vec3::Y * 0.25;
            for _ in 0..4 {
                let d = (target - start).normalize();
                game.sim.yaw = d.x.atan2(-d.z);
                game.sim.pitch = d.y.asin();
                let view =
                    Quat::from_rotation_y(-game.sim.yaw) * Quat::from_rotation_x(game.sim.pitch);
                start = eye + view * Vec3::new(0.0, -0.25, -0.47);
            }
            game.sim.enemies.push(enemy);
        }
    }
    game.sim.position = Vec3::new(
        value("END_GAME_X", game.sim.position.x),
        value("END_GAME_Y", game.sim.position.y),
        value("END_GAME_Z", game.sim.position.z),
    );
    game.sim.yaw = value("END_GAME_YAW", game.sim.yaw);
    game.sim.pitch = value("END_GAME_PITCH", game.sim.pitch).clamp(-1.2, 1.15);
    game.sim.combat.active = Some(Strike::new(kind));
    if kind == StrikeKind::JumpHeavy {
        game.sim.position.y += value("END_GAME_AIR_HEIGHT", 0.75).max(0.0);
        game.sim.grounded = false;
        game.sim.velocity_y = 0.0;
    }
    let aimed = game.sim.aimed_zone();
    let mut hit = false;
    for _ in 0..180 {
        game.sim.tick(Controls {
            crouch: game.sim.crouched,
            ..Controls::default()
        });
        if game.sim.combat.impact_event > 0 {
            hit = true;
            break;
        }
        if game.sim.combat.active.is_none() {
            break;
        }
    }
    let frames = value("END_GAME_AFTER_HIT_FRAMES", 12.0).clamp(0.0, 120.0) as usize;
    if hit {
        // Register the armor contact in its actual struck pose before advancing
        // the passive fixture to the requested reaction/recovery frame.
        let _ = game
            .scene
            .frame_with_camera_lift(&game.sim, game.third_person, 0.0, 0.0);
        for _ in 0..frames {
            game.sim.tick(Controls {
                crouch: game.sim.crouched,
                ..Controls::default()
            });
        }
    }
    eprintln!(
        "V11 passive contact: {}",
        serde_json::json!({
            "hit":hit,"strike":kind.label(),"aimed":aimed.map(|z|z.label()),
            "zone":game.sim.combat.impact_zone.map(|z|z.label()),
            "kind":format!("{:?}",game.sim.combat.impact_kind),
            "surface":format!("{:?}",game.sim.combat.impact_surface),
            "point":game.sim.combat.impact_point.to_array(),
            "marks":game.sim.surface_impacts.impacts().count(),
            "position":game.sim.position.to_array(),"time":game.sim.time,
            "health":game.sim.enemies.first().map(|e|e.health),
            "reaction":game.sim.enemies.first().and_then(|e|e.reaction).map(|r|r.zone.label()),
            "yaw":game.sim.yaw,"pitch":game.sim.pitch,
        })
    );
    if std::env::var("END_GAME_VIEW_MARK").as_deref() == Ok("1") {
        if let Some(hit) = game.sim.surface_impacts.impacts().last().copied() {
            let eye = game.sim.position + Vec3::Y * if game.sim.crouched { 1.0 } else { 1.65 };
            let d = (hit.point - eye).normalize();
            game.sim.yaw = d.x.atan2(-d.z);
            game.sim.pitch = d.y.asin().clamp(-1.2, 1.15);
        }
    }
}
