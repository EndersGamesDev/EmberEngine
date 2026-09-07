//! Deterministic player motion shared by authority and prediction.
//!
//! Ordinary motion keeps the established movement/vertical helpers. Parkour
//! carries explicit momentum and contact state, never wall-clock time or RNG.

use crate::shooter::{
    AIR_MOVE_SPEED, BODY_H_STAND, FIXED_DT, GRAVITY, JUMP_VEL, Obstacle, PLAYER_R, SPRINT_MULT,
    STEP_UP, move_circle_in, movement_speed, step_vertical, support_height, valid_arena_half,
};

pub const MAX_HORIZONTAL_SPEED: f32 = AIR_MOVE_SPEED * SPRINT_MULT;
pub const SLIDE_SPEED: f32 = 10.0;
pub const SLIDE_SECONDS: f32 = 0.65;
pub const SLIDE_FRICTION: f32 = 7.0;
pub const SLIDE_COOLDOWN: f32 = 1.0;
pub const WALL_JUMP_OUTWARD: f32 = 6.8;
pub const WALL_JUMP_COOLDOWN: f32 = 0.18;
pub const WALL_SLIDE_SPEED: f32 = 2.5;
pub const AIR_STEER: f32 = 6.0;
/// A wall grip needs a face wider than the body; poles are not climbing walls.
pub const MIN_WALL_SPAN: f32 = 2.0 * PLAYER_R + 0.1;
const WALL_SKIN: f32 = 0.06;

/// Complete replay state; weapon changes and reloads must not reset it.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParkourState {
    pub velocity: [f32; 2],
    pub slide_remaining: f32,
    pub slide_cooldown: f32,
    pub wall_cooldown: f32,
    /// Outward normal of a currently contacted physical obstacle side.
    pub wall_normal: [f32; 2],
    /// Last jumped face; the same normal cannot launch again until landing
    /// or a successful jump from a different wall.
    pub last_wall_normal: [f32; 2],
    /// Airborne velocity came from a slide or wall jump and must be retained.
    pub momentum: bool,
    /// Previous crouch intent, so holding crouch cannot restart ground slides.
    pub crouch_held: bool,
}

impl ParkourState {
    pub const READY: Self = Self {
        velocity: [0.0; 2],
        slide_remaining: 0.0,
        slide_cooldown: 0.0,
        wall_cooldown: 0.0,
        wall_normal: [0.0; 2],
        last_wall_normal: [0.0; 2],
        momentum: false,
        crouch_held: false,
    };
}

/// One command's movement intent. `jump` is a consumed press, not a held key.
// Independent controls mirror the existing wire protocol.
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent controls mirror the stable wire protocol"
)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MovementInput {
    pub mv: [f32; 2],
    pub jump: bool,
    pub sprint: bool,
    pub crouch: bool,
    /// Actual active shield, never the raw held intent during cooldown.
    pub shield: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementStep {
    pub pos: [f32; 2],
    pub y: f32,
    pub vy: f32,
    pub state: ParkourState,
    pub grounded: bool,
    /// Upward head contact only, including the first frame of a wall jump.
    pub bonked: Option<usize>,
    /// Slides force the lowered stance until they end, even if C is released.
    pub crouch: bool,
}

fn length(vector: [f32; 2]) -> f32 {
    (vector[0] * vector[0] + vector[1] * vector[1]).sqrt()
}

fn limited(vector: [f32; 2], limit: f32) -> [f32; 2] {
    if !vector[0].is_finite() || !vector[1].is_finite() {
        return [0.0; 2];
    }
    let magnitude = length(vector);
    if !magnitude.is_finite() {
        return [0.0; 2];
    }
    if magnitude > limit {
        [vector[0] / magnitude * limit, vector[1] / magnitude * limit]
    } else {
        vector
    }
}

const fn timer(value: f32, limit: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, limit)
    } else {
        0.0
    }
}

fn countdown(value: f32, dt: f32) -> f32 {
    if value <= dt + 0.00001 {
        0.0
    } else {
        value - dt
    }
}

