# Tiled reprojection for the Julibrot lab

Status: draft design study.

## Decision summary

The cache should live in the two-dimensional chart of one affine Julibrot slice, not in screen space: its durable payload is a palette-independent escape-record tile addressed by an exact bignum anchor, a dyadic zoom-pyramid level, and signed integer tile coordinates.

Screen images are derived data and never tile-cache payload. The cache holds only the value map: Julibrot escape records here and, by the same arena contract, linear HDR values where an arena layer supplies them. The reprojection layer performs the per-pixel palette lookup, exposure, tone mapping, and output encoding at placement; none of those display inputs enters any cache key.

The first implementation should use one 256×256 heap page per tile, with a 254×254 core sample grid spanning 253 dyadic intervals and one record of apron on every edge, 16-byte RGBA32F escape records, and at most 56 resident record tiles in the example 64-page DATA heap. Twelve tile pages are a protected backdrop tier, 44 are detail and history, and eight DATA pages outside the tile budget remain available for the active reference orbit, publication, and non-cache allocations; there is no palette-color atlas.

Every request is decided in three separate steps: semantic validity asks whether the records describe the requested mathematical slice and MAIN generation; placement admission asks whether the requested camera projection is finite and stays within the 1.0 px error ceiling; quality selection asks whether a candidate is better than the picture already covering that screen region.

The hot selector should inverse-map the screen bounds into the canonical slice chart and walk intersecting pyramid nodes directly. The walk cost is proportional to visited visible and backdrop nodes and independent of total cache population; it emits a disjoint fine-over-coarse cover without a general spatial search.

One GPU render pass remains the goal, but quality must not replace true lifted depth. The CPU walk resolves duplicate zoom, refinement, and generation representations and supplies parent-exclusion masks; the depth attachment then carries only projected relief depth, which correctly resolves distinct surface points that overlap on screen.

A protected 3×3 backdrop at a coarse pyramid level covers at least three times the current conservative chart bounds on both axes. It remains below every admitted detail tile, so rotations, zoom-out, and pitch ordinarily reveal a coarse valid picture rather than clear without allowing the backdrop to displace a better tile.

A cache hit may eliminate fractal iteration, but it does not weaken presentation correctness. Flat tiles use their exact chart-to-screen homography; relief tiles use a height-aware mesh whose measured interpolation error is at most 1.0 px; a pole, missing proof, or unbounded error refuses that placement and exposes only the affected region for refinement.

The migration should preserve the current page after every stage: first retain several whole-screen value maps by chart footprint and shade them at reprojection, then introduce chart keys and a catalog, then split computation and presentation into fixed tiles with a coarse backdrop, then add bounded relief placement, and only then replace the single global ladder with visible-Detail plus bounded-backdrop scheduling.

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

For each placed fragment, the reprojection layer evaluates a current value-to-linear-color map `L(r; palette)`, applies exposure and tone mapping, and encodes the result for the presentation target. These operations are frame shading, not computation or cache mutation; the arena analogue begins with an already linear HDR value and applies the same exposure, tone-mapping, and output stage.

For a fixed target camera pose `P`, neutral height has a projective map `s ~ M_P [a b 1]^T`. A rectangular chart tile is therefore placed exactly by a homography when `z = 0`; no source-screen pose is needed. When height is nonzero, vertices use the full five-dimensional projection `s = Pi_P(Y)`, and the rasterized triangle interpolation is an approximation whose screen-space error must be measured.

The current retained-scene warp composes a source screen map with a target screen map and validates the composition over a 9×9×5 screen-and-height corpus. The tile oracle keeps its 1.0 px ceiling but changes the object being certified: it measures each tile's chart homography or relief mesh against direct projection at the target pose, then measures agreement along selected tile boundaries.

The word “level” is overloaded and must be split in all APIs and facts. `pyramid_level` is the dyadic chart sampling scale; `refinement_level` is Preview, Interactive, or Final; `iteration_cap` is the delivered escape cap; and `quality` is the complete ordered record used by selection.

The word “valid” means that a record still represents the requested mathematical content. “Placeable” means that a valid record can be projected at the requested pose under the finite 1.0 px contract. “Selected” means that a placeable record wins the regional quality comparison. A valid tile can therefore be temporarily unplaceable without being evicted or recomputed.

The word “MAIN generation” means the coherently published content token, not a frame counter: it binds the canonical slice, origin, precision policy, requested cap, reference-orbit generation, and record ABI. HOT camera and navigation state is deliberately excluded.

One canonical chart frame is interned with each geometric slice partition and remains fixed across plane-preserving object and in-plane-origin controls. Those controls change the request-to-canonical isometry, not the stored tile rectangles; this invariant is what makes direct pyramid traversal possible.

## Where tiles live

Today's retained scene is a screen-sized RGBA8 image sampled on the inverse-homography screen grid. Its coordinates, extent, camera pose, and palette are baked into the image, so the two-slot ledger can reproject only the most recently promoted view and loses an older position when that slot is reused. [Julibrot presentation](present.md) describes that two-texture ownership model, and [Julibrot kernels](kernels.md) describes the one Final-capacity screen-aligned `EscapeGrid` behind it.

A reusable tile instead owns a rectangle in chart coordinates and carries the escape records for fixed chart sample points. Returning to a chart rectangle with the same content token finds the old records even if the canvas, camera, observer, palette, or intervening view differed.

The pyramid uses an immutable bignum chart anchor `A` on the slice and dyadic texel spacing `sigma_l = 2^-l` chart units at signed integer `pyramid_level l`. A key contains `anchor_id`, `l`, and arbitrary-precision signed tile coordinates `(i,j)`; it never contains an f64 absolute coordinate.

For a 256×256 physical tile, indices `r,s` range from 0 through 255, indices 1 through 254 form the core sample grid, and the chart sample is `A + sigma_l ((253 i + r - 1) u + (253 j + s - 1) v)`. The tile owns the half-open chart rectangle `[253 i sigma_l, 253 (i+1) sigma_l) × [253 j sigma_l, 253 (j+1) sigma_l)`; the high core row and column are shared boundary samples, while indices 0 and 255 are the one-sample exterior aprons.

The 253-interval half-open core gives every chart cell one owner, adjacent cores share an identical boundary sample, and the apron gives nearest sampling and relief triangulation one sample beyond each boundary. Two child rectangles at spacing `sigma_l/2` exactly partition a parent axis, and every parent sample coincides with an even-index child sample; the complete record grid still maps exactly to the heap's 256×256 page class and avoids buddy-allocation expansion.

At a canonical flat view, the current pixel scale is `4 / (2^q W)` for zoom `q` and grid width `W`; the matching pyramid level is therefore near `q + log2(W/4)`. A tilted or perspective view chooses levels from the local chart-to-screen Jacobian rather than from zoom alone, so different screen regions may select different pyramid levels without changing their mathematical position.

Deep-zoom identity is exact because the anchor and signed indices are serialized from bignum and integer components and the offset is dyadic. Hashing may accelerate lookup, but equality must compare the canonical serialized value after the hash matches; a finite mirror is never an identity key.

Precision is decided per tile. The tile computation must publish a conservative coordinate-error bound `epsilon_coord` satisfying `epsilon_coord <= sigma_l / 4` at every owned sample after bignum center arithmetic, plane-basis multiplication, reference displacement, and the cap-dependent perturbation allowance; its descriptor records requested bits, delivered bits, and the proven bound. The existing precision policy and quarter-pixel discipline provide the starting contract in [Julibrot math](math.md) and [the precision ledger](precision-ledger.md).

The durable value-map record retains smooth escape, escaped classification, rebase count, and status, as today's 16-byte RGBA32F escape record does. Height and linear palette input are derived from those facts, so a tile does not become invalid when height scale, palette, exposure, tone mapping, or output encoding changes.

