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

use ember_engine::glam::{Quat, Vec3};
use ember_engine::{Camera, Fog, Frame, Instance, MeshData, MeshVertex};
use league_core::data;
use league_core::proto::{BuffSnap, ProjSnap};

use crate::world::{FxLite, UnitLite, ZoneLite};

pub const MESH_PLANE: u32 = 1;
/// A capped cylinder, radius 1, from y=-1 to y=1.
pub const MESH_FRUSTUM: u32 = 2;
pub const MESH_OCTA: u32 = 3;
/// A thin disc, radius 1, at y=0.
pub const MESH_DISC: u32 = 4;
/// A cone, base radius 1 at y=-1, apex at y=1.
pub const MESH_CONE: u32 = 5;
/// A flat annulus, outer radius 1, inner 0.78, at y=0.
pub const MESH_RING: u32 = 6;
/// A tapered blade from x=0 to x=1, thin in y, narrow in z.
pub const MESH_BLADE: u32 = 7;
/// A toothed wheel, radius 1, axis Y, thickness 0.3.
pub const MESH_GEAR: u32 = 8;
/// A low-poly sphere, radius 1.
pub const MESH_SPHERE: u32 = 9;

/// Camera height over the focus, its offset toward +Z and the vertical
/// field of view. The pitch these give is what [`bar_tilt`] faces.
pub const CAM_HEIGHT: f32 = 30.0;
pub const CAM_BACK: f32 = 14.0;
pub const CAM_FOV: f32 = 40.0;

/// The camera sits high and behind the focus, looking along -Z.
#[must_use]
pub const fn camera_for(focus: (f32, f32)) -> Camera {
    let (x, z) = focus;
    Camera {
        eye: Vec3::new(x, CAM_HEIGHT, z + CAM_BACK),
        target: Vec3::new(x, 0.0, z),
        fov_y_deg: CAM_FOV,
    }
}

