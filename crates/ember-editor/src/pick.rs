//! Cursor picking: turning a point on screen into a ray, and a ray into a
//! selected object.
//!
//! The ray is built from `look_at_rh`'s own basis and `perspective_rh`'s own
//! tangents rather than by inverting a view-projection matrix. Two reasons.
//! It agrees with the shader algebraically instead of approximately, and it
//! does not depend on the near/far planes hardcoded inside
//! `Camera::view_proj` — numbers game code cannot read. The equivalence with
//! the inverse is asserted in the tests, at aspects that are not 16:9,
//! because that is the only thing keeping two expressions of one piece of
//! maths from drifting apart.

use ember_engine::glam::{Quat, Vec3};
use ember_engine::{Camera, Instance};

/// A ray through the cursor, in world space.
///
/// `ndc` is `InputState::cursor_ndc()`: x and y in -1..1, y already up-positive.
/// `aspect` is `InputState::aspect()`.
///
/// Returns `(origin, direction)` with `direction` normalized. The origin is
/// the eye, so `t` is distance along the ray in world units.
#[must_use]
pub fn ray_from_cursor(camera: &Camera, aspect: f32, ndc: [f32; 2]) -> (Vec3, Vec3) {
    // The same right-handed basis `look_at_rh` builds: forward is the way we
    // look, right = forward x up, and up is re-derived so a camera pitched
    // toward world-up stays orthonormal.
    let forward = (camera.target - camera.eye).normalize_or_zero();
    let forward = if forward == Vec3::ZERO {
        Vec3::NEG_Z
    } else {
        forward
    };
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    // Looking straight up or down leaves `right` degenerate; any perpendicular
    // will do, and X is as good as another.
    let right = if right == Vec3::ZERO { Vec3::X } else { right };
    let up = right.cross(forward);

    // `perspective_rh` maps a point at NDC y = 1 to tan(fov_y / 2) at unit
    // depth, and x scales that by the aspect.
    let ty = (camera.fov_y_deg.to_radians() * 0.5).tan();
    let tx = ty * aspect;

    let dir = forward + right * (ndc[0] * tx) + up * (ndc[1] * ty);
    (camera.eye, dir.normalize())
}

/// Distance along `dir` at which the ray enters `inst`'s box, or `None`.
///
/// The instance is the engine's unit cube scaled then rotated (that order is
/// the renderer's, not a choice made here), so the test un-rotates the ray
/// into the box's own frame and slab-tests an axis-aligned box of half-extent
/// `scale.abs() * 0.5`. `abs` matters: a negative scale is a legal mirror and
/// would otherwise produce an inside-out slab that never hits.
#[must_use]
pub fn ray_obb(origin: Vec3, dir: Vec3, inst: &Instance) -> Option<f32> {
    ray_box_local(origin, dir, inst.position, inst.rot, inst.scale * 0.5)
}

/// `ray_obb` with the box given directly, for pick proxies that are fatter
/// than the thing they stand for — a gizmo handle needs a grabbable volume
/// several times its drawn thickness.
#[must_use]
pub fn ray_box(origin: Vec3, dir: Vec3, centre: Vec3, rot: Quat, half: Vec3) -> Option<f32> {
    ray_box_local(origin, dir, centre, rot, half)
}

fn ray_box_local(origin: Vec3, dir: Vec3, centre: Vec3, rot: Quat, half: Vec3) -> Option<f32> {
    let inv = rot.conjugate();
    let o = inv * (origin - centre);
    let d = inv * dir;
    let half = half.abs();

    let mut t_near = f32::NEG_INFINITY;
    let mut t_far = f32::INFINITY;
    for a in 0..3 {
        let (o_a, d_a, h_a) = (o[a], d[a], half[a]);
        if d_a.abs() < 1e-8 {
            // Parallel to this slab: a miss only if the origin is outside it.
            if o_a < -h_a || o_a > h_a {
                return None;
            }
            continue;
        }
        let inv_d = 1.0 / d_a;
        let (mut t0, mut t1) = ((-h_a - o_a) * inv_d, (h_a - o_a) * inv_d);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        t_near = t_near.max(t0);
        t_far = t_far.min(t1);
        if t_near > t_far {
            return None;
        }
    }
    if t_far < 0.0 {
        return None; // entirely behind the eye
    }
    // Inside the box counts as a hit at the eye, not as a miss.
    Some(t_near.max(0.0))
}

