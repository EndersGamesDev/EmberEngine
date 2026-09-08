# Fire Racer core source

`lib.rs` exposes six modules: `car` defines world-space velocity, steering, drift, boost, and the fixed tick; `track` builds and queries the closed Catmull–Rom racing line; and `castle` authors that line plus trackside props.

`sim` combines cars, lap tracking, countdown, walls, standings, and race completion; `ai` drives unoccupied cars through the same step function; `proto` defines Fire protocol 1 messages, limits, sanitization, and compatibility defaults.

In-module tests pin deterministic stepping, hostile-input bounds, real drift and boost behavior, track seam and arc length, lap anti-farming, circuit drivability, AI completion, race ordering, and exact JSON semantics.

Client prediction and server authority both consume these modules, so operation-order or default changes must follow the shared-simulation and protocol rules in [`../../../CLAUDE.md`](../../../CLAUDE.md).
