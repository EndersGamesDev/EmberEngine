# Julibrot app slice — integration contract

Status: round-one slice document for joint review; where another slice document disagrees with this file, the disagreement is a joint-review finding and this app document is the integration contract until that review changes it.

## 1. Ownership and boundary

The app slice owns `crates/labs/julibrot/app`, `web/labs/julibrot/`, assembly of the four sibling slices, the GL-only wgpu device and surface, the single surface owner, startup diagnostics, frame and refinement scheduling, controls, measurement, facts overlay, page-contract tests, the versioned loader, and the release-bundle report.

The app slice does not own plane algebra, arbitrary-precision orbit arithmetic, the bignum implementation, kernel bodies or refinement-level definitions, heap allocation and scratch-copy lowering, transferred-buffer channel internals, owner-drain internals, palettes, flat or tumbled presentation, warp math, or the hot-ring allocation; math, kernels, worker, present, and `crates/labs/heap` respectively own those concerns.

The app consumes the math, kernels, worker, present, and heap crates as dependencies and never copies their source; the implementation round may make only the explicit minimal heap visibility seams in §3.8, after joint review accepts them.

This lab has one world-shaped object but no gameplay authority, no simulation tick, no general resource DAG, no petgraph, no second heap class, no shared-memory thread path, and no WebGPU path.

## 2. Design

### 2.1 Coordinate, plane, and precision model

The ordered fractal axes are `(z.re, z.im, c.re, c.im) = (e₁,e₂,e₃,e₄)` in ℝ⁴; `e₅` carries escape height only in the tumbled view and is never an axis of the sampled fractal plane.

The user-controlled plane rotation is `Rₚ(θ₁,θ₂) = R₁₂(θ₁)·R₃₅(θ₂)`, applied to column vectors as `v′ = R₁₂(R₃₅(v))`, with independent angles in radians and the standard two-axis block `[[cos θ,−sin θ],[sin θ,cos θ]]`; math projects with `P₄`, drops `e₅`, and Gram–Schmidt re-orthonormalizes the resulting basis.

The standing view rotation is separate: present uses `R(t) = R₁₂(t)·R₃₅(φt)` only for the tumbled view, where `t` is elapsed visible animation time in seconds, `φ = (1+√5)/2`, and the two fixed perspective distances are `d₅ = d₄ = 8`; a plane-slider change never changes view time, and view animation never changes the fractal plane.

The Mandelbrot preset uses basis axes `(e₃,e₄)`, plane origin `(0,0,0,0)`, and identity `Rₚ`; the Julia preset uses `(e₁,e₂)`, plane origin `(0,0,c₀.re,c₀.im)`, and identity `Rₚ`, so `c₀` is MAIN state rather than a shader-only decoration.

Pixel `(i,j)` samples its centre, with `x = i+0.5−width/2`, `y = j+0.5−height/2`, `+v` upward, row zero at the bottom, square pixels, and `height` chosen from canvas aspect rather than by stretching the plane.

The worker slice owns the exact centre `C ∈ ℝ⁴` in its pure-Rust bignum representation and the owner mirrors `C` as four `f64` values for display, local warp deltas, and shallow setup; app code never reconstructs a deep absolute centre from that mirror.

For `zoom_log2 = q`, grid width `W`, and square pixels, the CPU computes `pixel_scale = 4.0/(2^q·W)` in `f64`, uploads the result as `f32`, displays decimal zoom depth `q·log10(2)`, and requests `D = ceil(q·log10(2)+log10(W))+8` decimal digits and `P = ceil(D·log2(10))` precision bits.

The shallow kernel receives the absolute centre split per coordinate as `hi = f32(C)` and `lo = f32(C−f64(hi))`; the perturbation kernel receives no absolute origin and starts from the relative offset `δ = (x·u+y·v)·pixel_scale`, with `δz₀` from axes `(e₁,e₂)` and `δc` from axes `(e₃,e₄)`.

The active reference is renewed when the exact centre moves more than one quarter of the current view extent or `|zoom_log2−reference_zoom_log2| > 2`; the worker owns hysteresis around those boundaries, while the app reports the trigger and never labels a policy as a device wall.

### 2.2 Device, surface, and error lifetime

The wasm start function installs the panic hook before adapter or device work; after `request_device` returns, the app installs the non-panicking device-lost and uncaptured-error handlers before invoking the first method on that device or queue, then begins capability selection and resource initialization inside a validation error scope.

The instance descriptor contains only `wgpu::Backends::GL`, the accepted adapter must report `wgpu::Backend::Gl`, the limits begin at `wgpu::Limits::downlevel_webgl2_defaults()`, and initialization refuses a missing WebGL2 or `EXT_color_buffer_float` floor with a typed page message rather than choosing another backend or format.

One generation-tagged `SurfaceOwnership` token is the only route to `get_current_texture`; an owned frame retains its `SurfaceTexture` across any fence yield, presents once after timing ends, and releases on present, typed failure, cancellation, or drop.

Surface acquisition matches every result without panic: `Lost` and `Outdated` reconfigure and retry once, `Timeout` skips that frame with a typed status, other errors fail only the current epoch, and a newer selection waits cooperatively until an older owned frame drops rather than acquiring a second image.

Initialization, selection, and measurement each use a generation-tagged validation error scope; a captured error drops an unpresented surface image, publishes both page and console diagnostics, preserves requested controls, and cannot poison a later generation.

Before the first successful scene frame the canvas contains only the configured clear colour and honest overlay text describing initialization or refusal; no diagnostic pattern is rendered.

### 2.3 State, controls, and latest-wins publication

HOT state contains desired `zoom_log2`, the two independent plane angles, and the local centre displacement from the accepted reference; it is drained and written through `Presenter::write_hot` on every refresh even when no scene frame is due.

MAIN state contains the accepted reference-orbit handle and generation, requested and delivered iteration caps, precision, orbit length, plane origin including Julia `c₀`, palette, reference centre mirror, and worker credit facts; it is drained after the warp submission so a late orbit cannot delay immediate motion.

Both drains are infallible, share one `u64` epoch, and increment that epoch on every drain call; `changed` reports whether payload changed, while an epoch wrap is documented as impossible in a browser session and is a typed refusal if nevertheless observed.

