# Julibrot worker slice

Status: round-one implementation and cross-slice interface contract for `crates/labs/julibrot/worker`; the five slice documents are reviewed together before implementation, and the app document is authoritative when joint review finds a disagreement.

## 1. Ownership

The worker slice owns the reference-orbit producer, the Web Worker entry point, ownership-transfer messaging, buffer credit and producer shaping, generation cancellation, the versioned `ViewerState` owner, the HOT and MAIN drains, the same-thread channel lowering, and native concurrency/accounting tests.

The worker slice depends on math for bignum operations and reference-orbit values; it does not choose the Julibrot algebra independently, implement perturbation or escape kernels, allocate heap spans, upload orbit records to the GPU, define refinement levels, schedule refinement, allocate the presentation hot ring, render either view, own the device or surface, or author application policy.

Kernels own refinement LEVELS, including grid sizes, iteration caps, and span reuse; app owns the SCHEDULE; present owns the palette record and the three-slot hot-ring GPU buffer; app chooses a palette through MAIN state and calls `Presenter::write_hot(slot, hot)` after each HOT drain.

The worker imports `crates/labs/heap` only through downstream typed seams where implementation needs its public handles; it never copies heap code, and any heap generalization remains an app-documented implementation-round seam under the visibility-only rule.

The general resource DAG and petgraph, more than one world, the simulation tick, more than one heap class, shared-memory threads, WebGPU, and second-reference repair of glitched pixels are deliberately absent.

## 2. Design

### 2.1 Coordinates, reference values, and precision

The common coordinate order is `(z.re, z.im, c.re, c.im) = (e₁,e₂,e₃,e₄)`; `e₅` carries escape height only in the tumbled VIEW and never enters the fractal plane or a worker message.

The user-controlled PLANE rotation is `Rₚ(θ₁,θ₂) = R₁₂(θ₁)·R₃₅(θ₂)`, applied to column vectors as `v' = R₁₂(R₃₅(v))` with the standard `[[cos,−sin],[sin,cos]]` block, radians, and independent angles; it is HOT state and frozen per frame.

The time-driven VIEW rotation uses the same family only in present's tumbled view, with `φ = (1+√5)/2` and `θ₂ = φθ₁`; it is not stored by this owner and cannot change the reference orbit.

Mandelbrot uses basis axes `(e₃,e₄)` and origin `(0,0,0,0)` with identity `Rₚ`, while Julia at `c₀` uses `(e₁,e₂)` and origin `(0,0,c₀.re,c₀.im)` with identity `Rₚ`; `c₀` is carried in MAIN's plane origin, a preset initializes the absolute centre `C` to that origin, and later navigation moves `C` within the plane without changing the defining origin.

Math forms `u = P₄(Rₚe_a)` and `v = P₄(Rₚe_b)`, drops `e₅`, and Gram–Schmidt re-orthonormalizes them under math's documented degenerate-case rule before narrowing the basis to `f32`.

The authoritative centre `C ∈ ℝ⁴` is decoded and held as a pure-Rust bignum in the worker; MAIN retains an `f64` mirror for controls, facts, and pose arithmetic but never supplies that mirror as deep-zoom truth.

For a grid of width `W`, `pixel_scale = 4/(2^zoom_log2·W)` in CPU `f64`, decimal depth is `zoom_log2·log10(2)`, required decimal precision is `p₁₀ = ceil(zoom_log2·log10(2)+log10(W))+8`, and requested bits are `precision_bits = ceil(p₁₀·log2(10))`.

Deep samples contain no absolute GPU origin: a pixel-centred offset is `δ = ((i+0.5−W/2)u + (j+0.5−H/2)v)·pixel_scale`, `δz₀` is its `(e₁,e₂)` part, and `δc` is its `(e₃,e₄)` part; `+v` is up, row zero is at the bottom, pixels are square, and `W×H` follows canvas aspect.

The Mandelbrot plane therefore has `δz₀ = 0`, the Julia plane has `δc = 0`, and every rotated plane follows the same perturbation interface without a special kernel.

The shallow path receives the origin as four `f32` high parts plus four `f32` low parts, followed by `u`, `v`, and `pixel_scale`; this is a math-to-kernels interface, not a worker payload.

The bignum reference starts at the centre's z component and holds the centre's c component fixed: `Z₀ = (C₀,C₁)`, `c = (C₂,C₃)`, and `Zₙ₊₁ = Zₙ²+c`.

Reference entry zero is `Z₀`; if escape is first observed at index `n`, stored length is `min(max_iter,n+1)`, and a non-escaping orbit stores exactly `max_iter` entries indexed `0..max_iter−1`.

Each high/low component is split without decimal formatting: `hi = round_f32(x)` and `lo = round_f32(x−exact(hi))`; the worker performs the sole wasm-to-standalone-buffer copy after math has filled its reusable linear-memory scratch.

