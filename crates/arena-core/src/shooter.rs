//! The arena shooter sim: pure, deterministic, fixed 60 Hz.
//!
//! Runs authoritatively on the server; clients render its broadcast state
//! and build the identical arena from the `Level` the lobby names (and, for
//! the seeded arena, from the lobby's seed).

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
///
/// 0.30 is not a tuning knob: it is the height of the head part the client
/// actually draws (`ember-engine/src/rig.rs:686-694` anchors it bottom-centre
/// at NECK + 0.01 with a target height of 0.30). Because `BODY_H_*` above is
/// now the drawn head's TOP, subtracting the drawn head's HEIGHT puts the zone
/// exactly on the drawn head: [1.56, 1.86] standing, [1.25, 1.55] crouched.
/// Change the model's head and this must follow, or headshots stop landing
/// where players aim.
///
/// It must also stay under `BODY_H_STAND - EYE_STAND` = 0.41. Rounds leave the
/// muzzle at `EYE_STAND` 1.45 and fly level at pitch 0, so a band reaching down
/// to 1.45 would make every level shot between two standing players a headshot
/// and, with headshots lethal, the pistol would one-shot the whole game with
/// nobody aiming at a head. At 0.30 the band starts at 1.56, so level fire
/// lands in the chest and a standing kill needs deliberate upward aim.
/// `level_fire_is_not_a_free_headshot` pins that margin.
pub const HEAD_H: f32 = 0.30;

/// Melee reach from the attacker's centre, before the target's own radius is
/// added.
///
/// 2.0 + `PLAYER_R` 0.6 strikes a standing target at 2.6 centre to centre,
/// about one and a half body widths - a lunge, not a spear.
pub const MELEE_RANGE: f32 = 2.0;
/// Full width of the melee cone, radians (~115 deg).
///
/// Wider than `SHIELD_ARC` on purpose: the shield answers "am I covered from
/// that round", which wants to be demanding, while a swing at contact range
/// wants to land when it visually should.
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
/// The vertical extent matches the silhouette the client draws, so that what
/// you see is what you hit. These live here, not in the renderer, because
/// client and server must agree where a body IS.
///
/// They were 1.70 / 1.25 and did NOT match it. The rig draws a standing head
/// at [1.56, 1.86]: ROOT sits at `pelvis_h` 0.98, SPINE is +0.05 above it,
/// NECK is +`spine_len` 0.52 above that (`ember-engine/src/rig.rs:128-130`,
/// defaults at `:100-105`), and the head part is anchored bottom-centre 0.01
/// above NECK with a target height of 0.30 (`rig.rs:686-694`). So 1.70 cut the
/// drawn head in half and left its top 16 cm unhittable - not even for body
/// damage. Crouched was worse: `walk_pose` sinks the root by
/// `crouch * (thigh_len 0.44 + shin_len 0.43) * 0.36` = 0.313 (`rig.rs:434`),
/// putting the drawn crouched head near [1.25, 1.55] against a volume that
/// stopped at 1.25 - a crouched player's visible head sat entirely OUTSIDE
/// their own hitbox.
///
/// That was survivable while every hit was worth the same. It is not
/// survivable with a head zone, because the player aims at a head they can
/// see. These are now the drawn heights, so `HEAD_H` below lands on the drawn
/// head in both stances.
///
/// Consequence, taken deliberately: crouch is weaker than it was. The old note
/// here observed that a standing muzzle at 1.45 merely GRAZED a crouched band
/// topping out at 1.47. That band now reaches 1.77, so level fire connects
/// solidly - and since 1.45 falls inside the crouched head band [1.25, 1.55],
/// level fire at a crouched target is a headshot. Crouch still shrinks your
/// radius; it no longer also hides the part of you that was never in the
/// hitbox to begin with.
pub const BODY_H_STAND: f32 = 1.86;
pub const BODY_H_CROUCH: f32 = 1.55;
// Worth knowing before tuning either of the above: they are tied to the rig
// now, so moving one without moving the model reintroduces exactly the
// aim-at-what-you-cannot-hit bug they were changed to fix. If the character
// model changes height these follow it, and HEAD_H follows its head part.

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

/// Bottom of the head zone, measured from the target's feet. A round arriving
/// at or above this, and still inside the body volume, kills outright.
///
/// Lives beside `body_h` and `eye_h` and for the same reason: it decides who
/// dies, so client and server must agree on it, and the renderer must not be
/// the one to define it.
#[must_use]
pub const fn head_lo(crouch: bool) -> f32 {
    body_h(crouch) - HEAD_H
}

/// Hard clamp on aim pitch, radians (~83°). The client clamps its own look
/// identically, but that clamp is cosmetic — a peer's pitch is untrusted
/// input and is re-clamped here, where it decides who dies.
pub const MAX_PITCH: f32 = 1.45;
/// The sidearm's numbers, kept as named constants because the sidearm row
/// of the weapon table IS today's pistol and every shot test written before
/// v18 reads its world through these.
pub const BULLET_SPEED: f32 = 34.0;
pub const BULLET_R: f32 = 0.22;
pub const BULLET_TTL: f32 = 1.6;
pub const RELOAD_SECS: f32 = 1.1;
pub const PAD_RESPAWN_SECS: f32 = 15.0;
pub const PAD_RADIUS: f32 = 1.3;
/// A pad is taken only with the feet below this.
///
/// The contact test is a horizontal circle, so without it a player standing
/// on a tunnel roof collected the pad 2.9 m under their boots through the
/// slab. Under every roof base (2.5); and a hop over a pad takes off and
/// lands below it, so a pad on open floor is still grabbed in passing.
pub const PAD_PICK_H: f32 = 1.0;

/// The weapon everyone spawns with and falls back to: today's pistol, bit
/// for bit, with an infinite reserve.
pub const SIDEARM: u8 = 1;
/// Weapon ids run `1..=WEAPON_COUNT`. `weapon_stats` answers any id, so a
/// client that reads an id it does not know still draws something, but the
/// loot roll only ever hands out members of `LOOT_POOL`.
pub const WEAPON_COUNT: u8 = 7;
/// A reserve that reload never draws down. Only the sidearm carries it; a
/// looted gun carries one finite reserve and no pickup refills it, which is
/// what makes ammo the clock on every gun but the sidearm.
pub const RESERVE_INFINITE: u8 = 255;
/// What a block or a pad may hand out. Server-side only: clients never
/// derive from it, so adding the M4 (id 4) when its mesh exists is not a
/// protocol question.
pub const LOOT_POOL: [u8; 5] = [2, 3, 5, 6, 7];
/// An airborne shooter's cone widens by this much, so a jumping spray is a
/// worse spray than a planted one.
pub const ADS_SPREAD_AIR_MULT: f32 = 1.6;
/// A block's edge length. Every loot block is one metre on a side.
pub const LOOT_SIZE: f32 = 1.0;
/// How long a bonked block stays dark before it pays again.
pub const LOOT_RESPAWN_SECS: f32 = 18.0;
/// The `roll` salt that separates a loot roll from a spread roll on the same
/// tick for the same player: without it the gun a block hands out and the
/// cone of the round fired that tick would be one number.
pub const SALT_LOOT: u16 = 0x1007;

/// What leaves the muzzle. A rocket detonates on whatever it touches and
/// splashes; a bullet hits one body (or, with pierce, the next as well).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Projectile {
    Bullet,
    Rocket,
}

/// One row of the weapon table.
///
/// `docs/plans/arena-v18-freight-yard.md` section 3.1. Every number here is
/// the number that ships; the table is the one source and the sim reads
/// gravity, radius and splash through it by `Bullet.weapon` rather than
/// carrying them on every round.
#[derive(Clone, Copy, Debug)]
pub struct WeaponStats {
    pub name: &'static str,
    pub cooldown: f32,
    pub mag: u8,
    /// Rounds outside the magazine when the gun is picked up.
    pub reserve: u8,
    pub damage: u8,
    /// Muzzle speed, units per second; the horizontal `vel` magnitude.
    pub speed: f32,
    /// Seconds of flight; `speed * ttl` is the range.
    pub ttl: f32,
    /// Hit radius of the round itself.
    pub radius: f32,
    /// Base cone half-angle, radians; `bloom` widens it per round fired
    /// since the last reload, capped at `spread_max`.
    pub spread: f32,
    pub bloom: f32,
    pub spread_max: f32,
    /// Cone multiplier while aiming down the sights.
    pub ads_spread: f32,
    /// Vertical acceleration on the round, units per second squared, zero
    /// or negative.
    pub gravity: f32,
    /// How many bodies the round passes through before it stops.
    pub pierce: u8,
    pub kind: Projectile,
    /// Splash radius of a rocket; zero for a bullet.
    pub splash_r: f32,
    pub reload: f32,
}

/// The table row for an id.
///
/// A match rather than a clamp because `Ord::clamp` is not const; the `_`
/// arm is the sidearm, so an id above the table (a v13 client reading
/// `weapon: 7` is the documented case) and zero both read as the gun
/// everyone holds by default. One match, not seven functions, so every
/// number sits beside the others it is tuned against.
#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn weapon_stats(id: u8) -> WeaponStats {
    match id {
        2 => WeaponStats {
            name: "Vityaz",
            cooldown: 0.08,
            mag: 30,
            reserve: 60,
            damage: 1,
            speed: 34.0,
            ttl: 0.75,
            radius: BULLET_R,
            spread: 0.015,
            bloom: 0.005,
            spread_max: 0.075,
            ads_spread: 0.5,
            gravity: 0.0,
            pierce: 0,
            kind: Projectile::Bullet,
            splash_r: 0.0,
            reload: 1.3,
        },
        3 => WeaponStats {
            name: "AK-47",
            cooldown: 0.115,
            mag: 30,
            reserve: 30,
            damage: 1,
            speed: 44.0,
            ttl: 1.1,
            radius: BULLET_R,
            spread: 0.006,
            bloom: 0.006,
            spread_max: 0.05,
            ads_spread: 0.5,
            gravity: 0.0,
            pierce: 0,
            kind: Projectile::Bullet,
            splash_r: 0.0,
            reload: 1.5,
        },
        4 => WeaponStats {
            name: "M4",
            cooldown: 0.09,
            mag: 30,
            reserve: 30,
            damage: 1,
            speed: 40.0,
            ttl: 0.85,
            radius: BULLET_R,
            spread: 0.008,
            bloom: 0.003,
            spread_max: 0.04,
            ads_spread: 0.5,
            gravity: 0.0,
            pierce: 0,
            kind: Projectile::Bullet,
            splash_r: 0.0,
            reload: 1.4,
        },
        5 => WeaponStats {
            name: "Revolver",
            cooldown: 0.42,
            mag: 6,
            reserve: 12,
            damage: 2,
            speed: 30.0,
            ttl: 1.5,
            radius: BULLET_R,
            spread: 0.0,
            bloom: 0.0,
            spread_max: 0.0,
            ads_spread: 1.0,
            gravity: -3.0,
            pierce: 0,
            kind: Projectile::Bullet,
            splash_r: 0.0,
            reload: 1.5,
        },
        6 => WeaponStats {
            name: "Sniper",
            cooldown: 0.9,
            mag: 5,
            reserve: 5,
            damage: 2,
            // 60, not 90: at MAX_PITCH the head-band walk clamps at 32
            // samples, and 60 m/s is 0.26 m per sample, under HEAD_H 0.30.
            // At 90 a steep headshot would be a coin flip.
            speed: 60.0,
            ttl: 1.0,
            radius: BULLET_R,
            spread: 0.06,
            bloom: 0.0,
            spread_max: 0.06,
            ads_spread: 0.0,
            gravity: 0.0,
            pierce: 1,
            kind: Projectile::Bullet,
            splash_r: 0.0,
            reload: 1.8,
        },
        7 => WeaponStats {
            name: "RPG-7",
            cooldown: 1.2,
            mag: 1,
            reserve: 2,
            damage: 3,
            speed: 24.0,
            ttl: 2.5,
            radius: 0.35,
            spread: 0.0,
            bloom: 0.0,
            spread_max: 0.0,
            ads_spread: 1.0,
            gravity: -5.0,
            pierce: 0,
            kind: Projectile::Rocket,
            splash_r: 3.0,
            reload: 2.4,
        },
        // The sidearm: today's pistol, through the same constants the
        // pre-v18 shot tests read, so its round is bit-identical.
        _ => WeaponStats {
            name: "Sidearm",
            cooldown: 0.18,
            mag: 8,
            reserve: RESERVE_INFINITE,
            damage: 1,
            speed: BULLET_SPEED,
            ttl: BULLET_TTL,
            radius: BULLET_R,
            spread: 0.0,
            bloom: 0.0,
            spread_max: 0.0,
            ads_spread: 1.0,
            gravity: 0.0,
            pierce: 0,
            kind: Projectile::Bullet,
            splash_r: 0.0,
            reload: RELOAD_SECS,
        },
    }
}

#[must_use]
pub const fn weapon_name(id: u8) -> &'static str {
    weapon_stats(id).name
}

/// splitmix64's finaliser: integer arithmetic only, so it is identical on
/// every peer and platform, which a float hash is not.
#[must_use]
pub const fn hash64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// One roll for (level seed, tick, player, salt).
///
/// This is the seeded and tick-indexed per-tick randomness `CLAUDE.md`
/// demands, with no RNG state at all: the same inputs give the same number
/// on every peer and in every replay. The salt separates the pellets of one
/// tick from each other and a loot roll from a spread roll.
#[must_use]
pub const fn roll(seed: u64, tick: u64, who: u8, salt: u16) -> u64 {
    hash64(
        seed ^ hash64(tick.wrapping_add(0x9e37_79b9_7f4a_7c15))
            ^ ((who as u64) << 56)
            ^ ((salt as u64) << 40),
    )
}

/// Two uniforms in [0, 1) from one roll: 24 mantissa bits each, so both are
/// exact in f32 and the same bits give the same float everywhere.
#[must_use]
// 24 bits into a 24-bit mantissa, divided by a power of two: exact, no loss.
#[allow(clippy::cast_precision_loss)]
pub fn unit_pair(h: u64) -> (f32, f32) {
    const BITS: u64 = 0x00ff_ffff;
    const SCALE: f32 = 16_777_216.0;
    let u = ((h >> 40) & BITS) as f32 / SCALE;
    let v = ((h >> 16) & BITS) as f32 / SCALE;
    (u, v)
}

/// The gun a block or a pad hands out.
///
/// Uniform over `LOOT_POOL` minus the gun in hand (when that is in the
/// pool), indexed by the loot-salted roll. No weights: five guns of five
/// roles, and weighting is balance tuning with no evidence yet.
#[must_use]
pub fn loot_roll(seed: u64, tick: u64, who: u8, holding: u8) -> u8 {
    let mut offered = LOOT_POOL.iter().copied().filter(|&w| w != holding);
    // Never zero: the pool has five entries and at most one is held.
    let n = u64::try_from(offered.clone().count()).unwrap_or(1);
    // The top bits of the hash are the best mixed; `>> 33` leaves 31 of
    // them, plenty for a modulus of five.
    let k = (roll(seed, tick, who, SALT_LOOT) >> 33) % n;
    // `k < n`, so the nth always exists and neither fallback is taken.
    offered
        .nth(usize::try_from(k).unwrap_or(0))
        .unwrap_or(LOOT_POOL[0])
}

/// Exact slab test of a 3D segment `a -> b` against a box.
///
/// The box is `[min, max] x [base, h]`. Arithmetic only, so it is safe
/// wherever the sweep runs: a rocket's splash reads it to ask whether a body
/// has line of sight to the blast, and unlike the sweep's conservative span
/// test it clears a target whose chest is above a crate the blast sits
/// behind.
#[must_use]
pub fn segment_hits_box(a: [f32; 3], b: [f32; 3], o: &Obstacle) -> bool {
    let lo = [o.min[0], o.base, o.min[1]];
    let hi = [o.max[0], o.h, o.max[1]];
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for axis in 0..3 {
        let d = b[axis] - a[axis];
        if d.abs() < 1e-9 {
            // Parallel to this slab: inside it or never.
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return false;
            }
            continue;
        }
        let mut near = (lo[axis] - a[axis]) / d;
        let mut far = (hi[axis] - a[axis]) / d;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t0 = t0.max(near);
        t1 = t1.min(far);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// `segment_hits_box` against every box: does any cover lie on the line?
#[must_use]
pub fn segment_hits_cover(a: [f32; 3], b: [f32; 3], obstacles: &[Obstacle]) -> bool {
    obstacles.iter().any(|o| segment_hits_box(a, b, o))
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

/// What a cover box IS.
///
/// Cosmetic to the sim - every kind blocks, supports and stops rounds by the
/// same three numbers - and load-bearing to the client, which draws a
/// container, a crate and a sandbag as three different things. Carried on
/// the obstacle rather than derived from its height because the authored
/// arena has 1.1-tall sandbags and 0.7-tall rubble that no height rule could
/// tell apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Cover {
    /// A closed shipping container: hard cover you cannot climb from the
    /// floor. The default, so a pre-kind level decodes to the safe reading
    /// (nothing is jumpable that was not meant to be).
    #[default]
    Container,
    /// A wooden crate: jumpable, fought from.
    Crate,
    /// An ammunition box: the lowest step of a climbing chain.
    Ammo,
    /// A sandbag line: waist-high spawn cover.
    Sandbag,
    /// A trench wall: hard cover from the floor, seen over - and mounted,
    /// one hop - from a fire step.
    Wall,
    /// A tunnel roof: the one kind that is expected to have a `base` above
    /// the floor, so you walk under it and stand on it.
    Roof,
    /// Low rubble.
    Rubble,
    /// The statue's plinth.
    Plinth,
    /// A loot block (v18): a one-metre box hung above head height that
    /// hands out a gun to whoever bonks it from below. To the four rules
    /// that read `base` it is an ordinary solid box, walked under, stood on
    /// and stopping rounds; only the ceiling clamp treats it specially, by
    /// reporting which box it hit. Appended last so every existing match
    /// grows one arm and the decode default stays `Container`.
    Loot,
}

/// Axis-aligned obstacle on the XZ plane, with a top and a bottom.
///
/// `h` used to be derived on demand by hashing `min` — which meant a box
/// could not *state* how tall it was, and an authored one had no way into
/// the sim. It is now carried. Seeded arenas fill it with exactly the old
/// hash, so nothing observable changed when it moved.
///
/// `base` arrived with the authored arena (v13): a roofed trench section is
/// a box that starts above the floor, walkable underneath and standable on
/// top. Every box that existed before has `base == 0`, and every rule below
/// is written so that a zero base changes nothing about it. Both new fields
/// default on decode, so a level written before they existed still loads.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Obstacle {
    pub min: [f32; 2],
    pub max: [f32; 2],
    /// Top of the box, measured from the floor at 0 (NOT from `base`).
    pub h: f32,
    /// Bottom of the box; 0 is on the floor. Must stay below `h`.
    #[serde(default)]
    pub base: f32,
    #[serde(default)]
    pub kind: Cover,
}

impl Obstacle {
    /// A box at seeded-arena proportions: the height the generator would
    /// have derived for this footprint, on the floor, classed by that
    /// height exactly as the client has always drawn it.
    #[must_use]
    pub fn seeded(min: [f32; 2], max: [f32; 2]) -> Self {
        let h = seeded_height(min);
        Self {
            min,
            max,
            h,
            base: 0.0,
            kind: if h < CONTAINER_MIN_H {
                Cover::Crate
            } else {
                Cover::Container
            },
        }
    }

