# The input latch: two consumers, one accumulator

## 1. What ships today

`InputState` accumulates raw mouse motion into a single tuple and zeroes it once per frame.

The accumulator is fed from `DeviceEvent::MouseMotion`, which gives unaccelerated deltas independent of cursor position (`crates/ember-engine/src/app.rs:135-138` into `input.rs:61-64`). It is read by `mouse_delta()` (`input.rs:57-59`) and drained by `end_frame()` (`input.rs:66-69`). The engine calls the drain exactly once, immediately after the game's update returns:

```rust
let frame = self.game.update(&self.input, dt);
self.input.end_frame();
```

That is `app.rs:280-281`. One producer, one consumer, one drain, and it is correct for what it does — the shooter's first-person look reads it once per frame and integrates it into yaw and pitch (`crates/pong/src/online.rs:644-653`), which is the only consumer in the workspace.

The design is also unextendable, and the specific line where that becomes true is `app.rs:281`. Everything after it — the scene pass, the present pass, the overlay composite at `app.rs:297` — sees an accumulator that has already been zeroed. Any second reader placed at present time reads `(0.0, 0.0)`, always, with no error and no warning. The failure mode of adding a second consumer to this API is not a compile error or a panic; it is a feature that silently does nothing.

## 2. Why a second read point is not optional

Late reprojection is defined by reading input at a *later* point than the sim did. The adopted document states this twice, as two separate architectural consequences: raw input accumulates in a latch that is read at warp-encode time (`docs/atw-first-rendering.md:96-98`), and there are two camera reads — the sim/net camera that determinism and the server see, and the view camera evaluated at warp time — from the same source at two read points, with the split required to exist in the API from the start (`docs/atw-first-rendering.md:99-101`).

The reason the split has to be in the API from the start, rather than added when stage B lands, is arithmetic rather than taste. The entire latency win is the motion that arrives between the sim's read and the present. If both reads happen at the same instant, the warp corrects a delta of zero and the whole presenter is an expensive blit. So the second read point is not an implementation detail of stage B — it is the input side of what stage B *is*, and a latch that cannot express it means stage B has nowhere to land.

The scale is worth being concrete about. At a 15 Hz scene cap and a 60 Hz display — the throttle the rig already ships (`crates/ember-engine/src/overlay.rs:82-88`, `renderer.rs:608-612`) — three of every four displayed frames carry motion the sim never saw. That motion is the product.

## 3. Why drain-and-steal cannot be patched

The obvious extension is to keep draining and let each consumer drain what it finds. It fails, and it fails in a way no amount of ordering discipline fixes.

Whichever consumer reads first takes the accumulated motion and leaves zero behind. If the sim reads first, the warp corrects nothing and the presenter is decorative. If the warp reads first, the sim loses the player's aim input and the character stops turning — which is worse, because it is a gameplay bug produced by a cosmetic subsystem. Neither ordering is correct, and there is no third ordering.

The near-misses are worth naming, because each looks plausible for about a minute:

- **Drain for the sim, peek for the warp.** The warp then re-sees motion the sim already consumed, so the correction double-counts every delta and the view over-rotates relative to the world by exactly one sim frame's worth of motion. This looks fine while standing still and smears under sustained turning, which is the hardest failure mode to catch by eye.
- **Two accumulators, both fed by the producer.** This works, and it is the same design as the one below with the subtraction spelled out twice — one field per consumer instead of one total plus one mark per consumer. It stops working the moment a third read point appears (a replay recorder, a network intent stamp, a debug trace), because the producer has to learn about every consumer. The mark form makes a new consumer a new mark and no producer change at all.
- **Drain, but only at end of frame.** That is `end_frame()`, which is the current design; it just moves the question to which reads happen before the drain, and both consumers are before it.

The property that actually has to hold is: **each unit of motion is delivered to each consumer exactly once, and no consumer's read affects any other consumer's read.** A drain cannot satisfy that, because a drain is destructive and there is only one thing to destroy.

## 4. The design: a monotonic total with per-consumer marks

Keep a total that never resets, and one mark per consumer recording where that consumer last read.

```rust
pub struct InputState {
    pressed: HashSet<KeyCode>,
    mouse: HashSet<MouseButton>,
    cursor_ndc: Option<[f32; 2]>,
    aspect: f32,
    /// Raw device units since start. Never reset.
    mouse_total: (f64, f64),
    /// Value of mouse_total at the last sim read.
    mark_sim: (f64, f64),
    /// Value of mouse_total at the last view (warp-encode) read.
    mark_view: (f64, f64),
}
```

A consumer's delta is `mouse_total - mark_c`, and advancing that consumer's mark to the current total is what "consuming" means. The subtraction is per-consumer, so the reads are independent by construction rather than by convention; there is nothing one consumer can do to the shared state that another consumer can observe.

