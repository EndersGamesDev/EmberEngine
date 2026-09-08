# V7 generated dungeon jailer

This character starts with an SDXL concept and a textured TRELLIS.2 sculpt. The gaunt scarred face, high collar, leather coat, pouches, straps, folds, boots and curled hands come from that generated sculpt. Offline conversion cleans its surfaces, separates moving parts, fits the existing bind dimensions and rebakes RGB albedo maps. Small connective covers and inner trousers fill areas hidden by the original fused coat; they sample the generated material. The separate V5 chair and knife are retained with their existing bind geometry and atlas.

The runtime contract remains 20 named rigid nodes with identity transforms, one primitive per node, metres, +Y up and facing -Z. `warden-rig.json` retains all V5 bind pivots and limb lengths, and additionally records the source joint positions used for retargeting. The figure is normalized to approximately 1.865 m tall. `manifest.json` records the exported bounds, triangle counts, texture sizes, payload and SHA-256 hash. The budget is at most 22,500 triangles and 20 MiB of cloned texture memory including mipmaps for the whole character, chair and knife.

The verified export contains **20,780 triangles**, occupies **4,138,652 bytes**, and uses **20,097,700 bytes (19.17 MiB)** of cloned texture memory including mipmaps. Its standing body bounds are (-0.34858, 0, -0.27009) to (0.32065, 1.86932, 0.20774) m. GLB SHA-256: `c4e4feb82628223bc3203bd275902522ac4dabd441821182223853a1d59cc446`. The manifest separately records the sidecar and source hashes.

## Generator provenance

| Stage | Actual generator/job | Settings and measured wall time |
|---|---|---|
| Concept | SDXL 1.0 base, ComfyUI on Specht `:8188`; prompt `4fc99fa8-71cc-470e-9835-2380dce3f4ef` | Seed 709081, 1024×1536, 35 steps, CFG 6.5; 16.657 s through asset CLI |
| Cutout | Existing rembg/u2net CPU tool on Knecht | 1647² RGBA, margin 0.06; 4.031 s |
| Textured mesh | TRELLIS.2 4B on Adler `:8190`, GPU 0; job `20260908-153917-a6b547` | Seed 709081, `1024_cascade`, 2048² source texture; 16.7 s generation, 28.4 s worker total, 31.631 s including fetch |

The fetched high-resolution GLB is 21,569,692 bytes and 479,888 triangles. Its original facing is +Z; conversion explicitly turns it to the runtime's -Z. The single reference provides strong face/front detail; the generator inferred the back and occluded surfaces. Back texture and fine fabric detail are softer than the reference. The generated hands retain curled forms, with some fingers fused together; they are rigid hand parts rather than a finger rig. These limits are not presented as artist-authored anatomy or motion capture.

The final local conversion and preview took 141.751 s with four CPU threads at Idle. The subsequent index-only remnant cleanup took 0.050 s; exported-file verification took about 0.1 s. Lower legs use full knee-to-ankle generated-material trouser covers unioned with the outer sculpt folds, because cuts through the original fused coat left open surfaces. Both shins have zero welded boundary or nonmanifold edges. Each upper leg also retains a 120-triangle inner trouser volume spanning y=0.505..1.005 m; these are added after decimation so simplification cannot discard the hidden coverage. They use interior samples from the generated thigh atlas and add no textures. Restoration took 0.015 s. Thin collar and coat cuts remain rigid overlapping surfaces; this is a runtime visual mesh, not a uniformly watertight fabrication model. The final cleanup removes small disconnected remnants without modifying the baked images, UVs or normals.

Barza claim #282 covered one image job and one mesh job after fresh empty-queue checks. Specht was released in #283 and the remaining TRELLIS reservation in #284. No service/profile changes or peer interruptions were performed. Client/conversion work runs at Idle; Blender uses CPU rendering with four threads. No additional models or large source archives are committed.

## Rig and material contract

