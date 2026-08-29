//! Jointed forward-kinematics rig for multi-part characters.
//!
//! Where `puppet` fakes limb motion with sliding XZ offsets, this module
//! gives each body part a joint in a parent-child tree and animates joint
//! rotations, so elbows and knees actually bend. Parts are AI-generated
//! GLB segments anchored to their joint pivot (a thigh hangs from the hip,
//! a forearm from the elbow); one mesh serves both sides via X-mirroring.
//!
//! Character space: +Y up, faces +Z, "L" limbs on -X. `push_rig` turns a
//! posed skeleton into engine instances (scale, then quat rotation, then
//! translation — matching the scene shader).

use glam::{Quat, Vec2, Vec3};

use crate::renderer::{Frame, Instance, MeshData};

/// Joint indices. Parents come before children (FK evaluates in order).
pub mod joint {
    pub const ROOT: usize = 0; // pelvis
    pub const SPINE: usize = 1; // torso base
    pub const NECK: usize = 2; // head base
    pub const SHOULDER_L: usize = 3;
    pub const ELBOW_L: usize = 4;
    pub const WRIST_L: usize = 5;
    pub const SHOULDER_R: usize = 6;
    pub const ELBOW_R: usize = 7;
    pub const WRIST_R: usize = 8;
    pub const HIP_L: usize = 9;
    pub const KNEE_L: usize = 10;
    pub const ANKLE_L: usize = 11;
    pub const HIP_R: usize = 12;
    pub const KNEE_R: usize = 13;
    pub const ANKLE_R: usize = 14;
    pub const COUNT: usize = 15;
}

/// One joint: bind-pose pivot offset from the parent's pivot.
#[derive(Clone, Copy)]
pub struct JointDef {
    pub parent: Option<usize>,
    pub offset: Vec3,
}

pub struct Skeleton {
    pub joints: [JointDef; joint::COUNT],
}

/// Segment lengths of the humanoid skeleton, in character units
/// (a ~1.8-tall figure; `push_rig`'s scale_mult resizes the whole rig).
#[derive(Clone, Copy)]
pub struct HumanoidDims {
    /// Ground to hip pivot. Must equal thigh_len + shin_len + ankle_h.
    pub pelvis_h: f32,
    pub hip_w: f32,
    pub thigh_len: f32,
    pub shin_len: f32,
    /// Ankle pivot to sole.
    pub ankle_h: f32,
    /// Pelvis to shoulder line.
    pub spine_len: f32,
    pub shoulder_w: f32,
    pub upperarm_len: f32,
    pub forearm_len: f32,
    /// Shoulder line to head base.
    pub neck_len: f32,
}

impl Default for HumanoidDims {
    fn default() -> Self {
        Self {
            pelvis_h: 0.98,
            hip_w: 0.22,
            thigh_len: 0.44,
            shin_len: 0.43,
            ankle_h: 0.11,
            spine_len: 0.52,
            shoulder_w: 0.44,
            upperarm_len: 0.31,
            forearm_len: 0.28,
            neck_len: 0.13,
        }
    }
}

/// Build the standard humanoid joint tree from segment dimensions.
pub fn humanoid(d: &HumanoidDims) -> Skeleton {
    use joint::*;
    let mut joints = [JointDef { parent: None, offset: Vec3::ZERO }; COUNT];
    let mut set = |j: usize, parent: Option<usize>, offset: Vec3| {
        joints[j] = JointDef { parent, offset };
    };
    set(ROOT, None, Vec3::ZERO);
    set(SPINE, Some(ROOT), Vec3::new(0.0, 0.05, 0.0));
    set(NECK, Some(SPINE), Vec3::new(0.0, d.spine_len, 0.0));
    for (side, sh, el, wr, hip, knee, ankle) in [
        (-1.0f32, SHOULDER_L, ELBOW_L, WRIST_L, HIP_L, KNEE_L, ANKLE_L),
        (1.0, SHOULDER_R, ELBOW_R, WRIST_R, HIP_R, KNEE_R, ANKLE_R),
    ] {
        set(sh, Some(SPINE), Vec3::new(side * d.shoulder_w * 0.5, d.spine_len - 0.03, 0.0));
        set(el, Some(sh), Vec3::new(0.0, -d.upperarm_len, 0.0));
        set(wr, Some(el), Vec3::new(0.0, -d.forearm_len, 0.0));
        set(hip, Some(ROOT), Vec3::new(side * d.hip_w * 0.5, 0.0, 0.0));
        set(knee, Some(hip), Vec3::new(0.0, -d.thigh_len, 0.0));
        set(ankle, Some(knee), Vec3::new(0.0, -d.shin_len, 0.0));
    }
    Skeleton { joints }
}

