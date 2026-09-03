# Rendered-tile reprojection for the Julibrot lab

Status: draft design study.

## Decision summary

The cache is a set of rendered tiles captured from many five-dimensional render poses of the same scene; a requested frame reprojects every candidate tile from its own pose through the full five-dimensional camera chain and depth-composites their transformed meshes, so tiles rendered at different angles may overlap and intersect and are layered by nearness.

The cached unit is a screen-aligned fragment of one completed render, not a rectangle of a canonical chart image. Each tile is stamped with the source object `O` and origin, camera `Q` and translation, height, `d5`, `d4`, yaw, pitch, zoom, canvas extent, source pixel rectangle, slice identity, and MAIN generation, and it retains both palette-independent value records and lifted source depth for its samples.

Reprojection is per tile and per mesh vertex. A source pixel plus its depth and source pose reconstructs the lifted point represented by that sample; the requested pose then runs that point through its own full five-dimensional projection and produces a new screen position and true target depth. There is no single global homography warp in this design, including when many tiles happen to share a source pose.

Composition is one depth-tested render pass over the selected transformed tile meshes. True target depth chooses between distinct surface points, while refinement level, sampling density, error bound, and age choose only between competing representations of the same surface point; quality is never added to physical depth.

The chart-space pyramid remains, but only as a semantic and spatial index. It partitions slice and MAIN identity, records each rendered tile's derived chart coverage and density, drives the direct candidate walk, and names the protected coarse backdrop window; it is not the tile's storage geometry or sampling lattice.

The two-dimensional Julibrot slice makes every valid rendered tile a resampling of the same parameterized lifted surface, so chart identity can recognize duplicate coverage and the full projection can reproduce the arena's view-to-view mechanism. A three-dimensional arena slice does not share that completeness: genuine occlusion means one rendered view contains only its visible layer, and a requested view can expose geometry absent from every cached tile. The lab rehearses pose-stamped depth reprojection and composition without claiming to solve arena disocclusion.

The color channel inside each rendered tile is a value map, never palette colors, and it remains paired with the tile's lifted-depth channel. The reprojection layer performs the per-pixel palette lookup, exposure, tone mapping, and output encoding after visibility is resolved; palette and HDR controls are frame inputs, appear in no cache key, and invalidate nothing.

A protected backdrop keeps nine active low-resolution rendered tiles plus three rolling replacements around the requested chart footprint. It is a depth-bearing fallback rendered from broad source views, not a chart image and not a draft allowed to displace better visible content.

## Scope and binding invariants

The picture is a two-dimensional affine plane in the four-dimensional Julibrot model, lifted by one value-derived fifth coordinate and observed by an independent five-dimensional camera. Object controls choose `O` and the plane origin; camera controls choose `Q`, translation, yaw, pitch, `d5`, `d4`, height, zoom, and extent. Every degree of freedom remains a slider and every preset remains a row.

Reprojection is a navigation primitive. Cached rendered tiles may hold the view together while a new render resolves, but a resolving tile adds detail without moving already-correct features, a draft never replaces a better representation of the same surface region, and no admitted warp may exceed the existing 1.0 px screen-error ceiling.

Semantic reuse remains bounded by the stabilizer of the slice. A camera or other presentation change may use any cached render pose of the same slice; a certified plane-preserving object or in-plane-origin change is another parameterization of the same surface; a slice tilt or out-of-plane origin change asks for new content and starts a new partition. This is the after-time-warp boundary stated in `origin/docs/4d-arena:docs/4d-first-engine.md` and reinforced by `origin/docs/4d-arena:docs/4d-torus-world.md` and `origin/docs/4d-arena:docs/4d-content.md`.

The arena's three-dimensional solid slice remains out of scope. This study does not add volume samples, hidden layers, or a neighboring slice, and it does not treat a depth buffer as evidence for geometry that the source view never rendered.

The design preserves the repository's one-way crate layering and uses only the WebGL2 plus `EXT_color_buffer_float` floor in [minimum requirements](../minimum-requirements.md). It assumes nearest RGBA32F value access, `Depth24Plus` raster depth, no float blending, no storage buffers, no timestamp queries, and finite descriptor, uniform, attachment, and draw budgets.

## Rendered-view coordinate model

Let `S` identify the canonical affine slice and let a source render pose be `F = (O_F, o_F, Q_F, t_F, h_F, d5_F, d4_F, yaw_F, pitch_F, q_F, W_F, H_F, M_F)`, where `M_F` is the accepted neutral-height source-screen-to-plane map and the remaining terms are the controls and extent used to render the source view.

After the source scene's own visibility test, a retained source pixel center `s_F` carries the winning value record `r_F(s_F)` and source-depth record `d_F(s_F)` or an explicit no-surface mark. The tile is therefore a post-raster value/depth view fragment: a fold hidden at the source pose is absent even if its pre-raster escape sample existed.

The source-depth record is `d_F=(zeta_F,a_F,b_F,v_F)`: positive linear view distance `zeta_F` after the source lift, both perspective divides, and source observer rotation but before the finite near/far depth map; normalized source-local plane coordinates `(a_F,b_F)` relative to the pose's exact bignum center and scale; and validity `v_F`. It is not the quantized `Depth24Plus` attachment value. In the lab `U_F(s_F,d_F,r_F)` reconstructs the lifted point from the exact pose anchor, `(a_F,b_F)`, and the value-derived height, then requires its recomputed source screen position and `zeta_F` to agree with the retained sample; the arena analogue instead reconstructs a visible three-dimensional slice point from source screen position, linear depth, and its camera convention.

For requested pose `T`, reprojection reconstructs the visible surface sample from `(s_F,d_F,r_F,F)`, derives its canonical chart identity in the lab, applies any certified source-chart-to-canonical transform, changes the presentation-only height to `h_T H(r_F)` when requested, and evaluates the complete target chain `Q_T`, `t_T`, `P5(d5_T)`, `P4(d4_T)`, yaw, pitch, zoom, clip, and viewport. The result is target position `s_T` and true target depth `z_T` for that retained visible point.

