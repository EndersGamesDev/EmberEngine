# Arena v16 tools

These two programs served Arena v16 by replacing the bare-hand revolver presentation with a first-person rifle and gloved hands cut from the same SWAT operator source shown to remote players.

`prep_pictures.py` creates bounded RGB textures for the body, hands, and rifle, while `build_operator_viewmodel.py` applies the authored armature pose, extracts dominant-bone hand geometry, aligns the weapon frame, and updates the shared viewmodel GLB and muzzle sidecar consumed by `crates/arena`.

The source FBX stays in the ignored artist archive; `docs/asset-pipeline.md` records the v16 provenance, pose-versus-bind-pose lesson, coordinate measurements, and one-texture-per-mesh budget.
