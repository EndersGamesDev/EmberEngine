# Ember legacy capabilities

`ember-legacy` is the narrow, moving in-tree capability surface between current infrastructure and frozen hosted game versions; it is not a published API or a promise of semantic compatibility to external callers.

Hosted version crates consume neutral clocks, keyed randomness, transport, assets, codec, ingress, session, and factory contracts from here instead of depending on sockets, server internals, rendering, operating-system services, or another game version.

The current `ember-server` implements the host side of those capabilities, and any surface change must update every in-tree hosted consumer atomically without changing its frozen gameplay or wire meaning.

The dependency boundary is summarized in [`../../docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md), and the hosting, copy-on-write, and update-duty rules are defined in [`../../docs/one-server-evergreen.md`](../../docs/one-server-evergreen.md).