Three properties follow directly and are worth stating as the things a test should assert:

- **No starvation.** Each consumer's delta depends only on its own mark, so neither can zero the other.
- **No double-count.** Advancing a mark to the current total makes the next read of that consumer start from where this one ended, so each unit is delivered once per consumer.
- **Never negative in the accumulation sense.** A mark is only ever set to a value the total has actually held, so a delta is a genuine difference and not an artifact of a reset racing a read.

**The total is `f64`, not `f32`, and this is load-bearing rather than fastidious.** An `f32` mantissa is 24 bits, so once the magnitude of the total exceeds about 16.7 million device counts, one ULP exceeds one count and a single-count mouse movement adds nothing at all. The failure is not gradual noise that degrades smoothly — it is a threshold past which small motions vanish entirely while large ones still register, which presents as mouse-look that mysteriously stops responding to slow movement partway through a long session. Sustained turning in one direction is exactly how a player reaches that magnitude. With 53 bits the threshold is past any plausible session, and the consumers can keep returning `f32` because a *delta* is small even when the total is not.

**Reads are pure; the engine advances the marks.** The consuming half is a separate crate-internal call made by the engine at the point where that consumer's read is finished:

```rust
pub fn delta_since_sim(&self) -> (f32, f32);   // pure read
pub fn delta_since_view(&self) -> (f32, f32);  // pure read
pub(crate) fn advance_sim_mark(&mut self);
pub(crate) fn advance_view_mark(&mut self);
```

This is a refinement of the self-advancing sketch in `docs/plans/milestone-1.md:118`, and there are two reasons to prefer it. The first is mechanical: `EmberGame::update` takes `&InputState` (`crates/ember-engine/src/lib.rs:32`), so a self-advancing read needs either a signature change across all three shipped clients or interior mutability on the marks — and the second camera read has the same shape, so the cost is paid twice. The second reason is the real one: a getter that mutates is a getter that returns a different answer the second time it is called. Today `mouse_delta()` is idempotent within a frame, and game code may reasonably read it twice — once for look, once for a sensitivity readout — with no consequence. A self-advancing `delta_since_sim()` returns the full delta to the first call and zero to the second, which reintroduces the steal bug *inside a single consumer*. Pure reads plus an engine-driven advance keep the property that reading is free and consuming is explicit, and the advance points are exactly where the engine already has control.

**Focus loss snaps the marks, it does not reset the total.** `clear()` (`input.rs:98-101`) drops held keys and buttons on focus loss (`app.rs:158`) but does not touch the accumulator today, which is harmless while the accumulator is drained every frame. With a monotonic total it stops being harmless: motion delivered while the window is unfocused would be handed to whichever consumer reads next, as one large jump. The fix keeps monotonicity intact — set both marks to the current total, discarding the unfocused interval without ever moving the total backwards. The same snap belongs wherever capture is acquired or released, for the same reason.

## 5. Determinism is untouched

The sim's guarantee is that identical inputs at identical ticks produce identical state. Nothing here changes when or what the sim reads: it reads at its own read point, through its own mark, and gets exactly the motion that arrived since its previous read — which is precisely what `mouse_delta()` returns today. The value is the same; only the mechanism by which it is made available changes.

What the warp reads is never seen by the sim, never sent to the server, and never affects the next tick. It moves pixels on a frame that has already been rendered. That is what "the warp is cosmetic" means operationally (`docs/atw-first-rendering.md:96-98`), and it is the reason a second read point is safe to add to a networked game at all: a second *consumer* of input is not a second *author* of state.

The one place to be careful is a game that derives its network intent from the view camera rather than the sim camera. The shooter does not — it sends intents from its own smoothed state on a fixed cadence (`crates/pong/src/online.rs:695-699`) — but the rule should be written down before someone wires it the other way: **the value returned by `delta_since_view` must not reach anything that is sent, stored, or ticked.**

## 6. Migration from the current API

The migration is small because the current API is already the sim consumer's view of the new one, wearing a different name.

1. **Add the fields and the four methods.** `mouse_total` replaces `mouse_delta` as the field written by `add_mouse_delta` (`input.rs:61-64`); the accumulate becomes `+=` into an `f64` total with no zeroing anywhere.
2. **`mouse_delta()` becomes the sim consumer's read.** Keep the name and the `(f32, f32)` return, implemented as `delta_since_sim()`. The shooter's call site (`crates/pong/src/online.rs:646`) does not change, and neither does its behaviour: it is called once per update, before the sim mark advances, and sees the same motion it sees today. Whether `mouse_delta` is then kept as an alias or the call site is renamed to `delta_since_sim` is a naming question, not a behavioural one; the alias is worth keeping for one release so the change is provably behaviour-preserving.
3. **`end_frame()` retires.** Its single call site (`app.rs:281`) becomes `advance_sim_mark()`. This is a one-word edit at the exact point the drain happens today, which is why the sim consumer's semantics are preserved by construction rather than by argument.
4. **The view read is added after the scene submission.** In the split of `docs/presenter-architecture.md` §7 step F, the redraw path is: `update` → `advance_sim_mark` → `render_scene` → the game's view-pose read, which calls `delta_since_view` → `advance_view_mark` → `present`. The view read must sit between the scene submission and the present encode; that is the whole point, and it is also the only ordering constraint in the design.
5. **The second camera read lands as a defaulted trait method**, so no shipped client is edited:

