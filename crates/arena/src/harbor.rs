//! Hand-authored working container terminal, in metres. There is no layout
//! RNG and no generated prop placement. Collision cover is drawn from the
//! server's boxes; scenery below head height stays outside the playable quay.
//! The ship and cranes are batched geometry, not thousands of cube instances.
//!
//! Existing v13 material pixels are reused at modest resolution; the palette
//! applies authored paint colours to their neutral surface grain. No new
//! downloaded or AI-generated asset is used. Scalar surface presets distinguish
//! painted metal, bare hardware and rough paving without extra texture maps.

// Authored geometry loops are bounded below 256 and texture channels at 255.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::f32::consts::FRAC_PI_2;
use std::sync::OnceLock;

use arena_core::harbor::{
    CONTAINER_20, CONTAINER_40, CONTAINER_H, CONTAINER_W, CRANE_CENTERS_Z, CRANE_LEGS, HARBOR_HALF,
    WAREHOUSE, WAREHOUSE_MAX, WAREHOUSE_MIN, WAREHOUSE_ROOF_BASE,
};
use arena_core::shooter::{Cover, Obstacle};
use ember_engine::glam::{Quat, Vec3};
use ember_engine::{Frame, Instance, MeshData, MeshVertex, TextureData};

const STEEL: &[u8] = include_bytes!("../../../assets/textures/v13/cast-iron.png");
const STONE: &[u8] = include_bytes!("../../../assets/textures/v13/plinth.png");
const CONTAINER: &[u8] = include_bytes!("../../../assets/textures/v13/container.png");

// The order is also the registration contract, kept in one fixed array below.
const ASPHALT: u32 = 0;
const QUAY: u32 = 1;
const WATER: u32 = 2;
const STRUCTURES: u32 = 3;
const MARKINGS: u32 = 4;
const BOX20: u32 = 5;
const BOX40: u32 = 6;
const HARDWARE20: u32 = 7;
const HARDWARE40: u32 = 8;
const SOLID: u32 = 9;
const ROOF: u32 = 10;
pub const MESH_COUNT: usize = 11;
// Distant terminal continuation, not a larger playable map. Its edges lie
// beyond the camera's 500m far plane even from the authored overview views.
const FAR_GROUND: f32 = 700.0;

// White, crane blue, deep hull, red oxide, rubber, galvanized steel, safety
// yellow, window blue, concrete, ochre, evergreen, ivory, water and markings.
const PAINT: [[u8; 3]; 16] = [
    [203, 205, 193],
    [58, 101, 121],
    [34, 48, 57],
    [126, 55, 43],
    [27, 31, 32],
    [128, 141, 143],
    [212, 167, 66],
    [45, 76, 86],
    [139, 142, 135],
    [146, 116, 62],
    [60, 89, 79],
    [176, 171, 148],
    [36, 76, 89],
    [221, 220, 199],
    [87, 105, 113],
    [89, 71, 62],
];
const CRANE_PAINT: usize = 1;
// The lower crane columns share this neutral steel texture with the roof.
// Their instance tint must compensate in linear light, not multiply two
// independently chosen display colours at the seam with the upper structure.
const STEEL_TILE_COLOR: [u8; 3] = [209, 215, 212];
const CONTAINER_PAINT: [Vec3; 6] = [
    Vec3::new(0.61, 0.28, 0.21),
    Vec3::new(0.27, 0.42, 0.49),
    Vec3::new(0.37, 0.46, 0.39),
    Vec3::new(0.70, 0.63, 0.44),
    Vec3::new(0.63, 0.65, 0.59),
    Vec3::new(0.40, 0.48, 0.53),
];

fn srgb_component(channel: u8) -> f32 {
    let encoded = f32::from(channel) / 255.0;
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

fn crane_leg_tint() -> Vec3 {
    static TINT: OnceLock<Vec3> = OnceLock::new();
    *TINT.get_or_init(|| {
        Vec3::from_array(PAINT[CRANE_PAINT].map(srgb_component))
            / Vec3::from_array(STEEL_TILE_COLOR.map(srgb_component))
    })
}

#[derive(Clone, Copy, Debug)]
pub struct HarborArt {
    base: u32,
}

impl HarborArt {
    pub const fn new(base: u32) -> Self {
        Self { base }
    }

    pub fn push_ground(self, frame: &mut Frame) {
        for (offset, roughness) in [(ASPHALT, 0.98), (QUAY, 0.92), (WATER, 0.12)] {
            let mut part = Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::ONE)
                .with_mesh(self.base + offset)
                .with_surface(roughness, 0.0)
                .with_wetness();
            if offset == WATER {
                // Water reflects even in dry weather; rain wetness is only an
                // additional weather response, not what makes this water.
                part = part.without_shadow();
            }
            frame.instances.push(part);
        }
    }

    pub fn push_decor(self, frame: &mut Frame) {
        frame.instances.push(
            Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::ONE)
                .with_mesh(self.base + STRUCTURES)
                // This coarse batch is predominantly painted steel, not a
                // bare-metal mirror: bridge paint and crane coatings insulate.
                .with_surface(0.55, 0.0)
                .with_wetness(),
        );
        frame.instances.push(
            Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::ONE)
                .with_mesh(self.base + MARKINGS)
                .with_surface(0.85, 0.0)
                .without_shadow(),
        );
    }

    /// Solid covers match their authoritative AABB exactly. Corrugations and
    /// castings are recessed inside the container's ISO envelope, not bolted
    /// into a traversable lane. Each tier draws two reusable meshes.
    pub fn push_cover(self, frame: &mut Frame, obstacle: &Obstacle) {
        let size = Vec3::new(
            obstacle.max[0] - obstacle.min[0],
            obstacle.h - obstacle.base,
            obstacle.max[1] - obstacle.min[1],
        );
        let center = Vec3::new(
            f32::midpoint(obstacle.min[0], obstacle.max[0]),
            f32::midpoint(obstacle.base, obstacle.h),
            f32::midpoint(obstacle.min[1], obstacle.max[1]),
        );
        if obstacle.kind == Cover::Container {
            let along_x = size.x >= size.z;
            let length = size.x.max(size.z);
            let long = length > 9.0;
            let nominal = if long { CONTAINER_40 } else { CONTAINER_20 };
            let tiers = (size.y / CONTAINER_H).round().max(1.0);
            let rot = if along_x {
                Quat::IDENTITY
            } else {
                Quat::from_rotation_y(FRAC_PI_2)
            };
            let scale = Vec3::new(
                length / nominal,
                size.y / tiers / CONTAINER_H,
                size.x.min(size.z) / CONTAINER_W,
            );
            // Stable authored lane/row colour; not a pseudo-random prop picker.
            let column = if center.x < -10.0 {
                0
            } else if center.x < 10.0 {
                1
            } else {
                2
            };
            let row = usize::from(center.z > 0.0) * 3;
            let mut tier = 0.0;
            while tier < tiers {
                let pos = Vec3::new(
                    center.x,
                    obstacle.base + size.y / tiers * (tier + 0.5),
                    center.z,
                );
                for (offset, color, roughness, metallic) in [
                    (
                        if long { BOX40 } else { BOX20 },
                        CONTAINER_PAINT[row + column],
                        0.58,
                        0.0,
                    ),
                    (
                        if long { HARDWARE40 } else { HARDWARE20 },
                        Vec3::ONE,
                        0.38,
                        0.8,
                    ),
                ] {
                    frame.instances.push(
                        Instance::new(pos, scale, color)
                            .with_rot(rot)
                            .with_mesh(self.base + offset)
                            .with_surface(roughness, metallic)
                            .with_wetness(),
                    );
                }
                tier += 1.0;
            }
        } else {
            let roof = obstacle.kind == Cover::Roof;
            let crane_leg = CRANE_LEGS.contains(obstacle);
            let color = if crane_leg {
                crane_leg_tint()
            } else if roof {
                Vec3::new(0.57, 0.61, 0.61)
            } else if obstacle.kind == Cover::Crate {
                Vec3::new(0.55, 0.45, 0.31)
            } else {
                Vec3::new(0.64, 0.65, 0.61)
            };
            let roughness = if roof || crane_leg {
                0.55
            } else if obstacle.kind == Cover::Crate {
                0.9
            } else {
                0.92
            };
            frame.instances.push(
                Instance::new(center, size, color)
                    .with_mesh(self.base + if roof || crane_leg { ROOF } else { SOLID })
                    .with_surface(roughness, 0.0)
                    .with_wetness(),
            );
        }
    }
}