Perturbation downstream is `δₙ₊₁ = 2Zₙδₙ+δₙ²+δc` with `δ₀ = δz₀` and `zₙ = Zₙ+δₙ`; when `|zₙ| < |δₙ|`, kernels set `δ ← zₙ`, reset the reference index to zero, and increment `rebase_count`, repeatably.

When a reference index reaches `length` before escape or `max_iter`, kernels set `glitch = 1`, stop that pixel, and present uses the honest debug tint; computing a second reference is out of scope and remains a displayed limitation.

The common squared bailout radius is `256.0`; at escape the grid stores `smooth_iter = n+1−log₂(log₂|zₙ|)`, while a non-escaping sample stores `−1.0`.

### 2.2 One-module worker packaging

One wasm module is loaded on the main thread and again in the Web Worker; the worker calls the exported `worker_main` entry, the browser cache avoids a second network payload, and the second instance still pays separate wasm linear memory, globals, initialization, and bignum scratch.

A second wasm artifact is rejected because it adds a separately versioned URL, duplicate code-generation output, cache identity, and loader failure mode without reducing `postMessage` payload bytes; the one-module choice makes deployment atomic even though instance memory cannot be shared.

Every wasm entry installs the readable panic hook before work; the app installs the non-panicking wgpu uncaptured-error handler before its first device call, and `worker_main` makes no device call.

Startup selects `WorkerMode::WebWorker` by default or `WorkerMode::SameThread` when the page query contains `worker=same-thread`; native tests always select `SameThread`.

### 2.3 Four-buffer transfer channel

The channel allocates two request-pool buffers and two orbit-pool buffers, four total, at startup; the two pools circulate independently, and transfer always calls `postMessage(buffer, [buffer])` so the sender is detached rather than structured-clone copying the payload.

Main initially owns both request buffers and transfers both orbit buffers to the producer; a request buffer moves main → worker as `OrbitRequest` and worker → main as `RequestReturn`, while an orbit buffer moves worker → main as `OrbitResponse` or `OrbitCancelled` and main → worker as `CreditApplied` or `CreditStale`.

The request pair permits one message to be in browser delivery while main overwrites the other with the newest request; the orbit pair permits one completed orbit to be uploaded and credited while the producer fills the other.

Each message kind has capacity one in its pending queue and a later message of the same kind replaces the earlier unstarted message; request-buffer returns and credit returns are ownership traffic and are never coalesced.

For current `max_iter = M`, every buffer has `capacity_bytes = 48+16M`: 32 header bytes, room for `M` orbit records or the request body, and a 16-byte immutable pool trailer; changing `max_iter` replaces all four buffers only after all four return to the allocator, increments `allocation_events`, and is the only steady-session resize event.

The request body must fit before the trailer, so `112+4·limb_word_count ≤ 32+16M`; failure is a displayed `CentreEncodingWall` with requested bytes and capacity, never truncation or a hidden allocation.

The main-to-worker boundary copies no orbit payload, and the worker-to-main boundary performs exactly one `O(16L)` memcpy from wasm linear scratch into the standalone orbit buffer before transfer; transfer and same-thread queue movement are `O(1)` ownership changes, so the path is `O(payload)` rather than `O(DAG)`.

The standalone buffer is returned only after app has synchronously handed its orbit bytes to kernels for a regional heap write and installed the resulting `OrbitHandle`; holding a transferred buffer across a frame is a channel bug visible as an outstanding-buffer count.

No Rust-managed buffer allocation occurs per message after startup; browser-internal task and transfer bookkeeping is outside that claim and is measured rather than inferred.

### 2.4 Generation, cancellation, and recompute hysteresis

`generation` is a checked, monotonically increasing `u32`; `checked_add` failure produces `GenerationExhausted` and stops new work, so wraparound is impossible within a session rather than handled by modular comparison.

An edit that requires a reference increments generation before publication, replaces the single pending orbit request, and invalidates older computation; an in-progress computation checks generation after each cooperative browser-task yield, returns no partial orbit, and reports `OrbitCancelled` with measured work when stale.

The compute loop checks elapsed wall after every iteration and yields after at most 64 iterations or 2,000 microseconds of measured worker CPU wall, whichever comes first; a yield schedules one browser task with zero-delay timer semantics, never a microtask-only yield.

Because posted edits are delivered at that task boundary, stale work is discarded at its next yield; owner validation repeats the generation check on receipt, so a delayed stale transferable can never publish.

Let `extent = 4/2^zoom_log2` in plane units and let `d` be the `f64` Euclidean distance between the latest centre mirror and the applied reference centre; the reference trigger trips when `d > extent/4` or `|zoom_log2−reference_zoom_log2| > 2`.

After a trigger, the worker remains disarmed while work is in flight and coalesces newer edits; it re-arms after an applied reference when `d ≤ extent/8` and `|zoom_log2−reference_zoom_log2| ≤ 1`, otherwise it immediately retains only the newest pending request, giving the thresholds explicit hysteresis.

