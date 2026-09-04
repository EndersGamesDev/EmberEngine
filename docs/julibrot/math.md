# Julibrot math slice contract

Status: refined slice document for `crates/labs/julibrot/math` after the five-document written review; this is the implementation contract for math and the interface contract its four peer slices code against.

## 1. Ownership and exclusions

The math slice owns the Julibrot definition, coordinate conventions, PLANE construction and presets, `f32` escape reference, Astro-float centre, authoritative drag and anchored-zoom algebra, reference-orbit arithmetic, the owner's `f64` centre mirror, exact hi/lo splitting, scaled perturbation and rebasing semantics, precision selection, pose and flat warp matrices, and the navigation-drift and warp-accuracy oracles.

The slice is CPU-only: it supplies records, pure functions, arithmetic contracts, and native tests, while kernels own dialect-v2 registration, GPU dispatch, the three refinement levels, iteration caps per level, span reuse, conformance readback, and scratch-copy landing; worker owns scheduling, transfer, validation, credit, and owner publication; present owns rendering, palettes, scene textures, the hot-ring allocation, view-specific warp planning and submission; app owns the runtime and refinement schedule.

The slice uses `crates/labs/heap` by dependency and does not copy or edit its dialect-v2, span, scratch-copy, presentation, fence, surface-ownership, panic-hook, or error-handler mechanisms in this document round.

The slice does not own WebGPU, a general DAG or petgraph, more than one world, a simulation tick, more than one heap class, shared-memory threads, GPU shaders, gameplay truth, or a second-reference repair for glitched pixels.

## 2. Mathematical design

### 2.1 Julibrot and escape reference

The Julibrot is `J = {(z₀,c) ∈ ℂ² : zₙ₊₁ = zₙ²+c is bounded}`; real axes are ordered `(z.re,z.im,c.re,c.im) = (e₁,e₂,e₃,e₄)`, so Mandelbrot is the plane `z₀=0`, Julia at `c₀` is the plane `c=c₀`, and reaching a finite iteration cap means only “not escaped by this cap.”

The CPU `f32` escape reference examines states at indices `n=0..max_iter−1`, declares escape at the first `n` for which `|zₙ|² > bailout`, and otherwise advances `zₙ₊₁=zₙ²+c`; `bailout` is the squared radius and is exactly `256.0`, so equality does not escape.

At escape index `n`, `smooth_iter = n+1−log₂(log₂|zₙ|)`; using natural logarithms this is `n+1−ln(ln(|zₙ|)/ln 2)/ln 2`, while a sample not escaped by the cap stores exactly `−1.0`.

`max_iter=0`, a non-finite input, or a non-finite intermediate is a typed error; the f32 classification-and-index path is operation-identical to shallow WGSL, spelling squared magnitude as `z_re*z_re+z_im*z_im`, real advance as `(z_re*z_re−z_im*z_im)+c_re`, and imaginary advance as `(2*z_re*z_im)+c_im` in that exact source order with one binary32 rounding per written operator and no fused multiply-add contraction, while smooth output retains its separate `10⁻⁴` tolerance.

### 2.2 Object and camera poses

The fractal plane lives only in ℝ⁴; `e₅` carries escape height only after present lifts that plane and never enters a plane, centre, worker message, or perturbation offset.

For column vectors, `Rᵢⱼ(θ)` has the standard block `[[cos θ,−sin θ],[sin θ,cos θ]]` on axes `(i,j)` and identity elsewhere, every angle is an independent HOT control in `[−π,π]`, and the rightmost factor acts first.

The object rotation is `O=R₁₂(ρ₁₂)R₁₃(ρ₁₃)R₁₄(ρ₁₄)R₂₃(ρ₂₃)R₂₄(ρ₂₄)R₃₄(ρ₃₄)∈SO(4)`; with column vectors its explicit application order is `R₃₄`, `R₂₄`, `R₂₃`, `R₁₄`, `R₁₃`, then `R₁₂`. There is one seed and the sampled basis is always `u=Oe₃`, `v=Oe₄`; a general object-angle change tilts the 2D slice in ambient ℝ⁴, while a member of the current plane's stabilizer changes only its chart basis. Stabilizer membership belongs to the composed pose rather than to a parameter name: at `O=I`, `ρ₃₄` rotates within the sampled plane and `ρ₁₂` is inert for every absolute plane origin, while from a tilted base the same `ρ₁₂` change can tilt the sampled span.

`ObjectAngles` stores the six angles in product order. The legacy `PlaneAngles` adapter maps `θ₁` to `ρ₁₃` and `θ₂` to `ρ₂₄`; setting the other four components to zero makes `O=R₁₃(θ₁)R₂₄(θ₂)` with the former application order and bit-for-bit results after the shared rounding pass.

Construction evaluates `O` in `f64`, applies it to the orthonormal seed, rounds each published basis component exactly once to nearest `f32` with ties to even, performs no projection or Gram–Schmidt stage, and requires `|u·u−1|`, `|v·v−1|`, and `|u·v|` each at most `8·f32::EPSILON` after rounding.

Plane compatibility compares the constructed bases rather than the six parameters. The requested basis is projected into the retained span and is the same plane when both four-dimensional residual norms are at most `8·f32::EPSILON`; its 2-by-2 overlap is then replaced by the exact orthogonal rotation or reflection with the same first-column angle. The published chart map is the transpose, taking retained chart coordinates to requested chart coordinates. At `O=I`, changing `ρ₃₄` by `α` gives basis rotation `α` and chart rotation `−α`, while `ρ₁₂` is identity on the seed plane; after a general `O`, a rightmost `ρ₃₄` increment remains an in-plane rotation even though the six parameter roles otherwise mix. This plane stabilizer is the slice-level rule one dimension down from the arena's ATW limit: only motion outside the sampled 2-flat asks for new content.

The object rows are Mandelbrot `O=I` with origin `(0,0,0,0)` and Julia at finite `c₀` with `ρ₁₃=ρ₂₄=−π/2`, the other four angles zero, and origin `(0,0,c₀.re,c₀.im)`. The Julia row therefore preserves the former once-rounded `(e₁,e₂)` basis, while the four origin coordinates are the translation of the object pose in `SE(4)` and remain independent MAIN state that set the absolute centre.

The camera rotation is `Q=R₁₂(q₁₂)R₁₃(q₁₃)R₁₄(q₁₄)R₂₃(q₂₃)R₂₄(q₂₄)R₃₄(q₃₄)R₁₅(q₁₅)R₂₅(q₂₅)R₃₅(q₃₅)R₄₅(q₄₅)∈SO(5)`; its explicit application order is `R₄₅`, `R₃₅`, `R₂₅`, `R₁₅`, `R₃₄`, `R₂₄`, `R₂₃`, `R₁₄`, `R₁₃`, then `R₁₂`. The camera translation `t∈ℝ⁵` is then added after `Q` and before `P₅`, so the ambient camera pose is `SE(5)` rather than rotation alone.

`ViewControls` is `{camera:[f64;10],camera_translation:[f64;5],camera_yaw,camera_pitch,height_scale,distance_five,distance_four}`, twenty scalars with neutral rotations and translation zero, neutral height zero, and both neutral distances eight. The page limits each translation to `[−8,8]` chart units: the neutral chart spans four units, so this range crosses it completely and can retreat past either perspective pole.

The flat Mandelbrot preset faces its seed with `q₁₃=q₂₄=−π/2` and all other camera values zero; the flat Julia preset uses `Q=I`, `t=0`. At height zero, zero 3D camera angles, and `d₅=d₄=8`, each paired object/camera row is the exact identity chart map. Object and camera poses remain independent: changing `O` at fixed `(Q,t)` turns and foreshortens the slice in ℝ⁴ until it can be edge-on, while changing `(Q,t)` at fixed object pose moves the observer around and through that slice.

### 2.3 Pixels, the screen map, centre, zoom, precision, and splitting

For a `W×H` grid, pixel `(i,j)` has centred coordinates `x=i+0.5−W/2` and `y=j+0.5−H/2`; row zero is at the bottom, `+v` points up, pixels are square, and `H` follows the canvas aspect without changing horizontal pixel scale.

The escape grid is screen-aligned: a grid pixel denotes the inverse image of that screen-pixel centre through the accepted neutral-height view, not a cell of a fixed rectangle projected afterward. The inverse returns plane offsets `(o_u,o_v)` in units of the current pixel scale about the centre, so the sampled point remains `C+pixel_scale·(o_u u+o_v v)` and the reference orbit remains at the centre pixel.

