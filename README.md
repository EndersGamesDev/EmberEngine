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
  game/           the TCP arena client (ember-net stack): net session, world
                  interpolation, scene — separate from the WebSocket shooter
  pong/           the web client crate: BOTH the arena shooter and 3D pong,
                  local + online; native bin + wasm lib
  pong-core/      the shared deterministic 60 Hz sims — the arena shooter
                  (shooter.rs) and pong (sim.rs) — plus their JSON protocol
  pong-server/    public matchmaking + match server (WebSocket, lobbies with
                  optional passwords, authoritative sim per match)
  fire/           Fire Racer client: local + online; native bin + wasm lib
  fire-core/      shared deterministic racing sim (track, car, laps) + its own
                  protocol, versioned independently of pong-core's
  fire-server/    Fire Racer matchmaking + race server (WebSocket)
  ember-editor/   the native level builder: fly, place, move/rotate/scale,
                  spawn points; its document is pong-core's Level
web/              static page: menu, lobby browser, wasm game (GitHub Pages)
deploy/           deploy-specht.sh (arena server) · deploy-pages.sh (web) ·
                  deploy-pong-online.sh (pong server + tunnel + server.json) ·
                  deploy-fire-online.sh (fire server + its own tunnel)
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
- [x] **5.5 Authored levels + the graphics update (arena v13)**: the shooter
      sim takes a `Level` off the wire instead of a seed, so the server names
      one authored map ("Trench City") and every client builds the same
      obstacles from it; `Obstacle` grows a bottom (`base`) and a cover
      `kind`, which makes tunnels and stacked cover representable; the
      renderer grows CPU-built mipmaps and per-frame fog; and the picture →
      mesh → texture runbook that produced the map is checked in under
      `tools/v13/` (`docs/asset-pipeline.md`, Path E)
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

## Arena Shooter

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — pick Arena Shooter. First-person drop-in deathmatch for up to 8 players: WASD + mouse look, Space jump, Shift sprint, C crouch, RMB zoom, R reload, Q raises a shield that reflects the round back at the shooter, E is a melee that goes through a raised shield, and headshots kill outright.

This is what "online" means in this repo. It runs on the WebSocket stack, not the TCP one: the shared deterministic 60 Hz sim is `crates/pong-core/src/shooter.rs` (the paddle sim in `sim.rs` is local pong only) and the authoritative server is `pong-server`. Clients predict their own movement against the same sim and reconcile against 30 Hz server state; **bullets are stepped server-side only**, which is why hit registration may use `f32` transcendentals at all.

### v13 — Trench City

v13 replaces the seeded box field with **one authored map every peer builds identically**. The server names it (`GameJoined.map`) and `Level::named` resolves it, so the next map is an additive string rather than another protocol bump.

The map is a square in an old European city that is being fought over, four-fold symmetric, three concentric bands so eight players spread out instead of piling into one lane. The cover is real object classes rather than anonymous boxes:

| Class | Height | What it does |
|---|---|---|
| `Container` | 2.6 (and one stacked pair at 5.2) | Closed 40-ft shipping container at real proportions. Hard cover, and its roof is high ground you cannot reach from the floor — the jump apex is 1.76. |
| `Crate` | 1.2 | Wooden supply crate. A climbing step, and a **fire step** against a trench's outer wall: standing on one puts the eye at 2.65, over a 2.5 wall. |
| `Ammo` | 0.55 | Ammunition box. The first step of every climbing chain: floor → ammo → crate → container roof. |
| `Wall` | 2.5 | The trench lines. Two parallel walls with a 3 m corridor between them is a trench. |
| `Roof` | base 2.5, top 2.9 | The first obstacle with a **bottom**. Walk under it, stand on it, and no round passes through it — a roofed trench section is a tunnel. **All four weapon pads are inside the tunnels.** |
| `Sandbag`, `Rubble`, `Plinth` | 1.1 / 0.7 / 2.2 | Spawn cover, low cover, and the granite plinth under the bronze statue at the centre. |

Around the square the client draws the city — a cathedral, a ring of Haussmann and art-nouveau façades, street lamps, burnt-out cars, and a golden-hour sky. All of it is client-only decor listed in the `Level`, so every client draws the same city; none of it is a collision volume. How the pictures and meshes were generated is `docs/asset-pipeline.md`, Path E, with the runbook checked in under `tools/v13/`.

### What the protocol bump means for old builds

The authored level is a protocol change, so `PROTO_VERSION` in `crates/pong-core/src/proto.rs` goes to **13**. It has to: a v12 client would predict its movement against the seeded boxes while the server resolved it against the trench city, so every wall would be either invisible or imaginary. That is the `CLAUDE.md` test — an old peer that "plays a different game" is a bump, not a `#[serde(default)]`.

