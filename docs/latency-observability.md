# Latency observability

*Provenance: static assessment of the adopted ATW design and the v7 engine, arena client, protocol, and arena server at this tree's HEAD. No runtime numbers in this document were inferred from source.*

## 1. Verdict and acceptance test

**Verified — verdict.** The concept meets the data-flow half of the requirement but not the latency-accounting half: it names sim, scene, and present clocks, orders the presenter after the scene renderer, gives `SceneFrame` simulation identity, and gives input two independent read points, but it does not define a monotonic timestamp at every hop or a statistics contract (`docs/atw-first-rendering.md:35-54`, `docs/atw-first-rendering.md:90-122`, `docs/presenter-architecture.md:26-69`, `docs/input-latch.md:50-105`). The implementation meets only fragments of either half: it has redraw-gap diagnostics, one overwritten scene-start time, a native-only scene-age readout, logical network sequence/tick identities, and a server transport-RTT sample, but no end-to-end trace, no complete stage breakdown, no GPU timestamps, and no percentile distributions (`crates/ember-engine/src/app.rs:258-277`, `crates/ember-engine/src/renderer.rs:532-548`, `crates/pong-core/src/proto.rs:94-115`, `crates/pong-server/src/lib.rs:264-311`).

**Derived — what would satisfy the question.** For one causally identified input, the toolchain must retain the local monotonic times at which it crossed each named boundary, report the total from engine-visible input arrival to the last honestly observable presentation handoff, report overlapping CPU and GPU lanes without pretending their durations add linearly, and accumulate each duration into a distribution from which p50, p95, and p99 are extracted. A current value, average, maximum, tick number, or frame sequence can help diagnose a symptom but cannot answer the percentile half.

**Proposed — acceptance bar.** A capture is complete only when every enabled stage reports sample count, p50, p95, p99, maximum, overflow count, and unavailable/unsupported count; a selected input/frame trace shows its causal IDs and raw boundary stamps; and the display explicitly labels its endpoint `present handoff`, never `photon`. The network fork must show client-local enqueue-to-ack, server-local receive-to-apply and receive-to-broadcast, and socket-queue delays separately; it must not subtract clocks from different machines.

## 2. What the adopted concept accounts for

### 2.1 It defines stages and ordering

**Verified.** The adopted policy defines three clocks and the path `input latch → sim → scene render → SceneFrame ring → warp + UI → canvas`; it also requires the scene renderer to output a `SceneFrame`, the presenter to own the surface, raw input to be read again at warp encode, and scene work to be submitted in slices (`docs/atw-first-rendering.md:35-54`, `docs/atw-first-rendering.md:92-101`, `docs/atw-first-rendering.md:112-122`). These are useful attribution seams: sim read, scene production, late view read, present encode, and platform handoff have distinct owners.

**Verified.** The presenter extraction sharpens those seams into `render_scene` and `present`, makes `SceneFrame` the value passed between them, and intends separate queue submissions (`docs/presenter-architecture.md:13-22`, `docs/presenter-architecture.md:90-106`, `docs/presenter-architecture.md:126-167`). That architecture can host timing instrumentation without introducing a second data path.

### 2.2 `sim_time` and `seq` are necessary provenance, not latency provenance

**Verified.** The proposed `SceneFrame` carries `sim_time: f64` and `seq: u64`; the stated purposes are world-age reporting and selection of the newest complete ring slot (`docs/presenter-architecture.md:30-44`, `docs/presenter-architecture.md:62-65`). `sim_time` identifies the simulated world instant and `seq` orders frames, but neither records when input arrived, when the sim read it, how long CPU encoding took, how long work waited on the queue, when GPU passes ran, or when ownership crossed to the platform.

**Derived.** Two frames may have equal simulation age but different input wait, CPU encode, queue, GPU, and compositor delays; one monotonic `seq` can order those frames but supplies zero durations. The pair is therefore sufficient for scene identity and logical freshness, not for locating milliseconds.

**Verified.** The concept itself exposes the distinction: it says the desired `sim_time` differs from v7's wall-clock `last_scene_at`, because world age and time since the scene pass ran diverge when sim and scene rates diverge (`docs/presenter-architecture.md:62`). Both values are useful, so latency accounting must retain both domains under different names rather than treating either as the other.

### 2.3 Input marks are consumption provenance, not time provenance

**Verified.** The latch design proposes a never-reset mouse total plus `mark_sim`, with the warp's baseline stamped on each scene frame as `input_mark`; every mark is only a pair of accumulated device counts, and the consumer delta is total minus its mark (`docs/input-latch.md:50-68`). Its public sketch returns deltas and advances the sim mark, but contains no event sequence, arrival timestamp, or read timestamp (`docs/input-latch.md:89-96`).

**Derived.** Those marks can prove which motion each consumer has consumed, but not when any motion arrived or how long it waited. The design therefore establishes exactly the right carrier and read points while leaving time unaccounted between them.

### 2.4 Concept-level answer

