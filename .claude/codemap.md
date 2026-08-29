# ember code map

Generated 2026-08-29 by local GLM-4.7-Flash workers. Per-file summaries; verify before relying on details.

## crates\ember-engine\src\app.rs
**Purpose:** Core entry point and main loop orchestrating window creation, input handling, rendering, and game updates across desktop and WebAssembly.

**Key items:**
*   `EngineConfig`: Struct for window title, mouse capture, and initial meshes.
*   `run`: Entry point function creating the event loop and handler.
*   `App`: Private `ApplicationHandler` struct containing game state, renderer, and input.

**Depends on:** `crate::renderer::Renderer`, `crate::input::InputState`, `crate::EmberGame`, `winit`.

**Gotchas:**
*   Wasm resize synchronization: `request_inner_size` is intentionally omitted; layout is handled via CSS, and backing store sizing syncs manually in `RedrawRequested`.
*   Renderer initialization safety: On Wasm, the renderer is async. `update` is guarded against running before the renderer is ready.
*   Frame stall threshold: `FRAME_STALL_THRESHOLD_MS` (100ms) used for detecting GPU starvation.
*   Platform-specific control flow: Desktop uses `Poll`, Wasm uses `Wait`.


## crates\ember-engine\src\assets.rs
**Purpose:** Loads binary GLB files, flattens triangle meshes, and transforms vertices for the unindexed renderer.

**Key items:**
*   `GlbPart`: Named mesh chunk with material color.
*   `load_glb()`: Parses glTF, extracts scenes/nodes, returns part list.
*   `collect()`: Recursive traversal applying world transforms and de-indexing.

**Depends on:**
*   `glam` (Mat4, Vec3), `renderer` (MeshData, MeshVertex), `gltf`

**Gotchas:**
*   Y-up to Y-up conversion; assumes +X forward, +Z up in Blender.
*   `MeshData` is unindexed; normals are calculated from world transform.
*   `MeshVertex` pos is world-space; normal is normalized world-space.


## crates\ember-engine\src\input.rs
**Purpose:** Stores current keyboard/mouse state and cursor position for snapshot-style polling.

**Key items:** InputState, down(), axis(), mouse_down(), cursor_ndc(), mouse_delta(), set_aspect(), clear()

**Depends on:** winit

**Gotchas:** Aspect ratio defaults to 16/9 in `Default`; `mouse_delta` is raw pixels, not normalized; `clear()` prevents key sticking on focus loss.


## crates\ember-engine\src\lib.rs
**Purpose:** Core entry point for the 3D engine, defining the game trait and re-exporting core types (winit, glam) to isolate the platform layer.

**Key items:** `EmberGame` trait, `EngineConfig`, `Frame`, `Instance`, `MeshData`, `MeshVertex`, `Camera`

**Depends on:** `assets`, `renderer`, `app`, `input`, `winit`, `glam`

**Gotchas:** `EmberGame::update` signature is strictly defined; `Frame` ownership transfer pattern must be respected.


## crates\ember-engine\src\present.wgsl
**Purpose:** Fullscreen identity warp shader for the presenter, sampling the latest SceneFrame.

**Key items:**
*   `vs_main`: Generates a single fullscreen triangle and maps UVs to [0,1] range.
*   `fs_main`: Samples the scene texture using the bound sampler.
*   `scene_tex`: The output of the renderer (SceneFrame).
*   `scene_samp`: The sampler state.

**Depends on:** `ember-core` (WGSL definitions), `ember-renderer` (SceneFrame generation).

**Gotchas:** None noted.


## crates\ember-engine\src\renderer.rs
**Purpose:** Implements the ATW-first rendering architecture, separating the scene pass (writes to an offscreen LDR target) from the presenter pass (samples and blits to the swapchain).