In compact form, `x=U_F(s_F,d_F,r_F) in S`, `y_T=(x,h_T H(r_F)) in R5`, and `(s_T,z_T)=Pi_T(y_T)`, where `Pi_T` is the requested pose's ordered `Q_T`, `t_T`, `P5`, `P4`, observer, clip, and viewport chain. Both the target screen location and the physical depth used for composition therefore come from `T`; neither is copied from the source view.

Every admitted vertex therefore follows the same ambient construction as a newly rendered scene vertex. Reprojection is exact at retained visible samples where the value, source depth, reconstruction auxiliaries, and target projection are valid; triangle interpolation between retained pixels is an approximation measured against direct projection and bounded to 1.0 px.

At neutral height the mapping of one source tile may reduce algebraically to a projective map, but that is an oracle and optimization opportunity, not the composition model. The presenter still owns an independently transformed mesh and target depth for every selected tile, and it never substitutes one global frame homography for those meshes.

The current implementation is the migration baseline, not the target: its scene pass already reconstructs each screen-grid sample, lifts it, applies the complete camera chain, rasterizes visibility with depth, and shades LDR color, while its retained warp deliberately samples one LDR texture through a depth-free homography. The new source pass retains post-visibility value and reusable depth instead of LDR color, and the reprojection pass reuses the scene's ambient projection algebra.

## Where the lab and arena coincide

Both systems retain completed views rather than authoritative worlds. A retained unit has a source pose, a finite visible footprint, value or linear shading inputs, depth, holes, and an age; a later presenter reconstructs source samples, transforms them toward a requested view, submits overlapping meshes, and lets target depth establish visibility.

Both systems also share the same hard invalidation boundary. Motion inside the stored slice changes presentation and is eligible for reprojection; changing the slice normal or offset requests points absent from the retained artifact and requires new content before it can be presented truthfully.

The Julibrot lab is simpler because its rendered views sample one known two-parameter surface. Different source angles may sample that surface at different chart positions and densities, but every valid sample has a canonical chart address, so duplicate surface coverage and expected target projection have unusually strong oracles.

That canonical chart also gives the lab a stable same-surface identity for quality ownership. The arena transfer needs an equivalent persistent surface or primitive identity, or another certified equivalence relation, before it can prefer one quality representation over another; physical target depth alone still orders distinct visible points, but approximate equality of depth is not a safe substitute for identity.

The arena's visible artifact is a two-dimensional image of a three-dimensional slice with genuine occlusion. A source depth sample reconstructs the visible surface point along that source ray, but it says nothing about a hidden surface behind it; another source angle can contribute that missing surface, and when no cached view saw it the requested frame must expose a hole and schedule a new render.

Consequently, a successful lab proof establishes pose provenance, full-chain reprojection, true-depth intersection, quality tie-breaking, bounded error, cache selection, and honest holes. It does not establish complete arena coverage, correct hidden-layer recovery, or a substitute for the arena's slice renderer.

## Where rendered tiles live and how the chart indexes them

A rendered tile lives in post-visibility source-screen pixel space. Its physical record grid is initially 256×256, with a 254×254 drawn core and one retained-pixel apron on every edge; its key names the source render pose and integer source pixel rectangle. Adjacent tiles cut from the same rendered view share source edge pixels, but tiles from different views need not share surface samples, axes, or edges.

The durable payload is two equal-length post-visibility arrays: the winning palette-independent escape-value record per source pixel and its lifted source-depth/reconstruction record with validity. No screen color, palette result, exposure result, tone-mapped pixel, hidden source fragment, or target depth attachment is retained as tile cache.

Deep-zoom surface identity remains exact because the source pose carries the bignum center and scale provenance and the reconstruction record identifies the source-local plane point represented by a retained pixel. A finite `f64` offset may drive projection after that mapping is certified, but it is never the equality key for an absolute deep position.

The chart pyramid has an immutable bignum anchor and signed dyadic node coordinates. Publication conservatively derives each rendered tile's canonical chart footprint, density interval, horizon cuts, and uncertainty from its source map and valid sample domain, then inserts the tile ID into every bounded index node required by that footprint. A tile may occupy several nodes, and a node may reference tiles from many render poses.

Pyramid level describes derived chart sampling density, not stored tile dimensions. Two rendered tiles at the same derived level can have different quadrilateral footprints and source poses, and a rendered tile never becomes a dyadic chart rectangle merely because the index bins it there.

For a requested pose, the selector inverse-maps a conservative target screen bound into the canonical chart, walks only intersecting pyramid nodes from coarse fallback toward requested density, gathers their tile IDs, deduplicates them, and then forward-projects each candidate's actual retained mesh bounds. This retains the direct-walk objective: work is proportional to visited query nodes and returned candidates, not total cache population.

Relief, a perspective pole, or an edge-on target can defeat a finite inverse chart bound. In that case the selector may forward-test the bounded active backdrop roster and a bounded recent-pose roster, but it must publish that fallback and refuse any unbounded tile; it may not disguise a full-cache scan as constant-time indexing.

The backdrop window is a 3×3 set of coarse chart-index cells centered on the requested footprint, plus three rolling replacement slots. Each cell is backed by a screen-aligned rendered tile from a deliberately broad source pose whose derived valid footprint covers that cell with guard samples; if the requested cell is not completely covered, only the proven footprint is published.

Choose the backdrop density so one active cell spans between one and two times the longest conservative chart dimension of the current view. Nine cells then span at least three times both current chart-axis extents, with roughly 127–253 retained core intervals across the longest current-view dimension. The source pose and depth remain attached, so rotation, pitch, translation, and zoom-out transform the backdrop as old rendered geometry rather than sampling a fixed chart picture.

Every backdrop tile must pass the normal precision, glitch, source-depth, and placement proofs. If one active reference orbit cannot certify the intended wide region, the delivered backdrop is smaller and facts identify its holes; the design does not trade correctness for a no-clear slogan.

|Property|Current retained scene|Proposed rendered tile|
|--------|----------------------|----------------------|
|Retained unit|One whole LDR screen image|One source-screen fragment with value and lifted depth|
|Pose provenance|One retained pose|Full render pose per tile, with many poses resident together|
|Reprojection|One depth-free screen homography|One full-chain depth-bearing mesh transform per tile|
|Overlap|Only the last image|Arbitrary transformed overlap among source poses|
|Visibility|Already baked into source color|Recomputed per pixel with true target depth for retained samples|
|Chart role|Source screen maps indirectly to the plane|Derived coverage and density index only|
|Palette and HDR|Baked into retained LDR color|Applied per pixel after target visibility|

