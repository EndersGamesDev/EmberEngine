# Bound movement: position as world phase

Every construct introduced here is proposed. Names describe contracts rather than APIs or types.

The adopted architecture makes higher-dimensional state authoritative and its visible three-dimensional geometry derived, places an irrational three-dimensional leaf in a compact quotient, and assigns the two internal circles distinct world and content jobs (`docs/4d-first-engine.md:5-7`, `docs/4d-torus-world.md:7-23`, `docs/4d-content.md:33-49`). This document adds a manifest-selectable movement profile in which physical position determines the world-circle offset: travel through the world is travel through world phase.

## 1. Ruling: bind exactly the world circle

The binding acts on the world-circle offset `h_u` and exactly no other channel. The adopted oriented internal chart assigns compact-world recurrence and placement to `u`, while `v` owns genuine-content occupancy and the player's clamped content-phase ability; rotating or exchanging those jobs changes the session and every affected certificate (`docs/4d-content.md:33-43`).

The rejected alternative is a position binding with a content-circle component. It would make physical position and content extent carry the same information, erase the player's independent clamped ability, move proper content-window edges during ordinary travel, and destroy the rule that ordinary scenery spans the entire content circle (`docs/4d-content.md:19-31`, `docs/4d-content.md:41-43`, `docs/4d-content.md:51-59`).

Therefore `h_v`, its clamp, its phase anchor and certified anchor transitions, and ordinary scenery's full-content-circle windows pass through this profile untouched. Binding-driven membership churn is entirely a world-circle-window phenomenon; content membership changes only through the already-adopted genuine-content rules, never as a side effect of `B` (`docs/4d-content.md:27-31`, `docs/4d-content.md:41-43`, `docs/4d-content.md:53-61`).

## 2. The proposed binding contract

A proposed binding object is a map `B: D -> C_u`, where `D` is the declared connected playable domain in physical `E ~= R^3` and `C_u = R / P_u Z` is the world circle of manifest circumference `P_u`; the adopted internal chart already makes circle periods and chart transitions manifest data (`docs/4d-content.md:35-39`). `B` is at least `C^2`. On every certified chart it has a real lift `b`, and two lifts differ by an integer multiple of `P_u`, so their Jacobian and Hessian agree.

The bound playable leaf in the covering space is the proposed graph `F_B(x) = x + b(x)e_u + h_v e_v`, reduced to the quotient only after its geometry is evaluated. This sign convention defines increasing `B` as increasing `h_u`; negating `B` is a distinct manifest orientation, not an implementation choice.

The certificate set is contractual:

1. **Evaluator and regularity.** It identifies `D`, `P_u`, the profile formula or finite data, its chart and seam rules, an evaluator for `B`, and evidence that `B` is `C^2` over the whole declared domain.
2. **Differential evidence.** It carries `J = transpose(grad b)`, `H = Hessian(b)`, a declared norm policy, global or cellwise bounds `G >= norm(grad b)` and `K >= norm(H)`, and checks analytic derivatives against an independent finite-difference or automatic-differentiation oracle.
3. **Derived geometry.** It derives the graph embedding, induced metric and inverse, volume element, intrinsic phase gradient, second fundamental form, full intrinsic-curvature evaluation, and a developing map when flatness is claimed.
4. **Derived occupancy.** It maps every admitted physical collider and swept trajectory through `B`, records the resulting world-circle support rather than accepting an authored substitute, inserts that support into exhaustive image enumeration, and derives membership-revision, candidate-count, and churn bounds.
5. **Operational bounds.** For declared speed `V` and acceleration `A`, it certifies `abs(dot b) <= G V` and `abs(ddot b) <= K V^2 + G A`, then carries those phase-rate and phase-acceleration bounds into fixed-tick latency and source-query budgets.