**Key items:**
- `Renderer`: Core GPU abstraction owning wgpu device, pipelines, and scene targets.
- `Frame`: Container for `Camera` and `Instance` lists to be rendered.
- `SceneTargets`: Internal struct holding the offscreen color/depth textures.
- `create_instance_buf`: Allocates vertex buffer for instanced draws.
- `cube_vertices`: Generates the default unit cube geometry (Mesh ID 0).

**Depends on:** wgpu, glam, bytemuck, tracing

**Gotchas:**
- Mesh ID 0 is reserved for the built-in unit cube; registered meshes use IDs 1..N.
- The presenter always renders linear light, so the swapchain format must be sRGB or the image displays crushed gamma.
- `scene_scale` controls dynamic resolution; presenter resamples regardless.
- Instance buffer is reallocated dynamically if `frame.instances.len()` exceeds `instance_cap`.
- `resize` recreates `scene` targets and `present_bind` (not just surface).


## crates\ember-engine\src\shader.wgsl
**Purpose:** WGSL vertex/fragment shader for instanced entities with uniform-per-axis scaling and Y-axis rotation.

**Key items:**
*   `CameraUniform`: View-projection matrix binding.
*   `VsIn`: Input with per-instance scale (vec3), position (vec3), color (vec3), and Y-rotation (f32).
*   `vs_main`: Handles 3D rotation and lighting math; normal rotation matches vertex rotation.
*   `fs_main`: Basic directional lighting with fixed ambient term.

**Depends on:** ember-wgpu

**Gotchas:** 
*   Normal and position rotation must use identical `c/s` sine/cosine calculations.
*   Y-axis rotation uses `vec3(c, 1.0, -s)` layout for X/Z components.


## crates\ember-net\examples\netbot.rs
**Purpose:** Headless verification client testing multiplayer loop connectivity, movement, and snapshot delivery. Connects, walks in a circle, pings, and validates RTTs and snapshot volume before exit code 0.

**Key items:**
*   `main`: Entry point connecting via `TcpStream`, spawning reader thread, and writing movement/Bye messages.
*   `Stats`: Struct capturing snapshot count, first/last position, max players, and RTT vector.
*   `ClientMsg::Input`: Sends move direction vectors.
*   `ClientMsg::Ping`: Sends nonce for RTT measurement.
*   `ClientMsg::Bye`: Gracefully closes connection to differentiate early death from expected server close.

**Depends on:** `std`, `ember_net` (ClientMsg, ServerMsg, read_msg, write_msg, PlayerId, PROTOCOL_VERSION, DEFAULT_PORT, TICK_HZ)

**Gotchas:** Exits with code 1 on disconnect; `Bye` sent *after* checking liveness to distinguish expected server shutdown from network failure; `min_snapshots` heuristic accepts half the expected rate (TICK_HZ/2) to tolerate slow links.


## crates\ember-net\src\lib.rs
**Purpose:** Defines the shared multiplayer protocol for the game, including message serialization, wire framing (length-prefixed postcard), and constants required for synchronization.

**Key items:**
*   `ClientMsg`, `ServerMsg` - Enumerations for all network messages.
*   `PlayerId`, `PlayerMeta`, `PlayerState` - Core data structures for entities.
*   `write_msg`, `read_msg` - I/O wrappers handling the 4-byte LE length prefix and validation.
*   `sanitize_dir`, `sanitize_name` - Security functions preventing malicious input.

**Depends on:** `serde` (de/Serialize), `postcard`, `std::io`

**Gotchas:**
*   `MAX_FRAME_BYTES` (64KB) is the strict hard limit; anything larger is rejected.
*   `Input` acts as the keepalive; `CLIENT_TIMEOUT_SECS` is the silence threshold.
*   Movement magnitude is capped to 1.0 to prevent desyncs.
*   `PROTOCOL_VERSION` must match between client and server.


## crates\ember-server\src\lib.rs
**Purpose:** Headless dedicated server architecture with a single 60Hz simulation thread and separate network IO threads.

**Key items:** `ServerConfig`, `run`, `sim_loop`, `spawn_reader`, `spawn_writer`, `handle_event`, `OUTBOUND_QUEUE`, `LAG_THRESHOLD`

