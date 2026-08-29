//! The arena shooter sim: pure, deterministic, fixed 60 Hz. Runs
//! authoritatively on the server; clients render its broadcast state and
//! generate the identical arena from the lobby's seed.

pub const FIXED_DT: f32 = 1.0 / 60.0;
pub const ARENA_HALF: f32 = 24.0;
pub const MOVE_SPEED: f32 = 9.0;
/// Shift: faster. C: slower (and a lower profile, cosmetically).
pub const SPRINT_MULT: f32 = 1.6;
pub const CROUCH_MULT: f32 = 0.55;
pub const PLAYER_R: f32 = 0.6;
/// Crouching shrinks the HIT circle (movement blocking keeps PLAYER_R).
pub const CROUCH_HIT_MULT: f32 = 0.72;

/// Stance-dependent hit-test radius: crouch = lower profile = smaller target.
pub fn hit_radius(crouch: bool) -> f32 {
    if crouch {
        PLAYER_R * CROUCH_HIT_MULT
    } else {
        PLAYER_R
    }
}

/// Eye height above the feet, per stance. A shot leaves the weapon at eye
/// level, so this is a bullet's starting height.
pub const EYE_STAND: f32 = 1.45;
pub const EYE_CROUCH: f32 = 0.85;
/// Body height above the feet, per stance: the vertical extent of the hit
/// volume. Matched to the silhouette the client draws (head centre plus half
/// a head) so that what you see is what you hit — these live here, not in
/// the renderer, because client and server must agree where a body IS.
pub const BODY_H_STAND: f32 = 1.70;
pub const BODY_H_CROUCH: f32 = 1.25;
// Worth knowing before tuning either of the above: a standing shooter's
// muzzle sits at 1.45 and a crouched target's band tops out at
// BODY_H_CROUCH + BULLET_R = 1.47, so perfectly level fire GRAZES a
// crouching player rather than passing over them. That is the honest
// geometry, not a fudge — but it is a 2 cm margin, so lowering
// BODY_H_CROUCH turns level fire into a clean miss and makes crouch
// dramatically stronger. Aiming AT a crouched target pitches down and hits
// solidly either way.

/// Height a shot leaves from, measured from the shooter's feet.
pub fn eye_h(crouch: bool) -> f32 {
    if crouch {
        EYE_CROUCH
    } else {
        EYE_STAND
    }
}

/// Vertical extent of the hit volume, measured from the target's feet.
/// Together with `hit_radius` this makes the hitbox a finite cylinder; it
/// used to be one of infinite height, which is why pitch never mattered.
pub fn body_h(crouch: bool) -> f32 {
    if crouch {
        BODY_H_CROUCH
    } else {
        BODY_H_STAND
    }
}

/// Hard clamp on aim pitch, radians (~83°). The client clamps its own look
/// identically, but that clamp is cosmetic — a peer's pitch is untrusted
/// input and is re-clamped here, where it decides who dies.
pub const MAX_PITCH: f32 = 1.45;
pub const BULLET_SPEED: f32 = 34.0;
pub const BULLET_R: f32 = 0.22;
pub const BULLET_TTL: f32 = 1.6;
pub const RELOAD_SECS: f32 = 1.1;
pub const MAX_WEAPON: u8 = 3;
pub const PAD_RESPAWN_SECS: f32 = 15.0;
pub const PAD_RADIUS: f32 = 1.3;

/// Weapon levels: 1 = pistol, 2 = rapid, 3 = heavy. Picked up on pads,
/// reset on death.
#[derive(Clone, Copy, Debug)]
pub struct WeaponStats {
    pub cooldown: f32,
    pub mag: u8,
    pub damage: u8,
}

pub fn weapon_stats(level: u8) -> WeaponStats {
    match level {
        3 => WeaponStats {
            cooldown: 0.22,
            mag: 6,
            damage: 2,
        },
        2 => WeaponStats {
            cooldown: 0.11,
            mag: 12,
            damage: 1,
        },
        _ => WeaponStats {
            cooldown: 0.18,
            mag: 8,
            damage: 1,
        },
    }
}

pub fn weapon_name(level: u8) -> &'static str {
    match level {
        3 => "Heavy",
        2 => "Rapid",
        _ => "Pistol",
    }
}
/// Active bullets per player, so holding fire can't flood the state.
pub const MAX_BULLETS_PER_PLAYER: usize = 10;
pub const MAX_HP: u8 = 3;
pub const RESPAWN_SECS: f32 = 2.5;
pub const MAX_PLAYERS: usize = 8;
/// Lag compensation: hit tests may rewind targets at most this many ticks
/// (300 ms) toward what the shooter was seeing.
pub const MAX_REWIND_TICKS: u16 = 18;
const HISTORY_LEN: usize = MAX_REWIND_TICKS as usize + 2;
const SPAWN_RING_R: f32 = ARENA_HALF - 4.0;

/// Downward acceleration while airborne, units/s².
pub const GRAVITY: f32 = -24.0;
/// Take-off speed. The apex (v²/2g ≈ 1.76) clears every crate top with a
/// little room, and falls well short of the containers.
pub const JUMP_VEL: f32 = 9.2;
/// Cover comes in two classes: crates you can jump onto, and containers
/// that stay hard cover. These bound the low class.
pub const CRATE_MIN_H: f32 = 0.9;
pub const CRATE_MAX_H: f32 = 1.5;
pub const CONTAINER_MIN_H: f32 = 2.4;
/// A player may only be pushed up onto a surface this much higher than
/// their feet; anything taller is a wall to them.
pub const STEP_UP: f32 = 0.35;

/// Axis-aligned obstacle on the XZ plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

/// Height of a cover box, derived from its own coordinates so the server,
/// every client's collision, and the renderer agree without sending it.
///
/// Roughly three in five are crates low enough to jump onto and fight from;
/// the rest are shipping containers that stay hard cover.
pub fn obstacle_height(o: &Obstacle) -> f32 {
    let k = (o.min[0] * 12.9898 + o.min[1] * 78.233).sin() * 43758.547;
    let f = k - k.floor();
    if f < 0.6 {
        CRATE_MIN_H + (f / 0.6) * (CRATE_MAX_H - CRATE_MIN_H)
    } else {
        CONTAINER_MIN_H + ((f - 0.6) / 0.4) * 0.8
    }
}

