# The five state models

*Scope: a conceptual frame for the whole engine, written above the five documents that precede it. Claims are labelled **verified** (read out of the tree at the cited lines), **derived** (argued here from verified inputs), or **proposed** (a design suggestion, not a description). No numbers here were measured; nothing in this document was run.*

## 1. The thesis

Ember's data exists at five distinct levels, and only two of them have names in the code. The corpus that precedes this document is, read in hindsight, five separate defences of four boundaries between those levels — the presenter split defends one, the input latch defends another, the latency taxonomy stamps clocks on the crossings, the weak-device assessment finds a boundary that has already failed, and the scene-timing controller establishes which levels an adaptive policy is allowed to touch. Each was reached independently, from its own starting question. That they converge is the argument for naming the levels explicitly.

The five levels are the abstract scene concept, network state in its two forms, local state, rendered state, and presentation state. The claim of this document is narrow and checkable: **most of the corpus's hard-won findings are consequences of two levels being conflated, and the conflation is legible in the type that holds both.** Where a single struct spans two levels, the corpus found a bug or a blocked design; where the levels happen to be separated, it found nothing to report. §8 lists the conflations in v7 and §7 maps the corpus onto the boundaries.

This is a frame, not a refactor. Nothing below requires moving code to be useful — the levels are already there, doing their jobs, mostly correctly. What is missing is the vocabulary to say which one a given field belongs to, and the vocabulary is what makes the next several design questions answerable instead of arguable.

## 2. The five levels

**Level 1 — the abstract scene concept.** What the game *has*: entities, their properties, their relationships. The arena has obstacles with extents; players have a position, an aim, a stance, a weapon, a health value; bullets have a position, a velocity, an owner, and a lifetime. This is the AST of the game's content, and every other level is a projection, encoding, or sample of it. **Verified as an absence:** level 1 is not a data structure anywhere in the tree. There is no schema, no IDL, no generated types. It exists implicitly in three independent hand-written places — the sim structs (`crates/arena-core/src/shooter.rs:236-265`), the protocol types (`crates/arena-core/src/proto.rs:36-73`), and the instance-construction code that turns state into draw calls (`crates/arena/src/online.rs:788-961`) — and nothing checks them against each other. §5.4 shows a live drift between two of the three.

**Level 2 — network state, in two forms.** The wire is a codec over the scene (§5), and the two directions are not symmetric enough to be one thing.

*Level 2a, sent state:* the encoding of our intents against the local model that we most recently transmitted. **Verified:** the shooter's is `C2S::Input` (`crates/arena-core/src/proto.rs:96-115`), built and sent on a fixed 0.05 s cadence of accumulated client game time (`crates/arena/src/online.rs:695-727`). Its identity is a client-assigned `seq` (`crates/arena-core/src/proto.rs:97-99`, assigned at `crates/arena/src/online.rs:697-698`). The system must track the difference between what was sent and the local model as that model keeps evolving; in v7 that difference is the unacked command history (`crates/arena/src/online.rs:232-238`, `crates/arena/src/online.rs:699-709`).

*Level 2b, received state:* the authoritative encoding arriving from the network. **Verified:** `S2C::State` (`crates/arena-core/src/proto.rs:151-159`), broadcast every second sim tick (`crates/arena-core/src/proto.rs:14-15`, gated at `crates/arena-server/src/lib.rs:395`). Its identity is the server's `tick` (`crates/arena-core/src/proto.rs:152`) plus a per-player `ack` echoing the last input the server applied from that player (`crates/arena-core/src/proto.rs:58-61`, filled at `crates/arena-server/src/lib.rs:416`). It is applied onto the local model and overrides it; what remains unacked is then re-applied on top (`crates/arena/src/online.rs:559-589`).

**Level 3 — local state.** The live local model: deterministic simulation plus prediction. On the server this is the whole authoritative sim (`crates/arena-core/src/shooter.rs:272-283`), stepped at a fixed `FIXED_DT` of 1/60 s (`crates/arena-core/src/shooter.rs:5`, `crates/arena-core/src/shooter.rs:349-352`). On the arena client it is far smaller than the name suggests — §8.2 shows it is essentially one predicted position.

**Level 4 — rendered state.** What the renderer actually consumed for a given frame: the sampled snapshot handed to the scene pass. **Verified:** in v7 this is `Frame { camera, instances }` (`crates/ember-engine/src/renderer.rs:118-123`), built once per update and consumed by the scene pass, whose only use of the camera is to compute and upload one view-projection matrix (`crates/ember-engine/src/renderer.rs:638-640`).

**Level 5 — presentation state.** What actually reached the screen: post-warp, post-throttle, possibly a re-presented or reprojected older rendered state. The ATW decision exists precisely because 4 and 5 are distinct — the scene renderer never touches the swapchain, and a cheap warp pass owns presentation (`docs/atw-first-rendering.md:3-8`).

### 2.1 Clock, owner, identity

|Level|Clock|Owner|Identity in v7|
|---|---|---|---|
|1 abstract scene|none — atemporal|nobody|none|
|2a sent|client accumulated game time (`crates/arena/src/online.rs:405`)|the client|`seq: u32` (`crates/arena-core/src/proto.rs:97-99`)|
|2b received|server fixed tick (`crates/arena-core/src/shooter.rs:349-352`)|the server|`tick: u64` + per-player `ack` (`crates/arena-core/src/proto.rs:152`, `:58-61`)|
|3 local|client render `dt` (`crates/ember-engine/src/app.rs:258-263`)|the client|none|
|4 rendered|scene clock, throttled (`crates/ember-engine/src/renderer.rs:605-612`)|the renderer|none (`crates/ember-engine/src/renderer.rs:118-123`)|
|5 presentation|display rate, rAF-driven|the presenter|none|