**Verified — concept verdict.** The architecture defines attributable owners and causal data flow, not latency observations. The promised timing HUD is a requirement without a metric schema, timestamp contract, observation boundary, or percentile accumulator (`docs/atw-first-rendering.md:120-122`, `docs/atw-first-rendering.md:138-144`). It is ready to be instrumented, but it does not yet make the requirement true.

## 3. What v7 measures on the local display path

### 3.1 Stage-boundary inventory

**Verified — local boundary inventory.** These are the live boundaries and stamps in the order the current redraw path crosses them (`crates/ember-engine/src/app.rs:258-301`, `crates/ember-engine/src/renderer.rs:597-800`).

|Boundary|Current evidence|Timestamp at that boundary?|What is knowable today|
|--------|----------------|---------------------------|----------------------|
|OS/winit relative-motion arrival|`device_event` matches `DeviceEvent::MouseMotion` and immediately calls the latch writer (`crates/ember-engine/src/app.rs:129-137`).|No.|The event was delivered before the call returned; its arrival time and prior OS/device residence are unknown.|
|OS/winit key or mouse-button arrival|`window_event` mutates key and button state (`crates/ember-engine/src/app.rs:159-164`, `crates/ember-engine/src/app.rs:188-203`).|No.|Current held state is known; edge arrival time is not retained.|
|Input latch write|Relative motion adds into one tuple; key/button methods mutate sets (`crates/ember-engine/src/input.rs:8-19`, `crates/ember-engine/src/input.rs:61-84`).|No.|Values are retained until the next game update, with no sequence or time.|
|Sim/update read begins and returns a `Frame`|The engine makes one call and then drains mouse motion (`crates/ember-engine/src/app.rs:280-281`); the shooter reads mouse motion and held buttons inside that call (`crates/pong/src/online.rs:639-667`).|No exact boundary stamp.|A redraw-level `now` was sampled earlier, but it is not attached to the read or returned `Frame` (`crates/ember-engine/src/app.rs:258-263`, `crates/ember-engine/src/renderer.rs:118-123`).|
|Surface acquisition|`get_current_texture` precedes both encoders (`crates/ember-engine/src/renderer.rs:614-630`).|No.|Success, reconfigure, or drop is known; acquire wait is not measured.|
|Scene CPU encode begins|Native-only `last_scene_at = Instant::now()` occurs before camera upload, bucketing, and encoder creation (`crates/ember-engine/src/renderer.rs:632-680`).|Partial: one overwritten start time, native only.|Wall time since the latest due scene began can later be computed; no frame identity or end time survives.|
|Scene CPU encode ends|`scene_enc.finish()` creates the command buffer (`crates/ember-engine/src/renderer.rs:676-725`).|No.|CPU scene-encode duration is unknown.|
|Scene GPU pass begins/ends|The scene render-pass descriptor explicitly sets `timestamp_writes: None` (`crates/ember-engine/src/renderer.rs:681-709`).|No.|GPU queue wait and GPU scene duration are unknown.|
|Present CPU encode begins/ends|A second encoder records the identity-present pass and optional native overlay (`crates/ember-engine/src/renderer.rs:727-796`).|No.|CPU present/overlay encode duration is unknown.|
|Present GPU pass begins/ends|Present and overlay render-pass descriptors both set `timestamp_writes: None` (`crates/ember-engine/src/renderer.rs:733-746`, `crates/ember-engine/src/renderer.rs:776-788`).|No.|GPU present and overlay duration are unknown.|
|Queue submission|Scene and present command buffers are submitted together and the returned value is not retained (`crates/ember-engine/src/renderer.rs:798-799`).|No.|The program knows only that it invoked submission; it cannot attribute CPU queue delay or join completion to a frame.|
|Surface present/handoff|`surface_tex.present()` is called immediately after submission (`crates/ember-engine/src/renderer.rs:798-800`).|No.|This is the last engine-controlled presentation call, but v7 records neither entry nor return and observes no compositor, scanout, or photon time.|

**Verified.** The current renderer also requests `wgpu::Features::empty()`, so it does not enable optional GPU timestamp-query support during device creation (`crates/ember-engine/src/renderer.rs:240-250`). The locked wgpu version is 24.0.5 (`Cargo.lock:3519-3541`).

**Verified.** The current `Frame` contains only a camera and instances, while the current `SceneTargets` contains only texture views and dimensions; neither can carry input, sim, or render provenance (`crates/ember-engine/src/renderer.rs:118-123`, `crates/ember-engine/src/renderer.rs:146-154`).

### 3.2 Existing local timing artifacts and their exact questions

**Verified — local artifact inventory.** Each artifact below is retained or emitted by the engine; none is a frame-tagged stage distribution (`crates/ember-engine/src/app.rs:258-290`, `crates/ember-engine/src/renderer.rs:532-548`, `crates/ember-engine/src/overlay.rs:60-92`).

