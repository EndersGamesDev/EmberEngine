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

The irrationality contract has two parts. `E intersect Lambda = {0}` prevents a nonzero exact translation of the placed set, while `Lambda* intersect E_perp = {0}` makes the image of `E` dense in `T`, where `Lambda*` is the reciprocal lattice. The first says the leaf never closes; the second says travel samples every quotient phase arbitrarily closely. Both are cooker checks on the declared lattice and orientation, and motif decoration must separately reject any accidental translation symmetry.

A concrete design example, not a production guarantee, takes `A = L I_4` and `nu = (1, sqrt(2), sqrt(3), sqrt(5)) / sqrt(11)`. The four numerator components are linearly independent over the rationals because they are distinct members of the standard basis of the multiquadratic field `Q(sqrt(2),sqrt(3),sqrt(5))`; consequently no nonzero integer lattice vector lies in `E`, and no reciprocal-lattice vector is parallel to `nu`. Production parameters still require a bounded Diophantine search because mathematical irrationality alone says nothing useful about how soon a near-alignment appears.

The columns of `A` are therefore a content lever, not a packing detail. Their Gram matrix fixes covolume, shortest loops, injectivity radius, and the lengths and directions of projected near-neighbors; a short vector nearly parallel to `E` produces a conspicuous sightline, while an ill-conditioned cell makes candidate enumeration and minimum-image search expensive. Replacing `A` by `A U` for a unimodular integer matrix `U` merely renames the same lattice and changes nothing physical, so design review compares lattices and projected spectra rather than matrix columns in isolation.

Each quotient-cell record stores only `A`, `nu`, the leaf phase, a finite list of motif phases and windows, references to a finite asset vocabulary, and finite deterministic decoration parameters. The asymptotic site density contributed by record `j` is `length(W_j) / abs(det(A))` per unit physical 3-volume: a large physical region times its bounded internal window forms a 4D cylinder, and lattice-point count divided by cylinder volume tends to one over the lattice covolume. Summing this quantity over records sets population density without allocating the population.

The window convention is lower-inclusive and upper-exclusive. An exact lower endpoint selects the site and an exact upper endpoint rejects it, with the same adaptive sign discipline already required for slice degeneracies; adjacent windows may therefore partition internal space without double ownership (`docs/4d-first-engine.md:45-53`). Phase movement or slice tilt may change site membership at such a boundary, but ordinary travel within the fixed leaf cannot: it moves the observer through one already-defined aperiodic set.

The compact source does not grant infinitely many mutable places. All lifts of one quotient record are manifestations of the same finite authority, so a source mutation affects its whole equivalence class; distinguishing and remembering every visited lift would introduce the growing database this design forbids. Persistent one-off environmental edits are therefore outside the world contract, while a bounded number of dynamic 4D bodies remains ordinary level-3 state.

## 3. Aperiodicity, recurrence, and the novelty limit

## 4. Coordinates without a privileged origin

## 5. Rendering a bounded endless view

## 6. Physics and constraints in the quotient

## 7. The network, wire, prediction, and rollback

## 8. Consequences for the architecture corpus

## 9. Honest costs, failure modes, and rejection gates

## 10. Migration from the bounded arena
