# Game pages

This directory groups browser entry pages by game and published version; generated wasm packages are assembled onto the published branch rather than tracked beside these sources.

The root launcher depends on [`../games.json`](../games.json) to map catalogue entries to these paths, and [`deploy/deploy-pages.sh`](../../deploy/deploy-pages.sh) copies only declared live sources while preserving frozen releases.

Game-page compatibility, archive and tag rules live in [`docs/versioning.md`](../../docs/versioning.md), while each game directory identifies its page family.