### 2.5 Credit and producer shaping

App supplies the displayed POLICY `budget_us_per_second = B` at startup with `1 ≤ B ≤ 1,000,000`; the owner reports credit but never delays, rejects, or coalesces user edits on the producer's behalf.

The owner maintains a microsecond token bucket of capacity `B`; immediately before charging a returned computation at owner time `t`, `refilled = min(B, credit_previous + floor((t−t_previous)·B/1,000,000))`, `credit_us = max(0,refilled−compute_us)`, and `overfeed_us = max(0,compute_us−refilled)`.

Every completed or cancelled computation is charged, including stale work, because worker CPU time was consumed; wire-header validation, buffer-return transit, app upload, and main-thread rendering are excluded from `compute_us` and from this producer budget, while bignum centre decoding is included.

The producer projects a returned credit after local elapsed time `Δt` as `projected = min(B,returned_credit + floor(Δt·B/1,000,000))`.

For a fixed `max_iter`, the admission estimate `E` is the greatest nonzero `compute_us` observed since the last resize; exactly one first request after startup or resize is admitted as a labelled unpriced warm-up, and later work starts only when `projected ≥ E`.

When `projected < E`, producer delay is `ceil((E−projected)·1,000,000/B)` microseconds followed by one browser-task yield and recomputation; pending edits continue to coalesce during the delay.

At admission the producer subtracts `E` from its projected local balance, and the next owner return reconciles the estimate with measured cost; any actual excess is `overfeed_us`, displayed as a producer defect rather than repaired by owner throttling.

Worker `compute_us` begins immediately before decoding the centre into bignum scratch and ends after the one standalone-buffer copy, uses `ceil(1,000·performance.now elapsed milliseconds)`, and returns a typed `TimingOverflow` rather than saturating beyond `u32::MAX`.

### 2.6 Versioned owner and two drains

The owner is a hand-rolled, `Rc`-free and `RefCell`-free versioned swap made from `Cell` over `Copy` records; wasm32 has no shared-memory threads here, so its state operations are plain same-thread loads and stores, and no lock or borrow can fail.

HOT and MAIN stage independently but publish one coherent `ViewerState`; later writes replace earlier undrained writes, so latest wins without a queue.

`drain_hot` runs once every refresh, copies the newest HOT beside current MAIN, increments the common checked `u64` epoch, publishes the snapshot, and returns it infallibly even when HOT is unchanged.

`drain_main` runs on a MAIN arrival, where arrival means an accepted orbit, iteration-cap change, palette selection, or plane-preset/origin change; it copies newest MAIN beside current HOT, increments the same epoch, publishes the snapshot, and returns it infallibly.

An impossible `u64` epoch increment failure freezes publication with visible `EpochExhausted`; ordinary drains have no allocation, result, borrow, or refusal path.

App calls `Presenter::write_hot(frame_index % 3, drained.hot)` immediately after HOT drain; present owns allocation, alignment, and dynamic-offset selection for that three-slot buffer.

MAIN stores a session-local `OrbitHandle` only after kernels have synchronously accepted the response bytes and returned their typed heap-span wrapper; handle zero means no orbit, and app owns the registry from the compact owner ID to kernels' `ReferenceOrbit` wrapper.

## 3. INTERFACES

All wire integers and floats are little-endian; reserved fields and unoccupied bytes are zero on send and ignored only after version validation; all byte offsets below are from the start of the standalone `ArrayBuffer`.

### 3.1 Shared wire header and trailer

Every message starts with `MessageHeader`, exactly eight `u32` words and 32 bytes; `MAGIC = 0x314c424a` is the little-endian byte string `JBL1`, and `VERSION = 1`.

|Byte|Field|Type|Meaning|
|---:|-----|----|-------|
|0|`magic`|`u32`|`0x314c424a`|
|4|`version`|`u32`|wire version `1`|
|8|`generation`|`u32`|request generation, response generation, or generation being acknowledged|
|12|`kind`|`u32`|`MessageKind` discriminant|
|16|`length`|`u32`|kind-specific length from the table below|
|20|`precision_bits`|`u32`|requested or delivered bignum precision; zero when inapplicable|
|24|`compute_us`|`u32`|worker compute wall in microseconds; zero before computation|
|28|`credit_us`|`u32`|admission credit on producer output or remaining owner credit on return|

The last 16 bytes are `PoolTrailer { pool: u32, slot: u32, capacity_bytes: u32, trailer_magic: u32 }`, with `trailer_magic = 0x544c424a`, request `pool = 1`, orbit `pool = 2`, and `slot ∈ {0,1}`; the trailer is initialized only on allocation and must round-trip bit-exactly.

