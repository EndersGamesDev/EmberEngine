# Arena v18 — "Freight Yard": seven guns, loot blocks, and the feel pass

The weapons update. The five new weapon archives in `assets/` become held guns with their own bullets; a Mario-style loot block hands out a random one to whoever head-butts it from below; every gun gets its own recoil, sound, tracer and rumble; the engine learns gamepads and force feedback; and a second authored map, Freight Yard, is built for it from the v13 prop set.

Written before any of it was built, in the shape of `docs/plans/arena-v13-trench-city.md`, as the coordination surface for the work packages in §9. It was synthesised from three independent designs (feel-first, simulation-rigour-first, level-first) and two judges' verdicts over them; every constraint below was verified in the tree on 2026-09-03, and every number is the number that ships unless a test proves it wrong. Where the judges disagreed the choice is stated with its reason.

## 1. What is being asked, in this engine's terms

| Ask | What it means here |
|---|---|
| "in the asset folder you have weapon assets now" | `assets/ak-47.zip`, `pp-19-01-vityaz.zip`, `rpg7.zip`, `sci-fi-sniper-rifle.zip`, `m4-carbine-rifle.zip`, plus the v15 revolver already converted under `assets/revolver/`. They go through Path D of `docs/asset-pipeline.md` (a static prop in parts, hand-wired base colour, measured frame) into the one viewmodel GLB, as `w_*` nodes. Two of them (Vityaz, sniper) are multi-material and force a new step: a **Cycles atlas bake** into one picture, because the renderer samples one base-colour texture per mesh (§8). The M4's mesh is inside a RAR5 archive with compressed entries and this workstation has no extractor; it ships as a table row with no mesh (§8.5). |
| "create a map" | A second authored `Level`, **Freight Yard** (`MAP_FREIGHT_YARD = "freight-yard"`), chosen per lobby by a new `CreateLobby.map` field, built entirely from the v13 props plus the loot block (§4). |
| "a plan to code a own unique playstyle" | This document. The playstyle is §1.1: every gun is a loan you take by bonking a block, and every gun is a different bullet. |
| "feedback haptiks" | A feel pass on the client (per-weapon recoil curves, camera kick and shake, ten new synthesised sounds, hit/kill/bonk/blast cues) and a haptics channel through the engine's platform layer: gamepad rumble on native (gilrs force feedback) and on the web (the Gamepad API's `vibrationActuator`), plus gamepad input on the same intents the keyboard has. The renderer learns nothing (§6). |
| "bullet behaviour" | Per-weapon ballistics in the shared sim: speed, range, a deterministic spread cone with bloom, gravity on the revolver's slug and the rocket, a piercing sniper round, and a rocket that detonates on whatever it touches with a line-of-sight splash that hurts the shooter too (§3). |
| "a lootbox where jumping from below like in super mario on it gives you a random new weapon" | `Cover::Loot`: a 1 m box hung above head height. The sim's ceiling clamp already stops a jumping head at a box's bottom; it now reports which box it was, and the server hands out a random weapon from a stateless hash of (level seed, tick, player). The block bumps, the gun pops out, the block goes dark for 18 s (§5). |

### 1.1 The playstyle

