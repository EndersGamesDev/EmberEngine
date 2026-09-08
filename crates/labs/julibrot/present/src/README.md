# Julibrot presentation source

`lib.rs` re-exports frame and event contracts, presentation facts and ledgers, `Presenter`, readback routes, warp plans, mesh and homography helpers, palettes, uniforms, shaders, and shared tile identities.

`contract.rs`, `state.rs`, and `fence.rs` model ownership and completion; `planner.rs`, `lattice.rs`, and `homography.rs` decide retained-image coverage; `mesh.rs` reconstructs relief geometry; and `palette.rs` maps escape records to visible values.

`uniform.rs`, `shader.rs`, `shade_shader.rs`, and `warp_shader.rs` pin CPU/GPU layouts and generated WGSL, while `gpu.rs` is the exclusive device lowering.

Unit tests cover ledgers, fences, homographies, geometry, palette sentinels, warp ceilings, uniform layouts, shader validation, and tile transitions under the interface record in [`../../../../../docs/julibrot/present.md`](../../../../../docs/julibrot/present.md).
