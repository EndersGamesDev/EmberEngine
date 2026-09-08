# Arena hosted contract v4

This directory is the archived Arena version slot `arena/4`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-arena-v4` with fixture suite `arena-v4-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) says source commit `9798cc58` is recorded and records stamp — for Arena v4; the slot freezes the first-person contract with mouse look, sprinting and crouching.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).