|Artifact|Exact question it answers|What it does not answer|
|--------|-------------------------|-----------------------|
|`raw_gap = now - last_frame`|How much monotonic wall time elapsed between the starts of two handled redraws (`crates/ember-engine/src/app.rs:258-263`).|Which part was event-loop wait, sim, CPU render, queue wait, GPU work, present blocking, browser scheduling, or compositor delay.|
|`dt = min(raw_gap, 0.1 s)`|What clamped elapsed value the game receives for this update (`crates/ember-engine/src/app.rs:258-263`, `crates/ember-engine/src/lib.rs:29-32`).|Actual latency after a gap over 100 ms; the clamp deliberately destroys that equality.|
|frame-stall warning|Whether a redraw gap exceeded 100 ms, with at most one warning per second (`crates/ember-engine/src/app.rs:38-39`, `crates/ember-engine/src/app.rs:265-277`).|A distribution, root cause, or causal frame/input identity.|
|`last_scene_at` and `scene_age_ms()`|On native, how long it has been since the latest due scene pass set its start time (`crates/ember-engine/src/renderer.rs:532-548`, `crates/ember-engine/src/renderer.rs:632-637`).|World age, scene duration, GPU completion, present age, or input latency.|
|scene-Hz cap|Whether enough elapsed wall time has passed to start another native scene pass; wasm always renders one (`crates/ember-engine/src/renderer.rs:605-612`).|Actual scene Hz or any latency percentile.|
|overlay presenter FPS|An exponential moving average of reciprocal redraw `dt`, with weights 0.95 old and 0.05 new (`crates/ember-engine/src/overlay.rs:65-76`, `crates/ember-engine/src/overlay.rs:90`).|A present-completion rate, a percentile, or any individual stage duration.|
|overlay scene-frame age|One instantaneous `scene_age_ms()` value sampled while building UI before `render_with_overlay` starts (`crates/ember-engine/src/app.rs:283-297`, `crates/ember-engine/src/overlay.rs:60-92`).|If the same call renders a new scene, the label was computed from the previous `last_scene_at`; it is not a stamp on the image being handed off.|

**Derived.** At the documented 15 Hz cap and 60 Hz presentation rate, three of four presents reuse a scene frame because `60 / 15 = 4` presents per scene and `4 - 1 = 3` are re-presents; that is a useful stress condition, not a latency measurement (`docs/input-latch.md:26`).

## 4. What v7 measures on the network fork

### 4.1 Client input to socket

**Verified — client boundary inventory.** The shooter constructs a held-input message on the update thread, after which native and wasm use different send paths (`crates/pong/src/online.rs:693-727`, `crates/pong/src/online.rs:967-1186`).

|Boundary|Current evidence|Timestamp?|
|--------|----------------|----------|
|Input consumed and intent built|The shooter derives aim, movement, fire, and stance from the current input, then constructs `C2S::Input` on a 0.05 s cadence (`crates/pong/src/online.rs:639-727`).|No. The cadence uses accumulated, clamped game `dt`, not an `Instant` at construction (`crates/pong/src/online.rs:404-409`, `crates/pong/src/online.rs:693-705`).|
|Native client enqueue|`NetChan::send` clones the message into an unbounded mpsc channel (`crates/pong/src/online.rs:1009-1010`, `crates/pong/src/online.rs:1063-1068`).|No. The call marks enqueue, not socket send.|
|Native WebSocket send call|A worker drains the channel and calls `ws.send` (`crates/pong/src/online.rs:1014-1033`).|No. Queue residence, serialization, send-call duration, and kernel/network departure are unmeasured.|
|Wasm WebSocket send call|When open, the main thread serializes and invokes `send_with_str`; otherwise it queues text until `onopen` (`crates/pong/src/online.rs:1093-1101`, `crates/pong/src/online.rs:1131-1139`, `crates/pong/src/online.rs:1166-1176`).|No. Browser buffer residence and network departure are hidden and unstamped.|

**Verified.** `C2S::Input.seq` and `PState.ack` give a causal acknowledgement path, while `view_tick` identifies the remote simulation view used for lag compensation (`crates/pong-core/src/proto.rs:35-61`, `crates/pong-core/src/proto.rs:94-115`). They identify which command was applied and which simulated view was claimed; they contain no timestamp or accumulated duration.

**Verified.** `Cmd.sent_at` is a float copied from the shooter's accumulated game time and is used only to replay unacknowledged movement for prediction (`crates/pong/src/online.rs:231-238`, `crates/pong/src/online.rs:559-575`, `crates/pong/src/online.rs:699-705`). It does not measure socket send or acknowledgement latency, and clamped `dt` means it is not an exact wall clock after stalls (`crates/ember-engine/src/app.rs:258-263`).

**Verified.** The shooter's other time-like fields are also `dt` accumulators: `time`, `since_input`, `since_ping`, `since_status`, `since_score_ui`, `bullets_age`, and interpolation `t` drive prediction, animation, send cadence, keepalive, and UI cadence (`crates/pong/src/online.rs:248-278`, `crates/pong/src/online.rs:404-409`, `crates/pong/src/online.rs:740-770`). They answer gameplay/scheduling questions in clamped game time and retain no stage samples.

### 4.2 Server receive to state broadcast

