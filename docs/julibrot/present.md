# Julibrot presentation slice

Status: implementation complete for `crates/labs/julibrot/present`; the merged math and heap seams drive the f64 anchor planner, exact app records, two-texture runtime, HOT ring, the one scene pass, the sole warp pass, bounded four-byte fences, and app-facing facts, while target-browser facts remain labelled `requires visible replay`.

## 1. Ownership and boundary

The present slice owns pixels after an `EscapeGrid` exists: the one height-field scene from flat chart to full relief, palette records and palette evaluation, the two scene textures, the one warp pass, the three-slot HOT uniform ring, scene and warp completion measurements, and the present facts exported to the app.

The present slice allocates the HOT GPU buffer and exposes infallible `Presenter::write_hot`; the app calls it from the owner's HOT drain once per surface refresh, selects the slot by refresh number, acquires and presents the surface texture, and draws the honest page overlay outside the warped scene image.

The present slice consumes a typed `EscapeGrid` and immutable heap resource identities; kernels own refinement LEVEL definitions, escape dispatches, span reuse, and SCRATCH-to-DATA landing, while the app owns the refinement SCHEDULE and decides when to ask present for a scene.

Math owns `Plane`, `CentreSplit`, the shared `Pose`, the high-precision centre, presets, scaled perturbation and corrected rebasing theory, `warp_matrix`, and the navigation and warp arithmetic reference oracles; present consumes those records and turns the f64 matrix result into the GPU warp plan contracted below.

Worker owns reference-orbit computation and transfer, and present neither sees nor retains an orbit buffer; the reference record is repeated in §3 only to pin the transitive ABI against which the escape grid was produced.

The app owns wgpu 24 GL device creation, the sole surface-acquisition token, surface configuration and recovery, panic-hook installation, the non-panicking uncaptured-error handler, owner draining, controls, scheduling, and the facts overlay; present receives a borrowed surface view and never acquires, retains, or presents a `SurfaceTexture`.

Present is cosmetic authority only: a missing grid, stale span, invalid warp, pole rejection, timeout, or device error can change or clear pixels and publish a typed fact, but cannot author navigation, iteration, worker, simulation, protocol, or reconciliation truth.

The general DAG and petgraph, more than one world, a simulation tick, more than one heap class, shared-memory workers, WebGPU, a second glitch reference, mipmaps, blending, MSAA, motion vectors, depth-aware warp, and any pass beyond the selected scene pass plus the warp pass are deliberately absent.

## 2. Design

### 2.1 Coordinates, records, and sampling

The four fractal axes are ordered `(z.re, z.im, c.re, c.im) = (e₁,e₂,e₃,e₄)`; `e₅` is the escape height used only by the VIEW and never belongs to the sampled fractal plane.

The PLANE rotation acts only in ℝ⁴ and mixes the z and c subspaces: `Rₚ(θ₁,θ₂) = R₁₃(θ₁)R₂₄(θ₂)` on column vectors, where each named plane uses `[[cos θ,−sin θ],[sin θ,cos θ]]`, the two angles are independent radians, and the map is applied as `v′ = R₁₃(R₂₄(v))`.

For preset seed pair `(e_a,e_b)`, math evaluates `u = Rₚe_a` and `v = Rₚe_b` in f64 and performs one f32 rounding pass; no `P₄`, Gram–Schmidt, or degenerate-plane stage exists because an orthogonal ℝ⁴ map preserves an orthonormal pair, and the postcondition is `|u·u−1|`, `|v·v−1|`, and `|u·v|` each at most `8·f32::EPSILON`.

There is one seed, `(e₃,e₄)`; a Mandelbrot row is zero angles with plane origin `(0,0,0,0)` and a Julia row at `c₀` is `θ₁=θ₂=−π/2` with plane origin `(0,0,c₀.re,c₀.im)`, the rotated seed being `(e₁,e₂)` there to binary32 rounding and the reversed pair at `+π/2`, and every angle strictly between requires nonzero components in both z and c subspaces.

The VIEW rotation is distinct and present-only: `Rᵥ = R₁₂(θᵥ₁)R₃₅(θᵥ₂)`, both angles independent controls in radians, both frozen into the HOT slot used by one scene or warp submission. Present reads no clock: a monotonic time still times fences and still drives the app's schedule, but no geometric term is a function of it.

Grid sample `(i,j)` is the pixel centre `(i+0.5,j+0.5)`; `+v` is up, row zero is the bottom row, the linear record index is `j·width+i`, and square pixels require `height/width` to equal the delivered canvas aspect up to the kernels level's integer rounding.

The exact CPU scale remains `pixel_scale = 4/(2^zoom_log2·grid_width)` in f64, but deep dispatch decomposes it as `pixel_scale = m·2^s` with f32 mantissa `m∈[0.5,1)` and i32 exponent `s`; the GPU forms only scaled offset `o′=((i+0.5−width/2)u+(j+0.5−height/2)v)·m` at initial exponent `e₀=s`, so no absolute tiny f32 scale is ever formed.

The perturbation kernel is selected at `zoom_log2≥14`, a displayed POLICY, while shallower zoom uses the 96-byte shallow kernel; the reference orbit is maintained at every depth so switching kernels neither stalls presentation nor changes the `EscapeGrid` interface, and present never derives a scalar GPU scale from zoom.

The escape DATA texel is loaded with integer `textureLoad` through the `DataSpan` directory and descriptor table; no float sampler, interpolation, CPU readback, or re-packed presentation copy lies between the paid kernel output and the scene shader.

### 2.2 One scene pass and the height-zero image

There is one scene pipeline: the §2.3 height-field mesh drawn into an LDR `Rgba8Unorm` scene target. The fullscreen scene triangle and the branch that chose between it and the mesh are deleted rather than hidden, because a branch on a control value is a mode, and at height zero, zero VIEW angles, and zero camera angles the mesh projects to exactly the chart NDC `(q_u/2, aspect·q_v/2)` — the image the fullscreen pass drew. Reaching the flat picture therefore costs a control value, not a second pipeline, and the two pictures cannot drift apart because only one of them exists.

The scene target extent equals the delivered `EscapeGrid` extent, so a refinement level is both the delivered escape resolution and the delivered scene resolution; a changed extent is an allocation event, not a per-frame write, and is counted in `texture_reallocations`.

For an escaped non-glitch record, `hue = fract(smooth_iter/period + phase)`, `phase_rgb = clamp(abs(fract(hue + (0,2/3,1/3))·6−3)−1,0,1)`, and `rgb = value·mix((1,1,1),phase_rgb,colour_mix)`; an interior record uses `interior_rgba` exactly.

The shader tests `glitch == 1` before `escaped`, emits the fixed opaque debug tint `(1,0,1,1)`, and never filters that classification; malformed non-binary `escaped` or `glitch` is also debug tinted and counted as a presentation contract violation.

### 2.3 Height field, VIEW, camera, and projection

The scene uses one indexed triangle-list mesh with `width·height` vertices and `6·(width−1)·(height−1)` `u32` indices; for cell lower-left `a = j·width+i`, `b=a+1`, `c=a+width`, and `d=c+1`, the exact index sequence is `[a,b,c,b,d,c]`, although culling remains disabled.

The display-normalized plane coordinates are `q_u = 4·((i+0.5)/width−1/2)` and `q_v = 4·(j+0.5−height/2)/width`; this equals the physical plane offset multiplied by `2^zoom_log2`, preserves square pixels, spans almost four units horizontally, and avoids multiplying very small deep-zoom offsets in the vertex shader.

For a valid escaped sample the record's own height is `H = 4·clamp(smooth_iter/max(max_iter,1),0,1)−2`; an interior sample uses `H = −2`, and a glitch or malformed sample uses neutral `H = 0` plus the debug tint so the geometry does not pretend to know the missing orbit continuation.

The displayed fifth coordinate is `h₅ = h·H` for the height control `h ∈ [0,4]`, so `h=0` is exactly the flat chart, `h=1` is the amplitude the lab shipped with, and every value between is a continuous morph rather than a switch; the range extends to four because the relief is a display choice and there is no reason to forbid exaggerating it, while `h<0` is refused because it would silently invert interior and escaped and is reachable anyway by a half turn of `θᵥ₂`.

Each vertex begins as `p = (q_u,q_v,0,0,h₅) ∈ ℝ⁵` — the chart's own orthonormal display frame, in which the two spanning directions `u` and `v` are display axes one and two by construction — then applies the frozen VIEW rotation `R₁₂(θᵥ₁)R₃₅(θᵥ₂)` and the double perspective `P₅(p) = d₅/(d₅−p₅)·(p₁,p₂,p₃,p₄)` followed by `P₄(y) = d₄/(d₄−y₄)·(y₁,y₂,y₃)`, both distances controls in `[2,64]` whose neutral value is the `8` the heap lattice pins.

