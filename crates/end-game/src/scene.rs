//! Material-built dungeon: seeded surface coordinates, metres, authored collision in core.
use ember_engine::environment::PointLight;
use ember_engine::{
    Camera, Environment, Fog, Frame, Instance, MeshData, Particle, TextureData, assets::load_glb,
};
use end_game_core::{Dungeon, Material};
use glam::{Quat, Vec2, Vec3};

fn castle_camera(target: Vec3, ideal: Vec3, gate: Option<end_game_core::layout::Aabb>) -> Vec3 {
    let steps = ((ideal - target).length() / 0.06).ceil().max(1.0) as usize;
    let mut safe = target.lerp(ideal, 0.005);
    for step in 1..=steps {
        let p = target.lerp(ideal, step as f32 / steps as f32);
        let blocked = end_game_core::layout::castle()
            .solids
            .iter()
            .map(|s| s.bounds)
            .chain(gate)
            .any(|bounds| {
                let min = Vec3::from_array(bounds.min) - Vec3::splat(0.12);
                let max = Vec3::from_array(bounds.max) + Vec3::splat(0.12);
                p.cmpge(min).all() && p.cmple(max).all()
            });
        if blocked {
            break;
        }
        safe = p;
    }
    safe
}

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
    castle: super::castle::CastleScene,
    quest: super::quest::QuestScene,
    enemies: super::enemies::EnemyScene,
    oak: u32,
    iron: u32,
    wolf: Model,
    werewolf: Model,
    sword: Model,
    warden: super::warden::WardenRig,
    cot: Model,
    torch: Model,
    props: Vec<Prop>,
    hands: super::hands::Hands,
    key: Model,
}

