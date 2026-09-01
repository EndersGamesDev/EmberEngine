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
/// Crouching shrinks the HIT circle (movement blocking keeps `PLAYER_R`).
pub const CROUCH_HIT_MULT: f32 = 0.72;

/// The arc the raised off-hand shield covers, radians: 120° centred on the
/// holder's horizontal aim, so it protects what you are looking at and
/// nothing else.
///
/// Widening this toward TAU makes the shield omnidirectional and removes the
/// only way to beat it (flank it), so it is the tuning knob that matters most.
pub const SHIELD_ARC: f32 = std::f32::consts::FRAC_PI_3 * 2.0;

/// Height of the head zone, measured DOWN from the top of the hit volume.
/// `BODY_H_*` is documented above as "head centre plus half a head", so the
/// top of the cylinder already IS the top of the head: the head is a sub-band
/// just under it, never something added above. 0.22 puts a standing head at
/// [1.48, 1.70] and a crouched one at [1.03, 1.25].
///
/// The value is not cosmetic and MUST stay under
/// BODY_H_STAND - EYE_STAND = 0.25. A round leaves the muzzle at EYE_STAND
/// 1.45 and flies level at pitch 0, so the moment the head band reaches down
/// to 1.45, every level shot between two standing players is a headshot - and
/// since a headshot kills outright, the pistol would one-shot the entire game
/// without anyone aiming at a head. At 0.22 the band starts at 1.48, so level
/// fire lands in the chest and a kill needs deliberate upward aim. The test
/// `level_fire_is_not_a_free_headshot` pins that 3 cm and will fail loudly if
/// this is ever raised past it.
pub const HEAD_H: f32 = 0.22;

/// Melee reach from the attacker's centre, before the target's own radius is
/// added. 2.0 + PLAYER_R 0.6 strikes a standing target at 2.6 centre to
/// centre, about one and a half body widths - a lunge, not a spear.
pub const MELEE_RANGE: f32 = 2.0;
/// Full width of the melee cone, radians (~115 deg). Wider than SHIELD_ARC on
/// purpose: the shield answers "am I covered from that round", which wants to
/// be demanding, while a swing at contact range wants to land when it
/// visually should.
pub const MELEE_ARC: f32 = 2.0;
/// Seconds between swings. Every connect is a kill, so this is the only thing
/// stopping melee from out-killing the pistol at range zero by spamming.
pub const MELEE_COOLDOWN: f32 = 0.8;

/// Stance-dependent hit-test radius: crouch = lower profile = smaller target.
#[must_use]
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
/// Body height above the feet, per stance.
///
/// The vertical extent matches the silhouette the client draws (head centre
/// plus half a head) so that what you see is what you hit. These live here, not in
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
#[must_use]
pub const fn eye_h(crouch: bool) -> f32 {
    if crouch { EYE_CROUCH } else { EYE_STAND }
}

/// Vertical extent of the hit volume, measured from the target's feet.
///
/// Together with `hit_radius` this makes the hitbox a finite cylinder; it
/// used to be one of infinite height, which is why pitch never mattered.
#[must_use]
pub const fn body_h(crouch: bool) -> f32 {
    if crouch { BODY_H_CROUCH } else { BODY_H_STAND }
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

#[must_use]
pub const fn weapon_stats(level: u8) -> WeaponStats {
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

#[must_use]
pub const fn weapon_name(level: u8) -> &'static str {
    match level {
        3 => "Heavy",
        2 => "Rapid",
        _ => "Pistol",
    }
}
/// Active bullets per player, so holding fire can't flood the state.
///
/// Counted by owner, and a reflected round changes owner — so rounds you
/// caught on the shield sit against your own cap until they expire. Left
/// that way deliberately: you cannot fire behind a raised shield anyway, no
/// round lives longer than `BULLET_TTL`, and being briefly short of the cap
/// after catching ten rounds in 1.6 s is a fair price for having caught
/// them. Telling the two apart would need a flag on `Bullet`.
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

/// Axis-aligned obstacle on the XZ plane, with a top.
///
/// `h` used to be derived on demand by hashing `min` — which meant a box
/// could not *state* how tall it was, and an authored one had no way into
/// the sim. It is now carried. Seeded arenas fill it with exactly the old
/// hash, so nothing observable changed when it moved.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Obstacle {
    pub min: [f32; 2],
    pub max: [f32; 2],
    /// Top of the box; the floor is 0.
    pub h: f32,
}

impl Obstacle {
    /// A box at seeded-arena proportions: the height the generator would
    /// have derived for this footprint.
    #[must_use]
    pub fn seeded(min: [f32; 2], max: [f32; 2]) -> Self {
        Self {
            min,
            max,
            h: seeded_height(min),
        }
    }
}

/// Height a seeded arena gives a box at this corner.
///
/// Roughly three in five are crates low enough to jump onto and fight from;
/// the rest are shipping containers that stay hard cover. Kept as the
/// generator's own rule rather than a property of every obstacle: an
/// authored box sets `h` directly and never consults this.
#[must_use]
pub fn seeded_height(min: [f32; 2]) -> f32 {
    let k = (min[0] * 12.9898 + min[1] * 78.233).sin() * 43758.547;
    let f = k - k.floor();
    if f < 0.6 {
        CRATE_MIN_H + (f / 0.6) * (CRATE_MAX_H - CRATE_MIN_H)
    } else {
        CONTAINER_MIN_H + ((f - 0.6) / 0.4) * 0.8
    }
}

/// Height of a cover box. Now simply what the box says it is — kept as a
/// function because the sim, the renderer and the editor all read heights
/// through it.
#[must_use]
pub const fn obstacle_height(o: &Obstacle) -> f32 {
    o.h
}

