# Client networking source

`lib.rs` re-exports this crate's public surface: `frame` defines transport-neutral frames and keepalives, `transport` owns platform WebSockets, `handshake` gates application traffic, and `connection` combines those pieces into client-visible progress and diagnostics.

`prediction` supplies serial sequence allocation, bounded input history, and authoritative rebase/replay, while `snapshot` buffers remote samples for game-defined interpolation or dead reckoning.

The hooks in `hooks.rs` are the game boundary: codecs decide payload representation, prediction hooks decide acknowledgement and correction semantics, and remote-entity hooks decide how snapshots become render state.

Module tests pin serial wraparound, eviction, partial acknowledgement, replay order, handshake gating, stale snapshots, and hook-controlled corrections; state ownership rules are developed in [`../../../docs/state-model.md`](../../../docs/state-model.md).