fn clean_state(mut state: ParkourState) -> ParkourState {
    state.velocity = limited(state.velocity, MAX_HORIZONTAL_SPEED);
    state.slide_remaining = timer(state.slide_remaining, SLIDE_SECONDS);
    state.slide_cooldown = timer(state.slide_cooldown, SLIDE_COOLDOWN);
    state.wall_cooldown = timer(state.wall_cooldown, WALL_JUMP_COOLDOWN);
    state.wall_normal = limited(state.wall_normal, 1.0);
    state.last_wall_normal = limited(state.last_wall_normal, 1.0);
    state
}

fn grounded(pos: [f32; 2], y: f32, vy: f32, obstacles: &[Obstacle]) -> bool {
    vy == 0.0 && y <= support_height(pos, PLAYER_R, y, obstacles) + 1e-3
}

/// Physical box sides only: neither roofs overhead nor invisible level bounds
/// grant wall jumps. Uses the same base/top/step allowance as body blocking.
fn wall_contact(pos: [f32; 2], y: f32, obstacles: &[Obstacle]) -> [f32; 2] {
    let mut best = (f32::INFINITY, [0.0; 2]);
    for obstacle in obstacles {
        if y >= obstacle.h - STEP_UP || y + BODY_H_STAND <= obstacle.base {
            continue;
        }
        let closest = [
            pos[0].clamp(obstacle.min[0], obstacle.max[0]),
            pos[1].clamp(obstacle.min[1], obstacle.max[1]),
        ];
        let delta = [pos[0] - closest[0], pos[1] - closest[1]];
        let distance = length(delta);
        if distance <= 1e-5 || distance > PLAYER_R + WALL_SKIN || distance >= best.0 {
            continue;
        }
        // Axis-separated motion has axis-aligned faces, including at a box
        // corner. Stable ties preserve obstacle order and prefer the X face.
        let normal = if delta[0].abs() >= delta[1].abs() {
            [delta[0].signum(), 0.0]
        } else {
            [0.0, delta[1].signum()]
        };
        let tangent_span = if normal[0] == 0.0 {
            obstacle.max[0] - obstacle.min[0]
        } else {
            obstacle.max[1] - obstacle.min[1]
        };
        if tangent_span < MIN_WALL_SPAN {
            continue;
        }
        best = (distance, normal);
    }
    best.1
}

/// Shared motion step, bounded to 250 ms and split into at most 60 Hz slices.
///
/// A capped slice travels at most 0.24 m, less than the body radius, so even a
/// thin wall cannot be crossed between endpoint collision checks. Jump pulses
/// are consumed only on the first slice. Invalid/nonpositive time moves nothing.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "The public movement boundary keeps its position, velocity, state, input, level, and bounds explicit"
)]
pub fn step_movement(
    pos: [f32; 2],
    y: f32,
    vy: f32,
    state: ParkourState,
    input: MovementInput,
    dt: f32,
    obstacles: &[Obstacle],
    arena_half: f32,
) -> MovementStep {
    let arena_half = valid_arena_half(arena_half);
    let limit = arena_half - PLAYER_R;
    let pos = pos.map(|value| {
        if value.is_finite() {
            value.clamp(-limit, limit)
        } else {
            0.0
        }
    });
    let y = if y.is_finite() {
        y.clamp(0.0, 500.0)
    } else {
        0.0
    };
    let vy = if vy.is_finite() {
        vy.clamp(-100.0, 100.0)
    } else {
        0.0
    };
    let state = clean_state(state);
    let mut result = MovementStep {
        pos,
        y,
        vy,
        state,
        grounded: grounded(pos, y, vy, obstacles),
        bonked: None,
        crouch: input.crouch || state.slide_remaining > 0.0,
    };
    if !dt.is_finite() || dt <= 0.0 {
        return result;
    }
    let mut remaining = dt.min(0.25);
    let mut input = input;
    // Fifteen full 60 Hz slices cover .25 s. One additional slice consumes
    // any tiny f32 remainder without an open-ended float loop.
    for _ in 0..16 {
        if remaining <= 0.0 {
            break;
        }
        let slice = remaining.min(FIXED_DT);
        let bonked = result.bonked;
        result = motion_slice(result, input, slice, obstacles, arena_half);
        result.bonked = bonked.or(result.bonked);
        remaining = (remaining - slice).max(0.0);
        input.jump = false;
    }
    result
}

