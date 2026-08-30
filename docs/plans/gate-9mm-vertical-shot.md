# Gate report — branch `9mm-vertical-shot`

## Run @ facedcf — GREEN (current): `ember-editor` lands

| Check | Result |
|---|---|
| `cargo test -p ember-editor` | **18 passed, 0 failed** |
| `cargo build --workspace --all-targets` | clean — the new member registers |
| `cargo clippy --workspace --all-targets` | no errors; one cosmetic warning in the crate (see below) |
| Visual: `ember-editor-app` | axes render **distinctly red/green/blue** |

`the_ray_agrees_with_unprojecting_the_view_projection` **passes**: the
hand-built basis ray and `camera.view_proj(aspect).inverse()` agree at
three non-16:9 aspects, so neither is wrong and the later bites can build
on the basis form. That test gives presenter-architecture.md's oracle O3
its first consumer.

**The axes were measured, not eyeballed** — brightest pixel per bar, and
the ratio of its own channel to the next highest:

| Axis | Sampled | Ratio |
|---|---|---|
| +X red | rgb(229, 32, 35) | 6.5× |
| +Y green | rgb(32, 222, 49) | 4.5× |
| +Z blue | rgb(30, 89, 226) | 2.5× |

The over-drive constant (4.0) is correct. Note for anyone tuning it
*down*: blue has the least headroom because the scene shader's cool fill
light (`fill_col` 0.42/0.50/0.68) lifts the green channel of anything
blue. Below roughly 2.5, re-measure rather than reason.

Outstanding, cosmetic: `crates/ember-editor/src/lib.rs:356` assigns
fields after `Default::default()` where clippy wants struct-update
syntax. In a test; author's call.

## Re-run @ 80aa8b1 — GREEN

Re-run rather than carrying the previous green forward: the diff was not
docs-only, because the branch merged main and brought `rig.rs` with it.
Merges are where a green stops transferring.

`cargo test --workspace`: no failures — pong-core 31, ember-server e2e
11, ember-engine 9 (including `rig_json_joints_resolve_whatever_the_key_order`,
confirming the merge kept the `parse_joint` fix rather than resolving it
away). Merging to main is Ender's call.

### `PROTO_VERSION` — decided: bump to 8

Ender ruled to bump; the branch already carries `PROTO_VERSION = 8`, main
is still 7, so the bump lands with the merge and takes jump with it.

The deploy coupling this creates is now **self-detecting**: `server.json`
records the protocol the bundle speaks and `deploy-pages.sh` prints a
loud block when it changes, naming the exact error players see and that
`pong-server` must be redeployed in the same window (27c20d6).

Two corrections to the earlier framing in this file, both verified in the
source rather than assumed:

- The mismatch is **not silent**. `pong-server` sends `S2C::Error` —
  "this build speaks protocol vN, the live game is vM — play the live
  version" (lib.rs:628, :710) — and the client shows it (online.rs:898).
  Archived pages fail with an accurate sentence, not a mystery.
- The window is **two-sided**: page-first and server-first each leave one
  side unable to join until the other moves. There is no zero-downtime
  order without dual-version support in the server, which is not worth
  building here. Move both together.

## Re-run @ 1921fb1 — GREEN

dev-a1 fixed the failure below by giving the shooter headroom
(`players[0].y = 8.0`) rather than dropping the clamp angles, keeping
coverage at ±`MAX_PITCH` where a `tan()` bug would hide, and added a
floor-cull test with a negative control.

| Check | Result |
|---|---|
| `cargo test -p pong-core` | **31 passed, 0 failed** |
| `cargo test --workspace` | no failures; ember-server e2e 11 passed |
| `cargo check -p pong --target wasm32-unknown-unknown` | clean |

Still open for the merge: the branch edits `docs/asset-pipeline.md`,
which main has also changed — take dev-a1's Path D and conventions
sections, keep A/B/C from main. `PROTO_VERSION` is an open decision for
Ender (see below).

---

## Original run @ 7a66493

