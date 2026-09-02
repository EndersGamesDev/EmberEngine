//! Everything arena v13 ("Trench City") draws that is not a player or a
//! bullet: the cover boxes by kind, the city outside the wall, the sky and
//! the ground.
//!
//! The renderer samples one base-colour texture per mesh, multiplied by the
//! instance colour, and that is the whole material model (see the table in
//! `CLAUDE.md`). So a container is a box whose six faces each map to a
//! rectangle of one picture (an *atlas box*), a trench wall is a box whose
//! faces tile one picture at about a tile per 1.5 m, and a statue is a
//! generated position-only mesh given faceted normals and a box-projected
//! material picture. Every picture is embedded with `include_bytes!`; every
//! byte here is a byte in the web bundle.
//!
//! Textured parts are pushed with `Vec3::ONE` (or a uniform grey), never a
//! tint: the picture already carries the colour, and a tint on top of it
//! double-tints.

use std::f32::consts::{FRAC_PI_2, TAU};

use ember_engine::glam::{Quat, Vec3};
use ember_engine::{Frame, Instance, MeshData, MeshVertex, TextureData};
use pong_core::shooter::{Cover, Decor, DecorKind, Obstacle};

/// One registered v13 mesh. The discriminant is the mesh's offset from the
/// base `run_online` hands to `ShooterGame::set_props`, so the order here IS
/// the registration order and `prop_meshes` builds from `ALL` to keep them
/// from drifting apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Prop {
    /// Closed shipping container: side / doors / roof / floor atlas.
    Container,
    /// Wooden crate: side / top atlas.
    Crate,
    /// Ammunition box: side / top atlas.
    Ammo,
    /// Trench wall, tiled.
    TrenchWall,
    /// Tunnel roof, tiled; the underside is what players see.
    TunnelRoof,
    /// Low rubble, tiled.
    Rubble,
    /// The statue's granite plinth, tiled.
    Plinth,
    /// The arena boundary: the balustrade picture instead of basalt.
    CityWall,
    /// The arena floor: cobbles.
    Floor,
    /// The far ground plane closing the void beyond the wall.
    Ground,
    /// The sky cylinder.
    Sky,
    /// Generated: bronze equestrian statue.
    Statue,
    /// Generated: the cathedral behind the south wall.
    Cathedral,
    /// Generated: a Haussmann façade.
    FacadeA,
    /// Generated: an art-nouveau façade.
    FacadeB,
    /// Generated: a sandbag line, fitted to every `Cover::Sandbag` box.
    Sandbags,
    /// Generated: a burnt-out car.
    Wreck,
    /// Generated: a street lamp.
    Lamp,
}

impl Prop {
    /// Every prop, in registration order.
    pub const ALL: [Self; 18] = [
        Self::Container,
        Self::Crate,
        Self::Ammo,
        Self::TrenchWall,
        Self::TunnelRoof,
        Self::Rubble,
        Self::Plinth,
        Self::CityWall,
        Self::Floor,
        Self::Ground,
        Self::Sky,
        Self::Statue,
        Self::Cathedral,
        Self::FacadeA,
        Self::FacadeB,
        Self::Sandbags,
        Self::Wreck,
        Self::Lamp,
    ];
    pub const COUNT: usize = Self::ALL.len();

    /// Offset from the registered base.
    const fn offset(self) -> u32 {
        self as u32
    }

    const fn index(self) -> usize {
        self as usize
    }
}

// ---- the pictures ----

const TEX_CONTAINER: &[u8] = include_bytes!("../../../assets/textures/v13/container.png");
const TEX_CRATE: &[u8] = include_bytes!("../../../assets/textures/v13/crate.png");
const TEX_AMMO: &[u8] = include_bytes!("../../../assets/textures/v13/ammo.png");
const TEX_TRENCH_WALL: &[u8] = include_bytes!("../../../assets/textures/v13/trench-wall.png");
const TEX_TUNNEL_ROOF: &[u8] = include_bytes!("../../../assets/textures/v13/tunnel-roof.png");
const TEX_RUBBLE: &[u8] = include_bytes!("../../../assets/textures/v13/rubble.png");
const TEX_COBBLE: &[u8] = include_bytes!("../../../assets/textures/v13/cobble.png");
const TEX_CITY_WALL: &[u8] = include_bytes!("../../../assets/textures/v13/city-wall.png");
const TEX_PLINTH: &[u8] = include_bytes!("../../../assets/textures/v13/plinth.png");
const TEX_SKY: &[u8] = include_bytes!("../../../assets/textures/v13/sky.png");
const TEX_LIMESTONE: &[u8] = include_bytes!("../../../assets/textures/v13/limestone.png");
const TEX_SANDSTONE: &[u8] = include_bytes!("../../../assets/textures/v13/sandstone.png");
const TEX_BRONZE: &[u8] = include_bytes!("../../../assets/textures/v13/bronze.png");
const TEX_BURLAP: &[u8] = include_bytes!("../../../assets/textures/v13/burlap.png");
const TEX_SCORCHED_STEEL: &[u8] = include_bytes!("../../../assets/textures/v13/scorched-steel.png");
const TEX_CAST_IRON: &[u8] = include_bytes!("../../../assets/textures/v13/cast-iron.png");

