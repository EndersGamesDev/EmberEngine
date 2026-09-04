# Julibrot app slice — refined integration contract

Status: implementation in progress after the five-document written review and final integration rulings; this remains the contract the math, kernels, worker, and present implementations meet where a later disagreement is discovered.

## 1. Ownership and boundary

The app slice owns `crates/labs/julibrot/app`, `web/labs/julibrot/`, construction and integration of the four sibling slices, the wgpu 24 GL device and surface, the single surface token, startup diagnostics, cross-slice call order, refinement scheduling, controls, measurement policy, facts overlay, page-contract tests, the versioned loader, and release-bundle evidence.

The app slice owns the implementation-round heap extraction seams approved in §3.8; those seams expose paid heap behavior to kernels and present, and the app lane stops and reports if extraction needs more than moving existing code behind public visibility.

Math owns Julibrot algebra, corrected plane construction, `CentreSplit`, scaled perturbation theory, reference precision, the bignum adapter, `Pose`, warp matrices, and CPU oracles; kernels owns both GPU kernels, the exact three refinement levels, `DispatchFacts`, grid spans, dense-prefix headers, and SCRATCH-to-DATA landing.

Worker owns the bignum centre, transfer protocol, credit, cancellation, owner records and drains; math defines `ViewControls` and present re-exports it, while present owns palettes, `HotUniform`, `SceneUniform`, its two scene textures, the hot ring, scene and warp fences, warp planning, and present facts.

The app does not copy sibling or heap code, reinterpret an owner’s byte layout, author fractal or gameplay truth, add a general DAG or petgraph, add another world or simulation tick, add another heap class, use shared-memory threads, repair glitches with a second reference, or add WebGPU.

Every interface owned elsewhere is duplicated below intentionally so the joint documents can be mechanically compared; an implementation-discovered disagreement is reported rather than papered over locally.

## 2. Design

### 2.1 Coordinates, corrected rotations, and poses

The fractal axes are ordered `(z.re,z.im,c.re,c.im) = (e₁,e₂,e₃,e₄)` in ℝ⁴; `e₅` carries escape height only in present’s height field and never enters the fractal plane, centre, worker message, or fractal kernel.

The object rotation is the math-owned six-factor `O∈SO(4)` applied to the one seed `(e₃,e₄)`; `u=Oe₃`, `v=Oe₄` receive one f32 rounding pass, and the legacy two-angle adapter maps exactly to object factors `ρ₁₃` and `ρ₂₄`.

Because `O` is orthogonal, no projection or Gram–Schmidt repair enters plane construction; math's postcondition remains `|u·u−1|`, `|v·v−1|`, and `|u·v|` each at most `8·f32::EPSILON`.

The object pose is affine: its four-coordinate plane origin is a `Pose` field. Mandelbrot is `O=I`, origin zero; Julia at `c₀` uses `ρ₁₃=ρ₂₄=−π/2` and origin `(0,0,c₀.re,c₀.im)`.

The camera pose is affine in ℝ⁵: math's ten-factor `Q∈SO(5)` acts on the lifted object point, then translation `t∈ℝ⁵` is added before the two perspective stages. `ViewControls` therefore carries ten camera angles, five translations, yaw, pitch, height, and two distances.

The Mandelbrot preset faces its unrotated seed with `q₁₃=q₂₄=−π/2`; the Julia preset uses `Q=I`; both have zero camera translation and are exact flat identity pictures. No geometric control reads time.

Math defines the immutable semantic `Pose` consumed by app and present; it records owner epoch, orbit generation, once-rounded plane, six object angles, object origin, `zoom_log2`, all twenty view controls, level extent, `PoseMap::Mapped|EdgeOn`, and centre displacement from the accepted reference.

### 2.2 Pixels, arbitrary zoom, perturbation, and rebasing

For an active grid `W×H`, pixel `(i,j)` samples its centre at `x=i+0.5−W/2`, `y=j+0.5−H/2`; row zero is bottom, `+v` is up, pixels are square, and the grid aspect follows the physical canvas without stretching.

Each refinement grid is screen-aligned. App builds math's neutral-height `M` for that level from the accepted object and camera poses and passes it to both the selected kernel and present; `PoseMap::EdgeOn` skips invalid mapping work and requests an all-sky scene painted with exterior-zero colour.

For `q=zoom_log2`, the mathematical pixel scale is `p=4/(2^q·W)`; shallow work at `q<14` computes `p` in f64 and rounds it once to f32, while perturbation at `q≥14` never forms an absolute tiny f32 scale.

For perturbation, math returns `p=m·2^s` with f32 mantissa `m∈[0.5,1)` and signed integer exponent `s`; equivalently `a=2−q−log₂W`, `s=floor(a)+1`, and `m=2^(a−s)`, with non-finite or i32-exponent overflow becoming typed refusal.

The GPU starts with `o′=(o_u·u+o_v·v)·m` from `M(x,y)`, so the true offset is `2^e·o′`; Mandelbrot still has `δz₀′=0`, Julia still has `δc′=0`, and no kernel uniform contains an absolute centre.

The scaled recurrence is `δ′ₙ₊₁=2Zᵣδ′ₙ+S·δ′ₙ²+δc′`, the full value is `zₙ=Zᵣ+S·δ′ₙ` through `ldexp`, and the rebase predicate is `|zₙ|<|S·δ′ₙ|`; underflow of `S·δ′` makes the predicate false because a negligible delta must not rebase.

When `|δ′|` leaves `[2⁻⁶⁴,2⁶⁴]`, the pixel renormalizes by `δ′←δ′·2^∓64` and `e←e±64` under math’s scaled-state invariant; the f64 CPU mirror performs the identical exponent changes and is the kernel oracle.

On rebase, the implementation reconstructs `Z₀` from reference record zero, sets the unscaled delta to `zₙ−Z₀`, represents that difference at the current scale, resets `r←0`, increments `rebase_count`, and performs exactly one ordinary advance against `Z₀`; therefore `zₙ=Zᵣ+δₙ` remains true for nonzero `Z₀`.

When the reference index reaches `length` before escape or `max_iter`, the pixel sets status `Glitch` and stops; present shows the opaque orange diagnostic, while the magenta debug tint remains reserved for contract violations.

Reference transfers carry `[re,im]`, one binary32 word per coordinate; app expands them to heap texels `[re,im,0,0]`, and math’s `D_work` versus `D_work+16` comparison plus the scaled deep-classification corpus decide whether the words are sufficient. A failing Final escalates precision and is never silently accepted.

The shallow kernel is selected below `zoom_log2=14` and perturbation at or above it; 14 is a displayed POLICY justified by math's f32 error argument. Below the switch app accepts the current centre revision and starts shallow refinement without requesting, transferring, or credit-gating an orbit; at or above the switch app requires both the accepted orbit generation and its captured zoom to match current MAIN and the requested zoom exactly, so a 12-to-14 crossing cannot dispatch the carried shallow-generation reference and waits for a freshly generated zoom-14 orbit.

Displayed integral depth is `depth_digits=ceil(max(0,zoom_log2·log10(2)))`; the overlay separately reports `D_floor=ceil(zoom_log2·log10(2)+log10(W))+8`, `D_work=D_floor+ceil(log10(max(max_iter,1)))`, requested bits, and delivered bits after Astro-float’s word rounding and `D_work+16` validation.

### 2.3 Device, surface, and error lifetime

The main wasm start function installs the panic hook before adapter work; immediately after `request_device` returns, app installs non-panicking device-lost and uncaptured-error handlers before any other device or queue call, then performs capability selection and resource initialization inside a validation error scope.

The instance requests only `wgpu::Backends::GL`, the accepted adapter must report `wgpu::Backend::Gl`, limits begin at `wgpu::Limits::downlevel_webgl2_defaults()`, and missing WebGL2 or `EXT_color_buffer_float` produces a typed refusal rather than another backend, format, or blank canvas.

Initialization, selection, and requested measurement each own a generation-tagged validation error scope; captured and uncaptured failures publish page and console text without panic, drop any unpresented surface image, preserve requested controls, and cannot publish into a later generation.

One generation-tagged `SurfaceOwnership` token is the only path to `get_current_texture`; app retains an acquired `SurfaceTexture` after `Presenter::frame` returns, keyed by its `warp_id`, and calls `present()` only after `Presenter::poll` returns the matching completed warp event and its timing endpoint has been captured.

While a warp fence is pending, later refresh callbacks may poll and drain HOT but cannot acquire another image; `Lost` and `Outdated` reconfigure and retry once, `Timeout` skips with a typed status, every other acquisition error releases ownership, and the acquisition path contains neither panic accessor.

Before the first completed scene the surface receives only present’s configured clear colour and the DOM overlay says `waiting for first completed scene` or names the current typed refusal; no diagnostic pattern or incompatible retained image is substituted.

### 2.4 State, controls, and refinement schedule

The worker-owned HOT and MAIN records are `Copy`, `repr(C)`, independently staged, and published through infallible drains sharing one checked u64 epoch; each drain increments the epoch even when unchanged, and compatibility never relies on epoch equality.

Worker HOT remains transport-compatible and carries zoom, the legacy `ρ₁₃`/`ρ₂₄` aliases, and `centre_from_reference_px`; app's `ViewStamp` carries all six object angles, four origin coordinates, and twenty view scalars used by math and present.

MAIN contains requested and delivered cap, delivered precision and orbit length, palette identifier, orbit registry identifier, f64 centre and plane-origin mirrors, the seed-axis words the worker record still carries and the app now pins to `(e₃,e₄)` because no control selects axes any more, `reference_shift_px`, the new minus old accepted reference centre in current pixels, and the `PrecisionMode` discriminant. `PrecisionMode` is `Deterministic=0` or `PictureFast=1`; PictureFast is the requested page default, while changing the mode is incompatible MAIN work that advances the request generation and clears retained and in-flight imagery exactly like changing the iteration cap.

When a reference is accepted, present re-expresses each retained or pending pose from the pose at which that scene was sampled. Generation change alone no longer clears; an object change is slice-incompatible only when its constructed plane span changes, while an in-plane basis rotation and in-plane origin delta are exact chart transforms and an out-of-plane origin delta is refused. Stabilizer membership is evaluated from each pair of constructed planes rather than assigned permanently to a slider: `ρ₁₂` is inert at `O=I` for any absolute origin but can tilt a plane from a different object pose.

