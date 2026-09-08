# Fire Racer v1 source

This directory holds the frozen Fire Racer v1 implementation: the car, track, castle, AI and simulation modules feed the protocol, legacy bridge and hosted adapter exported by `lib.rs`.

Package `ember-game-fire-v1` and the evergreen server depend on these modules; never edit them by hand, and follow the [version-slot rules](../../../../docs/one-server-evergreen.md) and [parent contract record](../README.md) for changes.