/// Registration is a fixed material/shape list. Static decorations are built
/// once, not regenerated by the render loop. No runtime file fetches.
pub fn harbor_meshes() -> Vec<MeshData> {
    let palette = palette_texture();
    let mut structure = Builder::new(Some(palette.clone()));
    let mut markings = Builder::new(Some(palette.clone()));
    ship(&mut structure);
    for z in CRANE_CENTERS_Z {
        crane(&mut structure, z);
    }
    dock_details(&mut structure, &mut markings);
    warehouse_details(&mut structure);
    boundary(&mut structure);
    let mut asphalt = Builder::new(Some(surface_texture(STONE, 128, [75, 79, 79])));
    asphalt.horizontal(
        [-FAR_GROUND, -FAR_GROUND],
        [HARBOR_HALF, FAR_GROUND],
        -0.025,
        3.5,
    );
    let mut quay = Builder::new(Some(surface_texture(STONE, 128, [156, 158, 149])));
    quay.horizontal([30.0, -FAR_GROUND], [HARBOR_HALF, FAR_GROUND], -0.012, 4.0);
    quay.box_plain(
        Vec3::new(48.3, -2.1, 0.0),
        Vec3::new(0.6, 4.2, FAR_GROUND * 2.0),
    );
    let sea = water();
    let mut body20 = container_body(CONTAINER_20);
    let mut body40 = container_body(CONTAINER_40);
    let neutral_container = neutral_atlas();
    body20.texture = Some(neutral_container.clone());
    body40.texture = Some(neutral_container);
    let mut hardware20 = Builder::new(Some(palette.clone()));
    let mut hardware40 = Builder::new(Some(palette));
    container_hardware(&mut hardware20, CONTAINER_20);
    container_hardware(&mut hardware40, CONTAINER_40);
    let meshes = vec![
        asphalt.mesh,
        quay.mesh,
        sea,
        structure.mesh,
        markings.mesh,
        body20,
        body40,
        hardware20.mesh,
        hardware40.mesh,
        MeshData::textured_box(2.0, Some(surface_texture(STONE, 128, [209, 209, 203]))),
        MeshData::textured_box(5.0, Some(surface_texture(STEEL, 128, STEEL_TILE_COLOR))),
    ];
    debug_assert_eq!(meshes.len(), MESH_COUNT);
    tracing::info!(
        meshes = meshes.len(),
        triangles = meshes
            .iter()
            .map(|mesh| mesh.vertices.len() / 3)
            .sum::<usize>(),
        texture_bytes = meshes
            .iter()
            .filter_map(|mesh| mesh.texture.as_ref())
            .map(|texture| texture.rgba8.len())
            .sum::<usize>(),
        "authored harbor built"
    );
    meshes
}

/// A neutral crop avoids importing the old material's border, grout or frame.
fn source_pixels(bytes: &[u8]) -> TextureData {
    crate::props::tex(bytes, "harbor reused material").unwrap_or_else(|| TextureData {
        width: 1,
        height: 1,
        rgba8: vec![160, 160, 160, 255],
    })
}

fn surface_texture(bytes: &[u8], size: u32, tint: [u8; 3]) -> TextureData {
    let source = source_pixels(bytes);
    let mut rgba8 = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let sx = source.width / 4 + x * source.width / (size * 2);
            let sy = source.height / 4 + y * source.height / (size * 2);
            let at = ((sy * source.width + sx) * 4) as usize;
            let luminance = (u32::from(source.rgba8[at])
                + u32::from(source.rgba8[at + 1])
                + u32::from(source.rgba8[at + 2]))
                / 3;
            for channel in tint {
                rgba8.push(((u32::from(channel) * (208 + luminance / 4)) / 255).min(255) as u8);
            }
            rgba8.push(255);
        }
    }
    TextureData {
        width: size,
        height: size,
        rgba8,
    }
}

