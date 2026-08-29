NOTE 2026-08-29: written against the v6-era tree; superseded in part by upstream Arena v7 — re-framing into architecture docs is in progress.

# Adoption survey — ember at the threshold of roadmap item 1

A read-only survey of the whole workspace (7 crates, 23 `.rs` files, 5825 lines of Rust, plus four web pages and three deploy scripts), taken to establish what actually exists before roadmap item 1 (first triangle → textured cube → fly camera, ATW stage B) is designed on top of it. Nothing was built or run; every claim below is a claim about source text, with `file:line` references.

The governing constraint is `docs/atw-first-rendering.md`, which is adopted policy. The survey pays particular attention to the gap between what that document requires and what the code does, because roadmap 0.6 claims stage A of the warp ladder is complete.

**Delta note — re-verified 2026-08-28 against `7588841`.** The survey was first written against `d3c6f48`. Three commits landed after it: `7e1cac0` "Turn online mode into a drop-in arena shooter", `224bbd2` "Fix 13 confirmed shooter review findings (proto v3)", and `b4a9ad1` "Games hub: version catalog, live-lobby showcase, auto-created accounts" — about 1.9k inserted lines, concentrated in `pong-core` (new `shooter.rs`, proto v3), `pong-server`, `pong/src/online.rs`, and `web/`. A workspace-wide `rustfmt` pass and a run of clippy-lint fixes (`5745954` through `7588841`) landed on top of those while this re-verification was in progress; they changed no behaviour but moved most line numbers, so every reference here is pinned to `7588841` and will need re-checking after any future formatting pass. The engine moved by 108 lines: `InputState` gained mouse buttons, an absolute cursor position in NDC, and the viewport aspect; `Instance` gained a yaw. Every `file:line` reference below has been re-checked and drifted ones corrected. The substantive findings about the presenter, the `SceneFrame` type, the ring, the guard band, the two camera reads, and the late input latch are all unchanged — nothing upstream landed any of them. §3 is the section the delta genuinely falsified, and it has been rewritten to match; §5 has lost two entries that upstream fixed.

## 1. Per-crate map

### `ember-engine` — 1021 lines Rust + 84 lines WGSL

The engine library: platform layer, GPU layer, input. Dependencies are `winit`, `wgpu`, `glam`, `tracing`, `bytemuck`, `web-time`, plus `pollster`/`tracing-subscriber`/`tracing-log` on native and the wasm-bindgen/web-sys stack on wasm (`crates/ember-engine/Cargo.toml:6-28`). It depends on no other workspace crate.

Public surface (`crates/ember-engine/src/lib.rs:9-30`): module `renderer`; re-exports `run`, `EngineConfig`, `init_diagnostics` (native only), `InputState`, `Camera`, `Frame`, `Instance`, and convenience re-exports of `glam`, `winit::event::MouseButton`, and `winit::keyboard::KeyCode`. The entire game-facing contract is still one trait with one method: `EmberGame::update(&mut self, input: &InputState, dt: f32) -> Frame` (`lib.rs:28-30`).

