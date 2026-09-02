# Minimum device requirements

Ember targets low-specification devices through one substrate, wgpu 24 on WebGL2, and this document is the floor that substrate is held to: what a device must expose for Ember to run, what Ember deliberately does not require, and what happens when the floor is missing.

## 1. The floor

WebGL2, that is OpenGL ES 3.0 semantics in the browser, is required; WebGL1 is not supported and no ES 2.0 path exists or is planned.

`EXT_color_buffer_float` is required: it makes `RGBA32F` and `RGBA16F` textures renderable as colour attachments, and Ember's DATA heap, its fragment-compute kernels and its GPU-resident geometry pipeline all render into `RGBA32F` array layers (`docs/gpu-resource-heap.md`, `docs/gpu-heap-lattice.md`); without the extension there is no kernel path, and a device without it is unsupported rather than degraded.

The decision rests on where the extension is missing: WebGL2 implementations on desktop GPUs, on Apple GPUs since the WebGL2 introduction in Safari, and on the Adreno, Mali and PowerVR generations that expose WebGL2 at all carry it; the devices that lack it are overwhelmingly devices that lack WebGL2 entirely, so requiring the extension excludes almost nobody the WebGL2 requirement did not already exclude.

The WebGL2 guaranteed minimums Ember relies on, and therefore never queries as if they were optional, are 4 colour attachments (the dialect's maximum of four kernel outputs is chosen to match), 16 KiB uniform blocks (1,024 descriptor records), 16 vertex-stage texture units, 256 texture-array layers and 2,048 texels of texture dimension; a device may expose more, and the runtime capability probe reports what it actually exposes, but the design must work at the minimums.

## 2. What is deliberately not required

`OES_texture_float_linear` is not required: every float heap is sampled nearest and unfiltered by contract, so linear filtering of float textures is never requested.

`EXT_float_blend` is not required: blending on `RGBA32F` targets is never requested, and wgpu already refuses it on this backend, so every compute colour target carries no blend state.

Timestamp queries are not required: all timing is wall time around a four-byte mapped fence, and a GPU timestamp series appears only as a separately labelled extra when the path exposes one.

WebGPU is not required and is not a target: portability on the WebGL2 floor is the project's purpose, and performance comes from the descriptor-heap architecture on that floor rather than from a newer API.

Shared-memory threads are not required: concurrency is Web Workers with message passing and ownership transfer, so no page depends on cross-origin isolation headers.

## 3. When the floor is missing

Initialization probes the adapter once and reports the exact missing capability as a typed refusal with the adapter string, the backend and the failed usage, in the page and in any submitted report; it never falls back to a silently different renderer, never retries a lesser format on its own, and never presents a blank canvas as a running game.

The probe result is a displayed fact beside every capacity wall a page computes, so a reader can tell a refused device from a refused workload.

## 4. How the floor is verified

The output-path golden spike in the heap lab (`web/labs/heap/spike.html`) is the standing conformance test for the floor: it renders into an `RGBA32F` array layer, copies the result into the heap, selects a dispatch header by dynamic uniform offset, loads the record from the vertex stage and demands an exact readback; a device that passes it meets this document.

Adding a requirement to this floor is a project decision and is recorded here first, with the feature that needs it and the devices it would exclude, before any code depends on it.
