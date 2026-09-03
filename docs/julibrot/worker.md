# Julibrot worker slice

Status: refined implementation and cross-slice interface contract for `crates/labs/julibrot/worker` after the written joint review; the five refined slice documents are reviewed together before implementation, and the app document is authoritative if a disagreement remains.

## 1. Ownership

The worker slice owns the reference-orbit producer, the Web Worker entry point, ownership-transfer messaging, buffer credit and producer shaping, generation cancellation, the versioned `ViewerState` owner, the HOT and MAIN drains, the same-thread channel lowering, and native concurrency/accounting tests.

The worker slice depends on math for bignum operations and reference-orbit values; it does not choose the Julibrot algebra independently, implement perturbation or escape kernels, allocate heap spans, upload orbit records to the GPU, define refinement levels, schedule refinement, allocate the presentation hot ring, render either view, own the device or surface, or author application policy.

Kernels own refinement LEVELS, including grid sizes, iteration caps, and span reuse; app owns the SCHEDULE; present owns the palette record and the three-slot hot-ring GPU buffer; app chooses a palette through MAIN state and lowers each HOT drain through present's `HotSlot` and `write_hot` API.

The worker imports `crates/labs/heap` only through downstream typed seams where implementation needs its public handles; it never copies heap code, and any heap generalization remains an app-documented implementation-round seam under the visibility-only rule.

The general resource DAG and petgraph, more than one world, the simulation tick, more than one heap class, shared-memory threads, WebGPU, and second-reference repair of glitched pixels are deliberately absent.

## 2. Design

### 2.1 Coordinates, reference values, and precision

The common coordinate order is `(z.re, z.im, c.re, c.im) = (e₁,e₂,e₃,e₄)`; `e₅` carries escape height only in present's height field and never enters the fractal plane or a worker message.

The user-controlled PLANE rotation acts only in `ℝ⁴` and mixes the z and c subspaces: `Rₚ(θ₁,θ₂) = R₁₃(θ₁)·R₂₄(θ₂)`, applied to column vectors as `v′ = R₁₃(R₂₄(v))` with the standard `[[cos,−sin],[sin,cos]]` block, radians, and independent angles; it is HOT state and frozen per frame.

The VIEW rotation is present-only and reads no clock: `Rᵥ(θᵥ₁,θᵥ₂) = R₁₂(θᵥ₁)·R₃₅(θᵥ₂)`, both angles independent controls in radians; it acts only in present's height field, is not stored by this owner, and cannot change the reference orbit.

Mandelbrot uses seed axes `(e₃,e₄)` and origin `(0,0,0,0)` with identity `Rₚ`, while Julia at `c₀` uses seed axes `(e₁,e₂)` and origin `(0,0,c₀.re,c₀.im)` with identity `Rₚ`; `c₀` is carried in MAIN's plane origin, a preset initializes the absolute centre `C` to that origin, and later navigation moves `C` within the plane without changing the defining origin.

For seed axes `(e_a,e_b)`, math computes `u = Rₚe_a` and `v = Rₚe_b` in `f64` and rounds each component once to `f32`; no projection, Gram–Schmidt, or degenerate-plane stage exists because an orthogonal map preserves the seed pair's orthonormality, and the post-rounding oracle requires `|u·u−1|`, `|v·v−1|`, and `|u·v|` each at most `8·f32::EPSILON`.

At `θ₁ = θ₂ = π/2` the Mandelbrot seed becomes the Julia plane at the current centre; for `0 < θ₁,θ₂ < π/2`, the hybrid oracle requires the rotated basis to have nonzero components in both `span(e₁,e₂)` and `span(e₃,e₄)`.

The authoritative centre `C ∈ ℝ⁴` is decoded and held as a pure-Rust bignum in the worker; MAIN retains an `f64` mirror for controls, facts, and pose arithmetic but never supplies that mirror as deep-zoom truth.

For a grid of width `W`, the conceptual `pixel_scale = 4/(2^zoom_log2·W)` stays in CPU bignum or `f64` arithmetic and is factored as `pixel_scale = m·2^s`, with one round-to-nearest `m: f32` in `[0.5,1)` and `s: i32`; no absolute tiny scale is formed in `f32`.

Displayed zoom depth is `zoom_digits = zoom_log2·log10(2)` and `depth_digits = ceil(max(0,zoom_digits))`; precision uses `D_floor = ceil(zoom_log2·log10(2)+log10(W))+8`, `D_work = D_floor+ceil(log10(max(max_iter,1)))`, and `precision_bits = ceil(D_work·log₂(10))`, rounded upward by math to a 64-bit boundary before it reaches Astro-float, so the delivered precision is the same number in the native gate and in the browser, where Astro-float's own word is 32 bits.

The default `Deterministic` policy validates each reference by recomputing at `D_work+16` decimal digits. `PictureFast` Preview instead publishes the single working-precision orbit immediately and marks verification deferred; PictureFast Final and Measure compute the verification orbit, require the same escape index plus both GPU-consumed coordinate words within two `f32` ulps, publish the maximum word error and escalation count, and raise `D_work` in 16-digit steps through the displayed 300-digit POLICY before re-issuing Final, with exhaustion returned as `PrecisionExhausted` rather than silently accepting an unstable orbit.

Deep samples contain no absolute GPU origin: centred pixel coordinates are `x = i+0.5−W/2` and `y = j+0.5−H/2`, the scaled offset is `o′ = (x·u+y·v)·m`, the per-pixel exponent begins at `e₀ = s`, `δz₀′` is the `(e₁,e₂)` part of `o′`, and `δc′` is its `(e₃,e₄)` part; `+v` is up, row zero is at the bottom, pixels are square, and `W×H` follows canvas aspect.

The Mandelbrot plane therefore has `δz₀′ = 0`, the Julia plane has `δc′ = 0`, and every rotated plane follows the same scaled perturbation interface without a special kernel.

The shallow path receives `CentreSplit` as four `f32` high parts plus four `f32` low parts, followed by `u`, `v`, and scalar `pixel_scale`; the perturbation kernel is selected when `zoom_log2 ≥ 14`, a displayed POLICY whose `f32` error argument belongs to math. Below the switch the owner accepts the latest centre revision without producing or transferring an orbit; a crossing to the deep side releases a normal reference request and waits for acceptance before perturbation.

Scaled perturbation does not change the hot ring or warp: present computes zoom ratios as `2^(zoom_log2_to−zoom_log2_from)` in `f64` and never divides two tiny absolute scales.

The bignum reference starts at the centre's z component and holds the centre's c component fixed: `Z₀ = (C₀,C₁)`, `c = (C₂,C₃)`, and `Zₙ₊₁ = Zₙ²+c`.

Reference entry zero is `Z₀`; if escape is first observed at index `n`, stored length is `min(max_iter,n+1)`, and a non-escaping orbit stores exactly `max_iter` entries indexed `0..max_iter−1`.

Each reference coordinate is narrowed without decimal formatting as `round_f32(x)`. The production-shaped PL-05 fixture proved that its old residual never changed a consumed coordinate word, Final/Measure validation may escalate the source precision, and the worker performs the sole wasm-to-standalone-buffer copy after math has filled reusable linear-memory scratch.

