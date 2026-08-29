# The 3D asset pipeline

How every model currently in the game was produced, and how to run the
pipeline for a new one. Written down because most of what follows was
learned by getting it wrong first — the failure modes are silent, and
each one costs an afternoon to rediscover.

There are two separate paths. Pick by what you are starting from:

| Starting from | Path | Example in the repo |
|---|---|---|
| Nothing but an idea | **Generated** — images → mesh | the veteran (`assets/models/parts2/vet-*.glb`) |
| An artist's rigged model (FBX/glTF) | **Imported** — split by bone | the SWAT operator (`assets/models/swat-parts.glb`) |
| An artist's scene/level | **Imported** — split by island | the factory skyline (`assets/models/level-backdrop.glb`) |

Everything the engine consumes is a **GLB with base-color textures
embedded**, loaded by `ember_engine::assets::load_glb`.

## Path A — generated: images to mesh

1. **Concept views.** Render four orthographic views per part (front /
   back / left / right) at 1024². These are the mesh generator's input,
   so what matters is a clean silhouette on a plain light-gray
   background, one subject, centred, no shadow.

   Generator: ComfyUI + Qwen-Image on adler, driven by
   `.claude/veteran-v2/gen_views.py` (and `gen_views_v3.py` for the
   detail set). It talks to `http://127.0.0.1:8188` over ssh-side curl
   and writes `~/comfy/out/<part>-<view>_*.png`. ~32 s per view on the
   4090. The prompt bible lives in `.claude/veteran-v2/prompts.md` —
   reuse it so a new part matches the existing character.

2. **Mesh.** `.claude/veteran-v2/mesh-parts.ps1` feeds each part's views
   to `C:\hy3d\gen3d_mv.py` (Hunyuan3D-2mv, multi-view — much better
   backs and sides than the single-view model), then decimates to 30k
   triangles. ~130 s per part on the 4080. It pauses the local GLM
   worker for VRAM and restarts it in a `finally`, and skips parts whose
   GLB already exists, so a re-run resumes.

3. **Rig it.** Register each GLB and describe the character with
   `ember_engine::rig::VeteranSources`; `veteran_rig()` anchors each
   part to its joint. Parts are anchored by *bounds fraction* (a thigh
   hangs from the top of its box), so mesh scale and origin do not
   matter — only proportions do.

**Gotcha — authored facing.** A single-view concept faces the camera
(engine −Z); a Hunyuan multi-view mesh comes out facing +Z. Mixing them
silently renders half the character backwards. `PartSource.flipped`
records which is which; set it at load time, never by rotating the mesh.

## Path B — imported: an artist's rigged model

Worked example: `tools/swat_split.py` (Mixamo-rigged FBX → 15 GLB parts
+ a bind-pose JSON), consumed by `ember_engine::rig::skinned_from_glb`.

1. **Flatten the import.** Blender's glTF/FBX importers parent everything
   under a rotated root. Any measurement you take in world space and
   write back into local coordinates will be wrong by that rotation.
   Clear parents (`CLEAR_KEEP_TRANSFORM`) and apply transforms
   immediately after import, so local == world.

2. **Normalise UV layer names BEFORE joining.** `object.join()` merges UV
   layers *by name*. Objects whose layer is named differently end up
   with an empty first layer, glTF exports only `TEXCOORD_0`, and the
   entire model then samples one texel — it renders as flat colour that
   looks like a lighting bug, not a UV bug. Rename every object's active
   layer to a shared name first. This one cost the most time; both
   `swat_convert.py` and `swat_split.py` now do it.

3. **Split by dominant bone weight.** Each vertex goes to the joint of
   its heaviest weight, walking up the bone chain for unmapped bones
   (fingers → hand, toes → foot). Faces follow their majority vertex.
   Put the clavicle with the ARM, not the spine, or the shoulder seam
   tears open when the arms come down.

4. **Place rigid, unweighted geometry by hand.** A held weapon usually
   has no skin weights, so it gets no vote and is silently dropped. Map
   it by material instead (`mat_fallback` in `swat_split.py`).

5. **Bake transforms before the final join**, for the same reason as (1):
   joining leaves everything inside the first object's local space, and
   the exporter's up-axis conversion then tips the result on its side.

6. **Retarget, don't re-author.** The model is authored in an A-pose;
   the engine rig expects arms down. `rig::skeleton_from_bind()` measures
   the joint offsets from the bind pose and solves a per-joint
   correction rotation, applied innermost, so the existing walk/idle/
   crouch animation drives the model unchanged. Tests:
   `imported_bind_pose_retargets_arms_down`,
   `retargeted_arms_still_swing_when_walking`.

## Path C — imported: a scene or level

`tools/level_convert.py` (full level + collision boxes) and
`tools/level_backdrop.py` (scenery ring) show both shapes.

- Source scenes ship **merged instances** — every cargo container in one
  object spanning the whole map — which are useless as collision
  volumes. Separate by loose parts first, take one AABB per island, then
  re-join for rendering.
- Derive world scale from a known reference rather than guessing: a
  cargo container is 2.6 m tall, which fixed this source at 0.65 m/unit.
- Decimate scenery hard (the backdrop runs at ratio 0.25). It is 40 m
  away and the wasm build embeds it.

## Engine-side constraints

- **One texture per mesh.** `MeshData` owns its texture, so every
  registered mesh carries its own copy — 16 parts sharing a 1024² atlas
  cost ~60 MB of VRAM. Keep part atlases at 1024² until the renderer
  learns to share textures.
- **The wasm build embeds assets** via `include_bytes!`. Every megabyte
  of GLB is a megabyte of download: the arena bundle is ~17 MB, of which
  the operator is ~5 MB. Check the size before embedding.
- **Meshes are de-indexed** into flat triangle lists at load, so vertex
  count is 3× the triangle count in memory.
- The loader reads `TEXCOORD_0` and the material's base-colour texture
  only; roughness/normal/AO maps in a source archive are ignored today.

## Running it for a new model

```bash
# Path B, an artist-rigged character:
blender --background --python tools/swat_split.py     # edit SRC/OUT at the top
cargo run -p game                                     # verify in the native client
```

Review the result in-game before committing: `EMBER_CAM="ex,ey,ez,tx,ty,tz"`
pins a fixed camera in both the native game and the arena client, which
is what every asset screenshot in this repo was taken with.
`EMBER_CHAR=rig|puppet|mesh` switches the character path for comparison.

Assets themselves stay out of git when they are large source archives
(`assets/swat/`, `assets/level/` are ignored); the produced GLBs are
committed, because the wasm build embeds them at compile time.
