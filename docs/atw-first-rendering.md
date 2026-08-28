# ATW-first rendering

**Decision:** build the renderer around late-reprojection presentation from day
one. The scene renderer never touches the swapchain. A cheap *warp pass* owns
presentation: every displayed frame it samples input at the last possible
moment and reprojects the newest finished scene frame to the canvas, then
composites the UI. Camera latency becomes the cost of the warp pass (a
fraction of a millisecond), not the cost of rendering the scene.

This is the web-shaped version of VR asynchronous timewarp (ATW). Deciding it
now is cheap; retrofitting it later touches every layer (render targets, input
path, UI compositing, the render graph), which is why it goes in before the
first triangle.

## 1. What "ATW" can and cannot be in a browser

True ATW (Oculus/SteamVR) relies on a high-priority GPU context that
**preempts** in-flight rendering to warp the last frame right before scanout.
None of that machinery exists on the web:

- WebGPU exposes **exactly one queue**, no priorities, no preemption
  ([multi-queue is an open investigation](https://github.com/gpuweb/gpuweb/issues/1065),
  and wgpu tracks the WebGPU API, so native gets no extra queue either).
- Presentation goes through the browser compositor, effectively FIFO, and
  typically adds a frame. The low-latency
  [`desynchronized` canvas path](https://developer.chrome.com/blog/desynchronized)
  is a WebGL/2D affordance and is not available to WebGPU canvases.

So what we build is **cooperative asynchronous reprojection**: async at
*submission* granularity instead of preemption granularity. All the
architectural consequences are identical to real ATW, and the latency win —
sampling the pose ~0.5 ms before present instead of 1–3 scene frames before —
is preserved. Only the worst-case guarantee is weaker (see §5, slicing).

## 2. The pipeline: three clocks

| Clock | Rate | Produces |
|---|---|---|
| **Sim** | fixed 60 Hz, deterministic (pillar #3; matches server `TICK_HZ`) | game state |
| **Scene** | variable, budgeted (target ≥ 40 Hz, floor ~30) | `SceneFrame` |
| **Present** | display rate, driven by rAF | swapchain image |

```
input latch ──────────────────────────────┐ (read at warp encode)
                                          ▼
sim 60Hz ──► scene render (30–72Hz) ──► SceneFrame ring ──► warp + UI ──► canvas
             pose P_scene (predicted)    {color, depth,      every rAF,
                                          P_scene, proj,     pose P_now
                                          timestamp}
```

`SceneFrame` = post-tonemap LDR color + depth + the exact pose/projection it
was rendered with + sim timestamp. Ring of 2–3 (renderer writes one, presenter
reads the newest complete one).

**Why it fits ember specifically:**

- *Web frame variance.* GC pauses, tab compositing, thermal throttling, and
  Intel iGPUs make scene-rate hitches unavoidable. With ATW-first, a missed
  scene frame costs visual freshness of the world, not mouse-look latency —
  the camera stays glued to the mouse.
- *Frame-rate amplification.* Weak client renders the scene at 30–40 Hz, warp
  fills to 60/120. Combined with dynamic resolution on the SceneFrame (free —
  the warp resamples anyway), this is the main lever for "runs on whatever
  laptop opens the tab".
- *Multiplayer.* Remote state arrives as 60 Hz snapshots and is interpolated
  ~1–2 ticks in the past regardless of what we do; the thing the player
  *feels* is their own camera. ATW puts the camera at ~1 display frame of
  latency independent of scene cost and of the network.
- *GPU-driven pillar.* "The CPU hands over a camera" splits cleanly into a
  predicted camera at scene submit and a corrected camera at warp.

## 3. The warp ladder (built in stages)

- **A. Identity blit** — offscreen render + fullscreen blit to canvas.
  Establishes the architecture at zero risk. Lands with the *first triangle*.
- **B. Rotation-only warp** — apply `R_now · R_scene⁻¹` as a homography on the
  frame (depth-free, no disocclusions). Mouse-look is the highest-frequency,
  most latency-critical signal, and rotation-only handles it exactly. Needs a
  **guard band**: render the scene with a slightly widened FOV (~+7°) so edge
  reveal doesn't show black. Lands with the fly camera.
- **C. Depth-aware reprojection** — reproject with the full pose delta using
  the depth buffer, which handles translation (WASD/strafe). Implementation:
  warp a coarse grid mesh (~64×64) displaced by depth — the classic robust
  form, cheaper and better-behaved than per-pixel scatter. Disocclusion holes
  get skirt-stretch fill. Lands once the depth buffer exists (roadmap step 3).
- **D. Motion-vector extrapolation** (ASW-style, animated objects) — noted for
  completeness; not planned. Animated objects simply update at scene rate.

## 4. Architectural consequences — why this is decided now

1. **Layering gains a stage.** `game → sim → scene renderer → presenter →
   platform`. The presenter owns the surface; `renderer.rs`'s output contract
   is a `SceneFrame`, never the swapchain texture.
2. **Late input latch.** Raw input accumulates in a latch that is read at
   *warp-encode time*. The sim consumes input at fixed ticks as before —
   determinism is untouched because the warp is cosmetic; the sim never sees
   the late pose.
3. **Two camera reads.** The sim/net camera (what determinism and the server
   see) and the view camera (evaluated at warp time). Same source, two read
   points — the split must exist in the API from the start.
4. **UI composites in the presenter, never in the scene pass.** Anything drawn
   into the SceneFrame swims when warped. Crosshair/HUD/egui (roadmap step 5)
   are presenter-side. This is the single most common retrofit pain.
5. **Guard band from day one.** Projection and culling code must handle
   oversized/asymmetric frusta (also later useful for shadow cascades).
6. **Depth is a first-class sampled texture.** Never discarded at end of
   frame; needed by stage C and by SSAO/TAA later anyway.
7. **Tonemap inside the scene pass.** Warp LDR — cheap, no HDR history
   buffers. MSAA (if any) resolves before warp. TAA, if it ever lands, shares
   the motion-vector machinery of stage D.
8. **Sliced submissions.** The one real "async" constraint: on a single queue,
   a monolithic 20 ms scene submission blocks the warp submitted behind it.
   Scene work must go to the queue as multiple chunky command buffers
   (per-pass), so the rAF warp slots in within a slice, not a frame. This
   shapes the render graph (step 7): nodes → separate submissions, not one
   megabuffer.
9. **Dynamic resolution is free.** The SceneFrame can be any size; the warp
   resamples to the canvas. Scale to hold scene Hz.
10. **Debug hooks early.** The step-5 overlay needs a *scene-Hz throttle*
    slider and a latency/timing HUD, so warp artifacts are exercised
    continuously instead of discovered on a slow machine.

## 5. Honest costs

- One fullscreen pass per displayed frame (~0.2–0.5 ms on weak GPUs) plus 2–3
  color+depth targets of memory.
- Artifacts under stress: disocclusion smears when strafing fast at low scene
  Hz; particles/transparents warp as opaque-at-their-depth; animated objects
  judder below ~40 Hz scene rate. Mitigation is budget (keep scene ≥ 40 Hz)
  — warp quality degrades gracefully rather than the whole frame missing.
- Not true ATW: one pathological draw can still delay the warp by a whole
  slice. Slicing (§4.8) and dynamic resolution bound this in practice.
- Slightly more machinery before the first pretty screenshot. Contained: the
  complexity lives entirely in the presenter + input latch; everything above
  the renderer is unaffected.

## 6. Roadmap deltas

- **Step 1:** first triangle renders offscreen through the identity presenter
  (stage A). Fly camera lands as rotation-only warp (stage B).
- **Step 3:** depth buffer → depth-aware reprojection (stage C).
- **Step 5:** egui drawn presenter-side; add scene-Hz throttle + timing HUD.
- **Step 7:** render graph emits sliced submissions (required by §4.8).

## Prior art

- J. Carmack, *Latency Mitigation Strategies* (the original timewarp writeup)
- Oculus, *Asynchronous Timewarp Examined*; Valve, SteamVR Asynchronous
  Reprojection; A. Vlachos, *Advanced VR Rendering* (GDC 2015 — guard bands)
- Comrade Stinger's desktop "async reprojection" demo (2022) — the same idea
  applied to a flat-screen FPS, and the best visual demo of why this works
- [gpuweb#1065](https://github.com/gpuweb/gpuweb/issues/1065) — WebGPU
  multi-queue status