fn palette_texture() -> TextureData {
    let mut rgba8 = vec![255; 256 * 256 * 4];
    for (index, paint) in PAINT.into_iter().enumerate() {
        let tile = surface_texture(STEEL, 64, paint);
        for y in 0..64 {
            let start = ((index / 4 * 64 + y) * 256 + index % 4 * 64) * 4;
            rgba8[start..start + 256].copy_from_slice(&tile.rgba8[y * 256..(y + 1) * 256]);
        }
    }
    TextureData {
        width: 256,
        height: 256,
        rgba8,
    }
}

fn neutral_atlas() -> TextureData {
    let source = source_pixels(CONTAINER);
    let size = 512;
    let mut rgba8 = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let at = ((y * source.height as usize / size) * source.width as usize
                + x * source.width as usize / size)
                * 4;
            let grey = (u16::from(source.rgba8[at]) * 2
                + u16::from(source.rgba8[at + 1])
                + u16::from(source.rgba8[at + 2]))
                / 4;
            let value = (grey + 70).min(255) as u8;
            rgba8.extend_from_slice(&[value, value, value, 255]);
        }
    }
    TextureData {
        width: size as u32,
        height: size as u32,
        rgba8,
    }
}

struct Builder {
    mesh: MeshData,
}

impl Builder {
    const fn new(texture: Option<TextureData>) -> Self {
        Self {
            mesh: MeshData {
                vertices: Vec::new(),
                texture,
            },
        }
    }

    fn quad(&mut self, corners: [Vec3; 4], uv: [[f32; 2]; 4]) {
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or_zero();
        if normal.length_squared() < 0.5 {
            return;
        }
        for index in [0, 1, 2, 0, 2, 3] {
            self.mesh.vertices.push(MeshVertex {
                pos: corners[index].to_array(),
                normal: normal.to_array(),
                uv: uv[index],
            });
        }
    }

    fn paint_quad(&mut self, corners: [Vec3; 4], paint: usize) {
        let u = (paint % 4) as f32 * 0.25 + 0.015_625;
        let v = (paint / 4) as f32 * 0.25 + 0.015_625;
        self.quad(
            corners,
            [
                [u, v],
                [u + 0.21875, v],
                [u + 0.21875, v + 0.21875],
                [u, v + 0.21875],
            ],
        );
    }

    fn box_plain(&mut self, center: Vec3, size: Vec3) {
        self.add_box(center, size, Quat::IDENTITY, None);
    }

    fn box_paint(&mut self, center: Vec3, size: Vec3, paint: usize) {
        self.add_box(center, size, Quat::IDENTITY, Some(paint));
    }

    fn add_box(&mut self, center: Vec3, size: Vec3, rotation: Quat, paint: Option<usize>) {
        for (normal, tangent) in [
            (Vec3::X, Vec3::Z),
            (-Vec3::X, -Vec3::Z),
            (Vec3::Z, -Vec3::X),
            (-Vec3::Z, Vec3::X),
            (Vec3::Y, Vec3::X),
            (-Vec3::Y, Vec3::X),
        ] {
            let vertical = normal.cross(tangent);
            let corners = [
                normal - tangent - vertical,
                normal + tangent - vertical,
                normal + tangent + vertical,
                normal - tangent + vertical,
            ]
            .map(|point| center + rotation * (point * size * 0.5));
            if let Some(paint) = paint {
                self.paint_quad(corners, paint);
            } else {
                self.quad(corners, [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]);
            }
        }
    }

    fn beam(&mut self, start: Vec3, end: Vec3, width: f32, paint: usize) {
        let direction = end - start;
        if direction.length_squared() < 1e-8 {
            return;
        }
        self.add_box(
            (start + end) * 0.5,
            Vec3::new(width, direction.length(), width),
            Quat::from_rotation_arc(Vec3::Y, direction.normalize()),
            Some(paint),
        );
    }

    fn horizontal(&mut self, min: [f32; 2], max: [f32; 2], y: f32, tile_metres: f32) {
        let tiles_x = (max[0] - min[0]) / tile_metres;
        let tiles_z = (max[1] - min[1]) / tile_metres;
        self.quad(
            [
                Vec3::new(min[0], y, min[1]),
                Vec3::new(min[0], y, max[1]),
                Vec3::new(max[0], y, max[1]),
                Vec3::new(max[0], y, min[1]),
            ],
            [
                [0.0, 0.0],
                [0.0, tiles_z],
                [tiles_x, tiles_z],
                [tiles_x, 0.0],
            ],
        );
    }
}

fn container_body(length: f32) -> MeshData {
    let half = Vec3::new(length, CONTAINER_H, CONTAINER_W) * 0.5;
    let faces = [
        [0.0, 0.0, 0.5, 0.5],
        [0.0, 0.0, 0.5, 0.5],
        [0.5, 0.0, 1.0, 0.5],
        [0.0, 0.0, 0.5, 0.5],
        [0.0, 0.5, 0.5, 1.0],
        [0.5, 0.5, 1.0, 1.0],
    ];
    let mut mesh = crate::props::atlas_box(faces, None);
    for vertex in &mut mesh.vertices {
        vertex.pos = (Vec3::from_array(vertex.pos)
            * Vec3::new(length - 0.06, CONTAINER_H - 0.06, CONTAINER_W - 0.07))
        .to_array();
    }
    let mut builder = Builder { mesh };
    // Trapezoidal corrugations run vertically; all crests stay on the ISO face.
    let ribs = if length > 9.0 { 40 } else { 20 };
    for side in [-1.0, 1.0] {
        for index in 0..ribs {
            let x0 = -half.x + 0.16 + (length - 0.32) * index as f32 / ribs as f32;
            let x1 = x0 + (length - 0.32) / ribs as f32 * 0.65;
            let z_back = side * (half.z - 0.035);
            let z_front = side * half.z;
            let path = [
                (x0, z_back),
                (x0 + 0.035, z_front),
                (x1 - 0.035, z_front),
                (x1, z_back),
            ];
            for edge in path.windows(2) {
                let [(xa, za), (xb, zb)] = [edge[0], edge[1]];
                let u0 = f32::midpoint(xa / length, 0.5);
                let u1 = f32::midpoint(xb / length, 0.5);
                let mut corners = [
                    Vec3::new(xa, -half.y + 0.08, za),
                    Vec3::new(xb, -half.y + 0.08, zb),
                    Vec3::new(xb, half.y - 0.08, zb),
                    Vec3::new(xa, half.y - 0.08, za),
                ];
                let mut uvs = [[u0, 0.48], [u1, 0.48], [u1, 0.02], [u0, 0.02]];
                if side < 0.0 {
                    corners.reverse();
                    uvs.reverse();
                }
                builder.quad(corners, uvs);
            }
        }
    }
    builder.mesh
}