At neutral height the object point is `p=(4/W)(o_u u+o_v v)+plane_origin`; present lifts it to `(p,0)∈ℝ⁵`, applies `Q`, adds `t`, then applies `P₅`, `P₄`, the yaw/pitch 3D camera, clip, and the centred-pixel viewport. Restricted to the plane this chain is projective, so math constructs its forward 3-by-3 homography `F`, inverts it with partial pivoting, refuses a selected pivot below `1e−12`, normalizes the inverse `M` to positive centre denominator, and publishes `κ∞(F)=‖F‖∞‖F⁻¹‖∞`.

`screen_to_plane(object,view,zoom_log2,W,H,aspect)` returns `PoseMap::Mapped(Homography { rows:M, inverse:F, condition_number, apron_scale:1.0 })` or `PoseMap::EdgeOn` for the physical singularity. For `r=M·(x,y,1)`, `(o_u,o_v)=(r_x/r_w,r_y/r_w)` when `r_w>0`; the two preset pairs at `t=0` take canonical identity paths and pack exactly `[1,0,0,0]`, `[0,1,0,0]`, `[0,0,1,0]`. A control edit that leaves `F` and the slice unchanged to `1e−12`, including `d₅` at `h=0` in the inert fixture, does not restart refinement.

The scalar `Homography.apron_scale` is presentation-and-sampling metadata rather than a matrix coefficient. Every ordinary main map returned by `screen_to_plane` has the exact binary64 value one; app forms a backdrop map by copying that same map and replacing only the scalar with the unclamped footprint request, leaving `rows`, `inverse`, `condition_number`, centre and projection aspect bit-identical.

For a backdrop scale `a`, kernels alone use `zoom_log2−log₂a`, hence `pixel_scale_backdrop=a·pixel_scale_main`, while present alone uses `chart_scale_backdrop=a·4/W`; applying both to the same unmodified map makes a backdrop record and vertex name the same wider plane point. The `a=1` branch returns the original zoom bits without evaluating a logarithm, so the main grid and every height-zero deterministic request retain exact identity.

`scene_footprint` mirrors the scene vertex chain in binary64 over the bounded record-height domain `[-2,2]`. For positive relief it publishes the conservative floor contraction `boundary_scale=d₅/(d₅+2·height_scale)`, requested backdrop span `apron_scale=1/boundary_scale`, the post-backdrop uncovered lattice fraction, and a fixed 9-by-9-by-5 record-domain clipping census; at zero height it returns the exact constant `{boundary_scale:1,apron_scale:1,uncovered_fraction:0,relief_clipped_fraction:0}`.

The uncovered fraction is measured by rasterizing the mirrored 65-by-65 sampling mesh at each of the five census heights, through the backdrop's apron, into a 65-by-65 lattice closed on `[-1,1]` so the frame corners and edges are tested points. A triangle covers nothing unless all three of its vertices project and at least one is not held at the near clamp, which is the scene shader's own primitive rule; a mesh that falls apart therefore reports more sky and can never improve the answer by failing. The clipping census counts all 405 sampled points in its denominator, projectable or not, so a pose that projects almost nothing cannot publish a small clipped share.

The five-dimensional mirror applies the same near rule as the scene shader after camera rotation and translation: set `near₅=0.05d₅`, clamp the fifth coordinate to at most `d₅−near₅`, and only then divide. Thus every lifted point has denominator at least `0.05d₅`, magnification at most twenty, and a closed surface; the four-dimensional perspective retains its separate epsilon refusal. The clamp is paired with a primitive rule, in the mirror as in the shader: a triangle whose three vertices are all held at the limit has no honest vertex left and is dropped, while one with at least one unclamped vertex is kept so the surface still closes. Without the pairing the bound of twenty only relocates the invented geometry, drawing across the frame what the unclamped chain had left as sky. The constant is a deliberately small projection safeguard, while a later limit-model study owns a true model-space clipping distance.

HOT state carries finite `zoom_log2:f64`; the mathematical pixel scale is `p=4/(2^zoom_log2·W)`, displayed decimal depth is `zoom_digits=zoom_log2·log10 2`, and request label `depth_digits=ceil(max(0,zoom_log2·log10 2))`; after validating and rounding the scaled representation, `pixel_scale` returns the canonical delivered CPU scale reconstructed as `f64(mantissa)·2^exponent` rather than recomputing a second exponential.

The precision floor is `D_floor=max(1,ceil(zoom_log2·log10 2+log10 W)+8)` decimal digits, where the lower clamp only gives extreme zoom-out a valid bignum precision; the eight guard digits protect coordinate formation but cannot bound accumulated chaotic orbit roundoff, so it is explicitly a floor rather than a sufficiency claim.

The first working request is `D_work=D_floor+ceil(log10(max(max_iter,1)))`, requested bits are `ceil(D_work·log₂10)`, Astro-float rounds upward to its 64-bit word boundary, and the overlay reports floor digits, working digits, requested bits, and delivered bits separately.

Centre precision is a separate picture-space plan. For `PictureFast`, one quarter pixel is `2^-(zoom_log2+log₂W)`, so a centre rounded at `b` bits and allowed a conservative one last-place unit of accumulated error per edit needs `E·2^-b≤2^-(zoom_log2+log₂W+2)` for edit budget `E`; therefore `b=ceil(zoom_log2+log₂W)+ceil(log₂(4E))`, clamped to 64 and rounded upward to Astro-float's 64-bit unit. The shipped edit budget is 10,000, hence the guard is `ceil(log₂40,000)=16` bits. Width may grow as zoom or the consumed edit count grows but never shrinks within a navigation session, because widening cannot recover bits already rounded away. `Deterministic` retains 1,024 bits.

`PrecisionMode::Deterministic` retains the paired `D_work` and `D_work+16` computation for every pass. In `PrecisionMode::PictureFast`, `ReferencePass::Preview` publishes the single `D_work` orbit immediately with `ReferenceVerification::Deferred`, while Final and Measure compute the pair, require the same escape index and both GPU-consumed coordinate words within two `f32` ulps, publish the maximum observed word error, and raise `D_work` by 16 on failure until a stable Final is re-issued or the displayed 300-digit POLICY returns `PrecisionExhausted`; escalation count and failure are never silent.

The authoritative centre `C∈ℝ⁴` is four Astro-float-backed `BigScalar` values in the worker; the owner mirror rounds each coordinate directly to nearest `f64` with ties to even, rejects a non-finite result, and is only navigation, display, pose, and shallow-path evidence, never deep arithmetic authority.

For a finite exact scalar `x`, `split_scalar(x)` computes `hi=round_f32_ties_even(x)` and then `lo=round_f32_ties_even(x−exact(hi))` at the source precision; `CentreSplit` applies this independently in axis order and never narrows through the `f64` mirror.

Below the deep switch, the shallow kernel receives that split and `round_f32_ties_even(p)`; at `zoom_log2=14`, `p=2⁻²³` for `W=2048` and `p=2⁻²⁴` for `W=4096`, equal to or below one `f32` step near unit-magnitude coordinates, so direct absolute iteration can alias adjacent samples.

The perturbation kernel is therefore selected when `zoom_log2≥14`, a displayed POLICY rather than a device wall; below 14 the shallow kernel runs from the accepted current centre without a reference orbit. At or above 14 a current reference is mandatory, so a shallow-to-deep crossing waits for its newly requested orbit before perturbation can start.

The reference recompute trigger is a worker POLICY with hysteresis: it trips when the centre moves more than one quarter of the view extent or `|zoom_log2−reference_zoom_log2|>2`, remains disarmed while work is in flight, and rearms only inside the worker's accepted inner thresholds.

For the outer centre threshold, view extent is `4/2^zoom_log2`, so the norm condition is `|C−C_ref|₂>2⁻zoom_log2`; math evaluates the bignum difference before any mirror conversion.

### 2.4 Scaled perturbation, rebasing, and glitch

For reference centre `C=(C_z,C_c)`, the worker evaluates `Z₀=C_z` and `Zₙ₊₁=Zₙ²+C_c` at delivered precision, stores record zero for `Z₀`, and stores `length=min(max_iter,escape_index+1)` on escape or exactly `max_iter` otherwise, with valid indices `0..max_iter−1`.

The deep path never receives an absolute origin; its per-pixel offset is `o=p(xu+yv)`, with `δz₀=(o₁,o₂)` and `δc=(o₃,o₄)`, so Mandelbrot has `δz₀=0` and Julia has `δc=0` while a hybrid uses both.

To avoid ever forming tiny `p` in `f32`, let `q=2−zoom_log2−log₂W`, choose checked integer `s=floor(q)+1`, and choose `m=2^(q−s)`, so `m∈[0.5,1)` and `p=m·2^s`; round `m` once to ties-even `f32`, and if that rounding produces `1.0`, publish `m=0.5` and increment `s`.

