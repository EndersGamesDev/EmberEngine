//! The arena shooter sim: pure, deterministic, fixed 60 Hz. Runs
//! authoritatively on the server; clients render its broadcast state and
//! generate the identical arena from the lobby's seed.

/// Frozen Arena v2 fixed dt simulation value.
pub const FIXED_DT: f32 = 1.0 / 60.0;
/// Frozen Arena v2 arena half simulation value.
pub const ARENA_HALF: f32 = 24.0;
/// Frozen Arena v2 move speed simulation value.
pub const MOVE_SPEED: f32 = 12.0;
/// Frozen Arena v2 player r simulation value.
pub const PLAYER_R: f32 = 0.6;
/// Frozen Arena v2 bullet speed simulation value.
pub const BULLET_SPEED: f32 = 34.0;
/// Frozen Arena v2 bullet r simulation value.
pub const BULLET_R: f32 = 0.22;
/// Frozen Arena v2 bullet ttl simulation value.
pub const BULLET_TTL: f32 = 1.6;
/// Frozen Arena v2 fire cooldown simulation value.
pub const FIRE_COOLDOWN: f32 = 0.18;
/// Active bullets per player, so holding fire can't flood the state.
/// Frozen Arena v2 max bullets per player simulation value.
pub const MAX_BULLETS_PER_PLAYER: usize = 10;
/// Frozen Arena v2 max hp simulation value.
pub const MAX_HP: u8 = 3;
/// Frozen Arena v2 respawn secs simulation value.
pub const RESPAWN_SECS: f32 = 2.5;
/// Frozen Arena v2 max players simulation value.
pub const MAX_PLAYERS: usize = 8;
const SPAWN_RING_R: f32 = ARENA_HALF - 4.0;

/// Axis-aligned obstacle on the XZ plane.
#[derive(Clone, Copy, Debug, PartialEq)]
/// Frozen Arena v2 Obstacle simulation state.
pub struct Obstacle {
    /// Frozen Arena v2 min field.
    pub min: [f32; 2],
    /// Frozen Arena v2 max field.
    pub max: [f32; 2],
}

/// Deterministic arena from a seed: every client and the server generate
/// the same obstacle course. Obstacles stay inside a donut that keeps both
/// the center brawl area and the spawn ring clear.
#[allow(clippy::cast_precision_loss)]
#[must_use]
/// Executes the frozen Arena v2 generate arena operation.
pub fn generate_arena(seed: u64) -> Vec<Obstacle> {
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut rand01 = move || -> f32 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as u32) as f32 / u32::MAX as f32
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

#[allow(clippy::cast_precision_loss)]
fn spawn_point(slot: u32) -> [f32; 2] {
    let angle = slot as f32 * 2.399_963; // golden angle: spread out
    [angle.cos() * SPAWN_RING_R, angle.sin() * SPAWN_RING_R]
}

#[derive(Clone, Copy, Debug, Default)]
/// Frozen Arena v2 PlayerIn simulation state.
pub struct PlayerIn {
    /// Held movement intent, -1..1 per axis.
    /// Frozen Arena v2 mv field.
    pub mv: [f32; 2],
    /// Aim direction (normalized by the sim if not).
    /// Frozen Arena v2 aim field.
    pub aim: [f32; 2],
    /// Frozen Arena v2 fire field.
    pub fire: bool,
}

#[derive(Clone, Debug)]
/// Frozen Arena v2 PlayerSt simulation state.
pub struct PlayerSt {
    /// Frozen Arena v2 id field.
    pub id: u8,
    /// Frozen Arena v2 pos field.
    pub pos: [f32; 2],
    /// Frozen Arena v2 aim field.
    pub aim: [f32; 2],
    /// Frozen Arena v2 hp field.
    pub hp: u8,
    /// Frozen Arena v2 score field.
    pub score: u32,
    /// Frozen Arena v2 alive field.
    pub alive: bool,
    /// Frozen Arena v2 respawn in field.
    pub respawn_in: f32,
    /// Frozen Arena v2 cooldown field.
    pub cooldown: f32,
    deaths: u32,
}

#[derive(Clone, Copy, Debug)]
/// Frozen Arena v2 Bullet simulation state.
pub struct Bullet {
    /// Frozen Arena v2 pos field.
    pub pos: [f32; 2],
    /// Frozen Arena v2 vel field.
    pub vel: [f32; 2],
    /// Frozen Arena v2 ttl field.
    pub ttl: f32,
    /// Frozen Arena v2 owner field.
    pub owner: u8,
}

/// Frozen Arena v2 Sim simulation state.
pub struct Sim {
    /// Frozen Arena v2 obstacles field.
    pub obstacles: Vec<Obstacle>,
    /// Frozen Arena v2 players field.
    pub players: Vec<PlayerSt>,
    /// Frozen Arena v2 bullets field.
    pub bullets: Vec<Bullet>,
    /// (killer, victim) pairs from the last step.
    /// Frozen Arena v2 events field.
    pub events: Vec<(u8, u8)>,
}

