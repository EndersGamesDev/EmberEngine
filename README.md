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
  ember-net/      shared protocol: messages, framing, sanitization
                  (+ examples/netbot.rs — headless verification client)
  ember-server/   headless dedicated server: single 60 Hz sim thread,
                  thread-per-connection IO feeding it events over channels
  game/           the arena client: net session, world interpolation, scene
  pong/           3D pong client — local + online modes; native bin + wasm lib
  pong-core/      shared pong sim (deterministic, 60 Hz) + online JSON protocol
  pong-server/    public matchmaking + match server (WebSocket, lobbies with
                  optional passwords, authoritative sim per match)
web/              static page: menu, lobby browser, wasm game (GitHub Pages)
deploy/           deploy-specht.sh (arena server) · deploy-pages.sh (web) ·
                  deploy-pong-online.sh (pong server + tunnel + server.json)
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

### Diagnostics (tracing)

All crates emit structured [tracing](https://docs.rs/tracing) events
(`RUST_LOG` filtering still works; wgpu/winit `log` records are bridged in;
on wasm, events land in the browser console). Built-in stall/lag detection:

| Where  | Signal | Meaning |
|--------|--------|---------|
| server | `sim stall: fell behind the tick clock` | sim thread starved >10 ticks; clock resynced |
| server | `tick_overruns` / `max_tick_busy_us` in `server health` | tick body exceeded its 16.7 ms budget |
| server | `client lagging` / `client recovered from lag` | joined client silent >3 s (kick at 10 s) |
| client | `frame stall` | >100 ms gap between frames (GC, OS hitch, hidden tab) |
| client | `snapshot stream stale` / `recovered` | no server snapshot for >300 ms (lag spike) |
| client | `network lag: high round-trip time` | keepalive RTT >250 ms (RTT shown in the periodic `online` status) |

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

## Pong

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — local (2 players, one
keyboard: `A`/`D` vs `←`/`→`, first to 7) or **online matchmaking**: pick a
handle, create a lobby (password optional) or join an open one from the
list. The match server is authoritative (the shared deterministic 60 Hz sim
in `crates/pong-core/src/sim.rs` runs server-side); clients stream inputs
and interpolate 30 Hz state. Either key set steers your paddle online, and
player 2 gets a flipped camera so they also play from "their" side.

Online infrastructure: `pong-server` (WebSocket + JSON, `pong-core/proto.rs`)
runs on specht bound to loopback, fronted by a **Cloudflare quick tunnel** —
a free public `https://….trycloudflare.com` domain that CHANGES every time
the tunnel restarts. `deploy/deploy-pong-online.sh` rebuilds + restarts
server and tunnel, then publishes the fresh domain to `server.json` on the
Pages site (fetched cache-busted, so the page always finds the current
server; its `v` stamp also cache-busts the wasm bundle per deploy). Server
log: `~/pong-server.log`, tunnel log: `~/cloudflared.log` on specht.
Headless check: `cargo run -p pong-server --example wsbot -- <URL> create|join <LOBBY> [PW|-] [HANDLE] [SECS] [MODES]`.
`MODES` is a comma-separated list of `shield`, `jump`, `nofire` — without it
the bot never raises the shield or jumps, so a green run says nothing about
either. Two bots, one plain and one `shield,nofire`, demonstrate a reflect.

Native: `cargo run -p pong --bin pong-app` (local) or
`pong-app online wss://… create|join LOBBY [PASSWORD|-] [HANDLE]`

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
