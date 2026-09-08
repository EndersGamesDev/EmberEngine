# Arena v15 tools

This directory served Arena v15's first shipped heavy-revolver viewmodel, combining a converted twenty-part Collada weapon with posed, textured hands in a headless Blender build.

The conversion and inspection helpers preserve named moving parts, `prep_pictures.py` fixes shipping picture formats and sizes, and `build_viewmodel.py` emits `crates/arena/assets/viewmodel.glb` plus `viewmodel-rig.json`; the client animates the named cylinder, hammer, trigger, and muzzle pivots from that sidecar.

Artist archives remain outside Git and reviewed derivatives carry their source identity through the build; the Path D worked example in `docs/asset-pipeline.md` is the authority for fitting, coordinate, texture, and sidecar decisions.
