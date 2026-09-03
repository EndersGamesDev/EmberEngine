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

The object pose is the math-owned `O∈SO(4)` with six ordered angles, applied to the one seed as `u=Oe₃`, `v=Oe₄`; its four-coordinate plane origin is the translation part of the affine object pose. The legacy two-angle adapter occupies `ρ₁₃` and `ρ₂₄`, and all published basis components undergo one shared f32 rounding pass.

No projection or Gram–Schmidt stage enters plane construction because `O` preserves the orthonormal seed; the postcondition remains `|u·u−1|`, `|v·v−1|`, and `|u·v|` each at most `8·f32::EPSILON`.

Mandelbrot uses `O=I` and the camera-facing factors `q₁₃=q₂₄=−π/2`; Julia uses object `ρ₁₃=ρ₂₄=−π/2` and `Q=I`. Both use zero camera translation and are exact identity pictures at height zero.

The camera pose is the independent math-owned `Q∈SO(5)` with ten ordered angles followed by translation `t∈ℝ⁵`; both are frozen into the HOT slot used by one scene or warp submission. Present reads no clock: monotonic time times fences, but no geometric term depends on it.

Grid sample `(i,j)` is screen-aligned: its centred screen pixel is mapped through the scene's `M`, yielding plane offset `(o_u,o_v)` about the accepted centre. `+v` is up, row zero is bottom, record index is `j·width+i`, and kernels and scene consume the same packed map.

The exact CPU scale remains `pixel_scale = 4/(2^zoom_log2·grid_width)` in f64, but deep dispatch decomposes it as `m·2^s`; the GPU forms only `(o_u u+o_v v)·m` at exponent `s`, so no absolute tiny f32 scale is formed.

The perturbation kernel is selected at `zoom_log2≥14`, a displayed POLICY, while shallower zoom uses the 144-byte shallow kernel and can accept navigation without an orbit; switching kernels does not change the `EscapeGrid` interface, and present never derives a scalar GPU scale from zoom.

The escape DATA texel is loaded with integer `textureLoad` through the `DataSpan` directory and descriptor table; no float sampler, interpolation, CPU readback, or re-packed presentation copy lies between the paid kernel output and the scene shader.

### 2.2 One scene pass and the height-zero image

The scene pass draws the screen-grid height field into an LDR `Rgba8Unorm` target over a sky clear set to the palette's exterior colour at zero smooth iterations. At height zero every mesh vertex is its own screen position by construction, while the background guarantees that perspective-pole clipping, near-edge-on relief, and border pull-in cannot leave black or clear holes after scene completion.

The scene target extent equals the delivered `EscapeGrid` extent, so a refinement level is both the delivered escape resolution and the delivered scene resolution; a changed extent is an allocation event, not a per-frame write, and is counted in `texture_reallocations`.

For an escaped non-glitch record, `hue = fract(smooth_iter/period + phase)`, `phase_rgb = clamp(abs(fract(hue + (0,2/3,1/3))·6−3)−1,0,1)`, and `rgb = value·mix((1,1,1),phase_rgb,colour_mix)`; an interior record uses `interior_rgba` exactly.

A status-2 Horizon record is shaded as exterior at zero smooth iterations, not clear; status-3 MapUncertain is shaded from its sampled record. Only a warp coordinate outside its retained source uses the palette's honest clear colour temporarily.

The shader tests `glitch == 1` before `escaped`, emits the fixed opaque debug tint `(1,0,1,1)`, and never filters that classification; malformed non-binary `escaped` or `glitch` is also debug tinted and counted as a presentation contract violation.

### 2.3 Height field, VIEW, camera, and projection

The scene uses one indexed triangle-list mesh with `width·height` vertices and `6·(width−1)·(height−1)` `u32` indices; for cell lower-left `a = j·width+i`, `b=a+1`, `c=a+width`, and `d=c+1`, the exact index sequence is `[a,b,c,b,d,c]`, although culling remains disabled.

The neutral-height screen coordinate comes directly from `(i,j)`. For relief, the vertex maps that screen pixel through the scene's `M`, forms the ambient object point `p=plane_origin+(4/width)(o_u u+o_v v)`, lifts it to `(p,hH)∈ℝ⁵`, and projects that point through the same camera chain math used to build `F`; therefore height zero returns to the starting screen coordinate by construction.

For a valid escaped sample the record's own height is `H = 4·clamp(smooth_iter/max(max_iter,1),0,1)−2`; an interior sample uses `H = −2`, and a glitch or malformed sample uses neutral `H = 0` plus the debug tint so the geometry does not pretend to know the missing orbit continuation.

The displayed fifth coordinate is `h₅ = h·H` for the height control `h ∈ [0,4]`, so `h=0` is exactly the flat chart, `h=1` is the amplitude the lab shipped with, and every value between is a continuous morph rather than a switch; the range extends to four because the relief is a display choice and there is no reason to forbid exaggerating it, while `h<0` is refused because it would silently invert interior and escaped and is reachable anyway by a half turn of `θᵥ₂`.

The lifted ambient point first receives the frozen ten-factor camera rotation `Q`, then camera translation `t`, then the double perspective `P₅(p)=d₅/(d₅−p₅)·(p₁,p₂,p₃,p₄)` and `P₄(y)=d₄/(d₄−y₄)·(y₁,y₂,y₃)`, followed by the yaw/pitch observer and clip transform.

The retired chart-display frame replaced the ambient point by `(q_u,q_v,0,0,hH)`, making the old two-angle view visible but turning the picture about its own centre rather than moving the observer in ℝ⁵. The still earlier fixed two-angle ambient rotation collapsed `span(e₃,e₄)` because its rotations preserved the subspace with zero display axes. The correct cure is neither chart coordinates nor a fixed mount: the general independent `Q`, with preset rows chosen to face their slices, preserves the real ambient object and allows physical edge-on views.

Both perspective stages remain physically active because `O`, `Q`, `t`, and relief can place a component on either projected axis; both denominators are tested before division.