- `app.rs` (277 lines) — platform layer. `App<G>` implements winit's `ApplicationHandler`; `run<G>(config, game)` builds the event loop and either runs it (native, `ControlFlow::Poll`, `app.rs:250`) or spawns it on rAF (wasm, `ControlFlow::Wait`, `app.rs:254`). Handles eight window events: `CloseRequested`, `Focused(false)`, `KeyboardInput`, `Resized`, `CursorMoved`, `CursorLeft`, `MouseInput`, `RedrawRequested` (`app.rs:114-234`) — the last three arrived with the shooter. It still implements no `device_event`. Frame-stall detection at a 100 ms threshold with 1 s warn rate-limiting (`app.rs:39`, `app.rs:212-223`). `dt` is clamped to 0.1 s before the game sees it (`app.rs:207`). The viewport aspect is pushed into `InputState` once per redraw (`app.rs:197-201`). On wasm the renderer is created asynchronously and picked up on a later redraw (`app.rs:59-60`, `app.rs:151-165`), the sim is gated until it lands (`app.rs:190-195`), and the canvas backing store is synced to CSS layout by polling `client_width`/`devicePixelRatio` every redraw (`app.rs:170-184`).
- `input.rs` (83 lines) — `InputState { pressed: HashSet<KeyCode>, mouse: HashSet<MouseButton>, cursor_ndc: Option<[f32; 2]>, aspect: f32 }` with `down()`, `axis(neg, pos)`, `mouse_down()`, `cursor_ndc()`, `aspect()`, and crate-private `press`/`release`/`mouse_press`/`mouse_release`/`set_cursor_ndc`/`set_aspect`/`clear`. Still snapshot-style and still polled once per frame; the pointer half is an *absolute* position, not a delta. `clear()` on focus loss now drops held mouse buttons as well as keys (`input.rs:79-82`).
- `renderer.rs` (631 lines) — the GPU layer. Public types: `Instance { position, scale, color, yaw }` with `Instance::new` and `with_yaw` (`:17`, `:25-39`), `Camera { eye, target, fov_y_deg }` with `view_proj(aspect)` (`:42`, `:59`), `Frame { camera, instances }` (`:68`), and `Renderer` with `new`, `resize`, `resize_if_changed`, `render(&Frame)` (`:104-527`). Private: `Vertex { pos, normal }` (`:75`), `InstanceRaw` (`:82`), `SceneTargets { color_view, depth_view, width, height }` (`:96`), and the helpers `create_scene_targets` (`:529`), `create_present_bind` (`:569`), `create_instance_buf` (`:591`), `cube_vertices` (`:601`). The `yaw` field is a fourth instance vertex attribute at shader location 5 (`:260`); it is the only new rendering feature the shooter needed.
- `shader.wgsl` — scene pass: instanced boxes, per-instance rotation about Y applied to both position and normal (`shader.wgsl:24-40`), per-face normals, hardcoded directional light with 0.35 ambient (`shader.wgsl:46-51`). No texture bindings, no UVs.
- `present.wgsl` — presenter pass: fullscreen triangle, single `textureSample` (`present.wgsl:14-32`). No uniform buffer.

### `ember-net` — 227 lines + a 155-line example

Shared arena protocol: length-prefixed postcard frames over TCP. Depends only on `serde` and `postcard`. `PROTOCOL_VERSION = 2`, `DEFAULT_PORT = 7777`, `TICK_HZ = 60`, `ARENA_HALF = 20.0`, `MOVE_SPEED = 10.0`, `MAX_FRAME_BYTES = 64 KiB`, `MAX_NAME_LEN = 24`, `CLIENT_TIMEOUT_SECS = 10` (`crates/ember-net/src/lib.rs:14-28`). Types `PlayerId`, `PlayerMeta`, `PlayerState`, `ClientMsg`, `ServerMsg`; functions `write_msg`, `read_msg`, `color_for`, `sanitize_dir`, `sanitize_name`. `examples/netbot.rs` is a headless verification client that exits non-zero unless it sees at least half the expected snapshot count and 2.0 units of movement.

### `ember-server` — 521 lines

Headless dedicated arena server. Depends on `ember-net` and tracing only — no engine, no wgpu, no winit. Public surface is two items: `ServerConfig { max_players }` (default 32) and `run(listener, cfg)` (`crates/ember-server/src/lib.rs:22-30`, `:81`). One sim thread owns all state; an accept thread plus a reader and a writer thread per connection feed it over an mpsc `Event` channel. Connection cap is `max_players * 2 + 16`, enforced before thread spawn (`:323`).

### `pong-core` — 963 lines

Two shared deterministic sims plus the online JSON protocol. Depends only on `serde`. `sim.rs` (the pong sim) is fully engine-independent: it has no `use` statements at all, no `glam`, no math library — positions are bare `[f32; 2]` interpreted as (x, z) by convention, and the input type is two `f32` axes via `Sim::step(p1_axis, p2_axis)` (`crates/pong-core/src/sim.rs:70`). It defines no camera; camera choice lives entirely in the game layer.

`shooter.rs` (466 lines, new) is the arena-shooter sim that now backs online play: up to 8 players, 3 HP, seeded procedural obstacles via `generate_arena(seed)` (`shooter.rs:30`), bullets with a per-player cap, and `Sim::step(&dyn Fn(u8) -> PlayerIn)` (`shooter.rs:142`). It holds the same discipline as `sim.rs` — no engine, no math library, positions as bare `[f32; 2]`, no camera — so the layering argument below is unaffected by it. Its `ARENA_HALF`, `MOVE_SPEED`, and `FIXED_DT` constants are its own, deliberately not shared with `ember-net`'s same-named ones.

