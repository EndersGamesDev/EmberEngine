//! Weapon-specific sight alignment and bounded ADS presentation.
//!
//! Timing and optical magnification come from arena-core; only the authored
//! model's sight position and the raise/settle flourish belong to the client.

use ember_engine::glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Sight {
    /// Weapon-local point on the sight line, in metres (+X forward, +Y up).
    pub point: Vec3,
    /// Eye to sight-reference distance; weapon geometry remains metre-sized.
    pub distance: f32,
    pub raise_pitch: f32,
    pub raise_roll: f32,
}

/// Measured against the shipped GLB in engine space. The M4 currently draws
/// the KSVR fallback, so its sight point must agree with that actual mesh.
/// AK front/rear post tops are 0.1175/0.1168 m; revolver rail/post reaches
/// 0.114 m. KSVR's decorative rear sight obstructs its nominal 0.045 m line,
/// so aim just over its 0.060 m housing. Vityaz's clear tube centre is 0.205 m;
/// RPG's rear aperture/front post are 0.015 m left of the barrel centreline.
pub const fn sight(weapon: u8, own_mesh: bool) -> Sight {
    let point = if own_mesh {
        match weapon {
            2 => Vec3::new(0.0, 0.205, 0.0),
            3 => Vec3::new(0.0, 0.120, 0.0),
            5 => Vec3::new(0.0, 0.116, 0.0),
            6 => Vec3::new(0.0, 0.194, 0.0),
            7 => Vec3::new(0.0, 0.177, -0.015),
            _ => Vec3::new(0.0, 0.061, 0.0),
        }
    } else {
        Vec3::new(0.0, 0.061, 0.0)
    };
    let (distance, raise_pitch, raise_roll) = match weapon {
        2 => (0.48, -0.040, -0.030),
        3 => (0.55, -0.075, -0.045),
        4 => (0.54, -0.060, -0.025),
        5 => (0.58, -0.055, 0.035),
        6 => (0.55, -0.100, -0.050),
        7 => (0.82, -0.110, 0.055),
        _ => (0.50, -0.045, 0.025),
    };
    Sight {
        point,
        distance,
        raise_pitch,
        raise_roll,
    }
}

/// C1-continuous raise/lower with an exact settled endpoint.
pub fn blend(fraction: f32) -> f32 {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    fraction * fraction * (3.0 - 2.0 * fraction)
}

/// A small intermediate roll/dip: zero at both ends, never retained while
/// fully ADS. It adds a visible raise and settle without detaching the hands.
pub fn settle(fraction: f32) -> f32 {
    let raised = blend(fraction);
    4.0 * raised * (1.0 - raised)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sight_has_a_stable_distinct_raise_and_a_valid_metric_point() {
        let mut previous = 0.0;
        for i in 0..=100_u8 {
            let t = f32::from(i) / 100.0;
            assert!(blend(t) >= previous);
            assert!((0.0..=1.0).contains(&settle(t)));
            previous = blend(t);
        }
        assert_eq!(
            (blend(0.0), blend(1.0), settle(0.0), settle(1.0)),
            (0.0, 1.0, 0.0, 0.0)
        );
        for id in 1..=7 {
            let s = sight(id, id != 4);
            assert!(s.point.is_finite() && (0.0..0.3).contains(&s.point.y));
            assert!((0.4..0.9).contains(&s.distance));
            assert!(s.raise_pitch.abs() > 0.01 && s.raise_roll.abs() > 0.01);
        }
        assert_eq!(sight(4, false).point, sight(1, true).point);
        assert_eq!(sight(7, false).point, sight(1, true).point);
    }
}
