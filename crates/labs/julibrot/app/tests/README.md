# Julibrot application contracts

`final_conformance.rs` drives the assembled native slices through the published final-view cases, checking that math, kernels, ownership, and presentation agree on the result rather than merely compiling together.

`page_contract.rs` pins the wasm-facing names, facts, saved-view fields, controls, and error/status vocabulary expected by the surrounding Julibrot page.

Together these integration tests protect the application boundary above the dense unit suites in each sibling crate; changes should update them only when the documented page or final-conformance contract intentionally moves.

The expected integration surface and gate inventory are maintained in [`../../../../../docs/julibrot/app.md`](../../../../../docs/julibrot/app.md).
