# Arena v27: local ambient/contact shading

## Scope

Follow v26 with nearby static light blockage, making container feet, wall junctions and cover more grounded. Work starts from main `dae87d1` in isolated `codex/contact-shading`; preserve the original dirty checkout and the v26 build for comparison. Presentation only: no movement, simulation, map geometry or protocol change, and no live server restart. Root owns GPU integration/release, separate workers own the pure CPU baker, Arena integration and independent verification.

## Selected approach and limits

Precompute six directional diffuse visibility values on a small 3D grid from the actual map cover boxes, excluding gameplay loot. Trace 26 fixed symmetric directions with a 2.25 m radius and smooth distance falloff, cosine-weighted for the six axes; no randomness or baked sun direction. Spatial bins bound candidate work. Store the six linear values in two RGBA8 3D textures, no mipmaps. Harbor's padded 100×100 m footprint and -0.5–7.5 m height at 0.5 m spacing use 201×17×201 nodes, about 5.24 MiB. Fail closed on invalid bounds or capacity rather than allocate unbounded memory. This is local static ambient occlusion, not global illumination or dynamic player contact shadows. [Ambient accessibility background](https://developer.nvidia.com/gpugems/gpugems/part-iii-materials/chapter-17-ambient-occlusion).

Bake once after the authoritative level arrives, hold an immutable shared field in frames and upload only when its identity changes. The renderer samples two trilinear volumes for eligible outdoor geometry, blends directional channels with squared normal weights, and attenuates indirect ambient/fill and sky-reflection light only. Direct sun, shadow-map visibility, fog, sky, particles and shadow-disabled first-person/HUD geometry are not darkened. Sample outside the receiver by the normal-projected grid spacing plus 6 cm, and fade to neutral at field boundaries; interior probes remain dark to resist light leaking through walls. These guards require actual thin-wall, corrugation and raised-slab tests; they are not assumed sufficient by inspection.

No new scene pass, per-frame ray tracing, extra vertex tessellation, imported assets or optional WebGL extension. RGBA8 3D textures and ordinary filtering are within the existing WebGL2 floor. This is intentionally separate from detailed normal/material maps.

## Gates

Pure CPU tests for bounds/capacities, neutral space, directionality, occupied probes, distance falloff, box-order invariance and raised cover; actual production-pipeline GPU tests for junction darkening, no isolated-plane halos, unaffected sun/sky/HUD, thin walls and detached objects. Run the integrated native suite, strict Clippy, all five WASM builds and Pages fixtures.

Use fixed-Frame native pixel comparisons for exact invariants: the existing browser fixture starts its clock after asynchronous startup, so separate screenshots cannot prove sky equality. Inspect matched v26/v27 views of all three maps. Measure CPU bake wall time and synchronized actual WebGL2 frame latency with a GPU completion fence, not asynchronous command-submission timing or a claimed FPS. Report memory, draw count, median/p95 and desktop noise honestly. If quality or join/render cost is unacceptable, reduce scope or fix it before release; do not silently publish a regression.

## Status

Plan and interfaces selected. Implementation, execution, visual review and publication pending.