|Discriminant|Name|Direction and pool|`length` meaning|
|-----------:|----|------------------|----------------|
|1|`OrbitRequest`|main → worker, request|requested `max_iter`|
|2|`RequestReturn`|worker → main, request|zero|
|3|`OrbitResponse`|worker → main, orbit|stored orbit-entry count|
|4|`CreditApplied`|main → worker, orbit|zero; generation was installed|
|5|`CreditStale`|main → worker, orbit|zero; generation was discarded|
|6|`OrbitCancelled`|worker → main, orbit|zero; measured stale work is still charged|
|7|`ChannelError`|either direction in an owned buffer|four `u32` words in `ErrorRecord`|
|8|`Shutdown`|main → worker, request|zero|
|9|`ShutdownAck`|worker → main, request|zero|

`ErrorRecord` starts at byte 32 and is exactly `{ code: u32, detail: u32, requested_bytes: u32, available_bytes: u32 }`; stable codes are `1 BadMagic`, `2 BadVersion`, `3 BadKind`, `4 BadLength`, `5 BadTrailer`, `6 CentreEncodingWall`, `7 GenerationExhausted`, `8 EpochExhausted`, `9 TimingOverflow`, `10 BufferStarved`, and `11 MathFailure`.

### 3.2 Orbit request and bignum centre encoding

The Rust-level request is `OrbitRequest { generation: u32, centre: EncodedCentre, depth_digits: u32, precision_bits: u32, max_iter: u32, reason: OrbitReason }`; header words carry generation, precision, and max iteration, while the request body carries the remaining fields.

`depth_digits = ceil(max(0,zoom_log2·log10(2)))`; it is the integral request label, while the overlay retains the unrounded `f64` decimal-depth fact.

The request body is `{ depth_digits: u32, reason_bits: u32, centre_revision: u32, limb_word_count: u32, coordinates: [CoordinateDescriptor; 4], limbs: [u32; limb_word_count] }` with its fixed prefix at bytes 32 through 111 and limbs beginning at byte 112.

`reason_bits` assigns bit 0 to initial reference, bit 1 to centre-threshold crossing, bit 2 to zoom-threshold crossing, and bit 3 to max-iteration change; unknown bits are a version-one `BadLength` error rather than silently ignored.

Each 16-byte `CoordinateDescriptor` is `{ sign: u32, exponent_twos_complement: u32, limb_start: u32, limb_count: u32 }`; descriptors appear in `(z.re,z.im,c.re,c.im)` order at bytes 48, 64, 80, and 96.

A nonzero coordinate is exactly `(−1)^sign · (Σ limbs[limb_start+k]·2^(32k)) · 2^exponent`, limbs are least-significant first, `sign ∈ {0,1}`, the top stored limb is nonzero, and descriptor ranges are ordered, contiguous, non-overlapping, and cover `limb_word_count` exactly.

Canonical zero is `{ sign: 0, exponent: 0, limb_start: previous_end, limb_count: 0 }`; negative zero, leading zero limbs, unused limbs, and out-of-range descriptors are rejected.

The library-independent dyadic encoding lets dashu-float, astro-float, or a hand-rolled fixed-point adapter round-trip the same mathematical centre; math owns the adapter and precision semantics, while worker owns validation and transport.

### 3.3 Orbit response and credit return

`OrbitResponse` is the 32-byte header followed immediately by `length` reference records; used bytes are `32+16·length`, unused capacity before the pool trailer is zero, and `1 ≤ length ≤ max_iter`.

The high-level response view is `OrbitResponseView { generation: u32, length: u32, compute_us: u32, precision_bits: u32, admission_credit_us: u32, records: OrbitLease }`; `compute_ms()` is exactly `f64::from(compute_us)/1,000` and is a display conversion, not another measurement.

On return, main preserves `generation`, `precision_bits`, and `compute_us`, changes kind to `CreditApplied` or `CreditStale`, sets length to zero, and writes its newly computed `credit_us`; that header is the CREDIT record and states whether the named generation was applied.

`OrbitLease::return_credit(disposition, owner_now_us)` performs the owner accounting, updates facts, rewrites the header, and transfers the orbit buffer back exactly once; dropping a live lease is a debug failure and becomes `BufferStarved` plus a visible outstanding-buffer fact in release behavior.

### 3.4 Shared GPU records recorded on the worker side

`ReferenceOrbitRecord` is one little-endian RGBA32F texel and 16 bytes: byte 0 `re_hi: f32`, byte 4 `im_hi: f32`, byte 8 `re_lo: f32`, and byte 12 `im_lo: f32` for `Zₙ`.

`EscapeGridRecord` is one little-endian RGBA32F texel and 16 bytes: byte 0 `smooth_iter: f32`, byte 4 `escaped: f32`, byte 8 `rebase_count: f32`, and byte 12 `glitch: f32`; the last three are independently interpreted, `escaped` and `glitch` are exactly `0.0` or `1.0`, and `rebase_count` is integer-valued.