fn container_hardware(builder: &mut Builder, length: f32) {
    let half = Vec3::new(length, CONTAINER_H, CONTAINER_W) * 0.5;
    for x in [-half.x + 0.08, half.x - 0.08] {
        for z in [-half.z + 0.07, half.z - 0.07] {
            builder.box_paint(
                Vec3::new(x, 0.0, z),
                Vec3::new(0.10, CONTAINER_H - 0.1, 0.10),
                5,
            );
            for y in [-half.y + 0.075, half.y - 0.075] {
                builder.box_paint(Vec3::new(x, y, z), Vec3::new(0.16, 0.15, 0.14), 5);
                // Recessed twist-lock pocket; a dark inset, not a hole through cover.
                builder.box_paint(
                    Vec3::new(x, y, z.signum() * (half.z - 0.002)),
                    Vec3::new(0.075, 0.052, 0.004),
                    4,
                );
            }
        }
    }
    for z in [-0.85, -0.32, 0.32, 0.85] {
        builder.box_paint(
            Vec3::new(half.x - 0.017, 0.0, z),
            Vec3::new(0.033, CONTAINER_H - 0.20, 0.033),
            5,
        );
        builder.box_paint(
            Vec3::new(half.x - 0.012, -0.36, z + 0.07),
            Vec3::new(0.023, 0.045, 0.17),
            5,
        );
    }
    for y in [-half.y + 0.055, half.y - 0.055] {
        for z in [-half.z + 0.04, half.z - 0.04] {
            builder.box_paint(Vec3::new(0.0, y, z), Vec3::new(length, 0.11, 0.08), 5);
        }
    }
}

fn water() -> MeshData {
    let mut builder = Builder::new(Some(surface_texture(STEEL, 128, PAINT[12])));
    // Long low swells, fixed authored geometry. Cosmetic sea never moves the
    // simulation floor and is outside the quay's physical perimeter.
    // Preserve every original 6m near-quay cell. A coarse outer skirt shares
    // its boundary vertices, so there are no overlaps or cracked seams.
    let mut xs: Vec<f32> = (0..=40).map(|index| 48.0 + index as f32 * 6.0).collect();
    xs.push(FAR_GROUND);
    let mut zs = vec![-FAR_GROUND];
    zs.extend((0..=64).map(|index| -192.0 + index as f32 * 6.0));
    zs.push(FAR_GROUND);
    let point = |x: f32, z: f32| Vec3::new(x, -2.25 + (x * 0.65 + z * 0.28).sin() * 0.055, z);
    for xx in xs.windows(2) {
        for zz in zs.windows(2) {
            let corners = [
                point(xx[0], zz[0]),
                point(xx[0], zz[1]),
                point(xx[1], zz[1]),
                point(xx[1], zz[0]),
            ];
            builder.quad(
                corners,
                corners.map(|corner| [corner.x / 6.0, corner.z / 6.0]),
            );
        }
    }
    builder.mesh
}

fn ship(builder: &mut Builder) {
    ship_hull(builder);
    ship_bridge(builder);
    ship_cargo(builder);
}

fn ship_hull(builder: &mut Builder) {
    // A compact 110 m feeder, not a box-shaped super-ship: narrowed transom,
    // flared shoulders, a tapered bow and a raked submerged lower hull.
    let stations: [(f32, f32); 7] = [
        (-54.0, 5.8),
        (-48.0, 8.5),
        (-36.0, 9.0),
        (28.0, 9.0),
        (43.0, 7.5),
        (52.0, 3.7),
        (57.0, 0.15),
    ];
    for pair in stations.windows(2) {
        let [(z0, width0), (z1, width1)] = [pair[0], pair[1]];
        for side in [-1.0, 1.0] {
            let mut lower = [
                Vec3::new(64.0 + side * width0 * 0.73, -5.2, z0),
                Vec3::new(64.0 + side * width1 * 0.73, -5.2, z1),
                Vec3::new(64.0 + side * width1, 0.0, z1),
                Vec3::new(64.0 + side * width0, 0.0, z0),
            ];
            let mut upper = [
                Vec3::new(64.0 + side * width0, 0.0, z0),
                Vec3::new(64.0 + side * width1, 0.0, z1),
                Vec3::new(64.0 + side * width1, 4.8, z1),
                Vec3::new(64.0 + side * width0, 4.8, z0),
            ];
            if side > 0.0 {
                lower.reverse();
                upper.reverse();
            }
            builder.paint_quad(lower, 3);
            builder.paint_quad(upper, 2);
            builder.beam(
                Vec3::new(64.0 + side * width0, 4.8, z0),
                Vec3::new(64.0 + side * width1, 4.8, z1),
                0.16,
                3,
            );
            builder.beam(
                Vec3::new(64.0 + side * width0, 5.7, z0),
                Vec3::new(64.0 + side * width1, 5.7, z1),
                0.055,
                0,
            );
        }
        builder.paint_quad(
            [
                Vec3::new(64.0 - width0, 4.78, z0),
                Vec3::new(64.0 - width1, 4.78, z1),
                Vec3::new(64.0 + width1, 4.78, z1),
                Vec3::new(64.0 + width0, 4.78, z0),
            ],
            8,
        );
    }
    builder.box_paint(Vec3::new(64.0, 0.0, -54.0), Vec3::new(11.6, 9.5, 0.12), 2);
}