The perturbation uniform carries `pixel_scale=m` and `scale_exponent=s`; each pixel forms `o′=(xu+yv)m`, begins with exponent `e=s`, `δ′₀=δz₀′=(o′₁,o′₂)`, and `δc′=(o′₃,o′₄)=δc/2^e`, so the represented actual delta is `δ=2^eδ′` without an absolute small `f32` scale.

At global iteration `n` and reference index `r`, the stored reference is exactly `Zᵣ=re+i·im` with one binary32 word per coordinate; form the actual delta with exponent-aware `ldexp`, set `zₙ=Zᵣ+2^eδ′ₙ`, and test escape before any rebase or advance. The ordinary advance uses the original single `complex_mul(Zᵣ,δ′ₙ)` operation sequence in both precision modes.

An ordinary advance is `δ′ₙ₊₁=2Zᵣδ′ₙ+2^e(δ′ₙ)²+δc′`, followed by `r←r+1`; the f64 mirror uses the same operation sequence, `ldexp` points, exponent changes, escape order, and reference-index order.

For the propagated per-pixel envelope, let `Dₙ` bound the represented perturbation error, `Rᵣ` be the half-ulp reconstruction bound of the stored binary32 reference, `C` bound the current scale-split centre offset, and `ηₙ` bound all binary32 products, sums, scaling and subnormal loss in the advance; the implemented recurrence is `Dₙ₊₁ ≤ 2(|Zᵣ|+|δₙ|)Dₙ + Dₙ² + 2Rᵣ(|δₙ|+Dₙ) + C + ηₙ`, display error adds `Rᵣ`, the represented-delta error and binary32 addition allowance, every renormalization adds a scaled minimum-subnormal allowance, and every rebase replaces the carried term by `D_rebase ≤ Eₙ + R₀ + η_rebase` before the next ordinary advance rather than resetting it.

The `gamma(20)` factor in `ηₙ` follows the contracted binary32 advance term by term: `2Zᵣδ′` costs four component multiplications plus two additions for the complex product and two multiplications for doubling, or 8; `2^e(δ′)²` costs four multiplications plus two additions for the complex product and two component scale multiplications, another 8; adding those two complex terms and then `δc′` costs four additions, for `8+8+4=20` rounded operations, while integer exponent construction is exact and subnormal loss is charged separately.

REBASING is repeatable and occurs after the current escape test but before advancing when `|zₙ|<|2^eδ′ₙ|`; if the scaled delta underflows to zero the predicate is false, equality is false, and the comparison uses a robust norm that does not square into overflow or underflow.

On rebase, set the actual delta to `zₙ−Z₀`, represent it at the current exponent as `δ′←(zₙ−Z₀)/2^e`, set `r←0`, increment `rebase_count`, and perform exactly one ordinary advance against `Z₀` to global iteration `n+1` and reference index one; thus `zₙ=Zᵣ+2^eδ′ₙ` holds before and after a nonzero-`Z₀` restart.

After an advance or rebase conversion, if nonzero `|δ′|>2⁶⁴`, multiply both `δ′` and the stored `δc′` by `2⁻⁶⁴` and set `e←e+64`; if `0<|δ′|<2⁻⁶⁴`, multiply both by `2⁶⁴` and set `e←e−64`; repeat until the inclusive interval is restored, leaving exact boundary values unchanged.

Rescaling `δc′` with `δ′` is required by `δc′=δc/2^e`; each power-of-two adjustment preserves the actual values `2^eδ′` and `2^eδc′`, and exponent overflow is a typed glitch rather than wraparound.

GLITCH occurs when `r=length` before the sample has escaped or reached `max_iter`: set `glitch=1`, stop, preserve `escaped=0` and `smooth_iter=−1.0`, and use the honest debug tint; re-rendering with a second reference is an explicitly displayed v1 limit.

The count is incremented only when a rebase is performed; when the current `rebase_count` is exactly `2²⁴`, another rebase request glitches rather than incrementing, so the maximum written value is the exactly representable `2²⁴`.

### 2.5 Centre displacement, navigation, and warp math

App's `anchor_px_up` produces a centred render-grid point with positive y up, while `drag_delta_px_down` preserves DOM-positive-down displacement until math changes it to `(dx,−dy)`. `navigation_delta(M,drag,Δq,anchor)` always receives the main presented-camera map, never the backdrop map: it maps the four-dimensional crosshair anchor through `M` and maps a drag as the difference of its two endpoints, then returns the worker-facing `NavigationDelta`, whose two pixel vectors mean local plane-offset pixels rather than an assumed affine screen vector.

The caller supplies `q_after=q_before+Δq`; because `M` is expressed in pixel-scale units and is zoom-invariant, `BigCentre::apply_navigation` retains the atomic formula `ΔC=(s₀−s₁)B a−s₁B p` for `s₀=pixel_scale(q_before)`, `s₁=pixel_scale(q_after)`, basis `B=[u v]`, mapped anchor `a`, and mapped drag `p`. A click with `Δq=0` only selects the later zoom anchor, a scale or box zoom preserves the plane point under that anchor, and a plain drag applies the mapped point difference; invalid input, horizon crossing, or finite-mirror overflow rejects the whole edit without partial mutation.

For a plane basis `B=[u v]`, scale `p`, desired centre `C`, and accepted reference centre `C_ref`, worker-side bignum arithmetic publishes `centre_from_reference_px=d=[u·(C−C_ref)/p,v·(C−C_ref)/p]`; subtraction and division occur before rounding the two results to nearest finite `f64`, which remains safe because the recompute policy bounds the ratio rather than the absolute depth.

On an accepted replacement, `reference_shift_px` is the new reference centre minus the old reference centre projected onto the current `(u,v)` basis and divided by current `p`; an initial reference publishes zero because no retained scene exists, while later values are measured bignum differences rather than mirror subtraction.

If a retained pose `f` must be expressed against the new reference using a shift `s_t` measured in current-pose pixels, convert it as `s_f=p_f⁻¹B_fᵀB_t p_t s_t` and set `d_f←d_f−s_f`; current displacement similarly becomes `d_t←d_t−s_t`, so a generation change with a valid shift does not clear the retained image.

For pose `p`, the accepted `M_p` converts a centred screen pixel to local plane-offset pixels; `pixel_scale`, `B_p`, the pose's `plane_origin`, and `d_p=centre_from_reference_px` then give its common-reference ambient point without materializing an absolute deep GPU coordinate.

For retained source pose `f` and current target pose `t`, the middle chart map `T` applies the relative centre, pixel-scale ratio, basis projection, and in-plane origin translation. The exact neutral-height forward screen homography is `H(f→t)=M_t⁻¹·T·M_f`; the upload used for inverse texture sampling is its explicit `H(t→f)` inverse, and the scale ratio is evaluated directly from zoom and widths without forming either deep scale.

The retained and requested object bases must span the same plane to the once-rounded `8·f32::EPSILON` floor; their angle parameters need not match. The exact orthogonal chart relation is composed into `T`. An origin delta is compatible exactly when its component outside the source plane is at most half a source pixel: an in-plane origin move becomes exact pan, while an out-of-plane move changes the slice and is refused. The normalized `chart_residual` is likewise refused above half a source pixel.

`warp_matrix(from,to)` requires two mapped poses and returns both row-major directions, rejects non-finite arithmetic and the shared `1e−12` pivot floor, and preserves horizon signs. Present re-expresses each retained or pending pose from the pose at which that scene was sampled, never from the newest HOT epoch, then binds the solved plan to that source scene and texture identity.

### 2.6 Interpolating one view into another

A view is a row of numbers, so a path between two views is a path in that row and nothing more. App composes `lerp_object_angles`, `lerp_origin`, `lerp_view`, and `lerp_f64` so all six object angles, four object-origin coordinates, twenty `ViewControls` scalars, and `zoom_log2` interpolate linearly. Angles do not shortest-arc rewrap: the sliders show `[−π,π]`, and a rewrap would move opposite to the visible handle.

`zoom_log2` linear on the exponent makes the zoom morph geometric, which is the only reading of "half way between" that is scale-free: the midpoint between `zoom_log2` 10 and 30 is 20, a factor of 2¹⁰ from each end, rather than an arithmetic midpoint that would spend almost the whole slider inside the deeper view.

The centre is a bignum interpolation, `C(t)=C_a+t(C_b−C_a)`, evaluated at `max(bits_a,bits_b)+64`: the higher of the two stored precisions, plus the bits the morph itself needs so that consecutive steps of the slider are distinguishable rather than rounding to the same centre at depth. The result is rounded back to that working precision once, at the end, and a non-finite intermediate rejects the whole interpolation rather than returning a partly moved row.