The view does not respond to the mouse wheel. A click on the canvas names a point and moves nothing: the pixel under the pointer is converted once, through the plane basis and pixel scale the owner's own navigation arithmetic uses, into the bignum point of the slice it falls on, and the app keeps that point. The crosshair is drawn wherever that point currently projects to, recomputed every refresh, so it rides with its feature under a translation and stays on it through a zoom; a plane-preserving object rotation re-expresses the owner's displacement in the new chart basis and keeps the same stored 4D point, while a genuine plane tilt clears it. When the point leaves the canvas box nothing is drawn and the point is kept. A plain drag is a translation, applied on every pointer move because the displacement from the accepted reference is the HOT field built to carry a pan smoothly. A shift-drag is a box selection, and on release the view zooms so the box fills the screen, with the box's own centre becoming the point; a box under four pixels in either dimension is a click instead. One `scale` slider carries `zoom_log2` over `[−2,120]` and zooms about the stored point — which is what makes the point an accuracy oracle rather than a decoration: click a feature, translate, zoom, and the feature is still under the crosshair or the conversion is wrong, with `crosshair_plane_px`, `crosshair_css`, `crosshair_precision_bits` and `crosshair_point_f64` beside it in the overlay to say by how much. A row loaded into a view box clears the point, because the point belonged to the picture the row replaced. All of them reach the worker through the one pixel-to-plane mapping — `anchor_px_up` for a screen point, `css_from_anchor_px_up` for its exact inverse, `drag_delta_px_down` for a screen displacement, and one `NavigationDelta` per edit — so a click, a translation, a box release and a slider move are four callers of one conversion rather than four derivations of it. The projection is the inverse of the conversion and is tested as such: a crosshair drawn by remembering the pixel it was clicked at is a crosshair that lies the moment the picture moves, and only one re-projected from the point can be measured against the picture at all. The named app-side entries are `set_crosshair`, `crosshair_plane_px`, `pan_px` and `zoom_about_crosshair`, so a slice whose screen map is not this one replaces the two conversions without touching the page. The two rotation sliders stage `θ₁` and `θ₂` independently, the four origin sliders stage the plane origin as MAIN with a centre revision exactly as the retired preset selection did, the precision select stages incompatible MAIN, the VIEW, camera, height, and distance sliders stage HOT alone, and every widget retains its requested value across stale work, degradation, `SceneBusy`, or typed refusal.

Six object sliders, four origin sliders, ten camera-angle sliders, five camera-translation sliders, observer, height, distances, and precision preserve their requested values across stale work or refusal.

Worker recomputes a reference when centre displacement exceeds one quarter of the view extent or `|zoom_log2−reference_zoom_log2|>2`, with worker-owned re-arm hysteresis; app reports trigger and policy separately from device walls.

Kernels defines the mode-dependent ordered ladder. `Deterministic` keeps Preview `ceil(W/4)×ceil(H/4)` at `min(requested_cap,64)`, Interactive `ceil(W/2)×ceil(H/2)` at `min(requested_cap,256)`, and Final `W×H` at `min(requested_cap,4096)`; `PictureFast` uses Preview `ceil(W/8)×ceil(H/8)` at `min(requested_cap,32)`, omits Interactive, and retains that full Final.

The 4,096 cap is a labelled POLICY; if Final allocation fails, kernels selects the smallest power-of-two extent divisor whose cloned-arena trial succeeds, preserves requested extent and cap, and reports the limiting live wall.

App’s schedule begins at Preview after a compatible selection or extent change, asks kernels for the next level under the selected precision mode, may skip a level when newer work makes it irrelevant, and requests at most one scene submission while present’s sole target is in flight; `SceneBusy` leaves the same next level pending.

A kernel level is due when sampling state or the composed neutral-height map changes, when an edge-on stamp becomes mapped, or when refinement remains pending. A scene is due when presentation changes, when a warp exposes source-free pixels, or when a compatible kernel level completes; a control edit that leaves both the slice and `F` unchanged to `1e−12` does not restart the ladder.

An exact edge-on scene stamps its scheduled level even though its all-sky path skips kernel dispatch, so its one Final completion retires the ladder in both automatic and manual scene modes instead of resubmitting unchanged work.

Scene-target extent equals that level’s delivered grid extent, so Preview, Interactive, and Final may reallocate only the available member of present’s two-texture pair; every such event increments `texture_reallocations`, while the retained source remains valid until promotion.

Four facts separate the three ways the reference path can go quiet, because the visible symptom of all three is one frozen overlay: `worker_request_depth` is the depth of orbit requests main has handed the producer, `outstanding_reference_count` and `outstanding_reference_generation` are how many submissions the app is still waiting on and the newest generation among them, and `navigation_pending_depth` is the owner’s coalesced-navigation depth. A producer that never admits shows a non-zero request depth beside a frozen worker epoch; an app that never submits shows depth zero beside a stale requested view; navigation that coalesces without ever resolving shows a non-zero pending depth while both other counts sit at zero.

The loop reports its own liveness on the same terms: `refresh_status` names the terminal state of the last completed turn, `transient_fence_refusals` counts the bounded fence refusals the loop retried rather than died on, `last_transient_refusal` renders the newest of them as its typed text, `presented_view_stale` says whether the image on the canvas belongs to an older requested view than the one now requested, and `loop_stopped_reason` is absent until the loop stops and then carries the one typed cause it stopped for. None of these is a measurement and none is ever given a number that was not observed.

Aggregate rebase totals remain `unavailable` during normal gather-only rendering. Every Final requests numeric `glitch_pixel_count` from present's status census; Preview and Interactive report it as unavailable, and a census callback that has not succeeded when the independent scene fence completes leaves the optional fact unavailable without delaying or refusing the picture.

App publishes `horizon_pixels`/`horizon_fraction`, `uncertain_pixels`/`uncertain_fraction`, `edge_on`, and `map_condition_number`. PictureFast samples and paints positive-denominator MapUncertain pixels; Deterministic may refuse them. Edge-on becomes an all-sky scene, and a completed scene paints every surface pixel as mesh or exterior sky.

### 2.5 One refresh, in exact cross-slice call order

At the start of a refresh or cooperative completion turn, app calls present `Presenter::poll(now_ms)`; it handles `SceneCompleted`, `SceneDropped`, `WarpCompleted`, and `FenceRefused`, updates schedule and facts, and presents or drops an app-held surface image only for the matching `warp_id`.

App then drains HOT, constructs `O`, builds `F`/`M` for the active level from `O`, `Q`, camera translation and observer controls, converts a singular map to `PoseMap::EdgeOn`, builds `PresentHot` and the math-owned `Pose`, selects `WarpValidation`, and calls `Presenter::write_hot`.

App services worker `OwnerEndpoint::next_arrival()` without blocking; for a current orbit it synchronously gives the leased records to kernels’ executor for regional reference upload, registers the resulting `ReferenceOrbit`, calls owner `accept_orbit`, drains MAIN, calls present `Presenter::set_main`, and returns `CreditApplied`, while a stale response returns `CreditStale` without publication. Before submitting new worker work, a latest shallow navigation instead calls owner `accept_navigation_without_orbit`, retains its bignum centre for `CentreSplit`, restarts Preview in the same refresh, and sends no transport message.

If MAIN changed, app asks kernels `JulibrotKernels::plan` and `allocate_grid` as needed, installs the exact three-level schedule, and passes the resulting grid plus `reference_shift_px`, plane origin, cap, palette and view through `PresentMain`; no heap or present bind-group identity is replaced by ordinary MAIN publication.

If mapped kernel work is due, app passes that level's `M` to `encode_shallow` or `encode_perturbation`, then gives present the same sampled map. Edge-on skips kernel encoding and installs an all-sky scene. `SceneBusy` leaves the work pending and allocates no third texture.

App next acquires the sole surface image, calls present `frame`, latches any out-of-source exposure as a reason a scene is due, retains the image under its `warp_id`, and polls cooperatively. Present draws only when the retained `(scene_id,texture_index)` still matches the plan; otherwise this warp is clear-only.

On matching `WarpCompleted`, present has completed its four-byte fence and recorded the ending timestamp, so app calls `SurfaceTexture::present`, releases the token, publishes facts, and schedules another refresh only when animation, input, pending refinement, pending scene completion, or pending worker work requires one.

On matching warp `FenceRefused`, cancellation, device loss, or generation-tagged error, app drops the unpresented image, releases the token, preserves controls, and prints the typed event; it never calls `present()` inside either scene or warp measurement. Dropping the image is not the same as giving up on the page: whether the refresh loop continues is decided by §2.7’s classification of the refusal, not by the fact that one image was discarded.

### 2.6 One zoom step, in exact cross-slice call order

A target click stores its mapped screen anchor and changes no projection. A plain drag maps its endpoint difference through `M` and pans; a Shift-box release maps its centre and applies `Δq=log₂(min(W/w_box,H/h_box))`; a scale move applies the requested `zoom_log2` delta about the stored target. The worker then atomically updates the bignum centre and displacement for edits that move or scale the chart.

The next refresh performs `poll → drain_hot → construct_plane/screen_to_plane/Pose → write_hot`; `Warp::reproject` composes retained screen through `M_from`, compatible plane translation, and current screen through `M_to⁻¹`, and `HotUniform` receives the final rows and current map.

If source identity, cap, precision, slice, residual, solve, or one-pixel error bound fails, `source_valid=0` clears until the next scene. Horizon is exterior, MapUncertain remains sampled in PictureFast, and a reference shift rebases the sampled retained pose.

When a deep edit reaches or crosses `zoom_log2=14`, the owner endpoint writes the nine-kind JBL1 `OrbitRequest` into a request-pool buffer, transfers it, and the producer admits it under the 250,000-microsecond-per-second credit policy, computes in bounded chunks, validates at `D_work+16`, and returns an orbit or typed cancellation/error. A shallow edit bypasses this whole path, including an older deep submission in app's wait set, while any late old response remains generation-stale and returns credit without publication.

App handles the response after the next present poll and HOT write, uploads and registers a current orbit before accepting MAIN, returns its lease credit exactly once, installs Preview/Interactive/Final, and submits Preview when present has an available scene target.

The next completed scene atomically promotes its texture, pose, grid, palette, generation, measured fence and level; later due turns submit Interactive then Final, while every intervening surface frame warps the newest compatible completed scene.

### 2.7 Measurement and never-hang policy

Present owns both four-byte fences: scene cost begins before scene uniform writes and encoding, warp cost begins before the 288-byte HOT write and warp encoding, and each ends when its mapped fence completion is observed; app presents afterward.

No timestamp query is requested; present’s `poll` performs at most one `device.poll(wgpu::Maintain::Poll)` per pending fence per call, counts every poll, checks generation and device loss, refuses at 4,096 polls or 30,000 ms, and relies on app’s zero-timeout yield between calls.

The bounded fence is the never-hang mechanism; abandoning the page is not, and the two must not be confused. A `Deadline` or `PollLimit` refusal on either fence says only that the bounded observation window closed before the fence did, which is exactly what a background-throttled tab manufactures: the wall keeps running at full speed while the callback queue is served once a second. The GPU in that case is healthy and the next frame would have completed. So app counts the refusal, displays it, retires the refused submission, and retries the same work: the same refinement level stays due, and a refused warp re-arms the run so that the next surface image is actually requested. A `Cancelled` refusal is treated the same way, since app or presenter cancelling its own submission is a reason to submit again, never a reason to stop.