Orbit-affecting controls increment a monotonic `u32` generation, coalesce while work is in flight, and accept publication only from the current generation; wraparound would require more than four billion orbit-affecting actions in one page session and is therefore refused before wrap rather than compared with modular arithmetic.

Wheel zoom is anchored at the pointer, primary drag pans, DOM-down is converted to plane-up before crossing the worker boundary, and the exact centre adjustment is performed by the worker-owned bignum path; rotation sliders set `θ₁` and `θ₂` independently, and preset, Julia `c₀`, iteration cap, and palette controls retain their requested values across degradation, stale work, or refusal.

The owner never copies an arriving MAIN value into a newer desired HOT field; accepted MAIN publication changes only fields owned by MAIN, which structurally prevents a delayed orbit from snapping zoom, rotation, or the requested control widgets backward.

### 2.4 Progressive refinement and scene admission

Kernels defines an ordered `RefinementPlan` of grid sizes, iteration caps, and legal span reuse from requested canvas extent and live walls; app owns only the schedule, preserves the requested extent and cap, and displays every delivered level and limiting term.

A scene frame is DUE exactly when the MAIN drain changed anything since the last completed scene frame or at least one refinement level remains pending; a HOT-only change produces a warp frame but does not independently dispatch a fractal kernel.

The scheduler takes exactly the next pending level, makes one logical kernel dispatch for that level, lets dialect v2 lower it to the required page passes, lands every output page through the paid SCRATCH-to-DATA copy path, draws that level, and marks it complete only after the scene fence succeeds for the same generation.

A newer orbit generation cancels every older pending level at the next cooperative yield, never publishes its grid or last-frame handle, and begins the newest plan at its first level; a failure leaves the latest requested controls selected and names the failed wall or policy.

The first fenced scene frame after initialization, orbit acceptance, view switch, extent change, or pipeline selection is labelled cold/pipeline warm-up and excluded; the second fenced scene frame decides the policy, with `scene_ms > 100` selecting single-frame-on-demand and `scene_ms ≤ 100` admitting continuous refinement and animation.

Single-frame-on-demand still renders one explicitly requested refresh and each pending refinement level, but does not schedule an otherwise idle animation frame; it is a page POLICY derived from the second measured frame, never an admission test for requested work.

### 2.5 One refresh frame, in cross-slice call order

The app request-animation callback first calls worker `ViewerOwner::drain_hot`, derives math `Plane::from_spec` and the current present `Pose`, and chooses `HotSlot(frame_serial mod 3)`; it does not inspect MAIN or dispatch a scene before completing the warp stage.

The app then starts the warp wall, calls present `Presenter::write_hot(slot, hot)` as the only refresh-rate CPU-to-GPU write, acquires the one surface token, calls present `Warp::reproject(encoder, target, last_frame, from_pose, to_pose, slot)` when a completed frame exists or present `Warp::clear(encoder, target, clear_colour, slot)` otherwise, submits, appends and maps the four-byte completion fence, counts `device.poll(Poll)` calls through cooperative zero-timeout yields, and records the warp wall when the fence resolves.

After the warp fence, app calls worker `ViewerOwner::drain_main`; if `changed` is true it calls worker `ViewerOwner::orbit_bytes(main.orbit_handle)`, kernels `Kernels::upload_reference` for a regional write into the stable reference span, kernels `Kernels::plan_refinement`, and present `Presenter::select_palette`, then invalidates any older pending plan without moving controls.

If no scene frame is due, app ends timing and presents the warped surface image; if a scene frame is due, app takes one `RefinementLevel`, calls kernels `Kernels::dispatch_level` exactly once, receives typed `EscapeGrid`, calls present `Presenter::frame(encoder, target, state, slot)`, submits, appends and maps a separate four-byte scene fence, counts polls, calls present `Presenter::commit_frame` only for a successful current-generation fence or `Presenter::discard_frame` otherwise, and then presents.

Both paths call `SurfaceTexture::present` only after the last applicable mapped fence has resolved and the ending timestamp has been captured, so compositor scheduling is outside both measured regions and the surface image remains singly owned throughout.

### 2.6 One zoom step, in cross-slice call order

For a wheel event, app computes the pointer coordinates about canvas centre in pixels with `+y` up and calls worker `ViewerOwner::navigate(NavigationDelta)`; the worker-owned endpoint updates exact `C`, desired `zoom_log2`, its `f64` mirror and local reference displacement, advances generation, coalesces any older request, and immediately makes those desired values available to the next HOT drain.

The next refresh drains HOT, calls math `Plane::from_spec`, writes the selected present hot-ring slot, and calls present `Warp::reproject` from the last completed pose to the desired pose, so input motion is visible without waiting for an orbit.

When the one-quarter-extent or two-`zoom_log2` hysteresis rule fires, the worker endpoint serializes the exact centre into `OrbitRequest`, transfers one owner-to-worker buffer, and the Web Worker computes the reference in bounded chunks under the returned credit and latest-generation checks.

The Web Worker transfers `OrbitResponse`; the owner rejects and returns a stale buffer without publication, or for the current generation publishes MAIN with a new opaque orbit handle, measured precision, orbit length, `compute_us`, and credit facts while leaving desired HOT fields untouched.

After the next warp submission, app drains MAIN, borrows the transferred orbit bytes, calls kernels `Kernels::upload_reference` and `Kernels::plan_refinement`, returns the no-longer-needed worker buffer with updated credit, dispatches the first level when due, calls present `Presenter::frame`, fences it, promotes its pose and frame handle, and continues later pending levels one per due scene frame.

### 2.7 Measurement and bounded progress

Warp cost is wall time from immediately before its hot regional write and command encoding through resolution of the four-byte fence submitted after the warp commands; scene cost is wall time from immediately before kernel uniform writes and encoding through resolution of the separate four-byte fence submitted after the scene draw, and neither interval includes `present()`.

No timestamp query is requested; every fence maps exactly four bytes, calls `device.poll(wgpu::Maintain::Poll)` once before the first yield and once per retry, reports poll count and fence-wait wall, checks cancellation and a `30,000 ms` deadline before accepting completion after any yield, and refuses at `4,096` polls.

The timer probe makes at most `4,000,000` consecutive `performance.now()` reads, stops after `32` positive transitions or `500 ms`, and adopts the smallest positive transition `Q` as the visible clock quantum; no positive transition makes timing unavailable without preventing rendering.

