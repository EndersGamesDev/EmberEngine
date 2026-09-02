//! Faceted procedural primitives for the nine piece silhouettes, the tile,
//! the target disc and the ring.
//!
//! Everything is a flat triangle list with per-face normals, which is the
//! vertex layout `ember_engine::MeshData` wants (see `fire::meshes`). No
//! textures: the instance colour is the whole look, so one mesh per kind
//! serves all four seats.
//!
//! Every triangle goes through `tri`, which computes the geometric normal
//! and winds the face so that normal agrees with an `outward` hint. That is
//! what keeps a low-segment cylinder or sphere lit from the outside without
//! reasoning about winding at every call site; the renderer does not cull
//! back faces, so winding matters for lighting only.

use std::f32::consts::TAU;

use ember_engine::glam::{Quat, Vec3};
use ember_engine::{MeshData, MeshVertex};

/// Segments around a cylinder or sphere: few, so the facets read as style.
const SEGMENTS: usize = 8;

/// Push one triangle with its face normal, wound so the normal points the
/// way `outward` does.
fn tri(out: &mut Vec<MeshVertex>, a: Vec3, b: Vec3, c: Vec3, outward: Vec3) {
    let mut normal = (b - a).cross(c - a).normalize_or_zero();
    let (b, c) = if normal.dot(outward) < 0.0 {
        normal = -normal;
        (c, b)
    } else {
        (b, c)
    };
    if normal == Vec3::ZERO {
        // A degenerate triangle must not render as an unlit black sliver.
        normal = Vec3::Y;
    }
    for p in [a, b, c] {
        out.push(MeshVertex {
            pos: p.to_array(),
            normal: normal.to_array(),
            uv: [0.0, 0.0],
        });
    }
}

/// Two triangles for the quad `a b c d`, both wound toward `outward`.
fn quad(out: &mut Vec<MeshVertex>, a: Vec3, b: Vec3, c: Vec3, d: Vec3, outward: Vec3) {
    tri(out, a, b, c, outward);
    tri(out, a, c, d, outward);
}

/// A box of half extents `half` around `center`, rotated by `rot`.
fn cuboid(out: &mut Vec<MeshVertex>, center: Vec3, half: Vec3, rot: Quat) {
    // (normal, tangent u, tangent v) per face, as the engine's cube table.
    const FACES: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
        (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
    ];
    for (n, u, v) in FACES {
        let face_centre = n * (n.abs() * half).length();
        let hu = (u.abs() * half).length();
        let hv = (v.abs() * half).length();
        let corners = [
            face_centre - u * hu - v * hv,
            face_centre + u * hu - v * hv,
            face_centre + u * hu + v * hv,
            face_centre - u * hu + v * hv,
        ]
        .map(|p| center + rot * p);
        quad(
            out,
            corners[0],
            corners[1],
            corners[2],
            corners[3],
            rot * n,
        );
    }
}

/// A point on a circle of radius `r` at height `y` around `base`.
fn around(base: Vec3, r: f32, y: f32, i: usize) -> Vec3 {
    // At most SEGMENTS + 1 steps: exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let theta = TAU * (i as f32) / (SEGMENTS as f32);
    base + Vec3::new(r * theta.cos(), y, r * theta.sin())
}

/// A capped cylinder, or a cone when `r_top` is 0, standing on `base`
/// (its bottom centre) and `height` tall.
fn frustum(out: &mut Vec<MeshVertex>, base: Vec3, r_bottom: f32, height: f32, r_top: f32) {
    let top_centre = base + Vec3::Y * height;
    for i in 0..SEGMENTS {
        let b0 = around(base, r_bottom, 0.0, i);
        let b1 = around(base, r_bottom, 0.0, i + 1);
        let t0 = around(base, r_top, height, i);
        let t1 = around(base, r_top, height, i + 1);
        let mid = (b0 + b1 + t0 + t1) * 0.25;
        let radial = Vec3::new(mid.x - base.x, 0.0, mid.z - base.z);
        if r_top > 0.0 {
            quad(out, b0, b1, t1, t0, radial);
            tri(out, top_centre, t0, t1, Vec3::Y);
        } else {
            tri(out, b0, b1, top_centre, radial);
        }
        tri(out, base, b0, b1, Vec3::NEG_Y);
    }
}

/// A low-poly sphere.
fn sphere(out: &mut Vec<MeshVertex>, centre: Vec3, r: f32) {
    const RINGS: usize = 4;
    let point = |ring: usize, seg: usize| {
        // At most RINGS + 1 and SEGMENTS + 1 steps: exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let phi = std::f32::consts::PI * (ring as f32) / (RINGS as f32);
        #[allow(clippy::cast_precision_loss)]
        let theta = TAU * (seg as f32) / (SEGMENTS as f32);
        centre + Vec3::new(r * phi.sin() * theta.cos(), r * phi.cos(), r * phi.sin() * theta.sin())
    };
    for ring in 0..RINGS {
        for seg in 0..SEGMENTS {
            let p00 = point(ring, seg);
            let p01 = point(ring, seg + 1);
            let p10 = point(ring + 1, seg);
            let p11 = point(ring + 1, seg + 1);
            let outward = (p00 + p01 + p10 + p11) * 0.25 - centre;
            if ring == 0 {
                tri(out, p00, p11, p10, outward);
            } else if ring == RINGS - 1 {
                tri(out, p00, p01, p11, outward);
            } else {
                quad(out, p00, p01, p11, p10, outward);
            }
        }
    }
}

