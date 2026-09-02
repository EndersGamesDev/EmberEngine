# GPU heap lattice architecture

Status: shipped WebGL2-only heap-lattice contract through the v7 rawgl-presentation repair; paid browser evidence selects SCRATCH-to-DATA copy, while optional Mode B is deferred and its design remains recorded.

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

The complete `FrameUniform` is 192 bytes: 16 bytes of rotation coefficients, 16 of projection and spacing values, 16 of presentation padding, aspect, presentation padding, and rotation time, 16 of the first four lattice counts, 16 of fifth-count, rawgl hue extent, and two former-camera padding values, followed by the 112-byte rotated-basis block; this is one CPU write per rendered frame in every heap mode.

Projection is deliberately not moved across the equality because perspective is not linear: `P₅(x) = (d₅ / (d₅ − x₅)) × (x₁,x₂,x₃,x₄)` and `P₄(y) = (d₄ / (d₄ − y₄)) × (y₁,y₂,y₃)`, and their coordinate-dependent denominators make `P(a+b)` unequal to `P(a)+P(b)` in general.

For one Mode A endpoint, the vertex-stage algorithm loads the rotated first four coordinates and rotated fifth coordinate through two handle-resolved accessors, evaluates `R(t)v + s × Σᵢ kᵢ(R(t)eᵢ)` as five integer-weighted vector contributions, performs the fifth- and fourth-axis perspective divisions, and tests both denominators against the declared positive epsilon.

One box-vertex invocation additionally loads the edge endpoint-index pair once, so its exact Mode A data path is five accessor calls: one edge record and two rotated records for each of two endpoints; each accessor performs one span-directory lookup, one descriptor-table lookup, and one DATA-array `textureLoad` when its span has one page.

The logical edge algorithm names 6,000 endpoints per copy because 3,000 edges name two endpoints and each of 1,200 vertices is shared by five edges, but the rendered box is obligatorily indexed with eight unique box vertices and 36 indices in every mode and comparator, preserving identical triangles while allowing the post-transform cache to reduce endpoint work.

The arithmetic ideal is `8E` unique box-vertex invocations and `36E` submitted indices per frame; eight invocations per edge is a post-transform-cache expectation rather than a guaranteed counter value, so the page labels `8E` as ideal arithmetic, reports `36E` as submitted index arithmetic, and treats measured frame wall as fact.

Mode B is the projected-vertex reuse control: its compute stage rotates and projects each of the 1,200 vertices once per copy, after which the vertex stage fetches the materialized endpoint records and builds the same indexed boxes without the five-term add or either perspective divide.

Mode C is the allocation-and-indirection control: it runs layer's exact edge-pose kernel and stores two records per submitted edge, `[midpoint_x,midpoint_y,midpoint_z,hue]` and `[direction_x,direction_y,direction_z,length]`, in heap spans, after which its vertex stage performs only indexed box construction.

The declared experimental axis is heap bytes versus vertex-stage work: Mode A stores 1,200 rotated vertices and pays endpoint algebra, Mode B stores `1,200C` projected vertices and pays endpoint lookup, Mode C stores `3,000C` edge poses and pays box construction, and layer stores the same `3,000C` edge poses and runs the same box construction as Mode C through standalone square slots.

Mode C versus layer is the clean allocation-and-indirection comparison because kernel, logical records, indexed mesh, and vertex work are equal; Mode A and Mode C are shipped, while Mode B was third priority and is deferred because the Phase 5 checkpoint reached 5,106 net new lines against the 3,290-line implementation estimate.

## 4. DATA records, spans, and walls

Every DATA record is one nearest-sampled unfiltered RGBA32F texel and therefore exactly 16 bytes; five-dimensional values use two same-length structure-of-arrays spans rather than an overloaded sentinel or lossy packing.

Static base slot `base_four[j]` is `[x₁,x₂,x₃,x₄]` and `base_fifth[j]` is `[x₅,0,0,0]`, so 1,200 base vertices consume `1,200 × 2 × 16 = 38,400` logical bytes: 24,000 bytes hold the 6,000 meaningful coordinates and 14,400 bytes are zero padding imposed by the second RGBA record.

Static edge slot `edge[j]` is `[endpoint_a,endpoint_b,0,0]` represented as exactly integral floats below 1,200, so 3,000 pairs consume `3,000 × 16 = 48,000` logical bytes; static shared DATA is therefore 86,400 logical bytes and is uploaded once.

Mode A output uses `rotated_four[j] = [r₁,r₂,r₃,r₄]` and `rotated_fifth[j] = [r₅,0,0,0]`, exactly `1,200 × 2 × 16 = 38,400` mutable logical bytes and 124,800 logical bytes including static shared DATA, independent of copy count.

Mode B output uses `projected[j] = [p₁,p₂,p₃,r₅]` and `projected_meta[j] = [valid,0,0,0]`, exactly 32 bytes per projected vertex; the separate validity record wastes 12 bytes per vertex but keeps finite coordinates, fifth-axis hue, and rejection state independently testable without NaN or sign-bit conventions.

