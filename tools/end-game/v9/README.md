# V9 generated environment reproduction

These tools reproduce the three environment props in `assets/end-game/v9`. Generation uses the existing asset-forge CLI, SDXL on Specht and TRELLIS.2 on Adler; no new service or profile is installed. Review fresh Barza reservations and worker queues before a new bounded generation run. The recorded V9 jobs already finished and their fleet slots are released.

Set `END_GAME_V9_WORK` to an absolute asset workspace outside the engine checkout before running these tools. The original workspace is `C:/Users/end/dev/end-game/runtime-v9-assets`. Large concepts/source meshes and scratch textures stay there. Installed generation/conversion tools require the explicit workspace setting. Python/Pillow and Blender locations currently match this workstation; update them for another host.

```powershell
$env:END_GAME_V9_WORK = 'C:/Users/end/dev/end-game/runtime-v9-assets'
& 'C:/hy3d/venv/Scripts/python.exe' tools/end-game/v9/generate_kit.py gothic-pillar image
& 'C:/hy3d/venv/Scripts/python.exe' tools/end-game/v9/generate_kit.py gothic-pillar cutout
& 'C:/hy3d/venv/Scripts/python.exe' tools/end-game/v9/generate_kit.py gothic-pillar mesh
```

The prop names are `gothic-pillar`, `courtyard-fountain` and `tower-doorway`. Each stage refuses duplicate output, records arguments/results/hashes/wall time, and image/mesh stages check the corresponding queue is idle. The original run generated exactly three concepts and three meshes sequentially. `write_provenance.py` reads only matching concept-job identities from ComfyUI history into the local manifest.

Run Blender with `--background --python tools/end-game/v9/convert_prop.py -- <prop>` at Idle priority. On Windows, use `Start-Process -WindowStyle Hidden -PassThru`, set the returned process's `PriorityClass` to `Idle`, and redirect its logs outside the checkout. The converter flattens source transforms, normalizes measured height, decimates within the per-prop budget, preserves UVs, keeps one primitive and embeds a 768 × 768 RGB8 atlas. `dry_basin_albedo.py` replaces the generated pool-like interior with matte stone; `warm_stone_albedo.py` places the actual pixels in the shared warm limestone palette.

Run `preview_prop.py` through the same passive Blender path to render front/back views with four CPU threads. `measure_prop.py` checks the portal's recommended transformed visual aperture. `verify_kit.py` inspects the real exported GLB bytes and enforces the combined 35,000 triangle / 6 MB / 12 MiB texture budgets. The converter explicitly resizes saved PNG bytes: Blender's packed imported images can otherwise save their original 2048² pixels after `image.scale()`, defeating the budget.

The asset pipeline cannot replace runtime verification. Generated meshes have some broken/nonmanifold decorative edges; author measured collision geometry, load the converted assets in Ember, and inspect the integrated route before publication. Do not use the full portal AABB as an obstacle or treat the smaller Gothic support openings as walkable.
