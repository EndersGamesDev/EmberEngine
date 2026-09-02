# Julibrot math slice contract

Status: round-one slice document for `crates/labs/julibrot/math`; joint review may refine this contract before implementation, and the app document is the integration contract where slice documents disagree.

## 1. Ownership and exclusions

The math slice owns the Julibrot definition, coordinate conventions, plane construction, presets, `f32` escape reference, high-precision centre and reference-orbit arithmetic, centre `f64` mirror, `f64`-to-`f32` hi/lo split, perturbation and rebasing semantics, precision selection, warp matrices, and the navigation-drift and warp-accuracy oracles.

The slice is CPU-only: it supplies records, functions, exact arithmetic contracts, and native tests, while kernels own dialect-v2 registration, dispatch, refinement levels, iteration caps per level, span reuse, GPU conformance, and scratch-copy landing; worker owns scheduling and transfer; present owns rendering, palettes, the hot-ring allocation, and warp submission; app owns the runtime and refinement schedule.

The slice depends on the paid `crates/labs/heap` API for `DataSpan`, handles, dialect v2, presentation conventions, fences, surface ownership, the panic hook, and non-panicking error reporting; it neither copies those mechanisms nor edits the heap in this round.

The slice does not own WebGPU, a general DAG or petgraph, more than one world, a simulation tick, more than one heap class, shared-memory threads, GPU shaders, gameplay truth, or a second-reference repair for glitched pixels.

## 2. Mathematical design

### 2.1 Julibrot and escape reference

The Julibrot is `J = {(z₀,c) ∈ ℂ² : the sequence zₙ₊₁ = zₙ² + c is bounded}`; the real coordinate order is `(z.re,z.im,c.re,c.im) = (e₁,e₂,e₃,e₄)`, so Mandelbrot is the plane `z₀ = 0`, Julia at `c₀` is the plane `c = c₀`, and a finite iteration cap proves only “not escaped by this cap,” never mathematical membership.

The CPU escape reference examines states in increasing index `n = 0..max_iter-1`, declares escape at the first `n` for which `|zₙ|² > bailout`, and otherwise advances `zₙ₊₁ = zₙ² + c`; `bailout` is a squared radius and its standing value is exactly `256.0`, so equality does not escape.

At escape index `n`, `smooth_iter = n + 1 - log₂(log₂|zₙ|)`; both logarithms are base two, and in natural logarithms this is `n + 1 - ln(ln(|zₙ|)/ln(2))/ln(2)`; a sample not escaped by the cap stores `-1.0`.

`max_iter = 0` is a typed `InvalidMaxIter` error rather than an empty successful orbit, non-finite inputs are typed errors, and all complex products use `(a+bi)(c+di) = (ac-bd)+(ad+bc)i` in the stated operation order.

### 2.2 Plane and view rotations

Real five-space uses the ordered basis `(e₁,e₂,e₃,e₄,e₅)`, but `e₅` carries escape height only in the tumbled VIEW and is never an input coordinate of the fractal plane.

For column vectors, `Rᵢⱼ(θ)` has the standard two-by-two block `[[cos θ,-sin θ],[sin θ,cos θ]]` in axes `(i,j)` and identity elsewhere, angles are radians, multiplication is `R₁₂(θ₁)·R₃₅(θ₂)`, and application is explicitly `v′ = R₁₂(R₃₅(v))`.

The user-controlled PLANE rotation is `Rₚ(θ₁,θ₂) = R₁₂(θ₁)·R₃₅(θ₂)` with independent hot-state angles frozen per frame.

The standing time-driven VIEW rotation belongs to present and is `R(t) = R₁₂(0.4t)·R₃₅(φ·0.4t)`, where `t` is seconds and `φ = (1+√5)/2`; math supplies the coefficients and oracle, while present alone applies this rotation to the tumbled view.

The Mandelbrot preset has seed basis `(e₃,e₄)`, origin `(0,0,0,0)`, and identity `Rₚ`; the Julia preset at finite `c₀` has seed basis `(e₁,e₂)`, origin `(0,0,c₀.re,c₀.im)`, and identity `Rₚ`, with `c₀` in MAIN state.

For seed axes `(e_a,e_b)`, form `a = P₄(Rₚe_a)` and `b = P₄(Rₚe_b)` with `P₄(x₁,x₂,x₃,x₄,x₅) = (x₁,x₂,x₃,x₄)`, then compute `u = a/|a|`, `q = b-u(u·b)`, and `v = q/|q|` in `f64` before one rounding of each component to `f32`.

Plane construction returns `DegeneratePlane { stage: FirstAxis }` when `|a| ≤ 2⁻²⁰`, returns `DegeneratePlane { stage: SecondAxis }` when `|q| ≤ 2⁻²⁰`, and publishes no replacement plane; controls reject that candidate before state publication, so HOT and MAIN drains remain infallible and the previous valid state stays current.

After component rounding, the implementation repeats modified Gram–Schmidt in `f32` once and rejects the same two degenerate stages if either divisor is at or below `2⁻²⁰`; the native plane oracle requires `|u·u-1|`, `|v·v-1|`, and `|u·v|` each at most `8×f32::EPSILON`.

