# GPU resource heap

Status: implemented contract for `ember-lab-heap`; the prototype is first evidence for the WebGL2 lowering, browser numbers still require visible replay, and this remains outside the engine.

## 1. What this is and is not

The resource heap gives Ember the descriptor-heap effect: bind once, pass an integer handle, and express material, image, and compute-data variety as mutable data behind that handle.

This is not bindless: WebGL2 still sees a fixed finite pair of texture arrays, two fixed samplers, and one fixed descriptor table, and every allocation must fit those declared resources.

The five-entry competition measured box-edge work as raster-bound on the owner's Intel iGPU, with four architectures within about 1.8 times throughput at roughly 20–37 thousand edges per millisecond, so ergonomics and portability choose the architecture while per-draw binding churn and wasm-to-JavaScript calls remain the unpriced lever.

The tessellation entry's giant instanced draw intentionally removed that lever from its measurement, while the layer entry proved that bindings frozen at registration are workable but a single texture slot cannot admit a 50-million-edge, 1.6-GB step and that each slot must decode indices in its own physical index space.

Ember's fixed-order mesh IDs are already a proto-heap; this contract makes allocation, lifetime, capacity, and backend lowering explicit without granting heap contents authority over simulation.

## 2. Physical layout

There are two physical heaps, each one `TEXTURE_2D_ARRAY`: DATA is RGBA32F with nearest, non-filtering sampling for compute-dialect slots and structured numeric regions across as many layers as needed, while IMAGE is RGBA8 with linear sampling for game images.

There is exactly one sampler per heap, and format and filtering uniformity within a heap are constraints rather than bugs; data requiring integer precision, mipmaps, anisotropy, sRGB reinterpretation, or a different filter belongs in a future separately contracted heap class rather than a special-case binding.

Initialization queries `max_texture_dimension_2d`, `max_texture_array_layers`, `max_uniform_buffer_binding_size`, and format capabilities; it clamps the allocatable side to 65,535 texels because descriptors use 16-bit coordinates and refuses RGBA32F DATA initialization unless rendering, copying, and non-filtering sampling are exposed.

Configured memory budgets choose actual array extents below hardware maxima, all layers in one heap have the same square side, and the displayed physical byte walls are `side × side × layers × 16` for DATA and `side × side × layers × 4` for IMAGE.

The prototype requests 256×256 arrays with up to eight layers per heap, capped by the live array-layer limit, for 8,388,608 DATA bytes and 2,097,152 IMAGE bytes when all eight layers are available; it displays the live cap, chosen extent, byte wall, descriptor wall, and remaining allocatable buddy blocks rather than extrapolating from the requested configuration.

Logical compute slots are ordinary DATA descriptors and may occupy independent regions on independent layers, so the layer experiment's single-slot ceiling becomes aggregate heap capacity plus descriptor capacity instead of one texture's width or one frozen slot binding.

Requested work renders immediately: if a requested distinct-resource count exceeds a live descriptor or physical allocation wall, the page keeps the requested step selected, draws every requested object using the delivered resident set cyclically, and labels requested descriptors, delivered distinct descriptors, repeated descriptors, and the arithmetic that produced the wall.

## 3. Descriptor table

The descriptor table is one uniform buffer of 16-byte records represented as four little-endian `u32` words: word 0 packs `layer` in bits 0–15 and `x` in bits 16–31, word 1 packs `y` in bits 0–15 and `w` in bits 16–31, word 2 stores `h` in bits 0–15 with bits 16–31 reserved as zero, and word 3 stores heap kind in bit 0 (`0` DATA, `1` IMAGE) with bits 1–31 reserved as zero.

Width and height are in texels and must be 1–65,535, layer and origin coordinates must fit 16 bits, a zero width or height is the canonical free descriptor, and unknown reserved bits are a typed decode error rather than an invitation to guess.

The guaranteed 16-KiB UBO therefore holds 1,024 records; initialization requests at most 65,536 bytes when the adapter exposes it, so the prototype can deliver up to 4,096 distinct benchmark descriptors on common WebGL2 implementations while remaining honest on 1,024-record devices.

Descriptor slot zero is the permanent missing-resource descriptor, so live handles begin at index 1 and excess requested resources cycle through indices `1..delivered` while the page reports the repetition.

Beyond the 4,096-record prototype ceiling, the intended escape hatch is an RGBA32UI descriptor texture with the same four-word record and integer `textureLoad`; adopting it requires a new leaked-difference ledger entry and measurements because it replaces a uniform fetch with another texture fetch.

## 4. Handles and allocation