`proto.rs` carries `PROTO_VERSION = 3` (`proto.rs:10`), `STATE_EVERY_TICKS = 2` (so 30 Hz state broadcast against a 60 Hz sim), the `C2S`/`S2C` enums, `color_for`, and `sanitize_text`. The wire is now aim-carrying: `C2S::Input { mx, my, ax, az, fire }` (`proto.rs:77`) replaces the single axis, and `S2C` gained `GameJoined`, `PlayerJoined`, `PlayerLeft`, and `Kill` for N-player drop-in play. `sanitize_axis` is gone.

### `pong-server` — 835 lines + a 172-line example

WebSocket + JSON matchmaking and match server. Depends on `pong-core`, `serde_json`, `tungstenite`, tracing. Public surface is unchanged: `ServerConfig { max_conns, max_lobbies }` (128/64, `crates/pong-server/src/lib.rs:36-48`) and `run(listener, cfg)` (`:121`). It now runs an authoritative `pong_core::shooter::Sim` per lobby (`:32`, `:625`) rather than a pong `Sim` per match, with drop-in join and leave rather than a fixed pair. A lobby is dropped only when its last member leaves (`:801-804`). `examples/wsbot.rs` is the headless check.

### `game` — 541 lines

The arena client. Depends on `ember-engine`, `ember-net`, `glam`, `tracing`. `main.rs` implements `EmberGame` over a `Session` enum (online with a `NetClient`, or an offline local-only fallback when the server is unreachable, `game/src/main.rs:19-25`). `net.rs` is the client connection: a reader thread feeding an mpsc channel, a mutex-guarded writer, and a keepalive thread that pings every 2 s so a minimized window does not time out. `world.rs` is the interpolated client-side view of remote players, blending `from`→`to` at `dt * TICK_HZ` per snapshot interval (`world.rs:121-127`).

### `pong` — 1005 lines

The client crate; a native binary (`pong-app`) and a wasm cdylib. Despite the name it now ships two different games: local pong, and the online arena shooter. Depends on `ember-engine`, `pong-core`, `glam`, `serde`, `serde_json`, plus `tungstenite`/`rustls` natively and `web-sys` on wasm. `lib.rs` holds the pong scene builder `build_scene(&SceneParams) -> Frame` (`pong/src/lib.rs:33-122`) and `LocalGame` (fixed 60 Hz accumulator with render interpolation, `:155-206`), both now local-only; the `flip` camera flag survives but no online path reaches it. `lib.rs` also exposes `proto_version()` to wasm (`:248`) so the hub page reads the protocol number out of the build instead of hardcoding it.

`online.rs` (713 lines) holds `ShooterGame` — formerly `OnlineGame` (`online.rs:139`) — and the same platform-split `NetChan` (native tungstenite thread vs. wasm `WebSocket` callbacks). It builds its own scene rather than calling `build_scene`, drives a smoothed follow camera (`online.rs:327-338`), and aims by unprojecting the cursor onto the play plane through `Camera::view_proj(aspect).inverse()` (`online.rs:114-131`). That inverse is the first consumer anywhere in the workspace of the camera's *projection* as a value rather than as a matrix handed to the GPU, and it constrains the milestone's camera rework — see §3.

### Layering

The declared order (`README.md:44`, `ember-engine/src/lib.rs:3-7`) is game → scene/simulation → renderer → platform. **In the crate dependency graph it holds cleanly and acyclically** — verified in every `Cargo.toml`. `ember-net` pulls nothing but serde/postcard; `ember-server` never pulls the engine; `pong-core` never pulls the engine; both servers duplicate their own fixed-timestep loop rather than importing one, which is precisely what keeps them engine-free. No crate-level violation exists.

Three softer observations:

- The "scene/simulation" layer named in the contract **does not exist**. `ember-engine/src/lib.rs:4` says "scene/simulation (soon)". Games talk to the renderer directly by building a `Frame` of `Instance`s.
- The ATW document amends the layering to `game → sim → scene renderer → presenter → platform` (`docs/atw-first-rendering.md:92-95`) and requires that the renderer's output contract be a `SceneFrame` and never the swapchain texture. That amended layering is not represented anywhere in the module structure, and the README was never updated to match the adopted document. See §2.
- `game` and `pong` both take a direct `glam` dependency (`game/Cargo.toml:9`, `pong/Cargo.toml:17`) while the engine also re-exports `glam` (`lib.rs:22`); `game/src/world.rs:9` imports from the direct dependency. Workspace pinning makes this safe today, but a version skew would produce two mutually incompatible `Vec2` types with a confusing error.