The earlier form `p = (q_u·u + q_v·v, h₅)` embedded the chart's ambient ℝ⁴ components directly, and that is not a frame choice but an accident of which fractal axes a preset happens to name: the VIEW rotation `R₁₂(θ)R₃₅(φθ)` leaves `span(e₃,e₄,e₅)` invariant, so every plane inside `span(e₃,e₄)` — the Mandelbrot seed among them — has `world.x = world.y = 0` at every angle, collapsing both the height field and its four warp anchors onto one line through the origin and forcing a clear-only plan. No fixed ambient embedding escapes this: composing a constant orthogonal mount `G` with the plane rotation only moves the annihilated planes to the angles where `GRₚ` again preserves `span(e₃,e₄)`, so a two-parameter family of degenerate plane angles always survives. The chart's own frame has none, and the four-dimensional body remains fully visible where it actually lives, in the escape DATA the plane selects.

The chart is an isometric two-plane, so `q_u` and `q_v` exhaust its degrees of freedom and display axis four is identically zero; `P₄` is therefore a unit scale and its pole test never fires for an in-plane sample. Both stages and both tests are kept because the double perspective is the pinned heap operation order and the guard costs nothing, and both poles now read their control rather than a literal, so a small `d₅` against a tall relief is refused honestly instead of dividing through a pole.

Because `P₄` is a unit scale, `d₄` would be an inert control if it named only that stage. It names the observer distance of §2.3's camera as well, which is the same projective operation one dimension down and the only place a fourth-to-third distance can be seen; one control, one meaning, and the pinned stage keeps its guard.

The `plane_u` and `plane_v` lanes leave the HOT layout with the pipeline that never read them: the vertex shader reads the display frame, the warp shader reads only the homography rows, and the plane-chart homography is built from `Pose.plane` on the CPU. The 128-byte contract is unchanged and those two lanes now carry the camera and scale terms of §3.5.

Either perspective denominator at or below `ε = 1e−4` invalidates the vertex and clips its incident triangles by emitting the fixed outside-clip position; denominators are tested before division so a pole never becomes a NaN convention.

The vertex also emits a numeric validity value and the fragment discards any interpolation below one, so every triangle incident to an invalid vertex is rejected rather than relying on the fixed outside-clip position alone.

The implementation depends on `ember-lab-heap` and reuses the exact exported pure CPU oracle `mode_a_endpoint(base:[f64;5],coordinate:[i32;5],frame:&FrameUniform)->ModeAEndpoint` with zero lattice coordinate and a `FrameUniform` carrying `[cos θᵥ₁,sin θᵥ₁,cos θᵥ₂,sin θᵥ₂]`, poles `[d₅,d₄]`, and epsilon `1e−4`; the present WGSL operation order is tested against that function rather than copied into a second Rust oracle.

The indexed-grid construction follows the pure-data pattern of `ember_lab_heap::box_vertices()->[BoxVertex;8]` and `ember_lab_heap::BOX_INDICES:[u16;36]`, but it does not call or duplicate their long-box geometry because the contracted object is a triangle height field; `ember_lab_heap::frame_for(object:&Prism,axes:[u32;5],time:f32,aspect:f32)->FrameUniform` is no longer called: it derives one angle from a clock and the other as a golden-ratio multiple of the first, and both of the lab's VIEW angles are independent controls. Its two pole constants survive as the neutral values of `d₅` and `d₄`, and its `axes` argument was never an axis permutation but the lattice step count per axis; the lattice may embed its vertices in standard ℝ⁵ coordinates precisely because its object genuinely occupies all five axes, which a two-dimensional chart does not.

After double perspective the three-space observer is two more controls, not a fixed mount: yaw `θ_c1` and pitch `θ_c2` in `[−π,π]`, observer distance `d₄`, near `0.1`, and far `4·d₄`.

Writing `cy=cos θ_c1`, `sy=sin θ_c1`, `cp=cos θ_c2`, and `sp=sin θ_c2`, for world point `(x,y,z)` the camera evaluates `yawed = (cy·x+sy·z,y,−sy·x+cy·z)`, `view = (yawed.x,cp·yawed.y−sp·yawed.z,sp·yawed.y+cp·yawed.z−d₄)`, and clip position `(k·view.x/aspect,k·view.y,(far/(near−far))·view.z+far·near/(near−far),−view.z)` with perspective scale `k = aspect·d₄/2`.

That one choice of `k` is what makes the height-zero image exact. At `z=0` the perspective divide is by `d₄`, the two cancel, and NDC is `(x/2,aspect·y/2)`, which is the §2.1 chart map for every `d₄` and every extent. So `d₄` sets how strongly depth foreshortens and never reframes the height-zero chart, which is what lets it be an honest perspective control instead of a disguised zoom.

The retired mount was a fixed 20-degree yaw, 15-degree pitch, distance `9`, scale `1.72` camera inherited from rawgl. It cannot be kept. It is a view degree of freedom hard-coded into the pipeline, which is exactly what the control model abolishes, and its framing differs from the chart map by the aspect-dependent factor `2·1.72/(9·aspect)`, so under it no height-zero image is the flat image and the two pictures could never be reconciled. Its two angles return as the yaw and pitch controls, where a preset that names them recovers the inherited look as a row of numbers that a user can leave, and their neutral value is zero because the flat chart is the picture the lab opens on.

The depth expression is rawgl's OpenGL projection converted to wgpu's zero-to-one depth range, maps view z `−0.1` to zero and `−30` to one, and uses `LessEqual`; the pipeline is one-sample, has no blend, no mipmaps, and `cull_mode: None`.

The scene fragment obtains a surface normal from derivatives of the interpolated double-projected world position, uses fallback `(0,0,1)` when the derivative cross product is degenerate, and applies the heap-pinned light `0.58 + 0.24·|n·normalize(0.4,0.7,0.6)|` and colour `mix(white,hue_rgb,colour_mix)·value·light`.

The fragment also performs a nearest integer escape-record load from the interpolated grid coordinate and branches on its exact glitch flag, so the debug tint is not interpolated across neighbouring vertices; rawgl's `0.013` long-box thickness is explicitly inapplicable to a triangle height field and is the only §10 heap presentation literal not used.

The VIEW rotation, the camera angles, the height scale, and both distances are HOT: each is frozen into one slot and the warp re-projects the last completed scene under the new values at refresh rate. The PLANE angles stay HOT as before. The four plane-origin coordinates are MAIN, because moving the origin selects different samples and needs a new reference orbit; that is the same publication the retired preset selection performed.

### 2.4 Scene texture pair and submission

Exactly two single-sample, one-mip `Rgba8Unorm` scene textures exist: one is the newest fence-completed texture sampled by warp and the other is the sole in-flight scene target; before the first completion there is no retained texture and the non-target texture has no semantic content.

A scene submission captures reconciled `PresentMain` plus the referenced HOT slot into math's immutable `Pose`, encodes exactly one scene pass, submits its four-byte MAP_READ fence immediately after that pass, and returns without awaiting completion.

While the scene fence is pending, every surface refresh may submit a warp against the previous completed texture; after the fence callback, `Presenter::poll` atomically promotes the in-flight texture, pose, palette, grid extent, and measurement, and only then makes the previous texture available as the next target.

If a new level has a different extent, only the available target is reallocated before scene submission, its immutable warp bind group is rebuilt once and the allocation count advances; the retained texture and its bind group stay valid until promotion, keeping the total at two textures.

Scene requests while a target is already in flight return `PresentError::SceneBusy` instead of allocating a third texture, blocking, or overwriting work; an older reference generation may complete and be measured, and present rebases its captured pose on an accepted reference shift before promotion unless changed `max_iter` or plane origin makes the image incompatible.

### 2.5 Exact plane-chart homography

Let `B_p=[u_p v_p]` be the 4-by-2 orthonormal basis, `E_p=diag(width_p/2,height_p/2)` convert NDC to pixels, and `d_p=centre_from_reference_px`; relative to accepted reference `R`, chart point `q=(x,y)` semantically denotes `X_p(q)=R+B_p·s_p(d_p+E_pq)` for `s_p=4/(2^zoom_p·width_p)`, but present never materializes either arbitrarily small `s_p`.

For current destination pose `t` and retained source pose `f` expressed against the same accepted reference, present computes only ratio `r=s_t/s_f=2^(zoom_f−zoom_t)·width_f/width_t` in f64 and the inverse-sampling homography `H_(t→f)=[[A₀₀,A₀₁,b₀],[A₁₀,A₁₁,b₁],[0,0,1]]`, where `A=E_f^(−1)rB_fᵀB_tE_t` and `b=E_f^(−1)(rB_fᵀB_td_t−d_f)`.

