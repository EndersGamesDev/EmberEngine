//! V2 architectural kit: metre-scaled material surfaces, bevels and fitted joints.
use ember_engine::{Instance, MeshData, MeshVertex, TextureData};
use glam::{Quat, Vec2, Vec3};

// A single allocation per embedded source image keeps reused surface kits from
// duplicating the original PNG bytes in the WASM payload.
pub(crate) static LIMESTONE_PNG: &[u8] =
    include_bytes!("../../../assets/end-game/v2/limestone.png");
pub(crate) static OAK_PNG: &[u8] = include_bytes!("../../../assets/end-game/v2/oak.png");

pub struct Cell {
    pub world: Vec<Instance>,
    pub distant: Vec<Instance>,
    pub gate: u32,
    pub link: u32,
    pub cloth: Vec<u32>,
    pub water: u32,
}

pub fn hash(mut seed: u32) -> f32 {
    seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
    seed = ((seed >> ((seed >> 28) + 4)) ^ seed).wrapping_mul(277803737);
    ((seed ^ (seed >> 22)) & 65535) as f32 / 65535.0
}

#[derive(Default)]
struct Kit {
    vertices: Vec<MeshVertex>,
    seed: u32,
    wood: bool,
    rough: bool,
    uv_origin: Vec3,
    uv_inverse: Quat,
}

impl Kit {
    fn polygon(&mut self, points: &[Vec3], outward: Vec3, uv_seed: u32) {
        let offset = Vec2::new(hash(uv_seed) * 11.0, hash(uv_seed + 63) * 13.0);
        let wood = self.wood;
        let origin = self.uv_origin;
        let inverse = self.uv_inverse;
        let abs = if wood { inverse * outward } else { outward }.abs();
        let project = |p: Vec3| {
            let p = if wood { inverse * (p - origin) } else { p };
            // Physical texel density, including the narrow faces and bevels.
            if abs.y >= abs.x && abs.y >= abs.z {
                Vec2::new(p.x, p.z)
            } else if abs.x >= abs.z {
                Vec2::new(p.z, p.y)
            } else {
                Vec2::new(p.x, p.y)
            }
        };
        let min = points
            .iter()
            .map(|p| project(*p))
            .fold(Vec2::splat(f32::INFINITY), Vec2::min);
        let max = points
            .iter()
            .map(|p| project(*p))
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
        let uv = |p: Vec3| {
            let p = project(p);
            if wood {
                // One cropped, non-repeating patch per plank face. Grain follows the long axis.
                let span = (max - min).max(Vec2::splat(0.001));
                let q = (p - min) / span;
                let q = if span.x > span.y {
                    Vec2::new(q.y, q.x)
                } else {
                    q
                };
                q * Vec2::new(0.25 + hash(uv_seed + 9) * 0.35, 0.8)
                    + Vec2::new(hash(uv_seed) * 0.35, 0.05)
            } else {
                p * 1.15 + offset
            }
        };
        for i in 1..points.len() - 1 {
            let mut tri = [points[0], points[i], points[i + 1]];
            let mut normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
            if normal.dot(outward) < 0.0 {
                tri.swap(1, 2);
                normal = -normal;
            }
            for p in tri {
                self.vertices.push(MeshVertex {
                    pos: p.to_array(),
                    normal: normal.to_array(),
                    uv: uv(p).to_array(),
                });
            }
        }
    }

