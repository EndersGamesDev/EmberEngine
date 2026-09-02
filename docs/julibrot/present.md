# Julibrot presentation slice

Status: round-one implementation and cross-slice interface contract for `crates/labs/julibrot/present`; joint review may refine this document before implementation, and the app document wins any unresolved integration disagreement.

## 1. Ownership and boundary

The present slice owns pixels after an `EscapeGrid` exists: the flat view, the tumbled view, palette records and palette evaluation, the two scene textures, the one warp pass, the three-slot HOT uniform ring, scene and warp completion measurements, and the present facts exported to the app.

The present slice allocates the HOT GPU buffer and exposes infallible `Presenter::write_hot`; the app calls it from the owner's HOT drain once per surface refresh, selects the slot by refresh number, acquires and presents the surface texture, and draws the honest page overlay outside the warped scene image.

The present slice consumes a typed `EscapeGrid` and immutable heap resource identities; kernels own refinement LEVEL definitions, escape dispatches, span reuse, and SCRATCH-to-DATA landing, while the app owns the refinement SCHEDULE and decides when to ask present for a scene.

Math owns the construction and validation of `Plane`, the high-precision centre, presets, perturbation and rebasing theory, and the navigation and warp arithmetic reference oracles; present consumes the frozen plane and performs only the f64 image-warp algebra contracted below.

Worker owns reference-orbit computation and transfer, and present neither sees nor retains an orbit buffer; the reference record is repeated in §3 only to pin the transitive ABI against which the escape grid was produced.

The app owns wgpu 24 GL device creation, the sole surface-acquisition token, surface configuration and recovery, panic-hook installation, the non-panicking uncaptured-error handler, owner draining, controls, scheduling, and the facts overlay; present receives a borrowed surface view and never acquires, retains, or presents a `SurfaceTexture`.

Present is cosmetic authority only: a missing grid, stale span, invalid warp, pole rejection, timeout, or device error can change or clear pixels and publish a typed fact, but cannot author navigation, iteration, worker, simulation, protocol, or reconciliation truth.

The general DAG and petgraph, more than one world, a simulation tick, more than one heap class, shared-memory workers, WebGPU, a second glitch reference, mipmaps, blending, MSAA, motion vectors, depth-aware warp, and any pass beyond the selected scene pass plus the warp pass are deliberately absent.

## 2. Design

### 2.1 Coordinates, records, and sampling

The four fractal axes are ordered `(z.re, z.im, c.re, c.im) = (e₁,e₂,e₃,e₄)`; `e₅` is the escape height used only by the tumbled VIEW and never belongs to the sampled fractal plane.

The PLANE rotation is `Rₚ(θ₁,θ₂) = R₁₂(θ₁)R₃₅(θ₂)` on column vectors, where each named plane uses `[[cos θ,−sin θ],[sin θ,cos θ]]`, the two plane angles are independent radians, and the map is applied as `v′ = R₁₂(R₃₅(v))`.

For a preset basis pair `(e_a,e_b)`, math supplies `u = P₄(Rₚe_a)` and `v = P₄(Rₚe_b)` after Gram–Schmidt re-orthonormalization, with `P₄` dropping `e₅`; Mandelbrot uses `(e₃,e₄)` and origin `(0,0,0,0)`, while Julia at `c₀` uses `(e₁,e₂)` and origin `(0,0,c₀.re,c₀.im)`.

The VIEW rotation is distinct and present-only: `R(t) = R₁₂(θ)R₃₅(φθ)`, `θ = 0.4t`, `φ = (1+√5)/2`, `t` is monotonic seconds, and both angles are frozen into the HOT slot used by one scene or warp submission.

Grid sample `(i,j)` is the pixel centre `(i+0.5,j+0.5)`; `+v` is up, row zero is the bottom row, the linear record index is `j·width+i`, and square pixels require `height/width` to equal the delivered canvas aspect up to the kernels level's integer rounding.

The CPU scale is `pixel_scale = 4/(2^zoom_log2·grid_width)`, so the sampled offsets are `δ = ((i+0.5−width/2)u + (j+0.5−height/2)v)·pixel_scale`; the GPU deep kernel receives only this relative f32 offset and never an absolute deep centre.

The escape DATA texel is loaded with integer `textureLoad` through the `DataSpan` directory and descriptor table; no float sampler, interpolation, CPU readback, or re-packed presentation copy lies between the paid kernel output and the scene shader.

### 2.2 Flat scene

The flat scene is one fullscreen triangle covering an LDR `Rgba8Unorm` scene target and one fragment per scene-target pixel; it converts the fragment centre to the nearest delivered grid `(i,j)`, performs one handle-resolved escape-record load, and maps that record to an opaque palette colour.

The scene target extent equals the delivered `EscapeGrid` extent, so a refinement level is both the delivered escape resolution and the delivered scene resolution; a changed extent is an allocation event, not a per-frame write, and is counted in `texture_reallocations`.

For an escaped non-glitch record, `hue = fract(smooth_iter/period + phase)`, `phase_rgb = clamp(abs(fract(hue + (0,2/3,1/3))·6−3)−1,0,1)`, and `rgb = value·mix((1,1,1),phase_rgb,colour_mix)`; an interior record uses `interior_rgba` exactly.

The shader tests `glitch == 1` before `escaped`, emits the fixed opaque debug tint `(1,0,1,1)`, and never filters that classification; malformed non-binary `escaped` or `glitch` is also debug tinted and counted as a presentation contract violation.

### 2.3 Tumbled scene

The tumbled view uses one indexed triangle-list mesh with `width·height` vertices and `6·(width−1)·(height−1)` `u32` indices; for cell lower-left `a = j·width+i`, `b=a+1`, `c=a+width`, and `d=c+1`, the exact index sequence is `[a,b,c,b,d,c]`, although culling remains disabled.

