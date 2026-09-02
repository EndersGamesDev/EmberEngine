# Julibrot kernels slice contract

Status: first-round slice document for `crates/labs/julibrot/kernels`; the five slice documents must receive written joint review before implementation, and the app document is authoritative where an interface disagreement remains.

## 1. Ownership and boundary

The kernels slice owns GPU data-to-data work: the production shallow `f32` escape kernel, the production perturbation-and-rebasing kernel, their dialect-v2 registration and dispatch plans, one reusable escape-grid DATA span, SCRATCH-to-DATA landing, deterministic conformance fixtures, capacity arithmetic, and the definitions of the three refinement levels.

The kernels slice owns level definitions but not their schedule: app decides when or whether Preview, Interactive, and Final run, cancels stale work by the current owner epoch, and never relabels an unfinished level as delivered.

The kernels slice consumes the math slice's plane, centre split, CPU escape and perturbation oracles, and reference-orbit semantics; it neither constructs `Rₚ`, performs bignum navigation, nor chooses the bignum implementation.

The kernels slice consumes a reference-orbit DATA span produced through worker and owner state; it does not parse worker messages, own transfer buffers, grant credit, select latest-wins generations, or invent a same-thread transport.

The kernels slice exposes an `EscapeGrid` typed wrapper to present; it does not own palettes, flat or tumbled presentation, the standing VIEW rotation, warp, the three-slot hot ring, surface acquisition, or presentation fences.

The kernels slice does not create the GL device, install the panic hook or uncaptured-error handler, define the facts overlay, or own the application runtime; those are app duties and are preconditions of any kernels construction.

The heap dependency remains the shipped `ember-lab-heap` implementation: `DataSpan`, the span directory, descriptor handles, dialect v2, static dispatch headers, the immutable heap bind group, and the paid SCRATCH copy path are reused by dependency and are not copied into this package.

No kernel result authors simulation, collision, protocol, reconciliation, or gameplay truth; a result is presentation data and a stale or failed dispatch produces a typed refusal or stale visual, never a truth-state change.

## 2. Design and arithmetic

### 2.1 Coordinates, planes, and pixels

The fractal coordinates are ordered `(z.re, z.im, c.re, c.im) = (e₁,e₂,e₃,e₄)`; `e₅` carries escape height only in present's tumbled VIEW and never enters either fractal kernel.

The user-controlled PLANE rotation is `Rₚ(θ₁,θ₂) = R₁₂(θ₁)·R₃₅(θ₂)`, applied to column vectors as `R₁₂(R₃₅(v))` with the standard `[cos,−sin; sin,cos]` blocks and independent radian angles, then `u = P₄(Rₚe_a)` and `v = P₄(Rₚe_b)` are Gram–Schmidt re-orthonormalized by math after `P₄` drops `e₅`.

The standing VIEW rotation uses the same family only in present: `R(t) = R₁₂(t)·R₃₅(φt)`, `φ = (1+√5)/2`; kernels never apply it and therefore cannot accidentally rotate the fractal plane at presentation rate.

The Mandelbrot preset is basis `(e₃,e₄)`, origin `(0,0,0,0)`, and identity `Rₚ`; the Julia preset at `c₀` is basis `(e₁,e₂)`, origin `(0,0,c₀.re,c₀.im)`, and identity `Rₚ`, with `c₀` retained in MAIN state as part of the plane origin.

For an active grid of width `W` and height `H`, pixel `(i,j)` samples its centre with `x = f32(i)+0.5−0.5·f32(W)` and `y = f32(j)+0.5−0.5·f32(H)`, and its four-dimensional offset is `o = (x·u+y·v)·pixel_scale`; row zero is the bottom row, `+v` is up, and linear record index is `j·W+i`.

Square pixels use `pixel_scale = f32(4.0/(2^zoom_log2·W))`, where the division and exponentiation occur in `f64` on the CPU before the one `f32` conversion; the horizontal view extent is four scaled units and the vertical extent follows `H/W`.

Zoom depth is `zoom_log2·log10(2)` decimal digits, and the worker precision request is `ceil(zoom_log2·log10(2)+log10(W))+8` decimal digits converted by worker to its bignum precision bits; kernels report the supplied bits but do not recompute or reinterpret them.

The owner requests a new reference when the centre moves more than one quarter of the view extent or `zoom_log2` differs by more than two from the reference zoom, with worker-owned hysteresis; kernels only reject a reference whose published generation or logical length disagrees with the dispatch input.

### 2.2 Shallow escape kernel

