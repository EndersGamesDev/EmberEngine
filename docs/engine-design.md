# Engine design — authority to surface

Status: design reference for EO-04; proposed contracts are implementation gates until measured.

Claims use the corpus vocabulary: **Verified** means checked against the cited repository evidence, **Derived** means a consequence of verified facts, and **Proposed** means a design choice that remains subject to its stated rejection measurement.

## 1. The engine is a forward compiler

`ASTs → lock boundary → DAGs → descriptors → heaps → kernels → geo → projection → scene → re-projection → surface`

**Verified — the labelled backlog already defines EC-01 through EC-10 as this chain and EL-02 as its authority law.** It calls the engine a compiler from world descriptions to GPU tables, the render loop its runtime, every arrow a pure function of the preceding stage plus time, and every stage after the AST presentation or prediction rather than truth (`docs/plans/engine-chain-backlog.md:5-34`).

**Verified — EL-01, EL-02, EL-03, EL-04, EL-05, EL-06, and EL-07 bind the chain's substrate, authority, exclusive ownership, regional traffic, measurement honesty, readable failure, and scratch-copy safety.** The design below adds no exception to those laws (`docs/plans/engine-chain-backlog.md:24-34`).

**Proposed — this document is EO-04's executable design reference.** An implementation conforms only when it can identify the input version, explicit time value, output version, ownership transition, bounded work, and rejection result at every arrow; an implicit read of mutable state is an undeclared backward arrow (`docs/plans/engine-chain-backlog.md:46-57`).

**Derived — “pure” describes the semantic transform, not an allocation strategy.** A stage may reuse storage, issue regional GPU writes, or submit work, but the bytes and commands it produces must be determined by the preceding stage's published version plus an explicit time or refresh stamp; memory addresses, queue timing, and which worker happened to run may not change meaning.

**Proposed — time is an input, never a global engine clock.** Every time-sensitive transform names the domain it consumes: a simulation tick, producer epoch, scene deadline, refresh sequence, or presentation time. A stage that needs two domains carries both and their causal identity rather than subtracting unrelated clocks (`docs/state-model.md:29-40`).

**Proposed — no arrow points backward.** Surface visibility, GPU completion, scene quality, descriptor residency, and channel pressure may produce telemetry to the producer that owns the relevant policy, but they may not mutate an earlier stage's truth. Feedback becomes a new AST edit at the responsible producer or remains observation.

**Derived — recomputation and loss are safe to the right of the lock boundary because authority remains left.** A stale scene can cost freshness, a missing descriptor can draw the missing resource, and a rejected warp can hold or clear; none may rewrite world state. This is the same distinction the state model draws between authoritative state, rendered sampling, and presentation (`docs/state-model.md:15-29`, `docs/state-model.md:58-64`).

### 1.1 Stage contracts

**Proposed — the following table fixes each stage's input, publication, and lawful loss boundary.**

|Label|Stage|Consumes|Publishes|May discard or retry|
|-----|-----|--------|---------|--------------------|
|EC-01|ASTs|Producer-owned intent, state, and private time|Epoch-stamped edit batches|Only under that producer's semantics|
|EC-02|Lock boundary|Owned edit buffers|An atomic drain snapshot and returned credit|Unpublished producer drafts|
|EC-03|DAGs|Drain snapshots, prior back graph, explicit time|Versioned dependency graph and dirty frontier|Unpublished back-copy work|
|EC-04|Descriptors|Topologically ordered dirty nodes|Regional table and instance writes|Superseded regions before publication|
|EC-05|Heaps|Handles, descriptor records, resource payloads|Stable slots behind immutable bindings|Unreferenced generations|
|EC-06|Kernels|Dialect programs and heap spans|GPU-resident derived spans|Superseded scratch results before copy|
|EC-07|Geo|Instance and geometry handles|GPU-resident primitives|Culled or invalid instances|
|EC-07|Projection|Geometry and explicit view state|Projected primitives and depth|Unprojectable primitives|
|EC-08|Scene|Projected primitives, materials, one scene target|A stamped completed scene|Incomplete or superseded submissions|
|EC-09|Re-projection|Completed scene and refresh-time hot values|A measured warp, redraw, hold, or clear decision|Unsafe corrections|
|EC-10|Surface|One completed presentation submission|Visible pixels and presentation facts|Stale tokens and failed generations|

**Proposed — every boundary exposes typed refusal rather than best effort.** Cycles, stale generations, incompatible epochs, invalid projection, exhausted capacity, missed safety guards, unsupported devices, and unsafe reprojection are distinct results because silently guessing would erase the evidence needed to accept or reject the design.

## 2. ASTs own truth and time

**Verified — EC-01 names many independent producers: game worlds, UI, network replicas, an editor, and a lab.** Each producer is the only owner of its truth; the current arena, fire, and Julibrot viewer supply single-world evidence but do not establish the multi-AST engine (`docs/plans/engine-chain-backlog.md:11-15`).

**Proposed — an AST is any producer-owned description from which engine-visible edits can be compiled.** It need not be a syntax tree in memory: a deterministic game world, retained UI tree, replicated remote snapshot, editor document, or laboratory control model qualifies when it has stable identities, versioned meaning, and exclusive mutation.

**Proposed — AST identity is a pair of producer identity and producer-local object identity.** No central registry may silently merge two producers' names. Cross-AST references are explicit imported handles or declared relations, so destroying one producer cannot make another producer's unrelated local ID change meaning.

**Derived — producer plurality forbids a single “world update” callback as the engine contract.** Such a callback would make unrelated authors share a clock and mutation window; edit channels instead allow each AST to publish whenever its own semantics say a coherent change exists.

**Verified — the fixed 60 Hz simulation tick belongs to authoritative simulation rather than presentation.** The state model treats the simulation's fixed step as one state level's clock, and the ATW design separates that clock from scene and display cadence (`docs/state-model.md:23-40`; `docs/atw-first-rendering.md:35-49`).

**Proposed — 60 Hz is one AST's private rhythm, not the engine's clock.** UI may publish on input, a network replica on packet acceptance, an editor on transaction commit, and a lab on control change. Their epochs establish order within each producer; they do not pretend to be comparable timestamps across producers.

**Proposed — each AST adapter emits intent at the highest semantic level the DAG compiler understands.** It names created, removed, or changed identities and properties; it does not calculate heap coordinates, command draw order, mutate graph nodes, or retain a graph reference.

**Derived — the authority rule gives recovery a single direction.** When any derived stage is lost, reset, or found corrupt, the engine reconstructs it from AST snapshots and subsequent edits. Recovering an AST from descriptors, pixels, or heap contents would reverse the chain and is forbidden.

## 3. The lock boundary

