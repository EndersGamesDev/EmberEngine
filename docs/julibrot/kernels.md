# Julibrot kernels slice contract

Status: implemented slice contract for `crates/labs/julibrot/kernels`; the review rulings in J1–J31 and the adopted mutable allocation/free receiver ruling supersede round-one differences, and the app document remains the integration contract.

## 1. Ownership and boundary

The kernels slice owns GPU data-to-data work: the production shallow `f32` escape kernel, the production perturbation-and-rebasing kernel, their dialect-v2 registration and dispatch plans, one reusable escape-grid DATA span, SCRATCH-to-DATA landing, deterministic conformance fixtures, capacity arithmetic, and the definitions of the three refinement levels.

The kernels slice owns the mode-dependent ladder but not its submission timing: app runs the levels returned for the selected precision mode, cancels stale work by orbit generation rather than owner-epoch equality, and never relabels an unfinished level as delivered.

The kernels slice consumes the math slice's plane, centre split, CPU escape and perturbation oracles, and reference-orbit semantics; it neither constructs `O` or `Q`, performs bignum navigation, nor chooses the bignum implementation.

The kernels slice consumes a reference-orbit DATA span produced through worker and owner state; it does not parse worker messages, own transfer buffers, grant credit, select latest-wins generations, or invent a same-thread transport.

The kernels slice exposes an `EscapeGrid` typed wrapper to present; it does not own palettes, ambient camera presentation, warp, the three-slot hot ring, surface acquisition, or presentation fences.

The kernels slice does not create the GL device, install the panic hook or uncaptured-error handler, define the facts overlay, or own the application runtime; those are app duties and are preconditions of any kernels construction.

The heap dependency remains the shipped `ember-lab-heap` implementation: `DataSpan`, the span directory, descriptor handles, dialect v2, static dispatch headers, the immutable heap bind group, and the paid SCRATCH copy path are reused through the review-approved `GpuKernelExecutor` and are not copied into this package.

No kernel result authors simulation, collision, protocol, reconciliation, or gameplay truth; a result is presentation data and a stale or failed dispatch produces a typed refusal or stale visual, never a truth-state change.

## 2. Design and arithmetic

### 2.1 Coordinates, planes, and pixels

The fractal coordinates are ordered `(z.re, z.im, c.re, c.im) = (e₁,e₂,e₃,e₄)`; `e₅` carries escape height only in present's height field and never enters either fractal kernel.

The user-controlled object rotation is the math-owned six-factor `O∈SO(4)` applied to the one seed `(e₃,e₄)`; kernels receive only its published `u=Oe₃` and `v=Oe₄`, after one rounding to `f32` and the `8·f32::EPSILON` norm/dot postcondition. `e₅`, camera rotation `Q`, both perspective stages, Gram–Schmidt, and any degenerate-plane repair play no part in either fractal kernel.

The former plane angles remain an exact subfamily: with only `ρ₁₃` and `ρ₂₄` nonzero, `O=R₁₃(ρ₁₃)R₂₄(ρ₂₄)` reproduces the previous basis bit for bit. Mandelbrot uses `O=I` and origin zero; Julia at `c₀` uses `ρ₁₃=ρ₂₄=−π/2`, the other four angles zero, and origin `(0,0,c₀.re,c₀.im)`.

For an active grid of width `W` and height `H`, pixel `(i,j)` is the screen-pixel centre `x=f32(i)+0.5−0.5·f32(W)` and `y=f32(j)+0.5−0.5·f32(H)`; row zero is the bottom row, screen `+y` is up, and linear record index is `j·W+i`. The grid is screen-aligned at every refinement level rather than a plane rectangle later projected into a floating quad.

Each uniform carries math's normalized screen-to-plane homography as three padded row-major f32 rows. The kernel evaluates `r_k=(m_k0·x+m_k1·y)+m_k2` in that written order; when its denominator is accepted, `(o_u,o_v)=(r_0/r_2,r_1/r_2)` is the plane offset in pixel-scale units and the four-dimensional offset is `o_u·u+o_v·v`.

Below the displayed switch POLICY `zoom_log2 < 14`, the shallow path uses `pixel_scale=f32(4.0/(2^zoom_log2·W))`, evaluated in `f64` before one `f32` conversion, and forms `o=(o_u·u+o_v·v)·pixel_scale`; at `zoom_log2≥14` the perturbation path forms the same basis combination in the scaled representation of §2.3 and never forms the tiny absolute scale in `f32`. The screen centre remains offset zero under every accepted map, so the reference orbit remains the existing centre and no absolute deep coordinate is introduced.

Let binary32 unit roundoff be `u=2⁻²⁴`, `γ₅=5u/(1−5u)`, `X=W/2−1/2`, `Y=H/2−1/2`, and `S_k=|m_k0|X+|m_k1|Y+|m_k2|` for the f64 normalized row before packing. Coefficient rounding plus the two multiplies and two adds gives the conservative Final-grid dot bound `E_k=γ₅S_k`; for exact `w=r_2`, exact quotient `o_k=r_k/w`, and `|w|>E_2`, the packed-and-evaluated quotient error is bounded by `D_k=(E_k+|o_k|E_2)/(|w|−E_2)+u(|o_k|+(E_k+|o_k|E_2)/(|w|−E_2))` plane pixels. This is evaluated with f64 map evidence at the Final bounds; the kernel mirror and WGSL use the same row parenthesization and an inflated `8u` sum-of-absolute-products allowance in the equivalent quotient test, which dominates `γ₅` without adding a builtin.

At the ledger's 960-by-540 Final extent the largest centred coordinates are `X=479.5` and `Y=269.5`; the formula gives a conservative `3.15e−4`-pixel bound for the exact identity rows, about 795 times inside one quarter pixel, while the actual identity evaluation is exact. Starting from either flat preset-faced row with `d₄=8` and the other 3D camera angle zero, the f64 bound first exceeds one quarter pixel at about `75.019°` pure yaw or `81.454°` pure pitch on the approaching edge, before the geometric horizon reaches that grid at `75.978°` or `82.010°`; the inflated GPU guard may mark uncertainty conservatively earlier.

The unconditional coordinate budget remains `max(D_0,D_1)≤0.25` pixel: the palette's `0.03565` smooth-iteration allowance cannot certify a larger coordinate error because moving the sample can cross an arbitrary escape boundary or change an exact terminal class before palette evaluation. A nonpositive denominator is terminal `Horizon`; a positive denominator whose bound is not finite or exceeds one quarter pixel is the sticky `MapUncertain` annotation. `PictureFast` still divides, samples, and paints every such positive-denominator pixel with the f32 map result, while `Deterministic` may refuse the dispatch rather than claim the uncertified coordinate.

For either flat preset-faced chart and exact `aspect=W/H`, the normalized inverse denominator under pure 3D camera motion is `w(x,y)=1−4[ tan(camera_yaw)·sec(camera_pitch)·x−tan(camera_pitch)·y ]/(W·d₄)`; with `a=aspect·H/W`, the general second term is `tan(camera_pitch)·y/a`. A horizon first enters the Final grid when `4(X|tan(camera_yaw)sec(camera_pitch)|+Y|tan(camera_pitch)|/a)/(W d₄)≥1`; for a pure yaw this is `|camera_yaw|≥atan(d₄W/(4X))`, and for a pure pitch it is `|camera_pitch|≥atan(a d₄W/(4Y))`. General `O` and `Q` rows use the same coefficient bound directly; an edge-on singular forward map is refused by math's `1e−12` pivot gate and app produces an all-sky scene without dispatching invalid rows.