**Verified — server boundary inventory.** The arena server crosses a connection thread, an mpsc event channel, the single hub/sim thread, a bounded outbound channel, and the connection thread again (`crates/pong-server/src/lib.rs:204-207`, `crates/pong-server/src/lib.rs:252-324`, `crates/pong-server/src/lib.rs:326-441`).

|Boundary|Current evidence|Timestamp?|
|--------|----------------|----------|
|Server WebSocket read/parse|The connection thread reads a text frame, parses `C2S`, and sends `Ev::Msg` to the hub (`crates/pong-server/src/lib.rs:291-302`).|No socket-arrival or parse-complete stamp.|
|Hub dequeue and input storage|The hub drains events before each tick; `handle_event` writes `last_seen = Instant::now()` and stores held input, sequence, and clamped view tick (`crates/pong-server/src/lib.rs:335-354`, `crates/pong-server/src/lib.rs:557-565`, `crates/pong-server/src/lib.rs:796-845`).|Partial: `last_seen` is an overwritten connection-health time sampled at hub handling, not attached to the input.|
|Authoritative sim application|The next tick reads the latest held input and steps the lobby (`crates/pong-server/src/lib.rs:355-386`).|Logical `sim.tick` exists; no wall-clock apply stamp and no receive-to-apply duration.|
|State construction/serialization/enqueue|Every second server tick constructs one `S2C::State`, includes each player's latest ack, serializes once, and `try_send`s it to each connection queue (`crates/pong-server/src/lib.rs:395-441`, `crates/pong-server/src/lib.rs:487-494`).|Tick and ack exist; construction, serialization, and outbound-queue stamps do not.|
|Server WebSocket send call|The connection thread drains its bounded outbound queue and calls `ws.send` (`crates/pong-server/src/lib.rs:252-290`).|No enqueue-to-send or call-duration stamp.|
|Client receive callback/thread and game poll|Native parses on the worker and enqueues to `in_rx`; wasm parses in `onmessage` and appends to `inbox`; the game drains at its next update (`crates/pong/src/online.rs:1033-1069`, `crates/pong/src/online.rs:1117-1127`, `crates/pong/src/online.rs:1179-1181`, `crates/pong/src/online.rs:415-420`).|No receive, queue, or consume stamp.|

**Verified.** The arena server has a real transport timing artifact: it stamps a WebSocket ping send and converts pong elapsed time to integer milliseconds, then smooths the result into `rtt_ticks` for rewind bounds (`crates/pong-server/src/lib.rs:264-311`, `crates/pong-server/src/lib.rs:542-551`, `crates/pong-server/src/lib.rs:822-843`). That answers an occasional connection-level WebSocket RTT question; it is not attached to gameplay input, not split into uplink/server/downlink, not retained as a distribution, and not surfaced as p50/p95/p99.

**Verified.** The remaining server `Instant` values schedule ticks and reports or enforce handshake, silence, Hello, and lobbyless timeouts; the only latency-like emitted number is a `behind_ms` value when the hub falls more than ten ticks behind (`crates/pong-server/src/lib.rs:50-68`, `crates/pong-server/src/lib.rs:326-364`, `crates/pong-server/src/lib.rs:449-475`, `crates/pong-server/src/lib.rs:523-560`). They answer liveness and catastrophic-stall questions, not receive-to-broadcast latency.

**Verified — adjacent artifact, not this path.** The separate postcard/TCP `ember-server` records maximum tick-body busy time and overrun count for each ten-second report (`crates/ember-server/src/lib.rs:157-173`, `crates/ember-server/src/lib.rs:283-309`). The shipped online shooter uses the WebSocket/JSON `pong-server` path described above (`crates/pong/src/online.rs:967-1186`, `crates/pong-server/src/lib.rs:1-15`), so that maximum does not instrument the assessed path and is not a distribution in any case.

## 5. Statistics verdict

**Verified.** Nothing on the assessed path owns a histogram, sample reservoir, quantile sketch, or percentile extractor. The overlay owns only one `fps_smoothed: f32`; the renderer owns one `last_scene_at`; the client command history owns logical command fields; and the arena server connection owns health times plus one smoothed RTT-in-ticks value (`crates/ember-engine/src/overlay.rs:20-29`, `crates/ember-engine/src/renderer.rs:193-201`, `crates/pong/src/online.rs:231-280`, `crates/pong-server/src/lib.rs:90-108`).

**Verified — statistics verdict.** The scene-age label is instantaneous, presenter FPS is an exponential moving average, stall and `behind_ms` values are threshold-triggered individual events, the adjacent server reports a window maximum and count, and the arena RTT is a smoothed latest value. None can yield p50, p95, or p99 after the underlying samples have been discarded (`crates/ember-engine/src/overlay.rs:69-92`, `crates/ember-engine/src/app.rs:265-277`, `crates/pong-server/src/lib.rs:355-364`, `crates/ember-server/src/lib.rs:283-309`, `crates/pong-server/src/lib.rs:542-551`). The percentile half of the requirement is therefore unmet.

## 6. Proposed latency-accounting architecture

