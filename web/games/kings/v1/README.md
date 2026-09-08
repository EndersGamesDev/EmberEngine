# Live Four Kings page

This directory holds the live v1 page for the 10-by-10 four-corner game, within-class card swaps and 15-second online turns.

[`web/games.json`](../../../games.json) sends Four Kings traffic here, and [`deploy/deploy-pages.sh`](../../../../deploy/deploy-pages.sh) combines the page with only the generated Kings wasm bundle.

Board behavior is governed by [`docs/kings-design.md`](../../../../docs/kings-design.md), and release compatibility and tags are governed by [`docs/versioning.md`](../../../../docs/versioning.md).
