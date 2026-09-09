//! Client placement/guard adapter for the shared collision-visible sword path.
use super::hands::Placement;
use end_game_core::blade::{self, BladePose};
use end_game_core::blade::{blade_frame, smooth};
use end_game_core::combat::{Combat, Recovery, Strike};
#[cfg(test)]
use end_game_core::{
    blade::{angles, direction},
    combat::StrikeKind,
};
use glam::{Quat, Vec3};

fn placement(p: BladePose) -> Placement {
    Placement {
        p: p.position,
        r: p.rotation,
    }
}
fn blade_pose(p: Placement) -> BladePose {
    BladePose {
        position: p.p,
        rotation: p.r,
    }
}
pub fn ready() -> Placement {
    placement(blade::ready())
}
pub(super) fn mix(from: Placement, to: Placement, t: f32) -> Placement {
    placement(blade::mix(blade_pose(from), blade_pose(to), t))
}
pub(super) fn keep_blade_in_front(p: Placement) -> Placement {
    placement(blade::keep_blade_in_front(blade_pose(p)))
}
pub fn sample_strike(strike: Strike, elapsed: f32) -> Placement {
    placement(blade::strike_sample(strike, elapsed))
}
#[cfg(test)]
pub fn sample(kind: StrikeKind, elapsed: f32) -> Placement {
    sample_strike(Strike::new(kind), elapsed)
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
    #[test]
    fn resting_blade_stays_below_the_central_view() {
        let meshes = ember_engine::assets::load_glb(include_bytes!(
            "../../../assets/end-game/v3/wolf-greatsword.glb"
        ))
        .unwrap();
        for part in meshes {
            for vertex in part.mesh.vertices {
                if vertex.pos[0] > 0.10 {
                    let p = ready().point(Vec3::from_array(vertex.pos));
                    assert!(
                        p.z < -0.10 && p.y / (-p.z) < -0.10,
                        "idle blade obscures target center: {p:?}"
                    );
                }
            }
        }
    }
    use end_game_core::combat::StrikeLink;
    const KINDS: [StrikeKind; 6] = [
        StrikeKind::Cut,
        StrikeKind::Backhand,
        StrikeKind::Finisher,
        StrikeKind::Overhead,
        StrikeKind::Rising,
        StrikeKind::JumpHeavy,
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
    #[test]
    fn contact_rebound_is_continuous_and_preserves_queued_entry() {
        for kind in KINDS {
            for stop in [
                kind.windup_time() + 0.02,
                kind.contact_time(),
                kind.follow_end() - 0.01,
            ] {
                let original = Strike {
                    link: Some(StrikeLink {
                        next: StrikeKind::Backhand,
                        at: 0.1,
                        cancelled_at: None,
                    }),
                    ..Strike::new(kind)
                };
                let stopped = Strike {
                    surface_stop: Some(stop),
                    ..original
                };
                near(sample_strike(original, stop), sample_strike(stopped, stop));
                near(
                    sample_strike(original, kind.duration()),
                    sample_strike(stopped, kind.duration()),
                );
                let mut previous = sample_strike(stopped, stop);
                for i in 1..=180 {
                    let at = stop + (kind.duration() - stop) * i as f32 / 180.;
                    let pose = sample_strike(stopped, at);
                    assert!(
                        pose.p.distance(previous.p) < 0.04
                            && pose.r.angle_between(previous.r) < 0.18,
                        "{kind:?} rebound jumps at{at}"
                    );
                    previous = pose;
                }
            }
        }
        // A normal standing landing can hold the full blade at follow-through.
        let pose = sample(StrikeKind::JumpHeavy, StrikeKind::JumpHeavy.follow_end());
        let meshes = ember_engine::assets::load_glb(include_bytes!(
            "../../../assets/end-game/v3/wolf-greatsword.glb"
        ))
        .unwrap();
        for part in meshes {
            for vertex in part.mesh.vertices {
                assert!(
                    1.65 + pose.point(Vec3::from_array(vertex.pos)).y >= 0.005,
                    "jumping follow enters floor"
                );
            }
        }
    }
}
