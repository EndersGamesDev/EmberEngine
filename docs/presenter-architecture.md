# The presenter architecture

*Provenance: a working four-module extraction of this design was built and gate-proven on the v6-era tree and is preserved at `archive/v6-stack` (commit `a4dd8a7`); this document is that design restated against the current v7 renderer, not a record of it.*

## 1. The problem this solves

`crates/ember-engine/src/renderer.rs` is 1089 lines and owns the device, the queue, the surface, the surface configuration, the scene pass, the present pass, the shader hot-reload clock, the scene-rate throttle, and the egui compositing step. Its own doc comment (`renderer.rs:1-8`) describes a scene stage that "never touches the swapchain" and a presenter stage that "owns presentation", and the file then implements both inside one struct that holds `surface` and `scene` as sibling fields (`renderer.rs:164-202`).

The behaviour is right. The architecture is a comment. Every invariant the ATW document depends on is currently maintained by a reader agreeing with a doc comment rather than by a type or a module boundary, which means each invariant survives exactly as long as the next person to edit the file happens to read it. That is the whole cost being paid here: not that the pixels are wrong today, but that stage B, stage C, the offscreen readback lane, and presenter-side UI each become a retrofit of a file that has no seam to land on.

The fix is to make the seam real. Four modules with one-way dependencies, a value type that carries the renderer's output contract, and a presentation stage that is the only code in the engine that knows a swapchain exists.

## 2. Module boundaries

Four modules replace one, each owning exactly one thing:

- **`gpu.rs`** owns the `Device`, the `Queue`, and the `AdapterInfo` they came from. Both GPU stages borrow it; neither owns it. This is what lets the presenter be re-targeted (surface today, offscreen texture on a headless lane) and the scene pass be rebuilt without tearing down the device. Its constructor must also return the `Surface` and the `Adapter`, because adapter selection needs a compatible surface (`renderer.rs:222-229`) — the surface is born in `gpu.rs` and handed straight to the presenter, which owns it from that moment.
- **`scene_frame.rs`** owns the value types: `ViewPose`, `Projection`, `SceneFrame`, `Camera`, and the engine-wide near/far constants. This is the renderer's output contract, and it deliberately contains no pass logic — both GPU stages depend on it and neither depends on the other.
- **`renderer.rs`** owns the scene pass and nothing else: pipelines, mesh table, instance buffer, camera uniform. It holds no `Surface`, no `SurfaceConfiguration`, and no knowledge that a swapchain exists. Its entry point is `render_scene(&mut self, gpu: &Gpu, target: &mut SceneFrame, frame: &Frame)`.
- **`presenter.rs`** owns presentation: the `PresentTarget`, the surface configuration, the warp pipeline and its uniform, the sampler, the per-slot bind groups, and the UI composite. Its entry point is `present(&mut self, gpu: &Gpu, frame: &SceneFrame, view: &ViewPose)`.

The dependency graph is a tree, not a cycle: `renderer` and `presenter` both depend on `gpu` and `scene_frame`, and on nothing else in the engine. The single argument for files over fields inside one struct is that a field boundary permits the shortcut and a module boundary does not — `renderer.rs` cannot reach the surface if the surface is not in scope.

The engine's public re-export surface absorbs the move. Client crates import `ember_engine::Camera` (`crates/pong/src/online.rs:9`, `crates/game/src/main.rs:19`), not `ember_engine::renderer::Camera`, so moving `Camera` into `scene_frame.rs` and re-exporting it from `lib.rs` is invisible to all three shipped clients. This should be treated as a constraint on the split, not a happy accident: the split is only cheap while it stays behind the re-exports.

## 3. The `SceneFrame` value contract

A `SceneFrame` is a rendered image **plus the exact camera state it was rendered with**. The presenter's job is to warp that image against a *newer* pose, so it cannot reconstruct the older one for itself; anything the warp needs and cannot derive must travel on the frame.

```rust
pub struct SceneFrame {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub pose: ViewPose,
    pub proj: Projection,
    pub sim_time: f64,
    pub seq: u64,
}
```