Kernels expose `EscapeGrid { span: DataSpan, width: u32, height: u32, level: u32 }`; `width·height` equals the span logical length, kernels define level contents, app schedules one dispatch per selected refinement level, and present consumes the wrapper without CPU readback.

Kernels expose `ReferenceOrbit { span: DataSpan, generation: u32, length: u32, precision_bits: u32 }` after the app's regional upload; app registers it under the worker's compact `OrbitHandle` and returns the standalone buffer immediately.

`EscapeParams` is `#[repr(C)] { max_iter: u32, bailout_squared: f32, _reserved: [u32; 2] }`, 16 bytes with offsets 0, 4, and 8; `bailout_squared` is exactly `256.0`.

The deep plane record is `#[repr(C)] Plane { basis_u: [f32; 4], basis_v: [f32; 4] }`, 32 bytes at offsets 0 and 16; the perturbation kernel has no origin field.

The shallow origin record is `#[repr(C)] ShallowOrigin { origin_hi: [f32; 4], origin_lo: [f32; 4] }`, 32 bytes at offsets 0 and 16; math computes it from the absolute centre `C`, not by adding MAIN's defining plane origin a second time, and does not narrow through the owner mirror.

### 3.5 Owner records and exact CPU layouts

All owner records below are `Copy` plus `#[repr(C)]`; they are compile-time Rust interfaces rather than message payloads, but their layouts are pinned so app and present cannot silently reorder the contract.

`HotState` is 24 bytes and alignment 8: `zoom_log2: f64` at byte 0, `plane_theta_1: f64` at byte 8, and `plane_theta_2: f64` at byte 16, all in radians except the dimensionless logarithm.

`MainState` is 104 bytes and alignment 8 with the following exact layout.

|Byte|Field|Type|Meaning|
|---:|-----|----|-------|
|0|`generation_applied`|`u32`|latest orbit generation installed; zero means none|
|4|`centre_revision`|`u32`|revision of authoritative encoded centre|
|8|`requested_iter_cap`|`u32`|application request|
|12|`delivered_iter_cap`|`u32`|kernel level actually installed|
|16|`precision_bits`|`u32`|delivered reference precision|
|20|`orbit_length`|`u32`|delivered reference records|
|24|`palette_id`|`u32`|present-owned palette selection|
|28|`orbit_id`|`u32`|app registry ID; zero means no orbit|
|32|`centre_f64`|`[f64; 4]`|owner mirror ordered `(z.re,z.im,c.re,c.im)`|
|64|`plane_axis_a`|`u32`|zero-based axis index in `e₁..e₄`|
|68|`plane_axis_b`|`u32`|zero-based axis index in `e₁..e₄`|
|72|`plane_origin_f64`|`[f64; 4]`|includes Julia `c₀`; display and pose mirror|

`OrbitHandle` is the logical pair `{ id: u32, generation: u32 }`; MAIN stores `id` and `generation_applied`, and app rejects a registry lookup whose generation differs.

`ViewerState` is 136 bytes and alignment 8: `epoch: u64` at byte 0, `hot: HotState` at byte 8, and `main: MainState` at byte 32.

`HotDrain` and `MainDrain` each return a full `ViewerState`; the distinct names make the update rate explicit even though their payload layout is identical.

### 3.6 Owner API

`ViewerOwner::new(initial: ViewerState) -> ViewerOwner` creates epoch zero with no pending edit and no failure path.

`ViewerOwner::stage_hot(hot: HotState)` replaces the undrained HOT value and performs no allocation.

`ViewerOwner::stage_main(main: MainState)` replaces the undrained MAIN value and performs no allocation; app uses it for palette, cap, and preset/origin arrivals.

`ViewerOwner::accept_orbit(response: &OrbitResponseView, handle: OrbitHandle) -> OrbitDisposition` returns `Applied` only when response generation equals the latest requested generation and handle generation matches it, stages the orbit fields into MAIN, and otherwise returns `Stale`; both outcomes are infallible and require the lease to return credit.

`ViewerOwner::drain_hot() -> HotDrain` publishes and returns the coherent snapshot every refresh, incrementing epoch once.

`ViewerOwner::drain_main() -> MainDrain` publishes and returns the coherent snapshot for each MAIN arrival, incrementing the same epoch once.

`ViewerOwner::snapshot() -> ViewerState` is an infallible copy that does not increment epoch and is diagnostic only; consumers act on drain returns, not polling snapshots for changes.

### 3.7 Channel API and same-thread lowering

`WorkerChannel::new(config: WorkerConfig, mode: WorkerMode) -> Result<(OwnerEndpoint, ProducerEndpoint), ChannelError>` allocates exactly four buffers and validates initial capacity; allocation or encoding-wall failure is typed initialization refusal.