The oracle is exactness at the ends and the declared midpoint between them: `t=0` reproduces `a` and `t=1` reproduces `b` in every field including the centre's full precision, and `t=0.5` is the midpoint of every scalar to `1e-15` relative and of the centre to its working precision. `t` outside `[0,1]` is rejected rather than extrapolated, because a control that reads `A ↔ B` is not a promise about what lies beyond either end.

## 3. INTERFACES

All transferred and GPU words are little-endian, `f32` and `f64` are IEEE-754 binary32 and binary64, byte offsets start at the named record, coordinate arrays use `(z.re,z.im,c.re,c.im)`, reserved words are zero, orbit-pool bytes outside the declared record prefix and fixed fact tail are producer-owned and unread, and CPU-only records marked “no byte ABI” are not serialized by native layout.

### 3.1 Math-owned types and functions

|Interface|Exact contract|Consumer|
|---------|--------------|--------|
|`Axis4`|`#[repr(u32)] { E1=0,E2=1,E3=2,E4=3 }`|worker, app|
|`PrecisionMode`|`#[repr(u32)] { Deterministic=0,PictureFast=1 }`; one shared policy switch, with stable string spellings and cfg-free bit-identity predicate|worker, kernels, present, app|
|`PlanePreset`|`Mandelbrot` or `Julia { c0:[f64;2] }`; `c0` is finite and lives in MAIN's plane origin|worker, app|
|`PlaneSpec`|`{ axis_a:Axis4, axis_b:Axis4, plane_origin:[f64;4] }`; CPU-only, distinct seed axes|worker, app|
|`ObjectAngles`|`{rho_12,rho_13,rho_14,rho_23,rho_24,rho_34}`; six finite radians in object product order|kernels, present, app|
|`PlaneAngles`|Legacy `{theta_1,theta_2}` adapter to object `rho_13,rho_24`|worker, app|
|`NavigationDelta`|`{ pan_canvas_px:[f64;2], zoom_delta_log2:f64, anchor_canvas_px:[f64;2] }`; CPU-only, canvas-centred pixels with positive y upward|worker, app|
|`CentreF64`|`{ coords:[f64;4] }`; 32 native bytes, finite owner mirror without deep authority|worker, owner, present|
|`CentreSplit`|`#[repr(C,align(16))] { hi:[f32;4], lo:[f32;4] }`; 32 bytes at offsets 0 and 16|kernels|
|`Plane`|`#[repr(C,align(16))] { basis_u:[f32;4], basis_v:[f32;4] }`; 32 bytes at offsets 0 and 16, dimensionless|kernels, present, app|
|`EscapeParams`|`#[repr(C)] { max_iter:u32, bailout:f32 }`; 8 bytes at offsets 0 and 4, `max_iter>0`, squared `bailout=256.0`|kernels, worker, app|
|`ScaledPixelScale`|`{ mantissa:f32, exponent:i32 }`; CPU-only, `p=mantissa·2^exponent`, mantissa in `[0.5,1)`; `ScaleSplit` is a compatibility alias|kernels, overlay|
|`PrecisionPlan`|`{ floor_digits:u32, working_digits:u32, requested_bits:u32, policy_digits:u32 }`; decimal digits except bits|worker, overlay|
|`ReferencePass`|`#[repr(u32)] { Preview=0, Final=1, Measure=2 }`; selects deferred or immediate verification in PictureFast|worker, app|
|`ReferenceVerification`|`#[repr(u32)] { Deferred=0, Stable=1 }`; published orbit-verification state|worker, app|
|`BigCentre`|`{ coords:[BigScalar;4], precision_bits:u32 }`; Astro-float-backed, finite, no byte ABI|worker|
|`EscapeSample`|`{ smooth_iter:f32, escaped:bool, escape_index:Option<u32> }`; CPU-only oracle output|kernels tests|
|`PerturbSample`|`{ smooth_iter:f32, escaped:bool, escape_index:Option<u32>, rebase_count:u32, glitch:bool }`; CPU-only oracle output|kernels tests|
|`PerturbationEnvelope`|`{ delta_abs_error:f64, escape_norm2_error:f64, smooth_error:f64, minimum_escape_margin:f64 }`; CPU-only propagated-error evidence|kernels tests|
|`ReferenceOrbitRecord`|`#[repr(C)] { re:f32, im:f32 }`; 8 bytes|worker, kernels|
|`ComputedOrbit`|`{ records:Vec<ReferenceOrbitRecord>, length:u32, precision_bits:u32, escape_index:Option<u32>, verification:ReferenceVerification, max_consumed_word_error_ulps:Option<u32>, precision_escalations:u32 }`; reusable linear-memory records plus verification facts|worker|
|`OrbitStep`|`Pending { stored:u32 }` or `Complete(ComputedOrbit)`; CPU-only cooperative result|worker|
|`Pose`|CPU-only exact field list below, no byte ABI|present, app|
|`ViewControls`|`{camera:[f64;10],camera_translation:[f64;5],camera_yaw,camera_pitch,height_scale,distance_five,distance_four}`; twenty f64 scalars|present, app|
|`PoseMap`|`Mapped(Homography)` or `EdgeOn`|present, app|
|`Homography`|`{rows:[f64;9],inverse:[f64;9],condition_number:f64,apron_scale:f64}`; normalized screen-to-plane map, forward inverse, and layer-local span|kernels, present, app|
|`SceneFootprint`|`{boundary_scale:f64,apron_scale:f64,uncovered_fraction:f64,relief_clipped_fraction:f64}`; requested backdrop and post-backdrop pose facts|present, app|
|`WarpMatrix`|`{ forward:[f64;9], inverse:[f64;9] }`; row-major, 144 native bytes|present|
|`MathError`|`NonFinite`, `InvalidExtent`, `InvalidMaxIter`, `InvalidBailout`, `InvalidViewControls`, `PlaneRoundingBound`, `InvalidCentreEncoding`, `PrecisionMismatch`, `ScaleExponentOverflow`, `DegenerateWarp`, `OrbitTooLong`, `InvalidOrbitState`, `EmptyReferenceOrbit`, `InvalidPrecisionPlan`, `CounterOverflow`, `DurationOverflow`, `BigFloat`, or `PrecisionExhausted { requested_digits,policy_digits }`|all slices|

`Pose` is `pub struct Pose { pub epoch:u64, pub orbit_generation:u32, pub plane:Plane, pub object:ObjectAngles, pub plane_origin:[f64;4], pub zoom_log2:f64, pub view:ViewControls, pub grid_width:u32, pub grid_height:u32, pub map:PoseMap, pub centre_from_reference_px:[f64;2] }`; it carries the exact object and camera pose, the origin at which the slice was sampled, and that level's accepted map.

The plane, map, and footprint signatures are `construct_plane(angles:ObjectAngles)->Result<Plane,MathError>`, `construct_plane_from_spec(spec:PlaneSpec,angles:ObjectAngles)->Result<Plane,MathError>`, `screen_to_plane(object:&ObjectAngles,view:&ViewControls,zoom_log2:f64,grid_w:u32,grid_h:u32,aspect:f64)->Result<PoseMap,MathError>`, and `scene_footprint(object:&ObjectAngles,view:&ViewControls,grid_w:u32,grid_h:u32)->Result<SceneFootprint,MathError>`. The split, scale, precision, escape, centre-displacement, and reference-shift functions retain their typed checked results and never infer camera state.

Orbit and perturbation signatures are `ReferenceOrbitBuilder::new(centre:&BigCentre,plan:PrecisionPlan,params:EscapeParams)->Result<ReferenceOrbitBuilder,MathError>`, policy-aware `ReferenceOrbitBuilder::new_with_policy(centre:&BigCentre,plan:PrecisionPlan,params:EscapeParams,mode:PrecisionMode,pass:ReferencePass)->Result<ReferenceOrbitBuilder,MathError>`, `ReferenceOrbitBuilder::step(&mut self,max_entries:NonZeroU32)->Result<OrbitStep,MathError>`, `perturb_scaled_f64(orbit:&[ReferenceOrbitRecord],offset_prime:[f64;4],scale_exponent:i32,params:EscapeParams)->Result<PerturbSample,MathError>`, and `perturb_scaled_f64_with_envelope` with the same inputs returning `Result<(PerturbSample,PerturbationEnvelope),MathError>`.

Interpolation signatures include `lerp_f64`, `lerp_view`, `lerp_object_angles`, the legacy `lerp_plane_angles`, `lerp_origin`, `lerp_centre`, and `morph_precision_bits`; each rejects non-finite input or `t` outside `[0,1]`, and app composes them rather than repeating arithmetic.

