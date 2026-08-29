NOTE 2026-08-29: written against the v6-era tree; superseded in part by upstream Arena v7 — re-framing into architecture docs is in progress.

**Status at v7 (`c48d72b`): that re-framing has landed, and the presenter and input designs are no longer maintained here.** `docs/presenter-architecture.md` is the plan of record for §2.1, §2.2, §2.10 and bite 0; `docs/input-latch.md` is the plan of record for §2.6, §2.7, bite 3 and bite 4. Both restate their design against the v7 renderer rather than the v6-era one, and both carry the coordination items where the v6 shape does not fit v7 — read them before sizing any of those sections. What remains live here and nowhere else: bites 1, 2, 5, 7 and 9, and the verification lane in §5.

**Three prerequisites were met upstream rather than by a bite**, and are marked at their bites below: per-mesh textures and UVs landed (`README.md:95-97`), retiring bite 8 outright; the egui overlay landed with a scene-Hz throttle and a frame-age readout (`README.md:105-108`), meeting bite 6's observability rationale without its ring; and WGSL hot-reload landed in the same roadmap item, which is new surface area the split has to re-home (`docs/presenter-architecture.md` §8.1).

**Two citations in this plan are stale against v7 and must not be used to size work.** `crates/pong/src/online.rs:114-131`, cited in §2.3 and in bite 1's oracle as the shooter's cursor unprojection, is now lobby-join message construction; no cursor unprojection exists anywhere in the workspace, and the shooter aims with relative mouse deltas (`crates/pong/src/online.rs:644-653`). `crates/pong/src/online.rs:357`, cited in §2.6 as the fire button, is now `crates/pong/src/online.rs:667`. The design conclusions drawn from both still hold; only their evidence moved.

# Milestone 1 — fly camera on a real presenter (ATW stages A and B)

Roadmap item 1 reads "first triangle → textured cube → fly camera (WASD + mouse; fly camera lands as rotation-only warp, ATW stage B)". The adoption survey (`docs/plans/adoption-survey.md`) establishes two facts that shape the whole plan:

- The triangle and the cube already exist. Instanced, depth-tested, Lambert-lit boxes have shipped since 0.5 (`crates/ember-engine/src/renderer.rs:601-631`). The only missing part of that phrase is texturing — there are no UVs on `Vertex` and no texture binding in the scene pass.
- Stage A is complete as a rendering topology and incomplete as an architecture. There is no `SceneFrame` type, no `Presenter` type, the renderer owns and presents the swapchain, there is no ring, no warp uniform, no guard band, no input latch, and no view-camera read point. Every one of those is a prerequisite for stage B.

So the milestone is really: **make the presenter split real, then land rotation-only warp with a fly camera on top of it, and pick up texturing along the way.** The plan opens with a bite-zero refactor for exactly that reason.

**Re-verified 2026-08-28 against `b1ef9af`.** The plan was written against `d3c6f48`; three commits landed after it (`7e1cac0`, `224bbd2`, `b4a9ad1`) turning online play into an arena shooter and adding a games hub, followed by a workspace `rustfmt` pass and clippy-lint fixes (`e300995` through `b1ef9af`) that changed no behaviour but moved most line numbers. Neither the milestone's goal nor any bite's purpose changed, and none of the prerequisites listed above were landed upstream. Three things did change underneath the plan and are carried into the sections below: `InputState` already has a pointer half, so bite 3 *extends* rather than introduces it; `Camera` acquired a third construction site and a gameplay consumer that inverts it, so bite 1 is a three-file change with a new oracle; and `Instance` took instance attribute location 5 for a yaw, which constrains bite 8's vertex layout. One design decision was genuinely invalidated — the left-click cursor grab in §2.6 — and is replaced rather than patched.

## 1. Scope

In scope: the presenter extraction, `SceneFrame` as a value type, a rotation-carrying camera, the warp shader and its uniform, the guard band, opt-in mouse capture and the late input latch, the two camera reads, a scene-rate throttle so warp is observable, a fly-camera demo, texturing, and the first engine unit tests.

