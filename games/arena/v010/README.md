# Arena hosted contract v10

This directory is the archived Arena version slot `arena/10`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v10` with fixture suite `arena-v10-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says source commit `2afec7a7` is recorded and records stamp — for Arena v10; the slot freezes the SWAT-character and jumping contract.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).

