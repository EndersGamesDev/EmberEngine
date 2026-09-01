//! Pure deterministic Pong simulation using fixed 60 Hz steps.

/// Fixed simulation delta in seconds.
pub const FIXED_DT: f32 = 1.0 / 60.0;
/// Playable x range for the ball center.
pub const COURT_HALF_W: f32 = 9.0;
/// Absolute z coordinate beyond which the ball is out.
pub const COURT_END_Z: f32 = 14.5;
/// Absolute paddle z coordinate.
pub const PADDLE_Z: f32 = 13.0;
/// Paddle half-width on the x axis.
pub const PADDLE_HALF_W: f32 = 2.2;
/// Paddle half-thickness on the z axis.
pub const PADDLE_HALF_T: f32 = 0.4;
/// Paddle speed in world units per second.
pub const PADDLE_SPEED: f32 = 16.0;
/// Ball radius.
pub const BALL_R: f32 = 0.45;
/// Initial serve speed.
pub const SERVE_SPEED: f32 = 11.0;
/// Maximum ball speed.
pub const MAX_SPEED: f32 = 26.0;
/// Ball-speed multiplier on a paddle hit.
pub const SPEEDUP: f32 = 1.05;
/// Minimum fraction of ball speed retained on the z axis.
pub const MIN_Z_FRACTION: f32 = 0.55;
/// Score needed to win a match.
pub const WIN_SCORE: u32 = 7;
/// Pause before a normal serve, in seconds.
pub const SERVE_PAUSE: f32 = 1.2;

/// Current ball-play phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Ball parked at center before launching toward `dir`.
    Serving {
        /// Remaining pause in seconds.
        timer: f32,
        /// Serve direction, positive toward the near player.
        dir: f32,
    },
    /// Ball in active play.
    Playing,
}

/// Complete authoritative state for one Pong match.
pub struct Sim {
    /// Near paddle x coordinate.
    pub p1_x: f32,
    /// Far paddle x coordinate.
    pub p2_x: f32,
    /// Ball coordinates in `(x, z)` order.
    pub ball_pos: [f32; 2],
    /// Ball velocity in `(x, z)` order.
    pub ball_vel: [f32; 2],
    /// Near and far player scores.
    pub score: [u32; 2],
    /// Current serve or play phase.
    pub phase: Phase,
    /// Total serves, used to alternate serve angle without random input.
    pub serves: u32,
    /// One-step scoring event as `(scorer, won)`.
    pub event: Option<(usize, bool)>,
    pending_reset: bool,
}

impl Sim {
    /// Constructs the era's initial match state.
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

    /// Advances one frozen fixed step with near and far paddle intents.
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
                    if self.pending_reset {
                        self.pending_reset = false;
                        self.score = [0, 0];
                    }
                    let side = if self.serves.is_multiple_of(2) {
                        1.0
                    } else {
                        -1.0
                    };
                    let angle = 0.45_f32;
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

                let wall = COURT_HALF_W - BALL_R;
                if self.ball_pos[0] > wall {
                    self.ball_pos[0] = wall;
                    self.ball_vel[0] = -self.ball_vel[0].abs();
                } else if self.ball_pos[0] < -wall {
                    self.ball_pos[0] = -wall;
                    self.ball_vel[0] = self.ball_vel[0].abs();
                }

                if self.ball_vel[1] > 0.0 {
                    self.try_paddle_hit(self.p1_x, PADDLE_Z);
                } else {
                    self.try_paddle_hit(self.p2_x, -PADDLE_Z);
                }

