# Milestone 2 — the engine editor

A level builder and a character inspector, reachable from the hub beside the games. Designed against the code, not against a wish: every constraint below was verified in the tree first.

*Nothing in this document has been built. Bites are ordered so each is independently gateable.*

## 1. The ask, and the three things about it that have to change

> "the game and engine should be separated … a button for games then the game HUD pops up, but also a page for the editor. if clicked engine there should be a character engine and a level builder. in the level builder you should be able to fly around … a side bar with geometric shapes … w should move them, e should rotate, r scale them … all three coordination axes shown in different colors … asset class objects and then characters to place spawn points"

**The sidebar cannot be engine-drawn.** `egui` sits under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` and `pub mod overlay` is `#[cfg]`'d out on wasm — so there is no in-engine UI on the web at all. Native is no better: `Overlay::run` builds a hardcoded ATW-rig window and `EmberGame` is one method with no UI hook, so a game cannot contribute a widget anywhere. The sidebar is therefore **page DOM on the web** talking to Rust through a `#[wasm_bindgen]` string bridge — the shape `start_online(config_json)` already uses — and **digit keys on native**. Both feed one command queue. This is not a compromise chosen over something better; it is the only shape that exists.

**W is already fly-forward.** Every client in this repo uses WASD to move. One key cannot be both move-forward and translate-mode. Resolved by modality: **WASD flies only while RMB is held**; released, W/E/R are the gizmo modes and LMB selects. One `if input.mouse_down(MouseButton::Right)` branch.

**"Fly around and place things" and "the game plays what you placed" are two projects.** The sim has no level: `Sim::new` takes a seed, `S2C::GameJoined` carries `{id, seed, arena_half, players}`, every client regenerates obstacles locally, and `obstacle_height` derives each box's height *by hashing its own min corner* — so an authored box cannot state how tall it is and has no door into the running game. Closing that needs `Obstacle.h`, a `Level` on the wire, `PROTO_VERSION` 8→9, and a server redeploy the backlog records as blocked. It is bite 12, and it is **landable but not shippable**. No work is thrown away over this: the editor's document *is* the sim's `Level` from bite 7, so the last bite is a wire change rather than a rewrite.

## 2. Decisions

| Question | Decision | Why |
|---|---|---|
| Fly-look input | Difference `cursor_ndc()` | It is *required* for picking, so it is already on the critical path. `mouse_delta()` comes from `DeviceEvent::MouseMotion`, and whether winit's web backend emits that without pointer lock is unverified. One pointer source cannot disagree with itself. |
| Level format | New `Level` in `arena-core`; leave `assets/layouts/arena.json` alone | That file belongs to `crates/game`, a different net stack whose `ARENA_HALF` is 20.0 against the shooter's 24.0, loaded via `std::fs` in a crate with no wasm deps — it has never reached a web player. `h` and `spawns` are **sim truth** and must live where the server and the wasm client both see them. |
| Editor crate | Separate `crates/ember-editor` | The live bundle is 18,438,122 bytes, 12,714,206 of it `include_bytes!` assets that gzip poorly because PNGs are incompressible. Someone who came to *look at the engine* should not download ~11 MB gz plus tungstenite, rustls and audio. A separate crate also gets its own mesh-id space, so it cannot shift the arena's `set_env_base`/`set_parts`/`set_backdrop` bases. |
| Axis colours | Over-drive the instance colour past 1.0 | The shader lights, ACES-tonemaps and fogs every fragment. A gizmo coloured `(1,0,0)` comes out dark red on unlit faces and washes toward fog with distance — defeating the entire point of "different colors to know where you place things". Instance colour is an unclamped multiplier; clamping happens inside `aces()`. |
| E = rotate, on a box with no rotation | Free yaw on decor; **90° snap** on anything that becomes an `Obstacle` | `Obstacle` is an AABB on XZ. A freely rotated collidable is unrepresentable without OBB collision in `overlaps`, `blocked`, `support_height` and the bullet test. A 90° yaw *is* representable, by swapping extents. The UI must show the snap happening rather than silently discard the rotation at save. |
| Gizmo occlusion | No overlay pass in v1 | One scene pass, one depth buffer, `depth_compare: Less`, `BlendState::REPLACE` — the gizmo is occluded by the world and cannot be ghosted. The overlay pass would fix this *and* retire the colour hack, but it changes `Frame`, the public game contract, in a renderer `docs/presenter-architecture.md` is mid-restructure on. Ship v1, measure, then argue. |
| Native or web first | Native (1–5, 7), web second (9–10) | Native needs zero web plumbing, zero deploy changes and no engine change, so it is a window a peer can open on day one. One command queue serves both, so neither shell is a throwaway. |

**The plan needs exactly one engine change, and it is a bug fix** (bite 8). Everything else — fly camera, picking, axes, W/E/R, selection, spawn markers — fits inside `EmberGame` with the renderer untouched.

## 3. Bites

| # | Title | Gate |
|---|---|---|
| 1 | `ember-editor`: a native window you can fly around in — grid, three colour-overdriven world axes, static boxes | display |
| 2 | Picking maths, headless: cursor ray and ray-vs-OBB | **peer** |
| 3 | Select, and the three-axis gizmo drawn | **peer** |
| 4 | Drag: translate, rotate, scale on the locked axis | **peer** |
| 5 | The command queue, the palette, and spawn markers | **peer** |
| 6 | `Level` in `arena-core`; `Obstacle` gains `h`. No proto, no wire, nothing observable changes | **peer** (disjoint from 1–5 — run in parallel) |
| 7 | The editor authors and exports a `Level` | **peer** |
| 8 | The one engine change: measure, then fix, the wasm aspect/cursor divergence | browser |
| 9 | The web shell: wasm entries, the DOM sidebar, the editor bundle | browser |
| 10 | Hub entry and deploy: `engine/` beside `games/` | deploy + browser |
| 11 | The character half: a proportions-and-gait inspector | display |
| 12 | The proto bump: an authored level reaches the sim. **Land it, do not deploy it** | peer for build; not shippable |

Bite 2 is the first consumer of `cursor_ndc()` and `aspect()` anywhere in the workspace. `docs/presenter-architecture.md` says of oracle O3 that it "protects a public contract that nothing currently consumes" — writing bite 2's test discharges that.

## 4. Deferred, with the reason

- **"A character engine" beyond bite 11.** The genuinely underspecified item. That noun phrase spans a proportions slider (~150 lines on code that already exists) to a keyframe animation authoring tool. Bite 11 builds the largest piece that exists today; anything past it needs the user to say which.
- **The unlit depth-cleared overlay pass.** The engine change worth wanting and worth not doing yet. Nearly additive — `Frame` derives `Default`, so the three exhaustive literals each need one `..Default::default()` line — but it should wait for the presenter split rather than be rebased through it.
- **New primitive meshes** (cylinder, wedge, sphere). The user said "geometric shapes"; the engine has exactly one — the built-in unit cube. An honest gap this plan does not close.
- **`just_pressed` in `InputState`.** The editor tracks it in five lines and is the only consumer. Adding it to the engine means settling its semantics against the two-consumer latch `docs/input-latch.md` owns.
- **Level upload from the browser.** No runtime asset fetch and a static Pages site: v1's round trip is *the editor exports, a human commits the JSON, the server redeploys*. Stated plainly rather than implied away.
- **OBB collision in the sim**, the alternative to the 90° snap. Larger and riskier than the level format itself.
- **Spawn yaw.** `add_player` hardcodes `aim: [1.0, 0.0]`, so an authored facing has nowhere to land. Wire it or omit the field — do not ship a yaw the game ignores.
