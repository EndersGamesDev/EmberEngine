//! Authored heavy-weapon arcs in eye space; both hands stay on the same hilt.
use super::hands::Placement;
use end_game_core::combat::{Strike, StrikeKind};
use end_game_core::interaction::ease;
use glam::{Quat, Vec3};

fn arc(x: f32, y: f32, z: f32, plane: f32, travel: f32) -> Placement {
    Placement {
        p: Vec3::new(x, y, z),
        r: Quat::from_rotation_z(plane.to_radians())
            * Quat::from_rotation_y(travel.to_radians())
            * Quat::from_rotation_x(65_f32.to_radians()),
    }
}
pub fn ready() -> Placement {
    Placement {
        p: Vec3::new(0.07, -0.30, -0.40),
        r: Quat::from_xyzw(0.29315, 0.48667, 0.32568, 0.75574).normalize(),
    }
}
fn keys(kind: StrikeKind) -> [Placement; 3] {
    match kind {
        StrikeKind::Cut => [
            arc(-0.10, -0.12, -0.39, 135.0, 25.0),
            arc(0.02, -0.25, -0.56, 135.0, 88.0),
            arc(0.15, -0.39, -0.39, 135.0, 140.0),
        ],
        StrikeKind::Backhand => [
            arc(-0.15, -0.30, -0.39, 180.0, 28.0),
            arc(0.00, -0.25, -0.56, 180.0, 85.0),
            arc(0.16, -0.21, -0.39, 180.0, 150.0),
        ],
        StrikeKind::Finisher => [
            arc(0.07, -0.02, -0.39, 55.0, 22.0),
            arc(0.02, -0.24, -0.56, 55.0, 88.0),
            arc(-0.16, -0.42, -0.39, 55.0, 138.0),
        ],
        StrikeKind::Overhead => [
            arc(0.00, 0.035, -0.39, 90.0, 18.0),
            arc(0.00, -0.25, -0.56, 90.0, 85.0),
            arc(0.00, -0.45, -0.39, 90.0, 125.0),
        ],
        StrikeKind::Rising => [
            arc(-0.09, -0.45, -0.39, -110.0, 25.0),
            arc(0.00, -0.26, -0.56, -110.0, 85.0),
            arc(0.10, -0.04, -0.39, -110.0, 155.0),
        ],
    }
}
/// Each segment eases at a physically legible change of direction. The cut
/// accelerates out of the held wind-up and decelerates through follow-through.
pub fn sample(kind: StrikeKind, elapsed: f32) -> Placement {
    let [wind, contact, follow] = keys(kind);
    let w = kind.windup_time();
    let c = kind.contact_time();
    let f = kind.follow_end();
    let authored = if elapsed < w {
        ready().mix(wind, ease(0.0, w, elapsed))
    } else if elapsed < c {
        let t = ((elapsed - w) / (c - w)).clamp(0.0, 1.0);
        wind.mix(contact, t * t)
    } else if elapsed < f {
        let t = ((elapsed - c) / (f - c)).clamp(0.0, 1.0);
        contact.mix(follow, 1.0 - (1.0 - t) * (1.0 - t))
    } else {
        follow.mix(ready(), ease(f, kind.duration(), elapsed))
    };
    keep_blade_in_front(authored)
}
fn keep_blade_in_front(mut p: Placement) -> Placement {
    // Conservative bounds of the actual V3 pommel, guard and blade. Correct
    // the shared weapon transform before IK so neither hand slides off its grip.
    let d = p.r * Vec3::X;
    let n = p.r * Vec3::Y;
    let w = p.r * Vec3::Z;
    let pommel = -0.37 * d.z + 0.027;
    let guard = 0.05 * d.z.abs() + 0.212 * w.z.abs() + 0.03 * n.z.abs();
    let blade =
        if d.z < 0.0 { 0.02 * d.z } else { 1.85 * d.z } + 0.13 * w.z.abs() + 0.013 * n.z.abs();
    p.p.z = p.p.z.min(-0.15 - pommel.max(guard).max(blade));
    p
}
pub fn weapon(strike: Option<Strike>, head: Vec3, view: Quat) -> Placement {
    let local = strike.map_or_else(ready, |s| sample(s.kind, s.elapsed));
    Placement {
        p: head + view * local.p,
        r: view * local.r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arcs_are_distinct_and_return_to_guard_without_a_snap() {
        let kinds = [
            StrikeKind::Cut,
            StrikeKind::Backhand,
            StrikeKind::Finisher,
            StrikeKind::Overhead,
            StrikeKind::Rising,
        ];
        for kind in kinds {
            let end = sample(kind, kind.duration());
            assert!(end.p.abs_diff_eq(ready().p, 1e-5));
            assert!(end.r.abs_diff_eq(ready().r, 1e-5));
            let cut_travel = sample(kind, kind.windup_time())
                .r
                .angle_between(sample(kind, kind.contact_time()).r);
            assert!(cut_travel > 0.6, "{kind:?} should sweep a substantial arc");
        }
        let down = sample(StrikeKind::Overhead, StrikeKind::Overhead.follow_end()).r * Vec3::X;
        let up = sample(StrikeKind::Rising, StrikeKind::Rising.follow_end()).r * Vec3::X;
        assert!(down.y < -0.4 && up.y > 0.7);
    }
}
