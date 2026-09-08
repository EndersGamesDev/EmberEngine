# Julibrot application

`ember-julibrot-app` is the integration and page-contract crate for the Julibrot laboratory, combining numeric truth, GPU kernels, reference-orbit workers, heap resources, and presentation into one cooperative browser viewer.

The cdylib exports the wasm-facing application while the rlib supports headless contract tests; saved views use exact JSON float round trips so reloading a coordinate cannot silently select a neighboring row.

This crate owns orchestration and user-request state, not the numerical, compute, ownership, or rendering rules delegated to its four sibling Julibrot crates.

Its runtime boundaries, refresh order, page API, and evidence obligations are specified in [`../../../../docs/julibrot/app.md`](../../../../docs/julibrot/app.md), with the overall experiment in [`../../../../docs/julibrot-lab.md`](../../../../docs/julibrot-lab.md).
