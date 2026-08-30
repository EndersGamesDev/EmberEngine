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

### 3.2 Degeneracy and numerical policy

### 3.3 Execution placement and cost model

### 3.4 Attribute transport

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
