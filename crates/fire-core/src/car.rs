//! Car simulation: acceleration, drift, and a limited-charge boost.
//!
//! The one idea everything else follows from: **velocity is a world-space
//! vector, stored independently of the car's heading.** The old code advanced
//! the car as `pos += forward(yaw) * speed * dt`, which defines velocity to be
//! parallel to the nose — under that model a drift is not merely absent, it is
//! unrepresentable. Here the nose can point somewhere the car is not going,
//! and the gap between the two *is* the slip angle.
//!
//! Determinism: fixed step, fixed update order, no RNG. Both the client's
//! prediction and the authoritative server run this exact function, so any
//! change to the order of operations below is a protocol change.

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Simulation rate. The step is a constant, not a parameter, because the
/// server and the client must agree tick for tick.
pub const TICK_HZ: u32 = 60;
// Sixty is exactly representable; preserve this shared simulation expression.
#[allow(clippy::cast_precision_loss)]
pub const DT: f32 = 1.0 / TICK_HZ as f32;

// ---- tuning ---------------------------------------------------------------
// Dimensioned for a ~4 m car on a ~12 m wide, ~700 m circuit.

/// Top speed under engine power alone, m/s (~160 km/h).
pub const MAX_SPEED: f32 = 45.0;
/// Forward acceleration at full throttle, m/s^2.
pub const ENGINE_ACCEL: f32 = 16.0;
/// Deceleration when braking against forward motion, m/s^2.
pub const BRAKE_ACCEL: f32 = 26.0;
/// Reverse is deliberately feeble — this is a racing game, not a parking sim.
pub const REVERSE_MAX: f32 = 9.0;
/// Quadratic drag, chosen so engine force balances drag exactly at
/// `MAX_SPEED`: `ENGINE_ACCEL = DRAG * MAX_SPEED^2`.
pub const DRAG: f32 = ENGINE_ACCEL / (MAX_SPEED * MAX_SPEED);
/// Rolling resistance when coasting, as an exponential rate (1/s).
pub const ROLL_RESIST: f32 = 0.55;

/// Lateral grip as an exponential decay rate (1/s). High: the car goes where
/// it points. This is the number that separates "on rails" from "on ice".
pub const GRIP: f32 = 9.0;
/// Lateral grip while drifting. Low enough to hold a slide, high enough that
/// the slide still scrubs speed and eventually recovers on its own.
pub const GRIP_DRIFT: f32 = 1.6;
/// Off the racing surface, grip and drive are scaled by this.
pub const OFFROAD_FACTOR: f32 = 0.45;

/// Yaw rate at a standstill, rad/s.
pub const STEER_MAX: f32 = 2.4;
/// Steering authority falls off with speed: full lock at 45 m/s would be a
/// spin every corner. `rate = STEER_MAX / (1 + speed * STEER_FALLOFF)`.
pub const STEER_FALLOFF: f32 = 0.045;
/// While drifting the driver needs *more* yaw authority, not less — that is
/// what counter-steering is, and without it a slide cannot be held or exited.
pub const STEER_DRIFT_BONUS: f32 = 1.7;

/// Hard cap on slip angle, radians.
///
/// Beyond this the car is spinning rather than drifting, so yaw is bled back
/// toward the velocity direction. This is the guard against the classic
/// arcade failure where dropping rear grip turns every handbrake tap into an
/// unrecoverable pirouette.
pub const MAX_SLIP: f32 = 0.95; // ~54 degrees
/// How hard yaw is pulled back once past `MAX_SLIP` (1/s).
pub const SLIP_RECOVER: f32 = 6.0;
/// Below this speed there is no meaningful slip angle and the maths is noise.
pub const SLIP_MIN_SPEED: f32 = 2.5;

pub const BOOST_CHARGES: u8 = 3;
pub const BOOST_SECS: f32 = 1.8;
pub const BOOST_ACCEL_MULT: f32 = 1.9;
pub const BOOST_SPEED_MULT: f32 = 1.3;

