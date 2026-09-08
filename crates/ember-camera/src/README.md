# Exact camera source

`lib.rs` exposes the camera contract, while `fixed.rs` implements the dependency-free signed fixed-point arithmetic used by exact view edits.

The library uses `core` alone. Tests may use `std` for timing, but platform or renderer types do not enter this folder.

Precision, coordinate, reversibility, and operation-count rules live in [`../../../docs/camera.md`](../../../docs/camera.md); repository-wide source rules live in [`../../../CLAUDE.md`](../../../CLAUDE.md).