No durable or rebuildable per-tile colorization is retained. If composition requires an intermediate color attachment, it is a frame-scoped reprojection output that is cleared and released logically with that frame, never a keyed catalog entry or an eviction candidate.

Backdrop tiles use the same keys and record ABI as detail tiles but carry a `Backdrop` residency class. For current conservative chart bounds `D_x × D_y`, choose a dyadic backdrop tile side `S_b = 253 sigma_b` such that `max(D_x,D_y) <= S_b < 2 max(D_x,D_y)`, and pin the 3×3 neighborhood whose center tile contains the view center. Its union spans at least three times each current chart-axis extent, and its pitch provides roughly 127–253 sample intervals across the longest view dimension.

That is the requested geometric and memory envelope, not permission to publish uncertified perturbation. Every backdrop tile must pass the same coordinate and glitch rules as Detail; if the one active reference orbit cannot certify the whole 3×3 region, the delivered backdrop extent is smaller and public facts identify the missing cells. A universal 3× no-clear promise at deep zoom therefore depends on certified records surviving reference replacement or on another proved multi-reference policy.

| Property | Current screen scene | Proposed chart tile |
|---|---|---|
| Identity | Screen extent, screen map, pose, palette, scene generation | Canonical slice and MAIN generation, bignum anchor, pyramid level, integer chart coordinates, record ABI; no display controls |
| Durable payload | RGBA8 pixels plus one globally reused record grid | Value-map escape records per tile; never palette colors |
| Camera change | Warp the last screen image if admitted | Reproject the same chart records; no content invalidation |
| Return to an older position | Miss after the retained slot is replaced | Hit while the keyed tile remains resident |
| Deep zoom | Position is relative to the current reference and screen width | Exact bignum anchor plus dyadic integer offsets; precision receipt per tile |
| Height lift | Approximation inferred from the old scene's record field | Records remain resident and directly supply mesh height |
| Palette, exposure, or tone change | Re-render the full scene image | Shade the next placement from unchanged records; no invalidation or cache work |

## Tile descriptor and heap relationship

The CPU catalog owns semantic descriptors. Heap descriptors remain physical GPU addresses and must not be overloaded with bignum keys or cache policy.

Each semantic tile descriptor contains a stable `tile_id` and a `content_key` comprising the formula and record-ABI version, canonical geometric slice key, MAIN generation, precision mode, requested and delivered cap, and reference-orbit generation. Palette, exposure, tone mapping, output encoding, target format, camera, and canvas are explicitly absent from every key.

The descriptor retains slice provenance sufficient to audit canonicalization: the six requested object angles or their exact control bytes, the delivered plane basis, the bignum plane origin and its finite mirror, the canonical normal-space identity, and the chart-frame transform. Keeping both provenance and a canonical key prevents a plane-preserving object reparameterization from masquerading as a new slice.

The chart portion contains `anchor_id`, canonical anchor serialization or an interned handle to it, signed `pyramid_level`, arbitrary-precision `(i,j)`, physical extent 256×256, core sample extent 254×254, owned span 253×253 intervals, the exact half-open chart rectangle, texel spacing, and orientation from stored chart coordinates to the canonical slice chart.

The quality portion contains `residency_class` (`Detail` or `Backdrop`), `refinement_level`, delivered sample extent, delivered iteration cap, coordinate-error bound, record completeness, and a monotonic quality key. Pyramid level, refinement level, and fallback role are separate fields; every admitted Detail entry sorts above every Backdrop entry for the same chart point.

The orbit provenance contains reference generation, reference-center revision, orbit registry identity while live, orbit length, delivered precision bits, and perturbation policy. The registry handle is operational and may expire; the immutable generation and precision receipt remain part of the tile descriptor.

Intrinsic error facts contain sampled, glitch, and coordinate-uncertain counts, maximum coordinate error in tile-texel units, and whether every Final record was certified. Horizon and projection uncertainty are camera-dependent, so the descriptor stores them in a latest-placement receipt keyed by target pose rather than treating them as permanent content facts.

Lifetime facts contain creation serial, last-selected serial, last-visible serial, hit count, recompute count, current pin reasons, publication fence state, eviction class, backdrop-window generation, and whether the entry is one of the nine active or three rolling backdrop slots. Age is a monotonic serial rather than wall-clock time so ordering remains deterministic in tests.

Storage facts contain the generation-checked `DataSpan`, logical and reserved record bytes, initialized prefix or full-tile state, and any temporary scratch or publication ticket. A catalog entry is not selectable until its DATA copy has crossed its completion fence; frame color attachments and shading revisions are not tile-descriptor state.

Placement receipts contain target pose digest, chosen warp kind, screen coverage polygon, selected pyramid mismatch, maximum and p95 projection error, horizon-clipped samples, projection-uncertain samples, seam checks, and refusal reason. These receipts can be replaced freely because they do not define content identity.

[GPU resource heap](../gpu-resource-heap.md) already supplies generation-checked logical handles, stable DATA and IMAGE heap bindings, typed allocation walls, regional content writes, and square buddy allocations. [GPU heap lattice](../gpu-heap-lattice.md) adds multi-page `DataSpan` ownership, a uniform span directory, transactional paired allocation, fixed input/output arity, fixed kernel-uniform capacity, SCRATCH-to-DATA publication, and generation-safe free.

The 256×256 tile choice deliberately fits one DATA page, so most tile spans have one page and one directory entry. Larger reference orbits may still use the existing multi-page span machinery, and the presenter can read every selected tile through the same stable heap view.

The heap does not supply the semantic pyramid-node catalog, canonical bignum key, slice-equivalence proof, direct hierarchy walker, quality order, parent-exclusion masks, Detail/backdrop scheduler, LRU and distance policy, multi-tile publication transaction, seam ownership, per-pose oracle, or value-to-pixel shading policy. Those belong above `heap`, split between pure Julibrot math/policy and the app/present owners according to the existing layering.

The executor also does not promise arbitrary tile counts. Its descriptor, span-directory, handle, input, output, header-set, scratch, and kernel-uniform capacities are constructor contracts; tile planning must reserve them explicitly, publish requested versus delivered counts, and degrade the cache rather than indexing beyond a wall.

## Validity, compatibility, and invalidation

The active `ContentKey` is equal only when the canonical geometric slice, plane origin, formula and record ABI, precision mode, requested cap, delivered-cap semantics, and MAIN/reference generation agree. “The model has not changed” means exactly that equality; visual similarity and nearby slider values are not substitutes.

Content invalidation is immediate logical unavailability, not necessarily immediate GPU destruction. When MAIN changes incompatibly, all old descriptors leave the selectable set before the next frame; their spans may be reclaimed later after fences, but they cannot fill the new picture.

Camera `Q`, five-dimensional camera translation, yaw, pitch, `d5`, `d4`, canvas extent, and height scale do not change escape records. They leave a tile content-valid; the tile is placeable only if target projection is finite and its flat or relief placement proof is at most 1.0 px. A large or pole-crossing camera move may therefore refuse every old placement while retaining every record for a later return.

Chart pan and zoom do not change content. They alter the requested chart footprint and preferred pyramid level, selecting existing tiles where available and scheduling only uncovered visible tiles. Resizing the canvas behaves the same way.

A palette, exposure, tone-mapping, or output-encoding change updates only reprojection-layer frame inputs. The next placement draw shades every covered pixel from the same value-map records with no tile invalidation, recolor job, publication fence, asynchronous gap, or clear; this instant response is part of the interface contract.

A plane-preserving object motion is valid as a chart re-key. The compatibility proof first shows equality of the two affine planes: their two basis spans agree within the delivered basis-rounding bound and the origin's normal residual is zero within the same bound. It then derives the two-dimensional isometry `c_new = R c_old + delta` from basis dot products and the in-plane origin displacement.