The 3D observer continues to use the same `d₄` as observer distance, with near `0.1`, far `4·d₄`, and perspective scale `aspect·d₄/2`; the generated WGSL and CPU mirror consume these quantities in that order.

The 288-byte HOT payload carries the ten `Q` sine/cosine pairs, five translations, observer, view scale, inverse-sampling warp, current `M`, exterior and clear colours, and flags. The 160-byte scene payload carries the sampled basis and map, so the scene shader reconstructs the same ambient point the kernels sampled.

Either perspective denominator at or below `ε = 1e−4` invalidates the vertex and clips its incident triangles by emitting the fixed outside-clip position; denominators are tested before division so a pole never becomes a NaN convention, and the exterior sky remains behind every discarded fragment.

The vertex also emits a numeric validity value and the fragment discards any interpolation below one, so every triangle incident to an invalid vertex is rejected rather than relying on the fixed outside-clip position alone.

The implementation depends on `ember-lab-heap` and reuses the exact exported pure CPU oracle `mode_a_endpoint(base:[f64;5],coordinate:[i32;5],frame:&FrameUniform)->ModeAEndpoint` with zero lattice coordinate and a `FrameUniform` carrying `[cos θᵥ₁,sin θᵥ₁,cos θᵥ₂,sin θᵥ₂]`, poles `[d₅,d₄]`, and epsilon `1e−4`; the present WGSL operation order is tested against that function rather than copied into a second Rust oracle.

The indexed-grid construction follows the heap slice's pure-data mesh pattern but not its clock-derived frame: all ten `Q` angles and five translations are independent controls. The two pole constants survive as neutral `d₅` and `d₄`, while this two-dimensional slice enters standard ℝ⁵ only through its true ambient basis and relief lift.

After double perspective the three-space observer is two more controls, not a fixed mount: yaw `θ_c1` and pitch `θ_c2` in `[−π,π]`, observer distance `d₄`, near `0.1`, and far `4·d₄`.

Writing `cy=cos θ_c1`, `sy=sin θ_c1`, `cp=cos θ_c2`, and `sp=sin θ_c2`, for world point `(x,y,z)` the camera evaluates `yawed = (cy·x+sy·z,y,−sy·x+cy·z)`, `view = (yawed.x,cp·yawed.y−sp·yawed.z,sp·yawed.y+cp·yawed.z−d₄)`, and clip position `(k·view.x/aspect,k·view.y,(far/(near−far))·view.z+far·near/(near−far),−view.z)` with perspective scale `k = aspect·d₄/2`.

That one choice of `k` is what makes the height-zero image exact. At `z=0` the perspective divide is by `d₄`, the two cancel, and NDC is `(x/2,aspect·y/2)`, which is the §2.1 chart map for every `d₄` and every extent. So `d₄` sets how strongly depth foreshortens and never reframes the height-zero chart, which is what lets it be an honest perspective control instead of a disguised zoom.

The retired mount was a fixed 20-degree yaw, 15-degree pitch, distance `9`, scale `1.72` camera inherited from rawgl. It cannot be kept. It is a view degree of freedom hard-coded into the pipeline, which is exactly what the control model abolishes, and its framing differs from the chart map by the aspect-dependent factor `2·1.72/(9·aspect)`, so under it no height-zero image is the flat image and the two pictures could never be reconciled. Its two angles return as the yaw and pitch controls, where a preset that names them recovers the inherited look as a row of numbers that a user can leave, and their neutral value is zero because the flat chart is the picture the lab opens on.

The depth expression is rawgl's OpenGL projection converted to wgpu's zero-to-one depth range, maps view z `−0.1` to zero and `−30` to one, and uses `LessEqual`; the pipeline is one-sample, has no blend, no mipmaps, and `cull_mode: None`.

The scene fragment obtains a surface normal from derivatives of the interpolated double-projected world position, uses fallback `(0,0,1)` when the derivative cross product is degenerate, and applies the heap-pinned light `0.58 + 0.24·|n·normalize(0.4,0.7,0.6)|` and colour `mix(white,hue_rgb,colour_mix)·value·light`.

The fragment also performs a nearest integer escape-record load from the interpolated grid coordinate and branches on its exact glitch flag, so the debug tint is not interpolated across neighbouring vertices; rawgl's `0.013` long-box thickness is explicitly inapplicable to a triangle height field and is the only §10 heap presentation literal not used.

All six object rotations, ten camera rotations, five camera translations, observer angles, height, and both distances are HOT presentation controls; the four plane-origin coordinates remain MAIN sampling state because moving the origin selects a translated affine slice and may need a new reference orbit.

### 2.4 Scene texture pair and submission

Exactly two single-sample, one-mip `Rgba8Unorm` scene textures exist: one is the best compatible fence-completed texture sampled by warp and the other is the sole in-flight scene target; before the first completion there is no retained texture and the non-target texture has no semantic content.

A scene submission captures reconciled `PresentMain` plus the referenced HOT slot into math's immutable `Pose`, including object angles, object origin, twenty view scalars, and the level's map; that captured sampled pose remains the sole basis for later reference-shift rebasing.

While the scene fence is pending, every surface refresh may submit a warp against the retained texture. After the fence callback, `Presenter::poll` promotes the completed texture, pose, palette, grid extent, and measurement unless the retained higher-level or larger-extent scene still has an accepted, non-exposed warp for the latest HOT pose; in that case the draft completion advances the app schedule without replacing the retained source and its texture remains the next target. A Final completion replaces the older Final as usual.

If a new level has a different extent, only the available target is reallocated before scene submission, its immutable warp bind group is rebuilt once and the allocation count advances; the retained texture and its bind group stay valid until promotion, keeping the total at two textures.

Scene requests while a target is already in flight return `PresentError::SceneBusy` instead of allocating a third texture, blocking, or overwriting work; an accepted reference shift rebases each retained or pending scene from its own sampled pose, never from `max_by_key(epoch)` or a newer HOT pose.

### 2.5 Exact plane-chart homography

