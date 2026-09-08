# Arena server examples

`wsbot.rs` is a headless arena participant for smoke-testing both plain and TLS WebSocket endpoints: it can create or join a lobby, circle and fire, exercise optional shield, jump, crouch, ADS, and loot-bonk inputs, and report the state stream it observed.

Operators and end-to-end diagnostics consume this example when a real client window would obscure protocol behavior; its deliberately pulsed jump and bounded loot-target fallback make input semantics visible rather than merely keeping a socket open.

The bot reconstructs levels from `GameJoined` through `arena-core`, so its map and mode options must follow the same protocol-version rules as shipping clients.

The command-line contract is documented at the top of `wsbot.rs`; broader shared-state constraints live in [`../../../CLAUDE.md`](../../../CLAUDE.md) and [`../../../docs/state-model.md`](../../../docs/state-model.md).
