# GPU heap source

`lib.rs` re-exports the heap's public contract: generation-checked descriptors and handles, buddy allocation, paged `DataSpan` planning, dialect registration, immutable executor resources, completion polling, surface ownership, and lattice records.

`heap.rs` and `span.rs` own allocation and logical paging; `dialect.rs` validates author kernels; `executor.rs` lowers dispatches to fragment passes; `completion.rs` bounds fence observation; and `selection.rs` protects replacement generations.

`lattice.rs`, `mode_c.rs`, and `kernels.rs` define the equal-work comparison, while `lattice_gpu.rs`, `wasm.rs`, browser error handling, and the spike/page-contract code connect that core to browser evidence. The WGSL files are the generated-compute, fetch, and draw paths those modules validate.

Unit and conformance tests pin allocator reuse, handle generations, span planning, forbidden shader constructs, dispatch layout, output transfers, lattice invariants, page-visible facts, and completion bounds under [`../../../../docs/gpu-heap-lattice.md`](../../../../docs/gpu-heap-lattice.md).