The shared rotation family has a known geometric limit: after `P₄` and Gram–Schmidt it rotates Julia’s basis only inside `span(e₁,e₂)` and restores Mandelbrot’s nondegenerate basis inside `span(e₃,e₄)`, so it does not generate the charter’s promised hybrid z/c planes; this is an unresolved joint-review finding, not hidden by inventing another rotation.

### 2.3 Pixels, centre, zoom, and splitting

For a `width × height` grid, integer pixel `(i,j)` has centred coordinates `x = i+0.5-width/2` and `y = j+0.5-height/2`, row zero is at the bottom, `+v` points up, pixels are square, and height is selected from the canvas aspect without changing `pixel_scale`.

HOT state carries finite `zoom_log2: f64`; the CPU computes `pixel_scale = 4.0/(2^zoom_log2·grid_width)` in `f64`, and kernels receive its one-time round-to-nearest-ties-to-even `f32` value.

Displayed zoom depth is `zoom_digits = zoom_log2·log10(2)` decimal digits, while the R6 precision floor is `D_floor = ceil(zoom_log2·log10(2)+log10(grid_width))+8` decimal digits.

The floor alone does not cover roundoff accumulated across a long reference orbit, so the first working request is `D_work = D_floor + ceil(log10(max(max_iter,1)))`; the worker converts this to `precision_bits = ceil(D_work·log₂(10))`, Astro-float rounds that upward to its 64-bit word boundary, and the overlay reports requested floor digits, working digits, and delivered bits separately.

The worker validates the orbit by recomputing at `D_work+16` digits and requiring identical escape index plus every emitted hi/lo record within two `f32` ulps componentwise; on failure it raises `D_work` by 16 and repeats up to the explicitly configured 300-digit POLICY, after which it returns `PrecisionExhausted` rather than publishing an unverified orbit.

The high-precision centre `C ∈ ℝ⁴` lives in the worker, the owner holds an `f64` mirror plus an opaque canonical centre encoding, and the full Julia `c₀` or Mandelbrot origin is included in `C`; an `f64` mirror is display, warp, and shallow-navigation state, not deep arithmetic authority.

For each finite `x: f64`, `split_f64(x)` computes `hi = round_f32_ties_even(x)` and `lo = round_f32_ties_even(x-f64(hi))`; reconstruction is `f64(hi)+f64(lo)`, and the four-coordinate split applies this independently in axis order.

`Plane.origin_lo` is exactly the low array from the current centre split; the shallow uniform supplies its matching high array, while the perturbation uniform supplies no absolute origin.

The current reference is replaced when the worker-measured centre displacement exceeds one quarter of the width extent `4/2^zoom_log2`, equivalently `|C-C_ref|₂ > 2^-zoom_log2`, or when `|zoom_log2-zoom_log2_ref| > 2`; one request per pending generation and reset to zero displacement on accepted arrival supply hysteresis, and threshold tuning is a displayed POLICY rather than a hardware wall.

Because the smallest positive `f32` subnormal is `2⁻¹⁴⁹`, the mandated upload becomes zero when `zoom_log2 > 151-log₂(grid_width)`; for width 1,920 this is about 42.17 displayed digits, so the present `f32 pixel_scale` interface is not arbitrary zoom and must be repaired in joint review with a mantissa/exponent scale before the charter claim can be met.

### 2.4 Reference orbit, perturbation, rebasing, and glitch

For reference centre `C = (C_z,C_c)`, the worker evaluates `Z₀ = C_z` and `Zₙ₊₁ = Zₙ²+C_c` at the delivered precision, stores record zero for `Z₀`, and stores at most `max_iter` entries; `length = min(max_iter,escape_index+1)` when the reference escapes and `length = max_iter` otherwise.

The deep pixel offset is `o = pixel_scale·(x·u+y·v)` in `f32`, `δz₀ = (o₁,o₂)`, `δc = (o₃,o₄)`, and no absolute centre enters the perturbation kernel; therefore Mandelbrot has `δz₀=0` exactly when `u,v ⊂ span(e₃,e₄)`, and Julia has `δc=0` exactly when `u,v ⊂ span(e₁,e₂)`.

At global iteration `n` and reference index `r`, reconstruct `Zᵣ = (re_hi+re_lo)+i(im_hi+im_lo)`, set the full value `zₙ = Zᵣ+δₙ`, test escape first, and if work remains advance `δₙ₊₁ = 2Zᵣδₙ+δₙ²+δc` and `r ← r+1` in the contracted `f32` operation order.

REBASING is repeatable and occurs after the current escape test but before advancing whenever `|zₙ| < |δₙ|`: set `δ ← zₙ`, reset `r ← 0`, increment `rebase_count`, then perform exactly one advance against `Z₀` to global iteration `n+1` and reference index 1; the predicate is not re-evaluated at the same `n`, equality does not rebase, and a next count at or above `2²⁴` produces a typed glitch instead of an inexact RGBA32F count.