The interned canonical chart absorbs `R` and `delta`: the request polygon is transformed into canonical coordinates and existing tile keys remain unchanged. Exact dyadic automorphisms may use integer aliases as an optimization, but arbitrary in-plane rotations and translations still walk the same axis-aligned canonical pyramid without transforming or resampling stored escape records.

The present implementation compares object-angle samples at the f32 plane floor and permits at most one-half source pixel of origin residual before clearing. Tile compatibility should preserve that fail-closed tolerance while finite mirrors exist, but the long-term key should use bignum origin arithmetic and a certified normal residual measured against the finest source texel, not a screen pixel.

A slice-tilting object motion changes the plane span and invalidates all active tiles. An out-of-plane origin motion changes the affine plane and invalidates all active tiles; an arbitrarily small nonzero mathematical displacement is new content, with only representation-rounding uncertainty admitted by the proof.

Changing requested iteration cap invalidates all active tiles in version one. The existing four-lane record does not retain the first escape iteration in a form that proves what classification a different lower cap would have produced, and increasing the cap plainly needs new work. A future richer record may define a safe cap partial order, but visual inference is forbidden.

Changing precision mode invalidates all active tiles because Deterministic and PictureFast select different refinement and arithmetic promises. Merely delivering more bits within the same policy may produce a strictly better quality entry, but it must carry a new descriptor and pass the regional replacement rule.

Accepting a new reference orbit or shallow MAIN generation invalidates all active tiles in the conservative first version. Today's whole-scene ledger rebases a retained pose across an accepted reference shift, but the app simultaneously replaces and frees the prior orbit span; generation-keyed tiles cannot claim equivalent perturbation provenance without a separate certification. This restriction is intentionally visible as a risk and owner question because frequent reference regeneration can erase the principal pan/return benefit.

Reference generation is a computation provenance boundary, not inherently a change to the fractal formula. A later stage may promote fully certified records to a reference-independent content generation, but only after a proof shows that their values and uncertainty bounds are independent of the discarded orbit; that optimization is not assumed here.

| Requested change | Record validity | Catalog action | Placement action |
|---|---|---|---|
| Camera rotation or translation, yaw, pitch, `d5`, `d4`, height scale | Keep | None | Recompute target placement; accept only finite error at most 1.0 px |
| Pan, zoom, resize | Keep | Walk the canonical pyramid and roll the backdrop window | Prefer Detail, fall back to valid backdrop, and expose only regions outside delivered coverage |
| Palette, exposure, tone mapping, output encoding | Keep | None | Shade every placed fragment with the current frame inputs |
| Plane-preserving `O` change or in-plane origin move | Keep after affine-plane proof | Transform the request into the fixed canonical chart; tile keys remain aligned | Recompute placement from the request-to-canonical isometry |
| Slice tilt | Invalidate all active tiles | Start new content partition | Clear or sky until new content arrives |
| Out-of-plane origin move | Invalidate all active tiles | Start new content partition | Clear or sky until new content arrives |
| Iteration cap | Invalidate all active tiles in version one | Start new MAIN partition | Do not reuse visually similar records |
| Precision mode | Invalidate all active tiles | Start new MAIN partition | Do not mix policies |
| Reference-orbit or shallow MAIN generation | Invalidate all active tiles in version one | Start new generation partition | Do not select old provenance |

## Selection and placement

Selection begins by deriving the target's visible chart region. For the flat surface, inverse-map the screen corners and any horizon intersections into a chart polygon. For relief, conservatively bound projected tile meshes and use the current height range; a bound that crosses a pole is not evidence of coverage.

The ordinary selector does not search resident tiles or partition the screen into macroregions. It arithmetically enumerates the coarse-level integer nodes intersecting the canonical request polygon, normally at the desired backdrop level, looks up each node by its exact pyramid key, and descends only intersecting child quadrants toward the locally ideal level. A nonresident parent does not stop descent, so cached Detail outside the active backdrop window remains discoverable; each valid, placeable node remembers the best ancestor for its footprint, and a missing or refused descendant emits that ancestor or an explicit hole.

Each pyramid key has a small quality-sorted bucket for the active content key, so zoom-level, refinement, publication-generation, and any future certified orbit-generation alternatives are direct node-local lookups. Strict version one rejects old reference generations before traversal rather than emitting them as overlapping candidates. Each intersecting node is visited at most once and the coarser levels form a geometric series beneath the visible leaf count, making selection `O(N_walk)` in requested visible and fallback nodes, independent of the total cache population; the page publishes `N_walk`, key lookups, empty nodes, rejected entries, emitted tiles, and maximum descent.

The flat inverse polygon is exact for height zero. Relief may move silhouettes, create screen overlap, or approach a pole, so traversal inflates the canonical bounds by the descriptor's certified height envelope and validates emitted mesh bounds against direct projection. If no finite conservative inverse bound exists, selection starts from all active backdrop roots whose forward-projected bounds intersect the screen and remains bounded by the 12-page backdrop roster; it does not pretend the neutral plane inverse sees every lifted point.

| Fast-solver element | Judgment | Required form |
|---|---|---|
| Inverse-map the screen rectangle onto the slice | Adopt | Map into the fixed canonical chart; clip horizons and conservatively inflate for relief |
| Walk the pyramid with cost proportional to tiles in view | Adopt and replace the macroregion spatial search | Direct integer-key descent from backdrop roots with node-local quality buckets and ancestor fallback |
| Write `(level,bound,age)` as depth for flat overlaps | Optional, not the baseline | The hierarchy already removes representation overlap; benchmark only after identical output is proved |
| Combine quality depth with true lifted depth under relief | Reject as one scalar ordering | Quality resolves duplicate representations, while physical depth resolves distinct surface points; these priorities cannot share one general depth value |
| Resolve all selected tiles in one GPU render pass | Adopt | CPU emits a disjoint chart cover and child-exclusion masks; GPU depth stores only true lifted depth |

The incompatibility is mathematical, not merely a Depth24Plus precision concern. If a scalar depth `D(q,z)` gives quality `q` enough separation to make the better representation win for every admissible depth error at the same chart point, that separation can reverse two distinct lifted surfaces whose true depths are arbitrarily close; if the quality separation is zero, it cannot resolve the duplicate representations. The hierarchy must establish representation ownership before rasterization, leaving projected `z` as the sole depth order.

Each candidate receives a placement receipt for the target pose before ranking. Candidates with an incompatible content key, stale span generation, incomplete publication fence, projective pole, unbounded relief, or maximum error above 1.0 px are removed rather than penalized. Palette, exposure, tone mapping, and output format do not participate in candidate admission.

The quality order is lexicographic and total: admitted Detail beats Backdrop; within Detail, certified Final beats Interactive beats Preview; within a rung, a delivered cap satisfying the request beats a lower cap; then smaller projected texel error and closer pyramid match win; then lower coordinate, glitch, projection, and warp error win; then newer last-selection age breaks an otherwise exact tie. Backdrop entries use the same internal ordering but never cross the Detail boundary, and tile identity is the final deterministic tie-breaker.

Age can never make a draft beat a better refinement or make an uncertain tile beat a certified one. It exists only to choose between equivalent pictures and to stabilize cache behavior.

The pyramid walk emits non-overlapping chart ownership: a selected child excludes its quadrant from every emitted ancestor, while the nearest selected ancestor remains beneath missing descendants. Per-parent child masks are built during descent and passed with the placement entry, so a backdrop can cover the screen and still discard every fragment owned by a fine tile without a second selection search or a quality depth value.