Zoom depth is `zoom_log2·log10(2)` decimal digits, integral `depth_digits = ceil(max(0,zoom_log2·log10(2)))`, and the precision floor is `D_floor = ceil(zoom_log2·log10(2)+log10(W))+8`; worker adds its working margin and reports floor, working, and delivered precision separately, while kernels retain only the supplied orbit bits.

At or above the deep switch, the owner requests a new reference when the centre moves more than one quarter of the view extent or `zoom_log2` differs by more than two from the reference zoom, with worker-owned hysteresis; below the switch app accepts the current centre directly and shallow kernels receive no reference input.

### 2.2 Shallow escape kernel

The shallow kernel receives no heap input, receives only its 144-byte uniform, and writes one RGBA32F escape record per active pixel into the escape-grid span.

Before constructing a fractal point, the shallow path evaluates the map denominator and error guard. It writes `[-1.0,0.0,0.0,2.0]` for `Horizon` without dividing because the plane at that horizon is treated as escaping immediately at infinity; for a positive `MapUncertain` denominator it evaluates the same f32 quotient and recurrence as `Sampled` and writes status `3.0` on the resulting finite escaped or capped record. If the positive divide or coordinate formation overflows, the finite fallback is `[0.0,1.0,0.0,3.0]`, the palette's immediate-exterior sample, never NaN, infinity, or clear.

For coordinate component `k`, the absolute shallow sample is evaluated in the pinned `f32` order `p[k] = centre.hi[k] + (centre.lo[k] + o[k])`; `z₀ = (p[0],p[1])` and `c = (p[2],p[3])`.

Math supplies the separate `CentreSplit` from the exact current centre, including the preset origin: `hi[k] = f32(C[k])` and `lo[k] = f32(C[k]−f64(hi[k]))`; the worker retains the authoritative bignum centre and the shallow GPU pair is not a deep-zoom absolute coordinate.

Complex multiplication is pinned as `mul(a,b) = (a.re·b.re−a.im·b.im, a.re·b.im+a.im·b.re)`, and the shallow recurrence uses `zₙ₊₁ = (zₙ.re²−zₙ.im²+c.re, 2·zₙ.re·zₙ.im+c.im)` in that source order.

For integer `n` from zero through `max_iter−1`, the kernel first tests the current `zₙ`; escape is the strict condition `|zₙ|² > bailout`, equality does not escape, and only a non-escaping state with another iteration remaining evaluates the recurrence.

At escape index `n`, `smooth_iter = n+1−log₂(log₂|zₙ|)`; the natural-log expansion is `n+1−ln(ln(|zₙ|)/ln(2))/ln(2)`, `escaped = 1`, `rebase_count = 0`, and status is the pixel's sticky `Sampled` or `MapUncertain` classification.

At the iteration cap the shallow record is `[-1.0,0.0,0.0,status]` with status zero or three; `max_iter = 0` is a typed refusal and `bailout` is a squared radius fixed to exactly `256.0f32`, so `|z| > 16` escapes.

### 2.3 Perturbation and rebasing kernel

The perturbation kernel receives one reference-orbit DATA span through the descriptor table, receives no absolute centre, receives its 112-byte uniform containing the screen map plus scale mantissa and exponent, and writes the same escape-grid record as the shallow kernel.

Let the exact pixel scale be `m·2^s` with `m ∈ [0.5,1)` carried as `f32` and `s` carried as `i32`; after the identical denominator and error classification, the normalized offset is `o′=(o_u·u+o_v·v)·m`, `δz₀′=(o′[0],o′[1])`, `δc′=(o′[2],o′[3])`, and the per-pixel exponent begins at `e₀=s`, so no absolute tiny scale is formed in `f32`. `Horizon` terminates before a reference load; a finite positive-denominator `MapUncertain` follows the ordinary scaled recurrence with sticky status three, and non-finite coordinate formation produces the same finite immediate-exterior status-three record as shallow.

The represented initial components are `δz₀ = 2^s·δz₀′` and `δc = 2^s·δc′`; Mandelbrot has `δz₀′ = 0` when its rotated basis remains in `span(e₃,e₄)`, Julia has `δc′ = 0` when its rotated basis remains in `span(e₁,e₂)`, and a hybrid plane retains both components.

Reference record `r` supplies the displayed `Zᵣ = (re,im)` directly as two `f32` values; the represented delta is `δₙ = S·δ′ₙ` for `S = 2^e`, full pixel state is `zₙ = Zᵣ+S·δ′ₙ`, and the scaled update is `δ′ₙ₊₁ = 2·Zᵣ·δ′ₙ+S·δ′ₙ²+δc′` with the merge-base complex multiplication order and shared bit-constructed scaling below.

For each outer iteration `n < max_iter`, the kernel first refuses an unavailable reference index as a glitch, then loads `Zᵣ`, constructs `zₙ`, and tests escape; if the pixel does not escape and `n+1 = max_iter`, it records a capped pixel without loading or advancing to another reference, otherwise it applies any rebase and performs exactly one ordinary advance, so escape wins over rebase at the same state.

The corrected rebasing rule is repeatable: when `|zₙ| < |2^e·δ′ₙ|`, set represented `δ ← zₙ−Z₀`, reset reference index `r ← 0`, increment `rebase_count`, normalize that delta as `(δ′,e)`, then perform exactly one ordinary scaled advance against `Z₀` and advance `r` to one; the invariant `zₙ = Zᵣ+δₙ` holds by construction.

A rebase attempt when the current `rebase_count = 2²⁴` glitches before incrementing because the next count is not exactly representable in `f32`; the prior accepted increment may produce the exactly representable value `2²⁴`, so the CPU mirror, WGSL, and record validator share the same boundary.

Before the first state is tested and after each ordinary advance, a nonzero `|δ′|` outside `[2⁻⁶⁴,2⁶⁴]` is renormalized in 64-bit exponent steps repeatedly until it is inside: `δ′ ← δ′·2⁻⁶⁴, e ← e+64` above the range or `δ′ ← δ′·2⁶⁴, e ← e−64` below it; `δc′` is rescaled by the same factor on every step so `δc = 2^e·δc′` remains invariant, and checked `i32` exponent overflow is a typed pixel glitch rather than wraparound.

The mirrors retain a defensive limit of `floor((i32::MAX−i32::MIN)/64) = 67,108,863` successful renormalization steps; every step moves the exponent monotonically by 64 toward one `i32` bound and checked arithmetic refuses the next step, so exceeding that limit is provably unreachable and never replaces repeat-until-restored behavior.

Every represented-value operation uses one shared bit construction in the WGSL kernel and CPU mirror: finite nonzero inputs with exponent above `512` produce signed infinity, those below `−512` produce signed zero, and an exponent in `[-512,512]` is consumed in steps clamped to `[-126,127]`, each multiplying by the exact normal power of two whose f32 bits are `(step+127)<<23`; at most five steps are required. The repeated f32 multiplication preserves IEEE-754 overflow and gradual-underflow behavior bit for bit across the mirror and kernel, infinity is caught by the following exponent-bit finiteness check as a glitch, and an underflowed rebase comparison remains false because a negligible delta must not trigger rebasing.