Field by field, with the reason each one is on the frame rather than somewhere else:

- **`color`** is post-tonemap LDR (`Rgba8UnormSrgb`). The warp resamples it, so it must already be display-referred; warping HDR would require the presenter to own tonemapping and a history buffer, which is precisely the machinery the ATW document declines (`docs/atw-first-rendering.md:109-111`).
- **`depth`** is a first-class sampled texture, stored and never discarded, because stage C reprojects with the full pose delta using depth, and SSAO/TAA read it later (`docs/atw-first-rendering.md:107-108`). It exists on the frame even while stage A ignores it, because a depth buffer that is discarded at end of pass is an attachment configuration change, and configuration changes are where the invariant quietly dies.
- **The texture handles alongside the views** are retained because a `TextureView` cannot recover the `Texture` it came from, and the offscreen present target copies out of the texture itself. Stage A binds only the views, so the handles are dead code until the offscreen arm lands — see §5 on why that is an allowance and not a smell.
- **`width`/`height`** are the frame's own size, independent of the canvas. Decoupling them is what makes dynamic resolution free: the presenter resamples regardless, so scaling the SceneFrame to hold scene Hz costs nothing new (`docs/atw-first-rendering.md:118-119`).
- **`pose`** is `ViewPose { eye: Vec3, rot: Quat }`. Rotation is a quaternion, not a look-at target, because the warp's entire job is `R_now · R_scene⁻¹` — one multiplication, no degenerate cases to special-case. A look-at target cannot express a rotation delta without being converted to a rotation first, so storing the target would push the same conversion into the presenter and lose precision at the poles on the way.
- **`proj`** is `Projection { fov_y_deg, guard_deg, aspect, near, far }`. The display FOV and the guard band are **separate fields, not a single widened FOV**, because the presenter needs both numbers: the display FOV defines its output frustum, the sum defines what is actually inside the texture, and storing only the sum makes the crop unrecoverable. `guard_deg` is zero until the presenter starts widening it, which is what makes stage A the identity case rather than a special case.
- **`sim_time`** is the sim time the frame was submitted at, so scene age is reportable rather than inferred. v7 currently derives scene age from a wall-clock `Instant` held in the renderer (`renderer.rs:546-548`), which answers "how long since the scene pass ran" and not "how old is the world in this image" — those differ the moment the scene rate and the sim rate diverge, which is the case the rig exists to exhibit.
- **`seq`** is monotonic per slot. With one slot it is bookkeeping; with a ring it is the only correct way for the presenter to pick the newest *complete* frame, and it must be in the contract before the ring exists, because a ring added on top of a frame type with no ordering field is a ring with a race in it.

`near` and `far` are engine-wide constants in `scene_frame.rs`, not per-camera fields. The warp rebuilds view rays from the same frustum the scene was rendered with; if the scene pass and the presenter read two definitions they will eventually read two different ones. v7 has these inline at the single call site (`renderer.rs:113`), which is safe only because there is exactly one call site today.

## 4. Invariants

These are the properties the split exists to enforce. Each is stated as a rule about who may touch what, because that is the form a module boundary can actually hold.

**I1 — The scene renderer never touches the surface.** It has no `Surface`, no `SurfaceConfiguration`, and no swapchain texture in scope. Its output is a `SceneFrame`. This is the layering the adopted document specifies (`docs/atw-first-rendering.md:92-95`).

**I2 — The presenter owns configuration, resize, and reconfigure.** The surface configuration lives in the presenter; `resize` reconfigures the surface and is the only place that does; a `Lost`/`Outdated` surface is reconfigured in the presenter and nowhere else. In v7 all three live in the renderer (`renderer.rs:562-576` for resize, `renderer.rs:616-619` for the reconfigure-on-lost path), which is exactly the coupling that makes a headless present target impossible to add without touching resize.

**I3 — Scene-frame size and surface size are independent.** `Presenter::surface_size()` and `SceneFrame::{width,height}` are different numbers by construction. v7 keeps them coupled through a `scene_scale` that is fixed at 1.0 and never varied (`renderer.rs:171`, `renderer.rs:278`, `renderer.rs:868-869`); the hook is present, unused, and correct.