For a path admitted by its second frame, adaptive measurement performs three named untimed warm-ups and 15 recorded samples; each sample repeats the exact whole submission until elapsed time is at least `32Q`, caps its target at `250 ms`, caps repeats at `4,096`, ends the suite after `30,000 ms`, normalizes by repeats, and reports the middle sorted median and nearest-rank p95 at rank `ceil(0.95n)`.

The first and second fenced frames, adaptive warm-ups, candidates, repeats, poll counts, fence waits, cancellations, and single-frame observations remain listed; warm-ups are visibly labelled and excluded, and browser values say `requires visible replay` until a visible replay supplies them.

### 2.8 Page and bundle

The page exposes flat and tumbled views, wheel zoom, drag pan, two plane-rotation sliders, Mandelbrot and Julia-with-editable-`c₀` presets, requested iteration cap, present-owned palette selection, explicit one-frame and measure controls, one canvas, one status element, and one facts overlay.

The browser loads one wasm module in both the main thread and module worker through the same versioned URL; browser fetch caching avoids a second network payload, but two wasm instances and their independent linear memories remain an explicit cost.

Loader, JavaScript glue, wasm export, and worker handshake all use `JULIBROT_ABI_VERSION = 1` and URLs `./pkg/ember_lab_julibrot.js?v=1`, `./pkg/ember_lab_julibrot_bg.wasm?v=1`, and `./worker.js?v=1`; any version disagreement refuses startup as `VersionSkew { component, expected, observed }` before device initialization or orbit work.

Every redeploy increments the query version whenever JavaScript, wasm, worker protocol, or page-contract semantics change and publishes the page, glue, wasm, and worker bootstrap atomically; an old cached page therefore either loads its matching artifacts or receives a typed skew refusal, never a partially compatible session.

The wgpu lab bundle is expected to be approximately `4.5 MB`; the release build records exact wasm and JavaScript byte counts as build facts, the overlay reports those counts without hiding the duplicated-instance memory cost, and the approximation is not presented as a measurement of this not-yet-built slice.

## 3. INTERFACES

All wire and GPU records below are little-endian, all byte offsets are from the start of the named record, all reserved fields are written as zero and rejected when nonzero on decode, `f32` and `f64` mean IEEE 754 binary32 and binary64, and every GPU uniform size is a multiple of 16 bytes.

|Provider → consumer|Pinned interface|Units and ownership|
|---|---|---|
|math → app/kernels/present|`Plane`, `PlaneSpec`, `EscapeParams`, centre splitting, pixel mapping, drift and warp oracles|Radians, fractal-plane units, squared bailout radius; values copied, no GPU ownership|
|worker owner → app|`ViewerOwner`, `HotDrain`, `MainDrain`, `ViewerState`, `OrbitHandle`, `WorkerCreditStats`|Shared epoch, latest-wins generation; transferred orbit remains worker-slice storage|
|app → worker owner|`NavigationDelta`, preset, Julia constant, iteration-cap and palette selections|Canvas pixels, `zoom_log2`, radians; calls enqueue/coalesce infallibly|
|owner ↔ Web Worker|`MessageHeader`, `OrbitRequest`, `OrbitResponse`, `BigCentre4`, four transferred buffers|Microseconds, bits, decimal digits, 16-byte orbit records; exclusive buffer ownership|
|app → kernels|`KernelSceneInput`, `RefinementPlan`, `Kernels::upload_reference`, `Kernels::dispatch_level`|One logical dispatch per level; app owns schedule|
|kernels → app/present|`EscapeGrid { span, width, height, level }`|Texels and typed heap span; kernels owns allocation/reuse|
|app → present|`Pose`, `HotFrame`, `SceneState`, `PaletteRecord`, `Presenter::write_hot`, `Presenter::frame`, `Presenter::commit_frame`, `Warp::reproject`|Radians, pixels, seconds; present owns pipelines, hot ring and last-frame images|
|present → app|`HotSlot`, `LastFrameHandle`, `WarpStats`, `PresentStats`|Opaque generation-tagged handles and measured submission facts|
|heap → all GPU slices|`DataSpan`, immutable heap group, dialect v2 dispatch, SCRATCH copy, `PollCounter`, `SurfaceOwnership`|16-byte DATA records, dynamic offsets, counted polls; reuse by dependency|

### 3.1 Math records and formulas

`PlaneSpec` is the logical record `{ axis_u: Axis4, axis_v: Axis4, plane_origin: [f64;4], theta_1: f64, theta_2: f64 }`, where `Axis4` is the closed `u32` enum `E1=0, E2=1, E3=2, E4=3`; `plane_origin` is ℝ⁴ in fractal-plane units and the angles are independent radians.

`Plane` is a 48-byte, 16-byte-aligned record with `basis_u: [f32;4]` at bytes `0..16`, `basis_v: [f32;4]` at `16..32`, and `origin_lo: [f32;4]` at `32..48`; `origin_lo[k] = f32(C[k]−f64(origin_hi[k]))` for shallow work and is the local residual relative to the active reference for deep presentation.

`Plane::from_spec(spec, centre_f64) -> Result<Plane, PlaneError>` applies `Rₚ`, projects with `P₄`, Gram–Schmidt re-orthonormalizes, and returns math's typed degenerate case rather than a substitute basis; presets are exactly the records in §2.1.

`EscapeParams` is an 8-byte, 4-byte-aligned logical record with `max_iter: u32` at bytes `0..4` and `bailout_squared: f32` at `4..8`; `bailout_squared` is fixed at `256.0`, comparisons use `|z|² > bailout_squared`, and UI input changes only `max_iter`.

`ReferenceOrbitRecord` is one 16-byte RGBA32F texel `[re_hi, im_hi, re_lo, im_lo]` for `Zₙ`, with index zero equal to `Z₀` from the centre's `(e₁,e₂)` coordinates and stored length `min(max_iter, escape_index+1)`.

`EscapeGridRecord` is one 16-byte RGBA32F texel `[smooth_iter, escaped, rebase_count, glitch]`; `escaped` and `glitch` are exactly `0.0` or `1.0`, `rebase_count` is an exactly integer-valued `f32`, and `smooth_iter` is `n+1−log₂(log₂|zₙ|)` at escape and `−1.0` otherwise.

