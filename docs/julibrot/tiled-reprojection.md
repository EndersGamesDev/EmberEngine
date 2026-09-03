# Tiled reprojection for the Julibrot lab

Status: draft design study.

## Decision summary

The cache should live in the two-dimensional chart of one affine Julibrot slice, not in screen space: its durable payload is a palette-independent escape-record tile addressed by an exact bignum anchor, a dyadic zoom-pyramid level, and signed integer tile coordinates.

Screen images are derived data. A small RGBA8 atlas may cache the current palette's chart-space texels, but camera pose, observer pose, canvas extent, palette, and surface format must not enter the escape-tile key.

The first implementation should use one 256×256 heap page per tile, with a 254×254 owned core and a one-record apron on every edge, 16-byte RGBA32F escape records, at most 56 resident record tiles in the example 64-page DATA heap, and one 2048×2048 RGBA8 atlas containing 64 256×256 slots. Eight DATA pages remain available for the active reference orbit, publication, and non-cache allocations.

Every request is decided in three separate steps: semantic validity asks whether the records describe the requested mathematical slice and MAIN generation; placement admission asks whether the requested camera projection is finite and stays within the 1.0 px error ceiling; quality selection asks whether a candidate is better than the picture already covering that screen region.

A cache hit may eliminate fractal iteration, but it does not weaken presentation correctness. Flat tiles use their exact chart-to-screen homography; relief tiles use a height-aware mesh whose measured interpolation error is at most 1.0 px; a pole, missing proof, or unbounded error refuses that placement and exposes only the affected region for refinement.

The migration should preserve the current page after every stage: first retain several whole scenes by chart footprint, then introduce chart keys and a catalog, then split computation and presentation into fixed tiles, then add bounded relief placement, and only then replace the single global ladder with a visible-first per-tile scheduler.

## Scope and binding invariants

The picture remains a two-dimensional plane in a four-dimensional slice. Object controls choose an oriented plane with origin in four-dimensional model space; an escape record supplies the optional fifth-coordinate height; the independent camera rotation, camera translation, yaw, pitch, fifth- and fourth-dimensional perspective distances, and height scale only determine how that already-computed surface is observed. This follows the lab model in [Julibrot math](math.md) and [the Julibrot lab charter](../julibrot-lab.md).

All view degrees of freedom remain sliders and presets remain rows. Tiling changes storage, scheduling, and presentation ownership, not the control model or its user-facing parameterization.

Reprojection remains a navigation primitive. A cached or stale tile may hold the picture together while navigation is active, but a resolving tile adds detail without moving already-correct features, and no approximation may be shown when its bound is absent or exceeds 1.0 px. The current contract is recorded in [Julibrot presentation](present.md) and [the precision ledger](precision-ledger.md).

A draft may fill a hole but may never displace a better picture. This is a regional monotonicity rule, not merely a scheduler preference: publication and compositing both compare the incumbent and candidate quality keys.

The warpable model motions are exactly the stabilizer of the sampled affine plane: changes within that plane may be expressed as a chart isometry and re-keying, while a plane-normal rotation or a nonzero out-of-plane origin displacement asks for samples that a two-dimensional tile does not contain. This is the after-time-warp limit in `origin/docs/4d-arena:docs/4d-first-engine.md` §5, with the same phase and normal-motion boundary described in `origin/docs/4d-arena:docs/4d-torus-world.md` and `origin/docs/4d-arena:docs/4d-content.md`.

The three-dimensional solid slice belongs to the arena and is out of scope. This proposal neither adds volume samples nor pretends that depth, a guard band, or neighboring chart tiles can recover data off the selected two-plane.

The implementation must keep the repository's one-way crate layering and heap ownership rules, and must run on the WebGL2 plus `EXT_color_buffer_float` device floor in [minimum requirements](../minimum-requirements.md). It may not rely on float filtering, float blending, timestamp queries, storage buffers, or an unbounded number of bindings.

The priorities are finish the two-dimensional lab, make it fast, make it robust, and expose an honest interface. Accordingly, this study prefers a conservative fixed tile geometry and explicit walls over a more general sparse-texture system.

## Coordinate model and terminology

Let the requested affine slice be `X(a,b) = o + a u + b v`, where `o` is the four-dimensional origin and the orthonormal columns `u` and `v` are obtained from the object rotation `O`. Let `r(a,b)` be the palette-independent escape record and `z(r)` its normalized height before the current height-scale slider. The point presented to the five-dimensional camera is `Y(a,b) = (X(a,b), z(r))`.

For a fixed target camera pose `P`, neutral height has a projective map `s ~ M_P [a b 1]^T`. A rectangular chart tile is therefore placed exactly by a homography when `z = 0`; no source-screen pose is needed. When height is nonzero, vertices use the full five-dimensional projection `s = Pi_P(Y)`, and the rasterized triangle interpolation is an approximation whose screen-space error must be measured.

The current retained-scene warp composes a source screen map with a target screen map and validates the composition over a 9×9×5 screen-and-height corpus. The tile oracle keeps its 1.0 px ceiling but changes the object being certified: it measures each tile's chart homography or relief mesh against direct projection at the target pose, then measures agreement along selected tile boundaries.

The word “level” is overloaded and must be split in all APIs and facts. `pyramid_level` is the dyadic chart sampling scale; `refinement_level` is Preview, Interactive, or Final; `iteration_cap` is the delivered escape cap; and `quality` is the complete ordered record used by selection.

The word “valid” means that a record still represents the requested mathematical content. “Placeable” means that a valid record can be projected at the requested pose under the finite 1.0 px contract. “Selected” means that a placeable record wins the regional quality comparison. A valid tile can therefore be temporarily unplaceable without being evicted or recomputed.

The word “MAIN generation” means the coherently published content token, not a frame counter: it binds the canonical slice, origin, precision policy, requested cap, reference-orbit generation, and record ABI. HOT camera and navigation state is deliberately excluded.

## Where tiles live

Today's retained scene is a screen-sized RGBA8 image sampled on the inverse-homography screen grid. Its coordinates, extent, camera pose, and palette are baked into the image, so the two-slot ledger can reproject only the most recently promoted view and loses an older position when that slot is reused. [Julibrot presentation](present.md) describes that two-texture ownership model, and [Julibrot kernels](kernels.md) describes the one Final-capacity screen-aligned `EscapeGrid` behind it.

