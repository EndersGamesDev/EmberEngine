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

The reason the split has to be in the API from the start, rather than added when stage B lands, is that it is the input side of what stage B *is*. A latch that cannot express two read points means stage B has nowhere to land, and §3 below is the demonstration that retrofitting a second read point into a drained accumulator has no correct form.

**Two different stalenesses, and only one of them needs a second input read.** They are easy to conflate, and the distinction decides what the latch has to do.

The first is **scene-pose staleness**: the image on screen was rendered from a pose older than the newest pose the sim has produced. This is what the throttle rig exercises. At the 15 Hz scene cap and 60 Hz redraw the rig already ships (`crates/ember-engine/src/overlay.rs:82-88`, `renderer.rs:608-612`), three of every four displayed frames re-present a scene image whose pose is up to three updates old, and the warp corrects each of them against a newer pose. This is real, it is the frame-rate-amplification argument the adopted document makes (`docs/atw-first-rendering.md:56-65`), and it needs **no second input read at all** — the pose it corrects toward is one the sim has already computed.

The second is **input staleness**: motion has arrived that no sim read has consumed, so *no* pose the sim has produced reflects it. Only this one requires a second read point, because only this one asks for a pose the sim never computed.

**A correction, recorded because the mistake is cheap to repeat.** An earlier draft of this section justified the second read point with the first staleness and asserted that at a 15 Hz cap on a 60 Hz display "three of every four displayed frames carry motion the sim never saw." That is false against this tree. `update()` runs on *every* redraw (`crates/ember-engine/src/app.rs:280`); the cap gates only the scene pass, inside `render_impl` (`renderer.rs:605-612`). Both reads would therefore execute synchronously inside one `RedrawRequested` callback, and no `DeviceEvent` can be dispatched between them, so the view read sees exactly the bytes the sim read saw and the delta is identically zero. The scene cap manufactures pose staleness, never input staleness, and no arrangement of two reads inside one callback manufactures the latter.

**Where the input delta is genuinely nonzero: a sim tick rate below the display rate.** The adopted pipeline specifies a fixed 60 Hz deterministic sim matched to the server's `TICK_HZ`, with the present clock at display rate (`docs/atw-first-rendering.md:37-41`). Once the sim is fixed-timestep — roadmap item 4, still open (`README.md:104`) — a redraw either runs a tick or does not, and on a 144 Hz display with a 60 Hz sim, 84 of every 144 presents advance no tick at all. Roughly four presents in every seven display a pose the sim has not revised while the mouse has kept moving, and that motion is visible only to a read taken at view time against a mark no sim tick has advanced. The fraction grows with the panel rather than shrinking: 64% at 165 Hz, 75% at 240 Hz.

The current tree does not have that property, because `update()` is called once per redraw with a variable `dt` (`app.rs:280`), so the sim rate and the present rate are equal by construction and the view delta is always zero. That is exactly why the split belongs in the API before it pays. The consumer that needs it does not exist yet and the fixed-timestep sim it needs does not exist yet, but the latch sits underneath both and is the expensive thing to change once they do. A second read point whose delta is currently zero costs one subtraction and yields the identity warp — the degenerate case the present shader already implements (`crates/ember-engine/src/present.wgsl:30-31`).

## 3. Why drain-and-steal cannot be patched

The obvious extension is to keep draining and let each consumer drain what it finds. It fails, and it fails in a way no amount of ordering discipline fixes.

Whichever consumer reads first takes the accumulated motion and leaves zero behind. If the sim reads first, the warp corrects nothing and the presenter is decorative. If the warp reads first, the sim loses the player's aim input and the character stops turning — which is worse, because it is a gameplay bug produced by a cosmetic subsystem. Neither ordering is correct, and there is no third ordering.

The near-misses are worth naming, because each looks plausible for about a minute:

- **Drain for the sim, peek for the warp.** The warp then re-sees motion the sim already consumed, so the correction double-counts every delta and the view over-rotates relative to the world by exactly one sim frame's worth of motion. This looks fine while standing still and smears under sustained turning, which is the hardest failure mode to catch by eye.
- **Two accumulators, both fed by the producer.** This works, and it is the same design as the one below with the subtraction spelled out twice — one field per consumer instead of one total plus one mark per consumer. It stops working the moment a third read point appears (a replay recorder, a network intent stamp, a debug trace), because the producer has to learn about every consumer. The mark form makes a new consumer a new mark and no producer change at all.
- **Drain, but only at end of frame.** That is `end_frame()`, which is the current design; it just moves the question to which reads happen before the drain, and both consumers are before it.