```rust
pub trait EmberGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame;
    /// Evaluated at warp-encode time, after the scene was submitted.
    fn view_pose(&self, _input: &InputState, scene: &ViewPose) -> ViewPose { *scene }
}
```

The default returns the scene pose unchanged, which is exactly the identity warp — equal poses being the degenerate case the present shader already implements. Local `pong`, the arena shooter, and `game` compile and behave identically with no edit, which is also the test that the default is genuinely non-invasive.

Steps 1 through 3 are independently landable and change nothing observable; they can go in before the presenter split. Steps 4 and 5 depend on the split existing.

## 7. Capture policy

Two findings, both verified against the current tree, both about the relationship between pointer capture and the fact that a mouse button is also a gameplay button.

**Capture must be opt-in per game, and a bare left click must not be the gesture.** `EngineConfig::capture_mouse` (`app.rs:44-46`) is already per-game and defaults to false, which is the right half of the policy. The wrong half is still in place: the grab fires on *any* `MouseInput` press with no button filter (`app.rs:195-203`). The shooter opts in (`crates/pong/src/lib.rs:228`) and uses left click to fire (`crates/pong/src/online.rs:667`), so in the one game that opts in today, the fire button is also the capture gesture. Nothing is broken, because that game wants pointer lock and would have taken it on the first click regardless. But the coupling is engine-level and the safety is game-level, which is the wrong way round: a cursor-aimed game that opts into capture for some other reason would have its aim cursor swallowed the first time the player shoots, with no code change on its side to explain it. Capture should be a request the game makes — `request_capture` on the engine side — with release staying automatic on Escape and on focus loss. The button that happens to be down when the request is made is then the game's business, not the engine's.

**Absolute and relative pointer models coexist; neither replaces the other.** They answer different questions and one cannot be derived from the other. The absolute path answers "where is the cursor" and is what a cursor-aimed game needs. The relative path answers "how far did the pointer move" and is what mouse-look and the warp need — and the absolute path cannot supply it, because a windowed cursor position saturates at the window edge while the physical mouse keeps moving. So `cursor_ndc` must keep accumulating from `CursorMoved` (`app.rs:176-186`) and `mouse_total` must keep accumulating from `DeviceEvent::MouseMotion` (`app.rs:135-138`), from separate sources, with neither derived from the other.

Two consequences of that coexistence are currently unhandled:

- **`cursor_ndc` goes stale under capture rather than going absent.** Under `CursorGrabMode::Locked` (`app.rs:198-200`) no `CursorMoved` arrives, so `cursor_ndc()` keeps returning the last `Some` from before the grab — a position that is no longer where anything is. It should report `None` while captured, for the same reason `CursorLeft` clears it (`app.rs:187`): under pointer lock there is no meaningful absolute position, and `None` is the signal a cursor-aiming game would need if it ever coexisted with capture.
- **The absolute half currently has no consumers at all.** `cursor_ndc()` (`input.rs:47-49`) and `aspect()` (`input.rs:51-53`) are called from nowhere in the workspace, and the shooter's head comment claiming cursor unprojection (`crates/pong/src/online.rs:2`) is stale — it aims with relative deltas. This does not argue for deleting the absolute path, which is a correct and cheap API that a cursor-aimed game needs on day one. It does mean nobody should size work by assuming a shipped game is currently protecting it. See `docs/presenter-architecture.md` §6 on the oracle this affects.

On the web, capture maps to the Pointer Lock API, which requires a user gesture originating from the page. The per-version game pages carry their own canvas-focusing pointer listeners (`web/games/arena/v7/index.html:133-134`, `web/games/pong/v2/index.html:66-67`) and are frozen once published, so a new capture-using build gets its listener when its page is published and no already-frozen page can gain one.

## 8. What this supersedes

This document is the plan of record for the input half of `docs/plans/milestone-1.md`: §2.6, §2.7, bite 3, and bite 4. It keeps that plan's monotonic-total-with-marks structure and changes two things on the merits — the total is `f64` rather than an unstated width, and reads are pure with the engine advancing the marks rather than the reads self-advancing.

The presenter half is in `docs/presenter-architecture.md`. The two are separable: steps 1 through 3 of §6 here land against the current tree with no presenter split, and the split lands with no input change. Only the second read point needs both.