A reusable tile instead owns a rectangle in chart coordinates and carries the escape records for fixed chart sample points. Returning to a chart rectangle with the same content token finds the old records even if the canvas, camera, observer, palette, or intervening view differed.

The pyramid uses an immutable bignum chart anchor `A` on the slice and dyadic texel spacing `sigma_l = 2^-l` chart units at signed integer `pyramid_level l`. A key contains `anchor_id`, `l`, and arbitrary-precision signed tile coordinates `(i,j)`; it never contains an f64 absolute coordinate.

For a 256×256 physical tile, indices `r,s` range from 0 through 255, indices 1 through 254 are owned, and the chart sample is `A + sigma_l ((254 i + r - 1/2) u + (254 j + s - 1/2) v)`. The owned core covers the half-open chart rectangle `[254 i sigma_l, 254 (i+1) sigma_l) × [254 j sigma_l, 254 (j+1) sigma_l)`, while indices 0 and 255 duplicate the adjacent tile's last and first owned sample respectively.

The half-open core gives every chart point one owner and the apron gives nearest sampling and relief triangulation one sample beyond every owned edge. It also maps exactly to the heap's 256×256 page class, avoiding the fourfold buddy-allocation waste that a 258×258 allocation would cause.

At a canonical flat view, the current pixel scale is `4 / (2^q W)` for zoom `q` and grid width `W`; the matching pyramid level is therefore near `q + log2(W/4)`. A tilted or perspective view chooses levels from the local chart-to-screen Jacobian rather than from zoom alone, so different screen regions may select different pyramid levels without changing their mathematical position.

Deep-zoom identity is exact because the anchor and signed indices are serialized from bignum and integer components and the offset is dyadic. Hashing may accelerate lookup, but equality must compare the canonical serialized value after the hash matches; a finite mirror is never an identity key.

Precision is decided per tile. The tile computation must publish a conservative coordinate-error bound `epsilon_coord` satisfying `epsilon_coord <= sigma_l / 4` at every owned sample after bignum center arithmetic, plane-basis multiplication, reference displacement, and the cap-dependent perturbation allowance; its descriptor records requested bits, delivered bits, and the proven bound. The existing precision policy and quarter-pixel discipline provide the starting contract in [Julibrot math](math.md) and [the precision ledger](precision-ledger.md).

The durable record is palette-independent and retains smooth escape, escaped classification, rebase count, and status, as today's 16-byte RGBA32F escape record does. Height is derived from those facts and the current height slider, so a tile does not become invalid when height scale or palette changes.

An RGBA8 atlas slot is an optional, rebuildable chart-space colorization of a record tile for one palette version. It accelerates placement but is not authoritative: dropping an atlas slot leaves the record tile valid, and changing palette invalidates or recolors atlas slots without invalidating record spans.

| Property | Current screen scene | Proposed chart tile |
|---|---|---|
| Identity | Screen extent, screen map, pose, palette, scene generation | Canonical slice and MAIN generation, bignum anchor, pyramid level, integer chart coordinates, record ABI |
| Durable payload | RGBA8 pixels plus one globally reused record grid | Palette-independent escape records per tile |
| Camera change | Warp the last screen image if admitted | Reproject the same chart records; no content invalidation |
| Return to an older position | Miss after the retained slot is replaced | Hit while the keyed tile remains resident |
| Deep zoom | Position is relative to the current reference and screen width | Exact bignum anchor plus dyadic integer offsets; precision receipt per tile |
| Height lift | Approximation inferred from the old scene's record field | Records remain resident and directly supply mesh height |
| Palette change | Re-render the full scene image | Recolor selected atlas patches from unchanged records |

## Tile descriptor and heap relationship

The CPU catalog owns semantic descriptors. Heap descriptors remain physical GPU addresses and must not be overloaded with bignum keys or cache policy.

Each semantic tile descriptor contains a stable `tile_id` and a `content_key` comprising the formula and record-ABI version, canonical geometric slice key, MAIN generation, precision mode, requested and delivered cap, and reference-orbit generation. Palette is explicitly absent.

The descriptor retains slice provenance sufficient to audit canonicalization: the six requested object angles or their exact control bytes, the delivered plane basis, the bignum plane origin and its finite mirror, the canonical normal-space identity, and the chart-frame transform. Keeping both provenance and a canonical key prevents a plane-preserving object reparameterization from masquerading as a new slice.

The chart portion contains `anchor_id`, canonical anchor serialization or an interned handle to it, signed `pyramid_level`, arbitrary-precision `(i,j)`, physical extent 256×256, owned extent 254×254, the exact half-open chart rectangle, texel spacing, and orientation from stored chart coordinates to the canonical slice chart.

The quality portion contains `refinement_level`, delivered sample extent, delivered iteration cap, coordinate-error bound, record completeness, and a monotonic quality key. Pyramid level and refinement level are separate fields.

The orbit provenance contains reference generation, reference-center revision, orbit registry identity while live, orbit length, delivered precision bits, and perturbation policy. The registry handle is operational and may expire; the immutable generation and precision receipt remain part of the tile descriptor.

Intrinsic error facts contain sampled, glitch, and coordinate-uncertain counts, maximum coordinate error in tile-texel units, and whether every Final record was certified. Horizon and projection uncertainty are camera-dependent, so the descriptor stores them in a latest-placement receipt keyed by target pose rather than treating them as permanent content facts.

Lifetime facts contain creation serial, last-selected serial, last-visible serial, hit count, recompute count, current pin reasons, publication fence state, and eviction class. Age is a monotonic serial rather than wall-clock time so ordering remains deterministic in tests.

Storage facts contain the generation-checked `DataSpan`, logical and reserved record bytes, initialized prefix or full-tile state, optional RGBA8 atlas slot plus slot generation and palette version, and any temporary scratch or publication ticket. A catalog entry is not selectable until its DATA copy and atlas write, if required, have crossed their completion fence.

Placement receipts contain target pose digest, chosen warp kind, screen coverage polygon, selected pyramid mismatch, maximum and p95 projection error, horizon-clipped samples, projection-uncertain samples, seam checks, and refusal reason. These receipts can be replaced freely because they do not define content identity.