fn ship_bridge(builder: &mut Builder) {
    // Raised aft accommodation and wheelhouse with wraparound window bands.
    builder.box_paint(Vec3::new(64.0, 8.0, -43.0), Vec3::new(13.8, 6.4, 15.0), 11);
    builder.box_paint(Vec3::new(64.0, 12.2, -39.0), Vec3::new(15.2, 2.1, 8.0), 0);
    builder.box_paint(Vec3::new(64.0, 13.38, -39.0), Vec3::new(16.1, 0.26, 9.0), 2);
    for level in [7.1, 9.4, 12.3] {
        for column in -3..=3 {
            builder.box_paint(
                Vec3::new(
                    64.0 + column as f32 * 1.75,
                    level,
                    if level > 11.0 { -34.97 } else { -35.47 },
                ),
                Vec3::new(1.25, 1.05, 0.06),
                7,
            );
        }
        for side in [-1.0, 1.0] {
            for z in [-46.0, -42.7, -39.4] {
                builder.box_paint(
                    Vec3::new(
                        64.0 + side * if level > 11.0 { 7.63 } else { 6.93 },
                        level,
                        z,
                    ),
                    Vec3::new(0.06, 0.95, 1.8),
                    7,
                );
            }
        }
    }
    builder.box_paint(Vec3::new(64.0, 14.5, -47.0), Vec3::new(3.2, 4.0, 3.0), 3);
    builder.box_paint(
        Vec3::new(64.0, 16.55, -47.0),
        Vec3::new(3.35, 0.35, 3.15),
        4,
    );
    builder.beam(
        Vec3::new(64.0, 13.5, -39.0),
        Vec3::new(64.0, 21.0, -39.0),
        0.22,
        0,
    );
    builder.beam(
        Vec3::new(61.0, 18.5, -39.0),
        Vec3::new(67.0, 18.5, -39.0),
        0.14,
        0,
    );
    for z in (-48..=48).step_by(4) {
        for x in [55.1, 72.9] {
            if z < 28 {
                builder.beam(
                    Vec3::new(x, 4.8, z as f32),
                    Vec3::new(x, 5.7, z as f32),
                    0.045,
                    0,
                );
            }
        }
    }
}

fn ship_cargo(builder: &mut Builder) {
    // Cargo cells lie parallel to the ship's keel. These are scenery outside
    // the arena, so their low-detail corrugations are batched in this mesh.
    for row in 0..4 {
        for lane in 0..5 {
            for tier in 0..2 {
                let center = Vec3::new(
                    58.2 + lane as f32 * 2.7,
                    4.8 + CONTAINER_H * (tier as f32 + 0.5),
                    -25.0 + row as f32 * 13.0,
                );
                let paint = [3, 14, 10, 9, 11][(row + lane + tier) % 5];
                builder.box_paint(
                    center,
                    Vec3::new(CONTAINER_W, CONTAINER_H, CONTAINER_40),
                    paint,
                );
                for rib in 0..16 {
                    let z = center.z - 5.75 + rib as f32 * 0.75;
                    for side in [-1.0, 1.0] {
                        builder.box_paint(
                            Vec3::new(center.x + side * 1.22, center.y, z),
                            Vec3::new(0.05, 2.42, 0.10),
                            paint,
                        );
                    }
                }
            }
        }
    }
    for z in [-43.0, 42.0] {
        builder.beam(
            Vec3::new(48.1, 0.35, z - 4.0),
            Vec3::new(55.0, 4.9, z),
            0.09,
            15,
        );
    }
}

fn crane(builder: &mut Builder, z: f32) {
    let paint = CRANE_PAINT;
    // Lower legs are the authoritative solids drawn by push_cover. The
    // superstructure begins at their top and follows their actual centres.
    for leg in CRANE_LEGS
        .iter()
        .filter(|leg| (f32::midpoint(leg.min[1], leg.max[1]) - z).abs() < 4.0)
    {
        let x = f32::midpoint(leg.min[0], leg.max[0]);
        let zz = f32::midpoint(leg.min[1], leg.max[1]);
        builder.beam(Vec3::new(x, leg.h, zz), Vec3::new(x, 25.0, zz), 1.0, paint);
    }
    for side in [-1.0, 1.0] {
        let zz = z + side * 2.6;
        builder.beam(
            Vec3::new(32.0, 25.0, zz),
            Vec3::new(44.0, 25.0, zz),
            1.2,
            paint,
        );
        builder.beam(
            Vec3::new(32.0, 15.0, zz),
            Vec3::new(38.0, 25.0, zz),
            0.5,
            paint,
        );
        builder.beam(
            Vec3::new(44.0, 15.0, zz),
            Vec3::new(38.0, 25.0, zz),
            0.5,
            paint,
        );
        // The boom is an open truss, with alternating diagonals, not a slab.
        for y in [31.0, 34.0] {
            builder.beam(Vec3::new(13.0, y, zz), Vec3::new(91.0, y, zz), 0.55, paint);
        }
        for segment in 0..13 {
            let x = 13.0 + segment as f32 * 6.0;
            builder.beam(
                Vec3::new(x, 31.0, zz),
                Vec3::new(x + 6.0, 34.0, zz),
                0.28,
                0,
            );
            builder.beam(
                Vec3::new(x, 34.0, zz),
                Vec3::new(x + 6.0, 31.0, zz),
                0.28,
                paint,
            );
        }
        builder.beam(
            Vec3::new(35.0, 25.0, zz),
            Vec3::new(38.0, 48.0, zz),
            0.75,
            paint,
        );
        builder.beam(
            Vec3::new(42.0, 25.0, zz),
            Vec3::new(38.0, 48.0, zz),
            0.75,
            paint,
        );
        for end in [13.0, 89.0] {
            builder.beam(
                Vec3::new(38.0, 47.8, zz),
                Vec3::new(end, 34.0, zz),
                0.095,
                5,
            );
        }
    }
    for x in [13.0, 32.0, 44.0, 62.0, 80.0, 91.0] {
        builder.beam(
            Vec3::new(x, 31.0, z - 2.6),
            Vec3::new(x, 31.0, z + 2.6),
            0.45,
            paint,
        );
    }
    builder.box_paint(Vec3::new(29.0, 28.5, z), Vec3::new(10.0, 4.0, 5.2), 0);
    builder.box_paint(Vec3::new(62.0, 30.8, z), Vec3::new(3.5, 1.2, 5.0), 2);
    builder.box_paint(Vec3::new(60.5, 28.8, z - 3.9), Vec3::new(2.0, 2.7, 1.8), 6);
    builder.box_paint(
        Vec3::new(59.46, 28.9, z - 3.9),
        Vec3::new(0.05, 1.8, 1.4),
        7,
    );
    for x in [60.9, 63.1] {
        // Hoist crossheads physically support each vertical cable; the narrow
        // travel bogie alone would leave the outer anchors hanging in air.
        builder.box_paint(Vec3::new(x, 30.15, z), Vec3::new(0.20, 0.30, 10.0), 2);
        for dz in [-4.8, 4.8] {
            builder.beam(
                Vec3::new(x, 30.1, z + dz),
                Vec3::new(x, 15.0, z + dz),
                0.045,
                4,
            );
        }
    }
    builder.box_paint(Vec3::new(62.0, 14.8, z), Vec3::new(2.5, 0.45, 10.0), 6);
}

