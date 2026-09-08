# Crystalforge story media

This directory holds the trailer, captions, poster and chapter or champion images named by `manifest.json` and consumed by the UltimateLegue world landing page.

`landing.js` depends on these stable filenames to present the story without loading a game bundle.

Asset intent and provenance are recorded in [`docs/plans/league-launch-story.md`](../../../../docs/plans/league-launch-story.md); update the manifest with any media replacement and preserve the web publication rules in [`deploy/deploy-pages.sh`](../../../../deploy/deploy-pages.sh).