GLITCH occurs when `r = length` before the pixel has escaped or reached `max_iter`: set `glitch = 1`, stop that pixel, preserve `escaped = 0`, preserve `smooth_iter = -1.0`, and show the honest debug tint; a second reference for glitched pixels is out of scope.

The stipulated rebase assignment is algebraically exact only when `Z₀ = 0`; for a general Julibrot or Julia reference with nonzero `Z₀`, restarting against orbit index zero would require `δ ← zₙ-Z₀`, so the shared rule `δ ← zₙ` is an unresolved correctness defect that the nonzero-`Z₀` rebase oracle must expose before implementation.

### 2.5 Warp math

For a pose, lift its plane columns to five-space with zero escape height, apply `R₁₂(view_theta_1)·R₃₅(view_theta_2)`, drop e₅, and Gram–Schmidt the result by the same rule as plane construction to obtain view-linearized columns `B=[u_view v_view]`; flat view uses identity VIEW angles.

Let a pose contain centre `C`, view-linearized columns `B`, scale `s`, zoom, and view angles; the least-squares screen warp from pose `f` to pose `t` is the homogeneous row-major matrix `H(f,t) = [[A₀₀,A₀₁,b₀],[A₁₀,A₁₁,b₁],[0,0,1]]`, where `A = (s_f/s_t)B_tᵀB_f` and `b = B_tᵀ(C_f-C_t)/s_t`.

The ratio is evaluated as `2^(zoom_log2_t-zoom_log2_f)` rather than by dividing independently rounded scales, translation uses the owner’s `f64` centre mirrors, and inversion uses the explicit two-by-two determinant; `|det A| ≤ 2⁻⁴⁰` returns `DegenerateWarp` and requests a scene frame instead of manufacturing a warp.

When planes or VIEW angles differ, this warp is a zero-height least-squares visual prediction: it cannot reconstruct components orthogonal to the target plane, escape-height displacement, or the nonlinear standing double perspective; it never authors fractal truth and is replaced by the next completed scene.

## 3. INTERFACES

All wire and GPU records below are little-endian, all RGBA32F channels are IEEE-754 binary32, arrays preserve the coordinate order `(z.re,z.im,c.re,c.im)`, byte offsets are from the containing record, and padding bytes must be zero.

### 3.1 Math-owned CPU types and functions

|Interface|Exact contract|Consumer|
|---------|--------------|--------|
|`CentreF64`|`{ coords: [f64;4] }`; native 32 bytes, finite only, owner mirror with no deep-authority claim|worker, owner, present|
|`CentreSplit`|`{ hi: [f32;4], lo: [f32;4] }`; 32 bytes, offsets 0 and 16, ties-to-even split|kernels|
|`Plane`|`{ basis_u: [f32;4], basis_v: [f32;4], origin_lo: [f32;4] }`; 48 bytes, offsets 0, 16, 32; basis vectors are dimensionless and origin is plane-coordinate units|kernels, present, app|
|`PlaneAngles`|`{ theta_1: f64, theta_2: f64 }`; radians, finite, PLANE angles independent|owner, app|
|`PlanePreset`|`Mandelbrot` or `Julia { c0: [f64;2] }`; `c0` finite and MAIN state|owner, app|
|`EscapeParams`|`{ max_iter: u32, bailout: f32 }`; 8 CPU bytes, `max_iter > 0`, `bailout` is a squared radius with standing value `256.0`|kernels, worker, app|
|`EscapeSample`|`{ smooth_iter: f32, escaped: bool, escape_index: Option<u32> }`; CPU-only oracle result, no stable byte representation|kernels tests|
|`PerturbSample`|`{ smooth_iter: f32, escaped: bool, escape_index: Option<u32>, rebase_count: u32, glitch: bool }`; CPU-only oracle result|kernels tests, overlay arithmetic|
|`OrbitRecord`|`{ re_hi: f32, im_hi: f32, re_lo: f32, im_lo: f32 }`; `repr(C)`, 16 bytes with the RGBA layout below|worker, kernels tests|
|`ReferenceOrbitBytes`|`{ records: Vec<OrbitRecord>, length: u32, precision_bits: u32, escape_index: Option<u32> }`; CPU-owned result whose records become the response payload|worker|
|`OrbitStep`|`Pending { stored: u32 }` or `Complete(ReferenceOrbitBytes)`; CPU-only progress result|worker|
|`DegenerateStage`|`FirstAxis` or `SecondAxis`; CPU-only typed diagnostic|app, tests|
|`WarpMatrix`|`{ forward: [f64;9], inverse: [f64;9] }`; row-major homogeneous matrices, native 144 bytes|present|
|`MathError`|`NonFinite`, `InvalidExtent`, `InvalidMaxIter`, `DegeneratePlane { stage: DegenerateStage }`, `DegenerateWarp`, `InvalidCentreEncoding`, `OrbitTooLong`, `CounterOverflow`, `DurationOverflow`, or `PrecisionExhausted { requested_digits, policy_digits }`|all slices|