impl Sim {
    #[must_use]
    /// Executes the frozen Arena v2 new operation.
    pub fn new(seed: u64) -> Self {
        Self {
            obstacles: generate_arena(seed),
            players: Vec::new(),
            bullets: Vec::new(),
            events: Vec::new(),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    /// Executes the frozen Arena v2 add player operation.
    pub fn add_player(&mut self, id: u8) {
        let slot = self.players.len() as u32;
        self.players.push(PlayerSt {
            id,
            pos: spawn_point(slot),
            aim: [1.0, 0.0],
            hp: MAX_HP,
            score: 0,
            alive: true,
            respawn_in: 0.0,
            cooldown: 0.0,
            deaths: slot,
        });
    }

    /// Executes the frozen Arena v2 remove player operation.
    pub fn remove_player(&mut self, id: u8) {
        self.players.retain(|p| p.id != id);
        self.bullets.retain(|b| b.owner != id);
    }

    fn blocked(&self, pos: [f32; 2], r: f32) -> bool {
        if pos[0].abs() > ARENA_HALF - r || pos[1].abs() > ARENA_HALF - r {
            return true;
        }
        self.obstacles.iter().any(|o| {
            let cx = pos[0].clamp(o.min[0], o.max[0]);
            let cz = pos[1].clamp(o.min[1], o.max[1]);
            let (dx, dz) = (pos[0] - cx, pos[1] - cz);
            dx * dx + dz * dz < r * r
        })
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_lines
    )]
    /// Executes the frozen Arena v2 step operation.
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
                    let point = spawn_point(p.deaths.wrapping_mul(3).wrapping_add(p.id as u32));
                    p.pos = point;
                    p.hp = MAX_HP;
                    p.alive = true;
                    p.cooldown = 0.3;
                }
                continue;
            }

            // Sanitize + apply movement, one axis at a time so walls slide.
            let mut mv = input.mv;
            if !mv[0].is_finite() || !mv[1].is_finite() {
                mv = [0.0, 0.0];
            }
            let len_sq = mv[0] * mv[0] + mv[1] * mv[1];
            if len_sq > 1.0 {
                let len = len_sq.sqrt();
                mv = [mv[0] / len, mv[1] / len];
            }
            let pos = self.players[i].pos;
            let try_x = [pos[0] + mv[0] * MOVE_SPEED * dt, pos[1]];
            let pos = if self.blocked(try_x, PLAYER_R) {
                pos
            } else {
                try_x
            };
            let try_z = [pos[0], pos[1] + mv[1] * MOVE_SPEED * dt];
            let pos = if self.blocked(try_z, PLAYER_R) {
                pos
            } else {
                try_z
            };
            let p = &mut self.players[i];
            p.pos = pos;

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
                    let muzzle = [
                        p.pos[0] + p.aim[0] * (PLAYER_R + BULLET_R + 0.15),
                        p.pos[1] + p.aim[1] * (PLAYER_R + BULLET_R + 0.15),
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

        // Bullets: integrate, expire, collide with world and players.
        let mut hits: Vec<(u8, u8)> = Vec::new(); // (owner, victim)
        let obstacles = std::mem::take(&mut self.obstacles);
        self.bullets.retain_mut(|b| {
            b.ttl -= dt;
            if b.ttl <= 0.0 {
                return false;
            }
            b.pos[0] += b.vel[0] * dt;
            b.pos[1] += b.vel[1] * dt;
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
            for p in &self.players {
                if !p.alive || p.id == b.owner {
                    continue;
                }
                let (dx, dz) = (p.pos[0] - b.pos[0], p.pos[1] - b.pos[1]);
                let rr = PLAYER_R + BULLET_R;
                if dx * dx + dz * dz < rr * rr {
                    hits.push((b.owner, p.id));
                    return false;
                }
            }
            true
        });
        self.obstacles = obstacles;

        // Apply damage after the bullet pass (avoids double-borrow).
        for (owner, victim) in hits {
            let Some(v) = self.players.iter_mut().find(|p| p.id == victim) else {
                continue;
            };
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
    fn walls_block_movement() {
        let mut sim = Sim::new(1);
        sim.add_player(0);
        sim.players[0].pos = [ARENA_HALF - PLAYER_R - 0.05, 0.0];
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                mv: [1.0, 0.0],
                aim: [1.0, 0.0],
                fire: false,
            },
        );
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
        inputs.insert(
            0,
            PlayerIn {
                mv: [0.0, 0.0],
                aim: [1.0, 0.0],
                fire: true,
            },
        );
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
        inputs.insert(
            0,
            PlayerIn {
                mv: [0.0, 0.0],
                aim: [1.0, 0.0],
                fire: true,
            },
        );
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
