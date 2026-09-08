# Four Kings core

`kings-core` is the deterministic, renderer-free rules and wire crate shared by the Four Kings client and authoritative server.

It owns the four-corner board representation, per-seat coordinate frames, setup and formations, legal actions and outcomes, elapsed-millisecond turn clock, and game-specific protocol 1 without floats, randomness, or an internal wall clock.

The online client consumes these rules for highlights and applies only server-echoed state, while hotseat play and `kings-server` both use the same transition functions directly.

[`../../docs/kings-design.md`](../../docs/kings-design.md) is the rules and architecture record; exact protocol gating remains subject to [`../../CLAUDE.md`](../../CLAUDE.md).
