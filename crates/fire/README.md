# Fire Racer client

This crate is the native and wasm client for Fire Racer, an arcade race through a gothic castle bailey; `fire-core` supplies the race simulation and `ember-engine` supplies input, presentation, and GPU ownership.

`src/lib.rs` exposes local and online startup, `fire-app` is the native shell, and wasm exports let the web page launch the same game. Online mode uses `ember-client-net` for transport and reconciliation against `fire-server`.

The client builds track ribbons and scenery meshes, loads generated props from the repository asset tree, generates compact tiling textures at startup, and turns authoritative or local race state into frames and HUD data.

The game-to-engine layering and embedded-asset costs are governed by [`../../CLAUDE.md`](../../CLAUDE.md), with model conversion rules in [`../../docs/asset-pipeline.md`](../../docs/asset-pipeline.md) and state boundaries in [`../../docs/state-model.md`](../../docs/state-model.md).
