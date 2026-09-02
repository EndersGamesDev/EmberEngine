# Engine chain backlog — the full system, what is built, what is paid, what remains

Status: the labelled backlog for the engine as designed in the heap-lattice and Julibrot rounds; every item carries a stable label so briefs, reports and reviews can cite it; statuses are `PAID` (proven on the reference hardware), `BUILT` (code with native gates green, browser proof pending), `DESIGNED` (contract or memory only), `OPEN` (not yet designed).

## 1. The chain

`ASTs → lock boundary → DAGs → descriptors → heaps → kernels → geo → projection → scene → re-projection → surface`

The engine is a compiler from world descriptions to GPU tables and the render loop is its runtime; each arrow is a pure function of the previous stage plus time, authority never leaves the first stage, and no arrow points backwards.

|Label|Stage|What it is|Status|Evidence and location|
|---|---|---|---|---|
|EC-01|ASTs|Many producers, each the only owner of its truth: game worlds, UI, network replicas, an editor, a lab; the simulation's fixed 60 Hz tick is one AST's private rhythm|DESIGNED (multi-AST); BUILT (single-world: arena, fire, the Julibrot viewer state)|`crates/arena-core`, `crates/fire-core`, `crates/labs/julibrot/worker` owner|
|EC-02|Lock boundary|Exclusive ownership moved by transfer, never shared memory; ping-pong buffer pairs; a credit header on the returned buffer; producers shape themselves to it (forward pressure); the same-thread lowering is the cheapest case of one abstraction|BUILT|`crates/labs/julibrot/worker`: nine message kinds, four buffers, credit accounting, same-thread twin with a mode-equivalence trace; browser transfer `requires visible replay`|
|EC-03|DAGs|One owner thread holds two petgraph graphs, each double-buffered: a pre-projection DAG on a hard per-refresh deadline and the main scene DAG under a soft budget applied in slices; each swap publishes through a versioned pointer with epochs; the owner is a minimal drain and all shaping lives at the producers|DESIGNED; a miniature BUILT|The Julibrot owner cell with two infallible drains and exhaustive interleaving tests is the miniature; the general graphs, the AST-to-DAG compiler and the dirty-set walk are not built|
|EC-04|Descriptors|Sixteen-byte integer records {class, layer, origin, extent}; handles = index + generation, slot zero is the missing resource; two update rates: the scene-rate table written by the walk, the hot ring of three slots by dynamic offset rewritten at refresh|PAID|`crates/labs/heap` descriptor table and span directory; hot ring in the lattice and in `crates/labs/julibrot/present`|
|EC-05|Heaps|A fixed small set of texture arrays, one format and filter each, bound once; not bindless; relocation is a record change|PAID|`docs/gpu-resource-heap.md`; 2.81× at 1,048,576 draws with 16 B per frame; the lattice at two million edges per frame|
|EC-06|Kernels|Dialect v2: pure WGSL bodies over heap spans lowered to gather-only fragment passes, rendered into SCRATCH and copied into the heap|PAID (lattice); BUILT (Julibrot)|`docs/gpu-heap-lattice.md`; the direct-overlap refusal and the copy path proven by the output-path spike; the escape and scaled-perturbation kernels conformant against the math oracles|
|EC-07|Geo and projection|Handle-fetch geometry, instances carrying mesh and material handles, few draws binned by mesh; projection is the last non-linear step, per vertex|PAID|Mode A and Mode C of the lattice; the double perspective; the rawgl presentation adopted by value with a pinned-constants test|
|EC-08|Scene|One pass, one depth buffer, no mips, no blend, no culling; a retained plus an in-flight scene texture|BUILT|`crates/labs/julibrot/present`; not yet driven by the app's frame loop|
|EC-09|Re-projection|Cooperative asynchronous re-projection owns presentation: at every refresh the last completed scene is warped to the latest pose from the hot ring; reference-shift rebasing keeps deep pans smooth|BUILT|`docs/atw-first-rendering.md` for the decision; the flat exact homography and the bounded tumbled approximation in `present`; not yet seen on screen|
|EC-10|Surface|Single surface ownership by a generation-tagged token, counted fences, present outside every timed region, a panic hook and a non-panicking GPU error handler before the first device call|PAID|Paid in the lattice's v8 and v9 rounds against real crashes|