## Rendered-tile descriptor and heap relationship

Each descriptor has a stable `tile_id` and a `content_key`. The content key contains the canonical slice identity, formula and record ABI, MAIN generation, precision mode, requested-cap semantics, reference-orbit generation under the conservative first policy, and any record interpretation version; it contains no render pose or display control.

The separate `render_key` contains the complete source pose: object angles `O`, plane origin, certified plane basis and canonical transform, exact bignum-center serialization and revision, scale, camera `Q`, five-dimensional translation, height scale, `d5`, `d4`, yaw, pitch, zoom, source canvas extent, and accepted source screen map. The cache lookup key is `(content_key, render_key, source_rect, delivered_rung)`; publication epoch and age are lifetime facts, not pose identity.

Source geometry facts contain the 256×256 physical extent, 254×254 core, apron, post-visibility valid-pixel mask or validity channel, source depth and reconstruction definition, minimum and maximum finite source depth, horizon and pole classifications, and enough mesh topology information to reproduce the rendered fragment without consulting a discarded scene texture or pre-raster grid.

Payload facts contain the generation-checked value `DataSpan`, depth `DataSpan`, logical and reserved bytes for both, initialized prefix, publication fences, and record count. On the current heap floor the two columns are separate RGBA32F spans; a later compact depth format is a distinct capability and ABI, not assumed padding savings.

Derived index facts contain the canonical chart footprint and conservative bound, chart-density interval, pyramid nodes containing the tile ID, bignum anchor digest, source-to-canonical error, and index revision. These facts may be rebuilt from the render pose and sample domain; they accelerate selection but never redefine the stored samples.

Quality facts contain Detail or Backdrop residency, Preview, Interactive, or Final rung, delivered cap, sample density, coordinate and source-depth errors, glitch and uncertain counts, mesh interpolation bound, and a monotonic quality tuple. Age is a deterministic serial and breaks ties only after refinement, density, completeness, and error.

Lifetime facts contain creation, last-selected and last-visible serials, hit count, target-pose reuse count, source-pose family, pin reasons, backdrop-window generation, publication state, and eviction class. Pose diversity is observable rather than inferred from tile coordinates.

A target-placement receipt is keyed by tile ID and requested pose digest and contains transformed screen footprint, target depth interval, mesh class, maximum and p95 screen error, rejected cells, horizon clips, target pole status, same-surface ownership mask revision, and refusal reason. It is replaceable derived state and carries no palette or cached color state.

[GPU resource heap](../gpu-resource-heap.md) already supplies stable RGBA32F DATA bindings, generation-checked handles, typed allocation walls, and square pages. [GPU heap lattice](../gpu-heap-lattice.md) supplies multi-page `DataSpan` ownership, span directories, transactional allocation, fixed executor input and output arity, fixed uniforms, SCRATCH-to-DATA publication, and generation-safe free.

The heap does not supply render-pose identity, source-depth semantics, chart-footprint indexing, candidate deduplication, per-tile mesh reprojection, same-surface ownership, target-depth composition, pose-diverse eviction, backdrop policy, or reprojection oracles. Those remain Julibrot math, present, and app policy above the generic heap.

## Validity, compatibility, and invalidation

A rendered tile is semantically valid for every requested render pose when its `content_key` matches. Source and requested camera positions may be arbitrarily different; pose distance affects projected coverage, error, and usefulness, but it does not change the mathematical value or source depth that was rendered.

“The model has not changed” means the same canonical slice and MAIN generation with compatible value, depth, and record ABIs. The source render pose is deliberately part of tile identity but deliberately not part of semantic validity.

Plane-preserving object changes and in-plane origin moves remain valid after an affine-plane proof maps the source chart into the canonical slice chart. A different parameterization or angle of the same plane creates another render key and can coexist with old tiles; it does not force their eviction.

Camera `Q`, camera translation, yaw, pitch, `d5`, `d4`, zoom, canvas extent, requested height, palette, exposure, tone mapping, and output encoding invalidate nothing. Height changes use the retained value record to rebuild the lifted coordinate before target projection; display changes affect only the current fragment shading.

A slice tilt or out-of-plane origin move changes the canonical surface and starts a new content partition. Version one also treats cap, precision mode, record ABI, and accepted reference-orbit generation as MAIN boundaries; later reference-independent certification may relax only the last of those without changing the rendered-view rule.

A stale span generation, incomplete fence, corrupt source pose, missing source depth, or failed reconstruction makes an individual descriptor unusable, not semantically equivalent. A target pole, missing target-depth proof, or error above 1.0 px makes the tile unplaceable for that request but leaves it resident for later poses.

|Requested change|Semantic validity|Index action|Reprojection action|
|----------------|-----------------|------------|-------------------|
|Any camera, observer, zoom, extent, or height pose|Keep every matching-content tile|Query the new derived footprint|Transform each candidate mesh through the full target chain|
|Plane-preserving `O` or in-plane origin change|Keep after slice-equivalence proof|Transform request through the canonical chart|Reconstruct from each source pose and project to target|
|Palette, exposure, tone, or output change|Keep|None|Shade current fragments with new frame inputs|
|Slice tilt|Invalidate the active partition|Start a new slice index|Clear or sky until matching rendered content arrives|
|Out-of-plane origin move|Invalidate the active partition|Start a new slice index|Clear or sky until matching rendered content arrives|
|Cap, precision, record ABI, or strict reference generation|Invalidate in version one|Start a new MAIN index|Never mix old records into the requested model|

## Candidate selection and full five-dimensional reprojection

Selection starts with the target pose, not a preferred source pose. It derives the conservative target chart query, walks the chart index, deduplicates tile IDs, rejects incompatible or stale descriptors, and forward-projects each retained valid footprint through that tile's own source reconstruction and the requested target chain.

A candidate is admitted only where its transformed mesh is finite, intersects the requested screen, has valid value and source-depth samples, and carries a measured interpolation bound at most 1.0 px. Admission may be cell-local: a pole or missing sample clips the affected cells rather than converting a valid remainder into invented geometry.

