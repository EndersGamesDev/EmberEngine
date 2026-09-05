//! First-person sleeves from the operator's textured arm parts. Gloves are
//! weapon-specific meshes; this module connects their wrist sockets to elbows
//! below the view without bringing the operator's original open hands along.

use ember_engine::glam::{Quat, Vec3};
use ember_engine::rig::{ArmSide, RigCharacter, RigPart, joint};
use ember_engine::{Frame, Instance, MeshData};

const SLEEVES_GLB: &[u8] = include_bytes!("../assets/weapon-sleeves.glb");
const SLEEVE_JOINTS: [(&str, usize); 5] = [
    ("rig_shoulder_l", joint::SHOULDER_L),
    ("rig_elbow_l", joint::ELBOW_L),
    ("rig_shoulder_r", joint::SHOULDER_R),
    ("rig_elbow_r", joint::ELBOW_R),
    ("rig_spine", joint::SPINE),
];

/// Append sealed artist sleeves and the torso's repaired shoulder borders.
/// `first_mesh` is the base ID of `meshes`, as passed to `skinned_from_glb`.
/// Joint anchors, retarget corrections, hand geometry and all other body
/// parts are untouched, so both first- and third-person reuse the repair.
pub fn replace_parts(meshes: &mut Vec<MeshData>, character: &mut RigCharacter, first_mesh: u32) {
    let Ok(parts) = ember_engine::assets::load_glb(SLEEVES_GLB) else {
        tracing::warn!("sealed operator sleeves could not be loaded");
        return;
    };
    if parts.len() != SLEEVE_JOINTS.len()
        || SLEEVE_JOINTS.iter().any(|(name, joint)| {
            parts.iter().filter(|part| part.name == *name).count() != 1
                || !character.parts.iter().any(|part| part.joint == *joint)
        })
    {
        tracing::warn!("sealed operator sleeve nodes do not match the character");
        return;
    }
    let Some(end) = meshes
        .len()
        .checked_add(parts.len())
        .and_then(|n| u32::try_from(n).ok())
        .and_then(|n| first_mesh.checked_add(n))
    else {
        tracing::warn!("sealed operator sleeve mesh IDs exceed u32");
        return;
    };
    let mut mesh_id = end - u32::try_from(parts.len()).unwrap_or(0);
    for part in parts {
        let Some((_, joint)) = SLEEVE_JOINTS.iter().find(|(name, _)| part.name == *name) else {
            continue;
        };
        for rig_part in &mut character.parts {
            if rig_part.joint == *joint {
                rig_part.mesh = mesh_id;
            }
        }
        meshes.push(part.mesh);
        mesh_id += 1;
    }
}

const FOREARM_LENGTH: f32 = 0.27;
const UPPERARM_LENGTH: f32 = 0.29;

#[derive(Clone, Copy)]
struct ArmPoints {
    shoulder: Vec3,
    elbow: Vec3,
    wrist: Vec3,
}

/// The elbow stays outside and below the hand, with the shoulder continuing
/// toward the lower screen edge. Each segment has a fixed anatomical length;
/// moving a weapon socket never scales a sleeve to bridge a changing distance.
fn arm_points(wrist: Vec3, side: f32, eye: Vec3, camera_rotation: Quat) -> ArmPoints {
    let forward = camera_rotation * Vec3::X;
    let up = camera_rotation * Vec3::Y;
    let right = camera_rotation * Vec3::Z;
    let elbow_hint = eye + forward * 0.18 - up * 0.42 + right * (side * 0.28);
    let to_elbow = (elbow_hint - wrist)
        .try_normalize()
        .unwrap_or(-forward * 0.8 - up * 0.6);
    let elbow = wrist + to_elbow * FOREARM_LENGTH;
    let shoulder_hint = eye - forward * 0.08 - up * 0.72 + right * (side * 0.22);
    let to_shoulder = (shoulder_hint - elbow)
        .try_normalize()
        .unwrap_or(-forward * 0.8 - up * 0.6);
    ArmPoints {
        shoulder: elbow + to_shoulder * UPPERARM_LENGTH,
        elbow,
        wrist,
    }
}

