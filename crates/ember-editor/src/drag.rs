//! Dragging a gizmo handle: translate, rotate and scale, locked to one axis.
//!
//! One invariant runs through all three, and it is the difference between an
//! editor that feels precise and one that feels broken: **record the
//! parameter at press, and apply the difference**. Never the absolute. If a
//! drag sets the object to where the cursor is, the object teleports the
//! instant you grab it — by exactly the distance between where you clicked
//! and where the object's centre happens to be.
//!
//! Rotation is about Y only, and that is a data fact rather than an
//! omission. `Obj` carries a yaw, because the sim's `Obstacle` is an
//! axis-aligned box on XZ with no rotation at all: a freely rotated
//! collidable is unrepresentable without OBB collision in `overlaps`,
//! `blocked`, `support_height` and the bullet test. So a handle that cannot
//! produce storable data does not grab, rather than turning something the
//! export would later silently discard.

use ember_engine::glam::Vec3;

use crate::gizmo::{Axis, Mode};

/// Below this the axis and the view ray are close enough to parallel that
/// the closest-point solve is numerically meaningless — the denominator
/// vanishes and a tiny cursor movement swings the result across the level.
const PARALLEL_EPS: f32 = 1e-3;
/// A scale drag may not take an extent below this.
///
/// Zero would make the box
/// unpickable (a degenerate slab) and negative would mirror it, both of
/// which are one-way trips for someone dragging past the middle.
pub const MIN_SCALE: f32 = 0.05;

/// The part of an object a drag can change.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Xform {
    pub pos: Vec3,
    pub scale: Vec3,
    pub yaw: f32,
}

/// Where along an infinite axis line the cursor ray comes closest.
///
/// Returns the parameter in world units measured from `axis_point` along
/// `axis_dir`, or `None` when the two lines are too close to parallel to
/// give an answer worth having.
#[must_use]
pub fn closest_param_on_axis(
    ray_org: Vec3,
    ray_dir: Vec3,
    axis_point: Vec3,
    axis_dir: Vec3,
) -> Option<f32> {
    // Standard two-line closest approach, with both directions unit length
    // so the Gram determinant reduces to 1 - (d1.d2)^2.
    let r = ray_org - axis_point;
    let b = ray_dir.dot(axis_dir);
    let denom = b.mul_add(-b, 1.0);
    if denom.abs() < PARALLEL_EPS {
        return None;
    }
    let d = ray_dir.dot(r);
    let e = axis_dir.dot(r);
    Some(b.mul_add(-d, e) / denom)
}

/// Angle of the cursor ray's hit on the plane through `centre` whose normal
/// is `axis_dir`, measured in that plane.
///
/// `None` when the ray is nearly parallel to the plane, where the
/// intersection races off to infinity for a pixel of cursor movement.
#[must_use]
pub fn angle_on_plane(ray_org: Vec3, ray_dir: Vec3, centre: Vec3, axis_dir: Vec3) -> Option<f32> {
    let denom = ray_dir.dot(axis_dir);
    if denom.abs() < PARALLEL_EPS {
        return None;
    }
    let t = (centre - ray_org).dot(axis_dir) / denom;
    if t <= 0.0 {
        return None; // the plane is behind the eye
    }
    let hit = ray_org + ray_dir * t - centre;
    // Any orthonormal basis of the plane will do; the angle is only ever
    // used as a difference, so the choice of zero does not matter as long as
    // it is stable for a given axis.
    let u = perpendicular(axis_dir);
    let v = axis_dir.cross(u);
    Some(hit.dot(v).atan2(hit.dot(u)))
}

/// Some unit vector perpendicular to `n`, chosen to stay well-conditioned.
fn perpendicular(n: Vec3) -> Vec3 {
    // Cross with whichever cardinal axis `n` is least aligned to, so the
    // cross product never approaches zero length.
    let a = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    n.cross(a).normalize()
}