[GPU resource heap](../gpu-resource-heap.md) already supplies generation-checked logical handles, stable DATA and IMAGE heap bindings, typed allocation walls, regional content writes, and square buddy allocations. [GPU heap lattice](../gpu-heap-lattice.md) adds multi-page `DataSpan` ownership, a uniform span directory, transactional paired allocation, fixed input/output arity, fixed kernel-uniform capacity, SCRATCH-to-DATA publication, and generation-safe free.

The 256×256 tile choice deliberately fits one DATA page, so most tile spans have one page and one directory entry. Larger reference orbits may still use the existing multi-page span machinery, and the presenter can read every selected tile through the same stable heap view.

The heap does not supply the semantic catalog, canonical bignum key, spatial index, slice-equivalence proof, quality order, regional cover solver, visible-first queue, atlas-slot generation, LRU and distance policy, multi-tile publication transaction, seam ownership, or per-pose oracle. Those belong above `heap`, split between pure Julibrot math/policy and the app/present owners according to the existing layering.

The executor also does not promise arbitrary tile counts. Its descriptor, span-directory, handle, input, output, header-set, scratch, and kernel-uniform capacities are constructor contracts; tile planning must reserve them explicitly, publish requested versus delivered counts, and degrade the cache rather than indexing beyond a wall.

## Validity, compatibility, and invalidation

The active `ContentKey` is equal only when the canonical geometric slice, plane origin, formula and record ABI, precision mode, requested cap, delivered-cap semantics, and MAIN/reference generation agree. “The model has not changed” means exactly that equality; visual similarity and nearby slider values are not substitutes.

Content invalidation is immediate logical unavailability, not necessarily immediate GPU destruction. When MAIN changes incompatibly, all old descriptors leave the selectable set before the next frame; their spans and atlas slots may be reclaimed later after fences, but they cannot fill the new picture.

Camera `Q`, five-dimensional camera translation, yaw, pitch, `d5`, `d4`, canvas extent, and height scale do not change escape records. They leave a tile content-valid; the tile is placeable only if target projection is finite and its flat or relief placement proof is at most 1.0 px. A large or pole-crossing camera move may therefore refuse every old placement while retaining every record for a later return.

Chart pan and zoom do not change content. They alter the requested chart footprint and preferred pyramid level, selecting existing tiles where available and scheduling only uncovered visible tiles. Resizing the canvas behaves the same way.

A palette change leaves every record tile valid and changes only derived atlas colorization. Until recoloring completes, a slot with the wrong palette is not selected; the composer may color directly from records, use a correctly colored older slot, or show the clear color in the uncovered region, but it must not label the wrong palette as current.

A plane-preserving object motion is valid as a chart re-key. The compatibility proof first shows equality of the two affine planes: their two basis spans agree within the delivered basis-rounding bound and the origin's normal residual is zero within the same bound. It then derives the two-dimensional isometry `c_new = R c_old + delta` from basis dot products and the in-plane origin displacement.

If `R` and `delta` map the dyadic lattice exactly, the catalog may rewrite or alias integer keys. For an arbitrary in-plane rotation or non-dyadic translation, records still describe the same geometric sample points, but their footprint is a transformed quadrilateral in the new chart; the catalog re-indexes that footprint without resampling it, and new edge wedges remain holes until computed. This is semantic re-keying, not interpolation of escape records.

The present implementation compares object-angle samples at the f32 plane floor and permits at most one-half source pixel of origin residual before clearing. Tile compatibility should preserve that fail-closed tolerance while finite mirrors exist, but the long-term key should use bignum origin arithmetic and a certified normal residual measured against the finest source texel, not a screen pixel.

A slice-tilting object motion changes the plane span and invalidates all active tiles. An out-of-plane origin motion changes the affine plane and invalidates all active tiles; an arbitrarily small nonzero mathematical displacement is new content, with only representation-rounding uncertainty admitted by the proof.

Changing requested iteration cap invalidates all active tiles in version one. The existing four-lane record does not retain the first escape iteration in a form that proves what classification a different lower cap would have produced, and increasing the cap plainly needs new work. A future richer record may define a safe cap partial order, but visual inference is forbidden.

Changing precision mode invalidates all active tiles because Deterministic and PictureFast select different refinement and arithmetic promises. Merely delivering more bits within the same policy may produce a strictly better quality entry, but it must carry a new descriptor and pass the regional replacement rule.

Accepting a new reference orbit or shallow MAIN generation invalidates all active tiles in the conservative first version. Today's whole-scene ledger rebases a retained pose across an accepted reference shift, but the app simultaneously replaces and frees the prior orbit span; generation-keyed tiles cannot claim equivalent perturbation provenance without a separate certification. This restriction is intentionally visible as a risk and owner question because frequent reference regeneration can erase the principal pan/return benefit.

Reference generation is a computation provenance boundary, not inherently a change to the fractal formula. A later stage may promote fully certified records to a reference-independent content generation, but only after a proof shows that their values and uncertainty bounds are independent of the discarded orbit; that optimization is not assumed here.

| Requested change | Record validity | Catalog action | Placement action |
|---|---|---|---|
| Camera rotation or translation, yaw, pitch, `d5`, `d4`, height scale | Keep | None | Recompute target placement; accept only finite error at most 1.0 px |
| Pan, zoom, resize | Keep | Query different chart footprint and pyramid levels | Select partial cover and expose holes |
| Palette | Keep | Retain record keys; expire or recolor atlas slots | Use only matching palette version |
| Plane-preserving `O` change or in-plane origin move | Keep after affine-plane proof | Exact key alias or transformed-footprint re-index | Recompute placement in new chart frame |
| Slice tilt | Invalidate all active tiles | Start new content partition | Clear or sky until new content arrives |
| Out-of-plane origin move | Invalidate all active tiles | Start new content partition | Clear or sky until new content arrives |
| Iteration cap | Invalidate all active tiles in version one | Start new MAIN partition | Do not reuse visually similar records |
| Precision mode | Invalidate all active tiles | Start new MAIN partition | Do not mix policies |
| Reference-orbit or shallow MAIN generation | Invalidate all active tiles in version one | Start new generation partition | Do not select old provenance |

## Selection and placement

Selection begins by deriving the target's visible chart region. For the flat surface, inverse-map the screen corners and any horizon intersections into a chart polygon. For relief, conservatively bound projected tile meshes and use the current height range; a bound that crosses a pole is not evidence of coverage.

