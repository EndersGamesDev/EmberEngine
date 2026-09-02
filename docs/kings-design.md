# Four Kings: design of record

Four Kings (game id `kings`) is a four-corner chess variant on the ember engine: four players, a 10x10 board, sixteen pieces each in a 4x4 corner block, one action per 15-second turn, the last king on the board wins. It is chess plus two new legend pieces, the Joker (a teleporting sniper with a single capture tile) and the Hero (a dormant piece that trades a pawn for a rook-plus-knight body), with the pawn rules widened so that a corner formation is not a wall. This document is the rules of record and the build plan: every rule the spec left open is decided here, with the reason, so that a server validator and a client written independently from it agree on every legal move.

The spec and the diagram are the source. Where a sentence of the spec is made precise, the rule says so. Where the spec is silent, the decision is listed in section 3 with a one-line reason. Rule constants named in code style (`TURN_MS`, `NO_PROGRESS_TURNS`) are the constants of `kings-core`, so the doc and the crate cannot drift apart silently.

## 1. Rules

### 1.1 Board and coordinates

The board is 10x10, 100 tiles, all playable. A tile is `(x, y)` with `x` to the right (east) and `y` up (north), both `0..=9`; `(0,0)` is the south-west tile and `(9,9)` the north-east tile. Tile index for storage and tests is `i = y * 10 + x`. Tile colour, for bishops, is the parity of `x + y`.

The four 4x4 home blocks are the corners: SW `x,y in 0..=3`, SE `x in 6..=9, y in 0..=3`, NE `x,y in 6..=9`, NW `x in 0..=3, y in 6..=9`. The 36 remaining tiles (`x in {4,5}` or `y in {4,5}`) form the neutral cross. Neither the blocks nor the cross carry any rule of their own after setup: every piece may enter every tile, and the names are descriptive only. The diagram shows exactly this geometry.

### 1.2 Seats and per-seat frames

There are four seats, engine indices 0..=3, one per corner, counter-clockwise from the south-west: seat 0 = SW, seat 1 = SE, seat 2 = NE, seat 3 = NW. The diagram labels the corners 1 (SE), 2 (NE), 3 (NW), 4 (SW); the engine index is the label modulo 4, so the same cycle is preserved and seat 0 is the seat whose local frame is the identity. Turn order is 0, 1, 2, 3, 0, ... (counter-clockwise), skipping seats that are not alive.

Each seat has a fixed forward vector `f` and a fixed left vector `l`. Forward points along the seat's home edge toward the next seat in turn order; left is forward rotated 90 degrees counter-clockwise, as for a person standing on the tile facing forward. The four frames are 90-degree rotations of one another, never reflections, so handedness is identical for every seat. Pieces never turn: a piece's frame is its owner's frame wherever it stands, including inside another seat's block and after a teleport.

| Seat | Corner tile `c` | forward `f` | left `l` | front-left `f+l` | front-right `f-l` | left-forward `-f+l` | local to global `(u,v) -> (x,y)` | global to local |
|---|---|---|---|---|---|---|---|---|
| 0 (SW) | (0,0) | (+1, 0) east | (0, +1) north | (+1, +1) | (+1, -1) | (-1, +1) | `(u, v)` | `u = x, v = y` |
| 1 (SE) | (9,0) | (0, +1) north | (-1, 0) west | (-1, +1) | (+1, +1) | (-1, -1) | `(9-v, u)` | `u = y, v = 9-x` |
| 2 (NE) | (9,9) | (-1, 0) west | (0, -1) south | (-1, -1) | (-1, +1) | (+1, -1) | `(9-u, 9-v)` | `u = 9-x, v = 9-y` |
| 3 (NW) | (0,9) | (0, -1) south | (+1, 0) east | (+1, -1) | (-1, -1) | (+1, +1) | `(v, 9-u)` | `u = 9-y, v = x` |

The local frame `(u, v)` has `u` = tiles forward of the corner and `v` = tiles left of the corner, each `0..=9` across the whole board. A seat's home block is `u, v <= 3`. Front-left is `(u+1, v+1)` in every seat's own frame, which is the diagonal pointing at the board centre from the home block, so it is on the board from every home tile and off the board only from tiles with `u = 9` or `v = 9`. The two far edges of a seat are `u = 9` and `v = 9`; they are the tiles whose forward or left neighbour is off the board, which is how the engine computes them (no table).

### 1.3 Setup

Each seat owns the 16 tiles of its home block in three rings, exactly as drawn: the corner 2x2 (`max(u,v) <= 1`, 4 tiles), tier 1 (`max(u,v) == 2`, an L of 5 tiles), tier 2 (`max(u,v) == 3`, an L of 7 tiles). 4 + 5 + 7 = 16 pieces per seat, 64 on a full board.

The default formation in local coordinates, with the piece id within the seat (piece id on the wire is `seat * 16 + index`, stable for the whole game; promotion and awakening change the kind, never the id):

| index | piece | class | local (u,v) |
|---|---|---|---|
| 0 | King | Legend | (0,0) |
| 1 | Queen | Legend | (1,0) |
| 2 | Hero | Legend | (0,1) |
| 3 | Joker | Legend | (1,1) |
| 4 | Rook | Epic | (2,0) |
| 5 | Bishop | Epic | (2,1) |
| 6 | Bishop | Epic | (2,2) |
| 7 | Knight | Epic | (1,2) |
| 8 | Rook | Epic | (0,2) |
| 9..15 | Pawn x7 | Common | (3,0) (3,1) (3,2) (3,3) (2,3) (1,3) (0,3) |

The same formation in global coordinates, recomputed from the transforms above and pinned by a table-driven test:

| index | piece | seat 0 SW | seat 1 SE | seat 2 NE | seat 3 NW |
|---|---|---|---|---|---|
| 0 | King | (0,0) | (9,0) | (9,9) | (0,9) |
| 1 | Queen | (1,0) | (9,1) | (8,9) | (0,8) |
| 2 | Hero | (0,1) | (8,0) | (9,8) | (1,9) |
| 3 | Joker | (1,1) | (8,1) | (8,8) | (1,8) |
| 4 | Rook | (2,0) | (9,2) | (7,9) | (0,7) |
| 5 | Bishop | (2,1) | (8,2) | (7,8) | (1,7) |
| 6 | Bishop | (2,2) | (7,2) | (7,7) | (2,7) |
| 7 | Knight | (1,2) | (7,1) | (8,7) | (2,8) |
| 8 | Rook | (0,2) | (7,0) | (9,7) | (2,9) |
| 9 | Pawn | (3,0) | (9,3) | (6,9) | (0,6) |
| 10 | Pawn | (3,1) | (8,3) | (6,8) | (1,6) |
| 11 | Pawn | (3,2) | (7,3) | (6,7) | (2,6) |
| 12 | Pawn | (3,3) | (6,3) | (6,6) | (3,6) |
| 13 | Pawn | (2,3) | (6,2) | (7,6) | (3,7) |
| 14 | Pawn | (1,3) | (6,1) | (8,6) | (3,8) |
| 15 | Pawn | (0,3) | (6,0) | (9,6) | (3,9) |

Verified by recomputation (script in the synthesis session, to be turned into the `setup_is_four_rotations` test): for every index, seat `s+1`'s tile is `rot(seat s tile)` with `rot(x,y) = (9-y, x)`, and seat 0 is `rot` of seat 3; the two bishops stand on tiles of opposite colour in every seat (seat 0: (2,1) odd, (2,2) even; seat 1: (8,2) even, (7,2) odd; seat 2: (7,8) odd, (7,7) even; seat 3: (1,7) even, (2,7) odd); the joker's front-left from its start tile is its own second bishop's tile in every seat ((2,2), (7,2), (7,7), (2,7)), on the board; the joker's front-left is also on the board from all three mirror tiles of its start tile in every seat (seat 0: (8,1) to (9,2), (1,8) to (2,9), (8,8) to (9,9); seat 1: (1,1) to (0,2), (8,8) to (7,9), (1,8) to (0,9); seat 2: (1,8) to (0,7), (8,1) to (7,0), (1,1) to (0,0); seat 3: (8,8) to (9,7), (1,1) to (2,0), (8,1) to (9,0)); and all seven pawns of a seat have a legal first move (8 pawn moves per seat at turn 1).

### 1.4 Lobby, seating, start

A lobby holds 2 to 4 humans (`MIN_PLAYERS = 2`, `MAX_PLAYERS = 4`). The player who created it is the creator. Seats are assigned from join order through the fixed table `SEAT_BY_JOIN = [0, 2, 1, 3]`: the creator sits at seat 0, the second joiner at seat 2 (the diagonal corner), the third at seat 1, the fourth at seat 3. The table is re-applied on every roster change while Waiting, so two players are always diagonal; no RNG is involved anywhere. If the creator leaves while Waiting, the longest-present member becomes creator and moves to seat 0.