Packing one validity record per four vertices would reduce Mode B to 20 bytes per vertex, but it is rejected for this experiment because four projected MRT records plus one validity MRT exceed the guaranteed four color attachments and otherwise require a second packing or recomputation pass; Mode C now supplies the like-for-like capacity oracle, so preserving a one-pass two-output Mode B is the less confounded control and its padding remains explicit.

Mode C output uses exactly layer's two edge-pose records per submitted edge and therefore consumes 32 mutable logical bytes per edge with no additional validity record.

A physical heap descriptor still describes one square region in one DATA array layer, while a `DataSpan` is the typed logical allocation `{logical_len, page_records, page_count, first_directory_slot}` whose ordered page handles may occupy non-contiguous regions and layers; this preserves the existing 20-bit descriptor-index and 12-bit generation handle ABI while lifting a logical slot beyond one two-dimensional texture.

The fixed-capacity span-directory UBO stores each span header `{page_records, page_count, first_directory_slot}` followed by its ordered page handles beside the descriptor UBO and frame/dispatch uniforms; this lab configures 16 span headers and 128 page handles in 768 bytes, validates that allocation against `max_uniform_buffer_binding_size`, reports both the configured and exposed values, and changes directory data only at allocation or step setup.

Every page of one span uses one buddy class with side `q` and `page_records = q²`; an accessor computes `page = index / page_records` and `local = index % page_records`, resolves the handle at `first_directory_slot + page`, and computes x and y from that selected descriptor's own width, with no search and no output-grid dimension in input decoding.

The last page may be partially used but is never assigned a smaller class, so span reservation is `page_count × q² × 16`, padding waste is less than one page per span, and logical bytes, reserved bytes, and last-page waste are all reported; allocator planning, descriptor availability, directory capacity, fragmentation, configured heap side, created layer count, and driver allocation success remain visible walls.

For two equal-length MRT output spans, the planner performs an atomic dry run for both spans and returns the greatest whole-copy count it can place without mutating the heap; delivery is then `min(requested copies, planned heap copies, address copies, draw copies)`, and a later driver refusal is displayed as a runtime refusal rather than converted into a smaller unreported workload.

The implementation queries texture dimension, array-layer count, UBO size, attachment count, sampled-texture count, buffer limits, and RGBA32F render/sample usage, then displays the actual created DATA side, layers, bytes, free buddy classes, descriptors, directory entries, and every term of the delivery minimum.

WebGL2 exposes no trustworthy total free-VRAM query, so this lab requests the configured 512 by 512 by 16 DATA array, exactly 67,108,864 physical bytes, after validating its side and layer count against the exposed limits; an insufficient exposed limit is a displayed initialization refusal, while the configured request, successful creation, allocator delivery, and later driver allocation failures remain distinct facts.

## 5. Mode arithmetic

Let `L_A = 86,400 + 38,400 = 124,800` be Mode A logical heap bytes, let `L_B(k) = 86,400 + 1,200 × C(k) × 32 = 86,400 + 38,400C(k)` be Mode B logical heap bytes, and let `L_C(k) = 86,400 + 3,000 × C(k) × 32 = 86,400 + 96,000C(k)` be Mode C logical heap bytes; these formulas exclude buddy padding and page-directory metadata, which are runtime physical facts.

The layer comparator materializes the same two RGBA32F edge-pose slots as Mode C, so its logical payload is `L_layer = E × 32`, while its actual one-texture-per-slot allocation is `P_layer = ceil(√E)² × 32`; one slot alone is `ceil(√E)² × 16`, which is the ceiling the multi-layer heap is intended to remove without changing Mode C's workload.

|Rung|Copies `C`|Edges `E`|Mode A total|Mode B output|Mode B total|Mode C output|Mode C total|Layer one slot|Layer two-slot allocation|
|----|---------:|--------:|-----------:|------------:|-----------:|------------:|-----------:|-------------:|------------------------:|
|`(1,1,1,1,1)`|1|3,000|124,800 B|38,400 B|124,800 B|96,000 B|182,400 B|48,400 B|96,800 B|
|`(3,3,3,3,3)`|243|729,000|124,800 B|9,331,200 B|9,417,600 B|23,328,000 B|23,414,400 B|11,669,056 B|23,338,112 B|
|`(7,7,5,5,5)`|6,125|18,375,000|124,800 B|235,200,000 B|235,286,400 B|588,000,000 B|588,086,400 B|294,053,904 B|588,107,808 B|
|`(47,45,45,45,45)`|192,729,375|578,188,125,000|124,800 B|7,400,808,000,000 B|7,400,808,086,400 B|18,502,020,000,000 B|18,502,020,086,400 B|9,251,014,236,304 B|18,502,028,472,608 B|

Every number in this table is arithmetic: for example, `(7,7,5,5,5)` gives `C = 7 × 7 × 5³ = 6,125`, Mode B mutable bytes `= 6,125 × 1,200 × 32 = 235,200,000`, Mode C mutable bytes `= 18,375,000 × 32 = 588,000,000`, and layer side `= ceil(√18,375,000) = 4,287`, where `4,286² < 18,375,000 ≤ 4,287²`.

