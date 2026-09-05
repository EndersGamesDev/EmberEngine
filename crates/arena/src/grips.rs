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
        self.weapons.get(usize::from(id)).and_then(Option::as_ref)
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
    let reach = |side: ArmSide| {
        let [_, elbow, wrist] = side.joints();
        (character.skel.joints[elbow].offset.length()
            + character.skel.joints[wrist].offset.length())
            * BODY_SCALE
            * 0.985
    };
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
}