## 2. The laws

|Label|Law|Status|
|---|---|---|
|EL-01|Substrate: wgpu 24 over WebGL2 only; the device floor is WebGL2 plus `EXT_color_buffer_float` (`docs/minimum-requirements.md`); WebGPU is not a target|PAID|
|EL-02|Authority stays at the AST; everything to the right is presentation or prediction and can only produce a wrong pixel|DESIGNED, honoured by every lab|
|EL-03|Exclusive ownership everywhere: transfer between threads, versioned swap inside the owner, handles into the GPU|BUILT|
|EL-04|The CPU sends integers: uniforms plus regional writes for dirtied nodes per scene frame, one ring slot per refresh|PAID|
|EL-05|Honesty is structural: requested versus delivered, walls computed from live limits and labelled apart from policies, warm-ups excluded, polls counted, never a hang, never a number that was not measured|PAID|
|EL-06|Every wasm entry installs a panic hook and replaces wgpu's fatal uncaptured-error handler before the first device call|PAID|
|EL-07|Kernel outputs never render into the array the immutable bind group samples; they land through SCRATCH and a copy|PAID|
|EL-08|Compute happens only on the servers; every action reports its wall time; commits are unsigned and authored by the loop identity|in force|

## 3. Known costs no architecture removes

|Label|Cost|Evidence|
|---|---|---|
|EK-01|The raster ceiling of an integrated GPU: a few million small triangles per frame at 60 fps|The competition's 1.8× spread across four architectures; the lattice at about 47,600 edges per millisecond|
|EK-02|The wgpu bundle weight, about 4.5 MB of wasm, mostly the shader translator|The raw-GL entry at about 200 KB against every wgpu entry near 4 MB|
|EK-03|Fragment compute is gather-only: no scatter, no atomics, no indirect draw on WebGL2|Dialect v2's forbidden-construct list; draw merging is same-mesh instancing plus binning|
|EK-04|One format and filter per heap and a fixed small number of heap classes, bounded by the fragment stage's sixteen texture units|`docs/gpu-resource-heap.md` §2|

## 4. Open work, in the order it should run

|Label|Item|Depends on|
|---|---|---|
|EO-01|Finish the Julibrot app's frame loop (Phase 3), remove the duplicate `worker_main` placeholder, rebuild the bundle, replay the rapid-input and deep-zoom checklists in the reference browser|worker's four integration seams (in flight)|
|EO-02|Visible replay of the Julibrot: Mandelbrot and Julia presets, a hybrid slice, a zoom past 42 digits under scaled perturbation, continuous zoom under re-projection, the worker's transfer facts|EO-01|
|EO-03|Merge `feature/julibrot` to main after the owner's walk; fold the measured ledger into the five slice documents|EO-02|
|EO-04|The engine design document: the AST-to-DAG compiler, the two petgraph DAGs with double drains and swaps, the credit-based channel as the engine's producer API, the hot ring for camera and late-latched objects, the migration of the arena renderer from fixed mesh ids to handles, one tier at a time with a measured reason each|EO-03; the Julibrot's evidence|
|EO-05|The arena as the first engine customer: mesh table in the heap, instance records with handles, the warp pass presenting the arena, the simulation AST in a worker over the transfer channel|EO-04|
|EO-06|A capability matrix across real low-specification devices, since the whole point is the devices where the surprises live|a device pool|
|EO-07|Bundle-size work: measure what the translator costs, trim features, decide whether a precompiled shader path is worth its complexity|EK-02|
|EO-08|Replay from recorded edit batches: every lock-boundary message is already serialized bytes with an epoch, so recording them makes any frame reproducible; build it before the DAGs, not after|EO-04|

## 5. Where the code lives today

Landed on main: `docs/gpu-resource-heap.md` and `crates/labs/heap`, `docs/gpu-heap-lattice.md` and the lattice inside the heap lab, `crates/labs/layer` as its comparator, `docs/minimum-requirements.md`.

On `feature/julibrot`: `docs/julibrot-lab.md`, the five slice contracts under `docs/julibrot/`, the packages under `crates/labs/julibrot/{math,kernels,worker,present,app}`, and the heap executor seams in `crates/labs/heap/src/executor.rs`.

Not in the repository, by design: the engine model itself, which lives as rulings until EO-04 turns it into a document.