Navigation and warp signatures include `navigation_delta(screen_to_plane:&Homography,drag_delta_px_down:[f64;2],zoom_delta_log2:f64,anchor_px_up:[f64;2])->Result<NavigationDelta,MathError>`, `BigCentre::apply_navigation`, `BigCentre::displacement_px`, `centre_from_reference_px`, `reference_shift_px`, `warp_matrix(from:&Pose,to:&Pose)->Result<WarpMatrix,MathError>`, and `warp_identity_error`; `WarpMatrix.forward` is source-to-target and `.inverse` is inverse sampling.

The worker document's unresolved authoritative-navigation API is resolved by adopting `NavigationDelta` and these three `BigCentre` methods by reference; worker retains the centre and sequencing, while math exclusively owns the mutation and projection algebra.

`ReferenceOrbitBuilder` owns partial Astro-float state and emits at most `max_entries` records per call; worker chooses the chunk, checks generation, credit, and deadline, and yields, so high-precision arithmetic cannot turn latest-wins into an unbounded wait.

### 3.2 GPU records and uniform blocks

|Record|Bytes and exact fields|Producer → consumer|
|------|----------------------|-------------------|
|Reference orbit transfer / RGBA32F heap texel|8 transferred bytes: 0 `re:f32`, 4 `im:f32`; app expands each point to 16 GPU bytes `(re,im,0,0)`; texel zero is `Z₀`|worker → app → kernels|
|Escape grid RGBA32F|16 bytes: 0 `smooth_iter:f32`, 4 `escaped:f32`, 8 `rebase_count:f32`, 12 `glitch:f32`; flags are 0 or 1 and count is integer-valued|kernels → present|
|`ShallowUniform`|144 bytes: basis at 0/16, `M` rows at 32/48/64, centre at 80/96, scalar tail at 112|app/math → kernels|
|`PerturbUniform`|112 bytes: basis at 0/16, `M` rows at 32/48/64, scaled scalar tail at 80|app/math → kernels|
|`HotUniform`|288 bytes: ten camera pairs at 0–64, translation at 80/96, observer at 112, scale at 128, warp rows at 144–176, current map at 192–224, exterior at 240, clear at 256, flags at 272|present → GPU|
|`SceneUniform`|160 bytes: grid/span at 0/16, basis at 32/48, sampled map at 64–96, palette/interior/clear at 112/128/144|present → GPU|

Each homography row has three coefficients plus zero padding, and `flags=[epoch_low,epoch_high,source_valid,edge_on]`; the shader receives the current screen map but never two semantic poses.

The hot ring has exactly three slots, `hot_stride=align_up(288,min_uniform_buffer_offset_alignment)`, total bytes are `3·hot_stride`, present owns its buffer and bind group, one refresh writes exactly 288 bytes to one slot, and selection is by dynamic offset.

`SceneUniform.grid=[width,height,level,max_iter]`, `span=[directory_index,logical_len,edge_on,0]`, and present updates it only on changed scene inputs.

`RefinementLevel` is `#[repr(u32)] { Preview=0,Interactive=1,Final=2 }`; an unknown discriminant is a typed error.

`EscapeGrid` is the kernels-owned CPU wrapper `{ span:DataSpan, width:u32, height:u32, level:RefinementLevel }`; its initialized dense prefix has `width·height` records, `span.logical_len` is Final capacity for reuse, present never samples the inactive suffix, and only kernels free the span.

Preview is `ceil(W/4)×ceil(H/4)` at `min(requested_cap,64)`, Interactive is `ceil(W/2)×ceil(H/2)` at `min(requested_cap,256)`, and Final is `W×H` at `min(requested_cap,4096)`; 4,096 and the shallow/deep switch are displayed policies, power-of-two extent degradation is a delivered fact, kernels define levels, and app schedules them in order with permission to skip.

All kernel output lands in DATA only through the paid SCRATCH-copy path, reference arrival is a regional DATA write, heap bind-group identities never change, and normal rendering cannot aggregate rebase or glitch totals without a separately requested and labelled measurement readback.

### 3.3 Worker wire protocol and centre adapter

Every standalone message buffer begins with `MessageHeader`, eight little-endian `u32` words and 32 bytes: 0 `magic`, 4 `version`, 8 `generation`, 12 `kind`, 16 `length`, 20 `precision_bits`, 24 `compute_us`, and 28 `credit_us`.

`magic=0x314c424a` is byte string `JBL1`, wire `version=3`, `JULIBROT_ABI_VERSION=3`, and loader URLs independently remain pinned to `?v=1`; any module/wire version skew is a typed refusal.

|Kind|Name|Direction and `length`|
|---:|----|----------------------|
|1|`OrbitRequest`|main → worker; requested `max_iter`|
|2|`RequestReturn`|worker → main; zero|
|3|`OrbitResponse`|worker → main; stored orbit records|
|4|`CreditApplied`|main → worker; zero, installed generation|
|5|`CreditStale`|main → worker; zero, discarded generation|
|6|`OrbitCancelled`|worker → main; zero, measured stale work is charged|
|7|`ChannelError`|either direction; four-word `ErrorRecord`|
|8|`Shutdown`|main → worker; zero|
|9|`ShutdownAck`|worker → main; zero|

The last 16 bytes are `PoolTrailer { pool:u32,slot:u32,capacity_bytes:u32,trailer_magic:u32 }`, `trailer_magic=0x544c424a`, request pool is 1, orbit pool is 2, and `slot∈{0,1}`; it is initialized once and round-trips bit-exactly.

For current `max_iter=M`, each of the four buffers has capacity `max(644,64+8M)`; the 644-byte floor fits the maximum 300-digit request at cap 64, two buffers circulate independently in each direction, resizing all four occurs only when `max_iter` changes after ownership reconciliation, and each resize is a reported allocation event.

`OrbitRequest` is `{ generation:u32, centre:EncodedCentre, depth_digits:u32, precision_bits:u32, max_iter:u32, precision_mode:PrecisionMode, reason:OrbitReason, reference_pass:ReferencePass }`; header fields carry generation, precision, and cap, while the body at byte 32 is `{ depth_digits:u32,reason_bits_and_pass:u32,centre_revision:u32,limb_word_count:u32,coordinates:[CoordinateDescriptor;4],precision_mode:u32,limbs:[u32;limb_word_count] }`.

Coordinate descriptors start at bytes 48, 64, 80, and 96, the precision-mode word is at byte 112, and limbs start at byte 116; request fit requires `116+4·limb_word_count≤capacity−16`, otherwise worker returns the displayed `CentreEncodingWall` without truncation or hidden allocation.

`CoordinateDescriptor` is exactly 16 bytes `{ sign:u32,exponent_twos_complement:u32,limb_start:u32,limb_count:u32 }`; a nonzero value is `(−1)^sign·(Σ limbs[limb_start+k]·2^(32k))·2^exponent`, limbs are least-significant first, `sign∈{0,1}`, and the high stored limb is nonzero.

Descriptor ranges are ordered, contiguous, non-overlapping, and cover `limb_word_count`; canonical zero is `{sign:0,exponent:0,limb_start:previous_end,limb_count:0}`, with no negative zero, leading high zero, unused limb, or out-of-range descriptor.

`reason_bits_and_pass` assigns bit 0 to initial reference, bit 1 to centre-threshold crossing, bit 2 to zoom-threshold crossing, bit 3 to max-iteration change, bit 4 to precision-mode change, and bits 5–6 to `ReferencePass::{Preview,Final,Measure}` only for `PictureFast`; `PrecisionMode` itself is read exclusively from byte 112, deterministic requests require zero pass bits and decode as Final, and any unknown or contradictory bit pattern is a version-three `BadLength` refusal.

Math's `encode_big_scalar` and `decode_big_scalar` adapters map Astro-float values to exactly that dyadic representation, use the `u32` bit pattern of the two's-complement `i32` exponent, preserve exact value at delivered precision, and impose no extra odd-low-limb rule; worker alone validates and transports bytes.

`decode_big_scalar` delivers the requested `precision_bits` rounded up to 64 and nothing else, whatever width the record happens to carry: a record within that precision decodes exactly, a wider one rounds to nearest with ties to even, and the low zero bits of a non-minimal record never reach the bignum. Delivering the record's own width instead would make the delivered precision a property of the individual coordinate, and the four coordinates of one centre do not hold mantissas of equal significant width — the authoritative navigator runs at 1,024 bits while a request declares only the depth's plan bits, so a coordinate that the last anchored zoom moved carries a product of two 53-bit mantissas and one the plane basis left alone does not. `EncodedCentre::decode_math` therefore states an invariant rather than gating on it, and the narrowing discards nothing that would have survived: `ReferenceOrbitBuilder` restates the centre at `bits_for_digits(D_work)`, which is never above the declared bits.

