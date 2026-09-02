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
deploy/           host.sh (run a host on any Linux box) · host-name.sh ·
                  publish-host.sh (the ONLY writer of the server.json
                  address book: one entry per host, merged never clobbered)
                  deploy-pong-online.sh / deploy-fire-online.sh (ssh deploys)
                  deploy-pages.sh (web) · watchdog.sh · deploy-specht.sh
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

## Many hosts, one address book

**Why.** One machine ran the games. When it went down, or when it came back at a new address, nobody could play — and there were idle machines around that could have carried the game the whole time. The games now run on as many machines as we like, at the same time, and a page finds a working one by itself.

**What runs where now.** A **host** is one machine running the game servers behind its own public address. Hosts are independent: they share nothing, they never talk to each other, and a lobby lives on exactly one of them. What they share is the **address book** — a single file, `server.json`, published on the Pages site — which lists every host with its addresses, the commit it was built from, and which protocol each of its servers speaks. ("Protocol" here is the wire format a build speaks. A page and a server have to agree on it exactly before anyone can create or join; that is why the book records it per host.)

- **Names are automatic.** Each machine works out its own name once — two words, like `amber-otter` — from a hash of its hostname and user, and keeps it in `~/.ember/host-name`. Same box, same name, every deploy, so a publish replaces that host's line in the book instead of adding a new one. Every other host's line is left untouched.
- **Every host records its build**, as `r<N>` plus a short commit — `N` is the number of commits, so a bigger number is a newer build. It is stamped into the binary at compile time, so a machine sitting on an old commit keeps honestly reporting that commit.
- **Every server introduces itself in one round trip.** A page says hello; the reply now carries the host's name, its build, its commit, how many people are playing and how many lobbies are open. That is all a page needs to rank the hosts, and it costs no extra request.

**How a page chooses.** It loads the book, asks every listed host at once, and keeps the ones that answer. Then it picks the **newest build that speaks this page's protocol**, and among those, the **emptiest**, and among those, the **nearest** (whoever answered fastest). Everything else that answered becomes the fallback list, in that same order. If the chosen host goes quiet in the seconds before the game starts, the page walks down the list and tells the player it moved. Once the game is actually running there is no failover: a host that dies mid-game ends that game, exactly as before. Falling back is about never being unable to *start*.

Every page carries a small chip in its header naming the host it is on, its build and the round trip, with the commit and the raw address in the tooltip. The hub does more: it lists every host that answered, and every open lobby across all of them, each row tagged with the machine it lives on — so two people who happened to land on different machines can still find each other's game. A dropdown lets a player pin one host and always play there; "automatic" clears it again. A URL typed into the hub's server settings still overrides all of this for the arena, as it always did.

**How to run a host.** Three ways, in increasing order of independence.

1. **From this workstation, over ssh** — the existing path, for a machine we already have a key for:

   ```
   EMBER_HOST=<ssh name> bash deploy/deploy-pong-online.sh
   EMBER_HOST=<ssh name> bash deploy/deploy-fire-online.sh
   ```

   The target machine needs a Rust toolchain reachable the way those scripts expect, `~/bin/cloudflared`, and ssh that works without a prompt.

2. **On any Linux box, by itself** — no ssh from here, nothing to configure on this side:

   ```
   bash deploy/host.sh up        # clone or update, build, start, prove, publish
   bash deploy/host.sh status    # what is running, from what, at what address
   bash deploy/host.sh update    # rebuild only if the commit moved or something died
   bash deploy/host.sh down      # stop it
   ```

   It needs `git`, a Rust toolchain, `cloudflared` and `python3`. The first build takes minutes; every later one takes seconds. Settings live in `~/.ember/host.env`, which the script writes on first run with everything commented out at its default, so a one-off change needs no edit — just put it in front of the command. `up` proves both servers answer on loopback, then brings up the tunnels, then proves they answer again through the public address, and only publishes after that. A failed deploy leaves the previous address in the book rather than sending players at a server nobody proved was alive.

3. **As a mirror**, if you cannot write our address book — you publish your own one-host file and we link to it once:

   ```
   EMBER_PUBLISH='git@github.com:you/EmberEngine.git#gh-pages' bash deploy/host.sh up
   ```

   Send the resulting `host.json` URL over once; after that every update is yours alone and needs nobody's permission. `EMBER_PUBLISH=upstream` publishes straight into our book instead (needs push rights), and the default, `none`, prints the entry it *would* publish and changes nothing — useful for a dry run.

