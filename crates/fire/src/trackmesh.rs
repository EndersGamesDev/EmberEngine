//! Building the road, kerbs and courtyard walls as continuous ribbons.
//!
//! The obvious approach — one box instance per centreline segment — costs
//! three hundred instances and still splits open on the outside of every
//! corner, because a box cannot taper. A ribbon stitched straight from the
//! centreline has neither problem: it is one mesh, it follows the curve
//! exactly, and its UVs can run continuously along the lap so the cobbles do
//! not restart every few metres.

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{MeshData, MeshVertex, TextureData};
use fire_core::track::Track;

/// Left-hand normal of a direction on the XZ plane, matching
/// `Track::locate`'s sign convention (positive lateral is to the left).
fn left_of(t: Vec2) -> Vec2 {
    Vec2::new(-t.y, t.x)
}

fn vert(p: Vec3, n: Vec3, u: f32, v: f32) -> MeshVertex {
    MeshVertex { pos: p.to_array(), normal: n.to_array(), uv: [u, v] }
}

/// Walk the centreline, handing each segment's start/end centre, tangent and
/// arc length to `f`. Shared by every ribbon below so they stay in lockstep.
fn for_each_segment(track: &Track, mut f: impl FnMut(Vec2, Vec2, Vec2, Vec2, f32, f32)) {
    let pts = track.centreline();
    let n = pts.len();
    let mut s = 0.0;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let seg = (b - a).length();
        if seg < 1e-6 {
            continue;
        }
        let dir = (b - a) / seg;
        // Average with the neighbouring segment so the ribbon's edges meet
        // cleanly on a curve rather than fanning at each joint.
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 2) % n];
        let ta = ((a - prev).normalize_or_zero() + dir).normalize_or_zero();
        let tb = (dir + (next - b).normalize_or_zero()).normalize_or_zero();
        f(a, b, if ta == Vec2::ZERO { dir } else { ta }, if tb == Vec2::ZERO { dir } else { tb }, s, s + seg);
        s += seg;
    }
}

/// A flat horizontal ribbon centred `offset` metres left of the centreline,
/// `width` wide, at height `y`. Used for the road and the kerbs.
///
/// `tile_len` is how many metres of track one texture repeat covers along the
/// lap; `tile_across` how many repeats span the width.
pub fn flat_ribbon(
    track: &Track,
    offset: f32,
    width: f32,
    y: f32,
    tile_len: f32,
    tile_across: f32,
    texture: Option<TextureData>,
) -> MeshData {
    let mut vertices = Vec::new();
    let up = Vec3::Y;
    for_each_segment(track, |a, b, ta, tb, s0, s1| {
        let (la, lb) = (left_of(ta), left_of(tb));
        let (a_in, a_out) = (a + la * (offset - width * 0.5), a + la * (offset + width * 0.5));
        let (b_in, b_out) = (b + lb * (offset - width * 0.5), b + lb * (offset + width * 0.5));
        let p = |q: Vec2| Vec3::new(q.x, y, q.y);
        let (v0, v1) = (s0 / tile_len, s1 / tile_len);
        let quad = [
            (p(a_in), 0.0, v0),
            (p(b_in), 0.0, v1),
            (p(b_out), tile_across, v1),
            (p(a_in), 0.0, v0),
            (p(b_out), tile_across, v1),
            (p(a_out), tile_across, v0),
        ];
        for (pos, u, v) in quad {
            vertices.push(vert(pos, up, u, v));
        }
    });
    MeshData { vertices, texture }
}

