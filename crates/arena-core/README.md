# Arena core

`arena-core` is the engine-free authority shared by the arena client and `arena-server`: it defines deterministic pong and Killshot simulation, player movement, authored maps, and the JSON WebSocket protocol.

Both peers depend on this crate, so its fixed 60 Hz update order, seeded generation, geometry, weapon rules, and protocol semantics are part of one compatibility boundary rather than client-only implementation details.

The public modules are rooted in `src/lib.rs`; `sim` serves local pong, while `shooter`, `parkour`, `proto`, `freight_yard`, and `harbor` serve the online game and its level contracts.

The shared-simulation and protocol invariants are summarized in [`../../CLAUDE.md`](../../CLAUDE.md), and the relationships between authoritative, transmitted, predicted, and rendered state are traced in [`../../docs/state-model.md`](../../docs/state-model.md).