Only two things stop the refresh loop: device loss — an uncaptured error, a lost device, or a fence callback that itself failed, which is a `Device` refusal — and a typed worker refusal, which is every worker refusal that reaches app, the credit shaper’s `Delay` being a producer-internal wait that surfaces as a pending request depth rather than as an error. A typed refusal that escapes one refresh turn is latched: the loop stops, `loop_stopped_reason` carries that one cause, `app_needs_refresh` answers false and keeps answering false, and the page states the typed cause instead of restating a broken invariant sixty times a second.

`app_needs_refresh` is true while measurement, fences, surface ownership, worker work, refinement, run, exposure fill, or presented-view staleness remains. Staleness compares the slice plus the composed forward map at height zero, so an inert edit such as the pinned `d₅` fixture does not restart refinement, while an exposed warp cannot leave the loop asleep before its filling scene completes. That map is always solved at the requested extent, never at the level the ladder happens to have prepared. A screen map's perspective row scales with the grid width, so one unchanged requested view has a different map at every level, and stamping the prepared one makes the ladder's own progress read as a control change: the schedule then restarts at Preview forever. The symptom is invisible while the warp still has a usable source and total once a horizon crosses the frame, because a cross-extent reprojection past a horizon is refused, the refused plan paints the clear colour, and its exposure latch restarts the ladder again.

Scene policy is `auto` by default and preserves that scheduling unchanged. In `manual`, a material control change still drains and writes HOT and requests the bounded warp needed to show the new pose, but a refused `ClearOnly` plan with a retained picture becomes `HoldStale`: the last accepted best remains visible, unmoved and explicitly stale, instead of yielding a permanent clear while no scene is scheduled. MAIN changes still request their reference orbit, but the Preview-to-Final ladder remains paused until `update-scene` restarts it for the current pose; the initial page scene remains automatic, and returning to `auto` with an update pending restarts Preview immediately. `scene_update_pending` is a fact, not an outstanding-work term: once the requested warp or hold is presented, manual mode may leave it true while `app_needs_refresh` becomes false, so the checkbox cannot latch the cooperative loop.

A draft never displaces a better picture. After HOT planning, any accepted warp from a compatible higher-level retained scene sends the schedule directly to Final, skipping Preview in PictureFast and Preview plus Interactive in Deterministic; this includes `ReliefRedraw` only for pure height or `d₅` changes with all other sampling inputs fixed and for compatible motions whose retained and destination 5D camera rotation and translation are neutral. App submits that redraw before the Final kernel may overwrite the shared DATA grid and leaves it on the surface while Final is in flight. An exposed accepted warp retains the sharp source over its covered region and leaves only permitted disocclusion temporarily at the distinct clear colour; an over-ceiling non-neutral cross term is refused. The first scene and any refused, slice-incompatible, or MAIN-incompatible warp keep the ordinary Preview-first ladder. The same rule applies after the manual update button starts a ladder, while manual mode without an update presents and holds the redraw; `draft_skipped_count` counts omitted levels, and `last_draft_skip_reason` distinguishes covering from exposed acceptance without becoming a never-hang work term.

The page owns the other half of the same policy, because a loop that answers true forever is only as live as the clock that asks it. The page drives one turn per schedule and guards the schedule with a pending flag, and that flag is exactly where a browser can strand it: a `requestAnimationFrame` queued while the tab or the pane is not painting is never called, so a flag cleared only inside that callback outlives the frame it was guarding and every later schedule returns immediately while `app_needs_refresh` still answers true. The page therefore clears the flag on the way *into* a turn rather than on the way out of a callback, retires the outstanding schedule whenever the document becomes visible again or the window receives `pageshow` or `focus`, and stands one 250 ms timer behind every schedule. That timer runs the turn the animation callback did not, and only that: arriving after a callback that already ran, or carrying a ticket a later schedule has superseded, it does nothing, and arriving with no refresh due it still releases the flag, since the callback it was waiting on may never come and the next control move has to be able to schedule. The two clocks cannot both drive one schedule — whichever arrives first clears the flag, and the other finds it clear.

The timer is a floor under a stopped clock, never a second animation clock: at 250 ms it cannot pace a picture, and when the animation callback is healthy it never runs a turn at all. The page publishes `frame_schedules`, `frames_from_raf`, `frames_from_fallback`, `frame_latch_clears`, and `frame_loop_wakeups` as page facts, so which clock drove a turn is a reported number rather than a claim, and a hidden pane returning to a completed picture can be told apart from one that was woken by a control move. `page_contract` pins the structure the numbers depend on — the flag cleared on frame entry under the ticket guard, the single 250 ms timer, the single animation request, the three wake-up listeners, and the five counters — while the numbers themselves are browser observations and are only ever obtained by replaying the page.

The timer probe performs at most 4,000,000 consecutive `performance.now()` reads, stops after 32 positive transitions or 500 ms, and uses the smallest positive transition `Q`; no positive transition makes timing unavailable without preventing requested rendering.

After initialization, texture reallocation, or pipeline creation, the first completed warp and first completed scene are labelled warm-up and excluded; the second fenced frame decides the 100 ms policy, with `>100 ms` selecting single-frame-on-demand and `≤100 ms` permitting continuous refinement. Nothing animates on its own under either policy: the geometry has no clock, so an untouched page reaches a fixed image and the loop is allowed to go quiet, which is what makes an unattended canvas pixel-identical minutes apart.

An admitted measurement performs three named untimed warm-ups and 15 samples, repeats the exact submission until a batch spans at least `32Q`, caps target at 250 ms, repeats at 4,096, and suite wall at 30,000 ms, then reports the middle sorted median and nearest-rank p95 at `ceil(0.95n)`.

Every warm-up, decision frame, adaptive candidate, repeat count, fence wall, fence-wait subset, poll count, cancellation and single-frame observation remains visible; browser values remain `requires visible replay` until a visible replay supplies them.

Worker `compute_us` is separately measured from centre decode through the standalone-buffer copy, includes cancelled work, and is never substituted for scene wall, warp wall, end-to-end latency, credit, or browser transit.

### 2.8 Page, worker module, and deployment

The page exposes crosshair click, translation drag and shift-drag box selection on the canvas, and one control per view degree of freedom: two plane-angle sliders, four plane-origin sliders each paired with a number box on the same value, two VIEW-angle sliders, two camera-angle sliders, one height slider, two perspective-distance sliders, and one `scale` slider carrying `zoom_log2`, beside iteration-cap and Classic/Ember/Ice palette selectors, explicit one-frame and measurement controls, the two view boxes with their row lists and their morph slider, followed by one select labelled `precision` with `Deterministic` and default-selected `PictureFast`, one canvas, one status element, and one DOM facts overlay. The page exposes crosshair click, translation drag and shift-drag box selection on the canvas, and one control per view degree of freedom: six object rotations, four plane-origin sliders each paired with a number box on the same value, ten camera rotations, five camera translations, observer yaw and pitch, height, two perspective distances, and one `scale` slider carrying `zoom_log2`, beside iteration-cap and Classic/Ember/Ice palette selectors, a checked-by-default `auto-scene` checkbox, `update-scene`, explicit one-frame and measurement controls, the two view boxes with their row lists and their morph slider, followed by one select labelled `precision` with `Deterministic` and default-selected `PictureFast`, one canvas, one status element, and one DOM facts overlay. Camera translation spans `[−8,8]` chart units because the neutral chart spans four units and the range can cross it and retreat past either perspective pole.

The page is one viewport-filling stage rather than a column, because a control you cannot move while the picture is on screen is a control you cannot use. The named regions are `masthead`, `morph-bar` — the A-to-B transition across the full width above the view — `box-a` and `box-b` in the side columns, `stage-view` between them, `zoom-bar` carrying the `scale` slider as the wide bar directly below the picture, and `dash`, the dense control strip and the collapsible facts panel, which is the only region that scrolls: the render surface holds the middle row at every viewport and never scrolls away. The canvas keeps its 960x540 delivered grid and is scaled by CSS alone, its box sized to the shorter of the centre column's two extents so the 16:9 the pointer conversion assumes is exact; the viewer's frame is an inset shadow rather than a border for the same reason, since a border would take two pixels out of that ratio. Below the width at which three columns fit, the stage becomes one column and the page scrolls. The target marker and the rubber-band rectangle are DOM elements positioned over the canvas, never geometry in the scene pass; the scene pass has one pipeline and one depth buffer and gains nothing by drawing an overlay the page can position exactly.

A preset is a row of control values and nothing else, and it is no longer a thing of its own: the built-in rows and the user's saved rows are one list, offered on each side of the view. `app_preset(id)` returns a built-in row as the exported control record, and the page reads them in order until the app runs out of them, so a row added there appears on both sides with no edit to the loader. Choosing a row on a side applies it and makes it that side's view, which is what the load buttons did on their own; a built-in row names every control but the centre and the depth, so what the side then holds is the whole row the app is showing rather than the fragment that was chosen. A save asks for a name, refuses an empty or already-taken one with a visible message, captures the current row, adds it to both sides' lists and selects it on the side it was saved from; the named rows and the two selections persist in `localStorage` through the same wrapped reader and writer the boxes use. Built-in rows are never deletable and a saved row deleted on one side leaves both. Each box and the sampling group carry a copy control that puts the row on the clipboard as its exact JSON on one line and the box's own summary on the next, the boxes copying what they hold and `copy-current` copying the live control state, so a broken picture is reported as the row that produced it rather than as a description of it; the row is stringified whole and never assembled field by field, and a clipboard that is absent or refuses falls back to the same text selected in a read-only textarea. A row is written into the controls field by field, by name — the control that carries a field is the one whose id is the field's name with underscores as dashes, with `height_scale` and `zoom_log2` the two exceptions — and then applied through the same handlers a user's own movement reaches, so there is exactly one path from a control value to the worker, no row can set a state the controls cannot express or leave, moving a control after a row morphs one row continuously into another, and a row that grows a field reaches a slider named after it with no edit to the loader at all. The rows are Mandelbrot, Julia at `c₀=(−0.8,0.156)`, and a relief row for each; the relief rows differ only in `h=1`, `θᵥ=(0.6,0.97)`, and observer `(0.349,0.262)`, which is the orientation the retired fixed mount had, now a row of numbers a user can leave.

Mandelbrot carries `O=I`, `q₁₃=q₂₄=−π/2`, and `t=0`; Julia at `c₀=(−0.8,0.156)` carries object `ρ₁₃=ρ₂₄=−π/2`, `Q=I`, and `t=0`. Relief rows retain those facing values and their established tilt.