The screen is partitioned into a small deterministic ownership grid, initially 32×32 pixel macroregions clipped at the canvas edge. A spatial index enumerates every valid tile whose transformed owned-core footprint intersects each macroregion, considering the locally ideal pyramid level and at least its adjacent coarser and finer levels.

Each candidate receives a placement receipt for the target pose before ranking. Candidates with an incompatible content key, stale span or atlas generation, incomplete publication fence, wrong palette slot, projective pole, unbounded relief, or maximum error above 1.0 px are removed rather than penalized.

The quality order is lexicographic and total: certified Final beats Interactive beats Preview; within a rung, a delivered cap satisfying the request beats a lower cap; then smaller projected texel error and closer pyramid match win; then lower coordinate, glitch, projection, and warp error win; then newer last-selection age breaks an otherwise exact tie. Tile identity is the final deterministic tie-breaker.

Age can never make a draft beat a better refinement or make an uncertain tile beat a certified one. It exists only to choose between equivalent pictures and to stabilize cache behavior.

The cover solver visits candidates from best to worst and assigns only macroregion fragments not already owned by a better candidate. A candidate may fill a hole or replace a strictly worse incumbent after a completed publication, but it cannot draw over an equal-or-better incumbent. The resulting coverage mask, not draw submission order, is the authority.

The composer groups selected fragments by pipeline, record or atlas binding, and mesh class. Flat tiles emit the chart rectangle through their target homography. Relief tiles emit the adaptive indexed mesh from chart positions and record-derived heights. Both are clipped to their assigned coverage polygon, so overlapping aprons and coarser fallback tiles do not double-own a pixel.

For a flat tile, the homography is evaluated from the tile's stored chart frame directly into the target screen. Its proof checks coefficient rounding and representative points against direct five-dimensional projection at height zero; the map is exact in real arithmetic, and uploaded f32 coefficients still obey the 1.0 px ceiling.

For a relief tile, start with a fixed 16×16 cell mesh over the owned core and evaluate direct projection at vertices plus deterministic interior and edge witnesses using actual record height bounds. Subdivide a cell while its interpolation bound exceeds 1.0 px, up to a configured vertex and depth wall; if the wall is reached without proof, refuse only that cell and expose it for a newly rendered exact patch rather than displaying an unbounded warp.

The existing 9×9×5 corpus remains a regression corpus, but actual tile records tighten the height range and local witnesses. A proof may never conclude that skipped samples beyond a horizon establish a bound; those samples are classified as exterior or refused according to the same horizon semantics as [Julibrot presentation](present.md).

Seams use four independent rules: half-open cores choose one owner, duplicate one-record aprons make nearest sampling agree at equal levels, the quality mask prevents alpha overlap, and a mixed-level transition subdivides the coarser edge at coincident fine dyadic points without moving any shared projected point. If unequal height samples prevent coincidence, the better edge owns the apron strip or the strip is refused; no correctness argument depends on float linear filtering, blending, or moving a feature to hide a crack.

At a mixed-level boundary, the oracle compares direct projections of all shared dyadic edge points and checks that the two rasterized coverage polygons differ by no more than 1.0 px and leave no uncovered sample. If that proof fails, the lower-quality side is clipped back and the seam strip becomes an explicit hole to refine.

The exterior pass paints the palette's sky or exterior color wherever the requested slice is beyond the projective horizon; this is known absence of surface, not a cache miss. The transient clear color is used only for in-front-of-horizon screen fragments for which no valid, placeable tile owns coverage. Published facts report sky and clear coverage separately.

An all-edge-on request remains an all-sky state. It does not schedule invisible fractal tiles merely to make cache statistics look busy.

## Per-tile refinement and scheduling

The global Preview, Interactive, Final ladder becomes a `TileFamily` ladder for one requested owned chart footprint. Deterministic keeps quarter-resolution cap 64, half-resolution cap 256, and full-resolution cap up to 4096; PictureFast keeps one-eighth-resolution cap 32, omits Interactive, and goes to Final, matching [Julibrot kernels](kernels.md).

Draft resolution is represented explicitly. A Preview or Interactive tile may use a smaller heap page and upscale into the same 254×254 atlas core, or it may be a coarser pyramid tile linked to the family; it must not masquerade as a Final tile at the fine pyramid level. The descriptor publishes both sample extent and pyramid footprint.

The queue priority is: uncovered visible macroregions first, then visible incumbents whose next rung strictly improves quality, then visible seam repairs, with projected screen area and distance from the interaction focus as deterministic tie-breakers. A cached invisible tile may remain resident, but no Preview, Interactive, Final, recolor, or seam job is launched solely for an invisible tile.

The scheduler spends explicit per-frame walls on CPU selection, kernel dispatch count, record-copy bytes, atlas-colorization patches, mesh vertices, and draw batches. It does not express a GPU duration that the WebGL2 floor cannot measure; it publishes normalized work and fence wall time just as the current ledgers do.

One accepted reference orbit is shared by every deep tile job in its MAIN generation. The reference span is pinned while any submitted tile dispatch can read it, and every resulting descriptor records the generation, length, bits, policy, and center revision. Tiles do not copy the orbit.

Jobs whose tile center falls outside the accepted reference's perturbation validity region are not silently attempted. The controller either leaves the region covered by an older valid tile, marks it clear, or requests a new reference generation; under the version-one invalidation rule, accepting that generation retires the former active tile partition.

Publication is atomic per tile. A job writes SCRATCH, copies its initialized records to its planned DATA span, optionally colorizes a vacant atlas slot, and crosses a completion fence; only then does the catalog publish the new quality. Cancellation, stale generation, device refusal, or atlas failure frees unpublished resources and leaves the incumbent untouched.

The draft policy is enforced twice. The publication ledger rejects a candidate whose quality key is not strictly better than the resident entry for the same footprint, and the regional cover solver refuses to assign it over a better overlapping tile. Thus an out-of-order Preview fence cannot displace an Interactive or Final picture.

The first complete pass for a newly exposed region may be Preview, because a bounded draft is better than clear. Once a better tile is published, later navigation can still choose the old draft only for portions the better tile does not geometrically cover.

## Device-floor resource model

The design assumes only the floor in [minimum requirements](../minimum-requirements.md): WebGL2 or OpenGL ES 3.0, `EXT_color_buffer_float`, four color attachments, a 16 KiB uniform block, 16 vertex texture units, 256 texture-array layers, 2048 maximum texture dimension, core RGBA8 presentation, and unfilterable RGBA32F DATA access.

