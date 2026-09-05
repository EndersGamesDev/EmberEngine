# Rendered-tile reprojection for the Julibrot lab

Status: draft design study.

## Decision summary

The cache is a set of rendered tiles captured from many five-dimensional render poses of the same scene, represented to the GPU by an RGBA32F descriptor map that joins each tile's pose header to its value and lifted-position samples. A requested frame lays every candidate tile out from its own pose through the full five-dimensional camera chain and depth-composites the transformed meshes, so tiles rendered at different angles may overlap and intersect and are layered by nearness.

Rendering itself is tile-based and demand-ordered: a missing part of the requested wide scene is filled at the coarsest sufficient level before any already-covered region receives more detail. The cached unit is a screen-aligned fragment of one completed render, not a rectangle of a canonical chart image. Each tile is stamped with the source object `O` and origin, camera `Q` and translation, height, `d5`, `d4`, yaw, pitch, zoom, canvas extent, source pixel rectangle, slice identity, and MAIN generation, and it retains both palette-independent value records and lifted source depth for its samples.

Reprojection is per tile and per mesh vertex. A source pixel plus its depth and source pose reconstructs the lifted point represented by that sample; the requested pose then runs that point through its own full five-dimensional projection and produces a new screen position and true target depth. There is no single global homography warp in this design, including when many tiles happen to share a source pose.

Composition is one depth-tested render pass over instanced tile meshes whose vertex stage reads source pose and lifted samples from the descriptor map. True target depth chooses between distinct surface points, while refinement level, sampling density, error bound, and age choose only between competing representations of the same surface point; quality is never added to physical depth.

Resolution follows geometry rather than a CPU quality ranking: a tile captured nearer to its scene part carries denser surface samples, that density remains attached to its reconstructed points on the target screen, and after every candidate is laid out in the requested five-dimensional pose the nearest-by-depth winner is the best-resolved visible part by construction. The CPU does not solve a set cover to choose resolution.

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

The durable GPU payload is the descriptor map: one pose header per rendered tile and two equal-length post-visibility RGBA32F sample columns, the winning palette-independent escape-value record and the lifted source-depth/reconstruction record with validity. No screen color, palette result, exposure result, tone-mapped pixel, hidden source fragment, or target depth attachment is retained as tile cache.

Deep-zoom surface identity remains exact because the CPU semantic descriptor carries an interned bignum-anchor ID and scale provenance and the reconstruction record identifies the source-local plane point represented by a retained pixel. RGBA32F never stores the absolute bignum coordinate: before composition the CPU resolves the source and requested anchors exactly and materializes a compensated high/low local delta in the tile header; a finite mirror drives projection only after that mapping is certified and is never an equality key.

The chart pyramid has an immutable bignum anchor and signed dyadic node coordinates. Publication conservatively derives each rendered tile's canonical chart footprint, density interval, horizon cuts, and uncertainty from its source map and valid sample domain, then inserts the tile ID into every bounded index node required by that footprint. A tile may occupy several nodes, and a node may reference tiles from many render poses.

Pyramid level describes derived chart sampling density, not stored tile dimensions. Two rendered tiles at the same derived level can have different quadrilateral footprints and source poses, and a rendered tile never becomes a dyadic chart rectangle merely because the index bins it there.

For a requested pose, the selector inverse-maps a conservative target screen bound into the canonical chart, walks only intersecting pyramid nodes from coarse fallback toward requested density, gathers their tile IDs, deduplicates them, and then forward-projects each candidate's actual retained mesh bounds. This retains the direct-walk objective: work is proportional to visited query nodes and returned candidates, not total cache population.

Relief, a perspective pole, or an edge-on target can defeat a finite inverse chart bound. In that case the selector may forward-test the bounded active backdrop roster and a bounded recent-pose roster, but it must publish that fallback and refuse any unbounded tile; it may not disguise a full-cache scan as constant-time indexing.

The backdrop window is a 3×3 set of coarse chart-index cells centered on the requested footprint, plus three rolling replacement slots. Each cell is backed by a screen-aligned rendered tile from a deliberately broad source pose whose derived valid footprint covers that cell with guard samples; if the requested cell is not completely covered, only the proven footprint is published.

Choose the backdrop density so one active cell spans between one and two times the longest conservative chart dimension of the current view. Nine cells then span at least three times both current chart-axis extents, with roughly 127–253 retained core intervals across the longest current-view dimension. The source pose and depth remain attached, so rotation, pitch, translation, and zoom-out transform the backdrop as old rendered geometry rather than sampling a fixed chart picture.

Every backdrop tile must pass the normal precision, glitch, source-depth, and placement proofs. If one active reference orbit cannot certify the intended wide region, the delivered backdrop is smaller and facts identify its holes; the design does not trade correctness for a no-clear slogan.

|Property|Current retained scene|Proposed rendered tile|
|--------|----------------------|----------------------|
|Retained unit|One whole LDR screen image|One source-screen fragment with descriptor header and `S0/S1` samples|
|Pose provenance|One retained pose|Full RGBA32F pose header per tile, with many poses resident together|
|Reprojection|One depth-free screen homography|One full-chain depth-bearing mesh transform per tile|
|Overlap|Only the last image|Arbitrary transformed overlap among source poses|
|Visibility|Already baked into source color|Recomputed per pixel with true target depth for retained samples|
|Chart role|Source screen maps indirectly to the plane|Derived coverage and density index only|
|Palette and HDR|Baked into retained LDR color|Applied per pixel after target visibility|

## Rendered-tile descriptor and heap relationship

Each CPU semantic descriptor has a stable `tile_id` and a `content_key`. The content key contains the canonical slice identity, formula and record ABI, MAIN generation, precision mode, requested-cap semantics, reference-orbit generation under the conservative first policy, and any record interpretation version; it contains no render pose or display control. This CPU form remains the f64 and bignum identity authority, while the GPU descriptor map is its bounded RGBA32F projection for composition.