## 2. Presenter / renderer state: how far stage A actually is

Roadmap 0.6 is marked done and claims "scene renders offscreen (SceneFrame, color+depth); a presenter pass owns the swapchain" (`README.md:90-92`). The survey finds that the *pixel path* of stage A is real and the *architecture* of stage A is not.

**What genuinely exists.** Offscreen color and depth targets are created at scene resolution (`renderer.rs:529-567`). Depth is `Depth32Float` with `StoreOp::Store`, explicitly not discarded, as ATW consequence 6 requires (`renderer.rs:476-485`). The scene colour format is `Rgba8UnormSrgb` — post-tonemap LDR, as ATW consequence 7 requires (`renderer.rs:91`). The scene pass renders only into those targets (`renderer.rs:461-495`). A fullscreen-triangle identity blit samples the scene colour onto the surface (`present.wgsl:14-32`, `renderer.rs:504-521`). Scene and present work go into two separate command encoders, which is the right *shape* for the sliced-submission rule (`renderer.rs:455`, `:498`). The sRGB-on-write problem for WebGPU canvases that expose only non-sRGB formats is handled by rendering the present pass into an sRGB reinterpreting view (`renderer.rs:178-191`, `:421-426`).

**What the ATW document requires and the code does not have.**

1. **There is no `SceneFrame` type.** The identifier appears four times in `renderer.rs` comments and once in `present.wgsl`, and nowhere in executable code. The actual struct is `SceneTargets` (`renderer.rs:96-101`) and it carries `color_view`, `depth_view`, `width`, `height` — and nothing else. The document defines a `SceneFrame` as colour plus depth plus *the exact pose and projection it was rendered with* plus a sim timestamp (`atw-first-rendering.md:52-54`). The pose and projection are exactly the fields the warp needs to compute `R_now · R_scene⁻¹`; without them there is no stage B, and their absence is the single largest gap in the milestone.
2. **The renderer owns and presents the swapchain.** `Renderer` holds the `Surface` (`renderer.rs:105`), and `Renderer::render` acquires the surface texture (`renderer.rs:410`) and calls `surface_tex.present()` (`renderer.rs:525`). This directly contradicts both the README's claim that a presenter pass owns the swapchain and the document's rule that the scene renderer never touches it. There is no `Presenter` type and no `presenter.rs`; the presenter is four fields and one render pass inside the renderer (`renderer.rs:120-123`, `:504-521`).
3. **Scene and present are locked to one clock.** `Renderer::render(&Frame)` does both, once, per redraw (`renderer.rs:409`, called from `app.rs:227`). The document's three-clock model — sim at fixed 60 Hz, scene variable and budgeted, present at display rate (`atw-first-rendering.md:36-41`) — cannot be expressed. Scene rate equals present rate by construction, so frame-rate amplification, the lever the document calls the main one for "runs on whatever laptop opens the tab", is unreachable.
4. **There is no ring.** The document specifies a ring of 2–3 frames so the renderer writes one while the presenter reads the newest complete one (`atw-first-rendering.md:54-55`). There is exactly one `SceneTargets`, recreated on resize (`renderer.rs:400`).
5. **`scene_scale` is a dead knob.** It is initialised to 1.0 (`renderer.rs:194`), stored (`renderer.rs:111`), read on resize (`renderer.rs:400`), and never written — there is no setter and no caller. Dynamic resolution is claimed by the module doc comment but cannot be exercised.
6. **The present bind group has no uniform.** It is texture plus sampler only (`renderer.rs:299-319`). Stage B needs the rotation delta and both frusta's tangents, so the pipeline layout must change; this is a structural edit, not a shader tweak.
7. **No guard band, and no support for one.** `Camera::view_proj` builds a symmetric `perspective_rh` from `fov_y_deg` with hardcoded near 0.1 and far 500.0 (`renderer.rs:59-63`). There is no widened-FOV concept and no asymmetric-frustum handling, which ATW consequence 5 says must exist from day one.
8. **Sliced submission is shaped but not behaving.** The two command buffers are handed to a single `queue.submit([scene, present])` call (`renderer.rs:523-524`). The point of the rule is that the warp slots in at a different cadence than the scene work; one submit of both, once per frame, obtains none of that.
9. **UI is not composited in the presenter, because there is no presenter-side UI slot at all.** Status text and the shooter's scoreboard are DOM, written from Rust into the page (`pong/src/online.rs:50-62`); local pong's score display is 3D cube pips placed in the world (`pong/src/lib.rs:105-119`), and the shooter's gun is likewise world-anchored instanced boxes (`pong/src/online.rs:73-111`). World-anchored geometry will warp correctly along with the world, so today there is still no violation of ATW consequence 4 — but there is also nowhere to put a crosshair when the fly camera lands, and that is the retrofit the document calls the most common pain. The shooter has sharpened this: it aims at the OS cursor and therefore uses the desktop pointer as its de facto reticle, which is neither in the scene nor in a presenter overlay. A pointer-locked fly camera hides that cursor, so the first game that captures the mouse is also the first game that *needs* the overlay slot.

