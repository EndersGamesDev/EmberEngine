# The short posts — X / Bluesky / LinkedIn

## X (7 posts, one per day-or-immediately)

1. I built a game engine from scratch in Rust. It ships an online FPS to the browser. No install, no CDN, no backend for the player — one wasm bundle per game, every asset baked in at compile time.
2. The netcode is deterministic 60 Hz lockstep: fixed update order, two seeded RNGs, and bullets stepped server-side only. The client predicts its own movement and reconciles against 30 Hz state. Replays and rollback fall out for free.
3. The renderer is wgpu on a WebGL2 floor — one extension, no WebGPU. One base-colour texture per mesh, no blending, no culling. The constraints are the design, and the doc explaining them is a coffee break long.
4. There is no central game server. Any machine can host a game; the page carries an address book and asks every host in one round trip. It picks the newest build that speaks its protocol, the emptiest, the nearest. Old versions stay playable on old hosts.
5. The games: an 8-player FPS in three modes with seven guns (latest pass: real muzzle velocities, tracers, spatial sound), a drift racer, a four-corner chess variant, and a browser/hardware diagnostic.
6. Play it: https://endersgamesdev.github.io/EmberEngine/
7. Source, design docs, and a backlog of one line per known gap: https://github.com/EndersGamesDev/EmberEngine — I would rather be told what I got wrong than be told it is cool.

## Bluesky (3 posts)

1. Posts 1 + 4 from the X thread, joined.
2. Post 6 (the hub link).
3. Post 7 (the repo link + the "told I got it wrong" line).

## LinkedIn (one post)

```
Most game engines ask you to install something. Mine asks for a tab.

Over the past month I have been building Ember, a game engine in Rust (wgpu rendering, deterministic 60 Hz lockstep simulation), and the test of the engine is the game running on it: an 8-player online shooter that runs entirely in a browser tab — one wasm bundle per game, every asset baked in at compile time, no install, no CDN, no backend for the player.

The part I am most interested in getting right is the multiplayer: there is no central game server. Any machine can host a game, and the page finds one by asking every host in a published address book in a single round trip — newest build that speaks its protocol, then emptiest, then nearest. The full design is written down in the repo.

Repo + design docs: https://github.com/EndersGamesDev/EmberEngine
Play it: https://endersgamesdev.github.io/EmberEngine/

I would rather be told what I got wrong than be told it is cool.
```