/// A vertical wall standing `offset` metres left of the centreline (negative
/// for the right-hand side), `height` metres tall.
///
/// The renderer does not cull backfaces, so a single-sided wall is visible
/// from both sides and costs half the triangles of a boxed one. The normal
/// faces the track, which is the side that matters for lighting.
pub fn wall_ribbon(
    track: &Track,
    offset: f32,
    height: f32,
    tile_len: f32,
    tile_up: f32,
    texture: Option<TextureData>,
) -> MeshData {
    let mut vertices = Vec::new();
    let inward = if offset >= 0.0 { -1.0 } else { 1.0 };
    for_each_segment(track, |a, b, ta, tb, s0, s1| {
        let (la, lb) = (left_of(ta), left_of(tb));
        let (fa, fb) = (a + la * offset, b + lb * offset);
        // Face the racing line.
        let n2 = left_of((fb - fa).normalize_or_zero()) * inward;
        let n = Vec3::new(n2.x, 0.0, n2.y);
        let lo = |q: Vec2| Vec3::new(q.x, 0.0, q.y);
        let hi = |q: Vec2| Vec3::new(q.x, height, q.y);
        let (v0, v1) = (s0 / tile_len, s1 / tile_len);
        let quad = [
            (lo(fa), v0, 0.0),
            (lo(fb), v1, 0.0),
            (hi(fb), v1, tile_up),
            (lo(fa), v0, 0.0),
            (hi(fb), v1, tile_up),
            (hi(fa), v0, tile_up),
        ];
        for (pos, u, v) in quad {
            vertices.push(vert(pos, n, u, v));
        }
    });
    MeshData { vertices, texture }
}

/// A band across the track at arc length `s`, `depth` metres long — the
/// start/finish line.
pub fn cross_band(
    track: &Track,
    s: f32,
    depth: f32,
    tiles: f32,
    texture: Option<TextureData>,
) -> MeshData {
    let (centre, tangent) = track.at(s);
    let l = left_of(tangent);
    let half = track.half_width();
    let p = |along: f32, across: f32| {
        let q = centre + tangent * along + l * across;
        Vec3::new(q.x, 0.02, q.y)
    };
    let up = Vec3::Y;
    let (d, h) = (depth * 0.5, half);
    let corners = [
        (p(-d, -h), 0.0, 0.0),
        (p(d, -h), 0.0, 1.0),
        (p(d, h), tiles, 1.0),
        (p(-d, -h), 0.0, 0.0),
        (p(d, h), tiles, 1.0),
        (p(-d, h), tiles, 0.0),
    ];
    MeshData {
        vertices: corners.into_iter().map(|(q, u, v)| vert(q, up, u, v)).collect(),
        texture,
    }
}

