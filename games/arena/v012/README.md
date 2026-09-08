# Arena hosted contract v12

This directory is the current Arena version slot `arena/12`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v12` with fixture suite `arena-v12-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says source commit `12193548` is recorded and records stamp r439 for Arena v12; the slot freezes the melee-through-shields and headshot contract.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).