**I4 — UI composites presenter-side, never into the SceneFrame.** Anything drawn into the scene image swims when the image is warped, which is the single most common retrofit pain the document names (`docs/atw-first-rendering.md:102-104`).

**On I4, verified against v7:** the egui overlay is already composited on the presenter side of the boundary, and correctly so. `render_impl` builds the present pass into `present_enc` targeting `surface_view` (`renderer.rs:733-750`), and the overlay then opens a *second* render pass on the same encoder, also targeting `surface_view`, with `LoadOp::Load` so it blends over the presented image (`renderer.rs:776-792`). It never touches `self.scene.color_view`. The overlay's own doc comment claims this (`overlay.rs:1-5`) and the claim holds.

Two structural notes follow from that, and they are the reason I4 needs a module boundary rather than a comment:

- The overlay's GPU resources live in the renderer struct as `egui_renderer: Option<egui_wgpu::Renderer>` (`renderer.rs:201`), lazily constructed against `surface_view_format.unwrap_or(config.format)` (`renderer.rs:756-764`). Both of those are presentation state. In the split, `egui_wgpu::Renderer`, the overlay pass, and `render_with_overlay`'s parameter all move into `presenter.rs`, and `Presenter::present` grows the overlay argument. Nothing about this is hard — but nothing about the current arrangement prevents someone from moving that pass onto the scene target instead, and after the split the scene target is not reachable from there.
- The overlay is native-only (`lib.rs:10-11`, `renderer.rs:754`). The wasm build presents without it. So the invariant is enforced on one target and vacuous on the other, which is fine, and worth stating so nobody reads the wasm path as a counterexample.

## 5. Lessons carried from the proven extraction

These four were measured, not predicted. Each is a constraint on how the v7 split should be written.

**Stage entry points and the GPU context are `pub(crate)`.** `Gpu` is a crate-internal type, and `Renderer` and `Presenter` are `pub` structs in `pub` modules. A reachable `pub fn` whose signature names a `pub(crate)` type is a private-interface leak, and the gate runs `cargo clippy --workspace --all-targets -- -D warnings` (`docs/plans/milestone-1.md:300`), under which `private_interfaces` is denied. So `render_scene`, `present`, `resize`, `bind_scene`, and both constructors are `pub(crate)`. The alternative — making `Gpu` public — exports the device to game code and deletes the reason the layering exists, so the visibility is the right answer rather than a workaround. Note that this is a property of the *gate invocation*, not of a lint table: the workspace `Cargo.toml` has no `[lints]` section and the repo carries no `clippy.toml`, so a local build without the gate flags will not reproduce it.

**Scene and present go to the queue as two submissions.** Two `submit` calls, not one call carrying two command buffers. On a single queue the ordering is identical either way, so this changes nothing today; it is the shape §4.8 asks for, and it is the shape that stays correct when the scene pass grows to several chunky command buffers and the present must slot in between them rather than behind all of them. **v7 does not do this**: it builds two encoders and then submits them together in one call (`renderer.rs:798-799`), directly under a comment claiming the sliced-submission rule (`renderer.rs:673-675`). The comment describes the intent and the code implements the thing the intent rejects. Splitting the submit is a one-line change and should land with the extraction, because after the extraction it is not a choice — the two stages hold the queue at different times and cannot share an encoder list.

**The surface is acquired after the scene render.** The presenter acquires the swapchain texture inside `present`, which necessarily runs after `render_scene` returned. The tradeoff is explicit and was accepted: on `Lost`/`Outdated` the scene render for that frame is wasted work, because the acquire that discovers the problem happens after the scene pass already ran. What is bought is that the scene stage has no acquire in it at all, which is what I1 means operationally — an acquire in the scene stage is a surface in the scene stage. The dropped frame is the same dropped frame either way; only the wasted GPU work differs, and it is one frame's scene pass on a path that is already reconfiguring the surface. **v7 has the opposite ordering**: it acquires at `renderer.rs:614` and renders the scene at `renderer.rs:632-725`, so today a lost surface costs nothing. The extraction inverts this, and it is a deliberate, stated regression in a rare path rather than an oversight.