/// Held driver intents for one tick.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct CarInput {
    /// -1 (full brake / reverse) .. +1 (full throttle).
    pub throttle: f32,
    /// -1 (right) .. +1 (left).
    pub steer: f32,
    /// Handbrake, held: drops lateral grip and raises yaw authority.
    pub handbrake: bool,
    /// A boost PRESS, not the held key. The repo already paid for this bug
    /// once with jump: a held flag re-triggers on every tick the server
    /// receives, which here would drain all charges in three frames. The
    /// client latches the rising edge; the sim consumes it in one tick.
    pub boost: bool,
}

impl CarInput {
    /// Untrusted input arrives over the wire. Strip NaN/inf and clamp, or a
    /// single malformed packet teleports a car and poisons every later tick.
    #[must_use]
    pub const fn sanitized(mut self) -> Self {
        self.throttle = if self.throttle.is_finite() {
            self.throttle.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.steer = if self.steer.is_finite() {
            self.steer.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self
    }
}

/// Not serialisable on purpose: the wire carries a flat scalar snapshot
/// (see `proto`), so the internal sim state stays free to change shape
/// without that being a protocol question.
#[derive(Clone, Copy, Debug)]
pub struct Car {
    /// Position on the XZ plane, metres.
    pub pos: Vec2,
    /// World-space velocity, m/s. Not necessarily parallel to the heading —
    /// that is the point.
    pub vel: Vec2,
    /// Heading, radians. 0 faces +Z, matching `Quat::from_rotation_y`, so the
    /// renderer can pass this straight to `Instance::with_yaw`.
    pub yaw: f32,
    /// Smoothed 0..1 drift intensity, for the camera and the tyre smoke.
    pub drift: f32,
    pub boost_charges: u8,
    /// Seconds of boost remaining; > 0 means boosting.
    pub boost_left: f32,
}

/// Unit forward vector for a heading. Matches `Quat::from_rotation_y(yaw)`
/// applied to +Z, so sim and renderer cannot disagree about which way a car
/// faces.
#[must_use]
pub fn forward(yaw: f32) -> Vec2 {
    Vec2::new(yaw.sin(), yaw.cos())
}

/// Unit right vector for a heading.
#[must_use]
pub fn right(yaw: f32) -> Vec2 {
    Vec2::new(yaw.cos(), -yaw.sin())
}

impl Car {
    #[must_use]
    pub const fn new(pos: Vec2, yaw: f32) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            yaw,
            drift: 0.0,
            boost_charges: BOOST_CHARGES,
            boost_left: 0.0,
        }
    }

    #[must_use]
    pub fn speed(&self) -> f32 {
        self.vel.length()
    }

    #[must_use]
    pub const fn boosting(&self) -> bool {
        self.boost_left > 0.0
    }

    /// Signed angle between where the car points and where it is going.
    /// Near zero for grip driving; large during a slide. Returns 0 below
    /// `SLIP_MIN_SPEED`, where the direction of a near-stationary velocity is
    /// numerical noise.
    #[must_use]
    pub fn slip_angle(&self) -> f32 {
        let speed = self.vel.length();
        if speed < SLIP_MIN_SPEED {
            return 0.0;
        }
        let f = forward(self.yaw);
        let v = self.vel / speed;
        // atan2 of (cross, dot) gives the signed angle from f to v.
        (f.x * v.y - f.y * v.x).atan2(f.dot(v))
    }