**Verified — EC-02 and EL-03 establish exclusive ownership by transfer, a returned credit header, and a same-thread lowering of the same abstraction.** The Julibrot worker proves four-buffer ownership, nine message kinds, same-thread trace equivalence, and browser Transferables; it deliberately uses no shared memory (`docs/plans/engine-chain-backlog.md:14`, `docs/julibrot/worker.md:79-95`, `docs/julibrot/worker.md:313-349`).

**Proposed — the general boundary is crossbeam-shaped: one bounded edit channel per AST, many producers in aggregate, and exactly one DAG consumer.** “Crossbeam-shaped” specifies exclusive send/receive ownership and bounded queues; it does not require the browser lowering to implement shared-memory queues or a particular Rust channel crate.

**Proposed — an edit buffer header carries producer identity, monotonically increasing producer epoch, buffer identity and generation, payload length, and the last consumer credit returned to that producer.** The payload is a sequence of framed transactions whose internal order is authoritative for that producer.

**Proposed — the DAG thread drains each channel atomically at a named boundary.** Atomic drain means ownership of the currently available complete buffers transfers as one snapshot: edits arriving after the boundary wait for the next drain, and no producer can mutate a transferred buffer. It does not mean every drained transaction must fit in one MAIN budget slice.

**Proposed — epochs reject ambiguity rather than repair it.** A duplicate epoch is idempotently reported, a skipped epoch is a typed gap requiring producer recovery, and an older epoch is stale. The consumer never invents a missing edit or reorders epochs to improve throughput.

**Proposed — no AST ever holds the DAG.** Producers receive handles, schemas, credits, and statistics; they never receive a graph node reference, mutation closure, lock guard, or callback executed inside the graph. The single consumer is therefore the only place graph invariants can be violated.

**Derived — there is no mutual-exclusion lock in the transfer proof.** The “lock” is the point where an immutable semantic transaction crosses by exclusive buffer ownership. A native mutex around a shared graph would allow producer code to run inside the graph's critical section and would fail this contract even if it happened not to contend.

**Proposed — one producer cannot block another's drain.** Each bounded channel is polled independently in stable producer order with a per-channel maximum accepted byte count learned from credit; a starved or corrupt channel produces facts and refusal for its producer while other channels remain drainable.

**Proposed — boundary measurements record queued buffers, queued bytes, drain wall, oldest epoch age, gaps, stale arrivals, returned buffers, and buffer-starvation events by producer.** Reject the boundary if a producer obeying its credit can exhaust the pool, if drain p95 breaks the PRE-PROJECTION guard, or if any interleaving changes the accepted epoch sequence.

**Verified — EO-08 requires replay from recorded epoch-stamped edit batches before the general DAGs.** Recording the boundary bytes and delivery order therefore supplies P-01's frame-reproduction input without treating a derived graph as authority (`docs/plans/engine-chain-backlog.md:57`).

## 4. The AST-to-DAG compiler

**Verified — EC-03 identifies two future petgraph graphs and names the Julibrot owner cell as only a miniature.** The general AST-to-DAG compiler and dirty-set walk are explicitly not built; the lab itself excludes a general DAG and petgraph (`docs/plans/engine-chain-backlog.md:15`; `docs/julibrot-lab.md:52-54`).

**Proposed — each graph is one `petgraph::StableGraph` containing the resource graph and frame graph together.** Stable indices support long-lived engine handles, while an engine generation beside each index prevents removal and reuse from reviving an old reference. The active dependency relation must remain acyclic even though the container alone cannot guarantee that property.

**Proposed — a node is a stable, typed declaration with source provenance, inputs, output identity, last-applied producer epochs, dirty state, and one pure evaluator.** Resource nodes declare durable data such as meshes, images, materials, kernel spans, and descriptor regions; frame nodes declare transient derivations such as instance sets, kernel dispatches, geometry bins, projections, scene passes, and publication records.

**Proposed — an edge is a typed forward dependency from a node whose published output is consumed by another node.** Edge kinds distinguish semantic data dependence, resource lifetime, execution order, and write hazard; they never encode reverse notification. Dirty propagation follows the forward adjacency regardless of edge kind, while scheduling may apply the stricter kind-specific constraints.

**Derived — placing resources and passes in one graph makes lifetime and execution order jointly checkable.** A pass cannot read a resource that has no live producing path, and resource relocation can dirty every consuming pass without a second graph's identity join. “Resource graph plus frame graph” denotes two roles in one acyclic dependency structure, not two separately synchronized databases.

**Proposed — compilation is a deterministic fold of each drained transaction into the back graph.** Create resolves or allocates stable nodes, update changes declared inputs, relate adds or removes typed edges, and destroy tombstones the identity after dependents are either removed or redirected to the missing resource. Invalid identity, generation, schema, or cycle closes the transaction without publishing half of it.

**Proposed — cycle detection is an admission gate.** Edge insertion checks whether the destination already reaches the source in the active relation; a cycle is a typed compiler error attributed to the producer, its epoch, and the proposed edge. The engine does not break the cycle by deleting an arbitrary dependency.

**Proposed — dirty marking records causes, not merely a boolean.** A node carries the earliest unapplied producer epoch per contributing AST and reason bits for value, topology, resource, pass-order, or time invalidation; repeated causes merge without losing the oldest provenance needed for latency accounting.

**Proposed — the dirty walk uses a stable topological order over the affected forward closure.** A node evaluates only after all dirty predecessors have produced their new back-copy outputs; stable node identity breaks otherwise equivalent ordering ties, so producer interleavings that yield the same epoch snapshot yield the same bytes.

**Proposed — evaluators emit regional descriptor writes, regional instance writes, heap uploads or relocations, kernel jobs, and pass records into an append-only back-copy publication plan.** They do not call the surface and do not mutate the front publication. EL-04's CPU contract remains uniforms plus dirtied regional writes, not a whole-table upload (`docs/plans/engine-chain-backlog.md:26-34`).

**Proposed — time dirtiness is explicit.** Nodes that depend on scene time declare their cadence or predicate and are inserted into the dirty frontier with the time stamp that triggered them; walking the entire graph merely because a frame exists is a failed implementation.

**Proposed — compiler measurements record transaction validation, nodes touched, forward-closure size, topological-walk wall, writes and bytes by region, allocation churn, cycle refusals, and no-op edits.** Reject the design if steady unchanged refreshes walk the graph, if equal snapshots emit unequal plans, or if regional traffic approaches full-table traffic under representative localized edits.

## 5. Two DAGs, four graph instances, one owner thread

**Proposed — one DAG thread owns PRE-PROJECTION and MAIN, and each DAG has a front and a back `StableGraph`.** There are four graph instances, never one graph guarded for readers. The front is immutable for its published version; only the DAG thread mutates the back.

