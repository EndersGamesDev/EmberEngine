# The 3D asset pipeline

How every model currently in the game was produced, and how to run the
pipeline for a new one. Written down because most of what follows was
learned by getting it wrong first — the failure modes are silent, and
each one costs an afternoon to rediscover.

There are five paths. Pick by what you are starting from:

| Starting from | Path | Example in the repo |
|---|---|---|
| Nothing but an idea | **A** — generated: images → mesh | the veteran (`assets/models/parts2/vet-*.glb`) |
| An artist's rigged model (FBX/glTF) | **B** — imported: split by bone | the SWAT operator (`assets/models/swat-parts.glb`) |
| An artist's scene/level | **C** — imported: split by island | the factory skyline (`assets/models/level-backdrop.glb`) |
| An artist's static prop, already in named parts | **D** — imported: keep the parts, add pivots | the 9mm (`tools/9mm_convert.py`) |
| A whole map's worth of surfaces and props, from nothing | **E** — generated pictures onto boxes and generated meshes | arena v13 "Trench City" — runbook `tools/v13/`, meshes `assets/models/v13/`, textures `assets/textures/v13/` |

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

**Worked example — the Fire Racer props.** `assets/models/fire/fire-{car,gatehouse,tower,fountain}.glb` came through this path in one sitting: `.claude/fire-castle/gen_views.py` rendered 4 views x 4 props on adler (~32 s each), then `.claude/fire-castle/mesh-props.ps1` fed them to `gen3d_mv.py` and decimated (~135 s per prop, 9.1 min for the set). Two things that run differently from the veteran set:

* **Decimate twice.** `gen3d_mv.py` leaves 30k faces, which is a hero-asset budget. These props are embedded in a wasm bundle every web player downloads, so `mesh-props.ps1` runs `decimate.py` again at 6000 faces. The four props total ~430 KB.
* **The output has POSITION and nothing else** — no NORMAL, no TEXCOORD_0, no material. `load_glb` fills those with +Y and (0,0), which loads but renders flat-lit and samples a single texel of any texture attached. `crates/fire/src/meshes.rs` recomputes face normals and projects planar UVs on the way in. It does this in the *game* crate deliberately: the arena's character parts are POSITION-only too, so "fixing" the loader's defaults would silently restyle a live game.

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
- **Mipmaps exist as of arena v13.** Texture upload builds a full box-filtered
  chain on the CPU, sets `mip_level_count` to the chain length and gives the
  sampler `mipmap_filter: Linear`, so a detailed texture no longer shimmers at
  distance; it costs ~33% more texture memory. Downscaling at bake time still
  matters — every texture is embedded in the wasm bundle by `include_bytes!`,
  so source resolution is still a download cost for every web player — but it
  is now a **bundle-size** argument, not an aliasing one.
- **No backface culling** (`cull_mode: None`): the interior of an open or
  non-manifold shape renders solid rather than vanishing.
- **No blending** (`BlendState::REPLACE`): there is no additive or
  transparent anything. A muzzle flash or a glow can only be an opaque
  mesh.
- `baseColorFactor` should be **white**, and the failure differs by path.
  The viewmodel path (`push_parts`) multiplies its own per-instance colour
  in, so a tinted factor **double-tints**. The skinned character path
  ignores the factor entirely — `skinned_from_glb` takes only `part.mesh`
  and `push_rig` supplies the colour — so a tinted factor is silently
  **discarded**. Different symptom, same rule: author it white and let the
  runtime colour it.

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

## Path E — generated pictures onto boxes and generated meshes

A whole map, with no artist and no source archive: every surface and every prop is generated. Arena v13 "Trench City" is the worked example, and unlike the earlier `.claude/*` runbooks **the scripts are committed**, under `tools/v13/`, so the next map does not start from a chat transcript.

The path splits in two because the renderer does. Cover volumes are AABBs the sim already owns, so they are drawn as boxes and the realism has to live entirely in the picture; decor is silhouette work, so it is a generated mesh. Both ends terminate in exactly one base-colour texture, because that is all `MeshData` holds.