The translation term is the desired-centre displacement difference in retained-frame pixels; when the bases agree its components reduce to `b_x=2·(r·d_t.x−d_f.x)/width_f` and `b_y=2·(r·d_t.y−d_f.y)/height_f`, so pans remain smooth without an absolute centre or absolute deep scale.

The shader evaluates rows explicitly as `r = H_(t→f)·(x,y,1)`, rejects non-finite `r` or `|r.z|≤1e−12`, computes source NDC `s=(r.x/r.z,r.y/r.z)`, converts to source UV `(s+1)/2`, and emits clear colour rather than clamping whenever either UV component lies outside `[0,1]`.

This is the exact projective map between the two plane charts, with aspect folded into `E`; it is exact image reprojection when both poses span the same affine plane, while normalized residual `ρ(q)=||(I−B_fB_fᵀ)rB_t(d_t+E_tq)||₂` is reported in retained-frame pixel units when changed PLANE rotation moves samples outside that plane.

On an accepted reference, worker publishes `reference_shift_px` as new minus old reference centre in current-zoom pixels along accepted basis `B_a`; present computes only `r_a_f=s_a/s_f=2^(zoom_f−zoom_a)·width_f/width_a` and re-expresses every retained or in-flight pose as `d_f←d_f−r_a_fB_fᵀB_a·reference_shift_px`, then advances its reference generation without clearing the image.

The absolute bignum centre never enters this matrix: `centre_from_reference_px` and `reference_shift_px` are worker-computed from bignum differences divided by the current scale and remain f64-safe at arbitrary depth; a reference-generation change alone does not clear, while changed `max_iter` or changed MAIN plane origin including `c₀` invalidates retained and in-flight images.

A pan translates old content and exposes a border on the entering side; every mapped source coordinate outside `[0,1]` shows `clear_rgba` with no clamp, stretch, wrap, or stale edge pixel, while the next completed scene supplies the newly revealed fractal samples.

All powers, dot products, the 2-by-2 inverse, and the 3-by-3 forward/inverse check are evaluated by hand-written `f64` CPU code; only the final three padded rows are rounded to f32 for `HotUniform`.

### 2.6 Anchor warp, and why it is the only plan

Warp is deliberately depth-free and uses one 2D homography of the already presented image, for every view, because adding depth, a coarse displaced warp mesh, or a second warp pass would answer a different lab question and violate the one-extra-pass constraint.

The four destination anchors are current chart corners `q ∈ {(-1,-1),(1,-1),(-1,1),(1,1)}`; each corner maps through the flat chart homography to source chart coordinate `q_f`, both source and destination build the §2.3 display-frame five-dimensional point at neutral height `h₅=0`, and each is taken through its pose's VIEW rotation, double perspective, camera, and NDC projection.

The planner and the scene shader build that point by the identical construction, so the warp anchors and the surface they approximate are projections of the same points; a plan is degenerate only if the scene it warps is degenerate too, which the display frame makes impossible for every plane. At neutral height all four anchors carry world `z=0`, so the anchor solve is exact for the height-zero chart and the sampled corpus measures only the escape relief a plane homography cannot follow.

There is therefore one plan kind. The exact `FlatExact` plan is retired because selecting it meant asking whether both poses were in a named mode, and a branch on control values is the mode the lab abolished; the four-anchor solve is not an approximation of the exact map where the exact map exists, it reproduces it. At zero camera angles and neutral height the four anchors are the chart corners under the exact plane-chart homography, so the solved matrix equals math's `warp_matrix` forward to the solve residual, which a native test pins, and the sampled corpus reports exactly zero because every sampled height is `h·H` at `h=0`. Math's `warp_matrix` remains the anchor source, so the exact map is still computed and still audited; what is gone is the second code path that consumed it.

The implementation solves the eight projective coefficients of the current-NDC-to-source-NDC homography with f64 Gaussian elimination and partial pivoting, fixes `h₂₂=1`, refuses a pivot below `1e−12`, and rounds the valid result to the same three-row HOT layout used by flat warp.

Neutral height makes the approximation exact at the four anchors but not between them or at nonzero escape height; the native oracle samples a 9-by-9 chart lattice at `h₅ ∈ h·{−2,−1,0,1,2}`, compares homography source pixels with full per-point reprojection, and reports maximum and p95 error in output pixels; scaling the sampled heights by the height control is what makes the reported error the error of the picture actually on screen, and is why the corpus reports zero at `h=0` rather than a relief error nobody is looking at. The four-anchor solve remains unconditional, as does the uploaded-row quarter-pixel oracle; `WarpValidation::Ordinary` skips this 405-point allocation, projection, sort, max, and p95 work only under `PrecisionMode::PictureFast`, while explicit `Measure`, newly prepared `Final`, and every Deterministic refresh run it. A skipped refresh writes `None` to both error facts, never the preceding values.

The acceptance envelope is `|Δθ_view|≤0.002 rad`, `|Δzoom_log2|≤0.025`, a successfully rebased common reference, and maximum sampled error at or below `8.0` pixels for a 1920-by-1080 target. That bound is a re-measurement, not a loosening: the approximation is unchanged in chart terms, but the height-zero framing is now the chart map rather than the retired mount's `2·1.72/(9·aspect)`, so the same geometric error covers about `4.65` times as many pixels at 16:9. The `θᵥ₁=0.6` relief fixture measures `7.704` pixels where the retired mount measured under `2.0`. The envelope was also swept over one moving angle and now has four angles, a height, and two distances; the `0.002` rad bound is asserted for each angle by analogy and the height and distance terms are unmeasured, which §8 records as an open measurement rather than a bound already met; the original argued `0.02`/`0.25` envelope measured `6.394` pixels already at `0.01`/`0.1`, so outside the narrower envelope the warp remains a visibly labelled approximation, publishes that a fresh scene is needed immediately, and never turns its observed error into an invented correction.

A native sweep of 256 VIEW angles across the full turn measures that budget honestly at 1920 by 1080: the full envelope reaches `15.650` pixels and rotation with a two-pixel pan reaches `3.792`, against `3.094` and `0.762` under the retired mount — the same `4.65` framing factor, applied to a measurement that was already known to exceed its published bound. The swept oracle requires the measured `16.0` pixels for the full envelope and `4.0` for rotation and pan. The zoom step remains the dominant error source, and §8 still records the open choice between a tighter zoom step and a published bound this large; what the new observer changed is how visible that choice is, because the picture now fills the frame.

Newly exposed source coordinates outside the retained texture show `clear_rgba`; a single homography cannot detect internal visibility changes in a relief, so internal disocclusion is a candid stale-image limitation rather than being called filled or corrected.

### 2.7 Refresh, initial image, and measurements

Every refresh follows the fixed order `poll completed fences → drain HOT → write_hot(refresh_id mod 3) → frame(state,hot_slot) → app present`, with `submit_scene` when the app schedule says a scene is due; after `frame` the app drives `poll` through cooperative browser yields until the matching warp fence completes, captures the ending timestamp, and only then presents its singly owned surface texture.

When no compatible completed frame exists, the warp pass writes only `clear_rgba`; the app simultaneously displays the literal overlay text `waiting for first completed scene` or the current typed refusal, and no diagnostic pattern or stale incompatible image is substituted.

The warp samples the retained `Rgba8Unorm` texture with a nearest sampler and no mipmaps, preserving debug-tint classification; the disocclusion test happens before the sample and uses the palette's clear colour.

Timing uses no timestamp query: scene cost starts immediately before scene uniform writes and encoding and ends when the four-byte fence mapped after scene submission completes, while warp cost starts immediately before HOT write and warp encoding and ends when the four-byte fence mapped after warp submission completes.

App's per-level timing ring consumes these existing scene and warp completion measurements without adding a fence, wait, timestamp query, or present-owned record. There is no completion boundary between the separately submitted kernels encoder and the scene encoder, so app records kernel `dispatch_us` as unavailable; the scene fence remains the first GPU completion boundary and the warp fence remains the second.

Each fence records total wall milliseconds, the subset spent from first `map_async` poll through callback observation, and every `device.poll`; the first poll precedes yielding, the bound is 4,096 polls and 30,000 ms, and timeout or cancellation becomes a typed event rather than an unbounded wait.

The first fenced scene and warp after initialization, texture reallocation, or pipeline creation are labelled cold warm-up and excluded from aggregates, but their walls and polls remain displayed; the second fenced scene is the labelled policy probe and selects continuous animation at `scene_ms≤100` or single-frame-on-demand at `scene_ms>100` without becoming an admission test.

`reprojected_per_scene` counts fence-completed warp submissions that sampled one completed scene and is published when that scene is replaced; refreshes shown as clear before the first frame are counted separately and are never credited to a scene.

## 3. INTERFACES

### 3.1 Representation conventions