**Proposed — PRE-PROJECTION is the hard-deadline graph.** It contains structural choices needed at surface refresh, including which handles are hot, their descriptor locations, warp inputs, UI/viewmodel membership, and required motion records. At every refresh its sequence is `drain → swap → emit hot descriptor metadata → warp`, with compilation of complete admitted transactions contained inside the bounded drain phase.

**Proposed — MAIN is the budgeted scene graph.** It receives scene-rate world, resource, and pass edits, applies coherent transactions in stable order until the safety deadline guard would be crossed, and carries the unapplied suffix forward. The producer's credit should make the guard exceptional; that thread does not invent a second pacing policy.

**Proposed — one refresh contributes one MAIN application slice.** After the refresh-critical PRE-PROJECTION path is secured, the DAG thread drains MAIN, applies a coherent prefix within the remaining declared budget, and swaps that prefix for subsequent scene work; later refreshes resume the ordered suffix rather than replaying or coalescing it.

**Proposed — every surface refresh performs two independent drains and two swaps.** PRE-PROJECTION drains and swaps before warp; MAIN drains and swaps at its scene-work boundary so a coherent applied prefix becomes available for subsequent scene submission. An empty drain still advances refresh observation but may publish the same graph version with a new refresh fact rather than clone data.

**Proposed — a graph swap exchanges front and back roles after the back represents a coherent prefix.** Once the former front has no reader lease, the DAG thread brings it from its recorded base epoch to the published baseline by replaying the committed mutation log, then applies new work there; this keeps exactly four graph instances and makes catch-up bytes and wall first-class costs.

**Derived — partial MAIN application is safe because no reader sees the graph being changed.** Transactions are atomic within the back copy, the swap exposes only the last coherent prefix, and the ordered suffix remains owned for later application. Partial does not mean a node record, descriptor write, or edge set can be visible half-formed.

**Proposed — the back graph and its emitted publication plan are invisible until swap.** Descriptor uploads or instance writes staged for an uncommitted transaction cannot leak ahead of the graph version that gives them meaning.

**Proposed — publication uses an arc-swap-shaped versioned pointer carrying graph kind, publication epoch, included producer epochs, graph generation, and descriptor-plan identity.** “Arc-swap-shaped” specifies atomic whole-version replacement and retained lifetime; browser and single-thread lowerings may implement the same semantics without shared-memory atomics.

**Proposed — a reader loads exactly one published version and holds it for a whole frame.** Scene encoding, descriptor resolution, kernel scheduling, geometry, and projection for that frame use the same MAIN version; warp reads one PRE-PROJECTION version and the one scene version stamped on its source. Reloading mid-frame is a torn-frame bug.

**Proposed — graph reader leases end after CPU frame construction and before that graph instance can become the mutable back copy; GPU submissions retain emitted tables and commands, not a graph reference.** A lease surviving into the next reuse is a typed `GraphLeaseBusy` failure that preserves the last publication and counts a missed swap rather than blocking the refresh.

**Proposed — publication epochs are monotonic per graph and carry a producer-epoch vector.** Equality of publication epochs means identity of the published graph, while producer epoch comparison answers freshness. A single scalar is insufficient to claim that independently clocked ASTs are mutually current.

**Proposed — hard and soft limits are measured separately.** PRE-PROJECTION records drain, apply, emit, and guard margin against each refresh; MAIN records admitted credit, coherent transactions applied, suffix depth, wall consumed, and budget remaining. Reject the four-graph design if copying dominates the saved work, readers retain versions until memory is unbounded, PRE-PROJECTION misses its guard, or MAIN age grows while compliant producers receive positive credit.

## 6. Credit is the producer API

**Verified — the Julibrot channel already returns consumer facts in transferred buffers and leaves shaping at the producer.** Its consumer reports credit but never delays, rejects, or coalesces edits for the producer; the same-thread and worker endpoints expose one transport-independent state machine (`docs/julibrot/worker.md:123-153`, `docs/julibrot/worker.md:313-349`).

**Proposed — every returned edit buffer carries the next credit plus the consumer's apply wall, budget remaining, highest epoch applied, queue depth, and refusal counters for that producer.** These facts describe observed capacity; they are not promises that the next edit has zero cost.

**Proposed — credit is denominated in estimated consumer work and capped bytes, not message count alone.** Each edit opcode has a conservative learned cost charged by touched nodes, expected forward fan-out, and payload bytes; the return path updates the estimate from measured application without allowing a single underestimated edit to exceed the absolute safety cap.

**Proposed — producers own coalescing.** Before transfer, each AST may replace superseded value edits, collapse create-update chains, combine adjacent regions, or choose a lower semantic rate when its own rules allow. It must preserve transactions, epochs, destroys, cross-object ordering, and any edit whose omission changes truth.

**Proposed — producers size the next batch to returned credit.** Zero credit means retain and coalesce locally; positive credit bounds the next offered work. A correctness transaction that cannot be split and cannot fit the maximum credit is a typed `EditTooLarge` configuration refusal, not permission to overfeed.

**Proposed — the DAG thread performs no coalescing, throttling, sleeps, token-bucket delay, or clock slicing.** It drains the already-shaped buffers, applies complete transactions in order, and stops MAIN only at the safety deadline guard. PRE-PROJECTION rejects or defers a buffer that cannot fit its hard boundary rather than beginning an unsafe transaction.

**Derived — overfeeding is a producer-side bug.** The consumer reports offered work, admitted work, applied wall, guard stops, epoch age, and returned credit under that producer's identity; the engine does not hide a producer's excess by making every other AST wait.

**Proposed — initial credit is deliberately small and calibration is monotone-safe.** A new producer proves cost with bounded batches, credit grows only from sustained margin, and any guard stop cuts its next credit immediately. Reject the API if compliant producers oscillate without converging, if cost prediction systematically understates fan-out, or if consumer-side queueing becomes the normal backpressure mechanism.

## 7. Hot descriptors at warp rate

**Verified — EC-04 and EL-04 require a scene-rate descriptor table plus one hot ring slot per refresh.** The Julibrot present crate implements a three-slot `HotUniform` selected by dynamic offset, and app performs `poll → drain_hot → construct pose → write_hot` before each frame (`docs/plans/engine-chain-backlog.md:16`, `docs/julibrot/present.md:381-387`, `docs/julibrot/app.md:147-159`).

**Proposed — hot descriptors are a small value class updated at WARP rate, meaning surface refresh rather than scene production.** The initial class contains camera pose, late-latched player-controlled poses such as a viewmodel, UI transforms, and motion data needed to reproject the retained scene.

**Proposed — the immutable bind group contains one fixed-identity hot UBO whose storage is a dynamic-offset ring.** The refresh chooses a slot that cannot still be in flight, writes only that slot, and binds its offset; scene submissions holding older slots never stall or observe replacement.

