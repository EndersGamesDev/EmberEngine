//! Blocky humanoid player character: six box instances (head, torso, two
//! arms, two legs) with a simple walk-swing animation. Mesh ids are
//! registered by the caller as head, torso, limb — in that order.

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{Frame, Instance};

/// Number of meshes the character registers (head, torso, limb).
#[allow(dead_code)]
pub const PART_MESHES: u32 = 3;

/// Rotate a local XZ offset by the character's facing yaw, matching the
/// engine shader's yaw convention (x' = x*c + z*s, z' = -x*s + z*c).
fn rotate(local: Vec2, yaw: f32) -> Vec2 {
    let (s, c) = yaw.sin_cos();
    Vec2::new(local.x * c + local.y * s, -local.x * s + local.y * c)
}

/// Push one humanoid at `pos` (XZ, feet on the ground at y = 0).
/// `walk_phase` advances while moving (radians); 0 = idle stance.
pub fn push_character(
    frame: &mut Frame,
    pos: Vec2,
    facing_yaw: f32,
    color: [f32; 3],
    is_me: bool,
    walk_phase: f32,
    first_mesh: u32,
) {
    let s = if is_me { 1.1 } else { 1.0 };
    let col = Vec3::from_array(color);
    let swing = walk_phase.sin();

    // (mesh offset, local x, local z, center y, scale) per part.
    // Legs: side by side (x), swinging fore/aft (z) in opposition.
    // Arms: outside the torso, swinging opposite their same-side leg.
    let leg_scale = Vec3::new(0.20, 0.75, 0.20) * s;
    let torso_scale = Vec3::new(0.55, 0.65, 0.30) * s;
    let head_scale = Vec3::splat(0.32) * s;
    let arm_scale = Vec3::new(0.16, 0.65, 0.16) * s;

    let parts: [(u32, Vec2, f32, Vec3); 6] = [
        // legs (limb mesh)
        (2, Vec2::new(-0.13, swing * 0.18), 0.375, leg_scale),
        (2, Vec2::new(0.13, -swing * 0.18), 0.375, leg_scale),
        // arms (limb mesh), opposite swing to the same-side leg
        (2, Vec2::new(-0.36, -swing * 0.14), 1.075, arm_scale),
        (2, Vec2::new(0.36, swing * 0.14), 1.075, arm_scale),
        // torso
        (1, Vec2::ZERO, 1.075, torso_scale),
        // head
        (0, Vec2::ZERO, 1.57, head_scale),
    ];

    for (mesh_off, local, y, scale) in parts {
        let world_xz = pos + rotate(local * s, facing_yaw);
        frame.instances.push(
            Instance::new(Vec3::new(world_xz.x, y * s, world_xz.y), scale, col)
                .with_yaw(facing_yaw)
                .with_mesh(first_mesh + mesh_off),
        );
    }
}

/// Walk-phase advance for a movement speed over `dt` seconds.
pub fn walk_speed_to_phase_delta(speed: f32, dt: f32) -> f32 {
    speed * dt * 6.0
}
