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
  arena/          Arena client — the v0 pong classic + the online shooter;
                  native bin (arena-app) + wasm lib; props.rs draws the v13 map
  arena-core/     the shared deterministic 60 Hz sims — the arena shooter
                  (shooter.rs, with the authored Level) and the v0 pong classic
                  (sim.rs) — plus their JSON protocol (proto.rs)
  arena-server/   public matchmaking + match server (WebSocket, lobbies with
                  optional passwords, authoritative sim per match)
  fire/           Fire Racer client: local + online; native bin + wasm lib
  fire-core/      shared deterministic racing sim (track, car, laps) + its own
                  protocol, versioned independently of arena-core's
  fire-server/    Fire Racer matchmaking + race server (WebSocket)
  kings/ kings-core/ kings-server/   Four Kings: client, rules + protocol, server
  what-is-this/   the browser + hardware diagnostic (local, optional upload)
  ember-editor/   the native level builder: fly, place, move/rotate/scale,
                  spawn points; its document is arena-core's Level
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

- **Local (Windows)**: primary dev machine.
- **Remote servers** (via wireproxy tunnels + SSH, see `~/.ssh/config`):
  - `sokol` — transient rented pod for the live Arena and Fire services and remote repository gates; the SSH alias and its account key belong in `~/.ssh/config`.

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

4. **From this Windows workstation, natively** - no WSL here, and a Rust toolchain on the box:

   ```
   bash deploy/deploy-arena-local.sh          # build, start, prove on loopback and through the tunnel, publish
   bash deploy/deploy-arena-local.sh install   # register a logon task that runs `up`, and run it now
   bash deploy/deploy-arena-local.sh status
   bash deploy/deploy-arena-local.sh down
   ```

   The arena only. It reuses `host-name.sh` and `publish-host.sh` and stamps the build the way `host.sh` does, so its entry in the book is indistinguishable from a Linux host's; `install` registers a Task Scheduler job that runs `up` at every logon, which is what brings it back after a reboot; a sleep still needs a logon or a manual `up`. A server started from an interactive shell dies with that shell's owner, so the task, not a shell, must own the processes. It exists because the day v13 was ready, specht's sshd had stopped answering and no other host carried a toolchain.

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

## Arena Shooter (v13: Trench City, v14: fullscreen, v15: the revolver, v16: one operator, v17: scutum and Murasama, v18: Freight Yard)

**v14 adds a fullscreen mode.** On the native client F11 toggles borderless fullscreen; the engine owns the window, so every ember game gets the key. On the web page, F or the ⛶ button fullscreens the stage (canvas plus crosshair and scoreboard) and the canvas fills the screen at the screen's own aspect; Esc leaves it. No protocol change: v14 pages play on the same protocol-13 hosts as v13, and the v13 page stays playable.

**v15 replaces the box pistol with a heavy revolver in real hands.** The weapon is an artist's Collada model (`assets/heavy-revolver-concept.zip`, twenty parts, 4096² pictures) converted by `tools/v15/` into five textured parts; the hands are a rigged, textured game hand (`assets/rigged-hand-game-model.zip`) whose finger chains are posed around the grip in Blender and baked, then mirrored for the other side. The cylinder advances one chamber per confirmed round, the hammer cocks and falls, the trigger pulls, and the muzzle flash sits on the real muzzle, all driven from the sidecar `crates/arena/assets/viewmodel-rig.json`. Remote players carry the same revolver. No protocol change.

**v16 makes the player and the character one operator.** The v15 revolver in bare hands did not match the character, who visibly carries a rifle and wears gloves, and every remote player showed both weapons: the rifle welded to the spine by `tools/swat_split.py` and the gun the client drew at the hand. `tools/v16/build_operator_viewmodel.py` now builds the viewmodel from the operator's own FBX: its armature carries the artist's grip pose, so the rifle (31 parts joined into one) and the gloved fists (cut from the posed body) come out already holding each other, aligned to the pipeline's frame from the weapon's measured axis. The forearms are tapered tubes in the sleeve's own colour, because the operator's sleeves are a dozen big polygons. The split leaves the rifle out of the body parts, and remote players hold this rifle at the hand. The melee key (E) now plays a butt-strike with the rifle, first person only: the protocol carries no melee state for remote players. No protocol change.