**Proposed — the ring starts at three slots because the Julibrot has paid that layout, but slot count is a measured capability value rather than a universal constant.** It must cover the maximum simultaneously in-flight scene and warp submissions on the target; exhaustion is a typed `HotRingBusy` refusal or retained presentation, never an overwrite (`docs/julibrot/present.md:381-387`, `docs/julibrot/present.md:423-427`).

**Proposed — PRE-PROJECTION decides which handles are hot and publishes their stable locations; the warp pass's own producer writes their latest values directly into the selected ring slot.** This value-only path bypasses the edit-channel drain and graph mutation, while any membership, identity, layout, or resource change remains a graph edit for the next PRE-PROJECTION swap.

**Derived — bypass is safe only because it cannot change meaning.** A direct hot write may update the value of an already-declared field for an already-selected handle; it may not allocate, free, change generation, alter dependencies, switch a material, create geometry, or move truth between ASTs.

**Proposed — hot data is warp-only and cosmetic.** It is never fed into simulation, sent on the network, stored in a save, used to advance a tick, or copied back into an AST. If gameplay needs the same input, its authoritative consumer reads the input stream independently and publishes a later AST epoch (`docs/atw-first-rendering.md:90-122`; `docs/state-model.md:58-64`).

**Proposed — scene frames stamp the hot slot or pose version from which they were rendered.** Re-projection computes from that stamped source to the newest compatible destination; treating the newest hot values as both source and destination would erase the motion the warp must correct.

**Proposed — hot measurements include bytes per refresh, slot wait/refusal count, source-to-destination age, late-latch age, write wall, and warp compatibility outcome by hot class.** Reject a proposed hot class if it changes non-cosmetic truth, commonly forces redraw, makes the UBO exceed the floor, or stalls an in-flight scene often enough to miss the refresh guard.

## 8. Descriptors, heaps, and kernels

**Verified — EC-04 through EC-07 are paid stages on the WebGL2 floor.** They establish integer descriptors, fixed heaps, dialect-v2 kernels, handle-fetch geometry, and projection as the last nonlinear per-vertex step (`docs/plans/engine-chain-backlog.md:16-19`).

**Verified — the heap is bind-once integer indirection, explicitly not bindless.** WebGL2 sees a fixed finite DATA texture array, IMAGE texture array, their fixed samplers, and a descriptor UBO; immutable bind-group identity survives allocation and relocation because updates change contents and records rather than bindings (`docs/gpu-resource-heap.md:5-23`, `docs/gpu-resource-heap.md:59-67`).

**Verified — descriptors and handles already have a paid lifetime contract.** The descriptor table is a UBO of fixed-size integer records, slot zero is the missing resource, and a handle combines descriptor index with a generation that changes on free so stale CPU use is rejected (`docs/gpu-resource-heap.md:33-57`).

**Proposed — the DAG compiler targets that existing ABI rather than replacing it.** Resource nodes allocate or resolve generation handles, descriptor nodes emit regional UBO writes, instance nodes carry handles, and relocation changes a descriptor record while stable upstream identities remain unchanged.

**Derived — DATA and IMAGE are heap classes, not AST categories.** An AST describes semantic resources; descriptor compilation chooses a physical heap class only after format, sampling, capacity, and lifetime requirements are known. A new format or filtering rule requires a separately measured class rather than a hidden special binding (`docs/gpu-resource-heap.md:17-31`).

**Verified — kernel dialect v2 runs pure kernel bodies over heap spans and lowers them to gather-only fragment work through SCRATCH before copying results into the sampled heap.** Direct output overlap is refused, and compute-to-geometry stays GPU-resident (`docs/gpu-heap-lattice.md:29-57`, `docs/gpu-heap-lattice.md:136-166`; `docs/plans/engine-chain-backlog.md:18`, `docs/plans/engine-chain-backlog.md:34`).

**Proposed — a kernel node consumes generation-checked input spans and publishes a new output-span generation only after the scratch copy completes.** Geometry nodes may consume that published span by handle; no normal path reads kernel output back to an AST or expands it into per-instance CPU data.

**Derived — bind-once does not mean write-once.** Regional descriptor, texture, instance, and uniform writes are the intended mutation mechanism; resource identity is immutable at binding boundaries while data versions advance behind handles. Whole-table rebuilds remain evidence of a failed dirty walk, not a feature of the heap.

**Proposed — heap pressure returns through typed allocation facts to the AST producer responsible for policy.** Missing-resource substitution is legal for a derived pixel, but eviction priority, content reduction, or retry becomes a producer edit; the heap cannot delete authoritative objects merely to fit.

**Proposed — descriptor and heap gates record regional bytes, descriptor resolves, stale-generation refusals, relocations, fragmentation, live spans, scratch copies, bind changes, readbacks, and compute-to-geometry residency.** Reject the integration if graph-driven rendering reintroduces per-object bind groups, per-draw uploads, silent generation wrap, direct sampled-output overlap, or routine CPU readback.

## 9. Concurrency lowerings

**Verified — the Julibrot worker contract uses Web Workers, `postMessage`, and Transferables with exclusive buffer ownership, and explicitly excludes shared-memory threads.** Its same-thread mode preserves the same state machine and ownership trace (`docs/julibrot-lab.md:41-54`; `docs/julibrot/worker.md:69-95`, `docs/julibrot/worker.md:313-349`).

**Proposed — Web Workers are the browser concurrency lowering.** Each producer that warrants isolation owns its AST and producer endpoint in one worker; the main or dedicated DAG worker owns the consumer endpoint and graph state. Messages carry framed transferred buffers, never references into another wasm instance.

**Proposed — the browser design has no shared-memory wasm threads and therefore no COOP/COEP dependency.** Correctness may not rely on `SharedArrayBuffer`, atomics, shared linear memory, or cross-origin isolation; a platform may still supply those features without changing this contract (`docs/minimum-requirements.md:19-29`).

**Derived — Transferable means an ownership proof, not a physical zero-copy promise.** Sender detachment and unique return establish who may mutate the bytes; browsers remain free to copy internally, and transfer wall and allocation behavior stay measured facts (`docs/julibrot/worker.md:437-445`).

**Proposed — native lowering may use bounded ownership-transfer channels with the same edit framing, epochs, credits, and returned buffers.** Native scheduling must not leak into semantics: an equal producer trace yields the same accepted transaction order and publication epochs as the browser trace.

**Proposed — single-thread wasm is the cheapest lowering of the channel abstraction, not a degraded mode.** Endpoints hand the same owned buffers through an in-process queue and run producer steps cooperatively; message kinds, epochs, credit, cancellation, statistics, and refusal results remain identical.

**Derived — “same abstraction” forbids a direct-call shortcut into the DAG.** Removing transfer overhead is permitted; removing exclusive buffer state, bounded queue, producer credit, epoch validation, or return path would create a second engine whose behavior cannot validate the worker path.