The shallow kernel receives no heap input, receives only its 96-byte uniform, and writes one RGBA32F escape record per active pixel into the escape-grid span.

For coordinate component `k`, the absolute shallow sample is evaluated in the pinned `f32` order `p[k] = origin_hi[k] + (origin_lo[k] + o[k])`; `z₀ = (p[0],p[1])` and `c = (p[2],p[3])`.

Math supplies the split from the owner's `f64` mirror of the current centre, including preset origin: `origin_hi[k] = f32(C[k])` and `origin_lo[k] = f32(C[k]−f64(origin_hi[k]))`; the worker retains the authoritative bignum centre and the shallow GPU pair is not a deep-zoom absolute coordinate.

Complex multiplication is pinned as `mul(a,b) = (a.re·b.re−a.im·b.im, a.re·b.im+a.im·b.re)`, and the shallow recurrence uses `zₙ₊₁ = (zₙ.re²−zₙ.im²+c.re, 2·zₙ.re·zₙ.im+c.im)` in that source order.

For integer `n` from zero through `max_iter−1`, the kernel first tests the current `zₙ`; escape is the strict condition `|zₙ|² > bailout`, equality does not escape, and only a non-escaping state with another iteration remaining evaluates the recurrence.

At escape index `n`, `smooth_iter = n+1−log₂(log₂|zₙ|)`; the natural-log expansion is `n+1−ln(ln(|zₙ|)/ln(2))/ln(2)`, `escaped = 1`, `rebase_count = 0`, and `glitch = 0`.

At the iteration cap the shallow record is `[-1.0,0.0,0.0,0.0]`; `max_iter = 0` is a typed refusal and `bailout` is a squared radius fixed to exactly `256.0f32`, so `|z| > 16` escapes.

### 2.3 Perturbation and rebasing kernel

The perturbation kernel receives one reference-orbit DATA span through the descriptor table, receives no absolute origin, receives its 64-byte uniform, and writes the same escape-grid record as the shallow kernel.

The offset split is `δz₀ = (o[0],o[1])` and `δc = (o[2],o[3])`; Mandelbrot therefore has `δz₀ = 0` exactly when `u,v` lie in `span(e₃,e₄)`, while Julia has `δc = 0`.

Reference record `k` reconstructs `Zₖ = (re_hi+re_lo, im_hi+im_lo)` in `f32`, full pixel state is `zₙ = Zₖ+δₙ`, and the update is `δₙ₊₁ = 2·Zₖ·δₙ+δₙ²+δc` with complex products evaluated by the same pinned `mul` operation.

Each outer iteration first refuses an unavailable reference index as a glitch, then loads `Zₖ`, constructs `zₙ`, tests escape, tests rebase, and finally updates the delta and increments the reference index; escape therefore wins over rebase at the same state.

The binding rebasing rule is repeatable: when `|zₙ| < |δₙ|`, set `δ ← zₙ`, reset reference index `k ← 0`, and increment `rebase_count`, then evaluate the ordinary perturbation update against `Z₀` and advance `k` to one.

When `k` reaches reference `length` before escape or the outer iteration cap, iteration stops with `smooth_iter = −1.0`, `escaped = 0`, the accumulated integer-valued `rebase_count`, and `glitch = 1`; re-rendering those pixels with a second reference is explicitly out of scope and present uses the honest debug tint.

A non-escaping perturbation pixel that reaches `max_iter` records `[-1.0,0.0,rebase_count,0.0]`; all branch outputs are finite, so NaN and infinity are neither sentinel values nor accepted records.

### 2.4 Refinement, reuse, and latest-wins

There are exactly three kernel-defined levels: Preview is `ceil(W/4) × ceil(H/4)` with `min(requested_max_iter,64)`, Interactive is `ceil(W/2) × ceil(H/2)` with `min(requested_max_iter,256)`, and Final is `W × H` with `min(requested_max_iter,4096)`.

The value 4,096 is a tab-safety POLICY rather than a hardware wall; `RefinementPlan` retains both requested and delivered caps, and app may request a different future policy only after review rather than presenting 4,096 as detected capacity.

App owns scheduling and latest-wins cancellation: it may skip a level, but each level it does run is one logical dialect dispatch, and publication is accepted only when the dispatch's `owner_epoch` still equals the owner's current `u64` epoch.

The grid allocation reserves one span for the delivered Final extent and never reallocates between levels; each level densely overwrites the prefix of `width·height` records, `EscapeGrid.width`, `height`, and `level` are changed only after submission is accepted, and present never indexes beyond that active prefix.

