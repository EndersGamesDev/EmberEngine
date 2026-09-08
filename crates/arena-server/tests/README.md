# Arena server integration tests

`ws_e2e.rs` starts the actual server on loopback and drives real WebSocket peers, pinning behavior that unit calls cannot cover: password admission, join and leave flow, lobby metadata, protocol gating, input ordering, state acknowledgements, and event broadcast timing.

The suite also fixes gameplay-visible wire contracts for inventory and reloads, shields and health, airborne state, maps and modes, Harbor team spawns, and round-ending shot events.

These tests consume both `arena-server` and `arena-core`; failures should be treated as a disagreement between transport, authority, and shared codec semantics rather than as a snapshot update opportunity.

The public-listener threat model is stated in `src/lib.rs`, and the protocol compatibility rules the cases defend live in [`../../../CLAUDE.md`](../../../CLAUDE.md) and [`../../../docs/state-model.md`](../../../docs/state-model.md).
