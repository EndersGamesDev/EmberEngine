# Ember as a 4D-first engine

## 1. Decision and coordinate model

Ember's authoritative world is spatial four-space, `R^4`, with coordinates `(x, y, z, w)`. Time is not the fourth coordinate: it remains the separate parameter of a deterministic fixed-step 60 Hz simulation, while the player sees the three-dimensional hyperplane slice occupied by the current viewpoint. Reading the fourth coordinate as time would make “render a 3D slice” mean only “render the present,” which every real-time engine already does; it would add no representation, rendering, input, or physics decision. The spatial reading is therefore not an analogy layered over a 3D engine but the engine's defining model.

This decision sets the direction of authority: simulation owns 4D bodies, the slice stage derives transient 3D render geometry, and the rasterizer and presenter consume that derivation. A rendered slice may never become the collision world, the network authority, or the source from which the 4D state is reconstructed.

## 2. Authoritative 4D world representation

### 2.1 Boundary cells, connectivity, and solids

The canonical asset is a conforming, oriented simplicial complex in `R^4`. Its vertices carry stable asset-local IDs and four coordinates; each volume cell is a 4-simplex with five vertex indices, density, and material ownership; each of its five tetrahedral facets records the adjacent volume cell or an exterior marker. The union of the 4-simplices is the solid body, and the exterior facets, oriented outward from their single incident volume cell, form the closed tetrahedral 3-manifold that this document calls a 4D mesh.

That distinction is load-bearing. A triangle is only two-dimensional and therefore cannot tile the three-dimensional boundary of a 4D solid; a triangle soup with a fourth coordinate is still a 2-manifold and cannot define the interior required by mass properties or collision. A boundary-only tetrahedral complex is acceptable for static scenery when it is closed and oriented, but every dynamic rigid body also retains either its 4-simplex fill or a derived convex 4D decomposition so volume, center of mass, inertia, support mappings, and containment never have to be guessed from the visible slice.

Connectivity is explicit rather than recovered by hashing coordinates at runtime. A build step validates that every interior tetrahedral facet has exactly two incident 4-simplices with opposite induced orientations, every exterior triangular face has exactly two incident boundary tetrahedra, no volume cell has zero signed 4-volume, and material seams duplicate corner attributes without duplicating topology. Stable vertex, edge, facet, and cell IDs make slicing deterministic and let a lower-detail complex declare a correspondence to the next level.

The rejected primary representation is a sampled 4D signed-distance field. An SDF makes primitives and booleans pleasant, but a dense field grows with the fourth power of resolution, a sparse field still needs a contouring policy to recover crack-free tetrahedral connectivity, and sampled gradients make exact material seams and deterministic networked mass properties depend on resolution. SDFs remain an authoring intermediate; the cooked artifact is the validated simplicial complex.

### 2.2 Authoring and generalized normals

Content is authored from operations people can reason about: tesseracts and other parameterized 4D primitives; Cartesian extrusion of a closed 3D solid through a nonzero `w` interval; sweeps of a 3D solid along a 4D path; products and 4D rotations of lower-dimensional profiles; and boolean composition before constrained 4D tetrahedralization. Importing a conventional model does not pretend that its triangles already form a 4D mesh: the importer first repairs and closes the 3D boundary, treats it as a 3D solid, gives that solid explicit `w` thickness, and only then cooks the resulting 4D volume.

This requires a new proposed 4D asset format and cooker beside, not inside, glTF. The current loader turns glTF positions, normals, and UVs into flat 3D triangle lists (`crates/ember-engine/src/assets.rs:70-107`), and the established asset paths all end in GLB (`docs/asset-pipeline.md:8-18`); preserving that path as an extrusion input keeps existing art useful without misrepresenting what GLB can encode.

The current editor is likewise a 3D front end, not a latent 4D editor: its placed object stores `Vec3` position and scale plus one yaw scalar (`crates/ember-editor/src/lib.rs:254-264`), and export reduces objects to grounded obstacles and 2D spawn points (`crates/ember-editor/src/level.rs:78-109`). Stage one of 4D authoring therefore adds an explicit `w_min`/`w_max` slab and a 4D-preview slice to this editor; arbitrary vertex-level 4D sculpting is deferred until the primitive-and-sweep workflow proves insufficient.

The normal analogue is simpler than the geometry. At a regular point of a three-dimensional hypersurface in `R^4`, the tangent space is three-dimensional and its orthogonal complement is one-dimensional, so an oriented unit normal is a four-vector. For a boundary tetrahedron with tangent edges `a`, `b`, and `c`, the cooker takes the Hodge dual of `a ∧ b ∧ c`, flips it away from the incident volume cell, and normalizes it; a smooth vertex normal is the normalized 3-volume-weighted sum of incident outward cell normals, while creases retain per-corner normals.

