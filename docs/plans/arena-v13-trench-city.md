# Arena v13 — "Trench City"

The graphics and map update. One authored arena replaces the seeded box field; the cover becomes real objects (closed shipping containers, wooden crates, ammunition boxes, sandbags, trench walls, covered tunnels); the fight is set in the square of a beautiful old European city that is being fought over; and the pipeline that produced every picture and every mesh is checked in under `tools/v13/` so the next map can be made the same way.

Written before any of it was built, as the coordination surface for the workers building it in parallel. Every constraint below was verified in the tree first; every number in §4 is the number that ships unless a test proves it wrong.

## 1. What is being asked, in this engine's terms

| Ask | What it means here |
|---|---|
| "hyper realistic" | The scene pass samples **one base-colour texture per mesh**, has no normal/roughness maps, no shadows, no blending, and until this update no mipmaps. So realism has exactly three levers: (a) photographic albedo textures with the shading detail *painted in*, (b) real geometry where a box reads as a box, (c) lighting and atmosphere the shader already has (Blinn-Phong, hemisphere ambient, ACES, fog). This update pulls all three, and adds mipmaps so the textures stop shimmering at distance. It will look like a well-textured, well-lit game of the early-2000s generation; it will not look like a PBR renderer, and this document says so rather than promising otherwise. |
| "check the boxes, make them containers and closed" | The sim's tall cover class becomes `Cover::Container`: a closed 40-ft container at real proportions (2.4 wide, 2.6 tall, 6.0 long), drawn as a six-face **atlas box** whose doors, sides and roof are separate pictures. |
| "wood boxes and munition boxes to jump on, the smaller connecting parts" | Two new low classes, `Cover::Crate` (1.2 tall) and `Cover::Ammo` (0.55 tall), placed as **climbing chains**: floor → ammo box → crate → container roof. The jump apex is 1.76 (`JUMP_VEL²/2g`), so each step is reachable from the one below and no container roof is reachable from the floor directly. A test proves each chain by stepping the sim's own `move_circle`/`step_vertical`. |
| "tunnels, trench warfare, cover options" | Two new obstacle semantics. `Cover::Wall` is a 2.5-tall hard-cover line from the floor; a corridor between two walls is a **trench**, with `Crate` fire steps against the outer wall so a player standing on one (eye 1.45 + 1.2 = 2.65) sees over it — and, one hop up, stands on it (§9). `Cover::Roof` is the first obstacle with a **bottom**: `Obstacle.base` > 0. A roofed trench section is a **tunnel** — walkable underneath, standable on top, opaque to bullets through it. |
| "optimize for 8 players" | Four-fold rotational symmetry, eight spawns two per side with sandbag cover at each, three concentric play bands (outer flank, trench ring, inner square) so eight players spread instead of piling into one lane, and four weapon pads all inside the tunnels so the tunnels are contested. |
| "a heaven of a beautiful city, a background" | A **sky cylinder** with a generated golden-hour panorama, a ring of generated Haussmann and art-nouveau façades outside the wall, a cathedral behind the south wall, a bronze equestrian statue on a plinth at the centre. All decor is client-only and listed in the `Level` so every client draws the same city. |
| "picture generator → 3D generator → attach detailed pictures to every polygon" | Path A of `docs/asset-pipeline.md`, run through a new checked-in runbook (§6): four orthographic concept views per prop from the picture generator, Hunyuan3D-2mv for the mesh, then a generated **material picture per prop** attached by box projection onto every face (the same triplanar the Fire Racer props use, with a per-prop picture instead of one stone tile). "Every polygon" is honest about the renderer: one picture per mesh, mapped so every face of the mesh gets a photographic surface at the right scale. |

## 2. Why this is a protocol bump (v12 → v13)

`Level` existed since `fcc4c68` but nothing consumed it: `Sim::new` takes a seed and every peer regenerates the same random boxes. Shipping an authored arena means the server and every client must agree on a **different** obstacle set, and `Obstacle` grows a `base`. Apply the rule from `CLAUDE.md`: what does an old peer *do* when the field is absent? A v12 client joining a v13 server would predict its movement against the seeded boxes while the server resolves it against the trench city — every wall would be either invisible or imaginary. That is "plays a different game", so `PROTO_VERSION` goes to 13. `GameJoined` gains `map: String` (`#[serde(default)]`, so the frame decodes everywhere; the bump is what stops it being *used* across versions), and `Level::named(map, seed)` resolves it — `"trench-city"` today, the seeded arena for any other value, so a future map is an additive string rather than another bump.

