//! Pong simulation: pure, deterministic, fixed 60 Hz steps. No engine or
//! platform types in here — the sim is testable headless and is the shape a
//! future networked version would replicate.

pub const FIXED_DT: f32 = 1.0 / 60.0;

pub const COURT_HALF_W: f32 = 9.0; // playable x range for the ball center
pub const COURT_END_Z: f32 = 14.5; // beyond this the ball is out
pub const PADDLE_Z: f32 = 13.0;
pub const PADDLE_HALF_W: f32 = 2.2;
pub const PADDLE_HALF_T: f32 = 0.4; // half thickness in z
pub const PADDLE_SPEED: f32 = 16.0;
pub const BALL_R: f32 = 0.45;
pub const SERVE_SPEED: f32 = 11.0;
pub const MAX_SPEED: f32 = 26.0;
pub const SPEEDUP: f32 = 1.05;
/// The ball always keeps at least this fraction of its speed in z, so
/// English can't produce endless sideways rallies.
pub const MIN_Z_FRACTION: f32 = 0.55;
pub const WIN_SCORE: u32 = 7;
pub const SERVE_PAUSE: f32 = 1.2;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    /// Ball parked at center; `timer` counts down, then it launches toward
    /// `dir` (+1.0 = toward player 1 at +z, -1.0 = toward player 2 at -z).
    Serving {
        timer: f32,
        dir: f32,
    },
    Playing,
}

pub struct Sim {
    pub p1_x: f32,          // paddle at +z (near)
    pub p2_x: f32,          // paddle at -z (far)
    pub ball_pos: [f32; 2], // (x, z)
    pub ball_vel: [f32; 2],
    pub score: [u32; 2], // [p1, p2]
    pub phase: Phase,
    /// Total points played; used to alternate the serve angle
    /// deterministically instead of using an RNG.
    pub serves: u32,
    /// Set for one step when someone scores or wins: (scorer index, won).
    pub event: Option<(usize, bool)>,
    /// After a win the final score stays on the board through the (longer)
    /// serve pause; it resets when the next game's serve launches.
    pending_reset: bool,
}

impl Sim {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            p1_x: 0.0,
            p2_x: 0.0,
            ball_pos: [0.0, 0.0],
            ball_vel: [0.0, 0.0],
            score: [0, 0],
            phase: Phase::Serving {
                timer: SERVE_PAUSE,
                dir: 1.0,
            },
            serves: 0,
            event: None,
            pending_reset: false,
        }
    }

    /// One fixed step. `p1_axis`/`p2_axis` are -1..1 paddle intents.
    pub fn step(&mut self, p1_axis: f32, p2_axis: f32) {
        self.event = None;
        let clamp_x = COURT_HALF_W - PADDLE_HALF_W;
        self.p1_x = (self.p1_x + p1_axis.clamp(-1.0, 1.0) * PADDLE_SPEED * FIXED_DT)
            .clamp(-clamp_x, clamp_x);
        self.p2_x = (self.p2_x + p2_axis.clamp(-1.0, 1.0) * PADDLE_SPEED * FIXED_DT)
            .clamp(-clamp_x, clamp_x);

        match self.phase {
            Phase::Serving { timer, dir } => {
                let timer = timer - FIXED_DT;
                if timer > 0.0 {
                    self.phase = Phase::Serving { timer, dir };
                } else {
                    // A finished game's score stays visible through the
                    // pause; a new game starts with the serve.
                    if self.pending_reset {
                        self.pending_reset = false;
                        self.score = [0, 0];
                    }
                    // Alternate the serve angle left/right deterministically.
                    let side = if self.serves.is_multiple_of(2) {
                        1.0
                    } else {
                        -1.0
                    };
                    let angle = 0.45_f32; // ~26 degrees
                    self.serves += 1;
                    self.ball_vel = [
                        angle.sin() * side * SERVE_SPEED,
                        angle.cos() * dir * SERVE_SPEED,
                    ];
                    self.phase = Phase::Playing;
                }
            }
            Phase::Playing => {
                self.ball_pos[0] += self.ball_vel[0] * FIXED_DT;
                self.ball_pos[1] += self.ball_vel[1] * FIXED_DT;

                // Side walls.
                let wall = COURT_HALF_W - BALL_R;
                if self.ball_pos[0] > wall {
                    self.ball_pos[0] = wall;
                    self.ball_vel[0] = -self.ball_vel[0].abs();
                } else if self.ball_pos[0] < -wall {
                    self.ball_pos[0] = -wall;
                    self.ball_vel[0] = self.ball_vel[0].abs();
                }

                // Paddles.
                if self.ball_vel[1] > 0.0 {
                    self.try_paddle_hit(self.p1_x, PADDLE_Z);
                } else {
                    self.try_paddle_hit(self.p2_x, -PADDLE_Z);
                }

                // Out of court -> point.
                if self.ball_pos[1] > COURT_END_Z {
                    self.point_scored(1); // past P1 -> P2 scores
                } else if self.ball_pos[1] < -COURT_END_Z {
                    self.point_scored(0);
                }
            }
        }
    }

    fn try_paddle_hit(&mut self, paddle_x: f32, paddle_z: f32) {
        let z = self.ball_pos[1];
        let front = paddle_z - paddle_z.signum() * (PADDLE_HALF_T + BALL_R);
        // Between the paddle's front face and slightly past its center.
        let reach = (z - front) * paddle_z.signum();
        if !(0.0..=PADDLE_HALF_T + BALL_R).contains(&reach) {
            return;
        }
        let offset = self.ball_pos[0] - paddle_x;
        if offset.abs() > PADDLE_HALF_W + BALL_R {
            return;
        }

        // Reflect off the front face, add English from the hit offset,
        // speed up, and re-normalize.
        self.ball_pos[1] = front;
        let speed =
            (self.ball_vel[0] * self.ball_vel[0] + self.ball_vel[1] * self.ball_vel[1]).sqrt();
        let new_speed = (speed * SPEEDUP).min(MAX_SPEED);
        let mut vx = self.ball_vel[0] + (offset / PADDLE_HALF_W) * 7.0;
        let vz_sign = -paddle_z.signum(); // away from this paddle
        // Enforce a minimum z fraction so the ball always makes progress.
        let max_vx = new_speed * (1.0 - MIN_Z_FRACTION * MIN_Z_FRACTION).sqrt();
        vx = vx.clamp(-max_vx, max_vx);
        let vz = (new_speed * new_speed - vx * vx).sqrt() * vz_sign;
        self.ball_vel = [vx, vz];
    }

    fn point_scored(&mut self, scorer: usize) {
        self.score[scorer] += 1;
        let won = self.score[scorer] >= WIN_SCORE;
        self.event = Some((scorer, won));
        if won {
            self.pending_reset = true;
        }
        self.ball_pos = [0.0, 0.0];
        self.ball_vel = [0.0, 0.0];
        // Serve toward the player who conceded.
        let dir = if scorer == 1 { 1.0 } else { -1.0 };
        let pause = if won { SERVE_PAUSE * 2.0 } else { SERVE_PAUSE };
        self.phase = Phase::Serving { timer: pause, dir };
    }
}