An authored two-component UV is not enough to parameterize a three-dimensional boundary without pervasive collapse. The canonical vertex attribute is therefore a three-component intrinsic texture coordinate, with optional four-component object-space procedural coordinates; a material chooses a 3D texture, triplanar/procedural evaluation, or an explicit chart that reduces those coordinates to 2D. This costs more texture storage and tooling than glTF's `TEXCOORD_0`, but it makes texture transport through a changing slice a defined interpolation rather than a new unwrap every frame.

## 3. The slice operation

### 3.1 Exact boundary-cell algorithm

Let the oriented viewing hyperplane be `H = { p | dot(n, p - o) = 0 }`, with unit 4-vector normal `n`, origin point `o`, and an orthonormal world-from-slice basis `B = [b0 b1 b2]` whose columns span `H`. The slicer's input is the exterior tetrahedral boundary, not the volume fill; intersecting one boundary tetrahedron with `H` yields the triangle or quadrilateral surface patch that the 3D rasterizer needs.

For every unique boundary vertex `i` reached by the plane query, compute and cache `d_i = dot(n, p_i - o)` and its sign. Reject a tetrahedron whose four signs are equal. A one-versus-three split crosses three of the tetrahedron's six edges and emits one triangle; a two-versus-two split crosses four edges and emits one quadrilateral, split into two triangles by the predeclared diagonal in the sign-mask case table. There is no other nondegenerate topology.

For each crossing edge with canonical endpoint order `(a,b)`, compute `t = d_a / (d_a - d_b)`, the 4D point `p = p_a + t(p_b - p_a)`, and its slice-space position `q = transpose(B)(p-o)`. Cache that result by the stable undirected edge ID, so adjacent tetrahedra reuse the same position and attributes bit for bit. A 16-entry sign-mask table supplies polygon vertex order; compare its 3D triangle normal with the projected outward hypersurface normal and reverse the order when necessary, making the rasterizer winding follow the solid's orientation rather than the arbitrary order of cell indices.

The current rasterizer contract is already the right final shape—flat 3D triangle vertices with 3D positions and normals plus texture coordinates (`crates/ember-engine/src/renderer.rs:15-32`)—but the proposed slicer owns a transient indexed buffer before the existing mesh upload expands or consumes it. The 4D complex never enters a vertex shader as if it were render-ready.

### 3.2 Degeneracy and numerical policy

Classification is global per vertex and uses an adaptive-precision sign of the affine plane expression over the exact binary inputs; it never uses a cell-local epsilon. If that expression is exactly zero, the definition is the one-sided symbolic limit obtained by translating the oriented plane an infinitesimal distance along `+n`, so every shared occurrence of the vertex receives the same side without moving any stored coordinate.

That rule resolves the dangerous cases decisively. A tetrahedral cell lying wholly in `H` is excluded because it has no patch in the chosen one-sided limit; a triangular face lying in `H` is owned by the limiting side rather than emitted twice; a tangent vertex or edge produces zero area and is dropped; and a genuine topology change as the plane passes a tangency occurs on one declared side instead of flickering between two floating-point answers. The slicer increments separate counters for exact-zero classifications, discarded zero-area patches, and topology changes, because a content pipeline that produces many of them is numerically legal but operationally hostile.

The intersection division is performed in `f64` from the canonical endpoint order. Opposite exact signs guarantee a nonzero denominator; `t` may be clamped only when it lies within four representable steps of `[0,1]`, and anything farther outside fails the cell and reports a numerical error instead of drawing a spike. The slice basis is regenerated from the normalized orientation, checked for orthogonality and positive handedness, and attached to the slice identity so two stages cannot silently project with different bases.

Crack prevention follows from three rules together: one cached sign per stable vertex, one cached intersection per stable edge, and one sign-mask topology table for every cell. Recomputing any of those independently per tetrahedron is rejected even if it looks equivalent, because roundoff on a shared triangular face would then be allowed to select different topology or positions on its two sides.

### 3.3 Execution placement and cost model

The first implementation runs on the CPU and uploads an indexed 3D slice. This is a portability and topology decision: Ember's wasm target explicitly includes a WebGL fallback (`crates/ember-engine/Cargo.toml:30-35`), while GPU construction requires compute, output-size prefix sums, cross-workgroup edge deduplication, and a second path for non-compute devices. A later GPU slicer is permitted only as an oracle-checked acceleration of the same sign masks and edge identities, not as a second slicing definition.

Let `B` be the number of exterior tetrahedra, `N_q` the number of 4D BVH nodes visited by the hyperplane query, `C` the candidate tetrahedra returned, `V_c` their unique vertices, and `A ≤ C` the cells actually cut. One slice performs `V_c` four-component plane evaluations, at most `6C` edge classifications, at most `4A` edge interpolations, and emits at most `2A` triangles. The traversal costs `O(N_q + C)` and is `O(B)` in the worst case of a plane crossing every BVH node; the document does not hide that worst case behind “GPU-friendly.”