The separate `render_key` contains the complete source pose: object angles `O`, plane origin, certified plane basis and canonical transform, exact bignum-center serialization and revision, scale, camera `Q`, five-dimensional translation, height scale, `d5`, `d4`, yaw, pitch, zoom, source canvas extent, and accepted source screen map. The cache lookup key is `(content_key, render_key, source_rect, delivered_rung)`; publication epoch and age are lifetime facts, not pose identity.

The GPU descriptor map is one logical record set over the heap lattice's stable RGBA32F DATA view: a shared header `DataSpan`, one existing value `DataSpan` per tile, and one lifted-sample `DataSpan` per tile. The header references both sample spans through their generation-checked directory indices, so after the CPU materializes the requested anchor residual the composition pass needs only the descriptor map, one current-target uniform, and current palette or HDR inputs; it does not rebuild source pose from CPU uniforms.

One header slot is exactly 32 RGBA32F texels, 512 bytes, aligned to the existing 16-byte heap record and to a power-of-two texel stride. A 256×256 descriptor page reserves texels 0–63 for the frame's maximum 64 active-instance records `[header_slot,mesh_class,ownership_revision,flags]`, texels 64–2,111 for 64 complete header slots at `64+32*header_slot`, and texels 2,112–65,535 as a 63,424-record compact chart-ownership arena addressed by each header. The three regions are 1,024, 32,768, and 1,014,784 bytes and fill the 1 MiB page exactly.

|Header texels|RGBA32F lanes|Meaning|
|-------------|-------------|-------|
|`H00`|`tile_id, content_key_id, anchor_id, flags`|Stable tile, semantic partition, exact-anchor interner reference, and validity bits|
|`H01`|`value_span, lifted_span, ownership_base, header_generation`|Generation-checked sample-span directory indices, ownership-arena base, and header generation|
|`H02–H04`|Six ordered `(cos O_ij,sin O_ij)` factor pairs, two per texel|Six `SO(4)` object factors in `12,13,14,23,24,34` order|
|`H05–H09`|Ten ordered `(cos Q_ij,sin Q_ij)` factor pairs, two per texel|Ten `SO(5)` camera factors in `12,13,14,23,24,34,15,25,35,45` order|
|`H10`|`cos yaw,sin yaw,cos pitch,sin pitch`|Source observer rotation|
|`H11–H12`|`origin0_hi..origin3_hi, origin0_lo..origin3_lo`|Compensated four-dimensional source plane origin|
|`H13`|`t0,t1,t2,t3`|First four source camera-translation coordinates|
|`H14`|`t4,height,d5,d4`|Fifth translation, source height, and both source perspective distances|
|`H15`|`zoom_log2,extent_w,extent_h,chart_density`|Source zoom, canvas extent, and delivered density|
|`H16`|`rect_x,rect_y,rect_w,rect_h`|Integer source-screen rectangle|
|`H17`|`anchor_dx_hi,anchor_dx_lo,anchor_dy_hi,anchor_dy_lo`|Requested-pose-relative deep-anchor delta materialized before the pass|
|`H18–H20`|Three padded rows of the accepted source screen map|Source pixel-to-plane reconstruction and round-trip oracle|
|`H21`|`depth_min,depth_max,coordinate_error,reprojection_error`|Finite source-depth range and admission bounds|
|`H22`|`residency_rank,refinement_rung,iteration_cap,age_rank`|Same-surface ownership and scheduling facts|
|`H23`|`valid_count,glitch_count,uncertain_count,mesh_class`|Sample status and mesh selection|
|`H24`|`chart_scale_hi,chart_scale_lo,anchor_precision_bits,anchor_revision`|Compensated local scale and exact-anchor provenance|
|`H25`|`slice_key_id,MAIN_generation,record_ABI,reference_generation`|Semantic and record provenance checked before admission|
|`H26`|`ownership_count,ownership_revision,value_generation,lifted_generation`|Bounded same-surface mask records and sample-span generations|
|`H27–H31`|Zero|Reserved; nonzero use requires a descriptor-map ABI revision|

Every ID, count, extent, rank, and flag lane is an integer-valued finite `f32` strictly below `2^24`; arbitrary integer bit patterns and NaN payloads are forbidden. The rotation lanes carry the same precomputed sine/cosine factor representation already used by the present HOT payload, and the high/low lanes carry a checked compensated split rather than claiming one `f32` is the source authority.

Each compact ownership-arena texel is `[first_local_cell,run_length,owner_header_slot,quality_rank]`, an exact-in-f32 run over same-surface chart cells sorted by local cell. `H01.ownership_base` and `H26.ownership_count` bound the runs for one tile; zero runs means no duplicate exclusion, and exhaustion refuses additional duplicates rather than suppressing a distinct intersection. These records implement only the same-surface tie-break and are not a CPU-selected screen cover.

Each physical 256×256 tile has 65,536 sample indices and exactly two RGBA32F texels per sample, expressed as paired structure-of-arrays spans so the existing two-output executor can publish without an interleave copy.

|Sample texel|RGBA32F lanes|Bytes|Meaning|
|------------|-------------|----:|-------|
|`S0[k]`|Existing escape-record lanes unchanged|16|Palette-independent value, escape classification, rebase, and status under the existing record ABI|
|`S1[k]`|`a_F,b_F,zeta_F,validity`|16|Source-local plane coordinates, positive linear lifted source depth, and exact zero-or-one validity|

The descriptor map therefore costs exactly 32 bytes per sample, 2,097,152 sample bytes per physical tile, plus one 512-byte header slot: 2,097,664 logical bytes per resident tile. `S0` preserves the existing 16-byte record byte-for-byte and `S1` replaces the former ad-hoc depth record with the versioned lifted-position half of the descriptor-map ABI.

Source geometry facts contain the 256×256 physical extent, 254×254 core, apron, post-visibility validity channel, source depth and reconstruction definition, minimum and maximum finite source depth, horizon and pole classifications, and enough mesh topology information to reproduce the rendered fragment without consulting a discarded scene texture or pre-raster grid.

