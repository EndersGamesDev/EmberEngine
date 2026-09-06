//! Weapon-specific reload presentation driven by authoritative progress.
//!
//! This is not a second reload clock: pass `1 - reload_remaining / reload_secs`
//! from the server state, optionally interpolated by the client. All offsets are
//! metres in weapon space (+X forward, +Y up, +Z right). The weapon/right glove
//! share one transform; only the support hand releases its authored socket.
//!
//! Most shipped weapon meshes are fused GLB nodes, so they cannot eject a real
//! magazine independently. The authored support glove follows the magazine /
//! receiver operation, while the separate revolver cylinder and RPG round can
//! use the part-stage fields. Breach-12 additionally owns a separate authored
//! box magazine, whose translation follows the support palm during exchange.

use ember_engine::glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct ReloadPose {
    pub offset: Vec3,
    pub rotation: Quat,
    /// Translation relative to the authored left palm, not the weapon origin.
    pub left_offset: Vec3,
    /// Rotate the glove about its palm to avoid orbiting around the gun origin.
    pub left_rotation: Quat,
    /// Existing revolver cylinder moves left by 0.065 m and down by 0.008 m.
    pub cylinder_open: f32,
    /// RPG seating fraction: hide at zero, otherwise place the existing round
    /// `0.34 * (1 - rocket_load)` metres forward of its normal loaded position.
    /// It stays fully seated through the return to rest; non-reload rendering
    /// continues to use the authoritative loaded/ammo state.
    pub rocket_load: f32,
    /// Breach-12's separately authored box magazine moves with the support
    /// palm during withdrawal/reinsertion. Zero for all legacy weapon nodes.
    pub magazine_offset: Vec3,
}

impl ReloadPose {
    pub const REST: Self = Self {
        offset: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        left_offset: Vec3::ZERO,
        left_rotation: Quat::IDENTITY,
        cylinder_open: 0.0,
        rocket_load: 0.0,
        magazine_offset: Vec3::ZERO,
    };

    /// Transform the static weapon-local glove about its authored palm. The
    /// matching arm target is `hand_base + hand_rotation * grip.left.wrist`.
    /// First-person callers must retain the viewmodel's `without_shadow()`.
    pub fn left_hand(self, base: Vec3, rotation: Quat, palm: Vec3) -> (Vec3, Quat) {
        (
            base + rotation * (palm + self.left_offset - self.left_rotation * palm),
            rotation * self.left_rotation,
        )
    }
}

#[derive(Clone, Copy)]
struct Key {
    at: f32,
    offset: Vec3,
    angles: Vec3,
    left_offset: Vec3,
    left_angles: Vec3,
}

impl Key {
    const fn rest(at: f32) -> Self {
        Self {
            at,
            offset: Vec3::ZERO,
            angles: Vec3::ZERO,
            left_offset: Vec3::ZERO,
            left_angles: Vec3::ZERO,
        }
    }
}

const fn key(
    at: f32,
    offset: [f32; 3],
    angles: [f32; 3],
    left_offset: [f32; 3],
    left_angles: [f32; 3],
) -> Key {
    Key {
        at,
        offset: Vec3::from_array(offset),
        angles: Vec3::from_array(angles),
        left_offset: Vec3::from_array(left_offset),
        left_angles: Vec3::from_array(left_angles),
    }
}