/// A flat annulus between `r_in` and `r_out`, from `y0` up to `y1`.
fn annulus(out: &mut Vec<MeshVertex>, r_in: f32, r_out: f32, y0: f32, y1: f32) {
    let origin = Vec3::ZERO;
    for i in 0..SEGMENTS {
        let i0 = around(origin, r_in, y0, i);
        let i1 = around(origin, r_in, y0, i + 1);
        let o0 = around(origin, r_out, y0, i);
        let o1 = around(origin, r_out, y0, i + 1);
        let lift = Vec3::Y * (y1 - y0);
        let radial = Vec3::new((o0.x + o1.x) * 0.5, 0.0, (o0.z + o1.z) * 0.5);
        // Top, bottom, outer wall, inner wall.
        quad(out, i0 + lift, o0 + lift, o1 + lift, i1 + lift, Vec3::Y);
        quad(out, i0, o0, o1, i1, Vec3::NEG_Y);
        quad(out, o0, o1, o1 + lift, o0 + lift, radial);
        quad(out, i0, i1, i1 + lift, i0 + lift, -radial);
    }
}

fn finish(vertices: Vec<MeshVertex>) -> MeshData {
    MeshData {
        vertices,
        texture: None,
    }
}

/// The board tile: a flat slab whose top face is at `y = 0`, a hair under
/// one unit wide so the seams between tiles read.
#[must_use]
pub fn tile() -> MeshData {
    let mut v = Vec::new();
    cuboid(
        &mut v,
        Vec3::new(0.0, -0.06, 0.0),
        Vec3::new(0.49, 0.06, 0.49),
        Quat::IDENTITY,
    );
    finish(v)
}

/// The legal-target disc, lying just above the tile.
#[must_use]
pub fn disc() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::new(0.0, 0.02, 0.0), 0.28, 0.04, 0.28);
    finish(v)
}

/// The selection and cursor ring, lying just above the tile.
#[must_use]
pub fn ring() -> MeshData {
    let mut v = Vec::new();
    annulus(&mut v, 0.37, 0.46, 0.02, 0.07);
    finish(v)
}

/// King: a tall cylinder with a cross on top.
#[must_use]
pub fn king() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.22, 0.72, 0.17);
    cuboid(
        &mut v,
        Vec3::new(0.0, 0.88, 0.0),
        Vec3::new(0.04, 0.16, 0.04),
        Quat::IDENTITY,
    );
    cuboid(
        &mut v,
        Vec3::new(0.0, 0.9, 0.0),
        Vec3::new(0.12, 0.035, 0.035),
        Quat::IDENTITY,
    );
    finish(v)
}

/// Queen: a tall cylinder with a sphere on top.
#[must_use]
pub fn queen() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.22, 0.7, 0.15);
    sphere(&mut v, Vec3::new(0.0, 0.83, 0.0), 0.14);
    finish(v)
}

/// Rook: a cylinder with a crenellated top.
#[must_use]
pub fn rook() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.24, 0.55, 0.21);
    frustum(&mut v, Vec3::new(0.0, 0.55, 0.0), 0.25, 0.06, 0.25);
    for (dx, dz) in [(0.16, 0.0), (-0.16, 0.0), (0.0, 0.16), (0.0, -0.16)] {
        cuboid(
            &mut v,
            Vec3::new(dx, 0.69, dz),
            Vec3::new(0.06, 0.08, 0.06),
            Quat::IDENTITY,
        );
    }
    finish(v)
}

/// Bishop: a cone with a small sphere on the point.
#[must_use]
pub fn bishop() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.24, 0.72, 0.0);
    sphere(&mut v, Vec3::new(0.0, 0.76, 0.0), 0.08);
    finish(v)
}

/// Knight: a short base with an angled box head leaning forward (+X, the
/// owner's forward once the instance is yawed by seat).
#[must_use]
pub fn knight() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.22, 0.3, 0.2);
    cuboid(
        &mut v,
        Vec3::new(0.03, 0.5, 0.0),
        Vec3::new(0.1, 0.22, 0.13),
        Quat::from_rotation_z(-0.35),
    );
    cuboid(
        &mut v,
        Vec3::new(0.17, 0.74, 0.0),
        Vec3::new(0.17, 0.09, 0.1),
        Quat::from_rotation_z(-0.2),
    );
    finish(v)
}