The session manifest immutably selects the profile, `D`, its coordinate frame and phase gauge, `P_u`, every coefficient, the designated calibration surface, derivative and numeric policies, plateau heights when present, the certificate hashes, support envelopes, and all rejection thresholds. A snapshot, replay, or peer with different binding identity is incompatible input, matching the adopted treatment of ambient rank and source-manifest identity as session facts (`docs/4d-content.md:87-101`, `docs/4d-torus-world.md:143-159`).

The derivation order is `B -> F_B -> (J,H) -> metric, inverse, volume and phase gradient -> curvature -> support and source-query preimages -> membership cadence -> rendering, physics, networking and presentation stamps`. A downstream consumer that assumes Euclidean leaf geometry can return the wrong distance, collider, or population for a manifest-valid `B`, so allowing that assumption would make the manifest cease to be authoritative.

Geometry is therefore an output of the binding, never a premise supplied by its consumer. No renderer, physics path, source query, controller, or gameplay rule may skip ahead in the derivation order because two bindings with the same phase range can have different metric, support, curvature, and query preimages.

## 3. Horizontal profile and antipodal corners

The **horizontal affine** profile chooses a constant gradient vector `beta in E` with `dot(beta,e_z) = 0` for the declared vertical unit vector `e_z`, then uses the global lift `b_H(x) = u_0 + dot(beta,x-x_0)` and circle value `B_H(x) = b_H(x) mod P_u`. Its Jacobian is `transpose(beta)`, its Hessian is zero, and its phase rate is `dot b_H = dot(beta,v)`.

Let opposite corners of the designated two-dimensional surface be `p_A` and `p_C = p_A + Delta`, and define `[u_A] = B_H(p_A)`. “Extreme” and “antipode” mean the two circle points `[u_A]` and `[u_A + P_u/2]`, not an inner point, outer point, minimum, or maximum; the adopted quotient is flat and its serialization cell has no physical center (`docs/4d-torus-world.md:27-35`, `docs/4d-torus-world.md:72-80`). The exact corner condition is the single scalar congruence `dot(beta,Delta) = (m + 1/2)P_u` for integer `m`.

The ruling selects the shortest half-turn, `dot(beta,Delta) = P_u/2`, with the sign absorbed into the manifest orientation of `beta`. The manifest chooses `beta` and a prototype surface shape and orientation whose diagonal is `Delta_0`; uniform surface scale `s` is then derived from `Delta = s Delta_0` as `s = P_u / (2 dot(beta,Delta_0))`, requiring `dot(beta,Delta_0) > 0`. Equivalently, for rectangular edge directions `a_1,a_2` and lengths `L_1,L_2`, the one sizing relation is `L_1 dot(beta,a_1) + L_2 dot(beta,a_2) = P_u/2`; fixing one length derives the other when its coefficient is nonzero.

Thus the goal constrains surface size, not binding orientation. One scalar equation cannot determine a multi-component orientation and multiple surface dimensions, while deriving one uniform scale uses exactly its one degree of information.

This order also protects aperiodicity. First choose `beta` and certify the embedded affine plane `E_beta = {v + dot(beta,v)e_u | v in E}` against `E_beta intersect Lambda = {0}` and the corresponding injectivity and reciprocal-density conditions; only then derive `s`, which changes no lattice or tangent plane and therefore forces no rational relation between `beta` and `Lambda` (`docs/4d-torus-world.md:27-35`, `docs/4d-content.md:33-39`). The condition `dot(beta,e_z)=0` preserves pure vertical travel as a tangent direction but neither proves nor replaces those lattice checks: a horizontal tilt can still create a bad lattice alignment and is rejected when the exhaustive spectrum exposes one.

The shipped profiles declare `D = E`, and the selected horizontal continuation is globally affine modulo the circle over that whole unbounded domain. Clamping at the surface is rejected because it would invent phase plateaus and derivative transitions at arbitrary arena edges, and resetting the lift is rejected because a circle-chart seam is not a world event; `B_H` instead continues smoothly past both corners and winds whenever accumulated phase reaches `P_u`. A displacement that returns `B_H` to the same circle value does not repeat the world unless its full embedded displacement is a lattice vector, which the `E_beta intersect Lambda = {0}` gate forbids.

