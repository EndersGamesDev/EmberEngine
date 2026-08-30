# The 4D torus world

This document adopts the world topology that follows from Ember's 4D-first authority. The preceding architecture makes unsliced four-space the source of simulation truth and makes a three-dimensional slice a disposable render derivation; this successor compactifies that source and specifies how a player can inhabit its slice without entering a periodically copied room (`docs/4d-first-engine.md:5-7`, `docs/4d-first-engine.md:174-182`).

Every construct introduced here is proposed. Names describe contracts rather than APIs or types in the current tree.

## 1. Ruling: an irrational leaf, not a recycled room

The world is the flat four-torus `T = R^4 / Lambda`, but the player does not walk a rational three-torus cut from it. The traversable world is an irrational three-dimensional leaf `E + h` immersed in `T`; moving along any of its three axes never closes, and a bounded acceptance window selects lattice lifts whose projections place intact three-dimensional objects around the player. The finite source is one 4D quotient cell containing lattice, window, motif, and asset records, while each bounded local view is a deterministic query of that source.

This is the cut-and-project design. It buys the exact compactness, boundarylessness, homogeneity, and finite source of a quotient without any nonzero 3D translation that repeats the entire visible arrangement. It costs simple global coordinates, unique landmarks, cheap quotient physics, and unlimited novelty: the asset vocabulary is finite, every finite patch recurs, and sufficiently good near-periods eventually make the world's family resemblance visible.

The direct quotient is rejected as the shipped world. A flat three-torus satisfies endless travel only topologically: its shortest lattice translations reproduce every landmark in aligned copies, a view longer than those translations exposes the cell, and delaying recognition by enlarging the cell spends authored content and memory in direct proportion to the volume being hidden. It remains the first migration profile because its exact repeats make wrapping bugs easy to see.

Unbounded procedural generation is also rejected as world authority. A pure random-access function can be deterministic and bounded in cache size, but conventional chunks introduce an absolute integer address, generation-version protocol, rollback inputs, and either forgotten edits or an ever-growing persistence log; networking must reproduce the generator bit for bit before it can reproduce a scene. Procedural variation remains legal inside a finite motif record, but it receives quotient phase and stable site identity as inputs and may not create an independently expanding world database.

|Family|What it satisfies cleanly|Decisive cost|Disposition|
|---|---|---|---|
|Direct quotient|Finite storage, exact quotient physics, seamless wrap, no origin|Global exact periods and aligned self-images|Migration and diagnostic profile only|
|Irrational cut-and-project|Finite source, no exact 3D period, bounded local materialization|Recurrence, harder coordinates and physics, finite vocabulary|Shipped design|
|Unbounded procedural chunks|Arbitrary apparent extent and random-access variety|Absolute chunk identity, persistence growth, generator agreement on the wire|Rejected as authority|

The representation ruling is equally firm: ordinary environmental content is **placed**, not exposed as a changing cross-section. Selected lattice lifts project stable site centers into the leaf, and each site instantiates an intact 3D asset, collider, and semantic identity; genuinely 4D cross-section-changing geometry remains reserved for explicit phase mechanics and 4D actors already defined by the adopted slicer. This prevents a tree or doorway from changing topology merely because the player walked, while preserving the prior rule that no rendered slice becomes authority (`docs/4d-first-engine.md:37-43`, `docs/4d-first-engine.md:67-71`).

## 2. The quotient, lattice, and cut-and-project source

Let `A` be an invertible `4 x 4` real matrix, let `Lambda = A Z^4`, and identify `p` with `p + A k` for every integer four-vector `k`. The result `T = R^4 / Lambda` is compact, flat, and boundaryless; translation acts transitively on it, so the fundamental cell used by a serializer is a gauge choice rather than a physical center. All metric questions use the Euclidean metric upstairs in `R^4` and all equivalent lifts, not the visual shape of a chosen cell.

Choose a unit normal `nu`, the physical subspace `E = {x | dot(nu,x) = 0}`, and orthogonal projections `pi_parallel` onto `E` and `pi_perp` onto its one-dimensional complement. For motif record `j` with quotient phase `s_j`, half-open acceptance interval `W_j`, and leaf offset `o`, the placed centers are `P_j = { pi_parallel(s_j + A k - o) | k in Z^4 and pi_perp(s_j + A k - o) in W_j }`. A local query enumerates only the `k` whose projected centers can affect its bounded region; `k` is an ephemeral lift label, not a persistent world address.

The irrationality contract has three independent checks. `E intersect Lambda = {0}` prevents a nonzero exact translation of the placed set, `E_perp intersect Lambda = {0}` makes projection of lattice sites into physical space injective, and `Lambda* intersect E_perp = {0}` makes the image of `E` dense in `T`, where `Lambda*` is the reciprocal lattice. The first says the leaf never closes, the second prevents distinct source sites from collapsing onto one center, and the third says travel samples every quotient phase arbitrarily closely; motif decoration must separately reject any accidental translation symmetry.

A concrete design example, not a production guarantee, takes `A = L I_4` and `nu = (1, sqrt(2), sqrt(3), sqrt(5)) / sqrt(11)`. The four numerator components are linearly independent over the rationals because they are distinct members of the standard basis of the multiquadratic field `Q(sqrt(2),sqrt(3),sqrt(5))`; consequently no nonzero integer lattice vector lies in `E`, and no reciprocal-lattice vector is parallel to `nu`. Production parameters still require a bounded Diophantine search because mathematical irrationality alone says nothing useful about how soon a near-alignment appears.