The display-normalized plane coordinates are `q_u = 4·((i+0.5)/width−1/2)` and `q_v = 4·(j+0.5−height/2)/width`; this equals the physical plane offset multiplied by `2^zoom_log2`, preserves square pixels, spans almost four units horizontally, and avoids multiplying very small deep-zoom offsets in the vertex shader.

For a valid escaped sample, escape height is `h₅ = 4·clamp(smooth_iter/max(max_iter,1),0,1)−2`; an interior sample uses `h₅ = −2`, and a glitch or malformed sample uses neutral height zero plus the debug tint so the geometry does not pretend to know the missing orbit continuation.

Each vertex begins as `p = (q_u·u + q_v·v, h₅) ∈ ℝ⁵`, then applies the frozen VIEW rotation and the standing double perspective `P₅(p) = 8/(8−p₅)·(p₁,p₂,p₃,p₄)` followed by `P₄(y) = 8/(8−y₄)·(y₁,y₂,y₃)`.

Either perspective denominator at or below `ε = 1e−4` invalidates the vertex and clips its incident triangles by emitting the fixed outside-clip position; denominators are tested before division so a pole never becomes a NaN convention.

The implementation depends on `ember-lab-heap` and reuses its exported pure CPU oracle `mode_a_endpoint` with zero lattice coordinate and a `FrameUniform` carrying `[cos θ,sin θ,cos(φθ),sin(φθ)]`, poles `[8,8]`, and epsilon `1e−4`; the present WGSL operation order is tested against that function rather than copied into a second Rust oracle.

The indexed-grid construction follows the pure-data pattern of `ember_lab_heap::box_vertices` and `ember_lab_heap::BOX_INDICES`, but it does not call or duplicate their long-box geometry because the contracted object is a triangle height field; `ember_lab_heap::frame_for` remains the pinned source for `θ=0.4t`, the golden ratio second angle, and the two pole constants.

After double perspective, the rawgl camera adopted by the heap lattice is carried by value: yaw is 20 degrees with cosine `0.9396926208` and sine `0.3420201433`, pitch is 15 degrees with cosine `0.9659258263` and sine `0.2588190451`, camera distance is `9.0`, perspective scale is `1.72`, near is `0.1`, and far is `30.0`.

For world point `(x,y,z)`, `yawed = (cy·x+sy·z,y,−sy·x+cy·z)`, `view = (yawed.x,cp·yawed.y−sp·yawed.z,sp·yawed.y+cp·yawed.z−9)`, and clip position is `(1.72·view.x/aspect,1.72·view.y,(far/(near−far))·view.z+far·near/(near−far),−view.z)`.

The depth expression is rawgl's OpenGL projection converted to wgpu's zero-to-one depth range, maps view z `−0.1` to zero and `−30` to one, and uses `LessEqual`; the pipeline is one-sample, has no blend, no mipmaps, and `cull_mode: None`.

The tumbled fragment obtains a surface normal from derivatives of the interpolated double-projected world position, uses fallback `(0,0,1)` when the derivative cross product is degenerate, and applies the heap-pinned light `0.58 + 0.24·|n·normalize(0.4,0.7,0.6)|` and colour `mix(white,hue_rgb,colour_mix)·value·light`.

The fragment also performs a nearest integer escape-record load from the interpolated grid coordinate and branches on its exact glitch flag, so the debug tint is not interpolated across neighbouring vertices; rawgl's `0.013` long-box thickness is explicitly inapplicable to a triangle height field and is the only §10 heap presentation literal not used.

### 2.4 Scene texture pair and submission

Exactly two single-sample, one-mip `Rgba8Unorm` scene textures exist: one is the newest fence-completed texture sampled by warp and the other is the sole in-flight scene target; before the first completion there is no retained texture and the non-target texture has no semantic content.

A scene submission captures `PresentMain` plus the referenced HOT slot into an immutable `Pose`, encodes exactly one flat or tumbled scene pass, submits its four-byte MAP_READ fence immediately after that pass, and returns without awaiting completion.

While the scene fence is pending, every surface refresh may submit a warp against the previous completed texture; after the fence callback, `Presenter::poll` atomically promotes the in-flight texture, pose, palette, grid extent, and measurement, and only then makes the previous texture available as the next target.

If a new level has a different extent, only the available target is reallocated before scene submission, its immutable warp bind group is rebuilt once and the allocation count advances; the retained texture and its bind group stay valid until promotion, keeping the total at two textures.

Scene requests while a target is already in flight return `PresentError::SceneBusy` instead of allocating a third texture, blocking, or overwriting work; an older orbit generation may complete and be measured but is dropped rather than promoted when it is no longer the app's current generation.

### 2.5 Exact flat homography

Let `B_p = [u_p v_p]` be the 4-by-2 orthonormal plane basis for pose `p`, let `D_p = diag(2·2^(−zoom_p), 2·2^(−zoom_p)·height_p/width_p)`, and let NDC chart point `q=(x,y)` denote the four-dimensional point `X_p(q)=origin_p+B_pD_pq`.

For a current destination pose `t` and retained source pose `f` with the same orbit generation and bit-identical `origin_lo`, the inverse-sampling affine homography is `H_(t→f) = [[A₀₀,A₀₁,0],[A₁₀,A₁₁,0],[0,0,1]]`, where `A = D_f^(−1)B_fᵀB_tD_t`.

The shader evaluates rows explicitly as `r = H_(t→f)·(x,y,1)`, rejects non-finite `r` or `|r.z|≤1e−12`, computes source NDC `s=(r.x/r.z,r.y/r.z)`, converts to source UV `(s+1)/2`, and emits clear colour rather than clamping whenever either UV component lies outside `[0,1]`.