The designated surface is consequently a calibration region of the unbounded leaf, never a walled box or the boundary of the certificate. No seam, wall, clamp, teleport, cache edge, or collision change occurs at either corner, and any playable continuation beyond it remains inside the same global derivative, support, enumeration, churn, and latency certificate; the adopted far plane and resident halo likewise remain bounds on queries rather than world boundaries (`docs/4d-torus-world.md:88-123`).

## 4. Two exclusive orientation profiles

The manifest offers exactly two named variants of this one-channel orientation choice:

1. **Horizontal affine.** `B_H` is section 3's globally affine circle map with `dot(beta,e_z)=0`; jumping and falling never change world phase, avatar height contributes no bound support, and the tilted affine plane must pass its own no-lattice-vector, injectivity, density, near-period, sightline, and self-image searches rather than inheriting them by name (`docs/4d-torus-world.md:27-35`, `docs/4d-torus-world.md:45-70`, `docs/4d-torus-world.md:98-121`).
2. **Vertical antipodal plateaus.** `B_V` depends only on height, is exactly constant below a lower threshold and above an upper threshold, and traverses exactly half the world circle inside one finite transition band; horizontal walking is phase-free everywhere, while vertical movement is phase-active only in that band.

Both variants ship as proposed profiles, and a title selects exactly one in its immutable session manifest. Adding horizontal and vertical components together would create different null directions, support, metric, churn, navigation, and multiplayer behavior, so that combination is rejected here and requires its own design and certificate.

For the vertical profile, choose `z_- < z_+`, set `H = z_+ - z_-`, and define `q(t) = 0` for `t <= 0`, `q(t) = 10t^3 - 15t^4 + 6t^5` for `0 < t < 1`, and `q(t) = 1` for `t >= 1`. The lift is `b_V(x,y,z) = u_- + (P_u/2)q((z-z_-)/H)`: every point at or below `z_-` has phase `[u_-]`, every point at or above `z_+` has the antipode `[u_- + P_u/2]`, and no circle seam is crossed merely by joining either plateau.

Each vertical plateau is a parallel fixed leaf and must retain the adopted irrationality checks, while the transition additionally certifies every graph chord and image through its full phase range. In particular, the cooker searches possible images between the antipodal plateaus and through the transition rather than inferring global injectivity from the plateau tangents (`docs/4d-content.md:33-49`, `docs/4d-content.md:103-117`).

The minimum admitted smoothness is `C^2`. A constant plateau has first and second derivatives zero, so a compact transition must satisfy `q'(0)=q'(1)=q''(0)=q''(1)=0` as well as its two endpoint values; those six constraints require at least a quintic polynomial, and the stated quintic satisfies them, while its third derivative is allowed to step because the contract consumes no third derivative. A merely `C^1` join is mechanically visible: the Hessian becomes undefined or discontinuous at the join, while the first derivative of the induced metric, the connection and curvature certificate, and bound phase acceleration all consume that Hessian.

The asymptotic sigmoid is the named rejected alternative. It reaches neither plateau at finite height, leaves nonzero phase rate and derived support at every finite altitude, provides no finite domain on which a plateau certificate can prove zero churn, and makes “sufficiently high” an arbitrary tolerance instead of a stable mechanical fact.

Inside the compact transition, `q'(t) = 30t^2(1-t)^2`, so the coordinate phase gradient is `b_V'(z) = (P_u/(2H))q'(t)` and peaks at `t = 1/2` with `15P_u/(16H)`, which is `15/8` times the band's mean rate `P_u/(2H)`. The Hessian has only `b_V''(z) = (P_u/(2H^2))q''(t)`, and `max abs(q'') = 10 sqrt(3) / 3` gives the certified bound `5 sqrt(3) P_u / (3H^2)`.

