# Fire Racer client source

`lib.rs` publishes `run_local` and `run_online` and hosts the wasm entry points; `main.rs` is the native launcher. `game.rs` reads controls, registers meshes, maintains the chase camera and HUD, and renders local race state.

`net.rs` adapts Fire protocol 1 to shared client transport, `online.rs` owns lobby state, input history, authoritative reconciliation, and remote dead reckoning, and `online_game.rs` combines that networking with the engine loop.

`trackmesh.rs` stitches road, kerb, wall, ground, and start-line ribbons from the simulation centreline; `meshes.rs` repairs normals and UVs on generated GLBs; `texgen.rs` builds low-frequency seamless base-colour textures without increasing the wasm download.

Module tests pin mesh registration and geometry, generated-prop loading, input latching, frozen codec shape, prediction history and convergence rules, online configuration, and texture validity under the renderer constraints in [`../../../CLAUDE.md`](../../../CLAUDE.md).