    fn block(&mut self, center: Vec3, size: Vec3, rot: Quat, bevel: f32) {
        self.uv_origin = center;
        self.uv_inverse = rot.conjugate();
        self.seed += 1;
        let seed = self.seed;
        let h = size * 0.5;
        let b = bevel.min(h.min_element() * 0.4);
        let rough = self.rough;
        let transform = |p: Vec3| {
            let p = if rough {
                // Shared vertices get identical small chips, keeping the surface closed.
                let k = ((p.x * 8192.0).round() as i32 as u32).wrapping_mul(71)
                    ^ ((p.y * 8192.0).round() as i32 as u32).wrapping_mul(919)
                    ^ ((p.z * 8192.0).round() as i32 as u32).wrapping_mul(179);
                p + (Vec3::new(
                    hash(k.wrapping_add(seed)),
                    hash(k.wrapping_add(seed + 23)),
                    hash(k.wrapping_add(seed + 41)),
                ) - Vec3::splat(0.5))
                    * b
                    * 0.6
            } else {
                p
            };
            center + rot * p
        };
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        // Six broad faces, twelve bevel strips and eight clipped corners form a closed solid.
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for sign in [-1.0, 1.0] {
                let n = axes[axis] * sign;
                let c = n * h[axis];
                let mut p = Vec::new();
                for (a, d) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    p.push(transform(
                        c + axes[u] * a * (h[u] - b) + axes[v] * d * (h[v] - b),
                    ));
                }
                self.polygon(&p, rot * n, seed);
            }
        }
        if b == 0.0 {
            return;
        }
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for a in [-1.0, 1.0] {
                for d in [-1.0, 1.0] {
                    let n = axes[u] * a + axes[v] * d;
                    let mut points = Vec::new();
                    for (end, face) in [(-1.0, 0), (1.0, 0), (1.0, 1), (-1.0, 1)] {
                        let p = axes[axis] * end * (h[axis] - b)
                            + axes[u] * a * (h[u] - if face == 0 { 0.0 } else { b })
                            + axes[v] * d * (h[v] - if face == 0 { b } else { 0.0 });
                        points.push(transform(p));
                    }
                    self.polygon(&points, rot * n, seed);
                }
            }
        }
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let signs = Vec3::new(x, y, z);
                    let points = [
                        Vec3::new(h.x, h.y - b, h.z - b),
                        Vec3::new(h.x - b, h.y, h.z - b),
                        Vec3::new(h.x - b, h.y - b, h.z),
                    ]
                    .map(|p| transform(p * signs));
                    self.polygon(&points, rot * signs, seed);
                }
            }
        }
    }

    fn bar(&mut self, a: Vec3, b: Vec3, radius: f32, sides: u32) {
        self.seed += 1;
        let forward = (b - a).normalize();
        let u = forward
            .cross(if forward.y.abs() < 0.95 {
                Vec3::Y
            } else {
                Vec3::X
            })
            .normalize();
        let v = forward.cross(u);
        for i in 0..sides {
            let angle = i as f32 * std::f32::consts::TAU / sides as f32;
            let next = (i + 1) as f32 * std::f32::consts::TAU / sides as f32;
            let p = (u * angle.cos() + v * angle.sin()) * radius;
            let q = (u * next.cos() + v * next.sin()) * radius;
            self.polygon(&[a + p, b + p, b + q, a + q], p + q, self.seed);
        }
    }

    fn ring(&mut self, center: Vec3, radii: Vec2, thickness: f32, rot: Quat) {
        for i in 0..20 {
            let point = |i: u32| {
                let a = i as f32 * std::f32::consts::TAU / 20.0;
                center + rot * Vec3::new(a.cos() * radii.x, a.sin() * radii.y, 0.0)
            };
            self.bar(point(i), point(i + 1), thickness, 6);
        }
    }

    fn register(self, meshes: &mut Vec<MeshData>, texture: Option<TextureData>) -> u32 {
        meshes.push(MeshData {
            vertices: self.vertices,
            texture,
        });
        meshes.len() as u32
    }
}

fn instance(id: u32, color: Vec3, roughness: f32, metallic: f32) -> Instance {
    Instance::new(Vec3::ZERO, Vec3::ONE, color)
        .with_mesh(id)
        .with_surface(roughness, metallic)
}