Scaled perturbation carries `δ′ = δ/S` with `S = 2^e` and iterates `δ′ₙ₊₁ = 2Zᵣδ′ₙ+S·δ′ₙ²+δc′`, with `δ′₀ = δz₀′`, `δc′ = δc/S`, and full value `zₙ = Zᵣ+S·δ′ₙ` evaluated through `ldexp`.

When `|δ′| > 2⁶⁴`, kernels set `δ′ ← δ′·2⁻⁶⁴`, `δc′ ← δc′·2⁻⁶⁴`, and `e ← e+64`; when `0 < |δ′| < 2⁻⁶⁴`, they set `δ′ ← δ′·2⁶⁴`, `δc′ ← δc′·2⁶⁴`, and `e ← e−64`, so both `S·δ′` and `S·δc′` remain invariant and zero never enters a renormalization loop.

REBASING occurs after escape testing when `|zₙ| < |S·δ′ₙ|`; kernels reconstruct `Z₀` from reference record zero, set `δ′ ← (zₙ−Z₀)/S`, reset reference index `r ← 0`, increment `rebase_count`, and perform exactly one ordinary advance against `Z₀`, so the invariant `zₙ = Zᵣ+S·δ′ₙ` holds for nonzero `Z₀` as well as zero.

If forming `S·δ′` underflows during the rebase predicate, the predicate is false because the delta is negligible; equality does not rebase, and the predicate is not repeated at the same global iteration.

When a reference index reaches `length` before escape or `max_iter`, kernels set `glitch = 1`, stop that pixel, and present uses the honest debug tint; computing a second reference is out of scope and remains a displayed limitation.

The common squared bailout radius is `256.0`; at escape the grid stores `smooth_iter = n+1−log₂(log₂|zₙ|)`, while a non-escaping sample stores `−1.0`.

### 2.2 One-module worker packaging

One wasm module is loaded on the main thread and again in the Web Worker; the worker calls the exported `worker_main` entry, all loader URLs retain the independently pinned `?v=1`, exported `JULIBROT_ABI_VERSION = 3` must equal the message version before startup, the browser cache avoids a second network payload, and the second instance still pays separate wasm linear memory, globals, initialization, and bignum scratch.

A second wasm artifact is rejected because it adds a separately versioned URL, duplicate code-generation output, cache identity, and loader failure mode without reducing `postMessage` payload bytes; the one-module choice makes deployment atomic even though instance memory cannot be shared.

Every wasm entry installs the readable panic hook before work; the app installs the non-panicking wgpu uncaptured-error handler before its first device call, and `worker_main` makes no device call.

Startup selects `WorkerMode::WebWorker` by default or `WorkerMode::SameThread` when the page query contains `worker=same-thread`; on wasm32 `WorkerChannel::new` constructs a module Worker at `./worker.js?v=1`, waits for ABI acceptance, and then transfers the initial orbit pair, while native tests exercise the deterministic queue lowering for both mode tags.

### 2.3 Four-buffer transfer channel

The channel allocates two request-pool buffers and two orbit-pool buffers, four total, at startup; the two pools circulate independently, transfer always calls `postMessage(buffer, [buffer])` so the sender is detached rather than structured-clone copying the payload, and the Rust binary listener coexists with the JavaScript object-handshake listener rather than replacing it.

Main initially owns both request buffers and transfers both orbit buffers to the producer; a request buffer moves main → worker as `OrbitRequest` and worker → main as `RequestReturn`, while an orbit buffer moves worker → main as `OrbitResponse` or `OrbitCancelled` and main → worker as `CreditApplied` or `CreditStale`.

The request pair permits one message to be in browser delivery while main overwrites the other with the newest request; the orbit pair permits one completed orbit to be uploaded and credited while the producer fills the other.

Each message kind has capacity one in its pending queue and a later message of the same kind replaces the earlier unstarted message; request-buffer returns and credit returns are ownership traffic and are never coalesced.

For current `max_iter = M`, every buffer has `capacity_bytes = max(644,64+8M)`: 32 header bytes, room for `M` orbit records or the request body, a 16-byte orbit-verification tail, and a 16-byte immutable pool trailer; app's minimum requestable `M` is 64, changing `M` arms one drain of all four buffers, delivers a queued arrival to app for its own stale disposition rather than swallowing it behind app's one-in-flight coalescing, replaces all four buffers only after all four return to the allocator or the four-second return deadline expires, restarts the producer from the cached module artifact, increments `allocation_events`, re-encodes the coalesced request at the new capacity, and is the only steady-session resize event.

The request body must fit before the trailer, so `116+4·limb_word_count ≤ capacity_bytes−16`; at the 300-digit POLICY four coordinates need at most `4·ceil(300·log₂(10)/32) = 128` limbs, hence request bytes are at most `116+4·128 = 628`, exactly the usable request region at the 644-byte floor, while any failure remains a displayed `CentreEncodingWall` with requested bytes and capacity, never truncation or a hidden allocation.

The main-to-worker boundary copies no orbit payload, and the worker-to-main boundary performs exactly one `O(16L)` memcpy from wasm linear scratch into the standalone orbit buffer before transfer; transfer and same-thread queue movement are `O(1)` ownership changes, so the path is `O(payload)` rather than `O(DAG)`.

The standalone buffer is returned only after app has synchronously handed its orbit bytes to kernels for a regional heap write and installed the resulting `OrbitHandle`; holding a transferred buffer across a frame is a channel bug visible as an outstanding-buffer count.

No standalone transport-buffer allocation occurs per message after startup; canonical-centre and Astro-float semantic storage remains ordinary wasm linear-memory work storage, browser-internal task and transfer bookkeeping is outside the transport claim, and both are measured rather than inferred.

### 2.4 Generation, cancellation, and recompute hysteresis

`generation` is a checked, monotonically increasing `u32`; `checked_add` failure produces `GenerationExhausted` and stops new work, so wraparound is impossible within a session rather than handled by modular comparison.

An edit that requires a reference increments generation before publication, replaces the single pending orbit request, and invalidates older computation; an in-progress computation checks generation after each cooperative browser-task yield, returns no partial orbit, and reports `OrbitCancelled` with measured work when stale.

A latest edit below the deep switch does not require a reference: app retires any older deep submission from its wait set, takes the newest coalesced navigation snapshot, and calls the owner's orbit-free acceptance. The owner publishes its generation and centre revision with zero orbit metadata while retaining centre displacement against the prior reference basis, so shallow rendering starts without a request, transfer, or credit dependency and a later deep crossing still waits for a matching new orbit.

The compute loop checks elapsed wall after every iteration and yields after at most 64 iterations or 2,000 microseconds of measured worker CPU wall, whichever comes first; a yield schedules one browser task with zero-delay timer semantics, never a microtask-only yield.

Because posted edits are delivered at that task boundary, stale work is discarded at its next yield; owner validation repeats the generation check on receipt, so a delayed stale transferable can never publish.

The worker endpoint projects the bignum difference `C−C_ref` onto current `u,v`, divides those bignum components by conceptual `pixel_scale`, and only then converts the dimensionless result to `centre_from_reference_px: [f64;2]`; this ratio is representable at arbitrary depth even when neither absolute centre difference nor scale is representable in `f64`.