    /// Advance one tick. `surface_grip` is 1.0 on the racing surface and
    /// `OFFROAD_FACTOR` off it; the caller owns that decision because only it
    /// knows the track.
    pub fn step(&mut self, input: &CarInput, surface_grip: f32, dt: f32) {
        let input = input.sanitized();

        // 1. Boost: consume one charge on a press, then run the timer down.
        if input.boost && !self.boosting() && self.boost_charges > 0 {
            self.boost_charges -= 1;
            self.boost_left = BOOST_SECS;
        }
        if self.boost_left > 0.0 {
            self.boost_left = (self.boost_left - dt).max(0.0);
        }
        let boosting = self.boost_left > 0.0;

        // 2. Steer FIRST, and leave the world-space velocity alone while doing
        //    it. This ordering is the whole mechanism. Rotating the car does
        //    not rotate its momentum; it only changes which direction counts
        //    as "forward". The velocity that was forward a moment ago is now
        //    partly lateral, and step 5 decides whether that lateral part is
        //    killed (grip: the car follows its nose) or kept (drift: it
        //    slides). Decomposing in the old frame and recomposing in the new
        //    one would rigidly carry the velocity around with the heading, and
        //    the slip angle could never become non-zero.
        let speed = self.vel.length();
        let heading_sign = if self.vel.dot(forward(self.yaw)) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let mut rate = STEER_MAX / (1.0 + speed * STEER_FALLOFF);
        if input.handbrake {
            rate *= STEER_DRIFT_BONUS;
        }
        // Fade steering out at a standstill instead of letting a parked car
        // pirouette on the spot, and reverse it in reverse as a real car does.
        let authority = (speed / 3.0).min(1.0) * heading_sign;
        // Subtract, so that +steer really is left. `forward = (sin y, cos y)`
        // means a rising yaw sweeps the nose clockwise, i.e. to the right; a
        // `+=` here would make the documented sign of `steer` a lie and send
        // anything that trusts it — the chase AI, the client's prediction —
        // the wrong way round every corner.
        self.yaw -= input.steer * rate * authority * dt;

        // 3. Decompose the (untouched) velocity into the car's NEW frame.
        let f = forward(self.yaw);
        let r = right(self.yaw);
        let mut v_fwd = self.vel.dot(f);
        let mut v_lat = self.vel.dot(r);

        // 3. Longitudinal. Throttle drives, brake opposes actual motion (so
        //    holding S at a standstill reverses rather than braking forever).
        let drive = ENGINE_ACCEL * if boosting { BOOST_ACCEL_MULT } else { 1.0 } * surface_grip;
        if input.throttle > 0.0 {
            v_fwd += drive * input.throttle * dt;
        } else if input.throttle < 0.0 {
            if v_fwd > 0.1 {
                v_fwd -= BRAKE_ACCEL * -input.throttle * dt;
            } else {
                v_fwd += ENGINE_ACCEL * input.throttle * dt * surface_grip;
            }
        }

        // 4. Resistances. Quadratic drag plus an exponential roll-off; the
        //    exponential form is dt-correct, unlike the `v *= 0.98` idiom
        //    which silently changes the car's feel with the frame rate.
        v_fwd -= DRAG * v_fwd * v_fwd.abs() * dt;
        v_fwd *= (-ROLL_RESIST * dt * (1.0 - input.throttle.abs().min(1.0))).exp();

        let top = MAX_SPEED * if boosting { BOOST_SPEED_MULT } else { 1.0 };
        v_fwd = v_fwd.clamp(-REVERSE_MAX, top);

        // 5. Lateral grip. Exponential decay again, and again for the same
        //    reason. Handbrake swaps in the low coefficient: that, and only
        //    that, is what makes the car slide.
        let grip = if input.handbrake { GRIP_DRIFT } else { GRIP } * surface_grip;
        v_lat *= (-grip * dt).exp();

        // 6. Recompose in the same frame we decomposed in, then guard the
        //    spin. Past MAX_SLIP the car is no longer drifting, it is
        //    spinning; pull the nose back toward the direction of travel
        //    rather than letting it wind up.
        self.vel = f * v_fwd + r * v_lat;

        let slip = self.slip_angle();
        if slip.abs() > MAX_SLIP {
            // Mind the sign. `slip` is the counter-clockwise angle from the
            // nose to the velocity, but increasing `yaw` rotates the nose
            // CLOCKWISE (forward = (sin y, cos y), so d/dy = right). To bring
            // the nose back toward the velocity we therefore SUBTRACT. Adding
            // here pushes the nose further from where the car is going, which
            // turns the guard against spinning into a spin generator.
            let excess = slip.abs() - MAX_SLIP;
            self.yaw -= slip.signum() * (excess * SLIP_RECOVER * dt).min(excess);
        }

        // 8. Integrate position, and track drift intensity for presentation.
        self.pos += self.vel * dt;

        let target = if input.handbrake && speed > SLIP_MIN_SPEED {
            (slip.abs() / MAX_SLIP).min(1.0)
        } else {
            0.0
        };
        // dt-correct smoothing toward the target.
        let k = (-6.0 * dt).exp();
        self.drift = target + (self.drift - target) * k;
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

    fn run(car: &mut Car, input: CarInput, ticks: u32) {
        for _ in 0..ticks {
            car.step(&input, 1.0, DT);
        }
    }

    const THROTTLE: CarInput = CarInput {
        throttle: 1.0,
        steer: 0.0,
        handbrake: false,
        boost: false,
    };

    #[test]
    fn accelerates_and_settles_at_top_speed() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        run(&mut car, THROTTLE, 60);
        let after_1s = car.speed();
        assert!(
            after_1s > 8.0,
            "1 s of full throttle only reached {after_1s} m/s"
        );
        run(&mut car, THROTTLE, 60 * 60);
        let top = car.speed();
        assert!(
            (top - MAX_SPEED).abs() < 1.5,
            "terminal speed {top} should balance drag at {MAX_SPEED}"
        );
    }