/// A posed skeleton: per-joint local rotation plus the root position
/// (character space; y = pelvis height after crouch and bob).
pub struct Pose {
    pub local_rot: [Quat; joint::COUNT],
    pub root_pos: Vec3,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            local_rot: [Quat::IDENTITY; joint::COUNT],
            root_pos: Vec3::new(0.0, HumanoidDims::default().pelvis_h, 0.0),
        }
    }
}

/// Evaluate world (character-space) transforms for every joint.
pub fn world_joints(skel: &Skeleton, pose: &Pose) -> [(Vec3, Quat); joint::COUNT] {
    let mut out = [(Vec3::ZERO, Quat::IDENTITY); joint::COUNT];
    for (i, j) in skel.joints.iter().enumerate() {
        let (pp, pr) = match j.parent {
            Some(p) => out[p],
            None => (pose.root_pos, Quat::IDENTITY),
        };
        out[i] = (pp + pr * j.offset, (pr * pose.local_rot[i]).normalize());
    }
    out
}

/// Procedural locomotion pose.
///
/// `phase` advances with distance walked (radians), `amp` eases the gait in
/// and out (0 = standing), `crouch` (0..1) sinks into a knees-bent stance,
/// and `idle_t` is wall-clock time driving the idle breathing sway.
pub fn walk_pose(phase: f32, amp: f32, crouch: f32, idle_t: f32, d: &HumanoidDims) -> Pose {
    use joint::*;
    let pitch = Quat::from_rotation_x;
    let swing = phase.sin() * amp;
    let mut p = Pose::default();

    // Legs. Rotating a hanging (-Y) limb by a negative X angle swings it
    // toward +Z (forward). Knees flex (positive pitch) as the leg passes
    // under the body, straighten at the stride extremes.
    let hip_amp = 0.55;
    for (side_phase, hip, knee, ankle) in
        [(0.0f32, HIP_L, KNEE_L, ANKLE_L), (std::f32::consts::PI, HIP_R, KNEE_R, ANKLE_R)]
    {
        let s = (phase + side_phase).sin() * amp;
        let flex = (phase + side_phase).cos().max(0.0) * amp;
        let hip_a = -hip_amp * s - 1.0 * crouch;
        let knee_a = 0.10 * amp + 0.70 * flex + 1.35 * crouch;
        p.local_rot[hip] = pitch(hip_a);
        p.local_rot[knee] = pitch(knee_a);
        // Keep the boot roughly level with the ground.
        p.local_rot[ankle] = pitch(-(hip_a + knee_a) * 0.85);
    }

    // Torso: slight forward lean while moving or crouched, a breathing sway
    // when idle, and a counter-twist against the hips.
    let breathe = (idle_t * 1.7).sin() * 0.02 * (1.0 - amp);
    let spine_pitch = 0.08 * amp + 0.38 * crouch + breathe;
    p.local_rot[SPINE] = Quat::from_rotation_y(0.12 * swing) * pitch(spine_pitch);
    p.local_rot[NECK] = pitch(-spine_pitch * 0.8);

    // Arms swing opposite their same-side leg; elbows keep a soft bend that
    // deepens as the arm travels back.
    for (sign, sh, el) in [(1.0f32, SHOULDER_L, ELBOW_L), (-1.0, SHOULDER_R, ELBOW_R)] {
        let s = sign * swing;
        p.local_rot[sh] = pitch(0.45 * s);
        p.local_rot[el] = pitch(-(0.30 + 0.30 * s.max(0.0) + 0.25 * crouch));
    }

    // Root: crouch sink, plus the walk's vaulting bob (highest mid-stride).
    let sink = crouch * (d.thigh_len + d.shin_len) * 0.36;
    let bob = phase.cos().abs() * 0.035 * amp;
    p.root_pos = Vec3::new(0.0, d.pelvis_h - sink + bob, 0.0);
    p
}