fn wall(kit: &mut [Kit; 3], origin: Vec3, rot: Quat, width: f32, height: f32, window: bool) {
    let mut y = 0.0;
    let mut row = 0u32;
    while y < height {
        let course = (0.23 + hash(row * 53 + 13) * 0.10).min(height - y);
        let mut x = -width * 0.5;
        while x < width * 0.5 - 0.02 {
            let seed = ((x * 137.0 + 1900.0) as u32).wrapping_add(row * 791);
            let length = (0.34 + hash(seed) * 0.38).min(width * 0.5 - x);
            let cx = x + length * 0.5;
            if !window || cx.abs() > 0.52 || y < 2.10 || y > 3.15 {
                let color = if y < 0.6 && hash(seed + 18) > 0.4 {
                    2
                } else if hash(seed + 41) < 0.23 {
                    1
                } else {
                    0
                };
                kit[color].block(
                    origin + rot * Vec3::new(cx, y + course * 0.5, 0.0),
                    Vec3::new(
                        (length - 0.018).max(0.025),
                        course - 0.015,
                        0.23 + hash(seed + 71) * 0.045,
                    ),
                    rot * Quat::from_rotation_z((hash(seed + 11) - 0.5) * 0.018),
                    0.011 + hash(seed + 92) * 0.013,
                );
            }
            x += length;
        }
        y += course;
        row += 1;
    }
}

fn arch(kit: &mut Kit, center: Vec3, radius: Vec2, depth: f32, width: f32, rot: Quat, count: u32) {
    for i in 0..count {
        let sample = |j: u32| {
            let a = std::f32::consts::PI * j as f32 / count as f32;
            Vec2::new(a.cos() * radius.x, a.sin() * radius.y)
        };
        let (a, b) = (sample(i), sample(i + 1));
        let middle = (a + b) * 0.5;
        let tangent = b - a;
        let p = center + rot * middle.extend(0.0);
        // Segment lengths and normals follow the ellipse, including its tight springings.
        kit.block(
            p,
            Vec3::new(width, tangent.length() * 0.96, depth),
            rot * Quat::from_rotation_z(tangent.y.atan2(tangent.x) - std::f32::consts::FRAC_PI_2),
            0.018,
        );
    }
}