All GPU records and transferred numeric buffers are little-endian; byte offsets below are from record start, every listed reserved word is written as zero and rejected if nonzero on decode, units are explicit, and CPU-only Rust records marked “no byte ABI” must not be serialized by layout.

`u32` generation is a monotonic orbit counter whose wrap is impossible within one session, the owner's published `epoch` is `u64`, and HOT and MAIN drains each bump that shared epoch; consumers never use epoch equality for compatibility because HOT epochs advance between scene frames by design.

### 3.2 Shared math and kernel records

`Plane` is `#[repr(C)] { basis_u:[f32;4], basis_v:[f32;4] }`, exactly 32 bytes: `basis_u` occupies bytes 0–15 and `basis_v` occupies bytes 16–31; the origin is not a plane property, and both vectors use `(z.re,z.im,c.re,c.im)` order and satisfy math's `8·f32::EPSILON` orthonormal postcondition.

`CentreSplit` is the separate shallow-only `#[repr(C)] { hi:[f32;4], lo:[f32;4] }`, exactly 32 bytes with `hi` at bytes 0–15 and `lo` at 16–31; worker retains the authoritative bignum centre, and neither this record nor the owner's f64 mirror is deep arithmetic authority.

`EscapeParams` is `#[repr(C)] { max_iter: u32, bailout: f32 }`, exactly 8 bytes with `max_iter` at 0–3 and squared-radius `bailout` at 4–7; Julibrot v1 fixes `bailout = 256.0`.

The reference orbit transfers as 8-byte `[re,im]` points; app expands each point to one `RGBA32F` heap texel `[re,im,0,0]` for `Zₙ`, with index zero equal to `Z₀`, the centre's z part, and stored length `min(max_iter,escape_index+1)`.

The escape-grid `RGBA32F` texel is exactly 16 bytes `[smooth_iter,escaped,rebase_count,glitch]`; `escaped` and `glitch` are binary f32 values, `rebase_count` is integer-valued f32, and `smooth_iter = n+1−log₂(log₂|zₙ|)` at escape or `−1.0` when not escaped.

Scaled perturbation stores `δ′=δ/S` with `S=2^e`: `δ′ₙ₊₁=2Zᵣδ′ₙ+S·δ′ₙ²+δc′`, `δ′₀=δz₀′`, `δc′=δc/S`, and full value `zₙ=Zᵣ+S·δ′ₙ` through `ldexp`; leaving `|δ′|∈[2^−64,2^64]` renormalizes by a factor `2^∓64` and adjusts `e` by `±64`.

After the current escape test and before ordinary advance, rebase tests `|zₙ|<|S·δ′ₙ|`, with an underflowed `S·δ′` correctly making the test false; on success kernels set `δ←zₙ−Z₀` using reference record zero reconstructed hi plus lo, reset `r←0`, increment `rebase_count`, and perform exactly one ordinary advance against `Z₀`, preserving `zₙ=Zᵣ+δₙ` for nonzero `Z₀`.

If reference index `r` reaches the stored orbit length before escape or `max_iter`, kernels stop that pixel with `glitch=1`; re-rendering it from a second reference is out of scope, and present must show the fixed debug tint rather than interpolate, conceal, or continue it.

Perturbation conformance requires CPU-f64 and GPU escape classification to agree exactly outside math's propagated error envelope and smooth iteration to agree within `2×10⁻³`; boundary fixtures inside the envelope remain explicitly labelled rather than converted into a false exactness claim.

`RefinementLevel` has `#[repr(u32)]` discriminants `Preview=0`, `Interactive=1`, and `Final=2`; an unknown value is a typed decode error, never a numeric level guessed by present.

The three levels are exactly Preview `ceil(W/4)×ceil(H/4)` at `min(requested_cap,64)`, Interactive `ceil(W/2)×ceil(H/2)` at `min(requested_cap,256)`, and Final `W×H` at `min(requested_cap,4096)`; 4,096 is a displayed POLICY, allocation degrades extent by a power-of-two divisor, and app may run levels in order or skip one.

`EscapeGrid` is the kernels-owned Rust record `pub struct EscapeGrid { pub span:ember_lab_heap::DataSpan, pub width:u32, pub height:u32, pub level:RefinementLevel }` with no byte ABI; `span.logical_len` is Final capacity, `width·height≤span.logical_len` is the initialized dense prefix for this level, and publication follows accepted SCRATCH-to-DATA ordering.

The present slice borrows or clones the `DataSpan` handle record but never frees it; kernels retain allocation authority until app scheduling proves that no in-flight or completed `SceneFrame` names the span, and a stale generation resolves to a typed heap error rather than missing pixels.

### 3.3 Inherited heap ABI

`ember_lab_heap::Handle` is one `u32`: descriptor index occupies bits 0–19, generation occupies bits 20–31, raw zero is invalid, and generation zero is never allocated.

One descriptor is 16 bytes and four u32 words: word 0 packs `layer` bits 0–15 and `x` bits 16–31, word 1 packs `y` bits 0–15 and `width` bits 16–31, word 2 stores `height` bits 0–15 with its high half zero, and word 3 stores heap kind in bit zero (`0` DATA) with all other bits zero.

One span header is 16 bytes `[page_records,page_count,first_directory_slot,0]`; `DataSpan` additionally carries public CPU fields `{logical_len:u32,page_records:u32,page_count:u32,first_directory_slot:u32,directory_index:u32}` plus private `handles` exposed as `handles(&self)->&[Handle]`, with no Rust byte ABI.

For active logical index `k<width·height`, span lookup is `page=k/page_records`, `local=k%page_records`, handle slot `first_directory_slot+page`, descriptor index `handle & 0x000f_ffff`, and descriptor-local texel `(local%descriptor_width,local/descriptor_width)`; the resource entry supplies the dense active length rather than Final capacity, and present never samples stale prefix tail or padding.

Kernels obtain per-level `StaticHeaders` through the executor's `prefix_headers(span,active_len)` seam; present consumes the resulting dense prefix only and never mutates a header byte by convention.

`HeapPresentResources` is the heap-produced CPU record `pub struct HeapPresentResources { pub data_view: Arc<wgpu::TextureView>, pub descriptor_buffer: Arc<wgpu::Buffer>, pub span_directory_buffer: Arc<wgpu::Buffer>, pub descriptor_capacity: u32, pub span_capacity: u32, pub handle_capacity: u32 }` with no byte ABI; present creates its heap bind group once from these identities and specializes WGSL array lengths from the three capacities.

The DATA texture is nearest, unfiltered `Rgba32Float`; descriptor and directory buffer contents may receive coalesced regional writes at allocation or relocation, but their resource identities and present's heap bind-group identity never change.

### 3.4 Owner/app to present CPU records

Math defines `ViewControls` as the CPU-only record `pub struct ViewControls { pub theta_1:f64, pub theta_2:f64, pub camera_yaw:f64, pub camera_pitch:f64, pub height_scale:f64, pub distance_five:f64, pub distance_four:f64 }` with the neutral value `{0,0,0,0,0,8,8}`; present re-exports it, every field is finite or the pose is refused, and no enum names a view any more.

`PaletteId` is `#[repr(u32)]` with `Classic=0`, `Ember=1`, and `Ice=2`; app MAIN state selects only the identifier, while present owns the records and rejects no valid enum during the infallible drain.

`PaletteRecord` is `#[repr(C, align(16))] { map:[f32;4], interior_rgba:[f32;4], clear_rgba:[f32;4] }`, exactly 48 bytes, where `map=[period,phase,colour_mix,value]`, period is iterations per hue cycle, phase is turns, and colours are linear RGBA.

The exact v1 records are Classic `{map:[64,0,0.78,1], interior:[0.005,0.005,0.008,1], clear:[0.015,0.018,0.025,1]}`, Ember `{map:[48,0.02,0.88,1], interior:[0.01,0,0,1], clear:[0.015,0.008,0.005,1]}`, and Ice `{map:[80,0.55,0.72,1], interior:[0,0.005,0.01,1], clear:[0.005,0.01,0.015,1]}`.

Worker's `HotState` is `#[repr(C)]`, exactly 40 bytes at alignment 8: `zoom_log2:f64` at byte 0, `plane_theta_1:f64` at byte 8, `plane_theta_2:f64` at byte 16, and `centre_from_reference_px:[f64;2]` at byte 24; the displacement is from the accepted reference centre to the desired centre in current-zoom pixels along current `(u,v)`.

Worker's `MainState` is `#[repr(C)]`, exactly 128 bytes at alignment 8: `generation_applied:u32` at byte 0, `centre_revision:u32` at 4, `requested_iter_cap:u32` at 8, `delivered_iter_cap:u32` at 12, `precision_bits:u32` at 16, `orbit_length:u32` at 20, `palette_id:u32` at 24, `orbit_id:u32` at 28, `centre_f64:[f64;4]` at 32, `plane_axis_a:u32` at 64, `plane_axis_b:u32` at 68, `plane_origin_f64:[f64;4]` at 72, `reference_shift_px:[f64;2]` at 104, and `precision_mode:u32` at 120, followed by four bytes of tail padding.