**v17 gives the shield and the melee their own objects.** Q raises a Roman scutum (`assets/scutum.zip`) in place of the box plate, first person and on every remote player whose state says shield, since the protocol already carries that; E draws the Murasama (`assets/metal-gear-rising-murasama.zip`) in the operator's right fist and cuts diagonally, edge leading, while the rifle drops out of frame, first person only because the protocol carries no melee state. `tools/v17/build_viewmodel.py` builds both onto the v16 operator viewmodel, which is now an importable library: the scutum's convex face is found from its curvature and turned to +X so the client's shield centres carry over, the sword's tip is the thin end and its guard the widest slice, and the fist is the operator's own rifle fist rotated so the grip axis it closes on becomes the sword's. No protocol change.

**v18 is the weapons update: seven guns, loot blocks, and the feel pass.** The five weapon archives in `assets/` (AK-47, Vityaz, RPG-7, sniper, and the v15 revolver) become held guns with their own bullets in the shared sim: per-weapon speed, range, a deterministic spread cone with bloom, gravity on the revolver's slug and the rocket, a sniper round that pierces one body, and a rocket that detonates on whatever it touches with a line-of-sight splash that hurts the shooter too. You spawn with the sidearm (today's pistol, infinite reserve); everything better hangs in the air as a `?` block, and the only way to hold it is to jump so your head hits the block from below. The sim's ceiling clamp already stopped a jumping head at a box's bottom; it now reports which box, and the server hands out a random pool gun from a stateless hash of (level seed, tick, player), so there is still no per-tick RNG state. A looted gun carries one magazine and one reserve, nothing refills it, and when it runs dry the sidearm is back; death does the same. Trench City's four weapon pads are gone: four `?` blocks hang at its tunnel mouths instead, one per side, so the block is the one reward on both maps. The second authored map, **Freight Yard** (`Level::freight_yard()`, chosen per lobby by `CreateLobby.map`), is built from the v13 props plus the block: two trains of containers, an open yard with a loading dock under the king block, a close train zone with blocks hung off the crossings, and walled backlots to spawn in. Every gun has its own recoil curve, camera kick, tracer, sound and rumble; the engine learns gamepads (gilrs on native, the Gamepad API on the web) and force feedback (Windows.Gaming.Input natively, `vibrationActuator` through a shim on the page, since the deploy ships no wasm-bindgen snippets), and every intent keeps its key. Full design in `docs/plans/arena-v18-freight-yard.md`. **Protocol 15**, two bumps in one release. The first (14): `PState.weapon` now carries an id 1..7 where a v13 client reads a level 1..3, so a v13 client holding `weapon: 7` would draw the pistol for a rocket launcher and lie on every HUD number; that is the `CLAUDE.md` test, and the frozen v13 to v17 pages go list-only exactly as at the v13 bump. The second (15): Trench City itself changed, pads out and blocks in, under a name that did not, so a first-cut v18 bundle would predict the old map, be paid from a block it cannot see and run across pads that never pay; the map still travels by name, which keeps a NEW map from bumping, but it cannot gate a CHANGED one, and the join gate is exact equality.

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — pick Arena Shooter. First-person drop-in deathmatch for up to 8 players: WASD + mouse look, Space jump, Shift sprint, C crouch, RMB aims down the sights, R reload, Q raises a shield that reflects the round back at the shooter, E is a melee that goes through a raised shield, headshots kill outright, and the guns hang in `?` blocks you bonk from below. A standard gamepad works beside the keyboard, with rumble on Chromium.

