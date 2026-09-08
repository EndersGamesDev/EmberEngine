# Exact camera source

`lib.rs` exposes the camera contract. `fixed.rs` implements checked signed fixed-point arithmetic, `types.rs` owns the exact records, `scale.rs` derives pixel scale from integer exponents, `basis.rs` rebuilds the floating-point frame, `frame.rs` derives a frame from exact points, and `navigation.rs` and `projection.rs` own the nine camera calculations.

The library uses `core` alone. Tests may use `std` for timing, but platform or renderer types do not enter this folder.

Precision, coordinate, reversibility, and operation-count rules live in [`../../../docs/camera.md`](../../../docs/camera.md); repository-wide source rules live in [`../../../CLAUDE.md`](../../../CLAUDE.md).