Worker's `ViewerState` is `#[repr(C)]`, exactly 176 bytes at alignment 8: `epoch:u64` at byte 0, `hot:HotState` at byte 8, and `main:MainState` at byte 48; each HOT or MAIN drain bumps the shared epoch and returns a full copy, while `reference_shift_px` is applied once for the accepted `centre_revision` and generation.

`PresentHot` is the CPU-only adapter `pub struct PresentHot { pub epoch:u64, pub state:HotState, pub plane:Plane, pub view:ViewControls }` with no byte ABI; app constructs it from the newest infallible HOT drain, the math plane, and the current control values, and present evaluates nothing from a clock.

`PresentMain` is the CPU-only adapter `pub struct PresentMain { pub epoch:u64, pub state:MainState, pub grid:EscapeGrid }` with no byte ABI; app constructs it after an infallible MAIN drain and kernels publication, and present derives generation, delivered cap, palette, reference shift, and `c₀` compatibility from the worker-owned state without variants.

Math defines the immutable CPU-only `Pose` exactly as `pub struct Pose { pub epoch:u64, pub orbit_generation:u32, pub plane:Plane, pub plane_theta_1:f64, pub plane_theta_2:f64, pub zoom_log2:f64, pub view:ViewControls, pub grid_width:u32, pub grid_height:u32, pub centre_from_reference_px:[f64;2] }`; present stores and consumes it, every PLANE and VIEW angle is independent, and nothing in it is derived from another field.

`HotSlot` is the opaque CPU token `{index:u32,dynamic_offset:u32,epoch:u64}` returned by `HotSlot::for_refresh(refresh_id,slot_stride,epoch)`, where `index=refresh_id mod 3` and `dynamic_offset=index·slot_stride`; only this constructor can create a slot, making `write_hot` infallible.

`FrameState<'a>` is the CPU-only app input `{surface_view:&'a wgpu::TextureView, canvas_width:u32, canvas_height:u32, refresh_id:u64, now_ms:f64}`; dimensions are physical pixels, `now_ms` is the app's monotonic `performance.now()` sample, and the app keeps the surface texture alive after `frame` returns until `poll` reports the matching warp fence complete, then presents outside the measured region.

### 3.5 GPU uniform blocks

`HotUniform` is exactly 128 bytes in eight 16-byte lanes: byte 0 `camera:[f32;4]=[cos θ_c1,sin θ_c1,cos θ_c2,sin θ_c2]`, 16 `view_scale:[f32;4]=[h,d₅,d₄,0]`, 32 `view_rotation:[f32;4]=[cos θᵥ₁,sin θᵥ₁,cos θᵥ₂,sin θᵥ₂]`, 48 `homography_row_0:[f32;4]`, 64 `homography_row_1:[f32;4]`, 80 `homography_row_2:[f32;4]`, 96 `clear_rgba:[f32;4]`, and 112 `flags:[u32;4]=[epoch_low,epoch_high,source_valid,0]`.

The first two lanes previously carried `plane_u` and `plane_v`, which no shader read; the byte size, alignment, lane count, and every later offset are unchanged, so the reuse costs no ring arithmetic and the offset test moves two names rather than a layout. The fourth flag word previously carried the view discriminant and is now a reserved zero, which the same test asserts.

Each homography row stores three coefficients and a zero padding word, and the shader uses explicit row dot products rather than WGSL matrix layout; `source_valid` is one only when reference-shift rebasing, delivered-cap and plane-origin compatibility, dimensions, finite arithmetic, and matrix construction all pass.

The HOT buffer size is `3·slot_stride`, where `slot_stride = align_up(128,device.limits.min_uniform_buffer_offset_alignment)`; one bind group covers the whole buffer, each pass selects exactly one slot by dynamic offset, and a refresh writes exactly 128 payload bytes to that slot.

`SceneUniform` is exactly 80 bytes in five 16-byte lanes: byte 0 `grid:[u32;4]=[width,height,level,max_iter]`, 16 `span:[u32;4]=[directory_index,active_len,0,0]`, 32 `palette_map:[f32;4]`, 48 `interior_rgba:[f32;4]`, and 64 `clear_rgba:[f32;4]`, where `active_len=width·height` by checked arithmetic.

`SceneUniform` is rewritten only when MAIN selection, palette, level, extent, span, or iteration cap changes; index-buffer updates and texture/bind-group replacement are regional allocation events tied to an extent change and are never per-refresh work.

### 3.6 Scene, warp, events, and facts

`SceneFrame` is the presenter-owned CPU record `{scene_id:u64,pose:Pose,palette:PaletteId,iteration_cap:u32,level:RefinementLevel,extent:[u32;2],texture_index:u32,centre_revision:u32,plane_origin_f64:[f64;4],precision_mode:&'static str,measurement:SubmissionMeasurement}` with no byte ABI; `texture_index` is zero or one, and revision, origin, plus precision mode prove whether a reference shift can rebase it or a MAIN change must clear it.

`SubmissionMeasurement` is `{kind:SubmissionKind,id:u64,source_scene_id:Option<u64>,sample_class:SampleClass,precision_mode:&'static str,wall_ms:f64,fence_wait_ms:f64,polls:u32}`; `SubmissionKind` is `Scene` or `Warp`, `SampleClass` is `ColdWarmUp`, `PolicyProbe`, or `Measured`, milliseconds are measured monotonic walls, and `source_scene_id` is unavailable for a clear-only warp.

`WarpPlan` is `{rows:[[f32;4];3],source_valid:bool,kind:WarpKind,chart_residual:f64,approx_max_error_px:Option<f64>,approx_p95_error_px:Option<f64>}` with no byte ABI; `WarpKind` is `AnchorHomography` or `ClearOnly`. `PresentEvent` carries a documented `large_enum_variant` allow: a `Pose` now holds every control, so the completed-frame variant is 288 bytes, and boxing it would trade one fixed move for a heap allocation on every fenced scene completion.

`PresentEvent` messages are `SceneCompleted { frame:SceneFrame }`, `SceneDropped { scene_id:u64, orbit_generation:u32, reason:DropReason, measurement:SubmissionMeasurement }`, `WarpCompleted { measurement:SubmissionMeasurement }`, and `FenceRefused { kind:SubmissionKind,id:u64,reason:FenceRefusal,polls:u32,wall_ms:f64,precision_mode:&'static str }`.

`DropReason` is `IncompatibleMain`, `ReplacedMain`, or `InvalidExtent`; `FenceRefusal` is `PollLimit`, `Deadline`, `Device`, or `Cancelled`, and each variant is rendered verbatim by the app rather than collapsed into “slow.”

`PresentFacts` is `{completed_scene_id:Option<u64>,in_flight_scene_id:Option<u64>,source_generation:Option<u32>,precision_mode:&'static str,delivered_width:u32,delivered_height:u32,delivered_level:Option<RefinementLevel>,iteration_cap:Option<u32>,palette:PaletteId,view:ViewControls,centre_from_reference_px:[f64;2],reference_shift_px:[f64;2],last_scene:Option<SubmissionMeasurement>,last_warp:Option<SubmissionMeasurement>,reprojected_per_scene:Option<u32>,refreshes_without_scene:u64,texture_reallocations:u32,chart_residual:Option<f64>,warp_max_error_px:Option<f64>,warp_p95_error_px:Option<f64>,status:PresentStatus}`; `PresentFacts::record_warp_plan(&mut self,plan:&WarpPlan)` is the sole writer of those three planner-owned facts, so the residual, the sampled maximum, and the sampled ninety-fifth percentile always describe the same plan and a clear-only or skipped-corpus plan leaves both sampled errors absent rather than stale. The two error facts are named for the warp that owns them rather than for a view that no longer exists, and a validated height-zero plan publishes the measured zero it observed.

`PresentStatus` is `WaitingForFirstScene`, `ShowingCompletedScene`, `ShowingStaleApproximation`, `ClearForIncompatibleMain`, or `Refused(PresentError)`; app combines these delivered and measured facts with its own requested resolution, requested level, requested iteration cap, zoom digits, floor/working/delivered precision, orbit length, and rebase/glitch availability without substitution.

### 3.7 Callable API

`Presenter::new(device:Arc<wgpu::Device>,queue:Arc<wgpu::Queue>,heap:HeapPresentResources,config:PresentConfig)->Result<Presenter,PresentError>` allocates the 3-slot ring, two initially empty texture slots, fixed pipelines, immutable heap group, and two immutable warp groups as texture slots become allocated; it performs no device call before app has installed both error handlers.

