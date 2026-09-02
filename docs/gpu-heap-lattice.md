# GPU heap lattice architecture

Status: proposed implementation contract for a WebGL2-only heap-lattice lab; this document defines the architecture, evidence plan, and implementation order, but reports no new browser measurement.

## 1. Decision and evidence boundary

The prize is not edges per millisecond: the dated field replay below and the earlier competition summary in [GPU resource heap §1](gpu-resource-heap.md#1-what-this-is-and-is-not) show a raster-bound workload with only about a 1.8-times throughput spread across four materially different architectures, so a large future frames-per-second win is the first result to distrust and re-run for unequal work, missing fences, or changed raster coverage.

On 2026-09-02, visible replay on the reference Intel integrated GPU under ANGLE in Firefox recorded rawgl at `(7,7,5,5,5)`, 18,375,000 submitted edges, and 500 ms; layer at `(5,3,3,3,3)`, 1,215,000 edges, and 35 ms; pure at `(3,3,3,3,3)`, 729,000 edges, and 30 ms; and min at `(3,3,3,3,1)`, 243,000 edges, and 12 ms.

Those observations derive 36,750, about 34,714, 24,300, and 20,250 edges per millisecond respectively, with `36,750 / 20,250 = 1.8148…`; the frame times are measurements tied to that dated visible replay, while the rates and ratio are arithmetic derived from them.

The 2026-09-02 visible replay of the prior heap page at revision `90ec390` recorded a 2.81-times many-draw advantage at 1,048,576 draws, 16 bytes versus 16 MiB of per-frame CPU-to-GPU writes, and an approximately 2.18-microsecond per-draw dispatch floor; these are motivation for handle-based binding and a warning that draw count remains expensive, not predictions for this lattice.

The new lab answers architecture and capacity questions while holding the rendered object, submitted edge definition, ladder tuple, overlay, timing method, and WebGL2 substrate fixed.

## 2. Ladder and invariant workload

The object is the centered five-dimensional hypercubic lattice of one 1,200-vertex 120-cell prism per copy, spacing `s = 8`, and exactly 3,000 submitted edges per copy; an edge rejected at either perspective pole remains submitted and counted even though its box is clipped.

The ladder has 113 steps numbered 0 through 112: step 0 is `(0,0,0,0,0)`, step 1 is `(1,1,1,1,1)`, and each later step adds two to one odd axis count in round-robin order `e₁, e₂, e₃, e₄, e₅`.

For step `n > 0` and zero-based axis `a`, the count is `k_a(n) = 1 + 2 × floor((n - 1 + 4 - a) / 5)`; the implementation in `lane/comp-min:crates/labs/min/src/math.rs` fixes step 111 at `(45,45,45,45,45)` and step 112 at `(47,45,45,45,45)`.

For tuple `k = (k₁,…,k₅)`, copies are `C(k) = Πᵢ kᵢ` and requested submitted edges are `E(k) = 3,000 × C(k)`; the top therefore has `C = 47 × 45⁴ = 192,729,375` copies and `E = 578,188,125,000` requested edges.

The standing geometry invariants are 600 vertices in the regular 120-cell cap, 1,200 cap edges, 1,200 vertices in the prism, 3,000 prism edges, edge length `3 − √5`, and cap circumradius `2√2`; construction and every lowering must preserve them rather than accepting a visually similar substitute.

## 3. Algebra decides the passes

Let a base vertex be `v ∈ ℝ⁵`, let a copy center be `c(k) = s × Σᵢ₌₁⁵ kᵢeᵢ` for centered signed mixed-radix digits `kᵢ`, and let the time-dependent five-dimensional rotation be the linear map `R(t)`.

Linearity gives the pass boundary exactly: `R(t)(v + c(k)) = R(t)v + R(t)c(k) = R(t)v + s × Σᵢ₌₁⁵ kᵢ(R(t)eᵢ)`.

The compute stage therefore rotates only the 1,200 base vertices per frame and writes their five rotated coordinates; the CPU evaluates the same five constant basis vectors through `R(t)` once per frame and places the resulting 25 floats in the frame uniform, so no scale-dependent basis heap region or ten repeated basis fetches enter a vertex invocation.

The 25 basis floats occupy seven `vec4` uniform lanes: five lanes hold coordinates one through four of `R(t)eᵢ`, and two lanes hold the five fifth coordinates plus three explicit padding floats, for 112 bytes with 12 bytes of padding.

The complete `FrameUniform` is 192 bytes: 16 bytes of rotation coefficients, 16 of projection and spacing values, 16 of render values, 16 of the first four lattice counts, 16 of fifth-count, fifth-range, yaw, and pitch values, followed by the 112-byte rotated-basis block; this is one CPU write per rendered frame in every heap mode.

Projection is deliberately not moved across the equality because perspective is not linear: `P₅(x) = (d₅ / (d₅ − x₅)) × (x₁,x₂,x₃,x₄)` and `P₄(y) = (d₄ / (d₄ − y₄)) × (y₁,y₂,y₃)`, and their coordinate-dependent denominators make `P(a+b)` unequal to `P(a)+P(b)` in general.

For one Mode A endpoint, the vertex-stage algorithm loads the rotated first four coordinates and rotated fifth coordinate through two handle-resolved accessors, evaluates `R(t)v + s × Σᵢ kᵢ(R(t)eᵢ)` as five integer-weighted vector contributions, performs the fifth- and fourth-axis perspective divisions, and tests both denominators against the declared positive epsilon.

One box-vertex invocation additionally loads the edge endpoint-index pair once, so its exact Mode A data path is five accessor calls: one edge record and two rotated records for each of two endpoints; each accessor performs one descriptor-table lookup and one DATA-array `textureLoad` when its span has one page.

The logical edge algorithm evaluates 6,000 endpoints per copy because 3,000 edges name two endpoints and each of 1,200 vertices is shared by five edges; a literal 36-vertex box draw invokes the endpoint code again for every box vertex, so the implementation must report vertex invocations separately and must not present 6,000 as the physical shader-invocation count.

Mode B is the reuse control: its compute stage rotates and projects each of the 1,200 vertices once per copy, after which the vertex stage fetches the materialized endpoint records and builds the same boxes without the five-term add or either perspective divide.

## 4. DATA records, spans, and walls

Every DATA record is one nearest-sampled unfiltered RGBA32F texel and therefore exactly 16 bytes; five-dimensional values use two same-length structure-of-arrays spans rather than an overloaded sentinel or lossy packing.

Static base slot `base_four[j]` is `[x₁,x₂,x₃,x₄]` and `base_fifth[j]` is `[x₅,0,0,0]`, so 1,200 base vertices consume `1,200 × 2 × 16 = 38,400` logical bytes: 24,000 bytes hold the 6,000 meaningful coordinates and 14,400 bytes are zero padding imposed by the second RGBA record.

Static edge slot `edge[j]` is `[endpoint_a,endpoint_b,0,0]` represented as exactly integral floats below 1,200, so 3,000 pairs consume `3,000 × 16 = 48,000` logical bytes; static shared DATA is therefore 86,400 logical bytes and is uploaded once.

Mode A output uses `rotated_four[j] = [r₁,r₂,r₃,r₄]` and `rotated_fifth[j] = [r₅,0,0,0]`, exactly `1,200 × 2 × 16 = 38,400` mutable logical bytes and 124,800 logical bytes including static shared DATA, independent of copy count.

Mode B output uses `projected[j] = [p₁,p₂,p₃,r₅]` and `projected_meta[j] = [valid,0,0,0]`, exactly 32 bytes per projected vertex; the separate validity record wastes 12 bytes per vertex but keeps finite coordinates, fifth-axis hue, and rejection state independently testable without NaN or sign-bit conventions.

A physical heap descriptor still describes one region in one DATA array layer, while a `DataSpan` is the typed logical allocation `{logical_len, pages}` whose ordered page handles may occupy non-contiguous regions and layers; this preserves the existing 20-bit descriptor-index and 12-bit generation handle ABI while lifting a logical slot beyond one two-dimensional texture.

The one immutable bind group adds a fixed-capacity span-directory UBO beside the descriptor UBO and frame/dispatch uniforms; page handles enter that directory only at allocation or step setup, never per frame, and its runtime capacity is derived from `max_uniform_buffer_binding_size` after fixed metadata is charged.

An input accessor maps its logical index to a page through its `DataSpan`, resolves that page handle through the descriptor table, and computes local x and y from that selected page descriptor's width and height; no output-grid dimension participates in input decoding.

Bulk spans use full free buddy blocks when possible and may finish with a smaller block, but physical reservation is always reported as `Σ page_class² × 16`, not as logical bytes; allocator planning, descriptor availability, span-directory capacity, fragmentation, configured heap side, created layer count, and driver allocation success all remain visible walls.

For two equal-length MRT output spans, the planner performs an atomic dry run for both spans and returns the greatest whole-copy count it can place without mutating the heap; delivery is then `min(requested copies, planned heap copies, address copies, draw copies)`, and a later driver refusal is displayed as a runtime refusal rather than converted into a smaller unreported workload.

The implementation queries texture dimension, array-layer count, UBO size, attachment count, sampled-texture count, buffer limits, and RGBA32F render/sample usage, then displays the actual created DATA side, layers, bytes, free buddy classes, descriptors, directory entries, and every term of the delivery minimum.

WebGL2 exposes no trustworthy total free-VRAM query, so a configured DATA byte budget is clamped by exposed dimensions and layers before creation; that configured cap, the successfully created allocation, and later allocation failures are distinct reported facts.

## 5. Mode arithmetic

Let `L_A = 86,400 + 38,400 = 124,800` be Mode A logical heap bytes and let `L_B(k) = 86,400 + 1,200 × C(k) × 32 = 86,400 + 38,400C(k)` be Mode B logical heap bytes; these formulas exclude buddy padding and page-directory metadata, which are runtime physical facts.

The old layer comparator materializes two RGBA32F edge-pose slots, `[midpoint_x,midpoint_y,midpoint_z,hue]` and `[direction_x,direction_y,direction_z,length]`, so its logical payload is `L_layer = E × 32`, while its actual one-texture-per-slot allocation is `P_layer = ceil(√E)² × 32`; one slot alone is `ceil(√E)² × 16`, which is the ceiling the multi-layer heap is intended to remove.

|Rung|Copies `C`|Submitted edges `E`|Mode A logical DATA|Mode B mutable output|Mode B total logical DATA|Layer one square slot|Layer two-slot allocation|
|----|---------:|------------------:|------------------:|--------------------:|------------------------:|--------------------:|------------------------:|
|`(1,1,1,1,1)`|1|3,000|124,800 B|38,400 B|124,800 B|48,400 B|96,800 B|
|`(3,3,3,3,3)`|243|729,000|124,800 B|9,331,200 B|9,417,600 B|11,669,056 B|23,338,112 B|
|`(7,7,5,5,5)`|6,125|18,375,000|124,800 B|235,200,000 B|235,286,400 B|294,053,904 B|588,107,808 B|
|`(47,45,45,45,45)`|192,729,375|578,188,125,000|124,800 B|7,400,808,000,000 B|7,400,808,086,400 B|9,251,014,236,304 B|18,502,028,472,608 B|

Every number in this table is arithmetic: for example, `(7,7,5,5,5)` gives `C = 7 × 7 × 5³ = 6,125`, Mode B mutable bytes `= 6,125 × 1,200 × 32 = 235,200,000`, and layer side `= ceil(√18,375,000) = 4,287`.

The table is not a promise that any row is allocatable or drawable; the page computes delivered copies and edges from the live heap plan, `u32` kernel index space, the gles draw-instance policy wall, and any validation refusal while leaving the requested tuple selected.

## 6. Mode A: algebraic heap path

At scene setup, Mode A uploads the two base-coordinate spans and the edge span once, allocates the two 1,200-record rotated-output spans once, registers one rotation kernel, and creates the immutable heap bind group.

Each frame writes the 192-byte frame uniform, dispatches the 1,200-element rotation kernel through one two-attachment fragment pass, and renders one instanced box draw with `instance_index / 3,000` as copy and `instance_index % 3,000` as base edge.

The compute output is GPU-resident and no readback gates rendering; queue order makes the fragment-compute write visible to the following vertex texture loads, while an explicit mapped fence exists only around requested measurements.

The lattice center is decoded from the odd tuple in mixed-radix order, converted to centered integer digits, and combined with the five CPU-rotated basis vectors; changing the rung changes only uniforms and the requested instance count, never heap allocation.

The Mode A delivery wall is `min(E_requested, E_draw)`, where `E_draw` is computed from the wgpu `u32` instance range and the configured conservative gles instance ceiling exposed in page facts; the implementation must not attempt the 578-billion-edge top in one draw merely because the control can request it.

## 7. Mode B: materialized projected vertices

Mode B keeps the same static base and edge spans but allocates paired projected and metadata spans for `1,200 × delivered copies` records, then dispatches a kernel whose output index decodes copy and base vertex, rotates, translates, double-projects, and writes the two MRT fields.

For each output page pair, one ordered submission writes a 16-byte dispatch header containing global base and valid length and runs one fragment pass into one-layer D2 views of the DATA array; separate submissions keep each header write ordered before its pass without dynamic offsets or per-page bind groups, and page handles and page geometry were uploaded at step setup.

Mode B per-frame CPU-to-GPU bytes are therefore `192 + 16P` for `P` output page pairs, while Mode A uses `192 + 16 = 208`; the page reports those actual bytes and pass count rather than describing multi-layer capacity as free.

The Mode B vertex stage loads one edge record and two materialized records per endpoint, five accessor calls per box-vertex invocation, then builds the same box and hue without lattice addition or perspective division.

The Mode B copy wall is `min(C_requested, floor(H_pair / 1,200), floor(I_kernel / 1,200), floor(E_draw / 3,000))`, where `H_pair` is the exact paired-span allocator plan in records, `I_kernel` is the dialect's `u32` index ceiling, and `E_draw` is the displayed effective instanced-draw ceiling.

Step changes may free and replace Mode B spans, but allocation never occurs per frame; a failed larger request leaves the requested control at that rung, renders the greatest planned whole-copy delivery immediately when nonzero, and prints requested and delivered tuples, copies, vertices, edges, bytes, pages, and the limiting term.

## 8. Kernel dialect v2 over heap handles

The author-visible body remains entry-point-free WGSL declarations plus `fn kernel(index: u32, uniforms: UniformType) -> ResultType`; the author declares stable kernel name, uniform type and nonzero 16-byte-multiple size, accessor names, result field names, and an output count from one through four.

Every declared input produces the author-visible signature `fn load_name(index: u32) -> vec4<f32>`; access outside that input span's logical length returns canonical zero, so padding and absent pages never become observable data.

Registration receives names and shapes but no resource handles, builds accessor stubs, parses the body, rejects forbidden IR, assembles the fullscreen-triangle vertex and fragment entry points, maps result fields to MRT locations, validates with Naga, and creates the pipeline once.

The forbidden constructs remain module-scope workgroup variables, atomic types or operations, barriers including workgroup-uniform loads, and raw storage resource declarations; kernel bodies also cannot declare bindings or entry points because all resources and entry points belong to the generated lowering.

Dispatch supplies ordered input `DataSpan` references, ordered output `DataSpan` references, and exact uniform bytes; it binds no new bind group, validates handles and spans on the CPU, writes only uniform metadata, selects output layer views as render attachments, and sets the one immutable heap bind group.

The logical index space comes from the first output span's `logical_len`; every other output must have the same length and matching page geometry for each MRT pass, the generated fragment entry computes `index = page_base + local_y × output_page_width + local_x`, and the last page discards indices at or beyond logical length.

Each generated input accessor independently selects its input page and derives coordinates from that page descriptor's own width and height, which removes v1's addressing-bug class structurally: changing the output grid can no longer change how an input index is decoded.

Version 1 froze texture views and accessor bindings into a bind group at registration and made one physical texture per slot the allocation unit; version 2 freezes only code, layout, and pipeline, while dispatch-time span handles choose data behind the already-bound heap arrays and logical slots may cross layers.

Registration errors are the typed set `InvalidDescriptor`, `Parse`, `Forbidden(WorkgroupVariable|Atomic|Barrier|RawStorageAccess)`, and `Validation`; each carries the stable kernel name and exact diagnostic.

Dispatch and resource errors are typed as `WrongDevice`, `InvalidHandle`, `StaleHandle`, `WrongHeapKind`, `EmptySpan`, `SpanDirectoryFull`, `SpanPageMissing`, `SpanGeometryMismatch`, `OutputLengthMismatch`, `OutputAlias`, `ReadWriteAlias`, `UniformSizeMismatch`, `IndexSpaceOverflow`, `ViewportLimit`, `HeapCapacity`, `Pipeline`, `DeviceLost`, `StaleGeneration`, `Deadline`, and `Mapping`.

`OutputAlias` rejects two MRT fields targeting the same page, `ReadWriteAlias` rejects sampling any page rendered by the same pass, and generation checks occur on every CPU-side resolve in debug builds before views or directory entries are used.

## 9. Outputs land in heap layers

The first implementation risk is unpaid: wgpu 24 on the gles backend must permit an RGBA32Float `TEXTURE_2D_ARRAY` created for sampling, copying, and render attachment use to expose a one-layer D2 view as a fragment output and later sample that layer through the array view.

RGBA32Float is not blendable, so every compute color target uses no blend state and `ColorWrites::ALL`; blending is neither requested nor used as an accidental capability test.

The implementation round's first commit is a standalone golden spike before any lattice kernel: create a small DATA array, reserve a known descriptor at a known layer and origin, render one invocation that writes `[0.25,-0.5,1.5,7.0]`, copy the target layer region to a mapped buffer with padded rows, drive mapping through `device.poll` in a browser-yield loop, and require the four exact binary-representable f32 values.

The golden also samples the written record through the immutable array view into a second readback target, so it proves both render-to-layer and later array-layer lookup rather than proving only a copy path.

Failure is a typed `RenderToArrayLayerUnsupported` capability refusal containing selected adapter, backend, format usages, view dimensions, layer, origin, and scoped wgpu validation text; implementation stops there instead of silently allocating standalone textures.

## 10. Render path

Both heap modes use one instanced draw and the same static 36-vertex long-box mesh, surface format, depth target, camera, thickness, lighting, no mipmaps, and `cull_mode: None`; only endpoint production changes.

The vertex stage derives copy and edge exclusively from `instance_index`, loads the fixed endpoint pair, resolves the selected mode's endpoint spans by handle, rejects the box when either endpoint is invalid, and otherwise constructs the same midpoint, direction, length, side, up, clip position, and normal.

Hue uses the midpoint post-rotation fifth coordinate normalized by a symmetric lattice-extended fifth range computed from the current tuple, rotated basis, base extent, and spacing; the CPU reference and both modes must agree before timing is reportable.

Static heap contents and span-directory records change only at initialization or step setup; dynamic uploads are the frame uniform and compute dispatch headers, and their exact bytes are included in every result.

## 11. Substrate posture

The initial and implementation target is wgpu 24 with `wgpu::Backends::GL` over WebGL2 only; the page explicitly requests GL and neither autodetects nor prefers browser WebGPU.

Handle encoding, descriptor meaning, `DataSpan` indexing, kernel-body syntax, typed errors, and requested-versus-delivered semantics avoid WebGL-specific author behavior so a future WebGPU lowering can preserve kernel bodies and handle semantics, but no WebGPU design, measurement, promise, or empty leaked-differences ledger belongs in this contract.

A leaked-differences ledger is opened only when another physical lowering exists and has field evidence to populate it.

## 12. Layer comparator

The future lab carries a private faithful copy of the old layer path from `lane/comp-layer` rather than changing the engine or importing a mutable sibling lab; its v1 generated source, square-slot arithmetic, golden checksum, and CPU kernel oracle pin comparator fidelity.

At each selected ladder step, Mode A, Mode B, and layer receive the identical tuple, geometry, 3,000-edge-per-copy submitted count, pole epsilon, animation time, camera, render target, box mesh, and overlay fields; clipped edges remain included in submitted counts in all paths.

The comparator reports requested and delivered tuple, copies, vertices, submitted edges, shown edges, compute passes, render draws, frame median and p95, microseconds per submitted edge, CPU-to-GPU bytes per frame, logical heap bytes, physical heap bytes, layer one-slot and two-slot bytes, limiting wall, and the full wall arithmetic.

Fair timing uses the same timer-quantum probe, three warmups, 15 samples, and repeat-until-32-observed-quanta batching scheme of record from `crates/what-is-this/src/kernels.rs`, with the same ordered four-byte mapped fence after the final presentation work.

The first timed frame is measured alone; if it exceeds the 100 ms animation threshold it is retained as the single on-demand observation and is not repeated, while faster rungs enter the adaptive series and all modes share the same rule.

Primary time begins before CPU uniform writes and command encoding and ends after the mapped completion fence; GPU timestamp values appear only if the GL path actually exposes and uses them, labeled separately and never substituted for primary wall time.

The gles backend requires `map_async` progress through `device.poll` in a zero-timeout browser-yield loop; every wait has a finite deadline and generation guard, and `Queue::on_submitted_work_done` is not used because the four-byte MAP_READ fence is the single contracted completion path.

## 13. Questions, claims, and oracles

Claim A is that compute output can remain GPU-resident from fragment-compute production through geometry consumption on WebGL2; its oracle is the render-to-array-layer golden plus an end-to-end frame whose sampled endpoints and image checksum match the CPU reference without a readback between passes.

Claim B is that handle indirection has a measurable price in a real vertex-fetch path; its oracle is a diagnostic Mode A direct-descriptor control that supplies the same physical layer/origin/extent in uniforms, uses the same DATA array, projections, draw, fence, and pixels, and differs only by omitting descriptor-table lookup, with delta reported in frame time and microseconds per edge.

Claim C is that heap spans lift layer's one-texture-slot capacity wall for Mode B; its oracle is the greatest whole-copy rung admitted by each runtime arithmetic plan and then successfully allocated, with heap aggregate bytes and layer square-slot bytes printed beside any refusal.

Claim D is that Ember's fixed mesh-ID convention can migrate to handles without changing simulation authority; its oracle is a lab-only synthetic registry mapping stable IDs to generation-checked handles, proving lookup, stale rejection, fallback, and static instance transport, and its result is an implication for later engine design rather than authorization for engine edits.

No claim is established by a faster picture alone: conformance, submitted counts, byte accounting, completion ordering, and pixel or numeric oracles must pass before a timing series is eligible for comparison.

## 14. Page and measurement plan

One self-contained page exposes the 113-rung control, Mode A, Mode B, layer, and the Mode A direct-descriptor diagnostic, with one common canvas and one common facts/results overlay; the deployed JavaScript and wasm URLs use a versioned `module_or_path` on every redeploy.

Selecting any rung or mode immediately attempts its runtime plan and renders the delivered work without measurement admission; controls never snap to the delivered rung, and zero delivery is a visible refusal rather than the previous frame mislabeled as current.

Every selection resets samples, p95 state, timer batches, frame counters, shown counts, and generation tokens; an older allocation, map callback, or measurement can neither publish into nor restore a newer selection.

Animation continues only while the latest measured frame remains at or below 100 ms; above that threshold the page becomes single-frame-on-demand, measures exactly one requested frame at a time, yields before and after it, and displays its true wall time even when it takes seconds.

The timer probe performs at most 4,000,000 consecutive `performance.now()` reads, stops after 32 positive transitions or 500 ms, and uses the smallest positive transition as the observed quantum; unresolved timers make timing unavailable without preventing rendering.

Adaptive samples increase whole-workload repeats until a batch spans 32 observed quanta, cap the batch target at 250 ms and repeats at 4,096, normalize by repeats, and stop a mode after a finite 30-second suite budget; medians use the middle sorted sample and p95 uses nearest-rank rank `ceil(0.95n)`.

Counts are literal: requested is the tuple-derived count, delivered is the runtime-wall count, submitted is the draw instance count, shown excludes pole-discarded boxes, and measured is the work enclosed by the fence; none is substituted for another.

## 15. Test plan

Algebra tests generate deterministic base vertices, centered digits, times, and tuples and compare direct `R(v+c)` with `Rv+sΣkᵢReᵢ` in f64 and the shipped f32 order, including zero, negative digits, the four named rungs, and near-pole inputs.

Projection tests prove the two denominators, epsilon validity, fifth-coordinate hue input, and an explicit counterexample to projection linearity against a CPU reference.

Heap tests cover paired-span planning across layers, page-directory capacity, descriptor generation reuse, input indices crossing page boundaries, last-page padding returning zero, atomic two-output rollback, buddy fragmentation arithmetic, alias rejection, and requested-versus-delivered wall strings.

The render-to-layer golden is the mandatory first implementation test and proves exact write, copy readback, and array sampling at a nonzero layer and nonzero origin.

Dialect tests parse and validate author bodies with generated accessor stubs, exercise every forbidden construct and typed descriptor failure, prove dispatch-time handle replacement changes data without pipeline replacement, and reproduce v1's unequal input/output-width case to show each accessor follows its own descriptor.

Kernel conformance compares Mode A rotation, Mode A endpoint reconstruction, Mode B materialization, validity, hue, and box pose against CPU reference values at deterministic indices and tuples.

Geometry tests pin 600 cap vertices, 1,200 cap edges, 1,200 prism vertices, 3,000 prism edges, edge length `3−√5`, circumradius `2√2`, step count 113, step 111 `(45,45,45,45,45)`, step 112 `(47,45,45,45,45)`, and top edge arithmetic 578,188,125,000.

Page-contract tests pin explicit GL backend selection, versioned loader paths, immediate rendering without admission, stable requested controls, generation cancellation, sample resets, bounded waits, single-frame fallback, and exact median, p95, and byte formulas.

Comparator tests run identical small rungs through heap and layer kernels, require matching numeric and image checksums within a declared f32 tolerance, and reject timing publication when tuples, delivered edges, or fence placement differ.

## 16. Implementation phases and line budget

Phase 0, required first commit, is the render-to-array-layer golden spike and capability report, estimated at 220 new Rust, WGSL, and test lines.

Phase 1 adds `DataSpan`, paired allocator planning, page-directory metadata, runtime wall arithmetic, and generation checks without engine changes, estimated at 420 lines.

Phase 2 implements dialect v2 registration, generated accessors, dispatch validation, page passes, typed errors, and conformance fixtures, estimated at 500 lines.

Phase 3 implements invariant geometry data, the 113-step ladder, the 192-byte frame uniform, Mode A rotation and rendering, and CPU oracles, estimated at 420 lines.

Phase 4 implements Mode B materialization, the private layer comparator, the direct-descriptor diagnostic, and equal-work checksum gates, estimated at 480 lines.

Phase 5 implements the self-contained WebGL2 page, versioned loader, measurement state machine, overlays, requested-versus-delivered presentation, and cancellation, estimated at 420 lines.

Phase 6 is test completion, lint repair, bundle evidence, and contract reconciliation, estimated at 260 lines; the total planning estimate is 2,720 new lines, and any phase exceeding its estimate by more than 25 percent requires a reported reason rather than hidden compression.

## 17. What does not change

Renderer austerity remains no mipmaps, `cull_mode: None`, one presentation pass, and no decorative pass; fragment-compute passes exist only to produce the contracted GPU-resident data.

Heap and compute contents feed presentation and prediction only and cannot author simulation, collision, protocol state, reconciliation, or other gameplay truth; missing or stale data produces fallback pixels or a typed error.

The one heap bind group is created at initialization and its resource identities remain immutable; heap contents, descriptors, span directory, and uniforms may change only through the contracted regional writes and dispatch sequence.

The page reports requested, delivered, submitted, shown, and measured quantities separately, computes walls from live facts and fixed type ceilings, performs no measure-first admission, preserves controls across refusal, guards every asynchronous publication by generation, and never waits without a deadline and browser yield.

Browser numbers remain `requires visible replay` until a visible replay supplies them, and arithmetic is labeled arithmetic with its formula.

## 18. Unresolved risks

Render-to-array-layer plus later sampling is unproved on the target gles backend and is intentionally the first implementation spike rather than an assumption buried under lattice code.

The effective maximum safe DATA allocation is not exposed as free VRAM, so configured byte budget, successful texture creation, allocator capacity, and driver allocation failure cannot be collapsed into one predictive number.

Multi-page Mode B requires one fragment pass and dispatch-header write per page pair; pass count and state-diff cost may erase any capacity benefit as a throughput benefit, and capacity rather than speed is the claim until measured.

The span-directory UBO introduces a second finite metadata wall and uniform dynamic-indexing cost; both require runtime facts and a handle-versus-direct measurement.

Mode A removes repeated rotation but not projection or the literal 36-box-vertex repetition of endpoint fetches and projection, so its algebraic reduction may be invisible under raster cost or vertex invocation count.

Mode B's two-record validity layout deliberately spends 12 padding bytes per vertex; a later packed validity scheme could move its capacity wall but would require new numeric and performance evidence.

The top ladder requests far more records and draw instances than the dialect and gles draw ranges can deliver, so it primarily tests honest wall arithmetic and control stability rather than rendering 578 billion edges.

The old layer comparator is copied into an isolated lab rather than shared, which risks drift; generated-source snapshots, square-slot arithmetic, golden checksum, and small-rung image agreement are required to keep it faithful.

CPU-computed rotated basis values and GPU-computed rotated base vertices can differ by operation order or transcendental implementation, so the f32 algebra tolerance and hue range must be fixed before timing.

No visible browser result yet identifies the crossover between Mode A, Mode B, pure, and layer, and the raster-bound evidence makes a clean crossover uncertain rather than guaranteed.