The implementation signatures are `construct_plane(preset: PlanePreset, angles: PlaneAngles, centre: CentreF64) -> Result<(Plane,CentreSplit),MathError>`, `split_centre(centre: CentreF64) -> Result<CentreSplit,MathError>`, `pixel_scale(zoom_log2: f64, grid_width: u32) -> Result<f64,MathError>`, `precision_for(zoom_log2: f64, grid_width: u32, max_iter: u32) -> Result<PrecisionPlan,MathError>`, `escape_f32(point: [f32;4], params: EscapeParams) -> Result<EscapeSample,MathError>`, `ReferenceOrbitBuilder::new(centre: &BigCentre, plan: PrecisionPlan, params: EscapeParams) -> Result<ReferenceOrbitBuilder,MathError>`, `ReferenceOrbitBuilder::step(&mut self, max_entries: NonZeroU32) -> Result<OrbitStep,MathError>`, `perturb_f64(orbit: &[OrbitRecord], delta_z0: [f64;2], delta_c: [f64;2], params: EscapeParams) -> Result<PerturbSample,MathError>`, and `warp_matrix(from: &Pose, to: &Pose) -> Result<WarpMatrix,MathError>`.

`ReferenceOrbitBuilder` owns the partial Astro-float state and emits at most `max_entries` new records per call; worker chooses the chunk, checks generation, credit, and deadline and yields between calls, so high-precision arithmetic cannot turn latest-wins into an unbounded wait.

`PrecisionPlan` is `{ floor_digits: u32, working_digits: u32, requested_bits: u32, policy_digits: u32 }` in those units; conversion arithmetic is checked and never saturates silently.

`BigCentre` is `{ coords: [BigScalar;4], precision_bits: u32 }`, where `BigScalar` is the selected Astro-float-backed finite binary value; neither type has a stable native byte layout, and only the canonical encoding in §3.3 crosses a message boundary.

Except for explicitly marked GPU, wire, or `repr(C)` records, CPU types in this section are semantic interfaces with the exact field lists shown and no cross-crate native-layout ABI promise.

### 3.2 GPU records and uniform blocks

|Record|Bytes and offsets|Producer → consumer|
|------|-----------------|-------------------|
|Reference-orbit RGBA32F|16 bytes: 0 `re_hi:f32`, 4 `im_hi:f32`, 8 `re_lo:f32`, 12 `im_lo:f32`; index 0 is `Z₀`; `length` counts stored entries|worker → kernels|
|Escape-grid RGBA32F|16 bytes: 0 `smooth_iter:f32`, 4 `escaped:f32`, 8 `rebase_count:f32`, 12 `glitch:f32`; flags are exactly 0 or 1, count is integer-valued and exactly representable or the pixel glitches before `2²⁴`|kernels → present|
|`ShallowUniform`|96 bytes: 0 `origin_hi:[f32;4]`, 16 `origin_lo:[f32;4]`, 32 `basis_u:[f32;4]`, 48 `basis_v:[f32;4]`, 64 `pixel_scale:f32`, 68 `grid_width:u32`, 72 `grid_height:u32`, 76 `max_iter:u32`, 80 `bailout:f32`, 84 `level:u32`, 88 `reserved:[u32;2]`|math/app → kernels|
|`PerturbUniform`|64 bytes: 0 `basis_u:[f32;4]`, 16 `basis_v:[f32;4]`, 32 `pixel_scale:f32`, 36 `grid_width:u32`, 40 `grid_height:u32`, 44 `max_iter:u32`, 48 `bailout:f32`, 52 `orbit_length:u32`, 56 `level:u32`, 60 `reserved:u32`; no origin field exists|math/app → kernels|
|`HotUniform` payload|64 bytes: 0 `basis_u:[f32;4]`, 16 `basis_v:[f32;4]`, 32 `origin_lo:[f32;4]`, 48 `pixel_scale:f32`, 52 `view_theta_1:f32`, 56 `view_theta_2:f32`, 60 `reserved:u32`; `view_theta_1=0.4t`, `view_theta_2=φ·view_theta_1`|math/app → present|

The hot ring has exactly three `HotUniform` payloads; present owns the buffer, each slot starts at `slot·align_up(64,min_uniform_buffer_offset_alignment)`, selection is by dynamic offset, and the bind-group identity never changes.

Within that payload, `pixel_scale` is the GPU lowering of zoom, `basis_u` and `basis_v` are the GPU lowering of PLANE rotation, and the two view angles are the GPU lowering of VIEW rotation; the owner retains the authoritative f64 zoom and angles for state, overlay, and warp math.

Kernels expose `EscapeGrid { span: DataSpan, width: u32, height: u32, level: u32 }`; `width·height` must equal `span.logical_len`, the span contains the escape-grid record above, kernels own LEVEL definitions and span reuse, present consumes the wrapper, and app alone owns the refinement SCHEDULE.

Kernels expose `ReferenceOrbitSpan { span: DataSpan, generation: u32, length: u32, precision_bits: u32 }`; `length = span.logical_len`, the records have the reference-orbit layout above, and upload is a regional write only when an accepted latest generation arrives.

Every kernel output reaches DATA through the paid scratch-copy path, every heap bind-group identity is stable, and neither escape-grid nor reference-orbit data crosses back to the CPU except for bounded conformance measurement.