Precision rounds to 64 rather than to the machine word because Astro-float's word is 64 bits on a 64-bit target and 32 bits on wasm32; 64 is a multiple of both, so every `BigScalar` reports the precision Astro-float actually allocated in the native gate and in the browser alike. Passing an unrounded request straight through would run a 90-bit operation at 128 bits natively and at 96 bits in the browser while both claimed 128.

`ErrorRecord` begins at byte 32 and is `{ code:u32,detail:u32,requested_bytes:u32,available_bytes:u32 }`; stable codes are `1 BadMagic`, `2 BadVersion`, `3 BadKind`, `4 BadLength`, `5 BadTrailer`, `6 CentreEncodingWall`, `7 GenerationExhausted`, `8 EpochExhausted`, `9 TimingOverflow`, `10 BufferStarved`, and `11 MathFailure`.

`OrbitResponse` is the header followed at byte 32 by `length` 8-byte reference records and a fixed 16-byte fact tail immediately before the pool trailer; the fact tail is `{ verification:u32,max_consumed_word_error_ulps:u32,precision_escalations:u32,reserved:u32 }`, `u32::MAX` denotes a deferred maximum, `1≤length≤max_iter`, unused capacity between records and facts remains producer-owned and is never read, and `compute_ms=f64(compute_us)/1000` is only a display conversion.

The owner's credit POLICY is `250,000` microseconds per second and is displayed; the returned `CreditApplied` or `CreditStale` header preserves generation, precision, and compute time, sets length zero, and carries the measured remaining `credit_us` without fabrication.

One wasm module is instantiated on main and in the worker with exported `worker_main`; browser fetch caching avoids a second download, separate instance memory remains a reported cost, and the same-thread lowering uses the identical four ownership states, headers, generation checks, credit events, and buffer moves.

### 3.4 Worker owner state

The ABI-two owner records below are `Copy` and `#[repr(C)]`, both drains are infallible, each drain bumps one shared checked `u64` epoch, later staged values replace undrained values, and consumers never use epoch equality as an orbit or warp compatibility test.

`HotState` is 40 bytes, alignment 8: byte 0 `zoom_log2:f64`, byte 8 `plane_theta_1:f64`, byte 16 `plane_theta_2:f64`, and byte 24 `centre_from_reference_px:[f64;2]`.

`MainState` is 128 bytes, alignment 8: byte 0 `generation_applied:u32`, 4 `centre_revision:u32`, 8 `requested_iter_cap:u32`, 12 `delivered_iter_cap:u32`, 16 `precision_bits:u32`, 20 `orbit_length:u32`, 24 `palette_id:u32`, 28 `orbit_id:u32`, 32 `centre_f64:[f64;4]`, 64 `plane_axis_a:u32`, 68 `plane_axis_b:u32`, 72 `plane_origin_f64:[f64;4]`, 104 `reference_shift_px:[f64;2]`, and 120 `precision_mode:u32`, followed by four tail-padding bytes.

`ViewerState` is 176 bytes, alignment 8: byte 0 `epoch:u64`, byte 8 `hot:HotState`, and byte 48 `main:MainState`; `HotDrain` and `MainDrain` each return the full record.

`OrbitHandle` is `{ id:u32,generation:u32 }`, zero ID means no orbit, and app rejects a registry lookup whose generation differs; orbit generation is checked monotonic `u32` and wrap is impossible within a session because exhaustion ends new work.

`ViewerOwner::drain_hot()->HotDrain` runs each refresh and `ViewerOwner::drain_main()->MainDrain` runs on accepted orbit, cap, palette, or plane-origin arrival; both increment epoch even when the corresponding staged value is unchanged, as intentionally accepted in the review.

`ViewerOwner::accept_orbit` publishes the latest matching generation and `reference_shift_px`, while stale responses return credit without publication; a new accepted reference re-expresses retained poses, and cap, plane-origin, or precision-mode changes force present to clear.

### 3.5 Presentation-owned records and calls on math's boundary

Math defines the twenty-scalar `ViewControls`, `ObjectAngles`, `Homography`, `PoseMap`, and `Pose`, and present re-exports the view record; present defines `PaletteId` as `#[repr(u32)] { Classic=0,Ember=1,Ice=2 }`.

`PaletteRecord` is `#[repr(C,align(16))] { map:[f32;4],interior_rgba:[f32;4],clear_rgba:[f32;4] }`, 48 bytes; Classic is `{map:[64,0,0.78,1],interior:[0.005,0.005,0.008,1],clear:[0.015,0.018,0.025,1]}`, Ember is `{map:[48,0.02,0.88,1],interior:[0.01,0,0,1],clear:[0.015,0.008,0.005,1]}`, and Ice is `{map:[80,0.55,0.72,1],interior:[0,0.005,0.01,1],clear:[0.005,0.01,0.015,1]}`.

`PresentHot` carries the accepted epoch, plane, six object angles, origin, zoom, twenty view scalars, level map, and reference-relative centre; `PresentMain` carries the epoch, orbit generation, grid, cap, palette, origin, reference shift, and sampled map.

`HotSlot` is `{index:u32,dynamic_offset:u32,epoch:u64}`, where `index=refresh_id mod 3` and `dynamic_offset=index·hot_stride`; its checked constructor makes `Presenter::write_hot(slot,hot,validation)` infallible.

`PresentConfig` is `{ surface_format:wgpu::TextureFormat,min_uniform_buffer_offset_alignment:u32,fence_deadline_ms:f64,max_fence_polls:u32 }`; v1 passes the live alignment, `30_000.0`, and `4_096`, and the scene texture format is `Rgba8Unorm`.

`Presenter::new(device:Arc<wgpu::Device>,queue:Arc<wgpu::Queue>,heap:HeapPresentResources,config:PresentConfig)->Result<Presenter,PresentError>` allocates the three-slot ring, two empty texture slots, fixed pipelines, and immutable heap group only after both error handlers exist.

`Presenter::set_main(&mut self,main:PresentMain)` and `Presenter::write_hot(&mut self,slot:HotSlot,hot:PresentHot,validation:WarpValidation)` are the infallible MAIN and HOT endpoints; the latter computes the f64 plan, writes exactly 288 bytes, and lowers a refused plan to `source_valid=0`.

`Presenter::submit_scene(&mut self,hot_slot:HotSlot,now_ms:f64)->Result<u64,PresentError>` submits one scene plus its four-byte fence, while `Presenter::frame(&mut self,state:FrameState<'_>,hot_slot:HotSlot)->Result<FrameReceipt,PresentError>` submits the sole warp pass to the borrowed surface view and returns before app presents.

`FrameState<'a>` is `{ surface_view:&'a wgpu::TextureView,canvas_width:u32,canvas_height:u32,refresh_id:u64,now_ms:f64 }`; `FrameReceipt` is `{ refresh_id:u64,warp_id:u64,source_scene_id:Option<u64>,precision_mode:&'static str,status:PresentStatus }`, both are CPU-only, and receipt contains no wall before fence completion.

`Presenter::poll(&mut self,now_ms:f64)->Vec<PresentEvent>` observes each pending fence at most once per call and never waits, while `Presenter::facts(&self)->PresentFacts` is an immutable, non-polling snapshot.

`SceneFrame` stores the scene id, the pose at which it was sampled, palette, cap, level, extent, texture index, precision provenance, and measurement. `WarpPlan` stores its three packed rows, source validity, kind, chart residual, measured max and p95 error, and the exact source scene and texture identities it was solved against.

`Warp::reproject(last_frame:&SceneFrame,from_pose:&Pose,to_pose:&Pose,precision_mode:PrecisionMode,validation:WarpValidation)->WarpPlan` is a pure CPU planner; `last_frame.pose` must equal `from_pose`, the slice checks in §2.5 must pass, and every accepted plan carries a measured bound no greater than `WARP_MAX_ERROR_PX=1.0`. Deterministic and explicit Measure/Final validation use the full corpus; ordinary PictureFast still measures the smaller mandatory corpus needed to enforce the bound.

The refresh order is `poll → drain HOT → write_hot(refresh_id mod 3) → frame → app present`, with `submit_scene` only when app's schedule says a scene is due; present owns both scene textures and both four-byte fences, refuses `SceneBusy` rather than allocating a third texture, and app presents the surface outside all measured regions.

