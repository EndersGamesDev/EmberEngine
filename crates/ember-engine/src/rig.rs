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
    /// Per-joint rest correction, applied inside the joint before the
    /// animation rotation. Identity for rigs authored in the engine's rest
    /// pose; for an imported model it takes the bind pose (arms out) to a
    /// natural standing pose, so one animation drives both.
    pub correction: [Quat; joint::COUNT],
}

/// Parent of each joint, shared by every skeleton builder.
const PARENTS: [Option<usize>; joint::COUNT] = {
    use joint::{
        ANKLE_L, ANKLE_R, COUNT, ELBOW_L, ELBOW_R, HIP_L, HIP_R, KNEE_L, KNEE_R, NECK, ROOT,
        SHOULDER_L, SHOULDER_R, SPINE, WRIST_L, WRIST_R,
    };
    let mut p = [None; COUNT];
    p[SPINE] = Some(ROOT);
    p[NECK] = Some(SPINE);
    p[SHOULDER_L] = Some(SPINE);
    p[ELBOW_L] = Some(SHOULDER_L);
    p[WRIST_L] = Some(ELBOW_L);
    p[SHOULDER_R] = Some(SPINE);
    p[ELBOW_R] = Some(SHOULDER_R);
    p[WRIST_R] = Some(ELBOW_R);
    p[HIP_L] = Some(ROOT);
    p[KNEE_L] = Some(HIP_L);
    p[ANKLE_L] = Some(KNEE_L);
    p[HIP_R] = Some(ROOT);
    p[KNEE_R] = Some(HIP_R);
    p[ANKLE_R] = Some(KNEE_R);
    p
};

