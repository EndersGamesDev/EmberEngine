# Ember as a 4D-first engine

## 1. Decision and coordinate model

Ember's authoritative world is spatial four-space, `R^4`, with coordinates `(x, y, z, w)`. Time is not the fourth coordinate: it remains the separate parameter of a deterministic fixed-step 60 Hz simulation, while the player sees the three-dimensional hyperplane slice occupied by the current viewpoint. Reading the fourth coordinate as time would make “render a 3D slice” mean only “render the present,” which every real-time engine already does; it would add no representation, rendering, input, or physics decision. The spatial reading is therefore not an analogy layered over a 3D engine but the engine's defining model.

This decision sets the direction of authority: simulation owns 4D bodies, the slice stage derives transient 3D render geometry, and the rasterizer and presenter consume that derivation. A rendered slice may never become the collision world, the network authority, or the source from which the 4D state is reconstructed.

## 2. Authoritative 4D world representation

### 2.1 Boundary cells, connectivity, and solids

### 2.2 Authoring and generalized normals

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