    /// An authored box: bottom at `base`, top at `h`.
    #[must_use]
    pub const fn boxed(kind: Cover, min: [f32; 2], max: [f32; 2], base: f32, h: f32) -> Self {
        Self {
            min,
            max,
            h,
            base,
            kind,
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

/// The surface a player at `pos` with feet at `y` stands on: the tallest
/// overlapping box whose bottom is at or below their feet, or the floor.
///
/// This uses the same overlap test as `blocked`, so support always comes from
/// a box the player is actually on. The feet height is what makes a roof a
/// roof: a player walking through a tunnel at `y = 0` is on the floor, not
/// on the box 2.5 m above their head, and a player dropped onto that box
/// from above is on it. `base <= y + 1e-3` - the same slack the landing
/// check uses - so a player resting exactly on a box they cannot see under
/// is still counted as on it. For `base == 0` the clause is always true.
#[must_use]
pub fn support_height(pos: [f32; 2], r: f32, y: f32, obstacles: &[Obstacle]) -> f32 {
    let mut h = 0.0f32;
    for o in obstacles {
        if o.base <= y + 1e-3 && overlaps(pos, r, o) {
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

/// Where player `slot` starts given a level's spawn list: wrapping if an
/// authored level supplies fewer spawns than players, and the seeded ring
/// when it carries none, so a half-authored level still runs. Shared by
/// `Level::spawn` and by the sim's own placement so the two cannot drift.
fn spawn_from(spawns: &[[f32; 2]], slot: u32) -> [f32; 2] {
    if spawns.is_empty() {
        return spawn_point(slot);
    }
    spawns[slot as usize % spawns.len()]
}

/// A client-only prop the level lists so every client draws the same city.
///
/// Nothing in the sim reads these; they are here because the level is the
/// one document both peers agree on, and a statue that only some clients
/// draw is a landmark that only some players can navigate by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DecorKind {
    Statue,
    Cathedral,
    FacadeA,
    FacadeB,
    Lamp,
    Wreck,
}

/// One placed decor prop.
///
/// `pos` is the prop's feet (its base point) in world units, `(x, y, z)`.
/// `yaw` is radians about +Y, with 0 facing +Z and the facing direction
/// `(sin yaw, 0, cos yaw)` - which is what `Quat::from_rotation_y(yaw)`
/// does to +Z - so a prop on the ring at angle `phi` faces the centre at
/// `yaw = phi + PI`. `scale` is the prop's target HEIGHT in world units,
/// never a multiplier: the meshes are generated at arbitrary size and the
/// client fits each one to this.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Decor {
    pub kind: DecorKind,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub scale: f32,
}

/// The name `GameJoined.map` carries for the authored v13 arena.
///
/// Any other value - including the empty string an older frame decodes to -
/// names the seeded arena, so a future map is an additive string rather than
/// a bump.
pub const MAP_TRENCH_CITY: &str = "trench-city";
/// The name of the authored v18 arena, `Level::freight_yard()` in
/// `freight_yard.rs`, and what an empty `CreateLobby.map` resolves to.
pub const MAP_FREIGHT_YARD: &str = "freight-yard";

/// A whole arena as data: what the editor authors and what the sim is
/// handed.
///
/// The server builds each lobby from one, and the client rebuilds the same
/// one from the map name in `GameJoined`, so both resolve movement against
/// identical cover.
///
/// Spawns are carried rather than derived because an authored arena
/// decides where players start; a seeded one reproduces the golden-angle
/// ring the sim has always used. Pads are carried for the same reason.
/// `pads` and `decor` default on decode so a level written before they
/// existed still loads (and plays without pads, as its author left it).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Level {
    pub arena_half: f32,
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<[f32; 2]>,
    #[serde(default)]
    pub pads: Vec<[f32; 2]>,
    #[serde(default)]
    pub decor: Vec<Decor>,
}

/// One authored box before rotation: kind, the two XZ corners, base, top.
type AuthoredBox = (Cover, [f32; 2], [f32; 2], f32, f32);

/// Trench City, the part that is not rotated: the statue's granite base.
/// Not reachable from the floor (jump apex 1.69, top 2.2), reachable from
/// the sandbags around it (1.1 + 1.69). King-of-the-hill.
const TRENCH_CENTRE: &[AuthoredBox] = &[(Cover::Plinth, [-1.6, -1.6], [1.6, 1.6], 0.0, 2.2)];

/// Trench City, the north side; the level rotates it by 90 degrees three
/// times. Transcribed from `docs/plans/arena-v13-trench-city.md` section
/// 4.2, in its order: inner square, trench ring, outer flank.
///
/// The trench corridor is z 11.0..14.0 between the walls; the roof over its
/// middle 12 m is the one box with a bottom above the floor, and the pad
/// sits under it so the tunnels are contested. The three 2.6-tall containers
/// each have a crate within 0.5 of a face and an ammo box within 0.5 of the
/// crate: a climbing chain, floor -> 0.55 -> 1.2 -> 2.6, each step under the
/// jump apex. The 5.2-tall stacked pair is a landmark nobody can climb.
const TRENCH_NORTH: &[AuthoredBox] = &[
    // Inner square dressing.
    (Cover::Sandbag, [-2.0, 4.6], [2.0, 5.4], 0.0, 1.1),
    (Cover::Rubble, [-8.0, 7.6], [-6.0, 9.6], 0.0, 0.7),
    // Trench ring: inner wall, west and east, with the tunnel mouth between.
    (Cover::Wall, [-11.0, 10.6], [-1.2, 11.0], 0.0, 2.5),
    (Cover::Wall, [1.2, 10.6], [11.0, 11.0], 0.0, 2.5),
    // Outer wall in three segments; the gaps at |x| 4.8..7.2 are the entrances.
    (Cover::Wall, [-14.4, 14.0], [-7.2, 14.4], 0.0, 2.5),
    (Cover::Wall, [-4.8, 14.0], [4.8, 14.4], 0.0, 2.5),
    (Cover::Wall, [7.2, 14.0], [14.4, 14.4], 0.0, 2.5),
    // The tunnel: 12 m long, clearance 2.5 over a 1.86 standing body.
    (Cover::Roof, [-6.0, 11.0], [6.0, 14.0], 2.5, 2.9),
    // Fire steps against the outer wall.
    (Cover::Crate, [-9.6, 12.8], [-8.4, 14.0], 0.0, 1.2),
    (Cover::Crate, [8.4, 12.8], [9.6, 14.0], 0.0, 1.2),
    // Low cover inside the tunnel, staggered inner/outer.
    (Cover::Ammo, [-3.4, 11.0], [-2.4, 11.7], 0.0, 0.55),
    (Cover::Ammo, [2.4, 13.3], [3.4, 14.0], 0.0, 0.55),
    // Outer flank: the container that blocks the spawn-to-tunnel sightline
    // and its chain.
    (Cover::Container, [-3.0, 19.2], [3.0, 21.6], 0.0, 2.6),
    (Cover::Crate, [3.4, 19.6], [4.8, 21.0], 0.0, 1.2),
    (Cover::Ammo, [5.2, 19.8], [6.2, 20.6], 0.0, 0.55),
    // The corner container along x, and its chain. Its near corner sits on
    // the diagonal between this side's east spawn and the next side's
    // north spawn (x + z = 30), which is what stops those two seeing each
    // other past the wall corner; the flank still connects around its
    // east end.
    (Cover::Container, [14.8, 15.0], [20.8, 17.4], 0.0, 2.6),
    (Cover::Crate, [13.2, 15.4], [14.6, 16.8], 0.0, 1.2),
    (Cover::Ammo, [11.8, 15.6], [12.8, 16.4], 0.0, 0.55),
    // The stacked pair: corner landmark, unreachable.
    (Cover::Container, [17.0, 20.4], [23.0, 22.8], 0.0, 5.2),
    // Spawn cover.
    (Cover::Sandbag, [-11.0, 17.6], [-7.0, 18.4], 0.0, 1.1),
    (Cover::Sandbag, [7.0, 17.6], [11.0, 18.4], 0.0, 1.1),
];
/// North-side spawns; rotated they make eight, two per side, each 2.6 from
/// the nearest box edge and at least 16 from each other.
const TRENCH_NORTH_SPAWNS: [[f32; 2]; 2] = [[-9.0, 21.0], [9.0, 21.0]];
/// The north pad, inside the tunnel.
const TRENCH_NORTH_PAD: [f32; 2] = [0.0, 12.5];

/// A quarter turn about the origin, `(x, z) -> (-z, x)`.
const fn rot90_point(p: [f32; 2]) -> [f32; 2] {
    [-p[1], p[0]]
}

/// A quarter turn of a box. Both corners are mapped and min/max re-derived:
/// the rotation negates an axis, which swaps min and max on it, so mapping
/// the corners componentwise emits a box with `min > max` (the mistake
/// `docs/asset-pipeline.md` warns about).
const fn rot90_box(o: &Obstacle) -> Obstacle {
    let a = rot90_point(o.min);
    let b = rot90_point(o.max);
    Obstacle {
        min: [a[0].min(b[0]), a[1].min(b[1])],
        max: [a[0].max(b[0]), a[1].max(b[1])],
        ..*o
    }
}

impl Level {
    /// The arena a seed has always produced — same obstacles, same
    /// heights, same pads, same spawn ring - bit for bit, so every test
    /// written against the seeded arena keeps its world.
    #[must_use]
    // `MAX_PLAYERS` is eight, so this conversion cannot truncate in supported builds.
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_seed(seed: u64) -> Self {
        Self {
            arena_half: ARENA_HALF,
            obstacles: generate_arena(seed),
            spawns: (0..MAX_PLAYERS as u32).map(spawn_point).collect(),
            pads: generate_pads(seed),
            decor: Vec::new(),
        }
    }

    /// The authored v13 arena: `docs/plans/arena-v13-trench-city.md`
    /// section 4, tables transcribed above, four-fold symmetric about the
    /// origin. Spawns are listed one per side first (slots 0..4 land on
    /// four different sides) and then the second of each side.
    #[must_use]
    pub fn trench_city() -> Self {
        let mut obstacles: Vec<Obstacle> = TRENCH_CENTRE
            .iter()
            .map(|&(kind, min, max, base, h)| Obstacle::boxed(kind, min, max, base, h))
            .collect();
        let mut pads = Vec::with_capacity(4);
        for turns in 0..4 {
            for &(kind, min, max, base, h) in TRENCH_NORTH {
                let mut o = Obstacle::boxed(kind, min, max, base, h);
                for _ in 0..turns {
                    o = rot90_box(&o);
                }
                obstacles.push(o);
            }
            let mut pad = TRENCH_NORTH_PAD;
            for _ in 0..turns {
                pad = rot90_point(pad);
            }
            pads.push(pad);
        }
        let mut spawns = Vec::with_capacity(8);
        for spawn in TRENCH_NORTH_SPAWNS {
            for turns in 0..4 {
                let mut s = spawn;
                for _ in 0..turns {
                    s = rot90_point(s);
                }
                spawns.push(s);
            }
        }
        Self {
            arena_half: ARENA_HALF,
            obstacles,
            spawns,
            pads,
            decor: trench_city_decor(),
        }
    }

    /// The level a `GameJoined` names: `MAP_TRENCH_CITY` and
    /// `MAP_FREIGHT_YARD` are the authored arenas, anything else (including
    /// the empty string an older frame decodes to) is the seeded one.
    #[must_use]
    pub fn named(map: &str, seed: u64) -> Self {
        if map == MAP_TRENCH_CITY {
            Self::trench_city()
        } else if map == MAP_FREIGHT_YARD {
            Self::freight_yard()
        } else {
            Self::from_seed(seed)
        }
    }

    /// Where player `slot` starts, wrapping if an authored level supplies
    /// fewer spawns than players. Falls back to the seeded ring when a
    /// level carries none, so a half-authored level still runs.
    #[must_use]
    pub fn spawn(&self, slot: u32) -> [f32; 2] {
        spawn_from(&self.spawns, slot)
    }
}

/// Trench City's decor, `docs/plans/arena-v13-trench-city.md` section 4.3.
///
/// The façade ring has twelve 30-degree slots and the cathedral's slot
/// (180 degrees, behind the south wall) is left empty, so eleven façades
/// ship, alternating A and B; the spec's "6 and 6" does not fit twelve
/// slots minus one and this is the reading that keeps the spacing.
///
/// The wreck's `scale` is a height: the spec gives its LENGTH as ~4.4, and
/// a car that long is about 1.5 tall, which is what the client is asked to
/// fit it to. `sin_cos` here is fine for determinism: decor is client-only
/// and never reaches the sim.
fn trench_city_decor() -> Vec<Decor> {
    use std::f32::consts::{FRAC_PI_4, PI, TAU};
    const RING_R: f32 = 44.0;
    const CATHEDRAL_SLOT: u8 = 6;
    let mut decor = vec![
        Decor {
            kind: DecorKind::Statue,
            pos: [0.0, 2.2, 0.0],
            yaw: 0.0,
            scale: 4.0,
        },
        Decor {
            kind: DecorKind::Cathedral,
            pos: [0.0, 0.0, -46.0],
            yaw: 0.0,
            scale: 34.0,
        },
    ];
    for slot in 0..12u8 {
        if slot == CATHEDRAL_SLOT {
            continue;
        }
        let phi = f32::from(slot) * (TAU / 12.0);
        let (s, c) = phi.sin_cos();
        decor.push(Decor {
            kind: if slot.is_multiple_of(2) {
                DecorKind::FacadeA
            } else {
                DecorKind::FacadeB
            },
            pos: [RING_R * s, 0.0, RING_R * c],
            yaw: (phi + PI) % TAU,
            scale: 18.0,
        });
    }
    for &(x, z) in &[(26.0, 12.0), (12.0, 26.0)] {
        for &(sx, sz) in &[(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            decor.push(Decor {
                kind: DecorKind::Lamp,
                pos: [x * sx, 0.0, z * sz],
                yaw: 0.0,
                scale: 5.0,
            });
        }
    }
    for &(sx, sz) in &[(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        decor.push(Decor {
            kind: DecorKind::Wreck,
            pos: [27.0 * sx, 0.0, 27.0 * sz],
            yaw: FRAC_PI_4,
            scale: 1.5,
        });
    }
    decor
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

/// Put weapon `id` in a player's hands with a full magazine and its table
/// reserve. The old weapon's remaining rounds are discarded: there is no
/// inventory. The fifth of a second of cooldown is the "new gun" beat in
/// which the client plays the pop-out.
const fn grant(p: &mut PlayerSt, id: u8) {
    let stats = weapon_stats(id);
    p.weapon = id;
    p.ammo = stats.mag;
    p.reserve = stats.reserve;
    p.fired = 0;
    p.reload_t = 0.0;
    p.cooldown = 0.2;
}

/// Is this spot blocked for a player whose feet are at `y`? The arena wall
/// always blocks; a box only blocks while the player's feet are below its
/// top (by more than a step), so you can walk across boxes you have jumped
/// onto - AND while their head is above its bottom, so you can walk under a
/// box that starts above you. The head is always the standing one: crouch
/// is cosmetic to movement here as everywhere, and a rule that let a
/// crouched player under a lower roof would need `move_circle` to know the
/// stance on both peers. For `base == 0` the head clause is always true.
fn blocked(pos: [f32; 2], y: f32, r: f32, obstacles: &[Obstacle]) -> bool {
    if pos[0].abs() > ARENA_HALF - r || pos[1].abs() > ARENA_HALF - r {
        return true;
    }
    obstacles.iter().any(|o| {
        overlaps(pos, r, o) && y < obstacle_height(o) - STEP_UP && y + BODY_H_STAND > o.base
    })
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

/// What one vertical step produced.
///
/// A struct rather than a tuple so that adding `bonked` made every caller
/// (the sim, the client's prediction and its reconciliation replay) fail to
/// compile until it read the new shape, which is the point of a shared-code
/// signature change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VStep {
    /// Feet height after the step.
    pub y: f32,
    /// Vertical speed after the step.
    pub vy: f32,
    pub grounded: bool,
    /// The index into `obstacles` of the box whose bottom clamped the head
    /// this step, when one did. The client predicts the bump from it and
    /// the server pays out a loot block from it.
    pub bonked: Option<usize>,
}

/// Vertical motion for one step: gravity, jump take-off, and landing on
/// whatever surface is under the player. Shared VERBATIM by the server sim
/// and the client's prediction, exactly like `move_circle`.
#[must_use]
pub fn step_vertical(
    pos: [f32; 2],
    y: f32,
    vy: f32,
    jump: bool,
    dt: f32,
    obstacles: &[Obstacle],
) -> VStep {
    let ground = support_height(pos, PLAYER_R, y, obstacles);
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
    let next = y + vy * dt;
    let (mut next, mut vy, landed) = if vy <= 0.0 && next <= ground {
        (ground, 0.0, true)
    } else {
        (next, vy, false)
    };
    // Ceiling: a box whose bottom is above the feet stops the head. Judged
    // on where the feet WERE (below the bottom) and where the head would END
    // (above it), so a player standing on a roof is never clamped by the
    // roof they stand on. A jump inside a tunnel bonks; a player on a
    // container cannot jump up into the underside of anything. The clamp
    // never goes below the surface underfoot: a box hung lower than a
    // standing body over a player (which `blocked` never lets them walk
    // into) leaves them standing where they are rather than pushed into the
    // floor and oscillating.
    //
    // Of the boxes that clamp, the one with the LOWEST bottom wins and is
    // reported: the head meets it first. No shipped level has two raised
    // boxes over one footprint (`no_two_raised_boxes_share_a_footprint`
    // keeps it that way), so for every existing level this is the one box
    // the old loop assigned, bit for bit.
    let mut bonked: Option<(usize, f32)> = None;
    for (k, o) in obstacles.iter().enumerate() {
        if y < o.base
            && next + BODY_H_STAND > o.base
            && overlaps(pos, PLAYER_R, o)
            && bonked.is_none_or(|(_, base)| o.base < base)
        {
            bonked = Some((k, o.base));
        }
    }
    if let Some((_, base)) = bonked {
        next = (base - BODY_H_STAND).max(ground);
        vy = 0.0;
    }
    VStep {
        y: next,
        vy,
        grounded: bonked.is_none() && (landed || (grounded && vy <= 0.0)),
        bonked: bonked.map(|(k, _)| k),
    }
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
    /// A jump PRESS, consumed on the tick it is applied (arena-server clears
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
    /// Aiming down the sights (RMB or LT). HELD, like `shield`: it scales
    /// the spread cone of the round fired this tick and nothing else.
    pub ads: bool,
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
    /// A weapon id, `1..=WEAPON_COUNT`; `SIDEARM` on spawn and on death.
    pub weapon: u8,
    pub ammo: u8,
    /// Rounds outside the magazine; `RESERVE_INFINITE` for the sidearm.
    /// When both this and `ammo` reach zero the gun is gone and the sidearm
    /// is back, which is what makes a looted gun a loan.
    pub reserve: u8,
    /// Rounds fired since the magazine was last filled: the bloom's input.
    /// Counted rather than derived from `mag - ammo` because a reserve-short
    /// refill fills the magazine only partly, and the difference would then
    /// read a fresh magazine as twenty rounds into a spray. Server-only: the
    /// wire carries `ammo`, never this, and it resets on every event that
    /// fills the magazine (reload, grant, respawn, the dry swap).
    fired: u8,
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

#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// The table row this round was fired from. Gravity, kind, radius and
    /// splash are looked up through `weapon_stats(weapon)` so the struct
    /// stays small and the table stays the one source.
    pub weapon: u8,
    /// Bodies this round may still pass through; `stats.pierce` at launch.
    pub pierce: u8,
    /// A bit per player id (ids are `0..8` by the server's `alloc_pid`)
    /// already hit by this round, so a pierced body is never hit twice.
    pub hit_mask: u8,
}

/// The bit of a player id in `Bullet.hit_mask`.
///
/// Ids are `0..8` by the server's `alloc_pid` and the eight-player cap, so
/// one bit each fits a byte. An id past the byte gets no bit rather than a
/// panic on the shift: such a player can only exist in a hand-built test
/// sim, and the cost there is that a pierced body could be hit twice, never
/// a crash in the sweep.
const fn id_bit(id: u8) -> u8 {
    if id < 8 { 1 << id } else { 0 }
}

/// One round leaving the muzzle: the pure function behind the trigger.
///
/// Pure so a test can hand it any table row, and because everything that
/// decides where the round goes is an argument: the shooter's state, the
/// row, whether the sights are up, whether the feet are planted, and the
/// (seed, tick) the cone is rolled from. The caller charges the cooldown,
/// the magazine and the shooter's `fired` count; `fired` is read here BEFORE
/// that increment, so the first round after a reload is round zero of the
/// bloom (the Vityaz starts tight and opens up; the AK's climb reads as the
/// cone). It is the count, not `mag - ammo`: a reserve-short refill leaves
/// the magazine part-full, and the difference would open the first round of
/// a fresh magazine to the cap.
///
/// When the effective cone is zero the aim is used exactly as it was before
/// v18 and no rotation happens at all: the sidearm's ray is bit-identical
/// to v17, which is what keeps every pre-v18 shot test's world untouched. A
/// non-zero cone is a uniform draw over the disc (`sqrt` on the radius),
/// rolled from `roll(seed, tick, id, 0)` with no RNG state anywhere. The
/// `sin_cos` and `tan` here are transcendentals in the launch, sound for the
/// reason the `tan` always was: bullets are stepped server-side only.
// Eight arguments because each one is an independent input of the round and
// bundling them into a struct would only hide the list the doc above names.
#[allow(clippy::too_many_arguments)]
pub fn launch(
    p: &PlayerSt,
    stats: &WeaponStats,
    ads: bool,
    grounded: bool,
    seed: u64,
    tick: u64,
    delay: u16,
    out: &mut Vec<Bullet>,
) {
    let fired = f32::from(p.fired);
    let cone = (stats.spread + stats.bloom * fired).min(stats.spread_max)
        * if ads { stats.ads_spread } else { 1.0 }
        * if grounded { 1.0 } else { ADS_SPREAD_AIR_MULT };
    let (aim, pitch) = if cone == 0.0 {
        (p.aim, p.pitch)
    } else {
        let (disc, turn) = unit_pair(roll(seed, tick, p.id, 0));
        let radius = cone * disc.sqrt();
        let theta = std::f32::consts::TAU * turn;
        let (yaw_off, pitch_off) = (radius * theta.cos(), radius * theta.sin());
        let (sin_yaw, cos_yaw) = yaw_off.sin_cos();
        (
            [
                p.aim[0] * cos_yaw - p.aim[1] * sin_yaw,
                p.aim[0] * sin_yaw + p.aim[1] * cos_yaw,
            ],
            (p.pitch + pitch_off).clamp(-MAX_PITCH, MAX_PITCH),
        )
    };
    // Spawn just in front of the centre, along the round's own line: the
    // swept collision skips the owner, and starting further out would
    // leave a point-blank dead zone.
    let muzzle = [p.pos[0] + aim[0] * 0.2, p.pos[1] + aim[1] * 0.2];
    out.push(Bullet {
        pos: muzzle,
        vel: [aim[0] * stats.speed, aim[1] * stats.speed],
        // Leaves at eye level and climbs or falls at the tangent of the aim
        // elevation, which makes the ray exactly the shooter's look
        // direction.
        y: p.y + eye_h(p.crouch),
        vy: pitch.tan() * stats.speed,
        ttl: stats.ttl,
        owner: p.id,
        dmg: stats.damage,
        delay: delay.min(MAX_REWIND_TICKS),
        weapon: p.weapon,
        pierce: stats.pierce,
        hit_mask: 0,
    });
}

/// One tick's snapshot per player (id, pos, feet y, alive, crouch) for
/// lag-compensated rewinds — stance AND height rewind with position, so a
/// shot at a target who was then standing on a crate uses the standing
/// hitbox at the crate's height, even if they have since crouched or
/// dropped off it. Without the height the vertical band below would test a
/// current position against a rewound one and miss for the wrong reason.
type HistoryFrame = Vec<(u8, [f32; 2], f32, bool, bool)>;

/// One loot block's state: which obstacle it is, and whether it is armed.
#[derive(Clone, Debug)]
pub struct LootBlock {
    /// Index into `Sim.obstacles` of the `Cover::Loot` box.
    pub obstacle: usize,
    /// 0 = armed; counting down after a bonk.
    pub respawn_t: f32,
}

pub struct Sim {
    pub obstacles: Vec<Obstacle>,
    pub pads: Vec<Pad>,
    /// One per `Cover::Loot` obstacle, in obstacle order, so `State.loot`
    /// is index-aligned with the blocks every client derives from the level.
    pub loot: Vec<LootBlock>,
    /// The level's spawn points; empty means the seeded golden-angle ring.
    /// Placement goes through `Level::spawn` semantics for both the first
    /// spawn and every respawn.
    pub spawns: Vec<[f32; 2]>,
    pub players: Vec<PlayerSt>,
    pub bullets: Vec<Bullet>,
    /// (killer, victim) pairs from the last step.
    pub events: Vec<(u8, u8)>,
    /// (shooter, victim, damage, head) for every hit the last step applied,
    /// from the damage loop, so it is authoritative: it replaces the
    /// client's "my bullet vanished and someone lost hp" guess, which
    /// pierce would break and which already fires falsely on a reflected
    /// round.
    pub hits: Vec<(u8, u8, u8, bool)>,
    /// (position, owner) of every rocket that detonated the last step.
    pub blasts: Vec<([f32; 3], u8)>,
    /// (player, block index into `loot`, weapon) for every block paid out
    /// the last step.
    pub loot_events: Vec<(u8, u8, u8)>,
    /// The lobby's seed: what the world was built from, and the one input
    /// besides the tick and the player that every roll hashes.
    pub seed: u64,
    /// Ticks stepped since creation; State broadcasts echo it and clients
    /// report it back as their view tick.
    pub tick: u64,
    history: std::collections::VecDeque<HistoryFrame>,
}

impl Sim {
    /// The seeded arena: exactly `from_level(&Level::from_seed(seed), seed)`,
    /// kept as the short spelling every existing test uses.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::from_level(&Level::from_seed(seed), seed)
    }

    /// A sim on an authored level: its obstacles, its pads (all active), its
    /// loot blocks (all armed), its spawns. `arena_half` is NOT read - the
    /// wall is `ARENA_HALF` in the shared `blocked`, and making it per-level
    /// is a `move_circle` signature change on both peers that eight players
    /// did not need. `seed` is the lobby's, which the server already mints
    /// and sends in `GameJoined`; every spread and loot roll hashes it.
    #[must_use]
    pub fn from_level(level: &Level, seed: u64) -> Self {
        Self {
            obstacles: level.obstacles.clone(),
            pads: level
                .pads
                .iter()
                .map(|&pos| Pad {
                    pos,
                    respawn_t: 0.0,
                })
                .collect(),
            loot: level
                .obstacles
                .iter()
                .enumerate()
                .filter(|(_, o)| o.kind == Cover::Loot)
                .map(|(obstacle, _)| LootBlock {
                    obstacle,
                    respawn_t: 0.0,
                })
                .collect(),
            spawns: level.spawns.clone(),
            players: Vec::new(),
            bullets: Vec::new(),
            events: Vec::new(),
            hits: Vec::new(),
            blasts: Vec::new(),
            loot_events: Vec::new(),
            seed,
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
            pos: spawn_from(&self.spawns, slot),
            y: 0.0,
            vy: 0.0,
            aim: [1.0, 0.0],
            pitch: 0.0,
            hp: MAX_HP,
            score: 0,
            alive: true,
            crouch: false,
            shield: false,
            weapon: SIDEARM,
            ammo: weapon_stats(SIDEARM).mag,
            reserve: RESERVE_INFINITE,
            fired: 0,
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

    /// A rocket going off at `blast`.
    ///
    /// `direct` is the body it struck, which takes the round's own damage
    /// and is left out of the splash; `spare` is a body excluded from the
    /// splash without being hit (the holder of the plate it went off on).
    /// Everyone else alive, the owner included, is judged by the distance
    /// from the blast to the edge of their hit circle: two points inside
    /// half the splash radius, one out to the edge, nothing beyond. The
    /// blast must lie within the radius of the body's height band, and the
    /// body's chest must have line of sight to it through `segment_hits_box`
    /// rather than the sweep's conservative span test, so a target on a
    /// crate whose chest is above the crate top is clear and a target
    /// crouched behind rubble with the blast at floor level is not.
    ///
    /// Bodies are REWOUND by the round's delay, with the same `rewound` the
    /// sweep uses: an explosion is the shooter's aim, not the defender's
    /// decision, so it is judged where the shooter saw the bodies. The
    /// blast is pushed lifted to 0.05 so a floor hit is drawn above the
    /// ground; the damage above used the true point.
    // Eight arguments including self: the round, where it went off, the two
    // exclusions and the two output lists. A struct would hide the list.
    #[allow(clippy::too_many_arguments)]
    fn detonate(
        &self,
        obstacles: &[Obstacle],
        b: &Bullet,
        blast: [f32; 3],
        direct: Option<u8>,
        spare: Option<u8>,
        hits: &mut Vec<(u8, u8, u8, bool)>,
        blasts: &mut Vec<([f32; 3], u8)>,
    ) {
        let stats = weapon_stats(b.weapon);
        if let Some(victim) = direct {
            hits.push((b.owner, victim, b.dmg, false));
        }
        for q in &self.players {
            if Some(q.id) == direct || Some(q.id) == spare {
                continue;
            }
            let (qpos, qy, qalive, qcrouch) = self
                .rewound(q.id, b.delay)
                .unwrap_or((q.pos, q.y, q.alive, q.crouch));
            if !qalive || !q.alive {
                continue;
            }
            let (dx, dz) = (qpos[0] - blast[0], qpos[1] - blast[2]);
            let d = ((dx * dx + dz * dz).sqrt() - hit_radius(qcrouch)).max(0.0);
            let dmg = if d <= stats.splash_r * 0.5 {
                2
            } else if d <= stats.splash_r {
                1
            } else {
                continue;
            };
            if blast[1] < qy - stats.splash_r || blast[1] > qy + body_h(qcrouch) + stats.splash_r {
                continue;
            }
            let chest = [qpos[0], qy + 0.9, qpos[1]];
            if segment_hits_cover(blast, chest, obstacles) {
                continue;
            }
            hits.push((b.owner, q.id, dmg, false));
        }
        blasts.push(([blast[0], blast[1].max(0.05), blast[2]], b.owner));
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
        self.hits.clear();
        self.blasts.clear();
        self.loot_events.clear();
        self.tick += 1;
        let dt = FIXED_DT;
        // What every roll this tick hashes: the lobby's seed and this tick.
        let (roll_seed, roll_tick) = (self.seed, self.tick);

        // Players: respawn timers, movement (axis-separated slide), firing.
        let mut new_bullets: Vec<Bullet> = Vec::new();
        // (player index, obstacle index) of every head that met a loot
        // block's bottom this tick, resolved after the loop in player order.
        let mut bonks: Vec<(usize, usize)> = Vec::new();
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
                    p.pos = spawn_from(
                        &self.spawns,
                        p.deaths.wrapping_mul(3).wrapping_add(u32::from(p.id)),
                    );
                    p.y = 0.0;
                    p.vy = 0.0;
                    p.hp = MAX_HP;
                    p.alive = true;
                    p.cooldown = 0.3;
                    // Death costs your loot: the sidearm is back.
                    p.weapon = SIDEARM;
                    p.ammo = weapon_stats(SIDEARM).mag;
                    p.reserve = RESERVE_INFINITE;
                    p.fired = 0;
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
            let v = step_vertical(
                pos,
                feet_height,
                vertical_speed,
                input.jump,
                dt,
                &self.obstacles,
            );
            // A bonk is the clamp firing on the way UP into a loot block.
            // Requiring the pre-step `vy > 0` is belt and braces: the clamp
            // can only fire on the way up, because `blocked` stops a body
            // walking into a box its head reaches.
            if let Some(k) = v.bonked
                && vertical_speed > 0.0
                && self.obstacles[k].kind == Cover::Loot
            {
                bonks.push((i, k));
            }
            let p = &mut self.players[i];
            p.pos = pos;
            p.y = v.y;
            p.vy = v.vy;
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
                    // The magazine fills from the reserve, and the reserve
                    // pays for it unless it is the sidearm's bottomless one.
                    // Saturating, so a weapon set by hand with more rounds
                    // than its magazine holds is left alone rather than
                    // wrapped.
                    let short = stats.mag.saturating_sub(p.ammo);
                    let take = if p.reserve == RESERVE_INFINITE {
                        short
                    } else {
                        short.min(p.reserve)
                    };
                    p.ammo += take;
                    if p.reserve != RESERVE_INFINITE {
                        p.reserve -= take;
                    }
                    // A short refill is still a fresh magazine to the bloom.
                    p.fired = 0;
                }
            } else if p.ammo == 0 && p.reserve == 0 {
                // Dry: the loot gun is gone and the sidearm is back, this
                // tick. A quarter second before it fires.
                p.weapon = SIDEARM;
                p.ammo = weapon_stats(SIDEARM).mag;
                p.reserve = RESERVE_INFINITE;
                p.fired = 0;
                p.reload_t = 0.0;
                p.cooldown = 0.25;
            } else if (input.reload && p.ammo < stats.mag && p.reserve > 0) || p.ammo == 0 {
                p.reload_t = stats.reload;
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
                    // The round reads `fired` before it is counted, so
                    // the first round after a refill is round zero of the
                    // bloom. `v.grounded` is this tick's own vertical step,
                    // so a jumping spray is judged airborne on the tick it
                    // leaves the ground.
                    launch(
                        p,
                        &stats,
                        input.ads,
                        v.grounded,
                        roll_seed,
                        roll_tick,
                        input.delay_ticks,
                        &mut new_bullets,
                    );
                    p.cooldown = stats.cooldown;
                    p.ammo -= 1;
                    // Saturating because a hand-built test can fire past
                    // any magazine; the cone is capped long before 255.
                    p.fired = p.fired.saturating_add(1);
                }
            }
        }
        self.bullets.extend(new_bullets);

        // Weapon pads: tick respawns, hand out loot on contact. A pad rolls
        // the same table as a block, so there is exactly one reward rule.
        for pad in &mut self.pads {
            if pad.respawn_t > 0.0 {
                pad.respawn_t = (pad.respawn_t - dt).max(0.0);
                continue;
            }
            for p in &mut self.players {
                if !p.alive {
                    continue;
                }
                let (dx, dz) = (p.pos[0] - pad.pos[0], p.pos[1] - pad.pos[1]);
                if p.y < PAD_PICK_H && dx * dx + dz * dz < PAD_RADIUS * PAD_RADIUS {
                    let w = loot_roll(roll_seed, roll_tick, p.id, p.weapon);
                    grant(p, w);
                    pad.respawn_t = PAD_RESPAWN_SECS;
                    break;
                }
            }
        }

        // Loot blocks: tick respawns, then pay out this tick's bonks in
        // player order. Two players bonking one block on one tick pay once:
        // the first arms the timer and the second finds it dark.
        for slot in &mut self.loot {
            if slot.respawn_t > 0.0 {
                slot.respawn_t = (slot.respawn_t - dt).max(0.0);
            }
        }
        for (i, k) in bonks {
            let Some(slot_index) = self.loot.iter().position(|l| l.obstacle == k) else {
                continue;
            };
            let slot = &mut self.loot[slot_index];
            if slot.respawn_t > 0.0 {
                continue;
            }
            let p = &mut self.players[i];
            let w = loot_roll(roll_seed, roll_tick, p.id, p.weapon);
            grant(p, w);
            slot.respawn_t = LOOT_RESPAWN_SECS;
            // Seven blocks on the biggest map: the index fits a byte.
            self.loot_events
                .push((p.id, u8::try_from(slot_index).unwrap_or(u8::MAX), w));
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
        let mut hits: Vec<(u8, u8, u8, bool)> = Vec::new(); // (owner, victim, dmg, head)

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
                hits.push((aid, tid, MAX_HP, false));
            }
        }
        let obstacles = std::mem::take(&mut self.obstacles);
        let mut bullets = std::mem::take(&mut self.bullets);
        let mut blasts: Vec<([f32; 3], u8)> = Vec::new();
        bullets.retain_mut(|b| {
            // The table is the one source: gravity, kind and radius are read
            // through the round's weapon rather than carried on it.
            let stats = weapon_stats(b.weapon);
            let rocket = stats.kind == Projectile::Rocket;
            let radius = stats.radius;
            b.ttl -= dt;
            if b.ttl <= 0.0 {
                // A rocket out of flight time goes off where it is.
                if rocket {
                    self.detonate(
                        &obstacles,
                        b,
                        [b.pos[0], b.y, b.pos[1]],
                        None,
                        None,
                        &mut hits,
                        &mut blasts,
                    );
                }
                return false;
            }
            // Gravity, semi-implicit Euler like the player's: the vertical
            // speed is charged BEFORE the segment is formed. The horizontal
            // `vel` is untouched, so every range-per-tick invariant stands,
            // and a zero-gravity row adds exactly zero, so the sidearm's
            // line is the v17 line bit for bit.
            b.vy += stats.gravity * dt;
            let p0 = b.pos;
            let p1 = [p0[0] + b.vel[0] * dt, p0[1] + b.vel[1] * dt];
            let y0 = b.y;
            let y1 = b.y + b.vy * dt;
            let (sx, sz) = (p1[0] - p0[0], p1[1] - p0[1]);
            let seg_len_sq = sx * sx + sz * sz;
            for p in &self.players {
                // A body this round already went through is never hit
                // twice: a pierced target is overlapped on consecutive
                // ticks, and the mask is what stops the second one.
                if p.id == b.owner || b.hit_mask & id_bit(p.id) != 0 {
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
                let rr = hit_radius(tcrouch) + radius;
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
                let (lo, hi) = (ty - radius, ty + body_h(tcrouch) + radius);
                // The head is the top HEAD_H of the volume. No BULLET_R pad on
                // its underside: that boundary is internal, between head and
                // chest, not a silhouette edge, and padding it outward would
                // quietly make the head bigger than the one being drawn.
                let head_lo = ty + head_lo(tcrouch);
                let travel = (y1 - b.y).abs();
                let by = b.y + (y1 - b.y) * t;
                let mut connected = by >= lo && by <= hi;
                let mut head = connected && by >= head_lo;
                // Where along the tick the round met the body: the closest
                // approach, or the first walked sample that connected when
                // the closest approach did not. The cover test below runs
                // there.
                let mut contact = t;
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
                            if !connected {
                                contact = u;
                            }
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
                    // Cover between the muzzle and the body stops the round
                    // before the body does. This is the world test below
                    // (which judges the tick's END point) run at the point
                    // of CONTACT, with the tick's vertical span up to it. A
                    // round that meets a body from INSIDE a tunnel roof -
                    // climbing at 30 degrees from the tunnel floor at a
                    // player on the roof, whose hit column starts BULLET_R
                    // below their feet, or dropping at MAX_PITCH from the
                    // roof at the player underneath - was passing through
                    // the slab when it connected, and the slab wins. Judged
                    // at the contact and not the end point so a point-blank
                    // hit on a body backed against a wall is still a hit:
                    // the end point is inside the wall, the contact is not.
                    // Conservative like the world test (span, not segment),
                    // for the same reason. Ahead of the shield: cover in
                    // front of a raised plate stops the round before the
                    // plate could send it back.
                    let cy = b.y + (y1 - b.y) * contact;
                    let (cx, cz) = (p0[0] + sx * contact, p0[1] + sz * contact);
                    if obstacles.iter().any(|o| {
                        y0.min(cy) < obstacle_height(o)
                            && y0.max(cy) > o.base
                            && cx > o.min[0] - radius
                            && cx < o.max[0] + radius
                            && cz > o.min[1] - radius
                            && cz < o.max[1] + radius
                    }) {
                        // A rocket stopped by cover goes off at the last
                        // free point, the start of this tick's segment.
                        if rocket {
                            self.detonate(
                                &obstacles,
                                b,
                                [p0[0], y0, p0[1]],
                                None,
                                None,
                                &mut hits,
                                &mut blasts,
                            );
                        }
                        return false;
                    }
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
                        // registration, sound for exactly the reason the
                        // tan() at launch is — bullets are stepped
                        // server-side only.
                        if speed_h > 1e-6 && -dot >= (SHIELD_ARC * 0.5).cos() * speed_h {
                            // A rocket detonates ON the plate and never
                            // comes back: a plate is cover, not a launcher,
                            // and a three-damage rocket bounced by a 120
                            // degree arc would be a free kill on the
                            // shooter with no counterplay. The holder is
                            // spared the splash; the plate took it.
                            if rocket {
                                self.detonate(
                                    &obstacles,
                                    b,
                                    [cx, cy, cz],
                                    None,
                                    Some(p.id),
                                    &mut hits,
                                    &mut blasts,
                                );
                                return false;
                            }
                            // Mirror about the plate: v' = v - 2(v·n)n. The
                            // normal is horizontal, so vy is untouched and a
                            // round arcing down at you comes back arcing
                            // down, keeping its range rather than being
                            // launched at the sky. A mirror is an isometry,
                            // so horizontal speed stays BULLET_SPEED and the
                            // invariant pinned by pitch_does_not_shorten_a_shot
                            // survives reflection. Damage rides along: catch
                            // a revolver round and you send two damage back.
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
                            // A caught round is a fresh round: whatever it
                            // went through on the way in is forgotten, and
                            // it does not go through anyone on the way
                            // back. The mask must clear or the round could
                            // never hit the body it pierced coming in, and
                            // the pierce must clear or a reflected sniper
                            // round would be a two-body reward for the
                            // catcher that the shooter never earned.
                            b.pierce = 0;
                            b.hit_mask = 0;
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
                    // A rocket on a body is a direct hit and a blast at the
                    // point of contact; the body it struck takes the direct
                    // damage and is left out of the splash.
                    if rocket {
                        self.detonate(
                            &obstacles,
                            b,
                            [cx, cy, cz],
                            Some(p.id),
                            None,
                            &mut hits,
                            &mut blasts,
                        );
                        return false;
                    }
                    // A head hit kills outright, whatever the weapon and
                    // whatever the remaining HP. Routed as damage rather than
                    // as a special case so respawn, scoring and the kill event
                    // all stay on the one path.
                    hits.push((b.owner, p.id, if head { MAX_HP } else { b.dmg }, head));
                    if b.pierce == 0 {
                        return false;
                    }
                    // Pierce: the round goes on, remembering this body, and
                    // the loop CONTINUES so a second body on this same
                    // tick's segment is hit too. A `break` here would skip
                    // that body for good, because the segment is formed
                    // once per tick and next tick's starts beyond it.
                    b.pierce -= 1;
                    b.hit_mask |= id_bit(p.id);
                }
            }
            b.pos = p1;
            b.y = y1;
            // Into the floor. A rocket goes off where it crossed it, lifted
            // to 0.05 so the blast is drawn above the ground it hit.
            if b.y < 0.0 {
                if rocket {
                    // y0 >= 0 (a round below the floor was culled last
                    // tick) and y1 < 0, so the crossing parameter is in
                    // [0, 1) and the divisor is never zero.
                    let t = y0 / (y0 - y1);
                    self.detonate(
                        &obstacles,
                        b,
                        [p0[0] + sx * t, 0.05, p0[1] + sz * t],
                        None,
                        None,
                        &mut hits,
                        &mut blasts,
                    );
                }
                return false;
            }
            if b.pos[0].abs() > ARENA_HALF - radius || b.pos[1].abs() > ARENA_HALF - radius {
                // A rocket goes off against the wall, at the position
                // clamped back inside it.
                if rocket {
                    let lim = ARENA_HALF - radius;
                    self.detonate(
                        &obstacles,
                        b,
                        [b.pos[0].clamp(-lim, lim), b.y, b.pos[1].clamp(-lim, lim)],
                        None,
                        None,
                        &mut hits,
                        &mut blasts,
                    );
                }
                return false;
            }
            // Cover now stops only what actually passes THROUGH it, so a
            // shot arcing over a crate from a container top clears it, and
            // a level round travels the length of a tunnel under its roof.
            // Heights used to be hashed with a sin() right here; they are
            // carried on the box now, but the rule that made that sound
            // still holds and still matters: bullets are simulated
            // server-side exclusively — clients never step their own. If
            // client-side shot prediction is ever added, every f32
            // transcendental on the shot's path (the tan() at launch)
            // becomes a desync source.
            // Gated on the tick's vertical SPAN against the box's [base, h],
            // not on its end point: a climbing bullet that enters a crate's
            // footprint below the top and ends the tick above it would
            // otherwise pass straight through the crate's side wall.
            // Conservative — this can only over-block — and a shot arcing
            // down from a container top over a crate still has both
            // endpoints above it. For `base == 0` the lower clause is always
            // true, because a round below the floor was already culled.
            if obstacles.iter().any(|o| {
                y0.min(b.y) < obstacle_height(o)
                    && y0.max(b.y) > o.base
                    && b.pos[0] > o.min[0] - radius
                    && b.pos[0] < o.max[0] + radius
                    && b.pos[1] > o.min[1] - radius
                    && b.pos[1] < o.max[1] + radius
            }) {
                // The last free point is where this tick's segment began.
                if rocket {
                    self.detonate(
                        &obstacles,
                        b,
                        [p0[0], y0, p0[1]],
                        None,
                        None,
                        &mut hits,
                        &mut blasts,
                    );
                }
                return false;
            }
            true
        });
        self.bullets = bullets;
        self.obstacles = obstacles;
        self.blasts = blasts;

        // Apply damage after the bullet pass (avoids double-borrow). Only a
        // hit that lands on a living body is reported, so `hits` is what
        // happened and never what a round merely touched.
        for (owner, victim, dmg, head) in hits {
            let Some(v) = self.players.iter_mut().find(|p| p.id == victim) else {
                continue;
            };
            if !v.alive {
                continue;
            }
            self.hits.push((owner, victim, dmg, head));
            v.hp = v.hp.saturating_sub(dmg);
            if v.hp == 0 {
                v.alive = false;
                v.respawn_in = RESPAWN_SECS;
                v.death_count += 1;
                // The kill event is pushed whoever did it, so a self-kill
                // reads as `Kill { killer: id, victim: id }` on every
                // client; only the score asks whether it was someone else.
                self.events.push((owner, victim));
                if owner != victim
                    && let Some(k) = self.players.iter_mut().find(|p| p.id == owner)
                {
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
    // The v13 level drivers live beside the yard tests so both maps run one driver.
    use crate::freight_yard::level_helpers::{centre, climb, contains, dist, gap, hop};

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
    fn pads_hand_out_the_same_loot() {
        // A pad rolls the same table as a block: the weapon is in the pool,
        // the load is full, the pad cools down for 15 s, and death returns
        // the sidearm. Replaces `pads_upgrade_and_death_resets`, whose
        // pistol -> rapid -> heavy ladder no longer exists.
        let mut sim = Sim::new(8);
        sim.obstacles.clear();
        sim.add_player(0);
        let pad = sim.pads[0].pos;
        sim.players[0].pos = pad;
        let idle = HashMap::new();
        step_with(&mut sim, &idle);
        let p = &sim.players[0];
        assert!(
            LOOT_POOL.contains(&p.weapon),
            "a pad hands out a pool gun, not {}",
            p.weapon
        );
        let stats = weapon_stats(p.weapon);
        assert_eq!(p.ammo, stats.mag, "full magazine");
        assert_eq!(p.reserve, stats.reserve, "the table reserve");
        assert_eq!(p.reload_t, 0.0);
        assert!(
            (sim.pads[0].respawn_t - PAD_RESPAWN_SECS).abs() < 1e-6,
            "pad goes on cooldown for {PAD_RESPAWN_SECS} s"
        );

        // Death returns the sidearm and counts.
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
        assert_eq!(p.weapon, SIDEARM, "death returns the sidearm");
        assert_eq!(p.ammo, weapon_stats(SIDEARM).mag);
        assert_eq!(p.reserve, RESERVE_INFINITE);
        assert_eq!(p.death_count, 1);
    }

    #[test]
    fn the_revolver_kills_in_two_hits() {
        let mut sim = Sim::new(9);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        sim.players[0].weapon = 5;
        sim.players[0].ammo = weapon_stats(5).mag;
        sim.players[0].reserve = weapon_stats(5).reserve;
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
        assert_eq!(sim.events, vec![(0, 1)], "the revolver killed the target");
        assert!(
            hits <= 1,
            "at most one non-lethal hit before the kill (2 dmg per hit)"
        );
        // The killing round is reported from the damage loop: shooter,
        // victim, the two points of body damage, not a head.
        assert_eq!(sim.hits, vec![(0, 1, 2, false)]);
    }

    #[test]
    fn reload_draws_from_the_reserve_and_stops_when_it_is_empty() {
        // Revolver, 6 + 12: two full reloads empty the reserve, and a
        // reload with nothing left does not start. Fired through the sim's
        // own trigger so the count is the economy's, not the test's.
        let mut sim = Sim::new(7);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.players[0].pos = [0.0, 0.0];
        grant(&mut sim.players[0], 5);
        let stats = weapon_stats(5);
        assert_eq!((sim.players[0].ammo, sim.players[0].reserve), (6, 12));
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                fire: true,
                ..Default::default()
            },
        );
        let idle = HashMap::new();
        let empty_the_mag = |sim: &mut Sim| {
            let mut ticks = 0;
            while sim.players[0].ammo > 0 && ticks < 2000 {
                step_with(sim, &inputs);
                ticks += 1;
            }
            // The auto-reload has started on the empty tick; let it finish.
            for _ in 0..((stats.reload / FIXED_DT) as u32 + 3) {
                step_with(sim, &idle);
            }
        };
        empty_the_mag(&mut sim);
        assert_eq!((sim.players[0].ammo, sim.players[0].reserve), (6, 6));
        empty_the_mag(&mut sim);
        assert_eq!((sim.players[0].ammo, sim.players[0].reserve), (6, 0));
        // Six more rounds and the reserve cannot refill the magazine: the
        // dry arm hands the sidearm back instead of starting a reload.
        empty_the_mag(&mut sim);
        let p = &sim.players[0];
        assert_eq!(p.weapon, SIDEARM, "a dry loot gun is gone");
        assert_eq!(p.reserve, RESERVE_INFINITE);
        assert_eq!(p.ammo, weapon_stats(SIDEARM).mag);
        // And the sidearm's reserve is never consumed by its reloads.
        for _ in 0..3 {
            let mut reload = inputs.clone();
            reload.get_mut(&0).unwrap().fire = false;
            reload.get_mut(&0).unwrap().reload = true;
            sim.players[0].ammo = 1;
            for _ in 0..((RELOAD_SECS / FIXED_DT) as u32 + 3) {
                step_with(&mut sim, &reload);
            }
            assert_eq!(sim.players[0].ammo, weapon_stats(SIDEARM).mag);
            assert_eq!(sim.players[0].reserve, RESERVE_INFINITE);
        }
    }

    #[test]
    fn the_weapon_table_is_well_formed() {
        for id in 1..=WEAPON_COUNT {
            let s = weapon_stats(id);
            assert!(s.mag >= 1, "{id}: empty magazine");
            assert!(s.cooldown > 0.0, "{id}: no cooldown");
            assert!(s.speed > 0.0, "{id}: no speed");
            assert!(s.ttl > 0.0, "{id}: no range");
            assert!(s.gravity <= 0.0, "{id}: gravity points up");
            assert!(
                s.spread <= s.spread_max,
                "{id}: base cone wider than its cap"
            );
            // No held trigger is ever throttled by the bullet cap.
            let in_flight = (s.ttl / s.cooldown).ceil() as usize;
            assert!(
                in_flight <= MAX_BULLETS_PER_PLAYER,
                "{id}: {in_flight} rounds in flight exceed the cap"
            );
            match s.kind {
                Projectile::Rocket => {
                    assert!(s.splash_r > 0.0, "{id}: a rocket without splash");
                    assert_eq!(s.pierce, 0, "{id}: a rocket never pierces");
                }
                Projectile::Bullet => assert_eq!(s.splash_r, 0.0, "{id}: a bullet with splash"),
            }
            assert_eq!(weapon_name(id), s.name);
        }
        let mut seen = Vec::new();
        for w in LOOT_POOL {
            assert!(
                (2..=WEAPON_COUNT).contains(&w),
                "pool entry {w} is off the table"
            );
            assert_ne!(w, SIDEARM, "the pool never hands out the sidearm");
            assert!(!seen.contains(&w), "pool entry {w} repeats");
            seen.push(w);
        }
        // Off-table ids are the sidearm, so a v13 client reading an id it
        // does not know still holds a gun the table describes.
        assert_eq!(weapon_stats(0).name, "Sidearm");
        assert_eq!(weapon_stats(200).name, "Sidearm");
        assert_eq!(weapon_stats(SIDEARM).name, "Sidearm");
        // The sidearm row IS today's pistol.
        let s = weapon_stats(SIDEARM);
        assert_eq!((s.cooldown, s.mag, s.damage), (0.18, 8, 1));
        assert_eq!(
            (s.speed, s.ttl, s.reload),
            (BULLET_SPEED, BULLET_TTL, RELOAD_SECS)
        );
        assert_eq!(s.reserve, RESERVE_INFINITE);
    }

    #[test]
    fn hash64_is_the_splitmix_finaliser() {
        // The finaliser alone has no published vector set; these were
        // computed by an independent big-integer implementation of the
        // same three steps and pinned. Zero is a fixed point of the
        // finaliser, which is why every roll salts the tick before hashing.
        assert_eq!(hash64(0), 0);
        assert_eq!(hash64(1), 0x5692_161d_100b_05e5);
        assert_eq!(hash64(2), 0xdbd2_3897_3a2b_148a);
        assert_eq!(hash64(0x9e37_79b9_7f4a_7c15), 0xe220_a839_7b1d_cdaf);
    }

    #[test]
    fn roll_differs_across_tick_player_and_salt() {
        let base = roll(7, 100, 0, 0);
        assert_eq!(base, roll(7, 100, 0, 0), "a roll is a pure function");
        assert_ne!(base, roll(7, 101, 0, 0), "the tick changes it");
        assert_ne!(base, roll(7, 100, 1, 0), "the player changes it");
        assert_ne!(base, roll(7, 100, 0, SALT_LOOT), "the salt changes it");
        assert_ne!(base, roll(8, 100, 0, 0), "the seed changes it");
        // And tick 0 for player 0 is not the degenerate zero hash.
        assert_ne!(roll(0, 0, 0, 0), 0);
    }

    #[test]
    fn unit_pair_is_in_range_and_exact_for_the_mantissa() {
        for i in 0..10_000u64 {
            let (u, v) = unit_pair(hash64(i));
            assert!((0.0..1.0).contains(&u), "{u}");
            assert!((0.0..1.0).contains(&v), "{v}");
            // 24 bits over 2^24: multiplying back gives the integer exactly.
            assert_eq!((u * 16_777_216.0).fract(), 0.0);
            assert_eq!((v * 16_777_216.0).fract(), 0.0);
        }
        assert_eq!(unit_pair(0), (0.0, 0.0));
        assert_eq!(
            unit_pair(u64::MAX),
            (1.0 - 2f32.powi(-24), 1.0 - 2f32.powi(-24))
        );
    }

    #[test]
    fn no_rng_state_lives_on_the_sim() {
        // A grep-shaped assertion: the only `rand01` generators in this file
        // are the two world generators, and every per-tick roll goes through
        // the stateless `roll`. A third `rand01` here would be exactly the
        // per-tick RNG state that replays and rollback cannot survive.
        let src = include_str!("shooter.rs");
        // Assembled so this test's own source does not count as a third.
        let needle = ["let mut ", "rand01"].concat();
        assert_eq!(src.matches(&needle).count(), 2);
    }

    #[test]
    fn segment_hits_box_never_blocks_where_the_span_test_is_clear() {
        // Five hundred hashed segments around the tunnel roof. The exact
        // slab test may only report a hit when the segment's vertical span
        // reaches the box AND its horizontal projection reaches the
        // footprint: where the sweep's conservative test is clear because
        // the segment is wholly outside on any one axis, the exact test is
        // clear too. And a segment that ends inside the box is a hit.
        let obs = roof();
        let o = &obs[0];
        let coord = |h: u64, shift: u32, lo: f32, span: f32| {
            lo + unit_pair(hash64(h.wrapping_add(u64::from(shift)))).0 * span
        };
        for i in 0..500u64 {
            let h = hash64(i);
            let a = [
                coord(h, 1, -4.0, 8.0),
                coord(h, 2, 0.0, 5.0),
                coord(h, 3, -4.0, 8.0),
            ];
            let b = [
                coord(h, 4, -4.0, 8.0),
                coord(h, 5, 0.0, 5.0),
                coord(h, 6, -4.0, 8.0),
            ];
            let hit = segment_hits_box(a, b, o);
            let (ylo, yhi) = (a[1].min(b[1]), a[1].max(b[1]));
            let outside_y = yhi < o.base || ylo > o.h;
            let outside_x =
                (a[0] < o.min[0] && b[0] < o.min[0]) || (a[0] > o.max[0] && b[0] > o.max[0]);
            let outside_z =
                (a[2] < o.min[1] && b[2] < o.min[1]) || (a[2] > o.max[1] && b[2] > o.max[1]);
            if outside_y || outside_x || outside_z {
                assert!(!hit, "segment {a:?} -> {b:?} is wholly outside yet blocked");
            }
            let end_inside = b[0] > o.min[0]
                && b[0] < o.max[0]
                && b[1] > o.base
                && b[1] < o.h
                && b[2] > o.min[1]
                && b[2] < o.max[1];
            if end_inside {
                assert!(
                    hit,
                    "segment {a:?} -> {b:?} ends inside the box yet is clear"
                );
            }
        }
        // A level line through the slab's height band across its footprint
        // is stopped; the same line under it is not.
        assert!(segment_hits_box([-3.0, 2.7, 0.0], [3.0, 2.7, 0.0], o));
        assert!(!segment_hits_box([-3.0, 1.0, 0.0], [3.0, 1.0, 0.0], o));
        assert!(segment_hits_cover([-3.0, 2.7, 0.0], [3.0, 2.7, 0.0], &obs));
        assert!(!segment_hits_cover([-3.0, 2.7, 0.0], [3.0, 2.7, 0.0], &[]));
    }

    #[test]
    fn a_bonk_grants_a_pool_weapon_with_a_full_load() {
        // One loot block hung 2.3 up, like a train-zone block, and a player
        // jumping under it from the floor: the clamp names the block, the
        // sim pays out a pool gun with a full load, and the block goes dark.
        let mut sim = Sim::new(3);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.obstacles.push(Obstacle::boxed(
            Cover::Loot,
            [-0.5, -0.5],
            [0.5, 0.5],
            2.3,
            2.3 + LOOT_SIZE,
        ));
        sim.loot = vec![LootBlock {
            obstacle: 0,
            respawn_t: 0.0,
        }];
        sim.add_player(0);
        sim.players[0].pos = [0.0, 0.0];
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                jump: true,
                ..Default::default()
            },
        );
        let idle = HashMap::new();
        let mut paid = None;
        for tick in 0..90 {
            sim.players[0].pos = [0.0, 0.0];
            step_with(&mut sim, if tick == 0 { &inputs } else { &idle });
            if let Some(&ev) = sim.loot_events.first() {
                assert!(paid.is_none(), "paid twice");
                paid = Some(ev);
            }
        }
        let (who, block, w) = paid.expect("the jump never bonked the block");
        assert_eq!((who, block), (0, 0));
        assert!(LOOT_POOL.contains(&w), "{w} is not a pool gun");
        let p = &sim.players[0];
        assert_eq!(p.weapon, w);
        assert_eq!(p.ammo, weapon_stats(w).mag);
        assert_eq!(p.reserve, weapon_stats(w).reserve);
        assert!(sim.loot[0].respawn_t > 0.0, "the block went dark");
        assert!(
            sim.loot[0].respawn_t > LOOT_RESPAWN_SECS - 1.6,
            "the timer started at {LOOT_RESPAWN_SECS}: {}",
            sim.loot[0].respawn_t
        );
        // Walking under it never pays.
        sim.loot[0].respawn_t = 0.0;
        for _ in 0..60 {
            sim.players[0].pos = [0.0, 0.0];
            step_with(&mut sim, &idle);
            assert!(sim.loot_events.is_empty(), "walking under a block paid out");
        }
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
        let authored = Obstacle::boxed(Cover::Container, [3.0, 3.0], [5.0, 5.0], 0.0, 7.25);
        assert_eq!(obstacle_height(&authored), 7.25);
        // And it is honoured by the physics, not just stored: a player at
        // floor level is blocked by it, and its top supports them.
        let obs = vec![authored];
        assert_eq!(support_height([4.0, 4.0], PLAYER_R, 0.0, &obs), 7.25);
        let walked = move_circle([1.0, 4.0], 0.0, [1.0, 0.0], MOVE_SPEED, 0.5, &obs);
        assert!(walked[0] < 3.0, "authored height did not block: {walked:?}");
    }

    #[test]
    fn a_level_with_no_spawns_still_starts_players() {
        let level = Level {
            arena_half: ARENA_HALF,
            obstacles: Vec::new(),
            spawns: Vec::new(),
            pads: Vec::new(),
            decor: Vec::new(),
        };
        assert_eq!(level.spawn(3), spawn_point(3));
        // A short authored list wraps rather than panicking.
        let two = Level {
            arena_half: ARENA_HALF,
            obstacles: Vec::new(),
            spawns: vec![[1.0, 2.0], [3.0, 4.0]],
            pads: Vec::new(),
            decor: Vec::new(),
        };
        assert_eq!(two.spawn(0), [1.0, 2.0]);
        assert_eq!(two.spawn(5), [3.0, 4.0]);
        // And the sim places players by the same rule, first spawn and
        // respawn alike.
        let mut sim = Sim::from_level(&two, 0);
        sim.add_player(0);
        sim.add_player(1);
        sim.add_player(2);
        assert_eq!(sim.players[0].pos, [1.0, 2.0]);
        assert_eq!(sim.players[1].pos, [3.0, 4.0]);
        assert_eq!(sim.players[2].pos, [1.0, 2.0], "wraps");
        let mut none = Sim::from_level(&level, 0);
        none.add_player(3);
        assert_eq!(
            none.players[0].pos,
            spawn_point(0),
            "golden ring when empty"
        );
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
            let VStep { y: ny, vy: nvy, .. } =
                step_vertical([0.0, 0.0], y, vy, tick == 0, FIXED_DT, &obs);
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
            let VStep { y: ny, vy: nvy, .. } =
                step_vertical([0.0, 0.0], y, vy, false, FIXED_DT, &obs);
            y = ny;
            vy = nvy;
        }
        assert!((y - top).abs() < 1e-3, "settled at {y}, box top {top}");
        // Stepping off the edge starts a fall back to the floor.
        let (mut y, mut vy) = (top, 0.0);
        for _ in 0..240 {
            let VStep { y: ny, vy: nvy, .. } =
                step_vertical([9.0, 9.0], y, vy, false, FIXED_DT, &obs);
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
        shield_duel_with(SIDEARM, dist, def_aim, shield, ticks)
    }

    /// `shield_duel` with the shooter holding `weapon`, sights up so a
    /// sniper round flies its exact line.
    fn shield_duel_with(weapon: u8, dist: f32, def_aim: [f32; 2], shield: bool, ticks: u32) -> Sim {
        let mut sim = Sim::new(31);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0); // shooter
        sim.add_player(1); // defender
        arm(&mut sim.players[0], weapon);
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
                ads: true,
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

    #[test]
    fn level_fire_at_a_crouched_target_is_a_headshot() {
        // DELIBERATE, and pinned because it is the surprising one.
        //
        // Level fire is forbidden from being a free headshot in three of the
        // four stance pairings, by assertion. This is the fourth, and it goes
        // the other way:
        //
        //   standing -> standing   1.45 vs [1.56, 1.86]   body
        //   standing -> CROUCHED   1.45 vs [1.25, 1.55]   HEAD
        //   crouched -> standing   0.85 vs [1.56, 1.86]   body
        //   crouched -> crouched   0.85 vs [1.25, 1.55]   body
        //
        // A crouched player's head rises into a standing player's eye line -
        // which is what crouching does in life and in most shooters. And
        // because a pitch-0 round has vy = 0, it holds 1.45 forever: this is
        // true at EVERY range, and the shooter aiming at what looks like a
        // chest does not have to know. Crouch is therefore a hard commitment,
        // not free value: it still shrinks your radius, and it now also puts
        // your head where the bullets already are.
        //
        // The point of the test is not the direction, it is that the next
        // person to tune BODY_H_CROUCH finds out they changed this. It moved
        // once already (1.25 -> 1.55) and this behaviour came with it.
        assert!(
            head_lo(true) < EYE_STAND && EYE_STAND < BODY_H_CROUCH,
            "a standing muzzle {EYE_STAND} must sit inside the crouched head band [{}, {BODY_H_CROUCH}]",
            head_lo(true)
        );
        assert!(
            one_shot_kills(0.0, 5.0, 0.0, true),
            "level fire at a crouched target is a headshot, deliberately"
        );
    }

    #[test]
    fn the_head_band_matches_the_drawn_model() {
        // The guard for "what you see is what you hit". These numbers are the
        // rig's, not this crate's, and the only way they stay true is if
        // someone is told when they stop being true.
        //
        // Standing, from ember-engine/src/rig.rs: ROOT pelvis_h 0.98, SPINE
        // +0.05, NECK +spine_len 0.52 => neck at 1.55; the head part is
        // anchored 0.01 above NECK and is 0.30 tall => drawn head [1.56, 1.86].
        let drawn_neck = 0.98 + 0.05 + 0.52;
        let drawn_head_lo = drawn_neck + 0.01;
        let drawn_head_hi = drawn_head_lo + 0.30;
        assert!(
            (BODY_H_STAND - drawn_head_hi).abs() < 1e-4,
            "standing hit volume must top out at the drawn head: {BODY_H_STAND} vs {drawn_head_hi}"
        );
        assert!(
            (head_lo(false) - drawn_head_lo).abs() < 1e-4,
            "standing head band must start at the drawn head: {} vs {drawn_head_lo}",
            head_lo(false)
        );

        // Crouched: walk_pose sinks the root by crouch * (0.44 + 0.43) * 0.36.
        let sink = (0.44f32 + 0.43) * 0.36;
        assert!(
            (BODY_H_STAND - BODY_H_CROUCH - sink).abs() < 0.02,
            "the crouched volume must sink with the model: {} vs {sink}",
            BODY_H_STAND - BODY_H_CROUCH
        );
    }

    #[test]
    fn a_crouched_head_is_reachable_at_all() {
        // Before the volume was tied to the model, a crouched player's drawn
        // head sat entirely ABOVE their own hitbox, so it could not be hit for
        // a headshot or even for body damage. Assert the band is inside the
        // volume rather than floating above it.
        let lo = head_lo(true);
        assert!(
            lo < BODY_H_CROUCH && lo > 0.0,
            "the crouched head band [{lo}, {BODY_H_CROUCH}] must lie inside the crouched volume"
        );
        // And it must still be above a crouched player's OWN muzzle, or a
        // crouched player shooting level would headshot another crouched one
        // for free - the same trap HEAD_H is sized to avoid when standing.
        assert!(
            lo > EYE_CROUCH,
            "crouched head band {lo} must sit above the crouched muzzle {EYE_CROUCH}"
        );
    }

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

    /// Fire one shot from `from_y` at `pitch` into a target `gap` away, and
    /// report whether it died on the FIRST round that connected.
    fn one_shot_kills(from_y: f32, gap: f32, pitch: f32, target_crouch: bool) -> bool {
        one_shot_kills_with(SIDEARM, from_y, gap, pitch, target_crouch)
    }

    /// `one_shot_kills` with the shooter holding `weapon`, sights up so a
    /// weapon with a hip cone fires its exact line.
    fn one_shot_kills_with(
        weapon: u8,
        from_y: f32,
        gap: f32,
        pitch: f32,
        target_crouch: bool,
    ) -> bool {
        let mut sim = Sim::new(11);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        arm(&mut sim.players[0], weapon);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                pitch,
                fire: true,
                ads: true,
                ..Default::default()
            },
        );
        inputs.insert(
            1,
            PlayerIn {
                crouch: target_crouch,
                ..Default::default()
            },
        );
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
            one_shot_kills(0.0, 5.0, 0.03, false),
            "a round arriving in the head band must kill from full health"
        );
    }