/// Segment lengths of the humanoid skeleton, in character units
/// (a ~1.8-tall figure; `push_rig`'s `scale_mult` resizes the whole rig).
#[derive(Clone, Copy)]
pub struct HumanoidDims {
    /// Ground to hip pivot. Must equal `thigh_len` + `shin_len` + `ankle_h`.
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
#[must_use]
pub fn humanoid(d: &HumanoidDims) -> Skeleton {
    use joint::{
        ANKLE_L, ANKLE_R, COUNT, ELBOW_L, ELBOW_R, HIP_L, HIP_R, KNEE_L, KNEE_R, NECK, ROOT,
        SHOULDER_L, SHOULDER_R, SPINE, WRIST_L, WRIST_R,
    };
    let mut joints = [JointDef {
        parent: None,
        offset: Vec3::ZERO,
    }; COUNT];
    let mut set = |j: usize, parent: Option<usize>, offset: Vec3| {
        joints[j] = JointDef { parent, offset };
    };
    set(ROOT, None, Vec3::ZERO);
    set(SPINE, Some(ROOT), Vec3::new(0.0, 0.05, 0.0));
    set(NECK, Some(SPINE), Vec3::new(0.0, d.spine_len, 0.0));
    for (side, sh, el, wr, hip, knee, ankle) in [
        (
            -1.0f32, SHOULDER_L, ELBOW_L, WRIST_L, HIP_L, KNEE_L, ANKLE_L,
        ),
        (1.0, SHOULDER_R, ELBOW_R, WRIST_R, HIP_R, KNEE_R, ANKLE_R),
    ] {
        set(
            sh,
            Some(SPINE),
            Vec3::new(side * d.shoulder_w * 0.5, d.spine_len - 0.03, 0.0),
        );
        set(el, Some(sh), Vec3::new(0.0, -d.upperarm_len, 0.0));
        set(wr, Some(el), Vec3::new(0.0, -d.forearm_len, 0.0));
        set(hip, Some(ROOT), Vec3::new(side * d.hip_w * 0.5, 0.0, 0.0));
        set(knee, Some(hip), Vec3::new(0.0, -d.thigh_len, 0.0));
        set(ankle, Some(knee), Vec3::new(0.0, -d.shin_len, 0.0));
    }
    Skeleton {
        joints,
        correction: [Quat::IDENTITY; COUNT],
    }
}

/// Build a skeleton from an imported model's bind-pose joint positions
/// (engine space, Y up), plus the dimensions its animation needs.
///
/// Joint offsets come straight from the measured positions, so meshes
/// anchored at those points stay connected. Corrections rotate each limb
/// from its bind direction to a natural standing direction: an arms-out
/// A-pose becomes a figure with its arms down, without touching the mesh.
#[must_use]
pub fn skeleton_from_bind(pos: &[Vec3; joint::COUNT]) -> (Skeleton, HumanoidDims) {
    use joint::{
        ANKLE_L, ANKLE_R, COUNT, ELBOW_L, ELBOW_R, HIP_L, HIP_R, KNEE_L, KNEE_R, NECK, SHOULDER_L,
        SHOULDER_R, SPINE, WRIST_L, WRIST_R,
    };
    let mut joints = [JointDef {
        parent: None,
        offset: Vec3::ZERO,
    }; COUNT];
    for j in 0..COUNT {
        let parent = PARENTS[j];
        joints[j] = JointDef {
            parent,
            offset: match parent {
                Some(p) => pos[j] - pos[p],
                None => Vec3::ZERO,
            },
        };
    }

    // Rest direction per limb joint: arms hang slightly out and forward,
    // legs straight down. Joints not listed keep their parent's frame.
    let want = |x: f32, y: f32, z: f32| Vec3::new(x, y, z).normalize();
    let targets: [(usize, usize, Vec3); 8] = [
        (SHOULDER_L, ELBOW_L, want(0.18, -1.0, 0.06)),
        (ELBOW_L, WRIST_L, want(0.06, -1.0, 0.10)),
        (SHOULDER_R, ELBOW_R, want(-0.18, -1.0, 0.06)),
        (ELBOW_R, WRIST_R, want(-0.06, -1.0, 0.10)),
        (HIP_L, KNEE_L, want(0.0, -1.0, 0.02)),
        (KNEE_L, ANKLE_L, want(0.0, -1.0, -0.02)),
        (HIP_R, KNEE_R, want(0.0, -1.0, 0.02)),
        (KNEE_R, ANKLE_R, want(0.0, -1.0, -0.02)),
    ];
    // acc[j] is the joint's world rotation at rest; a joint's correction is
    // whatever its parent's rest frame still needs to reach that.
    let mut acc = [Quat::IDENTITY; COUNT];
    let mut correction = [Quat::IDENTITY; COUNT];
    for j in 0..COUNT {
        let parent_acc = PARENTS[j].map_or(Quat::IDENTITY, |p| acc[p]);
        acc[j] = match targets.iter().find(|(from, ..)| *from == j) {
            Some((_, child, desired)) => {
                let bind_dir = (pos[*child] - pos[j]).normalize_or_zero();
                if bind_dir == Vec3::ZERO {
                    parent_acc
                } else {
                    Quat::from_rotation_arc(bind_dir, *desired)
                }
            }
            None => parent_acc,
        };
        correction[j] = (parent_acc.inverse() * acc[j]).normalize();
    }

    let len = |a: usize, b: usize| (pos[b] - pos[a]).length();
    let thigh_len = len(HIP_L, KNEE_L);
    let shin_len = len(KNEE_L, ANKLE_L);
    let ankle_h = pos[ANKLE_L].y;
    let dims = HumanoidDims {
        // Keeps the soles on the ground once the legs stand vertical.
        pelvis_h: thigh_len + shin_len + ankle_h,
        hip_w: (pos[HIP_L].x - pos[HIP_R].x).abs(),
        thigh_len,
        shin_len,
        ankle_h,
        spine_len: len(SPINE, NECK),
        shoulder_w: (pos[SHOULDER_L].x - pos[SHOULDER_R].x).abs(),
        upperarm_len: len(SHOULDER_L, ELBOW_L),
        forearm_len: len(ELBOW_L, WRIST_L),
        neck_len: 0.13,
    };
    (Skeleton { joints, correction }, dims)
}

/// Engine joint names, in `joint` index order — the node-name suffixes
/// `tools/swat_split.py` writes ("`rig_head`", "`rig_shoulder_l`", ...).
pub const JOINT_NAMES: [&str; joint::COUNT] = [
    "root",
    "spine",
    "neck",
    "shoulder_l",
    "elbow_l",
    "wrist_l",
    "shoulder_r",
    "elbow_r",
    "wrist_r",
    "hip_l",
    "knee_l",
    "ankle_l",
    "hip_r",
    "knee_r",
    "ankle_r",
];

/// Pull one `"name": [x, y, z]` triple out of the rig JSON. The file is
/// ours and tiny, so this avoids a serde dependency in the engine.
///
/// The key must be followed by `:` and an array, and every occurrence is
/// tried. Matching the bare name anywhere would find it inside a list of
/// part names too — and then read whichever array came next, silently
/// giving every joint the same pivot. (Found by dev-a1 in review.)
fn parse_joint(json: &str, name: &str) -> Option<Vec3> {
    let key = format!("\"{name}\"");
    let mut from = 0;
    while let Some(hit) = json[from..].find(&key) {
        let after = from + hit + key.len();
        from = after;
        let Some(body) = json[after..].trim_start().strip_prefix(':') else {
            continue;
        };
        let Some(inner) = body.trim_start().strip_prefix('[') else {
            continue;
        };
        let Some(close) = inner.find(']') else {
            continue;
        };
        let nums: Vec<f32> = inner[..close]
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if nums.len() == 3 {
            return Some(Vec3::new(nums[0], nums[1], nums[2]));
        }
    }
    None
}

/// Load a split skinned model.
///
/// The input is a GLB whose nodes are named `rig_<joint>` plus the JSON of
/// bind-pose joint positions that `tools/swat_split.py`
/// writes beside it. Returns the meshes to register (starting at
/// `first_mesh`) and the rig that drives them.
///
/// # Errors
///
/// Returns an error when the rig JSON omits a joint, the GLB cannot be loaded
/// or has no rig nodes, or the assigned mesh identifiers exceed `u32`.
pub fn skinned_from_glb(
    glb: &[u8],
    rig_json: &str,
    first_mesh: u32,
) -> Result<(Vec<MeshData>, RigCharacter), String> {
    let mut bind = [Vec3::ZERO; joint::COUNT];
    for (i, name) in JOINT_NAMES.iter().enumerate() {
        bind[i] = parse_joint(rig_json, name).ok_or_else(|| format!("rig json lacks {name}"))?;
    }
    let parts = crate::assets::load_glb(glb)?;
    let mut meshes = Vec::new();
    let mut bound = Vec::new();
    for part in parts {
        // Blender suffixes duplicate node names ("rig_neck.001").
        let stem = part.name.trim_start_matches("rig_");
        let stem = stem.split('.').next().unwrap_or(stem);
        let Some(j) = JOINT_NAMES.iter().position(|n| *n == stem) else {
            continue;
        };
        let mesh_offset = u32::try_from(meshes.len())
            .map_err(|_| "skinned model has more than u32::MAX meshes")?;
        let mesh_id = first_mesh
            .checked_add(mesh_offset)
            .ok_or("skinned model mesh id exceeds u32::MAX")?;
        bound.push((mesh_id, j));
        meshes.push(part.mesh);
    }
    if bound.is_empty() {
        return Err("no rig_<joint> nodes in the glb".into());
    }
    Ok((meshes, skinned_rig(&bind, &bound)))
}

/// Assemble a rig from an imported skinned model.
///
/// Every part is already in bind space, so its anchor is its joint's bind position, at model scale
/// and unrotated. `parts` pairs a registered mesh id with its joint.
#[must_use]
pub fn skinned_rig(bind: &[Vec3; joint::COUNT], parts: &[(u32, usize)]) -> RigCharacter {
    let (skel, dims) = skeleton_from_bind(bind);
    let parts = parts
        .iter()
        .map(|&(mesh, joint)| RigPart {
            mesh,
            joint,
            anchor: bind[joint],
            offset: Vec3::ZERO,
            scale: 1.0,
            pre_rot: Quat::IDENTITY,
            mirror_x: false,
            // Textured model: keep the authored colors.
            tint: None,
        })
        .collect();
    RigCharacter { skel, dims, parts }
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
#[must_use]
pub fn world_joints(skel: &Skeleton, pose: &Pose) -> [(Vec3, Quat); joint::COUNT] {
    let mut out = [(Vec3::ZERO, Quat::IDENTITY); joint::COUNT];
    for (i, j) in skel.joints.iter().enumerate() {
        let (pp, pr) = match j.parent {
            Some(p) => out[p],
            None => (pose.root_pos, Quat::IDENTITY),
        };
        // The correction sits innermost: it poses the bind skeleton, then
        // the animation swings that corrected limb in its parent's frame.
        out[i] = (
            pp + pr * j.offset,
            (pr * pose.local_rot[i] * skel.correction[i]).normalize(),
        );
    }
    out
}

/// Procedural locomotion pose.
///
/// `phase` advances with distance walked (radians), `amp` eases the gait in
/// and out (0 = standing), `crouch` (0..1) sinks into a knees-bent stance,
/// and `idle_t` is wall-clock time driving the idle breathing sway.
#[must_use]
pub fn walk_pose(phase: f32, amp: f32, crouch: f32, idle_t: f32, d: &HumanoidDims) -> Pose {
    use joint::{
        ANKLE_L, ANKLE_R, ELBOW_L, ELBOW_R, HIP_L, HIP_R, KNEE_L, KNEE_R, NECK, SHOULDER_L,
        SHOULDER_R, SPINE,
    };
    let pitch = Quat::from_rotation_x;
    let swing = phase.sin() * amp;
    let mut p = Pose::default();

    // Legs. Rotating a hanging (-Y) limb by a negative X angle swings it
    // toward +Z (forward). Knees flex (positive pitch) as the leg passes
    // under the body, straighten at the stride extremes.
    let hip_amp = 0.55;
    for (side_phase, hip, knee, ankle) in [
        (0.0f32, HIP_L, KNEE_L, ANKLE_L),
        (std::f32::consts::PI, HIP_R, KNEE_R, ANKLE_R),
    ] {
        let s = (phase + side_phase).sin() * amp;
        let flex = (phase + side_phase).cos().max(0.0) * amp;
        let hip_a = 1.0f32.mul_add(-crouch, -hip_amp * s);
        let knee_a = 1.35f32.mul_add(crouch, 0.70f32.mul_add(flex, 0.10 * amp));
        p.local_rot[hip] = pitch(hip_a);
        p.local_rot[knee] = pitch(knee_a);
        // Keep the boot roughly level with the ground.
        p.local_rot[ankle] = pitch(-(hip_a + knee_a) * 0.85);
    }

    // Torso: slight forward lean while moving or crouched, a breathing sway
    // when idle, and a counter-twist against the hips.
    let breathe = (idle_t * 1.7).sin() * 0.02 * (1.0 - amp);
    let spine_pitch = 0.38f32.mul_add(crouch, 0.08 * amp) + breathe;
    p.local_rot[SPINE] = Quat::from_rotation_y(0.12 * swing) * pitch(spine_pitch);
    p.local_rot[NECK] = pitch(-spine_pitch * 0.8);

    // Arms swing opposite their same-side leg; elbows keep a soft bend that
    // deepens as the arm travels back.
    for (sign, sh, el) in [(1.0f32, SHOULDER_L, ELBOW_L), (-1.0, SHOULDER_R, ELBOW_R)] {
        let s = sign * swing;
        p.local_rot[sh] = pitch(0.45 * s);
        p.local_rot[el] = pitch(-0.25f32.mul_add(crouch, 0.30f32.mul_add(s.max(0.0), 0.30)));
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
// A rig placement is defined by these independent pose, mesh, and world inputs.
#[allow(clippy::too_many_arguments)]
pub fn push_rig(
    frame: &mut Frame,
    parts: &[RigPart],
    skel: &Skeleton,
    pose: &Pose,
    pos: Vec2,
    // feet_y: height of the ground under the character — 0 on the floor, a
    // crate top when standing on cover, rising while airborne.
    feet_y: f32,
    facing_yaw: f32,
    color: [f32; 3],
    scale_mult: f32,
) {
    let joints = world_joints(skel, pose);
    let face = Quat::from_rotation_y(facing_yaw);
    let origin = Vec3::new(pos.x, feet_y, pos.y);
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
            Instance::new(
                origin + face * (p_local * scale_mult),
                sv * scale_mult,
                part_col,
            )
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
    /// Mesh authored facing the camera (-Z): give it half a turn to face
    /// the rig's +Z forward. Single-view v1 meshes need this; multi-view
    /// Hunyuan-2mv meshes come out already facing +Z.
    pub flipped: bool,
}

impl PartSource {
    #[must_use]
    pub fn height(&self) -> f32 {
        (self.max[1] - self.min[1]).max(1e-3)
    }

    /// Mesh-space point at the given fraction of the bounds per axis.
    #[must_use]
    pub fn anchor(&self, fx: f32, fy: f32, fz: f32) -> Vec3 {
        Vec3::new(
            (self.max[0] - self.min[0]).mul_add(fx, self.min[0]),
            (self.max[1] - self.min[1]).mul_add(fy, self.min[1]),
            (self.max[2] - self.min[2]).mul_add(fz, self.min[2]),
        )
    }
}

/// Load GLB bytes into one merged mesh plus its bounds source.
///
/// # Errors
///
/// Returns an error when the GLB cannot be loaded or the merged mesh has no
/// usable vertical extent.
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
    Ok((
        merged,
        PartSource {
            flipped: false,
            mesh: mesh_id,
            min,
            max,
        },
    ))
}

/// The jointed veteran: a skeleton plus its bound mesh parts.
pub struct RigCharacter {
    pub skel: Skeleton,
    pub dims: HumanoidDims,
    pub parts: Vec<RigPart>,
}

/// Everything the veteran can be assembled from.
///
/// The five base meshes are
/// required (the v1 set); each optional v2 segment upgrades detail when its
/// GLB exists — `arm` doubles as upper arm + forearm and `leg` as thigh +
/// shin until the dedicated segments land.
#[derive(Default, Clone, Copy)]
pub struct VeteranSources {
    pub head: Option<PartSource>,
    pub torso: Option<PartSource>,
    pub arm: Option<PartSource>,
    pub leg: Option<PartSource>,
    pub boot: Option<PartSource>,
    pub helmet: Option<PartSource>,
    pub pelvis: Option<PartSource>,
    pub upperarm: Option<PartSource>,
    pub forearm: Option<PartSource>,
    pub hand: Option<PartSource>,
    pub thigh: Option<PartSource>,
    pub shin: Option<PartSource>,
    pub backpack: Option<PartSource>,
    pub rifle: Option<PartSource>,
}

impl VeteranSources {
    #[must_use]
    pub const fn has_base(&self) -> bool {
        self.head.is_some()
            && self.torso.is_some()
            && self.arm.is_some()
            && self.leg.is_some()
            && self.boot.is_some()
    }
}

/// Assemble the jointed veteran from the available part meshes.
///
/// # Panics
///
/// Panics if any base mesh reported by [`VeteranSources::has_base`] is absent.
#[must_use]
// The assembly is declarative and remains linear so each optional mesh fallback is visible.
#[allow(clippy::too_many_lines)]
pub fn veteran_rig(s: &VeteranSources) -> RigCharacter {
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
    let glove = Vec3::new(0.16, 0.14, 0.12);
    let gunmetal = Vec3::new(0.13, 0.13, 0.14);
    let (head, torso, arm, leg, boot) = (
        s.head.expect("veteran_rig needs the base set"),
        s.torso.unwrap(),
        s.arm.unwrap(),
        s.leg.unwrap(),
        s.boot.unwrap(),
    );
    let mut parts = Vec::new();
    let mut add = |src: PartSource,
                   joint: usize,
                   anchor: Vec3,
                   offset: Vec3,
                   target_h: f32,
                   mirror_x: bool,
                   tint: Vec3,
                   extra_rot: Quat| {
        let base = if src.flipped { flip } else { Quat::IDENTITY };
        parts.push(RigPart {
            mesh: src.mesh,
            joint,
            anchor,
            offset,
            scale: target_h / src.height(),
            pre_rot: base * extra_rot,
            mirror_x,
            tint: Some(tint),
        });
    };
    // Torso: with a dedicated pelvis it ends at the waist; otherwise it
    // hangs low enough to cover the hips.
    let (torso_h, torso_off) = if s.pelvis.is_some() {
        (0.60, -0.04)
    } else {
        (0.70, -0.16)
    };
    add(
        torso,
        joint::SPINE,
        torso.anchor(0.5, 0.0, 0.5),
        Vec3::new(0.0, torso_off, 0.0),
        torso_h,
        false,
        tan,
        Quat::IDENTITY,
    );
    if let Some(p) = s.pelvis {
        add(
            p,
            joint::ROOT,
            p.anchor(0.5, 1.0, 0.5),
            Vec3::new(0.0, 0.10, 0.0),
            0.28,
            false,
            olive,
            Quat::IDENTITY,
        );
    }
    add(
        head,
        joint::NECK,
        head.anchor(0.5, 0.0, 0.5),
        Vec3::new(0.0, 0.01, 0.0),
        0.30,
        false,
        skin,
        Quat::IDENTITY,
    );
    if let Some(hm) = s.helmet {
        add(
            hm,
            joint::NECK,
            hm.anchor(0.5, 0.0, 0.5),
            Vec3::new(0.0, 0.10, 0.0),
            0.26,
            false,
            olive,
            Quat::IDENTITY,
        );
    }
    if let Some(bp) = s.backpack {
        add(
            bp,
            joint::SPINE,
            bp.anchor(0.5, 0.5, 0.5),
            Vec3::new(0.0, 0.26, -0.20),
            0.52,
            false,
            tan * 0.8,
            Quat::IDENTITY,
        );
    }
    if let Some(rf) = s.rifle {
        // Slung diagonally across the back until a proper held pose exists.
        let sling = Quat::from_rotation_z(0.6);
        add(
            rf,
            joint::SPINE,
            rf.anchor(0.5, 0.5, 0.5),
            Vec3::new(0.0, 0.30, -0.34),
            0.34,
            false,
            gunmetal,
            sling,
        );
    }
    for (mirror_x, sh, el, wr, hip, knee, ankle) in [
        (
            false,
            joint::SHOULDER_L,
            joint::ELBOW_L,
            joint::WRIST_L,
            joint::HIP_L,
            joint::KNEE_L,
            joint::ANKLE_L,
        ),
        (
            true,
            joint::SHOULDER_R,
            joint::ELBOW_R,
            joint::WRIST_R,
            joint::HIP_R,
            joint::KNEE_R,
            joint::ANKLE_R,
        ),
    ] {
        let ua = s.upperarm.unwrap_or(arm);
        add(
            ua,
            sh,
            ua.anchor(0.5, 1.0, 0.5),
            Vec3::ZERO,
            dims.upperarm_len + 0.05,
            mirror_x,
            olive,
            Quat::IDENTITY,
        );
        let fa = s.forearm.unwrap_or(arm);
        let fa_h = if s.forearm.is_some() {
            dims.forearm_len + 0.04
        } else {
            dims.forearm_len + 0.12
        };
        add(
            fa,
            el,
            fa.anchor(0.5, 1.0, 0.5),
            Vec3::ZERO,
            fa_h,
            mirror_x,
            skin,
            Quat::IDENTITY,
        );
        if let Some(h) = s.hand {
            add(
                h,
                wr,
                h.anchor(0.5, 1.0, 0.5),
                Vec3::ZERO,
                0.155,
                mirror_x,
                glove,
                Quat::IDENTITY,
            );
        }
        let th = s.thigh.unwrap_or(leg);
        add(
            th,
            hip,
            th.anchor(0.5, 1.0, 0.5),
            Vec3::ZERO,
            dims.thigh_len + 0.05,
            mirror_x,
            trouser,
            Quat::IDENTITY,
        );
        let sn = s.shin.unwrap_or(leg);
        add(
            sn,
            knee,
            sn.anchor(0.5, 1.0, 0.5),
            Vec3::ZERO,
            dims.shin_len + 0.04,
            mirror_x,
            trouser,
            Quat::IDENTITY,
        );
        // Boot pivots above the heel, a third of the way along the foot.
        add(
            boot,
            ankle,
            boot.anchor(0.5, 1.0, 0.38),
            Vec3::ZERO,
            dims.ankle_h + 0.10,
            mirror_x,
            leather,
            Quat::IDENTITY,
        );
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
            let legacy = Vec3::new(
                f32::mul_add(v.z, s, v.x * c),
                v.y,
                f32::mul_add(v.z, c, -v.x * s),
            );
            let q = Quat::from_rotation_y(yaw);
            assert!(
                (q * v - legacy).length() < 1e-5,
                "glam {:?} vs legacy {legacy:?}",
                q * v
            );
            assert!((wgsl_quat_rotate(q.to_array(), v) - legacy).length() < 1e-5);
        }
    }

    #[test]
    fn wgsl_formula_matches_glam_for_arbitrary_rotations() {
        let q =
            (Quat::from_rotation_y(1.1) * Quat::from_rotation_x(-0.6) * Quat::from_rotation_z(0.4))
                .normalize();
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
        assert!(f32::mul_add(d.hip_w, 0.5, ankle.x).abs() < 1e-4);
        assert!(f32::mul_add(d.hip_w, -0.5, j[joint::ANKLE_R].0.x).abs() < 1e-4);
        let neck = j[joint::NECK].0;
        assert!((neck.y - (d.pelvis_h + 0.05 + d.spine_len)).abs() < 1e-4);
    }

    #[test]
    fn walking_feet_stay_near_ground_and_move_forward() {
        let d = HumanoidDims::default();
        let skel = humanoid(&d);
        for i in 0i16..16 {
            let phase = f32::from(i) / 16.0 * std::f32::consts::TAU;
            let j = world_joints(&skel, &walk_pose(phase, 1.0, 0.0, 0.0, &d));
            for ankle in [j[joint::ANKLE_L].0, j[joint::ANKLE_R].0] {
                assert!(
                    ankle.y > -0.02 && ankle.y < 0.45,
                    "ankle y {} at phase {phase}",
                    ankle.y
                );
            }
        }
        // A forward-swung hip (swing > 0 on L) puts the left ankle ahead (+Z).
        let j = world_joints(
            &skel,
            &walk_pose(std::f32::consts::FRAC_PI_2, 1.0, 0.0, 0.0, &d),
        );
        assert!(
            j[joint::ANKLE_L].0.z > 0.1,
            "left ankle z {}",
            j[joint::ANKLE_L].0.z
        );
        assert!(
            j[joint::ANKLE_R].0.z < -0.05,
            "right ankle z {}",
            j[joint::ANKLE_R].0.z
        );
    }

    /// An imported A-pose bind skeleton, arms out along X (Mixamo-like).
    fn bind_a_pose() -> [Vec3; joint::COUNT] {
        use joint::*;
        let mut p = [Vec3::ZERO; COUNT];
        p[ROOT] = Vec3::new(0.0, 0.96, 0.0);
        p[SPINE] = Vec3::new(0.0, 1.07, 0.0);
        p[NECK] = Vec3::new(0.0, 1.51, 0.0);
        for (s, sh, el, wr, hip, knee, ankle) in [
            (1.0f32, SHOULDER_L, ELBOW_L, WRIST_L, HIP_L, KNEE_L, ANKLE_L),
            (-1.0, SHOULDER_R, ELBOW_R, WRIST_R, HIP_R, KNEE_R, ANKLE_R),
        ] {
            p[sh] = Vec3::new(s * 0.16, 1.42, 0.0);
            p[el] = Vec3::new(s * 0.39, 1.34, 0.0);
            p[wr] = Vec3::new(s * 0.58, 1.28, 0.0);
            p[hip] = Vec3::new(s * 0.11, 0.90, 0.0);
            p[knee] = Vec3::new(s * 0.13, 0.47, 0.0);
            p[ankle] = Vec3::new(s * 0.16, 0.14, 0.0);
        }
        p
    }

    #[test]
    fn rig_json_joints_resolve_whatever_the_key_order() {
        // A list of part names before the pivot map used to hijack every
        // lookup: the scan matched the name inside the list and then read
        // the next array it found, so every joint shared one pivot.
        let json = r#"{
          "parts": ["root", "spine", "neck"],
          "joints": {
            "root": [1.0, 2.0, 3.0],
            "spine": [4.0, 5.0, 6.0],
            "neck": [7.0, 8.0, 9.0]
          }
        }"#;
        assert_eq!(parse_joint(json, "root"), Some(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(parse_joint(json, "spine"), Some(Vec3::new(4.0, 5.0, 6.0)));
        assert_eq!(parse_joint(json, "neck"), Some(Vec3::new(7.0, 8.0, 9.0)));
        // The same file with the map first must of course still work.
        let flipped = r#"{"joints": {"root": [1.0, 2.0, 3.0]}, "parts": ["root"]}"#;
        assert_eq!(parse_joint(flipped, "root"), Some(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(parse_joint(flipped, "elbow_l"), None);
    }

    #[test]
    fn imported_bind_pose_retargets_arms_down() {
        let bind = bind_a_pose();
        let (skel, dims) = skeleton_from_bind(&bind);
        // Segment lengths survive the retarget.
        assert!(
            (dims.upperarm_len - (bind[joint::ELBOW_L] - bind[joint::SHOULDER_L]).length()).abs()
                < 1e-5
        );
        assert!((dims.pelvis_h - (dims.thigh_len + dims.shin_len + dims.ankle_h)).abs() < 1e-5);

        let j = world_joints(&skel, &walk_pose(0.0, 0.0, 0.0, 0.0, &dims));
        // Arms hang: each wrist sits well below its shoulder and much
        // closer to the body than the arms-out bind pose.
        for (sh, wr) in [
            (joint::SHOULDER_L, joint::WRIST_L),
            (joint::SHOULDER_R, joint::WRIST_R),
        ] {
            let (s, w) = (j[sh].0, j[wr].0);
            assert!(w.y < s.y - 0.4, "wrist y {} vs shoulder {}", w.y, s.y);
            assert!((w.x - s.x).abs() < 0.2, "wrist x drift {}", w.x - s.x);
        }
        // Soles land on the ground and the head stays on top.
        assert!(
            j[joint::ANKLE_L].0.y < 0.20,
            "ankle {}",
            j[joint::ANKLE_L].0.y
        );
        assert!(j[joint::NECK].0.y > 1.4, "neck {}", j[joint::NECK].0.y);
    }

    #[test]
    fn retargeted_arms_still_swing_when_walking() {
        let bind = bind_a_pose();
        let (skel, dims) = skeleton_from_bind(&bind);
        let front = world_joints(
            &skel,
            &walk_pose(std::f32::consts::FRAC_PI_2, 1.0, 0.0, 0.0, &dims),
        );
        let back = world_joints(
            &skel,
            &walk_pose(-std::f32::consts::FRAC_PI_2, 1.0, 0.0, 0.0, &dims),
        );
        // The same wrist travels fore/aft between opposite phases.
        let travel = (front[joint::WRIST_L].0.z - back[joint::WRIST_L].0.z).abs();
        assert!(travel > 0.15, "wrist z travel {travel}");
        // Legs still swing in opposition.
        assert!(front[joint::ANKLE_L].0.z > front[joint::ANKLE_R].0.z);
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
            0.0,
            [1.0; 3],
            1.0,
        );
        let (l, r) = (frame.instances[0], frame.instances[1]);
        assert!(
            (l.position.x + r.position.x).abs() < 1e-4,
            "{} vs {}",
            l.position.x,
            r.position.x
        );
        assert!((l.position.y - r.position.y).abs() < 1e-4);
        assert!((l.scale.x + r.scale.x).abs() < 1e-4);
    }
}