### 6.1 One trace clock per process, causal IDs across processes

**Proposed.** Add a crate-internal `latency.rs` beside the engine's existing private `app` and `input` modules (`crates/ember-engine/src/lib.rs:9-15`). `TraceClock` owns one `web_time::Instant` epoch and returns `TraceStamp(u64)` in integer microseconds since that epoch; the dependency is already selected because it maps to native time and browser Performance time (`crates/ember-engine/Cargo.toml:6-15`). Stamps are comparable only inside one process and one epoch.

**Proposed.** Reuse the input event sequence, `SceneFrame.seq`, `C2S::Input.seq`, `PState.ack`, and server tick as causal join keys, but never subtract a client stamp from a server stamp. The protocol already echoes the latest applied input sequence in state (`crates/pong-core/src/proto.rs:58-61`, `crates/pong-core/src/proto.rs:94-115`); the client should retain `seq → enqueue stamp` until ack so it can observe client-local enqueue-to-authoritative-state-return without putting a wall clock on the wire.

### 6.2 Timestamped input latch

**Proposed.** Replace the current scalar mouse accumulator fields at `crates/ember-engine/src/input.rs:8-19` with the adopted `mouse_total`, but make each consumer mark `InputMark { total, through_event_seq, read_at }`. Add a fixed ring of `InputEventStamp { seq, arrived_at, kind }`, fed at the actual winit handlers before the writes at `crates/ember-engine/src/app.rs:129-137` and `crates/ember-engine/src/app.rs:159-192`. `delta_since_sim` and `delta_since_view` return the delta plus the unconsumed event-sequence range and oldest/newest arrival stamps; the engine stamps `read_at` immediately before the consumer call and advances `total`/`through_event_seq` immediately after it, at the split points around `crates/ember-engine/src/app.rs:280-281`.

**Proposed.** A button or key transition receives one event ID; continued held-state frames do not masquerade as new clicks. If the fixed event ring overruns before a consumer advances, increment `input_provenance_dropped` and mark that trace incomplete instead of silently pairing a frame with the wrong click. This lands at the current key/button mutation seams (`crates/ember-engine/src/input.rs:71-84`) and focus-loss clear seam (`crates/ember-engine/src/input.rs:97-101`).

### 6.3 Frame-tagged CPU and GPU boundaries

**Proposed.** When current `SceneTargets` becomes the designed `SceneFrame` at `crates/ember-engine/src/renderer.rs:146-154`, add `trace: SceneTrace` beside the planned `sim_time` and `seq`. Its exact fields are `input_first_seq`, `input_last_seq`, `input_oldest_at`, `input_newest_at`, `sim_read_at`, `sim_done_at`, `scene_encode_begin_at`, `scene_encode_end_at`, and `scene_submit_at`; absent input is represented explicitly. `sim_time` remains deterministic world time and is not reused as a wall stamp (`docs/presenter-architecture.md:30-67`).

**Proposed.** Stamp `sim_read_at` immediately before and `sim_done_at` immediately after `game.update` at `crates/ember-engine/src/app.rs:280`; stamp scene encode around the current due branch and `scene_enc.finish()` at `crates/ember-engine/src/renderer.rs:632-725`; stamp scene submit where the architecture splits the current combined submit at `crates/ember-engine/src/renderer.rs:798-799`. A frame keeps all of these values for every later re-present.

**Proposed.** A present is not a mutable field on `SceneFrame`, because one scene may be presented several times. Create an ephemeral `PresentTrace { scene_seq, view_input_range, present_seq, view_read_at, acquire_begin_at, acquire_end_at, present_encode_begin_at, present_encode_end_at, present_submit_at, surface_present_enter_at, surface_present_return_at }` at the present call. The seams are surface acquisition at `crates/ember-engine/src/renderer.rs:614-630`, present encoder/pass at `crates/ember-engine/src/renderer.rs:727-750`, optional overlay at `crates/ember-engine/src/renderer.rs:752-796`, queue submit at `crates/ember-engine/src/renderer.rs:798-799`, and surface handoff at `crates/ember-engine/src/renderer.rs:800`.

**Proposed.** Add GPU query pairs for scene, present, and overlay at the three descriptors that currently set `timestamp_writes: None` (`crates/ember-engine/src/renderer.rs:681-709`, `crates/ember-engine/src/renderer.rs:733-746`, `crates/ember-engine/src/renderer.rs:776-788`). At adapter/device setup, inspect support and request only available timestamp features instead of unconditional `Features::empty()` (`crates/ember-engine/src/renderer.rs:222-250`). Resolve results into a readback ring keyed by `scene_seq`/`present_seq`; unsupported adapters increment an unavailable counter and continue with CPU metrics. Query pairs supply GPU pass durations, not a CPU-comparable GPU start time; absent a calibrated cross-clock API, exact queue wait remains unavailable and must not be fabricated.