Perturbation is `δₙ₊₁ = 2Zₙδₙ+δₙ²+δc`, `δ₀=δz₀`, and `zₙ=Zₙ+δₙ`; when `|zₙ| < |δₙ|`, the kernel sets `δ←zₙ`, resets the reference index to zero, increments `rebase_count`, and may repeat, while reaching reference length before escape or `max_iter` sets `glitch=1` and stops.

The honest debug tint is selected solely by `glitch=1`; a second reference and re-render of glitched pixels are out of scope, and the overlay reports glitch pixels and the sum and maximum of rebase counts only when those values were actually obtained by a contracted measurement/readback.

The navigation-drift oracle composes `10⁴` and `10⁵` steps of `R(Δθ)` at `Δθ=10⁻³ rad`, measures `‖MᵀM−I‖F`, and passes at `≤10⁻⁵` for hand-written `f64` without re-orthonormalization and for `f32` with Gram–Schmidt every 64 steps.

The warp oracle evaluates `max|H⁻¹H−I|` in hand-written `f64` and passes at `≤10⁻⁹` for `zoom_log2 ∈ {0,10,20,40,80,100}`; faer may enter only if this `f64` case or the `f64` navigation case fails.

### 3.2 Kernel uniform blocks and grid contract

`ShallowUniform` is 96 bytes aligned to 16: `basis_u: [f32;4]` at `0`, `basis_v: [f32;4]` at `16`, `origin_hi: [f32;4]` at `32`, `origin_lo: [f32;4]` at `48`, `scalars: [f32;4] = [pixel_scale,bailout_squared,0,0]` at `64`, and `counts: [u32;4] = [width,height,max_iter,0]` at `80`.

`PerturbUniform` is 64 bytes aligned to 16: `basis_u: [f32;4]` at `0`, `basis_v: [f32;4]` at `16`, `scalars: [f32;4] = [pixel_scale,bailout_squared,0,0]` at `32`, and `counts: [u32;4] = [width,height,max_iter,orbit_length]` at `48`; it has no origin field.

`Extent2d` is `{ width: u32, height: u32 }` in texels; both values are nonzero, row zero is bottom, and delivered height is the greatest positive integer no larger than the canvas-derived square-pixel height that fits the same live walls as width.

`RefinementLevel` is `{ level: u32, width: u32, height: u32, iteration_cap: u32 }`, and `RefinementPlan` is `{ generation: u32, requested: Extent2d, delivered: Extent2d, limiting_term: WallTerm, levels: Vec<RefinementLevel> }`; kernels owns the ordered level values and legal span-reuse rules, and app does not synthesize missing levels.

`KernelSceneInput` is `{ generation: u32, plane: Plane, origin_hi: [f32;4], params: EscapeParams, pixel_scale: f32, reference: Option<ReferenceSpan> }`; `reference=None` selects shallow and `Some` selects perturbation, while kernels rejects any perturbation input whose orbit length disagrees with `PerturbUniform.counts[3]`.

`ReferenceSpan` is `{ span: DataSpan, length: u32, generation: u32 }`; kernels allocates it once for the current `max_iter`, updates only the changed regional records through `Queue::write_texture`, and never changes the heap bind-group identity.

`EscapeGrid` is `{ span: DataSpan, width: u32, height: u32, level: u32 }`; its logical length is exactly `width·height`, overflow is a typed refusal, every record has the §3.1 layout, and present borrows the typed wrapper without learning scratch textures or page handles.

`Kernels::upload_reference(queue, bytes, current) -> Result<ReferenceSpan, KernelError>` validates header generation, length, 16-byte records, and finite split components before a regional write; `Kernels::plan_refinement(requested, requested_iter_cap, walls) -> Result<RefinementPlan, KernelError>` preserves requested values and returns delivered facts; `Kernels::dispatch_level(encoder, input, level) -> Result<EscapeGrid, KernelError>` is exactly one logical dispatch for the named level.

Every `dispatch_level` writes into heap DATA only through dialect v2's paid SCRATCH render and texture-copy path; page splitting may produce multiple physical passes and copy commands, all are reported, and neither app nor present may attach DATA while its full sampled view is bound.

### 3.3 Worker message header, bignum, and transfer ownership

`MessageHeader` is exactly 32 bytes and eight little-endian `u32` words: byte `0 magic`, `4 version`, `8 generation`, `12 kind`, `16 length`, `20 precision_bits`, `24 compute_us`, and `28 credit_us`.

`magic` has bytes `JBRT` and numeric value `0x5452_424a`, `version` is `1`, and `kind` is the closed set `ORBIT_REQUEST=1`, `ORBIT_RESPONSE=2`, `REQUEST_RETURN=3`, and `RESPONSE_RETURN=4`; an unknown value, bad magic, version, reserved word, size, or kind-specific length is a typed decode refusal.

For `ORBIT_REQUEST`, `length` is the count of payload `u32` words; for `ORBIT_RESPONSE`, `length` is the number of 16-byte orbit records; return messages have `length=0`; `compute_us` is zero on requests/returns and the measured worker computation wall on responses, while `credit_us` is the owner's remaining worker-compute budget for the next one-second window.

`BigCentre4` uses a library-neutral canonical binary encoding with `L = ceil(precision_bits/32)` limbs per coordinate; its request payload begins with `depth_digits: u32`, `max_iter: u32`, `limb_count: u32 = L`, and reserved zero, followed in axis order by four records `{ exponent: i32 encoded in one word, sign: u32, limbs: [u32;L] }`.

For a nonzero coordinate, `sign` is zero for positive and one for negative, `M = Σ limbs[k]·2^(32k)`, the highest stored limb has its high bit set, and the value is `(−1)^sign·M·2^(exponent−32L)`; canonical zero has sign, exponent, and all limbs zero, unused low precision bits are zero, and no NaN or infinity encoding exists.

The request payload therefore has `12+4L` words and `48+16L` bytes; the whole request buffer has `80+16L` used bytes, and decode requires header `length=12+4L`, header `precision_bits=P`, and payload `limb_count=L`.

`OrbitRequest` is the logical record `{ generation: u32, centre: BigCentre4, depth_digits: u32, max_iter: u32 }`; its wire header repeats generation and precision, `depth_digits` is the §2.1 result, and `max_iter` is the requested cap rather than the later delivered orbit length.