The table is not a promise that any row is allocatable or drawable; the page computes delivered copies and edges from the live heap plan, `u32` kernel and instance index spaces, the overridable gles draw policy, and any validation refusal while leaving the requested tuple selected.

## 6. Mode A: algebraic heap path

At scene setup, Mode A uploads the two base-coordinate spans and the edge span once, allocates the two 1,200-record rotated-output spans once, registers one rotation kernel, and creates the immutable heap resources selected by the output-path spike.

Each frame writes the 192-byte frame uniform, dispatches the 1,200-element rotation kernel through one two-attachment fragment pass, completes the selected GPU-side output transfer, and renders one indexed instanced box draw with `instance_index / 3,000` as copy and `instance_index % 3,000` as base edge.

The compute output is GPU-resident and no readback gates rendering; encoder and queue order make the fragment-compute write, transfer, and following vertex texture loads visible in order, while an explicit mapped fence exists only around requested measurements.

The lattice center is decoded from the odd tuple in mixed-radix order, converted to centered integer digits, and combined with the five CPU-rotated basis vectors; changing the rung changes only uniforms and the requested instance count, never heap allocation.

The only type WALL on Mode A's draw count is the wgpu `u32` instance range; the implementation page starts with a 2,000,000-instance tab-safety POLICY, permits an explicit override through 8,000,000, and separately displays WebGL2's positive signed `GLsizei` range of 2,147,483,647 as an arithmetic wall rather than a detected safe workload.

Mode A delivery is `min(E_requested, E_u32_wall, E_policy)` with every term displayed, and changing the policy never changes the selected tuple; allocation or validation refusal remains a separate runtime fact rather than a guessed wall.

## 7. Materialized modes and dispatch

Mode B keeps the same static base and edge spans but allocates paired projected and metadata spans for `1,200 × delivered copies` records, then dispatches a kernel whose output index decodes copy and base vertex, rotates, translates, double-projects, and writes the two MRT fields.

Mode C allocates paired edge-pose spans for `3,000 × delivered copies` records and runs layer's exact kernel body, input data, output field order, index decoding, and numeric operation order through dialect v2, so only heap paging, handle indirection, and output transfer differ from layer.

All `P` per-page dispatch headers `{global_base, valid_length}` are static for a step plan and are uploaded once at step setup into one uniform buffer at strides aligned to the runtime `min_uniform_buffer_offset_alignment`; each pass selects its header with `set_bind_group` and a dynamic offset on the unchanged immutable bind group, which creates no bind group and performs no per-frame upload.

Per-frame CPU-to-GPU bytes are therefore exactly 192 in Modes A, B, C, and layer; the page separately reports step-setup header bytes, `P` fragment-compute passes, transfer command count and bytes, encoder count, and queue submission count.

The Mode B vertex stage loads one edge record and two materialized records per endpoint, five accessor calls per ideal unique indexed box vertex, then builds the same box and hue without lattice addition or perspective division; Mode C loads two edge-pose records and performs only the common indexed box construction.

The Mode B delivery is `min(C_requested, floor(H_pair / 1,200), floor(I_kernel / 1,200), floor(E_u32_wall / 3,000), floor(E_policy / 3,000))`, and Mode C substitutes paired edge-pose capacity divided by 3,000; `H_pair` is the exact atomic paired-span plan, `I_kernel` is the dialect's `u32` index ceiling, and policy is displayed independently from each WALL.

Step changes may free and replace Mode B or Mode C spans, but allocation never occurs per frame; a failed larger request leaves the requested control at that rung, renders the greatest planned whole-copy delivery immediately when nonzero, and prints requested and delivered tuples, copies, vertices, edges, bytes, pages, and the limiting term.

## 8. Kernel dialect v2 over heap handles

The author-visible body remains entry-point-free WGSL declarations plus `fn kernel(index: u32, uniforms: UniformType) -> ResultType`; the author declares stable kernel name, uniform type and nonzero 16-byte-multiple size, accessor names, result field names, and an output count from one through four.

Every declared input produces the author-visible signature `fn load_name(index: u32) -> vec4<f32>`; access outside that input span's logical length returns canonical zero, so padding and absent pages never become observable data.

Registration receives names and shapes but no resource handles, builds accessor stubs, parses the body, rejects forbidden IR, assembles the fullscreen-triangle vertex and fragment entry points, maps result fields to MRT locations, validates with Naga, and creates the pipeline once.

The forbidden constructs remain module-scope workgroup variables, atomic types or operations, barriers including workgroup-uniform loads, and raw storage resource declarations; kernel bodies also cannot declare bindings or entry points because all resources and entry points belong to the generated lowering.

Dispatch supplies ordered input `DataSpan` references, ordered output `DataSpan` references, and exact uniform bytes; it creates no bind group, validates handles and constant-class spans on the CPU, selects a pre-uploaded header by dynamic uniform offset, attaches output views selected by the output-path contract, and sets the applicable immutable heap bind group.

The logical index space comes from the first output span's `logical_len`; every other output must have the same length and the same `page_records` and `page_count` for each MRT pass, the generated fragment entry computes `index = global_base + local_y × output_page_width + local_x`, and the last page discards indices at or beyond valid length.

