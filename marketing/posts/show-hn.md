# Show HN

Title:

```
Show HN: Ember – a from-scratch Rust+wgpu engine that ships an online FPS to the browser
```

Body:

```
Play it: https://endersgamesdev.github.io/EmberEngine/
Source: https://github.com/EndersGamesDev/EmberEngine

Ember is a game engine I am building from scratch in Rust. The piece I would like your eyes on: an online first-person shooter, deterministic lockstep netcode, running entirely in a browser tab.

What it is:

- One codebase for desktop and web. The sim, the renderer and the games are the same crates; the web build is wasm, one bundle per game, every asset baked in at compile time (include_bytes!), so the page ships with no runtime fetch.
- The sim is a fixed 60 Hz deterministic lockstep: seeded RNG (exactly two world-generation LCGs, nothing per-tick), fixed update order. The client predicts its own movement and reconciles against 30 Hz server state. Bullets are stepped server-side only — that is the whole reason hit registration can use f32 at all.
- The renderer is wgpu on a deliberately low floor: WebGL2 plus one extension, no WebGPU. The floor is documented, and a device without it gets a typed refusal naming the missing capability, not a blank canvas.
- Multiplayer runs on an address book, not a central server. Any machine can host a game behind a tunnel; the page fetches the book, asks every host in one round trip, and picks the newest build that speaks its protocol, the emptiest among those, the nearest among those. Old frozen versions stay playable on old hosts.
- The games: an 8-player FPS in three modes with seven guns (the latest pass puts every round at its real muzzle velocity, with tracers, material-matched impacts and spatial sound), a drift racer on a 920 m castle circuit, a four-corner chess variant with two new legend pieces, and a browser/hardware diagnostic.

What is not there: shadow mapping, physics (rapier is the next sim addition), an offline asset compiler, GPU-driven culling. The roadmap is public in the README, and the backlog is one line per known gap.

I would especially like criticism of the netcode and the host-picking scheme — docs/hosts.md is the full model. What would you not ship?
```

Notes: post 09:00–11:00 US Eastern; the author account answers the first replies; pin the hub link; do not edit the body after posting except to add a link someone asked for.
