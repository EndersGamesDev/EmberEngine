//! Edge-leading cuts in eye space; both articulated hands share the hilt transform.
//!
//! Fiore's descending/rising cuts and Meyer's linked openings inform the paths;
//! input rhythms remain game choreography. Asset +X is length, +/-Z are edges,
//! and +/-Y are flats (V3 greatsword-rig.json).
use super::hands::Placement;
use end_game_core::combat::{Combat, Recovery, Strike, StrikeKind};
use glam::{Mat3, Quat, Vec3};

pub fn ready() -> Placement {
    Placement {
        p: Vec3::new(0.07, -0.30, -0.40),
        r: Quat::from_xyzw(0.29315, 0.48667, 0.32568, 0.75574).normalize(),
    }
}
fn direction(kind: StrikeKind) -> Vec3 {
    match kind {
        StrikeKind::Cut => Vec3::new(-0.68, -0.7332, 0.0),
        StrikeKind::Backhand => Vec3::new(0.985, 0.174, 0.0),
        StrikeKind::Finisher => Vec3::new(0.62, -0.7846, 0.0),
        StrikeKind::Overhead => -Vec3::Y,
        StrikeKind::Rising => Vec3::new(0.60, 0.80, 0.0),
    }
    .normalize()
}
fn angles(kind: StrikeKind) -> (f32, f32) {
    let (wind, follow): (f32, f32) = match kind {
        StrikeKind::Cut => (-60.0, 58.0),
        StrikeKind::Backhand => (-58.0, 58.0),
        StrikeKind::Finisher => (-65.0, 62.0),
        StrikeKind::Overhead => (-62.0, 62.0),
        StrikeKind::Rising => (-60.0, 66.0),
    };
    (wind.to_radians(), follow.to_radians())
}
fn smooth(t: f32) -> f32 {
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
fn blade_frame(length: Vec3, edge: Vec3) -> Quat {
    let length = length.normalize();
    let edge = (edge - length * edge.dot(length)).normalize();
    Quat::from_mat3(&Mat3::from_cols(length, edge.cross(length), edge)).normalize()
}
fn mix(from: Placement, to: Placement, t: f32) -> Placement {
    // A whole-quaternion slerp can swing the long blade behind the eye during
    // a 180-degree edge change. Swing the point along its short forward arc,
    // then turn the hilt around that axis while preparing the next cut.
    let t = t.clamp(0.0, 1.0);
    let swing = Quat::from_rotation_arc(from.r * Vec3::X, to.r * Vec3::X);
    let twist = (swing * from.r).conjugate() * to.r;
    Placement {
        p: from.p.lerp(to.p, t),
        r: (Quat::IDENTITY.slerp(swing, t) * from.r * Quat::IDENTITY.slerp(twist, t)).normalize(),
    }
}
fn cut_pose(kind: StrikeKind, angle: f32) -> Placement {
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
    keep_blade_in_front(Placement {
        p: Vec3::new(0.0, -0.25, -0.47) + travel * (angle * 0.075),
        r: blade_frame(length, edge * edge_sign),
    })
}
fn chamber(previous: StrikeKind, next: StrikeKind) -> Placement {
    let from = cut_pose(previous, angles(previous).1);
    let to = cut_pose(next, angles(next).0);
    keep_blade_in_front(mix(from, to, 0.30))
}
fn link_strength(previous: StrikeKind, at: f32) -> f32 {
    // A press in the final frame must not force an entire chamber into that
    // frame. Preserve the partial return and let the next preparation finish it.
    smooth((previous.duration() - at.max(previous.follow_end())) / 0.18)
}
fn entry(previous: StrikeKind, next: StrikeKind, at: f32) -> Placement {
    keep_blade_in_front(mix(
        ready(),
        chamber(previous, next),
        link_strength(previous, at),
    ))
}
fn base_sample(strike: Strike, elapsed: f32) -> Placement {
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
pub fn sample(kind: StrikeKind, elapsed: f32) -> Placement {
    sample_strike(Strike::new(kind), elapsed)
}
/// A late press starts with zero blend weight at the exact current pose.
/// Linked recovery ends in the next chamber, whose first sample matches.
pub fn sample_strike(strike: Strike, elapsed: f32) -> Placement {
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
fn keep_blade_in_front(mut p: Placement) -> Placement {
    // Correct the shared weapon before IK; neither hand slides along its grip.
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
/// Angled chest cover, with the strong near the hands toward an incoming knife.
/// A short deflection keeps the blade interposed between threat and body.
pub fn guard(amount: f32, impact_left: f32) -> Placement {
    let recoil = (impact_left / 0.24).clamp(0.0, 1.0);
    let held = Placement {
        p: Vec3::new(0.035, -0.27 - recoil * 0.015, -0.40 + recoil * 0.025),
        r: Quat::from_rotation_z(-recoil * 0.055)
            * blade_frame(Vec3::new(0.64, 0.72, -0.27), -Vec3::Z),
    };
    keep_blade_in_front(mix(ready(), held, smooth(amount)))
}
pub fn weapon(
    combat: &Combat,
    guard_amount: f32,
    guard_impact_left: f32,
    head: Vec3,
    view: Quat,
) -> Placement {
    let local = if let Some(strike) = combat.active {
        let pose = sample_strike(strike, strike.elapsed);
        // Releasing block can start a cut while the guard lowers. Carry that
        // pose into preparation; the authored edge frame owns the contact.
        let guard_weight =
            smooth(guard_amount) * (1.0 - smooth(strike.elapsed / strike.kind.windup_time()));
        keep_blade_in_front(mix(pose, guard(1.0, guard_impact_left), guard_weight))
    } else if let Some(recovery) = combat.recovery {
        let from = sample_strike(recovery.strike, recovery.strike.kind.duration());
        keep_blade_in_front(mix(
            from,
            ready(),
            smooth(recovery.elapsed / Recovery::DURATION),
        ))
    } else {
        guard(guard_amount, guard_impact_left)
    };
    Placement {
        p: head + view * local.p,
        r: view * local.r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use end_game_core::combat::StrikeLink;
    const KINDS: [StrikeKind; 5] = [
        StrikeKind::Cut,
        StrikeKind::Backhand,
        StrikeKind::Finisher,
        StrikeKind::Overhead,
        StrikeKind::Rising,
    ];
    fn near(a: Placement, b: Placement) {
        assert!(
            a.p.distance(b.p) < 0.00001,
            "position {:?} != {:?}",
            a.p,
            b.p
        );
        assert!(
            a.r.dot(b.r).abs() > 0.999999,
            "rotation {:?} != {:?}",
            a.r,
            b.r
        );
    }
    #[test]
    fn sharp_edge_leads_actual_blade_velocity_through_every_contact() {
        for kind in KINDS {
            for offset in [-0.025, -0.01, 0.0, 0.01, 0.025] {
                let t = kind.contact_time() + offset;
                let pose = sample(kind, t);
                let length = pose.r * Vec3::X;
                let edge = pose.r * Vec3::Z;
                let normal = pose.r * Vec3::Y;
                for along in [0.30, 1.0, 1.75] {
                    let velocity = (sample(kind, t + 0.0001).point(Vec3::X * along)
                        - sample(kind, t - 0.0001).point(Vec3::X * along))
                        / 0.0002;
                    let across = (velocity - length * velocity.dot(length)).normalize();
                    assert!(
                        across.dot(edge).abs() > 0.999,
                        "{kind:?} t={t}: flat-led contact"
                    );
                    assert!(
                        across.dot(normal).abs() < 0.002,
                        "{kind:?} t={t}: flat velocity"
                    );
                }
            }
        }
    }
    #[test]
    fn contact_speed_is_continuous_and_cut_never_reverses_before_followthrough() {
        for kind in KINDS {
            let dt = 0.00005;
            let c = kind.contact_time();
            let tip = |t| sample(kind, t).point(Vec3::X);
            let before = (tip(c) - tip(c - dt)) / dt;
            let after = (tip(c + dt) - tip(c)) / dt;
            assert!(
                before.distance(after) / before.length() < 0.015,
                "{kind:?}: contact velocity discontinuity"
            );
            let mut previous = angles(kind).0;
            for i in 0..=120 {
                let t = kind.windup_time()
                    + (kind.follow_end() - kind.windup_time()) * i as f32 / 120.0;
                let d = sample(kind, t).r * Vec3::X;
                let a = d.dot(direction(kind)).atan2(-d.z);
                assert!(a + 0.00001 >= previous, "{kind:?}: cut reversed");
                previous = a;
            }
            near(sample(kind, kind.duration()), ready());
        }
    }
    #[test]
    fn early_and_late_buffers_join_the_next_chamber_without_a_neutral_reset() {
        for (from, to) in [
            (StrikeKind::Cut, StrikeKind::Backhand),
            (StrikeKind::Cut, StrikeKind::Overhead),
            (StrikeKind::Backhand, StrikeKind::Finisher),
            (StrikeKind::Backhand, StrikeKind::Rising),
        ] {
            for at in [0.10, from.follow_end() + 0.09, from.duration() - 0.035] {
                let linked = Strike {
                    link: Some(StrikeLink {
                        next: to,
                        at,
                        cancelled_at: None,
                    }),
                    ..Strike::new(from)
                };
                near(sample_strike(linked, at), sample(from, at));
                let next = Strike {
                    previous: Some(from),
                    previous_link_at: at,
                    ..Strike::new(to)
                };
                let end = sample_strike(linked, from.duration());
                near(end, sample_strike(next, 0.0));
                if at < from.duration() - 0.18 {
                    assert!(
                        end.r.dot(ready().r).abs() < 0.995,
                        "early link returned to neutral"
                    );
                }
                near(
                    sample_strike(next, to.contact_time()),
                    sample(to, to.contact_time()),
                );
            }
        }
    }
    #[test]
    fn cancelled_links_and_failed_starts_recover_from_the_reached_pose() {
        let mut strike = Strike {
            link: Some(StrikeLink {
                next: StrikeKind::Backhand,
                at: 0.1,
                cancelled_at: None,
            }),
            ..Strike::new(StrikeKind::Cut)
        };
        let cancelled = 0.62;
        let before = sample_strike(strike, cancelled);
        strike.link.as_mut().unwrap().cancelled_at = Some(cancelled);
        near(sample_strike(strike, cancelled), before);
        near(sample_strike(strike, strike.kind.duration()), ready());
        strike.link.as_mut().unwrap().cancelled_at = None;
        let mut combat = Combat::default();
        combat.recovery = Some(Recovery {
            strike,
            elapsed: 0.0,
        });
        near(
            weapon(&combat, 0.0, 0.0, Vec3::ZERO, Quat::IDENTITY),
            sample_strike(strike, strike.kind.duration()),
        );
        combat.recovery.as_mut().unwrap().elapsed = Recovery::DURATION;
        near(
            weapon(&combat, 0.0, 0.0, Vec3::ZERO, Quat::IDENTITY),
            ready(),
        );
        // Breaking the escape chain clears queued input at contact. It must
        // still leave the current edge-leading follow-through untouched.
        strike.link.as_mut().unwrap().cancelled_at = Some(strike.kind.contact_time());
        for i in 0..=120 {
            let t = strike.kind.duration() * i as f32 / 120.0;
            near(sample_strike(strike, t), sample(strike.kind, t));
        }
    }
    #[test]
    fn guard_covers_chest_with_the_strong_and_keeps_the_blade_forward() {
        near(guard(0.0, 0.24), ready());
        for impact in [0.0, 0.12, 0.24] {
            let pose = guard(1.0, impact);
            let strong = pose.point(Vec3::X * 0.30);
            assert!(strong.z < -0.40 && strong.y > -0.10 && strong.x.abs() < 0.30);
            assert!(
                (pose.r * Vec3::Z).z < -0.9,
                "edge should face the incoming blade"
            );
        }
    }

    #[test]
    fn linked_guards_and_releases_keep_actual_grips_reachable_and_mesh_in_front() {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/end-game/v3/hands-rig.json"))
                .unwrap();
        let vec = |v: &serde_json::Value| {
            Vec3::new(
                v[0].as_f64().unwrap() as f32,
                v[1].as_f64().unwrap() as f32,
                v[2].as_f64().unwrap() as f32,
            )
        };
        let wrists = ["contacts_right", "contacts_left"].map(|side| {
            let contact = &spec[side]["power_grip"];
            let alignment = Quat::from_rotation_arc(vec(&contact["axis"]).normalize(), Vec3::X);
            Vec3::X
                * if side == "contacts_right" {
                    -0.13
                } else {
                    -0.25
                }
                - alignment * vec(&contact["point"])
        });
        let sword = ember_engine::assets::load_glb(include_bytes!(
            "../../../assets/end-game/v3/wolf-greatsword.glb"
        ))
        .unwrap();
        let check = |pose: Placement, context: &str| {
            for side in 0..2 {
                let shoulder = Vec3::new(if side == 0 { 0.18 } else { -0.18 }, -0.18, 0.0);
                let distance = pose.point(wrists[side]).distance(shoulder);
                assert!(
                    distance < 0.57,
                    "{context} side {side}: unreachable {distance} at {:?} r {:?} wrist {:?}",
                    pose.p,
                    pose.r,
                    pose.point(wrists[side])
                );
            }
            for part in &sword {
                for vertex in &part.mesh.vertices {
                    let point = pose.point(Vec3::from_array(vertex.pos));
                    assert!(point.z <= -0.10, "{context}: near-plane clip {point:?}");
                }
            }
        };
        for (from, to) in [
            (StrikeKind::Cut, StrikeKind::Backhand),
            (StrikeKind::Cut, StrikeKind::Overhead),
            (StrikeKind::Backhand, StrikeKind::Finisher),
            (StrikeKind::Backhand, StrikeKind::Rising),
        ] {
            for at in [0.1, from.duration() - 0.12, from.duration() - 0.0167] {
                let linked = Strike {
                    link: Some(StrikeLink {
                        next: to,
                        at,
                        cancelled_at: None,
                    }),
                    ..Strike::new(from)
                };
                let next = Strike {
                    previous: Some(from),
                    previous_link_at: at,
                    ..Strike::new(to)
                };
                for i in 0..=120 {
                    check(
                        sample_strike(linked, from.duration() * i as f32 / 120.0),
                        "linked recovery",
                    );
                    check(
                        sample_strike(next, to.duration() * i as f32 / 120.0),
                        "linked preparation",
                    );
                }
            }
        }
        for i in 0..=60 {
            for recoil in [0.0, 0.12, 0.24] {
                check(guard(i as f32 / 60.0, recoil), "guard");
            }
        }
        for kind in KINDS {
            let mut combat = Combat::default();
            for i in 0..=30 {
                let t = i as f32 / 120.0;
                combat.active = Some(Strike {
                    elapsed: t,
                    ..Strike::new(kind)
                });
                check(
                    weapon(
                        &combat,
                        (1.0 - t / 0.12).max(0.0),
                        0.0,
                        Vec3::ZERO,
                        Quat::IDENTITY,
                    ),
                    "guard release into cut",
                );
            }
        }
    }
}
