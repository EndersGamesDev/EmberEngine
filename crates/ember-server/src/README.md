# Ember host source

`lib.rs` exposes registry construction and the `Host`, `HostConfig`, `DrainHandle`, and occupancy API; `main.rs` builds the product registry and starts the only deployed host binary.

`product.rs` joins compiled registrations to `games/hosted.toml`, `registry.rs` validates and selects that closed set, and `runtime.rs` owns the single-writer hub, lobby lifecycle, admission, stepping, budgets, outbound routing, timeouts, and drain.

`connection.rs` confines WebSocket handshake and per-peer I/O, `capabilities.rs` implements the neutral `ember-legacy` services, and `digest.rs` preserves length-prefixed fingerprints. The feature-gated `fixture.rs` supplies a minimal version for host-level demonstrations.

Module tests pin manifest completeness, exact selection and refusal values, legacy routing, capability fingerprints, admission state, rate charging, lobby identity, and drain behavior under the contracts in [`../../../docs/one-server-evergreen.md`](../../../docs/one-server-evergreen.md).
