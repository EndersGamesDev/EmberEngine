# Julibrot kernel source

`lib.rs` re-exports escape records, shallow and perturbation CPU functions, GPU ownership, kernel construction, conformance verdicts, refinement planning, and tile identities and ledgers.

`shallow.rs` and `perturb.rs` mirror their WGSL kernels; `dialect.rs` assembles those kernels for heap execution; `gpu.rs` owns registered resources and dispatch; and `records.rs` defines the exact uniforms, grids, status values, and reference inputs crossing the GPU boundary.

`refinement.rs` selects level extents and caps, `conformance.rs` evaluates visible replay cards, and `tile_job.rs` pins tiled demand, residency, invalidation, cost, and paired-output identities for the reprojection path.

Module tests validate CPU/shader operation order, ABI offsets, grid and tile bounds, reference exhaustion and glitches, dispatch accounting, transition ownership, and conformance tolerances under [`../../../../../docs/julibrot/kernels.md`](../../../../../docs/julibrot/kernels.md).
