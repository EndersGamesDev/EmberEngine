//! Weapon-local gloves and a shared, reachable third-person attachment.
//!
//! A gun, its fingers and its muzzle use ONE transform. The body reaches to
//! authored wrist sockets; it never stretches an arm to the old box-body mount.

use std::collections::HashMap;

use ember_engine::glam::{Quat, Vec2, Vec3};
use ember_engine::rig::{self, ArmSide, Pose, RigCharacter};
use ember_engine::{Frame, Instance, MeshData};
use serde::Deserialize;

pub const BODY_SCALE: f32 = 0.95;
const SLOTS: usize = arena_core::shooter::WEAPON_COUNT as usize + 1;

#[derive(Clone, Copy, Debug)]
pub struct Hand {
    pub mesh: u32,
    pub wrist: Vec3,
    pub palm: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct WeaponGrip {
    pub right: Hand,
    pub left: Hand,
}

#[derive(Clone, Default)]
pub struct Grips {
    weapons: [Option<WeaponGrip>; SLOTS],
}

impl Grips {
    /// The M4 uses the sidearm mesh today, so it must use that mesh's grip too.
    pub fn get(&self, id: u8) -> Option<&WeaponGrip> {
        // Breach-12 geometry is authored around the unchanged AK glove and
        // wrist sockets; no extra texture uploads or loose fallback hands.
        let grip_id = if id == arena_core::shooter::SHOTGUN {
            3
        } else {
            id
        };
        self.weapons
            .get(usize::from(grip_id))
            .and_then(Option::as_ref)
    }
}

#[derive(Deserialize)]
struct Socket {
    node: String,
    wrist: [f32; 3],
    palm: [f32; 3],
}

#[derive(Deserialize)]
struct Pair {
    right: Socket,
    left: Socket,
}

#[derive(Deserialize)]
struct Sidecar {
    weapons: HashMap<String, Pair>,
}

pub fn load(meshes: &mut Vec<MeshData>) -> Grips {
    let sidecar: Sidecar = match serde_json::from_str(include_str!("../assets/weapon-grips.json")) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "grip sockets unavailable; using legacy viewmodel hands");
            return Grips::default();
        }
    };
    let parts = match ember_engine::assets::load_glb(include_bytes!("../assets/weapon-grips.glb")) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "grip meshes unavailable; using legacy viewmodel hands");
            return Grips::default();
        }
    };
    let mut nodes = HashMap::new();
    for part in parts {
        let mesh = u32::try_from(meshes.len() + 1).expect("grip mesh count fits u32");
        nodes.insert(part.name, mesh);
        meshes.push(part.mesh);
    }
    let hand = |socket: &Socket| -> Option<Hand> {
        let wrist = Vec3::from_array(socket.wrist);
        let palm = Vec3::from_array(socket.palm);
        (wrist.is_finite() && palm.is_finite()).then_some(Hand {
            mesh: *nodes.get(&socket.node)?,
            wrist,
            palm,
        })
    };
    let mut grips = Grips::default();
    for (key, pair) in sidecar.weapons {
        if let Ok(id @ 1..SLOTS) = key.parse::<usize>() {
            grips.weapons[id] = hand(&pair.right)
                .zip(hand(&pair.left))
                .map(|(right, left)| WeaponGrip { right, left });
        }
    }
    grips
}

pub fn push_hand(frame: &mut Frame, hand: &Hand, base: Vec3, rotation: Quat) {
    frame.instances.push(
        Instance::new(base, Vec3::ONE, Vec3::ONE)
            .with_rot(rotation)
            .with_mesh(hand.mesh),
    );
}