The floor profile uses a 16×16, 256-bit exclusion mask per emitted ancestor, representing four descendant levels in eight `u32` words; the flat 960×540 and 1920×1080 backdrop-to-ideal gaps are ordinarily two and three levels. If a requested gap or irregular cover cannot be represented exactly, traversal emits an intermediate ancestor or coarsens the selected Detail cover and publishes that wall; it never rounds an exclusion outward and creates a false hole.

The composer groups selected entries by value-map binding, pipeline, and mesh class in one render pass. Flat tiles emit the chart rectangle through their target homography, and relief tiles emit the adaptive indexed mesh from chart positions and record-derived heights. For every surviving output fragment, the GPU looks up the escape record, evaluates the current palette to linear color, applies exposure and tone mapping, and encodes the presentation pixel; chart ownership and child-exclusion masks clip duplicate representations, while the depth attachment receives true projected lifted depth and resolves only overlaps from distinct chart points, including relief folds. In optional arena HDR accumulation mode the fragment instead writes the linear frame transient and the reprojection layer owns its final exposure, tone-map, and output resolve.

For a flat tile, the homography is evaluated from the tile's stored chart frame directly into the target screen. Its proof checks coefficient rounding and representative points against direct five-dimensional projection at height zero; the map is exact in real arithmetic, and uploaded f32 coefficients still obey the 1.0 px ceiling.

For a relief tile, start with a fixed 16×16 cell mesh over the owned core and evaluate direct projection at vertices plus deterministic interior and edge witnesses using actual record height bounds. Subdivide a cell while its interpolation bound exceeds 1.0 px, up to a configured vertex and depth wall; if the wall is reached without proof, refuse only that cell and expose it for a newly rendered exact patch rather than displaying an unbounded warp.

The existing 9×9×5 corpus remains a regression corpus, but actual tile records tighten the height range and local witnesses. A proof may never conclude that skipped samples beyond a horizon establish a bound; those samples are classified as exterior or refused according to the same horizon semantics as [Julibrot presentation](present.md).

Seams use four independent rules: half-open cores choose one owner, duplicate one-record aprons make nearest sampling agree at equal levels, the hierarchy mask prevents alpha overlap, and a mixed-level transition subdivides the coarser edge at coincident fine dyadic points without moving any shared projected point. If unequal height samples prevent coincidence, the better edge owns the apron strip or the strip is refused; no correctness argument depends on float linear filtering, blending, or moving a feature to hide a crack.

At a mixed-level boundary, the oracle compares direct projections of all shared dyadic edge points and checks that the two rasterized coverage polygons differ by no more than 1.0 px and leave no uncovered sample. If that proof fails, the lower-quality side is clipped back and the seam strip becomes an explicit hole to refine.

The exterior pass shades the current reprojection style's sky or exterior value wherever the requested slice is beyond the projective horizon; this is known absence of surface, not a cache miss. Once a backdrop window is established, ordinary bounded rotations, zoom-out, pitch, and detail eviction fall back to its coarse fragments rather than clear. The transient clear color remains only where the active slice or MAIN has no valid, placeable record, most notably immediately after an invalidation or at an oracle refusal; published facts report backdrop, sky, and clear coverage separately.

An all-edge-on request remains an all-sky state. It does not schedule invisible fractal tiles merely to make cache statistics look busy.

## Per-tile refinement and scheduling

The global Preview, Interactive, Final ladder becomes a `TileFamily` ladder for one requested owned chart footprint. Deterministic keeps quarter-resolution cap 64, half-resolution cap 256, and full-resolution cap up to 4096; PictureFast keeps one-eighth-resolution cap 32, omits Interactive, and goes to Final, matching [Julibrot kernels](kernels.md).

Draft resolution is represented explicitly. A Preview or Interactive tile may use a smaller heap page and upscale into the same 254×254 core sample footprint, or it may be a coarser pyramid tile linked to the family; it must not masquerade as a Final tile at the fine pyramid level. The descriptor publishes both sample extent and pyramid footprint.

The queue priority is: a missing central or visible backdrop after MAIN invalidation; visible clear or backdrop-covered regions at their cheapest Detail rung; missing cells in the protected 3×3 backdrop window; visible Detail incumbents whose next rung strictly improves quality; then visible seam repairs. Projected screen area and distance from the interaction focus are deterministic tie-breakers.

Invisible Detail tiles are still never computed. Backdrop maintenance is the sole deliberate exception: after the visible region has a bounded picture, the scheduler may compute coarse off-screen cells inside the protected window so a subsequent rotation, pitch, pan, or zoom-out has a valid fallback. Backdrop work has its own per-frame dispatch and copy wall and yields immediately to a new visible hole; display-only changes create no tile work anywhere.

The nine active backdrop tiles use the current policy's cheapest rung, cap 64 for Deterministic or cap 32 for PictureFast, and remain presentation class `Backdrop` even if their records are complete at that rung. When one becomes visible, ordinary Detail work is scheduled over it; improving or refreshing a backdrop can never publish over an admitted Detail incumbent.

The backdrop window rolls only after replacement cells are complete. Three additional reserved pages permit a row or column to be prepared while the prior nine remain selectable; a level change that needs more than three simultaneous replacements proceeds in bounded strips and retains the older coarser cover until the new window is complete.

The scheduler spends explicit per-frame walls on CPU selection, kernel dispatch count, record-copy bytes, mesh vertices, shaded fragments, optional transient-target bytes, and draw batches. It does not express a GPU duration that the WebGL2 floor cannot measure; it publishes normalized work and fence wall time just as the current ledgers do.

One accepted reference orbit is shared by every deep tile job in its MAIN generation. The reference span is pinned while any submitted tile dispatch can read it, and every resulting descriptor records the generation, length, bits, policy, and center revision. Tiles do not copy the orbit.

Jobs whose tile center falls outside the accepted reference's perturbation validity region are not silently attempted. The controller leaves the region covered by a cached Detail or certified backdrop tile where possible, publishes a missing-backdrop cell otherwise, or requests a new reference generation; under the version-one invalidation rule, accepting that generation retires the former active tile partition and temporarily returns to clear until its new central backdrop arrives.

Publication is atomic per tile. A job writes SCRATCH, copies its initialized value-map records to its planned DATA span, and crosses a completion fence; only then does the catalog publish the new quality. Cancellation, stale generation, or device refusal frees unpublished resources and leaves the incumbent untouched. Reprojection shading has frame lifetime and is never part of tile publication.

The draft policy is enforced twice. The publication ledger rejects a candidate whose quality key is not strictly better than the resident entry for the same footprint, and the pyramid walk never assigns a Backdrop or draft ancestor over a better selected descendant. Thus an out-of-order Preview or backdrop fence cannot displace an Interactive or Final picture.

The first complete pass for a newly exposed region may be Backdrop or Preview, because either bounded fallback is better than clear. Once a better tile is published, later navigation can still choose the older fallback only for portions the better tile does not geometrically cover.

## Device-floor resource model

The design assumes only the floor in [minimum requirements](../minimum-requirements.md): WebGL2 or OpenGL ES 3.0, `EXT_color_buffer_float`, four color attachments, a 16 KiB uniform block, 16 vertex texture units, 256 texture-array layers, 2048 maximum texture dimension, core RGBA8 presentation, and unfilterable RGBA32F DATA access.

Version one keeps 16-byte escape records because that is the existing RGBA32F heap dialect. An 8-byte logical format does not save physical heap memory if it is merely padded back into RGBA32F; realizing the smaller column requires a separately proven RG32F or packed-record heap kind and corresponding shader, descriptor, and copy contracts.

One physical tile consumes one 256×256 RGBA32F DATA page, exactly 1 MiB. In the documented 512×512×16 DATA heap there are 64 such pages; the floor profile caps the cache at 56 tile pages, assigns 12 to backdrop ownership and rolling replacement, assigns 44 to Detail and ordinary history, and reserves eight pages outside the tile cache for the current orbit, transient publication needs, and other lattice users. If configuration supplies fewer pages, planning reduces Detail history first, then publishes a smaller backdrop window rather than silently over-allocating.