Each generated input accessor independently selects its input page and derives coordinates from that page descriptor's own width and height, which removes v1's addressing-bug class structurally: changing the output grid can no longer change how an input index is decoded.

Version 1 froze texture views and accessor bindings into a bind group at registration and made one physical texture per slot the allocation unit; version 2 freezes only code, layout, and pipeline, while dispatch-time span handles choose data behind the already-bound heap arrays and logical slots may cross layers.

Registration errors are the typed set `InvalidDescriptor`, `Parse`, `Forbidden(WorkgroupVariable|Atomic|Barrier|RawStorageAccess)`, and `Validation`; each carries the stable kernel name and exact diagnostic.

Dispatch and resource errors are typed as `WrongDevice`, `InvalidHandle`, `StaleHandle`, `WrongHeapKind`, `EmptySpan`, `SpanDirectoryFull`, `SpanPageMissing`, `SpanGeometryMismatch`, `OutputLengthMismatch`, `OutputAlias`, `ReadWriteAlias`, `UniformSizeMismatch`, `DynamicOffsetAlignment`, `IndexSpaceOverflow`, `ViewportLimit`, `HeapCapacity`, `ScratchCapacity`, `OutputTransferUnsupported`, `Pipeline`, `DeviceLost`, `StaleGeneration`, `Deadline`, and `Mapping`.

`OutputAlias` rejects two MRT fields targeting the same logical output page, `ReadWriteAlias` rejects sampling a page that is the physical attachment of the same pass, and generation checks occur on every CPU-side resolve in debug builds before views, offsets, or directory entries are used.

## 9. Outputs land in heap layers

The shipping design never renders into a DATA layer while a full-array sampled view of that same texture is bound: wgpu-core merges attachment and bind-group texture usage by overlapping subresource and rejects that configuration before a GL call, while WebGL2 independently forbids the resulting feedback loop.

The default output path creates a separate RGBA32Float SCRATCH array with the live DATA side and four layers, omits SCRATCH from the immutable heap bind group, renders each kernel page into one-layer SCRATCH views, and then issues `copy_texture_to_texture` operations from the written SCRATCH regions to their destination DATA pages before the indexed draw.

SCRATCH is transient workspace rather than heap capacity, so its physical bytes are runtime arithmetic `DATA_side² × 4 × 16`; the implementation copies full valid rows plus one exact-width tail row when needed for each output page, reports one or two copy commands and exact copied texels, and never counts SCRATCH bytes as logical heap bytes.

The default transfer moves zero CPU bytes and copies 38,400 GPU bytes per Mode A frame, `38,400C` per Mode B frame, and `96,000C` per Mode C frame by arithmetic; command cost and measured wall remain reported facts rather than inferred to be free.

The copy path was paid on the target gles backend by the 2026-09-02 Phase 0 visible replay: SCRATCH layer 1 copied through wgpu into DATA origin `[4,5]` on layer 2 and vertex-stage consumption read back `[0.25,-0.5,1.5,7.0]`; command cost remains a per-frame measurement rather than an architectural assumption.

The independently tested fallback is two DATA arrays and two immutable bind groups, never rebuilt, alternated by frame: all static inputs are duplicated, compute samples the previous array while attaching destination layers of the other array, and the indexed draw samples the newly written destination array after the pass.

The ping-pong fallback has zero transfer bytes but doubles resident DATA storage, uses one bind-group selection per pass rather than rebuilding state, and computes capacity against one destination array rather than pretending the two arrays extend one logical heap; its Phase 0 replay passed but SCRATCH-copy is selected, so ping-pong remains proven contingency and is not developed further in this implementation round.

RGBA32Float is not blendable, so every compute color target uses no blend state and `ColorWrites::ALL`; blending is neither requested nor used as an accidental capability test.

The implementation round's first commit is a standalone golden spike before any lattice kernel, and its first case deliberately reproduces the invalid shipping-shaped conflict by binding the full DATA-array heap view, sampling a known record from one DATA layer, attaching another DATA layer from the same array, and recording the exact scoped wgpu diagnostic and backend facts when validation refuses it.

The golden's default-path case binds the real heap group, reads the known input from DATA in the fragment kernel, writes `[0.25,-0.5,1.5,7.0]` into SCRATCH at a known layer and origin, exercises the framebuffer-copy lowering into a known DATA destination, and uses the real dynamic dispatch-header offset.

Consumption in the golden is a point or indexed draw whose vertex stage performs `textureLoad` on the destination `texture_2d_array<f32>`, forwards the loaded value flat to an RGBA32Float fragment target, copies that target to a padded mapped buffer, drives `map_async` through `device.poll` in a browser-yield loop, and requires all four exact binary-representable f32 values.

The 2026-09-02 visible replay ran in Firefox on `ANGLE (Intel, Mesa Intel(R) Graphics (MTL), OpenGL ES 3.2)` with backend Gl and dynamic-uniform stride 256: direct-overlap refusal, SCRATCH-copy, and ping-pong cards all printed PASS, both transfer paths returned exact `[0.25,-0.5,1.5,7.0]`, and the console was clean.

