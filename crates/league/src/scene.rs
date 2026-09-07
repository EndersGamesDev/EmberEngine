//! The world as ember draws it: procedural meshes registered in a fixed
//! order, and a frame builder that reads snapshot units, buffs, shots and
//! transient effects. The scene pass has one base colour per instance and
//! no text, so identity is silhouette, colour and motion; HUD text is the
//! page's job.
//!
//! Camera: a fixed-pitch perspective looking straight along -Z, so the lane
//! runs left-right on screen (blue core left, red core right) and a
//! right-click lands where the eye says it will. `project` and
//! `ground_point` use the same basis, so picking and moving agree with the
//! picture by construction.
//!
//! Facing: the sim's yaw is `forward = (cos f, sin f)` in (x, z). Every
//! mesh here is authored with its front along local +X, and [`face`] turns
//! a sim yaw into the quaternion that puts local +X on that forward.
//!
//! Mesh ids are allocated here in order and mirrored by the `MESH_*`
//! constants: 0 is the engine cube, then the list in [`build_meshes`].
//! Adding a mesh at the end is safe; inserting in the middle shifts all.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    clippy::imprecise_flops,
    clippy::many_single_char_names,
    clippy::too_many_lines
)]

use std::f32::consts::{PI, TAU};

use ember_engine::glam::{Quat, Vec2, Vec3};
use ember_engine::{
    Camera, Environment, Fog, Frame, Instance, MeshData, MeshVertex, Particle, Weather,
};
use league_core::data;
use league_core::proto::{BuffSnap, ProjSnap};

use crate::world::{FxLite, UnitLite, ZoneLite};

/// Baked champion GLBs from the fleet, registered after the procedural set.
pub mod art;

pub const MESH_PLANE: u32 = 1;
/// A capped cylinder, radius 1, from y=-1 to y=1.
pub const MESH_FRUSTUM: u32 = 2;
pub const MESH_OCTA: u32 = 3;
/// A thin disc, radius 1, at y=0.
pub const MESH_DISC: u32 = 4;
/// A cone, base radius 1 at y=-1, apex at y=1.
pub const MESH_CONE: u32 = 5;
/// A fine inset annulus, outer radius 1, inner 0.96, at y=0.
pub const MESH_RING: u32 = 6;
/// A tapered blade from x=0 to x=1, thin in y, narrow in z.
pub const MESH_BLADE: u32 = 7;
/// A toothed wheel, radius 1, axis Y, thickness 0.3.
pub const MESH_GEAR: u32 = 8;
/// A low-poly sphere, radius 1.
pub const MESH_SPHERE: u32 = 9;
/// Textured ground from `art::surfaces`, unit-sized in x/z at y=0.
/// Garden/lane UVs tile over their field; the court is converted to a
/// unit-diameter disc with one complete emblem, shared by both plazas.
pub const MESH_GARDEN: u32 = 10;
pub const MESH_LANE: u32 = 11;
pub const MESH_COURT: u32 = 12;
/// Baked arena props from `art::props`, origin on the ground, +X forward:
/// the three-spire obelisk (5 tall), the ruined arch (6.5 tall), the jade
/// canopy tree (7 tall).
pub const MESH_OBELISK: u32 = 13;
pub const MESH_ARCH: u32 = 14;
pub const MESH_TREE: u32 = 15;

/// Camera height over the focus, its offset toward +Z and the vertical
/// field of view. The pitch these give (56 degrees, the MOBA norm) is what
/// [`bar_tilt`] faces; a steeper camera flattened every body to its
/// footprint in the first capture.
pub const CAM_HEIGHT: f32 = 24.0;
pub const CAM_BACK: f32 = 16.0;
pub const CAM_FOV: f32 = 40.0;

/// The camera sits high and behind the focus, looking along -Z.
#[must_use]
pub const fn camera_for(focus: (f32, f32)) -> Camera {
    camera_zoomed(focus, 1.0)
}

/// The same camera pulled in by `zoom` (0.3 is a close-up of a body, 1.0
/// the play view); the pitch is unchanged, so [`bar_tilt`] still holds.
#[must_use]
pub const fn camera_zoomed(focus: (f32, f32), zoom: f32) -> Camera {
    let (x, z) = focus;
    Camera {
        eye: Vec3::new(x, CAM_HEIGHT * zoom, z + CAM_BACK * zoom),
        target: Vec3::new(x, 0.0, z),
        fov_y_deg: CAM_FOV,
    }
}

/// Team tint: blue side, red side. Deeper than the page's HUD tints
/// (#6ea0ff / #ff7a70) on purpose: the scene pass lifts every colour by
/// its ambient term and tonemap, and the first captures came back lavender
/// and pink where the eye needs blue and red.
#[must_use]
pub const fn team_colour(team: u8) -> [f32; 3] {
    match team {
        0 => [0.2, 0.42, 1.0],
        1 => [1.0, 0.22, 0.16],
        _ => [0.75, 0.68, 0.45], // courts belong to nobody
    }
}

/// The quaternion that puts a mesh's local +X on the sim's forward
/// `(cos yaw, sin yaw)`. `Quat::from_rotation_y(t)` sends +X to
/// `(cos t, 0, -sin t)`, hence the sign.
#[must_use]
pub fn face(yaw: f32) -> Quat {
    Quat::from_rotation_y(-yaw)
}

/// The tilt that turns a box thin in Z into a card facing the camera.
#[must_use]
pub fn bar_tilt() -> Quat {
    Quat::from_rotation_x(-CAM_HEIGHT.atan2(CAM_BACK))
}

/// Warm late-day sun against a subdued jade sky. Fog must not lift the
/// nearby dark garden into gray. Time drives clouds and decorative motes
/// only; this per-game palette never changes the shared renderer or sim.
#[must_use]
pub fn garden_light(time: f32) -> Environment {
    let mut env = Environment::outdoor(Weather::Clear, time);
    env.sun_direction = Vec3::new(-0.48, 0.76, -0.42).normalize();
    // Warm, but not so amber that jade moss turns olive: the ground palette
    // is carried by the baked pictures, the sun only has to keep it green.
    env.sun_color = Vec3::new(1.0, 0.90, 0.76);
    env.sun_intensity = 0.96;
    env.sky_zenith = Vec3::new(0.065, 0.14, 0.16);
    env.sky_horizon = Vec3::new(0.36, 0.46, 0.40);
    env.cloud_coverage = 0.20;
    env.wetness = 0.08;
    env.shadow_extent = 34.0;
    env
}

// ---------------------------------------------------------------------------
// meshes
// ---------------------------------------------------------------------------

const SEG: usize = 16;

fn push_tri(v: &mut Vec<MeshVertex>, a: [f32; 3], b: [f32; 3], c: [f32; 3], n: [f32; 3]) {
    for p in [a, b, c] {
        v.push(MeshVertex {
            pos: p,
            normal: n,
            uv: [0.0, 0.0],
        });
    }
}

/// A triangle with the normal computed from its winding (counter-clockwise
/// seen from outside).
fn push_flat(v: &mut Vec<MeshVertex>, a: [f32; 3], b: [f32; 3], c: [f32; 3]) {
    let ab = Vec3::from(b) - Vec3::from(a);
    let ac = Vec3::from(c) - Vec3::from(a);
    let n = ab.cross(ac).normalize_or_zero();
    let n = if n.length_squared() < 0.5 { [0.0, 1.0, 0.0] } else { n.to_array() };
    push_tri(v, a, b, c, n);
}

fn around(cx: f32, cz: f32, r: f32, y: f32, i: usize, seg: usize) -> [f32; 3] {
    let t = TAU * (i as f32) / (seg as f32);
    [cx + t.cos() * r, y, cz + t.sin() * r]
}

fn norm_or_up(n: [f32; 3]) -> [f32; 3] {
    let d = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if d < 1e-6 {
        [0.0, 1.0, 0.0]
    } else {
        [n[0] / d, n[1] / d, n[2] / d]
    }
}

/// The outward normal of a rim segment between two points on a circle
/// around the y axis: the direction of the segment's midpoint.
fn rim_normal(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    norm_or_up([f32::midpoint(a[0], b[0]), 0.0, f32::midpoint(a[2], b[2])])
}

/// The whole mesh set, in registration order: the nine procedural meshes,
/// the three ground surfaces (`art::surfaces`, ids [`MESH_GARDEN`]..), then
/// every baked champion part (`art::meshes`). `lib.rs` registers exactly
/// this list.
#[must_use]
pub fn build_meshes() -> Vec<MeshData> {
    let mut meshes = vec![
        plane_mesh(),
        frustum_mesh(),
        octa_mesh(),
        disc_mesh(),
        cone_mesh(),
        ring_mesh(),
        blade_mesh(),
        gear_mesh(),
        sphere_mesh(),
    ];
    let mut surfaces = art::surfaces();
    // A unique radial image must never become four mirrored motifs or a
    // square decal. Keep its texture and registration id; only clip the quad.
    surfaces[(MESH_COURT - MESH_GARDEN) as usize].vertices = medallion_vertices();
    meshes.extend(surfaces);
    meshes.extend(art::props());
    // the fixed ids above must agree with what the two tables return
    assert_eq!(
        meshes.len() as u32,
        MESH_SPHERE + art::SURFACE_COUNT + art::PROP_COUNT
    );
    assert_eq!(meshes.len() as u32, MESH_TREE);
    meshes.extend(art::meshes(MESH_TREE + 1));
    meshes
}

fn plane_mesh() -> MeshData {
    // a unit quad on y=0, facing +Y, centred on origin
    let n = [0.0, 1.0, 0.0];
    let a = [-0.5, 0.0, -0.5];
    let b = [0.5, 0.0, -0.5];
    let c = [0.5, 0.0, 0.5];
    let d = [-0.5, 0.0, 0.5];
    let mut vertices = Vec::with_capacity(6);
    push_tri(&mut vertices, a, c, b, n);
    push_tri(&mut vertices, a, d, c, n);
    MeshData { vertices, texture: None }
}

fn medallion_vertices() -> Vec<MeshVertex> {
    const SIDES: usize = 48;
    let vertex = |x: f32, z: f32| MeshVertex {
        pos: [x, 0.0, z],
        normal: [0.0, 1.0, 0.0],
        // The authored circle occupies 94% of the square; exclude its gray
        // corner margin without repeating or mirroring the compass motif.
        uv: [0.5 + x * 0.94, 0.5 - z * 0.94],
    };
    let mut vertices = Vec::with_capacity(SIDES * 3);
    for i in 0..SIDES {
        let a = TAU * i as f32 / SIDES as f32;
        let b = TAU * (i + 1) as f32 / SIDES as f32;
        vertices.extend([
            vertex(0.0, 0.0),
            vertex(b.cos() * 0.5, b.sin() * 0.5),
            vertex(a.cos() * 0.5, a.sin() * 0.5),
        ]);
    }
    vertices
}

/// A capped cylinder centred on the origin, radius 1, height 2.
fn frustum_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(SEG * 12);
    for i in 0..SEG {
        let b0 = around(0.0, 0.0, 1.0, -1.0, i, SEG);
        let b1 = around(0.0, 0.0, 1.0, -1.0, i + 1, SEG);
        let t0 = around(0.0, 0.0, 1.0, 1.0, i, SEG);
        let t1 = around(0.0, 0.0, 1.0, 1.0, i + 1, SEG);
        let n = rim_normal(b0, b1);
        push_tri(&mut vertices, b0, b1, t1, n);
        push_tri(&mut vertices, b0, t1, t0, n);
        push_tri(&mut vertices, [0.0, 1.0, 0.0], t0, t1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, [0.0, -1.0, 0.0], b1, b0, [0.0, -1.0, 0.0]);
    }
    MeshData { vertices, texture: None }
}

/// A diamond: two square-based pyramids, point up and down.
fn octa_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(24);
    let pts = [
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let up = [0.0, 1.2, 0.0];
    let dn = [0.0, -1.2, 0.0];
    for (i, a) in pts.into_iter().enumerate() {
        let b = pts[(i + 1) % 4];
        push_flat(&mut vertices, a, up, b);
        push_flat(&mut vertices, b, dn, a);
    }
    MeshData { vertices, texture: None }
}