Let `d_px = hypot(centre_from_reference_px[0],centre_from_reference_px[1])`; the reference trigger trips when `d_px > grid_width/4` or `|zoom_log2−reference_zoom_log2| > 2`, exactly the centre displacement of one quarter of the width extent without a tiny absolute subtraction.

After a trigger, the worker remains disarmed while work is in flight and coalesces newer edits; it re-arms after an applied reference when `d_px ≤ grid_width/8` and `|zoom_log2−reference_zoom_log2| ≤ 1`, otherwise it immediately retains only the newest pending request, giving the thresholds explicit hysteresis.

On acceptance, worker publishes `reference_shift_px = project(C_ref_new−C_ref_old)/pixel_scale_current` in MAIN and recomputes HOT's `centre_from_reference_px` against the new accepted reference; present re-bases the retained pose by that shift and clears only when `max_iter` or the defining plane origin `c₀` changes, not merely because orbit generation changed.

For panning warp, present's translation term is the from-pose displacement minus the to-pose displacement in pixels after applying any accepted `reference_shift_px`; this preserves smooth motion while the newest orbit and scene are pending without treating warped pixels as new fractal truth.

### 2.5 Credit and producer shaping

The implementation constant `ORBIT_BUDGET_US_PER_SECOND` fixes the displayed POLICY `B = 250,000` microseconds per second; changing it requires an implementation-constant edit, and the owner reports credit but never delays, rejects, or coalesces user edits on the producer's behalf.

The owner maintains a microsecond token bucket of capacity `B`; immediately before charging a returned computation at owner time `t`, `refilled = min(B, credit_previous + floor((t−t_previous)·B/1,000,000))`, `credit_us = max(0,refilled−compute_us)`, and `overfeed_us = max(0,compute_us−refilled)`.

Every completed or cancelled computation is charged, including stale work, because worker CPU time was consumed; wire-header validation, buffer-return transit, app upload, and main-thread rendering are excluded from `compute_us` and from this producer budget, while bignum centre decoding is included.

The producer projects a returned credit after local elapsed time `Δt` as `projected = min(B,returned_credit + floor(Δt·B/1,000,000))`.

For a fixed `max_iter`, the admission estimate `E` tracks measured cost with immediate forward pressure upward and a halving decay downward: a nonzero `compute_us` at or above `E` replaces it at once, a cheaper nonzero measurement sets `E ← compute_us+floor((E−compute_us)/2)`, and a measured zero prices nothing and leaves `E` unchanged. A never-decaying maximum would let one expensive orbit price every later cheap one for the rest of the pricing epoch, while the halving reaches the cheap cost exactly in a bounded number of returns and still refuses to under-price the request after the expensive one.

Exactly one first request after startup or resize is admitted as a labelled unpriced warm-up, no second request starts until that warm-up buffer returns, and later work starts only when `projected ≥ P`, where the admission price is `P = min(E,B)`.

The price is bounded by the bucket capacity because the projection is also bounded by it: asking for a balance above `B` is asking for a balance that can never arrive, so an `E` above `B` would make every later admission wait forever, freeze credit at zero, and stop the producer for the rest of the session. Bounding the price instead makes the wait at most one second at any measured cost.

When `projected < P`, producer delay is `ceil((P−projected)·1,000,000/B)` microseconds, at most one second, followed by one browser-task yield and recomputation; pending edits continue to coalesce during the delay.

At admission the producer subtracts `P` from its projected local balance, and the next owner return reconciles the estimate with measured cost; the owner charges the full measured cost, so an orbit costing more than `P` shows its excess as `overfeed_us`, displayed as a producer defect rather than repaired by owner throttling or by a producer that stops admitting work.

Worker `compute_us` begins immediately before decoding the centre into bignum scratch and ends after the one standalone-buffer copy, uses `ceil(1,000·performance.now elapsed milliseconds)`, and returns a typed `TimingOverflow` rather than saturating beyond `u32::MAX`.

The sokol worker measurement uses seven orbit samples and reports the median, then repeats fixed-buffer packing 256 times. At zoom 100 and width 960, the paired 16-byte baseline versus the final one-orbit 8-byte path was: cap 512, time-to-first orbit 1,416→324 us, payload 8,192→4,096 bytes, pack mean 19,681→6,076 ns, admission price 1,416→324 us and depleted wait 5,664→1,296 us; cap 4,096, time 6,740→2,572 us, payload 65,536→32,768 bytes, pack mean 94,821→47,981 ns, price 6,740→2,572 us and wait 26,960→10,288 us. The baseline arm is documented historical evidence rather than code in the final harness and remains a PF-R follow-up.

If the returned warm-up measurement is zero, `Admission::TimingUnavailable` emits a typed `TimingOverflow` channel event instead of inventing a price or admitting an unbounded stream; the overlay distinguishes this unavailable state from a measured zero credit balance.

### 2.6 Versioned owner and two drains

The owner is a hand-rolled, `Rc`-free and `RefCell`-free versioned swap made from `Cell` over `Copy` records; wasm32 has no shared-memory threads here, so its state operations are plain same-thread loads and stores, and no lock or borrow can fail.

HOT and MAIN stage independently but publish one coherent `ViewerState`; later writes replace earlier undrained writes, so latest wins without a queue.

`drain_hot` runs once every refresh, copies the newest HOT beside current MAIN, increments the common checked `u64` epoch, publishes the snapshot, and returns it infallibly even when HOT is unchanged.

`drain_main` runs on a MAIN arrival, where arrival means an accepted orbit, iteration-cap change, palette selection, or plane-preset/origin change; it copies newest MAIN beside current HOT, increments the same epoch, publishes the snapshot, and returns it infallibly.

An impossible `u64` epoch increment failure freezes publication with visible `EpochExhausted`; ordinary drains have no allocation, result, borrow, or refusal path.

App converts each HOT drain to `PresentHot`, constructs `HotSlot` at `refresh_id mod 3`, and calls present's infallible `write_hot`; present owns allocation, alignment, and dynamic-offset selection for that three-slot buffer.

MAIN stores a session-local `OrbitHandle` only after kernels have synchronously accepted the response bytes and returned their typed heap-span wrapper; handle zero means no orbit, and app owns the registry from the compact owner ID to kernels' `ReferenceOrbit` wrapper.

## 3. INTERFACES

All wire integers and floats are little-endian; reserved fields and unoccupied bytes are zero on send and ignored only after version validation; all byte offsets below are from the start of the standalone `ArrayBuffer`.

### 3.1 Shared wire header and trailer

Every message starts with `MessageHeader`, exactly eight `u32` words and 32 bytes; `MAGIC = 0x314c424a` is the little-endian byte string `JBL1`, and `VERSION = 2`.

|Byte|Field|Type|Meaning|
|---:|-----|----|-------|
|0|`magic`|`u32`|`0x314c424a`|
|4|`version`|`u32`|wire version `3`|
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

