//! Compact instanced castle kit. Collision bounds come exclusively from core.
use super::cell::hash;
use super::scene::{Model, draw_model, model};
use ember_engine::environment::PointLight;
use ember_engine::{
    Camera, Environment, Frame, Instance, MeshData, MeshVertex, Particle, TextureData,
};
use end_game_core::{
    Material,
    layout::{self, SolidKind, Surface, Zone},
};
use glam::{Quat, Vec2, Vec3};

struct Piece {
    instance: Instance,
    zone: Zone,
    detail: bool,
    front: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_uses_shared_bounds_and_reuses_generated_meshes() {
        let mut meshes = vec![
            MeshData::textured_box(1., None),
            MeshData::textured_box(1., None),
        ];
        let scene = CastleScene::build(&mut meshes, 2);
        for solid in &layout::castle().solids {
            if solid.kind == SolidKind::PropCollider
                || (solid.kind == SolidKind::Rail && solid.material == Surface::Iron)
            {
                continue;
            }
            assert!(
                scene.pieces.iter().any(|p| p.instance.mesh == 0
                    && p.instance.position == Vec3::from_array(solid.bounds.center())
                    && p.instance.scale == Vec3::from_array(solid.bounds.size())),
                "{} has no exact structural backing",
                solid.name
            );
        }
        for model in &scene.models {
            assert_eq!(model.ids.len(), 1);
        }
        let pillar = scene.models[0].ids[0].0;
        let mut frame = Frame::default();
        frame.camera.eye = Vec3::new(0., 1.7, -23.);
        frame.camera.target = frame.camera.eye - Vec3::Z;
        scene.draw(&mut frame, &Model { ids: vec![] }, 0.);
        assert_eq!(
            frame.instances.iter().filter(|i| i.mesh == pillar).count(),
            6,
            "all six pillar sockets should reference one uploaded mesh"
        );
        let prop_bytes: usize = scene
            .models
            .iter()
            .flat_map(|m| &m.ids)
            .map(|(id, _, _)| {
                meshes[*id as usize - 1]
                    .texture
                    .as_ref()
                    .unwrap()
                    .rgba8
                    .len()
            })
            .sum();
        assert_eq!(prop_bytes, 3 * 768 * 768 * 4);
        // Stone caps must sit beyond their backing: coplanar fronts flicker into
        // large triangular holes at the long camera distances in the courtyard.
        let wall = layout::castle()
            .solids
            .iter()
            .find(|s| s.zone == Zone::Hall && s.kind == SolidKind::Wall && s.bounds.min[0] > 1.9)
            .unwrap();
        let panel = scene
            .pieces
            .iter()
            .find(|p| {
                p.front
                    && p.zone == Zone::Hall
                    && (p.instance.rot * Vec3::Z).x > 0.9
                    && (p.instance.position.x - wall.bounds.max[0] - 0.003).abs() < 0.0001
            })
            .unwrap();
        let cap = panel.instance.position + panel.instance.rot * Vec3::new(0., 0., 0.018);
        assert!((cap.x - wall.bounds.max[0] - 0.021).abs() < 0.0001);
        let floor = scene
            .pieces
            .iter()
            .find(|p| p.instance.mesh == scene.paving && p.zone == Zone::Hall)
            .unwrap();
        assert!((floor.instance.position.y + 0.018 - 0.021).abs() < 0.0001);
    }