Every control lands on `input` except the four origin coordinates, which land on `change`. The origin is MAIN work — releasing it asks the worker for a new reference orbit — and one request per pixel of a drag would be a request storm; the paired number box gives exact entry either way.

One wasm module is instantiated on the main thread and in the module worker, whose entry is `worker_main`; fetch caching avoids a second network payload, but two wasm instances and separate linear memories remain displayed costs.

JavaScript glue, wasm export, worker bootstrap, and wire protocol use `JULIBROT_ABI_VERSION=3`, while the independently pinned loader query remains `v=1` in URLs `./pkg/ember_lab_julibrot.js?v=1`, `./pkg/ember_lab_julibrot_bg.wasm?v=1`, and `./worker.js?v=1`; the browser owner creates that worker and withholds every orbit-pool transfer until the object handshake accepts ABI three, while any disagreement becomes typed `VersionSkew` before orbit work.

The loader query version is deployment cache policy rather than the wire ABI and remains pinned to `v=1` by the page contract; protocol changes bump `JULIBROT_ABI_VERSION`, and a cached old page either obtains matching artifacts or receives the typed skew refusal.

Existing wgpu lab bundles are approximately 4.5 MB; release gates record exact wasm and JavaScript byte counts, the overlay reports them beside two-instance memory, and the approximation is never substituted for this lab’s unbuilt artifact.

### 2.9 Two view boxes, one morph slider, and saved views

A view is one row of control values. `SavedView` carries six object angles, four origin coordinates, ten camera angles, five camera translations, yaw, pitch, height, two distances, `zoom_log2`, and the authoritative encoded centre; the finite centre mirror is readout-only. Loading and morphing decode the bignum centre, and older stored rows without `camera_translation` receive the neutral zero array before application.

Two boxes, A and B, each hold at most one `SavedView`. Each has a button that saves the current row into it and a button that loads it back, and each shows a compact readout of what it holds: angles in radians to three places, origin to three places, `zoom_log2`, and the centre to as many digits as that zoom needs. Presets fill the current row before it is saved, so a preset is a convenient starting point for a box rather than a second kind of thing a box can hold.

One slider `t` runs from 0 to 1 and is labelled A ↔ B. It is disabled until both boxes hold a view. Moving it asks math for `lerp_view` at that `t`, writes every field of the interpolated row into its control element, and then applies the row through the same handlers a user's own movement reaches — the one path from a control value to the worker, exactly as a preset does. There is no second path and no interpolation that the controls cannot express or leave: releasing `t` anywhere leaves a live row a user can carry on moving by hand.

The morph runs at more bits than either endpoint — one Astro-float word beyond the deeper of the two — so that centres separated far below the deeper precision still move step by step rather than snapping. Those bits belong to the arithmetic and not to the row: a row is installed as the viewer's own centre *and* its reference, and a displacement against a reference is refused outright when the two precisions differ, so a row handed back at working precision stops the loop on the slider's first step with a typed math failure. The interpolated centre is therefore rounded back to the deeper endpoint's precision before it becomes a row, which is exact at both ends and leaves the sub-binary64 step intact in between.

`t` is page and facts state. It is not persisted and it is not a field of `SavedView`, because a saved view is a place the picture can be, while `t` is where the user currently is between two of them; saving it would make loading a box ambiguous about which of the two rows it meant.

Origin morphs remain MAIN work. In-plane changes may reproject as exact pan when a compatible retained source survives; out-of-plane changes are different slices and correctly clear until their scenes arrive.

The boxes persist per viewer in `localStorage` under one key per box. Every read and every write is wrapped, because a private window, cleared site data, or a browser configured to refuse storage makes the accessor itself throw; the page renders correctly with no stored value, treats a malformed or version-mismatched record as absent, and never reports a box as holding a view it could not decode. The facts overlay shows which boxes hold a view and the current `t`.

## 3. INTERFACES

All wire and GPU records are little-endian, every listed reserved word is written as zero, all RGBA32F lanes are IEEE-754 binary32, byte offsets are from record start, and CPU-only semantic records explicitly have no serialization-by-layout promise unless marked `repr(C)`.

### 3.1 Math-owned records and functions

`CentreF64` is `{ coords:[f64;4] }`, finite, 32 native bytes, and display/pose state without deep arithmetic authority; `CentreSplit` is `repr(C) { hi:[f32;4], lo:[f32;4] }`, exactly 32 bytes at offsets 0 and 16.

`Plane` is `repr(C,align(16)) { basis_u:[f32;4], basis_v:[f32;4] }`, exactly 32 bytes at offsets 0 and 16; it has no origin field, and the retired origin field name does not appear in this contract.

`ObjectAngles` carries six object rotations in product order; `PlaneAngles` is the legacy two-field adapter to `ρ₁₃` and `ρ₂₄`.

`NavigationDelta` is math’s CPU-only `{ pan_canvas_px:[f64;2], zoom_delta_log2:f64, anchor_canvas_px:[f64;2] }`; both pixel vectors are canvas-centred with positive y upward.

`EscapeParams` is `repr(C) { max_iter:u32, bailout:f32 }`, exactly 8 bytes at offsets 0 and 4; `max_iter>0` and `bailout` is the squared radius fixed to `256.0`.

`Pose` is `{epoch,orbit_generation,plane,object,plane_origin,zoom_log2,view,grid_width,grid_height,map,centre_from_reference_px}`; `ViewControls` is `{camera:[f64;10],camera_translation:[f64;5],camera_yaw,camera_pitch,height_scale,distance_five,distance_four}`. `PoseMap` is `Mapped(Homography)` or `EdgeOn`; none has a byte ABI.

Math exposes `construct_plane(object)`, `screen_to_plane(object,view,zoom_log2,width,height,aspect)`, `navigation_delta(map,drag,zoom,anchor)`, split/scale/precision and escape oracles, and `warp_matrix(from,to)` with typed checked results.

`ScaleSplit` is math’s `{ mantissa:f32, exponent:i32 }` with `mantissa∈[0.5,1)`; `PrecisionPlan` is `{ floor_digits:u32, working_digits:u32, requested_bits:u32, policy_digits:u32 }`, and the policy ceiling is 300 decimal digits.

`ReferenceOrbitRecord` is an 8-byte transfer record: byte 0 `re` and byte 4 `im`; app expands each point to one 16-byte heap RGBA32F texel `(re,im,0,0)`. Index zero is `Z₀`, stored indices are `0..max_iter−1`, and `length=min(max_iter,escape_index+1)`.

`EscapeGridRecord` is one 16-byte RGBA32F texel: byte 0 `smooth_iter`, 4 `escaped`, 8 `rebase_count`, and 12 `glitch`; flags are exactly 0 or 1, the count is integer-valued, escape stores `n+1−log₂(log₂|zₙ|)`, and non-escape stores `−1.0`.

### 3.2 Kernels-owned uniform tables

`ShallowUniform` is exactly 144 bytes and 16-byte aligned, with the following owner-defined layout.

|Byte range|Field|Type|Meaning|
|---------:|-----|----|-------|
|0–15|`basis_u`|`[f32;4]`|Corrected PLANE basis u|
|16–31|`basis_v`|`[f32;4]`|Corrected PLANE basis v|
|32–47|`screen_to_plane_row_0`|`[f32;4]`|First padded `M` row|
|48–63|`screen_to_plane_row_1`|`[f32;4]`|Second padded `M` row|
|64–79|`screen_to_plane_row_2`|`[f32;4]`|Denominator `M` row|
|80–95|`centre_hi`|`[f32;4]`|`CentreSplit.hi`|
|96–111|`centre_lo`|`[f32;4]`|`CentreSplit.lo`|
|112–143|scalar tail|mixed|scale, extent, cap, bailout, level, zero padding|

`PerturbUniform` is exactly 112 bytes and 16-byte aligned, with the following owner-defined layout and no centre field.

|Byte range|Field|Type|Meaning|
|---------:|-----|----|-------|
|0–15|`basis_u`|`[f32;4]`|Corrected PLANE basis u|
|16–31|`basis_v`|`[f32;4]`|Corrected PLANE basis v|
|32–47|`screen_to_plane_row_0`|`[f32;4]`|First padded `M` row|
|48–63|`screen_to_plane_row_1`|`[f32;4]`|Second padded `M` row|
|64–79|`screen_to_plane_row_2`|`[f32;4]`|Denominator `M` row|
|80–111|scalar tail|mixed|mantissa, extent, cap, bailout, orbit length, level, exponent|

The inherited `DispatchHeader` is 16 bytes `{global_base:u32,valid_length:u32,padding:[u32;2]}` and the input resource record is 16 bytes `{directory_index:u32,logical_len:u32,0,0}`; dynamic offsets select immutable prefix headers without mutating their bytes.

### 3.3 Kernels-owned refinement, grids, and callable API

`GridExtent` is `repr(C) { width:u32,height:u32 }`, exactly 8 bytes; both values are nonzero and their product must fit u32.

`RefinementLevel` is `repr(u32)` with `Preview=0`, `Interactive=1`, and `Final=2`; `EscapeGrid` is `{ span:DataSpan,width:u32,height:u32,level:RefinementLevel }`, where `width·height` is the initialized dense prefix of the Final-capacity span.

`LevelSpec` is `{ level:RefinementLevel,extent:GridExtent,iteration_cap:u32 }`; `RefinementPlan` is `{ requested_extent:GridExtent,delivered_extent:GridExtent,extent_divisor:u32,requested_max_iter:u32,delivered_max_iter:u32,page_side:u16,levels:[LevelSpec;3] }`.

`KernelMode` is `repr(u32)` with `Shallow=0` and `Perturbation=1`.

`DispatchFacts` is `{ owner_epoch:u64,precision_mode:&'static str,mode:KernelMode,level:RefinementLevel,requested_extent:GridExtent,delivered_extent:GridExtent,requested_max_iter:u32,delivered_max_iter:u32,active_pixels:u32,worst_case_pixel_iterations:u64,page_passes:u32,copy_commands:u32,gpu_copy_bytes:u64,logical_heap_bytes:u64,reserved_heap_bytes:u64,scratch_bytes:u64,orbit_generation:Option<u32>,orbit_length:u32 }`; `owner_epoch` is observation attribution and never a compatibility key, while `precision_mode` is required provenance.

`ReferenceOrbitInput` is the borrowed semantic record `{ span:&DataSpan,generation:u32,length:u32,precision_bits:u32,precision_mode:&'static str }`; `length=span.logical_len`, and current-generation plus mode validation precedes dispatch.

`JulibrotKernels::new(executor:&mut GpuKernelExecutor)->Result<JulibrotKernels,KernelError>` registers exactly the shallow and perturbation dialect-v2 pipelines; `JulibrotKernels::plan(executor:&GpuKernelExecutor,requested_extent:GridExtent,params:EscapeParams)->Result<RefinementPlan,KernelError>` applies the exact levels, policy and cloned-arena delivery arithmetic.

