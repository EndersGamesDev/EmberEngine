# Arena v5 source

This directory holds the frozen Arena v5 implementation: `adapter.rs`, `proto.rs` and `shooter.rs` define the adapter, wire contract and deterministic gameplay exported by `lib.rs`.

Package `ember-game-arena-v5` and the evergreen server depend on these modules; never edit them by hand, and follow the [version-slot rules](../../../../docs/one-server-evergreen.md) and the [parent contract record](../README.md) for changes.