**Dead-code allowances on the retained texture handles are correct and should be written as such.** `SceneFrame::color` and `SceneFrame::depth` are held but unread while only the `Surface` present arm exists. The alternative is to not hold them and to add them back when the offscreen arm lands, which means changing the frame type at the moment the readback tests are being written. Holding them with an explicit allowance and a comment naming what will read them costs one attribute and makes the offscreen arm additive. The same reasoning already appears in v7 for the hot-reload pipeline layouts, which are dead on wasm and allowed there explicitly (`renderer.rs:189-192`).

## 6. Testable oracles

The point of the value contract is that most of this arithmetic becomes assertable rather than eyeballed. Three oracles are worth stating because each catches a distinct class of error, and two of them run without a display.

**O1 — The identity warp reduces algebraically to the blit.** One warp shader, with stage A as its degenerate case rather than a separate pipeline. For each output pixel the fragment stage builds the view ray implied by the *display* frustum, rotates it by the stored delta, and reprojects it with the *scene* frustum's tangents. When the rotation is identity and the two tangent pairs are equal, the expression collapses to `uv_out == uv_in`, which is exactly what `present.wgsl:30-31` does today. The assertion is a pure-maths unit test on the uniform derivation: equal poses and equal projections must produce the identity transform. This is the reason for one shader rather than two — stage A becomes a tested special case of stage B rather than a second code path that silently drifts out of agreement with it.

**O2 — A 7° guard band with identity rotation is pixel-exact against a 0° band.** Widening the rendered FOV and cropping back must be a no-op when there is no rotation to correct for. This is checkable by texture readback and is not checkable by eye — a crop that is wrong by a fraction of a degree looks like a slightly different framing, which is indistinguishable from a slightly different framing. It requires the `PresentTarget::Offscreen` arm, which is why that arm is designed in during the extraction rather than added later: it is the difference between two of the riskiest pieces of arithmetic in the milestone being assertions or being screenshots.

**O3 — `view_proj` and its inverse round-trip a known point.** The combined matrix must keep returning the same matrix for the same camera across the refactor, including under a representation change from look-at target to quaternion, and its inverse must stay stable at aspect ratios other than the 16:9 default so an aspect mistake cannot hide behind a square window.

O3 needs an honest caveat, because the v7 tree does not support the usual justification for it. The claim that this inverse is load-bearing for cursor aim does not currently hold: `InputState::cursor_ndc()` (`input.rs:47-49`) and `InputState::aspect()` (`input.rs:51-53`) have **zero callers anywhere in the workspace**, and the shooter aims with relative mouse deltas driving yaw and pitch (`crates/pong/src/online.rs:644-653`), not by unprojecting the cursor. The head comment claiming cursor unprojection (`crates/pong/src/online.rs:2`) is stale, left from the top-down build that preceded first-person look. So O3 protects a public contract that nothing currently consumes. It is still worth asserting — it is three lines of test, `cursor_ndc` is live API that a cursor-aimed game would use tomorrow, and an inverse that silently drifts is the kind of bug that surfaces as "aim feels slightly off" a month later — but it must not be sold as protecting a shipped game, because it does not.

## 7. Mapping onto the v7 renderer

What moves where, by name, in an order where each step builds and changes nothing on screen.

### Step A — `gpu.rs`

Extract `Gpu { device, queue, adapter_info }` from `Renderer::new`'s prologue (`renderer.rs:217-251`), returning `(Gpu, Surface, Adapter)` so adapter selection keeps its compatible surface. The `required_limits` branch for the WebGL2 fallback (`renderer.rs:235-239`) and the adapter trace line (`renderer.rs:230-231`) move with it verbatim.

### Step B — `scene_frame.rs`

