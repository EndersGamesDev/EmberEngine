//! Metric articulated viewmodel. Item sockets and finger pivots come from the asset worker.
use ember_engine::{
    Instance, MeshData,
    assets::load_glb,
    rig::{self, ArmSide, HumanoidDims, Pose},
};
use end_game_core::{
    Dungeon, InteractionKind,
    interaction::{board_grip, ease},
};
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;
use std::f32::consts::FRAC_PI_2;

#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub p: Vec3,
    pub r: Quat,
}
impl Placement {
    fn mix(self, to: Self, t: f32) -> Self {
        Self {
            p: self.p.lerp(to.p, t),
            r: self.r.slerp(to.r, t),
        }
    }
    fn point(self, local: Vec3) -> Vec3 {
        self.p + self.r * local
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pickup_contacts_are_reachable_without_stretching_or_sliding() {
        let hands = Hands::load(&mut Vec::new());
        for (kind, stage, crouched) in [
            (InteractionKind::Board, 0),
            (InteractionKind::Key, 1),
            (InteractionKind::Lock, 2),
            (InteractionKind::Sword, 3),
        ]
        .into_iter()
        .flat_map(|(k, s)| [(k, s, false), (k, s, true)])
        {
            for tick in 0..=(kind.duration() * 60.0) as usize {
                let elapsed = tick as f32 / 60.0;
                let action = end_game_core::Interaction {
                    kind,
                    elapsed,
                    origin: kind.approach(),
                    committed: elapsed >= kind.commit_time(),
                };
                let mut game = Dungeon::default();
                game.position = kind.approach();
                game.crouched = crouched;
                game.stage = stage + u8::from(action.committed);
                game.interaction = Some(action);
                game.board_open = if kind == InteractionKind::Board {
                    action.manipulate()
                } else {
                    1.0
                };
                let v = view(&game, 0.0);
                let m = hands.motion(&game, &v);
                let (_, solved) = hands.skeleton_pose(&v, &m);
                for (side, arm) in solved.iter().enumerate() {
                    assert!(
                        arm.reached,
                        "{kind:?} t={elapsed:.3} side={side} target={:?} shoulder={:?} error={}",
                        m.poses[side].wrist.p,
                        arm.shoulder,
                        arm.wrist.distance(m.poses[side].wrist.p)
                    );
                    assert!((arm.shoulder.distance(arm.elbow) - 0.30).abs() < 0.0001);
                    assert!((arm.elbow.distance(arm.wrist) - 0.27).abs() < 0.0001);
                }
                if let Some(key) = m.key {
                    assert!(key.p.distance(m.poses[0].wrist.point(hands.pinch[0])) < 0.0001);
                }
                if kind == InteractionKind::Sword && action.reach() == 1.0 {
                    let sword = m.sword.unwrap();
                    if elapsed >= 1.20 {
                        for i in 0..=20 {
                            let point = sword.point(Vec3::X * (1.85 * i as f32 / 20.0));
                            if point.z < -4.0 {
                                assert!(
                                    point.y > 0.83,
                                    "blade swept through stone at {elapsed}: {point:?}"
                                );
                            }
                        }
                    }
                    for side in 0..2 {
                        assert!(
                            sword
                                .point(Vec3::new(if side == 0 { -0.13 } else { -0.25 }, 0.0, 0.0))
                                .distance(m.poses[side].wrist.point(hands.power[side]))
                                < 0.0001
                        );
                    }
                }
                let mut instances = Vec::new();
                hands.draw(&mut instances, &v, &m);
                assert_eq!(instances.len(), 34);
                assert!(
                    instances
                        .iter()
                        .all(|i| i.rot.is_finite() && i.position.is_finite())
                );
            }
        }
    }

    #[test]
    fn resting_and_sword_hands_reach_at_full_look_limits() {
        let hands = Hands::load(&mut Vec::new());
        for pitch in [-1.2, -0.5, 0.0, 0.5, 1.15] {
            for yaw in [-2.8, 0.0, 2.2] {
                for stage in [0, 4] {
                    let mut game = Dungeon::default();
                    game.pitch = pitch;
                    game.yaw = yaw;
                    game.stage = stage;
                    let v = view(&game, 0.0);
                    let m = hands.motion(&game, &v);
                    let (_, arms) = hands.skeleton_pose(&v, &m);
                    for arm in arms {
                        assert!(arm.reached, "pitch {pitch} stage {stage} arm {arm:?}");
                    }
                }
            }
        }
    }
}
pub struct View {
    pub head: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub rot: Quat,
    pub body_rot: Quat,
    pub yaw: f32,
}
pub fn view(game: &Dungeon, wake: f32) -> View {
    let focus = game.interaction.map_or(0.0, |a| a.focus());
    let normal_height = if game.crouched {
        1.0
    } else {
        1.65 - wake * 1.18
    };
    let height = game.interaction.map_or(normal_height, |a| {
        normal_height + (a.kind.head_height() - normal_height) * focus
    });
    let mut yaw = game.yaw;
    if let Some(a) = game.interaction {
        let d = a.kind.target() - a.kind.approach();
        let target = d.x.atan2(-d.z);
        yaw += ((target - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI)
            * focus;
    }
    let flat = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
    // Crouching moves the eye and shoulders together; the reach never lengthens a bone.
    let lean = game.interaction.map_or(0.12, |a| {
        if a.kind == InteractionKind::Sword {
            0.08
        } else {
            0.12
        }
    });
    let head = game.position + Vec3::Y * height + flat * (lean * focus);
    let pitch = game.interaction.map_or(game.pitch, |a| {
        let delta = a.kind.target() - head;
        let aim = delta.y.atan2(Vec3::new(delta.x, 0.0, delta.z).length());
        game.pitch + (aim - game.pitch) * focus
    });
    let rot = Quat::from_rotation_y(-yaw) * Quat::from_rotation_x(pitch);
    View {
        head,
        forward: rot * -Vec3::Z,
        right: rot * Vec3::X,
        up: rot * Vec3::Y,
        rot,
        body_rot: rot.slerp(Quat::from_rotation_y(-yaw), focus),
        yaw,
    }
}

struct Part {
    id: u32,
    parent: Option<usize>,
    pivot: Vec3,
    axis: Vec3,
    opposition: Vec3,
    side: usize,
    category: String,
    power: [f32; 2],
    pinch: [f32; 2],
}
pub struct Hands {
    parts: Vec<Part>,
    pinch: [Vec3; 2],
    power: [Vec3; 2],
    power_axis: [Vec3; 2],
    skeleton: rig::Skeleton,
}
#[derive(Clone, Copy)]
struct HandPose {
    wrist: Placement,
    curl: f32,
    pinch: f32,
}
pub struct Motion {
    poses: [HandPose; 2],
    pub key: Option<Placement>,
    pub sword: Option<Placement>,
}
fn vec(v: &Value) -> Vec3 {
    Vec3::new(
        v[0].as_f64().unwrap() as f32,
        v[1].as_f64().unwrap() as f32,
        v[2].as_f64().unwrap() as f32,
    )
}
fn angles(v: &Value) -> [f32; 2] {
    [
        v.as_f64().or_else(|| v["curl"].as_f64()).unwrap_or(0.0) as f32,
        v["opposition"].as_f64().unwrap_or(0.0) as f32,
    ]
}
pub fn standing_sword() -> Placement {
    Placement {
        p: Vec3::new(2.6, 1.50, -2.95),
        r: Quat::from_rotation_y(FRAC_PI_2) * Quat::from_rotation_z(-0.421),
    }
}
pub fn ground_key() -> Placement {
    Placement {
        p: InteractionKind::Key.target(),
        r: Quat::from_rotation_y(FRAC_PI_2),
    }
}
impl Hands {
    pub fn load(meshes: &mut Vec<MeshData>) -> Self {
        let spec: Value =
            serde_json::from_str(include_str!("../../../assets/end-game/v3/hands-rig.json"))
                .expect("hand rig sidecar");
        let mut loaded = load_glb(include_bytes!("../../../assets/end-game/v3/wolf-hands.glb"))
            .expect("articulated hands");
        let rows = spec["parts"].as_array().unwrap();
        let parts = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let name = row["name"].as_str().unwrap();
                let found = loaded
                    .iter()
                    .position(|p| p.name == name)
                    .expect("rig node exists exactly once");
                let part = loaded.remove(found);
                assert!(part.mesh.texture.is_some(), "hand atlas decoded");
                meshes.push(part.mesh);
                let parent = row["parent"]
                    .as_str()
                    .map(|n| rows.iter().position(|p| p["name"] == n).unwrap());
                assert!(parent.is_none_or(|p| p < index), "parent precedes child");
                let suffix = &name[7..];
                Part {
                    id: meshes.len() as u32,
                    parent,
                    pivot: vec(&row["pivot"]),
                    axis: vec(&row["curl_axis"]),
                    opposition: if row["opposition_axis"].is_array() {
                        vec(&row["opposition_axis"])
                    } else {
                        Vec3::Y
                    },
                    side: usize::from(name.starts_with("hand_l_")),
                    category: row["category"].as_str().unwrap_or(suffix).into(),
                    power: angles(&spec["presets"]["power_grip"][suffix]),
                    pinch: angles(&spec["presets"]["pinch"][suffix]),
                }
            })
            .collect();
        assert!(loaded.is_empty(), "unmapped hand mesh");
        let contact = |side: &str, grip: &str, field: &str| vec(&spec[side][grip][field]);
        Self {
            parts,
            pinch: [
                contact("contacts_right", "pinch", "point"),
                contact("contacts_left", "pinch", "point"),
            ],
            power: [
                contact("contacts_right", "power_grip", "point"),
                contact("contacts_left", "power_grip", "point"),
            ],
            power_axis: [
                contact("contacts_right", "power_grip", "axis"),
                contact("contacts_left", "power_grip", "axis"),
            ],
            skeleton: rig::humanoid(&HumanoidDims {
                upperarm_len: 0.30,
                forearm_len: 0.27,
                shoulder_w: 0.36,
                ..HumanoidDims::default()
            }),
        }
    }
    pub fn motion(&self, game: &Dungeon, v: &View) -> Motion {
        let bob = (game.time * 1.7).sin() * 0.006;
        let neutral = Quat::from_rotation_y(FRAC_PI_2 - v.yaw);
        let poses = std::array::from_fn(|side| HandPose {
            wrist: Placement {
                p: v.head + v.forward * 0.33 + v.right * (if side == 0 { 0.22 } else { -0.22 })
                    - v.up * (0.30 + bob),
                r: v.rot * Quat::from_rotation_y(FRAC_PI_2),
            },
            curl: 0.20 + (game.time * 1.7).sin() * 0.025,
            pinch: 0.0,
        });
        let mut m = Motion {
            poses,
            key: None,
            sword: None,
        };
        let swing = if game.attack_time > 0.0 {
            (game.attack_time / 0.7 * std::f32::consts::PI).sin()
        } else {
            0.0
        };
        let ready = Placement {
            p: v.head + v.forward * 0.30 + v.right * (0.12 - swing * 0.20) - v.up * 0.29,
            r: v.rot
                * Quat::from_rotation_y(1.0 - swing * 0.4)
                * Quat::from_rotation_z(0.9 - swing * 1.0),
        };
        if game.stage >= 4 {
            m.sword = Some(ready);
            self.sword_hands(&mut m, ready, 1.0);
        }
        if let Some(a) = game.interaction {
            let recovery = a.recover();
            match a.kind {
                InteractionKind::Board => {
                    let r = neutral;
                    let target = Placement {
                        p: board_grip(game.board_open) - r * self.pinch[0],
                        r,
                    };
                    m.poses[0].wrist = m.poses[0].wrist.mix(target, a.reach() * (1.0 - recovery));
                    m.poses[0].curl = a.grasp() * (1.0 - recovery) + poses[0].curl * recovery;
                    m.poses[0].pinch = 1.0 - recovery;
                }
                InteractionKind::Key => {
                    let lift = ease(0.84, 1.18, a.elapsed);
                    let pocket = ease(1.20, 1.68, a.elapsed);
                    let return_hand = ease(1.70, a.kind.duration(), a.elapsed);
                    let ground = ground_key();
                    let display = Placement {
                        p: v.head + v.forward * 0.34 + v.right * 0.09 - v.up * 0.08,
                        r: neutral * Quat::from_rotation_x(-0.15),
                    };
                    let stow = Placement {
                        p: v.head + v.right * 0.28 - v.up * 0.50 - v.forward * 0.05,
                        r: neutral,
                    };
                    let item = ground.mix(display, lift).mix(stow, pocket);
                    let target = Placement {
                        p: item.p - item.r * self.pinch[0],
                        r: item.r,
                    };
                    m.poses[0].wrist = m.poses[0]
                        .wrist
                        .mix(target, a.reach() * (1.0 - return_hand));
                    m.poses[0].curl = a.grasp() * (1.0 - return_hand) + poses[0].curl * return_hand;
                    m.poses[0].pinch = 1.0 - return_hand;
                    if (0.81..1.70).contains(&a.elapsed) {
                        m.key = Some(Placement {
                            p: m.poses[0].wrist.point(self.pinch[0]),
                            r: m.poses[0].wrist.r,
                        });
                    }
                }
                InteractionKind::Lock => {
                    let turn = ease(0.81, 1.16, a.elapsed);
                    let withdraw = ease(1.20, 1.40, a.elapsed);
                    let r = neutral * Quat::from_rotation_x(-turn * FRAC_PI_2);
                    // Shaft is 14 cm long; its tip enters the front of the lock.
                    let ring = a.kind.target() - neutral * Vec3::X * (0.115 - withdraw * 0.08);
                    let target = Placement {
                        p: ring - r * self.pinch[0],
                        r,
                    };
                    let pocket = Placement {
                        p: v.head + v.right * 0.28 - v.up * 0.50 - v.forward * 0.05,
                        r: neutral,
                    };
                    let return_hand = ease(1.68, a.kind.duration(), a.elapsed);
                    m.poses[0].wrist = poses[0]
                        .wrist
                        .mix(pocket, ease(0.0, 0.22, a.elapsed))
                        .mix(target, a.reach())
                        .mix(pocket, ease(1.40, 1.65, a.elapsed))
                        .mix(poses[0].wrist, return_hand);
                    m.poses[0].curl = 1.0 - return_hand + poses[0].curl * return_hand;
                    m.poses[0].pinch = 1.0 - return_hand;
                    if (0.22..1.68).contains(&a.elapsed) {
                        m.key = Some(Placement {
                            p: m.poses[0].wrist.point(self.pinch[0]),
                            r: m.poses[0].wrist.r,
                        });
                    }
                }
                InteractionKind::Sword => {
                    // Only the tip is seated in stone. Lift clear before rotating the blade.
                    let pull = ease(1.20, 1.94, a.elapsed);
                    let mut lifted = standing_sword();
                    lifted.p.y += ease(1.02, 1.20, a.elapsed) * 0.14;
                    let sword = lifted.mix(ready, pull);
                    m.sword = Some(sword);
                    self.sword_hands(&mut m, sword, a.reach());
                    for hand in &mut m.poses {
                        hand.curl = a.grasp();
                    }
                }
            }
        }
        m
    }
    fn sword_hands(&self, m: &mut Motion, item: Placement, reach: f32) {
        for side in 0..2 {
            let r = item.r * Quat::from_rotation_arc(self.power_axis[side].normalize(), Vec3::X);
            let socket = item.point(Vec3::new(if side == 0 { -0.13 } else { -0.25 }, 0.0, 0.0));
            let target = Placement {
                p: socket - r * self.power[side],
                r,
            };
            m.poses[side].wrist = m.poses[side].wrist.mix(target, reach);
            m.poses[side].curl = 1.0;
            m.poses[side].pinch = 0.0;
        }
    }
    fn skeleton_pose(&self, v: &View, m: &Motion) -> (Pose, [rig::ArmSolution; 2]) {
        let mut pose = Pose {
            root_pos: v.head - v.body_rot * Vec3::Y * 0.72,
            ..Pose::default()
        };
        pose.local_rot[0] = v.body_rot;
        let solved = std::array::from_fn(|side| {
            rig::solve_arm(
                &self.skeleton,
                &mut pose,
                if side == 0 {
                    ArmSide::Right
                } else {
                    ArmSide::Left
                },
                m.poses[side].wrist.p,
                Some(m.poses[side].wrist.r),
            )
        });
        (pose, solved)
    }
    pub fn draw(&self, out: &mut Vec<Instance>, v: &View, m: &Motion) {
        let (_, solved) = self.skeleton_pose(v, m);
        let mut transforms = vec![Mat4::IDENTITY; self.parts.len()];
        for (index, part) in self.parts.iter().enumerate() {
            let h = m.poses[part.side];
            let arm = solved[part.side];
            let matrix = if part.category == "ik_upperarm" || part.category == "ik_forearm" {
                let (start, end) = if part.category == "ik_upperarm" {
                    (arm.shoulder, arm.elbow)
                } else {
                    (arm.elbow, arm.wrist)
                };
                let r = Quat::from_rotation_arc(h.wrist.r * Vec3::X, (end - start).normalize())
                    * h.wrist.r;
                Mat4::from_rotation_translation(r, start - r * part.pivot)
            } else {
                let a = std::array::from_fn::<_, 2, _>(|i| {
                    part.power[i] + (part.pinch[i] - part.power[i]) * h.pinch
                });
                let r = Quat::from_axis_angle(part.opposition, a[1] * h.curl)
                    * Quat::from_axis_angle(part.axis, a[0] * h.curl);
                let local = Mat4::from_translation(part.pivot)
                    * Mat4::from_quat(r)
                    * Mat4::from_translation(-part.pivot);
                let parent = part
                    .parent
                    .filter(|&p| self.parts[p].category != "ik_forearm")
                    .map_or(Mat4::from_rotation_translation(h.wrist.r, arm.wrist), |p| {
                        transforms[p]
                    });
                parent * local
            };
            transforms[index] = matrix;
            let (_, rot, p) = matrix.to_scale_rotation_translation();
            out.push(
                Instance::new(p, Vec3::ONE, Vec3::ONE)
                    .with_mesh(part.id)
                    .with_rot(rot)
                    .with_surface(0.57, 0.30)
                    .without_shadow(),
            );
        }
    }
}
