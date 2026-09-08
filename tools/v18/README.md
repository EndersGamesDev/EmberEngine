# Arena v18 tools

These tools served Arena v18, the Freight Yard release, by building the multi-weapon viewmodel, drawing the deterministic loot-block texture, preparing source albedos, and capturing hands-off native multiplayer evidence.

`build_weapons.py` composes the v16 and v17 assets with named loot weapons and atlas-baked materials into the GLB and pivot sidecar consumed by `crates/arena`; `capture.ps1` owns only its loopback server and scripted clients and pins the rule that evidence must never take operator input or foreground focus.

Weapon fits, texture budgets, outputs, and capture acceptance live in `docs/plans/arena-v18-freight-yard.md`, with reusable imported-asset and atlas-bake rules in `docs/asset-pipeline.md`; large artist sources remain outside Git.
