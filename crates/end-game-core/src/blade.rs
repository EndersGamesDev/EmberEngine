//! Shared eye-space sword choreography used by collision and articulated hands.
//!
//! The generated blade's +X is length, +/-Z are sharp edges and +/-Y are flats.
//! Every active cut accelerates along its edge plane, with one continuous contact
//! velocity. Input rhythm and animation playback both remain fixed-step state.
use crate::combat::{Strike, StrikeKind};
use glam::{Mat3, Quat, Vec3};

pub const BLADE_ROOT: f32 = 0.02;
pub const BLADE_TIP: f32 = 1.85;
pub const BLADE_HALF_WIDTH: f32 = 0.13;

#[derive(Clone, Copy, Debug)]
pub struct BladePose {
    pub position: Vec3,
    pub rotation: Quat,
}
impl BladePose {
    pub fn point(self, point: Vec3) -> Vec3 {
        self.position + self.rotation * point
    }
}
pub fn ready() -> BladePose {
    BladePose {
        position: Vec3::new(0.20, -0.43, -0.55),
        // Low, forward carry leaves the enemy's torso and quest markers clear.
        // The flat is nearly horizontal, presenting the thin edge to the eye.
        rotation: blade_frame(Vec3::new(0.57, 0.06, -0.819), Vec3::new(0.819, 0., 0.57)),
    }
}
pub fn direction(kind: StrikeKind) -> Vec3 {
    match kind {
        StrikeKind::Cut => Vec3::new(-0.68, -0.7332, 0.0),
        StrikeKind::Backhand => Vec3::X,
        StrikeKind::Finisher => Vec3::new(-0.42, -0.9075, 0.0),
        StrikeKind::Overhead | StrikeKind::JumpHeavy => -Vec3::Y,
        StrikeKind::Rising => Vec3::new(0.60, 0.80, 0.0),
    }
    .normalize()
}
pub fn angles(kind: StrikeKind) -> (f32, f32) {
    let (wind, follow): (f32, f32) = match kind {
        StrikeKind::Cut => (-60.0, 58.0),
        StrikeKind::Backhand => (-58.0, 58.0),
        StrikeKind::Finisher => (-65.0, 62.0),
        StrikeKind::Overhead => (-62.0, 62.0),
        StrikeKind::Rising => (-60.0, 66.0),
        StrikeKind::JumpHeavy => (-78.0, 42.0),
    };
    (wind.to_radians(), follow.to_radians())
}
pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
fn hermite(a: f32, b: f32, va: f32, vb: f32, duration: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * a
        + (t3 - 2.0 * t2 + t) * duration * va
        + (-2.0 * t3 + 3.0 * t2) * b
        + (t3 - t2) * duration * vb
}
pub fn blade_frame(length: Vec3, edge: Vec3) -> Quat {
    let length = length.normalize();
    let edge = (edge - length * edge.dot(length)).normalize();
    Quat::from_mat3(&Mat3::from_cols(length, edge.cross(length), edge)).normalize()
}
pub fn mix(from: BladePose, to: BladePose, t: f32) -> BladePose {
    // A whole-quaternion slerp can swing the long blade behind the eye during
    // a 180-degree edge change. Swing the point along its short forward arc,
    // then turn the hilt around that axis while preparing the next cut.
    let t = t.clamp(0.0, 1.0);
    let swing = Quat::from_rotation_arc(from.rotation * Vec3::X, to.rotation * Vec3::X);
    let twist = (swing * from.rotation).conjugate() * to.rotation;
    BladePose {
        position: from.position.lerp(to.position, t),
        rotation: (Quat::IDENTITY.slerp(swing, t) * from.rotation * Quat::IDENTITY.slerp(twist, t))
            .normalize(),
    }
}
fn cut_pose(kind: StrikeKind, angle: f32) -> BladePose {
    let travel = direction(kind);
    let length = -Vec3::Z * angle.cos() + travel * angle.sin();
    let edge = travel * angle.cos() + Vec3::Z * angle.sin();
    // Backhand uses the opposite edge. Roll changes occur while chambering,
    // never through contact: the blade's flat normal stays constant in the cut.
    let edge_sign = if kind == StrikeKind::Backhand {
        -1.0
    } else {
        1.0
    };
    let mut position = Vec3::new(0.0, -0.25, -0.47) + travel * (angle * 0.075);
    if kind == StrikeKind::JumpHeavy {
        // Keep the very high chamber within the measured forearm reach. The
        // shift vanishes with zero velocity at the shared central contact axis.
        position.z += 0.016 * angle.sin().powi(2);
    }
    keep_blade_in_front(BladePose {
        position,
        rotation: blade_frame(length, edge * edge_sign),
    })
}
fn chamber(previous: StrikeKind, next: StrikeKind) -> BladePose {
    let from = cut_pose(previous, angles(previous).1);
    let to = cut_pose(next, angles(next).0);
    keep_blade_in_front(mix(from, to, 0.30))
}
fn link_strength(previous: StrikeKind, at: f32) -> f32 {
    // A press in the final frame must not force an entire chamber into that
    // frame. Preserve the partial return and let the next preparation finish it.
    smooth((previous.duration() - at.max(previous.follow_end())) / 0.18)
}
fn entry(previous: StrikeKind, next: StrikeKind, at: f32) -> BladePose {
    keep_blade_in_front(mix(
        ready(),
        chamber(previous, next),
        link_strength(previous, at),
    ))
}
fn base_sample(strike: Strike, elapsed: f32) -> BladePose {
    let kind = strike.kind;
    let (wind, follow) = angles(kind);
    let w = kind.windup_time();
    let c = kind.contact_time();
    let f = kind.follow_end();
    if elapsed < w {
        let start = strike
            .previous
            .map_or_else(ready, |p| entry(p, kind, strike.previous_link_at));
        return keep_blade_in_front(mix(start, cut_pose(kind, wind), smooth(elapsed / w)));
    }
    if elapsed <= f {
        // One shared contact velocity joins acceleration and deceleration.
        // Below 3x each segment average keeps the blade angle monotonic.
        let speed = ((-wind / (c - w)).min(follow / (f - c))) * 2.4;
        let angle = if elapsed < c {
            hermite(wind, 0.0, 0.0, speed, c - w, (elapsed - w) / (c - w))
        } else {
            hermite(0.0, follow, speed, 0.0, f - c, (elapsed - c) / (f - c))
        };
        return cut_pose(kind, angle);
    }
    keep_blade_in_front(mix(
        cut_pose(kind, follow),
        ready(),
        smooth((elapsed - f) / (kind.duration() - f)),
    ))
}