The paid direct-overlap diagnostic was: "Validation Error / Caused by: In RenderPass::end / In a pass parameter / Attempted to use Texture with 'spike DATA A' label (mips 0..1 layers 2..3) with conflicting usages. Current usage TextureUses(RESOURCE) and new usage TextureUses(COLOR_TARGET). TextureUses(COLOR_TARGET) is an exclusive usage and cannot be used with any other usages within the usage scope (renderpass or compute dispatch)."

The diagnostic localizes the conflict to attached DATA layers 2 through 3 against the full-array resource view, the SCRATCH case used producer offset 256 and vertex offset 512, and the ping-pong case used two immutable groups A-to-B then B-to-A with offsets 256 and 512; these observed facts select SCRATCH-copy as the sole shipping path while preserving the spike as the fallback oracle.

## 10. Render path

All heap modes and the in-page layer comparator use one indexed instanced draw and the same static long-box mesh with eight unique vertices and 36 indices, surface format, depth target, rawgl presentation values, no mipmaps, `cull_mode: None`, no blending, and one-sample rasterization; only endpoint production and storage change.

The vertex stage derives copy and edge exclusively from `instance_index`, resolves the selected mode's endpoint or edge-pose spans, rejects the box when either endpoint is invalid, and otherwise follows rawgl's BOX construction by value: direction is normalized endpoint delta, helper is `(0,0,1)` when `|direction.z| < 0.9` and `(0,1,0)` otherwise, side is normalized `direction × helper`, up is normalized `side × direction`, and thickness is `0.013`.

The fixed rawgl camera is yaw 20 degrees with cosine `0.9396926208` and sine `0.3420201433`, pitch 15 degrees with cosine `0.9659258263` and sine `0.2588190451`, distance `9.0`, and perspective scale `1.72`; only the object rotates in five dimensions, so no camera term depends on time.

Rawgl's OpenGL clip row uses near `0.1`, far `30.0`, and depth range minus one through one; wgpu uses zero through one, so the shader applies `z_wgpu = (z_gl + w) / 2`, which reduces to `far / (near − far) × view_z + far × near / (near − far)` with `w = −view_z`, maps view z `−0.1` to normalized depth zero and `−30` to one, and uses `LessEqual` depth comparison.

Hue uses `3 + 8 × hypot((k₃−1)/2,(k₅−1)/2)` as rawgl's lattice extent and maps the post-rotation fifth midpoint as `clamp(0.5 + 0.5 × midpoint_x5 / extent,0,1)`; colour uses the exact phase-offset hue function, light `0.58 + 0.24 × |normalize(side+up)·normalize(0.4,0.7,0.6)|`, and `mix(white,hue_rgb,0.78) × light` with alpha one.

Static heap contents, span-directory records, and dispatch headers change only at initialization or step setup; the sole per-frame CPU upload is the 192-byte frame uniform in every mode, while GPU transfer bytes and commands are reported separately.

## 11. Substrate posture

The initial and implementation target is wgpu 24 with `wgpu::Backends::GL` over WebGL2 only; the page explicitly requests GL and neither autodetects nor prefers browser WebGPU.

Handle encoding, descriptor meaning, `DataSpan` indexing, kernel-body syntax, typed errors, and requested-versus-delivered semantics avoid WebGL-specific author behavior so a future WebGPU lowering can preserve kernel bodies and handle semantics, but no WebGPU design, measurement, promise, or empty leaked-differences ledger belongs in this contract.

A leaked-differences ledger is opened only when another physical lowering exists and has field evidence to populate it.

## 12. Layer comparator

Comparator provenance has two explicit branches: if `ember-lab-layer` from `lane/comp-layer` is present in main before implementation begins, the heap lab takes it as a workspace package dependency and makes no shared-crate edit or copy; otherwise the heap lab carries a private faithful copy budgeted at about 2,400 lines, with v1 generated source, square-slot arithmetic, golden checksum, and CPU kernel oracle pinning fidelity.

At each selected ladder step, shipped Modes A and C and layer receive the identical tuple, geometry, 3,000-edge-per-copy submitted count, pole epsilon, animation time, camera, render target, indexed box mesh, and overlay fields; clipped edges remain included in submitted counts in all paths, and the retained Mode B design must obey the same rule if implemented later.

The comparator reports requested and delivered tuple, copies, projected or edge-pose records, submitted edges, submitted indices `36E`, ideal unique vertex invocations `8E`, compute passes, copy commands and GPU bytes, frame median and p95, microseconds per submitted edge, CPU-to-GPU bytes per frame, logical heap bytes, physical heap and SCRATCH bytes, layer one-slot and two-slot bytes, policy value, limiting WALL, and full arithmetic; shown-edge count is explicitly unavailable because this GL path has no pipeline-statistics query and never substitutes a CPU guess.

Mode C and layer are the primary equal-work pair: they use the exact same kernel body and numeric operation order, two-record edge-pose model, 192-byte frame uniform, indexed vertex work, delivered copies, pixels, fence, adaptive timing, and tuple; any mismatch disqualifies the rung before allocation or timing differences are interpreted.