The dialect lowering may require several page render passes for one logical level dispatch: for page side `q = 256`, active record count `N`, and `Q = q² = 65,536`, the dispatch uses `P = ceil(N/Q)` prefix pages, with the final dispatch header's `valid_length = N−(P−1)Q`.

Three sets of 16-byte `{global_base,valid_length,0,0}` page headers are generated and uploaded at grid allocation, each header begins at a stride aligned to live `min_uniform_buffer_offset_alignment`, and each page pass selects its header by dynamic offset without replacing the heap bind group.

If the requested Final extent does not fit after current live allocations, delivery chooses the smallest power-of-two divisor `d` for which an exact cloned-arena allocation of `ceil(W_requested/d)·ceil(H_requested/d)` records succeeds; those dimensions are delivered, and failure even at `1×1` is a typed zero-delivery refusal.

The allocation report keeps requested extent, delivered extent, divisor, logical bytes, reserved bytes, descriptor consumption, directory-handle consumption, and the first failing live wall separate; it does not infer free VRAM from exposed texture limits.

### 2.5 Output landing and cost model

RGBA32F is not blendable on the device floor, every fragment-compute target uses no blend state and `ColorWrites::ALL`, and both production kernels are gather-only with no scatter, atomics, barriers, workgroup variables, raw storage resources, author bindings, or author entry points.

The heap DATA array remains bound only as a sampled resource during a kernel pass; output first renders into one layer of the inherited four-layer RGBA32F SCRATCH array and is copied by `copy_texture_to_texture` into the destination DATA page before presentation reads it.

For a page with side `q` and `V` valid records, landing copies `floor(V/q)` complete rows and, when `V mod q` is nonzero, one exact-width tail row; there is no CPU readback or CPU copy between production and presentation.

For active `N` pixels, logical output is `B_logical = 16N` bytes, the one full-capacity span reserves `B_reserved = 16q²·ceil(WH/q²)` bytes, and each level moves `B_copy = 16N` GPU bytes; SCRATCH is reported separately as `B_scratch = DATA_side²·4·16` bytes and never counted as heap capacity.

At a 960×540 delivered Final extent and `q = 256`, the grid has 518,400 logical records, eight pages, 8,294,400 logical bytes, 8,388,608 reserved bytes, and 94,208 bytes of last-page padding; Preview is 240×135 and copies 518,400 bytes, Interactive is 480×270 and copies 2,073,600 bytes, and Final copies 8,294,400 bytes.

With the inherited 512-side SCRATCH array, its separately reported physical allocation is `512²·4·16 = 16,777,216` bytes; this number is configuration arithmetic, not a claim about free VRAM or a measured wall.

The per-level worst-case work is `C = width·height·iteration_cap` pixel-iterations; early escape and glitch reduce executed iterations but are unavailable without forbidden counters or an explicit measurement readback, so the overlay labels `C` as worst-case arithmetic and never reports it as measured work.

Perturbation performs at most one reference `textureLoad` per executed pixel-iteration, while shallow performs none; rebase does not add an outer iteration, so `rebase_count ≤ executed_iterations ≤ iteration_cap`.

The exact copy-command arithmetic is one command per complete page, plus one extra command only when the last partial page contains at least one complete row and a nonempty tail; command count, page-pass count, copied bytes, and encoder submissions remain separate facts.

## 3. INTERFACES

Every interface in this section is this slice's side of the shared contract and is intentionally duplicated in the other slice documents; disagreement is recorded for joint review and the app document decides integration.

### 3.1 Math to kernels: coordinate records

`Plane` is a 64-byte, 16-byte-aligned, little-endian `#[repr(C, align(16))]` value; adding `origin_hi` to the seed is required by the shared shallow hi-plus-lo ruling, while the worker's bignum centre remains outside this record.

|Byte range|Field|Type|Unit and meaning|
|---------:|-----|----|----------------|
|0–15|`basis_u`|`[f32; 4]`|Unit four-vector in `(z.re,z.im,c.re,c.im)` after projection and Gram–Schmidt|
|16–31|`basis_v`|`[f32; 4]`|Unit four-vector orthogonal to `basis_u` in the same axis order|
|32–47|`origin_hi`|`[f32; 4]`|High `f32` words of the shallow absolute centre, including preset origin|
|48–63|`origin_lo`|`[f32; 4]`|Residual `f32` words from the owner's `f64` centre mirror|

`EscapeParams` is the CPU-facing `#[repr(C)]` record `{ max_iter: u32, bailout: f32 }`, size eight and alignment four; `max_iter` is requested iterations, `bailout` is the squared-radius value and must be exactly `256.0f32`, and uniform packing supplies explicit padding rather than transmuting this record.