`OrbitResponse` is `{ header: { generation: u32, length: u32, compute_us: u32, precision_bits: u32, credit_us: u32 }, orbit: TransferredBuffer }`; bytes `32..32+16·length` are consecutive `ReferenceOrbitRecord` values, and `length ≤ max_iter` even when the used orbit stops earlier at escape.

The channel owns two owner-to-worker buffers and two worker-to-owner buffers, four total, exchanged independently; every buffer capacity is exactly `32+16·max_iter` bytes, transfer detaches the sender's view, each receiver validates before use, and buffers resize only when `max_iter` changes, with old/new capacity and allocation count reported.

Because request capacity is also fixed by `max_iter`, encoding requires `80+16L ≤ 32+16·max_iter`; failure is an honest `PrecisionCapacity` WALL that leaves the requested zoom selected rather than resizing for precision and violating the allocation rule.

The worker returns a consumed request buffer as `REQUEST_RETURN`; the owner returns a stale, superseded, uploaded, or otherwise released response buffer as `RESPONSE_RETURN` after writing current `credit_us`; neither direction borrows a transferred buffer or waits for a specific slot when the other slot is available.

The worker computes in bounded chunks, checks newest generation and returned credit at least every 64 orbit iterations and before every transfer, yields cooperatively when credit is exhausted, and refuses one orbit after `30,000 ms` measured worker wall; the initial per-second credit policy is `250,000 us`, labelled POLICY and configurable only in implementation constants, not presented as a device wall.

The same-thread lowering uses these same four ownership states, headers, generation checks, credit debits, chunk boundaries, and return events but replaces `postMessage` with direct moves; it is the cheapest lowering of one abstraction and not an uncredited special mode.

### 3.4 Owner state and app control API

`VIEWER_STATE_VERSION` is `1`; `ViewerState` is the logical record `{ schema_version: u32, epoch: u64, hot: HotState, main: MainState }`, owned by the worker slice's main-thread endpoint and observed by app only through drains.

`HotState` is `{ generation: u32, zoom_log2: f64, plane_theta_1: f64, plane_theta_2: f64, centre_from_reference_uv: [f64;2] }`, where the local centre displacement is in fractal-plane units along the current orthonormal `u,v` basis and remains relative rather than an absolute deep coordinate.

`OrbitHandle` is the opaque logical record `{ response_slot: u32, generation: u32 }`; only values returned by the current `MainDrain` are valid, and worker `orbit_bytes` returns a checked borrowed byte view until app calls `release_orbit` after upload.

`WorkerCreditStats` is `{ policy_us_per_second: u32, remaining_us: u32, last_compute_us: u32, completed_orbits: u32, stale_orbits: u32, credit_stalls: u32, request_buffer_allocations: u32, response_buffer_allocations: u32 }`; every field is a count or measured microseconds, and counter overflow is a typed refusal rather than saturation.

`MainState` is `{ generation: u32, orbit: Option<OrbitHandle>, orbit_length: u32, precision_bits: u32, depth_digits: u32, max_iter_requested: u32, max_iter_delivered: u32, centre_f64: [f64;4], reference_centre_f64: [f64;4], plane_origin: [f64;4], palette: PaletteRecord, credit: WorkerCreditStats }`.

`HotDrain` is `{ epoch: u64, changed: bool, state: HotState }` and `MainDrain` is `{ epoch: u64, changed: bool, state: MainState }`; `ViewerOwner::drain_hot() -> HotDrain` and `ViewerOwner::drain_main() -> MainDrain` never return an error, each advances the shared epoch exactly once, and payload publication never partially mutates a snapshot.

`NavigationDelta` is `{ pan_canvas_px: [f64;2], zoom_delta_log2: f64, anchor_canvas_px: [f64;2] }`, with both pixel vectors measured from canvas centre, `+x` right and `+y` up; worker `ViewerOwner::navigate(delta) -> u32` applies the anchored centre formula at bignum precision, coalesces pending work, and returns the new generation without blocking.

For old and new scales `s₀,s₁` and pointer anchor `(a,b)`, anchored zoom changes the exact centre by `ΔC = (s₀−s₁)(a·u+b·v)`; drag by `(dx,dy)` changes it by `ΔC = −s·(dx·u+dy·v)`, so content remains under the pointer and no f64 absolute-centre round trip occurs.

`ViewerOwner::select_plane(spec) -> u32`, `ViewerOwner::set_julia_c([f64;2]) -> u32`, and `ViewerOwner::set_max_iter(u32) -> u32` are infallible latest-wins enqueue operations that return a new orbit generation; `ViewerOwner::set_palette(PaletteRecord)` is an infallible MAIN update that does not invalidate the orbit.

`ViewerOwner::orbit_bytes(handle) -> Result<OrbitBytes, WorkerError>` checks slot, generation, header, and exclusive ownership; `ViewerOwner::release_orbit(handle, credit_us) -> Result<(), WorkerError>` returns the response buffer exactly once, and app calls it on success, stale cancellation, and upload failure.

### 3.5 Presentation, pose, palette, and hot ring

`ViewKind` is the closed `u32` enum `Flat=0, Tumbled=1`; `Pose` is `{ plane: Plane, zoom_log2: f64, plane_theta_1: f64, plane_theta_2: f64, centre_from_reference_uv: [f64;2], view_theta_1: f64, view_theta_2: f64 }`, with `view_theta_2=φ·view_theta_1` and `view_theta_1=t` for the tumbled view.

`PaletteStop` is a 16-byte record with `at: f32` at byte `0`, packed `rgba8: u32` at `4`, and reserved zero words at `8` and `12`; `rgba8` stores R in bits `0..8`, G in `8..16`, B in `16..24`, and A in `24..32`.

`PaletteRecord` is 144 bytes aligned to 16: header `{ palette_id: u32, stop_count: u32, interior_rgba8: u32, glitch_rgba8: u32 }` at bytes `0..16`, followed by exactly eight `PaletteStop` records at `16+16k`; `stop_count` is `1..8`, unused stops are zero, stop positions are finite and nondecreasing in `[0,1]`, and `glitch_rgba8` is the required honest debug tint.

`HotFrame` is `{ epoch: u64, extent: Extent2d, view: ViewKind, from_pose: Pose, to_pose: Pose }`; app constructs it from the last completed pose and current HOT snapshot, and present alone packs it into `HotUniform`.

