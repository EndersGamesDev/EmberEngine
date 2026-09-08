# Ultimate League baked art

`art.rs` registers baked champion, surface, and prop GLBs after the client's procedural meshes, reads their adjacent sidecars, and falls back to procedural bodies when a champion delivery is absent.

The scene builder consumes its mesh lists and champion lookup; textured parts use white instance colour so the renderer does not multiply the authored base colour twice, while intentional hologram or form tints remain explicit.

The baked files originate from `tools/league/art/bake_champion.py` and the fleet references it consumes; GLBs and sidecars are shipping outputs, while large artist source remains outside Git.

Axes, one-texture-per-mesh limits, provenance, and sidecar expectations live in [`../../../../docs/asset-pipeline.md`](../../../../docs/asset-pipeline.md), with the game-specific bake workflow documented under `tools/league/art/`.
