//! Material-built dungeon: seeded surface coordinates, metres, authored collision in core.
use ember_engine::environment::PointLight;
use ember_engine::{
    Camera, Environment, Fog, Frame, Instance, MeshData, MeshVertex, Particle, TextureData,
    assets::load_glb,
};
use end_game_core::{Dungeon, Material};
use glam::{Quat, Vec2, Vec3};

pub struct Model {
    pub ids: Vec<(u32, Vec3, Option<Material>)>,
}
pub struct Scene {
    static_world: Vec<Instance>,
    oak: u32,
    iron: u32,
    wolf: Model,
    werewolf: Model,
    sword: Model,
    warden: Model,
    cot: Model,
    torch: Model,
}

fn hash(mut seed: u32) -> f32 {
    seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
    seed = ((seed >> ((seed >> 28) + 4)) ^ seed).wrapping_mul(277803737);
    ((seed ^ (seed >> 22)) & 65535) as f32 / 65535.0
}
fn model(meshes: &mut Vec<MeshData>, bytes: &[u8]) -> Model {
    let parts = load_glb(bytes).expect("validated End Game GLB");
    let ids = parts
        .into_iter()
        .map(|part| {
            let color = if part.mesh.texture.is_some() {
                Vec3::ONE
            } else {
                Vec3::from_array(part.color)
            };
            meshes.push(part.mesh);
            (meshes.len() as u32, color, None)
        })
        .collect();
    Model { ids }
}
fn armor(meshes: &mut Vec<MeshData>, bytes: &[u8]) -> Model {
    let mut ids = Vec::new();
    for part in load_glb(bytes).expect("validated armor GLB") {
        let height = part
            .mesh
            .vertices
            .iter()
            .map(|v| v.pos[1])
            .fold(0.0_f32, f32::max);
        let mut groups = [Vec::new(), Vec::new(), Vec::new()];
        for tri in part.mesh.vertices.chunks_exact(3) {
            let p = tri.iter().map(|v| Vec3::from_array(v.pos)).sum::<Vec3>() / 3.0;
            // Prototype spatial material zones: cape behind the torso, leather belt,
            // iron shell elsewhere. A future authored material ID map replaces this.
            let zone = if p.x < -0.16 && p.y < height * 0.78 && p.y > 0.15 {
                2
            } else if (p.y - height * 0.43).abs() < 0.09 {
                1
            } else {
                0
            };
            groups[zone].extend_from_slice(tri);
        }
        for (vertices, material) in
            groups
                .into_iter()
                .zip([Material::Iron, Material::Leather, Material::Cloth])
        {
            if vertices.is_empty() {
                continue;
            }
            meshes.push(MeshData {
                vertices,
                texture: part.mesh.texture.clone(),
            });
            ids.push((meshes.len() as u32, Vec3::ONE, Some(material)));
        }
    }
    Model { ids }
}
fn draw_model(
    out: &mut Vec<Instance>,
    model: &Model,
    position: Vec3,
    scale: f32,
    rot: Quat,
    surface: Material,
    first_person: bool,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let first_person = first_person || std::env::var_os("END_GAME_NO_MODEL_SHADOW").is_some();
    for &(id, color, material) in &model.ids {
        let properties = material.unwrap_or(surface).properties();
        let item = Instance::new(position, Vec3::splat(scale), color)
            .with_mesh(id)
            .with_rot(rot)
            .with_surface(properties.roughness, properties.metallic);
        out.push(if first_person {
            item.without_shadow()
        } else {
            item
        });
    }
}

fn box_vertices(target: &mut Vec<MeshVertex>, position: Vec3, size: Vec3, rot: Quat, seed: u32) {
    // Every face samples its own deterministic patch and direction in the material.
    // Textures are shared at material level, avoiding one allocation per stone.
    let mesh = MeshData::textured_box(1.0, None);
    for (face, triangle) in mesh.vertices.chunks(6).enumerate() {
        let u = hash(seed.wrapping_add(face as u32 * 331)) * 13.0;
        let v = hash(seed.wrapping_add(face as u32 * 7919 + 41)) * 13.0;
        for vertex in triangle {
            let pos = position + rot * (Vec3::from_array(vertex.pos) * size);
            let normal = rot * Vec3::from_array(vertex.normal);
            target.push(MeshVertex {
                pos: pos.to_array(),
                normal: normal.to_array(),
                uv: [vertex.uv[0] * 0.55 + u, vertex.uv[1] * 0.55 + v],
            });
        }
    }
}