New file. Move `Camera` (`renderer.rs:93-116`), `DEPTH_FORMAT` and `SCENE_FORMAT` (`renderer.rs:142-144`), and promote `SceneTargets` (`renderer.rs:149-154`) into `SceneFrame` by adding `pose`, `proj`, `sim_time`, `seq` and retaining the two texture handles. Add `ViewPose` and `Projection`. Lift `0.1`/`500.0` out of `Camera::view_proj` (`renderer.rs:113`) into `NEAR`/`FAR` constants. `create_scene_targets` (`renderer.rs:863-901`) becomes `SceneFrame::new`. Re-export `Camera` from `lib.rs` so `lib.rs:21`'s consumers are unaffected.

### Step C — `presenter.rs`

New file. Move out of `Renderer`: `surface`, `config`, `surface_view_format`, `present_pipeline`, `present_layout`, `present_bind`, `present_sampler`, `present_pipeline_layout` (`renderer.rs:165-192`), the surface-format negotiation (`renderer.rs:253-276`), `create_present_bind` (`renderer.rs:903-923`), `build_present_pipeline` (`renderer.rs:980-1011`), the acquire-and-present block (`renderer.rs:614-630`, `727-750`, `800`), and `surface_size` (`renderer.rs:551-553`). Add `PresentTarget` with the `Surface` arm implemented and the `Offscreen` arm reserved. `Presenter::resize` reconfigures the surface; `Presenter::bind_scene` rebuilds the bind group when a slot's textures are re-created, which is the second half of what `Renderer::resize` does today (`renderer.rs:570-575`).

### Step D — the overlay moves with presentation

`egui_renderer` (`renderer.rs:201`), the whole overlay block (`renderer.rs:752-796`), and the `overlay` parameter threaded through `render_with_overlay`/`render_impl` (`renderer.rs:586-601`) move into `presenter.rs`. `Presenter::present` takes the optional `OverlayDraw`. The overlay pass keeps `LoadOp::Load` on the surface view and keeps running after the present pass — behaviour identical, boundary now structural.

### Step E — `renderer.rs` becomes the scene pass

What remains: `MeshVertex`, `MeshData`, `TextureData`, `Instance`, `Frame`, `Vertex`, `InstanceRaw`, `MeshEntry`, the mesh/texture bind-group construction (`renderer.rs:291-415`), the camera uniform (`renderer.rs:417-443`), `build_scene_pipeline`, `create_instance_buf`, `cube_vertices`, and the scene-pass body (`renderer.rs:632-725`) as `render_scene(&mut self, gpu, target: &mut SceneFrame, frame)` — which additionally stamps `target.pose`, `target.proj`, and `target.seq += 1` before writing the camera uniform, and finishes with its own `gpu.queue.submit`.

### Step F — `app.rs` drives two stages

A `Gfx` unit holding `gpu`, `renderer`, `presenter`, `scene`, `scene_scale`, built together because the adapter is chosen against the surface the presenter goes on to own. `resize` reconfigures the presenter, re-creates the `SceneFrame` at the scaled size, and re-binds. The redraw path becomes: stamp `sim_time`, `render_scene`, then `present` with a view pose that is the scene's own pose — equal poses being precisely the identity warp. `app.rs:297`'s single `render_with_overlay` call becomes those two calls.

### Step G — split the submit

Independently landable and independently valuable: `renderer.rs:798-799`'s single `submit` of two command buffers becomes one `submit` per stage. After steps A–F this is forced by the structure; before them it is a one-line change that makes the code agree with the comment above it.

### Steps that are not part of the extraction

The scene-Hz throttle (`renderer.rs:196-199`, `538-548`, `608-612`) and shader hot-reload (`renderer.rs:204-211`, `803-860`) both need re-homing, and both are the subject of §8 rather than a mechanical move. Neither should be attempted in the same step as the split.

## 8. Where the archived design does not map cleanly onto v7

Five things changed upstream that the extraction did not have to handle. Each is a coordination item, not a blocker.

