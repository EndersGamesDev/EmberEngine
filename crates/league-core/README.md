# Ultimate League core

`league-core` is the pure server-authoritative simulation and wire-format crate for Ultimate League, with no threads, clocks, I/O, platform, or renderer dependencies.

`league-server` runs its 60 Hz `Match`; the client receives snapshots and uses shared data definitions but never simulates abilities, so combat physics and deterministic hash rolls have exactly one authority.

The crate owns duel and squad rules, five champion kits, bots, waves, courts, cores, progression, items and runes, projectiles and zones, match lifecycle, and the game-specific JSON protocol.

The current design and balance argument lives in [`../../docs/plans/ultimate-league-v4.md`](../../docs/plans/ultimate-league-v4.md), with repository authority and layering constraints in [`../../CLAUDE.md`](../../CLAUDE.md).
