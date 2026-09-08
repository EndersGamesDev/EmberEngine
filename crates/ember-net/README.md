# Ember outer networking

`ember-net` defines the game-neutral JSON bootstrap and lobby protocol used before a WebSocket connection enters a hosted game, plus shared sanitization and transient-read helpers.

`ember-server` and client networking adapters consume this crate to agree on hello, lobby discovery, version selection, admission, and refusal messages; after `Joined`, exact game-version frames pass through without translation.

Keeping this protocol separate from every inner game codec lets one evergreen host select frozen Arena, Fire, and future contracts without teaching those games about the current server.

The outer-versus-inner boundary and hosting guarantees are defined in [`../../docs/one-server-evergreen.md`](../../docs/one-server-evergreen.md), with the repository dependency direction summarized in [`../../docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md).