`JulibrotKernels::allocate_grid(&mut self,executor:&mut GpuKernelExecutor,plan:&RefinementPlan)->Result<EscapeGrid,KernelError>` allocates one Final-capacity span and immutable prefix headers for all three levels without partial publication.

`JulibrotKernels::encode_shallow(&self,executor:&GpuKernelExecutor,encoder:&mut wgpu::CommandEncoder,grid:&mut EscapeGrid,owner_epoch:u64,precision_mode:PrecisionMode,level:RefinementLevel,plane:&Plane,centre:&CentreSplit,pixel_scale:f32,params:EscapeParams)->Result<DispatchFacts,KernelError>` packs the owner table and encodes one logical level.

`JulibrotKernels::encode_perturbation(&self,executor:&GpuKernelExecutor,encoder:&mut wgpu::CommandEncoder,grid:&mut EscapeGrid,owner_epoch:u64,precision_mode:PrecisionMode,level:RefinementLevel,plane:&Plane,scale:ScaleSplit,params:EscapeParams,reference:ReferenceOrbitInput)->Result<DispatchFacts,KernelError>` packs mantissa and signed exponent and encodes one scaled logical level.

`JulibrotKernels::free_grid(&mut self,executor:&mut GpuKernelExecutor,grid:EscapeGrid)->Result<(),KernelError>` transactionally returns the span after present relinquishes it; the error set is `InvalidExtent`, `ArithmeticOverflow`, `InvalidEscapeParams`, `UnknownLevel`, `MissingReference`, `StaleReference`, `ReferenceLengthMismatch`, `ReferencePrecisionMismatch`, `Heap`, `Register`, `Dispatch`, `OutputTransferUnsupported`, and `DeviceLost`, with generation and span identity rather than epoch equality deciding staleness.

### 3.4 Worker-owned wire header, trailer, and messages

Every transferred buffer starts with `MessageHeader`, eight little-endian u32 words and exactly 32 bytes: offsets 0 `magic`, 4 `version`, 8 `generation`, 12 `kind`, 16 `length`, 20 `precision_bits`, 24 `compute_us`, and 28 `credit_us`.

`magic=0x314c424a` is byte string `JBL1`, wire and module ABI `version=3`, and message kinds and `length` meanings are exactly these worker-owned values; this version is independent of the loader query version.

|Discriminant|Name|Direction and pool|`length` meaning|
|-----------:|----|------------------|----------------|
|1|`OrbitRequest`|main → worker, request|requested `max_iter`|
|2|`RequestReturn`|worker → main, request|zero|
|3|`OrbitResponse`|worker → main, orbit|stored orbit-entry count|
|4|`CreditApplied`|main → worker, orbit|zero; generation installed|
|5|`CreditStale`|main → worker, orbit|zero; generation discarded|
|6|`OrbitCancelled`|worker → main, orbit|zero; measured stale work charged|
|7|`ChannelError`|either direction in owned buffer|four words in `ErrorRecord`|
|8|`Shutdown`|main → worker, request|zero|
|9|`ShutdownAck`|worker → main, request|zero|

The last 16 bytes are immutable `PoolTrailer {pool:u32,slot:u32,capacity_bytes:u32,trailer_magic:u32}`, with `trailer_magic=0x544c424a`, request pool 1, orbit pool 2, and slot 0 or 1; all four buffers preserve it bit-exactly.

`ErrorRecord` at byte 32 is `{code:u32,detail:u32,requested_bytes:u32,available_bytes:u32}`; codes are 1 `BadMagic`, 2 `BadVersion`, 3 `BadKind`, 4 `BadLength`, 5 `BadTrailer`, 6 `CentreEncodingWall`, 7 `GenerationExhausted`, 8 `EpochExhausted`, 9 `TimingOverflow`, 10 `BufferStarved`, and 11 `MathFailure`.

`OrbitRequest` is `{ generation:u32,centre:EncodedCentre,depth_digits:u32,precision_bits:u32,max_iter:u32,precision_mode:PrecisionMode,reason:OrbitReason }`; its body at byte 32 is `{depth_digits:u32,reason_bits:u32,centre_revision:u32,limb_word_count:u32,coordinates:[CoordinateDescriptor;4],precision_mode:u32,limbs:[u32;limb_word_count]}`, with descriptors at bytes 48, 64, 80 and 96, precision mode at byte 112, and limbs beginning at 116.

`reason_bits_and_pass` assigns bit 0 initial reference, bit 1 centre threshold, bit 2 zoom threshold, bit 3 max-iteration change, bit 4 precision-mode change, and bits 5–6 the PictureFast reference pass; unknown or contradictory bits are `BadLength` in version three.

Each 16-byte `CoordinateDescriptor` is `{sign:u32,exponent_twos_complement:u32,limb_start:u32,limb_count:u32}`; for nonzero coordinates, value is `(−1)^sign·Σ(limb[k]·2^(32k))·2^exponent`, `sign∈{0,1}`, limbs are least-significant first, the top limb is nonzero, and descriptor ranges are ordered, contiguous, non-overlapping, and exhaustive.

Canonical zero has sign zero, exponent zero, `limb_count=0`, and `limb_start=previous_end`; negative zero, leading zero limbs, unused limbs, range overlap, and out-of-range descriptors are typed refusals.

For current `max_iter=M`, two request-pool and two orbit-pool buffers each have capacity `max(644,64+8M)`; request fit is `116+4·limb_word_count≤capacity−16`, resize occurs only when M changes after all four buffers return, and each replacement increments `allocation_events`.

App pins minimum requestable `max_iter=64`, which admits the 300-digit centre under that request-pool inequality; exceeding capacity remains honest `CentreEncodingWall` rather than a precision-driven resize.

`OrbitResponseView` is `{generation:u32,length:u32,compute_us:u32,precision_bits:u32,admission_credit_us:u32,records:OrbitLease}`; records begin at byte 32, `1≤length≤max_iter`, and returning the lease preserves generation, precision and compute time while setting `CreditApplied` or `CreditStale`, zero length, and updated `credit_us`.

Worker credit is the displayed POLICY `budget_us_per_second=250_000`; admission, refill, overfeed, first unpriced warm-up, cancellation charge, and mode equivalence use worker §§2.5 and 3.3 without app-side reinterpretation.

### 3.5 Worker-owned owner records and APIs

`HotState` is `repr(C)`, 40 bytes and alignment 8: byte 0 `zoom_log2:f64`, 8 `plane_theta_1:f64`, 16 `plane_theta_2:f64`, and 24 `centre_from_reference_px:[f64;2]` in current-zoom pixels along `(u,v)`.

`MainState` is `repr(C)`, 128 bytes and alignment 8, with this exact reconciled layout.

|Byte|Field|Type|Meaning|
|---:|-----|----|-------|
|0|`generation_applied`|`u32`|Latest installed orbit; zero means none|
|4|`centre_revision`|`u32`|Authoritative encoded-centre revision|
|8|`requested_iter_cap`|`u32`|Application request|
|12|`delivered_iter_cap`|`u32`|Installed level cap|
|16|`precision_bits`|`u32`|Delivered reference precision|
|20|`orbit_length`|`u32`|Delivered reference records|
|24|`palette_id`|`u32`|Present-owned `PaletteId` discriminant|
|28|`orbit_id`|`u32`|App registry ID; zero means none|
|32|`centre_f64`|`[f64;4]`|Display/pose mirror in fractal axis order|
|64|`plane_axis_a`|`u32`|Zero-based seed axis in e₁ through e₄|
|68|`plane_axis_b`|`u32`|Zero-based seed axis in e₁ through e₄|
|72|`plane_origin_f64`|`[f64;4]`|Defining origin including Julia `c₀`|
|104|`reference_shift_px`|`[f64;2]`|New minus old reference centre in current pixels|
|120|`precision_mode`|`u32`|`PrecisionMode` discriminant; bytes 124–127 are tail padding|

`OrbitHandle` is `{id:u32,generation:u32}`; MAIN stores its two words separately, app’s registry validates both, and handle zero is invalid.

`ViewerState` is `repr(C)`, 176 bytes and alignment 8: byte 0 `epoch:u64`, byte 8 `hot:HotState`, and byte 48 `main:MainState`; `HotDrain` and `MainDrain` each return the entire record.

`ViewerOwner::new(initial:ViewerState)->ViewerOwner`, `stage_hot(hot:HotState)`, and `stage_main(main:MainState)` allocate nothing; `accept_orbit(response:&OrbitResponseView,handle:OrbitHandle,reference_shift_px:[f64;2])->OrbitDisposition` returns `Applied` only for matching latest generation and otherwise `Stale`, with both outcomes infallible.

`ViewerOwner::configure_navigation(config:NavigationConfig)->Result<(),OwnerError>` installs math’s `BigCentre`, accepted reference centre, `Plane`, and grid width; `ViewerOwner::navigate(&mut self,delta:NavigationDelta)->u32` delegates mutation and displacement arithmetic to math, stages the updated HOT and MAIN mirrors, and exposes any refusal through `take_navigation_error` without publishing partial state. `ViewerOwner::navigation_centre(&self)->Option<BigCentre>` reads that desired centre without consuming the pending submission, and `ViewerController::set_centre(&mut self,centre:BigCentre)` installs a named centre as its own reference, which is what loading a saved view does and what nothing else in the lab needs.

`ViewerOwner::drain_hot()->HotDrain` and `drain_main()->MainDrain` each return the full coherent viewer record and increment the shared epoch exactly once; `snapshot()->ViewerState` is diagnostic and does not increment.

`WorkerChannel::new(config:WorkerConfig,mode:WorkerMode)->Result<(OwnerEndpoint,ProducerEndpoint),ChannelError>` allocates the four buffers; `WorkerConfig` is `{max_iter:u32}`, and app passes minimum max iteration 64 while worker pins budget 250,000 and the four-second return deadline as displayed constants.

`OwnerEndpoint::submit(request:OrbitRequest)->SubmitOutcome` returns `Transferred`, `Coalesced`, or `GenerationExhausted`; `next_arrival()->Option<OrbitResponseView>` is nonblocking/event-driven, and `shutdown()` reconciles all four slots within the fixed return deadline or names the missing pool and slot.

`ProducerEndpoint::run(math:impl ReferenceOrbitComputer)` preserves credit, cancellation and shutdown semantics; `ReferenceOrbitComputer::compute(centre,precision_bits,max_iter,bailout,cancellation)->Result<ComputedOrbit,MathFailure>` supplies reusable orbit records, delivered precision and optional escape index without a transport allocation.