`PresentConfig` is `{surface_format:wgpu::TextureFormat,min_uniform_buffer_offset_alignment:u32,fence_deadline_ms:f64,max_fence_polls:u32}` and v1 requires the app to pass the live alignment, `30_000.0`, and `4_096`; scene format remains fixed `Rgba8Unorm` independent of surface format.

`Presenter::set_main(&mut self,main:PresentMain)` is the infallible MAIN-drain endpoint: it records latest-wins state, applies a not-yet-consumed `reference_shift_px` to retained and in-flight poses for the accepted revision, and invalidates them when delivered `max_iter`, `plane_origin_f64`, or `precision_mode` changed, without allocating, submitting, waiting, or returning an error.

`Presenter::write_hot(&mut self,slot:HotSlot,hot:PresentHot,validation:WarpValidation)` is the infallible HOT-drain endpoint: it stores math's CPU `Pose` and the plan's source validity for that same slot, passes MAIN's `PrecisionMode` plus caller-owned validation reason into the caller-side planner entry, calls `math::warp_matrix` for the f64 anchor source and the planner for the four-anchor solve, writes exactly one 128-byte ring payload, and falls back to `source_valid=0` on invalid arithmetic.

`Presenter::submit_scene(&mut self,hot_slot:HotSlot,now_ms:f64)->Result<u64,PresentError>` captures current MAIN and the exact HOT pose, asks the scene ledger to construct the pending record and return its single authoritative texture index, prepares and encodes against that same index, submits a four-byte fence, and returns its monotonically increasing `scene_id` without waiting.

`Presenter::frame(&mut self,state:FrameState<'_>,hot_slot:HotSlot)->Result<FrameReceipt,PresentError>` is called once per surface refresh, encodes the sole warp pass to `state.surface_view`, samples the newest compatible completed scene or writes clear colour, submits its four-byte fence, and returns before completion; app retains the surface token, polls cooperatively for that `warp_id`, then presents outside the measurement.

`FrameReceipt` is `{refresh_id:u64,warp_id:u64,source_scene_id:Option<u64>,precision_mode:&'static str,status:PresentStatus}` with no byte ABI, reports `source_scene_id=None` when that slot's warp plan paints only clear even if a retained scene exists, and contains no fabricated wall because its fence has not completed.

`Presenter::poll(&mut self,now_ms:f64)->Vec<PresentEvent>` performs at most one shared `device.poll` per call to service both pending fences, increments each pending fence's own observation count once for that shared poll, promotes only a current completed scene, retires bounded failures, and never waits or yields internally; app's refresh loop supplies the browser yield between polls.

`Presenter::facts(&self)->PresentFacts` returns the latest immutable snapshot without polling, allocating, submitting, or draining owner state.

`Warp::reproject(last_frame:&SceneFrame,from_pose:&Pose,to_pose:&Pose,precision_mode:PrecisionMode,validation:WarpValidation)->WarpPlan` is the pure CPU planner, delegates the exact plane-chart matrix to math's `warp_matrix(from_pose,to_pose)`, solves the four anchors in f64, conditionally samples only at the caller entry rather than inside the anchor solve, validates compatibility and finite results, and never touches the GPU; `last_frame.pose` must equal `from_pose` or the result is `ClearOnly`.

`PresentError` is `InvalidGrid { width:u32,height:u32,logical_len:u32 }`, `SceneBusy { scene_id:u64 }`, `StaleSpan { directory_index:u32 }`, `UnsupportedSceneFormat`, `UnsupportedSurfaceFormat { format:wgpu::TextureFormat }`, `ExtentAllocation { width:u32,height:u32 }`, `IndexCountOverflow { width:u32,height:u32 }`, `Device { operation:&'static str }`, or `SurfaceTargetZero`; no variant panics and none mutates requested app controls.

### 3.8 Interface table for joint review

|Producer → consumer|Interface|Pinned payload or call|Units and byte ABI|
|-------------------|---------|----------------------|------------------|
|math → present|`Plane`|`basis_u@0`, `basis_v@16`|32 bytes; f32 ℝ⁴ coordinates|
|math → shallow kernel|`CentreSplit`|`hi@0`, `lo@16`|32 bytes; four f32 hi+lo pairs|
|math → present|`Pose`|`epoch,orbit_generation,plane,plane_theta_1,plane_theta_2,zoom_log2,view,grid_width,grid_height,centre_from_reference_px`|CPU-only math record; radians, log₂ zoom, pixels|
|math → kernels/present|`EscapeParams`|`max_iter`, `bailout=256.0`|8 bytes; iterations and squared radius|
|worker → app → kernels|reference record|`re,im`, then zero padding|8 bytes transferred, RGBA32F heap texel per iteration|
|kernels → present|escape record|`smooth_iter,escaped,rebase_count,glitch`|RGBA32F, 16 bytes per pixel|
|kernels → present|`RefinementLevel`|`Preview=0,Interactive=1,Final=2`|`repr(u32)` closed enum|
|kernels → present|`EscapeGrid`|`DataSpan,width,height,RefinementLevel`|CPU record; row-major from bottom; active prefix `width·height`|
|heap → present|`HeapPresentResources`|DATA view, descriptor UBO, span-directory UBO, three capacities|CPU ownership record; immutable resource identities|
|worker owner → app/present|`HotState`|`zoom_log2@0,plane_theta_1@8,plane_theta_2@16,centre_from_reference_px@24`|40 bytes, align 8; radians, log₂ zoom, current pixels|
|worker owner → app/present|`MainState`|generation/revision/caps/precision/orbit/palette IDs @0–31, centre @32, axes @64/68, plane origin @72, reference shift @104, mode @120|128 bytes, align 8; exact §3.4 layout|
|worker owner → app/present|`ViewerState`|`epoch@0,hot@8,main@48`|176 bytes, align 8; each drain bumps epoch|
|owner/app → present HOT|`PresentHot`|`epoch,state,plane,view`|CPU-only adapter; latest HOT drain plus the VIEW controls|
|owner/app → present MAIN|`PresentMain`|`epoch,state,grid`|CPU-only adapter; latest MAIN drain plus published grid|
|math → present/app|`ViewControls`|seven f64 VIEW controls; present re-exports it|CPU-only record; radians and distances|
|present → app|`PaletteId`,`PaletteRecord`|Classic/Ember/Ice IDs and exact map/interior/clear literals|`repr(u32)` ID; 48-byte linear-RGBA record|
|present → GPU|`HotUniform`|camera, height and distances, view rotation, three homography rows, clear, flags|128-byte payload at dynamic ring offset|
|present → GPU|`SceneUniform`|grid, span, palette map, interior, clear|80-byte regional MAIN payload|
|app → present|`PresentConfig`,`HotSlot`,`FrameState`|surface format and fence limits; ring offset token; borrowed surface view and refresh facts|CPU-only records; milliseconds and physical pixels|
|app ↔ present|callable API|`new,set_main,write_hot,submit_scene,frame,poll,facts,Warp::reproject`|Exact signatures in §3.7; drains infallible and fences asynchronous|
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

Honesty is structural: requested values remain app facts, delivered extent and iteration cap come from the current grid, `depth_digits=ceil(max(0,zoom_log2·log10(2)))`, `D_floor=max(1,ceil(zoom_log2·log10(2)+log10(grid_width))+8)`, `D_work=D_floor+ceil(log10(max(max_iter,1)))`, and the overlay keeps floor, working, and delivered precision separate.

Aggregate `rebase_count` and `glitch` totals are `unavailable` in normal rendering because present only gathers texels for rasterization; only an explicitly requested and labelled measurement readback may count them, every other unavailable measurement uses `Option`, warm-ups stay labelled, polls are counted, and no timeout path loops forever.

The app installs the panic hook and replaces wgpu's fatal uncaptured-error handler before `Presenter::new`; every present device operation returns or publishes a typed error, and no bare `unreachable`, unchecked surface acquisition, or panic is an error protocol.

Hand-written f64 remains the matrix implementation unless the shared warp oracle fails its `1e−9` bound; `faer` may enter only after that measured failure, never for style or anticipation.

Renderer austerity is one selected scene pass plus the sole warp pass, no mips, no blend, `cull_mode: None`, one sample, and two scene textures total; kernel fragment-compute passes are data production and do not loosen this render-pass count.

## 5. Oracles and tests

Native layout tests assert exact sizes and offsets for 32-byte `Plane`, 32-byte `CentreSplit`, 40-byte `HotState`, 128-byte `MainState`, 176-byte `ViewerState`, 8-byte `EscapeParams`, both RGBA32F records, 48-byte `PaletteRecord`, 128-byte `HotUniform`, and 80-byte `SceneUniform`, assert every reserved word is zero, and round-trip little-endian fixture bytes.