/// A drag in progress.
pub struct Drag {
    pub axis: Axis,
    pub mode: Mode,
    /// The parameter at the moment of the press, in the same units the mode
    /// reads: world distance for translate and scale, radians for rotate.
    anchor: f32,
    /// The object's transform at the press. Everything is applied as a
    /// difference from this, never as an absolute.
    start: Xform,
    /// Rotate only: the previous frame's raw angle, and the accumulated
    /// whole turns needed to unwrap it. Without this a drag crossing the
    /// +/-pi seam jumps a full revolution in one frame.
    prev_angle: f32,
    turns: f32,
}

impl Drag {
    /// Begins a drag, or returns `None` if this handle cannot be grabbed
    /// from this angle — a nearly edge-on axis, or a rotation this data
    /// model cannot store.
    #[must_use]
    pub fn begin(
        axis: Axis,
        mode: Mode,
        start: Xform,
        ray_org: Vec3,
        ray_dir: Vec3,
    ) -> Option<Self> {
        let anchor = match mode {
            Mode::Translate | Mode::Scale => {
                closest_param_on_axis(ray_org, ray_dir, start.pos, axis.dir())?
            }
            Mode::Rotate => {
                // See the module head: yaw is the only rotation `Obstacle`
                // can represent, so X and Z simply do not grab.
                if axis != Axis::Y {
                    return None;
                }
                angle_on_plane(ray_org, ray_dir, start.pos, axis.dir())?
            }
        };
        Some(Self {
            axis,
            mode,
            anchor,
            start,
            prev_angle: anchor,
            turns: 0.0,
        })
    }