A jump that crosses the whole band spends exactly `P_u/2` of phase regardless of jump speed, but speed determines how many fixed ticks are available to materialize the changing world. On each tick the authoritative path sweeps `dot b_V = b_V'(z)v_z`; every crossed world-window edge can revise placement membership under the adopted one-sided rule, and the cooker must enumerate the actual edge count and resident-set churn because the derivative bound alone cannot supply it (`docs/4d-content.md:53-59`, `docs/4d-torus-world.md:37-43`, `docs/4d-torus-world.md:191-206`). Every tick with nonzero swept phase also requires a matching site query, affected slice work, and scene render, so a jump through the band pays categorically non-warpable latency rather than presenter correction (`docs/4d-first-engine.md:102-118`, `docs/4d-torus-world.md:161-173`).

## 5. Metric, flatness, and the curvature gate

Take a local lift `b` and write `b_i = partial_i b` and `b_ij = partial_i partial_j b`. The graph tangent vectors are `partial_i F_B = e_i + b_i e_u`, so direct dot products in the flat covering space give the induced metric `g_ij = delta_ij + b_i b_j`; the matrix-determinant and rank-one-inverse identities then give `det(g) = 1 + norm(grad b)^2`, volume element `dV_B = sqrt(1 + norm(grad b)^2)d^3x`, and `inverse(g) = I - grad(b) transpose(grad(b)) / (1 + norm(grad b)^2)`.

The coordinate differential of phase is `db`, while the intrinsic phase-gradient vector is `grad_g B = inverse(g)grad b = grad b/(1 + norm(grad b)^2)`. This distinction is contractual: `db(v)` gives phase rate along a coordinate velocity, whereas `g(grad_g B,v)` gives the same differential after metric duality.

The unit graph normal is `n_B = (e_u - grad b)/sqrt(1 + norm(grad b)^2)`, and direct differentiation gives second fundamental form `II_ij = b_ij/sqrt(1 + norm(grad b)^2)`. The Euclidean Gauss equation therefore yields `R_ijkl = (b_ik b_jl - b_il b_jk)/(1 + norm(grad b)^2)`. These numerators are exactly the `2 x 2` minors of `Hessian(b)`, so a three-dimensional scalar graph is intrinsically flat exactly when `rank(Hessian(b)) <= 1` at every point; this is the general developability condition admitted by this contract, not an assertion that every binding is flat.

For **horizontal affine**, `Hessian(b_H) = 0`, hence every curvature component is zero. Its metric `g_H = I + beta transpose(beta)` and volume multiplier `sqrt(1 + norm(beta)^2)` are constant, and a fixed linear square root of `g_H` supplies the certified Euclidean developing coordinates.

For **vertical antipodal plateaus**, `grad b_V = b_V'(z)e_z` and `Hessian(b_V) = b_V''(z)e_z transpose(e_z)`, which has rank at most one throughout the transition and rank zero on both plateaus and joins. Thus every Gauss numerator evaluates to zero, while the metric is `dx^2 + dy^2 + (1 + b_V'(z)^2)dz^2`, the volume multiplier is `sqrt(1 + b_V'(z)^2)`, and the explicit coordinate `Z(z) = integral_0^z sqrt(1 + b_V'(r)^2)dr` changes it to `dx^2 + dy^2 + dZ^2`. The profile is intrinsically flat even where it is extrinsically bent, and that conclusion follows from both the curvature evaluation and the independent developing map.

The cooker evaluates the full curvature tensor rather than accepting a `flat` manifest flag. For the two shipped formulas it checks the analytic identities above, evaluates derivative and curvature expressions through an independent oracle over the certified cells and joins, and stores the zero-curvature result with the binding certificate; a sampled near-zero result is not a proof.

A binding whose certified intrinsic curvature is nonzero is rejected absent certified metric-aware rendering, physics, and enumeration. That missing machinery would minimally require geodesic-consistent visibility, distance, LOD and occlusion; covariant constrained trajectories or full-dimensional trajectories with consistent forces, contacts and rollback; and complete curved source-query preimages with conservative frustum, interaction-halo and image bounds. The adopted corpus defines none of that machinery and this document neither proposes nor implies it: its renderer and physics must share one authoritative geometry, and bounded enumeration may omit no relevant image (`docs/4d-first-engine.md:120-168`, `docs/4d-first-engine.md:246-261`, `docs/4d-torus-world.md:88-141`).