use super::cell::{Cell, hash};
pub(super) fn model(meshes: &mut Vec<MeshData>, bytes: &[u8]) -> Model {
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
pub(super) fn draw_model(
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
                Some(TextureData::from_png_bytes(super::cell::OAK_PNG).unwrap()),
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
            include_bytes!("../../../assets/end-game/v3/wolf-greatsword.glb"),
        );
        let warden = super::warden::WardenRig::load(&mut meshes);
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
        let hands = super::hands::Hands::load(&mut meshes);
        let key = model(
            &mut meshes,
            include_bytes!("../../../assets/end-game/v3/iron-key.glb"),
        );
        let castle = super::castle::CastleScene::build(&mut meshes, 2);
        let quest = super::quest::QuestScene::load(&mut meshes, 2);
        let enemies = super::enemies::EnemyScene::load(&mut meshes);
        (
            Self {
                cell,
                castle,
                quest,
                enemies,
                oak: 1,
                iron: 2,
                wolf,
                werewolf,
                sword,
                warden,
                cot,
                torch,
                props,
                hands,
                key,
            },
            meshes,
        )
    }

    #[cfg(test)]
    pub fn frame(&self, game: &Dungeon, third_person: bool, wake: f32) -> Frame {
        self.frame_with_camera_lift(game, third_person, wake, 0.0)
    }

    pub fn frame_with_camera_lift(
        &self,
        game: &Dungeon,
        third_person: bool,
        wake: f32,
        camera_lift: f32,
    ) -> Frame {
        let fwd = game.forward();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let mut presentation = super::hands::view(game, wake);
        presentation.head += Vec3::Y * camera_lift;
        let head = presentation.head;
        let view = presentation.forward;
        let motion = self.hands.motion(game, &presentation);
        let transform_view = game.transformation > 0.0;
        let see_hero = (third_person || transform_view)
            && game.interaction.is_none()
            && game.combat.active.is_none();
        let camera = if see_hero {
            let offset = if transform_view {
                fwd * 2.2 + right * 1.1
            } else {
                -fwd * 2.1 + right * 0.6
            };
            let target = game.position + Vec3::Y * (1.25 + camera_lift);
            let eye = if game.position.z > -7.0 {
                Vec3::new(
                    (head.x + offset.x).clamp(-2.5, 4.4),
                    head.y + 0.25,
                    (head.z + offset.z).clamp(-6.5, 4.6),
                )
            } else {
                castle_camera(
                    target,
                    head + offset + Vec3::Y * 0.25,
                    Some(game.quest.gate_bounds()),
                )
            };
            Camera {
                eye,
                target,
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
        let basement = super::castle::CastleScene::basement_detail(&camera);
        let mut frame = Frame {
            camera,
            environment: env,
            fog: Fog {
                color: [0.012, 0.019, 0.03],
                density: 0.022,
            },
            instances: if basement {
                self.cell.world.clone()
            } else {
                self.cell.distant.clone()
            },
            ..Frame::default()
        };
        self.castle.light(&mut frame, t);
        self.castle.draw(&mut frame, &self.torch, t);
        self.quest.draw(&mut frame, game);
        self.enemies.draw(&mut frame, game);
        let out = &mut frame.instances;
        if basement {
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
                    position: Vec3::new(
                        -1.417 + (t * 5.0).sin() * 0.008,
                        0.94 + phase * 0.11,
                        4.578,
                    ),
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
            self.warden.draw(out, game);
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
            let (plank_position, plank_rotation) =
                end_game_core::interaction::board_pose(game.board_open);
            out.push(
                Instance::new(
                    plank_position,
                    Vec3::new(0.30, 0.07, 1.30),
                    Vec3::new(0.9, 0.84, 0.72),
                )
                .with_mesh(self.oak)
                .with_rot(plank_rotation)
                .with_surface(0.95, 0.0),
            );
        }
        if let Some(key) = motion
            .key
            .or_else(|| (game.stage == 1).then(super::hands::ground_key))
        {
            draw_model(
                out,
                &self.key,
                key.p,
                1.0,
                key.r,
                Material::Iron,
                motion.key.is_some(),
            );
        }
        if basement {
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
                        Vec3::new(
                            x,
                            1.35 + end_game_core::EXIT_GATE_LIFT * game.exit_open,
                            -7.0,
                        ),
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
        }
        if basement
            && game.stage < 4
            && !game
                .interaction
                .is_some_and(|a| a.kind == end_game_core::InteractionKind::Sword)
        {
            let sword = super::hands::standing_sword();
            draw_model(
                out,
                &self.sword,
                sword.p,
                1.0,
                sword.r,
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
            self.hands.draw(out, &presentation, &motion);
            if let Some(sword) = motion.sword {
                draw_model(
                    out,
                    &self.sword,
                    sword.p,
                    1.0,
                    sword.r,
                    Material::Iron,
                    true,
                );
            }
        }
        if game.combat.impact_left > 0.0 {
            let age = game.combat.impact_duration() - game.combat.impact_left;
            for i in 0..22u32 {
                let direction = Vec3::new(
                    hash(i * 17) * 2.0 - 1.0,
                    hash(i * 97) * 1.3 + 0.25,
                    hash(i * 71) * 2.0 - 1.0,
                )
                .normalize();
                frame.particles.push(Particle {
                    position: game.combat.impact_point
                        + direction * age * (1.1 + hash(i * 39) * 2.2)
                        - Vec3::Y * 4.905 * age * age,
                    color: Vec3::new(0.95, 0.66 + hash(i) * 0.2, 0.32),
                    size: Vec2::splat(0.009 + hash(i * 11) * 0.016),
                    opacity: (game.combat.impact_left / game.combat.impact_duration())
                        .clamp(0.0, 1.0)
                        * 0.88,
                });
            }
        }
        if game.guard.impact_left > 0.0 {
            let age = end_game_core::guard::IMPACT_TIME - game.guard.impact_left;
            for i in 0..14u32 {
                let direction = Vec3::new(
                    hash(i * 31) * 2.0 - 1.0,
                    0.25 + hash(i * 67),
                    hash(i * 47) * 2.0 - 1.0,
                )
                .normalize();
                frame.particles.push(Particle {
                    position: game.guard.impact_point + direction * age * 1.7
                        - Vec3::Y * 4.905 * age * age,
                    color: Vec3::new(0.90, 0.82, 0.61),
                    size: Vec2::splat(0.012),
                    opacity: (game.guard.impact_left / end_game_core::guard::IMPACT_TIME) * 0.85,
                });
            }
        }
        // A sparse, short-lived steel-colored trail makes the fast cut readable.
        if !see_hero {
            if let Some(strike) = game.combat.active {
                if strike.elapsed >= strike.kind.windup_time()
                    && strike.elapsed < strike.kind.follow_end() + 0.045
                {
                    for age in [0.02, 0.04, 0.06] {
                        let pose = super::sword_motion::sample_strike(
                            strike,
                            (strike.elapsed - age).max(0.0),
                        );
                        for i in 1..=7 {
                            let point = pose.point(Vec3::X * (i as f32 * 0.24));
                            frame.particles.push(Particle {
                                position: head + presentation.rot * point,
                                color: Vec3::new(0.49, 0.55, 0.60),
                                size: Vec2::splat(0.028),
                                opacity: (1.0 - age / 0.08) * 0.12,
                            });
                        }
                    }
                }
            }
        }
        if basement {
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
    fn generated_props_decode_and_representative_frames_fit_the_v10_budget() {
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
        let count = |frame: &Frame| -> usize {
            frame
                .instances
                .iter()
                .map(|i| {
                    if i.mesh == 0 {
                        12
                    } else {
                        meshes[i.mesh as usize - 1].vertices.len() / 3
                    }
                })
                .sum::<usize>()
                + frame.particles.len() * 2
        };
        let mut maximum = count(&scene.frame(&Dungeon::default(), false, 0.0));
        for position in [
            Vec3::new(0., 0., -6.),
            Vec3::new(0., 0., -8.),
            Vec3::new(0., 0., -12.),
            Vec3::new(0., 0., -21.),
            Vec3::new(0., 0., -27.),
            Vec3::new(0., 0., -38.),
            Vec3::new(0., 0., -41.),
            Vec3::new(7., 0., -44.),
            Vec3::new(0., 3., -59.),
            Vec3::new(0., 6., -66.),
            Vec3::new(0., 6., -80.),
            Vec3::new(-7., 6., -77.9),
            Vec3::new(-23., 6., -80.),
            Vec3::new(-27., 6., -80.),
            Vec3::new(14., 6., -87.5),
            Vec3::new(18.5, 10., -97.),
            Vec3::new(24., 14., -80.),
            Vec3::new(-24., 14., -84.),
            Vec3::new(16., 22., -90.),
            Vec3::new(21., 22., -95.),
        ] {
            let mut local_max = 0;
            for yaw in 0..8 {
                let mut game = Dungeon::default();
                game.position = position;
                game.stage = 5;
                game.exit_open = 1.;
                game.warden_health = 0.;
                game.warden_ai.on_sword_hit(true, true);
                game.yaw = yaw as f32 * std::f32::consts::FRAC_PI_4;
                for third_person in [false, true] {
                    let frame = scene.frame(&game, third_person, 0.0);
                    let triangles = count(&frame);
                    local_max = local_max.max(triangles);
                    assert!(
                        triangles <= 300_000,
                        "{position:?}, yaw {yaw}, third {third_person}: {triangles}"
                    );
                    assert!(
                        frame.camera.eye.is_finite()
                            && frame.camera.eye.distance(frame.camera.target) > 0.001
                    );
                }
            }
            eprintln!("V10 position {position:?}: maximum {local_max} submitted triangles");
            maximum = maximum.max(local_max);
        }
        for id in 0..9 {
            let mut game = Dungeon::default();
            game.stage = 5;
            game.exit_open = 1.;
            game.warden_health = 0.;
            game.quest.sequence = 3;
            let enemy = &mut game.enemies[id];
            enemy.phase = end_game_core::enemies::EnemyPhase::Attacking;
            enemy.elapsed = enemy.attack.contact_time();
            if enemy.kind == end_game_core::enemies::EnemyKind::Cyclops {
                enemy.attack = end_game_core::enemies::EnemyAttack::Slam;
                enemy.elapsed = enemy.attack.windup_time();
                enemy.phase_two = true;
            }
            game.position = enemy.position + Vec3::Z * if id == 8 { 5. } else { 3. };
            let frame = scene.frame(&game, false, 0.);
            let triangles = count(&frame);
            assert!(
                triangles <= 300_000,
                "enemy {id} contact frame: {triangles}"
            );
            assert!(frame.particles.iter().all(|p| p.position.is_finite()));
            maximum = maximum.max(triangles);
        }
        eprintln!(
            "V10 maximum frame triangles: {maximum}; unique mesh triangles: {}; texture bytes incl. mip estimate: {}",
            meshes.iter().map(|m| m.vertices.len() / 3).sum::<usize>(),
            textures * 4 / 3
        );
        assert!(maximum <= 300_000);
        assert!(textures * 4 / 3 < 156 * 1024 * 1024);
    }

    #[test]
    fn castle_camera_stays_local_and_pulls_in_before_a_wall() {
        let target = Vec3::new(1.65, 1.3, -12.);
        let eye = castle_camera(target, Vec3::new(4.0, 1.7, -12.), None);
        assert!(eye.x < 1.89 && eye.x > target.x);
        assert!((eye.z + 12.).abs() < 0.001);
        let target = Vec3::new(0., 7.3, -80.);
        let ideal = target + Vec3::new(0.6, 0.4, 2.1);
        assert!(castle_camera(target, ideal, None).distance(ideal) < 0.001);
        let mut quest = end_game_core::quest::Quest::default();
        let target = Vec3::new(-24.4, 7.3, -80.);
        let ideal = Vec3::new(-26.3, 7.7, -80.);
        let closed = castle_camera(target, ideal, Some(quest.gate_bounds()));
        assert!(
            closed.x > -24.88,
            "camera passes through the closed sally port"
        );
        quest.gate_open = 1.;
        assert!(castle_camera(target, ideal, Some(quest.gate_bounds())).distance(ideal) < 0.001);
    }
}