/// Team tint: blue side, red side. The page's scoreboard uses the same
/// values divided by 255 so the two agree.
#[must_use]
pub const fn team_colour(team: u8) -> [f32; 3] {
    match team {
        0 => [0.42, 0.62, 1.0],
        1 => [1.0, 0.42, 0.38],
        _ => [0.75, 0.72, 0.55], // courts belong to nobody
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

/// The whole mesh set, in registration order.
#[must_use]
pub fn build_meshes() -> Vec<MeshData> {
    vec![
        plane_mesh(),
        frustum_mesh(),
        octa_mesh(),
        disc_mesh(),
        cone_mesh(),
        ring_mesh(),
        blade_mesh(),
        gear_mesh(),
        sphere_mesh(),
    ]
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
    let mut vertices = Vec::with_capacity(SEG * 12);
    let h = 0.04;
    for i in 0..SEG {
        let a0 = around(0.0, 0.0, 1.0, -h, i, SEG);
        let a1 = around(0.0, 0.0, 1.0, -h, i + 1, SEG);
        let b0 = around(0.0, 0.0, 1.0, h, i, SEG);
        let b1 = around(0.0, 0.0, 1.0, h, i + 1, SEG);
        let n = rim_normal(a0, a1);
        push_tri(&mut vertices, a0, a1, b1, n);
        push_tri(&mut vertices, a0, b1, b0, n);
        push_tri(&mut vertices, [0.0, h, 0.0], b0, b1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, [0.0, -h, 0.0], a1, a0, [0.0, -1.0, 0.0]);
    }
    MeshData { vertices, texture: None }
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

/// A flat annulus with a little thickness, outer radius 1, inner 0.78.
fn ring_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(SEG * 24);
    let (ro, ri, h) = (1.0, 0.78, 0.05);
    for i in 0..SEG {
        let o0 = around(0.0, 0.0, ro, h, i, SEG);
        let o1 = around(0.0, 0.0, ro, h, i + 1, SEG);
        let i0 = around(0.0, 0.0, ri, h, i, SEG);
        let i1 = around(0.0, 0.0, ri, h, i + 1, SEG);
        let ob0 = around(0.0, 0.0, ro, -h, i, SEG);
        let ob1 = around(0.0, 0.0, ro, -h, i + 1, SEG);
        let ib0 = around(0.0, 0.0, ri, -h, i, SEG);
        let ib1 = around(0.0, 0.0, ri, -h, i + 1, SEG);
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
    MeshData { vertices, texture: None }
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

/// Static ground geometry: the dark field, the lit lane, its edge lines and
/// distance marks, the two base plazas, the court yards and the walls.
fn push_ground(frame: &mut Frame) {
    let field = [0.07, 0.11, 0.07];
    let lane = [0.17, 0.17, 0.11];
    let paint = [0.36, 0.34, 0.24];
    let stone = [0.24, 0.24, 0.28];
    frame.instances.push(
        ins(v3(0.0, -0.05, 0.0), v3(160.0, 1.0, 92.0), field).with_mesh(MESH_PLANE),
    );
    // the lane corridor and its edges
    frame.instances.push(
        ins(v3(0.0, -0.02, 0.0), v3(2.0 * data::CORE_X + 16.0, 1.0, 2.0 * data::LANE_Z), lane)
            .with_mesh(MESH_PLANE),
    );
    for z in [-data::LANE_Z, data::LANE_Z] {
        frame.instances.push(
            ins(v3(0.0, 0.02, z), v3(2.0 * data::CORE_X - 14.0, 0.05, 0.16), paint).with_mesh(0),
        );
    }
    // distance marks every ten units and the halfway line
    for i in -5i8..=5 {
        if i != 0 {
            let x = f32::from(i) * 10.0;
            frame.instances.push(
                ins(v3(x, 0.01, 0.0), v3(0.45, 1.0, 0.45), scale3(paint, 0.8)).with_mesh(MESH_DISC),
            );
        }
    }
    frame.instances.push(
        ins(v3(0.0, 0.02, 0.0), v3(0.18, 0.05, 2.0 * data::LANE_Z), paint).with_mesh(0),
    );
    // base plazas: a paved disc and a ring in the owner's colour
    for team in 0..2u8 {
        let cx = if team == 0 { -data::CORE_X } else { data::CORE_X };
        let tint = mix(stone, team_colour(team), 0.25);
        frame.instances.push(
            ins(v3(cx, 0.0, 0.0), v3(data::FOUNTAIN_R + 0.5, 1.0, data::FOUNTAIN_R + 0.5), tint)
                .with_mesh(MESH_DISC),
        );
        frame.instances.push(
            ins(v3(cx, 0.03, 0.0), v3(data::FOUNTAIN_R + 0.5, 1.0, data::FOUNTAIN_R + 0.5), team_colour(team))
                .with_mesh(MESH_RING),
        );
        // the gate pillars where the lane leaves the base
        let gx = cx - cx.signum() * 10.0;
        for z in [-data::LANE_Z - 0.9, data::LANE_Z + 0.9] {
            frame.instances.push(
                ins(v3(gx, 1.5, z), v3(0.5, 1.5, 0.5), stone).with_mesh(MESH_FRUSTUM),
            );
            frame.instances.push(
                ins(v3(gx, 3.2, z), v3(0.45, 0.45, 0.45), team_colour(team)).with_mesh(MESH_OCTA),
            );
        }
    }
    // court yards: a paved square, a path from the lane and four posts
    for c in data::COURT_POS {
        let [cx, cz] = c;
        frame.instances.push(
            ins(v3(cx, -0.01, cz), v3(11.0, 1.0, 11.0), mix(lane, stone, 0.5)).with_mesh(MESH_PLANE),
        );
        let path_len = cz.abs() - data::LANE_Z - 5.5;
        frame.instances.push(
            ins(v3(cx, -0.015, cz.signum() * (data::LANE_Z + path_len / 2.0)), v3(3.0, 1.0, path_len), mix(lane, stone, 0.5))
                .with_mesh(MESH_PLANE),
        );
        for (dx, dz) in [(-4.6, -4.6), (4.6, -4.6), (-4.6, 4.6), (4.6, 4.6)] {
            frame.instances.push(
                ins(v3(cx + dx, 1.1, cz + dz), v3(0.32, 1.1, 0.32), stone).with_mesh(MESH_FRUSTUM),
            );
        }
    }
    // walls at the field edge, so the dark field ends somewhere
    let wall = [0.11, 0.10, 0.17];
    for z in [-data::FIELD_Z, data::FIELD_Z] {
        frame.instances.push(ins(v3(0.0, 0.6, z), v3(160.0, 1.2, 1.6), wall).with_mesh(0));
    }
    for x in [-70.0, 70.0] {
        frame.instances.push(ins(v3(x, 0.6, 0.0), v3(1.6, 1.2, 2.0 * data::FIELD_Z), wall).with_mesh(0));
    }
    // scattered rocks off the lane, fixed positions, for a sense of scale
    let rock = [0.14, 0.15, 0.14];
    for (i, (x, z, s)) in [
        (-38.0, 24.0, 1.4),
        (-22.0, -27.0, 1.1),
        (-9.0, 30.0, 1.8),
        (14.0, -31.0, 1.3),
        (27.0, 25.0, 1.0),
        (41.0, -22.0, 1.6),
        (-47.0, -30.0, 1.2),
        (49.0, 31.0, 1.5),
        (-30.0, -14.0, 0.8),
        (33.0, 12.0, 0.9),
    ]
    .into_iter()
    .enumerate()
    {
        let yaw = i as f32 * 0.7;
        frame.instances.push(
            ins(v3(x, s * 0.5, z), v3(s, s * 0.6, s * 0.8), rock).with_yaw(yaw).with_mesh(MESH_OCTA),
        );
    }
}

// ---------------------------------------------------------------------------
// objectives
// ---------------------------------------------------------------------------

fn push_core(frame: &mut Frame, u: &UnitLite, t: f32) {
    let col = if u.dead { [0.18, 0.16, 0.16] } else { team_colour(u.t) };
    let stone = [0.30, 0.30, 0.42];
    let bob = if u.dead { -1.2 } else { (t * 1.4).sin() * 0.25 };
    let spin = if u.dead { 0.0 } else { t * 0.6 };
    frame.instances.push(ins(v3(u.x, 0.4, u.z), v3(2.8, 0.4, 2.8), stone).with_mesh(MESH_FRUSTUM));
    frame.instances.push(
        ins(v3(u.x, 2.9 + bob, u.z), v3(1.6, 2.4, 1.6), col)
            .with_rot(Quat::from_rotation_y(spin))
            .with_mesh(MESH_OCTA),
    );
    // the nexus frame: four pillars around the crystal
    for i in 0..4 {
        let a = TAU * (i as f32) / 4.0 + PI / 4.0;
        frame.instances.push(
            ins(v3(u.x + a.cos() * 3.6, 1.4, u.z + a.sin() * 3.6), v3(0.55, 2.8, 0.55), stone).with_mesh(0),
        );
        frame.instances.push(
            ins(v3(u.x + a.cos() * 3.6, 3.1, u.z + a.sin() * 3.6), v3(0.4, 0.4, 0.4), col).with_mesh(MESH_OCTA),
        );
    }
    if !u.dead {
        frame.instances.push(ins(v3(u.x, 0.06, u.z), v3(5.5, 1.0, 5.5), col).with_mesh(MESH_RING));
    }
}

fn push_court(frame: &mut Frame, u: &UnitLite, t: f32) {
    let stone = [0.34, 0.33, 0.30];
    if u.dead {
        // a taken court: a broken stump and a dark ring, waiting to respawn
        frame.instances.push(ins(v3(u.x, 0.5, u.z), v3(1.15, 0.5, 1.15), [0.22, 0.20, 0.20]).with_mesh(MESH_FRUSTUM));
        frame.instances.push(ins(v3(u.x, 0.06, u.z), v3(3.2, 1.0, 3.2), [0.28, 0.26, 0.2]).with_mesh(MESH_RING));
        return;
    }
    let pulse = 0.85 + 0.15 * (t * 2.0).sin();
    let gem = [0.95 * pulse, 0.88 * pulse, 0.55];
    frame.instances.push(ins(v3(u.x, 2.6, u.z), v3(1.0, 2.6, 1.0), stone).with_mesh(MESH_FRUSTUM));
    frame.instances.push(ins(v3(u.x, 5.35, u.z), v3(1.3, 0.15, 1.3), stone).with_mesh(MESH_FRUSTUM));
    frame.instances.push(
        ins(v3(u.x, 6.3, u.z), v3(0.8, 0.8, 0.8), gem)
            .with_rot(Quat::from_rotation_y(t * 0.9))
            .with_mesh(MESH_OCTA),
    );
    frame.instances.push(ins(v3(u.x, 0.06, u.z), v3(3.2, 1.0, 3.2), team_colour(2)).with_mesh(MESH_RING));
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
fn push_champion(frame: &mut Frame, u: &UnitLite, t: f32, wear: Wear, mine: bool) {
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
    // is doubled so the eye finds it in a brawl
    frame.instances.push(ins(v3(x, 0.05, z), v3(1.15 * wob, 1.0, 1.15 * wob), team).with_mesh(MESH_RING));
    if mine {
        frame.instances.push(ins(v3(x, 0.04, z), v3(1.45, 1.0, 1.45), [0.95, 0.95, 0.9]).with_mesh(MESH_RING));
    }
    if wear.slow {
        frame.instances.push(ins(v3(x, 0.07, z), v3(0.85, 1.0, 0.85), [0.55, 0.8, 1.0]).with_mesh(MESH_DISC));
    }

    let y = bob;
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

    // wearables shared by every kit
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
        0 | 3 => (1.4, 2.55),
        4 | 5 => (2.4, 7.2),
        6 | 7 => (3.0, 5.9),
        _ => (0.7, 1.8),
    };
    let tilt = bar_tilt();
    let h = if objective { 0.16 } else { 0.12 };
    frame.instances.push(
        ins(v3(u.x, y, u.z), v3(w + 0.08, h + 0.06, 0.03), [0.04, 0.04, 0.05]).with_rot(tilt).with_mesh(0),
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
        ins(v3(u.x - w * (1.0 - frac) / 2.0, y, u.z), v3(w * frac, h, 0.035), col).with_rot(tilt).with_mesh(0),
    );
    if (u.k == 0 || u.k == 3) && u.mm > 0.0 {
        let mf = (u.mn / u.mm).clamp(0.0, 1.0);
        // the mana sliver hangs just under the bar, in the card's own plane
        let down = tilt * Vec3::new(0.0, -0.13, 0.0);
        frame.instances.push(
            ins(v3(u.x - w * (1.0 - mf) / 2.0, y, u.z) + down, v3(w * mf, 0.05, 0.035), [0.35, 0.55, 1.0]).with_rot(tilt).with_mesh(0),
        );
    }
}

// ---------------------------------------------------------------------------
// shots, zones, effects
// ---------------------------------------------------------------------------

/// A projectile in flight. `k` is `sim::ProjKind` as the wire numbers it:
/// 0 auto-attack, 1 drone, 2 gear bolt, 3 hook.
fn push_proj(frame: &mut Frame, p: &ProjSnap, t: f32) {
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
    let (zk, x, z, r, spin) = *zone;
    match zk {
        0 => {
            // a tornado: a funnel of three spinning tiers over a scorched disc
            for (i, (rr, h)) in [(0.35, 0.5), (0.6, 1.4), (0.9, 2.3)].into_iter().enumerate() {
                frame.instances.push(
                    ins(v3(x, h, z), v3(r * rr, 0.45, r * rr), [1.0, 0.45 + 0.1 * i as f32, 0.12])
                        .with_rot(Quat::from_rotation_y(spin * (1.0 + i as f32 * 0.5)))
                        .with_mesh(MESH_GEAR),
                );
            }
            frame.instances.push(ins(v3(x, 0.07, z), v3(r, 1.0, r), [0.55, 0.2, 0.05]).with_mesh(MESH_DISC));
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

/// Test helper for a scene without buffs or an own-seat marker.
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
            color: [0.015, 0.02, 0.03],
            density: 0.0025,
        },
        ..Frame::default()
    };
    push_ground(&mut frame);
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
                push_champion(&mut frame, u, t, wear_of(u.id, input.buffs), mine);
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
    static MODE: OnceLock<Option<Mode>> = OnceLock::new();
    let mode = MODE.get_or_init(|| {
        let raw = std::env::var("LEAGUE_CAM").ok()?;
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("auto") {
            return Some(Mode::Auto);
        }
        if let Some(n) = raw.strip_prefix("slot:") {
            return n.trim().parse().ok().map(Mode::Slot);
        }
        let (x, z) = raw.split_once(',')?;
        Some(Mode::Fixed(x.trim().parse().ok()?, z.trim().parse().ok()?))
    });
    let focus = match (*mode)? {
        Mode::Fixed(x, z) => (x, z),
        Mode::Slot(s) => {
            let u = input.units.iter().find(|u| u.k == 0 && u.slot == s)?;
            (u.x, u.z)
        }
        Mode::Auto => action_focus(input.units)?,
    };
    Some(camera_for(focus))
}

#[cfg(target_arch = "wasm32")]
fn review_camera(_input: &SceneInput<'_>) -> Option<Camera> {
    None
}

/// Where the fight is: the midpoint of the closest pair of opposing living
/// champions, else the centroid of every living champion. Native only,
/// like the review camera that is its one caller.
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
    if let Some((p, _)) = best {
        return Some(p);
    }
    if champs.is_empty() {
        return None;
    }
    let n = champs.len() as f32;
    Some((
        champs.iter().map(|u| u.x).sum::<f32>() / n,
        champs.iter().map(|u| u.z).sum::<f32>() / n,
    ))
}

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
        assert_eq!(meshes.len() as u32, MESH_SPHERE, "the last id names the count");
        for (i, m) in meshes.iter().enumerate() {
            assert!(!m.vertices.is_empty(), "mesh {i} is empty");
            assert_eq!(m.vertices.len() % 3, 0, "mesh {i} is not a triangle list");
            for v in &m.vertices {
                let d = v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2];
                assert!((d - 1.0).abs() < 0.01, "mesh {i} has an unnormalized or zero normal");
                assert!(v.pos.iter().all(|c| c.is_finite()), "mesh {i} has a non-finite vertex");
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
        let one_side = [unit(1, 0, 0, 0, -30.0, 4.0, 0), unit(2, 0, 0, 1, -10.0, 0.0, 1)];
        let (cx, cz) = action_focus(&one_side).unwrap();
        assert!((cx + 20.0).abs() < 1e-5 && (cz - 2.0).abs() < 1e-5);
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
        for i in &frame.instances {
            assert!(i.position.is_finite() && i.scale.is_finite() && i.color.is_finite(), "{i:?}");
            assert!(i.rot.is_finite() && (i.rot.length() - 1.0).abs() < 1e-3, "{i:?}");
            assert!(i.mesh <= MESH_SPHERE, "mesh id {} is not registered", i.mesh);
        }
        // the callers' entry draws the same world without wearables
        let plain = scene(&units, &zones, &fx, 1.0, camera_for((0.0, 0.0)), &projs);
        assert!(plain.instances.len() > 100);
        assert!(plain.instances.len() < frame.instances.len());
    }
}
