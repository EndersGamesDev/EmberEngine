# Models

This directory holds runtime-ready GLB geometry plus the level and SWAT rig metadata that interprets selected meshes.

Native legacy paths load the large root models from disk, while current Arena, Fire Racer and UltimateLegue clients embed selected child assets into their bundles.

Only glTF or GLB ships here; conversion paths, node naming, pivots and mesh budgets are defined in [`docs/asset-pipeline.md`](../../docs/asset-pipeline.md), and consumers must also respect [`CLAUDE.md`](../../CLAUDE.md).