    #[test]
    fn dressed_faces_have_outward_normals_and_metre_uvs() {
        let mesh = face(
            1.25,
            0.65,
            Vec2::ZERO,
            TextureData {
                width: 1,
                height: 1,
                rgba8: vec![255; 4],
            },
        );
        for v in &mesh.vertices {
            let p = Vec3::from_array(v.pos);
            let n = Vec3::from_array(v.normal);
            assert!(n.is_normalized() && n.z > 0.);
            assert!(p.x.abs() <= 0.625 && p.y.abs() <= 0.325 && p.z >= 0. && p.z <= 0.018);
            assert!((Vec2::from_array(v.uv) - Vec2::new(p.x, p.y) * 1.15).length() < 0.00001);
        }
        let floor_rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        assert!((floor_rotation * Vec3::Z).distance(Vec3::Y) < 0.00001);
        assert!((floor_rotation * Vec3::new(0., 0., 0.018) - Vec3::Y * 0.018).length() < 0.00001);
    }
}
struct Lamp {
    p: Vec3,
    zone: Zone,
    yaw: f32,
}
pub struct CastleScene {
    pieces: Vec<Piece>,
    models: [Model; 3],
    lamps: Vec<Lamp>,
    stone: [u32; 3],
    paving: u32,
    beam: u32,
    grass: u32,
    leaf: u32,
    iron: u32,
}

fn register(meshes: &mut Vec<MeshData>, mesh: MeshData) -> u32 {
    meshes.push(mesh);
    meshes.len() as u32
}
fn atlas(bytes: &[u8], size: u32) -> TextureData {
    let source = TextureData::from_png_bytes(bytes).expect("castle stone atlas");
    let mut rgba8 = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let sx = (x * source.width / size).min(source.width - 1);
            let sy = (y * source.height / size).min(source.height - 1);
            let at = ((sy * source.width + sx) * 4) as usize;
            rgba8.extend_from_slice(&source.rgba8[at..at + 4]);
        }
    }
    TextureData {
        width: size,
        height: size,
        rgba8,
    }
}
fn triangle(out: &mut Vec<MeshVertex>, a: Vec3, b: Vec3, c: Vec3, offset: Vec2) {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    for p in [a, b, c] {
        out.push(MeshVertex {
            pos: p.to_array(),
            normal: normal.to_array(),
            uv: (Vec2::new(p.x, p.y) * 1.15 + offset).to_array(),
        });
    }
}
/// A shallow dressed face, backed by the exact collision solid. UVs use metres;
/// repeated modules retain their pitch, with only final boundary pieces trimmed.
fn face(width: f32, height: f32, offset: Vec2, texture: TextureData) -> MeshData {
    let a = width * 0.5;
    let b = height * 0.5;
    let bevel = 0.018;
    let outer = [
        Vec3::new(-a, -b, 0.),
        Vec3::new(a, -b, 0.),
        Vec3::new(a, b, 0.),
        Vec3::new(-a, b, 0.),
    ];
    let inner = [
        Vec3::new(-a + bevel, -b + bevel, bevel),
        Vec3::new(a - bevel, -b + bevel, bevel),
        Vec3::new(a - bevel, b - bevel, bevel),
        Vec3::new(-a + bevel, b - bevel, bevel),
    ];
    let mut vertices = Vec::new();
    triangle(&mut vertices, inner[0], inner[1], inner[2], offset);
    triangle(&mut vertices, inner[0], inner[2], inner[3], offset);
    for i in 0..4 {
        let n = (i + 1) % 4;
        triangle(&mut vertices, outer[i], outer[n], inner[n], offset);
        triangle(&mut vertices, outer[i], inner[n], inner[i], offset);
    }
    MeshData {
        vertices,
        texture: Some(texture),
    }
}
fn grass_texture() -> TextureData {
    let mut rgba8 = Vec::with_capacity(128 * 128 * 4);
    for y in 0..128u32 {
        for x in 0..128u32 {
            let n = hash(x * 97 + y * 7919);
            let coarse = hash((x / 8) * 47 + (y / 8) * 927);
            rgba8.extend_from_slice(&[
                (36. + n * 20. + coarse * 12.) as u8,
                (48. + n * 30. + coarse * 20.) as u8,
                (25. + n * 15.) as u8,
                255,
            ]);
        }
    }
    TextureData {
        width: 128,
        height: 128,
        rgba8,
    }
}
fn leaf_mesh() -> MeshData {
    let mut vertices = Vec::new();
    for angle in [0.0f32, 1.05, 2.1] {
        let r = Quat::from_rotation_y(angle);
        for sign in [-1., 1.] {
            triangle(
                &mut vertices,
                r * Vec3::new(-0.08, 0., 0.),
                r * Vec3::new(0.08, 0., 0.),
                r * Vec3::new(sign * 0.10, 0.38, 0.),
                Vec2::ZERO,
            );
        }
    }
    MeshData {
        vertices,
        texture: None,
    }
}

