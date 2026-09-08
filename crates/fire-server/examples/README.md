# Fire server health probe

`probe.rs` is the deployment-facing health check for `ws` and `wss` endpoints: it performs the WebSocket handshake, sends the Fire `Hello`, and requires a version-matching `Welcome` from the hub event loop.

Deployment consumes this example because an HTTP upgrade alone only proves that a connection thread is accepting sockets; the protocol response proves the simulation-owning hub is alive and speaking the expected version.

The process exits successfully only for that full exchange and prints the refusal reason otherwise, turning a protocol mismatch into deploy-time evidence instead of a player report.

Its transport assumptions match the server boundary documented in `src/lib.rs`, with exact protocol gating governed by [`../../../CLAUDE.md`](../../../CLAUDE.md).
