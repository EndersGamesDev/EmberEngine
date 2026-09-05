//! The race: a track, some cars, and the rules that connect them.
//!
//! This is the authority. The server runs it and broadcasts the result; the
//! client runs the same code to predict its own car. Fixed order, fixed step,
//! no RNG — see `car::step` for why that matters.

use glam::Vec2;

use crate::car::{Car, CarInput, OFFROAD_FACTOR, forward};
use crate::powerups::{self, Oil, Pickup, Pulse};
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
    pub finish_time: Option<f32>,
    /// Authoritative recovery bookkeeping; never part of client prediction.
    pub stalled_for: f32,
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
    pub elapsed: f32,
    pub pickups: Vec<Pickup>,
    pub projectiles: Vec<Pulse>,
    pub hazards: Vec<Oil>,
    countdown_left: f32,
}

impl Race {
    /// Lay `count` cars out on a staggered grid behind the start line and
    /// hold them there until the countdown finishes.
    #[must_use]
    pub fn new(track: Track, count: usize, laps_to_win: u32) -> Self {
        let racers = (0..count).map(|i| Self::grid_slot(&track, i)).collect();
        let mut pickups = Vec::new();
        for fraction in [0.12, 0.35, 0.59, 0.82] {
            let (centre, tangent) = track.at(track.length() * fraction);
            let left = Vec2::new(-tangent.y, tangent.x);
            for lane in [-0.5, 0.0, 0.5] {
                pickups.push(Pickup {
                    id: u16::try_from(pickups.len()).unwrap_or(0),
                    pos: centre + left * lane * track.half_width(),
                    respawn_left: 0.0,
                });
            }
        }
        Self {
            track,
            racers,
            tick: 0,
            state: RaceState::Waiting,
            laps_to_win,
            elapsed: 0.0,
            pickups,
            projectiles: Vec::new(),
            hazards: Vec::new(),
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
        let mut car = Car::new(pos, yaw);
        car.vehicle = u8::try_from(i % 3).unwrap_or(0);
        Racer {
            car,
            lap: LapTracker::new(SECTORS, s),
            finish_tick: None,
            finish_time: None,
            stalled_for: 0.0,
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

    #[must_use]
    pub const fn elapsed_seconds(&self) -> f32 {
        self.elapsed
    }

    /// Put a stuck car back on the nearest section, never further along the
    /// course. Its checkpoint sequence and race progress remain untouched.
    pub fn recover(&mut self, index: usize) {
        let Some(racer) = self.racers.get_mut(index) else {
            return;
        };
        let loc = self.track.locate(racer.car.pos);
        let (pos, tangent) = self.track.at(loc.s);
        racer.car.pos = pos;
        racer.car.vel = tangent * 6.0;
        racer.car.yaw = tangent.x.atan2(tangent.y);
        racer.car.steer_angle = 0.0;
        racer.car.drift = 0.0;
        racer.car.drift_charge = 0.0;
        racer.car.boost_left = 0.0;
        racer.car.oil_left = 0.0;
        racer.car.hit_left = 1.5;
        racer.stalled_for = 0.0;
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

        self.elapsed += dt;
        for pickup in &mut self.pickups {
            pickup.respawn_left = (pickup.respawn_left - dt).max(0.0);
        }
        // Defensive actions resolve before attacks for fair same-tick blocks.
        for (i, input) in inputs.iter().enumerate().take(self.racers.len()) {
            if input.use_item
                && matches!(self.racers[i].car.item, powerups::SHIELD | powerups::GRIP)
            {
                self.use_item(i);
            }
        }
        for (i, input) in inputs.iter().enumerate().take(self.racers.len()) {
            if input.use_item {
                self.use_item(i);
            }
        }

        let idle = CarInput::default();
        for i in 0..self.racers.len() {
            let input = if self.racers[i].finish_tick.is_some() {
                // A finished car follows the road while braking to rest. A
                // free coast can run off the next bend while others finish.
                let car = &self.racers[i].car;
                CarInput {
                    steer: crate::ai::chase(&self.track, car, 0.45).steer,
                    throttle: if car.speed() > 1.0 { -0.65 } else { 0.0 },
                    ..idle
                }
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

            let loc = self.track.locate(self.racers[i].car.pos);
            let stalled = self.racers[i].finish_tick.is_none()
                && input.throttle > 0.2
                && ((self.racers[i].car.speed() < 2.5 && self.racers[i].car.hit_left <= 0.0)
                    || forward(self.racers[i].car.yaw).dot(loc.tangent) < -0.45);
            if stalled {
                self.racers[i].stalled_for += dt;
                if self.racers[i].stalled_for > 3.0 {
                    self.recover(i);
                }
            } else {
                self.racers[i].stalled_for = 0.0;
            }
        }

        self.resolve_contacts();
        self.step_combat(dt);
        self.collect_pickups();

        for i in 0..self.racers.len() {
            let s = self.track.locate(self.racers[i].car.pos).s;
            let racer = &mut self.racers[i];
            if racer.finish_tick.is_none()
                && racer.lap.update(&self.track, s)
                && racer.lap.lap >= self.laps_to_win
            {
                racer.finish_tick = Some(self.tick);
                racer.finish_time = Some(self.elapsed);
            }
        }

        if !self.racers.is_empty() && self.racers.iter().all(|r| r.finish_tick.is_some()) {
            self.state = RaceState::Finished;
        }
    }

    fn collect_pickups(&mut self) {
        let order = self.standings();
        for (i, racer) in self.racers.iter_mut().enumerate() {
            if racer.car.item != 0 || racer.finish_tick.is_some() {
                continue;
            }
            for pickup in &mut self.pickups {
                if pickup.respawn_left <= 0.0
                    && racer.car.pos.distance_squared(pickup.pos) < powerups::PICKUP_RADIUS.powi(2)
                {
                    let place = order.iter().position(|&n| n == i).unwrap_or(0);
                    racer.car.item = powerups::draw(self.tick, pickup.id, i, place, order.len());
                    pickup.respawn_left = powerups::PICKUP_RESPAWN;
                    break;
                }
            }
        }
    }

    fn use_item(&mut self, index: usize) {
        if self.racers[index].finish_tick.is_some() {
            return;
        }
        let item = self.racers[index].car.item;
        self.racers[index].car.item = 0;
        let car = &mut self.racers[index].car;
        match item {
            powerups::NITRO => car.boost_left = car.boost_left.max(2.5),
            powerups::SHIELD => car.shield_left = 7.0,
            powerups::GRIP => {
                car.grip_left = 6.0;
                car.oil_left = 0.0;
            }
            powerups::OIL => self.hazards.push(Oil {
                owner: u8::try_from(index).unwrap_or(0),
                pos: car.pos - forward(car.yaw) * 4.5,
                life_left: 9.0,
            }),
            powerups::PULSE => {
                let order = self.standings();
                let target = order
                    .iter()
                    .position(|&i| i == index)
                    .and_then(|place| place.checked_sub(1))
                    .map(|place| order[place])
                    .filter(|&i| self.racers[i].finish_tick.is_none());
                if let Some(target) = target {
                    let car = &self.racers[index].car;
                    self.projectiles.push(Pulse {
                        owner: u8::try_from(index).unwrap_or(0),
                        target: u8::try_from(target).unwrap_or(0),
                        pos: car.pos + forward(car.yaw) * 2.4,
                        life_left: 5.0,
                    });
                } else {
                    // Leaders retain a Pulse until somebody is ahead of them.
                    self.racers[index].car.item = item;
                }
            }
            _ => {}
        }
    }

    fn hit(car: &mut Car, oil: bool) {
        if car.hit_left > 0.0 || (oil && car.grip_left > 0.0) {
            return;
        }
        if car.shield_left > 0.0 {
            car.shield_left = 0.0;
            car.hit_left = 1.0;
            return;
        }
        car.vel *= if oil { 0.82 } else { 0.58 };
        car.boost_left = 0.0;
        car.drift_charge = 0.0;
        car.hit_left = 2.5;
        if oil {
            car.oil_left = 1.7;
        }
    }

    fn step_combat(&mut self, dt: f32) {
        for projectile in &mut self.projectiles {
            projectile.life_left -= dt;
            if projectile.life_left <= 0.0 {
                continue;
            }
            let Some(racer) = self.racers.get_mut(usize::from(projectile.target)) else {
                projectile.life_left = 0.0;
                continue;
            };
            if racer.finish_tick.is_some() {
                projectile.life_left = 0.0;
                continue;
            }
            let delta = racer.car.pos - projectile.pos;
            let travel = powerups::PULSE_SPEED * dt;
            if delta.length_squared() <= (travel + 1.5).powi(2) {
                Self::hit(&mut racer.car, false);
                projectile.life_left = 0.0;
            } else {
                projectile.pos += delta.normalize_or_zero() * travel;
            }
        }
        self.projectiles.retain(|p| p.life_left > 0.0);
        for hazard in &mut self.hazards {
            hazard.life_left -= dt;
            if hazard.life_left <= 0.0 {
                continue;
            }
            for (i, racer) in self.racers.iter_mut().enumerate() {
                if i != usize::from(hazard.owner)
                    && racer.finish_tick.is_none()
                    && racer.car.pos.distance_squared(hazard.pos) < powerups::OIL_RADIUS.powi(2)
                {
                    Self::hit(&mut racer.car, true);
                }
            }
        }
        self.hazards.retain(|h| h.life_left > 0.0);
    }

    /// Two discs along each chassis approximate its full length. Iterated
    /// mass-weighted separation prevents a pile-up from becoming one object.
    fn resolve_contacts(&mut self) {
        for _ in 0..3 {
            for a in 0..self.racers.len() {
                for b in a + 1..self.racers.len() {
                    let (before, after) = self.racers.split_at_mut(b);
                    if before[a].finish_tick.is_some() || after[0].finish_tick.is_some() {
                        continue;
                    }
                    let ca = &mut before[a].car;
                    let cb = &mut after[0].car;
                    if ca.pos.distance_squared(cb.pos) > 25.0 {
                        continue;
                    }
                    let (mut deepest, mut normal) = (0.0, Vec2::X);
                    for offset_a in [-1.05, 1.05] {
                        for offset_b in [-1.05, 1.05] {
                            let delta = cb.pos + forward(cb.yaw) * offset_b
                                - ca.pos
                                - forward(ca.yaw) * offset_a;
                            let distance = delta.length();
                            let depth = 2.05 - distance;
                            if depth > deepest {
                                deepest = depth;
                                normal = if distance > 0.001 {
                                    delta / distance
                                } else {
                                    Vec2::X
                                };
                            }
                        }
                    }
                    if deepest <= 0.0 {
                        continue;
                    }
                    let inv_a = 1.0 / ca.profile().mass;
                    let inv_b = 1.0 / cb.profile().mass;
                    let total = inv_a + inv_b;
                    ca.pos -= normal * (deepest + 0.002) * inv_a / total;
                    cb.pos += normal * (deepest + 0.002) * inv_b / total;
                    let closing = (cb.vel - ca.vel).dot(normal);
                    if closing < 0.0 {
                        let impulse = -closing * 1.12 / total;
                        ca.vel -= normal * impulse * inv_a;
                        cb.vel += normal * impulse * inv_b;
                    }
                }
            }
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
        CarInput {
            throttle: 1.0,
            steer: (ang * 2.0).clamp(-1.0, 1.0),
            handbrake: false,
            boost: false,
            use_item: false,
        }
    }

    fn race_for(ticks: u32, laps: u32) -> Race {
        let mut race = Race::new(test_track(), 2, laps);
        race.start_countdown();
        for _ in 0..ticks {
            let inputs: Vec<CarInput> = (0..race.racers.len())
                .map(|i| chase_input(&race, i))
                .collect();
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
        assert!(
            (59..=61).contains(&total),
            "144 Hz produced {total} ticks in a second"
        );

        // A 30 Hz frame is longer than a tick: two ticks each.
        let mut fs = FixedStep::default();
        let mut total = 0;
        for _ in 0..30 {
            total += fs.ticks(1.0 / 30.0);
        }
        assert!(
            (59..=61).contains(&total),
            "30 Hz produced {total} ticks in a second"
        );
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
        assert_eq!(
            n,
            (MAX_CATCHUP / DT) as u32,
            "a 10 s stall yielded {n} ticks"
        );
        assert!(n > 0, "a stall should still advance the sim a little");
    }

    #[test]
    fn cars_start_on_the_grid_and_on_the_track() {
        let race = Race::new(test_track(), 8, 3);
        for (i, r) in race.racers.iter().enumerate() {
            assert!(
                !race.track.off_track(r.car.pos),
                "grid slot {i} is off the track"
            );
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
            CarInput {
                throttle: 1.0,
                steer: 0.0,
                handbrake: false,
                boost: true,
                use_item: false,
            };
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
        assert!(
            a.finish_tick.unwrap() <= b.finish_tick.unwrap(),
            "standings out of order"
        );
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
        let ram = CarInput {
            throttle: 1.0,
            steer: 1.0,
            handbrake: false,
            boost: true,
            use_item: false,
        };
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
        assert_eq!(
            sorted.len(),
            race.racers.len(),
            "standings lost or duplicated a racer"
        );
    }

    #[test]
    fn the_whole_race_is_deterministic() {
        let a = race_for(60 * 45, 3);
        let b = race_for(60 * 45, 3);
        for i in 0..a.racers.len() {
            assert_eq!(
                a.racers[i].car.pos.to_array(),
                b.racers[i].car.pos.to_array()
            );
            assert_eq!(a.racers[i].lap.lap, b.racers[i].lap.lap);
        }
        assert_eq!(a.tick, b.tick);
    }

    fn active_race(count: usize) -> Race {
        let mut race = Race::new(test_track(), count, 3);
        race.state = RaceState::Racing;
        race
    }

    #[test]
    fn pickup_fills_one_slot_and_respawns_without_replacing_items() {
        let mut race = active_race(1);
        race.racers[0].car.pos = race.pickups[0].pos;
        race.step(&[CarInput::default()], DT);
        let first = race.racers[0].car.item;
        assert!((1..=5).contains(&first));
        assert_eq!(race.pickups[0].respawn_left, powerups::PICKUP_RESPAWN);
        for _ in 0..310 {
            race.step(&[CarInput::default()], DT);
        }
        assert_eq!(race.racers[0].car.item, first);
        assert_eq!(race.pickups[0].respawn_left, 0.0);
        race.racers[0].car.item = 0;
        race.step(&[CarInput::default()], DT);
        assert_ne!(race.racers[0].car.item, 0);
    }

    #[test]
    fn each_item_has_an_effect_and_consumes_one_slot() {
        for item in 1..=5 {
            let mut race = active_race(2);
            race.racers[1].car.item = item;
            race.racers[0].lap.progress = 20.0;
            race.use_item(1);
            assert_eq!(race.racers[1].car.item, 0, "item {item} was not consumed");
            match item {
                powerups::NITRO => assert!(race.racers[1].car.boost_left > 2.0),
                powerups::SHIELD => assert!(race.racers[1].car.shield_left > 6.0),
                powerups::PULSE => assert_eq!(race.projectiles.len(), 1),
                powerups::OIL => assert_eq!(race.hazards.len(), 1),
                powerups::GRIP => assert!(race.racers[1].car.grip_left > 5.0),
                _ => unreachable!(),
            }
            race.use_item(1);
            assert!(race.projectiles.len() <= 1 && race.hazards.len() <= 1);
        }
    }

    #[test]
    fn shield_blocks_one_hit_and_recovery_prevents_attack_chains() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        car.vel = Vec2::Y * 30.0;
        car.shield_left = 5.0;
        Race::hit(&mut car, false);
        assert_eq!(car.speed(), 30.0);
        assert_eq!(car.shield_left, 0.0);
        car.hit_left = 0.0;
        Race::hit(&mut car, false);
        let hit_speed = car.speed();
        assert!(hit_speed < 20.0 && hit_speed > 10.0);
        Race::hit(&mut car, false);
        assert_eq!(car.speed(), hit_speed, "second hit bypassed recovery");
        car.hit_left = 0.0;
        car.grip_left = 4.0;
        Race::hit(&mut car, true);
        assert_eq!(car.oil_left, 0.0);
        assert_eq!(car.speed(), hit_speed);
    }

    #[test]
    fn pulse_reaches_target_and_oil_expires() {
        let mut race = active_race(2);
        race.racers[0].car.pos = Vec2::ZERO;
        race.racers[1].car.pos = Vec2::Y * 20.0;
        race.racers[1].car.vel = Vec2::Y * 25.0;
        race.projectiles.push(Pulse {
            owner: 0,
            target: 1,
            pos: Vec2::ZERO,
            life_left: 5.0,
        });
        for _ in 0..40 {
            race.step_combat(DT);
        }
        assert_eq!(race.projectiles, []);
        assert!(race.racers[1].car.speed() < 20.0);
        race.hazards.push(Oil {
            owner: 0,
            pos: race.racers[1].car.pos,
            life_left: 0.2,
        });
        for _ in 0..15 {
            race.step_combat(DT);
        }
        assert_eq!(race.hazards, []);
    }

    #[test]
    fn expired_attacks_do_not_hit_and_same_tick_shield_blocks() {
        let mut race = active_race(2);
        let pos = race.racers[0].car.pos;
        race.racers[0].car.vel = forward(race.racers[0].car.yaw) * 30.0;
        race.projectiles.push(Pulse {
            owner: 1,
            target: 0,
            pos,
            life_left: DT * 0.5,
        });
        race.hazards.push(Oil {
            owner: 1,
            pos,
            life_left: DT * 0.5,
        });
        race.step_combat(DT);
        assert_eq!(race.racers[0].car.hit_left, 0.0);
        assert!((race.racers[0].car.speed() - 30.0).abs() < 0.001);
        race.racers[0].car.item = powerups::SHIELD;
        race.projectiles.push(Pulse {
            owner: 1,
            target: 0,
            pos,
            life_left: 5.0,
        });
        race.step(
            &[CarInput {
                use_item: true,
                ..CarInput::default()
            }],
            DT,
        );
        assert_eq!(race.racers[0].car.shield_left, 0.0);
        assert!(
            race.racers[0].car.speed() > 25.0,
            "shield was applied after the attack"
        );
        assert!(race.racers[0].car.hit_left > 0.0);
    }

    #[test]
    fn contacts_separate_cars_and_exchange_momentum_without_explosion() {
        let mut race = active_race(2);
        race.racers[0].car = Car::new(Vec2::ZERO, 0.0);
        race.racers[1].car = Car::new(Vec2::new(1.0, 0.0), 0.0);
        race.racers[0].car.vel = Vec2::X * 10.0;
        race.resolve_contacts();
        assert!(race.racers[0].car.pos.distance(race.racers[1].car.pos) >= 2.04);
        assert!(race.racers[1].car.vel.x > 0.0);
        assert!(race.racers.iter().all(|r| r.car.speed() <= 10.0));
        let momentum = race.racers[0].car.vel + race.racers[1].car.vel;
        assert!((momentum.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn recovery_keeps_checkpoint_progress_and_selected_chassis() {
        let mut race = active_race(1);
        race.racers[0].car.vehicle = 2;
        race.racers[0].lap.lap = 2;
        race.racers[0].lap.progress = 500.0;
        race.racers[0].car.pos += Vec2::new(5.0, 5.0);
        let before = race.track.locate(race.racers[0].car.pos).s;
        race.recover(0);
        assert!(
            race.track
                .delta_s(race.track.locate(race.racers[0].car.pos).s, before)
                .abs()
                < 0.2
        );
        assert_eq!(race.racers[0].lap.lap, 2);
        assert_eq!(race.racers[0].lap.progress, 500.0);
        assert_eq!(race.racers[0].car.vehicle, 2);
        assert!(!race.track.off_track(race.racers[0].car.pos));
    }

    #[test]
    fn eight_drivers_all_finish_three_laps_with_deterministic_items() {
        let play = || {
            let mut race = Race::new(crate::castle::track(), 8, 3);
            race.start_countdown();
            for _ in 0..60 * 260 {
                let inputs: Vec<_> = race
                    .racers
                    .iter()
                    .map(|r| crate::ai::chase(&race.track, &r.car, crate::ai::DEFAULT_SKILL))
                    .collect();
                race.step(&inputs, DT);
                if race.state == RaceState::Finished {
                    break;
                }
            }
            race
        };
        let a = play();
        let b = play();
        assert_eq!(
            a.state,
            RaceState::Finished,
            "laps: {:?}",
            a.racers.iter().map(|r| r.lap.lap).collect::<Vec<_>>()
        );
        assert_eq!(a.standings(), b.standings());
        assert_eq!(a.pickups, b.pickups);
        assert_eq!(a.projectiles, b.projectiles);
        assert_eq!(a.hazards, b.hazards);
        for (left, right) in a.racers.iter().zip(&b.racers) {
            assert_eq!(left.car.pos, right.car.pos);
            assert_eq!(left.car.item, right.car.item);
            assert_eq!(left.finish_tick, right.finish_tick);
            assert_eq!(left.finish_time, right.finish_time);
            assert!(left.finish_time.is_some_and(|time| time > 30.0));
        }
    }
}