`GridExtent` is the CPU-facing `#[repr(C)]` record `{ width: u32, height: u32 }`, size eight and alignment four, measured in pixels; both fields must be nonzero and their checked product must fit `u32` before any allocation or dispatch.

### 3.2 Worker and owner to kernels: reference orbit

`ReferenceOrbitInput<'a>` is the borrowed record `{ span: &'a DataSpan, generation: u32, length: u32, precision_bits: u32 }`; `length` must equal `span.logical_len`, generation zero is valid only if worker defines it as the first session generation, and the caller must prove this is the latest accepted orbit before dispatch.

The underlying worker message has its independent eight-word little-endian `u32` header `{magic,version,generation,kind,length,precision_bits,compute_us,credit_us}` at bytes 0–31, followed at byte 32 by `length` reference records; kernels do not parse or retain this transport header, and `compute_us` and `credit_us` remain worker-owner facts in microseconds.

Reference-orbit texel `n` is one 16-byte RGBA32F record with index zero equal to `Z₀`, the centre's complex `z` part, with `length = min(max_iter,escape_index+1)` when the reference escapes and `length = max_iter` when it does not.

|Byte range|RGBA lane|Meaning|
|---------:|---------|-------|
|0–3|R|`re_hi`, the high `f32` word of `Zₙ.re`|
|4–7|G|`im_hi`, the high `f32` word of `Zₙ.im`|
|8–11|B|`re_lo`, the residual `f32` word of `Zₙ.re`|
|12–15|A|`im_lo`, the residual `f32` word of `Zₙ.im`|

The transferred payload is little-endian IEEE-754 binary32; owner uploads changed orbit records with regional writes into ordinary DATA pages, and the perturbation accessor resolves the span by `directory_index`, ordered descriptor handles, and each page descriptor's own width like every dialect-v2 DATA input.

### 3.3 Kernels to present: escape grid

`RefinementLevel` has `#[repr(u32)]` discriminants `Preview = 0`, `Interactive = 1`, and `Final = 2`; no numeric value is inferred for an unknown discriminant.

`EscapeGrid` is the typed Rust wrapper `{ span: DataSpan, width: u32, height: u32, level: RefinementLevel }`; `span.logical_len` is Final capacity, while `width·height` is the initialized dense prefix present may fetch for the current level.

Escape-grid texel `(i,j)` is record `j·width+i`, one 16-byte RGBA32F value with the following exact independent channels.

|Byte range|RGBA lane|Meaning|
|---------:|---------|-------|
|0–3|R|`smooth_iter: f32`, the specified smooth value at escape or exactly `−1.0` otherwise|
|4–7|G|`escaped: f32`, exactly `0.0` or `1.0`|
|8–11|B|`rebase_count: f32`, a nonnegative exactly representable integer, zero for shallow|
|12–15|A|`glitch: f32`, exactly `0.0` or `1.0`|

Present consumes `&EscapeGrid`, resolves its DATA span through the unchanged heap bind group, treats row zero as bottom and `+v` as up, displays every glitch in the honest debug tint, and never samples padding records from `span.logical_len−width·height`.

### 3.4 Uniform blocks

All scalar words below are little-endian, every block is `#[repr(C, align(16))]`, `Pod`, and 16-byte aligned, every padding word is written as zero, and native layout tests assert size, alignment, and each byte offset rather than trusting source declaration order.

The shallow block is exactly 96 bytes and is the only CPU-to-GPU payload of one shallow logical dispatch.

|Byte range|Field|Type|Meaning|
|---------:|-----|----|-------|
|0–15|`basis_u`|`[f32; 4]`|Plane basis `u`|
|16–31|`basis_v`|`[f32; 4]`|Plane basis `v`|
|32–47|`origin_hi`|`[f32; 4]`|Shallow centre high words|
|48–63|`origin_lo`|`[f32; 4]`|Shallow centre residual words|
|64–67|`pixel_scale`|`f32`|Four-dimensional units per pixel|
|68–71|`width`|`u32`|Active grid width in pixels|
|72–75|`height`|`u32`|Active grid height in pixels|
|76–79|`max_iter`|`u32`|Delivered level iteration cap|
|80–83|`bailout`|`f32`|Squared escape radius, exactly 256.0|
|84–87|`level`|`u32`|`RefinementLevel` discriminant|
|88–95|`padding`|`[u32; 2]`|Zero|

