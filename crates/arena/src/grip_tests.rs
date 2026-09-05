//! Integration regressions using the shipped operator, gun and glove assets.
//! These pin metric contact and render transforms, not visual finger quality.

use std::collections::HashSet;
use std::sync::OnceLock;

use arena_core::shooter::{MAX_PITCH, SIDEARM, WEAPON_COUNT};
use ember_engine::glam::{Quat, Vec2, Vec3};
use ember_engine::rig::{self, ArmSide, Pose, RigCharacter};
use ember_engine::{Frame, Instance, MeshData};

use crate::{grips, online};

struct Fixture {
    meshes: Vec<MeshData>,
    assets: online::Assets,
    character: RigCharacter,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let (meshes, assets) = online::load_assets();
        // Load directly so a broken SWAT asset cannot silently test the box or
        // generated-character fallback instead of the actual shipped rig.
        let (_, character) = rig::skinned_from_glb(
            include_bytes!("../../../assets/models/swat-parts.glb"),
            include_str!("../../../assets/models/swat-rig.json"),
            u32::try_from(meshes.len() + 1).expect("fixture mesh count fits u32"),
        )
        .expect("shipped SWAT operator must load");
        Fixture {
            meshes,
            assets: assets.expect("shipped viewmodel must load"),
            character,
        }
    })
}

#[derive(Clone, Copy, Debug)]
struct Case {
    weapon: u8,
    yaw: f32,
    pitch: f32,
    crouch: f32,
    phase: f32,
    amplitude: f32,
    origin: Vec3,
}

impl Case {
    fn pose(self, character: &RigCharacter) -> Pose {
        rig::walk_pose(
            self.phase,
            self.amplitude,
            self.crouch,
            2.3,
            &character.dims,
        )
    }

    fn aim(self) -> Vec2 {
        Vec2::new(self.yaw.cos(), self.yaw.sin())
    }

    fn position(self) -> Vec2 {
        Vec2::new(self.origin.x, self.origin.z)
    }

    fn world_point(self, point: Vec3) -> Vec3 {
        let aim = self.aim();
        self.origin + Quat::from_rotation_y(aim.x.atan2(aim.y)) * (point * grips::BODY_SCALE)
    }
}

