# Julibrot math source

`lib.rs` exposes the crate's numeric interface: `big` owns exact centre encoding and precision selection, `orbit` builds reference sequences, and `perturb` evaluates deep offsets with an error envelope.

`plane`, `screen`, `scale`, and `navigation` connect five-dimensional object and camera controls to screen-relative coordinates; `morph` interpolates those controls at sufficient precision; `drift` measures narrowing error.

`footprint`, `warp`, and `reprojection` decide what a retained scene covers and reconstruct source samples, while `types.rs` centralizes poses, maps, records, precision modes, and typed failures.

In-module tests pin orthonormal construction, exact splitting and round trips, anchor-preserving navigation, orbit and perturbation agreement, precision thresholds, footprint bounds, warp inversion, and retained-value reconstruction according to [`../../../../../docs/julibrot/math.md`](../../../../../docs/julibrot/math.md).