When `r` reaches reference `length` before escape or the outer iteration cap, iteration stops with `smooth_iter = −1.0`, `escaped = 0`, the accumulated integer-valued `rebase_count`, and `status = Glitch`; glitch takes precedence over the uncertainty annotation, re-rendering those pixels with a second reference is explicitly out of scope, and present uses the honest debug tint.

A non-escaping perturbation pixel that reaches `max_iter` records `[-1.0,0.0,rebase_count,status]` with status zero or three; all branch outputs are finite, so NaN and infinity are neither sentinel values nor accepted records.

### 2.4 Refinement, reuse, and latest-wins

`Deterministic` retains exactly three kernel-defined levels: Preview is `ceil(W/4) × ceil(H/4)` with `min(requested_max_iter,64)`, Interactive is `ceil(W/2) × ceil(H/2)` with `min(requested_max_iter,256)`, and Final is `W × H` with `min(requested_max_iter,4096)`. `PictureFast` instead schedules Preview at `ceil(W/8) × ceil(H/8)` with `min(requested_max_iter,32)`, omits Interactive, and then schedules the same full delivered Final extent and cap.

The value 4,096 is a tab-safety POLICY rather than a hardware wall; `RefinementPlan` retains both requested and delivered caps, and app may request a different future policy only after review rather than presenting 4,096 as detected capacity.

App owns scheduling and latest-wins cancellation: it runs levels in order and may skip, each level it does run is one logical dialect dispatch, and publication compatibility uses the current orbit generation and app plan token rather than owner-epoch equality; `owner_epoch` remains a versioned fact because each HOT or MAIN drain bumps it.

The grid allocation reserves one span for the delivered Final extent and never reallocates between levels; each level densely overwrites the prefix of `width·height` records, `EscapeGrid.width`, `height`, and `level` change only after the command encoder accepts every page pass and exact copy, app applies its current-token publication gate, and present never indexes beyond that active prefix.

The dialect lowering may require several page render passes for one logical level dispatch: for page side `q = 256`, active record count `N`, and `Q = q² = 65,536`, the dispatch uses `P = ceil(N/Q)` prefix pages, with the final dispatch header's `valid_length = N−(P−1)Q`.

Three sets of 16-byte `{global_base,valid_length,0,0}` page headers are generated by the executor's `prefix_headers(span,active_len)` seam and uploaded at grid allocation, each header begins at a stride aligned to live `min_uniform_buffer_offset_alignment`, and each page pass selects its header by dynamic offset without replacing the heap bind group.

If the requested Final extent does not fit after current live allocations, delivery chooses the smallest power-of-two divisor `d` for which an exact cloned-arena allocation of `ceil(W_requested/d)·ceil(H_requested/d)` records succeeds; those dimensions are delivered, and failure even at `1×1` is a typed zero-delivery refusal.

The allocation report keeps requested extent, delivered extent, divisor, logical bytes, reserved bytes, descriptor consumption, directory-handle consumption, and the first failing live wall separate; it does not infer free VRAM from exposed texture limits.

### 2.5 Output landing and cost model

RGBA32F is not blendable on the device floor, every fragment-compute target uses no blend state and `ColorWrites::ALL`, and both production kernels are gather-only with no scatter, atomics, barriers, workgroup variables, raw storage resources, author bindings, or author entry points.

The heap DATA array remains bound only as a sampled resource during a kernel pass; output first renders into one layer of the inherited four-layer RGBA32F SCRATCH array and is copied by `copy_texture_to_texture` into the destination DATA page before presentation reads it.

Dialect v2 forbids `ldexp`, `frexp`, the `packHalf2x16` family, `bitfieldExtract`, `bitfieldInsert`, `countOneBits`, `firstLeadingBit`, `firstTrailingBit`, `dpdx` or `dpdy` outside fragment stage, `textureNumLevels`, every `atomic*` builtin, and any other builtin absent from GLSL ES 3.00. WebGL2 is GLSL ES 3.00.

For a page with side `q` and `V` valid records, landing copies `floor(V/q)` complete rows and, when `V mod q` is nonzero, one exact-width tail row; there is no CPU readback or CPU copy between production and presentation.

For active `N` pixels, logical output is `B_logical = 16N` bytes, the one full-capacity span reserves `B_reserved = 16q²·ceil(WH/q²)` bytes, and each level moves `B_copy = 16N` GPU bytes; SCRATCH is reported separately as `B_scratch = DATA_side²·4·16` bytes and never counted as heap capacity.

At a 960×540 delivered Final extent and `q = 256`, the grid has 518,400 logical records, eight pages, 8,294,400 logical bytes, 8,388,608 reserved bytes, and 94,208 bytes of last-page padding; Preview is 240×135 and copies 518,400 bytes, Interactive is 480×270 and copies 2,073,600 bytes, and Final copies 8,294,400 bytes.

With the inherited 512-side SCRATCH array, its separately reported physical allocation is `512²·4·16 = 16,777,216` bytes; this number is configuration arithmetic, not a claim about free VRAM or a measured wall.

The per-level worst-case work is `C = width·height·iteration_cap` pixel-iterations; early escape and glitch reduce executed iterations but are unavailable without forbidden counters or an explicit measurement readback, so the overlay labels `C` as worst-case arithmetic and never reports it as measured work.

Perturbation performs at most one reference `textureLoad` per executed pixel-iteration, while shallow performs none; rebase does not add an outer iteration, so `rebase_count ≤ executed_iterations ≤ iteration_cap`.

For `R = N mod q²`, exact copy-command arithmetic is `floor(N/q²) + 1[floor(R/q) > 0] + 1[R mod q > 0]`: one command per complete page, one for any complete rows in the partial page, and one for any tail row; command count, page-pass count, copied bytes, and encoder submissions remain separate facts.

## 3. INTERFACES

Every interface in this section is this slice's side of the shared contract and is intentionally duplicated in the other slice documents; disagreement is recorded for joint review and the app document decides integration.

### 3.1 Math to kernels: coordinate records

`Plane` is the math-owned 32-byte little-endian `#[repr(C)]` value `{ basis_u: [f32;4], basis_v: [f32;4] }`; the centre is not a plane property, the deep path has no absolute centre, and shallow receives the separate `CentreSplit` below.

|Byte range|Field|Type|Unit and meaning|
|---------:|-----|----|----------------|
|0–15|`basis_u`|`[f32; 4]`|Unit four-vector in `(z.re,z.im,c.re,c.im)` after the single `f32` rounding pass|
|16–31|`basis_v`|`[f32; 4]`|Unit four-vector orthogonal to `basis_u` in the same axis order|

`CentreSplit` is the math-owned 32-byte little-endian `#[repr(C)]` value `{ hi: [f32;4], lo: [f32;4] }`, with `hi` at bytes 0–15 and `lo` at bytes 16–31; it is supplied only to shallow dispatch and includes the preset's absolute centre.

