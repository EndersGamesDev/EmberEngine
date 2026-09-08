//! Articulated prison warden. Every motion is evaluated from the fixed-step AI state.
use ember_engine::{Instance, MeshData, assets::load_glb};
use end_game_core::{
    Dungeon,
    interaction::ease,
    warden::{KNIFE_CONTACT, KNIFE_DURATION, KNIFE_FOLLOW, KNIFE_WINDUP, WardenPhase},
};
use glam::{Quat, Vec3};
use serde_json::Value;

const NAMES: [&str; 20] = [
    "pelvis",
    "torso",
    "neck",
    "head",
    "upperarm_r",
    "forearm_r",
    "hand_r",
    "thigh_r",
    "shin_r",
    "boot_r",
    "coat_r",
    "upperarm_l",
    "forearm_l",
    "hand_l",
    "thigh_l",
    "shin_l",
    "boot_l",
    "coat_l",
    "chair",
    "knife",
];
const CHAIR: usize = 18;
const KNIFE: usize = 19;
const INITIAL_POSITION: Vec3 = Vec3::new(-2.9, 0.0, -3.7);

#[derive(Clone, Copy, Debug)]
struct Joint {
    p: Vec3,
    r: Quat,
}
impl Joint {
    const IDENTITY: Self = Self {
        p: Vec3::ZERO,
        r: Quat::IDENTITY,
    };
    fn point(self, p: Vec3) -> Vec3 {
        self.p + self.r * p
    }
    fn child(self, offset: Vec3, r: Quat) -> Self {
        Self {
            p: self.point(offset),
            r: self.r * r,
        }
    }
    fn mix(self, other: Self, t: f32) -> Self {
        Self {
            p: self.p.lerp(other.p, t),
            r: self.r.slerp(other.r, t),
        }
    }
}
struct Part {
    mesh: u32,
    pivot: Vec3,
}
pub struct WardenRig {
    parts: Vec<Part>,
}

#[derive(Clone, Copy, Debug)]
struct Chain {
    middle: Vec3,
    end: Vec3,
    #[cfg(test)]
    reached: bool,
}
fn two_bone(start: Vec3, target: Vec3, upper: f32, lower: f32, pole: Vec3) -> Chain {
    let delta = target - start;
    let distance = delta.length();
    let direction = delta.try_normalize().unwrap_or(-Vec3::Y);
    let length = distance.clamp((upper - lower).abs() + 0.0001, upper + lower - 0.0001);
    let along = (upper * upper - lower * lower + length * length) / (2.0 * length);
    let bend = (upper * upper - along * along).max(0.0).sqrt();
    let projected = pole - direction * pole.dot(direction);
    let perpendicular = projected
        .try_normalize()
        .unwrap_or_else(|| direction.cross(Vec3::X).normalize_or_zero());
    Chain {
        middle: start + direction * along + perpendicular * bend,
        end: start + direction * length,
        #[cfg(test)]
        reached: (distance - length).abs() < 0.001,
    }
}
fn bone(start: Vec3, end: Vec3, roll: Quat) -> Joint {
    let direction = (end - start).normalize_or_zero();
    Joint {
        p: start,
        r: Quat::from_rotation_arc(roll * -Vec3::Y, direction) * roll,
    }
}
fn read_vec(value: &Value) -> Vec3 {
    Vec3::new(
        value[0].as_f64().unwrap() as f32,
        value[1].as_f64().unwrap() as f32,
        value[2].as_f64().unwrap() as f32,
    )
}

/// The core advances one gait cycle per 1.2 metres. During stance, local Z
/// moves backwards by exactly the body's forward travel, planting the sole in
/// world space. The returning foot lifts with zero vertical speed at contact.
fn gait_foot(phase: f32) -> (f32, f32) {
    const STRIDE: f32 = 1.2;
    const STANCE: f32 = 0.55;
    let cycle = phase.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    let reach = STRIDE * STANCE * 0.5;
    if cycle <= STANCE {
        (-reach + STRIDE * cycle, 0.0)
    } else {
        let t = (cycle - STANCE) / (1.0 - STANCE);
        let t2 = t * t;
        let t3 = t2 * t;
        let tangent = STRIDE * (1.0 - STANCE);
        let z = (2.0 * t3 - 3.0 * t2 + 1.0) * reach + (t3 - 2.0 * t2 + t) * tangent
            - (-2.0 * t3 + 3.0 * t2) * reach
            + (t3 - t2) * tangent;
        (z, (std::f32::consts::PI * t).sin().powi(2) * 0.075)
    }
}