`WorkerConfig` is `{ max_iter: u32, budget_us_per_second: u32 }`; `max_iter` must be nonzero, the implementation pins the buffer-return deadline to `4,000,000` microseconds, and max iteration plus budget remain displayed application policies.

`OwnerEndpoint::submit(request: OrbitRequest) -> SubmitOutcome` returns `Transferred`, `Coalesced`, or `GenerationExhausted`; it never blocks and retains only the latest untransferred orbit request.

`OwnerEndpoint::next_arrival() -> Option<OrbitResponseView>` is non-blocking on same-thread and event-driven on web; a response owns its `OrbitLease` until explicitly credited.

`OwnerEndpoint::shutdown()` sends `Shutdown`, stops accepting requests, waits at most the configured buffer-return deadline for four-buffer reconciliation, and reports any missing pool/slot instead of hanging.

`ProducerEndpoint::run(math: impl ReferenceOrbitComputer)` dispatches one request at a time, applies credit admission and cooperative cancellation, and stops only after returning `ShutdownAck` or a typed channel failure.

`ReferenceOrbitComputer::compute(centre, precision_bits, max_iter, bailout_squared, cancellation) -> Result<ComputedOrbit, MathFailure>` is supplied by math; `ComputedOrbit` exposes a reusable linear-memory slice of `ReferenceOrbitRecord`, delivered precision, escape index if any, and no transport allocation.

The Web Worker lowering backs endpoints with the four transferable `ArrayBuffer` objects; `SameThread` backs the identical ownership states with four preallocated byte buffers moved through bounded queues, bypasses the wasm-boundary memcpy only because producer and consumer share linear memory, and changes no ordering, generation, credit, or drain result.

### 3.8 Facts supplied to app

`WorkerFacts` is a 64-byte `#[repr(C)]` record: `epoch: u64` at byte 0, then `last_applied_generation`, `last_ack_generation`, `orbit_queue_depth`, `shutdown_queue_depth`, `credit_us`, `last_compute_us`, `last_overfeed_us`, `applied_count`, `stale_count`, `cancelled_count`, `allocation_events`, `request_buffers_owned_main`, `orbit_buffers_owned_main`, and `mode` as consecutive `u32` fields at bytes 8 through 60.

App displays queue depth, credits in microseconds, `compute_ms = last_compute_us/1,000`, owner epochs acknowledged, applied and acknowledged generations, stale/cancel counts, overfeed, buffer ownership, allocation events, and worker mode; zero and unavailable are distinct labels.

Requested generation, requested iteration cap, requested precision, and requested centre revision remain beside delivered generation, cap, precision, orbit length, and resolution; warm-up is labelled, no field is inferred from a policy or hardware wall, and no missing measurement becomes zero.

### 3.9 Presentation call and uniform ownership

The worker defines no GPU uniform block and performs no GPU call; present owns its WebGL2-compatible hot uniform, alignment, three-slot allocation, and dynamic offsets.

The shared call is `Presenter::write_hot(slot: u32, hot: HotState)`, where `slot ∈ {0,1,2}` and app supplies `frame_index mod 3`; present converts owner `f64` values to its pinned GPU representation and app uses the matching dynamic offset for that frame.

## 4. Inherited laws and satisfaction

The substrate law is satisfied because worker has no backend branch, shared-memory prerequisite, or GPU feature request; app alone creates wgpu 24 with `Backends::GL` over WebGL2 and checks `EXT_color_buffer_float` plus the documented minimums.

The traffic law is satisfied because HOT becomes one present-owned uniform write per refresh, MAIN changes only on arrival, reference-orbit bytes upload regionally only when the reference changes, and the escape grid never returns to CPU except explicit conformance measurement.

The heap law is satisfied because kernels receive the orbit through typed regional upload, kernel outputs land by heap's paid SCRATCH-to-DATA copy, heap bind-group identities never change, and worker creates neither bind groups nor alternate output paths.

The hot-ring law is satisfied by app draining HOT each refresh and calling `Presenter::write_hot(frame_index mod 3, hot)`; worker neither reallocates nor selects the presentation buffer.

The no-shared-memory law is satisfied by four-buffer ownership transfer and a credit header on the returning orbit buffer; same-thread is the bounded-queue lowering of the same protocol, not a behavioral special case.

The honesty law is satisfied by separate requested and delivered fields, live queue/credit/buffer facts, measured compute microseconds, displayed allocation and overfeed events, explicit warm-up, finite deadlines, checked generations, and `requires visible replay` labels for browser-only facts.

The panic/error law is satisfied by installing the hook at both wasm entries and by keeping all worker failures typed; app's non-panicking wgpu handler precedes the first device call, which the worker never makes.

The CPU-math law remains shared: navigation drift composes `10⁴` and `10⁵` steps of `R(Δθ)` with `Δθ = 1e−3` radians and measures `‖MᵀM−I‖_F`, passing at `≤ 1e−5` for `f64` without re-orthonormalization and for `f32` with Gram–Schmidt every 64 steps.