The live equality gate reports Mode C and layer delivered counts and static signatures, maps both GPU-produced edge-pose records for eight deterministic indices spanning the selected range, and renders both paths into a 64 by 36 offscreen target at step 1; PASS requires equal counts and signatures, every sampled component exact or within `4 × 10⁻⁵`, and byte-identical 2,304-pixel images with displayed checksums, otherwise both timing cards remain visibly disqualified.

That equality gate proves two in-page paths share one presentation but cannot detect both drifting together from rawgl; the native presentation oracle therefore parses all three shipped WGSL modules and pins rawgl's camera, projection, thickness, hue, lighting, colour, depth conversion, `LessEqual`, no-blend, and no-MSAA values.

Fair timing labels the first fenced frame after every path or rung switch as cold/pipeline warm-up and excludes it, uses the second fenced frame for the 100 ms animation decision, then uses three additional warmups, 15 samples, and repeat-until-32-observed-quanta batching from `crates/what-is-this/src/kernels.rs`, with the same ordered four-byte mapped fence submitted immediately after the final presentation draw submission.

Both initial fenced frames remain visible in the overlay even when they are cold-start outliers; if the second exceeds 100 ms the rung uses explicit single-frame-on-demand observations, while a second frame at or below 100 ms admits animation and the adaptive series.

Primary time begins before CPU uniform writes and command encoding and ends when the mapped completion fence reports the final presentation draw complete; the surface texture remains alive across that wait, the end timestamp is captured, and only then does `present()` hand the completed image to the compositor, so compositor scheduling is outside the measured region while raster work remains inside.

The gles backend requires `map_async` progress through `device.poll` in a zero-timeout browser-yield loop; the first poll precedes the first yield, every poll is counted, every result prints poll count and fence-wait wall, and every wait has both a 4,096-poll bound and 30,000 ms deadline plus a generation guard; after any yield the deadline is checked before another poll or completed callback can be accepted, while browser suspension can still delay when JavaScript resumes to observe that refusal; `Queue::on_submitted_work_done` is not used because the four-byte MAP_READ fence is the single contracted completion path.

## 13. Questions, claims, and oracles

Claim A is that compute output can remain GPU-resident from fragment-compute production through geometry consumption on WebGL2; its oracle is the selected spike path plus an end-to-end frame whose vertex-loaded values and image checksum match the CPU reference without a CPU readback between production and consumption.

Claim B is that heap allocation and handle indirection have a measurable price in a real vertex-fetch path; its clean oracle is Mode C versus layer at an equal delivered rung, where kernel, edge-pose records, indexed box work, pixels, fence, and adaptive timing are identical and the delta is reported in frame time and microseconds per submitted edge.

Claim C is that heap spans lift layer's one-texture-slot capacity wall without changing the workload; its oracle is the greatest whole-copy rung admitted and successfully allocated by Mode C and layer, with identical logical edge-pose bytes, heap aggregate and reserved bytes, layer square-slot bytes, and every refusal term printed together.

A later engine-design round may use Claims A through C to reason about mesh-ID-to-handle migration, but this lab makes no Claim D, registry design, engine edit, or engine-migration promise.

No claim is established by a faster picture alone: conformance, submitted counts, byte accounting, completion ordering, and pixel or numeric oracles must pass before a timing series is eligible for comparison.

## 14. Page and measurement plan

One self-contained page exposes the 113-rung control and shipped Modes A and C and layer with one common canvas and one common facts/results overlay; Mode B remains a documented deferred design, and the deployed JavaScript and wasm URLs use a versioned `module_or_path` on every redeploy.

Selecting any rung or mode immediately attempts its runtime plan and renders the delivered work without measurement admission; controls never snap to the delivered rung, and zero delivery is a visible refusal rather than the previous frame mislabeled as current.

Every selection resets samples, p95 state, timer batches, frame counters, shown counts, and generation tokens; an older allocation, map callback, or measurement can neither publish into nor restore a newer selection.

The equality gate runs before warm-up timing, uses bounded `map_async` readbacks solely as an oracle, restores the requested path and rung before measuring, and is generation-guarded so an older comparison cannot publish or restore resources after a newer selection.

Animation begins only when the second fenced post-switch frame is at or below 100 ms; above that threshold the page becomes single-frame-on-demand, measures exactly one requested frame at a time, yields around asynchronous completion, and displays its true wall time even when it takes seconds.

The timer probe performs at most 4,000,000 consecutive `performance.now()` reads, stops after 32 positive transitions or 500 ms, and uses the smallest positive transition as the observed quantum; unresolved timers make timing unavailable without preventing rendering.

Adaptive samples increase whole-workload repeats until a batch spans 32 observed quanta, cap the batch target at 250 ms and repeats at 4,096, normalize by repeats, and stop a mode after a finite 30-second suite budget; medians use the middle sorted sample and p95 uses nearest-rank rank `ceil(0.95n)`.

Every fenced warm-up, one-frame request, adaptive warm-up, and adaptive candidate is listed beside the result with its batch repeat count, completion-poll count, and fence-wait wall; a large poll count or quantized wait is therefore evidence rather than silently folded into frame time.

