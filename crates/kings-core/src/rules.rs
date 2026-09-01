//! The rules: legal targets, the turn loop, elimination and the end of the
//! game (sections 1.5 to 1.9 of the design).
//!
//! Every action is one `apply(state, from, to)`; it is legal iff `from`
//! holds a piece of the seat to move and `to` is in `targets(state, from)`.
//! The server validates with exactly this function and the client uses
//! `targets` for highlights, so the two can never disagree about a move.

use serde::{Deserialize, Serialize};

use crate::board::{
    ALL8, DIAG4, Dir, KNIGHT8, ORTHO4, Outcome, Piece, State, TILES, Tile, frame, front_left,
    mirrors, on_far_edge,
};
use crate::proto::{ActionKind, EndReason, Kind, LastAction};

/// Whether the joker has its one-tile non-capturing step (section 3: a
/// playtest knob; without it the joker is idle until its fifth turn).
pub const JOKER_STEP: bool = true;
/// The joker may be placed on any empty tile on its owner's own turns
/// `JOKER_PLACE_EVERY`, `2 * JOKER_PLACE_EVERY`, ...
pub const JOKER_PLACE_EVERY: u32 = 5;
/// The pawn's capture directions as `(a, b)` coefficients of the owner's
/// `(forward, left)` basis: `f + l`, `f - l`, `-f + l`. The documented
/// fallback if playtesting finds the third diagonal too strong is
/// `&[(1, 1), (1, -1)]`.
pub const PAWN_CAPTURES: &[(i8, i8)] = &[(1, 1), (1, -1), (-1, 1)];
/// Consecutive own-turn timeouts that eliminate a seat.
pub const TIMEOUTS_TO_ELIMINATE: u8 = 3;
/// Completed turns without progress after which the game ends by material.
pub const NO_PROGRESS_TURNS: u32 = 100;
/// The turn number at which the game ends by material regardless.
pub const MAX_TURNS: u32 = 600;
/// Material values for the ranking of section 1.8, pinned by a test.
pub const MATERIAL: [(Kind, u32); 9] = [
    (Kind::Queen, 9),
    (Kind::HeroAwake, 8),
    (Kind::Rook, 5),
    (Kind::Joker, 4),
    (Kind::Knight, 3),
    (Kind::Bishop, 3),
    (Kind::Hero, 3),
    (Kind::Pawn, 1),
    (Kind::King, 0),
];

/// The material value of a kind.
#[must_use]
pub const fn material(kind: Kind) -> u32 {
    match kind {
        Kind::Queen => 9,
        Kind::HeroAwake => 8,
        Kind::Rook => 5,
        Kind::Joker => 4,
        Kind::Knight | Kind::Bishop | Kind::Hero => 3,
        Kind::Pawn => 1,
        Kind::King => 0,
    }
}

/// What a legal target does, for highlights.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// Move onto an empty tile.
    Move,
    /// Take the foreign piece there.
    Capture,
    /// The joker's mirror-tile teleport.
    Teleport,
    /// The joker's fifth-turn placement.
    Place,
    /// The dormant hero's swap onto an own pawn.
    Swap,
    /// The dormant hero's awakening in place.
    Wake,
}

/// One legal destination of a piece.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    /// Column.
    pub x: u8,
    /// Row.
    pub y: u8,
    /// What moving there does.
    pub kind: TargetKind,
}

impl Target {
    /// The target's tile.
    #[must_use]
    pub const fn tile(self) -> Tile {
        Tile { x: self.x, y: self.y }
    }
}

/// A target list that keeps the first kind offered for each tile, so a tile
/// reachable two ways (a joker step that is also a row mirror) appears once.
struct Targets {
    list: Vec<Target>,
    seen: [bool; TILES],
}

impl Targets {
    const fn new() -> Self {
        Self {
            list: Vec::new(),
            seen: [false; TILES],
        }
    }

    fn push(&mut self, t: Tile, kind: TargetKind) {
        if !self.seen[t.index()] {
            self.seen[t.index()] = true;
            self.list.push(Target {
                x: t.x,
                y: t.y,
                kind,
            });
        }
    }
}

/// `steps(from, dirs)` that are empty or foreign.
fn steps(state: &State, from: Tile, owner: u8, dirs: &[Dir], out: &mut Targets) {
    for &d in dirs {
        let Some(t) = from.offset(d) else { continue };
        match state.piece(t) {
            None => out.push(t, TargetKind::Move),
            Some(p) if p.owner != owner => out.push(t, TargetKind::Capture),
            Some(_) => {}
        }
    }
}

/// `rays(from, dirs)`: along each direction, empty tiles and continuing, a
/// foreign tile and stopping, stopping before an own tile.
fn rays(state: &State, from: Tile, owner: u8, dirs: &[Dir], out: &mut Targets) {
    for &d in dirs {
        let mut t = from;
        while let Some(next) = t.offset(d) {
            match state.piece(next) {
                None => {
                    out.push(next, TargetKind::Move);
                    t = next;
                }
                Some(p) => {
                    if p.owner != owner {
                        out.push(next, TargetKind::Capture);
                    }
                    break;
                }
            }
        }
    }
}

fn pawn_targets(state: &State, from: Tile, owner: u8, out: &mut Targets) {
    let fr = frame(owner);
    for d in [fr.f, fr.l] {
        if let Some(t) = from.offset(d)
            && state.piece(t).is_none()
        {
            out.push(t, TargetKind::Move);
        }
    }
    for &(a, b) in PAWN_CAPTURES {
        if let Some(t) = from.offset(Dir::in_frame(a, b, fr))
            && state.piece(t).is_some_and(|p| p.owner != owner)
        {
            out.push(t, TargetKind::Capture);
        }
    }
}

/// Whether the joker of `owner` may be placed right now: the owner is to
/// move and this is one of its own turns 5, 10, 15, ...
fn joker_may_place(state: &State, owner: u8) -> bool {
    let own_turns = state.seats[usize::from(owner)].own_turns;
    state.to_move == owner && own_turns > 0 && own_turns.is_multiple_of(JOKER_PLACE_EVERY)
}

fn joker_targets(state: &State, from: Tile, owner: u8, out: &mut Targets) {
    // The capture first, so the front-left tile is narrated as a capture
    // even on a placement turn.
    if let Some(t) = from.offset(front_left(owner))
        && state.piece(t).is_some_and(|p| p.owner != owner)
    {
        out.push(t, TargetKind::Capture);
    }
    if JOKER_STEP {
        for d in ALL8 {
            if let Some(t) = from.offset(d)
                && state.piece(t).is_none()
            {
                out.push(t, TargetKind::Move);
            }
        }
    }
    for t in mirrors(from) {
        if state.piece(t).is_none() {
            out.push(t, TargetKind::Teleport);
        }
    }
    if joker_may_place(state, owner) {
        for t in Tile::all() {
            if t != from && state.piece(t).is_none() {
                out.push(t, TargetKind::Place);
            }
        }
    }
}

fn hero_targets(state: &State, from: Tile, owner: u8, out: &mut Targets) {
    let mut any = false;
    for (t, p) in state.pieces_of(owner) {
        if p.kind == Kind::Pawn {
            any = true;
            out.push(t, TargetKind::Swap);
        }
    }
    if !any {
        out.push(from, TargetKind::Wake);
    }
}

/// Every legal destination of the piece on `from` (the table of section
/// 1.6), or nothing for an empty tile. Independent of whose turn it is,
/// except that the joker's placement is offered only to the seat to move,
/// so highlights for the other seats never show a placement they could not
/// make.
#[must_use]
pub fn targets(state: &State, from: Tile) -> Vec<Target> {
    let Some(piece) = state.piece(from) else {
        return Vec::new();
    };
    let owner = piece.owner;
    let mut out = Targets::new();
    match piece.kind {
        Kind::King => steps(state, from, owner, &ALL8, &mut out),
        Kind::Queen => rays(state, from, owner, &ALL8, &mut out),
        Kind::Rook => rays(state, from, owner, &ORTHO4, &mut out),
        Kind::Bishop => rays(state, from, owner, &DIAG4, &mut out),
        Kind::Knight => steps(state, from, owner, &KNIGHT8, &mut out),
        Kind::Pawn => pawn_targets(state, from, owner, &mut out),
        Kind::Joker => joker_targets(state, from, owner, &mut out),
        Kind::Hero => hero_targets(state, from, owner, &mut out),
        Kind::HeroAwake => {
            rays(state, from, owner, &ORTHO4, &mut out);
            steps(state, from, owner, &KNIGHT8, &mut out);
        }
    }
    out.list
}

