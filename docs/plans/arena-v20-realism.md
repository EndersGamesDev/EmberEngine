# Arena v20 — the realism pass: ballistics, tracers, impacts, sound

The immersion update. Rounds fly at their real muzzle velocities and are stopped where they actually meet cover; what the player sees of a round is a tracer streak, a muzzle plume, a casing and an impact that depends on what was hit, and what the player hears is a layered gunshot that arrives late from far away, low-passed by the distance and panned to where it came from, a supersonic crack when a round passes close, an impact that sounds like the material, a casing on the cobbles, and a reload that belongs to the gun in hand. AAA is the direction; this document says exactly how far this renderer and this audio path can carry it and where the next tier starts.

Written before the code, in the shape of the v18 and v19 plans. Every constraint below was verified in the tree on 2026-09-04.

## 1. What is being asked, in this engine's terms

| Ask | What it means here |
|---|---|
| "bullets are not the right speed for weapons" | Rounds fly at 34 to 60 m/s today because the sweep tests cover at the tick's END point and samples the head band in at most 32 steps; a round at 900 m/s crosses 15 m per tick and would tunnel through a 0.4 m wall and skip a 0.30 m head. v20 makes both tests exact on the segment (entry parameter against the box slab, analytic overlap of the head band with the round's height over the segment) and sets every speed to the round's real muzzle velocity (§3). |
| "not the right shape" | A round is a rod 0.45 to 1.6 m long drawn wherever the last 30 Hz state put it. A round that lives four ticks never appears in a state at all. v20 draws from **shot events**: the server reports each round's origin, end point and what it hit the tick it ends, and the client draws a tracer streak along that line whose bright head travels at the real speed and whose tail fades in 120 ms, a muzzle plume, a casing, and an impact effect and mark by material (§5). The rocket keeps its mesh in flight. |
| "not the right sound" | Every gun has one 50 to 160 ms sweep today, played at one volume. v20 builds a small synthesis kit (noise bursts, one-pole filters, envelopes, layering, a tail) and gives every gun a layered shot (mechanism, blast, body, tail) in three distance variants, played late by the speed of sound, panned to the shooter's bearing, with a supersonic crack for a near miss, material impacts, ricochets, casings and per-gun reloads (§6). Recorded samples would be the next tier; the slot for them is defined and empty (§6.5). |
| "AAA is the goal" | What this pass can reach: correct ballistics, event-driven tracers and impacts, layered spatial audio, persistent impact marks. What it cannot, and says so: no transparency or additive blending (every effect is opaque geometry), one texture per mesh (no decal atlas), no recorded sound. Those are renderer and asset decisions recorded in the backlog, not promises. |

## 2. Why this is a protocol bump (16 → 17)

A v16 client draws rounds from `BState` alone; against a v17 server it sees almost nothing fly and hears shots only when a state happens to catch a round in flight, while the server's rounds cross the map in three ticks. `S2C::Shot` is a new variant an old peer drops. `BState` keeps its shape. That is "plays a different game", so `PROTO_VERSION` goes to 17 and the frozen v19 page goes list-only against a v17 host, as at every bump.

### 2.1 Every wire change