Payload facts contain the generation-checked header slot, value `DataSpan`, lifted-sample `DataSpan`, logical and reserved bytes, initialized prefixes, publication fences, and record count. Header, `S0`, and `S1` publish transactionally; a later compact representation is a distinct descriptor-map ABI and capability, not assumed padding savings.

Derived index facts contain the canonical chart footprint and conservative bound, chart-density interval, pyramid nodes containing the tile ID, bignum anchor digest, source-to-canonical error, and index revision. These facts may be rebuilt from the render pose and sample domain; they accelerate selection but never redefine the stored samples.

Quality facts contain Detail or Backdrop residency, Preview, Interactive, or Final rung, delivered cap, sample density, coordinate and source-depth errors, glitch and uncertain counts, mesh interpolation bound, and a monotonic quality tuple. These facts schedule upgrades and break ties only for the same canonical surface point; they never choose resolution between intersecting surface points. Age is a deterministic serial and breaks same-surface ties only after refinement, density, completeness, and error.

Lifetime facts contain creation, last-selected and last-visible serials, hit count, target-pose reuse count, source-pose family, pin reasons, backdrop-window generation, publication state, and eviction class. Pose diversity is observable rather than inferred from tile coordinates.

A target-placement receipt is keyed by tile ID and requested pose digest and contains transformed screen footprint, target depth interval, mesh class, maximum and p95 screen error, rejected cells, horizon clips, target pole status, same-surface ownership mask revision, and refusal reason. It is replaceable derived state and carries no palette or cached color state.

[GPU resource heap](../gpu-resource-heap.md) already supplies stable RGBA32F DATA bindings, generation-checked handles, typed allocation walls, and square pages. [GPU heap lattice](../gpu-heap-lattice.md) supplies multi-page `DataSpan` ownership, span directories, transactional allocation, fixed executor input and output arity, fixed uniforms, SCRATCH-to-DATA publication, and generation-safe free.

The heap does not supply render-pose identity, source-depth semantics, chart-footprint indexing, candidate deduplication, per-tile mesh reprojection, same-surface ownership, target-depth composition, pose-diverse eviction, backdrop policy, or reprojection oracles. Those remain Julibrot math, present, and app policy above the generic heap.

## Validity, compatibility, and invalidation

Implementation decision, 2026-09-05 on `lane/jb-retain`: stage 0 retains one completed pre-transition image as a partition-stamped held source while the first scene of a new partition is in flight. That source admits only an unchanged hold, never cross-partition reprojection, and is replaced by the first accepted completion in the new partition. This transitional two-image ledger preserves honest visible continuity while the descriptor-map tile cache remains unshipped.

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

Pose proximity is a bounded-enumeration heuristic, not a correctness or resolution rule. A distant-angle tile can contribute the nearest visible surface while a nearby source can become edge-on or expose no useful footprint, so the selector emits every valid intersecting tile admitted by the direct index walk and fixed placement wall; overflow follows the coverage-first scheduler and is published as omitted candidates or holes, not optimized by a CPU set-cover search. Same-surface ownership may discard duplicate representations, but only the GPU target-depth result chooses among different surface points.

For every admitted mesh vertex, instance index selects the tile header in the descriptor map, vertex index selects the retained source sample, and two nearest RGBA32F loads return its value and lifted source-depth/reconstruction records. The vertex stage reconstructs that source-visible surface point under the header's `O`, origin, `Q`, translation, observer, height, and perspective facts, maps it through canonical slice identity, applies the requested height, and evaluates the requested five-dimensional camera and depth equations. The target position and `Depth24Plus` value therefore come from the requested pose rather than from interpolating a screen homography or copying source depth.

Version one reuses the present scene's screen-grid relief topology and complete five-dimensional vertex algebra once per tile, changing their inputs from the live grid and one pose to the tile's retained `S0/S1` descriptor samples, source header, and requested pose. This is the direct mechanical bridge from today's proven scene projection to multi-view reprojection.

The baseline mesh draws the complete 254×254 retained core, with 253×253 cells and two triangles per cell. A coarser mesh class is allowed only when deterministic interior witnesses prove its target interpolation error remains at most 1.0 px; no mesh may subdivide past unavailable descriptor samples and pretend to discover surface detail.

The source-depth receipt is checked during reconstruction. Reprojecting a tile back to its own render pose must reproduce every valid source pixel center, winning surface identity, and source depth within the declared numeric bound; disagreement marks the tile corrupt or the ABI mismatched before it can contaminate another pose.

The current depth-free `Warp::reproject` incompatibility checks remain useful inputs, but its one global homography is not reused as the new warp. The replacement planner returns one placement receipt per tile, and the GPU applies one independently parameterized mesh transform per placement entry.

## One-pass depth composition and intersections

The target render pass begins with the transient clear, establishes only certified sky or exterior at far depth with a bounded background classification in that same pass, and submits every selected rendered tile mesh with depth writes enabled, `LessEqual`, no blending, and no cached color input. Its required machinery is one `Depth24Plus` attachment, instanced per-tile meshes, an active-instance list into the descriptor map, and integer-addressed `textureLoad` lowered to nearest RGBA32F vertex-stage fetches on WebGL2 plus `EXT_color_buffer_float`. A fragment with no valid retained surface sample is discarded so source sky cannot occlude geometry contributed by another source view, while an unknown uncovered region remains clear rather than being mislabeled as exterior.

Tiles from different render poses are independent meshes and may intersect one another in the lifted surface. When different surface points reach the same target pixel, the smallest true target depth wins regardless of source pose, refinement, age, or draw order; this includes projected folds of the lifted Julibrot surface and every cross-pose tile intersection.