`WorkerFacts` is 64-byte `repr(C)`: `epoch:u64` at 0, followed by `last_applied_generation`, `last_ack_generation`, `orbit_queue_depth`, `shutdown_queue_depth`, `credit_us`, `last_compute_us`, `last_overfeed_us`, `applied_count`, `stale_count`, `cancelled_count`, `allocation_events`, `request_buffers_owned_main`, `orbit_buffers_owned_main`, and `mode` as u32 fields at bytes 8 through 60.

### 3.6 Present-owned records, uniforms, and API

Math defines the twenty-scalar `ViewControls` with neutral rotations and translation zero, height zero, and distances eight; present re-exports it and owns `PaletteId`.

`PaletteRecord` is `repr(C,align(16)) {map:[f32;4],interior_rgba:[f32;4],clear_rgba:[f32;4]}`, exactly 48 bytes; Classic is `{map:[64,0,0.78,1],interior:[0.005,0.005,0.008,1],clear:[0.015,0.018,0.025,1]}`, Ember is `{map:[48,0.02,0.88,1],interior:[0.01,0,0,1],clear:[0.015,0.008,0.005,1]}`, and Ice is `{map:[80,0.55,0.72,1],interior:[0,0.005,0.01,1],clear:[0.005,0.01,0.015,1]}`.

`PresentHot` carries epoch, plane, six object angles, origin, zoom, twenty view scalars, the level map, and centre displacement.

`PresentMain` is the CPU-only record `{epoch:u64,orbit_generation:u32,grid:EscapeGrid,max_iter:u32,palette:PaletteId,plane_origin_f64:[f64;4],centre_revision:u32,reference_shift_px:[f64;2]}`; present applies a given shift once when `centre_revision` advances.

`HotSlot` is opaque `{index:u32,dynamic_offset:u32,epoch:u64}`, created only by `HotSlot::for_refresh(refresh_id,slot_stride,epoch)` with index `refresh_id mod 3` and dynamic offset `index·slot_stride`.

`FrameState` is `{surface_view:&wgpu::TextureView,canvas_width:u32,canvas_height:u32,refresh_id:u64,now_ms:f64}` with no byte ABI; app keeps the corresponding surface image alive until the matching warp completion event.

`HotUniform` is exactly 288 bytes: ten camera sine/cosine pairs at 0–64, camera translation at 80/96, observer at 112, view scale at 128, warp rows at 144–176, screen-map rows at 192–224, exterior and clear colours at 240/256, and flags at 272.

Each homography row stores three coefficients and one zero; present computes it in f64 from math's composed maps, and the shader never receives two semantic poses or an absolute tiny pixel scale.

The ring has three slots, `hot_stride=align_up(288,min_uniform_buffer_offset_alignment)`, total size `3·hot_stride`, one immutable bind group, and one 288-byte regional write per surface refresh.

`SceneUniform` is exactly 160 bytes: grid/span at 0/16, sampled basis at 32/48, sampled `M` at 64/80/96, and palette/interior/clear at 112/128/144.

`SceneFrame` is `{scene_id:u64,pose:Pose,palette:PaletteId,iteration_cap:u32,level:RefinementLevel,extent:[u32;2],texture_index:u32,precision_mode:&'static str,measurement:SubmissionMeasurement}`; `SubmissionMeasurement` is `{kind:SubmissionKind,id:u64,source_scene_id:Option<u64>,sample_class:SampleClass,precision_mode:&'static str,wall_ms:f64,fence_wait_ms:f64,polls:u32}`, with kind `Scene` or `Warp`.

`WarpPlan` carries rows, validity, `AnchorHomography|ClearOnly|HoldStale|ReliefRedraw`, chart residual, measured max/p95 error, and the source scene and texture identities it was solved against; `warp_kind=HoldStale` is the page-visible fact that distinguishes a refused manual hold from an accepted moving warp.

`PresentEvent` is `SceneCompleted {frame:SceneFrame}`, `SceneDropped {scene_id:u64,orbit_generation:u32,reason:DropReason,measurement:SubmissionMeasurement}`, `WarpCompleted {measurement:SubmissionMeasurement}`, or `FenceRefused {kind:SubmissionKind,id:u64,reason:FenceRefusal,polls:u32,wall_ms:f64,precision_mode:&'static str}`.

`DropReason` is `StaleGeneration`, `ReplacedMain`, or `InvalidExtent`; `FenceRefusal` is `PollLimit`, `Deadline`, `Device`, or `Cancelled`; `PresentStatus` is `WaitingForFirstScene`, `ShowingCompletedScene`, `ShowingStaleApproximation`, `ClearForIncompatibleGeneration`, or `Refused(PresentError)`.

`PresentFacts` is `{completed_scene_id:Option<u64>,in_flight_scene_id:Option<u64>,source_generation:Option<u32>,precision_mode:&'static str,delivered_width:u32,delivered_height:u32,delivered_level:Option<RefinementLevel>,iteration_cap:Option<u32>,palette:PaletteId,view:ViewControls,last_scene:Option<SubmissionMeasurement>,last_warp:Option<SubmissionMeasurement>,reprojected_per_scene:Option<u32>,refreshes_without_scene:u64,texture_reallocations:u32,chart_residual:Option<f64>,warp_max_error_px:Option<f64>,warp_p95_error_px:Option<f64>,status:PresentStatus}`.

`PresentConfig` is `{surface_format:wgpu::TextureFormat,min_uniform_buffer_offset_alignment:u32,fence_deadline_ms:f64,max_fence_polls:u32}`; `Presenter::new(device:Arc<wgpu::Device>,queue:Arc<wgpu::Queue>,heap:HeapPresentResources,config:PresentConfig)->Result<Presenter,PresentError>` receives deadline 30,000 ms and poll limit 4,096 after app installs error handlers.

`Presenter::set_main(&mut self,main:PresentMain)` and `Presenter::write_hot(&mut self,slot:HotSlot,hot:PresentHot,validation:WarpValidation)` are infallible; `submit_scene(&mut self,hot_slot:HotSlot,now_ms:f64)->Result<u64,PresentError>`, `frame(&mut self,state:FrameState,hot_slot:HotSlot)->Result<FrameReceipt,PresentError>`, `poll(&mut self,now_ms:f64)->Vec<PresentEvent>`, and `facts(&self)->PresentFacts` preserve present §3.7 semantics.

`FrameReceipt` carries refresh and warp ids, optional source scene, precision provenance, the exposure bit that keeps a scene due, and status, but no timing before fence completion.

`Warp::reproject(last_frame,from_pose,to_pose,precision_mode,validation)` is pure f64 planning. It refuses absent or mismatched source identity, cap/precision mismatch, object-angle incompatibility, more than half a source pixel of out-of-plane origin or chart residual, edge-on/invalid arithmetic, any unprojectable sample in the full 9-by-9-by-5 corpus, and measured maximum above `WARP_MAX_ERROR_PX=1.0`; every accepted plan publishes that complete measured bound, including ordinary PictureFast.

### 3.7 App-owned runtime, surface, and facts

`App::start(canvas_id:&str,status_id:&str)->Result<App,AppError>` performs version handshake, hook/error ordering, GL-only capability selection, heap executor construction, sibling construction, worker start, clear-first-frame setup, and finite initial scheduling.

`PendingSurface` is the app-only record `{warp_id:u64,generation:u32,frame:SurfaceTexture}`; exactly zero or one exists, app presents it only for matching `WarpCompleted`, and every other terminal event drops it unpresented.

`App::refresh(now_ms:f64)->Result<RefreshOutcome,AppError>` executes §2.5, while input callbacks stage worker/palette/view controls and schedule rather than re-entering GPU state; every Promise rejection is caught and published.

`PrecisionMode` has one requested source in `ViewerController`: `BrowserFrameLoop::new` constructs from that value, and every refresh reconciles a changed request through `apply_precision_mode`, where incompatible MAIN work re-plans the kernel ladder, applies the owner's centre width, updates the frame loop's mode, and restarts scheduling at Preview before another scene can be submitted.

`RefreshOutcome` is `{epoch:u64,generation:u32,refresh_id:u64,warp_id:Option<u64>,scene_id:Option<u64>,presented:bool,status:RefreshStatus}`, where status is `Waiting`, `Submitted`, `Presented`, `SkippedTimeout`, `Cancelled`, or `FailedTyped`.

`SavedView` carries six object angles, four origin coordinates, ten camera angles, five camera translations, yaw, pitch, height, distances, zoom, encoded centre, and readout mirror; `SavedView::lerp` composes math's interpolators.

`PageFacts` follows source publication order and page-contract coverage. In addition to the existing transport, precision, ladder, timing, allocation, and delivery fields it publishes `glitch_pixel_count` for a delivered Final, `scene_mode`, `scene_update_pending`, `draft_skipped_count`, `last_draft_skip_reason`, `object_angles`, `plane_origin`, `camera_angles`, `camera_translation`, observer and distance controls, `horizon_pixels`, `horizon_fraction`, `uncertain_pixels`, `uncertain_fraction`, `edge_on`, `map_condition_number`, `warp_exposed_fraction`, `relief_redraw_count`, warp kind and error facts, `level_timings`, and discarded draft pixels/iterations.

The page contributes four more facts of its own to the same overlay: `view_box_a_held` and `view_box_b_held`, `morph_t`, and `view_box_storage`. They are not `PageFacts` fields because none of them is app state — two boxes and a slider live in the document and in per-viewer storage, and pushing them into the app would invent a second owner for them. `morph_t` is `null` while the slider is disabled, and `view_box_storage` names the refusal when the storage accessor threw, so a page with no memory says so rather than silently holding nothing.

`level_timings` is one additional JSON fact containing the last 64 completed or discarded per-level records in oldest-to-newest order. Each record carries `edit`, `level`, nullable `dispatch_us`, nullable `scene_us`, nullable `warp_us`, nullable `worker_reference_us`, nullable `credit_wait_us`, and `discarded`; a new edit inherits only its own accepted worker measurement, scene completion attaches the existing scene-fence wall, a warp attaches only when it names that scene as its source, and a dropped scene sets `discarded=true`. Missing measurements serialize as `null`, never zero.

The current queue has no fence between the kernels SCRATCH-to-DATA dispatch and the following scene pass, so `dispatch_us` is unavailable without adding a fence and remains `null`. `scene_us` and `warp_us` are converted from the two present-owned four-byte fence walls already submitted, with no added fence or wait. Worker reference time comes from the response's measured `compute_us`; the existing `credit_us` response word is admission balance rather than elapsed wait, so `credit_wait_us` remains unavailable until the worker protocol exposes an elapsed boundary instead of relabelling balance as time. The native frame-loop harness drives submission, scene completion, source-named warp completion, and discard events through this ledger and requires a populated record.

Normal rendering leaves both rebase aggregate fields unavailable and publishes the numeric status-one count when the Final census is mapped by the time its independent scene fence completes; all genuinely absent browser facts, including a failed or late census, render `requires visible replay` rather than zero.