/// The surface a player at `pos` stands on: its tallest overlapping box or the floor.
///
/// This uses the same overlap test as `blocked`, so support always comes from
/// a box the player is actually on.
#[must_use]
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
        obstacles.push(Obstacle::seeded([cx - hx, cz - hz], [cx + hx, cz + hz]));
    }
    obstacles
}

// This cast is part of the deterministic spawn formula's expression tree.
#[allow(clippy::cast_precision_loss)]
fn spawn_point(slot: u32) -> [f32; 2] {
    let angle = slot as f32 * 2.399_963; // golden angle: spread out
    [angle.cos() * SPAWN_RING_R, angle.sin() * SPAWN_RING_R]
}

/// A whole arena as data: what the editor authors, and what the sim will
/// eventually be handed instead of a seed.
///
/// Today every peer regenerates the world from the lobby seed and this
/// type is only ever *produced* — `Sim` still takes a seed and nothing
/// reads a `Level` off the wire. That is deliberate: the format lands, is
/// proved identical to the generator, and the protocol change that
/// delivers it is a separate step behind a server redeploy.
///
/// Spawns are carried rather than derived because an authored arena
/// decides where players start; a seeded one reproduces the golden-angle
/// ring the sim has always used.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Level {
    pub arena_half: f32,
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<[f32; 2]>,
}

impl Level {
    /// The arena a seed has always produced — same obstacles, same
    /// heights, same spawn ring.
    #[must_use]
    // `MAX_PLAYERS` is eight, so this conversion cannot truncate in supported builds.
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_seed(seed: u64) -> Self {
        Self {
            arena_half: ARENA_HALF,
            obstacles: generate_arena(seed),
            spawns: (0..MAX_PLAYERS as u32).map(spawn_point).collect(),
        }
    }

    /// Where player `slot` starts, wrapping if an authored level supplies
    /// fewer spawns than players. Falls back to the seeded ring when a
    /// level carries none, so a half-authored level still runs.
    #[must_use]
    pub fn spawn(&self, slot: u32) -> [f32; 2] {
        if self.spawns.is_empty() {
            return spawn_point(slot);
        }
        self.spawns[slot as usize % self.spawns.len()]
    }
}