/// Whether a seat has any legal action at all (the forced-pass test).
#[must_use]
pub fn has_any_move(state: &State, seat: u8) -> bool {
    state
        .pieces_of(seat)
        .any(|(t, _)| !targets(state, t).is_empty())
}

/// Why a move was refused; `reason()` is what the player reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Illegal {
    /// The game has a result; nothing is applied any more.
    GameOver,
    /// A coordinate above 9.
    OffBoard,
    /// `from` is empty.
    NoPiece,
    /// The piece on `from` belongs to another seat.
    NotYours,
    /// `from == to` for anything but a dormant hero.
    SelfMove,
    /// `to` holds an own piece (and this is not the hero swap).
    OwnPiece,
    /// A dormant hero asked to do anything but the swap or the wake.
    DormantHero,
    /// `to` is not in the piece's targets, for any other reason.
    NotATarget,
}

impl Illegal {
    /// The text a player reads in `Rejected`. Every refusal of a target
    /// begins with "cannot move there".
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::GameOver => "the game is over",
            Self::OffBoard => "that tile is off the board",
            Self::NoPiece => "there is no piece on that tile",
            Self::NotYours => "that is not your piece",
            Self::SelfMove => "cannot move there: a piece cannot move onto its own tile",
            Self::OwnPiece => "cannot move there: that tile holds your own piece",
            Self::DormantHero => {
                "cannot move there: a dormant hero can only swap places with one of your pawns, or awaken in place once you have none"
            }
            Self::NotATarget => "cannot move there",
        }
    }
}

impl std::fmt::Display for Illegal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

impl std::error::Error for Illegal {}

/// The mover, if `from -> to` is legal for the seat to move, else why not.
fn check_legal(state: &State, from: Tile, to: Tile) -> Result<Piece, Illegal> {
    if state.result.is_some() {
        return Err(Illegal::GameOver);
    }
    let piece = state.piece(from).ok_or(Illegal::NoPiece)?;
    if piece.owner != state.to_move {
        return Err(Illegal::NotYours);
    }
    if targets(state, from).iter().any(|t| t.tile() == to) {
        return Ok(piece);
    }
    if piece.kind == Kind::Hero {
        return Err(Illegal::DormantHero);
    }
    if from == to {
        return Err(Illegal::SelfMove);
    }
    if state.piece(to).is_some_and(|p| p.owner == piece.owner) {
        return Err(Illegal::OwnPiece);
    }
    Err(Illegal::NotATarget)
}

/// The narration kind of an applied move (section 1.6, "Narration").
#[must_use]
pub fn action_kind_of(mover: Kind, from: Tile, to: Tile) -> ActionKind {
    match mover {
        Kind::Hero if from == to => ActionKind::HeroWake,
        Kind::Hero => ActionKind::HeroSwap,
        Kind::Joker => {
            let dx = i16::from(from.x) - i16::from(to.x);
            let dy = i16::from(from.y) - i16::from(to.y);
            if dx.abs() <= 1 && dy.abs() <= 1 {
                ActionKind::Move
            } else if mirrors(from).contains(&to) {
                ActionKind::JokerTeleport
            } else {
                ActionKind::JokerPlace
            }
        }
        _ => ActionKind::Move,
    }
}

const fn pass(seat: u8, kind: ActionKind, eliminated: Option<u8>) -> LastAction {
    LastAction {
        seat,
        kind,
        fx: 0,
        fy: 0,
        tx: 0,
        ty: 0,
        captured: None,
        promoted: false,
        eliminated,
    }
}

/// Apply one action for the seat to move (section 1.9), then end the turn.
///
/// The returned action is what was applied; `state.last` may already be a
/// forced pass of the next seat when `end_turn` had to skip it.
///
/// # Errors
/// The move is not legal; the state is untouched.
pub fn apply(state: &mut State, from: Tile, to: Tile) -> Result<LastAction, Illegal> {
    let piece = check_legal(state, from, to)?;
    let owner = piece.owner;
    let victim = state.piece(to);
    let progress = victim.is_some() || matches!(piece.kind, Kind::Pawn | Kind::Hero);

    let mut moved = piece;
    let captured = victim.filter(|v| v.owner != owner).map(|v| v.kind);
    if let Some(kind) = captured {
        state.seats[usize::from(owner)].captured.push(kind);
    }
    let promoted = piece.kind == Kind::Pawn && on_far_edge(owner, to);
    if promoted {
        moved.kind = Kind::Queen;
    }
    if piece.kind == Kind::Hero {
        moved.kind = Kind::HeroAwake;
    }
    state.set(from, None);
    state.set(to, Some(moved));

    let mut eliminated = None;
    if let Some(v) = victim
        && v.kind == Kind::King
        && v.owner != owner
        && state.seats[usize::from(v.owner)].alive
    {
        eliminate(state, v.owner);
        eliminated = Some(v.owner);
    }

    state.seats[usize::from(owner)].timeouts = 0;
    state.stalls = 0;
    state.quiet = if progress { 0 } else { state.quiet + 1 };
    let last = LastAction {
        seat: owner,
        kind: action_kind_of(piece.kind, from, to),
        fx: from.x,
        fy: from.y,
        tx: to.x,
        ty: to.y,
        captured,
        promoted,
        eliminated,
    };
    state.last = Some(last);
    end_turn(state);
    Ok(last)
}

/// `apply` from wire coordinates, refusing anything off the board.
///
/// # Errors
/// As `apply`, plus `Illegal::OffBoard`.
pub fn apply_xy(state: &mut State, fx: u8, fy: u8, tx: u8, ty: u8) -> Result<LastAction, Illegal> {
    let from = Tile::new(fx, fy).ok_or(Illegal::OffBoard)?;
    let to = Tile::new(tx, ty).ok_or(Illegal::OffBoard)?;
    apply(state, from, to)
}

/// A seat is out: it takes no more turns, and unless it is a garrison every
/// piece it owns leaves the board (section 1.7).
pub fn eliminate(state: &mut State, seat: u8) {
    state.seats[usize::from(seat)].alive = false;
    if !state.seats[usize::from(seat)].garrison {
        state.remove_pieces_of(seat);
    }
}

/// The material sum of a seat's pieces on the board.
#[must_use]
pub fn material_of(state: &State, seat: u8) -> u32 {
    state.pieces_of(seat).map(|(_, p)| material(p.kind)).sum()
}

/// End the game by material ranking among the alive seats: the strictly
/// greatest sum wins, a shared maximum is a draw.
pub fn finish_by_material(state: &mut State, end: EndReason) {
    let scores: Vec<(u8, u32)> = (0..state.seats.len())
        .filter(|&s| state.seats[s].alive)
        .map(|s| {
            let seat = u8::try_from(s).unwrap_or(u8::MAX);
            (seat, material_of(state, seat))
        })
        .collect();
    let best = scores.iter().map(|&(_, m)| m).max().unwrap_or(0);
    let mut leaders = scores.iter().filter(|&&(_, m)| m == best);
    let winner = match (leaders.next(), leaders.next()) {
        (Some(&(seat, _)), None) => Some(seat),
        _ => None,
    };
    state.result = Some(Outcome { winner, end });
}

/// Steps (1) to (3) of `end_turn`: whether the game is over now, setting
/// the result if it is. Also the check a non-mover's disconnect runs.
pub fn check_end(state: &mut State) -> bool {
    if state.result.is_some() {
        return true;
    }
    match state.alive_count() {
        0 => {
            state.result = Some(Outcome {
                winner: None,
                end: EndReason::Abandoned,
            });
        }
        1 => {
            state.result = Some(Outcome {
                winner: state.sole_survivor(),
                end: EndReason::LastKing,
            });
        }
        _ if state.quiet >= NO_PROGRESS_TURNS => finish_by_material(state, EndReason::NoProgress),
        _ if state.turn >= MAX_TURNS => finish_by_material(state, EndReason::TurnCap),
        _ => return false,
    }
    true
}

/// The turn loop of section 1.9: settle the game if it is over, otherwise
/// hand the turn to the next alive seat, passing for it (and possibly
/// ending by stalemate) if it has no legal move.
pub fn end_turn(state: &mut State) {
    loop {
        if check_end(state) {
            return;
        }
        state.turn += 1;
        let Some(next) = state.next_alive_after(state.to_move) else {
            return;
        };
        state.to_move = next;
        state.seats[usize::from(next)].own_turns += 1;
        state.clock.reset();
        if has_any_move(state, next) {
            return;
        }
        state.stalls = state.stalls.saturating_add(1);
        state.quiet += 1;
        state.last = Some(pass(next, ActionKind::Pass, None));
        if state.stalls >= state.alive_count() {
            finish_by_material(state, EndReason::Stalemate);
            return;
        }
    }
}