`AppError` is the typed union `VersionSkew`, `Capability`, `DeviceLost`, `UncapturedGpu`, `CapturedGpu`, `SurfaceBusy`, `SurfaceSkipped`, `Surface`, `StaleGeneration`, `EpochExhausted`, `GenerationExhausted`, `Deadline`, `CompletionPollLimit`, `Mapping`, `Worker`, `Math`, `Kernel`, `Present`, and `Serialization`; each carries operation and generation or epoch where meaningful.

### 3.8 Heap extraction seams owned by app

The implementation round publicly re-exports the already generic `publish_browser_error` and `install_logging_handler` from `browser_error.rs`, `SelectionEpoch` and `SurfaceOwnership` from `selection.rs`, and `PollCounter` plus `MAX_COMPLETION_POLLS` from `completion.rs`; this is visibility-only and preserves the page status id `status`.

`GpuKernelExecutor` is a public opaque wrapper extracted from the heap lattice runtime; it owns the DATA and four-layer SCRATCH textures, `SpanArena`, immutable heap bind group, descriptor, span-directory, dispatch-header, resource and kernel-uniform buffers, exact-row copy encoding, and live capacity report without changing their algorithms.

The executor surface required by kernels is `GpuKernelExecutor::new(device:Arc<wgpu::Device>,queue:Arc<wgpu::Queue>,config:GpuKernelExecutorConfig)->Result<Self,ExecutorError>`, `capacity_report(&self)->ExecutorCapacity`, `present_resources(&self)->HeapPresentResources`, and `prefix_headers(&self,span:&DataSpan,active_len:u32)->Result<StaticHeaders,DispatchError>` in addition to the allocation, registration and encoding methods already exercised privately by the lattice runtime.

`prefix_headers(span,active_len)` validates `0<active_len≤span.logical_len`, returns immutable `StaticHeaders` for exactly the dense prefix, computes every `{global_base,valid_length,0,0}` record from span geometry, and never relies on conventionally mutating header bytes.

`HeapPresentResources` is `pub struct HeapPresentResources { pub data_view:Arc<wgpu::TextureView>, pub descriptor_buffer:Arc<wgpu::Buffer>, pub span_directory_buffer:Arc<wgpu::Buffer>, pub descriptor_capacity:u32, pub span_capacity:u32, pub handle_capacity:u32 }`; the three Arc identities and capacities remain stable for present’s bind group lifetime.

The executor is extraction, not a fork or new backend abstraction; if implementation requires algorithm changes, duplicate state, new output lowering, or more than moving the paid code behind `pub`, the app lane stops and reports before editing heap further.

## 4. Inherited laws and satisfaction

|Law|App-side satisfaction|
|---|---------------------|
|WebGL2 only|Requests only `Backends::GL`, verifies backend Gl and the minimum floor, and has no WebGPU fallback or promise.|
|Uniform and regional traffic|Each surface frame writes one 288-byte HOT slot; a scene writes one 144- or 112-byte kernel uniform and changed 160-byte scene uniform, while orbit, metadata and textures update regionally.|
|GPU-resident data|Kernels land through paid SCRATCH-to-DATA copy and present samples the typed span; normal rendering reads no grid back.|
|Immutable identities|Heap resources and ordinary bind groups stay stable; texture-pair reallocation is an extent event counted separately.|
|Three-slot dynamic ring|Present owns three slots at runtime-aligned stride and app selects `refresh_id mod 3`.|
|No shared memory|Four transferable buffers and returned credit enforce ownership; same-thread preserves the same state machine.|
|Single surface owner|App holds one token from acquisition until matching warp completion and post-timing present or drop.|
|Readable failure|Panic and error handlers precede GPU use, scopes wrap init/selection/measurement, and every async terminal path publishes typed text.|
|Honesty|Requested, delivered, arithmetic, measured, unavailable, policy, wall, warm-up, poll, worker, warp and bundle facts remain distinct.|
|Never hang|Worker yields, four-second buffer return, 4,096 GPU polls, 30-second fences/suites, finite timer probes, cancellation and checked generations bound progress.|
|Arbitrary zoom|Scaled perturbation carries mantissa and signed exponent and never uploads a tiny absolute f32 scale.|
|Math evidence|Astro-float remains selected; hand-written f64 stays for navigation and warp unless its binding oracle fails.|
|Austere authority|One world, one image scene pass plus warp and a Final-only status census, one heap class, no tick, DAG, shared-memory path, second reference, WebGPU, or gameplay truth.|
|Versioned deployment|One ABI value pins page, glue, wasm, worker and JBL1; mismatch refuses before orbit transfer and deploy is atomic.|

## 5. Oracles and tests

Native math integration tests pin the `O` and `Q` orders and 1e−12 orthonormality, legacy two-angle equivalence, both identity preset pictures, edge-on refusal and the single object-morph crossing, nonzero camera-translation map fixtures, the 9-by-9 forward/inverse oracle, centre split, scaled perturbation, and switch at 14.

Native precision tests pin `depth_digits`, `D_floor`, `D_work`, bit conversion, Astro-float word rounding, the 300-digit policy, D-versus-D+16 convergence, the deep classification corpus, and minimum request cap 64 fitting the centre payload.

Native layout tests assert `Plane=32`, `CentreSplit=32`, `EscapeParams=8`, shallow uniform 144, perturbation uniform 112, both RGBA records 16, HOT 288, scene 160, palette 48, owner and wire records, every offset, discriminant, and zero padding word.

Native wire tests construct all nine message kinds, trailers and `ErrorRecord` values, validate canonical coordinate descriptors, capacity `max(644,64+8M)`, request inequality, exact lease disposition, four-buffer ownership, resize-only-on-cap change, credit 250,000, cancellation charge, same-thread trace equality, and shutdown bounds.

Native owner tests enumerate HOT/MAIN staging and drains, require one shared incrementing epoch without using equality for compatibility, prove the new centre and reference-shift fields across two deep recentres, reject registry generation mismatch, and prove no accepted MAIN state snaps newer controls backward.

Native kernels integration tests pin all three levels and caps, power-of-two degradation, immutable Final span, dense-prefix headers, `RefinementLevel` propagation, one logical dispatch per submitted level, exact SCRATCH rows/bytes, shallow classification and index exact with smooth `≤10⁻⁴`, scaled perturbation smooth `≤2·10⁻³` with exact classification outside math’s error envelope, and the status-one result from reusing the pinned 78-record zoom-12 reference at zoom 14.

Native present tests pin screen-aligned identity, exterior sky coverage, status shading, sampled-pose reference rebasing, source texture identity, slice and half-pixel residual compatibility, exact flat and camera-translation warps, one-pixel enforcement, 288/160-byte layouts, WebGL2 shader translation, three slots, and two textures.

Native app state tests model §§2.5–2.7 at every asynchronous boundary and require poll before HOT drain, HOT write before frame, current deep orbit upload before acceptance, direct shallow acceptance without an orbit, a deep crossing that remains blocked until an orbit with matching generation and exact zoom, scene submission only when due and available, app-held surface keyed by warp id, fence event before present, stale drop, both precision ladders, latest-wins schedule, manual HOT-only reprojection, explicit manual ladder restart, unsuppressed MAIN orbit work, pending-manual return to auto, automatic first scene, and no re-entrant borrow. The 960-by-540 seahorse viewer harness drives the 12-to-14 guard, generates the matching 512-record zoom-14 orbit, renders Final through the CPU perturbation mirror, and requires `glitch_pixel_count=0`. On osprey, the native frame-loop timing oracle measured median time to first shallow scene over five warmed samples as 1,317.104892 ms before the removed paired-orbit wait and 0.000100 ms after it; this is a test-profile wall under idle scheduling, not a browser GPU measurement.

The native `accepted_reference_upload_reuses_scratch_without_copying_the_transfer` frame-loop harness measures a cap-4,096 accepted reference at two per-edit allocations and 65,536 copied transfer bytes before, versus zero per-edit allocations and 32,768 copied bytes after the app-owned 65,536-byte upload scratch is warm; the remaining copy expands each eight-byte transfer record into its required sixteen-byte heap texel.

The fake-clock `accepted_deep_reference_submits_its_first_scene_in_the_accepting_refresh` frame-loop harness measures acceptance-to-first-scene at 16.666667 ms before and 0 ms after; the accepting refresh drains the installed MAIN state and then follows the existing draft-policy scheduling seam without changing its selected level.

The native `facts_refresh_reuses_cached_text_and_borrows_the_timing_ledger` frame-loop harness measures at least eleven snapshot-owned allocations before and zero after across 120 steady-state refreshes; zero is the documented steady-state bound for `PageFacts::snapshot`, with device strings, typed worker facts, policy slices, timing records, navigation mirrors, and cached refusal text borrowed or shared, while the separately returned JSON output buffer remains boundary-owned.

The native `unchanged_refreshes_reuse_one_checked_plane` viewer-refresh harness measures 120 plane constructions before and zero after across 120 unchanged refreshes, with one construction at initialization and exactly one more when the object angles change.

The controller retains two accepted neutral-height `PoseMap` values by every bit-exact input to their construction — all object angles, all VIEW controls, zoom and extent — so an unchanged Preview or Interactive HOT extent and the requested `ViewStamp` extent both stay resident; the ignored `measures_viewer_refresh_map_constructions_before_after` viewer-refresh harness measures 240 constructions before and two after across 120 alternating unchanged refreshes, the frame-loop pin drives distinct Preview and requested extents, and the controller refuses a HOT zoom that is not bit-identical to the requested zoom before constructing the pose.

The native viewer harness pins the manual refusal as `HoldStale` with the retained scene still presented, drives Update scene through Preview, Interactive, and Final, then requires the new Final to become the presented source and the hold fact to clear; companion arms require the same refusal in auto mode to present clear before its Final fill and a bounded manual homography to keep presenting the retained source.

Native page-contract tests pin no wheel handler, click target, plain-drag pan, Shift-box zoom, scale range, all six object, ten camera, and five translation ids, checked `auto-scene` and `update-scene` bindings, both view boxes, row persistence, morph, loader `?v=1`, and ABI 3.

Native page-contract tests pin ABI and URLs, one wasm artifact plus `worker_main`, browser-owner construction and typed skew refusal before orbit transfer, explicit GL-only descriptor, hook and handler ordering, clear-first overlay text, no panic accessor in acquisition, all facts fields, DOM overlay, and approximately 4.5 MB disclosure beside exact release bytes.

Native measurement tests pin separate present-owned scene and warp fences, poll-before-yield, 4,096 polls, 30,000 ms, post-fence present, first-frame labels, second-frame 100 ms decision, timer-probe bounds, three adaptive warm-ups, 15 samples, 32 quanta, 250 ms batch, 4,096 repeats, 30-second suite, median and nearest-rank p95.

