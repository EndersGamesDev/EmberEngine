# Live Fire Racer page

This directory holds the live v2 page for the castle circuit, drifting, boost and online lobbies.

[`web/games.json`](../../../games.json) sends Fire Racer traffic here, and [`deploy/deploy-pages.sh`](../../../../deploy/deploy-pages.sh) combines the page with only the generated Fire wasm bundle while protecting a newer published protocol.

Release, protocol and series-prefixed three-grade tag rules live in [`docs/versioning.md`](../../../../docs/versioning.md), with shipped changes recorded in [`CHANGELOG.md`](../../../../CHANGELOG.md).
