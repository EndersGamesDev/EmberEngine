# what is this? hosted contract v1

This directory is the current what is this? version slot `what-is-this/1`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-what-is-this-v1` with fixture suite `what-is-this-v1-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) records source commit `f28a145f` and stamp `r1469` for what is this? v1; the slot freezes the bounded diagnostic-report schema, receipt protocol and hosted receiver for the nine-stage browser diagnostics build.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).