This is what "online" means in this repo. The shared deterministic 60 Hz sim is `crates/arena-core/src/shooter.rs` (the paddle sim in `sim.rs` is the v0 pong classic, local only) and the authoritative server is `arena-server`. Clients predict their own movement against the same sim and reconcile against 30 Hz server state; **bullets are stepped server-side only**, which is why hit registration may use `f32` transcendentals at all.

### v13 — Trench City

v13 replaces the seeded box field with **one authored map every peer builds identically**. The server names it (`GameJoined.map`) and `Level::named` resolves it, so the next map is an additive string rather than another protocol bump.

The map is a square in an old European city that is being fought over, four-fold symmetric, three concentric bands so eight players spread out instead of piling into one lane. The cover is real object classes rather than anonymous boxes:

| Class | Height | What it does |
|---|---|---|
| `Container` | 2.6 (and one stacked pair at 5.2) | Closed 40-ft shipping container at real proportions. Hard cover, and its roof is high ground you cannot reach from the floor — the jump apex is 1.76. |
| `Crate` | 1.2 | Wooden supply crate. A climbing step, and a **fire step** against a trench's outer wall: standing on one puts the eye at 2.65, over a 2.5 wall. |
| `Ammo` | 0.55 | Ammunition box. The first step of every climbing chain: floor → ammo → crate → container roof. |
| `Wall` | 2.5 | The trench lines. Two parallel walls with a 3 m corridor between them is a trench. |
| `Roof` | base 2.5, top 2.9 | The first obstacle with a **bottom**. Walk under it, stand on it, and no round passes through it — a roofed trench section is a tunnel. **All four weapon pads are inside the tunnels** (until v18, which replaced them with a `?` block at each tunnel mouth). |
| `Sandbag`, `Rubble`, `Plinth` | 1.1 / 0.7 / 2.2 | Spawn cover, low cover, and the granite plinth under the bronze statue at the centre. |

Around the square the client draws the city — a cathedral, a ring of Haussmann and art-nouveau façades, street lamps, burnt-out cars, and a golden-hour sky. All of it is client-only decor listed in the `Level`, so every client draws the same city; none of it is a collision volume. How the pictures and meshes were generated is `docs/asset-pipeline.md`, Path E, with the runbook checked in under `tools/v13/`.

### What the protocol bump means for old builds

The authored level is a protocol change, so `PROTO_VERSION` in `crates/arena-core/src/proto.rs` goes to **13**. It has to: a v12 client would predict its movement against the seeded boxes while the server resolved it against the trench city, so every wall would be either invisible or imaginary. That is the `CLAUDE.md` test — an old peer that "plays a different game" is a bump, not a `#[serde(default)]`.

**The join gate is exact equality**, so a page and a host must agree on 13 before anyone can create or join. With the address book (above) that is a per-host fact rather than a cliff: the live v13 page picks only hosts whose `arena-server` speaks 13, and until one is published its host chip says so ("*host* is on protocol 12 and this build speaks 13") rather than "offline". The frozen v12 page keeps finding hosts still on 12 for as long as any are in the book; the older frozen pages v7–v11 read only the legacy `ws` key, which follows the live protocol, so they go **list-only** — they still see the lobby list (listing is ungated on purpose) but cannot create or join, and they already say "archived" in the version picker. The hub's lobby browser is unaffected: it sends `proto: 0` against `ListLobbies` and routes a Join only to the page whose `games.json` entry declares that lobby's `proto` with `handover` set.

Deploy a host first, then the pages, in one window: `EMBER_HOST=<ssh name> bash deploy/deploy-pong-online.sh` (or `bash deploy/host.sh up` on the box itself) puts an entry that speaks 13 in the book; `bash deploy/deploy-pages.sh` then ships the staged v13 page, writes `proto: 13` into the book, prints the bump warning it detects there, and re-points the legacy `ws` at a host on 13 — leaving it alone if none is published yet.

### Running it

