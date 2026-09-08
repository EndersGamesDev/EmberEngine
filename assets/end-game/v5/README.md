# V5 articulated prison warden

`warden.glb` contains a bald human warden in worn brown cloth and leather, a separate wooden chair, and a right-hand Bowie knife. The 20 rigid parts let the renderer show sleeping, planting the feet, standing, drawing the knife, walking, attacking, staggering, and collapsing. V1–V4 assets remain independent.

The final export is 486,824 bytes and 11,830 triangles. It embeds one opaque RGB8 128×128 material atlas (14,330 bytes). The current loader copies this atlas per part: approximately 1.67 MiB of GPU texture storage including mipmaps. `manifest.json` records per-part bounds, triangle counts, payload hash, and validation results.

## Geometry and animation contract

- Metres, +Y up, character faces -Z. Standing body bounds are X ±0.356, Y 0.001–1.865, Z -0.188–0.155.
- Every uniquely named node has exactly one primitive and identity transforms. Vertices already occupy the standing bind frame. No skeleton, skin weights, or GLB animations are required.
- `warden-rig.json` lists the parent and bind pivot for every node. It includes upper-arm/forearm lengths of 0.32/0.29 m and thigh/shin lengths of 0.43/0.43 m.
- Parts are pelvis, torso, neck, head, and paired upper arms, forearms, hands, thighs, shins, boots, and coat tails; the chair and knife complete the 20 nodes. Names use the `warden_` prefix and `_r`/`_l` suffixes.
- Chair origin is its floor anchor, placed permanently at world (-2.9, 0, -3.7), yaw π. Its seat top is Y 0.54 m. It stays in place after the enemy walks away.
- Right-hand pivot is (0.26, 0.85, 0). Knife grip is (0.26, 0.782, -0.012), so its hand-local socket is (0, -0.068, -0.012). Knife blade points +X and extends 0.314 m beyond its guard. The knife remains in the belt before the draw grasp, then follows this rigid hand socket.
- Character world orientation is `Ry(-warden_ai.yaw)`; yaw zero faces -Z, while the initial yaw π faces the player's cell.

`crates/end-game/src/warden.rs` loads this contract through `WardenRig::load(&mut Vec<MeshData>)` and emits instances with `draw(&self, &mut Vec<Instance>, &Dungeon)`. It derives all action progress from `Dungeon.warden_ai`, including the retained stand/draw amounts on death and actual-movement `walk_blend`. Two-bone IK keeps the feet planted through the rise and the arms connected. Knife anticipation/contact/follow-through/recovery use the core knife phase constants. Sleeping has a small breathing/head-doze movement.

## Reproduction and provenance

The geometry and small material atlas are deterministic local code assets. The generator reuses the existing V3 geometry helpers without modifying them. No image-generation service, downloaded mesh, or fused V1 character geometry is included. The modeled head is intentionally stylized; its separate facial features do not reproduce the organic detail of the V1 baked head.

Run from the repository root in PowerShell with Python/Pillow/NumPy and Blender available. All intermediate images, previews, logs, and GLBs go to the explicit scratch directory; no `.blend` is saved.

```powershell
$v5Shell = [Diagnostics.Process]::GetCurrentProcess()
$v5Shell.PriorityClass = 'Idle'
$env:END_GAME_V5_ASSET_WORK = 'C:\Users\end\dev\end-game\runtime-v5-assets\warden'
& 'C:\hy3d\venv\Scripts\python.exe' tools/end-game/v5_generate_atlas.py
$v5Build = Start-Process -FilePath 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' -ArgumentList @('--background', '--threads', '4', '--python', 'tools/end-game/v5_build_warden.py') -WindowStyle Hidden -PassThru
$v5Build.PriorityClass = 'Idle'
$v5Build.WaitForExit()
& 'C:\hy3d\venv\Scripts\python.exe' tools/end-game/v5_verify_warden.py
```

The atlas seed is 5090926. The verified build used Blender 5.2.1, CPU rendering, and four threads; the final geometry/export/standing preview pass took 11.896 seconds. Preview output is `warden-standing-preview.png` in the scratch directory. After verification, copy only `warden.glb`, `warden-rig.json`, and `manifest.json` into this directory.

The GLB verifier checks node names, identity transforms, one primitive per node, finite positions/UVs, normalized normals, winding agreement, absence of degenerate triangles, RGB8 images, opaque white material factors, valid sidecar hierarchy, and payload/triangle budgets. Renderer tests sweep all six phases and seated/partially risen deaths, checking IK reach, bone lengths, planted boots, rigid knife contact, stationary chair, and knife geometry remaining above ground. The final two targeted renderer tests passed with the shared Cargo cache at Idle in 3.44 seconds. Native in-game visual review and publication are separate release checks.