impl CastleScene {
    pub fn build(meshes: &mut Vec<MeshData>, iron: u32) -> Self {
        let stone_texture = atlas(super::cell::LIMESTONE_PNG, 512);
        let stone = std::array::from_fn(|i| {
            register(
                meshes,
                face(
                    1.25,
                    0.65,
                    Vec2::new(i as f32 * 0.31, i as f32 * 0.27),
                    stone_texture.clone(),
                ),
            )
        });
        let paving = register(
            meshes,
            face(1.5, 1.5, Vec2::new(0.17, 0.49), stone_texture.clone()),
        );
        let beam = register(meshes, MeshData::textured_box(1.15, Some(stone_texture)));
        let grass = register(meshes, MeshData::textured_plane(2., Some(grass_texture())));
        let leaf = register(meshes, leaf_mesh());
        let models = [
            model(
                meshes,
                include_bytes!("../../../assets/end-game/v9/gothic-pillar.glb"),
            ),
            model(
                meshes,
                include_bytes!("../../../assets/end-game/v9/courtyard-fountain.glb"),
            ),
            model(
                meshes,
                include_bytes!("../../../assets/end-game/v9/tower-doorway.glb"),
            ),
        ];
        let mut out = Self {
            pieces: Vec::new(),
            models,
            lamps: Vec::new(),
            stone,
            paving,
            beam,
            grass,
            leaf,
            iron,
        };
        for solid in &layout::castle().solids {
            if solid.kind == SolidKind::PropCollider {
                continue;
            }
            out.solid(solid);
        }
        out.architecture();
        out.garden();
        out
    }
    fn add(
        &mut self,
        mesh: u32,
        p: Vec3,
        scale: Vec3,
        rot: Quat,
        color: Vec3,
        zone: Zone,
        detail: bool,
        front: bool,
    ) {
        self.pieces.push(Piece {
            instance: Instance::new(p, scale, color)
                .with_mesh(mesh)
                .with_rot(rot)
                .with_surface(
                    if mesh == self.iron { 0.63 } else { 0.94 },
                    if mesh == self.iron { 0.55 } else { 0. },
                ),
            zone,
            detail,
            front,
        });
    }
    fn box_part(&mut self, p: Vec3, size: Vec3, zone: Zone, color: Vec3) {
        self.add(self.beam, p, size, Quat::IDENTITY, color, zone, true, false);
    }
    fn solid(&mut self, s: &layout::Solid) {
        let lo = Vec3::from_array(s.bounds.min);
        let hi = Vec3::from_array(s.bounds.max);
        let size = hi - lo;
        let center = (hi + lo) * 0.5;
        if s.kind == SolidKind::Rail && s.material == Surface::Iron {
            let along_x = size.x > size.z;
            self.add(
                self.iron,
                Vec3::new(center.x, hi.y - 0.055, center.z),
                if along_x {
                    Vec3::new(size.x, 0.065, 0.065)
                } else {
                    Vec3::new(0.065, 0.065, size.z)
                },
                Quat::IDENTITY,
                Vec3::splat(0.36),
                s.zone,
                false,
                false,
            );
            self.add(
                self.iron,
                Vec3::new(center.x, center.y, center.z),
                Vec3::new(0.052, size.y, 0.052),
                Quat::IDENTITY,
                Vec3::splat(0.34),
                s.zone,
                false,
                false,
            );
            return;
        }
        let base = match s.material {
            Surface::Grass => Vec3::new(0.13, 0.15, 0.09),
            Surface::Paving => Vec3::new(0.20, 0.205, 0.19),
            _ => Vec3::new(0.22, 0.215, 0.195),
        };
        self.add(0, center, size, Quat::IDENTITY, base, s.zone, false, false);
        if matches!(
            s.kind,
            SolidKind::Floor | SolidKind::Landing | SolidKind::Step
        ) {
            if s.material == Surface::Grass {
                // Fixed 4 m patches retain close-up texel pitch over the large lawn.
                let mut x = lo.x;
                while x < hi.x - 0.001 {
                    let end_x = (x + 4.0).min(hi.x);
                    let mut z = lo.z;
                    while z < hi.z - 0.001 {
                        let end_z = (z + 4.0).min(hi.z);
                        self.add(
                            self.grass,
                            Vec3::new((x + end_x) * 0.5, hi.y + 0.001, (z + end_z) * 0.5),
                            Vec3::new(end_x - x, 1., end_z - z),
                            Quat::IDENTITY,
                            Vec3::ONE,
                            s.zone,
                            false,
                            false,
                        );
                        z = end_z;
                    }
                    x = end_x;
                }
            } else {
                self.pave(
                    lo.x,
                    hi.x,
                    lo.z,
                    hi.z,
                    hi.y,
                    s.zone,
                    Vec3::new(0.70, 0.69, 0.64),
                );
            }
        }
        if s.kind == SolidKind::Wall || s.kind == SolidKind::Rail {
            let along_x = size.x >= size.z;
            let width = if along_x { size.x } else { size.z };
            let rows = (size.y / 0.65).ceil().max(1.) as usize;
            let h = size.y / rows as f32;
            for side in [-1., 1.] {
                let rotation = Quat::from_rotation_y(if along_x {
                    if side > 0. { 0. } else { std::f32::consts::PI }
                } else {
                    side * std::f32::consts::FRAC_PI_2
                });
                for row in 0..rows {
                    let mut start = -width * 0.5;
                    let offset = if row % 2 == 0 { 0.0 } else { 0.625 };
                    while start < width * 0.5 - 0.001 {
                        let end = (start
                            + if start == -width * 0.5 && offset > 0. {
                                offset
                            } else {
                                1.25
                            })
                        .min(width * 0.5);
                        let w = end - start;
                        let p = if along_x {
                            Vec3::new(
                                center.x + (start + end) * 0.5,
                                lo.y + (row as f32 + 0.5) * h,
                                center.z + side * (size.z * 0.5 + 0.003),
                            )
                        } else {
                            Vec3::new(
                                center.x + side * (size.x * 0.5 + 0.003),
                                lo.y + (row as f32 + 0.5) * h,
                                center.z + (start + end) * 0.5,
                            )
                        };
                        let seed =
                            s.index as u32 * 7919 + row as u32 * 173 + (start * 128.).abs() as u32;
                        let color = Vec3::new(0.67, 0.655, 0.60) * (0.86 + hash(seed) * 0.22);
                        self.add(
                            self.stone[(seed % 3) as usize],
                            p,
                            Vec3::new((w - 0.012) / 1.25, (h - 0.012) / 0.65, 1.),
                            rotation,
                            color,
                            s.zone,
                            true,
                            true,
                        );
                        start = end;
                    }
                }
            }
            if s.kind == SolidKind::Rail {
                self.box_part(
                    Vec3::new(center.x, hi.y - 0.04, center.z),
                    Vec3::new(size.x + 0.07, 0.12, size.z + 0.07),
                    s.zone,
                    Vec3::new(0.69, 0.68, 0.61),
                );
            }
        }
    }
    fn pave(&mut self, x0: f32, x1: f32, z0: f32, z1: f32, y: f32, zone: Zone, color: Vec3) {
        let mut x = x0;
        while x < x1 - 0.001 {
            let end_x = (x + 1.5).min(x1);
            let mut z = z0;
            while z < z1 - 0.001 {
                let end_z = (z + 1.5).min(z1);
                let noise = hash(((x * 16.).abs() as u32) * 71 + (z * 16.).abs() as u32);
                self.add(
                    self.paving,
                    Vec3::new((x + end_x) * 0.5, y + 0.003, (z + end_z) * 0.5),
                    Vec3::new(
                        (end_x - x - 0.012).max(0.02) / 1.5,
                        (end_z - z - 0.012).max(0.02) / 1.5,
                        1.,
                    ),
                    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                    color * (0.89 + noise * 0.18),
                    zone,
                    true,
                    true,
                );
                z = end_z;
            }
            x = end_x;
        }
    }
    fn bar(&mut self, a: Vec3, b: Vec3, width: f32, zone: Zone) {
        let d = b - a;
        self.add(
            self.beam,
            (a + b) * 0.5,
            Vec3::new(d.length(), width, width),
            Quat::from_rotation_arc(Vec3::X, d.normalize()),
            Vec3::new(0.67, 0.66, 0.61),
            zone,
            true,
            false,
        );
    }
    fn arch(&mut self, center: Vec3, rx: f32, ry: f32, width: f32, zone: Zone) {
        for i in 0..24 {
            let a = std::f32::consts::PI * i as f32 / 24.;
            let b = std::f32::consts::PI * (i + 1) as f32 / 24.;
            self.bar(
                center + Vec3::new(rx * a.cos(), ry * a.sin(), 0.),
                center + Vec3::new(rx * b.cos(), ry * b.sin(), 0.),
                width,
                zone,
            );
        }
    }
    fn architecture(&mut self) {
        for z in [-9.5, -14.5, -19.5] {
            self.arch(Vec3::new(0., 2.35, z), 1.94, 1.0, 0.22, Zone::Hall);
            for x in [-1.86, 1.86] {
                self.box_part(
                    Vec3::new(x, 1.18, z),
                    Vec3::new(0.20, 2.36, 0.28),
                    Zone::Hall,
                    Vec3::splat(0.62),
                );
            }
        }
        for z in [-25., -30., -40., -50., -53.] {
            self.arch(Vec3::new(0., 7.1, z), 13.35, 5.65, 0.30, Zone::GreatRoom);
            for x in [-13.45, 13.45] {
                self.box_part(
                    Vec3::new(x, 3.5, z),
                    Vec3::new(0.78, 7., 0.85),
                    Zone::GreatRoom,
                    Vec3::new(0.56, 0.55, 0.51),
                );
                self.box_part(
                    Vec3::new(x, 7., z),
                    Vec3::new(1.15, 0.32, 1.10),
                    Zone::GreatRoom,
                    Vec3::splat(0.7),
                );
            }
        }
        for z in [-30., -40., -50.] {
            for x in [-11., 11.] {
                self.box_part(
                    Vec3::new(x, 5.75, z),
                    Vec3::new(0.68, 2.70, 0.68),
                    Zone::GreatRoom,
                    Vec3::new(0.56, 0.55, 0.51),
                );
                self.box_part(
                    Vec3::new(x, 7.05, z),
                    Vec3::new(1.15, 0.24, 1.15),
                    Zone::GreatRoom,
                    Vec3::splat(0.67),
                );
            }
        }
        // A center aisle and its borders make the monumental room navigable.
        self.pave(
            -2.5,
            2.5,
            -54.,
            -22.,
            0.007,
            Zone::GreatRoom,
            Vec3::new(0.43, 0.38, 0.30),
        );
        for x in [-2.6, 2.6] {
            self.box_part(
                Vec3::new(x, 0.007, -38.),
                Vec3::new(0.16, 0.014, 32.),
                Zone::GreatRoom,
                Vec3::new(0.75, 0.69, 0.48),
            );
        }
        for (p, zone, yaw) in [
            (Vec3::new(1.72, 2.45, -10.), Zone::Hall, -1.57),
            (Vec3::new(-1.72, 2.45, -15.), Zone::Hall, 1.57),
            (Vec3::new(1.72, 2.45, -20.), Zone::Hall, -1.57),
            (Vec3::new(-12.9, 5.0, -30.), Zone::GreatRoom, 1.57),
            (Vec3::new(12.9, 5.0, -30.), Zone::GreatRoom, -1.57),
            (Vec3::new(-12.9, 5.0, -46.), Zone::GreatRoom, 1.57),
            (Vec3::new(12.9, 5.0, -46.), Zone::GreatRoom, -1.57),
            (Vec3::new(-2.8, 4.0, -57.), Zone::GrandStair, 1.57),
            (Vec3::new(2.8, 6.3, -61.), Zone::GrandStair, -1.57),
            (Vec3::new(15.35, 8.4, -88.8), Zone::Tower, 1.57),
            (Vec3::new(24.5, 12., -96.5), Zone::Tower, -1.57),
            (Vec3::new(15.4, 16., -87.5), Zone::Tower, 1.57),
            (Vec3::new(24.5, 20., -96.5), Zone::Tower, -1.57),
        ] {
            self.lamps.push(Lamp { p, zone, yaw });
        }
        // Merlons rise above the already solid parapet; no walking opening is closed.
        for z in (-101..=-65).step_by(2) {
            for x in [-25.28, 25.28] {
                self.box_part(
                    Vec3::new(x, 15.65, z as f32),
                    Vec3::new(0.68, 0.95, 0.9),
                    Zone::WallWalk,
                    Vec3::splat(0.60),
                );
            }
        }
        for x in (-24..=24).step_by(2) {
            for z in [-102.28, -63.72] {
                self.box_part(
                    Vec3::new(x as f32, 15.65, z),
                    Vec3::new(0.9, 0.95, 0.68),
                    Zone::WallWalk,
                    Vec3::splat(0.60),
                );
            }
        }
        for z in [-98.25, -85.75] {
            for x in (15..=25).step_by(2) {
                self.box_part(
                    Vec3::new(x as f32, 23.65, z),
                    Vec3::new(0.9, 0.95, 0.6),
                    Zone::Lookout,
                    Vec3::splat(0.65),
                );
            }
        }
        for z in (-97..=-87).step_by(2) {
            for x in [14.8, 25.3] {
                self.box_part(
                    Vec3::new(x, 23.65, z as f32),
                    Vec3::new(0.65, 0.95, 0.9),
                    Zone::Lookout,
                    Vec3::splat(0.65),
                );
            }
        }
    }
    fn garden(&mut self) {
        self.pave(
            -2.,
            2.,
            -100.,
            -64.,
            6.006,
            Zone::Garden,
            Vec3::new(0.67, 0.65, 0.56),
        );
        self.pave(
            2.,
            15.,
            -88.5,
            -86.5,
            6.006,
            Zone::Garden,
            Vec3::new(0.67, 0.65, 0.56),
        );
        self.pave(
            -9.4,
            -4.6,
            -82.4,
            -77.6,
            6.006,
            Zone::Garden,
            Vec3::new(0.65, 0.61, 0.49),
        );
        for side in [-1., 1.] {
            for bed in 0..5u32 {
                for plant in 0..32u32 {
                    let seed = bed * 139 + plant * 7919 + if side > 0. { 47 } else { 0 };
                    let p = Vec3::new(
                        side * (5. + hash(seed) * 14.),
                        6.01,
                        -68. - bed as f32 * 6. - hash(seed + 3) * 2.5,
                    );
                    if p.distance(Vec3::new(-7., 6., -80.)) < 3. || (p.z + 87.5).abs() < 1.5 {
                        continue;
                    }
                    let color = if plant % 9 == 0 {
                        Vec3::new(0.36, 0.20, 0.32)
                    } else {
                        Vec3::new(0.20, 0.33, 0.13)
                    };
                    self.add(
                        self.leaf,
                        p,
                        Vec3::splat(0.5 + hash(seed + 7) * 0.7),
                        Quat::from_rotation_y(hash(seed + 9) * core::f32::consts::TAU),
                        color,
                        Zone::Garden,
                        true,
                        false,
                    );
                }
            }
        }
        for p in [
            Vec3::new(-3., 8., -67.),
            Vec3::new(3., 8., -94.),
            Vec3::new(-18., 8., -83.),
            Vec3::new(12.8, 8., -85.),
        ] {
            self.lamps.push(Lamp {
                p,
                zone: Zone::Garden,
                yaw: 0.,
            });
            self.add(
                self.iron,
                p - Vec3::Y,
                Vec3::new(0.06, 2., 0.06),
                Quat::IDENTITY,
                Vec3::splat(0.35),
                Zone::Garden,
                false,
                false,
            );
        }
    }
    pub fn basement_detail(camera: &Camera) -> bool {
        let forward = (camera.target - camera.eye).normalize_or_zero();
        let portal = Vec3::new(0., 1.5, -7.) - camera.eye;
        camera.eye.z > -7.4
            || (camera.eye.z > -18. && portal.dot(forward) + 2.2 > portal.length() * 0.55)
    }
    fn zone_visible(zone: Zone, eye: Vec3) -> bool {
        match zone {
            Zone::Hall => eye.z > -46.,
            Zone::GreatRoom => eye.z > -75. || eye.y > 16.,
            Zone::GrandStair => eye.z < -8. && eye.z > -90.,
            Zone::Garden | Zone::WallWalk | Zone::Tower | Zone::Lookout => eye.z < -38.,
        }
    }
    pub fn draw(&self, frame: &mut Frame, torch: &Model, time: f32) {
        let eye = frame.camera.eye;
        let forward = (frame.camera.target - eye).normalize_or_zero();
        for piece in &self.pieces {
            if !Self::zone_visible(piece.zone, eye) {
                continue;
            }
            let d = piece.instance.position - eye;
            if piece.detail {
                if piece.zone == Zone::GreatRoom && eye.z > -9. {
                    continue;
                }
                if d.length() > 46. {
                    continue;
                }
                let radius = piece.instance.scale.max_element() * 1.2;
                if d.dot(forward) + radius < 0. && d.length() > 8. {
                    continue;
                }
                if piece.front && (piece.instance.rot * Vec3::Z).dot(-d) < -0.05 {
                    continue;
                }
            }
            frame.instances.push(piece.instance);
        }
        for prop in &layout::castle().props {
            if !Self::zone_visible(prop.zone, eye) {
                continue;
            }
            // The narrow entrance passage hides the side-bay pillars until its
            // mouth. Keep their high-detail meshes out of basement frames.
            if prop.zone == Zone::GreatRoom && eye.z > -20. {
                continue;
            }
            let p = Vec3::from_array(prop.position);
            let d = p + Vec3::Y * 2. - eye;
            if d.length() > if eye.z > -22. { 35. } else { 60. } || d.dot(forward) + 3. < 0. {
                continue;
            }
            let index = match prop.name {
                "gothic-pillar" => 0,
                "courtyard-fountain" => 1,
                "tower-doorway" => 2,
                _ => continue,
            };
            draw_model(
                &mut frame.instances,
                &self.models[index],
                p,
                prop.scale,
                Quat::from_rotation_y(prop.yaw),
                Material::Stone,
                false,
            );
        }
        for lamp in &self.lamps {
            if !Self::zone_visible(lamp.zone, eye) || lamp.p.distance(eye) > 35. {
                continue;
            }
            if lamp.zone == Zone::GreatRoom && eye.z > -20. {
                continue;
            }
            draw_model(
                &mut frame.instances,
                torch,
                lamp.p - Vec3::Y * 0.5,
                0.85,
                Quat::from_rotation_y(lamp.yaw),
                Material::Iron,
                false,
            );
            for i in 0..6u32 {
                let phase = (time * 0.85 + i as f32 * 0.17).fract();
                frame.particles.push(Particle {
                    position: lamp.p + Vec3::new((hash(i + 9) - 0.5) * 0.09, phase * 0.35, 0.),
                    color: Vec3::new(1., 0.49 + phase * 0.22, 0.12),
                    size: Vec2::new(0.035, 0.13) * (1. - phase * 0.6),
                    opacity: 0.8 * (1. - phase),
                });
            }
        }
    }
    pub fn light(&self, frame: &mut Frame, time: f32) {
        let eye = frame.camera.eye;
        let entry = ((-eye.z - 5.) / 6.).clamp(0., 1.);
        if entry <= 0. {
            return;
        }
        let entry = entry * entry * (3. - 2. * entry);
        let basement = frame.environment;
        let outdoor =
            (((-eye.z - 60.) / 5.).clamp(0., 1.) * ((eye.y - 5.) / 2.).clamp(0., 1.)).clamp(0., 1.);
        let mut env = Environment {
            enabled: true,
            sun_direction: Vec3::new(-0.48, 0.34, 0.64).normalize(),
            sun_color: Vec3::new(1., 0.64, 0.38),
            sun_intensity: 0.22 + outdoor * 0.85,
            sky_zenith: Vec3::new(0.09, 0.12, 0.20).lerp(Vec3::new(0.18, 0.27, 0.43), outdoor),
            sky_horizon: Vec3::new(0.12, 0.115, 0.095).lerp(Vec3::new(0.60, 0.38, 0.23), outdoor),
            cloud_coverage: 0.32 * outdoor,
            time,
            shadow_extent: 24. + outdoor * 26.,
            ..Environment::default()
        };
        let mut lamps: Vec<PointLight> = self
            .lamps
            .iter()
            .filter(|l| Self::zone_visible(l.zone, eye))
            .map(|lamp| PointLight {
                position: lamp.p,
                color: Vec3::new(1., 0.61, 0.30),
                intensity: match lamp.zone {
                    Zone::GreatRoom => 38.,
                    Zone::GrandStair => 8.,
                    _ => 6.,
                } * (1. + (time * 8.3).sin() * 0.045)
                    * entry,
                radius: if lamp.zone == Zone::GreatRoom {
                    26.
                } else {
                    12.
                },
            })
            .collect();
        if entry < 1. {
            lamps.extend(basement.lights.into_iter().map(|mut light| {
                light.intensity *= 1. - entry;
                light
            }));
        }
        lamps.sort_by(|a, b| {
            a.position
                .distance_squared(eye)
                .total_cmp(&b.position.distance_squared(eye))
        });
        for (i, lamp) in lamps.into_iter().take(4).enumerate() {
            env.lights[i] = lamp;
        }
        env.sun_direction = basement
            .sun_direction
            .lerp(env.sun_direction, entry)
            .normalize();
        env.sun_color = basement.sun_color.lerp(env.sun_color, entry);
        env.sun_intensity =
            basement.sun_intensity + (env.sun_intensity - basement.sun_intensity) * entry;
        env.sky_zenith = basement.sky_zenith.lerp(env.sky_zenith, entry);
        env.sky_horizon = basement.sky_horizon.lerp(env.sky_horizon, entry);
        env.shadow_extent =
            basement.shadow_extent + (env.shadow_extent - basement.shadow_extent) * entry;
        frame.environment = env;
        let fog = Vec3::from_array([
            0.055 + outdoor * 0.07,
            0.062 + outdoor * 0.06,
            0.078 + outdoor * 0.055,
        ]);
        frame.fog.color = Vec3::from_array(frame.fog.color)
            .lerp(fog, entry)
            .to_array();
        frame.fog.density += (0.008 + outdoor * 0.002 - frame.fog.density) * entry;
    }
}
