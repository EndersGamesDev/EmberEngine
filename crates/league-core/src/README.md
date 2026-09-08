# Ultimate League core source

`lib.rs` exposes `Match`, the fixed tick constants, and six public modules: `sim` owns the ordered world update, `kits` implements all champion abilities, and `ai` produces deterministic bot commands.

`data` is the champion, ability, item, spell, rune, and stat table; `rng` is the stateless tick-and-identity hash; `proto` defines lobbies, selections, commands, snapshots, effects, limits, and exact game-version messages.

The simulation order is phase, queued commands, ID-ordered units, projectiles, zones, cleanup, waves and objectives, then the win check; callers must not infer authority from client presentation between those steps.

Module tests pin all combat identities and kits, deterministic bot matches, lifecycle and objectives, economy and progression, protocol round trips and defaults, data-table validity, hash behavior, and hostile-input bounds under [`../../../docs/plans/ultimate-league-v4.md`](../../../docs/plans/ultimate-league-v4.md).