**Depends on:** `ember_net` (constants: `TICK_HZ`, `PROTOCOL_VERSION`, `CLIENT_TIMEOUT_SECS`, `ARENA_HALF`, `MOVE_SPEED`)

**Gotchas:** 30s write timeout prevents writer thread blocking; outbound queue size limits replay depth; sim resyncs if 10 ticks behind; `spawn_writer` clones the socket, so `run` must pass a cloned `TcpStream` to it.


## crates\ember-server\src\main.rs
**Purpose:** CLI entry point for the network server, parses arguments, configures tracing, and binds TCP sockets.

**Key items:** 
- `main`: Argument parsing, env var logging config.
- `ServerConfig`: Max player limit.
- `DEFAULT_PORT`: Port constant from `ember_net`.

**Depends on:** `tracing_subscriber`, `std`, `ember_net`, `ember_server`.

**Gotchas:** 
- `bind` defaults to `127.0.0.1` (localhost) if not overridden via `--bind`.
- Uses `panic!` on binding failures; does not gracefully handle errors for `--max-players` or `--bind` arguments.


## crates\ember-server\tests\e2e.rs
**Purpose:** Integration test verifying real TCP networking, thread lifecycle, and protocol correctness.

**Key items:**
*   `start_server()`: Spawns server thread on random port
*   `connect()`: Handshake logic with nodelay and timeouts
*   `two_players_see_each_other_move()**: Tests snapshot propagation and roster logic
*   `protocol_mismatch_is_rejected()`: Validates `PROTOCOL_VERSION` check
*   `input_before_hello_disconnects()`: Enforces hello-before-input protocol

**Depends on:** `ember_net`, `ember_server`, `std`

**Gotchas:** None noted


## crates\game\src\main.rs
**Purpose:** Entry point for the client application, manages session state (Online/Offline), handles network input/output, and renders the arena.

**Key items:**
*   `Game` struct: Main state container; tracks session, world, and network health.
*   `Session` enum: Discriminates between `Online(NetClient)` and `Offline { pos }`.
*   `SNAPSHOT_STALE_AFTER`: 300ms threshold for detecting snapshot lag spikes.
*   `HIGH_RTT_MS`: 250ms threshold for flagging network latency.
*   `push_player`: Renders a player cube with dynamic scaling based on `is_me`.

**Depends on:** `ember_engine`, `ember_net`, `world`, `net`

**Gotchas:**
*   `MOVE_SPEED` and `ARENA_HALF` (constants from `ember_net`) are used for offline physics clamping.
*   Snapshot staleness logic resets `stale_since` only when age drops back below threshold.
*   `last_rtt_ms` is derived from `ClientMsg::Pong` nonce subtraction.


## crates\game\src\net.rs
**Purpose:** Manages the client-server TCP connection via two threads: a reader thread feeding messages into a channel and a keepalive thread preventing timeouts.

**Key items:** NetClient, connect, send, elapsed_ms, is_dead, Welcome

**Depends on:** TcpStream, mpsc::channel, ember_net

**Gotchas:** Keepalive uses `started.elapsed()` as a nonce for RTT calculation; writes require `Mutex` because the game loop and keepalive thread both produce them; `SERVER_SILENCE_TIMEOUT` (15s) is the death threshold.


## crates\game\src\world.rs
**Purpose:** Client-side view of shared world state, handling server snapshot interpolation for smooth 60Hz rendering.

**Key items:**
*   `World`: Main struct holding arena, player map, and interpolation state.
*   `handle()`: Processes `ServerMsg` (PlayerJoined, Snapshot, etc.).
*   `advance()`: Moves interpolation `t` forward based on `dt` and `TICK_HZ`.
*   `render_players()`: Returns iterator of (pos, color, is_me).

**Depends on:** ember_net, glam

**Gotchas:** `Snapshot` tick numbers must be strictly increasing to prevent reordering bugs; `Entry.t` clamped to [0.0, 1.0].


## crates\pong-core\src\lib.rs
**Purpose:** Provides the pure deterministic simulation and online wire protocol for Pong; core logic and networking shared by wgpu client and headless matchmaking server.

**Key items:**
*   `proto` - Wire protocol definitions (bytes/serde)
*   `shooter` - Ball physics and state machine
*   `sim` - Deterministic simulation runner

**Depends on:** serde_bytes, serde, rand

**Gotchas:** `sim` assumes deterministic RNG seeding for replayability; `shooter` requires `f32` for physics calculations.


## crates\pong-core\src\proto.rs
**Purpose:** Defines JSON-over-WebSocket online protocol v7 for arena shooter matchmaking and state synchronization, using serde for serialization.

**Key items:**
*   `C2S`, `S2C` enums (tagged "t" and snake_case)
*   `LobbyInfo`, `PlayerMeta`, `PState`, `BState` structs
*   `STATE_EVERY_TICKS` (2), `CLIENT_PING_SECS` (5), `PROTO_VERSION` (7)
*   `sanitize_text` utility

**Depends on:** serde

**Gotchas:** `PState.ack` is client-generated and echoed back; `BState.pads` is index-aligned with client-local seed calculations. `sanitize_text` trims whitespace *after* control stripping.


## crates\pong-core\src\shooter.rs
**Purpose:** Deterministic arena shooter simulation running at 60Hz, used by server and clients for authoritative physics and lag-compensated hit testing.

**Key items:**
*   `Sim`, `Sim::step`: Server state machine; axis-separated movement, weapon cooldowns, lag compensation.
*   `PlayerIn`, `PlayerSt`, `Bullet`: State definitions; `PlayerSt` holds authoritative `death_count` and `respawn_in`.
*   `move_circle`, `generate_arena`, `generate_pads`: Shared deterministic functions for movement and seeded geometry.
*   `MAX_REWIND_TICKS`, `MAX_BULLETS_PER_PLAYER`: Lag compensation limits; prevents state bloat.

**Depends on:** `std`, `std::collections::VecDeque`

**Gotchas:** `MAX_REWIND_TICKS` (18 ticks = 300ms) limits lag compensation; `death_count` is reused for spawn points; rewinding purges removed player IDs; deterministic RNG requires matching seeds exactly.


## crates\pong-core\src\sim.rs
**Purpose:** Pure 60 Hz simulation state for Pong, testable headless without platform I/O.

**Key items:** `Sim` struct, `Phase` enum, `step()`, `try_paddle_hit()`, `point_scored()`, `FIXED_DT`, `SPEEDUP`, `WIN_SCORE`, `SERVE_PAUSE`

**Depends on:** none

**Gotchas:** Determinism relies on `serves` counter for serve angle; `try_paddle_hit` uses signed geometry (`signum`) for paddle Z limits; `p1_axis`/`p2_axis` clamp to -1..1 before applying speed.


## crates\pong-server\examples\wsbot.rs
**Purpose:** Headless arena bot connecting via WebSocket to create or join a lobby, then simulating gameplay and reporting state updates.

**Key items:**
*   `PROTO_VERSION` - Protocol version constant for `Hello` message.
*   `C2S::Input` - Sends cyclic movement/firing commands based on time.
*   `C2S::Ping` - Heartbeat every 4 seconds.
*   `rustls::crypto::ring::default_provider()` - Required for wss:// TLS support.

**Depends on:** pong_core::proto, tungstenite, serde_json.

**Gotchas:** Must explicitly install rustls ring provider for WSS.


## crates\pong-server\src\lib.rs
**Purpose:** Multiplayer matchmaking and arena shooter game state server; hub thread owns state, connection threads handle WebSocket transport.

**Key items:**
*   `run`: Entry point, per-IP caps, connection limits
*   `Conn`: Per-connection state, RTT tracking, message flood limits
*   `Lobby`: Running Sim, `alloc_pid`, password protection, input buffering
*   `handle_event`: Protocol version checks, lobby creation/join, lag compensation (view_tick clamp)

**Depends on:** pong_core, tungstenite, serde_json

**Gotchas:** `PROTO_VERSION` must match client; `view_tick` clamped by measured RTT (`allowed_delay = rtt_ticks / 2 + 6`) to prevent free rewinds; `alloc_pid` linear search for smallest unused ID; `Sim::tick + 1` offset for lag compensation math; `lobbyless_since` must not be reset on LeaveLobby spam.


## crates\pong-server\src\main.rs
**Purpose:** Entry point for the Pong server, handling CLI args for binding and initializing tracing.

**Key items:** `main`, `run`, `ServerConfig`, `TcpListener`

**Depends on:** `pong_server`, `tracing_subscriber`

**Gotchas:** Default binding address is `127.0.0.1:7778`; argument parsing is strict (`--bind` consumes the next arg).


## crates\pong-server\tests\ws_e2e.rs
**Purpose:** Verifies real WebSocket protocol flow: lobby creation, password validation, correct seed sharing between players, input firing, and disconnect handling.

**Key items:**
*   `C2S::CreateLobby` (passworded), `JoinLobby`
*   `S2C::LobbyList` (player count, `has_password` flag)
*   `S2C::GameJoined` (seed synchronization check)
*   `S2C::State` (acks and bullet presence)
*   `recv_until` (custom predicate timeout helper)

**Depends on:** `pong_core::proto`, `tungstenite`

**Gotchas:** `PROTO_VERSION` mismatch rejects protocol-0 clients from joining games but allows listing; `s3cret` password is the only valid passcode for the specific lobby.


## crates\pong\src\lib.rs
**Purpose:** Implements a 3D Pong game for the ember engine, supporting local (keyboard) and online (WebSocket) modes with authoritative simulation.

**Key items:**
- `LocalGame`: Manages fixed-timestep physics (60Hz), interpolation, and input handling.
- `build_scene`: Generates the 3D court, paddles, ball, and score pips.
- `ShooterGame`: Manages WebSocket connection and networked state for online play.
- `wasm_api`: Entry points for WebAssembly, including protocol version export.

**Depends on:** pong_core, ember_engine, web-sys, tracing.

**Gotchas:**
- `FIXED_DT` (0.016s) enforces strict physics timing; visual interpolation uses `alpha` clamped to [0,1].
- `PROTO_VERSION` must match the browser lobby JS to ensure compatibility.
- Ball teleport logic resets `prev.ball` to prevent interpolation artifacts on score events.


## crates\pong\src\main.rs
**Purpose:** CLI entry point for local 2P Pong or online matchmaking via URL.

**Key items:**
*   `OnlineConfig` struct for remote connection args
*   `run_online` function for networked play
*   `run_local` function for local hotseat play

**Depends on:**
*   `pong` crate

**Gotchas:**
*   `PASSWORD` defaults to empty string if passed as `-` (intentional)
*   `USERNAME` env var is a fallback for player handle


## crates\pong\src\online.rs
**Purpose:** Server-authoritative arena shooter client with client-side movement prediction, deterministic obstacle rendering, and a procedurally generated cyberpunk sidearm (or GLB fallback).

**Key items:**
*   `ShooterGame` struct: Main game loop, input handling, prediction, and rendering.
*   `push_gun` / `push_parts`: Procedural cube pistol or GLB viewmodel rendering.
*   `obstacle_height`: Deterministic cosmetic height derived from obstacle coords.
*   `NetChan`: Platform-specific (native thread or WASM) WebSocket abstraction for `C2S`/`S2C`.

**Depends on:** `ember_engine`, `pong_core::proto`, `tungstenite`, `serde`, `wasm-bindgen`.

**Gotchas:**
*   `PROTO_VERSION` and `STATE_EVERY_TICKS` are critical constants; prediction relies on `last_tick - lag`.
*   Native builds require `rustls` crypto provider installation; WASM builds use JS closures.
*   `Sfx` playback is throttled to a max of 6 per frame to prevent audio buffer overflow.


## crates\pong\src\sound.rs
**Purpose:** Procedurally generates 44.1kHz mono sound effects for Pong (Shot, Hit, etc.) using synthesized waveforms; abstracts Rodio (native) and Web Audio (wasm) platforms.

**Key items:**
*   `Sfx` enum: Sound effect identifiers.
*   `synth`: Generates `Vec<f32>` samples using noise, exponential decay envelopes, and pitch sweeps.
*   `Audio`: Platform-specific struct managing streams and contexts.

**Depends on:** `rodio`, `wasm-bindgen`, `web_sys`

**Gotchas:** Web Audio context is lazily created on the first user gesture (pointerdown) to comply with browser autoplay policies; `SAMPLE_RATE` (44100) is used for buffer sizing.


## deploy\deploy-pages.sh
**Purpose:** Builds WASM bundles and publishes the games hub to GitHub Pages, materializing an archived game version from a specific commit.

**Key items:**
*   `V1_COMMIT` ("e7b85e8") — Hash of the original web build used to materialize v1 archives
*   `ARENA_LIVE` ("games/arena/v7") & `PONG_LIVE` ("games/pong/v2") — Live version directories
*   `server.json` — Cache-busting version stamp updated via embedded Python
*   `wasm-bindgen` — Generates JS/WASM bundles without TypeScript

**Depends on:** `git`, `cargo`, `wasm-bindgen`, `python`

**Gotchas:** 
*   The `server.json` version stamp `v` is updated every deploy (int timestamp).
*   v1 Pong archives are only materialized on first run if missing.
*   `git worktree` cleanup (`remove --force`) is required after push.


## deploy\deploy-pong-online.sh
**Purpose:** Automates full online deployment of the Pong server: builds, tunnels via Cloudflare, publishes the public URL to GitHub Pages, and validates WebSocket connectivity.

**Key items:** `deploy-pong-online.sh` script; `specht` SSH host; `cloudflared` tunnel; `gh-pages` Git worktree; `server.json` publication.

**Depends on:** `ssh` remote execution, `git` worktrees, `cloudflared` binary, `pong-server` binary.

**Gotchas:** Cloudflare mints a fresh domain on every restart; port 7778 is denied by SELinux, port 7780 is used; SSH `BatchMode=yes` required for no-tty execution; `pkill` regex must avoid matching its own shell process.


## deploy\deploy-specht.sh
**Purpose:** Deploys ember-server to the Specht instance via a WireGuard tunnel.

**Key items:**
*   `BIND` (10.72.0.1:7777)
*   `ember-build` toolbox invocation
*   `pkill` pattern matching hack

**Depends on:** `specht` (SSH), `ember-build` (Toolbox)

**Gotchas:** `pkill` shell injection bug prevents combined launch/restart calls. Server binds exclusively to the WG tunnel IP (10.72.0.1), not public internet.


## target\debug\build\glutin_wgl_sys-bdd43bb34c7c497c\out\wgl_bindings.rs
**Purpose:** Generated C FFI bindings for Windows OpenGL extension functions (WGL).

**Key items:**
*   **types:** HGLRC, HDC, PIXELFORMATDESCRIPTOR, LAYERPLANEDESCRIPTOR, RECT, GLDEBUGPROC
*   **Functions:** wglGetProcAddress, wglCreateContext, wglMakeCurrent, wglSwapLayerBuffers, wglUseFontOutlines
*   **Constants:** SWAP_OVERLAY1-15, SWAP_UNDERLAY1-15, FONT_LINES, FONT_POLYGONS

**Depends on:** std::os::raw, winapi types

**Gotchas:** `__gl_imports` module shadows raw types; extern "system" linkage required for Windows APIs.


## target\debug\build\glutin_wgl_sys-bdd43bb34c7c497c\out\wgl_extra_bindings.rs
**Purpose:** Generated procedural bindings for the Windows-specific WGL (Win32 Graphics Library) extension API, wrapping OpenGL functions for creating contexts, swapping buffers, and managing pixel buffers.

**Key items:**
*   `Wgl`: Wrapper struct with `load_with` and unsafe function pointers
*   `ChoosePixelFormatARB`, `CreateContextAttribsARB`, `MakeCurrent`: Core WGL context functions
*   `SwapIntervalEXT`, `SwapLayerBuffers`: Buffer synchronization
*   `GetPixelFormatAttribivARB`: Querying framebuffer attributes
*   `UseFontBitmaps`, `UseFontOutlines`: Text rendering

**Depends on:** `std::os::raw`, `std::mem`

**Gotchas:** All functions are `#[inline] unsafe`; must manually call `load_with` and check `is_loaded` before calling; `GetProcAddress` returns `*mut PROC` (opaque function pointer); `Wgl` implements `Send` implicitly.


