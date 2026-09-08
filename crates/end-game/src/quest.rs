//! Physical quest markers and the final portcullis, driven by shared quest state.
use ember_engine::{Frame, Instance, MeshData, MeshVertex, Particle};
use end_game_core::{
    Dungeon,
    enemies::CastleEventKind,
    quest::{QuestKind, SITES},
};
use glam::{Quat, Vec2, Vec3};

#[derive(Default)]
struct Kit {
    vertices: Vec<MeshVertex>,
}
impl Kit {
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let cross = (b - a).cross(c - a);
        if cross.length_squared() < 0.0000000001 {
            return;
        }
        let normal = cross.normalize();
        for p in [a, b, c] {
            self.vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: normal.to_array(),
                uv: [0., 0.],
            });
        }
    }
    fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }
    fn ring(&mut self, radius: f32, tube: f32, y: f32) {
        for i in 0..32 {
            for j in 0..6 {
                let p = |a: usize, b: usize| {
                    let angle = a as f32 * std::f32::consts::TAU / 32.;
                    let cross = b as f32 * std::f32::consts::TAU / 6.;
                    Vec3::new(
                        angle.cos() * (radius + tube * cross.cos()),
                        y + tube * cross.sin(),
                        angle.sin() * (radius + tube * cross.cos()),
                    )
                };
                self.quad(p(i, j), p(i + 1, j), p(i + 1, j + 1), p(i, j + 1));
            }
        }
    }
    fn lathe(&mut self, profile: &[(f32, f32)]) {
        for pair in profile.windows(2) {
            for i in 0..24 {
                let p = |index: usize, point: (f32, f32)| {
                    let a = index as f32 * std::f32::consts::TAU / 24.;
                    Vec3::new(a.cos() * point.0, point.1, a.sin() * point.0)
                };
                self.quad(
                    p(i, pair[0]),
                    p(i + 1, pair[0]),
                    p(i + 1, pair[1]),
                    p(i, pair[1]),
                );
            }
        }
    }
    fn extrude(&mut self, points: &[Vec2], depth: f32) {
        let center = points.iter().copied().sum::<Vec2>() / points.len() as f32;
        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            let p = |index: usize, z: f32| points[index].extend(z);
            self.tri(center.extend(depth), p(i, depth), p(j, depth));
            self.tri(center.extend(0.), p(j, 0.), p(i, 0.));
            self.quad(p(i, 0.), p(j, 0.), p(j, depth), p(i, depth));
        }
    }
    fn register(self, meshes: &mut Vec<MeshData>) -> u32 {
        meshes.push(MeshData {
            vertices: self.vertices,
            texture: None,
        });
        meshes.len() as u32
    }
}
pub struct QuestScene {
    sun: u32,
    wolf: u32,
    bell: u32,
    crown: u32,
    font: u32,
    iron: u32,
}
fn item(out: &mut Vec<Instance>, id: u32, p: Vec3, s: Vec3, r: Quat, color: Vec3, metallic: f32) {
    out.push(
        Instance::new(p, s, color)
            .with_mesh(id)
            .with_rot(r)
            .with_surface(0.58, metallic),
    );
}
impl QuestScene {
    pub fn load(meshes: &mut Vec<MeshData>, iron: u32) -> Self {
        let mut sun = Kit::default();
        sun.ring(0.16, 0.023, 0.);
        for i in 0..12 {
            let a = i as f32 * std::f32::consts::TAU / 12.;
            let p = |angle: f32, r: f32| Vec3::new(angle.cos() * r, 0., angle.sin() * r);
            sun.tri(p(a - 0.10, 0.20), p(a, 0.29), p(a + 0.10, 0.20));
        }
        let sun = sun.register(meshes);
        let mut wolf = Kit::default();
        wolf.extrude(
            &[
                Vec2::new(-0.23, 0.26),
                Vec2::new(-0.04, 0.13),
                Vec2::new(0.02, 0.24),
                Vec2::new(0.10, 0.07),
                Vec2::new(0.28, -0.02),
                Vec2::new(0.15, -0.13),
                Vec2::new(0.02, -0.25),
                Vec2::new(-0.19, -0.10),
                Vec2::new(-0.17, 0.03),
            ],
            0.025,
        );
        let wolf = wolf.register(meshes);
        let mut bell = Kit::default();
        bell.lathe(&[
            (0.08, 0.30),
            (0.14, 0.27),
            (0.16, 0.05),
            (0.24, -0.17),
            (0.28, -0.20),
            (0.27, -0.24),
            (0.22, -0.21),
            (0.12, 0.04),
            (0.10, 0.22),
        ]);
        bell.ring(0.09, 0.021, 0.33);
        let bell = bell.register(meshes);
        let mut crown = Kit::default();
        crown.lathe(&[
            (0.13, -0.07),
            (0.16, -0.03),
            (0.16, 0.02),
            (0.13, 0.02),
            (0.13, -0.07),
        ]);
        for i in 0..7 {
            let a = i as f32 * std::f32::consts::TAU / 7.;
            let p = |angle: f32, y: f32, r: f32| Vec3::new(angle.cos() * r, y, angle.sin() * r);
            crown.quad(
                p(a - 0.16, 0., 0.15),
                p(a, 0.16, 0.18),
                p(a + 0.16, 0., 0.15),
                p(a, 0.035, 0.13),
            );
        }
        let crown = crown.register(meshes);
        let mut font = Kit::default();
        font.lathe(&[
            (0.25, 0.),
            (0.27, 0.03),
            (0.23, 0.12),
            (0.20, 0.15),
            (0.17, 0.12),
            (0.07, 0.035),
            (0., 0.035),
        ]);
        font.ring(0.21, 0.025, 0.13);
        let font = font.register(meshes);
        Self {
            sun,
            wolf,
            bell,
            crown,
            font,
            iron,
        }
    }
    pub fn draw(&self, frame: &mut Frame, game: &Dungeon) {
        let eye = frame.camera.eye;
        let out = &mut frame.instances;
        let bronze = Vec3::new(0.56, 0.37, 0.12);
        let gold = Vec3::new(0.92, 0.65, 0.24);
        let stone = Vec3::new(0.24, 0.25, 0.23);
        for site in SITES {
            if eye.distance(site.position) > 37. {
                continue;
            }
            let active = game.quest.active(site.kind);
            let color = if active { gold } else { bronze };
            match site.kind {
                QuestKind::Inscription => {
                    let p = site.position + Vec3::Y * 0.65;
                    item(
                        out,
                        0,
                        p,
                        Vec3::new(1.10, 0.55, 0.09),
                        Quat::IDENTITY,
                        stone,
                        0.,
                    );
                    for x in [-0.46, 0.46] {
                        item(
                            out,
                            self.iron,
                            Vec3::new(p.x + x, 7.7125, p.z),
                            Vec3::new(0.026, 10.575, 0.026),
                            Quat::IDENTITY,
                            bronze,
                            0.5,
                        );
                    }
                    item(
                        out,
                        self.sun,
                        p + Vec3::new(-0.34, 0., 0.057),
                        Vec3::splat(0.47),
                        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        color,
                        0.60,
                    );
                    item(
                        out,
                        self.wolf,
                        p + Vec3::new(0., 0., 0.057),
                        Vec3::splat(0.47),
                        Quat::IDENTITY,
                        color,
                        0.60,
                    );
                    item(
                        out,
                        self.bell,
                        p + Vec3::new(0.34, -0.02, 0.07),
                        Vec3::splat(0.43),
                        Quat::IDENTITY,
                        color,
                        0.60,
                    );
                }
                QuestKind::Sun => {
                    item(
                        out,
                        self.sun,
                        site.position + Vec3::Y * 0.04,
                        Vec3::splat(1.4),
                        Quat::IDENTITY,
                        color,
                        0.6,
                    );
                }
                QuestKind::Wolf => {
                    let p = site.position - Vec3::X * 0.92;
                    let r = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
                    item(out, 0, p, Vec3::new(0.66, 0.69, 0.12), r, stone, 0.);
                    item(
                        out,
                        self.wolf,
                        p + Vec3::X * 0.07,
                        Vec3::splat(1.0),
                        r,
                        color,
                        0.6,
                    );
                }
                QuestKind::Bell => {
                    let p = site.position;
                    for x in [-0.53, 0.53] {
                        item(
                            out,
                            self.iron,
                            p + Vec3::new(x, -0.06, 0.),
                            Vec3::new(0.065, 1.90, 0.065),
                            Quat::IDENTITY,
                            bronze,
                            0.45,
                        );
                    }
                    item(
                        out,
                        self.iron,
                        p + Vec3::Y * 0.87,
                        Vec3::new(1.15, 0.095, 0.095),
                        Quat::IDENTITY,
                        bronze,
                        0.45,
                    );
                    let age = game
                        .castle_events
                        .events()
                        .filter(|e| e.kind == CastleEventKind::Bell)
                        .last()
                        .map_or(100., |e| game.time - e.time);
                    let angle = if age < 3. {
                        (age * 10.).sin() * 0.34 * (-age * 1.3).exp()
                    } else {
                        0.
                    };
                    let r = Quat::from_rotation_z(angle);
                    let anchor = p + Vec3::Y * 0.72;
                    item(
                        out,
                        self.bell,
                        anchor + r * (-Vec3::Y * 0.36),
                        Vec3::splat(1.2),
                        r,
                        color,
                        0.6,
                    );
                    item(
                        out,
                        self.iron,
                        anchor + r * (-Vec3::Y * 0.39),
                        Vec3::new(0.035, 0.46, 0.035),
                        r,
                        Vec3::splat(0.23),
                        0.5,
                    );
                }
                QuestKind::Shrine(id) => {
                    let p = site.position;
                    item(
                        out,
                        0,
                        p - Vec3::Y * 0.44,
                        Vec3::new(0.31, 0.72, 0.31),
                        Quat::IDENTITY,
                        stone,
                        0.,
                    );
                    item(
                        out,
                        0,
                        p - Vec3::Y * 0.77,
                        Vec3::new(0.49, 0.10, 0.49),
                        Quat::IDENTITY,
                        stone,
                        0.,
                    );
                    item(
                        out,
                        self.font,
                        p - Vec3::Y * 0.10,
                        Vec3::ONE,
                        Quat::IDENTITY,
                        if active { bronze * 0.5 } else { bronze },
                        0.35,
                    );
                    if game.quest.shrine_available(id) {
                        for i in 0..5 {
                            let phase = (game.time * 0.35 + i as f32 * 0.2).fract();
                            frame.particles.push(Particle {
                                position: p + Vec3::new(
                                    (i as f32 * 2.4).sin() * 0.10,
                                    phase * 0.27,
                                    (i as f32 * 2.4).cos() * 0.10,
                                ),
                                color: Vec3::new(0.32, 0.70, 0.56),
                                size: Vec2::splat(0.022),
                                opacity: (1. - phase) * 0.48,
                            });
                        }
                    }
                }
                QuestKind::SallyPort => {}
            }
            if active
                && matches!(
                    site.kind,
                    QuestKind::Sun | QuestKind::Wolf | QuestKind::Bell
                )
            {
                for i in 0..5 {
                    let a = game.time * 0.6 + i as f32 * 1.256;
                    frame.particles.push(Particle {
                        position: site.position
                            - if site.kind == QuestKind::Wolf {
                                Vec3::X * 0.85
                            } else {
                                Vec3::ZERO
                            }
                            + Vec3::new(
                                a.sin() * 0.22,
                                0.10 + (a * 0.7).cos() * 0.18,
                                a.cos() * 0.22,
                            ),
                        color: gold,
                        size: Vec2::splat(0.018),
                        opacity: 0.4,
                    });
                }
            }
        }
        if let Some(p) = game.crown_position() {
            if p.distance(eye) < 35. {
                item(
                    out,
                    self.crown,
                    p - Vec3::Y * 0.12,
                    Vec3::ONE,
                    Quat::from_rotation_y(0.35),
                    gold,
                    0.7,
                );
                for i in 0..5 {
                    let a = game.time * 0.7 + i as f32 * 1.256;
                    frame.particles.push(Particle {
                        position: p + Vec3::new(
                            a.sin() * 0.22,
                            0.07 + (a * 0.8).sin() * 0.08,
                            a.cos() * 0.22,
                        ),
                        color: gold,
                        size: Vec2::splat(0.024),
                        opacity: 0.48,
                    });
                }
            }
        }
        if eye.z < -54. {
            let bounds = game.quest.gate_bounds();
            let center = Vec3::from_array(bounds.center());
            for index in 0..9 {
                item(
                    out,
                    self.iron,
                    Vec3::new(
                        center.x,
                        center.y,
                        bounds.min[2] + 0.13 + index as f32 * 0.267,
                    ),
                    Vec3::new(0.08, 3., 0.08),
                    Quat::IDENTITY,
                    Vec3::splat(0.28),
                    0.6,
                );
            }
            for y in [
                bounds.min[1] + 0.22,
                bounds.min[1] + 1.25,
                bounds.max[1] - 0.15,
            ] {
                item(
                    out,
                    self.iron,
                    Vec3::new(center.x, y, center.z),
                    Vec3::new(0.09, 0.09, 2.4),
                    Quat::IDENTITY,
                    Vec3::splat(0.28),
                    0.6,
                );
            }
            for z in [-81.32, -78.68] {
                item(
                    out,
                    0,
                    Vec3::new(-25.20, 7.65, z),
                    Vec3::new(0.64, 3.3, 0.28),
                    Quat::IDENTITY,
                    stone,
                    0.,
                );
            }
            item(
                out,
                0,
                Vec3::new(-25.20, 9.25, -80.),
                Vec3::new(0.64, 0.38, 2.92),
                Quat::IDENTITY,
                stone,
                0.,
            );
            if !game.quest.gate_unlocked {
                item(
                    out,
                    self.crown,
                    Vec3::new(-24.86, 7.2, -80.),
                    Vec3::splat(0.62),
                    Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
                    bronze,
                    0.65,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quest_kit_is_finite_untextured_and_stays_small() {
        let mut meshes = Vec::new();
        let _ = QuestScene::load(&mut meshes, 0);
        assert!(meshes.iter().all(|m| m.texture.is_none()));
        assert!(meshes.iter().map(|m| m.vertices.len() / 3).sum::<usize>() < 3000);
        for mesh in meshes {
            for v in mesh.vertices {
                assert!(
                    Vec3::from_array(v.pos).is_finite()
                        && Vec3::from_array(v.normal).is_normalized()
                );
            }
        }
    }
}
