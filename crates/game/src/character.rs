//! Blocky humanoid player character: six box instances (head, torso, two
//! arms, two legs) with a simple walk-swing animation. Mesh ids are
//! registered by the caller as head, torso, limb — in that order.

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{Frame, Instance};

/// Number of meshes the character registers (head, torso, limb).
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

/// Articulated part-character: shared engine implementation (also used by
/// the web arena build).
pub use ember_engine::puppet::{MeshPart, PartCharacter};

/// Jointed rig veteran: shared engine implementation (also used by the web
/// arena build).
pub use ember_engine::rig::{veteran_rig, PartSource, RigCharacter};

/// A full-body character mesh (AI-generated GLB), bounds-normalized at
/// registration so any authoring scale/origin stands feet-on-ground.
pub struct MeshCharacter {
    pub first_mesh: u32,
    pub parts: u32,
    /// Lowest vertex Y in mesh space (the feet).
    pub feet_y: f32,
    /// Mesh-space height (max Y - min Y).
    pub height: f32,
    /// Yaw added so the mesh's authored facing aligns with +facing.
    pub yaw_offset: f32,
}

/// Push the mesh character: scaled to ~1.8 units tall, feet on the ground,
/// with a walk bob and a subtle side sway (rigid mesh — no limb animation).
pub fn push_character_mesh(
    frame: &mut Frame,
    pos: Vec2,
    facing_yaw: f32,
    color: [f32; 3],
    is_me: bool,
    walk_phase: f32,
    m: &MeshCharacter,
) {
    let scale = (if is_me { 1.1 } else { 1.0 }) * (1.8 / m.height.max(0.01));
    let bob = walk_phase.sin().abs() * 0.05;
    let sway = rotate(Vec2::new(walk_phase.sin() * 0.03, 0.0), facing_yaw);
    // Engine transform is scale-then-translate: lift by -feet_y * scale so
    // the lowest vertex lands on y = 0, plus the bob.
    let y = -m.feet_y * scale + bob;
    let world = Vec3::new(pos.x + sway.x, y, pos.y + sway.y);
    for p in 0..m.parts {
        frame.instances.push(
            Instance::new(world, Vec3::splat(scale), Vec3::from_array(color))
                .with_yaw(facing_yaw + m.yaw_offset)
                .with_mesh(m.first_mesh + p),
        );
    }
}