**Derived — the levels that have identity are exactly the ones that cross a process boundary.** Sequence numbers, ticks and acks exist at 2a and 2b, which are separated by a network; they are absent at 3, 4 and 5, which live inside one address space. Identity was added where a wire forced it and nowhere else. That is a natural way for a codebase to grow and it is also the root of §8's conflations: without an identity of its own, a level cannot be distinguished from the level it is stored beside, and the compiler cannot object. The presenter and latency documents both arrive at adding identity to level 4 — `seq`, `sim_time`, `input_mark` on `SceneFrame` (`docs/presenter-architecture.md:30-44`) and a `present_seq` on an ephemeral present trace (`docs/latency-observability.md:143-144`) — which under this frame is the same move, made twice, for two levels that had none.

### 2.2 The transitions

Each adjacency is a sampling, an encoding, or an override. Naming which one it is decides what may be lost at the crossing.

|From → to|Kind|v7 instance|
|---|---|---|
|1 → 3|instantiation|implicit; hand-written sim structs (`crates/arena-core/src/shooter.rs:236-265`)|
|1 → 2|encoding|implicit; hand-written protocol types (`crates/arena-core/src/proto.rs:36-73`)|
|3 → 2a|encoding (intent extraction)|`crates/arena/src/online.rs:715-726`|
|2b → 3|**override**, then re-derivation|`crates/arena/src/online.rs:561-589`|
|2b → 4|sampling, bypassing 3|remote players interpolated straight from received state (`crates/arena/src/online.rs:325-330`, `crates/arena/src/online.rs:856-900`)|
|3 → 4|sampling|`crates/arena/src/online.rs:772-787`|
|4 → 5|presentation (identity today)|`crates/ember-engine/src/present.wgsl:30-31`|

The 2b → 4 row is the one a clean five-level model does not predict, and it is not a defect: remote players are display-only, so routing them from received state straight to the renderer is cheaper and simpler than instantiating them into a local sim. §9 treats it as the place the model is least comfortable.

## 3. What each level is allowed to lose

A transition that is a *sampling* may lose freshness but not authority. A transition that is an *encoding* may lose precision and detail but must preserve meaning. A transition that is an *override* discards the destination wholesale and must therefore be followed by re-derivation of anything the source did not carry. Stating it that way makes several v7 behaviours predictable rather than incidental:

- The bullet extrapolation clamp exists because 2b → 4 is a sampling that loses lifetime. **Verified:** `BState` carries position, velocity and owner (`crates/arena-core/src/proto.rs:64-73`) while the sim's `Bullet` also carries `ttl`, `dmg` and the shooter's view `delay` (`crates/arena-core/src/shooter.rs:256-265`); the encoder drops all three (`crates/arena-server/src/lib.rs:419-430`). A client therefore cannot know when a bullet expires, so it may only extrapolate for a bounded time — which is exactly what the renderer does, clamping the extrapolation age to 0.12 s (`crates/arena/src/online.rs:902-912`, clamp at `:904`).
- The reconciliation replay exists because 2b → 3 is an override. The authoritative position carries no knowledge of the commands the client has issued since, so everything unacked must be re-applied after the override (`crates/arena/src/online.rs:566-575`).
- The warp is cosmetic because 4 → 5 is a presentation, not an authorship. The rule that the frame-relative view read must not reach anything sent, stored or ticked (`docs/input-latch.md:106-112`) is the same statement in the input latch's vocabulary.

## 4. Level 1: named by its absence

**Verified — there is no schema.** The scene concept is spelled out three times in three languages of the same codebase, and no tool relates them:

1. **The sim structs.** `PlayerSt` carries id, position, aim, hp, score, alive, crouch, weapon, ammo, a reload timer, a death count, a respawn timer and a cooldown (`crates/arena-core/src/shooter.rs:236-254`).
2. **The protocol types.** `PState` carries id, x, z, ax, az, hp, score, alive, crouch, weapon, ammo, a reloading *flag*, deaths, and ack (`crates/arena-core/src/proto.rs:36-62`).
3. **The mesh and instance tables.** The renderable form is a flat list of coloured boxes with a yaw and a mesh id (`crates/ember-engine/src/renderer.rs:58-69`), assembled by hand from the other two (`crates/arena/src/online.rs:788-961`).

The three are related by hand-written projections at exactly two places — the server's state encoder (`crates/arena-server/src/lib.rs:396-432`) and the client's instance builder — and by nothing else. Every field correspondence is a convention held in a maintainer's head.

**Verified — a live drift between two of the three, in one word.** `PlayerSt` has *two* fields that could be called deaths. `death_count` is documented as the authoritative death count and the scoreboard's DEATHS column (`crates/arena-core/src/shooter.rs:249-250`); a second, private `deaths` (`crates/arena-core/src/shooter.rs:253`) is not a death count at all — it is initialised to the player's join slot (`crates/arena-core/src/shooter.rs:320`) and incremented on respawn purely to rotate the spawn point (`crates/arena-core/src/shooter.rs:362-363`). The wire field named `deaths` (`crates/arena-core/src/proto.rs:56-57`) is encoded from `death_count`, not from `deaths` (`crates/arena-server/src/lib.rs:415`). So the identifier `deaths` denotes two different quantities in two structs that describe the same entity, and the encoder is the only place that knows which one is which.

