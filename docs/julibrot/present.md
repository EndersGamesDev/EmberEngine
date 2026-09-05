# Julibrot presentation slice

Status: implementation complete for `crates/labs/julibrot/present`; the merged math and heap seams drive the f64 anchor planner, exact app records, two-texture runtime, HOT ring, the one image scene pass, the per-level status and reference census, the sole warp pass, bounded four-byte fences, and app-facing facts, while target-browser facts remain labelled `requires visible replay`.

## 1. Ownership and boundary

The present slice owns pixels after an `EscapeGrid` exists: the main height-field image scene from flat chart to full relief plus its optional coarse backdrop, palette records and palette evaluation, the two scene textures, the packed status and reference census target, the one warp pass, the three-slot HOT uniform ring, scene and warp completion measurements, and the present facts exported to the app.

The present slice allocates the HOT GPU buffer and exposes infallible `Presenter::write_hot`; the app calls it from the owner's HOT drain once per surface refresh, selects the slot by refresh number, acquires and presents the surface texture, and draws the honest page overlay outside the warped scene image.

The present slice consumes a typed `EscapeGrid` and immutable heap resource identities; kernels own refinement LEVEL definitions, escape dispatches, span reuse, and SCRATCH-to-DATA landing, while the app owns the refinement SCHEDULE and decides when to ask present for a scene.

Math owns `Plane`, `CentreSplit`, the shared `Pose`, the high-precision centre, presets, scaled perturbation and corrected rebasing theory, `warp_matrix`, and the navigation and warp arithmetic reference oracles; present consumes those records and turns the f64 matrix result into the GPU warp plan contracted below.

Worker owns reference-orbit computation and transfer, and present neither sees nor retains an orbit buffer; the reference record is repeated in §3 only to pin the transitive ABI against which the escape grid was produced.

The app owns wgpu 24 GL device creation, the sole surface-acquisition token, surface configuration and recovery, panic-hook installation, the non-panicking uncaptured-error handler, owner draining, controls, scheduling, and the facts overlay; present receives a borrowed surface view and never acquires, retains, or presents a `SurfaceTexture`.

Present is cosmetic authority only: a missing grid, stale span, invalid warp, pole rejection, timeout, or device error can change or clear pixels and publish a typed fact, but cannot author navigation, iteration, worker, simulation, protocol, or reconciliation truth.

The general DAG and petgraph, more than one world, a simulation tick, more than one heap class, shared-memory workers, WebGPU, a second glitch reference, mipmaps, blending, MSAA, motion vectors, depth-aware warp, and any image pass beyond the selected scene pass plus the warp pass are deliberately absent; the status and reference census is measurement work and never paints or replaces the retained image.

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

The scene pass draws the screen-grid height field into an LDR `Rgba8Unorm` target over a sky clear distinct from the palette's exterior colour at zero smooth iterations. At height zero every interior main-mesh vertex is its own screen position by construction and the outermost ring is drawn on the frame boundary, so the mesh tiles the frame exactly and the clear is never seen. A `width`-sample grid covers `width` pixels, so a mesh spanning only their centres falls half a pixel short at both ends: the rasterizer's fill rule counts the low boundary as covered and the high boundary as outside, which left the last column and row painted with the pass's clear and showing a colour no record carries. The ring is a drawn rule and not a sampling one, so the plane point each record carries is unchanged and every pixel still resolves to its own record. At positive height the floor remains on that chart, so a face-on frame has no contracted edge; the optional backdrop is reserved for raster-measured holes in tilted views. The five-dimensional near clamp keeps the lifted surface closed, while a later four-dimensional pole, a near-edge-on plane, the far side of the horizon, or ground reached by neither grid remains honest sky.

The scene target extent equals the delivered `EscapeGrid` extent, so a refinement level is both the delivered escape resolution and the delivered scene resolution; a changed extent is an allocation event, not a per-frame write, and is counted in `texture_reallocations`.

Vertex placement being the identity at height zero is not the camera being ignored there. The picture at height zero is decided entirely by which chart point each pixel samples, and that is the screen map, which carries the whole camera rotation: turning a camera factor off a preset row makes the same screen pixel land on a different point of the same slice, so the picture foreshortens. The identity picture belongs to the preset rows alone, because the canonical short-circuit keys on the object and camera pair and never on the height. Measured on the deployed page at 960 by 540, the Mandelbrot row's map has condition number `1` and `q₁₂ = 0.8` has `1.99957`, and the settled scenes differ by a mean `21.17`/255 per channel with the interior centroid `49` px apart.

That behaviour is easy to misread, so it is worth naming what a turn looks like while it is happening. A reprojected rectangle becomes a tilted quadrilateral with exposed corners — at `q₁₂ = 0.8` the warp covers `375,958` of `518,400` pixels — and the next completed scene fills those corners, at which point the frame boundary stops being tilted. Read quickly that is "the picture rotated and then snapped back upright"; measured, the picture never moved at all. Over the covered region the warp and the scene that replaced it differ by a mean `1.75`/255 per channel, with `2.1%` of pixels past `24`/255 at the fractal boundary where a resolving scene is expected to add detail, and the interior centroid moves `0.063` px. The drafts still run for such a move, and must: an exposing warp is never a covering frame, so the ladder rebuilds the corners the turn uncovered.

For an escaped non-glitch record, `hue = fract(max(smooth_iter,0)/period + phase)`, `phase_rgb = clamp(abs(fract(hue + (0,2/3,1/3))·6−3)−1,0,1)`, and `rgb = value·mix((1,1,1),phase_rgb,colour_mix)`; an interior record uses `interior_rgba` exactly.

The clamp is where the beyond-bailout samples land, and they are not rare. The squared bailout is fixed at `256`, so a sample already outside radius `16` when the recurrence starts escapes at index zero with `smooth_iter = 1−log₂(log₂|z₀|) ≤ −1`, and a first-iteration escape reaches `−1` as well; kernels' own record law asks only that an escaped count be finite, so those records are exactly as well formed as any other. Clamping the hue at zero paints the whole beyond-bailout region the palette exterior at zero smooth iterations — the colour the horizon already carries, which is the limit these samples approach as they run off to infinity. A hue left to cycle on an unbounded negative count would instead alias into stripes of ever-increasing frequency against that horizon. The height law's own `clamp` puts the same samples on the floor, so colour and geometry agree without a second rule.

A status-2 Horizon record is shaded as exterior at zero smooth iterations, not clear; status-3 MapUncertain is shaded from its sampled record. Only a warp coordinate outside its retained source uses the palette's honest clear colour temporarily.

The shader tests status `Glitch` before `escaped`, emits the fixed opaque diagnostic `(1,0.375,0,1)`, and never filters that classification; the magenta debug tint `(1,0,1,1)` is reserved for malformed records and other presentation contract violations. A non-finite `smooth_iter` on an escaped record is a violation too. A negative one is not: treating a legitimate beyond-bailout escape as a violation painted an opaque magenta half-frame over every pose whose sampled plane reached past radius `16`.

### 2.3 Height field, VIEW, camera, and projection

The scene uses one indexed triangle-list mesh with `width·height` vertices and `6·(width−1)·(height−1)` `u32` indices; for cell lower-left `a = j·width+i`, `b=a+1`, `c=a+width`, and `d=c+1`, the exact index sequence is `[a,b,c,b,d,c]`, although culling remains disabled.

The neutral-height screen coordinate comes from `(i,j)` through `grid_screen`, which places an interior sample at its own pixel centre `i+½−width/2` and the first and last sample of each axis on the frame boundary `∓width/2`. For relief, the vertex maps that screen pixel through the scene's `M`, forms the ambient object point `p=plane_origin+(4/width)(o_u u+o_v v)`, lifts it to `(p,h(H+2)/2)∈ℝ⁵`, and projects that point through the same camera chain math used to build `F`; therefore the floor and every height-zero-control vertex return to the starting screen coordinate by construction.

For a valid escaped sample the record's own height is `H = 4·clamp(smooth_iter/max(max_iter,1),0,1)−2`; an interior sample uses `H = −2`, and a glitch or malformed sample uses neutral `H = 0` so the geometry does not pretend to know the missing continuation. Glitches receive the orange diagnostic and malformed records receive the magenta debug tint. The clamp's lower bound is reached, not decorative: a beyond-bailout escape's negative count puts it on the floor beside the interior, which is where a sample that escaped at or before iteration zero belongs.