    /// The transform for this frame's cursor ray, or `None` when the ray has
    /// swung to where the solve is meaningless — in which case the caller
    /// should hold the last good transform rather than snap anywhere.
    pub fn update(&mut self, ray_org: Vec3, ray_dir: Vec3) -> Option<Xform> {
        let d = self.axis.dir();
        match self.mode {
            Mode::Translate => {
                let now = closest_param_on_axis(ray_org, ray_dir, self.start.pos, d)?;
                let mut out = self.start;
                out.pos = self.start.pos + d * (now - self.anchor);
                Some(out)
            }
            Mode::Scale => {
                let now = closest_param_on_axis(ray_org, ray_dir, self.start.pos, d)?;
                let delta = now - self.anchor;
                let mut out = self.start;
                // Only the dragged axis changes: an editor that scales all
                // three from one handle cannot make a wall.
                let comp = (self.start.scale.dot(d) + delta).max(MIN_SCALE);
                out.scale = self.start.scale - d * self.start.scale.dot(d) + d * comp;
                Some(out)
            }
            Mode::Rotate => {
                let raw = angle_on_plane(ray_org, ray_dir, self.start.pos, d)?;
                // Unwrap: fold any jump larger than half a turn into `turns`,
                // so the accumulated angle is continuous across the seam.
                let step = raw - self.prev_angle;
                if step > std::f32::consts::PI {
                    self.turns -= 1.0;
                } else if step < -std::f32::consts::PI {
                    self.turns += 1.0;
                }
                self.prev_angle = raw;
                let total = self.turns.mul_add(std::f32::consts::TAU, raw) - self.anchor;
                let mut out = self.start;
                // Negated: a plane angle increasing counter-clockwise about
                // +Y corresponds to a decreasing yaw in the client's
                // convention, and an editor that rotates the opposite way to
                // the drag is worse than one that does not rotate.
                out.yaw = self.start.yaw - total;
                Some(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xform() -> Xform {
        Xform {
            pos: Vec3::ZERO,
            scale: Vec3::new(2.0, 2.0, 2.0),
            yaw: 0.0,
        }
    }

    /// A ray from directly above, aimed at a point on the ground plane.
    fn ray_down_at(x: f32, z: f32) -> (Vec3, Vec3) {
        (Vec3::new(x, 40.0, z), Vec3::NEG_Y)
    }

    #[test]
    fn the_closest_param_reads_off_the_axis_directly() {
        let (o, d) = ray_down_at(5.0, 0.0);
        let t = closest_param_on_axis(o, d, Vec3::ZERO, Vec3::X).expect("perpendicular solve");
        assert!((t - 5.0).abs() < 1e-4, "expected 5 along X, got {t}");
    }

    #[test]
    fn an_axis_edge_on_to_the_view_refuses_rather_than_guessing() {
        // Looking straight down the X axis: the solve's denominator vanishes
        // and any answer is noise. The guard is what stops a pixel of cursor
        // movement throwing the object across the level.
        let o = Vec3::new(-50.0, 0.0, 0.0);
        assert!(closest_param_on_axis(o, Vec3::X, Vec3::ZERO, Vec3::X).is_none());
        // A degree or two off is fine again.
        let d = Vec3::new(1.0, 0.12, 0.0).normalize();
        assert!(closest_param_on_axis(o, d, Vec3::ZERO, Vec3::X).is_some());
    }

    #[test]
    fn a_translate_drag_moves_by_the_difference_not_the_absolute() {
        // THE property. Grab at x = 5 on an object sitting at the origin and
        // move the cursor to x = 8: the object moves by 3, it does not jump
        // to 8.
        let start = xform();
        let (o, d) = ray_down_at(5.0, 0.0);
        let mut drag = Drag::begin(Axis::X, Mode::Translate, start, o, d).expect("grab");
        let (o2, d2) = ray_down_at(8.0, 0.0);
        let out = drag.update(o2, d2).expect("still solvable");
        assert!(
            (out.pos.x - 3.0).abs() < 1e-4,
            "moved to {} — that is the absolute, not the difference",
            out.pos.x
        );
    }

    #[test]
    fn a_drag_that_has_not_moved_changes_nothing() {
        // The frame right after the press must be a no-op, or every grab
        // nudges the object slightly.
        let start = xform();
        let (o, d) = ray_down_at(5.0, 0.0);
        let mut drag = Drag::begin(Axis::X, Mode::Translate, start, o, d).expect("grab");
        let out = drag.update(o, d).expect("solvable");
        assert_eq!(out.pos, start.pos);
    }

    #[test]
    fn translating_one_axis_leaves_the_others_alone() {
        let start = xform();
        let (o, d) = ray_down_at(0.0, 4.0);
        let mut drag = Drag::begin(Axis::Z, Mode::Translate, start, o, d).expect("grab");
        let (o2, d2) = ray_down_at(0.0, 9.0);
        let out = drag.update(o2, d2).unwrap();
        assert!((out.pos.z - 5.0).abs() < 1e-4);
        assert_eq!(out.pos.x, 0.0);
        assert_eq!(out.pos.y, 0.0);
    }

    #[test]
    fn a_scale_drag_grows_only_the_dragged_axis() {
        let start = xform();
        let (o, d) = ray_down_at(2.0, 0.0);
        let mut drag = Drag::begin(Axis::X, Mode::Scale, start, o, d).expect("grab");
        let (o2, d2) = ray_down_at(5.0, 0.0);
        let out = drag.update(o2, d2).unwrap();
        assert!(
            (out.scale.x - 5.0).abs() < 1e-4,
            "x scale is {}",
            out.scale.x
        );
        assert_eq!(out.scale.y, 2.0, "y must not follow x");
        assert_eq!(out.scale.z, 2.0, "z must not follow x");
    }

    #[test]
    fn a_scale_drag_through_zero_clamps_instead_of_mirroring() {
        // Dragging past the centre would otherwise produce a negative extent:
        // the box turns inside out, and it is not obvious how to get back.
        let start = xform();
        let (o, d) = ray_down_at(2.0, 0.0);
        let mut drag = Drag::begin(Axis::X, Mode::Scale, start, o, d).expect("grab");
        let (o2, d2) = ray_down_at(-30.0, 0.0);
        let out = drag.update(o2, d2).unwrap();
        assert_eq!(out.scale.x, MIN_SCALE);
        assert!(out.scale.x > 0.0, "an extent must never go negative");
    }

    #[test]
    fn rotation_only_grabs_the_axis_the_data_can_store() {
        // Obstacle is an AABB with a yaw and nothing else. A handle that
        // cannot produce storable data must not grab — the alternative is
        // rotating something and having the export quietly drop it.
        let start = xform();
        let (o, d) = ray_down_at(3.0, 3.0);
        assert!(Drag::begin(Axis::Y, Mode::Rotate, start, o, d).is_some());
        assert!(Drag::begin(Axis::X, Mode::Rotate, start, o, d).is_none());
        assert!(Drag::begin(Axis::Z, Mode::Rotate, start, o, d).is_none());
    }

    #[test]
    fn a_rotate_drag_turns_by_the_angle_swept() {
        let start = xform();
        // Grab at +X of the object, drag round to +Z: a quarter turn.
        let (o, d) = ray_down_at(6.0, 0.0);
        let mut drag = Drag::begin(Axis::Y, Mode::Rotate, start, o, d).expect("grab");
        let (o2, d2) = ray_down_at(0.0, 6.0);
        let out = drag.update(o2, d2).unwrap();
        let quarter = std::f32::consts::FRAC_PI_2;
        assert!(
            (out.yaw.abs() - quarter).abs() < 1e-3,
            "a quarter sweep gave {} rad",
            out.yaw
        );
    }

    #[test]
    fn a_rotate_drag_across_the_seam_stays_continuous() {
        // Walking the cursor all the way round in small steps must produce a
        // monotone yaw. Without unwrapping, the step across +/-pi flips the
        // object a full turn in one frame — the classic seam bug.
        let start = xform();
        let r = 6.0;
        let step = std::f32::consts::TAU / 24.0;
        let (o, d) = ray_down_at(r, 0.0);
        let mut drag = Drag::begin(Axis::Y, Mode::Rotate, start, o, d).expect("grab");
        let mut prev = 0.0f32;
        let mut total_jump = 0.0f32;
        for i in 1i16..=24 {
            let a = step * f32::from(i);
            let (oi, di) = ray_down_at(r * a.cos(), r * a.sin());
            let out = drag.update(oi, di).unwrap();
            total_jump = total_jump.max((out.yaw - prev).abs());
            prev = out.yaw;
        }
        assert!(
            total_jump < step * 2.0,
            "largest single-frame yaw jump was {total_jump} rad for a {step} rad step — the seam was not unwrapped"
        );
        // A full lap must land back near where it started, not a turn away.
        assert!(
            (prev.abs() - std::f32::consts::TAU).abs() < 0.05 || prev.abs() < 0.05,
            "a full lap ended at {prev} rad"
        );
    }

    #[test]
    fn a_rotate_plane_seen_edge_on_refuses() {
        // Eye level with the object, looking flat: the Y-normal plane is
        // edge-on and the intersection is meaningless.
        let start = xform();
        let o = Vec3::new(0.0, 0.0, 20.0);
        assert!(angle_on_plane(o, Vec3::NEG_Z, start.pos, Vec3::Y).is_none());
    }

    #[test]
    fn a_plane_behind_the_eye_refuses() {
        // Looking up, away from a ground-plane object: the "intersection" is
        // behind the camera and would drag the object from off-screen.
        let o = Vec3::new(0.0, 5.0, 0.0);
        assert!(angle_on_plane(o, Vec3::Y, Vec3::ZERO, Vec3::Y).is_none());
    }

    #[test]
    fn the_perpendicular_helper_is_always_unit_and_orthogonal() {
        for n in [
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::new(1.0, 1.0, 1.0).normalize(),
        ] {
            let p = perpendicular(n);
            assert!((p.length() - 1.0).abs() < 1e-5, "not unit for {n:?}");
            assert!(p.dot(n).abs() < 1e-5, "not perpendicular to {n:?}");
        }
    }
}