// ---- the generated meshes (Hunyuan shape output: POSITION only) ----

const GLB_STATUE: &[u8] = include_bytes!("../../../assets/models/v13/statue.glb");
const GLB_CATHEDRAL: &[u8] = include_bytes!("../../../assets/models/v13/cathedral.glb");
const GLB_FACADE_A: &[u8] = include_bytes!("../../../assets/models/v13/facade-a.glb");
const GLB_FACADE_B: &[u8] = include_bytes!("../../../assets/models/v13/facade-b.glb");
const GLB_SANDBAGS: &[u8] = include_bytes!("../../../assets/models/v13/sandbags.glb");
const GLB_WRECK: &[u8] = include_bytes!("../../../assets/models/v13/wreck.glb");
const GLB_LAMP: &[u8] = include_bytes!("../../../assets/models/v13/lamp.glb");

/// Decode an embedded PNG, or log and go untextured. The engine decodes only
/// 8-bit `R8G8B8(A8)`; anything else comes back as an untextured mesh with no
/// line of its own, so the bake asserts the format on its side.
pub fn tex(bytes: &[u8], name: &str) -> Option<TextureData> {
    match TextureData::from_png_bytes(bytes) {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!(name, "texture decode failed ({e}); untextured");
            None
        }
    }
}

/// About one picture tile per this many metres on a tiled box.
const TILE_M: f32 = 1.5;

/// Sky cylinder: radius, floor and ceiling heights, segments around.
pub const SKY_RADIUS: f32 = 60.0;
pub const SKY_Y0: f32 = -5.0;
pub const SKY_Y1: f32 = 70.0;
const SKY_SEGMENTS: u32 = 48;
/// The far ground plane is drawn this wide, at this height, dimmed this much.
pub const GROUND_SIZE: f32 = 200.0;
pub const GROUND_Y: f32 = -0.05;
pub const GROUND_DIM: f32 = 0.55;
/// The sky is lit like everything else, so its picture is over-driven to
/// read as bright sky rather than as a lit surface.
pub const SKY_DRIVE: f32 = 1.6;
/// Tiles across the arena floor (about one per 1.5 m over 50 m).
const FLOOR_TILES: f32 = 32.0;
/// Tiles across the far ground plane.
const GROUND_TILES: f32 = 40.0;

/// A single container unit is this tall; a taller `Container` box is drawn
/// as a stack of these so the picture is never stretched to twice its
/// height.
const CONTAINER_UNIT_H: f32 = 2.6;

/// (normal, tangent u, tangent v) per cube face, in the order `+Z, -Z, +X,
/// -X, +Y, -Y`. A copy of the renderer's private `CUBE_FACES`, so an atlas
/// box and `MeshData::textured_box` agree on face order and winding; the
/// test below pins that they do.
const CUBE_FACES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
    ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
    ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
    ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
];

/// A unit box whose six faces each map to one rectangle of the texture.
///
/// `faces` is `[u0, v0, u1, v1]` per face in the `CUBE_FACES` order (`+Z,
/// -Z, +X, -X, +Y, -Y`), with `u` right and `v` down as the bake lays the
/// atlas out; the picture's top edge lands on the top edge of every side
/// face, exactly as `textured_box` places it. A rectangle may run past 1 -
/// the sampler repeats - which is how the tiled boxes below are built from
/// the same function with a tile count per face instead of a uniform one.
///
/// Atlases do not tile, so the box is scaled to the obstacle's real size and
/// the picture stretches with it: right for a container, one picture per
/// face, and why containers are drawn at their authored proportions.
#[must_use]
pub fn atlas_box(faces: [[f32; 4]; 6], texture: Option<TextureData>) -> MeshData {
    let mut vertices = Vec::with_capacity(36);
    for ((n, u, v), [u0, v0, u1, v1]) in CUBE_FACES.into_iter().zip(faces) {
        let n3 = Vec3::from(n);
        let u3 = Vec3::from(u);
        let v3 = Vec3::from(v);
        let center = n3 * 0.5;
        let corners = [
            center - u3 * 0.5 - v3 * 0.5,
            center + u3 * 0.5 - v3 * 0.5,
            center + u3 * 0.5 + v3 * 0.5,
            center - u3 * 0.5 + v3 * 0.5,
        ];
        let uvs = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
        for idx in [0usize, 1, 2, 0, 2, 3] {
            vertices.push(MeshVertex {
                pos: corners[idx].to_array(),
                normal: n,
                uv: uvs[idx],
            });
        }
    }
    MeshData { vertices, texture }
}