Out of scope, deliberately: threaded or fence-synchronised scene submission (the document defers real submission slicing to roadmap step 7), depth-aware reprojection (stage C, roadmap step 3), egui (step 5), any asset loading (step 6), and any change to the sim, the wire protocols, or the servers. Nothing in this milestone touches `ember-net`, `ember-server`, `pong-core`, or `pong-server`.

Done means: `cargo run -p flycam` opens a window where mouse-look is glued to the mouse while the scene renders at a throttled 15 Hz, the cube is textured, all three shipped games — local `pong`, the online arena shooter, and `game` — render pixel-identically to today and keep their existing input behaviour, and the engine has unit tests covering camera math, warp derivation, guard-band cropping, and the latch.

## 2. Design decisions

### 2.1 Module split in `ember-engine`

`renderer.rs` currently holds the GPU device, the scene pass, and the presenter in one 631-line file. It becomes four:

- `gpu.rs` — `pub(crate) struct Gpu { device, queue, adapter_info }`, created once and shared. Both the renderer and the presenter borrow it; neither owns it.
- `scene_frame.rs` — the value types: `ViewPose`, `Projection`, `SceneFrame`, `SceneFrameRing`, and `Camera`. This is the file that carries the renderer's output contract.
- `renderer.rs` — the scene pass only. Its entry point becomes `Renderer::render_scene(&mut self, gpu: &Gpu, target: &mut SceneFrame, frame: &Frame)`. It no longer holds a `Surface` and no longer knows the swapchain exists.
- `presenter.rs` — `Presenter`, which owns the `Surface`, the surface configuration, the warp pipeline, the warp uniform buffer, and one bind group per ring slot. Entry point `Presenter::present(&mut self, gpu: &Gpu, frame: &SceneFrame, view: &ViewPose)`.

This is the module structure the adopted document's amended layering describes (`docs/atw-first-rendering.md:92-95`), and having it as files rather than as fields inside one struct is what stops stage C and egui from being retrofits.

### 2.2 `SceneFrame` is a value type carrying its own pose

```rust
pub struct ViewPose { pub eye: Vec3, pub rot: Quat }

pub struct Projection {
    pub fov_y_deg: f32,      // display FOV, before the guard band
    pub guard_deg: f32,      // extra FOV rendered but not displayed
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

pub struct SceneFrame {
    color: wgpu::Texture, color_view: wgpu::TextureView,
    depth: wgpu::Texture, depth_view: wgpu::TextureView,
    pub width: u32, pub height: u32,
    pub pose: ViewPose,
    pub proj: Projection,
    pub sim_time: f64,
    pub seq: u64,
}
```

Rotation is stored as a `Quat`, not as a look-at target, because the warp's whole job is to compute `R_now · R_scene⁻¹` and a quaternion makes that one multiplication with no degenerate cases. `seq` and `sim_time` let the presenter pick the newest complete frame and let the timing HUD (step 5) report scene age.

`Projection` deliberately keeps the display FOV and the guard band as separate fields rather than storing a single widened FOV. The presenter needs both: the display FOV defines the output frustum, the widened one defines what is actually in the texture. Storing only the sum would make the crop unrecoverable.

### 2.3 `Camera` gains a rotation

```rust
pub struct Camera { pub eye: Vec3, pub rot: Quat, pub fov_y_deg: f32 }
impl Camera { pub fn look_at(eye: Vec3, target: Vec3, fov_y_deg: f32) -> Self; }
```

This replaces `Camera { eye, target, fov_y_deg }`. There are now three construction sites across three files, not two: struct literals in local pong (`crates/pong/src/lib.rs:34-46`), `Camera::default()` in the arena client (`crates/game/src/main.rs:161`, `:171`), and the shooter's follow camera (`crates/pong/src/online.rs:334-338`), which is rebuilt every frame from a smoothed target. All three convert to `Camera::look_at` mechanically; the change is a three-file, roughly fifteen-line update. The `look_at` constructor assumes `Vec3::Y` up, matching the current `Mat4::look_at_rh` call, so the resulting view matrix is bit-comparable to today's — which is the verification oracle for that bite.