**Proposed — lowering equivalence is tested with adversarial traces.** Reordered delivery, delayed returns, cancellation, stale epochs, pool growth, zero credit, shutdown, and producer overfeed must produce the same semantic events across Web Worker, native, and single-thread modes, with only measured transport facts differing.

**Proposed — reject a concurrency lowering if it requires cross-origin isolation, permits two writers to one buffer or graph, changes accepted epoch order, strands a transferred buffer, or makes progress depend on an unbounded wait.** A performance loss alone selects another lowering; a semantic difference rejects it.

## 10. The device floor

**Verified — EL-01 fixes the target at wgpu 24 over WebGL2, not WebGPU.** The required device floor is WebGL2 plus `EXT_color_buffer_float`, and the project does not assume optional float filtering, float blending, timestamps, WebGPU, or shared-memory wasm (`docs/plans/engine-chain-backlog.md:26-34`; `docs/minimum-requirements.md:5-29`).

**Proposed — initialization requests and validates exactly that floor before the first engine resource is created.** WebGL2 context creation and the float render-target extension are conjunctive requirements; live limits then size heaps, descriptor tables, hot-ring alignment, texture extents, and in-flight work.

**Verified — a missing floor is already specified as a typed refusal rather than a degraded picture.** Initialization must name the exact absent capability and stop; it may not switch to WebGPU, CPU rendering, reduced precision, or an untested texture path (`docs/minimum-requirements.md:31-39`).

**Derived — “nothing else assumed” makes optional features optimizations with equivalence oracles.** A faster path using an exposed optional capability must retain the floor path, report which path ran, and demonstrate semantic equivalence; absence of the feature cannot change supported content meaning.

**Proposed — engine-level initialization aggregates refusals without obscuring the first cause.** Surface, heap, descriptor, shader translation, and float-target checks return typed variants carrying requested and delivered limits; UI may format them, but no subsystem replaces them with a panic or blank canvas.

**Proposed — floor conformance is rejected when any mandatory scene, kernel scratch-copy, hot-ring, reprojection, or surface path needs an undeclared capability.** A content tier may exceed live capacity and refuse that content, but the base engine must still run its minimum conformance scene on every admitted device.

## 11. The Julibrot as the first chain miniature

**Verified — the Julibrot charter says the lab pays the smallest real form of the chain's remaining stages and deliberately omits mechanisms it does not need.** Its five crates are separate slices integrated by app, and the general DAG, multiple worlds, a simulation tick, multiple heap classes, shared-memory threads, and WebGPU are explicitly absent (`docs/julibrot-lab.md:7-11`, `docs/julibrot-lab.md:25-58`).

**Derived — the lab is a miniature and evidence source, not a disguised general engine.** Refactoring should expose the contracts already proved, then add the missing DAG and producer plurality around them; turning Julibrot-specific orbit, grid, or pose meaning into engine vocabulary would invert that dependency.

### 11.1 Current stage map

**Verified — the following map uses the five current crates under `crates/labs/julibrot/` and the modules named by their slice contracts and implementation maps.** The shortened `julibrot/...` paths below all have that directory as their prefix (`docs/julibrot-lab.md:29-37`; `docs/julibrot/present.md:566-584`; `docs/julibrot/app.md:565-579`).

|Engine stage|Julibrot crate and current modules|What is already the seam|What is still missing|
|------------|----------------------------------|------------------------|---------------------|
|ASTs|`julibrot/app`: `state`, `saved`, `runtime`; `julibrot/math`: `types`, `plane`, `screen`, `navigation`|Viewer controls and saved rows own one world's intent; math supplies pure pose and map records|Multiple producers and a general AST adapter|
|Lock boundary|`julibrot/worker`: `channel`, `endpoint`, `browser`, `browser_owner`, `slots`, `wire`|`OwnerEndpoint`/`ProducerEndpoint`, four transferred buffers, epochs, typed messages, returned ownership|One edit schema shared by arbitrary AST adapters|
|DAGs|`julibrot/worker`: `owner`, `registry`; app's HOT and MAIN drains|`ViewerOwner` is a versioned single-consumer cell with two drains|No `StableGraph`, resource/frame nodes, edge admission, double graph copies, or dirty walk|
|Descriptors|`julibrot/present`: `uniform`, `tile`, `state`; app's `frame`|`HotSlot`, `HotUniform`, `SceneUniform`, tile keys and headers establish value layouts and identities|A general descriptor table emitted from graph nodes|
|Heaps|`crates/labs/heap`, consumed by `julibrot/kernels` and `julibrot/present`|One DATA heap class, typed spans, immutable presentation identities|IMAGE use in this lab and multiple engine heap classes|
|Kernels|`julibrot/kernels`: `dialect`, `shallow`, `perturb`, `refinement`, `tile_job`, `gpu`|Dialect-v2 escape and perturbation jobs land grids through scratch copy|Graph-issued jobs rather than app-issued refinement work|
|Geo|`julibrot/present`: `mesh`, `lattice`, `gpu/device/scene`|The grid mesh fetches GPU-resident escape records by span|General instance records and mesh/material handles|
|Projection|`julibrot/math`: `reprojection`, `warp`; `julibrot/present`: `homography`, `planner`|Exact plane maps and the bounded pose-to-pose planner|A general projection node vocabulary|
|Scene|`julibrot/present`: `gpu/device/scene`, `gpu/device/scene/submit`, `gpu/device/ledger`; app's `frame/schedule`|Retained plus pending scene identities, one depth composition, progressive levels|A MAIN DAG determining scene work|
|Re-projection|`julibrot/present`: `planner`, `gpu/device/warp`, `gpu/device/redraw`; app's `frame/warp`|Measured homography, relief redraw, honest hold/clear, HOT consumption|Tile-resolved source selection and composition|
|Surface|`julibrot/app`: `surface`, `frame/loop`, `frame/loop/browser/submit`, `measurement`|Single surface token, separate scene/warp fences, post-fence present|Only extraction into the engine surface contract|

**Verified — these module boundaries agree with the five slice contracts.** Math owns pure algebra and oracles, kernels own GPU computation, worker owns scheduling/transfer/credit/publication, present owns scene textures/HOT/planning/submission, and app owns device, surface, schedule, controls, facts, and integration (`docs/julibrot/math.md:5-13`; `docs/julibrot/kernels.md:319-355`; `docs/julibrot/worker.md:313-373`; `docs/julibrot/present.md:417-437`; `docs/julibrot/app.md:147-175`).

**Derived — `worker::{channel,endpoint,browser_owner,owner,registry}` is the existing lock-boundary miniature.** Channel and endpoints transfer owned buffers and return credit; `ViewerOwner` is the single consumer with HOT and MAIN drains at the point where a future graph-owning thread will compile edits. It proves the handoff, not the missing DAG.