The perturbation block is exactly 64 bytes and is the only CPU-to-GPU payload of one perturbation logical dispatch; reference selection remains in the heap resource table and is not duplicated as a raw descriptor handle here.

|Byte range|Field|Type|Meaning|
|---------:|-----|----|-------|
|0–15|`basis_u`|`[f32; 4]`|Plane basis `u`|
|16–31|`basis_v`|`[f32; 4]`|Plane basis `v`|
|32–35|`pixel_scale`|`f32`|Four-dimensional units per pixel|
|36–39|`width`|`u32`|Active grid width in pixels|
|40–43|`height`|`u32`|Active grid height in pixels|
|44–47|`max_iter`|`u32`|Delivered level iteration cap|
|48–51|`bailout`|`f32`|Squared escape radius, exactly 256.0|
|52–55|`orbit_length`|`u32`|Number of valid reference records|
|56–59|`level`|`u32`|`RefinementLevel` discriminant|
|60–63|`padding`|`u32`|Zero|

The inherited per-page `DispatchHeader` is exactly 16 bytes `{ global_base: u32, valid_length: u32, padding: [u32;2] }`; the header buffer uses live dynamic-uniform stride and the heap bind-group identity never changes.

The inherited input resource entry is exactly 16 bytes `{ directory_index: u32, logical_len: u32, 0, 0 }`; shallow registers zero inputs, perturbation registers one accessor named `reference`, and out-of-range generated loads return canonical zero only after the explicit glitch check has prevented their use.

### 3.5 Refinement and dispatch records

`LevelSpec` is `{ level: RefinementLevel, extent: GridExtent, iteration_cap: u32 }` and `RefinementPlan` is `{ requested_extent: GridExtent, delivered_extent: GridExtent, extent_divisor: u32, requested_max_iter: u32, delivered_max_iter: u32, page_side: u16, levels: [LevelSpec;3] }`.

`KernelMode` has `#[repr(u32)]` discriminants `Shallow = 0` and `Perturbation = 1`.

`DispatchFacts` is `{ owner_epoch: u64, mode: KernelMode, level: RefinementLevel, requested_extent: GridExtent, delivered_extent: GridExtent, requested_max_iter: u32, delivered_max_iter: u32, active_pixels: u32, worst_case_pixel_iterations: u64, page_passes: u32, copy_commands: u32, gpu_copy_bytes: u64, logical_heap_bytes: u64, reserved_heap_bytes: u64, scratch_bytes: u64, orbit_generation: Option<u32>, orbit_length: u32 }`.

Every `DispatchFacts` byte and count is arithmetic from the accepted plan or a copied owner fact; GPU duration and poll count are deliberately absent because app measures submissions with its four-byte fence.

### 3.6 Public function surface

`JulibrotKernels::new(executor: &mut ember_lab_heap::GpuKernelExecutor) -> Result<JulibrotKernels, KernelError>` registers exactly two production dialect-v2 kernels and their immutable pipelines against the executor's immutable heap layout.

`JulibrotKernels::plan(executor: &ember_lab_heap::GpuKernelExecutor, requested_extent: GridExtent, params: EscapeParams) -> Result<RefinementPlan, KernelError>` performs checked extent arithmetic, applies the 4,096 policy, and uses exact cloned-arena allocation trials without mutation.

`JulibrotKernels::allocate_grid(executor: &mut ember_lab_heap::GpuKernelExecutor, plan: &RefinementPlan) -> Result<EscapeGrid, KernelError>` allocates one Final-capacity `DataSpan`, pre-uploads all three static prefix-header sets, and returns no partially allocated value on failure.

`JulibrotKernels::encode_shallow(&self, executor: &ember_lab_heap::GpuKernelExecutor, encoder: &mut wgpu::CommandEncoder, grid: &mut EscapeGrid, owner_epoch: u64, level: RefinementLevel, plane: &Plane, zoom_log2: f64, params: EscapeParams) -> Result<DispatchFacts, KernelError>` packs the 96-byte uniform, encodes page passes and exact-region copies, and publishes the active grid fields only for the accepted epoch.

`JulibrotKernels::encode_perturbation(&self, executor: &ember_lab_heap::GpuKernelExecutor, encoder: &mut wgpu::CommandEncoder, grid: &mut EscapeGrid, owner_epoch: u64, level: RefinementLevel, plane: &Plane, zoom_log2: f64, params: EscapeParams, reference: ReferenceOrbitInput<'_>) -> Result<DispatchFacts, KernelError>` performs the corresponding 64-byte uniform and one-span gather dispatch.