`Camera` also gained a *second kind* of consumer since this plan was written, and it is the one that constrains the change. The shooter unprojects the mouse cursor by inverting the camera's combined matrix (`crates/pong/src/online.rs:114-131`) to intersect a ray with the play plane, and that is how aiming works in a shipped game. So `view_proj(aspect)` must keep returning exactly the same matrix for the same camera — the quaternion is an internal representation change, not a change to the projection — and bite 1's oracle must cover the inverse as well as the forward matrix. If `look_at` normalises differently than `Mat4::look_at_rh` does, aim drifts, and it drifts subtly rather than visibly.

Alternative considered and rejected: keep `eye`/`target` and derive the quaternion at warp time. Rejected because the derivation is lossy at the poles and because a fly camera is naturally authored as yaw and pitch, so every consumer would immediately convert back.

### 2.4 One warp shader, with stage A as its degenerate case

`present.wgsl` becomes `warp.wgsl` and its fragment stage does the full rotation-only reprojection rather than a bare sample. For each output pixel: build the view-space ray implied by the *display* frustum, rotate it into the scene's view space by the stored delta, then project it with the *scene* frustum's tangents to get the texture coordinate.

```wgsl
struct Warp {
    scene_from_now: mat4x4<f32>,   // R_scene * R_now^-1, as a rotation
    tan_half_display: vec2<f32>,   // tan(fov/2) of the output frustum
    tan_half_scene: vec2<f32>,     // tan(fov/2) of the rendered (guard-banded) frustum
};
```

When the rotation is identity and the two tangent pairs are equal, this reduces algebraically to `uv_out == uv_in` — the identity blit, exactly what ships today. That is the reason for one shader instead of two: stage A becomes a *tested special case* of stage B rather than a separate code path that silently drifts out of agreement. The cost is roughly ten ALU operations per pixel on a path that was one texture fetch, which is negligible against the 0.2–0.5 ms the document already budgets for the pass.

The alternative — keeping the identity pipeline and selecting between two pipelines at runtime — stays open if measurement on a weak iGPU ever shows the arithmetic mattering. It is a small change to make later and a needless fork to make now.

The sampler must clamp to edge so that a ray leaving the guard band smears the border pixel instead of wrapping; `wgpu::SamplerDescriptor::default()` already gives `ClampToEdge`, but the presenter should state it explicitly rather than inherit it, since correctness at the band edge now depends on it.

### 2.5 Guard band

`Projection::guard_deg` defaults to 0.0 and is set to 7.0 by the presenter when warp is enabled, per the document (`docs/atw-first-rendering.md:79-81`). The scene renders at `fov_y_deg + guard_deg`; the presenter crops back to `fov_y_deg`. Consequence for the scene pass: it must build its projection from the widened FOV, and the resulting frustum is wider than the display, which is the first time the codebase has a frustum that is not the display frustum. Since there is no culling yet, nothing else has to change — but the *shape* required by ATW consequence 5 is in place for when culling arrives.

The strong self-check this buys: with a 7° guard band and an identity rotation, the presenter's output must be pixel-identical to a 0° guard band with an identity rotation. If the crop arithmetic is wrong, that equality fails. It is checkable by texture readback, not by eye.

### 2.6 The late input latch: a monotonic accumulator with per-consumer marks

The obvious latch design — accumulate deltas, drain on read — breaks with two readers, because whichever reads first steals the motion from the other. Instead `InputState` keeps a monotonic total and one mark per consumer.

`InputState` is no longer the two-field struct this plan was drafted against: it already carries a pointer half (a held-button set, an absolute cursor position in NDC, and the viewport aspect), added upstream for the shooter's cursor aim. The latch therefore *extends* it rather than replacing it, and the absolute fields must survive untouched:

```rust
pub struct InputState {
    pressed: HashSet<KeyCode>,
    mouse: HashSet<MouseButton>,      // existing: held buttons
    cursor_ndc: Option<[f32; 2]>,     // existing: absolute cursor, -1..1, +y up
    aspect: f32,                      // existing: viewport aspect for unprojection
    mouse_total: (f64, f64),          // new: raw device units since start, never reset
    mark_sim: (f64, f64),             // new: value at the last sim read
    mark_view: (f64, f64),            // new: value at the last warp read
}
```