/// Raise/cant, release, withdraw, insert, work action, settle. Each authored
/// operation eases into the next with zero endpoint velocity (C1 continuous).
/// M4 is deliberately fitted to the current sidearm fallback geometry rather
/// than pretending the absent M4 mesh has rifle-sized magazine coordinates.
#[rustfmt::skip]
const fn keys(weapon: u8) -> [Key; 7] {
    match weapon {
        // Compact SMG: short straight pull, quick seating tap and receiver slap.
        2 => [
            Key::rest(0.0),
            key(0.14, [-0.025,-0.035,-0.018], [-0.32, 0.04, 0.15], [-0.08,-0.05,-0.025], [0.00, 0.08,-0.15]),
            key(0.30, [-0.045,-0.040,-0.015], [-0.46, 0.07, 0.18], [-0.22,-0.13,-0.035], [0.00, 0.12,-0.22]),
            key(0.48, [-0.060,-0.060,-0.010], [-0.48, 0.07, 0.17], [-0.24,-0.27,-0.040], [0.10, 0.16,-0.30]),
            key(0.68, [-0.030,-0.025,-0.010], [-0.35, 0.04, 0.20], [-0.21,-0.10,-0.015], [0.00, 0.06,-0.10]),
            key(0.85, [-0.020,-0.025, 0.000], [-0.12, 0.00, 0.07], [-0.17, 0.07,-0.040], [0.05,-0.18, 0.22]),
            Key::rest(1.0),
        ],
        // AK: broad cant, curved magazine rocked out and heel-first back in.
        3 => [
            Key::rest(0.0),
            key(0.16, [-0.040,-0.030,-0.010], [-0.40,-0.05, 0.18], [-0.06,-0.04,-0.020], [0.00, 0.00, 0.10]),
            key(0.33, [-0.060,-0.055,-0.020], [-0.62,-0.06, 0.23], [-0.16,-0.10,-0.030], [0.05, 0.08, 0.38]),
            key(0.52, [-0.075,-0.065,-0.025], [-0.66,-0.08, 0.22], [-0.22,-0.29,-0.045], [0.10, 0.12, 0.62]),
            key(0.73, [-0.045,-0.035,-0.015], [-0.43,-0.03, 0.26], [-0.15,-0.09,-0.015], [0.00, 0.04,-0.18]),
            key(0.88, [-0.060,-0.020, 0.000], [-0.16, 0.06, 0.06], [-0.22, 0.08, 0.075], [0.00,-0.38, 0.32]),
            Key::rest(1.0),
        ],
        // M4/fallback: tighter tactical tilt, fast straight insertion, release.
        4 => [
            Key::rest(0.0),
            key(0.12, [-0.030,-0.035,-0.012], [-0.26, 0.02, 0.12], [-0.05,-0.02,-0.025], [0.02, 0.06,-0.12]),
            key(0.31, [-0.035,-0.050,-0.020], [-0.38, 0.03, 0.18], [-0.17,-0.06,-0.040], [0.00, 0.10,-0.20]),
            key(0.50, [-0.055,-0.060,-0.020], [-0.40, 0.04, 0.16], [-0.19,-0.22,-0.035], [0.08, 0.08,-0.25]),
            key(0.69, [-0.025,-0.025,-0.010], [-0.28, 0.00, 0.19], [-0.15,-0.05,-0.015], [0.00, 0.05,-0.08]),
            key(0.83, [-0.020,-0.030, 0.000], [-0.10,-0.02, 0.05], [-0.14, 0.12,-0.030], [0.08,-0.16, 0.18]),
            Key::rest(1.0),
        ],
        // Revolver: expose the cylinder, eject, speedloader seat, close.
        5 => [
            Key::rest(0.0),
            key(0.17, [-0.040,-0.005,-0.020], [-0.52, 0.10, 0.28], [0.07, 0.04,-0.035], [0.08, 0.16, 0.10]),
            key(0.34, [-0.045, 0.010,-0.030], [-0.78, 0.12, 0.42], [0.13, 0.09,-0.055], [0.15, 0.24, 0.24]),
            key(0.52, [-0.060,-0.030,-0.025], [-0.70, 0.14, 0.08], [0.09,-0.17,-0.120], [0.28, 0.18, 0.36]),
            key(0.73, [-0.040,-0.015,-0.020], [-0.60, 0.08, 0.12], [0.13, 0.07,-0.045], [0.06, 0.14, 0.10]),
            key(0.87, [-0.015,-0.020, 0.000], [ 0.12,-0.04, 0.10], [0.10, 0.05, 0.010], [0.00,-0.14,-0.18]),
            Key::rest(1.0),
        ],
        // Sniper: measured magazine swap with a deliberate receiver/bolt reach.
        6 => [
            Key::rest(0.0),
            key(0.18, [-0.090,-0.040,-0.015], [-0.25,-0.05, 0.14], [-0.08,-0.03,-0.025], [0.00, 0.04,-0.12]),
            key(0.35, [-0.110,-0.055,-0.020], [-0.39,-0.08, 0.19], [-0.23,-0.11,-0.040], [0.08, 0.14,-0.20]),
            key(0.55, [-0.120,-0.080,-0.025], [-0.42,-0.06, 0.16], [-0.25,-0.28,-0.055], [0.12, 0.20,-0.32]),
            key(0.72, [-0.080,-0.035,-0.015], [-0.31,-0.02, 0.20], [-0.21,-0.08,-0.020], [0.00, 0.08,-0.06]),
            key(0.90, [-0.105,-0.020, 0.005], [-0.18, 0.08, 0.05], [-0.28, 0.09, 0.090], [0.08,-0.42, 0.30]),
            Key::rest(1.0),
        ],
        // RPG: lower the long tube, bring a round forward, seat, re-shoulder.
        7 => [
            Key::rest(0.0),
            key(0.16, [-0.105,-0.065,-0.025], [ 0.22,-0.04, 0.14], [0.08,-0.09,-0.070], [0.12, 0.05,-0.12]),
            key(0.34, [-0.160,-0.100,-0.025], [ 0.35,-0.05, 0.18], [0.20,-0.20,-0.120], [0.22, 0.10,-0.35]),
            key(0.54, [-0.190,-0.095,-0.020], [ 0.38,-0.06, 0.23], [0.52, 0.09,-0.070], [0.18,-0.15,-0.12]),
            key(0.78, [-0.150,-0.080,-0.015], [ 0.28,-0.02, 0.20], [0.34, 0.07,-0.030], [0.08,-0.08, 0.10]),
            key(0.90, [-0.070,-0.040, 0.000], [ 0.10, 0.02, 0.06], [0.12, 0.03,-0.010], [0.00, 0.00, 0.05]),
            Key::rest(1.0),
        ],
        // Breach-12: weighty box-mag swap, firm seat, then receiver release.
        8 => [
            Key::rest(0.0),
            key(0.15, [-0.060,-0.025,-0.010], [-0.32, 0.04, 0.13], [-0.10,-0.04,-0.025], [0.00, 0.03,-0.10]),
            key(0.32, [-0.075,-0.040,-0.020], [-0.53, 0.08, 0.20], [-0.19,-0.12,-0.010], [0.02, 0.04,-0.14]),
            key(0.53, [-0.095,-0.060,-0.025], [-0.56, 0.08, 0.18], [-0.21,-0.36,-0.010], [0.02, 0.04,-0.14]),
            key(0.72, [-0.055,-0.020,-0.015], [-0.40, 0.03, 0.23], [-0.19,-0.12,-0.010], [0.02, 0.04,-0.14]),
            key(0.90, [-0.080,-0.025, 0.000], [-0.12,-0.06, 0.06], [-0.24, 0.10, 0.085], [0.08,-0.32, 0.22]),
            Key::rest(1.0),
        ],
        // Pistol: compact chest-high magazine reach, seat and overhand rack.
        _ => [
            Key::rest(0.0),
            key(0.15, [-0.015,-0.025,-0.015], [-0.22, 0.07, 0.25], [-0.06,-0.01,-0.025], [0.02, 0.08,-0.12]),
            key(0.32, [-0.025,-0.045,-0.020], [-0.36, 0.10, 0.32], [-0.18,-0.04,-0.045], [0.05, 0.15,-0.18]),
            key(0.50, [-0.045,-0.055,-0.020], [-0.40, 0.10, 0.28], [-0.20,-0.21,-0.050], [0.14, 0.20,-0.34]),
            key(0.70, [-0.020,-0.020,-0.010], [-0.25, 0.04, 0.32], [-0.17,-0.03,-0.025], [0.00, 0.08,-0.10]),
            key(0.86, [-0.055,-0.025, 0.000], [-0.08,-0.08, 0.08], [-0.20, 0.15, 0.035], [0.12,-0.25, 0.28]),
            Key::rest(1.0),
        ],
    }
}

