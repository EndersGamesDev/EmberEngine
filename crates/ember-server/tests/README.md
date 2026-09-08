# Ember host integration test

`host_e2e.rs` starts a real loopback `Host`, connects through WebSocket, and exercises the feature-gated fixture version from canonical hello through lobby admission, inner frame exchange, and graceful drain.

The test pins the seam between `ember-net` framing, `ember-server` routing, and an `ember-legacy` game session, including the requirement that drain refuses new admission while an established session completes.

This suite is enabled by the crate's `demo` feature so the production registry remains the checked-in hosted manifest rather than silently acquiring a test-only game.

The lifecycle it covers is the executable host slice of [`../../../docs/one-server-evergreen.md`](../../../docs/one-server-evergreen.md).
