<!-- Drafted by the AI worker cluster from the code map and repo audit (2026-08-29); reviewed lightly - verify details against the source before relying on them. -->
# ember architecture

## Crate responsibilities and boundaries

The workspace is organized around three core layers: the **Platform** (`ember-engine`), the **Game Logic** (`game`, `pong`), and the **Shared Infrastructure** (`ember-net`, `pong-core`).

*   **`ember-engine`**: The platform and rendering abstraction. It provides the `EmberGame` trait and the `Renderer`. It handles window creation (`winit`), GPU context (`wgpu`), and the ATW-first rendering flow (SceneFrame -> Presenter -> Swapchain). It is the only layer aware of platform-specific I/O and windowing events.
*   **`game`**: Client-side logic for the simple multiplayer arena (movement + animated characters; no combat — the shooter lives in the web games). It implements `EmberGame` and manages session state (Online/Offline). It consumes `ember-engine` for rendering and `ember-net` for networking. It handles client-side interpolation and input prediction.
*   **`pong`**: Client-side logic for the 3D Pong game. It implements `EmberGame` and provides local (hotseat) and online (WebSocket) modes. It consumes `ember-engine` for rendering and `pong-core` for authoritative physics.
*   **`pong-core`**: Shared deterministic sims for BOTH web games: the pong sim (sim.rs) and the arena-shooter sim (shooter.rs), plus their WebSocket protocol (proto.rs). It isolates the deterministic simulation (`Sim`), state definitions (`PlayerSt`, `Bullet`), and the WebSocket protocol (`proto`). It is designed to be platform-agnostic and testable headlessly.
*   **`ember-net`**: A shared library for the arena shooter network protocol. It defines the TCP message format (`ClientMsg`, `ServerMsg`), serialization using postcard, and constants for the server architecture. It is intended for use by the `game` crate and the `ember-server`.
*   **`ember-server`**: A headless dedicated server for the arena shooter. It runs the simulation loop at 60Hz and handles TCP I/O via separate threads. It re-exports constants from `ember-net`.

## Parallel Protocols

The workspace maintains two parallel, non-overlapping protocols: **TCP Arena Shooter** and **WebSocket Pong/Shooter**. They are separate to maintain strict isolation between distinct gameplay modes and to facilitate independent development and deployment of the two games.

*   **TCP Protocol (`ember-net`)**:
    *   Used by the `game` crate (client) and `ember-server`.
    *   Transport: TCP.
    *   Format: Length-prefixed binary using `postcard`.
    *   Purpose: Reliable, low-latency transport for the arena shooter's movement and state snapshots.
    *   Key items: `ClientMsg`, `ServerMsg`, `PlayerState`, `TICK_HZ` (60).

*   **WebSocket Protocol (`pong-core`)**:
    *   Used by the `pong` crate (client), `pong-server`, and the web lobby.
    *   Transport: WebSocket (JSON).
    *   Format: Tagged enums (`C2S`, `S2C`) using `serde`.
    *   Purpose: Matchmaking, lobby management, and state synchronization for the Pong game and the arena shooter (in `pong`).
    *   Key items: `LobbyInfo`, `PState`, `BState`, `PROTO_VERSION` (7), `STATE_EVERY_TICKS` (2).

## ATW-first render flow

The renderer implements an **Always-Texture-Warp (ATW)** architecture, separating the scene pass from the presentation pass.

1.  **Scene Pass (`Renderer`)**:
    *   The renderer owns an offscreen `SceneTargets` (color/depth).
    *   The game provides a `Frame` containing the `Camera`, `Instance` list, and `MeshData`.
    *   The renderer performs all lighting and geometry rendering into the offscreen target, producing a linear LDR `SceneFrame` texture.
2.  **Presenter Pass (`present.wgsl`)**:
    *   The presenter binds the `SceneFrame` and renders a fullscreen triangle.
    *   The WGSL shader samples the scene texture and applies a viewport transformation, outputting directly to the swapchain.
3.  **Swapchain Integration**:
    *   This flow allows for dynamic resolution scaling (`scene_scale`) and platform-agnostic rendering (e.g., resizing logic for WASM vs. Desktop).

## Asset pipelines

Assets are managed by `ember-engine` and `game`, focusing on procedural generation and deterministic loading.

*   **Textures**: Managed by `game`. Supports standard image loading and procedural generation via AI generation (referenced in code map).
*   **3D Geometry**:
    *   **GLB**: Loaded via `assets::load_glb()`. This parses glTF files, flattens triangles, and transforms vertices into unindexed `MeshData` for the renderer.
    *   **Procedural**: Used by `pong` and `game` to generate geometry (paddles, balls, weapons) on the fly. This includes deterministic obstacle rendering for the arena shooter and procedural pistol construction (`push_gun`).
*   **Shaders**: Defined in `.wgsl` files. `shader.wgsl` handles instanced drawing with uniform-per-axis scaling and Y-rotation. `present.wgsl` handles the ATW fullscreen blit.

## Deterministic-sim rules

The simulation is designed for replayability and synchronization.

*   **Fixed Timestep**: The simulation runs at a fixed 60Hz (`TICK_HZ`).
*   **Determinism**: The `pong-core` simulation relies on **deterministic RNG**. Seeding is critical; clients and servers must use identical seeds to ensure physics results match exactly.
*   **State Definition**: The `shooter` module defines the authoritative state (`PlayerSt`, `Bullet`). The `sim` module provides a pure, headless runner for the Pong game.
*   **Lag Compensation**: The arena shooter implements a rewind mechanism (`MAX_REWIND_TICKS` = 18) to look back in time to calculate hits. This is handled in `pong-core`.

