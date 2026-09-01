//! The arena shooter sim: pure, deterministic, fixed 60 Hz. Runs
//! authoritatively on the server; clients render its broadcast state and
//! generate the identical arena from the lobby's seed.

/// Fixed simulation time step in seconds.
pub const FIXED_DT: f32 = 1.0 / 60.0;
/// Arena boundary half-extent.
pub const ARENA_HALF: f32 = 24.0;
/// Base horizontal player speed in world units per second.
pub const MOVE_SPEED: f32 = 9.0;
/// Shift: faster. C: slower (and a lower profile, cosmetically).
pub const SPRINT_MULT: f32 = 1.6;
/// Horizontal speed multiplier while crouched.
pub const CROUCH_MULT: f32 = 0.55;
/// Player collision radius.
pub const PLAYER_R: f32 = 0.6;
/// Bullet speed in world units per second.
pub const BULLET_SPEED: f32 = 34.0;
/// Bullet collision radius.
pub const BULLET_R: f32 = 0.22;
/// Bullet lifetime in seconds.
pub const BULLET_TTL: f32 = 1.6;
/// Minimum interval between shots in seconds.
pub const FIRE_COOLDOWN: f32 = 0.18;
/// Active bullets per player, so holding fire can't flood the state.
pub const MAX_BULLETS_PER_PLAYER: usize = 10;
/// Full player hit points.
pub const MAX_HP: u8 = 3;
/// Delay between death and respawn in seconds.
pub const RESPAWN_SECS: f32 = 2.5;
/// Maximum players admitted to one Arena lobby.
pub const MAX_PLAYERS: usize = 8;
const SPAWN_RING_R: f32 = ARENA_HALF - 4.0;

/// Axis-aligned obstacle on the XZ plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    /// Minimum horizontal X/Z corner.
    pub min: [f32; 2],
    /// Maximum horizontal X/Z corner.
    pub max: [f32; 2],
}

/// Deterministic arena from a seed: every client and the server generate
/// the same obstacle course. Obstacles stay inside a donut that keeps both
/// the center brawl area and the spawn ring clear.
// These integer-to-float casts are part of the deterministic arena formula's expression tree.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn generate_arena(seed: u64) -> Vec<Obstacle> {
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut rand01 = move || -> f32 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Top 32 bits over 2^32: uniform in [0, 1). (Dividing 31 bits by
        // u32::MAX halved the range and put every obstacle in one half of
        // the arena.)
        ((state >> 32) as u32) as f32 / (u32::MAX as f32 + 1.0)
    };
    let mut obstacles = Vec::with_capacity(14);
    for _ in 0..14 {
        let angle = rand01() * std::f32::consts::TAU;
        let radius = 5.0 + rand01() * 10.5; // 5..15.5, spawn ring is at 20
        let cx = angle.cos() * radius;
        let cz = angle.sin() * radius;
        let hx = 0.8 + rand01() * 1.7;
        let hz = 0.8 + rand01() * 1.7;
        obstacles.push(Obstacle {
            min: [cx - hx, cz - hz],
            max: [cx + hx, cz + hz],
        });
    }
    obstacles
}

// This cast is part of the deterministic spawn formula's expression tree.
#[allow(clippy::cast_precision_loss)]
fn spawn_point(slot: u32) -> [f32; 2] {
    let angle = slot as f32 * 2.399_963; // golden angle: spread out
    [angle.cos() * SPAWN_RING_R, angle.sin() * SPAWN_RING_R]
}

fn blocked(pos: [f32; 2], r: f32, obstacles: &[Obstacle]) -> bool {
    if pos[0].abs() > ARENA_HALF - r || pos[1].abs() > ARENA_HALF - r {
        return true;
    }
    obstacles.iter().any(|o| {
        let cx = pos[0].clamp(o.min[0], o.max[0]);
        let cz = pos[1].clamp(o.min[1], o.max[1]);
        let (dx, dz) = (pos[0] - cx, pos[1] - cz);
        dx * dx + dz * dz < r * r
    })
}