`delta_since_sim()` and `delta_since_view()` each return the difference from their own mark and then advance it. Neither consumer can starve the other, and because the view read happens at warp-encode time it naturally sees motion that arrived *after* `update()` returned — which is the entire point of the latch. The sim's determinism is untouched: it still consumes input at its own read point, and the warp is cosmetic (`docs/atw-first-rendering.md:96-98`).

**The two pointer models coexist; neither replaces the other.** The absolute path answers "where is the cursor", which is what a cursor-aimed game needs and what the shooter ships today. The relative path answers "how far did the pointer move", which is what a fly camera and the warp need, and which the absolute path cannot supply because a windowed cursor position saturates at the window edge. `cursor_ndc` must keep meaning what it means today, so `mouse_total` accumulates from a separate source (`DeviceEvent::MouseMotion`) and never derives from `CursorMoved`.

Capture: `ApplicationHandler::device_event` handling `DeviceEvent::MouseMotion { delta }` on native, which gives unaccelerated deltas independent of cursor position.

**Design change — the original left-click grab is withdrawn.** This plan originally requested cursor grab on left mouse press. That is no longer available: left mouse press is the shooter's fire button (`crates/pong/src/online.rs:357`), so an engine-level grab on left click would capture the pointer the first time a player shoots, hide the cursor the game aims with, and break a shipped game with no code change on its side. Capture becomes opt-in and game-driven instead: the game asks for it (a `request_capture` on the engine side, called by `flycam` and by nothing else), and `Window::set_cursor_grab(CursorGrabMode::Locked)` with `set_cursor_visible(false)` follows from that request. Release stays automatic on Escape and on focus loss. While captured, `cursor_ndc()` reports `None`, because under pointer lock there is no meaningful absolute position — which is also the signal a cursor-aiming game would need if it ever coexisted with capture.

On the web this maps to the Pointer Lock API, which requires a user gesture, so the gesture must come from the game page. The listener the plan expected to reuse has moved: `web/index.html` is now the games hub and hosts no canvas, and the canvas-focusing pointer listener lives in each per-version game page (`web/games/arena/v3/index.html:121-123`, `web/games/pong/v2/index.html:66-68`). Those directories are frozen once published, so `flycam`'s page gets its own listener when it is published and no existing page needs editing — but no already-frozen page can ever gain one either.

### 2.7 Two camera reads, added without breaking existing games

`EmberGame` gains a second method with a default implementation:

```rust
pub trait EmberGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame;
    /// Evaluated at warp-encode time, after the scene was submitted.
    fn view_pose(&self, _input: &InputState, scene: &ViewPose) -> ViewPose { *scene }
}
```

The default returns the scene pose unchanged, which is exactly the identity warp — so local `pong`, the arena shooter, and `game` all compile and behave identically with no edit at all. The fly-camera demo overrides it, reads `delta_since_view()`, and returns the corrected pose. This is the document's "same source, two read points" (`docs/atw-first-rendering.md:99-101`), and making it a defaulted method is what lets it land without touching either shipped client.

### 2.8 Scene-rate throttle instead of threaded decoupling

Warp is invisible when the scene runs at display rate — it corrects a delta of zero. To make stage B both demonstrable and testable, the loop presents every redraw but renders the scene only when a rate cap allows:

```rust
// in App::window_event, RedrawRequested
if scene_clock.due(now) { renderer.render_scene(&gpu, ring.write_slot(), &frame); ring.publish(); }
presenter.present(&gpu, ring.newest(), &game.view_pose(&input, ring.newest_pose()));
```

The cap is cycled by a function key (uncapped / 30 / 15 / 5 Hz) and logged. This is the debug hook the document asks for at step 5 (`docs/atw-first-rendering.md:120-122`), landed early because it is the only practical oracle for the warp, and it delivers genuine frame-rate amplification within one thread and one queue. Real asynchronous submission remains roadmap step 7.

A ring of two slots is enough for this: the presenter reads the last published slot while the renderer writes the other. Three slots buy nothing without asynchronous submission.

### 2.9 Texturing without an asset pipeline

`Vertex` gains a `uv: [f32; 2]`. `cube_vertices` already computes per-face tangent vectors (`renderer.rs:602-610`), so the four corners map to (0,0), (1,0), (1,1), (0,1) with no new maths.

