# Live browser diagnostic

This directory holds the live v1 page for the nine-stage browser and hardware diagnostic, including the Julibrot fast-slide probe and optional report submission.

[`web/games.json`](../../../games.json) sends diagnostic traffic here, and [`deploy/deploy-pages.sh`](../../../../deploy/deploy-pages.sh) combines the page with only the generated `what_is_this` wasm bundle.

Keep probes resilient when a feature is unavailable, keep upload a separate choice, and apply the release rules in [`docs/versioning.md`](../../../../docs/versioning.md).