Warp accuracy requires `max|H⁻¹H−I| ≤ 1e−9` in `f64` for `zoom_log2 ∈ {0,10,20,40,80,100}`; faer enters only if hand-written `f64` fails either oracle, never because it is convenient for the worker.

The pixel and conformance laws are preserved downstream: shallow classification and integer iteration must equal CPU exactly with smooth value within `1e−4`, and perturbation uses exact classification plus math's argued `f64`-delta tolerance.

The timing law remains downstream for GPU work: scene and warp each measure wall around a four-byte fence, count polls, and use no timestamp query; worker `compute_us` is separately labelled CPU wall and is never substituted for either GPU cost.

The first-frame law remains app-owned: clear colour plus honest overlay text precedes the first frame, with no diagnostic pattern.

## 5. Oracles and tests

All tests in this section are native unless explicitly labelled `requires visible replay`; native tests use the same-thread lowering and injected monotonic clocks, never browser guesses.

The wire-layout golden constructs every message kind byte-for-byte, checks all offsets, endianness, reserved zeroes, canonical centre descriptors, record count arithmetic `32+16L`, trailer preservation, bad magic/version/kind/length rejection, and all typed errors.

The orbit-record golden checks index zero, escaping and non-escaping lengths, high/low reconstruction, squared bailout `256.0`, and exact `[re_hi,im_hi,re_lo,im_lo]` bytes supplied by deterministic math fixtures.

The ownership model enumerates every legal transfer of request slots zero and one and orbit slots zero and one, proving exactly one owner per slot, no send by a detached owner, exactly-once credit return, resize only after all four return, and bounded shutdown diagnostics for each missing slot.

Following heap `selection.rs`, the state-machine test places each newer edit at every yield point before and after request transfer, compute start, each cooperative yield, response transfer, app upload, orbit acceptance, HOT drain, and MAIN drain; every schedule ends on the newest generation, stale work never publishes, and both drains return without failure.

The drain interleaving test enumerates HOT and MAIN write permutations around both drain calls, proving HOT refresh cadence, MAIN arrival cadence, one shared strictly increasing epoch, coherent snapshots, latest-wins coalescing, and absence of a borrow/refusal path.

The credit arithmetic test exhausts boundary values for refill, capacity clamp, exact depletion, underflow, overfeed, timer advance, and `u32` conversion; a model token bucket and implementation must agree exactly in integer microseconds.

The shaping test proves one labelled unpriced warm-up per resize epoch, `E` as the observed maximum, the ceiling wait formula, coalescing while delayed, charge of cancelled work, and an overfeed fact whenever actual cost exceeds admission credit.

The mode-equivalence trace feeds identical requests, edit timings, clock ticks, cancellations, credits, and drains to transferable and same-thread abstract backends and requires identical messages, dispositions, epochs, facts, and final state modulo the wasm-copy counter.

The centre-trigger test covers equality and one-unit-beyond cases at `extent/4`, zoom delta `2`, re-arm thresholds `extent/8` and `1`, rotated-plane distance, and a newer edit arriving while disarmed.

`requires visible replay`: browser transfer proves `byteLength == 0` at every sender immediately after `postMessage`, trailer identity survives round trips, all four pool/slot pairs return, and managed allocation counters remain unchanged until a displayed max-iteration resize.

`requires visible replay`: module packaging records one versioned wasm URL fetched from cache for the worker load, two wasm instances with distinct linear memories, worker initialization wall, duplicated live memory, and no structured-clone payload path.

`requires visible replay`: cancellation sends edits before each scheduled browser-task yield, confirms acknowledgement of the final generation, observes no stale publication, counts yields, and meets the four-second buffer-return deadline without a hang.

`requires visible replay`: the page displays queue depth, credit microseconds, compute milliseconds, overfeed, allocation events, buffer owners, worker mode, owner epoch, requested generation, applied generation, and acknowledged generation, and cross-checks them against the trace log.

`requires visible replay`: the one-copy claim compares bytes written from reusable wasm scratch with bytes in the transferred standalone buffer and reports payload bytes, copy count, copy wall, and orbit length; it does not claim that browser or driver internals allocate nothing.

The shared navigation-drift, warp-accuracy, shallow-kernel, and perturbation-conformance oracles retain the exact thresholds in Section 4; worker tests consume their fixtures but do not weaken their pass criteria.

## 6. Risks and retiring oracles