No CPU set cover, global quality sort, or resolution-selection pass precedes that result. A nearer source capture already samples its represented scene part more densely, its descriptor reconstructs that density geometrically, and target depth exposes the best-resolved nearest part automatically after layout; candidate traversal supplies bounded inputs but does not decide the visible resolution.

Quality is consulted only among representations of the same canonical surface point or chart microcell. The index walk assigns a deterministic same-surface owner using Detail before Backdrop, Final before Interactive before Preview, finer certified density, lower coordinate/depth/reprojection error, then newer age and stable tile ID; a compact per-tile chart-ownership mask suppresses inferior duplicate claims before depth testing.

This is the depth-first rule: same-surface ownership removes duplicate approximations, then physical depth layers the remaining distinct points. All selected tiles remain independently transformed meshes, and the ownership mask never suppresses an intersection between different canonical surface points merely because their target pixels overlap.

Equivalently, the composite order is target nearness first across different lifted surface points and the quality tuple `(residency, rung, density, error, age, tile_id)` only inside one canonical same-point equivalence class. There is no total tile-level quality order that can replace this per-fragment decision.

Exact coincident fragments use low-to-high quality draw order with `LessEqual` as a deterministic final tie, but draw order is not the proof for approximate duplicates. Adding quality offsets or polygon bias to target depth is forbidden because it can move a lower-quality near surface behind a higher-quality far surface.

Tiles from the same source view share apron samples at source edges and must reproduce identical transformed edge vertices. Tiles from different source views usually have different triangulations, so the same-surface ownership partition chooses one representation across an overlap band; boundary witnesses require no uncovered target pixel and at most 1.0 px disagreement with direct projection.

The per-pixel palette lookup happens after the fragment survives ownership and depth. The shader maps the winning escape record to linear color, applies exposure and tone mapping, and encodes the target; an optional arena HDR path writes one frame-scoped linear target and performs its resolve in the same reprojection layer.

The pass may load the transient clear color, but known exterior is then shaded as sky and every valid tile overwrites its covered samples. A whole-frame clear is reserved for a slice or MAIN change with no matching rendered content; within a matching partition, clear can survive only as an explicitly counted local hole outside delivered coverage, at a disocclusion no cached view contains, or at an oracle refusal. Ordinary camera motion inside the delivered backdrop must therefore land on coarse depth-bearing content rather than clear, and an all-edge-on request may be all sky.

## Per-tile refinement, backdrop, and scheduling

The Preview, Interactive, and Final ladder becomes a family of rendered tiles for a requested source-screen region and render pose. Refinement may replace value and depth together only after both spans and their reconstruction receipt complete; a value-only or depth-only completion is not publishable.

The scheduler is back-ordered by need with the explicit ascending key `K(job)=(coverage_class,-visible_benefit,work_cost,stable_job_id)`. `coverage_class=0` means the job closes at least one coverage hole in the requested frame at any pyramid or refinement level, and the scheduler chooses the coarsest admitted level sufficient to close that hole, including a Backdrop tile; `coverage_class=1` means every affected pixel is already covered and the job only upgrades quality. Every class-zero job precedes every class-one job regardless of their relative detail levels.

Within either class, `visible_benefit=visible_area*quality_gain`, where visible area counts requested-frame pixels newly covered or improved and quality gain is the monotonic delivered-quality increase from Missing, Backdrop, Preview, Interactive, or Final. Larger benefit runs first, lower proved work cost breaks equal benefit, and stable job ID makes the order reproducible; thus a missing part of a wide view outranks a Final upgrade over an already usable picture.

Visible-first rendered-tile families remain the source of demand, but “visible first” now means visible coverage before visible refinement. A draft may fill a hole because no incumbent exists, yet the draft-never-displaces rule still forbids it from replacing any better covered sample; a class-one publication replaces an incumbent only after its header and paired `S0/S1` spans complete transactionally and win the same-surface chart-ownership comparison. Quality and age never select resolution between different surface points.

Invisible Detail tiles are never newly rendered. A Backdrop job whose transformed footprint closes a requested-frame hole is ordinary class-zero work and may consume the main tile budget before every Detail upgrade. Proactive off-screen maintenance of the protected 3×3 backdrop window uses only its separately bounded rolling budget, may begin only when no runnable class-zero requested-frame job remains, and may not consume the last credit, span pair, or publication slot needed to close a visible hole.

Nine active backdrop tiles stay pinned, while three rolling rendered tiles, each with a descriptor header and paired `S0/S1` spans, let one row or column update without discarding the prior coverage. Each backdrop is still stamped with the source pose that rendered it; rolling the chart window or changing its preferred source angle creates new rendered tiles and leaves old ones as ordinary history once pins transfer.

The cache should retain useful pose diversity rather than forty-four nearly identical angles. After visible quality and recency, eviction policy favors tiles that uniquely cover a chart region or source-view direction, and the page publishes the delivered distribution rather than hiding it behind one LRU count.

One reference orbit is shared and pinned by tile jobs in a MAIN generation. Every tile records its reference and precision receipt; a broad backdrop cell outside the orbit's certified region remains missing or triggers the explicit reference policy rather than receiving guessed records.

Palette, exposure, and tone changes enqueue no tile work. The next placement pass shades the same selected value records with new frame inputs, so display response is immediate even when all geometry comes from old render poses.

Publication and selection both preserve the draft rule. A completed tile can enter the cache without displacing a better same-pose family member, and it can contribute only chart cells for which it wins same-surface quality or supplies coverage absent from better tiles.

## Device-floor resource model

The current heap's unit is one nearest-loaded RGBA32F record of 16 bytes, exactly the descriptor map's texel. Each rendered tile uses one 256×256 page for `S0` values and one for `S1` lifted positions, while all tile headers and the active-instance list share one 256×256 descriptor page; no new texture format, filterability, or binding class is required.

The version-one ABI is decided at two records and 32 bytes per sample, not an information-theoretic compact estimate. The 512-byte header is a power-of-two record block, each sample column is exactly one 1 MiB page, and the shared descriptor page's active list, 64 headers, and chart-ownership arena exactly occupy 1 MiB.