The columns of `A` are therefore a content lever, not a packing detail. Their Gram matrix fixes covolume, shortest loops, injectivity radius, and the lengths and directions of projected near-neighbors; a short vector nearly parallel to `E` produces a conspicuous sightline, while an ill-conditioned cell makes candidate enumeration and minimum-image search expensive. Replacing `A` by `A U` for a unimodular integer matrix `U` merely renames the same lattice and changes nothing physical, so design review compares lattices and projected spectra rather than matrix columns in isolation.

Each quotient-cell record stores only `A`, `nu`, the leaf phase, a finite list of motif phases and windows, references to a finite asset vocabulary, and finite deterministic decoration parameters. The asymptotic site density contributed by record `j` is `length(W_j) / abs(det(A))` per unit physical 3-volume: a large physical region times its bounded internal window forms a 4D cylinder, and lattice-point count divided by cylinder volume tends to one over the lattice covolume. Summing this quantity over records sets population density without allocating the population.

The window convention is lower-inclusive and upper-exclusive. An exact lower endpoint selects the site and an exact upper endpoint rejects it, with the same adaptive sign discipline already required for slice degeneracies; adjacent windows may therefore partition internal space without double ownership (`docs/4d-first-engine.md:45-53`). Phase movement or slice tilt may change site membership at such a boundary, but ordinary travel within the fixed leaf cannot: it moves the observer through one already-defined aperiodic set.

The compact source does not grant infinitely many mutable places. All lifts of one quotient record are manifestations of the same finite authority, so a source mutation affects its whole equivalence class; distinguishing and remembering every visited lift would introduce the growing database this design forbids. Persistent one-off environmental edits are therefore outside the world contract, while a bounded number of dynamic 4D bodies remains ordinary level-3 state.

## 3. Aperiodicity, recurrence, and the novelty limit

“Aperiodic” means only that no nonzero translation preserves the whole decorated 3D set. It does not mean that the player receives endless new authorship: the same finite assets, materials, encounter rules, and motif records remain visible forever, and their neighborhoods exhibit family resemblance even though the entire arrangement never snaps into exact registration.

This design has finite local complexity when the asset vocabulary is finite, windows are bounded intervals, and overlapping sites are resolved by a finite deterministic rule. For any fixed patch radius there are finitely many adjacency and motif patterns up to translation. Enlarging the radius can reveal more combinations, but it cannot create a new authored asset or a one-off historical place.

Every finite nonsingular patch recurs within a bounded distance. A patch of radius `R` depends on finitely many window membership tests; if none lies exactly on a window endpoint, the same answers hold on an open neighborhood of the corresponding quotient phase. The irrational `E` action is dense on compact `T`, and finitely many translates of that open neighborhood cover `T`; the largest translation used by that finite cover is a recurrence bound. This is an existence derivation, not a useful production number, and it is why a shipped leaf phase must be nonsingular even though the half-open rule still defines exact-boundary behavior.

The useful measurable warning is a near-period. For internal tolerance `delta`, define `D(delta) = min { norm(pi_parallel(lambda)) | lambda in Lambda minus {0}, norm(pi_perp(lambda)) <= delta }`. Translating by the winning projected vector exactly aligns site centers and changes membership only for lattice sites whose internal coordinates lie within `delta` of a window endpoint; smaller `delta` demands a more faithful resemblance and normally pushes `D` outward. The cooker must enumerate this spectrum through the largest required travel horizon and reject a lattice, orientation, or window set whose `D(delta_visual)` falls inside the distance at which repeated composition is unacceptable.

For the cubic example in section 2, an exhaustive integer search gives `D(10^-5 L) = 22.1359436 L`, attained by lattice coefficients `(-4,11,-17,8)` with internal miss `8.82748 x 10^-6 L`. At the purely illustrative scale `L = 128 m`, that is about `2.83 km`; it is a warning that “irrational” by itself can still look related on an ordinary play route, not a recommended production scale. The search is globally complete for that threshold because every vector that could beat the result has Euclidean norm below `22.136 L`, and all such integer vectors were enumerated.

The distance at which a player notices self-similarity is controlled jointly by the near-period spectrum `D`, the total window-edge margin of the visible patch, view distance, lattice scale and conditioning, motif count, and the perceptual distinctness of the finite assets. Increasing motif entropy can disguise recurrence but consumes source data; increasing lattice scale pushes near-alignments outward but can lower site density or require more motif records; reducing view distance hides correlations but weakens the endless vista. These are trades, not proof that recognition can be postponed forever.

The player therefore gets an everywhere-continuing, nonperiodic arrangement with no seam and no globally repeating translation. The player does not get unlimited landmarks, permanent evidence of unique travel, a promise that a long route never resembles an earlier route, or a conventional exploration game in which novelty certifies distance from home.

## 4. Coordinates without a privileged origin

## 5. Rendering a bounded endless view

## 6. Physics and constraints in the quotient

## 7. The network, wire, prediction, and rollback

## 8. Consequences for the architecture corpus

## 9. Honest costs, failure modes, and rejection gates

## 10. Migration from the bounded arena
