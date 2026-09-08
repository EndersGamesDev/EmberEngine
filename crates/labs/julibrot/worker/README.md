# Julibrot reference worker

`ember-julibrot-worker` moves expensive arbitrary-precision reference-orbit construction off the browser main thread through four transferable, ownership-tracked buffers and a versioned fixed-layout wire ABI.

The application consumes its owner endpoint and versioned viewer state; the worker side consumes Julibrot math, returns compact orbit records and verification facts, and receives explicit applied-or-stale credit when ownership comes back.

The same-thread native lowering exercises buffer reconciliation, generation rules, cancellation, deadlines, and producer credit without pretending to be a browser Worker implementation.

Pool ownership, ABI fields, precision requests, credit shaping, HOT and MAIN publication, and shutdown are specified in [`../../../../docs/julibrot/worker.md`](../../../../docs/julibrot/worker.md).