Online infrastructure: `arena-server` (WebSocket + JSON, `arena-core/proto.rs`) runs on each host bound to loopback, fronted by a **Cloudflare quick tunnel** — a free public `https://….trycloudflare.com` domain that CHANGES every time the tunnel restarts. `EMBER_HOST=<ssh name> bash deploy/deploy-pong-online.sh` rebuilds + restarts server and tunnel, then publishes that host's fresh domain into its own entry in `server.json` on the Pages site, leaving every other host's entry alone; `bash deploy/host.sh up` does the same from the host itself. The server takes its name from `EMBER_HOST_NAME` (or `--name`) and answers every `Hello` with that name, its build stamp and its load. Server log: `~/pong-server.log` under the ssh deploy, `$EMBER_HOME/log/arena-server.log` under `host.sh`; tunnel log: `~/cloudflared.log`. See **Many hosts, one address book** above and `docs/hosts.md`.

Headless check: `cargo run -p arena-server --example wsbot -- <URL> create|join <LOBBY> [PW|-] [HANDLE] [SECS] [MODES]`. `MODES` is a comma-separated list of `shield`, `jump`, `nofire` — without it the bot never raises the shield or jumps, so a green run says nothing about either. Two bots, one plain and one `shield,nofire`, demonstrate a reflect. The bot names the host that welcomed it and the map it was dropped into (`map "trench-city"`), so a green run against a tunnel address also says which machine and which level answered.

Native: `cargo run -p arena --bin arena-app online wss://… create|join LOBBY [PASSWORD|-] [HANDLE]`

Web build: the same commands as the v0 section below (`-p arena --lib`, `arena.wasm`); the shooter and the v0 classic ship in one bundle, and `bash deploy/deploy-pages.sh` builds it.

## Arena v0: the pong classic

**Play: <https://endersgamesdev.github.io/EmberEngine/>** — pick the arena's v0 entry, the pong classic. The first ember game, and today a **local two-player game only**: one keyboard, `A`/`D` against `←`/`→`, first to 7. There is no online pong; the paddle sim in `crates/arena-core/src/sim.rs` runs entirely in the client.

Native: `cargo run -p arena --bin arena-app`. It ships in the same wasm bundle as the arena shooter, built by the commands below.

Web build (needs `wasm32-unknown-unknown` target + `wasm-bindgen-cli`):

```
cargo build --target wasm32-unknown-unknown --release -p arena --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm
```

then publish `web/` to the `gh-pages` branch — or just run `bash deploy/deploy-pages.sh`, which does all of the above.

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

Kings carries its **own** `PROTO_VERSION`, independent of `arena-core`'s and `fire-core`'s. That is deliberate: bumping one game's protocol must never gate another game's join. `server.json` records it as `kings_proto` next to the address in `kings_ws`, the `<id>_ws` / `<id>_proto` convention.

Online infrastructure: `kings-server` on port 7782 (7780 is the arena, 7781 fire) behind its own Cloudflare quick tunnel, published under its own keys, so redeploying one game can never knock another offline. Unlike the other two it is hosted on the developer's Windows PC, inside the `claude-sdk` WSL distro, because that is where the toolchain, `cloudflared` and `python3` already live: `bash deploy/deploy-kings-online.sh` (from Git Bash) builds, restarts, and republishes, probing the protocol on loopback and then through the public URL before it publishes anything, exactly as fire's deploy does; `down` stops the pair and `status` reports what is running. There is no systemd in that distro, so nothing restarts the server after a reboot, a sleep or a `wsl --shutdown`; run the script again. The reason no link in this README carries a tunnel domain is explained in the Fire Racer section above, and it applies unchanged here.

## Run

```
cargo run -p game                 # multiplayer arena (auto-connects to sokol)
cargo run -p arena --bin arena-app  # 3D pong, 2 players at one keyboard
cargo run -p kings --bin kings-app  # Four Kings hotseat, 4 seats at one keyboard
```
