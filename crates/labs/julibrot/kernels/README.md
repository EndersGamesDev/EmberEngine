# Julibrot kernels

`ember-julibrot-kernels` owns GPU escape-grid contracts and their CPU mirrors for both shallow direct iteration and deep reference-orbit perturbation.

The application consumes its refinement plans and heap-backed `JulibrotKernels`; presentation consumes the same grid records and levels, while conformance code compares visible GPU-compatible outputs with CPU truth.

Shader bodies are lowered through `ember-lab-heap` so WebGL2 fragment compute, output spans, dispatch receipts, and typed refusals share one allocation and execution model.

Record layouts, numerical tolerances, shader dialect, refinement policy, and conformance cases are defined in [`../../../../docs/julibrot/kernels.md`](../../../../docs/julibrot/kernels.md).
