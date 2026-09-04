# ember — rules for anyone working in this repo

Read this before touching the engine or an asset. Everything here was paid for once, in a bug; the point of writing it down is that it is not paid for twice.

Depth lives in linked docs, loaded only when the task needs them. This file stays short on purpose: it is loaded into **every** session in this repo, so it carries constraints and pointers, never essays.

## The one-way layering

`game → scene/simulation → renderer → platform`. Nothing reaches back up. The renderer owns wgpu and nothing above it touches the GPU.

## What ships here

Every game in this repo runs on ember: Rust against the layering above, compiled to one wasm bundle, drawn by the renderer whose constraints are listed below, and listed in the launcher (`web/games.json`) only once it does. A game that brings its own engine, its own runtime or its own build toolchain is not merged and is not put in the launcher, however finished it is: two engines in one repo means two sets of constraints, two deploy paths and two things to keep alive.

Denied on this rule, 2026-09-04: the `verschaetz-dich` branch, a standalone Next.js app whose five commits replace the whole repository root. It stays on `origin` as its own branch, unmerged and unlisted. Port a game like that onto ember, or host it as its own project; do not land it here.

## Simulation rules

- **`crates/arena-core/` is shared between the client's prediction and the authoritative server.** A change there is a change to both. Assume every edit is a protocol question until you have proved it is not.
- Fixed 60 Hz, seeded RNG, fixed update order. Do not add a per-tick RNG: the only randomness today is two world-generation LCGs, and the first per-tick one has to be seeded and tick-indexed or replays and rollback die.
- **Bullets are stepped server-side only.** Clients never simulate their own. That is the sole reason the `f32` transcendental in the launch (`tan` for aim elevation) is safe in hit registration. Add client-side shot prediction and it becomes a desync source. (Obstacle heights used to be a `sin` hash too; since `fcc4c68` every box carries its own `h`.)
- **An obstacle has a bottom.** `Obstacle.base` (0 = on the floor) makes a roof a walkable slab with a tunnel under it. Four rules read it and they must agree: horizontal blocking, support, the ceiling clamp, and the bullet span test. Change one, change all four, and re-run the trench-city and freight-yard invariants in `arena-core`. Since v18 the ceiling clamp also reports which box it hit (`VStep.bonked`): that is the loot bonk, both peers predict it, and when two raised boxes overlap a footprint it picks the lowest base, so a box added over another changes what a jump under it does.
- Aim is a 2D unit vector **plus a scalar elevation**, never a 3D direction. Folding them collapses bullet range by `cos(pitch)` and freezes a player's facing on a near-vertical aim. See `docs/state-model.md`.
- Body geometry (`eye_h`, `body_h`, `hit_radius`) lives in `arena-core`, not the renderer. Client and server must agree where a body *is*.

## Protocol rules

- `#[serde(default)]` makes a new field **decode** across versions. Decoding is not working. Ask what an old peer *does* when the field is absent — if the answer is "plays a different game", bump `PROTO_VERSION`.
- The join gate is exact equality. Bumping makes frozen hub builds list-only; the lobby browser is unaffected (it sends `proto: 0` against an ungated `ListLobbies`).

## Renderer constraints — the ones that bite

The scene pass is deliberately small. Work with it, not against it.

| Constraint | Consequence |
|---|---|
| **One base-colour texture per mesh**, multiplied by the per-instance colour | No normal/roughness/metallic/AO/emissive. A PBR set arrives 6/7 useless. Push a textured part with `Vec3::ONE` or you double-tint it. |
| **8-bit `R8G8B8A8`/`R8G8B8`/`R8` only** | Anything else decodes to `None` **silently, with no log**. A 16-bit export ships an untextured model. |
| **Mipmaps are built at upload** (full CPU box-filtered chain, trilinear sampler) | Distant textures no longer shimmer, and texture memory is 4/3 of the picture's own size. Bake at the size the picture needs up close; the chain handles distance. |
| **One GPU texture per mesh id, cloned per primitive, never shared** | VRAM = parts x w x h x 4. Seven 2048² parts is ~112 MB for one prop. |
| **`cull_mode: None`** | No backface culling. Interiors of open shapes render solid. |
| **`BlendState::REPLACE`** | No additive or transparent anything. A muzzle flash can only be an opaque box. |
| **One scene pass, one depth buffer, one camera, near/far `0.1`/`500`** | There is no separate viewmodel pass, so the gun clips into walls and anything within 0.1 of the eye is clipped. Expect it; it is not a regression you introduced. |
| **`Instance`**: position, `Vec3` scale, colour, `rot: Quat`, mesh id | Scale applies **before** rotation. Normals use the same matrix, so non-uniform scale skews lighting. |

The device floor is WebGL2 plus `EXT_color_buffer_float`, nothing else: **`docs/minimum-requirements.md`**. A feature that needs more is a project decision recorded there first.

`include_bytes!` bakes assets into the wasm bundle and the deploy ships one bundle with no runtime fetch — every byte you add to an asset is a byte every web player downloads.

Mesh ids are allocated in a fixed order in `crates/arena/src/lib.rs`. Adding parts shifts every later base; the `set_*` setters exist to absorb that.

## Assets

**glTF/GLB only.** There is no FBX importer anywhere in the workspace, and adding one is not the answer — the pipeline converts offline. Full recipe, the four production paths, the silent-failure modes and the conventions: **`docs/asset-pipeline.md`**. Read it before converting anything.

Large artist source stays **out of git** (see `.gitignore`); only the converted `.glb` and its sidecar are committed.

## Working rules

- **Measure and report wall time.** An action that did not say how long it took is a process bug.
- **Builds run at minimum priority** so they never contend with the interactive session.
- Markdown paragraphs run: one line per paragraph, soft-wrapped by the viewer. Never hard-wrap prose.
- LF line endings, UTF-8 without BOM, everywhere. Never let a host tool write UTF-16 or a BOM into the tree.
- **State plainly what was and was not verified.** "Compiles" and "reviewed by reading" are different claims. A commit whose work could not be built says so in its message.
- Follow-ups go in `docs/plans/backlog.md`, one line each.

## Coordination between workers

The repo is the medium, not the chat. See **`docs/worker-protocol.md`**.
