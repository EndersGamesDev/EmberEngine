# Julibrot frame-loop tests

`tests.rs` is the large native mirror of the browser refresh loop, using fake worker and presenter endpoints to exercise the same scheduling transactions without a DOM or GPU.

Its scenarios pin HOT and MAIN drain order, worker service and surface resolution, accepted and refused warps, progressive ladders, stale generations, reference renewal, backdrop alternation, heap fallback, manual controls, deadlines, and convergence back to idle.

Frozen trace builders record complete turns as plain data, making browser/native ordering and named interaction histories reviewable independently of asynchronous callbacks.

These tests enforce the orchestration contract in [`../../../../../../../docs/julibrot/app.md`](../../../../../../../docs/julibrot/app.md), while the nested `browser/` directory supplies the wasm lowering they model.