**Summary judgement:** stage A is complete as a *rendering topology* and roughly a third complete as an *architecture*. Everything the document says must exist "in the API from the start" — the `SceneFrame` value type, the presenter as an owning stage, the two camera reads, the input latch, the guard band — is absent. That is why the milestone plan opens with a bite-zero refactor.

**What each client uses.** All three games use the same surface: implement `EmberGame::update`, return a `Frame` with one `Camera` and a `Vec<Instance>` of coloured boxes, now optionally yawed. `game` builds an arena floor slab and four corner markers, then one cube per player, always with `Camera::default()` (`game/src/main.rs:47-79`, `:161`, `:171`). Local `pong` builds a court, two paddles, a ball, and score pips, with one of two hardcoded cameras selected by the `flip` flag (`pong/src/lib.rs:34-46`). The arena shooter builds obstacles, players, guns, and bullets, and drives a follow camera that smooths toward the local player each frame (`pong/src/online.rs:327-338`) — the first camera in the tree that is neither fixed nor `default()`.

Two of these three now do more than hand the camera to the renderer. The shooter reads `Camera::view_proj(aspect).inverse()` to turn the cursor into a world position (`pong/src/online.rs:114-131`), which means the `Camera` type has acquired a second consumer with its own correctness requirement: any change to how a camera stores its orientation must keep that inverse well-defined. Still, no client has any notion of a *view* camera distinct from the scene camera; the split described in §3 remains absent.

## 3. Input path

Capture is event-driven into snapshot state. `WindowEvent::KeyboardInput` maps a `PhysicalKey::Code` to `InputState::press`/`release` (`app.rs:117-128`); `WindowEvent::Focused(false)` clears both the key set and the button set so nothing sticks across alt-tab (`app.rs:116`, `input.rs:79-82`); Escape exits on native (`app.rs:123-126`).

Consumption is a single poll per frame: `self.game.update(&self.input, dt)` (`app.rs:225`). Games read it through `down()`, `axis()`, `mouse_down()`, `cursor_ndc()`, and `aspect()`. `game` composes a WASD-plus-arrows direction vector and sends it as a held intent to the server on change or every 300 ms (`game/src/main.rs:88-97`, `:136-141`). Local `pong` reads two axes for two players (`pong/src/lib.rs:157-158`). The arena shooter reads a WASD vector, the left mouse button (or Space) as fire, and the cursor as aim, and sends all of it at a fixed 20 Hz (`pong/src/online.rs:351-365`).

**Absolute mouse input exists; relative mouse input does not.** This is the distinction that matters for the fly camera, and the two halves are on opposite sides of it.

What exists: `WindowEvent::CursorMoved` converts the cursor to normalized device coordinates and stores it (`app.rs:134-144`), `WindowEvent::CursorLeft` clears it (`app.rs:145`), and `WindowEvent::MouseInput` maintains a held-button set (`app.rs:146-149`). The viewport aspect is refreshed each redraw (`app.rs:197-201`) precisely so a game can unproject that NDC position itself, which the shooter does (`pong/src/online.rs:114-131`). `MouseButton` is re-exported for games (`lib.rs:23`).

