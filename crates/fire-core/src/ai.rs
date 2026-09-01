//! A driver for the cars nobody is steering.
//!
//! Deliberately simple: aim at a point down the racing line, brake for
//! corners it cannot take flat, and use the handbrake when the corner is
//! tight enough to need it. It is not trying to be fast — it is trying to be
//! a moving obstacle that behaves plausibly, and to exercise the same
//! `Car::step` a human drives so the sim is tested by playing it.

use crate::car::{forward, Car, CarInput, MAX_SPEED, STEER_FALLOFF, STEER_MAX};
use crate::track::Track;

/// How far down the line to aim, metres. Too short and the car saws at the
/// wheel; too long and it cuts every corner.
const LOOKAHEAD: f32 = 22.0;
/// Extra distance looked ahead per m/s of speed.
const LOOKAHEAD_PER_SPEED: f32 = 0.55;
/// Steering gain from bearing error to stick position.
const STEER_GAIN: f32 = 1.8;
/// Skill scales the target speed, so a field of AI cars is not a train.
pub const DEFAULT_SKILL: f32 = 0.88;

/// The fastest this car could hold a corner of the given radius, from the
/// same steering model the sim uses: `R = v (1 + v k) / STEER_MAX`.
/// Solving for `v` gives the positive root of
/// `k v^2 + v - R * STEER_MAX = 0`.
fn corner_speed(radius: f32) -> f32 {
    let k = STEER_FALLOFF;
    let c = radius * STEER_MAX;
    ((1.0 + 4.0 * k * c).sqrt() - 1.0) / (2.0 * k)
}

/// Radius of the turn between three points on the line, metres.
fn radius_ahead(track: &Track, s: f32, span: f32) -> f32 {
    let (a, _) = track.at(s);
    let (b, _) = track.at(s + span);
    let (c, _) = track.at(s + span * 2.0);
    let (ab, bc, ca) = ((b - a).length(), (c - b).length(), (a - c).length());
    let cross = (b - a).x * (c - a).y - (b - a).y * (c - a).x;
    if cross.abs() < 1e-4 {
        return f32::INFINITY;
    }
    ab * bc * ca / (2.0 * cross.abs())
}

/// Drive one car for one tick.
#[must_use]
pub fn chase(track: &Track, car: &Car, skill: f32) -> CarInput {
    let speed = car.speed();
    let loc = track.locate(car.pos);
    let aim_s = loc.s + LOOKAHEAD + speed * LOOKAHEAD_PER_SPEED;
    let (target, _) = track.at(aim_s);

    // Signed bearing from the nose to the target. Positive means the target
    // is to the left, and `steer` is left-positive, so no sign flip here.
    let to = target - car.pos;
    let f = forward(car.yaw);
    let bearing = (f.x * to.y - f.y * to.x).atan2(f.dot(to));
    let steer = (bearing * STEER_GAIN).clamp(-1.0, 1.0);

    // Look far enough ahead to brake in time: roughly the distance covered
    // in two seconds at the current speed.
    let radius = radius_ahead(track, loc.s + speed * 0.8, 24.0);
    let limit = (corner_speed(radius) * skill).min(MAX_SPEED * skill);

    let throttle = if speed > limit * 1.08 {
        -1.0
    } else if speed > limit {
        0.0
    } else {
        1.0
    };

    // A slow corner taken with a dab of handbrake looks like racing and helps
    // the nose round. Only when actually going fast enough to need it.
    let handbrake = radius < 34.0 && speed > limit * 0.9 && speed > 18.0;

    // Spend a boost on anything that looks like a straight, but only when
    // there is room to use it.
    let boost = radius > 150.0 && speed > MAX_SPEED * 0.5 && car.boost_charges > 0 && !car.boosting();

    CarInput { throttle, steer, handbrake, boost }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::castle;

    #[test]
    fn corner_speed_inverts_the_steering_model() {
        for r in [15.0f32, 30.0, 60.0, 120.0] {
            let v = corner_speed(r);
            // Feed it back through the forward model.
            let back = v * (1.0 + v * STEER_FALLOFF) / STEER_MAX;
            assert!((back - r).abs() < 0.5, "radius {r} -> speed {v} -> radius {back}");
        }
    }

    #[test]
    fn radius_is_large_on_the_straight_and_small_in_the_hairpin() {
        let t = castle::track();
        let mut smallest = f32::INFINITY;
        let mut largest: f32 = 0.0;
        let mut s = 0.0;
        while s < t.length() {
            let r = radius_ahead(&t, s, 24.0);
            if r.is_finite() {
                smallest = smallest.min(r);
                largest = largest.max(r);
            }
            s += 5.0;
        }
        assert!(smallest < 60.0, "no real corner found (tightest {smallest:.0} m)");
        assert!(largest > 200.0, "no real straight found (longest {largest:.0} m)");
    }

    /// The AI must not simply drive off. This is also an end-to-end test of
    /// the whole sim: track, car, grip, walls and laps together.
    #[test]
    fn the_ai_gets_round_the_castle() {
        use crate::car::DT;
        use crate::sim::Race;
        let mut race = Race::new(castle::track(), 4, 3);
        race.start_countdown();
        for _ in 0..60 * 200 {
            let inputs: Vec<CarInput> = race
                .racers
                .iter()
                .map(|r| chase(&race.track, &r.car, DEFAULT_SKILL))
                .collect();
            race.step(&inputs, DT);
        }
        for (i, r) in race.racers.iter().enumerate() {
            assert!(r.lap.lap >= 2, "AI {i} only managed {} laps in 200 s", r.lap.lap);
            assert!(
                !race.track.off_track(r.car.pos),
                "AI {i} finished off the track"
            );
        }
    }

    /// A field of identical AI cars should not finish in a dead heat — they
    /// start from different grid slots and interact with the walls.
    #[test]
    fn the_ai_uses_its_boost() {
        use crate::car::DT;
        use crate::sim::Race;
        let mut race = Race::new(castle::track(), 2, 3);
        race.start_countdown();
        for _ in 0..60 * 90 {
            let inputs: Vec<CarInput> = race
                .racers
                .iter()
                .map(|r| chase(&race.track, &r.car, DEFAULT_SKILL))
                .collect();
            race.step(&inputs, DT);
        }
        assert!(
            race.racers.iter().any(|r| r.car.boost_charges < crate::car::BOOST_CHARGES),
            "no AI ever spent a boost charge"
        );
    }
}
