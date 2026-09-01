# ember

https://endersgamesdev.github.io/EmberEngine/
#Ender build this with ai 

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
  ember-client-net/ shared native/wasm connection lifecycle and replay plumbing
  ember-legacy/   moving capability boundary for frozen hosted versions
  ember-net/      canonical outer JSON/WebSocket lobby protocol
  ember-server/   sole game-neutral host and version registry
  arena/          Arena client — v0 pong classic + online shooter; native bin + wasm lib
  arena-core/      shared pong sim (deterministic, 60 Hz) + online JSON protocol
  arena-server/    public matchmaking + match server (WebSocket, lobbies with
                  optional passwords, authoritative sim per match)
games/            frozen game/version crates plus the hosted-set manifest
web/              static page: menu, lobby browser, wasm game (GitHub Pages)
deploy/           page, Arena, Fire, and watchdog deployment scripts
docs/             design documents (atw-first-rendering.md is adopted policy)
```

Strict one-way layering: game → scene/simulation → renderer → platform.

## Multiplayer

The native TCP cube demo has been retired. Multiplayer is migrating to one host, one outer protocol, and versioned frozen game contracts as described in [`docs/one-server-evergreen.md`](docs/one-server-evergreen.md).

## Roadmap

- [x] **0. Base**: workspace, window, wgpu surface, clear color
- [x] **0.5 Multiplayer online**: the retired cube prototype established an authoritative server, client snapshot interpolation, and shared protocol
- [x] **0.6 ATW-first presenter (stage A)**: scene renders offscreen
      (SceneFrame, color+depth); a presenter pass owns the swapchain
      (identity warp today — see `docs/atw-first-rendering.md`, adopted)
- [x] **0.7 First game + web build**: 3D pong (2 players, one keyboard),
      compiled to wasm (WebGPU with WebGL2 fallback), hosted on GitHub Pages
- [x] **1 (textures half). Per-mesh textures + UVs**: scene pass samples a
      per-mesh texture at group(1); GLB loader reads TEXCOORD_0; fly camera
      still open (lands as rotation-only warp, ATW stage B)
- [x] **2 (first half). glTF mesh pipeline**: Blender (scripted, headless)
      → .glb → engine multi-mesh instancing; the arena viewmodel (pistol +
      hands) is authored via `tools/make_assets.py`. Level editing and
      textures still ahead.
- [x] **3 (lighting half)**: Blinn-Phong + top sheen + view-depth fog in the
      scene shader; shadow mapping still open
- [ ] 4. Fixed-timestep loop + physics (rapier)
- [x] **5. egui debug overlay + hot-reloadable WGSL shaders**: the ATW test
      rig from the doc's §6 — F3 overlay (presenter-composited) with a
      scene-Hz throttle + frame-age/latency readouts; shaders hot-reload
      from disk on native
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

- **Local (Windows)**: primary dev machine.
- **Remote servers** (via wireproxy tunnels + SSH, see `~/.ssh/config`):
  - `specht` — hosts the live Arena and Fire deployment-continuity servers.
  - `adler` — RTX 4090: reserved for asset baking, Linux/Vulkan testing, CI-style builds.

## Games hub

**<https://endersgamesdev.github.io/EmberEngine/>** is a games hub: an active-lobby
showcase loads first (jump into any running game), below it the catalog from
`games.json` — every game with a version picker where the newest build is
always the "live" one and old builds stay playable from frozen
`games/<game>/<version>/` directories on gh-pages. Clicking any game
auto-creates an account (a generated handle like `neon-fox-42`, stored only
in the player's browser, renameable via the header chip) — no forms.
The hub lists lobbies without loading a game bundle: the server lets any
protocol version Hello + list, and enforces the live protocol only on
create/join.

## Arena v0: the pong classic

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — local (2 players, one
keyboard: `A`/`D` vs `←`/`→`, first to 7) or **online matchmaking**: pick a
handle, create a lobby (password optional) or join an open one from the
list. The match server is authoritative (the shared deterministic 60 Hz sim
in `crates/arena-core/src/sim.rs` runs server-side); clients stream inputs
and interpolate 30 Hz state. Either key set steers your paddle online, and
player 2 gets a flipped camera so they also play from "their" side.

Online infrastructure: `arena-server` (WebSocket + JSON, `arena-core/proto.rs`)
runs on specht bound to loopback, fronted by a **Cloudflare quick tunnel** —
a free public `https://….trycloudflare.com` domain that CHANGES every time
the tunnel restarts. `deploy/deploy-pong-online.sh` rebuilds + restarts
server and tunnel, then publishes the fresh domain to `server.json` on the
Pages site (fetched cache-busted, so the page always finds the current
server; its `v` stamp also cache-busts the wasm bundle per deploy). Server
log: `~/arena-server.log`, tunnel log: `~/cloudflared.log` on specht.
Headless check: `cargo run -p arena-server --example wsbot -- <URL> create|join <LOBBY> [PW|-] [HANDLE] [SECS] [MODES]`.
`MODES` is a comma-separated list of `shield`, `jump`, `nofire` — without it
the bot never raises the shield or jumps, so a green run says nothing about
either. Two bots, one plain and one `shield,nofire`, demonstrate a reflect.