Native PLANE tests pin `R₁₃(θ₁)R₂₄(θ₂)` operation order, the zero-angle seed, the exact `(e₁,e₂)` seed at `θ₁=θ₂=−π/2` and its reversal at `+π/2`, nonzero z and c components for every angle strictly between, one f32 rounding pass, and the three `8·f32::EPSILON` orthonormal bounds.

Native heap-address tests build multipage `DataSpan` fixtures and prove index `j·width+i`, page quotient/remainder, descriptor decoding, bottom-row orientation, last-page padding, stale-handle rejection, and canonical-zero out-of-range behavior.

Native pixel tests use asymmetric 2-by-2 and 3-by-2 fixtures to prove centre sampling, `+v` up, row zero at bottom, nearest level resampling, exact interior classification, exact magenta glitch classification, malformed-flag tinting, and palette arithmetic within one f32 ulp of its scalar reference.

Native mesh tests prove vertex count `width·height`, index count `6(width−1)(height−1)`, the exact six-index cell order, bounds for every index, no triangles across rows, square-pixel q coordinates, neutral glitch height, and overflow refusal before allocation.

Native scene-algebra tests compare deterministic vertices at fixed control values with `ember_lab_heap::mode_a_endpoint` at zero lattice coordinate, including both perspective poles at slider distances, and parse generated WGSL to pin the control-driven camera, the wgpu depth row, `LessEqual`, lighting literals, no blend, no MSAA, and `cull_mode: None`.

One native test carries the height-zero claim: at `h=0`, zero VIEW angles, zero camera angles, and every distance in `{2,8,64}`, the four chart corners and a sampled interior lattice project to exactly the flat chart NDC `(q_u/2,aspect·q_v/2)` on a square and a 16:9 extent. It compares to `1e−12` across two extents, three distances, six chart points, and all five sampled record heights, since the height control flattens every one of them. It is the reason the fullscreen pass could be deleted rather than kept beside the mesh, so it fails loudly if the framing constant `k=aspect·d₄/2` is ever edited to something convenient.

The shared navigation-drift oracle composes `10⁴` and `10⁵` steps of `R(Δθ)` with `Δθ=1e−3` radians and metric `||MᵀM−I||_F`; pass is at most `1e−5` for f64 without re-orthonormalization and for f32 with Gram–Schmidt every 64 steps.

The flat warp oracle evaluates hand-written f64 forward and inverse matrices for `zoom_log2 ∈ {0,10,20,40,80,100}`, both presets, square and 16:9 extents, finite angle pairs, subpixel and thousand-pixel pans, and accepted-reference shifts, and requires `max|H⁻¹H−I|≤1e−9`; the uploaded f32 matrix separately must map test pixels within `0.25` source pixel where representable.

Native reference-shift fixtures construct one physical centre before and after acceptance, apply `d_f←d_f−r_a_fB_fᵀB_a·reference_shift_px` exactly once, and require identical flat source coordinates; changed `max_iter` or `plane_origin_f64` must clear, while generation change alone must retain and rebase both completed and in-flight poses.

Native zoom-interface tests decompose scales on both sides of `zoom_log2=14`, prove present never forms the deep absolute f32 scale, and pin the displayed shallow/perturbation POLICY transition while leaving the `EscapeGrid` and warp-ratio interfaces unchanged.

Native anchor-warp tests pin the four neutral-height anchors exactly, exercise pivot refusal, report the 9-by-9-by-5 maximum and p95 errors, and require the §2.6 `8.0`-pixel bound at the single `θᵥ₁=0.6` relief fixture, which measures `7.704`; failures outside the acceptance envelope are reported facts, not test failures. A counted policy test requires twelve ordinary PictureFast refreshes to execute the corpus zero times and one Measure to execute it exactly once, while the uploaded-row quarter-pixel oracle remains an ordinary PictureFast test.

A second anchor test pins the retirement of the exact plan: at zero camera angles and `h=0`, for every distance in `{2,8,64}`, the solved four-anchor rows equal math's `warp_matrix` forward within `1e−6` per f32-packed coefficient and the sampled corpus reports below `1e−9`, which is the evidence that deleting `WarpKind::FlatExact` deleted a code path and not a capability.

Exact planner word and residual comparisons are guarded by the cfg-free `PrecisionMode::requires_bit_identity` helper and are labelled Deterministic conformance; finite plans, poles, error bounds, and every exact-path accuracy oracle execute for PictureFast as well.

A native oracle sweeps 256 VIEW angles across the full turn for the Mandelbrot row, the Julia row, and the hybrid plane at `θ₁=θ₂=π/4`, and requires every swept plan to be an anchor homography with no clear-only fallback and within the §2.6 measured swept bounds of `16.0` pixels for the full envelope and `4.0` for rotation and pan; a companion test requires the plan to be independent of which ambient axes the plane names, to binary32 basis tolerance, which is the property §2.3 argues for and the property whose absence collapsed the Mandelbrot height field.

Native state-machine tests permute scene completion, HOT writes, MAIN replacement, accepted-reference shift, incompatible cap, plane origin, or precision mode, control movement, resize, warp completion, deadline, and poll-limit events and prove exactly one retained plus one in-flight texture, latest-wins promotion, exactly-once pose rebasing, no third allocation, bounded retirement, and correct `reprojected_per_scene` attribution.

Native page-contract tests pin the eight callable entries `new`, `set_main`, `write_hot`, `submit_scene`, `frame`, `poll`, `facts`, and `Warp::reproject`, the exact initial overlay phrase, required facts fields, requested-versus-delivered separation, unavailable aggregate counts, all three sample labels, refresh order, app surface ownership through warp completion, and panic/error setup before `Presenter::new`.

Browser initialization, GL backend identity, `EXT_color_buffer_float`, vertex-stage DATA access, actual surface format, exact scratch-copy visibility, and absence of validation or console errors are `requires visible replay`.

Image orientation at height zero and under relief, the continuity of the morph as each control moves, pole clipping, magenta glitch isolation under scene and warp movement, clear disocclusion, clear first frame, refinement resizing, and rapid HOT motion while a scene is in flight are `requires visible replay`.

Warp cost per refresh, scene-frame cost, fence wait, polls, warm-up exclusion, frames reprojected per scene, texture reallocations, and direct-versus-warp pixel error under relief on the target browser are `requires visible replay`; no native estimate may populate those overlay fields.

## 6. Risks and retirement oracles

|Risk|Consequence|Oracle that retires it|
|----|-----------|----------------------|
|Heap shader sees the wrong page or row|wrong fractal pixels|native multipage address fixture plus visible scratch-copy grid replay|
|A glitch flag is interpolated or filtered|hidden numerical failure|asymmetric native tint fixture plus moving visible replay with nearest warp|
|PLANE and VIEW rotations are conflated|wrong slice or animation|native preset matrices and heap `mode_a_endpoint` comparison|
|Deep perturbation enters the GPU as an absolute centre or tiny scalar scale|precision collapse|perturbation-layout fixture proving no centre field and mantissa/exponent scale decomposition|
|Flat warp reverses zoom, rows, or aspect|swimming or mirrored image|analytic pixel correspondences and the `H⁻¹H` oracle at all six zooms|
|A reference shift has the wrong sign, units, or revision|old pixels swim during a valid deep pan|physical-centre invariance oracle plus exactly-once completed/in-flight rebasing test|
|The anchor 2D warp exceeds its useful envelope|visible nonlinear swimming|9-by-9-by-5 oracle rejected the argued `0.02`/`0.25` envelope and pins the narrower `0.002`/`0.025` boundary at the measured `8.0` pixels for the relief fixture and `16.0` swept, plus labelled visible direct-versus-warp replay|
|Warp clamps an exposed edge|smeared disocclusion|UV-outside unit test and visible clear-border replay|
|An internal relief disocclusion is mistaken for corrected|overstated capability|explicit status/overlay contract and visible stress replay; it remains an accepted limit|
|A scene target is overwritten while sampled|race or validation error|all state-machine interleavings plus rapid visible refinement replay|
|HOT slot reuse races queued work|pose tearing|queue-order test with tagged slots and rapid visible HOT replay|
|A fence stalls forever|hung page|4,096-poll and 30,000-ms refusal tests plus suspension replay|
|Warm-up becomes a performance claim|misleading timing|measurement reset/label tests and visible sample ledger|
|Scene and warp walls include different work|invalid comparison|submission-order page-contract test and visible fence ledger|
|Rawgl presentation drifts with all in-page paths|self-consistent wrong image|native pinned-literal parser against heap §10 plus side-by-side visible replay|
|Extent churn dominates refinement|avoidable allocation wall|reported `texture_reallocations`, allocation walls, and visible level walk|
|An error handler is installed too late|unreadable wasm trap|app page-contract ordering test and deliberate visible validation refusal|

