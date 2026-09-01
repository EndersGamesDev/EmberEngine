//! The circuit: a gothic castle bailey with a racing line through the yard.
//!
//! The layout is a lap around the inside of the curtain wall. Reading from
//! the start line under the gatehouse, clockwise:
//!
//!   * a ~230 m **main straight** down the south wall — long enough that a
//!     boost is worth saving for it;
//!   * **turn 1**, a hard right off the end of the straight;
//!   * the **east sweeper**, a long constant-radius bend you can hold a
//!     drift through for several seconds;
//!   * the **fountain chicane** across the north yard, left-right around the
//!     courtyard fountain;
//!   * the **well hairpin** at the west end, the slowest corner on the
//!     circuit, tight enough to force a real lift;
//!   * a short run back onto the straight.
//!
//! A constant-radius circle would have been three lines of code and no fun at
//! all: one corner, one speed, nothing to learn. Every number below is
//! checked by the tests at the bottom — closure, self-intersection, corner
//! radius against the car's actual turning circle, and straight length
//! against what the boost can do with it.

use glam::Vec2;

use crate::track::Track;

/// Half the racing surface's width, metres.
///
/// The car is ~2.5 m wide, so this is a little over seven cars abreast: room
/// for eight on the grid, room to take two lines through a corner, and room to
/// get a drift wrong.
pub const HALF_WIDTH: f32 = 9.0;

/// Control points of the centreline, metres on the XZ plane. Order is the
/// direction of travel. Each point appears exactly once — `Track::new` wraps
/// the loop itself.
pub const CENTRELINE: [[f32; 2]; 13] = [
    [-110.0, -80.0], // start / finish, under the gatehouse arch
    [10.0, -95.0],   // main straight
    [120.0, -78.0],  // end of straight, braking for turn 1
    [168.0, -30.0],  // turn 1
    [175.0, 35.0],   // east sweeper apex
    [140.0, 88.0],   // sweeper exit
    [75.0, 108.0],   // north-east corner
    [10.0, 92.0],    // fountain chicane, tuck left
    [-50.0, 112.0],  // fountain chicane, back right
    [-112.0, 92.0],  // north-west corner
    [-160.0, 45.0],  // well hairpin entry
    [-172.0, -5.0],  // well hairpin apex
    [-140.0, -52.0], // hairpin exit, back onto the straight
];

/// What sits around the yard.
///
/// Positions are metres; `yaw` is radians about +Y, matching
/// `Instance::with_yaw`. `scale` is the metre length of the prop's longest
/// axis — the renderer divides by the mesh's own extent, so these read as real
/// sizes rather than as mesh-space multipliers.
#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub kind: PropKind,
    pub pos: Vec2,
    pub yaw: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Gatehouse,
    Tower,
    Fountain,
}

/// Landmarks around the bailey.
///
/// Deliberately sparse: each one is a separate mesh in the wasm bundle, and a
/// landmark you pass once a lap does more for orientation than a hedge of
/// clutter that costs the same bytes.
#[must_use]
pub fn props() -> Vec<Prop> {
    vec![
        // Beside the start line, not across it.
        //
        // The first draft straddled the line so every lap began by ducking
        // under the arch. It looked splendid and it walled the track off: the
        // generated gatehouse's opening is roughly a third of its width, so
        // spanning an 18 m road needs a 55 m building, and at any size that
        // fits the yard the arch is narrower than the cars. Landmark beside
        // the line, chequered band on it.
        Prop {
            kind: PropKind::Gatehouse,
            pos: Vec2::new(-104.0, -50.0),
            yaw: -1.45,
            scale: 26.0,
        },
        // Corner towers: the four points you navigate by.
        Prop {
            kind: PropKind::Tower,
            pos: Vec2::new(200.0, -60.0),
            yaw: 0.0,
            scale: 30.0,
        },
        Prop {
            kind: PropKind::Tower,
            pos: Vec2::new(205.0, 75.0),
            yaw: 0.0,
            scale: 30.0,
        },
        Prop {
            kind: PropKind::Tower,
            pos: Vec2::new(105.0, 140.0),
            yaw: 0.0,
            scale: 26.0,
        },
        Prop {
            kind: PropKind::Tower,
            pos: Vec2::new(-145.0, 128.0),
            yaw: 0.0,
            scale: 30.0,
        },
        Prop {
            kind: PropKind::Tower,
            pos: Vec2::new(-205.0, 20.0),
            yaw: 0.0,
            scale: 34.0,
        },
        Prop {
            kind: PropKind::Tower,
            pos: Vec2::new(-175.0, -95.0),
            yaw: 0.0,
            scale: 26.0,
        },
        // The fountain the chicane is named for, in the notch between the two
        // apexes where it is a landmark rather than an obstacle.
        Prop {
            kind: PropKind::Fountain,
            pos: Vec2::new(-20.0, 138.0),
            yaw: 0.0,
            scale: 12.0,
        },
    ]
}