    #[test]
    fn a_body_shot_still_takes_three() {
        // The same geometry aimed down into the chest must NOT one-shot, or
        // the head zone has swallowed the whole body.
        assert!(
            !one_shot_kills(0.0, 5.0, -0.05, false),
            "a chest hit must not be lethal"
        );
    }

    #[test]
    // The first assertion IS on constants, deliberately: it exists to name the
    // two numbers in its failure message when someone retunes one of them.
    #[allow(clippy::assertions_on_constants)]
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
            !one_shot_kills(0.0, 5.0, 0.0, false),
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
            one_shot_kills(from_y, gap, pitch, false),
            "a steep round through the head must still be a headshot"
        );
        // And on the sniper, whose 60 m/s is the fastest round in the
        // table: at MAX_PITCH the walk clamps at 32 samples and 60 gives
        // 0.26 m per sample, under HEAD_H. This is the number that kept the
        // sniper off 90.
        let sniper = weapon_stats(6);
        let per_tick = pitch.tan().abs() * sniper.speed * FIXED_DT;
        assert!(
            per_tick > HEAD_H,
            "the sniper must straddle too: {per_tick}"
        );
        assert!(
            one_shot_kills_with(6, from_y, gap, pitch, false),
            "a steep sniper round through the head must still be a headshot"
        );
    }

    // ---- v13: an obstacle with a bottom --------------------------------

    /// A tunnel roof and nothing else: a 4 x 4 box hung 2.5 above the
    /// floor, 0.4 thick, so a standing body (1.86) walks under it.
    fn roof() -> Vec<Obstacle> {
        vec![Obstacle::boxed(
            Cover::Roof,
            [-2.0, -2.0],
            [2.0, 2.0],
            2.5,
            2.9,
        )]
    }

    /// Walks +x for `ticks` at `y`, one sim tick at a time, so a box in the
    /// way is met at its face rather than jumped over by a long dt.
    fn walk(mut pos: [f32; 2], y: f32, ticks: u32, obs: &[Obstacle]) -> [f32; 2] {
        for _ in 0..ticks {
            pos = move_circle(pos, y, [1.0, 0.0], MOVE_SPEED, FIXED_DT, obs);
        }
        pos
    }

    #[test]
    fn a_raised_box_blocks_only_a_body_that_reaches_it() {
        let obs = roof();
        // On the floor the head (1.86) is under the bottom (2.5): walk
        // straight through, 60 ticks = 9 units, from -4 to 5.
        let under = walk([-4.0, 0.0], 0.0, 60, &obs);
        assert!(
            under[0] > 4.0,
            "blocked by a roof from the floor: {under:?}"
        );
        // Feet at 1.0: head 2.86 is inside the box and the feet are more
        // than a step below its top, so its side is a wall.
        let mid = walk([-4.0, 0.0], 1.0, 60, &obs);
        assert!(
            mid[0] <= -2.0 - PLAYER_R + 1e-3,
            "walked through the roof's side: {mid:?}"
        );
        // Standing on it: walk across.
        let over = walk([-4.0, 0.0], 2.9, 60, &obs);
        assert!(over[0] > 4.0, "could not walk across the roof: {over:?}");
        // And a floor box is exactly what it was: a wall from the floor.
        let floor_box = one_box();
        let walked = walk([-3.0, 0.0], 0.0, 60, &floor_box);
        assert!(
            walked[0] <= -1.5 - PLAYER_R + 1e-3,
            "a floor box stopped blocking: {walked:?}"
        );
    }

    #[test]
    fn a_raised_box_supports_only_feet_at_or_above_its_bottom() {
        let obs = roof();
        let at = [0.0, 0.0];
        assert_eq!(
            support_height(at, PLAYER_R, 0.0, &obs),
            0.0,
            "under it you are on the floor"
        );
        assert_eq!(
            support_height(at, PLAYER_R, 2.4, &obs),
            0.0,
            "just under its bottom you are still not on it"
        );
        assert_eq!(
            support_height(at, PLAYER_R, 2.5, &obs),
            2.9,
            "at its bottom you are on it"
        );
        assert_eq!(
            support_height(at, PLAYER_R, 4.0, &obs),
            2.9,
            "above it you are on it"
        );
        // Driven: dropped from above, the player lands ON the roof.
        let (mut y, mut vy) = (5.0f32, 0.0f32);
        for _ in 0..240 {
            let VStep { y: ny, vy: nvy, .. } = step_vertical(at, y, vy, false, FIXED_DT, &obs);
            y = ny;
            vy = nvy;
        }
        assert!((y - 2.9).abs() < 1e-3, "settled at {y}, roof top 2.9");
        // And a floor box supports from the floor exactly as before.
        let floor_box = one_box();
        assert_eq!(
            support_height(at, PLAYER_R, 0.0, &floor_box),
            obstacle_height(&floor_box[0])
        );
    }

    #[test]
    fn a_jump_under_a_raised_box_bonks_at_its_bottom() {
        let obs = roof();
        let cap = 2.5 - BODY_H_STAND;
        let (mut y, mut vy) = (0.0f32, 0.0f32);
        let mut peak = 0.0f32;
        let mut bonk_ticks = 0;
        for tick in 0..180 {
            let VStep {
                y: ny,
                vy: nvy,
                grounded,
                bonked,
            } = step_vertical([0.0, 0.0], y, vy, tick == 0, FIXED_DT, &obs);
            y = ny;
            vy = nvy;
            peak = peak.max(y);
            assert!(
                y <= cap + 1e-4,
                "the head went into the roof: feet {y}, cap {cap}"
            );
            assert!(
                y < 1e-3 || !grounded,
                "reported grounded in mid-air at {y} (the bonk tick must not re-arm a jump)"
            );
            // The clamp names its box on the tick it fires, and only then.
            if (y - cap).abs() < 1e-4 && vy == 0.0 && bonk_ticks == 0 {
                assert_eq!(bonked, Some(0), "the clamp tick must name the roof");
                bonk_ticks += 1;
            } else {
                assert_eq!(bonked, None, "tick {tick} reported a bonk at {y}");
            }
        }
        assert_eq!(bonk_ticks, 1, "exactly one clamp tick");
        // Pinned, not bracketed: the open-air apex is 1.6867, so anything
        // near the cap proves the clamp and not a short jump.
        assert!(
            (peak - cap).abs() < 1e-3,
            "jump peaked at {peak}, expected the ceiling clamp {cap}"
        );
        assert!(y.abs() < 1e-3, "did not fall back to the floor: {y}");
    }

    #[test]
    fn a_round_passes_under_a_raised_box_and_is_stopped_inside_it() {
        // The same 3-wide box between a shooter and a target 9 apart, hung
        // at different heights. Level fire leaves at EYE_STAND 1.45.
        let run = |base: f32, h: f32| -> bool {
            let mut sim = Sim::new(22);
            sim.obstacles.clear();
            sim.pads.clear();
            sim.obstacles.push(Obstacle::boxed(
                Cover::Roof,
                [3.0, -2.0],
                [6.0, 2.0],
                base,
                h,
            ));
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
                        p.y = 0.0;
                        p.vy = 0.0;
                    }
                    1 if p.alive => {
                        p.pos = [9.0, 0.0];
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
        };
        assert!(
            run(2.5, 2.9),
            "level fire at 1.45 must pass under a box hung at 2.5"
        );
        assert!(
            !run(1.0, 3.0),
            "the same box hung at 1.0 spans 1.45 and must stop it"
        );
        assert!(!run(0.0, 3.0), "a floor box stops it exactly as before");
    }

    // ---- v13: Trench City ----------------------------------------------

    #[test]
    fn trench_city_has_eight_clear_spawns_far_apart() {
        let level = Level::trench_city();
        assert_eq!(level.spawns.len(), 8);
        let inside = ARENA_HALF - PLAYER_R;
        for (i, s) in level.spawns.iter().enumerate() {
            assert!(
                s[0].abs() < inside && s[1].abs() < inside,
                "spawn {i} {s:?} is outside the arena"
            );
            for o in &level.obstacles {
                assert!(!overlaps(*s, PLAYER_R, o), "spawn {i} {s:?} overlaps {o:?}");
            }
            for (j, t) in level.spawns.iter().enumerate().skip(i + 1) {
                let d = dist(*s, *t);
                // Section 4.2 promises 16 and the map delivers 17.0; the
                // invariant list used to say 12, which a relayout could
                // have met while breaking the layout's own promise.
                assert!(d >= 16.0, "spawns {i} and {j} are only {d} apart");
            }
        }
        // The sim places players there in slot order, and the first four
        // slots land on four different sides.
        let mut sim = Sim::from_level(&level, 0);
        for id in 0..8u8 {
            sim.add_player(id);
        }
        for (p, s) in sim.players.iter().zip(&level.spawns) {
            assert_eq!(p.pos, *s);
        }
        let mut sides: Vec<i32> = level.spawns[..4]
            .iter()
            .map(|s| {
                if s[1].abs() > s[0].abs() {
                    s[1].signum() as i32
                } else {
                    2 * s[0].signum() as i32
                }
            })
            .collect();
        sides.sort_unstable();
        sides.dedup();
        assert_eq!(sides.len(), 4, "slots 0..4 must be one per side");
    }

    #[test]
    fn trench_city_boxes_are_inside_the_arena_and_well_formed() {
        let level = Level::trench_city();
        assert_eq!(level.obstacles.len(), 1 + 4 * TRENCH_NORTH.len());
        assert_eq!(level.arena_half, ARENA_HALF);
        for o in &level.obstacles {
            assert!(
                o.min[0] < o.max[0] && o.min[1] < o.max[1],
                "inverted or degenerate box {o:?}"
            );
            assert!(
                o.min[0] >= -ARENA_HALF
                    && o.min[1] >= -ARENA_HALF
                    && o.max[0] <= ARENA_HALF
                    && o.max[1] <= ARENA_HALF,
                "{o:?} leaves the arena"
            );
            assert!(o.base < o.h, "{o:?} has its bottom above its top");
            assert!(o.base >= 0.0, "{o:?} starts below the floor");
            if o.kind == Cover::Roof {
                assert!(
                    o.base >= CONTAINER_MIN_H,
                    "roof {o:?} is too low to walk under"
                );
            } else {
                assert_eq!(o.base, 0.0, "{o:?}: only roofs leave the floor");
            }
        }
    }

    #[test]
    fn trench_city_is_four_fold_symmetric() {
        let level = Level::trench_city();
        // The quarter turn is written out here rather than borrowed from
        // the builder, so a builder that rotates wrongly cannot agree with
        // itself. Both corners are mapped and min/max re-derived.
        let turn = |o: &Obstacle| {
            let (ax, az) = (-o.min[1], o.min[0]);
            let (bx, bz) = (-o.max[1], o.max[0]);
            Obstacle {
                min: [ax.min(bx), az.min(bz)],
                max: [ax.max(bx), az.max(bz)],
                ..*o
            }
        };
        let key = |o: &Obstacle| -> [f32; 7] {
            [
                f32::from(o.kind as u8),
                o.min[0],
                o.min[1],
                o.max[0],
                o.max[1],
                o.base,
                o.h,
            ]
        };
        let order = |x: &[f32; 7], y: &[f32; 7]| {
            x.iter()
                .zip(y)
                .map(|(p, q)| p.total_cmp(q))
                .find(|c| c.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        };
        let mut a: Vec<[f32; 7]> = level.obstacles.iter().map(key).collect();
        let mut b: Vec<[f32; 7]> = level.obstacles.iter().map(|o| key(&turn(o))).collect();
        a.sort_by(order);
        b.sort_by(order);
        for (x, y) in a.iter().zip(&b) {
            for k in 0..7 {
                assert!(
                    (x[k] - y[k]).abs() < 1e-4,
                    "obstacle multiset is not closed under a quarter turn: {x:?} vs {y:?}"
                );
            }
        }
        let turn_pt = |p: [f32; 2]| [-p[1], p[0]];
        for p in &level.pads {
            let q = turn_pt(*p);
            assert!(
                level.pads.iter().any(|r| dist(*r, q) < 1e-4),
                "pad {p:?} turned to {q:?} is not a pad"
            );
        }
        for s in &level.spawns {
            let q = turn_pt(*s);
            assert!(
                level.spawns.iter().any(|r| dist(*r, q) < 1e-4),
                "spawn {s:?} turned to {q:?} is not a spawn"
            );
        }
    }

    #[test]
    fn every_container_roof_is_reached_by_a_climbing_chain() {
        let level = Level::trench_city();
        let obs = &level.obstacles;
        let containers: Vec<&Obstacle> = obs
            .iter()
            .filter(|o| o.kind == Cover::Container && (o.h - 2.6).abs() < 1e-6 && o.base == 0.0)
            .collect();
        assert_eq!(containers.len(), 8, "two climbable containers per side");
        for c in containers {
            let chain = obs
                .iter()
                .filter(|k| k.kind == Cover::Crate && gap(k, c) <= 0.5)
                .find_map(|k| {
                    obs.iter()
                        .find(|a| a.kind == Cover::Ammo && gap(a, k) <= 0.5)
                        .map(|a| (k, a))
                });
            let Some((step, ammo)) = chain else {
                panic!("container {c:?} has no crate within 0.5 with an ammo box within 0.5 of it");
            };
            let (ac, kc, cc) = (centre(ammo), centre(step), centre(c));
            // Start on the floor 1.4 beyond the ammo box, on the side away
            // from the crate, and prove that is open floor.
            let away = [
                (ac[0] - kc[0]) / dist(ac, kc),
                (ac[1] - kc[1]) / dist(ac, kc),
            ];
            let start = [ac[0] + away[0] * 1.4, ac[1] + away[1] * 1.4];
            assert!(
                !obs.iter().any(|o| overlaps(start, PLAYER_R, o)),
                "chain start {start:?} for {c:?} is inside cover"
            );
            let (pos, y) = climb(start, 0.0, ac, ammo.h, 600, obs)
                .unwrap_or_else(|| panic!("floor -> ammo failed for {c:?}"));
            let (pos, y) = climb(pos, y, kc, step.h, 600, obs)
                .unwrap_or_else(|| panic!("ammo -> crate failed for {c:?}"));
            let (_, y) = climb(pos, y, cc, c.h, 600, obs)
                .unwrap_or_else(|| panic!("crate -> container failed for {c:?}"));
            assert!((y - 2.6).abs() < 1e-3, "ended with feet at {y}");

            // And the floor alone is not enough: from the floor on the far
            // side of the container, the same driver never gets on top.
            // That is what the container class promises - not that the
            // chain is the only way onto anything raised: from a fire step
            // the wall tops and the tunnel roof are one hop away, pinned in
            // wall_tops_and_the_roof_are_one_hop_from_a_fire_step.
            let toward = [-away[0], -away[1]];
            let half = if toward[0].abs() > toward[1].abs() {
                (c.max[0] - c.min[0]) * 0.5
            } else {
                (c.max[1] - c.min[1]) * 0.5
            };
            let far = [
                cc[0] + toward[0] * (half + 1.4),
                cc[1] + toward[1] * (half + 1.4),
            ];
            assert!(
                !obs.iter().any(|o| overlaps(far, PLAYER_R, o)),
                "far start {far:?} for {c:?} is inside cover"
            );
            assert!(
                climb(far, 0.0, cc, c.h, 600, obs).is_none(),
                "container {c:?} was climbed from the floor without its chain"
            );
        }
    }

    /// One shot from `from` at `from_y`, elevation `pitch`, at a target
    /// standing on the floor at `to`; both pinned every tick. Whether it
    /// connected within two seconds.
    fn shot_over(obs: &[Obstacle], from: [f32; 2], from_y: f32, pitch: f32, to: [f32; 2]) -> bool {
        shot_over_at(obs, from, from_y, pitch, to, 0.0)
    }

    /// `shot_over` with the target's feet pinned at `to_y` - on a roof, say.
    fn shot_over_at(
        obs: &[Obstacle],
        from: [f32; 2],
        from_y: f32,
        pitch: f32,
        to: [f32; 2],
        to_y: f32,
    ) -> bool {
        let mut sim = Sim::new(0);
        sim.obstacles = obs.to_vec();
        sim.pads.clear();
        sim.add_player(0);
        sim.add_player(1);
        let aim = [(to[0] - from[0]), (to[1] - from[1])];
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim,
                pitch,
                fire: true,
                ..Default::default()
            },
        );
        inputs.insert(1, PlayerIn::default());
        for _ in 0..120 {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => {
                    p.pos = from;
                    p.y = from_y;
                    p.vy = 0.0;
                }
                1 if p.alive => {
                    p.pos = to;
                    p.y = to_y;
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
    fn a_tunnel_is_walked_under_shot_along_shielded_through_and_stood_on() {
        let level = Level::trench_city();
        let obs = &level.obstacles;
        // The north tunnel: the roof that straddles x = 0 on the +z side.
        let roof = obs
            .iter()
            .find(|o| o.kind == Cover::Roof && o.min[0] < 0.0 && o.max[0] > 0.0 && o.min[1] > 0.0)
            .expect("a roof on the north side");
        let mid_z = f32::midpoint(roof.min[1], roof.max[1]);

        // Walked the full length at y = 0, driven by the sim's own step.
        // Starts on open floor one metre before the roof, where the shot
        // below also starts: two metres out is inside the fire step crate's
        // collision circle, and the strict floor assertion then held only
        // because move_circle happens to run before step_vertical.
        let mut pos = [roof.min[0] - 1.0, mid_z];
        assert!(
            !obs.iter().any(|o| overlaps(pos, PLAYER_R, o)),
            "tunnel walk start {pos:?} is inside cover"
        );
        let (mut y, mut vy) = (0.0f32, 0.0f32);
        let mut reached = false;
        for _ in 0..200 {
            pos = move_circle(pos, y, [1.0, 0.0], MOVE_SPEED, FIXED_DT, obs);
            let VStep { y: ny, vy: nvy, .. } = step_vertical(pos, y, vy, false, FIXED_DT, obs);
            y = ny;
            vy = nvy;
            assert_eq!(y, 0.0, "lifted off the floor inside the tunnel at {pos:?}");
            if pos[0] > roof.max[0] + 1.0 {
                reached = true;
                break;
            }
        }
        assert!(reached, "blocked inside the tunnel at {pos:?}");

        // A jump inside it caps at base - BODY_H_STAND.
        let cap = roof.base - BODY_H_STAND;
        let (mut y, mut vy, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        for tick in 0..180 {
            let VStep { y: ny, vy: nvy, .. } =
                step_vertical([0.0, mid_z], y, vy, tick == 0, FIXED_DT, obs);
            y = ny;
            vy = nvy;
            peak = peak.max(y);
        }
        assert!(
            (peak - cap).abs() < 1e-3,
            "jump in the tunnel peaked at {peak}, expected {cap}"
        );
        assert!(y.abs() < 1e-3, "did not land back on the tunnel floor: {y}");

        // A level shot at chest height passes along it, end to end.
        let (west, east) = ([roof.min[0] - 1.0, mid_z], [roof.max[0] + 1.0, mid_z]);
        assert!(
            shot_over(obs, west, 0.0, 0.0, east),
            "level fire must travel the length of the tunnel"
        );

        // A shot from the roof top, down through the roof, is stopped - and
        // the same shot with the roof gone lands, so it is the roof that
        // stopped it and not the geometry.
        let (on_roof, below) = ([-3.0, mid_z], [3.0, mid_z]);
        let drop = roof.h + EYE_STAND - 1.0;
        let pitch = -(drop / dist(on_roof, below)).atan();
        assert!(
            !shot_over(obs, on_roof, roof.h, pitch, below),
            "a round fired down through the roof must be stopped by it"
        );
        let unroofed: Vec<Obstacle> = obs.iter().filter(|o| *o != roof).copied().collect();
        assert!(
            shot_over(&unroofed, on_roof, roof.h, pitch, below),
            "control: with the roof removed the same shot must connect"
        );

        // The slab also stops what passes through it on the way to a body.
        // A round climbing at 30 degrees from the tunnel floor at a player
        // standing on the roof meets that body from INSIDE the slab (the
        // hit column starts BULLET_R below the feet), and one dropped at
        // MAX_PITCH from the roof at the player underneath likewise. Both
        // were hits while the body test ran before the cover test; both
        // connect with the roof removed, so it is the roof that stops them.
        let (below_w, roof_mid) = ([-3.0, mid_z], [0.0, mid_z]);
        assert!(
            !shot_over_at(obs, below_w, 0.0, 0.52, roof_mid, roof.h),
            "a round through the roof must not reach the player standing on it"
        );
        assert!(
            shot_over_at(&unroofed, below_w, 0.0, 0.52, roof_mid, roof.h),
            "control: with the roof removed the 30-degree shot connects"
        );
        let under = [0.5, mid_z];
        assert!(
            !shot_over_at(obs, roof_mid, roof.h, -MAX_PITCH, under, 0.0),
            "a round dropped through the roof must not reach the player under it"
        );
        assert!(
            shot_over_at(&unroofed, roof_mid, roof.h, -MAX_PITCH, under, 0.0),
            "control: with the roof removed the drop connects"
        );

        // Dropped from above, a player lands ON the roof at h.
        let (mut y, mut vy) = (roof.h + 3.0, 0.0f32);
        for _ in 0..240 {
            let VStep { y: ny, vy: nvy, .. } =
                step_vertical([0.0, mid_z], y, vy, false, FIXED_DT, obs);
            y = ny;
            vy = nvy;
        }
        assert!(
            (y - roof.h).abs() < 1e-3,
            "settled at {y}, the roof's top is {}",
            roof.h
        );
    }

    #[test]
    fn a_fire_step_lifts_the_eye_over_the_outer_wall() {
        let level = Level::trench_city();
        let outer: Vec<&Obstacle> = level
            .obstacles
            .iter()
            .filter(|o| o.kind == Cover::Wall && (o.min[1] - 14.0).abs() < 1e-6)
            .collect();
        assert_eq!(outer.len(), 3, "the north outer wall has three segments");
        let wall_h = outer[0].h;
        let steps: Vec<&Obstacle> = level
            .obstacles
            .iter()
            .filter(|k| k.kind == Cover::Crate && outer.iter().any(|w| gap(k, w) < 1e-6))
            .collect();
        assert_eq!(
            steps.len(),
            2,
            "two fire steps against the north outer wall"
        );
        assert!(
            EYE_STAND < wall_h,
            "from the floor the eye {EYE_STAND} must be under the wall {wall_h}"
        );
        for k in steps {
            let stood = support_height(centre(k), PLAYER_R, 0.0, &level.obstacles);
            assert_eq!(stood, k.h, "standing on the step puts the feet at its top");
            assert!(
                stood + EYE_STAND > wall_h,
                "from the step the eye {} must clear the wall {wall_h}",
                stood + EYE_STAND
            );
        }
    }

    #[test]
    fn all_four_pads_are_in_the_open_under_a_roof() {
        let level = Level::trench_city();
        assert_eq!(level.pads.len(), 4);
        for pad in &level.pads {
            assert!(
                !level
                    .obstacles
                    .iter()
                    .any(|o| o.base == 0.0 && overlaps(*pad, PLAYER_R, o)),
                "pad {pad:?} is inside cover"
            );
            assert!(
                level
                    .obstacles
                    .iter()
                    .any(|o| o.kind == Cover::Roof && contains(o, *pad)),
                "pad {pad:?} is not under a roof"
            );
        }
        let sim = Sim::from_level(&level, 0);
        assert_eq!(sim.pads.len(), 4);
        assert!(
            sim.pads.iter().all(|p| p.respawn_t == 0.0),
            "pads start active"
        );
        for (p, want) in sim.pads.iter().zip(&level.pads) {
            assert_eq!(p.pos, *want);
        }

        // Taken from the tunnel floor, not from the roof over it: the
        // pickup is gated on the feet being under PAD_PICK_H, or a roof
        // camper collected the pad 2.9 m below their boots through the slab
        // without ever entering a tunnel.
        let mut inputs = HashMap::new();
        inputs.insert(0, PlayerIn::default());
        for pad in &level.pads {
            let roof = level
                .obstacles
                .iter()
                .find(|o| o.kind == Cover::Roof && contains(o, *pad))
                .unwrap();
            assert!(
                roof.base >= PAD_PICK_H,
                "roof {roof:?} hangs below the pickup height"
            );
            let mut sim = Sim::from_level(&level, 0);
            sim.add_player(0);
            let pin = |sim: &mut Sim, y: f32| {
                let p = &mut sim.players[0];
                p.pos = *pad;
                p.y = y;
                p.vy = 0.0;
            };
            pin(&mut sim, roof.h);
            step_with(&mut sim, &inputs);
            assert_eq!(
                sim.players[0].weapon, SIDEARM,
                "pad {pad:?} taken from the roof top"
            );
            assert_eq!(sim.players[0].y, roof.h, "did not stay on the roof");
            pin(&mut sim, 0.0);
            step_with(&mut sim, &inputs);
            assert!(
                LOOT_POOL.contains(&sim.players[0].weapon),
                "pad {pad:?} not taken from the tunnel floor"
            );
        }
    }

    /// A fire step is a step onto the walls, not only a place to see over
    /// them.
    ///
    /// It must be tall enough to lift the eye over the wall (1.2 +
    /// `EYE_STAND` 1.45 > 2.5) and would have to be short enough that a jump
    /// from it stays under the wall's step-up line (1.2 + apex 1.69 < 2.5 -
    /// `STEP_UP`) to keep the wall unmountable; no wall height does both,
    /// because `EYE_STAND < apex + STEP_UP`. So the wall tops and the tunnel
    /// roof are standing surfaces one hop from any fire step. Accepted in
    /// plan section 9 and pinned here, so the next constant change is
    /// caught rather than discovered in play.
    #[test]
    fn wall_tops_and_the_roof_are_one_hop_from_a_fire_step() {
        let level = Level::trench_city();
        let obs = &level.obstacles;
        let find = |kind: Cover, min: [f32; 2]| {
            obs.iter()
                .find(|o| o.kind == kind && dist(o.min, min) < 1e-4)
                .unwrap_or_else(|| panic!("no {kind:?} at {min:?}"))
        };
        let step = find(Cover::Crate, [-9.6, 12.8]);
        let outer = find(Cover::Wall, [-14.4, 14.0]);
        let inner = find(Cover::Wall, [-11.0, 10.6]);
        let roof = find(Cover::Roof, [-6.0, 11.0]);
        // On the fire step, against the outer wall.
        let start = [centre(step)[0], step.max[1] - PLAYER_R];
        assert_eq!(support_height(start, PLAYER_R, 0.0, obs), step.h);

        // Fire step -> outer wall top.
        let (on_outer, y) = hop(start, step.h, [start[0], centre(outer)[1]], obs);
        assert_eq!(
            y, outer.h,
            "rested at {y} at {on_outer:?}, not on the outer wall"
        );
        assert!(
            contains(outer, on_outer),
            "{on_outer:?} is not over the outer wall"
        );
        // Outer wall top -> inner wall top, across the corridor.
        let (on_inner, y) = hop(on_outer, y, [on_outer[0], centre(inner)[1]], obs);
        assert_eq!(
            y, inner.h,
            "rested at {y} at {on_inner:?}, not on the inner wall"
        );
        assert!(
            contains(inner, on_inner),
            "{on_inner:?} is not over the inner wall"
        );
        // Fire step -> tunnel roof, no chain involved.
        let onto = [roof.min[0] + 1.0, f32::midpoint(roof.min[1], roof.max[1])];
        let (on_roof, y) = hop(start, step.h, onto, obs);
        assert_eq!(y, roof.h, "rested at {y} at {on_roof:?}, not on the roof");
        assert!(contains(roof, on_roof), "{on_roof:?} is not over the roof");
    }

    /// No spawn sees another spawn. Cover between every pair is what the
    /// three bands are for; the four adjacent-corner pairs, 17 m apart,
    /// looked straight at each other past the wall corner until the corner
    /// container was moved onto that diagonal. Proven with the sim's own
    /// shot - level fire from the floor - both ways round, because where a
    /// round's per-tick samples fall depends on which end it leaves from.
    #[test]
    fn no_spawn_sees_another_spawn() {
        let level = Level::trench_city();
        for (i, a) in level.spawns.iter().enumerate() {
            for (j, b) in level.spawns.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert!(
                    !shot_over(&level.obstacles, *a, 0.0, 0.0, *b),
                    "spawn {i} {a:?} shoots spawn {j} {b:?} on level fire"
                );
            }
        }
    }

    #[test]
    fn trench_city_survives_serde_and_a_v12_level_decodes_with_defaults() {
        let level = Level::trench_city();
        let json = serde_json::to_string(&level).unwrap();
        let back: Level = serde_json::from_str(&json).unwrap();
        assert_eq!(back, level);

        // What a v12 editor wrote: no base, no kind, no pads, no decor.
        let v12 = r#"{"arena_half":24.0,
            "obstacles":[{"min":[1.0,2.0],"max":[3.0,4.0],"h":1.2}],
            "spawns":[[5.0,6.0]]}"#;
        let old: Level = serde_json::from_str(v12).unwrap();
        assert_eq!(old.arena_half, 24.0);
        assert_eq!(old.obstacles.len(), 1);
        assert_eq!(old.obstacles[0].base, 0.0, "an absent base is the floor");
        assert_eq!(
            old.obstacles[0].kind,
            Cover::Container,
            "an absent kind is the safe default"
        );
        assert_eq!(old.obstacles[0].h, 1.2);
        assert_eq!(old.pads, Vec::<[f32; 2]>::new());
        assert_eq!(old.decor, Vec::<Decor>::new());
        assert_eq!(old.spawns, vec![[5.0, 6.0]]);
        // And it still plays.
        let mut sim = Sim::from_level(&old, 0);
        sim.add_player(0);
        assert_eq!(sim.players[0].pos, [5.0, 6.0]);
        assert_eq!(sim.pads.len(), 0, "a level without pads plays without pads");
    }

    #[test]
    fn a_seeded_level_still_plays_exactly_the_v12_arena() {
        // Every seed the tests above use, plus a spread. The expectations
        // are v12's own: obstacles from generate_arena, pads from
        // generate_pads, the golden-angle spawn ring, all on the floor,
        // classed by height, nothing decorative.
        let seeds = [
            0u64, 1, 2, 3, 4, 5, 7, 8, 9, 11, 12, 21, 22, 23, 24, 25, 31, 32, 33, 34, 42, 43,
            20_260_829,
        ];
        for seed in seeds.into_iter().chain(0..64) {
            let level = Level::from_seed(seed);
            assert_eq!(level.obstacles, generate_arena(seed), "seed {seed}");
            assert_eq!(level.pads, generate_pads(seed), "seed {seed}");
            assert_eq!(level.decor, Vec::<Decor>::new(), "seed {seed}");
            for o in &level.obstacles {
                assert_eq!(o.base, 0.0, "seed {seed}: {o:?}");
                let want = if o.h < CONTAINER_MIN_H {
                    Cover::Crate
                } else {
                    Cover::Container
                };
                assert_eq!(o.kind, want, "seed {seed}: {o:?}");
            }
            for slot in 0..MAX_PLAYERS as u32 {
                assert_eq!(level.spawn(slot), spawn_point(slot), "seed {seed}");
            }
            // Sim::new IS from_level of this: identical world either way.
            let a = Sim::new(seed);
            let b = Sim::from_level(&level, seed);
            assert_eq!(a.obstacles, b.obstacles);
            assert_eq!(a.spawns, b.spawns);
            assert_eq!(a.spawns, level.spawns);
            assert_eq!(
                a.pads.iter().map(|p| p.pos).collect::<Vec<_>>(),
                generate_pads(seed)
            );
            // Any name but the authored ones resolves to it.
            assert_eq!(Level::named("", seed), level);
            assert_eq!(Level::named("moon-base", seed), level);
        }
        assert_eq!(Level::named(MAP_TRENCH_CITY, 7), Level::trench_city());
        assert_eq!(Level::named(MAP_FREIGHT_YARD, 7), Level::freight_yard());
    }

    // ---- v18: seven bullets, the reserve, rockets and loot blocks -------

    /// A sim with no cover and no pads and `n` players, so a shot test's
    /// geometry is exactly what the test placed.
    fn open_sim(seed: u64, n: u8) -> Sim {
        let mut sim = Sim::new(seed);
        sim.obstacles.clear();
        sim.pads.clear();
        for id in 0..n {
            sim.add_player(id);
        }
        sim
    }

    /// A sim on exactly these boxes, with the loot blocks the sim derives
    /// from them, no spawns (players land on the seeded ring and every test
    /// places them by hand) and no pads.
    fn block_sim(seed: u64, obstacles: Vec<Obstacle>) -> Sim {
        Sim::from_level(
            &Level {
                arena_half: ARENA_HALF,
                obstacles,
                spawns: Vec::new(),
                pads: Vec::new(),
                decor: Vec::new(),
            },
            seed,
        )
    }

    /// A loot block hung with its bottom at `base`, centred on `(x, z)`.
    fn block(x: f32, z: f32, base: f32) -> Obstacle {
        Obstacle::boxed(
            Cover::Loot,
            [x - 0.5, z - 0.5],
            [x + 0.5, z + 0.5],
            base,
            base + LOOT_SIZE,
        )
    }

    fn player(sim: &Sim, id: u8) -> &PlayerSt {
        sim.players.iter().find(|p| p.id == id).unwrap()
    }

    /// `grant` without the fifth of a second of "new gun": the drivers
    /// below pull the trigger on tick 0, and a granted gun's cooldown would
    /// swallow that pull.
    fn arm(p: &mut PlayerSt, weapon: u8) {
        grant(p, weapon);
        p.cooldown = 0.0;
    }

    /// Holds every listed living player at a spot and a height for the
    /// tick, so neither gravity nor the trigger moves the geometry under
    /// test.
    fn hold(sim: &mut Sim, spots: &[(u8, [f32; 2], f32)]) {
        for p in &mut sim.players {
            if let Some(&(_, pos, y)) = spots.iter().find(|s| s.0 == p.id)
                && p.alive
            {
                p.pos = pos;
                p.y = y;
                p.vy = 0.0;
            }
        }
    }

    /// One trigger pull on tick 0 along `aim` at `pitch`, sights down.
    fn shot(t: u32, aim: [f32; 2], pitch: f32) -> PlayerIn {
        PlayerIn {
            aim,
            pitch,
            fire: t == 0,
            ..Default::default()
        }
    }

    /// Every field of a round as bits, so two sims are compared exactly and
    /// not up to a float tolerance.
    fn bullet_bits(b: &Bullet) -> Vec<u64> {
        vec![
            u64::from(b.pos[0].to_bits()),
            u64::from(b.pos[1].to_bits()),
            u64::from(b.vel[0].to_bits()),
            u64::from(b.vel[1].to_bits()),
            u64::from(b.y.to_bits()),
            u64::from(b.vy.to_bits()),
            u64::from(b.ttl.to_bits()),
            u64::from(b.owner),
            u64::from(b.dmg),
            u64::from(b.delay),
            u64::from(b.weapon),
            u64::from(b.pierce),
            u64::from(b.hit_mask),
        ]
    }

    fn player_bits(p: &PlayerSt) -> Vec<u64> {
        vec![
            u64::from(p.id),
            u64::from(p.pos[0].to_bits()),
            u64::from(p.pos[1].to_bits()),
            u64::from(p.y.to_bits()),
            u64::from(p.vy.to_bits()),
            u64::from(p.aim[0].to_bits()),
            u64::from(p.aim[1].to_bits()),
            u64::from(p.pitch.to_bits()),
            u64::from(p.hp),
            u64::from(p.score),
            u64::from(p.alive),
            u64::from(p.crouch),
            u64::from(p.shield),
            u64::from(p.weapon),
            u64::from(p.ammo),
            u64::from(p.reserve),
            u64::from(p.reload_t.to_bits()),
            u64::from(p.death_count),
            u64::from(p.respawn_in.to_bits()),
            u64::from(p.cooldown.to_bits()),
            u64::from(p.melee_cd.to_bits()),
        ]
    }

    /// The angle, in radians, between a round's 3D direction and the line
    /// the shooter looked along. In f64 from the f32 fields: `acos` near 1
    /// loses the small angles this measures, `atan2` of the cross product
    /// does not.
    fn offset_angle(b: &Bullet, aim: [f32; 2], pitch: f32, speed: f32) -> f64 {
        let a = [f64::from(b.vel[0]), f64::from(b.vel[1]), f64::from(b.vy)];
        let r = [
            f64::from(aim[0] * speed),
            f64::from(aim[1] * speed),
            f64::from(pitch.tan() * speed),
        ];
        let cross = [
            a[1] * r[2] - a[2] * r[1],
            a[2] * r[0] - a[0] * r[2],
            a[0] * r[1] - a[1] * r[0],
        ];
        let sin = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        let cos = a[0] * r[0] + a[1] * r[1] + a[2] * r[2];
        sin.atan2(cos)
    }

    #[test]
    fn a_looted_gun_runs_dry_and_the_sidearm_comes_back() {
        // Exactly eighteen revolver rounds (6 + 12) leave; the nineteenth
        // trigger pull fires a sidearm round a quarter second later, with
        // the bottomless reserve back in hand.
        let mut sim = open_sim(7, 1);
        grant(&mut sim.players[0], 5);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                fire: true,
                ..Default::default()
            },
        );
        let (mut revolver_rounds, mut last_revolver_tick, mut first_sidearm_tick) = (0, 0, None);
        let (mut prev_weapon, mut prev_ammo) = (sim.players[0].weapon, sim.players[0].ammo);
        for tick in 0..3000u32 {
            hold(&mut sim, &[(0, [0.0, 0.0], 0.0)]);
            step_with(&mut sim, &inputs);
            let p = &sim.players[0];
            // A round left when the magazine dropped by one under the same
            // gun; the swap tick changes the gun and counts as nothing.
            if p.weapon == prev_weapon && p.ammo + 1 == prev_ammo {
                if p.weapon == 5 {
                    revolver_rounds += 1;
                    last_revolver_tick = tick;
                } else if first_sidearm_tick.is_none() {
                    first_sidearm_tick = Some(tick);
                }
            }
            prev_weapon = p.weapon;
            prev_ammo = p.ammo;
            if first_sidearm_tick.is_some() {
                break;
            }
        }
        assert_eq!(revolver_rounds, 18, "6 in the magazine and 12 in reserve");
        let first = first_sidearm_tick.expect("the sidearm never fired");
        // The swap happens on the tick after the last round, with a 0.25 s
        // cooldown: fifteen ticks, plus one for the float remainder.
        let gap = first - last_revolver_tick;
        assert!(
            (16..=17).contains(&gap),
            "the sidearm's first round came {gap} ticks after the last revolver round"
        );
        let p = &sim.players[0];
        assert_eq!(p.weapon, SIDEARM);
        assert_eq!(p.reserve, RESERVE_INFINITE);
        assert!(
            sim.bullets.iter().any(|b| b.weapon == SIDEARM),
            "the round in flight is a sidearm round"
        );
    }

    #[test]
    fn the_sidearm_reserve_is_never_consumed() {
        let mut sim = open_sim(7, 1);
        let mut reload = HashMap::new();
        reload.insert(
            0,
            PlayerIn {
                reload: true,
                ..Default::default()
            },
        );
        let ticks = (RELOAD_SECS / FIXED_DT) as u32 + 3;
        for _ in 0..100 {
            sim.players[0].ammo = 1;
            for _ in 0..ticks {
                step_with(&mut sim, &reload);
            }
            assert_eq!(sim.players[0].ammo, weapon_stats(SIDEARM).mag);
            assert_eq!(sim.players[0].reserve, RESERVE_INFINITE);
        }
    }

    #[test]
    fn death_returns_the_sidearm() {
        let mut sim = open_sim(7, 1);
        grant(&mut sim.players[0], 3);
        sim.players[0].ammo = 11;
        sim.players[0].reserve = 4;
        sim.players[0].reload_t = 0.7;
        let p = &mut sim.players[0];
        p.hp = 0;
        p.alive = false;
        p.respawn_in = 0.01;
        let idle = HashMap::new();
        for _ in 0..3 {
            step_with(&mut sim, &idle);
        }
        let p = &sim.players[0];
        assert!(p.alive);
        assert_eq!(p.weapon, SIDEARM);
        assert_eq!(p.ammo, weapon_stats(SIDEARM).mag);
        assert_eq!(p.reserve, RESERVE_INFINITE);
        assert_eq!(p.reload_t, 0.0, "a reload in progress died with the gun");
    }

    #[test]
    fn spread_is_a_pure_function_of_seed_tick_and_shooter() {
        let mut sim = open_sim(1, 2);
        grant(&mut sim.players[0], 2);
        grant(&mut sim.players[1], 2);
        let stats = weapon_stats(2);
        let mut out = Vec::new();
        launch(&sim.players[0], &stats, false, true, 7, 100, 0, &mut out);
        launch(&sim.players[0], &stats, false, true, 7, 100, 0, &mut out);
        assert_eq!(
            bullet_bits(&out[0]),
            bullet_bits(&out[1]),
            "the same inputs are the same round"
        );
        launch(&sim.players[0], &stats, false, true, 7, 101, 0, &mut out);
        assert_ne!(out[0].vel, out[2].vel, "the tick changes the cone roll");
        launch(&sim.players[0], &stats, false, true, 8, 100, 0, &mut out);
        assert_ne!(out[0].vel, out[3].vel, "the seed changes it");
        sim.players[1].pos = sim.players[0].pos;
        launch(&sim.players[1], &stats, false, true, 7, 100, 0, &mut out);
        assert_ne!(out[0].vel, out[4].vel, "the shooter changes it");
    }

    #[test]
    fn a_zero_spread_weapon_fires_exactly_along_the_aim() {
        // The sidearm and the revolver have no cone, so their ray is the
        // v17 ray bit for bit: no rotation ran at all, whatever the sights,
        // the feet or the roll would have said.
        for weapon in [SIDEARM, 5] {
            let mut sim = open_sim(1, 1);
            grant(&mut sim.players[0], weapon);
            let p = &mut sim.players[0];
            p.aim = [0.6, 0.8];
            p.pitch = 0.3;
            let stats = weapon_stats(weapon);
            for (ads, grounded) in [(false, true), (true, false), (false, false)] {
                let mut out = Vec::new();
                launch(p, &stats, ads, grounded, 7, 100, 0, &mut out);
                let b = &out[0];
                assert_eq!(b.vel[0].to_bits(), (0.6f32 * stats.speed).to_bits());
                assert_eq!(b.vel[1].to_bits(), (0.8f32 * stats.speed).to_bits());
                assert_eq!(b.vy.to_bits(), (0.3f32.tan() * stats.speed).to_bits());
                assert_eq!(b.weapon, weapon);
                assert_eq!(b.pierce, 0);
                assert_eq!(b.hit_mask, 0);
            }
        }
    }

    #[test]
    fn spread_never_leaves_the_cone() {
        // The widest cone in the table: an empty Vityaz magazine's bloom,
        // capped at spread_max, fired in the air. Ten thousand rolls, and
        // the round is never further off the look line than the cone.
        let mut sim = open_sim(1, 1);
        grant(&mut sim.players[0], 2);
        let stats = weapon_stats(2);
        for (aim, pitch) in [([1.0, 0.0], 0.0), ([0.6, -0.8], 0.5)] {
            let p = &mut sim.players[0];
            p.aim = aim;
            p.pitch = pitch;
            p.fired = stats.mag;
            let cone = f64::from(stats.spread_max * ADS_SPREAD_AIR_MULT);
            let mut widest: f64 = 0.0;
            for tick in 0..10_000u64 {
                let mut out = Vec::new();
                launch(p, &stats, false, false, 7, tick, 0, &mut out);
                let off = offset_angle(&out[0], aim, pitch, stats.speed);
                assert!(
                    off <= cone + 1e-4,
                    "tick {tick}: {off} off the aim, cone {cone}"
                );
                widest = widest.max(off);
            }
            // And the cone is used: the widest round is near its edge.
            assert!(widest > cone * 0.9, "widest {widest} for a cone of {cone}");
        }
    }

    #[test]
    fn bloom_widens_the_cone_as_the_magazine_empties() {
        // The Vityaz starts tight and opens up: the widest offset over the
        // first five rounds is smaller than over the last five.
        let mut sim = open_sim(1, 1);
        grant(&mut sim.players[0], 2);
        let stats = weapon_stats(2);
        let mut widest = |rounds: std::ops::Range<u8>| -> f64 {
            let mut w: f64 = 0.0;
            for fired in rounds {
                sim.players[0].fired = fired;
                for tick in 0..300u64 {
                    let mut out = Vec::new();
                    launch(&sim.players[0], &stats, false, true, 7, tick, 0, &mut out);
                    w = w.max(offset_angle(&out[0], [1.0, 0.0], 0.0, stats.speed));
                }
            }
            w
        };
        let early = widest(0..5);
        let late = widest(24..29);
        assert!(
            early < late,
            "early {early} is not tighter than late {late}"
        );
    }

    #[test]
    fn a_reserve_short_refill_starts_the_bloom_over() {
        // AK, 30 + 30. Twenty rounds, a manual reload (the reserve pays
        // twenty, ten are left), thirty more, and the auto-reload can only
        // hand back ten: the magazine reads 10 of 30. Derived from the
        // magazine that is twenty rounds into a spray; counted, it is a
        // fresh magazine, and its first round is the same round a full
        // magazine's first round would be for the same roll.
        let mut sim = open_sim(7, 1);
        arm(&mut sim.players[0], 3);
        let stats = weapon_stats(3);
        let mut fire = HashMap::new();
        fire.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                fire: true,
                ..Default::default()
            },
        );
        let mut reload = HashMap::new();
        reload.insert(
            0,
            PlayerIn {
                reload: true,
                ..Default::default()
            },
        );
        let idle = HashMap::new();
        let fire_rounds = |sim: &mut Sim, n: u8| {
            let target = sim.players[0].ammo - n;
            let mut ticks = 0;
            while sim.players[0].ammo > target && ticks < 2000 {
                step_with(sim, &fire);
                ticks += 1;
            }
            assert_eq!(sim.players[0].ammo, target);
        };
        let settle = |sim: &mut Sim, inputs: &HashMap<u8, PlayerIn>| {
            for _ in 0..((stats.reload / FIXED_DT) as u32 + 3) {
                step_with(sim, inputs);
            }
        };
        fire_rounds(&mut sim, 20);
        assert_eq!(sim.players[0].fired, 20);
        settle(&mut sim, &reload);
        assert_eq!((sim.players[0].ammo, sim.players[0].reserve), (30, 10));
        assert_eq!(
            sim.players[0].fired, 0,
            "a manual reload restarts the count"
        );
        fire_rounds(&mut sim, 30);
        assert_eq!(sim.players[0].fired, 30);
        // The empty magazine started the auto-reload on its own tick.
        settle(&mut sim, &idle);
        let p = &sim.players[0];
        assert_eq!(
            (p.weapon, p.ammo, p.reserve),
            (3, 10, 0),
            "the short refill"
        );
        assert_eq!(p.fired, 0, "the short magazine is a fresh magazine");
        // Its first round is a full magazine's first round, roll for roll.
        let mut fresh = p.clone();
        arm(&mut fresh, 3);
        assert_eq!((fresh.ammo, fresh.fired), (30, 0));
        for tick in 0..50u64 {
            let (mut a, mut b) = (Vec::new(), Vec::new());
            launch(p, &stats, false, true, 7, tick, 0, &mut a);
            launch(&fresh, &stats, false, true, 7, tick, 0, &mut b);
            assert_eq!(bullet_bits(&a[0]), bullet_bits(&b[0]), "tick {tick}");
            assert!(
                offset_angle(&a[0], [1.0, 0.0], 0.0, stats.speed) <= f64::from(stats.spread) + 1e-4
            );
        }
        // And the count is the rounds fired, until the magazine is empty
        // and the dry swap hands the sidearm back at zero.
        fire_rounds(&mut sim, 4);
        assert_eq!(sim.players[0].fired, 4);
        fire_rounds(&mut sim, 6);
        for _ in 0..2 {
            step_with(&mut sim, &idle);
        }
        let p = &sim.players[0];
        assert_eq!((p.weapon, p.fired), (SIDEARM, 0), "the dry swap resets it");
    }

    #[test]
    fn ads_and_air_scale_the_cone() {
        // The sniper's sights are a line: ads_spread 0 makes the cone zero
        // and the ray exact. The AK in the air is the grounded AK times
        // ADS_SPREAD_AIR_MULT, roll for roll, so its widest round is wider.
        let mut sim = open_sim(1, 1);
        grant(&mut sim.players[0], 6);
        let sniper = weapon_stats(6);
        sim.players[0].pitch = 0.2;
        for tick in 0..200u64 {
            let mut out = Vec::new();
            launch(&sim.players[0], &sniper, true, true, 7, tick, 0, &mut out);
            assert_eq!(out[0].vel[0].to_bits(), sniper.speed.to_bits());
            assert_eq!(out[0].vel[1].to_bits(), 0.0f32.to_bits());
            assert_eq!(out[0].vy.to_bits(), (0.2f32.tan() * sniper.speed).to_bits());
        }
        grant(&mut sim.players[0], 3);
        sim.players[0].pitch = 0.0;
        sim.players[0].fired = 15;
        let ak = weapon_stats(3);
        let widest = |grounded: bool| -> f64 {
            let mut w: f64 = 0.0;
            for tick in 0..300u64 {
                let mut out = Vec::new();
                launch(&sim.players[0], &ak, false, grounded, 7, tick, 0, &mut out);
                w = w.max(offset_angle(&out[0], [1.0, 0.0], 0.0, ak.speed));
            }
            w
        };
        let (planted, airborne) = (widest(true), widest(false));
        assert!(
            airborne > planted,
            "airborne {airborne} vs planted {planted}"
        );
        // Same rolls, one multiplier: the ratio is the constant.
        assert!(
            (airborne / planted - f64::from(ADS_SPREAD_AIR_MULT)).abs() < 1e-3,
            "ratio {}",
            airborne / planted
        );
    }

    #[test]
    fn a_revolver_slug_drops_under_gravity_and_keeps_its_horizontal_speed() {
        // Level fire from eight metres up so the slug has a second of air.
        // After sixty ticks the horizontal speed is exactly the table's and
        // the drop is a half g t squared (semi-implicit Euler lands a hair
        // over: 1.525 for 1.5).
        let mut sim = open_sim(1, 1);
        arm(&mut sim.players[0], 5);
        let stats = weapon_stats(5);
        let mut inputs = HashMap::new();
        let mut eye = 0.0f32;
        for t in 0..60u32 {
            hold(&mut sim, &[(0, [-20.0, 0.0], 8.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            step_with(&mut sim, &inputs);
            if t == 0 {
                // The held shooter falls one tick of gravity before the
                // launch: the eye is where the player ended this tick.
                eye = sim.players[0].y + EYE_STAND;
            }
        }
        let b = sim.bullets.first().expect("the slug is still in flight");
        assert_eq!(b.weapon, 5);
        let h_speed = (b.vel[0] * b.vel[0] + b.vel[1] * b.vel[1]).sqrt();
        assert_eq!(h_speed.to_bits(), stats.speed.to_bits());
        let fell = eye - b.y;
        let expected = 0.5 * stats.gravity.abs() * 1.0;
        assert!(
            (fell - expected).abs() < 0.03,
            "fell {fell}, expected {expected}"
        );
        assert!(b.vy < 0.0, "the slug is falling");
    }

    #[test]
    fn a_zero_gravity_round_flies_the_old_straight_line() {
        // A sidearm round's height each tick is the v17 formula, `y += vy
        // dt`, bit for bit: a zero gravity row adds exactly zero.
        let mut sim = open_sim(1, 1);
        let pitch = 0.3f32;
        let vy = pitch.tan() * BULLET_SPEED;
        let mut inputs = HashMap::new();
        let mut y = 0.0f32;
        for t in 0..40u32 {
            hold(&mut sim, &[(0, [-20.0, 0.0], 8.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], pitch));
            step_with(&mut sim, &inputs);
            let b = sim.bullets.first().expect("the round is in flight");
            assert_eq!(b.vy.to_bits(), vy.to_bits());
            if t == 0 {
                // The held shooter falls one tick of gravity before the
                // launch, so the eye is read from the round itself; from
                // here every tick is `y += vy dt` and nothing else.
                y = b.y;
                let eye = sim.players[0].y + EYE_STAND;
                assert_eq!(y.to_bits(), (eye + vy * FIXED_DT).to_bits());
            } else {
                y += vy * FIXED_DT;
                assert_eq!(b.y.to_bits(), y.to_bits(), "tick {t}: {} vs {y}", b.y);
            }
        }
    }

    /// The sniper duel: shooter at the origin with the sights up, targets
    /// down the +x line at `xs`, run `ticks` and the targets' hit points
    /// read back in order.
    fn sniper_line(obstacles: Vec<Obstacle>, xs: &[f32], ticks: u32) -> (Sim, Vec<u8>) {
        let mut sim = open_sim(1, 1 + xs.len() as u8);
        sim.obstacles = obstacles;
        arm(&mut sim.players[0], 6);
        let mut spots = vec![(0, [0.0, 0.0], 0.0)];
        for (i, &x) in xs.iter().enumerate() {
            spots.push((i as u8 + 1, [x, 0.0], 0.0));
        }
        let mut inputs = HashMap::new();
        for t in 0..ticks {
            hold(&mut sim, &spots);
            inputs.insert(
                0,
                PlayerIn {
                    ads: true,
                    ..shot(t, [1.0, 0.0], 0.0)
                },
            );
            step_with(&mut sim, &inputs);
        }
        let hp = (1..=xs.len() as u8).map(|id| player(&sim, id).hp).collect();
        (sim, hp)
    }

    #[test]
    fn a_sniper_round_passes_through_one_body_and_hits_the_next() {
        // Three targets on a line 3 m apart: the first two lose two points
        // each, the third is untouched, because pierce 1 is two bodies.
        let (sim, hp) = sniper_line(Vec::new(), &[3.0, 6.0, 9.0], 30);
        assert_eq!(hp, vec![1, 1, MAX_HP]);
        assert!(
            sim.bullets.is_empty(),
            "the round stopped in the second body"
        );
    }

    #[test]
    fn two_bodies_on_one_segment_are_both_hit() {
        // Two bodies inside one tick's metre of sniper travel: both are hit
        // on the SAME tick, which is what the loop continuing after a
        // pierced hit buys. A break would have skipped the second for good,
        // because the next tick's segment starts beyond it.
        let mut sim = open_sim(1, 3);
        arm(&mut sim.players[0], 6);
        let spots = [
            (0, [0.0, 0.0], 0.0),
            (1, [4.05, 0.0], 0.0),
            (2, [4.15, 0.0], 0.0),
        ];
        let mut inputs = HashMap::new();
        let mut both_on_one_tick = false;
        for t in 0..30u32 {
            hold(&mut sim, &spots);
            inputs.insert(
                0,
                PlayerIn {
                    ads: true,
                    ..shot(t, [1.0, 0.0], 0.0)
                },
            );
            step_with(&mut sim, &inputs);
            if sim.hits.len() == 2 {
                both_on_one_tick = true;
                let victims: Vec<u8> = sim.hits.iter().map(|h| h.1).collect();
                assert_eq!(victims, vec![1, 2]);
            }
        }
        assert!(both_on_one_tick, "the two bodies were not hit on one tick");
        assert_eq!((player(&sim, 1).hp, player(&sim, 2).hp), (1, 1));
    }

    #[test]
    fn a_pierced_body_is_not_hit_twice() {
        // A body is 1.64 m across to a sniper round doing a metre a tick,
        // so the round overlaps it on consecutive ticks; the mask is what
        // makes that one hit.
        let (sim, hp) = sniper_line(Vec::new(), &[5.0], 30);
        assert_eq!(hp, vec![1]);
        assert_eq!(player(&sim, 1).death_count, 0);
    }

    #[test]
    fn pierce_does_not_pass_through_cover() {
        // A body, then a container, then a body: the round goes through the
        // first and is stopped by the container, so the second is untouched.
        let container = Obstacle::boxed(Cover::Container, [5.0, -1.0], [6.0, 1.0], 0.0, 2.6);
        let (_, hp) = sniper_line(vec![container], &[3.0, 8.0], 30);
        assert_eq!(hp, vec![1, MAX_HP]);
    }

    #[test]
    fn a_reflected_sniper_round_no_longer_pierces() {
        // Caught at 5 m (tick 4 of a metre a tick) and read on tick 6, on
        // its way back: the pierce and the mask are gone, the weapon stays.
        let sim = shield_duel_with(6, 5.0, [-1.0, 0.0], true, 6);
        let b = sim.bullets.first().expect("the round is coming back");
        assert_eq!(b.owner, 1, "caught");
        assert!(b.vel[0] < 0.0);
        assert_eq!(b.weapon, 6);
        assert_eq!(b.pierce, 0, "a caught round pierces nothing");
        assert_eq!(b.hit_mask, 0);
        // The sniper round (2 damage) came home: a body hit, not a kill.
        let sim = shield_duel_with(6, 5.0, [-1.0, 0.0], true, 20);
        assert_eq!(player(&sim, 0).hp, MAX_HP - 2);
        assert_eq!(player(&sim, 1).hp, MAX_HP);
    }

    #[test]
    fn a_reflected_round_keeps_its_weapon_and_gravity() {
        // A revolver slug (0.5 m a tick, so caught around tick 8) read on
        // ticks 10 and 11: still a revolver slug, still falling faster.
        let a = shield_duel_with(5, 5.0, [-1.0, 0.0], true, 10);
        let b = shield_duel_with(5, 5.0, [-1.0, 0.0], true, 11);
        let (ra, rb) = (
            a.bullets.first().expect("in flight at 10"),
            b.bullets.first().expect("in flight at 11"),
        );
        assert_eq!((ra.owner, ra.weapon), (1, 5));
        assert!(ra.vel[0] < 0.0, "coming back");
        assert!(ra.vy < 0.0, "level fire under gravity is falling");
        assert!(rb.vy < ra.vy, "and still accelerating down after the catch");
        assert_eq!(ra.dmg, 2, "damage rides along");
    }

    #[test]
    fn a_rocket_on_a_raised_shield_spares_the_holder_and_hits_the_flank() {
        // The holder faces the rocket with the plate up; a third player
        // stands 1.5 m beside them. The rocket goes off on the plate: the
        // holder keeps 3, the flank loses 2, and nothing comes back at the
        // shooter, who is out of the splash at four metres.
        let mut sim = open_sim(1, 3);
        arm(&mut sim.players[0], 7);
        let spots = [
            (0, [0.0, 0.0], 0.0),
            (1, [5.0, 0.0], 0.0),
            (2, [5.0, 1.5], 0.0),
        ];
        let mut inputs = HashMap::new();
        for t in 0..30u32 {
            hold(&mut sim, &spots);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            inputs.insert(
                1,
                PlayerIn {
                    aim: [-1.0, 0.0],
                    shield: true,
                    ..Default::default()
                },
            );
            step_with(&mut sim, &inputs);
        }
        assert_eq!(player(&sim, 1).hp, MAX_HP, "the holder is spared");
        assert_eq!(player(&sim, 2).hp, MAX_HP - 2, "the flank eats the splash");
        assert_eq!(player(&sim, 0).hp, MAX_HP, "nothing came back");
        assert!(sim.bullets.is_empty(), "a rocket never reflects");
        assert!(sim.players.iter().all(|p| p.alive));
    }

    #[test]
    fn a_direct_rocket_hit_kills_outright() {
        let mut sim = open_sim(1, 2);
        arm(&mut sim.players[0], 7);
        let spots = [(0, [0.0, 0.0], 0.0), (1, [5.0, 0.0], 0.0)];
        let mut inputs = HashMap::new();
        let mut blasts = Vec::new();
        let mut hits = Vec::new();
        for t in 0..30u32 {
            hold(&mut sim, &spots);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            step_with(&mut sim, &inputs);
            blasts.extend(sim.blasts.iter().copied());
            hits.extend(sim.hits.iter().copied());
            if !sim.events.is_empty() {
                assert_eq!(sim.events, vec![(0, 1)]);
            }
        }
        assert_eq!(hits, vec![(0, 1, 3, false)], "one direct hit, the full bar");
        assert_eq!(blasts.len(), 1, "one blast");
        assert_eq!(blasts[0].1, 0, "owned by the shooter");
        assert!(!player(&sim, 1).alive);
        assert_eq!(player(&sim, 0).score, 1);
    }

    /// A rocket fired level along +x into a container face at x 5, with
    /// targets wherever the test puts them. The rocket does 0.4 m a tick
    /// from a muzzle at 0.2, so its last free point is x 4.6 and that is
    /// where it goes off. Returns the sim after the blast.
    fn rocket_into_the_wall(targets: &[[f32; 2]], delay: u16, dodge_to: Option<[f32; 2]>) -> Sim {
        let mut sim = open_sim(1, 1 + targets.len() as u8);
        sim.obstacles = vec![Obstacle::boxed(
            Cover::Container,
            [5.0, -1.0],
            [6.0, 1.0],
            0.0,
            2.6,
        )];
        arm(&mut sim.players[0], 7);
        let mut spots = vec![(0, [0.0, 0.0], 0.0)];
        for (i, &t) in targets.iter().enumerate() {
            spots.push((i as u8 + 1, t, 0.0));
        }
        let idle = HashMap::new();
        // Fifteen ticks of history where the targets stand now.
        for _ in 0..15 {
            hold(&mut sim, &spots);
            step_with(&mut sim, &idle);
        }
        // The first target may dodge after the shooter has taken aim.
        if let Some(to) = dodge_to {
            spots[1].1 = to;
        }
        let mut inputs = HashMap::new();
        for t in 0..30u32 {
            hold(&mut sim, &spots);
            inputs.insert(
                0,
                PlayerIn {
                    delay_ticks: delay,
                    ..shot(t, [1.0, 0.0], 0.0)
                },
            );
            step_with(&mut sim, &inputs);
            if !sim.blasts.is_empty() {
                assert_eq!(sim.blasts.len(), 1);
                let ([x, y, z], owner) = sim.blasts[0];
                assert!(
                    (x - 4.6).abs() < 1e-3,
                    "went off at x {x}, not the last free point"
                );
                assert!(y > 1.0 && y < EYE_STAND, "at {y}, a little under the eye");
                assert!(z.abs() < 1e-6);
                assert_eq!(owner, 0);
                return sim;
            }
        }
        panic!("the rocket never went off");
    }

    #[test]
    fn a_rocket_detonates_on_cover_and_splashes_round_the_corner_only_with_line_of_sight() {
        // A target 2 m behind the container is inside the splash radius
        // and keeps every point: the container is between its chest and
        // the blast. A target off the container's side, whose chest sees
        // the blast, loses two.
        let sim = rocket_into_the_wall(&[[8.0, 0.0], [4.7, 1.9]], 0, None);
        assert_eq!(player(&sim, 1).hp, MAX_HP, "behind the container");
        assert_eq!(player(&sim, 2).hp, MAX_HP - 2, "beside it, in sight");
        assert_eq!(player(&sim, 0).hp, MAX_HP, "the shooter is out of range");
    }

    #[test]
    fn splash_falls_off_by_radius_two_then_one() {
        // From the blast at (4.6, 0) to the edge of each 0.6 hit circle:
        // 1.2 m is inside half the radius, 2.7 m is inside the radius, 3.9 m
        // is outside it.
        let sim = rocket_into_the_wall(&[[4.6, 1.8], [4.6, -3.3], [4.6, 4.5]], 0, None);
        assert_eq!(player(&sim, 1).hp, MAX_HP - 2, "inside 1.5 m");
        assert_eq!(player(&sim, 2).hp, MAX_HP - 1, "inside 3 m");
        assert_eq!(player(&sim, 3).hp, MAX_HP, "beyond 3 m");
    }

    #[test]
    fn splash_uses_the_rewound_body() {
        // The target stood beside the container while the shooter aimed,
        // then dodged 4 m away up the side. A shooter twelve ticks behind
        // saw it where it stood, and the blast is judged there: hurt. The
        // same dodge against a zero-delay shooter is clean.
        let sim = rocket_into_the_wall(&[[4.7, 1.9]], 12, Some([4.7, 6.0]));
        assert_eq!(
            player(&sim, 1).hp,
            MAX_HP - 2,
            "judged where the shooter saw it"
        );
        let sim = rocket_into_the_wall(&[[4.7, 1.9]], 0, Some([4.7, 6.0]));
        assert_eq!(
            player(&sim, 1).hp,
            MAX_HP,
            "in the present the dodge worked"
        );
    }

    #[test]
    fn the_shooter_eats_their_own_splash_and_it_is_not_a_frag() {
        // A rocket into the floor at the feet with one point left: the kill
        // event names the shooter twice, the death counts, the score does
        // not.
        let mut sim = open_sim(1, 1);
        arm(&mut sim.players[0], 7);
        sim.players[0].hp = 1;
        let mut inputs = HashMap::new();
        let mut events = Vec::new();
        for t in 0..5u32 {
            inputs.insert(0, shot(t, [1.0, 0.0], -MAX_PITCH));
            step_with(&mut sim, &inputs);
            events.extend(sim.events.iter().copied());
            if t == 0 {
                assert_eq!(sim.blasts.len(), 1, "went off on the first tick");
                assert_eq!(sim.blasts[0].0[1].to_bits(), 0.05f32.to_bits());
                assert_eq!(sim.hits, vec![(0, 0, 2, false)]);
            }
        }
        assert_eq!(events, vec![(0, 0)]);
        let p = player(&sim, 0);
        assert_eq!(p.score, 0, "not a frag");
        assert_eq!(p.death_count, 1);
        assert!(!p.alive);
    }

    #[test]
    fn a_rocket_detonates_at_its_ttl_at_the_floor_and_at_the_wall() {
        // Three exits, one blast each: the flight time runs out in the air,
        // the floor is crossed (blast lifted to 0.05), the wall is met (the
        // position clamped back inside it). No rocket ever just vanishes.
        let run = |spot: [f32; 2], y: f32, pitch: f32, cut_ttl: bool| -> ([f32; 3], usize) {
            let mut sim = open_sim(1, 1);
            arm(&mut sim.players[0], 7);
            let mut inputs = HashMap::new();
            let mut blasts = Vec::new();
            for t in 0..60u32 {
                hold(&mut sim, &[(0, spot, y)]);
                inputs.insert(0, shot(t, [1.0, 0.0], pitch));
                step_with(&mut sim, &inputs);
                blasts.extend(sim.blasts.iter().copied());
                if cut_ttl && t == 0 {
                    sim.bullets[0].ttl = 0.5 * FIXED_DT;
                }
            }
            assert!(sim.bullets.is_empty(), "the rocket is gone");
            assert_eq!(blasts.len(), 1, "exactly one blast");
            (blasts[0].0, blasts.len())
        };
        let (ttl, _) = run([-20.0, 0.0], 8.0, 0.0, true);
        assert!(ttl[1] > 8.0, "went off in the air at {ttl:?}");
        assert!(
            (ttl[0] - (-20.0 + 0.2 + 0.4)).abs() < 1e-3,
            "after one tick of flight: {ttl:?}"
        );
        let (floor, _) = run([0.0, 0.0], 0.0, -1.0, false);
        assert_eq!(floor[1].to_bits(), 0.05f32.to_bits(), "{floor:?}");
        assert!(floor[0] > 0.2 && floor[0] < 2.0, "{floor:?}");
        let (wall, _) = run([20.0, 0.0], 8.0, 0.0, false);
        let lim = ARENA_HALF - weapon_stats(7).radius;
        assert_eq!(wall[0].to_bits(), lim.to_bits(), "{wall:?}");
        assert!(wall[1] > 8.0, "{wall:?}");
    }

    #[test]
    fn two_sims_with_the_same_seed_and_inputs_agree_bit_for_bit() {
        // Four players on the yard, 600 ticks of a scripted input table
        // that fires, jumps, crouches, scopes, shields, swings, reloads and
        // cycles every weapon in the table, compared field by field as bits
        // after every tick. What this pins is that nothing in the step is
        // hidden state: no RNG, no hash-map order, no time.
        let level = Level::freight_yard();
        let mut a = Sim::from_level(&level, 7);
        let mut b = Sim::from_level(&level, 7);
        for id in 0..4 {
            a.add_player(id);
            b.add_player(id);
        }
        let script = |tick: u64, id: u8| -> PlayerIn {
            let hash = hash64(tick.wrapping_mul(8).wrapping_add(u64::from(id)) ^ 0xabcd);
            let (lift, turn) = unit_pair(hash);
            let ang = std::f32::consts::TAU * turn;
            let wander = std::f32::consts::TAU * (tick / 60) as f32 / 7.0;
            PlayerIn {
                mv: [wander.cos(), wander.sin()],
                aim: [ang.cos(), ang.sin()],
                pitch: (lift - 0.5) * 2.0 * MAX_PITCH,
                fire: hash & 1 == 0,
                sprint: hash & 16 != 0,
                crouch: hash & 4 != 0,
                reload: hash & 32 != 0,
                jump: tick % 50 == u64::from(id) * 7,
                shield: id == 2 && tick % 97 < 10,
                melee: tick % 123 == u64::from(id),
                ads: hash & 8 != 0,
                delay_ticks: ((hash >> 8) % 19) as u16,
            }
        };
        let mut rounds_seen = [false; 8];
        for tick in 0..600u64 {
            if tick % 75 == 0 {
                for sim in [&mut a, &mut b] {
                    for (k, p) in sim.players.iter_mut().enumerate() {
                        grant(p, 1 + ((tick / 75 + k as u64) % 7) as u8);
                    }
                }
            }
            a.step(&|id| script(tick, id));
            b.step(&|id| script(tick, id));
            assert_eq!(a.players.len(), b.players.len());
            for (pa, pb) in a.players.iter().zip(&b.players) {
                assert_eq!(
                    player_bits(pa),
                    player_bits(pb),
                    "tick {tick} player {}",
                    pa.id
                );
            }
            assert_eq!(a.bullets.len(), b.bullets.len(), "tick {tick}");
            for (ba, bb) in a.bullets.iter().zip(&b.bullets) {
                assert_eq!(bullet_bits(ba), bullet_bits(bb), "tick {tick}");
                rounds_seen[usize::from(ba.weapon)] = true;
            }
            assert_eq!(a.events, b.events);
            assert_eq!(a.hits, b.hits);
            assert_eq!(a.blasts, b.blasts);
            assert_eq!(a.loot_events, b.loot_events);
        }
        // The script really did fire every row.
        for id in 1..=WEAPON_COUNT {
            assert!(rounds_seen[usize::from(id)], "weapon {id} never flew");
        }
    }

    #[test]
    fn client_prediction_and_server_agree_on_every_bonk() {
        // The client predicts with the bare `move_circle` and
        // `step_vertical`; the server runs `Sim::step`. On the yard, for
        // 600 ticks of the same inputs, feet, speed and the box the head
        // met are the same tick by tick, and the server pays out exactly
        // when the client's prediction says the head met an armed block.
        let level = Level::freight_yard();
        let mut sim = Sim::from_level(&level, 7);
        sim.add_player(0);
        // Start on the dock under the king block: a hop from there bonks it.
        let start = ([0.0f32, 0.0f32], 1.2f32);
        sim.players[0].pos = start.0;
        sim.players[0].y = start.1;
        let script = |tick: u64| -> PlayerIn {
            let wander = std::f32::consts::TAU * (tick / 90) as f32 / 5.0;
            PlayerIn {
                mv: if tick < 100 {
                    [0.0, 0.0]
                } else {
                    [wander.cos() * 0.6, wander.sin() * 0.6]
                },
                aim: [1.0, 0.0],
                jump: tick % 45 == 10,
                crouch: tick % 200 > 170,
                sprint: tick % 300 > 250,
                ..Default::default()
            }
        };
        let (mut pos, mut y, mut vy) = (start.0, start.1, 0.0f32);
        let mut bonks = 0;
        let mut paid = 0;
        for tick in 0..600u64 {
            let input = script(tick);
            let armed_before: Vec<bool> =
                sim.loot.iter().map(|l| l.respawn_t <= FIXED_DT).collect();
            sim.step(&|_| input);
            // The client's replay of the same tick.
            let speed = stance_speed(input.sprint, input.crouch, input.shield);
            let npos = move_circle(pos, y, input.mv, speed, FIXED_DT, &sim.obstacles);
            let v = step_vertical(npos, y, vy, input.jump, FIXED_DT, &sim.obstacles);
            let predicted = v
                .bonked
                .filter(|&k| vy > 0.0 && sim.obstacles[k].kind == Cover::Loot);
            pos = npos;
            y = v.y;
            vy = v.vy;
            let p = &sim.players[0];
            assert_eq!(pos, p.pos, "tick {tick}");
            assert_eq!(y.to_bits(), p.y.to_bits(), "tick {tick}: {y} vs {}", p.y);
            assert_eq!(vy.to_bits(), p.vy.to_bits(), "tick {tick}");
            match predicted {
                Some(k) => {
                    bonks += 1;
                    let slot = sim.loot.iter().position(|l| l.obstacle == k).unwrap();
                    if armed_before[slot] {
                        paid += 1;
                        assert_eq!(sim.loot_events.len(), 1, "tick {tick}");
                        assert_eq!(
                            (sim.loot_events[0].0, sim.loot_events[0].1),
                            (0, slot as u8)
                        );
                    } else {
                        assert!(sim.loot_events.is_empty(), "tick {tick}: a dark block paid");
                    }
                }
                None => assert!(
                    sim.loot_events.is_empty(),
                    "tick {tick}: paid without a bonk"
                ),
            }
        }
        assert!(bonks >= 1, "the script never bonked the king block");
        assert!(paid >= 1, "the king block never paid");
    }

    #[test]
    fn a_floor_jump_under_a_train_zone_block_bonks_it_and_names_the_block() {
        // A crate, a block elsewhere, and the block under test: the event
        // names the block by its index among the blocks (1), not among the
        // obstacles (2). From the floor the head meets a 2.30 bottom three
        // ticks after the press: the Mario snap.
        let mut sim = block_sim(
            3,
            vec![
                Obstacle::boxed(Cover::Crate, [5.0, 5.0], [6.2, 6.2], 0.0, 1.2),
                block(-3.0, -3.0, 2.30),
                block(0.0, 0.0, 2.30),
            ],
        );
        assert_eq!(sim.loot.len(), 2);
        sim.add_player(0);
        let mut inputs = HashMap::new();
        let mut bonk_tick = None;
        for tick in 0..90u32 {
            sim.players[0].pos = [0.0, 0.0];
            inputs.insert(
                0,
                PlayerIn {
                    jump: tick == 0,
                    ..Default::default()
                },
            );
            step_with(&mut sim, &inputs);
            if let Some(&(who, slot, w)) = sim.loot_events.first() {
                assert!(bonk_tick.is_none(), "paid twice");
                bonk_tick = Some(tick);
                assert_eq!((who, slot), (0, 1), "the event names the block's slot");
                assert!(LOOT_POOL.contains(&w));
                assert_eq!(sim.loot[1].obstacle, 2);
            }
        }
        assert_eq!(
            bonk_tick,
            Some(3),
            "the head meets the block on the third tick"
        );
        assert!(sim.loot[1].respawn_t > 0.0, "the block went dark");
        assert_eq!(sim.loot[0].respawn_t, 0.0, "the other block is untouched");
    }

    #[test]
    fn walking_under_a_block_is_not_a_bonk() {
        let mut sim = block_sim(3, vec![block(0.0, 0.0, 2.30)]);
        sim.add_player(0);
        sim.players[0].pos = [-3.0, 0.0];
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                mv: [1.0, 0.0],
                ..Default::default()
            },
        );
        for _ in 0..60 {
            step_with(&mut sim, &inputs);
            assert!(sim.loot_events.is_empty(), "walking under the block paid");
            assert_eq!(sim.players[0].y, 0.0);
        }
        assert!(
            sim.players[0].pos[0] > 3.0,
            "walked through: {:?}",
            sim.players[0].pos
        );
        assert_eq!(sim.loot[0].respawn_t, 0.0);
    }

    #[test]
    fn a_block_is_bonked_only_from_below() {
        // Three ways to be near a block that are not a bonk: walking under
        // it, dropping past its side, and jumping while standing on top of
        // it (the perch). Only a head rising into its bottom pays.
        let mut sim = block_sim(3, vec![block(0.0, 0.0, 2.30)]);
        sim.add_player(0);
        let idle = HashMap::new();
        let mut jump = HashMap::new();
        jump.insert(
            0,
            PlayerIn {
                jump: true,
                ..Default::default()
            },
        );
        // Dropped from 6 m beside the block, just outside its footprint.
        sim.players[0].pos = [1.2, 0.0];
        sim.players[0].y = 6.0;
        for _ in 0..120 {
            step_with(&mut sim, &idle);
            assert!(sim.loot_events.is_empty(), "a drop past the side paid");
        }
        assert_eq!(sim.players[0].y, 0.0, "landed on the floor");
        // Perched on top, jumping.
        sim.players[0].pos = [0.0, 0.0];
        sim.players[0].y = 2.30 + LOOT_SIZE;
        for tick in 0..120 {
            step_with(&mut sim, if tick % 40 == 0 { &jump } else { &idle });
            assert!(sim.loot_events.is_empty(), "jumping off the top paid");
        }
        // And the same block from below does pay, so the three are not
        // trivially true.
        sim.players[0].pos = [0.0, 0.0];
        sim.players[0].y = 0.0;
        sim.players[0].vy = 0.0;
        let mut paid = false;
        for tick in 0..60 {
            step_with(&mut sim, if tick == 0 { &jump } else { &idle });
            paid |= !sim.loot_events.is_empty();
        }
        assert!(paid);
    }

    #[test]
    fn the_reward_never_repeats_the_gun_in_hand() {
        for holding in LOOT_POOL {
            for tick in 0..500u64 {
                assert_ne!(
                    loot_roll(7, tick, 0, holding),
                    holding,
                    "tick {tick} handed back the gun in hand"
                );
            }
        }
        let mut seen: Vec<u8> = (0..500u64)
            .map(|tick| loot_roll(7, tick, 0, SIDEARM))
            .collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen,
            LOOT_POOL.to_vec(),
            "holding the sidearm, every pool gun appears"
        );
    }

    #[test]
    fn the_roll_is_uniform_enough() {
        let mut counts = [0u32; 8];
        let n = 10_000u64;
        for tick in 0..n {
            counts[usize::from(loot_roll(11, tick, 3, SIDEARM))] += 1;
        }
        let mean = n as f32 / LOOT_POOL.len() as f32;
        for w in LOOT_POOL {
            let c = counts[usize::from(w)] as f32;
            assert!(
                (c - mean).abs() < mean * 0.2,
                "weapon {w} rolled {c} times against a mean of {mean}"
            );
        }
    }

    #[test]
    fn the_roll_is_a_pure_function_of_seed_tick_and_player() {
        let base = loot_roll(7, 100, 0, SIDEARM);
        assert_eq!(base, loot_roll(7, 100, 0, SIDEARM));
        // Any one input changing changes SOME roll: with a pool of five a
        // single tick can coincide, so look across a run of them.
        let differs =
            |f: &dyn Fn(u64) -> u8| (0..50u64).any(|t| f(t) != loot_roll(7, t, 0, SIDEARM));
        assert!(differs(&|t| loot_roll(8, t, 0, SIDEARM)), "the seed");
        assert!(differs(&|t| loot_roll(7, t + 1000, 0, SIDEARM)), "the tick");
        assert!(differs(&|t| loot_roll(7, t, 1, SIDEARM)), "the player");
        // And the spread roll on the same tick is a different number.
        assert_ne!(roll(7, 100, 0, SALT_LOOT), roll(7, 100, 0, 0));
    }

    #[test]
    fn a_used_block_is_dead_for_eighteen_seconds() {
        let mut sim = block_sim(3, vec![block(0.0, 0.0, 2.30)]);
        sim.add_player(0);
        let idle = HashMap::new();
        let mut jump = HashMap::new();
        jump.insert(
            0,
            PlayerIn {
                jump: true,
                ..Default::default()
            },
        );
        let mut paid_at: Vec<u32> = Vec::new();
        // The server's `State.loot` reading, before each press.
        let mut armed_at_press: Vec<bool> = Vec::new();
        let presses = [0u32, 63, 3 + (18.1 * 60.0) as u32];
        for tick in 0..=presses[2] + 10 {
            sim.players[0].pos = [0.0, 0.0];
            let press = presses.contains(&tick);
            if press {
                armed_at_press.push(sim.loot[0].respawn_t <= 0.0);
            }
            step_with(&mut sim, if press { &jump } else { &idle });
            if !sim.loot_events.is_empty() {
                paid_at.push(tick);
            }
        }
        assert_eq!(armed_at_press, vec![true, false, true]);
        assert_eq!(
            paid_at,
            vec![3, presses[2] + 3],
            "paid on the first and third bonk only"
        );
    }

    #[test]
    fn two_players_bonking_one_block_on_one_tick_pay_once() {
        let mut sim = block_sim(3, vec![block(0.0, 0.0, 2.30)]);
        sim.add_player(0);
        sim.add_player(1);
        let mut inputs = HashMap::new();
        let mut events = Vec::new();
        for tick in 0..30u32 {
            sim.players[0].pos = [0.0, 0.0];
            sim.players[1].pos = [0.2, 0.0];
            for id in 0..2 {
                inputs.insert(
                    id,
                    PlayerIn {
                        jump: tick == 0,
                        ..Default::default()
                    },
                );
            }
            step_with(&mut sim, &inputs);
            events.extend(sim.loot_events.iter().copied());
        }
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(
            (events[0].0, events[0].1),
            (0, 0),
            "the first in player order"
        );
        assert_eq!(sim.players[1].weapon, SIDEARM, "the second found it dark");
        assert!(LOOT_POOL.contains(&sim.players[0].weapon));
    }

    #[test]
    fn a_dead_player_cannot_bonk() {
        let mut sim = block_sim(3, vec![block(0.0, 0.0, 2.30)]);
        sim.add_player(0);
        sim.players[0].pos = [0.0, 0.0];
        sim.players[0].alive = false;
        sim.players[0].respawn_in = 5.0;
        let mut jump = HashMap::new();
        jump.insert(
            0,
            PlayerIn {
                jump: true,
                ..Default::default()
            },
        );
        for _ in 0..60 {
            step_with(&mut sim, &jump);
            assert!(sim.loot_events.is_empty(), "a corpse bonked");
        }
        assert_eq!(sim.loot[0].respawn_t, 0.0);
        assert_eq!(sim.players[0].y, 0.0);
    }
}
