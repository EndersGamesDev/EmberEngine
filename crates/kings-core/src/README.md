# Four Kings core source

`lib.rs` re-exports the principal board, clock, protocol, and rule types and functions. `board.rs` defines tiles, directions, seat frames, pieces, setup, serialization state, and terminal outcomes.

`rules.rs` computes targets and applies moves, timeouts, and disconnects; `clock.rs` advances the externally fed turn budget; `proto.rs` defines every client and server frame, lobby metadata, limits, compatibility defaults, and sanitization.

The module suites pin coordinate and mirror algebra, setup tables and formation validation, board-state round trips and rejection, every piece's movement and elimination behavior, clock boundaries, exact JSON keys and variants, frame budgets, and version mismatch handling.

Section references in the source resolve to the rules of record in [`../../../docs/kings-design.md`](../../../docs/kings-design.md); changes must preserve that document's board and turn semantics across both client and server.