Let `M_p` be pose `p`'s accepted screen-to-plane map and `B_p=[u_p v_p]` its once-rounded basis. A screen point first passes through `M_p`; its reference-relative ambient point then follows from `B_p`, pixel scale, centre displacement, and plane origin without materializing an absolute deep GPU coordinate.

For current destination pose `t` and retained source pose `f`, math composes `H(f→t)=M_t⁻¹·T·M_f`; `T` contains basis overlap, scale ratio, centre displacement, and compatible in-plane origin translation. Present uploads the explicit inverse `H(t→f)` for texture sampling.

The translation term is the desired-centre displacement difference in retained-frame pixels; when the bases agree its components reduce to `b_x=2·(r·d_t.x−d_f.x)/width_f` and `b_y=2·(r·d_t.y−d_f.y)/height_f`, so pans remain smooth without an absolute centre or absolute deep scale.

The shader evaluates rows explicitly as `r = H_(t→f)·(x,y,1)`, rejects non-finite `r` or `|r.z|≤1e−12`, computes source NDC `s=(r.x/r.z,r.y/r.z)`, converts to source UV `(s+1)/2`, and emits clear colour rather than clamping whenever either UV component lies outside `[0,1]`.

This is exact at height zero when both poses sample the same affine slice. Six object angles must agree to the f32 plane-rounding floor; a plane-origin delta may pass only when its out-of-plane component and the normalized chart residual are each at most half a source pixel, so in-plane origin motion is exact pan and out-of-plane motion is a slice change.

On an accepted reference, worker publishes `reference_shift_px` as new minus old reference centre in current-zoom pixels along accepted basis `B_a`; present computes only `r_a_f=s_a/s_f=2^(zoom_f−zoom_a)·width_f/width_a` and re-expresses every retained or in-flight pose as `d_f←d_f−r_a_fB_fᵀB_a·reference_shift_px`, then advances its reference generation without clearing the image.

The absolute bignum centre never enters this matrix: `centre_from_reference_px` and `reference_shift_px` remain f64-safe at arbitrary depth. A reference-generation change alone does not clear, while cap, precision mode, object-slice, or out-of-plane origin incompatibility refuses the warp.

A pan translates old content and exposes a border on the entering side; every mapped source coordinate outside `[0,1]` shows `clear_rgba` with no clamp, stretch, wrap, or stale edge pixel, while the next completed scene supplies the newly revealed fractal samples.

All powers, dot products, the 2-by-2 inverse, and the 3-by-3 forward/inverse check are evaluated by hand-written `f64` CPU code; only the final three padded rows are rounded to f32 for `HotUniform`.

### 2.6 Anchor warp, and why it is the only plan

Warp is deliberately depth-free and uses one 2D homography of the already presented image, for every view, because adding depth, a coarse displaced warp mesh, or a second warp pass would answer a different lab question and violate the one-extra-pass constraint.

The four destination anchors are the current screen corners. Each passes through the current screen map, the compatible plane-chart map, and the retained forward screen map; the solve therefore composes the same maps used by screen-aligned sampling.

The planner and scene shader use the identical ambient construction. At neutral height the composed map is exact for the compatible affine plane; under relief the sampled corpus measures the nonlinear error that one 2D homography cannot follow.

`WarpKind` is `AnchorHomography`, `ClearOnly`, or `ReliefRedraw`. Flat compatible plans are exact; a missing source, incompatible slice, edge-on map, source-identity mismatch, failed solve, or an unmeasurable corpus is `ClearOnly` and is never shown as a moving feature. A plan whose corpus was measured and came out above the ceiling is `ReliefRedraw`, described in 2.7: the picture it presents today is the same honest clear, but the retained records still describe the destination and the refusal names why the image could not be carried.

The implementation solves the eight projective coefficients of the current-NDC-to-source-NDC homography with f64 Gaussian elimination and partial pivoting, fixes `h₂₂=1`, refuses a pivot below `1e−12`, and rounds the valid result to the same three-row HOT layout used by flat warp.

Every plan, including ordinary PictureFast, carries a measured maximum error and p95 from the same full 9-by-9-by-5 corpus. A sample with no finite destination, retained projection, or warp image makes the plan unbounded and therefore `ClearOnly`; honest texture-edge disocclusion remains the separately flagged temporary clear region that the next scene fills. The uploaded f32 rows retain their separate quarter-source-pixel accuracy oracle.

The `h=0` slice of this admission corpus is structurally zero because the height-zero projection short-circuit returns each screen point directly; even a corrupted sampling matrix can therefore publish `approx_max_error_px=0.0` when `height_scale=0`. Flat exactness rests on the `warp_matrix` algebra and its full-forward-chain tests plus the unconditional quarter-source-pixel uploaded-row oracle, not on the corpus.

The named acceptance ceiling is `WARP_MAX_ERROR_PX=1.0`: a resolving scene may fill missing detail but no displayed reprojection may move a feature by more than one pixel. A plan above the ceiling is refused and temporarily clear until the due scene completes.

At 1920 by 1080 with `height_scale=1`, measured admission thresholds mean most camera planes clear beyond roughly `0.001` to `0.02` radians; yaw/pitch reaches the ceiling near `0.0033`, height near `0.0027`, and `d₅` motion near `0.018`, while `q₁₄` near `0.067` and `q₂₄` near `0.091` tolerate a few degrees. Relief navigation is therefore clear-then-fill except for sub-degree nudges; the owner-facing levers are a higher `WARP_MAX_ERROR_PX` or a relief-aware non-homography warp.

The measured 1920-by-1080 ambient relief sweep reaches `46.94` pixels over the full envelope and `31.59` pixels for rotation with pan. These numbers describe what relief reprojection can reach, not what is accepted: they explain why relief warps outside small motions are refused under the one-pixel ceiling.

