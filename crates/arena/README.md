# Arena client

This crate is the player-facing arena application: it runs the local two-player pong mode and the networked Killshot shooter on the shared ember renderer, while `arena-core` supplies simulation and protocol types.

`src/lib.rs` exposes `OnlineConfig`, `run_local`, and `run_online`; `src/main.rs` turns those entry points into the native `arena-app` binary, and the wasm exports let the web launcher select a mode and query the protocol version.

The native and web clients consume the models and rig metadata in `assets/`, and `run_online` registers their meshes in the fixed order required by the setters in `src/lib.rs`.

Repository-wide layering and renderer limits live in [`../../CLAUDE.md`](../../CLAUDE.md); authoritative versus predicted state is documented in [`../../docs/state-model.md`](../../docs/state-model.md), outdoor presentation in [`../../docs/environment-weather.md`](../../docs/environment-weather.md), and model import rules in [`../../docs/asset-pipeline.md`](../../docs/asset-pipeline.md).