The start notification of the spec is the server message `CanStart { players }`, sent to the creator whenever the lobby has at least `MIN_PLAYERS` humans and again on every roster change while that holds (with four humans it is the "table is full" cue). Only the creator may send `Start`; it is accepted only while Waiting with at least `MIN_PLAYERS` humans, otherwise `Rejected` with the reason. Nobody may join a lobby whose game is Playing (`LobbyInfo.playing`); there are no spectators in v1.

While Waiting, every member may edit their formation within the class rules of section 2 (`SetFormation`); the board is rebuilt and broadcast on every join, leave or formation change so the page shows who sits where. Formations are frozen at Start. Seat 0's clock starts on the tick the server accepts `Start`.

Corners with no human (2- and 3-player games) are garrisons: set up in full with the default formation, marked `garrison = true` and `alive = false` from turn 0. Garrison pieces never move (the seat never gets a turn), block movement like any piece, and are foreign to every seat, so anyone may capture them. Their king is an ordinary inert piece; capturing it changes nothing further. Garrisons exist so a short-handed game has no empty corner to promote into unopposed; a seated player who is eliminated gets the opposite treatment (section 1.7).

Hotseat (`start_local`) is four human seats on one keyboard with the same engine, the default formation and the same 15 s clock run by the client; there are no garrisons and no AI in v1. A local game therefore always has at least two non-garrison seats, matching the online gate, and cannot finish at its first `end_turn`.

### 1.5 Turn structure and the 15-second clock

Play proceeds 0, 1, 2, 3, 0, ... skipping seats that are not alive (garrisons and eliminated seats). One turn = one seat = exactly one action, and every action ends the turn. Seat 0 moves first on global turn `n = 1`. Every completed turn (a move, a forced pass or a timeout) increments `n`; skipped seats consume no turn number. Each seat also has its own counter `own_turns`, incremented on the start of each of its own turns, including turns that end as a timeout or a forced pass; `own_turns` is never touched by other seats' eliminations. A round is one pass over the alive seats.

The server is the sole clock. A turn lasts `TURN_MS = 15_000` ms of server time from the moment the server applied the previous action (or timeout) and broadcast the new state. The server repeats `Clock { turn, seat, left_ms }` every `CLOCK_EVERY_MS = 1000` ms; the client counts down between messages and never extrapolates past what it was told. A move is accepted while less than `TURN_MS + GRACE_MS` (`GRACE_MS = 300`) ms of server time have elapsed, so a move sent at the displayed 0.0 still lands; the timeout pass is applied when elapsed time reaches `TURN_MS + GRACE_MS`. A move arriving after the timeout has been applied is refused because the turn has ended. An illegal move is `Rejected` to its sender, leaves the board and the clock untouched, and the player may try again until the deadline. Hotseat uses the same `TurnClock` without grace.

Timeout: the turn ends with no board change; `seats[seat].timeouts += 1`. If that reaches `TIMEOUTS_TO_ELIMINATE = 3` the seat is eliminated (section 1.7). A legal move resets the seat's `timeouts` to 0. A timeout counts as a quiet turn (section 1.8) and resets the stall counter.

Forced pass: if at the start of a seat's turn the union of `targets()` over its pieces is empty, the server passes instantly without starting the clock. A forced pass is not a timeout and does not touch `timeouts`; it increments the global `stalls` counter (a legal move by anyone, and a timeout, resets `stalls` to 0). If `stalls` reaches the number of alive seats, every alive seat has been unable to move in a row and the game ends by material ranking (section 1.8).

There is no voluntary pass. A player who has any legal action must take one or let the clock run out, which costs a timeout mark. A move is stamped with the `turn` it was computed against; a stale stamp is refused, never applied to a later turn.

### 1.6 Actions and piece rules

Every action is one `Move { from, to }`. It is legal iff `board[from]` is a piece owned by the seat to move and `to` is in `targets(state, from)`. The joker's step, teleport, placement and capture, the hero's swap and awakening, and every ordinary move are instances of this one shape; the only self-move (`from == to`) in the game is the dormant hero's awaken-in-place, and `from == to` is rejected for every other kind and situation. A joker placement never targets the joker's own tile.

Foreign means `piece.owner != mover`: every other seat's pieces, including garrison pieces. Empty means no piece. Moving onto a foreign piece captures it: the piece is removed permanently and its kind is appended to the mover's `captured` list. A piece never moves onto an own piece, with the single exception of the hero swap, which removes the own pawn without crediting a capture. A move captures at most one piece. Sliding pieces stop at the first piece on their line, capturing it if foreign. There is no check, no checkmate, no stalemate and no castling; a king may step onto an attacked tile, and the king is captured like any other piece.

Helpers: `steps(from, D)` = `{ from + d | d in D, on board }`; `rays(from, D)` = for each `d` in `D`, walk `from + d, from + 2d, ...` while on board, including an empty tile and continuing, including a foreign tile and stopping, stopping before an own tile. `ALL8` = the eight king directions; `ORTHO4` = `(+-1,0), (0,+-1)`; `DIAG4` = `(+-1,+-1)`; `KNIGHT8` = `(+-1,+-2), (+-2,+-1)`. `f`, `l` are the mover's seat vectors. `mirrors(t)` = `{ (9-x, y), (x, 9-y), (9-x, 9-y) }`.

| Kind | `targets(state, from)` |
|---|---|
| King | `steps(from, ALL8)` that are empty or foreign. |
| Queen | `rays(from, ALL8)`. |
| Rook | `rays(from, ORTHO4)`. |
| Bishop | `rays(from, DIAG4)`. |
| Knight | `steps(from, KNIGHT8)` that are empty or foreign. |
| Pawn | Moves: `from + f` and `from + l`, each if on board and empty. Captures: `from + d` for `d` in `PAWN_CAPTURES = [f+l, f-l, -f+l]`, each if on board and foreign. One tile only, never backward (`-f-l` is never a capture, `-f` and `-l` never a move), no double step, no en passant. |
| Joker | Step: `steps(from, ALL8)` that are empty (never a capture; `JOKER_STEP = true` is a one-line playtest knob). Teleport: each of `mirrors(from)` that is empty; pieces in between are irrelevant, never a capture. Placement: only while the owner is the seat to move and `seats[owner].own_turns > 0 && seats[owner].own_turns % JOKER_PLACE_EVERY == 0` (`JOKER_PLACE_EVERY = 5`, so the owner's own turns 5, 10, 15, ...; a client computing highlights for a seat that is not to move gets none), every empty tile of the board except `from`; never a capture; an unused placement is not banked. Capture: `from + f + l` (front-left in the OWNER's frame, wherever the joker stands) if on board and foreign. This is the joker's only capture. |
| Hero (dormant) | Every tile holding an own Pawn, anywhere on the board (the swap). If the owner has no Pawn on the board: `{ from }` (awaken in place). Nothing else: a dormant hero cannot move or capture, but it blocks and can be captured. |
| HeroAwake | `rays(from, ORTHO4)` plus `steps(from, KNIGHT8)` that are empty or foreign: rook and knight combined, moving and capturing. |

Pawn details. A pawn marches one tile along either of its two outward axes, forward `f` or left `l` (for seat 0: east or north), which is what makes both arms of the tier-2 L mobile from turn 1. It captures on the three diagonals that are not its backward diagonal; this is the union of the chess capture sets of its two forward directions. `PAWN_CAPTURES` is a named constant pinned by the frame test; the documented fallback if playtesting finds the third diagonal too strong is `[f+l, f-l]` (the chess captures of `f` only). Promotion: a pawn that ends a move on a tile where `to + f` or `to + l` is off the board (`u = 9` or `v = 9`; for seat 0 `x = 9` or `y = 9`) is replaced in the same action by a Queen of its owner. Promotion is automatic and always to Queen (no choice on the wire); a promoted piece is a Queen, not a Pawn, so it is no longer a hero swap target.

Joker details. Facing never changes: a seat-0 joker standing in seat 2's block still captures at `(+1,+1)`. A joker standing on `u = 9` or `v = 9` has no capture until it steps, teleports or is placed elsewhere. The three mirrors from the start tile are the other three seats' joker tiles, so at turn 1 every teleport is blocked by a live or garrison joker; the step and the fifth-own-turn placement unblock it. The centre mirror of the joker's start tile is the diagonal opponent's joker tile, whose front-left is that opponent's king tile: a joker teleporting onto a vacated diagonal joker tile threatens the king next turn. That is intended; vacating your joker tile is a visible risk. A captured joker is gone; placement moves a living joker and never respawns one.

Hero details. The swap is the owner's whole turn: the chosen own pawn is removed from the game (credited to nobody), the hero moves to its tile, the hero's old tile is left empty, the hero becomes `HeroAwake` and acts from the owner's next turn. Once per game by construction (there is no second dormant hero). Awaken-in-place exists only so a hero can never be stranded asleep: it is legal only while the owner has zero pawns on the board, and it consumes the turn like the swap. An awake hero never sleeps again and never promotes.