## 7. Implementation phases and line budget

Phase 0A is the dependency-independent subset now implemented: the package shell, exact palette records and scalar oracle, 128-byte HOT and 80-byte scene layouts, checked three-slot ring arithmetic, and finite homography solver and packer; it intentionally defines no substitute for math-owned `ViewControls`, `Plane`, `Pose`, or `warp_matrix`.

Phase 3A now also implements checked mesh index and coordinate construction, present's VIEW coefficients, and an oracle bridge to heap's exported `mode_a_endpoint` without constructing any heap resource or math-owned type.

Phase 2/3 shader work now implements the heap-capacity-specialized height-field scene WGSL, dense-prefix handle resolution, bottom-row addressing, honest record shading, double-perspective pole rejection, the control-driven camera, and derivative lighting without constructing the pending heap seam.

Phase 4A now implements and validates the sole fullscreen warp WGSL, including source-valid clear, f32 pole rejection, out-of-bounds clear disocclusion, nearest-sampler compatibility, and no mip-level path; the f64 plan still awaits math's `warp_matrix`.

Phase 0 adds the present package shell, shared records, byte-layout assertions, palette scalar reference, consumption of the app-lane `HeapPresentResources` seam, and pure f64 homography/oracle code, estimated at 360 new Rust and test lines.

Phase 1 is implemented by the two-texture state ledger, 3-slot dynamic HOT ring, 80-byte MAIN block, bounded four-byte fence polling, typed events, and native interleaving tests, estimated at 480 lines.

Phase 2 is implemented by the flat fullscreen scene pipeline, heap-capacity descriptor/span accessor generation, palette and honest glitch shading, and target-resize handling; browser image facts remain visible replay, estimated at 340 Rust/WGSL/test lines.

Phase 3 is implemented by height-field grid/index generation, five-dimensional vertex algebra conformance through `mode_a_endpoint`, camera/depth/lighting, nearest fragment classification, and mesh/pole tests, estimated at 520 lines.

Phase 4 is implemented by math's exact plane-chart matrix, the f64 four-anchor planner and 9-by-9-by-5 error corpus, the one fullscreen warp pipeline, clear disocclusions, and the no-frame path, estimated at 440 lines.

Phase 5 is implemented by the exact app-facing API, facts snapshots, warm-up and reproject counting, callable-surface tests, visible-replay labels, release reconciliation, and documentation updates, estimated at 360 lines.

The implementation estimate is 2,500 net new lines across Rust, WGSL, tests, manifests, and present-owned documentation; only the app lane may make the J16 heap seam edits, so no heap edit is hidden inside this present estimate.

## 8. Unresolved joint-review findings

- `HeapPresentResources` is now published and sufficient for immutable presentation bindings, but the GL backend's validation of the independently reconstructed three-binding layout remains `requires visible replay` because the seam deliberately exposes resources rather than the private heap bind-group layout object.

- The present `EscapeGrid` contract embeds a cloneable `DataSpan`; kernels must confirm the exact lifetime handoff that prevents freeing a span still named by a retained or in-flight scene.

- The exact browser behavior of sampling an `Rgba8Unorm` scene texture whose extent differs from the surface is unmeasured on the GL backend, including whether nearest warp is visually acceptable at coarse refinement levels.

- The `8.0`-pixel anchor approximation envelope is a native measurement, not field evidence; a visible direct-versus-warp replay may narrow the allowed motion or reject the approximation while leaving the one-pass interface intact.

- A homography exposes only external borders, so internal relief disocclusion remains stale until a later depth-aware design; this round intentionally has no honest one-pass repair for it.

- The height normalization maps `max_iter` to a fixed `[-2,2]` range, which makes levels comparable but may visually compress interesting low-iteration structure; palette tuning cannot answer that geometry question.

- The three v1 palettes and clear colours are interface-stable literals but have not had accessibility or target-display review; changing them after implementation would require a documented palette-version change.

- Two immutable warp bind groups are rebuilt when their corresponding texture slot changes extent; this obeys the heap identity law but the allocation/rebuild cost across rapid progressive levels is unknown until visible replay.

- `performance.now()` callback observation can be delayed by browser suspension beyond the nominal deadline; the implementation can bound active polls and reject upon resumption but cannot promise wall-clock wake-up while JavaScript is not scheduled.

- The page overlay must remain outside the measured render regions and cannot consume another pass; this document assumes DOM text, while the integration implementation still has to confirm that the app's chosen page mechanism preserves that ordering.

- A reference shift is expressed in the newly accepted current basis; when the retained basis differs, projection into the retained basis has the same chart residual already reported for PLANE motion, and visible replay must establish whether that warning remains usable during simultaneous deep pan and rotation.

- The anchor budget is not satisfied across VIEW angle, and the control-driven observer made the shortfall more visible rather than less: the swept worst case is now `15.650` pixels for the full envelope against `3.094` under the retired mount, purely because the height-zero framing is the chart map and the picture fills the frame. Either the zoom step tightens until the sweep meets a bound worth publishing, or the published bound stays at the measured `16.0`; that is a schedule decision the app acts on, so present reports the measurement rather than changing the policy unilaterally.

- The acceptance envelope has more terms than the sweep that produced it: the `0.002` rad bound is now asserted for each of the four VIEW and camera angles by analogy with the single swept angle, and no sweep exists for a moving height or a moving perspective distance. Either the sweep grows those axes or the published envelope names only the angle it actually measured.

- `PresentFacts::warp_p95_error_px` is published, but the app's page-facts mirror still hardcodes `None` at `crates/labs/julibrot/app/src/facts.rs`; that one-line read belongs to the app package and is the remaining half of the overlay reading `unavailable` for a value present has always computed.

- App §3.6 still restates an older flattened `PresentHot`/`PresentMain`, `warm_up:bool`, and generation-named clear status, while J14 assigns this API to present and present §3.4/§3.6 pins worker-owned state wrappers, `SampleClass`, and `ClearForIncompatibleMain`; implementation follows the owning present contract and app must reconcile its duplicate before consuming the package.

## 9. Refinement evidence

Semantic contract commit `df5280f959bfa67d4359110a6f66c33a8af6d85a` was checked out exactly on barza and passed `cargo test -p linter -- --skip the_repository_snapshot_loads_as_one_exact_surface --skip the_repository_owner_surface_reconciles_in_both_directions`: 727 library tests and 39 corpus tests passed, both named repository tests were filtered, exit was 0, `RUN-REPORT` wall was 0.8 seconds, and SSH-observed wall was 1.80 seconds.

The same semantic contract commit passed `cargo-fmt --all -- --check` on barza with exit 0, `RUN-REPORT` wall 1.3 seconds, and SSH-observed wall 2.51 seconds.

Before the semantic commit, local non-toolchain checks found only `docs/julibrot/present.md` changed, `git diff --check` passed, the banned token and superseded field names were absent, line endings were LF, and no UTF-8 BOM was present.

## 10. Implementation evidence

Implementation head `66fb25e093d73982f9cab2d92b5395a828e97974` was checked out exactly on barza and passed the required nine gates: workspace build `2.9 s`, workspace clippy with warnings denied `1.8 s`, cargo-fmt check `9.5 s`, workspace tests excluding linter `56.6 s`, linter tests with the two repository checks skipped `4.7 s`, wasm library checks for arena `1.8 s`, what-is-this `0.5 s`, fire `1.3 s`, and heap plus present `2.3 s`; each value is the corresponding `RUN-REPORT` wall and every exit was zero.

The present package contributes 36 unit tests and two integration tests covering exact layouts, palette honesty, heap-specialized WGSL validation, mesh order and heap algebra, two-slot state transitions, ledger-authored target identity, clear-only source attribution, opaque HOT offsets, replacement disposition, exactly-once reference rebasing, bounded fence outcomes, all six required deep-zoom warp rows, the corrected display-chart conversion, the 9-by-9-by-5 anchor corpus, app-facing signatures, and warp-completion identity.

The final handoff audit additionally proves that synchronous `submit_scene` and `frame` refusals enter `PresentFacts.status`, all fallible scene preparation completes before the ledger reserves the in-flight slot, and palette, view, or grid replacement marks a pending scene `ReplacedMain` while retaining the last completed texture.

The native oracle rejected the pre-implementation anchor envelope for `Δθ_view=0.01` and `Δzoom_log2=0.1`; the implementation contract therefore narrows the accepted tested fixture to `0.002` and `0.025` at no more than the measured `8.0` pixels instead of converting a failed risk oracle into a claim.

Barza establishes native and wasm compilation, byte/layout tests, CPU arithmetic, WGSL parse/validation, and bounded state transitions, but it cannot establish GL surface behavior, actual browser fence scheduling, visual orientation, disocclusion quality, console silence, or measured scene/warp costs; every such fact remains `requires visible replay`.