**Picking or keeping a version.** `EMBER_REF` says which commit a host runs. The ssh deploys default to `HEAD`; `host.sh` defaults to `origin/main`. Pin one deliberately with `EMBER_REF=v12 bash deploy/host.sh up`. This is the point of the whole scheme: a host that stays on an older commit keeps the frozen pages of that era playable, because those pages find the hosts on their own protocol while the live page keeps landing on the newest build. Everything a deploy publishes — version, commit, protocol number — is read from that ref and never from the working tree, so a host can never advertise a build it is not running.

**How updates flow.** Three routes, and they do not fight:

- The workstation deploys **push**: they build, restart, and republish the new address.
- `bash deploy/host.sh update` **pulls**: a host checks whether its ref moved and rebuilds only if it did, or if a server stopped running.
- `deploy/watchdog.sh` runs here, where the git credentials already are, and watches the hosts named in `EMBER_HOSTS`. It redeploys a host when `origin/main` moves or when a published address stops answering — which is what covers the Cloudflare quick tunnel taking a new random name after every restart. It refuses to redeploy on top of people who are playing, and it keeps state per host, so one machine that keeps failing cannot hold the others back.

**Where this honestly stands.** The pieces are built and the test suites pass; nothing has touched a real server yet.

- **Verified by running it**, on the final tree `5d8e94e` inside the build distro: `cargo clippy` clean and `cargo test` green on every crate this touches and their dependents (19 suites, 168 tests, 24 s), `cargo fmt` clean, the wasm check of both game bundles ok, the deploy suites green (syntax 55/55, publish-host 101/101, ssh-deploys 52/52, watchdog 16/16, host-pids 16/16, 5 s), the host-picking rule's 53 node tests green, and the loopback suite green (38/38, 26 s) — that one is the real thing on one machine: it builds both servers from a bare clone, starts them under the machine's generated name, answers the repo's own probes on loopback *and* through the address it published, publishes into a local repository with per-game build stamps, then updates, reports status and shuts down clean. An adversarial review (four lenses, two independent refuters per finding) then confirmed 32 defects, and all but the few named in the backlog are fixed on this branch.
- **Not verified**: the ssh deploys (`deploy-pong-online.sh`, `deploy-fire-online.sh`, `install-watchdog.sh`, `watchdog.sh`) have still not been run against a remote host since these changes — their ssh paths are exercised only through stand-in `ssh`/`scp` commands. What HAS run for real: `deploy/host.sh up` on the developer PC (inside its build distro) on 2026-09-02, which built r437, brought the arena and Fire Racer servers up behind quick tunnels, proved both through their public addresses (after learning to wait for DNS), and published the host `brass-heron` into the book; `deploy-pages.sh` then shipped this hub from the same clone, and the live hub picked that host with a 115 ms round trip. The old server on specht keeps running its previous arena build but is no longer in the book, because the book's legacy keys now follow the live protocol.
- **The protocol did not move.** Both games keep the version they had, because every new field is informational and no shot, join or race resolves differently without it. Every build that worked before still works, and old servers and new pages understand each other in both directions.

The full model — the file format, the exact picking rule, what a server reports and how a mirror works — is `docs/hosts.md`. What was deliberately *not* built, and what is still owed, is in `docs/plans/backlog.md`.

## Arena v0: the pong classic

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — local (2 players, one
keyboard: `A`/`D` vs `←`/`→`, first to 7) or **online matchmaking**: pick a
handle, create a lobby (password optional) or join an open one from the
list. The match server is authoritative (the shared deterministic 60 Hz sim
in `crates/arena-core/src/sim.rs` runs server-side); clients stream inputs
and interpolate 30 Hz state. Either key set steers your paddle online, and
player 2 gets a flipped camera so they also play from "their" side.

Online infrastructure: `arena-server` (WebSocket + JSON, `arena-core/proto.rs`) runs on each host bound to loopback, fronted by a **Cloudflare quick tunnel** — a free public `https://….trycloudflare.com` domain that CHANGES every time the tunnel restarts. `EMBER_HOST=<ssh name> bash deploy/deploy-pong-online.sh` rebuilds + restarts server and tunnel, then publishes that host's fresh domain into its own entry in `server.json` on the Pages site, leaving every other host's entry alone. The book is fetched cache-busted, so a page always sees the current addresses; its `v` stamp also cache-busts the wasm bundle per deploy. Server log: `~/pong-server.log`, tunnel log: `~/cloudflared.log` on the host. See **Many hosts, one address book** above and `docs/hosts.md`.
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