/// The surface a player at `pos` stands on: the tallest box top they
/// overlap, or the arena floor. Uses the same overlap test as `blocked`,
/// so a player can only ever be supported by a box they are actually on.
pub fn support_height(pos: [f32; 2], r: f32, obstacles: &[Obstacle]) -> f32 {
    let mut h = 0.0f32;
    for o in obstacles {
        if overlaps(pos, r, o) {
            h = h.max(obstacle_height(o));
        }
    }
    h
}

fn overlaps(pos: [f32; 2], r: f32, o: &Obstacle) -> bool {
    let cx = pos[0].clamp(o.min[0], o.max[0]);
    let cz = pos[1].clamp(o.min[1], o.max[1]);
    let (dx, dz) = (pos[0] - cx, pos[1] - cz);
    dx * dx + dz * dz < r * r
}

/// Deterministic arena from a seed: every client and the server generate
/// the same obstacle course. Obstacles stay inside a donut that keeps both
/// the center brawl area and the spawn ring clear.
pub fn generate_arena(seed: u64) -> Vec<Obstacle> {
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut rand01 = move || -> f32 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
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

fn spawn_point(slot: u32) -> [f32; 2] {
    let angle = slot as f32 * 2.399963; // golden angle: spread out
    [angle.cos() * SPAWN_RING_R, angle.sin() * SPAWN_RING_R]
}

/// Weapon-upgrade pad positions: seeded and shared, like the arena itself.
/// Four pads on a mid ring, nudged outward if a pad lands inside cover.
pub fn generate_pads(seed: u64) -> Vec<[f32; 2]> {
    let obstacles = generate_arena(seed);
    let mut state = seed ^ 0x5bd1_e995_c0ff_ee00;
    let mut rand01 = move || -> f32 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 32) as u32) as f32 / (u32::MAX as f32 + 1.0)
    };
    (0..4)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::FRAC_PI_2
                + rand01() * 0.8
                + std::f32::consts::FRAC_PI_4;
            let mut radius = 9.0 + rand01() * 3.0;
            for _ in 0..6 {
                let p = [angle.cos() * radius, angle.sin() * radius];
                let inside = obstacles.iter().any(|o| {
                    p[0] > o.min[0] - PAD_RADIUS
                        && p[0] < o.max[0] + PAD_RADIUS
                        && p[1] > o.min[1] - PAD_RADIUS
                        && p[1] < o.max[1] + PAD_RADIUS
                });
                if !inside {
                    return p;
                }
                radius += 1.6;
            }
            [angle.cos() * radius, angle.sin() * radius]
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct Pad {
    pub pos: [f32; 2],
    /// 0 = active; counting down after a pickup.
    pub respawn_t: f32,
}

/// Is this spot blocked for a player whose feet are at `y`? The arena wall
/// always blocks; a box only blocks while the player's feet are below its
/// top, so you can walk across boxes you have jumped onto.
fn blocked(pos: [f32; 2], y: f32, r: f32, obstacles: &[Obstacle]) -> bool {
    if pos[0].abs() > ARENA_HALF - r || pos[1].abs() > ARENA_HALF - r {
        return true;
    }
    obstacles
        .iter()
        .any(|o| overlaps(pos, r, o) && y < obstacle_height(o) - STEP_UP)
}

/// Stance-adjusted movement speed. Crouch wins if both are held.
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
pub fn move_circle(
    pos: [f32; 2],
    y: f32,
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
    let pos = if blocked(try_x, y, PLAYER_R, obstacles) {
        pos
    } else {
        try_x
    };
    let try_z = [pos[0], pos[1] + mv[1] * speed * dt];
    if blocked(try_z, y, PLAYER_R, obstacles) {
        pos
    } else {
        try_z
    }
}