`JulibrotKernels::free_grid(executor: &mut ember_lab_heap::GpuKernelExecutor, grid: EscapeGrid) -> Result<(), KernelError>` returns the span and directory entries transactionally after app and present have relinquished all borrows.

The public error set is `KernelError::{InvalidExtent,ArithmeticOverflow,InvalidEscapeParams,UnknownLevel,StaleOwnerEpoch,MissingReference,StaleReference,ReferenceLengthMismatch,ReferencePrecisionMismatch,Heap,Register,Dispatch,OutputTransferUnsupported,DeviceLost}`; wrapped heap, registration, and dispatch diagnostics retain their original typed source and stable kernel name.

`GpuKernelExecutor` is a required minimal public seam extracted from the already-paid heap lattice runtime: it owns DATA and four-layer SCRATCH textures, `SpanArena`, immutable bind group, descriptor/directory/header/resource/uniform buffers, exact-row copy encoding, and live capacity reports; its extraction may expose existing behavior but must not fork or generalize the algorithm in this round.

### 3.7 App and present coordination

HOT state carries `zoom_log2: f64` and the two independent PLANE rotation angles in radians; app drains HOT every refresh, freezes one snapshot per frame, calls present-owned `Presenter::write_hot(slot,hot)` for one of three dynamic-offset slots, and passes the same snapshot values to a kernel dispatch selected by its schedule.

MAIN state carries the reference-orbit handle, requested iteration cap, plane origin including Julia `c₀`, and palette selection under the shared `u64` owner epoch; a MAIN arrival never rebuilds heap bind groups and changes only the orbit DATA region, resource words, uniforms, or palette record that actually changed.

Both HOT and MAIN drains are infallible and each bumps the shared epoch; an encoded dispatch with an older epoch may complete physically but cannot replace the published `EscapeGrid` level or facts.

Present's entry remains `Presenter::frame(state,hot_slot)` for flat and tumbled views, and warp remains `Warp::reproject(last_frame,from_pose,to_pose)` where pose contains plane, zoom, and rotation; neither call changes escape-grid records.

## 4. Inherited laws and satisfaction

|Law|Kernels-side satisfaction|
|---|-------------------------|
|WebGL2 floor|Pipelines are created only under wgpu 24 `Backends::GL`; RGBA32F render, copy, and nearest-sample support is a typed initialization requirement, and no WebGPU path exists.|
|Uniforms-only per frame|A logical dispatch uploads only its 96-byte or 64-byte uniform; orbit payload and heap metadata are regional writes only when generation, allocation, or extent changes, and static level headers are pre-uploaded.|
|GPU-resident output|Escape records render to SCRATCH, copy directly into DATA, and reach present by `DataSpan`; production performs no CPU readback.|
|Immutable binding identities|DATA, SCRATCH executor state, descriptor UBO, directory UBO, dispatch header buffer, resource buffer, and kernel uniform buffer are created before frames; dynamic header offsets and regional writes change contents only.|
|Hot ring|Present owns exactly three slots selected by dynamic offset; kernels consume the same frozen HOT snapshot through their own dispatch uniform and do not allocate a second hot ring.|
|No shared memory|The only deep input is an owner-uploaded transferred orbit span; no shared-memory worker path or same-thread special representation enters kernels.|
|Gather-only fragment compute|Each output fragment owns one linear pixel and may only load its matching reference indices; dialect rejection covers scatter-capable storage, atomics, barriers, bindings, and entry points.|
|No float blending|RGBA32F targets specify no blend state and overwrite all four lanes.|
|Honest facts|Requests, delivered extent and cap, policies, live walls, logical and reserved heap bytes, separate SCRATCH bytes, passes, copies, worst-case work, orbit metadata, and browser measurements are named separately.|
|Never hang|Every loop is bounded by the delivered cap of at most 4,096; app owns deadline-bounded four-byte fences, counted polls, cancellation, and visible failure publication.|
|Panic and GPU errors|App installs the panic hook and non-panicking uncaptured-error handler before constructing the executor; kernels return typed errors and contain no bare unreachable path.|
|Math evidence|Kernel tests consume hand-written CPU references; `faer` is absent unless the binding drift or warp oracle fails its `f64` threshold.|
|Scope austerity|There is one world, one heap class, no DAG or petgraph, no simulation tick, no shared-memory threads, no WebGPU, and no second-reference glitch repair.|

The external navigation oracle composes `10⁴` and `10⁵` steps of `R(Δθ)` with `Δθ = 10⁻³` radians and measures `‖MᵀM−I‖_F`; pass is at most `10⁻⁵` for `f64` without re-orthonormalization and for `f32` with Gram–Schmidt every 64 steps.