fn smooth(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ramp(progress: f32, from: f32, to: f32) -> f32 {
    smooth((progress - from) / (to - from))
}

fn rotation(angles: Vec3) -> Quat {
    Quat::from_rotation_x(angles.x)
        * Quat::from_rotation_y(angles.y)
        * Quat::from_rotation_z(angles.z)
}

/// Evaluate one pose without retaining client time or changing game state.
/// Invalid progress safely returns the beginning pose; unknown ids use the
/// pistol's authored fallback. Progress outside the interval clamps to rest.
pub fn pose(weapon: u8, progress: f32) -> ReloadPose {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let keys = keys(weapon);
    let mut result = ReloadPose::REST;
    for pair in keys.windows(2) {
        let [from, to] = [pair[0], pair[1]];
        if progress <= to.at {
            let t = ramp(progress, from.at, to.at);
            result.offset = from.offset.lerp(to.offset, t);
            result.rotation = rotation(from.angles.lerp(to.angles, t));
            result.left_offset = from.left_offset.lerp(to.left_offset, t);
            result.left_rotation = rotation(from.left_angles.lerp(to.left_angles, t));
            break;
        }
    }
    if weapon == 5 {
        result.cylinder_open = ramp(progress, 0.16, 0.30) * (1.0 - ramp(progress, 0.76, 0.88));
    } else if weapon == 7 {
        result.rocket_load = ramp(progress, 0.35, 0.78);
    } else if weapon == 8 {
        let withdrawn = ramp(progress, 0.32, 0.53) * (1.0 - ramp(progress, 0.53, 0.72));
        result.magazine_offset = Vec3::new(-0.02, -0.24, 0.0) * withdrawn;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rest(value: ReloadPose) {
        assert_eq!(value.offset, Vec3::ZERO);
        assert_eq!(value.left_offset, Vec3::ZERO);
        assert_eq!(value.rotation, Quat::IDENTITY);
        assert_eq!(value.left_rotation, Quat::IDENTITY);
        assert_eq!(value.cylinder_open, 0.0);
        assert_eq!(value.magazine_offset, Vec3::ZERO);
    }

    #[test]
    fn endpoints_do_not_leave_the_gun_or_glove_displaced() {
        for weapon in 1..=8 {
            for p in [-1.0, 0.0, 1.0, 2.0, f32::NAN, f32::INFINITY] {
                assert_rest(pose(weapon, p));
            }
        }
    }

    #[test]
    fn every_weapon_has_a_distinct_complete_motion() {
        for weapon in 1..=8 {
            for other in 1..weapon {
                let signature = |id| {
                    [0.2, 0.4, 0.6, 0.8]
                        .map(|p| pose(id, p).offset.length() + pose(id, p).left_offset.length())
                };
                assert_ne!(signature(weapon), signature(other));
            }
            let mid = pose(weapon, 0.5);
            assert!(mid.offset.length() > 0.04);
            assert!(mid.left_offset.length() > 0.12);
            assert!(mid.rotation.angle_between(Quat::IDENTITY) > 0.2);
        }
        assert_eq!(pose(0, 0.5).offset, pose(1, 0.5).offset);
        assert_eq!(pose(255, 0.5).left_offset, pose(1, 0.5).left_offset);
    }

    #[test]
    fn all_samples_are_finite_metric_and_normalized() {
        for weapon in 1..=8 {
            for tick in 0..=1000_u16 {
                let value = pose(weapon, f32::from(tick) / 1000.0);
                assert!(value.offset.is_finite() && value.offset.length() < 0.25);
                assert!(value.left_offset.is_finite() && value.left_offset.length() < 0.60);
                for rotation in [value.rotation, value.left_rotation] {
                    assert!(rotation.is_finite() && (rotation.length() - 1.0).abs() < 0.00001);
                }
                assert!((0.0..=1.0).contains(&value.cylinder_open));
                assert!((0.0..=1.0).contains(&value.rocket_load));
                assert!(value.magazine_offset.is_finite() && value.magazine_offset.length() < 0.25);
            }
        }
    }

    #[test]
    fn pose_and_velocity_are_continuous_across_each_authored_phase() {
        let h = 0.0001;
        for weapon in 1..=8 {
            for key in keys(weapon) {
                let before = pose(weapon, key.at - h);
                let at = pose(weapon, key.at);
                let after = pose(weapon, key.at + h);
                assert!((after.offset - before.offset).length() < 0.00001);
                assert!((after.left_offset - before.left_offset).length() < 0.00001);
                assert!((after.magazine_offset - before.magazine_offset).length() < 0.00001);
                // Zero derivatives at the knots, including entry/exit. Compare
                // quaternion components rather than acos near exactly one.
                assert!((after.rotation - before.rotation).length() < 0.00001);
                assert!((after.left_rotation - before.left_rotation).length() < 0.00001);
                let incoming = (at.left_offset - before.left_offset) / h;
                let outgoing = (after.left_offset - at.left_offset) / h;
                assert!((outgoing - incoming).length() < 0.04);
            }
        }
    }

    #[test]
    fn glove_rotation_keeps_the_palm_at_its_contact_target() {
        let base = Vec3::new(4.0, 2.0, -3.0);
        let weapon_rotation = Quat::from_rotation_y(0.8) * Quat::from_rotation_z(-0.4);
        let palm = Vec3::new(0.24, -0.11, -0.07);
        for weapon in 1..=8 {
            for tick in 0..=100_u8 {
                let value = pose(weapon, f32::from(tick) / 100.0);
                let (hand_base, hand_rotation) = value.left_hand(base, weapon_rotation, palm);
                let actual = hand_base + hand_rotation * palm;
                let intended = base + weapon_rotation * (palm + value.left_offset);
                assert!((actual - intended).length() < 0.00001);
            }
        }
    }

    #[test]
    fn independent_parts_have_correct_open_and_seated_stages() {
        assert_eq!(pose(5, 0.1).cylinder_open, 0.0);
        assert_eq!(pose(5, 0.5).cylinder_open, 1.0);
        assert_eq!(pose(5, 0.9).cylinder_open, 0.0);
        assert_eq!(pose(7, 0.3).rocket_load, 0.0);
        assert!(pose(7, 0.6).rocket_load > 0.5);
        assert_eq!(pose(7, 0.8).rocket_load, 1.0);
        assert_eq!(pose(7, 1.0).rocket_load, 1.0);
        for weapon in [1, 2, 3, 4, 6] {
            let value = pose(weapon, 0.5);
            assert_eq!(value.cylinder_open, 0.0);
            assert_eq!(value.rocket_load, 0.0);
        }
    }

    #[test]
    fn shotgun_magazine_tracks_the_support_palm_during_exchange() {
        let grabbed = pose(8, 0.32);
        for p in [0.32, 0.4, 0.53, 0.62, 0.72] {
            let sample = pose(8, p);
            assert!(
                (sample.left_offset - grabbed.left_offset - sample.magazine_offset).length()
                    < 0.00001
            );
        }
        assert_eq!(pose(8, 0.0).magazine_offset, Vec3::ZERO);
        assert_eq!(pose(8, 0.53).magazine_offset, Vec3::new(-0.02, -0.24, 0.0));
        assert_eq!(pose(8, 1.0).magazine_offset, Vec3::ZERO);
        for weapon in 1..=7 {
            assert_eq!(pose(weapon, 0.53).magazine_offset, Vec3::ZERO);
        }
    }
}