/// Vertical motion for one step: gravity, jump take-off, and landing on
/// whatever surface is under the player. Shared VERBATIM by the server sim
/// and the client's prediction, exactly like `move_circle`.
///
/// Returns the new (feet height, vertical speed, grounded).
pub fn step_vertical(
    pos: [f32; 2],
    y: f32,
    vy: f32,
    jump: bool,
    dt: f32,
    obstacles: &[Obstacle],
) -> (f32, f32, bool) {
    let ground = support_height(pos, PLAYER_R, obstacles);
    // Landing check runs against where the feet were, so walking off a box
    // starts a fall instead of snapping to the floor.
    let grounded = y <= ground + 1e-3;
    let mut vy = if grounded && vy <= 0.0 { 0.0 } else { vy };
    if grounded && jump {
        vy = JUMP_VEL;
    }
    vy += GRAVITY * dt;
    let mut y = y + vy * dt;
    let mut landed = false;
    if vy <= 0.0 && y <= ground {
        y = ground;
        vy = 0.0;
        landed = true;
    }
    (y, vy, landed || (grounded && vy <= 0.0))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerIn {
    /// Held movement intent, -1..1 per axis (world space).
    pub mv: [f32; 2],
    /// HORIZONTAL aim direction (normalized by the sim if not). Deliberately
    /// stays 2D and unit-length: see `pitch`.
    pub aim: [f32; 2],
    /// Aim elevation in radians, positive = up. Kept as a SCALAR beside the
    /// horizontal aim rather than folded into a 3D direction, for two
    /// reasons. A 3D-normalized aim would scale the horizontal component by
    /// cos(pitch), shrinking a bullet's TTL-bounded range from ~54 units to
    /// ~6.5 at full elevation; and a near-vertical aim would collapse the
    /// horizontal length below the sanitizer's epsilon below, which is a
    /// no-op that keeps the PREVIOUS aim — freezing the player's facing.
    pub pitch: f32,
    pub fire: bool,
    pub sprint: bool,
    pub crouch: bool,
    /// Held reload intent (R).
    pub reload: bool,
    /// Held jump intent (Space). Only takes effect while grounded.
    pub jump: bool,
    /// How many ticks behind the present this player's view is (derived by
    /// the server from the client's reported view tick, clamped). Bullets
    /// they fire hit-test against targets rewound this far.
    pub delay_ticks: u16,
}

#[derive(Clone, Debug)]
pub struct PlayerSt {
    pub id: u8,
    pub pos: [f32; 2],
    /// Feet height: 0 on the arena floor, a box top when standing on cover.
    pub y: f32,
    /// Vertical speed; non-zero only while airborne.
    pub vy: f32,
    pub aim: [f32; 2],
    /// Aim elevation, radians, positive = up. Broadcast so remote players'
    /// weapons tilt with their actual aim instead of staying level.
    pub pitch: f32,
    pub hp: u8,
    pub score: u32,
    pub alive: bool,
    pub crouch: bool,
    pub weapon: u8,
    pub ammo: u8,
    /// Counting down while reloading; 0 = ready.
    pub reload_t: f32,
    /// Authoritative death count (the scoreboard's DEATHS column).
    pub death_count: u32,
    pub respawn_in: f32,
    pub cooldown: f32,
    deaths: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Bullet {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    /// Height above the arena floor, and its rate of change. Height is a
    /// scalar RIDING ALONGSIDE the 2D path rather than a third component of
    /// it: `vel` keeps its full BULLET_SPEED magnitude at any elevation, so
    /// horizontal range, flight time and every timing-sensitive test behave
    /// exactly as before. The ray's DIRECTION is still exactly the shooter's
    /// look direction, because vy/BULLET_SPEED == tan(pitch).
    pub y: f32,
    pub vy: f32,
    pub ttl: f32,
    pub owner: u8,
    pub dmg: u8,
    /// Owner's view delay when fired: targets rewind this far on hit tests.
    pub delay: u16,
}

/// One tick's snapshot per player (id, pos, feet y, alive, crouch) for
/// lag-compensated rewinds — stance AND height rewind with position, so a
/// shot at a target who was then standing on a crate uses the standing
/// hitbox at the crate's height, even if they have since crouched or
/// dropped off it. Without the height the vertical band below would test a
/// current position against a rewound one and miss for the wrong reason.
type HistoryFrame = Vec<(u8, [f32; 2], f32, bool, bool)>;

pub struct Sim {
    pub obstacles: Vec<Obstacle>,
    pub pads: Vec<Pad>,
    pub players: Vec<PlayerSt>,
    pub bullets: Vec<Bullet>,
    /// (killer, victim) pairs from the last step.
    pub events: Vec<(u8, u8)>,
    /// Ticks stepped since creation; State broadcasts echo it and clients
    /// report it back as their view tick.
    pub tick: u64,
    history: std::collections::VecDeque<HistoryFrame>,
}

impl Sim {
    pub fn new(seed: u64) -> Self {
        Self {
            obstacles: generate_arena(seed),
            pads: generate_pads(seed)
                .into_iter()
                .map(|pos| Pad {
                    pos,
                    respawn_t: 0.0,
                })
                .collect(),
            players: Vec::new(),
            bullets: Vec::new(),
            events: Vec::new(),
            tick: 0,
            history: std::collections::VecDeque::new(),
        }
    }

    pub fn add_player(&mut self, id: u8) {
        let slot = self.players.len() as u32;
        self.players.push(PlayerSt {
            id,
            pos: spawn_point(slot),
            y: 0.0,
            vy: 0.0,
            aim: [1.0, 0.0],
            pitch: 0.0,
            hp: MAX_HP,
            score: 0,
            alive: true,
            crouch: false,
            weapon: 1,
            ammo: weapon_stats(1).mag,
            reload_t: 0.0,
            death_count: 0,
            respawn_in: 0.0,
            cooldown: 0.0,
            deaths: slot,
        });
    }

    pub fn remove_player(&mut self, id: u8) {
        self.players.retain(|p| p.id != id);
        self.bullets.retain(|b| b.owner != id);
        // Purge the id from rewind history: server-side ids are reused, and
        // a joiner must not inherit the leaver's ghost (lag-comp hit tests
        // would land on positions the new player never occupied).
        for frame in self.history.iter_mut() {
            frame.retain(|(pid, _, _, _, _)| *pid != id);
        }
    }

    /// Target state for a hit test — (pos, feet y, alive, crouch) rewound
    /// `delay` ticks for lag compensation (None when history is short or
    /// delay 0).
    fn rewound(&self, id: u8, delay: u16) -> Option<([f32; 2], f32, bool, bool)> {
        if delay == 0 {
            return None;
        }
        let idx = self.history.len().checked_sub(1 + delay as usize)?;
        let frame = self.history.get(idx)?;
        frame
            .iter()
            .find(|(pid, _, _, _, _)| *pid == id)
            .map(|&(_, pos, y, alive, crouch)| (pos, y, alive, crouch))
    }

    pub fn step(&mut self, inputs: &dyn Fn(u8) -> PlayerIn) {
        self.events.clear();
        self.tick += 1;
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
                    p.y = 0.0;
                    p.vy = 0.0;
                    p.hp = MAX_HP;
                    p.alive = true;
                    p.cooldown = 0.3;
                    // Death costs your upgrades.
                    p.weapon = 1;
                    p.ammo = weapon_stats(1).mag;
                    p.reload_t = 0.0;
                }
                continue;
            }

            // Shared movement code (also used by client prediction);
            // stance speed is server-authoritative — no speed cheats.
            let speed = stance_speed(input.sprint, input.crouch);
            let (old_pos, old_y, old_vy) = (
                self.players[i].pos,
                self.players[i].y,
                self.players[i].vy,
            );
            let pos = move_circle(old_pos, old_y, input.mv, speed, dt, &self.obstacles);
            let (y, vy, _grounded) =
                step_vertical(pos, old_y, old_vy, input.jump, dt, &self.obstacles);
            let p = &mut self.players[i];
            p.pos = pos;
            p.y = y;
            p.vy = vy;
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
            // Elevation is sanitized and clamped HERE rather than trusted:
            // the client's own look clamp is cosmetic, and this value now
            // decides where a bullet goes.
            p.pitch = if input.pitch.is_finite() {
                input.pitch.clamp(-MAX_PITCH, MAX_PITCH)
            } else {
                0.0
            };

            // Weapon handling: reload, then fire.
            let stats = weapon_stats(p.weapon);
            p.cooldown = (p.cooldown - dt).max(0.0);
            if p.reload_t > 0.0 {
                p.reload_t -= dt;
                if p.reload_t <= 0.0 {
                    p.reload_t = 0.0;
                    p.ammo = stats.mag;
                }
            } else if (input.reload && p.ammo < stats.mag) || p.ammo == 0 {
                p.reload_t = RELOAD_SECS;
            } else if input.fire && p.cooldown == 0.0 && p.ammo > 0 {
                let owner = p.id;
                let active = self.bullets.iter().filter(|b| b.owner == owner).count()
                    + new_bullets.iter().filter(|b| b.owner == owner).count();
                if active < MAX_BULLETS_PER_PLAYER {
                    let p = &mut self.players[i];
                    p.cooldown = stats.cooldown;
                    p.ammo -= 1;
                    // Spawn just in front of the center: the swept collision
                    // below skips the owner, and starting further out would
                    // leave a point-blank dead zone.
                    let muzzle = [p.pos[0] + p.aim[0] * 0.2, p.pos[1] + p.aim[1] * 0.2];
                    new_bullets.push(Bullet {
                        pos: muzzle,
                        vel: [p.aim[0] * BULLET_SPEED, p.aim[1] * BULLET_SPEED],
                        // Leaves at eye level and climbs or falls at the
                        // tangent of the aim elevation, which makes the
                        // ray exactly the shooter's look direction.
                        y: p.y + eye_h(p.crouch),
                        vy: p.pitch.tan() * BULLET_SPEED,
                        ttl: BULLET_TTL,
                        owner,
                        dmg: stats.damage,
                        delay: input.delay_ticks.min(MAX_REWIND_TICKS),
                    });
                }
            }
        }
        self.bullets.extend(new_bullets);

        // Weapon pads: tick respawns, hand out upgrades on contact.
        for pad in self.pads.iter_mut() {
            if pad.respawn_t > 0.0 {
                pad.respawn_t = (pad.respawn_t - dt).max(0.0);
                continue;
            }
            for p in self.players.iter_mut() {
                if !p.alive || p.weapon >= MAX_WEAPON {
                    continue;
                }
                let (dx, dz) = (p.pos[0] - pad.pos[0], p.pos[1] - pad.pos[1]);
                if dx * dx + dz * dz < PAD_RADIUS * PAD_RADIUS {
                    p.weapon += 1;
                    p.ammo = weapon_stats(p.weapon).mag;
                    p.reload_t = 0.0;
                    pad.respawn_t = PAD_RESPAWN_SECS;
                    break;
                }
            }
        }

        // Record this tick's positions + stance for lag-compensated rewinds.
        self.history.push_back(
            self.players
                .iter()
                .map(|p| (p.id, p.pos, p.y, p.alive, p.crouch))
                .collect(),
        );
        if self.history.len() > HISTORY_LEN {
            self.history.pop_front();
        }

        // Bullets: swept player collision (segment vs circle — no tunneling,
        // no point-blank dead zone) against targets REWOUND by the shooter's
        // view delay, then integrate, then world collision.
        let mut hits: Vec<(u8, u8, u8)> = Vec::new(); // (owner, victim, dmg)
        let obstacles = std::mem::take(&mut self.obstacles);
        let mut bullets = std::mem::take(&mut self.bullets);
        bullets.retain_mut(|b| {
            b.ttl -= dt;
            if b.ttl <= 0.0 {
                return false;
            }
            let p0 = b.pos;
            let p1 = [p0[0] + b.vel[0] * dt, p0[1] + b.vel[1] * dt];
            let y0 = b.y;
            let y1 = b.y + b.vy * dt;
            let (sx, sz) = (p1[0] - p0[0], p1[1] - p0[1]);
            let seg_len_sq = sx * sx + sz * sz;
            for p in self.players.iter() {
                if p.id == b.owner {
                    continue;
                }
                // Where (in what stance, and at what height) the shooter
                // SAW this target.
                let (tpos, ty, talive, tcrouch) = self
                    .rewound(p.id, b.delay)
                    .unwrap_or((p.pos, p.y, p.alive, p.crouch));
                if !talive || !p.alive {
                    continue;
                }
                let rr = hit_radius(tcrouch) + BULLET_R;
                // Closest point on [p0, p1] to the (rewound) center.
                let t = if seg_len_sq <= 1e-8 {
                    0.0
                } else {
                    (((tpos[0] - p0[0]) * sx + (tpos[1] - p0[1]) * sz) / seg_len_sq).clamp(0.0, 1.0)
                };
                let (ex, ez) = (tpos[0] - (p0[0] + sx * t), tpos[1] - (p0[1] + sz * t));
                if ex * ex + ez * ez >= rr * rr {
                    continue;
                }
                // Vertical band, with the bullet's own radius on both ends
                // so it matches the horizontal sum-of-radii test. This is
                // what turns the hit volume from a cylinder of infinite
                // height into a real body, and so what makes pitch matter.
                let (lo, hi) = (ty - BULLET_R, ty + body_h(tcrouch) + BULLET_R);
                let by = b.y + (y1 - b.y) * t;
                let mut connected = by >= lo && by <= hi;
                if !connected && (y1 - b.y).abs() > hi - lo {
                    // The horizontal test is swept but this one is a point
                    // sample at the horizontal closest approach, so the two
                    // are not required to hold at the same instant. Once a
                    // tick's vertical travel exceeds the whole band — past
                    // ~1.31 rad, inside MAX_PITCH — consecutive samples can
                    // straddle a body and the shot passes clean through it.
                    // Walk the segment, both tests at the SAME parameter so
                    // no phantom hit can be introduced.
                    let steps = (((y1 - b.y).abs() / (hi - lo)).ceil() as u32 * 4).min(32);
                    for k in 0..=steps {
                        let u = k as f32 / steps as f32;
                        let byk = b.y + (y1 - b.y) * u;
                        if byk < lo || byk > hi {
                            continue;
                        }
                        let (gx, gz) = (tpos[0] - (p0[0] + sx * u), tpos[1] - (p0[1] + sz * u));
                        if gx * gx + gz * gz < rr * rr {
                            connected = true;
                            break;
                        }
                    }
                }
                if connected {
                    hits.push((b.owner, p.id, b.dmg));
                    return false;
                }
            }
            b.pos = p1;
            b.y = y1;
            // Into the floor.
            if b.y < 0.0 {
                return false;
            }
            if b.pos[0].abs() > ARENA_HALF - BULLET_R || b.pos[1].abs() > ARENA_HALF - BULLET_R {
                return false;
            }
            // Cover now stops only what actually passes THROUGH it, so a
            // shot arcing over a crate from a container top clears it.
            // This pulls obstacle_height's sin() into hit registration;
            // that is sound only because bullets are simulated server-side
            // exclusively — clients never step their own. If client-side
            // shot prediction is ever added, this becomes a desync source.
            // Gated on the tick's vertical SPAN, not its end point: a
            // climbing bullet that enters a crate's footprint below the top
            // and ends the tick above it would otherwise pass straight
            // through the crate's side wall. Conservative — this can only
            // over-block — and a shot arcing down from a container top over
            // a crate still has both endpoints above it.
            if obstacles.iter().any(|o| {
                y0.min(b.y) < obstacle_height(o)
                    && b.pos[0] > o.min[0] - BULLET_R
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
        for (owner, victim, dmg) in hits {
            let Some(v) = self.players.iter_mut().find(|p| p.id == victim) else {
                continue;
            };
            if !v.alive {
                continue;
            }
            v.hp = v.hp.saturating_sub(dmg);
            if v.hp == 0 {
                v.alive = false;
                v.respawn_in = RESPAWN_SECS;
                v.death_count += 1;
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
            inputs.insert(
                0,
                PlayerIn {
                    mv: [1.0, 0.0],
                    aim: [1.0, 0.0],
                    fire: false,
                    sprint,
                    crouch,
                    ..Default::default()
                },
            );
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
        inputs.insert(
            0,
            PlayerIn {
                mv: [0.0, 0.0],
                aim: [1.0, 0.0],
                fire: true,
                ..Default::default()
            },
        );
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
        inputs.insert(
            0,
            PlayerIn {
                mv: [1.0, 0.0],
                aim: [1.0, 0.0],
                fire: false,
                ..Default::default()
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
                ..Default::default()
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
    fn lag_compensation_rewinds_targets() {
        // The target stood at (6, 0) for a while, then dodged to (6, 5).
        // A shooter whose view is 12 ticks behind still sees it at the old
        // spot and fires there: with rewind the shot lands, without it the
        // same shot whiffs.
        let run = |delay: u16| -> bool {
            let mut sim = Sim::new(5);
            sim.obstacles.clear();
            sim.add_player(0); // shooter
            sim.add_player(1); // target
            sim.players[0].pos = [0.0, 0.0];
            sim.players[1].pos = [6.0, 0.0];
            let idle = HashMap::new();
            for _ in 0..15 {
                sim.players[0].pos = [0.0, 0.0];
                sim.players[1].pos = [6.0, 0.0];
                step_with(&mut sim, &idle);
            }
            // The dodge (teleport keeps the geometry exact).
            sim.players.iter_mut().find(|p| p.id == 1).unwrap().pos = [6.0, 5.0];
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    mv: [0.0, 0.0],
                    aim: [1.0, 0.0],
                    fire: true,
                    delay_ticks: delay,
                    ..Default::default()
                },
            );
            // Bullet flight to x=6 takes ~11 ticks — still inside the
            // 12-tick rewind window.
            for _ in 0..12 {
                sim.players[0].pos = [0.0, 0.0];
                sim.players.iter_mut().find(|p| p.id == 1).unwrap().pos = [6.0, 5.0];
                step_with(&mut sim, &inputs);
                if sim.players.iter().find(|p| p.id == 1).unwrap().hp < MAX_HP {
                    return true;
                }
            }
            false
        };
        assert!(
            run(12),
            "rewound shot must land where the shooter saw the target"
        );
        assert!(!run(0), "without rewind the dodged shot must miss");
    }

    #[test]
    fn reload_cycle_and_ammo_gate() {
        let mut sim = Sim::new(7);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.players[0].pos = [0.0, 0.0];
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                mv: [0.0, 0.0],
                aim: [1.0, 0.0],
                fire: true,
                ..Default::default()
            },
        );
        let mag = weapon_stats(1).mag;
        // Hold fire long enough to empty the mag.
        let mut fired = 0u32;
        let mut prev_bullets = 0usize;
        for _ in 0..((weapon_stats(1).cooldown / FIXED_DT) as u32 + 2) * (mag as u32 + 4) {
            step_with(&mut sim, &inputs);
            // Bullets fly off and expire; count spawns via ammo drops.
            let b = sim.bullets.len();
            if b > prev_bullets {
                fired += (b - prev_bullets) as u32;
            }
            prev_bullets = b;
        }
        assert_eq!(
            fired, mag as u32,
            "exactly one magazine before auto-reload gates fire"
        );
        // Let the auto-reload finish: ammo must be full again.
        let idle = HashMap::new();
        for _ in 0..((RELOAD_SECS / FIXED_DT) as u32 + 3) {
            step_with(&mut sim, &idle);
        }
        assert_eq!(sim.players[0].ammo, mag, "reload refills the magazine");
    }

    #[test]
    fn pads_upgrade_and_death_resets() {
        let mut sim = Sim::new(8);
        sim.obstacles.clear();
        sim.add_player(0);
        let pad = sim.pads[0].pos;
        sim.players[0].pos = pad;
        let idle = HashMap::new();
        step_with(&mut sim, &idle);
        let p = &sim.players[0];
        assert_eq!(p.weapon, 2, "standing on a pad upgrades the weapon");
        assert_eq!(p.ammo, weapon_stats(2).mag);
        assert!(sim.pads[0].respawn_t > 0.0, "pad goes on cooldown");

        // Death resets the weapon and counts.
        let p = &mut sim.players[0];
        p.hp = 0;
        p.alive = false;
        p.respawn_in = 0.01;
        p.death_count += 1;
        for _ in 0..3 {
            step_with(&mut sim, &idle);
        }
        let p = &sim.players[0];
        assert!(p.alive);
        assert_eq!(p.weapon, 1, "death resets upgrades");
        assert_eq!(p.death_count, 1);
    }

    #[test]
    fn heavy_kills_in_two_hits() {
        let mut sim = Sim::new(9);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        sim.players[0].weapon = 3;
        sim.players[0].ammo = weapon_stats(3).mag;
        sim.players[0].pos = [0.0, 0.0];
        sim.players[1].pos = [5.0, 0.0];
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                mv: [0.0, 0.0],
                aim: [1.0, 0.0],
                fire: true,
                ..Default::default()
            },
        );
        let mut hits = 0;
        let mut last_hp = MAX_HP;
        for _ in 0..240 {
            sim.players.iter_mut().for_each(|p| {
                if p.id == 1 && p.alive {
                    p.pos = [5.0, 0.0];
                }
            });
            step_with(&mut sim, &inputs);
            if !sim.events.is_empty() {
                break;
            }
            let hp = sim.players.iter().find(|p| p.id == 1).unwrap().hp;
            if hp < last_hp {
                hits += 1;
                last_hp = hp;
            }
        }
        assert_eq!(sim.events, vec![(0, 1)], "heavy weapon killed the target");
        assert!(
            hits <= 1,
            "at most one non-lethal hit before the kill (2 dmg per hit)"
        );
    }

    #[test]
    fn crouch_shrinks_the_hit_circle() {
        // A graze shot aimed just inside the STANDING radius but outside the
        // crouched one: hits a standing target, misses a crouched target.
        let graze_z = hit_radius(false) + BULLET_R - 0.05; // inside standing
        assert!(
            graze_z > hit_radius(true) + BULLET_R,
            "graze must clear the crouched circle"
        );
        let run = |crouch: bool| -> bool {
            let mut sim = Sim::new(11);
            sim.obstacles.clear();
            sim.pads.clear();
            sim.add_player(0);
            sim.add_player(1);
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    aim: [1.0, 0.0],
                    fire: true,
                    ..Default::default()
                },
            );
            inputs.insert(
                1,
                PlayerIn {
                    crouch,
                    ..Default::default()
                },
            );
            for _ in 0..120 {
                sim.players.iter_mut().for_each(|p| match p.id {
                    0 if p.alive => p.pos = [0.0, 0.0],
                    1 if p.alive => p.pos = [5.0, graze_z],
                    _ => {}
                });
                step_with(&mut sim, &inputs);
                if sim.players.iter().find(|p| p.id == 1).unwrap().hp < MAX_HP {
                    return true;
                }
            }
            false
        };
        assert!(run(false), "graze shot must hit a standing target");
        assert!(!run(true), "the same shot must miss a crouched target");
    }

    #[test]
    fn rewind_uses_historical_stance() {
        // Target stood (large circle) when the lagged shooter fired, then
        // crouched. The rewound hit test must use the standing hitbox.
        let graze_z = hit_radius(false) + BULLET_R - 0.05;
        let mut sim = Sim::new(12);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        let idle = HashMap::new();
        // History fills while the target STANDS at the graze offset.
        for _ in 0..15 {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 => p.pos = [0.0, 0.0],
                1 => p.pos = [5.0, graze_z],
                _ => {}
            });
            step_with(&mut sim, &idle);
        }
        // Now the target crouches; a 12-tick-lagged shot sees them standing.
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                fire: true,
                delay_ticks: 12,
                ..Default::default()
            },
        );
        inputs.insert(
            1,
            PlayerIn {
                crouch: true,
                ..Default::default()
            },
        );
        let mut hit = false;
        for _ in 0..12 {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 => p.pos = [0.0, 0.0],
                1 if p.alive => p.pos = [5.0, graze_z],
                _ => {}
            });
            step_with(&mut sim, &inputs);
            if sim.players.iter().find(|p| p.id == 1).unwrap().hp < MAX_HP {
                hit = true;
                break;
            }
        }
        assert!(
            hit,
            "rewound shot must use the stance the shooter saw (standing)"
        );
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
                ..Default::default()
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

    /// One waist-high crate at the origin, and nothing else.
    fn one_box() -> Vec<Obstacle> {
        vec![Obstacle {
            min: [-1.5, -1.5],
            max: [1.5, 1.5],
        }]
    }

    #[test]
    fn jump_rises_then_lands_back_on_the_floor() {
        let obs = Vec::new();
        let (mut y, mut vy) = (0.0f32, 0.0f32);
        let (mut peak, mut airborne_ticks) = (0.0f32, 0);
        // One tap: jump only on the first tick.
        for tick in 0..180 {
            let (ny, nvy, _) = step_vertical([0.0, 0.0], y, vy, tick == 0, FIXED_DT, &obs);
            y = ny;
            vy = nvy;
            peak = peak.max(y);
            if y > 1e-3 {
                airborne_ticks += 1;
            }
        }
        assert!(peak > 1.0 && peak < 2.0, "jump peak {peak}");
        assert!(airborne_ticks > 20, "hang time {airborne_ticks} ticks");
        assert!(y.abs() < 1e-3, "did not land: {y}");
    }

    #[test]
    fn a_box_blocks_on_the_ground_but_not_from_above() {
        let obs = one_box();
        let top = obstacle_height(&obs[0]);
        // Walking into the crate from outside gets stopped.
        let walked = move_circle([-3.0, 0.0], 0.0, [1.0, 0.0], MOVE_SPEED, 0.5, &obs);
        assert!(walked[0] < -1.5 - PLAYER_R + 0.01, "walked into box: {walked:?}");
        // The same move with the feet above the crate's top goes through.
        let over = move_circle([-3.0, 0.0], top + 0.1, [1.0, 0.0], MOVE_SPEED, 0.5, &obs);
        assert!(over[0] > -1.0, "could not walk over the box: {over:?}");
    }

    #[test]
    fn landing_on_a_box_top_supports_the_player() {
        let obs = one_box();
        let top = obstacle_height(&obs[0]);
        // Falling from above the crate lands on its top, not the floor.
        let (mut y, mut vy) = (top + 2.0, 0.0);
        for _ in 0..240 {
            let (ny, nvy, _) = step_vertical([0.0, 0.0], y, vy, false, FIXED_DT, &obs);
            y = ny;
            vy = nvy;
        }
        assert!((y - top).abs() < 1e-3, "settled at {y}, box top {top}");
        // Stepping off the edge starts a fall back to the floor.
        let (mut y, mut vy) = (top, 0.0);
        for _ in 0..240 {
            let (ny, nvy, _) = step_vertical([9.0, 9.0], y, vy, false, FIXED_DT, &obs);
            y = ny;
            vy = nvy;
        }
        assert!(y.abs() < 1e-3, "did not fall off the box: {y}");
    }

    #[test]
    fn a_jump_clears_crates_but_not_containers() {
        // apex = v^2 / 2g must clear every crate and no container, or the
        // two cover classes stop meaning anything.
        let apex = JUMP_VEL * JUMP_VEL / (2.0 * -GRAVITY);
        assert!(apex > CRATE_MAX_H, "apex {apex} cannot clear a crate");
        assert!(apex < CONTAINER_MIN_H, "apex {apex} clears containers too");
        // And the generator must actually produce both classes.
        let obs = generate_arena(20260829);
        let heights: Vec<f32> = obs.iter().map(obstacle_height).collect();
        assert!(heights.iter().any(|h| *h <= CRATE_MAX_H), "no crates: {heights:?}");
        assert!(heights.iter().any(|h| *h >= CONTAINER_MIN_H), "no containers");
        for h in heights {
            assert!(
                (CRATE_MIN_H..=CRATE_MAX_H).contains(&h) || h >= CONTAINER_MIN_H,
                "height {h} falls between the two classes"
            );
        }
    }

    #[test]
    fn the_sim_keeps_players_on_the_ground_until_they_jump() {
        let mut sim = Sim::new(7);
        sim.add_player(0);
        let mut inputs = HashMap::new();
        inputs.insert(0, PlayerIn::default());
        for _ in 0..30 {
            step_with(&mut sim, &inputs);
        }
        assert_eq!(sim.players[0].y, 0.0);
        inputs.insert(
            0,
            PlayerIn {
                jump: true,
                ..Default::default()
            },
        );
        step_with(&mut sim, &inputs);
        step_with(&mut sim, &inputs);
        assert!(sim.players[0].y > 0.1, "jump did not lift: {}", sim.players[0].y);
    }

    /// Fires from `shoot_y` at a target on the floor `dist` away, holding
    /// both players in place so only the elevation under test varies.
    /// Returns whether the target was ever hit.
    fn shot_connects(shoot_y: f32, dist: f32, pitch: f32) -> bool {
        let mut sim = Sim::new(21);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                pitch,
                fire: true,
                ..Default::default()
            },
        );
        inputs.insert(1, PlayerIn::default());
        for _ in 0..120 {
            // Held every tick: gravity would otherwise pull the shooter off
            // its perch and quietly change the elevation under test.
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => {
                    p.pos = [0.0, 0.0];
                    p.y = shoot_y;
                    p.vy = 0.0;
                }
                1 if p.alive => {
                    p.pos = [dist, 0.0];
                    p.y = 0.0;
                    p.vy = 0.0;
                }
                _ => {}
            });
            step_with(&mut sim, &inputs);
            if sim.players.iter().find(|p| p.id == 1).unwrap().hp < MAX_HP {
                return true;
            }
        }
        false
    }

    #[test]
    fn a_shot_from_above_needs_pitch_to_connect() {
        // THE regression this whole vertical-shot change exists for: from a
        // container top, level fire sails over a target on the floor, and
        // the same shot aimed down at them lands. Before bullets had a
        // height, both of these hit — elevation simply did not exist.
        let shoot_y = 2.4;
        let dist = 6.0;
        assert!(
            !shot_connects(shoot_y, dist, 0.0),
            "level fire from {shoot_y} up must sail over a target on the floor"
        );
        // Aim at chest height: drop 3.0 over 6.0 of run.
        let aimed = -(3.0f32 / dist).atan();
        assert!(
            shot_connects(shoot_y, dist, aimed),
            "the same shot aimed down at the target must connect"
        );
    }

    #[test]
    fn pitch_does_not_shorten_a_shot() {
        // The 3D-normalize trap: scaling the horizontal aim by cos(pitch)
        // would collapse the TTL-bounded range. Horizontal speed must stay
        // BULLET_SPEED at every elevation, so a steeply-aimed shot still
        // reaches as far downrange as a level one.
        // Inside the arena wall, not just inside the TTL range: bullets are
        // culled at ARENA_HALF - BULLET_R, so a target parked beyond that is
        // unreachable no matter how far the round could otherwise fly.
        let far = ARENA_HALF - 4.0;
        assert!(
            shot_connects(0.0, far, 0.0),
            "level fire must reach {far} units"
        );
        // Same distance, fired from high up and angled down to arrive.
        let shoot_y = 8.0;
        let aimed = -((shoot_y + EYE_STAND - 0.85) / far).atan();
        assert!(
            shot_connects(shoot_y, far, aimed),
            "a steeply-angled shot must reach the same distance"
        );

        // The two range checks above are necessary but NOT sufficient: at
        // these angles a cos(pitch)-scaled implementation would still have
        // enough range to pass them. Pin the property itself — horizontal
        // speed is BULLET_SPEED at any elevation — on the spawned bullet,
        // where a 3D normalize would be caught immediately.
        for &pitch in &[0.0, 0.4, -0.9, MAX_PITCH, -MAX_PITCH] {
            let mut sim = Sim::new(24);
            sim.obstacles.clear();
            sim.pads.clear();
            sim.add_player(0);
            // Headroom, so this test measures what it claims to. At the
            // clamp a round descends tan(1.45) * BULLET_SPEED * dt ~= 4.67
            // units in its FIRST tick, and bullets are extended into the
            // list before that same tick's sweep runs — so a straight-down
            // shot fired from ground level is culled by the floor before it
            // can be inspected at all. That culling is correct, and is
            // asserted on its own in the test below; it is just not the
            // property being pinned here.
            sim.players[0].y = 8.0;
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    aim: [1.0, 0.0],
                    pitch,
                    fire: true,
                    ..Default::default()
                },
            );
            step_with(&mut sim, &inputs);
            let b = sim.bullets.first().expect("a shot must spawn a bullet");
            let h_speed = (b.vel[0] * b.vel[0] + b.vel[1] * b.vel[1]).sqrt();
            assert!(
                (h_speed - BULLET_SPEED).abs() < 1e-3,
                "horizontal speed must stay BULLET_SPEED at pitch {pitch}, got {h_speed}"
            );
            // And the ray must point exactly where the player looked.
            assert!(
                (b.vy / h_speed - pitch.tan()).abs() < 1e-3,
                "the shot's slope must equal tan(pitch) at pitch {pitch}"
            );
            assert!(b.vy.is_finite(), "vertical speed must stay finite");
        }
    }

    #[test]
    fn cover_stops_only_what_passes_through_it() {
        // Height-aware cover is the other half of giving bullets a height:
        // without it, a shot fired over a container from above would still
        // be eaten by the container's floor plan.
        // Scan for a placement the generator makes a CONTAINER (>= 2.4);
        // deterministic, unlike hardcoding an input to obstacle_height's hash.
        let container = (0..400)
            .map(|i| {
                let x = 3.0 + i as f32 * 0.013;
                Obstacle {
                    min: [x, -2.0],
                    max: [x + 1.0, 2.0],
                }
            })
            .find(|o| obstacle_height(o) >= CONTAINER_MIN_H)
            .expect("some placement must yield a container");
        let h = obstacle_height(&container);

        let run = |shoot_y: f32| -> bool {
            let mut sim = Sim::new(22);
            sim.obstacles.clear();
            sim.pads.clear();
            sim.obstacles.push(container);
            sim.add_player(0);
            sim.add_player(1);
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    aim: [1.0, 0.0],
                    fire: true,
                    ..Default::default()
                },
            );
            inputs.insert(1, PlayerIn::default());
            for _ in 0..120 {
                sim.players.iter_mut().for_each(|p| match p.id {
                    0 if p.alive => {
                        p.pos = [0.0, 0.0];
                        p.y = shoot_y;
                        p.vy = 0.0;
                    }
                    1 if p.alive => {
                        p.pos = [9.0, 0.0];
                        p.y = shoot_y;
                        p.vy = 0.0;
                    }
                    _ => {}
                });
                step_with(&mut sim, &inputs);
                if sim.players.iter().find(|p| p.id == 1).unwrap().hp < MAX_HP {
                    return true;
                }
            }
            false
        };

        assert!(
            !run(0.0),
            "a level shot from the floor must be stopped by a {h}-tall container"
        );
        assert!(
            run(h),
            "the same shot fired from the container's own top must clear it"
        );
    }

    #[test]
    fn pitch_is_clamped_and_nan_safe() {
        // The client's look clamp is cosmetic; a hostile peer can send
        // anything, and this value decides where a bullet goes.
        let mut sim = Sim::new(23);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                pitch: f32::NAN,
                ..Default::default()
            },
        );
        step_with(&mut sim, &inputs);
        assert_eq!(sim.players[0].pitch, 0.0, "NaN pitch must fall back to level");

        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                pitch: 99.0,
                ..Default::default()
            },
        );
        step_with(&mut sim, &inputs);
        assert!(
            (sim.players[0].pitch - MAX_PITCH).abs() < 1e-6,
            "out-of-range pitch must clamp to MAX_PITCH, got {}",
            sim.players[0].pitch
        );
        // The clamp is what keeps tan() finite: an unclamped pi/2 would make
        // the vertical speed infinite and the bullet's height NaN.
        assert!(sim.players[0].pitch.tan().is_finite());
    }

    #[test]
    fn a_shot_into_the_ground_is_culled_by_the_floor() {
        // The behaviour that broke the test above, pinned in its own right:
        // firing at your own feet must stop at the floor, not leave a round
        // skimming along underneath the arena hitting people from below.
        // The spawn happens before the same tick's sweep, so at the clamp
        // this is culled within one tick of being fired.
        let mut sim = Sim::new(25);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                pitch: -MAX_PITCH,
                fire: true,
                ..Default::default()
            },
        );
        step_with(&mut sim, &inputs);
        assert!(
            sim.bullets.is_empty(),
            "a straight-down shot must be stopped by the floor, not fly under the arena"
        );

        // Negative control: the same shot from high enough up survives the
        // tick, so the assertion above is about the floor and not about
        // steep shots failing to spawn at all.
        let mut sim = Sim::new(25);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.players[0].y = 8.0;
        step_with(&mut sim, &inputs);
        assert_eq!(
            sim.bullets.len(),
            1,
            "the same steep shot with headroom below must still be in flight"
        );
    }
}