Pose proximity is a cost heuristic, not a correctness rule. A distant-angle tile can be the best source for a newly visible region, while a nearby source can become edge-on or expose no useful footprint. The selector ranks projected hole coverage, certified sampling density, refinement rung, reprojection error, and age when reducing candidates to the placement capacity; the definitive per-pixel ranking remains true target depth between distinct points and quality only between representations of the same point.

For every admitted mesh vertex, the vertex stage identifies the retained source pixel, loads its winning value and lifted source-depth/reconstruction record, reconstructs that source-visible surface point under the source pose, maps it through canonical slice identity, applies the requested height, and evaluates the requested five-dimensional camera and depth equations. The target position and `Depth24Plus` value therefore come from the requested pose rather than from interpolating a screen homography or copying source depth.

Version one reuses the present scene's screen-grid relief topology and complete five-dimensional vertex algebra once per tile, changing their inputs from the live grid and one pose to the tile's retained value/depth samples, source pose, and requested pose. This is the direct mechanical bridge from today's proven scene projection to multi-view reprojection.

The baseline mesh draws the complete 254×254 retained core, with 253×253 cells and two triangles per cell. A coarser mesh class is allowed only when deterministic interior witnesses prove its target interpolation error remains at most 1.0 px; no mesh may subdivide past unavailable value/depth samples and pretend to discover surface detail.

The source-depth receipt is checked during reconstruction. Reprojecting a tile back to its own render pose must reproduce every valid source pixel center, winning surface identity, and source depth within the declared numeric bound; disagreement marks the tile corrupt or the ABI mismatched before it can contaminate another pose.

The current depth-free `Warp::reproject` incompatibility checks remain useful inputs, but its one global homography is not reused as the new warp. The replacement planner returns one placement receipt per tile, and the GPU applies one independently parameterized mesh transform per placement entry.

## One-pass depth composition and intersections

The target render pass begins with the transient clear, establishes only certified sky or exterior at far depth with a bounded background classification in that same pass, and submits every selected rendered tile mesh with depth writes enabled, `LessEqual`, no blending, and no cached color input. A fragment with no valid retained surface sample is discarded so source sky cannot occlude geometry contributed by another source view, while an unknown uncovered region remains clear rather than being mislabeled as exterior.

For different surface points that reach the same target pixel, the smallest true target depth wins regardless of source pose, refinement, age, or draw order. This includes projected folds of the lifted Julibrot surface and intersections between tiles rendered at different angles.

Quality is consulted only among representations of the same canonical surface point or chart microcell. The index walk assigns a deterministic same-surface owner using Detail before Backdrop, Final before Interactive before Preview, finer certified density, lower coordinate/depth/reprojection error, then newer age and stable tile ID; a compact per-tile chart-ownership mask suppresses inferior duplicate claims before depth testing.

This is the depth-first rule: same-surface ownership removes duplicate approximations, then physical depth layers the remaining distinct points. All selected tiles remain independently transformed meshes, and the ownership mask never suppresses an intersection between different canonical surface points merely because their target pixels overlap.

Equivalently, the composite order is target nearness first across different lifted surface points and the quality tuple `(residency, rung, density, error, age, tile_id)` only inside one canonical same-point equivalence class. There is no total tile-level quality order that can replace this per-fragment decision.

Exact coincident fragments use low-to-high quality draw order with `LessEqual` as a deterministic final tie, but draw order is not the proof for approximate duplicates. Adding quality offsets or polygon bias to target depth is forbidden because it can move a lower-quality near surface behind a higher-quality far surface.

Tiles from the same source view share apron samples at source edges and must reproduce identical transformed edge vertices. Tiles from different source views usually have different triangulations, so the same-surface ownership partition chooses one representation across an overlap band; boundary witnesses require no uncovered target pixel and at most 1.0 px disagreement with direct projection.

The per-pixel palette lookup happens after the fragment survives ownership and depth. The shader maps the winning escape record to linear color, applies exposure and tone mapping, and encodes the target; an optional arena HDR path writes one frame-scoped linear target and performs its resolve in the same reprojection layer.

The pass may load the transient clear color, but known exterior is then shaded as sky and every valid tile overwrites its covered samples. A whole-frame clear is reserved for a slice or MAIN change with no matching rendered content; within a matching partition, clear can survive only as an explicitly counted local hole outside delivered coverage, at a disocclusion no cached view contains, or at an oracle refusal. Ordinary camera motion inside the delivered backdrop must therefore land on coarse depth-bearing content rather than clear, and an all-edge-on request may be all sky.

## Per-tile refinement, backdrop, and scheduling

The Preview, Interactive, and Final ladder becomes a family of rendered tiles for a requested source-screen region and render pose. Refinement may replace value and depth together only after both spans and their reconstruction receipt complete; a value-only or depth-only completion is not publishable.

The first priority is a visible clear region, then a visible Backdrop region whose requested-pose render can improve, then a visible lower rung, then intersection and ownership repairs. Projected screen area, expected hole reduction, target-pose distance, and focus distance break scheduling ties without becoming validity rules.

Invisible Detail tiles are never newly rendered. The protected backdrop is the bounded exception: after visible holes have a coarse answer, the scheduler may render missing cells in the 3×3 coarse chart window so later camera motion has depth-bearing fallback views.

Nine active backdrop tiles stay pinned, while three rolling rendered tiles, each with paired value/depth spans, let one row or column update without discarding the prior coverage. Each backdrop is still stamped with the source pose that rendered it; rolling the chart window or changing its preferred source angle creates new rendered tiles and leaves old ones as ordinary history once pins transfer.

The cache should retain useful pose diversity rather than forty-four nearly identical angles. After visible quality and recency, eviction policy favors tiles that uniquely cover a chart region or source-view direction, and the page publishes the delivered distribution rather than hiding it behind one LRU count.

One reference orbit is shared and pinned by tile jobs in a MAIN generation. Every tile records its reference and precision receipt; a broad backdrop cell outside the orbit's certified region remains missing or triggers the explicit reference policy rather than receiving guessed records.

Palette, exposure, and tone changes enqueue no tile work. The next placement pass shades the same selected value records with new frame inputs, so display response is immediate even when all geometry comes from old render poses.