/// Build the circuit.
#[must_use]
pub fn track() -> Track {
    Track::new(
        CENTRELINE.iter().map(|&[x, z]| Vec2::new(x, z)).collect(),
        HALF_WIDTH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::car::{STEER_FALLOFF, STEER_MAX};

    /// The car's tightest possible turn at a given speed, metres.
    /// `yaw_rate = STEER_MAX / (1 + v * STEER_FALLOFF)`, and
    /// `R = v / yaw_rate`.
    fn turning_radius(v: f32) -> f32 {
        v * (1.0 + v * STEER_FALLOFF) / STEER_MAX
    }

    #[test]
    fn the_circuit_does_not_cross_itself() {
        let t = track();
        assert_eq!(
            t.self_intersection(),
            None,
            "centreline crosses itself — lap progress would jump at the crossing"
        );
    }

    /// Every corner must be drivable. A corner tighter than the car's turning
    /// circle at walking pace is not a challenge, it is a dead end.
    #[test]
    fn every_corner_is_drivable() {
        let t = track();
        let r = t.min_curvature_radius();
        let tightest_the_car_can_do = turning_radius(8.0);
        assert!(
            r > tightest_the_car_can_do,
            "tightest corner is {r:.1} m but the car needs {tightest_the_car_can_do:.1} m even at 8 m/s"
        );
        // And it should still be tight enough to be a corner rather than a
        // kink you take flat out.
        assert!(
            r < turning_radius(crate::car::MAX_SPEED),
            "the whole lap is flat out — no corner slows anyone"
        );
    }

    #[test]
    fn the_lap_is_a_sensible_length() {
        let t = track();
        let len = t.length();
        assert!((700.0..1200.0).contains(&len), "lap is {len:.0} m");
    }

    /// The main straight has to be long enough that spending a boost on it is
    /// a real decision. Measure the run where the centreline stays near
    /// straight, starting from the line.
    #[test]
    fn the_main_straight_rewards_a_boost() {
        let t = track();
        let len = t.length();
        let mut straight = 0.0;
        let step = 2.0;
        let mut s = 0.0;
        while s < len {
            let (_, t0) = t.at(s);
            let (_, t1) = t.at(s + step);
            if t0.dot(t1) > 0.9995 {
                straight += step;
            } else if straight > 120.0 {
                break;
            } else {
                straight = 0.0;
            }
            s += step;
        }
        assert!(straight > 120.0, "longest straight is only {straight:.0} m");
    }

    /// Nothing may stand on the racing surface. There is no exemption for the
    /// gatehouse: it had one, and the exemption was hiding a building parked
    /// squarely across the start line.
    ///
    /// The bar is the wall line — a prop inside that is something the car can
    /// reach, and since props have no collision it would be driven through
    /// rather than around.
    #[test]
    fn props_stand_clear_of_the_racing_surface() {
        let t = track();
        let clearance = HALF_WIDTH + crate::sim::WALL_MARGIN;
        for p in props() {
            let lat = t.locate(p.pos).lateral.abs();
            // Half the prop's footprint has to clear the wall too, or a wide
            // building's centre sits outside while its flank is on the road.
            let footprint = p.scale * 0.5;
            assert!(
                lat - footprint > clearance,
                "{:?} at {:?}: centre is {lat:.1} m from the line and it is {:.0} m across, \
                 so it reaches to {:.1} m — inside the {clearance:.1} m wall line",
                p.kind,
                p.pos,
                p.scale,
                lat - footprint,
            );
        }
    }

    #[test]
    fn the_track_is_wide_enough_for_the_grid() {
        // Eight cars, two abreast, each ~2.5 m wide plus clearance.
        const { assert!(HALF_WIDTH * 2.0 > 2.5 * 4.0, "grid rows will not fit") };
    }
}
