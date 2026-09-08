# Fire track measurement example

`track_stats.rs` constructs the shipping castle circuit and prints measured length, bounds, segment count, and width so authored claims about the course can be checked against the actual spline.

Track and client developers consume this diagnostic when changing control points, sampling, or scenery assumptions; it reports geometry but does not mutate assets or produce runtime data.

The example uses the same `castle::track` and `Track` queries as simulation and rendering, avoiding a second measurement implementation that could drift.

The fixed shared-track contract is documented in `src/track.rs`, with repository-wide simulation discipline in [`../../../CLAUDE.md`](../../../CLAUDE.md).