The corpus is measured where there is something to measure. A lattice sample beyond either pose's screen-map horizon is skipped rather than refusing the whole plan: the scene pass leaves the exterior sky there and the warp pass carries that sky across, so the sample has no reprojection error to report. Anything else that fails to project — a perspective pole on the sampled relief surface — still refuses, and a plan with nothing measurable at all is refused too. Skipping is what makes a horizon a horizon rather than a wall: refusing instead left every pose whose horizon crossed the frame permanently unpainted, because the exact plan a settled pose warps onto itself was refused along with the rest.

Exposure is measured against the retained image, which reaches half a texel past its outermost sample centres. That reach is not cosmetic. A pose composed onto itself is the identity only as closely as the f32 plane basis it was built from allows: at `o₁₃=1.5` the frame's own border lands `2.1e-5` pixels outside itself, two orders of magnitude inside the half-texel footprint and three orders below any disocclusion a moved view produces. Without the footprint that rounding read as a disocclusion, and because an exposed warp latches the exposure that restarts the refinement ladder, the ladder restarted for as long as the pose was held.

Newly exposed source coordinates outside the retained texture show `clear_rgba` and set the exposure latch; the next completed scene fills them over the exterior sky. The plan carries the exact `(scene_id,texture_index)` it was solved against, and draw clears if the retained source differs, preventing a scene promoted between HOT write and frame from sampling a different texture with stale rows.

### 2.7 Relief redraw

Reprojection is a navigation primitive and has to cover every viewpoint degree of freedom, but two of the observer bars are not viewpoint degrees of freedom at all. The escape height enters the projection on the fifth ambient axis, so the height amplitude and the fifth-space distance — together with the four camera factors that turn that axis into the chart, and the fifth translation — decide where a record of a given escape height sits. Under them each retained pixel moves by an amount proportional to its own escape height, and no map of the image can express a per-pixel displacement. Everything else — yaw, pitch, `d₄`, the six chart-only camera factors, pan, zoom — acts on the point after the lift and is carried exactly by the one image homography at any height.

That is why the screen map is bit-identical across height amplitudes, pinned by `the_screen_map_is_independent_of_the_height_amplitude` in `math/src/screen.rs`: the height never reaches the map the warp is fitted from. Measured on the deployed page at 960 by 540, a height step of `0.005` from a settled flat scene is the whole motion the one-pixel ceiling admits, at `0.689` px; `0 → 1` measures `183.58` px and `1 → 0.5` measures `104.90` px, and `d₅ 8 → 6` at height one measures `91.79` px. Each is an honest refusal of an unbounded warp, and each leaves the surface cleared until the next scene completes.

The retained records are the way out. A plan only reaches the ceiling test once the object samples match and the plane-chart residual is inside its limit, which is to say the retained escape grid already describes the destination's chart. Redrawing that grid through the existing screen-aligned scene mesh under the new pose — same records, same `record_height`, the new `view_scale` and camera — places every retained sample exactly where a freshly computed scene at that pose would place it, with no kernel dispatch and no new sampling. Where the relief moves to hide a region the result is an honest disocclusion that the next completed scene fills, which is the behaviour the surface already has for texture-edge exposure.

The redraw reuses the scene pass rather than adding one. The scene pass already reads its records from the heap through the descriptor lattice and takes the pose from the HOT uniform, and the app already submits a scene with no kernel dispatch for the edge-on map, so a redraw is that submission with the retained grid and the new HOT write: `group(0)` heap data, descriptors and directory unchanged, `group(1)` binding the same `SceneUniform` and a HOT uniform carrying the destination `view_scale`, `camera_rotation_pairs`, `camera_translation` and `screen_to_plane` rows. No new bind-group layout, no second texture bind, no shader variant, and nothing beyond the WebGL2 and `EXT_color_buffer_float` floor the device walls already name.

The plan kind is what selects it. `ReliefRedraw` means the corpus was measured and exceeded the ceiling while the records remained compatible; `ClearOnly` means the corpus could not be measured at all, which is a perspective pole on the sampled relief and has no finite answer to redraw towards. The oracle asserts the split directly: `observer distance five h1`, `observer height 0 to 1` and `observer height 1 to half` must refuse as `ReliefRedraw` and then prove, sample by sample against independently computed fresh scenes, that the retained record a destination pixel would receive is the record that pose puts there; `observer yaw`, `observer pitch` and `observer distance four` must agree within the published bound at height zero and at height one alike.

What is not yet built is the scheduling: the app still presents a `ReliefRedraw` plan as the clear it is, so the picture the owner sees for a height change is unchanged from before. Making the redraw appear means submitting the retained grid as a dispatch-free scene when the plan names it, which is a change to the scene schedule in `app/src/frame.rs`.

### 2.8 Refresh, initial image, and measurements

Every refresh follows the fixed order `poll completed fences → drain HOT → write_hot(refresh_id mod 3) → frame(state,hot_slot) → app present`, with `submit_scene` when the app schedule says a scene is due; after `frame` the app drives `poll` through cooperative browser yields until the matching warp fence completes, captures the ending timestamp, and only then presents its singly owned surface texture.

When no compatible completed frame exists, the warp pass writes only `clear_rgba`; a completed scene always covers the whole target with mesh or exterior sky, and no diagnostic pattern or stale incompatible image is substituted.

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

Math defines the immutable CPU-only `Pose` as `{epoch,orbit_generation,plane,object,plane_origin,zoom_log2,view,grid_width,grid_height,map,centre_from_reference_px}`; present stores the pose used by each submitted scene, including all affine object/camera state and that level's `PoseMap`.

`HotSlot` is the opaque CPU token `{index:u32,dynamic_offset:u32,epoch:u64}` returned by `HotSlot::for_refresh(refresh_id,slot_stride,epoch)`, where `index=refresh_id mod 3` and `dynamic_offset=index·slot_stride`; only this constructor can create a slot, making `write_hot` infallible.

`FrameState<'a>` is the CPU-only app input `{surface_view:&'a wgpu::TextureView, canvas_width:u32, canvas_height:u32, refresh_id:u64, now_ms:f64}`; dimensions are physical pixels, `now_ms` is the app's monotonic `performance.now()` sample, and the app keeps the surface texture alive after `frame` returns until `poll` reports the matching warp fence complete, then presents outside the measured region.