The property that actually has to hold is: **each unit of motion is delivered to each consumer exactly once, and no consumer's read affects any other consumer's read.** A drain cannot satisfy that, because a drain is destructive and there is only one thing to destroy.

## 4. The design: a never-reset total read against marks

Keep a total that is never reset, and read each consumer's motion as a difference against a mark — a value the total previously held.

```rust
pub struct InputState {
    pressed: HashSet<KeyCode>,
    mouse: HashSet<MouseButton>,
    cursor_ndc: Option<[f32; 2]>,
    aspect: f32,
    /// Raw device units since start. Never reset, and only ever fed motion
    /// that every consumer is entitled to see (see the focus rule below).
    mouse_total: (f64, f64),
    /// Value of mouse_total at the last sim read.
    mark_sim: (f64, f64),
}
```

A consumer's delta is `mouse_total - mark`, and advancing a mark to the current total is what "consuming" means. The subtraction is per-consumer, so the reads are independent by construction rather than by convention; there is nothing one consumer can do to the shared state that another consumer can observe.

**The sim's mark lives here; the view's mark does not.** This is the one place the design departs from the obvious symmetry, and it is load-bearing. A second `mark_view` field advanced at every present is *not* a correct warp baseline, for two independent reasons:

- **It double-applies motion the scene pose already contains.** When the scene pass runs, the pose it records was built from motion the sim had already consumed. A warp that then adds "everything since the last present" adds some of that motion a second time, and the view over-rotates relative to the world by one scene interval's worth of turning. This is the same double-count as the drain-for-sim / peek-for-warp near-miss in §3, arriving by a different route.
- **It forgets motion accumulated across repeated presents of one frame.** Under the throttle the presenter re-presents one scene frame many times. If each present advances `mark_view`, the second present of that frame sees only the motion since the *first* present, not the motion since the frame's pose was read — so the correction shrinks toward zero exactly while the scene is stalest and the correction matters most.

The quantity the warp actually needs is *motion since this frame's pose was read*, which is per-frame data. So **the mark travels with the scene frame**: `SceneFrame` carries an `input_mark: (f64, f64)`, stamped from `mouse_total` at the moment the scene pose is read, and the warp's delta is `mouse_total - frame.input_mark`. Both failures disappear structurally — a refreshed scene frame carries a fresh mark so nothing is double-applied, and a re-presented frame keeps its original mark so the delta grows correctly across repeated presents. It also survives the ring: the presenter picks the newest complete frame and gets *that* frame's mark, which a single presenter-side field could not express. The corresponding field is specified in `docs/presenter-architecture.md` §3.

The alternative the design rejects is a persistent view pose integrated independently of the sim. It works, but it is a second integrator of the same input, and two integrators of one signal are two things that can disagree — it would need its own re-anchoring rule every time the scene pose is refreshed, which is the frame-stamped mark again, reached by a longer path and with drift possible in between.

Three properties follow directly and are worth stating as the things a test should assert:

- **No starvation.** Each consumer's delta depends only on its own mark, so neither can zero the other.
- **No double-count.** Advancing a mark to the current total makes the next read of that consumer start from where this one ended, so each unit is delivered once per consumer.
- **Every delta is a genuine difference.** A mark only ever holds a value the total actually held, so a delta is the motion between two real instants and never an artifact of a reset racing a read. Note what this property is *not*: the totals are signed and their components decrease under leftward or upward motion, so neither the totals nor the deltas are monotonic and neither is non-negative. A test asserting "marks never go negative" rejects valid input; the assertable property is that a delta equals the sum of the motion delivered between the two marks.

**The total is `f64`, not `f32`, and this is load-bearing rather than fastidious.** The argument is about *relative* precision, and it has to be stated that way because the units are not under our control. winit delivers `DeviceEvent::MouseMotion { delta }` as an `(f64, f64)` in units it documents as device-dependent and unspecified — not guaranteed to be integer counts, and on macOS the values forwarded from `NSEvent` are genuinely fractional. So there is no "one count" to reason about. What holds regardless of units is the mantissa: `f32` carries 24 bits, so once a total reaches magnitude `M`, any increment smaller than about `M · 2⁻²⁴` is absorbed entirely and contributes nothing. Expressed in whole counts that is the familiar 16.7 million threshold, but with fractional deltas significance is lost proportionally earlier, and sub-unit motion is precisely the slow, careful movement a player is most likely to notice failing. The failure mode is not gradual noise — it is a threshold past which small motions vanish while large ones still register, presenting as mouse-look that stops responding to slow movement partway through a long session, and sustained turning in one direction is exactly how a player reaches the magnitude. With 53 bits the threshold is past any plausible session. The consumers can still return `f32`, because a *delta* is small even when the total is not.