This is the exact projective map between the two plane charts, with aspect folded into `D`; it is exact image reprojection when both poses span the same affine plane, while `ρ(q)=||(I−B_fB_fᵀ)(X_t(q)−origin_f)||₂` is reported as a chart-residual warning when a changed PLANE rotation moves samples outside that plane.

The absolute bignum centre never enters this matrix: a centre or `c₀` change advances orbit generation and invalidates the retained image, so present clears until a matching scene completes instead of manufacturing a low-precision translation at arbitrary depth.

All powers, dot products, the 2-by-2 inverse, and the 3-by-3 forward/inverse check are evaluated by hand-written `f64` CPU code; only the final three padded rows are rounded to f32 for `HotUniform`.

### 2.6 Tumbled warp approximation

Tumbled warp is deliberately depth-free and uses one 2D homography of the already presented image, because adding depth, a coarse displaced warp mesh, or a second warp pass would answer a different lab question and violate the one-extra-pass constraint.

The four destination anchors are current chart corners `q ∈ {(-1,-1),(1,-1),(-1,1),(1,1)}`; each corner maps through the flat chart homography to source chart coordinate `q_f`, both source and destination build their display-normalized five-dimensional point at neutral height `h₅=0`, and each is taken through its pose's VIEW rotation, double perspective, fixed rawgl camera, and NDC projection.

The implementation solves the eight projective coefficients of the current-NDC-to-source-NDC homography with f64 Gaussian elimination and partial pivoting, fixes `h₂₂=1`, refuses a pivot below `1e−12`, and rounds the valid result to the same three-row HOT layout used by flat warp.

Neutral height makes the approximation exact at the four anchors but not between them or at nonzero escape height; the native oracle samples a 9-by-9 chart lattice at `h₅ ∈ {−2,−1,0,1,2}`, compares homography source pixels with full per-point reprojection, and reports maximum and p95 error in output pixels.

The acceptance envelope is `|Δθ_view|≤0.02 rad`, `|Δzoom_log2|≤0.25`, unchanged orbit generation, and maximum sampled error at or below `2.0` pixels for a 1920-by-1080 target; outside it the warp remains a visibly labelled approximation, publishes that a fresh scene is needed immediately, and never turns its observed error into an invented correction.

For either view, newly exposed source coordinates outside the retained texture show `clear_rgba`; a single homography cannot detect internal tumbled visibility changes, so internal disocclusion is a candid stale-image limitation rather than being called filled or corrected.

### 2.7 Refresh, initial image, and measurements

Every refresh follows the fixed order `poll completed fences → drain HOT → write_hot(refresh_id mod 3) → frame(state,hot_slot) → app present`, so a promoted scene and its warp matrix cannot cross unnoticed and an earlier queued submission reads its slot before a later queue write reuses that slot.

When no compatible completed frame exists, the warp pass writes only `clear_rgba`; the app simultaneously displays the literal overlay text `waiting for first completed scene` or the current typed refusal, and no diagnostic pattern or stale incompatible image is substituted.

The warp samples the retained `Rgba8Unorm` texture with a nearest sampler and no mipmaps, preserving debug-tint classification; both flat and tumbled disocclusion tests happen before the sample and use the palette's clear colour.

Timing uses no timestamp query: scene cost starts immediately before scene uniform writes and encoding and ends when the four-byte fence mapped after scene submission completes, while warp cost starts immediately before HOT write and warp encoding and ends when the four-byte fence mapped after warp submission completes.

Each fence records total wall milliseconds, the subset spent from first `map_async` poll through callback observation, and every `device.poll`; the first poll precedes yielding, the bound is 4,096 polls and 30,000 ms, and timeout or cancellation becomes a typed event rather than an unbounded wait.

The first fenced scene and warp after initialization, texture reallocation, pipeline creation, or view-mode change are labelled warm-up and excluded from aggregates, but their walls and polls remain displayed.

`reprojected_per_scene` counts fence-completed warp submissions that sampled one completed scene and is published when that scene is replaced; refreshes shown as clear before the first frame are counted separately and are never credited to a scene.

## 3. INTERFACES

### 3.1 Representation conventions

All GPU records and transferred numeric buffers are little-endian; byte offsets below are from record start, every listed reserved word is written as zero and rejected if nonzero on decode, units are explicit, and CPU-only Rust records marked “no byte ABI” must not be serialized by layout.

`u32` generation is a monotonic orbit counter whose wrap is impossible within one session, the owner's published `epoch` is `u64`, and HOT and MAIN drains each bump that shared epoch; presentation compatibility uses orbit generation and origin, not epoch equality, because HOT epochs advance between scene frames by design.

### 3.2 Shared math and kernel records

`Plane` is `#[repr(C)] { basis_u: [f32;4], basis_v: [f32;4], origin_lo: [f32;4] }`, exactly 48 bytes: `basis_u` occupies bytes 0–15, `basis_v` 16–31, and the f32 low mirror of the four-dimensional plane origin 32–47; all coordinates use axis order `(z.re,z.im,c.re,c.im)` and basis vectors are finite, unit length, and mutually orthogonal within math's bound.

The worker retains the authoritative bignum centre and the owner mirrors it as `[f64;4]`; `Plane::origin_lo` is sufficient only for compatibility and shallow provenance, never for reconstructing an absolute deep centre or translating a retained image.

`EscapeParams` is `#[repr(C)] { max_iter: u32, bailout: f32 }`, exactly 8 bytes with `max_iter` at 0–3 and squared-radius `bailout` at 4–7; Julibrot v1 fixes `bailout = 256.0`.

