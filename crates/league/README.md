# Ultimate League client

This crate is the native and wasm client for Ultimate League, a one-lane 1v1 or 3v3 MOBA with local bot practice and authoritative online matches.

`league-core` owns combat outcomes and the server simulation; the client turns local or received snapshots into one `World`, renders it through `ember-engine`, and exchanges page-owned draft, HUD, shop, minimap, binding, and command data through JSON.

`league-app` launches practice or online play natively, while the library exports the same modes and browser bridge for the web release.

The current game and interface design is recorded in [`../../docs/plans/ultimate-league-v4.md`](../../docs/plans/ultimate-league-v4.md), under the rendering and layering constraints in [`../../CLAUDE.md`](../../CLAUDE.md).