`HotUniform` is 192 bytes aligned to 16: from-plane `basis_u`, `basis_v`, `origin_lo` at bytes `0`, `16`, `32`; to-plane equivalents at `48`, `64`, `80`; `[from_pixel_scale,to_pixel_scale,2^from_zoom_log2,2^to_zoom_log2]` at `96`; `[from_plane_theta_1,from_plane_theta_2,to_plane_theta_1,to_plane_theta_2]` as `f32` at `112`; `[from_view_theta_1,from_view_theta_2,to_view_theta_1,to_view_theta_2]` as `f32` at `128`; `[width,height,view_kind,epoch_low_u32]` at `144`; centre offsets divided by their respective pixel scales as `[from_u_px,from_v_px,to_u_px,to_v_px]` at `160`; and four reserved zero `f32` values at `176`.

Present allocates exactly three hot slots; `hot_stride = align_up(192, device.limits().min_uniform_buffer_offset_alignment)`, the buffer size is `3·hot_stride`, `HotSlot` is a checked `u32` in `0..3`, and every pipeline selects byte offset `slot·hot_stride` through a dynamic uniform offset without replacing a bind group.

`SceneState` is `{ generation: u32, grid: EscapeGrid, pose: Pose, palette: PaletteRecord, max_iter_delivered: u32, view: ViewKind }`; `PendingFrameHandle` and `LastFrameHandle` are opaque `{ frame_id: u64, generation: u32, level: u32 }` values usable only by the present instance that returned them, and a pending value is not a warp source.

`Presenter::new(device, queue, heap_group, surface_format, extent) -> Result<Presenter, PresentError>` owns pipelines, depth and last-frame targets and the hot ring but never the surface; `Presenter::write_hot(slot, hot) -> Result<u64, PresentError>` performs one 192-byte regional write and returns the actual uploaded byte count.

`Presenter::select_palette(palette) -> Result<(), PresentError>` performs a regional update only when the MAIN palette changes; `Presenter::frame(encoder, target, state, hot_slot) -> Result<PendingFrameHandle, PresentError>` encodes flat or tumbled drawing from `EscapeGrid` into the surface and present-owned last-frame targets without CPU readback.

`Presenter::commit_frame(pending) -> Result<LastFrameHandle, PresentError>` promotes exactly one current-generation pending image after its successful fence, while `Presenter::discard_frame(pending)` releases a failed, cancelled, or stale image infallibly; only the committed handle and pose become inputs to a later warp.

`Warp::reproject(encoder, target, last_frame, from_pose, to_pose, hot_slot) -> Result<WarpStats, PresentError>` reprojects the last completed colour/depth frame, and `Warp::clear(encoder, target, clear_colour, hot_slot) -> Result<WarpStats, PresentError>` supplies the pre-scene clear; neither function submits, presents, maps, or writes the hot ring.

`WarpStats` is `{ source_frame_id: Option<u64>, submitted_pixels: u64, rejected_pixels: u64, disoccluded_pixels: u64 }` and `PresentStats` is `{ view: ViewKind, width: u32, height: u32, level: u32, submitted_pixels: u64 }`; only GPU readback or shader-visible counters can populate rejected/disoccluded counts, otherwise those overlay fields are `unavailable`, never CPU guesses.

### 3.6 App runtime and page facts

`App::start(canvas_id: &str, status_id: &str) -> Result<App, AppError>` performs the ordered startup in §2.2, constructs one `ViewerOwner`, `Kernels`, `Presenter`, and `Warp`, and returns only after loader and worker ABI handshakes agree.

`App::refresh(now_seconds: f64) -> Result<FrameOutcome, AppError>` executes §2.5, `App::request_one_frame()`, `App::measure_warp()`, and `App::measure_scene()` enqueue finite latest-wins work, and every page callback catches and publishes `AppError` rather than allowing a rejected promise to become the diagnostic.

`FrameOutcome` is `{ epoch: u64, generation: u32, warped: bool, scene_level: Option<u32>, presented: bool, status: FrameStatus }`, where `FrameStatus` is the closed set `ClearOnly`, `Presented`, `SkippedTimeout`, `Cancelled`, and `FailedTyped`.

`PageFacts` contains exactly `{ abi_version, adapter_name, backend, rgba32f_renderable, requested_width, requested_height, delivered_width, delivered_height, requested_iteration_cap, delivered_iteration_cap, requested_zoom_log2, presented_zoom_log2, reference_zoom_log2, zoom_digits, requested_precision_bits, delivered_precision_bits, orbit_length, orbit_generation, owner_epoch, rebase_count_sum, rebase_count_max, glitch_pixel_count, worker_credit, worker_compute_us, worker_buffer_allocations, refinement_level, refinement_pending, kernel_page_passes, scratch_copy_commands, scratch_copy_bytes, hot_write_bytes, event_upload_bytes, palette_id, view_kind, warp_source_frame, warp_submitted_pixels, warp_rejected_pixels, warp_disoccluded_pixels, warp_ms, scene_ms, warp_completion_polls, scene_completion_polls, warp_fence_wait_ms, scene_fence_wait_ms, warmup_label, animation_policy, device_walls, app_policies, limiting_term, wasm_bundle_bytes, javascript_bundle_bytes, wasm_instance_count, timing_status }`.

Requested and delivered fields are never substituted; `device_walls` and `app_policies` are separately labelled formula strings; zoom, precision, orbit, refinement, byte, credit, timing, poll, rebase, glitch, and warp fields retain their stated units or say `unavailable`/`requires visible replay`.

### 3.7 Error set

`AppError` is the typed union `VersionSkew`, `Capability`, `DeviceLost`, `UncapturedGpu`, `CapturedGpu`, `SurfaceBusy`, `SurfaceSkipped`, `Surface`, `StaleGeneration`, `EpochOverflow`, `GenerationOverflow`, `Deadline`, `CompletionPollLimit`, `Mapping`, `Worker`, `Math`, `Kernel`, `Present`, and `Serialization`; each case carries operation, generation or epoch where relevant, and the original typed source text.

No error handler panics, no async borrow waits while held, no callback publishes without checking generation, and no surface-acquire path uses `.unwrap()` or `.expect()`.

### 3.8 Heap dependency seams