This is also a constraint on the plumbing, not only on the field: `add_mouse_delta` must accept `f64`, and the narrowing cast in the producer (`crates/ember-engine/src/app.rs:136`, `delta.0 as f32`) must go. Widening the field while leaving the cast in place buys nothing — the precision is already gone before the accumulator sees it.

**Reads are pure; the engine advances the sim mark.** The consuming half is a separate crate-internal call made by the engine at the point where the sim's read is finished. The view side has no mark to advance, because its mark is the one stamped on the scene frame:

```rust
pub fn delta_since_sim(&self) -> (f32, f32);          // pure read
pub fn mouse_total(&self) -> (f64, f64);              // the stamp a scene frame records
pub fn delta_since(&self, mark: (f64, f64)) -> (f32, f32);  // pure read, frame-relative
pub(crate) fn advance_sim_mark(&mut self);
```

This is a refinement of the self-advancing sketch the milestone plan originally carried, now corrected there (`docs/plans/milestone-1.md:128`), and there are two reasons to prefer it. The first is mechanical: `EmberGame::update` takes `&InputState` (`crates/ember-engine/src/lib.rs:32`), so a self-advancing read needs either a signature change across all three shipped clients or interior mutability on the marks. The second reason is the real one: a getter that mutates is a getter that returns a different answer the second time it is called. Today `mouse_delta()` is idempotent within a frame, and game code may reasonably read it twice — once for look, once for a sensitivity readout — with no consequence. A self-advancing `delta_since_sim()` returns the full delta to the first call and zero to the second, which reintroduces the steal bug *inside a single consumer*. Pure reads plus an engine-driven advance keep the property that reading is free and consuming is explicit, and the advance points are exactly where the engine already has control.

**Motion that must be discarded is dropped at the producer, not snapped away at the marks.** `clear()` (`input.rs:98-101`) drops held keys and buttons on focus loss (`app.rs:158`) but does not touch the accumulator today, which is harmless while the accumulator is drained every frame. With a total that is never reset it stops being harmless: motion delivered while the window is unfocused would be handed to whichever consumer reads next, as one large jump.

The obvious fix — snap the marks to the current total on focus loss — is no longer sufficient once the view's mark lives on the scene frame, because a frame already in flight still carries a mark from *before* the discarded interval, and the warp would apply the whole jump on the next present. Snapping would have to reach into every live frame's stamp, which is a rule that has to be re-applied every time a new place stores a mark. The rule that does not have that shape is to keep the unwanted motion out of the total in the first place: `InputState` tracks focus, and `add_mouse_delta` ignores motion while unfocused. Every consumer, present and future, is then correct without knowing the rule exists, and marks never have to move except forward through motion that really happened.

This has to be enforced engine-side rather than assumed from the platform. winit's device-event delivery defaults to `WhenFocused` where the backend supports it, but that policy is explicitly platform-dependent and some backends keep delivering device events to an unfocused window; a design that relies on the platform filtering is correct on the developer's machine and wrong somewhere else. The same producer-side drop covers pointer-capture transitions, where entering or leaving lock can emit one large synthetic delta: discard the first motion event after a capture-mode change, for the same reason and by the same mechanism.

## 5. Determinism is untouched

The sim's guarantee is that identical inputs at identical ticks produce identical state. Nothing here changes when or what the sim reads: it reads at its own read point, through its own mark, and gets exactly the motion that arrived since its previous read — which is precisely what `mouse_delta()` returns today. The value is the same; only the mechanism by which it is made available changes.

What the warp reads is never seen by the sim, never sent to the server, and never affects the next tick. It moves pixels on a frame that has already been rendered. That is what "the warp is cosmetic" means operationally (`docs/atw-first-rendering.md:96-98`), and it is the reason a second read point is safe to add to a networked game at all: a second *consumer* of input is not a second *author* of state.

The one place to be careful is a game that derives its network intent from the view camera rather than the sim camera. The shooter does not — it gates sending on a fixed 20 Hz cadence (`crates/pong/src/online.rs:695-696`) and builds the `C2S::Input` payload from its own sim-side movement and aim state (`crates/pong/src/online.rs:715-726`) — but the rule should be written down before someone wires it the other way: **the value returned by the frame-relative view read must not reach anything that is sent, stored, or ticked.**

## 6. Migration from the current API