What still does not exist: a workspace-wide grep for `MouseMotion`, `DeviceEvent`, `MouseWheel`, `CursorGrab`, `set_cursor_grab`, and `set_cursor_visible` returns nothing in any `.rs` file. **`ApplicationHandler::device_event` is not implemented**, so there is no source of unaccelerated relative deltas — and a cursor position clamped to the window cannot substitute for one, because it saturates at the window edge exactly when a fly camera needs to keep turning. **Pointer lock is not used anywhere** — no `requestPointerLock` in any page, no `set_cursor_grab` in Rust. The canvas-focusing pointer listener that used to live in `web/index.html` has moved: `web/index.html` is now the hub and hosts no canvas, and each game page carries its own copy (`web/games/arena/v3/index.html:121-123`, `web/games/pong/v2/index.html:66-68`). Since those per-version directories are frozen on gh-pages once published, any page-side listener a future feature needs — a pointer-lock gesture among them — can only be added to *new* versions.

So the shooter moved the input path forward without moving it toward the fly camera: it needed to know *where* the pointer is, which is the absolute question, while warp needs to know *how far it moved*, which is the relative one. Both readings must survive — the shooter's aim is a genuine consumer of `cursor_ndc`, and removing or repurposing it to carry deltas would break a shipped game.

**There is no late input latch.** Input is still read once, at the top of the frame, before scene submission — the new pointer state included. There is no accumulator, no per-consumer mark, and no second read point: a grep for `latch`, `delta_since`, `mark_sim`, and `mark_view` finds nothing. The document requires raw input to accumulate in a latch read at warp-encode time (`atw-first-rendering.md:96-98`). Nothing in the current loop has a second read point to attach it to.

**The sim/net camera vs. view camera split does not exist.** `EmberGame::update` returns a `Frame` carrying *the* camera (`lib.rs:29`, `renderer.rs:68-71`), and there is exactly one camera read point in the whole API. The document says the split "must exist in the API from the start" (`atw-first-rendering.md:99-101`); it does not.

What has changed is that the single camera is no longer used only for the scene matrices (`renderer.rs:429`). The shooter also inverts it to unproject the cursor (`pong/src/online.rs:114-131`), so the one camera value now serves both rendering and gameplay. That makes the missing split slightly worse than it was: when a view camera is introduced, it will have to be unambiguous which of the two a game means when it unprojects a pointer — and for a cursor drawn by the OS at its true screen position, the answer must be the *view* camera, not the scene camera the frame was rendered with.

One point still in the split's favour: the sim side remains clean. `pong-core::sim` takes two floats, `pong_core::shooter::Sim` takes held intents, and neither knows anything about cameras. The wire carries an aim as a world-space direction (`proto.rs:77`) rather than a view pose, and `ember-net` still transmits only a movement direction. Nothing on the deterministic or wire side would need to change to introduce a view camera — the split is purely an engine-API question.

## 4. Test inventory

Twenty-three `#[test]` functions exist, all in the protocol, sim, and server crates. There is no async test framework — tokio is not in the workspace at all; every server is sync threads.

| Crate | Tests | Kind |
|---|---|---|
| `ember-engine` | **0** | — |
| `game` | **0** | — |
| `pong` | **0** | — |
| `ember-net` | 3 unit | `roundtrip_messages`, `sanitize_rejects_bad_input`, `oversized_frame_is_rejected` |
| `ember-server` | 3 integration | `two_players_see_each_other_move`, `protocol_mismatch_is_rejected`, `input_before_hello_disconnects` |
| `pong-core` | 14 unit | 2 protocol (`json_roundtrip`, `sanitizers`) + 6 pong sim, including `determinism_same_inputs_same_result` over 3600 steps + 6 shooter sim (`arena_is_deterministic_and_bounded`, `arena_covers_all_quadrants`, `point_blank_shots_connect`, `walls_block_movement`, `three_hits_kill_score_and_respawn`, `bullet_cap_holds`) |
| `pong-server` | 3 integration | `drop_in_arena_flow_with_password`, `old_proto_may_list_but_not_join`, `message_before_hello_disconnects` |

The seven tests the delta added are all in the same tradition: deterministic sim arithmetic and loopback protocol flows. `pong-server`'s `full_match_flow_with_password` became `drop_in_arena_flow_with_password`, and `old_proto_may_list_but_not_join` is new — it pins the hub's rule that any protocol version may list lobbies while only the live one may create or join.

The integration tests bind `127.0.0.1:0` and spawn the real server on a thread, so they need loopback but no fixed port and no external service — they are safe for an unattended lane.

**Everything visual, GPU-side, or platform-side is verified only by running it and looking.** That includes: surface format and sRGB selection, the offscreen render and identity blit, the depth buffer, the wasm canvas/DPR sync, the WebGL2 fallback path, frame-stall detection, camera framing, and both clients' scene construction. There is not one assertion anywhere about the renderer, the presenter, input handling, or camera math. Two headless bots (`ember-net/examples/netbot.rs`, `pong-server/examples/wsbot.rs`) provide exit-code verification of the network paths, and the deploy scripts are unverified except by their own success.