/// Pawn: a short cylinder with a sphere on top.
#[must_use]
pub fn pawn() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.2, 0.36, 0.15);
    sphere(&mut v, Vec3::new(0.0, 0.46, 0.0), 0.13);
    finish(v)
}

/// Joker: a cylinder with a three-pointed cap.
#[must_use]
pub fn joker() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.2, 0.55, 0.16);
    for k in 0..3u8 {
        let angle = TAU * f32::from(k) / 3.0;
        let base = Vec3::new(0.1 * angle.cos(), 0.55, 0.1 * angle.sin());
        frustum(&mut v, base, 0.08, 0.26, 0.0);
    }
    finish(v)
}

/// Hero, dormant: a low slab.
#[must_use]
pub fn hero() -> MeshData {
    let mut v = Vec::new();
    cuboid(
        &mut v,
        Vec3::new(0.0, 0.13, 0.0),
        Vec3::new(0.3, 0.13, 0.3),
        Quat::IDENTITY,
    );
    finish(v)
}

/// Hero, awake: a tall column with a wide top.
#[must_use]
pub fn hero_awake() -> MeshData {
    let mut v = Vec::new();
    frustum(&mut v, Vec3::ZERO, 0.19, 0.9, 0.16);
    frustum(&mut v, Vec3::new(0.0, 0.9, 0.0), 0.32, 0.12, 0.3);
    finish(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, m: &MeshData) {
        assert!(!m.vertices.is_empty(), "{name}: empty");
        assert_eq!(m.vertices.len() % 3, 0, "{name}: not a triangle list");
        assert!(m.texture.is_none(), "{name}: untextured by design");
        for v in &m.vertices {
            let n = Vec3::from(v.normal);
            assert!(n.is_finite(), "{name}: non-finite normal {n}");
            assert!((n.length() - 1.0).abs() < 1e-4, "{name}: normal not unit: {n}");
            assert!(Vec3::from(v.pos).is_finite(), "{name}: non-finite position");
        }
    }

    #[test]
    fn every_primitive_is_a_lit_triangle_list() {
        for (name, m) in [
            ("tile", tile()),
            ("disc", disc()),
            ("ring", ring()),
            ("king", king()),
            ("queen", queen()),
            ("rook", rook()),
            ("bishop", bishop()),
            ("knight", knight()),
            ("pawn", pawn()),
            ("joker", joker()),
            ("hero", hero()),
            ("hero_awake", hero_awake()),
        ] {
            check(name, &m);
        }
    }

    /// The winding fix in `tri`: a cylinder's side normals point away from
    /// its axis, its caps up and down, whichever way the points were listed.
    #[test]
    fn normals_point_outward() {
        let mut v = Vec::new();
        frustum(&mut v, Vec3::ZERO, 0.5, 1.0, 0.5);
        for face in v.chunks(3) {
            let n = Vec3::from(face[0].normal);
            let centroid = face.iter().map(|p| Vec3::from(p.pos)).sum::<Vec3>() / 3.0;
            if n.y.abs() > 0.99 {
                // A cap: up on the top, down on the bottom.
                assert!((n.y > 0.0) == (centroid.y > 0.5), "cap {n} at {centroid}");
            } else {
                let radial = Vec3::new(centroid.x, 0.0, centroid.z);
                assert!(n.dot(radial) > 0.0, "side normal {n} points in at {centroid}");
            }
        }
        let mut s = Vec::new();
        sphere(&mut s, Vec3::new(1.0, 2.0, 3.0), 0.5);
        for face in s.chunks(3) {
            let n = Vec3::from(face[0].normal);
            let centroid = face.iter().map(|p| Vec3::from(p.pos)).sum::<Vec3>() / 3.0;
            assert!(
                n.dot(centroid - Vec3::new(1.0, 2.0, 3.0)) > 0.0,
                "sphere normal {n} points in"
            );
        }
    }

    /// A box's face normals are the six axes, rotated with the box.
    #[test]
    fn a_rotated_box_keeps_axis_normals() {
        let mut v = Vec::new();
        let rot = Quat::from_rotation_z(0.3);
        cuboid(&mut v, Vec3::ZERO, Vec3::new(0.2, 0.4, 0.1), rot);
        assert_eq!(v.len(), 36);
        for face in v.chunks(3) {
            let n = rot.inverse() * Vec3::from(face[0].normal);
            let axis = n.abs().max_element();
            assert!((axis - 1.0).abs() < 1e-4, "normal {n} is not an axis");
        }
    }

    #[test]
    fn the_tile_top_is_at_zero_and_the_pieces_stand_on_it() {
        let top = tile()
            .vertices
            .iter()
            .map(|v| v.pos[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(top.abs() < 1e-6, "tile top at {top}");
        for (name, m) in [("king", king()), ("pawn", pawn()), ("hero", hero())] {
            let bottom = m
                .vertices
                .iter()
                .map(|v| v.pos[1])
                .fold(f32::INFINITY, f32::min);
            assert!(bottom.abs() < 1e-6, "{name} bottom at {bottom}");
        }
    }
}