The join gate is exact equality, so frozen hub builds v7–v12 go list-only the moment the server moves, exactly as at every previous bump. The page entry for v13 is staged on `main` alongside the bump; `deploy-pages.sh` ships it in the same run that the server redeploy makes it joinable.

## 3. The sim change: an obstacle with a bottom

`Obstacle { min, max, h }` becomes `Obstacle { min, max, h, base, kind }`. `base` is the bottom of the box (0 = on the floor, which is every obstacle that existed before), `kind` is `Cover`, cosmetic to the sim and load-bearing to the client. Both are `#[serde(default)]`; a v12 `Level` JSON still decodes.

Semantics, each with a test:

- **Horizontal blocking** (`blocked`): a box blocks a player whose feet are below its top by more than `STEP_UP` — *and* whose head is above its bottom. `y < h − STEP_UP && y + BODY_H_STAND > base`. For `base = 0` the second clause is always true, so nothing that existed changes.
- **Support** (`support_height`): a box supports a player only if their feet are at or above its bottom: `base <= y + 1e-3`. A player walking through a tunnel at `y = 0` is on the floor, not on the roof. New `y` parameter; `step_vertical` passes its own.
- **Ceiling** (`step_vertical`): if the player overlaps a box, their feet are below its bottom, and the step would carry their head (`y + BODY_H_STAND`) above `base`, clamp `y = base − BODY_H_STAND` and zero `vy`. A jump inside a tunnel bonks; a player on a container cannot jump *up into* the underside of anything.
- **Bullets**: a box stops a round whose vertical span over the tick intersects `[base, h]`: `y0.min(y1) < h && y0.max(y1) > base`. Still conservative (span, not segment), same as today. The same test also runs at the point of **contact** with a body, with the span up to it, before the hit is credited: a round that meets a body from inside a roof slab (climbing at the player standing on it, or dropping at the player under it) is stopped by the slab, while a point-blank hit on a body backed against a wall — whose tick *ends* inside the wall — still lands.
- **Pads**: a pad is taken with the feet below `PAD_PICK_H` (1.0), under every roof base, so a player on a tunnel roof does not collect the pad through the slab; a hop over a pad on open floor takes off and lands below it and still grabs it.
- **Lag-compensated hits** are untouched: the target's body has always been a column from its feet, and feet are wherever `support_height` put them.

`Sim::from_level(&Level)` builds a sim from a level; `Sim::new(seed)` becomes `from_level(&Level::from_seed(seed))` so every existing test keeps its arena. `Sim` carries the level's `spawns`, and `add_player` and respawn both go through `Level::spawn(slot)` semantics (wrap, fall back to the golden ring when empty). Pads come from `Level.pads`; `Level::from_seed` fills them from `generate_pads(seed)` so the seeded arena is byte-identical to before.

The editor's document is the `Level`, so `ember-editor` gains: `base` from an object's lifted position (the `FloatingObject` refusal becomes representable and goes away, with its test rewritten to prove the lift round-trips), and `kind` from the palette entry (`Cover::Crate` for "crate", `Cover::Container` for "container", a `Cover::Roof` entry with a default `base`, the rest `Cover::Wall`). `from_level` places a raised box at `base + h/2`.

## 4. The layout

Frame: engine +X right, +Z toward the camera's south, `ARENA_HALF = 24.0` (unchanged — the shared `blocked` reads the constant, and 48 m with three bands is dense enough for eight). Four-fold symmetry: define the **north side** below, then rotate by 90° about the origin three times, `(x, z) → (−z, x)`. AABB extents swap under rotation; the level builder maps both corners and re-derives min/max (the componentwise mistake `docs/asset-pipeline.md` warns about).

All heights: floor 0. All boxes `base = 0` unless stated.

### 4.1 Centre (not rotated)

| Kind | x | z | base | h | Note |
|---|---|---|---|---|---|
| Plinth | −1.6..1.6 | −1.6..1.6 | 0 | 2.2 | The statue's granite base. Not reachable from the floor (apex 1.76), reachable from the sandbags (1.1 + 1.76). King-of-the-hill. |

### 4.2 North side (rotate ×4)

**Inner square dressing**

| Kind | x | z | h | Note |
|---|---|---|---|---|
| Sandbag | −2.0..2.0 | 4.6..5.4 | 1.1 | Four of these make a sandbag square around the statue. |
| Rubble | −8.0..−6.0 | 7.6..9.6 | 0.7 | Low cover near the inner wall. |