`EscapeParams` is the CPU-facing `#[repr(C)]` record `{ max_iter: u32, bailout: f32 }`, size eight and alignment four; `max_iter` is requested iterations, `bailout` is the squared-radius value and must be exactly `256.0f32`, and uniform packing supplies explicit padding rather than transmuting this record.

`ScaleSplit` is the math-owned CPU record `{ mantissa: f32, exponent: i32 }` with no byte ABI; it represents `pixel_scale = mantissa·2^exponent`, requires `mantissa ∈ [0.5,1)`, and app may re-export or alias the name without changing the kernels type.

`GridExtent` is the CPU-facing `#[repr(C)]` record `{ width: u32, height: u32 }`, size eight and alignment four, measured in pixels; both fields must be nonzero and their checked product must fit `u32` before any allocation or dispatch.

### 3.2 Worker and owner to kernels: reference orbit

`ReferenceOrbitInput<'a>` is the kernels-owned borrowed record `{ span: &'a DataSpan, generation: u32, length: u32, precision_bits: u32 }`; `length` must equal `span.logical_len`, and app's registry proves the generation is the latest accepted orbit before perturbation dispatch without transferring or cloning span ownership.

The underlying worker message has the independent eight-word little-endian `u32` header `{magic,version,generation,kind,length,precision_bits,compute_us,credit_us}` at bytes 0–31, `magic = 0x314c424a` (`JBL1`), and `version = 1`; its nine kinds are `OrbitRequest = 1`, `RequestReturn = 2`, `OrbitResponse = 3`, `CreditApplied = 4`, `CreditStale = 5`, `OrbitCancelled = 6`, `ChannelError = 7`, `Shutdown = 8`, and `ShutdownAck = 9`.

Each pool buffer has capacity `max(644,64+8·M)` for current maximum orbit length `M` and ends with a 16-byte worker verification-fact tail followed by the 16-byte worker-owned trailer `{pool:u32,slot:u32,capacity_bytes:u32,trailer_magic:u32}`; an `OrbitResponse` begins its 8-byte `length` records at byte 32, and kernels do not parse, retain, credit, or return this transport buffer.

Reference point `n` is transferred as 8-byte `(re,im)` with index zero equal to `Z₀`, the centre's complex `z` part; app expands it to one 16-byte RGBA32F heap texel `(re,im,0,0)`. Length is `min(max_iter,escape_index+1)` when the reference escapes and `max_iter` when it does not.

|Byte range|RGBA lane|Meaning|
|---------:|---------|-------|
|0–3|R|`re`, the consumed `f32` word of `Zₙ.re`|
|4–7|G|`im`, the consumed `f32` word of `Zₙ.im`|
|8–11|B|zero padding added by app during heap upload|
|12–15|A|zero padding added by app during heap upload|

The transferred payload is little-endian IEEE-754 binary32; owner uploads changed orbit records with regional writes into ordinary DATA pages, and the perturbation accessor resolves the span by `directory_index`, ordered descriptor handles, and each page descriptor's own width like every dialect-v2 DATA input.

### 3.3 Kernels to present: escape grid

`RefinementLevel` has `#[repr(u32)]` discriminants `Preview = 0`, `Interactive = 1`, and `Final = 2`; no numeric value is inferred for an unknown discriminant.

`EscapeGrid` is the typed Rust wrapper `{ span: DataSpan, width: u32, height: u32, level: RefinementLevel }`; `span.logical_len` is Final capacity, while `width·height` is the initialized dense prefix present may fetch for the current level.

`SampleStatus` is the closed semantic enum `Sampled=0`, `Glitch=1`, `Horizon=2`, and `MapUncertain=3`; its RGBA32F encoding is the exactly representable corresponding f32 integer and unknown values are malformed. `Sampled` and `MapUncertain` both carry escape/interior results for palette evaluation, `Glitch` is the debug terminal, and `Horizon` is the immediate-exterior terminal.

|Status|Kernel action|Scene presentation|
|------|-------------|------------------|
|`Sampled=0`|Run the recurrence and retain its finite escaped or capped record.|Evaluate the ordinary exterior/interior palette path.|
|`Glitch=1`|Stop on the existing arithmetic/reference failure with a finite record.|Paint the fixed opaque debug tint.|
|`Horizon=2`|Do not divide or iterate; write `[-1,0,0,2]`.|Paint the palette exterior at zero smooth iterations.|
|`MapUncertain=3`|In `PictureFast`, divide and run the recurrence with sticky status three; only `Deterministic` may refuse the grid.|Evaluate the recorded exterior/interior result exactly like status zero.|

Escape-grid texel `(i,j)` is record `j·width+i`, one 16-byte RGBA32F value with the following exact independent channels.

|Byte range|RGBA lane|Meaning|
|---------:|---------|-------|
|0–3|R|`smooth_iter: f32`, the specified smooth value at escape or exactly `−1.0` otherwise|
|4–7|G|`escaped: f32`, exactly `0.0` or `1.0`|
|8–11|B|`rebase_count: f32`, a nonnegative exactly representable integer, zero for shallow|
|12–15|A|`status: f32`, exactly `0.0` sampled, `1.0` glitch, `2.0` horizon, or `3.0` uncertified near horizon|

Present consumes `&EscapeGrid`, resolves its DATA span through the unchanged heap bind group, treats row zero as bottom and `+v` as up, displays status one in the honest debug tint, displays status two as the palette exterior at zero smooth iterations, evaluates status three from its sampled record exactly like status zero, and never samples padding records from `span.logical_len−width·height`.

### 3.4 Uniform blocks

All scalar words below are little-endian, every block is `#[repr(C, align(16))]`, `Pod`, and 16-byte aligned, every padding word is written as zero, and native layout tests assert size, alignment, and each byte offset rather than trusting source declaration order.

The shallow block is exactly 144 bytes and is the only CPU-to-GPU payload of one shallow logical dispatch; its centre words are copied from one `CentreSplit`, its map rows are rounded from math's `Homography`, and neither belongs to `Plane`.

|Byte range|Field|Type|Meaning|
|---------:|-----|----|-------|
|0–15|`basis_u`|`[f32; 4]`|Plane basis `u`|
|16–31|`basis_v`|`[f32; 4]`|Plane basis `v`|
|32–47|`screen_to_plane_row_0`|`[f32; 4]`|First row of `M`, zero-padded|
|48–63|`screen_to_plane_row_1`|`[f32; 4]`|Second row of `M`, zero-padded|
|64–79|`screen_to_plane_row_2`|`[f32; 4]`|Denominator row of `M`, zero-padded|
|80–95|`centre_hi`|`[f32; 4]`|`CentreSplit.hi`|
|96–111|`centre_lo`|`[f32; 4]`|`CentreSplit.lo`|
|112–115|`pixel_scale`|`f32`|Four-dimensional units per pixel|
|116–119|`width`|`u32`|Active grid width in pixels|
|120–123|`height`|`u32`|Active grid height in pixels|
|124–127|`max_iter`|`u32`|Delivered level iteration cap|
|128–131|`bailout`|`f32`|Squared escape radius, exactly 256.0|
|132–135|`level`|`u32`|`RefinementLevel` discriminant|
|136–143|`padding`|`[u32; 2]`|Zero|