|Risk|Consequence|Oracle that retires it|
|----|-----------|----------------------|
|Transfer accidentally clones or a detached buffer is reused|Payload-scaled main-thread cost or exception|Visible transfer replay checks sender detachment, trailers, and copy counter|
|A buffer is lost during cancellation, resize, error, or shutdown|Producer starvation or hang|Four-slot ownership model plus visible bounded-shutdown replay|
|Credit rounds or underflows differently between owner and producer|Bursting, permanent delay, or invented budget|Exact integer token-bucket and shaping model tests|
|Owner throttles instead of producer shaping|Input latency is hidden as application policy|Trace proves every edit is accepted/coalesced immediately and attributes delay to producer|
|A stale orbit becomes MAIN state|Wrong centre or reference reaches kernels|Every-yield interleaving test and visible cancellation replay|
|HOT and MAIN snapshots tear or a drain borrow fails|Pose and orbit belong to different versions|Exhaustive drain interleavings over Copy-cell owner|
|One-module packaging duplicates unacceptable memory|Tab memory pressure|Visible cached-fetch and two-instance memory report; app may reopen packaging only in joint review|
|Canonical centre bytes do not round-trip the selected math crate|Deep navigation drift or library lock-in|Per-candidate encode/decode property tests against dyadic fixtures|
|Centre bytes outgrow an orbit-sized request buffer|Deep request refusal at fixed iteration cap|Displayed `CentreEncodingWall` with exact byte inequality; joint review decides whether a fifth size class is justified|
|The two-millisecond yield target is missed by one expensive bignum iteration|Late cancellation and poor responsiveness|Visible per-chunk wall histogram and maximum edit-to-cancel acknowledgement|
|`performance.now` is coarse or quantized|Zero or biased compute and credit facts|Visible timer-quantum report and integer-clock native tests; unavailable stays labelled|
|App returns an orbit buffer before regional upload consumes it|Corrupted GPU reference|OrbitLease ordering test plus visible checksum after immediate buffer return|
|Compact orbit registry reuses an ID across generations|Wrong heap span selected|Registry generation mismatch test and typed refusal|
|Generation or epoch approaches exhaustion|Modular latest-wins failure|Checked increment boundary tests; no wrapping operation exists|
|Worker panic or channel failure yields a blank wait|Unreadable failure or hang|Panic-hook test, typed error golden, and four-second main-side deadline|

## 7. Implementation phases and line budget

Phase 0 adds the package shell, pinned records, canonical codec, four-slot ownership model, typed failures, and wire-layout goldens, estimated at 360 Rust and test lines.

Phase 1 adds the same-thread bounded queues, generation/coalescing state machine, Copy-cell owner, two drains, exhaustive interleavings, and compact orbit registry seam, estimated at 430 lines.

Phase 2 adds the math adapter, reusable bignum/orbit scratch, reference computation, high/low packing, cooperative task yields, cancellation, and deterministic fixtures, estimated at 420 lines.

Phase 3 adds the wasm `worker_main`, transferable backend, one-copy bridge, four-buffer resize/reconciliation, shutdown deadline, panic reporting, and page-flag selection, estimated at 390 Rust and JavaScript lines.

Phase 4 adds credit/token-bucket shaping, facts snapshots, app/kernels/present integration seams, native trace equivalence, and visible-replay instrumentation, estimated at 360 lines.

The worker slice is therefore budgeted at about 1,960 implementation and test lines; generated wasm glue and downstream app, kernel, heap, and presentation code are excluded.

## 8. Unresolved joint-review findings

- Math must select dashu-float, astro-float, or a justified fixed-point implementation and prove that its adapter preserves the canonical dyadic centre at requested precision; worker transport does not select the arithmetic crate.
- The authoritative centre lives decoded only in the worker while `OrbitRequest` carries an absolute canonical centre and MAIN carries only an `f64` mirror; joint review must pin which math/app navigation API produces updated canonical bytes without creating a second authoritative bignum on main.
- App must choose and display the initial `budget_us_per_second`; this contract pins its range and arithmetic but does not invent a workload-independent default.
- Present must publish the palette record and stable `palette_id` registry semantics; worker pins only the `u32` selection carried in MAIN.
- Kernels and app must reconcile the compact `OrbitHandle` registry with kernels' full `ReferenceOrbit { span, generation, length, precision_bits }` lifetime, especially span reuse between refinement levels.
- The orbit-sized request-pool rule creates an honest `CentreEncodingWall` when canonical centre limbs exceed available record bytes at current `max_iter`; joint review must accept that live wall or explicitly add another buffer size class before implementation.
- The 64-iteration or 2,000-microsecond yield check cannot pre-empt one bignum iteration; visible replay must determine whether the one-iteration worst case needs finer math-level cancellation checks.
- Browser transfer APIs can prove sender detachment and trailer continuity but not physical zero-copy inside the browser; the document claims ownership transfer and one explicit wasm memcpy, not an unobservable engine implementation.
- `compute_us` includes centre decode and the wasm-to-standalone copy but excludes transit and upload; joint review must ensure every overlay uses that exact boundary rather than calling it end-to-end orbit latency.
- Same-thread mode deliberately skips the wasm-boundary memcpy while preserving messages and state; performance comparisons must label the mode and may not compare its compute wall to Web Worker wall as though the boundary were identical.
