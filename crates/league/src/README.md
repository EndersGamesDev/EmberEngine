# Ultimate League client source

`lib.rs` publishes page bindings, practice and online game types, world state, HUD access, and local or online startup; `main.rs` is the native duel, squad, and network launcher.

`game.rs` maps shared controls and runs the in-process practice authority, `online_game.rs` applies server snapshots and sends commands, `net.rs` keeps WebSocket hello and ping independent of rendering, and `world.rs` is the single display and page projection for both modes.

`bindings.rs` validates physical-key maps, `feedback.rs` derives presentation only from confirmed snapshots and effects, `combat.rs` draws identity-specific projectiles and casts, and `scene.rs` plus `scene/` build the lane and champion art.

Tests pin input edges and rebinding, command targeting, snapshot conversion, authoritative feedback, bounded combat geometry, mesh identity, protocol keepalives, and parseable page JSON under [`../../../docs/plans/ultimate-league-v4.md`](../../../docs/plans/ultimate-league-v4.md).