In the existing example 64-page DATA heap, the shared descriptor page consumes one of the eight pages already reserved for the orbit and other non-sample users, leaving seven such pages and 56 sample pages, or 28 rendered tiles. Protecting twelve backdrop tiles consumes 24 sample pages and leaves sixteen Detail/history tiles; if the other users require all eight pages independently, the constructor must reduce the cache to 27 tiles or grow the heap rather than alias the descriptor map.

Keeping twelve backdrop plus forty-four Detail/history tiles requires 112 sample pages, one descriptor page, and seven other pages: 120 pages total. With four 256 pages per 512×512 layer that is 30 RGBA32F array layers and 120 MiB of physical DATA allocation, subject to the constructor's memory and layer walls even though it is below the 256-layer dimensional limit.

The descriptor lattice has an independent span-directory wall: one shared header span plus two sample spans per resident tile requires 57 entries for 28 tiles and 113 entries for 56 tiles before the reference orbit or other live spans. The heap demo's 16-entry configuration is therefore insufficient; the Julibrot constructor must deliver and publish at least 64 entries for the constrained tier and 128 for the expanded tier, with enough page-handle records for every sample, header, orbit, and scratch page, or reduce the cache explicitly. These directory records remain comfortably below a 16 KiB binding when configured, but their actual delivered capacity is a gate rather than an assumption.

Each full-core mesh has 64,516 vertices, 64,009 cells, 128,018 triangles, and 384,054 `u32` indices. One immutable 1,536,216-byte index buffer is shared by every full-core tile; vertex positions are generated from vertex index and placement entry, so cached per-tile vertex buffers are unnecessary.

A naïve implementation issues one draw per transformed tile. The target instead uses one instanced indexed draw for every common mesh and pipeline class: instance index loads an active-list texel, that texel names a descriptor header, and the header names the two sample spans. Unusual clipped or coarsened meshes form a small bounded number of additional batches inside the same render pass.

The former ad-hoc 192-byte per-placement presentation-uniform entry is removed. Source pose, source rectangle, sample handles, quality tie-break, and ownership reference live in the descriptor map; the bounded presentation uniform contains only the requested target pose and frame shading inputs, while the 64-entry active list costs 1,024 bytes in the descriptor page. The heap's generic descriptor, span-directory, header, and resource uniforms remain fixed executor bindings. This keeps per-tile presentation state out of the 16 KiB uniform floor and makes the 64-instance wall explicit.

The target uses one canvas-sized `Depth24Plus` attachment, cleared once and shared by every tile batch. Four bytes per pixel is a useful planning estimate—1.98 MiB at 960×540 and 7.91 MiB at 1920×1080—but actual allocation and padding are implementation facts, while the descriptor map's lifted-position spans remain separate and persistent.

Two RGBA32F outputs fit under the four-attachment floor when `S0` and `S1` are produced together. The executor must nevertheless declare output, SCRATCH, directory, handle, header-slot, and copy capacities for transactional descriptor-map publication; no design argument assumes an unused attachment or directory entry.

The cache allocates no palette-color atlas. Direct shading adds no color target beyond ordinary presentation; if wider arena composition needs linear accumulation, exactly one frame-scoped RGBA16F transient costs 3.96 MiB at 960×540 or 15.82 MiB at 1920×1080, is cleared and logically released every frame, and is never keyed or evicted as cache.

## Cost model and eviction

All retained totals below include the descriptor-map sample pair and 512-byte header slot, but exclude the one shared physical header page, SCRATCH, reference orbit, shared index buffer, target depth, ordinary presentation, and optional frame transient unless stated otherwise.

|Resident set|`S0+S1` sample bytes|Header bytes|Logical total bytes|Binary MiB|
|------------|-------------------:|-----------:|------------------:|---------:|
|One 256×256 rendered tile|2,097,152|512|2,097,664|2.000488|
|Nine active backdrop tiles|18,874,368|4,608|18,878,976|18.004395|
|Twelve protected and rolling backdrop tiles|25,165,824|6,144|25,171,968|24.005859|
|Sixteen Detail/history tiles in the 64-page profile|33,554,432|8,192|33,562,624|32.007813|
|Twenty-eight total tiles in the 64-page profile|58,720,256|14,336|58,734,592|56.013672|
|Forty-four Detail/history tiles in the expanded profile|92,274,688|22,528|92,297,216|88.021484|
|Fifty-six total tiles in the expanded profile|117,440,512|28,672|117,469,184|112.027344|

The screen-aligned 254-sample core covers 253 source intervals per axis. A phase-friendly source view therefore needs 12 tiles at 960×540 and 40 at 1920×1080; arbitrary alignment can require 20 and 54 respectively.

With nine active backdrop tiles, the friendly and worst 960×540 demand is 21 and 29 drawn tiles, referencing 42.010254 and 58.014160 logical MiB of descriptor records, invoking 1,354,836 or 1,870,964 mesh vertices, and submitting 2,688,378 or 3,712,522 triangles before clipping. The 64-page profile can retain twelve Detail plus all twelve protected backdrop tiles in 48 sample pages and 48.011719 logical MiB plus the shared header page, with four additional Detail history tiles; at worst it retains sixteen Detail plus twelve backdrop tiles in 56 sample pages and 56.013672 logical MiB, draws only 25, and leaves four fine regions at backdrop quality.

At 1920×1080 the expanded profile retains forty friendly Detail plus all twelve protected backdrop tiles in 104 sample pages and 104.025391 logical MiB plus the shared header page, leaving four Detail history slots. It draws 49 active tiles with 98.023926 logical MiB of referenced records, 3,161,284 vertices, and 6,272,882 triangles; at worst it retains forty-four Detail plus twelve backdrop tiles in 112 sample pages and 112.027344 logical MiB, draws 53 with 106.025879 logical MiB, 3,419,348 vertices, and 6,784,954 triangles, while ten unavailable fine regions deliberately fall back to depth-bearing backdrop.

