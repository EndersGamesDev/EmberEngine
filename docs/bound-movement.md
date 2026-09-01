# Bound movement: position as world phase

Every construct introduced here is proposed. Names describe contracts rather than APIs or types.

The adopted architecture separates higher-dimensional authority from its derived three-dimensional view, places an irrational three-dimensional leaf in a compact quotient, and assigns the two internal circles distinct world and content jobs (`docs/4d-first-engine.md:5-7`, `docs/4d-torus-world.md:7-23`, `docs/4d-content.md:33-49`). This document adds a manifest-selectable movement profile: the player's physical position determines the world-circle offset, so travel through the world is travel through world phase.

## 1. Ruling: bind exactly the world circle

The binding acts on the world-circle offset `h_u` and no other channel. The content-circle offset `h_v`, its clamped player ability, its anchor rule, and ordinary scenery's full-content-circle windows pass through unchanged; binding position to the content circle would collapse authored occupancy and player position into the same information, destroying both the ability and the occupancy semantics (`docs/4d-content.md:17-31`, `docs/4d-content.md:37-43`, `docs/4d-content.md:51-61`).

## 2. The proposed binding contract

A binding profile supplies a map `B` from its declared playable domain to the world circle, at least two continuously differentiable derivatives, and a certificate from which the engine derives the playable leaf's metric before deriving any downstream geometry. The profile, its coefficients, its derivative evidence, its domain, its world-circle chart, and every certified bound are immutable session-manifest facts.

Geometry is an output of `B`, never an assumption made by a consumer. Deriving rather than assuming is mandatory because two bindings with the same pointwise phase range can have different distances, support, curvature, and source-query preimages.

## 3. Horizontal profile and antipodal corners

The horizontal profile uses one globally extended affine lift of `B`; it does not clamp, wall, teleport, or reset at the designated surface. The corner condition is one scalar antipodal congruence, so this document holds the certified binding orientation fixed and calibrates one surface-size degree of freedom rather than pretending that the same equation determines both.

The designated surface is a region of the unbounded leaf, and its two corners are calibration facts rather than boundaries. The affine law continues smoothly beyond them, while the resulting graph leaf must independently pass the adopted no-lattice-vector and near-period gates (`docs/4d-torus-world.md:27-35`, `docs/4d-torus-world.md:45-70`).

## 4. Two exclusive orientation profiles

The horizontal profile has exactly zero vertical binding component, so jumping and falling do not change world phase. The vertical plateau profile instead assigns exact antipodal phases above and below a finite smooth transition band; both profiles ship, one title selects one, and combining them is a different design outside this document.

An asymptotic sigmoid is rejected because it reaches neither plateau at finite height and therefore supplies neither an exact phase fact nor a region of exactly zero vertical phase rate. The compact transition must meet the binding contract at both plateau joins, and its concentrated phase rate makes a jump through the band a concentrated world-membership and non-warpable rendering event (`docs/4d-first-engine.md:102-118`, `docs/4d-torus-world.md:161-173`).

## 5. Metric, flatness, and the curvature gate

The binding certificate derives the induced metric, volume element, phase gradient, curvature tensor, and every dependent spatial quantity in that order. Even a shipped profile is called flat only after its certifier evaluates the curvature and obtains zero.

A profile whose certified intrinsic curvature is nonzero is rejected unless separately certified metric-aware rendering, physics, and enumeration exist. The adopted corpus defines no such exception: its renderer, dynamics, and bounded source queries all depend on one authoritative spatial geometry and may not infer a replacement from visible output (`docs/4d-first-engine.md:120-168`, `docs/4d-first-engine.md:246-261`, `docs/4d-torus-world.md:88-141`).

## 6. Derived support and revision cadence

An avatar's world-circle support is derived from its physical footprint and height through `B`; it is never an authored transverse allowance. That support enters the same exhaustive joint internal-support and image enumeration already required for actors and players (`docs/4d-content.md:19-31`, `docs/4d-content.md:83-85`, `docs/4d-torus-world.md:53-55`, `docs/4d-torus-world.md:104-121`).

The binding gradient also determines phase-free instantaneous directions and the cadence at which locomotion revises world membership. A zero net phase after a route does not make the intervening motion phase-free: every nonzero intermediate phase change remains authoritative fixed-tick work.

## 7. Profile-scoped supersessions

Under the adopted compact-world profile, ordinary travel changes the bounded materialization region but leaves the leaf offset and slice identity fixed; under the adopted content profile, world-circle phase is an explicit fixed-tick input (`docs/4d-torus-world.md:37-43`, `docs/4d-torus-world.md:161-173`, `docs/4d-content.md:51-61`). The bound profile supersedes both decisions only for `h_u`: ordinary locomotion changes the world offset whenever its velocity has a nonzero component along the binding gradient, and no separate actuated world-phase input exists.

The fixed-leaf profile remains valid and manifest-selectable. Bound locomotion inherits the categorically non-warpable phase path, so a missed matching scene frame becomes locomotion latency rather than only stale world response (`docs/4d-first-engine.md:102-118`).

## 8. What binding buys and costs

Binding makes world phase legible from place, gives co-located players the same bound world-circle slice, and makes antipodal corners or plateaus mechanically discoverable phase facts. Those properties bear directly on the adopted navigation and comprehension gates in a world that otherwise denies a unique global landmark and requires measured route understanding (`docs/4d-first-engine.md:208-224`, `docs/4d-torus-world.md:177-208`).

The price is a preferred frame, lost translation homogeneity in the movement rule, locomotion-driven membership churn and phase latency, and the deliberate identification of phase with place. The fixed-leaf profile preserves homogeneous originless travel and keeps ordinary locomotion off the phase path; neither profile wins for every game.

## 9. Cumulative rejection gates

The bound profile ships only after its smoothness, flatness, derivative, support, churn, latency, navigation, multiplayer, and target-population certificates all pass over declared representative domains. These gates accumulate with every adopted geometry, slice, quotient, content, physics, networking, authoring, weak-target, and comprehension gate rather than weakening one (`docs/4d-first-engine.md:208-261`, `docs/4d-torus-world.md:177-225`, `docs/4d-content.md:119-140`).

The honest exit is the fixed leaf. A game whose fiction requires homogeneous originless travel rejects binding rather than disguising its preferred phase direction.

## 10. Consequences for the architecture corpus

The three adopted documents remain unchanged here. They would need profile-scoped amendments for a manifest binding and derived leaf metric, a position-driven world offset and source-query cadence, and the replacement of actuated `h_u` by bound `h_u` while leaving every `h_v` contract untouched (`docs/4d-first-engine.md:73-118`, `docs/4d-torus-world.md:161-175`, `docs/4d-content.md:51-61`, `docs/4d-content.md:142-170`).

No adopted invariant or rejection gate is relaxed. The binding profile is admissible only as a derived-geometry specialization that passes the cumulative corpus and preserves the fixed-leaf profile as the exit.