Run by dev-a5 on 2026-08-29 at dev-a1's request, on a workstation with
the full Rust toolchain and Blender 5.2. **That box is the build lane —
gating does not need adler or specht provisioning.**

## Result: one real failure, everything else green

| Check | Result |
|---|---|
| `cargo test --workspace` | 1 failure (below); all other suites pass, including the new ember-server e2e: **11 passed** |
| `cargo check -p pong --target wasm32-unknown-unknown` | clean |
| `cargo clippy --workspace --all-targets` | no errors; the one warning (unused `mut`, ember-engine renderer) is pre-existing on main |
| `blender --background --python tools/9mm_convert.py` | loads clean on Blender 5.2, dies correctly on the missing source |

### `shooter::tests::pitch_does_not_shorten_a_shot` — a test bug, not a sim bug

```
panicked at crates/pong-core/src/shooter.rs:1472: "a shot must spawn a bullet"
```

The failing iteration is `pitch = -MAX_PITCH`. `MAX_PITCH` is 1.45 rad, so
`tan(1.45) = 8.238` and `vy = -280` units/s. In the single tick the test
runs, the bullet falls 4.67 from `EYE_STAND` 1.45 to −3.2, and the floor
check at `shooter.rs:723` (`b.y < 0.0 → return false`) culls it *inside
the same step it spawned*, because `self.bullets.extend(new_bullets)`
(:611) runs before the sweep (:651). `sim.bullets` is therefore empty and
`.first()` unwraps `None`.

The behaviour is correct — firing at your feet should hit the floor
immediately. Fix the assertion:

- drop `±MAX_PITCH` from that loop (the property being pinned — horizontal
  speed stays `BULLET_SPEED` — is angle-independent), or
- restrict to pitches whose first-tick drop stays above ground
  (`|tan(pitch)| * BULLET_SPEED * FIXED_DT < eye height`, i.e. `|tan| < ~2.5`), or
- spawn the shooter on a crate so there is headroom below.

Worth adding: a test that a straight-down shot **is** culled by the floor.

## Interaction with main

The branch's merge-base is `daa2351` (current main), so it already
contains the vertical axis landed earlier the same day — `PlayerSt.y/vy`,
`PlayerIn.jump`, `step_vertical()`, and a height-aware `blocked()` where
crates (0.9–1.5) are jumpable and containers (2.4+) are not. Nothing to
rebase. The branch's bullet-height design (height rides alongside the 2D
path so `vel` keeps its full `BULLET_SPEED`) composes cleanly with it.

**`PROTO_VERSION`: an open decision for Ender.** `PState.y` and
`Input.jump` shipped as `#[serde(default)]` with the version left at 7,
so the frozen hub pages keep working and jumping activates when the
server redeploys. dev-a1 argues pitch cannot ship the same way, and on
review that argument is right: jump is purely additive (an old client
plays a smaller game, every existing interaction still resolves the
same), while pitch gives the hit volume a finite top and therefore
changes what an *existing* interaction means — the same level shot that
used to connect now legitimately misses. `serde(default)` protects the
wire format, not the semantics.

dev-a5's recommendation: **bump to 8 when pitch lands and let it carry
jump**, so both vertical changes ship as one coherent version rather
than one versioned and one smuggled. The cost is real but bounded — the
archived pages (already marked not-live in `games.json`) stop being
joinable; the server accepts any version for Hello/ListLobbies but
requires equality to create or join. The sequencing constraint matters
more: the server and the live page must move together, and the server
redeploy has been blocked on account issues.

Also note arena **v10** is live (`web/games/arena/v10`, `games.json`,
`deploy-pages.sh`); rebase before touching those.

## Open request back to dev-a1

`assets/9mm/` is not in git and not reachable from this box. Push it on a
branch (17 MB is fine for plain git) or drop it on adler and say where;
the conversion, verification and in-game screenshots can then be turned
around here. The converter expects
`assets/9mm/source/0ae7c8526de44d0ab63e6b5d21341fd2.fbx.fbx`.

Pipeline questions are answered in [asset-pipeline.md](../asset-pipeline.md).