`ErrorRecord` starts at byte 32 and is exactly `{ code: u32, detail: u32, requested_bytes: u32, available_bytes: u32 }`; stable codes are `1 BadMagic`, `2 BadVersion`, `3 BadKind`, `4 BadLength`, `5 BadTrailer`, `6 CentreEncodingWall`, `7 GenerationExhausted`, `8 EpochExhausted`, `9 TimingOverflow`, `10 BufferStarved`, `11 MathFailure`, and `12 UnexpectedWork`.

### 3.2 Orbit request and bignum centre encoding

The Rust-level request is `OrbitRequest { generation: u32, centre: EncodedCentre, depth_digits: u32, precision_bits: u32, max_iter: u32, precision_mode: PrecisionMode, reason: OrbitReason, reference_pass: ReferencePass }`; header words carry generation, precision, and max iteration, while the request body carries the remaining fields.

`depth_digits = ceil(max(0,zoom_log2·log10(2)))`; it is the integral request label, while the overlay retains the unrounded `f64` decimal-depth fact.

The request body is `{ depth_digits: u32, reason_bits_and_pass: u32, centre_revision: u32, limb_word_count: u32, coordinates: [CoordinateDescriptor; 4], precision_mode: u32, limbs: [u32; limb_word_count] }` with descriptors at bytes 48, 64, 80, and 96, the validated mode discriminant at byte 112, and limbs beginning at byte 116.

The combined word assigns bit 0 to initial reference, bit 1 to centre-threshold crossing, bit 2 to zoom-threshold crossing, bit 3 to max-iteration change, bit 4 to precision-mode change, and bits 5–6 to Preview, Final, or Measure only for `PictureFast`; mode itself is read exclusively from byte 112, deterministic requests require zero pass bits and decode as Final, and unknown or contradictory bits are a version-three `BadLength` error rather than silently ignored.

Each 16-byte `CoordinateDescriptor` is `{ sign: u32, exponent_twos_complement: u32, limb_start: u32, limb_count: u32 }`; descriptors appear in `(z.re,z.im,c.re,c.im)` order at bytes 48, 64, 80, and 96.

A nonzero coordinate is exactly `(−1)^sign · (Σ limbs[limb_start+k]·2^(32k)) · 2^exponent`, limbs are least-significant first, `sign ∈ {0,1}`, the top stored limb is nonzero, and descriptor ranges are ordered, contiguous, non-overlapping, and cover `limb_word_count` exactly.

Canonical zero is `{ sign: 0, exponent: 0, limb_start: previous_end, limb_count: 0 }`; negative zero, leading zero limbs, unused limbs, and out-of-range descriptors are rejected.

The library-independent dyadic encoding lets math's selected Astro-float `BigScalar` round-trip the same mathematical centre; math provides the exact `BigScalar ↔ (sign, exponent, limbs)` adapter and precision semantics, while worker owns canonical validation and transport.

`decode_math` decodes all four coordinates at the request's declared `precision_bits`, which math delivers rounded up to 64 regardless of any coordinate's record width, so the four delivered precisions are equal by construction and the equality check states that invariant instead of gating on it. The transported record is the navigator's full-precision centre, not a centre already narrowed to the request, so a coordinate is routinely wider than the declaration; refusing on that width refuses every navigation whose anchor is a full-mantissa double, which is every navigation once the anchor is a canvas-relative CSS pixel rather than an integer.

### 3.3 Orbit response and credit return

`OrbitResponse` is the 32-byte header followed immediately by `length` reference records, a zero unused region, a 16-byte verification-fact tail, and the 16-byte pool trailer; records use `32+8·length` bytes, the fact tail is `{ verification:u32,max_consumed_word_error_ulps:u32,precision_escalations:u32,reserved:u32 }`, deferred error is `u32::MAX`, and `1 ≤ length ≤ max_iter`.

The high-level response view exposes generation, length, compute wall, delivered precision, admission credit, cancellation, the exclusive orbit lease, `reference_verification()`, `max_consumed_word_error_ulps()`, and `precision_escalations()`; a cancelled response has `length = 0`, no record bytes, and deferred verification, while `compute_ms()` is exactly `f64::from(compute_us)/1,000` and is a display conversion, not another measurement.

On return, main preserves `generation`, `precision_bits`, and `compute_us`, changes kind to `CreditApplied` or `CreditStale`, sets length to zero, and writes its newly computed `credit_us`; that header is the CREDIT record and states whether the named generation was applied.

`OrbitLease::return_credit(&mut self, disposition, owner_now_us)` performs the owner accounting, updates facts, rewrites the header, and transfers the orbit buffer back exactly once; a clock refusal retains the lease for retry, while dropping a live lease is a debug failure and becomes `BufferStarved` plus a visible outstanding-buffer fact in release behavior.

On wasm32 `OrbitResponseView::from_transfer(buffer: ArrayBuffer) -> Result<OrbitResponseView, ChannelError>` adopts a standalone buffer and applies the same shared trailer, header, pool, kind, length, and zero-unused-byte validator as `WireBuffer`; its detached view is inspection-only, while `BrowserOwnerEndpoint::next_arrival` binds the same checked view to the owner port so `OrbitLease::return_credit` can transfer it back.

`OrbitLease::transfer_record_bytes() -> Result<Uint8Array, ChannelError>` is the zero-copy browser payload view corresponding to same-thread `record_bytes() -> Result<&[u8], ChannelError>`; both expose exactly `8·length` initialized bytes and refuse use after credit return.

### 3.4 Shared GPU records recorded on the worker side

`ReferenceOrbitRecord` is 8 little-endian bytes: byte 0 `re: f32` and byte 4 `im: f32` for `Zₙ`. App expands each transfer record to one 16-byte heap RGBA32F texel `(re,im,0,0)` because packing two points per texel would spread address changes into heap code owned by another lane; transfer and credit pay only the 8-byte record.

`EscapeGridRecord` is one little-endian RGBA32F texel and 16 bytes: byte 0 `smooth_iter: f32`, byte 4 `escaped: f32`, byte 8 `rebase_count: f32`, and byte 12 `glitch: f32`; the last three are independently interpreted, `escaped` and `glitch` are exactly `0.0` or `1.0`, and `rebase_count` is integer-valued.

`RefinementLevel` is `#[repr(u32)]` with `Preview = 0`, `Interactive = 1`, and `Final = 2`; kernels expose `EscapeGrid { span: DataSpan, width: u32, height: u32, level: RefinementLevel }`, app schedules the kernel-defined levels in order and may skip one, and present consumes the initialized dense prefix without CPU readback.

Kernels expose `ReferenceOrbit { span: DataSpan, generation: u32, length: u32, precision_bits: u32 }` after the app's regional upload; app registers it under the worker's compact `OrbitHandle` and returns the standalone buffer immediately.

`EscapeParams` is `#[repr(C)] { max_iter: u32, bailout: f32 }`, exactly 8 bytes with offsets 0 and 4; `bailout` is a squared radius and is exactly `256.0`.

The deep plane record is `#[repr(C)] Plane { basis_u: [f32; 4], basis_v: [f32; 4] }`, 32 bytes at offsets 0 and 16; the perturbation kernel has no origin field.

