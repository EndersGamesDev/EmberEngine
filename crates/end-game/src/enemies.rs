//! Reused articulated castle rigs; every pose follows the fixed-step encounter clock.
//!
//! The generated bodies use standing bind-space vertices at 1.865 m. Their
//! sidecar pivots are retargeted with rigid two-bone limbs, then the complete
//! character is scaled to the core's physical height and rotated by Ry(-yaw).
//! This is rigid articulation of baked generated surfaces, not deforming skin.
//! The soldier mesh IDs are shared by both weapon variants and every instance.
//!
//! Full-blend, straight walking cancels actual core stride distance during
//! stance; turns and starting/stopping blend that planted pose. Weapon grips,
//! interruptions and slam warnings use the authored core contact clock. The
//! primary hand retains its measured weapon socket throughout a fall, while
//! the support hand releases and the actual weapon vertices settle above floor.
use ember_engine::{Frame, Instance, MeshData, Particle, assets::load_glb};
use end_game_core::{
    Dungeon,
    enemies::{Enemy, EnemyAttack, EnemyKind, EnemyPhase},
    interaction::ease,
};
use glam::{Mat3, Quat, Vec2, Vec3};
use serde_json::Value;

const BODY: [&str; 18] = [
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
];
const HAND_SOCKET: Vec3 = Vec3::new(0., -0.068, -0.012);
const BIND_HEIGHT: f32 = 1.865;
const FALL_TIME: f32 = 1.35;
const WEAPON_ASSETS: [(&str, &[u8]); 4] = [
    (
        "sword",
        include_bytes!("../../../assets/end-game/v10/weapon-sword.glb"),
    ),
    (
        "spear",
        include_bytes!("../../../assets/end-game/v10/weapon-spear.glb"),
    ),
    (
        "axe",
        include_bytes!("../../../assets/end-game/v10/weapon-axe.glb"),
    ),
    (
        "maul",
        include_bytes!("../../../assets/end-game/v10/weapon-maul.glb"),
    ),
];

fn weapon_metadata() -> Value {
    serde_json::from_str(include_str!("../../../assets/end-game/v10/weapons.json"))
        .expect("measured weapon metadata")
}

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
    fn child(self, p: Vec3, r: Quat) -> Self {
        Self {
            p: self.point(p),
            r: self.r * r,
        }
    }
}
struct Part {
    mesh: u32,
    pivot: Vec3,
    floor_points: Vec<Vec3>,
}
struct Weapon {
    name: String,
    mesh: u32,
    support: Option<Vec3>,
    tip: Vec3,
    min: Vec3,
    floor_points: Vec<Vec3>,
}
struct Rig {
    parts: Vec<Part>,
    weapons: Vec<Weapon>,
    eye: Option<Vec3>,
}
pub struct EnemyScene {
    rigs: [Rig; 3],
}

#[derive(Clone, Copy, Debug)]
struct Chain {
    middle: Vec3,
    end: Vec3,
    // Geometry tests inspect reachability across the full animation timeline.
    #[cfg_attr(not(test), allow(dead_code))]
    reached: bool,
}
fn two_bone(start: Vec3, target: Vec3, upper: f32, lower: f32, pole: Vec3) -> Chain {
    let delta = target - start;
    let distance = delta.length();
    let direction = delta.try_normalize().unwrap_or(-Vec3::Y);
    let length = distance.clamp((upper - lower).abs() + 0.0001, upper + lower - 0.0001);
    let along = (upper * upper - lower * lower + length * length) / (2. * length);
    let bend = (upper * upper - along * along).max(0.).sqrt();
    let perpendicular = (pole - direction * pole.dot(direction))
        .try_normalize()
        .unwrap_or(Vec3::Z);
    Chain {
        middle: start + direction * along + perpendicular * bend,
        end: start + direction * length,
        reached: (distance - length).abs() < 0.001,
    }
}
fn bone(start: Vec3, end: Vec3, roll: Quat) -> Joint {
    Joint {
        p: start,
        r: Quat::from_rotation_arc(roll * -Vec3::Y, (end - start).normalize_or_zero()) * roll,
    }
}
fn arm_bone(start: Vec3, end: Vec3, plane: Vec3, twist: f32) -> Joint {
    let y = (start - end).normalize();
    let x = y.cross(plane).normalize();
    Joint {
        p: start,
        r: Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y))) * Quat::from_rotation_y(twist),
    }
}
fn vec(value: &Value) -> Vec3 {
    Vec3::new(
        value[0].as_f64().unwrap() as f32,
        value[1].as_f64().unwrap() as f32,
        value[2].as_f64().unwrap() as f32,
    )
}
fn unique_points(mesh: &MeshData) -> Vec<Vec3> {
    let mut points: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| Vec3::from_array(v.pos))
        .collect();
    points.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z))
    });
    points.dedup();
    points
}