**Derived — `present::{uniform,planner,gpu}` spans the existing hot-descriptor, re-projection, and surface-facing presentation seams.** The HOT ring carries refresh-rate values, the planner classifies pose changes, and the GPU modules render or reproject into app's exclusively held surface; `kernels::{dialect,shallow,perturb,gpu}` is the kernel stage rather than presentation policy.

**Verified — the present crate already contains stage-0 tile value types without putting them in the shipped draw path.** `TileContentKey`, `TileRenderKey`, `TilePoseHeader`, `DescriptorSamplePair`, `CanonicalChartCellKey`, and `TileQuality` pin content/render identity, pose-stamped tile metadata, paired value/depth samples, same-surface cells, and deterministic replacement policy (`docs/julibrot/present.md:299-305`).

**Derived — the present crate is the value/shade seam.** Today it retains escape records and presents them through palette/shading, while the tiled design makes the split explicit: cache semantic value plus reconstruction data, then shade after selecting and reprojecting the target contribution. Palette changes therefore belong to presentation values, not tile recomputation (`docs/julibrot/present.md:45-55`; `docs/julibrot/tiled-reprojection.md:101-160`).

### 11.2 Tiled-reprojection constructs on the chain

**Derived — each tiled-study construct lands where its identity first becomes meaningful and remains a forward input to later stages.**

|Study construct|Landing stage|Reason|
|---------------|-------------|------|
|Pose-stamped tile descriptor|Descriptors|It names content identity, source pose, sampled rectangle, quality, heap spans, and validity consumed by later stages|
|Descriptor map|Descriptors → heaps|It is the integer indirection from logical tile/cell identity to resident value and reconstruction spans|
|Chart pyramid as an index|DAGs|It finds overlapping candidate descriptors and drives dirtiness/demand; it is not rendered geometry or authority|
|Demand-ordered scheduler|DAGs → kernels|MAIN graph nodes emit deterministic jobs ordered by holes, visible benefit, cost, and stable ID|
|Tile reconstruction values|Kernels → geo|Kernels publish paired GPU-resident value and source reconstruction records consumed without readback|
|One-pass depth composition|Scene → re-projection|Candidate tiles reconstruct into the target pose and compete in one target depth domain before the surface is shaded|

**Verified — the tiled study defines these responsibilities rather than a texture cache alone.** It separates coordinate identity, descriptor/heap relationship, compatibility, target selection, one-pass depth composition, demand order, bounded resource policy, and staged migration (`docs/julibrot/tiled-reprojection.md:37-89`, `docs/julibrot/tiled-reprojection.md:101-160`, `docs/julibrot/tiled-reprojection.md:187-249`, `docs/julibrot/tiled-reprojection.md:347-360`).

**Derived — chart hierarchy is an index, never a compositor.** A chart-pyramid cell narrows the descriptor candidates for a target region; physical target depth resolves intersections. Allowing the index to choose the visible surface would confuse spatial lookup with geometric truth (`docs/julibrot/tiled-reprojection.md:69-89`, `docs/julibrot/tiled-reprojection.md:205-225`).

**Proposed — the one-pass target is value first, shade once.** Each selected candidate reconstructs its target-space point and contributes semantic escape value plus validity to one depth-tested composition; the winning value is shaded for the current presentation after composition, avoiding pre-shaded tile seams and palette invalidation.

### 11.3 Refactoring rounds and named seams

**Proposed — round one is survey.** It records existing ownership, call direction, versions, allocations, regional writes, fences, and facts at each seam below without changing behavior; any undocumented dependency becomes a survey finding before extraction.

**Proposed — round two is debt clearing.** It removes duplicate entry points, hidden global access, mixed ownership, stale naming, and app-to-internal-module reach-through only where the survey shows they obstruct a named seam; every removal preserves existing lab traces and visible behavior.

**Proposed — round three exposes the tile-based render and reprojection seams.** It introduces engine-neutral records and ports at the named boundaries, keeps the current whole-grid path as the reference arm, and lands tile constructs in the stages of §11.2 one independently measurable step at a time.

**Proposed — the refactor names the following twelve seams.**

- **J-01 AST edit seam:** `app::{state,saved,runtime}` to an epoch-stamped producer adapter.
- **J-02 ownership-transfer seam:** `worker::{channel,endpoint,browser_owner,wire}` behind the engine edit-buffer channel.
- **J-03 owner/drain seam:** `worker::{owner,registry}` and HOT/MAIN drains behind the single DAG consumer port.
- **J-04 graph-compilation seam:** drained Julibrot edits to future resource/frame nodes without importing Julibrot types into the graph core.
- **J-05 hot-publication seam:** `HotSlot`, `HotUniform`, and `Presenter::write_hot` behind fixed-identity dynamic-offset writes.
- **J-06 scene-descriptor seam:** `PresentMain`, `SceneUniform`, tile keys, pose headers, and descriptor pairs behind regional descriptor emission.
- **J-07 kernel-job seam:** `kernels::{refinement,tile_job,gpu}` behind graph-issued dialect-v2 jobs and published span generations.
- **J-08 value/reconstruction seam:** paired semantic escape values and source reconstruction data from kernels into resident tile spans.
- **J-09 geometry/projection seam:** `present::{mesh,lattice,homography,planner}` consumes handles and explicit pose-stamped records.
- **J-10 scene-publication seam:** `present::gpu::device::{scene,ledger,poll}` publishes a completed scene identity and source pose.
- **J-11 reprojection/composition seam:** `planner`, `device::{warp,redraw}`, and tile candidate composition produce measured warp, redraw, hold, or clear.
- **J-12 surface seam:** `app::{surface,frame::loop,measurement}` owns acquisition through matching completion and presents outside timing.

**Proposed — seam acceptance requires a two-arm oracle.** Before replacing a current call path, the extracted path must produce equal owner events, accepted epochs, descriptor bytes, scene identities, planner outcomes, pixels within the existing oracle, fence order, and facts on the whole-grid fixtures; tile-only behavior adds separate study oracles instead of weakening those pins.

**Derived — the lab still remains one world and one heap class after seam extraction.** The engine can prove multiple-producer and multiple-class behavior with synthetic adapters before asking the Julibrot to become a use case it was designed to exclude (`docs/julibrot-lab.md:52-54`).

## 12. Arena migration and the 4D target

**Verified — Ember's fixed-order mesh IDs are already described as a proto-heap.** The heap contract makes allocation, lifetime, capacity, and backend lowering explicit without giving heap contents simulation authority (`docs/gpu-resource-heap.md:13-15`).

**Verified — EO-05 names the arena as the first engine customer after this EO-04 design.** Its intended entry points are the mesh table, handle-bearing instances, warp presentation, and simulation AST over the transfer channel (`docs/plans/engine-chain-backlog.md:53-55`).

