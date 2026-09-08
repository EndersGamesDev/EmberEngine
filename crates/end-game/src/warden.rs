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

impl WardenRig {
    pub fn load(meshes: &mut Vec<MeshData>) -> Self {
        let rig: Value =
            serde_json::from_str(include_str!("../../../assets/end-game/v5/warden-rig.json"))
                .expect("warden rig sidecar");
        let mut loaded = load_glb(include_bytes!("../../../assets/end-game/v5/warden.glb"))
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
        let flinch = (ai.flinch_left / 0.22).clamp(0.0, 1.0);
        let sleeping = ai.phase == WardenPhase::Sleeping;
        let mut joints = [Joint::IDENTITY; 20];
        let mut pelvis = Vec3::new(0.0, 0.67 + 0.31 * stand, 0.40 * (1.0 - stand));
        pelvis.y += walking * (walk * 2.0).cos() * 0.005;
        pelvis = pelvis.lerp(Vec3::new(0.0, 0.30, -0.18), dead);
        joints[0] = Joint {
            p: pelvis,
            r: Quat::from_rotation_z(walking * walk.sin() * 0.015 - dead * 0.07),
        };
        let mut lean = -0.22 * (1.0 - stand) - 0.05 * stand;
        if sleeping {
            lean += (ai.elapsed * 1.45).sin() * 0.008;
        }
        // The stand value is retained at fatal contact, so a half-risen death
        // keeps its forward weight shift instead of snapping to the idle torso.
        lean -= (stand * std::f32::consts::PI).sin() * 0.24;
        let mut twist = walking * walk.sin() * 0.035;
        if ai.phase == WardenPhase::Attacking {
            let t = ai.elapsed;
            let anticipation = ease(0.0, KNIFE_WINDUP, t);
            let contact = ease(KNIFE_WINDUP, KNIFE_CONTACT, t);
            let recovery = ease(KNIFE_FOLLOW, KNIFE_DURATION, t);
            lean += (-0.07 * anticipation - 0.13 * contact) * (1.0 - recovery);
            twist += (0.20 * anticipation - 0.38 * contact) * (1.0 - recovery);
        }
        lean += flinch * 0.16;
        lean = lean * (1.0 - dead) - 1.22 * dead;
        joints[1] = joints[0].child(
            self.parts[1].pivot - self.parts[0].pivot,
            Quat::from_rotation_x(lean) * Quat::from_rotation_y(twist),
        );
        joints[2] = joints[1].child(
            self.parts[2].pivot - self.parts[1].pivot,
            Quat::from_rotation_x(-0.10 * (1.0 - stand) - dead * 0.25),
        );
        joints[3] = joints[2].child(
            self.parts[3].pivot - self.parts[2].pivot,
            Quat::from_rotation_x(
                -0.32 * (1.0 - stand) + flinch * 0.15 - dead * 0.20
                    + if sleeping {
                        (ai.elapsed * 1.10).sin() * 0.012
                    } else {
                        0.0
                    },
            ) * Quat::from_rotation_z(if sleeping { 0.12 } else { 0.12 * (1.0 - stand) }),
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
            let ankle = Vec3::new(
                sign * 0.105,
                0.11 + walking * phase.sin().max(0.0) * 0.055,
                -walking * phase.cos() * 0.14,
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
            p: Vec3::new(0.34, 1.14, -0.25),
            r: Quat::from_rotation_z(-0.12)
                * Quat::from_rotation_y(0.65)
                * Quat::from_rotation_x(1.0),
        };
        right_ready.p.y += walking * walk.sin() * 0.022;
        let left_ready = Joint {
            p: Vec3::new(-0.32, 1.08 + walking * (-walk).sin() * 0.035, -0.16),
            r: Quat::from_rotation_x(0.22),
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
        if ai.phase == WardenPhase::Attacking {
            let wind = Joint {
                p: Vec3::new(0.42, 1.56, -0.10),
                r: Quat::from_rotation_z(1.05)
                    * Quat::from_rotation_y(0.25)
                    * Quat::from_rotation_x(0.90),
            };
            let contact = Joint {
                p: Vec3::new(0.02, 1.27, -0.47),
                r: Quat::from_rotation_z(-0.15)
                    * Quat::from_rotation_y(1.35)
                    * Quat::from_rotation_x(0.80),
            };
            let follow = Joint {
                p: Vec3::new(-0.14, 1.13, -0.21),
                r: Quat::from_rotation_z(0.75)
                    * Quat::from_rotation_y(2.35)
                    * Quat::from_rotation_x(0.80),
            };
            let t = ai.elapsed;
            targets[0] = if t < KNIFE_WINDUP {
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
            targets[1] = left_ready.mix(
                Joint {
                    p: Vec3::new(-0.34, 1.36, -0.21),
                    r: Quat::from_rotation_x(0.60),
                },
                ease(0.0, KNIFE_WINDUP, t) * (1.0 - ease(KNIFE_FOLLOW, KNIFE_DURATION, t)),
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
            let shoulder = joints[1].point(self.parts[upper].pivot - self.parts[1].pivot);
            let chain = two_bone(
                shoulder,
                targets[side].p,
                0.32,
                0.29,
                Vec3::new(sign, -0.20, 0.25),
            );
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
                        if index == KNIFE { 0.48 } else { 0.82 },
                        if index == KNIFE { 0.50 } else { 0.06 },
                    ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