### 3.5 GPU uniform blocks

`HotUniform` is exactly 288 bytes in eighteen 16-byte lanes: bytes 0–64 hold ten camera sine/cosine pairs, 80/96 camera translation, 112 observer rotation, 128 view scale, 144/160/176 inverse-sampling warp rows, 192/208/224 current screen-map rows, 240 exterior-zero colour, 256 clear colour, and 272 flags `[epoch_low,epoch_high,source_valid,edge_on]`.

Every f64 camera value is validated and narrowed once into this payload; the explicit factor lanes avoid dynamically indexed shader writes and translate to GLSL ES 3.00 on the WebGL2 device floor.

Each homography row stores three coefficients and zero padding; `source_valid` is one only when compatibility, the half-source-pixel residuals, finite arithmetic, the one-pixel measured ceiling, and the bound `(scene_id,texture_index)` all pass at draw time.

The HOT buffer size is `3·slot_stride`, where `slot_stride=align_up(288,device.limits.min_uniform_buffer_offset_alignment)`; one bind group covers the whole buffer, each pass selects exactly one slot by dynamic offset, and a refresh writes exactly 288 payload bytes.

`SceneUniform` is exactly 160 bytes in ten 16-byte lanes: byte 0 grid, 16 span with edge-on flag, 32/48 sampled basis, 64/80/96 sampled map rows, 112 palette map, 128 interior colour, and 144 clear colour.

`SceneUniform` is rewritten only when MAIN selection, palette, level, extent, span, or iteration cap changes; index-buffer updates and texture/bind-group replacement are regional allocation events tied to an extent change and are never per-refresh work.

### 3.6 Scene, warp, events, and facts

`SceneFrame` is the presenter-owned CPU record `{scene_id:u64,pose:Pose,palette:PaletteId,iteration_cap:u32,level:RefinementLevel,extent:[u32;2],texture_index:u32,centre_revision:u32,plane_origin_f64:[f64;4],precision_mode:&'static str,measurement:SubmissionMeasurement}` with no byte ABI; `texture_index` is zero or one, and revision, origin, plus precision mode prove whether a reference shift can rebase it or a MAIN change must clear it.

`SubmissionMeasurement` is `{kind:SubmissionKind,id:u64,source_scene_id:Option<u64>,sample_class:SampleClass,precision_mode:&'static str,wall_ms:f64,fence_wait_ms:f64,polls:u32}`; `SubmissionKind` is `Scene` or `Warp`, `SampleClass` is `ColdWarmUp`, `PolicyProbe`, or `Measured`, milliseconds are measured monotonic walls, and `source_scene_id` is unavailable for a clear-only warp.

`WarpPlan` is `{rows,source_scene_id,source_texture_index,source_valid,edge_on,exposed,kind,chart_residual,approx_max_error_px,approx_p95_error_px}` with no byte ABI; `WarpKind` is `AnchorHomography` or `ClearOnly`. `PresentEvent` carries a documented `large_enum_variant` allow because the completed scene owns its full sampled pose.

`PresentEvent` messages are `SceneCompleted { frame:SceneFrame }`, `SceneDropped { scene_id:u64, orbit_generation:u32, reason:DropReason, measurement:SubmissionMeasurement }`, `WarpCompleted { measurement:SubmissionMeasurement }`, and `FenceRefused { kind:SubmissionKind,id:u64,reason:FenceRefusal,polls:u32,wall_ms:f64,precision_mode:&'static str }`.

`DropReason` is `IncompatibleMain`, `ReplacedMain`, or `InvalidExtent`; `FenceRefusal` is `PollLimit`, `Deadline`, `Device`, or `Cancelled`, and each variant is rendered verbatim by the app rather than collapsed into “slow.”

`PresentFacts` publishes retained and pending identities, delivered state, precision provenance, view and reference displacement, scene/warp measurements, warp counts, texture reallocations, exposure/fill state, chart residual, measured maximum and p95 error, and status. `record_warp_plan` is the sole writer of the three planner facts, so they describe the same plan; every planned source has a measured maximum, while a pre-solve incompatibility has no fabricated number.

`PresentStatus` is `WaitingForFirstScene`, `ShowingCompletedScene`, `ShowingStaleApproximation`, `ClearForIncompatibleMain`, or `Refused(PresentError)`; app combines these delivered and measured facts with its own requested resolution, requested level, requested iteration cap, zoom digits, floor/working/delivered precision, orbit length, and rebase/glitch availability without substitution.

### 3.7 Callable API

`Presenter::new(device:Arc<wgpu::Device>,queue:Arc<wgpu::Queue>,heap:HeapPresentResources,config:PresentConfig)->Result<Presenter,PresentError>` allocates the 3-slot ring, two initially empty texture slots, fixed pipelines, immutable heap group, and two immutable warp groups as texture slots become allocated; it performs no device call before app has installed both error handlers.

`PresentConfig` is `{surface_format:wgpu::TextureFormat,min_uniform_buffer_offset_alignment:u32,fence_deadline_ms:f64,max_fence_polls:u32}` and v1 requires the app to pass the live alignment, `30_000.0`, and `4_096`; scene format remains fixed `Rgba8Unorm` independent of surface format.

`Presenter::set_main(&mut self,main:PresentMain)` is the infallible MAIN-drain endpoint: it records latest-wins state, applies a not-yet-consumed `reference_shift_px` to retained and in-flight poses for the accepted revision, and invalidates them when delivered `max_iter`, `plane_origin_f64`, or `precision_mode` changed, without allocating, submitting, waiting, or returning an error.

`Presenter::write_hot(&mut self,slot:HotSlot,hot:PresentHot,validation:WarpValidation)` stores the planned source scene and texture beside the slot, measures and enforces the plan bound, writes one 288-byte payload, and falls back to `source_valid=0` on refusal; `covering_warp_source_level(slot)` reports a source only when that same accepted plan is not exposed, and `frame` rechecks the retained identity before selecting it.

