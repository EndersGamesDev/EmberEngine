# Arena core source

This directory holds the pure gameplay model that can run without a renderer or platform: `sim.rs` implements classic pong, and `shooter.rs` owns the authoritative Killshot world, combat, resources, rounds, and seeded Trench City generation.

`proto.rs` publishes the `C2S` and `S2C` message families and protocol constants; `parkour.rs` exposes deterministic movement shared by server authority and client prediction; `freight_yard.rs` and `harbor.rs` expose authored level geometry and placement tables.

The dense in-module test suites pin tick determinism, codec compatibility, collision and vertical-span behavior, map traversal and sightlines, weapons, shields, scoring modes, lag compensation, and client/server agreement. `killshot_tests.rs` and `shotgun_tests.rs` isolate release-level combat contracts that cut across the main modules.

Any change here must be evaluated for both peers under the simulation and protocol rules in [`../../../CLAUDE.md`](../../../CLAUDE.md); [`../../../docs/state-model.md`](../../../docs/state-model.md) documents which types and clocks belong to each state boundary.