Version one keeps 16-byte escape records because that is the existing RGBA32F heap dialect. An 8-byte logical format does not save physical heap memory if it is merely padded back into RGBA32F; realizing the smaller column requires a separately proven RG32F or packed-record heap kind and corresponding shader, descriptor, and copy contracts.

One physical tile consumes one 256×256 RGBA32F DATA page, exactly 1 MiB. In the documented 512×512×16 DATA heap there are 64 such pages; the floor profile caps the cache at 56 tile pages and reserves eight pages for the current orbit, transient publication needs, and other lattice users. If configuration supplies fewer pages, planning lowers the delivered tile count before allocation.

The color cache is one 2048×2048 RGBA8 atlas, 16 MiB, divided into 64 256×256 slots. A slot includes the same one-texel apron as its record page, is addressed by slot plus generation, and is written only while vacant. One or more vacant slots serve as publication staging, so the practical colorized resident count never exceeds the record budget.

A 256-layer 256×256 texture array is a viable later alternative, but the first atlas uses one core-sized texture and one binding, leaves array layers for other systems, and stays at the guaranteed dimension. The page must publish whether atlas or array mode was delivered rather than infer it from adapter class.

The placement block is capped at 64 entries. Budgeting twelve `vec4` values, or 192 bytes, per entry costs 12,288 bytes; a 256-byte header keeps the binding at 12,544 bytes, below the 16 KiB floor. The exact layout must be a constructor-validated capacity, and relief data that cannot fit that record is moved to vertex/index buffers or split into another batch rather than growing the uniform silently.

The executor's existing descriptor buffer already consumes its own fixed capacity and the span directory consumes another fixed uniform binding. Tile placement must use a separate declared presenter block and must validate the combined binding count and per-stage texture count; it cannot assume unused bytes in executor uniforms.

Naively drawing one tile at a time would make an ordinary 1080p view cost 40 to 54 draw calls. The target is instanced or multi-tile batches: one opaque batch for flat atlas tiles, a small number of relief mesh classes, one exterior pass, and at most one explicit clear/mask pass. A delivered frame publishes actual batch and draw counts, because the heap study already warns that per-draw CPU cost is material on the intended architecture in [GPU heap lattice](../gpu-heap-lattice.md).

Nearest sampling is the correctness baseline for record and atlas textures. Aprons and ownership masks replace any reliance on unavailable float linear filtering, and opaque replacement replaces any reliance on float blending.

SCRATCH remains executor-owned RGBA32F storage. Tile jobs reduce the active rectangle and copy bytes, but they do not resize, alias, or bypass the executor's configured scratch layers; requested and delivered scratch, DATA, span, handle, header-set, and uniform walls remain public facts.

## Cost model and eviction

All byte figures below are binary MiB unless marked as bytes. They count record storage and the RGBA8 chart atlas; scratch, reference-orbit storage, depth, index and vertex buffers, uniforms, surface textures, and implementation metadata are stated separately rather than hidden in the total.

| Unit | 8-byte record column | 16-byte RGBA32F record column | RGBA8 atlas |
|---|---:|---:|---:|
| One 256×256 physical tile | 0.5 MiB | 1 MiB | 0.25 MiB |
| 56 resident record tiles | 28 MiB | 56 MiB | 14 MiB for 56 occupied slots |
| Full 64-slot allocation | 32 MiB | 64 MiB | 16 MiB |

The 8-byte column is a design comparison, not a claim about the current heap: today's RGBA32F DATA path charges 16 bytes per physical record. CPU or worker compaction alone does not change that GPU cost.

With 254 owned pixels per tile axis, a phase-friendly cover needs `ceil(W/254) × ceil(H/254)` tiles, while an arbitrary alignment can require one more tile on each axis. The table includes both because a cache budget based only on the friendly phase will fail during a pan.

| Canvas | Friendly / worst tile count | Occupied 8-byte records plus atlas share | Occupied 16-byte records plus atlas share | Physical samples, friendly / worst |
|---|---:|---:|---:|---:|
| 960×540 | 12 / 20 | 9 / 15 MiB | 15 / 25 MiB | 786,432 / 1,310,720 |
| 1920×1080 | 40 / 54 | 30 / 40.5 MiB | 50 / 67.5 MiB | 2,621,440 / 3,538,944 |

The viewport table charges each occupied atlas slot its 0.25 MiB share, which is useful for comparing tile geometries, but the proposed atlas is one fixed 16 MiB allocation. With that allocation present, 16-byte record reservations make a friendly and worst-phase 960×540 cover 28 and 36 MiB, and a friendly and worst-phase 1920×1080 cover 56 and 70 MiB; the corresponding realized 8-byte figures would be 22 and 26 MiB, then 36 and 43 MiB.

At cap 4096, one full physical tile has a worst-case 268,435,456 pixel-iterations. A friendly 960×540 full cover has about 3.22 billion worst-case iterations versus about 2.12 billion for today's exact screen grid; a friendly 1920×1080 cover has about 10.74 billion versus about 8.49 billion. Aprons and whole-tile rounding make a cold full render more expensive, so tiling is justified by reuse and partial work, not by claiming a cheaper cold frame.

Today's 960×540 design reserves 8 MiB for the 16-byte Final grid and allocates two 1.98 MiB RGBA8 scene textures, about 11.96 MiB combined before 16 MiB SCRATCH and other resources. At 1920×1080 it reserves 32 MiB for records and two 7.91 MiB scene textures, about 47.82 MiB combined. These values follow the resource arithmetic in [Julibrot kernels](kernels.md) and [the precision ledger](precision-ledger.md).

The proposed occupied tile sets are 15 MiB at 960×540 and 50 MiB at 1920×1080 with current 16-byte records, but allocation-level comparison must use the fixed-atlas figures of 28 and 56 MiB. The 960×540 floor profile is therefore materially larger than today's retained footprint, while the 1920×1080 friendly cover is moderately larger. A full 56-tile cache plus the complete atlas reserves 72 MiB; after a friendly 40-tile 1080p cover, 16 additional occupied record-and-atlas slots can hold 20 MiB of return history and the atlas still has 2 MiB of vacant or staging slots. The separate documented 16 MiB SCRATCH and at least one reference page still apply, and the shared heap texture's total allocation is outside both span-reservation comparisons.