The external warp oracle requires `max|H⁻¹H−I| ≤ 10⁻⁹` in `f64` at `zoom_log2 ∈ {0,10,20,40,80,100}`; kernels gain no `faer` dependency unless the hand-written `f64` case fails.

## 5. Oracles and tests

Native layout tests assert `Plane = 64`, shallow uniform `= 96`, perturbation uniform `= 64`, reference and escape records `= 16`, every byte offset in §3, little-endian pack/unpack fixtures, zero padding, and exact enum discriminants.

Native coordinate tests cover both presets, rotated hybrid bases supplied by math, odd and even extents, bottom-left and top-right pixel centres, row-zero-at-bottom indexing, square pixels, the fixed `f64` pixel-scale calculation, and checked overflow.

Native refinement tests pin the three dimensions and caps, exact power-of-two extent degradation, one Final-capacity allocation, prefix page counts and last valid lengths, immutable span handles across all levels, requested-versus-delivered facts, and zero-delivery behavior.

Native heap-path tests register both bodies through dialect v2, prove shallow has zero accessors and perturbation has exactly `load_reference`, exercise alias and stale-generation refusals, pin dynamic header offsets, and verify copy rows, tail width, copied bytes, command counts, and separate SCRATCH accounting.

The shallow CPU conformance fixture uses deterministic pixels in both presets and one rotated hybrid plane; pass requires GPU and CPU escape classification and integer escape index to match exactly and `|smooth_gpu−smooth_cpu| ≤ 10⁻⁴`, with a conformance-only auxiliary target carrying the integer index because the production grid does not.

The perturbation CPU fixture uses `f64` deltas and deterministic pixels that include `δz₀ = 0`, `δc = 0`, both nonzero, zero rebases, repeated rebases, reference exhaustion, and nonzero `Z₀`; classification, rebase count, glitch, and integer escape index must match exactly, while the provisional smooth tolerance is `2×10⁻³` pending math's written error-bound review.

The production-output oracle checks every sampled record for finite channels, exact binary encodings of booleans and `−1.0`, integer-valued rebase count, zero shallow rebase count, and no read or presentation access beyond the active prefix.

The dialect source oracle parses and validates the generated WGSL, rejects every forbidden construct, proves input addressing uses the reference page descriptor rather than output width, and proves changing the reference span changes resource words without pipeline or bind-group replacement.

The shallow GPU readback, conformance-only integer target, perturbation GPU readback, vertex-stage consumption, SCRATCH copy at nonzero DATA origin and layer, row orientation, and a clean browser console all require visible replay.

The immutable-bind-group identity across orbit replacement and all three levels, live capability facts, exact copied-byte report, and absence of a DATA feedback-loop validation error require visible replay.

Dispatch walls, scene walls, and poll counts are browser facts measured by app around four-byte fences with no timestamp queries; every such number remains `requires visible replay` until that replay occurs.

## 6. Risks and retirement oracles

|Risk|Oracle that retires it|
|----|----------------------|
|The mandated rebase assignment may not preserve the perturbation invariant when `Z₀ ≠ 0`.|The nonzero-`Z₀` Julia fixture compares every pre- and post-rebase full value and recurrence against the `f64` CPU oracle; implementation cannot claim general-plane conformance until it passes or joint review changes the rule.|
|A provisional perturbation smooth tolerance could hide unstable deltas near the boundary.|Math supplies a written bound and adversarial deterministic pixels; exact classification, index, count, and glitch still cannot be waived, and any widened tolerance is a reviewed contract change.|
|Dense-prefix reuse could expose stale coarse or padding records.|Native index bounds plus visible replay of Preview→Interactive→Final and Final→Preview require checksums for only each declared active extent and poison padding in the test build.|
|Buddy fragmentation or directory exhaustion can defeat byte-only capacity estimates.|Every delivery uses an exact cloned-arena allocation, and fragmentation fixtures compare reported first wall with the typed allocator refusal.|
|Runtime-sized fragment loops can trigger a slow-driver watchdog despite the 4,096 policy.|Visible replay fences each level, counts polls, enforces app's deadline, and retains requested versus delivered caps; a timeout is failure evidence, never admission data.|
|Reference replacement could pair new metadata with old DATA records.|Generation-tagged upload fixtures delay publication until data and resource words are queued, then deliberately dispatch stale generations and require `StaleReference`.|
|Extracting the paid GPU executor could accidentally fork heap semantics.|An integration oracle runs the heap golden and Julibrot dispatch through the same executor type and compares bind-group identity, header bytes, copy regions, and typed failures.|
|RGBA32F or copy usage may be absent despite nominal WebGL2.|Initialization checks live format usages and the standing output-path golden; refusal names the adapter, backend, and failed usage.|
|Aggregate rebase and glitch totals are unavailable without readback or another reduction kernel.|The normal overlay labels totals unavailable and presents per-pixel debug tint; an explicitly requested measurement readback may count them and must report its fence and polls.|

