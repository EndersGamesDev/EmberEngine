# Julibrot presentation contracts

`api_contract.rs` pins callable application-facing signatures, and `allocation_contract.rs` counts allocations across the hot planning and event paths that must stay bounded.

`reprojection_oracle.rs` compares retained-image decisions, homographies, relief redraw, palettes, and sample placement against CPU truth over named views.

`steep_row.rs` inspects the exact binary32 payloads for a fully rotated saved row, while `steep_frame.rs` rasterizes that row natively so conclusions about visible streaks, depth, and background come from a picture-level census.

These suites protect the public, allocation, numerical, and visual-oracle surfaces listed in [`../../../../../docs/julibrot/present.md`](../../../../../docs/julibrot/present.md); GPU execution remains a separately reported gate.