/// A unit box tiled so that, drawn at `nominal` world size, one picture tile
/// covers about `TILE_M` on every face. The tiling is baked into the mesh,
/// so a box of another size stretches or squeezes it; `nominal` is the size
/// the kind is usually drawn at, long axis along +X (see `push_fitted`).
#[must_use]
pub fn tiled_box(nominal: Vec3, texture: Option<TextureData>) -> MeshData {
    let t = nominal / TILE_M;
    let rect = |w: f32, h: f32| [0.0, 0.0, w.max(0.05), h.max(0.05)];
    atlas_box(
        [
            rect(t.x, t.y),
            rect(t.x, t.y),
            rect(t.z, t.y),
            rect(t.z, t.y),
            rect(t.x, t.z),
            rect(t.x, t.z),
        ],
        texture,
    )
}

/// A cylinder the camera sits inside, its panorama wrapped once around, with
/// a disc closing the top.
///
/// Every normal is forced to +Y: the scene pass lights the sky like any other
/// surface, and a wall of side-facing normals would put the sun's highlight
/// on one side of the sky and leave the other in ambient. Pointing them all
/// up lights every segment the same. The sampler repeats, so `u` runs 0..1
/// around and `v` 0 (top) to 1 (bottom); the cap samples the top row.
///
/// `u` runs with the ring angle but `x` is negated: in a Y-up right-handed
/// frame a viewer at the centre facing +Z has +X on their LEFT, so wrapping
/// `u` straight onto the angle drew the panorama mirror-reversed, lettering
/// and all. Seen from inside, the picture now reads left to right.
#[must_use]
pub fn sky_cylinder(
    radius: f32,
    y0: f32,
    y1: f32,
    segments: u32,
    texture: Option<TextureData>,
) -> MeshData {
    let segments = segments.max(3);
    let up = [0.0, 1.0, 0.0];
    let mut vertices = Vec::with_capacity(segments as usize * 9);
    // Cast: segment counts are tiny (48), exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let ring = |i: u32| -> (f32, f32, f32) {
        let f = i as f32 / segments as f32;
        let (s, c) = (f * TAU).sin_cos();
        (-radius * s, radius * c, f)
    };
    for i in 0..segments {
        let (x0, z0, u0) = ring(i);
        let (x1, z1, u1) = ring(i + 1);
        let quad = [
            ([x0, y0, z0], [u0, 1.0]),
            ([x1, y0, z1], [u1, 1.0]),
            ([x1, y1, z1], [u1, 0.0]),
            ([x0, y1, z0], [u0, 0.0]),
        ];
        for idx in [0usize, 1, 2, 0, 2, 3] {
            let (pos, uv) = quad[idx];
            vertices.push(MeshVertex {
                pos,
                normal: up,
                uv,
            });
        }
        // Cap fan: centre, then the two rim points at the ceiling.
        vertices.push(MeshVertex {
            pos: [0.0, y1, 0.0],
            normal: up,
            uv: [0.5, 0.0],
        });
        vertices.push(MeshVertex {
            pos: [x0, y1, z0],
            normal: up,
            uv: [u0, 0.0],
        });
        vertices.push(MeshVertex {
            pos: [x1, y1, z1],
            normal: up,
            uv: [u1, 0.0],
        });
    }
    MeshData { vertices, texture }
}

/// Axis-aligned extent of a mesh, in mesh units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    /// The unit cube: what every box mesh here spans, and the stand-in for
    /// an empty mesh so a fit never divides by zero.
    pub const UNIT: Self = Self {
        min: Vec3::splat(-0.5),
        max: Vec3::splat(0.5),
    };

    #[must_use]
    pub fn of(mesh: &MeshData) -> Self {
        if mesh.vertices.is_empty() {
            return Self::UNIT;
        }
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for v in &mesh.vertices {
            let p = Vec3::from(v.pos);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        Self { min: lo, max: hi }
    }

    #[must_use]
    pub fn size(&self) -> Vec3 {
        (self.max - self.min).max(Vec3::splat(1e-3))
    }

    #[must_use]
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}

