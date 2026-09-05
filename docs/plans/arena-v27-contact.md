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

Implemented and verified; publication and public artifact verification follow the gates below. Client-only protocol 20: no live server restart. The original checkout's seven pre-existing modifications remain untouched. Work is coordinated on `codex/contact-shading` and Barza; no Fable acknowledgement or PR review is claimed.

## Measured browser, visual and cost evidence

The local evidence is in `target/contact-{baseline,first,baseline-repeat,repeat,release}/results.json` and `target/contact-release-old-maps/browser-results.json` (the older harness uses a different report filename). Both timing pairs used the same three viewpoints, 1600×900, one headless Edge browser at Idle priority, WebGL2 through ANGLE/D3D11 on Intel UHD Graphics 770, and protocol 20. Every Harbor checkpoint and measured callback batch had 101 draw calls; all reports passed their rendering assertions with no reported errors. The v26 baseline WASM was 43,161,225 bytes, SHA-256 `73f598b3ed56ec3e33c0e18c10a0d6d8e63176b21ec9c9128c60097ec1841efd`; the v27 candidate used in both timing runs and final Harbor captures was 43,188,044 bytes, SHA-256 `2a224c0bea9747a4e5003acc931a93a68d63c931f5dff604c80a15317e0aab80` (+26,819 bytes).

### Synchronized frame latency

Each view drained the GPU, warmed eight frames, then recorded 32 samples using the original wall clock immediately before fixture RAF callbacks through `gl.finish()` afterward. This measures synchronized update/submission/GPU-completion latency, including CPU/driver scheduling; it is not GPU-only timing or FPS. RAF pacing, snapshot injection, screenshots and browser-control transport are outside the measured interval. Serial completion changes normal pipelining. Median averages the middle two samples; p95 uses nearest rank.

| Pair | View | v26 median / p95 (ms) | v27 median / p95 (ms) | Median increase (ms) |
|---|---|---:|---:|---:|
| First | Central stacks | 0.90 / 2.30 | 2.35 / 9.00 | +1.45 |
| First | Warehouse | 1.20 / 1.90 | 2.35 / 9.90 | +1.15 |
| First | Quay / ship | 1.10 / 3.10 | 2.60 / 9.40 | +1.50 |
| Repeat | Central stacks | 1.15 / 2.10 | 1.30 / 2.00 | +0.15 |
| Repeat | Warehouse | 1.10 / 1.60 | 1.30 / 2.70 | +0.20 |
| Repeat | Quay / ship | 1.10 / 1.30 | 1.50 / 2.20 | +0.40 |

The first pair showed substantial measured overhead and long tails; it is not discarded because the repeat was better. The unchanged candidate binary then showed smaller median increases, but only two runs on one shared-desktop GPU cannot separate shader cost from scheduling, driver warmup and GPU-clock variance or certify the minimum supported device. The repeat supports provisional acceptance of a small steady-frame cost on this machine, not a zero-cost claim or a guaranteed frame budget. Full harness wall times were 7.702 s baseline / 8.238 s first candidate and 7.747 s baseline-repeat / 8.034 s candidate-repeat; these include startup and capture work and are not rendering benchmarks.

### Join bake and memory

Actual startup logs recorded the following CPU bake times and total bytes across the two RGBA8 volumes. Initial baking happens before the harness freezes its presentation clock, so these are real wall-time measurements. The cache reuses identical geometry within the client; switching to different geometry can bake again.

| Map | Boxes / volume dimensions | CPU bake (ms) | Retained CPU volume bytes | Nominal GPU volume bytes | CPU + GPU payload |
|---|---|---:|---:|---:|---:|
| Harbor | 72 / 201×17×201 | 198.1 | 5,494,536 | 5,494,536 | 10.48 MiB |
| Freight Yard | 88 / 105×17×105 | 75.9 | 1,499,400 | 1,499,400 | 2.86 MiB |
| Trench City | 90 / 105×17×105 | 88.1 | 1,499,400 | 1,499,400 | 2.86 MiB |

These figures describe the active map's retained CPU arrays and the equal nominal texture payload, not measured peak process RAM/VRAM; driver allocation overhead, upload staging and transient bake bins are excluded. Frames share the immutable CPU field rather than copying its bytes. The roughly 0.2 s Harbor join bake is an explicit remaining startup hitch, not hidden inside the steady-frame median or claimed solved by caching.