fn dock_details(builder: &mut Builder, markings: &mut Builder) {
    // Physical fence/curb collisions come from the core. Rails are flush,
    // and bollards/fenders are outside x48, never phantom cover in the apron.
    for x in [32.0, 44.0] {
        markings.box_paint(Vec3::new(x, 0.007, 0.0), Vec3::new(0.12, 0.018, 96.0), 5);
    }
    for z in (-44..=44).step_by(8) {
        let z = z as f32;
        builder.box_paint(Vec3::new(48.8, -0.5, z), Vec3::new(0.55, 2.4, 1.4), 4);
        builder.box_paint(Vec3::new(48.65, 0.30, z), Vec3::new(0.55, 0.60, 0.55), 6);
        builder.box_paint(Vec3::new(48.65, 0.64, z), Vec3::new(0.95, 0.18, 0.60), 6);
    }
    for x in [-25.0, 25.0] {
        for z in (-44..=44).step_by(8) {
            markings.box_paint(
                Vec3::new(x, 0.008, z as f32),
                Vec3::new(0.12, 0.016, 3.8),
                13,
            );
        }
    }
    for x in [-28.0, -22.0, 22.0, 28.0, 46.5] {
        markings.box_paint(Vec3::new(x, 0.009, 0.0), Vec3::new(0.10, 0.018, 95.0), 6);
    }
    for z in [-34.0, 0.0, 34.0] {
        markings.box_paint(Vec3::new(0.0, 0.01, z), Vec3::new(43.0, 0.018, 0.12), 6);
    }
    // Road arrows and zebra crossings show circulation rather than randomly
    // sprinkling painted stripes between containers.
    for x in [-25.0, 25.0, 38.0] {
        for z in [-31.0, 31.0] {
            markings.box_paint(Vec3::new(x, 0.02, z), Vec3::new(0.18, 0.025, 2.4), 13);
            for side in [-1.0, 1.0] {
                markings.beam(
                    Vec3::new(x, 0.025, z + 1.4),
                    Vec3::new(x + side * 0.7, 0.025, z + 0.4),
                    0.16,
                    13,
                );
            }
        }
    }
    for step in 0..7 {
        markings.box_paint(
            Vec3::new(21.0 + step as f32 * 0.6, 0.019, 34.0),
            Vec3::new(0.3, 0.026, 3.0),
            13,
        );
    }
    // Perimeter mast locations are beyond the walkable limit.
    for (x, z) in [
        (-50.0, -38.0),
        (-50.0, 0.0),
        (-50.0, 38.0),
        (47.5, -54.0),
        (47.5, 54.0),
    ] {
        builder.beam(Vec3::new(x, 0.0, z), Vec3::new(x, 17.0, z), 0.22, 5);
        builder.beam(
            Vec3::new(x - 2.0, 17.0, z),
            Vec3::new(x + 2.0, 17.0, z),
            0.18,
            5,
        );
        for dx in [-1.5, -0.5, 0.5, 1.5] {
            builder.box_paint(Vec3::new(x + dx, 16.8, z), Vec3::new(0.65, 0.28, 0.6), 11);
        }
    }
}

fn warehouse_details(builder: &mut Builder) {
    // Ridge and clerestory sit wholly above the authoritative warehouse roof.
    let center_x = f32::midpoint(WAREHOUSE_MIN[0], WAREHOUSE_MAX[0]);
    builder.box_paint(
        Vec3::new(center_x, WAREHOUSE_ROOF_BASE + 0.8, 0.0),
        Vec3::new(7.0, 0.8, 51.0),
        14,
    );
    // Folded roof seams and wall frames are shallow details against solids,
    // never beams spanning the actual north/south/east door openings.
    for z in (-27..=27).step_by(2) {
        builder.box_paint(
            Vec3::new(center_x, 4.58, z as f32),
            Vec3::new(15.9, 0.06, 0.08),
            5,
        );
    }
    for wall in WAREHOUSE
        .iter()
        .filter(|wall| wall.kind == Cover::Wall && wall.h > 4.0)
    {
        let x = f32::midpoint(wall.min[0], wall.max[0]);
        let z = f32::midpoint(wall.min[1], wall.max[1]);
        let size = Vec3::new(wall.max[0] - wall.min[0], 0.15, wall.max[1] - wall.min[1]);
        builder.box_paint(Vec3::new(x, 3.95, z), size + Vec3::new(0.03, 0.0, 0.03), 1);
    }
    for z in (-24..=24).step_by(4) {
        builder.box_paint(
            Vec3::new(-32.47, 5.0, z as f32),
            Vec3::new(0.05, 0.48, 2.8),
            7,
        );
        builder.box_paint(
            Vec3::new(-39.53, 5.0, z as f32),
            Vec3::new(0.05, 0.48, 2.8),
            7,
        );
    }
}

