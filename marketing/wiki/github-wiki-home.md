# GitHub wiki — Home page (ready to paste)

The wiki is enabled on the repo (`has_wiki: true`), but at session end the wiki git repo had not materialized yet, so this seed has not been pushed. When `https://github.com/EndersGamesDev/EmberEngine.wiki.git` clones, push this as `Home.md` — or paste it through the web UI.

---

**Ember** is a game engine written in Rust. It renders with wgpu, runs natively on desktop, and compiles its games to WebAssembly for the browser. It is developed by Ender ([EndersGamesDev](https://github.com/EndersGamesDev)) and is the engine behind the games on [the hub](https://endersgamesdev.github.io/EmberEngine/).

## Design

The engine is a one-way layering: game → scene/simulation → renderer → platform. The renderer owns the GPU (wgpu); nothing above it touches it. The simulation is deterministic: a fixed 60 Hz timestep, a seeded RNG (two world-generation LCGs), a fixed update order. That buys replays, save states, and rollback netcode; the client predicts its own movement and reconciles against 30 Hz server state, and projectiles are stepped server-side only.

## Rendering

wgpu on a documented device floor: WebGL2 + EXT_color_buffer_float for the web (no WebGPU required), Vulkan/DX12/Metal natively. The scene pass is deliberately small: one base-colour texture per mesh multiplied by a per-instance colour, 8-bit formats only, no blending, no backface culling. The floor and the reason for it: [docs/minimum-requirements.md](https://github.com/EndersGamesDev/EmberEngine/blob/main/docs/minimum-requirements.md).

## Multiplayer

There is no central game server. Any machine runs a host (game server + tunnel) and publishes an entry in an address book (`server.json` on the static site) naming its build, its commit, and the protocol version each of its servers speaks. A page asks every host in one round trip and picks the newest build that speaks its protocol, the emptiest, the nearest; old frozen versions stay playable on old hosts. The full model: [docs/hosts.md](https://github.com/EndersGamesDev/EmberEngine/blob/main/docs/hosts.md).

## Games

| Game | What it is |
|---|---|
| Arena Shooter | an 8-player first-person shooter, three modes, seven guns; the live build is v20 |
| Fire Racer | drift racing on a 920 m castle circuit, local + online |
| Four Kings | four-corner chess, 2–4 players, 15-second turns |
| what is this? | a browser + hardware diagnostic |

## Architecture and contributing

The workspace and the layering: [README.md](https://github.com/EndersGamesDev/EmberEngine#workspace-layout). Design documents live in [`docs/`](https://github.com/EndersGamesDev/EmberEngine/tree/main/docs) (state model, presenter architecture, the asset pipeline, the GPU heap). Contributing: [CONTRIBUTING.md](https://github.com/EndersGamesDev/EmberEngine/blob/main/CONTRIBUTING.md); the known gaps are one line each in [the backlog](https://github.com/EndersGamesDev/EmberEngine/blob/main/docs/plans/backlog.md).

## See also

- [The repository](https://github.com/EndersGamesDev/EmberEngine)
- [The games hub](https://endersgamesdev.github.io/EmberEngine/)
- [The developer page](https://endersgamesdev.github.io/EmberEngine/engine.html)
