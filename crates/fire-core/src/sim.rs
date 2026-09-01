//! The race: a track, some cars, and the rules that connect them.
//!
//! This is the authority. The server runs it and broadcasts the result; the
//! client runs the same code to predict its own car. Fixed order, fixed step,
//! no RNG — see `car::step` for why that matters.

use glam::Vec2;

use crate::car::{Car, CarInput, OFFROAD_FACTOR};
use crate::track::{LapTracker, Track};

/// Checkpoint sectors per lap. Enough that a car cannot skip half the circuit
/// by cutting a corner, few enough that a legitimate racing line never misses
/// one.
pub const SECTORS: u16 = 12;

/// Beyond the racing surface the car keeps going, on bad grip, until it hits
/// the courtyard wall this far out. Without the margin the track edge is an
/// invisible kerb that stops a drift dead.
pub const WALL_MARGIN: f32 = 6.0;

/// How hard the wall pushes back, and how much speed it costs. A wall that
/// merely clamps position lets a car grind along it at full speed, which is
/// faster than driving the corner.
pub const WALL_RESTITUTION: f32 = 0.35;

/// Seconds of countdown before the lights go out.
pub const COUNTDOWN_SECS: f32 = 3.0;

/// Turns a variable frame delta into whole simulation ticks.
///
/// `EmberGame::update` is handed real wall-clock time — vsync-bound, and
/// clamped to 100 ms after a stall — not a fixed 1/60. Stepping the sim by
/// that directly means a 144 Hz machine and a 60 Hz one integrate different
/// numbers of times with different `dt`, and, far worse, that a client
/// predicting at render rate diverges from a server ticking at exactly `DT`.
/// The prediction in `fire`'s online mode replays inputs at `DT`, so if the
/// forward prediction used anything else the two would never agree.
///
/// So: accumulate, and only ever advance the simulation in whole `DT` steps.
#[derive(Default, Debug)]
pub struct FixedStep {
    acc: f32,
}

/// Ceiling on accumulated time, seconds.
///
/// Past this we drop the backlog rather than run a burst of catch-up ticks — a
/// spiral where each frame's catch-up makes the next frame later is worse than
/// a visible skip.
pub const MAX_CATCHUP: f32 = 0.25;

