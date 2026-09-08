# Legacy capability source

`lib.rs` contains the crate's entire public contract: version keys and limits, monotonic scheduling values, deterministic keyed randomness, peer and session identities, transport and asset handles, codec and ingress traits, session updates, factories, and hosted-manifest parsing.

`FrozenKeyedRandom` supplies the reference hash-based random stream whose construction is frozen, while object-safe capability traits let current adapters change implementation without handing their types to versioned game code.

The in-module tests pin manifest parsing and semantic validation, duplicate and latest-version rejection, selector syntax, and keyed randomness repeatability and domain separation.

Signature ownership and the exact randomness construction are specified in [`../../../docs/one-server-evergreen.md`](../../../docs/one-server-evergreen.md) and its implementation plan, [`../../../docs/plans/one-server-interface.md`](../../../docs/plans/one-server-interface.md).
