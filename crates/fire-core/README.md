# Fire Racer core

`fire-core` is the renderer-free racing model shared by the Fire client’s prediction and `fire-server` authority: it owns car physics, the castle circuit, lap and race state, AI drivers, tactical items, and protocol 2.

Its 60 Hz step, operation order, track sampling, and wire behavior are compatibility facts because both peers execute or encode them independently.

The crate remains separate from `arena-core` so a racing message or physics change cannot force an unrelated arena protocol bump or strand a frozen arena client at the join gate.

Cross-peer simulation rules are summarized in [`../../CLAUDE.md`](../../CLAUDE.md), and the distinction between game-local codecs and the evergreen outer protocol is defined in [`../../docs/one-server-evergreen.md`](../../docs/one-server-evergreen.md).
