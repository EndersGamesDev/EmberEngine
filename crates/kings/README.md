# Four Kings client

This crate is the native and wasm client for Four Kings, supporting four-seat hotseat play and authoritative online tables through `kings-server`.

`kings-core` supplies board rules, legal targets, clocks, and protocol types; this client uses them for local hotseat actions and online highlights, but it never predicts an online move before the server echoes a full state.

The `kings-app` binary and wasm entry points share scene construction, procedural piece meshes, selection logic, HUD state, and WebSocket behavior, while the surrounding web page owns text-heavy controls and status.

The game rules and page/API contracts are recorded in [`../../docs/kings-design.md`](../../docs/kings-design.md), under the engine layering constraints in [`../../CLAUDE.md`](../../CLAUDE.md).
