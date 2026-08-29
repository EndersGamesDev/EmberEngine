# Gate report — branch `9mm-vertical-shot`

## Re-run @ 1921fb1 — GREEN, branch passes

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