Native: `cargo run -p arena --bin arena-app` (local) or
`arena-app online wss://… create|join LOBBY [PASSWORD|-] [HANDLE]`

Web build (needs `wasm32-unknown-unknown` target + `wasm-bindgen-cli`):

```
cargo build --target wasm32-unknown-unknown --release -p arena --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm
```

then publish `web/` to the `gh-pages` branch — or just run
`bash deploy/deploy-pages.sh`, which does all of the above.

## Fire Racer

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — pick Fire Racer, then *Practice alone* against seven AI cars or *Race online*. `W`/`S` drive, `A`/`D` steer, `Space` is the handbrake that breaks traction into a drift, `Shift` spends one of three boost charges. Three laps of a castle bailey.

Track, car physics and lap timing live in `crates/fire-core`, which is shared between the client's prediction and the authoritative server exactly as `arena-core` is for the arena — the client predicts its own car and reconciles against server state thirty times a second. The castle props are generated meshes and every texture is procedural, so none of them costs a download byte.

Fire carries its **own** `PROTO_VERSION`, independent of `arena-core`'s. That is deliberate: bumping one game's protocol must never gate the other game's join. `server.json` records both, as `proto` and `fire_proto`.

Online infrastructure: `fire-server` on its own port behind its own Cloudflare quick tunnel, published to `server.json` under its own `fire_ws` key — a separate script, port, tunnel and key from the arena's, so redeploying one game can never knock the other offline. `bash deploy/deploy-fire-online.sh` rebuilds, restarts, and republishes; it health-checks by speaking the protocol (Hello must be answered with Welcome) before it publishes anything, so a failed deploy leaves the previous address in place rather than pointing players at a server that never came up.

### Why no link here ever carries a tunnel domain

A quick tunnel mints a **new** random `*.trycloudflare.com` hostname every time it restarts, so any such URL written into this README, or into a page, would be wrong by the next restart.

Nothing needs one. The hub and each game page fetch `server.json` cache-busted at load and take the current address from it, so the only link anyone needs is the stable Pages URL above — the game selector already hooks itself to whichever server is live. `server.json` is the single source of truth and the deploy scripts are the only things that write it, each merging its own key so they cannot clobber each other.

What that does *not* survive is an unattended restart: the servers come back at an address `server.json` does not yet name. `deploy/watchdog.sh` closes that gap by probing the published address and redeploying when it stops answering. See `deploy/README-watchdog.md`.

## Run

```
cargo run -p game                 # multiplayer arena (auto-connects to specht)
cargo run -p arena --bin arena-app  # 3D pong, 2 players at one keyboard
```