**Proposed — arena migration advances one tier only when the preceding tier supplies its measured reason.** Each tier retains the previous arm as an equivalence and rollback path until its gate passes; “the heap lab was faster” cannot substitute for an arena measurement.

|Tier|Change|Measured reason to advance|Reject or remain on prior tier when|
|----|------|--------------------------|-----------------------------------|
|A0 Baseline|Instrument fixed mesh IDs, per-draw bindings, uploads, CPU encode, GPU scene, memory, and pixels without changing representation|Produces the arena-specific cost distribution and parity images needed by every later comparison|Counters are incomplete, unstable, or distort frame cost beyond the declared probe bound|
|A1 Handle shell|Replace external fixed IDs with index-plus-generation handles that resolve to the same fixed mesh table|Proves stale-reference rejection, deterministic allocation, and pixel identity before moving data|Any stale handle resolves live, generation retirement is unbounded, or CPU cost exceeds the predeclared noise band|
|A2 Descriptor mesh table|Move mesh metadata behind the descriptor UBO while keeping current geometry and draw topology|Advances only if regional bytes, binding calls, or encode wall improve on representative arena frames|Whole-table writes appear, binds rise, pixels diverge, or the improvement is below measurement resolution|
|A3 Instance handles|Make instance records carry mesh and material handles, then bin by resolved mesh|Advances only if draw/bind count or CPU encode p95 falls without unacceptable bin/build cost|Sorting dominates, instance bytes grow past budget, or stable ordering and pixel oracles fail|
|A4 DATA and IMAGE heaps|Place numeric/material data and eligible images in their declared heap classes behind immutable bindings|Advances only if upload, relocation, residency, and scene cost beat the A3 arm within live device walls|Fragmentation, class proliferation, optional-capability dependence, or missing-resource frequency exceeds its gate|
|A5 GPU-derived geometry|Let eligible dialect-v2 kernels publish spans consumed by geometry handles without CPU readback|Advances only when a real arena workload shows lower CPU/upload cost and acceptable GPU/latency cost|Readback returns, scratch-copy or GPU cost erases the gain, or derived data can influence simulation authority|
|A6 Producer and DAG|Move the simulation AST to its channel and compile arena scene edits through MAIN/PRE-PROJECTION graphs|Advances only when equal fixed-tick replays and pixels accompany bounded drain, dirty-walk, publication, and scene age|Simulation rhythm changes, a graph becomes authoritative, graph overhead exceeds regional savings, or compliant backlog grows|

**Derived — the first three tiers isolate identity from placement and placement from draw policy.** That separation gives failures a single cause: generation safety at A1, regional descriptor economics at A2, and batching economics at A3. Combining them would make a faster frame unable to prove which contract earned it.

**Proposed — arena hot adoption is likewise incremental.** Camera values enter the hot ring first, then a player-controlled viewmodel pose, then UI transforms, and finally motion data; each class advances only when late-latch age improves without increasing incompatible warp/redraws or crossing the refresh guard.

### 12.1 The unmerged 4D corpus is a target, not this design

**Verified — `.lane/4d-arena/` contains four branch-only documents: `4d-first-engine.md`, `4d-content.md`, `4d-torus-world.md`, and `bound-movement.md`.** They remain an unmerged target corpus; this document does not copy their designs or promote that branch into the current engine contract (`.lane/4d-arena/4d-first-engine.md:1`, `.lane/4d-arena/4d-content.md:1`, `.lane/4d-arena/4d-torus-world.md:1`, `.lane/4d-arena/bound-movement.md:1`).

**Proposed — the chain must be able to carry three generalization points without knowing their mathematics.** The higher-dimensional world quotient belongs in the authoritative AST, the slice operation becomes an explicit derived stage, and the after-the-fact warp limit becomes the structural split between scene work and safe presentation work.

**Derived — the world quotient belongs in the AST because it defines source identity and placement, not pixels.** The target topology is a finite quotient source queried into local views; treating its manifestations as heap or scene authority would make residency decide what exists (`.lane/4d-arena/4d-torus-world.md:9-41`; `.lane/4d-arena/4d-content.md:19-31`).

**Derived — slicing is a stage between authoritative world data and ordinary geometry.** The target describes the visible slice as a lossy render derivation that carries slice identity into scene state; it may be discarded and rebuilt and never becomes a collider or world source (`.lane/4d-arena/4d-first-engine.md:33-63`, `.lane/4d-arena/4d-first-engine.md:122-156`, `.lane/4d-arena/4d-first-engine.md:176-182`).

**Derived — the ATW limit defines the hot/cold split.** Motion that preserves the source slice may be corrected in re-projection; phase or hyperplane changes require slice and scene work, so a completed scene may be held but not reinterpreted as another slice (`.lane/4d-arena/4d-first-engine.md:87-118`). Bound movement makes some ordinary locomotion revise phase and inherit that non-warpable path (`.lane/4d-arena/bound-movement.md:104-118`).

**Proposed — a future slice node consumes one versioned world description, explicit slice definition, and time, then publishes generation-handled geometry plus a slice stamp.** This is only a carrier requirement: algorithms, topology policy, physics, content encoding, quotient search, and movement binding remain outside this document.

**Proposed — the 4D target rejects any engine shortcut that assumes geometry is authoritative, every hot pose is warpable, world identity is a resident instance ID, or a scene frame lacks the derivation stamp needed to prove compatibility.** Passing these negative constraints does not validate or adopt the 4D design.

## 13. Costs, failure modes, and rejection gates

**Verified — the heap lab measured 2.81× throughput at 1,048,576 draws while reducing the compared per-frame upload from 16 MiB to 16 bytes, about one million times less.** The lattice document states that this result motivates indirection and separation of data from commands but is neither a portable prediction nor proof for other workloads (`docs/gpu-heap-lattice.md:7-15`).

**Derived — that evidence is the reason to test this architecture, not its promised outcome.** The engine chain spends graph memory, compilation work, extra indirection, transferred buffers, version retention, and more failure states in pursuit of regional traffic and stable bindings; an arena or product workload may reject the trade even though the heap microbenchmark remains true.

### 13.1 Proposal register and gates

**Proposed — every unimplemented construct in this document remains conditional on the corresponding evidence below.**

