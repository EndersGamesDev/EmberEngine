# ember backlog

Follow-ups and known gaps, one line each; pull into a milestone plan before working. Frictions with the orchestration itself go to the orchestration repo's backlog instead.

## Infrastructure

- Server lane: no `ember` loop account exists on any server; provisioning on adler (designated CI/build box) is blocked pending owner action — until then no builds/tests/gates can run (compute never runs on the workstation).
- The live specht services (ember-server, pong-server + Cloudflare tunnel) run under the `ender` human account via nohup and do not survive a reboot; migrating them to the ember loop account with systemd user units is future work.
- The Cloudflare quick-tunnel domain changes on every restart; a stable domain or a health-checked republish loop would remove the manual redeploy coupling.

## From the adoption survey (at b1ef9af)

- ~~ember-server pre-Hello `Ping` slot-parking hole~~ — DONE 2026-08-28, commit 3332f55 (guard originally from d3c6f48's pong-server, not 224bbd2; e2e test with negative control). Residual hardening divergences from pong-server, found in the same lane:
- ember-server has no per-IP connection cap (pong-server: MAX_CONNS_PER_IP = 6) — one host can still occupy the global admission cap for 10 s windows.
- ember-server has no per-connection message-rate cap (pong-server: 30 msgs/tick) — a post-Hello peer can dominate the shared event channel.
- ember-server has no socket read timeout or handshake watchdog — a byte-dribbling client holds two threads until the 10 s sweep reaps it.
- The README "## Pong" section still describes online play as paddle pong over `sim.rs`; online is now the arena shooter over `shooter.rs`. Rewrite outside milestone 1.
- `Instance::yaw` rotates normals with the same matrix as positions; under non-uniform scale that breaks lighting — fine today, trap later.
- Frozen hub game versions become unjoinable on every protocol bump; the hub needs a compatibility story (`old_proto_may_list_but_not_join` covers the server side only).

## From README known limitations

- Snapshot interpolation degenerates to snap-to-latest at ≤60 fps; a proper ~100 ms interpolation delay buffer is future work.
- The arena client connects before the window opens; an unreachable server pauses launch ~4 s before the offline fallback.