The two admitted flat profiles reuse Euclidean downstream reasoning only through their certified developing coordinates and exact inverse maps. Raw playable coordinates are never silently treated as developed coordinates when `g_B` is not the identity.

## 6. Derived support and revision cadence

Let `K_x` be the avatar collider's certified footprint and height expressed as center-relative offsets in the playable chart at `x`. Its exact derived world-circle support is `Sigma_B(x,K_x) = {B(x+r)-B(x) | r in K_x}`, with subtraction on `C_u`; the certificate retains this set, including seam splits, rather than replacing it by a scalar when it spans an ambiguous circle arc. This is the same support discipline under which the adopted rank-5 search consumes a full internal region and rejects independent scalar shortcuts (`docs/4d-content.md:31`, `docs/4d-content.md:83-85`).

For **horizontal affine**, `Sigma_H = {dot(beta,r) mod P_u | r in K_x}` and is independent of `x`. For a centered rectangular footprint with orthonormal horizontal axes `a_1,a_2`, half-extents `r_1,r_2`, and any height, the nonwrapping world half-support is `rho_H = abs(dot(beta,a_1))r_1 + abs(dot(beta,a_2))r_2`; this is the support-function maximum of a box, and height contributes exactly zero because `dot(beta,e_z)=0`.

For **vertical antipodal plateaus**, horizontal footprint contributes exactly zero and a body with center height `z` and half-height `c` has exact lifted support interval `[b_V(z-c)-b_V(z), b_V(z+c)-b_V(z)]`, split at a circle seam if necessary. Its width is `b_V(z+c)-b_V(z-c)` because `b_V` is monotone, and the mean-value bound is at most `2c sup_[z-c,z+c] abs(b_V')`; support vanishes when the entire height interval lies inside either plateau. The local support density `abs(b_V')` peaks at the band midpoint; when `2c <= H`, total support is maximized by centering the body's height interval there, and a taller body that covers the whole transition saturates at `P_u/2`.

The derived world support is combined with the unchanged authored content support as the actual joint 2D internal region, then enters every adopted exhaustive first-image, self-overlap, contact, visibility and candidate-count search; a favorable content miss still cannot excuse a world-support failure (`docs/4d-content.md:19-31`, `docs/4d-content.md:103-117`, `docs/4d-torus-world.md:53-55`, `docs/4d-torus-world.md:104-121`). A title may constrain a collider or reject a pose when derived support fails, but it may not author a smaller world half-width.

Binding evaluation and world membership remain fixed-tick authority, consistent with the adopted treatment of phase, placement, collision and rollback (`docs/4d-first-engine.md:112-118`, `docs/4d-content.md:51-61`). Every tick evaluates `B` from the canonical physical position and sweeps `B` over the tick's certified trajectory; a proposed bound-phase revision advances whenever that sweep is nonconstant, even if its endpoints coincide after winding or cancellation. Source membership processes every world-window edge crossed by the sweep under the adopted one-sided ownership rule instead of sampling only the endpoint (`docs/4d-torus-world.md:37-43`).

The certificate derives cadence rather than guessing it. A tick path of physical length `ell_tick` has phase excursion at most `G ell_tick`, while the declared `G`, `K`, speed and acceleration bounds constrain phase rate and curvature inside the tick; exhaustive window-edge enumeration turns that swept arc into maximum additions, removals, collider revisions, image candidates and bytes. Membership is still evaluated every fixed tick when the gradient is active: a budget failure rejects the profile or its movement envelope rather than authorizing a slower membership clock (`docs/4d-first-engine.md:112-118`, `docs/4d-torus-world.md:191-206`).