These triangle counts are topology submissions, not raster cost. Projection can shrink, magnify, overlap, or clip every source tile, so the page must publish surviving fragments, depth-passed fragments, overdraw, rejected cells, and batches rather than deriving GPU time from source sample count.

A whole screen-aligned descriptor sample pair before tiling logically costs 32 bytes per pixel plus its 512-byte pose header and reserves about 16 MiB at 960×540 or 64 MiB at 1920×1080 in the current RGBA32F page classes; retaining `N` whole views in migration stage 2 multiplies that cost. Splitting into tiles adds page-edge rounding but permits regional eviction and mixed-pose coverage.

Today's retained design costs about 11.96 MiB at 960×540 and 47.82 MiB at 1920×1080 for one Final record reservation plus two LDR scene textures. Friendly rendered-tile draws reference about 42.010 and 98.024 logical MiB, their resident sets including three rolling backdrop tiles cost about 48.012 and 104.025 logical MiB plus the shared header page, and the expanded full cache costs 112.027 logical MiB plus that page before other resources. The design buys GPU-readable pose descriptors, multi-angle lifted history, and return continuity with real memory rather than presenting tiling as a free optimization.

The active backdrop's nine tiles contain 589,824 physical samples in each descriptor column plus 4,608 header bytes. At cap 32 its value computation ceiling is 18,874,368 iterations and at cap 64 it is 37,748,736; lifted-descriptor generation and mesh presentation are additional measured work.

Eviction first removes invalid partitions, corrupt descriptor sets, and failed drafts. It never reuses a header slot or frees either sample span of a selected, in-flight, publication-source, ownership-source, or pinned backdrop tile until all fences and frame references release the complete set.

Among unpinned valid tiles, eviction uses last-selected age, distance from the requested chart footprint, projected usefulness, source-pose distance, unique chart coverage, and pose-direction diversity in that order after protecting better same-surface quality. Stable tile ID resolves exact ties.

The principal wins are reuse after returning to a stored render pose, useful coverage from a different angle, camera continuity through old lifted descriptors, regional replacement, and a coarse backdrop where no fine tile survives. The losses are two RGBA32F sample records per point, millions of mesh vertices, target overdraw, and inevitable holes where retained views never contained the newly visible arena surface.

## Oracle, browser proofs, and published facts

The descriptor-map layout oracle pins 32 header texels, both two-record sample columns, every lane and factor order, the 512-byte slot stride, the 64-record active prefix, 64 header slots, 63,424-record ownership arena, header and span generations, exact-in-f32 integer limits, zero reserved lanes, and the 2,097,664 logical bytes per tile. CPU packing followed by GPU readback must reproduce every finite header lane and both sample records bit-for-bit except the explicitly tolerance-bounded compensated splits.

The descriptor round-trip oracle selects a header and sample using the same active-instance and vertex indices as the draw, reconstructs `O`, `Q`, origin, translation, observer, perspectives, source rectangle, exact-anchor-relative position, value height, and lifted source depth, and produces the source-visible five-dimensional point. It then projects that point back through the stored source pose to the original source pixel and depth, and through an independently supplied requested pose to screen and target depth; both results must agree with direct f64 construction within the declared source and 1.0 px target bounds.

Per-tile reprojection tests transform every edge, a deterministic interior lattice, depth extrema, all five existing height witnesses, and adversarial pole cases from many source poses to many requested poses. Each admitted mesh reports finite maximum and p95 disagreement with direct full-chain projection, with the maximum at most 1.0 px.

Composition tests place two distinct chart points at one target pixel and require the nearer target depth to win for every draw order, source age, and quality ordering. They include crossing relief folds, nearly coincident but distinct depths, a far Final against a near Preview, and tiles rendered from opposing source angles.

Same-surface tests render one chart region from several poses and rungs, require the ownership mask to choose the best representation independent of catalog and draw order, and then require exact coincident remnants to follow low-to-high quality `LessEqual` ordering. A quality change may never reverse two different surface points.

Intersection and seam tests cover same-view tile edges, arbitrary different-view footprint intersections, mixed densities, partial validity masks, horizons, subpixel target motion, and chart-ownership boundaries. They require no double same-surface claim, no false hole, correct near/far ownership, and at most 1.0 px disagreement with direct projection.

Shading tests begin with the winning value record after depth and ownership, evaluate a CPU reference palette-to-linear mapping plus exposure, tone mapping, and output encoding, and compare the final GPU pixel within format tolerance. Display-only changes must retain tile IDs, source depth, and escape-dispatch counts and appear in the next frame without recolor work.

The arena-boundary oracle includes a synthetic three-dimensional slice scene in which one source view hides a surface that a target view reveals. The expected result is a reprojection hole until another rendered view or new render supplies it; any test that fills it from the foreground depth fails because the lab must not teach a false disocclusion guarantee.

The first browser proof renders pose A, a materially different angle B, then returns to A under one content key. It shows both pose families resident, zero new escape work for A, per-tile descriptor header and sample-span identities, vertex-stage RGBA32F reads, correct target depth, and no stale LDR texture or per-tile source uniform use.

The second proof retains several whole rendered views before chart tiling, reprojects all of them into an intermediate pose, and shows a one-pass depth image with constructed intersections whose result is independent of submission order. This pins migration stage 2 as the mechanism rather than merely a larger scene ledger.

The third proof enables screen tiles and the direct chart walk, pans and zooms by partial tiles, and reports visited nodes, returned and deduplicated IDs, transformed footprints, admitted meshes, holes, and work independent of unrelated cache population at 960×540 and 1920×1080.

The fourth proof establishes the nine-tile backdrop, then exercises every camera and observer slider, height zero and nonzero, both perspective distances, rotation, pitch, zoom-out, resize, palette, exposure, and tone without changing MAIN. It shows depth-bearing coarse fallback inside delivered coverage and honest clear, sky, or refusal outside it.