### 3.3 Centre encoding and worker messages

`BigCentre` uses four canonical signed dyadics in coordinate order; a coordinate represents `sign·mantissa·2^exponent`, where sign word 0 means canonical zero, 1 positive, 2 negative, exponent is a little-endian two’s-complement `i32`, and mantissa is an unsigned base-`2³²` integer.

Each coordinate encoding begins with four u32 words `{sign, exponent_bits, limb_count, reserved}`, followed by `limb_count` least-significant-first u32 limbs; reserved is zero, a nonzero mantissa’s highest limb is nonzero and bit zero is one after all powers of two move into the exponent, zero has exponent zero and no limbs, and the value is rounded to `precision_bits` ties-to-even before encoding.

Every message buffer begins with the 32-byte, eight-word u32 header `{magic, version, generation, kind, length, precision_bits, compute_us, credit_us}` at byte offsets `{0,4,8,12,16,20,24,28}`; `magic = 0x3152424a` whose bytes spell `JBR1`, `version = 1`, request kind is 1, response kind is 2, and unknown values are typed refusals.

For request kind 1, `length` is the centre-encoding byte count, `precision_bits` is the requested working precision, `compute_us = 0`, and bytes 32 through 47 are `{depth_digits:u32,max_iter:u32,centre_bytes:u32,reserved:u32}` followed at byte 48 by exactly `centre_bytes = length` bytes of canonical `BigCentre`; `depth_digits = ceil(zoom_log2·log10(2))` and reserved is zero.

For response kind 2, `length` is the number of 16-byte orbit records, `precision_bits` is delivered precision, `compute_us` is measured worker compute time in microseconds, and the orbit starts at byte 32 with no gap; conversion of a measured duration beyond `u32::MAX` microseconds is a reported `DurationOverflow`, not truncation.

`credit_us` is the owner’s remaining orbit-compute budget for the next second in u32 microseconds; the owner writes it before returning a transferred buffer, the producer never fabricates credit, and exhausted credit delays work without changing the latest requested generation.

The semantic messages are `OrbitRequest { generation: u32, centre: BigCentreEncoding, depth_digits: u32, max_iter: u32 }` and `OrbitResponse { header: { generation: u32, length: u32, compute_us: u32, precision_bits: u32, credit_us: u32 }, orbit: transferred buffer }` with the exact header and payload layouts above.

There is one wasm module loaded once on the main thread and once in the worker with `worker_main`; browser fetch caching avoids a second download but the two wasm instances have distinct memories, and that duplicated memory is a reported cost.

Transfer uses two independently exchanged buffers in each direction, four total; every buffer is sized for the maximum orbit length of current `max_iter`, resizing occurs only when `max_iter` changes, and every resize is a reported allocation event.

The same-thread lowering invokes the identical request/response abstraction and returns the same buffers synchronously; it is the cheapest lowering, not a separate protocol.

### 3.4 Owner, presentation, and app interfaces

`OrbitHandle(u32)` is an owner-local opaque handle with zero invalid; it is neither a heap handle nor serialized, and it resolves to the latest accepted `ReferenceOrbitSpan`.

`PaletteId(u32)` is an opaque selector into present-owned immutable palette records; app may select it through MAIN state but math, worker, and kernels do not interpret palette contents.

`HotState` is `{ centre_f64: CentreF64, zoom_log2: f64, plane_angles: PlaneAngles, view_time_s: f64 }`; all values are finite, centre and zoom are in fractal plane units and binary-log units respectively, and this is drained every refresh.

`MainState` is `{ orbit: OrbitHandle, orbit_generation: u32, orbit_length: u32, precision_bits: u32, max_iter: u32, palette: PaletteId, preset: PlanePreset }`; `c₀` therefore moves only through MAIN state.

`ViewerState` is `{ epoch: u64, hot: HotState, main: MainState }`; HOT and MAIN share the one epoch, each successful `drain_hot()` or `drain_main()` performs checked `epoch += 1` and returns the corresponding `{epoch,state}` snapshot, both drains are infallible, publication is latest-wins, and u64 wrap is impossible within one session because reaching it at one drain per nanosecond exceeds 584 years.

Orbit `generation` is a monotonically increasing u32 with checked increment; wrap is documented as impossible within a session, stale requests and responses are discarded without publication, and a generation is never inferred from the u64 owner epoch.

`Pose` is `{ plane: Plane, centre_f64: CentreF64, zoom_log2: f64, view_angles: [f64;2] }`; it is CPU-only, angles are radians, and `view_angles` are the evaluated standing VIEW angles for that frame.

Present exposes `Presenter::accept_grid(grid: EscapeGrid) -> Result<(),PresentError>`, `Presenter::write_hot(slot: u32, hot: &HotState) -> Result<(),PresentError>` where slot is 0, 1, or 2, and `Presenter::frame(state: &ViewerState, hot_slot: u32) -> Result<FrameReport,PresentError>` for both flat and tumbled views; `accept_grid` retains the latest typed wrapper without changing heap bind-group identity.