The perturbation block is exactly 112 bytes and is the only CPU-to-GPU payload of one perturbation logical dispatch; reference selection remains in the heap resource table and is not duplicated as a raw descriptor handle here.

|Byte range|Field|Type|Meaning|
|---------:|-----|----|-------|
|0–15|`basis_u`|`[f32; 4]`|Plane basis `u`|
|16–31|`basis_v`|`[f32; 4]`|Plane basis `v`|
|32–47|`screen_to_plane_row_0`|`[f32; 4]`|First row of `M`, zero-padded|
|48–63|`screen_to_plane_row_1`|`[f32; 4]`|Second row of `M`, zero-padded|
|64–79|`screen_to_plane_row_2`|`[f32; 4]`|Denominator row of `M`, zero-padded|
|80–83|`pixel_scale`|`f32`|Scale mantissa `m ∈ [0.5,1)`|
|84–87|`width`|`u32`|Active grid width in pixels|
|88–91|`height`|`u32`|Active grid height in pixels|
|92–95|`max_iter`|`u32`|Delivered level iteration cap|
|96–99|`bailout`|`f32`|Squared escape radius, exactly 256.0|
|100–103|`orbit_length`|`u32`|Valid reference records usable at this level, `min(reference.length,iteration_cap)`|
|104–107|`level`|`u32`|`RefinementLevel` discriminant|
|108–111|`scale_exponent`|`i32`|Initial per-pixel exponent `s` in `pixel_scale = m·2^s`|

The inherited per-page `DispatchHeader` is exactly 16 bytes `{ global_base: u32, valid_length: u32, padding: [u32;2] }`; the header buffer uses live dynamic-uniform stride and the heap bind-group identity never changes.

The inherited input resource entry is exactly 16 bytes `{ directory_index: u32, logical_len: u32, 0, 0 }`; shallow registers zero inputs, perturbation registers one accessor named `reference`, and out-of-range generated loads return canonical zero only after the explicit glitch check has prevented their use.

### 3.5 Refinement and dispatch records

`LevelSpec` is `{ level: RefinementLevel, extent: GridExtent, iteration_cap: u32 }` and `RefinementPlan` is `{ requested_extent: GridExtent, delivered_extent: GridExtent, extent_divisor: u32, requested_max_iter: u32, delivered_max_iter: u32, page_side: u16, precision_mode: PrecisionMode, levels: [LevelSpec;3] }`; the fixed array retains stable per-discriminant header selection while `next_refinement_level(mode,level)` omits Interactive in `PictureFast`.

`KernelMode` has `#[repr(u32)]` discriminants `Shallow = 0` and `Perturbation = 1`.

`KernelMode::for_zoom(zoom_log2: f64) -> KernelMode` returns `Shallow` below 14 and `Perturbation` at or above 14; 14 is a displayed POLICY, and app keeps a reference orbit maintained at every depth so crossing the boundary does not create a reference gap.

`ShallowUniform::pack(plane: Plane, screen_to_plane: &Homography, centre: CentreSplit, pixel_scale: f32, extent: GridExtent, params: EscapeParams, level: RefinementLevel) -> Result<ShallowUniform, KernelError>` validates the finite normalized map, nonempty checked extent, fixed bailout, nonzero cap, and positive finite scale before producing the exact 144-byte payload.

`PerturbUniform::pack(plane: Plane, screen_to_plane: &Homography, scale: ScaleSplit, extent: GridExtent, params: EscapeParams, orbit_length: u32, level: RefinementLevel) -> Result<PerturbUniform, KernelError>` additionally validates `mantissa ∈ [0.5,1)` and `1 ≤ orbit_length ≤ max_iter` before producing the exact 112-byte payload.

`escape_shallow_point(point: [f32;4], params: EscapeParams) -> Result<KernelSample, KernelError>` and `escape_shallow_pixel(uniforms: &ShallowUniform, index: u32) -> Result<KernelSample, KernelError>` expose the source-ordered CPU mirror, where `KernelSample` is `{ record: EscapeGridRecord, escape_index: Option<u32> }` and the integer index is conformance evidence rather than a production-grid lane.

`perturb_scaled_offset(uniforms: &PerturbUniform, orbit: &[ReferenceOrbitRecord], offset_prime: [f32;4]) -> Result<KernelSample, KernelError>` and `perturb_scaled_pixel(uniforms: &PerturbUniform, orbit: &[ReferenceOrbitRecord], index: u32) -> Result<KernelSample, KernelError>` expose the source-ordered scaled CPU mirror and return arithmetic failure as the honest per-pixel glitch record while refusing invalid host inputs.

`evaluate_shallow_conformance(observed: KernelSample, expected: math::EscapeSample) -> ConformanceResult` applies the exact classification/index and `1×10⁻⁴` smooth criteria, while `evaluate_perturbation_conformance(observed: KernelSample, expected: math::PerturbSample, envelope: math::PerturbationEnvelope) -> ConformanceResult` returns the closed `ConformanceVerdict::{Pass,Boundary,Fail}`, never promotes a predeclared boundary sample to an exact pass, requires the observed smooth error to fit both the propagated smooth envelope and the `2×10⁻³` picture tolerance, and retains exact rebase and glitch criteria.

`ConformanceVerdict` is `repr(u32)` with `Pass=0`, `Boundary=1`, and `Fail=2`; `ConformanceResult` is the CPU-only record `{ verdict:ConformanceVerdict,boundary:bool,record_well_formed:bool,classification_exact:bool,escape_index_exact:bool,rebase_count_exact:bool,status_exact:bool,smooth_abs_error:f32,smooth_tolerance:f32 }` with no stable byte ABI.

`record_is_well_formed(sample: KernelSample, mode: KernelMode) -> bool` checks finite lanes, the closed exact status code, escape-index presence, integer-valued rebases through `2²⁴`, zero shallow rebases, the fixed `Horizon` record, and the same escaped/capped invariants for `MapUncertain` as for `Sampled`; `VisibleReplayCard` is the CPU-only record `{ id:&'static str,requirement:&'static str }`, and `VISIBLE_REPLAY_CARDS` is the seven-entry kernels-to-app list whose requirement strings all begin `requires visible replay:` and cover both readbacks, SCRATCH landing, present consumption, binding identity, the zoom-14 switch, and the four-byte scene-fence handoff.

`DispatchFacts` is `{ owner_epoch: u64, mode: KernelMode, level: RefinementLevel, requested_extent: GridExtent, delivered_extent: GridExtent, requested_max_iter: u32, delivered_max_iter: u32, active_pixels: u32, worst_case_pixel_iterations: u64, page_passes: u32, copy_commands: u32, gpu_copy_bytes: u64, logical_heap_bytes: u64, reserved_heap_bytes: u64, scratch_bytes: u64, orbit_generation: Option<u32>, orbit_length: u32, draft_pixels_discarded:u32, draft_iterations_discarded:u64 }`; the two appended fields are the exact scheduled pixel count and worst-case iteration count discarded when a Preview or Interactive successor lands, and are zero for Final.

Every `DispatchFacts` byte and count is arithmetic from the accepted plan or a copied owner fact; GPU duration and poll count are deliberately absent because app measures submissions with its four-byte fence.

### 3.6 Public function surface

