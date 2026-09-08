# Client networking contract test

`fake_game.rs` defines a minimal game that implements the codec, prediction, and remote-entity hooks together, proving that the crate's generic pieces compose through the public API rather than only inside their defining modules.

The test pins outbound and inbound frame encoding, acknowledgement and server timestamps, authoritative replay, local correction, remote interpolation, dead reckoning, and remote snap-or-smooth decisions in one deliberately small model.

Consumers should extend the hook traits without weakening this compile-time integration surface; game-specific behavior belongs in the consumer rather than in the fake implementation.

The conceptual boundaries exercised here are documented in [`../../../docs/state-model.md`](../../../docs/state-model.md), under the repository layering rules in [`../../../CLAUDE.md`](../../../CLAUDE.md).
