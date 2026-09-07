# Engine design — authority to surface

Status: design reference for EO-04; proposed contracts are implementation gates until measured.

Claims use the corpus vocabulary: **Verified** means checked against the cited repository evidence, **Derived** means a consequence of verified facts, and **Proposed** means a design choice that remains subject to its stated rejection measurement.

## 1. The engine is a forward compiler

`ASTs → lock boundary → DAGs → descriptors → heaps → kernels → geo → projection → scene → re-projection → surface`

**Verified — the labelled backlog already defines EC-01 through EC-10 as this chain and EL-02 as its authority law.** It calls the engine a compiler from world descriptions to GPU tables, the render loop its runtime, every arrow a pure function of the preceding stage plus time, and every stage after the AST presentation or prediction rather than truth (`docs/plans/engine-chain-backlog.md:5-34`).

**Proposed — this document is EO-04's executable design reference.** An implementation conforms only when it can identify the input version, explicit time value, output version, ownership transition, bounded work, and rejection result at every arrow; an implicit read of mutable state is an undeclared backward arrow (`docs/plans/engine-chain-backlog.md:46-57`).

**Derived — “pure” describes the semantic transform, not an allocation strategy.** A stage may reuse storage, issue regional GPU writes, or submit work, but the bytes and commands it produces must be determined by the preceding stage's published version plus an explicit time or refresh stamp; memory addresses, queue timing, and which worker happened to run may not change meaning.

**Proposed — time is an input, never a global engine clock.** Every time-sensitive transform names the domain it consumes: a simulation tick, producer epoch, scene deadline, refresh sequence, or presentation time. A stage that needs two domains carries both and their causal identity rather than subtracting unrelated clocks (`docs/state-model.md:29-40`).

**Proposed — no arrow points backward.** Surface visibility, GPU completion, scene quality, descriptor residency, and channel pressure may produce telemetry to the producer that owns the relevant policy, but they may not mutate an earlier stage's truth. Feedback becomes a new AST edit at the responsible producer or remains observation.

**Derived — recomputation and loss are safe to the right of the lock boundary because authority remains left.** A stale scene can cost freshness, a missing descriptor can draw the missing resource, and a rejected warp can hold or clear; none may rewrite world state. This is the same distinction the state model draws between authoritative state, rendered sampling, and presentation (`docs/state-model.md:15-29`, `docs/state-model.md:58-64`).

### 1.1 Stage contracts

|Stage|Consumes|Publishes|May discard or retry|
|---|---|---|---|
|ASTs|Producer-owned intent, state, and private time|Epoch-stamped edit batches|Only under that producer's semantics|
|Lock boundary|Owned edit buffers|An atomic drain snapshot and returned credit|Unpublished producer drafts|
|DAGs|Drain snapshots, prior back graph, explicit time|Versioned dependency graph and dirty frontier|Unpublished back-copy work|
|Descriptors|Topologically ordered dirty nodes|Regional table and instance writes|Superseded regions before publication|
|Heaps|Handles, descriptor records, resource payloads|Stable slots behind immutable bindings|Unreferenced generations|
|Kernels|Dialect programs and heap spans|GPU-resident derived spans|Superseded scratch results before copy|
|Geo|Instance and geometry handles|GPU-resident primitives|Culled or invalid instances|
|Projection|Geometry and explicit view state|Projected primitives and depth|Primitives outside the valid projection|
|Scene|Projected primitives, materials, one scene target|A stamped completed scene|Incomplete or superseded submissions|
|Re-projection|Completed scene and refresh-time hot values|A measured warp, redraw, hold, or clear decision|Unsafe corrections|
|Surface|One completed presentation submission|Visible pixels and presentation facts|Stale tokens and failed generations|

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

**Derived — there is no mutual-exclusion lock in the ownership proof.** The “lock” is the point where an immutable semantic transaction crosses by exclusive buffer ownership. A native mutex around a shared graph would allow producer code to run inside the graph's critical section and would fail this contract even if it happened not to contend.

**Proposed — one producer cannot block another's drain.** Each bounded channel is polled independently in stable producer order with a per-channel maximum accepted byte count learned from credit; a starved or corrupt channel produces facts and refusal for its producer while other channels remain drainable.

**Proposed — boundary measurements record queued buffers, queued bytes, drain wall, oldest epoch age, gaps, stale arrivals, returned buffers, and buffer-starvation events by producer.** Reject the boundary if a producer obeying its credit can exhaust the pool, if drain p95 breaks the PRE-PROJECTION guard, or if any interleaving changes the accepted epoch sequence.

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

**Proposed — one DAG thread owns PRE-PROJECTION and MAIN, and each DAG has a front and a back `StableGraph`.** There are four graph instances, never one graph guarded for readers. The front is immutable for its published version; only the owner mutates the back.

**Proposed — PRE-PROJECTION is the hard-deadline graph.** It contains structural choices needed at surface refresh, including which handles are hot, their descriptor locations, warp inputs, UI/viewmodel membership, and required motion records. At every refresh its sequence is `drain → apply admitted transactions → swap → emit hot descriptor metadata → warp`.