The reference-orbit `RGBA32F` texel is exactly 16 bytes `[re_hi,im_hi,re_lo,im_lo]` for `Zₙ`, with index zero equal to `Z₀`, the centre's z part, and stored length `min(max_iter,escape_index+1)`.

The escape-grid `RGBA32F` texel is exactly 16 bytes `[smooth_iter,escaped,rebase_count,glitch]`; `escaped` and `glitch` are binary f32 values, `rebase_count` is integer-valued f32, and `smooth_iter = n+1−log₂(log₂|zₙ|)` at escape or `−1.0` when not escaped.

Perturbation is `δₙ₊₁ = 2Zₙδₙ+δₙ²+δc`, `δ₀=δz₀`, and `zₙ=Zₙ+δₙ`; when `|zₙ|<|δₙ|`, kernels set `δ←zₙ`, reset reference index to zero, and increment `rebase_count`, while reaching reference length before escape or `max_iter` sets `glitch=1` and stops.

`EscapeGrid` is the kernels-owned Rust record `pub struct EscapeGrid { pub span: ember_lab_heap::DataSpan, pub width: u32, pub height: u32, pub level: u32 }` with no byte ABI; kernels publish it only after SCRATCH-to-DATA copy ordering is established, `span.logical_len == width·height`, and the level's delivered extent and iteration cap are known.

The present slice borrows or clones the `DataSpan` handle record but never frees it; kernels retain allocation authority until app scheduling proves that no in-flight or completed `SceneFrame` names the span, and a stale generation resolves to a typed heap error rather than missing pixels.

### 3.3 Inherited heap ABI

`ember_lab_heap::Handle` is one `u32`: descriptor index occupies bits 0–19, generation occupies bits 20–31, raw zero is invalid, and generation zero is never allocated.

One descriptor is 16 bytes and four u32 words: word 0 packs `layer` bits 0–15 and `x` bits 16–31, word 1 packs `y` bits 0–15 and `width` bits 16–31, word 2 stores `height` bits 0–15 with its high half zero, and word 3 stores heap kind in bit zero (`0` DATA) with all other bits zero.

One span header is 16 bytes `[page_records,page_count,first_directory_slot,0]`; `DataSpan` additionally carries CPU fields `{logical_len:u32,page_records:u32,page_count:u32,first_directory_slot:u32,directory_index:u32,handles:Vec<Handle>}` with no Rust byte ABI.

For logical index `k`, span lookup is `page=k/page_records`, `local=k%page_records`, handle slot `first_directory_slot+page`, descriptor index `handle & 0x000f_ffff`, and descriptor-local texel `(local%width,local/width)`; out-of-range access returns canonical zero in generated accessors but is a typed error in CPU validation.

`HeapPresentResources` is the heap-produced CPU record `pub struct HeapPresentResources { pub data_view: Arc<wgpu::TextureView>, pub descriptor_buffer: Arc<wgpu::Buffer>, pub span_directory_buffer: Arc<wgpu::Buffer>, pub descriptor_capacity: u32, pub span_capacity: u32, pub handle_capacity: u32 }` with no byte ABI; present creates its heap bind group once from these identities and specializes WGSL array lengths from the three capacities.

The DATA texture is nearest, unfiltered `Rgba32Float`; descriptor and directory buffer contents may receive coalesced regional writes at allocation or relocation, but their resource identities and present's heap bind-group identity never change.

### 3.4 Owner/app to present CPU records

`ViewMode` is `#[repr(u32)]` with `Flat=0` and `Tumbled=1`; every other value is a typed decode error.

`PaletteId` is `#[repr(u32)]` with `Classic=0`, `Ember=1`, and `Ice=2`; app MAIN state selects only the identifier, while present owns the records and rejects no valid enum during the infallible drain.

`PaletteRecord` is `#[repr(C, align(16))] { map:[f32;4], interior_rgba:[f32;4], clear_rgba:[f32;4] }`, exactly 48 bytes, where `map=[period,phase,colour_mix,value]`, period is iterations per hue cycle, phase is turns, and colours are linear RGBA.

The exact v1 records are Classic `{map:[64,0,0.78,1], interior:[0.005,0.005,0.008,1], clear:[0.015,0.018,0.025,1]}`, Ember `{map:[48,0.02,0.88,1], interior:[0.01,0,0,1], clear:[0.015,0.008,0.005,1]}`, and Ice `{map:[80,0.55,0.72,1], interior:[0,0.005,0.01,1], clear:[0.005,0.01,0.015,1]}`.

`PresentHot` is the CPU-only record `pub struct PresentHot { pub epoch:u64, pub plane:Plane, pub plane_theta_1:f64, pub plane_theta_2:f64, pub zoom_log2:f64, pub view_time_seconds:f64 }`; both plane angles are independent radians, and app constructs the record from the newest owner HOT values and current math plane after each infallible HOT drain.

`PresentMain` is the CPU-only record `pub struct PresentMain { pub epoch:u64, pub orbit_generation:u32, pub grid:EscapeGrid, pub max_iter:u32, pub palette:PaletteId, pub view:ViewMode }`; app constructs it after each infallible MAIN drain and current kernels publication, and `max_iter` is both the delivered iteration cap and the height-normalization denominator.

`Pose` is the immutable CPU-only scene snapshot `pub struct Pose { pub epoch:u64, pub orbit_generation:u32, pub plane:Plane, pub plane_theta_1:f64, pub plane_theta_2:f64, pub zoom_log2:f64, pub view_theta_1:f64, pub grid_width:u32, pub grid_height:u32, pub view:ViewMode }`; the PLANE angles remain independent, while `view_theta_2` is derived as `φ·view_theta_1` and is not stored independently.