Present exposes `Warp::reproject(last_frame: &CompletedFrame, from_pose: &Pose, to_pose: &Pose) -> Result<FrameReport,PresentError>` and uses math’s `warp_matrix`; a degenerate or absent prior frame requests an ordinary scene frame.

App’s HOT drain calls `Presenter::write_hot`, MAIN arrival publishes the accepted orbit handle, iteration cap, preset, and palette, app schedules one kernel dispatch per selected refinement LEVEL without redefining the levels, and each completed level passes its `EscapeGrid` to `Presenter::accept_grid`.

The facts overlay fields consumed from this slice are `{requested_generation,accepted_generation,owner_epoch,zoom_log2,zoom_digits,precision_floor_digits,precision_working_digits,precision_bits,orbit_length,max_iter,bailout,rebase_count,glitch_count,grid_width,grid_height,level,pixel_scale,centre_recompute_policy,worker_compute_us,credit_us,allocation_events}`; app adds requested/delivered resolution, warm-up labels, four-byte-fence walls, poll counts, scene wall, warp wall, backend facts, and typed refusals.

Before the first frame, app displays clear colour plus honest overlay text and no diagnostic pattern.

## 4. Inherited laws and satisfaction

WebGL2 via wgpu 24 `Backends::GL` is the sole substrate and the `EXT_color_buffer_float` floor is mandatory; this CPU slice adds no feature, format, WebGPU, timestamp-query, or shared-memory requirement.

Per-frame CPU-to-GPU traffic is uniforms only, plus regional writes for changed data: reference-orbit upload happens only on accepted arrival, escape-grid storage stays GPU-resident, centre bignum bytes stay in transferred worker buffers, and heap descriptors change only at allocation.

Kernel outputs use the heap’s paid SCRATCH-to-DATA copy path, heap bind-group identities never change, and the three-slot hot ring uses dynamic offsets into a present-owned buffer.

No shared memory exists: worker buffers transfer ownership with credit in the header, four ping-pong buffers exchange independently, and same-thread execution preserves the same abstraction.

Honesty is structural: requested and delivered precision, resolution, iterations, digits, orbit length, rebase and glitch counts, compute and credit time, allocation events, warm-up, polls, policies, runtime walls, and measurement walls are distinct fields; no number is invented, no control silently snaps, and no wait lacks generation cancellation and a deadline.

Browser-only conformance or performance observations are labelled `requires visible replay`; native arithmetic and Barza probe values are labelled by machine, fixture, command, and measured wall instead.

App installs the heap-provided panic hook and a non-panicking uncaptured-error handler before the first device call, owns the single surface token, and retains the paid four-byte MAP_READ fence discipline with counted polls and no timestamp query.

Scene cost is wall time around a four-byte fence submitted after the scene submission, warp cost is wall time around a separate four-byte fence submitted after the warp submission, every poll is counted, warm-up is labelled and excluded, and neither series uses timestamp queries.

Hand-written `f64` remains the navigation and warp implementation; `faer` enters only if the specified `f64` navigation-drift or warp-accuracy case fails, and prior fixed-size evidence predicts it will not.

Renderer austerity, one-way authority, one world, one heap class, no sim tick, and no gameplay truth in math, heap, kernels, or presentation remain unchanged.

## 5. Oracles and tests

The native Julibrot tests pin bounded fixed points, escaping points including exact bailout equality, `max_iter` edge cases, the state-index convention, smooth values from the natural-log expansion, non-finite rejection, and `-1.0` for every capped non-escape.

The native plane tests pin coordinate order, exact Mandelbrot and Julia presets, the stated matrix multiplication order and signs, `φ`, VIEW angles at deterministic times, independent PLANE angles, `P₄`, f64 then f32 Gram–Schmidt, both degenerate stages, and the `8×f32::EPSILON` postcondition.

Navigation drift composes the five-by-five golden rotation step `R₁₂(Δθ)·R₃₅(φΔθ)` for both `10⁴` and `10⁵` steps with `Δθ = 10⁻³` radians, measures `‖MᵀM-I‖_F = sqrt(Σᵢⱼ(MᵀM-I)ᵢⱼ²)`, and passes at no more than `10⁻⁵` in hand-written f64 with no re-orthonormalization and in f32 with modified Gram–Schmidt on all columns every 64 steps.

Warp accuracy constructs `H` and its explicit inverse for both presets, PLANE- and VIEW-angle fixtures `(0,0)`, `(0.3,-0.2)`, and `(1.1,0.7)`, nonzero finite centre deltas, and `zoom_log2 ∈ {0,10,20,40,80,100}`; it requires `max|H⁻¹H-I| ≤ 10⁻⁹` in hand-written f64 and separately proves the degenerate determinant refusal.

If and only if the f64 case of either CPU-math oracle fails, implementation may add `faer`, rerun the identical corpus and metric, and record the failing hand-written value plus the faer value; an f32 failure alone does not admit faer.

The native split tests cover signed zero, exact f32 values, halfway cases, largest finite values whose `f32` high remains finite, reconstruction error, coordinate order, and typed rejection when a finite f64 cannot produce finite split components.