## 7. Implementation phases and line budget

Phase 0, estimated 220 new lines, creates the kernels package, pins all shared records and layouts, exposes the two dialect descriptors, and adds source, packing, and coordinate tests.

Phase 1, estimated 300 new lines, implements the shallow CPU mirror and WGSL body, exact escape-index conformance instrumentation, shallow uniform packing, and deterministic native fixtures.

Phase 2, estimated 380 new lines, implements reference upload validation, perturbation and rebasing CPU mirror and WGSL body, glitch behavior, provisional tolerance fixtures, and the nonzero-`Z₀` blocking oracle.

Phase 3, estimated 340 new lines, implements the three-level plan, power-of-two delivered extent, full-span dense-prefix reuse, static header sets, capacity reports, and typed allocation errors.

Phase 4, estimated 320 new lines, integrates the public heap GPU executor seam, one logical dispatch over page passes, exact SCRATCH copies, bind-group identity evidence, and kernels-to-present publication.

Phase 5, estimated 260 new lines, adds browser conformance entry points, four-byte-fence handoff facts, visible replay cards, cancellation interleavings, and page-contract fixtures owned jointly with app.

Phase 6, estimated 180 new lines, reconciles all five documents, resolves or records interface differences, runs package and workspace gates, and accounts for actual net lines against the 2,000-line estimate.

The total implementation estimate is 2,000 net new Rust, WGSL, JavaScript fixture, and test lines; documentation and any minimal visibility-only heap seam are reported separately rather than hidden inside that estimate.

## 8. Unresolved joint-review findings

- The binding rule `δ ← z` followed by `k ← 0` makes the reconstructed value `Z₀+z`, not `z`, whenever `Z₀ ≠ 0`; the algebra-preserving assignment appears to be `δ ← z−Z₀`, so math and app must resolve this conflict before general-plane implementation can pass.

- `Plane` was seeded as 48 bytes with only `origin_lo`, while the shared ruling requires eight shallow-origin floats; this document pins a 64-byte `Plane` with both `origin_hi` and `origin_lo`, and joint review must confirm the same representation in math, present, and app.

- The `GpuKernelExecutor` behavior is paid inside heap's private lattice runtime but is not currently a public crate type; app must list the same minimal extraction seam before the implementation round, or the public signatures in §3.6 cannot compile without copying code.

- The `2×10⁻³` perturbation smooth tolerance is provisional because the math document owns the error argument; the written joint review must accept it, tighten it, or replace it with a condition-based bound before timing can be qualified.

- The 4,096 iteration ceiling is a kernels policy chosen to bound browser work, not a shared ruling; worker buffer sizing and app's displayed requested-versus-delivered cap must agree or record a cross-document difference.

- The inherited heap API builds headers for an entire `DataSpan`, while dense-prefix refinement needs a reviewed prefix-plan seam or a kernel-owned transformation of public `DispatchPlan` and `StaticHeaders`; mutation of header bytes by convention alone is too fragile.

- The worker header fixes field order but not numeric `magic`, `version`, or `kind` constants; kernels do not parse them, but app and worker must pin the values before the implementation round.

- `length = min(max_iter,escape_index+1)` needs one shared definition of `escape_index` at the cap boundary; this document defines current-state indices `0..max_iter−1`, and math and worker must match it byte-for-byte.

- Aggregate rebase and glitch totals cannot be produced by either gather-only production kernel without readback or a third reduction kernel; the app overlay must accept “unavailable” during normal rendering or explicitly schedule and label a measurement readback.

- A reference can legitimately end before a nearby pixel escapes, but no policy yet states whether a high glitch fraction requests a new centre reference; second-reference repair is out of scope, so the first lab may only expose the debug tint and measured limit.

- Exact shallow classification across CPU and browser shader depends on deterministic fixtures avoiding boundary-sensitive fused-operation differences; the fixture-selection margin must be written by math rather than selected after seeing GPU results.

- The power-of-two resolution degradation is deterministic but not yet shared with app or present; if app pins a different delivered-resolution policy, its integration contract supersedes this one and the difference must remain visible in joint review.