`Presenter::submit_scene(&mut self,hot_slot:HotSlot,now_ms:f64)->Result<u64,PresentError>` captures current MAIN and the exact HOT pose, asks the scene ledger to construct the pending record and return its single authoritative texture index, prepares and encodes against that same index, submits a four-byte fence, and returns its monotonically increasing `scene_id` without waiting.

`Presenter::frame(&mut self,state:FrameState<'_>,hot_slot:HotSlot)->Result<FrameReceipt,PresentError>` is called once per surface refresh, encodes the sole warp pass to `state.surface_view`, samples the best compatible completed scene or writes clear colour, submits its four-byte fence, and returns before completion; app retains the surface token, polls cooperatively for that `warp_id`, then presents outside the measurement.

`FrameReceipt` is `{refresh_id:u64,warp_id:u64,source_scene_id:Option<u64>,precision_mode:&'static str,status:PresentStatus}` with no byte ABI, reports `source_scene_id=None` when that slot's warp plan paints only clear even if a retained scene exists, and contains no fabricated wall because its fence has not completed.

`Presenter::poll(&mut self,now_ms:f64)->Vec<PresentEvent>` performs at most one shared `device.poll` per call to service both pending fences, increments each pending fence's own observation count once for that shared poll, retains the best accepted compatible scene across draft completions, promotes Final or a draft needed after refusal/exposure, retires bounded failures, and never waits or yields internally; app's refresh loop supplies the browser yield between polls.

`Presenter::facts(&self)->PresentFacts` returns the latest immutable snapshot without polling, allocating, submitting, or draining owner state.

`Warp::reproject(last_frame:&SceneFrame,from_pose:&Pose,to_pose:&Pose,precision_mode:PrecisionMode,validation:WarpValidation)->WarpPlan` is the pure CPU planner, delegates the exact plane-chart matrix to math's `warp_matrix(from_pose,to_pose)`, solves the four anchors in f64, conditionally samples only at the caller entry rather than inside the anchor solve, validates compatibility and finite results, and never touches the GPU; `last_frame.pose` must equal `from_pose` or the result is `ClearOnly`.

`PresentError` is `InvalidGrid { width:u32,height:u32,logical_len:u32 }`, `SceneBusy { scene_id:u64 }`, `StaleSpan { directory_index:u32 }`, `UnsupportedSceneFormat`, `UnsupportedSurfaceFormat { format:wgpu::TextureFormat }`, `ExtentAllocation { width:u32,height:u32 }`, `IndexCountOverflow { width:u32,height:u32 }`, `Device { operation:&'static str }`, or `SurfaceTargetZero`; no variant panics and none mutates requested app controls.

### 3.8 Interface table for joint review

|Producer → consumer|Interface|Pinned payload or call|Units and byte ABI|
|-------------------|---------|----------------------|------------------|
|math → present|`Plane`|`basis_u@0`, `basis_v@16`|32 bytes; f32 ℝ⁴ coordinates|
|math → shallow kernel|`CentreSplit`|`hi@0`, `lo@16`|32 bytes; four f32 hi+lo pairs|
|math → present|`Pose`|epoch, generation, object basis/angles/origin, zoom, twenty view scalars, extent, map, centre displacement|CPU-only semantic record|
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
|math → present/app|`ViewControls`|ten `Q` angles, five translations, yaw, pitch, height, two distances|twenty f64 scalars|
|present → app|`PaletteId`,`PaletteRecord`|Classic/Ember/Ice IDs and exact map/interior/clear literals|`repr(u32)` ID; 48-byte linear-RGBA record|
|present → GPU|`HotUniform`|camera rotation/translation, observer, scale, warp and screen maps, colours, flags|288-byte payload at dynamic ring offset|
|present → GPU|`SceneUniform`|grid, span, basis, sampled map, palette and colours|160-byte regional MAIN payload|
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

Per refresh CPU-to-GPU traffic is exactly one 288-byte HOT slot write; scene changes may regionally write the 160-byte scene block, an index-buffer prefix, descriptors, or changed texture resources, and every such event is counted separately.

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

Native layout tests assert exact sizes and offsets for the shared records, the 288-byte `HotUniform`, and the 160-byte `SceneUniform`, including every zero padding word.

Native PLANE tests pin `R₁₃(θ₁)R₂₄(θ₂)` operation order, the zero-angle seed, the exact `(e₁,e₂)` seed at `θ₁=θ₂=−π/2` and its reversal at `+π/2`, nonzero z and c components for every angle strictly between, one f32 rounding pass, and the three `8·f32::EPSILON` orthonormal bounds.

Native heap-address tests build multipage `DataSpan` fixtures and prove index `j·width+i`, page quotient/remainder, descriptor decoding, bottom-row orientation, last-page padding, stale-handle rejection, and canonical-zero out-of-range behavior.

Native pixel tests use asymmetric 2-by-2 and 3-by-2 fixtures to prove centre sampling, `+v` up, row zero at bottom, nearest level resampling, exact interior classification, exact magenta glitch classification, malformed-flag tinting, and palette arithmetic within one f32 ulp of its scalar reference.

Native mesh tests prove vertex count `width·height`, index count `6(width−1)(height−1)`, the exact six-index cell order, bounds for every index, no triangles across rows, square-pixel q coordinates, neutral glitch height, and overflow refusal before allocation.

Native scene-algebra tests compare deterministic vertices at fixed control values with `ember_lab_heap::mode_a_endpoint` at zero lattice coordinate, including both perspective poles at slider distances, and parse generated WGSL to pin the control-driven camera, the wgpu depth row, `LessEqual`, lighting literals, no blend, no MSAA, and `cull_mode: None`.

The height-zero identity test evaluates both preset-facing object/camera rows over their screen lattice and requires every mesh vertex to return to its own NDC position; translation is zero in these rows and distance fixtures remain exact. The screen-map oracle separately covers nonzero translation and general camera factors.

