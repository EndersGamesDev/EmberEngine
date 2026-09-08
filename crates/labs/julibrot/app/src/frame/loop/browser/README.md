# Browser frame-loop lowering

These modules implement the wasm-only side of `BrowserFrameLoop`: `submit.rs` prepares refinement levels and installs scene inputs, while `reference.rs` validates worker arrivals, expands compact orbit records into heap texels, and applies accepted reference generations.

`backdrop.rs` owns the coarse coverage layer, alternating it with main refinement so continuous navigation gains coverage without starving the higher-quality ladder.

`facts.rs` exposes borrowed presentation, kernel, worker, and pending-work facts to the page without polling or mutating the loop; all four modules extend the single frame-loop type rather than creating another scheduler.

Browser ordering, ownership, and refusal recovery are specified in [`../../../../../../../../docs/julibrot/app.md`](../../../../../../../../docs/julibrot/app.md), with worker transfer rules in [`../../../../../../../../docs/julibrot/worker.md`](../../../../../../../../docs/julibrot/worker.md).