Counts are literal: requested is the tuple-derived count, delivered and submitted are the runtime-WALL-and-policy draw instance count, measured is the work enclosed by the fence, and shown is unavailable rather than guessed because pole clipping occurs in the vertex stage; none is substituted for another.

The page labels 2,000,000 instances as its initial tab-safety POLICY, accepts an explicit policy through 8,000,000, labels 2,147,483,647 from WebGL2's positive signed `GLsizei` draw-count range and the `u32` index limit as arithmetic WALLS, and never presents policy as detected hardware capacity.

## 15. Test plan

Algebra tests generate deterministic base vertices, centered digits, times, and tuples and compare direct `R(v+c)` with `Rv+sΣkᵢReᵢ` in f64 and the shipped f32 order, including zero, negative digits, the four named rungs, and near-pole inputs.

Projection tests prove the two denominators, epsilon validity, fifth-coordinate hue input, and an explicit counterexample to projection linearity against a CPU reference.

Heap tests cover constant-class paired-span planning across layers, `{page_records,page_count,first_directory_slot}` packing, directory capacity, descriptor generation reuse, quotient/remainder page crossings, last-page padding and waste, atomic two-output rollback, buddy fragmentation arithmetic, alias rejection, and requested-versus-delivered WALL strings.

The output-path golden is the mandatory first implementation test and proves the exact direct-overlap diagnostic, dynamic uniform offset, SCRATCH render and framebuffer-copy lowering or ping-pong fallback, vertex-stage array load at nonzero layer and origin, and exact readback value.

Dialect tests parse and validate author bodies with generated accessor stubs, exercise every forbidden construct and typed descriptor failure, prove dispatch-time handle replacement changes data without pipeline replacement, and reproduce v1's unequal input/output-width case to show each accessor follows its own descriptor.

Kernel conformance compares Mode A rotation and endpoint reconstruction, Mode B materialization and validity, Mode C's exact layer edge-pose kernel, hue, and box pose against CPU reference values at deterministic indices and tuples.

Geometry tests pin 600 cap vertices, 1,200 cap edges, 1,200 prism vertices, 3,000 prism edges, edge length `3−√5`, circumradius `2√2`, the eight-vertex and 36-index box, step count 113, step 111 `(45,45,45,45,45)`, step 112 `(47,45,45,45,45)`, and top edge arithmetic 578,188,125,000.

Page-contract tests pin explicit GL backend selection, versioned loader paths, immediate rendering without admission, stable requested controls, generation cancellation, sample resets, bounded waits, submit-then-fence-then-end-timestamp-then-present ordering, displayed poll evidence, single-frame fallback, and exact median, p95, and byte formulas.

Comparator tests and the live page run identical work through Mode C and layer's exact kernel, compare two edge-pose records at eight deterministic selected-rung indices within `4 × 10⁻⁵`, compare exact 64 by 36 presentation-image bytes and checksums at the 3,000-edge rung, and disqualify timing when delivery counts, signatures, sampled values, or image bytes disagree.

## 16. Implementation phases and line budget

Phase 0, required first commit, is the direct-conflict diagnostic plus SCRATCH-copy and ping-pong output-path golden, vertex-stage consumption, dynamic-offset test, and capability report, estimated at 360 new Rust, WGSL, and test lines.

Phase 1 adds constant-class `DataSpan`, paired allocator planning, quotient/remainder access, page-directory metadata, static dispatch headers, runtime WALL arithmetic, and generation checks without engine changes, estimated at 480 lines.

Phase 2 implements dialect v2 registration, generated accessors, dispatch validation, page passes, typed errors, and conformance fixtures, estimated at 500 lines.

Phase 3 implements invariant geometry data, the indexed box, the 113-step ladder, the 192-byte frame uniform, Mode A rotation and rendering, and CPU oracles, estimated at 450 lines.

Phase 4 implements mandatory Mode C over the merged `ember-lab-layer` workspace dependency, comparator integration, and equal-work checksum gates without a private copy, estimated at 420 lines.

Phase 5 implements the self-contained WebGL2 page, versioned loader, measurement state machine, overlays, requested-versus-delivered presentation, cancellation, and a link to the preserved historical heap benchmark, estimated at 480 lines.

Phase 6 is skipped: Mode B was optional only if budget remained, while the Phase 5 checkpoint was already 5,106 net new lines against the 3,290-line estimate; §7 remains its future implementation contract rather than pretending it shipped.

Phase 7 completes the two-frame warm-up rule, live numeric and image equality gate, page-contract tests, lint repair, bundle evidence, and contract reconciliation; the budget of record remains 3,290 lines, and the final report accounts for the overrun rather than compressing the oracles.

## 17. What does not change

Renderer austerity remains no mipmaps, `cull_mode: None`, one indexed presentation pass, and no decorative pass; fragment-compute passes exist only to produce the contracted GPU-resident data.

Heap and compute contents feed presentation and prediction only and cannot author simulation, collision, protocol state, reconciliation, or other gameplay truth; missing or stale data produces fallback pixels or a typed error.

The default SCRATCH-copy path creates one heap bind group at initialization and keeps its resource identities immutable; the fallback has two immutable bind groups, never rebuilt, while heap contents, descriptors, span directory, step headers, and frame uniform may change only through the contracted setup and dispatch sequence.

