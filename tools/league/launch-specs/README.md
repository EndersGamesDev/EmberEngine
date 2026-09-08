# League launch-media specifications

This folder preserves reproducible generation requests for League's Crystalforge hero image, three story chapters, four champion portraits, establishing motion, and two sound effects.

`launch-assets.py` consumes one specification per fleet job and writes resumable records under `target`; `launch-provenance.py`, the landing-page story, and trailer editor select reviewed derivatives without treating every generated result as a release asset.

The prompts, negative prompts, fixed seeds, frame or audio parameters, and model-facing dimensions are part of the source provenance; selection and publication rules live in `docs/plans/league-launch-story.md` and `docs/asset-pipeline.md`.