/// A flat disc: a thin cylinder at y=0 for team rings, pads and zones.
fn disc_mesh() -> MeshData {
    const SIDES: usize = 48;
    let mut vertices = Vec::with_capacity(SIDES * 12);
    let h = 0.04;
    for i in 0..SIDES {
        let a0 = around(0.0, 0.0, 1.0, -h, i, SIDES);
        let a1 = around(0.0, 0.0, 1.0, -h, i + 1, SIDES);
        let b0 = around(0.0, 0.0, 1.0, h, i, SIDES);
        let b1 = around(0.0, 0.0, 1.0, h, i + 1, SIDES);
        let n = rim_normal(a0, a1);
        push_tri(&mut vertices, a0, a1, b1, n);
        push_tri(&mut vertices, a0, b1, b0, n);
        push_tri(&mut vertices, [0.0, h, 0.0], b0, b1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, [0.0, -h, 0.0], a1, a0, [0.0, -1.0, 0.0]);
    }
    MeshData {
        vertices,
        texture: None,
    }
}

/// A cone: base radius 1 at y=-1, apex at y=1.
fn cone_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(SEG * 6);
    let apex = [0.0, 1.0, 0.0];
    for i in 0..SEG {
        let b0 = around(0.0, 0.0, 1.0, -1.0, i, SEG);
        let b1 = around(0.0, 0.0, 1.0, -1.0, i + 1, SEG);
        push_flat(&mut vertices, b0, apex, b1);
        push_tri(&mut vertices, [0.0, -1.0, 0.0], b1, b0, [0.0, -1.0, 0.0]);
    }
    MeshData { vertices, texture: None }
}

/// Fine inlaid linework, never a broad opaque team-coloured plate.
fn ring_mesh() -> MeshData {
    const SIDES: usize = 48;
    let mut vertices = Vec::with_capacity(SIDES * 24);
    let (ro, ri, h) = (1.0, 0.96, 0.012);
    for i in 0..SIDES {
        let o0 = around(0.0, 0.0, ro, h, i, SIDES);
        let o1 = around(0.0, 0.0, ro, h, i + 1, SIDES);
        let i0 = around(0.0, 0.0, ri, h, i, SIDES);
        let i1 = around(0.0, 0.0, ri, h, i + 1, SIDES);
        let ob0 = around(0.0, 0.0, ro, -h, i, SIDES);
        let ob1 = around(0.0, 0.0, ro, -h, i + 1, SIDES);
        let ib0 = around(0.0, 0.0, ri, -h, i, SIDES);
        let ib1 = around(0.0, 0.0, ri, -h, i + 1, SIDES);
        // top and bottom faces
        push_tri(&mut vertices, i0, o0, o1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, i0, o1, i1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, ib0, ob1, ob0, [0.0, -1.0, 0.0]);
        push_tri(&mut vertices, ib0, ib1, ob1, [0.0, -1.0, 0.0]);
        // outer and inner walls
        let n = rim_normal(o0, o1);
        push_tri(&mut vertices, ob0, ob1, o1, n);
        push_tri(&mut vertices, ob0, o1, o0, n);
        let m = [-n[0], 0.0, -n[2]];
        push_tri(&mut vertices, ib0, i1, ib1, m);
        push_tri(&mut vertices, ib0, i0, i1, m);
    }
    MeshData {
        vertices,
        texture: None,
    }
}

/// A blade along +X: a flat lozenge from the hilt at x=0 to the point at
/// x=1, half a unit wide at the guard, with a ridge so it catches light.
fn blade_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(24);
    let hilt = [0.0, 0.0, 0.0];
    let tip = [1.0, 0.0, 0.0];
    let l = [0.18, 0.0, -0.5];
    let r = [0.18, 0.0, 0.5];
    let up = [0.22, 0.5, 0.0];
    let dn = [0.22, -0.5, 0.0];
    // four faces of a double pyramid stretched to the tip
    push_flat(&mut vertices, hilt, l, up);
    push_flat(&mut vertices, l, tip, up);
    push_flat(&mut vertices, up, tip, r);
    push_flat(&mut vertices, hilt, up, r);
    push_flat(&mut vertices, hilt, dn, l);
    push_flat(&mut vertices, l, dn, tip);
    push_flat(&mut vertices, dn, r, tip);
    push_flat(&mut vertices, hilt, r, dn);
    MeshData { vertices, texture: None }
}

/// A toothed wheel: a 12-tooth gear, axis Y, radius 1 at the tooth tips,
/// 0.8 at the root, 0.3 thick, with a hub hole left solid for simplicity.
fn gear_mesh() -> MeshData {
    const TEETH: usize = 12;
    const STEPS: usize = TEETH * 4;
    let mut vertices = Vec::with_capacity(STEPS * 12);
    let h = 0.15;
    let radius_at = |i: usize| -> f32 {
        // two of every four steps sit on the tooth tip, two at the root
        if (i / 2).is_multiple_of(2) {
            1.0
        } else {
            0.8
        }
    };
    for i in 0..STEPS {
        let r0 = radius_at(i);
        let r1 = radius_at(i + 1);
        let t0 = around(0.0, 0.0, r0, h, i, STEPS);
        let t1 = around(0.0, 0.0, r1, h, i + 1, STEPS);
        let b0 = around(0.0, 0.0, r0, -h, i, STEPS);
        let b1 = around(0.0, 0.0, r1, -h, i + 1, STEPS);
        push_tri(&mut vertices, [0.0, h, 0.0], t0, t1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, [0.0, -h, 0.0], b1, b0, [0.0, -1.0, 0.0]);
        push_flat(&mut vertices, b0, b1, t1);
        push_flat(&mut vertices, b0, t1, t0);
    }
    MeshData { vertices, texture: None }
}

/// A low-poly sphere: an octahedron subdivided twice and normalised.
fn sphere_mesh() -> MeshData {
    fn subdivide(tris: Vec<[Vec3; 3]>) -> Vec<[Vec3; 3]> {
        let mut out = Vec::with_capacity(tris.len() * 4);
        for [a, b, c] in tris {
            let ab = ((a + b) * 0.5).normalize();
            let bc = ((b + c) * 0.5).normalize();
            let ca = ((c + a) * 0.5).normalize();
            out.push([a, ab, ca]);
            out.push([ab, b, bc]);
            out.push([ca, bc, c]);
            out.push([ab, bc, ca]);
        }
        out
    }
    let (px, nx) = (Vec3::X, -Vec3::X);
    let (py, ny) = (Vec3::Y, -Vec3::Y);
    let (pz, nz) = (Vec3::Z, -Vec3::Z);
    let mut tris = vec![
        [px, py, pz],
        [pz, py, nx],
        [nx, py, nz],
        [nz, py, px],
        [px, pz, ny],
        [pz, nx, ny],
        [nx, nz, ny],
        [nz, px, ny],
    ];
    tris = subdivide(subdivide(tris));
    let mut vertices = Vec::with_capacity(tris.len() * 3);
    for [a, b, c] in tris {
        // smooth normals: on a unit sphere the normal is the position
        for p in [a, b, c] {
            vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: p.to_array(),
                uv: [0.0, 0.0],
            });
        }
    }
    MeshData { vertices, texture: None }
}

// ---------------------------------------------------------------------------
// instance helpers
// ---------------------------------------------------------------------------

fn ins(position: Vec3, scale: Vec3, colour: [f32; 3]) -> Instance {
    Instance::new(position, scale, Vec3::from(colour))
}

const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

