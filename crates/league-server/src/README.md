# Ultimate League server source

`lib.rs` publishes `ServerConfig`, `build_stamp`, and `run`; it owns bounded connection threads and the single hub that admits lobbies, manages rosters and drafts, advances each authoritative `league-core::Match` at 60 Hz, and broadcasts snapshots at 20 Hz.

Unfilled or disconnected seats become deterministic bots, late joins take available bot seats, creator authority follows the lowest occupied human slot, and lobbies cycle from selection through live play and results back to selection.

`main.rs` preserves the positional bind address, optional host name, and plain-TCP deployment boundary. `tests.rs` combines direct hub scenarios with real WebSocket duel and squad clients.

Those tests pin protocol and duplicate-hello gates, atomic lobby switching, host handoff, team-unique drafts with opponent mirrors, early start, lifecycle cleanup, command ownership, and live movement, rank, purchase, and disconnect messages under [`../README.md`](../README.md).