**8.1 — Shader hot-reload straddles the split.** `maybe_reload_shaders` (`renderer.rs:807-835`) polls two files on one frame counter and rebuilds *both* pipelines. The scene half needs `scene_pipeline_layout`; the present half needs `present_pipeline_layout` and `surface_view_format.unwrap_or(config.format)` (`renderer.rs:825`) — presentation state that will not be in scope in `renderer.rs` after the split. `try_compile` (`renderer.rs:840-860`) is shared and uses `pollster::block_on` on the error scope, so it also cannot simply be duplicated on wasm. The cheapest resolution is a small reload clock owned by `Gfx` in `app.rs` that ticks once and calls a `reload_if_changed(&Gpu)` on each stage, with `try_compile` moved to a free function taking `&Gpu`. This needs deciding before step C, because whoever writes the presenter needs to know whether it owns a pipeline rebuild path.

**8.2 — The scene-rate throttle lives in the wrong stage.** `scene_hz_cap` and `last_scene_at` are renderer fields (`renderer.rs:195-199`), `set_scene_hz_cap` and `scene_age_ms` are renderer methods (`renderer.rs:538-548`), and the decision to skip the scene pass is taken inside `render_impl` (`renderer.rs:608-612`) — which means the renderer decides whether the renderer runs. Conceptually this is a scene clock owned above both stages: `app.rs` asks the clock whether the scene is due, calls `render_scene` or not, and calls `present` unconditionally. The archived extraction had no throttle at all, so there is no precedent to copy. Additionally, `scene_age_ms` is a wall-clock measure taken from the renderer (`renderer.rs:546-548`) while the `SceneFrame` contract wants `sim_time` on the frame; the overlay consumes the wall-clock version (`app.rs:288`, `overlay.rs:62-68`). Both can coexist, but somebody should decide which one the rig's readout means.

**8.3 — The scene pass grew textures and a mesh table.** `Renderer::new(&gpu)` in the archive took no meshes; v7's takes `extra_meshes: Vec<MeshData>` and builds a `MeshEntry` table with a per-mesh texture bind group (`renderer.rs:291-415`), and the scene pass buckets instances by mesh id into contiguous ranges of a shared buffer (`renderer.rs:644-663`). This maps cleanly — it is all scene-side, and none of it touches the surface — but it means the extraction is not a copy of the archived `renderer.rs`; only its *shape* transfers, and `render_scene`'s body must be the v7 body with the pose/proj/seq stamp added, not the archived body with textures bolted back on.

**8.4 — `capture_mouse` grabs on any mouse press, including left.** `EngineConfig::capture_mouse` (`app.rs:44-46`) is per-game and defaults to false, which is the opt-in policy the design wanted. But the grab is triggered by *any* `MouseInput` press with no button filter (`app.rs:195-203`), and the shooter both opts in (`crates/pong/src/lib.rs:228`) and uses left click to fire (`crates/pong/src/online.rs:667`). It works, because that game wants pointer lock anyway — but the engine-level coupling the design warned about is present in the code, benign only by coincidence of which game is shipping. Related: `cursor_ndc` is not cleared while captured; under `CursorGrabMode::Locked` no `CursorMoved` arrives (`app.rs:176-186`), so `cursor_ndc()` returns a stale `Some` rather than `None`. See `docs/input-latch.md` §7.

**8.5 — Two stale line references in the milestone plan.** `docs/plans/milestone-1.md:72` and `:198` cite `crates/pong/src/online.rs:114-131` as the shooter's cursor unprojection; that range is now lobby-join message construction and no unprojection exists anywhere in the workspace. `:124` cites `online.rs:357` as the fire button; fire is now `online.rs:667`. Anyone sizing bite 1's oracle from those citations will size it against code that is not there.

## 9. Relationship to the rest of the plan

This document is the plan of record for the presenter split, superseding `docs/plans/milestone-1.md` §2.1, §2.2, §2.10, and bite 0. The input half of that milestone — §2.6, §2.7, bite 3, bite 4 — is superseded by `docs/input-latch.md`. The remaining bites (the warp shader, the guard band, the ring, the fly camera, the tests) keep their design as written there; only their prerequisites have moved.