With a 40-byte transient vertex—3D position, 3D normal, three texture coordinates, and packed material metadata—and 32-bit indices, the worst output traffic is `40 × 4A + 4 × 6A = 184A` bytes before allocator overhead. At `A = 10,000` that is 1.84 MB per changed slice; at `A = 100,000` it is 18.4 MB, which is already too much to rebuild and upload at 60 Hz on a weak client. Static objects cache by `(asset_id, object_pose, slice_id, lod)`, and moving bodies slice only their own boundary; nevertheless, content budgets must cap active boundary cells rather than assuming caching makes arbitrary 4D detail free.

The rejected first implementation is a full GPU scan of all `B` cells. It avoids CPU upload only after paying `B` classifications, needs capacity for the worst output or a count-and-prefix pass, and makes the weakest fallback run a different algorithm. CPU slicing also gives the editor, collision visualizer, headless tests, and renderer the same inspectable 3D artifact.

### 3.4 Attribute transport

Every edge intersection uses the same `t` for position, smooth four-normal, intrinsic texture coordinate, vertex color, skin weights, and any declared continuous custom attribute. The interpolated four-normal is projected into the slice tangent space as `m_H = m - dot(m,n)n`, transformed by `transpose(B)`, and normalized; if its length falls below the declared tangency threshold, the patch is degenerate and is discarded rather than assigned an arbitrary normal.

Material ID, crease group, collision class, and other discrete attributes do not interpolate. They belong to the oriented boundary tetrahedron or to a duplicated corner at a declared seam, so output triangles remain flat-tagged; a quadrilateral is split without crossing a material boundary because it came from one cell. Skinning, when supported, deforms the 4D vertices before distance classification, since slicing first and then applying a 3D skin can neither reveal nor remove geometry along `w` correctly.

Texture continuity is therefore a property of the cooked 3D boundary coordinates, not of the transient cut polygon. Two adjacent cells that share topological corners share their interpolants through the edge cache; a deliberate chart or material seam duplicates attributes at that topology while retaining the same geometric edge point.

## 4. Position and orientation of the viewing hyperplane

An oriented affine 3-hyperplane has four degrees of freedom: a unit normal `n ∈ S^3` contributes three and the signed offset `h = dot(n,o)` contributes one. Translating the eye within the hyperplane changes the 3D camera position but not the set of world points in the slice; rotating the camera within the hyperplane changes the 3D view but not the slice. Those ordinary three translational and three rotational camera freedoms sit on top of, rather than inside, the hyperplane's four freedoms.

The engine represents a complete 4D view frame by a position `o ∈ R^4` and `R ∈ SO(4)`, with the first three columns spanning the visible hyperplane and the fourth column its oriented normal. The slice artifact stores the `n`, `h`, stable slice basis, source simulation tick, and monotonically increasing slice ID with which it was built. A later view may move and rotate within that stored basis without changing its geometry; a change to `n` or `h` invalidates the artifact.

The player controls both kinds of freedom, but through different input classes. Ordinary locomotion supplies a three-vector in the current slice and ordinary look supplies an `SO(3)` rotation within it. A dedicated phase input changes `h` by moving along `n`, and a dedicated three-component slice-tilt input rotates `n` toward each of the three current basis directions; phase and tilt are gameplay inputs sampled only by the fixed-step simulation, never cosmetic presenter inputs.

This mapping is deliberately not “the mouse controls an arbitrary `SO(4)` rotation.” Mouse yaw and pitch remain comprehensible in the visible 3D world and remain eligible for low-latency presentation correction; slice tilt is slower, explicit, and allowed to reveal, merge, or remove geometry. Games may constrain any of the three tilt components or the phase range, but the engine representation does not collapse them.

A relative 4D rotation lies in the six-dimensional `SO(4)`. Its three generators wholly within the current hyperplane form the `SO(3)` stabilizer of `n` and only change the 3D camera; its other three generators rotate a basis direction with `n` and therefore change which world points satisfy the slice equation. The pipeline tests that distinction geometrically: `n_new = n_old` and `h_new = h_old` permits slice reuse, while any other result re-queries the 4D BVH, rebuilds affected cut geometry, and renders a new `SceneFrame`.

When a tilt changes `n`, the new slice basis is obtained by applying the simulated 4D orientation delta to the old basis and re-orthonormalizing, not by choosing three fresh arbitrary perpendicular vectors. That parallel transport prevents a small slice tilt from causing a large, unrelated roll of the visible 3D coordinate frame; the slice ID still changes because the hyperplane changed.

## 5. The hard limit of asynchronous timewarp

### 5.1 Inputs that remain warpable

The adopted presenter warps an already-rendered image from a scene pose toward a later view pose, with rotation-only correction first and depth-aware translation later (`docs/atw-first-rendering.md:73-88`). In a 4D-first engine that remains valid only for the subgroup that preserves the frame's slice hyperplane. The presenter is a 3D image reprojection stage, not a deferred 4D renderer.

