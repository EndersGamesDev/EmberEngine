# Ultimate League WebSocket probe

`wsprobe.rs` is the deploy-time full-path client for a fresh duel or squad lobby: it greets, selects a champion, starts the match, observes increasing ticks, ranks a skill, buys an item, moves, exchanges ping and pong, then leaves.

The probe supports `ws` and `wss` and can require the welcome's build commit, so it detects both a stalled hub and an older binary still occupying the deployment port.

It exits on any refusal, incompatible identity, or missing progress rather than reducing health to a successful socket upgrade.

The invocation and server behavior are documented in [`../README.md`](../README.md), with the underlying game sequence in [`../../../docs/plans/ultimate-league-v4.md`](../../../docs/plans/ultimate-league-v4.md).