`HotSlot` is the opaque CPU token `{index:u32,dynamic_offset:u32,epoch:u64}` returned by `HotSlot::for_refresh(refresh_id,slot_stride,epoch)`, where `index=refresh_id mod 3` and `dynamic_offset=index·slot_stride`; only this constructor can create a slot, making `write_hot` infallible.

`FrameState<'a>` is the CPU-only app input `{surface_view:&'a wgpu::TextureView, canvas_width:u32, canvas_height:u32, refresh_id:u64, now_ms:f64}`; dimensions are physical pixels, `now_ms` is the app's monotonic `performance.now()` sample, and the app keeps the surface texture alive until `frame` returns and then presents without awaiting its fence.

### 3.5 GPU uniform blocks

`HotUniform` is exactly 128 bytes in eight 16-byte lanes: byte 0 `plane_u:[f32;4]`, 16 `plane_v:[f32;4]`, 32 `view_rotation:[f32;4]=[cos θ,sin θ,cos(φθ),sin(φθ)]`, 48 `homography_row_0:[f32;4]`, 64 `homography_row_1:[f32;4]`, 80 `homography_row_2:[f32;4]`, 96 `clear_rgba:[f32;4]`, and 112 `flags:[u32;4]=[epoch_low,epoch_high,source_valid,view_mode]`.

Each homography row stores three coefficients and a zero padding word, and the shader uses explicit row dot products rather than WGSL matrix layout; `source_valid` is one only when generation, origin, dimensions, finite arithmetic, and matrix construction all pass.

The HOT buffer size is `3·slot_stride`, where `slot_stride = align_up(128,device.limits.min_uniform_buffer_offset_alignment)`; one bind group covers the whole buffer, each pass selects exactly one slot by dynamic offset, and a refresh writes exactly 128 payload bytes to that slot.

`SceneUniform` is exactly 80 bytes in five 16-byte lanes: byte 0 `grid:[u32;4]=[width,height,level,max_iter]`, 16 `span:[u32;4]=[directory_index,logical_len,0,0]`, 32 `palette_map:[f32;4]`, 48 `interior_rgba:[f32;4]`, and 64 `clear_rgba:[f32;4]`.

`SceneUniform` is rewritten only when MAIN selection, palette, view, level, extent, span, or iteration cap changes; index-buffer updates and texture/bind-group replacement are regional allocation events tied to an extent change and are never per-refresh work.

### 3.6 Scene, warp, events, and facts

`SceneFrame` is the presenter-owned CPU record `{scene_id:u64,pose:Pose,palette:PaletteId,iteration_cap:u32,level:u32,extent:[u32;2],texture_index:u32,measurement:SubmissionMeasurement}` with no byte ABI; `texture_index` is zero or one and names the fence-completed retained texture.

`SubmissionMeasurement` is `{kind:SubmissionKind,id:u64,source_scene_id:Option<u64>,warm_up:bool,wall_ms:f64,fence_wait_ms:f64,polls:u32}`; `SubmissionKind` is `Scene` or `Warp`, milliseconds are measured monotonic walls, and `source_scene_id` is unavailable for a clear-only warp.

`WarpPlan` is `{rows:[[f32;4];3],source_valid:bool,kind:WarpKind,chart_residual:f64,approx_max_error_px:Option<f64>,approx_p95_error_px:Option<f64>}` with no byte ABI; `WarpKind` is `FlatExact`, `TumbledHomography`, or `ClearOnly`.

`PresentEvent` messages are `SceneCompleted { frame:SceneFrame }`, `SceneDropped { scene_id:u64, orbit_generation:u32, reason:DropReason, measurement:SubmissionMeasurement }`, `WarpCompleted { measurement:SubmissionMeasurement }`, and `FenceRefused { kind:SubmissionKind,id:u64,reason:FenceRefusal,polls:u32,wall_ms:f64 }`.

`DropReason` is `StaleGeneration`, `ReplacedMain`, or `InvalidExtent`; `FenceRefusal` is `PollLimit`, `Deadline`, `Device`, or `Cancelled`, and each variant is rendered verbatim by the app rather than collapsed into “slow.”

`PresentFacts` is `{completed_scene_id:Option<u64>,in_flight_scene_id:Option<u64>,source_generation:Option<u32>,delivered_width:u32,delivered_height:u32,delivered_level:Option<u32>,iteration_cap:Option<u32>,palette:PaletteId,view:ViewMode,last_scene:Option<SubmissionMeasurement>,last_warp:Option<SubmissionMeasurement>,reprojected_per_scene:Option<u32>,refreshes_without_scene:u64,texture_reallocations:u32,chart_residual:Option<f64>,tumbled_max_error_px:Option<f64>,status:PresentStatus}`.

`PresentStatus` is `WaitingForFirstScene`, `ShowingCompletedScene`, `ShowingStaleApproximation`, `ClearForIncompatibleGeneration`, or `Refused(PresentError)`; app combines these delivered and measured facts with its own requested resolution, requested level, requested iteration cap, zoom digits, precision, orbit length, and rebase/glitch counts without substitution.

### 3.7 Callable API

`Presenter::new(device:Arc<wgpu::Device>,queue:Arc<wgpu::Queue>,heap:HeapPresentResources,config:PresentConfig)->Result<Presenter,PresentError>` allocates the 3-slot ring, two initially empty texture slots, fixed pipelines, immutable heap group, and two immutable warp groups as texture slots become allocated; it performs no device call before app has installed both error handlers.