A handle is a `u32` with bits 0–19 as the descriptor index and bits 20–31 as the generation, allowing 1,048,576 addressable descriptor indices and 4,095 nonzero generations; raw zero is invalid, generation zero is never allocated, and the current UBO wall is intentionally smaller than the handle index space.

The allocator owns a descriptor free list and one square quadtree buddy allocator per physical layer and heap; an allocation rounds `max(w, h)` up to the next power-of-two size class from 1 texel through the heap side, takes the lowest-address block from the smallest class on the lowest layer, splits larger blocks into four Morton-ordered children, then rolls to the next layer only when no suitable block remains.

Freeing a region marks its descriptor unavailable, increments its 12-bit generation, returns the square to its layer, and recursively coalesces only when all four siblings are free; after generation 4,095 the descriptor index is retired for the process lifetime rather than wrapping and making an ancient handle valid again.

This policy has deterministic placement and complete buddy reclamation but no compaction: interleaved live blocks can prevent a large allocation despite sufficient total free texels, a rectangle can waste `class² − w×h` texels, and a 1×N region may reserve N² texels, so skinny structured data is expected to be tiled by its producer rather than hidden behind optimistic capacity claims.

Every CPU-side resolve, write, free, and descriptor query checks index allocation and generation in debug builds and returns the typed `HeapError::StaleHandle { handle, current_generation }` on use-after-free; release builds retain bounds and allocation checks but may omit the equality assertion on trusted hot paths.

The prototype deliberately does not perform shader-side generation checking because it would add a generation fetch to the kernel whose indirection cost is under measurement; the browser lab cannot manufacture handles except through the checked Rust allocator, and this evidence call is recorded as a lowering difference rather than described as free safety.

## 5. The one bind group

The heap path creates one bind group at initialization containing the DATA array and non-filtering sampler, IMAGE array and filtering sampler, descriptor UBO, and frame uniform buffer, and that bind group's resource identities and ranges never change.

Allocation, freeing, streaming, and animation mutate contents through regional `Queue::write_texture` and `Queue::write_buffer` calls, never through bind-group replacement.

Benchmark B transports handles in a static instance vertex buffer built at scene setup, while one small frame-uniform write supplies time and viewport state; it issues the same N small draw calls as the traditional arm so the comparison prices bind-group switches and dynamic writes rather than silently turning the heap arm into one giant draw.

The traditional arm intentionally switches a per-material bind group and performs one 16-byte material-uniform write per draw, whereas the heap arm binds once before its draw loop and performs no per-draw CPU upload; the state-diffing GL backend can therefore approach zero per-draw state changes only in the heap arm.

## 6. Dual-lowering posture

Handle encoding, generations, allocation results, logical descriptor fields, missing-resource behavior, and requested-versus-delivered accounting are substrate-independent.

WebGPU lowers the descriptor table to a read-only storage buffer today and may lower resource selection to `binding_array` when the web platform exposes it broadly; WebGL2 lowers the table to the UBO and resources to the two array textures, so only physical access changes.

The dual entry's leaked-differences ledger format is retained here: every divergence names the semantic invariant, both physical lowerings, the observable cost, and the conformance oracle.

|Semantic invariant|WebGL2 lowering|WebGPU lowering|Observable leak|Oracle|
|------------------|---------------|---------------|---------------|------|
|Descriptor lookup|16-byte UBO record|16-byte storage-buffer record|UBO capacity is at least 1,024 and commonly 4,096; storage capacity is much larger|Pack/decode round trip and identical sampled coordinates|
|Resource selection|Two fixed texture arrays|Texture arrays today; future binding arrays|Per-heap format/filter uniformity remains on WebGL2|CPU reference color for every descriptor edge and layer|
|Generation safety|CPU debug check; no shader fetch|CPU debug check; optional storage-buffer generation field|Shader stale-handle rejection is absent in this prototype|Allocator generation-reuse test and typed stale error|
|Completion|Four-byte MAP_READ fence plus explicit `device.poll` yield loop|Four-byte MAP_READ fence; browser backend normally progresses mappings|Polling semantics and latency differ; `on_submitted_work_done` is forbidden on browser WebGPU|Deadline-bounded fence completion and generation cancellation|
|Capacity|Live array, UBO, and configured byte walls|Live array and storage-buffer walls|Requested distinct resources may be delivered cyclically on WebGL2|Runtime wall arithmetic displayed beside every step|

## 7. Update paths and costs

DATA and IMAGE uploads are rectangular `write_texture` operations equivalent to `texSubImage3D` regions, descriptor edits are coalesced `write_buffer` ranges aligned to complete 16-byte records, and the frame uniform is one fixed-size write per rendered frame.