The primary win is a zero-iteration return to a resident chart position, including after unrelated camera motion; continuous pan and zoom reuse the overlap and compute only newly exposed tiles; a small camera, observer, height, palette, or canvas change reprojects or recolors records instead of rerunning escape; and a cancelled or failed tile leaves other regions intact. Under strict version-one rules there is no reuse after slice tilt, out-of-plane origin motion, cap change, precision change, or accepted reference regeneration, and the page must not call those cases partial hits.

Eviction first removes logically invalid partitions, stale atlas generations, and failed drafts. It then considers only unpinned valid tiles: selected, in-flight, publication-source, seam-source, and active-orbit-dependent entries are pinned until their fences and frame ownership release them.

Among remaining tiles, eviction is LRU by `last_selected_serial` with chart distance from the current visible footprint as the secondary policy requested by the owner. Distance is compared in integer tile space at a common pyramid level, using saturating magnitude or shared-key-prefix order so a deep bignum coordinate is never narrowed to f64; larger pyramid mismatch is the next tie-breaker and stable tile identity is last.

The policy retains invisible tiles when budget permits but never computes them. It protects the current visible cover before history, protects Final over an equivalent draft, and keeps a small balance across recent pyramid levels so a one-notch zoom does not evict every useful parent or child.

When no safe victim exists, allocation fails closed: the scheduler lowers the number of concurrent tile jobs or leaves a clear hole and publishes the capacity wall. It never frees a span or atlas slot still reachable by a submitted draw.

## Oracle, browser proofs, and published facts

Pure key tests must prove that bignum anchor serialization and signed dyadic indices round-trip, adjacent aprons name identical chart sample points, parent and child pyramid points agree where their dyadic lattices coincide, extreme deep-zoom keys do not pass through f64, and arbitrary plane-preserving chart isometries preserve ambient sample points within the stated basis bound.

Compatibility table tests must pin every row in the invalidation matrix, including exact in-plane origin motion, representation-noise tolerance, first out-of-plane displacement beyond the proof bound, plane tilt, cap, precision, reference generation, camera-only changes, palette-only changes, and stale heap or atlas generations.

Selection tests must prove deterministic covers independent of catalog iteration order, no double owner for a macroregion, no uncovered region falsely called sky, no candidate above 1.0 px, strict no-draft-downgrade behavior, finer and coarser fallback, and stable age tie-breaking. Property tests should compare the chosen cover with a slow exhaustive quality oracle for small random catalogs.

Flat placement tests compare every tile homography at corners, edges, interior witnesses, multiple extents, deep zooms, and the shipped preset rows against direct five-dimensional projection at height zero. Uploaded coefficients must remain within the ceiling, and a horizon or f32 uncertainty must refuse rather than produce non-finite output.

Relief tests compare the tessellated placement against direct projection using actual tile record heights, the existing five-height regression values, adversarial height discontinuities, near-pole cameras, and maximum subdivision walls. They publish per-tile maximum and p95 error and require every selected tile to have a finite maximum at most 1.0 px.

Seam tests render equal-level and one- or two-level transitions at subpixel pans, fractional zooms, all four edges, mixed quality, and relief. They compare shared-edge screen positions and an ownership image, requiring no clear crack, no double ownership, no wrong-palette texel, and at most 1.0 px geometric disagreement.

Heap and publication tests inject stale handles, span exhaustion, directory exhaustion, atlas-slot reuse, cancellation before and after DATA copy, out-of-order fences, and eviction during queued work. The incumbent must remain selectable until a strictly better completed tile is atomically published.

The first browser proof navigates from view A to disjoint view B and back to A under the same MAIN generation, and demonstrates identical selected tile identities, zero new escape dispatches for the resident A cover, and no transient clear after the return frame. A control run with capacity below the A-plus-B working set must show honest evictions and misses.

The second browser proof pans by less than one tile and zooms fractionally, demonstrating reuse of the overlap, computation only for visible holes, correct mixed-level seams, and monotonically improving regional quality. It repeats at 960×540 and 1920×1080 on the device floor.

The third browser proof exercises every camera and observer slider, height zero and nonzero, both perspective distances, resize, and palette change without changing MAIN. It demonstrates record-cache retention, flat homography or bounded relief selection, refusal at a constructed pole, and separate sky versus clear accounting.

The fourth browser proof changes plane within its stabilizer, then tilts it, moves origin in-plane, then out-of-plane, changes cap and precision, and accepts a new reference. It demonstrates re-keying only for the plane-preserving cases and immediate logical invalidation for every new-content case.

The fifth browser proof runs automatic and manual scene modes through Preview, Interactive, and Final with delayed and reversed completion callbacks. It proves that visible work is prioritized, invisible work is absent, manual controls remain honest, and no draft ever replaces a better regional picture.

The page must publish the requested and delivered tile geometry; record and atlas format; record, atlas, scratch, orbit, descriptor, span, handle, header-set, uniform, vertex, index, and staging byte walls; resident, valid, invalid, pinned, visible, candidate, selected, queued, in-flight, completed, cancelled, hit, miss, recolor, and eviction counts; and atlas occupancy plus slot generations.

For the current frame it must publish MAIN and reference generation, slice-key digest, chart anchor digest, requested chart footprint, ideal and selected pyramid-level histograms, refinement-level histogram, covered, clear, and sky pixels or fractions, tiles per draw batch, total draw calls, placement-uniform bytes, mesh vertices and triangles, and normalized kernel/copy work.

For correctness it must publish the latest invalidation or refusal reason, per-tile or aggregate maximum and p95 warp error, unbounded and pole refusal counts, coordinate-uncertain, projection-uncertain, horizon, and glitch counts, seam witnesses and maximum disagreement, draft-downgrade rejection count, stale-generation rejection count, and whether every selected region has an oracle receipt.

For timing it must retain measured CPU and fence wall times and explicitly label normalized or estimated GPU work. The WebGL2 floor has no timestamp query, so the interface must not turn iteration counts, copy bytes, or fence latency into invented kernel milliseconds.

## Staged migration

Lane-hours are focused implementation-and-test hours for one engineer familiar with the lab, not calendar promises; ranges include Rust tests, browser proofs, documentation, and review fixes but exclude a new compact heap dialect and reference-independent perturbation certification.

