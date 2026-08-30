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

## 3. Aperiodicity, recurrence, and the novelty limit

## 4. Coordinates without a privileged origin

## 5. Rendering a bounded endless view

## 6. Physics and constraints in the quotient

## 7. The network, wire, prediction, and rollback

## 8. Consequences for the architecture corpus

## 9. Honest costs, failure modes, and rejection gates

## 10. Migration from the bounded arena