/// Feet travel one stride in local +Z during stance, cancelling forward body
/// travel in world space. The return arc has zero vertical speed at both ends.
fn foot(phase: f32) -> (f32, f32) {
    let cycle = phase.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    const STRIDE: f32 = 1.2;
    const STANCE: f32 = 0.55;
    let reach = STRIDE * STANCE * 0.5;
    if cycle <= STANCE {
        (-reach + STRIDE * cycle, 0.)
    } else {
        let t = (cycle - STANCE) / (1. - STANCE);
        let tangent = STRIDE * (1. - STANCE);
        let z = hermite(reach, -reach, tangent, tangent, t);
        (z, (std::f32::consts::PI * t).sin().powi(2) * 0.075)
    }
}
fn hermite<T>(a: T, b: T, va: T, vb: T, t: f32) -> T
where
    T: Copy + std::ops::Mul<f32, Output = T> + std::ops::Add<Output = T>,
{
    let t2 = t * t;
    let t3 = t2 * t;
    a * (2. * t3 - 3. * t2 + 1.)
        + va * (t3 - 2. * t2 + t)
        + b * (-2. * t3 + 3. * t2)
        + vb * (t3 - t2)
}
#[derive(Clone, Copy, Debug)]
struct Grip {
    p: Vec3,
    angles: Vec3,
}
impl Grip {
    fn new(p: Vec3, yaw: f32, pitch: f32, roll: f32) -> Self {
        Self {
            p,
            angles: Vec3::new(yaw, pitch, roll),
        }
    }
    fn mix(self, to: Self, t: f32) -> Self {
        Self {
            p: self.p.lerp(to.p, t),
            angles: self.angles.lerp(to.angles, t),
        }
    }
    fn joint(self) -> Joint {
        Joint {
            p: self.p,
            r: Quat::from_rotation_y(self.angles.x)
                * Quat::from_rotation_z(self.angles.y)
                * Quat::from_rotation_x(self.angles.z),
        }
    }
}
fn ready(kind: EnemyKind) -> Grip {
    use std::f32::consts::FRAC_PI_2 as H;
    match kind {
        EnemyKind::SwordSoldier => Grip::new(Vec3::new(0.24, 1.15, -0.20), 1.20, 0.60, 0.),
        EnemyKind::SpearSoldier => Grip::new(Vec3::new(-0.10, 1.13, -0.10), H, 0.10, 0.),
        EnemyKind::HollowAxeKnight => Grip::new(Vec3::new(0.06, 1.16, -0.20), H, 0.75, H),
        EnemyKind::Cyclops => Grip::new(Vec3::new(0.08, 1.08, -0.15), 1.30, 0.55, H),
    }
}
fn attack_grip(kind: EnemyKind, attack: EnemyAttack, t: f32) -> Grip {
    use std::f32::consts::{FRAC_PI_2 as H, PI};
    let rest = ready(kind);
    let (wind, contact, follow) = match attack {
        EnemyAttack::Slash => (
            Grip::new(Vec3::new(0.36, 1.41, -0.07), 0.10, 0.20, 0.),
            Grip::new(Vec3::new(0.06, 1.18, -0.40), H, -0.10, 0.),
            Grip::new(Vec3::new(-0.17, 1.05, -0.22), PI - 0.12, -0.20, 0.),
        ),
        EnemyAttack::Thrust => (
            Grip::new(Vec3::new(-0.08, 1.12, 0.01), H, 0.03, 0.),
            Grip::new(Vec3::new(-0.10, 1.13, -0.26), H, 0., 0.),
            Grip::new(Vec3::new(-0.10, 1.13, -0.26), H, 0., 0.),
        ),
        EnemyAttack::Chop => (
            Grip::new(Vec3::new(0.03, 1.64, -0.04), H, 1.34, H),
            Grip::new(Vec3::new(0.02, 1.08, -0.39), H, -0.32, H),
            Grip::new(Vec3::new(0.01, 0.94, -0.30), H, -0.56, H),
        ),
        EnemyAttack::Sweep => (
            Grip::new(Vec3::new(0.23, 1.21, -0.03), 0.12, 0.28, H),
            Grip::new(Vec3::new(0.03, 1.05, -0.40), H, -0.08, H),
            Grip::new(Vec3::new(-0.08, 1.08, -0.20), PI - 0.12, -0.15, H),
        ),
        EnemyAttack::Slam => (
            Grip::new(Vec3::new(0.02, 1.68, -0.03), H, 1.40, H),
            Grip::new(Vec3::new(0.02, 1.00, -0.47), H, -0.70, H),
            Grip::new(Vec3::new(0.02, 1.00, -0.46), H, -0.72, H),
        ),
    };
    let windup = attack.windup_time();
    let impact = attack.contact_time();
    let end = attack.follow_end();
    if t < windup {
        return rest.mix(wind, ease(0., windup * 0.88, t));
    }
    if t >= end {
        return follow.mix(rest, ease(end, attack.duration(), t));
    }
    // A thrust stops at its furthest reach, while cuts retain velocity through
    // contact and decelerate in their follow-through rather than pausing there.
    let velocity = if attack == EnemyAttack::Thrust || attack == EnemyAttack::Slam {
        Vec3::ZERO
    } else {
        (follow.p - wind.p) / (end - windup) * 1.35
    };
    let spin = if attack == EnemyAttack::Thrust || attack == EnemyAttack::Slam {
        Vec3::ZERO
    } else {
        (follow.angles - wind.angles) / (end - windup) * 1.35
    };
    if t < impact {
        let dt = impact - windup;
        let u = ((t - windup) / dt).clamp(0., 1.);
        Grip {
            p: hermite(wind.p, contact.p, Vec3::ZERO, velocity * dt, u),
            angles: hermite(wind.angles, contact.angles, Vec3::ZERO, spin * dt, u),
        }
    } else {
        let dt = end - impact;
        let u = ((t - impact) / dt).clamp(0., 1.);
        Grip {
            p: hermite(contact.p, follow.p, velocity * dt, Vec3::ZERO, u),
            angles: hermite(contact.angles, follow.angles, spin * dt, Vec3::ZERO, u),
        }
    }
}

