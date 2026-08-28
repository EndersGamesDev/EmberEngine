# ember

A 3D game and its engine, built from scratch in Rust. No general-purpose engine —
the engine exists to serve exactly one game and gets to make sharp trade-offs
because of it.

## Design pillars

1. **Engine = compiler + small runtime.** Assets (glTF, textures, shaders) get
   baked offline into flat, memory-mappable binary blobs. The shipped runtime
   maps, uploads, runs — no parsing at startup.
2. **GPU-driven rendering.** Bindless resources, GPU culling via compute,
   indirect draws, a small render graph. The CPU hands over a camera, not a
   draw list.
3. **Deterministic simulation.** Fixed timestep, seeded RNG, fixed update
   order. Buys free replays, save states, rollback netcode, and reproducible
   bugs.
4. **Iteration speed is a feature.** Hot-reloadable shaders and game code,
   in-engine inspector/profiler, time-travel debugging on top of the
   deterministic sim. Blender is the content editor; the engine is where the
   *running* game is inspected.

## Workspace layout

```
crates/
  ember-engine/   the engine library
    app.rs        platform layer: window, event loop, input (winit)
    renderer.rs   GPU layer: owns wgpu; nothing above it touches the GPU
  ember-net/      shared protocol: messages, framing, sanitization
                  (+ examples/netbot.rs — headless verification client)
  ember-server/   headless dedicated server: single 60 Hz sim thread,
                  thread-per-connection IO feeding it events over channels
  game/           the arena client: net session, world interpolation, scene
  pong/           3D pong — first playable; native bin + wasm lib
web/              static page for the wasm build (deployed to GitHub Pages)
deploy/           deploy-specht.sh — package, remote-build, restart
docs/             design documents (atw-first-rendering.md is adopted policy)
```

Strict one-way layering: game → scene/simulation → renderer → platform.

## Multiplayer

Server-authoritative over TCP (length-prefixed [postcard] frames — TCP because
the WireGuard userspace tunnels forward TCP only; the framing keeps the
transport swappable). Clients send held movement intents; the server
integrates at a fixed 60 Hz and broadcasts snapshots; clients interpolate
between snapshots. All client input is sanitized (NaN/magnitude/name/frame
size). Protocol changes bump `PROTOCOL_VERSION` in `ember-net`.

```
cargo run -p game                              # connect to 127.0.0.1:7777 (tunnel → specht)
cargo run -p game -- 127.0.0.1:7799 alice      # explicit server + name
cargo run -p ember-server -- --bind 127.0.0.1:7799        # local server
cargo run -p ember-net --example netbot -- 127.0.0.1:7777 bot 6   # headless check
```

If the server is unreachable the client runs offline (local-only cube).

Known limitations (accepted for now): snapshot interpolation degenerates to
snap-to-latest at frame rates ≤ 60 fps (a proper ~100 ms interpolation delay
buffer is future work), and the client connects before the window opens, so an
unreachable server makes launch pause ~4 s before the offline fallback.

## Roadmap

- [x] **0. Base**: workspace, window, wgpu surface, clear color
- [x] **0.5 Multiplayer online**: shared protocol (`ember-net`), authoritative
      60 Hz dedicated server (`ember-server`) deployed on specht, client with
      snapshot interpolation, players rendered as lit cubes
- [x] **0.6 ATW-first presenter (stage A)**: scene renders offscreen
      (SceneFrame, color+depth); a presenter pass owns the swapchain
      (identity warp today — see `docs/atw-first-rendering.md`, adopted)
- [x] **0.7 First game + web build**: 3D pong (2 players, one keyboard),
      compiled to wasm (WebGPU with WebGL2 fallback), hosted on GitHub Pages
- [ ] 1. First triangle → textured cube → fly camera (WASD + mouse;
      fly camera lands as rotation-only warp, ATW stage B)
- [ ] 2. glTF loading (Blender becomes the level editor)
- [ ] 3. Depth buffer, Blinn-Phong lighting, basic shadow mapping
- [ ] 4. Fixed-timestep loop + physics (rapier)
- [ ] 5. egui debug overlay + hot-reloadable WGSL shaders
- [ ] 6. Offline asset compiler (glTF → baked blobs)
- [ ] 7. GPU-driven rendering pass (bindless, culling in compute, indirect draws)
- [ ] Game-specific systems — driven by the game design, step by step

## Stack

| Concern    | Choice                                | Why                                        |
|------------|---------------------------------------|--------------------------------------------|
| Windowing  | `winit`                               | ecosystem standard, all platforms          |
| GPU        | `wgpu`                                | Vulkan/DX12/Metal/WebGPU from one codebase |
| Math       | `glam`                                | fast, SIMD, used by everyone               |
| Physics    | `rapier` (later)                      | writing collision detection is its own project |
| Assets     | glTF only                             | Blender exports it natively; one format done well |
| Debug UI   | `egui` (later)                        | inspector panels & sliders, quickly        |

## Infrastructure

- **Local (Windows)**: primary dev machine; `cargo run -p game` opens the dev window.
- **Remote servers** (via wireproxy tunnels + SSH, see `~/.ssh/config`):
  - `specht` — hosts the live dedicated server: `~/ember-src/target/release/ember-server`
    bound to `10.72.0.1:7777` (WireGuard-only, not public), log `~/ember-server.log`,
    built in toolbox `ember-build` (rustup + gcc). Redeploy: `bash deploy/deploy-specht.sh`.
    Local tunnel `127.0.0.1:7777 → 10.72.0.1:7777` in `~/.config/wireproxy-specht.conf`;
    other WG peers connect to `10.72.0.1:7777` directly.
    Caveats: started via nohup — does NOT survive a specht reboot; pkill patterns
    must not match their own ssh command line (see deploy script).
  - `adler` — RTX 4090: reserved for asset baking, Linux/Vulkan testing, CI-style builds.

## Pong

**Play online: <https://enderpeer.github.io/ember/>** — Player 1 (blue,
near): `A`/`D` · Player 2 (red, far): `←`/`→` · first to 7. Score pips sit on
top of the walls. The sim is pure, deterministic, fixed 60 Hz
(`crates/pong/src/sim.rs`) — the shape a networked version would replicate.

Native: `cargo run -p pong --bin pong-app`

Web build (needs `wasm32-unknown-unknown` target + `wasm-bindgen-cli`):

```
cargo build --target wasm32-unknown-unknown --release -p pong --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/pong.wasm
```

then publish `web/` to the `gh-pages` branch — or just run
`bash deploy/deploy-pages.sh`, which does all of the above.

## Run

```
cargo run -p game                 # multiplayer arena (auto-connects to specht)
cargo run -p pong --bin pong-app  # 3D pong, 2 players at one keyboard
```