| Stage | Working implementation at the end of the stage | Pinning tests and proofs | Estimate |
|---|---|---|---:|
| 0. Pure policy foundation | Current one-scene page unchanged; add canonical chart keys, affine-plane equivalence, quality ordering, invalidation decisions, cost arithmetic, and public fact types behind unused seams | Deep key round-trips, dyadic parent/child and apron identity, full invalidation matrix, exhaustive small-cover quality oracle, byte and capacity walls | 16–24 lane-hours |
| 1. N retained whole scenes | Generalize the retained ledger from one promoted scene to a bounded LRU of full-screen scenes catalogued by content key and chart footprint while retaining source pose and palette as presentation metadata; current renderer and whole-grid ladder remain intact, and N=1 reproduces today's behavior | N=1 compatibility, A→B→A cache hit, fence-safe texture reuse, palette/version handling, no draft downgrade, LRU under a forced wall | 20–32 lane-hours |
| 2. Fixed record tiles and flat composition | Kernels can sample canonical chart rectangles into one-page spans; a semantic catalog and 2048² atlas publish fixed tiles; height-zero presentation selects a regional cover and batches exact homographies, while the legacy whole scene remains a fallback switch | Tile-versus-direct flat image oracle, equal- and mixed-level seam ownership, partial-pan dispatch count, stale span and atlas generations, 960×540 and 1920×1080 floor walls | 48–72 lane-hours |
| 3. Relief-aware tile placement | Height-bearing tiles use adaptive meshes and per-tile 1.0 px admission; poles and unproved cells expose regional holes while flat and legacy fallbacks continue to work | Direct-projection mesh corpus, five-height regression, adversarial discontinuities, horizon and pole refusal, subdivision wall, seam continuity under camera motion | 36–56 lane-hours |
| 4. Per-tile refinement scheduler | Replace the one global ladder with visible-first TileFamily jobs sharing one pinned orbit per MAIN generation; atomic publication and the regional ledger enforce monotonic quality | Automatic and manual ladder ordering, delayed/reversed fences, cancellation, zero invisible dispatches, shared-orbit pinning, Preview never replacing Interactive or Final | 36–52 lane-hours |
| 5. Cache policy, facts, and hardening | Enable the 56-page LRU-plus-distance budget by default, remove the legacy fallback only after parity, publish the complete interface facts, and ship browser return/pan/camera/invalidation proofs | Long navigation/eviction soak, device loss and capacity refusals, both target extents, preset rows and every slider, A→B→A zero-dispatch proof, console and shader cleanliness | 24–40 lane-hours |

The estimated total is 180–276 lane-hours. Stage 2 is the largest seam because screen-aligned kernel output, full-screen scene targets, and a single `EscapeGrid` all change ownership together; splitting it further is acceptable if the legacy presenter remains the working fallback after each intermediate commit.

Stage 1 is intentionally useful even if later tile work pauses: it proves keying, return-to-position behavior, N-way ledger ownership, and eviction with minimal shader change. It does not claim pan continuity or reduce partial recomputation.

Stage 2 should initially support height zero and one refinement rung behind a runtime capability flag. Stage 3 removes that geometric limitation, Stage 4 adds performance policy, and Stage 5 makes the tile path the default only after the same page facts and browser pins are stronger than the legacy path.

## Backlog

- `JB-TILE-001` — Specify and test canonical geometric-slice, bignum-anchor, dyadic pyramid, and arbitrary-precision tile-key encodings, including the 254-core apron equations and plane-preserving chart transforms.
- `JB-TILE-002` — Implement the pure compatibility and invalidation matrix with content-valid, placeable, and preferred as distinct results, preserving the half-texel representation bound and refusing genuine off-plane motion.
- `JB-TILE-003` — Generalize `SceneLedger` to a bounded N-scene whole-frame cache with generation-safe texture ownership, monotonic quality, LRU facts, and the A→B→A browser proof.
- `JB-TILE-004` — Add the semantic tile catalog, chart spatial index, capacity planner, pin ledger, and fence-safe `DataSpan` lifetime without changing heap descriptor semantics.
- `JB-TILE-005` — Add canonical chart-rectangle shallow and perturbation dispatches with one shared, pinned reference orbit per active MAIN generation and per-tile precision and glitch receipts.
- `JB-TILE-006` — Add the 2048² RGBA8 atlas, 64 generation-tagged slots, palette recolor path, atomic tile publication, and stale-slot refusal.
- `JB-TILE-007` — Implement deterministic regional cover selection, 64-entry placement batches, exact flat homographies, half-open ownership, aprons, mixed-level transitions, sky, and clear masks.
- `JB-TILE-008` — Extend the reprojection oracle to adaptive relief meshes, actual record heights, seam witnesses, local refusal, and the hard 1.0 px ceiling.
- `JB-TILE-009` — Replace the global ladder with the visible-only TileFamily queue, per-frame work walls, no-draft-downgrade publication, and cancellation tests.
- `JB-TILE-010` — Publish tile, cache, resource, cover, quality, error, seam, invalidation, scheduling, eviction, and timing facts and add the five browser proof sequences.
- `JB-TILE-011` — Study an actual 8-byte GPU escape-record dialect; accept it only if the device-floor format, exact status and rebase representation, heap kind, shader access, and measured bandwidth benefit are all proved.
- `JB-TILE-012` — Study promotion of certified Final tiles from orbit-generation provenance to reference-independent mathematical content so a reference refresh does not discard a valid return cache.

## Risks

Reference churn is the largest product risk. The current controller requests a new reference after bounded center or zoom motion and frees the previous orbit; strict generation invalidation can turn an ordinary long pan into a whole-cache miss, so the first facts must expose invalidations by reason and the reference-independent certification study should follow soon after correctness lands.

Cold rendering does more work than the screen-aligned grid because full pages, aprons, and phase rounding add samples. At 960×540 the friendly 12-tile cover is about 52 percent more samples than the screen grid; cache-hit and partial-pan proofs must demonstrate that saved iterations repay this overhead in real navigation traces.

Arbitrary plane-preserving rotations do not map an axis-aligned dyadic tile to another axis-aligned tile. Treating them as transformed footprints preserves samples but can fragment the cover and expose edge wedges; pretending they are integer key rewrites would be wrong.

