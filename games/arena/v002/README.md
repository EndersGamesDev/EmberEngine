# Arena hosted contract v2

This directory is the archived Arena version slot `arena/2`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v2` with fixture suite `arena-v2-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says no Arena v2 release entry or source commit is recorded and records stamp — for Arena v2; the slot freezes the second hosted protocol slot and its deterministic shooter contract.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).