The page reports requested, delivered, submitted, and measured quantities separately, labels shown unavailable, computes walls from live facts and fixed type ceilings, performs no measure-first admission, preserves controls across refusal, guards every asynchronous publication by generation, and never waits without a deadline and browser yield.

Browser numbers remain `requires visible replay` until a visible replay supplies them, and arithmetic is labeled arithmetic with its formula.

## 18. Unresolved risks

The SCRATCH-to-DATA texture copy and ping-pong fallback are paid on the dated target replay, while their costs under multi-page lattice loads remain unresolved measurements; SCRATCH-copy is the selected default and direct overlap is a paid validation refusal.

The effective maximum safe DATA allocation is not exposed as free VRAM, so configured byte budget, successful texture creation, allocator capacity, and driver allocation failure cannot be collapsed into one predictive number.

Multi-page Modes B and C require one fragment pass per page pair plus GPU output movement on the default path; a 2026-09-02 hidden Firefox-pane observation at step 6 recorded Mode C producing 729,000 edges in 12 compute passes with 26 copy commands and 23,328,000 GPU-copy bytes, yet its 17.1 ms warmed single frame was faster than layer's 25.7 and 30.3 ms observations and Mode A's 33.4 and 45.8 ms observations, so copy did not erase throughput at that rung, while scaling and causal isolation remain unresolved and every number requires visible replay.

The same hidden-pane walk used `ANGLE (Intel, Mesa Intel(R) Graphics (MTL), OpenGL ES 3.2)`, backend Gl, heap 512 by 512 by 16, 0.1 ms timer quantum, and 0.1 ms zero-timeout latency; the console was clean, all paths rendered, controls held the request, and the page showed wall arithmetic, 192 bytes per frame, and paid SCRATCH copy.

Cold observations are evidence for the warm-up rule rather than timing claims: step 6 first frames included Mode C at 54.0 ms, layer at 140.1 ms, and Mode A at 99.0 ms before lower later frames, while step 1 Mode A immediately after load recorded 412.2, 102.0, and 993.1 ms before later 33.5, 8.9, and 9.0 ms observations; all were hidden-pane single-frame observations and require visible replay.

The 2026-09-02 hidden Firefox replay of v5 exposed a fence-order regression after the equality oracle passed with identical `367996de3f159a9f` image checksums and zero mismatches: step-1 fenced walls landed on one-second boundaries, including 1,996.0 and 1,000.1 ms for Mode A, 1,999.2 and 2,000.0 ms for Mode C, and one 65,998.9 ms Mode A request, while a page-context zero-timeout probe remained 0.1 ms and v4 in the same pane had produced 8.9 and 9.0 ms warmed step-1 observations.

The v5 cause was presentation ordering, not an initial-yield delay or leaked conformance mapping: source inspection showed that polling preceded every yield and every conformance readback was awaited and unmapped, but each timed draw called `present()` before submitting its separate fence, placing hidden-document compositor work ahead of completion; v6 submits the fence immediately after the draw batch, waits and captures the end timestamp, then presents, and prints the poll count and wait wall so the diagnosis requires visible replay rather than being assumed fixed.

The visible side-by-side presentation review found a separate corrected defect: the heap page used an orthographic `w = 1` clip with depth compressed by `0.002`, scale `0.075`, flat unlit colour, and camera yaw and pitch driven by time, producing an object about half rawgl's width with a flat blue-green disc appearance; v7 replaces all three in-page presentation paths with the rawgl values in §10, while the lesson is that an in-page equality oracle cannot detect shared drift from its external visual reference and the pinned-literal test must carry that role.

The span-directory UBO introduces a second finite metadata WALL and uniform dynamic-indexing cost; Mode C versus layer prices the resulting handle and allocation path, while runtime facts expose directory consumption and padding waste.

The indexed box relies on the driver's post-transform cache to approach the `8E` ideal; the submitted work is always `36E` indices, and absent pipeline statistics the measured wall cannot prove an exact physical vertex-invocation count.

Mode B's two-record validity layout deliberately spends 12 padding bytes per vertex; packing four flags would require an extra output or pass and is deferred because Mode C supplies the uncontaminated capacity comparison.

The top ladder requests far more records and draw instances than the dialect and gles draw ranges can deliver, so it primarily tests honest wall arithmetic and control stability rather than rendering 578 billion edges.

Comparator provenance is resolved by the merged `ember-lab-layer` workspace dependency; any minimal public seam it requires is an explicit sibling-lab change, and no private comparator copy is permitted.

CPU-computed rotated basis values and GPU-computed rotated base vertices can differ by operation order or transcendental implementation, so the f32 algebra tolerance and hue range must be fixed before timing.

Dynamic uniform offsets at stride 256, full-array DATA sampling alongside separate SCRATCH attachments, framebuffer-copy lowering, and alternating immutable ping-pong groups all passed the dated Phase 0 target replay; performance, larger page counts, and allocation scale remain later browser facts.

No visible browser result yet identifies the crossover among Modes A, B, C, pure, and layer, and the raster-bound evidence makes a clean crossover uncertain rather than guaranteed.