The displayed fifth coordinate is `h₅ = h·(H+2)·0.5` for the validated height control `h ∈ [0,4]`, evaluated in that written order in WGSL and in both binary64 mirrors. The requested zoom describes the floor: `H=-2` lifts by exactly zero, while `H=+2` lifts by `2h`, exactly the former peak position. At `h=0` the shader takes its direct identity branch before the projection chain and the mirrors produce zero lift for every `H`, so the flat picture is bit-identical rather than merely close; bounding valid `h` to the page range also excludes the binary32 overflow at which `h·4` could cease to be finite. Every intermediate valid control value remains a continuous morph.

The displayed height span fell from the former `4h` to `2h`. The five-dimensional near rule caps the peak at `0.95d₅` under either law, so the maximum representable relief depth is halved and no slider value can recover the discarded lower half; at `d₅=8` the maximum perspective contrast falls from 39-fold to 20-fold. The two shipped relief presets deliberately remain at `height_scale=1.0`: they now render half their former depth while retaining the same peak position. They are not retuned because their visual peak was the feature they were tuned around, while the floor must now stay where the requested zoom places it.

Negative `h` is refused because under `h·(H+2)·0.5` it leaves the interior and exterior floor on the chart while sending escaped records behind it: that is the receding-pit alternative, not an inversion of the floor and interior. The rejected alternative was to anchor the floor and send increasing escape height away from the eye. That relief never approaches the five-dimensional near limit, but its lit structure reads as a pit cut behind the chart rather than terrain rising from it; preserving the existing forward peaks while moving only the floor to the requested chart therefore keeps the established visual reading and removes the false zoom contraction on its merits.

The lifted ambient point first receives the frozen ten-factor camera rotation `Q`, then camera translation `t`, then the double perspective `P₅(p)=d₅/(d₅−p₅)·(p₁,p₂,p₃,p₄)` and `P₄(y)=d₄/(d₄−y₄)·(y₁,y₂,y₃)`, followed by the yaw/pitch observer and clip transform.

The retired chart-display frame replaced the ambient point by `(q_u,q_v,0,0,hH)`, making the old two-angle view visible but turning the picture about its own centre rather than moving the observer in ℝ⁵. The still earlier fixed two-angle ambient rotation collapsed `span(e₃,e₄)` because its rotations preserved the subspace with zero display axes. The correct cure is neither chart coordinates nor a fixed mount: the general independent `Q`, with preset rows chosen to face their slices, preserves the real ambient object and allows physical edge-on views.

Both perspective stages remain physically active because `O`, `Q`, `t`, and relief can place a component on either projected axis; both denominators are tested before division.

The 3D observer continues to use the same `d₄` as observer distance, with near `0.1`, far `4·d₄`, and perspective scale `aspect·d₄/2`; the generated WGSL and CPU mirror consume these quantities in that order.

The 288-byte HOT payload carries the ten `Q` sine/cosine pairs, five translations, observer, view scale, inverse-sampling warp, current `M`, exterior and clear colours, and flags. The 160-byte scene payload carries the sampled basis and map, so the scene shader reconstructs the same ambient point the kernels sampled.

The five-dimensional lift clamps the rotated and translated fifth coordinate to `d₅−0.05d₅`, so its projected denominator is at least `0.05d₅` and its perspective magnification is at most twenty; the same constant and operation order are used in WGSL and the binary64 footprint mirror. This small projection-space guard is deliberately not the later limit-model study's true model-space clipping distance: it keeps every lifted vertex strictly in front of the eye and draws a clamped vertex at the limit so both main and backdrop surfaces remain closed.

Above `h > 1.9d₅`, even the lowest lifted census height satisfies `0.5h > 0.95d₅`, so all four lifted census meshes reach the clamp, every lifted triangle is all-clamped and dropped, and the drawn scene is the flat floor chart alone. The state is reachable inside the slider range whenever `d₅ < 2.1` and is exactly the pinned close row at `h=4`, `d₅=2`; it is a limit-model consequence rather than extra relief detail.

The later four-dimensional perspective still invalidates a vertex at denominator `ε = 1e−4` and emits the fixed outside-clip position; the vertex also emits a numeric validity value and the fragment discards any interpolation below one, so every triangle incident to that invalid vertex is rejected rather than relying on the fixed position alone.

The implementation depends on `ember-lab-heap` and reuses the exact exported pure CPU oracle `mode_a_endpoint(base:[f64;5],coordinate:[i32;5],frame:&FrameUniform)->ModeAEndpoint` with zero lattice coordinate and a `FrameUniform` carrying `[cos θᵥ₁,sin θᵥ₁,cos θᵥ₂,sin θᵥ₂]`, poles `[d₅,d₄]`, and epsilon `1e−4`; the present WGSL operation order is tested against that function rather than copied into a second Rust oracle.

The indexed-grid construction follows the heap slice's pure-data mesh pattern but not its clock-derived frame: all ten `Q` angles and five translations are independent controls. The two pole constants survive as neutral `d₅` and `d₄`, while this two-dimensional slice enters standard ℝ⁵ only through its true ambient basis and relief lift.

After double perspective the three-space observer is two more controls, not a fixed mount: yaw `θ_c1` and pitch `θ_c2` in `[−π,π]`, observer distance `d₄`, near `0.1`, and far `4·d₄`.

Writing `cy=cos θ_c1`, `sy=sin θ_c1`, `cp=cos θ_c2`, and `sp=sin θ_c2`, for world point `(x,y,z)` the camera evaluates `yawed = (cy·x+sy·z,y,−sy·x+cy·z)`, `view = (yawed.x,cp·yawed.y−sp·yawed.z,sp·yawed.y+cp·yawed.z−d₄)`, and clip position `(k·view.x/aspect,k·view.y,(far/(near−far))·view.z+far·near/(near−far),−view.z)` with perspective scale `k = aspect·d₄/2`.

That one choice of `k` is what makes the height-zero image exact. At `z=0` the perspective divide is by `d₄`, the two cancel, and NDC is `(x/2,aspect·y/2)`, which is the §2.1 chart map for every `d₄` and every extent. So `d₄` sets how strongly depth foreshortens and never reframes the height-zero chart, which is what lets it be an honest perspective control instead of a disguised zoom.

The retired mount was a fixed 20-degree yaw, 15-degree pitch, distance `9`, scale `1.72` camera inherited from rawgl. It cannot be kept. It is a view degree of freedom hard-coded into the pipeline, which is exactly what the control model abolishes, and its framing differs from the chart map by the aspect-dependent factor `2·1.72/(9·aspect)`, so under it no height-zero image is the flat image and the two pictures could never be reconciled. Its two angles return as the yaw and pitch controls, where a preset that names them recovers the inherited look as a row of numbers that a user can leave, and their neutral value is zero because the flat chart is the picture the lab opens on.

The depth expression is rawgl's OpenGL projection converted to wgpu's zero-to-one depth range, maps view z `−0.1` to zero and `−30` to one, and uses `LessEqual`; the pipeline is one-sample, has no blend, no mipmaps, and `cull_mode: None`. The attachment is `Depth24PlusStencil8`, whose stencil aspect carries the §2.9 layer stamp; the format is core WebGL2 and is recorded in `docs/minimum-requirements.md`.

The scene fragment obtains a surface normal from derivatives of the interpolated double-projected world position, uses fallback `(0,0,1)` when the derivative cross product is degenerate, and applies the heap-pinned light `0.58 + 0.24·|n·normalize(0.4,0.7,0.6)|` and colour `mix(white,hue_rgb,colour_mix)·value·light`.

The fragment also performs a nearest integer escape-record load from the interpolated grid coordinate and branches on its exact glitch status, so the diagnostic is not interpolated across neighbouring vertices; rawgl's `0.013` long-box thickness is explicitly inapplicable to a triangle height field and is the only §10 heap presentation literal not used.

All six object rotations, ten camera rotations, five camera translations, observer angles, height, and both distances are HOT presentation controls; the four plane-origin coordinates remain MAIN sampling state because moving the origin selects a translated affine slice and may need a new reference orbit.

### 2.4 Scene texture pair and submission

Exactly two single-sample, one-mip `Rgba8Unorm` scene textures exist: one is the best compatible fence-completed texture sampled by warp and the other is the sole in-flight scene target; before the first completion there is no retained texture and the non-target texture has no semantic content.

A scene submission captures reconciled `PresentMain` plus the referenced HOT slot into math's immutable `Pose`, including object angles, object origin, twenty view scalars, and the level's map; that captured sampled pose remains the sole basis for later reference-shift rebasing.

While the scene fence is pending, every surface refresh may submit a warp against the retained texture. After the fence callback, `Presenter::poll` promotes the completed texture, pose, palette, grid extent, and measurement unless the retained higher-level or larger-extent scene still has an accepted warp for the latest HOT pose, exposed or covering; in that case the draft completion advances the app schedule without replacing the retained source and its texture remains the next target. A Final completion replaces the older Final as usual.