Relief can fold, cross a perspective pole, or vary too sharply inside a cell. Adaptive tessellation must have finite walls and regional refusal, which means a camera pose can retain valid records yet temporarily show clear holes; the interface must explain that as placement refusal rather than model invalidation.

Mixed-level seams are a correctness problem, not cosmetic filtering. Nearest sampling, duplicated aprons, half-open ownership, non-moving shared transition points, and an ownership oracle all need to agree under fractional zoom and camera tilt.

The 56-page record budget leaves little return history at a phase-worst 1080p cover of 54 tiles. A one-view-plus-history promise cannot be universal on that floor; requested versus delivered history capacity must be explicit, and the implementation must not begin invisible prefetch that evicts visible Final tiles.

One 2048² atlas is simple but palette recoloring can temporarily contend for its vacant slots. Record validity must survive atlas eviction, and a wrong-palette slot must never be used as a visually convenient stale fallback.

CPU cover solving, bignum spatial comparisons, mesh building, and per-tile descriptors can move cost from GPU iteration to the main thread. Fixed macroregions, stable integer keys, bounded candidate counts, cached placement receipts, and batched draws require measured browser walls before the legacy path is removed.

The present heap lattice has fixed uniform and directory capacities and a non-compacting buddy allocator. Fragmentation, span/header exhaustion, or a live fence can prevent publication even when byte totals look sufficient; every allocation path needs a typed wall and an incumbent-preserving failure mode.

Stage 1 full-screen scenes and later chart tiles have different semantics. The migration must not fossilize screen pose or palette in the final `ContentKey`; the multi-scene cache is a proof scaffold whose key and facts should already separate content from presentation.

## Open questions for the owner

1. Should accepting a new reference orbit invalidate all cached tiles indefinitely, or may certified Final records outlive their computational reference? Recommendation: ship strict generation equality first because it matches current ownership and the requested invalidation rule, then prioritize `JB-TILE-012`; reference-independent promotion is likely necessary for satisfying long-pan return behavior without weakening correctness.

2. Is the proposed 256×256 physical tile with a 254×254 owned core and one-sample apron the desired first fixed geometry? Recommendation: yes; it fits one existing 256 page, gives exact neighbor samples, and keeps ordinary 1080p selection within the 64-entry placement block, while facts can reveal whether a later 128 or 512 class is warranted.

3. Should version one pursue an 8-byte record? Recommendation: no; use the existing 16-byte RGBA32F record and treat 8 bytes as a measured follow-up, because padding an 8-byte logical record into the present heap saves nothing and packing status plus rebase must remain exact.

4. What floor cache promise should the interface make? Recommendation: promise a 56-page record budget and a 64-slot atlas, not “two complete views”; pin the current visible cover and describe remaining pages as opportunistic history because a phase-worst 1080p view alone may need 54 tiles.

5. How broadly should “plane-preserving re-key” apply? Recommendation: accept every certified affine-plane stabilizer motion as semantically valid, use direct integer aliases only for dyadic lattice automorphisms, and spatially index transformed quadrilateral footprints for general in-plane rotations or translations rather than resampling records.

6. Should the color atlas retain more than one palette? Recommendation: no for the first version; keep records authoritative, dedicate the atlas to the current palette version, recolor visible tiles first, and revisit multi-palette residency only if measured palette toggling justifies stealing history slots.

7. Should relief use a fixed mesh or adaptive subdivision? Recommendation: begin at 16×16 cells per tile and adapt under the existing 1.0 px oracle with hard vertex and depth walls; a fixed mesh alone cannot honestly cover near-pole and discontinuous-height cases.

8. Should the cache persist across page reloads? Recommendation: keep it session-local. Persistent records need a stable formula ABI, canonical bignum serialization, corruption checks, quota policy, and privacy/product decisions that are not needed to prove in-memory return-to-position value.

9. May a lower-cap tile be reused after a cap reduction or increase? Recommendation: keep cap equality in the first `ContentKey`; add a partial order only after the record stores enough first-escape evidence to prove equivalence for the requested cap.

10. When no tile meets the 1.0 px relief ceiling, should the presenter fall back to a stale full-screen scene? Recommendation: only if that scene independently passes the same content and placement oracle for the affected region; otherwise render sky where geometrically exterior and transient clear where content is missing.

## Repository sources

- [Repository charter](../../CLAUDE.md) supplies layering, renderer, reporting, Markdown, and device-floor ownership constraints.
- [Minimum requirements](../minimum-requirements.md) supplies the WebGL2, `EXT_color_buffer_float`, texture, attachment, uniform, and unavailable-feature floor.
- [GPU resource heap](../gpu-resource-heap.md) supplies physical descriptor, handle, buddy-allocation, texture-kind, and lifetime contracts.
- [GPU heap lattice](../gpu-heap-lattice.md) supplies `DataSpan`, directory, executor, scratch-copy, fixed-capacity, and measured draw-overhead contracts.
- [Julibrot lab charter](../julibrot-lab.md) supplies scope, component layering, and the two-dimensional-slice/five-dimensional-view model.
- [Julibrot math](math.md) supplies controls, screen mapping, deep precision, reference displacement, warp compatibility, record layouts, and the refinement policy.
- [Julibrot kernels](kernels.md) supplies the screen-aligned grid, page arithmetic, record bytes, scratch-copy behavior, status values, and normalized work facts.
- [Julibrot worker](worker.md) supplies the bignum reference orbit, generation, replacement, precision, transfer ABI, and credit ownership contracts.
- [Julibrot presentation](present.md) supplies the retained/pending scene ledger, two RGBA8 targets, homography and relief warp, horizon behavior, exposure, and 1.0 px ceiling.
- [Julibrot app](app.md) supplies HOT/MAIN publication, automatic and manual scene modes, latest-wins scheduling, page facts, and browser proof conventions.
- [Julibrot precision ledger](precision-ledger.md) supplies coordinate, smooth-escape, projection, memory, copy, and timing budgets.
- `origin/docs/4d-arena:docs/4d-first-engine.md` supplies the oriented view frame and the stabilizer-of-the-slice after-time-warp limit.
- `origin/docs/4d-arena:docs/4d-torus-world.md` supplies the phase, bounded-region, and reslice distinction in a compact higher-dimensional world.
- `origin/docs/4d-arena:docs/4d-content.md` supplies stamped normal-frame and offset identity and the boundary between within-plane motion and new higher-dimensional content.
