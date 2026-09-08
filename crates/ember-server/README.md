# Ember server

`ember-server` is the sole game-neutral host binary: it compiles the explicit hosted game-version registry, admits canonical and retained legacy clients, and runs every lobby through one simulation-owning hub.

It consumes `ember-net` for the outer protocol and `ember-legacy` for neutral version capabilities, while each registered game crate retains its own exact codec and session semantics.

The host supports bounded WebSocket I/O, exact game-and-version selection, manifest validation, graceful drain, and occupancy reporting without linking clients or renderer code.

Its evergreen hosting promise and registry rules live in [`../../docs/one-server-evergreen.md`](../../docs/one-server-evergreen.md), and the dependency flow is summarized in [`../../docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md).