The native precision tests pin every formula and rounding boundary, distinguish floor, working, and delivered precision, verify checked u32 conversion, exercise the `D+16` convergence loop, and expose the f32 scale-underflow wall instead of calling it arbitrary.

The native orbit tests compare Astro-float results with the same recurrence at `D+16`, pin record zero and length, exact hi/lo channel order, escape truncation, canonical centre-encoding round trips, malformed encodings, generation independence, and the 300-digit policy refusal.

The native perturbation tests use f64 deltas against direct f64 orbit iteration, exercise Mandelbrot, Julia, mixed four-axis fixtures, zero and repeated rebases, reference exhaustion, counter limits, and nonzero `Z₀`; the last fixture is expected to fail under the current shared rebase assignment and blocks implementation until joint review resolves it.

Shallow-kernel conformance requires escape classification and integer escape index exactly equal to `escape_f32` at every sampled pixel and smooth value within `10⁻⁴`; GPU readback and image evidence `requires visible replay`.

Perturbation-kernel conformance uses the CPU f64-delta oracle, requires exact classification and integer escape index on a deterministic corpus whose squared-radius distance from the bailout exceeds the propagated f32 error envelope, and requires smooth value within `2×10⁻³`; samples inside that uncertainty envelope are explicit boundary fixtures, not silently removed from counts, and GPU evidence `requires visible replay`.

The perturbation error envelope starts at the actual f32 rounding error of `δz₀`, propagates `eₙ₊₁ ≤ 2(|Zᵣ|+|δₙ|)eₙ+eₙ²+ρₙ` with `ρₙ` the measured one-ulp bound for the contracted f32 operation sequence, and converts it to squared-radius uncertainty `2|zₙ|eₙ+eₙ²`; this makes the classification tolerance arithmetic rather than a guessed pixel exclusion.

The latest-wins native state tests interleave HOT drain, MAIN arrival, stale generations, and same-thread responses, require monotonic shared u64 epochs and infallible drains, and prove no stale orbit handle can replace the newest accepted generation.

Worker buffer tests pin all header constants and offsets, canonical centre bytes, both kind-specific meanings of `length`, four independent transfer buffers, credit echo, resize-only-on-`max_iter` change, u32 duration overflow, and byte-identical same-thread lowering; browser ownership-transfer behavior `requires visible replay`.

The Barza selection probe used one warmed release run of the exact complex recurrence at bounded fixture `z₀=0`, `c=-0.5+0.5i`: Astro-float 0.9.6 measured 7.530 ms and 73.125 ms at 100 digits for `10⁴` and `10⁵` iterations and 19.516 ms and 229.887 ms at 300 digits, while Dashu 0.6.0 measured 19.852 ms, 204.679 ms, 26.034 ms, and 275.267 ms respectively; these are selection evidence on one machine, not browser predictions.

The same Barza probe built both candidates for `wasm32-unknown-unknown` after Astro-float disabled default features; the initial default-feature build failed because its optional random dependency required a JavaScript getrandom feature, so the pinned dependency is `astro-float = { version = "=0.9.6", default-features = false }`.

## 6. Bignum decision and risks