fn vault_stone(target: &mut Vec<MeshVertex>, z: f32, segment: u32) {
    let a = -std::f32::consts::FRAC_PI_2 + segment as f32 * std::f32::consts::PI / 28.0 + 0.002;
    let b = a + std::f32::consts::PI / 28.0 - 0.004;
    let point = |angle: f32, outer: bool, depth: f32| {
        Vec3::new(
            angle.sin() * if outer { 5.06 } else { 4.85 },
            1.6 + angle.cos() * if outer { 2.67 } else { 2.4 },
            depth,
        )
    };
    let p = [
        point(a, true, z - 0.22),
        point(b, true, z - 0.22),
        point(b, false, z - 0.22),
        point(a, false, z - 0.22),
        point(a, true, z + 0.22),
        point(b, true, z + 0.22),
        point(b, false, z + 0.22),
        point(a, false, z + 0.22),
    ];
    let center = p.iter().copied().sum::<Vec3>() / 8.0;
    for face in [
        [0, 1, 2, 3],
        [7, 6, 5, 4],
        [0, 4, 5, 1],
        [3, 2, 6, 7],
        [0, 3, 7, 4],
        [1, 5, 6, 2],
    ] {
        let mut n = (p[face[1]] - p[face[0]])
            .cross(p[face[2]] - p[face[0]])
            .normalize();
        let mid = face.map(|i| p[i]).iter().copied().sum::<Vec3>() / 4.0;
        if n.dot(mid - center) < 0.0 {
            n = -n;
        }
        let uv = [[0.0, 0.0], [0.5, 0.0], [0.5, 0.5], [0.0, 0.5]];
        for index in [0, 1, 2, 0, 2, 3] {
            target.push(MeshVertex {
                pos: p[face[index]].to_array(),
                normal: n.to_array(),
                uv: [
                    uv[index][0] + hash(segment) * 8.0,
                    uv[index][1] + hash(segment + 7) * 8.0,
                ],
            });
        }
    }
}