One layout constraint arrived with the shooter: shader locations are global across vertex buffers, and the instance buffer now runs 2 through 5 because `Instance::yaw` took location 5 (`renderer.rs:260`). The vertex buffer holds 0 and 1, so `uv` becomes `@location(6)` rather than the 2 it would have taken before. Renumbering the instance attributes instead would be a larger and pointless edit. Bite 8 must also leave `yaw` working: the scene shader rotates position and normal together (`shader.wgsl:24-40`), and UVs are per-face constants that rotation does not touch, so the two features compose without interacting.

The texture itself is generated procedurally at startup — a checker or grid pattern written into an `Rgba8UnormSrgb` texture — and modulated by the per-instance colour, so both shipped games keep their palettes and simply gain surface detail.

Procedural rather than a PNG: it adds no dependency, no asset path, no loading failure mode, and no wasm fetch, and roadmap step 6 is where real assets belong. It also happens to be the best possible test pattern for warp artifacts, because high-frequency detail makes resampling error obvious. If a real texture is wanted before step 6, adding the `image` crate is a contained follow-up.

### 2.10 A headless present target, for automated verification

`Presenter` takes its output through a small enum rather than hardcoding the surface:

```rust
pub enum PresentTarget { Surface(wgpu::Surface<'static>), Offscreen(wgpu::Texture) }
```

Bite 0 implements only the `Surface` arm. The `Offscreen` arm (bite 9) is what makes the identity-warp and guard-band-crop properties testable by readback on a server lane with no display, turning two of this milestone's riskiest pieces of arithmetic from eyeball checks into assertions. Designing the enum in during the refactor costs one indirection; adding it afterwards means touching the presenter's whole surface again.

## 3. Bites

Each bite is independently gate-able: it builds, it does not regress the two shipped clients, and it has a stated oracle. No bite carries an open design question.

### Bite 0 — extract the presenter (required by the survey)

**NOT IN THE v7 TREE.** This bite was built and gate-proven, then deliberately dropped when upstream's renderer grew past it; `main` at `c48d72b` has no `presenter.rs`, no `gpu.rs`, and no `scene_frame.rs`. The extraction survives only on `archive/v6-stack`, and its design is restated against the current renderer in `docs/presenter-architecture.md`, which is what should be implemented — not this sketch and not a cherry-pick of the archived commit. The record below is kept for the deviations it measured, which that document carries forward as constraints.

**LANDED** 2026-08-28, commit a4dd8a7 (six files, +764/−465, five gates green). Deviations from the sketch, accepted: no `SceneFrameRing` yet (bite 6 owns it); stage entry points and `Gpu` are `pub(crate)` (a reachable `pub fn` taking a `pub(crate)` type trips `-D private_interfaces`); scene and present are two queue submissions (closer to the ATW slicing rule); the surface is acquired after the scene render, so a `Lost`/`Outdated` surface now wastes one scene render but drops the same frame. On-screen pixel identity across the three clients is still unverified — needs a human with a display.

The pure refactor that makes the split real. Split `renderer.rs` into `gpu.rs`, `scene_frame.rs`, `renderer.rs`, `presenter.rs`; introduce `SceneFrame` with pose, projection, and timestamp; move `Surface` ownership into `Presenter`; define `PresentTarget` with the `Surface` arm implemented; rewire `app.rs` to call the scene pass and the present pass as two steps. Behaviour is unchanged throughout: one scene and one present per redraw, identity warp, one ring slot.

- Files: `crates/ember-engine/src/lib.rs`, `renderer.rs`, and new `gpu.rs`, `scene_frame.rs`, `presenter.rs`; `crates/ember-engine/src/app.rs`.
- New: `Gpu`, `ViewPose`, `Projection`, `SceneFrame`, `Presenter`, `PresentTarget`, `Renderer::render_scene`, `Presenter::present`.
- Diff scale: roughly +410 / −290, almost all of it moved code.
- Oracle: `cargo build --workspace --all-targets`; `cargo check -p pong --lib --target wasm32-unknown-unknown`. Visual: local `pong`, the arena shooter, and `game` are pixel-identical to the previous commit — this bite must change nothing on screen. `Instance::yaw` must survive the move of the instance-buffer layout into the new scene pass; a dropped attribute shows up as unrotated guns in the shooter, not as a build error.

