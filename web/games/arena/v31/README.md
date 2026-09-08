# Live Killshot page

This directory holds the live v31 page and settings source for the Breach-12 shotgun release, including its shell-by-shell reload, slot-eight pickups and custom starting loadout.

[`web/games.json`](../../../games.json) sends launcher traffic here, and [`deploy/deploy-pages.sh`](../../../../deploy/deploy-pages.sh) combines these sources with only the generated Arena wasm bundle.

Protocol, frozen-page and `arena-31.1.0`-style tag rules live in [`docs/versioning.md`](../../../../docs/versioning.md); shipped changes are recorded in [`CHANGELOG.md`](../../../../CHANGELOG.md).