/// Place one already-registered artist mesh onto a segment. `RigPart::anchor`
/// is in the same imported bind frame as the skeleton's child offset; a
/// shortest rotation aligns that axis to this frame's elbow/wrist line.
fn push_segment(
    frame: &mut Frame,
    part: &RigPart,
    bind_axis: Vec3,
    from: Vec3,
    to: Vec3,
    color: Vec3,
) {
    let bind_length = bind_axis.length();
    let segment = to - from;
    let length = segment.length();
    if bind_length < 1e-6 || length < 1e-6 || !bind_length.is_finite() || !length.is_finite() {
        return;
    }
    // Uniform fitting is constant for this mesh (0.27/0.29 m targets), so
    // sleeve width, texture detail and elbow shape retain their proportions.
    let fit = length / bind_length;
    let joint_rotation = Quat::from_rotation_arc(bind_axis / bind_length, segment / length);
    let rotation = joint_rotation * part.pre_rot;
    let mut scale = Vec3::splat(part.scale * fit);
    if part.mirror_x {
        scale.x = -scale.x;
    }
    let position = from + joint_rotation * (part.offset * fit) - rotation * (part.anchor * scale);
    let tint = part.tint.map_or(color, |tint| tint * color);
    frame.instances.push(
        Instance::new(position, scale, tint)
            .with_rot(rotation)
            .with_mesh(part.mesh)
            .without_shadow(),
    );
}

/// Draw textured sleeves reaching the glove wrists in world space. The camera
/// rotation maps +X forward, +Y up and +Z right, as the weapon does. Pass white
/// for `color` to retain the operator's authored sleeve picture. A missing
/// left socket means the off hand is occupied (shield/melee) and is omitted.
#[allow(clippy::too_many_arguments)]
pub fn push(
    frame: &mut Frame,
    character: &RigCharacter,
    right_wrist: Vec3,
    left_wrist: Option<Vec3>,
    eye: Vec3,
    camera_rotation: Quat,
    color: Vec3,
) {
    push_arm(
        frame,
        character,
        ArmSide::Right,
        right_wrist,
        eye,
        camera_rotation,
        color,
    );
    if let Some(wrist) = left_wrist {
        push_left(frame, character, wrist, eye, camera_rotation, color);
    }
}

/// Draw the support sleeve independently when its glove is on a shield or
/// another off-hand item instead of the weapon's support socket.
pub fn push_left(
    frame: &mut Frame,
    character: &RigCharacter,
    wrist: Vec3,
    eye: Vec3,
    camera_rotation: Quat,
    color: Vec3,
) {
    push_arm(
        frame,
        character,
        ArmSide::Left,
        wrist,
        eye,
        camera_rotation,
        color,
    );
}