|Late change after the slice was built|Presenter action|Truthfulness|
|---|---|---|
|Rotation wholly inside the stored hyperplane|Apply the existing 3D rotation homography with the frame's view basis|Exact for the rotation model already adopted, subject to its guard band|
|Translation wholly inside the stored hyperplane|Apply depth-aware 3D reprojection when that stage exists|Approximate at disocclusions, exactly the limitation already accepted for translation|
|Projection jitter or presenter-side UI|Update the presenter without touching the slice|Safe while UI remains outside the warped scene image, as adopted (`docs/atw-first-rendering.md:102-104`)|
|No new world or slice input|Re-present the newest complete frame|Safe; world freshness changes, slice meaning does not|

The group statement is precise: the warpable rotations are the `SO(3)` stabilizer of the oriented normal `n`. A general `SO(4)` delta cannot be encoded as a 3D homography, because the frame contains color and depth only for points already in `H`; it contains no samples from the neighboring `w` positions that a tilted plane would intersect.

### 5.2 Inputs that force a slice and render

Normal translation, any of the three plane-tilt components, and any simulated body motion that changes a body's intersection with `H` force a re-slice of the affected geometry and a scene render. Depth does not rescue these cases: it reconstructs 3D positions inside the old hyperplane, not geometry outside it. No guard band can contain missing fourth-dimensional samples, because a guard band widens the field of view within one slice.

This is the sharp cost of the design: the adopted claim that a missed scene frame costs world freshness but not mouse-look latency remains true for ordinary within-slice look (`docs/atw-first-rendering.md:56-65`), but it is false for slice tilt and phase motion. Those controls have at least slice-build plus scene-render latency, and when the scene controller runs at 30–40 Hz they update at that cadence unless the engine can afford an additional slice-and-render. Calling a 3D warp “4D ATW” would conceal rather than solve that boundary.

A frame carrying a different slice ID may still be displayed as a stale view, but it may not be warped toward the new hyperplane. The presenter either uses within-slice deltas relative to that frame's own basis or presents it unchanged until a matching slice arrives; it never blends topology between slice IDs. Cross-fading two completed slices is a possible aesthetic transition, not reprojection and not a latency guarantee.

The rejected alternative is to store a thick 4D slab or several neighboring slices in every `SceneFrame` so a late tilt can interpolate between them. A finite slab only postpones the same failure to its edge, multiplies slice, shading, and memory cost on every frame, and still cannot predict topology beyond the sampled planes. If future hardware can ray-cast the authoritative 4D representation at present time, that is a new renderer and may earn a new policy; it is not an extension of the current image warp.

### 5.3 Consequence for the input latch

The input latch keeps two consumers but narrows the presenter's contract. Its never-reset total and frame-relative mark remain the correct way to prevent starvation and double application (`docs/input-latch.md:50-83`), yet the late read returns only presentation-safe within-slice view deltas. Phase and slice-tilt totals have simulation marks only; consuming them changes authoritative state and therefore cannot be the cosmetic second read whose safety depends on never reaching anything sent, stored, or ticked (`docs/input-latch.md:106-112`).

The proposed `SceneFrame` contract consequently gains `slice_id`, `slice_normal`, `slice_offset`, and `slice_basis` beside its pose, projection, simulation time, sequence, and input mark; the existing proposed contract already requires the rendered pose and its frame-relative input baseline to travel on the frame (`docs/presenter-architecture.md:26-44`, `docs/presenter-architecture.md:62-67`). At warp encode, the presenter verifies that the requested correction preserves the stamped plane before applying it. A failed check is a counted `reslice_required` event and an unchanged presentation, never a best-effort 3D approximation to a 4D tilt.

Input arriving between a scene submission and present therefore has two honest latency classes. Within-slice look may move the displayed view immediately through ATW; phase or tilt waits for the next fixed tick, slice build, and scene completion. The UI should expose that distinction during development, because one “camera latency” number would average together a fast path and a categorically non-warpable path.

## 6. Physics is four-dimensional

Physics consumes the authoritative 4D solids and their 4D poses. The visible slice is neither a collider nor a broad-phase proxy: two bodies may collide outside the current hyperplane, and a body that has no visible cross-section may still carry momentum into it on a later tick. The current project already defines its authority as a pure deterministic fixed-60-Hz simulation (`crates/pong-core/src/shooter.rs:1-5`); this design preserves that clock while replacing every spatial assumption underneath it.

### 6.1 Orientation and angular velocity