Scene targets use delivered grid extent and each per-level reallocation increments `texture_reallocations`; scene and warp walls are separate four-byte-fence measurements, every poll is counted, warm-up is labelled, timestamp queries are absent, and the second completed frame decides the displayed 100 ms policy.

The page facts contributed or constrained here are `{ requested_generation,accepted_generation,owner_epoch,zoom_log2,zoom_digits,depth_digits,precision_floor_digits,precision_working_digits,precision_requested_bits,precision_delivered_bits,orbit_length,max_iter_requested,max_iter_delivered,bailout,scale_mantissa,scale_exponent,centre_from_reference_px,reference_shift_px,refinement_level,grid_width,grid_height,rebase_total,glitch_total,centre_recompute_policy,worker_compute_us,credit_us,allocation_events,texture_reallocations }`; gather-only totals are `unavailable` unless a labelled measurement readback populated them.

### 3.6 Joint-review interface table

|Producer → consumer|Pinned interface|Exact payload or rule|
|-------------------|----------------|---------------------|
|math → worker/app|`PlaneSpec`, `ObjectAngles`, legacy `PlaneAngles`, `CentreF64`, centre adapter|ℝ⁴ object pose, worker-owned bignum, f64 mirror|
|math → kernels/present|`Plane`|32 bytes, two rounded f32 ℝ⁴ basis vectors|
|math → kernels|`CentreSplit`, `ScaleSplit`, `EscapeParams`|32 bytes; f32 mantissa plus i32 exponent; 8 bytes|
|worker → app → kernels|reference record|8-byte `[re,im]` transfer, expanded to RGBA32F `[re,im,0,0]` per heap index|
|kernels → present|escape record|RGBA32F `[smooth_iter,escaped,rebase_count,glitch]`, 16 bytes per pixel|
|kernels → present|`EscapeGrid`|typed `DataSpan`, active `width,height`, `RefinementLevel`|
|owner → app/present|`ViewerState`|176-byte repr(C), shared epoch, latest-wins HOT and MAIN|
|owner ↔ worker|wire protocol|32-byte `JBL1` header, nine kinds, 16-byte trailer, four buffers|
|math → present/app|`Pose`|CPU-only object origin, twenty-scalar view, map, and centre displacement|
|present → GPU|`HotUniform`|288-byte payload, three dynamic-offset slots|
|present → GPU|`SceneUniform`, palette|160-byte scene block, selected present-owned 48-byte palette|
|present → app|present API and facts|two textures, separate scene/warp fences, delivered facts only|

## 4. Inherited laws and satisfaction

WebGL2 through wgpu 24 `Backends::GL` is the sole substrate and the minimum-requirements format floor is mandatory; this CPU slice adds no feature, format, WebGPU, timestamp-query, or shared-memory requirement.

Per-frame CPU-to-GPU traffic is uniforms only plus regional writes for changed data: accepted orbit bytes update their DATA region, plane and scale enter dispatch uniforms, hot pose enters one ring slot, and unchanged bignum, orbit, grid, palette, descriptors, and bind groups do not move.

Kernel outputs use the heap's paid SCRATCH-to-DATA copy path, the executor's descriptor, directory, header, resource, uniform, and texture identities remain stable, and the hot ring uses a live dynamic offset.

No shared memory exists: four buffers transfer exclusive ownership with measured credit headers, and same-thread execution is the same protocol abstraction with direct moves.

Honesty is structural: requested and delivered resolution, iterations, precision, zoom digits, scale exponent, orbit length, reference generation, centre displacement, reference shift, rebase/glitch availability, allocation events, policies, hardware walls, warm-up, polls, and measured walls remain distinct; unavailable is never zero, and no wait lacks cancellation plus a deadline.

Before the first frame, app shows clear colour and honest overlay text with no diagnostic pattern; browser-only conformance and performance facts are labelled `requires visible replay`.

App installs the heap-provided panic hook and non-panicking uncaptured-error handler before the first device call, owns the single surface token, and presents outside the measured scene and warp regions.

Hand-written `f64` remains the CPU matrix implementation; both required native matrix oracles pass in the package and workspace gates, so the decision rule keeps `faer` absent, and it may enter later only if the identical f64 oracle fails, never because its API is convenient or because an f32 case fails.

One world, one heap class, no simulation tick, no general graph, no shared-memory worker, no WebGPU path, and no second-reference repair remain deliberate prototype boundaries.

## 5. Oracles and tests

Native escape tests pin fixed points, escaping points, exact bailout equality, `max_iter` edges, state-index order, natural-log expansion of smooth iteration, non-finite rejection, exact `−1.0` for capped non-escape, and a bit-exact bailout fixture where unfused WGSL order escapes at index zero while fused arithmetic would incorrectly remain capped.

Native plane tests pin the six-factor `O` order, exact legacy `R₁₃R₂₄` equivalence, one f32 rounding pass, the `8·f32::EPSILON` postcondition, and random-angle `O` orthonormality to `1e−12`; camera tests pin the ten-factor `Q` order and random-angle orthonormality to the same bound.

Navigation drift composes the five-by-five VIEW step `R₁₂(Δθ)·R₃₅(φΔθ)` for `10⁴` and `10⁵` steps with `Δθ=10⁻³` radians, measures `‖MᵀM−I‖_F=sqrt(Σᵢⱼ(MᵀM−I)ᵢⱼ²)`, and passes at `≤10⁻⁵` in hand-written f64 without re-orthonormalization and in f32 with modified Gram–Schmidt every 64 steps.

The screen-map oracle requires `forward∘inverse` identity within `1e−9` on a 9-by-9 screen lattice across both exact presets, general object/camera angles, observer angles, distances, and nonzero five-dimensional translations. Preset tests require exact identity rows, the un-faced Mandelbrot plane at `Q=I` to refuse edge-on, and the 256-step Julia-to-Mandelbrot object morph at fixed `Q` to cross edge-on exactly once.

Warp accuracy covers exact pan, in-plane origin translation, and flat camera translation plus relief and mixed affine-camera fixtures; it requires the published plan bound and separately pins object-angle, out-of-plane origin, half-source-pixel residual, and determinant refusals.

If and only if an f64 case of either matrix oracle fails, implementation may add `faer`, rerun the identical corpus and metric, and record both values; the prior 3–13× tiny-matrix evidence otherwise keeps faer out.

Native scale tests cover integral and fractional zoom, widths 1, 64, 1,920, 2,048, and 4,096, the rounded-mantissa carry, exponents through the 300-digit policy range, exact pixel centres, and reconstruction against high-precision `p` without ever requiring a tiny `f32` scalar.

Native navigation tests require anchored zoom to preserve the anchor's fractal point within one binary64 ulp at `zoom_log2∈{0,40,100}`, require drag to equal exactly `−s(dx u+dy v)`, and round-trip centre/reference displacement through plane pixels.

The centre-width oracle runs the same 10,000 mixed navigation edits at the derived `PictureFast` width and at the 1,024-bit `Deterministic` width, requires the widened fast centre to remain within 0.25 current pixel of the deterministic centre, reports measured pixel drift and growth per edit, and reports both navigation walls. At `zoom_log2=100`, `W=1024`, and `E=10,000`, the formula requests 126 bits and Astro-float delivers 128; osprey measured 0.001557775202 pixel total drift, or 1.557775202×10⁻⁷ pixel/edit, leaving about 160.5 times the quarter-pixel allowance, while the test walls were 3,967.833 ms at 1,024 bits and 3,770.115 ms at 128 bits.

The scaled f64 perturbation oracle compares direct f64 iteration with the scaled recurrence at zoom values `{14,40,80,100,256,512,900}`, forces upward and downward 64-bit renormalizations, pins inclusive thresholds, adjusts `δc′` with every exponent change, and proves the represented actual delta is invariant across each rescale.

Native rebase tests cover zero, repeated, and nonzero-`Z₀` rebases; the nonzero fixture must pass direct-orbit equality after `δ′←(z−Z₀)/2^e`, index reset, and one ordinary advance, so the reviewed correction is a pass criterion rather than a known failure.

Native split tests cover signed zero, exact and halfway values, finite range edges, direct big-to-f32 residual rounding, reconstruction error, coordinate order, and typed refusal when either the f64 mirror or split component would be non-finite.

Native precision tests pin every rounding boundary, distinguish floor, working, requested, and delivered precision, exercise checked conversion and the `D+16` loop, and prove exhaustion reports the 300-digit policy rather than silently lowering a request.

Native orbit tests compare Astro-float with the same recurrence at `D+16`, pin record zero, escape truncation and `0..max_iter−1`, `re/im` channel order, codec round trips, malformed codec refusal, generation independence, and the request-pool `CentreEncodingWall`; app's minimum requestable cap of 64 must fit every canonical centre through 300 digits.