Narration. The server derives `ActionKind` for `LastAction` from the applied move, in this order: dormant Hero mover with `to == from` is `HeroWake`; dormant Hero mover otherwise is `HeroSwap`; Joker mover with `|dx| <= 1 && |dy| <= 1` is `Move` (step or front-left capture); Joker mover with `to in mirrors(from)` is `JokerTeleport`; any other Joker move is `JokerPlace`; everything else is `Move`. A tile reachable two ways (a row mirror is also a step from `x in {4,5}`) has one result and is narrated as the first matching kind; nothing is spent either way because placement is a schedule, not a charge.

### 1.7 Elimination

A seated player is eliminated the moment one of these happens: their King is captured by any foreign piece; they time out `TIMEOUTS_TO_ELIMINATE = 3` own turns with no legal move of theirs in between (a forced pass in between neither counts nor resets the count; only a legal move resets it, as section 1.5 says); their connection drops during Playing (v1 has no reconnection, so a lost socket is a lost seat; while Waiting a leave merely frees the seat).

On elimination the seat's `alive` becomes false, every piece it still owns is removed from the board immediately (before the next turn starts), and the seat is skipped in turn order from then on. Removal, not freezing, is deliberate: inert walls create unreachable kings and stalemate traps. Garrisons are the one class of inert material, they sit in corners, and they are capturable. Capturing a king whose owner is not alive (a garrison king) is an ordinary capture with no further effect.

Order of operations on a timeout: `timeouts += 1`, then elimination if the threshold is reached, then `quiet += 1`, `stalls = 0`, then `end_turn`. A disconnect of the seat to move ends that turn immediately after the elimination.

### 1.8 End of the game

The game ends the instant only one alive seated seat remains: that seat wins (`end = LastKing`). It also ends the instant no alive seated seat remains, which is reachable only through the last player disconnecting (`end = Abandoned`, no winner).

Three further terminators guarantee every game finishes without RNG, and all three resolve by material ranking: `NoProgress` when `quiet` reaches `NO_PROGRESS_TURNS = 100`, where `quiet` counts consecutive completed turns (all seats, including timeouts and forced passes) with no capture, no pawn move, no promotion and no hero swap or awakening, and is reset to 0 by any of those progress events; `Stalemate` when `stalls` reaches the number of alive seats (a full round of forced passes); `TurnCap` when `turn` reaches `MAX_TURNS = 600` (150 rounds of four), a backstop that the no-progress rule makes almost unreachable.

Material ranking: each alive seated seat sums `MATERIAL` over its pieces: Queen 9, HeroAwake 8, Rook 5, Joker 4, Knight 3, Bishop 3, Hero (dormant) 3, Pawn 1, King 0. The seat with the strictly greatest sum wins; if two or more share the maximum the result is a draw (`winner = None`). The values are tunable and pinned by a test; the tiebreak is deterministic by construction.

After a result the server sends `Phase { Finished, winner, end }`, holds it for `RESULTS_SECS = 10`, then returns the lobby to Waiting with the same members, seats and creator; a new game needs a new `Start`.

### 1.9 The engine's turn loop, exactly

`apply(state, from, to)`, given `to in targets(state, from)`: (1) `victim = board[to]`; `progress = victim.is_some() || mover is Pawn || mover is dormant Hero`. (2) Move the piece: `board[to] = board[from]; board[from] = None` (for `to == from` the piece stays). If `victim` is foreign, push its kind onto the mover's `captured`; if it was the own pawn of a hero swap, push nothing. (3) If `victim` is a King whose owner is alive, `eliminate(owner)`. (4) If the mover is a Pawn and `to + f` or `to + l` is off the board, it becomes a Queen. (5) If the mover is a dormant Hero, it becomes `HeroAwake`. (6) `seats[mover].timeouts = 0; stalls = 0; quiet = if progress { 0 } else { quiet + 1 }`; record `last`; `end_turn`.

`eliminate(seat)`: `alive = false`; if the seat is not a garrison, remove every piece it owns.

`end_turn`: (1) if exactly one seated seat is alive, `result = Winner(seat), end = LastKing`; if none, `end = Abandoned`; return. (2) if `quiet >= NO_PROGRESS_TURNS`, `finish_by_material(NoProgress)`; return. (3) if `turn >= MAX_TURNS`, `finish_by_material(TurnCap)`; return. (4) `turn += 1`; `to_move` = next alive seat after `to_move` in 0, 1, 2, 3 order; `seats[to_move].own_turns += 1`; `clock = TURN_MS`. (5) If `to_move` has no legal move: `stalls += 1; quiet += 1; last = Pass { seat }`; if `stalls >= alive_count`, `finish_by_material(Stalemate)`; else go to (1). The loop is bounded because `quiet` grows on every pass.

`timeout(state)`: as in section 1.7, then `end_turn`. `disconnect(state, seat)`: `present = false`; if alive, `eliminate(seat)`; if `seat == to_move`, `last = Pass { seat, eliminated: Some(seat) }` and `end_turn`; otherwise the end-of-game check (`end_turn` steps 1 to 3) runs immediately without advancing the turn, and the seat is skipped when its turn would have come. In both cases the win is awarded the instant one seat is left, as section 1.8 requires.

## 2. Classes and figure cards (v1)

The three character classes are the three rings of the diagram (the "Klassen" caption sits beside them), in the usual rarity order:

| Class | Ring | Pieces |
|---|---|---|
| Legend | corner 2x2, local (0,0) (1,0) (0,1) (1,1) | King, Queen, Hero, Joker |
| Epic | tier 1, local (2,0) (2,1) (2,2) (1,2) (0,2) | Rook, Rook, Bishop, Bishop, Knight |
| Common | tier 2, the seven tier-2 tiles | Pawn x7 |

In v1 a piece's class decides only where it may start; it has no effect on movement. "The pawn class starts with the normal chess pieces as standard, but with joker and hero added" is read as: the default figure-card set is standard chess plus the two new legends.

A figure card in v1 is a piece kind with the rule text of section 1.6; there is exactly one card per piece and no variants. "Cards can be exchanged before the game starts" is implemented as the within-class formation swap: while the lobby is Waiting, a player may send `SetFormation { legend: [Kind; 4], epic: [Kind; 5] }` naming the kinds for the four Legend tiles in the order (0,0) (1,0) (0,1) (1,1) and the five Epic tiles in the order (2,0) (2,1) (2,2) (1,2) (0,2). The server accepts it iff `legend` is a permutation of `{King, Queen, Hero, Joker}`, `epic` is a permutation of the multiset `{Rook, Rook, Bishop, Bishop, Knight}`, and the two bishops end on tiles of opposite colour (parity of `x + y` of their global tiles; the multiset check alone would admit (2,0) plus (2,2)). Commons are all pawns, so there is nothing to swap. The default is the table of section 1.3, so a client that never sends the message plays the default game. Cards never cross the class boundary and never change hands between players.

The swap is a real choice. The king may leave the corner tile; the joker may be moved to (0,0), (1,0) or (0,1), which keeps its front-left on the board (verified: front-left is on the board from every home tile) but makes one of its mirror tiles land on `u = 9` or `v = 9`, where it cannot capture; the knight may take the ring elbow. The validator does not judge these, the HUD shows the joker's capture tile, and the player owns the consequence. Card variants (alternative rule texts per piece) and trading between players are the stated follow-up and need a protocol bump, because a peer that does not know a card plays a different game.

## 3. Assumptions where the spec is silent