Publication and selection both preserve the draft rule. A completed tile can enter the cache without displacing a better same-pose family member, and it can contribute only chart cells for which it wins same-surface quality or supplies coverage absent from better tiles.

## Device-floor resource model

The current heap's unit is one nearest-sampled RGBA32F record of 16 bytes. A rendered tile therefore uses one 256×256 RGBA32F span for escape values and, on the conservative floor, a second equal span for source depth and validity: 2 MiB per physical tile and 32 bytes per sample.

The information lower bound is smaller only if every reconstruction auxiliary is derived exactly from pose: current 16-byte values plus one 4-byte depth is 20 bytes per sample, and a future 8-byte value dialect plus depth is 12 bytes. Version one may need source-local coordinates and validity in the depth record, and neither lower bound saves physical memory until a compact ABI is proved; padding either path into two RGBA32F spans still costs 32 bytes.

The existing example 64-page DATA heap cannot hold the earlier 56-tile promise once every tile owns two spans. Reserving eight pages for the orbit and other users leaves 56 pages, or 28 rendered tiles; protecting twelve backdrop tiles consumes 24 pages and leaves sixteen Detail/history tiles. This constrained profile must be published rather than silently dropping depth.

Keeping the former twelve-backdrop plus forty-four-Detail policy requires 56 rendered tiles, 112 DATA pages for their paired spans, and eight non-tile pages: 120 pages total. With four 256 pages per 512×512 layer that is 30 RGBA32F array layers and 120 MiB of physical DATA allocation, subject to the constructor's memory and layer walls even though it is below the 256-layer dimensional limit.

Each full-core mesh has 64,516 vertices, 64,009 cells, 128,018 triangles, and 384,054 `u32` indices. One immutable 1,536,216-byte index buffer is shared by every full-core tile; vertex positions are generated from vertex index and placement entry, so cached per-tile vertex buffers are unnecessary.

A naïve implementation issues one draw per transformed tile. The target uses one instanced indexed draw for every common mesh and pipeline class, with the tile descriptor selected by instance index; unusual clipped or coarsened meshes form a small bounded number of additional batches inside the same render pass.

The placement uniform remains a constructor contract. A 192-byte entry for source inverse map, source rect, value and depth handles, canonical transform, target receipt, quality, and ownership-mask references allows 64 entries to consume 12,288 bytes, leaving a 256-byte header within the 16 KiB floor; if the proven layout is larger, capacity decreases explicitly or immutable data moves to an indexed texture, never past the uniform wall.

The target uses one canvas-sized `Depth24Plus` attachment, cleared once and shared by every tile batch. Four bytes per pixel is a useful planning estimate—1.98 MiB at 960×540 and 7.91 MiB at 1920×1080—but actual allocation and padding are implementation facts, while the cached source-depth spans remain separate and persistent.

Two RGBA32F outputs fit under the four-attachment floor if value and source depth are produced together. The executor must nevertheless declare output, SCRATCH, directory, handle, and copy capacities for the paired publication; no design argument assumes an unused attachment or uniform slot.

The cache allocates no palette-color atlas. Direct shading adds no color target beyond ordinary presentation; if wider arena composition needs linear accumulation, exactly one frame-scoped RGBA16F transient costs 3.96 MiB at 960×540 or 15.82 MiB at 1920×1080, is cleared and logically released every frame, and is never keyed or evicted as cache.

## Cost model and eviction

All retained byte figures are binary MiB and exclude SCRATCH, the reference orbit, descriptors, the shared index buffer, target depth, ordinary presentation, and the optional frame transient unless stated otherwise.

|Resident set|Lower bound: 8-byte value plus 4-byte depth|Lower bound: 16-byte value plus 4-byte depth|Version-one floor: two RGBA32F spans|
|------------|------------------------------------------:|-------------------------------------------:|-----------------------------------:|
|One 256×256 rendered tile|0.75 MiB|1.25 MiB|2 MiB|
|Nine active backdrop tiles|6.75 MiB|11.25 MiB|18 MiB|
|Twelve protected and rolling backdrop tiles|9 MiB|15 MiB|24 MiB|
|Sixteen Detail/history tiles in the 64-page profile|12 MiB|20 MiB|32 MiB|
|Twenty-eight total tiles in the 64-page profile|21 MiB|35 MiB|56 MiB|
|Forty-four Detail/history tiles in the expanded profile|33 MiB|55 MiB|88 MiB|
|Fifty-six total tiles in the expanded profile|42 MiB|70 MiB|112 MiB|

The screen-aligned 254-sample core covers 253 source intervals per axis. A phase-friendly source view therefore needs 12 tiles at 960×540 and 40 at 1920×1080; arbitrary alignment can require 20 and 54 respectively.

With nine active backdrop tiles, the friendly and worst 960×540 demand is 21 and 29 drawn tiles, costing 42 and 58 MiB on the floor, invoking 1,354,836 or 1,870,964 mesh vertices, and submitting 2,688,378 or 3,712,522 triangles before clipping. The 64-page profile can retain twelve Detail plus all twelve protected backdrop tiles for 48 MiB and four additional Detail history tiles; at worst it retains sixteen Detail plus twelve backdrop tiles for 56 MiB, draws only 25, and leaves four fine regions at backdrop quality.

At 1920×1080 the expanded profile retains forty friendly Detail plus all twelve protected backdrop tiles for 104 MiB and leaves four Detail history slots. It draws 49 active tiles for 98 MiB of referenced payload, 3,161,284 vertices, and 6,272,882 triangles; at worst it retains forty-four Detail plus twelve backdrop tiles for the full 112 MiB, draws 53 for 106 MiB of referenced payload, 3,419,348 vertices, and 6,784,954 triangles, while ten unavailable fine regions deliberately fall back to depth-bearing backdrop.

These triangle counts are topology submissions, not raster cost. Projection can shrink, magnify, overlap, or clip every source tile, so the page must publish surviving fragments, depth-passed fragments, overdraw, rejected cells, and batches rather than deriving GPU time from source sample count.

A whole screen-aligned value-plus-depth view before tiling reserves about 16 MiB at 960×540 and 64 MiB at 1920×1080 when both columns use the current RGBA32F page classes; retaining `N` whole views in migration stage 2 multiplies that cost. Splitting into tiles adds page-edge rounding but permits regional eviction and mixed-pose coverage.