**Derived.** CPU submit/handoff stamps and later-resolved GPU durations occupy overlapping lanes, so the overlay must draw them as a timeline and must not sum them. Useful distributions are input-arrival→sim-read, sim update, scene CPU encode, GPU scene, late-input-arrival→view-read, acquire, present CPU encode, GPU present, and input-arrival→surface-present-handoff. If a queue work-done callback is available, submit→callback is a coarse queue-plus-GPU-plus-callback total, not exact queue wait. The residual between named same-clock CPU boundaries is displayed as `unattributed`, while cross-clock queue wait is displayed as `unavailable` unless a calibrated API supplies it.

**Proposed — local stage taxonomy.** These stable IDs are the metric and trace vocabulary; begin/end pairs produce a duration, while point boundaries join adjacent stages.

|ID|Named boundary|Clock/carrier|Exact landing seam|
|--|--------------|-------------|------------------|
|`I0 input_arrived`|winit callback entry|CPU `InputEventStamp`|`crates/ember-engine/src/app.rs:129-137`, `crates/ember-engine/src/app.rs:159-192`|
|`I1 sim_read`|game consumer entry|CPU `InputMark`/`SceneTrace`|`crates/ember-engine/src/app.rs:280-281`|
|`I2 sim_done`|game returns `Frame`|CPU `SceneTrace`|`crates/ember-engine/src/app.rs:280-281`|
|`R0 scene_encode`|begin/end scene CPU recording|CPU `SceneTrace`|`crates/ember-engine/src/renderer.rs:632-725`|
|`R1 scene_submit`|scene command buffer handed to queue|CPU `SceneTrace`|split the combined call at `crates/ember-engine/src/renderer.rs:798-799`|
|`G0 scene_gpu`|begin/end scene render pass|GPU query pair keyed by scene sequence|`crates/ember-engine/src/renderer.rs:681-709`|
|`I3 view_read`|late input/view consumer entry|CPU `PresentTrace`|the presenter call that replaces `crates/ember-engine/src/app.rs:297-301`|
|`P0 acquire`|begin/end surface acquisition|CPU `PresentTrace`|`crates/ember-engine/src/renderer.rs:614-630`|
|`P1 present_encode`|begin/end present plus overlay CPU recording|CPU `PresentTrace`|`crates/ember-engine/src/renderer.rs:727-796`|
|`P2 present_submit`|present command buffer handed to queue|CPU `PresentTrace`|split the combined call at `crates/ember-engine/src/renderer.rs:798-799`|
|`G1 present_gpu`|begin/end present and overlay passes|GPU query pairs keyed by present sequence|`crates/ember-engine/src/renderer.rs:733-788`|
|`P3 present_handoff`|enter/return surface present|CPU `PresentTrace`|`crates/ember-engine/src/renderer.rs:800`|

### 6.4 Network boundaries and clock discipline

**Proposed.** On native, change the client channel payload at `crates/pong/src/online.rs:1009-1025` from bare `C2S` to a stamped envelope carrying sequence, intent-built time, and channel-enqueue time; record worker-dequeue and `ws.send` entry/return there. On wasm, record the same boundaries around serialization, pending-queue insertion/drain, and `send_with_str` at `crates/pong/src/online.rs:1104-1176`. A successful WebSocket send call is labeled `socket API accepted`, not `bytes left host`.

**Proposed.** Stamp native receive after `ws.read` and parse and stamp wasm receive at the start of `onmessage`, carry that stamp through each inbound queue, and record game consumption when `ShooterGame::update` drains it (`crates/pong/src/online.rs:1033-1069`, `crates/pong/src/online.rs:1117-1127`, `crates/pong/src/online.rs:415-420`). On ack, observe client-local intent-enqueue→state-receive and state-receive→game-consume distributions keyed by the existing sequence.

**Proposed.** Extend `Ev::Msg` with `socket_received_at` and `parsed_at` where the server currently sends the bare event (`crates/pong-server/src/lib.rs:70-88`, `crates/pong-server/src/lib.rs:291-297`). Carry a stamped input record instead of `(PlayerIn, seq, view_tick)` at `crates/pong-server/src/lib.rs:110-121` and `crates/pong-server/src/lib.rs:829-844`; record hub-dequeue, sim-apply, state-build, serialization, outbound enqueue, connection-thread dequeue, and `ws.send` entry/return at `crates/pong-server/src/lib.rs:335-386`, `crates/pong-server/src/lib.rs:395-441`, `crates/pong-server/src/lib.rs:487-494`, and `crates/pong-server/src/lib.rs:276-290`.

**Derived.** Without synchronized clocks, uplink and downlink cannot be separated from client/server stamps. Honest products are client-local round trip, server-local receive-to-broadcast, and independently sampled WebSocket RTT; one-way latency remains unavailable. Existing `seq`/`ack` is enough to join the input to the first state that acknowledges it, while server-local stamps identify where that server residence was spent (`crates/pong-core/src/proto.rs:58-61`, `crates/pong-server/src/lib.rs:395-417`).

