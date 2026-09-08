# Fragment-compute layer source

`lib.rs` publishes `compute`, `geometry`, and `kernels`, and on wasm exports initialization, step selection, display-cadence rendering, and an explicitly fenced probe.

`compute.rs` defines the reusable entry-point-free kernel dialect, capability checks, square dispatch plans, buffer handles, and `ComputeDevice`; `geometry.rs` constructs the exact 1,200-vertex, 3,000-edge prism and CPU/GPU projection mirrors.

`kernels.rs` contains the small vertex, edge, and lattice-edge clients of that dialect, while `demo.rs` alone owns the browser surface and thin comparator renderer.

Tests pin forbidden constructs, uniform layouts, dispatch ceilings, completion, prism invariants, lattice indexing, projection agreement, and browser-report arithmetic against [`../../../../docs/gpu-heap-lattice.md`](../../../../docs/gpu-heap-lattice.md).
