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

### 3.1 Exact cell-to-tetrahedra algorithm

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

## 5. The hard limit of asynchronous timewarp

### 5.1 Inputs that remain warpable

### 5.2 Inputs that force a slice and render

### 5.3 Consequence for the input latch

## 6. Physics is four-dimensional

### 6.1 Orientation and angular velocity

### 6.2 Inertia and angular momentum

### 6.3 Collision and contact geometry

### 6.4 Constraints, impulses, and determinism

## 7. Integration with the architecture corpus

### 7.1 The five state levels

### 7.2 Latency taxonomy and adaptive control

### 7.3 Wire representation and quantization

## 8. Honest costs and failure modes

## 9. Migration from the 3D engine

## 10. Architectural invariants