`plan_refinement(requested_extent: GridExtent, params: EscapeParams, accepts_records: impl FnMut(u32) -> bool) -> Result<RefinementPlan, KernelError>` is the seam-independent Phase-3 planner: it tests power-of-two degraded Final record counts in order and accepts the first count approved by the exact caller predicate; Phase 4 supplies the executor's cloned-arena predicate rather than replacing this arithmetic.

`JulibrotKernels::new(executor: &mut ember_lab_heap::GpuKernelExecutor) -> Result<JulibrotKernels, KernelError>` registers exactly two production dialect-v2 kernels and their immutable pipelines against the executor's immutable heap layout.

`JulibrotKernels::plan(executor: &ember_lab_heap::GpuKernelExecutor, requested_extent: GridExtent, params: EscapeParams) -> Result<RefinementPlan, KernelError>` performs checked extent arithmetic, applies the 4,096 policy, and uses exact cloned-arena allocation trials without mutation.

`JulibrotKernels::allocate_grid(&mut self, executor: &mut ember_lab_heap::GpuKernelExecutor, plan: &RefinementPlan) -> Result<EscapeGrid, KernelError>` allocates one Final-capacity `DataSpan`, asks `prefix_headers` for all three static header sets, uploads them once, privately retains the `(DataSpan,HeaderSetHandle)` lifetime pair keyed by exact span identity, and returns no partially allocated value on failure; a header-capacity refusal reports the requested contiguous set count, total free set regions, and longest free run; app.md §3.2 adopts this receiver.

`JulibrotKernels::encode_shallow(&self, executor: &ember_lab_heap::GpuKernelExecutor, encoder: &mut wgpu::CommandEncoder, grid: &mut EscapeGrid, owner_epoch: u64, level: RefinementLevel, plane: &Plane, screen_to_plane: &Homography, centre: &CentreSplit, pixel_scale: f32, params: EscapeParams) -> Result<DispatchFacts, KernelError>` packs the 144-byte uniform, encodes page passes and exact-region copies, and tags the arithmetic receipt with the supplied epoch without using epoch equality as a compatibility test.

`JulibrotKernels::encode_perturbation(&self, executor: &ember_lab_heap::GpuKernelExecutor, encoder: &mut wgpu::CommandEncoder, grid: &mut EscapeGrid, owner_epoch: u64, level: RefinementLevel, plane: &Plane, screen_to_plane: &Homography, scale: ScaleSplit, params: EscapeParams, reference: ReferenceOrbitInput<'_>) -> Result<DispatchFacts, KernelError>` packs the map and math-owned scale split into the 112-byte uniform, rejects older or same-generation conflicting orbit identities, refreshes resource words only when the accepted orbit span identity changes, and performs the one-span gather dispatch.

`JulibrotKernels::free_grid(&mut self, executor: &mut ember_lab_heap::GpuKernelExecutor, grid: EscapeGrid) -> Result<(), KernelError>` looks up the private resident-header lifetime record, returns the span, header reservation and directory entries transactionally after app and present have relinquished all borrows, and removes private state only after executor release succeeds; app.md §3.2 adopts this receiver.

The public category-only error set is `KernelError::{InvalidExtent,ArithmeticOverflow,ScaleExponentOverflow,InvalidEscapeParams,UnknownLevel,MissingReference,StaleReference,ReferenceLengthMismatch,ReferencePrecisionMismatch,Heap,Register,Dispatch,OutputTransferUnsupported,DeviceLost}`; kernels collapses heap implementation detail at this boundary, while the direct executor diagnostics retain their typed source and registered kernel name for app initialization scopes.

`GpuKernelExecutor` is the review-approved minimal public seam extracted by the app lane from the already-paid heap lattice runtime: it owns DATA and four-layer SCRATCH textures, `SpanArena`, immutable bind group, descriptor/directory/header/resource/uniform buffers, exact-row copy encoding, and live capacity reports; if extraction requires more than moving existing code behind a public boundary, the app lane stops and reports rather than forking behavior.

The executor method is `GpuKernelExecutor::prefix_headers(&self, span: &DataSpan, active_len: u32) -> Result<StaticHeaders, DispatchError>`; it requires `1 ≤ active_len ≤ span.logical_len`, emits only the prefix page headers with the exact final valid length, uses the live dynamic-uniform alignment, and performs no upload or allocation.

`GpuKernelExecutor::reserve_header_sets(&mut self, sets: &[StaticHeaders]) -> Result<HeaderSetHandle, DispatchError>` uploads the three immutable level sets once and ties their reclamation to the owning span; each page is then encoded with `encode_dispatch_selected` and `DispatchSelector { set: level as u32, page }`, so changing level changes only the dynamic header offset.

### 3.7 App and present coordination

Worker-owned `HotState` remains the 40-byte record `{ zoom_log2:f64 @0, plane_theta_1:f64 @8, plane_theta_2:f64 @16, centre_from_reference_px:[f64;2] @24 }`; the two angle words mirror object `ρ₁₃` and `ρ₂₄` for worker compatibility, while app carries the complete six-angle record in its accepted stamp. App drains HOT every refresh and calls present-owned `Presenter::write_hot` for one of three dynamic-offset slots, while kernels use only zoom and the math-produced plane for a due scene.

MAIN state carries the reference-orbit handle, requested and delivered iteration caps, plane seed axes and origin including Julia `c₀`, palette selection, precision and orbit facts, and the accepted `reference_shift_px`; a MAIN arrival never rebuilds heap bind groups and changes only the orbit DATA region, resource words, uniforms, or present-owned records that actually changed.

Both HOT and MAIN drains are infallible and each bumps the shared `u64` epoch, but consumers never use epoch equality for compatibility; app uses orbit generation and its refinement plan token to prevent an older encoded dispatch from replacing the published `EscapeGrid` level or facts.

Present owns `new`, `set_main`, infallible `write_hot`, `submit_scene`, `frame`, `poll`, and `facts`; `Warp::reproject(last_frame,from_pose,to_pose)` is a pure CPU planner over math's `Pose`, scene and warp own separate four-byte fences, and none of those calls changes escape-grid records.

## 4. Inherited laws and satisfaction

|Law|Kernels-side satisfaction|
|---|-------------------------|
|WebGL2 floor|App supplies an executor built from its wgpu 24 `Backends::GL` device after checking RGBA32F render, copy, and nearest-sample support; kernels creates no instance, adapter, device, or WebGPU path.|
|Uniforms-only per frame|A logical dispatch uploads only its 144-byte or 112-byte uniform; accepted orbit identity changes refresh resource words, orbit payload and heap metadata change only on MAIN arrival or allocation, and static level headers are pre-uploaded.|
|GPU-resident output|Escape records render to SCRATCH, copy directly into DATA, and reach present by `DataSpan`; production performs no CPU readback.|
|Immutable binding identities|DATA, SCRATCH executor state, descriptor UBO, directory UBO, dispatch header buffer, resource buffer, and kernel uniform buffer are created before frames; dynamic header offsets and regional writes change contents only.|
|Hot ring|Present owns exactly three slots selected by dynamic offset; kernels consume the same frozen HOT snapshot through their own dispatch uniform and do not allocate a second hot ring.|
|No shared memory|The only deep input is an owner-uploaded transferred orbit span; no shared-memory worker path or same-thread special representation enters kernels.|
|Gather-only fragment compute|Each output fragment owns one linear pixel and may only load its matching reference indices; dialect rejection covers scatter-capable storage, atomics, barriers, bindings, and entry points.|
|No float blending|RGBA32F targets specify no blend state and overwrite all four lanes.|
|Honest facts|Requests, delivered extent and cap, policies, live walls, logical and reserved heap bytes, separate SCRATCH bytes, passes, copies, worst-case work, orbit metadata, and browser measurements are named separately.|
|Never hang|Every loop is bounded by the delivered cap of at most 4,096; app owns deadline-bounded four-byte fences, counted polls, cancellation, and visible failure publication.|
|Panic and GPU errors|App installs the panic hook and non-panicking uncaptured-error handler before constructing the executor; kernels return typed errors and contain no bare unreachable path.|
|Math evidence|Kernel tests consume hand-written CPU references for shallow escape and scaled perturbation; `faer` is absent unless the navigation-drift or warp oracle fails its `f64` threshold.|
|Scope austerity|There is one world, one heap class, no DAG or petgraph, no simulation tick, no shared-memory threads, no WebGPU, and no second-reference glitch repair.|