If a new level has a different extent, only the available target is reallocated before scene submission, its immutable warp bind group is rebuilt once and the allocation count advances; the retained texture and its bind group stay valid until promotion, keeping the total at two textures.

Scene requests while a target is already in flight return `PresentError::SceneBusy` instead of allocating a third texture, blocking, or overwriting work; an accepted reference shift rebases each retained or pending scene from its own sampled pose, never from `max_by_key(epoch)` or a newer HOT pose.

Partition-retention decision, 2026-09-05 on `lane/jb-retain`: an incompatible slice, delivered cap, precision mode, or MAIN generation moves the last completed image into one explicit held slot instead of erasing its texture identity. The held image keeps the partition stamp under which it completed and may participate only in an unchanged `HoldStale` plan; it is never a source for a homography or relief redraw across partitions. One new-partition scene may remain in flight beside that held image, and its first accepted completion replaces the held slot. This implements the held-transition portion of [tiled reprojection's validity and invalidation design](tiled-reprojection.md#validity-compatibility-and-invalidation) without changing the tile migration path.

The corresponding additive facts are `held_frame_partition` and `held_since_scene_id`. They identify the stale source rather than claiming it belongs to the requested partition, and they clear when a matching completed scene replaces it or when its underlying texture is explicitly forgotten. Choosing whether automatic or manual scheduling presents that honest hold remains app policy.

### 2.5 Exact plane-chart homography

Let `M_p` be pose `p`'s accepted screen-to-plane map and `B_p=[u_p v_p]` its once-rounded basis. A screen point first passes through `M_p`; its reference-relative ambient point then follows from `B_p`, pixel scale, centre displacement, and plane origin without materializing an absolute deep GPU coordinate.

For current destination pose `t` and retained source pose `f`, math composes `H(f→t)=M_t⁻¹·T·M_f`; `T` contains basis overlap, scale ratio, centre displacement, and compatible in-plane origin translation. Present uploads the explicit inverse `H(t→f)` for texture sampling.

The translation term is the desired-centre displacement difference in retained-frame pixels; when the bases agree its components reduce to `b_x=2·(r·d_t.x−d_f.x)/width_f` and `b_y=2·(r·d_t.y−d_f.y)/height_f`, so pans remain smooth without an absolute centre or absolute deep scale.

The shader evaluates rows explicitly as `r = H_(t→f)·(x,y,1)`, rejects non-finite `r` or `|r.z|≤1e−12`, computes source NDC `s=(r.x/r.z,r.y/r.z)`, converts to source UV `(s+1)/2`, and emits clear colour rather than clamping whenever either UV component lies outside `[0,1]`.

This is exact at height zero when both poses sample the same affine slice. Present compares the two constructed ambient 2-flat spans at the f32 rounding floor rather than comparing six object parameters, and composes math's exact in-plane orthogonal chart map when they coincide; a plane-origin delta may pass only when its out-of-plane component and the normalized chart residual are each at most half a source pixel, so in-plane origin motion is exact pan and out-of-plane motion is a slice change.

On an accepted reference, worker publishes `reference_shift_px` as new minus old reference centre in current-zoom pixels along accepted basis `B_a`; present computes only `r_a_f=s_a/s_f=2^(zoom_f−zoom_a)·width_f/width_a` and re-expresses every retained or in-flight pose as `d_f←d_f−r_a_fB_fᵀB_a·reference_shift_px`, then advances its reference generation without clearing the image.

The absolute bignum centre never enters this matrix: `centre_from_reference_px` and `reference_shift_px` remain f64-safe at arbitrary depth. A reference-generation change alone does not clear, and a plane-preserving object rotation remains a valid source; cap, precision mode, a genuine object-plane tilt, or out-of-plane origin incompatibility refuses the warp.

A pan translates old content and exposes a border on the entering side; every mapped source coordinate outside `[0,1]` shows `clear_rgba` with no clamp, stretch, wrap, or stale edge pixel, while the next completed scene supplies the newly revealed fractal samples.

All powers, dot products, the 2-by-2 inverse, and the 3-by-3 forward/inverse check are evaluated by hand-written `f64` CPU code; only the final three padded rows are rounded to f32 for `HotUniform`.

### 2.6 Anchor warp and relief fallback

The ordinary warp is deliberately depth-free and uses one 2D homography of the already presented image. When the measured relief deformation exceeds that map's bound, the retained-record redraw in §2.7 reuses the existing scene mesh as the one presentation pass rather than moving features with an invalid homography.

The four destination anchors are the current screen corners. Each passes through the current screen map, the compatible plane-chart map, and the retained forward screen map; the solve therefore composes the same maps used by screen-aligned sampling.

The planner and scene shader use the identical ambient construction. At neutral height the composed map is exact for the compatible affine plane, including its in-plane object-basis rotation; under relief the sampled corpus measures the nonlinear error that one 2D homography cannot follow, and the proved redraw family remains the only over-ceiling fallback. Across a plane-preserving object turn, the fixed plane is the common ambient 2-flat spanned by both constructed bases: the retained grid record stays attached to its ambient four-point, the destination lift supplies the new height, and a neutral 5D camera preserves the per-height projective plane used by the redraw.

`WarpKind` is `AnchorHomography`, `ClearOnly`, `HoldStale`, or `ReliefRedraw`. Flat compatible plans are exact; a missing source, incompatible slice, edge-on map, source-identity mismatch, failed solve, an unmeasurable corpus, or an over-ceiling plan outside the proved redraw family is `ClearOnly` and is never shown as a moving feature. Manual presentation converts that refusal to `HoldStale` only when a retained picture exists: the source identity is restored with identity rows, so the accepted best stays on the surface unmoved instead of becoming a permanent clear while no scene is scheduled. An over-ceiling plan is `ReliefRedraw` only for a pure height or `d₅` change with every other sampling input fixed, or when both retained and destination poses have a neutral 5D camera rotation and translation and the sampling lattice — zoom, plane origin, centre displacement, extent, and main map — is unchanged; it retains the source identity and presents the same main-grid records through the scene mesh under the destination HOT pose.

The implementation solves the eight projective coefficients of the current-NDC-to-source-NDC homography with f64 Gaussian elimination and partial pivoting, fixes `h₂₂=1`, refuses a pivot below `1e−12`, and rounds the valid result to the same three-row HOT layout used by flat warp.

Every plan, including ordinary PictureFast, carries a measured maximum error and p95 from the same full 9-by-9-by-5 corpus. A sample with no finite destination, retained projection, or warp image makes the plan unbounded and therefore `ClearOnly`; honest texture-edge disocclusion remains the separately flagged temporary clear region that the next scene fills. The uploaded f32 rows retain their separate quarter-source-pixel accuracy oracle.

The `h=0` slice of this admission corpus is structurally zero because the height-zero projection short-circuit returns each screen point directly; moreover, `sampled_errors` defines `source_screen` by applying the candidate warp itself, so this corpus cannot independently bound a corrupted chart map. Even a corrupted sampling matrix can therefore publish `approx_max_error_px=0.0` when `height_scale=0`. Flat exactness rests on the `warp_matrix` algebra and its full-forward-chain tests plus the unconditional quarter-source-pixel uploaded-row oracle, while `retained_warp_matches_independent_fresh_scenes` validates the chart map against independently sampled retained and fresh pictures.

The ceiling is asked only where there is a reprojection to judge. A retained scene whose pose is the pose being displayed is sampled by the identity: every destination pixel reads the source pixel it came from, so there is nothing to approximate and nothing to measure, and the plan is exact by construction rather than by corpus. Sameness here is the picture, not the record: every field that decides a pixel is compared — slice, origin, object angles, zoom, view controls, sampled extent, screen map, displacement from the accepted reference — and the publication bookkeeping is not. The epoch advances on every HOT write, so a whole-pose equality is false on the refresh after the one that captured it and stays false for as long as the view is held; it cannot answer what is on screen. That case is decided before the corpus is built, because the corpus can fail to be measurable for reasons that say nothing about it — a relief lattice sample behind a perspective pole, a screen sample beyond the horizon — and an unmeasurable corpus refuses. Refusing the identity is the one refusal that cannot recover: the clear counts as exposure, exposure restarts the refinement ladder, and the ladder can only deliver the same scene at the same pose to be refused again. A held relief pose with a horizon inside its frame sat in exactly that loop, showing the clear colour with a completed Final in hand. For the same reason present does not latch exposure from any refusal measured against the pose the retained scene was rendered at: exposure names ground the source cannot cover, and there is no such ground when the source is already this pose's completed scene.

The named homography acceptance ceiling is `WARP_MAX_ERROR_PX=1.0`: a resolving scene may fill missing detail but no displayed image warp may move a feature by more than one pixel. A measurable plan above the ceiling selects retained-record redraw only inside the proved exact family; any other over-ceiling or unmeasurable plan remains clear until the due scene completes.

At 1920 by 1080 with `height_scale=1`, measured admission thresholds put most camera-plane homographies beyond the ceiling after roughly `0.001` to `0.02` radians; yaw/pitch reaches it near `0.0033`, height near `0.0027`, and `d₅` motion near `0.018`, while `q₁₄` near `0.067` and `q₂₄` near `0.091` tolerate a few degrees. Pure height and `d₅` motions select retained-record redraw; observer motion does so only at a neutral 5D camera, while an over-ceiling non-neutral cross term clears then fills.

The measured 1920-by-1080 ambient relief sweep reaches `46.94` pixels over the full envelope and `31.59` pixels for rotation with pan. These numbers describe what relief reprojection can reach, not what is accepted: they explain why relief warps outside small motions are refused under the one-pixel ceiling.

The corpus is measured where there is something to measure. A lattice sample beyond either pose's screen-map horizon is skipped rather than refusing the whole plan: the scene pass leaves the exterior sky there and the warp pass carries that sky across, so the sample has no reprojection error to report. The five-dimensional lift cannot fail at its former pole because it uses the shared near clamp; any later projection failure still refuses, and a plan with nothing measurable at all is refused too. Skipping is what makes a horizon a horizon rather than a wall: refusing instead left every pose whose horizon crossed the frame permanently unpainted, because the exact plan a settled pose warps onto itself was refused along with the rest.

Exposure is measured against the retained image, which reaches half a texel past its outermost sample centres. That reach is not cosmetic. A pose composed onto itself is the identity only as closely as the f32 plane basis it was built from allows: at `o₁₃=1.5` the frame's own border lands `2.1e-5` pixels outside itself, two orders of magnitude inside the half-texel footprint and three orders below any disocclusion a moved view produces. Without the footprint that rounding read as a disocclusion, and because an exposed warp latches the exposure that restarts the refinement ladder, the ladder restarted for as long as the pose was held.

Newly exposed source coordinates outside the retained texture show `clear_rgba` and set the exposure latch; the next completed scene fills them over the exterior sky. An exposed accepted warp continues sampling the sharper retained source wherever it has coverage instead of promoting a draft over the whole surface; this lane deliberately keeps the uncovered region temporarily clear rather than adding a second texture sample and bind group. `warp_exposed_fraction` reports the share of the same fixed 9-by-9 destination lattice that the actual uploaded image-warp rows put out of source while excluding points the shader paints as horizon sky; it is absent for `ReliefRedraw` because those holes depend on the retained records' heights rather than the homography. The plan carries the exact `(scene_id,texture_index)` it was solved against, and draw clears if the retained source differs, preventing a scene promoted between HOT write and frame from sampling a different texture with stale rows.

### 2.7 Relief redraw

Reprojection is a navigation primitive and has to cover every viewpoint degree of freedom, but two of the observer bars are not viewpoint degrees of freedom at all. The escape height enters the projection on the fifth ambient axis, so the height amplitude and the fifth-space distance — together with the four camera factors that turn that axis into the chart, and the fifth translation — decide where a record of a given escape height sits. Under them each retained pixel moves by an amount proportional to its own escape height, and no map of the image can express a per-pixel displacement. Everything else — yaw, pitch, `d₄`, the six chart-only camera factors, pan, zoom — acts on the point after the lift and is carried exactly by the one image homography at any height.

That is why the screen map is bit-identical across height amplitudes, pinned by `the_screen_map_is_independent_of_the_height_amplitude` in `math/src/screen.rs`: the height never reaches the map the warp is fitted from. Freshly measured through the 960-by-540 planner corpus after floor anchoring, the `0.005` step is `0.689270` px, `0 → 1` is `183.575598` px, `1 → 0.5` is `104.900341` px, and `d₅ 8 → 6` at height one is `91.787799` px. Those maxima round to the former published `0.689`, `183.58`, `104.90`, and `91.79` values because every maximum occurs at `H=+2`, whose new `h·(H+2)·0.5 = 2h` lift is bit-identical to its former `hH = 2h` lift. The fresh pin records that invariance rather than manufacturing a numerical change, and the over-ceiling cases still select retained-record redraw.

The retained records are the way out only when geometry proves that their lifted points describe the destination. A pure height or `d₅` change keeps every other sampled-chart input fixed, so the retained records are the destination records. More generally, when both 5D camera rotations and translations are neutral, every lifted grid lies in one fixed plane and later yaw, pitch, `d₄`, zoom and pan are projective views of that plane. Redrawing through the existing screen-aligned scene mesh then places every covered retained sample exactly where a fresh scene does, without a kernel dispatch. A non-neutral 5D camera can mix the height axis out of that plane: the review mirror measured maximum plane deviation `3.886e-16` for the neutral family and `1.214` for a representative non-neutral rotation, with uncertain/compared counts `89/92` for camera factor 3, `122/56` for cross terms, `78/83` for factor 6, `64/82` for factor 7, `79/95` for yaw/pitch and `38/143` for `d₄`; these over-ceiling cases now clear instead of moving features.

The redraw reuses the scene pass rather than adding a shader path. The scene vertex stage already reads records from the RGBA32F heap through the descriptor lattice and takes the destination lift and camera from HOT, so redraw binds the retained extent, plane, source map and record span through the existing `SceneUniform`, then uses the destination HOT write: `group(0)` heap data, descriptors and directory unchanged, `group(1)` the same scene and HOT bindings. The same WGSL entry points and indexed mesh are specialized into a second render-pipeline object only because the browser surface format differs from the `Rgba8Unorm` retained-image target; there is no new bind-group layout, shader variant, uniform byte, texture sample, or device feature beyond WebGL2 and `EXT_color_buffer_float`. Because Preview, Interactive, and Final overwrite one Final-capacity span, app alternates two such spans by ladder round: the accepted Final keeps its own records while the other span is refined, and a promoted Final exchanges their roles. At 960 by 540 the extra logical reservation is one eight-page DATA span, 8,388,608 bytes inside the already allocated 64 MiB DATA texture, plus three 64-page header sets, 49,152 bytes at the WebGL 256-byte alignment, for 8,437,760 bytes of reservation accounting; the DATA texture does not grow, so only the header buffer is new VRAM, and its three-set growth is 12,288 bytes on a device with 64-byte uniform alignment. No third main span is live.

The plan kind selects the draw. `ReliefRedraw` means the corpus was measured above the homography ceiling and the pose pair is in the exact family, so it keeps the retained scene identity, counts one warp-class redraw, clears the target to the same distinct `clear_rgba` used by the image warp, and draws the retained main-grid records under the destination pose. When the current compatible MAIN selection still owns a validated coarse backdrop grid, redraw binds that grid's own uniform and index mesh too and runs the same main-first, stencil-owned fallback composition in the one surface depth pass: fine main content owns every pixel it reaches and coarse content fills only its holes. Without a resident backdrop, `warp_exposed_fraction` publishes the binary64 coverage mirror's main-only clear fraction instead of leaving the relief hole fact absent; with a backdrop it publishes the combined-apron remainder. Uniform construction reads the retained frame's own span, dimensions, source map, and level and refuses if those dimensions no longer equal the frame it belongs to; the span may also be the idle live main grid, because identity alone does not mean the ladder is overwriting it. App explicitly drops only the retained record binding immediately before kernel encoding starts on that span, while preserving the completed texture for image reprojection and holds; freeing a main or backdrop span drops both the binding and its retained image before the directory entry can be reused. A record refusal is a presentation choice rather than a loop error: it follows the same `ClearOnly` or `HoldStale` fallback policy. The live ladder level and extent are otherwise irrelevant, while a real surface resize still fails the planner's equal-lattice proof before drawing. `ClearOnly` also covers every over-ceiling pair outside the exact family and every corpus that cannot be measured. `HoldStale` draws a retained source with identity rows, reports no exposure, and is not an accepted warp for draft-skipping; manual mode may keep it indefinitely while waiting for Update scene, and auto mode keeps it only while replacement work remains pending. There is no wall or hold-count expiry: a stalled current Final can hold the stale picture indefinitely and honestly continues to report `HoldStale`; completing or retiring the replacement work removes the automatic hold condition. The oracle asserts the split directly: `observer distance five h1`, `observer height 0 to 1`, `height 0 → 2.165 at the floor`, and `observer height 1 to half` select `ReliefRedraw` and prove sample by sample against independently computed fresh main scenes that every covered destination pixel agrees with `uncertain=0` and `disoccluded=0`; neutral-camera yaw, pitch and `d₄` agree within the homography bound, while their over-ceiling non-neutral-camera cross terms clear.

The rebased exact pins make the floor change visible without changing the peak envelope. The former height `0 → 2.165` across-apron fixture measured `(compared, uncertain, disoccluded)=(127,0,54)` and now measures `(181,0,0)` at the floor because the face-on destination has complete main-mesh coverage and no backdrop, while its maximum stays `64.977` px. Observer height `0 → 1` stays `18.358` px, height `1 → 0.5` stays `10.490` px, `d₅ 8 → 6` stays `5.433` px, and the `ρ₃₄` redraw stays `9.179` px because each maximum remains on the unchanged `H=+2` peak.

For scheduling, `ReliefRedraw` is an accepted warp from the retained level. Automatic and manual refreshes both present it immediately; automatic refinement skips drafts and requests Final, while manual mode holds the redraw until Update scene is pressed. A `HoldStale` remains a refusal rather than an acceptance, so the ordinary requested ladder resolves it and the completed requested picture replaces the hold. `relief_redraw_count` counts submitted redraw passes, `warp_hold_count` counts submitted stale holds, `warp_kind` distinguishes the redraw and stale hold, and the existing maximum and p95 facts retain the per-point homography error that selected the fallback.

### 2.8 Refresh, initial image, and measurements

Every refresh follows the fixed order `poll completed fences → drain HOT → write_hot(refresh_id mod 3) → frame(state,hot_slot) → app present`, with `submit_scene` when the app schedule says a scene is due; after `frame` the app drives `poll` through cooperative browser yields until the matching warp fence completes, captures the ending timestamp, and only then presents its singly owned surface texture.

When no compatible completed frame exists, the warp pass writes only `clear_rgba`; a completed scene always covers the whole target with mesh or exterior sky. In automatic mode a refused warp may instead hold the last retained picture unmoved exactly while newer work is pending, with no round, elapsed-time, or hold-count expiry; completion or retirement ends that condition, and slice incompatibility is never shown as motion.

The warp samples the retained `Rgba8Unorm` texture with a nearest sampler and no mipmaps, preserving debug-tint classification; the disocclusion test happens before the sample and uses the palette's clear colour.

Timing uses no timestamp query: scene cost starts immediately before scene uniform writes and encoding and ends when the four-byte fence mapped after scene submission completes, while warp cost starts immediately before HOT write and warp encoding and ends when the four-byte fence mapped after warp submission completes.

App's per-level timing ring consumes these existing scene and warp completion measurements without adding a fence, wait, timestamp query, or present-owned record. There is no completion boundary between the separately submitted kernels encoder and the scene encoder, so app records kernel `dispatch_us` as unavailable; the scene fence remains the first GPU completion boundary and the warp fence remains the second.

Each fence records total wall milliseconds, the subset spent from first `map_async` poll through callback observation, and every `device.poll`; the first poll precedes yielding, the bound is 4,096 polls and 30,000 ms, and timeout or cancellation becomes a typed event rather than an unbounded wait.

The first fenced scene and warp after initialization, texture reallocation, or pipeline creation are labelled cold warm-up and excluded from aggregates, but their walls and polls remain displayed; the second fenced scene is the labelled policy probe and selects continuous animation at `scene_ms≤100` or single-frame-on-demand at `scene_ms>100` without becoming an admission test.

`reprojected_per_scene` counts fence-completed warp submissions that sampled one completed scene and is published when that scene is replaced; refreshes shown as clear before the first frame are counted separately and are never credited to a scene.

### 2.9 The backdrop grid

Screen-aligned sampling inverts the chart map, and the displayed lift now leaves `H=-2` on that chart. The requested zoom therefore describes the floor: at the first owner row (`height_scale=2.165`, `d₅=8`, yaw and pitch zero) the far exterior and interior remain on the frame they were sampled for, the peaks remain at their former `d₅-4.33` depth, the raster mirror reports zero uncovered surface at apron one, and no backdrop exists to substitute a coarser zoom.

An amplitude-only edge estimate no longer describes coverage: floor anchoring leaves the floor on its chart while tilted cameras can still expose surface that a wider sampling map reaches. `scene_footprint` therefore rasterizes the mirrored mesh at apron one and at the candidate set `{1.25,1.5,2,3,5}` and compares exact uncovered-point counts on the interior 63-by-63 census. It finds the candidate with the largest gain over apron one, then chooses the smallest candidate that both recovers at least half that best gain and itself recovers at least one percent of the 3,969 admitted samples, meaning at least 40 points. If no candidate meets both conditions, the request stays one. A best gain of zero identifies sky that no reviewed apron reaches, while a positive sub-threshold gain is reachable ground deliberately not worth a separate coarse scene.

The taken design leaves every main map at `apron_scale=1.0` and allocates one separate coarse backdrop only when that measured request is greater than one. The backdrop uses the same plane, centre, accepted reference orbit, precision mode, height records, and presented camera as the main grid, but samples the selected `scene_footprint.apron_scale`; it is a real escape scene rather than a flat floor or a stretched edge.

Only two quantities carry that wider sampling span. Kernels receive `zoom_log2−log₂(apron_scale)`, which widens their `pixel_scale`, and the scene uniform multiplies only `chart_scale` by the same apron; the homography rows, inverse, condition number, centre, reference orbit, and projection aspect are unchanged. The main path takes an explicit bit-preserving branch at scale one, and at height zero `scene_footprint` returns scale one exactly, allocates no backdrop, and leaves every main map and deterministic request bit-identical to the former construction.

The backdrop request uses floor-half dimensions in both axes and its own capacity-selected Final-level plan, so its records are at most one quarter of the delivered main Final count; at 960 by 540 it is at most 480 by 270, or 129,600 records. It uses the same delivered iteration cap and therefore costs at most one quarter of the main Final's worst-case iteration work and record memory. App schedules it before Preview because missing wide ground has priority over a detail level; the temporary backdrop-only frame has Preview presentation rank so accepted-warp draft skipping cannot mistake its Final-level backdrop records for a main Final, and its completion does not advance the main refinement ladder.

One uncached footprint performs six rasterizations. At 960 by 540 that is 126,750 vertex projections, 245,760 triangle setups, and approximately 907,000 barycentric tests. The native `profile.dev` (`opt-level=1`) x86 test on sokol measured ten close-row calls in `247.983 ms`, or `24.798 ms` per call; the audit correction's release-profile native x86 run measured `201.215 ms`, or `20.121 ms` per call. `ViewerController` therefore memoizes the result in a three-slot cache keyed by the bit patterns of the complete object, complete view, and extent; requested backdrop preparation, delivered backdrop preparation, and the full-device-extent page-facts snapshot reuse their matching entries instead of repeating this work, with FIFO insertion and promotion of a hit to the newest slot.

Composition is one render pass with one combined depth-and-stencil buffer, and the stencil is what decides it. Clear to distinct sky, depth to the far plane and the stamp to zero; draw the main mesh first with its fragments stamping one, then draw the backdrop with the stencil test `Equal` against zero and no stencil write. So a covered pixel shows the fine main record whatever the two depths say, a pixel the main grid never reaches shows the backdrop record, and sky survives only where neither mesh reaches or a projection honestly clips. Within each layer the depth buffer still orders that layer against itself with `LessEqual`, so the backdrop keeps its own internal occlusion.

Depth alone cannot do this, and drawing the backdrop first under `LessEqual` was the earlier mistake. The two grids are independent samplings of the same escape-time field: the half-extent backdrop is coarser again by its selected apron, its record heights are uncorrelated with the main grid's, and its long chords can therefore land nearer than the fine surface over large areas. A depth test admits those chords straight through the interior of the picture, which is the coarse layer overwriting the very detail it exists to surround. `LessEqual` gives the main grid only the bit-equal case, and the two grids almost never agree bit for bit.

"The main grid wins" is a rule about these two layers and holds only while they share one sampling map family and one pose, which is exactly the backdrop's contract: same plane, same centre, same reference orbit, same camera, differing only by a screen-space apron about the view centre. Under that contract the main grid is strictly the finer sampling of the same surface wherever it reaches, so preferring it is preferring resolution. It is not a general composition law. Once layers may carry different poses — retained tiles from earlier views, the reprojection design's per-tile source poses — the winner is chosen by projected sample density at the pixel, per `docs/julibrot/tiled-reprojection.md`, and a stencil stamp keyed to one layer is no longer the right mechanism.

The five-dimensional near rule clamps every lifted vertex, on both grids, to a denominator of at least `0.05d₅` before projection. Five percent bounds the fifth perspective's magnification at twenty while keeping the clamp small relative to the view distance; drawing the clamped vertex rather than dropping it keeps the mesh closed. The shader and binary64 mirror share the constant, while the later limit-model study retains ownership of a true model-space clipping distance.

A bound of twenty is not by itself an honest picture, and the clamp has to be paired with a primitive rule or it merely moves the garbage. A triangle whose three vertices are all held at the limit has no honest vertex left: nothing about its position comes from the record field, and before the pairing it was drawn as a smeared sheet across as much as the whole frame, at up to twenty-times magnification, where the earlier code had drawn sky. So the vertex stage flags the clamp and the fragment stage discards where the interpolated flag is one. That is the entire primitive exactly when all three vertices are clamped, and nothing but a degenerate edge otherwise, so a triangle with at least one unclamped vertex is still drawn and the surface stays closed at the limit. The binary64 coverage mirror applies the same rule, which is why a pose whose mesh mostly fails to project now reports the sky it cannot reach instead of a magnified invention.

`scene_footprint` reports the raster-selected requested `scene_apron_scale`, the interior uncovered fraction after that requested span, and the share of its fixed 9-by-9-by-5 bounded record-domain census at the near clamp. `scene_backdrop_scale` and `scene_backdrop_extent` are absent until a backdrop is actually allocated and then report its applied scale and capacity-selected extent; `relief_clipped_fraction` publishes the census result.

Coverage is measured the way the picture is drawn, because the earlier boundary formula ceased to describe the anchored geometry and the still earlier ring measurement was vacuous exactly where the answer mattered. The mirror rasterizes the sampling mesh at every census height and candidate apron into a 65-by-65 lattice whose samples run to `±1` inclusive, then excludes the outermost ring and counts the 3,969 interior samples. The ring is exactly where `grid_screen` places the outermost mesh vertices, so the last bit and the triangle fill rule can classify an isolated boundary sample differently across faithful mirrors or same-aspect extents; buying a plan, coarse grid, dispatch, and composition pass for that arithmetic would repeat the category error the anchored floor removed. A triangle is drawn only when all three vertices project and not all three are held at the near clamp — the scene shader's own primitive rule — so a pose whose mesh falls apart reports more sky and never less. The candidate comparison always uses this same deterministic raster mirror; its sampled heights and 65-by-65 mesh make the result a stated measurement rather than a mathematical bound.

The clipping census keeps its whole fixed denominator, all 405 points. A census point beyond the projective horizon is not at the clamp and is not removed from the denominator; the close row's near-limit share at the selected scale is pinned in the table below.

|Owner row|Main uncovered|`scene_apron_scale` / applied backdrop|Post-request uncovered|Near-clipped census|
|---------|-------------:|-------------------------------------:|---------------------:|------------------:|
|Julia, `height_scale=2.165`, `d₅=8`|0|1 / none|0|0|
|Julia, `height_scale=4`, `d₅=8`|0|1 / none|0|81/405 = 0.2|
|Tilted Julia, `height_scale=4`, `d₅=d₄=2`|611/3969 ≈ 0.1540|2 / 2|571/3969 ≈ 0.1439|252/405 ≈ 0.6222|

The close row uses `o₁₃=o₂₄=-1.3166537201715494`, `q₁₃=q₂₄=-0.2541426066233471`, yaw `0.960422302787256`, and pitch `π`. Its floor mesh is the flat chart and reaches 3,358 of 3,969 interior lattice points at apron one. Each of the four lifted census meshes has projectable vertices but every one is held at the near clamp, so every lifted triangle is all-clamped and dropped; that measured mechanism, rather than equality between two derived fractions, is why the relief coverage is exactly the floor coverage at this row.

The close row's candidate uncovered counts are `1.25 → 572`, `1.5 → 573`, `2 → 571`, `3 → 569`, and `5 → 585`. Scale three is best and recovers 42 points from the main count of 611. Scale 1.25 recovers 39 points, so it fails the absolute 40-point floor even though it exceeds half the best gain; scale two is the smallest candidate that clears both tests, recovers exactly 40 points, and is selected. The non-monotonic spread is raster quantization, not a geometric ordering by apron width. A dedicated page fact that classifies residual sky remains a backlog item so browser readers do not have to infer its cause from the coverage count.

At the floor-half 480-by-270 policy extent, all four shipped presets request no backdrop: the flat Mandelbrot and Julia rows return the structural zero, while both relief rows leave fewer than 40 residual interior samples. The native test prints each target's exact count for diagnosis without making a near-tie-sensitive count a cross-target contract; the recorded sokol run reports Mandelbrot relief's seven residual interior samples and Julia relief's zero, both below the admission floor, so the sub-threshold verdict does not buy a coarse grid.

Backdrop policy runs at the floor-half extent selected in `app/src/frame/loop.rs`, while `PageFacts` measures at the device extent. With zero camera translation the forward homography's extent terms cancel under any rescale in exact arithmetic; the power-of-two rescale between a Final and its half-Final also reproduces the rounding bit for bit, and the mirror pins bit-identical answers at 480 by 270, 960 by 540, and 1920 by 1080 for both shipped relief rows and the close owner row. That agreement is arithmetic, not a general extent-invariance theorem: the prior rim-inclusive census changed at the same-aspect 1024-by-576 extent, which is why the policy now excludes the frame tie rather than treating the power-of-two coincidence as geometry.

These fractions are the drawn rule evaluated at a stated resolution, not bounds. Sampling five heights understates what the full domain reaches; a 65-by-65 mirror mesh coarser than the drawn one overstates it, because near the projective horizon one long chord cuts across the curved image and covers ground a fine mesh leaves as sky. A point scatter of the same mesh, which counts only where vertices land and never fills a triangle, reports far more sky than either. The published share is therefore comparable against itself — main against backdrop, pose against pose — and is not a claim about the exact pixel count of the delivered frame.

The retained scene texture already contains the composed backdrop and main image, so `AnchorHomography` remains an image warp on the presented frame and gains no scale field or second source. The sampled `Pose` and every warp anchor retain the main map at scale one; the independent fresh-scene oracle likewise renders its main samples unchanged, while the backdrop has no role in the warp algebra.

`ReliefRedraw` always redraws the retained main-grid records through their source main map and additionally draws the current validated coarse backdrop when its records remain resident. The same stencil ownership as an ordinary scene prevents the coarse grid from displacing fine content. If the backdrop is absent or stale, the main-only coverage fraction remains explicit and ground outside the retained mesh stays honest clear until a coarse or new main scene lands.

A full-resolution records apron with its own Final budget clamp remains a future quality option. It may replace or supplement the coarse backdrop only after its memory and iteration cost has an explicit admission policy; it is not silently inferred from the existing capacity divisor.

### 2.10 Stage-0 rendered-view foundations

The pure stage-0 types implement the [rendered-view coordinate model](tiled-reprojection.md#rendered-view-coordinate-model), [descriptor and heap relationship](tiled-reprojection.md#rendered-tile-descriptor-and-heap-relationship), [validity matrix](tiled-reprojection.md#validity-compatibility-and-invalidation), and [same-surface ownership rule](tiled-reprojection.md#one-pass-depth-composition-and-intersections) without entering the shipped draw path. `TileContentKey` names the versioned slice, MAIN generation, cap, precision, record ABI, and strict reference generation; `TileRenderKey` captures exact bit patterns for `O`, origin, `Q`, translation, height, both perspective distances, observer, zoom, extent, source rectangle, source map, slice, and MAIN generation.

`TilePoseHeader` is exactly 32 aligned RGBA32F texels and requires `H27–H31` to remain positive zero. `DescriptorSamplePair` is exactly 32 bytes: `S0` is the unchanged four-lane escape record and `S1` is `(a_F,b_F,zeta_F,validity)`. The cost ledger pins 2,097,152 sample bytes plus 512 header bytes, or 2,097,664 logical bytes per 256-square tile, and reproduces the design table's resident-set totals.

`CanonicalChartCellKey` names same-surface cells only; it never substitutes for physical target depth. `TileQuality` applies the deterministic tuple `(residency,rung,density,error,age,tile_id)` inside one such cell, and order-independent selection tests keep that ownership separate from intersections. The table-driven `RenderControlChange` oracle keeps all camera, observer, height, zoom, extent, plane-preserving, in-plane-origin, and display changes while starting a held-only new partition for slice tilt, out-of-plane origin, cap, precision, record ABI, or strict MAIN generation.

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

If reference index `r` reaches the stored orbit length before escape or `max_iter`, kernels stop that pixel with status `Glitch`; present must show the fixed orange diagnostic rather than interpolate, conceal, continue, or mislabel it as a magenta contract violation. App prevents stale-reference dispatch, and the census candidate lets app replace a reference that is too short for the level before the next delivery; regional second-reference repair of a residual cluster remains deferred.

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

`PresentBackdrop` is the CPU-only adapter `{grid:EscapeGrid,iteration_cap:u32,plane:Plane,map:PoseMap}` and `PresentMain` is `{epoch:u64,state:MainState,grid:EscapeGrid,object:ObjectAngles,plane:Plane,map:PoseMap,backdrop:Option<PresentBackdrop>}`; app constructs them after publication, and present derives generation, delivered cap, palette, reference shift, and `c₀` compatibility from the worker-owned main state.

Math defines the immutable CPU-only `Pose` as `{epoch,orbit_generation,plane,object,plane_origin,zoom_log2,view,grid_width,grid_height,map,centre_from_reference_px}`; present stores the pose used by each submitted scene, including all affine object/camera state and that level's `PoseMap`.

`HotSlot` is the opaque CPU token `{index:u32,dynamic_offset:u32,epoch:u64}` returned by `HotSlot::for_refresh(refresh_id,slot_stride,epoch)`, where `index=refresh_id mod 3` and `dynamic_offset=index·slot_stride`; only this constructor can create a slot, making `write_hot` infallible.

`FrameState<'a>` is the CPU-only app input `{surface_view:&'a wgpu::TextureView, canvas_width:u32, canvas_height:u32, refresh_id:u64, now_ms:f64}`; dimensions are physical pixels, `now_ms` is the app's monotonic `performance.now()` sample, and the app keeps the surface texture alive after `frame` returns until `poll` reports the matching warp fence complete, then presents outside the measured region.

### 3.5 GPU uniform blocks

`HotUniform` is exactly 288 bytes in eighteen 16-byte lanes: bytes 0–64 hold ten camera sine/cosine pairs, 80/96 camera translation, 112 observer rotation, 128 view scale, 144/160/176 inverse-sampling warp rows, 192/208/224 current screen-map rows, 240 exterior-zero colour, 256 clear colour, and 272 flags `[epoch_low,epoch_high,source_valid,edge_on]`.

Every f64 camera value is validated and narrowed once into this payload; the explicit factor lanes avoid dynamically indexed shader writes and translate to GLSL ES 3.00 on the WebGL2 device floor.

Each homography row stores three coefficients and zero padding; `source_valid` is one only when compatibility, the half-source-pixel residuals, finite arithmetic, the one-pixel measured ceiling, and the bound `(scene_id,texture_index)` all pass at draw time.

The HOT buffer size is `3·slot_stride`, where `slot_stride=align_up(288,device.limits.min_uniform_buffer_offset_alignment)`; one bind group covers the whole buffer, each pass selects exactly one slot by dynamic offset, and a refresh writes exactly 288 payload bytes.

`SceneUniform` is exactly 160 bytes in ten 16-byte lanes: byte 0 grid, 16 span with edge-on flag, 32/48 sampled basis, 64/80/96 sampled map rows, 112 palette map, 128 interior colour, and 144 clear colour. The first three words at byte 96 are the denominator row and its fourth word is the applied sampling apron: exactly one for the main grid and the requested footprint scale for the backdrop.

The presenter owns independent main and backdrop scene-uniform buffers, bind groups, and index buffers. Each is rewritten only when its selection, palette, level, extent, span, iteration cap, or sampled map changes; index-buffer updates and texture/bind-group replacement are regional allocation events tied to an extent change and are never per-refresh work.

### 3.6 Scene, warp, events, and facts

`SceneFrame` is the presenter-owned CPU record `{scene_id:u64,pose:Pose,palette:PaletteId,iteration_cap:u32,level:RefinementLevel,extent:[u32;2],texture_index:u32,centre_revision:u32,plane_origin_f64:[f64;4],precision_mode:&'static str,measurement:SubmissionMeasurement}` with no byte ABI; `texture_index` is zero or one, and revision, origin, plus precision mode prove whether a reference shift can rebase it or a MAIN change must clear it.

`SubmissionMeasurement` is `{kind:SubmissionKind,id:u64,source_scene_id:Option<u64>,sample_class:SampleClass,precision_mode:&'static str,wall_ms:f64,fence_wait_ms:f64,polls:u32}`; `SubmissionKind` is `Scene` or `Warp`, `SampleClass` is `ColdWarmUp`, `PolicyProbe`, or `Measured`, milliseconds are measured monotonic walls, and `source_scene_id` is unavailable for a clear-only warp.

`WarpPlan` is `{rows,source_scene_id,source_texture_index,source_valid,edge_on,exposed,kind,chart_residual,approx_max_error_px,approx_p95_error_px}` with no byte ABI; `WarpKind` is `AnchorHomography`, `ReliefRedraw`, `HoldStale`, or `ClearOnly`. `PresentEvent` carries a documented `large_enum_variant` allow because the completed scene owns its full sampled pose.

`PresentEvent` messages are `SceneCompleted { frame:SceneFrame }`, `SceneDropped { scene_id:u64, orbit_generation:u32, reason:DropReason, measurement:SubmissionMeasurement }`, `WarpCompleted { measurement:SubmissionMeasurement }`, and `FenceRefused { kind:SubmissionKind,id:u64,reason:FenceRefusal,polls:u32,wall_ms:f64,precision_mode:&'static str }`.

`DropReason` is `IncompatibleMain`, `ReplacedMain`, or `InvalidExtent`; `FenceRefusal` is `PollLimit`, `Deadline`, `Device`, or `Cancelled`, and each variant is rendered verbatim by the app rather than collapsed into “slow.”

`PresentFacts` publishes retained and pending identities, delivered state, precision provenance, view and reference displacement, scene/warp measurements, image-warp and relief-redraw counts, texture reallocations, exposure/fill state and fraction, chart residual, measured maximum and p95 error, and status. `record_warp_plan` is the sole writer of the planner and exposure facts, so they describe the same plan; every planned source has a measured maximum, while a pre-solve incompatibility has no fabricated number.

`PresentStatus` is `WaitingForFirstScene`, `ShowingCompletedScene`, `ShowingStaleApproximation`, `ClearForIncompatibleMain`, or `Refused(PresentError)`; app combines these delivered and measured facts with its own requested resolution, requested level, requested iteration cap, zoom digits, floor/working/delivered precision, orbit length, and rebase/glitch availability without substitution.

### 3.7 Callable API

`Presenter::new(device:Arc<wgpu::Device>,queue:Arc<wgpu::Queue>,heap:HeapPresentResources,config:PresentConfig)->Result<Presenter,PresentError>` allocates the 3-slot ring, two initially empty texture slots, fixed pipelines, immutable heap group, and two immutable warp groups as texture slots become allocated; it performs no device call before app has installed both error handlers.

`PresentConfig` is `{surface_format:wgpu::TextureFormat,min_uniform_buffer_offset_alignment:u32,fence_deadline_ms:f64,max_fence_polls:u32}` and v1 requires the app to pass the live alignment, `30_000.0`, and `4_096`; scene format remains fixed `Rgba8Unorm` independent of surface format.

`Presenter::set_main(&mut self,main:PresentMain)` is the infallible MAIN-drain endpoint: it records latest-wins state, applies a not-yet-consumed `reference_shift_px` to retained and in-flight poses for the accepted revision, and invalidates them when delivered `max_iter`, `plane_origin_f64`, or `precision_mode` changed, without allocating, submitting, waiting, or returning an error.

`Presenter::write_hot(&mut self,slot:HotSlot,hot:PresentHot,validation:WarpValidation,hold_refused_warp:bool)` stores the planned source scene and texture beside the slot, measures and enforces the plan bound, writes one 288-byte payload, and falls back to `source_valid=0` on refusal unless manual policy requests an identity `HoldStale` against an existing retained source; `accepted_warp_source(slot)` excludes a stale hold and otherwise reports the accepted source level plus its exposure bit, and `frame` rechecks the retained identity before selecting it.

`Presenter::submit_scene(&mut self,hot_slot:HotSlot,now_ms:f64)->Result<u64,PresentError>` captures current MAIN and the exact HOT pose, asks the scene ledger to construct the pending record and return its single authoritative texture index, prepares and encodes against that same index, submits a four-byte fence, and returns its monotonically increasing `scene_id` without waiting.

`Presenter::frame(&mut self,state:FrameState<'_>,hot_slot:HotSlot)->Result<FrameReceipt,PresentError>` is called once per surface refresh, encodes either the image warp or retained-record relief redraw to `state.surface_view`, samples the best compatible completed scene or writes clear colour, submits the shared four-byte warp fence, and returns before completion; app retains the surface token, polls cooperatively for that `warp_id`, then presents outside the measurement.

`FrameReceipt` is `{refresh_id:u64,warp_id:u64,source_scene_id:Option<u64>,precision_mode:&'static str,status:PresentStatus}` with no byte ABI, reports `source_scene_id=None` when that slot's warp plan paints only clear even if a retained scene exists, and contains no fabricated wall because its fence has not completed.

`Presenter::poll(&mut self,now_ms:f64)->Vec<PresentEvent>` performs at most one shared `device.poll` per call to service both pending fences, increments each pending fence's own observation count once for that shared poll, retains the best accepted compatible scene across draft completions including exposed accepted warps, promotes Final or a draft needed after refusal, retires bounded failures, and never waits or yields internally; app's refresh loop supplies the browser yield between polls.

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
|owner/app → present MAIN|`PresentMain`,`PresentBackdrop`|main state/grid/object/plane/map plus optional backdrop grid/cap/plane/map|CPU-only adapters; latest published main and optional coarse layer|
|math → present/app|`ViewControls`|ten `Q` angles, five translations, yaw, pitch, height, two distances|twenty f64 scalars|
|present → app|`PaletteId`,`PaletteRecord`|Classic/Ember/Ice IDs and exact map/interior/clear literals|`repr(u32)` ID; 48-byte linear-RGBA record|
|present → GPU|`HotUniform`|camera rotation/translation, observer, scale, warp and screen maps, colours, flags|288-byte payload at dynamic ring offset|
|present → GPU|`SceneUniform`|grid, span, basis, sampled map plus apron, palette and colours|160-byte layer-local payload|
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

Aggregate `rebase_count` remains unavailable in normal rendering. Every completed level renders an exact census in which one `Rgba8Unorm` audit texel covers 255 escape records; at 960 by 540 the audit target is 960 by 3 and the mapped payload is 11,520 bytes. Its target and readback buffer are cached together by census extent. Each texel carries four things about its 255 records: the exact status-one count in red, the best reference candidate's rank in green, that candidate's local offset in blue, and whether the group holds a candidate at all in alpha. The rank is a total order over the records a reference may be taken from: a record that never escaped within this level's cap ranks 255, a glitch that exhausted its reference ranks 254, an escaped record ranks by its own count over 1..253, and a glitch from arithmetic failure ranks 0; equal ranks keep the lowest record index, so the same completed grid always names the same point.

Only the exhaustion glitch says anything about orbit length, which is why the kernel's smooth lane carries the glitch kind: that pixel survived every reference step available to it, while the six arithmetic-failure paths say nothing and would otherwise have steered the reference toward the numerically worst records in the frame. The top rank is a heuristic and not a certificate. A record can report no escape within the cap and still name a point whose exact orbit is far shorter — measured on the zoom-14 seahorse row, where the first candidate reported no escape at a 512 cap while its own reference orbit ended at 251 records — because the rebase recomputes the delta in binary32 and the kernel has no relative-precision test to catch the cancellation. The exchange is therefore bounded rather than monotone, and it is made safe by keeping the longest accepted orbit: an arriving sampled reference that does not lengthen the accepted one is discarded. The scene fence alone controls picture delivery: present samples the census callback without waiting when that fence completes, publishes `glitch_pixel_count` and the candidate only if the mapping is already successful, and otherwise cancels the mapping and leaves both unavailable. `glitch_pixel_count` stays unavailable for Preview and Interactive, while a failed or slow instrumentation callback can neither delay nor refuse a correct Final, and the candidate it did not deliver only means no reference request is made from that grid.

The app installs the panic hook and replaces wgpu's fatal uncaptured-error handler before `Presenter::new`; every present device operation returns or publishes a typed error, and no bare `unreachable`, unchecked surface acquisition, or panic is an error protocol.

Hand-written f64 remains the matrix implementation unless the shared warp oracle fails its `1e−9` bound; `faer` may enter only after that measured failure, never for style or anticipation.

Renderer austerity is one selected image scene pass plus the sole warp pass, no mips, no blend, `cull_mode: None`, one sample, and two retained scene textures total; the packed census is an auxiliary measurement render target rather than another image pass, and kernel fragment-compute passes remain data production.

## 5. Oracles and tests

Native layout tests assert exact sizes and offsets for the shared records, the 288-byte `HotUniform`, and the 160-byte `SceneUniform`, including every zero padding word.

Native PLANE tests pin `R₁₃(θ₁)R₂₄(θ₂)` operation order, the zero-angle seed, the exact `(e₁,e₂)` seed at `θ₁=θ₂=−π/2` and its reversal at `+π/2`, nonzero z and c components for every angle strictly between, one f32 rounding pass, and the three `8·f32::EPSILON` orthonormal bounds.

Native heap-address tests build multipage `DataSpan` fixtures and prove index `j·width+i`, page quotient/remainder, descriptor decoding, bottom-row orientation, last-page padding, stale-handle rejection, and canonical-zero out-of-range behavior.

Native pixel tests use asymmetric 2-by-2 and 3-by-2 fixtures to prove centre sampling, `+v` up, row zero at bottom, nearest level resampling, exact interior classification, exact orange glitch classification distinct from magenta malformed-record tinting, and palette arithmetic within one f32 ulp of its scalar reference.

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

The ambient oracle covers the two identity presets, random `O` and `Q` orthonormality, legacy two-angle equivalence, edge-on refusal, exactly one edge-on crossing on the Julia-to-Mandelbrot object morph at fixed `Q`, nonzero camera translations, and forward-after-inverse identity over a 9-by-9 screen lattice. The reprojection oracle covers pan, zoom, view rotation with and without relief, yaw/pitch, compatible object-rounding noise, exact `ρ₃₄` plane rotation, inert `ρ₁₂`, incompatible object tilt, in-plane and out-of-plane origin translation, camera translation, and cross terms against the published bound.

The coverage oracle rasterizes the CPU vertex mirror over a pose lattice including near-edge-on and `h=2` and requires every surface pixel to be covered by the mesh or by sky the geometry accounts for. Accepting any sky pixel at all was the weaker predicate this oracle started with, and it is what let the lifted border pull-in pass unnoticed: the sky is a legitimate answer only where a pole, a horizon or an edge-on plane produced it, and `scene_footprint` is what separates those from a mesh that fell short. Both scene and warp shaders are parsed with the normal capability set and translated to GLSL ES 3.00.

Native state-machine tests permute scene completion, HOT writes, MAIN replacement, accepted-reference shift, incompatible cap, plane origin, or precision mode, control movement, resize, warp completion, deadline, and poll-limit events and prove exactly one retained plus one in-flight texture, latest-wins promotion, exactly-once pose rebasing, no third allocation, bounded retirement, and correct `reprojected_per_scene` attribution.

Native page-contract tests pin the eight callable entries `new`, `set_main`, `write_hot`, `submit_scene`, `frame`, `poll`, `facts`, and `Warp::reproject`, the exact initial overlay phrase, required facts fields, requested-versus-delivered separation, Final glitch count and unavailable rebase aggregates, all three sample labels, refresh order, app surface ownership through warp completion, and panic/error setup before `Presenter::new`.

Browser initialization, GL backend identity, `EXT_color_buffer_float`, vertex-stage DATA access, actual surface format, exact scratch-copy visibility, and absence of validation or console errors are `requires visible replay`.

Image orientation at height zero and under relief, the continuity of the morph as each control moves, pole clipping, orange glitch isolation from magenta contract violations under scene and warp movement, clear disocclusion, clear first frame, refinement resizing, and rapid HOT motion while a scene is in flight are `requires visible replay`.

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

The GPU implementation map is `gpu.rs` for the public façade and re-export, `gpu/device.rs` for adapter/resource construction and selection setup, `device/uniforms.rs` for bind-group layouts, `device/scene.rs` plus `scene/submit.rs` for target management, stencil-composed draw order and scene submission, `device/warp.rs` for HOT publication and image warp, `device/redraw.rs` for relief redraw, `device/census.rs` for census render/readback/decode, `device/ledger.rs` for retained-source and HoldStale policy, `device/poll.rs` for fence/event publication, and `device/tests.rs` for the moved presenter test corpus.

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

- The limit-model study must decide whether the reachable `h > 1.9d₅` state should continue presenting only the flat floor chart or publish and handle relief saturation explicitly.

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