The shallow centre record is `#[repr(C)] CentreSplit { hi: [f32; 4], lo: [f32; 4] }`, 32 bytes at offsets 0 and 16; math computes it from the absolute centre `C`, not by adding MAIN's defining plane origin a second time, and does not use it as deep arithmetic authority.

Worker does not pack kernel uniforms, but its scale handoff is pinned to kernels' layouts: the 96-byte shallow block places `CentreSplit.hi` at byte 32 and `.lo` at 48, while the 64-byte perturbation block places mantissa `pixel_scale: f32` at byte 32 and `scale_exponent: i32` at byte 60.

### 3.5 Owner records and exact CPU layouts

All owner records below are `Copy` plus `#[repr(C)]`; they are compile-time Rust interfaces rather than message payloads, but their layouts are pinned so app and present cannot silently reorder the contract.

`HotState` is 40 bytes and alignment 8: `zoom_log2: f64` at byte 0, `plane_theta_1: f64` at byte 8, `plane_theta_2: f64` at byte 16, and `centre_from_reference_px: [f64; 2]` at byte 24; angles are radians, zoom is dimensionless, and displacement is in current-zoom pixels along `(u,v)`.

`MainState` is 128 bytes and alignment 8 with the following exact layout.

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
|104|`reference_shift_px`|`[f64; 2]`|new accepted reference minus old reference, in current-zoom `(u,v)` pixels|
|120|`precision_mode`|`u32`|`PrecisionMode` discriminant; bytes 124–127 are tail padding|

`plane_axis_a` and `plane_axis_b` remain the seed-axis indices `0..3` for `(e₁,e₂,e₃,e₄)`; `R₁₃·R₂₄` changes the derived basis, not the preset or stored meaning of these two fields.

`OrbitHandle` is the logical pair `{ id: u32, generation: u32 }`; MAIN stores `id` and `generation_applied`, and app rejects a registry lookup whose generation differs.

`ViewerState` is 176 bytes and alignment 8: `epoch: u64` at byte 0, `hot: HotState` at byte 8, and `main: MainState` at byte 48.

`HotDrain` and `MainDrain` each return a full `ViewerState`; the distinct names make the update rate explicit even though their payload layout is identical.

### 3.6 Owner API

`ViewerOwner::new(initial: ViewerState) -> ViewerOwner` creates epoch zero with no pending edit and no failure path.

`ViewerOwner::stage_hot(hot: HotState)` replaces the undrained HOT value and performs no allocation.

`ViewerOwner::stage_main(main: MainState)` replaces the undrained MAIN value and performs no allocation; app uses it for palette, cap, precision mode, and preset/origin arrivals. A mode change checked-increments generation and centre revision through the same navigation path as a cap change, captures the mode word in `NavigationSubmission`, and adds `OrbitReason::PRECISION_MODE_CHANGE`.

`ViewerOwner::configure_navigation(config: NavigationConfig) -> Result<(), OwnerError>` installs the authoritative `BigCentre`, accepted reference centre, math-produced `Plane`, and current grid width; `ViewerOwner::configure_precision_mode(mode:PrecisionMode,edit_budget:u32)` installs the centre-width policy before navigation; `ViewerOwner::navigate(&mut self, delta: NavigationDelta) -> u32` delegates centre mutation to math's `BigCentre::apply_navigation` in `navigation.rs`, grows both centre and reference widths monotonically when the fast plan requires it, computes HOT displacement with `BigCentre::displacement_px`, stages the f64 mirror, checked-increments centre revision and generation, and leaves a typed `OwnerError` retrievable after any refusal. `latest_requested_generation` exposes that generation to the app's incompatible-MAIN scheduler without consuming the submission. `take_navigation_submission` releases one exact centre snapshot only when no request is in flight, while later edits replace its single pending successor. `navigation_centre` returns a clone of that same desired centre without touching the pending successor, which is what saving a view needs: the row must record the centre the owner is actually holding, and reading it must not consume the submission the way taking one does.

`ViewerOwner::accept_navigation_without_orbit(generation,centre_revision)->bool` completes only the matching latest in-flight navigation, publishes the selection generation and centre revision, clears orbit ID, length, precision, and reference shift, and deliberately leaves the reference centre plus HOT displacement unchanged; this keeps retained-pose coordinates coherent while the shallow kernel receives the accepted current centre separately.

`ViewerOwner::accept_orbit(response: &OrbitResponseView, handle: OrbitHandle, reference_shift_px: [f64; 2]) -> OrbitDisposition` returns `Applied` only when response generation equals the latest requested generation and handle generation matches it, stages the orbit fields and reference shift into MAIN, recomputes staged HOT displacement from the latest desired centre against the newly accepted reference, and otherwise returns `Stale`; both outcomes are infallible and require the lease to return credit.

`ViewerOwner::drain_hot() -> HotDrain` publishes and returns the coherent snapshot every refresh, incrementing epoch once.

`ViewerOwner::drain_main() -> MainDrain` publishes and returns the coherent snapshot for each MAIN arrival, incrementing the same epoch once.

`ViewerOwner::snapshot() -> ViewerState` is an infallible copy that does not increment epoch and is diagnostic only; consumers act on drain returns, not polling snapshots for changes.

Consumers never use owner-epoch equality as a compatibility test: each HOT or MAIN drain deliberately advances the shared epoch, while orbit generation, iteration cap, plane origin, and reference-shift semantics decide whether retained work is compatible.

### 3.7 Channel API and same-thread lowering

`WorkerChannel::new(config: WorkerConfig, mode: WorkerMode) -> Result<(OwnerEndpoint, ProducerEndpoint), ChannelError>` allocates exactly four buffers and validates initial capacity; on wasm32 `WebWorker` constructs `BrowserOwnerEndpoint` plus its Worker port rather than an internal queue pair, and allocation, browser-construction, listener-installation, or encoding-wall failure is a typed initialization refusal.

`WorkerConfig` is `{ max_iter: u32 }`; app accepts no request below `max_iter = 64`, the implementation pins the buffer-return deadline to `4,000,000` microseconds and credit policy to `250,000` microseconds per second, and max iteration plus both constants remain displayed policies.

`OwnerEndpoint::submit(request: OrbitRequest) -> SubmitOutcome` returns `Transferred`, `Coalesced`, or `GenerationExhausted`; it never blocks and retains only the latest untransferred orbit request.

`OwnerEndpoint::next_arrival() -> Option<OrbitResponseView>` is non-blocking on same-thread and event-driven on web; a response owns its `OrbitLease` until explicitly credited.

`OwnerEndpoint::return_credit(response: &mut OrbitResponseView, disposition: OrbitDisposition, owner_now_us: u64) -> Result<(), ChannelError>` delegates the response's exclusive lease return for both modes, so app need not inspect the lowering; direct `OrbitLease::return_credit` remains the equivalent lower-level call.

`OwnerEndpoint::shutdown()` closes an already reconciled same-thread channel or immediately returns `BufferStarved`; the browser owner sends `Shutdown`, stops accepting requests, and drives event-based reconciliation for at most the app-enforced four-second buffer-return deadline, `shutdown_acknowledged()` reports completion without blocking, and timeout reports the missing pool/slot rather than hanging.

