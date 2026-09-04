# The Julibrot lab — charter

Status: charter for the lab that prototypes the real toolchain; five slice documents under `docs/julibrot/` refine it, and the implementation follows those documents, not this one, wherever they are more specific.

## 1. Why this lab exists

The heap lattice (`docs/gpu-heap-lattice.md`) paid for the middle of the engine chain on the WebGL2 floor: descriptors, heaps, kernels landing GPU-resident data into geometry. This lab pays for the rest of the chain once, in its smallest real form, on one object that needs every stage: a worker producing data under forward pressure, a lock boundary with a versioned owner, two update rates, kernels over heap spans, geometry and projection, and cooperative re-projection that keeps the picture moving while the next frame computes.

It is a prototype of the toolchain, so every mechanism it builds is meant to be lifted into the engine; every mechanism it does not need is deliberately absent, and the absence is a design statement, not an omission.

## 2. The object

The Julibrot is the set of pairs `(z, c)` in ℂ², which is ℝ⁴, whose orbit under `z → z² + c` stays bounded; the Mandelbrot set is its slice at `z = 0`, every Julia set is its slice at a fixed `c`, and every other 2D plane through it is a hybrid between them.

The lab renders a 2D slice of the 4D Julibrot, chosen and positioned by an affine object pose in ℝ⁴; the escape value lifts that plane by height into ℝ⁵, and an independent affine 5D camera pose sees the lifted slice through the double perspective `d₅` and `d₄` and the 3D observer.

Nothing in the lab reads a clock for geometry and nothing selects a view mode: every degree of freedom is a slider, and a preset is a named row of slider values. Two boxes hold saved rows and one slider morphs between them; navigation uses a target click, plain-drag pan, Shift-drag box zoom, and a `scale` slider, never the wheel.

Arbitrary zoom means arbitrary: shallow zoom runs in `f32`; deep zoom uses perturbation, one reference orbit iterated in high precision on the CPU and per-pixel deltas iterated in `f32` on the GPU, with rebasing so glitches are corrected rather than hidden; zoom depth is displayed in decimal digits with the precision in use beside it.

Every refinement grid is screen-aligned: each grid pixel samples the inverse image of its own render-surface pixel centre through the accepted neutral-height view, so object rotation, camera rotation, scaling, tilt, and perspective change the sampled plane points while every completed scene paints the whole page-owned render surface.

## 3. The chain, and which slice owns which link

`AST → lock boundary → owner → descriptors → heaps → kernels → geo → projection → scene → re-projection → surface`

|Slice|Package|Owns|
|-----|-------|----|
|math|`crates/labs/julibrot/math`|the Julibrot algebra: object rotation `O`, camera rotation `Q`, plane basis, presets, `f32` escape reference, high-precision reference orbits, perturbation and rebasing theory, the navigation-drift and warp-accuracy oracles, the bignum choice|
|kernels|`crates/labs/julibrot/kernels`|the dialect v2 kernels over heap spans: the `f32` escape kernel and the perturbation kernel, their conformance against the math oracles, the scratch-copy landing of the escape grid|
|worker|`crates/labs/julibrot/worker`|the Web Worker producer of reference orbits, the ownership-transfer channel with its credit header, the versioned owner with its two drains, the same-thread lowering of the same channel|
|present|`crates/labs/julibrot/present`|the one ambient height-field scene fetching the escape grid by handle, the slider-driven 5D camera presentation, the warp pass that re-projects the last completed frame under the current zoom, object and camera, the hot ring|
|app|`crates/labs/julibrot/app` and `web/labs/julibrot/`|the GL-only device and surface, single surface ownership, the panic hook and the non-panicking error handler, counted fences, the progressive-refinement policy, controls, the facts overlay, page-contract tests, the versioned loader|

Each slice is a package with its own tests and its own example or page where one makes sense; `app` integrates the other four through the interfaces their documents pin, and no slice edits another slice's package.

## 4. Laws that bind every slice

- Substrate: wgpu 24 `Backends::GL` over WebGL2 only, at the project's established minimum device floor, nothing more.
- Per-frame CPU-to-GPU traffic is uniforms plus regional writes for changed data; the reference orbit changes only when the zoom centre moves far enough, and the escape grid never crosses back to the CPU except for measurement and conformance.
- Kernel outputs land in the heap through the paid scratch-copy path; the heap bind group's identities never change; a hot ring selected by dynamic offset carries zoom and rotation at refresh rate.
- No shared memory: the worker owns what it writes, transfers it, and reads the owner's credit from the buffer that comes back; the same-thread lowering of the channel is the cheapest case of the same abstraction, not a special mode.
- Honesty: requested versus delivered, walls computed from live limits and labelled apart from policies, zoom depth in digits, precision in use, orbit length, rebase and glitch counts, delivered resolution and iteration cap, warm-up frames labelled and excluded, polls counted, never a hang, never a number that was not measured, browser values `requires visible replay` until a visible replay supplies them.
- Every wasm entry installs a panic hook and replaces wgpu's fatal uncaptured-error handler before the first device call; a bare `unreachable` is a bug nobody can read.
- The CPU-math question is answered by measurement, not assumption: random-angle object/camera oracles measure orthonormality in `f64`, a warp-accuracy oracle checks inverse times forward against identity across the zoom range, and `faer` enters only if one of the hand-written `f64` cases fails.
- Renderer austerity, authority and the one-way layering are unchanged; nothing here authors gameplay truth.

## 5. Deliberately out of scope

The general resource DAG and petgraph, more than one world, the simulation tick, more than one heap class, shared-memory threads, WebGPU; each is a later decision that this lab's evidence informs and must not pre-empt.

## 6. Process

Each slice runs the same sequence the heap lattice ran: a slice document under `docs/julibrot/<slice>.md` written first and reviewed in writing, a refined document, then implementation in phases with a commit per phase, gates green at every stop; the five documents are reviewed together for interface consistency before any implementation begins, and the app slice's document is the integration contract the other four must satisfy.

Gates are the workspace's nine plus the release bundle of the app package; they run on the sanctioned server, never on the workstation.