|Candidate|wasm32 build|Slice-shaped Barza speed|License and maintenance|Decision|
|---------|------------|------------------------|-----------------------|--------|
|[Astro-float 0.9.6](https://docs.rs/astro-float/0.9.6/astro_float/)|PASS with `default-features = false`|Faster at all four final points|MIT; current published release and [active upstream repository](https://github.com/stencillogic/astro-float)|Selected|
|[Dashu-float 0.6.0](https://docs.rs/dashu-float/0.6.0/dashu_float/)|PASS|Slower at all four final points|MIT OR Apache-2.0; current published release and [active upstream repository](https://github.com/cmpute/dashu)|Measured fallback|
|Hand-rolled fixed point|Expected portable but not built|Not measured|Local maintenance and proof burden|Rejected for this prototype|

Astro-float 0.9.6 is selected: it is pure Rust, MIT-licensed, actively maintained, exposes explicit bit precision and ties-to-even rounding, builds for the required wasm target with default features disabled, and won all four final `10⁴`/`10⁵` points in the slice-shaped Barza probe.

Dashu 0.6.0 is also pure Rust, `no_std`, maintained, and MIT-or-Apache-2.0; its operator syntax is simpler and it won the discarded earlier trivial/converging fixture plus no point in the final 100-digit pair and one pre-final 300-digit short point, but its final long-orbit walls were 2.80 times and 1.20 times Astro-float at 100 and 300 digits.

A hand-rolled fixed-point scalar would build on wasm and might remove general-float overhead, but every square requires a specified rescale and rounding, exponent range becomes a local design burden, and there is no independent maintenance or correctness corpus; it is rejected unless the selected crate later fails its bounded orbit budget.

|Risk|Oracle that retires it|
|----|----------------------|
|The shared PLANE rotation cannot create hybrid z/c slices.|Joint review either adds a cross-subspace rotation and updates all five docs, or an algebra test proves a replacement spans both z and c axes; implementation does not begin on the false claim.|
|The mandated rebase assignment is wrong for nonzero `Z₀`.|The direct-orbit versus perturbation test forces a rebase with nonzero `Z₀`; joint review must select an algebraically equal restart before implementation.|
|`f32 pixel_scale` underflows after a width-dependent finite depth.|The precision test reports the exact first zero scale for widths 1, 64, 1,920, and 4,096; joint review must pin a mantissa/exponent uniform to retire arbitrary-zoom risk.|
|Two-f32 reference records carry far fewer bits than a 100–300 digit worker orbit.|The D-versus-D+16 orbit test plus direct deep-pixel classification corpus must show record rounding is adequate, otherwise the orbit record grows or uses scaled residuals before timing.|
|The single native probe does not predict wasm worker throughput.|A worker benchmark at all four selected points, labelled `requires visible replay`, reports compute_us, credit_us, and allocation events before performance claims.|
|The `D_floor+log10(max_iter)` rule cannot bound chaotic amplification by itself.|The mandatory D-versus-D+16 convergence loop either accepts with measured agreement or raises precision to the 300-digit policy/refuses.|
|The `2×10⁻³` perturbation smooth tolerance may be too tight or too loose.|The propagated-error corpus records the worst eligible smooth error; any reviewed change names that observation and does not relax classification.|
|Least-squares warp loses information when planes diverge.|The determinant/refusal test and visible scene-versus-warp error overlay bound the allowed pose delta; visible error evidence `requires visible replay`.|
|Opaque centre bytes in the owner may not be sufficient for deep recentering workflow.|Worker/owner joint review traces initial centre, relative navigation, replacement encoding, and latest-wins transfer byte for byte; a protocol test must complete two deep recenterings without owner-side bignum arithmetic.|
|Astro-float’s default-feature set is incompatible with bare wasm.|The manifest pins default features off and the wasm check remains a gate.|
|Generation and epoch wrap claims could mask unchecked arithmetic.|Native tests start one below each maximum and require a typed session refusal rather than wrap.|
|The smooth formula’s two base-two logarithms may disagree with another slice’s shader.|The shallow conformance test pins source literals and deterministic values before any timing result is eligible.|

## 7. Implementation phases and line budget

Phase 0 adds the package skeleton, Astro-float pin, core errors, coordinate/record layout assertions, centre codec, wasm build check, and native bignum probe fixture, estimated at 330 Rust and test lines.

Phase 1 adds five-dimensional rotation coefficients, presets, plane construction, both Gram–Schmidt passes, centre splitting, pixel mapping, zoom and precision plans, estimated at 380 lines.

Phase 2 adds the contracted `f32` escape reference, smooth count, high-precision reference orbit, D-versus-D+16 validation, record conversion, and native fixtures, estimated at 430 lines.

Phase 3 adds f64 perturbation, rebasing and glitch state machines, propagated error envelopes, mixed-plane conformance fixtures, and counter limits after joint review resolves the rebase defect, estimated at 440 lines.

Phase 4 adds pose and warp matrices, explicit inversion, navigation-drift and warp-accuracy oracles, and the conditional faer decision point, estimated at 330 lines.

Phase 5 reconciles all shared layouts with worker, kernels, present, and app, adds compile-time layout assertions and integration fixtures without editing sibling packages, and closes documentation, estimated at 220 lines.

The implementation budget is therefore about 2,130 new Rust and test lines; Cargo metadata and generated lockfile movement are reported separately, and no phase begins until the five-document written review and refined document are complete.

## 8. Unresolved joint-review list

- R1’s PLANE rotation preserves the Julia and Mandelbrot subspaces after projection and Gram–Schmidt, so it cannot deliver hybrid Julibrot planes.
- R5’s `δ ← zₙ` rebase restart does not preserve `zₙ = Z₀+δ` when the reference starts at nonzero `Z₀`; `δ ← zₙ-Z₀` is the algebraic alternative but is not silently substituted here.
- R6’s scalar `f32 pixel_scale` becomes zero at finite zoom and contradicts arbitrary zoom; the five docs need one mantissa/exponent replacement layout.
- The two-f32 reference record has roughly double-single rather than 100–300 decimal-digit precision, and its adequacy for deep perturbation remains an oracle result rather than a premise.
- The worker-held bignum plus owner-held opaque bytes needs a complete two-recentering ownership trace; the seeded absolute-centre request does not state who updates those bytes after deep relative navigation.
- The request header’s kind-specific `length` meaning and this document’s proposed magic/kind constants need exact agreement with worker and app.
- The 300-digit precision ceiling is a POLICY chosen for this lab, not proof that every requested reference orbit converges before it.
- The exact allowed pose delta and visible-error bound for least-squares warp remain presentation policy and require visible replay.
- Perturbation smooth tolerance `2×10⁻³` is reasoned from an error envelope but lacks GPU evidence until implementation.
- Astro-float won the native slice-shaped probe, but browser worker speed, wasm size, and duplicated instance-memory cost remain unmeasured.
- `Plane.origin_lo` now means the centre split’s low half; the other four documents must confirm that name rather than treating it as an independent affine plane origin.
- The shared wording says each drain bumps the epoch, which makes observation mutate version state; joint review should confirm this is intentional rather than bump-on-publication semantics.