Reconciliation is recomputed on every owner interaction rather than only when `ShutdownAck` lands, because the browser endpoint owns no timer and an acknowledgement can arrive while a slot is still out; an armed resize drain is bounded by `BUFFER_RETURN_DEADLINE_US`, after which the producer is terminated and the pool replaced regardless, publishing a typed `BufferStarved` that names the pool and slot that never came home. No request is measured against a pool that is being replaced, so a cap LOWER than the current one is never refused for `BadLength` against the old capacity, and each pool carries an epoch so a lease returned from a superseded pool is charged and dropped instead of transferred into the restarted producer.

`ProducerEndpoint::admit(producer_now_us) -> Result<Admission,ChannelError>` applies the same producer shaper used by the browser backend; `next_request`, `complete`, and `cancel` expose one deterministic step for native traces, while the browser-private run loop repeats those steps until `ShutdownAck` or a typed channel failure.

On wasm32 `BrowserOwnerEndpoint::new(config: WorkerConfig) -> Result<BrowserOwnerEndpoint, ChannelError>` constructs the pinned module Worker and `BrowserOwnerEndpoint::from_worker(config, worker)` attaches an app-created port; its `submit`, `next_arrival`, `return_credit`, `take_error`, `latest_generation`, `pending_request_depth`, `facts`, `shutdown`, and `shutdown_acknowledged` methods are the browser implementation behind `OwnerEndpoint`, so app call sites do not branch by mode.

That endpoint is one lowering of a transport-agnostic owner: the private `OwnerCore` holds the pool, arrivals, credit, facts, and the drain, and an `OwnerPort` supplies allocation, ownership transfer, producer restart, and the owner clock. `BrowserPort` is that seam over one module Worker and its transferable buffers, and the native tests supply an in-process port and clock over the same wire buffers, so the resize handshake, its deadline, and four-slot reconciliation are proved without a browser.

`EncodedCentre::encode_math(centre:&BigCentre,revision:u32)->Result<EncodedCentre,ChannelError>` and `decode_math(precision_bits:u32)->Result<BigCentre,ChannelError>` are worker's canonical adapter over math's `encode_big_scalar` and `decode_big_scalar`; `ReferenceOrbitTask::start(request:&OrbitRequest,clock:&impl MonotonicClock)->Result<ReferenceOrbitTask,ChannelError>` constructs math's published `ReferenceOrbitBuilder`, and `poll(latest_generation:u32,clock:&impl MonotonicClock)->Result<OrbitTaskPoll,ChannelError>` returns `Pending`, `Complete`, or `Cancelled` after at most 64 builder steps or 2,000 measured microseconds.

`ComputedOrbit` and `ReferenceOrbitRecord` are re-exported from math rather than copied; the completed `Vec` is reusable linear-memory storage for the one transport copy, and transport itself performs no per-message allocation.

The Web Worker lowering backs endpoints with the four transferable `ArrayBuffer` objects and routes returned request, orbit, credit, and shutdown buffers through `BrowserOwnerEndpoint`; `SameThread` backs the identical ownership states with four preallocated byte buffers moved through bounded queues, bypasses the wasm-boundary memcpy only because producer and consumer share linear memory, and changes no ordering, generation, credit, or drain result.

`JULIBROT_PHASE_IMPLEMENTED = 4`; on wasm32, `allocate_transfer_buffer(pool:u32,slot:u32,max_iter:u32)->Result<ArrayBuffer,JsValue>` creates the exact trailer-bearing standalone buffers and `worker_main(expected_abi:u32)->Result<u32,JsValue>` installs the heap panic hook, refuses ABI skew or a non-worker global, ignores the loader's object handshake, receives transferred buffers through a coexisting event listener, cooperatively runs the latest request, and returns both orbit slots before `ShutdownAck`.

The wasm main-side bridge is `encode_transfer_request(&ArrayBuffer,&OrbitRequest)->Result<(),ChannelError>`, `read_transfer_header(&ArrayBuffer)->Result<MessageHeader,ChannelError>`, `transfer_record_bytes(&ArrayBuffer)->Result<Uint8Array,ChannelError>`, `write_transfer_credit(&ArrayBuffer,OrbitDisposition,&mut CreditAccount,owner_now_us:u64)->Result<CreditCharge,ChannelError>`, and `write_transfer_shutdown(&ArrayBuffer,generation:u32)->Result<(),ChannelError>`; each validates the immutable trailer and mutates or views the same standalone allocation, `transfer_record_bytes` is a zero-copy initialized-range view, and none creates a replacement transport buffer.

`CreditAccount::charge(owner_now_us,compute_us) -> Result<CreditCharge,ChannelError>` implements the owner formula exactly; `ProducerShaper::observe_return(producer_now_us,returned_credit_us,compute_us)` reconciles a CREDIT header and folds the measurement into `E`, `estimate_us()` and `admission_price_us()` expose `E` and `P = min(E,B)`, `admit(producer_now_us)` returns `Ready { credit_us, warm_up }`, `Delay { wait_us }` with `wait_us ≤ 1,000,000`, or `TimingUnavailable`, and `reset_for_resize` creates exactly one new warm-up epoch.

### 3.8 Facts supplied to app

`WorkerFacts` is a 64-byte `#[repr(C)]` record: `epoch: u64` at byte 0, then `last_applied_generation`, `last_ack_generation`, `orbit_queue_depth`, `shutdown_queue_depth`, `credit_us`, `last_compute_us`, `last_overfeed_us`, `applied_count`, `stale_count`, `cancelled_count`, `allocation_events`, `request_buffers_owned_main`, `orbit_buffers_owned_main`, and `mode` as consecutive `u32` fields at bytes 8 through 60.

`WorkerFacts::new(mode)` creates the startup snapshot with `allocation_events = 1` for the four-buffer allocation, and that count increments once only after a reconciled max-iteration resize; `OwnerEndpoint::facts()` and `ProducerEndpoint::facts()` return the same coherent snapshot without inventing unavailable browser observations.

App displays queue depth, credits in microseconds, `compute_ms = last_compute_us/1,000`, owner epochs acknowledged, applied and acknowledged generations, stale/cancel counts, overfeed, buffer ownership, allocation events, worker mode, `centre_from_reference_px`, and `reference_shift_px`; zero and unavailable are distinct labels.

Requested generation, requested iteration cap, precision floor digits, working digits, requested bits, and requested centre revision remain beside delivered generation, cap, bits, orbit length, and resolution; `scale_exponent` and the shallow/deep switch POLICY are shown by app beside kernel facts, warm-up is labelled, no field is inferred from a policy or hardware wall, and no missing measurement becomes zero.

Aggregate rebase and glitch totals are `unavailable` during normal gather-only rendering; only an explicitly requested, labelled measurement readback may count them, and worker never substitutes a CPU estimate.

### 3.9 Presentation call and uniform ownership

The worker defines no GPU uniform block and performs no GPU call; present owns `HotUniform`, exactly 128 bytes with `plane_u: [f32;4]` at byte 0, `plane_v` at 16, `view_rotation` at 32, three padded homography rows at 48, 64, and 80, `clear_rgba` at 96, and `flags: [u32;4]` at 112.