The tile cache allocates no RGBA8 atlas, color texture array, palette patch, or other retained pixel payload. Reprojection reads the stable RGBA32F DATA view directly in the fragment shader, uses nearest value-map sampling as the correctness baseline, and shades the current pixel from current frame inputs.

Direct shading into the presentation target requires no additional color attachment beyond the frame's ordinary output and depth. If a wider arena composition genuinely requires linear HDR accumulation before tone mapping, it may request exactly one canvas-sized frame transient, preferably RGBA16F on the stated extension floor; that attachment may be physically pooled but is logically cleared and released each frame, has no tile key or descriptor, and never participates in cache eviction.

The placement block is capped at 64 entries. Budgeting twelve `vec4` values, or 192 bytes, per entry costs 12,288 bytes; a 256-byte header keeps the binding at 12,544 bytes, below the 16 KiB floor. The record includes chart bounds, DATA span coordinates, transform, quality facts, and a fixed child-exclusion mask; 44 Detail plus nine active backdrop entries leave eleven entries for transitions or temporary overlap. The exact layout must be a constructor-validated capacity, and relief data that cannot fit that record is moved to vertex/index buffers or split into another draw batch rather than growing the uniform silently.

The executor's existing descriptor buffer already consumes its own fixed capacity and the span directory consumes another fixed uniform binding. Tile placement must use a separate declared presenter block and must validate the combined binding count and per-stage texture count; it cannot assume unused bytes in executor uniforms.

Naively drawing one tile at a time would make an ordinary 1080p view cost 40 to 54 draw calls before backdrop. The target is one render pass containing the surface load clear, exterior draw where needed, one instanced flat-tile batch or a small number of relief mesh-class batches, and no quality-resolution pass. Child masks discard parent fragments and the normal depth test uses true lifted depth. A delivered frame publishes actual batch and draw counts, because the heap study already warns that per-draw CPU cost is material on the intended architecture in [GPU heap lattice](../gpu-heap-lattice.md).

Depth remains `Depth24Plus`, written from the projected lifted geometry and compared in the ordinary direction. Quality, bound, and age never consume depth bits or polygon offset, so a detail/backdrop choice cannot perturb physical visibility or create quality-dependent relief z-fighting.

Nearest sampling is the correctness baseline for value-map records. Aprons and ownership masks replace any reliance on unavailable float linear filtering, and the fragment shader applies the palette and HDR output transform only after the winning record sample and true-depth visibility are known.

SCRATCH remains executor-owned RGBA32F storage. Tile jobs reduce the active rectangle and copy bytes, but they do not resize, alias, or bypass the executor's configured scratch layers; requested and delivered scratch, DATA, span, handle, header-set, and uniform walls remain public facts.

## Cost model and eviction

All byte figures below are binary MiB unless marked as bytes. Tile-cache figures count value-map record storage only; scratch, reference-orbit storage, depth, index and vertex buffers, uniforms, the ordinary presentation target, any optional frame transient, and implementation metadata are stated separately rather than hidden in the total.

| Unit | 8-byte record column | 16-byte RGBA32F record column |
|---|---:|---:|
| One 256×256 physical tile | 0.5 MiB | 1 MiB |
| Nine active backdrop tiles | 4.5 MiB | 9 MiB |
| Twelve protected backdrop and rolling tiles | 6 MiB | 12 MiB |
| 44 Detail and history tiles | 22 MiB | 44 MiB |
| All 56 resident record tiles | 28 MiB | 56 MiB |
| Full 64-page DATA allocation | 32 MiB | 64 MiB |

The 8-byte column is a design comparison, not a claim about the current heap: today's RGBA32F DATA path charges 16 bytes per physical record. CPU or worker compaction alone does not change that GPU cost.

With 253 owned chart intervals per tile axis, a phase-friendly Detail cover needs `ceil(W/253) × ceil(H/253)` tiles at one chart sample per screen pixel, while an arbitrary alignment can require one more tile on each axis. The two target canvases retain the same 12/20 and 40/54 counts as the earlier approximation. The table first shows uncapped Detail demand without the separately resident backdrop, because that reveals where the 44-page Detail budget degrades to coarse fallback.

| Canvas | Friendly / worst tile count | Occupied 8-byte records | Occupied 16-byte records | Physical samples, friendly / worst |
|---|---:|---:|---:|---:|
| 960×540 | 12 / 20 | 6 / 10 MiB | 12 / 20 MiB | 786,432 / 1,310,720 |
| 1920×1080 | 40 / 54 | 20 / 27 MiB | 40 / 54 MiB | 2,621,440 / 3,538,944 |

Adding nine active backdrop tiles makes the friendly and worst-phase 960×540 sets 21 and 29 active tiles, occupying 10.5 or 14.5 MiB with an actual 8-byte dialect and 21 or 29 MiB with current 16-byte records. At 1920×1080, the floor delivers 40 friendly or at most 44 of the 54 worst-phase Detail tiles plus nine backdrop tiles, giving 49 or 53 active tiles and occupying 24.5 or 26.5 MiB at 8 bytes or 49 or 53 MiB at 16 bytes.

The fully delivered active backdrop is exactly 589,824 records. It costs 4.5 MiB at 8 bytes or 9 MiB at 16 bytes; the full 12-page protected budget including three rolling replacements is 6 or 12 MiB of records. At cap 32 its nine-tile worst case is 18,874,368 pixel-iterations, and at cap 64 it is 37,748,736, small enough to establish before expensive visible Final work but still charged and published; reference-limited missing cells reduce delivered bytes and coverage rather than becoming uncertified records.

| Canvas | Direct shading extra | One frame-scoped RGBA16F transient | One frame-scoped RGBA32F transient |
|---|---:|---:|---:|
| 960×540 | 0 MiB | 3.96 MiB | 7.91 MiB |
| 1920×1080 | 0 MiB | 15.82 MiB | 31.64 MiB |

The transient table excludes the ordinary presentation target and depth attachment, which exist independently of tile caching. Version one recommends direct shading with zero extra color target; if arena composition requires the linear transient, its exact delivered format and canvas-sized bytes are frame resource facts, never cache capacity or retained-history cost.

At cap 4096, one full physical tile has a worst-case 268,435,456 pixel-iterations. A friendly 960×540 full cover has about 3.22 billion worst-case iterations versus about 2.12 billion for today's exact screen grid; a friendly 1920×1080 cover has about 10.74 billion versus about 8.49 billion. Aprons and whole-tile rounding make a cold full render more expensive, so tiling is justified by reuse and partial work, not by claiming a cheaper cold frame.

Today's 960×540 design reserves 8 MiB for the 16-byte Final grid and allocates two 1.98 MiB RGBA8 scene textures, about 11.96 MiB combined before 16 MiB SCRATCH and other resources. At 1920×1080 it reserves 32 MiB for records and two 7.91 MiB scene textures, about 47.82 MiB combined. These values follow the resource arithmetic in [Julibrot kernels](kernels.md) and [the precision ledger](precision-ledger.md).

With the backdrop active, the retained 16-byte record working set is 21 MiB versus today's approximately 11.96 MiB combined retained allocation at friendly 960×540 and 49 MiB versus approximately 47.82 MiB at friendly 1920×1080. The floor profile deliberately spends memory on navigation continuity. A full 56-tile value-map cache reserves 56 MiB: 12 record pages are protected for backdrop and 44 for Detail and history. After a friendly 40-tile 1080p Detail cover only four Detail pages, or 4 MiB, are guaranteed return history; at 960×540 the friendly and worst phases leave 32 and 24 Detail pages. The separate documented 16 MiB SCRATCH, eight non-tile DATA pages, ordinary frame targets, and any optional frame transient still apply, and the shared heap texture's total allocation is outside both span-reservation comparisons.