- Seat numbering follows the diagram's corner labels 1 (SE), 2 (NE), 3 (NW), 4 (SW); the engine uses indices 0..=3 with 0 = SW so that seat 0's local frame is the identity, and the diagram's "4" is read as both the seat label and the count of first-square units, consistent with the other three corners.
- Turn order is counter-clockwise 0, 1, 2, 3; the spec names no order and this is the diagram's cycle.
- Forward per seat points toward the next seat, left is forward rotated CCW, frames are rotations never reflections: this is the only assignment under which the literal "front-left" points at the board centre from every home tile of every corner.
- Legend layout King (0,0), Queen (1,0), Hero (0,1), Joker (1,1): the spec gives counts per ring only; (1,1) is the tile from which all three joker mirrors are capture-live on landing (verified), and it keeps the immobile hero off the ring's most central tile.
- Epic layout R (2,0) B (2,1) B (2,2) N (1,2) R (0,2): asymmetric on purpose so the two bishops start on opposite colours (verified per seat).
- Pawns move along either outward axis: one direction per seat leaves three of seven pawns queued behind the elbow pawn at turn 1, which is not a playable formation; two directions are the smallest change that gives every pawn a first move.
- Pawn captures on the three non-backward diagonals: the union of the chess capture sets of the two forward directions, kept as a named constant with the two-diagonal fallback documented, because it is a balance knob not a rules question.
- No double step and no en passant: the double step needs a per-piece moved flag (a pawn can reach another pawn's vacated start tile) and en passant is the only cross-turn state and the only rule that could not be made symmetric across four seats without inventing a holder; neither is worth that in v1.
- Promotion is automatic and always to Queen: a choice cannot be forced under a 15 s clock and would be the only action needing a second field; underpromotion is a backlog line.
- Joker teleport targets are all three mirror tiles (row, column, centre): "the tile exact across him in a straight line" admits the row and column readings and the centre reading, and offering all three is the superset that never makes a design argument the player's problem.
- Joker has a one-tile non-capturing step: the spec does not list it, but without it the joker's only non-placement moves at the start are three blocked teleports, and a piece idle until its fifth turn is dead weight; it is a one-line knob (`JOKER_STEP`).
- "Every 5 turns" counts the OWNER's own turns (5, 10, 15, ...), timed-out and forced-pass turns included, not banked: the literal per-piece cadence, the same for every corner in every round, and a per-seat `u32` costs nothing; a global `n % 5` would give seat 0 its first placement on its second turn and seat 3 on its fifth.
- Teleport and placement need an empty target and never capture, because the spec says the joker can "only hit" its front-left tile.
- Joker facing is its owner's facing everywhere (pieces do not turn), so front-left is always the owner's `f + l`.
- "The joker can be placed at any tile" moves a living joker; a captured joker stays captured.
- Hero swap "at any time" means on any of the owner's turns, unscheduled, and it consumes the turn: a free swap followed by a rook-plus-knight capture in the same turn would allow a swap-then-take-the-king combination that dominates the game.
- Hero "can attack like a rook and knight combined" is read as moves and captures like both (the chancellor idiom), not captures only.
- A dormant hero cannot capture ("can not move"), can be captured, and may awaken in place when its owner has no pawn, so no piece is ever dead weight forever.
- No check, checkmate, stalemate-draw or castling: check across four players is undefined and expensive, and king capture is the simplest literal loss condition.
- Timeout is a pass; three consecutive timeouts eliminate; a forced pass is instant and is not a timeout: an absent player must not hold a table for more than about a minute of its own clock, and a boxed-in player must not be eliminated for being boxed in.
- Disconnect is immediate elimination: v1 has no reconnection, and a seat that can never act again is functionally eliminated anyway.
- An eliminated seated player's pieces are removed immediately; never-seated corners are inert capturable garrisons: removal closes walled-in kings, garrisons close unopposed promotion into empty corners, and both reuse one setup and one elimination path with a single flag.
- Seating by the fixed join-order table [0, 2, 1, 3]: two players sit diagonally, the creator moves first, and the core stays RNG-free; random seating would need a seed on the wire for no gain.
- The start notification is `CanStart` to the creator from two humans upward, repeated on roster changes; the spec's four-player lobby still gets its "full" cue at four.
- Termination by 100 quiet turns, a full round of forced passes, or 600 turns, all resolved by material ranking with a draw on a tied maximum: the spec has no termination rule and two bare kings would otherwise run forever; timeouts reset the stall counter so an AFK opponent cannot convert a stalled seat's passes into a draw before the third timeout eliminates him.
- Material values (Queen 9, HeroAwake 8, Rook 5, Joker 4, Knight 3, Bishop 3, Hero 3, Pawn 1, King 0) are tunable and pinned by a test.
- The neutral cross has no special rule; the diagram shows only geometry.
- The clock is the server's, in milliseconds, with a 300 ms grace after the displayed deadline; a client's countdown is display only.
- No voluntary pass: zugzwang is part of chess, and a free pass would let a player sit out the fighting at no cost.
- Classes map Legend = corner 2x2, Epic = tier 1, Common = tier 2 pawns, in the usual rarity order, because the diagram's "Klassen" caption pairs the classes with the three rings.
- The v1 figure-card exchange is the within-class formation swap: a no-op would make the spec's sentence false, and new piece kinds need art, rules and balance that do not exist yet.
- Two marks on the diagram that read as "OV" (beside seat 1 and below the board) are not interpreted.

## 4. Architecture

### 4.1 Where to build, and the shape

Branch `feat/four-kings`, checked out at `C:\Users\Admin\dev\ember\.claude\worktrees\kings` (base `502414c`, carrying arena v12). Three crates mirror Fire Racer file for file: `kings-core` (rules and wire, no glam, no floats, no RNG), `kings-server` (authoritative hub), `kings` (client, native and wasm). Page `web/games/kings/v1/index.html`. Every build runs through `wsl -d claude-sdk --cd '<win path>' -- bash -lc 'CARGO_TARGET_DIR=$HOME/targets/ember chrt --idle 0 ionice -c3 cargo ...'`, timed and reported; the first commit lands early because worktree isolation reaps an uncommitted worktree.

The engine contract that both the server validator and the client are written against: `Piece { id: u8, owner: u8, kind: Kind }` with `HeroAwake` a distinct kind, so a piece carries no other state; one `Move { from, to }` action for everything; pure `setup()`, `targets()`, `apply()`, `timeout()`, `disconnect()` over a `State`; `setup()` bit-identical on server and client and compared tile by tile in a test. The client calls `targets()` for highlights only and `apply()` on the server's echo, never speculatively; `kings-core` is shared for highlights, not prediction, which keeps it outside the arena's rollback concerns.

```
State {
  board:   [Option<Piece>; 100],   // i = y * 10 + x
  seats:   [Seat; 4],              // Seat { present, alive, garrison, own_turns: u32, timeouts: u8, captured: Vec<Kind> }
  to_move: u8,
  turn:    u32,                    // global turn number, starts at 1
  quiet:   u32,                    // completed turns since the last progress event
  stalls:  u8,                     // consecutive forced passes
  clock:   TurnClock,              // left_ms; fed by the server hub or the hotseat client
  last:    Option<LastAction>,
  result:  Option<(Option<u8>, EndReason)>,
}
```

### 4.2 Crates and files

- NEW `crates/kings-core/Cargo.toml`: serde, serde_json only.
- NEW `crates/kings-core/src/lib.rs`: module list and the independence-from-pong/fire doc comment.
- NEW `crates/kings-core/src/proto.rs`: `PROTO_VERSION = 1`, the wire constants, `C2S`/`S2C`, wire structs, `sanitize`/`sanitize_handle`/`is_transient_read` copied from `fire-core` (kings cannot depend on fire-core; lifting them to a shared crate is a backlog line), house-style tests.
- NEW `crates/kings-core/src/board.rs`: `State`, `Piece`, `Seat`, `Kind`, the seat frame table (`f`, `l`, `to_global`, `to_local`, `mirrors`), `SETUP_LOCAL`, `SEAT_BY_JOIN`, `setup(present: [bool; 4], formations: [Formation; 4]) -> State`, `Formation { legend: [Kind; 4], epic: [Kind; 5] }` with `validate()`, `to_state(&State) -> BoardState`, `from_state(&BoardState) -> State`.
- NEW `crates/kings-core/src/rules.rs`: the rule constants (`JOKER_STEP`, `JOKER_PLACE_EVERY`, `PAWN_CAPTURES`, `TIMEOUTS_TO_ELIMINATE`, `NO_PROGRESS_TURNS`, `MAX_TURNS`, `MATERIAL`), `Target { x, y, kind: TargetKind { Move, Capture, Teleport, Place, Swap, Wake } }`, `targets(&State, from) -> Vec<Target>`, `apply(&mut State, from, to) -> Result<Applied, Illegal>`, `timeout`, `disconnect`, `end_turn`, `eliminate`, `finish_by_material`, `action_kind_of` (the narration derivation), `Illegal::reason()` whose text becomes `Rejected.reason`.
- NEW `crates/kings-core/src/clock.rs`: `TurnClock { left_ms: u32 }`, `reset()`, `tick(ms) -> bool` (true once `TURN_MS + GRACE_MS` has elapsed), `display_left_ms()` clamped to `TURN_MS..0`; shared by the hotseat game and the server so both count the same way.
- NEW `crates/kings-server/Cargo.toml`, `build.rs` (rerun on `EMBER_BUILD_VERSION`/`EMBER_BUILD_COMMIT`), `src/lib.rs`, `src/main.rs` (`kings-server [bind] [--name x]`, default `127.0.0.1:7782`; 7780 is the arena, 7781 fire), `examples/probe.rs`, `tests/ws_e2e.rs`.
- NEW `crates/kings/Cargo.toml` (cdylib + rlib, bin `kings-app`, ember-engine, kings-core, wasm-bindgen/web-sys on wasm, tungstenite natively, dev-dep kings-server), `src/lib.rs` (`run_local`, `run_online`, the wasm API), `src/main.rs` (`kings-app` hotseat, or `kings-app online <ws> create|join <lobby> [pw|-] [handle]`), `src/game.rs` (mesh ids in registration order, seat colours, tile and camera maths, `scene()`, HUD thread-local, `UiCmd` queue, keyboard cursor in the local seat's frame), `src/meshes.rs` (faceted primitives for the nine silhouettes plus tile, disc and ring), `src/ui.rs` (the `Selection` machine), `src/hotseat.rs`, `src/net.rs` (fire's `Net` over `kings_core::proto`), `src/online.rs` (pure client state), `src/online_game.rs`, `tests/online_e2e.rs`.
- NEW `web/games/kings/v1/index.html`, NEW `docs/kings-rules.md` (a pointer to sections 1 to 3 of this document, so the rules have one home).
- NEW `deploy/deploy-kings-online.sh`, `deploy/wsl-detach.ps1`, `deploy/merge-server-json.py`.
- CHANGE `Cargo.toml` (three members), `Cargo.lock`, `deploy/deploy-pages.sh`, `web/games.json`, `web/index.html`, `README.md`, `docs/plans/backlog.md`; outside the repo, the claude-sdk toolchain list in `C:\Users\Admin\.claude\CLAUDE.md`.

### 4.3 Server (`kings-server`)

Structure is fire-server's: a thread per connection owning its socket (5 ms read timeout, bounded `OUTBOUND_QUEUE 256`, handshake deadline, `MAX_MSGS_PER_TICK 32`), one hub thread owning `HashMap<String, Lobby>`, everything over `mpsc`. There is no sim to step: the hub loop polls every 10 ms, drains events, then calls the pure `tick_lobby(&mut Lobby, conns, elapsed_ms)` for every lobby, which advances the `TurnClock`, sends `Clock` when 1000 ms have accumulated, and on expiry applies `timeout()` and broadcasts `State`. Tests drive `tick_lobby` with synthetic elapsed time and never sleep.

`Lobby { password, creator: u64, members: Vec<u64>, formations: HashMap<u64, Formation>, state: State, phase, clock_since_ms, results_left_ms }`. Seats are `SEAT_BY_JOIN[position in members]`, recomputed on every Waiting roster change and broadcast as `Roster`. `SetFormation` in Waiting validates, stores per member, rebuilds the Waiting board and broadcasts `State`; anything invalid is `Rejected`. `Start`: creator only, Waiting only, at least `MIN_PLAYERS` members, else `Rejected`; builds `setup(present, formations)` with garrisons for absent corners, sends `Phase { Playing }` and `State`, resets the clock. A `Move` is checked for phase, `seat == state.to_move`, `turn == state.turn`, then `rules::apply`; `Err` is `Rejected` to the sender only, `Ok` is `State` to all and the clock resets. Leaving mid-game calls `disconnect(seat)` and broadcasts `Roster` and `State`. Finished holds `RESULTS_SECS`, then Waiting with the same members. Version gate is exact equality, listing is ungated, `Welcome` carries host, version, commit, players and lobbies from day one.

### 4.4 Client (`kings`)

`scene(&State, &Meshes, view, cam) -> Frame` is shared by hotseat and online: 100 tile instances (two-tone checker, corner blocks tinted 15 percent toward the seat colour, cross neutral), one instance per piece (kind mesh, seat colour, HeroAwake drawn 1.2x on a base ring), a flat disc per legal target coloured by `TargetKind` (green move, red capture, violet teleport, amber place, cyan swap or wake), a ring on the selected tile and on the keyboard cursor, the last move marked with two dim discs. No text and no transparency in the wasm: the page draws everything narrative. Camera per seat: eye on the seat's corner diagonal, about 12 up and 11 out, target the centre, fov about 50 so the whole board fits; hotseat lerps to the active seat's corner in 0.4 s.

`ui.rs` is a pure `Selection` machine over `rules::targets`: Idle, click an own piece (or, while Waiting, an own Legend or Epic piece) to Selected, click a target to emit `Move { turn, from, to }` (or, while Waiting, a second own piece of the same class to emit `SetFormation` with the two tiles swapped), anything else clears; `pending` is set until the next `State` or `Rejected`. Keyboard: arrows and WASD move the cursor in the local seat's frame (Right is `+u`, forward, and Up is `+v`, left: the pure rotation that puts the local corner bottom-left on screen, which is also how the page draws its 2D board), rising-edge latched like fire's boost; Enter or Space clicks, Esc clears.

`online.rs` is pure client state (`welcomed, screen, my_seat, creator, phase, winner, end, state: Option<State>, roster, lobbies, notice, pending, can_start`) with `apply(S2C)` and `tick(dt)` for the countdown; `online_game.rs` wires `Net`, drains `UiCmd`s, publishes the HUD, and calls `scene()`; `Net` itself sends `Hello` on open and `Ping` every `CLIENT_PING_SECS` off the frame loop (a JS interval on the web, the reader thread natively), so a hidden tab keeps its seat.

### 4.5 Wire protocol (`kings-core/src/proto.rs`)

Changes from the architecture plan, and why: `HeroSwap` was a free action, the rules make it the whole turn, so it is no longer its own message; with placement a fixed schedule instead of a charge, no action needs a kind on the wire, so the four action variants collapse into one `Move` (one validator, no precedence rule in the client); voluntary `Pass` is gone with the rule; `SetFormation` carries the card swap; `Roster` replaces `PlayerJoined`, `PlayerLeft` and `Creator` because seats are recomputed on every roster change and a full roster cannot get out of step; `CanStart` is the spec's notification; `Kind` gains `HeroAwake` and `PieceState` drops `awake`; `SeatState` carries `own_turns` and `timeouts` instead of a derived `joker_ready_in` and a redundant `hero_used`; `LastAction` gains `promoted` and `eliminated`; `Phase` gains `end`.

```rust
//! The Four Kings wire protocol: JSON over WebSocket.
//!
//! Its own `PROTO_VERSION`, independent of `pong_core` and `fire_core`, for
//! the reason `fire_core::proto` gives: the join gate is exact equality, and
//! a board-game message must never be able to make the arena or the racer
//! list-only. Same house style: `#[serde(tag = "t", rename_all =
//! "snake_case")]`, JSON text frames, `#[serde(default)]` on anything added
//! after v1, and for every defaulted field a comment saying what an old peer
//! DOES when it is absent.

use serde::{Deserialize, Serialize};

/// Kings' own protocol version.
pub const PROTO_VERSION: u16 = 1;

pub const MAX_HANDLE_LEN: usize = 20;
pub const MAX_LOBBY_LEN: usize = 24;
pub const MAX_PASSWORD_LEN: usize = 40;
/// Seats: one per corner of the board.
pub const MAX_PLAYERS: u8 = 4;
/// The creator may start with this many; empty corners become garrisons.
pub const MIN_PLAYERS: u8 = 2;
/// Per-turn budget shown to the player, enforced by the server.
pub const TURN_MS: u32 = 15_000;
/// Server-side grace after `TURN_MS`: a move that arrives before
/// `TURN_MS + GRACE_MS` of server time is still applied; at that instant
/// the server applies the timeout pass instead.
pub const GRACE_MS: u32 = 300;
/// How often the server repeats the clock while a game is running.
pub const CLOCK_EVERY_MS: u32 = 1000;
/// How long Finished is shown before the lobby returns to Waiting.
pub const RESULTS_SECS: u32 = 10;
/// Clients ping at least this often; there is no input stream to keep the
/// connection alive as the racer has.
pub const CLIENT_PING_SECS: u64 = 5;
pub const CLIENT_TIMEOUT_SECS: u64 = 30;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
    Joker,
    /// Dormant: cannot move or capture; its only moves are the swap onto
    /// an own pawn and, with no pawns left, awakening in place.
    Hero,
    /// Awakened: moves and captures as rook and knight combined.
    HeroAwake,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// In the lobby; the board shows the setup for the seats held so far.
    Waiting,
    Playing,
    Finished,
}