Today's retained design costs about 11.96 MiB at 960×540 and 47.82 MiB at 1920×1080 for one Final record reservation plus two LDR scene textures. Friendly rendered-tile draws reference 42 and 98 MiB, their resident sets including three rolling backdrop tiles cost 48 and 104 MiB, and the expanded full cache costs 112 MiB before shared resources. The design buys multi-angle depth history and return continuity with real memory rather than presenting tiling as a free optimization.

The active backdrop's nine tiles contain 589,824 physical samples in each column. At cap 32 its value computation ceiling is 18,874,368 iterations and at cap 64 it is 37,748,736; source-depth generation and mesh presentation are additional measured work.

Eviction first removes invalid partitions, corrupt pairs, and failed drafts. It never frees either span of a selected, in-flight, publication-source, ownership-source, or pinned backdrop tile until all fences and frame references release both.

Among unpinned valid tiles, eviction uses last-selected age, distance from the requested chart footprint, projected usefulness, source-pose distance, unique chart coverage, and pose-direction diversity in that order after protecting better same-surface quality. Stable tile ID resolves exact ties.

The principal wins are reuse after returning to a stored render pose, useful coverage from a different angle, camera continuity through old depth, regional replacement, and a coarse backdrop where no fine tile survives. The losses are paired value/depth storage, millions of mesh vertices, target overdraw, and inevitable holes where retained views never contained the newly visible arena surface.

## Oracle, browser proofs, and published facts

Source reconstruction tests take retained source sample centers and depths through source unprojection and require agreement with independent canonical-chart reconstruction, then project them back through the source pose and require the original screen center and source depth within the declared numeric bound.

Per-tile reprojection tests transform every edge, a deterministic interior lattice, depth extrema, all five existing height witnesses, and adversarial pole cases from many source poses to many requested poses. Each admitted mesh reports finite maximum and p95 disagreement with direct full-chain projection, with the maximum at most 1.0 px.

Composition tests place two distinct chart points at one target pixel and require the nearer target depth to win for every draw order, source age, and quality ordering. They include crossing relief folds, nearly coincident but distinct depths, a far Final against a near Preview, and tiles rendered from opposing source angles.

Same-surface tests render one chart region from several poses and rungs, require the ownership mask to choose the best representation independent of catalog and draw order, and then require exact coincident remnants to follow low-to-high quality `LessEqual` ordering. A quality change may never reverse two different surface points.

Intersection and seam tests cover same-view tile edges, arbitrary different-view footprint intersections, mixed densities, partial validity masks, horizons, subpixel target motion, and chart-ownership boundaries. They require no double same-surface claim, no false hole, correct near/far ownership, and at most 1.0 px disagreement with direct projection.

Shading tests begin with the winning value record after depth and ownership, evaluate a CPU reference palette-to-linear mapping plus exposure, tone mapping, and output encoding, and compare the final GPU pixel within format tolerance. Display-only changes must retain tile IDs, source depth, and escape-dispatch counts and appear in the next frame without recolor work.

The arena-boundary oracle includes a synthetic three-dimensional slice scene in which one source view hides a surface that a target view reveals. The expected result is a reprojection hole until another rendered view or new render supplies it; any test that fills it from the foreground depth fails because the lab must not teach a false disocclusion guarantee.

The first browser proof renders pose A, a materially different angle B, then returns to A under one content key. It shows both pose families resident, zero new escape work for A, per-tile source identities, correct target depth, and no stale LDR texture use.

The second proof retains several whole rendered views before chart tiling, reprojects all of them into an intermediate pose, and shows a one-pass depth image with constructed intersections whose result is independent of submission order. This pins migration stage 2 as the mechanism rather than merely a larger scene ledger.

The third proof enables screen tiles and the direct chart walk, pans and zooms by partial tiles, and reports visited nodes, returned and deduplicated IDs, transformed footprints, admitted meshes, holes, and work independent of unrelated cache population at 960×540 and 1920×1080.

The fourth proof establishes the nine-tile backdrop, then exercises every camera and observer slider, height zero and nonzero, both perspective distances, rotation, pitch, zoom-out, resize, palette, exposure, and tone without changing MAIN. It shows depth-bearing coarse fallback inside delivered coverage and honest clear, sky, or refusal outside it.

The fifth proof performs plane-preserving object and origin changes, then slice tilt, out-of-plane origin, cap, precision, and reference changes. It keeps every render-pose family for certified same-slice moves and immediately partitions every genuine content change.

The sixth proof reverses Preview, Interactive, Final, backdrop, and fence completion order across overlapping source poses. It proves visible-first scheduling, paired value/depth publication, no draft downgrade, same-surface quality ownership, and physical-depth priority across distinct surfaces.

The page must publish content and render-key digests; source-pose family; source rectangle and extent; value and depth formats, spans, bytes, generations, ranges, and validity; derived chart footprint, density, index nodes, and uncertainty; requested and delivered backdrop extent; total, backdrop, Detail, pinned, candidate, deduplicated, selected, clipped, corrupt, and evicted tile counts; and pose-diversity facts.

For each frame it must publish the requested pose digest, chart nodes visited, candidate IDs returned, forward bounds tested, placement receipts accepted and refused by reason, mesh classes, vertices, triangles, batches, draw calls, target-depth mode, same-surface mask cells, shaded fragments, depth-passed fragments, overdraw, clear and sky fractions, optional HDR transient bytes, normalized compute and copy work, and CPU and fence walls.

Correctness facts include maximum and p95 source reconstruction error, target reprojection error, depth disagreement, seam disagreement, horizon and pole counts, glitch and coordinate-uncertain counts, same-surface conflicts, quality-tie decisions, distinct-surface intersection decisions, stale-generation refusals, draft-downgrade refusals, and whether every displayed tile region has a current oracle receipt.

## Staged migration

Every stage keeps the page usable behind a capability switch, retains the last proved path until the new one reaches parity, and adds browser-visible facts before deleting fallback code. Lane-hours are focused implementation, tests, browser proofs, documentation, and review work for one engineer familiar with the lab.