**Proposed — network stage taxonomy.** Use `C0 intent_built`, `C1 client_enqueued`, `C2 client_socket_accepted`, `S0 server_socket_received`, `S1 hub_dequeued`, `S2 input_applied`, `S3 state_built`, `S4 server_enqueued`, `S5 server_socket_accepted`, `C3 state_received`, and `C4 state_consumed`. Client seams are intent construction and both platform channels (`crates/pong/src/online.rs:693-727`, `crates/pong/src/online.rs:1009-1069`, `crates/pong/src/online.rs:1093-1181`); server seams are read/parse, hub drain, sim step, state construction, outbound channel, and send (`crates/pong-server/src/lib.rs:276-302`, `crates/pong-server/src/lib.rs:335-441`, `crates/pong-server/src/lib.rs:487-494`).

### 6.5 Fixed-size distributions

**Proposed.** Implement `LatencyHistogram` in engine `latency.rs` at the module seam `crates/ember-engine/src/lib.rs:9-15` and the same small type in `pong-server` beside its timing imports at `crates/pong-server/src/lib.rs:17-31`. Use 193 `u32` buckets: 128 linear buckets of 0.125 ms through 16 ms, 64 logarithmic buckets at eight buckets per octave through 4096 ms, and one overflow bucket; retain `count`, `sum`, `min`, and `max` separately.

**Derived.** The bucket array costs `193 × 4 = 772` bytes per metric; eighteen stage metrics cost `772 × 18 = 13,896` bytes before small counters. Insertion is one clamp/index/increment operation. Percentile extraction finds rank `ceil(p × count)` by scanning at most 193 buckets and reports the containing interval, not false point precision.

**Proposed.** Keep five two-second epochs and merge them only when the overlay refreshes, yielding a rolling ten-second view while keeping per-event insertion constant-time. Saturating counters, explicit overflow, epoch reset count, provenance-drop count, and unsupported-GPU count keep failure visible. Timer-call overhead and practical browser precision are measurement items, not assumed free.

### 6.6 Honest observation boundaries

**Verified — native current boundary.** The native path submits command buffers and then calls `SurfaceTexture::present`; the tree invokes no platform presentation-timing API after that call (`crates/ember-engine/src/renderer.rs:798-800`). Therefore the CPU observation boundary is `surface present return`. GPU timestamp queries can additionally locate the end of Ember's present render pass, but neither point is physical scanout or photon emission.

**Proposed — native label.** Report `input→present handoff` and `GPU present pass`, side by side. Never rename either to click-to-photon. Optional platform-specific display-timing integration may extend the boundary later, but it must be a separate adapter and must report unsupported rather than estimate.

**Verified — wasm current boundary.** The wasm loop is rAF-driven through winit and uses the same renderer `present()` call, while its monotonic `Instant` dependency is documented as Performance-backed on web (`crates/ember-engine/src/app.rs:322-352`, `crates/ember-engine/Cargo.toml:13-15`, `crates/ember-engine/src/renderer.rs:798-800`). The code receives no browser compositor, scanout, or paint-completion stamp.

**Requires measurement — wasm clock.** Browser timer precision and clamping vary with browser, context, and policy and cannot be recovered from this tree. Record the observed minimum nonzero delta, repeated-value rate, and call-pair overhead for each supported browser/configuration; histogram output must show the measured clock quantum and refuse sub-quantum precision.

**Proposed — wasm label.** The last honest CPU endpoint is `WebGPU surface present handoff`; optional GPU query results are reported only when feature negotiation succeeds. Everything the browser does after handoff remains `browser/compositor/display — unobserved`.

### 6.7 Overlay and capture surface

**Proposed.** Extend the native ATW window where it currently renders presenter FPS and scene age (`crates/ember-engine/src/overlay.rs:77-93`). The compact view shows total input→handoff p50/p95/p99, scene CPU/GPU, present CPU/GPU, submission-to-completion where available, acquire, scene age, sample count, and a red `unobserved/unsupported/dropped` row; expanding a row shows the ten-second histogram and the latest trace timeline.

**Proposed.** Build the overlay snapshot before its presenter pass as today, but label it as the previous completed statistics epoch; the current ordering builds UI before `render_with_overlay` (`crates/ember-engine/src/app.rs:283-297`). The overlay's own encode/GPU cost is measured on the next resolved sample so instrumentation does not claim knowledge of a pass that has not run yet.

**Proposed.** Keep metrics collection target-neutral even though egui is native-only (`crates/ember-engine/src/lib.rs:9-15`, `crates/ember-engine/Cargo.toml:17-35`). For wasm, expose the same immutable snapshot as JSON from the wasm API beside the existing exports at `crates/pong/src/lib.rs:236-270`; a future versioned arena page may render it in a DOM panel beside the existing page overlays (`web/games/arena/v7/index.html:115-135`). Do not mutate the frozen v7 page merely to add diagnostics.

## 7. Declared measurement items

**Requires measurement — M1, clock cost and resolution.** Add a `latency_clock` benchmark that performs one million back-to-back stamp pairs and reports call-pair p50/p95/p99, minimum nonzero delta, and repeated-value rate. Native server-lane command: `/usr/bin/time -p cargo bench -p ember-engine --bench latency_clock -- --pairs 1000000`. Browser harness: `node scripts/measure-latency-web.mjs --browser chromium --pairs 1000000 --url http://127.0.0.1:8000/web/tests/latency-clock.html`, repeated for each supported browser and isolation configuration.