/// Index of the nearest instance the ray hits, searching only `candidates`.
///
/// Nearest rather than first: the editor draws its own gizmo and grid into
/// the same instance list, so "first hit in draw order" would select whatever
/// happened to be pushed earliest.
#[must_use]
pub fn pick_nearest(origin: Vec3, dir: Vec3, candidates: &[Instance]) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for (i, inst) in candidates.iter().enumerate() {
        let Some(t) = ray_obb(origin, dir, inst) else {
            continue;
        };
        if best.is_none_or(|(_, bt)| t < bt) {
            best = Some((i, t));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_engine::glam::Vec4Swizzles;

    fn cam() -> Camera {
        Camera {
            eye: Vec3::new(3.0, 6.0, 12.0),
            target: Vec3::new(-1.0, 1.0, 0.0),
            fov_y_deg: 60.0,
        }
    }

    #[test]
    fn centre_ray_points_at_the_target() {
        let c = cam();
        let (org, dir) = ray_from_cursor(&c, 1.7, [0.0, 0.0]);
        assert_eq!(org, c.eye);
        let want = (c.target - c.eye).normalize();
        assert!(
            (dir - want).length() < 1e-5,
            "centre ray must look at the target, got {dir:?} want {want:?}"
        );
    }

    #[test]
    fn edge_ray_opens_by_the_horizontal_fov() {
        // The ray at ndc x = 1 must sit atan(tan(fov_y/2) * aspect) off the
        // centre ray. This is the assertion that catches an aspect applied to
        // the wrong axis, which otherwise looks almost right.
        let c = cam();
        for aspect in [1.0, 4.0 / 3.0, 21.0 / 9.0] {
            let (_, centre) = ray_from_cursor(&c, aspect, [0.0, 0.0]);
            let (_, edge) = ray_from_cursor(&c, aspect, [1.0, 0.0]);
            let got = centre.dot(edge).clamp(-1.0, 1.0).acos();
            let want = ((c.fov_y_deg.to_radians() * 0.5).tan() * aspect).atan();
            assert!(
                (got - want).abs() < 1e-4,
                "aspect {aspect}: opened {got} rad, want {want}"
            );
        }
    }

    #[test]
    fn the_ray_agrees_with_unprojecting_the_view_projection() {
        // presenter-architecture.md oracle O3: `view_proj`/`aspect` are a
        // public contract that, until this crate existed, nothing in the
        // workspace consumed. Two expressions of one piece of maths stay
        // honest only while something compares them — at aspects that are
        // NOT 16:9, because 16:9 is the default and would hide a divisor
        // that silently ignores its argument.
        let c = cam();
        for aspect in [1.0, 4.0 / 3.0, 21.0 / 9.0] {
            let inv = c.view_proj(aspect).inverse();
            for ndc in [[0.0, 0.0], [1.0, 0.0], [-0.6, 0.8], [0.35, -0.9]] {
                let near = inv * ember_engine::glam::Vec4::new(ndc[0], ndc[1], 0.0, 1.0);
                let far = inv * ember_engine::glam::Vec4::new(ndc[0], ndc[1], 1.0, 1.0);
                let near = near.xyz() / near.w;
                let far = far.xyz() / far.w;
                let want = (far - near).normalize();

                let (_, got) = ray_from_cursor(&c, aspect, ndc);
                assert!(
                    (got - want).length() < 1e-4,
                    "aspect {aspect} ndc {ndc:?}: basis ray {got:?} vs unprojected {want:?}"
                );
            }
        }
    }

    fn unit_box_at(p: Vec3) -> Instance {
        Instance::new(p, Vec3::ONE, Vec3::ONE)
    }

    #[test]
    fn ray_hits_a_box_in_front_and_misses_one_beside() {
        let org = Vec3::new(0.0, 0.0, 10.0);
        let dir = Vec3::NEG_Z;
        let hit = ray_obb(org, dir, &unit_box_at(Vec3::ZERO)).expect("straight-on hit");
        // Unit cube, half-extent 0.5, so the front face is at z = 0.5.
        assert!((hit - 9.5).abs() < 1e-4, "entered at {hit}, want 9.5");
        assert!(ray_obb(org, dir, &unit_box_at(Vec3::new(4.0, 0.0, 0.0))).is_none());
    }

    #[test]
    fn a_box_behind_the_eye_is_not_picked() {
        let org = Vec3::ZERO;
        assert!(ray_obb(org, Vec3::NEG_Z, &unit_box_at(Vec3::new(0.0, 0.0, 6.0))).is_none());
        assert!(ray_obb(org, Vec3::Z, &unit_box_at(Vec3::new(0.0, 0.0, 6.0))).is_some());
    }

    #[test]
    fn rotation_is_undone_before_the_slab_test() {
        // A box rotated 45 degrees about Y presents a corner to an axis-aligned
        // ray. A test that forgot to un-rotate would still "hit" the AABB and
        // report the wrong distance, so this checks the distance, not the hit.
        let inst = Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::ONE)
            .with_yaw(std::f32::consts::FRAC_PI_4);
        let t = ray_obb(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z, &inst).expect("corner-on hit");
        // Half-diagonal of a unit square is sqrt(2)/2, so the corner is nearer
        // than the face was: 10 - 0.7071.
        let want = std::f32::consts::SQRT_2.mul_add(-0.5, 10.0);
        assert!((t - want).abs() < 1e-4, "entered at {t}, want {want}");
    }

    #[test]
    fn a_negative_scale_still_picks() {
        // Mirrored instances are legal; an unsigned half-extent would make the
        // slab inside-out and the object unselectable, with nothing logged.
        let inst = Instance::new(Vec3::ZERO, Vec3::new(-2.0, 1.0, 1.0), Vec3::ONE);
        assert!(ray_obb(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z, &inst).is_some());
    }

    #[test]
    fn an_eye_inside_a_box_hits_at_zero() {
        let inst = Instance::new(Vec3::ZERO, Vec3::splat(4.0), Vec3::ONE);
        let t = ray_obb(Vec3::ZERO, Vec3::NEG_Z, &inst).expect("inside counts as a hit");
        assert_eq!(t, 0.0);
    }

    #[test]
    fn a_ray_parallel_to_a_slab_misses_only_when_outside_it() {
        let inst = unit_box_at(Vec3::ZERO);
        // Parallel to X, offset in Y beyond the half-extent: must miss, and
        // must not divide by zero on the way.
        assert!(ray_obb(Vec3::new(-10.0, 5.0, 0.0), Vec3::X, &inst).is_none());
        assert!(ray_obb(Vec3::new(-10.0, 0.0, 0.0), Vec3::X, &inst).is_some());
    }

    #[test]
    fn pick_takes_the_nearest_not_the_first() {
        // The editor pushes its grid and gizmo into the same list, so draw
        // order says nothing about what the user clicked.
        let far = unit_box_at(Vec3::new(0.0, 0.0, 0.0));
        let near = unit_box_at(Vec3::new(0.0, 0.0, 5.0));
        let (idx, _) = pick_nearest(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z, &[far, near])
            .expect("one of them is hit");
        assert_eq!(idx, 1, "picked the far box because it was pushed first");
    }

    #[test]
    fn nothing_under_the_cursor_picks_nothing() {
        let boxes = [unit_box_at(Vec3::new(20.0, 0.0, 0.0))];
        assert!(pick_nearest(Vec3::ZERO, Vec3::NEG_Z, &boxes).is_none());
    }
}