Nothing is broken — the encoder is correct. The finding is that its correctness is unenforced and unstated, which is the general condition of every level 1 → 2 and level 1 → 3 correspondence in the tree. **Derived:** a renaming refactor that unified the two spellings, or a second encoder written by someone reading only `shooter.rs`, would produce a silently wrong scoreboard with no compile error and no test failure.

**Verified — the encoding is lossy in ways only level 1 could adjudicate.** `PlayerSt.reload_t` is a float countdown; `PState.reloading` is a bool derived as `reload_t > 0.0` (`crates/arena-server/src/lib.rs:414`). The client then reconstructs a reload *progress* for the viewmodel dip by starting its own local timer when the flag first goes true (`crates/arena/src/online.rs:505-510`, consumed at `crates/arena/src/online.rs:923-930`). That is a level-3 quantity destroyed by the level-2 encoding and re-manufactured at level 4 from the flag's edge. It looks right and it is cheap. It is also exactly the kind of decision that has no home: whether reload progress is part of the scene concept, or a display affordance, is a level-1 question, and there is no level 1 to ask.

**Resolved — the aim encoding used to drop a dimension, and the drop was not invisible after all.** The client had always integrated both yaw and pitch from mouse deltas and built a full 3D look vector for its camera, while the wire carried a 2D aim only, the sim stored a 2D aim, and remote players were therefore drawn aiming level. This was recorded here as a deliberate consequence of a 2D sim, invisible in play. It was not invisible: because bullets were 2D too and the hit volume was a cylinder of unbounded height, *where you looked had no bearing on where your shot went*. The client even papered over it, flying your own tracers along your true look ray while the authoritative path stayed flat — so the laser dot, the tracer and the actual bullet could all disagree at once.

