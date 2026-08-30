# Gate report — branch `lane/q-shield-reflect`

The off-hand shield: held on Q, and it does not eat a bullet — it sends it back.

## Recon @ ea7dc40 (before any code)

What the feature has to fit into, read out of the source rather than assumed.

| Fact | Where | Why it constrains the design |
|---|---|---|
| Bullets are stepped only in `Sim::step`'s sweep, server-side | `crates/pong-core/src/shooter.rs` `retain_mut` block | Reflection is a server-only rule. The client never re-derives it; it sees reflected rounds as ordinary `BState` entries whose velocity changed between snapshots. |
| Aim is `[f32; 2]` unit horizontal + scalar `pitch` | `PlayerIn.aim` / `PlayerSt.pitch` | The shield normal is the horizontal aim. There is no 3D facing to mirror about, and building one is explicitly forbidden. |
| The sweep culls in a fixed order: TTL, player hit, integrate, floor, wall, cover | same block | Reflection has to be placed in that order deliberately — it replaces the *player hit* outcome, so it sits exactly where the hit did and the world checks that follow still apply to the reflected round in the same tick. |
| `remove_player` drops bullets by `owner` | `Sim::remove_player` | Ownership transfer means a reflected round outlives the player who fired it and dies with the reflector instead. |
| `hits` are applied after the sweep, keyed `(owner, victim, dmg)` | end of `step` | A reflection produces no `hits` entry at all, so no damage and no score path is touched on the reflect tick. |
| Held intents only, no toggles anywhere in `PlayerIn` | `PlayerIn` | `shield` is held, like `sprint`/`crouch`/`reload`/`jump`. No toggle state can desync. |

Filled in as the lane proceeds.
