# Julibrot worker source

`lib.rs` exposes worker configuration and endpoints, orbit request codecs, ownership leases, credit accounting, reference tasks, viewer drains, registry handles, wire records, and browser transfer entry points.

`wire.rs` validates the little-endian header, payload, facts, pool trailer, and ABI version; `channel.rs` and `endpoint.rs` reconcile the four buffers; `credit.rs` bounds producer work; and `compute.rs` advances cancellable reference orbits in measured chunks.

`owner.rs` publishes independently staged HOT and MAIN `ViewerState`, `registry.rs` generation-checks installed orbits, and the browser modules lower those contracts to transferable arrays and a dedicated Worker scope.

Module tests pin byte offsets and poisoned tails, pool return exactly once, resize and shutdown, stale work charging, timing walls, generation replacement, coalescing, registry reuse, and same-thread/browser-equivalent ownership under [`../../../../../docs/julibrot/worker.md`](../../../../../docs/julibrot/worker.md).
