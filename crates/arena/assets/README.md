# Arena client assets

This directory contains the GLB bundles embedded into the arena client for the shotgun, multi-weapon viewmodel, per-weapon gloves, and sleeves, together with JSON metadata that locates muzzles, moving parts, wrists, and finger poses in engine space.

`src/online.rs`, `src/viewarms.rs`, and `src/grips.rs` load these bytes at compile time; the renderer therefore performs no runtime asset fetch, and every committed byte contributes to the wasm download and decoded GPU memory.

The committed GLBs are export products, not artist source: large source archives remain outside Git, while the adjacent rig JSON preserves animation pivots and attachment data that glTF import does not retain reliably. `tools/v15`, `tools/v16`, `tools/v18`, and the weapon-grip tooling described in `docs/weapon-grips.md` record the production lineage for these generations.

Formats, coordinate axes, node naming, texture limits, provenance expectations, and the sidecar convention are defined in [`../../../docs/asset-pipeline.md`](../../../docs/asset-pipeline.md); recorded audio has its separate contract in [`sfx/README.md`](sfx/README.md).