|Stage|Working implementation at the end|Pinning tests and proofs|Estimate|
|-----|---------------------------------|------------------------|-------:|
|0. Rendered-view policy and math|Add render keys, source-depth ABI, source reconstruction, target full-chain projection, slice validity, derived chart footprints, quality ownership, and cost types without changing the current page|Pose serialization, source round-trip, direct target projection, depth definitions, same-slice equivalence, invalidation matrix, resource arithmetic|20–28 lane-hours|
|1. One depth-bearing retained view|Replace the retained LDR source dependency with one whole-screen value-plus-depth view; reproject its mesh per vertex through the full target chain and shade values at placement, retaining the old homography path as a switchable fallback|Self-reprojection, material camera deltas, height changes, poles, 1.0 px admission, palette and HDR immediacy, current preset parity|36–52 lane-hours|
|2. N retained rendered views reprojected and depth-composited|Generalize the ledger to bounded whole rendered views from different source poses and submit all selected meshes in one target depth pass before introducing chart tiling|Opposing angles, crossing relief, near Preview versus far Final, same-surface quality ties, order independence, holes, fence-safe paired spans, N=1 parity|40–60 lane-hours|
|3. Screen-aligned rendered tiles and chart index|Split each rendered view into fixed source-screen tiles, derive chart footprints and density nodes, direct-walk and deduplicate candidates, batch instanced meshes, and permit regional publication and eviction|Same-view aprons, arbitrary cross-view overlaps, exhaustive index versus slow scan, unrelated-cache scaling, uniform and draw walls, both target extents|56–84 lane-hours|
|4. Backdrop and per-tile refinement|Add nine active plus three rolling coarse rendered backdrop tiles, visible-first tile families, paired value/depth refinement, reference sharing, pose-diverse retention, and protected eviction|3× extent, 127–253 interval density, rotate/pitch/zoom-out fallback, rolling fences, no invisible Detail work, no backdrop or draft displacement|40–60 lane-hours|
|5. Hardening and arena-facing proof|Make the rendered-tile path default after parity, publish complete resource and intersection facts, add long navigation and device-loss soak, and pin the explicit arena disocclusion counterexample|All sliders and presets, 64- and 120-page profiles, capacity refusals, source corruption, depth precision, overdraw walls, return proofs, arena hole honesty, console cleanliness|32–48 lane-hours|

The total estimate is 224–332 lane-hours. Stage 2 intentionally precedes chart tiling: it proves that several independently rendered, depth-bearing views can be transformed and composited as the arena mechanism before storage subdivision and cache indexing complicate the evidence.

## Backlog

- `JB-TILE-001` — Specify canonical slice identity, full render-pose serialization, source pixel rectangles, source-depth semantics, and exact cache-key equality with display controls excluded.
- `JB-TILE-002` — Implement pure source-sample reconstruction and full target five-dimensional projection, including source self-round-trip and 1.0 px target receipts.
- `JB-TILE-003` — Define paired value/depth publication, corruption and fence behavior, and the conservative two-RGBA32F floor layout plus compact-format study.
- `JB-TILE-004` — Generalize the retained ledger to N whole rendered views from different poses and depth-compose their independently transformed meshes in one pass.
- `JB-TILE-005` — Implement same-surface identity and ownership masks, low-to-high exact-tie ordering, and adversarial proof that quality cannot reverse distinct physical depths.
- `JB-TILE-006` — Move palette-to-linear lookup, exposure, tone mapping, and output encoding into per-fragment reprojection shading with no retained color payload.
- `JB-TILE-007` — Split rendered views into 256×256 source-screen tiles with 254×254 cores, aprons, shared index topology, and paired span ownership.
- `JB-TILE-008` — Add conservative derived chart footprints, density levels, multi-node tile membership, direct candidate traversal, ID deduplication, and a slow exhaustive oracle.
- `JB-TILE-009` — Add per-tile placement receipts, cell-local poles and holes, full-core and proved-coarse mesh classes, instanced batching, and the 64-entry capacity wall.
- `JB-TILE-010` — Add and prove the nine-active plus three-rolling depth-bearing rendered backdrop with published coverage, source poses, certification holes, and work walls.
- `JB-TILE-011` — Replace the global ladder with visible rendered-tile families, paired value/depth publication, no-draft-downgrade rules, and pose-diverse eviction.
- `JB-TILE-012` — Publish source-pose, source-depth, chart-index, candidate, mesh, intersection, overdraw, shading, resource, eviction, and timing facts and add six browser proofs.
- `JB-TILE-013` — Study promotion of certified Final value records across reference generations without weakening source-depth or MAIN provenance.
- `JB-TILE-014` — Build the arena disocclusion counterexample and document which rendered-view mechanisms transfer from the 2D lab and which require new 3D slice content.

## Risks

The largest conceptual risk is accidentally returning to chart-image thinking. A chart footprint is only an index receipt; shader inputs, depth, reconstruction, coverage, and reuse must remain attached to the source screen tile and its render pose.

Source depth needs a precise reusable definition. Caching nonlinear `Depth24Plus`, omitting clip sign, or reconstructing through a different camera convention can move points or reverse intersections; the source self-round-trip and independent full-chain oracle are release gates.

The existing 64-page heap is too small for the earlier 56-tile promise once depth is honest. Its 28-tile constrained profile leaves only sixteen Detail tiles after backdrop protection, while the 56-tile profile needs 120 DATA pages including reserves. Memory refusal and delivered cache size must be visible interface facts.

Full-core meshes submit millions of vertices and triangles, and different source angles can generate heavy overdraw. Instancing removes draw-call overhead but not vertex work, record loads, clipping, or fragments; coarser meshes require proof and candidate caps may trade useful coverage for rate.

Same-surface duplicates are numerically dangerous. If approximate representations reach the depth test without ownership, they can z-fight; if quality is encoded as depth, it can reverse a real near/far relation. The canonical ownership mask is correctness machinery, not an optional seam polish.

Derived chart bounds can be loose under relief or fail near a pole. Loose bounds enlarge candidate buckets and tight but unsound bounds lose tiles; every fallback roster and forward-test count needs a wall and a slow-scan oracle.

