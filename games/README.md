# Frozen game contracts

This directory holds immutable hosted simulation and protocol crates for published game versions, with `hosted.toml` registering the versions available to the game-neutral server.

`ember-server` depends on these contracts to keep older clients playable without making current crates imitate historical behavior.

The one-server boundary is explained in [`docs/one-server-evergreen.md`](../docs/one-server-evergreen.md); release compatibility, series-prefixed tags and frozen-version rules live in [`docs/versioning.md`](../docs/versioning.md).