The fix is not a 3D aim vector. `PlayerIn.aim` stays 2D and unit-length, and elevation rides beside it as a scalar `pitch` (`C2S::Input.pitch` on the wire, re-clamped server-side in the sim's sanitizer). `Bullet` gains scalar `y`/`vy` the same way, so horizontal speed stays `BULLET_SPEED` at any elevation and the TTL-bounded range does not shrink with the cosine. The hit volume gained a vertical band from shared `eye_h`/`body_h` constants that now live in `arena-core` rather than in the renderer — the point being that client and server must agree where a body *is*, which is exactly the level-1 question this document says has no home. Here it finally has one.

## 5. The network as a codec over the scene

The productive way to read `proto.rs` is not as a message list but as a codec: synchronisation is encode, transmit, decode, apply of scene state and scene intents. Assessed that way, several properties that look like arbitrary choices become positions on known axes.

### 5.1 What subset of the scene it encodes

**Verified.** The state broadcast encodes players, bullets and pad availability (`crates/arena-core/src/proto.rs:151-159`). It does *not* encode the arena. Obstacles and pad positions are never transmitted; the server sends a `seed` once at join (`crates/arena-core/src/proto.rs:137-144`) and every client generates the identical geometry locally with the same deterministic generator (`crates/arena-core/src/shooter.rs:88-113`, `crates/arena-core/src/shooter.rs:122-153`, invoked client-side at `crates/arena/src/online.rs:431-432`).

**Derived — the codec already contains one instance of the strongest available compression, and it is the one that depends on level 1 being shared.** Sending a generator instead of its output is only possible because both ends agree on what an obstacle *is* and how obstacles are produced from a seed. That agreement is level 1, and here it is load-bearing enough that the arena would not merely look wrong but would be *differently solid* on each machine if it drifted — the same `move_circle` runs against the client's locally generated obstacles for prediction (`crates/arena/src/online.rs:732-738`) and against the server's for authority (`crates/arena-core/src/shooter.rs:379`). The pads field is the same trick one step further: `pads: Vec<bool>` is index-aligned with positions the client derived itself (`crates/arena-core/src/proto.rs:155-158`, encoded at `crates/arena-server/src/lib.rs:431`), so the wire carries only the mutable bit of an entity whose immutable part is shared knowledge.

That is a static/dynamic split, applied to exactly one entity class, by hand. The codec view says the same split is available for every entity class, and that what stops it from generalising is that the shared knowledge is implicit — there is no artefact naming which properties are derivable-from-seed and which must be transmitted.

### 5.2 Full state, not deltas

**Verified.** Every state broadcast is complete: all players, all bullets, all pads, serialised once per lobby and sent to every member (`crates/arena-server/src/lib.rs:396-441`, single serialisation at `:433-434`). There is no baseline, no dirty tracking, and no per-recipient tailoring. The uplink is likewise unconditional: the client builds and sends a `C2S::Input` every 0.05 s whether or not anything changed (`crates/arena/src/online.rs:695-727`).

**Derived — the single-serialisation design is a real constraint on how deltas could land, and it is worth naming before someone breaks it.** The per-player `ack` rides *inside* each player's own record in the broadcast (`crates/arena-server/src/lib.rs:416`) rather than being a per-recipient field on the message. That is what allows one `serde_json::to_string` per lobby per broadcast instead of one per member. A naive delta codec keyed per recipient — "what has changed since *your* last acknowledged state" — would multiply serialisation cost by the member count, up to eight (`crates/arena-core/src/shooter.rs:71`). The delta design that preserves the current cost profile is a shared baseline: everyone diffs against the same previous broadcast, which is sound precisely because the broadcast is already identical for all recipients and is sent on a fixed cadence.

### 5.3 What the ack/seq machinery buys the sent-versus-local diff

**Verified.** `C2S::Input.seq` is client-assigned and echoed back in that player's `PState.ack` (`crates/arena-core/src/proto.rs:97-99`, `crates/arena-core/src/proto.rs:58-61`). The server stores the latest input per player as a triple of held intent, its sequence, and the client's claimed view tick (`crates/arena-server/src/lib.rs:119-120`), overwriting on each arrival (`crates/arena-server/src/lib.rs:829-844`), and echoes the stored sequence in the next broadcast.

**Verified — `C2S::Input` is a held-state encoding, not a command stream.** The doc comment says so (`crates/arena-core/src/proto.rs:94-95`) and the server implements it: the stored input is never cleared, so each tick's step reads whatever intent last arrived (`crates/arena-server/src/lib.rs:369-386`) and a client that stops transmitting keeps moving until it is timed out. **Derived:** this makes packet loss benign — a dropped input is not a lost command, it is a slightly stale held state, superseded 0.05 s later — and it is why the same message doubles as the keepalive.

**Derived — the client tracks that same wire as a command log, and the two models are not the same shape.** `Cmd` records a sequence, a movement vector, a speed and a send time (`crates/arena/src/online.rs:232-238`); the history is a deque capped at 64 entries (`crates/arena/src/online.rs:699-709`). The unacked suffix of that deque *is* the sent-versus-local diff: it is precisely the set of intents the authority has not yet reflected. So the ack machinery gives the diff tracking that reconciliation needs. What it does not give, and is not used for, is transmission economy — nothing consults the diff to decide whether to send. The codec view separates those two uses of the same mechanism, and v7 uses exactly one of them.

### 5.4 Codec version

**Verified.** The version constant is `PROTO_VERSION`, a `u16` currently at 7 (`crates/arena-core/src/proto.rs:10`), sent in `Hello` (`crates/arena-core/src/proto.rs:80-83`, built at `crates/arena/src/online.rs:119-123`) and retained per connection (`crates/arena-server/src/lib.rs:94-97`). The retained comment records the policy: any version may list lobbies, but entering a game requires the current one.

**Derived — under the codec reading, this is a codec version, and it is a single scalar covering three separable things**: the wire syntax, the encoded scene subset, and the simulation rules that make a seed produce the same arena on both ends. A change to any one of them requires a bump, so every change costs the same — which is the mechanism behind the compatibility problem the backlog already records, that frozen game versions become unjoinable on every protocol bump (`docs/plans/backlog.md:31`). **Proposed:** if that story is ever built, the codec frame suggests the split to make — a syntax version that governs whether a peer can parse at all, and a scene-concept version that governs whether two peers agree on what exists. `serde`'s field defaults already give forward-compatible *syntax* for additive changes (`crates/arena-core/src/proto.rs:49-61`), so the syntax half is largely solved and the unsolved half is the one that has no artefact.

### 5.5 Where the codec view predicts natural evolution

**Proposed, in the order the frame makes them cheap:**

- **Delta encoding against the previous broadcast.** Available today in principle, because the broadcast is identical for every recipient and arrives on a fixed cadence. What it needs first is level 1 named, because a diff is a per-field statement and there is no enumeration of fields that both ends agree on.
- **Interest management.** The arena is 48 units across (`crates/arena-core/src/shooter.rs:6`) with at most eight players (`crates/arena-core/src/shooter.rs:71`), so there is nothing to gain today. The frame's point is structural: interest management is per-recipient by definition and therefore breaks the single-serialisation property of §5.2. It should be costed as a change to the broadcast architecture, not as a filter.
- **Extending the static/dynamic split.** §5.1's seed trick applied to more entity classes is the cheapest available compression and needs no new machinery — only a written statement of which properties are derivable.
- **Rate/precision separation.** Position is `f32` pairs at 30 Hz for everything (`crates/arena-core/src/proto.rs:36-62`). A codec that distinguishes properties by change rate — score and weapon change rarely, position changes every tick — is the standard next step, and it is again an enumeration problem before it is an encoding problem.

## 6. The reconciliation loop on this tree

v7 implements a real instance of send, track, override, adjust. Walking it precisely is worthwhile because the deviations from the clean model are where §7 and §8 get their evidence.

**Send.** Every 0.05 s of accumulated client game time (`crates/arena/src/online.rs:695-696`), the client takes the next sequence number (`crates/arena/src/online.rs:697-698`), records the movement intent and stance speed as a `Cmd` stamped with the current client time — but only while alive (`crates/arena/src/online.rs:699-709`) — and transmits the held intent with that sequence and a view tick (`crates/arena/src/online.rs:715-726`).

**Track.** The command deque is the sent-versus-local difference. It is bounded at 64 entries (`crates/arena/src/online.rs:706-708`), which at the 0.05 s cadence is 3.2 s of history — comfortably longer than any latency the rewind bound contemplates.

**Override.** On each state message, the client takes its own authoritative record, prunes every command the server has acknowledged (`crates/arena/src/online.rs:566-568`), and replaces its prediction with the authoritative position (`crates/arena/src/online.rs:562`, `crates/arena/src/online.rs:569`).

**Adjust.** It then replays each surviving command from the authoritative position, giving each a duration equal to the interval to the next command's send time — or to the present for the newest — clamped to 0.3 s (`crates/arena/src/online.rs:570-575`). If the result is implausible, or the player has just respawned, everything snaps instead (`crates/arena/src/online.rs:577-587`).

Between state messages, the live prediction advances by the render step (`crates/arena/src/online.rs:729-740`), and a separate smoothed value follows the prediction toward the camera (`crates/arena/src/online.rs:741-743`).

### 6.1 Where it deviates from the clean model

**Deviation 1 — level 3 is integrated on level 4's clock. Verified.** The live prediction calls `move_circle` with the render frame's `dt` (`crates/arena/src/online.rs:731-740`), which is the clamped redraw gap (`crates/ember-engine/src/app.rs:258-263`), while the authority calls the same function with `FIXED_DT` (`crates/arena-core/src/shooter.rs:352`, `crates/arena-core/src/shooter.rs:379`). The function is deliberately shared between the two — its comment says so (`crates/arena-core/src/shooter.rs:186-188`) — and it is not step-size independent: it tests only the destination point and resolves the two axes separately in a fixed order (`crates/arena-core/src/shooter.rs:205-216`). The weak-device assessment reaches this from the performance side and derives the consequence, that a slower client diverges further and is corrected more often (`docs/weak-device-performance.md:139-157`); the scene-timing document carries it forward as a gate on scene-Hz actuation (`docs/dynamic-scene-timing-and-scaling.md:128-130`).

Under this frame it has a one-line name: **the local model's identity is being supplied by the renderer's clock.** Level 3 has no clock of its own in the arena client (§2.1), so it borrows the only one in scope, which belongs to level 4. Everything downstream follows — that the divergence scales with frame rate, that it is a gameplay defect produced by a rendering property, and that no amount of work at levels 4 and 5 can fix it.

**Deviation 2 — the replay path diverges too, and for the same reason. Derived, and not previously recorded.** It would be easy to read the weak-device finding as "the live prediction is sloppy but the replay is exact". It is not. The server applies one held input across roughly three ticks: the send cadence is 0.05 s (`crates/arena/src/online.rs:695`) and the tick is 1/60 s (`crates/arena-core/src/shooter.rs:5`), so `0.05 / (1/60) = 3`. The client replays that same input as *one* `move_circle` call of duration ≈ 0.05 s (`crates/arena/src/online.rs:573-574`). In open space the two agree exactly, because the motion is linear. Against geometry they do not, because three sequential axis-separated slides against `blocked` are not the same as one — which is the identical step-subdivision property that makes deviation 1 a problem. So the reconciliation is exact only where the player is not touching anything, and the correction machinery absorbs the rest. The framework predicts this without any new reading: replay is a re-derivation across a clock boundary, and the two sides of that boundary tick at different rates.

**Deviation 3 — the override crosses a clock boundary with no conversion. Verified, then derived.** The authoritative position is indexed by a server tick (`crates/arena-core/src/proto.rs:152`). The replayed durations are differences of client send times (`crates/arena/src/online.rs:572-574`). Nothing in the reconciliation relates those two quantities; the client never converts `tick` into its own time base for this purpose. The replay is therefore a heuristic — "re-apply what I sent, for as long as I held it, on top of where you said I was" — that is correct when the round trip is close to one send interval and biased by at most one send interval otherwise, in a direction that the 4-unit snap threshold will never catch and the 25-per-second smoothing will quietly absorb (`crates/arena/src/online.rs:577`, `crates/arena/src/online.rs:742-743`).

What makes this a finding rather than a complaint is that **the conversion machinery exists in the same function and is used for something else.** The client does convert its render-side interpolation phase into a server tick, in order to claim a view for lag compensation: `view_tick` is the last received tick minus the fraction of the state interval already consumed (`crates/arena/src/online.rs:712-714`), and the server clamps that claim against what the connection's measured RTT can justify (`crates/arena-server/src/lib.rs:827-828`, `crates/arena-server/src/lib.rs:842`). So a tick-to-local-phase relation is computed every send, and the position rebase does not use it. Under this frame the asymmetry is legible: the hit-test path was designed as a cross-level claim and got a conversion; the position path was designed as a local correction and did not.

**Deviation 4 — the diff is tracked but never consulted before sending.** Noted in §5.3; it belongs on this list because it is the one place v7 is *more* conservative than the clean model, not less.

## 7. What the framework explains in the corpus

Each existing document guards one or two level boundaries. Stated surgically:

**The presenter architecture separates 4 from 5, structurally.** Its whole content is a module boundary that makes the scene renderer unable to reach the surface (invariant I1, `docs/presenter-architecture.md:75`) and a value type carrying what level 4 must hand to level 5 (`docs/presenter-architecture.md:26-69`). Two of its harder arguments are level-identity arguments in disguise. That `seq` must come from a ring-global counter rather than a per-slot increment (`docs/presenter-architecture.md:63`) is the observation that a level needs *one* identity, not one per storage location. That `seq` orders frames but does not witness completion, so the slot needs an explicit state as well (`docs/presenter-architecture.md:65`), is the observation that level 4's identity and level 4's readiness are different predicates. Its `sim_time` field exists precisely to keep a level-3 quantity from being confused with the level-4 wall clock that v7 currently reports instead (`docs/presenter-architecture.md:62`).

**The input latch gives levels 3 and 5 independent reads of one input stream.** Its central proof is that a destructive drain cannot serve two consumers and that no ordering fixes it (`docs/input-latch.md:36-48`), and its central design decision is that the view's mark travels on the scene frame rather than living in the latch (`docs/input-latch.md:70-75`). Under this frame the reason is immediate: the warp's baseline is a property of *a particular level-4 artefact*, not of the presenter, so it must be stored with the level-4 artefact — which is also why a per-present mark fails exactly when one frame is presented many times. Its determinism section is the level rule stated directly: a second consumer of input is not a second author of state (`docs/input-latch.md:106-112`).

**The latency taxonomy is the transitions between levels, stamped with clocks.** Its stage IDs map one-to-one: `I0`/`I1` are input arrival and the level-3 read, `I2` is level 3 producing level 4, `R0`/`R1` and `G0` are level 4 being produced, `I3` is the late read that level 5 uses, `P0`–`P3` and `G1` are level 4 becoming level 5 (`docs/latency-observability.md:150-165`). Its network fork `C0`–`C4` and `S0`–`S5` is the 3 → 2a → 2b → 3 round trip (`docs/latency-observability.md:167-177`). Its firmest rule — never subtract a client stamp from a server stamp (`docs/latency-observability.md:129-130`, restated at `:175`) — is the general form of §6.1's deviation 3: level 2a and level 2b live on different clocks and quantities from the two may be joined by causal identity but never by arithmetic. And its observation that `sim_time` and `seq` are necessary provenance but not latency provenance (`docs/latency-observability.md:21-25`) is the statement that a level's identity does not imply a level's timing.

**The weak-device assessment is where a boundary has already failed.** Its §9 finding is deviation 1 above (`docs/weak-device-performance.md:139-157`). Its other findings are the opposite case and are worth noting as such: backface culling, depth store, resolution cap and payload size are all *inside* level 4 (`docs/weak-device-performance.md:175-193`), touch no other level, and are therefore free of the coupling that makes the prediction finding hard. The document's own ranking has the level-4-internal items as one-liners and the level-boundary item as a measurement question, which is what this frame would predict.

**The scene-timing controller actuates 4 and 5 while leaving 1–3 untouched.** It says so explicitly and repeatedly: it never changes the authoritative simulation rate, fixed step, tick ordering or protocol meaning (`docs/dynamic-scene-timing-and-scaling.md:124`), never changes network cadence in response to graphics load (`docs/dynamic-scene-timing-and-scaling.md:126`), and never skips a present or moves the late input read (`docs/dynamic-scene-timing-and-scaling.md:122`). Its actuators are scene resolution, scene rate and guard band — all level-4 fidelity (`docs/dynamic-scene-timing-and-scaling.md:64-68`). And its dependency rule is deviation 1 stated as a gate: scene-Hz scaling must not alter the `dt` supplied to the game's update, so until prediction is render-independent only resolution may adapt (`docs/dynamic-scene-timing-and-scaling.md:128-130`). That is this document's thesis reached from the control side — a controller that may only touch levels 4 and 5 is unsafe precisely where a level-3 quantity is derived from a level-4 clock.

**Derived — the corpus is a boundary-defence corpus.** Four documents defend a boundary structurally; the fifth reports one that leaks. No document in the corpus is about a level's *interior* except the weak-device levers, and those are the only findings that were one-line fixes. That correlation is the practical argument for the frame: boundary problems are expensive and interior problems are cheap, so knowing which kind you have is worth the vocabulary.

## 8. The conflations in v7

Most consequential first. Each was checked against the tree rather than assumed.

### 8.1 Level 3's identity is supplied by level 4's clock

Deviation 1 of §6.1. **Consequence:** a rendering property determines a gameplay outcome. The player's correction rate is a function of their frame rate, the divergence is systematic rather than random, and it is unfixable at levels 4 and 5 where all the current tuning lives. It also blocks the scene-Hz ladder in the adaptive controller (`docs/dynamic-scene-timing-and-scaling.md:128-130`), which is a second, larger cost: the single most valuable weak-device lever after resolution is gated behind this conflation.

**The repository contains its own counterexample, which is why this is a conflation and not a necessity.** Local pong runs a proper fixed-step accumulator, snapshots the previous state before each step, and interpolates between the two for display with an explicit `alpha` (`crates/arena/src/lib.rs:161-170`, `crates/arena/src/lib.rs:188-190`, `PrevState` at `crates/arena/src/lib.rs:128`). That is levels 3 and 4 correctly separated, in the same crate, by the same author, in 30 lines. The arena client has neither the accumulator nor the previous-state snapshot.

### 8.2 `ShooterGame` holds all five levels in one struct

**Verified.** The struct's 35 fields (`crates/arena/src/online.rs:240-280`) sort by level once the question is asked, which is what makes the conflation legible rather than merely untidy:

- **level 2a (sent):** `history`, `next_seq`, `since_input`, `pitch` (elevation is now transmitted, not merely rendered)
- **level 2b (received):** `latest`, `bullets`, `last_tick`, `pads_active`, `metas`
- **level 3 (local):** `pred_pos`, `was_alive`, `obstacles`, `pads_pos`, `arena_half`, `my_id`
- **level 4 (rendered/display):** `from`, `to`, `t`, `own_render`, `bullets_age`, `zoom`, `eye_h`, `bob_t`, `aim`, `yaw`, `reload_started`, `shot_started`
- **level 5 / UI:** `score_shown`, `since_score_ui`, `since_status`

The remainder — `chan`, `audio`, `assets`, `lost`, `since_ping`, `time` — are transport, resource and clock handles that belong to no level, which is the correct answer for them.

**Consequence 1 — level 3 is a single `Vec2`.** Once sorted, the arena client's entire local model is `pred_pos` plus the static geometry. There is no client-side instance of `Sim`, no client-side tick, and no client-side representation of any entity but the player's own position: `move_circle` is the only sim function the client calls (`crates/arena/src/online.rs:732-738`). Everything else reaches the renderer from level 2b directly (§2.2's 2b → 4 row). This is a legitimate design — §9 defends it — but it means the phrase "the client's simulation" describes one vector, and any proposal that assumes otherwise is mis-sized.

**Consequence 2 — the display integrators are indistinguishable from the sim integrator.** `zoom`, `eye_h` and `bob_t` are advanced with the same `dt` as `pred_pos` (`crates/arena/src/online.rs:642`, `:659`, `:670-679`, against `:731-740`), in the same function, with no marker of which ones matter for correctness. Three of them may be integrated on any clock; one may not. Nothing in the code says so, which is precisely how deviation 1 survives review.

**Consequence 3 — the smoothed camera value is correctly quarantined, and only by luck of reading order.** `own_render` chases `pred_pos` through a damped follow (`crates/arena/src/online.rs:741-743`) and feeds only the camera (`crates/arena/src/online.rs:772-776`); the transmitted intent is built from raw input, not from `own_render` (`crates/arena/src/online.rs:661-665`, `:700-705`, `:715-726`). That is the right separation — a level-4 smoothing must not author level 2a — and it is exactly the rule the input latch writes down for the *warp* read (`docs/input-latch.md:112`). Here the same rule is being obeyed by a different subsystem with no statement anywhere that it is a rule. A future edit that aimed from the smoothed position would be a one-word change and would look like a bug fix.

### 8.3 `Frame` is the 3 → 4 sampling boundary and carries almost nothing

**Verified.** `Frame` is a camera and a list of instances (`crates/ember-engine/src/renderer.rs:118-123`); `Camera` is an eye, a look-at target and a vertical FOV (`crates/ember-engine/src/renderer.rs:93-98`); `Instance` is a position, scale, colour, yaw and mesh id (`crates/ember-engine/src/renderer.rs:58-69`). The scene pass's only use of the camera is one view-projection upload (`crates/ember-engine/src/renderer.rs:638-640`), with near and far planes inlined at the single call site (`crates/ember-engine/src/renderer.rs:113`).

**What it fails to carry, and what each omission costs:**

- **No identity.** No sequence, no sim time. A rendered frame cannot be named, so it cannot be ordered, acknowledged, re-presented deliberately, or joined to a measurement. Every downstream design in the corpus begins by adding one.
- **No world age.** The engine reports scene age from a wall-clock instant in the renderer instead (`crates/ember-engine/src/renderer.rs:545-548`), which answers a different question — the presenter document makes this exact distinction (`docs/presenter-architecture.md:62`).
- **No input mark.** So the warp has no baseline to difference against, which is one of the two structural reasons stage A cannot deliver a latency win today (`docs/weak-device-performance.md:37-43`).
- **No rotation.** The camera is a look-at target, so a rotation delta cannot be expressed without a conversion — the presenter document's argument for a quaternion pose (`docs/presenter-architecture.md:52`).
- **No projection detail.** One FOV scalar, with no separation of display FOV from guard band, which stage B requires (`docs/presenter-architecture.md:61`).

**Derived — the omissions are not five oversights but one.** `Frame` was designed as an *argument*, not as a *state*: a description of what to draw, consumed immediately and discarded. That is the correct shape for a level-3 → level-4 sampling in a renderer that draws every frame it is given. It becomes wrong exactly when level 4 acquires a lifetime independent of level 3 — a ring, a throttle, a re-present — which is what ATW makes true. Every field the corpus wants to add is a consequence of level 4 outliving the sample that produced it.

### 8.4 Level 1 is absent and its three surrogates drift unchecked

§4, with the `deaths` drift as the verified instance. **Consequence:** every cross-level correspondence in the codebase is maintained by convention, no tool can check one, and the two cheapest codec improvements (§5.5) are blocked behind an enumeration that does not exist.

### 8.5 Received state doubles as display state

**Verified.** `latest` is the decoded authoritative record and is also read directly by the renderer for health pips, colour, stance heights, weapon accent and the viewmodel (`crates/arena/src/online.rs:856-900`, `crates/arena/src/online.rs:914-961`), while a *separate* pair of maps carries the interpolation endpoints for the same players (`crates/arena/src/online.rs:246-248`, updated at `crates/arena/src/online.rs:529-557`). So a remote player's position is smoothed across a state interval but their stance, health and weapon change discontinuously at the state boundary — the crouch-height switch at `crates/arena/src/online.rs:867-871` steps instantly while `render_pos` slides (`crates/arena/src/online.rs:325-330`).

This is a modest visual matter and it is listed because of what it reveals: the decision about which properties interpolate was made per-field, implicitly, by which map a field happened to live in. A named level 4 would make it a stated policy. **Derived:** the same structure is why the backlog's interpolation item (`docs/plans/backlog.md:35`) is hard to act on — the interpolation state is spread across three fields of a struct that also holds the authority it interpolates.

## 9. Where the model is procrustean

Three places, stated because a frame that fits everything explains nothing.

**The client's remote players skip level 3 entirely, and should.** §2.2's 2b → 4 transition has no place in a clean five-level picture, which would route received state into the local model and sample the local model for rendering. v7 interpolates received state straight to the renderer (`crates/arena/src/online.rs:325-330`, `crates/arena/src/online.rs:856-900`). Instantiating eight remote players into a local sim purely to sample them again would be strictly more code, more memory and more divergence surface, for a display result that snapshot interpolation already produces correctly. The honest statement is that **level 3 is per-entity, not global**: an entity is in the local model only if the client needs to predict it, and in this game exactly one entity qualifies. The five levels are levels of *data*, not of *entities*, and any given entity may be absent from several of them.

**Level 5 barely exists yet, so its boundary with level 4 is currently a definition rather than an observation.** Stage A is an identity blit (`crates/ember-engine/src/present.wgsl:30-31`), the present pass has no pose input at all (`docs/weak-device-performance.md:37-43`), and the throttle that would make one frame outlive its sample is native-only (`docs/weak-device-performance.md:51-59`). So the 4/5 distinction is real in the architecture and, on the shipped web build today, vacuous in the data: every presented frame is the frame just rendered. The distinction is being *paid for* now and *collected* later, which the adopted policy argues for on retrofit-cost grounds (`docs/atw-first-rendering.md:10-13`) and the weak-device document prices honestly (`docs/weak-device-performance.md:37-43`). A reader who tests this frame against the running web build will find levels 4 and 5 indistinguishable, and that is the truth about the build rather than a flaw in the frame.

**`view_tick` is a legitimate level-4 → level-2a flow, which a naive layering rule would forbid.** The client derives its claimed view from the interpolation phase — a level-4 quantity — and sends it (`crates/arena/src/online.rs:712-714`). A rule of the form "presentation-side data must never reach the wire" would reject this, and the rule would be wrong: lag compensation is *about* what the player saw, so the shooter's rendered view is exactly the correct input, and the server defends itself by clamping the claim rather than by distrusting its provenance (`crates/arena-server/src/lib.rs:827-828`, `:842`). The correct rule is narrower and is the one the input latch actually writes: the *cosmetic late* read must not author state (`docs/input-latch.md:112`). A deliberate, clamped, adversarially-reviewed claim about what was displayed is a different thing from a warp delta leaking into a send.

## 10. What the model makes buildable

Each payoff with the seam it needs.

**A diff-based codec needs level 1 named.** §5.5. The seam is an enumeration of the scene's properties that both the sim structs and the protocol types are checkably projections of — whether that is a real IDL, a macro, or a hand-written table with a test that fails when the three surrogates disagree. The cheapest useful version is the last: a test that asserts the field correspondence the encoder implements (`crates/arena-server/src/lib.rs:396-432`) would have caught the `deaths` ambiguity of §4 and costs nothing to run.

**Replay and rollback need level 3's identity clean of level 4's clock.** The seam is a fixed-step accumulator in `ShooterGame::update`, structurally the one local pong already has (`crates/arena/src/lib.rs:161-170`). What it buys beyond fixing deviation 1: a local tick number, which is the identity level 3 currently lacks, which is in turn what makes a replay addressable and a rollback bounded. The scene-timing document's dependency rule (`docs/dynamic-scene-timing-and-scaling.md:128-130`) means this same seam also unlocks the scene-Hz actuator, so it is one change serving a gameplay correctness goal and a weak-device performance goal at once.

**A headless server needs levels 4 and 5 absent without touching 1–3.** Mostly already true, and the frame says why: `arena-server` links `arena-core` and never `ember-engine`, so the authoritative sim (`crates/arena-core/src/shooter.rs:272-283`) and the codec (`crates/arena-core/src/proto.rs`) are already free of any rendering type. The remaining seam is on the client side of the same idea — a headless *client* for determinism testing needs `ShooterGame` to be constructible without a renderer, which today it nearly is, since `update` takes only input and `dt` and returns a `Frame` (`crates/ember-engine/src/lib.rs:31-32`). The obstacle is not the trait; it is that the level-3 result is buried among the level-4 fields of §8.2 and there is no accessor that exposes it.

**Determinism testing is a level-3 property, statable only when the levels are separated.** The property one wants is: identical inputs at identical ticks produce identical level-3 state. In v7 that sentence cannot even be written for the arena client, because there are no client ticks and no client-side level-3 state to compare beyond one vector. The seam is the same accumulator as above, plus one method returning the local model. `arena-core` already demonstrates the payoff on the server side: because the sim is a separable, renderer-free structure, it is exercised directly by tests that need no engine at all, and the one determinism property currently asserted outright — that a seed produces the same arena twice and a different seed does not — is asserted there (`crates/arena-core/src/shooter.rs:549-560`). No equivalent assertion is possible for the client today, which is the point.

**A latency HUD that means something needs identity at levels 4 and 5.** Already specified in full (`docs/latency-observability.md:138-148`); the frame's contribution is only to say that `SceneFrame.seq` and `PresentTrace.present_seq` are one design decision applied to two levels, and that the reason a present trace must be ephemeral rather than a field on the frame (`docs/latency-observability.md:143-144`) is that levels 4 and 5 stand in a one-to-many relation.

## 11. Coordination items

The sharp decisions this frame surfaces, each of which changes an interface rather than an implementation:

- **Does level 1 get an artefact?** A schema, a macro, or a correspondence test — or a written statement that the three surrogates are maintained by hand and that drift is accepted. Either answer is defensible; the current state is that the question has not been asked. This gates both cheap codec improvements.
- **Does the arena client get a fixed-step accumulator, or merely a render-independent integration step?** The scene-timing document raises exactly this and leaves it open (`docs/dynamic-scene-timing-and-scaling.md:176`). The frame's contribution: only the accumulator gives level 3 an identity, and identity is what replay, rollback and determinism testing need. A render-independent step fixes the divergence without buying any of those.
- **Should the position rebase use the tick-to-phase conversion that the same function already computes for `view_tick`?** §6.1's deviation 3. This is a small change with a real risk of making things worse, and it should be decided against measurement rather than argument — the weak-device document's M8 already proposes the measurement (`docs/weak-device-performance.md:213`).
- **Does the compatibility story split `PROTO_VERSION` into syntax and scene-concept versions?** §5.4, against the backlog's frozen-version problem (`docs/plans/backlog.md:31`).
- **Which remote-player properties interpolate?** §8.5. Currently answered per-field by accident of storage.
- **Does the single-serialisation property of the broadcast become a stated invariant?** §5.2. It is load-bearing for cost and is nowhere written down, so the first per-recipient feature will silently discard it.