    #[test]
    fn never_exceeds_the_speed_cap() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        for _ in 0..60 * 120 {
            car.step(&THROTTLE, 1.0, DT);
            assert!(
                car.speed() <= MAX_SPEED * BOOST_SPEED_MULT + 0.5,
                "speed {} escaped",
                car.speed()
            );
        }
    }

    /// Grip driving must keep the nose pointed where the car is going.
    #[test]
    fn cornering_without_handbrake_barely_slips() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        run(&mut car, THROTTLE, 120);
        let turning = CarInput {
            throttle: 0.6,
            steer: 1.0,
            handbrake: false,
            boost: false,
        };
        let mut peak: f32 = 0.0;
        for _ in 0..120 {
            car.step(&turning, 1.0, DT);
            peak = peak.max(car.slip_angle().abs());
        }
        assert!(
            peak < 0.35,
            "grip cornering slipped {peak} rad — that is a slide, not a corner"
        );
    }

    /// The headline feature: the handbrake must actually break traction.
    #[test]
    fn handbrake_produces_a_real_slide() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        run(&mut car, THROTTLE, 150);
        let drifting = CarInput {
            throttle: 0.7,
            steer: 1.0,
            handbrake: true,
            boost: false,
        };
        let mut peak: f32 = 0.0;
        for _ in 0..90 {
            car.step(&drifting, 1.0, DT);
            peak = peak.max(car.slip_angle().abs());
        }
        assert!(
            peak > 0.4,
            "handbrake only reached {peak} rad of slip — no drift"
        );
        assert!(
            car.drift > 0.3,
            "drift intensity {} never registered",
            car.drift
        );
    }

    /// ...and it must be recoverable. This is the failure mode the research
    /// flagged: drop rear grip and the car pirouettes forever.
    #[test]
    fn a_drift_can_be_driven_out_of() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        run(&mut car, THROTTLE, 150);
        let drifting = CarInput {
            throttle: 0.7,
            steer: 1.0,
            handbrake: true,
            boost: false,
        };
        run(&mut car, drifting, 90);
        assert!(
            car.slip_angle().abs() > 0.3,
            "test precondition: should be sliding"
        );
        // Release the handbrake and straighten up.
        run(&mut car, THROTTLE, 120);
        assert!(
            car.slip_angle().abs() < 0.15,
            "still sliding at {} rad two seconds after release — unrecoverable",
            car.slip_angle()
        );
    }

    #[test]
    fn slip_is_capped_so_the_car_never_spins_freely() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        run(&mut car, THROTTLE, 150);
        let hard = CarInput {
            throttle: 1.0,
            steer: 1.0,
            handbrake: true,
            boost: false,
        };
        for _ in 0..60 * 20 {
            car.step(&hard, 1.0, DT);
            assert!(
                car.slip_angle().abs() <= MAX_SLIP + 0.2,
                "slip {} blew past the cap — the car is spinning",
                car.slip_angle()
            );
        }
    }

    /// A held boost key must not drain every charge. This is the jump bug the
    /// repo already paid for, in a new costume.
    #[test]
    fn held_boost_consumes_exactly_one_charge() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        let held = CarInput {
            throttle: 1.0,
            steer: 0.0,
            handbrake: false,
            boost: true,
        };
        run(&mut car, held, 30);
        assert_eq!(
            car.boost_charges,
            BOOST_CHARGES - 1,
            "a held key drained multiple charges"
        );
    }

    #[test]
    fn boost_is_limited_to_its_charges() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        let press = CarInput {
            throttle: 1.0,
            steer: 0.0,
            handbrake: false,
            boost: true,
        };
        for _ in 0..10 {
            car.step(&press, 1.0, DT);
            // Let the boost fully expire before pressing again.
            run(
                &mut car,
                THROTTLE,
                (BOOST_SECS * TICK_HZ as f32) as u32 + 10,
            );
        }
        assert_eq!(car.boost_charges, 0);
        assert!(!car.boosting(), "boost still active with no charges left");
    }

    #[test]
    fn boost_actually_makes_the_car_faster() {
        let mut plain = Car::new(Vec2::ZERO, 0.0);
        let mut boosted = Car::new(Vec2::ZERO, 0.0);
        run(&mut plain, THROTTLE, 90);
        run(&mut boosted, THROTTLE, 90);
        boosted.step(
            &CarInput {
                throttle: 1.0,
                steer: 0.0,
                handbrake: false,
                boost: true,
            },
            1.0,
            DT,
        );
        run(&mut plain, THROTTLE, 60);
        run(&mut boosted, THROTTLE, 60);
        assert!(
            boosted.speed() > plain.speed() + 2.0,
            "boost gained only {:.2} m/s",
            boosted.speed() - plain.speed()
        );
    }

    /// Friction written as a per-tick multiplier is timestep-dependent. The
    /// exponential form is not: halving dt and doubling the tick count must
    /// land in the same place.
    #[test]
    fn integration_is_timestep_correct() {
        let mut coarse = Car::new(Vec2::ZERO, 0.0);
        let mut fine = Car::new(Vec2::ZERO, 0.0);
        let turning = CarInput {
            throttle: 1.0,
            steer: 0.5,
            handbrake: false,
            boost: false,
        };
        for _ in 0..120 {
            coarse.step(&turning, 1.0, DT);
        }
        for _ in 0..480 {
            fine.step(&turning, 1.0, DT / 4.0);
        }
        let drift = (coarse.pos - fine.pos).length();
        assert!(
            drift < coarse.pos.length() * 0.05,
            "dt-dependence: {drift} m apart after 2 s ({} vs {})",
            coarse.pos,
            fine.pos
        );
    }

    /// The sim is shared with the authoritative server; identical inputs must
    /// produce bit-identical state or every client desyncs.
    #[test]
    fn stepping_is_deterministic() {
        let seq = [
            CarInput {
                throttle: 1.0,
                steer: 0.3,
                handbrake: false,
                boost: true,
            },
            CarInput {
                throttle: 0.2,
                steer: -1.0,
                handbrake: true,
                boost: false,
            },
            CarInput {
                throttle: -1.0,
                steer: 0.7,
                handbrake: false,
                boost: false,
            },
        ];
        let play = || {
            let mut c = Car::new(Vec2::new(3.0, -7.0), 0.9);
            for i in 0..600 {
                c.step(
                    &seq[i % seq.len()],
                    if i % 3 == 0 { OFFROAD_FACTOR } else { 1.0 },
                    DT,
                );
            }
            c
        };
        let a = play();
        let b = play();
        assert_eq!(a.pos.to_array(), b.pos.to_array());
        assert_eq!(a.vel.to_array(), b.vel.to_array());
        assert_eq!(a.yaw.to_bits(), b.yaw.to_bits());
    }

    /// A malformed packet must not be able to poison the simulation.
    #[test]
    fn hostile_input_cannot_break_the_car() {
        let mut car = Car::new(Vec2::ZERO, 0.0);
        let evil = CarInput {
            throttle: f32::NAN,
            steer: f32::INFINITY,
            handbrake: true,
            boost: true,
        };
        run(&mut car, evil, 300);
        assert!(car.pos.is_finite(), "position went non-finite: {}", car.pos);
        assert!(car.vel.is_finite(), "velocity went non-finite: {}", car.vel);
        assert!(car.yaw.is_finite(), "yaw went non-finite: {}", car.yaw);
    }

    #[test]
    fn offroad_costs_speed() {
        let mut road = Car::new(Vec2::ZERO, 0.0);
        let mut grass = Car::new(Vec2::ZERO, 0.0);
        for _ in 0..300 {
            road.step(&THROTTLE, 1.0, DT);
            grass.step(&THROTTLE, OFFROAD_FACTOR, DT);
        }
        assert!(
            grass.speed() < road.speed() * 0.85,
            "offroad was not slower"
        );
    }
}