Executor decisions dated 2026-09-05 retain square-padded DATA uploads as the default and expose an opt-in valid-row path whose native oracle reduces a 4,096-record reference from 1,048,576 uploaded bytes to 65,536; browser layout and readback proof remains required before enablement. Selected dispatch headers are compared once per page range, metadata tables pack through retained executor scratch, resource-word publication does not repack metadata, and one bounded set of SCRATCH views plus fixed attachment slots is reused across pages.

Writes issued before a submission are ordered before its draws, but updating a region still read by queued work can serialize staging, driver copies, and raster work; large streaming uploads therefore target newly allocated regions and publish their descriptors only after the copy is queued, while in-place hot updates are labeled as potential stalls.

The uniforms-only-per-frame law forbids rebuilding dynamic descriptor or handle streams every frame: frame state alone is rewritten per frame, descriptors change on allocation or content relocation, and Benchmark B's heap handles live in a static instance buffer created at scene setup.

The traditional benchmark arm violates that austerity on purpose by writing 16 bytes per draw and reports `16×N` CPU-to-GPU bytes, while the heap arm reports its single 16-byte frame write; scene-setup texture, descriptor, and instance uploads are displayed separately and never amortized into zero.

All measured GPU completion uses a four-byte MAP_READ fence; the GL backend drives `map_async` with `device.poll` inside a cooperative browser yield loop, Benchmark A waits have a four-second deadline, Benchmark B waits have a thirty-second deadline for intentionally extreme single frames, every wait has a generation guard, and browser-WebGPU must not use `Queue::on_submitted_work_done` because that path is known to panic.

## 8. Benchmarks and evidence contract

Benchmark A renders a 512×512 RGBA8 target with one fullscreen triangle and 16 dependent nearest RGBA32F texel loads per fragment from a deterministic 64×64 payload, for 4,194,304 payload loads per base invocation; the direct arm reads a bound 2D texture, while the heap arm reads descriptor 1 once per fragment, maps the same 16 logical coordinates into a DATA array layer, and performs the same accumulation and color store.

Benchmark A uses three untimed warmups and 15 timed samples per arm, primary submit-to-four-byte-mapped-fence wall time, and adaptive batches that increase repeat count until each recorded batch spans at least 32 measured `performance.now()` quanta; results normalize batch milliseconds by repeat count and report median and p95, while timestamp-query results appear only as a separately labeled secondary series if the selected GL path actually exposes the feature.

Benchmark B uses one three-vertex triangle per draw, static per-instance position, scale, pseudo-mesh shape, and handle data, and the requested N values 16, 64, 256, 1,024, 4,096, 16,384, 65,536, 262,144, and 1,048,576; both arms issue exactly the delivered N `draw` calls into a 960×540 target, the traditional arm switches among delivered one-texel IMAGE textures and bind groups with one 16-byte write per draw, and the heap arm keeps one bind group while each instance handle resolves a one-texel IMAGE descriptor.

Benchmark B first fences exactly one frame; when that frame exceeds the 100 ms animation threshold it becomes the sole single-frame-on-demand observation, while a faster step continues through three warmups and 15 adaptive 32-quantum wall/fence samples per arm, with median, p95, and the design-decision metric `frame_ms × 1000 / delivered_N` microseconds per draw displayed live.

Benchmark B computes its draw delivery wall at runtime as `min(requested N, max_buffer_size / 32-byte instance record)`, separately computes delivered material variety as `min(delivered N, descriptor-backed material capacity)`, reports cyclic reuse and both arithmetic chains without moving controls, and reports frame bytes as `16 × delivered N` traditional versus 16 heap plus separately labeled scene-setup bytes.

The page changes step and mode immediately, renders the newly requested workload before opportunistic sampling, never uses a measurement as admission, never moves a control back to a delivered value, invalidates older async work with a generation counter, and leaves visible replay as the only source of browser numbers.

Native tests cover handle encode/decode, free-list reuse, every buddy size class, layer rollover, four-way reclamation, stale generation rejection, generation retirement, descriptor packing, and coordinate/color CPU references for both Benchmark A address paths; browser timing is not simulated or invented by tests.

## 9. What does not change

Renderer austerity remains one render pass, no mipmaps, and `cull_mode: None`; the benchmarks vary binding and descriptor access, not renderer ornament.

Heap contents feed presentation and prediction only and cannot author simulation, collision, protocol state, or reconciliation; a missing or stale resource changes pixels or produces a typed error, never gameplay truth.

Reports continue to distinguish requested, delivered, shown, submitted, and measured work, derive capacity walls from runtime facts, preserve controls across refusals or degradation, label browser values `requires visible replay`, and list unresolved limitations even when every automated gate is green.
