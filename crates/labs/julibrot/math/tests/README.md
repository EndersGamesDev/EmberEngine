# Julibrot mathematical contracts

`contracts.rs` exercises the public math API as a downstream consumer, pinning cross-module invariants that should remain true even when internal representations are reorganized.

The suite covers canonical views and planes, screen round trips, scale and centre splits, deep precision choices, reference-orbit and perturbation agreement, navigation anchors, morph endpoints, scene footprints, and reprojection validity.

Keeping these checks outside `src` ensures the documented surface is sufficient and prevents tests from relying on private helpers unavailable to kernels, workers, or presentation.

The public truth boundary they defend is specified in [`../../../../../docs/julibrot/math.md`](../../../../../docs/julibrot/math.md).
