# Textures

This directory holds 8-bit runtime PNGs for legacy character materials and the current Arena operator, plus versioned environment and loot textures.

Arena embeds `player_armor.png` and the versioned children into its wasm bundle, while legacy native paths consume the older character textures.

The accepted channel depths, colour handling, texture budgets and bake paths are defined in [`CLAUDE.md`](../../CLAUDE.md) and [`docs/asset-pipeline.md`](../../docs/asset-pipeline.md).