|Proposal|Construct and cost paid|Required evidence|Rejection gate|
|--------|-----------------------|-----------------|--------------|
|P-01|Forward pure-stage contract; explicit version and time metadata on every boundary|Deterministic replay from equal AST snapshots and time stamps through equal command/table hashes|Any output depends on scheduling, hidden mutable state, or a later-stage value|
|P-02|Many ASTs with producer-local identities and clocks; adapter and schema complexity|Two or more unlike producers publish concurrently without identity collision or shared cadence|A producer loses sole authority, IDs alias, or engine timing changes producer truth|
|P-03|Crossbeam-shaped bounded edit channels; buffers, framing, validation, and return traffic|Adversarial ownership and epoch traces plus drain-wall and starvation distributions|Double ownership, gaps repaired silently, compliant starvation, or PRE-PROJECTION drain misses its guard|
|P-04|One combined resource/frame `StableGraph`; node/edge memory and cycle checks|Equal snapshots yield equal stable topology and plans; cycle, stale-generation, and destroy tests refuse cleanly|A cycle publishes, stable identity revives, or graph overhead exceeds the regional work saved|
|P-05|Dirty-cause propagation and stable topological walk; frontier bookkeeping|Localized edit corpus reports closure, bytes, and wall well below full-graph/table rebuild|Unchanged refresh walks, ordering changes bytes, or representative fan-out becomes effectively global|
|P-06|Two double-buffered DAGs; four graph instances and copy/version-retention cost|Refresh traces prove two drains/swaps, coherent-prefix publication, immutable frame reads, bounded retained memory|Torn reads, half transactions, hard-guard misses, copy domination, or unbounded old versions|
|P-07|Credit-shaped producer API; estimation and per-producer statistics|Mixed-cost producers converge to bounded queue age while the DAG thread performs no ordinary shaping|Oscillation, systematic undercharge, producer cross-starvation, or consumer queueing becomes normal pressure|
|P-08|Refresh-rate hot UBO ring; extra uniform bytes and compatibility checks|In-flight stress proves no overwrite/stall and measures lower late-latch age under the refresh guard|A value changes truth, ring exhaustion is routine, or redraw/refusal cost exceeds latency gain|
|P-09|DAG-to-descriptor/heap/kernel integration; indirection and allocation overhead|Regional-write, bind, generation, scratch-copy, residency, CPU, and GPU comparisons on real workloads|Per-object binding returns, readback returns, stale handles resolve, or indirection loses on the target|
|P-10|Worker/native/single-thread channel lowerings; duplicated wasm memory and transport work|Equal semantic trace across modes plus browser detachment, copy, allocation, shutdown, and wall facts|COOP/COEP becomes required, semantics diverge, a buffer strands, or progress waits without a bound|
|P-11|Typed WebGL2-floor initialization and optional-path equivalence|Minimum conformance scene and refusal fixtures on the capability matrix of EO-06|An admitted device lacks a mandatory path or an optional feature changes meaning|
|P-12|Julibrot seam extraction and tiled value/reconstruction path; dual arms and tile metadata cost|Whole-grid parity first, then tile identity, selection, depth, demand, seam, and palette-change oracles|Existing traces regress, app internals leak through seams, or tiles cost more without meeting freshness/quality gates|
|P-13|Tiered arena handle/heap/DAG migration; temporary duplicate paths and instrumentation|Every A0–A6 tier supplies its named arena-specific reason and preserves fixed-tick replay and pixel oracles|A tier cannot isolate its gain, changes authority, exceeds floor walls, or fails to beat its retained prior arm|
|P-14|Carrier seams for world quotient, slice stage, and warp/scene compatibility; added stamps and node kind|Synthetic higher-dimensional records cross the chain without geometry authority or unsafe warp acceptance|A later stage defines world identity, a slice becomes truth, or incompatible slice stamps can reach warp|

### 13.2 Cross-cutting failure modes

**Derived — `StableGraph` is not evidence of a DAG.** A container choice supplies stable slots but does not prevent cycles, stale edges, or semantically backward dependencies; only admission checks, topological oracles, and typed edge direction establish the contract.

**Derived — double buffering can move rather than remove contention.** Full graph cloning every refresh, long-held reader versions, or catch-up replay touching a wide frontier can cost more memory and wall time than a lock. P-06 rejects the design on measured catch-up and retention, not on nominal lock freedom.

**Derived — coherent-prefix publication can still be stale.** It prevents torn state but does not guarantee freshness; MAIN age and producer-epoch lag must remain visible, and the engine must hold a semantically older complete scene rather than call a growing backlog success.

**Derived — credit can become circular evidence.** If the consumer reports low credit because a producer sent too much, and the producer estimates cost only from that low credit, the system may never discover recovered capacity. Calibration therefore needs bounded probes and separate measured work cost while still keeping overfeed attributable to the producer.

**Proposed — the hot bypass is treated as the highest-risk backward path.** Its speed makes it tempting to place view-dependent gameplay, hit tests, network pose, allocation, or scene membership there; byte-layout tests alone cannot detect that authority error, so reviews and input-flow traces must prove every hot consumer terminates at presentation.

**Derived — a one-pass depth buffer proves only comparable target depth.** It cannot repair descriptors from incompatible slices, stale reference generations, unsupported reconstruction, or missing coverage. Compatibility and validity reject candidates before depth composition (`docs/julibrot/tiled-reprojection.md:162-225`).

**Derived — immutable bindings can conceal mutable lifetime bugs.** A stable bind group does not make a reused descriptor index safe; generation checks, in-flight leases, retirement on wrap, and frame-held versions remain necessary (`docs/gpu-resource-heap.md:45-57`).

**Derived — message passing bounds ownership, not scheduling.** A hidden browser tab or busy worker can delay progress beyond a nominal timer; deadlines bound the engine's reaction once scheduled and require stale hold, cancellation, or typed refusal rather than a claim about unscheduled wall time (`docs/julibrot/worker.md:487-496`).

**Derived — the floor may reject content even when it admits the engine.** Live texture, layer, UBO, and memory limits can be below a content tier's requirements; honesty requires separate engine-capability and content-capacity refusals rather than an expanded device promise (`docs/minimum-requirements.md:13-29`; `docs/gpu-resource-heap.md:23-31`).

**Derived — 4D generality can be faked by widening records.** The target requires an authoritative quotient, explicit lossy slice derivation, and a non-warpable compatibility boundary; extra coordinates without those stage semantics do not demonstrate the carrier contract.

### 13.3 Acceptance posture

**Proposed — thresholds are declared before each evidence run and reported with requested versus delivered values, warm-up policy, sample count, median, p95, maxima where safety matters, and target identity.** A threshold tuned after seeing the result is exploration and cannot accept a construct.

**Proposed — comparisons retain a reference arm until the new arm passes correctness and cost gates on the same workload.** A faster wrong pixel, a lower upload count with more GPU wall, or a smoother median with guard misses is a rejection, not a partial win.

**Proposed — failure removes the smallest responsible construct.** A failed hot class returns to scene rate, a failed arena tier stays on its predecessor, a failed worker lowering selects another lowering, and a failed DAG economics test retains direct compilation; failure does not authorize weakening authority, device, or honesty laws.

**Derived — the design succeeds only if its measurements preserve the chain's asymmetry.** Performance facts may cause an AST producer to publish a new policy edit, but no derived stage may silently change what the AST meant. That is the invariant against which the Julibrot refactor, arena migration, and any later 4D generalization are reviewed.
