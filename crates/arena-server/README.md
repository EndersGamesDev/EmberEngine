# Arena server

`arena-server` is the authoritative WebSocket host for Killshot matchmaking and live matches: one hub thread owns connections, lobbies, and 60 Hz `arena-core` simulations while per-connection threads translate JSON frames into bounded events.

The `arena-server` binary reads its deployment configuration and calls the library's `run` entry point; native arena clients, browser clients, and the headless example bot consume the service through the shared protocol.

TLS terminates in front of this process, but its public listener still treats every message as untrusted and enforces connection, queue, frame-size, string, and per-tick work limits.

Shared protocol and simulation rules live in [`../../CLAUDE.md`](../../CLAUDE.md), while [`../../docs/state-model.md`](../../docs/state-model.md) explains the authority, codec, and client-reconciliation boundaries this server participates in.