impl FixedStep {
    /// Feed a frame delta; returns how many `DT` ticks to run now.
    // The capped accumulator makes these casts bounded; preserve the shared simulation arithmetic.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    pub fn ticks(&mut self, dt: f32) -> u32 {
        if !dt.is_finite() || dt < 0.0 {
            return 0;
        }
        self.acc = (self.acc + dt).min(MAX_CATCHUP);
        let n = (self.acc / crate::car::DT) as u32;
        self.acc -= n as f32 * crate::car::DT;
        n
    }

    /// Fraction of a tick left over, for interpolating the render pose.
    #[must_use]
    pub const fn alpha(&self) -> f32 {
        self.acc / crate::car::DT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaceState {
    /// Waiting for enough players; cars are parked on the grid.
    Waiting,
    /// Lights out in `ticks` more ticks. Input is ignored.
    Countdown,
    Racing,
    Finished,
}

#[derive(Clone, Debug)]
pub struct Racer {
    pub car: Car,
    pub lap: LapTracker,
    /// Set on the tick this racer completes the final lap.
    pub finish_tick: Option<u64>,
}

impl Racer {
    /// Ordering key for the live standings: laps and distance combined, which
    /// `LapTracker::progress` already is. Finished racers sort by finish tick.
    #[must_use]
    pub const fn progress(&self) -> f32 {
        self.lap.progress
    }
}

pub struct Race {
    pub track: Track,
    pub racers: Vec<Racer>,
    pub tick: u64,
    pub state: RaceState,
    pub laps_to_win: u32,
    countdown_left: f32,
}

impl Race {
    /// Lay `count` cars out on a staggered grid behind the start line and
    /// hold them there until the countdown finishes.
    #[must_use]
    pub fn new(track: Track, count: usize, laps_to_win: u32) -> Self {
        let racers = (0..count).map(|i| Self::grid_slot(&track, i)).collect();
        Self {
            track,
            racers,
            tick: 0,
            state: RaceState::Waiting,
            laps_to_win,
            countdown_left: COUNTDOWN_SECS,
        }
    }

    /// Grid position `i`: two columns, staggered back down the centreline
    /// from the start line. Placed by arc length so the grid follows the
    /// track's curvature instead of poking through a wall on a bend.
    fn grid_slot(track: &Track, i: usize) -> Racer {
        let len = track.length();
        let row = i / 2;
        let side = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
        // Behind the line, so the first thing anyone crosses is the line.
        // Race grids are tiny; preserve the shared simulation conversion and operation order.
        #[allow(clippy::cast_precision_loss)]
        let s = (len - 12.0 - row as f32 * 9.0).rem_euclid(len);
        let (centre, tangent) = track.at(s);
        let left = Vec2::new(-tangent.y, tangent.x);
        let pos = centre + left * side * (track.half_width() * 0.42);
        // Heading is the tangent: yaw such that forward(yaw) == tangent.
        let yaw = tangent.x.atan2(tangent.y);
        Racer {
            car: Car::new(pos, yaw),
            lap: LapTracker::new(SECTORS, s),
            finish_tick: None,
        }
    }

    pub fn start_countdown(&mut self) {
        if self.state == RaceState::Waiting {
            self.state = RaceState::Countdown;
            self.countdown_left = COUNTDOWN_SECS;
        }
    }

    #[must_use]
    pub const fn countdown_left(&self) -> f32 {
        self.countdown_left
    }

    /// Advance one tick. `inputs` is parallel to `racers`; a short slice
    /// leaves the remaining cars coasting, which is what a dropped connection
    /// should look like.
    pub fn step(&mut self, inputs: &[CarInput], dt: f32) {
        self.tick += 1;

        match self.state {
            RaceState::Waiting | RaceState::Finished => return,
            RaceState::Countdown => {
                self.countdown_left -= dt;
                if self.countdown_left <= 0.0 {
                    self.countdown_left = 0.0;
                    self.state = RaceState::Racing;
                }
                // Cars are held on the grid: no input, no coasting.
                return;
            }
            RaceState::Racing => {}
        }

        let idle = CarInput::default();
        for i in 0..self.racers.len() {
            let input = if self.racers[i].finish_tick.is_some() {
                // A finished car stops taking orders but keeps rolling, so it
                // coasts off the line instead of stopping dead on it.
                idle
            } else {
                *inputs.get(i).unwrap_or(&idle)
            };

            let loc = self.track.locate(self.racers[i].car.pos);
            let over = loc.lateral.abs() - self.track.half_width();
            let grip = if over > 0.0 { OFFROAD_FACTOR } else { 1.0 };

            self.racers[i].car.step(&input, grip, dt);

            // Wall: only once past the run-off. Push the car back inside and
            // kill the component of velocity going into the wall, so grinding
            // along it is slower than driving the corner properly.
            let loc = self.track.locate(self.racers[i].car.pos);
            let limit = self.track.half_width() + WALL_MARGIN;
            if loc.lateral.abs() > limit {
                let inward = if loc.lateral > 0.0 { -1.0 } else { 1.0 };
                // Left of the tangent is (-t.y, t.x); scale by which side.
                let normal = Vec2::new(-loc.tangent.y, loc.tangent.x) * inward;
                let depth = loc.lateral.abs() - limit;
                let car = &mut self.racers[i].car;
                car.pos += normal * depth;
                let into = car.vel.dot(normal);
                if into < 0.0 {
                    car.vel -= normal * into * (1.0 + WALL_RESTITUTION);
                }
            }

            let s = self.track.locate(self.racers[i].car.pos).s;
            let racer = &mut self.racers[i];
            if racer.lap.update(&self.track, s) && racer.lap.lap >= self.laps_to_win {
                racer.finish_tick = Some(self.tick);
            }
        }

        if !self.racers.is_empty() && self.racers.iter().all(|r| r.finish_tick.is_some()) {
            self.state = RaceState::Finished;
        }
    }

    /// Racer indices in finishing order: those who have finished first, by
    /// finish tick, then the rest by distance covered.
    #[must_use]
    pub fn standings(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.racers.len()).collect();
        order.sort_by(|&a, &b| {
            let (ra, rb) = (&self.racers[a], &self.racers[b]);
            match (ra.finish_tick, rb.finish_tick) {
                (Some(x), Some(y)) => x.cmp(&y).then(a.cmp(&b)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                // Ties broken by index so the order is total and stable —
                // otherwise two cars abreast can swap places every tick.
                (None, None) => rb
                    .progress()
                    .partial_cmp(&ra.progress())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.cmp(&b)),
            }
        });
        order
    }
}

