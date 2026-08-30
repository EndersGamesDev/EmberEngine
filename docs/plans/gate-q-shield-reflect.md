# Gate report — branch `lane/q-shield-reflect`

The off-hand shield: held on Q, and it does not eat a bullet — it sends it back.

## Run @ ccbe293 — GREEN

Run in the `claude-ember` WSL toolbox, `CARGO_TARGET_DIR=$HOME/targets/ember-shield`, every cargo call at `nice -n19`.

| Gate | Result | Wall | Budget @ ea7dc40 |
|---|---|---|---|
| `cargo test --workspace` | **145 passed, 0 failed** | 68.9 s | 69 s |
| `cargo clippy --workspace --all-targets` | no errors, **no warnings** | 12.8 s | 20 s |
| `cargo check -p pong --target wasm32-unknown-unknown` | clean | 40.8 s | 44 s |

All three inside budget. The wasm check still emits the one pre-existing `unused_mut` in `ember-engine/src/renderer.rs:390`, which is on main and is not on the native clippy path.

`pong-core` went 34 → 41 tests: six on the sim's shield and one on the codec. `pong-server`'s three e2e stayed three, with `old_proto_may_list_but_not_join` strengthened rather than replaced.

## What is NOT verified

**Nobody has seen the shield.** The client needs a display and a server speaking v9; this lane had neither. Both draws — the first-person swing and the third-person plate — were derived from the existing viewmodel's own numbers and reviewed by reading. Treat the geometry as unproven until someone looks at it.

Reading did catch one thing execution could not: the third-person plate was first placed 0.34 forward and 0.30 to the side of a body box that is 1.0 across, which buries it inside the torso. It now reaches 0.52 forward, matching the gun hand's own 0.55 for the same reason. That is the class of bug this repo's worker protocol says review finds and tests do not, and it is exactly what happened.

## Recon @ ea7dc40 (before any code)

What the feature had to fit into, read out of the source rather than assumed.

| Fact | Where | Why it constrained the design |
|---|---|---|
| Bullets are stepped only in `Sim::step`'s sweep, server-side | `crates/pong-core/src/shooter.rs` `retain_mut` block | Reflection is a server-only rule. The client never re-derives it; it sees reflected rounds as ordinary `BState` entries whose velocity changed between snapshots. |
| Aim is `[f32; 2]` unit horizontal + scalar `pitch` | `PlayerIn.aim` / `PlayerSt.pitch` | The shield normal is the horizontal aim. There is no 3D facing to mirror about, and building one is explicitly forbidden. |
| The sweep culls in a fixed order: TTL, player hit, integrate, floor, wall, cover | same block | Reflection replaces the *player hit* outcome, so it sits exactly where the hit did. |
| `remove_player` drops bullets by `owner` | `Sim::remove_player` | Ownership transfer means a reflected round outlives the player who fired it and dies with the reflector instead. |
| `hits` are applied after the sweep, keyed `(owner, victim, dmg)` | end of `step` | A reflection produces no `hits` entry at all, so no damage and no scoring path is touched on the reflect tick. |
| Held intents only, no toggles anywhere in `PlayerIn` | `PlayerIn` | `shield` is held, like `sprint`/`crouch`/`reload`/`jump`. No toggle state can desync. |

## `PROTO_VERSION` — decided: bump to 9

Argued in full at the constant in `proto.rs` and in commit 2bd7c88. The short form: `serde(default)` makes both new fields decode in both directions, and decoding is not the test. A pre-v9 client cannot raise a shield *and* can be killed by its own returning shot fired by an opponent whose shield it does not draw; a v9 client against a pre-v9 server raises a shield the server discards and stands in the open trusting it. Both peers keep playing a game they cannot see. That is the v8 pitch case, not the v8 jump case.

The bump moves the existing deploy freeze rather than opening a new one: main stood at 8 against a live server at 7 already, so the live page was un-rebuildable before this lane started. The shield rides the same redeploy as jumping, aim elevation and authored levels. **Nothing under `deploy/` or `web/` was touched** — `deploy-pages.sh` reads `PROTO_VERSION` out of `proto.rs` at ship time and warns on its own.

## The design calls, and what would change them

Each is a default chosen here, not a law. All are cheap to move.

| Call | Where | The alternative |
|---|---|---|
| **Held intent**, not a toggle | `PlayerIn.shield` | A toggle is a few lines, and a worse wire: it puts a bit of state on each side that a dropped packet desyncs. |
| **120° arc**, `SHIELD_ARC` | `shooter.rs` | The knob that matters most. Widening it toward `TAU` removes flanking, which is the only counter the shield has. |
| Arc tested on the round's **heading**, not its bearing from the holder | the sweep | The point of contact sits perpendicular to the flight line by construction, so a bearing taken there is noise. The two agree within ~9° for anything but a point-blank shot. |
| **`vy` survives** the mirror | the sweep | The plate's normal is horizontal, so this is the physical answer: a round arcing down at you returns arcing down, keeping its range instead of being launched at the sky. |
| **Ownership transfers** to the reflector | the sweep | Decides the frag, and — through `remove_player` — that a reflected round outlives its shooter leaving and dies with its catcher. |
| Reflected rounds carry **`delay = 0`** | the sweep | Lag compensation honours what a shooter saw; nobody saw this one. |
| Shield judged in the **present**, body hit test still rewound | the sweep | Deliberately unlike crouch. A defender whose reaction is retroactively ignored has the worse experience, and rewinding the flag without the facing it points along would be neither answer. The visible cost: a rewound shot can be reflected off empty air where the target no longer stands. |
| Costs: **trigger blocked, sprint cancelled** | `step`, `stance_speed` | The anti-degeneracy pair. Without them everyone holds Q forever. Only the trigger is blocked — the cooldown still runs down behind the shield, so releasing Q fires on the next tick. |
| Reflected rounds count against the reflector's **bullet cap** | `MAX_BULLETS_PER_PLAYER` | Left as-is; separating caught rounds from fired ones needs a flag on `Bullet`. |

## Rendering, within the constraints

No new meshes: both draws go through the built-in cube, so **no mesh id is allocated and no later base shifts** — none of the `set_*` setters had to absorb anything, and every existing base still points where it did. Opaque, because `BlendState::REPLACE` means a see-through shield is not something this renderer can be asked for. Nothing textured is drawn, so there is no part to double-tint. The first-person plate clears the 0.1 near plane at both ends of its swing (0.62 → 0.74 along the look vector).