impl Rig {
    fn add_weapon(&mut self, meshes: &mut Vec<MeshData>, bytes: &[u8], metadata: &Value) {
        let mut parts = load_glb(bytes).expect("measured enemy weapon");
        assert_eq!(parts.len(), 1);
        let part = parts.remove(0);
        assert_eq!(part.name, metadata["node"].as_str().unwrap());
        assert!(part.mesh.texture.is_some(), "weapon RGB8 albedo");
        let min = part
            .mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.pos))
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let max = part
            .mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.pos))
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        let support = (!metadata["support_grip"].is_null()).then(|| vec(&metadata["support_grip"]));
        let tip = vec(&metadata["cutting_point"]);
        assert!(
            tip.cmpge(min - Vec3::splat(0.01)).all() && tip.cmple(max + Vec3::splat(0.01)).all(),
            "cutting point outside authored geometry"
        );
        let floor_points = unique_points(&part.mesh);
        meshes.push(part.mesh);
        self.weapons.push(Weapon {
            name: part.name,
            mesh: meshes.len() as u32,
            support,
            tip,
            min,
            floor_points,
        });
    }
    fn load(meshes: &mut Vec<MeshData>, bytes: &[u8], json: &str, prefix: &str) -> Self {
        let metadata: Value = serde_json::from_str(json).expect("enemy rig metadata");
        let mut loaded = load_glb(bytes).expect("generated enemy GLB");
        let rows = metadata["parts"].as_array().unwrap();
        let parts = BODY
            .iter()
            .map(|suffix| {
                let name = format!("{prefix}_{suffix}");
                let index = loaded
                    .iter()
                    .position(|p| p.name == name)
                    .expect("mapped enemy body part");
                let part = loaded.remove(index);
                assert!(part.mesh.texture.is_some(), "enemy RGB8 albedo");
                let row = rows
                    .iter()
                    .find(|r| r["name"] == name)
                    .expect("enemy pivot");
                let floor_points = if suffix.starts_with("boot_") || suffix.starts_with("hand_") {
                    unique_points(&part.mesh)
                } else {
                    Vec::new()
                };
                meshes.push(part.mesh);
                Part {
                    mesh: meshes.len() as u32,
                    pivot: vec(&row["pivot"]),
                    floor_points,
                }
            })
            .collect();
        if prefix == "enemy" {
            assert!(loaded.is_empty(), "unmapped generated enemy parts");
        }
        Self {
            parts,
            weapons: Vec::new(),
            eye: metadata
                .get("oneeye_local")
                .filter(|v| !v.is_null())
                .map(vec),
        }
    }
    fn weapon(&self, kind: EnemyKind) -> Option<&Weapon> {
        let name = match kind {
            EnemyKind::SwordSoldier => "weapon_sword",
            EnemyKind::SpearSoldier => "weapon_spear",
            EnemyKind::HollowAxeKnight => "weapon_axe",
            EnemyKind::Cyclops => "weapon_maul",
        };
        self.weapons.iter().find(|w| w.name == name)
    }
    fn pose(&self, e: &Enemy, time: f32) -> ([Joint; 18], Joint, [Chain; 4]) {
        let dead = if e.phase == EnemyPhase::Dead {
            ease(0., FALL_TIME, e.elapsed)
        } else {
            0.
        };
        let walk = e.walk_phase;
        let walking = e.walk_blend * (1. - dead);
        let breath = (time
            * if e.kind == EnemyKind::Cyclops {
                1.05
            } else {
                1.65
            })
        .sin();
        let attack = e.attack_pose();
        let (loaded, driven) = attack.map_or((0., 0.), |(a, t, w)| {
            let recovery = 1. - ease(a.follow_end(), a.duration(), t);
            (
                ease(0., a.windup_time(), t) * recovery * w,
                ease(a.windup_time(), a.contact_time(), t) * recovery * w,
            )
        });
        let heavy = matches!(e.kind, EnemyKind::HollowAxeKnight | EnemyKind::Cyclops);
        let flinch = e.flinch_amount() * 0.045;
        let mut pelvis = Vec3::new(
            walking * walk.sin() * 0.02,
            0.978 - walking * (0.055 + 0.004 * (walk * 2.).cos()) + breath * 0.002 - loaded * 0.018,
            loaded * 0.02 - driven * if heavy { 0.055 } else { 0.035 },
        );
        pelvis = pelvis.lerp(Vec3::new(0., 0.40, -0.15), dead);
        let mut joints = [Joint::IDENTITY; 18];
        joints[0] = Joint {
            p: pelvis,
            r: Quat::from_rotation_y(-walking * walk.sin() * 0.035 * (1. - dead))
                * Quat::from_rotation_z(-dead * 0.06),
        };
        let lean = (-0.07 - if heavy { 0.04 } else { 0. } - walking * 0.025 + loaded * 0.055
            - driven * 0.17
            + flinch)
            * (1. - dead)
            - dead * 1.22;
        let twist = (walking * walk.sin() * 0.055 - loaded * 0.16 + driven * 0.30) * (1. - dead);
        joints[1] = joints[0].child(
            self.parts[1].pivot - self.parts[0].pivot,
            Quat::from_rotation_x(lean) * Quat::from_rotation_y(twist),
        );
        joints[2] = joints[1].child(
            self.parts[2].pivot - self.parts[1].pivot,
            Quat::from_rotation_y(-twist * 0.55) * Quat::from_rotation_x(0.03 - dead * 0.30),
        );
        joints[3] = joints[2].child(
            self.parts[3].pivot - self.parts[2].pivot,
            Quat::from_rotation_x(-0.02 + flinch - dead * 0.15),
        );
        let mut chains = [Chain {
            middle: Vec3::ZERO,
            end: Vec3::ZERO,
            reached: false,
        }; 4];
        for (side, thigh, shin, boot, coat) in [(0, 7, 8, 9, 10), (1, 14, 15, 16, 17)] {
            let sign = if side == 0 { 1. } else { -1. };
            let (z, lift) = foot(walk + if side == 0 { 0. } else { std::f32::consts::PI });
            // A falling body rolls onto the toe and lets its knees splay. The
            // authored boot hull sets ankle height, retaining real floor
            // contact instead of driving a thick armored calf through stone.
            let foot_rotation = Quat::from_rotation_x(if e.phase == EnemyPhase::Dead {
                -0.80 * ease(0., 0.85, e.elapsed)
            } else {
                0.
            });
            let grounded_ankle = -self.parts[boot]
                .floor_points
                .iter()
                .map(|p| (foot_rotation * (*p - self.parts[boot].pivot)).y)
                .fold(f32::INFINITY, f32::min);
            let ankle = Vec3::new(
                self.parts[boot].pivot.x + sign * 0.023,
                self.parts[boot].pivot.y.max(grounded_ankle) + walking * lift,
                walking * z + dead * 0.66,
            );
            let hip = joints[0].point(self.parts[thigh].pivot - self.parts[0].pivot);
            let chain = two_bone(
                hip,
                ankle,
                0.43,
                0.43,
                Vec3::new(sign * dead * 0.8, dead * 0.2, -1. + dead * 0.7),
            );
            joints[thigh] = bone(hip, chain.middle, Quat::IDENTITY);
            joints[shin] = bone(chain.middle, chain.end, Quat::IDENTITY);
            joints[boot] = Joint {
                p: chain.end,
                r: foot_rotation,
            };
            joints[coat] = Joint {
                p: hip,
                // The rigid coat panel settles from the thigh toward a flat
                // fold before the hip reaches the floor. Its attachment stays
                // at the hip, with one continuous authored recovery curve.
                r: joints[thigh].r.slerp(
                    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                    if e.phase == EnemyPhase::Dead {
                        ease(0.4, 1.0, e.elapsed)
                    } else {
                        0.
                    },
                ),
            };
            chains[side + 2] = chain;
        }
        let rest = ready(e.kind);
        let mut grip = attack.map_or(rest, |(a, t, w)| rest.mix(attack_grip(e.kind, a, t), w));
        grip.p += Vec3::new(
            0.,
            walking * walk.sin() * 0.012,
            walking * walk.cos() * 0.020,
        );
        let drop_height = self
            .weapon(e.kind)
            .map_or(0.22, |w| 0.22f32.max(-w.min.y + 0.015));
        let dropped = Grip::new(Vec3::new(0.10, drop_height, -0.43), 0., 0., 0.);
        grip = grip.mix(dropped, dead);
        let mut weapon = grip.joint();
        if e.phase == EnemyPhase::Dead {
            if let Some(w) = self.weapon(e.kind) {
                let bottom = w
                    .floor_points
                    .iter()
                    .map(|p| weapon.point(*p).y)
                    .fold(f32::INFINITY, f32::min);
                weapon.p.y += (0.01 - bottom).max(0.);
            }
            let hand = &self.parts[6];
            let wrist = weapon.p - weapon.r * HAND_SOCKET;
            let bottom = hand
                .floor_points
                .iter()
                .map(|p| wrist.y + (weapon.r * (*p - hand.pivot)).y)
                .fold(f32::INFINITY, f32::min);
            weapon.p.y += (0.003 - bottom).max(0.);
        }
        let right = Joint {
            p: weapon.p - weapon.r * HAND_SOCKET,
            r: weapon.r,
        };
        let mut left = if let Some(support) = self.weapon(e.kind).and_then(|w| w.support) {
            let r = weapon.r * Quat::from_rotation_x(std::f32::consts::PI);
            Joint {
                p: weapon.point(support) - r * HAND_SOCKET,
                r,
            }
        } else {
            Joint {
                p: Vec3::new(
                    -0.30,
                    1.16 - walking * walk.sin() * 0.025,
                    -0.20 - walking * walk.cos() * 0.04,
                ),
                r: Quat::from_rotation_x(0.30 * (1. - dead)),
            }
        };
        // As the body collapses the support hand releases, while the primary
        // wrist retains the weapon through the fall without a detached prop.
        let mut released = joints[1].point(self.parts[11].pivot - self.parts[1].pivot)
            + Vec3::new(-0.03, -0.31, -0.20);
        released.y = released.y.max(0.18);
        left.p = left.p.lerp(released, dead);
        left.r = left.r.slerp(Quat::IDENTITY, dead);
        if e.phase == EnemyPhase::Dead {
            let hand = &self.parts[13];
            let bottom = hand
                .floor_points
                .iter()
                .map(|p| left.point(*p - hand.pivot).y)
                .fold(f32::INFINITY, f32::min);
            left.p.y += (0.003 - bottom).max(0.);
        }
        for (side, upper, forearm, hand, target) in [(0, 4, 5, 6, right), (1, 11, 12, 13, left)] {
            let sign = if side == 0 { 1. } else { -1. };
            let shoulder = joints[1].point(self.parts[upper].pivot - self.parts[1].pivot);
            let direction = (target.p - shoulder).normalize_or_zero();
            let pole = Quat::from_rotation_arc(-Vec3::Y, direction) * Vec3::new(sign, 0., 0.22);
            let chain = two_bone(shoulder, target.p, 0.32, 0.29, pole);
            // The elbow plane supplies a continuous frame even when a raised
            // forearm passes the bind axis's opposite pole. Mirrored palm
            // rotation stays at the glove instead of flipping the humerus.
            let twist = sign * grip.angles.z;
            let plane = (chain.middle - shoulder)
                .cross(chain.end - chain.middle)
                .normalize();
            joints[upper] = arm_bone(shoulder, chain.middle, plane, twist * 0.18);
            joints[forearm] = arm_bone(chain.middle, chain.end, plane, twist * 0.42);
            joints[hand] = Joint {
                p: chain.end,
                r: target.r,
            };
            chains[side] = chain;
        }
        // The primary attachment follows the solved wrist exactly.
        weapon = joints[6].child(HAND_SOCKET, Quat::IDENTITY);
        (joints, weapon, chains)
    }
}

