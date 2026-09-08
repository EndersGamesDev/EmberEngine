# Fire Racer hosted contract v1

This directory is the current Fire Racer version slot `fire/1`, registered by [`games/hosted.toml`](../../hosted.toml) as package `ember-game-fire-v1` with fixture suite `fire-v1-hosted-contract`.

The [release ledger](../../../CHANGELOG.md) records source commit `ebe380e0` and stamp `r109+dirty` for Fire Racer v1; the slot freezes protocol 1 racing simulation, wire messages and hosted-session adaptation from the first published build.

This directory freezes a hosted contract, not a browser page or wasm bundle; [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) writes the corresponding published page and bundle without rewriting this source contract.

Never edit this frozen directory by hand: incompatible behavior gets a new version slot and manifest entry under the [hosted-version rules](../../../docs/one-server-evergreen.md), with release identity governed by [versioning](../../../docs/versioning.md).
