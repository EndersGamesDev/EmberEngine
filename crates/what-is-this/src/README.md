# What Is This source

`lib.rs` exports diagnostic verdict records, progress calculation, fixed kernel and Julibrot scenario inventories, and the report-to-personality derivation; `wasm_api.rs` is the thin browser orchestration and canonical-submission surface.

`kernels.rs` implements versioned CPU, linear-algebra, quaternion, memory, transcendental, jank, and floating-point probes; `gpu.rs` owns compute-only WebGPU workloads and validation without a surface; `render_bar.rs` separately owns measured surface presentation.

`julibrot.rs` defines bounded named slide scenarios and interprets their stage measurements without conflating unavailable, partial, and complete runs.

Tests pin unique kernel IDs, finite adaptive batches, fused-rounding detection, report-backed badge text, real-work progress, GPU adapter and frame evidence, bounded Julibrot reporting, page disclosure, and submission-ready JSON under [`../../../CLAUDE.md`](../../../CLAUDE.md).