/// The seat to move ran out of time (section 1.7): a quiet pass that costs
/// a timeout mark, the third of which eliminates. Returns the recorded
/// action, or `None` when the game is already over.
pub fn timeout(state: &mut State) -> Option<LastAction> {
    if state.result.is_some() {
        return None;
    }
    let seat = state.to_move;
    let timeouts = &mut state.seats[usize::from(seat)].timeouts;
    *timeouts = timeouts.saturating_add(1);
    let eliminated = if *timeouts >= TIMEOUTS_TO_ELIMINATE {
        eliminate(state, seat);
        Some(seat)
    } else {
        None
    };
    state.quiet += 1;
    state.stalls = 0;
    let last = pass(seat, ActionKind::Timeout, eliminated);
    state.last = Some(last);
    end_turn(state);
    Some(last)
}

/// A seat's connection dropped mid-game (section 1.7): the seat is
/// eliminated at once. If it was the seat to move its turn ends with a
/// pass and the action is returned; otherwise the seat is simply skipped
/// from now on, the end-of-game check runs, and `None` is returned.
pub fn disconnect(state: &mut State, seat: u8) -> Option<LastAction> {
    state.seats[usize::from(seat)].present = false;
    if state.result.is_some() {
        return None;
    }
    if state.seats[usize::from(seat)].alive {
        eliminate(state, seat);
    }
    if seat == state.to_move {
        let last = pass(seat, ActionKind::Pass, Some(seat));
        state.last = Some(last);
        end_turn(state);
        Some(last)
    } else {
        check_end(state);
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::board::{PIECES_PER_SEAT, SEAT_BY_JOIN, setup, to_global, to_local};
    use crate::proto::Formation;

    fn full() -> State {
        setup([true; 4], [Formation::DEFAULT; 4])
    }

    /// The seats of `present`, an empty board, seat 0 (or the first present
    /// seat) to move.
    fn blank(present: [bool; 4]) -> State {
        let mut st = setup(present, [Formation::DEFAULT; 4]);
        st.board = [None; TILES];
        st
    }

    /// Seats 0 and 2 alive with kings in their corners, seats 1 and 3
    /// garrisons with no pieces.
    fn two() -> State {
        let mut st = blank([true, false, true, false]);
        put(&mut st, 0, Kind::King, 0, 0);
        put(&mut st, 2, Kind::King, 9, 9);
        st
    }

    fn put(st: &mut State, seat: u8, kind: Kind, x: u8, y: u8) -> Tile {
        let n = u8::try_from(st.pieces_of(seat).count()).unwrap();
        let t = Tile::at(x, y);
        st.set(
            t,
            Some(Piece {
                id: seat * PIECES_PER_SEAT + n,
                owner: seat,
                kind,
            }),
        );
        t
    }

    fn tiles(ts: &[Target]) -> BTreeSet<(u8, u8)> {
        ts.iter().map(|t| (t.x, t.y)).collect()
    }

    fn has(ts: &[Target], x: u8, y: u8, kind: TargetKind) -> bool {
        ts.iter().any(|t| t.x == x && t.y == y && t.kind == kind)
    }

    fn mv(st: &mut State, from: (u8, u8), to: (u8, u8)) -> Result<LastAction, Illegal> {
        apply(st, Tile::at(from.0, from.1), Tile::at(to.0, to.1))
    }

    fn count(st: &State, seat: u8) -> usize {
        st.pieces_of(seat).count()
    }

    #[test]
    fn king_steps_empty_or_foreign() {
        let mut st = two();
        let king = put(&mut st, 0, Kind::King, 4, 4);
        assert_eq!(targets(&st, king).len(), 8);
        put(&mut st, 0, Kind::Pawn, 5, 5);
        put(&mut st, 2, Kind::Pawn, 3, 3);
        let ts = targets(&st, king);
        assert_eq!(ts.len(), 7);
        assert!(!has(&ts, 5, 5, TargetKind::Move));
        assert!(has(&ts, 3, 3, TargetKind::Capture));
        assert!(has(&ts, 4, 5, TargetKind::Move));
        // The corner king of `two()` has three neighbours.
        assert_eq!(targets(&st, Tile::at(0, 0)).len(), 3);
        // No check: the king may step onto an attacked tile.
        put(&mut st, 2, Kind::Rook, 4, 9);
        assert!(has(&targets(&st, king), 4, 5, TargetKind::Move));
    }

    #[test]
    fn queen_rook_bishop_rays_stop_at_the_first_piece() {
        let mut st = two();
        let queen = put(&mut st, 0, Kind::Queen, 4, 4);
        // 9 + 9 on the axes, 9 + 8 on the diagonals, minus the own king at
        // (0,0) on the main diagonal.
        assert_eq!(targets(&st, queen).len(), 34);
        put(&mut st, 0, Kind::Pawn, 4, 7);
        put(&mut st, 2, Kind::Pawn, 7, 4);
        put(&mut st, 2, Kind::Pawn, 8, 4);
        let ts = targets(&st, queen);
        assert!(has(&ts, 4, 6, TargetKind::Move));
        assert!(!has(&ts, 4, 7, TargetKind::Move) && !has(&ts, 4, 7, TargetKind::Capture));
        assert!(!tiles(&ts).contains(&(4, 8)), "blocked behind an own piece");
        assert!(has(&ts, 7, 4, TargetKind::Capture));
        assert!(!tiles(&ts).contains(&(8, 4)), "blocked behind a foreign piece");
        assert!(!tiles(&ts).contains(&(0, 0)), "own king excluded");
        assert!(has(&ts, 9, 9, TargetKind::Capture), "the foreign king is a capture");

        let rook = put(&mut st, 0, Kind::Rook, 2, 6);
        let ts = targets(&st, rook);
        assert_eq!(tiles(&ts).len(), 9 + 9, "an empty row and column");
        assert!(ts.iter().all(|t| t.x == 2 || t.y == 6));
        let bishop = put(&mut st, 0, Kind::Bishop, 1, 6);
        let ts = targets(&st, bishop);
        assert!(ts.iter().all(|t| Tile::at(t.x, t.y).colour() == Tile::at(1, 6).colour()));
        assert!(has(&ts, 0, 7, TargetKind::Move));
        assert!(has(&ts, 0, 5, TargetKind::Move));
        assert!(has(&ts, 4, 9, TargetKind::Move));
        assert!(has(&ts, 7, 0, TargetKind::Move));
        assert!(!tiles(&ts).contains(&(2, 6)), "the own rook is not a target");
        assert_eq!(tiles(&ts).len(), 3 + 1 + 1 + 6);
    }

    #[test]
    fn knight_jumps_and_excludes_own() {
        let mut st = two();
        let knight = put(&mut st, 0, Kind::Knight, 4, 4);
        // Surround it: jumping over pieces is fine.
        for d in ALL8 {
            let t = Tile::at(4, 4).offset(d).unwrap();
            put(&mut st, 2, Kind::Pawn, t.x, t.y);
        }
        assert_eq!(targets(&st, knight).len(), 8);
        put(&mut st, 0, Kind::Pawn, 6, 5);
        put(&mut st, 2, Kind::Pawn, 2, 3);
        let ts = targets(&st, knight);
        assert_eq!(ts.len(), 7);
        assert!(has(&ts, 2, 3, TargetKind::Capture));
        assert!(!tiles(&ts).contains(&(6, 5)));
        let corner = put(&mut st, 0, Kind::Knight, 9, 0);
        assert_eq!(tiles(&targets(&st, corner)), [(7, 1), (8, 2)].into_iter().collect());
    }

    #[test]
    fn pawn_two_axes_and_three_captures() {
        assert_eq!(PAWN_CAPTURES, &[(1, 1), (1, -1), (-1, 1)]);
        for seat in 0..4u8 {
            let mut st = blank([true; 4]);
            let fr = frame(seat);
            let from = to_global(seat, 4, 4);
            put(&mut st, seat, Kind::Pawn, from.x, from.y);
            let ts = targets(&st, from);
            let fwd = from.offset(fr.f).unwrap();
            let lft = from.offset(fr.l).unwrap();
            assert_eq!(tiles(&ts), [(fwd.x, fwd.y), (lft.x, lft.y)].into_iter().collect());
            assert!(ts.iter().all(|t| t.kind == TargetKind::Move));
            assert_eq!(to_local(seat, fwd), (5, 4), "seat {seat} forward is +u");
            assert_eq!(to_local(seat, lft), (4, 5), "seat {seat} left is +v");

            // Foreign pieces on all eight neighbours: only the three
            // PAWN_CAPTURES diagonals are captures, and both moves are gone.
            let foreign = (seat + 1) % 4;
            for d in ALL8 {
                let t = from.offset(d).unwrap();
                put(&mut st, foreign, Kind::Pawn, t.x, t.y);
            }
            let ts = targets(&st, from);
            let expected: BTreeSet<(u8, u8)> = PAWN_CAPTURES
                .iter()
                .map(|&(a, b)| from.offset(Dir::in_frame(a, b, fr)).unwrap())
                .map(|t| (t.x, t.y))
                .collect();
            assert_eq!(tiles(&ts), expected, "seat {seat}");
            assert!(ts.iter().all(|t| t.kind == TargetKind::Capture));
            let locals: BTreeSet<(u8, u8)> = ts.iter().map(|t| to_local(seat, t.tile())).collect();
            assert_eq!(locals, [(5, 5), (5, 3), (3, 5)].into_iter().collect());

            // An own piece on the forward tile blocks the move without
            // becoming a capture.
            let mut st = blank([true; 4]);
            put(&mut st, seat, Kind::Pawn, from.x, from.y);
            put(&mut st, seat, Kind::Rook, fwd.x, fwd.y);
            assert_eq!(tiles(&targets(&st, from)), [(lft.x, lft.y)].into_iter().collect());
        }
    }

    #[test]
    fn pawn_never_moves_backward() {
        for seat in 0..4u8 {
            let mut st = blank([true; 4]);
            let fr = frame(seat);
            let from = to_global(seat, 4, 4);
            put(&mut st, seat, Kind::Pawn, from.x, from.y);
            let foreign = (seat + 1) % 4;
            for d in [fr.f.neg(), fr.l.neg(), fr.f.neg().sub(fr.l)] {
                let t = from.offset(d).unwrap();
                put(&mut st, foreign, Kind::Pawn, t.x, t.y);
            }
            let ts = targets(&st, from);
            assert_eq!(ts.len(), 2, "seat {seat}: only the two forward moves");
            assert!(ts.iter().all(|t| t.kind == TargetKind::Move));
            for t in &ts {
                let (u, v) = to_local(seat, t.tile());
                assert!(u + v == 9 && u >= 4 && v >= 4, "seat {seat} {t:?}");
            }
        }
    }

    #[test]
    fn pawn_promotes_to_queen_on_either_far_edge() {
        for seat in 0..4u8 {
            let foreign = (seat + 2) % 4;
            for (start, to, capture) in [
                ((8, 2), (9, 2), false),
                ((2, 8), (2, 9), false),
                ((8, 3), (9, 4), true),
                ((3, 8), (4, 9), true),
            ] {
                let mut st = blank([true; 4]);
                st.to_move = seat;
                for s in 0..4u8 {
                    // A king each, far from the action, so nobody stalls.
                    let corner = to_global(s, 0, 0);
                    put(&mut st, s, Kind::King, corner.x, corner.y);
                }
                let from = to_global(seat, start.0, start.1);
                let dest = to_global(seat, to.0, to.1);
                put(&mut st, seat, Kind::Pawn, from.x, from.y);
                if capture {
                    put(&mut st, foreign, Kind::Rook, dest.x, dest.y);
                }
                let last = apply(&mut st, from, dest).unwrap_or_else(|e| panic!("seat {seat} {start:?}->{to:?}: {e}"));
                assert!(last.promoted, "seat {seat} {start:?}->{to:?}");
                assert_eq!(last.captured, capture.then_some(Kind::Rook));
                assert_eq!(st.piece(dest).unwrap().kind, Kind::Queen);
                assert_eq!(st.piece(dest).unwrap().owner, seat);
                assert_eq!(st.quiet, 0);
            }
            // Not on an inner tile.
            let mut st = blank([true; 4]);
            st.to_move = seat;
            for s in 0..4u8 {
                let corner = to_global(s, 0, 0);
                put(&mut st, s, Kind::King, corner.x, corner.y);
            }
            let from = to_global(seat, 7, 8);
            let dest = to_global(seat, 8, 8);
            put(&mut st, seat, Kind::Pawn, from.x, from.y);
            let last = apply(&mut st, from, dest).unwrap();
            assert!(!last.promoted);
            assert_eq!(st.piece(dest).unwrap().kind, Kind::Pawn);
        }
    }

    #[test]
    fn joker_step_only_onto_empty() {
        let mut st = two();
        let joker = put(&mut st, 0, Kind::Joker, 4, 4);
        put(&mut st, 2, Kind::Pawn, 5, 4);
        put(&mut st, 0, Kind::Pawn, 4, 5);
        let ts = targets(&st, joker);
        assert!(!tiles(&ts).contains(&(5, 4)), "a foreign piece is not stepped on");
        assert!(!tiles(&ts).contains(&(4, 5)), "an own piece is not stepped on");
        assert!(has(&ts, 3, 3, TargetKind::Move));
        assert!(has(&ts, 5, 5, TargetKind::Move), "empty front-left is a step");
        assert_eq!(ts.iter().filter(|t| t.kind == TargetKind::Move).count(), 6);
        let last = mv(&mut st, (4, 4), (3, 3)).unwrap();
        assert_eq!(last.kind, ActionKind::Move);
        assert_eq!(last.captured, None);
    }

    #[test]
    fn joker_teleport_three_mirrors_empty_only() {
        let mut st = two();
        let joker = put(&mut st, 0, Kind::Joker, 2, 3);
        // Pieces in between are irrelevant.
        put(&mut st, 2, Kind::Rook, 5, 3);
        put(&mut st, 0, Kind::Rook, 2, 5);
        let ts = targets(&st, joker);
        for (x, y) in [(7, 3), (2, 6), (7, 6)] {
            assert!(has(&ts, x, y, TargetKind::Teleport), "({x},{y})");
        }
        put(&mut st, 2, Kind::Pawn, 7, 3);
        put(&mut st, 0, Kind::Pawn, 2, 6);
        let ts = targets(&st, joker);
        assert!(!tiles(&ts).contains(&(7, 3)), "a foreign piece blocks, never a capture");
        assert!(!tiles(&ts).contains(&(2, 6)), "an own piece blocks");
        assert!(has(&ts, 7, 6, TargetKind::Teleport));
        let last = mv(&mut st, (2, 3), (7, 6)).unwrap();
        assert_eq!(last.kind, ActionKind::JokerTeleport);
        assert_eq!(st.piece(Tile::at(7, 6)).unwrap().kind, Kind::Joker);
        assert_eq!(st.piece(Tile::at(5, 3)).unwrap().kind, Kind::Rook, "untouched");
    }

    #[test]
    fn joker_capture_only_front_left_only_foreign() {
        let mut st = two();
        let joker = put(&mut st, 0, Kind::Joker, 4, 4);
        // Foreign pieces on every neighbour but the front-left: no capture.
        for d in ALL8 {
            if d != front_left(0) {
                let t = Tile::at(4, 4).offset(d).unwrap();
                put(&mut st, 2, Kind::Pawn, t.x, t.y);
            }
        }
        let ts = targets(&st, joker);
        assert!(ts.iter().all(|t| t.kind != TargetKind::Capture));
        assert!(has(&ts, 5, 5, TargetKind::Move), "empty front-left is a step");
        // An own piece there: nothing.
        put(&mut st, 0, Kind::Pawn, 5, 5);
        let ts = targets(&st, joker);
        assert!(!tiles(&ts).contains(&(5, 5)));
        assert_eq!(mv(&mut st, (4, 4), (5, 5)), Err(Illegal::OwnPiece));
        // A garrison piece there: captured.
        st.set(Tile::at(5, 5), None);
        put(&mut st, 1, Kind::Rook, 5, 5);
        let ts = targets(&st, joker);
        assert!(has(&ts, 5, 5, TargetKind::Capture));
        assert_eq!(ts.iter().filter(|t| t.kind == TargetKind::Capture).count(), 1);
        let last = mv(&mut st, (4, 4), (5, 5)).unwrap();
        assert_eq!(last.kind, ActionKind::Move);
        assert_eq!(last.captured, Some(Kind::Rook));
        assert_eq!(st.seats[0].captured, vec![Kind::Rook]);
    }

    /// Knight out on odd rounds, back on even, for each seat.
    fn knight_hop(seat: u8, round: u32) -> ((u8, u8), (u8, u8)) {
        let (home, out) = [((1, 2), (2, 4)), ((7, 1), (5, 2)), ((8, 7), (7, 5)), ((2, 8), (4, 7))][seat as usize];
        if round.is_multiple_of(2) { (out, home) } else { (home, out) }
    }

    #[test]
    fn joker_placement_on_own_turns_5_10_15() {
        assert_eq!(JOKER_PLACE_EVERY, 5);
        let mut st = full();
        let joker = Tile::at(1, 1);
        let places = |st: &State| {
            targets(st, joker)
                .into_iter()
                .filter(|t| t.kind == TargetKind::Place)
                .count()
        };
        let mut banked_probe = None;
        for round in 1..=10u32 {
            // Seat 0's own turn: its own_turns equals the round.
            assert_eq!(st.to_move, 0, "round {round}");
            assert_eq!(st.seats[0].own_turns, round, "round {round}");
            let expected = if round.is_multiple_of(5) { 36 } else { 0 };
            assert_eq!(places(&st), expected, "round {round}: placements");
            if round == 4 || round == 6 {
                assert_eq!(mv(&mut st, (1, 1), (4, 4)), Err(Illegal::NotATarget));
            }
            if round == 5 {
                assert!(targets(&st, joker).iter().all(|t| t.tile() != joker), "never its own tile");
                // A placement is legal now (on a clone) and narrated as one.
                let mut placed = st.clone();
                let last = mv(&mut placed, (1, 1), (4, 4)).unwrap();
                assert_eq!(last.kind, ActionKind::JokerPlace);
                assert_eq!(placed.piece(Tile::at(4, 4)).unwrap().kind, Kind::Joker);
                banked_probe = Some(placed);
            }
            let (from, to) = knight_hop(0, round);
            mv(&mut st, from, to).unwrap();
            // The other seats: seat 1 times out in round 2, seat 3 leaves in
            // round 3; neither changes seat 0's own-turn count.
            for seat in 1..4u8 {
                if !st.seats[seat as usize].alive {
                    continue;
                }
                assert_eq!(st.to_move, seat, "round {round}");
                if seat == 1 && round == 2 {
                    timeout(&mut st).unwrap();
                } else if seat == 3 && round == 3 {
                    disconnect(&mut st, 3).unwrap();
                } else {
                    let (from, to) = knight_hop(seat, round);
                    mv(&mut st, from, to).unwrap();
                }
            }
        }
        assert_eq!(st.seats[1].own_turns, 10);
        assert_eq!(st.seats[3].own_turns, 3);
        assert!(!st.seats[3].alive);
        assert_eq!(st.result, None);
        // Not banked: the placement skipped in round 5 was not offered in
        // round 6 (asserted in the loop), and the seat that DID place has
        // no placement on its sixth turn either.
        let mut placed = banked_probe.unwrap();
        for seat in 1..4u8 {
            if st.seats[seat as usize].alive && placed.to_move == seat {
                let (from, to) = knight_hop(seat, 5);
                mv(&mut placed, from, to).unwrap();
            }
        }
        assert_eq!(placed.to_move, 0);
        assert_eq!(placed.seats[0].own_turns, 6);
        assert!(targets(&placed, Tile::at(4, 4)).iter().all(|t| t.kind != TargetKind::Place));
    }

    #[test]
    fn placement_is_only_offered_to_the_seat_to_move() {
        let mut st = full();
        st.seats[2].own_turns = 5;
        assert_eq!(st.to_move, 0);
        assert!(targets(&st, Tile::at(8, 8)).iter().all(|t| t.kind != TargetKind::Place));
        st.to_move = 2;
        assert!(targets(&st, Tile::at(8, 8)).iter().any(|t| t.kind == TargetKind::Place));
        // own_turns of 0 is never a placement turn.
        st.seats[2].own_turns = 0;
        assert!(targets(&st, Tile::at(8, 8)).iter().all(|t| t.kind != TargetKind::Place));
    }

    #[test]
    fn joker_facing_is_the_owners_everywhere() {
        let mut st = two();
        // A seat-0 joker in seat 2's block still captures at (+1,+1).
        let joker = put(&mut st, 0, Kind::Joker, 7, 7);
        put(&mut st, 2, Kind::Pawn, 8, 8);
        put(&mut st, 2, Kind::Pawn, 6, 6);
        let ts = targets(&st, joker);
        assert!(has(&ts, 8, 8, TargetKind::Capture));
        assert!(!tiles(&ts).contains(&(6, 6)));
        // A seat-2 joker in seat 0's block captures at (-1,-1).
        let mut st = two();
        let joker = put(&mut st, 2, Kind::Joker, 2, 2);
        put(&mut st, 0, Kind::Pawn, 1, 1);
        put(&mut st, 0, Kind::Pawn, 3, 3);
        let ts = targets(&st, joker);
        assert!(has(&ts, 1, 1, TargetKind::Capture));
        assert!(!tiles(&ts).contains(&(3, 3)));
        // On a far edge there is no capture at all.
        let mut st = two();
        let joker = put(&mut st, 0, Kind::Joker, 9, 3);
        put(&mut st, 2, Kind::Pawn, 8, 4);
        put(&mut st, 2, Kind::Pawn, 8, 2);
        assert!(targets(&st, joker).iter().all(|t| t.kind != TargetKind::Capture));
        assert!(Tile::at(9, 3).offset(front_left(0)).is_none());
    }

    #[test]
    fn hero_dormant_only_swaps() {
        let mut st = two();
        let hero = put(&mut st, 0, Kind::Hero, 4, 4);
        put(&mut st, 0, Kind::Pawn, 1, 1);
        put(&mut st, 0, Kind::Pawn, 8, 8);
        put(&mut st, 0, Kind::Rook, 6, 6);
        put(&mut st, 2, Kind::Pawn, 4, 5);
        let ts = targets(&st, hero);
        assert_eq!(tiles(&ts), [(1, 1), (8, 8)].into_iter().collect());
        assert!(ts.iter().all(|t| t.kind == TargetKind::Swap));
        assert_eq!(mv(&mut st, (4, 4), (4, 5)), Err(Illegal::DormantHero));
        assert_eq!(mv(&mut st, (4, 4), (3, 4)), Err(Illegal::DormantHero));
        assert_eq!(mv(&mut st, (4, 4), (6, 6)), Err(Illegal::DormantHero));
        assert_eq!(mv(&mut st, (4, 4), (4, 4)), Err(Illegal::DormantHero));
        // It blocks a ray and can be captured.
        let rook = put(&mut st, 2, Kind::Rook, 4, 9);
        st.set(Tile::at(4, 5), None);
        let ts = targets(&st, rook);
        assert!(has(&ts, 4, 4, TargetKind::Capture));
        assert!(!tiles(&ts).contains(&(4, 3)), "the hero blocks");
        st.to_move = 2;
        let last = mv(&mut st, (4, 9), (4, 4)).unwrap();
        assert_eq!(last.captured, Some(Kind::Hero));
    }

    #[test]
    fn hero_swap_consumes_the_turn_and_removes_the_pawn() {
        let mut st = two();
        put(&mut st, 0, Kind::Hero, 0, 1);
        put(&mut st, 0, Kind::Pawn, 3, 3);
        put(&mut st, 0, Kind::Pawn, 2, 2);
        st.quiet = 7;
        let before = count(&st, 0);
        let last = mv(&mut st, (0, 1), (3, 3)).unwrap();
        assert_eq!(last.kind, ActionKind::HeroSwap);
        assert_eq!(last.captured, None, "the own pawn is credited to nobody");
        assert!(st.seats[0].captured.is_empty());
        assert_eq!(st.piece(Tile::at(3, 3)).unwrap().kind, Kind::HeroAwake);
        assert_eq!(st.piece(Tile::at(3, 3)).unwrap().id, 1, "same id");
        assert_eq!(st.piece(Tile::at(0, 1)), None, "the old tile is empty");
        assert_eq!(count(&st, 0), before - 1);
        assert_eq!(st.to_move, 2, "the swap was the whole turn");
        assert_eq!(st.turn, 2);
        assert_eq!(st.quiet, 0, "a swap is progress");
        // An awake hero never sleeps again: its targets are moves.
        st.to_move = 0;
        assert!(targets(&st, Tile::at(3, 3)).iter().all(|t| matches!(t.kind, TargetKind::Move | TargetKind::Capture)));
    }

    #[test]
    fn hero_wakes_in_place_only_with_no_pawns() {
        let mut st = two();
        let hero = put(&mut st, 0, Kind::Hero, 4, 4);
        put(&mut st, 0, Kind::Pawn, 7, 1);
        assert_eq!(mv(&mut st, (4, 4), (4, 4)), Err(Illegal::DormantHero));
        // A promoted pawn is a queen, not a swap target.
        st.set(Tile::at(7, 1), None);
        put(&mut st, 0, Kind::Queen, 7, 1);
        let ts = targets(&st, hero);
        assert_eq!(ts, vec![Target { x: 4, y: 4, kind: TargetKind::Wake }]);
        let last = mv(&mut st, (4, 4), (4, 4)).unwrap();
        assert_eq!(last.kind, ActionKind::HeroWake);
        assert_eq!(st.piece(hero).unwrap().kind, Kind::HeroAwake);
        assert_eq!(st.to_move, 2, "waking consumes the turn");
        assert_eq!(st.quiet, 0);
    }

    #[test]
    fn self_move_rejected_for_every_other_kind() {
        for kind in [
            Kind::King,
            Kind::Queen,
            Kind::Rook,
            Kind::Bishop,
            Kind::Knight,
            Kind::Pawn,
            Kind::Joker,
            Kind::HeroAwake,
        ] {
            let mut st = two();
            put(&mut st, 0, kind, 4, 4);
            st.seats[0].own_turns = 5;
            assert_eq!(mv(&mut st, (4, 4), (4, 4)), Err(Illegal::SelfMove), "{kind:?}");
            assert!(targets(&st, Tile::at(4, 4)).iter().all(|t| t.tile() != Tile::at(4, 4)));
        }
    }

    #[test]
    fn hero_awake_is_rook_plus_knight() {
        let mut st = two();
        let hero = put(&mut st, 0, Kind::HeroAwake, 4, 4);
        put(&mut st, 2, Kind::Pawn, 4, 7);
        put(&mut st, 0, Kind::Pawn, 2, 5);
        let ts = targets(&st, hero);
        let mut expected = BTreeSet::new();
        for x in 0..10u8 {
            if x != 4 {
                expected.insert((x, 4));
            }
        }
        for y in 0..=7u8 {
            if y != 4 {
                expected.insert((4, y));
            }
        }
        for d in KNIGHT8 {
            if let Some(t) = Tile::at(4, 4).offset(d)
                && t != Tile::at(2, 5)
            {
                expected.insert((t.x, t.y));
            }
        }
        assert_eq!(tiles(&ts), expected);
        assert!(has(&ts, 4, 7, TargetKind::Capture));
        assert!(!tiles(&ts).contains(&(5, 5)), "no diagonal");
        assert_eq!(material(Kind::HeroAwake), 8);
    }

    #[test]
    fn turn_1_sweep() {
        // 17 per seat: 2 knight, 8 pawn, 7 hero swaps, joker 0.
        let per_seat = |st: &State, seat: u8| -> (usize, [usize; 4]) {
            let mut view = st.clone();
            view.to_move = seat;
            let mut total = 0;
            let mut by_kind = [0; 4];
            for (t, p) in view.pieces_of(seat) {
                let n = targets(&view, t).len();
                total += n;
                match p.kind {
                    Kind::Knight => by_kind[0] += n,
                    Kind::Pawn => by_kind[1] += n,
                    Kind::Hero => by_kind[2] += n,
                    Kind::Joker => by_kind[3] += n,
                    _ => assert_eq!(n, 0, "seat {seat} {p:?} at {t:?} is boxed in"),
                }
            }
            (total, by_kind)
        };
        let st = full();
        for seat in 0..4u8 {
            assert_eq!(per_seat(&st, seat), (17, [2, 8, 7, 0]), "seat {seat}");
        }
        // Garrisons block exactly like humans.
        let st = setup([true, false, true, false], [Formation::DEFAULT; 4]);
        assert_eq!(per_seat(&st, 0), (17, [2, 8, 7, 0]));
        assert_eq!(per_seat(&st, 2), (17, [2, 8, 7, 0]));
        // Through the join table, the same.
        let mut present = [false; 4];
        for &seat in &SEAT_BY_JOIN[..3] {
            present[seat as usize] = true;
        }
        let st = setup(present, [Formation::DEFAULT; 4]);
        assert_eq!(per_seat(&st, 1), (17, [2, 8, 7, 0]));
    }

    #[test]
    fn one_capture_per_apply() {
        let mut st = two();
        put(&mut st, 0, Kind::Queen, 4, 4);
        put(&mut st, 2, Kind::Pawn, 7, 4);
        put(&mut st, 2, Kind::Pawn, 8, 4);
        let last = mv(&mut st, (4, 4), (7, 4)).unwrap();
        assert_eq!(last.captured, Some(Kind::Pawn));
        assert_eq!(st.seats[0].captured, vec![Kind::Pawn]);
        assert_eq!(st.piece(Tile::at(8, 4)).unwrap().kind, Kind::Pawn, "the second is untouched");
        assert_eq!(count(&st, 2), 2);
    }

    #[test]
    fn king_capture_eliminates_and_removes_pieces() {
        let mut st = blank([true, true, true, false]);
        put(&mut st, 0, Kind::King, 0, 0);
        put(&mut st, 0, Kind::Rook, 5, 0);
        put(&mut st, 1, Kind::King, 5, 9);
        put(&mut st, 1, Kind::Pawn, 7, 7);
        put(&mut st, 1, Kind::Queen, 0, 5);
        put(&mut st, 2, Kind::King, 9, 9);
        let last = mv(&mut st, (5, 0), (5, 9)).unwrap();
        assert_eq!(last.captured, Some(Kind::King));
        assert_eq!(last.eliminated, Some(1));
        assert!(!st.seats[1].alive);
        assert!(st.seats[1].present, "a seated player stays present");
        assert_eq!(count(&st, 1), 0, "every piece of the seat is gone");
        assert_eq!(st.piece(Tile::at(7, 7)), None);
        assert_eq!(st.seats[0].captured, vec![Kind::King]);
        assert_eq!(st.alive_count(), 2);
        assert_eq!(st.result, None);
        assert_eq!(st.to_move, 2, "seat 1 is skipped");
        assert_eq!(st.turn, 2);
    }

    #[test]
    fn garrison_king_capture_is_plain() {
        let mut st = two();
        put(&mut st, 0, Kind::Rook, 9, 5);
        put(&mut st, 1, Kind::King, 9, 0);
        put(&mut st, 1, Kind::Pawn, 8, 0);
        let seats_before = st.seats.clone();
        let last = mv(&mut st, (9, 5), (9, 0)).unwrap();
        assert_eq!(last.captured, Some(Kind::King));
        assert_eq!(last.eliminated, None);
        assert_eq!(st.piece(Tile::at(8, 0)).unwrap().owner, 1, "the garrison stays");
        assert_eq!(st.seats[1], seats_before[1]);
        assert_eq!(st.seats[0].captured, vec![Kind::King]);
        assert_eq!(st.result, None);
        assert_eq!(st.to_move, 2);
    }

    #[test]
    fn turn_increments_once_per_end_turn_and_own_turns_on_own_turn_start_only() {
        let mut st = full();
        let own = |st: &State| [0, 1, 2, 3].map(|s| st.seats[s].own_turns);
        assert_eq!((st.turn, own(&st)), (1, [1, 0, 0, 0]));
        mv(&mut st, (1, 2), (2, 4)).unwrap();
        assert_eq!((st.turn, st.to_move, own(&st)), (2, 1, [1, 1, 0, 0]));
        mv(&mut st, (7, 1), (5, 2)).unwrap();
        assert_eq!((st.turn, st.to_move, own(&st)), (3, 2, [1, 1, 1, 0]));
        mv(&mut st, (8, 7), (7, 5)).unwrap();
        mv(&mut st, (2, 8), (4, 7)).unwrap();
        assert_eq!((st.turn, st.to_move, own(&st)), (5, 0, [2, 1, 1, 1]));
        timeout(&mut st).unwrap();
        assert_eq!((st.turn, st.to_move, own(&st)), (6, 1, [2, 2, 1, 1]));
        assert_eq!(st.seats[0].timeouts, 1);
        disconnect(&mut st, 2);
        assert_eq!(own(&st), [2, 2, 1, 1], "another seat's elimination touches nothing");
        mv(&mut st, (5, 2), (7, 1)).unwrap();
        assert_eq!((st.turn, st.to_move, own(&st)), (7, 3, [2, 2, 1, 2]));
        // A legal move resets the seat's timeouts.
        mv(&mut st, (4, 7), (2, 8)).unwrap();
        assert_eq!(st.to_move, 0);
        mv(&mut st, (2, 4), (1, 2)).unwrap();
        assert_eq!(st.seats[0].timeouts, 0);
        assert_eq!(st.quiet, 8, "eight quiet turns so far (seven moves, one timeout)");
    }

    /// Seat 0 with a king and stuck pawns in the NE corner: no legal move.
    fn stalled_seat_0(st: &mut State) {
        put(st, 0, Kind::King, 9, 9);
        put(st, 0, Kind::Pawn, 8, 9);
        put(st, 0, Kind::Pawn, 9, 8);
        put(st, 0, Kind::Pawn, 8, 8);
    }

    #[test]
    fn timeout_three_eliminates_and_resets_stalls() {
        // The Judge scenario: A (seat 0) stalled, B (seat 2) AFK.
        let mut st = blank([true, false, true, false]);
        stalled_seat_0(&mut st);
        put(&mut st, 2, Kind::King, 0, 0);
        st.to_move = 2;
        st.seats[0].own_turns = 0;
        st.seats[2].own_turns = 1;
        assert!(!has_any_move(&st, 0));
        assert!(has_any_move(&st, 2));

        let last = timeout(&mut st).unwrap();
        assert_eq!((last.seat, last.kind, last.eliminated), (2, ActionKind::Timeout, None));
        assert_eq!(st.seats[2].timeouts, 1);
        // A was passed for: not a timeout, a stall.
        assert_eq!(st.seats[0].timeouts, 0);
        assert_eq!(st.stalls, 1);
        assert_eq!(st.last, Some(pass(0, ActionKind::Pass, None)));
        assert_eq!((st.to_move, st.turn), (2, 3));
        assert_eq!(st.seats[0].own_turns, 1);

        timeout(&mut st).unwrap();
        assert_eq!(st.seats[2].timeouts, 2);
        assert_eq!(st.stalls, 1, "the timeout reset the stall count, A stalled again");
        assert_eq!(st.result, None);

        let last = timeout(&mut st).unwrap();
        assert_eq!(last.eliminated, Some(2));
        assert!(!st.seats[2].alive);
        assert_eq!(count(&st, 2), 0);
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: Some(0),
                end: EndReason::LastKing
            }),
            "never a draw"
        );
        assert_eq!(timeout(&mut st), None, "nothing after the result");
    }

    #[test]
    fn forced_pass_is_not_a_timeout() {
        let mut st = blank([true, false, true, false]);
        stalled_seat_0(&mut st);
        put(&mut st, 2, Kind::King, 0, 0);
        st.to_move = 2;
        st.seats[2].own_turns = 1;
        st.seats[0].own_turns = 0;
        mv(&mut st, (0, 0), (1, 1)).unwrap();
        assert_eq!(st.seats[0].timeouts, 0);
        assert_eq!(st.seats[0].own_turns, 1, "the pass was still A's own turn");
        assert_eq!(st.stalls, 1);
        assert_eq!(st.quiet, 2, "the king move and the pass are both quiet");
        assert_eq!(st.last.unwrap().kind, ActionKind::Pass);
        assert_eq!(st.last.unwrap().seat, 0);
        assert_eq!(st.to_move, 2);
        assert_eq!(st.turn, 3, "the pass consumed a turn number");
        assert_eq!(st.clock.left_ms, crate::clock::TURN_TOTAL_MS);
    }

    #[test]
    fn full_round_of_stalls_ends_by_material() {
        let build = |extra_for_2: bool| {
            let mut st = blank([true, false, true, false]);
            stalled_seat_0(&mut st);
            put(&mut st, 0, Kind::Pawn, 9, 7);
            put(&mut st, 2, Kind::King, 0, 0);
            put(&mut st, 2, Kind::Pawn, 1, 0);
            put(&mut st, 2, Kind::Pawn, 0, 1);
            put(&mut st, 2, Kind::Pawn, 2, 1);
            if extra_for_2 {
                put(&mut st, 2, Kind::Pawn, 0, 2);
            }
            st.to_move = 2;
            st.seats[2].own_turns = 1;
            st.seats[0].own_turns = 0;
            st
        };
        let mut st = build(false);
        assert_eq!(material_of(&st, 0), 4);
        assert_eq!(material_of(&st, 2), 3);
        // Seat 2 walls itself in; seat 0 cannot move; seat 2 cannot either.
        mv(&mut st, (2, 1), (1, 1)).unwrap();
        assert_eq!(st.stalls, 2);
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: Some(0),
                end: EndReason::Stalemate
            })
        );
        let mut st = build(true);
        mv(&mut st, (2, 1), (1, 1)).unwrap();
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: None,
                end: EndReason::Stalemate
            }),
            "a shared maximum is a draw"
        );
    }

    #[test]
    fn quiet_resets_on_capture_pawn_move_promotion_and_swap() {
        let mut st = two();
        put(&mut st, 0, Kind::Rook, 0, 5);
        put(&mut st, 0, Kind::Pawn, 4, 4);
        put(&mut st, 0, Kind::Pawn, 8, 0);
        put(&mut st, 0, Kind::Hero, 2, 2);
        put(&mut st, 2, Kind::Pawn, 0, 9);
        let king_hop = |st: &mut State, back: bool| {
            let (from, to) = if back { ((9, 8), (9, 9)) } else { ((9, 9), (9, 8)) };
            mv(st, from, to).unwrap();
        };
        st.quiet = 50;
        mv(&mut st, (0, 5), (0, 6)).unwrap();
        assert_eq!(st.quiet, 51, "a rook move is quiet");
        king_hop(&mut st, false);
        assert_eq!(st.quiet, 52);
        mv(&mut st, (4, 4), (5, 4)).unwrap();
        assert_eq!(st.quiet, 0, "a pawn move");
        king_hop(&mut st, true);
        st.quiet = 30;
        mv(&mut st, (0, 6), (0, 9)).unwrap();
        assert_eq!(st.quiet, 0, "a capture");
        king_hop(&mut st, false);
        st.quiet = 30;
        let last = mv(&mut st, (8, 0), (9, 0)).unwrap();
        assert!(last.promoted);
        assert_eq!(st.quiet, 0, "a promotion");
        king_hop(&mut st, true);
        st.quiet = 30;
        mv(&mut st, (2, 2), (5, 4)).unwrap();
        assert_eq!(st.quiet, 0, "a hero swap");
        king_hop(&mut st, false);
        st.quiet = 30;
        mv(&mut st, (5, 4), (5, 8)).unwrap();
        assert_eq!(st.quiet, 31, "an awake hero's quiet move");
    }

    #[test]
    fn no_progress_ends_at_100() {
        assert_eq!(NO_PROGRESS_TURNS, 100);
        let mut st = two();
        put(&mut st, 0, Kind::Rook, 4, 4);
        st.quiet = 98;
        mv(&mut st, (0, 0), (0, 1)).unwrap();
        assert_eq!(st.quiet, 99);
        assert_eq!(st.result, None);
        mv(&mut st, (9, 9), (9, 8)).unwrap();
        assert_eq!(st.quiet, 100);
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: Some(0),
                end: EndReason::NoProgress
            })
        );
        assert_eq!(mv(&mut st, (0, 1), (0, 0)), Err(Illegal::GameOver));
    }

    #[test]
    fn turn_cap_at_600() {
        assert_eq!(MAX_TURNS, 600);
        let mut st = two();
        st.turn = 599;
        mv(&mut st, (0, 0), (0, 1)).unwrap();
        assert_eq!(st.turn, 600);
        assert_eq!(st.result, None);
        mv(&mut st, (9, 9), (9, 8)).unwrap();
        assert_eq!(st.turn, 600, "the turn number does not advance past the cap");
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: None,
                end: EndReason::TurnCap
            }),
            "two bare kings tie"
        );
    }

    #[test]
    fn material_ranking_pinned() {
        let pinned = [
            (Kind::Queen, 9),
            (Kind::HeroAwake, 8),
            (Kind::Rook, 5),
            (Kind::Joker, 4),
            (Kind::Knight, 3),
            (Kind::Bishop, 3),
            (Kind::Hero, 3),
            (Kind::Pawn, 1),
            (Kind::King, 0),
        ];
        assert_eq!(MATERIAL, pinned);
        for (kind, value) in pinned {
            assert_eq!(material(kind), value, "{kind:?}");
        }
        let st = full();
        for seat in 0..4u8 {
            assert_eq!(material_of(&st, seat), 9 + 3 + 4 + 5 + 5 + 3 + 3 + 3 + 7);
        }
        // Unique maximum wins; garrison material never counts.
        let mut st = blank([true, true, true, false]);
        put(&mut st, 0, Kind::King, 0, 0);
        put(&mut st, 0, Kind::Rook, 0, 5);
        put(&mut st, 1, Kind::King, 9, 0);
        put(&mut st, 1, Kind::Knight, 9, 3);
        put(&mut st, 2, Kind::King, 9, 9);
        put(&mut st, 3, Kind::Queen, 5, 5);
        finish_by_material(&mut st, EndReason::TurnCap);
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: Some(0),
                end: EndReason::TurnCap
            })
        );
        // A tie at the top is a draw even with a third seat below.
        let mut st = blank([true, true, true, false]);
        put(&mut st, 0, Kind::King, 0, 0);
        put(&mut st, 0, Kind::Rook, 0, 5);
        put(&mut st, 1, Kind::King, 9, 0);
        put(&mut st, 1, Kind::Rook, 9, 3);
        put(&mut st, 2, Kind::King, 9, 9);
        put(&mut st, 2, Kind::Pawn, 8, 9);
        finish_by_material(&mut st, EndReason::NoProgress);
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: None,
                end: EndReason::NoProgress
            })
        );
    }

    #[test]
    fn disconnect_of_the_mover_ends_the_turn() {
        let mut st = blank([true, true, true, false]);
        put(&mut st, 0, Kind::King, 0, 0);
        put(&mut st, 0, Kind::Rook, 4, 4);
        put(&mut st, 1, Kind::King, 9, 0);
        put(&mut st, 2, Kind::King, 9, 9);
        let last = disconnect(&mut st, 0).unwrap();
        assert_eq!(last, pass(0, ActionKind::Pass, Some(0)));
        assert_eq!(st.last, Some(last));
        assert!(!st.seats[0].present && !st.seats[0].alive);
        assert_eq!(count(&st, 0), 0);
        assert_eq!((st.to_move, st.turn), (1, 2));
        assert_eq!(st.seats[1].own_turns, 1);
        assert_eq!(st.result, None);
        // A disconnect of a seat that is not to move: skipped, no action.
        assert_eq!(disconnect(&mut st, 2), None);
        assert_eq!(st.last, Some(last), "narration untouched");
        assert!(!st.seats[2].alive);
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: Some(1),
                end: EndReason::LastKing
            }),
            "the instant one seat is left"
        );
    }

    #[test]
    fn last_alive_wins() {
        let mut st = two();
        assert_eq!(disconnect(&mut st, 2), None);
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: Some(0),
                end: EndReason::LastKing
            })
        );
        let mut st = two();
        disconnect(&mut st, 0).unwrap();
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: Some(2),
                end: EndReason::LastKing
            })
        );
        // Once decided, a later disconnect changes nothing but presence.
        assert_eq!(disconnect(&mut st, 2), None);
        assert!(!st.seats[2].present);
        assert_eq!(st.result.unwrap().winner, Some(2));
    }

    #[test]
    fn all_gone_is_abandoned() {
        // Reachable only when the last seated player leaves before the
        // result was settled: a hand-built one-seat state.
        let mut st = blank([true, false, false, false]);
        put(&mut st, 0, Kind::King, 0, 0);
        assert_eq!(st.result, None);
        assert_eq!(disconnect(&mut st, 0), Some(pass(0, ActionKind::Pass, Some(0))));
        assert_eq!(
            st.result,
            Some(Outcome {
                winner: None,
                end: EndReason::Abandoned
            })
        );
    }

    #[test]
    fn action_kind_derivation() {
        let at = Tile::at;
        assert_eq!(action_kind_of(Kind::Hero, at(4, 4), at(4, 4)), ActionKind::HeroWake);
        assert_eq!(action_kind_of(Kind::Hero, at(4, 4), at(7, 7)), ActionKind::HeroSwap);
        assert_eq!(action_kind_of(Kind::Joker, at(4, 4), at(5, 5)), ActionKind::Move);
        assert_eq!(action_kind_of(Kind::Joker, at(4, 4), at(3, 4)), ActionKind::Move);
        // The row mirror of x = 4 is x = 5: a step, narrated as a step.
        assert_eq!(action_kind_of(Kind::Joker, at(4, 4), at(5, 4)), ActionKind::Move);
        assert_eq!(action_kind_of(Kind::Joker, at(5, 2), at(4, 2)), ActionKind::Move);
        assert_eq!(action_kind_of(Kind::Joker, at(2, 3), at(7, 3)), ActionKind::JokerTeleport);
        assert_eq!(action_kind_of(Kind::Joker, at(2, 3), at(2, 6)), ActionKind::JokerTeleport);
        assert_eq!(action_kind_of(Kind::Joker, at(2, 3), at(7, 6)), ActionKind::JokerTeleport);
        assert_eq!(action_kind_of(Kind::Joker, at(2, 3), at(6, 6)), ActionKind::JokerPlace);
        assert_eq!(action_kind_of(Kind::Joker, at(2, 3), at(2, 5)), ActionKind::JokerPlace);
        for kind in [Kind::King, Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight, Kind::Pawn, Kind::HeroAwake] {
            assert_eq!(action_kind_of(kind, at(2, 3), at(7, 6)), ActionKind::Move, "{kind:?}");
        }
        // The dedup in targets agrees: the row-mirror step is one Move.
        let mut st = two();
        let joker = put(&mut st, 0, Kind::Joker, 4, 4);
        let ts = targets(&st, joker);
        let at54: Vec<_> = ts.iter().filter(|t| (t.x, t.y) == (5, 4)).collect();
        assert_eq!(at54.len(), 1);
        assert_eq!(at54[0].kind, TargetKind::Move);
        assert!(has(&ts, 4, 5, TargetKind::Move));
        assert!(has(&ts, 5, 5, TargetKind::Move), "the centre mirror of (4,4) is also a step");
        assert_eq!(ts.iter().filter(|t| t.kind == TargetKind::Teleport).count(), 0);
    }

    #[test]
    fn illegal_reasons_are_readable() {
        let all = [
            Illegal::GameOver,
            Illegal::OffBoard,
            Illegal::NoPiece,
            Illegal::NotYours,
            Illegal::SelfMove,
            Illegal::OwnPiece,
            Illegal::DormantHero,
            Illegal::NotATarget,
        ];
        for e in all {
            let r = e.reason();
            assert!(!r.is_empty());
            assert!(r.chars().next().unwrap().is_lowercase(), "{r}");
            assert!(!r.ends_with('.'), "{r}");
            assert_eq!(e.to_string(), r);
        }
        for e in [Illegal::SelfMove, Illegal::OwnPiece, Illegal::DormantHero, Illegal::NotATarget] {
            assert!(e.reason().starts_with("cannot move there"), "{e}");
        }
        assert_eq!(Illegal::NotATarget.reason(), "cannot move there");

        // And each is produced where it should be.
        let mut st = full();
        assert_eq!(apply_xy(&mut st, 3, 0, 4, 10), Err(Illegal::OffBoard));
        assert_eq!(apply_xy(&mut st, 10, 0, 4, 0), Err(Illegal::OffBoard));
        assert_eq!(mv(&mut st, (4, 4), (5, 5)), Err(Illegal::NoPiece));
        assert_eq!(mv(&mut st, (9, 3), (9, 4)), Err(Illegal::NotYours));
        assert_eq!(mv(&mut st, (3, 0), (3, 0)), Err(Illegal::SelfMove));
        assert_eq!(mv(&mut st, (3, 0), (3, 1)), Err(Illegal::OwnPiece));
        assert_eq!(mv(&mut st, (3, 0), (5, 0)), Err(Illegal::NotATarget));
        assert_eq!(mv(&mut st, (0, 1), (0, 1)), Err(Illegal::DormantHero));
        assert_eq!(mv(&mut st, (0, 1), (4, 4)), Err(Illegal::DormantHero));
        assert_eq!(st, full(), "a refused move leaves the state untouched");
        let ok = apply_xy(&mut st, 3, 0, 4, 0).unwrap();
        assert_eq!(ok.kind, ActionKind::Move);
        assert_eq!(st.to_move, 1);
        assert_eq!(mv(&mut st, (3, 1), (4, 1)), Err(Illegal::NotYours), "not seat 0's turn now");
        st.result = Some(Outcome {
            winner: None,
            end: EndReason::Abandoned,
        });
        assert_eq!(mv(&mut st, (9, 3), (9, 4)), Err(Illegal::GameOver));
    }

    #[test]
    fn targets_of_an_empty_tile_are_none() {
        let st = full();
        assert!(targets(&st, Tile::at(5, 5)).is_empty());
        assert!(targets(&st, Tile::at(0, 0)).is_empty(), "the king is boxed in");
    }
}
