//! Shared articulated "puppet" character: five AI-generated mesh parts
//! (head, torso, arm, leg, boot — arm/leg/boot reused for both sides)
//! animated with opposing limb swings, step lift, walk bob, an eased swing
//! amplitude, and pivot-correct placement. Used by the native arena client
//! and the web arena build.

use glam::{Vec2, Vec3};

use crate::renderer::{Frame, Instance, MeshData};

/// One bounds-normalized mesh part.
pub struct MeshPart {
    pub mesh: u32,
    /// Uniform scale from mesh units to world (target height / mesh height).
    pub scale: f32,
    /// Mesh-space bounds center; parts pivot around their middle.
    pub center: [f32; 3],
}

/// The five-part character.
pub struct PartCharacter {
    pub head: MeshPart,
    pub torso: MeshPart,
    pub arm: MeshPart,
    pub leg: MeshPart,
    pub boot: MeshPart,
}

/// (file stem, target world height) for the standard part set.
pub const PART_SPECS: [(&str, f32); 5] = [
    ("part-head", 0.34),
    ("part-torso", 0.68),
    ("part-arm", 0.66),
    ("part-leg", 0.55),
    ("part-boot", 0.19),
];

/// Build one part from GLB bytes: merge its meshes, compute bounds, return
/// the mesh plus its normalized placement info for `mesh_id`.
///
/// # Errors
///
/// Returns an error when the GLB cannot be loaded or the merged mesh has no
/// usable vertical extent.
pub fn part_from_glb_bytes(
    bytes: &[u8],
    target_height: f32,
    mesh_id: u32,
) -> Result<(MeshData, MeshPart), String> {
    let parts = crate::assets::load_glb(bytes)?;
    let mut merged = MeshData::default();
    for p in parts {
        merged.vertices.extend(p.mesh.vertices);
    }
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &merged.vertices {
        for a in 0..3 {
            min[a] = min[a].min(v.pos[a]);
            max[a] = max[a].max(v.pos[a]);
        }
    }
    let h = max[1] - min[1];
    if h <= 1e-3 {
        return Err("part has no vertical extent".into());
    }
    let info = MeshPart {
        mesh: mesh_id,
        scale: target_height / h,
        center: [
            f32::midpoint(min[0], max[0]),
            f32::midpoint(min[1], max[1]),
            f32::midpoint(min[2], max[2]),
        ],
    };
    Ok((merged, info))
}

/// Rotate a local XZ offset by yaw, matching the scene shader's convention
/// (x' = x*c + z*s, z' = -x*s + z*c).
#[must_use]
pub fn rotate(local: Vec2, yaw: f32) -> Vec2 {
    let (s, c) = yaw.sin_cos();
    Vec2::new(
        local.y.mul_add(s, local.x * c),
        local.y.mul_add(c, -local.x * s),
    )
}

fn push_part(
    frame: &mut Frame,
    part: &MeshPart,
    body_scale: f32,
    target: Vec3,
    yaw: f32,
    col: Vec3,
) {
    let s = part.scale * body_scale;
    let c = part.center;
    let cr = rotate(Vec2::new(c[0] * s, c[2] * s), yaw);
    let i_pos = Vec3::new(target.x - cr.x, c[1].mul_add(-s, target.y), target.z - cr.y);
    frame.instances.push(
        Instance::new(i_pos, Vec3::splat(s), col)
            .with_yaw(yaw)
            .with_mesh(part.mesh),
    );
}

/// Push the articulated character. `amp` (0..1) eases the limb swing in and
/// out; `crouch` lowers the whole figure.
// A character placement is defined by these independent pose and mesh inputs.
#[allow(clippy::too_many_arguments)]
pub fn push_character_parts(
    frame: &mut Frame,
    pos: Vec2,
    facing_yaw: f32,
    color: [f32; 3],
    scale_mult: f32,
    walk_phase: f32,
    amp: f32,
    crouch: bool,
    ch: &PartCharacter,
) {
    let body = scale_mult * if crouch { 0.82 } else { 1.0 };
    let col = Vec3::from_array(color);
    // Parts are front-view renders: add PI so their faces align with facing.
    let facing_yaw = facing_yaw + std::f32::consts::PI;
    let swing = walk_phase.sin() * amp;
    let bob = walk_phase.sin().abs() * 0.035 * amp;
    let lift_l = swing.max(0.0) * 0.06;
    let lift_r = (-swing).max(0.0) * 0.06;

    let placements: [(&MeshPart, f32, f32, f32, f32); 8] = [
        (&ch.boot, -0.13, swing * 0.20, 0.09, lift_l),
        (&ch.boot, 0.13, -swing * 0.20, 0.09, lift_r),
        (&ch.leg, -0.13, swing * 0.16, 0.455, lift_l * 0.5),
        (&ch.leg, 0.13, -swing * 0.16, 0.455, lift_r * 0.5),
        (&ch.arm, -0.36, -swing * 0.14, 1.055, 0.0),
        (&ch.arm, 0.36, swing * 0.14, 1.055, 0.0),
        (&ch.torso, 0.0, 0.0, 1.055, bob),
        (&ch.head, 0.0, 0.0, 1.55, bob),
    ];
    for (part, lx, lz, cy, extra_y) in placements {
        let w = rotate(Vec2::new(lx * body, lz * body), facing_yaw);
        let target = Vec3::new(
            pos.x + w.x,
            bob.mul_add(0.3, f32::mul_add(cy, body, extra_y)),
            pos.y + w.y,
        );
        push_part(frame, part, body, target, facing_yaw, col);
    }
}

/// Animation slot: (`facing_yaw`, `walk_phase`, `swing_amplitude`). Advance from a
/// velocity with shortest-arc yaw smoothing and an eased amplitude.
pub fn advance_anim(slot: &mut (f32, f32, f32), vel: Vec2, dt: f32) {
    let speed = vel.length();
    let moving = speed > 0.05;
    if moving {
        let target = vel.x.atan2(vel.y);
        let mut diff = target - slot.0;
        while diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        }
        while diff < -std::f32::consts::PI {
            diff += std::f32::consts::TAU;
        }
        slot.0 = diff.mul_add(1.0 - (-12.0 * dt).exp(), slot.0);
        slot.1 = (speed * dt).mul_add(6.0, slot.1);
    }
    let amp_target = if moving { 1.0 } else { 0.0 };
    slot.2 = (amp_target - slot.2).mul_add(1.0 - (-9.0 * dt).exp(), slot.2);
}