The phase-free instantaneous directions are exactly the kernel of `dB`. In the horizontal profile they satisfy `dot(beta,v)=0`, a two-dimensional plane containing vertical motion and one horizontal direction perpendicular to `beta`; in the vertical profile every horizontal direction is phase-free, and vertical motion is additionally phase-free only while its whole swept path remains inside one plateau. A tangent instant at a plateau join, a route whose positive and negative phase changes cancel, or an endpoint that returns to the same circle value does not make the intervening movement phase-free.

## 7. Profile-scoped supersessions

This document explicitly supersedes two adopted decisions under the bound profile and nowhere else (`docs/4d-torus-world.md:37-43`, `docs/4d-content.md:51-61`).

First, the compact-world decision that ordinary travel stays in one fixed leaf, cannot change window membership, and leaves the leaf offset and slice identity fixed is superseded (`docs/4d-torus-world.md:37-43`, `docs/4d-torus-world.md:161-173`). Bound locomotion sets `h_u = B(x)` every fixed tick, so ordinary motion revises world phase and can revise world-window membership whenever `dB(v)` is nonzero.

Second, the content document's decision that world-circle phase is a distinct explicit fixed-tick gameplay input is superseded for `h_u` (`docs/4d-content.md:51-61`). The bound profile has no actuated world-offset control: locomotion is the sole input to `B`, while the independent clamped `h_v` content ability and its own input mark remain exactly as adopted.

The justification is mechanical legibility: the profile deliberately makes world phase a readable function of place, so retaining a second `h_u` actuator would let equal positions have unequal phase and break the binding's central fact. Co-located players therefore have the same bound world-circle component of their slice; independently different content ability states can still give them different `h_v`, because this profile makes no stronger multiplayer claim (`docs/4d-content.md:51-61`).

The fixed-leaf profile remains valid and manifest-selectable, with its explicit world-phase mechanic available wherever its own manifest enables it (`docs/4d-torus-world.md:161-173`, `docs/4d-content.md:51-61`). The supersessions are profile-scoped compatibility choices, not amendments to the fixed leaf and not permission for a replay or peer to switch profile during a session.

The honest consequence is that phase-active walking and vertical transition travel inherit the categorically non-warpable path. A completed frame has geometry only for its stamped slice, so a missed matching scene frame becomes locomotion latency rather than merely stale world response; only motion wholly inside the null directions in section 6 avoids that phase cost (`docs/4d-first-engine.md:87-118`, `docs/4d-torus-world.md:167-173`).

## 8. What binding buys and costs

Binding's proposed buy is legibility: the rule exposes world phase as a deterministic function of physical place, co-located players necessarily share the bound world-circle component of the slice, and movement direction determines the sign and rate of phase change instead of exposing an unrelated actuator. Whether players actually form that model is an asserted product hypothesis until the navigational-legibility gate passes.

The horizontal surface's calibrated corners and the vertical profile's exact plateaus become mechanically discoverable stable phase facts. They are local facts of the selected movement profile, not unique quotient landmarks or evidence of a universal-cover address; that distinction respects the adopted world's denial of a complete atlas, unique global landmarks, and origin-distance progression (`docs/4d-torus-world.md:177-189`).

Those facts bear directly on the adopted navigation and comprehension gates. Blind tasks can ask a player to predict phase from position, choose a route to an antipode, explain why a world-window transition occurred, and rendezvous with another player at a shared phase, replacing an abstract phase dial with bounded spatial evidence while retaining the requirement that measured understanding rather than a renderer feature proves success (`docs/4d-first-engine.md:208-224`, `docs/4d-torus-world.md:197-208`).

Binding spends translation homogeneity in the movement rule. Horizontal affine names `x_0`, `beta` and a calibrated surface frame; vertical antipodal plateaus name a vertical axis and two absolute threshold levels, so translating the same route relative to that frame can change its phase history even though the underlying quotient remains flat, compact and without a physical cell center (`docs/4d-torus-world.md:25-35`, `docs/4d-torus-world.md:72-86`).