This matters for the milestone: the warp is a piece of arithmetic whose correctness is checkable — the identity case must produce identity UVs, and a guard band with an identity rotation must crop back to exactly the original image. Those are unit tests, not screenshots, and the engine currently has no test module to put them in.

## 5. Risks and tech debt worth backlog entries

Renderer and ATW architecture:

- `SceneFrame` exists only in prose: the type the adopted document makes the renderer's output contract is not in the code, and `SceneTargets` (`renderer.rs:96`) carries no pose, projection, or timestamp.
- `Renderer` owns the `Surface` and calls `present()` (`renderer.rs:105`, `:525`), contradicting both `README.md:90-92` and `atw-first-rendering.md:92-95`.
- Scene and present share one clock, so frame-rate amplification and dynamic resolution — the two payoffs the ATW decision was made for — are unreachable without a decoupled presenter.
- `scene_scale` is a dead knob: set to 1.0 at construction (`renderer.rs:194`) with no setter and no caller.
- No `SceneFrame` ring; one target set, recreated on every resize (`renderer.rs:400`), so a stage-B ring will also need a bind group per slot.
- `Camera::view_proj` hardcodes near 0.1 and far 500.0 with a symmetric frustum (`renderer.rs:61`) — no guard band, no asymmetric-frustum support.
- The present bind group is texture plus sampler with no uniform buffer (`renderer.rs:299-319`); stage B forces a pipeline-layout change.
- Scene pipeline sets `cull_mode: None` (`renderer.rs:276`), drawing every backface — wasted fill that grows with a widened guard-band FOV.
- A fresh `Vec<InstanceRaw>` is heap-allocated every frame (`renderer.rs:433-442`), and `instance_buf` grows but never shrinks.
- `Renderer::new` panics via `.expect()` on adapter and device acquisition (`renderer.rs:145`, `:167`), so a WebGPU init failure is a console panic rather than a message the page can show.
- `Instance::yaw` rotates position and normal in the vertex shader (`shader.wgsl:24-40`) on the assumption that instance scale is uniform per axis; a non-uniform scale combined with a yaw will produce wrong normals, and nothing enforces the assumption.

Engine, input, and platform:

- No *relative* mouse input path exists: `app.rs` handles absolute `CursorMoved` and `MouseInput` but implements no `device_event`, so `DeviceEvent::MouseMotion`, cursor grab, and pointer lock are all still to be built. The fly camera starts from the absolute path, which cannot serve it.
- One camera read point only (`lib.rs:29`), with nowhere to express a sim camera distinct from a view camera — and that single camera now has a gameplay consumer as well as a rendering one (`pong/src/online.rs:114-131`).
- No presenter-side UI slot, so the first crosshair or HUD has nowhere to go except into the scene pass, which is the retrofit the document warns about.
- Zero tests in `ember-engine`, `game`, and `pong` — no `#[cfg(test)]` module anywhere in the client-side half of the workspace.
- Two different stall policies: `dt` is clamped to 0.1 s in the engine (`app.rs:185`) and pong's accumulator clamps again at 0.25 s (`pong/src/lib.rs:161`).
- The wasm resize path performs a layout read (`client_width`/`devicePixelRatio`) every redraw (`app.rs:158-165`) instead of using a `ResizeObserver`, and reacts one frame late.
- `game` and `pong` depend on `glam` directly while the engine re-exports it; a version skew would yield two incompatible vector types.
- `game` connects to the server before the window opens, so an unreachable server stalls launch about four seconds — acknowledged in `README.md:80-82` but still a first-run experience issue.

Protocol, servers, and deployment (from the net/server sweep):