The migration is small because the current API is already the sim consumer's view of the new one, wearing a different name.

1. **Replace the accumulator. One commit, not three.** The field, the getter, and the drain are a single unit: `mouse_delta` is written by `add_mouse_delta` (`input.rs:61-64`), read by `mouse_delta()` (`input.rs:57-59`), and zeroed by `end_frame()` (`input.rs:66-69`), so removing the field breaks the other two in the same edit and there is no ordering of the three that compiles in between. An earlier draft of this section numbered them as three independently landable steps; following that literally produces two failed builds. The single commit does all of:
   - `mouse_delta: (f32, f32)` becomes `mouse_total: (f64, f64)` plus `mark_sim: (f64, f64)`, and `add_mouse_delta` takes `f64` and accumulates with `+=`, with no zeroing anywhere. The producer's narrowing cast (`app.rs:136`) goes at the same time.
   - `mouse_delta()` keeps its name and its `(f32, f32)` return and becomes the sim consumer's read, implemented as `delta_since_sim()`. The shooter's call site (`crates/pong/src/online.rs:646`) does not change, and neither does its behaviour: it is called once per update, before the sim mark advances, and sees the same motion it sees today. Keeping `mouse_delta` as an alias for one release is what makes the change provably behaviour-preserving; renaming the call site afterwards is a naming question, not a behavioural one.
   - `end_frame()`'s single call site (`app.rs:281`) becomes `advance_sim_mark()` — a one-word edit at the exact point the drain happens today, which is why the sim consumer's semantics are preserved by construction rather than by argument.
   - `InputState` gains the focus flag and `add_mouse_delta` gains its guard, per §4.

   This commit changes nothing observable and can land before the presenter split. Steps 2 and 3 depend on the split existing.
2. **The scene frame carries the stamp, and the view read is frame-relative.** `SceneFrame` gains `input_mark`, stamped from `mouse_total()` where the scene pose is read. In the split of `docs/presenter-architecture.md` §7 step F, the redraw path is: `update` → `advance_sim_mark` → stamp the frame's pose and `input_mark` → `render_scene` → the game's view-pose read, which calls `delta_since(frame.input_mark)` → `present`. The view read must sit between the scene submission and the present encode; that is the whole point, and it is the only ordering constraint in the design. Note what is absent: no view mark is advanced, so re-presenting the same frame repeatedly re-reads against the same stamp and the correction grows as it should.
3. **The second camera read lands as a defaulted trait method**, so no shipped client is edited:

```rust
pub trait EmberGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame;
    /// Evaluated at warp-encode time, after the scene was submitted. `look`
    /// is the mouse motion accumulated since *this frame's* pose was read;
    /// the engine has already differenced it against the frame's mark.
    fn view_pose(&self, _input: &InputState, scene: &ViewPose, _look: (f32, f32)) -> ViewPose {
        *scene
    }
}
```

The default returns the scene pose unchanged, which is exactly the identity warp — equal poses being the degenerate case the present shader already implements. Local `pong`, the arena shooter, and `game` compile and behave identically with no edit, which is also the test that the default is genuinely non-invasive.

Handing the game a pre-differenced `look` rather than the mark keeps the frame's bookkeeping inside the engine, and the `&self` receiver does most of the work of enforcing §5's rule for free: a method that cannot mutate the game cannot stash the view delta anywhere that a later tick or a later send could read it.

## 7. Capture policy

Two findings, both verified against the current tree, both about the relationship between pointer capture and the fact that a mouse button is also a gameplay button.

**Capture must be opt-in per game, and a bare left click must not be the gesture.** `EngineConfig::capture_mouse` (`app.rs:44-46`) is already per-game and defaults to false, which is the right half of the policy. The wrong half is still in place: the grab fires on *any* `MouseInput` press with no button filter (`app.rs:195-203`). The shooter opts in (`crates/pong/src/lib.rs:228`) and uses left click to fire (`crates/pong/src/online.rs:667`), so in the one game that opts in today, the fire button is also the capture gesture. Nothing is broken, because that game wants pointer lock and would have taken it on the first click regardless. But the coupling is engine-level and the safety is game-level, which is the wrong way round: a cursor-aimed game that opts into capture for some other reason would have its aim cursor swallowed the first time the player shoots, with no code change on its side to explain it. Capture should be a request the game makes — `request_capture` on the engine side — with release staying automatic on Escape and on focus loss. The button that happens to be down when the grab occurs is then the game's business, not the engine's.

