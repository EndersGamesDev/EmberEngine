# Browser diagnostic pages

This directory owns the browser entry point for the “what is this?” hardware and browser diagnostic.

The launcher selects its version through [`../../games.json`](../../games.json), and Pages deployment combines the live page with the generated diagnostic wasm bundle.

Optional reporting must remain explicit, while publication and release ownership follow [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) and [`docs/versioning.md`](../../../docs/versioning.md).