**Trench ring** (corridor between the walls is z 11.0..14.0, 3.0 wide — a 1.2-wide crate leaves 1.8, the player is 1.2 across)

| Kind | x | z | h | Note |
|---|---|---|---|---|
| Wall | −11.0..−1.2 | 10.6..11.0 | 2.5 | Inner wall, west segment. |
| Wall | 1.2..11.0 | 10.6..11.0 | 2.5 | Inner wall, east segment. The 2.4 gap at the centre is the tunnel's inner mouth. |
| Wall | −14.4..−7.2 | 14.0..14.4 | 2.5 | Outer wall, west. |
| Wall | −4.8..4.8 | 14.0..14.4 | 2.5 | Outer wall, centre. |
| Wall | 7.2..14.4 | 14.0..14.4 | 2.5 | Outer wall, east. The two 2.4 gaps at |x| 4.8..7.2 are the outer entrances. |
| Roof | −6.0..6.0 | 11.0..14.0 | base **2.5**, h 2.9 | The tunnel. 12 m long, clearance 2.5 (standing body 1.86). |
| Crate | −9.6..−8.4 | 12.8..14.0 | 1.2 | Fire step against the outer wall. |
| Crate | 8.4..9.6 | 12.8..14.0 | 1.2 | Fire step. |
| Ammo | −3.4..−2.4 | 11.0..11.7 | 0.55 | Low cover inside the tunnel, inner side. |
| Ammo | 2.4..3.4 | 13.3..14.0 | 0.55 | Low cover inside the tunnel, outer side, staggered. |

Walls meet at the corners under rotation (the outer walls of adjacent sides both cover `[14.0,14.4]²`), so the ring is closed except at its gaps; the corridors of adjacent sides both cover `[11,14]²`, so the ring is connected around the corners.

**Outer flank** (z > 14.4)

| Kind | x | z | h | Note |
|---|---|---|---|---|
| Container | −3.0..3.0 | 19.2..21.6 | 2.6 | Blocks the spawn-to-tunnel sightline. |
| Crate | 3.4..4.8 | 19.6..21.0 | 1.2 | Chain step for the container above. |
| Ammo | 5.2..6.2 | 19.8..20.6 | 0.55 | Chain start. |
| Container | 14.8..20.8 | 15.0..17.4 | 2.6 | Corner container, along x. Its near corner sits on the diagonal `x + z = 30` between this side's east spawn (9, 21) and the next side's (21, 9), so the adjacent-corner spawns cannot see each other past the wall corner; the flank still connects around its east end (3.2 to the boundary). |
| Crate | 13.2..14.6 | 15.4..16.8 | 1.2 | Its chain step. |
| Ammo | 11.8..12.8 | 15.6..16.4 | 0.55 | Its chain start. |
| Container | 17.0..23.0 | 20.4..22.8 | **5.2** | Stacked pair: a corner landmark, unreachable. (It sits behind the spawn line, not on the corner diagonal — the corner container is what blocks that sightline.) |
| Sandbag | −11.0..−7.0 | 17.6..18.4 | 1.1 | Spawn cover. |
| Sandbag | 7.0..11.0 | 17.6..18.4 | 1.1 | Spawn cover. |

**Spawns**: (−9, 21) and (9, 21). Rotated: eight distinct points, all ≥ 2.6 from any box edge, pairwise ≥ 16 apart.

**Pads**: (0, 12.5) — inside the tunnel. Rotated: all four pads are tunnel pads.

### 4.3 Decor (client-only, carried in `Level.decor`)

| Kind | Where | Scale | Note |
|---|---|---|---|
| Statue | (0, 2.2, 0) | ~4.0 tall | On the plinth. |
| Cathedral | (0, 0, −46), facing +z | ~34 tall | Behind the south wall; the skyline hero. |
| FacadeA ×6, FacadeB ×6 | ring, radius 44, every 30°, skipping the cathedral's slot, facing inward | ~18 tall | The city. |
| Lamp ×8 | (±26, 0, ±12), (±12, 0, ±26) | 5 tall | Just outside the wall. |
| Wreck ×4 | (±27, 0, ±27), yaw 45° | ~4.4 long | Corner dressing. |

Plus, drawn by kind rather than listed: every `Sandbag` obstacle is drawn as the generated sandbag mesh scaled to its box; the sky cylinder (radius 60, y −5..70, normals forced to +Y so it lights evenly, colour over-driven 1.6); a far ground plane (200 × 200, cobble tiled 40×, dimmed 0.55) closing the void beyond the wall; the arena boundary as the `city-wall` balustrade picture instead of basalt.

