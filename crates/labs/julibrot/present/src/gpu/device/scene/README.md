# Julibrot scene submission

`submit.rs` contains the encoding half of the relief scene pass, separated from `scene.rs` resource construction so draw submission can be reviewed against an already validated grid and target set.

It binds heap records, scene uniforms, indices, optional coarse backdrop, and main-grid ownership in the required order before producing the offscreen scene texture later consumed by warp or direct presentation.

The parent module retains texture, depth, pipeline, extent, grid, and draw-order validation; this leaf does not allocate a competing scene owner or define another presentation policy.

The scene-pass and retained-frame contract lives in [`../../../../../../../../docs/julibrot/present.md`](../../../../../../../../docs/julibrot/present.md).