**Every gun is a different hand, and you get your hands by head-butting boxes.** You spawn with the sidearm (the KSVR bullpup already in the viewmodel: today's pistol, bit for bit, with an infinite reserve). Everything better hangs in the air as a `?` block, and the only way to hold it is to jump so your head hits the block from below. The bonk is felt instantly (the client already predicts `step_vertical`, so the bump, the boing and the rumble tick play before the server says a word) and the reward follows one round trip later: the gun pops out of the block spinning and is in your hands with a full magazine and a finite reserve. From then on the gun tells you what it is before you read the HUD: the Vityaz buzzes the weak motor and hoses a cone that never settles; the AK hammers the strong motor and climbs; the revolver flips the muzzle, hits twice as hard and drops its slug so you learn to lead up; the sniper cracks, scopes to a line and goes through the first body it meets; the RPG lobs a rocket you can see coming that blows up on anything with a 3 m splash you can hurt yourself with. Ammo is the clock: a looted gun carries one reserve and no pickup refills it; when the last round is gone the gun is gone and the sidearm is back. Death drops you to the sidearm too. The loop is spawn → hunt a block → spend a hand → hunt again, and the map puts the blocks where the fights already are.

Trench City loses its four pads: a `?` block hangs at the mouth of each tunnel instead (a `Cover::Loot` box at `[-0.5, 9.3]..[0.5, 10.3]`, base 2.30, in `TRENCH_NORTH`, so the quarter-turn scheme makes four), and the block is the one reward rule in the game on both maps. The first cut kept the pads and had them roll the loot table; the first lobby created on Trench City showed why that was wrong (v13 pads and no Mario mechanic on half the game), and replacing them is the second bump of this release (§2.2). The seeded arena keeps its pads, and `pads_hand_out_the_same_loot` keeps testing them there.

## 2. Why this is a protocol bump (v13 → v14)

Apply the rule from `CLAUDE.md`: `serde(default)` makes a field decode; the test is what an OLD peer *does* when the field is absent or means something else.

The field that decides it did not change shape. `PState.weapon` carried 1..3 and now carries an id 1..7. A v13 client receiving `weapon: 7` calls `weapon_stats(7)`, lands in the pistol arm, draws the KSVR and prints an eight-round magazine for a rocket launcher; every number on its HUD is a lie, and the round it then sees fly is a rocket it draws as a blue streak and never hears explode. The rest compounds it: `CreateLobby.map` defaults to Freight Yard, and a v13 client that never sent it predicts its movement against Trench City while the server resolves it against a yard it has never heard of (the v13 bump's own failure mode); `Input.ads` is dropped by a v13 server, so a v18 sniper scopes to a line and gets a hip-fire cone; `State.loot`, `S2C::Loot`, `S2C::Hit` and `S2C::Blast` are additive on their own (an unknown variant is dropped by the net layer, an absent list reads as empty) and would not have bumped alone. So `PROTO_VERSION` goes to 14, frozen pages v13–v17 go list-only against a v14 server exactly as at every earlier bump, and the lobby browser is unaffected (`proto: 0` against the ungated `ListLobbies`).

One rule is kept so the next map is not a bump: the map still travels by name. A peer that knows the name builds it; one that does not is stopped by the gate here, not by a level it cannot decode. Deliberately not bump triggers, so the next one is cheaper: `LobbyInfo.map` (listing only), the three cosmetic events, and a later addition of the M4 to the loot pool (its stats already ship; the client draws a fallback mesh for any id whose node is missing).

### 2.1 Every wire change

| Message / field | Change | serde | What an old (v13) peer does |
|---|---|---|---|
| `C2S::CreateLobby.map: String` | new | `#[serde(default)]`; empty means `MAP_FREIGHT_YARD`; any name that is neither map is answered `Error("unknown map")`, never silently seeded | creates a yard without knowing, then predicts against the wrong cover: a different game |
| `C2S::Input.ads: bool` | new, held (RMB or LT) | `#[serde(default)]` | a v13 client never tightens its cone; a v13 server drops a v18 client's scope |
| `LobbyInfo.map: String` | new | `#[serde(default)]` | listing only; the v17 page does not render it |
| `PState.weapon` | an id 1..7, not a level 1..3 | unchanged field | reads every id above 3 as the pistol: **this alone is the bump** |
| `PState.reserve: u8` | new | `#[serde(default)]` | display only |
| `BState.weapon: u8` | new; 0 reads as the sidearm | `#[serde(default)]` | draws a rocket as a blue tracer |
| `S2C::State.loot: Vec<bool>` | new, index-aligned with the level's `Cover::Loot` obstacles in obstacle order, like `pads` | `#[serde(default)]` | draws no blocks (it built the seeded arena anyway) |
| `S2C::Hit { shooter, victim, dmg, head }` | new variant, from `Sim.hits` | — | dropped by `NetChan::poll` (unknown tag) |
| `S2C::Blast { x, y, z, owner }` | new variant, from `Sim.blasts` | — | dropped |
| `S2C::Loot { player, block, weapon }` | new variant, from `Sim.loot_events` | — | dropped; the weapon still arrives in the next `State` |
| `S2C::Kill` with `killer == victim` | unchanged shape | prints "X fragged X"; v18 prints "you blew yourself up" |
| `S2C::GameJoined.map` | carries `"freight-yard"` or `"trench-city"` | unchanged | `Level::named` falls back to the seeded arena for the unknown name |
| `Cover::Loot` | new enum variant, appended after `Plinth` | on the wire only inside a level JSON, which never travels; the editor round trip covers it | — |
| `step_vertical -> VStep`, `Sim::from_level(&Level, u64)` | shared-code signatures | not on the wire; the compiler forces both peers | — |

Tests in `proto.rs`: `json_roundtrip` grows the new fields; `a_v13_create_lobby_names_the_freight_yard` (`{"t":"create_lobby","name":"x","password":null}` decodes with an empty `map` that the server resolves to the yard); `an_input_without_ads_reads_as_hip_fire`; `a_state_without_loot_reads_as_no_blocks`; `the_loot_hit_and_blast_events_survive_the_codec`; `a_v13_state_with_weapon_seven_is_why_this_bumps` (documentary: `weapon_stats(7).mag != 8`). Server tests: `old_proto_may_list_but_not_join` unchanged; `a_lobby_lists_its_map`; `an_unknown_map_is_refused`.

### 2.2 The second bump: 15

Nothing on the wire changed for it; the map did. Trench City's four pads are gone and four `Cover::Loot` blocks hang at its tunnel mouths (§1.1), under the same name `"trench-city"`. The rule that spared Freight Yard a bump (the map travels by name, and a peer that does not know the name is stopped by the gate) cannot help here, because the name is the one thing that stayed the same: a first-cut v18 bundle joining a v15 lobby would build the pads-and-no-blocks Trench City its own code says the name means, predict its jump at a tunnel mouth into a block it cannot see and be corrected under it, be paid a weapon by a `Loot` event it cannot place, and run across pads that never pay. Every frame decodes, and it plays a different game. So `PROTO_VERSION` goes to 15, the frozen v18 page goes list-only against a v15 host like every page before it, and `games.json` carries v18 on `proto: 15`. The frozen fixture is regenerated as `trench-city-v18.json` and `trench_city_matches_its_fixture` pins the new map; the lesson for the next time is written at the fixture's regenerator: a changed authored map is a bump every time, whatever the wire says.

## 3. The sim: seven bullets

All in `crates/arena-core/src/shooter.rs` unless stated. Bullets, rockets, splash and loot stay **server-side only**, exactly as today; that is what keeps the `tan`, `sin_cos` and `sqrt` in the launch and the blast test safe (`CLAUDE.md`). Client prediction still touches only `move_circle` and `step_vertical`.

### 3.1 The table

```rust
pub const SIDEARM: u8 = 1;
pub const WEAPON_COUNT: u8 = 7;                 // ids 1..=7; MAX_WEAPON is retired
pub const RESERVE_INFINITE: u8 = 255;
/// What a block or a pad may hand out. Server-side only: clients never derive from it, so adding the M4 when its mesh exists is not a protocol question.
pub const LOOT_POOL: [u8; 5] = [2, 3, 5, 6, 7];
pub const ADS_SPREAD_AIR_MULT: f32 = 1.6;
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Projectile { Bullet, Rocket }
#[derive(Clone, Copy, Debug)]
pub struct WeaponStats {
    pub name: &'static str,
    pub cooldown: f32, pub mag: u8, pub reserve: u8, pub damage: u8,
    pub speed: f32, pub ttl: f32, pub radius: f32,
    pub spread: f32, pub bloom: f32, pub spread_max: f32, pub ads_spread: f32,
    pub gravity: f32, pub pierce: u8, pub kind: Projectile,
    pub splash_r: f32, pub reload: f32,
}
pub const fn weapon_stats(id: u8) -> WeaponStats   // a match, not a clamp (Ord::clamp is not const); `_ =>` is the sidearm
pub const fn weapon_name(id: u8) -> &'static str
```

| id | name | node | cooldown | mag | reserve | damage | speed | ttl (range) | spread / bloom / max | ads_spread | gravity | pierce | kind | splash_r | reload | ADS FOV |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Sidearm | `rifle` | 0.18 | 8 | ∞ | 1 | 34 | 1.6 (54 m) | 0 / 0 / 0 | 1.0 | 0 | 0 | Bullet | 0 | 1.1 | 44 |
| 2 | Vityaz | `w_vityaz` | 0.08 | 30 | 60 | 1 | 34 | 0.75 (25 m) | 0.015 / 0.005 / 0.075 | 0.5 | 0 | 0 | Bullet | 0 | 1.3 | 52 |
| 3 | AK-47 | `w_ak47` | 0.115 | 30 | 30 | 1 | 44 | 1.1 (48 m) | 0.006 / 0.006 / 0.05 | 0.5 | 0 | 0 | Bullet | 0 | 1.5 | 48 |
| 4 | M4 | `w_m4` (absent) | 0.09 | 30 | 30 | 1 | 40 | 0.85 (34 m) | 0.008 / 0.003 / 0.04 | 0.5 | 0 | 0 | Bullet | 0 | 1.4 | 46 |
| 5 | Revolver | `w_revolver_*` | 0.42 | 6 | 12 | 2 | 30 | 1.5 (45 m) | 0 / 0 / 0 | 1.0 | −3 | 0 | Bullet | 0 | 1.5 | 50 |
| 6 | Sniper | `w_sniper` | 0.9 | 5 | 5 | 2 | 60 | 1.0 (60 m) | 0.06 / 0 / 0.06 | 0.0 | 0 | 1 | Bullet | 0 | 1.8 | 22 |
| 7 | RPG-7 | `w_rpg7` + `w_rpg7_rocket` | 1.2 | 1 | 2 | 3 | 24 | 2.5 | 0 / 0 / 0 | 1.0 | −5 | 0 | Rocket | 3.0 | 2.4 | 55 |

Radius is `BULLET_R` (0.22) for every bullet and 0.35 for the rocket. A head hit deals `MAX_HP` for every gun, so "headshots kill outright" is unchanged in effect. The sidearm row **is today's pistol**: `RELOAD_SECS`, `BULLET_SPEED` and `BULLET_TTL` stay as its numbers and its spread is 0, so `pitch_does_not_shorten_a_shot`, `reload_cycle_and_ammo_gate`, the head-band tests and the lag-compensation test keep their world untouched. The judges' reason for keeping it: two designs retuned it (10 rounds, 0.16 s; a 0.012 rad spread) and both quietly rewrote every existing shot test for no design gain. `heavy_kills_in_two_hits` becomes `the_revolver_kills_in_two_hits` on id 5.

Sniper speed is 60, not 90: at `MAX_PITCH` the head-band walk clamps at 32 samples, and 60 m/s gives 0.26 m per step, under `HEAD_H` 0.30; 90 gives 0.39 and a steep headshot becomes a coin flip. `a_steep_headshot_still_registers` is re-run on weapon 6 to pin it. Sniper body damage is 2 with pierce 1: a 3-damage body one-shot lining up three bodies at 60 m was over-tuned for a 3-hp game.

Test `the_weapon_table_is_well_formed`: for every id, `mag >= 1`, `cooldown > 0`, `speed > 0`, `ttl > 0`, `gravity <= 0`, `spread <= spread_max`, `ceil(ttl / cooldown) <= MAX_BULLETS_PER_PLAYER` (no held trigger is ever throttled by the cap; the KSVR sits at 8.9, the AK at 9.6, the Vityaz at 9.4), a Rocket has `splash_r > 0` and `pierce == 0`, a Bullet has `splash_r == 0`, every `LOOT_POOL` entry is in `2..=WEAPON_COUNT`, distinct, never the sidearm; `weapon_stats(0)` and `weapon_stats(200)` are the sidearm. `MAX_BULLETS_PER_PLAYER` stays 10 (fewer `BState`s on the wire than the 16 one design asked for; the table is tuned under it, and the first knob if the wsbot run shows the cap biting is the Vityaz `ttl`).

### 3.2 Player state and the ammo economy

`PlayerSt` gains `pub reserve: u8` (rounds outside the magazine; `RESERVE_INFINITE` for the sidearm) and a server-only `fired: u8` (rounds fired since the magazine was last filled, the bloom's input; not on the wire, `PState` is built without it; reset to 0 on reload completion, on grant, on respawn and on the dry swap, incremented per launched round). `PlayerIn` gains `pub ads: bool` (held, like `shield`). `add_player` and respawn set `weapon = SIDEARM, ammo = 8, reserve = RESERVE_INFINITE, reload_t = 0`.

Reload, replacing the two-arm `else if` in the weapon-handling block:

```rust
} else if p.ammo == 0 && p.reserve == 0 {
    // Dry: the loot gun is gone and the sidearm is back, this tick. A quarter second before it fires.
    p.weapon = SIDEARM; p.ammo = weapon_stats(SIDEARM).mag; p.reserve = RESERVE_INFINITE; p.reload_t = 0.0; p.cooldown = 0.25;
} else if (input.reload && p.ammo < stats.mag && p.reserve > 0) || p.ammo == 0 {
    p.reload_t = stats.reload;
}
```

On reload completion: `take = if reserve == INFINITE { mag - ammo } else { (mag - ammo).min(reserve) }; ammo += take; if reserve != INFINITE { reserve -= take }`. On loot or pad pickup (`grant`): `weapon = id; ammo = stats.mag; reserve = stats.reserve; reload_t = 0; cooldown = 0.2` (a fifth of a second of "new gun" in which the pop-out plays). The old weapon's remaining rounds are discarded: there is no inventory.

Tests: `reload_draws_from_the_reserve_and_stops_when_it_is_empty` (revolver 6+12: two full reloads, then a reload with nothing left does not start), `a_looted_gun_runs_dry_and_the_sidearm_comes_back` (exactly 18 revolver rounds leave; the 19th trigger pull fires a sidearm round 0.25 s later with `reserve == INFINITE`), `the_sidearm_reserve_is_never_consumed` (100 reloads, still 255), `death_returns_the_sidearm`.

### 3.3 The stateless hash

```rust
/// splitmix64's finaliser: integer arithmetic only, identical on every peer and platform.
pub const fn hash64(mut x: u64) -> u64 {
    x ^= x >> 30; x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27; x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
/// One roll for (level seed, tick, player, salt). The salt separates pellets of one tick from each other and loot from spread.
pub const fn roll(seed: u64, tick: u64, who: u8, salt: u16) -> u64 {
    hash64(seed ^ hash64(tick.wrapping_add(0x9e37_79b9_7f4a_7c15)) ^ ((who as u64) << 56) ^ ((salt as u64) << 40))
}
/// Two uniforms in [0, 1) from one roll: 24 mantissa bits each, exact in f32.
pub fn unit_pair(h: u64) -> (f32, f32)
pub const SALT_LOOT: u16 = 0x1007;
```

`Sim` gains `pub seed: u64`; `Sim::from_level(level: &Level, seed: u64)`; `Sim::new(seed)` is `from_level(&Level::from_seed(seed), seed)`. The server passes `lobby.seed`, which it already mints per lobby and already sends in `GameJoined`. This is the "seeded and tick-indexed" per-tick randomness `CLAUDE.md` demands, with no RNG state at all: the same inputs give the same round on every peer and in every replay. Tests: `hash64_is_the_splitmix_finaliser` (three vectors computed by hand and pinned; the finaliser alone has no published set), `roll_differs_across_tick_player_and_salt`, `unit_pair_is_in_range_and_exact_for_the_mantissa`, `no_rng_state_lives_on_the_sim` (a grep-shaped assertion in the test: the only `rand01` closures in the file are the two world generators).

### 3.4 Launch: spread, bloom, the cap

The firing branch becomes a pure function so a test can hand it any row:

```rust
pub fn launch(p: &PlayerSt, stats: &WeaponStats, ads: bool, grounded: bool, seed: u64, tick: u64, delay: u16, out: &mut Vec<Bullet>)
```

Effective cone: `cone = (stats.spread + stats.bloom * fired).min(stats.spread_max) * if ads { stats.ads_spread } else { 1.0 } * if grounded { 1.0 } else { ADS_SPREAD_AIR_MULT }`, where `fired` is `PlayerSt.fired`, the rounds fired since the magazine was last filled, read before this round's increment (the Vityaz starts tight and opens up; the AK's climb reads as the cone). It is a counter and not `stats.mag - p.ammo`, which the first cut used: a reserve-short refill leaves the magazine part-full (the AK's last magazine is 10 of 30), and the difference then reads a fresh magazine as twenty rounds into a spray, its first round already at `spread_max`. `grounded` is the `VStep.grounded` computed for this player this tick, already in scope. **If `cone == 0.0` the aim is used exactly as today and no rotation happens**: the sidearm's ray is bit-identical to v17. Otherwise `(u, v) = unit_pair(roll(seed, tick, p.id, 0))`, `r = cone * u.sqrt()` (uniform over the disc), `theta = TAU * v`, `yaw_off = r * theta.cos()`, `pitch_off = r * theta.sin()`; the horizontal aim is rotated by `yaw_off` and the pitch is `(p.pitch + pitch_off).clamp(-MAX_PITCH, MAX_PITCH)`; `vel = aim' * speed`, `vy = pitch'.tan() * speed`, `y = p.y + eye_h(p.crouch)`, muzzle 0.2 ahead as today. The bullet cap check is unchanged (`active < MAX_BULLETS_PER_PLAYER`; the loop is written for one round per shot, and pellets are not in v18).

`Bullet` gains `pub weapon: u8, pub pierce: u8, pub hit_mask: u8` (player ids are 0..7 by `alloc_pid`, so a bit per player); gravity, kind, radius and splash are looked up through `weapon_stats(b.weapon)` so the struct stays small and the table stays the one source.

Tests: `spread_is_a_pure_function_of_seed_tick_and_shooter` (same inputs twice: identical bullets; change the tick: different), `a_zero_spread_weapon_fires_exactly_along_the_aim` (`vel == aim * speed`, `vy == tan(pitch) * speed` bit for bit, and `pitch_does_not_shorten_a_shot` keeps passing on the sidearm), `spread_never_leaves_the_cone` (10 000 launches over ticks; the angle between `vel` and the aim is at most `cone + 1e-4`), `bloom_widens_the_cone_as_the_magazine_empties` (Vityaz: the max offset over rounds 0..5 is smaller than over rounds 24..29), `a_reserve_short_refill_starts_the_bloom_over` (AK: twenty rounds, a manual reload, thirty more, the reserve-short auto-reload leaves 10 of 30 with `fired == 0`, and its first round is bit-identical to a full magazine's first round for the same roll; the count follows the rounds and the dry swap zeroes it), `ads_and_air_scale_the_cone` (sniper with `ads`: offset exactly 0; airborne AK: the max observed offset exceeds the grounded max), `bullet_cap_holds` unchanged.

### 3.5 The sweep, per kind

Order inside `bullets.retain_mut` is unchanged: TTL, gravity, segment, per-player body test (rewound by `b.delay`), cover at contact, shield, damage, then integrate, floor, wall, cover span. The changes:

- **Gravity**: before the segment is formed, `b.vy += stats.gravity * dt` (semi-implicit Euler, like the player). The horizontal `vel` is untouched, so every range-per-tick invariant stands. Tests `a_revolver_slug_drops_under_gravity_and_keeps_its_horizontal_speed` (after 60 ticks `|vel| == 30` exactly and `y` fell by `0.5 * 3 * 1` ± 0.03) and `a_zero_gravity_round_flies_the_old_straight_line` (a sidearm round's `y` equals the v17 formula every tick).
- **Skip pierced bodies**: in the player loop, `if p.id == b.owner || b.hit_mask & (1 << p.id) != 0 { continue }`.
- **Pierce**: where today `hits.push(..); return false;`: push the hit; `if b.pierce == 0 { return false }`; else `b.pierce -= 1; b.hit_mask |= 1 << p.id;` and **continue the player loop** (two bodies on the same tick's segment are both hit; the `break` one design proposed skips the second for good, because the segment is only formed once per tick). After the loop the round integrates and takes the world tests as usual. Tests `a_sniper_round_passes_through_one_body_and_hits_the_next` (three targets on a line 3 m apart: the first two lose 2 hp, the third is untouched), `two_bodies_on_one_segment_are_both_hit`, `a_pierced_body_is_not_hit_twice` (a target the round overlaps on two consecutive ticks loses hp once), `pierce_does_not_pass_through_cover`.
- **Shield, per kind**: a bullet of every gun reflects exactly as today (mirror, owner transfer, `delay = 0`) and additionally `b.pierce = 0; b.hit_mask = 0`. A **rocket detonates on the plate**, with the holder excluded from the splash, and never reflects: a plate is cover, not a launcher, and a 3-damage rocket bounced by a 120° arc would be a free kill on the shooter with no counterplay. Tests `a_reflected_sniper_round_no_longer_pierces`, `a_reflected_round_keeps_its_weapon_and_gravity`, `a_rocket_on_a_raised_shield_spares_the_holder_and_hits_the_flank` (holder keeps 3 hp; a third player 1.5 m beside the holder loses 2); the existing shield tests are unchanged.
- **Rocket** (`Projectile::Rocket`, radius 0.35): the body sweep is the same; a connect calls `detonate(b, contact, cy, Some(victim))`; floor, wall, cover-span and TTL exits call `detonate` at the last free point (floor: the `y = 0` crossing at `y = 0.05`; wall: the clamped position; cover span: `(p0, y0)`; TTL: the position). `detonate` pushes `(owner, victim, 3, false)` for a direct hit, then for every other alive player `q` **rewound by `b.delay`** (the same `rewound()` the sweep uses: an explosion is the shooter's aim, not the defender's decision, so it is judged where the shooter saw the bodies) with `d = (dist_xz(blast, q) - hit_radius(q.crouch)).max(0)`: damage 2 if `d <= 1.5`, 1 if `d <= 3.0`, else nothing; the blast height must lie in `[ty - 3.0, ty + body_h + 3.0]`; and only when `!segment_hits_cover(blast, chest, &obstacles)` with `chest = (q.pos, ty + 0.9)`. The owner is included. Then `self.blasts.push(([bx, by.max(0.05), bz], b.owner))` and the round is gone. A rocket never pierces.

```rust
/// Exact slab test of a 3D segment against a box's [min, max] x [base, h]. Arithmetic only.
pub fn segment_hits_box(a: [f32; 3], b: [f32; 3], o: &Obstacle) -> bool
pub fn segment_hits_cover(a: [f32; 3], b: [f32; 3], obstacles: &[Obstacle]) -> bool
```

Scoring: in the damage loop, `if owner != victim { killer.score += 1 }`; the kill event is still pushed, so a self-kill reads as `Kill { killer: id, victim: id }` on every client. Tests: `a_direct_rocket_hit_kills_outright`, `a_rocket_detonates_on_cover_and_splashes_round_the_corner_only_with_line_of_sight` (two targets 2 m from a crate face: the one behind it keeps 3 hp, the one beside it loses 2), `splash_falls_off_by_radius_two_then_one`, `the_shooter_eats_their_own_splash_and_it_is_not_a_frag` (fire into the floor at `MAX_PITCH` down with 1 hp: `events == [(0, 0)]`, `score == 0`, `death_count == 1`), `a_rocket_detonates_at_its_ttl_at_the_floor_and_at_the_wall` (three sims, one blast each, the floor blast at `y == 0.05`), `splash_uses_the_rewound_body` (a target that dodged 3 m after launch, delay 12: still hurt), `segment_hits_box_never_blocks_where_the_span_test_is_clear` (500 hashed segments against `roof()`: the exact test is never stricter than the conservative one when the segment is wholly outside).

- **Lag compensation**: `HistoryFrame` is unchanged; the direct hit, the pierce and the splash all read `rewound(id, b.delay)`; a reflected round reads the present. `MAX_REWIND_TICKS` 18 unchanged.
- **Determinism**: `two_sims_with_the_same_seed_and_inputs_agree_bit_for_bit` (two `Sim::from_level(&Level::freight_yard(), 7)`, four players, a scripted 600-tick input table that cycles every weapon by setting `weapon`/`ammo`/`reserve` directly, comparing `players` and `bullets` field by field with `to_bits()` every tick) and `client_prediction_and_server_agree_on_every_bonk` (a `Sim` versus a bare `move_circle`/`step_vertical` replay with identical inputs on the yard for 600 ticks: `y`, `vy` and `bonked` equal tick by tick).

### 3.6 Events out of the sim

`Sim.events` stays (kills). New, all cleared at the top of `step`: `pub hits: Vec<(u8, u8, u8, bool)>` (shooter, victim, dmg, head; filled from the damage loop, so it is authoritative and replaces the client's "my bullet vanished and someone lost hp" heuristic, which pierce would break and which already fires falsely on a reflected round, `docs/plans/backlog.md`), `pub blasts: Vec<([f32; 3], u8)>`, `pub loot_events: Vec<(u8, u8, u8)>` (player, block index, weapon).

## 4. The map: Freight Yard

`MAP_FREIGHT_YARD = "freight-yard"`, `Level::freight_yard()`, authored in a new module `crates/arena-core/src/freight_yard.rs` (`shooter.rs` gains only `Cover::Loot`, the constant, and one arm in `Level::named`; the existing file is 4138 lines). Theme: a rail freight yard at golden hour. Two trains of closed containers run east–west, a loading dock stands in the open yard between them with the king block hanging over it, a walled backlot behind each train is where you spawn, and `?` blocks hang in the train zone like signal boxes; two more hang over the middle containers where only the roof network reaches them. The prop set is v13's (containers, crates, ammo boxes, sandbags, rubble, the plinth as the dock) plus `Cover::Loot`; no walls, no roofs, no pads.

### 4.1 Frame and symmetry

`ARENA_HALF` 24 unchanged. **Quadrant, then mirror** (D2): the north-east quadrant (`x >= 0`, `z >= 0`) is authored once and mirrored across `x = 0` and `z = 0`; pieces that straddle one axis are authored in a spine list and mirrored across the other axis only; the dock and the king block sit on the origin. `mirror_x`/`mirror_z` negate one axis of **both** corners and re-derive min/max (the componentwise trap `docs/asset-pipeline.md` warns about, the same lesson as `rot90_box`). Mirrors and not v13's quarter turn because of the lanes: a rotation would turn the trains into a ring; mirrors keep every container parallel to x, which is what gives the yard its two 48 m sightlines and boxes the train zone off from the yard and the backlot (the train zone is itself a 48 m lane, §4.5), and that is what makes the sniper and the SMG different guns here. Test `yard_lists_are_in_their_half_planes` pins the list contracts so no box is ever emitted twice.

Heights: container 2.6, stacked pair 5.2, crate 1.2, ammo 0.55, sandbag 1.1, rubble 0.7, dock 1.2. All `base 0` unless stated.

### 4.2 Centre (once)

| Kind | x | z | base | h | Note |
|---|---|---|---|---|---|
| Plinth ("the dock") | −4.0..4.0 | −2.0..2.0 | 0 | 1.2 | A jumpable stage in the open yard. |
| **Loot K** ("the king block") | −0.5..0.5 | −0.5..0.5 | **3.45** | 4.45 | From the dock a hop (met on the third integrator step, 0.42 m of rise against 0.39 of headroom). The dock lies under the block, so no floor stands directly beneath it: the floor bonk pinned by the test is a sprinting run-up from (0, 2.7) beside the dock, whose head meets the block once at the apex (feet at 1.59 m) before anything but the floor has touched its feet. |

### 4.3 Spine along x (authored for x > 0, mirrored in x → 2 each)

| Kind | x | z | h | Note |
|---|---|---|---|---|
| Container ("yard wagon") | 4.5..10.5 | −1.2..1.2 | 2.6 | Splits each yard lane; blocks the x = ±5.5 spawn-to-spawn line. |
| Crate | 10.9..12.1 | −0.6..0.6 | 1.2 | Wagon chain step (gap 0.4). |
| Ammo | 12.5..13.5 | −0.4..0.4 | 0.55 | Chain start (gap 0.4). |
| Rubble | 19.0..21.0 | −1.0..1.0 | 0.7 | Yard flank dressing. |

### 4.4 Spine along z (authored for z > 0, mirrored in z → 2 each)

| Kind | x | z | base | h | Note |
|---|---|---|---|---|---|
| Container C1 ("train 1 middle") | −3.6..3.6 | 5.0..7.4 | 0 | 2.6 | Blocks the yard's centre from the backlot diagonals. |
| Container C2 ("train 2 middle") | −3.6..3.6 | 14.0..16.4 | 0 | 2.6 | Carries the roof block. |
| **Loot R** ("roof block") | −0.5..0.5 | 14.7..15.7 | **4.9** | 5.9 | Over C2. From its roof (feet 2.6, head 4.46) it is walked under and bonked with a 0.44 m rise; from the floor the head reaches 3.55 at most, so it is a roof-only reward, and nothing in the yard reaches its top (feet would need 5.55). |
| Container C3 ("backlot divider", along z) | −1.2..1.2 | 18.6..22.6 | 0 | 2.6 | Stops the two spawns of a backlot seeing each other across x = 0. |

### 4.5 Quadrant (x > 0, z > 0; mirrored → 4 each)

| Kind | x | z | base | h | Note |
|---|---|---|---|---|---|
| Container Q1 ("train 1 outer") | 7.6..13.6 | 5.0..7.4 | 0 | 2.6 | Crossing x 3.6..7.6 between C1 and Q1; the flank x > 13.6 is open. |
| Crate | 4.0..5.2 | 6.2..7.4 | 0 | 1.2 | C1 chain step against C1's east face (gap 0.4); 2.4 m of the crossing (5.2..7.6) stays open. |
| Ammo | 4.0..5.0 | 7.8..8.6 | 0 | 0.55 | C1 chain start (gap 0.4 in z). |
| Crate | 14.0..15.2 | 6.2..7.4 | 0 | 1.2 | Q1 chain step (gap 0.4). |
| Ammo | 14.0..15.0 | 7.8..8.6 | 0 | 0.55 | Q1 chain start. |
| Container Q2 ("train 2 outer") | 7.6..14.0 | 14.0..16.4 | 0 | 2.6 | Backlot entrance x 3.6..7.6 (4 m). 6.4 long on purpose: closes the (19, 21.5) → (−6.5, −21.5) diagonal by 0.8 m. |
| Crate | 14.4..15.6 | 15.2..16.4 | 0 | 1.2 | Q2 chain step (gap 0.4). |
| Ammo | 14.4..15.4 | 16.8..17.6 | 0 | 0.55 | Q2 chain start. |
| Container Q3 ("stacked pair") | 17.6..23.6 | 14.0..16.4 | 0 | **5.2** | Corner landmark, unreachable; the second backlot entrance is the 2.0 m gap x 15.6..17.6. |
| Container Q4 ("backlot divider", along z) | 10.2..12.6 | 19.0..23.0 | 0 | 2.6 | Blocks the same-backlot spawn pair. |
| Ammo | 9.6..10.4 | 17.4..18.2 | 0 | 0.55 | Q4 chain start, 0.4 west of the crate. The draft put it against Q2's north face at 10.8..11.6 × 16.4..17.2, where a standing body on it is held between Q2 (its head) and the crate (`STEP_UP`) and the chain invariant cannot pass through it; the sim proved it (the map worker swapped the draft position back and watched the test fail), and neither of the named knobs could fix it, so the ammo box moved. |
| Crate | 10.8..12.0 | 17.6..18.8 | 0 | 1.2 | Q4 chain step (gap 0.2 to Q4, 0.4 to the ammo's east face); one hop from here also reaches Q2's roof. |
| Crate | 1.6..2.8 | 18.6..19.8 | 0 | 1.2 | C3 chain step (gap 0.4 to x 1.2). |
| Ammo | 3.0..3.8 | 18.6..19.4 | 0 | 0.55 | C3 chain start (gap 0.2 to the crate, 0.2 to the sandbag). |
| Sandbag | 4.0..8.0 | 18.0..18.8 | 0 | 1.1 | Spawn cover for (5.5, 21.5); hop-over cover, not a wall. |
| Sandbag | 15.0..19.0 | 18.0..18.8 | 0 | 1.1 | Spawn cover for (19, 21.5). |
| Rubble | 20.0..22.0 | 8.0..10.0 | 0 | 0.7 | Train-zone flank. |
| **Loot L** ("train-zone block") | 6.0..7.0 | 9.6..10.6 | **2.30** | 3.30 | Off the C1 crossing exit. Walked under with 0.44 m of headroom; a floor jump meets it on the fourth integrator step after the press (0.55 m of rise against 0.44 m of headroom; the third step gives 0.42): the Mario snap. Its top (3.30) is 0.70 m above a container roof (2.6), a hop, so it is a perch (§5.1). |

Counts: centre 2 + spine-x 4×2 + spine-z 4×2 + quadrant 18×4 = **90 boxes** (Trench City has 85, not the 89 an earlier draft claimed); **7 loot blocks** (K, R×2, L×4); 0 pads. The block that one design hung in the backlot 5.5 m from a spawn is dropped: a block top is a perch (§5.1) and a perch over a spawn is the worst thing this map could produce.

**Spawns** (quadrant, mirrored, listed so slots 0..4 land in four different quadrants): `(5.5, 21.5)` and `(19.0, 21.5)`. Clearances: (5.5, 21.5) — sandbag 2.7, the C3 ammo 2.6 (diagonal), Q4 4.7, C3 4.3; (19, 21.5) — sandbag 2.7, Q3 5.1, Q4 6.4, the boundary 2.5 (the wall is at 24, the clear zone ends at 23.4). Pairwise minimum 11.0 (the x-mirror pair, with C3 between; the same-backlot pair is 13.5 apart with Q4 between).

**Sightline reasoning** (level fire from the floor at eye 1.45; anything under 1.45 is not a blocker, so crates, sandbags, rubble and every block are transparent to it): the same-backlot pair is blocked by Q4; the x-mirror pair by C3 (z 21.5 lies inside 18.6..22.6); the z-mirror pairs by the yard wagon (x 5.5 inside 4.5..10.5) and Q3 (x 19 inside 17.6..23.6). The four diagonal classes: (5.5, 21.5) → (−19, −21.5) enters C2 at z 15.8; (5.5, 21.5) → (19, −21.5) enters Q2 at z 14.8; (5.5, 21.5) → (−5.5, −21.5) passes the train-2 gap at x 4.2 and meets C1 at z 7.4; (19, 21.5) → (−19, −21.5) passes the 15.6..17.6 gap and meets Q2's mirror at z −14; (19, 21.5) → (−5.5, −21.5) meets Q1 at z 7.4; (19, 21.5) → (5.5, −21.5) threads the train-2 gap and is stopped by Q1's z-mirror. Every class is a container more than 0.5 m deep by hand; **the test is the authority** (`no_yard_spawn_sees_another`, all 56 ordered pairs with the sim's own round, both directions; the driver holds every round at 4 s of life because the sidearm's own 54 m falls short of the far-corner pairs at 57.4 m, and it first proves that a round crosses an empty arena corner to corner; with that reach every pair still holds as authored and no box moved), none of the three panel designs' hand tables survived the judges' re-derivation without an error, and one iteration of moving a box is budgeted. The yard keeps its two long lanes on purpose, and so, it turned out, does the train zone: the draft claimed the train-zone alley (z 7.4..14) was short, but its blocks hang at 2.30 and are transparent to level fire at eye 1.45, and nothing else in the alley rises above 1.45, so the alley is a 48 m lane end to end as authored. `the_yard_has_a_long_sightline_and_the_train_zone_does_not` therefore pins the contrast the geometry delivers (the yard lane at z 3 connects; a body in the alley at (−12, 10) sees neither the yard at (−12, 3) nor the backlot at (−12, 21.5), both directions). Cutting the alley needs a new container, not a named knob; it is a backlog line, not done blind.

**Climbing chains**: every 2.6 container has a crate within 0.5 of a face and an ammo box within 0.5 of the crate — the yard wagon, C1, Q1, C2 (via Q2's crate at one hop), Q2, Q4, C3 — proven by the v13 `climb` driver, which moves with `hop`, `shot_over`, `centre`, `gap`, `contains` and `dist` into a shared `level_helpers` test module so both maps run one driver. Q3 at 5.2 has none and the floor never reaches it.

**Routes**: backlot ↔ train zone through the 4 m gap (x 3.6..7.6) and the 2 m gap (15.6..17.6); train zone ↔ yard through the C1/Q1 crossing (2.4 m clear beside the chain crate) and the open flank x > 15.2; sandbags are hopped.

**Decor** (`freight_yard_decor()`): twelve façades on the radius-44 ring every 30° facing in (the cathedral's slot filled: no cathedral, no statue), lamps at (±26, 0, ±8) and (±8, 0, ±26) scale 5, wrecks at (±27, 0, ±27) yaw π/4 scale 1.5 and two more at (±27, 0, 0) yaw π/2 (on the flanks they read as rolling stock). Sky, ground, cobble floor and city wall as v13. Nothing new is generated.

**Why it plays differently from Trench City**: rings versus lanes. Trench City funnels eight players into four identical tunnel fights over the block at each tunnel mouth (the pads, until §2.2). Freight Yard has an open, dangerous yard (long lanes for the AK and the sniper, the dock and the king block as the exposed prize), a train zone between the trains, boxed off from the yard and the backlot, a lane 6.6 m wide (z 7.4..14) with the container roofs over it (SMG, revolver, RPG lobs), protected backlots you leave through two gaps each, and a second storey with its own reward.

### 4.6 Invariants (tests in `freight_yard.rs`, one name each)

1. `yard_has_eight_clear_spawns` — inside `ARENA_HALF − PLAYER_R`, none overlapping a box (circle test, `PLAYER_R`), pairwise ≥ 11.0, ≥ 2.0 from any box edge, slots 0..4 in four different quadrants.
2. `yard_boxes_are_inside_and_well_formed` — `min < max`, inside 24, `base < h`, every `base > 0` box is `Cover::Loot`, every Loot is 1 × 1 with base in {2.30, 3.45, 4.9}; no two boxes overlap.
3. `yard_is_mirror_symmetric_in_x_and_z` — multiset equality under each mirror (sorted, 1e-4), spawns and the Loot subset included; the mirror is written out in the test, not borrowed from the builder.
4. `yard_lists_are_in_their_half_planes`.
5. `no_two_raised_boxes_share_a_footprint` — both maps; pins that the lowest-base clamp rule (§5.2) is bit-identical for every shipped level.
6. `every_yard_container_roof_is_reached_by_a_climbing_chain` (the shared driver; Q3 excluded by height).
7. `no_yard_spawn_sees_another`.
8. `the_yard_has_a_long_sightline_and_the_train_zone_does_not` (the yard lane connects; the alley is sealed from the yard and the backlot; see the sightline note above for why the alley itself is long).
9. `every_train_zone_block_is_bonked_from_the_floor_and_walked_under` — for each L: 90 ticks jumping under it reports `bonked == Some(i)` once; 52 ticks walking under it (3.9 m through and back) never does and never lifts the feet; `blocked` is false at `y 0` on its footprint.
10. `the_king_block_is_bonked_from_the_dock_and_from_the_floor`.
11. `the_roof_block_is_bonked_from_its_container_and_not_from_the_floor` — from C2's roof the clamp reports it; 200 ticks of jumping at it from the open floor beside C2 never do, and never get a body onto C2.
12. `the_backlot_reaches_the_train_zone_through_two_gaps` (walked with `move_circle`).
13. `every_spawn_reaches_every_block_footprint` — a flood fill on a 0.2 m grid from every spawn at `y 0` through `blocked` reaches every block's footprint and the dock.
14. `yard_survives_serde` and `a_v13_json_level_still_decodes_without_loot`.
15. `trench_city_matches_its_fixture` — `Level::trench_city()` equals a frozen JSON fixture at `crates/arena-core/tests/fixtures/trench-city-v18.json` (regenerated from the map with its four blocks and no pads; the v17 fixture is gone with the bump in §2.2); `a_seeded_level_is_exactly_the_arena_the_generator_made` and `a_seeded_level_still_plays_exactly_the_v12_arena` stay. In `shooter.rs`, `trench_city_has_four_blocks_at_the_tunnel_mouths` replaces `all_four_pads_are_in_the_open_under_a_roof` (four Loot boxes, each bonked from the floor on the fourth step by driving `step_vertical`, walked under without a bonk, over no roof, and reached by a flood fill from every spawn at y 0 through `blocked`), and `a_bonk_on_a_trench_city_block_pays_a_pool_weapon` drives a `Sim` built from the map.

## 5. Loot blocks

### 5.1 Geometry, and the perch decision

`Cover::Loot` is a new variant, appended last, so every existing `match` grows one arm and the default stays `Container`. A block is `Obstacle::boxed(Cover::Loot, [x − 0.5, z − 0.5], [x + 0.5, z + 0.5], base, base + 1.0)`; `pub const LOOT_SIZE: f32 = 1.0; pub const LOOT_RESPAWN_SECS: f32 = 18.0;` and the three authored bases above.

**The four rules that read `base` do not change.** `blocked`, `support_height`, the ceiling clamp and the bullet span test all read `base`/`h` kind-agnostically, and a Loot block is an ordinary solid box to all four: walked under when your head is below its base, a wall to a body whose head is above its base, stood on by feet at or above its top, and rounds through it are stopped. The only sim change is that the clamp now *reports* which box it hit. The consequence is accepted and pinned: a train-zone block's top (3.30) is 0.70 m above a container roof (2.6), a hop and not a step, from any roof within reach, so **block tops are perches** — 1 m wide, exposed on every side, 3.3 m up, and a bonk from below still pays while someone stands on top. One design forked `blocked` and `support_height` for `Cover::Loot` so a block has no top; `CLAUDE.md` says the four rules must agree and the judges split on it; the minimal change wins, the perch is placed nowhere near a spawn (§4.5), the king block and the roof blocks cannot be perched at all, and the backlog records that a support-skipping rule would be a bump if play proves the perches wrong.

### 5.2 Bonk detection: the signature change

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VStep { pub y: f32, pub vy: f32, pub grounded: bool, pub bonked: Option<usize> }
pub fn step_vertical(pos, y, vy, jump, dt, obstacles) -> VStep
```

The ceiling loop keeps the box with the **lowest `base`** among those that clamp (today the last box in list order wins the assignment; no shipped level has two raised boxes over one footprint and invariant 5 keeps it that way, so every existing test is bit-identical) and reports its index. `Sim::step`, the client's prediction and the client's reconciliation replay all fail to compile until they read the struct, which is the point. `a_jump_under_a_raised_box_bonks_at_its_bottom` additionally asserts `bonked == Some(0)` on the clamp tick and `None` on every other.

In `Sim::step`, right after `step_vertical`: `if let Some(k) = v.bonked && vertical_speed > 0.0 && self.obstacles[k].kind == Cover::Loot { bonks.push((i, k)) }`. Requiring the pre-step `vy > 0` is belt and braces: the clamp can only fire on the way up, because `blocked` stops horizontal entry; `a_block_is_bonked_only_from_below` pins it (a player dropped past a block's side, and one walking under it, never bonk).

### 5.3 State and the reward

```rust
pub struct LootBlock { pub obstacle: usize, pub respawn_t: f32 }   // 0 = armed
// Sim gains: pub loot: Vec<LootBlock> (from every Cover::Loot obstacle, in obstacle order), pub loot_events
pub fn loot_roll(seed: u64, tick: u64, who: u8, holding: u8) -> u8
```

`loot_roll` is uniform over `LOOT_POOL` minus the weapon held (if it is in the pool), indexed by `roll(seed, tick, who, SALT_LOOT) >> 33`. No weights: five guns of five roles, and weighting is balance tuning with no evidence yet. Bonks are resolved after the player loop **in player order**: `for (i, k) in bonks { if let Some(slot) = loot.iter_mut().find(|l| l.obstacle == k) && slot.respawn_t <= 0.0 { let w = loot_roll(seed, tick, pid, p.weapon); grant(p, w); slot.respawn_t = LOOT_RESPAWN_SECS; loot_events.push((pid, slot_index, w)) } }`. Timers tick down where the pads' do. **Pads** call the same `grant` with the same `loot_roll`: the `p.weapon >= MAX_WEAPON` gate goes, a pad is taken by anyone alive with feet below `PAD_PICK_H`, and it cools down `PAD_RESPAWN_SECS` 15 as before. Since the second bump (§2.2) only the seeded arena carries pads; neither authored map does.

Tests: `a_floor_jump_under_a_train_zone_block_bonks_it_and_names_the_block`, `walking_under_a_block_is_not_a_bonk`, `a_bonk_grants_a_pool_weapon_with_a_full_load` (`LOOT_POOL.contains(w)`, `ammo == mag`, `reserve == stats.reserve`, `loot_events == [(0, i, w)]`), `the_reward_never_repeats_the_gun_in_hand` (500 rolls holding each pool gun: never itself; holding the sidearm: every pool member appears), `the_roll_is_uniform_enough` (10 000 rolls: each pool entry within 20 % of the mean), `the_roll_is_a_pure_function_of_seed_tick_and_player`, `a_used_block_is_dead_for_eighteen_seconds` (a second bonk at +1 s pays nothing; at +18.1 s it pays; `State.loot` reads false then true), `two_players_bonking_one_block_on_one_tick_pay_once`, `a_dead_player_cannot_bonk`, `pads_hand_out_the_same_loot` (replaces `pads_upgrade_and_death_resets`: the weapon is in the pool, full load, respawn 15 s, death returns the sidearm).

### 5.4 What the client does with it

- **Predicted bump**: the client's own prediction `step_vertical` returns `bonked`; on the frame it flips to `Some(k)` for a Loot block whose `loot_active[k]` is true, the block starts a 0.25 s bump (`0.25 * sin(pi t / 0.25)` m up and back), `Sfx::Bonk` plays, the camera dips 0.06 m over 80 ms, and rumble (0.5, 1.0, 90 ms) fires. A bonk on a dead block plays a dull `Sfx::Click` and nothing else, so "nothing happened" is felt too. Remote bonks are not predicted; their bump comes with `S2C::Loot`.
- **Authoritative pop**: on `S2C::Loot { player, block, weapon }` the block goes dark (the next `State` keeps it so) and the granted weapon's own mesh (`assets.weapons[weapon]`, the sidearm mesh when absent) rises 0.6 m out of the block's top over 0.5 s spinning two turns about Y, then vanishes. For my own: `Sfx::Pop`, rumble (0.2, 0.6, 120 ms), status line `? popped: AK-47 (30+30)`; for others within 20 m the pop at 0.25. If the server disagrees with a predicted bonk (someone took the block 30 ms earlier) the bump plays and no pop follows; that is the honest reading of prediction, and the pad race has the same shape today.
- **Drawing**: `Prop::Loot` is a `tiled_box(Vec3::ONE, tex(TEX_LOOT))` with the 512 × 512 `assets/textures/v18/loot.png` (drawn by `tools/v18/loot_texture.py`: a riveted brass plate with a bevelled rim and a bold `?`, deterministic, free) on every face, pushed by `Props::push_obstacle` for `Cover::Loot` at `base + 0.5` with unit size and colour `Vec3::ONE`, plus a slow 0.6 rad/s yaw bob of ±0.02 m while armed. A **used block is the same mesh tinted `Vec3::splat(0.42)`**: the per-instance colour multiply is what the renderer has, and a second picture would be a byte every web player downloads for nothing.
- **HUD**: the status line reads `AK-47 24/30 · 30`; the sidearm shows `Sidearm 7/8 · ∞`.

## 6. Feedback and haptics

### 6.1 The engine API (`crates/ember-engine/src/feedback.rs`, new; `input.rs` extended; `app.rs` wires both)

```rust
/// One rumble request: motor magnitudes 0..1 and a duration. Requests in a frame are MERGED per channel (max) and the longest duration wins, so a 30 ms hitmarker tick never cancels a 400 ms death rumble.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rumble { pub strong: f32, pub weak: f32, pub ms: u16 }
#[derive(Clone, Debug, Default)]
pub struct Feedback { pub rumbles: Vec<Rumble> }
impl Feedback { pub fn rumble(&mut self, strong: f32, weak: f32, ms: u16) }
pub trait EmberGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame;
    /// Called by the platform right after `update`. Default: nothing, so pong, fire, kings and the editor are untouched.
    fn feedback(&mut self) -> Feedback { Feedback::default() }
}
```

The one-way layering holds: the game returns data, `app.rs` (platform) consumes it, `renderer.rs` never sees it. `App` holds a `Haptics` (platform-split) with `request(r, now)` (merge: `strong = max(new, running if now < ends_at)`, same for weak, `ends_at = max`), `tick(now)` (stop when `ends_at` passes), `status() -> &'static str` (`none | input-only | input+rumble`, logged once). `Focused(false)` stops both motors like it clears the keys. Tests (pure, native): `feedback_merges_per_channel_max` (0.2/0.8/40 then 0.9/0.1/300 → 0.9/0.8/300), `an_expired_rumble_does_not_leak_into_the_next`.

**Gamepad input** (`input.rs`): `InputState` gains `pad: Option<PadState>`, `pub fn pad(&self) -> Option<PadState>`, `pub(crate) fn set_pad`.

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState { pub left: [f32; 2], pub right: [f32; 2], pub lt: f32, pub rt: f32, pub buttons: u16 }
/// Bit index = the W3C Gamepad "standard" mapping button index, so both platforms produce one bitmask.
#[repr(u8)] pub enum PadButton { South = 0, East = 1, West = 2, North = 3, LB = 4, RB = 5, LT = 6, RT = 7, Back = 8, Start = 9, L3 = 10, R3 = 11, Up = 12, Down = 13, Left = 14, Right = 15 }
impl PadState { pub fn down(&self, b: PadButton) -> bool; pub fn stick(raw: [f32; 2]) -> [f32; 2] }
```

`PadState::stick`: radial dead zone 0.18, then `m' = ((m − 0.18) / 0.82)^1.8` on the magnitude, direction kept; sticks report Y up-positive (the web reports down-positive: negate). Test `pad_axis_curve_is_dead_below_threshold_and_monotonic`.

**Native**: `gilrs = "0.11"` under `cfg(not(wasm32))` (builds in 13 s on this workstation; its default Windows backend is Windows.Gaming.Input (WGI), which carries the force feedback). `Gilrs::new().ok()` in `run()` on the main thread; a failure logs and disables pads. Each redraw before `game.update`: drain `next_event()`, read the first connected pad's sticks, triggers and buttons through the table South→0, East→1, West→2, North→3, LeftTrigger→4, RightTrigger→5, LeftTrigger2→6, RightTrigger2→7, Select→8, Start→9, LeftThumb→10, RightThumb→11, DPad→12..15 (test `standard_mapping_indices_match_gilrs_names`). Rumble: on first sight of a pad with `is_ff_supported()`, build **two persistent effects once** (`BaseEffectType::Strong { magnitude: u16::MAX }` and `Weak`, `Replay { play_for: Ticks::from_ms(10_000) }`), and a request is `set_gain(r.strong); play()` on each; `tick` calls `stop()` when `ends_at` passes. Persistent effects because building a fresh `ff::Effect` per request exhausts a pad's effect slots.

**Web**: input through web-sys (features `Navigator`, `Gamepad`, `GamepadButton`, `GamepadMappingType`; all stable), so every page gets a pad: each frame `navigator().get_gamepads()`, the first entry that is a connected `Gamepad`, axes 0..3 and buttons by standard index with `value()` for the triggers. Rumble through a **shim on the page**, because `GamepadHapticActuator::play_effect` is behind `web_sys_unstable_apis` and `deploy-pages.sh`'s `copy_pkg` copies only `arena.js` + `arena_bg.wasm` (a wasm-bindgen `inline_js` snippet would never reach the live page):

```js
window.emberRumble = (strong, weak, ms) => {
  for (const g of navigator.getGamepads?.() ?? []) {
    g?.vibrationActuator?.playEffect?.('dual-rumble', { startDelay: 0, duration: ms, strongMagnitude: strong, weakMagnitude: weak })?.catch?.(() => {});
  }
};
```

called from Rust through `js_sys::Reflect::get(&window, "emberRumble")` (looked up once and cached as a `js_sys::Function`; `js-sys = "0.3"` joins the wasm deps) with `call3`. A page without the shim or a browser without the actuator (Firefox, Safari today; Chromium has it) is a silent no-op. Browsers surface a pad only after a button press on it; the page hint says so.

### 6.2 The gamepad mapping (client, `online.rs`; merged with keyboard and mouse, either device at any moment)

| Intent | Key / mouse | Pad (standard mapping / Xbox-class) |
|---|---|---|
| move | WASD | left stick (curved) into the same `mv` |
| look | mouse delta × `LOOK_SENS` | right stick: `yaw += rx · 2.8 · dt`, `pitch −= ry · 2.0 · dt`, × 0.55 while ADS |
| fire | LMB | RT > 0.5 |
| ADS (`Input.ads`, new on the wire) | RMB | LT > 0.5 |
| jump (press) | Space edge | South edge, same latch |
| crouch (held) | C | East |
| sprint | Shift | L3 press latches sprint until the left stick drops under 0.5 |
| shield (held) | Q | LB |
| melee (press) | E edge | RB edge |
| reload | R | West |
| scoreboard | Tab | Start |

### 6.3 The feel table (client; `crates/arena/src/feel.rs`, new: `WeaponFeel` per id and the event cues)

Per weapon: `kick_cam` (rad), `kick_model` (rad), `push` (m back along the look), `rise` (fraction of the cooldown), `settle_pow`, `yaw_alt` (rad, sign alternating by shot parity), rumble (strong, weak, ms), flash size, tracer (length, thickness, colour), sound.

| id | kick_cam / kick_model / push | rise / settle_pow / yaw_alt | rumble | flash | tracer | sound |
|---|---|---|---|---|---|---|
| 1 Sidearm | 0.012 / 0.16 / 0 | 0.16 / 2 / 0 | 0.20, 0.45, 45 | 0.14 for 45 ms | 0.68 × 0.075 `GLOW_BLUE·0.55`, hot head 0.22 (today's) | `Shot` (existing) |
| 2 Vityaz | 0.006 / 0.09 / 0 | 0.15 / 1 (linear: never settles between rounds) / 0.004 | 0.12, 0.55, 30 | 0.10 for 35 ms | 0.45 × 0.06 yellow-white | `ShotSmg` sweep(0.05, 700→300, sq 0.8, decay 40, noise 0.3) |
| 3 AK-47 | 0.028 / 0.24 / 0 | 0.10 / 1.5 / 0.010 | 0.55, 0.35, 55 | 0.18 for 45 ms | 0.9 × 0.08 orange (1.0, 0.62, 0.2) | `ShotRifle` sweep(0.11, 230→90, sq 0.7, decay 24, noise 0.45) |
| 4 M4 | 0.014 / 0.15 / 0 | 0.12 / 2 / 0 | 0.35, 0.40, 40 | 0.14 for 40 ms | 0.7 × 0.07 white | `ShotRifle` at 0.9 volume |
| 5 Revolver | 0.060 / 0.42 / 0.03 | 0.08 / 3 (snaps back) / 0 | 0.90, 0.30, 90 | 0.26 for 50 ms | 1.0 × 0.11 bright white, head 0.28; cylinder, hammer and trigger move (v15's `PartAnim`) | `ShotRevolver` click sweep(0.012, 2400→1800, sq 1.0, decay 120) then sweep(0.16, 150→55, sq 0.6, decay 16, noise 0.5) |
| 6 Sniper | 0.070 / 0.35 / 0.05 | 0.06 / 1.5 across the full 0.9 s / 0 | 1.00, 0.20, 110 | 0.22 for 40 ms | 1.6 × 0.05 cyan-white: a line, not a streak; ADS is a real scope at FOV 3.5 (20x): 24 opaque black slabs tangent to a 24-gon 0.30 m from the eye leave a round hole with a crossed reticle, the viewmodel is not drawn while scoped, and look sensitivity scales by the field of view (client-only; feel.rs holds the geometry) | `ShotSniper` crack sweep(0.04, 1800→400, sq 0.9, decay 60, noise 0.6) + boom sweep(0.22, 120→45, sq 0.5, decay 12, noise 0.35) |
| 7 RPG-7 | 0.05 / 0.30 / 0.08, plus shake 0.5 at launch | 0.10 / 2 / 0 | 1.0, 0.8, 200 | 0.30 for 60 ms + six grey smoke cubes drifting back | the `w_rpg7_rocket` mesh flown at the `BState` position oriented along its velocity, an orange exhaust rod 0.6 × 0.09 behind it | `Launch` whoosh sweep(0.25, 90→400, sq 0.2, decay 10, noise 0.9) |

**Camera kick**: `recoil(k) = if k < rise { k / rise } else { ((1 − k) / (1 − rise))^settle_pow }` with `k = (time − shot_started) / cooldown` clamped 0..1 (today's 0.16/0.84 curve, per weapon now). Full-autos add an accumulator: `climb += kick_cam · 0.5` per confirmed shot, `climb *= exp(−6 dt)`, so the AK climbs and the Vityaz never rests. The camera basis uses `pitch + kick_cam · recoil + climb` and `yaw + yaw_alt · side · recoil`; the viewmodel uses `weapon_rot(yaw + yaw_alt · side · recoil, pitch + kick_model · recoil)` and `base −= look · push · recoil`. **The sent pitch stays the player's own**: recoil is cosmetic and never moves the server's aim (`recoil_never_reaches_the_wire`: the `C2S::Input.pitch` sent during a burst equals `self.pitch`).

**Shake**: `shake_amp = max(shake_amp, a)` on an event, `shake_amp *= exp(−9 dt)`; per frame `n1 = sin(37.1 t) + 0.5 sin(71.3 t)`, `n2 = sin(41.7 t + 1.3) + 0.5 sin(67.9 t)`; the eye moves by `right · 0.03 · n1 · amp + Y · 0.02 · n2 · amp`, the total offset clamped to 0.08 m so the eye never crosses the 0.1 near plane into a wall; the look direction gets `right · 0.02 · n1 · amp + Y · 0.015 · n2 · amp` (positional and pitch/yaw jitter only: a roll needs a camera up vector, which is a renderer change). **No hit-stop**: the sim is fixed-step and the client never pauses or scales `dt`.

Event cues:

| Event (signal) | Camera | Shake | Rumble | Sound |
|---|---|---|---|---|
| own shot (authoritative ammo decrement, weapon unchanged, alive both sides; as today) | per-weapon kick | RPG 0.5 | per-weapon | per-weapon |
| empty trigger (fire held, `ammo == 0`, not reloading; edge-triggered once per press) | — | — | 0, 0.20, 30 | `Click` sweep(0.025, 900→700, sq 1.0, decay 120, noise 0.3) |
| reload start / end (`reloading` edge) | dip as today over `stats.reload` | — | 0.15, 0.25, 60 then 0.25, 0.15, 50 | `Reload` (existing) |
| holster (my weapon went from a pool gun to 1 with `alive` both sides) | the model drops 0.3 m and comes back over 0.35 s | 0.2 | 0.3, 0.3, 80 | `Holster` sweep(0.05, 600→300, sq 0.6, decay 50, noise 0.4) + sweep(0.08, 380→700, sq 0.2, decay 20) |
| hit (`S2C::Hit { shooter == me }`) | hitmarker 0.14 s (today's); head: the marker 1.5× | — | 0, 0.35, 40; head 0, 0.6, 35 | `Hit` |
| hurt (`S2C::Hit { victim == me }`) | — | 0.35 | 0.6, 0.2, 120 | `Hurt` |
| kill (`Kill { killer == me }`) | kill marker 0.55 s | — | 0.8, 0.8, 180 | `Kill` |
| death (`Kill { victim == me }`) | — | 1.0 decaying over the respawn | 1.0, 0.6, 400 | `Death` |
| self-kill (`Kill { killer == victim == me }`) | as death | 1.0 | as death | `Death` + status "you blew yourself up" |
| bonk (predicted `VStep.bonked` on an armed Loot block) | dip 0.06 m over 80 ms | 0.4 | 0.5, 1.0, 90 | `Bonk` sweep(0.09, 260→820, sine, decay 22, noise 0.05) |
| loot pop (`S2C::Loot`, mine / others within 20 m) | — | — | 0.2, 0.6, 120 / — | `Pop` sweep(0.06, 1480, sq 0.1, decay 30) + sweep(0.14, 1975, sq 0.1, decay 18) at 0.55 / 0.25 |
| blast (`S2C::Blast`), d = distance to my eye | — | (1 − d/14).clamp(0, 1); 1 flash cube 2.2 m for 80 ms, 12 shard cubes at 9 m/s under gravity for 0.35 s, 8 smoke cubes rising for 0.6 s | d < 14: 1.0, 1.0, 350 × (1 − d/14); else 0.3, 0.2, 150 | `Blast` sweep(0.45, 70→28, sq 0.5, decay 7, noise 0.95) + tail sweep(0.30, 40→25, sq 0.2, decay 8, noise 0.6) at `clamp(1 − d/40, 0.15, 0.9)` |

Sounds: `Sfx` grows ten variants (`ShotSmg, ShotRifle, ShotRevolver, ShotSniper, Launch, Blast, Bonk, Pop, Click, Holster`; 18 total), `ALL` follows, the per-frame budget of 6 stays, and queued cues are **sorted by priority** (`Blast, Death, Kill, Pop, Bonk` first) before `take(6)`, so a crowded frame drops a footfall, not the boom. Remote shots pick the shooter's weapon from `BState.weapon` for the cue.

Tests (`feel_tests`): `weapon_feel_table_covers_every_weapon_id`, `recoil_curve_peaks_at_the_rise_and_settles_to_zero` (for each weapon: `recoil(rise) == 1`, `recoil(1) == 0`, monotone after the rise), `shake_decays_to_under_one_percent_within_half_a_second` (`exp(−4.5) = 0.011`), `yaw_kick_alternates_sides`, `sfx_priority_keeps_the_boom_under_the_budget`.

### 6.4 `docs/minimum-requirements.md`, the entry (new §2 bullet, verbatim)

The Gamepad API and its `vibrationActuator` are **not required**. Ember reads `navigator.getGamepads()` when it exists and treats an absent API, an absent actuator (Firefox, Safari) or a rejected `playEffect` as "keyboard and mouse only": every intent has a key, and rumble is a silent no-op. No WebGL capability changes. The arena page reports the probe result (`gamepad: none | input-only | input+rumble`) in its status line once a pad is seen, beside the existing renderer probe, so a player without rumble can tell a missing feature from a broken one.

## 7. The client

- **Weapons by id**: `classify(name)` matches the exact `w_*` names first (`w_vityaz → Weapon(2)`, `w_ak47 → 3`, `w_m4 → 4`, `w_revolver_* → 5` with `PartAnim` from the suffix `_cylinder`/`_hammer`/`_trigger` and the pivot by full node name, `w_sniper → 6`, `w_rpg7 → 7`, `w_rpg7_rocket → Rocket`), then `rifle → Weapon(1)`, then v17's `shield`/`sword`/`hand_sword` and the `arm*`/`hand*` rule. `Assets.weapons: [Vec<Part>; 8]`, `Assets.muzzles: [Vec3; 8]` (falling back to the sidearm's), `Assets.rocket: Vec<Part>`. **A weapon id whose part list is empty draws `weapons[1]` with that weapon's accent**: the M4 path, and every future asset gap; a missing node shows as the wrong rifle, never as an empty hand. Drawn first person at the v16 hold offsets and on every remote player from `PState.weapon`; the rocket part rides the tube while `ammo > 0 && !reloading` and is hidden otherwise, first and third person both. Test `the_viewmodel_nodes_are_sorted_by_name` grows to all fifteen names; `every_table_weapon_has_a_node_or_a_fallback` loads the real GLB and asserts ids 1, 2, 3, 5, 6, 7 are non-empty and 4 falls back.
- **Mesh ids**: the GLB's parts are registered first as today, so `env_base`, `parts_base`, `props_base` shift by the ten new parts and the `set_*` setters absorb it. `Prop::ALL` gains `Loot` at the end (`every_prop_builds_and_lights` covers 19).
- **Tracers and rockets**: the tracer's rod length, thickness and colour come from the feel table by `BState.weapon`; a rocket is the rocket mesh flown along the server's path with the exhaust rod behind it; blasts are the cube burst above.
- **Hit and kill cues** come from `S2C::Hit` and `S2C::Kill`; the "my bullet vanished and an enemy lost hp" heuristic is deleted, which also closes the false hitmarker on a reflected round in the backlog.
- **Map**: `OnlineConfig.map: String` (`serde(default)` → empty → the server's default), passed into `CreateLobby`; the lobby list renders `LobbyInfo.map`.
- `ads` is sent every input frame (RMB or LT). ADS FOV per weapon from the table; the sniper's scope frame at FOV 22.

## 8. The asset pipeline (`tools/v18/`, committed like `tools/v13/`)

### 8.1 The sources, as Blender 5.2 sees them (probed headless on 2026-09-03, 9 s for all four)

Every material's image links are broken or absent (paths on the artist's machine, a `source/ak/` folder that does not exist, a `4k uv grid` placeholder): **every material is built by hand** from the baked picture with v16's `picture_material`, and the build asserts the image datablock decoded.

| Weapon | Source (unpacked, gitignored) | What Blender sees | Frame | Decision |
|---|---|---|---|---|
| AK-47 | `assets/ak47/source/source/AK47.blend` (append its objects; no FBX) | 1 mesh `AK`, 15 843 faces, one material, one UV layer | 8.80 long along **Y**, magazine hangs −Z; ~10 units per metre | `w_ak47`; the 4096 base colour baked to 1024 |
| Vityaz | `assets/vityaz/source/source/pp19 01 vityaz.fbx` | 17 meshes over 9 materials; UV layer names **inconsistent** (`UVMap`, `TEXCOORD_0`, `meshId0-tex0`; `pistol grip` and `receiver` carry all three); unapplied scales (0.203, 0.022) | 1.48 along **Y** with the stock extended (the stock is the +Y end, the muzzle −Y), magazine −Z, sight +Z | delete `Folding Stock`, the two glass planes and `reticle`; normalise UV names (check which layer holds the real UVs per object by coverage); apply transforms; join to one mesh; **bake an atlas**; `w_vityaz` at 0.72 m |
| RPG-7 | `assets/rpg7/source/RPG7/RPG7.fbx` | 5 meshes at scale 0.01 (`RPG7` 15 248 faces, `rocket` 3 326, `hammer`, `sight`, `sight_adjust`), UV layer `UVChannel_1`, materials `RPG7` and `RPG7Rocket` | launcher 0.48 along **Y**, sight +Z; the file lays the rocket BESIDE the launcher at 25° to the bore; about half real size | `w_rpg7` = launcher + hammer + sights joined (picture 1024) and `w_rpg7_rocket` separate (512), the rocket turned onto its own principal axis and seated in the tube (rear 0.43 m inside, warhead 0.37 m out of the muzzle), both with the launcher's origin so the rocket draws in place at the unit transform; 0.95 m. The tube reaches 0.78 m behind the grip, so at the v16 hold it sits beside the eye. |
| Sniper | `assets/sniper/source/source/1.fbx` | `rifle` **83 430 faces with four material slots** (back, SCOPE, front, middle), `rifle.001` 1 514, `mag` 3 254, `bullet` 1 634 (a loose display cartridge: dropped) | 3.26 along **X**, muzzle +X, scope +Z; about 2.7 units per metre | join rifle + rifle.001 + mag; **bake an atlas**; decimate to 15 000 triangles **after** the bake (every imported gun is held to that budget: the AK's 15 843 polygons were n-gons under a Subsurf modifier that `export_apply` turned into 360 000 triangles); `w_sniper` at 1.15 m |
| Revolver | `assets/revolver/revolver.obj` + `baked/M1.png, M2.png` (v15) | 20 named parts, merged to five by `tools/v15/build_viewmodel.py`'s `GROUPS` | v15's fit: 0.75 m, +X, origin on the grip | `tools/v15/build_viewmodel.py` gains an `if __name__ == "__main__"` guard and a `build_revolver()` that returns the five parts, their pivots and the muzzle, without changing its own output (proven byte-identical to commit 81bfe66); in the OBJ the frame group wears M2 and the receiver M1, so `w_revolver_frame` ships M2 at 1024, `w_revolver_receiver` M1 at 512, and cylinder, hammer and trigger share one M2 at 512 |
| M4 | `assets/m4/source/source/M4 Carbine by Umang Rank.rar` | RAR5; the FBX and the base colour are compressed entries (method 51 per `rarfile`), only the normal/roughness/height maps are stored | — | **blocked**: no unrar, 7-Zip or WinRAR on this workstation and bsdtar refuses RAR5. `winget install 7zip.7zip` is the one-command unblock; then `7z x` into `assets/m4/source/`, a `build_m4()` in the builder (one material, like the AK), and `LOOT_POOL` gains 4 on the server. No bump. |

### 8.2 The scripts

1. `tools/v18/loot_texture.py` (PIL, `C:\hy3d\venv\Scripts\python.exe`): the `?` block picture, 512 × 512 8-bit RGB, written and verified; 0.14 s. Done.
2. `tools/v18/prep_pictures.py` (PIL): downscales base colours to the shipping size, RGB, LANCZOS, re-opens to assert mode and size, reports any alpha coverage dropped (the v17 lesson): `assets/ak47/baked/ak47-1024.png`, `assets/rpg7/baked/rpg7-1024.png`, `assets/rpg7/baked/rocket-512.png`, `assets/revolver/baked/M1-512.png`, `M2-512.png` (the four small revolver parts ship at 512). The Vityaz and sniper sets are prepared at 1024 per material as bake **inputs** (`assets/vityaz/baked/<material>-1024.png`, `assets/sniper/baked/<material>-1024.png`) so Blender never touches a 4096 source.
3. `tools/v18/build_weapons.py` (Blender headless; imports `tools/v17/build_viewmodel.py` as a library, which imports v16): `main()` rebuilds the v17 five through `v16.build_operator()`, `v17.build_shield()`, `v17.build_sword()`, `v17.build_fist()`, then per weapon a `build_<w>()` returning `(objects, muzzle, pivots)`; `export(all, muzzles, pivots)`; `verify_glb()`; `--preview` renders side/front/top per weapon to `tools/v18/preview-<w>-*.png`. Per weapon: **delete the studio** (assert the surviving object set by name against §8.1 and fail listing what was found); **normalise UV layer names to `UVMap` before any join** (Path B step 2: a join by name empties the first layer and the model renders flat); unparent keep-world, apply transforms; **measure the frame** (long axis = the largest extent; the muzzle end is the thinner end, found as the end whose 15 % slab has the smaller cross-section; up = the side the sight sits on / opposite the magazine; asserted after the fit: muzzle forward of the origin and within 8 % of the front bound, magazine below the bore — Path D's "a derived fit cannot check itself"); **fit** with v17's `frame_and_fit` to the target length, origin at the top of the pistol grip at the trigger (v16's hold point, so the operator's hands draw at the same offset for every weapon); one `picture_material` per part; smooth; **names are the contract** (mesh data renamed to the node name: the `.001` lesson).
4. **The atlas bake** (Vityaz, sniper), the new step this update adds to Path D: after the join, add a UV layer `atlas` and pack the original islands of every material into it together (`bpy.ops.uv.pack_islands` on all faces with `margin_method='ADD'`, margin 0.002, concave shapes, rotation allowed: these meshes carry thousands of islands, and the per-island fraction margin of 0.02 the draft named left nothing but margin, spilled across a 4×4 grid and zeroed every face; the artist's islands keep their shape and only scale, which beats a fresh smart projection); in every material wire a `ShaderNodeUVMap` set to the ORIGINAL layer into each image texture's Vector so the samples read the original UVs; add a blank 1024 image node, selected and active, to every material; make `atlas` the active UV layer; Cycles, `bake_type='DIFFUSE'`, `use_pass_direct=False`, `use_pass_indirect=False`, `use_pass_color=True`, `margin=4`, 16 samples; then delete the original UV layer, rename `atlas → UVMap`, replace all materials with one `picture_material` on the baked image (packed, `baseColorFactor` white). **Assert the baked image is non-uniform** (the std-dev of a 1 % pixel sample > 8/255): a failed bake is black, silently. Decimate (ratio to the face budget) **after** the bake so the bake sees the artist's density. Fallback in the same script, `--split`: one part per material at 512 (Vityaz 8 parts ≈ 11 MB VRAM instead of 5.6), node names `w_vityaz_<material>` classified by prefix, so the lane is never blocked on Cycles.
5. **Sidecar** `viewmodel-rig.json`: `{"pivots": {"w_revolver_cylinder": [..], "w_revolver_hammer": [..], "w_revolver_trigger": [..]}, "muzzle": [the sidearm's, unchanged], "muzzles": {"w_vityaz": [..], "w_ak47": [..], "w_revolver_frame": [..], "w_sniper": [..], "w_rpg7": [..]}}` — pivot maps before any list (the scanner note in `docs/asset-pipeline.md`; serde reads this file, the convention costs nothing). `muzzles` is `#[serde(default)]` in the client.
6. `verify_glb()`: node AND mesh names equal the exact sorted list `[hand_sword, hands, rifle, shield, sword, w_ak47, w_revolver_cylinder, w_revolver_frame, w_revolver_hammer, w_revolver_receiver, w_revolver_trigger, w_rpg7, w_rpg7_rocket, w_sniper, w_vityaz]`; one primitive each with `TEXCOORD_0`, a material and a base-colour texture; every image `image/png`, re-decoded from the GLB's binary chunk with PIL to assert 8-bit and at most the intended size; `baseColorFactor` white; per weapon the front bound > 0 and the length within 2 % of its target; the rocket's rear inside the tube and its warhead out of the muzzle (no seated rocket can lie inside the tube's x-range, which the draft asked for); the GLB size printed.

### 8.3 Cost, honestly

Texture VRAM is `w × h × 4 × 4/3` per mesh id. Today's viewmodel: 44.7 MB (hands 2048², four parts at 1024²). Measured after the build: the viewmodel set's texture VRAM is **74.8 MB** with mip chains (every weapon is registered because any remote player may hold any gun; `CLAUDE.md`'s 112 MB line is the reference point). Bundle: `viewmodel.glb` was 8.0 MB; the shipped v18 GLB is **17.4 MB** (146 006 triangles, 13 PNGs totalling 11.5 MB) after taking the pre-decided first fallback, the Vityaz and sniper atlases at 768 (at 1024 it measured 18.2 MB), and it is still 1.4 MB over the 16 MB line this document drew; the next levers (the hands' 2048 picture at 2.7 MB, the shield's 1.6 MB, the AK and the revolver frame at 1024, the 15 000-triangle budgets) are a project decision recorded in the backlog, not taken blind. The wasm bundle measured **39.1 MB** after `wasm-bindgen` (`web/pkg/arena_bg.wasm`; 29.6 MB before v18; the "~17 MB" in `docs/asset-pipeline.md` was stale and is corrected there).

## 9. Work packages, verification, commits

### 9.1 Interfaces fixed first (one skeleton commit: compiles, every existing test green)

`arena-core`: `Cover::Loot`; the `WeaponStats` fields, `weapon_stats`/`weapon_name` with the table, `SIDEARM`, `WEAPON_COUNT`, `RESERVE_INFINITE`, `LOOT_POOL`, `Projectile`, `LOOT_SIZE`, `LOOT_RESPAWN_SECS`, `SALT_LOOT`; `hash64`/`roll`/`unit_pair`; `Bullet { weapon, pierce, hit_mask }`; `PlayerSt.reserve`; `PlayerIn.ads`; `VStep` and the new `step_vertical` return with the lowest-base clamp; `Sim::from_level(&Level, u64)`, `Sim.seed`, `Sim.loot`, `Sim.hits/blasts/loot_events`; `LootBlock`; `loot_roll`, `segment_hits_box`/`segment_hits_cover` signatures; `MAP_FREIGHT_YARD`, `Level::named`'s arm, and `freight_yard.rs` returning the centre alone. `proto.rs`: every field and variant in §2.1, `PROTO_VERSION = 14`. `arena-server`: compiles against it (map per lobby, events forwarded, `State.loot`, `ads`). The client and the frozen tests are mechanically updated (tuple → struct at the `step_vertical` call sites; `heavy_kills_in_two_hits` → `the_revolver_kills_in_two_hits`; `pads_upgrade_and_death_resets` → `pads_hand_out_the_same_loot`).

### 9.2 Packages (parallel after the skeleton; owned files never overlap; the repo is the medium, `docs/worker-protocol.md`)

| WP | Owns | Delivers |
|---|---|---|
| **A sim + wire + server** | `crates/arena-core/src/shooter.rs`, `proto.rs`, `crates/arena-server/src/lib.rs`, `examples/wsbot.rs` | §3 and §5 in full with every test named there; the §2.1 tests; wsbot gains `--map` and a `--bonk` mode |
| **B map** | `crates/arena-core/src/freight_yard.rs`, `crates/arena-core/tests/fixtures/trench-city-v18.json` (born as `trench-city-v17.json`, regenerated at the §2.2 bump), the shared `level_helpers` test module | §4 and its fifteen invariants |
| **C engine** | `crates/ember-engine/src/{feedback.rs,input.rs,app.rs,lib.rs}`, its `Cargo.toml` | §6.1 on both platforms, its four tests, the §6.4 doc bullet |
| **D client** | `crates/arena/src/{online.rs,feel.rs,sound.rs,props.rs,lib.rs}`, its `Cargo.toml` | §5.4, §6.2, §6.3, §7 |
| **E assets** | `tools/v18/*`, `tools/v15/build_viewmodel.py` (guard + `build_revolver`), `crates/arena/assets/viewmodel.glb`, `viewmodel-rig.json`, `docs/asset-pipeline.md` (the atlas-bake paragraph, the stale bundle number) | §8, with the wall time of every step |
| **F page + docs + deploy** | `web/games/arena/v18/index.html`, `web/games.json`, `deploy/deploy-pages.sh`, `README.md`, `docs/hosts.md`, `docs/minimum-requirements.md`, `docs/plans/backlog.md`, `CLAUDE.md` (the clamp reports its box) | the page with the map selector, the gamepad hint, the `emberRumble` shim and the lobby-row map pill; games.json `v18` live on proto 14 with v17 archived; `ARENA_LIVE=games/arena/v18` |

Ordering: this plan → the skeleton → A, B, C, E, F in parallel (B needs only the skeleton; C and E need nothing from the tree) → D (starts on the skeleton and rebases on A's numbers, C's `Feedback` and E's node names) → integration, review, staging.

### 9.3 Commands (every run reports wall time; builds at idle priority: on this Windows workstation `nice` is a no-op, so `Start-Process … -PriorityClass Idle` or `cmd /c start /low /b /wait`)

- `cargo check -p arena-core -p arena -p arena-server --tests` (9 s warm) after every edit; `cargo test -p arena-core -p arena` (87 tests green today, 9 s); `cargo test -p ember-engine`; `cargo test -p arena-server`.
- `cargo clippy --workspace --all-targets` (deny-warnings; pedantic; `unsafe_code = deny`: gilrs and `Reflect` need none).
- `cargo build --target wasm32-unknown-unknown --release -p arena --lib && wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm`, then the size of `web/pkg/arena_bg.wasm`.
- Assets: `C:\hy3d\venv\Scripts\python.exe tools/v18/prep_pictures.py`; `"C:\Program Files\Blender Foundation\Blender 5.2\blender.exe" --background --python tools/v18/build_weapons.py -- --preview`.

### 9.4 Verification, in order, each recorded in the commit that closes it

1. Unit: everything above green; the determinism test and the frozen Trench City fixture (`trench-city-v18.json`, the map with its blocks) pass.
2. `verify_glb` output and the previews reviewed by eye; `every_table_weapon_has_a_node_or_a_fallback` green.
3. Native two-client capture (v18 record — the input half of this is now FORBIDDEN: nothing may click, type or move the cursor on this machine, and a client drives itself from `EMBER_SCRIPT` instead; see CLAUDE.md's working rules and `crates/arena/src/script.rs`) (a local `arena-server` on 127.0.0.1:7778, two `arena-app` clients, `EMBER_CAM` on the observer and a click to focus the second client — the v17 lesson; `keybd_event` works, synthetic clicks do not, so the harness is rewritten under `tools/v18/` because nothing of it is committed today): each weapon first person and on the other client, a bonk with the pop, a rocket in flight and its blast on both clients, the sniper scope. Screenshots into `tools/v18/ingame-*.png`, paths in the commit.
4. Gamepad: no pad is attached to this workstation (gilrs listed none on 2026-09-03, and the engine's ignored host probe printed `0 pad(s)`), so the rumble and input paths are verified by compile and by their unit tests only, and the commit says exactly that; a pad plugged in later exercises them without a code change.
5. wsbot: two bots for 60 s on each map with `--bonk`; no panics, states flowing, `Loot` and `Blast` events counted > 0, outbound bytes per client reported against today's.
6. wasm bundle size reported; the v18 page opened in a real browser (the embedded pane has no WebGL2: v14's lesson).
7. Deploy is **not** part of v18's definition of done: the live host is this workstation on proto 13 and moving it (`deploy/deploy-arena-local.sh up` before `deploy/deploy-pages.sh`) is the user's separate decision. The page is staged on the branch and `games.json` carries v18 on `proto: 15` (§2.2), exactly as v13 was staged before its server moved.

### 9.5 Commit plan (branch `lane/arena-v18`, one topic each; every message states what was built, what was run, and what was only read)

1. `docs: arena v18 plan — Freight Yard, seven guns, loot blocks, the feel pass` (this document, plus the loot texture and its script).
2. `arena v18: the contracts` (§9.1).
3. `arena-core: the weapon table, the reserve, spread and gravity`.
4. `arena-core: pierce, rockets and splash with line of sight`.
5. `arena-core: loot blocks — the bonk names its box, the roll, the grant, pads unified`.
6. `arena-core: Freight Yard`.
7. `proto/server: v14 — a map per lobby, ads, hit, blast and loot events`.
8. `ember-engine: feedback and gamepads — gilrs on native, the Gamepad API on the web, rumble through the page shim`.
9. `tools/v15: an importable revolver builder`.
10. `tools/v18: the weapons build, the atlas bake, viewmodel.glb`.
11. `arena: the held gun by id, first person and on every player; rockets; the blocks`.
12. `arena: the feel pass — recoil curves, shake, rumble, ten sounds, gamepad play`.
13. `arena v18: stage the Freight Yard page and record it in the docs`.

## 10. Risks, and what is not done

- **The atlas bake fails silently** (black or uniform): the build asserts a non-uniform sample; `--split` ships per-material parts at 512 so the lane is never blocked on Cycles.
- **The M4 stays blocked**: its row ships, its node does not, `LOOT_POOL` excludes it, the client draws the sidearm mesh for it; the unblock is one tool install and one server constant, no bump. Backlog line.
- **Block-top perches** (§5.1): accepted, placed away from spawns, pinned; if play proves them wrong the fix is a sim rule and a bump — backlog line, not done blind.
- **Bundle and VRAM growth** (§8.3): +7 MB on 31, +36 MB VRAM; the 768 fallback is pre-decided; the loot picture is procedural so no generator is in the critical path.
- **Sniper pierce, the shield and lag compensation on one tick**: the body loop continues after a pierced hit, the shield is tested inside the body branch, and the determinism test runs the whole weapon set through the sweep; tick-order bugs are the class that reading does not catch.
- **Splash line of sight over-blocks** where the exact slab test disagrees with the conservative span test: a target on a crate whose chest is above the crate top is clear; a target crouched behind 0.7 m of rubble with the blast at floor level is blocked. Intended; test 2 of the rocket set covers the corner.
- **Predicted bonk versus authoritative loot**: the bump plays, no pop; documented in §5.4.
- **Gamepad platform quirks**: gilrs on Windows is Windows.Gaming.Input (WGI: rumble only on pads it classes as a Gamepad, no DirectInput force feedback; a DualShock without a driver has no rumble); browsers expose a pad only after a button press; Firefox and Safari have no actuator. All degrade to keyboard and mouse; the status line names the probe result; native Linux is untested and said so.
- **The freight yard numbers are argued, not played**: every sightline is decided by the test; one iteration of moving a box is budgeted; the knobs that move first are Q2's length, the sandbag positions and `LOOT_RESPAWN_SECS`.
- **Deploying proto 15** (14 was never deployed; §2.2 moved it before the host did) freezes v13–v17 list-only and needs the local host rebuilt before the pages deploy; the specht ssh stall (memory) means this workstation is the only host that can move, and that is the user's call.
- Not in v18, listed for the backlog: a roof-only reward beyond the two blocks over C2; the sniper's bolt-cycle keyframe and the landing dip; remote melee and remote bonk animation on `PState`; a `copy_pkg` that carries wasm-bindgen snippets; the M4.