### Bite 1 — rotation-carrying camera

Replace `Camera { eye, target, fov_y_deg }` with `Camera { eye, rot, fov_y_deg }` plus `Camera::look_at`, and derive the view matrix from the quaternion. Update all three clients' construction sites.

- Files: `crates/ember-engine/src/scene_frame.rs`, `crates/pong/src/lib.rs`, `crates/pong/src/online.rs`, `crates/game/src/main.rs`.
- New: `Camera::look_at`, `Camera::view_matrix`, `ViewPose::from(&Camera)`.
- Diff scale: roughly +85 / −45.
- Oracle: build; unit test asserting `Camera::look_at(e, t, f).view_matrix()` equals `Mat4::look_at_rh(e, t, Vec3::Y)` within tolerance, plus a second test that `view_proj(aspect)` and its inverse round-trip a known point — the shooter's aim depends on that inverse (`crates/pong/src/online.rs:114-131`). Visual: `pong` framing unchanged, and the shooter's cursor aim still lands where the cursor is, checked at a window aspect other than the 16:9 default so an aspect mistake cannot hide.

### Bite 2 — warp uniform and unified warp shader

Rename `present.wgsl` to `warp.wgsl`, replace its fragment body with the ray-unproject / rotate / reproject math, add the `Warp` uniform buffer and its bind-group entry, and have the presenter fill it. Stage A is expressed as identity rotation with equal tangents.

- Files: `crates/ember-engine/src/present.wgsl` → `warp.wgsl`, `crates/ember-engine/src/presenter.rs`.
- New: `WarpUniform` (Pod), `Presenter::write_warp`, explicit `ClampToEdge` on the sampler.
- Diff scale: roughly +150 / −45.
- Oracle: build; unit test that the uniform derived from equal poses and equal projections is the identity case. Visual: output unchanged from bite 1.

### Bite 3 — mouse capture and the input latch

Implement `device_event` for `DeviceEvent::MouseMotion`; add opt-in, game-requested cursor grab with release on Escape and focus loss; extend `InputState` with the monotonic accumulator and the two marks, leaving its existing pointer fields intact.

This bite is additive to an input path that already exists. `InputState` keeps `pressed`, `mouse`, `cursor_ndc`, and `aspect` exactly as they behave today — the shooter reads all but the first — and gains `mouse_total`, `mark_sim`, and `mark_view` alongside them. Capture is requested by the game, never triggered by a bare left click, for the reason given in §2.6.

- Files: `crates/ember-engine/src/app.rs`, `input.rs`, `lib.rs`.
- New: `InputState::delta_since_sim`, `delta_since_view`, `mouse_captured`, `App::set_grab`, an engine-side `request_capture`.
- Diff scale: roughly +170 / −15 — slightly more new code and less deletion than originally estimated, because the pointer plumbing this bite expected to add is partly present and must be preserved rather than rewritten.
- Oracle: build; unit test on the two-mark semantics (two consumers each see the full delta exactly once, and neither starves the other). Regression test that matters more than the new one: the arena shooter still aims and fires normally with capture never requested — `cursor_ndc()` keeps returning `Some` when uncaptured, and left click still reaches `mouse_down` rather than being consumed as a grab gesture. Manual: with a trace log of the per-frame delta, motion is nonzero under lock, zero when unfocused, and the marks never go negative.

### Bite 4 — the second camera read

Add `EmberGame::view_pose` with its identity default; have `app.rs` call it after the scene submission and pass the result to `Presenter::present`.

- Files: `crates/ember-engine/src/lib.rs`, `app.rs`, `presenter.rs`.
- Diff scale: roughly +55 / −12.
- Oracle: build; local `pong`, the arena shooter, and `game` compile untouched and render identically, proving the default is genuinely non-invasive.

### Bite 5 — guard band

Add `guard_deg` to `Projection`, render the scene at the widened FOV, and have the presenter's warp uniform carry both tangent pairs so it crops back.

- Files: `crates/ember-engine/src/scene_frame.rs`, `renderer.rs`, `presenter.rs`.
- Diff scale: roughly +80 / −25.
- Oracle: build; unit test that a 7° band with identity rotation produces the same sampled rectangle as a 0° band. Visual: with the band on and warp disabled, the image is unchanged; with the band on and the camera snapped, no black wedge appears at the edges.

