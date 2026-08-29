# The model pipeline

How a model becomes something ember can draw, and the specific ways that goes wrong without telling you.

*Scope: authoring and porting 3D assets. The renderer constraints this document works against are summarised in `CLAUDE.md`; this is the operational half.*

## 1. The shape of it

```
artist source (.fbx, .glb, sculpt)          NOT in git — large, and never loaded at runtime
        |
        |  Blender, headless, scripted        tools/*_convert.py, tools/make_*.py
        v
assets/models/<name>.glb  +  <name>.json     committed: the mesh, and its pivots
        |
        |  include_bytes!                     crates/pong/src/online.rs
        v
engine meshes + part list                    one mesh id per part, one texture per mesh id
```

Two rules follow from the diagram and are worth stating before any detail. **The conversion is offline and scripted** — there is no FBX importer in the engine and there will not be one; a model that cannot be converted cannot ship. And **every byte in `assets/models/` is a byte every web player downloads**, because `include_bytes!` bakes them into the wasm bundle and the deploy ships one bundle with no runtime fetch.

## 2. Conventions

Get these wrong and the model is sideways, mirrored, or inside the player's head.

- **Blender: +X forward (muzzle/face), +Z up.** Export with `export_format="GLB", export_yup=True`.
- That lands as **engine +X forward, +Y up, +Z right**. So Blender +Y is engine **left**.
- Point coordinates map **(x, y, z) → (x, z, −y)**.
- **A box does not map componentwise.** That mapping negates an axis, which swaps min and max on it. Map the corners then re-derive min/max, or you emit a box with `min > max`.
- **The origin is the hold point.** `push_parts` draws every part at one position with unit scale, so the GLB's origin *is* where the thing is held or stood. For a weapon that is the grip; for a character it is between the feet.
- Scale to the established size: the viewmodel pistol is **~0.9 units** long.

## 3. The node-name contract

Names are not decoration; the runtime classifies on them (`load_assets` in `crates/pong/src/online.rs`).

| Name | Meaning |
|---|---|
| `arm*`, `hand*` | Viewmodel-only. **Not** drawn on remote players. |
| anything else | Gun geometry — drawn in the viewmodel **and on every remote player**. |
| exactly `strip` | Gets the per-weapon-level accent colour. |

Forget a rename and every remote player sprouts your prop. There is no error.

## 4. The five silent failures

Each of these produces a model that loads, draws, and is wrong. None logs anything. A conversion script's real job is to make them loud.

1. **UV layer name mismatch.** Joining meshes merges UV layers **by name**, and glTF exports only `TEXCOORD_0`. Mismatched names collapse the atlas and the whole model samples a single texel — a flat blob in one colour. *Collapse every mesh to one layer named `UVMap` before any join.*

2. **Wrong bit depth.** The loader decodes **only** 8-bit `R8G8B8A8`/`R8G8B8`/`R8` and returns `None` otherwise. A 16-bit re-export from Blender ships an untextured model. *Verify the exported PNG's IHDR, not the source's.*

3. **An unconnected base colour.** Artist FBXs routinely ship textures unlinked, or link only the normal map. A naive import→export then yields a correctly-shaped, entirely untextured model. *Wire base colour explicitly and assert the image datablock actually decoded — a 0x0 image is "loaded".*

4. **A downscale silently undone.** Blender's glTF exporter will copy the **original file bytes** for an on-disk image it believes is unmodified, shipping the full-resolution texture you thought you shrank. *Pack the scaled image, then re-parse the exported PNG and fail if it is bigger than intended.*

5. **A muzzle pointing backwards.** If the fit derives scale and axis from measurement, the length and longest-axis checks become tautologies — the script rotated the longest axis onto +X, so of course +X is longest. Only a heuristic picks *which end* is forward, and flipping it leaves length, longest axis and handedness all correct. *Assert the muzzle is forward of the origin and near the front of the bounds.*

A sixth, not silent but expensive: **texture budget**. VRAM is `parts × w × h × 4`, with no sharing between mesh ids, and the PNG also lands in the wasm bundle. 512² is the working default; justify anything larger.

## 5. Pivots do not survive import

The GLB loader **bakes each node's world transform into its vertices and discards the hierarchy**. Nothing rotates about anything after that — a slide cannot cycle, a hammer cannot fall, a limb cannot swing.

So any model that will be animated per-part ships a **sidecar JSON** beside the GLB carrying each part's pivot in engine space (`assets/models/swat-rig.json` is the precedent, `tools/swat_split.py` writes it, `crates/ember-engine/src/rig.rs` reads it).

**Key order in that sidecar matters.** `rig.rs`'s reader is a substring scanner: it finds `"<name>"` and takes the **next** `[ … ]`. If a bare list of part-name strings appears before the pivot map, every part's first match lands inside that list and the next `[` is the *first* part's pivot — so every part silently shares one pivot, with no parse error. **Put the per-part maps first, and never name a list of part names something that could collide with a pivot key** (`part_order`, not `parts`).

## 6. Animation is procedural

Artist models generally arrive with no bones and no animation curves — a static mesh, at best split into named parts. That is enough, and it is the shape to aim for: **separate parts + pivots = animation**, driven from state the sim already tracks (`cooldown`, `reload_t`, ammo transitions), not from baked clips.

The cost of separate parts is one mesh id and one texture upload each. Weigh it: a slide that must cycle earns its part; a screw does not.

Drive animation from **authoritative** signals, not input. A muzzle flash keyed to a held trigger fires when the magazine is empty; keyed to a free-running clock it fires when nothing was shot. Key it to the server-confirmed ammo decrement — and then qualify *that*, because a pad pickup and a respawn also reduce ammo.

## 7. Running a conversion

Blender is not on the host and must not be installed there. It lives in a `claude-blender` WSL distro, reading the repo through `/mnt/c`:

```bash
wsl -d claude-blender --cd 'C:\Users\Admin\dev\ember' -- bash -lc 'chrt --idle 0 ionice -c3 blender --background --python tools/9mm_convert.py -- assets/models/9mm.glb'
```

`chrt --idle 0 ionice -c3` is not optional: bakes are batch work and must never contend with the interactive session.

**The first run is a measurement run.** A conversion script cannot know the source's unit scale, its forward axis, where the grip is, or whether the parts share one atlas. Write it to *print* all of that, derive what it can, shout when the derivation disagrees with the hardcoded guess — then paste the measured values in and freeze them. Fit constants carry a `TODO(fit)` until they have been seen on screen.

## 8. Checklist

Before committing a converted asset:

- [ ] Node names satisfy the contract (§3), verified **in the exported GLB**, not in Blender.
- [ ] Exactly one UV layer, named `UVMap`, on every mesh.
- [ ] Exported PNG is 8-bit, and no larger than intended.
- [ ] Every primitive has `TEXCOORD_0`, a material, and a base-colour texture.
- [ ] `baseColorFactor` is white — the runtime multiplies its own per-instance colour in.
- [ ] Origin is the hold point; +X is forward; overall size matches the established scale.
- [ ] Sidecar written, per-part maps **before** any name list, muzzle/anchor points included.
- [ ] Raw source is gitignored; only the `.glb` and `.json` are staged.
- [ ] Wall time of the bake reported.