The external navigation oracle composes `10⁴` and `10⁵` steps of `R(Δθ)` with `Δθ = 10⁻³` radians and measures `‖MᵀM−I‖_F`; pass is at most `10⁻⁵` for `f64` without re-orthonormalization and for `f32` with Gram–Schmidt every 64 steps.

The external warp oracle requires `max|H⁻¹H−I| ≤ 10⁻⁹` in `f64` at `zoom_log2 ∈ {0,10,20,40,80,100}`; kernels gain no `faer` dependency unless the hand-written `f64` case fails.

## 5. Oracles and tests

Native layout tests assert `Plane = 32`, `CentreSplit = 32`, shallow uniform `= 144`, perturbation uniform `= 112`, reference record `= 8`, escape record `= 16`, all six padded homography rows, every byte offset in §3, little-endian pack/unpack fixtures, zero padding, exact signed exponent bytes, and exact enum and status discriminants.

Native math-and-kernels coordinate tests cover both preset object rows, the six-angle reduction to the former `R₁₃·R₂₄` multiplication order, the `π/2` plane exchange, hybrid bases with nonzero z and c components, the `8·f32::EPSILON` postcondition, odd and even extents, bottom-left and top-row screen centres, row-zero-at-bottom indexing, exact identity-map offsets, non-affine perspective offsets, and checked scale-exponent overflow.

The screen-map CPU mirror evaluates the same packed rows, parenthesization, guard, and divides as both WGSL kernels. Its Final-grid corpus covers positive, zero, negative, and small-positive denominators, requires exact `Sampled`, `Horizon`, and `MapUncertain` status agreement, proves `Horizon` never divides or loads an orbit, proves `MapUncertain` follows the same recurrence and finite-record invariants as `Sampled` in `PictureFast`, and checks the §2.1 `D_k≤0.25` inequality at every certified point while retaining the measured failure for every uncertified point. A deterministic-policy fixture may refuse a grid containing status-three candidates, but the picture-fast fixture may not.

Native refinement tests pin the deterministic three-level dimensions and caps, the fast one-eighth Preview and Preview-to-Final sequence, per-level discarded-work facts, exact power-of-two extent degradation, one Final-capacity allocation, prefix page counts and last valid lengths, immutable span handles across all scheduled levels, requested-versus-delivered facts, and zero-delivery behavior.

Native heap-path tests register both bodies through dialect v2, prove shallow has zero accessors and perturbation has exactly `load_reference`, exercise alias and stale-generation refusals, require `prefix_headers` for every dense level, pin dynamic header offsets, and verify copy rows, tail width, copied bytes, command counts, and separate SCRATCH accounting.

The shallow CPU conformance fixture uses deterministic pixels in both presets and one rotated hybrid plane; pass requires GPU and CPU escape classification and integer escape index to match exactly and `|smooth_gpu−smooth_cpu| ≤ 10⁻⁴`, with a conformance-only auxiliary target carrying the integer index because the production grid does not.

The perturbation CPU fixture uses math's scaled `f64` mirror and deterministic pixels that include normalized `δz₀′ = 0`, `δc′ = 0`, both nonzero, exponents on both sides of the normal f32 range, exact bit-constructed scaling boundaries, gradual underflow and signed saturation, upward and downward repeat-until-restored renormalization from the smallest subnormal, zero and repeated rebases, reference exhaustion, and nonzero `Z₀`; the corrected nonzero-`Z₀` rebase is a PASS criterion.

Math's merged `escape_f32`, `perturb_scaled_f64`, and propagated-envelope functions are unconditional test dependencies with no placeholder feature; every ordinary package and workspace test run executes their cross-package comparisons.

Perturbation conformance requires exact classification and integer escape index outside math's propagated error envelope, exact rebase count and status, and `|smooth_gpu−smooth_cpu| ≤ 2×10⁻³`; samples inside the envelope remain explicit boundary fixtures and are never silently removed from reported counts.

The reference adequacy oracle recomputes at working precision `D` and `D+16`, requires identical escape index and both emitted coordinate words within two f32 ulps, then runs the deep scaled-classification corpus; failure escalates precision before a Final is re-issued.

The production-output oracle checks every record for finite channels, exact binary escaped values, one of the four exact status codes, the exact fixed `Horizon` record, sampled invariants for every `MapUncertain` record, integer-valued rebase count, zero shallow rebase count, and no read or presentation access beyond the active prefix.

The dialect source oracle parses and validates the generated WGSL, rejects every forbidden construct, proves the homography and guard add no builtin to the existing dialect list, lowers both the shallow and perturbation modules through naga 24.0.0 to WebGL GLSL ES 3.00 for their vertex and fragment entry points, proves input addressing uses the reference page descriptor rather than output width, and proves changing the reference span changes resource words without pipeline or bind-group replacement.

The shallow GPU readback, conformance-only integer target, perturbation GPU readback, vertex-stage consumption, SCRATCH copy at nonzero DATA origin and layer, row orientation, and a clean browser console all require visible replay.

The immutable-bind-group identity across orbit replacement and all three levels, live capability facts, exact copied-byte report, and absence of a DATA feedback-loop validation error require visible replay.

Dispatch walls, scene walls, and poll counts are browser facts measured by app around four-byte fences with no timestamp queries; every such number remains `requires visible replay` until that replay occurs.

## 6. Risks and retirement oracles