/// The measured extent of every registered prop, taken from the meshes
/// before they are handed to the engine (which keeps them). A generated
/// prop's size is whatever the generator produced; the fits below scale it
/// to the height the level asks for.
#[derive(Clone, Copy, Debug)]
pub struct PropFits {
    bounds: [Bounds; Prop::COUNT],
}

impl PropFits {
    #[must_use]
    pub const fn bounds(&self, p: Prop) -> Bounds {
        self.bounds[p.index()]
    }
}

/// Measure the meshes `prop_meshes` returned, in the same order.
#[must_use]
pub fn measure(meshes: &[MeshData]) -> PropFits {
    let mut bounds = [Bounds::UNIT; Prop::COUNT];
    for (slot, mesh) in bounds.iter_mut().zip(meshes) {
        *slot = Bounds::of(mesh);
    }
    PropFits { bounds }
}

/// A generated prop, untextured: the largest part of the GLB with faceted
/// normals and box-projected UVs, tiled so that at `nominal_height` in the
/// world one tile is about `TILE_M`.
fn generated(bytes: &[u8], nominal_height: f32, what: &str) -> Result<MeshData, String> {
    let mesh = ember_engine::assets::load_glb(bytes)?
        .into_iter()
        .max_by_key(|p| p.mesh.vertices.len())
        .map(|p| p.mesh)
        .ok_or_else(|| "glb had no parts".to_string())?;
    let mesh = ember_engine::assets::face_normals(mesh);
    let mesh_h = Bounds::of(&mesh).size().y;
    let tiles_per_unit = nominal_height / mesh_h / TILE_M;
    let mesh = ember_engine::assets::planar_uvs(mesh, tiles_per_unit);
    tracing::info!(what, tris = mesh.vertices.len() / 3, "prop loaded");
    Ok(mesh)
}

/// A generated prop wearing its material picture. A GLB that fails to load
/// becomes a box wearing the same picture - drawn at the right height, wrong
/// shape, with a log line saying which - rather than a missing mesh id.
fn prop_or_box(
    bytes: &[u8],
    texture: Option<TextureData>,
    nominal_height: f32,
    what: &str,
) -> MeshData {
    match generated(bytes, nominal_height, what) {
        Ok(mut mesh) => {
            mesh.texture = texture;
            mesh
        }
        Err(e) => {
            tracing::warn!("prop {what} unusable ({e}); box stand-in");
            tiled_box(Vec3::new(1.0, nominal_height, 1.0), texture)
        }
    }
}

/// Build every prop mesh, in `Prop::ALL` order.
#[must_use]
pub fn prop_meshes() -> Vec<MeshData> {
    Prop::ALL.into_iter().map(build).collect()
}

