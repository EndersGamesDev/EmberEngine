# r/rust

Title:

```
Ember: a game engine in Rust, built from scratch — wgpu, deterministic lockstep, one wasm bundle (the online FPS inside plays in a tab)
```

Body:

```
I have been building a game engine from scratch in Rust. I am posting because I want a review from people who have shipped wgpu or lockstep before me, not because I think it is good.

What it is, briefly:

- A one-way layering: game -> scene/simulation -> renderer -> platform. The renderer owns wgpu; nothing above it touches the GPU.
- A deterministic 60 Hz simulation: fixed update order, and the only randomness in the whole tree is two world-generation LCGs. No per-tick RNG, because the first per-tick one would have to be seeded and tick-indexed or replays and rollback die.
- The shared sim crate (arena-core) is the same code the client's prediction and the authoritative server run. That invariant is why the design is the way it is, and it is the constraint I keep hitting.
- The web target is wasm on a WebGL2 floor (one extension, no WebGPU). The page is one bundle per game with every asset baked in at compile time, so a static host is the whole deploy.
- Multiplayer is an address book: any machine hosts, the page probes every host in one round trip and picks the newest/emptiest/nearest build that speaks its protocol. The full model is docs/hosts.md.

The thing inside it that is easiest to judge: an 8-player online FPS that runs in a browser tab — https://endersgamesdev.github.io/EmberEngine/ — and the source is https://github.com/EndersGamesDev/EmberEngine.

Where I want eyes:

- crates/arena-core/src/shooter.rs — the sim. Is the segment-intersection bullet stepping sane? The latest pass moved every round to its real muzzle velocity (280-900 m/s), which means a round crosses 15 m in one tick, so the cover and head-band tests had to become exact on the segment.
- crates/ember-engine/src/renderer.rs — the scene pass. It is deliberately small (one base-colour texture per mesh, no blending, no culling). Too small, or the right size for a wasm target?
- The host-picking rule in web/hosts.js, spec'd in docs/hosts.md.

Honest state: one developer, AI-assisted, no release yet. The README is long on purpose; the backlog is one line per known gap. I will answer anything, and I would rather be told what I got wrong than be told it is cool.
```

Notes: r/rust rules — lead with substance, no drive-by links (the links above are the product and the repo, which is allowed), stay in the thread for the first few hours.
