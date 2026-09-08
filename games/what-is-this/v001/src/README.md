# what is this? v1 source

This directory holds the frozen what is this? v1 implementation in `lib.rs`: protocol limits, diagnostic-report types, validation, receipt derivation and the hosted session receiver live together because the contract is one bounded report exchange.

Package `ember-game-what-is-this-v1` and the evergreen server depend on this module; never edit it by hand, and follow the [version-slot rules](../../../../docs/one-server-evergreen.md) and [parent contract record](../README.md) for changes.