fn build(p: Prop) -> MeshData {
    // Atlas cells, `u` right and `v` down, as `bake_textures.py` lays them
    // out: the container is a 2x2 (side | doors over roof | floor), the
    // crate and the ammo box a 2x1 (side | top).
    const Q_TL: [f32; 4] = [0.0, 0.0, 0.5, 0.5];
    const Q_TR: [f32; 4] = [0.5, 0.0, 1.0, 0.5];
    const Q_BL: [f32; 4] = [0.0, 0.5, 0.5, 1.0];
    const Q_BR: [f32; 4] = [0.5, 0.5, 1.0, 1.0];
    const H_L: [f32; 4] = [0.0, 0.0, 0.5, 1.0];
    const H_R: [f32; 4] = [0.5, 0.0, 1.0, 1.0];
    match p {
        // Long faces (+/-Z) are the side, both ends the doors: a container
        // seen end-on should show doors whichever end faces you.
        Prop::Container => atlas_box(
            [Q_TL, Q_TL, Q_TR, Q_TR, Q_BL, Q_BR],
            tex(TEX_CONTAINER, "container"),
        ),
        Prop::Crate => atlas_box([H_L, H_L, H_L, H_L, H_R, H_R], tex(TEX_CRATE, "crate")),
        Prop::Ammo => atlas_box([H_L, H_L, H_L, H_L, H_R, H_R], tex(TEX_AMMO, "ammo")),
        Prop::TrenchWall => tiled_box(
            Vec3::new(9.8, 2.5, 0.4),
            tex(TEX_TRENCH_WALL, "trench-wall"),
        ),
        Prop::TunnelRoof => tiled_box(
            Vec3::new(12.0, 0.4, 3.0),
            tex(TEX_TUNNEL_ROOF, "tunnel-roof"),
        ),
        Prop::Rubble => tiled_box(Vec3::new(2.0, 0.7, 2.0), tex(TEX_RUBBLE, "rubble")),
        Prop::Plinth => tiled_box(Vec3::new(3.2, 2.2, 3.2), tex(TEX_PLINTH, "plinth")),
        // The boundary wall is 50 m long and 3.5 tall: one balustrade tile
        // per 3.5 m so the picture is not squashed to a strip.
        Prop::CityWall => tiled_box(
            Vec3::new(50.0 * TILE_M / 3.5, 3.5 * TILE_M / 3.5, 0.9),
            tex(TEX_CITY_WALL, "city-wall"),
        ),
        Prop::Floor => MeshData::textured_plane(FLOOR_TILES, tex(TEX_COBBLE, "cobble")),
        Prop::Ground => MeshData::textured_plane(GROUND_TILES, tex(TEX_COBBLE, "cobble")),
        Prop::Sky => sky_cylinder(
            SKY_RADIUS,
            SKY_Y0,
            SKY_Y1,
            SKY_SEGMENTS,
            tex(TEX_SKY, "sky"),
        ),
        Prop::Statue => prop_or_box(GLB_STATUE, tex(TEX_BRONZE, "bronze"), 4.0, "statue"),
        Prop::Cathedral => prop_or_box(
            GLB_CATHEDRAL,
            tex(TEX_LIMESTONE, "limestone"),
            34.0,
            "cathedral",
        ),
        Prop::FacadeA => prop_or_box(
            GLB_FACADE_A,
            tex(TEX_SANDSTONE, "sandstone"),
            18.0,
            "facade-a",
        ),
        Prop::FacadeB => prop_or_box(
            GLB_FACADE_B,
            tex(TEX_LIMESTONE, "limestone"),
            18.0,
            "facade-b",
        ),
        // The sandbag line is fitted to a 1.1-tall box. The burlap picture
        // is the generated mesh's material, and the box stand-in wears the
        // same one: `every_prop_builds_and_lights` opens the embedded GLB at
        // build time, so the stand-in branch cannot be reached by a shipped
        // build, and a picture of its own here would be 480 KB in every web
        // player's bundle that is never drawn.
        Prop::Sandbags => match generated(GLB_SANDBAGS, 1.1, "sandbags") {
            Ok(mut mesh) => {
                mesh.texture = tex(TEX_BURLAP, "burlap");
                mesh
            }
            Err(e) => {
                tracing::warn!("prop sandbags unusable ({e}); burlap box");
                tiled_box(Vec3::new(4.0, 1.1, 0.8), tex(TEX_BURLAP, "burlap"))
            }
        },
        Prop::Wreck => prop_or_box(
            GLB_WRECK,
            tex(TEX_SCORCHED_STEEL, "scorched-steel"),
            1.5,
            "wreck",
        ),
        Prop::Lamp => prop_or_box(GLB_LAMP, tex(TEX_CAST_IRON, "cast-iron"), 5.0, "lamp"),
    }
}

/// Where the props landed in the engine's mesh table, and how big each is.
#[derive(Clone, Copy, Debug)]
pub struct Props {
    pub base: u32,
    pub fits: PropFits,
}

impl Props {
    #[must_use]
    pub const fn mesh(&self, p: Prop) -> u32 {
        self.base + p.offset()
    }

    /// Draw `p` filling the axis-aligned box `size` centred at `center`.
    ///
    /// Every tiled and atlas mesh here is authored with its long faces along
    /// +X, and a generated prop has whichever axis the generator gave it, so
    /// when the box's long footprint axis is the other one the mesh is yawed
    /// a quarter turn and its scale swapped to match. Scale applies before
    /// rotation, so the swapped scale is what makes the rotated mesh land on
    /// the box exactly. Non-uniform scale skews the lighting a little (the
    /// normal matrix is the same one); accepted for a sandbag line.
    pub fn push_fitted(&self, frame: &mut Frame, p: Prop, center: Vec3, size: Vec3, color: Vec3) {
        let b = self.fits.bounds(p);
        let ms = b.size();
        let turn = (size.x >= size.z) != (ms.x >= ms.z);
        let (rot, target) = if turn {
            (
                Quat::from_rotation_y(FRAC_PI_2),
                Vec3::new(size.z, size.y, size.x),
            )
        } else {
            (Quat::IDENTITY, size)
        };
        let scale = target / ms;
        let pos = center - rot * (b.center() * scale);
        frame.instances.push(
            Instance::new(pos, scale, color)
                .with_rot(rot)
                .with_mesh(self.mesh(p)),
        );
    }