The implementation round generalizes `crates/labs/heap/src/browser_error.rs`, `selection.rs`, and `completion.rs` only by publicly re-exporting the already generic `publish_browser_error`, `install_logging_handler`, `SelectionEpoch`, `SurfaceOwnership`, `PollCounter`, and `MAX_COMPLETION_POLLS`; the app page retains status element id `status`, so no behavior or hard-coded DOM selector needs changing.

The app keeps its own generation-tagged error-scope guard because heap's `GpuErrorScope`, owned surface wrapper, and deadline loop are lattice-runtime-specific; it follows their ownership and completion patterns while depending on the generic counters and owners, and no source is copied.

No heap allocator, descriptor, span, dialect, scratch-copy, presentation, or fence behavior is generalized for this slice; any need beyond the visibility-only exports above returns to joint review before implementation.

## 4. Inherited laws and satisfaction

|Law|How this slice satisfies it|
|---|---|
|WebGL2 floor only|Requests only `Backends::GL`, verifies backend Gl, starts from downlevel WebGL2 limits, requires renderable/sampleable RGBA32F, and refuses rather than falls back.|
|Uniforms-only per frame|The refresh-rate write is exactly one 192-byte regional hot-slot write; a due scene additionally writes only its kernel uniforms, while orbit, palette, descriptors, and refinement headers change only on their named events.|
|Regional writes for change|Reference records, palette, and changed metadata update only their occupied regions; scene output stays GPU resident.|
|Paid kernel landing|Every escape output uses heap dialect v2 SCRATCH-to-DATA copy; app never creates a direct-overlap path.|
|Stable heap identities|Heap and present bind groups are created once; handles, directory contents, dynamic offsets, and regions change behind them.|
|Three-slot hot ring|Present owns exactly three slots and app selects `frame_serial mod 3` through the runtime-aligned dynamic offset.|
|No shared memory|Four transferable buffers enforce exclusive ownership and credit return; same-thread uses identical ownership states.|
|Single surface ownership|One generation-tagged token covers acquisition through fence and present/drop; selectors wait cooperatively.|
|Panic and GPU diagnostics|Panic hook precedes adapter work; uncaptured and device-lost handlers precede the first device/queue method after creation; scopes cover init, selection, and measurement.|
|Honest work|Overlay separates requested, delivered, submitted, measured, unavailable, walls, policies, warm-up, browser replay, and derived arithmetic.|
|Never hang|Generation checks, zero-timeout yields, 4,096 polls, 30-second fences/suites/orbits, finite timer probes, bounded repeats, and typed refusal bound every wait.|
|Math evidence before faer|The exact drift and warp oracles gate any faer dependency; hand-written `f64` is the default.|
|Renderer austerity and authority|One world, flat/tumbled presentation and warp only; no gameplay truth, DAG, tick, extra heap class, shared-memory worker, or WebGPU.|
|Versioned deployment|One version value pins page, glue, wasm, worker and protocol; skew refuses before runtime start and deploy is atomic.|

## 5. Oracles and tests

Native math-contract tests pin axis order, both rotations and their separation, preset origins, Gram–Schmidt behavior, pixel centres and row direction, centre split reconstruction, pixel-scale/digit/bit formulas, reference renewal boundaries, perturbation/rebase transitions, record sentinels, and the R13 drift and warp thresholds.

Native layout tests assert exact sizes, alignments, offsets, little-endian pack/decode round trips, reserved-zero rejection, header kind and length rules, bignum canonicality, orbit index zero, escape-grid channel independence, `HotUniform` packing, three-slot stride arithmetic, palette ordering, and overflow refusal.

Native worker-state tests enumerate both ping-pong slots in both directions, transfer/detach/return ownership, buffer resize only on `max_iter`, credit exhaustion/refill, 64-iteration cancellation points, same-thread trace equality, stale response return, generation refusal before wrap, shared epoch increments, and infallible HOT and MAIN drains.

Native integration-state tests model one refresh and one zoom step and require the exact cross-slice call order in §§2.5–2.6, including HOT before warp, MAIN after warp, one level per due scene frame, fence before promotion, presentation after the ending timestamp, cancellation at every yield, and no HOT snap-back from MAIN.

Native progressive tests require a due scene exactly for changed MAIN or pending refinement, preserve requested values under every wall, accept only current-generation grids, exclude the first fenced frame, let only the second choose the 100 ms policy, and keep single-frame-on-demand finite.

Native conformance consumes sibling fixtures: shallow escape classification and integer iteration count must exactly equal the CPU reference at sampled pixels and smooth values differ by at most `10⁻⁴`; perturbation must match classification and the math slice's argued f64-delta tolerance, including repeat rebases and the reference-length glitch stop.

Native page-contract tests pin `JULIBROT_ABI_VERSION=1` in loader, glue, wasm and worker, the three versioned URLs, one wasm module rather than a second worker wasm, explicit GL-only selection, hook/handler ordering before device use, clear-plus-overlay before the first frame, every facts field, and no `.unwrap()` or `.expect()` within the acquisition function.

Native measurement tests pin the four-byte fence, poll-before-yield order, 4,096-poll and 30,000 ms bounds, cancellation before resumed completion, two separate warp/scene fences, present outside measured intervals, timer-probe bounds, three warm-ups, 15 samples, 32-quanta target, 250 ms batch cap, 4,096 repeats, 30-second suite, median, and nearest-rank p95.

`requires visible replay`: a supported browser must show backend Gl and the live RGBA32F usages, render flat and tumbled views, keep content under wheel/drag input, show no diagnostic pattern, survive rapid controls and surface loss, and retain the last requested controls after stale work or typed refusal.

`requires visible replay`: network inspection must show one versioned wasm payload fetched or cache-hit for two module instances, four buffers alternating ownership without shared memory, allocation events only after `max_iter` changes, and a typed startup refusal when worker and main versions are deliberately mismatched.

`requires visible replay`: each warp and scene observation must display its wall, fence wait and nonzero poll count when polling occurs; warm-ups, second-frame decision, adaptive repeats, visibility, timer quantum, single-frame fallback, cancellation, and deadline refusal must remain readable.

`requires visible replay`: the release build must report exact uncompressed wasm and JavaScript bytes beside the approximately `4.5 MB` wgpu-lab expectation and must not hide that the main and worker wasm instances each allocate linear memory.

## 6. Risks and retirement oracles