**What is in `tools/v13/` today**

| Script | Does | Where it runs |
|---|---|---|
| `gen_textures.py` | The **material** pictures — 20 of them: `container-side/doors/roof`, `crate-side/top`, `ammo-side/top`, `sandbag`, `trench-wall`, `tunnel-roof`, `rubble`, `cobble`, `city-wall`, the six materials box-projected onto the props (`limestone`, `sandstone`, `bronze`, `burlap`, `scorched-steel`, `cast-iron`), and a 2048×512 `sky` panorama that is deliberately *not* a tile. Every prompt asks for a flat, evenly lit, seamless albedo **with soft ambient occlusion baked in**, because there is no AO map to add it later. Raw output lands in `assets/concepts/v13/textures/`. | the picture generator |
| `gen_views.py` | The four orthographic **concept views** per decor prop (7 props: `cathedral`, `facade-a`, `facade-b`, `statue`, `sandbags`, `wreck`, `lamp`), the Path A input for the mesh generator. It owns the ComfyUI graph, `post`/`wait`/`fetch`, and the prompt tables; `gen_textures.py` imports all four from it. Two long props (`wreck`, `sandbags`) override the four view phrases, because "front" and "back" of a long object otherwise read as two different objects — the same fix the Fire Racer car needed. ~32 s per 1024² view. | the picture generator |
| `fetch_pictures.py` | Pulls finished pictures **off the other generator's history** and names them from the prompt tables (see below). `--list` matches without fetching. Idempotent, and it refuses anything whose bytes are not a PNG. | workstation, over ssh |
| `mesh-props.ps1` | The **mesh** step: Hunyuan3D-2mv (`C:\hy3d\gen3d_mv.py`) over the fetched views of each of the 7 props, front view required, then decimated a second time to a **per-part budget** (8000 faces for the statue and sandbags that are stared at from arm's length, 6000 cathedral, 5000 wreck, 4000 façades, 3000 lamp) into `assets/models/v13/<prop>.glb`. Pauses the local GLM worker for the VRAM and restarts it even on failure; skips outputs that exist, so a rerun resumes. ~135 s per prop. | workstation 4080 |
| `bake_textures.py` | The **bake**: composes the container 2×2 and the crate/ammo 2×1 atlases, cuts the usable patch out of the three pictures the generator would not draw flat, downscales everything to the bundle budget and writes 8-bit RGB PNG into `assets/textures/v13/` — beside-then-rename, so a concurrent `include_bytes!` never reads half a file. Re-opens every output to assert the mode and size the engine will see, prints the set's total, and treats a missing raw picture as an error naming it, never a placeholder. | workstation, the Hunyuan venv's python (has PIL) |

Both generator scripts skip a picture whose file already exists, so a rerun resumes rather than re-billing the GPU.

**Two generators, one 4090, and why the fetch goes through `/history`**

adler has one card and two ComfyUI instances that both want it: the **Qwen-Image** instance on `:8188` (the Path A generator, reached from the workstation through a local relay at `127.0.0.1:9188`, which is what `gen_views.py`'s `COMFY_API` defaults to) and an **Ideogram 4** ComfyUI on `:8288` run by another account, which is what the picture-generator connector drives. They cannot run at once — only the Ideogram instance fits in VRAM while the other is resident — so v13 generated everything through the connector.

That is also why there is a fetch script at all. The connector answers with a **gallery URL that needs a key this repo does not hold**, so the pictures cannot be downloaded the obvious way. But `:8288` is a ComfyUI: its `/history` carries the full positive prompt of every job and `/view` serves the output, and both are readable on adler's loopback. `fetch_pictures.py` therefore lists the history over `ssh adler curl`, matches each job's longest `CLIPTextEncode` text (the negative prompt is the short one) against the prompt tables in `gen_textures.py` and `gen_views.py` by containment, and writes the match to the name those tables imply. **The prompt is the identity of the picture** — regenerate with the same prompt and it lands on the same filename, so delete the old file first if you want the new one.

**"A detailed picture on every polygon", honestly**

The ask behind v13 was a detailed picture on every polygon. This renderer samples **one texture per mesh** and has no second UV set, no atlas sharing and no decals, so that ask is met in the only two ways it can be:

- **Atlas boxes** for cover. One picture per *face* of the box, composed into a single texture — a 2×2 grid of 1024² tiles for the container (side, doors, roof, floor), a 2×1 for the crate and the ammo box — with the six faces UV-mapped into their own rectangles in `MeshData::textured_box`'s face order. The box is then scaled to the obstacle's real size and the picture stretches with it, which is *correct* for a container (one door picture per door) and is exactly why containers are drawn at real proportions rather than stretched to fit a gameplay volume.
- **Box-projected material pictures** for the generated props. A Hunyuan shape output is POSITION-only — no NORMAL, no `TEXCOORD_0`, no material — so the same treatment `crates/fire/src/meshes.rs` applies is used: face normals recomputed, planar UVs projected per dominant axis, and the prop's own generated material picture (`limestone` for the cathedral, `bronze` for the statue, `burlap` for the sandbags, `cast-iron` for the lamp, `scorched-steel` for the wreck) sampled through them. Every face gets a photographic surface at roughly the right scale; no face gets a *unique* picture, and this document is not going to pretend otherwise.

Anything beyond those two — per-face detail maps, normal or roughness maps — is a renderer change, not a pipeline change. See the engine-side constraints above.

## Conventions: axes and the hold point

- **Blender +X forward** (muzzle, face, front), **+Z up**. Export `export_format="GLB", export_yup=True`.
- That lands as **engine +X forward, +Y up, +Z right** — so Blender +Y is engine **left**.
- Points map **(x, y, z) → (x, z, −y)**.
- **A box does not map componentwise.** That mapping negates an axis, which swaps min and max on it. Map both corners then re-derive min/max, or you emit a box with `min > max`.
- **The origin is the hold point.** `push_parts` draws every part at one position with unit scale, so the GLB's origin *is* where the thing is held or stood — the grip for a weapon, between the feet for a character.
- Match the established scale: the viewmodel pistol is ~0.9 units long.

## The node-name contract

Names are load-bearing. `load_assets` in `crates/arena/src/online.rs` classifies on them:

| Name | Meaning |
|---|---|
| `arm*`, `hand*` | Viewmodel-only — **not** drawn on remote players |
| anything else | Gun geometry — drawn in the viewmodel **and on every remote player** |
| exactly `strip` | Takes the per-weapon-level accent colour |

Miss a rename and every remote player sprouts your prop, silently. Verify the names **in the exported GLB**, not in Blender.

## Pivots do not survive import

`load_glb` **bakes each node's world transform into its vertices and discards the hierarchy**. After import nothing can rotate about anything: a slide cannot cycle, a hammer cannot fall.

So any prop that will be animated per-part ships a **sidecar JSON** beside the GLB carrying each part's pivot in engine space — `swat-rig.json` is the precedent, written by `swat_split.py`, read by `rig.rs`.

**Key order in that sidecar used to matter, and the reason is worth keeping.** `rig.rs`'s reader is a substring scanner. It originally found `"<name>"` and took the **next** `[ … ]`, so a bare list of part-name strings written before the pivot map made every part's first match land inside that list — and every joint then silently resolved to the *first* pivot in the map, with no parse error. `swat-rig.json` escaped it only because its `"parts"` list happens to come last.

Fixed in `bbe82c3`: the key must now be followed by a colon and an array, and every occurrence is tried, so order cannot matter. `rig_json_joints_resolve_whatever_the_key_order` pins it.

The hazard is recorded because the parser is still a scanner rather than a real JSON reader, and the next format written against it can reintroduce the shape. Cheap insurance: give a list of part names a key that cannot collide with a pivot key — `part_order`, not `parts`.

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