/// Standalone strike, also useful for contact/geometry fixtures.
#[cfg(test)]
pub fn sample(kind: StrikeKind, elapsed: f32) -> BladePose {
    strike_sample(Strike::new(kind), elapsed)
}
/// A late press starts with zero blend weight at the exact current pose.
/// Linked recovery ends in the next chamber, whose first sample matches.
fn linked_sample(strike: Strike, elapsed: f32) -> BladePose {
    let mut pose = base_sample(strike, elapsed);
    if let Some(link) = strike.link {
        let begin = strike.kind.follow_end().max(link.at);
        let duration = strike.kind.duration();
        if link.cancelled_at.is_some_and(|at| at <= begin) {
            return pose;
        }
        if duration > begin {
            pose = mix(
                pose,
                chamber(strike.kind, link.next),
                smooth((elapsed - begin) / (duration - begin))
                    * link_strength(strike.kind, link.at),
            );
        }
        if let Some(cancelled) = link.cancelled_at {
            if duration > cancelled {
                pose = mix(
                    pose,
                    ready(),
                    smooth((elapsed - cancelled) / (duration - cancelled)),
                );
            }
        }
    }
    keep_blade_in_front(pose)
}
pub fn keep_blade_in_front(mut p: BladePose) -> BladePose {
    // Correct the shared weapon before IK; neither hand slides along its grip.
    let d = p.rotation * Vec3::X;
    let n = p.rotation * Vec3::Y;
    let w = p.rotation * Vec3::Z;
    let pommel = -0.37 * d.z + 0.027;
    let guard = 0.05 * d.z.abs() + 0.212 * w.z.abs() + 0.03 * n.z.abs();
    let blade =
        if d.z < 0.0 { 0.02 * d.z } else { 1.85 * d.z } + 0.13 * w.z.abs() + 0.013 * n.z.abs();
    p.position.z = p.position.z.min(-0.15 - pommel.max(guard).max(blade));
    p
}

/// Sample the same blade pose for drawing and swept collision. A solid contact
/// keeps the exact impact transform, then recoils into the established recovery.
pub fn strike_sample(strike: Strike, elapsed: f32) -> BladePose {
    let Some(stop) = strike.surface_stop.filter(|stop| elapsed > *stop) else {
        return linked_sample(strike, elapsed);
    };
    let age = elapsed - stop;
    let settle = (strike.kind.duration() - stop).max(0.001);
    let recoil_time = (settle * 0.30).min(0.10);
    let recoil = (stop - 0.035).max(strike.kind.windup_time());
    let impact = linked_sample(strike, stop);
    let back = linked_sample(strike, recoil);
    if age <= recoil_time {
        return keep_blade_in_front(mix(impact, back, smooth(age / recoil_time)));
    }
    keep_blade_in_front(mix(
        back,
        linked_sample(strike, strike.kind.duration()),
        smooth((age - recoil_time) / (settle - recoil_time).max(0.001)),
    ))
}
