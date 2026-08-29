# The 3D asset pipeline

How every model currently in the game was produced, and how to run the
pipeline for a new one. Written down because most of what follows was
learned by getting it wrong first — the failure modes are silent, and
each one costs an afternoon to rediscover.

There are four paths. Pick by what you are starting from:

| Starting from | Path | Example in the repo |
|---|---|---|
| Nothing but an idea | **A** — generated: images → mesh | the veteran (`assets/models/parts2/vet-*.glb`) |
| An artist's rigged model (FBX/glTF) | **B** — imported: split by bone | the SWAT operator (`assets/models/swat-parts.glb`) |
| An artist's scene/level | **C** — imported: split by island | the factory skyline (`assets/models/level-backdrop.glb`) |
| An artist's static prop, already in named parts | **D** — imported: keep the parts, add pivots | the 9mm (`tools/9mm_convert.py`) |

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
- **8-bit only.** The image decoder accepts `R8G8B8A8`, `R8G8B8` and `R8`
  and returns `None` for anything else — **with no log line**. A 16-bit
  re-export from Blender therefore ships a correctly-shaped, entirely
  untextured model. Verify the *exported* PNG's bit depth, not the
  source's.
- **No mipmaps** (`mip_level_count: 1`, and no mipmap filter on the
  sampler), so a detailed texture shimmers at distance. Another reason to
  downscale at bake time rather than ship source resolution.
- **No backface culling** (`cull_mode: None`): the interior of an open or
  non-manifold shape renders solid rather than vanishing.
- **No blending** (`BlendState::REPLACE`): there is no additive or
  transparent anything. A muzzle flash or a glow can only be an opaque
  mesh.
- `baseColorFactor` should be **white**. `push_parts` multiplies its own
  per-instance colour in, so a tinted factor double-tints the texture.

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

## Path D — imported: a static multi-part prop

An artist's prop with **no bones and no animation curves**, split into named parts — a weapon, a door, a machine. `tools/9mm_convert.py` is the worked example (a seven-part pistol: frame, slide, trigger, hammer, mag, ejector, slide stop).

The shape of this path is different from B: there is no skeleton to split by and nothing to weight against, so **the parts arrive already separated and the job is to keep them that way**. Separate parts plus pivots is what makes animation possible at all, since the engine has no skinning for props.

- **Delete the studio.** Artist scenes ship backdrops, ground planes and lights (`Plane001`, `Sky001`). Assert on the surviving object set by name and fail loudly listing what was actually found — converting whatever happened to survive is how a backdrop plane ends up welded to a pistol.
- **Wire the base colour by hand.** Marketplace FBXs routinely link only the normal map, or nothing. A naive import→export then yields an untextured model with no error anywhere. Assert the image datablock actually decoded — a 0×0 image counts as "loaded".
- **Do not join.** Each part costs one mesh id and one texture upload, so weigh it: a slide that must cycle earns its part, a screw does not. Seven parts at 512² is ~7 MB of VRAM; at source 2048² it would be ~112 MB for one pistol.
- **Watch the downscale survive export.** Blender's glTF exporter copies the *original file bytes* for an on-disk image it believes unmodified, silently shipping the full-resolution texture. Pack the scaled image, then re-parse the exported PNG and fail if it is bigger than intended.
- **The importer parents everything under a root.** Unparent with the world transform preserved (`parent_clear(CLEAR_KEEP_TRANSFORM)`, or snapshot `matrix_world` / clear `parent` / restore) *before* `transform_apply`, or the bake uses the wrong basis.
- **A derived axis fit cannot check itself.** If the script rotates the longest axis onto +X and scales it to a target length, then "is +X longest" and "is the length right" are tautologies. Only a heuristic picks *which end* is forward, and a 180° flip leaves length, longest axis and handedness all correct — the prop ships pointing backwards, exit 0, no warning. Assert the muzzle/front is forward of the origin and near the front of the bounds.

## Conventions: axes and the hold point

- **Blender +X forward** (muzzle, face, front), **+Z up**. Export `export_format="GLB", export_yup=True`.
- That lands as **engine +X forward, +Y up, +Z right** — so Blender +Y is engine **left**.
- Points map **(x, y, z) → (x, z, −y)**.
- **A box does not map componentwise.** That mapping negates an axis, which swaps min and max on it. Map both corners then re-derive min/max, or you emit a box with `min > max`.
- **The origin is the hold point.** `push_parts` draws every part at one position with unit scale, so the GLB's origin *is* where the thing is held or stood — the grip for a weapon, between the feet for a character.
- Match the established scale: the viewmodel pistol is ~0.9 units long.

## The node-name contract

Names are load-bearing. `load_assets` in `crates/pong/src/online.rs` classifies on them:

| Name | Meaning |
|---|---|
| `arm*`, `hand*` | Viewmodel-only — **not** drawn on remote players |
| anything else | Gun geometry — drawn in the viewmodel **and on every remote player** |
| exactly `strip` | Takes the per-weapon-level accent colour |

Miss a rename and every remote player sprouts your prop, silently. Verify the names **in the exported GLB**, not in Blender.

## Pivots do not survive import

`load_glb` **bakes each node's world transform into its vertices and discards the hierarchy**. After import nothing can rotate about anything: a slide cannot cycle, a hammer cannot fall.

So any prop that will be animated per-part ships a **sidecar JSON** beside the GLB carrying each part's pivot in engine space — `swat-rig.json` is the precedent, written by `swat_split.py`, read by `rig.rs`.

**Key order in that sidecar matters.** `rig.rs`'s reader is a substring scanner: it finds `"<name>"` and takes the **next** `[ … ]`. If a bare list of part-name strings appears before the pivot map, every part's first match lands inside that list and the next `[` is the *first* part's pivot — so every part silently shares one pivot, with no parse error. Put the per-part maps **first**, and never give a list of part names a key that could collide with a pivot key (`part_order`, not `parts`).

## Driving the animation

With no baked clips, prop animation is procedural: per-part transforms driven from state the sim already tracks (`cooldown`, `reload_t`, ammo transitions).

Drive it from **authoritative** signals, not from input. A muzzle flash keyed to a held trigger fires on an empty magazine; keyed to a free-running clock it fires when nothing was shot. Key it to the server-confirmed ammo decrement — and then qualify *that*, because a weapon pickup and a respawn also reduce ammo.

## Checklist

Before committing a converted asset:

- [ ] Node names satisfy the contract, verified in the **exported GLB**.
- [ ] Exactly one UV layer, one shared name, on every mesh.
- [ ] Exported PNG is 8-bit and no larger than intended.
- [ ] Every primitive has `TEXCOORD_0`, a material, and a base-colour texture.
- [ ] `baseColorFactor` is white.
- [ ] Origin is the hold point; +X is forward; size matches the established scale.
- [ ] Sidecar written, per-part maps **before** any name list.
- [ ] Reviewed in-game with `EMBER_CAM` before committing.
- [ ] Raw source gitignored; only the GLB and sidecar staged.
- [ ] Bake wall time reported.