Fire carries its **own** `PROTO_VERSION`, independent of `arena-core`'s. That is deliberate: bumping one game's protocol must never gate the other game's join. `server.json` records both per host, as `proto` and `fire_proto`.

Online infrastructure: `fire-server` on its own port behind its own Cloudflare quick tunnel, published under its own `fire_ws` key — a separate script, port, tunnel and key from the arena's, so redeploying one game can never knock the other offline. `EMBER_HOST=<ssh name> bash deploy/deploy-fire-online.sh` rebuilds, restarts, and republishes fire's two keys onto that host's entry, leaving the arena's keys on the same machine untouched. It health-checks by speaking the protocol (Hello must be answered with Welcome) before it publishes anything, so a failed deploy leaves the previous address in place rather than pointing players at a server that never came up — and it refuses to restart over people who are mid-race unless `EMBER_FORCE` is set.

### Why no link here ever carries a tunnel domain

A quick tunnel mints a **new** random `*.trycloudflare.com` hostname every time it restarts, so any such URL written into this README, or into a page, would be wrong by the next restart.

Nothing needs one. The hub and each game page fetch `server.json` cache-busted at load and pick a live host out of it, so the only link anyone needs is the stable Pages URL above. `server.json` is the single source of truth, and `deploy/publish-host.sh` is the only thing that writes its host list — each deploy upserts its own host's entry and merges only the keys for the game it deployed, so two machines, or two games on one machine, cannot clobber each other. It refuses to overwrite a book it could not parse, so one bad byte can never take every other host's address with it.

What that does *not* survive is an unattended restart: the servers come back at an address the book does not yet name. Many hosts already blunt this — the other machines keep answering while one is lost — and `deploy/watchdog.sh` closes the rest of the gap by probing each host's published addresses and redeploying just that host when one stops answering. See `deploy/README-watchdog.md`.

## Four Kings

**Play: <https://endersgamesdev.github.io/EmberEngine/>** and pick Four Kings: a four-corner chess variant, 2 to 4 players on a 10x10 board, one action per 15-second turn, last king standing wins. Chess plus two new legend pieces, the Joker (a teleporting sniper with a single capture tile) and the Hero (dormant until it trades places with one of your pawns and wakes as a rook-plus-knight), with pawns that march forward or left so a corner formation is never a wall. Click a piece, click a highlighted target; `Esc` clears. Before the game starts, swap your legend and epic cards within their class.

The rules of record are `docs/kings-design.md`, sections 1 to 3 (`docs/kings-rules.md` is the pointer); the server validator and the client are both written against that document, and no rule lives anywhere else. Rules and wire protocol are `crates/kings-core`, shared between the client (for move highlights only, never prediction) and the authoritative `kings-server`; the game has no simulation to step, so the server's hub only validates moves and runs the turn clock.

Native: `cargo run -p kings --bin kings-app` for hotseat at one keyboard, or `kings-app online wss://… create|join LOBBY [PASSWORD|-] [HANDLE]` against a server.

Kings carries its **own** `PROTO_VERSION`, independent of `pong-core`'s and `fire-core`'s. That is deliberate: bumping one game's protocol must never gate another game's join. `server.json` records it as `kings_proto` next to the address in `kings_ws`, the `<id>_ws` / `<id>_proto` convention.

Online infrastructure: `kings-server` on port 7782 (7780 is the arena, 7781 fire) behind its own Cloudflare quick tunnel, published under its own keys, so redeploying one game can never knock another offline. Unlike the other two it is hosted on the developer's Windows PC, inside the `claude-sdk` WSL distro, because that is where the toolchain, `cloudflared` and `python3` already live: `bash deploy/deploy-kings-online.sh` (from Git Bash) builds, restarts, and republishes, probing the protocol on loopback and then through the public URL before it publishes anything, exactly as fire's deploy does; `down` stops the pair and `status` reports what is running. There is no systemd in that distro, so nothing restarts the server after a reboot, a sleep or a `wsl --shutdown`; run the script again. The reason no link in this README carries a tunnel domain is explained in the Fire Racer section above, and it applies unchanged here.

## Run

```
cargo run -p game                 # multiplayer arena (auto-connects to specht)
cargo run -p arena --bin arena-app  # 3D pong, 2 players at one keyboard
cargo run -p kings --bin kings-app  # Four Kings hotseat, 4 seats at one keyboard
```
