# Fire Racer server

`fire-server` is the authoritative protocol-2 race host: connection threads own WebSockets and bounded outbound queues, while one hub thread owns every lobby and advances `fire-core::Race` at 60 Hz.

Fire clients send control intents rather than positions, and the server broadcasts authoritative car, lap, roster, item-box, projectile, hazard, and host state used for client reconciliation.

The production binary listens on plain TCP because TLS terminates at the deployment tunnel; its library exposes `ServerConfig`, `build_stamp`, and `run` for deployment and integration tests.

Shared simulation and protocol discipline comes from [`../../CLAUDE.md`](../../CLAUDE.md), and the client/server state relationship is described in [`../../docs/state-model.md`](../../docs/state-model.md).