### 4.4 Invariants (tests in `arena-core`)

1. Exactly eight spawns, all inside `ARENA_HALF − PLAYER_R`, none overlapping any box (circle test, `PLAYER_R`), pairwise ≥ 16 apart (the map delivers 17.0).
2. Every box inside the arena; every `base < h`; every `Roof` has `base ≥ CONTAINER_MIN_H` (a roof you can walk under is at least container height off the floor).
3. Four-fold symmetry: rotating the obstacle multiset by 90° yields the same multiset (compare sorted, with a 1e-4 tolerance).
4. Every `Container` at `h = 2.6` with `base = 0` has a `Crate` within 0.5 of one of its faces and an `Ammo` within 0.5 of that crate — and a player driven by the sim's own step functions climbs floor → ammo → crate → container roof (jump on each step, walk forward) and ends with feet at 2.6.
5. A player walks the full length of a tunnel at `y = 0` without being blocked; a jump inside it caps at `base − BODY_H_STAND`; a level shot at chest height passes along it; a shot fired from the roof top down through the roof is stopped; a player dropped from above lands **on** the roof at `h`.
6. From a fire step the eye (`EYE_STAND + 1.2`) is above the outer wall; from the floor it is not.
7. Pads: four, none inside any box, all under a roof.
8. `Level::trench_city()` serialises and deserialises to itself; a v12-shaped JSON (no `base`, no `kind`, no `pads`, no `decor`) decodes with the defaults.
9. `Level::from_seed(s)` produces the same obstacles, pads and spawns as v12 did for every seed in the existing tests (unchanged expectations).
10. No spawn sees another spawn: level fire from the floor at any spawn toward any other, both ways round, is stopped by cover (the sim's own shot, so the per-tick sampling is the real one).
11. The surface graph §9 accepts is pinned: from a fire step one hop rests on the outer wall top, from there one hop across the corridor rests on the inner wall top, and from the fire step one hop rests on the tunnel roof — driven by `move_circle`/`step_vertical` with a single jump.
12. A pad is taken from the tunnel floor and not from the roof over it; a round through the roof slab reaches neither the player standing on it (30° from the tunnel floor) nor the player under it (−`MAX_PITCH` from the roof), and both connect with the roof removed.

## 5. The client

New module `crates/arena/src/props.rs` owns everything v13 draws that is not a player or a bullet:

- `Prop` — an enum, one variant per registered mesh, in a fixed order; `prop_meshes()` returns them in that order and `ShooterGame::set_props(base)` records where they landed, following the `set_env_base`/`set_backdrop` pattern in `crates/arena/src/lib.rs`.
- **Atlas boxes.** `atlas_box(faces: [Rect; 6], texture)` builds a unit box whose six faces each map to a rectangle of one texture, using the same `CUBE_FACES` order as `MeshData::textured_box`. The container atlas is a 2×2 grid of 1024² pictures baked to one 2048² (side, doors, roof, floor); crate and ammo use a 2×1 (side, top). Because atlases are non-tiling, the box is scaled to the obstacle's real size and the picture stretches with it — which is right for a container (one picture per face) and why containers are drawn at their authored proportions.
- **Tiled boxes** for `Wall`, `Roof` (underside picture), `Rubble`, `Plinth`, using `textured_box` with tiles chosen so ~1 tile ≈ 1.5 m.
- **Generated props.** Each GLB in `assets/models/v13/` is POSITION-only (Hunyuan shape output); load through the same `face_normals` + `planar_uvs` treatment `crates/fire/src/meshes.rs` applies, with the prop's own material picture as the texture. Lift those two functions into `ember_engine::assets` (they are engine-shaped and now have two consumers) rather than copy them.
- **Draw by kind** in the obstacle loop: container/crate/ammo → atlas mesh; wall/roof/rubble/plinth → tiled mesh; sandbag → the sandbag GLB scaled to the box. A raised box is drawn at `base + (h − base)/2` with height `h − base`.
- **Sky and ground** as above; the factory skyline (`level-backdrop.glb`) is no longer drawn by the arena and its `include_bytes!` goes, saving 1.4 MB of bundle.

Bundle budget: every texture is embedded. Target ≤ +8 MB over v12's 18.4 MB, checked after `wasm-bindgen`: atlases at 2048² only for the container, 1024² for crate/ammo/floor/sky, 512² for everything else, all 8-bit RGB PNG.

## 6. The asset pipeline, checked in

`tools/v13/` is the runbook; unlike the earlier `.claude/*` runbooks it is committed, so the next map does not start from a chat transcript.

| Step | Script | Where it runs | Measured |
|---|---|---|---|
| Material pictures | `gen_textures.py` (prompts) → picture generator | adler 4090 | ~30–42 s per 1024² |
| Concept views (4 per prop, 7 props) | `gen_views.py` (prompts) → picture generator | adler 4090 | ~32 s per view |
| Fetch | `fetch_pictures.py` — pulls finished pictures off the generator's ComfyUI `/view` endpoint over ssh, names them by prompt | workstation | — |
| Mesh | `mesh-props.ps1` → `C:\hy3d\gen3d_mv.py`, decimated twice (30k then 6k faces) | workstation 4080, GLM worker paused | ~135 s per prop |
| Bake | `bake_textures.py` — composes atlases, downscales to budget, writes 8-bit RGB into `assets/textures/v13/` | workstation | seconds |

Two generators share adler's 4090 and cannot run at once: the ComfyUI Qwen-Image instance on :8188 (the Fire Racer path) and the Ideogram 4 instance on :8288 behind the picture-generator connector. Only the latter fits while the other is resident, so v13 uses it for everything and pulls results through its history — the connector's gallery URL needs a key this repo does not hold. Recorded in `docs/asset-pipeline.md`.

## 7. Engine changes (separate commits, each pixel-identical for every other game by default)

1. **Mipmaps.** Texture upload builds a full CPU box-filtered chain, `mip_level_count = n`, sampler `mipmap_filter: Linear`. Removes the distance shimmer the `CLAUDE.md` table warns about; +33% texture memory. The table row and `docs/asset-pipeline.md` bullet are rewritten.
2. **Per-frame fog.** `Frame` gains `fog: Fog { color, density }` with `Default` equal to today's constants, uploaded in the scene uniform. The arena sets a warm golden-hour haze so the sky cylinder reads bright instead of navy; pong, fire and the arena's own v12 look are unchanged unless they set it.

## 8. Verification, in order

1. `cargo test --workspace --exclude linter --no-fail-fast` — 36 suites, ≥ 320 passing, plus every new test in §4.4.
2. `cargo clippy --workspace --all-targets` under the workspace's deny-warnings lints.
3. `cargo build --target wasm32-unknown-unknown --release -p arena --lib` + `wasm-bindgen`; bundle size reported against the budget.
4. Native run against a local `arena-server`: `EMBER_CAM` overview screenshot and three eye-height screenshots (tunnel mouth, container chain, spawn) reviewed by eye and attached to the commit message by path.
5. `wsbot` two-bot run against the local server: join, move, shoot; no panics, states flowing.
6. Deploy: `deploy/deploy-pong-online.sh` then `deploy/deploy-pages.sh`, in that order (server first so the staged v13 page is joinable the moment it lands).

## 9. Deliberately not done

- Trenches that are *below* the floor. The floor is `y = 0` in a dozen places (landing, bullet floor test, eye height); a sunken corridor is a `Level` with a heightfield, which is a different project. Walls above the floor give the same gameplay.
- Per-face detail beyond one picture per mesh, and normal/roughness maps. The renderer has one sampler; see §1.
- OBB collision (rotated containers). AABB only; the container along z is a different AABB, not a rotated one.
- A bigger arena. `ARENA_HALF` is baked into the shared `blocked`; making it per-level is a `move_circle` signature change on both peers and was not needed for eight.
- Wall tops as hard cover from everywhere. A fire step has to satisfy `step + EYE_STAND > wall_h` to see over the wall (1.2 + 1.45 = 2.65 > 2.5) and would have to satisfy `step + apex < wall_h − STEP_UP` (1.2 + 1.69 < 2.15) to keep the wall unmountable; no wall height does both, because `EYE_STAND < apex + STEP_UP`. So a hop from any fire step rests on the outer wall top, one more hop across the corridor rests on the inner wall top, and a hop from the fire step toward the tunnel rests on its roof — a walkable line at eye height 3.95 overlooking every band. Accepted for v13 and pinned (§4.4 item 11): walls are hard cover from the floor and a standing surface from a fire step; the container class still promises only that the floor is not enough. Making wall tops unstandable is a shared sim rule (`support_height` ignoring `Cover::Wall`, plus keeping an airborne body out of a wall's footprint) and therefore a protocol question for a later bump, not a table edit.