| Message / field | Change | serde | What an old peer does |
|---|---|---|---|
| `S2C::Shot { owner, weapon, x0, y0, z0, x1, y1, z1, hit, cover, victim }` | new variant, one per round the tick it ends (`hit`: 0 expired, 1 cover, 2 body, 3 shield, 4 floor, 5 arena wall; `cover`: the `Cover` kind's index when `hit == 1`, else 255; `victim`: the id when `hit == 2` or `3`, else 255) | — | dropped |
| `BState` | unchanged; rounds now rarely appear in a state; the rocket always does | — | draws what it gets |
| `WEAPONS` speeds, ranges, gravities | table values | not on the wire | flies the old ballistics on an old server, which the gate prevents |

Tests in `proto.rs`: `json_roundtrip` grows the variant; `a_shot_event_survives_the_codec`; `a_v16_peer_is_why_this_bumps` (documentary).

## 3. The sim

All in `crates/arena-core/src/shooter.rs`. Bullets stay server-side only; the trig at launch stays safe for that reason.

### 3.1 The table (only the columns that change)

| id | name | speed m/s (real) | range m → ttl s | gravity | notes |
|---|---|---|---|---|---|
| 1 | Sidearm (KSVR, .45 ACP) | 280 | 60 → 0.214 | 0 | ttl/cooldown 1.19: the cap is no longer near |
| 2 | Vityaz (9×19) | 380 | 40 → 0.105 | 0 | |
| 3 | AK-47 (7.62×39) | 715 | 80 → 0.112 | 0 | |
| 4 | M4 (5.56×45) | 880 | 80 → 0.091 | 0 | no mesh, as before |
| 5 | Revolver (.454) | 450 | 60 → 0.133 | −9.81 | the real drop at 60 m is 0.09 m: the "lead up" identity goes, realism was asked for |
| 6 | Sniper (.338) | 900 | 120 → 0.133 | −9.81 | pierce 1 unchanged |
| 7 | RPG-7 | 120, then the sustainer: +180 m/s over the first 0.5 s to 300 (`accel` column, applied to the horizontal speed each tick until the cap) | 5.0 | −3.0 | visible in flight as today |

`BULLET_SPEED` and `BULLET_TTL` become the sidearm's new numbers; `pitch_does_not_shorten_a_shot` and the tests that pin 34 m/s are re-pinned to 280 (the expressions stay the same; the constants move). Every test that walks a round tick by tick (the shield duels, the headshot band tests, the tunnel and roof tests) keeps passing because the geometry of a hit does not depend on the speed once the tests below are exact; where a test placed a target by tick count it is rewritten to place it by distance.

### 3.2 Exact tests on the segment

- **World**: `segment_box_entry(a, b, o) -> Option<f32>` (the slab test's entry parameter, `segment_hits_box` becomes `is_some()`), and the per-tick world pass takes the smallest entry over all obstacles, the floor crossing and the arena wall crossing, ends the round at that point, and records the hit. A round is stopped by the FIRST thing on its segment, never by the box under its end point. Rockets detonate there. Test `a_fast_round_does_not_tunnel_through_a_wall` (a 900 m/s round fired at the 0.4 m trench wall from 3 m: stopped, `Shot.hit == 1`, the end point on the near face within 0.05) and `the_first_box_on_the_segment_wins`.
- **Body**: the closest-approach test stays; the head band becomes exact: with the segment's overlap interval `[t_in, t_out]` on the target's circle (solve the quadratic) and the round's height `y(t)` linear in `t`, the round is a head hit iff `[y(t_in), y(t_out)]` (ordered) intersects `[head_lo, hi]`, a body hit iff it intersects `[lo, hi]`; the contact parameter is `t_in`. The 32-sample walk is deleted. Tests `a_steep_headshot_still_registers` on every weapon including the sniper at 900 m/s (this is the test that a sampled band could not pass), `a_head_hit_is_found_wherever_along_the_tick_it_happens`, and the existing headshot tests unchanged.
- **Cover at contact**: `segment_hits_cover(p0 at y0, contact at cy)` replaces the span test at contact.
- **Pierce, shields, splash, lag compensation**: unchanged in rule; a pierced round continues along the same segment, so two bodies 10 m apart on one 15 m sniper segment are both hit in one tick (`two_bodies_on_one_segment_are_both_hit` re-pinned at that spacing).
- **Rocket sustainer**: `speed = min(speed + accel * dt, speed_max)` on the horizontal velocity while keeping its direction; `vy` keeps gravity. Test `the_rocket_reaches_its_sustainer_speed_in_half_a_second`.

### 3.3 Shot events

`Sim.shots: Vec<ShotEvent>` cleared each step, one entry pushed wherever a round ends (expiry, world, floor, wall, body without pierce, shield reflect, rocket detonation). `ShotEvent { owner, weapon, from: [f32; 3] (the muzzle at launch, carried on the Bullet), to: [f32; 3], hit: u8, cover: u8, victim: u8 }`. A reflected round ends its first event at the plate (`hit == 3`) and starts a new `from` there; a pierced round records one event per body it passes (`hit == 2`) and one at its end. Test `every_round_ends_in_exactly_one_shot_event_per_segment`.

## 4. The server

Forwards `sim.shots` as `S2C::Shot` beside the other events. wsbot counts `shots_seen`.

## 5. The client (`crates/arena/src/online.rs`, `feel.rs`, `props.rs`)

Everything opaque; nothing on the wire beyond §2.

- **Tracers from shots**: on `S2C::Shot` push a `Tracer { from, to, weapon, born }`; each frame draw a bright core rod (0.03 thick, the weapon's colour at full) from the head back `min(2.5 m, head progress)` where the head position is `from + dir * min(speed * (now − born), len)`, plus a dimmer tail rod (0.06 thick, colour × 0.45) of `min(len, 8 m)` behind the head; the whole thing lives `len / speed + 0.12 s` and fades by shrinking thickness over its last 120 ms. A round that ended in a body or shield shows the same streak up to the contact. Own shots use the muzzle from the viewmodel for `from` (the sim's origin is the eye height 0.2 ahead; the viewmodel muzzle reads better, and the end point is the server's). `BState` rounds (the rocket) are drawn as today.
- **Muzzle plume**: at every shot's `from` (own and remote), four grey cubes (0.10, drifting 1.2 m/s along the aim and rising, 0.25 s) beside the existing flash cube; remote muzzle flashes come from `Shot`, so a remote shot no longer waits for a state to carry a round.
- **Casings**: every bullet weapon ejects one 0.02 × 0.02 × 0.05 brass cube to the right and up from the muzzle on an own shot, falling under gravity, 0.6 s, with `Sfx::Casing` when it lands (floor only, delayed by the fall).
- **Impacts by material** at `to` when `hit ∈ {1, 4, 5}`: metal (`Container`, `Wall` when the picture is the city wall, arena wall) → 8 white-yellow sparks (0.04, 6 m/s, gravity, 0.25 s) and `Sfx::ImpactMetal`; stone/floor (`Plinth`, `Rubble`, `Cobble` floor, `Roof`) → 6 grey-brown dust cubes (0.12, rising 0.8 m/s, 0.5 s) and `Sfx::ImpactStone`; wood (`Crate`, `Ammo`) → 6 tan splinters and `Sfx::ImpactWood`; sandbag → 5 sand puffs and `Sfx::ImpactSand`; body (`hit == 2`) → the existing flash and red-brown sparks plus `Sfx::ImpactBody`; shield → the existing reflect cue plus sparks. A **mark**: a 0.10 × 0.10 × 0.01 near-black box on the hit surface (offset 0.006 along the surface normal, the normal from which box face the entry parameter met, or +Y on the floor), kept 20 s, at most 96 marks (oldest dropped). One deterministic ricochet in eight on metal (`hash(to)`), which is only a sound and a second short spark cone.
- **Supersonic crack**: when a `Shot` segment passes within 3 m of my eye and the weapon's speed is above 343 m/s, `Sfx::Crack` at a volume by closeness, no delay (it arrives with the round).
- **Distance and bearing**: every remote sound is played through `Audio::play_spatial(sfx, vol, pan, delay)` where `delay = distance / 343` and `pan = right3 · direction_to_source` (−1..1); the shot's distance variant is chosen at 0..12 m near, 12..40 m mid, beyond far. Own shots: near variant, no delay, centre.
- **Tests**: `a_tracer_head_travels_at_the_weapons_speed`, `an_impact_picks_the_material_cue`, `a_near_miss_cracks_only_above_the_speed_of_sound`, `marks_are_capped`, `spatial_pan_and_delay_follow_the_source`.

## 6. Sound (`crates/arena/src/sound.rs`)

### 6.1 The kit

Pure functions on `Vec<f32>` at 44.1 kHz, deterministic (the noise LCG is seeded per cue): `noise(dur)`, `sine(f, dur)`, `envelope(attack, hold, decay, curve)`, `lowpass(buf, cutoff)` and `highpass` (one-pole, run twice for 12 dB/oct), `bandpass(buf, centre, q)` (a biquad), `mix(&[(buf, gain)])`, `delay_tail(buf, secs, feedback, cutoff)` (a comb with a low-pass in the loop: the reverberant tail of a yard between containers), `soft_clip(buf, drive)`, `normalize(buf, peak)`. Tests: `every_kit_stage_is_finite_and_bounded`, `lowpass_removes_the_top_octave`.

### 6.2 A gunshot

`gunshot(p: &GunParams) -> Vec<f32>` layered: **mechanism** (a 3 ms click: high-passed noise at 2 kHz, sharp), **blast** (8 to 20 ms of noise band-passed around `p.blast_hz` with a 0.3 ms attack, the loudest layer), **body** (a sine sweep `p.body_hz` down half an octave over `p.body_ms`, decaying), **tail** (the blast fed through `delay_tail` for `p.tail_ms`, low-passed at 2.5 kHz), then soft-clipped and normalised. Per weapon:

| id | blast_hz | body_hz | body_ms | tail_ms | character |
|---|---|---|---|---|---|
| 1 Sidearm | 1400 | 140 | 90 | 260 | a flat, snappy .45 |
| 2 Vityaz | 1800 | 160 | 60 | 200 | quick and dry, the SMG chatter |
| 3 AK-47 | 900 | 110 | 140 | 420 | the heavy intermediate crack with a long yard tail |
| 4 M4 | 1600 | 130 | 100 | 340 | sharper than the AK |
| 5 Revolver | 700 | 90 | 200 | 520 | the deepest, longest |
| 6 Sniper | 500 | 70 | 260 | 700 | a boom with a whipcrack layered (a 2 ms 4 kHz click 8 ms after the blast) |
| 7 RPG-7 | 300 | 60 | 400 | 900 | the launch whoosh (noise band-passed 300 Hz sweeping up over 250 ms) over a low body |

Three variants per gun: **near** as above; **mid** low-passed at 3 kHz with the mechanism removed and the tail +6 dB; **far** low-passed at 900 Hz, the blast halved, the tail dominant. 21 shot buffers. Test `every_gunshot_has_a_sharper_attack_than_its_tail` (the peak sits in the first 25 ms; the far variant's spectral centroid is below the near one's, measured by a zero-crossing count).

### 6.3 The rest

`Crack` (a 1.5 ms click then 20 ms of high-passed noise decaying: the shock cone), `ImpactMetal` (a 4 kHz ring with two partials, 120 ms, plus a click), `ImpactStone` (band-passed noise 800 Hz, 60 ms, dry), `ImpactWood` (400 Hz thump plus a 2 kHz crack, 80 ms), `ImpactSand` (low-passed noise thud 200 Hz, 70 ms), `ImpactBody` (a wet low thud 150 Hz, 90 ms), `Ricochet` (a descending whine 3 kHz → 800 Hz over 220 ms with a click), `Casing` (a 5 kHz tink with two bounces 90 ms apart, quieter each), per-gun reloads (`ReloadPistol`: mag out click, mag in thunk, slide; `ReloadRifle`: mag out, mag in, bolt; `ReloadRevolver`: cylinder out, six rounds dropped as fast clicks, cylinder in; `ReloadSniper`: bolt back, forward; `ReloadRpg`: a hollow tube slide), and the v18 cues stay. `Sfx` grows to about 45 variants; `ALL` follows; the budget of 6 per frame stays and the priority order puts `Blast, Death, Kill, Crack, ImpactBody` first.

### 6.4 Playback

`Audio::play_spatial(sfx, vol, pan, delay_secs)`: native, rodio 0.19's `SamplesBuffer` through `.delay(Duration)` and a two-channel buffer built by the pan law (`left = cos(θ)`, `right = sin(θ)`, `θ = (pan + 1) * π/4`); web, an `AudioBufferSourceNode` into a `StereoPannerNode` (feature `StereoPannerNode` added) into the gain, started at `ctx.current_time() + delay`. `play(sfx, vol)` stays as centre, no delay. The per-frame budget counts spatial plays too.

### 6.5 The slot for recorded samples

`Sfx` cues are looked up through one `fn source(sfx) -> Cow<[f32]>`; a recorded sample dropped into `crates/arena/assets/sfx/<name>.wav` (16-bit mono 44.1 kHz, `include_bytes!`, decoded by a tiny RIFF reader, no new dependency) replaces the synthesised one at build time when present, else the synth is used. Nothing is shipped in that folder in v20; the reader and its test (`a_wav_in_the_slot_replaces_the_synth`) are. That is the next tier and it needs licensed recordings this repo does not have.

## 7. Work packages, verification, commits

| WP | Owns | Delivers |
|---|---|---|
| **A sim + wire + server** | `crates/arena-core/src/{shooter.rs,proto.rs,freight_yard.rs}`, `crates/arena-server/**` | §3, §2.1, §4 with every test named |
| **B client visuals** | `crates/arena/src/online.rs`, `feel.rs`, `props.rs` | §5 (tracers, plume, casings, impacts, marks, crack, spatial cue routing) |
| **C sound** | `crates/arena/src/sound.rs`, `crates/arena/Cargo.toml` (web-sys feature), `crates/arena/assets/sfx/` (empty, a README) | §6 |
| **D page + docs** | `web/games/arena/v20/`, `web/games.json`, `web/index.html`, `deploy/deploy-pages.sh`, `README.md`, `docs/plans/backlog.md`, `docs/asset-pipeline.md` (the sfx slot paragraph) | v20 live on proto 17, v19 archived; the hint; backlog lines for the next tier (blending for glow and smoke, a decal atlas, recorded samples, hit reactions on remote bodies) |

A's skeleton (types, events, proto 17, speeds) lands first; B, C, D in parallel on it; then integration, `cargo test --workspace --exclude linter`, clippy, the wasm check, bots with `shots_seen > 0` on both maps, captures through the harness (a tracer streak 30 ms after a shot, an impact mark on a container, casings), a sound check by plotting every cue's envelope and spectral centroid to a PNG (no ear here), commits (sim+server; client; sound; page+docs), then the host and pages deploy.

## 8. Not done, for the backlog

Additive blending for tracer glow, muzzle flash and smoke (a renderer change: a second pipeline with `BlendState::ADDITIVE` and a depth-read-only pass); a decal atlas so marks carry a picture; recorded samples in the §6.5 slot; hit reactions and death animations on remote bodies; bullet penetration through thin cover with damage falloff; wind and long-range drop tables for the sniper beyond 120 m.
