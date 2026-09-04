# Ember (game engine) — article draft

## Read this first (why this file is not on Wikipedia)

The Wikipedia policies this project does not meet yet:

- **WP:NOTABILITY** — a new, single-developer project with no coverage in independent, reliable sources is not notable. The repository's own documents do not count: they are the subject's own words, not sources about the subject.
- **WP:NOTADVERTISING** — an article written by the developer to promote the engine is treated as an attack article in the deletion process and deleted, not improved.
- **WP:COI** — the developer may edit, but must disclose the conflict on the talk page and write neutrally.

So this draft is a template. Publish it only when independent coverage exists (game press, a podcast covering it as a subject, independent reporting on a conference talk), and then only with that coverage as the references, from an account that is not the project's, with the COI disclosed. Until then the project's encyclopedic home is the GitHub wiki, which this draft seeds.

## Draft (neutral tone, for later)

**Ember** is an open-source video game engine written in Rust. It renders with wgpu, targets native desktop platforms and the web via WebAssembly, and is used to build and host several online browser games from a single codebase. The project is developed by Ender (GitHub: EndersGamesDev) and first appeared on GitHub in August 2026.

### Design

Ember is structured as a one-way layering: game, scene/simulation, renderer, platform. The renderer layer owns the wgpu GPU handle, and no layer above it touches the GPU directly. The simulation runs on a fixed 60 Hz timestep with a seeded random number generator and a fixed update order, which makes the simulation deterministic; the engine uses this for replays, save states, and rollback network code. In the multiplayer implementation, clients predict their own movement against the shared simulation and reconcile against server state broadcast at 30 Hz; projectiles are stepped only on the server.

### Web target

The web build compiles the engine and its games to WebAssembly. Assets are embedded in the binary at compile time, so a game page makes no runtime asset requests. The rendering floor is WebGL2 with the EXT_color_buffer_float extension; WebGPU is not required. Devices that do not expose the floor are refused with a message naming the missing capability.

### Games

Games built on the engine and hosted by the project include Arena Shooter, a first-person shooter for up to eight players in three modes; Fire Racer, a drift racing game on a castle circuit; Four Kings, a four-corner chess variant for two to four players; and what is this?, a browser and hardware diagnostic. (As of September 2026.)

### Development

The project is developed primarily by one developer with AI assistance. Its repository documents a protocol-versioning scheme in which old game versions remain playable on old server hosts, and a multi-host addressing scheme in which any machine can host a game and client pages discover hosts through a published address book. (The repository's backlog and design documents are primary sources; replace them with independent coverage before publishing.)

### References

1. <https://github.com/EndersGamesDev/EmberEngine> — the repository (primary source).
2. <https://endersgamesdev.github.io/EmberEngine/> — the games hub (primary source).
3. [Independent coverage — none as of the draft date.]