pub fn push(frame: &mut Frame, grip: &WeaponGrip, base: Vec3, rotation: Quat, support: bool) {
    push_hand(frame, &grip.right, base, rotation);
    if support {
        push_hand(frame, &grip.left, base, rotation);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mount {
    pub base: Vec3,
    pub rotation: Quat,
    /// The shield handle, upright even while the gun aims up or down.
    pub shield: Vec3,
}

fn arm_reach(character: &RigCharacter, side: ArmSide) -> f32 {
    let [_, elbow, wrist] = side.joints();
    (character.skel.joints[elbow].offset.length() + character.skel.joints[wrist].offset.length())
        * BODY_SCALE
        * 0.985
}

/// Project onto the intersection of two arm-reach balls. Unlike a fixed number
/// of alternating projections, this also handles a nearly tangent intersection
/// without retaining a wrist outside the opposite arm's reach sphere.
fn project_pair(point: Vec3, a: Vec3, ra: f32, b: Vec3, rb: f32) -> Option<Vec3> {
    let project = |center: Vec3, radius: f32| center + (point - center).clamp_length_max(radius);
    let pa = project(a, ra);
    if pa.distance_squared(b) <= rb * rb {
        return Some(pa);
    }
    let pb = project(b, rb);
    if pb.distance_squared(a) <= ra * ra {
        return Some(pb);
    }
    let delta = b - a;
    let distance = delta.length();
    if distance > ra + rb {
        // No translation can preserve both authored wrist contacts. The caller
        // must reduce/fall back from the animation, not stretch the skeleton.
        return None;
    }
    if distance <= (ra - rb).abs() || distance < 0.000_001 {
        return Some(if ra <= rb { pa } else { pb });
    }
    let axis = delta / distance;
    let along = (ra * ra - rb * rb + distance * distance) / (2.0 * distance);
    let center = a + axis * along;
    let radius = (ra * ra - along * along).max(0.0).sqrt();
    let from_circle = point - center;
    let radial = from_circle - axis * from_circle.dot(axis);
    let direction = if radial.length_squared() > 0.000_000_000_1 {
        radial.normalize()
    } else {
        // Point lies on the centres' axis: choose a deterministic point on the
        // intersection circle. Either perpendicular is equally near it.
        axis.cross(if axis.x.abs() < 0.9 { Vec3::X } else { Vec3::Y })
            .normalize()
    };
    Some(center + direction * radius)
}

/// Reproject AFTER changing a remote mount's rotation or position for reload.
/// Pass the animated wrist socket used by `pose_arms`; gun and both gloves keep
/// exactly the same relative transform. Only the weapon base is translated.
/// The shield remains at its existing independent handle. `None` explicitly
/// rejects a pose with no reachable two-hand placement; render the unanimated
/// grip/mount for that frame rather than letting IK clamp away from the glove.
#[allow(clippy::too_many_arguments)]
pub fn constrain_mount(
    character: &RigCharacter,
    pose: &Pose,
    grip: &WeaponGrip,
    mut mount: Mount,
    position: Vec2,
    feet_y: f32,
    aim: Vec2,
    shield: bool,
) -> Option<Mount> {
    if !mount.base.is_finite() || !mount.rotation.is_finite() {
        return None;
    }
    let joints = rig::world_joints(&character.skel, pose);
    let face = Quat::from_rotation_y(aim.x.atan2(aim.y));
    let origin = Vec3::new(position.x, feet_y, position.y);
    let center = |side: ArmSide, wrist: Vec3| {
        origin + face * (joints[side.joints()[0]].0 * BODY_SCALE) - mount.rotation * wrist
    };
    let right = center(ArmSide::Right, grip.right.wrist);
    let rr = arm_reach(character, ArmSide::Right);
    mount.base = if shield {
        right + (mount.base - right).clamp_length_max(rr)
    } else {
        project_pair(
            mount.base,
            right,
            rr,
            center(ArmSide::Left, grip.left.wrist),
            arm_reach(character, ArmSide::Left),
        )?
    };
    Some(mount)
}

/// Attach to the posed shoulders, then project the weapon into the overlap of
/// both arms' reach spheres. This retains adult limb lengths for every gun,
/// crouch and aim pitch instead of pulling the wrists away from the fingers.
// The same explicit body placement inputs are used by rig::push_rig.
#[allow(clippy::too_many_arguments)]
pub fn mount(
    character: &RigCharacter,
    pose: &Pose,
    grip: &WeaponGrip,
    position: Vec2,
    feet_y: f32,
    aim: Vec2,
    pitch: f32,
    shield: bool,
) -> Mount {
    let joints = rig::world_joints(&character.skel, pose);
    let face = Quat::from_rotation_y(aim.x.atan2(aim.y));
    let origin = Vec3::new(position.x, feet_y, position.y);
    let rotation =
        face * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_z(pitch);
    let shoulder = |side: ArmSide| origin + face * (joints[side.joints()[0]].0 * BODY_SCALE);
    let reach = |side: ArmSide| arm_reach(character, side);
    let right = shoulder(ArmSide::Right);
    let left = shoulder(ArmSide::Left);
    let toward_center = -joints[ArmSide::Right.joints()[0]].0.x.signum();
    let right_offset = rotation * grip.right.wrist;
    let left_offset = rotation * grip.left.wrist;
    let mut base = right + face * Vec3::new(toward_center * 0.06, -0.16, 0.24) - right_offset;
    let constrain =
        |point: Vec3, center: Vec3, radius: f32| center + (point - center).clamp_length_max(radius);
    // Alternating convex projections; more than enough for our measured grip
    // spans, with fixed iteration count for deterministic captures/tests.
    for _ in 0..32 {
        if !shield {
            base = constrain(base, left - left_offset, reach(ArmSide::Left));
        }
        base = constrain(base, right - right_offset, reach(ArmSide::Right));
    }
    Mount {
        base,
        rotation,
        shield: left + face * Vec3::new(0.0, -0.13, 0.25),
    }
}

/// Solve in character space while the authored gun/gloves remain metre-sized.
#[allow(clippy::too_many_arguments)]
pub fn pose_arms(
    character: &RigCharacter,
    pose: &mut Pose,
    grip: &WeaponGrip,
    mount: Mount,
    position: Vec2,
    feet_y: f32,
    aim: Vec2,
    shield: bool,
) {
    let face = Quat::from_rotation_y(aim.x.atan2(aim.y));
    let origin = Vec3::new(position.x, feet_y, position.y);
    let to_character = |world: Vec3| face.inverse() * (world - origin) / BODY_SCALE;
    let right = mount.base + mount.rotation * grip.right.wrist;
    let shield_rot = Quat::from_rotation_y(-aim.y.atan2(aim.x));
    let left = if shield {
        mount.shield + shield_rot * (grip.left.wrist - grip.left.palm)
    } else {
        mount.base + mount.rotation * grip.left.wrist
    };
    let _right_solution = rig::solve_arm(
        &character.skel,
        pose,
        ArmSide::Right,
        to_character(right),
        None,
    );
    let _left_solution = rig::solve_arm(
        &character.skel,
        pose,
        ArmSide::Left,
        to_character(left),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_authored_weapon_has_two_distinct_metric_hand_meshes() {
        let mut meshes = Vec::new();
        let grips = load(&mut meshes);
        assert_eq!(meshes.len(), 12);
        for id in [1, 2, 3, 5, 6, 7] {
            let pair = grips.get(id).expect("authored weapon grip");
            assert_ne!(pair.right.mesh, pair.left.mesh);
            for hand in [pair.right, pair.left] {
                assert!(hand.wrist.is_finite() && hand.palm.is_finite());
                assert!((hand.palm - hand.wrist).length() < 0.2);
            }
        }
        assert!(grips.get(4).is_none());
    }

    #[test]
    fn exact_reach_projection_handles_tangent_contained_and_impossible_balls() {
        for separation in [0.0, 0.01, 0.5, 1.0, 1.9, 1.9999, 2.0] {
            let a = Vec3::new(-0.7, 2.0, 4.1);
            let b = a + Vec3::X * separation;
            for point in [Vec3::ZERO, a, b, a + Vec3::Y * 20.0] {
                let result = project_pair(point, a, 1.0, b, 1.0).expect("balls intersect");
                assert!(result.distance(a) <= 1.00001);
                assert!(result.distance(b) <= 1.00001);
            }
        }
        let contained = project_pair(Vec3::Y * 4.0, Vec3::ZERO, 2.0, Vec3::X * 0.1, 0.2)
            .expect("smaller ball is contained");
        assert!(contained.distance(Vec3::X * 0.1) <= 0.20001);
        assert!(project_pair(Vec3::ZERO, Vec3::ZERO, 0.5, Vec3::X * 2.0, 0.5).is_none());
    }

    #[test]
    fn all_authored_reloads_keep_both_wrists_in_actual_operator_reach() {
        let (_, character) = rig::skinned_from_glb(
            include_bytes!("../../../assets/models/swat-parts.glb"),
            include_str!("../../../assets/models/swat-rig.json"),
            1,
        )
        .expect("shipped SWAT operator must load");
        let grips = load(&mut Vec::new());
        let position = Vec2::new(4.25, -2.5);
        let feet_y = 1.3;
        let origin = Vec3::new(position.x, feet_y, position.y);
        for weapon in 1..=7 {
            let authored = *grips.get(if weapon == 4 { 1 } else { weapon }).unwrap();
            for crouch in [0.0, 1.0] {
                let pose = rig::walk_pose(1.7, 0.8, crouch, 2.3, &character.dims);
                let joints = rig::world_joints(&character.skel, &pose);
                for yaw in [-1.2_f32, 0.7] {
                    let aim = Vec2::new(yaw.cos(), yaw.sin());
                    let face = Quat::from_rotation_y(aim.x.atan2(aim.y));
                    for pitch in [
                        -arena_core::shooter::MAX_PITCH,
                        0.0,
                        arena_core::shooter::MAX_PITCH,
                    ] {
                        for tick in 0..=20_u8 {
                            let reload = crate::reload::pose(weapon, f32::from(tick) / 20.0);
                            let mut animated = authored;
                            animated.left.wrist = authored.left.palm
                                + reload.left_offset
                                + reload.left_rotation * (authored.left.wrist - authored.left.palm);
                            let mut candidate = mount(
                                &character, &pose, &animated, position, feet_y, aim, pitch, false,
                            );
                            candidate.base += candidate.rotation * reload.offset * 0.45;
                            candidate.rotation *= reload.rotation;
                            let reachable = constrain_mount(&character, &pose, &animated, candidate, position, feet_y, aim, false)
                                .unwrap_or_else(|| panic!("weapon {weapon}, tick {tick}, pitch {pitch}, crouch {crouch}: impossible mount"));
                            assert_eq!(reachable.rotation, candidate.rotation);
                            assert_eq!(reachable.shield, candidate.shield);
                            // Pose is intentionally not Clone/Copy; copy its value fields
                            // so the fixture can reuse the original stance on each sample.
                            #[allow(clippy::unnecessary_struct_initialization)]
                            let mut solved = Pose {
                                local_rot: pose.local_rot,
                                root_pos: pose.root_pos,
                            };
                            pose_arms(
                                &character,
                                &mut solved,
                                &animated,
                                reachable,
                                position,
                                feet_y,
                                aim,
                                false,
                            );
                            let solved_joints = rig::world_joints(&character.skel, &solved);
                            for (side, wrist) in [
                                (ArmSide::Right, animated.right.wrist),
                                (ArmSide::Left, animated.left.wrist),
                            ] {
                                let shoulder =
                                    origin + face * (joints[side.joints()[0]].0 * BODY_SCALE);
                                let target = reachable.base + reachable.rotation * wrist;
                                assert!(
                                    target.distance(shoulder)
                                        <= arm_reach(&character, side) + 0.00001
                                );
                                let actual = origin
                                    + face * (solved_joints[side.joints()[2]].0 * BODY_SCALE);
                                assert!(
                                    actual.distance(target) < 0.00005,
                                    "weapon {weapon}, tick {tick}, pitch {pitch}, {side:?}: IK misses wrist by {}m",
                                    actual.distance(target)
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