/// Weapon-upgrade pad positions: seeded and shared, like the arena itself.
/// Four pads on a mid ring, nudged outward if a pad lands inside cover.
// These integer-to-float casts are part of the deterministic pad formula's expression tree.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn generate_pads(seed: u64) -> Vec<[f32; 2]> {
    let obstacles = generate_arena(seed);
    let mut state = seed ^ 0x5bd1_e995_c0ff_ee00;
    let mut rand01 = move || -> f32 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 32) as u32) as f32 / (u32::MAX as f32 + 1.0)
    };
    (0_u8..4)
        .map(|i| {
            let angle = f32::from(i) * std::f32::consts::FRAC_PI_2
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

/// Stance-adjusted movement speed. Crouch wins if both are held, and a
/// raised shield cancels sprint — you advance behind it, you do not charge
/// behind it.
///
/// `shield` is a parameter rather than something callers fold into `sprint`
/// themselves because this function is shared VERBATIM by the server sim and
/// the client's prediction: a rule applied at one call site and not the
/// other is a desync, and a signature change is the only way to make the
/// compiler say so.
#[must_use]
pub fn stance_speed(sprint: bool, crouch: bool, shield: bool) -> f32 {
    MOVE_SPEED
        * if crouch {
            CROUCH_MULT
        } else if sprint && !shield {
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
#[must_use]
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
    let mut vy = if grounded && jump {
        JUMP_VEL
    } else if grounded && vy <= 0.0 {
        0.0
    } else {
        vy
    };
    vy += GRAVITY * dt;
    let y = y + vy * dt;
    let (y, vy, landed) = if vy <= 0.0 && y <= ground {
        (ground, 0.0, true)
    } else {
        (y, vy, false)
    };
    (y, vy, landed || (grounded && vy <= 0.0))
}

// These independent input flags are shared public simulation API and cannot be consolidated.
#[allow(clippy::struct_excessive_bools)]
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
    /// A jump PRESS, consumed on the tick it is applied (pong-server clears
    /// it after each step). Only takes effect while grounded.
    pub jump: bool,
    /// Held off-hand shield intent (Q). HELD, like every other intent here
    /// and unlike a toggle: a toggle keeps a bit of state on each side of the
    /// wire that a dropped packet can desync, and nothing in this struct has
    /// ever needed one.
    pub shield: bool,
    /// A melee PRESS (E), consumed on the tick it lands, exactly like `jump`
    /// and for the same reason: held semantics would re-swing on every tick
    /// the server keeps applying the last input it received, and at one kill
    /// per connect that is not a weapon, it is a proximity field.
    pub melee: bool,
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
    /// Off-hand shield raised. Broadcast, because a shield nobody can see is
    /// a mechanic that kills you for no visible reason.
    pub shield: bool,
    pub weapon: u8,
    pub ammo: u8,
    /// Counting down while reloading; 0 = ready.
    pub reload_t: f32,
    /// Authoritative death count (the scoreboard's DEATHS column).
    pub death_count: u32,
    pub respawn_in: f32,
    pub cooldown: f32,
    /// Counting down between melee swings; 0 = ready.
    pub melee_cd: f32,
    deaths: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Bullet {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    /// Height above the arena floor, and its rate of change. Height is a
    /// scalar RIDING ALONGSIDE the 2D path rather than a third component of
    /// it: `vel` keeps its full `BULLET_SPEED` magnitude at any elevation, so
    /// horizontal range, flight time and every timing-sensitive test behave
    /// exactly as before. The ray's DIRECTION is still exactly the shooter's
    /// look direction, because `vy / BULLET_SPEED == tan(pitch)`.
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
    #[must_use]
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
        // Player count is capped at eight by the public protocol.
        #[allow(clippy::cast_possible_truncation)]
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
            shield: false,
            weapon: 1,
            ammo: weapon_stats(1).mag,
            reload_t: 0.0,
            death_count: 0,
            respawn_in: 0.0,
            cooldown: 0.0,
            melee_cd: 0.0,
            deaths: slot,
        });
    }

    pub fn remove_player(&mut self, id: u8) {
        self.players.retain(|p| p.id != id);
        self.bullets.retain(|b| b.owner != id);
        // Purge the id from rewind history: server-side ids are reused, and
        // a joiner must not inherit the leaver's ghost (lag-comp hit tests
        // would land on positions the new player never occupied).
        for frame in &mut self.history {
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

    // Splitting or rewriting the simulation loop or its casts could alter deterministic ordering.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
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
                // A corpse carries no shield. Without this a player who died
                // with Q held keeps a raised shield in every broadcast until
                // their first live tick after respawning, and remote clients
                // draw it.
                p.shield = false;
                p.respawn_in -= dt;
                if p.respawn_in <= 0.0 {
                    p.deaths = p.deaths.wrapping_add(1);
                    let point = spawn_point(p.deaths.wrapping_mul(3).wrapping_add(u32::from(p.id)));
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
            let speed = stance_speed(input.sprint, input.crouch, input.shield);
            let (old_pos, feet_height, vertical_speed) =
                (self.players[i].pos, self.players[i].y, self.players[i].vy);
            let pos = move_circle(old_pos, feet_height, input.mv, speed, dt, &self.obstacles);
            let (y, vy, _grounded) = step_vertical(
                pos,
                feet_height,
                vertical_speed,
                input.jump,
                dt,
                &self.obstacles,
            );
            let p = &mut self.players[i];
            p.pos = pos;
            p.y = y;
            p.vy = vy;
            p.crouch = input.crouch;
            p.shield = input.shield;

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
            p.melee_cd = (p.melee_cd - dt).max(0.0);
            if p.reload_t > 0.0 {
                p.reload_t -= dt;
                if p.reload_t <= 0.0 {
                    p.reload_t = 0.0;
                    p.ammo = stats.mag;
                }
            } else if (input.reload && p.ammo < stats.mag) || p.ammo == 0 {
                p.reload_t = RELOAD_SECS;
            } else if input.fire && !input.shield && p.cooldown == 0.0 && p.ammo > 0 {
                // Raising the shield blocks your own trigger — the price of
                // cover, and the reason the whole match does not degenerate
                // into everyone holding Q. Only the trigger is blocked: the
                // cooldown above still runs down behind the shield, so
                // releasing Q fires on the next tick exactly as if the
                // trigger had simply not been pulled, which is how every
                // other held intent here behaves.
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
        for pad in &mut self.pads {
            if pad.respawn_t > 0.0 {
                pad.respawn_t = (pad.respawn_t - dt).max(0.0);
                continue;
            }
            for p in &mut self.players {
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

        // ---- the melee strike (E) ----
        //
        // Resolved HERE, as its own pass, and deliberately never as a bullet.
        // That is the entire reason it goes through the shield: the shield
        // lives inside the bullet sweep below, gated on `if p.shield`, so
        // something that never becomes a bullet never reaches it. Expressing
        // that as structure rather than as an `if melee { skip_shield }` flag
        // means a later change to the shield cannot quietly start blocking
        // melee, and the shield's own tests keep their meaning untouched.
        //
        // Lethal on connect, so there is no damage number to balance. It goes
        // through the same `hits` vec as a round, which is what keeps scoring,
        // respawn, the death count and the kill event identical to every other
        // way of dying, rather than a second path that drifts from the first.
        //
        // Targets are rewound by the attacker's view delay for exactly the
        // reason bullets are: "where was the body" is the attacker's question.
        // The shield's counter-question, "am I blocking right now", does not
        // arise here, because there is no shield test left to answer it.
        for i in 0..self.players.len() {
            let (aid, apos, ay, aim, alive) = {
                let a = &self.players[i];
                (a.id, a.pos, a.y, a.aim, a.alive)
            };
            if !alive || self.players[i].melee_cd > 0.0 {
                continue;
            }
            let input = inputs(aid);
            if !input.melee {
                continue;
            }
            // Charged whether or not the swing connects: a miss costs the same
            // tempo as a hit, which is what makes spacing matter.
            self.players[i].melee_cd = MELEE_COOLDOWN;
            let delay = input.delay_ticks.min(MAX_REWIND_TICKS);
            let a_top = ay + BODY_H_STAND;
            for j in 0..self.players.len() {
                if i == j {
                    continue;
                }
                let tid = self.players[j].id;
                let (tpos, ty, talive, tcrouch) = self.rewound(tid, delay).unwrap_or_else(|| {
                    let t = &self.players[j];
                    (t.pos, t.y, t.alive, t.crouch)
                });
                if !talive {
                    continue;
                }
                // Reach is centre to centre less the target's own radius, so a
                // crouched (narrower) target must be closed on slightly
                // further - consistent with it being a smaller target to shoot.
                let (dx, dz) = (tpos[0] - apos[0], tpos[1] - apos[1]);
                let d2 = dx * dx + dz * dz;
                let reach = MELEE_RANGE + hit_radius(tcrouch);
                if d2 > reach * reach {
                    continue;
                }
                // Facing, on the HORIZONTAL aim only - the same choice the
                // shield makes, so look elevation never widens or narrows a
                // swing.
                let d = d2.sqrt();
                if d > 1e-4 {
                    let dot = (dx * aim[0] + dz * aim[1]) / d;
                    if dot < (MELEE_ARC * 0.5).cos() {
                        continue;
                    }
                }
                // The bodies must overlap vertically, so you cannot knife
                // someone standing on a container over your head.
                if ty > a_top || ty + body_h(tcrouch) < ay {
                    continue;
                }
                hits.push((aid, tid, MAX_HP));
            }
        }
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
            for p in &self.players {
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
                // The head is the top HEAD_H of the volume. No BULLET_R pad on
                // its underside: that boundary is internal, between head and
                // chest, not a silhouette edge, and padding it outward would
                // quietly make the head bigger than the one being drawn.
                let head_lo = ty + body_h(tcrouch) - HEAD_H;
                let travel = (y1 - b.y).abs();
                let by = b.y + (y1 - b.y) * t;
                let mut connected = by >= lo && by <= hi;
                let mut head = connected && by >= head_lo;
                // Walk the segment when one tick's vertical travel could
                // straddle the SMALLEST zone under test. This used to key on
                // the body band, and for the body that is right - a tick
                // cannot skip 2.14 m below ~1.31 rad. A head band is about
                // seven times smaller and the arithmetic is unforgiving:
                // vertical travel is tan(pitch) * BULLET_SPEED * dt, which is
                // 0.567 * tan(pitch), so a 0.30 m head is straddled from just
                // 0.49 rad (~28 deg) - ordinary aiming, not a trick shot.
                // Keyed on the body band a headshot between 28 and 75 degrees
                // would have been a coin flip, which is not worth shipping.
                //
                // Both tests are still evaluated at the SAME parameter, so
                // this can only find hits that are real; it cannot invent one.
                if travel > HEAD_H && !head {
                    let steps = ((travel / HEAD_H).ceil() as u32 * 4).clamp(1, 32);
                    for k in 0..=steps {
                        let u = k as f32 / steps as f32;
                        let byk = b.y + (y1 - b.y) * u;
                        if byk < lo || byk > hi {
                            continue;
                        }
                        let (gx, gz) = (tpos[0] - (p0[0] + sx * u), tpos[1] - (p0[1] + sz * u));
                        if gx * gx + gz * gz < rr * rr {
                            connected = true;
                            // Keep walking only to upgrade a body hit to a head
                            // hit: a round that passed through the head IS a
                            // headshot, even when the closest-approach sample
                            // happened to land in the chest.
                            if byk >= head_lo {
                                head = true;
                                break;
                            }
                        }
                    }
                }
                if connected {
                    // ---- the off-hand shield ----
                    //
                    // Placed exactly where the damage decision was, so a
                    // reflected round is precisely one that WOULD have hit.
                    // The rest of the sweep's order is deliberate around it:
                    // TTL was already charged above, so a shield never
                    // extends a round's life; and the floor/wall/cover checks
                    // below are skipped on the reflect tick because the round
                    // does not advance — it departs next tick from a position
                    // that already passed them.
                    //
                    // The shield is judged in the PRESENT — this tick's flag
                    // and this tick's facing — while the body test above
                    // stays lag-compensated. That is deliberately unlike
                    // crouch, which rewinds with position: crouch answers
                    // "where was the body", which is the shooter's question,
                    // and the shield answers "is the defender blocking right
                    // now", which is the defender's. Rewinding the flag
                    // without also rewinding the facing it points along
                    // would answer neither.
                    if p.shield {
                        let n = p.aim; // unit: the sim normalizes it on input
                        let dot = b.vel[0] * n[0] + b.vel[1] * n[1];
                        let speed_h = (b.vel[0] * b.vel[0] + b.vel[1] * b.vel[1]).sqrt();
                        // Inside the cover arc iff the round is travelling
                        // into the plate's face — its heading within half the
                        // arc of -n. Tested on the heading rather than on the
                        // bearing from holder to round, because the point of
                        // contact sits by construction perpendicular to the
                        // flight line and a bearing taken there is noise. For
                        // anything but a point-blank shot the two agree
                        // anyway: a round that connects with a 0.82-radius
                        // body from 5 units out arrives within ~9° of
                        // straight-on. cos() is a transcendental in hit
                        // registration, sound for exactly the reason
                        // obstacle_height's sin() is — bullets are stepped
                        // server-side only.
                        if speed_h > 1e-6 && -dot >= (SHIELD_ARC * 0.5).cos() * speed_h {
                            // Mirror about the plate: v' = v - 2(v·n)n. The
                            // normal is horizontal, so vy is untouched and a
                            // round arcing down at you comes back arcing
                            // down, keeping its range rather than being
                            // launched at the sky. A mirror is an isometry,
                            // so horizontal speed stays BULLET_SPEED and the
                            // invariant pinned by pitch_does_not_shorten_a_shot
                            // survives reflection. Damage rides along: catch
                            // a Heavy round and you send two damage back.
                            b.vel = [b.vel[0] - 2.0 * dot * n[0], b.vel[1] - 2.0 * dot * n[1]];
                            // The round belongs to whoever caught it. It can
                            // now kill anyone, the shooter included, and the
                            // frag is the reflector's. remove_player drops
                            // bullets by owner, so this also decides that a
                            // reflected round outlives the shooter leaving
                            // and dies with the reflector instead — which is
                            // what the transfer means, not an accident of it.
                            b.owner = p.id;
                            // Nobody aimed this round, so there is nothing for
                            // lag compensation to honour: from here it
                            // hit-tests against the present.
                            b.delay = 0;
                            // Keeping the round's position for this tick is
                            // what makes the reflection single-pass: the
                            // players after this one in the loop are being
                            // tested against a segment computed from the OLD
                            // velocity, and leaving now is the only way none
                            // of them is judged against a path the round is
                            // no longer on. It costs one tick, 17 ms.
                            return true;
                        }
                    }
                    // A head hit kills outright, whatever the weapon and
                    // whatever the remaining HP. Routed as damage rather than
                    // as a special case so respawn, scoring and the kill event
                    // all stay on the one path.
                    hits.push((b.owner, p.id, if head { MAX_HP } else { b.dmg }));
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
    // Test-only casts use small fixed ranges and intentionally exercise production formulas.
    #![allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]

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
                let cx = f32::midpoint(o.min[0], o.max[0]);
                let cz = f32::midpoint(o.min[1], o.max[1]);
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
        for _ in 0..((weapon_stats(1).cooldown / FIXED_DT) as u32 + 2) * (u32::from(mag) + 4) {
            step_with(&mut sim, &inputs);
            // Bullets fly off and expire; count spawns via ammo drops.
            let b = sim.bullets.len();
            if b > prev_bullets {
                fired += (b - prev_bullets) as u32;
            }
            prev_bullets = b;
        }
        assert_eq!(
            fired,
            u32::from(mag),
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

    #[test]
    fn a_seeded_level_is_exactly_the_arena_the_generator_made() {
        // The whole point of bite 6: `Level` carries heights and spawns as
        // data, and for a seed it must reproduce the world every peer
        // already generates — bit for bit, or a client and the server
        // would disagree about where cover is.
        for seed in 0..256u64 {
            let level = Level::from_seed(seed);
            let generated = generate_arena(seed);
            assert_eq!(
                level.obstacles, generated,
                "seed {seed}: level obstacles diverge from generate_arena"
            );
            assert_eq!(level.arena_half, ARENA_HALF);
            for o in &level.obstacles {
                // The height rule as it stood BEFORE `h` moved onto
                // Obstacle, with its constants written out here rather than
                // shared with the code under test — otherwise a later edit
                // to the generator could quietly redefine "unchanged".
                let k = (o.min[0] * 12.9898 + o.min[1] * 78.233).sin() * 43758.547;
                let f = k - k.floor();
                let want = if f < 0.6 {
                    0.9 + (f / 0.6) * (1.5 - 0.9)
                } else {
                    2.4 + ((f - 0.6) / 0.4) * 0.8
                };
                assert!(
                    (o.h - want).abs() < 1e-6,
                    "seed {seed}: carried h {} != derived {want}",
                    o.h
                );
                assert!(
                    (obstacle_height(o) - o.h).abs() < f32::EPSILON,
                    "obstacle_height must now just read the field"
                );
            }
            // Spawns reproduce the golden-angle ring, and the fallback
            // agrees with the carried list.
            assert_eq!(level.spawns.len(), MAX_PLAYERS);
            for slot in 0..MAX_PLAYERS as u32 {
                assert_eq!(
                    level.spawn(slot),
                    spawn_point(slot),
                    "seed {seed} slot {slot}"
                );
            }
        }
    }

    #[test]
    fn an_authored_box_keeps_the_height_it_was_given() {
        // The reason `h` moved at all: a box must be able to state its own
        // height instead of having one hashed out of its position.
        let authored = Obstacle {
            min: [3.0, 3.0],
            max: [5.0, 5.0],
            h: 7.25,
        };
        assert_eq!(obstacle_height(&authored), 7.25);
        // And it is honoured by the physics, not just stored: a player at
        // floor level is blocked by it, and its top supports them.
        let obs = vec![authored];
        assert_eq!(support_height([4.0, 4.0], PLAYER_R, &obs), 7.25);
        let walked = move_circle([1.0, 4.0], 0.0, [1.0, 0.0], MOVE_SPEED, 0.5, &obs);
        assert!(walked[0] < 3.0, "authored height did not block: {walked:?}");
    }

    #[test]
    fn a_level_with_no_spawns_still_starts_players() {
        let level = Level {
            arena_half: ARENA_HALF,
            obstacles: Vec::new(),
            spawns: Vec::new(),
        };
        assert_eq!(level.spawn(3), spawn_point(3));
        // A short authored list wraps rather than panicking.
        let two = Level {
            arena_half: ARENA_HALF,
            obstacles: Vec::new(),
            spawns: vec![[1.0, 2.0], [3.0, 4.0]],
        };
        assert_eq!(two.spawn(0), [1.0, 2.0]);
        assert_eq!(two.spawn(5), [3.0, 4.0]);
    }

    /// One waist-high crate at the origin, and nothing else.
    fn one_box() -> Vec<Obstacle> {
        vec![Obstacle::seeded([-1.5, -1.5], [1.5, 1.5])]
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
        // Pinned, not bracketed: the client replays this same function, and
        // the bug this guards against (reconciling y against a stale vy)
        // collapsed the arc to 1.393 m - comfortably inside a 1.0..2.0 window.
        assert!(
            (peak - 1.686_666_7).abs() < 5e-3,
            "jump peak {peak}, expected the discrete apex 1.6867"
        );
        assert!(airborne_ticks > 20, "hang time {airborne_ticks} ticks");
        assert!(y.abs() < 1e-3, "did not land: {y}");
    }

    #[test]
    fn a_box_blocks_on_the_ground_but_not_from_above() {
        let obs = one_box();
        let top = obstacle_height(&obs[0]);
        // Walking into the crate from outside gets stopped.
        let walked = move_circle([-3.0, 0.0], 0.0, [1.0, 0.0], MOVE_SPEED, 0.5, &obs);
        assert!(
            walked[0] < -1.5 - PLAYER_R + 0.01,
            "walked into box: {walked:?}"
        );
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
        let obs = generate_arena(20_260_829);
        let heights: Vec<f32> = obs.iter().map(obstacle_height).collect();
        assert!(
            heights.iter().any(|h| *h <= CRATE_MAX_H),
            "no crates: {heights:?}"
        );
        assert!(
            heights.iter().any(|h| *h >= CONTAINER_MIN_H),
            "no containers"
        );
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
        assert!(
            sim.players[0].y > 0.1,
            "jump did not lift: {}",
            sim.players[0].y
        );
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
                Obstacle::seeded([x, -2.0], [x + 1.0, 2.0])
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
        assert_eq!(
            sim.players[0].pitch, 0.0,
            "NaN pitch must fall back to level"
        );

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

    // ---- the off-hand shield ----

    /// A shooter at the origin firing +x at a defender `dist` away, for
    /// exactly one round on the first tick. The defender aims along
    /// `def_aim` and holds the shield iff `shield`. Both are pinned in place
    /// every tick so only the thing under test varies. Runs `ticks` steps.
    fn shield_duel(dist: f32, def_aim: [f32; 2], shield: bool, ticks: u32) -> Sim {
        let mut sim = Sim::new(31);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0); // shooter
        sim.add_player(1); // defender
        let defender = PlayerIn {
            aim: def_aim,
            shield,
            ..Default::default()
        };
        for t in 0..ticks {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => p.pos = [0.0, 0.0],
                1 if p.alive => p.pos = [dist, 0.0],
                _ => {}
            });
            let shooter = PlayerIn {
                aim: [1.0, 0.0],
                // One round, on the first tick only: a held trigger would
                // put several rounds in flight and make "the" reflected
                // bullet ambiguous.
                fire: t == 0,
                ..Default::default()
            };
            let mut inputs = HashMap::new();
            inputs.insert(0, shooter);
            inputs.insert(1, defender);
            step_with(&mut sim, &inputs);
        }
        sim
    }

    #[test]
    fn a_raised_shield_sends_the_round_back() {
        // The whole feature: a frontal round is not absorbed, it is mirrored
        // and changes hands. 10 ticks is past the ~7 the round needs to
        // reach the defender's circle and short of the ~15 by which it is
        // back at the shooter and consumed, so it is caught mid-return.
        let sim = shield_duel(5.0, [-1.0, 0.0], true, 10);
        let b = sim
            .bullets
            .first()
            .expect("a reflected round must still be in flight");
        assert_eq!(b.owner, 1, "the round belongs to whoever caught it");
        // Head-on off a shield facing -x: v = (+34, 0) becomes (-34, 0).
        assert!(
            b.vel[0] < 0.0,
            "the round must be travelling back downrange, got {:?}",
            b.vel
        );
        let speed_h = (b.vel[0] * b.vel[0] + b.vel[1] * b.vel[1]).sqrt();
        assert!(
            (speed_h - BULLET_SPEED).abs() < 1e-3,
            "a mirror is an isometry; horizontal speed must survive it, got {speed_h}"
        );
        // Level fire, so the reflection must leave the vertical alone.
        assert_eq!(b.vy, 0.0, "a horizontal normal cannot change vy");
        let defender = sim.players.iter().find(|p| p.id == 1).unwrap();
        assert_eq!(
            defender.hp, MAX_HP,
            "a reflected round must not also damage the reflector"
        );
        assert_eq!(sim.events, Vec::<(u8, u8)>::new());
    }

    #[test]
    fn a_reflected_round_kills_the_shooter_and_credits_the_reflector() {
        // The round trip: fire, catch, and be killed by your own bullet.
        let mut sim = Sim::new(31);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        sim.players[0].hp = 1; // one round back is lethal
        let defender = PlayerIn {
            aim: [-1.0, 0.0],
            shield: true,
            ..Default::default()
        };
        let mut killed = false;
        for t in 0..40 {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => p.pos = [0.0, 0.0],
                1 if p.alive => p.pos = [5.0, 0.0],
                _ => {}
            });
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    aim: [1.0, 0.0],
                    fire: t == 0,
                    ..Default::default()
                },
            );
            inputs.insert(1, defender);
            step_with(&mut sim, &inputs);
            if !sim.events.is_empty() {
                killed = true;
                break;
            }
        }
        assert!(killed, "the reflected round never came home");
        assert_eq!(
            sim.events,
            vec![(1, 0)],
            "the kill is credited to the reflector, against the shooter"
        );
        let reflector = sim.players.iter().find(|p| p.id == 1).unwrap();
        assert_eq!(reflector.score, 1);
        assert_eq!(reflector.hp, MAX_HP, "the reflector was never hit");
        let shooter = sim.players.iter().find(|p| p.id == 0).unwrap();
        assert!(!shooter.alive);
        assert_eq!(shooter.death_count, 1);
        assert_eq!(shooter.score, 0, "shooting yourself is not a frag");
    }

    #[test]
    fn the_shield_only_covers_the_arc_it_claims() {
        // Cover is frontal, so flanking beats it. The defender's aim is
        // rotated off head-on by theta; the round always arrives along +x,
        // so the angle between the round's heading and the plate's face is
        // exactly theta and the boundary sits at SHIELD_ARC / 2.
        let half = SHIELD_ARC * 0.5;
        let hp_after = |theta: f32, shield: bool| -> u8 {
            let (s, c) = theta.sin_cos();
            let sim = shield_duel(5.0, [-c, s], shield, 20);
            sim.players.iter().find(|p| p.id == 1).unwrap().hp
        };
        // Well inside the arc: caught, no damage.
        assert_eq!(hp_after(0.0, true), MAX_HP, "head-on must be reflected");
        assert_eq!(
            hp_after(half - 0.1, true),
            MAX_HP,
            "just inside the arc must be reflected"
        );
        // Outside it: the shield is irrelevant and the round damages exactly
        // as it would with no shield at all.
        let flanked = hp_after(half + 0.1, true);
        let bare = hp_after(half + 0.1, false);
        assert!(
            flanked < MAX_HP,
            "a round from outside the arc must still damage"
        );
        assert_eq!(
            flanked, bare,
            "outside the arc the shield must change nothing"
        );
        // A shot in the back, with the defender facing the same way the
        // round is travelling: the far end of the same rule.
        let back = hp_after(std::f32::consts::PI, true);
        assert_eq!(back, bare, "a shot in the back ignores the shield");
    }

    #[test]
    fn a_raised_shield_holds_the_trigger() {
        let mut sim = Sim::new(32);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        let mag = weapon_stats(1).mag;
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                fire: true,
                shield: true,
                ..Default::default()
            },
        );
        for _ in 0..60 {
            step_with(&mut sim, &inputs);
            assert!(
                sim.bullets.is_empty(),
                "no round may leave a weapon behind a raised shield"
            );
        }
        assert_eq!(
            sim.players[0].ammo, mag,
            "a blocked trigger must not spend ammunition either"
        );
        // Lowering it restores fire on the very next tick — the cooldown ran
        // down behind the shield, exactly as it does when the trigger is
        // simply not pulled.
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                fire: true,
                shield: false,
                ..Default::default()
            },
        );
        step_with(&mut sim, &inputs);
        assert_eq!(
            sim.bullets.len(),
            1,
            "releasing the shield must fire on the same tick"
        );
        assert_eq!(sim.players[0].ammo, mag - 1);
    }

    #[test]
    fn a_raised_shield_cancels_sprint() {
        // The rule itself, on the shared function both sides call...
        assert_eq!(
            stance_speed(true, false, true),
            stance_speed(false, false, false)
        );
        assert!(stance_speed(true, false, false) > stance_speed(true, false, true));
        // ...and crouch still wins over both.
        assert_eq!(
            stance_speed(true, true, true),
            stance_speed(false, true, false)
        );

        // ...and through the sim, where the client's prediction reads it.
        let run = |shield: bool| -> f32 {
            let mut sim = Sim::new(33);
            sim.obstacles.clear();
            sim.pads.clear();
            sim.add_player(0);
            sim.players[0].pos = [-20.0, 0.0];
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    mv: [1.0, 0.0],
                    aim: [1.0, 0.0],
                    sprint: true,
                    shield,
                    ..Default::default()
                },
            );
            for _ in 0..60 {
                step_with(&mut sim, &inputs);
            }
            sim.players[0].pos[0] + 20.0
        };
        assert!((run(false) - MOVE_SPEED * SPRINT_MULT).abs() < 0.3);
        assert!((run(true) - MOVE_SPEED).abs() < 0.2);
    }

    #[test]
    fn a_corpse_carries_no_shield() {
        let mut sim = Sim::new(34);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                shield: true,
                ..Default::default()
            },
        );
        step_with(&mut sim, &inputs);
        assert!(sim.players[0].shield, "a live player's Q is honoured");
        let p = &mut sim.players[0];
        p.hp = 0;
        p.alive = false;
        p.respawn_in = RESPAWN_SECS;
        step_with(&mut sim, &inputs);
        assert!(
            !sim.players[0].shield,
            "the dead broadcast no shield, whatever they are still holding"
        );
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

    // ---- v12: melee and headshots -------------------------------------

    /// Two players facing each other at `gap`, with the defender optionally
    /// holding the shield straight at the attacker. Returns whether the
    /// defender died within `ticks`.
    fn melee_duel(gap: f32, shielded: bool, attacker_aim: [f32; 2], ticks: u32) -> bool {
        let mut sim = Sim::new(11);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: attacker_aim,
                melee: true,
                ..Default::default()
            },
        );
        inputs.insert(
            1,
            PlayerIn {
                // Facing back down -x, straight into the attacker: the most
                // favourable case the shield can possibly have.
                aim: [-1.0, 0.0],
                shield: shielded,
                ..Default::default()
            },
        );
        for _ in 0..ticks {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => p.pos = [0.0, 0.0],
                1 if p.alive => p.pos = [gap, 0.0],
                _ => {}
            });
            step_with(&mut sim, &inputs);
            if !sim.players.iter().find(|p| p.id == 1).unwrap().alive {
                return true;
            }
        }
        false
    }

    #[test]
    fn melee_kills_through_a_raised_shield() {
        // The headline of v12. The shield blocks and REFLECTS a round from
        // dead ahead - `head_on_is_reflected` covers that - and a melee from
        // the identical geometry must ignore it completely.
        assert!(
            melee_duel(1.5, true, [1.0, 0.0], 4),
            "a melee must kill through a raised shield"
        );
        assert!(
            melee_duel(1.5, false, [1.0, 0.0], 4),
            "and must obviously still kill an unshielded target"
        );
    }

    #[test]
    fn melee_is_lethal_from_full_health() {
        // No chip damage: one connect is a kill regardless of HP.
        let mut sim = Sim::new(11);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        assert_eq!(sim.players.iter().find(|p| p.id == 1).unwrap().hp, MAX_HP);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                melee: true,
                ..Default::default()
            },
        );
        inputs.insert(1, PlayerIn::default());
        sim.players.iter_mut().for_each(|p| match p.id {
            0 => p.pos = [0.0, 0.0],
            1 => p.pos = [1.5, 0.0],
            _ => {}
        });
        step_with(&mut sim, &inputs);
        let v = sim.players.iter().find(|p| p.id == 1).unwrap();
        assert!(!v.alive, "one connect must kill outright from full health");
        assert_eq!(
            sim.players.iter().find(|p| p.id == 0).unwrap().score,
            1,
            "and must score exactly like any other kill"
        );
    }

    #[test]
    fn melee_misses_out_of_reach_and_behind() {
        // Reach is MELEE_RANGE plus the target's own radius, so a standing
        // target is struck out to 2.6 and not beyond.
        assert!(
            melee_duel(2.5, false, [1.0, 0.0], 4),
            "just inside reach must connect"
        );
        assert!(
            !melee_duel(3.2, false, [1.0, 0.0], 4),
            "beyond reach must miss"
        );
        // Facing away: the target is behind the swing, outside the arc.
        assert!(
            !melee_duel(1.5, false, [-1.0, 0.0], 4),
            "a swing away from the target must miss"
        );
    }

    #[test]
    fn melee_respects_its_cooldown() {
        // Hold E down and the server must not turn it into a proximity field:
        // the press is consumed, and the cooldown gates the next swing. Here
        // the input stays true every tick (which is what a naive client or a
        // hostile one sends), so only the cooldown stands between the
        // attacker and a kill every tick.
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
                melee: true,
                ..Default::default()
            },
        );
        inputs.insert(1, PlayerIn::default());
        // First swing lands immediately.
        sim.players.iter_mut().for_each(|p| match p.id {
            0 => p.pos = [0.0, 0.0],
            1 => p.pos = [1.5, 0.0],
            _ => {}
        });
        step_with(&mut sim, &inputs);
        assert!(!sim.players.iter().find(|p| p.id == 1).unwrap().alive);
        let cd = sim.players.iter().find(|p| p.id == 0).unwrap().melee_cd;
        assert!(
            (cd - MELEE_COOLDOWN).abs() < 1e-3,
            "a swing must charge the full cooldown, got {cd}"
        );
        // Revive the victim in place and keep holding E: the attacker must be
        // unable to swing again until the cooldown has run out.
        let ticks_until_ready = (MELEE_COOLDOWN / FIXED_DT) as u32;
        for _ in 0..(ticks_until_ready - 2) {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 => p.pos = [0.0, 0.0],
                1 => {
                    p.pos = [1.5, 0.0];
                    p.alive = true;
                    p.hp = MAX_HP;
                    p.respawn_in = 0.0;
                }
                _ => {}
            });
            step_with(&mut sim, &inputs);
            assert!(
                sim.players.iter().find(|p| p.id == 1).unwrap().alive,
                "no second swing may land while the cooldown is running"
            );
        }
    }

    /// Fire one shot from `from_y` at `pitch` into a standing target `gap`
    /// away, and report whether it died on the FIRST round that connected.
    fn one_shot_kills(from_y: f32, gap: f32, pitch: f32) -> bool {
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
                pitch,
                fire: true,
                ..Default::default()
            },
        );
        inputs.insert(1, PlayerIn::default());
        for _ in 0..240 {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => {
                    p.pos = [0.0, 0.0];
                    p.y = from_y;
                }
                1 if p.alive => {
                    p.pos = [gap, 0.0];
                    p.y = 0.0;
                }
                _ => {}
            });
            step_with(&mut sim, &inputs);
            let v = sim.players.iter().find(|p| p.id == 1).unwrap();
            if !v.alive {
                return true;
            }
            if v.hp < MAX_HP {
                // It connected and did NOT kill: a body hit.
                return false;
            }
        }
        panic!("the shot never connected at all - the test geometry is wrong");
    }

    #[test]
    fn a_headshot_kills_outright() {
        // Standing target: head band is [1.48, 1.70]. A round leaves at 1.45
        // and climbs tan(pitch) per unit travelled, so over 5 units a pitch of
        // 0.03 arrives at ~1.60 - the middle of the head.
        assert!(
            one_shot_kills(0.0, 5.0, 0.03),
            "a round arriving in the head band must kill from full health"
        );
    }

    #[test]
    fn a_body_shot_still_takes_three() {
        // The same geometry aimed down into the chest must NOT one-shot, or
        // the head zone has swallowed the whole body.
        assert!(
            !one_shot_kills(0.0, 5.0, -0.05),
            "a chest hit must not be lethal"
        );
    }

    #[test]
    fn level_fire_is_not_a_free_headshot() {
        // This is the balance guard for HEAD_H, and it is the whole reason
        // that constant is 0.22 rather than something rounder. A round leaves
        // at EYE_STAND 1.45 and flies flat at pitch 0, so if the head band
        // ever reaches down to 1.45 then two standing players simply shooting
        // at each other trade instant kills without anyone aiming at a head.
        assert!(
            HEAD_H < BODY_H_STAND - EYE_STAND,
            "HEAD_H {HEAD_H} must stay under {} or level fire becomes a headshot",
            BODY_H_STAND - EYE_STAND
        );
        assert!(
            !one_shot_kills(0.0, 5.0, 0.0),
            "a flat shot between two standing players must be a body hit"
        );
    }

    #[test]
    fn a_steep_headshot_still_registers() {
        // The regression guard for the sub-stepping threshold.
        //
        // Vertical travel in one tick is tan(pitch) * BULLET_SPEED * FIXED_DT
        // = 0.567 * tan(pitch). The walk used to trigger only when that
        // exceeded the whole BODY band (2.14 m), i.e. past ~1.31 rad. A head
        // band of 0.22 is straddled from just 0.37 rad, so between those two
        // angles consecutive samples could step clean over a head while the
        // guard stayed asleep - and the headshot became a coin flip across
        // the entire range of ordinary downward aiming.
        //
        // Fire down from a container top through the target's head. At this
        // angle one tick covers far more than the head band, so this only
        // passes because the walk is keyed on the SMALLEST zone under test.
        let from_y = 3.0;
        let gap = 3.0;
        // Aim so the round arrives at ~1.60, the middle of the head band.
        let drop = (from_y + EYE_STAND) - 1.60;
        let pitch = -(drop / gap).atan();
        let per_tick = pitch.tan().abs() * BULLET_SPEED * FIXED_DT;
        assert!(
            per_tick > HEAD_H,
            "test must actually exercise the straddle case: {per_tick} vs {HEAD_H}"
        );
        assert!(
            per_tick < BODY_H_STAND + 2.0 * BULLET_R,
            "and must sit BELOW the old body-band trigger, or it proves nothing"
        );
        assert!(
            one_shot_kills(from_y, gap, pitch),
            "a steep round through the head must still be a headshot"
        );
    }
}