## target\debug\build\khronos_api-b2b914fbe4108dff\out\webgl_exts.rs
**Purpose:** Embeds a static byte slice containing XML descriptors for 36 WebGPU/WebGL extensions, compiled from khronos_api source files during build time.
**Key items:** `&[&[u8]]`, `ANGLE_instanced_arrays`, `EXT_*`, `OES_*`, `WEBGL_*`
**Depends on:** khronos_api
**Gotchas:** `&*` dereferencing required to load `include_bytes!` output; XML paths are registry-specific, not source-relative.


## target\debug\build\serde_core-990d171663c0c16d\out\private.rs
**Purpose:** Re-exports public items from the `crate::private` module for internal use within the serde_core build artifact.

**Key items:**
*   `__private229`: Module container.

**Depends on:** `crate::private` (source module)

**Gotchas:** none noted


## target\debug\build\serde-294319a86a5677f1\out\private.rs
**Purpose:** Provides an internal module alias for serde_core to expose its private items, acting as a re-export stub in the build artifact.

**Key items:**
*   `__private229`: Module alias for `serde_core::__private229`
*   `serde_core_private`: Imported alias for serde_core private module

**Depends on:** `serde_core`

**Gotchas:** `none noted`


## target\debug\build\thiserror-d8803c9bb2a97fa4\out\private.rs
**Purpose:** Exposes internal `thiserror` crate internals (`crate::private`) under a hidden module alias for macro expansion.