/// Why a Finished game ended.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// One seated player still has a king.
    LastKing,
    /// `NO_PROGRESS_TURNS` turns without capture, pawn move or hero swap;
    /// resolved by material.
    NoProgress,
    /// Every alive seat was forced to pass in a row; resolved by material.
    Stalemate,
    /// `MAX_TURNS` reached; resolved by material.
    TurnCap,
    /// The last seated player left.
    Abandoned,
}

/// What the last applied action was, so the page can narrate it. Derived by
/// the server from the applied `Move`; the client never sends it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Any ordinary move or capture, including the joker's step and its
    /// front-left capture and an awakened hero's move.
    Move,
    JokerTeleport,
    JokerPlace,
    HeroSwap,
    /// Hero awakened in place (no pawn left to swap with).
    HeroWake,
    /// A forced pass: the seat had no legal move. Never a timeout.
    Pass,
    /// A pass the server made because the clock ran out.
    Timeout,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LobbyInfo {
    pub name: String,
    pub host: String,
    pub has_password: bool,
    pub players: u8,
    pub cap: u8,
    /// True once the game has started; joining is refused until it resets.
    #[serde(default)]
    pub playing: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerMeta {
    pub id: u8,
    pub handle: String,
    /// Corner, which also picks the colour: every client paints the same
    /// seat the same colour without another round trip.
    pub seat: u8,
}

/// The pre-game card swap: kinds for the four Legend tiles in the order
/// local (0,0) (1,0) (0,1) (1,1), and for the five Epic tiles in the order
/// local (2,0) (2,1) (2,2) (1,2) (0,2).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Formation {
    pub legend: [Kind; 4],
    pub epic: [Kind; 5],
}

