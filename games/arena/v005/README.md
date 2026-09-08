# Arena hosted contract v5

This directory is the archived Arena version slot `arena/5`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v5` with fixture suite `arena-v5-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says source commit `06201db3` is recorded and records stamp — for Arena v5; the slot freezes the client-prediction and authoritative-reconciliation contract.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).