The shared navigation-drift oracle composes `10⁴` and `10⁵` steps of `R(Δθ)` with `Δθ=1e−3` radians and metric `||MᵀM−I||_F`; pass is at most `1e−5` for f64 without re-orthonormalization and for f32 with Gram–Schmidt every 64 steps.

The flat warp oracle evaluates hand-written f64 forward and inverse matrices for `zoom_log2 ∈ {0,10,20,40,80,100}`, both presets, square and 16:9 extents, finite angle pairs, subpixel and thousand-pixel pans, and accepted-reference shifts, and requires `max|H⁻¹H−I|≤1e−9`; the uploaded f32 matrix separately must map test pixels within `0.25` source pixel where representable.

Native reference-shift fixtures construct one physical centre before and after acceptance, apply `d_f←d_f−r_a_fB_fᵀB_a·reference_shift_px` exactly once, and require identical flat source coordinates; changed `max_iter` or `plane_origin_f64` must clear, while generation change alone must retain and rebase both completed and in-flight poses.

Native zoom-interface tests decompose scales on both sides of `zoom_log2=14`, prove present never forms the deep absolute f32 scale, and pin the displayed shallow/perturbation POLICY transition while leaving the `EscapeGrid` and warp-ratio interfaces unchanged.

Native anchor-warp tests pin exact flat plans against the full forward chain, enforce `WARP_MAX_ERROR_PX=1.0`, require over-budget or unbounded relief to clear, and report max and p95 for every plan. A counted policy test requires all validation modes to execute the full 9-by-9-by-5 corpus; a warmed sokol run measured 1,536 fully planned and asserted cases in `0.302874` seconds, or `0.197184` milliseconds per plan. The uploaded-row quarter-pixel oracle remains unconditional.

A second anchor test pins the retirement of the exact plan: at zero camera angles and `h=0`, for every distance in `{2,8,64}`, the solved four-anchor rows equal math's `warp_matrix` forward within `1e−6` per f32-packed coefficient and the sampled corpus reports below `1e−9`, which is the evidence that deleting `WarpKind::FlatExact` deleted a code path and not a capability.

Exact planner word and residual comparisons are guarded by the cfg-free `PrecisionMode::requires_bit_identity` helper and are labelled Deterministic conformance; finite plans, poles, error bounds, and every exact-path accuracy oracle execute for PictureFast as well.

The ambient oracle covers the two identity presets, random `O` and `Q` orthonormality, legacy two-angle equivalence, edge-on refusal, exactly one edge-on crossing on the Julia-to-Mandelbrot object morph at fixed `Q`, nonzero camera translations, and forward-after-inverse identity over a 9-by-9 screen lattice. The reprojection oracle covers pan, zoom, view rotation with and without relief, yaw/pitch, compatible object-rounding noise, incompatible object motion, in-plane and out-of-plane origin translation, camera translation, and cross terms against the published bound.

The coverage oracle rasterizes the CPU vertex mirror over a pose lattice including near-edge-on and `h=2` and requires every surface pixel to be covered by the mesh or the exterior sky. Both scene and warp shaders are parsed with the normal capability set and translated to GLSL ES 3.00.

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
|Object and camera rotations are conflated|wrong slice or observer motion|independent `O`/`Q` matrices, preset identity rows, edge-on crossing, and screen-map oracle|
|Deep perturbation enters the GPU as an absolute centre or tiny scalar scale|precision collapse|perturbation-layout fixture proving no centre field and mantissa/exponent scale decomposition|
|Flat warp reverses zoom, rows, or aspect|swimming or mirrored image|analytic pixel correspondences and the `H⁻¹H` oracle at all six zooms|
|A reference shift has the wrong sign, units, or revision|old pixels swim during a valid deep pan|physical-centre invariance oracle plus exactly-once completed/in-flight rebasing test|
|The anchor 2D warp exceeds its useful envelope|visible nonlinear swimming|Every plan measures against `WARP_MAX_ERROR_PX=1.0`; the 46.94/31.59 relief envelope is evidence for refusal, not permission to move features|
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

Phase 0A is implemented: the package shell, exact palette records, 288-byte HOT and 160-byte scene layouts, checked three-slot ring arithmetic, and finite homography solver and packer consume math-owned `ViewControls`, `Plane`, `Pose`, and `warp_matrix`.

Phase 3A now also implements checked mesh index and coordinate construction, present's VIEW coefficients, and an oracle bridge to heap's exported `mode_a_endpoint` without constructing any heap resource or math-owned type.

Phase 2/3 shader work now implements the heap-capacity-specialized height-field scene WGSL, dense-prefix handle resolution, bottom-row addressing, honest record shading, double-perspective pole rejection, the control-driven camera, and derivative lighting without constructing the pending heap seam.

Phase 4A now implements and validates the sole fullscreen warp WGSL, including source-valid clear, f32 pole rejection, out-of-bounds clear disocclusion, nearest-sampler compatibility, and no mip-level path; the f64 plan still awaits math's `warp_matrix`.

Phase 0 adds the present package shell, shared records, byte-layout assertions, palette scalar reference, consumption of the app-lane `HeapPresentResources` seam, and pure f64 homography/oracle code, estimated at 360 new Rust and test lines.

Phase 1 is implemented by the two-texture state ledger, 3-slot dynamic HOT ring, 160-byte scene block, bounded four-byte fence polling, typed events, and native interleaving tests.

Phase 2 is implemented by the flat fullscreen scene pipeline, heap-capacity descriptor/span accessor generation, palette and honest glitch shading, and target-resize handling; browser image facts remain visible replay, estimated at 340 Rust/WGSL/test lines.

Phase 3 is implemented by height-field grid/index generation, five-dimensional vertex algebra conformance through `mode_a_endpoint`, camera/depth/lighting, nearest fragment classification, and mesh/pole tests, estimated at 520 lines.

Phase 4 is implemented by math's exact plane-chart matrix, the f64 four-anchor planner and 9-by-9-by-5 error corpus, the one fullscreen warp pipeline, clear disocclusions, and the no-frame path, estimated at 440 lines.