#[cfg(test)]
// Test durations are bounded constants; casts preserve the production formulas under test.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
mod tests {
    use super::*;
    use crate::car::DT;
    use crate::track::Track;

    fn test_track() -> Track {
        Track::new(
            vec![
                Vec2::new(90.0, 0.0),
                Vec2::new(60.0, 70.0),
                Vec2::new(-40.0, 80.0),
                Vec2::new(-90.0, 20.0),
                Vec2::new(-70.0, -60.0),
                Vec2::new(20.0, -80.0),
            ],
            12.0,
        )
    }

    /// An AI that just follows the racing line, so the sim can be exercised
    /// without a human. Aims at a point ahead on the centreline.
    fn chase_input(race: &Race, i: usize) -> CarInput {
        let car = &race.racers[i].car;
        let loc = race.track.locate(car.pos);
        let (target, _) = race.track.at(loc.s + 18.0);
        let to = target - car.pos;
        let f = crate::car::forward(car.yaw);
        // Signed angle from the nose to the target.
        let ang = (f.x * to.y - f.y * to.x).atan2(f.dot(to));
        CarInput { throttle: 1.0, steer: (ang * 2.0).clamp(-1.0, 1.0), handbrake: false, boost: false }
    }

    fn race_for(ticks: u32, laps: u32) -> Race {
        let mut race = Race::new(test_track(), 2, laps);
        race.start_countdown();
        for _ in 0..ticks {
            let inputs: Vec<CarInput> = (0..race.racers.len()).map(|i| chase_input(&race, i)).collect();
            race.step(&inputs, DT);
        }
        race
    }

    #[test]
    fn the_fixed_step_only_ever_yields_whole_ticks() {
        let mut fs = FixedStep::default();
        // A 144 Hz frame is shorter than a tick: most frames run none, some
        // run one, and over a second it must total exactly 60.
        let mut total = 0;
        for _ in 0..144 {
            total += fs.ticks(1.0 / 144.0);
        }
        assert!((59..=61).contains(&total), "144 Hz produced {total} ticks in a second");

        // A 30 Hz frame is longer than a tick: two ticks each.
        let mut fs = FixedStep::default();
        let mut total = 0;
        for _ in 0..30 {
            total += fs.ticks(1.0 / 30.0);
        }
        assert!((59..=61).contains(&total), "30 Hz produced {total} ticks in a second");
    }

    /// A long stall must not be repaid as a burst that makes the next frame
    /// later still.
    #[test]
    fn the_fixed_step_refuses_to_spiral() {
        let mut fs = FixedStep::default();
        let n = fs.ticks(10.0);
        assert!(
            n as f32 * DT <= MAX_CATCHUP + DT,
            "a 10 s stall asked for {n} ticks"
        );
    }

    /// A non-finite or negative delta is a broken clock, not a long frame:
    /// run nothing, and leave the accumulator untouched so the next good
    /// frame behaves normally.
    #[test]
    fn the_fixed_step_survives_hostile_deltas() {
        let mut fs = FixedStep::default();
        assert_eq!(fs.ticks(f32::NAN), 0);
        assert_eq!(fs.ticks(f32::INFINITY), 0);
        assert_eq!(fs.ticks(-1.0), 0);
        // ...and a normal frame afterwards still works.
        assert_eq!(fs.ticks(DT * 3.0), 3);
    }

