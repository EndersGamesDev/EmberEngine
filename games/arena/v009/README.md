# Arena hosted contract v9

This directory is the archived Arena version slot `arena/9`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v9` with fixture suite `arena-v9-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says source commit `3481a0fe` is recorded and records stamp — for Arena v9; the slot freezes the articulated-character hosted gameplay contract.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).