Shallow GPU conformance requires escape classification and integer escape index exactly equal to `escape_f32` at sampled pixels and smooth value within `10⁻⁴`; its readback and image evidence `requires visible replay`.

Perturbation GPU conformance uses the scaled f64 oracle, requires exact classification and integer index outside the propagated uncertainty envelope and smooth error at most `2×10⁻³`, retains explicit boundary fixtures inside the envelope, and `requires visible replay`.

The propagated envelope begins at actual rounded `δz₀′`, applies the contracted f32 operation sequence including reference reconstruction, `ldexp`, rebases, and exact power-of-two rescaling, and converts complex error `eₙ` to squared-radius uncertainty `2|zₙ|eₙ+eₙ²`; classification tolerance is arithmetic rather than a guessed pixel exclusion.

Native state tests interleave HOT and MAIN drains, accepted and stale generations, centre displacement, reference shifts, and same-thread responses; they require the 40/128/176-byte layouts, monotonic shared epochs, infallible drains, re-expressed retained poses, mode-change staleness, and no stale orbit publication.

Native test policy is cfg-free: tests iterate `PrecisionMode::ALL` where both policies are meaningful, and `requires_bit_identity` marks only exact CPU-mirror operation sequences, exact rebase counts, dyadic decode identity, Astro-float word-width identity, exact `D` versus `D+16` words, synthetic drift identity, palette words, and planner residual words as Deterministic/conformance assertions. Every semantic layout or wire check and every accuracy oracle against the Deterministic path remains unconditional.

Native wire tests pin all header, kind, trailer, request, descriptor, response, capacity, credit, and version bytes; browser ownership transfer, worker timing, fetch caching, and duplicated instance memory `requires visible replay`.

The Barza selection probe used one warmed release run of the bounded fixture `z₀=0`, `c=−0.5+0.5i`: Astro-float 0.9.6 measured 7.530 ms and 73.125 ms at 100 digits for `10⁴` and `10⁵` iterations and 19.516 ms and 229.887 ms at 300 digits, while Dashu 0.6.0 measured 19.852 ms, 204.679 ms, 26.034 ms, and 275.267 ms respectively.

Both candidates built for `wasm32-unknown-unknown` after Astro-float disabled default features; the initial default-feature build failed because its optional random dependency required a JavaScript getrandom feature, so the implementation pin is `astro-float = { version="=0.9.6", default-features=false }`.

## 6. Bignum decision and risks

|Candidate|wasm32 build|Barza slice-shaped speed|License and maintenance|Decision|
|---------|------------|------------------------|-----------------------|--------|
|Astro-float 0.9.6|PASS with default features off|Fastest at all four final points|MIT; current published release and active upstream|Selected|
|Dashu-float 0.6.0|PASS|Slower at all four final points|MIT OR Apache-2.0; current published release and active upstream|Measured fallback|
|Hand-rolled fixed point|Expected portable, not built|Not measured|Local proof and maintenance burden|Rejected for v1|

Astro-float is selected because it is pure Rust, exposes explicit precision and ties-to-even rounding, builds for wasm without default features, and won all four bounded final probe points; the measurements select a library and do not predict browser throughput.

A hand-rolled fixed-point scalar would make scaling explicit but would also make every product's rescale, rounding, exponent range, and correctness corpus local obligations, so it is not justified while a measured pure-Rust library satisfies the build and speed criteria.

|Risk|Oracle that retires it|
|----|----------------------|
|Two-f32 orbit records carry much less precision than a 100–300 digit worker orbit.|The `D` versus `D+16` comparison and deep scaled-classification corpus must pass; otherwise the record grows in a reviewed interface change.|
|The working-precision heuristic cannot bound every chaotic orbit.|The mandatory convergence loop either accepts measured agreement, raises precision, or returns `PrecisionExhausted` at the displayed policy.|
|Scaled recurrence may lose invariance at renormalization or nonzero-reference rebase.|The f64 mirror corpus forces both exponent directions and nonzero `Z₀`, comparing every step with direct iteration.|
|The f32 mantissa and one-word-per-coordinate reference may move a bailout-boundary classification.|The propagated envelope identifies boundary fixtures; all samples outside it require exact class and index.|
|A reference shift expressed in current pixels may be misapplied to an older pose.|The native pose-rebase test transforms the shift through both bases and scales, then compares reconstructed ℝ⁴ centres.|
|Authoritative navigation may use a rounded mirror, the wrong drag scale, or the wrong sign.|The Astro-float navigation fixtures pin the anchor invariant through depth, exact negative after-scale drag, displacement round-trip, and atomic failure on invalid arithmetic.|
|A single native bignum probe does not predict wasm worker speed or memory.|A labelled visible replay reports `compute_us`, credit, wasm size, and both instance memories at all four probe points.|
|The 300-digit precision ceiling rejects valid deeper requests.|Overlay distinguishes requested depth, working precision, and policy refusal; a later increase requires measured worker memory/time evidence.|
|The anchor homography cannot reconstruct internal disocclusion or escape height.|Present's 9×9×5 visible error corpus reports max and p95 pixels and labels stale regions; evidence `requires visible replay`.|
|Generation and epoch exhaustion could become silent wrap.|Native tests begin one below each maximum and require typed session refusal.|
|Normal rendering cannot cheaply total rebase and glitch channels.|Overlay says `unavailable`; an explicit labelled measurement readback is the sole counting oracle.|

## 7. Implementation phases and line budget

Phase 0 adds the package skeleton, Astro-float pin, core errors, exact record and layout assertions, worker codec adapter, wasm build check, and retained native probe fixture, estimated at 360 Rust and test lines.

Phase 1 adds ℝ⁴ PLANE coefficients, presets, one-pass f32 plane construction, centre mirror and split, centre displacement/reference shift, scale mantissa/exponent, zoom and precision plans, estimated at 420 lines.

Phase 2 adds the contracted `f32` escape reference, high-precision orbit builder, `D` versus `D+16` validation, record conversion, cooperative stepping, and native fixtures, estimated at 450 lines.

Phase 3 adds the scaled f64 perturbation mirror, exponent renormalization, corrected rebasing, glitch state machine, propagated error envelope, mixed-plane corpus, and counter limits, estimated at 540 lines.

Phase 4 adds `Homography`, the f64 neutral-height screen map and explicit inverse, `PoseMap`, reference-shift re-expression, composed screen-map warps, mapped target/drag/scale navigation, warp accuracy, ambient geometry oracles, and the conditional faer decision point.

Phase 5 reconciles compile-time interfaces with worker, kernels, present, and app, adds cross-package fixtures without editing sibling packages, and closes documentation, estimated at 250 lines.

Integration-audit addendum publishes the math-owned `NavigationDelta`, atomic Astro-float centre mutation, displacement projection, and direct f64 mirror needed by worker and app, estimated at 300 Rust, test, and contract lines.

Fullscreen implementation checkpoint: `screen_to_plane` constructs the forward affine-camera chain consumed by scene WGSL, takes the canonical identity paths bit for bit, inverts with the shared `1e−12` pivot refusal, publishes `κ∞`, routes target, box, scale and drag through `M`, and supplies each downstream level map without forming an absolute deep coordinate.

The implementation estimate is about 2,410 new Rust and test lines; Cargo metadata and generated lockfile movement are reported separately, and implementation starts only after this refined document is accepted.

## 8. Unresolved joint-review list

- The one-f32-per-coordinate reference remains an oracle-backed bet rather than a proof at every accepted 100–300 digit centre; a failed Final requires precision escalation and explicit refusal on policy exhaustion.
- The 300-digit ceiling and 4,096 iteration cap are product policies, not mathematical completeness claims, and some accepted navigation requests will honestly refuse.
- `reference_shift_px` is zero for the first accepted reference by convention because no old reference exists; worker and present tests must agree on that first-arrival sentinel without treating it as a measured zero shift.
- Cross-slice source fixtures pin math's records, discriminants, callable signatures, cooperative orbit boundary, displacement directions, and authoritative navigation API; worker's formerly unresolved navigation item is closed by this math-owned interface, while downstream adoption remains orchestrator integration work.
- The scaled GPU operation sequence, especially exponent-aware products near subnormal range, still needs browser conformance evidence against the f64 mirror.
- Astro-float won the native probe, but browser worker throughput, wasm size, and duplicated instance-memory cost remain unmeasured.
- The exact visible acceptance envelope for the anchor warp remains present policy and cannot be retired by math's inverse-times-forward oracle alone.
- Aggregate rebase and glitch totals remain unavailable during normal rendering; whether explicit measurement mode is worth its readback cost is an app decision.