const fn scale3(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Mix a colour toward another by `t` in 0..1.
const fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// A point `fwd` ahead and `side` to the right of a unit facing `yaw`.
fn ahead(x: f32, z: f32, yaw: f32, fwd: f32, side: f32) -> (f32, f32) {
    let (c, s) = (yaw.cos(), yaw.sin());
    // right of forward (cos, sin) in the x/z plane is (sin, -cos)
    (x + c * fwd + s * side, z + s * fwd - c * side)
}

// ---------------------------------------------------------------------------
// the field
// ---------------------------------------------------------------------------

/// Static scenery is vertically culled with a generous shadow margin. The
/// camera has no aspect here, so horizontal culling would break wide screens.
/// This never removes ground, gameplay units or their target markers.
fn scenery_visible(frame: &Frame, x: f32, z: f32, radius: f32) -> bool {
    let forward = (frame.camera.target - frame.camera.eye).normalize_or_zero();
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(forward);
    let relative = v3(x, 2.0, z) - frame.camera.eye;
    let depth = relative.dot(forward);
    let margin = radius + 8.0;
    depth + margin > 0.0
        && relative.dot(up).abs()
            < depth.max(0.0) * (frame.camera.fov_y_deg.to_radians() * 0.5).tan() + margin
}

fn stone_box(frame: &mut Frame, position: Vec3, scale: Vec3, color: [f32; 3], yaw: f32) {
    frame.instances.push(
        ins(position, scale, color)
            .with_yaw(yaw)
            .with_surface(0.88, 0.0),
    );
}

fn inset_ring(frame: &mut Frame, x: f32, y: f32, z: f32, radius: f32, color: [f32; 3]) {
    frame.instances.push(
        ins(v3(x, y, z), v3(radius, 1.0, radius), color)
            .with_mesh(MESH_RING)
            .with_surface(0.7, 0.12)
            .without_shadow(),
    );
}

/// The stone rim is shallow enough to walk across; the emblem stays at the
/// shared y=0 walking surface. Only the objective itself rises above it.
fn push_plaza(frame: &mut Frame, x: f32, z: f32, diameter: f32) {
    let radius = diameter * 0.5;
    frame.instances.push(
        ins(
            v3(x, -0.045, z),
            v3(radius + 0.22, 1.6, radius + 0.22),
            [0.11, 0.145, 0.13],
        )
        .with_mesh(MESH_DISC)
        .with_surface(0.94, 0.0),
    );
    frame.instances.push(
        ins(
            v3(x, 0.025, z),
            v3(diameter, 1.0, diameter),
            [1.0, 1.0, 1.0],
        )
        .with_mesh(MESH_COURT)
        .with_surface(0.86, 0.0),
    );
    inset_ring(frame, x, 0.043, z, radius + 0.08, [0.22, 0.15, 0.065]);
    for i in 0..12 {
        let angle = TAU * i as f32 / 12.0;
        let r = radius - 0.33;
        stone_box(
            frame,
            v3(x + angle.cos() * r, 0.046, z + angle.sin() * r),
            v3(0.30, 0.025, 0.08),
            [0.34, 0.28, 0.14],
            -angle,
        );
    }
}

/// Ground keeps the full authoritative walking rectangle. Depth comes from
/// the island below it, edge masonry and low inlaid detail, not fake holes
/// or tall decorative obstacles that champions can walk through.
fn push_ground(frame: &mut Frame) {
    // Water and the island's weathered cut faces are below all walkable land.
    frame.instances.push(
        ins(
            v3(0.0, -4.25, 0.0),
            v3(232.0, 1.0, 160.0),
            [0.018, 0.080, 0.073],
        )
        .with_mesh(MESH_PLANE)
        .with_surface(0.20, 0.0)
        .with_wetness(),
    );
    stone_box(
        frame,
        v3(0.0, -2.1, 0.0),
        v3(160.0, 4.05, 92.0),
        [0.065, 0.085, 0.072],
        0.0,
    );
    frame.instances.push(
        ins(v3(0.0, -0.05, 0.0), v3(160.0, 1.0, 92.0), [1.0, 1.0, 1.0])
            .with_mesh(MESH_GARDEN)
            .with_surface(0.96, 0.0),
    );
    frame.instances.push(
        ins(
            v3(0.0, -0.02, 0.0),
            v3(2.0 * data::CORE_X + 16.0, 1.0, 2.0 * data::LANE_Z),
            [1.0, 1.0, 1.0],
        )
        .with_mesh(MESH_LANE)
        .with_surface(0.90, 0.0),
    );
    // Recessed-looking gutters and broad limestone coping replace ruler-thin
    // yellow lane paint. Their tops remain within 4 cm of the walking plane.
    for side in [-1.0, 1.0] {
        stone_box(
            frame,
            v3(0.0, -0.008, side * (data::LANE_Z + 0.13)),
            v3(138.0, 0.035, 0.24),
            [0.045, 0.067, 0.054],
            0.0,
        );
        stone_box(
            frame,
            v3(0.0, -0.005, side * (data::LANE_Z + 0.46)),
            v3(138.0, 0.060, 0.42),
            [0.24, 0.255, 0.20],
            0.0,
        );
        for i in -8i8..=8 {
            let x = f32::from(i) * 8.0;
            if x.abs() < 4.0 {
                continue;
            } // the Court approaches stay open
            stone_box(
                frame,
                v3(x, 0.027, side * (data::LANE_Z + 0.48)),
                v3(0.13, 0.025, 0.51),
                [0.13, 0.115, 0.07],
                0.0,
            );
            stone_box(
                frame,
                v3(x + 0.25, 0.015, side * 8.2),
                v3(1.5, 0.040, 0.68),
                [0.18, 0.205, 0.15],
                0.04 * side,
            );
        }
    }
    // An understated central crossing connects the two Courts. Individual
    // slabs give it human scale instead of one dark rectangular corridor.
    for side in [-1.0, 1.0] {
        for row in 0..4 {
            let z = side * (data::LANE_Z + 0.45 + row as f32 * 0.86);
            stone_box(
                frame,
                v3(0.0, 0.015, z),
                v3(2.8, 0.04, 0.74),
                [0.25, 0.255, 0.20],
                0.0,
            );
        }
        for x in [-1.7, 1.7] {
            stone_box(
                frame,
                v3(x, 0.017, side * 8.8),
                v3(0.14, 0.045, 3.6),
                [0.15, 0.17, 0.13],
                0.0,
            );
        }
    }
    for team in 0..2u8 {
        let cx = if team == 0 {
            -data::CORE_X
        } else {
            data::CORE_X
        };
        push_plaza(frame, cx, 0.0, 13.6);
        // The outer line describes the actual healing/shop radius, not the
        // decorative plaza radius. No second broad filled team plate.
        inset_ring(
            frame,
            cx,
            0.050,
            0.0,
            data::FOUNTAIN_R,
            scale3(team_colour(team), 0.34),
        );
    }
    for [x, z] in data::COURT_POS {
        push_plaza(frame, x, z, 11.0);
    }
    push_low_garden(frame);
    push_perimeter(frame);
}

/// Leaf fans, broken pavers and tiny flowers decorate the lane shoulders.
/// Everything is at most 15 cm tall and remains visually walkable.
fn push_low_garden(frame: &mut Frame) {
    for i in -7i8..=7 {
        let x = f32::from(i) * 8.0;
        for side in [-1.0, 1.0] {
            if x.abs() < 5.0 {
                continue;
            }
            let z = side * (9.8 + (x * 0.31).sin() * 0.7);
            if !scenery_visible(frame, x, z, 1.8) {
                continue;
            }
            for leaf in 0..5 {
                let angle = leaf as f32 * 1.29 + x * 0.43;
                let length = 0.55 + leaf as f32 * 0.10;
                frame.instances.push(
                    ins(
                        v3(x, 0.03, z),
                        v3(length, 0.12, 0.28),
                        [0.055, 0.15 + leaf as f32 * 0.012, 0.085],
                    )
                    .with_mesh(MESH_BLADE)
                    .with_yaw(angle)
                    .with_surface(0.95, 0.0),
                );
            }
            for stone in 0..2 {
                let dx = stone as f32 * 0.57 - 0.6;
                frame.instances.push(
                    ins(
                        v3(x + dx, 0.045, z + side * 0.9),
                        v3(0.32, 0.075, 0.23),
                        [0.17, 0.19, 0.14],
                    )
                    .with_mesh(MESH_OCTA)
                    .with_yaw(x * 0.19)
                    .with_surface(0.94, 0.0),
                );
            }
            // A warm point of colour, small enough never to be a false unit.
            frame.instances.push(
                ins(
                    v3(x + 0.36, 0.08, z - 0.18),
                    v3(0.10, 0.045, 0.10),
                    [0.40, 0.28, 0.07],
                )
                .with_mesh(MESH_OCTA)
                .without_shadow(),
            );
        }
    }
}

/// A prop instance: origin on the ground, yaw in the sim's convention.
fn prop(frame: &mut Frame, mesh: u32, x: f32, z: f32, yaw: f32, scale: f32, rough: f32) {
    frame.instances.push(
        Instance::new(v3(x, 0.0, z), Vec3::splat(scale), Vec3::ONE)
            .with_rot(face(yaw))
            .with_mesh(mesh)
            .with_surface(rough, 0.0),
    );
}

/// Substantial scenery stays outside the entire x=+-68/z=+-40 walking
/// rectangle, including each mesh's footprint. There are no invisible tree
/// collisions and no arch standing over an unmodelled navigational obstacle.
fn push_perimeter(frame: &mut Frame) {
    let edge = [0.13, 0.17, 0.145];
    for side in [-1.0, 1.0] {
        stone_box(
            frame,
            v3(0.0, 0.16, side * 40.8),
            v3(138.0, 0.32, 0.65),
            edge,
            0.0,
        );
        stone_box(
            frame,
            v3(side * 68.8, 0.16, 0.0),
            v3(0.65, 0.32, 80.0),
            edge,
            0.0,
        );
        for i in -5i8..=5 {
            let x = f32::from(i) * 12.5;
            let z = side * 42.0;
            if !scenery_visible(frame, x, z, 4.0) {
                continue;
            }
            stone_box(
                frame,
                v3(x, 0.55, z),
                v3(1.1, 1.1, 1.2),
                [0.16, 0.195, 0.16],
                0.0,
            );
            stone_box(
                frame,
                v3(x, 1.13, z),
                v3(1.4, 0.16, 1.5),
                [0.28, 0.28, 0.20],
                0.0,
            );
            stone_box(
                frame,
                v3(x, -1.8, side * 46.3),
                v3(2.1, 3.3, 1.5),
                [0.10, 0.125, 0.10],
                0.0,
            );
            if i % 2 == 0 {
                prop(frame, MESH_TREE, x + 3.5, side * 44.1, x * 0.31, 0.72, 0.96);
            }
        }
        for z in [-27.0, -12.0, 12.0, 27.0] {
            if scenery_visible(frame, side * 73.0, z, 5.0) {
                prop(frame, MESH_TREE, side * 73.0, z, z * 0.11, 0.76, 0.96);
                stone_box(
                    frame,
                    v3(side * 70.2, 0.6, z + 2.0),
                    v3(1.1, 1.2, 1.1),
                    edge,
                    0.0,
                );
            }
        }
        if scenery_visible(frame, side * 73.0, 0.0, 6.0) {
            prop(
                frame,
                MESH_ARCH,
                side * 73.0,
                0.0,
                if side < 0.0 { 0.0 } else { PI },
                0.82,
                0.88,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// objectives
// ---------------------------------------------------------------------------

fn objective_motes(frame: &mut Frame, x: f32, z: f32, time: f32, color: [f32; 3], height: f32) {
    for i in 0..5 {
        let angle = time * 0.42 + i as f32 * TAU / 5.0;
        let rise = (time * 0.16 + i as f32 * 0.2).fract();
        frame.particles.push(Particle {
            position: v3(
                x + angle.cos() * 0.90,
                height + rise * 0.75,
                z + angle.sin() * 0.90,
            ),
            color: Vec3::from(color),
            size: Vec2::splat(0.09),
            opacity: (1.0 - rise) * 0.42,
        });
    }
}

fn push_core(frame: &mut Frame, u: &UnitLite, t: f32) {
    let col = if u.dead {
        [0.07, 0.085, 0.075]
    } else {
        team_colour(u.t)
    };
    // The complete silhouette stays within the core's 2.4m pick/hit radius.
    // Layered weathered stone supports a compact living crystal.
    for (radius, y, height, stone) in [
        (2.35, 0.13, 0.16, [0.11, 0.145, 0.12]),
        (2.05, 0.34, 0.09, [0.22, 0.24, 0.18]),
        (1.78, 0.48, 0.09, [0.09, 0.12, 0.10]),
    ] {
        frame.instances.push(
            ins(v3(u.x, y, u.z), v3(radius, height, radius), stone)
                .with_mesh(MESH_FRUSTUM)
                .with_surface(0.9, 0.0),
        );
    }
    inset_ring(frame, u.x, 0.58, u.z, 1.74, [0.26, 0.19, 0.08]);
    if u.dead {
        for i in 0..5 {
            let angle = i as f32 * TAU / 5.0;
            frame.instances.push(
                ins(
                    v3(u.x + angle.cos() * 0.8, 0.74, u.z + angle.sin() * 0.8),
                    v3(0.3, 0.25, 0.3),
                    col,
                )
                .with_mesh(MESH_OCTA)
                .with_yaw(angle),
            );
        }
        return;
    }
    // Taller spire, same footprint: the hearth is the landmark of its plaza
    // and has to read from mid-lane, while every ground point stays inside
    // the 2.4m pick radius (the test below checks the whole silhouette).
    prop(
        frame,
        MESH_OBELISK,
        u.x,
        u.z,
        if u.t == 0 { 0.0 } else { PI },
        0.72,
        0.88,
    );
    let bob = (t * 1.35).sin() * 0.11;
    frame.instances.push(
        ins(v3(u.x, 4.15 + bob, u.z), v3(0.66, 0.90, 0.66), col)
            .with_mesh(MESH_OCTA)
            .with_yaw(t * 0.22)
            .with_surface(0.28, 0.0),
    );
    let health = (u.hp / u.mh.max(1.0)).clamp(0.0, 1.0);
    for i in 0..8 {
        let angle = i as f32 * TAU / 8.0;
        let tint = if (i as f32 + 0.5) / 8.0 <= health {
            scale3(col, 0.35)
        } else {
            [0.045, 0.055, 0.045]
        };
        stone_box(
            frame,
            v3(u.x + angle.cos() * 2.1, 0.445, u.z + angle.sin() * 2.1),
            v3(0.26, 0.035, 0.10),
            tint,
            -angle,
        );
    }
    objective_motes(frame, u.x, u.z, t, scale3(col, 0.85), 2.7);
}

fn push_court(frame: &mut Frame, u: &UnitLite, t: f32) {
    let color = if u.k == 4 {
        [0.74, 0.40, 0.12]
    } else {
        [0.12, 0.57, 0.42]
    };
    for (radius, y, height, stone) in [
        (1.55, 0.13, 0.16, [0.12, 0.15, 0.12]),
        (1.31, 0.35, 0.08, [0.25, 0.25, 0.18]),
    ] {
        frame.instances.push(
            ins(v3(u.x, y, u.z), v3(radius, height, radius), stone)
                .with_mesh(MESH_FRUSTUM)
                .with_surface(0.9, 0.0),
        );
    }
    inset_ring(
        frame,
        u.x,
        0.445,
        u.z,
        1.28,
        if u.dead {
            [0.10, 0.12, 0.10]
        } else {
            scale3(color, 0.42)
        },
    );
    if u.dead {
        for i in 0..3 {
            let angle = i as f32 * TAU / 3.0;
            frame.instances.push(
                ins(
                    v3(u.x + angle.cos() * 0.5, 0.56, u.z + angle.sin() * 0.5),
                    v3(0.28, 0.18, 0.25),
                    [0.12, 0.16, 0.13],
                )
                .with_mesh(MESH_OCTA)
                .with_yaw(angle),
            );
        }
        return;
    }
    // 0.4 * the baked prop's bounding diagonal fits the Court's 1.6m body.
    prop(
        frame,
        MESH_OBELISK,
        u.x,
        u.z,
        if u.z > 0.0 { -PI / 2.0 } else { PI / 2.0 },
        0.46,
        0.9,
    );
    frame.instances.push(
        ins(
            v3(u.x, 2.85 + 0.09 * (t * 1.3).sin(), u.z),
            v3(0.42, 0.58, 0.42),
            color,
        )
        .with_mesh(MESH_OCTA)
        .with_yaw(t * 0.3)
        .with_surface(0.33, 0.0),
    );
    objective_motes(frame, u.x, u.z, t, color, 1.9);
}

// ---------------------------------------------------------------------------
// minions and champions
// ---------------------------------------------------------------------------

fn push_minion(frame: &mut Frame, u: &UnitLite, t: f32) {
    let col = team_colour(u.t);
    let r = face(u.fa);
    let step = (t * 9.0 + u.id as f32).sin() * 0.03;
    if u.k == 2 {
        // caster: a robe, a pale head and a staff held forward
        frame.instances.push(
            ins(v3(u.x, 0.72 + step, u.z), v3(0.42, 0.72, 0.42), scale3(col, 0.85)).with_rot(r).with_mesh(MESH_CONE),
        );
        frame.instances.push(
            ins(v3(u.x, 1.62 + step, u.z), v3(0.22, 0.22, 0.22), mix(col, [1.0, 1.0, 1.0], 0.45)).with_mesh(MESH_SPHERE),
        );
        let (sx, sz) = ahead(u.x, u.z, u.fa, 0.35, 0.3);
        frame.instances.push(
            ins(v3(sx, 1.0 + step, sz), v3(0.06, 0.9, 0.06), [0.5, 0.4, 0.25]).with_rot(r).with_mesh(0),
        );
        frame.instances.push(
            ins(v3(sx, 1.95 + step, sz), v3(0.14, 0.14, 0.14), [1.0, 0.9, 0.5]).with_mesh(MESH_OCTA),
        );
    } else {
        // melee: a squat body, a helmet and a shield on the left arm
        frame.instances.push(
            ins(v3(u.x, 0.6 + step, u.z), v3(0.46, 0.6, 0.38), col).with_rot(r).with_mesh(0),
        );
        frame.instances.push(
            ins(v3(u.x, 1.35 + step, u.z), v3(0.26, 0.28, 0.26), scale3(col, 0.6)).with_mesh(MESH_CONE),
        );
        let (hx, hz) = ahead(u.x, u.z, u.fa, 0.1, -0.42);
        frame.instances.push(
            ins(v3(hx, 0.7 + step, hz), v3(0.16, 0.34, 0.06), mix(col, [0.9, 0.9, 0.9], 0.3)).with_rot(r).with_mesh(0),
        );
    }
}

/// Buff kinds, as `sim::BuffKind::code` numbers them on the wire.
mod buff {
    pub const SLOW: u8 = 0;
    pub const MS: u8 = 1;
    pub const DMG_AMP: u8 = 2;
    pub const SHIELD: u8 = 3;
    pub const IMMUNE: u8 = 4;
    pub const ROOT: u8 = 5;
    pub const EXHAUST: u8 = 6;
    pub const BURN: u8 = 7;
    pub const REGEN: u8 = 8;
    pub const MANA_REGEN: u8 = 9;
    pub const FIRE: u8 = 10;
    pub const REVIVE: u8 = 11;
}

/// What a champion currently wears, read from the buff rows once per unit.
/// One flag per buff kind is the honest shape: they are independent and
/// the drawing code tests each on its own.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default, Clone, Copy)]
struct Wear {
    slow: bool,
    haste: bool,
    demon: bool,
    shield: bool,
    immune: bool,
    root: bool,
    exhaust: bool,
    burn: bool,
    regen: bool,
    mana: bool,
    fire: bool,
    revive: bool,
}

fn wear_of(id: u32, buffs: &[BuffSnap]) -> Wear {
    let mut w = Wear::default();
    for b in buffs.iter().filter(|b| b.u == id && b.ttl > 0.0) {
        match b.k {
            buff::SLOW => w.slow = true,
            buff::MS => w.haste = true,
            buff::DMG_AMP => w.demon = true,
            buff::SHIELD => w.shield = true,
            buff::IMMUNE => w.immune = true,
            buff::ROOT => w.root = true,
            buff::EXHAUST => w.exhaust = true,
            buff::BURN => w.burn = true,
            buff::REGEN => w.regen = true,
            buff::MANA_REGEN => w.mana = true,
            buff::FIRE => w.fire = true,
            buff::REVIVE => w.revive = true,
            _ => {}
        }
    }
    w
}

/// One champion or hologram, with its kit's silhouette and its buffs.
fn push_champion(
    frame: &mut Frame,
    u: &UnitLite,
    t: f32,
    wear: Wear,
    mine: bool,
    pose: crate::combat::AttackPose,
) {
    let def = u.def;
    let team = team_colour(u.t);
    let bob = (t * 5.0 + u.id as f32).sin() * 0.04;
    let holo = u.k == 3;
    // holograms flicker: same body, cyan cast, scale wobble
    let (mut col, wob) = if holo {
        (
            [0.45 * u.colour[0] + 0.25, 0.8, u.colour[2] * 0.5 + 0.5],
            1.0 + 0.05 * (t * 17.0).sin(),
        )
    } else {
        (u.colour, 1.0)
    };
    if wear.immune {
        col = mix(col, [1.0, 1.0, 1.0], 0.6);
    }
    if wear.exhaust {
        col = scale3(col, 0.55);
    }
    if wear.demon && def != data::KNIGHT {
        col = mix(col, [0.9, 0.1, 0.1], 0.5);
    }
    let r = face(u.fa);
    let (x, z) = (u.x, u.z);

    // the ground ring says the team even when colours run together; mine
    // is doubled so the eye finds it in a brawl. A baked body is bigger
    // than the procedural ones, so its fine outline grows with it. The
    // actual body and its ground shadow stay visible inside the outline.
    let baked = art::champion(def);
    let ring = baked.map_or(1.15, |c| 0.62 + c.height * 0.36);
    frame.instances.push(
        ins(v3(x, 0.05, z), v3(ring * wob, 1.0, ring * wob), team)
            .with_mesh(MESH_RING)
            .without_shadow(),
    );
    if mine {
        frame.instances.push(
            ins(
                v3(x, 0.04, z),
                v3(ring * 1.26, 1.0, ring * 1.26),
                [0.95, 0.95, 0.9],
            )
            .with_mesh(MESH_RING)
            .without_shadow(),
        );
    }
    if wear.slow {
        frame.instances.push(
            ins(v3(x, 0.07, z), v3(0.85, 1.0, 0.85), [0.25, 0.5, 0.7])
                .with_mesh(MESH_RING)
                .without_shadow(),
        );
    }

    let y = bob;
    if let Some(baked) = art::champion(def) {
        push_baked(frame, baked, u, t, wear, col, holo, wob, pose);
    } else {
        push_procedural(frame, def, u, t, wear, col, wob, r, x, y, z);
    }

    push_wearables(frame, u, t, wear, x, y, z, r);
}

/// A champion delivered as baked art: its parts at the unit's position,
/// facing the sim's yaw, drawn white so the texture is not double-tinted;
/// tints that mean something (hologram, immunity, exhaust, demon) still
/// multiply on top. Motion is procedural: a breath bob, a lean into the
/// facing, and a slow turn for the kits that are more orb than body.
#[allow(clippy::too_many_arguments)]
fn push_baked(frame: &mut Frame, baked: &art::Champion, u: &UnitLite, t: f32, wear: Wear, col: [f32; 3], holo: bool, wob: f32, pose: crate::combat::AttackPose) {
    let tinted = holo || wear.immune || wear.exhaust || (wear.demon && u.def != data::KNIGHT);
    let paint = if tinted { col } else { [1.0, 1.0, 1.0] };
    let breath = (t * 2.2 + u.id as f32).sin() * 0.015;
    let scale = Vec3::splat(wob * (1.0 + breath)) * pose.stretch;
    let spin = if u.def == data::SWARM { Quat::from_rotation_y(t * 0.35) } else { Quat::IDENTITY };
    let body = face(u.fa) * spin * pose.rotation;
    let lift = if u.def == data::SWARM { 0.25 + (t * 1.7 + u.id as f32).sin() * 0.06 } else { 0.0 };
    let origin = v3(u.x, lift, u.z) + pose.offset;
    for part in &baked.parts {
        // A part turns by `swing` about its own pivot p, then the whole body
        // by `body`: v' = body * (p + swing * (v - p)). The engine applies
        // scale, then rotation, then translation, so the instance rotation
        // is body * swing and the translation carries body * (p - swing p).
        // Parts named like a weapon sway at idle; an attack swing rides the
        // same hook once the wire says who struck.
        let swing = if part_is_weapon(&part.name) {
            Quat::from_rotation_z((t * 1.3 + u.id as f32).sin() * 0.08)
        } else {
            Quat::IDENTITY
        };
        let p = part.pivot * scale;
        frame.instances.push(
            Instance::new(origin + body * (p - swing * p), scale, Vec3::from(paint))
                .with_rot(body * swing)
                .with_mesh(part.mesh)
                .with_surface(0.65, 0.2),
        );
    }
    if u.def == data::SWARM {
        // the swarm around the orb stays procedural: three drones in orbit
        for i in 0..3 {
            let a = t * 2.4 + TAU * (i as f32) / 3.0 + u.id as f32;
            let h = lift + 0.9 + 0.25 * (t * 3.0 + i as f32 * 2.0).sin();
            frame.instances.push(
                ins(v3(u.x + a.cos() * 1.05, h, u.z + a.sin() * 1.05), v3(0.13, 0.13, 0.13), mix(col, [1.0, 1.0, 1.0], 0.35))
                    .with_rot(Quat::from_rotation_y(-a))
                    .with_mesh(MESH_OCTA),
            );
        }
    }
}

/// Whether a baked part's node name marks it as the thing the champion
/// swings: `sword`, `blade`, `staff`, `weapon`, `hook`, `hand` all count.
fn part_is_weapon(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["sword", "blade", "staff", "weapon", "hook", "hand"].iter().any(|k| n.contains(k))
}

/// The procedural bodies: one silhouette per kit from the primitive set,
/// used until a champion's baked art is delivered.
#[allow(clippy::too_many_arguments)]
fn push_procedural(frame: &mut Frame, def: u8, u: &UnitLite, t: f32, wear: Wear, col: [f32; 3], wob: f32, r: Quat, x: f32, y: f32, z: f32) {
    match def {
        data::KNIGHT => {
            let (body, s) = if wear.demon {
                ([0.55, 0.05, 0.06], 1.12)
            } else {
                (col, 1.0)
            };
            frame.instances.push(
                ins(v3(x, (0.95 + y) * s, z), v3(0.62 * s * wob, 0.78 * s, 0.46 * s * wob), body).with_rot(r).with_mesh(0),
            );
            frame.instances.push(
                ins(v3(x, (1.66 + y) * s, z), v3(0.25 * s, 0.25 * s, 0.25 * s), mix(body, [1.0, 0.85, 0.6], 0.3))
                    .with_mesh(MESH_SPHERE),
            );
            for side in [-0.46, 0.46] {
                let (px, pz) = ahead(x, z, u.fa, 0.0, side * s);
                frame.instances.push(
                    ins(v3(px, (1.3 + y) * s, pz), v3(0.2 * s, 0.14 * s, 0.24 * s), scale3(body, 0.8)).with_rot(r).with_mesh(0),
                );
            }
            if wear.demon {
                for side in [-0.16, 0.16] {
                    let (hx, hz) = ahead(x, z, u.fa, 0.05, side * s);
                    frame.instances.push(
                        ins(v3(hx, (2.0 + y) * s, hz), v3(0.08, 0.22, 0.08), [0.15, 0.05, 0.05])
                            .with_rot(r * Quat::from_rotation_x(side.signum() * 0.35))
                            .with_mesh(MESH_CONE),
                    );
                }
                frame.instances.push(
                    ins(v3(x, 0.08, z), v3(1.3, 1.0, 1.3), [0.9, 0.15, 0.05]).with_mesh(MESH_RING),
                );
            }
            // the blade, held on the right, pointing where the knight looks
            let (bx, bz) = ahead(x, z, u.fa, 0.15, 0.4 * s);
            let blade = if wear.fire { [1.0, 0.8, 0.25] } else { [0.85, 0.85, 0.92] };
            frame.instances.push(
                ins(v3(bx, (1.0 + y) * s, bz), v3(1.35 * s, 0.09, 0.16), blade).with_rot(r).with_mesh(MESH_BLADE),
            );
            if wear.fire {
                let (tx, tz) = ahead(x, z, u.fa, 1.3 * s, 0.4 * s);
                let flick = 0.14 + 0.05 * (t * 21.0).sin();
                frame.instances.push(
                    ins(v3(tx, 1.15 + y + flick, tz), v3(flick, flick * 1.6, flick), [1.0, 0.55, 0.1]).with_mesh(MESH_OCTA),
                );
            }
        }
        data::HALLOW => {
            let float = y * 2.0 + 0.1;
            frame.instances.push(
                ins(v3(x, 0.9 + float, z), v3(0.55 * wob, 0.9, 0.55 * wob), col).with_rot(r).with_mesh(MESH_CONE),
            );
            frame.instances.push(
                ins(v3(x, 1.95 + float, z), v3(0.22, 0.22, 0.22), mix(col, [1.0, 1.0, 1.0], 0.5)).with_mesh(MESH_SPHERE),
            );
            frame.instances.push(
                ins(v3(x, 2.35 + float, z), v3(0.36, 1.0, 0.36), [1.0, 0.9, 0.5]).with_mesh(MESH_RING),
            );
            // a hand raised toward whatever it faces
            let (hx, hz) = ahead(x, z, u.fa, 0.55, 0.25);
            frame.instances.push(
                ins(v3(hx, 1.35 + float, hz), v3(0.12, 0.12, 0.12), mix(col, [1.0, 1.0, 1.0], 0.5)).with_mesh(MESH_SPHERE),
            );
        }
        data::MAW => {
            frame.instances.push(
                ins(v3(x, 0.55 + y, z), v3(0.85 * wob, 0.55, 0.75 * wob), col).with_rot(r).with_mesh(MESH_FRUSTUM),
            );
            let (hx, hz) = ahead(x, z, u.fa, 0.35, 0.0);
            frame.instances.push(
                ins(v3(hx, 1.1 + y, hz), v3(0.62, 0.42, 0.62), scale3(col, 0.9)).with_rot(r).with_mesh(0),
            );
            // the jaws chew on a slow cycle
            let chew = 0.5 + 0.5 * (t * 2.6 + u.id as f32).sin();
            let (jx, jz) = ahead(x, z, u.fa, 0.85, 0.0);
            frame.instances.push(
                ins(v3(jx, 1.28 + y, jz), v3(0.5, 0.09, 0.56), scale3(col, 0.7)).with_rot(r).with_mesh(0),
            );
            frame.instances.push(
                ins(v3(jx, 0.92 + y - chew * 0.18, jz), v3(0.5, 0.09, 0.56), scale3(col, 0.7)).with_rot(r).with_mesh(0),
            );
            for side in [-0.2, 0.2] {
                let (ex, ez) = ahead(x, z, u.fa, 0.62, side);
                frame.instances.push(
                    ins(v3(ex, 1.32 + y, ez), v3(0.07, 0.07, 0.07), [1.0, 0.9, 0.2]).with_mesh(MESH_SPHERE),
                );
            }
            for (i, back) in [-0.15, -0.42, -0.68].into_iter().enumerate() {
                let (sx, sz) = ahead(x, z, u.fa, back, 0.0);
                let h = 0.32 - i as f32 * 0.06;
                frame.instances.push(
                    ins(v3(sx, 1.05 + h + y, sz), v3(0.11, h, 0.11), scale3(col, 0.6)).with_mesh(MESH_CONE),
                );
            }
        }
        data::TESSERA => {
            let spin = t * 1.6;
            let tilt = Quat::from_rotation_x(PI / 2.0);
            frame.instances.push(ins(v3(x, 0.22 + y, z), v3(0.3, 0.22, 0.3), scale3(col, 0.6)).with_mesh(MESH_FRUSTUM));
            frame.instances.push(
                ins(v3(x, 0.98 + y, z), v3(0.62 * wob, 0.62, 0.62 * wob), col)
                    .with_rot(r * tilt * Quat::from_rotation_y(spin))
                    .with_mesh(MESH_GEAR),
            );
            // two clock hands in the gear's plane
            for (len, rate) in [(0.5, 1.0), (0.34, 12.0)] {
                frame.instances.push(
                    ins(v3(x, 0.98 + y, z), v3(len, 0.05, 0.05), [0.95, 0.92, 0.8])
                        .with_rot(r * tilt * Quat::from_rotation_y(spin * rate))
                        .with_mesh(MESH_BLADE),
                );
            }
            frame.instances.push(
                ins(v3(x, 1.85 + y, z), v3(0.2, 0.2, 0.2), mix(col, [1.0, 1.0, 1.0], 0.4)).with_mesh(MESH_SPHERE),
            );
        }
        _ => {
            // SW4RM: a hovering core with three drones in orbit
            frame.instances.push(ins(v3(x, 0.35 + y, z), v3(0.7 * wob, 1.0, 0.7 * wob), scale3(col, 0.45)).with_mesh(MESH_DISC));
            frame.instances.push(
                ins(v3(x, 1.15 + y, z), v3(0.42 * wob, 0.42 * wob, 0.42 * wob), col)
                    .with_rot(r * Quat::from_rotation_y(t * 0.8))
                    .with_mesh(MESH_OCTA),
            );
            for i in 0..3 {
                let a = t * 2.4 + TAU * (i as f32) / 3.0 + u.id as f32;
                let h = 1.0 + 0.25 * (t * 3.0 + i as f32 * 2.0).sin();
                frame.instances.push(
                    ins(v3(x + a.cos() * 0.85, h + y, z + a.sin() * 0.85), v3(0.14, 0.14, 0.14), mix(col, [1.0, 1.0, 1.0], 0.35))
                        .with_rot(Quat::from_rotation_y(-a))
                        .with_mesh(MESH_OCTA),
                );
            }
        }
    }
}

/// Buff wearables shared by every kit, baked or procedural.
#[allow(clippy::too_many_arguments)]
fn push_wearables(frame: &mut Frame, u: &UnitLite, t: f32, wear: Wear, x: f32, y: f32, z: f32, r: Quat) {
    if wear.shield {
        frame.instances.push(
            ins(v3(x, 1.0 + y, z), v3(0.95, 1.0, 0.95), [0.55, 0.75, 1.0])
                .with_rot(Quat::from_rotation_y(t * 2.0) * Quat::from_rotation_x(0.35))
                .with_mesh(MESH_RING),
        );
    }
    if wear.immune {
        frame.instances.push(
            ins(v3(x, 0.6 + y, z), v3(1.05, 1.0, 1.05), [1.0, 1.0, 1.0])
                .with_rot(Quat::from_rotation_y(-t * 3.0) * Quat::from_rotation_x(0.5))
                .with_mesh(MESH_RING),
        );
    }
    if wear.root {
        for i in 0..3 {
            let a = TAU * (i as f32) / 3.0 + t * 0.2;
            frame.instances.push(
                ins(v3(x + a.cos() * 0.7, 0.12, z + a.sin() * 0.7), v3(0.14, 0.12, 0.14), [0.16, 0.14, 0.1]).with_mesh(0),
            );
        }
    }
    if wear.burn {
        for i in 0..2 {
            let ph = t * 9.0 + i as f32 * 2.1 + u.id as f32;
            frame.instances.push(
                ins(v3(x + ph.cos() * 0.3, 1.9 + y + 0.15 * (ph * 1.7).sin(), z + ph.sin() * 0.3), v3(0.1, 0.16, 0.1), [1.0, 0.5, 0.1])
                    .with_mesh(MESH_OCTA),
            );
        }
    }
    if wear.haste {
        for side in [-0.3, 0.3] {
            let (lx, lz) = ahead(x, z, u.fa, -0.9, side);
            frame.instances.push(
                ins(v3(lx, 0.6 + y, lz), v3(0.6, 0.04, 0.04), [0.9, 0.95, 1.0]).with_rot(r).with_mesh(0),
            );
        }
    }
    if wear.revive {
        frame.instances.push(
            ins(v3(x, 2.5 + y, z), v3(0.42, 1.0, 0.42), [1.0, 0.85, 0.35])
                .with_rot(Quat::from_rotation_y(t * 1.2))
                .with_mesh(MESH_RING),
        );
    }
    if wear.regen || wear.mana {
        let c = if wear.regen { [0.4, 1.0, 0.45] } else { [0.4, 0.6, 1.0] };
        let rise = (t * 1.5 + u.id as f32).fract();
        frame.instances.push(
            ins(v3(x + 0.45, 0.8 + rise * 1.2, z), v3(0.07, 0.07, 0.07), c).with_mesh(MESH_OCTA),
        );
    }
}

fn push_hp_bar(frame: &mut Frame, u: &UnitLite) {
    if u.dead || u.mh <= 0.0 {
        return;
    }
    let frac = (u.hp / u.mh).clamp(0.0, 1.0);
    let objective = matches!(u.k, 4..=7);
    if frac > 0.999 && objective {
        return; // objectives show their bar in the HUD, not in the world
    }
    let (w, y) = match u.k {
        // a baked body carries its own height in the sidecar; the bar rides
        // a hand above it and widens with it, the procedural bodies keep
        // their tuned constants
        0 | 3 => art::champion(u.def).map_or((1.4, 2.55), |c| (1.2 + c.height * 0.3, c.height + 1.1)),
        4 | 5 => (2.4, 3.8),
        6 | 7 => (3.0, 4.9),
        _ => (0.7, 1.8),
    };
    let tilt = bar_tilt();
    let h = if objective { 0.16 } else { 0.12 };
    // bars are HUD, not world: they cast no shadow
    frame.instances.push(
        ins(v3(u.x, y, u.z), v3(w + 0.08, h + 0.06, 0.03), [0.04, 0.04, 0.05]).with_rot(tilt).with_mesh(0).without_shadow(),
    );
    let col = if u.t == 2 {
        [0.85, 0.78, 0.4]
    } else if frac < 0.3 {
        mix(team_colour(u.t), [1.0, 0.2, 0.1], 0.5)
    } else {
        team_colour(u.t)
    };
    // the fill grows from the left edge; local +X is screen right
    frame.instances.push(
        ins(v3(u.x - w * (1.0 - frac) / 2.0, y, u.z), v3(w * frac, h, 0.035), col).with_rot(tilt).with_mesh(0).without_shadow(),
    );
    if (u.k == 0 || u.k == 3) && u.mm > 0.0 {
        let mf = (u.mn / u.mm).clamp(0.0, 1.0);
        // the mana sliver hangs just under the bar, in the card's own plane
        let down = tilt * Vec3::new(0.0, -0.13, 0.0);
        frame.instances.push(
            ins(v3(u.x - w * (1.0 - mf) / 2.0, y, u.z) + down, v3(w * mf, 0.05, 0.035), [0.35, 0.55, 1.0]).with_rot(tilt).with_mesh(0).without_shadow(),
        );
    }
}

// ---------------------------------------------------------------------------
// shots, zones, effects
// ---------------------------------------------------------------------------

/// A projectile in flight. `k` is `sim::ProjKind` as the wire numbers it:
/// 0 auto-attack, 1 drone, 2 gear bolt, 3 hook.
fn push_proj(frame: &mut Frame, p: &ProjSnap, t: f32) {
    if p.champ < 5 || p.k != 0 {
        crate::combat::draw_projectile(frame, p, t);
        return;
    }
    let (x, z) = (p.x, p.z);
    let yaw = p.dz.atan2(p.dx);
    let r = face(yaw);
    let col = team_colour(p.t);
    match p.k {
        1 => {
            // a drone: a jittering diamond with a cyan glint
            let jit = (t * 31.0 + x * 3.0).sin() * 0.12;
            let (px, pz) = ahead(x, z, yaw, 0.0, jit);
            frame.instances.push(
                ins(v3(px, 1.1 + jit * 0.5, pz), v3(0.16, 0.12, 0.16), [0.5, 0.95, 1.0]).with_rot(r).with_mesh(MESH_OCTA),
            );
        }
        2 => {
            // a gear bolt spinning flat
            frame.instances.push(
                ins(v3(x, 1.0, z), v3(0.3, 0.12, 0.3), [0.85, 0.7, 1.0])
                    .with_rot(Quat::from_rotation_y(t * 18.0))
                    .with_mesh(MESH_GEAR),
            );
        }
        3 => {
            // the hook head and three chain links trailing it
            frame.instances.push(ins(v3(x, 0.9, z), v3(0.24, 0.24, 0.24), [0.6, 0.75, 0.35]).with_rot(r).with_mesh(MESH_OCTA));
            for i in 1..=3 {
                let (lx, lz) = ahead(x, z, yaw, -0.45 * i as f32, 0.0);
                frame.instances.push(ins(v3(lx, 0.9, lz), v3(0.16, 0.08, 0.08), [0.45, 0.45, 0.4]).with_rot(r).with_mesh(0));
            }
        }
        _ => {
            // an auto-attack: a bright bead with a short tail
            let bead = mix(col, [1.0, 1.0, 1.0], 0.5);
            frame.instances.push(ins(v3(x, 1.05, z), v3(0.13, 0.13, 0.13), bead).with_mesh(MESH_SPHERE));
            let (tx, tz) = ahead(x, z, yaw, -0.3, 0.0);
            frame.instances.push(ins(v3(tx, 1.05, tz), v3(0.5, 0.05, 0.05), col).with_rot(r).with_mesh(0));
        }
    }
}

fn push_zone(frame: &mut Frame, zone: &ZoneLite, t: f32) {
    if crate::combat::draw_zone(frame, zone, t) {
        return;
    }
    let (zk, x, z, r, spin) = *zone;
    match zk {
        0 => {
            // a tornado, drawn OPEN: whoever stands inside must stay visible
            // and clickable, so there is no solid funnel. A scorched disc, a
            // ring at the ground and a tilted ring up high, a thin bright
            // column, and embers orbiting between them carry the motion.
            // (Stacked gears read as one flat cog from above, and a cone
            // hid the bodies it was meant to threaten.)
            let pulse = 0.85 + 0.15 * (t * 9.0).sin();
            frame.instances.push(ins(v3(x, 0.07, z), v3(r, 1.0, r), [0.55, 0.2, 0.05]).with_mesh(MESH_DISC));
            frame.instances.push(
                ins(v3(x, 0.16, z), v3(r, 1.0, r), [1.0 * pulse, 0.5 * pulse, 0.12])
                    .with_rot(Quat::from_rotation_y(spin))
                    .with_mesh(MESH_RING),
            );
            frame.instances.push(
                ins(v3(x, 2.5, z), v3(r * 0.65, 1.0, r * 0.65), [1.0, 0.75 * pulse, 0.2])
                    .with_rot(Quat::from_rotation_y(-spin * 1.5) * Quat::from_rotation_x(0.3))
                    .with_mesh(MESH_RING),
            );
            frame.instances.push(
                ins(v3(x, 1.4, z), v3(r * 0.12, 1.4, r * 0.12), [1.0, 0.85, 0.4]).with_mesh(MESH_FRUSTUM),
            );
            for i in 0..8 {
                let a = spin * 2.0 + TAU * (i as f32) / 8.0;
                let rr = r * (i as f32 * 1.7).sin().mul_add(0.25, 0.7);
                let h = (i as f32 * 0.9 + t * 0.7).sin().mul_add(1.0, 1.4);
                let hot = i % 2 == 0;
                frame.instances.push(
                    ins(
                        v3(x + a.cos() * rr, h, z + a.sin() * rr),
                        v3(0.18, 0.18, 0.18),
                        if hot { [1.0, 0.6, 0.15] } else { [0.3, 0.14, 0.06] },
                    )
                    .with_rot(Quat::from_rotation_y(a * 3.0))
                    .with_mesh(if hot { MESH_OCTA } else { 0 }),
                );
            }
        }
        1 => {
            // a chrono trap: a low violet plate with four teeth at the rim
            frame.instances.push(ins(v3(x, 0.08, z), v3(r, 1.0, r), [0.45, 0.35, 0.85]).with_mesh(MESH_RING));
            for i in 0..4 {
                let a = TAU * (i as f32) / 4.0 + spin * 0.3;
                frame.instances.push(
                    ins(v3(x + a.cos() * r * 0.9, 0.2, z + a.sin() * r * 0.9), v3(0.12, 0.2, 0.12), [0.7, 0.6, 1.0]).with_mesh(MESH_CONE),
                );
            }
        }
        2 => {
            // the grand mechanism: a great gear turning flat, a ring at its rim
            frame.instances.push(
                ins(v3(x, 0.35, z), v3(r, 1.0, r), [0.4, 0.32, 0.85])
                    .with_rot(Quat::from_rotation_y(spin * 0.5))
                    .with_mesh(MESH_GEAR),
            );
            frame.instances.push(ins(v3(x, 0.08, z), v3(r * 1.05, 1.0, r * 1.05), [0.75, 0.65, 1.0]).with_mesh(MESH_RING));
            for (len, rate) in [(r * 0.85, 0.5), (r * 0.6, 6.0)] {
                frame.instances.push(
                    ins(v3(x, 0.72, z), v3(len, 0.08, 0.1), [0.95, 0.92, 0.8])
                        .with_rot(Quat::from_rotation_y(spin * rate))
                        .with_mesh(MESH_BLADE),
                );
            }
        }
        _ => {
            // the fen shroud: a rotting ring and spores drifting up
            frame.instances.push(ins(v3(x, 0.1, z), v3(r, 1.0, r), [0.25, 0.5, 0.2]).with_mesh(MESH_RING));
            for i in 0..4 {
                let a = TAU * (i as f32) / 4.0 + t * 0.7;
                let rise = (t * 0.8 + i as f32 * 0.25).fract();
                frame.instances.push(
                    ins(v3(x + a.cos() * r * 0.6, 0.3 + rise * 1.6, z + a.sin() * r * 0.6), v3(0.1, 0.1, 0.1), [0.45, 0.8, 0.3])
                        .with_mesh(MESH_OCTA),
                );
            }
        }
    }
}

/// A chain of small links from (x, z) to (x2, z2) at height `y`.
fn push_chain(frame: &mut Frame, x: f32, z: f32, x2: f32, z2: f32, y: f32, col: [f32; 3]) {
    let (dx, dz) = (x2 - x, z2 - z);
    let len = (dx * dx + dz * dz).sqrt().max(0.01);
    let yaw = dz.atan2(dx);
    let r = face(yaw);
    let n = (len / 0.5).ceil().clamp(1.0, 40.0) as usize;
    for i in 0..n {
        let f = (i as f32 + 0.5) / n as f32;
        frame.instances.push(
            ins(v3(x + dx * f, y, z + dz * f), v3(0.3, 0.07, 0.09), col).with_rot(r).with_mesh(0),
        );
    }
}

/// Transient effects, aged by the caller. `age` is 0..1 over the effect's
/// short life.
fn push_fx(frame: &mut Frame, fx: &FxLite, age: f32) {
    if crate::combat::draw_fx(frame, fx, age) {
        return;
    }
    let fade = |c: f32| c * (1.0 - age * 0.6);
    let line_yaw = |fx: &FxLite| (fx.z2 - fx.z).atan2(fx.x2 - fx.x);
    match fx.k {
        0 => {
            // `v` is a flag word: bit 0 crit, bit 1 spell (from the sim's
            // `f32::from(crit) + 2.0`), carried as a float on the wire
            let flags = fx.v.max(0.0) as u8;
            let crit = flags & 1 == 1;
            let spell = flags & 2 == 2;
            if fx.x2.abs() > 1e-6 || fx.z2.abs() > 1e-6 {
                // a melee blow: the blade sweeps from the striker to the struck
                let yaw = line_yaw(fx) + (age - 0.5) * 1.6;
                let big = if crit { 1.35 } else { 1.0 };
                let col = if crit { [1.0, 0.95, 0.7] } else { [1.0, 0.85, 0.55] };
                frame.instances.push(
                    ins(v3(fx.x, 1.1, fx.z), v3(1.6 * big, 0.06, 0.5), col).with_rot(face(yaw)).with_mesh(MESH_BLADE),
                );
            } else {
                // a ranged hit lands: a spark that shrinks
                let col = if spell { [0.8, 0.6, 1.0] } else { [1.0, 0.9, 0.6] };
                let s = 0.45 * (1.0 - age) * if crit { 1.5 } else { 1.0 };
                frame.instances.push(ins(v3(fx.x, 1.2, fx.z), v3(s, s, s), col).with_rot(Quat::from_rotation_y(age * 4.0)).with_mesh(MESH_OCTA));
            }
        }
        1 => {
            // beam between two points: a hot core inside a wider glow
            let (dx, dz) = (fx.x2 - fx.x, fx.z2 - fx.z);
            let len = (dx * dx + dz * dz).sqrt().max(0.01);
            let r = face(line_yaw(fx));
            let mid = v3(f32::midpoint(fx.x, fx.x2), 1.05, f32::midpoint(fx.z, fx.z2));
            frame.instances.push(ins(mid, v3(len, 0.22 * (1.0 - age), 0.22 * (1.0 - age)), [1.0, fade(0.45), fade(0.15)]).with_rot(r).with_mesh(0));
            frame.instances.push(ins(mid, v3(len, 0.09, 0.09), [1.0, 1.0, fade(0.85)]).with_rot(r).with_mesh(0));
        }
        2 | 3 => {
            // expanding ring (explosion, zone spawn) with a flash at birth
            let r = fx.v * (0.4 + age * 1.1);
            let col = if fx.k == 2 { [fade(1.0), fade(0.6), fade(0.15)] } else { [fade(0.8), fade(0.7), 1.0] };
            frame.instances.push(ins(v3(fx.x, 0.12, fx.z), v3(r, 1.0, r), col).with_mesh(MESH_RING));
            if age < 0.35 {
                let h = fx.v.min(4.0) * (1.0 - age / 0.35);
                frame.instances.push(ins(v3(fx.x, h * 0.5, fx.z), v3(fx.v * 0.2, h * 0.5, fx.v * 0.2), col).with_mesh(MESH_CONE));
            }
        }
        4 => {
            // trap planted: a violet pulse
            frame.instances.push(ins(v3(fx.x, 0.09, fx.z), v3(fx.v * (1.2 - age * 0.4), 1.0, fx.v * (1.2 - age * 0.4)), [fade(0.6), fade(0.5), 1.0]).with_mesh(MESH_RING));
        }
        5 => {
            // death: five shards thrown out and up
            for i in 0..5 {
                let a = TAU * (i as f32) / 5.0 + fx.x * 0.1;
                let d = 0.3 + age * 1.6;
                let h = 1.0 + age * 2.2 - age * age * 3.0;
                frame.instances.push(
                    ins(v3(fx.x + a.cos() * d, h.max(0.1), fx.z + a.sin() * d), v3(0.14, 0.14, 0.14), [fade(0.7), fade(0.7), fade(0.7)])
                        .with_rot(Quat::from_rotation_y(a + age * 6.0))
                        .with_mesh(0),
                );
            }
        }
        6 => {
            // level-up: a column of light and a gold ring
            frame.instances.push(ins(v3(fx.x, 1.6 + age, fx.z), v3(0.25, 1.6, 0.25), [fade(0.9), fade(0.8), 1.0]).with_mesh(MESH_FRUSTUM));
            frame.instances.push(ins(v3(fx.x, 0.1, fx.z), v3(0.6 + age * 1.6, 1.0, 0.6 + age * 1.6), [1.0, fade(0.85), fade(0.3)]).with_mesh(MESH_RING));
        }
        7 => {
            // coin blink
            frame.instances.push(
                ins(v3(fx.x, 2.2 + age * 0.8, fx.z), v3(0.2, 0.2, 0.2), [1.0, fade(0.85), fade(0.3)])
                    .with_rot(Quat::from_rotation_y(age * 9.0))
                    .with_mesh(0),
            );
        }
        8 => {
            // dash / blink / split streak between points, rings at both ends
            let (dx, dz) = (fx.x2 - fx.x, fx.z2 - fx.z);
            let len = (dx * dx + dz * dz).sqrt();
            let col = if fx.v >= 1.0 { [fade(0.5), 1.0, 1.0] } else { [fade(0.5), fade(0.8), 1.0] };
            if len > 0.05 {
                frame.instances.push(
                    ins(v3(f32::midpoint(fx.x, fx.x2), 0.9, f32::midpoint(fx.z, fx.z2)), v3(len, 0.06, 0.35 * (1.0 - age)), col)
                        .with_rot(face(line_yaw(fx)))
                        .with_mesh(0),
                );
            }
            for (px, pz) in [(fx.x, fx.z), (fx.x2, fx.z2)] {
                frame.instances.push(ins(v3(px, 0.1, pz), v3(0.5 + age, 1.0, 0.5 + age), col).with_mesh(MESH_RING));
            }
        }
        9 => {
            // heal / hymn / revive mark / revive proc
            let col = match fx.v as u8 {
                1 => [fade(0.9), fade(0.95), 1.0],
                2 | 3 => [1.0, fade(0.85), fade(0.35)],
                _ => [fade(0.5), 1.0, fade(0.5)],
            };
            let s = if fx.v >= 3.0 { 2.0 } else { 1.0 };
            frame.instances.push(ins(v3(fx.x, 1.1, fx.z), v3((0.9 + age * 0.6) * s, 1.4, (0.9 + age * 0.6) * s), col).with_mesh(MESH_RING));
            for i in 0..3 {
                let a = TAU * (i as f32) / 3.0 + age * 3.0;
                frame.instances.push(
                    ins(v3(fx.x + a.cos() * 0.6, 0.8 + age * 1.8, fx.z + a.sin() * 0.6), v3(0.09, 0.14, 0.09), col).with_mesh(MESH_OCTA),
                );
            }
        }
        10 => push_chain(frame, fx.x, fx.z, fx.x2, fx.z2, 0.8, [0.5, fade(0.8), 0.35]),
        11 => {
            // exhaust mark
            frame.instances.push(
                ins(v3(fx.x, 1.9, fx.z), v3(0.5 * (1.0 - age), 0.5, 0.5 * (1.0 - age)), [fade(0.5), fade(0.4), fade(0.9)]).with_mesh(MESH_OCTA),
            );
        }
        12 => {
            // shield flash: a blue ring rising
            frame.instances.push(
                ins(v3(fx.x, 0.6 + age * 1.2, fx.z), v3(1.0 + age * 0.4, 1.0, 1.0 + age * 0.4), [fade(0.6), fade(0.8), 1.0])
                    .with_rot(Quat::from_rotation_x(0.3))
                    .with_mesh(MESH_RING),
            );
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// the frame
// ---------------------------------------------------------------------------

/// Everything the frame builder reads. `buffs` and `projs` may be empty
/// when the caller has nothing yet; the picture degrades to bodies only.
pub struct SceneInput<'a> {
    pub units: &'a [UnitLite],
    pub zones: &'a [ZoneLite],
    pub fx: &'a [FxLite],
    pub buffs: &'a [BuffSnap],
    pub projs: &'a [ProjSnap],
    pub time: f32,
    pub camera: Camera,
    /// The roster seat whose champion gets the bright ring.
    pub my_slot: Option<u8>,
}

/// Test helper for a scene without buffs or an own-seat marker; both game
/// modes call [`scene_with`].
#[cfg(test)]
#[must_use]
pub fn scene(
    units: &[UnitLite],
    zones: &[ZoneLite],
    fx: &[FxLite],
    time: f32,
    camera: Camera,
    projs: &[ProjSnap],
) -> Frame {
    scene_with(&SceneInput {
        units,
        zones,
        fx,
        buffs: &[],
        projs,
        time,
        camera,
        my_slot: None,
    })
}

/// The frame: field first, then zones and shots, bodies, bars, effects.
#[must_use]
pub fn scene_with(input: &SceneInput<'_>) -> Frame {
    let t = input.time;
    let camera = review_camera(input).unwrap_or(input.camera);
    let mut frame = Frame {
        camera,
        instances: Vec::with_capacity(input.units.len() * 8 + input.fx.len() * 3 + 96),
        fog: Fog {
            color: [0.045, 0.075, 0.070],
            density: 0.0005,
        },
        environment: garden_light(t),
        ..Frame::default()
    };
    push_ground(&mut frame);
    push_showcase(&mut frame, &camera, t);
    for zone in input.zones {
        push_zone(&mut frame, zone, t);
    }
    for p in input.projs {
        push_proj(&mut frame, p, t);
    }
    for u in input.units {
        match u.k {
            6 | 7 => push_core(&mut frame, u, t),
            4 | 5 => push_court(&mut frame, u, t),
            1 | 2 if !u.dead => push_minion(&mut frame, u, t),
            0 | 3 if !u.dead => {
                let mine = u.k == 0 && input.my_slot == Some(u.slot);
                push_champion(&mut frame, u, t, wear_of(u.id, input.buffs), mine, crate::combat::attack_pose(u, input.fx));
            }
            _ => {}
        }
    }
    for u in input.units {
        push_hp_bar(&mut frame, u);
    }
    for f in input.fx {
        let age = if f.life > 0.0 {
            ((f.life - f.left) / f.life).clamp(0.0, 1.0)
        } else {
            0.0
        };
        push_fx(&mut frame, f, age);
    }
    frame
}

// ---------------------------------------------------------------------------
// the showcase (native harness only)
// ---------------------------------------------------------------------------

/// `LEAGUE_SHOWCASE=1` lines the five champions up at mid-lane (x = -6.4,
/// -3.2, 0, 3.2, 6.4 at z = 1, in table order), turning slowly, so a baked
/// mesh can be photographed for sign-off without waiting for a match to
/// field it: `LEAGUE_CAM=0,0` frames the row, `LEAGUE_CAM=-6.4,1@0.3` is a
/// close-up of SW4RM. Baked art where it exists, the procedural body
/// otherwise, a team ring under each. Never on the web.
#[cfg(not(target_arch = "wasm32"))]
fn push_showcase(frame: &mut Frame, _camera: &Camera, t: f32) {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    if !*ON.get_or_init(|| std::env::var("LEAGUE_SHOWCASE").is_ok_and(|v| v != "0" && !v.is_empty())) {
        return;
    }
    let stand_in = |id: u32, k: u8, team: u8, def: u8, x: f32, z: f32, fa: f32| UnitLite {
        id,
        k,
        t: team,
        slot: def,
        def,
        x,
        z,
        fa,
        hp: 80.0,
        mh: 100.0,
        mn: 50.0,
        mm: 100.0,
        dead: false,
        colour: data::CHAMPS[usize::from(def.min(4))].colour,
    };
    for def in 0..5u8 {
        let u = stand_in(9000 + u32::from(def), 0, def % 2, def, (f32::from(def) - 2.0) * 3.2, 1.0, t * 0.6 + f32::from(def) * 0.4);
        push_champion(frame, &u, t, Wear::default(), false, crate::combat::AttackPose::default());
        push_hp_bar(frame, &u);
    }
    // the objectives are units too, and the draft has none: stand them in
    // so the bases and courts can be photographed dressed
    for (i, (k, team, x, z)) in [
        (6u8, 0u8, -data::CORE_X, 0.0),
        (7, 1, data::CORE_X, 0.0),
        (4, 2, data::COURT_POS[0][0], data::COURT_POS[0][1]),
        (5, 2, data::COURT_POS[1][0], data::COURT_POS[1][1]),
    ]
    .into_iter()
    .enumerate()
    {
        let u = stand_in(9100 + i as u32, k, team, 0, x, z, 0.0);
        match k {
            6 | 7 => push_core(frame, &u, t),
            _ => push_court(frame, &u, t),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn push_showcase(_frame: &mut Frame, _camera: &Camera, _t: f32) {}

// ---------------------------------------------------------------------------
// the review camera (native harness only)
// ---------------------------------------------------------------------------

/// `LEAGUE_CAM` moves the camera for a hands-off review client: `x,z` for a
/// fixed focus, `auto` to follow the closest opposing champions, `slot:N`
/// to ride a seat. Never read on the web; a player's camera is their own.
#[cfg(not(target_arch = "wasm32"))]
fn review_camera(input: &SceneInput<'_>) -> Option<Camera> {
    use std::sync::OnceLock;

    #[derive(Clone, Copy)]
    enum Mode {
        Fixed(f32, f32),
        Auto,
        Slot(u8),
    }
    static MODE: OnceLock<Option<(Mode, f32)>> = OnceLock::new();
    // `LEAGUE_CAM=<mode>[@zoom]`: `0,16@0.35` is a close-up of the North
    // Court, `auto@0.6` a tighter fight camera
    let mode = MODE.get_or_init(|| {
        let raw = std::env::var("LEAGUE_CAM").ok()?;
        let raw = raw.trim();
        let (raw, zoom) = raw
            .split_once('@')
            .map_or((raw, 1.0f32), |(m, k)| (m.trim(), k.trim().parse::<f32>().unwrap_or(1.0)));
        let zoom = if zoom.is_finite() && zoom > 0.05 { zoom } else { 1.0 };
        if raw.eq_ignore_ascii_case("auto") {
            return Some((Mode::Auto, zoom));
        }
        if let Some(n) = raw.strip_prefix("slot:") {
            return n.trim().parse().ok().map(|s| (Mode::Slot(s), zoom));
        }
        let (x, z) = raw.split_once(',')?;
        Some((Mode::Fixed(x.trim().parse().ok()?, z.trim().parse().ok()?), zoom))
    });
    let (mode, zoom) = (*mode)?;
    let focus = match mode {
        Mode::Fixed(x, z) => (x, z),
        Mode::Slot(s) => {
            let u = input.units.iter().find(|u| u.k == 0 && u.slot == s)?;
            (u.x, u.z)
        }
        Mode::Auto => action_focus(input.units)?,
    };
    Some(camera_zoomed(focus, zoom))
}

#[cfg(target_arch = "wasm32")]
fn review_camera(_input: &SceneInput<'_>) -> Option<Camera> {
    None
}

/// Where the fight is: the midpoint of the closest pair of opposing living
/// champions when they are within [`ENGAGE_RANGE`] of each other, else the
/// living champion that has pushed furthest from its own core (the one
/// about to make something happen). Native only, like the review camera
/// that is its one caller.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn action_focus(units: &[UnitLite]) -> Option<(f32, f32)> {
    let champs: Vec<&UnitLite> = units.iter().filter(|u| u.k == 0 && !u.dead).collect();
    let mut best: Option<((f32, f32), f32)> = None;
    for a in &champs {
        for b in &champs {
            if a.t >= b.t {
                continue;
            }
            let d = (a.x - b.x).powi(2) + (a.z - b.z).powi(2);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some(((f32::midpoint(a.x, b.x), f32::midpoint(a.z, b.z)), d));
            }
        }
    }
    if let Some((p, d)) = best
        && d <= ENGAGE_RANGE * ENGAGE_RANGE
    {
        return Some(p);
    }
    let home = |u: &UnitLite| if u.t == 0 { -data::CORE_X } else { data::CORE_X };
    champs
        .iter()
        .max_by(|a, b| (a.x - home(a)).abs().total_cmp(&(b.x - home(b)).abs()))
        .map(|u| (u.x, u.z))
}

/// Two opposing champions this close are a fight worth framing together.
#[cfg(not(target_arch = "wasm32"))]
pub const ENGAGE_RANGE: f32 = 26.0;

// ---------------------------------------------------------------------------
// picking
// ---------------------------------------------------------------------------

/// Project a world point to NDC (-1..1, +y up) through the same camera
/// the frame was drawn with. Clicking an enemy is a projection contest:
/// the unit whose chest lands nearest the cursor is the one you meant.
#[must_use]
pub fn project(camera: &Camera, aspect: f32, x: f32, y: f32, z: f32) -> (f32, f32) {
    let vp = camera.view_proj(aspect);
    let clip = vp * ember_engine::glam::Vec4::new(x, y, z, 1.0);
    let w = clip.w.max(1e-6);
    (clip.x / w, clip.y / w)
}

/// Where the cursor meets the ground plane y=0. `None` when the ray does
/// not point at the floor (aimed at the sky).
#[must_use]
pub fn ground_point(camera: &Camera, aspect: f32, ndc: [f32; 2]) -> Option<(f32, f32)> {
    let eye = camera.eye;
    let fwd = (camera.target - eye).normalize_or_zero();
    let right = fwd.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(fwd);
    let half = (camera.fov_y_deg.to_radians() * 0.5).tan();
    let dir = (fwd + right * (ndc[0] * aspect * half) + up * (ndc[1] * half)).normalize_or_zero();
    if dir.y > -1e-3 {
        return None; // parallel or upward
    }
    let t = -eye.y / dir.y;
    let p = eye + dir * t;
    Some((p.x, p.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: u32, k: u8, t: u8, slot: u8, x: f32, z: f32, def: u8) -> UnitLite {
        UnitLite {
            id,
            k,
            t,
            slot,
            def,
            x,
            z,
            fa: 0.3,
            hp: 70.0,
            mh: 100.0,
            mn: 40.0,
            mm: 100.0,
            dead: false,
            colour: data::CHAMPS[usize::from(def)].colour,
        }
    }

    #[test]
    fn every_mesh_has_vertices_and_normals() {
        let meshes = build_meshes();
        assert!(
            meshes.len() as u32 >= MESH_SPHERE + art::SURFACE_COUNT,
            "the nine procedural meshes come first, then the ground surfaces, then baked art"
        );
        // Surfaces keep their ids and sit on the floor (ids are 1-based:
        // id 1 is meshes[0], the engine cube being id 0)
        for i in MESH_GARDEN..=MESH_COURT {
            let m = &meshes[(i - 1) as usize];
            assert!(m.texture.is_some(), "surface {i} has no 8-bit texture");
            assert!(
                m.vertices.iter().all(|v| v.pos[1].abs() < 1e-6),
                "surface {i} is not on y=0"
            );
            if i == MESH_COURT {
                assert_eq!(
                    m.vertices.len(),
                    48 * 3,
                    "the plaza is one circular medallion"
                );
                assert!(
                    m.vertices
                        .iter()
                        .all(|v| v.uv.iter().all(|uv| (0.0..=1.0).contains(uv))),
                    "a unique plaza must not tile"
                );
                assert!(
                    m.vertices
                        .iter()
                        .all(|v| v.pos[0].hypot(v.pos[2]) <= 0.50001),
                    "the plaza must have unit diameter"
                );
            } else {
                assert_eq!(m.vertices.len(), 6, "surface {i} is not one quad");
                assert!(
                    m.vertices.iter().any(|v| v.uv[0] > 1.5),
                    "surface {i} has no tiled UVs"
                );
            }
        }
        // the props are textured and stand on the ground at prop height
        for i in MESH_OBELISK..=MESH_TREE {
            let m = &meshes[(i - 1) as usize];
            assert!(m.texture.is_some(), "prop {i} has no 8-bit texture");
            let top = m.vertices.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
            assert!((4.0..=8.0).contains(&top), "prop {i} stands {top} tall");
        }
        // every baked part is textured (the loader's silent 16-bit failure
        // would show up here as `None`) and sized like a champion
        for (i, m) in meshes.iter().enumerate().skip(MESH_TREE as usize) {
            assert!(
                m.texture.is_some(),
                "baked mesh {i} has no 8-bit base-colour texture"
            );
            let top = m.vertices.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
            let bottom = m.vertices.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min);
            assert!(
                bottom > -0.05 && (1.0..=2.4).contains(&top),
                "baked mesh {i} stands {bottom}..{top}, not on the floor at champion height"
            );
        }
        for (i, m) in meshes.iter().enumerate() {
            assert!(!m.vertices.is_empty(), "mesh {i} is empty");
            assert_eq!(m.vertices.len() % 3, 0, "mesh {i} is not a triangle list");
            for v in &m.vertices {
                let d = v.normal[0] * v.normal[0]
                    + v.normal[1] * v.normal[1]
                    + v.normal[2] * v.normal[2];
                assert!(
                    (d - 1.0).abs() < 0.01,
                    "mesh {i} has an unnormalized or zero normal"
                );
                assert!(
                    v.pos.iter().all(|c| c.is_finite()),
                    "mesh {i} has a non-finite vertex"
                );
            }
        }
    }

    #[test]
    fn face_puts_local_x_on_the_sim_forward() {
        for yaw in [0.0, 0.7, PI / 2.0, 2.5, -1.1] {
            let f = face(yaw) * Vec3::X;
            assert!((f.x - yaw.cos()).abs() < 1e-5 && (f.z - yaw.sin()).abs() < 1e-5, "yaw {yaw}: {f}");
            assert!(f.y.abs() < 1e-5);
        }
    }

    fn points(instance: &Instance, meshes: &[MeshData]) -> Vec<Vec3> {
        let local = if instance.mesh == 0 {
            [-0.5, 0.5]
                .into_iter()
                .flat_map(|x| {
                    [-0.5, 0.5]
                        .into_iter()
                        .flat_map(move |y| [-0.5, 0.5].into_iter().map(move |z| v3(x, y, z)))
                })
                .collect::<Vec<_>>()
        } else {
            meshes[(instance.mesh - 1) as usize]
                .vertices
                .iter()
                .map(|v| Vec3::from(v.pos))
                .collect()
        };
        local
            .into_iter()
            .map(|p| instance.position + instance.rot * (p * instance.scale))
            .collect()
    }

    #[test]
    fn scenery_preserves_the_walkable_rectangle_and_has_a_bounded_cost() {
        let meshes = build_meshes();
        let mut frame = Frame {
            camera: camera_zoomed((0.0, 0.0), 4.0),
            ..Frame::default()
        };
        push_ground(&mut frame);
        assert!(
            frame.instances.len() <= 500,
            "{} ground instances",
            frame.instances.len()
        );
        let triangles: usize = frame
            .instances
            .iter()
            .map(|i| {
                if i.mesh == 0 {
                    12
                } else {
                    meshes[(i.mesh - 1) as usize].vertices.len() / 3
                }
            })
            .sum();
        assert!(
            triangles <= 100_000,
            "{triangles} static triangles before shadows"
        );
        for instance in &frame.instances {
            for p in points(instance, &meshes) {
                assert!(
                    p.y <= 0.16
                        || p.x.abs() >= league_core::sim::FIELD_X
                        || p.z.abs() >= data::FIELD_Z,
                    "tall decoration enters the walkable rectangle: mesh {} at {p:?}",
                    instance.mesh
                );
            }
        }
        let fountains = frame
            .instances
            .iter()
            .filter(|i| i.mesh == MESH_RING && (i.scale.x - data::FOUNTAIN_R).abs() < 1e-5)
            .count();
        assert_eq!(fountains, 2, "both fountains retain their real radius");
    }

    #[test]
    fn objective_silhouettes_stay_inside_their_authoritative_pick_radii() {
        let meshes = build_meshes();
        for (kind, team, radius) in [(6, 0, 2.4), (7, 1, 2.4), (4, 2, 1.6), (5, 2, 1.6)] {
            let u = unit(1, kind, team, u8::MAX, 0.0, 0.0, 0);
            let mut frame = Frame::default();
            if kind >= 6 {
                push_core(&mut frame, &u, 1.2);
            } else {
                push_court(&mut frame, &u, 1.2);
            }
            assert!(frame.particles.len() <= 5);
            for instance in &frame.instances {
                for p in points(instance, &meshes) {
                    assert!(
                        p.x.hypot(p.z) <= radius + 0.001,
                        "kind {kind} exceeds its {radius}m pick radius at {p:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn bar_tilt_faces_the_camera() {
        let cam = camera_for((3.0, -4.0));
        let toward_eye = (cam.eye - cam.target).normalize();
        let normal = bar_tilt() * Vec3::Z;
        assert!(normal.dot(toward_eye) > 0.999, "{normal} vs {toward_eye}");
    }

    #[test]
    fn ground_point_inverts_project() {
        let cam = camera_for((10.0, 2.0));
        for (x, z) in [(10.0, 2.0), (18.0, -5.0), (-3.0, 9.0), (25.0, 12.0)] {
            for aspect in [16.0 / 9.0, 4.0 / 3.0] {
                let (nx, ny) = project(&cam, aspect, x, 0.0, z);
                let (gx, gz) = ground_point(&cam, aspect, [nx, ny]).expect("a ground point projects back");
                assert!((gx - x).abs() < 0.01 && (gz - z).abs() < 0.01, "({x},{z}) -> ({nx},{ny}) -> ({gx},{gz})");
            }
        }
        // screen right is +x: the lane runs left to right
        let (l, _) = project(&cam, 16.0 / 9.0, 0.0, 0.0, 2.0);
        let (r, _) = project(&cam, 16.0 / 9.0, 20.0, 0.0, 2.0);
        assert!(r > l);
        // and screen up is -z (away from the camera)
        let (_, near) = project(&cam, 16.0 / 9.0, 10.0, 0.0, 8.0);
        let (_, far) = project(&cam, 16.0 / 9.0, 10.0, 0.0, -4.0);
        assert!(far > near);
    }

    #[test]
    fn action_focus_finds_the_closest_opposing_pair() {
        let units = [
            unit(1, 0, 0, 0, -30.0, 0.0, 0),
            unit(2, 0, 0, 1, 4.0, 1.0, 1),
            unit(3, 0, 1, 3, 6.0, -1.0, 2),
            unit(4, 0, 1, 4, 40.0, 0.0, 3),
        ];
        let (x, z) = action_focus(&units).unwrap();
        assert!((x - 5.0).abs() < 1e-5 && z.abs() < 1e-5);
        assert!(action_focus(&[]).is_none());
        // one side only: the champion furthest from its own core
        let one_side = [unit(1, 0, 0, 0, -30.0, 4.0, 0), unit(2, 0, 0, 1, -10.0, 0.0, 1)];
        let (cx, cz) = action_focus(&one_side).unwrap();
        assert!((cx + 10.0).abs() < 1e-5 && cz.abs() < 1e-5);
        // a duel with the two far apart frames the pusher, not empty lane
        let apart = [unit(1, 0, 0, 0, -58.0, 0.0, 0), unit(2, 0, 1, 1, -20.0, 3.0, 1)];
        let (px, pz) = action_focus(&apart).unwrap();
        assert!((px + 20.0).abs() < 1e-5 && (pz - 3.0).abs() < 1e-5);
    }

    #[test]
    fn a_full_frame_draws_every_kind_and_stays_finite() {
        let mut units = vec![
            unit(10, 4, 2, u8::MAX, 0.0, 16.0, 0),
            unit(11, 5, 2, u8::MAX, 0.0, -16.0, 0),
            unit(12, 6, 0, u8::MAX, -62.0, 0.0, 0),
            unit(13, 7, 1, u8::MAX, 62.0, 0.0, 0),
            unit(14, 1, 0, u8::MAX, -5.0, 1.0, 0),
            unit(15, 2, 1, u8::MAX, 5.0, -1.0, 0),
            unit(16, 3, 0, 0, -2.0, 2.0, 0),
        ];
        for def in 0..5u8 {
            units.push(unit(20 + u32::from(def), 0, def % 2, def, f32::from(def) * 3.0, 0.0, def));
        }
        units[1].dead = true; // a taken court
        let buffs: Vec<BuffSnap> = (0..12u8)
            .map(|k| BuffSnap {
                u: 20 + u32::from(k % 5),
                k,
                ttl: 1.0,
                val: 1.0,
            })
            .collect();
        let fx: Vec<FxLite> = (0..13u8)
            .map(|k| FxLite {
                source: 0,
                champ: 255,
                ability: 255,
                k,
                x: 1.0,
                z: 2.0,
                x2: 4.0,
                z2: 3.0,
                v: 2.0,
                life: 0.4,
                left: 0.2,
            })
            .chain(std::iter::once(FxLite {
                source: 0,
                champ: 255,
                ability: 255,
                k: 0,
                x: 1.0,
                z: 1.0,
                x2: 0.0,
                z2: 0.0,
                v: 1.0,
                life: 0.4,
                left: 0.1,
            }))
            .collect();
        let zones: Vec<ZoneLite> = (0..4u8).map(|k| (k, f32::from(k) * 5.0, 3.0, 3.0, 0.5)).collect();
        let projs: Vec<ProjSnap> = (0..4u8)
            .map(|k| ProjSnap {
                champ: 255,
                id: 900 + u32::from(k),
                k,
                t: k % 2,
                x: 2.0,
                z: f32::from(k),
                dx: 0.6,
                dz: 0.8,
            })
            .collect();
        let frame = scene_with(&SceneInput {
            units: &units,
            zones: &zones,
            fx: &fx,
            buffs: &buffs,
            projs: &projs,
            time: 12.5,
            camera: camera_for((0.0, 0.0)),
            my_slot: Some(1),
        });
        assert!(frame.instances.len() > 150, "{} instances", frame.instances.len());
        // every id must be one `build_meshes` registers: the nine procedural
        // meshes plus whatever baked parts are embedded
        let registered = build_meshes().len() as u32;
        for i in &frame.instances {
            assert!(i.position.is_finite() && i.scale.is_finite() && i.color.is_finite(), "{i:?}");
            assert!(i.rot.is_finite() && (i.rot.length() - 1.0).abs() < 1e-3, "{i:?}");
            assert!(i.mesh <= registered, "mesh id {} is not registered ({registered} meshes)", i.mesh);
        }
        // the callers' entry draws the same world without wearables
        let plain = scene(&units, &zones, &fx, 1.0, camera_for((0.0, 0.0)), &projs);
        assert!(plain.instances.len() > 100);
        assert!(plain.instances.len() < frame.instances.len());
    }
}
