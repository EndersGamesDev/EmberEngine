# Julibrot application source

`lib.rs` exposes the browser `App`, explicit run requests, viewer controls, refinement scheduling, saved views, timing ledgers, measurement policy, surface state, and typed failures.

`state.rs` owns requested HOT and MAIN controls and navigation, `saved.rs` preserves exact coordinates, `measurement.rs` and `timing.rs` record bounded evidence, and `surface.rs` models acquisition and reconfiguration without hiding pending work.

`runtime.rs` and `facts.rs` lower browser device and page reporting, while `frame.rs` is the only route that coordinates workers, heap-backed kernels, presentation fences, retained images, and progressive refinement.

The module ownership and callable page surface are recorded in [`../../../../../docs/julibrot/app.md`](../../../../../docs/julibrot/app.md); numeric or GPU policy stays in the corresponding sibling crate documentation.