**But "the game requests it" cannot mean "the engine grabs when the game asks", and this is the constraint that shapes the API.** A game's only execution point is `update`, called from `RedrawRequested` (`app.rs:280`) — which is a different, later event than the click that prompted the request. On the web, pointer lock requires the call to occur inside a user-activated event handler; issued from a redraw it is outside user activation and the browser rejects it. Calling `request_capture` from `update` and grabbing immediately therefore produces an engine that works on native and silently fails on the web, which is the platform the capture story exists for.

So `request_capture` **arms** rather than grabs. It sets a flag; the engine performs the actual `set_cursor_grab` inside the `MouseInput` press handler (`app.rs:188-204`), which is a real user gesture and already the place the grab happens today. The armed flag is what converts the engine's current unconditional "any press grabs" into "a press grabs only for a game that asked", which is precisely the policy inversion this section wants, and it costs one bool. The game keeps control of *whether*, the platform keeps control of *when*, and the web path stays inside user activation without the engine needing a synchronous mouse callback into game code.

The alternative — giving `EmberGame` an event-time callback so it can request capture inside the originating event — was considered and rejected here. It is strictly more expressive, and a game wanting to grab on a specific button would need it. But it adds a second entry point into game code with a different execution context and different aliasing rules, for a decision ("do I want pointer lock at all") that does not change from frame to frame. Arming is the smaller API for the requirement that actually exists; if a game later needs to discriminate by button, arming can carry a button mask before a callback is warranted.

**Absolute and relative pointer models coexist; neither replaces the other.** They answer different questions and one cannot be derived from the other. The absolute path answers "where is the cursor" and is what a cursor-aimed game needs. The relative path answers "how far did the pointer move" and is what mouse-look and the warp need — and the absolute path cannot supply it, because a windowed cursor position saturates at the window edge while the physical mouse keeps moving. So `cursor_ndc` must keep accumulating from `CursorMoved` (`app.rs:176-186`) and `mouse_total` must keep accumulating from `DeviceEvent::MouseMotion` (`app.rs:135-138`), from separate sources, with neither derived from the other.

Two consequences of that coexistence are currently unhandled:

- **`cursor_ndc` goes stale under capture rather than going absent.** Under `CursorGrabMode::Locked` (`app.rs:198-200`) no `CursorMoved` arrives, so `cursor_ndc()` keeps returning the last `Some` from before the grab — a position that is no longer where anything is. It should report `None` while captured, for the same reason `CursorLeft` clears it (`app.rs:187`): under pointer lock there is no meaningful absolute position, and `None` is the signal a cursor-aiming game would need if it ever coexisted with capture.
- **The absolute half currently has no consumers at all.** `cursor_ndc()` (`input.rs:47-49`) and `aspect()` (`input.rs:51-53`) are called from nowhere in the workspace, and the shooter's head comment claiming cursor unprojection (`crates/pong/src/online.rs:2`) is stale — it aims with relative deltas. This does not argue for deleting the absolute path, which is a correct and cheap API that a cursor-aimed game needs on day one. It does mean nobody should size work by assuming a shipped game is currently protecting it. See `docs/presenter-architecture.md` §6 on the oracle this affects.

On the web, capture maps to the Pointer Lock API, which requires the request to be made under user activation — hence the arming design above, which keeps the grab itself inside the `MouseInput` handler where that activation exists. The page side is already in place and needs nothing new: the per-version game pages carry their own canvas-focusing pointer listeners (`web/games/arena/v7/index.html:133-134`, `web/games/pong/v2/index.html:66-67`), which is what routes the click to the canvas in the first place. Those directories are frozen once published, so a new capture-using build gets its listener when its page is published and no already-frozen page can gain one — a reason to keep the mechanism inside the wasm bundle rather than in page script.

## 8. What this supersedes

This document is the plan of record for the input half of `docs/plans/milestone-1.md`: §2.6, §2.7, bite 3, and bite 4. It keeps that plan's never-reset-total-with-marks structure and changes four things on the merits: the total is `f64` rather than an unstated width and the producer stops narrowing to `f32`; reads are pure with the engine advancing the sim mark rather than the reads self-advancing; the view's mark is stamped on the scene frame rather than held as a second field in `InputState`, because a per-present mark is not a correct warp baseline (§4); and capture is armed by the game but grabbed by the engine inside a user gesture, because a game cannot issue a pointer-lock request from a redraw (§7).

The presenter half is in `docs/presenter-architecture.md`. The two are separable: step 1 of §6 here lands against the current tree with no presenter split, and the split lands with no input change. Only the second read point needs both, and it needs one field on `SceneFrame` that the presenter document specifies.