fn push_arm(
    frame: &mut Frame,
    character: &RigCharacter,
    arm: ArmSide,
    wrist: Vec3,
    eye: Vec3,
    camera_rotation: Quat,
    color: Vec3,
) {
    if !eye.is_finite() || !camera_rotation.is_finite() || camera_rotation.length_squared() < 1e-8 {
        return;
    }
    if !wrist.is_finite() {
        return;
    }
    let camera_rotation = camera_rotation.normalize();
    let side = if arm == ArmSide::Right { 1.0 } else { -1.0 };
    let points = arm_points(wrist, side, eye, camera_rotation);
    let [shoulder, elbow, wrist_joint] = arm.joints();
    for part in &character.parts {
        if part.joint == elbow {
            push_segment(
                frame,
                part,
                character.skel.joints[wrist_joint].offset,
                points.elbow,
                points.wrist,
                color,
            );
        } else if part.joint == shoulder {
            push_segment(
                frame,
                part,
                character.skel.joints[elbow].offset,
                points.shoulder,
                points.elbow,
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_engine::rig::{HumanoidDims, humanoid, joint};

    #[test]
    fn sealed_sleeves_replace_only_arm_mesh_ids_with_original_anchors() {
        let first_mesh = 73;
        let (mut meshes, mut character) = ember_engine::rig::skinned_from_glb(
            include_bytes!("../../../assets/models/swat-parts.glb"),
            include_str!("../../../assets/models/swat-rig.json"),
            first_mesh,
        )
        .unwrap();
        let before: Vec<_> = character
            .parts
            .iter()
            .map(|p| (p.mesh, p.joint, p.anchor, p.offset, p.scale, p.pre_rot))
            .collect();
        let old_count = meshes.len();
        replace_parts(&mut meshes, &mut character, first_mesh);
        assert_eq!(meshes.len(), old_count + 5);
        for (part, old) in character.parts.iter().zip(before) {
            assert_eq!(
                (
                    part.joint,
                    part.anchor,
                    part.offset,
                    part.scale,
                    part.pre_rot
                ),
                (old.1, old.2, old.3, old.4, old.5)
            );
            if SLEEVE_JOINTS.iter().any(|(_, j)| *j == part.joint) {
                assert!(part.mesh >= first_mesh + u32::try_from(old_count).unwrap());
            } else {
                assert_eq!(part.mesh, old.0);
            }
        }
        for mesh in &meshes[old_count..] {
            let texture = mesh.texture.as_ref().unwrap();
            assert!(matches!(
                (texture.width, texture.height),
                (512, 512) | (1024, 1024)
            ));
            assert_eq!(
                texture.rgba8.len(),
                usize::try_from(texture.width * texture.height * 4).unwrap()
            );
            assert!(mesh.vertices.len() > 300);
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.pos.iter().all(|x| x.is_finite()))
            );
        }
        assert_eq!(
            meshes[old_count..]
                .iter()
                .filter(|mesh| mesh.texture.as_ref().unwrap().width == 1024)
                .count(),
            1
        );
    }

    #[test]
    fn arm_segments_keep_their_lengths_and_follow_camera_rotation() {
        for side in [-1.0, 1.0] {
            for wrist in [Vec3::new(0.47, -0.30, 0.24), Vec3::new(0.83, -0.34, 0.08)] {
                let neutral = arm_points(wrist, side, Vec3::ZERO, Quat::IDENTITY);
                assert!((neutral.elbow.distance(wrist) - FOREARM_LENGTH).abs() < 1e-6);
                assert!((neutral.shoulder.distance(neutral.elbow) - UPPERARM_LENGTH).abs() < 1e-6);
                assert!(neutral.elbow.x < wrist.x && neutral.elbow.y < wrist.y);
                assert!(neutral.shoulder.x < neutral.elbow.x);
                let eye = Vec3::new(4.0, 2.0, -3.0);
                let rotation = Quat::from_rotation_y(1.2) * Quat::from_rotation_z(0.4);
                let posed = arm_points(eye + rotation * wrist, side, eye, rotation);
                assert!(posed.elbow.distance(eye + rotation * neutral.elbow) < 1e-6);
                assert!(posed.shoulder.distance(eye + rotation * neutral.shoulder) < 1e-6);
            }
        }
    }

    #[test]
    fn sleeve_parts_keep_their_textured_mesh_ids_and_end_at_glove_wrists() {
        let dims = HumanoidDims::default();
        let skel = humanoid(&dims);
        let parts = [
            joint::SHOULDER_R,
            joint::ELBOW_R,
            joint::WRIST_R,
            joint::ELBOW_L,
        ]
        .into_iter()
        .map(|joint| RigPart {
            mesh: u32::try_from(joint).unwrap() + 100,
            joint,
            anchor: Vec3::ZERO,
            offset: Vec3::ZERO,
            scale: 1.0,
            pre_rot: Quat::IDENTITY,
            mirror_x: false,
            tint: None,
        })
        .collect();
        let character = RigCharacter { skel, dims, parts };
        let wrist = Vec3::new(0.50, -0.28, 0.23);
        let mut frame = Frame::default();
        push(
            &mut frame,
            &character,
            wrist,
            None,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
        );
        assert_eq!(
            frame.instances.len(),
            2,
            "no original hand or unoccupied left sleeve"
        );
        let forearm = frame
            .instances
            .iter()
            .find(|i| i.mesh == 100 + u32::try_from(joint::ELBOW_R).unwrap())
            .unwrap();
        let source_wrist = character.skel.joints[joint::WRIST_R].offset;
        let rendered_wrist = forearm.position + forearm.rot * (source_wrist * forearm.scale);
        assert!(rendered_wrist.distance(wrist) < 1e-6);
        assert!(frame.instances.iter().all(|i| !i.casts_shadow));
        assert!(frame.instances.iter().all(|i| i.color == Vec3::ONE));
    }
}
