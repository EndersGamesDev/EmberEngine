//! Blocky humanoid player character: six box instances (head, torso, two
//! arms, two legs) with a simple walk-swing animation. Mesh ids are
//! registered by the caller as head, torso, limb — in that order.

use ember_engine::glam::{Quat, Vec2, Vec3};
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

/// Articulated part-character: shared engine implementation (also used by
/// the web arena build).
pub use ember_engine::puppet::{MeshPart, PartCharacter};

use ember_engine::rig::{self, HumanoidDims, RigPart, Skeleton};

/// Bounds + mesh id of one loaded part GLB, for rig assembly.
#[derive(Clone, Copy)]
pub struct PartSource {
    pub mesh: u32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl PartSource {
    fn height(&self) -> f32 {
        (self.max[1] - self.min[1]).max(1e-3)
    }

    /// Mesh-space point at the given fraction of the bounds per axis.
    fn anchor(&self, fx: f32, fy: f32, fz: f32) -> Vec3 {
        Vec3::new(
            self.min[0] + (self.max[0] - self.min[0]) * fx,
            self.min[1] + (self.max[1] - self.min[1]) * fy,
            self.min[2] + (self.max[2] - self.min[2]) * fz,
        )
    }
}

/// The jointed veteran (v2): a skeleton plus its bound mesh parts.
pub struct RigCharacter {
    pub skel: Skeleton,
    pub dims: HumanoidDims,
    pub parts: Vec<RigPart>,
}

/// Assemble the jointed veteran from the available part meshes. With the v1
/// asset set, `arm` doubles as upper arm + forearm and `leg` as thigh +
/// shin; the dedicated v2 segment meshes replace them file-by-file later.
pub fn veteran_rig(
    head: PartSource,
    torso: PartSource,
    arm: PartSource,
    leg: PartSource,
    boot: PartSource,
    helmet: Option<PartSource>,
) -> RigCharacter {
    use rig::joint;
    let dims = HumanoidDims::default();
    let skel = rig::humanoid(&dims);
    // Part concepts are authored facing the camera; half a turn aligns them
    // with the rig's +Z forward.
    let flip = Quat::from_rotation_y(std::f32::consts::PI);
    // Veteran palette (untextured meshes shade from these).
    let olive = Vec3::new(0.32, 0.34, 0.22);
    let trouser = Vec3::new(0.29, 0.31, 0.21);
    let tan = Vec3::new(0.48, 0.40, 0.27);
    let skin = Vec3::new(0.70, 0.55, 0.44);
    let leather = Vec3::new(0.40, 0.31, 0.20);
    let mut parts = Vec::new();
    let mut add = |src: PartSource, joint: usize, anchor: Vec3, offset: Vec3, target_h: f32, mirror_x: bool, tint: Vec3| {
        parts.push(RigPart {
            mesh: src.mesh,
            joint,
            anchor,
            offset,
            scale: target_h / src.height(),
            pre_rot: flip,
            mirror_x,
            tint: Some(tint),
        });
    };
    // Torso hangs low enough to cover the hips until a pelvis part exists.
    add(torso, joint::SPINE, torso.anchor(0.5, 0.0, 0.5), Vec3::new(0.0, -0.16, 0.0), 0.70, false, tan);
    add(head, joint::NECK, head.anchor(0.5, 0.0, 0.5), Vec3::new(0.0, 0.01, 0.0), 0.30, false, skin);
    if let Some(hm) = helmet {
        add(hm, joint::NECK, hm.anchor(0.5, 0.0, 0.5), Vec3::new(0.0, 0.10, 0.0), 0.26, false, olive);
    }
    for (mirror_x, sh, el, hip, knee, ankle) in [
        (false, joint::SHOULDER_L, joint::ELBOW_L, joint::HIP_L, joint::KNEE_L, joint::ANKLE_L),
        (true, joint::SHOULDER_R, joint::ELBOW_R, joint::HIP_R, joint::KNEE_R, joint::ANKLE_R),
    ] {
        let top = arm.anchor(0.5, 1.0, 0.5);
        add(arm, sh, top, Vec3::ZERO, dims.upperarm_len + 0.05, mirror_x, olive);
        add(arm, el, top, Vec3::ZERO, dims.forearm_len + 0.12, mirror_x, skin);
        let leg_top = leg.anchor(0.5, 1.0, 0.5);
        add(leg, hip, leg_top, Vec3::ZERO, dims.thigh_len + 0.05, mirror_x, trouser);
        add(leg, knee, leg_top, Vec3::ZERO, dims.shin_len + 0.04, mirror_x, trouser);
        // Boot pivots above the heel, a third of the way along the foot.
        add(boot, ankle, boot.anchor(0.5, 1.0, 0.38), Vec3::ZERO, dims.ankle_h + 0.10, mirror_x, leather);
    }
    RigCharacter { skel, dims, parts }
}

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