`PresentConfig` is `{surface_format:wgpu::TextureFormat,min_uniform_buffer_offset_alignment:u32,fence_deadline_ms:f64,max_fence_polls:u32}` and v1 requires the app to pass the live alignment, `30_000.0`, and `4_096`; scene format remains fixed `Rgba8Unorm` independent of surface format.

`Presenter::set_main(&mut self,main:PresentMain)` is the infallible MAIN-drain endpoint: it records latest-wins state and invalidates incompatible retained publication without allocating, submitting, waiting, or returning an error.

`Presenter::write_hot(&mut self,slot:HotSlot,hot:PresentHot)` is the infallible HOT-drain endpoint: it stores the CPU pose, computes the f64 warp plan against the currently retained frame, writes exactly one 128-byte ring payload, and falls back to `source_valid=0` on invalid arithmetic.

`Presenter::submit_scene(&mut self,hot_slot:HotSlot,now_ms:f64)->Result<u64,PresentError>` captures current MAIN and the exact HOT pose, prepares changed regional data, encodes one scene pass to the available texture, submits a four-byte fence, and returns its monotonically increasing `scene_id` without waiting.

`Presenter::frame(&mut self,state:FrameState<'_>,hot_slot:HotSlot)->Result<FrameReceipt,PresentError>` is called once per surface refresh, encodes the sole warp pass to `state.surface_view`, samples the newest compatible completed scene or writes clear colour, submits its four-byte fence, and returns before the app presents.

`FrameReceipt` is `{refresh_id:u64,warp_id:u64,source_scene_id:Option<u64>,status:PresentStatus}` with no byte ABI and contains no fabricated wall because its fence has not completed.

`Presenter::poll(&mut self,now_ms:f64)->Vec<PresentEvent>` performs at most one `device.poll` observation per pending fence, counts it, promotes only a current completed scene, retires bounded failures, and never waits or yields internally; app's refresh loop supplies the browser yield between polls.

`Presenter::facts(&self)->PresentFacts` returns the latest immutable snapshot without polling, allocating, submitting, or draining owner state.

`Warp::reproject(last_frame:&SceneFrame,from_pose:&Pose,to_pose:&Pose)->WarpPlan` computes the flat exact or tumbled approximate current-to-source homography in f64, validates compatibility and finite results, and never touches the GPU; `last_frame.pose` must equal `from_pose` or the result is `ClearOnly`.

`PresentError` is `InvalidGrid { width:u32,height:u32,logical_len:u32 }`, `SceneBusy { scene_id:u64 }`, `StaleSpan { directory_index:u32 }`, `UnsupportedSceneFormat`, `UnsupportedSurfaceFormat { format:wgpu::TextureFormat }`, `ExtentAllocation { width:u32,height:u32 }`, `IndexCountOverflow { width:u32,height:u32 }`, `Device { operation:&'static str }`, or `SurfaceTargetZero`; no variant panics and none mutates requested app controls.

### 3.8 Interface table for joint review

|Producer → consumer|Interface|Pinned payload or call|Units and byte ABI|
|---|---|---|---|
|math → present|`Plane`|`basis_u`, `basis_v`, `origin_lo`|48 bytes; f32 ℝ⁴ coordinates|
|math → kernels/present|`EscapeParams`|`max_iter`, `bailout=256.0`|8 bytes; iterations and squared radius|
|worker → math/kernels|reference record|`re_hi,im_hi,re_lo,im_lo`|RGBA32F, 16 bytes per iteration|
|kernels → present|escape record|`smooth_iter,escaped,rebase_count,glitch`|RGBA32F, 16 bytes per pixel|
|kernels → present|`EscapeGrid`|`DataSpan,width,height,level`|CPU record; row-major from bottom|
|heap → present|`HeapPresentResources`|DATA view, descriptor UBO, span-directory UBO, three capacities|CPU ownership record; immutable resource identities|
|owner/app → present HOT|`PresentHot`|`epoch,plane,plane_theta_1,plane_theta_2,zoom_log2,view_time_seconds`|CPU record; radians, seconds, log₂ zoom|
|owner/app → present MAIN|`PresentMain`|`epoch,orbit_generation,grid,max_iter,palette,view`|CPU record; latest-wins|
|present → GPU|`HotUniform`|plane basis, view rotation, three homography rows, clear, flags|128-byte payload at dynamic ring offset|
|present → GPU|`SceneUniform`|grid, span, palette map, interior, clear|80-byte regional MAIN payload|
|present → app|`FrameReceipt`|refresh, warp, optional source scene, status|CPU record; submission facts only|
|present → app|`PresentEvent`|completed/dropped scene, completed warp, or fence refusal|CPU message; measured walls only after completion|
|present → app|`PresentFacts`|delivered extent/level/cap plus walls, polls, counts, approximation facts, status|CPU snapshot; ms, pixels, counts|
|present CPU → present GPU|`WarpPlan`|three padded homography rows and validity|f64 construction, f32 upload|

## 4. Inherited laws and satisfaction

WebGL2 through wgpu 24 `Backends::GL` is the only substrate; app selects it explicitly, present accepts that device without backend autodetection, and no WebGPU code, promise, or fallback is compiled into this slice.

The device floor is WebGL2 plus `EXT_color_buffer_float`, at least four colour attachments, a 16 KiB uniform binding, 16 vertex texture units, 256 array layers, and 2,048 texture dimension; present additionally validates that fixed `Rgba8Unorm` scene targets and the app's chosen surface format support their declared usages and otherwise returns a typed refusal.

No `OES_texture_float_linear`, `EXT_float_blend`, timestamp query, or shared-memory thread is required; escape DATA uses integer nearest loads, scene textures use nearest sampling, colour targets have no blend, and every timing fact comes from a four-byte fence wall.