Binding also makes phase and place the same information. A designer cannot independently move a player and preserve world phase outside the certified null directions, cannot give co-located players different `h_u`, and must pay locomotion-driven membership churn, derived avatar support, source queries, slice invalidation and non-warpable scene latency on ordinary movement.

The fixed-leaf profile pays the opposite trade. It preserves homogeneous originless ordinary travel, stable site membership while walking, and a separate optional world-phase mechanic, but phase is less spatially legible and co-location alone does not encode a title-wide phase landmark (`docs/4d-torus-world.md:37-43`, `docs/4d-torus-world.md:161-173`). A game chooses binding when readable phase-place identity repays the preferred frame and latency; it keeps the fixed leaf when originless homogeneous travel is part of the fiction, and neither profile is the universal winner.

## 9. Cumulative rejection gates

All thresholds below are immutable manifest values chosen before the evidence run. Churn, latency, comprehension and fiction-fit thresholds are asserted product criteria rather than geometric constants; mathematical and enumeration claims require complete certificates over their declared domains.

|Failure watched|Required evidence|Rejection condition|
|---|---|---|
|Contract and smoothness|Independent evaluation of `B`, `J`, `H`, derivative bounds, chart seams, and every plateau join over `D`|The binding is below `C^2`, a lift seam changes a derivative, a carried bound is violated, or a consumer cannot reproduce the derivation order|
|Intrinsic flatness|Full Gauss-curvature evaluation plus the profile's developing-map oracle|Any curvature component is nonzero, any interval proof is inconclusive, or metric-aware rendering, physics and enumeration would be required|
|Aperiodicity and images|No-lattice-vector, injectivity, density, near-period, sightline and self-image searches for the bound embedding and every admitted support|The bound embedding has a lattice resonance, any required search is incomplete, or an unintended repeat or self-image enters a certified horizon|
|Representative running|Fixed-tick captures over maximum certified speed, acceleration, all headings, diagonal arena crossings, null-direction controls, window edges and worst resident halos|World-window additions, removals, candidates, collider revisions, slice builds, scene latency, bytes or missed frames exceed the asserted running budgets|
|Representative jumping|Vertical-profile captures for jumps wholly inside each plateau, entering and leaving the band, crossing the midpoint, and traversing the whole band at every certified launch and fall speed|A plateau produces nonzero churn, a transition skips membership, the `P_u/2` sweep cannot materialize correctly, or non-warpable locomotion latency exceeds the asserted jump budget|
|Derived avatar support|Exact physical-footprint and height images under `B`, orientation and pose envelopes, joint 2D support, exhaustive self-overlap and interaction-halo searches|An authored scalar replaces derived support, a pose exceeds the world-support or image ceiling, or collision and rendering enumerate different images|
|Vertical multiplayer separation|Blind two-player tasks with equal positions, different heights, opposite plateaus, transition crossings, rendezvous, shared-object interaction, loss and rollback|Co-located peers disagree on bound `h_u`, height-separated players cannot predict phase separation, an interaction uses inconsistent memberships, or replay revisions diverge|
|Navigational legibility|Blind route, corner-to-antipode, plateau recognition, phase prediction, return and explanation tasks against a predeclared rubric|Players guess above the asserted threshold, mistake calibration facts for walls or global origins, or cannot explain a membership transition|
|Minimum correct population|Worst-frustum, interaction-halo, support-sweep and window-edge enumeration on every target before occlusion|The minimum correct query, resident, collider, slice, CPU, memory, upload or network population exceeds its target budget|
|Fiction fit|A title-level comparison against the fixed-leaf profile|The fiction requires homogeneous originless travel, phase independent of place, or phase-free ordinary locomotion outside the binding's true null directions|