### Bite 6 — scene-rate throttle and a ring of two

**PARTIALLY SUPERSEDED at v7.** The observability half landed upstream: the egui rig ships a scene-Hz throttle and a scene-age readout (`crates/ember-engine/src/overlay.rs:78-93`), and the scene pass is skipped while capped so the presenter re-presents the last frame (`crates/ember-engine/src/renderer.rs:608-612`). So the reason this bite existed — that warp is invisible at display rate and needs an oracle — is already met, and `EngineConfig::scene_hz_cap` is not needed. What remains outstanding is the ring of two, and one structural problem the upstream version introduced: the throttle state and the skip decision live inside the renderer, so the renderer decides whether the renderer runs (`docs/presenter-architecture.md` §8.2). Re-homing the clock above both stages is the live part of this bite.

Present every redraw, render the scene on a rate cap, publish into a two-slot ring, and cycle the cap from the keyboard with a trace line reporting scene Hz against present Hz.

- Files: `crates/ember-engine/src/app.rs`, `scene_frame.rs`, `presenter.rs`.
- New: `SceneFrameRing`, `SceneClock`, `EngineConfig::scene_hz_cap`.
- Diff scale: roughly +140 / −45.
- Oracle: build; manual: at a 15 Hz cap the reported present rate stays at display rate while the scene rate drops, and `pong` at an uncapped setting is unchanged.

### Bite 7 — fly camera demo, stage B lands

A new minimal crate implementing `EmberGame` with WASD plus mouse: `update` advances the sim camera from held keys and `delta_since_sim`, `view_pose` applies `delta_since_view` on top of the scene pose. A grid of textured cubes gives the eye something to judge the warp against.

- Files: new `crates/flycam/Cargo.toml` and `crates/flycam/src/main.rs`; workspace `Cargo.toml` member list.
- New: `FlyCam { eye, yaw, pitch, speed, sensitivity }`.
- Diff scale: roughly +220 new.
- Oracle: `cargo run -p flycam`. Manual, and this is the milestone's headline check: at a 15 Hz scene cap, mouse-look stays glued to the mouse while world motion visibly steps; disabling warp under the same cap makes the view judder with the scene. The difference between those two must be obvious without instrumentation.

### Bite 8 — textured cube

**DEAD — landed upstream at v7.** Per-mesh textures and UVs shipped with the textures half of roadmap item 1 (`README.md:95-97`): `MeshVertex` carries a `uv` (`crates/ember-engine/src/renderer.rs:17-22`), each mesh gets a texture bind group at group(1) with a shared 1x1 white pixel standing in for untextured meshes (`crates/ember-engine/src/renderer.rs:291-415`), and instances are bucketed by mesh id into one instanced draw per mesh (`crates/ember-engine/src/renderer.rs:644-663`). Real PNG assets replaced the procedural checker this bite proposed, so the vertex-layout and location-numbering analysis below is historical. The scene pass having grown a mesh table is itself a coordination item for the split (`docs/presenter-architecture.md` §8.3).

Add `uv` to `Vertex` at `@location(6)` (locations 2–5 belong to the instance buffer since `yaw` took 5), generate a procedural checker texture, add its bind group to the scene pass, and sample it modulated by the instance colour.