pub fn build(meshes: &mut Vec<MeshData>) -> Cell {
    let identity = Quat::IDENTITY;
    let mut stone: [Kit; 3] = std::array::from_fn(|i| Kit {
        seed: 910 + i as u32 * 509,
        rough: true,
        ..Kit::default()
    });
    let mut oak = Kit {
        wood: true,
        ..Kit::default()
    };
    let mut iron = Kit::default();
    let mut mortar = Kit::default();
    let mut debris = Kit::default();
    // Solid mortar backs every fitted course; the window is an actual deep recess.
    for (origin, rot, width, height, slit) in [
        (
            Vec3::new(-3.15, 0.0, 2.55),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            5.25,
            3.65,
            false,
        ),
        (
            Vec3::new(3.15, 0.0, 2.55),
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
            5.25,
            3.65,
            false,
        ),
        (Vec3::new(0.0, 0.0, 5.12), identity, 6.5, 3.65, true),
        (
            Vec3::new(-5.15, 0.0, -3.5),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            7.25,
            4.2,
            false,
        ),
        (
            Vec3::new(5.15, 0.0, -3.5),
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
            7.25,
            4.2,
            false,
        ),
        (Vec3::new(-3.0, 0.0, -7.12), identity, 4.0, 4.2, false),
        (Vec3::new(3.0, 0.0, -7.12), identity, 4.0, 4.2, false),
    ] {
        let backing =
            origin + rot * Vec3::new(0.0, height * 0.5, if origin.z > 5.0 { 0.3 } else { 0.0 });
        mortar.block(backing, Vec3::new(width, height, 0.12), rot, 0.0);
        wall(&mut stone, origin, rot, width, height, slit);
    }
    // Side returns at the cage and a continuous structural floor/ceiling.
    for x in [-4.12, 4.12] {
        wall(
            &mut stone,
            Vec3::new(x, 0.0, 0.14),
            identity,
            1.8,
            4.0,
            false,
        );
        mortar.block(
            Vec3::new(x, 1.9, 0.14),
            Vec3::new(1.8, 3.8, 0.2),
            identity,
            0.0,
        );
    }
    mortar.block(
        Vec3::new(0.0, -0.22, 2.5),
        Vec3::new(6.3, 0.25, 5.3),
        identity,
        0.0,
    );
    mortar.block(
        Vec3::new(0.0, -0.23, -3.5),
        Vec3::new(10.4, 0.25, 7.3),
        identity,
        0.0,
    );
    mortar.block(
        Vec3::new(0.0, 3.68, 2.55),
        Vec3::new(6.6, 0.22, 5.4),
        identity,
        0.0,
    );
    mortar.block(
        Vec3::new(0.0, 4.35, -3.55),
        Vec3::new(10.4, 0.3, 7.4),
        identity,
        0.0,
    );
    // Fitted oak floor: narrow boards, offset joints, worn bevels and hand-forged nails.
    for col in 0..25u32 {
        let x = -2.88 + col as f32 * 0.24;
        let mut z: f32 = 0.02;
        let mut row = 0u32;
        while z < 5.03 {
            let length = (1.1 + hash(col * 49 + row * 307) * 0.75).min(5.04 - z);
            oak.block(
                Vec3::new(x, -0.065, z + length * 0.5),
                Vec3::new(0.229, 0.13, length - 0.012),
                identity,
                0.007,
            );
            for end in [z + 0.07, z + length - 0.07] {
                for dx in [-0.075, 0.075] {
                    iron.block(
                        Vec3::new(x + dx, 0.001, end),
                        Vec3::new(0.013, 0.006, 0.017),
                        Quat::from_rotation_y(hash(col * 29 + row) * 1.5),
                        0.002,
                    );
                }
            }
            z += length;
            row += 1;
        }
    }
    // Damp footing closes the floor-wall joint and carries chips above the boards.
    for x in [-3.02, 3.02] {
        for z in 0..12 {
            stone[2].block(
                Vec3::new(x, 0.055, 0.24 + z as f32 * 0.43),
                Vec3::new(0.19, 0.11, 0.416),
                identity,
                0.012,
            );
        }
    }
    for x in 0..14 {
        stone[2].block(
            Vec3::new(-2.93 + x as f32 * 0.45, 0.055, 5.01),
            Vec3::new(0.436, 0.11, 0.17),
            identity,
            0.012,
        );
    }
    // Flagstones and rubble in the passage, kept level for traversability.
    for x in 0..18u32 {
        for z in 0..13u32 {
            let seed = x * 41 + z * 73;
            stone[(seed % 3) as usize].block(
                Vec3::new(-4.86 + x as f32 * 0.57, -0.085, -0.28 - z as f32 * 0.55),
                Vec3::new(0.552, 0.15, 0.532),
                identity,
                0.018,
            );
        }
    }
    // Cell roof joists: shoulders, diagonal corbels, pegs and iron straps.
    for z in [0.45, 2.55, 4.65] {
        oak.block(
            Vec3::new(0.0, 3.39, z),
            Vec3::new(6.08, 0.27, 0.26),
            identity,
            0.017,
        );
        for x in [-2.72, 2.72] {
            oak.block(
                Vec3::new(x, 3.13, z),
                Vec3::new(0.72, 0.15, 0.18),
                Quat::from_rotation_z(if x < 0.0 { 0.63 } else { -0.63 }),
                0.012,
            );
            iron.block(
                Vec3::new(x, 3.36, z),
                Vec3::new(0.08, 0.32, 0.29),
                identity,
                0.006,
            );
        }
    }
    for x in 0..23 {
        oak.block(
            Vec3::new(-2.96 + x as f32 * 0.267, 3.54, 2.55),
            Vec3::new(0.26, 0.06, 5.15),
            identity,
            0.004,
        );
    }
    // Deep barred window with a worn sill and a small Romanesque arch.
    stone[1].block(
        Vec3::new(0.0, 2.13, 4.99),
        Vec3::new(1.16, 0.12, 0.55),
        identity,
        0.022,
    );
    for x in [-0.51, 0.51] {
        for y in 0..4 {
            stone[1].block(
                Vec3::new(x, 2.25 + y as f32 * 0.23, 5.01),
                Vec3::new(0.18, 0.218, 0.4),
                identity,
                0.018,
            );
        }
    }
    arch(
        &mut stone[1],
        Vec3::new(0.0, 2.94, 5.0),
        Vec2::new(0.52, 0.34),
        0.42,
        0.16,
        identity,
        11,
    );
    for x in [-0.3, -0.1, 0.1, 0.3] {
        iron.bar(Vec3::new(x, 2.17, 5.05), Vec3::new(x, 3.17, 5.05), 0.018, 8);
    }
    iron.bar(
        Vec3::new(-0.46, 2.52, 5.05),
        Vec3::new(0.46, 2.52, 5.05),
        0.017,
        8,
    );
    // Layered cage frame with collars, rivets, top rail and stone sockets.
    for i in 0..25 {
        let x = -2.88 + i as f32 * 0.24;
        if x.abs() < 0.76 {
            continue;
        }
        iron.bar(Vec3::new(x, 0.07, 0.0), Vec3::new(x, 3.18, 0.0), 0.022, 8);
        for y in [0.2, 1.08, 2.72] {
            iron.block(
                Vec3::new(x, y, 0.0),
                Vec3::new(0.075, 0.05, 0.076),
                identity,
                0.006,
            );
            iron.bar(Vec3::new(x, y, -0.048), Vec3::new(x, y, 0.062), 0.022, 8);
        }
    }
    for x in [-2.96, -0.78, 0.78, 2.96] {
        iron.block(
            Vec3::new(x, 1.61, 0.0),
            Vec3::new(0.086, 3.22, 0.10),
            identity,
            0.012,
        );
        stone[1].block(
            Vec3::new(x, 0.08, 0.0),
            Vec3::new(0.20, 0.16, 0.24),
            identity,
            0.018,
        );
    }
    for y in [0.2, 1.08, 2.72, 3.2] {
        for x in [-1.90, 1.90] {
            iron.block(
                Vec3::new(x, y, 0.0),
                Vec3::new(2.16, 0.075, 0.075),
                identity,
                0.009,
            );
        }
    }
    iron.block(
        Vec3::new(0.0, 3.24, 0.0),
        Vec3::new(6.12, 0.1, 0.11),
        identity,
        0.009,
    );
    // Passage vault: smaller radial stones and clustered piers create a layered silhouette.
    for z in [-0.65, -3.65, -6.65] {
        arch(
            &mut stone[1],
            Vec3::new(0.0, 1.6, z),
            Vec2::new(4.9, 2.48),
            0.34,
            0.20,
            identity,
            37,
        );
        for x in [-4.90, 4.90] {
            for y in 0..6 {
                stone[1].block(
                    Vec3::new(x, 0.14 + y as f32 * 0.28, z),
                    Vec3::new(0.36, 0.263, 0.40),
                    identity,
                    0.021,
                );
            }
            stone[1].block(
                Vec3::new(x, 1.59, z),
                Vec3::new(0.47, 0.16, 0.51),
                identity,
                0.018,
            );
        }
    }
    // Close the barrel vault with fitted, textured courses behind its projecting ribs.
    for course in 0..15 {
        arch(
            &mut stone[0],
            Vec3::new(0.0, 1.6, -0.20 - course as f32 * 0.49),
            Vec2::new(5.04, 2.66),
            0.478,
            0.14,
            identity,
            43,
        );
    }
    for i in 0..48 {
        let point = |j: u32, z: f32| {
            let a = j as f32 * std::f32::consts::PI / 48.0;
            Vec3::new(a.cos() * 5.20, 1.6 + a.sin() * 2.92, z)
        };
        mortar.polygon(
            &[
                point(i, 0.05),
                point(i + 1, 0.05),
                point(i + 1, -7.5),
                point(i, -7.5),
            ],
            -(point(i, -3.5) - Vec3::new(0.0, 1.6, -3.5)),
            713,
        );
    }
    // Keep the same 1.8 m opening as the shared exit collision slab.
    for x in [-3.075, 3.075] {
        mortar.block(
            Vec3::new(x, 2.2, -7.45),
            Vec3::new(4.35, 4.5, 0.22),
            identity,
            0.0,
        );
    }
    mortar.block(
        Vec3::new(0.0, 3.65, -7.45),
        Vec3::new(1.8, 1.6, 0.22),
        identity,
        0.0,
    );
    wall(
        &mut stone,
        Vec3::new(0.0, 2.85, -7.12),
        identity,
        2.02,
        1.35,
        false,
    );
    // Exit's carved surround; the barred gate is animated by Scene.
    arch(
        &mut stone[1],
        Vec3::new(0.0, 2.3, -7.06),
        Vec2::new(1.02, 0.58),
        0.40,
        0.21,
        identity,
        15,
    );
    for x in [-1.02, 1.02] {
        for y in 0..9 {
            stone[1].block(
                Vec3::new(x, 0.13 + y as f32 * 0.26, -7.06),
                Vec3::new(0.21, 0.245, 0.36),
                identity,
                0.016,
            );
        }
    }
    // Corner fragments and scattered straw remain below the player's foot clearance.
    for i in 0..110u32 {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let p = Vec3::new(
            side * (2.63 + hash(i * 47) * 0.28),
            0.014,
            0.3 + hash(i * 23 + 1) * 4.55,
        );
        if i % 3 == 0 {
            stone[2].block(
                p,
                Vec3::new(0.035 + hash(i) * 0.035, 0.025, 0.025 + hash(i + 9) * 0.05),
                Quat::from_rotation_y(hash(i + 3) * 6.0),
                0.005,
            );
        } else {
            debris.block(
                p,
                Vec3::new(0.004, 0.003, 0.05 + hash(i) * 0.14),
                Quat::from_rotation_y(hash(i + 17) * 6.0),
                0.0,
            );
        }
    }
    // The sword plinth is a chipped monolith instead of intersecting cubes.
    stone[1].block(
        Vec3::new(2.6, 0.23, -4.6),
        Vec3::new(1.0, 0.46, 0.90),
        Quat::from_rotation_y(-0.23),
        0.12,
    );
    stone[0].block(
        Vec3::new(2.6, 0.59, -4.6),
        Vec3::new(0.80, 0.44, 0.71),
        Quat::from_rotation_y(-0.18),
        0.09,
    );
    // Root keeps materials shared by batch; texture allocation never grows per stone.
    let stone_texture = TextureData::from_png_bytes(LIMESTONE_PNG).unwrap();
    let oak_texture = TextureData::from_png_bytes(OAK_PNG).unwrap();
    let iron_texture = meshes[1].texture.clone();
    let mut world = Vec::new();
    for (i, kit) in stone.into_iter().enumerate() {
        let tint = [
            Vec3::new(0.88, 0.92, 0.98),
            Vec3::new(0.71, 0.74, 0.77),
            Vec3::new(0.48, 0.54, 0.46),
        ][i];
        world.push(instance(
            kit.register(meshes, Some(stone_texture.clone())),
            tint,
            0.95,
            0.0,
        ));
    }
    world.push(instance(
        oak.register(meshes, Some(oak_texture)),
        Vec3::ONE,
        0.86,
        0.0,
    ));
    world.push(instance(
        iron.register(meshes, iron_texture.clone()),
        Vec3::new(0.67, 0.66, 0.61),
        0.62,
        0.6,
    ));
    world.push(instance(
        mortar.register(meshes, None),
        Vec3::new(0.07, 0.069, 0.062),
        1.0,
        0.0,
    ));
    world.push(instance(
        debris.register(meshes, None),
        Vec3::new(0.31, 0.24, 0.11),
        1.0,
        0.0,
    ));

    let mut gate = Kit::default();
    for x in [-0.61, -0.305, 0.0, 0.305, 0.61] {
        gate.bar(Vec3::new(x, 0.1, 0.04), Vec3::new(x, 3.03, 0.04), 0.024, 8);
        for y in [0.18, 1.02, 2.73] {
            gate.block(
                Vec3::new(x, y, 0.04),
                Vec3::new(0.073, 0.075, 0.085),
                identity,
                0.007,
            );
        }
    }
    for y in [0.13, 1.02, 2.73, 3.04] {
        gate.block(
            Vec3::new(0.0, y, 0.04),
            Vec3::new(1.44, 0.073, 0.09),
            identity,
            0.006,
        );
    }
    gate.block(
        Vec3::new(0.45, 1.1, 0.13),
        Vec3::new(0.20, 0.29, 0.10),
        identity,
        0.022,
    );
    gate.ring(
        Vec3::new(0.46, 1.28, 0.14),
        Vec2::new(0.068, 0.09),
        0.014,
        identity,
    );
    let gate = gate.register(meshes, iron_texture.clone());
    let mut link = Kit::default();
    link.ring(Vec3::ZERO, Vec2::new(0.027, 0.043), 0.007, identity);
    let link = link.register(meshes, iron_texture);
    // Art-directed cloth poses are cached meshes; only one pose is drawn, no GPU uploads per frame.
    let mut cloth = Vec::new();
    for pose in 0..16 {
        let mut mesh = Kit::default();
        let phase = pose as f32 * std::f32::consts::TAU / 16.0;
        let point = |x: usize, y: usize| {
            let u = x as f32 / 10.0;
            let v = y as f32 / 16.0;
            let hem = 0.075 * (hash(x as u32 * 47) - 0.5) * v.powi(4);
            Vec3::new(
                (u - 0.5) * 0.72,
                -v * 0.92 + hem,
                (u * 22.0).sin() * 0.018
                    + v * v
                        * ((phase + u * 4.0).sin() * 0.034 + (phase * 2.0 + v * 3.0).sin() * 0.008),
            )
        };
        for y in 0..16 {
            for x in 0..10 {
                if y > 12 && (x == 3 || x == 8) {
                    continue;
                }
                mesh.polygon(
                    &[
                        point(x, y),
                        point(x + 1, y),
                        point(x + 1, y + 1),
                        point(x, y + 1),
                    ],
                    Vec3::Z,
                    79,
                );
            }
        }
        cloth.push(mesh.register(meshes, None));
    }
    let water = MeshData::textured_plane(1.0, None);
    meshes.push(water);
    // A tiny closed shell preserves distant silhouettes and shadows after the
    // detailed basement leaves the submitted frame. It keeps the exit open.
    let distant = [
        (Vec3::new(0.0, -0.12, -1.0), Vec3::new(10.0, 0.24, 12.0)),
        (Vec3::new(0.0, 4.35, -3.5), Vec3::new(10.4, 0.25, 7.0)),
        (Vec3::new(-5.1, 2.1, -3.5), Vec3::new(0.2, 4.2, 7.0)),
        (Vec3::new(5.1, 2.1, -3.5), Vec3::new(0.2, 4.2, 7.0)),
        (Vec3::new(-3.1, 1.8, 2.5), Vec3::new(0.2, 3.6, 5.0)),
        (Vec3::new(3.1, 1.8, 2.5), Vec3::new(0.2, 3.6, 5.0)),
        (Vec3::new(0.0, 1.8, 5.1), Vec3::new(6.4, 3.6, 0.2)),
        (Vec3::new(0.0, 3.7, 2.5), Vec3::new(6.4, 0.2, 5.0)),
        (Vec3::new(-3.075, 2.2, -7.45), Vec3::new(4.35, 4.5, 0.22)),
        (Vec3::new(3.075, 2.2, -7.45), Vec3::new(4.35, 4.5, 0.22)),
        (Vec3::new(0.0, 3.65, -7.45), Vec3::new(1.8, 1.6, 0.22)),
    ]
    .into_iter()
    .map(|(p, s)| Instance::new(p, s, Vec3::new(0.17, 0.17, 0.15)).with_surface(0.97, 0.0))
    .collect();
    Cell {
        world,
        distant,
        gate,
        link,
        cloth,
        water: meshes.len() as u32,
    }
}

