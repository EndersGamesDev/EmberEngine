# Julibrot mathematics

`ember-julibrot-math` is the CPU source of truth for Julibrot coordinates, five-dimensional slices, navigation, arbitrary-precision reference orbits, perturbation, projection, morphing, and retained-frame reprojection.

The app, worker, kernels, and presenter consume its value types and pure functions so precision selection and coordinate meaning are established once before platform or GPU lowering.

Astro-float is confined here; consumers receive typed exact encodings, split binary32 values, maps, poses, orbit records, and explicit errors rather than depending on a bignum implementation.

The coordinate model, precision ledger, reference policy, and numerical verification duties are defined in [`../../../../docs/julibrot/math.md`](../../../../docs/julibrot/math.md) and [`../../../../docs/julibrot/precision-ledger.md`](../../../../docs/julibrot/precision-ledger.md).