The fifth proof performs plane-preserving object and origin changes, then slice tilt, out-of-plane origin, cap, precision, and reference changes. It keeps every render-pose family for certified same-slice moves and immediately partitions every genuine content change.

The sixth proof reverses Preview, Interactive, Final, backdrop, and fence completion order across overlapping source poses. It proves visible-first scheduling, transactional header-plus-`S0/S1` publication, no draft downgrade, same-surface quality ownership, and physical-depth priority across distinct surfaces.

The seventh proof gives the scheduler one dispatch credit while a coarse tile can close a visible hole in the requested wide frame and a Final tile can upgrade a region already covered by Preview. Every insertion-order permutation must select and publish the hole-closing tile first, leave the Preview incumbent undisplaced elsewhere, report the class-zero key and visible benefit, and select the Final upgrade only after coverage is closed; the browser sequence visibly removes the clear hole before sharpening the covered region.

The page must publish content and render-key digests; source-pose family; descriptor-map ABI, header slot, active index, anchor ID and revision, compensated-anchor error, value and lifted span indices and generations, exact logical and reserved bytes, source rectangle and extent, depth range and validity; derived chart footprint, density, index nodes, and uncertainty; requested and delivered backdrop extent; total, backdrop, Detail, pinned, candidate, deduplicated, selected, clipped, corrupt, and evicted tile counts; and pose-diversity facts.

For each frame it must publish the requested pose digest, chart nodes visited, candidate IDs returned, forward bounds tested, placement receipts accepted and refused by reason, scheduler coverage class, visible area, quality gain, visible benefit, selected stable job ID, deferred upgrades, backdrop-maintenance credits, mesh classes, vertices, triangles, batches, draw calls, target-depth mode, same-surface mask cells, shaded fragments, depth-passed fragments, overdraw, clear and sky fractions, optional HDR transient bytes, normalized compute and copy work, and CPU and fence walls.

Correctness facts include descriptor pack/readback agreement, maximum and p95 source reconstruction error, target reprojection error, depth disagreement, seam disagreement, horizon and pole counts, glitch and coordinate-uncertain counts, same-surface conflicts, quality-tie decisions, distinct-surface intersection decisions, stale-generation refusals, draft-downgrade refusals, and whether every displayed tile region has a current oracle receipt.

## Staged migration

Every stage keeps the page usable behind a capability switch, retains the last proved path until the new one reaches parity, and adds browser-visible facts before deleting fallback code. Lane-hours are focused implementation, tests, browser proofs, documentation, and review work for one engineer familiar with the lab.

|Stage|Working implementation at the end|Pinning tests and proofs|Estimate|
|-----|---------------------------------|------------------------|-------:|
|0. Rendered-view policy and math|Add render keys, the exact descriptor-map ABI, source reconstruction, target full-chain projection, slice validity, derived chart footprints, same-surface ownership, and cost types without changing the current page|Header and sample packing, pose serialization, descriptor round-trip, direct target projection, same-slice equivalence, invalidation matrix, resource arithmetic|20–28 lane-hours|
|1. One depth-bearing retained view|Replace the retained LDR source dependency with one whole-screen descriptor-map view; reproject its mesh per vertex through the full target chain and shade values at placement, retaining the old homography path as a switchable fallback|GPU descriptor readback, self-reprojection, material camera deltas, height changes, poles, 1.0 px admission, palette and HDR immediacy, current preset parity|36–52 lane-hours|
|2. N retained rendered views reprojected and depth-composited|Generalize the ledger to bounded whole descriptor-map views from different source poses and submit all selected meshes in one target depth pass before introducing chart tiling|Opposing angles, crossing relief, near Preview versus far Final, same-surface quality ties, order independence, holes, fence-safe header and sample spans, N=1 parity|40–60 lane-hours|
|3. Screen-aligned rendered tiles and chart index|Split each rendered view into fixed source-screen tiles, derive chart footprints and density nodes, direct-walk and deduplicate candidates, batch instanced meshes, and permit regional publication and eviction|Same-view aprons, arbitrary cross-view overlaps, exhaustive index versus slow scan, unrelated-cache scaling, uniform and draw walls, both target extents|56–84 lane-hours|
|4. Backdrop and per-tile refinement|Add nine active plus three rolling coarse rendered backdrop tiles, hole-before-upgrade demand ordering, visible-first tile families, transactional descriptor-map refinement, reference sharing, pose-diverse retention, and protected eviction|3× extent, 127–253 interval density, rotate/pitch/zoom-out fallback, one-credit hole-versus-Final ordering, rolling fences, no invisible Detail work, no backdrop or draft displacement|40–60 lane-hours|
|5. Hardening and arena-facing proof|Make the rendered-tile path default after parity, publish complete resource and intersection facts, add long navigation and device-loss soak, and pin the explicit arena disocclusion counterexample|All sliders and presets, 64- and 120-page profiles, capacity refusals, source corruption, depth precision, overdraw walls, return proofs, arena hole honesty, console cleanliness|32–48 lane-hours|

The total estimate is 224–332 lane-hours. Stage 2 intentionally precedes chart tiling: it proves that several independently rendered, depth-bearing views can be transformed and composited as the arena mechanism before storage subdivision and cache indexing complicate the evidence.

## Backlog