**Requires measurement — M2, GPU feature and readback cost.** Add `scripts/latency-gpu.mjs` or a native equivalent that records adapter/backend, negotiated timestamp features, empty-pass query cost, query-resolution delay, and instrumentation-on/off frame distributions. Native command: `EMBER_LATENCY=1 /usr/bin/time -p cargo run --release -p pong --bin pong-app`; wasm command after serving the built v7 successor: `node scripts/latency-gpu.mjs --url http://127.0.0.1:8000/games/arena/v8/ --seconds 120`.

**Requires measurement — M3, local input-to-handoff.** With the proposed trace capture enabled, run the native shooter against loopback for two minutes while an input injector emits a seeded mix of button and relative-motion events. Server command: `/usr/bin/time -p cargo run --release -p pong-server -- --bind 127.0.0.1:7778`. Client harness: `/usr/bin/time -p scripts/latency-input-run.sh --url ws://127.0.0.1:7778 --seconds 120 --seed 7 --output latency-native.ndjson`. The harness must fail if any required stage lacks samples, if causal ranges drop, or if percentile order is not `p50 ≤ p95 ≤ p99`.

**Requires measurement — M4, server receive-to-broadcast.** Run the instrumented existing headless bot against loopback and a shaped-delay lane, exporting server histograms at shutdown. Loopback command: `/usr/bin/time -p cargo run --release -p pong-server --example wsbot -- ws://127.0.0.1:7778 create lagtrace - observer 120`. Network-shaped harness: `/usr/bin/time -p scripts/latency-server-run.sh --delay-ms 40 --jitter-ms 10 --loss-pct 0.1 --seconds 120 --output latency-server.ndjson`. Compare receive→hub, hub→apply, apply→broadcast-enqueue, and enqueue→WebSocket-send percentiles; do not infer one-way transit.

**Requires measurement — M5, browser clamping and hidden tail.** Run the same seeded input capture in normal and background-throttled browser contexts with `node scripts/latency-browser-run.mjs --browser chromium --url http://127.0.0.1:8000/games/arena/v8/ --seconds 120 --seed 7 --output latency-web.ndjson`. Record rAF/redraw gaps, stamp quantum, WebGPU feature support, input→handoff percentiles, and the count of traces whose post-handoff display time is unavailable.

**Requires measurement — M6, physical end to photon.** Engine instrumentation cannot supply this number. A hardware rig must toggle an input LED on the same actuator edge that clicks, film the LED and a deterministic full-screen response at a calibrated high frame rate, and compare video-derived click→photon against the trace's click→handoff for the same trace IDs. Analysis command: `python3 tools/latency_photon.py --video capture.mp4 --config tools/latency-rois.json --trace latency-native.ndjson --output latency-photon.json`. Report camera frame quantum, display refresh mode, input device polling mode, p50/p95/p99, and unmatched traces.

## 8. Scope boundary and implementation order

**Proposed.** Keep hardware switch debounce, USB polling before winit delivery, browser/native compositor scheduling after surface handoff, scanout, panel response, and photon emission outside the software total; show them as named unobserved regions. Keep globally synchronized client/server clocks, one-way network attribution, production telemetry export, user input contents, and a general distributed-tracing framework out of this engine-sized first implementation.

**Proposed.** Land in four independently useful steps at the cited seams: CPU trace clock, timestamped input marks, frame/present traces, and histograms in `input.rs`, `app.rs`, the `SceneFrame` promotion seam, and `overlay.rs` (`crates/ember-engine/src/input.rs:8-19`, `crates/ember-engine/src/app.rs:129-137`, `crates/ember-engine/src/app.rs:280-301`, `crates/ember-engine/src/renderer.rs:146-154`, `crates/ember-engine/src/overlay.rs:60-110`); conditional GPU queries at device/pass construction (`crates/ember-engine/src/renderer.rs:240-250`, `crates/ember-engine/src/renderer.rs:681-709`, `crates/ember-engine/src/renderer.rs:733-788`); client network envelopes and ack histograms (`crates/pong/src/online.rs:693-727`, `crates/pong/src/online.rs:967-1186`); then server stamped events and outbound envelopes (`crates/pong-server/src/lib.rs:70-121`, `crates/pong-server/src/lib.rs:252-311`, `crates/pong-server/src/lib.rs:335-441`).

**Derived — final answer.** Today Ember can say that a redraw stalled, approximately how often redraws arrive, how long since a native scene pass began, which input sequence a server state acknowledges, and an occasional smoothed connection RTT. It cannot say where each millisecond from input delivery to presentation handoff was spent, and it cannot state p50/p95/p99 for any stage. The adopted architecture supplies the seams to fix that without redesigning the data flow; timestamped carriers, conditional GPU queries, honest per-process boundaries, and fixed-size distributions are the missing layer.