                if self.ball_pos[1] > COURT_END_Z {
                    self.point_scored(1);
                } else if self.ball_pos[1] < -COURT_END_Z {
                    self.point_scored(0);
                }
            }
        }
    }

    fn try_paddle_hit(&mut self, paddle_x: f32, paddle_z: f32) {
        let z = self.ball_pos[1];
        let front = paddle_z - paddle_z.signum() * (PADDLE_HALF_T + BALL_R);
        let reach = (z - front) * paddle_z.signum();
        if !(0.0..=PADDLE_HALF_T + BALL_R).contains(&reach) {
            return;
        }
        let offset = self.ball_pos[0] - paddle_x;
        if offset.abs() > PADDLE_HALF_W + BALL_R {
            return;
        }

        let speed =
            (self.ball_vel[0] * self.ball_vel[0] + self.ball_vel[1] * self.ball_vel[1]).sqrt();
        let new_speed = (speed * SPEEDUP).min(MAX_SPEED);
        let mut vx = self.ball_vel[0] + (offset / PADDLE_HALF_W) * 7.0;
        let vz_sign = -paddle_z.signum();
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
#[allow(clippy::float_cmp)] // The era determinism checks require bit-identical state.
mod tests {
    use super::{
        COURT_END_Z, COURT_HALF_W, FIXED_DT, MIN_Z_FRACTION, PADDLE_HALF_T, PADDLE_Z, Phase,
        SERVE_PAUSE, SERVE_SPEED, Sim, WIN_SCORE,
    };

    fn run_serve(sim: &mut Sim) {
        let mut elapsed = 0.0;
        while elapsed <= SERVE_PAUSE * 2.0 + FIXED_DT * 2.0 {
            sim.step(0.0, 0.0);
            if sim.phase == Phase::Playing {
                return;
            }
            elapsed += FIXED_DT;
        }
        panic!("serve never launched");
    }

    #[test]
    fn serve_launches_toward_p1_first() {
        let mut sim = Sim::new();
        run_serve(&mut sim);
        assert!(sim.ball_vel[1] > 0.0, "first serve goes toward +z");
        let speed = (sim.ball_vel[0].powi(2) + sim.ball_vel[1].powi(2)).sqrt();
        assert!((speed - SERVE_SPEED).abs() < 1e-3);
    }

    #[test]
    fn wall_bounce_reflects_x() {
        let mut sim = Sim::new();
        run_serve(&mut sim);
        sim.ball_pos = [COURT_HALF_W - super::BALL_R - 0.01, 0.0];
        sim.ball_vel = [8.0, 4.0];
        sim.step(0.0, 0.0);
        assert!(sim.ball_vel[0] < 0.0);
    }

    #[test]
    fn missed_ball_scores_for_opponent_and_reserves() {
        let mut sim = Sim::new();
        run_serve(&mut sim);
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
        sim.ball_pos = [0.5, PADDLE_Z - PADDLE_HALF_T - super::BALL_R - 0.05];
        sim.ball_vel = [0.0, 12.0];
        sim.step(0.0, 0.0);
        assert!(sim.ball_vel[1] < 0.0);
        let speed = (sim.ball_vel[0].powi(2) + sim.ball_vel[1].powi(2)).sqrt();
        assert!(speed > 12.0);
        assert!(sim.ball_vel[1].abs() >= speed * MIN_Z_FRACTION - 1e-3);
    }

    #[test]
    fn win_resets_scores() {
        let mut sim = Sim::new();
        sim.score = [WIN_SCORE - 1, 3];
        run_serve(&mut sim);
        sim.ball_pos = [0.0, -(COURT_END_Z - 0.05)];
        sim.ball_vel = [0.0, -12.0];
        sim.p2_x = 6.0;
        sim.step(0.0, 0.0);
        assert_eq!(sim.event, Some((0, true)));
        assert_eq!(sim.score, [WIN_SCORE, 3]);
        run_serve(&mut sim);
        assert_eq!(sim.score, [0, 0]);
    }

    #[test]
    fn determinism_same_inputs_same_result() {
        let run = || {
            let mut sim = Sim::new();
            for index in 0..3600 {
                let axis = if index % 120 < 60 { 1.0 } else { -1.0 };
                sim.step(axis, -axis);
            }
            (sim.ball_pos, sim.ball_vel, sim.score, sim.p1_x, sim.p2_x)
        };
        assert_eq!(run(), run());
    }
}