Math defines the semantic, no-byte-ABI `Pose { epoch: u64, orbit_generation: u32, plane: Plane, plane_theta_1: f64, plane_theta_2: f64, zoom_log2: f64, view: ViewControls, grid_width: u32, grid_height: u32, centre_from_reference_px: [f64;2] }`; `ViewControls` is the seven-f64 record `{ theta_1, theta_2, camera_yaw, camera_pitch, height_scale, distance_five, distance_four }`, no field of which is derived from another or from a clock, and math exposes `warp_matrix(from: &Pose, to: &Pose)`.

App converts each `HotDrain` into math's `Pose` and present's `PresentHot`, constructs present's `HotSlot` with index `refresh_id mod 3`, and calls `Presenter::write_hot(&mut self, slot: HotSlot, hot: PresentHot, validation: WarpValidation)`; present computes the f64 warp plan CPU-side and uploads one `HotUniform`, with `hot_stride = align_up(128,min_uniform_buffer_offset_alignment)`.

The refresh order is `Presenter::poll → drain HOT → Presenter::write_hot(refresh_id mod 3) → Presenter::frame → app present`; present owns its two scene textures and all scene/warp fences, while app retains surface ownership and presents outside both measured regions.

## 4. Inherited laws and satisfaction

The substrate law is satisfied because worker has no backend branch, shared-memory prerequisite, or GPU feature request; app alone creates wgpu 24 with `Backends::GL` over WebGL2 and checks `EXT_color_buffer_float` plus the documented minimums.

The traffic law is satisfied because HOT becomes one present-owned uniform write per refresh, MAIN changes only on arrival, reference-orbit bytes upload regionally only when the reference changes, and the escape grid never returns to CPU except explicit conformance measurement.

The heap law is satisfied because kernels receive the orbit through typed regional upload, kernel outputs land by heap's paid SCRATCH-to-DATA copy, heap bind-group identities never change, and worker creates neither bind groups nor alternate output paths.

The hot-ring law is satisfied by app draining HOT each refresh, constructing `HotSlot` at `refresh_id mod 3`, and calling present's infallible `write_hot`; worker neither reallocates nor selects the presentation buffer.

The no-shared-memory law is satisfied by four-buffer ownership transfer and a credit header on the returning orbit buffer; same-thread is the bounded-queue lowering of the same protocol, not a behavioral special case.

The honesty law is satisfied by separate requested and delivered fields, live queue/credit/buffer facts, measured compute microseconds, displayed allocation and overfeed events, explicit warm-up, finite deadlines, checked generations, and `requires visible replay` labels for browser-only facts.

The panic/error law is satisfied by installing the hook at both wasm entries and by keeping all worker failures typed; app's non-panicking wgpu handler precedes the first device call, which the worker never makes.

The CPU-math law remains shared: navigation drift composes `10⁴` and `10⁵` steps of `R(Δθ)` with `Δθ = 1e−3` radians and measures `‖MᵀM−I‖_F`, passing at `≤ 1e−5` for `f64` without re-orthonormalization and for `f32` with Gram–Schmidt every 64 steps.

Warp accuracy requires `max|H⁻¹H−I| ≤ 1e−9` in `f64` for `zoom_log2 ∈ {0,10,20,40,80,100}`; faer enters only if hand-written `f64` fails either oracle, never because it is convenient for the worker.

The pixel and conformance laws are preserved downstream: shallow classification and integer iteration must equal CPU exactly with smooth value within `1e−4`; scaled perturbation classification is exact outside math's propagated error envelope, its smooth value tolerance is `2×10⁻³`, and boundary fixtures explicitly exercise the envelope.

The timing law remains downstream for GPU work: scene and warp each measure wall around a four-byte fence, count polls, and use no timestamp query; worker `compute_us` is separately labelled CPU wall and is never substituted for either GPU cost.

The first-frame law remains app-owned: clear colour plus honest overlay text precedes the first frame, with no diagnostic pattern.

## 5. Oracles and tests

All tests in this section are native unless explicitly labelled `requires visible replay`; native tests use the same-thread lowering and injected monotonic clocks, never browser guesses.

The wire-layout golden constructs every message kind byte-for-byte, checks all offsets, endianness, reserved zeroes, canonical centre descriptors, precision-mode round-trip, record count arithmetic `32+8L`, trailer preservation, bad magic/version/kind/length rejection, and all typed errors.

Bit-identity assertions use the cfg-free `PrecisionMode::requires_bit_identity` policy: dyadic word round-trips, identical Astro-float delivered widths, exact CPU-mirror operation order, exact rebase-count agreement, and exact `D` versus `D+16` words belong to Deterministic conformance, while canonical byte validation and every accuracy comparison to that exact path remain unconditional in both modes.

The orbit-record golden checks index zero, escaping and non-escaping lengths, squared bailout `256.0`, exact `[re,im]` bytes, zero-padded `(re,im,0,0)` GPU expansion, Astro-float word-rounded delivered precision, and the `D_work` versus `D_work+16` validation supplied by deterministic math fixtures.

The ownership model enumerates every legal transfer of request slots zero and one and orbit slots zero and one, proving exactly one owner per slot, no send by a detached owner, exactly-once credit return, resize only after all four return or the bounded deadline expires, and bounded shutdown diagnostics for each missing slot.

The resize test drives 512 to 64 to 4,096 to 512 through the in-process port with a request in flight at every change, proving no length refusal against the pool being replaced, one allocation event per change, the coalesced request re-encoded at the new capacity and submitted, `pending_request_depth` back to zero, the next orbit delivered in a buffer of exactly the new capacity, an unreturned lease unable to hold the drain past the return deadline, and one cap change compared field by field against the same-thread lowering.

Following heap `selection.rs`, the state-machine test places each newer edit at every yield point before and after request transfer, compute start, each cooperative yield, response transfer, app upload, orbit acceptance, HOT drain, and MAIN drain; every schedule ends on the newest generation, stale work never publishes, and both drains return without failure.

The drain interleaving test enumerates HOT and MAIN write permutations around both drain calls, proving HOT refresh cadence, MAIN arrival cadence, one shared strictly increasing epoch, coherent 40-byte HOT and 120-byte MAIN records, latest-wins coalescing, accepted-reference displacement reset and shift publication, and absence of a borrow/refusal path.

The credit arithmetic test exhausts boundary values for refill, capacity clamp, exact depletion, underflow, overfeed, timer advance, `u32` conversion, and an estimate at and beyond the one-second capacity `B`, where the price is `B`, the returned wait is exactly the wait that then admits, and the charge is the price rather than the estimate; a model token bucket and implementation must agree exactly in integer microseconds.

The shaping test proves one labelled unpriced warm-up per resize epoch, `E` rising at once to a costlier measurement and halving to the exact cheap cost afterwards, a measured zero leaving `E` alone, the ceiling wait formula under the bounded price, coalescing while delayed, charge of cancelled work, and an overfeed fact whenever actual cost exceeds admission credit.

The over-budget recovery test drives one 852,293-microsecond orbit through the same-thread lowering and requires the owner to report `last_overfeed_us = 602,293` with zero credit, the producer to wait exactly one second and then admit, and the next cheap orbit to refill credit and shorten the wait; a producer that stops admitting after a single over-budget computation fails it.