fn each_case(mut check: impl FnMut(Case)) {
    for weapon in 1..=WEAPON_COUNT {
        for yaw in [-std::f32::consts::PI, -1.1, 0.0, 1.6, std::f32::consts::PI] {
            for pitch in [-MAX_PITCH, -0.9, 0.0, 0.9, MAX_PITCH] {
                for crouch in [0.0, 0.55, 1.0] {
                    for (phase, amplitude) in [(0.0, 0.0), (1.7, 0.8), (4.1, 1.0)] {
                        for origin in [Vec3::ZERO, Vec3::new(8.25, -2.5, -6.75)] {
                            check(Case {
                                weapon,
                                yaw,
                                pitch,
                                crouch,
                                phase,
                                amplitude,
                                origin,
                            });
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_rendered_weapon_selects_its_own_gloves_or_the_same_sidearm_fallback() {
    let Fixture { meshes, assets, .. } = fixture();
    let mut authored_meshes = HashSet::new();
    for id in 1..=WEAPON_COUNT {
        let (_, own) = assets.weapon_parts(id);
        let source = if own { id } else { SIDEARM };
        let selected = assets.grip_of(id).expect("every rendered gun has gloves");
        let authored = assets.grips.get(source).expect("selected grip is authored");
        for (actual, expected) in [
            (selected.right, authored.right),
            (selected.left, authored.left),
        ] {
            assert_eq!(
                actual.mesh, expected.mesh,
                "weapon {id} selects another gun's glove"
            );
            assert_eq!(actual.wrist, expected.wrist);
            assert_eq!(actual.palm, expected.palm);
            assert!(actual.mesh > 0 && usize::try_from(actual.mesh).unwrap() <= meshes.len());
        }
        assert_ne!(selected.right.mesh, selected.left.mesh);
        if own {
            assert!(
                authored_meshes.insert(selected.right.mesh),
                "weapon {id} reuses a right glove"
            );
            assert!(
                authored_meshes.insert(selected.left.mesh),
                "weapon {id} reuses a left glove"
            );
        }
    }
    // M4 has no authored gun today: weapon, muzzle AND hands must agree on
    // the sidearm fallback, instead of attaching an M4 grip to that mesh.
    assert!(!assets.weapon_parts(4).1);
    assert_eq!(assets.muzzle_of(4), assets.muzzle_of(SIDEARM));
    assert_eq!(
        assets.grip_of(4).unwrap().right.mesh,
        assets.grip_of(SIDEARM).unwrap().right.mesh
    );
}

#[test]
fn actual_glove_meshes_are_textured_metric_geometry_near_their_sockets() {
    let Fixture { meshes, assets, .. } = fixture();
    for id in [1, 2, 3, 5, 6, 7] {
        let grip = assets.grip_of(id).unwrap();
        for hand in [grip.right, grip.left] {
            let mesh = &meshes[usize::try_from(hand.mesh - 1).unwrap()];
            assert!(!mesh.vertices.is_empty(), "weapon {id}: empty hand");
            assert!(
                mesh.texture.is_some(),
                "weapon {id}: glove texture failed to decode"
            );
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for vertex in &mesh.vertices {
                let point = Vec3::from_array(vertex.pos);
                assert!(point.is_finite(), "weapon {id}: nonfinite glove vertex");
                min = min.min(point);
                max = max.max(point);
            }
            let extent = (max - min).max_element();
            assert!(
                (0.05..0.5).contains(&extent),
                "weapon {id}: glove extent {extent} m"
            );
            for socket in [hand.wrist, hand.palm] {
                assert!(socket.is_finite());
                let distance = socket.distance(socket.clamp(min, max));
                assert!(
                    distance < 0.04,
                    "weapon {id}: socket {socket} is {distance} m outside glove"
                );
            }
        }
    }
}

fn check_reach(case: Case, shield: bool) {
    let Fixture {
        character, assets, ..
    } = fixture();
    let grip = assets.grip_of(case.weapon).unwrap();
    let mut pose = case.pose(character);
    let before = rig::world_joints(&character.skel, &pose);
    let old_rotations = pose.local_rot;
    let old_root = pose.root_pos;
    let mount = grips::mount(
        character,
        &pose,
        grip,
        case.position(),
        case.origin.y,
        case.aim(),
        case.pitch,
        shield,
    );
    assert!(
        mount.base.is_finite() && mount.shield.is_finite() && mount.rotation.is_finite(),
        "{case:?}"
    );
    assert!(
        (mount.rotation.length_squared() - 1.0).abs() < 1e-5,
        "{case:?}"
    );
    grips::pose_arms(
        character,
        &mut pose,
        grip,
        mount,
        case.position(),
        case.origin.y,
        case.aim(),
        shield,
    );
    let after = rig::world_joints(&character.skel, &pose);
    for (side, hand) in [(ArmSide::Right, grip.right), (ArmSide::Left, grip.left)] {
        let [shoulder, elbow, wrist] = side.joints();
        let target = if shield && side == ArmSide::Left {
            let aim = case.aim();
            mount.shield + Quat::from_rotation_y(-aim.y.atan2(aim.x)) * (hand.wrist - hand.palm)
        } else {
            mount.base + mount.rotation * hand.wrist
        };
        let actual = case.world_point(after[wrist].0);
        assert!(
            actual.distance(target) < 5e-5,
            "{case:?}, shield {shield}, {side:?}: wrist {actual} missed socket {target} by {} m",
            actual.distance(target)
        );
        assert!(
            before[shoulder].0.distance(after[shoulder].0) < 1e-6,
            "shoulder moved: {case:?}"
        );
        for (a, b) in [(shoulder, elbow), (elbow, wrist)] {
            let measured = case
                .world_point(after[a].0)
                .distance(case.world_point(after[b].0));
            let expected = character.skel.joints[b].offset.length() * grips::BODY_SCALE;
            assert!(
                (measured - expected).abs() < 5e-6,
                "{case:?}, {side:?}: limb stretched from {expected} to {measured} m"
            );
        }
    }
    assert_eq!(pose.root_pos, old_root);
    for (index, rotation) in pose.local_rot.iter().enumerate() {
        assert!(
            rotation.is_finite() && (rotation.length_squared() - 1.0).abs() < 1e-5,
            "{case:?}: joint {index}"
        );
        if !ArmSide::Left.joints().contains(&index) && !ArmSide::Right.joints().contains(&index) {
            assert_eq!(
                *rotation, old_rotations[index],
                "IK changed non-arm joint {index}: {case:?}"
            );
        }
    }
}

#[test]
fn shipped_operator_reaches_every_grip_without_stretching_at_all_stances_and_aim_limits() {
    each_case(|case| {
        check_reach(case, false);
        // The shield is legal with the sidearm. Its left hand must reach the
        // upright handle, while the right hand follows the pitched gun.
        if case.weapon == SIDEARM {
            check_reach(case, true);
        }
    });
}

#[test]
fn weapon_glove_and_muzzle_use_one_metric_transform_with_the_expected_bore_direction() {
    let Fixture {
        character, assets, ..
    } = fixture();
    each_case(|case| {
        let grip = assets.grip_of(case.weapon).unwrap();
        let pose = case.pose(character);
        let mount = grips::mount(
            character,
            &pose,
            grip,
            case.position(),
            case.origin.y,
            case.aim(),
            case.pitch,
            false,
        );
        let mut frame = Frame::default();
        grips::push(&mut frame, grip, mount.base, mount.rotation, true);
        assert_eq!(frame.instances.len(), 2);
        let gun = Instance::new(mount.base, Vec3::ONE, Vec3::ONE).with_rot(mount.rotation);
        let point = |instance: Instance, local: Vec3| {
            instance.position + instance.rot * (local * instance.scale)
        };
        let aim = case.aim();
        let expected_bore = Vec3::new(
            aim.x * case.pitch.cos(),
            case.pitch.sin(),
            aim.y * case.pitch.cos(),
        );
        assert!(
            (gun.rot * Vec3::X).distance(expected_bore) < 1e-6,
            "{case:?}: wrong bore direction"
        );
        let muzzle = point(gun, assets.muzzle_of(case.weapon));
        assert!(muzzle.is_finite());
        for (instance, hand) in frame.instances.iter().zip([grip.right, grip.left]) {
            assert_eq!(instance.mesh, hand.mesh);
            assert_eq!(
                instance.scale,
                Vec3::ONE,
                "glove must not inherit body scaling"
            );
            assert_eq!(
                instance.color,
                Vec3::ONE,
                "textured gloves must not be double-tinted"
            );
            assert_eq!(instance.position, gun.position);
            assert_eq!(instance.rot, gun.rot);
            let wrist = point(*instance, hand.wrist);
            let authored_span = assets.muzzle_of(case.weapon).distance(hand.wrist);
            assert!(
                (muzzle.distance(wrist) - authored_span).abs() < 3e-6,
                "{case:?}: muzzle-to-wrist distance changed"
            );
        }
    });
}