// One linear movement pass keeps transition ordering explicit.
// Contact normals are exact signed basis vectors; collision rejection is an
// exact unchanged endpoint, not an approximate proximity comparison.
#[allow(
    clippy::too_many_lines,
    clippy::float_cmp,
    reason = "One ordered movement pass keeps transitions explicit; comparisons use exact sentinel and endpoint values"
)]
fn motion_slice(
    previous: MovementStep,
    input: MovementInput,
    dt: f32,
    obstacles: &[Obstacle],
    arena_half: f32,
) -> MovementStep {
    let MovementStep {
        pos,
        y,
        mut vy,
        mut state,
        ..
    } = previous;
    let on_ground = grounded(pos, y, vy, obstacles);
    let intent = limited(input.mv, 1.0);
    let fresh_crouch = input.crouch && !state.crouch_held;
    state.crouch_held = input.crouch;
    state.slide_cooldown = countdown(state.slide_cooldown, dt);
    state.wall_cooldown = countdown(state.wall_cooldown, dt);
    if on_ground {
        state.momentum = false;
        state.last_wall_normal = [0.0; 2];
        state.wall_normal = [0.0; 2];
    }
    if state.slide_remaining > 0.0 && (!on_ground || input.shield) {
        if !on_ground {
            state.momentum = true;
        }
        state.slide_remaining = 0.0;
        state.slide_cooldown = SLIDE_COOLDOWN;
    }
    if on_ground
        && fresh_crouch
        && input.sprint
        && !input.shield
        && state.slide_remaining == 0.0
        && state.slide_cooldown == 0.0
        && length(intent) > 0.2
    {
        let direction = if length(state.velocity) > 0.2 {
            state.velocity
        } else {
            intent
        };
        let speed = length(direction);
        state.velocity = [
            direction[0] / speed * SLIDE_SPEED,
            direction[1] / speed * SLIDE_SPEED,
        ];
        state.slide_remaining = SLIDE_SECONDS;
    }
    let sliding = state.slide_remaining > 0.0;
    let mut momentum_motion = sliding || state.momentum;
    let ground_jump = on_ground && input.jump;
    if sliding {
        let speed = (length(state.velocity) - SLIDE_FRICTION * dt).max(0.0);
        state.velocity = limited(state.velocity, speed);
        state.slide_remaining = countdown(state.slide_remaining, dt);
        if ground_jump || state.slide_remaining == 0.0 {
            state.slide_remaining = 0.0;
            state.slide_cooldown = SLIDE_COOLDOWN;
            if ground_jump {
                state.momentum = true;
            }
        }
    } else if state.momentum && !on_ground {
        state.velocity = limited(
            [
                state.velocity[0] + intent[0] * AIR_STEER * dt,
                state.velocity[1] + intent[1] * AIR_STEER * dt,
            ],
            MAX_HORIZONTAL_SPEED,
        );
    }
    if input.shield && state.momentum {
        state.velocity = limited(state.velocity, AIR_MOVE_SPEED);
    }
    let speed = movement_speed(
        pos,
        y,
        vy,
        ground_jump,
        input.sprint,
        input.crouch,
        input.shield,
        obstacles,
    );
    if !momentum_motion {
        state.velocity = [intent[0] * speed, intent[1] * speed];
    }
    let desired = [
        pos[0] + state.velocity[0] * dt,
        pos[1] + state.velocity[1] * dt,
    ];
    let next = if momentum_motion {
        move_circle_in(
            pos,
            y,
            [
                state.velocity[0] / MAX_HORIZONTAL_SPEED,
                state.velocity[1] / MAX_HORIZONTAL_SPEED,
            ],
            MAX_HORIZONTAL_SPEED,
            dt,
            obstacles,
            arena_half,
        )
    } else {
        // Preserve the exact multiplication order of ordinary legacy movement.
        move_circle_in(pos, y, input.mv, speed, dt, obstacles, arena_half)
    };
    state.velocity = [(next[0] - pos[0]) / dt, (next[1] - pos[1]) / dt];
    let mut normal = wall_contact(next, y, obstacles);
    if normal == [0.0; 2] && next != desired {
        // The legacy slide helper stops before entering a wall. Its rejected
        // attempt is still a physical contact, not a requirement to overlap it.
        normal = wall_contact(desired, y, obstacles);
    }
    state.wall_normal = if on_ground { [0.0; 2] } else { normal };
    let same_wall =
        normal[0] * state.last_wall_normal[0] + normal[1] * state.last_wall_normal[1] > 0.5;
    let wall_jump = !on_ground
        && input.jump
        && input.crouch
        && !input.shield
        && normal != [0.0; 2]
        && !same_wall
        && state.wall_cooldown == 0.0;
    if wall_jump {
        let into = state.velocity[0] * normal[0] + state.velocity[1] * normal[1];
        state.velocity = limited(
            [
                state.velocity[0] - normal[0] * into + normal[0] * WALL_JUMP_OUTWARD,
                state.velocity[1] - normal[1] * into + normal[1] * WALL_JUMP_OUTWARD,
            ],
            MAX_HORIZONTAL_SPEED,
        );
        state.last_wall_normal = normal;
        state.wall_cooldown = WALL_JUMP_COOLDOWN;
        state.momentum = true;
        momentum_motion = true;
        vy = JUMP_VEL;
    } else if !on_ground && input.crouch && !input.shield && normal != [0.0; 2] && vy < 0.0 {
        // step_vertical applies gravity before integration, so compensate the
        // cap's input rather than correcting position through a floor afterward.
        vy = vy.max(-WALL_SLIDE_SPEED - GRAVITY * dt);
    }
    let rising = vy > 0.0 || ground_jump;
    let vertical = step_vertical(next, y, vy, ground_jump, dt, obstacles);
    if vertical.grounded {
        state.momentum = false;
        state.last_wall_normal = [0.0; 2];
        state.wall_normal = [0.0; 2];
    } else if momentum_motion && sliding {
        state.momentum = true;
    }
    state.velocity = limited(state.velocity, MAX_HORIZONTAL_SPEED);
    MovementStep {
        pos: next,
        y: vertical.y,
        vy: vertical.vy,
        state,
        grounded: vertical.grounded,
        bonked: if rising { vertical.bonked } else { None },
        crouch: input.crouch || state.slide_remaining > 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shooter::{ARENA_HALF, Cover, GameMode, Level, MOVE_SPEED, PlayerIn, Sim};

    fn stone(min: [f32; 2], max: [f32; 2], base: f32, top: f32) -> Obstacle {
        Obstacle {
            min,
            max,
            base,
            h: top,
            kind: Cover::Wall,
        }
    }

    fn start(pos: [f32; 2], y: f32, vy: f32) -> MovementStep {
        step_movement(
            pos,
            y,
            vy,
            ParkourState::READY,
            MovementInput::default(),
            0.0,
            &[],
            ARENA_HALF,
        )
    }

    fn advance(
        previous: MovementStep,
        input: MovementInput,
        obstacles: &[Obstacle],
    ) -> MovementStep {
        step_movement(
            previous.pos,
            previous.y,
            previous.vy,
            previous.state,
            input,
            FIXED_DT,
            obstacles,
            ARENA_HALF,
        )
    }

    fn near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0002,
            "actual {actual}, expected {expected}"
        );
    }

    #[test]
    fn ordinary_walk_and_jump_keep_the_existing_trajectory() {
        for sprint in [false, true] {
            let mut predicted = start([-12.0, 0.0], 0.0, 0.0);
            let (mut pos, mut y, mut vy) = (predicted.pos, 0.0, 0.0);
            for tick in 0..120 {
                let input = MovementInput {
                    mv: [0.7, 0.3],
                    sprint,
                    jump: tick == 12,
                    ..Default::default()
                };
                let speed = movement_speed(pos, y, vy, input.jump, sprint, false, false, &[]);
                pos = move_circle_in(pos, y, input.mv, speed, FIXED_DT, &[], ARENA_HALF);
                let vertical = step_vertical(pos, y, vy, input.jump, FIXED_DT, &[]);
                (y, vy) = (vertical.y, vertical.vy);
                predicted = advance(predicted, input, &[]);
                assert_eq!(
                    (predicted.pos, predicted.y, predicted.vy),
                    (pos, y, vy),
                    "tick {tick}"
                );
                assert!(!predicted.state.momentum);
            }
        }
    }

    #[test]
    fn slide_is_bounded_requires_fresh_crouch_and_cannot_restart_while_held() {
        let input = MovementInput {
            mv: [1.0, 0.0],
            sprint: true,
            crouch: true,
            ..Default::default()
        };
        let mut step = advance(start([-20.0, 0.0], 0.0, 0.0), input, &[]);
        assert!(step.state.slide_remaining > 0.0);
        assert!(step.state.velocity[0] > MOVE_SPEED * SPRINT_MULT);
        assert!(step.crouch);
        for _ in 1..39 {
            step = advance(step, input, &[]);
        }
        assert_eq!(
            step.state.slide_remaining, 0.0,
            "exactly 39 fixed ticks is .65 s"
        );
        near(step.state.slide_cooldown, SLIDE_COOLDOWN);
        for _ in 0..120 {
            step = advance(step, input, &[]);
        }
        assert_eq!(step.state.slide_remaining, 0.0);
        assert_eq!(step.state.slide_cooldown, 0.0);
        near(
            step.state.velocity[0],
            MOVE_SPEED * crate::shooter::CROUCH_MULT,
        );
        step = advance(
            step,
            MovementInput {
                crouch: false,
                ..input
            },
            &[],
        );
        step = advance(step, input, &[]);
        assert!(
            step.state.slide_remaining > 0.0,
            "a new press rearms after cooldown"
        );
        step = advance(
            step,
            MovementInput {
                crouch: false,
                ..input
            },
            &[],
        );
        assert!(step.crouch, "a released C does not stand up mid-slide");
    }

    #[test]
    fn slide_jump_keeps_horizontal_momentum_without_held_movement() {
        let slide = MovementInput {
            mv: [1.0, 0.0],
            sprint: true,
            crouch: true,
            ..Default::default()
        };
        let step = advance(start([-10.0, 0.0], 0.0, 0.0), slide, &[]);
        let mut step = advance(
            step,
            MovementInput {
                jump: true,
                ..slide
            },
            &[],
        );
        assert!(step.y > 0.0 && step.vy > 0.0 && step.state.momentum);
        let speed = step.state.velocity[0];
        assert!(speed > 9.0);
        for _ in 0..15 {
            step = advance(step, MovementInput::default(), &[]);
            near(step.state.velocity[0], speed);
        }
        assert!(step.pos[0] > -7.5);
    }

    #[test]
    fn crouch_slows_a_physical_wall_descent_but_not_open_air_or_shield() {
        let wall = [stone([0.0, -5.0], [0.1, 5.0], 0.0, 8.0)];
        let input = MovementInput {
            crouch: true,
            ..Default::default()
        };
        let initial = start([-0.61, 0.0], 4.0, -8.0);
        let slide = advance(initial, input, &wall);
        near(slide.vy, -WALL_SLIDE_SPEED);
        assert_eq!(slide.state.wall_normal, [-1.0, 0.0]);
        let open = advance(start([-3.0, 0.0], 4.0, -8.0), input, &wall);
        assert!(open.vy < -8.0);
        let shielded = advance(
            initial,
            MovementInput {
                shield: true,
                ..input
            },
            &wall,
        );
        assert!(shielded.vy < -8.0);
    }

    #[test]
    fn wall_jump_preserves_tangent_and_locks_same_wall_until_landing() {
        let wall = [stone([0.0, -8.0], [0.1, 8.0], 0.0, 12.0)];
        let input = MovementInput {
            jump: true,
            crouch: true,
            mv: [0.0, 1.0],
            ..Default::default()
        };
        let jumped = advance(start([-0.61, 0.0], 2.0, -1.0), input, &wall);
        near(jumped.state.velocity[0], -WALL_JUMP_OUTWARD);
        near(
            jumped.state.velocity[1],
            AIR_MOVE_SPEED * crate::shooter::CROUCH_MULT,
        );
        near(jumped.vy, JUMP_VEL + GRAVITY * FIXED_DT);
        assert!(jumped.state.momentum);
        // Even a hostile stream of distinct press pulses cannot farm one face.
        let mut locked = jumped;
        for _ in 0..30 {
            locked.pos = [-0.61, 0.0];
            locked.y = 2.0;
            locked.vy = -1.0;
            locked.state.velocity = [0.0; 2];
            locked = advance(locked, input, &wall);
            assert!(locked.vy < 0.0);
        }
        locked.pos = [-2.0, 0.0];
        locked.y = 0.0;
        locked.vy = 0.0;
        locked = advance(locked, MovementInput::default(), &wall);
        assert_eq!(locked.state.last_wall_normal, [0.0; 2]);
    }

    #[test]
    // Contacts are exact signed cardinal basis vectors, not measured distances.
    #[allow(clippy::float_cmp)]
    fn actual_harbor_opposing_walls_allow_chained_ascent() {
        let level = Level::harbor();
        for center in crate::harbor::WALL_JUMP_LANES {
            let mut step = step_movement(
                center,
                0.0,
                0.0,
                ParkourState::READY,
                MovementInput::default(),
                0.0,
                &level.obstacles,
                level.arena_half,
            );
            let mut sim = Sim::from_level(&level, 29, GameMode::Ffa);
            sim.add_player(0);
            sim.players[0].pos = center;
            let mut kicks = Vec::new();
            let mut high = 0.0_f32;
            for tick in 0..120 {
                let normal = step.state.wall_normal;
                let last = step.state.last_wall_normal;
                let contact_press =
                    normal != [0.0; 2] && normal != last && step.state.wall_cooldown == 0.0;
                let direction = if last[0] == 0.0 { -1.0 } else { last[0] };
                let input = MovementInput {
                    mv: [direction, 0.0],
                    jump: tick == 0 || contact_press,
                    crouch: true,
                    ..Default::default()
                };
                let next = step_movement(
                    step.pos,
                    step.y,
                    step.vy,
                    step.state,
                    input,
                    FIXED_DT,
                    &level.obstacles,
                    level.arena_half,
                );
                sim.step(&|_| PlayerIn {
                    mv: input.mv,
                    jump: input.jump,
                    crouch: input.crouch,
                    ..Default::default()
                });
                let player = &sim.players[0];
                assert_eq!(
                    (player.pos, player.y, player.vy, player.parkour),
                    (next.pos, next.y, next.vy, next.state),
                    "lane {center:?}, tick {tick}"
                );
                if next.state.wall_cooldown > step.state.wall_cooldown {
                    kicks.push(next.state.last_wall_normal);
                }
                high = high.max(next.y);
                assert!(length(next.state.velocity) <= MAX_HORIZONTAL_SPEED + 0.0001);
                step = next;
            }
            assert!(
                kicks.len() >= 2,
                "lane {center:?}, kicks {kicks:?}, high {high}"
            );
            assert!(kicks.windows(2).all(|pair| pair[0] != pair[1]));
            assert!(high > 3.5, "lane {center:?}, only rose {high}");
        }
    }

    #[test]
    fn raised_roofs_are_not_walls_but_upward_launches_bonk_their_underside() {
        let roof = stone([-5.0, -5.0], [5.0, 5.0], 4.0, 4.25);
        let input = MovementInput {
            jump: true,
            crouch: true,
            ..Default::default()
        };
        let below = advance(start([-5.61, 0.0], 1.0, -1.0), input, &[roof]);
        assert_eq!(below.state.wall_normal, [0.0; 2]);
        assert!(below.vy < 0.0);
        let wall = stone([0.0, -5.0], [0.1, 5.0], 0.0, 8.0);
        let touching = start([-0.61, 0.0], 2.1, -1.0);
        let jumped = advance(touching, input, &[wall, roof]);
        assert_eq!(
            jumped.bonked,
            Some(1),
            "wall jump's very first upward frame reports the roof"
        );
        near(jumped.y + BODY_H_STAND, 4.0);
        assert_eq!(jumped.vy, 0.0);
        let down = advance(
            start([-2.0, 0.0], 4.3, -8.0),
            MovementInput::default(),
            &[roof],
        );
        near(down.y, 4.25);
        assert!(down.grounded);
        assert_eq!(down.bonked, None);
    }

    #[test]
    fn substeps_prevent_thin_wall_tunneling_and_keep_harbor_bounds() {
        let wall = [stone([30.0, -4.0], [30.01, 4.0], 0.0, 8.0)];
        let state = ParkourState {
            velocity: [1000.0, 0.0],
            momentum: true,
            ..ParkourState::READY
        };
        let step = step_movement(
            [28.5, 0.0],
            2.0,
            0.0,
            state,
            MovementInput::default(),
            0.25,
            &wall,
            48.0,
        );
        assert!(step.pos[0] > 28.5 && step.pos[0] <= 29.4);
        assert!(length(step.state.velocity) <= MAX_HORIZONTAL_SPEED);
        let edge = step_movement(
            [47.3, 0.0],
            2.0,
            0.0,
            state,
            MovementInput::default(),
            1.0,
            &[],
            48.0,
        );
        assert!(edge.pos[0] <= 47.4 && edge.pos[0] >= 47.3);
        assert_eq!(
            edge.state.wall_normal, [0.0; 2],
            "invisible bounds do not grant wall kicks"
        );
    }

    #[test]
    fn state_roundtrip_and_snapshot_replay_are_deterministic() {
        let mut history = Vec::new();
        let mut authoritative = start([-15.0, 0.0], 0.0, 0.0);
        let mut checkpoint = authoritative;
        for tick in 0..80 {
            let input = MovementInput {
                mv: [1.0, 0.1],
                crouch: tick < 20,
                sprint: tick < 20,
                jump: tick == 5,
                ..Default::default()
            };
            authoritative = advance(authoritative, input, &[]);
            if tick == 11 {
                checkpoint = authoritative;
            }
            if tick > 11 {
                history.push(input);
            }
        }
        let wire = serde_json::to_string(&checkpoint.state).unwrap();
        checkpoint.state = serde_json::from_str(&wire).unwrap();
        for input in history {
            checkpoint = advance(checkpoint, input, &[]);
        }
        assert_eq!(checkpoint, authoritative);
    }

    #[test]
    fn different_wall_still_obeys_cooldown_and_air_steering_stays_capped() {
        let wall = [stone([0.0, -8.0], [0.1, 8.0], 0.0, 100.0)];
        let mut step = start([-0.61, 0.0], 20.0, -1.0);
        step.state.last_wall_normal = [1.0, 0.0];
        step.state.wall_cooldown = WALL_JUMP_COOLDOWN;
        let input = MovementInput {
            jump: true,
            crouch: true,
            ..Default::default()
        };
        step = advance(step, input, &wall);
        assert!(
            step.vy < 0.0,
            "opposite wall cannot bypass the .18 s launch cooldown"
        );
        for _ in 0..10 {
            step = advance(
                step,
                MovementInput {
                    jump: false,
                    ..input
                },
                &wall,
            );
        }
        assert_eq!(step.state.wall_cooldown, 0.0);
        step = advance(step, input, &wall);
        assert!(step.vy > 0.0);
        step.pos = [-10.0, 0.0];
        step.y = 100.0;
        for _ in 0..120 {
            step = advance(
                step,
                MovementInput {
                    mv: [-1.0, 1.0],
                    ..Default::default()
                },
                &[],
            );
            assert!(length(step.state.velocity) <= MAX_HORIZONTAL_SPEED + 0.0001);
        }
    }

    #[test]
    fn active_shield_prevents_slide_and_cancels_existing_ground_slide() {
        let input = MovementInput {
            mv: [1.0, 0.0],
            crouch: true,
            sprint: true,
            ..Default::default()
        };
        let initial = start([-10.0, 0.0], 0.0, 0.0);
        let shielded = advance(
            initial,
            MovementInput {
                shield: true,
                ..input
            },
            &[],
        );
        assert_eq!(shielded.state.slide_remaining, 0.0);
        near(
            shielded.state.velocity[0],
            MOVE_SPEED * crate::shooter::CROUCH_MULT,
        );
        let sliding = advance(initial, input, &[]);
        let cancelled = advance(
            sliding,
            MovementInput {
                shield: true,
                ..input
            },
            &[],
        );
        assert_eq!(cancelled.state.slide_remaining, 0.0);
        near(
            cancelled.state.velocity[0],
            MOVE_SPEED * crate::shooter::CROUCH_MULT,
        );
        near(cancelled.state.slide_cooldown, SLIDE_COOLDOWN);
    }

    #[test]
    fn narrow_crane_posts_are_solid_but_not_wall_grips() {
        let level = Level::harbor();
        for post in crate::harbor::CRANE_LEGS {
            let midpoint = [
                f32::midpoint(post.min[0], post.max[0]),
                f32::midpoint(post.min[1], post.max[1]),
            ];
            for (pos, toward) in [
                ([post.min[0] - PLAYER_R - 0.01, midpoint[1]], [1.0, 0.0]),
                ([post.max[0] + PLAYER_R + 0.01, midpoint[1]], [-1.0, 0.0]),
                ([midpoint[0], post.min[1] - PLAYER_R - 0.01], [0.0, 1.0]),
                ([midpoint[0], post.max[1] + PLAYER_R + 0.01], [0.0, -1.0]),
            ] {
                let input = MovementInput {
                    mv: toward,
                    jump: true,
                    crouch: true,
                    ..Default::default()
                };
                let step = step_movement(
                    pos,
                    5.0,
                    -8.0,
                    ParkourState::READY,
                    input,
                    FIXED_DT,
                    &level.obstacles,
                    level.arena_half,
                );
                assert_eq!(step.pos, pos, "post remains solid");
                assert_eq!(
                    step.state.wall_normal, [0.0; 2],
                    "narrow crane face cannot supply a grip"
                );
                assert!(step.vy < -8.0, "neither wall slide nor kick on a post");
                assert!(!step.state.momentum);
            }
        }
        let wall = stone(
            [0.0, -MIN_WALL_SPAN / 2.0],
            [0.1, MIN_WALL_SPAN / 2.0],
            0.0,
            8.0,
        );
        let step = advance(
            start([-PLAYER_R - 0.01, 0.0], 2.0, -1.0),
            MovementInput {
                jump: true,
                crouch: true,
                ..Default::default()
            },
            &[wall],
        );
        assert!(
            step.vy > 0.0,
            "a real wall at the minimum face span still works"
        );
        assert_eq!(step.state.last_wall_normal, [-1.0, 0.0]);
    }

    #[test]
    fn invalid_values_cannot_create_unbounded_motion() {
        let broken = ParkourState {
            velocity: [f32::INFINITY, f32::NAN],
            slide_remaining: f32::NAN,
            wall_cooldown: -10.0,
            ..ParkourState::READY
        };
        for dt in [f32::NAN, f32::INFINITY, -1.0, 0.0, 10.0] {
            let step = step_movement(
                [f32::NAN, f32::INFINITY],
                f32::NAN,
                f32::INFINITY,
                broken,
                MovementInput {
                    mv: [f32::NAN, f32::INFINITY],
                    jump: true,
                    sprint: true,
                    crouch: true,
                    shield: false,
                },
                dt,
                &[],
                f32::NAN,
            );
            assert!(step.pos.iter().all(|value| value.is_finite()));
            assert!(step.y.is_finite() && step.vy.is_finite());
            assert!(length(step.state.velocity) <= MAX_HORIZONTAL_SPEED);
            assert_eq!(step.state.slide_remaining, 0.0);
        }
    }
}