impl Default for Sim {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    // This fixed positive duration is converted only to bound a test loop.
    #![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

    use super::*;

    fn run_serve(sim: &mut Sim) {
        // Step through the serve pause (double-length after a win) until
        // the ball launches.
        for _ in 0..((SERVE_PAUSE * 2.0 / FIXED_DT) as u32 + 2) {
            sim.step(0.0, 0.0);
            if sim.phase == Phase::Playing {
                return;
            }
        }
        panic!("serve never launched");
    }

    #[test]
    fn serve_launches_toward_p1_first() {
        let mut sim = Sim::new();
        run_serve(&mut sim);
        assert!(sim.ball_vel[1] > 0.0, "first serve goes toward +z (P1)");
        let speed = (sim.ball_vel[0].powi(2) + sim.ball_vel[1].powi(2)).sqrt();
        assert!((speed - SERVE_SPEED).abs() < 1e-3);
    }

    #[test]
    fn wall_bounce_reflects_x() {
        let mut sim = Sim::new();
        run_serve(&mut sim);
        sim.ball_pos = [COURT_HALF_W - BALL_R - 0.01, 0.0];
        sim.ball_vel = [8.0, 4.0];
        sim.step(0.0, 0.0);
        assert!(sim.ball_vel[0] < 0.0, "vx must flip at the right wall");
    }

    #[test]
    fn missed_ball_scores_for_opponent_and_reserves() {
        let mut sim = Sim::new();
        run_serve(&mut sim);
        // Park paddles far left, fire the ball past P1 on the right.
        sim.p1_x = -6.0;
        sim.ball_pos = [6.0, COURT_END_Z - 0.05];
        sim.ball_vel = [0.0, 12.0];
        sim.step(-1.0, 0.0);
        assert_eq!(sim.score, [0, 1]);
        assert_eq!(sim.event, Some((1, false)));
        assert!(matches!(sim.phase, Phase::Serving { dir, .. } if dir > 0.0));
        assert_eq!(sim.ball_pos, [0.0, 0.0]);
    }

    #[test]
    fn paddle_returns_the_ball() {
        let mut sim = Sim::new();
        run_serve(&mut sim);
        sim.p1_x = 0.0;
        sim.ball_pos = [0.5, PADDLE_Z - PADDLE_HALF_T - BALL_R - 0.05];
        sim.ball_vel = [0.0, 12.0];
        let speed_before = 12.0_f32;
        sim.step(0.0, 0.0);
        assert!(sim.ball_vel[1] < 0.0, "ball must reflect back toward P2");
        let speed = (sim.ball_vel[0].powi(2) + sim.ball_vel[1].powi(2)).sqrt();
        assert!(speed > speed_before, "ball speeds up on paddle hits");
        assert!(
            sim.ball_vel[1].abs() >= speed * MIN_Z_FRACTION - 1e-3,
            "z fraction floor holds"
        );
    }

    #[test]
    fn win_resets_scores() {
        let mut sim = Sim::new();
        sim.score = [WIN_SCORE - 1, 3];
        run_serve(&mut sim);
        sim.ball_pos = [0.0, -(COURT_END_Z - 0.05)];
        sim.ball_vel = [0.0, -12.0];
        sim.p2_x = 6.0; // out of the way
        sim.step(0.0, 0.0);
        assert_eq!(sim.event, Some((0, true)));
        assert_eq!(
            sim.score,
            [WIN_SCORE, 3],
            "final score stays on the board through the win pause"
        );
        run_serve(&mut sim);
        assert_eq!(sim.score, [0, 0], "scores reset when the next game serves");
    }

    #[test]
    fn determinism_same_inputs_same_result() {
        let run = || {
            let mut sim = Sim::new();
            for i in 0..3600 {
                let a = if i % 120 < 60 { 1.0 } else { -1.0 };
                sim.step(a, -a);
            }
            (sim.ball_pos, sim.ball_vel, sim.score, sim.p1_x, sim.p2_x)
        };
        assert_eq!(run(), run());
    }
}