`requires visible replay`: GL identity, RGBA32F usages, scratch-copy visibility, height-zero and relief orientation, the continuity of the morph as each control moves, corrected hybrid planes, scaled deep zoom beyond the former f32-underflow boundary, orange glitch diagnostics distinct from magenta contract violations, numeric Final glitch census, clear disocclusion, and a clean console.

`requires visible replay`: rapid pan, zoom, angle, preset, cap, precision-mode, and palette changes while worker, scene and warp work overlap must end at the newest controls, translate smoothly at depth, rebase retained frames across accepted references, and clear only on cap, plane-origin, or precision-mode changes.

`requires visible replay`: worker transfer must detach senders, preserve all four trailers, report one explicit wasm copy, charge credit and cancelled work, allocate only on cap change, and return every buffer within the bounded shutdown window.

`requires visible replay`: every scene and warp observation must display wall, fence subset and polls; warm-ups, second-frame choice, texture reallocations, frames per scene, timer quantum, adaptive repeats, visibility, cancellation and refusal remain readable.

`requires visible replay`: network and memory facts must show one versioned wasm payload fetched or cache-hit for two distinct instances and report exact release bytes plus duplicated live memory.

## 6. Risks and retirement oracles

|Risk|Oracle that retires it|
|----|----------------------|
|Plane rotation still fails to make hybrids|π/2 endpoint and intermediate nonzero-both-subspaces math fixtures.|
|Scaled recurrence loses exponent state|f64 scaled mirror over renormalization, rebase, deep corpus and exact outside-envelope classification.|
|Nonzero-Z₀ rebase shifts the reconstructed pixel|Fixture checks invariant before rebase, after assignment and after the mandatory ordinary advance.|
|One-word reference coordinates are inadequate at deep zoom|D-versus-D+16 records plus deep classification corpus; failure escalates Final precision.|
|Reference acceptance causes a visual snap|Two-deep-recentre owner trace plus visible retained-pose rebase using `reference_shift_px`.|
|A stale orbit or scene publishes|Every-yield transport and app/present event model with delayed old generations.|
|Surface is presented inside its measured interval|Source-order test and event model require matching warp completion before app present.|
|Present API’s asynchronous fence strands the surface|Pending-surface model plus timeout, cancellation, device-loss and mismatched-event tests.|
|Dense-prefix headers expose stale records|`prefix_headers` byte fixtures, poison padding, and Preview→Interactive→Final replay.|
|Heap extraction forks paid behavior|One executor type runs the standing heap golden and Julibrot dispatch with equal identities, rows and errors.|
|Extent churn overwrites retained texture|Two-texture interleaving tests and visible level walk with `texture_reallocations`.|
|Transfer clones, leaks, or starves buffers|Four-slot model, sender-detachment replay, trailer identity and bounded shutdown.|
|Credit becomes invented throttling|Exact worker token-bucket model with 250,000 policy, warm-up, cancellation and overfeed facts.|
|Normal overlay invents rebase/glitch totals|Snapshot test keeps rebases unavailable; the ordered Final census alone supplies numeric `glitch_pixel_count`.|
|Loader combines cached incompatible artifacts|Pinned URL/ABI tests and deliberate main/worker skew refusal.|
|Bundle cost is hidden|Release artifact byte gate and visible two-instance memory report.|
|Math/present type ownership creates a dependency cycle|Compile-time dependency test plus resolution of the `Pose.view` representation before implementation imports.|

## 7. Implementation phases and line budget

Phase 0 extracts and re-exports only the §3.8 heap seams, runs the standing heap golden through `GpuKernelExecutor`, pins `HeapPresentResources` Arc identities, and proves immutable dense-prefix headers, estimated at 300 heap Rust and integration-test lines; the lane stops if the change exceeds visibility/extraction.

Phase 1 creates the app crate, ABI/error types, GL-only device, hook and handler ordering, scoped initialization, single surface owner, pending-surface event bridge, and clear-first-frame path, estimated at 390 Rust and test lines.

Phase 2 integrates the published `ViewerOwner`, owner records, centre/reference displacement fields, stable requested controls, corrected pose construction, and latest-wins HOT/MAIN staging; exact worker channel leases, orbit registry, and current-before-accept upload join Phase 3 when the worker endpoint and kernels upload API are published, estimated at 390 Rust and test lines.

Phase 3 integrates affine object/camera poses, screen-aligned kernel dispatch, mode ladders, dense-prefix facts, power-of-two delivery, present's 288-byte ring and exterior sky, source-bound measured warp planner, asynchronous events, and post-fence surface presentation.

Phase 4 builds the page, one-module worker bootstrap, version handshake, stable controls, requested/delivered facts, unavailable rebase totals, the Final glitch census, DOM overlay, bundle disclosure, and typed status rendering, estimated at 470 HTML, CSS, JavaScript, Rust and test lines.

Phase 5 adds adaptive timing, single-frame policy, finite fence and timer-probe accounting, and replay instrumentation, estimated at 210 Rust, JavaScript and test lines.

Phase 6 adds complete page-contract coverage, release bytes, rapid-interleaving scripts, doc reconciliation, and lint repair, estimated at 130 lines.

The refined app estimate is approximately 2,710 net new lines including the Phase 0 heap extraction; implementation reports actual lines per phase and stops rather than broadening the approved seam.

## 8. Unresolved for implementation review

- Backlog: second-reference correction for a real glitch from an otherwise current orbit is deferred because it requires cluster selection, another worker orbit, regional rerender, and merge ownership; its minimum cost is one additional high-precision reference computation and one regional kernel/presentation pass per corrected cluster, while the current contract reports and paints those residual pixels diagnostically.
- The 250,000-microsecond credit and zoom-14 switch are accepted policies without browser field evidence; implementation may not silently tune them.
- The concrete release script for atomic page/glue/wasm/worker publication remains to be selected, although ABI and refusal semantics are fixed.
- Hidden-page suspension can delay JavaScript before it observes a 30-second or four-second deadline; the first resumed poll refuses, but wasm cannot bound unscheduled browser time.
- Browser internals may copy a transferred buffer even though sender detachment and the one explicit wasm copy are observable; the contract claims ownership transfer, not physical browser zero-copy.
- The enforced one-pixel warp refusal and three palette choices still need visible target-display evidence; the 46.94/31.59 relief envelope must appear only as refused motion awaiting a scene.
- The existing approximately 4.5 MB bundle expectation is not this lab’s exact release size, and no reviewed regression threshold exists yet.

## 9. Joint-review ACCEPTED/ARGUED ledger

|Item|Disposition|App-slice action|
|----|-----------|----------------|
|J1|ACCEPTED|Uses ℝ⁴ `R₁₃·R₂₄`, removes projection/Gram–Schmidt/degenerate stages, pins one f32 rounding, hybrid oracle and unchanged presets.|
|J2|ACCEPTED|Uses `δ←zₙ−Z₀`, reset, count and exactly one advance; nonzero-Z₀ is a pass fixture.|
|J3|ACCEPTED|Adopts scaled perturbation, mantissa/exponent upload, renormalization, f64 mirror and consumed-word sufficiency oracle.|
|J4|ACCEPTED|Pins 32-byte origin-free `Plane` and separate 32-byte `CentreSplit`; retires the former origin field.|
|J5|ACCEPTED|Duplicates kernels' 144-byte shallow and 112-byte perturbation layouts with screen-map rows and their scalar tails.|
|J6|ACCEPTED|Withdraws the 192-byte dual-pose payload and adopts present’s 128-byte CPU-planned homography block and stride.|
|J7|ACCEPTED|Uses math-owned `Pose` and `ViewControls`; present re-exports `ViewControls`, so no dependency cycle exists.|
|J8|ACCEPTED|Withdraws JBRT/four kinds and adopts JBL1, nine kinds, a 16-byte trailer, and the versioned pool capacity.|
|J9|ACCEPTED|Withdraws app’s scalar codec and adopts worker’s four descriptors, 0/1 sign and canonical zero with no limbs.|
|J10|ACCEPTED|Pins 40-byte HOT, 120-byte MAIN, 168-byte viewer, centre displacement, reference shift, retained-pose rebase and cap/origin-only clear.|
|J11|ACCEPTED|Pins eight-byte `EscapeParams {max_iter,bailout}` with squared radius 256.0.|
|J12|ACCEPTED|Withdraws Vec/WallTerm levels and adopts the exact Preview, Interactive, Final array, caps, 4,096 policy and power-of-two degradation.|
|J13|ACCEPTED|Uses `RefinementLevel` enum in grids, schedule, scene frames and facts.|
|J14|ACCEPTED|Adopts present’s API and fence ownership plus the ruled app-held `PendingSurface` keyed by `warp_id`.|
|J15|ACCEPTED|Scene texture extent equals delivered level extent and every reallocation is counted.|
|J16|ACCEPTED|Expands Phase 0 to the six re-exports, `GpuKernelExecutor`, and exact `HeapPresentResources`, with the mandated stop condition.|
|J17|ACCEPTED|Pins `prefix_headers(span,active_len)` and forbids convention-only header mutation.|
|J18|ACCEPTED|Each drain bumps shared u64 epoch and app never uses equality as compatibility.|
|J19|ACCEPTED|Passes and displays 250,000 microseconds per second as implementation-constant POLICY.|
|J20|ACCEPTED|Keeps `c₀` in MAIN plane origin and sets absolute centre to preset origin.|
|J21|ACCEPTED|Rebase aggregates remain unavailable; every delivered Final gets an ordered, mapped and numerically published glitch census.|
|J22|ACCEPTED|Displays integral depth, floor, working, requested and delivered precision separately.|
|J23|ACCEPTED|Pins reference indices and `length=min(max_iter,escape_index+1)`.|
|J24|ACCEPTED|Pins `2·10⁻³` smooth tolerance, propagated envelope, exact outside-envelope classification and boundary fixtures.|
|J25|SUPERSEDED|Withdrew `view_theta_1=t` for `0.4t` with a golden-ratio second angle; both derivations are now retired and each VIEW angle is an independent control.|
|J26|ACCEPTED|Uses shallow below 14 without an orbit, perturbation at or above 14 with a matching orbit, and displays the switch policy.|
|J27|ACCEPTED|Keeps orbit-sized request buffers and `CentreEncodingWall`; minimum request cap is 64 for 300-digit fit.|
|J28|ACCEPTED|Keeps one wasm plus `worker_main`, version-one URLs and typed skew refusal.|
|J29|ACCEPTED|Withdraws 144-byte palette and adopts present’s three exact 48-byte records.|
|J30|SUPERSEDED|Used a math-defined `ViewMode` re-exported by present; the view is no longer an enum, and math defines `ViewControls` in its place.|
|J31|ACCEPTED|Uses separate present-owned fences and delays app presentation until the matching completed warp event, outside the measured region.|