The primary win is a zero-iteration return to a resident chart position, including after unrelated camera motion; continuous pan and zoom reuse the overlap and compute only newly exposed tiles; a camera, observer, height, palette, exposure, tone, output, or canvas change reprojects and shades records without rerunning escape; and a cancelled or failed tile leaves other regions intact. Under strict version-one rules there is no reuse after slice tilt, out-of-plane origin motion, cap change, precision change, or accepted reference regeneration, and the page must not call those cases partial hits.

Eviction first removes logically invalid partitions and failed drafts. It then considers only unpinned valid tiles: the nine active backdrop entries, selected Detail, in-flight, publication-source, seam-source, and active-orbit-dependent entries are pinned until their replacement, fences, and frame ownership release them. Frame color outputs are released by the render graph and never enter this policy.

Among remaining tiles, eviction is LRU by `last_selected_serial` with chart distance from the current visible footprint as the secondary policy requested by the owner. Distance is compared in integer tile space at a common pyramid level, using saturating magnitude or shared-key-prefix order so a deep bignum coordinate is never narrowed to f64; larger pyramid mismatch is the next tie-breaker and stable tile identity is last.

The policy retains invisible Detail tiles when budget permits but never computes them. The explicit backdrop window is exempt and may be maintained off screen within its 12-page wall. Eviction protects the active backdrop and current visible Detail before ordinary history, protects Final over an equivalent draft, and keeps a small balance across recent Detail pyramid levels when pages remain.

When no safe victim exists, allocation fails closed: the scheduler lowers concurrent work and Detail resolution while the protected backdrop continues to own the missing quadrants. Clear appears only when no valid, placeable backdrop exists, such as after a slice or MAIN invalidation, outside the finite backdrop window after an unbounded jump, or after an oracle refusal. The allocator never frees a value-map span still reachable by a submitted draw.

## Oracle, browser proofs, and published facts

Pure key tests must prove that bignum anchor serialization and signed dyadic indices round-trip, adjacent aprons name identical chart sample points, parent and child pyramid points agree where their dyadic lattices coincide, extreme deep-zoom keys do not pass through f64, and arbitrary plane-preserving chart isometries preserve ambient sample points within the stated basis bound.

Compatibility table tests must pin every row in the invalidation matrix, including exact in-plane origin motion, representation-noise tolerance, first out-of-plane displacement beyond the proof bound, plane tilt, cap, precision, reference generation, camera-only changes, palette-, exposure-, and tone-only changes, and stale heap generations.

Selection tests must prove that direct pyramid traversal visits only intersecting ancestors and children, is independent of catalog insertion order and total unrelated cache population, emits a disjoint chart cover, masks every selected child out of its ancestors, falls back to the nearest valid backdrop, rejects invalid generations before emission, never admits error above 1.0 px, and preserves stable quality and age tie-breaking. Property tests should compare the walk with a slow exhaustive cover oracle for small random pyramids.

Flat placement tests compare every tile homography at corners, edges, interior witnesses, multiple extents, deep zooms, and the shipped preset rows against direct five-dimensional projection at height zero. Uploaded coefficients must remain within the ceiling, and a horizon or f32 uncertainty must refuse rather than produce non-finite output.

Relief tests compare the tessellated placement against direct projection using actual tile record heights, the existing five-height regression values, adversarial height discontinuities, near-pole cameras, and maximum subdivision walls. They publish per-tile maximum and p95 error and require every selected tile to have a finite maximum at most 1.0 px. A separate overlap corpus places differently aged fine and backdrop representations over two nearly coincident lifted surfaces and proves that CPU masks choose representation quality while true depth, unaffected by quality, chooses physical visibility.

Seam tests render equal-level and one- or two-level transitions at subpixel pans, fractional zooms, all four edges, mixed quality, and relief. They compare shared-edge screen positions and an ownership image, requiring no clear crack, no double ownership, identical current-frame shading for an identical value record, and at most 1.0 px geometric disagreement.

The shading oracle starts from the selected value-map record, evaluates a CPU reference for palette-to-linear conversion, exposure, tone mapping, and output encoding, and compares it with the pixel produced after GPU placement within the delivered output-format tolerance. Changing any display control must keep selected tile identities and escape-dispatch counts unchanged, affect the next frame without an asynchronous recolor phase, and prove that no cached color payload was sampled.

Heap and publication tests inject stale handles, span exhaustion, directory exhaustion, cancellation before and after DATA copy, out-of-order fences, and eviction during queued work. Frame-resource tests inject optional-transient allocation refusal and prove direct shading or an explicit frame refusal without changing tile validity. The incumbent must remain selectable until a strictly better completed value-map tile is atomically published.

The first browser proof navigates from view A to disjoint view B and back to A under the same MAIN generation, and demonstrates identical selected tile identities, zero new escape dispatches for the resident A cover, and no transient clear after the return frame. A control run with capacity below the A-plus-B working set must show honest evictions and misses.

The second browser proof pans by less than one tile and zooms fractionally, demonstrating reuse of the overlap, direct-walk cost proportional to intersecting nodes, computation only for visible Detail holes plus bounded backdrop maintenance, correct parent masks and mixed-level seams, and monotonically improving regional quality. It repeats at 960×540 and 1920×1080 on the device floor.

The third browser proof first establishes the 3×3 backdrop, then exercises every camera and observer slider, height zero and nonzero, both perspective distances, rotation, pitch, zoom-out, resize, palette, exposure, and tone mapping without changing MAIN. It demonstrates fine-to-backdrop fallback without clear for motions inside the published chart extent, true lifted depth under relief, instant per-pixel reshading with unchanged tile identities and zero display-only escape dispatches, refusal at a constructed pole, and separate detail, backdrop, sky, and clear accounting.

The fourth browser proof changes plane within its stabilizer, then tilts it, moves origin in-plane, then out-of-plane, changes cap and precision, and accepts a new reference. It demonstrates re-keying only for the plane-preserving cases and immediate logical invalidation for every new-content case.

The fifth browser proof runs automatic and manual scene modes through Backdrop, Preview, Interactive, and Final with delayed and reversed completion callbacks. It proves that visible work is prioritized, invisible Detail work is absent, backdrop maintenance stays within its separate wall, manual controls remain honest, and no backdrop or draft ever replaces a better regional picture.

The page must publish the requested and delivered tile geometry; value-map record format; total, Detail, active-backdrop, and rolling-backdrop page budgets; backdrop level, 3×3 chart extent, extent multiples, samples across the longest view axis, cap, bytes, and window generation; record, scratch, orbit, descriptor, span, handle, header-set, uniform, vertex, index, staging, and optional frame-transient byte walls; resident, valid, invalid, pinned, visible, candidate, selected, queued, in-flight, completed, cancelled, hit, miss, and eviction counts; and whether direct or transient HDR composition was delivered.

For the current frame it must publish MAIN and reference generation, slice-key digest, chart anchor digest, requested chart footprint, inverse-bound kind, pyramid nodes visited, exact key lookups, maximum descent, rejected generations, emitted Detail and backdrop entries, parent-mask bits, ideal and selected pyramid-level histograms, refinement-level histogram, detail, backdrop, clear, and sky pixels or fractions, true-depth mode, value-to-pixel shader revision, shaded-fragment count, transient format and bytes if present, tiles per draw batch, total draw calls, render-pass count, placement-uniform bytes, mesh vertices and triangles, and normalized kernel/copy work.

