# Arena hosted contract v3

This directory is the archived Arena version slot `arena/3`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v3` with fixture suite `arena-v3-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says source commit `b4a9ad1e` is recorded and records stamp — for Arena v3; the slot freezes the top-down shooter contract with obstacles and line-of-sight combat.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).

