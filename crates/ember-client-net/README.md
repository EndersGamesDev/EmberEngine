# Ember client networking

`ember-client-net` provides game-neutral client networking machinery above `ember-net`: WebSocket lifecycle, handshake progress, wire frames, sequence histories, reconciliation, and remote snapshot buffering.

Games consume its traits and state machines while retaining ownership of payload schemas, simulation meaning, and presentation policy, which keeps the transport reusable without turning it into a second game model.

Native builds use tungstenite with rustls and wasm builds use the browser WebSocket API; both expose the same connection diagnostics and bounded bookkeeping to their callers.

The intended separation between wire, local, and rendered state is described in [`../../docs/state-model.md`](../../docs/state-model.md), and repository layering remains governed by [`../../CLAUDE.md`](../../CLAUDE.md).
