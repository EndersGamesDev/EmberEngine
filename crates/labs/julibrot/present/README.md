# Julibrot presentation

`ember-julibrot-present` turns heap-resident escape grids into shaded relief scenes, retains completed images, reprojects them during navigation, and reports bounded GPU completion and readback events.

The application supplies HOT controls and MAIN scene resources; this crate alone owns their GPU presentation state, scene textures, fences, exposure and glitch census, frame receipts, and warp admission.

Pure planners and CPU mirrors keep homography, mesh placement, palette, coverage, and transition decisions testable without claiming that those checks execute the shipping GPU pipelines.

Resource ownership, event order, shader interfaces, retention, and measurement rules are defined in [`../../../../docs/julibrot/present.md`](../../../../docs/julibrot/present.md), with tiled evolution in [`../../../../docs/julibrot/tiled-reprojection.md`](../../../../docs/julibrot/tiled-reprojection.md).