- `JB-TILE-001` — Specify canonical slice identity, full render-pose serialization, exact bignum-anchor interning, source pixel rectangles, the 32-texel header and two-texel sample ABI, and exact cache-key equality with display controls excluded.
- `JB-TILE-002` — Implement descriptor-map packing, GPU readback, pure source-sample reconstruction, and full target five-dimensional projection, including source self-round-trip and 1.0 px target receipts.
- `JB-TILE-003` — Define transactional header-plus-`S0/S1` publication, corruption and fence behavior, the shared header page and active list, and compensated deep-anchor placement.
- `JB-TILE-004` — Generalize the retained ledger to N whole rendered views from different poses and depth-compose their independently transformed meshes in one pass.
- `JB-TILE-005` — Implement same-surface identity and ownership masks, low-to-high exact-tie ordering, and adversarial proof that quality cannot reverse distinct physical depths.
- `JB-TILE-006` — Move palette-to-linear lookup, exposure, tone mapping, and output encoding into per-fragment reprojection shading with no retained color payload.
- `JB-TILE-007` — Split rendered views into 256×256 source-screen tiles with 254×254 cores, aprons, shared index topology, and paired span ownership.
- `JB-TILE-008` — Add conservative derived chart footprints, density levels, multi-node tile membership, direct candidate traversal, ID deduplication, and a slow exhaustive oracle.
- `JB-TILE-009` — Add per-tile placement receipts, cell-local poles and holes, full-core and proved-coarse mesh classes, instanced batching, and the 64-entry capacity wall.
- `JB-TILE-010` — Add and prove the nine-active plus three-rolling depth-bearing rendered backdrop with published coverage, source poses, certification holes, and work walls.
- `JB-TILE-011` — Replace the global ladder with visible rendered-tile families, the lexicographic hole-before-upgrade scheduler, bounded backdrop-maintenance credits, transactional descriptor-map publication, no-draft-downgrade rules, and pose-diverse eviction.
- `JB-TILE-012` — Publish source-pose, source-depth, chart-index, candidate, scheduler-priority, mesh, intersection, overdraw, shading, resource, eviction, and timing facts and add seven browser proofs.
- `JB-TILE-013` — Study promotion of certified Final value records across reference generations without weakening source-depth or MAIN provenance.
- `JB-TILE-014` — Build the arena disocclusion counterexample and document which rendered-view mechanisms transfer from the 2D lab and which require new 3D slice content.

## Risks

The largest conceptual risk is accidentally returning to chart-image thinking. A chart footprint is only an index receipt; shader inputs, depth, reconstruction, coverage, and reuse must remain attached to the source screen tile and its render pose.

Source depth needs a precise reusable definition. Caching nonlinear `Depth24Plus`, omitting clip sign, or reconstructing through a different camera convention can move points or reverse intersections; the source self-round-trip and independent full-chain oracle are release gates.

RGBA32F descriptor precision cannot become deep-position authority. Reusing an `anchor_id` after its exact CPU interner entry changes, uploading an uncertified high/low residual, or overflowing the exact-in-f32 integer range can place an otherwise valid tile on the wrong scene part; generation checks, compensated-split bounds, and descriptor round-trip refusal are release gates.

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

1. Descriptor-map ABI — decided by the owner: use the versioned RGBA32F descriptor map specified above, with a 32-texel pose header and the exact two-texel sample pair `S0=existing value record` and `S1=(a_F,b_F,zeta_F,validity)`. This is 512 header bytes plus 32 bytes per sample, uses the heap's existing 16-byte record alignment, and never caches quantized `Depth24Plus`.

2. Is 256×256 physical with a 254×254 source-screen core and one-sample apron still the first tile geometry? Recommendation: yes for the first proof because it reuses the heap page and one shared mesh, but treat 128 and 512 as measured source-screen alternatives rather than chart levels.

3. How should same-surface quality ties be implemented? Recommendation: use canonical chart-cell ownership masks before depth, then low-to-high quality submission with `LessEqual` only for exact coincident remnants; never bias true depth with quality.

4. Which cache profile is the product floor? Recommendation: expose the existing 64-page, 28-rendered-tile constrained profile first, and make the 120-page, 56-rendered-tile profile an explicitly delivered higher memory tier until weak-device measurements justify changing the floor.

5. How many transformed candidates may one frame submit? Recommendation: retain a constructor-validated 64-entry active-list ceiling, enumerate by direct chart walk and coverage-first scheduler order, submit every admitted intersection up to that wall, and publish overflow and remaining holes rather than running a CPU set-cover or resolution ranking.

6. Does reprojection need a frame-scoped HDR color transient? Recommendation: shade directly into presentation for the Julibrot proof; if arena composition requires linear accumulation, permit exactly one RGBA16F canvas target with published bytes and frame lifetime, never retained cache identity.

7. May certified value records survive a reference regeneration? Recommendation: no in version one; keep strict MAIN equality until an independent-value proof also specifies how the source-depth receipt is regenerated or retained.

8. May a tile rendered at one Julibrot height be reused at another? Recommendation: yes because the value record reconstructs base height, but store source height in the render key, validate stored source depth at that height, and recompute target position and depth at requested height.

9. Should version one use every retained sample as a mesh vertex? Recommendation: yes; the full 254×254 core is the honest baseline. Add coarser mesh classes only when interior witnesses prove the target error ceiling and measurements show the vertex reduction matters.

10. How should eviction value different render angles? Recommendation: protect visible quality and backdrop first, then retain tiles with unique chart coverage or view direction before redundant nearby poses; publish pose buckets so “multi-view” is measurable.

11. What arena claim may the lab make, and what identity must the arena provide? Recommendation: claim proof of pose-stamped value/depth retention, full-chain per-view reprojection, depth intersections, chart-backed same-surface quality ownership, and honest gaps; require the arena to supply stable surface or primitive identity before transferring that quality rule, and explicitly do not claim hidden-surface recovery or complete 3D slice reprojection.

12. Should rendered tiles persist across reloads? Recommendation: remain session-local until pose, bignum, value, depth, ABI, corruption, quota, and privacy contracts are versioned together.

13. What deep-zoom precision contract should the descriptor map expose? Recommendation: keep the absolute bignum anchor in the CPU's exact intern table, store only its exact-in-f32 `anchor_id` and revision in the RGBA32F header, materialize the source-to-request anchor delta as compensated high/low `H17` lanes before each placement, and refuse the tile whenever the independent f64 oracle cannot certify the uploaded split below the 1.0 px bound.

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
