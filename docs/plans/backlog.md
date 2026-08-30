# ember backlog

Follow-ups and known gaps, one line each; pull into a milestone plan before working. Frictions with the orchestration itself go to the orchestration repo's backlog instead.

Presenter and input work is planned in `docs/presenter-architecture.md` and `docs/input-latch.md`, not here and not in `docs/plans/milestone-1.md` — those two documents are the plan of record for the presenter split, the `SceneFrame` contract, the two-consumer input latch, and the capture policy, restated against the v7 renderer. Items below that they now own are marked.

## Architecture (from the v7 re-framing)

- The scene and present command buffers reach the queue in one `submit` call (`crates/ember-engine/src/renderer.rs:798-799`) under a comment claiming the ATW sliced-submission rule (`crates/ember-engine/src/renderer.rs:673-675`). Splitting the call is a one-line fix worth making for the stage boundary, but it does **not** deliver sliced scheduling — on one in-order queue the present still runs behind the scene work either way, and real slicing needs the scene emitted as several separately submitted chunks (roadmap step 7). Fix the comment along with the code (`docs/presenter-architecture.md` §5).
- Shader hot-reload rebuilds both the scene and the present pipeline from one clock (`crates/ember-engine/src/renderer.rs:807-835`) and needs re-homing before the presenter split can be written (`docs/presenter-architecture.md` §8.1).
- The scene-Hz throttle lives inside the renderer, so the renderer decides whether the renderer runs (`crates/ember-engine/src/renderer.rs:608-612`); the clock belongs above both GPU stages (`docs/presenter-architecture.md` §8.2).
- `cursor_ndc()` and `aspect()` have no callers anywhere in the workspace, and the shooter's head comment claiming cursor unprojection (`crates/pong/src/online.rs:2`) is stale — it aims with relative deltas. The API is worth keeping; nobody should assume a shipped game is exercising it (`docs/input-latch.md` §7).
- Pointer capture fires on any mouse press with no button filter (`crates/ember-engine/src/app.rs:195-203`), so in the one game that opts in the fire button is also the capture gesture; and `cursor_ndc` goes stale rather than `None` under pointer lock (`docs/input-latch.md` §7).
- `docs/state-model.md` names the five levels of state the engine holds (abstract scene, sent/received network, local, rendered, presented) and is the conceptual root above the other architecture documents; its open items are whether level 1 gets a checkable artefact at all (no schema exists — the scene concept is hand-written three times, in the sim structs, the protocol enums and the instance builder) and whether the arena client's prediction gets a fixed-step accumulator rather than integrating on the render clock (`crates/pong/src/online.rs:731-740`).
- Weak-device performance is assessed in `docs/weak-device-performance.md`, which owns the prioritized levers and the measurement items: backface culling is off (`crates/ember-engine/src/renderer.rs:963`), `scene_scale` is pinned at 1.0 against an uncapped device-pixel-ratio backing store (`crates/ember-engine/src/app.rs:233-234`), scene depth is stored every frame and read by nothing (`crates/ember-engine/src/renderer.rs:701-704`), the ATW rig is compiled out on the only target that is actually weak (`crates/ember-engine/src/renderer.rs:611-612`), and the workspace defines no `[profile.release]` for the wasm payload.

## Infrastructure

- Server lane: no `ember` loop account exists on any server; provisioning on adler (designated CI/build box) is blocked pending owner action — until then no builds/tests/gates can run (compute never runs on the workstation).
- The live specht services (ember-server, pong-server + Cloudflare tunnel) run under the `ender` human account via nohup and do not survive a reboot; migrating them to the ember loop account with systemd user units is future work.
- The Cloudflare quick-tunnel domain changes on every restart; a stable domain or a health-checked republish loop would remove the manual redeploy coupling.
- A protocol bump has a two-sided deploy window with no zero-downtime ordering: page-first leaves the live page unable to join until the server moves, server-first the reverse. `deploy-pages.sh` now warns at ship time (27c20d6) and the refusal is an accurate sentence rather than a mystery, so this is socially mitigated, not solved. The real fix is the server publishing its own protocol version into `server.json` so the hub can warn BEFORE a join is attempted — deliberately not built, because it needs a server change and the server redeploy is itself blocked, and shipping a field nothing populates is worse than not shipping it. Revisit when the redeploy unblocks.

## From the adoption survey (at b1ef9af)

- ~~ember-server pre-Hello `Ping` slot-parking hole~~ — DONE 2026-08-28, commit 3332f55 (guard originally from d3c6f48's pong-server, not 224bbd2; e2e test with negative control). Residual hardening divergences from pong-server, found in the same lane:
- ember-server has no per-IP connection cap (pong-server: MAX_CONNS_PER_IP = 6) — one host can still occupy the global admission cap for 10 s windows.
- ember-server has no per-connection message-rate cap (pong-server: 30 msgs/tick) — a post-Hello peer can dominate the shared event channel.
- ember-server has no socket read timeout or handshake watchdog — a byte-dribbling client holds two threads until the 10 s sweep reaps it.
- The README "## Pong" section still describes online play as paddle pong over `sim.rs`; online is now the arena shooter over `shooter.rs`. Rewrite outside milestone 1.
- `Instance` carries a full `rot: Quat` (not a bare yaw — `with_yaw` is just the common-case helper), and normals are rotated by the same matrix as positions; under non-uniform scale that breaks lighting. Now exercised more: bullet tracers are non-uniformly scaled rods.
- Frozen hub game versions become unjoinable on every protocol bump; the hub needs a compatibility story (`old_proto_may_list_but_not_join` covers the server side only).

## From README known limitations

- Snapshot interpolation degenerates to snap-to-latest at ≤60 fps; a proper ~100 ms interpolation delay buffer is future work.
- The arena client connects before the window opens; an unreachable server pauses launch ~4 s before the offline fallback.
- Do not ship a REBUILT bundle from the new home until pong-server is redeployed: main is PROTO 8, the running server is 7. Page-only edits (index.html, games.json) are safe. The redeploy gates jumping, aim elevation and authored levels.