/// Stance-adjusted movement speed. Crouch wins if both are held.
#[must_use]
pub fn stance_speed(sprint: bool, crouch: bool) -> f32 {
    MOVE_SPEED
        * if crouch {
            CROUCH_MULT
        } else if sprint {
            SPRINT_MULT
        } else {
            1.0
        }
}

/// Player movement: sanitize the intent, then integrate one axis at a time
/// so walls slide. Shared VERBATIM by the server sim and the client's
/// prediction, so both compute the exact same result.
#[must_use]
pub fn move_circle(
    pos: [f32; 2],
    mv: [f32; 2],
    speed: f32,
    dt: f32,
    obstacles: &[Obstacle],
) -> [f32; 2] {
    let mut mv = mv;
    if !mv[0].is_finite() || !mv[1].is_finite() {
        mv = [0.0, 0.0];
    }
    let len_sq = mv[0] * mv[0] + mv[1] * mv[1];
    if len_sq > 1.0 {
        let len = len_sq.sqrt();
        mv = [mv[0] / len, mv[1] / len];
    }
    let try_x = [pos[0] + mv[0] * speed * dt, pos[1]];
    let pos = if blocked(try_x, PLAYER_R, obstacles) { pos } else { try_x };
    let try_z = [pos[0], pos[1] + mv[1] * speed * dt];
    if blocked(try_z, PLAYER_R, obstacles) {
        pos
    } else {
        try_z
    }
}

#[derive(Clone, Copy, Debug, Default)]
/// Held player intents consumed by one simulation step.
pub struct PlayerIn {
    /// Held movement intent, -1..1 per axis (world space).
    pub mv: [f32; 2],
    /// Aim direction (normalized by the sim if not).
    pub aim: [f32; 2],
    /// Whether the trigger is held.
    pub fire: bool,
    /// Whether sprint is held.
    pub sprint: bool,
    /// Whether crouch is held.
    pub crouch: bool,
}

#[derive(Clone, Debug)]
/// Authoritative mutable state for one Arena player.
pub struct PlayerSt {
    /// In-lobby player identifier.
    pub id: u8,
    /// Horizontal X/Z position.
    pub pos: [f32; 2],
    /// Normalized horizontal aim direction.
    pub aim: [f32; 2],
    /// Remaining hit points.
    pub hp: u8,
    /// Authoritative kill score.
    pub score: u32,
    /// Whether the player is active rather than awaiting respawn.
    pub alive: bool,
    /// Whether the player uses the crouched stance.
    pub crouch: bool,
    /// Seconds remaining until respawn.
    pub respawn_in: f32,
    /// Seconds remaining until the next shot may be fired.
    pub cooldown: f32,
    deaths: u32,
}

#[derive(Clone, Copy, Debug)]
/// One authoritative bullet in flight.
pub struct Bullet {
    /// Horizontal X/Z position.
    pub pos: [f32; 2],
    /// Horizontal X/Z velocity.
    pub vel: [f32; 2],
    /// Remaining lifetime in seconds.
    pub ttl: f32,
    /// In-lobby identifier of the firing player.
    pub owner: u8,
}

/// Deterministic authoritative Arena simulation state.
pub struct Sim {
    /// Generated collision obstacles.
    pub obstacles: Vec<Obstacle>,
    /// Admitted player states.
    pub players: Vec<PlayerSt>,
    /// Active bullets.
    pub bullets: Vec<Bullet>,
    /// (killer, victim) pairs from the last step.
    pub events: Vec<(u8, u8)>,
}

impl Sim {
    /// Constructs a deterministic Arena from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            obstacles: generate_arena(seed),
            players: Vec::new(),
            bullets: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Adds a player at the next deterministic spawn slot.
    pub fn add_player(&mut self, id: u8) {
        // Player count is capped at eight by the public protocol.
        #[allow(clippy::cast_possible_truncation)]
        let slot = self.players.len() as u32;
        self.players.push(PlayerSt {
            id,
            pos: spawn_point(slot),
            aim: [1.0, 0.0],
            hp: MAX_HP,
            score: 0,
            alive: true,
            crouch: false,
            respawn_in: 0.0,
            cooldown: 0.0,
            deaths: slot,
        });
    }