    /// A long but finite stall is repaid up to the ceiling and no further.
    #[test]
    fn a_long_stall_is_capped_not_dropped() {
        let mut fs = FixedStep::default();
        let n = fs.ticks(10.0);
        assert_eq!(n, (MAX_CATCHUP / DT) as u32, "a 10 s stall yielded {n} ticks");
        assert!(n > 0, "a stall should still advance the sim a little");
    }

    #[test]
    fn cars_start_on_the_grid_and_on_the_track() {
        let race = Race::new(test_track(), 8, 3);
        for (i, r) in race.racers.iter().enumerate() {
            assert!(!race.track.off_track(r.car.pos), "grid slot {i} is off the track");
        }
        // No two cars share a slot.
        for i in 0..race.racers.len() {
            for j in (i + 1)..race.racers.len() {
                let d = (race.racers[i].car.pos - race.racers[j].car.pos).length();
                assert!(d > 2.0, "grid slots {i} and {j} overlap ({d:.2} m apart)");
            }
        }
    }

    #[test]
    fn the_countdown_holds_the_cars_still() {
        let mut race = Race::new(test_track(), 4, 3);
        race.start_countdown();
        let before: Vec<Vec2> = race.racers.iter().map(|r| r.car.pos).collect();
        let flat_out = vec![
            CarInput { throttle: 1.0, steer: 0.0, handbrake: false, boost: true };
            4
        ];
        for _ in 0..(COUNTDOWN_SECS * 60.0) as u32 - 2 {
            race.step(&flat_out, DT);
        }
        assert_eq!(race.state, RaceState::Countdown);
        for (i, r) in race.racers.iter().enumerate() {
            assert_eq!(r.car.pos, before[i], "car {i} jumped the start");
        }
        // And the lights do eventually go out.
        for _ in 0..10 {
            race.step(&flat_out, DT);
        }
        assert_eq!(race.state, RaceState::Racing);
    }

    #[test]
    fn a_car_following_the_line_completes_laps() {
        let race = race_for(60 * 90, 3);
        assert!(
            race.racers[0].lap.lap >= 1,
            "90 s of chasing the line produced {} laps",
            race.racers[0].lap.lap
        );
    }

    #[test]
    fn the_race_finishes_and_ranks() {
        let race = race_for(60 * 240, 1);
        assert_eq!(race.state, RaceState::Finished, "race never finished");
        let order = race.standings();
        assert_eq!(order.len(), 2);
        let (a, b) = (&race.racers[order[0]], &race.racers[order[1]]);
        assert!(a.finish_tick.unwrap() <= b.finish_tick.unwrap(), "standings out of order");
    }

    /// The wall must actually contain the car — a racer who holds full lock
    /// into the barrier should not end up in the next county.
    #[test]
    fn the_wall_contains_the_car() {
        let mut race = Race::new(test_track(), 1, 3);
        race.start_countdown();
        for _ in 0..200 {
            race.step(&[CarInput::default()], DT);
        }
        let ram = CarInput { throttle: 1.0, steer: 1.0, handbrake: false, boost: true };
        for _ in 0..60 * 30 {
            race.step(&[ram], DT);
            let lat = race.track.locate(race.racers[0].car.pos).lateral.abs();
            assert!(
                lat <= race.track.half_width() + WALL_MARGIN + 1.0,
                "car escaped to {lat:.1} m from the line"
            );
        }
    }

    #[test]
    fn standings_are_a_total_stable_order() {
        let race = race_for(60 * 40, 3);
        let a = race.standings();
        let b = race.standings();
        assert_eq!(a, b, "standings are not deterministic");
        let mut sorted = a;
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), race.racers.len(), "standings lost or duplicated a racer");
    }

    #[test]
    fn the_whole_race_is_deterministic() {
        let a = race_for(60 * 45, 3);
        let b = race_for(60 * 45, 3);
        for i in 0..a.racers.len() {
            assert_eq!(a.racers[i].car.pos.to_array(), b.racers[i].car.pos.to_array());
            assert_eq!(a.racers[i].lap.lap, b.racers[i].lap.lap);
        }
        assert_eq!(a.tick, b.tick);
    }
}
