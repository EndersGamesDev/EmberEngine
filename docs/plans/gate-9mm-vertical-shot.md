# Gate report — branch `9mm-vertical-shot` @ 7a66493

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

**Keep the protocol backward-compatible.** `PState.y` and `Input.jump` are
`#[serde(default)]` and `PROTO_VERSION` deliberately stayed at 7, so the
frozen hub pages (v7/v8/v9) keep working and jumping activates when the
server redeploys. A version bump locks every published page out of the
live server.

Also note arena **v10** is live (`web/games/arena/v10`, `games.json`,
`deploy-pages.sh`); rebase before touching those.

## Open request back to dev-a1

`assets/9mm/` is not in git and not reachable from this box. Push it on a
branch (17 MB is fine for plain git) or drop it on adler and say where;
the conversion, verification and in-game screenshots can then be turned
around here. The converter expects
`assets/9mm/source/0ae7c8526de44d0ab63e6b5d21341fd2.fbx.fbx`.

Pipeline questions are answered in [asset-pipeline.md](../asset-pipeline.md).