    /// Draw one cover box by kind. A raised box (`base > 0`) is drawn from
    /// its base to its top, not from the floor.
    pub fn push_obstacle(&self, frame: &mut Frame, o: &Obstacle) {
        let height = (o.h - o.base).max(0.01);
        let size = Vec3::new(o.max[0] - o.min[0], height, o.max[1] - o.min[1]);
        let center = Vec3::new(
            f32::midpoint(o.min[0], o.max[0]),
            o.base + height * 0.5,
            f32::midpoint(o.min[1], o.max[1]),
        );
        let prop = match o.kind {
            Cover::Container => {
                // A 5.2-tall container is two stacked, not one stretched.
                let layers = (height / CONTAINER_UNIT_H).round().max(1.0);
                let unit = height / layers;
                let mut k = 0.0;
                while k < layers {
                    let c = Vec3::new(center.x, o.base + unit * (k + 0.5), center.z);
                    self.push_fitted(
                        frame,
                        Prop::Container,
                        c,
                        Vec3::new(size.x, unit, size.z),
                        Vec3::ONE,
                    );
                    k += 1.0;
                }
                return;
            }
            Cover::Crate => Prop::Crate,
            Cover::Ammo => Prop::Ammo,
            Cover::Sandbag => Prop::Sandbags,
            Cover::Wall => Prop::TrenchWall,
            Cover::Roof => Prop::TunnelRoof,
            Cover::Rubble => Prop::Rubble,
            Cover::Plinth => Prop::Plinth,
        };
        self.push_fitted(frame, prop, center, size, Vec3::ONE);
    }

    /// Draw one listed decor prop: scaled uniformly so its height is
    /// `Decor.scale`, standing with its feet on `Decor.pos`, turned by
    /// `Decor.yaw` about +Y (0 faces +Z, the convention `Decor` documents).
    pub fn push_decor(&self, frame: &mut Frame, d: &Decor) {
        let prop = match d.kind {
            DecorKind::Statue => Prop::Statue,
            DecorKind::Cathedral => Prop::Cathedral,
            DecorKind::FacadeA => Prop::FacadeA,
            DecorKind::FacadeB => Prop::FacadeB,
            DecorKind::Lamp => Prop::Lamp,
            DecorKind::Wreck => Prop::Wreck,
        };
        let b = self.fits.bounds(prop);
        let s = d.scale / b.size().y;
        let rot = Quat::from_rotation_y(d.yaw);
        // The mesh's footprint centre at its lowest point is what stands on
        // `pos`; it is scaled and turned with the mesh before being taken
        // off the position.
        let foot = Vec3::new(b.center().x, b.min.y, b.center().z) * s;
        let pos = Vec3::from(d.pos) - rot * foot;
        frame.instances.push(
            Instance::new(pos, Vec3::splat(s), Vec3::ONE)
                .with_rot(rot)
                .with_mesh(self.mesh(prop)),
        );
    }

    /// The sky cylinder and the far ground plane.
    pub fn push_sky_and_ground(&self, frame: &mut Frame) {
        frame.instances.push(
            Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::splat(SKY_DRIVE))
                .with_mesh(self.mesh(Prop::Sky)),
        );
        frame.instances.push(
            Instance::new(
                Vec3::new(0.0, GROUND_Y, 0.0),
                Vec3::new(GROUND_SIZE, 1.0, GROUND_SIZE),
                Vec3::splat(GROUND_DIM),
            )
            .with_mesh(self.mesh(Prop::Ground)),
        );
    }
}