/// One piece in a board broadcast. Flat scalars: the rules crate's `Piece`
/// can be refactored without that being a protocol question.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceState {
    /// `seat * 16 + setup index`; stable for the whole game.
    pub id: u8,
    pub owner: u8,
    pub kind: Kind,
    pub x: u8,
    pub y: u8,
}

/// Per-corner bookkeeping.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SeatState {
    pub seat: u8,
    /// A human holds this corner right now. False for a garrison and for a
    /// player who left mid-game.
    pub present: bool,
    /// Takes turns and can win. False for garrisons and eliminated seats.
    pub alive: bool,
    /// A never-seated corner whose pieces stand inert and capturable.
    pub garrison: bool,
    /// Own turns started so far, timeouts and forced passes included. The
    /// joker may be placed on own turns 5, 10, 15, ... (`own_turns % 5 == 0`
    /// while this seat is to move).
    pub own_turns: u32,
    /// Consecutive own-turn timeouts; three eliminate.
    pub timeouts: u8,
    /// Enemy pieces this seat has taken, in order.
    pub captured: Vec<Kind>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastAction {
    pub seat: u8,
    pub kind: ActionKind,
    /// From/to tiles; for a pass or timeout all four are 0 and the page
    /// ignores them.
    pub fx: u8,
    pub fy: u8,
    pub tx: u8,
    pub ty: u8,
    pub captured: Option<Kind>,
    /// The mover was a pawn that became a queen.
    pub promoted: bool,
    /// A seat eliminated by this action (king captured, third timeout,
    /// disconnect).
    pub eliminated: Option<u8>,
}

/// The whole board. Sent in full on every change: at 64 pieces it is a few
/// kilobytes, and a full snapshot cannot get out of step the way a stream
/// of deltas can (the racer's one-shot events already showed what a missed
/// message costs).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BoardState {
    /// Increments on every completed turn. Moves carry it back so a stale
    /// intent is refused rather than applied to the wrong turn.
    pub turn: u32,
    /// Whose turn it is.
    pub seat: u8,
    /// Time left on this turn when the snapshot was taken.
    pub left_ms: u32,
    /// Turns since the last capture, pawn move or hero swap.
    pub quiet: u32,
    /// Consecutive forced passes.
    pub stalls: u8,
    pub pieces: Vec<PieceState>,
    /// Always four entries, indexed by seat.
    pub seats: Vec<SeatState>,
    #[serde(default)]
    pub last: Option<LastAction>,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum C2S {
    /// Must be the first message on a connection.
    Hello { proto: u16, handle: String },
    ListLobbies,
    CreateLobby { name: String, password: Option<String> },
    JoinLobby { name: String, password: Option<String> },
    LeaveLobby,
    /// Waiting only. Validated as a within-class permutation with the two
    /// bishops on opposite colours; the Waiting board is rebuilt and
    /// broadcast. Invalid: `Rejected`, formation unchanged.
    SetFormation { formation: Formation },
    /// Creator only, Waiting only, at least `MIN_PLAYERS` seated. Anyone
    /// else gets `Rejected` with the reason.
    Start,
    /// The one action shape. Ordinary moves, the joker's step, teleport
    /// (a mirror tile), placement (any empty tile on own turns 5, 10, ...)
    /// and front-left capture, the hero's swap (`to` = own pawn) and its
    /// awakening in place (`to == from`, only with no pawns). `turn` must
    /// equal the current `BoardState::turn`.
    Move { turn: u32, fx: u8, fy: u8, tx: u8, ty: u8 },
    /// Liveness. There is no input stream, so this is the keepalive.
    Ping { nonce: u32 },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    /// Reply to a valid Hello. The identity fields follow `docs/hosts.md`
    /// and are informational: a peer that ignores them plays the same game.
    Welcome {
        proto: u16,
        /// Host name, `""` when the server was started without one.
        #[serde(default)]
        host: String,
        /// `r<N>` of the build, `""` for an unstamped dev build.
        #[serde(default)]
        version: String,
        #[serde(default)]
        commit: String,
        #[serde(default)]
        players: u32,
        #[serde(default)]
        lobbies: u32,
    },
    /// A refused Hello, create, join, formation, start or move. The
    /// connection stays open; a refused move leaves the board as it was.
    Rejected { reason: String },
    Lobbies { lobbies: Vec<LobbyInfo> },
    /// Reply to a successful create or join; followed by `Roster`, `State`
    /// and `Phase` so the joiner sees the table immediately.
    Joined { lobby: String, id: u8 },
    /// The full roster with seats, on every join, leave, re-seat or creator
    /// handover. The client finds itself by `id`.
    Roster { creator: u8, roster: Vec<PlayerMeta> },
    /// The spec's start notification: to the creator only, whenever the
    /// lobby holds at least `MIN_PLAYERS` and on every roster change while
    /// it does.
    CanStart { players: u8 },
    /// `winner` is the seat that won; `None` outside Finished or for a draw.
    /// `end` says why a Finished game ended.
    Phase {
        phase: Phase,
        #[serde(default)]
        winner: Option<u8>,
        #[serde(default)]
        end: Option<EndReason>,
    },
    /// The full board, on every change and on join.
    State { board: BoardState },
    /// Once a second while Playing: whose turn, and how long is left.
    Clock { turn: u32, seat: u8, left_ms: u32 },
    Pong { nonce: u32 },
}
```

### 4.6 wasm API (`crates/kings/src/lib.rs`)

```rust
#[wasm_bindgen(start)] pub fn wasm_init() {}  // the engine installs the panic hook itself

#[wasm_bindgen] pub fn start_local()
    // Hotseat: four seats, one keyboard, default formation, 15 s turns,
    // camera turns to the seat to move.

#[wasm_bindgen] pub fn start_online(config_json: &str) -> Result<(), JsValue>
    // {"ws":"wss://…","handle":"…","lobby":"…","password":"","create":false}
    // Same shape and defaults as fire (handle -> "player", lobby -> "court").

#[wasm_bindgen] pub fn proto_version() -> u16
    // kings_core::proto::PROTO_VERSION; the page shows it beside the server's.

#[wasm_bindgen] pub fn state_json() -> String
    // Polled every animation frame. serde_json of HudState:
    // {
    //   "mode":"local"|"online", "connected":true, "screen":"browsing"|"lobby",
    //   "phase":"waiting"|"playing"|"finished", "winner":null|2, "end":null|"no_progress",
    //   "me":null|0, "creator":0, "is_creator":true, "can_start":false,
    //   "turn":12, "seat":1, "left_ms":8400, "my_turn":false,
    //   "quiet":3, "stalls":0,
    //   "roster":[{"id":0,"handle":"ada","seat":0}],
    //   "seats":[{"seat":0,"present":true,"alive":true,"garrison":false,"own_turns":3,"timeouts":0,"captured":["pawn","rook"]}, …4 entries],
    //   "pieces":[{"id":3,"owner":0,"kind":"joker","x":1,"y":1}, …],
    //   "joker_fl":[[2,2],null,[7,7],[2,7]],   // each seat's joker capture tile, null when off board or no joker
    //   "cursor":[2,3],
    //   "sel":null|[2,3],
    //   "targets":[{"x":2,"y":4,"k":"move"|"capture"|"teleport"|"place"|"swap"|"wake"}],
    //   "last":null|{"seat":0,"kind":"move","fx":3,"fy":0,"tx":3,"ty":1,"captured":null,"promoted":false,"eliminated":null},
    //   "pending":false, "notice":null|"not your turn"
    // }
    // Coordinates are ABSOLUTE board tiles; the page rotates for display only.

#[wasm_bindgen] pub fn click_tile(x: u8, y: u8)
    // Queues UiCmd::Click for the next update; same path as Enter on the
    // keyboard cursor. Runs the Selection machine: own piece -> select;
    // target -> C2S::Move stamped with the current turn (or a local apply in
    // hotseat); while Waiting, a second own piece of the same class ->
    // C2S::SetFormation with the two tiles swapped; anything else -> clear.

#[wasm_bindgen] pub fn start_game()      // queues UiCmd::Start -> C2S::Start (creator's button)
#[wasm_bindgen] pub fn clear_selection() // queues UiCmd::Clear (Esc)

