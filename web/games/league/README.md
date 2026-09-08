# UltimateLegue world and versions

This directory contains the public Crystalforge story landing page, its media and the versioned UltimateLegue game pages.

The launcher links the landing page and selects a live version through [`../../games.json`](../../games.json); the landing scripts depend on `story.json` and `media/`, while Pages deployment recursively copies the declared live version's page assets.

The launch-story contract lives in [`docs/plans/league-launch-story.md`](../../../docs/plans/league-launch-story.md), and release, frozen-page and tag rules live in [`docs/versioning.md`](../../../docs/versioning.md).