**Key items:**
*   `__private20` module (hidden)
*   Re-exports `crate::private` items

**Depends on:** `thiserror` crate

**Gotchas:** None noted


## web\index.html
**Purpose:** Landing page for the Ember web client; discovers server config, renders a game catalog, and connects to the live lobby feed via WebSocket.

**Key items:**
*   `getAccount()`: Generates or retrieves a random handle from localStorage.
*   `refreshLobbies()`: Polls the game server for open games and renders them.
*   `launch()`: Redirects to the selected game version path.

**Depends on:** `server.json`, `games.json` (external assets), `localStorage`

**Gotchas:**
*   `proto: 0` is hardcoded for the browser lobby handshake.
*   `arenaLivePath` is dynamically resolved from the catalog JSON.
*   Browser accounts are created automatically on first game click.
*   `WebSocket` URL defaults to `serverCfg.ws` if no manual override exists.


## Cargo.toml
**Purpose:** Defines the workspace configuration, listing all crates and shared dependencies for the Ember engine ecosystem.

**Key items:**
*   `workspace.members` - List of crate paths.
*   `workspace.package` - Global `edition` and `version`.
*   `workspace.dependencies` - Centralized `winit`, `wgpu`, `glam`, `serde`, `tracing`, and `tungstenite` versions.
*   `profile.dev` - `opt-level` 1 for dev code, 3 for dependencies.

**Depends on:** None.

**Gotchas:** The `rustls` dependency explicitly requires `default-features = false` and the `ring` feature to prevent crypto provider conflicts.