// All entry points are thread-local queue/snapshot exchanges, fire's
// hud_json pattern in both directions: wasm is single-threaded and the
// engine loop runs on rAF after `ember_engine::run` returns, so JS calls
// between frames never race the game. There is no pass_turn: the rules
// have no voluntary pass.
```

### 4.7 Page contract (`web/games/kings/v1/index.html`)

The page is fire v2's skeleton and owns everything that needs text:

1. Server discovery. Fetches `server.json?ts=` cache-busted and takes `kings_ws` (and `v` for bundle cache-busting). When a `hosts[]` array exists (the `docs/hosts.md` convention, which lives on `feat/multi-host` and is not on this branch yet), it prefers a host whose `kings_proto == proto_version()` and has `kings_ws`, newest `version` first, falling back to the top-level key; no host means "no online server is published right now, practice mode still works".
2. Lobby browser on its own short-lived socket: `{"t":"hello","proto":proto_version(),"handle":"browser"}` then `{"t":"list_lobbies"}`; renders `lobbies` rows (name, lock, `players/cap`, host, playing flag with Join disabled while playing); Create with name and optional password. Join and Create call `start_online(JSON)`; Practice calls `start_local()`.
3. The 2D clickable board: a 10x10 CSS grid of buttons. The cell for absolute `(x,y)` is placed rotated by the local seat (`me`, or seat 0 when `me` is null in hotseat) so the local corner sits bottom-left; every click sends absolute `click_tile(x,y)`. Glyphs per kind (the six chess glyphs, J for the joker, H for the dormant hero, a marked H for HeroAwake), coloured per seat with the same four colours as the wasm scene; garrison pieces greyed; corner blocks tinted, cross neutral; `sel` outlined; `targets` coloured by `k`; `last` from and to marked; `cursor` shown faintly; each seat's `joker_fl` tile marked with a small corner dot in that seat's colour so the single capture tile is never a surprise.
4. HUD from `state_json()` each rAF: phase banner; "seat N (handle) to move" or "your turn"; the timer as seconds and a bar from `left_ms` (the wasm counts down between Clock messages, the page never extrapolates); four seat panels (handle, present, garrison, eliminated, own turn count with "joker placement on turn N" for the next multiple of five, timeouts as "n of 3", captured glyphs); the quiet counter as "no progress n of 100"; the last action as a sentence; the `notice` line (Rejected reasons); the winner banner on Finished with the `end` reason, or "draw"; Esc and click-elsewhere clear via `clear_selection()`.
5. The creator's Start button: visible when `is_creator && phase == "waiting"`, enabled when `can_start`; calls `start_game()`. Non-creators see "waiting for <creator> to start". While Waiting, a hint explains the card swap: click one of your legends or epics, then another of the same class.
6. Keyboard hygiene as fire: arrows and space prevented from scrolling; canvas focused on pointerdown; the wasm canvas is a 3D view only and has no picking, so the grid is the pointer input.

### 4.8 Hosting on this PC

The server runs inside the `claude-sdk` WSL distro; the host has no toolchains by policy. `bash deploy/deploy-kings-online.sh [up|down]` from Git Bash mirrors `deploy-fire-online.sh` step for step with `sdk(){ MSYS_NO_PATHCONV=1 WSL_UTF8=1 wsl -d claude-sdk --cd "$REPO_WIN" -- bash -lc "$1"; }` in place of ssh, `PORT=7782`, `BIND=127.0.0.1:7782`, logs at `$HOME/kings-server.log` and `$HOME/cloudflared-kings.log` inside the distro, and every step timed with `SECONDS` and printed:

1. Refuse a dirty tree (host git). It matters more here than for fire: the distro builds the working tree at `/mnt/c` directly, so this guard is the only thing making "deployed == HEAD" true. Compute `EMBER_BUILD_VERSION=r$(git rev-list --count HEAD)` and `EMBER_BUILD_COMMIT=$(git rev-parse --short HEAD)` on the host and pass them into the `bash -lc` string, so distro git never needs `safe.directory` for `/mnt/c`.
2. Preflight: `sdk 'command -v cargo && command -v cloudflared && command -v python3 && command -v ss'`, failing with a sentence naming what is missing.
3. Build: `sdk 'export CARGO_TARGET_DIR=$HOME/targets/ember; EMBER_BUILD_VERSION=… EMBER_BUILD_COMMIT=… chrt --idle 0 ionice -c3 cargo build --release -p kings-server --bin kings-server --example probe'` (distro-local target dir, idle priority; the probe example is built once so the probes below run the binary directly).
4. Stop the old pair inside the distro: `pkill -f "targets/ember/release/kings-serve[r]"` and `pkill -f "cloudflare[d] tunnel --url http://127.0.0.1:7782"` (anchored on the port so the fire and pong tunnels are never hit), then the port guard `ss -ltnp 'sport = :7782'` must be empty.
5. Launch detached from the Windows side: `pwsh -NoProfile -File deploy/wsl-detach.ps1 -Distro claude-sdk -Command 'RUST_LOG=info exec $HOME/targets/ember/release/kings-server 127.0.0.1:7782 --name "$EMBER_HOST_NAME" >> $HOME/kings-server.log 2>&1'`. The helper does `Start-Process -FilePath wsl.exe -ArgumentList @('-d',$Distro,'--','bash','-lc',$Command) -WindowStyle Hidden -PassThru` and prints the PID. The Linux process is the foreground child of a `wsl.exe` that outlives the deploy script; a `nohup … &` inside `wsl -- bash -lc` is reaped the moment that command returns (the claude-web lesson). `.ps1` is the only host scripting, so the helper is PowerShell, invoked from bash via `pwsh` with a fallback to `powershell.exe`.
6. `sleep 2; sdk 'pgrep -f kings-serve[r]'`, else tail the log and fail; print the server's first log line (it names the stamp or says "unstamped").
7. Loopback probe inside the distro: `sdk '$HOME/targets/ember/release/examples/probe ws://127.0.0.1:7782 --expect-commit <sha>'`; a failure here is the server, not the tunnel.
8. Tunnel: truncate the log, detach `cloudflared tunnel --url http://127.0.0.1:7782 --no-autoupdate >> $HOME/cloudflared-kings.log 2>&1` the same way; poll `grep -oE "https://[a-z0-9-]+\.trycloudflare\.com"` on the log every 2 s for up to 60 s; `WS_URL=wss://…`.
9. Public probe inside the distro, retried 10 times at 3 s.
10. Publish only after 9 passed: on the host `PAGES_DIR=$(mktemp -d -t ember-pages-XXXX); git worktree add -q "$PAGES_DIR" gh-pages`, then `sdk "python3 \"\$(wslpath -u '$REPO_WIN')/deploy/merge-server-json.py\" \"\$(wslpath -u '$WIN')/server.json\" kings_ws '$WS_URL'"` (merge, never overwrite: keeps `ws`, `fire_ws`, `proto`, `fire_proto`, `hosts`; sets `v`), commit and push on the host exactly as fire does ("Point kings_ws at …", Co-Authored-By trailer), `git worktree remove --force`.
11. Print `== ONLINE: $WS_URL ==` with per-step durations. `down` runs the two pkills of step 4.

The probe verifies, before anything is published: (a) through the public wss URL a `Hello { proto: PROTO_VERSION }` is answered with `Welcome` (only the hub thread can produce it, so a listener with a dead hub fails); (b) `Welcome.proto` equals the `PROTO_VERSION` of the build being deployed, else "ALIVE but speaks v…"; (c) with `--expect-commit <sha>`, `Welcome.commit` equals the stamp of the binary just built, which catches a missed pkill leaving last week's server on 7782; (d) the deep step, always on: `CreateLobby { "probe-<nonce>" }` then `Joined` then `LeaveLobby`, proving the version gate admits this build and the lobby path runs; (e) the same probe passed on loopback first so a public failure is attributable to the tunnel. Exit 0 or 1 with the reason on stderr and the round trip on stdout.

`deploy-pages.sh` gains `KINGS_LIVE="games/kings/v1"` (rm, mkdir, cp its `index.html` alongside arena, pong and fire; check `ARENA_LIVE` is v12 on this branch first), `cargo build --target wasm32-unknown-unknown --release -p kings --lib` plus `wasm-bindgen --target web --no-typescript --out-dir web/pkg …/kings.wasm`, `copy_pkg "$PAGES_DIR/$KINGS_LIVE/pkg" kings`, `KINGS_PROTO` grepped from `crates/kings-core/src/proto.rs` and passed as a fourth argument to the python stamp, which writes `kings_proto` and prints the "!! KINGS PROTOCOL BUMP" warning naming `deploy-kings-online.sh`, and an updated layout comment. In a separate commit that is not kings-specific, the script's cargo, wasm-bindgen and python invocations are routed through `wsl -d claude-sdk --cd "$(cygpath -w "$REPO_DIR")" -- bash -lc '…'` with `CARGO_TARGET_DIR=$HOME/targets/ember` and `chrt --idle 0 ionice -c3`, reading the `.wasm` from `${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/`; python becomes python3 in WSL against `/mnt/c` paths.