impl WardenRig {
    pub fn load(meshes: &mut Vec<MeshData>) -> Self {
        let rig: Value =
            serde_json::from_str(include_str!("../../../assets/end-game/v7/warden-rig.json"))
                .expect("warden rig sidecar");
        let mut loaded = load_glb(include_bytes!("../../../assets/end-game/v7/warden.glb"))
            .expect("articulated warden GLB");
        let rows = rig["parts"].as_array().unwrap();
        let parts = NAMES
            .iter()
            .map(|suffix| {
                let name = format!("warden_{suffix}");
                let index = loaded
                    .iter()
                    .position(|part| part.name == name)
                    .expect("warden node exactly named");
                let part = loaded.remove(index);
                assert!(part.mesh.texture.is_some(), "warden RGB8 texture decoded");
                let row = rows
                    .iter()
                    .find(|row| row["name"] == name)
                    .expect("warden pivot");
                meshes.push(part.mesh);
                Part {
                    mesh: meshes.len() as u32,
                    pivot: read_vec(&row["pivot"]),
                }
            })
            .collect();
        assert!(loaded.is_empty(), "unmapped warden meshes");
        Self { parts }
    }

    fn pose(&self, game: &Dungeon) -> ([Joint; 20], [Chain; 4]) {
        let ai = &game.warden_ai;
        let stand = ai.stand_amount();
        let draw = ai.knife_draw();
        let dead = if ai.phase == WardenPhase::Dead {
            ease(0.0, 1.05, ai.elapsed)
        } else {
            0.0
        };
        let walking = ai.walk_blend * (1.0 - dead);
        let walk = ai.walk_phase;
        let flinch = ai.flinch_amount();
        let reaction = ai.reaction_pose().map(|mut r| {
            if ai.phase == WardenPhase::Dead {
                r.elapsed += ai.elapsed;
            }
            r
        });
        let response = super::marks::response(reaction, ai.yaw, 1. - dead);
        let doze = 1.0 - stand;
        let breath = (game.time * 1.35).sin();
        let (anticipation, contact, recovery, attack_weight) =
            ai.attack_pose()
                .map_or((0.0, 0.0, 0.0, 0.0), |(t, weight)| {
                    (
                        ease(0.0, KNIFE_WINDUP, t),
                        ease(KNIFE_WINDUP, KNIFE_CONTACT, t),
                        ease(KNIFE_FOLLOW, KNIFE_DURATION, t),
                        weight,
                    )
                });
        let loaded = anticipation * (1.0 - recovery) * attack_weight;
        let driven = contact * (1.0 - recovery) * attack_weight;
        let mut joints = [Joint::IDENTITY; 20];
        // Bring weight over the planted boots before lifting clear of the chair.
        let mut pelvis = Vec3::new(
            0.012 * stand,
            0.67 + 0.31 * ease(0.10, 1.0, stand),
            0.40 * (1.0 - ease(0.0, 0.85, stand)),
        );
        pelvis.x += walking * walk.sin() * 0.022 + loaded * 0.018 - driven * 0.025;
        pelvis.y += breath * 0.002 * stand
            - walking * (0.060 + 0.005 * (1.0 + (walk * 2.0).cos()))
            - loaded * 0.015;
        pelvis.z += loaded * 0.018 - driven * 0.040;
        pelvis = pelvis.lerp(Vec3::new(0.0, 0.30, -0.18), dead);
        pelvis += response.hip;
        joints[0] = Joint {
            p: pelvis,
            r: Quat::from_rotation_y(-walking * walk.sin() * 0.025)
                * Quat::from_rotation_z(-walking * walk.sin() * 0.020 - dead * 0.07),
        };
        let mut lean = -0.22 * doze - 0.09 * stand - walking * 0.025;
        lean += breath * (0.007 + doze * 0.004);
        // The stand value is retained at fatal contact, so a half-risen death
        // keeps its forward weight shift instead of snapping to the idle torso.
        lean -= (stand * std::f32::consts::PI).sin() * 0.28;
        let mut twist = walking * walk.sin() * 0.060;
        lean += loaded * 0.045 - driven * 0.17;
        twist += -loaded * 0.14 + driven * 0.36;
        lean += flinch * 0.16;
        lean = lean * (1.0 - dead) - 1.22 * dead;
        twist *= 1.0 - dead;
        joints[1] = joints[0].child(
            self.parts[1].pivot - self.parts[0].pivot,
            Quat::from_rotation_x(lean)
                * Quat::from_rotation_y(twist)
                * Quat::from_rotation_z(walking * walk.sin() * 0.015),
        );
        let chest_rotation = joints[1].r;
        joints[1].r *= super::marks::response_rotation(response.torso);
        let response_rotation = joints[1].r * chest_rotation.conjugate();
        joints[2] = joints[1].child(
            self.parts[2].pivot - self.parts[1].pivot,
            Quat::from_rotation_y(-twist * 0.55)
                * Quat::from_rotation_x(-0.10 * doze + 0.035 * stand - dead * 0.285),
        );
        joints[3] = joints[2].child(
            self.parts[3].pivot - self.parts[2].pivot,
            Quat::from_rotation_x(
                -0.32 * doze - stand * 0.035 + flinch * 0.15 - dead * 0.165
                    + (game.time * 0.78).sin() * 0.026 * doze * (1.0 - dead),
            ) * Quat::from_rotation_z(0.12 * doze * (1.0 - dead))
                * super::marks::response_rotation(response.head),
        );

        // Boots remain planted throughout the rise. Only a walking swing foot lifts.
        let empty = Chain {
            middle: Vec3::ZERO,
            end: Vec3::ZERO,
            #[cfg(test)]
            reached: false,
        };
        let mut chains = [empty; 4];
        for (side, thigh, shin, boot, coat) in [(0, 7, 8, 9, 10), (1, 14, 15, 16, 17)] {
            let sign = if side == 0 { 1.0 } else { -1.0 };
            let phase = walk + if side == 0 { 0.0 } else { std::f32::consts::PI };
            let (step_z, step_lift) = gait_foot(phase);
            let ankle = Vec3::new(
                self.parts[boot].pivot.x + sign * 0.020,
                self.parts[boot].pivot.y + walking * step_lift,
                walking * step_z,
            );
            let hip = joints[0].point(self.parts[thigh].pivot - self.parts[0].pivot);
            let chain = two_bone(hip, ankle, 0.43, 0.43, -Vec3::Z);
            joints[thigh] = bone(hip, chain.middle, Quat::IDENTITY);
            joints[shin] = bone(chain.middle, chain.end, Quat::IDENTITY);
            joints[boot] = Joint {
                p: chain.end,
                r: Quat::IDENTITY,
            };
            joints[coat] = Joint {
                p: hip,
                r: joints[thigh].r,
            };
            chains[side + 2] = chain;
        }

        let sheath = joints[0].child(
            Vec3::new(0.20, 0.01, 0.01),
            Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2 + dead * 0.85),
        );
        let socket = self.parts[KNIFE].pivot - self.parts[6].pivot;
        let sheath_wrist = Joint {
            p: sheath.p - sheath.r * socket,
            r: sheath.r,
        };
        let mut right_ready = Joint {
            p: Vec3::new(0.32, 1.18, -0.28),
            r: Quat::from_rotation_z(-0.12)
                * Quat::from_rotation_y(0.65)
                * Quat::from_rotation_x(1.0),
        };
        right_ready.p.y += walking * walk.sin() * 0.015;
        right_ready.p.z += walking * walk.cos() * 0.045;
        let left_ready = Joint {
            p: Vec3::new(
                -0.34,
                1.17 - walking * walk.sin() * 0.025,
                -0.20 - walking * walk.cos() * 0.065,
            ),
            r: Quat::from_rotation_x(0.35) * Quat::from_rotation_z(-0.10),
        };
        let mut targets = [right_ready, left_ready];
        if stand < 1.0 || draw < 1.0 {
            for side in 0..2 {
                let sign = if side == 0 { 1.0 } else { -1.0 };
                let rest = Vec3::new(sign * 0.22, 0.735, 0.12);
                let planted = chains[side + 2].middle + Vec3::new(sign * 0.065, 0.080, -0.025);
                let plant = ease(0.0, 0.30, stand);
                targets[side] = Joint {
                    p: rest.lerp(planted, plant),
                    r: Quat::from_rotation_x(0.60),
                };
            }
            // Hands release the knees as the hips rise; retaining the plant until
            // knife draw would require longer arms in the middle of the stand.
            targets[1] = targets[1].mix(left_ready, ease(0.08, 0.65, stand));
            targets[0] = targets[0].mix(sheath_wrist, ease(0.08, 0.50, stand));
            targets[0] = if draw <= 0.18 {
                targets[0].mix(sheath_wrist, ease(0.0, 0.18, draw))
            } else {
                sheath_wrist.mix(right_ready, ease(0.18, 1.0, draw))
            };
        }
        if let Some((t, weight)) = ai.attack_pose() {
            let wind = Joint {
                p: Vec3::new(0.42, 1.56, -0.10),
                r: Quat::from_rotation_z(1.05)
                    * Quat::from_rotation_y(0.25)
                    * Quat::from_rotation_x(0.90),
            };
            let contact = Joint {
                p: Vec3::new(0.02, 1.27, -0.50),
                r: Quat::from_rotation_z(-0.15)
                    * Quat::from_rotation_y(1.35)
                    * Quat::from_rotation_x(0.80),
            };
            let follow = Joint {
                p: Vec3::new(-0.17, 1.12, -0.24),
                r: Quat::from_rotation_z(0.75)
                    * Quat::from_rotation_y(2.35)
                    * Quat::from_rotation_x(0.80),
            };
            let attack = if t < KNIFE_WINDUP {
                right_ready.mix(wind, ease(0.0, KNIFE_WINDUP * 0.80, t))
            } else if t < KNIFE_CONTACT {
                let u = ((t - KNIFE_WINDUP) / (KNIFE_CONTACT - KNIFE_WINDUP)).clamp(0.0, 1.0);
                wind.mix(contact, u * u)
            } else if t < KNIFE_FOLLOW {
                let u = ((t - KNIFE_CONTACT) / (KNIFE_FOLLOW - KNIFE_CONTACT)).clamp(0.0, 1.0);
                contact.mix(follow, 1.0 - (1.0 - u).powi(2))
            } else {
                follow.mix(right_ready, ease(KNIFE_FOLLOW, KNIFE_DURATION, t))
            };
            targets[0] = right_ready.mix(attack, weight);
            targets[1] = left_ready.mix(
                Joint {
                    p: Vec3::new(-0.34, 1.36, -0.21),
                    r: Quat::from_rotation_x(0.60),
                },
                ease(0.0, KNIFE_WINDUP, t) * (1.0 - ease(KNIFE_FOLLOW, KNIFE_DURATION, t)) * weight,
            );
        }
        for (side, upper, forearm, hand) in [(0, 4, 5, 6), (1, 11, 12, 13)] {
            let sign = if side == 0 { 1.0 } else { -1.0 };
            targets[side].p += Vec3::new(sign * flinch * 0.025, flinch * 0.03, flinch * 0.025);
            targets[side] = targets[side].mix(
                Joint {
                    p: Vec3::new(sign * 0.26, 0.17, -0.47),
                    r: Quat::IDENTITY,
                },
                dead,
            );
            targets[side].p =
                joints[1].p + response_rotation * (targets[side].p + response.hip - joints[1].p);
            targets[side].r = response_rotation * targets[side].r;
            let shoulder = joints[1].point(self.parts[upper].pivot - self.parts[1].pivot);
            // Transport an outward bend from a downward reference arm. Unlike
            // a fixed world pole, it cannot become parallel to the arm while a
            // raised knife hand falls past its shoulder during an interruption.
            let direction = (targets[side].p - shoulder).normalize_or_zero();
            let pole = Quat::from_rotation_arc(-Vec3::Y, direction) * Vec3::new(sign, 0.0, 0.22);
            let chain = two_bone(shoulder, targets[side].p, 0.32, 0.29, pole);
            joints[upper] = bone(shoulder, chain.middle, targets[side].r);
            joints[forearm] = bone(chain.middle, chain.end, targets[side].r);
            joints[hand] = Joint {
                p: chain.end,
                r: targets[side].r,
            };
            chains[side] = chain;
        }
        // Before grasp the blade remains in its belt sheath; at grasp both transforms coincide.
        joints[KNIFE] = if draw < 0.18 {
            sheath
        } else {
            joints[6].child(socket, Quat::IDENTITY)
        };
        (joints, chains)
    }

    pub fn draw(&self, out: &mut Vec<Instance>, game: &Dungeon) {
        let (joints, _) = self.pose(game);
        let character = Joint {
            p: Vec3::new(game.warden.x, 0.0, game.warden.y),
            r: Quat::from_rotation_y(-game.warden_ai.yaw),
        };
        let chair = Joint {
            p: INITIAL_POSITION,
            r: Quat::from_rotation_y(std::f32::consts::PI),
        };
        for (index, part) in self.parts.iter().enumerate() {
            let joint = if index == CHAIR {
                chair
            } else {
                character.child(joints[index].p, joints[index].r)
            };
            out.push(
                Instance::new(joint.p - joint.r * part.pivot, Vec3::ONE, Vec3::ONE)
                    .with_mesh(part.mesh)
                    .with_rot(joint.r)
                    .with_surface(
                        match index {
                            KNIFE => 0.46,
                            2 | 3 => 0.92,           // skin
                            6 | 9 | 13 | 16 => 0.87, // gloves and boots
                            CHAIR => 0.90,
                            _ => 0.96, // worn coat and trousers
                        },
                        if index == KNIFE { 0.60 } else { 0.0 },
                    ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_same_pose(before: &[Joint; 20], after: &[Joint; 20]) {
        for (index, (a, b)) in before.iter().zip(after).enumerate() {
            assert!(
                a.p.abs_diff_eq(b.p, 0.0001),
                "{} position: {:?} -> {:?}",
                NAMES[index],
                a.p,
                b.p
            );
            assert!(
                a.r.abs_diff_eq(b.r, 0.0001),
                "{} rotation: {:?} -> {:?}",
                NAMES[index],
                a.r,
                b.r
            );
        }
    }

    #[test]
    fn stance_feet_cancel_the_body_travel_and_swing_contact_is_continuous() {
        let rig = WardenRig::load(&mut Vec::new());
        for (boot, start) in [(9, 0.0), (16, 0.6)] {
            let mut anchor = None;
            for step in 0..=30 {
                let distance = start + step as f32 * 0.02;
                let mut game = Dungeon::default();
                game.warden_ai.phase = WardenPhase::Hunting;
                game.warden_ai.yaw = 0.0;
                game.warden_ai.walk_blend = 1.0;
                game.warden_ai.walk_phase = distance / 1.2 * std::f32::consts::TAU;
                game.warden = glam::Vec2::new(0.0, -distance);
                let (joints, chains) = rig.pose(&game);
                assert!(chains[if boot == 9 { 2 } else { 3 }].reached);
                let world = Vec3::new(game.warden.x, 0.0, game.warden.y) + joints[boot].p;
                let planted = *anchor.get_or_insert(world);
                assert!(
                    world.abs_diff_eq(planted, 0.00001),
                    "stance sole slides: {planted:?} -> {world:?}"
                );
            }
        }
        for phase in [0.0, 0.55 * std::f32::consts::TAU, std::f32::consts::TAU] {
            let a = gait_foot(phase - 0.00001);
            let b = gait_foot(phase + 0.00001);
            assert!((a.0 - b.0).abs() < 0.00001);
            assert!((a.1 - b.1).abs() < 0.00001);
        }
    }

    #[test]
    fn rise_transfers_weight_before_lifting_and_keeps_both_soles_planted() {
        let rig = WardenRig::load(&mut Vec::new());
        let mut game = Dungeon::default();
        let (mut previous, _) = rig.pose(&game);
        let planted = [previous[9].p, previous[16].p];
        let initial_hip = previous[0].p;
        game.warden_ai.phase = WardenPhase::Waking;
        for tick in 0..=99 {
            game.time = tick as f32 * end_game_core::STEP;
            game.warden_ai.elapsed = game.time;
            let (joints, chains) = rig.pose(&game);
            for chain in chains {
                assert!(chain.reached, "unreachable rise at tick {tick}");
            }
            assert!(joints[9].p.abs_diff_eq(planted[0], 0.00001));
            assert!(joints[16].p.abs_diff_eq(planted[1], 0.00001));
            for (index, (a, b)) in previous.iter().zip(joints).enumerate() {
                assert!(
                    a.p.distance(b.p) < 0.08,
                    "{} jumps while rising at tick {tick}",
                    NAMES[index]
                );
            }
            if tick == 24 {
                assert!(initial_hip.z - joints[0].p.z > 3.0 * (joints[0].p.y - initial_hip.y));
            }
            previous = joints;
        }
        game.warden_ai.phase = WardenPhase::Hunting;
        game.warden_ai.elapsed = 0.0;
        assert_same_pose(&previous, &rig.pose(&game).0);
    }

    #[test]
    fn authored_contact_is_the_single_forward_knife_commit_and_returns_to_guard() {
        let rig = WardenRig::load(&mut Vec::new());
        let mut game = Dungeon::default();
        game.warden_ai.phase = WardenPhase::Hunting;
        let idle = rig.pose(&game).0;
        game.warden_ai.phase = WardenPhase::Attacking;
        assert_same_pose(&idle, &rig.pose(&game).0);
        game.warden_ai.elapsed = KNIFE_CONTACT;
        let contact = rig.pose(&game).0;
        assert!(
            contact[6]
                .p
                .abs_diff_eq(Vec3::new(0.02, 1.27, -0.50), 0.0001)
        );
        for tick in 0..=86 {
            game.warden_ai.elapsed = tick as f32 * end_game_core::STEP;
            let (joints, chains) = rig.pose(&game);
            assert!(chains[0].reached && chains[1].reached);
            assert!(
                joints[6].p.z >= contact[6].p.z - 0.0001,
                "an extra forward strike after/before authored contact"
            );
        }
        game.warden_ai.elapsed = KNIFE_DURATION;
        assert_same_pose(&idle, &rig.pose(&game).0);
    }

    #[test]
    fn walking_boot_geometry_never_enters_the_floor_and_stops_at_the_same_phase() {
        let mut meshes = Vec::new();
        let rig = WardenRig::load(&mut meshes);
        for tick in 0..=60 {
            let mut game = Dungeon::default();
            game.warden_ai.phase = WardenPhase::Hunting;
            game.warden_ai.walk_phase = tick as f32 / 60.0 * std::f32::consts::TAU;
            game.warden_ai.walk_blend = 1.0;
            let (joints, chains) = rig.pose(&game);
            for chain in chains {
                assert!(chain.reached);
            }
            for boot in [9, 16] {
                let mesh = &meshes[rig.parts[boot].mesh as usize - 1];
                for vertex in &mesh.vertices {
                    let p =
                        joints[boot].point(Vec3::from_array(vertex.pos) - rig.parts[boot].pivot);
                    assert!(
                        p.y >= -0.001,
                        "boot crosses floor at gait tick {tick}: {p:?}"
                    );
                }
            }
            let phase = game.warden_ai.walk_phase;
            let mut previous = joints;
            for stop in 1..=12 {
                game.warden_ai.walk_blend = (1.0 - stop as f32 / 12.0).max(0.0);
                let current = rig.pose(&game).0;
                for boot in [9, 16] {
                    assert!(previous[boot].p.distance(current[boot].p) < 0.035);
                }
                assert_eq!(game.warden_ai.walk_phase, phase);
                previous = current;
            }
            assert_eq!(previous[9].p.z, 0.0);
            assert_eq!(previous[16].p.z, 0.0);
        }
    }

    #[test]
    fn death_from_a_weighted_stride_preserves_the_pose_and_plants_the_lifted_foot() {
        let mut meshes = Vec::new();
        let rig = WardenRig::load(&mut meshes);
        let knife = &meshes[rig.parts[KNIFE].mesh as usize - 1];
        for phase in [0.0, 0.25, 0.5, 0.75] {
            let mut game = Dungeon::default();
            game.time = 3.1;
            game.warden_ai.phase = WardenPhase::Hunting;
            game.warden_ai.walk_phase = phase * std::f32::consts::TAU;
            game.warden_ai.walk_blend = 1.0;
            let before = rig.pose(&game).0;
            game.warden_ai.die();
            assert_same_pose(&before, &rig.pose(&game).0);
            for tick in 1..=64 {
                let elapsed = tick as f32 * end_game_core::STEP;
                game.time = 3.1 + elapsed;
                game.warden_ai.elapsed = elapsed;
                game.warden_ai.walk_blend = (1.0 - elapsed * 5.0).max(0.0);
                let (joints, chains) = rig.pose(&game);
                for chain in chains {
                    assert!(chain.reached, "stride death {phase}, tick {tick}");
                }
                for boot in [9, 16] {
                    assert!(joints[boot].p.y >= rig.parts[boot].pivot.y - 0.0001);
                }
                for vertex in &knife.vertices {
                    let point =
                        joints[KNIFE].point(Vec3::from_array(vertex.pos) - rig.parts[KNIFE].pivot);
                    assert!(
                        point.y >= -0.001,
                        "stride-death knife below floor: {point:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn knife_interruptions_start_at_the_struck_pose_then_settle_without_detaching() {
        let mut meshes = Vec::new();
        let rig = WardenRig::load(&mut meshes);
        let knife = &meshes[rig.parts[KNIFE].mesh as usize - 1];
        for time in [KNIFE_WINDUP, KNIFE_CONTACT] {
            for killed in [false, true] {
                let mut game = Dungeon::default();
                game.warden_ai.phase = WardenPhase::Attacking;
                game.warden_ai.elapsed = time;
                let before = rig.pose(&game).0;
                game.warden_ai.on_sword_hit(true, killed);
                assert_same_pose(&before, &rig.pose(&game).0);
                let mut previous = before;
                let duration = if killed {
                    1.05
                } else {
                    end_game_core::warden::STAGGER_DURATION
                };
                let ticks = (duration / end_game_core::STEP).ceil() as u32;
                for tick in 1..=ticks {
                    let elapsed = tick as f32 * end_game_core::STEP;
                    game.warden_ai.elapsed = elapsed;
                    game.warden_ai.flinch_left =
                        (end_game_core::warden::FLINCH_DURATION - elapsed).max(0.0);
                    let (joints, chains) = rig.pose(&game);
                    for chain in chains {
                        assert!(
                            chain.reached,
                            "interrupted {time}, killed {killed}, tick {tick}"
                        );
                    }
                    for (index, (a, b)) in previous.iter().zip(joints).enumerate() {
                        assert!(
                            a.p.distance(b.p) < 0.09,
                            "{} jumps at interrupted attack {time}, killed {killed}, tick {tick}: {:?} -> {:?}",
                            NAMES[index],
                            a.p,
                            b.p
                        );
                    }
                    let socket = joints[6].point(rig.parts[KNIFE].pivot - rig.parts[6].pivot);
                    assert!(socket.distance(joints[KNIFE].p) < 0.0001);
                    for vertex in &knife.vertices {
                        let p = joints[KNIFE]
                            .point(Vec3::from_array(vertex.pos) - rig.parts[KNIFE].pivot);
                        assert!(p.y >= -0.001, "interrupted knife below ground: {p:?}");
                    }
                    previous = joints;
                }
                let mut settled = Dungeon::default();
                settled.warden_ai.phase = WardenPhase::Hunting;
                if killed {
                    settled.warden_ai.die();
                    settled.warden_ai.elapsed = duration;
                }
                assert_same_pose(&rig.pose(&settled).0, &rig.pose(&game).0);
            }
        }
    }

    #[test]
    fn fatal_hit_during_stagger_keeps_the_partially_recovered_pose() {
        let rig = WardenRig::load(&mut Vec::new());
        for time in [KNIFE_WINDUP, KNIFE_CONTACT] {
            let mut game = Dungeon::default();
            game.warden_ai.phase = WardenPhase::Attacking;
            game.warden_ai.elapsed = time;
            game.warden_ai.on_sword_hit(true, false);
            game.warden_ai.elapsed = 0.13;
            game.warden_ai.flinch_left = end_game_core::warden::FLINCH_DURATION - 0.13;
            let before = rig.pose(&game).0;
            game.warden_ai.on_sword_hit(false, true);
            assert_same_pose(&before, &rig.pose(&game).0);
        }
    }

    #[test]
    fn collapse_preserves_partial_rise_and_keeps_the_knife_above_ground() {
        let mut meshes = Vec::new();
        let rig = WardenRig::load(&mut meshes);
        let knife = &meshes[rig.parts[KNIFE].mesh as usize - 1];
        for (start_phase, start_time) in [
            (WardenPhase::Sleeping, 0.0),
            (WardenPhase::Waking, 0.4),
            (WardenPhase::Waking, 0.9),
            (WardenPhase::Hunting, 0.0),
            (WardenPhase::Attacking, KNIFE_WINDUP),
            (WardenPhase::Attacking, KNIFE_CONTACT),
        ] {
            let mut game = Dungeon::default();
            game.warden_ai.phase = start_phase;
            game.warden_ai.elapsed = start_time;
            let before = rig.pose(&game).0;
            game.warden_ai.die();
            let first = rig.pose(&game).0;
            for (a, b) in before.iter().zip(&first) {
                assert!(a.p.abs_diff_eq(b.p, 0.0001));
                assert!(a.r.abs_diff_eq(b.r, 0.0001));
            }
            for tick in 0..=110 {
                game.warden_ai.elapsed = tick as f32 / 60.0;
                let (joints, chains) = rig.pose(&game);
                for chain in chains {
                    assert!(
                        chain.reached,
                        "collapse target {start_phase:?} at {start_time}, tick {tick}"
                    );
                }
                for boot in [9, 16] {
                    assert!((joints[boot].p.y - 0.11).abs() < 0.0001);
                }
                for vertex in &knife.vertices {
                    let p =
                        joints[KNIFE].point(Vec3::from_array(vertex.pos) - rig.parts[KNIFE].pivot);
                    assert!(
                        p.y >= -0.001,
                        "knife below ground {start_phase:?} at {start_time}, tick {tick}: {p:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_phase_keeps_connected_limbs_and_grounded_boots() {
        let rig = WardenRig::load(&mut Vec::new());
        for phase in [
            WardenPhase::Sleeping,
            WardenPhase::Waking,
            WardenPhase::Hunting,
            WardenPhase::Attacking,
            WardenPhase::Staggered,
            WardenPhase::Dead,
        ] {
            for tick in 0..=110 {
                let mut game = Dungeon::default();
                if phase == WardenPhase::Dead {
                    game.warden_ai.phase = WardenPhase::Hunting;
                    game.warden_ai.die();
                } else {
                    game.warden_ai.phase = phase;
                }
                game.warden_ai.elapsed = tick as f32 / 60.0;
                game.warden_ai.walk_phase = tick as f32 * 0.13;
                game.warden_ai.walk_blend = if phase == WardenPhase::Hunting {
                    1.0
                } else {
                    0.0
                };
                let (joints, chains) = rig.pose(&game);
                for (side, upper, forearm, hand) in [(0, 4, 5, 6), (1, 11, 12, 13)] {
                    assert!(
                        chains[side].reached,
                        "arm target unreachable {phase:?} tick {tick} side {side}"
                    );
                    assert!((joints[upper].p.distance(joints[forearm].p) - 0.32).abs() < 0.0001);
                    assert!((joints[forearm].p.distance(joints[hand].p) - 0.29).abs() < 0.0001);
                }
                for (side, thigh, shin, boot) in [(0, 7, 8, 9), (1, 14, 15, 16)] {
                    assert!(
                        chains[side + 2].reached,
                        "leg target unreachable {phase:?} tick {tick}"
                    );
                    assert!((joints[thigh].p.distance(joints[shin].p) - 0.43).abs() < 0.0001);
                    assert!((joints[shin].p.distance(joints[boot].p) - 0.43).abs() < 0.0001);
                    assert!(joints[boot].p.y >= 0.1099);
                    if phase != WardenPhase::Hunting {
                        assert!((joints[boot].p.y - 0.11).abs() < 0.0001);
                    }
                }
                if game.warden_ai.knife_draw() >= 0.18 {
                    let socket = joints[6].point(rig.parts[KNIFE].pivot - rig.parts[6].pivot);
                    assert!(socket.distance(joints[KNIFE].p) < 0.0001);
                }
                let mut instances = Vec::new();
                rig.draw(&mut instances, &game);
                assert_eq!(instances.len(), 20);
                assert!(
                    instances
                        .iter()
                        .all(|p| p.position.is_finite() && p.rot.is_finite())
                );
                assert!(
                    instances[CHAIR]
                        .position
                        .abs_diff_eq(INITIAL_POSITION, 1e-6)
                );
            }
        }
    }
    #[test]
    fn anatomical_response_keeps_warden_grips_and_death_origin_continuous() {
        use end_game_core::{HitReaction, HitZone};
        let rig = WardenRig::load(&mut Vec::new());
        for zone in [
            HitZone::Head,
            HitZone::LeftTorso,
            HitZone::RightTorso,
            HitZone::LeftLeg,
            HitZone::RightLeg,
        ] {
            let mut game = Dungeon::default();
            game.warden_ai.phase = WardenPhase::Attacking;
            game.warden_ai.elapsed = KNIFE_WINDUP;
            let original = rig.pose(&game).0;
            game.warden_ai.reaction = Some(HitReaction {
                id: 1,
                time: 0.,
                zone,
                point: Vec3::Y,
                direction: Vec3::new(-0.68, -0.73, 0.).normalize(),
                strength: 0.85,
                elapsed: 0.,
                origin: [Vec3::ZERO; 5],
            });
            let hit = rig.pose(&game).0;
            for (a, b) in original.iter().zip(hit) {
                assert!(a.p.distance(b.p) < 0.00001 && a.r.abs_diff_eq(b.r, 0.00001));
            }
            for tick in 0..=42 {
                game.warden_ai.reaction.as_mut().unwrap().elapsed = tick as f32 / 120.;
                let (joints, chains) = rig.pose(&game);
                assert!(chains.iter().all(|c| c.reached), "{zone:?} t{tick}");
                for boot in [9, 16] {
                    assert!((joints[boot].p.y - rig.parts[boot].pivot.y).abs() < 0.0001);
                }
            }
            game.warden_ai.reaction.as_mut().unwrap().elapsed = 0.06;
            let before = rig.pose(&game).0;
            let origin = game.warden_ai.reaction.unwrap().vectors();
            game.warden_ai.reaction = Some(HitReaction {
                id: 2,
                time: 0.06,
                zone: HitZone::Head,
                point: Vec3::Y,
                direction: Vec3::X,
                strength: 0.85,
                elapsed: 0.,
                origin,
            });
            game.warden_ai.on_sword_hit(true, true);
            let after = rig.pose(&game).0;
            for (a, b) in before.iter().zip(after) {
                assert!(
                    a.p.distance(b.p) < 0.00001 && a.r.abs_diff_eq(b.r, 0.00001),
                    "warden {zone:?} fatal response origin"
                );
            }
        }
    }
}