/// The courtyard floor: one big quad under everything, sized to the track's
/// bounding box plus a margin.
pub fn ground(track: &Track, margin: f32, tile: f32, texture: Option<TextureData>) -> MeshData {
    let mut lo = Vec2::splat(f32::INFINITY);
    let mut hi = Vec2::splat(f32::NEG_INFINITY);
    for p in track.centreline() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    lo -= Vec2::splat(margin);
    hi += Vec2::splat(margin);
    let size = hi - lo;
    let y = -0.05;
    let p = |x: f32, z: f32| Vec3::new(x, y, z);
    let tiles = (size / tile).max(Vec2::ONE);
    let quad = [
        (p(lo.x, lo.y), 0.0, 0.0),
        (p(hi.x, lo.y), tiles.x, 0.0),
        (p(hi.x, hi.y), tiles.x, tiles.y),
        (p(lo.x, lo.y), 0.0, 0.0),
        (p(hi.x, hi.y), tiles.x, tiles.y),
        (p(lo.x, hi.y), 0.0, tiles.y),
    ];
    MeshData {
        vertices: quad.into_iter().map(|(q, u, v)| vert(q, Vec3::Y, u, v)).collect(),
        texture,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fire_core::castle;

    #[test]
    fn the_road_is_a_closed_triangle_list_with_sane_normals() {
        let t = castle::track();
        let m = flat_ribbon(&t, 0.0, t.half_width() * 2.0, 0.0, 12.0, 3.0, None);
        assert_eq!(m.vertices.len() % 3, 0, "not a triangle list");
        // Two triangles per centreline segment.
        assert_eq!(m.vertices.len(), t.centreline().len() * 6);
        for v in &m.vertices {
            assert!(Vec3::from(v.pos).is_finite());
            assert!((Vec3::from(v.normal).y - 1.0).abs() < 1e-5, "road normal not up");
        }
    }

    /// Every road vertex must actually be on the road: within half a width of
    /// the centreline, plus a little slack for the mitred joints on corners.
    #[test]
    fn the_road_covers_the_racing_surface_and_no_more() {
        let t = castle::track();
        let m = flat_ribbon(&t, 0.0, t.half_width() * 2.0, 0.0, 12.0, 3.0, None);
        for v in &m.vertices {
            let p = Vec2::new(v.pos[0], v.pos[2]);
            let lat = t.locate(p).lateral.abs();
            assert!(
                lat <= t.half_width() + 1.5,
                "road vertex {lat:.2} m from the line, half width is {:.2}",
                t.half_width()
            );
        }
    }

    /// The v coordinate must climb monotonically around the lap, or the
    /// cobbles visibly restart mid-corner.
    #[test]
    fn road_uvs_run_continuously_along_the_lap() {
        let t = castle::track();
        let m = flat_ribbon(&t, 0.0, 18.0, 0.0, 12.0, 3.0, None);
        let last_v = m.vertices.last().unwrap().uv[1];
        let expect = t.length() / 12.0;
        assert!(
            (last_v - expect).abs() < expect * 0.05,
            "v ends at {last_v:.2}, expected about {expect:.2}"
        );
    }

    #[test]
    fn walls_stand_up_and_face_the_track() {
        let t = castle::track();
        let off = t.half_width() + 6.0;
        for (offset, side) in [(off, "left"), (-off, "right")] {
            let m = wall_ribbon(&t, offset, 5.0, 10.0, 1.0, None);
            assert_eq!(m.vertices.len() % 3, 0);
            let mut saw_top = false;
            for v in &m.vertices {
                let n = Vec3::from(v.normal);
                assert!(n.y.abs() < 1e-5, "{side} wall normal should be horizontal: {n}");
                assert!((n.length() - 1.0).abs() < 1e-3, "{side} wall normal not unit");
                if v.pos[1] > 4.9 {
                    saw_top = true;
                }
            }
            assert!(saw_top, "{side} wall has no height");
            // The normal must point toward the centreline, not away from it.
            let v = &m.vertices[0];
            let p = Vec2::new(v.pos[0], v.pos[2]);
            let n = Vec2::new(v.normal[0], v.normal[2]);
            let to_line = -t.locate(p).lateral.signum();
            let left = left_of(t.locate(p).tangent);
            assert!(
                n.dot(left * to_line) > 0.5,
                "{side} wall faces away from the track"
            );
        }
    }

    #[test]
    fn the_ground_encloses_the_whole_circuit() {
        let t = castle::track();
        let m = ground(&t, 60.0, 8.0, None);
        let xs: Vec<f32> = m.vertices.iter().map(|v| v.pos[0]).collect();
        let zs: Vec<f32> = m.vertices.iter().map(|v| v.pos[2]).collect();
        let (lo_x, hi_x) = (xs.iter().cloned().fold(f32::INFINITY, f32::min), xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max));
        let (lo_z, hi_z) = (zs.iter().cloned().fold(f32::INFINITY, f32::min), zs.iter().cloned().fold(f32::NEG_INFINITY, f32::max));
        for p in t.centreline() {
            assert!(p.x > lo_x && p.x < hi_x && p.y > lo_z && p.y < hi_z, "centreline escapes the ground quad");
        }
    }

    #[test]
    fn the_start_band_sits_across_the_line() {
        let t = castle::track();
        let m = cross_band(&t, 0.0, 3.0, 8.0, None);
        assert_eq!(m.vertices.len(), 6);
        for v in &m.vertices {
            let p = Vec2::new(v.pos[0], v.pos[2]);
            assert!(t.locate(p).lateral.abs() <= t.half_width() + 0.1);
            assert!(t.delta_s(t.locate(p).s, 0.0).abs() < 3.0, "band is not at the line");
        }
    }
}
