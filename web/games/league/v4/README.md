# Live UltimateLegue page

This directory holds the live v4 page and UI for the Crystalforge environment pass and clearer combat, spell and order feedback.

[`web/games.json`](../../../games.json) sends UltimateLegue traffic here, and [`deploy/deploy-pages.sh`](../../../../deploy/deploy-pages.sh) recursively copies its version-local assets before adding only the generated League wasm bundle.

Release, protocol and series-prefixed three-grade tag rules live in [`docs/versioning.md`](../../../../docs/versioning.md), with shipped changes recorded in [`CHANGELOG.md`](../../../../CHANGELOG.md).