    /// Removes a player and every bullet they own.
    pub fn remove_player(&mut self, id: u8) {
        self.players.retain(|p| p.id != id);
        self.bullets.retain(|b| b.owner != id);
    }

    /// Advances the authoritative state by exactly one fixed step.
    pub fn step(&mut self, inputs: &dyn Fn(u8) -> PlayerIn) {
        self.events.clear();
        let dt = FIXED_DT;

        // Players: respawn timers, movement (axis-separated slide), firing.
        let mut new_bullets: Vec<Bullet> = Vec::new();
        for i in 0..self.players.len() {
            let input = inputs(self.players[i].id);
            if !self.players[i].alive {
                let p = &mut self.players[i];
                p.respawn_in -= dt;
                if p.respawn_in <= 0.0 {
                    p.deaths = p.deaths.wrapping_add(1);
                    let point =
                        spawn_point(p.deaths.wrapping_mul(3).wrapping_add(u32::from(p.id)));
                    p.pos = point;
                    p.hp = MAX_HP;
                    p.alive = true;
                    p.cooldown = 0.3;
                }
                continue;
            }

            // Shared movement code (also used by client prediction);
            // stance speed is server-authoritative — no speed cheats.
            let speed = stance_speed(input.sprint, input.crouch);
            let pos = move_circle(self.players[i].pos, input.mv, speed, dt, &self.obstacles);
            let p = &mut self.players[i];
            p.pos = pos;
            p.crouch = input.crouch;

            // Aim.
            let mut aim = input.aim;
            if !aim[0].is_finite() || !aim[1].is_finite() {
                aim = [1.0, 0.0];
            }
            let alen = (aim[0] * aim[0] + aim[1] * aim[1]).sqrt();
            if alen > 1e-4 {
                p.aim = [aim[0] / alen, aim[1] / alen];
            }

            // Fire.
            p.cooldown = (p.cooldown - dt).max(0.0);
            if input.fire && p.cooldown == 0.0 {
                let owner = p.id;
                let active = self.bullets.iter().filter(|b| b.owner == owner).count()
                    + new_bullets.iter().filter(|b| b.owner == owner).count();
                if active < MAX_BULLETS_PER_PLAYER {
                    let p = &mut self.players[i];
                    p.cooldown = FIRE_COOLDOWN;
                    // Spawn just in front of the center: the swept collision
                    // below skips the owner, and starting further out would
                    // leave a point-blank dead zone.
                    let muzzle = [
                        p.pos[0] + p.aim[0] * 0.2,
                        p.pos[1] + p.aim[1] * 0.2,
                    ];
                    new_bullets.push(Bullet {
                        pos: muzzle,
                        vel: [p.aim[0] * BULLET_SPEED, p.aim[1] * BULLET_SPEED],
                        ttl: BULLET_TTL,
                        owner,
                    });
                }
            }
        }
        self.bullets.extend(new_bullets);

        // Bullets: swept player collision (segment vs circle — no tunneling,
        // no point-blank dead zone), then integrate, then world collision.
        let mut hits: Vec<(u8, u8)> = Vec::new(); // (owner, victim)
        let obstacles = std::mem::take(&mut self.obstacles);
        let players = &self.players;
        let mut bullets = std::mem::take(&mut self.bullets);
        bullets.retain_mut(|b| {
            b.ttl -= dt;
            if b.ttl <= 0.0 {
                return false;
            }
            let p0 = b.pos;
            let p1 = [p0[0] + b.vel[0] * dt, p0[1] + b.vel[1] * dt];
            let (sx, sz) = (p1[0] - p0[0], p1[1] - p0[1]);
            let seg_len_sq = sx * sx + sz * sz;
            let rr = PLAYER_R + BULLET_R;
            for p in players.iter() {
                if !p.alive || p.id == b.owner {
                    continue;
                }
                // Closest point on [p0, p1] to the player's center.
                let t = if seg_len_sq <= 1e-8 {
                    0.0
                } else {
                    (((p.pos[0] - p0[0]) * sx + (p.pos[1] - p0[1]) * sz) / seg_len_sq)
                        .clamp(0.0, 1.0)
                };
                let (ex, ez) = (p.pos[0] - (p0[0] + sx * t), p.pos[1] - (p0[1] + sz * t));
                if ex * ex + ez * ez < rr * rr {
                    hits.push((b.owner, p.id));
                    return false;
                }
            }
            b.pos = p1;
            if b.pos[0].abs() > ARENA_HALF - BULLET_R || b.pos[1].abs() > ARENA_HALF - BULLET_R {
                return false;
            }
            if obstacles.iter().any(|o| {
                b.pos[0] > o.min[0] - BULLET_R
                    && b.pos[0] < o.max[0] + BULLET_R
                    && b.pos[1] > o.min[1] - BULLET_R
                    && b.pos[1] < o.max[1] + BULLET_R
            }) {
                return false;
            }
            true
        });
        self.bullets = bullets;
        self.obstacles = obstacles;

        // Apply damage after the bullet pass (avoids double-borrow).
        for (owner, victim) in hits {
            let Some(v) = self.players.iter_mut().find(|p| p.id == victim) else { continue };
            if !v.alive {
                continue;
            }
            v.hp = v.hp.saturating_sub(1);
            if v.hp == 0 {
                v.alive = false;
                v.respawn_in = RESPAWN_SECS;
                self.events.push((owner, victim));
                if let Some(k) = self.players.iter_mut().find(|p| p.id == owner) {
                    k.score += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn step_with(sim: &mut Sim, inputs: &HashMap<u8, PlayerIn>) {
        sim.step(&|id| inputs.get(&id).copied().unwrap_or_default());
    }

    #[test]
    fn arena_is_deterministic_and_bounded() {
        let a = generate_arena(42);
        let b = generate_arena(42);
        let c = generate_arena(43);
        assert_eq!(a, b, "same seed, same arena");
        assert_ne!(a, c, "different seed, different arena");
        for o in &a {
            assert!(o.max[0] < ARENA_HALF - 4.0 && o.max[1] < ARENA_HALF - 4.0);
            assert!(o.min[0] > -(ARENA_HALF - 4.0) && o.min[1] > -(ARENA_HALF - 4.0));
        }
    }

    #[test]
    fn arena_covers_all_quadrants() {
        // Guards the RNG range: a half-range generator (the [0, 0.5) bug)
        // could never place obstacles at negative z.
        let (mut neg_x, mut neg_z, mut pos_x, mut pos_z) = (false, false, false, false);
        for seed in 0..16u64 {
            for o in generate_arena(seed) {
                let cx = (o.min[0] + o.max[0]) * 0.5;
                let cz = (o.min[1] + o.max[1]) * 0.5;
                neg_x |= cx < -3.0;
                pos_x |= cx > 3.0;
                neg_z |= cz < -3.0;
                pos_z |= cz > 3.0;
            }
        }
        assert!(
            neg_x && neg_z && pos_x && pos_z,
            "obstacles must appear in all four quadrants across seeds"
        );
    }

    #[test]
    fn sprint_and_crouch_change_speed() {
        let run = |sprint: bool, crouch: bool| -> f32 {
            let mut sim = Sim::new(9);
            sim.obstacles.clear();
            sim.add_player(0);
            sim.players[0].pos = [-20.0, 0.0];
            let mut inputs = HashMap::new();
            inputs.insert(0, PlayerIn { mv: [1.0, 0.0], aim: [1.0, 0.0], fire: false, sprint, crouch });
            for _ in 0..60 {
                step_with(&mut sim, &inputs);
            }
            sim.players[0].pos[0] + 20.0
        };
        let normal = run(false, false);
        let sprint = run(true, false);
        let crouch = run(false, true);
        assert!((normal - MOVE_SPEED).abs() < 0.2);
        assert!((sprint - MOVE_SPEED * SPRINT_MULT).abs() < 0.3);
        assert!((crouch - MOVE_SPEED * CROUCH_MULT).abs() < 0.2);
        // Crouch wins if both are held.
        let both = run(true, true);
        assert!((both - crouch).abs() < 0.2);
    }

    #[test]
    fn point_blank_shots_connect() {
        let mut sim = Sim::new(4);
        sim.obstacles.clear();
        sim.add_player(0);
        sim.add_player(1);
        let mut inputs = HashMap::new();
        inputs.insert(0, PlayerIn { mv: [0.0, 0.0], aim: [1.0, 0.0], fire: true, ..Default::default() });
        // Victim glued right in front of the shooter, inside the old
        // dead zone.
        let mut killed = false;
        for _ in 0..600 {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => p.pos = [0.0, 0.0],
                1 if p.alive => p.pos = [0.4, 0.0],
                _ => {}
            });
            step_with(&mut sim, &inputs);
            if !sim.events.is_empty() {
                killed = true;
                break;
            }
        }
        assert!(killed, "point-blank shots must connect");
    }

    #[test]
    fn walls_block_movement() {
        let mut sim = Sim::new(1);
        sim.add_player(0);
        sim.players[0].pos = [ARENA_HALF - PLAYER_R - 0.05, 0.0];
        let mut inputs = HashMap::new();
        inputs.insert(0, PlayerIn { mv: [1.0, 0.0], aim: [1.0, 0.0], fire: false, ..Default::default() });
        for _ in 0..30 {
            step_with(&mut sim, &inputs);
        }
        assert!(sim.players[0].pos[0] <= ARENA_HALF - PLAYER_R + 1e-3);
    }

    #[test]
    fn three_hits_kill_score_and_respawn() {
        let mut sim = Sim::new(2);
        sim.obstacles.clear(); // open field for a clean shot
        sim.add_player(0);
        sim.add_player(1);
        sim.players[0].pos = [-5.0, 0.0];
        sim.players[1].pos = [5.0, 0.0];
        let mut inputs = HashMap::new();
        inputs.insert(0, PlayerIn { mv: [0.0, 0.0], aim: [1.0, 0.0], fire: true, ..Default::default() });
        let mut killed_at = None;
        for i in 0..600 {
            // Keep the victim parked.
            sim.players.iter_mut().for_each(|p| {
                if p.id == 1 && p.alive {
                    p.pos = [5.0, 0.0];
                }
            });
            step_with(&mut sim, &inputs);
            if !sim.events.is_empty() {
                killed_at = Some(i);
                break;
            }
        }
        assert!(killed_at.is_some(), "victim was never killed");
        assert_eq!(sim.events, vec![(0, 1)]);
        let killer = sim.players.iter().find(|p| p.id == 0).unwrap();
        assert_eq!(killer.score, 1);
        let victim = sim.players.iter().find(|p| p.id == 1).unwrap();
        assert!(!victim.alive);

        // Victim comes back at full HP.
        let empty = HashMap::new();
        for _ in 0..((RESPAWN_SECS / FIXED_DT) as u32 + 2) {
            step_with(&mut sim, &empty);
        }
        let victim = sim.players.iter().find(|p| p.id == 1).unwrap();
        assert!(victim.alive);
        assert_eq!(victim.hp, MAX_HP);
    }

    #[test]
    fn bullet_cap_holds() {
        let mut sim = Sim::new(3);
        sim.obstacles.clear();
        sim.add_player(0);
        sim.players[0].pos = [0.0, 0.0];
        let mut inputs = HashMap::new();
        // Fire along +x forever with no cooldown constraint violations.
        inputs.insert(0, PlayerIn { mv: [0.0, 0.0], aim: [1.0, 0.0], fire: true, ..Default::default() });
        for _ in 0..240 {
            step_with(&mut sim, &inputs);
            // Teleport bullets back so they never expire or leave.
            sim.bullets.iter_mut().for_each(|b| {
                b.ttl = BULLET_TTL;
                b.pos = [0.0, 5.0];
                b.vel = [0.0, 0.0];
            });
            assert!(sim.bullets.len() <= MAX_BULLETS_PER_PLAYER);
        }
    }
}