|Risk|Oracle that retires it|
|---|---|
|Plane and view rotations are conflated|Preset/rotation native fixtures plus a visible tumbled replay where time moves only the view and sliders move only the sampled plane.|
|Deep input loses the centre through f64|Bignum anchored-zoom and drag fixtures at `zoom_log2=100`, proving the exact encoded centre while the owner mirror is display-only.|
|Perturbation hides a short reference failure|Reference-end fixture requires `glitch=1`, stopped iteration, debug tint, and visible glitch count; no second-reference repair exists.|
|A stale orbit snaps controls or publishes pixels|Cancellation-at-every-yield model plus rapid visible wheel, slider, preset, cap, and palette replay.|
|Two async frames acquire one surface image|Exhaustive `SurfaceOwnership` model plus rapid visible selection during a deliberately delayed fence.|
|A backend validation error becomes an unreadable panic|Ordering source test and injected scoped/uncaptured errors that publish typed page and console messages.|
|Warp or scene timing includes compositor delay|Source-order test requires submit, four-byte fence, end timestamp, then present; visible poll/wait facts expose stalls.|
|Adaptive timing fabricates precision|Coarse-clock fixture and visible timer probe require 32 measured quanta or mark timing unavailable.|
|Refinement silently changes work|Plan fixtures and overlay arithmetic compare requested/delivered extent, cap, level, passes, copy commands, and bytes.|
|Hot-ring reuse races queued GPU work|Three-slot dynamic-offset trace plus delayed-fence visible replay; a slot is not rewritten while its generation is still owned.|
|Transferred buffers allocate or alias unexpectedly|Four-slot ownership model and browser allocation counters before and after `max_iter` changes.|
|Loader combines incompatible cached artifacts|Native URL/version pins and deliberate visible worker skew producing `VersionSkew` before device work.|
|The bundle cost is obscured|Release artifact byte-count gate and overlay snapshot including both artifact sizes and two instances.|
|Hand-written matrix math is inadequate|R13 navigation and warp oracles; only a failing f64 result opens the faer decision.|

## 7. Implementation phases and line budgets

Phase 0 generalizes only the six heap visibility exports in §3.8 and adds app-side source-contract fixtures, estimated at 40 heap Rust lines and 120 app test lines.

Phase 1 creates the app crate, ABI/version types, GL-only instance/device selection, capability refusal, panic and error publication, scoped initialization, surface owner, acquisition recovery, and clear-first-frame path, estimated at 360 Rust lines.

Phase 2 integrates `ViewerOwner`, exact HOT/MAIN drain ordering, controls, bignum-preserving navigation calls, transferred-orbit release guards, generation/epoch cancellation, and worker facts, estimated at 320 Rust and test lines.

Phase 3 integrates math planes, kernel reference upload/refinement planning, one-level scene scheduling, scratch-copy facts, present hot slots, warp, flat/tumbled scene draw, and last-frame promotion, estimated at 420 Rust and test lines.

Phase 4 builds the self-contained page, one-module worker bootstrap, version-skew handshake, persistent controls, clear/status/facts UI, requested-versus-delivered arithmetic, and approximately 4.5 MB bundle disclosure, estimated at 480 HTML, CSS, JavaScript, and Rust lines.

Phase 5 adds separate warp and scene fences, counted cooperative polling, two-frame policy, single-frame-on-demand, adaptive measurement, typed cancellation/deadlines, and full evidence serialization, estimated at 390 Rust, JavaScript, and test lines.

Phase 6 completes native conformance and page-contract coverage, release-bundle byte gates, browser replay scripts/checklists, doc reconciliation fixes, and lint repair, estimated at 300 test and documentation lines.

The app-slice estimate is therefore approximately 2,430 new lines plus the sibling crates consumed by dependency; implementation reports actual net lines per phase and treats an overrun as evidence rather than deleting oracles to meet the estimate.

## 8. Unresolved for joint review

- The math slice must choose dashu-float, astro-float, or an argued hand-rolled fixed-point backend; this document pins a library-neutral wire encoding but has no comparative native probe result.
- Kernels owns the concrete refinement level sizes, caps, and span-reuse transitions, so this app schedule cannot pin their numeric ladder until the kernels document is compared.
- Math must state the exact perturbation f64-delta tolerance required by R15; this document pins classification and lifecycle but cannot invent that tolerance.
- Present must pin escape-height normalization and disocclusion treatment for the tumbled view; the grid channels and double perspective are fixed, but visual height scale is not yet jointly ruled.
- The palette catalogue, default `palette_id`, and exact honest glitch colour remain present-owned choices even though the record layout is fixed here.
- It remains to prove that `Plane.origin_lo` and `centre_from_reference_uv` have identical rebase semantics in the math, worker, and present documents, especially on the frame that accepts a new reference.
- The `250,000 us` per-second worker credit is an app POLICY without field evidence; visible responsiveness and orbit throughput may justify changing it during refined documentation, but the unit and accounting cannot change silently.
- Transferred `ArrayBuffer` bytes may require one unavoidable copy into wasm linear memory before the regional GPU upload; the worker and kernels documents must agree on who owns and reports that copy.
- Browser module caching avoids a second fetch but does not avoid a second wasm instance or memory; exact duplicated memory and startup walls remain visible-replay facts.
- Hidden-page suspension can delay JavaScript beyond a deadline before code resumes to observe it; the first resumed check is bounded, but browser suspension itself cannot be bounded by wasm.
- Warp statistics needing GPU counters or readback may cost more than they are worth; until present supplies a measured path, rejected and disoccluded counts must remain `unavailable`.
- Hot-ring safety across unusually long queued work needs reconciliation with present's slot-retirement rule; three slots are fixed, but whether app skips or waits when all three are live remains open.
- The deploy mechanism that atomically publishes page, glue, wasm, and worker bootstrap is not yet selected for this lab, although skew refusal and version increment law are fixed.
- The approximately `4.5 MB` expectation comes from existing wgpu labs, not this release artifact; the exact app bundle and any size regression threshold remain implementation-round evidence.
- The four-buffer protocol has no recovery path for a worker terminated while owning buffers; restart allocation, counters, and user-visible refusal need worker/app reconciliation.
- `u32` compute and credit microseconds cover about 4,294 seconds, far beyond the 30-second orbit deadline, but decoder behavior for malicious overflowed headers still needs a shared error spelling.
