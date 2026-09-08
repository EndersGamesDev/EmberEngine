# Fire server WebSocket tests

`ws_e2e.rs` starts the real server and races real WebSocket clients, establishing that the service is joinable through the public protocol rather than only correct through hub-internal unit calls.

The suite pins two-player racing progress, password refusal and acceptance, explicit version mismatch responses, ungated lobby listing, slow-peer roster delivery, slot reuse after disconnect, and named versus default host welcomes.

These cases consume `fire-core` messages and server entry points together; they protect transport and authority integration without substituting for the Fire client's separate prediction-convergence suite.

Protocol equality and bounded public-input rules follow [`../../../CLAUDE.md`](../../../CLAUDE.md), with state ownership described in [`../../../docs/state-model.md`](../../../docs/state-model.md).