**Proposed — MAIN is the budgeted scene graph.** It receives scene-rate world, resource, and pass edits, applies coherent transactions in stable order until the safety deadline guard would be crossed, and carries the unapplied suffix forward. The producer's credit should make the guard exceptional; the owner does not invent a second pacing policy.

**Proposed — every surface refresh performs two independent drains and two swaps.** PRE-PROJECTION drains and swaps before warp; MAIN drains and swaps at its scene-work boundary so a coherent applied prefix becomes available for subsequent scene submission. An empty drain still advances refresh observation but may publish the same graph version with a new refresh fact rather than clone data.

**Proposed — a graph swap exchanges front and back roles after the back represents a coherent prefix.** Before subsequent mutation, the new back is brought to the published baseline by structural sharing or measured copy-on-write; an implementation may choose the mechanism only if front immutability and stable identities remain true.

**Derived — partial MAIN application is safe because no reader sees the graph being changed.** Transactions are atomic within the back copy, the swap exposes only the last coherent prefix, and the ordered suffix remains owned for later application. Partial does not mean a node record, descriptor write, or edge set can be visible half-formed.

**Proposed — publication uses an arc-swap-shaped versioned pointer carrying graph kind, publication epoch, included producer epochs, graph generation, and descriptor-plan identity.** “Arc-swap-shaped” specifies atomic whole-version replacement and retained lifetime; browser and single-thread lowerings may implement the same semantics without shared-memory atomics.

**Proposed — a reader loads exactly one published version and holds it for a whole frame.** Scene encoding, descriptor resolution, kernel scheduling, geometry, and projection for that frame use the same MAIN version; warp reads one PRE-PROJECTION version and the one scene version stamped on its source. Reloading mid-frame is a torn-frame bug.

**Proposed — publication epochs are monotonic per graph and carry a producer-epoch vector.** Equality of publication epochs means identity of the published graph, while producer epoch comparison answers freshness. A single scalar is insufficient to claim that independently clocked ASTs are mutually current.

**Proposed — hard and soft limits are measured separately.** PRE-PROJECTION records drain, apply, emit, and guard margin against each refresh; MAIN records admitted credit, coherent transactions applied, suffix depth, wall consumed, and budget remaining. Reject the four-graph design if copying dominates the saved work, readers retain versions until memory is unbounded, PRE-PROJECTION misses its guard, or MAIN age grows while compliant producers receive positive credit.

## 6. Credit is the producer API

**Verified — the Julibrot channel already returns consumer facts in transferred buffers and leaves shaping at the producer.** Its owner reports credit but never delays, rejects, or coalesces edits for the producer; the same-thread and worker endpoints expose one transport-independent state machine (`docs/julibrot/worker.md:123-153`, `docs/julibrot/worker.md:313-349`).

**Proposed — every returned edit buffer carries the next credit plus the consumer's apply wall, budget remaining, highest epoch applied, queue depth, and refusal counters for that producer.** These facts describe observed capacity; they are not promises that the next edit has zero cost.

**Proposed — credit is denominated in estimated consumer work and capped bytes, not message count alone.** Each edit opcode has a conservative learned cost charged by touched nodes, expected forward fan-out, and payload bytes; the return path updates the estimate from measured application without allowing a single underestimated edit to exceed the absolute safety cap.

**Proposed — producers own coalescing.** Before transfer, each AST may replace superseded value edits, collapse create-update chains, combine adjacent regions, or choose a lower semantic rate when its own rules allow. It must preserve transactions, epochs, destroys, cross-object ordering, and any edit whose omission changes truth.

**Proposed — producers size the next batch to returned credit.** Zero credit means retain and coalesce locally; positive credit bounds the next offered work. A producer may send a correctness-critical transaction above credit only through a separately counted emergency class with an absolute byte wall, never by relabelling routine churn.

**Proposed — the DAG thread performs no coalescing, throttling, sleeps, token-bucket delay, or clock slicing.** It drains the already-shaped buffers, applies complete transactions in order, and stops MAIN only at the safety deadline guard. PRE-PROJECTION rejects or defers a buffer that cannot fit its hard boundary rather than beginning an unsafe transaction.

**Derived — overfeeding is a producer-side bug.** The consumer reports offered work, admitted work, applied wall, guard stops, epoch age, and returned credit under that producer's identity; the engine does not hide a producer's excess by making every other AST wait.

**Proposed — initial credit is deliberately small and calibration is monotone-safe.** A new producer proves cost with bounded batches, credit grows only from sustained margin, and any guard stop cuts its next credit immediately. Reject the API if compliant producers oscillate without converging, if cost prediction systematically understates fan-out, or if owner-side queueing becomes the normal backpressure mechanism.

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

## 9. Concurrency lowerings

## 10. The device floor

## 11. The Julibrot as the first chain miniature

## 12. Arena migration and the 4D target

## 13. Costs, failure modes, and rejection gates