Reference churn still threatens the backdrop and return cache. Strict generation identity can retire otherwise useful value/depth pairs, while a single centered orbit may not certify broad coarse views; reference-independent promotion remains a separate proof obligation.

The backdrop is finite and pose-stamped. It improves ordinary rotation, pitch, pan, and zoom-out only inside its delivered transformed coverage, and no finite set promises an arbitrary teleport, a target pole, or unseen arena geometry.

The 2D lab can overstate arena success because its chart names the entire lifted surface. The explicit occlusion counterexample must remain failing-until-rendered evidence so the project never converts “several depth views often fill the hole” into “depth views contain the scene.”

The lab's chart supplies same-surface identity for quality ownership, but an arbitrary arena view may not. Porting the quality tie-break without persistent surface or primitive identity can merge nearby distinct layers or leave duplicates fighting; this interface is an arena prerequisite, not evidence furnished by Julibrot depth alone.

Per-pixel value lookup, depth testing, palette evaluation, and HDR output add presentation cost even on an escape-compute cache hit. Browser facts must separate selection, vertex reconstruction, raster overdraw, shading, and fence latency instead of calling all saved iteration time a frame-time win.

## Open questions for the owner

1. What exact source-depth record should version one retain? Recommendation: begin with the specified second RGBA32F record containing positive linear post-observer view distance, two normalized source-local plane coordinates, and validity; prove source round-trip and target reconstruction across both perspective divides, never cache quantized `Depth24Plus`, and compact only after the oracle identifies which auxiliaries can be derived exactly.

2. Is 256×256 physical with a 254×254 source-screen core and one-sample apron still the first tile geometry? Recommendation: yes for the first proof because it reuses the heap page and one shared mesh, but treat 128 and 512 as measured source-screen alternatives rather than chart levels.

3. How should same-surface quality ties be implemented? Recommendation: use canonical chart-cell ownership masks before depth, then low-to-high quality submission with `LessEqual` only for exact coincident remnants; never bias true depth with quality.

4. Which cache profile is the product floor? Recommendation: expose the existing 64-page, 28-rendered-tile constrained profile first, and make the 120-page, 56-rendered-tile profile an explicitly delivered higher memory tier until weak-device measurements justify changing the floor.

5. How many transformed candidates may one frame submit? Recommendation: retain a constructor-validated 64-entry ceiling, rank by newly covered screen area and certified error after semantic admission, and publish dropped candidates and remaining holes rather than overflowing uniforms.

6. Does reprojection need a frame-scoped HDR color transient? Recommendation: shade directly into presentation for the Julibrot proof; if arena composition requires linear accumulation, permit exactly one RGBA16F canvas target with published bytes and frame lifetime, never retained cache identity.

7. May certified value records survive a reference regeneration? Recommendation: no in version one; keep strict MAIN equality until an independent-value proof also specifies how the source-depth receipt is regenerated or retained.

8. May a tile rendered at one Julibrot height be reused at another? Recommendation: yes because the value record reconstructs base height, but store source height in the render key, validate stored source depth at that height, and recompute target position and depth at requested height.

9. Should version one use every retained sample as a mesh vertex? Recommendation: yes; the full 254×254 core is the honest baseline. Add coarser mesh classes only when interior witnesses prove the target error ceiling and measurements show the vertex reduction matters.

10. How should eviction value different render angles? Recommendation: protect visible quality and backdrop first, then retain tiles with unique chart coverage or view direction before redundant nearby poses; publish pose buckets so “multi-view” is measurable.

11. What arena claim may the lab make, and what identity must the arena provide? Recommendation: claim proof of pose-stamped value/depth retention, full-chain per-view reprojection, depth intersections, chart-backed same-surface quality ownership, and honest gaps; require the arena to supply stable surface or primitive identity before transferring that quality rule, and explicitly do not claim hidden-surface recovery or complete 3D slice reprojection.

12. Should rendered tiles persist across reloads? Recommendation: remain session-local until pose, bignum, value, depth, ABI, corruption, quota, and privacy contracts are versioned together.

## Repository sources

- [Repository charter](../../CLAUDE.md) supplies layering, renderer, reporting, Markdown, and device-floor ownership constraints.
- [Minimum requirements](../minimum-requirements.md) supplies the WebGL2, `EXT_color_buffer_float`, attachment, texture, uniform, and unavailable-feature floor.
- [GPU resource heap](../gpu-resource-heap.md) supplies physical descriptors, generation-checked handles, RGBA32F DATA, RGBA8 IMAGE, buddy allocation, and lifetime contracts.
- [GPU heap lattice](../gpu-heap-lattice.md) supplies `DataSpan`, paired structure-of-arrays precedent, span directories, executor and SCRATCH contracts, fixed capacities, and draw-cost evidence.
- [Julibrot lab charter](../julibrot-lab.md) supplies component layering and the two-dimensional-slice/five-dimensional-view purpose.
- [Julibrot math](math.md) supplies the full pose, screen-aligned inverse map, direct five-dimensional projection, deep precision, compatibility, and 1.0 px warp contract.
- [Julibrot kernels](kernels.md) supplies escape-record semantics, screen-grid and page arithmetic, refinement work, copies, and normalized facts.
- [Julibrot worker](worker.md) supplies bignum reference-orbit generation, precision, transfer ABI, and credit ownership.
- [Julibrot presentation](present.md) supplies the existing scene mesh's full five-dimensional vertex chain, true scene depth, retained LDR ledger, depth-free homography warp, horizon behavior, palette, exposure, and measurement receipts.
- [Julibrot app](app.md) supplies HOT/MAIN publication, scene modes, latest-wins scheduling, page facts, presets, sliders, and browser proof conventions.
- [Julibrot precision ledger](precision-ledger.md) supplies coordinate, projection, memory, copy, and timing budgets.
- `origin/docs/4d-arena:docs/4d-first-engine.md` supplies the view-frame split, rendered-view authority direction, true depth-aware within-slice reprojection, genuine 3D disocclusion, and the stabilizer-of-the-slice limit.
- `origin/docs/4d-arena:docs/4d-torus-world.md` supplies the compact-world phase and bounded-region distinctions.
- `origin/docs/4d-arena:docs/4d-content.md` supplies higher-rank slice identity, genuine occlusion/content cautions, and the boundary between retained presentation and new slice content.
