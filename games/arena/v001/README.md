# Arena hosted contract v1

This directory is the archived Arena version slot `arena/1`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v1` with fixture suite `arena-v1-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says no source commit is recorded and records stamp — for Arena v1; the slot freezes the original two-player paddle simulation, protocol 1 wire codec and hosted-session adapter.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).
