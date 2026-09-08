# Arena client source

This source tree assembles the arena presentation around `arena-core`: `lib.rs` owns local pong and application startup, while `online.rs` connects the authoritative shooter stream to prediction, interpolation, input, and frame construction.

The public Rust surface is `OnlineConfig`, `run_local`, and `run_online`; wasm builds additionally export initialization, protocol-version, local-start, and online-start functions from `lib.rs`, and `main.rs` is the native command-line entry point.

Presentation is split across aiming and feel, contact feedback, HUD and supplies, reloads and rounds, sound, view arms and grips, scripted capture, settings, weather, and the hand-authored harbor and prop builders. Unit-only coverage lives beside those modules, including the dedicated `grip_tests.rs` module.

Changes here must preserve the one-way game-to-renderer layering and fixed mesh-ID registration described in [`../../../CLAUDE.md`](../../../CLAUDE.md); network-state boundaries are analysed in [`../../../docs/state-model.md`](../../../docs/state-model.md), and weather must remain presentation-only under [`../../../docs/environment-weather.md`](../../../docs/environment-weather.md).