- Files: `crates/ember-engine/src/renderer.rs`, `shader.wgsl`.
- New: `create_checker_texture`, a second scene bind group.
- Diff scale: roughly +150 / −30.
- Oracle: build; visual: `flycam`, local `pong`, the arena shooter, and `game` all show surface detail with their existing colours preserved, and yawed instances (the shooter's guns) show the texture rotating with the box rather than sliding across it.

### Bite 9 — engine tests and the headless present target

Implement the `PresentTarget::Offscreen` arm and add the first `ember-engine` test module: camera round-trip, warp identity, guard-band crop equality by texture readback, and latch semantics.

- Files: `crates/ember-engine/src/{scene_frame.rs, presenter.rs, input.rs}` test modules; possibly `crates/ember-engine/tests/warp.rs`.
- Diff scale: roughly +260 new.
- Oracle: `cargo test -p ember-engine`. Readback tests need a GPU adapter, so on a headless lane they must skip cleanly when no adapter is available rather than fail — the pure-maths tests must run everywhere.

## 4. Order and parallelism

```
0 ──┬── 1 ── 2 ── 5 ──┬── 7
    │                 │
    ├── 3 ── 4 ───────┘
    │
    ├── 6 (serialise with 3 and 4 on app.rs)
    │
    └── 8 (serialise with 5 on renderer.rs)

9 trails each of its subjects; splittable per-bite.
```

- **Bite 0 blocks everything.** It is the only bite that must land alone.
- **Bites 1 and 3 are parallelizable** after bite 0 — camera types versus input, disjoint except for a re-export line in `lib.rs`.
- **Bite 8 is parallelizable** from bite 0 onward; it is the only bite that touches neither the presenter nor the input path. It conflicts with bite 5 on `renderer.rs`, so land it either before 5 or after it, not concurrently.
- **Bites 3, 4, and 6 all edit `app.rs`** and should be sequenced in that order rather than run concurrently.
- **Bite 7 requires 4, 5, and 6** — it is the integration point where stage B becomes visible, and it should not be attempted before all three are in.
- **Bite 9 can be split**: the camera and latch tests can land with bites 1 and 3; the readback tests need bite 5.

The natural cut into review-sized pushes is 0 · then 1+3 in parallel · then 2+8 · then 4+5 · then 6 · then 7 · then 9.

## 5. Verification lane

No server lane is provisioned for this repo yet, so these are the commands to provision it with. All of them run on a server, at minimum build priority, on the nightly toolchain with a stable per-purpose target directory.

```
cargo build --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo-fmt --all -- --check
cargo test --workspace
cargo check -p pong --lib --target wasm32-unknown-unknown
```

Every gate reports its wall time. Two existing headless bots should stay in the lane as regression proof that this milestone did not disturb the network paths, since neither is touched by any bite: `cargo run -p ember-net --example netbot` and `cargo run -p pong-server --example wsbot`.

What the lane cannot check, and what therefore stays a manual pass on a machine with a display: the actual appearance of the warp under a throttled scene rate (bite 7's headline check), the WebGL2 fallback path, and pointer lock behaviour in a browser. Everything else in this milestone — the camera algebra, the warp derivation, the guard-band crop, and the latch semantics — was designed to be assertable, which is the main reason the milestone ends with a test bite rather than a screenshot.

## 6. README reconciliation

Landing this milestone makes three README statements true that are currently not, and they should be corrected in the same push rather than left to drift. All three were re-checked against the README as amended upstream; the "## Games hub" section was inserted at `README.md:129-140`, below every line cited here, so none of these references moved:

- The 0.6 line (`README.md:90-92`) claims a `SceneFrame` and a presenter that owns the swapchain; both become real at bite 0.
- The layering line (`README.md:44`) predates the adopted ATW document and omits the presenter stage; it should be restated as game → sim → scene renderer → presenter → platform.
- Item 1's "first triangle → textured cube" (`README.md:95`) describes work that was largely done at 0.5; only texturing (bite 8) is outstanding, and the item's real content is the fly camera and stage B. **Resolved upstream at v7**: the item was split into a checked textures half and an open fly-camera half (`README.md:95-97`), which is the reconciliation this line asked for.

Roadmap item 5 also went from planned to checked upstream (`README.md:105-108`), which changes what §2.8 and bite 6 are for — see the mark on bite 6. Its second half, hot-reloadable WGSL, is not anticipated anywhere in this plan and is the one piece of new upstream surface area that makes the presenter split harder rather than easier (`docs/presenter-architecture.md` §8.1).

A fourth README problem appeared with the upstream commits and is *not* fixed by this milestone: the "## Pong" section (`README.md:142-150`) still describes online play as two-player paddle pong with a flipped camera and names `crates/pong-core/src/sim.rs` as the server-side sim, when online play is now the arena shooter over `pong-core/src/shooter.rs`. Nothing in this plan touches it, and it should not be folded into a milestone-1 push — it belongs to whoever reconciles the shooter's own documentation. It is recorded here so the next person to edit this README does not assume the section was reviewed and found correct.
