# r/gamedev (and r/indiegaming)

Title:

```
I built an engine so I could ship an online FPS to the browser with zero installs — what the constraints forced (playable link inside)
```

Body:

```
The game: an 8-player first-person shooter that runs in a browser tab. Three modes (free for all, team deathmatch, king of the hill), seven guns you loot by jumping so your head hits a ? block from below, a Roman shield that reflects the round back at the shooter, a sword that goes straight through a raised shield, and since the latest pass, real muzzle velocities, tracers, material-matched impacts and gunshots that are delayed and panned by distance.

Play it: https://endersgamesdev.github.io/EmberEngine/

The hub also has a drift racer on a castle circuit, a four-corner chess variant, and a browser/hardware diagnostic — all built with the same engine. The engine (open source, Rust + wgpu) is https://github.com/EndersGamesDev/EmberEngine.

The interesting part for me is what the constraints forced:

- One wasm bundle per game, no runtime fetch. Every texture and mesh is baked in at compile time, so the page is the download. The cost is real — the FPS bundle is about 39 MB — and the tradeoff is that it runs from a static host with no CDN, no backend, no install.
- A WebGL2 floor, no WebGPU. The floor is exactly one extension, and the reason for that exact floor is written down (docs/minimum-requirements.md), including which real devices it excludes (almost none).
- The renderer is deliberately tiny: one base-colour texture per mesh, no blending, no backface culling. A muzzle flash is an opaque shape; a roof is a walkable slab with a tunnel under it. The constraints are the design, and the doc is short enough to read in a coffee break.
- Deterministic 60 Hz lockstep: the client predicts its own movement, the server is authoritative, and bullets are stepped server-side only. Replays, save states and rollback fall out of it for free.
- Multiplayer without a central server: any machine can host a game, and the page finds one by asking every host in the address book in one round trip — newest build that speaks its protocol, then emptiest, then nearest.

If you make games and want a second opinion on any of this — the netcode, the renderer, the host scheme — the repo is set up for it: the design docs are in docs/, the known gaps are one line each in docs/plans/backlog.md, and the issue template asks which layer a change touches before you write it.
```

Notes: attach a 20-second clip or a screenshot (one loot bonk plus one headshot is enough) — r/gamedev scrolls past text-only.