Startup frame-gap warnings were 105 ms in `contact-first`, 199 ms in `contact-repeat`, 114 ms in `contact-baseline-repeat`, 203 ms in the final Harbor capture and 158 ms in Trench City's old-map smoke; the first baseline and Freight Yard smoke had none. These warnings remain recorded even where rendering assertions passed. They are separate from the measured per-frame samples, and not every warning is attributed to the bake.

### Visual coverage and verification limits

`target/contact-release` contains all 15 Harbor views and the contact sheet, completed in 21.094 s with normal shadows, no diagnostic shader override and WebGL error zero at every checkpoint. Full-resolution central-stack and warehouse views were compared with the v26 baseline: container/box feet and wall/roof junctions gained localized darkening while open surfaces and the warehouse route remained readable. The complete sheet retained the ship/crane silhouettes, distant coastline and existing direct cast shadows. Small shadow-map contact-edge aliasing remains visible; this feature does not replace or claim to fix the shadow map.

The old-map smoke completed in 12.422 s: Freight Yard and Trench City each passed, rendered two differing frames, reported protocol 20 on the same WebGL2 adapter, and had no errors. Both initial images were inspected for container/wall contacts, readable geometry and intact first-person weapons. The older report's `drawCalls` values are cumulative, not per-frame counts; do not compare them to Harbor's 101-draw checkpoint metric. This is bounded visual/runtime coverage, not eight-player networking or a complete route/collision test.

The coordinating worker separately ran the shipping-pipeline GPU pixel gate: 11/11 passed in 5.40 s, covering exact controlled-frame invariants unavailable from separate browser startups. No full global-illumination, dynamic player contact-shadow, minimum-device performance or zero-hitch claim is made.

## Final code and release gates — 2026-09-05

The integrated native suite passed 717 tests across Arena/core/server, engine, editor, Fire, Kings, the diagnostic game and Julibrot in 79.18 s (16 opt-in tests ignored). An early compile caught a missing GPU-test import; the first full execution also caught the scene-uniform size assertion still expecting 336 bytes instead of the new 368. Both were corrected, with the old fog offsets retained and the appended neutral-field bytes checked explicitly.

Strict all-target Clippy passed in 2.11 s after test-helper lint cleanup. The production baker retains its tested non-fused arithmetic instead of introducing software-emulated fused multiply/add calls into the WASM hot loops. Engine tests then passed again (51 tests, 6.25 s), followed by all 11 explicit GPU checks again (0.77 s). These include six new AO tests: open-ground/outside-field neutrality, contact darkening on both material paths, both sides of a thin wall with recessed faces, lifted-roof distance behavior, bypasses, and recovered linear direct-sun energy within RGBA8 quantization. Formatting and whitespace checks passed.

All five published release WASM bundles and bindings built initially in 10.38 s and finally in 7.08 s. Pages assembly fixtures passed 41/41 in 6.61 s and shell syntax 91/91 in 2.67 s. The read-only live server Welcome reported host `dusky-osprey`, protocol 20, r1397/00982ed; no restart is necessary. The v27 page/catalog/publisher keep v26 frozen and do not alter gameplay, movement or collision.

The final rebuild after lint/test cleanup produced a different fingerprint despite unchanged byte length, so the previous browser-tested hash was not accepted as proof of the final artifact. The shipping Arena is 43,188,044 bytes, SHA-256 `ef592c9298a96a3ccda8b8f32090ab1b5b925b85df94d649c629ea951a22e71c`. All 15 Harbor views were rerun with this exact binary in `target/contact-final` (29.409 s): 101 draws throughout, no GL errors, eight warmups plus 32 completion-fenced timing samples per view. Final central stacks / warehouse / quay medians were 0.70 / 0.75 / 0.60 ms and p95 1.20 / 1.40 / 0.80 ms. This additional, unpaired run is not evidence of a performance improvement over v26; the large run-to-run variation and earlier slower pair remain relevant. Harbor baked in 201.4 ms and recorded one 104 ms startup gap warning.

Both old maps were also rerun with the shipping binary in `target/contact-final-old-maps` (12.032 s), passing with no errors: Freight baked in 76.0 ms with one 101 ms startup gap warning, and Trench City in 89.3 ms with no warning. Final warehouse and map captures were inspected again. Publication must use this final fingerprint, not the earlier candidate hash.