Per refresh CPU-to-GPU traffic is exactly one 128-byte HOT slot write; MAIN changes may regionally write the 80-byte scene block, an index-buffer prefix, descriptors, or changed texture resources, and every such event is counted separately rather than amortized into zero.

Kernel output reaches DATA only by the paid SCRATCH-to-DATA path, the `EscapeGrid` becomes publishable only after copy ordering, and present never copies that grid to a private texture or CPU array.

The DATA texture, descriptor buffer, span-directory buffer, HOT buffer, MAIN buffer, and their ordinary steady-state bind-group identities never change; two predeclared texture slots use immutable warp groups per allocation epoch, rebuilt only when a delivered extent requires a differently sized available slot.

The HOT ring has exactly three slots and is selected by dynamic offset; queue ordering plus refresh modulo three gives earlier submissions ordered reads before a later write reuses a slot, while the slot token prevents an arbitrary byte offset.

There is no shared memory or worker special path in present; ownership crosses this boundary only through immutable CPU records and heap handles, and same-thread worker lowering remains invisible here.

Honesty is structural: requested values remain app facts, delivered extent and iteration cap come from the current grid, zoom digits are `zoom_log2·log10(2)`, precision needed is `ceil(zoom_log2·log10(2)+log10(grid_width))+8`, unavailable measurements use `Option`, warm-ups stay labelled, polls are counted, and no timeout path loops forever.

The app installs the panic hook and replaces wgpu's fatal uncaptured-error handler before `Presenter::new`; every present device operation returns or publishes a typed error, and no bare `unreachable`, unchecked surface acquisition, or panic is an error protocol.

Hand-written f64 remains the matrix implementation unless the shared warp oracle fails its `1e−9` bound; `faer` may enter only after that measured failure, never for style or anticipation.

Renderer austerity is one selected scene pass plus the sole warp pass, no mips, no blend, `cull_mode: None`, one sample, and two scene textures total; kernel fragment-compute passes are data production and do not loosen this render-pass count.

## 5. Oracles and tests

Native layout tests assert exact sizes and offsets for `Plane`, `EscapeParams`, both RGBA32F records, `PaletteRecord`, `HotUniform`, and `SceneUniform`, assert every reserved word is zero, and round-trip little-endian fixture bytes.

Native heap-address tests build multipage `DataSpan` fixtures and prove index `j·width+i`, page quotient/remainder, descriptor decoding, bottom-row orientation, last-page padding, stale-handle rejection, and canonical-zero out-of-range behavior.

Native pixel tests use asymmetric 2-by-2 and 3-by-2 fixtures to prove centre sampling, `+v` up, row zero at bottom, nearest level resampling, exact interior classification, exact magenta glitch classification, malformed-flag tinting, and palette arithmetic within one f32 ulp of its scalar reference.

Native mesh tests prove vertex count `width·height`, index count `6(width−1)(height−1)`, the exact six-index cell order, bounds for every index, no triangles across rows, square-pixel q coordinates, neutral glitch height, and overflow refusal before allocation.

Native tumbled algebra tests compare deterministic vertices and times with `ember_lab_heap::mode_a_endpoint` at zero lattice coordinate, including both perspective poles, and parse generated WGSL to pin the rawgl camera, wgpu depth row, `LessEqual`, lighting literals, no blend, no MSAA, and `cull_mode: None`.

The shared navigation-drift oracle composes `10⁴` and `10⁵` steps of `R(Δθ)` with `Δθ=1e−3` radians and metric `||MᵀM−I||_F`; pass is at most `1e−5` for f64 without re-orthonormalization and for f32 with Gram–Schmidt every 64 steps.

The flat warp oracle evaluates hand-written f64 forward and inverse matrices for `zoom_log2 ∈ {0,10,20,40,80,100}`, both presets, square and 16:9 extents, and finite angle pairs, and requires `max|H⁻¹H−I|≤1e−9`; the uploaded f32 matrix separately must map test pixels within `0.25` source pixel where representable.

Native tumbled-warp tests pin the four neutral-height anchors exactly, exercise pivot refusal, report the 9-by-9-by-5 maximum and p95 errors, and require the §2.6 `2.0`-pixel bound throughout its acceptance envelope; failures outside that envelope are reported facts, not test failures.

Native state-machine tests permute scene completion, HOT writes, MAIN replacement, stale generation, view switch, resize, warp completion, deadline, and poll-limit events and prove exactly one retained plus one in-flight texture, latest-wins promotion, no third allocation, bounded retirement, and correct `reprojected_per_scene` attribution.

Native page-contract tests pin the five exported API calls, the exact initial overlay phrase, required facts fields, requested-versus-delivered separation, unavailable-option rendering, warm-up labels, slot order, app surface ownership, and panic/error setup before `Presenter::new`.

Browser initialization, GL backend identity, `EXT_color_buffer_float`, vertex-stage DATA access, actual surface format, exact scratch-copy visibility, and absence of validation or console errors are `requires visible replay`.

Flat/tumbled image orientation, rawgl camera resemblance, pole clipping, magenta glitch isolation under scene and warp movement, clear disocclusion, clear first frame, view switching, refinement resizing, and rapid HOT motion while a scene is in flight are `requires visible replay`.

Warp cost per refresh, scene-frame cost, fence wait, polls, warm-up exclusion, frames reprojected per scene, texture reallocations, and tumbled direct-versus-warp pixel error on the target browser are `requires visible replay`; no native estimate may populate those overlay fields.

## 6. Risks and retirement oracles