fn boundary(builder: &mut Builder) {
    // Visible inner face is exactly the hard boundary x/z=48. The actual
    // player centre stops its radius short of that face. Perimeter guardrails
    // identify water/ship as scenery, not an invisible no-swimming rule.
    let limit = HARBOR_HALF + 0.10;
    for x in [-limit, limit] {
        builder.box_paint(Vec3::new(x, 0.24, 0.0), Vec3::new(0.20, 0.48, 96.2), 8);
        for y in [0.85, 1.45, 2.05] {
            builder.box_paint(Vec3::new(x, y, 0.0), Vec3::new(0.07, 0.07, 96.2), 5);
        }
        for z in -24..=24 {
            builder.box_paint(
                Vec3::new(x, 1.22, z as f32 * 2.0),
                Vec3::new(0.09, 2.44, 0.09),
                5,
            );
        }
    }
    for z in [-limit, limit] {
        builder.box_paint(Vec3::new(0.0, 0.24, z), Vec3::new(96.2, 0.48, 0.20), 8);
        for y in [0.85, 1.45, 2.05] {
            builder.box_paint(Vec3::new(0.0, y, z), Vec3::new(96.2, 0.07, 0.07), 5);
        }
        for x in -24..=24 {
            builder.box_paint(
                Vec3::new(x as f32 * 2.0, 1.22, z),
                Vec3::new(0.09, 2.44, 0.09),
                5,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_surface(instance: &Instance, roughness: f32, metallic: f32) {
        let surface = instance.surface.expect("authored harbor surface");
        assert!((surface.roughness - roughness).abs() < 1e-6);
        assert!((surface.metallic - metallic).abs() < 1e-6);
    }

    #[test]
    fn crane_columns_share_the_upper_structures_paint_in_linear_light() {
        let upper_paint = Vec3::from_array(PAINT[CRANE_PAINT].map(srgb_component));
        let base_tile = Vec3::from_array(STEEL_TILE_COLOR.map(srgb_component));
        assert!((base_tile * crane_leg_tint() - upper_paint).length() < 1e-6);
        // The previous independently chosen tint visibly bleached the lower
        // twelve metres of the otherwise continuous painted columns.
        let previous = base_tile * Vec3::new(0.30, 0.48, 0.58);
        assert!((previous - upper_paint).length() > 0.25);
        let mut frame = Frame::default();
        HarborArt::new(100).push_cover(&mut frame, &CRANE_LEGS[0]);
        assert!((frame.instances[0].color - crane_leg_tint()).length() < 1e-6);

        // Compare actual textured grain as well as nominal palette values.
        // Eight-bit quantization and encoded grain modulation make the match
        // approximate, but keep its linear RGB error below a visible seam.
        let upper = surface_texture(STEEL, 64, PAINT[CRANE_PAINT]);
        let lower = surface_texture(STEEL, 64, STEEL_TILE_COLOR);
        for (upper_pixel, lower_pixel) in upper
            .rgba8
            .as_chunks::<4>()
            .0
            .iter()
            .zip(lower.rgba8.as_chunks::<4>().0)
        {
            let upper_rgb = Vec3::from_array(
                [upper_pixel[0], upper_pixel[1], upper_pixel[2]].map(srgb_component),
            );
            let lower_rgb = Vec3::from_array(
                [lower_pixel[0], lower_pixel[1], lower_pixel[2]].map(srgb_component),
            ) * crane_leg_tint();
            assert!((upper_rgb - lower_rgb).abs().max_element() < 0.006);
        }
    }

    #[test]
    fn dry_weather_keeps_reflective_water_and_distinct_paving_paint_and_hardware() {
        let art = HarborArt::new(100);
        let mut frame = Frame {
            environment: ember_engine::Environment::outdoor(ember_engine::Weather::Cloudy, 0.0),
            ..Frame::default()
        };
        assert!(frame.environment.wetness < 0.001);
        art.push_ground(&mut frame);
        assert_surface(&frame.instances[0], 0.98, 0.0);
        assert_surface(&frame.instances[1], 0.92, 0.0);
        assert_surface(&frame.instances[2], 0.12, 0.0);
        assert!(!frame.instances[2].casts_shadow);
        art.push_decor(&mut frame);
        assert_surface(&frame.instances[3], 0.55, 0.0);
        assert_surface(&frame.instances[4], 0.85, 0.0);

        frame.instances.clear();
        art.push_cover(&mut frame, &arena_core::harbor::CONTAINERS[0].obstacle());
        assert!(frame.instances.len() >= 2);
        let (pairs, remainder) = frame.instances.as_chunks::<2>();
        assert!(remainder.is_empty());
        for pair in pairs {
            assert_surface(&pair[0], 0.58, 0.0);
            assert_surface(&pair[1], 0.38, 0.8);
            assert!(pair[0].casts_shadow && pair[1].casts_shadow);
        }
        frame.instances.clear();
        art.push_cover(&mut frame, &CRANE_LEGS[0]);
        assert_surface(&frame.instances[0], 0.55, 0.0);
        frame.instances.clear();
        art.push_cover(&mut frame, &WAREHOUSE[0]);
        assert_surface(&frame.instances[0], 0.92, 0.0);
    }

    #[test]
    fn meshes_are_finite_textured_batched_and_inside_the_budget() {
        let meshes = harbor_meshes();
        assert_eq!(meshes.len(), MESH_COUNT);
        let mut vertices = 0;
        let mut bytes = 0;
        for mesh in &meshes {
            assert!(!mesh.vertices.is_empty());
            assert_eq!(mesh.vertices.len() % 3, 0);
            vertices += mesh.vertices.len();
            for vertex in &mesh.vertices {
                assert!(Vec3::from_array(vertex.pos).is_finite());
                assert!((Vec3::from_array(vertex.normal).length() - 1.0).abs() < 1e-4);
                assert!(vertex.uv.iter().all(|v| v.is_finite()));
            }
            let texture = mesh
                .texture
                .as_ref()
                .expect("all harbor surfaces are textured");
            assert!(texture.width <= 512 && texture.height <= 512);
            assert_eq!(
                texture.rgba8.len(),
                (texture.width * texture.height * 4) as usize
            );
            bytes += texture.rgba8.len();
        }
        assert!(vertices < 200_000, "harbor vertices {vertices}");
        assert!(bytes < 4_000_000, "base texture bytes {bytes}");
        let mut frame = Frame::default();
        let art = HarborArt::new(100);
        art.push_ground(&mut frame);
        art.push_decor(&mut frame);
        assert_eq!(frame.instances.len(), 5, "all scenery is five draw units");
    }

    #[test]
    fn dimensional_container_envelopes_and_raised_cover_match_collision() {
        let meshes = harbor_meshes();
        let art = HarborArt::new(1);
        for along_x in [true, false] {
            for length in [CONTAINER_20, CONTAINER_40] {
                let (width, depth) = if along_x {
                    (length, CONTAINER_W)
                } else {
                    (CONTAINER_W, length)
                };
                let obstacle = Obstacle::boxed(
                    Cover::Container,
                    [3.0, 5.0],
                    [3.0 + width, 5.0 + depth],
                    1.0,
                    1.0 + 2.0 * CONTAINER_H,
                );
                let mut frame = Frame::default();
                art.push_cover(&mut frame, &obstacle);
                assert_eq!(frame.instances.len(), 4);
                let mut min = Vec3::splat(f32::INFINITY);
                let mut max = Vec3::splat(f32::NEG_INFINITY);
                for instance in &frame.instances {
                    for vertex in &meshes[instance.mesh as usize - 1].vertices {
                        let point = instance.position
                            + instance.rot * (Vec3::from_array(vertex.pos) * instance.scale);
                        min = min.min(point);
                        max = max.max(point);
                    }
                }
                assert!(min.distance(Vec3::new(3.0, 1.0, 5.0)) < 1e-4, "{min}");
                assert!(
                    max.distance(Vec3::new(3.0 + width, obstacle.h, 5.0 + depth)) < 1e-4,
                    "{max}"
                );
            }
        }
    }

    #[test]
    fn dock_scenery_does_not_add_phantom_cover_to_the_playable_routes() {
        let meshes = harbor_meshes();
        for vertex in &meshes[STRUCTURES as usize].vertices {
            let point = Vec3::from_array(vertex.pos);
            if point.y < 3.7 {
                assert!(
                    point.x.abs() >= HARBOR_HALF || point.z.abs() >= HARBOR_HALF,
                    "low scenery {point} has no authoritative obstacle"
                );
            }
        }
        // In particular a crane starts exactly at its shared leg's top,
        // never with oversized uncollidable bogies between the drive lanes.
        for leg in CRANE_LEGS {
            let center = Vec3::new(
                f32::midpoint(leg.min[0], leg.max[0]),
                leg.h,
                f32::midpoint(leg.min[1], leg.max[1]),
            );
            assert!(
                meshes[STRUCTURES as usize].vertices.iter().any(|vertex| {
                    let point = Vec3::from_array(vertex.pos);
                    (point.y - center.y).abs() < 1e-5
                        && (point.x - center.x).abs() <= 0.501
                        && (point.z - center.z).abs() <= 0.501
                }),
                "no upper crane leg seats on {center}"
            );
        }
    }

    #[test]
    fn all_non_container_solids_fill_their_authoritative_boxes() {
        let art = HarborArt::new(1);
        let meshes = harbor_meshes();
        let level = arena_core::shooter::Level::harbor();
        for obstacle in level
            .obstacles
            .iter()
            .filter(|obstacle| !matches!(obstacle.kind, Cover::Container | Cover::Loot))
        {
            let mut frame = Frame::default();
            art.push_cover(&mut frame, obstacle);
            assert_eq!(frame.instances.len(), 1);
            let instance = frame.instances[0];
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for vertex in &meshes[instance.mesh as usize - 1].vertices {
                let point = instance.position + Vec3::from_array(vertex.pos) * instance.scale;
                min = min.min(point);
                max = max.max(point);
            }
            assert!(
                min.distance(Vec3::new(obstacle.min[0], obstacle.base, obstacle.min[1])) < 1e-4
            );
            assert!(max.distance(Vec3::new(obstacle.max[0], obstacle.h, obstacle.max[1])) < 1e-4);
        }
    }

    #[test]
    fn far_coast_extends_beyond_overview_frustum_without_changing_the_play_area() {
        let meshes = harbor_meshes();
        let asphalt = crate::props::Bounds::of(&meshes[ASPHALT as usize]);
        let sea = crate::props::Bounds::of(&meshes[WATER as usize]);
        assert_eq!(asphalt.min.x, -FAR_GROUND);
        assert_eq!(asphalt.max.x, HARBOR_HALF);
        assert_eq!((asphalt.min.z, asphalt.max.z), (-FAR_GROUND, FAR_GROUND));
        assert_eq!((sea.min.x, sea.max.x), (HARBOR_HALF, FAR_GROUND));
        assert_eq!((sea.min.z, sea.max.z), (-FAR_GROUND, FAR_GROUND));
        assert_eq!(arena_core::shooter::Level::harbor().arena_half, 48.0);
        let uv_min = meshes[ASPHALT as usize].vertices[0].uv;
        let opposite = meshes[ASPHALT as usize].vertices[2].uv;
        assert!(((opposite[0] - uv_min[0]) * 3.5 - asphalt.size().x).abs() < 0.001);
        assert!(((opposite[1] - uv_min[1]) * 3.5 - asphalt.size().z).abs() < 0.001);
        // The near-sea 6m sample grid, including its height, is retained.
        for x in [48.0, 54.0, 60.0, 288.0] {
            let expected = Vec3::new(x, -2.25 + (x * 0.65_f32).sin() * 0.055, 0.0);
            assert!(
                meshes[WATER as usize]
                    .vertices
                    .iter()
                    .any(|vertex| Vec3::from_array(vertex.pos).distance(expected) < 1e-5)
            );
        }
    }
}