impl Scene {
    pub fn build() -> (Self, Vec<MeshData>) {
        let mut meshes = vec![
            MeshData::textured_box(
                1.0,
                Some(
                    TextureData::from_png_bytes(include_bytes!(
                        "../../../assets/end-game/v1/oak.png"
                    ))
                    .unwrap(),
                ),
            ),
            MeshData::textured_box(
                1.0,
                Some(
                    TextureData::from_png_bytes(include_bytes!(
                        "../../../assets/end-game/v1/iron.png"
                    ))
                    .unwrap(),
                ),
            ),
            MeshData::textured_box(
                1.0,
                Some(
                    TextureData::from_png_bytes(include_bytes!(
                        "../../../assets/end-game/v1/stone.png"
                    ))
                    .unwrap(),
                ),
            ),
        ];
        let mut oak = Vec::new();
        let mut stone = Vec::new();
        let mut iron = Vec::new();
        let mut seed = 100u32;
        let mut block = |vertices: &mut Vec<MeshVertex>, p, s, q| {
            seed += 1;
            box_vertices(vertices, p, s, q, seed);
        };
        let identity = Quat::IDENTITY;
        // Structural masonry backs the joints: mortar cracks never reveal the sky.
        for x in [-3.40, 3.40] {
            block(
                &mut stone,
                Vec3::new(x, 1.8, 2.65),
                Vec3::new(0.30, 3.7, 5.5),
                identity,
            );
        }
        for x in [-5.38, 5.38] {
            block(
                &mut stone,
                Vec3::new(x, 2.2, -3.5),
                Vec3::new(0.34, 4.5, 7.4),
                identity,
            );
        }
        block(
            &mut stone,
            Vec3::new(0.0, 1.8, 5.4),
            Vec3::new(7.1, 3.7, 0.3),
            identity,
        );
        block(
            &mut stone,
            Vec3::new(0.0, 2.2, -7.4),
            Vec3::new(10.9, 4.5, 0.3),
            identity,
        );
        for x in [-4.2, 4.2] {
            block(
                &mut stone,
                Vec3::new(x, 2.2, 0.3),
                Vec3::new(2.0, 4.5, 0.35),
                identity,
            );
        }
        for x in 0..6 {
            block(
                &mut oak,
                Vec3::new(-0.75 + x as f32 * 0.3, 1.35, -7.18),
                Vec3::new(0.285, 2.7, 0.18),
                identity,
            );
        }
        // Individually fitted boards, seams, joists and square iron nails.
        for row in 0..20 {
            for col in 0..3 {
                block(
                    &mut oak,
                    Vec3::new(-2.85 + row as f32 * 0.3, -0.085, 0.85 + col as f32 * 1.65),
                    Vec3::new(0.286, 0.16, 1.63),
                    identity,
                );
                for end in [-0.68, 0.68] {
                    block(
                        &mut iron,
                        Vec3::new(
                            -2.85 + row as f32 * 0.3,
                            0.002,
                            0.85 + col as f32 * 1.65 + end,
                        ),
                        Vec3::new(0.025, 0.006, 0.025),
                        identity,
                    );
                }
            }
        }
        // Masonry with staggered courses and small geometric variation.
        for y in 0..9 {
            for z in 0..7 {
                for x in [-3.18, 3.18] {
                    block(
                        &mut stone,
                        Vec3::new(x, 0.2 + y as f32 * 0.4, 0.37 + z as f32 * 0.76),
                        Vec3::new(0.38, 0.38, 0.73),
                        identity,
                    );
                }
            }
            for x in 0..9 {
                block(
                    &mut stone,
                    Vec3::new(
                        -2.9 + x as f32 * 0.72 + (y % 2) as f32 * 0.2,
                        0.2 + y as f32 * 0.4,
                        5.15,
                    ),
                    Vec3::new(0.70, 0.38, 0.40),
                    identity,
                );
            }
            for z in 0..10 {
                for x in [-5.15, 5.15] {
                    block(
                        &mut stone,
                        Vec3::new(x, 0.2 + y as f32 * 0.4, -0.36 - z as f32 * 0.73),
                        Vec3::new(0.38, 0.38, 0.70),
                        identity,
                    );
                }
            }
            for x in 0..14 {
                let xx = -4.72 + x as f32 * 0.72;
                if xx.abs() > 1.0 || y > 6 {
                    block(
                        &mut stone,
                        Vec3::new(xx, 0.2 + y as f32 * 0.4, -7.1),
                        Vec3::new(0.70, 0.38, 0.4),
                        identity,
                    );
                }
            }
        }
        for x in 0..14 {
            for z in 0..10 {
                let id = x * 31 + z;
                block(
                    &mut stone,
                    Vec3::new(
                        -4.64 + x as f32 * 0.715,
                        -0.09 + hash(id) * 0.012,
                        -0.35 - z as f32 * 0.7,
                    ),
                    Vec3::new(0.698, 0.17, 0.684),
                    identity,
                );
            }
        }
        // Barrel-vault ribs and a dark stone ceiling.
        for z in [-0.7, -3.6, -6.5] {
            for x in [-4.75, 4.75] {
                block(
                    &mut stone,
                    Vec3::new(x, 1.4, z),
                    Vec3::new(0.48, 2.8, 0.5),
                    identity,
                );
            }
        }
        for z in [-0.7, -3.6, -6.5] {
            for i in 0..28 {
                vault_stone(&mut stone, z, i);
            }
        }
        block(
            &mut stone,
            Vec3::new(0.0, 4.35, -3.5),
            Vec3::new(10.3, 0.35, 7.4),
            identity,
        );
        block(
            &mut stone,
            Vec3::new(0.0, 3.7, 2.6),
            Vec3::new(6.5, 0.30, 5.3),
            identity,
        );
        for z in [0.6, 2.6, 4.6] {
            block(
                &mut oak,
                Vec3::new(0.0, 3.42, z),
                Vec3::new(6.0, 0.24, 0.22),
                identity,
            );
        }
        for i in 0..21 {
            let x = -3.0 + i as f32 * 0.30;
            if x.abs() > 0.75 {
                block(
                    &mut iron,
                    Vec3::new(x, 1.6, 0.0),
                    Vec3::new(0.048, 3.2, 0.055),
                    identity,
                );
            }
        }
        for y in [0.17, 2.7, 3.25] {
            block(
                &mut iron,
                Vec3::new(0.0, y, 0.0),
                Vec3::new(6.2, 0.08, 0.09),
                identity,
            );
        }
        // Sword stone, made from individually oriented mineral blocks.
        for i in 0..9 {
            block(
                &mut stone,
                Vec3::new(
                    2.6 + (hash(i + 9) - 0.5) * 0.8,
                    0.25 + hash(i + 11) * 0.25,
                    -4.6 + (hash(i + 4) - 0.5) * 0.7,
                ),
                Vec3::new(0.72, 0.64, 0.68),
                Quat::from_rotation_y(hash(i) * 4.0) * Quat::from_rotation_z(hash(i + 6) * 0.4),
            );
        }
        let mut static_world = Vec::new();
        for (vertices, source, material) in [
            (oak, 0, Material::Oak),
            (iron, 1, Material::Iron),
            (stone, 2, Material::Stone),
        ] {
            let texture = meshes[source].texture.clone();
            meshes.push(MeshData { vertices, texture });
            let p = material.properties();
            static_world.push(
                Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::ONE)
                    .with_mesh(meshes.len() as u32)
                    .with_surface(p.roughness, p.metallic),
            );
        }
        let wolf = armor(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v1/wolf.glb"),
        );
        let werewolf = armor(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v1/werewolf.glb"),
        );
        let sword = model(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v1/sword.glb"),
        );
        let warden = model(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v1/warden.glb"),
        );
        let cot = model(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v1/cot.glb"),
        );
        let torch = model(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v1/torch.glb"),
        );
        (
            Self {
                static_world,
                oak: 1,
                iron: 2,
                wolf,
                werewolf,
                sword,
                warden,
                cot,
                torch,
            },
            meshes,
        )
    }

    pub fn frame(&self, game: &Dungeon, third_person: bool, wake: f32) -> Frame {
        let fwd = game.forward();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let head = game.position
            + Vec3::Y
                * if game.crouched {
                    1.0
                } else {
                    1.65 - wake * 1.18
                };
        let view = fwd * game.pitch.cos() + Vec3::Y * game.pitch.sin();
        let view_rot = Quat::from_rotation_y(-game.yaw) * Quat::from_rotation_x(game.pitch);
        let view_up = view_rot * Vec3::Y;
        let transform_view = game.transformation > 0.0;
        let see_hero = third_person || transform_view;
        let camera = if see_hero {
            let offset = if transform_view {
                fwd * 2.2 + right * 1.1
            } else {
                -fwd * 2.1 + right * 0.6
            };
            let eye = Vec3::new(
                (head.x + offset.x).clamp(-2.5, 4.4),
                head.y + 0.25,
                (head.z + offset.z).clamp(-6.5, 4.6),
            );
            Camera {
                eye,
                target: game.position + Vec3::Y * 1.25,
                fov_y_deg: 64.0,
            }
        } else {
            Camera {
                eye: head,
                target: head + view,
                fov_y_deg: 76.0,
            }
        };
        let t = game.time;
        let flicker = 1.0 + (t * 8.0).sin() * 0.055 + (t * 13.7).sin() * 0.04;
        let positions = [
            Vec3::new(2.72, 2.05, 2.0),
            Vec3::new(-4.65, 2.15, -2.6),
            Vec3::new(4.65, 2.15, -4.9),
        ];
        let mut env = Environment {
            enabled: true,
            sun_direction: Vec3::new(-0.3, 0.7, -0.65).normalize(),
            sun_color: Vec3::new(0.38, 0.54, 0.86),
            sun_intensity: 0.18,
            sky_zenith: Vec3::splat(0.008),
            sky_horizon: Vec3::new(0.035, 0.06, 0.095),
            cloud_coverage: 0.0,
            time: t,
            shadow_extent: 16.0,
            ..Environment::default()
        };
        for (i, position) in positions.iter().enumerate() {
            env.lights[i] = PointLight {
                position: *position,
                color: Vec3::new(1.0, 0.42, 0.12),
                intensity: 12.0 * flicker,
                radius: 7.0,
            };
        }
        if game.stage < 4 {
            env.lights[3] = PointLight {
                position: Vec3::new(2.6, 1.2, -4.6),
                color: Vec3::new(0.22, 0.48, 1.0),
                intensity: 2.0,
                radius: 3.0,
            };
        }
        let mut frame = Frame {
            camera,
            environment: env,
            fog: Fog {
                color: [0.012, 0.019, 0.03],
                density: 0.022,
            },
            instances: self.static_world.clone(),
            ..Frame::default()
        };
        let out = &mut frame.instances;
        let gate_shift = if game.stage >= 3 { 1.55 } else { 0.0 };
        for i in 0..5 {
            out.push(
                Instance::new(
                    Vec3::new(-0.60 + i as f32 * 0.3 + gate_shift, 1.55, 0.04),
                    Vec3::new(0.055, 3.0, 0.07),
                    Vec3::ONE,
                )
                .with_mesh(self.iron)
                .with_surface(0.38, 0.9),
            );
        }
        for y in [0.1, 1.0, 2.8] {
            out.push(
                Instance::new(
                    Vec3::new(gate_shift, y, 0.04),
                    Vec3::new(1.40, 0.075, 0.10),
                    Vec3::ONE,
                )
                .with_mesh(self.iron)
                .with_surface(0.4, 0.9),
            );
        }
        out.push(
            Instance::new(
                Vec3::new(gate_shift + 0.45, 1.05, 0.13),
                Vec3::new(0.17, 0.22, 0.08),
                Vec3::new(0.63, 0.43, 0.19),
            )
            .with_mesh(self.iron)
            .with_surface(0.48, 0.72),
        );
        draw_model(
            out,
            &self.cot,
            Vec3::new(-2.25, 0.0, 3.55),
            1.0,
            Quat::from_rotation_y(0.0),
            Material::Oak,
            false,
        );
        let breath = (t * 1.6).sin() * 0.014;
        let warden_rot = if game.warden_health == 0.0 {
            Quat::from_rotation_z(-1.4)
        } else {
            Quat::from_rotation_y(-0.5) * Quat::from_rotation_x(breath)
        };
        draw_model(
            out,
            &self.warden,
            Vec3::new(game.warden.x, breath, game.warden.y),
            1.0,
            warden_rot,
            Material::Leather,
            false,
        );
        for (i, p) in positions.iter().enumerate() {
            draw_model(
                out,
                &self.torch,
                *p - Vec3::Y * 0.5,
                1.0,
                Quat::from_rotation_y(if i == 1 { 0.0 } else { std::f32::consts::PI }),
                Material::Iron,
                false,
            );
            for j in 0..16 {
                let phase = (t * (0.48 + hash(j + 2) * 0.6) + hash(j * 71)) % 1.0;
                frame.particles.push(Particle {
                    position: *p
                        + Vec3::new(
                            (hash(j * 97) - 0.5) * 0.15,
                            phase * 0.9,
                            (hash(j + 99) - 0.5) * 0.16,
                        ),
                    color: Vec3::new(1.0, 0.3 + phase * 0.4, 0.05),
                    size: Vec2::splat((1.0 - phase) * 0.12 + 0.012),
                    opacity: (1.0 - phase) * 0.85,
                });
            }
        }
        let plank_angle = if game.stage > 0 { -0.22 } else { 0.025 };
        out.push(
            Instance::new(
                Vec3::new(-1.25, 0.035 + if game.stage > 0 { 0.15 } else { 0.0 }, 2.1),
                Vec3::new(0.30, 0.07, 1.30),
                Vec3::new(0.9, 0.84, 0.72),
            )
            .with_mesh(self.oak)
            .with_rot(Quat::from_rotation_x(plank_angle))
            .with_surface(0.95, 0.0),
        );
        if game.stage == 1 {
            out.push(
                Instance::new(
                    Vec3::new(-1.0, 0.08, 2.1),
                    Vec3::new(0.05, 0.03, 0.24),
                    Vec3::new(1.0, 0.65, 0.2),
                )
                .with_surface(0.3, 0.8),
            );
            frame.particles.push(Particle {
                position: Vec3::new(-1.0, 0.2, 2.1),
                color: Vec3::new(1.0, 0.64, 0.22),
                size: Vec2::splat(0.16),
                opacity: 0.5,
            });
        }
        let body = &game.body;
        out.push(
            Instance::new(
                body.position,
                body.size,
                Vec3::splat(1.0 - body.wear * 0.25),
            )
            .with_mesh(self.oak)
            .with_surface(0.9, 0.0),
        );
        for x in [-0.28, 0.28] {
            out.push(
                Instance::new(
                    body.position + Vec3::new(x, 0.0, 0.0),
                    Vec3::new(0.025, 0.66, 0.67),
                    Vec3::splat(0.45),
                )
                .with_mesh(self.iron),
            );
        }
        for x in [-0.65, -0.32, 0.0, 0.32, 0.65] {
            out.push(
                Instance::new(
                    Vec3::new(x, 1.35, -7.0),
                    Vec3::new(0.07, 2.7, 0.08),
                    Vec3::ONE,
                )
                .with_mesh(self.iron),
            );
        }
        if game.stage < 5 {
            out.push(
                Instance::new(
                    Vec3::new(0.0, 1.05, -6.88),
                    Vec3::new(1.4, 0.065, 0.065),
                    Vec3::new(0.85, 0.62, 0.35),
                )
                .with_mesh(self.iron)
                .with_rot(Quat::from_rotation_z(0.35)),
            );
        }
        if game.stage < 4 {
            draw_model(
                out,
                &self.sword,
                Vec3::new(2.6, 1.9, -4.6),
                1.0,
                Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
                Material::Iron,
                false,
            );
        }
        if see_hero {
            let hero = if game.werewolf {
                &self.werewolf
            } else {
                &self.wolf
            };
            draw_model(
                out,
                hero,
                game.position,
                if game.crouched { 0.8 } else { 1.0 },
                Quat::from_rotation_y(-game.yaw + std::f32::consts::FRAC_PI_2),
                Material::Iron,
                false,
            );
            if game.stage >= 4 {
                draw_model(
                    out,
                    &self.sword,
                    game.position + Vec3::Y * 0.9 + right * 0.5,
                    0.85,
                    Quat::from_rotation_y(-game.yaw + std::f32::consts::FRAC_PI_2)
                        * Quat::from_rotation_z(-0.35),
                    Material::Iron,
                    false,
                );
            }
        } else {
            // Gauntlets are material-built placeholders until the generated hero is rigged.
            for side in [-1.0, 1.0] {
                let p = head + view * 0.43 + right * 0.30 * side - view_up * 0.40;
                out.push(
                    Instance::new(p, Vec3::new(0.105, 0.16, 0.20), Vec3::new(0.10, 0.12, 0.15))
                        .with_rot(view_rot)
                        .with_mesh(self.iron)
                        .with_surface(0.48, 0.7)
                        .without_shadow(),
                );
            }
            if game.stage >= 4 {
                let swing = if game.attack_time > 0.0 {
                    (game.attack_time / 0.7 * std::f32::consts::PI).sin()
                } else {
                    0.0
                };
                let p = head + view * 0.47 + right * (0.40 - swing * 0.62) - view_up * 0.36;
                let rot = view_rot
                    * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - swing * 0.7)
                    * Quat::from_rotation_z(0.7 - swing * 1.4);
                draw_model(out, &self.sword, p, 0.7, rot, Material::Iron, true);
            }
        }
        for i in 0..54u32 {
            let p = Vec3::new(
                (hash(i * 17) - 0.5) * 9.2,
                0.4 + (hash(i + 99) * 3.0 + t * 0.035) % 3.0,
                -6.0 + hash(i * 341) * 10.6,
            );
            frame.particles.push(Particle {
                position: p,
                color: Vec3::new(0.68, 0.62, 0.51),
                size: Vec2::splat(0.012 + hash(i) * 0.018),
                opacity: 0.25,
            });
        }
        if transform_view {
            for i in 0..40u32 {
                let a = i as f32 * 2.4 + t * 2.0;
                frame.particles.push(Particle {
                    position: game.position
                        + Vec3::new(a.sin() * 0.7, hash(i) * 2.6, a.cos() * 0.7),
                    color: Vec3::new(0.45, 0.62, 1.0),
                    size: Vec2::splat(0.055),
                    opacity: 0.75,
                });
            }
        }
        frame
    }
}