impl Cell {
    pub fn animate(&self, out: &mut Vec<Instance>, time: f32, gate_open: f32) {
        out.push(
            Instance::new(
                Vec3::X * gate_open * 1.55,
                Vec3::ONE,
                Vec3::new(0.72, 0.69, 0.62),
            )
            .with_mesh(self.gate)
            .with_surface(0.56, 0.65),
        );
        for (anchor, seed) in [
            (Vec3::new(-2.82, 2.05, 1.45), 0.0),
            (Vec3::new(2.84, 1.98, 4.22), 1.9),
        ] {
            let swing = (time * 1.35 + seed).sin() * 0.045;
            for link in 0..13 {
                let y = link as f32 * 0.067;
                out.push(
                    Instance::new(
                        anchor + Vec3::new(swing * y, -y, (time * 1.08 + seed).sin() * 0.028 * y),
                        Vec3::ONE,
                        Vec3::new(0.50, 0.47, 0.40),
                    )
                    .with_mesh(self.link)
                    .with_rot(
                        Quat::from_rotation_y(if link % 2 == 0 {
                            0.0
                        } else {
                            std::f32::consts::FRAC_PI_2
                        }) * Quat::from_rotation_z(swing),
                    )
                    .with_surface(0.61, 0.72),
                );
            }
        }
        let pose = ((time * 5.0) as usize) % self.cloth.len();
        out.push(
            Instance::new(
                Vec3::new(-2.25, 0.72, 2.47),
                Vec3::new(1.0, 0.65, 1.0),
                Vec3::new(0.24, 0.205, 0.14),
            )
            .with_mesh(self.cloth[pose])
            .with_surface(1.0, 0.0),
        );
        out.push(
            Instance::new(
                Vec3::new(2.60, 0.005, 4.46),
                Vec3::new(0.52, 1.0, 0.33),
                Vec3::new(0.075, 0.10, 0.11),
            )
            .with_mesh(self.water)
            .with_surface(0.13, 0.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_exit_has_no_static_wall_or_plank_across_the_passage() {
        let mut meshes = vec![
            MeshData::textured_box(1., None),
            MeshData::textured_box(1., None),
        ];
        let cell = build(&mut meshes);
        for x in [-0.65, 0., 0.65] {
            for y in [0.15, 0.8, 1.7] {
                let origin = Vec3::new(x, y, -6.7);
                let direction = -Vec3::Z;
                for instance in &cell.world {
                    for tri in meshes[instance.mesh as usize - 1].vertices.chunks_exact(3) {
                        let a = Vec3::from_array(tri[0].pos);
                        let b = Vec3::from_array(tri[1].pos);
                        let c = Vec3::from_array(tri[2].pos);
                        let edge = b - a;
                        let other = c - a;
                        let cross = direction.cross(other);
                        let det = edge.dot(cross);
                        if det.abs() < 0.000001 {
                            continue;
                        }
                        let offset = origin - a;
                        let u = offset.dot(cross) / det;
                        if !(0.0..=1.0).contains(&u) {
                            continue;
                        }
                        let q = offset.cross(edge);
                        let v = direction.dot(q) / det;
                        if v < 0. || u + v > 1. {
                            continue;
                        }
                        let distance = other.dot(q) / det;
                        assert!(
                            !(0.0..1.2).contains(&distance),
                            "static mesh {} closes exit at {x},{y} distance {distance}",
                            instance.mesh
                        );
                    }
                }
                for instance in &cell.distant {
                    let min = instance.position - instance.scale * 0.5;
                    let max = instance.position + instance.scale * 0.5;
                    assert!(
                        !(x > min.x
                            && x < max.x
                            && y > min.y
                            && y < max.y
                            && min.z < -6.7
                            && max.z > -7.9)
                    );
                }
            }
        }
    }

    #[test]
    fn bevels_are_closed_finite_and_keep_the_authored_bounds() {
        let mut mesh = Kit::default();
        mesh.block(Vec3::ZERO, Vec3::new(0.5, 0.3, 0.25), Quat::IDENTITY, 0.02);
        assert_eq!(mesh.vertices.len(), 132);
        for v in &mesh.vertices {
            let p = Vec3::from_array(v.pos);
            assert!(p.is_finite() && Vec3::from_array(v.normal).is_normalized());
            assert!(p.abs().cmple(Vec3::new(0.25, 0.15, 0.125)).all());
        }
        // Every geometric edge belongs to exactly two triangles (including face diagonals).
        let key = |p: [f32; 3]| p.map(|x| (x * 100000.0).round() as i32);
        let mut edges = std::collections::BTreeMap::new();
        for tri in mesh.vertices.chunks_exact(3) {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let mut edge = [key(tri[a].pos), key(tri[b].pos)];
                edge.sort();
                *edges.entry(edge).or_insert(0) += 1;
            }
        }
        assert!(edges.values().all(|count| *count == 2));
    }
}