/// One renderable part bound to a joint.
#[derive(Clone, Copy)]
pub struct RigPart {
    pub mesh: u32,
    pub joint: usize,
    /// Mesh-space point that lands on the joint pivot (plus `offset`).
    pub anchor: Vec3,
    /// Placement offset from the joint pivot, in joint-local space.
    pub offset: Vec3,
    /// Mesh-unit to character-unit scale.
    pub scale: f32,
    /// Fixed rotation applied inside the joint (e.g. PI yaw for meshes
    /// authored facing the camera).
    pub pre_rot: Quat,
    /// Reuse this mesh for the other side by mirroring across X.
    pub mirror_x: bool,
    /// Material tint multiplied with the character color (None = untinted).
    /// Untextured AI part meshes get their look from this.
    pub tint: Option<Vec3>,
}

/// Push a posed rig into the frame. `pos` is world XZ with feet at y = 0;
/// `facing_yaw` matches the engine's yaw convention.
#[allow(clippy::too_many_arguments)]
pub fn push_rig(
    frame: &mut Frame,
    parts: &[RigPart],
    skel: &Skeleton,
    pose: &Pose,
    pos: Vec2,
    facing_yaw: f32,
    color: [f32; 3],
    scale_mult: f32,
) {
    let joints = world_joints(skel, pose);
    let face = Quat::from_rotation_y(facing_yaw);
    let origin = Vec3::new(pos.x, 0.0, pos.y);
    let col = Vec3::from_array(color);
    for part in parts {
        let (jp, jr) = joints[part.joint];
        let mut sv = Vec3::splat(part.scale);
        if part.mirror_x {
            sv.x = -sv.x;
        }
        let r_local = jr * part.pre_rot;
        let p_local = jp + jr * part.offset - r_local * (part.anchor * sv);
        let part_col = match part.tint {
            Some(t) => t * col,
            None => col,
        };
        frame.instances.push(
            Instance::new(origin + face * (p_local * scale_mult), sv * scale_mult, part_col)
                .with_rot(face * r_local)
                .with_mesh(part.mesh),
        );
    }
}

/// Bounds + mesh id of one loaded part GLB, for rig assembly.
#[derive(Clone, Copy)]
pub struct PartSource {
    pub mesh: u32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl PartSource {
    pub fn height(&self) -> f32 {
        (self.max[1] - self.min[1]).max(1e-3)
    }

    /// Mesh-space point at the given fraction of the bounds per axis.
    pub fn anchor(&self, fx: f32, fy: f32, fz: f32) -> Vec3 {
        Vec3::new(
            self.min[0] + (self.max[0] - self.min[0]) * fx,
            self.min[1] + (self.max[1] - self.min[1]) * fy,
            self.min[2] + (self.max[2] - self.min[2]) * fz,
        )
    }
}

/// Load GLB bytes into one merged mesh plus its bounds source.
pub fn source_from_glb_bytes(bytes: &[u8], mesh_id: u32) -> Result<(MeshData, PartSource), String> {
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
    if max[1] - min[1] <= 1e-3 {
        return Err("part has no vertical extent".into());
    }
    Ok((merged, PartSource { mesh: mesh_id, min, max }))
}

/// The jointed veteran: a skeleton plus its bound mesh parts.
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
    let dims = HumanoidDims::default();
    let skel = humanoid(&dims);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The scene shader's quaternion rotation, mirrored here so a shader
    /// formula regression fails a native test.
    fn wgsl_quat_rotate(q: [f32; 4], v: Vec3) -> Vec3 {
        let qv = Vec3::new(q[0], q[1], q[2]);
        v + 2.0 * qv.cross(qv.cross(v) + q[3] * v)
    }

    #[test]
    fn yaw_quat_matches_legacy_shader_formula() {
        // Old shader: x' = x*c + z*s, z' = -x*s + z*c.
        let yaw = 0.7f32;
        let (s, c) = yaw.sin_cos();
        for v in [Vec3::X, Vec3::Z, Vec3::new(0.3, -0.2, 0.9)] {
            let legacy = Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
            let q = Quat::from_rotation_y(yaw);
            assert!((q * v - legacy).length() < 1e-5, "glam {:?} vs legacy {legacy:?}", q * v);
            assert!((wgsl_quat_rotate(q.to_array(), v) - legacy).length() < 1e-5);
        }
    }