For correctness it must publish the latest invalidation or refusal reason, per-tile or aggregate maximum and p95 warp error, unbounded and pole refusal counts, coordinate-uncertain, projection-uncertain, horizon, and glitch counts, seam witnesses and maximum disagreement, draft-downgrade rejection count, stale-generation rejection count, and whether every selected region has an oracle receipt.

For timing it must retain measured CPU and fence wall times and explicitly label normalized or estimated GPU work. The WebGL2 floor has no timestamp query, so the interface must not turn iteration counts, copy bytes, or fence latency into invented kernel milliseconds.

## Staged migration

Lane-hours are focused implementation-and-test hours for one engineer familiar with the lab, not calendar promises; ranges include Rust tests, browser proofs, documentation, and review fixes but exclude a new compact heap dialect and reference-independent perturbation certification.

| Stage | Working implementation at the end of the stage | Pinning tests and proofs | Estimate |
|---|---|---|---:|
| 0. Pure policy foundation | Current one-scene page unchanged; add canonical chart keys, stable slice-frame aliases, affine-plane equivalence, node-local quality ordering, direct pyramid-walk policy, backdrop sizing, display-input exclusion, invalidation decisions, cost arithmetic, and public fact types behind unused seams | Deep key round-trips, dyadic parent/child and apron identity, equivalent-chart request transforms, exhaustive walk-versus-cover oracle, full invalidation matrix including display-only changes, backdrop extent and byte arithmetic | 16–24 lane-hours |
| 1. N retained whole value maps | Generalize the retained ledger from one promoted picture to a bounded LRU of whole-grid escape-record maps catalogued by content key and chart footprint; the reprojection layer shades the selected records with current palette and HDR inputs, the screen-grid kernel and global ladder remain intact, and N=1 preserves current geometry | N=1 compatibility, A→B→A value-map hit, fence-safe span reuse, instant palette/exposure/tone changes with stable identities and zero escape work, no draft downgrade, LRU under a forced wall | 28–40 lane-hours |
| 2. Fixed record tiles, backdrop, and flat composition | Kernels sample canonical chart rectangles into one-page spans; a semantic pyramid catalog and 12-page backdrop tier publish fixed value-map tiles; inverse-map traversal emits direct-key parent/child masks and height-zero presentation batches exact homographies while per-fragment shading produces pixels, with the legacy whole-map path as fallback | Walk cost independent of unrelated cache size, nested 253-interval nodes, 3×3 extent and roughly 127–253 interval resolution, Detail-over-backdrop masks, flat geometry and post-shading image oracles, seams, stale generations, and both device-floor extents | 52–76 lane-hours |
| 3. Relief-aware one-pass tile placement | Height-bearing tiles use adaptive meshes and per-tile 1.0 px admission; CPU masks remove duplicate representations and one GPU render pass uses only true lifted depth; poles and unproved cells retain legacy fallback during migration | Direct-projection mesh corpus, quality-versus-depth counterexamples, nearly coincident relief surfaces, parent masks, five-height regression, horizon and pole refusal, subdivision wall, and seam continuity | 40–60 lane-hours |
| 4. Per-tile refinement and backdrop scheduler | Replace the one global ladder with visible-first TileFamily jobs plus bounded off-screen backdrop maintenance, sharing one pinned orbit per MAIN generation; atomic publication and pyramid ownership enforce monotonic quality | Automatic and manual ordering, delayed/reversed fences, cancellation, zero invisible Detail dispatches, exact backdrop work wall, rolling-window pinning, shared orbit, and no backdrop or Preview downgrade | 40–56 lane-hours |
| 5. Cache policy, facts, and hardening | Enable the 12-backdrop/44-Detail split by default, remove the legacy fallback only after parity, publish traversal, true-depth, value-shading, and transient-resource facts, and ship browser return/pan/camera/backdrop/invalidation proofs | Long navigation and rolling-backdrop soak, device loss and capacity refusals, both target extents, preset rows and every slider, display-only instant reshading, rotate/zoom-out/pitch fallback without clear inside the extent, A→B→A zero-dispatch proof, and console cleanliness | 28–44 lane-hours |

The revised estimate is 204–300 lane-hours. Stage 2 is the largest seam because screen-aligned kernel output, full-screen scene targets, a single `EscapeGrid`, direct hierarchy traversal, per-fragment value shading, and protected backdrop ownership all change together; splitting it further is acceptable if the legacy whole-map presenter remains the working fallback after each intermediate commit.

Stage 1 is intentionally useful even if later tile work pauses: it proves keying, return-to-position behavior, N-way value-map ownership, display-time shading, and eviction before tile geometry lands. It does not claim pan continuity or reduce partial recomputation.

Stage 2 should initially support height zero, one Detail rung, and the coarse backdrop behind a runtime capability flag. Stage 3 removes the geometric limitation while pinning true-depth semantics, Stage 4 adds Detail and backdrop scheduling policy, and Stage 5 makes the tile path the default only after the same page facts and browser pins are stronger than the legacy path.

## Backlog

- `JB-TILE-001` — Specify and test canonical geometric-slice, bignum-anchor, dyadic pyramid, and arbitrary-precision tile-key encodings, including the 254-sample/253-interval core, shared boundary, apron equations, nested parent/child samples, and plane-preserving chart transforms.
- `JB-TILE-002` — Implement the pure compatibility and invalidation matrix with content-valid, placeable, and preferred as distinct results, preserving the half-texel representation bound and refusing genuine off-plane motion.
- `JB-TILE-003` — Generalize `SceneLedger` to a bounded N-entry whole-grid value-map cache with generation-safe `DataSpan` ownership, monotonic quality, LRU facts, display-time shading, and the A→B→A browser proof.
- `JB-TILE-004` — Add the semantic pyramid-node catalog, fixed canonical-chart aliases, direct inverse-bound traversal, node-local quality buckets, capacity planner, pin ledger, and fence-safe `DataSpan` lifetime without a general spatial search or heap-descriptor change.
- `JB-TILE-005` — Add canonical chart-rectangle shallow and perturbation dispatches with one shared, pinned reference orbit per active MAIN generation and per-tile precision and glitch receipts.
- `JB-TILE-006` — Add reprojection-layer per-fragment value lookup, palette-to-linear conversion, exposure, tone mapping, output encoding, direct-output mode, and an optional single frame-scoped HDR transient with no cached colors or display controls in tile state.
- `JB-TILE-007` — Implement deterministic fine-over-coarse pyramid ownership, parent child-exclusion masks, 64-entry placement batches, exact flat homographies, half-open ownership, aprons, mixed-level transitions, sky, backdrop, and clear accounting.
- `JB-TILE-008` — Extend the reprojection oracle to adaptive relief meshes, actual record heights, seam witnesses, local refusal, and the hard 1.0 px ceiling.
- `JB-TILE-009` — Replace the global ladder with the visible-Detail TileFamily queue plus separately walled backdrop maintenance, no-draft-or-backdrop-downgrade publication, rolling-window pinning, and cancellation tests.
- `JB-TILE-010` — Publish tile, cache, traversal, backdrop, true-depth, value-shading, transient-resource, cover, quality, error, seam, invalidation, scheduling, eviction, and timing facts and add the five browser proof sequences.
- `JB-TILE-011` — Study an actual 8-byte GPU escape-record dialect; accept it only if the device-floor format, exact status and rebase representation, heap kind, shader access, and measured bandwidth benefit are all proved.
- `JB-TILE-012` — Study promotion of certified Final tiles from orbit-generation provenance to reference-independent mathematical content so a reference refresh does not discard a valid return cache.
- `JB-TILE-013` — Implement and prove the protected 3×3 coarse backdrop with nine active and three rolling pages, at least 3× chart extent, roughly 127–253 sample intervals across the longest view dimension, and fine-first fallback under rotation, pitch, pan, and zoom-out.
- `JB-TILE-014` — Benchmark flat-only quality-as-depth against the disjoint hierarchy output; retain it only if it produces bit-equivalent ownership and a measured win, and never share that encoding with true relief depth.