- Node names are `warden_pelvis`, `warden_torso`, `warden_neck`, `warden_head`, paired `warden_upperarm_*`, `warden_forearm_*`, `warden_hand_*`, `warden_thigh_*`, `warden_shin_*`, `warden_boot_*`, `warden_coat_*`, plus `warden_chair` and `warden_knife`.
- Upper-arm/forearm lengths remain 0.32/0.29 m. Thigh/shin lengths remain 0.43/0.43 m. Bind ankles remain (±0.105, 0.11, 0); boots are fitted to the floor.
- The two coat nodes carry separate left/right panels. Inner trouser coverage prevents an empty interior when the panels move apart during the rise or gait. The renderer owns their motion.
- The right hand retains the existing knife socket (0, -0.068, -0.012) relative to its wrist. The blade points +X in bind space. The fixed chair stays at world (-2.9, 0, -3.7), yaw π, after the warden leaves it.
- Every part has a dedicated opaque RGB8 albedo bake, except the retained chair/knife which reuse their existing embedded 128² image. Head/torso maps are at most 1024², hands/pelvis 512² and remaining body maps 256². The manifest measures the loader's per-primitive texture cloning, not merely the number of distinct embedded images.
- No skin weights or animation clips are embedded. The renderer uses the sidecar and authoritative simulation phases for articulation; cloth, skin and leather are rendered as dielectric surfaces.

## Reproduction

High-resolution sources and intermediate images stay outside the checkout under `C:/Users/end/dev/end-game/runtime-v7-assets/jailer/`. The concept is `image/20260908_00022_.png`, the alpha cutout is `cutout/20260908_00022_-rgba.png`, and the original sculpt is `mesh/20260908-153917-a6b547.glb`. JSON request/result files retain the exact prompts, arguments, source hashes and job output. The source turnaround is in `preview/`; its initial `source-back.png` filename shows the face because the source's facing is opposite the runtime convention.

`tools/end-game/v7_generate_asset.py` invokes the existing `C:/Users/end/dev/cluster/provision/asset-forge/asset_cli.py` for its `image`, `cutout` and `mesh` stages. Use an available worker slot and inspect the reference before submitting the mesh. It does not switch profiles or install models. Local conversion uses `v7_convert_warden.py`, then `v7_restore_trousers.py` to preserve full upper-leg coverage, followed by `v7_finalize_warden.py` to remove tiny disconnected sculpt remnants. Validation uses `v7_verify_warden.py`, and `v7_preview_warden.py` renders the final exported file. Run all stages in this order; the first conversion preview precedes the final cleanup.

```powershell
[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$env:END_GAME_V7_SOURCE = 'C:\Users\end\dev\end-game\runtime-v7-assets\jailer\mesh\20260908-153917-a6b547.glb'
$env:END_GAME_V7_WORK = 'C:\Users\end\dev\end-game\runtime-v7-assets\jailer\converted'
$v7Build = Start-Process -FilePath 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' -ArgumentList @('--background', '--threads', '4', '--python', 'tools/end-game/v7_convert_warden.py') -WindowStyle Hidden -PassThru
$v7Build.PriorityClass = 'Idle'
$v7Build.WaitForExit()
& 'C:\hy3d\venv\Scripts\python.exe' tools/end-game/v7_restore_trousers.py
& 'C:\hy3d\venv\Scripts\python.exe' tools/end-game/v7_finalize_warden.py
& 'C:\hy3d\venv\Scripts\python.exe' tools/end-game/v7_verify_warden.py
$v7Preview = Start-Process -FilePath 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' -ArgumentList @('--background', '--threads', '4', '--python', 'tools/end-game/v7_preview_warden.py') -WindowStyle Hidden -PassThru
$v7Preview.PriorityClass = 'Idle'
$v7Preview.WaitForExit()
```

The converter uses Blender 5.2.1 and writes GLB, sidecar, per-part bake PNGs and a standing preview into the scratch work directory. It does not save a `.blend`. After successful verification, only `warden.glb`, `warden-rig.json` and `manifest.json` belong beside this README. The scripts print measured wall time. Native seated, walking, attack and collapse review belongs to the release capture harness, separately from offline geometry/texture verification.