- `Ping` is matched before the "must have said Hello" arm in `ember-server` (`ember-server/src/lib.rs:448`), letting an unauthenticated peer hold a connection slot indefinitely by refreshing `last_seen`. **`pong-server` has since fixed this**: the catch-all pre-Hello arm now precedes the `Ping` arm and drops the connection, with a comment naming the parked-slot attack (`pong-server/src/lib.rs:533-538`). The fix has not been ported to `ember-server`, so the two servers now differ on a security-relevant rule — worth a backlog entry in its own right.
- ~~`pong-server` drops a lobby on a failed `State` send without sending `OpponentLeft`~~ — **fixed upstream.** Leaving a lobby now removes just that player, notifies the remaining members with `S2C::PlayerLeft`, and drops the lobby only when its last member leaves (`pong-server/src/lib.rs:793-804`). `OpponentLeft` no longer exists; the N-player `PlayerLeft` replaced it.
- Two divergent sanitizers for the same job: `ember_net::sanitize_name` does not trim while `pong_core::sanitize_text` does, so arena names can carry surrounding whitespace.
- `pong-server` defaults to port 7778 (`pong-server/src/main.rs:12`) but the deploy script must override to 7780 because SELinux denies it — a default that is known-broken on the only deployment target.
- Every deploy mints a new `trycloudflare.com` domain and requires a `gh-pages` commit to stay reachable, dropping any in-flight players.
- `deploy-pages.sh` and `deploy-pong-online.sh` each rewrite `server.json` independently — one merges, one overwrites wholesale — so running them out of order clobbers either the server URL or the cache-busting stamp.
- Deploy is `tar` plus `scp` plus `pkill` plus `nohup` with no systemd unit and no health gate before the old process is killed, so a failed remote build leaves nothing running.
- `ember-server` and `pong-server` duplicate the entire fixed-timestep, stall-resync, and event-drain loop nearly verbatim — two copies of the trickiest scheduling code in the tree.
- Lobby names are a flat global namespace and passwords are compared with a plain string inequality (`pong-server/src/lib.rs:690`).
- `pong-server` links a TLS stack it never uses, because `tungstenite` carries `rustls-tls-webpki-roots` in `[dependencies]` rather than `[dev-dependencies]`.
- `Sim` exposes every field publicly with no `Default` impl (`pong-core/src/sim.rs:34-52`), letting any consumer mutate authoritative state directly.
- The web page cannot recover from a stale tunnel: a failed lobby refresh reports "server not reachable" with no retry of `server.json`, and the online path uses blocking `alert`/`prompt` for errors and passwords.
- The games hub freezes old builds under `web/games/<game>/<version>/` and keeps them playable, but a frozen build speaks a frozen protocol version against the single live server; `old_proto_may_list_but_not_join` pins the intended behaviour (an old build can browse but not play), which means every archived version becomes a lobby browser that cannot join anything the moment the protocol moves.

## 6. Where the README overstates or understates the code

- **Overstates.** "0.6 ATW-first presenter (stage A): scene renders offscreen (SceneFrame, color+depth); a presenter pass owns the swapchain" (`README.md:90-92`). There is no `SceneFrame` type, and the renderer — not a presenter — owns and presents the swapchain.
- **Overstates.** "Strict one-way layering: game → scene/simulation → renderer → platform" (`README.md:44`). The crate graph is clean, but there is no scene/simulation layer in the engine (`lib.rs:4` says "soon"), and the presenter stage the adopted document adds to this chain is not in the module structure. The README also predates the document's amendment and was never reconciled with it.
- **Understates.** Roadmap item 1 opens with "first triangle → textured cube", both unchecked (`README.md:95`), but instanced, depth-tested, Lambert-lit cubes have shipped since 0.5 (`renderer.rs:601-631`, `shader.wgsl`). The only genuinely missing piece of that phrase is *texturing*: `Vertex` is position plus normal with no UV (`renderer.rs:75-78`), and the scene pass has no texture binding. The real content of item 1 is the fly camera and stage B.
- **Stale, newly.** The "## Pong" section (`README.md:142-150`) still describes online play as paddle pong: "online matchmaking" into a two-player match, the authoritative sim named as `crates/pong-core/src/sim.rs`, "either key set steers your paddle online", and a flipped camera for player 2. Online play is now the arena shooter — `pong-core/src/shooter.rs`, up to 8 drop-in players, WASD plus cursor aim plus a fire button — and none of that paragraph is true of it any more. The three upstream commits added a "## Games hub" section (`README.md:129-140`, accurate) without reconciling the section below it.
- **Accurate.** The diagnostics table (`README.md:70-77`) matches the code on every row, and the known-limitations paragraph (`README.md:79-82`) correctly describes the snapshot interpolation and the pre-window connect. The layering line (`README.md:44`), the 0.6 claim (`README.md:90-92`), and item 1 (`README.md:95`) are all still at the line numbers cited above — the Games hub section was inserted below them.