|Risk|Oracle that retires it|
|----|----------------------|
|Mantissa/exponent arithmetic may lose scale during renormalization or rebasing.|The scaled f64 mirror forces both ±64 exponent transitions and nonzero-`Z₀` rebases, checks `δ = 2^e·δ′` and `δc = 2^e·δc′` before and after each transition, and requires exact control-flow facts.|
|The accepted smooth tolerance could mask boundary classification drift.|Math's propagated envelope classifies every fixture as eligible or boundary before GPU execution; eligible classification and index remain exact, boundary fixtures remain counted, and smooth error stays at most `2×10⁻³`.|
|Dense-prefix reuse could expose stale coarse or padding records.|Native index bounds plus visible replay of Preview→Interactive→Final and Final→Preview require checksums for only each declared active extent and poison padding in the test build.|
|Buddy fragmentation or directory exhaustion can defeat byte-only capacity estimates.|Every delivery uses an exact cloned-arena allocation, and fragmentation fixtures compare reported first wall with the typed allocator refusal.|
|Runtime-sized fragment loops can trigger a slow-driver watchdog despite the 4,096 policy.|Visible replay fences each level, counts polls, enforces app's deadline, and retains requested versus delivered caps; a timeout is failure evidence, never admission data.|
|Reference replacement could pair new metadata with old DATA records.|Generation-tagged upload fixtures delay publication until data and resource words are queued, then deliberately dispatch stale generations and require `StaleReference`.|
|Extracting the paid GPU executor or prefix helper could accidentally fork heap semantics.|An integration oracle runs the heap golden and Julibrot dispatch through the same executor type and compares bind-group identity, full and prefix header bytes, copy regions, capacity facts, and typed failures; a non-visibility extraction stops the app lane.|
|RGBA32F or copy usage may be absent despite nominal WebGL2.|Initialization checks live format usages and the standing output-path golden; refusal names the adapter, backend, and failed usage.|
|Aggregate rebase and glitch totals are unavailable without readback or another reduction kernel.|The normal overlay labels totals unavailable and presents per-pixel debug tint; an explicitly requested measurement readback may count them and must report its fence and polls.|

## 7. Implementation phases and line budget

Implementation checkpoint: Phases 0–6 are complete in this slice, `JulibrotKernels` privately owns each span's resident header-set handle, exact cloned-arena trials select delivery, allocation reserves all three immutable prefix sets, every encode call lowers one logical level into page passes plus paid exact-row copies, math-oracle comparisons are unconditional, and the remaining browser observations stay explicitly replay-only.

Phase 0, estimated 230 new lines, creates the kernels package, pins `Plane`, `CentreSplit`, all GPU records and uniforms, exposes the two dialect descriptors, and adds source, packing, switch, and hybrid-coordinate tests.

Phase 1, estimated 310 new lines, implements the shallow CPU mirror and WGSL body, exact escape-index conformance instrumentation, `CentreSplit` uniform packing, and deterministic native fixtures below the zoom-14 switch.

Phase 2, estimated 480 new lines, implements reference upload validation, scaled perturbation, per-pixel exponent and ±64 renormalization, corrected rebasing, glitch behavior, accepted error-envelope fixtures, and the nonzero-`Z₀` pass oracle.

Phase 3, estimated 340 new lines, implements the three-level plan, power-of-two delivered extent, full-span dense-prefix reuse through `prefix_headers`, static header sets, capacity reports, and typed allocation errors.

Phase 4, estimated 340 new lines, integrates the app-owned public heap GPU executor seam, one logical dispatch over page passes, exact SCRATCH copies, bind-group identity evidence, and kernels-to-present publication.

Phase 5, estimated 260 new lines, adds browser conformance entry points, four-byte-fence handoff facts, visible replay cards, cancellation interleavings, and page-contract fixtures owned jointly with app.

Phase 6, estimated 180 new lines, reconciles all five documents, resolves or records interface differences, runs package and workspace gates, and accounts for actual net lines against the 2,000-line estimate.

The total implementation estimate is 2,140 net new Rust, WGSL, JavaScript fixture, and test lines; documentation and the app-owned visibility-only heap seams are reported separately rather than hidden inside that estimate.

The completed package is 2,685 lines, 545 lines or 25.5% above the 2,140-line estimate: the adopted lifetime ruling required the 538-line executor adapter with rollback, live header walls, span-keyed handles and stale-reference state, while the now-explicit conformance result and seven replay cards account for 283 lines; no heap implementation was copied to pay that overrun.

## 8. Final exact-versus-fast corpus

The Final exact-versus-fast corpus lives in `crates/labs/julibrot/app/tests/final_conformance.rs`, not inside kernels, because the acceptance result crosses the math and kernels CPU mirrors, present's palette/lighting result, and math/present warp oracle. The app integration-test crate already depends on all three owners, so this placement exercises their public contracts without adding a kernels-to-present dependency cycle. Each fixture is rendered through `PrecisionMode::Deterministic` and `PrecisionMode::PictureFast`; until a real fast implementation lands, the latter deliberately selects the same mirror after carrying the distinct mode flag, so a fast-path lane adds a case or changes one dispatch branch rather than rewriting the corpus.

The corpus covers the zoom-14 boundary, zooms 40, 80, 100, 256, 512, and the last positive binary64 zoom admitted by `precision_for(zoom,960,4096)` under the 300-digit policy; the admitted fixture is approximately `q=946.808400731952`, has floor 296 and working 300 decimal digits, and its immediately adjacent binary64 successor is asserted `PrecisionExhausted`. It crosses caps 64, 256, 512, and 4,096, the Mandelbrot and Julia preset rows plus a rotated hybrid plane, all three palettes, upward and downward normalization, zero/nonzero/repeated rebases, and the existing shallow and perturbation boundary fixtures. Outside the propagated envelope, terminal class and escape index are exact; escaped lit colour is within one `Rgba8Unorm` code per channel; interior, clear, debug, and alpha are exact; and uploaded warp source coordinates are within 0.25 pixel.

## 9. Unresolved joint-review findings

- Each reference coordinate carries one binary32 word rather than the worker's full 100–300 decimal-digit precision; the accepted D-versus-D+16 validation, half-ulp reconstruction bound, and deep scaled corpus decide adequacy, and a failure escalates Final precision.

- Bit-constructed power-of-two scaling now avoids GLSL ES 3.10-only builtins and has native GLSL ES 3.00 translation coverage for both stages; a clean visible WebGL2 replay must still confirm the device compiler and underflow mode, while exponent wrap, NaN, or backend validation failure remains unacceptable.

- The executor seams are now public and kernels retains resident headers privately under the adopted mutable allocation/free receiver ruling; browser evidence must still prove all three set selections preserve heap bind-group identity.

- `EscapeGrid` owns a cloneable `DataSpan`, while present may retain or submit a scene that names it; app and present must prove the lifetime handoff that prevents `free_grid` from reclaiming a span still in flight.

- Aggregate rebase and glitch totals remain unavailable during normal gather-only rendering; an explicitly requested measurement readback may count them, but its fence, polls, generation, and effect on timing qualification still need implementation evidence.

- A reference can legitimately end before a nearby pixel escapes, but no policy requests another reference from a high glitch fraction; second-reference repair is out of scope, so v1 exposes only the debug tint and measured limit.

- Exact shallow classification across CPU and browser shader still depends on math's predeclared boundary fixtures and contracted operation order; fused-operation behavior must not be accommodated by selecting samples after GPU results are seen.

- Math's shallow and scaled perturbation oracles are live unconditional fixtures; browser shader results remain the missing half of those comparisons until visible replay.

- The zoom-14 mode switch is mathematically safe but browser pipeline-switch cost, first-frame warm-up, and visual continuity across one shallow and one scaled perturbation frame remain `requires visible replay`.

- The four-layer SCRATCH allocation is inherited even though these kernels write one output; its paid compatibility is known, but the memory cost versus a one-layer extracted executor remains an implementation measurement and is not reopened in this lab.