These gates are cumulative with every adopted geometry, authority, slicing, presentation, quotient, recurrence, navigation, physics, networking, content, authoring, weak-target and comprehension gate; a bound profile supplies additional evidence and relaxes none (`docs/4d-first-engine.md:208-261`, `docs/4d-torus-world.md:177-225`, `docs/4d-content.md:119-140`). In particular, binding-driven churn is charged only to world-circle windows, while content occupancy, the `h_v` clamp, anchor transitions and proper content-window gates remain independently conjunctive (`docs/4d-content.md:17-31`, `docs/4d-content.md:41-61`).

The honest exit is the fixed leaf. A game whose fiction requires homogeneous originless travel rejects binding rather than disguising the profile's preferred phase origin, direction or altitude.

## 10. Consequences for the architecture corpus

The three adopted documents remain unchanged here and would require the following later amendments (`docs/4d-first-engine.md:1`, `docs/4d-torus-world.md:1`, `docs/4d-content.md:1`).

`docs/4d-first-engine.md` would require three profile-scoped changes:

1. Its view-frame and input sections would add binding identity, graph embedding, derived metric and bound-phase revision, source world-circle offset from canonical locomotion instead of a dedicated `h_u` input, and retain the independent `h_v` simulation input (`docs/4d-first-engine.md:73-85`, `docs/4d-first-engine.md:112-118`, `docs/4d-content.md:51-61`).
2. Its slice, state-level and presenter contracts would stamp the binding and derived phase sweep, rebuild affected geometry and placement for phase-active locomotion, and keep every normal-offset change outside ATW (`docs/4d-first-engine.md:87-118`, `docs/4d-first-engine.md:170-182`).
3. Its physics, latency and adaptive-control sections would consume the certified metric, developing coordinates, support and phase-rate bounds, then attribute bound membership, slice and scene costs to locomotion without claiming pixel-scale savings (`docs/4d-first-engine.md:120-194`).

`docs/4d-torus-world.md` would require three profile-scoped changes:

1. Its fixed irrational leaf would become a manifest choice beside the certified bound embedding, and every bound affine plane or nonlinear profile would rerun the applicable irrationality, density, near-period, sightline and image proofs (`docs/4d-torus-world.md:25-70`).
2. Its placement and ordinary-travel sections would evaluate world-window membership over each fixed-tick `B` sweep, derive avatar world support from physical extent, and treat the designated surface as queryable unbounded continuation rather than a boundary (`docs/4d-torus-world.md:88-123`, `docs/4d-torus-world.md:161-175`).
3. Its navigation, materialization, multiplayer and rejection gates would add phase-place legibility, bound revision identity, representative locomotion churn and the fixed-leaf fiction exit without weakening quotient identity or exhaustive enumeration (`docs/4d-torus-world.md:143-159`, `docs/4d-torus-world.md:177-225`).

`docs/4d-content.md` would require three profile-scoped changes:

1. Its two-phase-input section would replace actuated `h_u` with `h_u = B(x)` under the bound profile while preserving the clamped, anchored and independently stamped `h_v` ability (`docs/4d-content.md:51-61`).
2. Its 2D support certificate would derive the world-circle component from physical footprint and height through `B`, retain authored content occupancy, and feed their actual joint region to the same exhaustive rank-5 enumeration (`docs/4d-content.md:19-31`, `docs/4d-content.md:83-85`, `docs/4d-content.md:103-117`).
3. Its cumulative gates and corpus consequences would add binding identity, smoothness, flatness, churn, latency, navigation and multiplayer evidence while keeping ordinary scenery's content-circle windows, content anchors and every content-necessity gate intact (`docs/4d-content.md:119-170`).

No adopted invariant or rejection gate is relaxed. The only changed decisions are the fixed world-leaf offset during ordinary travel and actuated world-circle phase, both superseded solely because the selected bound profile makes `h_u` a manifest-certified function of position; the fixed-leaf profile remains the recovery path, and every authority, geometry, enumeration, content, physics, network, presentation, target and comprehension obligation remains cumulative (`docs/4d-first-engine.md:246-261`, `docs/4d-torus-world.md:197-225`, `docs/4d-content.md:138-170`).