impl EnemyScene {
    pub fn load(meshes: &mut Vec<MeshData>) -> Self {
        Self::from_sources(
            meshes,
            [
                (
                    include_bytes!("../../../assets/end-game/v10/soldier.glb"),
                    include_str!("../../../assets/end-game/v10/soldier-rig.json"),
                ),
                (
                    include_bytes!("../../../assets/end-game/v10/hollow-knight.glb"),
                    include_str!("../../../assets/end-game/v10/hollow-knight-rig.json"),
                ),
                (
                    include_bytes!("../../../assets/end-game/v10/cyclops.glb"),
                    include_str!("../../../assets/end-game/v10/cyclops-rig.json"),
                ),
            ],
        )
    }
    fn from_sources(meshes: &mut Vec<MeshData>, sources: [(&[u8], &str); 3]) -> Self {
        let mut rigs = sources.map(|(bytes, json)| Rig::load(meshes, bytes, json, "enemy"));
        let metadata = weapon_metadata();
        for (index, (name, bytes)) in WEAPON_ASSETS.iter().enumerate() {
            let row = metadata["weapons"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["name"] == *name)
                .expect("weapon contract");
            rigs[match index {
                0 | 1 => 0,
                2 => 1,
                _ => 2,
            }]
            .add_weapon(meshes, bytes, row);
        }
        Self { rigs }
    }
    fn rig(&self, kind: EnemyKind) -> &Rig {
        &self.rigs[match kind {
            EnemyKind::SwordSoldier | EnemyKind::SpearSoldier => 0,
            EnemyKind::HollowAxeKnight => 1,
            EnemyKind::Cyclops => 2,
        }]
    }
    pub fn draw(&self, frame: &mut Frame, game: &Dungeon) {
        let eye = frame.camera.eye;
        let forward = (frame.camera.target - eye).normalize_or_zero();
        for enemy in &game.enemies {
            let center = enemy.position + Vec3::Y * enemy.kind.height() * 0.5;
            let d = center - eye;
            let radius = enemy.kind.height().max(enemy.attack.range());
            if d.length()
                > if enemy.kind == EnemyKind::Cyclops {
                    42.
                } else {
                    30.
                }
                || d.dot(forward) + radius < 0.
            {
                continue;
            }
            if enemy.position.z < -22. && eye.z > -20. {
                continue;
            }
            if enemy.position.z < -64. && eye.z > -50. {
                continue;
            }
            let rig = self.rig(enemy.kind);
            let (joints, weapon, _) = rig.pose(enemy, game.time);
            let scale = enemy.kind.height() / BIND_HEIGHT;
            let root = Quat::from_rotation_y(-enemy.yaw);
            for (index, part) in rig.parts.iter().enumerate() {
                let r = root * joints[index].r;
                let p = enemy.position + root * joints[index].p * scale - r * part.pivot * scale;
                frame.instances.push(
                    Instance::new(p, Vec3::splat(scale), Vec3::ONE)
                        .with_mesh(part.mesh)
                        .with_rot(r)
                        .with_surface(
                            if index == 2 || index == 3 { 0.92 } else { 0.85 },
                            if enemy.kind == EnemyKind::Cyclops {
                                0.0
                            } else {
                                0.08
                            },
                        ),
                );
            }
            if let Some(part) = rig.weapon(enemy.kind) {
                let p = enemy.position + root * weapon.p * scale;
                let r = root * weapon.r;
                let (roughness, metallic) = match enemy.kind {
                    EnemyKind::SwordSoldier => (0.48, 0.48),
                    EnemyKind::SpearSoldier => (0.74, 0.10),
                    EnemyKind::HollowAxeKnight => (0.64, 0.28),
                    EnemyKind::Cyclops => (0.83, 0.05),
                };
                frame.instances.push(
                    Instance::new(p, Vec3::splat(scale), Vec3::ONE)
                        .with_mesh(part.mesh)
                        .with_rot(r)
                        .with_surface(roughness, metallic),
                );
                if enemy.phase == EnemyPhase::Attacking {
                    let t = enemy.elapsed;
                    let attack = enemy.attack;
                    if t > attack.windup_time() && t < attack.follow_end() {
                        for age in [0.02, 0.04, 0.06] {
                            let mut prior = enemy.clone();
                            prior.elapsed = (t - age).max(0.);
                            let (_, trail, _) = rig.pose(&prior, game.time - age);
                            frame.particles.push(Particle {
                                position: enemy.position + root * trail.point(part.tip) * scale,
                                color: Vec3::new(0.58, 0.59, 0.61),
                                size: Vec2::splat(0.035 * scale),
                                opacity: 0.20 * (1. - age / 0.08),
                            });
                        }
                    }
                }
            }
            if enemy.kind == EnemyKind::Cyclops && enemy.alive() {
                let head = joints[3];
                let point = head.point(rig.eye.expect("cyclops eye marker") - rig.parts[3].pivot);
                frame.particles.push(Particle {
                    position: enemy.position + root * point * scale,
                    color: if enemy.phase_two {
                        Vec3::new(1., 0.25, 0.055)
                    } else {
                        Vec3::new(0.71, 0.56, 0.23)
                    },
                    size: Vec2::splat(0.026 * scale),
                    opacity: 0.65,
                });
                if enemy.phase == EnemyPhase::Attacking && enemy.attack == EnemyAttack::Slam {
                    let after = (enemy.elapsed - enemy.attack.contact_time()).max(0.);
                    let intensity = if enemy.elapsed < enemy.attack.contact_time() {
                        ease(0., enemy.attack.windup_time(), enemy.elapsed) * 0.45
                    } else {
                        (1. - after / 0.5).max(0.)
                    };
                    let origin = enemy.position + enemy.forward() * 2.5 + Vec3::Y * 0.035;
                    if enemy.elapsed < enemy.attack.contact_time() {
                        let angle = 0.82f32.acos();
                        for i in 0..31 {
                            let a = -angle + 2. * angle * i as f32 / 30.;
                            let p = Vec3::new(a.sin(), 0., -a.cos()) * enemy.attack.range();
                            frame.particles.push(Particle {
                                position: enemy.position + root * p + Vec3::Y * 0.035,
                                color: Vec3::new(0.93, 0.35, 0.12),
                                size: Vec2::new(0.085, 0.024),
                                opacity: intensity,
                            });
                        }
                        for side in [-1., 1.] {
                            for i in 1..10 {
                                let p = Vec3::new((side * angle).sin(), 0., -(side * angle).cos())
                                    * enemy.attack.range()
                                    * i as f32
                                    / 10.;
                                frame.particles.push(Particle {
                                    position: enemy.position + root * p + Vec3::Y * 0.035,
                                    color: Vec3::new(0.93, 0.35, 0.12),
                                    size: Vec2::new(0.07, 0.023),
                                    opacity: intensity,
                                });
                            }
                        }
                    }
                    for i in 0..24 {
                        let a = i as f32 * std::f32::consts::TAU / 24.;
                        frame.particles.push(Particle {
                            position: origin
                                + Vec3::new(a.sin(), 0., a.cos()) * (0.55 + after * 1.1),
                            color: Vec3::new(0.87, 0.38, 0.12),
                            size: Vec2::new(0.055, 0.015),
                            opacity: intensity,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> &'static (EnemyScene, Vec<MeshData>) {
        static ASSETS: std::sync::OnceLock<(EnemyScene, Vec<MeshData>)> =
            std::sync::OnceLock::new();
        ASSETS.get_or_init(|| {
            let mut meshes = Vec::new();
            let scene = EnemyScene::load(&mut meshes);
            (scene, meshes)
        })
    }

    fn geometry_above_floor(rig: &Rig, meshes: &[MeshData], joints: &[Joint; 18], label: &str) {
        for (index, part) in rig.parts.iter().enumerate() {
            let min_y = meshes[part.mesh as usize - 1]
                .vertices
                .iter()
                .map(|v| joints[index].point(Vec3::from_array(v.pos) - part.pivot).y)
                .fold(f32::INFINITY, f32::min);
            assert!(
                min_y >= -0.001,
                "{label}: {} geometry crosses floor by {}m",
                BODY[index],
                -min_y
            );
        }
    }

    #[test]
    fn final_rigs_register_once_and_instances_reuse_textured_meshes() {
        let (scene, meshes) = fixture();
        assert_eq!(meshes.len(), 3 * 18 + 4);
        for mesh in meshes {
            let texture = mesh.texture.as_ref().expect("decoded RGB8 generated atlas");
            assert_eq!(
                texture.rgba8.len(),
                (texture.width * texture.height * 4) as usize
            );
            for vertex in &mesh.vertices {
                assert!(Vec3::from_array(vertex.pos).is_finite());
                assert!((Vec3::from_array(vertex.normal).length() - 1.).abs() < 0.005);
            }
        }
        let giant = scene.rig(EnemyKind::Cyclops);
        assert!((giant.parts[4].pivot.x - 0.36).abs() < 0.0001);
        assert!((giant.parts[7].pivot.x - 0.17).abs() < 0.0001);
        assert!(giant.eye.unwrap().distance(giant.parts[3].pivot) < 0.35);
        for kind in [
            EnemyKind::SwordSoldier,
            EnemyKind::HollowAxeKnight,
            EnemyKind::Cyclops,
        ] {
            let rig = scene.rig(kind);
            for (upper, lower, end, a, b) in [
                (4, 5, 6, 0.32, 0.29),
                (11, 12, 13, 0.32, 0.29),
                (7, 8, 9, 0.43, 0.43),
                (14, 15, 16, 0.43, 0.43),
            ] {
                assert!(
                    (rig.parts[upper].pivot.distance(rig.parts[lower].pivot) - a).abs() < 0.0001
                );
                assert!((rig.parts[lower].pivot.distance(rig.parts[end].pivot) - b).abs() < 0.0001);
            }
        }
        let mut game = Dungeon::default();
        game.enemies = vec![
            Enemy::new(0, EnemyKind::SwordSoldier, Vec3::new(-1., 0., 0.)),
            Enemy::new(1, EnemyKind::SwordSoldier, Vec3::new(1., 0., 0.)),
        ];
        let mut frame = Frame::default();
        frame.camera.eye = Vec3::new(0., 1.7, 4.);
        frame.camera.target = Vec3::Y;
        scene.draw(&mut frame, &game);
        assert_eq!(frame.instances.len(), 38);
        let first: Vec<_> = frame.instances[..19].iter().map(|i| i.mesh).collect();
        let second: Vec<_> = frame.instances[19..].iter().map(|i| i.mesh).collect();
        assert_eq!(
            first, second,
            "soldier instances duplicated registered assets"
        );
        let sole = |frame: &Frame, mesh: u32| {
            let instance = frame.instances.iter().find(|i| i.mesh == mesh).unwrap();
            assert!(instance.casts_shadow);
            meshes[mesh as usize - 1]
                .vertices
                .iter()
                .map(|v| {
                    instance.position + instance.rot * (Vec3::from_array(v.pos) * instance.scale)
                })
                .fold(Vec3::splat(f32::INFINITY), Vec3::min)
        };
        for kind in [
            EnemyKind::SwordSoldier,
            EnemyKind::SpearSoldier,
            EnemyKind::HollowAxeKnight,
            EnemyKind::Cyclops,
        ] {
            game.time = 1.4;
            game.enemies = vec![Enemy::new(0, kind, Vec3::ZERO)];
            let rig = scene.rig(kind);
            frame.instances.clear();
            frame.particles.clear();
            scene.draw(&mut frame, &game);
            let idle = [
                sole(&frame, rig.parts[9].mesh),
                sole(&frame, rig.parts[16].mesh),
            ];
            assert!(
                idle.iter().all(|p| p.y.abs() < 0.00001),
                "{kind:?} rendered idle sole gap: {idle:?}"
            );
            // This is the native capture's exact walking phase, a passing
            // pose that reads subtly from the front despite a lifted boot.
            game.enemies[0].phase = EnemyPhase::Hunting;
            game.enemies[0].walk_phase = 1.5;
            game.enemies[0].walk_blend = 1.;
            frame.instances.clear();
            frame.particles.clear();
            scene.draw(&mut frame, &game);
            let walking = [
                sole(&frame, rig.parts[9].mesh),
                sole(&frame, rig.parts[16].mesh),
            ];
            assert!(walking[0].y.abs() < 0.00001);
            assert!(walking[1].y > 0.05 * kind.height() / BIND_HEIGHT);
            assert!((walking[0].z - idle[0].z).abs() > 0.035);
            eprintln!(
                "{kind:?} rendered soles idle={:.6}/{:.6}m, walk={:.6}/{:.6}m; boot travel Z={:.6}/{:.6}m",
                idle[0].y,
                idle[1].y,
                walking[0].y,
                walking[1].y,
                walking[0].z - idle[0].z,
                walking[1].z - idle[1].z
            );
        }
    }
    #[test]
    fn enemy_attacks_reach_both_grips_without_stretching_or_floor_penetration() {
        let (scene, meshes) = fixture();
        for (kind, attack) in [
            (EnemyKind::SwordSoldier, EnemyAttack::Slash),
            (EnemyKind::SpearSoldier, EnemyAttack::Thrust),
            (EnemyKind::HollowAxeKnight, EnemyAttack::Chop),
            (EnemyKind::Cyclops, EnemyAttack::Sweep),
            (EnemyKind::Cyclops, EnemyAttack::Slam),
        ] {
            let rig = scene.rig(kind);
            let mut e = Enemy::new(0, kind, Vec3::ZERO);
            e.phase = EnemyPhase::Attacking;
            e.attack = attack;
            e.walk_phase = 1.5;
            for tick in 0..=(attack.duration() * 120.).ceil() as u32 {
                e.elapsed = tick as f32 / 120.;
                // Core stops pursuit during windup with this blend decay.
                e.walk_blend = (1. - e.elapsed * 5.).max(0.);
                let (joints, weapon, chains) = rig.pose(&e, 0.);
                for (index, chain) in chains.iter().enumerate() {
                    assert!(
                        chain.reached,
                        "{kind:?} {attack:?} tick{tick} chain{index}: {:?}",
                        chain
                    );
                }
                assert!(weapon.p.distance(joints[6].point(HAND_SOCKET)) < 0.00001);
                if let Some(support) = rig.weapon(kind).unwrap().support {
                    assert!(
                        weapon
                            .point(support)
                            .distance(joints[13].point(HAND_SOCKET))
                            < 0.001,
                        "support hand detached {attack:?} at {}",
                        e.elapsed
                    );
                }
                for boot in [9, 16] {
                    assert!(joints[boot].p.y >= rig.parts[boot].pivot.y - 0.0001);
                }
                for j in joints {
                    assert!(j.p.is_finite() && j.r.is_normalized());
                }
                geometry_above_floor(
                    rig,
                    meshes,
                    &joints,
                    &format!("{kind:?} {attack:?} t{}", e.elapsed),
                );
                let blade = rig.weapon(kind).unwrap();
                for vertex in &meshes[blade.mesh as usize - 1].vertices {
                    let point = weapon.point(Vec3::from_array(vertex.pos));
                    assert!(
                        point.y >= -0.001,
                        "{kind:?} {attack:?} t{} weapon below floor: {point:?}",
                        e.elapsed
                    );
                }
            }
        }
    }
    #[test]
    fn feet_cancel_actual_stride_and_stop_without_restarting_phase() {
        let (scene, _) = fixture();
        for kind in [
            EnemyKind::SwordSoldier,
            EnemyKind::HollowAxeKnight,
            EnemyKind::Cyclops,
        ] {
            let rig = scene.rig(kind);
            let scale = kind.height() / BIND_HEIGHT;
            for (boot, start) in [(9, 0.), (16, 0.6)] {
                let mut anchor = None;
                for tick in 0..=30 {
                    let distance = (start + tick as f32 * 0.02) * scale;
                    let mut e = Enemy::new(0, kind, -Vec3::Z * distance);
                    e.walk_blend = 1.;
                    e.walk_phase = distance / kind.stride_length() * std::f32::consts::TAU;
                    e.phase = EnemyPhase::Hunting;
                    let (joints, _, chains) = rig.pose(&e, 0.);
                    assert!(chains[if boot == 9 { 2 } else { 3 }].reached);
                    let world = e.position + joints[boot].p * scale;
                    let point = *anchor.get_or_insert(world);
                    assert!(
                        world.distance(point) < 0.00001,
                        "stance slide {kind:?} {tick}: {world:?} {point:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn loaded_rig_gaits_keep_actual_boots_and_coat_above_floor() {
        let (scene, meshes) = fixture();
        for kind in [
            EnemyKind::SwordSoldier,
            EnemyKind::HollowAxeKnight,
            EnemyKind::Cyclops,
        ] {
            let rig = scene.rig(kind);
            let mut e = Enemy::new(0, kind, Vec3::ZERO);
            e.phase = EnemyPhase::Hunting;
            for blend in [0., 0.5, 1.] {
                e.walk_blend = blend;
                for tick in 0..=120 {
                    e.walk_phase = tick as f32 / 120. * std::f32::consts::TAU;
                    let (joints, _, chains) = rig.pose(&e, 0.);
                    assert!(
                        chains.iter().all(|c| c.reached),
                        "{kind:?} gait chain at{tick} blend{blend}"
                    );
                    geometry_above_floor(
                        rig,
                        meshes,
                        &joints,
                        &format!("{kind:?} gait{tick} blend{blend}"),
                    );
                }
            }
        }
    }
    #[test]
    fn contact_and_recovery_match_authored_clocks_without_extra_strikes() {
        for (kind, a) in [
            (EnemyKind::SwordSoldier, EnemyAttack::Slash),
            (EnemyKind::HollowAxeKnight, EnemyAttack::Chop),
            (EnemyKind::Cyclops, EnemyAttack::Sweep),
        ] {
            let t = a.contact_time();
            let dt = 0.0001;
            let before = attack_grip(kind, a, t - dt);
            let contact = attack_grip(kind, a, t);
            let after = attack_grip(kind, a, t + dt);
            assert!(
                ((contact.p - before.p) / dt - (after.p - contact.p) / dt).length() < 0.10,
                "cut velocity jumps at {a:?} contact"
            );
            let end = attack_grip(kind, a, a.duration());
            let rest = ready(kind);
            assert!(end.p.distance(rest.p) < 0.00001 && end.angles.distance(rest.angles) < 0.00001);
        }
    }
    #[test]
    fn heavy_interruptions_begin_at_the_reached_attack_pose() {
        let (scene, _) = fixture();
        for (kind, attack) in [
            (EnemyKind::SwordSoldier, EnemyAttack::Slash),
            (EnemyKind::SpearSoldier, EnemyAttack::Thrust),
            (EnemyKind::HollowAxeKnight, EnemyAttack::Chop),
            (EnemyKind::Cyclops, EnemyAttack::Slam),
        ] {
            let rig = scene.rig(kind);
            for at in [attack.windup_time(), attack.contact_time()] {
                for killed in [false, true] {
                    let mut e = Enemy::new(0, kind, Vec3::ZERO);
                    e.phase = EnemyPhase::Attacking;
                    e.attack = attack;
                    e.elapsed = at;
                    let before = rig.pose(&e, 0.).0;
                    e.take_hit(if killed { 1000. } else { 1. }, true);
                    let after = rig.pose(&e, 0.).0;
                    for (index, (a, b)) in before.iter().zip(after).enumerate() {
                        assert!(
                            a.p.distance(b.p) < 0.00001 && a.r.abs_diff_eq(b.r, 0.00001),
                            "{kind:?} interrupted {at} killed{killed} part{index}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn interrupted_recovery_keeps_weapon_attached_and_above_floor() {
        let (scene, meshes) = fixture();
        for (kind, attack) in [
            (EnemyKind::SwordSoldier, EnemyAttack::Slash),
            (EnemyKind::SpearSoldier, EnemyAttack::Thrust),
            (EnemyKind::HollowAxeKnight, EnemyAttack::Chop),
            (EnemyKind::Cyclops, EnemyAttack::Sweep),
            (EnemyKind::Cyclops, EnemyAttack::Slam),
        ] {
            let rig = scene.rig(kind);
            for at in [attack.windup_time(), attack.contact_time()] {
                let mut e = Enemy::new(0, kind, Vec3::ZERO);
                e.phase = EnemyPhase::Attacking;
                e.attack = attack;
                e.elapsed = at;
                e.take_hit(1000., true);
                let mut previous: Option<[Joint; 18]> = None;
                for tick in 0..=180 {
                    e.elapsed = tick as f32 / 120.;
                    e.flinch_left = (0.22 - e.elapsed).max(0.);
                    let (joints, weapon, chains) = rig.pose(&e, 0.);
                    for (i, chain) in chains.iter().enumerate() {
                        assert!(
                            chain.reached,
                            "{kind:?} {attack:?} death from{at} t{} chain{i}: {chain:?}",
                            e.elapsed
                        );
                    }
                    assert!(weapon.p.distance(joints[6].point(HAND_SOCKET)) < 0.00001);
                    geometry_above_floor(
                        rig,
                        meshes,
                        &joints,
                        &format!("{kind:?} {attack:?} death{}", e.elapsed),
                    );
                    let blade = rig.weapon(kind).unwrap();
                    for vertex in &meshes[blade.mesh as usize - 1].vertices {
                        assert!(
                            weapon.point(Vec3::from_array(vertex.pos)).y >= -0.001,
                            "{attack:?} dead weapon passes through floor at {}",
                            e.elapsed
                        );
                    }
                    if let Some(prior) = previous {
                        for (i, (p, joint)) in prior.iter().zip(joints).enumerate() {
                            assert!(
                                p.p.distance(joint.p) < 0.065 && p.r.angle_between(joint.r) < 0.23,
                                "{attack:?} death pose jumps at {} joint{i}: {:?} -> {joint:?}",
                                e.elapsed,
                                p
                            );
                        }
                    }
                    previous = Some(joints);
                }
            }
        }
    }
}