#[cfg(test)]
// Exact float equality is the point of the face-for-face test: an atlas box
// must reproduce `textured_box`'s vertices bit for bit, not approximately.
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use pong_core::shooter::Level;

    /// Where an instance puts a mesh-space point, exactly as the renderer
    /// does it: scale, then rotate, then translate.
    fn world(inst: &Instance, p: Vec3) -> Vec3 {
        inst.rot * (p * inst.scale) + inst.position
    }

    fn corners(b: Bounds) -> [Vec3; 8] {
        let (lo, hi) = (b.min, b.max);
        [
            Vec3::new(lo.x, lo.y, lo.z),
            Vec3::new(hi.x, lo.y, lo.z),
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::new(hi.x, hi.y, lo.z),
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::new(lo.x, hi.y, hi.z),
            Vec3::new(hi.x, hi.y, hi.z),
        ]
    }

    fn world_bounds(inst: &Instance, b: Bounds) -> Bounds {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for c in corners(b) {
            let w = world(inst, c);
            lo = lo.min(w);
            hi = hi.max(w);
        }
        Bounds { min: lo, max: hi }
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).abs().max_element() < 1e-3
    }

    /// The whole point of copying `CUBE_FACES`: an atlas box with every face
    /// at the full picture IS `textured_box(1.0)`, vertex for vertex.
    #[test]
    fn atlas_box_matches_textured_box_face_for_face() {
        let full = [[0.0, 0.0, 1.0, 1.0]; 6];
        let a = atlas_box(full, None);
        let t = MeshData::textured_box(1.0, None);
        assert_eq!(a.vertices.len(), t.vertices.len());
        for (i, (x, y)) in a.vertices.iter().zip(&t.vertices).enumerate() {
            assert_eq!(x.pos, y.pos, "vertex {i} position");
            assert_eq!(x.normal, y.normal, "vertex {i} normal");
            assert_eq!(x.uv, y.uv, "vertex {i} uv");
        }
    }

    /// Faces are addressed in `+Z, -Z, +X, -X, +Y, -Y` order and each gets
    /// its own rectangle, with the picture upright on the side faces (v0 on
    /// the top edge).
    #[test]
    fn atlas_box_addresses_faces_in_cube_order() {
        let mut faces = [[0.0f32; 4]; 6];
        for (k, f) in [0.0f32, 0.1, 0.2, 0.3, 0.4, 0.5]
            .into_iter()
            .zip(&mut faces)
        {
            *f = [k, 0.2, k + 0.1, 0.4];
        }
        let m = atlas_box(faces, None);
        let normals = [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
        ];
        for (i, chunk) in m.vertices.chunks(6).enumerate() {
            for v in chunk {
                assert_eq!(v.normal, normals[i], "face {i}");
                assert!(v.uv[0] >= faces[i][0] - 1e-6 && v.uv[0] <= faces[i][2] + 1e-6);
                assert!(v.uv[1] >= faces[i][1] - 1e-6 && v.uv[1] <= faces[i][3] + 1e-6);
            }
        }
        // Side face +Z: the top corners carry v0, the bottom corners v1.
        for v in &m.vertices[..6] {
            let top = v.pos[1] > 0.0;
            assert_eq!(v.uv[1], if top { 0.2 } else { 0.4 });
        }
    }

    #[test]
    fn sky_cylinder_faces_up_everywhere_and_is_closed_on_top() {
        let m = sky_cylinder(60.0, -5.0, 70.0, 48, None);
        assert_eq!(m.vertices.len(), 48 * 9);
        let mut has_cap_centre = false;
        for v in &m.vertices {
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
            let r = v.pos[0].hypot(v.pos[2]);
            if r < 1e-3 {
                has_cap_centre = true;
                assert!((v.pos[1] - 70.0).abs() < 1e-4);
            } else {
                assert!((r - 60.0).abs() < 1e-2, "rim radius {r}");
            }
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
        }
        assert!(has_cap_centre);
        // Read from inside, not mirrored: a viewer facing +Z has +X on their
        // left, so `u` must grow toward -X. The first quad's second vertex
        // is the first rim point past u = 0.
        let v = &m.vertices[1];
        assert!(
            v.uv[0] > 0.0 && v.pos[0] < 0.0,
            "u {} at x {}: the panorama would read right to left",
            v.uv[0],
            v.pos[0]
        );
    }

    /// The generated meshes are opened at build time, so a broken GLB fails
    /// here, not in a player's browser; and every prop must light.
    #[test]
    fn every_prop_builds_and_lights() {
        let meshes = prop_meshes();
        assert_eq!(meshes.len(), Prop::COUNT);
        for (p, m) in Prop::ALL.into_iter().zip(&meshes) {
            assert!(!m.vertices.is_empty(), "{p:?}: no vertices");
            assert_eq!(m.vertices.len() % 3, 0, "{p:?}: not a triangle list");
            for v in &m.vertices {
                let n = Vec3::from(v.normal);
                assert!(n.is_finite(), "{p:?}: non-finite normal");
                assert!((n.length() - 1.0).abs() < 1e-3, "{p:?}: normal {n}");
            }
        }
        let fits = measure(&meshes);
        for p in Prop::ALL {
            let s = fits.bounds(p).size();
            assert!(s.min_element() > 0.0, "{p:?}: flat bounds {s}");
        }
    }

    fn props() -> Props {
        let meshes = prop_meshes();
        Props {
            base: 7,
            fits: measure(&meshes),
        }
    }

    /// A box longer in z than x is drawn turned, and still lands exactly on
    /// its footprint; one longer in x is not turned.
    #[test]
    fn fitted_boxes_land_on_their_footprint_either_way_round() {
        let props = props();
        for size in [Vec3::new(9.8, 2.5, 0.4), Vec3::new(0.4, 2.5, 9.8)] {
            let mut frame = Frame::default();
            let center = Vec3::new(3.0, 1.25, -7.0);
            props.push_fitted(&mut frame, Prop::TrenchWall, center, size, Vec3::ONE);
            let inst = frame.instances[0];
            assert_eq!(inst.mesh, 7 + Prop::TrenchWall as u32);
            let wb = world_bounds(&inst, props.fits.bounds(Prop::TrenchWall));
            assert!(close(wb.min, center - size * 0.5), "{size}: min {}", wb.min);
            assert!(close(wb.max, center + size * 0.5), "{size}: max {}", wb.max);
        }
    }

    /// A raised box is drawn between its base and its top, and a stacked
    /// container is two units, not one stretched.
    #[test]
    fn obstacles_draw_from_base_to_top() {
        let props = props();
        let roof = Obstacle::boxed(Cover::Roof, [-6.0, 11.0], [6.0, 14.0], 2.5, 2.9);
        let mut frame = Frame::default();
        props.push_obstacle(&mut frame, &roof);
        assert_eq!(frame.instances.len(), 1);
        let wb = world_bounds(&frame.instances[0], props.fits.bounds(Prop::TunnelRoof));
        assert!(close(wb.min, Vec3::new(-6.0, 2.5, 11.0)), "{}", wb.min);
        assert!(close(wb.max, Vec3::new(6.0, 2.9, 14.0)), "{}", wb.max);

        let stack = Obstacle::boxed(Cover::Container, [17.0, 20.4], [23.0, 22.8], 0.0, 5.2);
        let mut frame = Frame::default();
        props.push_obstacle(&mut frame, &stack);
        assert_eq!(frame.instances.len(), 2);
        let b = props.fits.bounds(Prop::Container);
        let lower = world_bounds(&frame.instances[0], b);
        let upper = world_bounds(&frame.instances[1], b);
        assert!(close(lower.min, Vec3::new(17.0, 0.0, 20.4)));
        assert!(close(lower.max, Vec3::new(23.0, 2.6, 22.8)));
        assert!(close(upper.min, Vec3::new(17.0, 2.6, 20.4)));
        assert!(close(upper.max, Vec3::new(23.0, 5.2, 22.8)));
    }

    /// Every sandbag box in the trench city is filled exactly by the sandbag
    /// mesh, whichever way it lies.
    #[test]
    fn sandbags_fill_their_boxes() {
        let props = props();
        let level = Level::trench_city();
        let b = props.fits.bounds(Prop::Sandbags);
        for o in level.obstacles.iter().filter(|o| o.kind == Cover::Sandbag) {
            let mut frame = Frame::default();
            props.push_obstacle(&mut frame, o);
            let wb = world_bounds(&frame.instances[0], b);
            assert!(
                close(wb.min, Vec3::new(o.min[0], 0.0, o.min[1])),
                "{o:?}: {}",
                wb.min
            );
            assert!(
                close(wb.max, Vec3::new(o.max[0], o.h, o.max[1])),
                "{o:?}: {}",
                wb.max
            );
        }
    }

    /// Decor stands on its point at the height the level asks for, turned by
    /// its yaw, with its footprint centred on the point.
    #[test]
    fn decor_stands_on_its_point_at_its_height() {
        let props = props();
        for d in Level::trench_city().decor {
            let mut frame = Frame::default();
            props.push_decor(&mut frame, &d);
            let inst = frame.instances[0];
            let prop = match d.kind {
                DecorKind::Statue => Prop::Statue,
                DecorKind::Cathedral => Prop::Cathedral,
                DecorKind::FacadeA => Prop::FacadeA,
                DecorKind::FacadeB => Prop::FacadeB,
                DecorKind::Lamp => Prop::Lamp,
                DecorKind::Wreck => Prop::Wreck,
            };
            let wb = world_bounds(&inst, props.fits.bounds(prop));
            assert!(
                (wb.min.y - d.pos[1]).abs() < 1e-3,
                "{d:?}: feet at {}",
                wb.min.y
            );
            assert!(
                (wb.max.y - wb.min.y - d.scale).abs() < 1e-3,
                "{d:?}: height {}",
                wb.max.y - wb.min.y
            );
            let c = (wb.min + wb.max) * 0.5;
            assert!(
                (c.x - d.pos[0]).abs() < 1e-2 && (c.z - d.pos[2]).abs() < 1e-2,
                "{d:?}: centre {c}"
            );
            let facing = inst.rot * Vec3::Z;
            assert!(
                close(facing, Vec3::new(d.yaw.sin(), 0.0, d.yaw.cos())),
                "{d:?}: facing {facing}"
            );
        }
    }
}