**The join gate is exact equality**, so the moment the server moves to 13 the frozen hub pages v7–v12 go **list-only**: they still see the lobby list (listing is ungated on purpose) but can no longer create or join, and they already say "archived" in the version picker. The lobby browser on the hub is unaffected — it sends `proto: 0` against `ListLobbies` and never loads a game bundle.

The two sides deploy in this order, in one window: `deploy/deploy-pong-online.sh` (server first, so it speaks 13) and then `deploy/deploy-pages.sh` (which ships the staged v13 page and prints the bump warning it detects from `server.json`).

### Running it

Online infrastructure: `pong-server` (WebSocket + JSON, `pong-core/proto.rs`) runs on the game host bound to loopback, fronted by a **Cloudflare quick tunnel** — a free public `https://….trycloudflare.com` domain that CHANGES every time the tunnel restarts. `deploy/deploy-pong-online.sh` rebuilds + restarts server and tunnel, then publishes the fresh domain to `server.json` on the Pages site (fetched cache-busted, so the page always finds the current server; its `v` stamp also cache-busts the wasm bundle per deploy). Server log: `~/pong-server.log`, tunnel log: `~/cloudflared.log`.

Headless check: `cargo run -p pong-server --example wsbot -- <URL> create|join <LOBBY> [PW|-] [HANDLE] [SECS] [MODES]`. `MODES` is a comma-separated list of `shield`, `jump`, `nofire` — without it the bot never raises the shield or jumps, so a green run says nothing about either. Two bots, one plain and one `shield,nofire`, demonstrate a reflect.

Native: `cargo run -p pong --bin pong-app online wss://… create|join LOBBY [PASSWORD|-] [HANDLE]`

Web build (needs `wasm32-unknown-unknown` target + `wasm-bindgen-cli`):

```
cargo build --target wasm32-unknown-unknown --release -p pong --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/pong.wasm
```

then publish `web/` to the `gh-pages` branch — or just run `bash deploy/deploy-pages.sh`, which does all of the above.

## Pong

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — pick Pong Classic. The first ember game, and today a **local two-player game only**: one keyboard, `A`/`D` against `←`/`→`, first to 7. There is no online pong; the paddle sim in `crates/pong-core/src/sim.rs` runs entirely in the client.

Native: `cargo run -p pong --bin pong-app`. It ships in the same wasm bundle as the arena shooter, built by the commands above.

## Fire Racer

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — pick Fire Racer, then *Practice alone* against seven AI cars or *Race online*. `W`/`S` drive, `A`/`D` steer, `Space` is the handbrake that breaks traction into a drift, `Shift` spends one of three boost charges. Three laps of a castle bailey.

Track, car physics and lap timing live in `crates/fire-core`, which is shared between the client's prediction and the authoritative server exactly as `pong-core` is for the arena — the client predicts its own car and reconciles against server state thirty times a second. The castle props are generated meshes and every texture is procedural, so none of them costs a download byte.

Fire carries its **own** `PROTO_VERSION`, independent of `pong-core`'s. That is deliberate: bumping one game's protocol must never gate the other game's join. `server.json` records both, as `proto` and `fire_proto`.

Online infrastructure: `fire-server` on its own port behind its own Cloudflare quick tunnel, published to `server.json` under its own `fire_ws` key — a separate script, port, tunnel and key from the arena's, so redeploying one game can never knock the other offline. `bash deploy/deploy-fire-online.sh` rebuilds, restarts, and republishes; it health-checks by speaking the protocol (Hello must be answered with Welcome) before it publishes anything, so a failed deploy leaves the previous address in place rather than pointing players at a server that never came up.

### Why no link here ever carries a tunnel domain

A quick tunnel mints a **new** random `*.trycloudflare.com` hostname every time it restarts, so any such URL written into this README, or into a page, would be wrong by the next restart.

Nothing needs one. The hub and each game page fetch `server.json` cache-busted at load and take the current address from it, so the only link anyone needs is the stable Pages URL above — the game selector already hooks itself to whichever server is live. `server.json` is the single source of truth and the deploy scripts are the only things that write it, each merging its own key so they cannot clobber each other.

What that does *not* survive is an unattended restart: the servers come back at an address `server.json` does not yet name. `deploy/watchdog.sh` closes that gap by probing the published address and redeploying when it stops answering. See `deploy/README-watchdog.md`.

## Run

```
cargo run -p game                 # multiplayer arena (auto-connects to specht)
cargo run -p pong --bin pong-app  # 3D pong, 2 players at one keyboard
```