    #[test]
    fn wgsl_formula_matches_glam_for_arbitrary_rotations() {
        let q = (Quat::from_rotation_y(1.1) * Quat::from_rotation_x(-0.6) * Quat::from_rotation_z(0.4)).normalize();
        for v in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(-0.7, 0.2, 1.3)] {
            assert!((wgsl_quat_rotate(q.to_array(), v) - q * v).length() < 1e-5);
        }
    }

    #[test]
    fn neutral_pose_stacks_joints() {
        let d = HumanoidDims::default();
        let j = world_joints(&humanoid(&d), &Pose::default());
        let ankle = j[joint::ANKLE_L].0;
        assert!((ankle.y - d.ankle_h).abs() < 1e-4, "ankle at {}", ankle.y);
        assert!((ankle.x + d.hip_w * 0.5).abs() < 1e-4);
        assert!((j[joint::ANKLE_R].0.x - d.hip_w * 0.5).abs() < 1e-4);
        let neck = j[joint::NECK].0;
        assert!((neck.y - (d.pelvis_h + 0.05 + d.spine_len)).abs() < 1e-4);
    }

    #[test]
    fn walking_feet_stay_near_ground_and_move_forward() {
        let d = HumanoidDims::default();
        let skel = humanoid(&d);
        for i in 0..16 {
            let phase = i as f32 / 16.0 * std::f32::consts::TAU;
            let j = world_joints(&skel, &walk_pose(phase, 1.0, 0.0, 0.0, &d));
            for ankle in [j[joint::ANKLE_L].0, j[joint::ANKLE_R].0] {
                assert!(ankle.y > -0.02 && ankle.y < 0.45, "ankle y {} at phase {phase}", ankle.y);
            }
        }
        // A forward-swung hip (swing > 0 on L) puts the left ankle ahead (+Z).
        let j = world_joints(&skel, &walk_pose(std::f32::consts::FRAC_PI_2, 1.0, 0.0, 0.0, &d));
        assert!(j[joint::ANKLE_L].0.z > 0.1, "left ankle z {}", j[joint::ANKLE_L].0.z);
        assert!(j[joint::ANKLE_R].0.z < -0.05, "right ankle z {}", j[joint::ANKLE_R].0.z);
    }

    #[test]
    fn crouch_lowers_root_and_bends_knees() {
        let d = HumanoidDims::default();
        let skel = humanoid(&d);
        let stand = world_joints(&skel, &walk_pose(0.0, 0.0, 0.0, 0.0, &d));
        let crouch = world_joints(&skel, &walk_pose(0.0, 0.0, 1.0, 0.0, &d));
        assert!(crouch[joint::NECK].0.y < stand[joint::NECK].0.y - 0.2);
        // Feet stay close to the ground when crouched.
        assert!(crouch[joint::ANKLE_L].0.y < 0.35);
    }

    #[test]
    fn mirrored_parts_land_symmetrically() {
        let d = HumanoidDims::default();
        let skel = humanoid(&d);
        let pose = Pose::default();
        let part = |joint, mirror_x| RigPart {
            mesh: 1,
            joint,
            anchor: Vec3::new(0.1, 0.5, 0.0),
            offset: Vec3::ZERO,
            scale: 0.4,
            pre_rot: Quat::IDENTITY,
            mirror_x,
            tint: None,
        };
        let mut frame = Frame::default();
        push_rig(
            &mut frame,
            &[part(joint::HIP_L, false), part(joint::HIP_R, true)],
            &skel,
            &pose,
            Vec2::ZERO,
            0.0,
            [1.0; 3],
            1.0,
        );
        let (l, r) = (frame.instances[0], frame.instances[1]);
        assert!((l.position.x + r.position.x).abs() < 1e-4, "{} vs {}", l.position.x, r.position.x);
        assert!((l.position.y - r.position.y).abs() < 1e-4);
        assert!((l.scale.x + r.scale.x).abs() < 1e-4);
    }
}