`SO(4)` has dimension `4(4-1)/2 = 6`, not three. Its infinitesimal rotations form `so(4)`, the six-dimensional vector space of 4×4 skew-symmetric matrices, equivalently the bivectors `Λ²(R^4)` with plane coordinates `(xy, xz, xw, yz, yw, zw)`. Angular velocity is one of these bivectors: it states instantaneous rotation rates in coordinate planes, and a generic value is a simultaneous double rotation rather than a vector pointing along an axis. This is the dimension-independent formulation derived in [Leyvraz's rigid-body treatment](https://arxiv.org/abs/1407.8155).

The proposed computational orientation is a Spin(4) pair of unit quaternions `(q_L, q_R)`. Identify a 4-vector with a quaternion `x` and act by `x' = q_L x inverse(q_R)`; every proper 4D rotation has such a pair, and `(q_L,q_R)` and `(-q_L,-q_R)` are the same rotation, as proved by the [4D quaternion representation theorem](https://arxiv.org/abs/math/0501249). This stores eight scalars subject to two unit constraints and one shared sign equivalence, exactly the required six continuous degrees of freedom.

Composition is componentwise with order preserved: applying `b` and then `a` yields `(a_L b_L, a_R b_R)`. Each fixed step converts the six body-space angular-velocity components through one pinned self-dual/anti-self-dual basis into imaginary quaternion rates `(u_L,u_R)`, forms `(exp(dt u_L/2), exp(dt u_R/2))`, and multiplies that increment on the body side; world-space rates use the opposite side. The basis signs, multiplication side, and an explicit noncommuting test case are part of the math contract, because identity-only tests cannot catch reversed composition.

Both quaternions are normalized independently after every integration step with the specified deterministic reciprocal-square-root routine. Serialization and state hashing then choose the shared-sign representative whose first nonzero component in `(q_L,q_R)` is positive; flipping only one quaternion is forbidden because it represents a different rotation. A zero or non-finite norm is a simulation fault, not an identity fallback.

The rejected runtime representation is a 4×4 orientation matrix. It carries sixteen scalars for six freedoms, ordinary integration loses orthogonality, and Gram–Schmidt or polar repair introduces ordering and platform-dependent branch choices. A general Clifford rotor is mathematically sound but has the same eight even components and more machinery than the quaternion pair; Spin(4)'s product structure gives the smaller implementation surface.

### 6.2 Inertia and angular momentum

The three-dimensional relation between angular-velocity and angular-momentum vectors relies on a dimension-specific duality. In four dimensions both quantities remain six-component bivectors, so inertia is a linear operator `I: Λ²(R^4) → Λ²(R^4)`, represented computationally as a symmetric positive-definite 6×6 body-frame matrix. In tensor notation it is rank four; [Parker's arbitrary-dimensional derivation](https://arxiv.org/abs/2302.04092) verifies that it maps bivectors to bivectors and is determined by the body's rank-two mass second moment rather than by 21 arbitrary matrix parameters.

Choose plane generators `E_ij` that rotate basis vector `i` toward `j`, write `Ω = Σ ω_ij E_ij`, and derive `I` from `T_rot = 1/2 ∫ |Ωr|² dm`. The momentum relation is `ell = I omega` and the energy check is `T_rot = 1/2 transpose(omega) I omega`. In principal mass axes, if `s_i = ∫ r_i² dm`, the six diagonal plane moments are `s_i + s_j`; off-diagonal terms vanish. The cooker integrates those moments over the 4-simplex fill and the runtime may store the resulting full 6×6 matrix for fast solves, but validation rejects a matrix that is asymmetric, non-positive, or inconsistent with the cooked mass distribution.

World-space inertia is not obtained by treating `omega` as a 4-vector. The orientation induces a 6×6 bivector transform `Λ²R`; therefore `I_world = (Λ²R) I_body transpose(Λ²R)` and its inverse transforms the same way. A force `f ∈ R^4` applied at center-relative point `r ∈ R^4` produces the six-component torque `r ∧ f`, and the angular-momentum update stays in that space.

The implementation stores angular momentum as the dynamic state and computes `omega = inverse(I_world) ell` for integration. This survives a changing orientation without pretending that angular velocity is conserved, gives impulses an additive momentum target, and makes rollback snapshots contain the quantity external torque actually changes.

### 6.3 Collision and contact geometry

Broad phase becomes four-dimensional AABB overlap with sweep-and-prune or a 4D BVH over all four spatial axes. Narrow phase operates on the cooked convex 4D decomposition: the Gilbert–Johnson–Keerthi distance algorithm was formulated for convex sets in `R^m` ([original paper](https://doi.org/10.1109/56.2083)), so its support-map loop carries over, but its working simplex may now contain five points. Overlap recovery uses a proposed 4D EPA whose expanding hull has tetrahedral boundary facets, not the triangular facets of 3D EPA; this is a new robustness burden and must be tested independently rather than described as a type-width change.

Two 4D solids occupy four-volume and their boundaries are three-manifolds. At smooth, nonpenetrating generic first contact they touch at a point; after generic interpenetration the two boundary hypersurfaces intersect in a two-dimensional patch because `3 + 3 - 4 = 2`; coincident supporting hyperfacets can share a three-dimensional contact region. Polytope feature pairs can also yield line or surface intermediates, so a contact manifold cannot be modeled as “the one 3D contact point with a normal.”

After EPA supplies a separating normal and witness features, manifold generation clips the two support features inside their common three-dimensional contact hyperplane. It reduces the resulting 0D-to-3D region to a deterministic bounded set of contact points chosen by stable feature ID and extremal coverage, retaining enough points to resist torque across the patch. Each point has one 4D normal and a three-dimensional tangent space for friction, one more friction freedom than a 3D contact.

Continuous collision remains parameterized by separate simulation time: conservative advancement asks the 4D distance query along the bodies' time-parametric poses and finds a time of impact within the fixed tick. It does not promote time into the world coordinates. Fast bodies cannot rely on a discrete overlap test merely because the extra spatial dimension makes broad phase more expensive.

The rejected collision design is to collide the rendered 3D slices. It misses off-slice impacts, changes collision results when the player changes viewpoint, turns a render LOD into gameplay authority, and can make an invisible body pass through a visible one before either cross-section overlaps.

### 6.4 Constraints, impulses, and determinism

A rigid body's generalized velocity has ten components: four linear plus six angular. For a scalar normal constraint at contact offset `r` with unit normal `n_c`, the body Jacobian is `[transpose(n_c), components(r ∧ n_c)]`; the second body's block is negated. The familiar 3D `r × n` term has not gained one coordinate—it has become a six-component bivector term—and the generalized inverse mass is block diagonal with `inverse(m) I_4` and `inverse(I_world)`.

The sequential-impulse equation and warm starting survive in form: solve `lambda = -(Jv + bias)/(J M^-1 transpose(J))`, clamp it to the constraint's admissible set, apply linear impulse `lambda n_c`, and apply angular impulse `lambda(r ∧ n_c)`. What changes is row width and constraint count. A point-to-point joint supplies four scalar positional rows rather than three; locking relative orientation supplies six rotational rows rather than three; a true one-parameter hinge must name its permitted rotation plane and lock the other five; and point-contact Coulomb friction solves in a three-dimensional tangent ball rather than a two-dimensional disk.

Island building, shock propagation, and solver iteration count remain policy, not dimension-specific mathematics. The decision is a fixed-count projected Gauss–Seidel solver ordered by stable body-pair, feature, and constraint IDs, with no “iterate until converged” early exit in authoritative simulation. Contact reduction and warm-start caches use ordered arrays, never hash iteration order, and rollback stores or deterministically reconstructs every cached impulse that can affect the next tick.

Four-dimensional physics magnifies the existing threats to determinism: GJK/EPA feature ties, near-zero predicates, contact-patch reduction, two-quaternion sign choice, normalization, parallel reductions, fused multiply-add differences, transcendental implementations, and solver ordering. The proposed authoritative scalar path uses strict IEEE-754 operations with contraction disabled, pinned software implementations for reciprocal square root and quaternion exponential, exact/adaptive signs for branch-sensitive geometry, fixed iteration counts, and canonical NaN rejection; GPU results never enter simulation. Assets are quantized and assigned stable topology IDs at cook time, while runtime state is quantized to its wire grid only after the solve, not before collision.

Determinism is a gate, not an aspiration. A seeded replay records fixed-tick inputs and hashes position, Spin(4) representative, linear momentum, angular momentum, contact IDs, and warm-start impulses after every tick; native and wasm must match bit for bit, and rollback must restore tick `k`, replay to `k+n`, and reproduce the uninterrupted hashes. Until that cross-target oracle is green, the 4D solver may run in shadow or server-authoritative snapshot mode but may not replace the project's replayable authority.

## 7. Integration with the architecture corpus

### 7.1 The five state levels

The five-level model remains five levels. It defines level 1 as the abstract scene, level 2 as sent and received wire encodings, level 3 as local simulation/prediction, level 4 as the sampled state consumed for rendering, and level 5 as post-warp presentation (`docs/state-model.md:13-27`). Four-dimensionality changes the contents and the 3→4 transition, not the number of authorities.

Level 1 now says that positions, shapes, relationships, and physical laws are four-dimensional. Level 2 encodes 4D intent and authoritative 4D poses without encoding a client-specific slice. Level 3 owns the full 4D local or server world, including Spin(4) orientation, four-velocity, angular momentum bivector, collision caches, and fixed-tick identity. A client may still route unpredicted remote level-2b entities straight toward rendering—the existing model already permits a 2b→4 sampling bypass (`docs/state-model.md:46-56`)—but it must decode enough 4D state to slice them correctly.

Level 4 changes meaning from “a camera plus ready 3D instances” to “a slice-derived render sample.” It includes the transient 3D geometry, exact slice plane and basis, source simulation tick, slice ID, camera/projection, material state, input mark, and the color/depth `SceneFrame` produced from them. The slice itself is not level 3½ or a sixth level because it has no independent authority: it is a lossy, viewpoint-dependent sampling inside the 3→4 transition, and it may be discarded and rebuilt from the same level-3 world.

Level 5 remains presentation and may lose freshness but not meaning. It can reproject a level-4 frame only within that frame's stamped hyperplane; it cannot reinterpret the frame as a different slice. Thus the state model's rule that sampling may lose freshness but not authority (`docs/state-model.md:58-64`) becomes a runtime slice-ID check rather than prose alone.

The level-4 identity already proposed as `sim_time`, `seq`, and `input_mark` (`docs/state-model.md:29-40`) grows a slice component rather than a new clock. The tuple `(world_tick, slice_id, scene_seq)` answers which authority was sampled, which hyperplane sampled it, and which rendered result was produced; `present_seq` continues to identify repeated level-5 presentations of that result.

### 7.2 Latency taxonomy and adaptive control

The local latency taxonomy gains `X0 slice_build` between `I2 sim_done` and `R0 scene_encode`; the existing taxonomy currently moves directly from simulation completion to scene encoding (`docs/latency-observability.md:150-165`). `X0` records CPU begin/end, world tick, slice ID, `N_q`, `C`, `A`, emitted vertices/triangles, cache hits, exact-zero and degenerate counts, and bytes uploaded. If a future GPU slicer lands, its query-pair duration is `GX0 slice_gpu`, while CPU dispatch and compaction remain `X0`; CPU and GPU lanes are displayed rather than summed.

The controller gains one primary new quality actuator: proposed `slice_error_px` with the discrete ladder `0.5, 1, 2, 4`. Assets cook nested, closed boundary complexes with a certified maximum object-space deviation; at slice build, projection converts that bound to pixels and selects the coarsest level no worse than the requested value. A level changes only on a slice rebuild, keeps physics and network geometry untouched, and uses the controller's existing one-lever-at-a-time, disturbed-epoch, and hysteresis rules rather than inventing a second controller; the current actuator contract already requires discrete reproducible values and one settled change at a time (`docs/dynamic-scene-timing-and-scaling.md:62-72`).

Sensor attribution determines lever order. If `X0` p95 or slice upload p95 breaks budget while `G0 scene_gpu` remains healthy, increase `slice_error_px`; lowering `scene_scale` cannot reduce candidate cells or CPU intersections. If `G0` is the pressure, keep slice topology and lower `scene_scale`; if both are high, change one lever, wait for attribution, then consider the other. Scene-Hz reduction lowers slice work per second only when the plane or bodies change, but it also directly increases non-warpable phase/tilt latency, so it remains the emergency lever after slice LOD and resolution rather than a free win.

This revises the weak-device floor honestly. The existing assessment treats pixels and memory bandwidth as the dominant present cost and ranks `scene_scale` first (`docs/weak-device-performance.md:15-35`, `docs/weak-device-performance.md:175-185`); 4D-first adds an `O(N_q+C)` CPU stage and topology upload that resolution scaling does not touch. A target is supported only if its minimum cooked slice LOD stays under declared `X0` and upload budgets in the worst authored scene; otherwise the engine must reject that content or the target, not silently punch holes by dropping active cells.

The performance corpus therefore needs later amendments, not edits in this lane: add `X0/GX0` distributions and slice counters to `latency-observability.md`; add `slice_error_px` and non-warpable slice-latency bounds to `dynamic-scene-timing-and-scaling.md`; and add active-boundary-cell, CPU slice, upload, and slice-cache budgets to `weak-device-performance.md`. Presentation cadence, fixed simulation rate, network cadence, and input sampling remain outside controller authority, consistent with the current controller's protected set (`docs/dynamic-scene-timing-and-scaling.md:120-130`).

### 7.3 Wire representation and quantization

The current protocols do not contain a full 3D rigid pose to widen mechanically. The postcard `PlayerState` carries only two-component position and velocity (`crates/ember-net/src/lib.rs:43-48`), while the WebSocket `PState` carries three position scalars and horizontal aim plus pitch rather than a quaternion (`crates/pong-core/src/proto.rs:42-59`). The following costs are therefore the proposed engine pose codec, not a claim about the byte size of either current message.

Raw `f32` comparison is unambiguous: a 3D position plus unit quaternion is seven scalars or 28 bytes, while a 4D position plus Spin(4) quaternion pair is twelve scalars or 48 bytes. The 4D pose costs 20 additional bytes, a 71% increase, before entity ID, velocity, tick, or framing.

The proposed network encoding is 25 bytes per pose versus a comparable 16-byte 3D pose. Position uses signed 24-bit millimeters relative to a sector origin—9 bytes for 3D, 12 for 4D, with ±8.39 km range and at most 0.5 mm error per component. A 3D unit quaternion uses smallest-three encoding with three signed 16-bit values and one metadata byte, 7 bytes; the Spin(4) pair stores three values from each quaternion plus one byte containing both omitted-component indices and the second quaternion's relative omitted sign, 13 bytes. The pair's simultaneous sign equivalence makes the first omitted component positive; the relative sign is necessary because flipping only one quaternion changes the rotation.

Sixteen-bit smallest-three components have a maximum stored-component error of approximately `sqrt(2)/(2×65534) = 1.08e-5`. Propagating that through reconstruction and both quaternion actions gives a conservative proposed 4D orientation-error acceptance bound of 0.02 degrees; an exhaustive boundary/corner test of the codec must prove the bound, and failure raises the component width rather than relaxing physics tolerances. Position sectors rebase only at snapshot boundaries, and the full-precision level-3 state is never overwritten by its wire quantization.

On decode, normalize both reconstructed quaternions, reject impossible radicands beyond the quantizer's rounding allowance, and choose between the received pair and its simultaneous negation to minimize distance from the previous decoded pair before interpolation. That continuity lift avoids a canonical-sign boundary turning a small physical rotation into a long interpolation path. Linear and angular momentum need separate range-derived codecs; their widths cannot be chosen honestly until gameplay force and speed bounds exist.

## 8. Honest costs and failure modes

Authoring is the first existential risk. Conventional DCC tools produce triangle surfaces in 3D, while this design needs tetrahedral boundary cells around a 4D volume, three-component boundary texture coordinates, adjacency, and validated 4-simplex fills. Extrusion preserves a path for closed existing assets, but it turns open planes, nonmanifold meshes, loose clothing, particle cards, and alpha-cut foliage into repair problems; a cooker that silently seals them would create false mass and collision.

The in-tree editor does not soften that gap merely by existing. It is a real separate workspace crate (`Cargo.toml:1-12`), but its camera and object model use 3D vectors (`crates/ember-editor/src/lib.rs:26-27`, `crates/ember-editor/src/lib.rs:254-264`) and its own module header says exported levels are not yet loaded by the running arena (`crates/ember-editor/src/lib.rs:1-11`). The chosen migration keeps it useful as a slab-and-primitive editor, yet arbitrary 4D rotation gizmos, hypersurface selection, slice animation, 4D UV inspection, manifold repair, and a view of invisible neighboring geometry are substantial new product work, not another axis button.

Player comprehension is the second existential risk. Phase movement can make a wall shrink, split, merge, or disappear; a fully 4D body can collide and transfer momentum while absent from the current slice; and a slice tilt can change topology under a stationary 4D object. The engine should provide slice-distance bands, ghosted near-slice projections, contact-direction cues, and a locked reference grid as optional game tools, but no renderer feature proves that players will form a useful mental model. Controlled playtests must show that players can predict a phase route and explain an unseen collision; repeated guessing is a design failure even if the mathematics is correct.

The weak-device floor is worse than ordinary 3D. Even after scene resolution reaches its minimum, every changed plane must traverse a 4D acceleration structure, classify boundary cells, build topology, and upload triangles; the `A = 100,000` example already costs up to 18.4 MB per slice before rasterization. Four-dimensional BVHs, 4-simplex fills, convex decompositions, multiple certified LODs, and three-coordinate textures also raise memory and download costs. Some authored worlds will simply not fit a WebGL-class target, and the engine must publish content compatibility tiers rather than claim that dynamic resolution solves a CPU/topology bottleneck.

ATW loses part of its headline benefit. Ordinary look remains late-warpable, but the input that most clearly demonstrates the fourth dimension—phase and plane tilt—waits for a slice and scene render. If gameplay makes slice tilt the dominant camera gesture, the adopted low-latency architecture protects the secondary motion while the primary motion takes the long path; no wording can turn that into success.

Physics is a research-sized implementation risk. GJK's dimension-independent core is established, but a robust 4D EPA, deterministic feature clipping, reduction of 0D-to-3D contact regions, three-dimensional friction, and stable stacks under six rotational freedoms are not commodities in this tree. More contacts and a ten-component body velocity enlarge islands and solver cost, while the strict scalar path needed for native/wasm bit equality may be materially slower than hardware-default floating point.

Numerical topology will remain visible even with exact signs. The one-sided policy prevents cracks and flicker at equality, but a real hyperplane crossing a vertex or becoming tangent to a hypersurface genuinely changes the slice topology; coarse LOD can shift the tick at which that happens. Content must avoid dense near-coplanar tetrahedra, and transitions may need aesthetic cross-fades that explicitly trade geometric immediacy for readability.

The architecture is rejected for a target or game if any of four gates fails: the minimum slice LOD misses its `X0`/upload budget in representative worst scenes; native and wasm replay hashes diverge; authors cannot produce a validated interactive asset in an acceptable content budget; or players cannot predict slice navigation and collision above a declared test threshold. Keeping the 3D compatibility profile until those gates pass is not hedging the model—it is the only way to learn whether the model is shippable without trapping the engine behind a one-way rewrite.

## 9. Migration from the 3D engine

## 10. Architectural invariants