The mode-equivalence trace feeds identical requests, edit timings, clock ticks, cancellations, credits, and drains to transferable and same-thread abstract backends and requires identical messages, dispositions, epochs, facts, and final state modulo the wasm-copy counter.

The centre-trigger test uses bignum differences at shallow and extreme depths and covers equality and one-unit-beyond cases at `grid_width/4` pixels, zoom delta `2`, re-arm thresholds `grid_width/8` and `1`, rotated-plane projection, `reference_shift_px` sign, and a newer edit arriving while disarmed.

The 10,000-edit navigation test runs identical mixed edits through 1,024-bit deterministic and derived-width fast owners, reports both walls plus drift per edit, and requires final centre error at most one quarter current pixel after widening the fast value for comparison. The osprey run at `zoom_log2=100` and width 1,024 measured 3,967.833 ms deterministic versus 3,770.115 ms fast, a 197.719 ms saving, with 0.001557775202 pixel total drift or 1.557775202×10⁻⁷ pixel/edit.

The inherited plane oracle checks the `R₁₃·R₂₄` operation order, exact presets, the `π/2` Mandelbrot-to-Julia result, nonzero z and c components at intermediate angles, and the `8·f32::EPSILON` postcondition without a degenerate stage.

The inherited scaled-perturbation oracle compares the f64 scaled recurrence with the unscaled reference across exponent boundaries, forces both `±64` renormalizations, checks underflow makes the rebase predicate false, and requires the nonzero-`Z₀` rebase fixture to preserve `zₙ = Zᵣ+S·δ′ₙ`.

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
|A buffer is lost during cancellation, resize, error, or shutdown|Producer starvation or hang|Four-slot ownership model, the deadline-bounded resize drain over the in-process port, plus visible bounded-shutdown replay|
|Credit rounds or underflows differently between owner and producer|Bursting, permanent delay, or invented budget|Exact integer token-bucket and shaping model tests|
|One computation costs more than the whole one-second budget|Producer never admits again and credit sticks at zero|Bounded admission price plus the over-budget recovery test through the same-thread lowering|
|Owner throttles instead of producer shaping|Input latency is hidden as application policy|Trace proves every edit is accepted/coalesced immediately and attributes delay to producer|
|A stale orbit becomes MAIN state|Wrong centre or reference reaches kernels|Every-yield interleaving test and visible cancellation replay|
|HOT and MAIN snapshots tear or a drain borrow fails|Pose and orbit belong to different versions|Exhaustive drain interleavings over Copy-cell owner|
|One-module packaging duplicates unacceptable memory|Tab memory pressure|Visible cached-fetch and two-instance memory report; app may reopen packaging only in joint review|
|Canonical centre bytes do not round-trip Astro-float|Deep navigation drift or library lock-in|Math-adapter encode/decode property tests against canonical dyadic fixtures|
|Centre bytes outgrow an orbit-sized request buffer|Deep request refusal at fixed iteration cap|Accepted `CentreEncodingWall` inequality plus the 300-digit, `max_iter = 64` fit fixture|
|The two-millisecond yield target is missed by one expensive bignum iteration|Late cancellation and poor responsiveness|Visible per-chunk wall histogram and maximum edit-to-cancel acknowledgement|
|`performance.now` is coarse or quantized|Zero or biased compute and credit facts|Visible timer-quantum report and integer-clock native tests; unavailable stays labelled|
|App returns an orbit buffer before regional upload consumes it|Corrupted GPU reference|OrbitLease ordering test plus visible checksum after immediate buffer return|
|Compact orbit registry reuses an ID across generations|Wrong heap span selected|Registry generation mismatch test and typed refusal|
|Scaled-delta exponent or renormalization changes the represented delta|Wrong deep classification|f64 scaled-recurrence oracle across both renormalization directions|
|A nonzero reference is rebased as though `Z₀` were zero|Broken Julibrot and Julia pixels|Mandatory nonzero-`Z₀` invariant fixture|
|Reference shift has the wrong sign or zoom units|Retained image jumps when a new orbit is accepted|Bignum pixel-displacement tests plus retained-pose visible replay|
|Generation or epoch approaches exhaustion|Modular latest-wins failure|Checked increment boundary tests; no wrapping operation exists|
|Worker panic or channel failure yields a blank wait|Unreadable failure or hang|Panic-hook test, typed error golden, and four-second main-side deadline|

## 7. Implementation phases and line budget

Phase 0 adds the package shell, pinned records, canonical codec, four-slot ownership model, typed failures, and wire-layout goldens, estimated at 360 Rust and test lines.

Phase 1 adds the same-thread bounded queues, generation/coalescing state machine, Copy-cell owner, bignum-derived centre displacement, reference-shift publication, two drains, exhaustive interleavings, and compact orbit registry seam, estimated at 460 lines.

Phase 2 adds the Astro-float codec adapter, reusable bignum/orbit scratch, validated reference computation, one-word coordinate packing, cooperative task yields, cancellation, and scaled-recurrence fixtures, estimated at 470 lines.

Phase 3 adds the wasm `worker_main`, transferable backend, one-copy bridge, four-buffer resize/reconciliation, shutdown deadline, panic reporting, and page-flag selection, estimated at 390 Rust and JavaScript lines.

Phase 4 adds fixed-policy credit/token-bucket shaping, facts snapshots, app/kernels/present integration seams, native trace equivalence, and visible-replay instrumentation, estimated at 380 lines.

The worker slice is therefore budgeted at about 2,060 implementation and test lines; generated wasm glue and downstream app, kernel, heap, and presentation code are excluded.

Implementation progress through Phase 4: the Phase 2 core, transferable `worker_main`, field-paid heap panic-hook installation, standalone buffer pool, checked browser owner endpoint, one-pass orbit copy, cooperative cancellation, ABI refusal, page-flag lowering, four-slot shutdown reconciliation, exact owner token bucket, producer shaper, cancelled-work accounting, deadline-bounded max-iteration resize over the owner port seam, and pinned facts snapshot are implemented; browser detachment, engine allocation, and timing claims remain visible-replay evidence.

## 8. Unresolved joint-review findings

The authoritative owner navigation API is resolved by direct adoption of math's `NavigationDelta`, `BigCentre::apply_navigation`, and `BigCentre::displacement_px` from `crates/labs/julibrot/math/src/navigation.rs`; worker owns only sequencing, coalescing, generation, and publication.

- A single Astro-float iteration cannot be pre-empted by the 64-iteration or 2,000-microsecond cooperative check; visible replay must decide whether math needs finer internal cancellation points.
- Successive accepted references may arrive before present promotes a retained scene; app and present must prove that composing queued `reference_shift_px` values re-bases that scene exactly once.
- Browser transfer proves detachment and trailer continuity but cannot reveal an engine-internal physical copy; evidence must keep the claim at ownership transfer plus one explicit wasm memcpy.
- Coarse `performance.now` resolution can yield a measured zero for short shallow references; the implemented `TimingUnavailable` admission and typed `TimingOverflow` event are honest, but visible replay must determine whether common browsers hit that state often enough to require a coarser clock accumulation policy in joint review.