## Risks

Reference churn is the largest product risk and now gates the strongest backdrop promise. The current controller requests a new reference after bounded center or zoom motion and frees the previous orbit; strict generation invalidation can turn an ordinary long pan into a whole-cache miss, while one centered orbit may not certify a 3× deep-zoom region. The first facts must expose reference-limited backdrop cells and invalidations by reason, and reference-independent certification should precede any unconditional no-clear claim.

Cold rendering does more work than the screen-aligned grid because full pages, aprons, phase rounding, and nine coarse backdrop jobs add samples. At 960×540 the friendly 12-tile Detail cover alone is about 52 percent more samples than the screen grid, although the backdrop's cap 32 or 64 limits its iteration cost; cache-hit, rotation, zoom-out, and partial-pan proofs must demonstrate that saved iterations and avoided clear repay this overhead in real navigation traces.

Canonical-chart aliasing is now load-bearing. Plane-preserving controls must be proved against one stable interned frame; if canonicalization changes sign, swaps axes, or drifts at the precision floor, the direct walk can query the wrong nodes even though the affine plane is unchanged. Exact request-to-canonical tests are required before direct traversal is trusted.

Relief can fold, cross a perspective pole, or vary too sharply inside a cell. Adaptive tessellation must have finite walls and regional refusal, which means a camera pose can retain valid records yet temporarily show clear holes; the interface must explain that as placement refusal rather than model invalidation.

A quality-biased depth value is unsafe under relief. Quality must dominate two representations of the same chart point, while physical depth must dominate different chart points; polygon offset or packed depth can reverse a genuine near/far relation. Any implementation shortcut that allows parent and child representations to reach the depth test together reopens this bug.

Mixed-level seams are a correctness problem, not cosmetic filtering. Nearest sampling, duplicated aprons, half-open ownership, non-moving shared transition points, and an ownership oracle all need to agree under fractional zoom and camera tilt.

The 12-backdrop/44-Detail split leaves only four guaranteed Detail history pages after a friendly 1080p cover and cannot hold the phase-worst demand of 54 fine tiles. Ten worst-phase regions deliberately remain coarse. A one-view-plus-fine-history promise cannot be universal on that floor; requested versus delivered resolution and history must be explicit, and backdrop maintenance must never evict visible Final tiles.

The backdrop is finite. Its 3×3 union covers at least three times the current conservative chart bounds, but no finite cache can guarantee an arbitrary teleport or an unbounded near-horizon inverse; outside that published extent the honest result may still be sky or clear until a new coarse window arrives, consistent with the arena's finite-slice limit.

Per-pixel value lookup, palette evaluation, exposure, and tone mapping add fragment bandwidth and arithmetic exactly when cached geometry avoids iteration work. Browser measurements must separate selection, value fetch, shading, and raster cost; an optional HDR transient must stay at one canvas-sized frame resource and may never quietly become retained color history.

Canonical inverse bounds, bignum key arithmetic, hierarchy masks, mesh building, and per-tile descriptors can move cost from GPU iteration to the main thread even without a search. Direct key descent, one visit per intersecting node, bounded node-local buckets, cached placement receipts, and batched draws require measured walk counts and browser walls before the legacy path is removed.

The present heap lattice has fixed uniform and directory capacities and a non-compacting buddy allocator. Fragmentation, span/header exhaustion, or a live fence can prevent publication even when byte totals look sufficient; every allocation path needs a typed wall and an incumbent-preserving failure mode.

Stage 1 whole-grid value maps and later chart tiles have different geometry but the same value-domain semantics. The migration must not fossilize screen pose, palette, exposure, tone mapping, or output format in any key; its first cache stage should already prove that presentation inputs remain frame-local.

## Open questions for the owner

1. Should accepting a new reference orbit invalidate all cached tiles indefinitely, or may certified records outlive their computational reference? Recommendation: keep strict generation equality while the proof is absent because it matches current ownership and the requested invalidation rule, but make `JB-TILE-012` a prerequisite for advertising a universal 3× deep-zoom backdrop or long-pan return guarantee; otherwise publish the smaller delivered backdrop honestly.

2. Is the proposed 256×256 physical tile with a 254×254 core sample grid, 253×253 owned intervals, shared high boundary, and one-sample exterior apron the desired first fixed geometry? Recommendation: yes; it fits one existing 256 page, makes dyadic parent and child samples nest, gives exact neighbor boundaries, and keeps ordinary 1080p selection within the 64-entry placement block, while facts can reveal whether a later 128 or 512 class is warranted.

3. Should version one pursue an 8-byte record? Recommendation: no; use the existing 16-byte RGBA32F record and treat 8 bytes as a measured follow-up, because padding an 8-byte logical record into the present heap saves nothing and packing status plus rebase must remain exact.

4. What floor cache promise should the interface make? Recommendation: promise 56 value-map tile pages split into 12 protected backdrop and 44 Detail/history pages, not “two complete fine views”; publish that a friendly 1080p view leaves four guaranteed Detail history pages and a worst-phase view falls back to backdrop for ten fine regions.

5. How broadly should “plane-preserving re-key” apply? Recommendation: accept every certified affine-plane stabilizer motion as semantically valid, intern one canonical chart per geometric slice, transform each request into that chart, and keep stored pyramid keys axis-aligned; use integer aliases only as an optional dyadic shortcut and never resample records.

6. Does reprojection need a frame-scoped HDR color transient? Recommendation: no extra target for the first Julibrot path; shade records directly into the ordinary presentation target. If wider arena composition requires linear accumulation, permit exactly one canvas-sized RGBA16F transient, 3.96 MiB at 960×540 or 15.82 MiB at 1920×1080, publish its delivered format and bytes, and never key, retain, or evict it as cache.

7. Should relief use a fixed mesh or adaptive subdivision? Recommendation: begin at 16×16 cells per tile and adapt under the existing 1.0 px oracle with hard vertex and depth walls; use true projected lifted depth after CPU hierarchy masks remove parent/child duplicates, because a fixed mesh or quality-biased depth cannot honestly cover near-pole, folded, and discontinuous-height cases.

8. Should the cache persist across page reloads? Recommendation: keep it session-local. Persistent records need a stable formula ABI, canonical bignum serialization, corruption checks, quota policy, and privacy/product decisions that are not needed to prove in-memory return-to-position value.

9. May a lower-cap tile be reused after a cap reduction or increase? Recommendation: keep cap equality in the first `ContentKey`; add a partial order only after the record stores enough first-escape evidence to prove equivalence for the requested cap.

10. When no Detail tile meets the 1.0 px relief ceiling, should the presenter fall back to backdrop or a stale whole-grid value map? Recommendation: use the backdrop only if its own mesh passes the same oracle and use a stale value map only if it independently passes content and regional placement; shade either with current frame inputs, otherwise render sky where geometrically exterior and transient clear where no valid placement exists.

11. Should `(level,bound,age)` be encoded into GPU depth to avoid CPU overlap selection? Recommendation: adopt the speed objective but not that relief encoding; direct pyramid traversal should eliminate representation overlap, child masks should preserve fine-first ownership, and the one render pass should reserve depth for true lifted visibility. A flat-only encoding remains a benchmarkable optimization, not a contract.

12. How much backdrop should be guaranteed? Recommendation: nine active coarse tiles plus three rolling replacements, with dyadic side chosen so each tile spans between one and two times the longest conservative view dimension; this targets at least a 3× union on both chart axes, roughly 127–253 sample intervals across the longest dimension, 9 MiB active records at 16 bytes, and bounded off-screen maintenance, subject to the published reference-certification limit.

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
