//! Material-built dungeon: seeded surface coordinates, metres, authored collision in core.
use ember_engine::environment::PointLight;
use ember_engine::{
    Camera, Environment, Fog, Frame, Instance, MeshData, Particle, TextureData, assets::load_glb,
};
use end_game_core::{Dungeon, Material};
use glam::{Quat, Vec2, Vec3};

pub struct Model {
    pub ids: Vec<(u32, Vec3, Option<Material>)>,
}
struct Prop {
    model: Model,
    position: Vec3,
    rot: Quat,
    material: Material,
    sway: bool,
}
pub struct Scene {
    cell: Cell,
    oak: u32,
    iron: u32,
    wolf: Model,
    werewolf: Model,
    sword: Model,
    warden: Model,
    cot: Model,
    torch: Model,
    props: Vec<Prop>,
}

use super::cell::{Cell, hash};
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

impl Scene {
    pub fn build() -> (Self, Vec<MeshData>) {
        let mut meshes = vec![
            MeshData::textured_box(
                1.0,
                Some(
                    TextureData::from_png_bytes(include_bytes!(
                        "../../../assets/end-game/v2/oak.png"
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
        ];
        let cell = super::cell::build(&mut meshes);
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
            include_bytes!("../../../assets/end-game/v2/cot-detailed.glb"),
        );
        let torch = model(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v1/torch.glb"),
        );
        let specs: &[(&[u8], Vec3, f32, Material, bool)] = &[
            (
                include_bytes!("../../../assets/end-game/v2/bucket.glb"),
                Vec3::new(2.58, 0.0, 4.54),
                -0.35,
                Material::Oak,
                false,
            ),
            (
                include_bytes!("../../../assets/end-game/v2/chain-shackle.glb"),
                Vec3::new(-2.94, 1.94, 1.38),
                std::f32::consts::FRAC_PI_2,
                Material::Iron,
                true,
            ),
            (
                include_bytes!("../../../assets/end-game/v2/hanging-rag.glb"),
                Vec3::new(-2.94, 1.76, 4.65),
                std::f32::consts::FRAC_PI_2,
                Material::Cloth,
                true,
            ),
            (
                include_bytes!("../../../assets/end-game/v2/drain-grate.glb"),
                Vec3::new(2.5, 0.004, 1.05),
                0.0,
                Material::Iron,
                false,
            ),
            (
                include_bytes!("../../../assets/end-game/v2/candle-stool.glb"),
                Vec3::new(-1.42, 0.0, 4.58),
                0.0,
                Material::Oak,
                false,
            ),
            (
                include_bytes!("../../../assets/end-game/v2/straw-scatter.glb"),
                Vec3::new(-1.74, 0.01, 2.72),
                0.15,
                Material::Cloth,
                false,
            ),
        ];
        let props = specs
            .iter()
            .map(|(bytes, position, yaw, material, sway)| Prop {
                model: model(&mut meshes, bytes),
                position: *position,
                rot: Quat::from_rotation_y(*yaw),
                material: *material,
                sway: *sway,
            })
            .collect();
        (
            Self {
                cell,
                oak: 1,
                iron: 2,
                wolf,
                werewolf,
                sword,
                warden,
                cot,
                torch,
                props,
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
            sun_intensity: 0.26,
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
                color: Vec3::new(1.0, 0.60, 0.30),
                intensity: 7.5 * flicker,
                radius: 7.0,
            };
        }
        env.lights[3] = PointLight {
            position: Vec3::new(-1.417, 0.928, 4.578),
            color: Vec3::new(1.0, 0.67, 0.32),
            intensity: 1.0 * flicker,
            radius: 3.6,
        };
        let mut frame = Frame {
            camera,
            environment: env,
            fog: Fog {
                color: [0.012, 0.019, 0.03],
                density: 0.022,
            },
            instances: self.cell.world.clone(),
            ..Frame::default()
        };
        let out = &mut frame.instances;
        self.cell.animate(out, t, game.gate_open);
        for prop in &self.props {
            let motion = if prop.sway {
                Quat::from_rotation_x((t * 1.3 + prop.position.z).sin() * 0.025)
            } else {
                Quat::IDENTITY
            };
            draw_model(
                out,
                &prop.model,
                prop.position,
                1.0,
                prop.rot * motion,
                prop.material,
                false,
            );
        }
        for i in 0..5 {
            let phase = (t * 1.7 + i as f32 * 0.2).fract();
            frame.particles.push(Particle {
                position: Vec3::new(-1.417 + (t * 5.0).sin() * 0.008, 0.94 + phase * 0.11, 4.578),
                color: Vec3::new(1.0, 0.63 + phase * 0.25, 0.20),
                size: Vec2::new(0.028 * (1.0 - phase) + 0.008, 0.062 * (1.0 - phase) + 0.015),
                opacity: (1.0 - phase) * 0.86,
            });
        }
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
        let plank_angle = 0.025 - game.board_open * 0.245;
        out.push(
            Instance::new(
                Vec3::new(-1.25, 0.035 + game.board_open * 0.15, 2.1),
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
        for i in 0..5 {
            let age = (t + i as f32 * 0.43).rem_euclid(2.1);
            let impact = (2.0 * 3.15 / end_game_core::GRAVITY).sqrt();
            let x = 2.60 + (hash(i * 91) - 0.5) * 0.18;
            let z = 4.46 + (hash(i * 19) - 0.5) * 0.14;
            if age <= impact {
                frame.particles.push(Particle {
                    position: Vec3::new(x, 3.15 - 0.5 * end_game_core::GRAVITY * age * age, z),
                    color: Vec3::new(0.48, 0.64, 0.72),
                    size: Vec2::new(0.012, 0.035),
                    opacity: 0.58,
                });
            }
            let splash = age - impact;
            if (0.0..0.32).contains(&splash) {
                let radius = splash * 0.4;
                for point in 0..10 {
                    let a = point as f32 * std::f32::consts::TAU / 10.0;
                    frame.particles.push(Particle {
                        position: Vec3::new(x + a.cos() * radius, 0.014, z + a.sin() * radius),
                        color: Vec3::new(0.25, 0.37, 0.41),
                        size: Vec2::splat(0.013),
                        opacity: (1.0 - splash / 0.32) * 0.28,
                    });
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_props_decode_and_the_cell_fits_its_render_budget() {
        let (scene, meshes) = Scene::build();
        for model in std::iter::once(&scene.cot).chain(scene.props.iter().map(|p| &p.model)) {
            for (id, _, _) in &model.ids {
                let mesh = &meshes[*id as usize - 1];
                assert!(mesh.texture.is_some(), "generated prop lost its RGB8 atlas");
                assert!(
                    mesh.vertices
                        .iter()
                        .all(|v| Vec3::from_array(v.pos).is_finite()
                            && Vec3::from_array(v.normal).is_finite())
                );
            }
        }
        let textures: usize = meshes
            .iter()
            .filter_map(|m| m.texture.as_ref())
            .map(|t| t.rgba8.len())
            .sum();
        let frame = scene.frame(&Dungeon::default(), false, 0.0);
        let triangles: usize = frame
            .instances
            .iter()
            .map(|i| {
                if i.mesh == 0 {
                    12
                } else {
                    meshes[i.mesh as usize - 1].vertices.len() / 3
                }
            })
            .sum();
        eprintln!(
            "V2 frame triangles: {triangles}; texture bytes incl. mip estimate: {}",
            textures * 4 / 3
        );
        assert!(triangles < 220_000);
        assert!(textures * 4 / 3 < 120 * 1024 * 1024);
    }
}