Catalogue: `web/games.json` gains `{"id":"kings","title":"Four Kings","tag":"online, turn based, 2-4 players, 15 s turns","desc":"…","versions":[{"v":"v1","path":"games/kings/v1/","live":true,"note":"first build: within-class card swap, 15 s turns"}]}`; `web/index.html` a third direct-link card `Four Kings` to `games/kings/v1/`; `server.json` keys `kings_ws` and `kings_proto` (the `<id>_ws` / `<id>_proto` convention), gaining the same two keys on a `hosts[]` entry when `publish-host.sh` lands. `README.md` gets a "Four Kings" section (rules pointer, run lines, own `PROTO_VERSION`, `kings_ws`/`kings_proto`, port 7782, the WSL deploy).

Caveats written into the script header and README: no systemd in `claude-sdk`, so nothing restarts the pair after a reboot, a `wsl --shutdown` (the documented fix for the HCS boot wedge) or a Windows sleep, and the watchdog is not ported (backlog); `MSYS_NO_PATHCONV=1` on every wsl call carrying a `/mnt/c` path; `WSL_UTF8=1` on anything whose output is grepped; the global CLAUDE.md toolchain list for `claude-sdk` must be updated to name rust nightly, the wasm32 target, wasm-bindgen-cli and cloudflared once they are confirmed present.

### 4.9 Test plan

Every test run reports its wall time; every commit says what was compiled and what was only read.

`kings-core`, board and frames: `setup_counts` (16 per seat, 64 total, block and cross occupancy); `setup_is_four_rotations` (for every index, seat s+1 = rot(seat s), seat 0 = rot(seat 3)); `setup_matches_the_tables` (the global table of section 1.3, all 64 tiles, pinned literally); `frames_round_trip` (`to_local(to_global(u,v)) == (u,v)` for all 100 tiles and 4 seats, and `to_global` of seat s+1 is `rot` of seat s); `frame_vectors_pinned` (the `f`, `l`, `f+l`, `f-l`, `-f+l` table of section 1.2, per seat); `front_left_on_board_from_every_home_tile` (all 16 tiles, 4 seats) and `front_left_on_board_from_every_start_mirror` (3 mirrors, 4 seats); `promotion_line_is_a_formula` (for every seat, the set of tiles where `t+f` or `t+l` is off board is exactly `u == 9 || v == 9`); `mirrors_are_involutions` (`mirror(mirror(t)) == t`, `mirror(t) != t`, all three mirrors, all tiles); `bishops_start_on_opposite_colours` (4 seats); `formation_validator` (default accepted; a Legend permutation accepted; `[B,R,B,R,N]`, `[R,B,R,B,N]`, `[B,R,R,N,B]` and `[R,B,N,B,R]` (the same-colour pairs) rejected, `[B,R,R,B,N]` accepted (opposite colours); wrong multisets rejected); `seat_by_join` ([0,2,1,3], two players diagonal); `to_state_from_state_round_trip` (bit-identical `State` through `BoardState`, with garrisons and captured lists).

`kings-core`, rules: per kind, every clause of the table in section 1.6 including blocking, own-piece exclusion and board edges; `pawn_two_axes_and_three_captures` per seat, pinned against `PAWN_CAPTURES`; `pawn_never_moves_backward`; `pawn_promotes_to_queen_on_either_far_edge` for all four seats, by move and by capture; `joker_step_only_onto_empty`; `joker_teleport_three_mirrors_empty_only`; `joker_capture_only_front_left_only_foreign` (own piece and empty tile refused, garrison piece captured); `joker_placement_on_own_turns_5_10_15` (a seat's `own_turns` reaches 5 on its fifth own turn regardless of other seats' timeouts and eliminations; placement legal exactly then; refused on turn 4 and 6; never targets its own tile; not banked); `joker_facing_is_the_owners_everywhere` (a seat-0 joker in seat 2's block captures at (+1,+1)); `hero_dormant_only_swaps` (targets are exactly the own pawns; cannot move or capture; can be captured); `hero_swap_consumes_the_turn_and_removes_the_pawn` (pawn not credited; `to_move` advances; kind is HeroAwake); `hero_wakes_in_place_only_with_no_pawns` (`from == to` accepted iff zero own pawns, rejected for every other kind); `hero_awake_is_rook_plus_knight`; `self_move_rejected_for_every_other_kind`; `turn_1_sweep` (exactly 17 legal (from, to) pairs per seat with four seats or garrisons: 2 knight, 8 pawn, 7 hero swap; the joker has 0); `one_capture_per_apply`; `king_capture_eliminates_and_removes_pieces` (seated), `garrison_king_capture_is_plain` (nothing else changes); `turn_increments_once_per_end_turn` and `own_turns_increments_on_own_turn_start_only`; `timeout_three_eliminates_and_resets_stalls` (the Judge scenario: A stalled, B AFK, ends with B eliminated and A the winner, never a draw); `forced_pass_is_not_a_timeout`; `full_round_of_stalls_ends_by_material`; `quiet_resets_on_capture_pawn_move_promotion_and_swap` and `no_progress_ends_at_100`; `turn_cap_at_600`; `material_ranking_pinned` (values, unique max wins, tie is a draw); `disconnect_of_the_mover_ends_the_turn`; `last_alive_wins`, `all_gone_is_abandoned`; `action_kind_derivation` (step vs mirror vs place precedence, the `x in {4,5}` row-mirror-as-step case); `illegal_reasons_are_readable`.

`kings-core`, proto: `the_wire_shape_is_the_house_style` (`"t"` tag, snake_case), `roundtrip_every_variant`, `defaulted_fields_decode_from_an_older_peer` (each defaulted field with its documented absent behaviour), `version_mismatch_is_rejectable`, `a_full_board_frame_stays_far_under_max_frame_bytes` (64 pieces, four captured lists), `sanitize_and_handle_limits`.

`kings-core`, clock: `tick_expires_at_turn_plus_grace` (not at 15 000, exactly at 15 300), `display_never_exceeds_turn_ms_or_goes_negative`.

`kings-server`: unit tests on `tick_lobby` with synthetic time (Clock every 1000 ms; a move at 15 100 ms elapsed applied; a move at 15 300 ms refused because the timeout already fired; three silent turns eliminate; Finished returns to Waiting after `RESULTS_SECS`); `SetFormation` accepted and rebroadcast in Waiting, refused in Playing, invalid refused with the reason; seats recomputed on leave; creator handover; `CanStart` sent at two and on roster changes; `tests/ws_e2e.rs` over raw tungstenite: create, list, join, guest `Start` refused, creator `Start` with two seats gives `Phase { Playing }` and a `State` with 64 pieces (two garrisons), a seat-0 pawn move `(3,0) to (4,0)` reaches seat 2 as `State` with `last.kind == move`, an out-of-turn move refused, a stale `turn` refused, with a test-only `turn_ms: 1000` a silent turn passes with `last.kind == timeout`; the probe's deep step against the test server.

`kings` client: `meshes` (ids match registration order, every mesh is a non-empty triangle list with unit normals); `ui::Selection` (select own, target emits a `Move` stamped with the current turn, foreign piece clears, Waiting-phase class swap emits `SetFormation` and refuses cross-class pairs, `pending` blocks a second click until the echo); `online::apply` against hand-built messages (Joined then Roster sets `me` by id, State replaces the board, Clock resyncs `left_ms`, Rejected surfaces `notice` and clears `pending`, Phase Finished stores winner and end, Roster handover updates `creator` and `is_creator`, `can_start` follows CanStart); `hotseat` (four seats, timeout pass on the client clock, camera target per seat); `tests/online_e2e.rs` with a real server, real `Net` and real `Online`: creator creates, guest joins, both see a 64-piece Waiting board, the guest's `Start` is `Rejected`, the creator's `Start` with two seats gives Playing on both, seat 0's pawn `(3,0) to (4,0)` agrees on both boards, seat 0 moving again gets "not your turn", an illegal target gets "cannot move there" with the board unchanged, with `turn_ms: 1000` the turn passes untouched, and a browser peer lists the lobby with `playing: true`.

## 5. Deliberately not built (backlog lines)

Card variants and cross-player trading (protocol bump); pawn capture-set and joker-step knobs to be settled by playtest; underpromotion; reconnection and spectators; AFK escalation beyond three timeouts; move animation keyed on piece ids; 3D picking; `net.rs` and `is_transient_read` duplication (lift to a shared crate); mesh primitives belong in `ember-engine`; `deploy-pages.sh` host-toolchain routing; a named tunnel; a Windows-side watchdog (no systemd in `claude-sdk`, `wsl --shutdown` kills the server); the hub lobby showcase stays arena-only; the `hosts[]` entry once `publish-host.sh` lands from `feat/multi-host`.