Phase 5 is implemented by the exact app-facing API, facts snapshots, warm-up and reproject counting, callable-surface tests, visible-replay labels, release reconciliation, and documentation updates, estimated at 360 lines.

The implementation estimate is 2,500 net new lines across Rust, WGSL, tests, manifests, and present-owned documentation; only the app lane may make the J16 heap seam edits, so no heap edit is hidden inside this present estimate.

## 8. Unresolved joint-review findings

- `HeapPresentResources` is now published and sufficient for immutable presentation bindings, but the GL backend's validation of the independently reconstructed three-binding layout remains `requires visible replay` because the seam deliberately exposes resources rather than the private heap bind-group layout object.

- The present `EscapeGrid` contract embeds a cloneable `DataSpan`; kernels must confirm the exact lifetime handoff that prevents freeing a span still named by a retained or in-flight scene.

- The exact browser behavior of sampling an `Rgba8Unorm` scene texture whose extent differs from the surface is unmeasured on the GL backend, including whether nearest warp is visually acceptable at coarse refinement levels.

- The one-pixel enforced ceiling is native evidence; visible replay still measures how often relief motion is refused and how quickly its replacement scene arrives.

- A homography exposes only external borders, so internal relief disocclusion remains stale until a later depth-aware design; this round intentionally has no honest one-pass repair for it.

- The height normalization maps `max_iter` to a fixed `[-2,2]` range, which makes levels comparable but may visually compress interesting low-iteration structure; palette tuning cannot answer that geometry question.

- The three v1 palettes and clear colours are interface-stable literals but have not had accessibility or target-display review; changing them after implementation would require a documented palette-version change.

- Two immutable warp bind groups are rebuilt when their corresponding texture slot changes extent; this obeys the heap identity law but the allocation/rebuild cost across rapid progressive levels is unknown until visible replay.

- `performance.now()` callback observation can be delayed by browser suspension beyond the nominal deadline; the implementation can bound active polls and reject upon resumption but cannot promise wall-clock wake-up while JavaScript is not scheduled.

- The page overlay must remain outside the measured render regions and cannot consume another pass; this document assumes DOM text, while the integration implementation still has to confirm that the app's chosen page mechanism preserves that ordering.

- A reference shift is expressed in the newly accepted current basis; when the retained basis differs, projection into the retained basis has the same chart residual already reported for PLANE motion, and visible replay must establish whether that warning remains usable during simultaneous deep pan and rotation.

- The ambient relief sweep can reach 46.94 pixels, or 31.59 pixels with rotation and pan; the enforced one-pixel ceiling therefore refuses these motions until a fresh scene resolves them.

- The acceptance envelope has more terms than the sweep that produced it: the `0.002` rad bound is now asserted for each of the four VIEW and camera angles by analogy with the single swept angle, and no sweep exists for a moving height or a moving perspective distance. Either the sweep grows those axes or the published envelope names only the angle it actually measured.

- `PresentFacts::warp_p95_error_px` is published, but the app's page-facts mirror still hardcodes `None` at `crates/labs/julibrot/app/src/facts.rs`; that one-line read belongs to the app package and is the remaining half of the overlay reading `unavailable` for a value present has always computed.

- App §3.6 still restates an older flattened `PresentHot`/`PresentMain`, `warm_up:bool`, and generation-named clear status, while J14 assigns this API to present and present §3.4/§3.6 pins worker-owned state wrappers, `SampleClass`, and `ClearForIncompatibleMain`; implementation follows the owning present contract and app must reconcile its duplicate before consuming the package.

## 9. Refinement evidence

Semantic contract commit `df5280f959bfa67d4359110a6f66c33a8af6d85a` was checked out exactly on barza and passed `cargo test -p linter -- --skip the_repository_snapshot_loads_as_one_exact_surface --skip the_repository_owner_surface_reconciles_in_both_directions`: 727 library tests and 39 corpus tests passed, both named repository tests were filtered, exit was 0, `RUN-REPORT` wall was 0.8 seconds, and SSH-observed wall was 1.80 seconds.

The same semantic contract commit passed `cargo-fmt --all -- --check` on barza with exit 0, `RUN-REPORT` wall 1.3 seconds, and SSH-observed wall 2.51 seconds.

Before the semantic commit, local non-toolchain checks found only `docs/julibrot/present.md` changed, `git diff --check` passed, the banned token and superseded field names were absent, line endings were LF, and no UTF-8 BOM was present.

## 10. Implementation evidence

Implementation head `66fb25e093d73982f9cab2d92b5395a828e97974` was checked out exactly on barza and passed the required nine gates: workspace build `2.9 s`, workspace clippy with warnings denied `1.8 s`, cargo-fmt check `9.5 s`, workspace tests excluding linter `56.6 s`, linter tests with the two repository checks skipped `4.7 s`, wasm library checks for arena `1.8 s`, what-is-this `0.5 s`, fire `1.3 s`, and heap plus present `2.3 s`; each value is the corresponding `RUN-REPORT` wall and every exit was zero.

The present package's unit and integration suites cover exact layouts, palette honesty, GLSL ES 3.00 translation, mesh and exterior-sky coverage, two-slot transitions, source-bound plans, sampled-pose rebasing, slice compatibility, one-pixel enforcement, bounded fences, screen maps, the full error corpus, app-facing signatures, and the cross-motion reprojection oracle.

The final handoff audit additionally proves that synchronous `submit_scene` and `frame` refusals enter `PresentFacts.status`, all fallible scene preparation completes before the ledger reserves the in-flight slot, and palette, view, or grid replacement marks a pending scene `ReplacedMain` while retaining the last completed texture.

The native oracle rejects any motion whose measured maximum exceeds one pixel; no angle or zoom envelope can override that measured ceiling.

Barza establishes native and wasm compilation, byte/layout tests, CPU arithmetic, WGSL parse/validation, and bounded state transitions, but it cannot establish GL surface behavior, actual browser fence scheduling, visual orientation, disocclusion quality, console silence, or measured scene/warp costs; every such fact remains `requires visible replay`.
