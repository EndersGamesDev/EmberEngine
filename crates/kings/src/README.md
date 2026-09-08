# Four Kings client source

`lib.rs` publishes local and online startup plus the wasm API, and `main.rs` selects hotseat or a native `ws` or `wss` session. `game.rs` shares mesh registration, board coordinates, seat cameras, scene construction, HUD, and page-command queues.

`hotseat.rs` runs four local seats and a client-fed turn clock; `net.rs` keeps hello and ping independent of frame updates; `online.rs` applies authoritative messages without prediction; and `online_game.rs` joins socket state, selection, page commands, and rendering.

`ui.rs` is the pure selection and formation state machine, while `meshes.rs` generates lit, texture-free triangle meshes for tiles, targets, rings, and every piece silhouette.

Module tests pin selection legality and pending state, board-to-scene mapping, mesh normals, hotseat clocks, background-safe keepalives, authoritative clock resync, online configuration, and rendering behavior against [`../../../docs/kings-design.md`](../../../docs/kings-design.md).
