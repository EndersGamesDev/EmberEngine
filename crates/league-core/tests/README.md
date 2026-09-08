# Ultimate League gameplay contracts

`controls.rs` pins attack-move serialization and field clamping, target acquisition and resumption, cooldown integrity, animation takeover, ranged impact identity, and Tessera's single-target gear shot through the public simulation API.

`gameplay.rs` covers progression, percentage stats, haste, assists and bounties, burn and revive interactions, squad fallbacks, champion selection, every ranked ability, spells, runes, items, courts, shopping, bots, snapshots, results, and defended-core completion.

These downstream-style suites complement the in-module cases by composing data, protocol commands, kits, AI, and the full `Match` lifecycle without access to private helpers.

Their expected game behavior is argued in [`../../../docs/plans/ultimate-league-v4.md`](../../../docs/plans/ultimate-league-v4.md), while the authoritative-only execution rule is stated in [`../../../CLAUDE.md`](../../../CLAUDE.md).
