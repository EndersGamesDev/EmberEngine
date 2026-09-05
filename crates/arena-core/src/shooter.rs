//! The arena shooter sim: pure, deterministic, fixed 60 Hz.
//!
//! Runs authoritatively on the server; clients render its broadcast state
//! and build the identical arena from the `Level` the lobby names (and, for
//! the seeded arena, from the lobby's seed).

pub const FIXED_DT: f32 = 1.0 / 60.0;
pub const ARENA_HALF: f32 = 24.0;
/// Invalid custom-level extents fall back to the original arena boundary.
#[must_use]
pub fn valid_arena_half(value: f32) -> f32 {
    if value.is_finite() && (4.0..=256.0).contains(&value) {
        value
    } else {
        ARENA_HALF
    }
}
pub const MOVE_SPEED: f32 = 4.0;
/// Base horizontal speed while airborne. Kept separate from `MOVE_SPEED` so
/// grounded movement can be tuned without changing the authored jump routes.
pub const AIR_MOVE_SPEED: f32 = 9.0;
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
/// of the weapon table IS the pistol and every shot test written before
/// v18 reads its world through these.
///
/// Historical v20 set the .45 ACP's real muzzle velocity and a 60 m range
/// (`docs/plans/arena-v20-realism.md` section 3.1). Protocol 19's handling
/// pass keeps that speed but limits the starter pistol to 30 m. It crosses
/// 4.67 m a tick, which is why every test on the segment below is exact
/// rather than sampled: the old 34 m/s was the speed at which a sampled
/// head band and an end-point cover test happened to be good enough. The
/// ttl is written as the range over the speed so the range is the number
/// that is exact and the seconds follow from it.
pub const BULLET_SPEED: f32 = 280.0;
pub const BULLET_R: f32 = 0.22;
pub const BULLET_TTL: f32 = 30.0 / BULLET_SPEED;
pub const RELOAD_SECS: f32 = 1.3;
pub const PAD_RESPAWN_SECS: f32 = 15.0;
pub const PAD_RADIUS: f32 = 1.3;
/// A pad is taken only with the feet below this.
///
/// The contact test is a horizontal circle, so without it a player standing
/// on a tunnel roof collected the pad 2.9 m under their boots through the
/// slab. Under every roof base (2.5); and a hop over a pad takes off and
/// lands below it, so a pad on open floor is still grabbed in passing.
pub const PAD_PICK_H: f32 = 1.0;

/// The modest close-range pistol everyone spawns with and falls back to,
/// with an infinite reserve. Loot improves firepower rather than ammunition access.
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
pub const ADS_SPREAD_AIR_MULT: f32 = 2.4;
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
    /// Muzzle speed, units per second; the horizontal `vel` magnitude at
    /// launch. Since v20 every bullet row is the cartridge's real muzzle
    /// velocity.
    pub speed: f32,
    /// The sustainer: how much the horizontal speed grows per second in
    /// flight, up to `speed_max`. Zero for every bullet; the rocket motor
    /// is the one row that has it, and it changes the round's direction
    /// as it goes because `vy` keeps only gravity.
    pub accel: f32,
    /// The speed the sustainer stops at. Equal to `speed` on a row with
    /// no sustainer, so `speed_max` is always the fastest the round flies.
    pub speed_max: f32,
    /// Seconds of flight; `speed * ttl` is the range of a round with no
    /// sustainer, and the rows below spell it as `range / speed` so the
    /// metres are the exact number.
    pub ttl: f32,
    /// Hit radius of the round itself.
    pub radius: f32,
    /// Base hip-fire cone half-angle, radians. A shot adds `bloom` radians
    /// of recoverable recoil spread, capped with the base at `spread_max`.
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

/// Shared server/client handling.
///
/// Timings describe a complete 0..1 sight
/// transition; optical zoom is true angular magnification, not a FOV angle.
/// Spread multipliers never change movement or the authored jump reach.
#[derive(Clone, Copy, Debug)]
pub struct WeaponHandling {
    pub ads_in_secs: f32,
    pub ads_out_secs: f32,
    pub optical_zoom: f32,
    pub crouch_spread: f32,
    pub moving_spread: f32,
    pub air_spread: f32,
    /// Minimum airborne half-cone, even through a settled sniper scope.
    pub air_floor: f32,
    /// Recoil cone radians recovered per second, including between shots.
    pub bloom_recovery: f32,
}

#[must_use]
pub const fn weapon_handling(id: u8) -> WeaponHandling {
    let (
        ads_in_secs,
        ads_out_secs,
        optical_zoom,
        crouch_spread,
        moving_spread,
        air_spread,
        air_floor,
        bloom_recovery,
    ) = match id {
        2 => (0.18, 0.12, 1.3, 0.72, 1.8, 2.3, 0.025, 0.028),
        3 => (0.24, 0.14, 1.5, 0.66, 1.9, 2.6, 0.035, 0.024),
        4 => (0.22, 0.13, 1.6, 0.66, 1.8, 2.4, 0.030, 0.020),
        5 => (0.19, 0.12, 1.4, 0.70, 1.85, 2.4, 0.028, 0.032),
        6 => (0.45, 0.20, 6.0, 0.60, 4.0, 3.0, 0.060, 0.040),
        7 => (0.30, 0.18, 2.0, 0.75, 1.9, 2.2, 0.035, 0.035),
        _ => (
            0.14,
            0.10,
            1.15,
            0.70,
            1.7,
            ADS_SPREAD_AIR_MULT,
            0.028,
            0.040,
        ),
    };
    WeaponHandling {
        ads_in_secs,
        ads_out_secs,
        optical_zoom,
        crouch_spread,
        moving_spread,
        air_spread,
        air_floor,
        bloom_recovery,
    }
}

/// Shared, finite-safe ADS progression. The authoritative sim calls this at
/// fixed dt; clients may render the same progression at their frame cadence.
#[must_use]
pub fn advance_ads(fraction: f32, held: bool, weapon: u8, dt: f32) -> f32 {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if !dt.is_finite() || dt <= 0.0 {
        return fraction;
    }
    let handling = weapon_handling(weapon);
    if held {
        (fraction + dt / handling.ads_in_secs).min(1.0)
    } else {
        (fraction - dt / handling.ads_out_secs).max(0.0)
    }
}

/// Effective half-angle in radians.
///
/// Crouch steadies a planted shooter only;
/// airborne floors prevent a zero-cone scoped jump. `moving` must describe
/// actual displacement, not a held direction while blocked by a wall.
#[must_use]
pub fn weapon_spread(
    weapon: u8,
    ads_fraction: f32,
    bloom: f32,
    crouch: bool,
    moving: bool,
    grounded: bool,
) -> f32 {
    spread_with_stats(
        &weapon_stats(weapon),
        &weapon_handling(weapon),
        ads_fraction,
        bloom,
        crouch,
        moving,
        grounded,
    )
}

fn spread_with_stats(
    stats: &WeaponStats,
    handling: &WeaponHandling,
    ads_fraction: f32,
    bloom: f32,
    crouch: bool,
    moving: bool,
    grounded: bool,
) -> f32 {
    let ads = if ads_fraction.is_finite() {
        ads_fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bloom = if bloom.is_finite() {
        bloom.max(0.0)
    } else {
        0.0
    };
    let mut cone =
        (stats.spread + bloom).min(stats.spread_max) * (1.0 + (stats.ads_spread - 1.0) * ads);
    if crouch && grounded {
        cone *= handling.crouch_spread;
    }
    if moving {
        cone *= handling.moving_spread;
    }
    if !grounded {
        cone = (cone * handling.air_spread).max(handling.air_floor);
    }
    cone
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
            // 9x19 out of a short barrel: 380 m/s, 40 m.
            speed: 380.0,
            accel: 0.0,
            speed_max: 380.0,
            ttl: 40.0 / 380.0,
            radius: BULLET_R,
            spread: 0.024,
            bloom: 0.006,
            spread_max: 0.085,
            ads_spread: 0.40,
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
            // 7.62x39: 715 m/s, 80 m.
            speed: 715.0,
            accel: 0.0,
            speed_max: 715.0,
            ttl: 80.0 / 715.0,
            radius: BULLET_R,
            spread: 0.022,
            bloom: 0.006,
            spread_max: 0.075,
            ads_spread: 0.28,
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
            // 5.56x45: 880 m/s, 80 m.
            speed: 880.0,
            accel: 0.0,
            speed_max: 880.0,
            ttl: 80.0 / 880.0,
            radius: BULLET_R,
            spread: 0.019,
            bloom: 0.004,
            spread_max: 0.060,
            ads_spread: 0.28,
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
            // .454 Casull: 450 m/s, 60 m, and the real g. The drop at 60
            // m is 0.09 m; the v18 "lead up" identity went with the v18
            // speed, because realism was what was asked for.
            speed: 450.0,
            accel: 0.0,
            speed_max: 450.0,
            ttl: 60.0 / 450.0,
            radius: BULLET_R,
            spread: 0.020,
            bloom: 0.014,
            spread_max: 0.050,
            ads_spread: 0.30,
            gravity: -9.81,
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
            // .338 Lapua: 900 m/s, 120 m, the real g. 15 m a tick, which
            // the v18 sampled head band could not have taken (60 m/s was
            // its ceiling); the exact overlap test in the sweep is what
            // lets the real number ship.
            speed: 900.0,
            accel: 0.0,
            speed_max: 900.0,
            ttl: 120.0 / 900.0,
            radius: BULLET_R,
            spread: 0.075,
            bloom: 0.025,
            spread_max: 0.10,
            ads_spread: 0.025,
            gravity: -9.81,
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
            // The booster leaves the tube at 120 m/s and the sustainer
            // adds 180 m/s over the first half second, to 300. Inside a
            // 48 m arena the cap is only ever reached by a round that
            // has already left it; the numbers are the launcher's.
            speed: 120.0,
            accel: 360.0,
            speed_max: 300.0,
            ttl: 5.0,
            radius: 0.35,
            spread: 0.027,
            bloom: 0.020,
            spread_max: 0.050,
            ads_spread: 0.40,
            gravity: -3.0,
            pierce: 0,
            kind: Projectile::Rocket,
            splash_r: 3.0,
            reload: 2.4,
        },
        // The sidearm: the pistol, through the same constants the
        // pre-v18 shot tests read.
        _ => WeaponStats {
            name: "Sidearm",
            cooldown: 0.32,
            mag: 6,
            reserve: RESERVE_INFINITE,
            damage: 1,
            speed: BULLET_SPEED,
            accel: 0.0,
            speed_max: BULLET_SPEED,
            ttl: BULLET_TTL,
            radius: BULLET_R,
            spread: 0.026,
            bloom: 0.010,
            spread_max: 0.055,
            ads_spread: 0.42,
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

/// The slab test itself: where a segment `a -> b` enters the box
/// `[lo, hi]`, as a parameter in `[0, 1]`, and through which face.
///
/// The face is the axis whose near plane set the final entry parameter,
/// signed toward the outside the segment came from; a segment that starts
/// inside the box enters at 0 through no face and reports a zero normal.
/// Arithmetic only, so it is safe wherever the sweep runs.
fn slab_entry(a: [f32; 3], b: [f32; 3], lo: [f32; 3], hi: [f32; 3]) -> Option<(f32, [i8; 3])> {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    let mut normal = [0i8; 3];
    for axis in 0..3 {
        let d = b[axis] - a[axis];
        if d.abs() < 1e-9 {
            // Parallel to this slab: inside it or never.
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return None;
            }
            continue;
        }
        let mut near = (lo[axis] - a[axis]) / d;
        let mut far = (hi[axis] - a[axis]) / d;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        if near > t0 {
            t0 = near;
            normal = [0; 3];
            // Travelling +d meets the low face, whose outward normal is
            // negative on this axis.
            normal[axis] = if d > 0.0 { -1 } else { 1 };
        }
        t1 = t1.min(far);
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, normal))
}

/// Exact slab test of a 3D segment `a -> b` against a box: the parameter
/// along the segment at which it enters, or `None` when it misses.
///
/// The box is `[min, max] x [base, h]`. This is the test the v20 sweep
/// ends a round with (the smallest entry over every box, the floor and the
/// wall is where the round stops), so it has to be exact and it has to be
/// arithmetic only: a rocket's splash reads it too, to ask whether a body
/// has line of sight to the blast.
#[must_use]
pub fn segment_box_entry(a: [f32; 3], b: [f32; 3], o: &Obstacle) -> Option<f32> {
    segment_box_entry_face(a, b, o).map(|(t, _)| t)
}

/// `segment_box_entry` with the face the segment came in through, as an
/// axis-aligned outward normal, which is what a client needs to lay an
/// impact mark flat on the surface.
#[must_use]
pub fn segment_box_entry_face(a: [f32; 3], b: [f32; 3], o: &Obstacle) -> Option<(f32, [i8; 3])> {
    slab_entry(
        a,
        b,
        [o.min[0], o.base, o.min[1]],
        [o.max[0], o.h, o.max[1]],
    )
}

/// Does the segment meet the box at all: `segment_box_entry` is some.
#[must_use]
pub fn segment_hits_box(a: [f32; 3], b: [f32; 3], o: &Obstacle) -> bool {
    segment_box_entry(a, b, o).is_some()
}

/// `segment_hits_box` against every box: does any cover lie on the line?
#[must_use]
pub fn segment_hits_cover(a: [f32; 3], b: [f32; 3], obstacles: &[Obstacle]) -> bool {
    obstacles.iter().any(|o| segment_hits_box(a, b, o))
}

/// How a round ended, on `ShotEvent.hit` and on the wire as `S2C::Shot.hit`.
/// Flight time ran out.
pub const SHOT_EXPIRED: u8 = 0;
/// Stopped by a cover box; `cover` names its kind.
pub const SHOT_COVER: u8 = 1;
/// Hit a body; `victim` names it. A pierced body gets one of these and the
/// round goes on.
pub const SHOT_BODY: u8 = 2;
/// Met a raised shield; `victim` is the holder. A bullet is reflected and
/// starts a new segment there, a rocket goes off there.
pub const SHOT_SHIELD: u8 = 3;
/// Into the floor.
pub const SHOT_FLOOR: u8 = 4;
/// Into the arena wall.
pub const SHOT_WALL: u8 = 5;
/// `ShotEvent.cover` and `ShotEvent.victim` when there is none.
pub const SHOT_NONE: u8 = 255;

/// A round's segment ended: where it started, where it stopped, what it met.
///
/// One per segment: a reflected round produces one at the plate and then a
/// fresh one from there, a pierced round one per body and one at its end.
/// The client draws a tracer along `from -> to` and an impact at `to`; the
/// server forwards it as `S2C::Shot`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotEvent {
    pub owner: u8,
    pub weapon: u8,
    /// The muzzle at launch, or the plate or body the previous segment
    /// ended at.
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// One of the `SHOT_*` kinds.
    pub hit: u8,
    /// `Cover::index` of the box when `hit == SHOT_COVER`, else `SHOT_NONE`.
    pub cover: u8,
    /// The player id when `hit` is `SHOT_BODY` or `SHOT_SHIELD`, else
    /// `SHOT_NONE`.
    pub victim: u8,
    /// The outward normal of the surface met, one of the six axis
    /// directions: the entry face of a box, up on the floor, inward off
    /// the arena wall. Zero on a body, a shield and an expiry.
    pub normal: [i8; 3],
}

/// The standoff a rocket's blast is pushed back off the face it hit, along
/// the face normal, so the line-of-sight test from the blast to each body
/// does not start exactly on the box and read as inside it.
pub const BLAST_STANDOFF: f32 = 0.02;

/// Active bullets per player, so holding fire can't flood the state.
///
/// Counted by owner, and a reflected round changes owner, so rounds you
/// caught on the shield sit against your own cap until they expire. Left
/// that way deliberately: you cannot fire behind a raised shield anyway, no
/// round outlives its row's ttl (a fifth of a second for the sidearm), and
/// being briefly short of the cap after catching ten rounds is a fair
/// price for having caught them. Telling the two apart would need a flag
/// on `Bullet`.
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

impl Cover {
    /// The kind as the byte `ShotEvent.cover` and `S2C::Shot.cover` carry:
    /// its position in the declaration above, which is append-only for
    /// exactly this reason (the decode default and every match rely on it
    /// too). `Container` is 0, `Loot` is 8.
    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// The kind an `S2C::Shot.cover` byte names, or `None` for `SHOT_NONE`
    /// and for an index this build does not know (a later kind appended by
    /// a newer peer), so the client picks a fallback material rather than
    /// a wrong one.
    #[must_use]
    pub const fn from_index(i: u8) -> Option<Self> {
        Some(match i {
            0 => Self::Container,
            1 => Self::Crate,
            2 => Self::Ammo,
            3 => Self::Sandbag,
            4 => Self::Wall,
            5 => Self::Roof,
            6 => Self::Rubble,
            7 => Self::Plinth,
            8 => Self::Loot,
            _ => return None,
        })
    }
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

/// Prefer the requested slot, then a free pocket in stable cyclic order.
/// Custom levels with too few pockets fall back to the greatest clearance.
fn available_spawn(
    spawns: &[[f32; 2]],
    slot: u32,
    players: &[PlayerSt],
    exclude: Option<u8>,
) -> [f32; 2] {
    let count = if spawns.is_empty() {
        MAX_PLAYERS
    } else {
        spawns.len()
    };
    let mut best = spawn_from(spawns, slot);
    let mut clearance = -1.0_f32;
    for offset in 0..count {
        #[allow(clippy::cast_possible_truncation)]
        let candidate = spawn_from(spawns, slot.wrapping_add(offset as u32));
        let nearest = players
            .iter()
            .filter(|p| p.alive && Some(p.id) != exclude && p.y < BODY_H_STAND)
            .map(|p| (p.pos[0] - candidate[0]).powi(2) + (p.pos[1] - candidate[1]).powi(2))
            .fold(f32::INFINITY, f32::min);
        if nearest >= (2.0 * PLAYER_R + 0.1).powi(2) {
            return candidate;
        }
        if nearest > clearance {
            best = candidate;
            clearance = nearest;
        }
    }
    best
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
/// Breakwater Harbor: the hand-authored 96 m port arena.
pub const MAP_HARBOR: &str = "harbor";

/// The rules a lobby plays by: who a round may hit, how a point is earned
/// and when the round ends (`docs/plans/arena-v19-modes.md`).
///
/// Chosen at creation beside the map and carried by name on the wire
/// (`CreateLobby.mode`, `LobbyInfo.mode`, `GameJoined.mode`), so the next
/// mode is an additive string rather than a bump, exactly as the map is.
/// The sim resolves the name once, in `Sim::from_level`, and every rule
/// below reads the enum; nothing reads the string twice.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GameMode {
    /// Every player for themselves, the v18 game with an end: first to
    /// `FFA_FRAG_LIMIT` frags wins the round.
    #[default]
    Ffa,
    /// Blue (team 0) against red (team 1): no friendly fire, spawns split
    /// by side, and the team's frag total is the score; first team to
    /// `TDM_FRAG_LIMIT` wins.
    Tdm,
    /// King of the hill: alone on the level's `Hill` earns a point a
    /// second, contested earns nothing, first to `HILL_LIMIT` wins. Frags
    /// still happen and are counted in `PlayerSt::frags`, but the hill is
    /// the score.
    Hill,
}

impl GameMode {
    /// The mode a wire name resolves to.
    ///
    /// The empty string is free for all because that is what an absent
    /// field decodes to and what a peer that predates modes plays. A name
    /// this build does not know is `None`, never a fallback: the server
    /// refuses it with `Error("unknown mode")`, so a typo on a page is
    /// told rather than quietly handed a different game.
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "" | "ffa" => Some(Self::Ffa),
            "tdm" => Some(Self::Tdm),
            "hill" => Some(Self::Hill),
            _ => None,
        }
    }

    /// The name that travels on the wire; `from_name` inverts it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ffa => "ffa",
            Self::Tdm => "tdm",
            Self::Hill => "hill",
        }
    }
}

/// Frags that end a free-for-all round.
pub const FFA_FRAG_LIMIT: u32 = 20;
/// A team's frags that end a team deathmatch round.
pub const TDM_FRAG_LIMIT: u32 = 30;
/// Hill points that end a king-of-the-hill round.
pub const HILL_LIMIT: u32 = 60;
/// Seconds alone on the hill per hill point.
pub const HILL_TICK_SECS: f32 = 1.0;
/// The pause between a round's end and the next round's start, during
/// which everyone keeps moving and shooting but nothing scores.
pub const ROUND_PAUSE_SECS: f32 = 10.0;
/// `Sim::hill_holder` (and `State.hill`) when nobody stands on the hill.
pub const HILL_FREE: u8 = 255;
/// `Sim::hill_holder` (and `State.hill`) when two or more stand on it.
/// Below `HILL_FREE` so `holder < HILL_CONTESTED` is "a player holds it".
pub const HILL_CONTESTED: u8 = 254;

/// The hill: a footprint and the height feet must be at to stand on it.
///
/// A level property beside the spawns rather than something derived from
/// a box, because the hill is a rule about where a body is and the box it
/// happens to sit on (the dock, the plinth) is cover like any other; the
/// seeded arena's hill is an open square with no box under it at all.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hill {
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub top: f32,
}

impl Hill {
    /// Whether a body with its centre at `pos` and its feet at `feet`
    /// stands on the hill. The 0.05 slack under `top` is for feet resting
    /// on a box top through `support_height`, which lands them exactly on
    /// it, and for a level whose hill is the floor.
    #[must_use]
    pub fn stands_on(&self, pos: [f32; 2], feet: f32) -> bool {
        feet >= self.top - 0.05
            && pos[0] >= self.min[0]
            && pos[0] <= self.max[0]
            && pos[1] >= self.min[1]
            && pos[1] <= self.max[1]
    }
}

/// Trench City's hill: the statue's plinth (`TRENCH_CENTRE`), reached
/// from the sandbags around it.
const TRENCH_HILL: Hill = Hill {
    min: [-1.6, -1.6],
    max: [1.6, 1.6],
    top: 2.2,
};