|Risk|Consequence|Oracle that retires it|
|---|---|---|
|Heap shader sees the wrong page or row|wrong fractal pixels|native multipage address fixture plus visible scratch-copy grid replay|
|A glitch flag is interpolated or filtered|hidden numerical failure|asymmetric native tint fixture plus moving visible replay with nearest warp|
|PLANE and VIEW rotations are conflated|wrong slice or animation|native preset matrices and heap `mode_a_endpoint` comparison|
|Deep zoom enters the GPU as an absolute centre|precision collapse|API/layout test proving no centre field in either GPU uniform|
|Flat warp reverses zoom, rows, or aspect|swimming or mirrored image|analytic pixel correspondences and the `H⁻¹H` oracle at all six zooms|
|A changed centre reuses an old frame|plausible but false pixels|state-machine generation/origin incompatibility test and clear status|
|Tumbled 2D warp exceeds its useful envelope|visible nonlinear swimming|9-by-9-by-5 error oracle plus labelled visible direct-versus-warp replay|
|Warp clamps an exposed edge|smeared disocclusion|UV-outside unit test and visible clear-border replay|
|An internal tumbled disocclusion is mistaken for corrected|overstated capability|explicit status/overlay contract and visible stress replay; it remains an accepted limit|
|A scene target is overwritten while sampled|race or validation error|all state-machine interleavings plus rapid visible refinement replay|
|HOT slot reuse races queued work|pose tearing|queue-order test with tagged slots and rapid visible HOT replay|
|A fence stalls forever|hung page|4,096-poll and 30,000-ms refusal tests plus suspension replay|
|Warm-up becomes a performance claim|misleading timing|measurement reset/label tests and visible sample ledger|
|Scene and warp walls include different work|invalid comparison|submission-order page-contract test and visible fence ledger|
|Rawgl presentation drifts with all in-page paths|self-consistent wrong image|native pinned-literal parser against heap §10 plus side-by-side visible replay|
|Extent churn dominates refinement|avoidable allocation wall|reported `texture_reallocations`, allocation walls, and visible level walk|
|An error handler is installed too late|unreadable wasm trap|app page-contract ordering test and deliberate visible validation refusal|

## 7. Implementation phases and line budget

Phase 0 adds the present package shell, shared records, byte-layout assertions, palette scalar reference, heap resource seam, and pure f64 homography/oracle code, estimated at 360 new Rust and test lines.

Phase 1 adds the two-texture state machine, 3-slot dynamic HOT ring, 80-byte MAIN block, bounded four-byte fence polling, typed events, and native interleaving tests, estimated at 480 lines.

Phase 2 adds the flat fullscreen scene pipeline, descriptor/span accessor generation by heap dependency, palette and honest glitch shading, resize handling, and flat image fixtures, estimated at 340 Rust/WGSL/test lines.

Phase 3 adds tumbled grid/index generation, five-dimensional vertex algebra conformance through `mode_a_endpoint`, rawgl camera/depth/lighting, nearest fragment classification, and mesh/pole tests, estimated at 520 lines.

Phase 4 adds flat and tumbled warp planning, the one fullscreen warp pipeline, clear/disocclusion behavior, f64 and f32 accuracy tests, approximation error facts, and the no-frame path, estimated at 440 lines.

Phase 5 adds the exact app-facing API, facts snapshots, warm-up and reproject counting, page-contract tests, visible-replay hooks, release reconciliation, and documentation updates, estimated at 360 lines.

The implementation estimate is 2,500 net new lines across Rust, WGSL, tests, manifests, and present-owned documentation; heap edits are limited to any joint-review-approved visibility seam and are not hidden inside this estimate.

## 8. Unresolved joint-review findings

- `HeapPresentResources` does not exist in the current heap public API; joint review must place this minimal identity-sharing seam in heap or replace it with an already planned app-owned equivalent without exposing backend-specific mutable internals.

- Existing `ember_lab_heap::mode_a_endpoint` is reusable as the CPU algebra oracle but requires a manually constructed `FrameUniform`; joint review should decide whether that is sufficient or whether a visibility-only general projection helper belongs on the app seam list for the implementation round.

- The present `EscapeGrid` contract embeds a cloneable `DataSpan`; kernels must confirm the exact lifetime handoff that prevents freeing a span still named by a retained or in-flight scene.

- The exact browser behavior of sampling an `Rgba8Unorm` scene texture whose extent differs from the surface is unmeasured on the GL backend, including whether nearest warp is visually acceptable at coarse refinement levels.

- The `2.0`-pixel tumbled approximation envelope is an argued budget, not field evidence; a visible direct-versus-warp replay may narrow the allowed motion or reject the approximation while leaving the one-pass interface intact.

- A homography exposes only external borders, so internal tumbled disocclusion remains stale until a later depth-aware design; this round intentionally has no honest one-pass repair for it.

- The height normalization maps `max_iter` to a fixed `[-2,2]` range, which makes levels comparable but may visually compress interesting low-iteration structure; palette tuning cannot answer that geometry question.

- The three v1 palettes and clear colours are interface-stable literals but have not had accessibility or target-display review; changing them after implementation would require a documented palette-version change.

- Two immutable warp bind groups are rebuilt when their corresponding texture slot changes extent; this obeys the heap identity law but the allocation/rebuild cost across rapid progressive levels is unknown until visible replay.

- `performance.now()` callback observation can be delayed by browser suspension beyond the nominal deadline; the implementation can bound active polls and reject upon resumption but cannot promise wall-clock wake-up while JavaScript is not scheduled.

- The app contract may choose a DOM overlay or presenter-side GPU text; this document assumes DOM overlay so it is late and unwarped without adding a pass, and joint review must flag any contrary app interface.

- Orbit-generation equality deliberately clears rather than translating across a centre move because owner f64 cannot represent arbitrary deep-centre deltas; whether a future bignum-relative warp is worth another interface is unresolved and out of this round.