/// The seeded arena's hill: the open centre at floor level, a placeholder
/// square so the seeded tests can drive the mode without an authored map.
const SEEDED_HILL: Hill = Hill {
    min: [-2.0, -2.0],
    max: [2.0, 2.0],
    top: 0.0,
};

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
/// `pads`, `decor` and `hill` default on decode so a level written before
/// they existed still loads (and plays without pads, as its author left
/// it, and without a hill, which the server refuses king of the hill on).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Level {
    pub arena_half: f32,
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<[f32; 2]>,
    #[serde(default)]
    pub pads: Vec<[f32; 2]>,
    #[serde(default)]
    pub decor: Vec<Decor>,
    /// Where king of the hill is played on this level; `None` for a level
    /// that has no such place, on which the mode cannot be created.
    #[serde(default)]
    pub hill: Option<Hill>,
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
/// middle 12 m is the first box with a bottom above the floor, and since
/// v18 a loot block hangs at the mouth of the tunnel so the tunnels stay
/// contested: the v13 pad that sat under the roof is gone, because the
/// Mario mechanic has to exist on every map and a second reward rule on one
/// of them was never wanted. The three 2.6-tall containers each have a
/// crate within 0.5 of a face and an ammo box within 0.5 of the crate: a
/// climbing chain, floor -> 0.55 -> 1.2 -> 2.6, each step under the jump
/// apex. The 5.2-tall stacked pair is a landmark nobody can climb.
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
    // The loot block, in the open inner square 0.3 m south of the inner
    // wall's gap, so it stands at the mouth of the tunnel: walked under with
    // 0.44 m of headroom, bonked from the floor on the fourth integrator
    // step like the yard's train-zone blocks, and over no roof, so the
    // lowest-base clamp rule never has two boxes to choose between here.
    (Cover::Loot, [-0.5, 9.3], [0.5, 10.3], 2.3, 3.3),
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
            hill: Some(SEEDED_HILL),
        }
    }

    /// The authored v13 arena: `docs/plans/arena-v13-trench-city.md`
    /// section 4, tables transcribed above, four-fold symmetric about the
    /// origin. Spawns are listed one per side first (slots 0..4 land on
    /// four different sides) and then the second of each side. No pads
    /// since v18: the four loot blocks in `TRENCH_NORTH` are the map's only
    /// reward, exactly as on Freight Yard, and the seeded arena is the only
    /// level that still carries pads.
    #[must_use]
    pub fn trench_city() -> Self {
        let mut obstacles: Vec<Obstacle> = TRENCH_CENTRE
            .iter()
            .map(|&(kind, min, max, base, h)| Obstacle::boxed(kind, min, max, base, h))
            .collect();
        for turns in 0..4 {
            for &(kind, min, max, base, h) in TRENCH_NORTH {
                let mut o = Obstacle::boxed(kind, min, max, base, h);
                for _ in 0..turns {
                    o = rot90_box(&o);
                }
                obstacles.push(o);
            }
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
            pads: Vec::new(),
            decor: trench_city_decor(),
            hill: Some(TRENCH_HILL),
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
        } else if map == MAP_HARBOR {
            Self::harbor()
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

    /// Team `team`'s spawns in team deathmatch: team 0 the level's spawns
    /// with `z > 0`, team 1 the rest, and the whole list when a half would
    /// be empty so a level authored with every spawn on one side still
    /// starts both teams. Slot order inside the half is `spawn_from`'s.
    /// The client never needs this: the server places every player and
    /// the wire carries the position.
    #[must_use]
    pub fn spawns_for(&self, team: u8) -> Vec<[f32; 2]> {
        let half: Vec<[f32; 2]> = self
            .spawns
            .iter()
            .copied()
            .filter(|s| (s[1] > 0.0) == (team == 0))
            .collect();
        if half.is_empty() {
            self.spawns.clone()
        } else {
            half
        }
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
    reset_handling(p);
}

const fn reset_handling(p: &mut PlayerSt) {
    p.ads_fraction = 0.0;
    p.bloom = 0.0;
    p.spread = weapon_stats(p.weapon).spread;
}

/// Is this spot blocked for a player whose feet are at `y`?
///
/// The legacy arena wall always blocks; a box only blocks while the player's feet are below its
/// top (by more than a step), so you can walk across boxes you have jumped
/// onto - AND while their head is above its bottom, so you can walk under a
/// box that starts above you. The head is always the standing one: crouch
/// is cosmetic to movement here as everywhere, and a rule that let a
/// crouched player under a lower roof would need `move_circle` to know the
/// stance on both peers. For `base == 0` the head clause is always true.
#[must_use]
pub fn blocked(pos: [f32; 2], y: f32, r: f32, obstacles: &[Obstacle]) -> bool {
    blocked_in(pos, y, r, obstacles, ARENA_HALF)
}

/// The same body/cover rule inside the supplied level boundary.
#[must_use]
pub fn blocked_in(pos: [f32; 2], y: f32, r: f32, obstacles: &[Obstacle], arena_half: f32) -> bool {
    let arena_half = valid_arena_half(arena_half);
    if !pos[0].is_finite()
        || !pos[1].is_finite()
        || pos[0].abs() > arena_half - r
        || pos[1].abs() > arena_half - r
    {
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
fn speed_at(base: f32, sprint: bool, crouch: bool, shield: bool) -> f32 {
    base * if crouch {
        CROUCH_MULT
    } else if sprint && !shield {
        SPRINT_MULT
    } else {
        1.0
    }
}

#[must_use]
pub fn stance_speed(sprint: bool, crouch: bool, shield: bool) -> f32 {
    speed_at(MOVE_SPEED, sprint, crouch, shield)
}

/// The horizontal speed to use before this movement step.
///
/// Grounded players use the current walking speed. A jump launches at the
/// legacy air speed on its very first tick, and stays there until landing, so
/// authored routes keep their horizontal jump reach while ordinary movement
/// slows down. This is shared by the server and client prediction.
// These independent input flags and body fields mirror shared prediction;
// grouping them would obscure which pre-step state controls jump reach.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
#[must_use]
pub fn movement_speed(
    pos: [f32; 2],
    y: f32,
    vy: f32,
    jump: bool,
    sprint: bool,
    crouch: bool,
    shield: bool,
    obstacles: &[Obstacle],
) -> f32 {
    // A body can overlap the edge of a roof while still rising or falling.
    // It is not standing there until the vertical step has resolved its
    // velocity to zero, so keep its full air speed through that landing.
    let grounded = vy == 0.0 && y <= support_height(pos, PLAYER_R, y, obstacles) + 1e-3;
    let base = if grounded && !jump {
        MOVE_SPEED
    } else {
        AIR_MOVE_SPEED
    };
    speed_at(base, sprint, crouch, shield)
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
    move_circle_in(pos, y, mv, speed, dt, obstacles, ARENA_HALF)
}

/// Body movement inside a level's walls, shared by prediction and authority.
#[must_use]
pub fn move_circle_in(
    pos: [f32; 2],
    y: f32,
    mv: [f32; 2],
    speed: f32,
    dt: f32,
    obstacles: &[Obstacle],
    arena_half: f32,
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
    let pos = if blocked_in(try_x, y, PLAYER_R, obstacles, arena_half) {
        pos
    } else {
        try_x
    };
    let try_z = [pos[0], pos[1] + mv[1] * speed * dt];
    if blocked_in(try_z, y, PLAYER_R, obstacles, arena_half) {
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
    /// Aiming down the sights (RMB or LT). HELD, like `shield`: requests a
    /// timed authoritative sight raise, not instant precision on this tick.
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
    /// The mode's score: frags in free for all and team deathmatch, hill
    /// points in king of the hill. Reset to zero when a round restarts.
    pub score: u32,
    /// Frags, whatever the mode, so the scoreboard shows them beside
    /// deaths when the hill is the score. Reset with `score`.
    pub frags: u32,
    /// 0 (blue) or 1 (red) in team deathmatch, assigned on join to the
    /// smaller team; 0 for everyone in every other mode.
    pub team: u8,
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
    /// Rounds fired since the magazine was last filled, for bookkeeping.
    /// Server-only; resets on reload, grant, respawn and the dry swap.
    /// Accuracy uses recovering `bloom`, never this lifetime shot count.
    fired: u8,
    /// Authoritative sight raise, 0 = hip and 1 = fully sighted.
    pub ads_fraction: f32,
    /// Current effective shot half-cone in radians, broadcast for the reticle.
    pub spread: f32,
    /// Recoverable recoil cone radians. Independent of ammunition spent.
    bloom: f32,
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

impl PlayerSt {
    /// Recoverable recoil radians before ADS/stance multipliers, for state
    /// reporting. Reading it does not advance recovery or change shot rules.
    #[must_use]
    pub const fn recoil_bloom(&self) -> f32 {
        self.bloom
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bullet {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    /// Height above the arena floor, and its rate of change. Height is a
    /// scalar RIDING ALONGSIDE the 2D path rather than a third component of
    /// it: `vel` keeps its row's full speed at any elevation, so horizontal
    /// range, flight time and every timing-sensitive test behave exactly as
    /// before. The ray's DIRECTION is still exactly the shooter's look
    /// direction, because `vy / speed == tan(pitch)` at launch.
    pub y: f32,
    pub vy: f32,
    pub ttl: f32,
    /// Where the current segment began: the muzzle at launch, then the
    /// plate or the pierced body the last `ShotEvent` ended at. Carried on
    /// the round so the event that ends it can say where it came from
    /// without the sim keeping a side table.
    pub from: [f32; 3],
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

impl Bullet {
    /// The event that ends this round's current segment at `to`.
    const fn ended(
        &self,
        to: [f32; 3],
        hit: u8,
        cover: u8,
        victim: u8,
        normal: [i8; 3],
    ) -> ShotEvent {
        ShotEvent {
            owner: self.owner,
            weapon: self.weapon,
            from: self.from,
            to,
            hit,
            cover,
            victim,
            normal,
        }
    }
}

/// What the world puts in a round's way this tick: the first thing on the
/// segment among every box, the floor and the arena wall.
#[derive(Clone, Copy, Debug)]
struct WorldEnd {
    /// Parameter along the tick's segment.
    t: f32,
    /// The point on the surface.
    at: [f32; 3],
    /// `SHOT_COVER`, `SHOT_FLOOR` or `SHOT_WALL`.
    hit: u8,
    cover: u8,
    normal: [i8; 3],
}

/// The first thing on the segment `p0 -> p1` (heights `y0 -> y1`), or
/// `None` when the tick is clear.
///
/// The round is a point against the world: a box is met where the segment
/// enters it, the floor where the height crosses zero, and the wall where
/// the position crosses `arena_half - radius` on either axis (the radius
/// keeps a rocket's blast drawn inside the wall, as it always was). The
/// smallest parameter wins; a tie keeps the earlier of floor, wall, boxes
/// in list order, so the answer is the same on every peer. The wall
/// coordinate is snapped to the wall exactly rather than recomputed from
/// the parameter, so the point is on the surface and not a rounding error
/// short of it.
fn world_end(
    p0: [f32; 2],
    p1: [f32; 2],
    y0: f32,
    y1: f32,
    radius: f32,
    obstacles: &[Obstacle],
    arena_half: f32,
) -> Option<WorldEnd> {
    let (sx, sz) = (p1[0] - p0[0], p1[1] - p0[1]);
    let at = |t: f32| [p0[0] + sx * t, y0 + (y1 - y0) * t, p0[1] + sz * t];
    let mut best: Option<WorldEnd> = None;
    let mut consider = |t: f32, at: [f32; 3], hit: u8, cover: u8, normal: [i8; 3]| {
        if best.is_none_or(|b| t < b.t) {
            best = Some(WorldEnd {
                t,
                at,
                hit,
                cover,
                normal,
            });
        }
    };
    if y1 < 0.0 && y0 > y1 {
        // y0 >= 0 (a round below the floor ended last tick) and y1 < 0,
        // so the crossing is in [0, 1) and the divisor is never zero.
        let t = (y0 / (y0 - y1)).clamp(0.0, 1.0);
        let p = at(t);
        consider(t, [p[0], 0.0, p[2]], SHOT_FLOOR, SHOT_NONE, [0, 1, 0]);
    }
    let lim = arena_half - radius;
    for axis in 0..2 {
        let (a, b) = (p0[axis], p1[axis]);
        let wall = if b > lim {
            lim
        } else if b < -lim {
            -lim
        } else {
            continue;
        };
        // b is outside and a inside (or the round ended last tick), so the
        // divisor is not zero.
        let t = ((wall - a) / (b - a)).clamp(0.0, 1.0);
        let mut p = at(t);
        p[if axis == 0 { 0 } else { 2 }] = wall;
        let mut normal = [0i8; 3];
        normal[if axis == 0 { 0 } else { 2 }] = if wall > 0.0 { -1 } else { 1 };
        consider(t, p, SHOT_WALL, SHOT_NONE, normal);
    }
    let a = [p0[0], y0, p0[1]];
    let b = [p1[0], y1, p1[1]];
    for o in obstacles {
        if let Some((t, normal)) = segment_box_entry_face(a, b, o) {
            consider(t, at(t), SHOT_COVER, o.kind.index(), normal);
        }
    }
    best
}

/// One round leaving the muzzle: the pure function behind the trigger.
///
/// Pure so a test can hand it any table row, and because everything that
/// decides where the round goes is an argument: the shooter's state, the
/// row, actual movement, whether the feet are planted, and the (seed, tick)
/// the cone is rolled from. The player's timed ADS fraction and recovering
/// bloom are read BEFORE this shot adds recoil. Magazine fill never stands
/// in for recoil: pausing a burst restores precision without reloading.
///
/// When the effective cone is zero the aim is used exactly as it was before
/// v18 and no rotation happens at all (useful for pure collision tests). A
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
    moving: bool,
    grounded: bool,
    seed: u64,
    tick: u64,
    delay: u16,
    out: &mut Vec<Bullet>,
) {
    let cone = spread_with_stats(
        stats,
        &weapon_handling(p.weapon),
        p.ads_fraction,
        p.bloom,
        p.crouch,
        moving,
        grounded,
    );
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
    let y = p.y + eye_h(p.crouch);
    out.push(Bullet {
        pos: muzzle,
        vel: [aim[0] * stats.speed, aim[1] * stats.speed],
        // Leaves at eye level and climbs or falls at the tangent of the aim
        // elevation, which makes the ray exactly the shooter's look
        // direction.
        y,
        vy: pitch.tan() * stats.speed,
        ttl: stats.ttl,
        from: [muzzle[0], y, muzzle[1]],
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
    /// Authoritative half-width of this level, shared by body and bullet walls.
    pub arena_half: f32,
    /// The rules this sim plays by, resolved once from the lobby's mode
    /// name in `from_level`.
    pub mode: GameMode,
    /// Frag totals per team in team deathmatch; both zero elsewhere.
    pub team_score: [u32; 2],
    /// Who stands alone on the hill, exactly as `State.hill` carries it:
    /// a player id, `HILL_FREE` or `HILL_CONTESTED`. `HILL_FREE` outside
    /// king of the hill.
    pub hill_holder: u8,
    /// The holder's accumulated seconds toward the next hill point; back
    /// to zero on every change of holder, so stepping off costs the
    /// partial second.
    hill_t: f32,
    /// Seconds left in the pause after a round; 0 while a round runs.
    pub round_pause: f32,
    /// Rounds completed since creation.
    pub round: u32,
    /// (winner, `is_team`) for a round that ended this step: a player id or
    /// a team index. Cleared each step like `events`.
    pub round_over: Vec<(u8, bool)>,
    /// The level's hill, read by the king-of-the-hill pass alone.
    pub hill: Option<Hill>,
    pub obstacles: Vec<Obstacle>,
    pub pads: Vec<Pad>,
    /// One per `Cover::Loot` obstacle, in obstacle order, so `State.loot`
    /// is index-aligned with the blocks every client derives from the level.
    pub loot: Vec<LootBlock>,
    /// The level's spawn points; empty means the seeded golden-angle ring.
    /// Placement goes through `Level::spawn` semantics for both the first
    /// spawn and every respawn.
    pub spawns: Vec<[f32; 2]>,
    /// `spawns` split by team through `Level::spawns_for`, indexed by
    /// team and read in team deathmatch only.
    team_spawns: [Vec<[f32; 2]>; 2],
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
    /// Every round segment that ended the last step, in sweep order (the
    /// bullets in list order, and for one round its bodies along the
    /// segment). What the client draws its tracers and impacts from,
    /// since a round at 280 m/s and up rarely lives long enough to appear
    /// in a state.
    pub shots: Vec<ShotEvent>,
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

/// Where a player of `team` spawns: the team's half of the list in team
/// deathmatch, the whole list in every other mode, so free for all places
/// players exactly as v18 did. A free function over the sim's fields so
/// the respawn path can call it while it holds one player mutably.
fn spawns_of<'a>(
    mode: GameMode,
    spawns: &'a [[f32; 2]],
    team_spawns: &'a [Vec<[f32; 2]>; 2],
    team: u8,
) -> &'a [[f32; 2]] {
    if mode == GameMode::Tdm {
        &team_spawns[usize::from(team & 1)]
    } else {
        spawns
    }
}

/// A player back to life at a fresh spawn with the sidearm: the one
/// respawn path, shared by the death timer and the round restart so the
/// two cannot drift. The spawn slot advances every time so consecutive
/// spawns walk the list.
const fn respawn(p: &mut PlayerSt, position: [f32; 2]) {
    p.deaths = p.deaths.wrapping_add(1);
    p.pos = position;
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
    reset_handling(p);
}

impl Sim {
    fn respawn_player(&mut self, index: usize) {
        let p = &self.players[index];
        let preferred = p
            .deaths
            .wrapping_add(1)
            .wrapping_mul(3)
            .wrapping_add(u32::from(p.id));
        let position = available_spawn(
            spawns_of(self.mode, &self.spawns, &self.team_spawns, p.team),
            preferred,
            &self.players,
            Some(p.id),
        );
        respawn(&mut self.players[index], position);
    }

    /// The seeded arena in free for all: exactly
    /// `from_level(&Level::from_seed(seed), seed, GameMode::Ffa)`, kept as
    /// the short spelling every existing test uses.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::from_level(&Level::from_seed(seed), seed, GameMode::Ffa)
    }

    /// A sim on an authored level: its obstacles, its pads (all active), its
    /// loot blocks (all armed), its spawns, its hill and its boundary.
    /// `seed` is the lobby's, which the server
    /// already mints and sends in `GameJoined`; every spread and loot roll
    /// hashes it. `mode` is the lobby's rules; king of the hill on a level
    /// with no hill plays as free for all, which the server refuses to
    /// create in the first place, so a sim never waits on a point nobody
    /// can earn.
    #[must_use]
    pub fn from_level(level: &Level, seed: u64, mode: GameMode) -> Self {
        let mode = if mode == GameMode::Hill && level.hill.is_none() {
            GameMode::Ffa
        } else {
            mode
        };
        Self {
            arena_half: valid_arena_half(level.arena_half),
            mode,
            team_score: [0; 2],
            hill_holder: HILL_FREE,
            hill_t: 0.0,
            round_pause: 0.0,
            round: 0,
            round_over: Vec::new(),
            hill: level.hill,
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
            team_spawns: [level.spawns_for(0), level.spawns_for(1)],
            players: Vec::new(),
            bullets: Vec::new(),
            events: Vec::new(),
            hits: Vec::new(),
            blasts: Vec::new(),
            shots: Vec::new(),
            loot_events: Vec::new(),
            seed,
            tick: 0,
            history: std::collections::VecDeque::new(),
        }
    }

    /// A player joins, on the smaller team in team deathmatch (ties by id
    /// parity, so two players joining an empty lobby face each other) and
    /// on team 0 everywhere else. Teams rebalance only on join:
    /// `remove_player` leaves them as they are.
    pub fn add_player(&mut self, id: u8) {
        // Player count is capped at eight by the public protocol.
        #[allow(clippy::cast_possible_truncation)]
        let mut slot = self.players.len() as u32;
        let team = if self.mode == GameMode::Tdm {
            let count = |t: u8| self.players.iter().filter(|p| p.team == t).count();
            match count(0).cmp(&count(1)) {
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Greater => 1,
                std::cmp::Ordering::Equal => id % 2,
            }
        } else {
            0
        };
        if self.mode == GameMode::Tdm {
            #[allow(clippy::cast_possible_truncation)]
            {
                slot = self.players.iter().filter(|p| p.team == team).count() as u32;
            }
        }
        let position = available_spawn(
            spawns_of(self.mode, &self.spawns, &self.team_spawns, team),
            slot,
            &self.players,
            None,
        );
        self.players.push(PlayerSt {
            id,
            pos: position,
            y: 0.0,
            vy: 0.0,
            aim: [1.0, 0.0],
            pitch: 0.0,
            hp: MAX_HP,
            score: 0,
            frags: 0,
            team,
            alive: true,
            crouch: false,
            shield: false,
            weapon: SIDEARM,
            ammo: weapon_stats(SIDEARM).mag,
            reserve: RESERVE_INFINITE,
            fired: 0,
            ads_fraction: 0.0,
            spread: weapon_stats(SIDEARM).spread,
            bloom: 0.0,
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

    /// Whether two ids are on one team. False for everyone outside team
    /// deathmatch, so every friendly-fire gate in `step` is a no-op in free
    /// for all and king of the hill and those modes resolve every hit
    /// exactly as v18 did. True for an id and itself in team deathmatch,
    /// which is why the splash rule below checks the owner separately.
    fn same_team(&self, a: u8, b: u8) -> bool {
        if self.mode != GameMode::Tdm {
            return false;
        }
        let team = |id: u8| self.players.iter().find(|p| p.id == id).map(|p| p.team);
        match (team(a), team(b)) {
            (Some(ta), Some(tb)) => ta == tb,
            _ => false,
        }
    }

    /// The next round: scores, frags, deaths and the team totals to zero,
    /// everyone alive at a fresh spawn with the sidearm through the one
    /// respawn path, every round cleared, every block and pad armed, the
    /// hill free. Stepped in the sim, so the server and any replay agree.
    fn restart_round(&mut self) {
        self.round += 1;
        self.team_score = [0; 2];
        self.hill_holder = HILL_FREE;
        self.hill_t = 0.0;
        self.bullets.clear();
        for slot in &mut self.loot {
            slot.respawn_t = 0.0;
        }
        for pad in &mut self.pads {
            pad.respawn_t = 0.0;
        }
        // Reserve the new placements in player order, not against old-round
        // bodies. This keeps all eight pockets distinct on team restarts.
        for p in &mut self.players {
            p.alive = false;
        }
        for index in 0..self.players.len() {
            let p = &mut self.players[index];
            p.score = 0;
            p.frags = 0;
            p.death_count = 0;
            p.respawn_in = 0.0;
            p.melee_cd = 0.0;
            p.shield = false;
            self.respawn_player(index);
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
            // A teammate is spared the splash; the owner's own splash still
            // hurts the owner, which is what makes a rocket at your feet a
            // self-kill and not a free escape.
            if q.id != b.owner && self.same_team(b.owner, q.id) {
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

    pub fn step(&mut self, inputs: &dyn Fn(u8) -> PlayerIn) {
        self.step_using(inputs, launch);
    }

    // The production caller always supplies `launch`. Collision-only tests
    // inject a zero-cone row without changing movement, fire, or hit ordering.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
    fn step_using<F>(&mut self, inputs: &dyn Fn(u8) -> PlayerIn, launch_round: F)
    where
        F: Fn(&PlayerSt, &WeaponStats, bool, bool, u64, u64, u16, &mut Vec<Bullet>),
    {
        self.events.clear();
        self.hits.clear();
        self.blasts.clear();
        self.shots.clear();
        self.loot_events.clear();
        self.round_over.clear();
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
                reset_handling(p);
                p.respawn_in -= dt;
                if p.respawn_in <= 0.0 {
                    self.respawn_player(i);
                }
                continue;
            }

            let (old_pos, feet_height, vertical_speed) =
                (self.players[i].pos, self.players[i].y, self.players[i].vy);
            // Shared movement code (also used by client prediction).
            // A jump keeps its authored air reach while the ground speed is
            // server-authoritative, so no client can claim a faster walk.
            let speed = movement_speed(
                old_pos,
                feet_height,
                vertical_speed,
                input.jump,
                input.sprint,
                input.crouch,
                input.shield,
                &self.obstacles,
            );
            let pos = move_circle_in(
                old_pos,
                feet_height,
                input.mv,
                speed,
                dt,
                &self.obstacles,
                self.arena_half,
            );
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
            let handling = weapon_handling(p.weapon);
            let moving = (pos[0] - old_pos[0]).abs() + (pos[1] - old_pos[1]).abs() > 1e-5;
            p.bloom = (p.bloom - handling.bloom_recovery * dt).max(0.0);
            p.ads_fraction = if p.reload_t > 0.0
                || p.shield
                || input.melee
                || p.melee_cd > 0.0
                || self.round_pause > 0.0
            {
                0.0
            } else {
                advance_ads(p.ads_fraction, input.ads, p.weapon, dt)
            };
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
                    reset_handling(p);
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
                reset_handling(p);
            } else if (input.reload && p.ammo < stats.mag && p.reserve > 0) || p.ammo == 0 {
                p.reload_t = stats.reload;
                reset_handling(p);
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
                    // The round reads recovering bloom before adding this
                    // shot's recoil. `v.grounded` is this tick's vertical step,
                    // so a jumping spray is judged airborne on the tick it
                    // leaves the ground.
                    launch_round(
                        p,
                        &stats,
                        moving,
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
                    p.bloom =
                        (p.bloom + stats.bloom).min((stats.spread_max - stats.spread).max(0.0));
                }
            }
            let p = &mut self.players[i];
            p.spread = weapon_spread(
                p.weapon,
                p.ads_fraction,
                p.bloom,
                p.crouch,
                moving,
                v.grounded,
            );
        }
        self.bullets.extend(new_bullets);

        // ---- the hill ----
        //
        // After the movement pass, so it judges where everyone stands this
        // tick. One living body on it alone is the holder and earns a point
        // for every whole `HILL_TICK_SECS` it stays; nobody frees it; two or
        // more contest it and earn nothing. Any change of holder drops the
        // partial second, so stepping off and back on starts over.
        if self.mode == GameMode::Hill
            && let Some(hill) = self.hill
        {
            let mut on = self
                .players
                .iter()
                .filter(|p| p.alive && hill.stands_on(p.pos, p.y))
                .map(|p| p.id);
            let holder = match (on.next(), on.next()) {
                (None, _) => HILL_FREE,
                (Some(id), None) => id,
                (Some(_), Some(_)) => HILL_CONTESTED,
            };
            if holder != self.hill_holder {
                self.hill_holder = holder;
                self.hill_t = 0.0;
            } else if holder < HILL_CONTESTED {
                self.hill_t += dt;
                if self.hill_t >= HILL_TICK_SECS {
                    self.hill_t -= HILL_TICK_SECS;
                    if self.round_pause == 0.0
                        && let Some(p) = self.players.iter_mut().find(|p| p.id == holder)
                    {
                        p.score += 1;
                    }
                }
            }
        }

        // Nothing pays during the pause after a round: the blocks and the
        // pads stay armed for the restart rather than being spent on a
        // round that is already over.
        let paused = self.round_pause > 0.0;
        if paused {
            bonks.clear();
        }

        // Weapon pads: tick respawns, hand out loot on contact. A pad rolls
        // the same table as a block, so there is exactly one reward rule.
        for pad in &mut self.pads {
            if pad.respawn_t > 0.0 {
                pad.respawn_t = (pad.respawn_t - dt).max(0.0);
                continue;
            }
            if paused {
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

        // Bullets: the world's first crossing on the tick's segment, then
        // swept player collision (segment vs circle, exact on the segment,
        // so no tunneling at any speed and no point-blank dead zone)
        // against targets REWOUND by the shooter's view delay, then the
        // round ends at whichever came first or integrates to the
        // segment's end.
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
                if !talive || self.same_team(aid, tid) {
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
        let mut shots: Vec<ShotEvent> = Vec::new();
        // (contact parameter, player index, head) of every body a round's
        // segment meets this tick, gathered and then taken in segment
        // order, so the nearest body is the one a round without pierce
        // stops in and the one a raised plate reflects it off, whatever
        // the players' list order. At 15 m a tick the list order would
        // otherwise decide who was hit.
        let mut bodies: Vec<(f32, usize, bool)> = Vec::new();
        bullets.retain_mut(|b| {
            // The table is the one source: gravity, kind and radius are read
            // through the round's weapon rather than carried on it.
            let stats = weapon_stats(b.weapon);
            let rocket = stats.kind == Projectile::Rocket;
            let radius = stats.radius;
            b.ttl -= dt;
            if b.ttl <= 0.0 {
                // A rocket out of flight time goes off where it is, and
                // every round reports the expiry, so a tracer ends where
                // the round faded and not where the last state left it.
                let at = [b.pos[0], b.y, b.pos[1]];
                if rocket {
                    self.detonate(&obstacles, b, at, None, None, &mut hits, &mut blasts);
                }
                shots.push(b.ended(at, SHOT_EXPIRED, SHOT_NONE, SHOT_NONE, [0; 3]));
                return false;
            }
            // Gravity, semi-implicit Euler like the player's: the vertical
            // speed is charged BEFORE the segment is formed. The horizontal
            // `vel` keeps its magnitude, so every range-per-tick invariant
            // stands, and a zero-gravity row adds exactly zero.
            b.vy += stats.gravity * dt;
            // The sustainer: the horizontal speed grows along its own
            // direction until the row's cap, charged before the segment
            // like gravity, so the launch tick already flies a little
            // faster than the muzzle speed. `vy` is left to gravity, so a
            // rocket flattens as its motor runs. Only the rocket has one.
            if stats.accel > 0.0 {
                let speed_h = (b.vel[0] * b.vel[0] + b.vel[1] * b.vel[1]).sqrt();
                if speed_h > 1e-6 && speed_h < stats.speed_max {
                    let k = (speed_h + stats.accel * dt).min(stats.speed_max) / speed_h;
                    b.vel = [b.vel[0] * k, b.vel[1] * k];
                }
            }
            let p0 = b.pos;
            let p1 = [p0[0] + b.vel[0] * dt, p0[1] + b.vel[1] * dt];
            let y0 = b.y;
            let y1 = b.y + b.vy * dt;
            let (sx, sz) = (p1[0] - p0[0], p1[1] - p0[1]);
            let seg_len_sq = sx * sx + sz * sz;
            let along = |t: f32| [p0[0] + sx * t, y0 + (y1 - y0) * t, p0[1] + sz * t];

            // ---- the world ----
            //
            // Judged FIRST and on the whole segment: the first box, floor
            // or wall crossing along it is where the round stops if no
            // body is met before it. A round is never stopped by the box
            // under its end point; it is stopped by the first thing on
            // its line, which at 4.67 to 15 m a tick is the only test that
            // does not tunnel. Every body below is gated on being nearer
            // than this, which is the old "cover at contact" test made
            // exact: the slab test from the segment's start to the point
            // of contact. Heights used to be hashed with a sin() in this
            // pass; they are carried on the box now, but the rule that
            // made that sound still holds and still matters: bullets are
            // simulated server-side exclusively, and if client-side shot
            // prediction is ever added, every f32 transcendental on the
            // shot's path (the tan() at launch) becomes a desync source.
            let world = world_end(p0, p1, y0, y1, radius, &obstacles, self.arena_half);
            let t_world = world.map_or(f32::INFINITY, |w| w.t);

            // ---- the bodies ----
            bodies.clear();
            for (j, p) in self.players.iter().enumerate() {
                // A body this round already went through is never hit
                // twice: the mask is what stops a pierced target being
                // counted again on the tick after.
                if p.id == b.owner || b.hit_mask & id_bit(p.id) != 0 {
                    continue;
                }
                // A teammate is neither hit nor pierced nor reflects: the
                // round passes as if the body were not there, raised plate
                // included, because the plate is tested only on a body the
                // round may hit and this is not one.
                if self.same_team(b.owner, p.id) {
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
                // Closest point on [p0, p1] to the (rewound) centre: the
                // cheap reject, which most bodies fail.
                let t = if seg_len_sq <= 1e-8 {
                    0.0
                } else {
                    (((tpos[0] - p0[0]) * sx + (tpos[1] - p0[1]) * sz) / seg_len_sq).clamp(0.0, 1.0)
                };
                let (ex, ez) = (tpos[0] - (p0[0] + sx * t), tpos[1] - (p0[1] + sz * t));
                if ex * ex + ez * ez >= rr * rr {
                    continue;
                }
                // The exact overlap of the segment with the circle:
                // |p0 + s t - tpos|^2 = rr^2 is a quadratic in t, and its
                // two roots clipped to [0, 1] are where the round enters
                // and leaves the circle. The round's height is linear in
                // t, so its height over that interval is an interval too,
                // and the hit is decided on intervals rather than samples:
                // there is no speed at which a head can fall between two
                // samples, because there are no samples.
                let (t_in, t_out) = if seg_len_sq <= 1e-8 {
                    (0.0, 0.0)
                } else {
                    let (fx, fz) = (p0[0] - tpos[0], p0[1] - tpos[1]);
                    let half_b = fx * sx + fz * sz;
                    let c = fx * fx + fz * fz - rr * rr;
                    let half_b_sq = half_b * half_b;
                    let disc = half_b_sq - seg_len_sq * c;
                    if disc < 0.0 {
                        continue;
                    }
                    let root = disc.sqrt();
                    let t_in = ((-half_b - root) / seg_len_sq).max(0.0);
                    let t_out = ((-half_b + root) / seg_len_sq).min(1.0);
                    if t_in > t_out {
                        continue;
                    }
                    (t_in, t_out)
                };
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
                let (ya, yb) = (y0 + (y1 - y0) * t_in, y0 + (y1 - y0) * t_out);
                let (ymin, ymax) = (ya.min(yb), ya.max(yb));
                // Body iff the height interval meets [lo, hi]; head iff it
                // meets [head_lo, hi], and given the first the second is
                // one comparison. A round that passed through the head IS
                // a headshot, wherever else on the body it also was.
                if ymax < lo || ymin > hi {
                    continue;
                }
                let head = ymax >= head_lo;
                // Where the round met the body: where it entered the
                // circle, unless it was still above or below the volume
                // there, in which case where its height came into the
                // band. That is the exact contact with the volume, and it
                // is where the tracer ends and the cover gate is judged.
                let contact = if ya > hi {
                    t_in + (t_out - t_in) * ((hi - ya) / (yb - ya))
                } else if ya < lo {
                    t_in + (t_out - t_in) * ((lo - ya) / (yb - ya))
                } else {
                    t_in
                };
                // Cover between the muzzle and the body stops the round
                // before the body does: the world's first crossing lies
                // before the contact. A round that meets a body from
                // INSIDE a tunnel roof was passing through the slab when
                // it connected, and the slab wins. Judged at the contact
                // and not the end point so a point-blank hit on a body
                // backed against a wall is still a hit. Ahead of the
                // shield: cover in front of a raised plate stops the round
                // before the plate could send it back.
                if contact >= t_world {
                    continue;
                }
                bodies.push((contact, j, head));
            }
            bodies.sort_by(|a, b| a.0.total_cmp(&b.0));
            for &(contact, j, head) in &bodies {
                let p = &self.players[j];
                let at = along(contact);
                // ---- the off-hand shield ----
                //
                // Placed exactly where the damage decision is, so a
                // reflected round is precisely one that WOULD have hit.
                // TTL was already charged above, so a shield never extends
                // a round's life.
                //
                // The shield is judged in the PRESENT, this tick's flag and
                // this tick's facing, while the body test above stays
                // lag-compensated. That is deliberately unlike crouch,
                // which rewinds with position: crouch answers "where was
                // the body", which is the shooter's question, and the
                // shield answers "is the defender blocking right now",
                // which is the defender's. Rewinding the flag without also
                // rewinding the facing it points along would answer
                // neither.
                if p.shield {
                    let n = p.aim; // unit: the sim normalizes it on input
                    let dot = b.vel[0] * n[0] + b.vel[1] * n[1];
                    let speed_h = (b.vel[0] * b.vel[0] + b.vel[1] * b.vel[1]).sqrt();
                    // Inside the cover arc iff the round is travelling
                    // into the plate's face: its heading within half the
                    // arc of -n. Tested on the heading rather than on the
                    // bearing from holder to round, because the point of
                    // contact sits by construction on the flight line and
                    // a bearing taken there is noise. For anything but a
                    // point-blank shot the two agree anyway. cos() is a
                    // transcendental in hit registration, sound for
                    // exactly the reason the tan() at launch is: bullets
                    // are stepped server-side only.
                    if speed_h > 1e-6 && -dot >= (SHIELD_ARC * 0.5).cos() * speed_h {
                        // A rocket detonates ON the plate and never comes
                        // back: a plate is cover, not a launcher, and a
                        // three-damage rocket bounced by a 120 degree arc
                        // would be a free kill on the shooter with no
                        // counterplay. The holder is spared the splash;
                        // the plate took it.
                        if rocket {
                            self.detonate(
                                &obstacles,
                                b,
                                at,
                                None,
                                Some(p.id),
                                &mut hits,
                                &mut blasts,
                            );
                            shots.push(b.ended(at, SHOT_SHIELD, SHOT_NONE, p.id, [0; 3]));
                            return false;
                        }
                        // Mirror about the plate: v' = v - 2(v.n)n. The
                        // normal is horizontal, so vy is untouched and a
                        // round arcing down at you comes back arcing down,
                        // keeping its range rather than being launched at
                        // the sky. A mirror is an isometry, so horizontal
                        // speed survives and the invariant pinned by
                        // pitch_does_not_shorten_a_shot survives
                        // reflection. Damage rides along: catch a revolver
                        // round and you send two damage back.
                        b.vel = [b.vel[0] - 2.0 * dot * n[0], b.vel[1] - 2.0 * dot * n[1]];
                        // The segment that ended at the plate was the
                        // shooter's: recorded before the round changes
                        // hands, so the tracer up to the plate is drawn
                        // in the shooter's name and the one back in the
                        // catcher's.
                        shots.push(b.ended(at, SHOT_SHIELD, SHOT_NONE, p.id, [0; 3]));
                        // The round belongs to whoever caught it. It can
                        // now kill anyone, the shooter included, and the
                        // frag is the reflector's. remove_player drops
                        // bullets by owner, so this also decides that a
                        // reflected round outlives the shooter leaving and
                        // dies with the reflector instead, which is what
                        // the transfer means, not an accident of it.
                        b.owner = p.id;
                        // Nobody aimed this round, so there is nothing for
                        // lag compensation to honour: from here it
                        // hit-tests against the present.
                        b.delay = 0;
                        // A caught round is a fresh round: whatever it went
                        // through on the way in is forgotten, and it does
                        // not go through anyone on the way back. The mask
                        // must clear or the round could never hit the body
                        // it pierced coming in, and the pierce must clear
                        // or a reflected sniper round would be a two-body
                        // reward for the catcher that the shooter never
                        // earned.
                        b.pierce = 0;
                        b.hit_mask = 0;
                        // The segment ends at the plate and the next one
                        // starts there: the round is moved to the point of
                        // contact rather than left at the tick's start, so
                        // the tracer the client draws from the next event
                        // begins where this one ended. Safe to do in one
                        // pass because the bodies are taken in segment
                        // order: everyone nearer was already tested, and
                        // everyone further was never on the new path. The
                        // holder is the owner now and is skipped. It costs
                        // one tick, as it always did.
                        b.pos = [at[0], at[2]];
                        b.y = at[1];
                        b.from = at;
                        return true;
                    }
                }
                // A rocket on a body is a direct hit and a blast at the
                // point of contact; the body it struck takes the direct
                // damage and is left out of the splash.
                if rocket {
                    self.detonate(&obstacles, b, at, Some(p.id), None, &mut hits, &mut blasts);
                    shots.push(b.ended(at, SHOT_BODY, SHOT_NONE, p.id, [0; 3]));
                    return false;
                }
                // A head hit kills outright, whatever the weapon and
                // whatever the remaining HP. Routed as damage rather than
                // as a special case so respawn, scoring and the kill event
                // all stay on the one path.
                hits.push((b.owner, p.id, if head { MAX_HP } else { b.dmg }, head));
                shots.push(b.ended(at, SHOT_BODY, SHOT_NONE, p.id, [0; 3]));
                if b.pierce == 0 {
                    return false;
                }
                // Pierce: the round goes on, remembering this body, and
                // the loop CONTINUES so a second body on this same tick's
                // segment is hit too. The next event starts at this body,
                // so a pierced round's events chain end to end.
                b.pierce -= 1;
                b.hit_mask |= id_bit(p.id);
                b.from = at;
            }
            if let Some(w) = world {
                // Stopped by the world. A rocket goes off there: on the
                // floor lifted to 0.05 so the blast is drawn above the
                // ground it hit, off a box pushed back by the standoff so
                // the splash's line-of-sight test does not start on the
                // box's own face, on the wall at the wall.
                if rocket {
                    let blast = match w.hit {
                        SHOT_FLOOR => [w.at[0], 0.05, w.at[2]],
                        SHOT_COVER => [
                            w.at[0] + f32::from(w.normal[0]) * BLAST_STANDOFF,
                            w.at[1] + f32::from(w.normal[1]) * BLAST_STANDOFF,
                            w.at[2] + f32::from(w.normal[2]) * BLAST_STANDOFF,
                        ],
                        _ => w.at,
                    };
                    self.detonate(&obstacles, b, blast, None, None, &mut hits, &mut blasts);
                }
                shots.push(b.ended(w.at, w.hit, w.cover, SHOT_NONE, w.normal));
                return false;
            }
            b.pos = p1;
            b.y = y1;
            true
        });
        self.bullets = bullets;
        self.obstacles = obstacles;
        self.blasts = blasts;
        self.shots = shots;

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
                reset_handling(v);
                v.respawn_in = RESPAWN_SECS;
                v.death_count += 1;
                // The kill event is pushed whoever did it, so a self-kill
                // reads as `Kill { killer: id, victim: id }` on every
                // client; only the score asks whether it was someone else.
                self.events.push((owner, victim));
                // A frag is always a frag; whether it is also a point is
                // the mode's question, and nothing is a point during the
                // pause after a round. A self-kill is neither.
                if owner != victim
                    && let Some(k) = self.players.iter_mut().find(|p| p.id == owner)
                {
                    k.frags += 1;
                    if self.round_pause == 0.0 {
                        match self.mode {
                            GameMode::Ffa => k.score += 1,
                            GameMode::Tdm => {
                                k.score += 1;
                                self.team_score[usize::from(k.team & 1)] += 1;
                            }
                            GameMode::Hill => {}
                        }
                    }
                }
            }
        }

        // ---- the round ----
        //
        // A limit reached ends the round: the winner is recorded for the
        // server to announce and the pause starts. During the pause
        // everything above keeps running but nothing scores (the gates on
        // `round_pause` above), and when it runs out the next round starts
        // from a clean slate. Checked last so the tick that reaches the
        // limit still reports its own kill and score.
        if self.round_pause > 0.0 {
            self.round_pause = (self.round_pause - dt).max(0.0);
            if self.round_pause == 0.0 {
                self.restart_round();
            }
        } else {
            let winner = match self.mode {
                GameMode::Ffa => self
                    .players
                    .iter()
                    .find(|p| p.score >= FFA_FRAG_LIMIT)
                    .map(|p| (p.id, false)),
                GameMode::Hill => self
                    .players
                    .iter()
                    .find(|p| p.score >= HILL_LIMIT)
                    .map(|p| (p.id, false)),
                GameMode::Tdm => (0..2u8)
                    .find(|&t| self.team_score[usize::from(t)] >= TDM_FRAG_LIMIT)
                    .map(|t| (t, true)),
            };
            if let Some(w) = winner {
                self.round_over.push(w);
                self.round_pause = ROUND_PAUSE_SECS;
            }
        }
        // No stored scope/bloom crosses a round boundary, including the
        // exact tick that reaches the limit. Paused rounds retain the old
        // firing/movement rules, but cannot build a ready scope for restart.
        if self.round_pause > 0.0 {
            for p in &mut self.players {
                reset_handling(p);
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
    use crate::freight_yard::level_helpers::{
        centre, climb, contains, dist, gap, hop, standable_floor,
    };

    fn step_with(sim: &mut Sim, inputs: &HashMap<u8, PlayerIn>) {
        sim.step(&|id| inputs.get(&id).copied().unwrap_or_default());
    }

    /// Explicit collision/trajectory fixture: real sim ordering and weapon
    /// ballistics, but no aim dispersion. Gameplay/handling tests use `step_with`.
    fn step_geometry(sim: &mut Sim, inputs: &HashMap<u8, PlayerIn>) {
        sim.step_using(
            &|id| inputs.get(&id).copied().unwrap_or_default(),
            |p, stats, moving, _grounded, seed, tick, delay, out| {
                let mut exact = *stats;
                exact.spread = 0.0;
                exact.spread_max = 0.0;
                // Grounding affects dispersion only in launch; suppress its
                // minimum cone while retaining p.y/vy from the real step.
                launch(p, &exact, moving, true, seed, tick, delay, out);
            },
        );
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
    fn jump_reach_is_independent_of_ground_speed() {
        let jump_distance = |sprint: bool| -> f32 {
            let mut sim = Sim::new(9);
            sim.obstacles.clear();
            sim.add_player(0);
            sim.players[0].pos = [-20.0, 0.0];
            let start = sim.players[0].pos[0];
            let mut was_airborne = false;
            for tick in 0..180 {
                sim.step(&|_| PlayerIn {
                    mv: [1.0, 0.0],
                    sprint,
                    jump: tick == 0,
                    ..Default::default()
                });
                let p = &sim.players[0];
                was_airborne |= p.vy != 0.0;
                if was_airborne && p.vy == 0.0 {
                    return p.pos[0] - start;
                }
            }
            panic!("jump did not land");
        };

        assert!((jump_distance(false) - 6.75).abs() < 1e-3);
        assert!((jump_distance(true) - 10.8).abs() < 1e-3);
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
            // Bullet flight to x=6 takes two ticks at 280 m/s, well inside
            // the 12-tick rewind window.
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
        for _ in 0..((weapon_stats(1).cooldown / FIXED_DT) as u32 + 2) * u32::from(mag) {
            let before = sim.players[0].ammo;
            step_with(&mut sim, &inputs);
            fired += u32::from(before.saturating_sub(sim.players[0].ammo));
            if sim.players[0].ammo == 0 {
                break;
            }
        }
        assert_eq!(
            fired,
            u32::from(mag),
            "exactly one magazine before auto-reload gates fire"
        );
        step_with(&mut sim, &inputs);
        assert!(
            sim.players[0].reload_t > 0.0,
            "empty gun starts auto-reload"
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
            assert!(s.accel >= 0.0, "{id}: a sustainer that slows the round");
            assert!(
                s.speed_max >= s.speed,
                "{id}: the cap {} is under the muzzle speed {}",
                s.speed_max,
                s.speed
            );
            assert_eq!(
                s.accel > 0.0,
                s.kind == Projectile::Rocket,
                "{id}: only the rocket has a motor"
            );
            assert!(
                s.accel > 0.0 || s.speed_max.to_bits() == s.speed.to_bits(),
                "{id}: a cap above the muzzle speed with nothing to reach it"
            );
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
        assert_eq!((s.cooldown, s.mag, s.damage), (0.32, 6, 1));
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
    fn segment_box_entry_reports_where_and_which_face() {
        // A unit-ish box on the floor at [0, 2] on both axes, two tall.
        // The entry parameter is where the segment meets the box, and the
        // face is the one it came in through, as an outward normal: that
        // is where a round stops and how the client lays its mark.
        let o = Obstacle::boxed(Cover::Crate, [0.0, 0.0], [2.0, 2.0], 0.0, 2.0);
        let entry = |a, b| segment_box_entry_face(a, b, &o).expect("a hit");
        let (t, n) = entry([-1.0, 1.0, 1.0], [3.0, 1.0, 1.0]);
        assert!((t - 0.25).abs() < 1e-6, "{t}");
        assert_eq!(n, [-1, 0, 0], "in through the west face");
        let (t, n) = entry([3.0, 1.0, 1.0], [-1.0, 1.0, 1.0]);
        assert!((t - 0.25).abs() < 1e-6, "{t}");
        assert_eq!(n, [1, 0, 0], "in through the east face");
        let (t, n) = entry([1.0, 5.0, 1.0], [1.0, -1.0, 1.0]);
        assert!((t - 0.5).abs() < 1e-6, "{t}");
        assert_eq!(n, [0, 1, 0], "down through the top");
        let (t, n) = entry([1.0, 1.0, 4.0], [1.0, 1.0, -2.0]);
        assert!((t - 1.0 / 3.0).abs() < 1e-6, "{t}");
        assert_eq!(n, [0, 0, 1], "in through the +z face");
        // Starting inside: entered at 0 through no face.
        assert_eq!(entry([1.0, 1.0, 1.0], [5.0, 1.0, 1.0]), (0.0, [0, 0, 0]));
        // A miss on any axis is a miss, and the two spellings agree.
        for (a, b) in [
            ([-1.0, 3.0, 1.0], [3.0, 3.0, 1.0]),
            ([-1.0, 1.0, 3.0], [3.0, 1.0, 3.0]),
            ([-1.0, 1.0, 1.0], [-0.5, 1.0, 1.0]),
        ] {
            assert_eq!(segment_box_entry(a, b, &o), None, "{a:?} -> {b:?}");
            assert!(!segment_hits_box(a, b, &o));
        }
        assert_eq!(
            segment_box_entry([-1.0, 1.0, 1.0], [3.0, 1.0, 1.0], &o),
            Some(0.25)
        );
        // Entry is monotone in where the segment starts: the same line
        // from further back enters later along itself.
        let (near, _) = entry([-1.0, 1.0, 1.0], [3.0, 1.0, 1.0]);
        let (far, _) = entry([-3.0, 1.0, 1.0], [3.0, 1.0, 1.0]);
        assert!(far > near);
    }

    #[test]
    fn cover_index_is_the_declaration_order() {
        // What `S2C::Shot.cover` carries; the enum is append-only so these
        // never move.
        let all = [
            Cover::Container,
            Cover::Crate,
            Cover::Ammo,
            Cover::Sandbag,
            Cover::Wall,
            Cover::Roof,
            Cover::Rubble,
            Cover::Plinth,
            Cover::Loot,
        ];
        for (i, k) in all.iter().enumerate() {
            assert_eq!(usize::from(k.index()), i, "{k:?}");
            assert_eq!(Cover::from_index(k.index()), Some(*k), "{k:?}");
        }
        assert_ne!(Cover::Loot.index(), SHOT_NONE);
        assert_eq!(Cover::from_index(SHOT_NONE), None);
        assert_eq!(
            Cover::from_index(9),
            None,
            "a kind this build does not know"
        );
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
            hill: None,
        };
        assert_eq!(level.spawn(3), spawn_point(3));
        // A short authored list wraps rather than panicking.
        let two = Level {
            arena_half: ARENA_HALF,
            obstacles: Vec::new(),
            spawns: vec![[1.0, 2.0], [3.0, 4.0]],
            pads: Vec::new(),
            decor: Vec::new(),
            hill: None,
        };
        assert_eq!(two.spawn(0), [1.0, 2.0]);
        assert_eq!(two.spawn(5), [3.0, 4.0]);
        // And the sim places players by the same rule, first spawn and
        // respawn alike.
        let mut sim = Sim::from_level(&two, 0, GameMode::Ffa);
        sim.add_player(0);
        sim.add_player(1);
        sim.add_player(2);
        assert_eq!(sim.players[0].pos, [1.0, 2.0]);
        assert_eq!(sim.players[1].pos, [3.0, 4.0]);
        assert_eq!(sim.players[2].pos, [1.0, 2.0], "wraps");
        let mut none = Sim::from_level(&level, 0, GameMode::Ffa);
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
        // Keep the 4.5 m travel distance independent of the ground-speed
        // tuning: collision should be what decides the result here.
        let walk_time = 4.5 / MOVE_SPEED;
        // Walking into the crate from outside gets stopped.
        let walked = move_circle([-3.0, 0.0], 0.0, [1.0, 0.0], MOVE_SPEED, walk_time, &obs);
        assert!(
            walked[0] < -1.5 - PLAYER_R + 0.01,
            "walked into box: {walked:?}"
        );
        // The same move with the feet above the crate's top goes through.
        let over = move_circle(
            [-3.0, 0.0],
            top + 0.1,
            [1.0, 0.0],
            MOVE_SPEED,
            walk_time,
            &obs,
        );
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
            // clamp a round descends tan(1.45) * BULLET_SPEED * dt ~= 38
            // units in its FIRST tick, and bullets are extended into the
            // list before that same tick's sweep runs, so a straight-down
            // shot fired from ground level is ended by the floor before it
            // can be inspected at all. That ending is correct, and is
            // asserted on its own in the test below; it is just not the
            // property being pinned here. And at the west wall, so the
            // east wall is not met on the same tick either.
            sim.players[0].y = 40.0;
            sim.players[0].pos = [-20.0, 0.0];
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
            step_geometry(&mut sim, &inputs);
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
            step_geometry(&mut sim, &inputs);
        }
        sim
    }

    #[test]
    fn a_raised_shield_sends_the_round_back() {
        // The whole feature: a frontal round is not absorbed, it is mirrored
        // and changes hands. Twenty metres at 4.67 m a tick: six ticks is
        // past the five the round needs to reach the defender's circle at
        // 19.2 m and short of the nine by which it is back at the shooter
        // and consumed, so it is caught mid-return.
        let sim = shield_duel(20.0, [-1.0, 0.0], true, 6);
        let b = sim
            .bullets
            .first()
            .expect("a reflected round must still be in flight");
        assert_eq!(b.owner, 1, "the round belongs to whoever caught it");
        // Head-on off a shield facing -x: v = (+280, 0) becomes (-280, 0).
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
        // At the west wall, firing east: from the spawn ring a 280 m/s
        // round can meet the wall on the tick it leaves and be gone before
        // the assertion below sees it.
        sim.players[0].pos = [-20.0, 0.0];
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
        // steep shots failing to spawn at all. Forty metres, because at the
        // clamp a 280 m/s round drops 38 m in one tick.
        let mut sim = Sim::new(25);
        sim.obstacles.clear();
        sim.pads.clear();
        sim.add_player(0);
        sim.players[0].y = 40.0;
        sim.players[0].pos = [-20.0, 0.0];
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

    /// Real gameplay shots with `weapon`: allow its actual sights to settle
    /// before firing, retaining the shipped nonzero cone and seeded roll.
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
        let settle_ticks = (weapon_handling(weapon).ads_in_secs / FIXED_DT).ceil() as usize + 1;
        for tick in 0..240 {
            inputs.get_mut(&0).unwrap().fire = tick >= settle_ticks;
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
        // Standing head band is [1.56, 1.86]. At the swept body's near
        // edge (4.18 m), pitch .06 puts a settled shot near 1.70 m, with
        // margin for its real nonzero cone instead of aiming at the edge.
        assert!(
            one_shot_kills(0.0, 5.0, 0.06, false),
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
        // The regression guard for the exact head band, on every bullet
        // weapon in the table.
        //
        // Vertical travel in one tick is tan(pitch) * speed * FIXED_DT: at
        // 280 m/s and a 45 degree drop that is 4.67 m, at the sniper's 900
        // it is 15 m, against a head band 0.30 tall. The v18 sweep walked
        // the tick in at most 32 samples and could only pass this up to
        // about 60 m/s, which is what kept the sniper off its real speed.
        // The v20 sweep intersects the segment with the circle and reads
        // the round's height over the overlap as an interval, so there is
        // no speed at which the head falls between two samples: this is
        // the test a sampled band could not pass.
        //
        // Fire down from a container top through the target's head, aimed
        // so the round's line crosses 1.60, the middle of the head band,
        // at the target's centre.
        let from_y = 3.0;
        let gap = 3.0;
        let drop = (from_y + EYE_STAND) - 1.60;
        let pitch = -(drop / gap).atan();
        for weapon in 1..=WEAPON_COUNT {
            let stats = weapon_stats(weapon);
            if stats.kind != Projectile::Bullet {
                continue;
            }
            let per_tick = pitch.tan().abs() * stats.speed * FIXED_DT;
            assert!(
                per_tick > HEAD_H,
                "{}: the tick must straddle the head band: {per_tick} vs {HEAD_H}",
                stats.name
            );
            assert!(
                one_shot_kills_with(weapon, from_y, gap, pitch, false),
                "{}: a steep round through the head must be a headshot at {} m/s",
                stats.name,
                stats.speed
            );
        }
    }

    #[test]
    fn a_head_hit_is_found_wherever_along_the_tick_it_happens() {
        // The sniper's tick is 15 m long. A head at any point along it,
        // from just past the muzzle to just short of the segment's end, is
        // found on that one tick: the overlap interval is solved, not
        // sampled, so where on the segment the head sits cannot matter.
        // Each shot is aimed to cross 1.60 at the target's centre.
        let from_y = 3.0;
        for gap in [1.5, 3.0, 6.0, 9.0, 12.0, 14.0] {
            let drop = (from_y + EYE_STAND) - 1.60;
            let pitch = -(drop / gap).atan();
            assert!(
                pitch.abs() <= MAX_PITCH,
                "gap {gap}: the aim {pitch} is past the clamp"
            );
            let mut sim = open_sim(11, 2);
            arm(&mut sim.players[0], 6);
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    ads: true,
                    ..shot(0, [1.0, 0.0], pitch)
                },
            );
            hold(&mut sim, &[(0, [0.0, 0.0], from_y), (1, [gap, 0.0], 0.0)]);
            step_geometry(&mut sim, &inputs);
            assert_eq!(
                sim.hits,
                vec![(0, 1, MAX_HP, true)],
                "gap {gap}: a headshot on the tick the round left"
            );
            let s = sim.shots.first().expect("the round ended in the body");
            assert_eq!((s.hit, s.victim), (SHOT_BODY, 1), "gap {gap}");
            assert!(
                s.to[0] > 0.2 && s.to[0] < gap,
                "gap {gap}: the contact {:?} is between the muzzle and the centre",
                s.to
            );
        }
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

    /// Walks +x for `distance` at `y`, one sim tick at a time, so a box in the
    /// way is met at its face rather than jumped over by a long dt.
    fn walk(mut pos: [f32; 2], y: f32, distance: f32, obs: &[Obstacle]) -> [f32; 2] {
        let ticks = (distance / (MOVE_SPEED * FIXED_DT)).ceil() as u32;
        for _ in 0..ticks {
            pos = move_circle(pos, y, [1.0, 0.0], MOVE_SPEED, FIXED_DT, obs);
        }
        pos
    }

    #[test]
    fn a_raised_box_blocks_only_a_body_that_reaches_it() {
        let obs = roof();
        // On the floor the head (1.86) is under the bottom (2.5): walk
        // straight through, 9 units from -4 to 5.
        let under = walk([-4.0, 0.0], 0.0, 9.0, &obs);
        assert!(
            under[0] > 4.0,
            "blocked by a roof from the floor: {under:?}"
        );
        // Feet at 1.0: head 2.86 is inside the box and the feet are more
        // than a step below its top, so its side is a wall.
        let mid = walk([-4.0, 0.0], 1.0, 9.0, &obs);
        assert!(
            mid[0] <= -2.0 - PLAYER_R + 1e-3,
            "walked through the roof's side: {mid:?}"
        );
        // Standing on it: walk across.
        let over = walk([-4.0, 0.0], 2.9, 9.0, &obs);
        assert!(over[0] > 4.0, "could not walk across the roof: {over:?}");
        // And a floor box is exactly what it was: a wall from the floor.
        let floor_box = one_box();
        let walked = walk([-3.0, 0.0], 0.0, 9.0, &floor_box);
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
        let mut sim = Sim::from_level(&level, 0, GameMode::Ffa);
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
            match o.kind {
                Cover::Roof => assert!(
                    o.base >= CONTAINER_MIN_H,
                    "roof {o:?} is too low to walk under"
                ),
                Cover::Loot => {
                    assert!(
                        o.base > BODY_H_STAND,
                        "block {o:?} hangs too low to walk under"
                    );
                    assert_eq!(o.h - o.base, LOOT_SIZE, "block {o:?} is not a unit cube");
                    assert_eq!(o.max[0] - o.min[0], LOOT_SIZE);
                    assert_eq!(o.max[1] - o.min[1], LOOT_SIZE);
                }
                _ => assert_eq!(
                    o.base, 0.0,
                    "{o:?}: only roofs and loot blocks leave the floor"
                ),
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
        assert!(
            level.pads.is_empty(),
            "Trench City has no pads since v18: {:?}",
            level.pads
        );
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

    /// Exact level sight-ray fixture, with feet pinned at `to_y` (a roof).
    /// Dispersion is isolated out: this tests cover visibility, not handling.
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
            step_geometry(&mut sim, &inputs);
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
        let distance_to_exit = roof.max[0] + 1.0 - pos[0];
        let max_ticks = (distance_to_exit / (MOVE_SPEED * FIXED_DT)).ceil() as u32 + 1;
        for _ in 0..max_ticks {
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

    /// One jump from the floor at `pos`, pressed on tick 0 and driven for
    /// 90 ticks of `step_vertical`: the 0-based ticks on which the clamp
    /// named box `k`, and the highest the feet got. Any other box named is
    /// a failure, because the point is which box the head met.
    fn bonk_ticks(pos: [f32; 2], k: usize, obs: &[Obstacle]) -> (Vec<u32>, f32) {
        let (mut y, mut vy, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        let mut hits = Vec::new();
        for tick in 0..90u32 {
            let v = step_vertical(pos, y, vy, tick == 0, FIXED_DT, obs);
            y = v.y;
            vy = v.vy;
            peak = peak.max(y);
            if v.bonked == Some(k) {
                hits.push(tick);
            } else {
                assert!(v.bonked.is_none(), "bonked {:?} instead of {k}", v.bonked);
            }
        }
        (hits, peak)
    }

    /// The v18 reward on this map: one `?` block per side, standing at the
    /// mouth of each tunnel where the v13 pad used to sit under the roof.
    /// Every claim the block makes is asked of the sim's own integrator and
    /// mover rather than read off the table: bonked from the floor on the
    /// fourth step after the press, walked under without lifting the feet,
    /// over no roof (so the lowest-base clamp never has to choose), and on
    /// floor every spawn reaches through `blocked`.
    #[test]
    fn trench_city_has_four_blocks_at_the_tunnel_mouths() {
        let level = Level::trench_city();
        let obs = &level.obstacles;
        assert!(level.pads.is_empty(), "no pads on Trench City since v18");
        let blocks: Vec<(usize, &Obstacle)> = obs
            .iter()
            .enumerate()
            .filter(|(_, o)| o.kind == Cover::Loot)
            .collect();
        assert_eq!(blocks.len(), 4, "one block per side");
        let sim = Sim::from_level(&level, 0, GameMode::Ffa);
        assert!(sim.pads.is_empty(), "the sim arms no pad on Trench City");
        assert_eq!(sim.loot.len(), 4, "the sim arms one slot per block");
        for (slot, (k, _)) in sim.loot.iter().zip(&blocks) {
            assert_eq!(slot.obstacle, *k);
            assert_eq!(slot.respawn_t, 0.0, "blocks start armed");
        }

        for &(k, block) in &blocks {
            let c = centre(block);
            // At a tunnel mouth: the nearest roof is the one over that
            // tunnel, 0.7 m away across the inner wall's gap, on the same
            // axis as the block, and the block hangs over no roof at all.
            let roof = obs
                .iter()
                .filter(|o| o.kind == Cover::Roof)
                .min_by(|a, b| gap(a, block).total_cmp(&gap(b, block)))
                .expect("a roof");
            let g = gap(roof, block);
            assert!(
                (g - 0.7).abs() < 1e-4,
                "block {block:?} is {g} from the nearest roof, not at a tunnel mouth"
            );
            let rc = centre(roof);
            assert!(
                (rc[0] - c[0]).abs() < 1e-4 || (rc[1] - c[1]).abs() < 1e-4,
                "block {block:?} is not on its tunnel's axis"
            );
            for o in obs.iter().filter(|o| o.base > 0.0 && o.kind != Cover::Loot) {
                assert!(
                    gap(o, block) > 0.0,
                    "block {block:?} shares a footprint with {o:?}"
                );
            }

            // Open floor under it: a standing body may occupy the centre.
            assert!(
                standable_floor(c, obs),
                "block {block:?} cannot be stood under"
            );
            assert_eq!(support_height(c, PLAYER_R, 0.0, obs), 0.0);

            // Bonked from the floor: the head meets the block once, on the
            // fourth integrator step after the press (0.55 m of rise
            // against 0.44 m of headroom; the third step gives 0.42 and
            // misses), and the feet stop exactly at bottom minus body.
            let (hits, peak) = bonk_ticks(c, k, obs);
            assert_eq!(hits, [3], "block {block:?}: bonk ticks (0-based)");
            assert!(
                (peak - (block.base - BODY_H_STAND)).abs() < 1e-4,
                "block {block:?}: feet peaked at {peak}"
            );

            // Walked under: 3.9 m through it and back at walking speed,
            // never touched and never lifted. The walk runs along the
            // tunnel's axis, through the inner wall's gap and out again.
            // A unit vector and not a per-axis `signum`: `0.0_f32.signum()`
            // is 1.0, which would turn the walk into a diagonal.
            let reach = dist(rc, c);
            let toward = [(rc[0] - c[0]) / reach, (rc[1] - c[1]) / reach];
            let start = [c[0] - toward[0] * 2.0, c[1] - toward[1] * 2.0];
            let (mut pos, mut y, mut vy) = (start, 0.0f32, 0.0f32);
            for tick in 0..52u32 {
                let dir = if tick < 26 { 1.0 } else { -1.0 };
                let mv = [toward[0] * dir, toward[1] * dir];
                pos = move_circle(pos, y, mv, MOVE_SPEED, FIXED_DT, obs);
                let v = step_vertical(pos, y, vy, false, FIXED_DT, obs);
                y = v.y;
                vy = v.vy;
                assert_eq!(v.bonked, None, "walking under {block:?} bonked at {pos:?}");
                assert_eq!(y, 0.0, "lifted off the floor under {block:?} at {pos:?}");
            }
            assert!(
                dist(pos, start) < 0.2,
                "did not walk under {block:?} and back: ended at {pos:?}"
            );
        }

        // Reachable: every block's footprint holds a cell the flood from
        // every spawn at y 0 reaches.
        let footprints: Vec<&Obstacle> = blocks.iter().map(|(_, b)| *b).collect();
        for (s, spawn) in level.spawns.iter().enumerate() {
            let reached = floor_reaches(obs, *spawn, &footprints);
            for (t, (k, b)) in blocks.iter().enumerate() {
                assert!(
                    reached[t],
                    "spawn {s} {spawn:?} never reaches block {k} {b:?}"
                );
            }
        }
    }

    /// A 0.2 m grid over the arena, each cell asked of the sim whether a
    /// standing body may occupy it, flooded 4-way from `spawn` at y 0.
    /// Which of `targets` hold a flooded cell. Panics when the spawn itself
    /// is not free floor, because a flood from inside cover proves nothing.
    fn floor_reaches(obs: &[Obstacle], spawn: [f32; 2], targets: &[&Obstacle]) -> Vec<bool> {
        let n: i32 = 116;
        let side = (2 * n + 1) as usize;
        let at = |i: i32, j: i32| [0.2 * i as f32, 0.2 * j as f32];
        let idx = |i: i32, j: i32| ((i + n) as usize) * side + (j + n) as usize;
        let mut free = vec![false; side * side];
        for i in -n..=n {
            for j in -n..=n {
                free[idx(i, j)] = standable_floor(at(i, j), obs);
            }
        }
        let (si, sj) = (
            (spawn[0] / 0.2).round() as i32,
            (spawn[1] / 0.2).round() as i32,
        );
        assert!(free[idx(si, sj)], "spawn {spawn:?} is not on free floor");
        let mut seen = vec![false; side * side];
        let mut stack = vec![(si, sj)];
        seen[idx(si, sj)] = true;
        let mut reached = vec![false; targets.len()];
        while let Some((i, j)) = stack.pop() {
            let p = at(i, j);
            for (t, b) in targets.iter().enumerate() {
                if !reached[t] && contains(b, p) {
                    reached[t] = true;
                }
            }
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (ni, nj) = (i + di, j + dj);
                if ni.abs() > n || nj.abs() > n {
                    continue;
                }
                let q = idx(ni, nj);
                if free[q] && !seen[q] {
                    seen[q] = true;
                    stack.push((ni, nj));
                }
            }
        }
        reached
    }

    /// The block is not decoration: a sim built from the authored map pays
    /// a pool weapon for a bonk on it, through the same `loot_roll` and
    /// `grant` as the yard, and the block goes dark.
    #[test]
    fn a_bonk_on_a_trench_city_block_pays_a_pool_weapon() {
        let level = Level::trench_city();
        let mut sim = Sim::from_level(&level, 5, GameMode::Ffa);
        sim.add_player(0);
        let (k, block) = level
            .obstacles
            .iter()
            .enumerate()
            .find(|(_, o)| o.kind == Cover::Loot)
            .expect("a block");
        let c = centre(block);
        let slot = sim
            .loot
            .iter()
            .position(|l| l.obstacle == k)
            .expect("the block is armed");
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
            sim.players[0].pos = c;
            step_with(&mut sim, if tick == 0 { &inputs } else { &idle });
            if let Some(&ev) = sim.loot_events.first() {
                assert!(paid.is_none(), "paid twice");
                paid = Some(ev);
            }
        }
        let (who, index, w) = paid.expect("the jump never bonked the block");
        assert_eq!((who, usize::from(index)), (0, slot));
        assert!(LOOT_POOL.contains(&w), "{w} is not a pool gun");
        let p = &sim.players[0];
        assert_eq!(p.weapon, w);
        assert_eq!(p.ammo, weapon_stats(w).mag);
        assert_eq!(p.reserve, weapon_stats(w).reserve);
        assert!(sim.loot[slot].respawn_t > 0.0, "the block went dark");
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
        let mut sim = Sim::from_level(&old, 0, GameMode::Ffa);
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
            let b = Sim::from_level(&level, seed, GameMode::Ffa);
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
                hill: None,
            },
            seed,
            GameMode::Ffa,
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
            u64::from(b.from[0].to_bits()),
            u64::from(b.from[1].to_bits()),
            u64::from(b.from[2].to_bits()),
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
            u64::from(p.ads_fraction.to_bits()),
            u64::from(p.bloom.to_bits()),
            u64::from(p.spread.to_bits()),
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
        // Collision/ballistic fixture, not a shipped weapon: explicitly
        // remove its cone to test the launch geometry in isolation.
        for weapon in [SIDEARM, 5] {
            let mut sim = open_sim(1, 1);
            grant(&mut sim.players[0], weapon);
            let p = &mut sim.players[0];
            p.aim = [0.6, 0.8];
            p.pitch = 0.3;
            let mut stats = weapon_stats(weapon);
            stats.spread = 0.0;
            stats.spread_max = 0.0;
            for moving in [false, true] {
                let mut out = Vec::new();
                launch(p, &stats, moving, true, 7, 100, 0, &mut out);
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
            p.bloom = stats.spread_max;
            let cone = f64::from(stats.spread_max * weapon_handling(2).air_spread);
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
    fn added_bloom_widens_the_seeded_shot_cone() {
        // The Vityaz starts tight and opens up: the widest offset over the
        // first five rounds is smaller than over the last five.
        let mut sim = open_sim(1, 1);
        grant(&mut sim.players[0], 2);
        let stats = weapon_stats(2);
        let mut widest = |rounds: std::ops::Range<u8>| -> f64 {
            let mut w: f64 = 0.0;
            for fired in rounds {
                sim.players[0].bloom = f32::from(fired) * stats.bloom;
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
        // A settled sniper is precise but never a perfect mathematical ray.
        // Identical seeds let us compare stance without comparing luck.
        let mut sim = open_sim(1, 1);
        grant(&mut sim.players[0], 6);
        let sniper = weapon_stats(6);
        sim.players[0].pitch = 0.2;
        sim.players[0].ads_fraction = 1.0;
        for tick in 0..200u64 {
            let mut out = Vec::new();
            launch(&sim.players[0], &sniper, false, true, 7, tick, 0, &mut out);
            let offset = offset_angle(&out[0], [1.0, 0.0], 0.2, sniper.speed);
            assert!(offset > 0.0 && offset <= f64::from(sniper.spread * sniper.ads_spread));
        }
        grant(&mut sim.players[0], 3);
        sim.players[0].pitch = 0.0;
        sim.players[0].bloom = weapon_stats(3).spread_max;
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
        // The scalar cone has the exact multiplier. The resulting angular
        // offsets are not linear (yaw/elevation are projected through tan).
        let ground_cone = weapon_spread(3, 0.0, ak.spread_max, false, false, true);
        let air_cone = weapon_spread(3, 0.0, ak.spread_max, false, false, false);
        assert_eq!(air_cone, ground_cone * weapon_handling(3).air_spread);
        assert!(airborne <= f64::from(air_cone));
    }

    #[test]
    fn every_weapon_has_real_hip_ads_and_crouched_ads_shot_clouds() {
        // Launch the actual shipped rows with matched seeded samples. Compare
        // directions, never bullet height (crouching lowers the muzzle).
        for id in 1..=WEAPON_COUNT {
            let mut sim = open_sim(7, 1);
            arm(&mut sim.players[0], id);
            let stats = weapon_stats(id);
            let mut clouds = Vec::new();
            for (ads, crouch, moving, grounded) in [
                (0.0, false, false, true),
                (1.0, false, false, true),
                (1.0, true, false, true),
                (1.0, false, true, true),
                (1.0, true, false, false),
            ] {
                let p = &mut sim.players[0];
                p.aim = [1.0, 0.0];
                p.pitch = 0.2;
                p.ads_fraction = ads;
                p.crouch = crouch;
                let cone = weapon_spread(id, ads, 0.0, crouch, moving, grounded);
                let mut sum = 0.0;
                for tick in 0..256 {
                    let mut out = Vec::new();
                    launch(p, &stats, moving, grounded, 37, tick, 0, &mut out);
                    let angle = offset_angle(&out[0], p.aim, p.pitch, stats.speed);
                    assert!(
                        angle > 0.0 && angle <= f64::from(cone),
                        "weapon {id}: {angle}/{cone}"
                    );
                    sum += angle * angle;
                }
                clouds.push((sum / 256.0).sqrt());
            }
            assert!(
                clouds[1] < clouds[0] * 0.5,
                "weapon {id}: hip/ads {clouds:?}"
            );
            assert!(
                clouds[2] < clouds[1] * 0.8,
                "weapon {id}: crouch {clouds:?}"
            );
            assert!(
                clouds[3] > clouds[1] * 1.5,
                "weapon {id}: moving {clouds:?}"
            );
            assert!(clouds[4] > clouds[1] * 1.5, "weapon {id}: air {clouds:?}");
        }
    }

    #[test]
    fn sights_take_weapon_specific_time_and_do_not_quickscope() {
        for id in 1..=WEAPON_COUNT {
            let mut sim = open_sim(8, 1);
            arm(&mut sim.players[0], id);
            sim.players[0].pos = [0.0, 0.0];
            let input = PlayerIn {
                ads: true,
                ..Default::default()
            };
            sim.step(&|_| input);
            let first = sim.players[0].ads_fraction;
            assert!(first > 0.0 && first < 0.13, "weapon {id}: {first}");
            assert!(sim.players[0].spread > weapon_spread(id, 1.0, 0.0, false, false, true) * 1.5);
            let in_ticks = (weapon_handling(id).ads_in_secs / FIXED_DT).ceil() as usize;
            for _ in 1..=in_ticks {
                sim.step(&|_| input);
            }
            assert_eq!(sim.players[0].ads_fraction, 1.0, "weapon {id}");
            sim.step(&|_| PlayerIn::default());
            assert!(sim.players[0].ads_fraction > 0.0 && sim.players[0].ads_fraction < 1.0);
            for _ in 0..(weapon_handling(id).ads_out_secs / FIXED_DT).ceil() as usize {
                sim.step(&|_| PlayerIn::default());
            }
            assert_eq!(sim.players[0].ads_fraction, 0.0, "weapon {id}");
        }
        // Firing on the first requested scope tick uses the partial raise,
        // not the final sniper cone. This is the authoritative bullet path.
        let mut sim = open_sim(9, 1);
        arm(&mut sim.players[0], 6);
        sim.players[0].pos = [0.0, 0.0];
        sim.step(&|_| PlayerIn {
            ads: true,
            fire: true,
            ..Default::default()
        });
        let bullet = sim
            .bullets
            .first()
            .expect("sniper crosses 15 m, inside arena");
        let angle = offset_angle(bullet, [1.0, 0.0], 0.0, weapon_stats(6).speed);
        // Subtract this tick's gravity before comparing launch elevation.
        let mut launch_bullet = *bullet;
        launch_bullet.vy -= weapon_stats(6).gravity * FIXED_DT;
        assert!(
            offset_angle(&launch_bullet, [1.0, 0.0], 0.0, weapon_stats(6).speed)
                > f64::from(weapon_spread(6, 1.0, 0.0, false, false, true))
        );
        assert!(angle.is_finite());
    }

    #[test]
    fn handling_uses_real_displacement_and_jumps_keep_an_air_cone() {
        for id in 1..=WEAPON_COUNT {
            let mut sim = open_sim(10, 1);
            arm(&mut sim.players[0], id);
            sim.players[0].pos = [ARENA_HALF - PLAYER_R, 0.0];
            sim.players[0].ads_fraction = 1.0;
            // Holding against the arena edge is not movement.
            sim.step(&|_| PlayerIn {
                mv: [1.0, 0.0],
                ads: true,
                ..Default::default()
            });
            assert_eq!(
                sim.players[0].spread,
                weapon_spread(id, 1.0, 0.0, false, false, true)
            );
            sim.step(&|_| PlayerIn {
                mv: [-1.0, 0.0],
                ads: true,
                ..Default::default()
            });
            assert_eq!(
                sim.players[0].spread,
                weapon_spread(id, 1.0, 0.0, false, true, true)
            );
            sim.step(&|_| PlayerIn {
                jump: true,
                crouch: true,
                ads: true,
                ..Default::default()
            });
            assert!(sim.players[0].y > 0.0, "weapon {id}: unchanged jump");
            assert!(sim.players[0].spread >= weapon_handling(id).air_floor);
            assert_eq!(
                sim.players[0].spread,
                weapon_spread(id, 1.0, 0.0, true, false, false)
            );
        }
    }

    #[test]
    fn bloom_recovers_without_a_reload_and_state_reports_next_shot() {
        let mut sim = open_sim(11, 1);
        arm(&mut sim.players[0], 3);
        sim.players[0].pos = [0.0, 0.0];
        for _ in 0..100 {
            sim.step(&|_| PlayerIn {
                fire: true,
                ads: true,
                ..Default::default()
            });
            let p = &sim.players[0];
            assert_eq!(p.recoil_bloom(), p.bloom);
            assert_eq!(
                p.spread,
                weapon_spread(3, p.ads_fraction, p.bloom, false, false, true)
            );
        }
        assert!(sim.players[0].bloom > 0.0);
        let ammo = sim.players[0].ammo;
        assert!(ammo > 0 && ammo < weapon_stats(3).mag);
        let mut previous = sim.players[0].bloom;
        for _ in 0..240 {
            sim.step(&|_| PlayerIn {
                ads: true,
                ..Default::default()
            });
            assert!(sim.players[0].bloom <= previous);
            previous = sim.players[0].bloom;
        }
        let p = &sim.players[0];
        assert_eq!(p.ammo, ammo, "recovery did not refill or spend ammunition");
        assert!(
            p.fired > 0,
            "lifetime magazine count remains, but accuracy recovers"
        );
        assert_eq!(p.bloom, 0.0);
        assert_eq!(p.recoil_bloom(), 0.0);
        assert_eq!(p.spread, weapon_spread(3, 1.0, 0.0, false, false, true));
    }

    #[test]
    fn scope_cannot_survive_reload_shield_melee_death_or_round_pause() {
        for cause in 0..7 {
            let mut sim = open_sim(12, 1);
            arm(&mut sim.players[0], 3);
            sim.players[0].pos = [0.0, 0.0];
            sim.players[0].ammo -= 1;
            sim.players[0].ads_fraction = 1.0;
            sim.players[0].bloom = 0.02;
            let mut input = PlayerIn {
                ads: true,
                ..Default::default()
            };
            match cause {
                0 => input.reload = true,
                1 => input.shield = true,
                2 => input.melee = true,
                3 => sim.players[0].melee_cd = 0.4,
                4 => {
                    sim.players[0].alive = false;
                    sim.players[0].respawn_in = 1.0;
                }
                5 => sim.round_pause = 1.0,
                6 => sim.players[0].score = FFA_FRAG_LIMIT,
                _ => unreachable!(),
            }
            sim.step(&|_| input);
            assert_eq!(sim.players[0].ads_fraction, 0.0, "cause {cause}");
            if matches!(cause, 0 | 4 | 5 | 6) {
                assert_eq!(sim.players[0].bloom, 0.0, "cause {cause}");
            }
        }
        let mut sim = open_sim(12, 1);
        sim.players[0].ads_fraction = 1.0;
        sim.players[0].bloom = 0.03;
        grant(&mut sim.players[0], 6);
        assert_eq!(
            (sim.players[0].ads_fraction, sim.players[0].bloom),
            (0.0, 0.0)
        );
        sim.players[0].ammo = 0;
        sim.players[0].reserve = 0;
        sim.players[0].ads_fraction = 1.0;
        sim.players[0].bloom = 0.02;
        sim.step(&|_| PlayerIn {
            ads: true,
            ..Default::default()
        });
        assert_eq!(sim.players[0].weapon, SIDEARM);
        assert_eq!(
            (sim.players[0].ads_fraction, sim.players[0].bloom),
            (0.0, 0.0)
        );

        // A genuine lethal bullet clears the victim in the damage pass,
        // not just when the next tick notices an already-dead body.
        let mut sim = open_sim(13, 2);
        sim.players[0].pos = [0.0, 0.0];
        sim.players[1].pos = [3.0, 0.0];
        sim.players[1].hp = 1;
        sim.players[1].ads_fraction = 1.0;
        sim.players[1].bloom = 0.02;
        sim.step(&|id| PlayerIn {
            fire: id == 0,
            ads: id == 1,
            ..Default::default()
        });
        assert!(!sim.players[1].alive);
        assert_eq!(
            (sim.players[1].ads_fraction, sim.players[1].bloom),
            (0.0, 0.0)
        );
    }

    #[test]
    fn raising_sights_does_not_change_ground_speed_or_jump_reach() {
        let mut hip = open_sim(14, 1);
        let mut aimed = open_sim(14, 1);
        for sim in [&mut hip, &mut aimed] {
            sim.players[0].pos = [-10.0, 0.0];
        }
        for tick in 0..120 {
            let input = PlayerIn {
                mv: [1.0, 0.0],
                jump: tick == 30,
                crouch: tick >= 90,
                ..Default::default()
            };
            hip.step(&|_| input);
            aimed.step(&|_| PlayerIn { ads: true, ..input });
            let (h, a) = (&hip.players[0], &aimed.players[0]);
            assert_eq!(h.pos, a.pos, "tick {tick}");
            assert_eq!(
                (h.y.to_bits(), h.vy.to_bits()),
                (a.y.to_bits(), a.vy.to_bits()),
                "tick {tick}"
            );
        }
    }

    #[test]
    fn handling_helpers_reject_nonfinite_inputs_and_spawn_is_a_weak_pistol() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(advance_ads(bad, false, 6, FIXED_DT), 0.0);
            assert_eq!(advance_ads(0.4, true, 6, bad), 0.4);
            assert_eq!(
                weapon_spread(6, bad, bad, false, false, true),
                weapon_stats(6).spread
            );
        }
        let sim = open_sim(13, 1);
        let p = &sim.players[0];
        let s = weapon_stats(SIDEARM);
        assert_eq!(
            (p.weapon, p.ammo, p.reserve),
            (SIDEARM, 6, RESERVE_INFINITE)
        );
        assert_eq!(p.ads_fraction, 0.0);
        assert_eq!(p.spread, s.spread);
        assert_eq!(s.damage, 1, "head/body damage rules are unchanged");
        assert!((s.ttl * s.speed - 30.0).abs() < 1e-5);
        for id in [2, 3, 4] {
            let loot = weapon_stats(id);
            assert!(s.cooldown > loot.cooldown * 2.0 && s.mag < loot.mag);
            assert!(s.ttl * s.speed < loot.ttl * loot.speed);
        }
    }

    #[test]
    fn a_revolver_slug_drops_under_gravity_and_keeps_its_horizontal_speed() {
        // Level fire from eight metres up, from the west wall so the slug
        // has the arena's width of air: at 7.5 m a tick it meets the east
        // wall on the sixth tick, so it is read after five. The horizontal
        // speed is exactly the table's, and the drop is semi-implicit
        // Euler's g dt^2 n(n+1)/2, which a half g t^2 lands a hair under.
        let mut sim = open_sim(1, 1);
        arm(&mut sim.players[0], 5);
        let stats = weapon_stats(5);
        let mut inputs = HashMap::new();
        let mut eye = 0.0f32;
        let ticks = 5u32;
        for t in 0..ticks {
            hold(&mut sim, &[(0, [-20.0, 0.0], 8.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            step_geometry(&mut sim, &inputs);
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
        let n = ticks as f32;
        let expected = stats.gravity.abs() * FIXED_DT * FIXED_DT * n * (n + 1.0) / 2.0;
        assert!(
            (fell - expected).abs() < 1e-3,
            "fell {fell}, expected {expected}"
        );
        assert!(b.vy < 0.0, "the slug is falling");
    }

    #[test]
    fn a_zero_gravity_round_flies_the_old_straight_line() {
        // A sidearm round's height each tick is the v17 formula, `y += vy
        // dt`, bit for bit: a zero gravity row adds exactly zero. Six
        // ticks from the west wall: the seventh expires the 30 m pistol.
        let mut sim = open_sim(1, 1);
        let pitch = 0.3f32;
        let vy = pitch.tan() * BULLET_SPEED;
        let mut inputs = HashMap::new();
        let mut y = 0.0f32;
        for t in 0..6u32 {
            hold(&mut sim, &[(0, [-20.0, 0.0], 8.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], pitch));
            step_geometry(&mut sim, &inputs);
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
            step_geometry(&mut sim, &inputs);
        }
        let hp = (1..=xs.len() as u8).map(|id| player(&sim, id).hp).collect();
        (sim, hp)
    }

    #[test]
    fn a_sniper_round_passes_through_one_body_and_hits_the_next() {
        // Three targets on a line 3 m apart: the first two lose two points
        // each, the third is untouched, because pierce 1 is two bodies.
        // All on the first tick, at 15 m a tick, and the two events chain:
        // the second body's segment starts where the first's ended.
        let (sim, hp) = sniper_line(Vec::new(), &[3.0, 6.0, 9.0], 1);
        assert_eq!(hp, vec![1, 1, MAX_HP]);
        assert!(
            sim.bullets.is_empty(),
            "the round stopped in the second body"
        );
        let shots = &sim.shots;
        assert_eq!(shots.len(), 2, "{shots:?}");
        assert_eq!((shots[0].hit, shots[0].victim), (SHOT_BODY, 1));
        assert_eq!((shots[1].hit, shots[1].victim), (SHOT_BODY, 2));
        assert!(
            same_point(shots[1].from, shots[0].to),
            "the events chain end to end: {shots:?}"
        );
        assert!(shots[0].to[0] < shots[1].to[0], "in segment order");
    }

    #[test]
    fn two_bodies_on_one_segment_are_both_hit() {
        // Two bodies ten metres apart inside one tick's fifteen metres of
        // sniper travel: both are hit on the SAME tick, which is what the
        // loop continuing after a pierced hit buys. A break would have
        // skipped the second for good, because the next tick's segment
        // starts beyond it.
        let mut sim = open_sim(1, 3);
        arm(&mut sim.players[0], 6);
        let spots = [
            (0, [0.0, 0.0], 0.0),
            (1, [4.05, 0.0], 0.0),
            (2, [14.05, 0.0], 0.0),
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
            step_geometry(&mut sim, &inputs);
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
        // A body is 1.64 m across; at v18's metre a tick the round
        // overlapped it on consecutive ticks and the mask was what made
        // that one hit. At 15 m a tick the segment leaves the body the
        // tick it entered, and the mask still stands for a round whose
        // next segment starts inside a body (a reflected one, say).
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
        // Caught at 20 m (on tick 1 of 15 m a tick) and read after tick 2,
        // on its way back: the pierce and the mask are gone, the weapon
        // stays.
        let sim = shield_duel_with(6, 20.0, [-1.0, 0.0], true, 3);
        let b = sim.bullets.first().expect("the round is coming back");
        assert_eq!(b.owner, 1, "caught");
        assert!(b.vel[0] < 0.0);
        assert_eq!(b.weapon, 6);
        assert_eq!(b.pierce, 0, "a caught round pierces nothing");
        assert_eq!(b.hit_mask, 0);
        // The sniper round (2 damage) came home: a body hit, not a kill.
        let sim = shield_duel_with(6, 20.0, [-1.0, 0.0], true, 20);
        assert_eq!(player(&sim, 0).hp, MAX_HP - 2);
        assert_eq!(player(&sim, 1).hp, MAX_HP);
    }

    #[test]
    fn a_reflected_round_keeps_its_weapon_and_gravity() {
        // A revolver slug (7.5 m a tick, so caught at 20 m on tick 2) read
        // after ticks 3 and 4: still a revolver slug, still falling faster.
        let a = shield_duel_with(5, 20.0, [-1.0, 0.0], true, 4);
        let b = shield_duel_with(5, 20.0, [-1.0, 0.0], true, 5);
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
    /// targets wherever the test puts them. The world pass ends the rocket
    /// where its segment enters the container, the face at x 5, and the
    /// blast is pushed back off it by `BLAST_STANDOFF` so the splash's
    /// line-of-sight test does not start on the face. Returns the sim
    /// after the blast.
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
            step_geometry(&mut sim, &idle);
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
            step_geometry(&mut sim, &inputs);
            if !sim.blasts.is_empty() {
                assert_eq!(sim.blasts.len(), 1);
                let ([x, y, z], owner) = sim.blasts[0];
                assert!(
                    (x - (5.0 - BLAST_STANDOFF)).abs() < 1e-3,
                    "went off at x {x}, not on the face"
                );
                let s = sim
                    .shots
                    .iter()
                    .find(|s| s.hit == SHOT_COVER)
                    .expect("the shot ended on cover");
                assert_eq!(s.cover, Cover::Container.index());
                assert_eq!(s.normal, [-1, 0, 0]);
                assert!(
                    (s.to[0] - 5.0).abs() < 1e-3,
                    "the event is on the face: {:?}",
                    s.to
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
        // One tick of flight, with the sustainer's first charge on it.
        let stats = weapon_stats(7);
        let first_tick = (stats.speed + stats.accel * FIXED_DT) * FIXED_DT;
        assert!(
            (ttl[0] - (-20.0 + 0.2 + first_tick)).abs() < 1e-3,
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
    fn a_fast_round_does_not_tunnel_through_a_wall() {
        // The sniper's 900 m/s round crosses 15 m a tick. Fired level at a
        // 0.4 m trench wall three metres away with a body ten metres
        // behind it: the round ends on the wall's near face, the event
        // says cover and names the wall, the mark's normal faces the
        // shooter, and the body is untouched. The v18 sweep tested cover
        // at the tick's end point, 12 m past the wall, and would have
        // hit the body.
        let wall = Obstacle::boxed(Cover::Wall, [3.0, -2.0], [3.4, 2.0], 0.0, 2.6);
        let (sim, hp) = sniper_line(vec![wall], &[13.0], 1);
        assert_eq!(hp, vec![MAX_HP], "the body behind the wall is untouched");
        assert!(sim.bullets.is_empty(), "the round is gone");
        assert_eq!(sim.shots.len(), 1, "{:?}", sim.shots);
        let s = sim.shots[0];
        assert_eq!((s.owner, s.weapon), (0, 6));
        assert_eq!(s.hit, SHOT_COVER);
        assert_eq!(s.cover, Cover::Wall.index());
        assert_eq!(s.victim, SHOT_NONE);
        assert!(
            (s.to[0] - 3.0).abs() < 0.05,
            "stopped on the near face: {:?}",
            s.to
        );
        assert!((s.to[1] - EYE_STAND).abs() < 1e-3 && s.to[2].abs() < 1e-6);
        assert_eq!(s.normal, [-1, 0, 0], "the face the round came in through");
        assert!(
            (s.from[0] - 0.2).abs() < 1e-6 && (s.from[1] - EYE_STAND).abs() < 1e-3,
            "from the muzzle: {:?}",
            s.from
        );
        // Control: with the wall gone the same round reaches the body.
        let (_, hp) = sniper_line(Vec::new(), &[13.0], 1);
        assert_eq!(hp, vec![1]);
    }

    #[test]
    fn the_first_box_on_the_segment_wins() {
        // A crate at 5 and a container at 10 on the line, listed container
        // first: the round ends on the crate's near face and the event
        // names the crate, not the first box in the list and not the box
        // the segment's end lies past. The sniper's 15 m tick reaches
        // beyond both.
        let crate_box = Obstacle::boxed(Cover::Crate, [5.0, -1.0], [6.0, 1.0], 0.0, 1.5);
        let container = Obstacle::boxed(Cover::Container, [10.0, -1.0], [11.0, 1.0], 0.0, 2.6);
        for order in [vec![container, crate_box], vec![crate_box, container]] {
            let (sim, _) = sniper_line(order, &[], 1);
            assert_eq!(sim.shots.len(), 1);
            let s = sim.shots[0];
            assert_eq!(s.hit, SHOT_COVER);
            assert_eq!(s.cover, Cover::Crate.index(), "{s:?}");
            assert!((s.to[0] - 5.0).abs() < 1e-3, "{:?}", s.to);
            assert_eq!(s.normal, [-1, 0, 0]);
        }
    }

    #[test]
    fn the_rocket_reaches_its_sustainer_speed_in_half_a_second() {
        // The horizontal speed climbs from the table's 120 by 360 m/s^2,
        // charged once a tick before the segment like gravity, and caps at
        // 300 on the thirtieth tick. The rocket is held at the origin
        // between ticks so the wall never ends it: in the arena the cap
        // lies 105 m out, beyond any wall, which the row's comment says.
        let mut sim = open_sim(1, 1);
        arm(&mut sim.players[0], 7);
        let stats = weapon_stats(7);
        let mut inputs = HashMap::new();
        let mut speeds = Vec::new();
        for t in 0..40u32 {
            hold(&mut sim, &[(0, [0.0, 0.0], 8.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            step_geometry(&mut sim, &inputs);
            let b = sim.bullets.first_mut().expect("the rocket is in flight");
            speeds.push((b.vel[0] * b.vel[0] + b.vel[1] * b.vel[1]).sqrt());
            assert_eq!(b.vel[1], 0.0, "tick {t}: the direction is kept");
            b.pos = [0.0, 0.0];
            b.y = 8.0;
        }
        for (t, &s) in speeds.iter().enumerate() {
            let charges = t as f32 + 1.0;
            let expected = (stats.speed + stats.accel * FIXED_DT * charges).min(stats.speed_max);
            assert!((s - expected).abs() < 1e-2, "tick {t}: {s} vs {expected}");
        }
        assert!(speeds[28] < stats.speed_max - 1.0, "not yet at tick 29");
        assert!(
            (speeds[29] - stats.speed_max).abs() < 1e-3,
            "at half a second the cap: {}",
            speeds[29]
        );
        assert!(
            (speeds[39] - stats.speed_max).abs() < 1e-3,
            "and it stays there: {}",
            speeds[39]
        );
        // And a bullet row has no motor: the sidearm's speed is the same
        // bits on every tick it flies.
        let mut sim = open_sim(1, 1);
        let mut inputs = HashMap::new();
        for t in 0..5u32 {
            hold(&mut sim, &[(0, [-20.0, 0.0], 8.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            step_geometry(&mut sim, &inputs);
            let b = sim.bullets.first().expect("in flight");
            assert_eq!(b.vel[0].to_bits(), BULLET_SPEED.to_bits(), "tick {t}");
        }
    }

    #[test]
    fn every_round_ends_in_exactly_one_shot_event_per_segment() {
        // Four players on the yard for 400 ticks of the v18 script with
        // the sidearm only (the blocks and pads removed, so nothing
        // pierces and nothing is a rocket) and the reload key ignored (the
        // script's reload, against the sidearm's bottomless reserve, would
        // spend most of the run reloading): on every tick the rounds that
        // left the list are exactly the events that end a segment for
        // good; a reflection leaves the round in the list and starts its
        // next segment where the event ended; and every event is a finite
        // segment on or inside the arena with the fields its kind says.
        let mut level = Level::freight_yard();
        level.obstacles.retain(|o| o.kind != Cover::Loot);
        level.pads.clear();
        let mut sim = Sim::from_level(&level, 7, GameMode::Ffa);
        for id in 0..4 {
            sim.add_player(id);
        }
        let mut segments = 0usize;
        let mut kinds = [0usize; 6];
        for tick in 0..600u64 {
            let before = sim.bullets.len();
            let ammo: Vec<u8> = sim.players.iter().map(|p| p.ammo).collect();
            sim.step(&|id| PlayerIn {
                reload: false,
                ..v18_script(tick, id)
            });
            // A magazine only ever drops by the rounds that left it; a
            // refill or a respawn raises it and counts as nothing.
            let launched: usize = sim
                .players
                .iter()
                .zip(&ammo)
                .map(|(p, &was)| usize::from(was.saturating_sub(p.ammo)))
                .sum();
            let removed = before + launched - sim.bullets.len();
            let ended = sim.shots.iter().filter(|s| s.hit != SHOT_SHIELD).count();
            assert_eq!(removed, ended, "tick {tick}: {:?}", sim.shots);
            for s in &sim.shots {
                assert_eq!(s.weapon, SIDEARM, "tick {tick}: {s:?}");
                assert!(s.owner < 4, "tick {tick}: {s:?}");
                for k in 0..3 {
                    assert!(
                        s.from[k].is_finite() && s.to[k].is_finite(),
                        "tick {tick}: {s:?}"
                    );
                }
                assert!(
                    s.to[0].abs() <= ARENA_HALF && s.to[2].abs() <= ARENA_HALF && s.to[1] >= 0.0,
                    "tick {tick}: {s:?}"
                );
                match s.hit {
                    SHOT_COVER => {
                        assert_ne!(s.cover, SHOT_NONE, "tick {tick}: {s:?}");
                        assert_eq!(s.victim, SHOT_NONE, "tick {tick}: {s:?}");
                        assert_ne!(s.normal, [0, 0, 0], "tick {tick}: {s:?}");
                    }
                    SHOT_BODY | SHOT_SHIELD => {
                        assert_eq!(s.cover, SHOT_NONE, "tick {tick}: {s:?}");
                        assert!(s.victim < 4 && s.victim != s.owner, "tick {tick}: {s:?}");
                        assert_eq!(s.normal, [0, 0, 0], "tick {tick}: {s:?}");
                    }
                    SHOT_FLOOR => assert_eq!(s.normal, [0, 1, 0], "tick {tick}: {s:?}"),
                    SHOT_WALL => assert_ne!(s.normal, [0, 0, 0], "tick {tick}: {s:?}"),
                    SHOT_EXPIRED => assert_eq!(s.normal, [0, 0, 0], "tick {tick}: {s:?}"),
                    other => panic!("tick {tick}: unknown kind {other}"),
                }
                if s.hit == SHOT_SHIELD {
                    assert!(
                        sim.bullets
                            .iter()
                            .any(|b| same_point(b.from, s.to) && b.owner == s.victim),
                        "tick {tick}: the caught round's next segment starts at the plate"
                    );
                }
                kinds[usize::from(s.hit)] += 1;
            }
            segments += sim.shots.len();
        }
        // Eight rounds at 0.18 s, then 1.1 s of auto-reload, on half the
        // ticks: about twenty rounds a player over the run.
        assert!(segments > 60, "the script fired {segments} segments");
        assert!(
            kinds[usize::from(SHOT_COVER)] > 0 && kinds[usize::from(SHOT_FLOOR)] > 0,
            "{kinds:?}"
        );
        assert!(
            kinds[usize::from(SHOT_WALL)] > 0 || kinds[usize::from(SHOT_EXPIRED)] > 0,
            "{kinds:?}"
        );

        // A reflection, pinned on its own: the plate ends one segment and
        // the round's next starts there, owned by the catcher.
        let sim = shield_duel(20.0, [-1.0, 0.0], true, 5);
        assert_eq!(sim.shots.len(), 1, "{:?}", sim.shots);
        let s = sim.shots[0];
        assert_eq!(
            (s.hit, s.victim, s.owner),
            (SHOT_SHIELD, 1, 0),
            "the shooter's segment"
        );
        let b = sim.bullets.first().expect("still flying");
        assert!(same_point(b.from, s.to), "{:?} vs {:?}", b.from, s.to);
        assert_eq!(b.owner, 1);
        assert!(
            (s.to[0] - (20.0 - hit_radius(false) - BULLET_R)).abs() < 1e-3,
            "{:?}",
            s.to
        );
    }

    /// What `Sim.hits` holds: (shooter, victim, damage, head), as the
    /// drivers below collect it across ticks.
    type HitList = Vec<(u8, u8, u8, bool)>;

    #[test]
    fn the_table_flies_real_muzzle_velocities() {
        // Section 3.1 of the v20 plan, pinned row by row: the muzzle
        // velocity, the range the ttl spells, the gravity, the cap and the
        // pierce of every bullet, and the rocket's booster, sustainer and
        // cap. These are the numbers that ship; a retune is a deliberate
        // edit here and in the plan, never a drift.
        let rows: [(u8, f32, f32, f32, u8); 6] = [
            (SIDEARM, 280.0, 30.0, 0.0, 0),
            (2, 380.0, 40.0, 0.0, 0),
            (3, 715.0, 80.0, 0.0, 0),
            (4, 880.0, 80.0, 0.0, 0),
            (5, 450.0, 60.0, -9.81, 0),
            (6, 900.0, 120.0, -9.81, 1),
        ];
        for (id, speed, range, gravity, pierce) in rows {
            let s = weapon_stats(id);
            assert_eq!(s.speed.to_bits(), speed.to_bits(), "{}: speed", s.name);
            assert!(
                (s.speed * s.ttl - range).abs() < 1e-3,
                "{}: range {} is not {range}",
                s.name,
                s.speed * s.ttl
            );
            assert_eq!(
                s.gravity.to_bits(),
                gravity.to_bits(),
                "{}: gravity",
                s.name
            );
            assert_eq!(s.accel.to_bits(), 0.0f32.to_bits(), "{}: a motor", s.name);
            assert_eq!(s.speed_max.to_bits(), speed.to_bits(), "{}: cap", s.name);
            assert_eq!(s.pierce, pierce, "{}: pierce", s.name);
            assert_eq!(s.kind, Projectile::Bullet, "{}", s.name);
        }
        let r = weapon_stats(7);
        assert_eq!(r.kind, Projectile::Rocket);
        assert_eq!(
            [r.speed, r.accel, r.speed_max, r.ttl, r.gravity].map(f32::to_bits),
            [120.0f32, 360.0, 300.0, 5.0, -3.0].map(f32::to_bits)
        );
        assert!(
            ((r.speed_max - r.speed) / r.accel - 0.5).abs() < 1e-6,
            "the sustainer takes half a second to reach the cap"
        );
        // The sidearm's constants are the sidearm's row, and a round flies
        // six full segments of 4.67 m before its seventh charge of
        // ttl ends it: 28 m in the sim against 30 m in the table, the
        // tick's worth the expiry charge takes, as it always has.
        assert_eq!(BULLET_TTL.to_bits(), (30.0f32 / BULLET_SPEED).to_bits());
        assert!((BULLET_SPEED * FIXED_DT - 4.6667).abs() < 1e-3);
        assert_eq!((BULLET_TTL / FIXED_DT).ceil() as u32, 7);
        assert!((weapon_stats(6).speed * FIXED_DT - 15.0).abs() < 1e-4);
        // The bullet cap: rounds in flight from a held trigger is the ttl
        // over the cooldown, rounded up, per row. Every row is inside the
        // ten the cap allows with room to spare; the AK, revolver and
        // sniper never have two rounds up at once.
        let in_flight: Vec<usize> = (1..=WEAPON_COUNT)
            .map(|id| {
                let s = weapon_stats(id);
                (s.ttl / s.cooldown).ceil() as usize
            })
            .collect();
        assert_eq!(in_flight, vec![1, 2, 1, 2, 1, 1, 5]);
        assert!(in_flight.iter().all(|&n| n <= MAX_BULLETS_PER_PLAYER));
    }

    /// One shooter held at `spot` and `y`, firing once along `aim` at
    /// `pitch` on tick 0 against `obstacles`, run for `ticks`: every shot
    /// event with the tick it landed on.
    fn one_shot(
        obstacles: Vec<Obstacle>,
        spot: [f32; 2],
        y: f32,
        aim: [f32; 2],
        pitch: f32,
        ticks: u32,
    ) -> Vec<(u32, ShotEvent)> {
        let mut sim = open_sim(3, 1);
        sim.obstacles = obstacles;
        let mut inputs = HashMap::new();
        let mut out = Vec::new();
        for t in 0..ticks {
            hold(&mut sim, &[(0, spot, y)]);
            inputs.insert(0, shot(t, aim, pitch));
            step_geometry(&mut sim, &inputs);
            out.extend(sim.shots.iter().map(|&s| (t, s)));
        }
        assert!(sim.bullets.is_empty(), "the round is still in flight");
        out
    }

    #[test]
    fn the_world_pass_ends_a_round_at_the_first_crossing_of_every_kind() {
        // The entry-parameter pass, exit by exit: the floor, a box, each
        // arena wall and the expiry, every one reporting exactly one
        // event, on the tick the round ended, with `to` on the surface it
        // met and on the round's own line. Where two things lie on one
        // segment the nearer one is the answer and the box under the
        // segment's end point is never consulted.
        let lim = ARENA_HALF - BULLET_R;
        let crate_box = Obstacle::boxed(Cover::Crate, [4.0, -1.0], [5.5, 1.0], 0.0, 1.5);
        let wall = Obstacle::boxed(Cover::Wall, [1.0, -1.0], [1.4, 1.0], 0.0, 2.6);
        let pitch = -0.5f32;
        // Down at the floor with a crate under the segment's end: the
        // floor is crossed at 2.65 m out, the crate would be entered at
        // 3.8, and the floor is the answer.
        let shots = one_shot(vec![crate_box], [0.0, 0.0], 0.0, [1.0, 0.0], pitch, 3);
        assert_eq!(shots.len(), 1, "{shots:?}");
        let (t, s) = shots[0];
        assert_eq!(t, 0, "on the tick it was fired");
        assert_eq!(
            (s.hit, s.cover, s.victim),
            (SHOT_FLOOR, SHOT_NONE, SHOT_NONE)
        );
        assert_eq!(s.normal, [0, 1, 0]);
        assert_eq!(s.to[1].to_bits(), 0.0f32.to_bits(), "on the floor exactly");
        let reach = 0.2 + EYE_STAND / pitch.abs().tan();
        assert!((s.to[0] - reach).abs() < 1e-3, "{:?} vs {reach}", s.to);
        assert_eq!(s.to[2].to_bits(), 0.0f32.to_bits());
        assert!((s.from[0] - 0.2).abs() < 1e-6 && (s.from[1] - EYE_STAND).abs() < 1e-3);
        // The same shot with a wall 0.8 m out: entered on its near face
        // at the height the line has there, before the floor.
        let shots = one_shot(vec![wall], [0.0, 0.0], 0.0, [1.0, 0.0], pitch, 3);
        assert_eq!(shots.len(), 1, "{shots:?}");
        let (t, s) = shots[0];
        assert_eq!(t, 0);
        assert_eq!(s.hit, SHOT_COVER);
        assert_eq!((s.cover, s.victim), (Cover::Wall.index(), SHOT_NONE));
        assert_eq!(s.normal, [-1, 0, 0]);
        let at_y = EYE_STAND - 0.8 * pitch.abs().tan();
        assert!(
            (s.to[0] - 1.0).abs() < 1e-3 && (s.to[1] - at_y).abs() < 1e-3,
            "{:?}",
            s.to
        );
        // Each arena wall, from the air so nothing else is on the line:
        // the crossing coordinate is the wall exactly, the normal points
        // back in, and level fire keeps its height bit for bit.
        for (spot, aim, axis, sign) in [
            ([20.0, 0.0], [1.0, 0.0], 0, 1.0f32),
            ([-20.0, 0.0], [-1.0, 0.0], 0, -1.0),
            ([0.0, 20.0], [0.0, 1.0], 2, 1.0),
            ([0.0, -20.0], [0.0, -1.0], 2, -1.0),
        ] {
            let shots = one_shot(Vec::new(), spot, 8.0, aim, 0.0, 3);
            assert_eq!(shots.len(), 1, "{shots:?}");
            let (t, s) = shots[0];
            assert_eq!(t, 0, "{aim:?}");
            assert_eq!(
                (s.hit, s.cover, s.victim),
                (SHOT_WALL, SHOT_NONE, SHOT_NONE)
            );
            assert_eq!(
                s.to[axis].to_bits(),
                (sign * lim).to_bits(),
                "{aim:?}: {:?}",
                s.to
            );
            let mut normal = [0i8; 3];
            normal[axis] = if sign > 0.0 { -1 } else { 1 };
            assert_eq!(s.normal, normal, "{aim:?}");
            assert_eq!(s.to[1].to_bits(), s.from[1].to_bits(), "{aim:?}: level");
            assert_eq!(
                s.to[2 - axis].to_bits(),
                0.0f32.to_bits(),
                "{aim:?}: {:?}",
                s.to
            );
        }
        // Along the diagonal from a corner the wall is 61 m off and the
        // round runs out of flight time first: six segments of 4.67 m,
        // the event on the seventh tick, where the round faded.
        let d = std::f32::consts::FRAC_1_SQRT_2;
        let shots = one_shot(Vec::new(), [-20.0, -20.0], 8.0, [d, d], 0.0, 20);
        assert_eq!(shots.len(), 1, "{shots:?}");
        let (t, s) = shots[0];
        assert_eq!(t, 6);
        assert_eq!(
            (s.hit, s.cover, s.victim),
            (SHOT_EXPIRED, SHOT_NONE, SHOT_NONE)
        );
        assert_eq!(s.normal, [0, 0, 0]);
        let flown = ((s.to[0] - s.from[0]).powi(2) + (s.to[2] - s.from[2]).powi(2)).sqrt();
        assert!(
            (flown - 6.0 * BULLET_SPEED * FIXED_DT).abs() < 0.02,
            "{flown}"
        );
        assert_eq!(s.to[1].to_bits(), s.from[1].to_bits());
        assert!((s.from[0] - (-20.0 + 0.2 * d)).abs() < 1e-5, "{:?}", s.from);
    }

    #[test]
    fn a_tracer_ends_on_the_body_it_hit() {
        // A body event's `to` is the contact with the body volume: on the
        // hit circle (`hit_radius + BULLET_R` from the centre) at a height
        // inside the band, for every bullet row at its real speed, so the
        // tracer the client draws ends just short of the body and never
        // in it or past it. A round that enters the circle above the head
        // is judged where its height came down into the band, so `to` sits
        // on the top of the volume and not up in the air over it.
        let rr = hit_radius(false) + BULLET_R;
        for weapon in 1..=WEAPON_COUNT {
            let stats = weapon_stats(weapon);
            if stats.kind != Projectile::Bullet {
                continue;
            }
            let mut sim = open_sim(11, 2);
            arm(&mut sim.players[0], weapon);
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    ads: true,
                    ..shot(0, [1.0, 0.0], 0.0)
                },
            );
            hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [3.0, 0.0], 0.0)]);
            step_geometry(&mut sim, &inputs);
            assert_eq!(sim.hits.len(), 1, "{}: {:?}", stats.name, sim.hits);
            let s = sim.shots.first().expect("the body event");
            assert_eq!(
                (s.hit, s.victim, s.weapon),
                (SHOT_BODY, 1, weapon),
                "{}",
                stats.name
            );
            let dist = ((s.to[0] - 3.0).powi(2) + s.to[2].powi(2)).sqrt();
            assert!(
                (dist - rr).abs() < 1e-3,
                "{}: {:?} is {dist} from the centre",
                stats.name,
                s.to
            );
            assert!(s.to[0] < 3.0, "{}: on the near side", stats.name);
            assert!(
                (s.to[1] - EYE_STAND).abs() < 0.01,
                "{}: {:?}",
                stats.name,
                s.to
            );
            // The muzzle is 0.2 out along the round's own line, which a hip
            // cone turns a hair off the x axis.
            assert!((s.from[0] - 0.2).abs() < 1e-3 && (s.from[1] - EYE_STAND).abs() < 1e-3);
        }
        // Steeply down from 6 m up at a body 1.5 m out, aimed at its
        // centre: the round enters the circle 5.4 m up, far above the
        // 2.08 top of the volume, and the contact is where it crossed
        // that top. Through the head, so it kills; the sidearm and the
        // sniper both, the second at 15 m a tick.
        let from_y = 6.0;
        let gap = 1.5;
        let pitch = -((from_y + EYE_STAND - 1.0) / gap).atan();
        assert!(pitch.abs() < MAX_PITCH);
        let hi = BODY_H_STAND + BULLET_R;
        for weapon in [SIDEARM, 6] {
            let mut sim = open_sim(11, 2);
            arm(&mut sim.players[0], weapon);
            let mut inputs = HashMap::new();
            inputs.insert(
                0,
                PlayerIn {
                    ads: true,
                    ..shot(0, [1.0, 0.0], pitch)
                },
            );
            hold(&mut sim, &[(0, [0.0, 0.0], from_y), (1, [gap, 0.0], 0.0)]);
            step_geometry(&mut sim, &inputs);
            assert_eq!(sim.hits, vec![(0, 1, MAX_HP, true)], "weapon {weapon}");
            let s = sim.shots.first().expect("the body event");
            assert_eq!((s.hit, s.victim), (SHOT_BODY, 1), "weapon {weapon}");
            assert!((s.to[1] - hi).abs() < 1e-3, "weapon {weapon}: {:?}", s.to);
            let x = 0.2 + (s.from[1] - hi) / pitch.abs().tan();
            assert!(
                (s.to[0] - x).abs() < 1e-2,
                "weapon {weapon}: {:?} vs {x}",
                s.to
            );
            let dist = ((s.to[0] - gap).powi(2) + s.to[2].powi(2)).sqrt();
            assert!(dist < rr, "weapon {weapon}: inside the circle's footprint");
        }
    }

    #[test]
    fn a_point_blank_hit_on_a_body_against_cover_still_lands() {
        // The body's back is to a container 0.4 m behind its circle and
        // the sidearm's 4.67 m segment ends inside the container: the
        // cover gate is judged at the contact, which lies before the box,
        // so the hit lands and the round's one event is the body. The
        // v18 sweep would have agreed here (it tested cover at contact
        // too); what is pinned is that the exact pass did not lose it.
        // The control puts a thin wall between them instead: cover, and
        // no hit.
        let run = |box_: Obstacle| -> (HitList, Vec<ShotEvent>) {
            let mut sim = open_sim(5, 2);
            sim.obstacles = vec![box_];
            let mut inputs = HashMap::new();
            inputs.insert(0, shot(0, [1.0, 0.0], 0.0));
            hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [2.0, 0.0], 0.0)]);
            step_with(&mut sim, &inputs);
            (sim.hits.clone(), sim.shots.clone())
        };
        let behind = Obstacle::boxed(Cover::Container, [3.0, -1.0], [4.0, 1.0], 0.0, 2.6);
        let (hits, shots) = run(behind);
        assert_eq!(hits, vec![(0, 1, 1, false)]);
        assert_eq!(shots.len(), 1, "{shots:?}");
        assert_eq!((shots[0].hit, shots[0].victim), (SHOT_BODY, 1));
        let between = Obstacle::boxed(Cover::Wall, [0.6, -1.0], [0.8, 1.0], 0.0, 2.6);
        let (hits, shots) = run(between);
        assert!(hits.is_empty(), "{hits:?}");
        assert_eq!(shots.len(), 1, "{shots:?}");
        assert_eq!(
            (shots[0].hit, shots[0].cover),
            (SHOT_COVER, Cover::Wall.index())
        );
        assert!((shots[0].to[0] - 0.6).abs() < 1e-3, "{:?}", shots[0].to);
    }

    #[test]
    fn a_rewound_headshot_uses_the_head_where_it_was() {
        // The target stood on a crate top (feet 1.5 up) while the shooter
        // took aim, then dropped to the floor. A shooter twelve ticks
        // behind fires at the head where it was, 3.2 m up: the rewound
        // body's exact head band is up there and it is a kill, and the
        // tracer ends up in the air where the shooter saw the head. The
        // same round in the present passes clean over a body whose top is
        // at 2.08. The v18 sampled band and the v20 interval must agree on
        // the rewind reading height and stance with position; this pins
        // the height at the new speed.
        let pitch = ((3.2 - EYE_STAND) / 5.8).atan();
        let run = |delay: u16| -> (HitList, Vec<ShotEvent>) {
            let mut sim = open_sim(12, 2);
            let idle = HashMap::new();
            for _ in 0..15 {
                hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [6.0, 0.0], 1.5)]);
                step_with(&mut sim, &idle);
            }
            // The drop, and the shot: the round reaches the body on its
            // second tick, still deep inside the rewind window.
            let mut inputs = HashMap::new();
            let (mut hits, mut shots) = (Vec::new(), Vec::new());
            for t in 0..3u32 {
                hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [6.0, 0.0], 0.0)]);
                inputs.insert(
                    0,
                    PlayerIn {
                        delay_ticks: delay,
                        ..shot(t, [1.0, 0.0], pitch)
                    },
                );
                step_with(&mut sim, &inputs);
                hits.extend(sim.hits.iter().copied());
                shots.extend(sim.shots.iter().copied());
            }
            (hits, shots)
        };
        let (hits, shots) = run(12);
        assert_eq!(
            hits,
            vec![(0, 1, MAX_HP, true)],
            "judged where the shooter saw it"
        );
        let s = shots.first().expect("the body event");
        assert_eq!((s.hit, s.victim), (SHOT_BODY, 1));
        assert!(
            s.to[1] > 1.5 + head_lo(false) - 0.3,
            "the contact is up at the old head: {:?}",
            s.to
        );
        let (hits, shots) = run(0);
        assert!(
            hits.is_empty(),
            "in the present the body is on the floor: {hits:?}"
        );
        assert!(
            shots.iter().all(|s| s.hit != SHOT_BODY),
            "nothing was hit: {shots:?}"
        );
    }

    #[test]
    fn a_reflected_round_reports_two_segments() {
        // The full arc of a caught round, event by event: the shooter's
        // segment ends on the plate at the holder's circle, owned by the
        // shooter and naming the holder, and the return segment starts at
        // that exact point, owned by the holder, and ends in the shooter's
        // body naming the shooter. Two events, one hit, credited to the
        // catcher.
        let mut sim = open_sim(31, 2);
        let mut inputs = HashMap::new();
        let mut shots = Vec::new();
        let mut hits = Vec::new();
        for t in 0..12u32 {
            // A 12 m duel keeps both legs within the 30 m starter range.
            hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [12.0, 0.0], 0.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            inputs.insert(
                1,
                PlayerIn {
                    aim: [-1.0, 0.0],
                    shield: true,
                    ..Default::default()
                },
            );
            step_geometry(&mut sim, &inputs);
            shots.extend(sim.shots.iter().map(|&s| (t, s)));
            hits.extend(sim.hits.iter().copied());
        }
        assert!(sim.bullets.is_empty(), "the round came home");
        assert_eq!(shots.len(), 2, "{shots:?}");
        let (t_out, out) = shots[0];
        let (t_back, back) = shots[1];
        assert_eq!(
            (out.owner, out.weapon, out.hit, out.victim),
            (0, SIDEARM, SHOT_SHIELD, 1)
        );
        assert_eq!((out.cover, out.normal), (SHOT_NONE, [0, 0, 0]));
        let plate = 12.0 - hit_radius(false) - BULLET_R;
        assert!((out.to[0] - plate).abs() < 1e-3, "{:?}", out.to);
        assert!((out.from[0] - 0.2).abs() < 1e-6, "{:?}", out.from);
        assert_eq!(
            (back.owner, back.weapon, back.hit, back.victim),
            (1, SIDEARM, SHOT_BODY, 0)
        );
        assert!(
            same_point(back.from, out.to),
            "{:?} vs {:?}",
            back.from,
            out.to
        );
        let home = hit_radius(false) + BULLET_R;
        assert!((back.to[0] - home).abs() < 1e-3, "{:?}", back.to);
        assert!(t_back > t_out, "{t_out} then {t_back}");
        assert_eq!(hits, vec![(1, 0, 1, false)]);
        assert_eq!(player(&sim, 0).hp, MAX_HP - 1);
        assert_eq!(player(&sim, 1).hp, MAX_HP);
    }

    #[test]
    fn a_rocket_direct_hit_reports_the_body_and_the_blast_together() {
        // A rocket into a body five metres out: the tick it lands carries
        // one `SHOT_BODY` event naming the body and one blast at the very
        // same point, the contact on the hit circle, so the client's
        // impact and the blast are drawn in one place. Nothing follows:
        // the rocket is gone.
        let mut sim = open_sim(1, 2);
        arm(&mut sim.players[0], 7);
        let mut inputs = HashMap::new();
        let mut landed = None;
        for t in 0..30u32 {
            hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [5.0, 0.0], 0.0)]);
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            step_with(&mut sim, &inputs);
            if landed.is_some() {
                assert!(sim.shots.is_empty() && sim.blasts.is_empty(), "tick {t}");
                continue;
            }
            if sim.blasts.is_empty() {
                assert!(sim.shots.is_empty(), "tick {t}: {:?}", sim.shots);
                continue;
            }
            assert_eq!(sim.shots.len(), 1, "tick {t}: {:?}", sim.shots);
            let s = sim.shots[0];
            assert_eq!((s.owner, s.weapon, s.hit, s.victim), (0, 7, SHOT_BODY, 1));
            assert_eq!((s.cover, s.normal), (SHOT_NONE, [0, 0, 0]));
            assert_eq!(sim.blasts.len(), 1);
            assert!(
                same_point(sim.blasts[0].0, s.to),
                "{:?} vs {:?}",
                sim.blasts[0],
                s.to
            );
            assert_eq!(sim.hits, vec![(0, 1, 3, false)]);
            let rr = hit_radius(false) + weapon_stats(7).radius;
            assert!((s.to[0] - (5.0 - rr)).abs() < 1e-3, "{:?}", s.to);
            assert!(sim.bullets.is_empty(), "the rocket is gone");
            landed = Some(t);
        }
        assert_eq!(
            landed,
            Some(1),
            "2.1 m on the launch tick, the body on the next"
        );
    }

    /// The v18 determinism driver's input table: four players on the yard
    /// firing, jumping, crouching, scoping, shielding, swinging and
    /// reloading on a hash of the tick and the id. Shared by the two-sim
    /// test and the v18 pin below so the pin is of exactly this table.
    fn v18_script(tick: u64, id: u8) -> PlayerIn {
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
    }

    /// The driver's weapon rotation: every 75 ticks everyone is handed the
    /// next row of the table, so every weapon flies inside 600 ticks.
    fn v18_grants(sim: &mut Sim, tick: u64) {
        if tick.is_multiple_of(75) {
            for (k, p) in sim.players.iter_mut().enumerate() {
                grant(p, 1 + ((tick / 75 + k as u64) % 7) as u8);
            }
        }
    }

    /// FNV-1a 64 over little-endian words. Spelled out rather than taken
    /// from `std::hash`, whose `DefaultHasher` is not promised to be the
    /// same function from one toolchain to the next, so that a number
    /// pinned in a test means the same thing next year.
    fn fold(h: &mut u64, words: impl IntoIterator<Item = u64>) {
        for w in words {
            for b in w.to_le_bytes() {
                *h ^= u64::from(b);
                *h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
    }

    /// Bit equality of two points, so an event's end and a round's start
    /// are compared exactly and not up to a float tolerance.
    fn same_point(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits())
    }

    /// Every field of a shot event as bits.
    fn shot_bits(s: &ShotEvent) -> Vec<u64> {
        let mut v = vec![u64::from(s.owner), u64::from(s.weapon)];
        v.extend(s.from.iter().chain(&s.to).map(|x| u64::from(x.to_bits())));
        v.extend([u64::from(s.hit), u64::from(s.cover), u64::from(s.victim)]);
        v.extend(
            s.normal
                .iter()
                .map(|&n| u64::from(n.unsigned_abs()) | (u64::from(n < 0) << 8)),
        );
        v
    }

    /// Folds one tick of a sim into `h`: every player and every round as
    /// bits, then the tick's kills, hits, blasts and loot payouts, then
    /// (since v20) its shot events. The v18 fields in the v18 order, with
    /// the shots appended last so the prefix of the fold is still the v18
    /// fold.
    fn fold_tick(h: &mut u64, sim: &Sim) {
        for p in &sim.players {
            fold(h, player_bits(p));
        }
        for b in &sim.bullets {
            fold(h, bullet_bits(b));
        }
        fold(
            h,
            sim.events
                .iter()
                .flat_map(|&(a, b)| [u64::from(a), u64::from(b)]),
        );
        fold(
            h,
            sim.hits
                .iter()
                .flat_map(|&(a, b, c, d)| [u64::from(a), u64::from(b), u64::from(c), u64::from(d)]),
        );
        fold(
            h,
            sim.blasts.iter().flat_map(|&(p, w)| {
                [
                    u64::from(p[0].to_bits()),
                    u64::from(p[1].to_bits()),
                    u64::from(p[2].to_bits()),
                    u64::from(w),
                ]
            }),
        );
        fold(
            h,
            sim.loot_events
                .iter()
                .flat_map(|&(a, b, c)| [u64::from(a), u64::from(b), u64::from(c)]),
        );
        for s in &sim.shots {
            fold(h, shot_bits(s));
        }
    }

    /// FNV-1a's offset basis: where every fold starts.
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

    /// `fold_tick` of the driver after ticks 99, 199, ... 599, computed
    /// from the protocol-20 four-metre walking tree on the Windows workstation,
    /// after independently replaying two simulations and checking every
    /// player's ADS fraction, recoverable bloom and effective cone as well
    /// as movement, bullets and events. The protocol-19 handling pin changed
    /// deliberately because the requested walking speed is now 4 m/s;
    /// the jump speed and handling rules remain unchanged. This is
    /// a regression fingerprint, not identity with old gameplay.
    /// The script and the launch go through `cos`, `sin` and
    /// `tan`, which are the platform's, so a toolchain on another libm
    /// could legitimately differ in the last bit; the tests have only ever
    /// run here, and if that changes the pin is regenerated the same way,
    /// from the tree that is being pinned.
    const FINGERPRINT_CHECKPOINTS: [u64; 6] = [
        0x55b7_e8cb_e70f_813b,
        0xdbe5_d977_bf15_e020,
        0xab42_b94a_f87c_740a,
        0xde2d_5013_d138_b096,
        0x13d4_88f8_da66_bdbd,
        0x9be6_5b95_a1b8_b22d,
    ];
    /// The script's kills over the 600 ticks, and every player's score
    /// at the end, from the same run. v18's script landed one kill, a
    /// self-kill by a rocket's own splash; at real speeds the same
    /// trigger pulls land none, because the rocket is well clear of its
    /// owner before anything stops it.
    const FINGERPRINT_KILLS: usize = 0;
    const FINGERPRINT_SCORES: [u32; 4] = [0, 0, 0, 0];

    #[test]
    fn free_for_all_handling_fingerprint_and_mode_equivalence() {
        // Free for all must match the handling pin until a round ends:
        // the same players, rounds, kills, hits, blasts, payouts and shot
        // events bit for bit, tick for tick. The frag limit is nowhere
        // near in 600 ticks, which is what "until the limit" means here:
        // the round never ends, so the v19 round state stays inert the
        // whole way. King of the hill on the same script is free for all
        // with a different scoreboard: identical in every bit but
        // `score`, whose frags move to `frags`. Driven alongside, because
        // that claim is what lets the hill pass sit inside `step` without
        // a pin of its own.
        let level = Level::freight_yard();
        let mut ffa = Sim::from_level(&level, 7, GameMode::Ffa);
        let mut hill = Sim::from_level(&level, 7, GameMode::Hill);
        for id in 0..4 {
            ffa.add_player(id);
            hill.add_player(id);
        }
        let mut h = FNV_OFFSET;
        let mut kills = 0;
        let mut seen = Vec::new();
        for tick in 0..600u64 {
            v18_grants(&mut ffa, tick);
            v18_grants(&mut hill, tick);
            ffa.step(&|id| v18_script(tick, id));
            hill.step(&|id| v18_script(tick, id));
            fold_tick(&mut h, &ffa);
            if tick % 100 == 99 {
                seen.push(h);
            }
            kills += ffa.events.len();
            // The v19 state a free-for-all round never touches.
            assert!(ffa.round_over.is_empty(), "tick {tick}");
            assert_eq!(ffa.round_pause, 0.0, "tick {tick}");
            assert_eq!(ffa.round, 0, "tick {tick}");
            assert_eq!(ffa.hill_holder, HILL_FREE, "tick {tick}");
            assert_eq!(ffa.team_score, [0, 0], "tick {tick}");
            for p in &ffa.players {
                assert_eq!(p.team, 0, "tick {tick} player {}", p.id);
                assert_eq!(p.frags, p.score, "tick {tick} player {}", p.id);
                assert!(p.score < FFA_FRAG_LIMIT, "tick {tick} player {}", p.id);
            }
            // And the hill sim, score masked, is the same sim.
            for (a, b) in ffa.players.iter().zip(&hill.players) {
                let mut masked = b.clone();
                masked.score = a.score;
                assert_eq!(
                    player_bits(a),
                    player_bits(&masked),
                    "tick {tick} player {}",
                    a.id
                );
                assert_eq!(a.score, b.frags, "tick {tick} player {}", a.id);
            }
            assert_eq!(ffa.bullets.len(), hill.bullets.len(), "tick {tick}");
            for (a, b) in ffa.bullets.iter().zip(&hill.bullets) {
                assert_eq!(bullet_bits(a), bullet_bits(b), "tick {tick}");
            }
            assert_eq!(ffa.events, hill.events, "tick {tick}");
            assert_eq!(ffa.hits, hill.hits, "tick {tick}");
            assert_eq!(ffa.blasts, hill.blasts, "tick {tick}");
            assert_eq!(ffa.loot_events, hill.loot_events, "tick {tick}");
            assert_eq!(ffa.shots, hill.shots, "tick {tick}");
        }
        let scores: Vec<u32> = ffa.players.iter().map(|p| p.score).collect();
        assert_eq!(
            seen,
            FINGERPRINT_CHECKPOINTS.to_vec(),
            "the fingerprint diverged in the hundred ticks before the first checkpoint that differs (kills {kills}, scores {scores:?})"
        );
        assert_eq!(h, FINGERPRINT_CHECKPOINTS[5], "the whole run");
        assert_eq!(kills, FINGERPRINT_KILLS, "the script's kills");
        assert_eq!(
            scores,
            FINGERPRINT_SCORES.to_vec(),
            "and who they were credited to"
        );
    }

    #[test]
    fn two_sims_with_the_same_seed_and_inputs_agree_bit_for_bit() {
        // Four players on the yard, 600 ticks of `v18_script`, which fires,
        // jumps, crouches, scopes, shields, swings, reloads and cycles
        // every weapon in the table, compared field by field as bits after
        // every tick. What this pins is that nothing in the step is hidden
        // state: no RNG, no hash-map order, no time.
        let level = Level::freight_yard();
        let mut a = Sim::from_level(&level, 7, GameMode::Ffa);
        let mut b = Sim::from_level(&level, 7, GameMode::Ffa);
        for id in 0..4 {
            a.add_player(id);
            b.add_player(id);
        }
        let mut rounds_seen = [false; 8];
        for tick in 0..600u64 {
            v18_grants(&mut a, tick);
            v18_grants(&mut b, tick);
            a.step(&|id| v18_script(tick, id));
            b.step(&|id| v18_script(tick, id));
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
            assert_eq!(a.shots.len(), b.shots.len(), "tick {tick}");
            for (sa, sb) in a.shots.iter().zip(&b.shots) {
                assert_eq!(shot_bits(sa), shot_bits(sb), "tick {tick}");
                // A round that ended on the tick it left never appears in
                // the list, and at real speeds most do: the event is the
                // record that it flew.
                rounds_seen[usize::from(sa.weapon)] = true;
            }
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
        let mut sim = Sim::from_level(&level, 7, GameMode::Ffa);
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
            let speed = movement_speed(
                pos,
                y,
                vy,
                input.jump,
                input.sprint,
                input.crouch,
                input.shield,
                &sim.obstacles,
            );
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
        let ticks = (9.0 / (MOVE_SPEED * FIXED_DT)).ceil() as u32;
        for _ in 0..ticks {
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

    // ---- v19: modes ----

    /// The seeded arena with no cover and no pads, in `mode`, so a mode
    /// test places every body by hand and nothing else moves the geometry.
    fn mode_sim(seed: u64, n: u8, mode: GameMode) -> Sim {
        let mut sim = Sim::from_level(&Level::from_seed(seed), seed, mode);
        sim.obstacles.clear();
        sim.pads.clear();
        for id in 0..n {
            sim.add_player(id);
        }
        sim
    }

    #[test]
    fn every_mode_survives_serde_and_an_unknown_name_is_refused() {
        for mode in [GameMode::Ffa, GameMode::Tdm, GameMode::Hill] {
            assert_eq!(GameMode::from_name(mode.name()), Some(mode));
        }
        assert_eq!(
            GameMode::from_name(""),
            Some(GameMode::Ffa),
            "an absent name is free for all"
        );
        assert_eq!(GameMode::from_name("ctf"), None);
        assert_eq!(GameMode::from_name("TDM"), None, "names are exact");
        // Every level carries its hill through the codec, and king of the
        // hill on a level with none plays as free for all.
        for level in [
            Level::freight_yard(),
            Level::trench_city(),
            Level::from_seed(3),
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: Level = serde_json::from_str(&json).unwrap();
            assert!(level.hill.is_some(), "every shipped level has a hill");
            assert_eq!(back.hill, level.hill);
            assert_eq!(
                Sim::from_level(&level, 1, GameMode::Hill).mode,
                GameMode::Hill
            );
        }
        let mut bare = Level::from_seed(3);
        bare.hill = None;
        assert_eq!(
            Sim::from_level(&bare, 1, GameMode::Hill).mode,
            GameMode::Ffa,
            "no hill, no king"
        );
        assert_eq!(Sim::new(3).mode, GameMode::Ffa);
    }

    #[test]
    fn teams_are_balanced_on_join() {
        let level = Level::freight_yard();
        let mut sim = Sim::from_level(&level, 5, GameMode::Tdm);
        for id in 0..8 {
            sim.add_player(id);
        }
        let count = |sim: &Sim, t: u8| sim.players.iter().filter(|p| p.team == t).count();
        assert_eq!((count(&sim, 0), count(&sim, 1)), (4, 4));
        // Every join was a tie broken by parity, so the sides alternate.
        for p in &sim.players {
            assert_eq!(p.team, p.id % 2, "player {}", p.id);
        }
        // A leaver from blue, and the ninth joiner lands on blue.
        sim.remove_player(2);
        sim.add_player(9);
        assert_eq!(player(&sim, 9).team, 0, "the smaller team");
        assert_eq!((count(&sim, 0), count(&sim, 1)), (4, 4));
        // Outside team deathmatch everyone is team 0, whatever the id.
        let mut ffa = Sim::from_level(&level, 5, GameMode::Ffa);
        for id in 0..8 {
            ffa.add_player(id);
        }
        assert!(ffa.players.iter().all(|p| p.team == 0));
        assert_eq!(ffa.team_score, [0, 0]);
    }

    #[test]
    fn teammates_spawn_on_their_side() {
        let level = Level::freight_yard();
        assert_eq!(level.spawns_for(0).len(), 4, "the north backlot");
        assert_eq!(level.spawns_for(1).len(), 4, "the south backlot");
        assert!(level.spawns_for(0).iter().all(|s| s[1] > 0.0));
        assert!(level.spawns_for(1).iter().all(|s| s[1] < 0.0));
        // A level whose spawns all sit on one side hands both teams the
        // whole list rather than nothing.
        let mut lop = level.clone();
        lop.spawns.retain(|s| s[1] > 0.0);
        assert_eq!(lop.spawns_for(1), lop.spawns);

        let mut sim = Sim::from_level(&level, 5, GameMode::Tdm);
        for id in 0..4 {
            sim.add_player(id);
        }
        for p in &sim.players {
            assert_eq!(p.pos[1] > 0.0, p.team == 0, "first spawn of {}", p.id);
        }
        // Two hundred respawns each: killed by hand, back on the next tick,
        // always on their own side.
        let inputs = HashMap::new();
        let mut respawns = 0;
        for _ in 0..200 {
            for p in &mut sim.players {
                p.alive = false;
                p.hp = 0;
                p.respawn_in = 0.0;
            }
            step_with(&mut sim, &inputs);
            for p in &sim.players {
                assert!(p.alive);
                assert_eq!(p.pos[1] > 0.0, p.team == 0, "respawn of {}", p.id);
                respawns += 1;
            }
        }
        assert_eq!(respawns, 800);
        // And free for all still walks the whole list, as v18 did.
        let mut ffa = Sim::from_level(&level, 5, GameMode::Ffa);
        ffa.add_player(0);
        ffa.add_player(1);
        assert_eq!(ffa.players[0].pos, level.spawn(0));
        assert_eq!(ffa.players[1].pos, level.spawn(1));
    }

    #[test]
    fn eight_players_get_distinct_spawns_on_join_respawn_rejoin_and_round_reset() {
        let unique = |sim: &Sim| {
            assert_eq!(sim.players.len(), 8);
            for (i, a) in sim.players.iter().enumerate() {
                assert!(a.alive);
                if sim.mode == GameMode::Tdm {
                    assert_eq!(a.pos[1] > 0.0, a.team == 0);
                }
                for b in &sim.players[i + 1..] {
                    let distance2 = (a.pos[0] - b.pos[0]).powi(2) + (a.pos[1] - b.pos[1]).powi(2);
                    assert!(
                        distance2 > (2.0 * PLAYER_R).powi(2),
                        "players {} and {} share {:?}",
                        a.id,
                        b.id,
                        a.pos
                    );
                }
            }
        };
        for level in [Level::freight_yard(), Level::trench_city(), Level::harbor()] {
            for mode in [GameMode::Ffa, GameMode::Tdm, GameMode::Hill] {
                let mut sim = Sim::from_level(&level, 22, mode);
                for id in 0..8 {
                    sim.add_player(id);
                }
                unique(&sim);
                sim.remove_player(2);
                sim.add_player(2);
                unique(&sim);
                // All dying together reserves each new spot before the next
                // body respawns, even when global ids alias a team's four slots.
                for _ in 0..4 {
                    for p in &mut sim.players {
                        p.alive = false;
                        p.respawn_in = 0.0;
                    }
                    sim.step(&|_| PlayerIn::default());
                    unique(&sim);
                }
                for _ in 0..4 {
                    sim.restart_round();
                    unique(&sim);
                }
                // A single respawn while seven living players occupy pockets.
                sim.players[0].alive = false;
                sim.players[0].respawn_in = 0.0;
                sim.step(&|_| PlayerIn::default());
                unique(&sim);
            }
        }
    }

    #[test]
    fn level_bounds_are_finite_and_legacy_movement_is_the_24_metre_wrapper() {
        for bad in [f32::NAN, f32::INFINITY, -1.0, 0.0, 512.0] {
            let mut level = Level::from_seed(0);
            level.arena_half = bad;
            assert_eq!(
                Sim::from_level(&level, 0, GameMode::Ffa).arena_half,
                ARENA_HALF
            );
            assert!(blocked_in([25.0, 0.0], 0.0, PLAYER_R, &[], bad));
        }
        for pos in [[0.0, 0.0], [23.0, 1.0], [-23.0, -5.0]] {
            for mv in [[0.0, 1.0], [1.0, 1.0], [-1.0, 0.0]] {
                assert_eq!(
                    move_circle(pos, 0.0, mv, 4.0, FIXED_DT, &[]),
                    move_circle_in(pos, 0.0, mv, 4.0, FIXED_DT, &[], ARENA_HALF)
                );
            }
        }
    }

    #[test]
    fn harbor_eight_player_replay_matches_every_tick() {
        let level = Level::harbor();
        let mut a = Sim::from_level(&level, 27, GameMode::Ffa);
        let mut b = Sim::from_level(&level, 27, GameMode::Ffa);
        for id in 0..8 {
            a.add_player(id);
            b.add_player(id);
        }
        let (mut ah, mut bh) = (FNV_OFFSET, FNV_OFFSET);
        for tick in 0..900 {
            v18_grants(&mut a, tick);
            v18_grants(&mut b, tick);
            a.step(&|id| v18_script(tick, id));
            b.step(&|id| v18_script(tick, id));
            fold_tick(&mut ah, &a);
            fold_tick(&mut bh, &b);
            assert_eq!(ah, bh, "tick {tick}");
            assert_eq!(a.arena_half, 48.0);
            for p in &a.players {
                assert!(
                    p.pos
                        .iter()
                        .all(|v| v.abs() <= a.arena_half - PLAYER_R + 1e-4)
                );
            }
        }
    }

    #[test]
    fn a_teammate_is_never_hit() {
        // 0 and 2 are blue, 1 is red. A round from 0 through 2 hits 1.
        let mut sim = mode_sim(1, 3, GameMode::Tdm);
        assert_eq!(
            (
                player(&sim, 0).team,
                player(&sim, 1).team,
                player(&sim, 2).team
            ),
            (0, 1, 0)
        );
        let spots = [
            (0, [0.0, 0.0], 0.0),
            (2, [2.0, 0.0], 0.0),
            (1, [4.0, 0.0], 0.0),
        ];
        let mut inputs = HashMap::new();
        let mut hits = Vec::new();
        for t in 0..15u32 {
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            hold(&mut sim, &spots);
            step_with(&mut sim, &inputs);
            hits.extend(sim.hits.iter().copied());
        }
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!((hits[0].0, hits[0].1), (0, 1), "the enemy behind");
        assert_eq!(player(&sim, 2).hp, MAX_HP, "the teammate in front");

        // A swing at a teammate is a swing at nothing, but it still costs
        // the cooldown.
        let mut sim = mode_sim(1, 3, GameMode::Tdm);
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [1.0, 0.0],
                melee: true,
                ..Default::default()
            },
        );
        hold(
            &mut sim,
            &[
                (0, [0.0, 0.0], 0.0),
                (2, [1.0, 0.0], 0.0),
                (1, [12.0, 0.0], 0.0),
            ],
        );
        step_with(&mut sim, &inputs);
        assert!(sim.hits.is_empty(), "{:?}", sim.hits);
        assert_eq!(player(&sim, 2).hp, MAX_HP);
        assert!(player(&sim, 0).melee_cd > 0.0, "the swing happened");

        // A rocket at the owner's own feet: the teammate beside is spared,
        // the enemy on the other side and the owner are not.
        let mut sim = mode_sim(1, 3, GameMode::Tdm);
        arm(&mut sim.players[0], 7);
        let mut inputs = HashMap::new();
        inputs.insert(0, shot(0, [1.0, 0.0], -MAX_PITCH));
        hold(
            &mut sim,
            &[
                (0, [0.0, 0.0], 0.0),
                (2, [0.9, 0.0], 0.0),
                (1, [-0.9, 0.0], 0.0),
            ],
        );
        step_with(&mut sim, &inputs);
        assert_eq!(sim.blasts.len(), 1, "went off on the first tick");
        let victims: Vec<u8> = sim.hits.iter().map(|h| h.1).collect();
        assert!(
            victims.contains(&0),
            "the owner eats the splash: {victims:?}"
        );
        assert!(victims.contains(&1), "the enemy beside it too: {victims:?}");
        assert!(!victims.contains(&2), "the teammate is spared: {victims:?}");
    }

    #[test]
    fn a_teammates_shield_does_not_catch_a_friendly_round() {
        // Blue 2 stands between blue 0 and red 1 with the plate up and
        // facing 0: a friendly round passes through the plate as if the
        // body were not there, hits the enemy, and is never reflected.
        let mut sim = mode_sim(1, 3, GameMode::Tdm);
        let spots = [
            (0, [0.0, 0.0], 0.0),
            (2, [2.0, 0.0], 0.0),
            (1, [4.0, 0.0], 0.0),
        ];
        let mut inputs = HashMap::new();
        inputs.insert(
            2,
            PlayerIn {
                aim: [-1.0, 0.0],
                shield: true,
                ..Default::default()
            },
        );
        let mut hits = Vec::new();
        for t in 0..15u32 {
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            hold(&mut sim, &spots);
            step_with(&mut sim, &inputs);
            hits.extend(sim.hits.iter().copied());
            assert!(
                sim.bullets.iter().all(|b| b.owner == 0),
                "tick {t}: a reflected round would belong to the catcher"
            );
        }
        assert!(player(&sim, 2).shield, "the plate was up");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!((hits[0].0, hits[0].1), (0, 1));
        assert_eq!(player(&sim, 0).hp, MAX_HP, "nothing came back");
    }

    /// One sidearm round from 0 at 1, both held in place, until it lands
    /// or `ticks` run out. Returns every kill event seen.
    fn kill_shot(sim: &mut Sim, ticks: u32) -> Vec<(u8, u8)> {
        let spots = [(0, [0.0, 0.0], 0.0), (1, [3.0, 0.0], 0.0)];
        let mut inputs = HashMap::new();
        let mut events = Vec::new();
        for t in 0..ticks {
            inputs.insert(0, shot(t, [1.0, 0.0], 0.0));
            hold(sim, &spots);
            step_with(sim, &inputs);
            events.extend(sim.events.iter().copied());
            if !events.is_empty() {
                break;
            }
        }
        events
    }

    #[test]
    fn team_frags_score_for_the_team_and_a_self_kill_scores_for_nobody() {
        let mut sim = mode_sim(1, 2, GameMode::Tdm);
        sim.players[1].hp = 1;
        assert_eq!(kill_shot(&mut sim, 20), vec![(0, 1)]);
        assert_eq!(sim.team_score, [1, 0], "blue's frag is blue's point");
        assert_eq!(player(&sim, 0).score, 1);
        assert_eq!(player(&sim, 0).frags, 1);
        assert_eq!(player(&sim, 1).death_count, 1);

        // A rocket into the floor at the owner's own feet with one point
        // left: a death, a self-kill event, and no score anywhere.
        let mut sim = mode_sim(1, 2, GameMode::Tdm);
        arm(&mut sim.players[0], 7);
        sim.players[0].hp = 1;
        let mut inputs = HashMap::new();
        inputs.insert(0, shot(0, [1.0, 0.0], -MAX_PITCH));
        hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [15.0, 0.0], 0.0)]);
        step_with(&mut sim, &inputs);
        assert_eq!(sim.events, vec![(0, 0)]);
        assert_eq!(sim.team_score, [0, 0]);
        assert_eq!(player(&sim, 0).score, 0);
        assert_eq!(player(&sim, 0).frags, 0);
        assert_eq!(player(&sim, 0).death_count, 1);
    }

    #[test]
    fn alone_on_the_hill_earns_a_point_a_second() {
        // The seeded hill is the open centre at floor level. Player 0
        // stands on it from tick 1 and starts earning from tick 2 (the tick
        // that makes it the holder pays nothing), so at tick 1800 it has
        // banked 1799 ticks, just under thirty seconds, and at 1830 just
        // over.
        let mut sim = mode_sim(3, 2, GameMode::Hill);
        let spots = [(0, [0.0, 0.0], 0.0), (1, [15.0, 15.0], 0.0)];
        let inputs = HashMap::new();
        let mut round_overs = Vec::new();
        for tick in 1..=61 * 60u32 {
            hold(&mut sim, &spots);
            step_with(&mut sim, &inputs);
            round_overs.extend(sim.round_over.iter().copied());
            assert_eq!(sim.hill_holder, 0, "tick {tick}");
            match tick {
                1800 => assert_eq!(player(&sim, 0).score, 29, "just under thirty seconds"),
                1830 => assert_eq!(player(&sim, 0).score, 30, "just over"),
                3500 => assert!(sim.round_pause == 0.0, "still running"),
                _ => {}
            }
        }
        assert_eq!(
            player(&sim, 0).score,
            HILL_LIMIT,
            "sixty points in sixty-one seconds"
        );
        assert_eq!(player(&sim, 1).score, 0);
        assert_eq!(player(&sim, 0).frags, 0, "hill points are not frags");
        assert_eq!(
            round_overs,
            vec![(0, false)],
            "the sixtieth point is the round"
        );
        assert!(sim.round_pause > 0.0 && sim.round_pause < ROUND_PAUSE_SECS);
    }

    #[test]
    fn a_contested_hill_pays_nobody() {
        let mut sim = mode_sim(3, 2, GameMode::Hill);
        let inputs = HashMap::new();
        for _ in 0..5 * 60 {
            hold(&mut sim, &[(0, [0.5, 0.0], 0.0), (1, [-0.5, 0.0], 0.0)]);
            step_with(&mut sim, &inputs);
            assert_eq!(sim.hill_holder, HILL_CONTESTED);
        }
        assert_eq!(player(&sim, 0).score, 0);
        assert_eq!(player(&sim, 1).score, 0);
        // The moment 1 steps off, 0 holds it and the clock starts from
        // zero: a point after a whole second, not before.
        for tick in 1..=61u32 {
            hold(&mut sim, &[(0, [0.5, 0.0], 0.0), (1, [15.0, 0.0], 0.0)]);
            step_with(&mut sim, &inputs);
            assert_eq!(sim.hill_holder, 0);
            if tick < 61 {
                assert_eq!(player(&sim, 0).score, 0, "tick {tick}");
            }
        }
        for _ in 0..10 {
            hold(&mut sim, &[(0, [0.5, 0.0], 0.0), (1, [15.0, 0.0], 0.0)]);
            step_with(&mut sim, &inputs);
        }
        assert_eq!(player(&sim, 0).score, 1);
        // A dead body on the hill does not hold it.
        sim.players[0].alive = false;
        sim.players[0].respawn_in = RESPAWN_SECS;
        step_with(&mut sim, &inputs);
        assert_eq!(sim.hill_holder, HILL_FREE);
    }

    #[test]
    fn stepping_off_the_hill_resets_the_second() {
        let mut sim = mode_sim(3, 1, GameMode::Hill);
        let inputs = HashMap::new();
        let on = [(0, [0.0, 0.0], 0.0)];
        let off = [(0, [10.0, 0.0], 0.0)];
        for _ in 0..54 {
            hold(&mut sim, &on);
            step_with(&mut sim, &inputs);
        }
        hold(&mut sim, &off);
        step_with(&mut sim, &inputs);
        assert_eq!(sim.hill_holder, HILL_FREE);
        for _ in 0..54 {
            hold(&mut sim, &on);
            step_with(&mut sim, &inputs);
        }
        assert_eq!(sim.hill_holder, 0);
        assert_eq!(player(&sim, 0).score, 0, "two part seconds are no second");
        for _ in 0..10 {
            hold(&mut sim, &on);
            step_with(&mut sim, &inputs);
        }
        assert_eq!(player(&sim, 0).score, 1, "and the second one completes");
    }

    #[test]
    fn the_hill_is_on_the_dock_and_the_plinth() {
        let yard = Level::freight_yard();
        let dock = yard.hill.unwrap();
        assert!(dock.stands_on([0.0, 0.0], 1.2), "on the dock");
        assert!(dock.stands_on([3.9, -1.9], 1.2), "at the dock's corner");
        assert!(
            !dock.stands_on([0.0, 0.0], 0.0),
            "at the floor under the king block"
        );
        assert!(!dock.stands_on([4.5, 0.0], 1.2), "beside the dock");
        let city = Level::trench_city();
        let plinth = city.hill.unwrap();
        assert!(plinth.stands_on([0.0, 0.0], 2.2), "on the plinth");
        assert!(
            !plinth.stands_on([0.0, 0.0], 1.1),
            "at the sandbags' height"
        );
        assert!(!plinth.stands_on([2.0, 0.0], 2.2), "beside the plinth");

        // And the sim agrees, with the level's own cover under the feet:
        // a body the dock supports holds the hill, the same body at a
        // spawn does not.
        for (level, top) in [(yard, 1.2), (city, 2.2)] {
            let mut sim = Sim::from_level(&level, 5, GameMode::Hill);
            sim.add_player(0);
            sim.add_player(1);
            let inputs = HashMap::new();
            let away = level.spawn(1);
            for _ in 0..3 {
                hold(&mut sim, &[(0, [0.0, 0.0], top), (1, away, 0.0)]);
                step_with(&mut sim, &inputs);
            }
            assert_eq!(sim.hill_holder, 0);
            assert_eq!(player(&sim, 0).y, top, "the box holds the feet at the top");
            for _ in 0..3 {
                hold(&mut sim, &[(0, level.spawn(0), 0.0), (1, away, 0.0)]);
                step_with(&mut sim, &inputs);
            }
            assert_eq!(sim.hill_holder, HILL_FREE);
        }
    }

    #[test]
    fn a_round_ends_at_the_frag_limit_and_restarts_after_the_pause() {
        // Free for all on one hanging block. Player 0 is one frag short and
        // the block is spent; the twentieth frag ends the round and, ten
        // seconds later, the next one starts from nothing.
        let mut sim = block_sim(1, vec![block(8.0, 8.0, 2.3)]);
        sim.add_player(0);
        sim.add_player(1);
        sim.players[0].score = FFA_FRAG_LIMIT - 1;
        sim.players[0].frags = FFA_FRAG_LIMIT - 1;
        sim.players[1].hp = 1;
        sim.loot[0].respawn_t = 7.0;
        arm(&mut sim.players[0], 3);
        assert_eq!(kill_shot(&mut sim, 20), vec![(0, 1)]);
        assert_eq!(sim.round_over, vec![(0, false)], "the winner, once");
        assert_eq!(sim.round_pause, ROUND_PAUSE_SECS);
        assert_eq!(player(&sim, 0).score, FFA_FRAG_LIMIT);
        assert_eq!(sim.round, 0);

        // The pause: everyone keeps playing (player 0 keeps firing, and
        // every round fired is parked in the air with its life held so
        // the restart has something to clear: at 715 m/s an AK round
        // would otherwise be gone within two ticks), nothing else ends.
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim: [0.0, 1.0],
                fire: true,
                ..Default::default()
            },
        );
        let mut ticks = 0u32;
        let mut round_overs = 0;
        while sim.round == 0 {
            hold(&mut sim, &[(0, [0.0, 0.0], 0.0), (1, [3.0, 0.0], 0.0)]);
            step_with(&mut sim, &inputs);
            for b in &mut sim.bullets {
                b.ttl = 4.0;
                b.pos = [0.0, 5.0];
                b.vel = [0.0, 0.0];
            }
            ticks += 1;
            round_overs += sim.round_over.len();
            assert!(ticks <= 602, "the pause never ended");
            if ticks == 599 {
                assert!(sim.round_pause > 0.0);
                assert!(!sim.bullets.is_empty(), "rounds fly during the pause");
            }
        }
        assert_eq!(round_overs, 0, "a round ends once");
        assert!(
            (600..=601).contains(&ticks),
            "the pause is ROUND_PAUSE_SECS to the tick: {ticks}"
        );
        assert_eq!(sim.round, 1);
        assert_eq!(sim.round_pause, 0.0);
        assert!(sim.bullets.is_empty(), "every round cleared");
        assert_eq!(sim.loot[0].respawn_t, 0.0, "the block is armed again");
        assert_eq!(sim.hill_holder, HILL_FREE);
        for p in &sim.players {
            assert_eq!(
                (p.score, p.frags, p.death_count),
                (0, 0, 0),
                "player {}",
                p.id
            );
            assert!(p.alive, "player {}", p.id);
            assert_eq!(
                (p.hp, p.weapon, p.ammo),
                (MAX_HP, SIDEARM, weapon_stats(SIDEARM).mag)
            );
            assert_eq!(p.reserve, RESERVE_INFINITE);
            assert_eq!(p.y, 0.0, "back on the floor at a spawn");
        }
        // The spawns are the seeded ring's, walked by the respawn rule.
        assert!(
            sim.players.iter().all(|p| (0..64).any(|slot| {
                let s = spawn_point(slot);
                p.pos[0].to_bits() == s[0].to_bits() && p.pos[1].to_bits() == s[1].to_bits()
            })),
            "everyone stands on a spawn point"
        );
    }

    #[test]
    fn nothing_scores_during_the_pause() {
        // A frag during the pause is a frag and a death, never a point.
        let mut sim = mode_sim(1, 2, GameMode::Tdm);
        sim.round_pause = 5.0;
        sim.players[1].hp = 1;
        assert_eq!(kill_shot(&mut sim, 20), vec![(0, 1)]);
        assert_eq!(player(&sim, 0).frags, 1);
        assert_eq!(player(&sim, 0).score, 0);
        assert_eq!(sim.team_score, [0, 0]);
        assert_eq!(player(&sim, 1).death_count, 1);
        assert!(sim.round_over.is_empty(), "{:?}", sim.round_over);
        // Alone on the hill during the pause earns nothing either.
        let mut sim = mode_sim(3, 1, GameMode::Hill);
        sim.round_pause = 5.0;
        let inputs = HashMap::new();
        for _ in 0..120 {
            hold(&mut sim, &[(0, [0.0, 0.0], 0.0)]);
            step_with(&mut sim, &inputs);
        }
        assert_eq!(sim.hill_holder, 0, "held, but not paid");
        assert_eq!(player(&sim, 0).score, 0);
        assert!(
            sim.round_pause > 0.0 && sim.round_pause < 5.0,
            "the pause ran"
        );
    }
}
