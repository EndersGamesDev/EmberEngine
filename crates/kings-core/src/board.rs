//! The board, the four seat frames, the setup and the `State` (sections 1.1
//! to 1.4 and 4.1 of the design).
//!
//! Everything here is integer arithmetic on `u8`/`i8` tiles. There is no
//! table of far edges or promotion lines: they are computed from the seat
//! vectors, exactly as the design says, so the doc's formulas and the crate
//! cannot drift apart.

use crate::clock::TurnClock;
use crate::proto::{BoardState, EndReason, Formation, Kind, LastAction, PieceState, SeatState};

/// Tiles per side.
pub const SIDE: u8 = 10;
/// Tiles on the board.
pub const TILES: usize = 100;
/// Seats at the table.
pub const SEATS: usize = 4;
/// `SEATS` as a seat index bound.
const SEATS_U8: u8 = 4;
/// Pieces per seat at setup.
pub const PIECES_PER_SEAT: u8 = 16;
/// The home block of a seat is `u, v <= HOME_MAX` in its local frame.
pub const HOME_MAX: u8 = 3;

/// A board tile. `x` is east, `y` is north, both `0..=9`; `(0,0)` is the
/// south-west corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tile {
    /// Column, 0 at the west edge.
    pub x: u8,
    /// Row, 0 at the south edge.
    pub y: u8,
}

impl Tile {
    /// A tile from board coordinates, or `None` off the board. This is the
    /// only way in from wire coordinates.
    #[must_use]
    pub const fn new(x: u8, y: u8) -> Option<Self> {
        if x < SIDE && y < SIDE {
            Some(Self { x, y })
        } else {
            None
        }
    }

    /// A tile that is known to be on the board (a literal in a test, a
    /// tile produced by `from_index`).
    ///
    /// # Panics
    /// Off the board.
    #[must_use]
    pub const fn at(x: u8, y: u8) -> Self {
        assert!(x < SIDE && y < SIDE, "tile off the board");
        Self { x, y }
    }

    /// Storage index `y * 10 + x`.
    #[must_use]
    pub const fn index(self) -> usize {
        self.y as usize * SIDE as usize + self.x as usize
    }

    /// The tile with storage index `i`, or `None` for `i >= 100`.
    #[must_use]
    pub const fn from_index(i: usize) -> Option<Self> {
        if i < TILES {
            // Both quotients are below SIDE, so the narrowing is exact.
            #[allow(clippy::cast_possible_truncation)]
            let t = Self {
                x: (i % SIDE as usize) as u8,
                y: (i / SIDE as usize) as u8,
            };
            Some(t)
        } else {
            None
        }
    }

    /// The tile `self + d`, or `None` off the board.
    #[must_use]
    pub const fn offset(self, d: Dir) -> Option<Self> {
        let x = self.x as i16 + d.dx as i16;
        let y = self.y as i16 + d.dy as i16;
        if x < 0 || y < 0 || x >= SIDE as i16 || y >= SIDE as i16 {
            None
        } else {
            // Both are within 0..SIDE after the check.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let t = Self {
                x: x as u8,
                y: y as u8,
            };
            Some(t)
        }
    }

    /// Tile colour for bishops: the parity of `x + y` (0 or 1).
    #[must_use]
    pub const fn colour(self) -> u8 {
        (self.x + self.y) % 2
    }

    /// Every tile, in storage order.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..TILES).filter_map(Self::from_index)
    }
}

/// A direction on the board: a step of `dx` east and `dy` north.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dir {
    /// Eastward component.
    pub dx: i8,
    /// Northward component.
    pub dy: i8,
}

impl Dir {
    /// A direction from its components.
    #[must_use]
    pub const fn new(dx: i8, dy: i8) -> Self {
        Self { dx, dy }
    }

    /// `self + other`.
    #[must_use]
    pub const fn add(self, other: Self) -> Self {
        Self {
            dx: self.dx + other.dx,
            dy: self.dy + other.dy,
        }
    }

    /// `self - other`.
    #[must_use]
    pub const fn sub(self, other: Self) -> Self {
        Self {
            dx: self.dx - other.dx,
            dy: self.dy - other.dy,
        }
    }

    /// `-self`.
    #[must_use]
    pub const fn neg(self) -> Self {
        Self {
            dx: -self.dx,
            dy: -self.dy,
        }
    }

    /// `a * f + b * l`: a direction in a seat's `(forward, left)` basis.
    #[must_use]
    pub const fn in_frame(a: i8, b: i8, frame: Frame) -> Self {
        Self {
            dx: a * frame.f.dx + b * frame.l.dx,
            dy: a * frame.f.dy + b * frame.l.dy,
        }
    }
}

/// The eight king directions.
pub const ALL8: [Dir; 8] = [
    Dir::new(1, 0),
    Dir::new(1, 1),
    Dir::new(0, 1),
    Dir::new(-1, 1),
    Dir::new(-1, 0),
    Dir::new(-1, -1),
    Dir::new(0, -1),
    Dir::new(1, -1),
];
/// The four rook directions.
pub const ORTHO4: [Dir; 4] = [
    Dir::new(1, 0),
    Dir::new(0, 1),
    Dir::new(-1, 0),
    Dir::new(0, -1),
];
/// The four bishop directions.
pub const DIAG4: [Dir; 4] = [
    Dir::new(1, 1),
    Dir::new(-1, 1),
    Dir::new(-1, -1),
    Dir::new(1, -1),
];
/// The eight knight offsets.
pub const KNIGHT8: [Dir; 8] = [
    Dir::new(1, 2),
    Dir::new(2, 1),
    Dir::new(-1, 2),
    Dir::new(-2, 1),
    Dir::new(1, -2),
    Dir::new(2, -1),
    Dir::new(-1, -2),
    Dir::new(-2, -1),
];

/// A seat's frame: its corner tile, its forward vector `f` (along the home
/// edge toward the next seat in turn order) and its left vector `l` (`f`
/// rotated 90 degrees counter-clockwise). Section 1.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    /// The corner tile, local `(0,0)`.
    pub corner: Tile,
    /// Forward.
    pub f: Dir,
    /// Left.
    pub l: Dir,
}

/// The four frames, indexed by seat: 0 = SW, 1 = SE, 2 = NE, 3 = NW.
pub const FRAMES: [Frame; 4] = [
    Frame {
        corner: Tile { x: 0, y: 0 },
        f: Dir::new(1, 0),
        l: Dir::new(0, 1),
    },
    Frame {
        corner: Tile { x: 9, y: 0 },
        f: Dir::new(0, 1),
        l: Dir::new(-1, 0),
    },
    Frame {
        corner: Tile { x: 9, y: 9 },
        f: Dir::new(-1, 0),
        l: Dir::new(0, -1),
    },
    Frame {
        corner: Tile { x: 0, y: 9 },
        f: Dir::new(0, -1),
        l: Dir::new(1, 0),
    },
];

/// The frame of a seat.
///
/// # Panics
/// For a seat above 3.
#[must_use]
pub const fn frame(seat: u8) -> Frame {
    FRAMES[seat as usize]
}

/// A seat's forward vector.
#[must_use]
pub const fn forward(seat: u8) -> Dir {
    frame(seat).f
}

/// A seat's left vector.
#[must_use]
pub const fn left(seat: u8) -> Dir {
    frame(seat).l
}

/// A seat's front-left, `f + l`: the joker's capture direction and one of
/// the pawn's.
#[must_use]
pub const fn front_left(seat: u8) -> Dir {
    frame(seat).f.add(frame(seat).l)
}

/// Local `(u, v)` (tiles forward and left of the corner) to the global tile.
///
/// # Panics
/// If `u` or `v` is above 9.
#[must_use]
pub const fn to_global(seat: u8, u: u8, v: u8) -> Tile {
    assert!(u < SIDE && v < SIDE, "local coordinate off the board");
    let fr = frame(seat);
    let x = fr.corner.x as i16 + u as i16 * fr.f.dx as i16 + v as i16 * fr.l.dx as i16;
    let y = fr.corner.y as i16 + u as i16 * fr.f.dy as i16 + v as i16 * fr.l.dy as i16;
    // A rotation of the board maps the board onto itself.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let t = Tile {
        x: x as u8,
        y: y as u8,
    };
    t
}

/// Global tile to the seat's local `(u, v)`.
#[must_use]
pub const fn to_local(seat: u8, t: Tile) -> (u8, u8) {
    let fr = frame(seat);
    let dx = t.x as i16 - fr.corner.x as i16;
    let dy = t.y as i16 - fr.corner.y as i16;
    let u = dx * fr.f.dx as i16 + dy * fr.f.dy as i16;
    let v = dx * fr.l.dx as i16 + dy * fr.l.dy as i16;
    // The frame vectors are unit axes, so u and v are 0..=9.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let uv = (u as u8, v as u8);
    uv
}

/// Whether a tile lies in a seat's home block (`u, v <= 3`).
#[must_use]
pub const fn in_home_block(seat: u8, t: Tile) -> bool {
    let (u, v) = to_local(seat, t);
    u <= HOME_MAX && v <= HOME_MAX
}

/// Whether a tile is on one of a seat's two far edges: its forward or left
/// neighbour is off the board. Computed, not tabulated, as section 1.2
/// says; `promotion_line_is_a_formula` pins it to `u == 9 || v == 9`.
#[must_use]
pub const fn on_far_edge(seat: u8, t: Tile) -> bool {
    t.offset(forward(seat)).is_none() || t.offset(left(seat)).is_none()
}

/// The 90-degree board rotation that carries seat `s` onto seat `s + 1`:
/// `(x, y) -> (9 - y, x)`.
#[must_use]
pub const fn rot(t: Tile) -> Tile {
    Tile {
        x: SIDE - 1 - t.y,
        y: t.x,
    }
}

/// Row mirror `(9 - x, y)`.
#[must_use]
pub const fn mirror_row(t: Tile) -> Tile {
    Tile {
        x: SIDE - 1 - t.x,
        y: t.y,
    }
}

/// Column mirror `(x, 9 - y)`.
#[must_use]
pub const fn mirror_col(t: Tile) -> Tile {
    Tile {
        x: t.x,
        y: SIDE - 1 - t.y,
    }
}

/// Centre mirror `(9 - x, 9 - y)`.
#[must_use]
pub const fn mirror_centre(t: Tile) -> Tile {
    Tile {
        x: SIDE - 1 - t.x,
        y: SIDE - 1 - t.y,
    }
}

/// The joker's three teleport tiles: row, column and centre mirror.
#[must_use]
pub const fn mirrors(t: Tile) -> [Tile; 3] {
    [mirror_row(t), mirror_col(t), mirror_centre(t)]
}

/// The default formation's local tiles by setup index (section 1.3): the
/// four Legend tiles, the five Epic tiles, the seven Common tiles.
pub const SETUP_LOCAL: [(u8, u8); 16] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (1, 1),
    (2, 0),
    (2, 1),
    (2, 2),
    (1, 2),
    (0, 2),
    (3, 0),
    (3, 1),
    (3, 2),
    (3, 3),
    (2, 3),
    (1, 3),
    (0, 3),
];

/// Setup indices `0..LEGEND_END` are Legend tiles.
pub const LEGEND_END: usize = 4;
/// Setup indices `LEGEND_END..EPIC_END` are Epic tiles; the rest are pawns.
pub const EPIC_END: usize = 9;

/// Seat by position in the lobby's join order: creator at 0, second joiner
/// diagonal at 2, then 1, then 3.
pub const SEAT_BY_JOIN: [u8; 4] = [0, 2, 1, 3];

/// Why a formation was refused; `reason()` is what the player reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormationError {
    /// The four Legend tiles do not hold king, queen, hero and joker once each.
    LegendNotAPermutation,
    /// The five Epic tiles do not hold two rooks, two bishops and one knight.
    EpicNotAPermutation,
    /// The two bishops stand on tiles of the same colour.
    BishopsOnOneColour,
}

impl FormationError {
    /// The text a player reads in `Rejected`.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::LegendNotAPermutation => {
                "the corner tiles must hold the king, queen, hero and joker once each"
            }
            Self::EpicNotAPermutation => {
                "the tier-1 tiles must hold two rooks, two bishops and one knight"
            }
            Self::BishopsOnOneColour => "the two bishops must start on tiles of opposite colour",
        }
    }
}

impl std::fmt::Display for FormationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

impl std::error::Error for FormationError {}

impl Formation {
    /// The default formation of section 1.3.
    pub const DEFAULT: Self = Self {
        legend: [Kind::King, Kind::Queen, Kind::Hero, Kind::Joker],
        epic: [
            Kind::Rook,
            Kind::Bishop,
            Kind::Bishop,
            Kind::Knight,
            Kind::Rook,
        ],
    };

    /// The kind at a setup index: Legend, Epic, or Pawn for `9..16`.
    #[must_use]
    pub const fn kind_at(&self, index: usize) -> Kind {
        if index < LEGEND_END {
            self.legend[index]
        } else if index < EPIC_END {
            self.epic[index - LEGEND_END]
        } else {
            Kind::Pawn
        }
    }

    /// The class rules of section 2: `legend` is a permutation of King,
    /// Queen, Hero, Joker; `epic` is a permutation of Rook, Rook, Bishop,
    /// Bishop, Knight; the two bishops stand on opposite colours. Colour is
    /// the parity of the local `u + v`, which every seat's transform maps
    /// to a global parity uniformly, so the check is seat-independent.
    ///
    /// # Errors
    /// The first rule broken, in that order.
    pub fn validate(&self) -> Result<(), FormationError> {
        let count = |kinds: &[Kind], k: Kind| kinds.iter().filter(|&&x| x == k).count();
        let legend_ok = [Kind::King, Kind::Queen, Kind::Hero, Kind::Joker]
            .into_iter()
            .all(|k| count(&self.legend, k) == 1);
        if !legend_ok {
            return Err(FormationError::LegendNotAPermutation);
        }
        let epic_ok = count(&self.epic, Kind::Rook) == 2
            && count(&self.epic, Kind::Bishop) == 2
            && count(&self.epic, Kind::Knight) == 1;
        if !epic_ok {
            return Err(FormationError::EpicNotAPermutation);
        }
        let colours: u8 = self
            .epic
            .iter()
            .zip(&SETUP_LOCAL[LEGEND_END..EPIC_END])
            .filter(|(k, _)| **k == Kind::Bishop)
            .map(|(_, (u, v))| (u + v) % 2)
            .sum();
        if colours == 1 {
            Ok(())
        } else {
            Err(FormationError::BishopsOnOneColour)
        }
    }
}

impl Default for Formation {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A piece on the board. The kind is its whole rule state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    /// `seat * 16 + setup index`, stable for the whole game.
    pub id: u8,
    /// Owning seat.
    pub owner: u8,
    /// Current kind.
    pub kind: Kind,
}

/// Per-seat bookkeeping (the engine-side twin of `proto::SeatState`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seat {
    /// A human holds the corner right now.
    pub present: bool,
    /// Takes turns and can win.
    pub alive: bool,
    /// A never-seated corner: inert, capturable pieces.
    pub garrison: bool,
    /// Own turns started, timeouts and forced passes included.
    pub own_turns: u32,
    /// Consecutive own-turn timeouts.
    pub timeouts: u8,
    /// Foreign kinds taken, in order.
    pub captured: Vec<Kind>,
}

/// How a finished game ended and who won (`None` for a draw or an
/// abandoned game).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The winning seat.
    pub winner: Option<u8>,
    /// Why the game ended.
    pub end: EndReason,
}

/// The whole game state (section 4.1). Pure data; `rules` drives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    /// The board, indexed by `Tile::index`.
    pub board: [Option<Piece>; TILES],
    /// The four seats.
    pub seats: [Seat; SEATS],
    /// Seat to move.
    pub to_move: u8,
    /// Global turn number, starting at 1.
    pub turn: u32,
    /// Completed turns since the last progress event.
    pub quiet: u32,
    /// Consecutive forced passes.
    pub stalls: u8,
    /// Time left on the current turn; fed by the server or the hotseat.
    pub clock: TurnClock,
    /// The last action, for narration.
    pub last: Option<LastAction>,
    /// Set once the game is over; nothing is applied after that.
    pub result: Option<Outcome>,
}

impl State {
    /// The piece on a tile.
    #[must_use]
    pub const fn piece(&self, t: Tile) -> Option<Piece> {
        self.board[t.index()]
    }

    /// Set or clear a tile.
    pub const fn set(&mut self, t: Tile, p: Option<Piece>) {
        self.board[t.index()] = p;
    }

    /// Seats that are alive (garrisons and eliminated seats excluded).
    #[must_use]
    pub fn alive_count(&self) -> u8 {
        // At most four.
        #[allow(clippy::cast_possible_truncation)]
        let n = self.seats.iter().filter(|s| s.alive).count() as u8;
        n
    }

    /// The single alive seat, if exactly one is alive.
    #[must_use]
    pub fn sole_survivor(&self) -> Option<u8> {
        let mut alive = self.seats.iter().enumerate().filter(|(_, s)| s.alive);
        let first = alive.next()?;
        if alive.next().is_some() {
            return None;
        }
        u8::try_from(first.0).ok()
    }

    /// The next alive seat after `seat` in 0, 1, 2, 3 order, wrapping; `seat`
    /// itself is returned last, and `None` means nobody is alive.
    #[must_use]
    pub fn next_alive_after(&self, seat: u8) -> Option<u8> {
        (1..=SEATS_U8)
            .map(|k| (seat + k) % SEATS_U8)
            .find(|&s| self.seats[usize::from(s)].alive)
    }

    /// Every piece of a seat with its tile, in storage order.
    pub fn pieces_of(&self, seat: u8) -> impl Iterator<Item = (Tile, Piece)> + '_ {
        self.board
            .iter()
            .enumerate()
            .filter_map(move |(i, p)| match p {
                Some(p) if p.owner == seat => Tile::from_index(i).map(|t| (t, *p)),
                _ => None,
            })
    }

    /// Whether the seat has any pawn on the board.
    #[must_use]
    pub fn has_pawn(&self, seat: u8) -> bool {
        self.pieces_of(seat).any(|(_, p)| p.kind == Kind::Pawn)
    }

    /// Remove every piece a seat owns.
    pub fn remove_pieces_of(&mut self, seat: u8) {
        for slot in &mut self.board {
            if slot.is_some_and(|p| p.owner == seat) {
                *slot = None;
            }
        }
    }
}

/// The initial state of a game (section 1.3 and 1.4).
///
/// Every corner is set up in full from its formation; a corner whose seat is
/// not present is a garrison: `alive = false`, `garrison = true`, pieces
/// inert and capturable. The first alive seat in 0..4 order is to move on
/// turn 1 and has its first own turn started (`own_turns = 1`). Formations
/// are placed as given; the server validates them on `SetFormation`.
#[must_use]
pub fn setup(present: [bool; SEATS], formations: [Formation; SEATS]) -> State {
    let mut state = State {
        board: [None; TILES],
        seats: std::array::from_fn(|seat| Seat {
            present: present[seat],
            alive: present[seat],
            garrison: !present[seat],
            own_turns: 0,
            timeouts: 0,
            captured: Vec::new(),
        }),
        to_move: 0,
        turn: 1,
        quiet: 0,
        stalls: 0,
        clock: TurnClock::new(),
        last: None,
        result: None,
    };
    for (seat, formation) in formations.iter().enumerate() {
        // Seat indices are 0..4.
        #[allow(clippy::cast_possible_truncation)]
        let owner = seat as u8;
        for (index, &(u, v)) in SETUP_LOCAL.iter().enumerate() {
            // Setup indices are 0..16.
            #[allow(clippy::cast_possible_truncation)]
            let id = owner * PIECES_PER_SEAT + index as u8;
            let piece = Piece {
                id,
                owner,
                kind: formation.kind_at(index),
            };
            state.set(to_global(owner, u, v), Some(piece));
        }
    }
    if let Some(first) = state.next_alive_after(SEATS_U8 - 1) {
        state.to_move = first;
        state.seats[usize::from(first)].own_turns = 1;
    }
    state
}

/// The wire snapshot of a state. `left_ms` is the displayed clock.
#[must_use]
pub fn to_state(state: &State) -> BoardState {
    let pieces = state
        .board
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let p = (*p)?;
            let t = Tile::from_index(i)?;
            Some(PieceState {
                id: p.id,
                owner: p.owner,
                kind: p.kind,
                x: t.x,
                y: t.y,
            })
        })
        .collect();
    let seats = state
        .seats
        .iter()
        .enumerate()
        .map(|(i, s)| SeatState {
            seat: u8::try_from(i).unwrap_or(u8::MAX),
            present: s.present,
            alive: s.alive,
            garrison: s.garrison,
            own_turns: s.own_turns,
            timeouts: s.timeouts,
            captured: s.captured.clone(),
        })
        .collect();
    BoardState {
        turn: state.turn,
        seat: state.to_move,
        left_ms: state.clock.display_left_ms(),
        quiet: state.quiet,
        stalls: state.stalls,
        pieces,
        seats,
        last: state.last,
    }
}

/// Why a `BoardState` could not become a `State`; `reason()` is readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidBoard {
    /// A piece stands off the board.
    PieceOffBoard,
    /// Two pieces share a tile.
    TwoPiecesOnOneTile,
    /// A piece belongs to a seat above 3.
    OwnerOutOfRange,
    /// The seat list is not exactly the four seats in order.
    SeatsNotFour,
    /// The seat to move is above 3.
    ToMoveOutOfRange,
}

impl InvalidBoard {
    /// A readable description.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::PieceOffBoard => "a piece stands off the board",
            Self::TwoPiecesOnOneTile => "two pieces share a tile",
            Self::OwnerOutOfRange => "a piece belongs to a seat that does not exist",
            Self::SeatsNotFour => "the board does not list the four seats in order",
            Self::ToMoveOutOfRange => "the seat to move does not exist",
        }
    }
}

impl std::fmt::Display for InvalidBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

impl std::error::Error for InvalidBoard {}

/// A `State` from a wire snapshot. The inverse of `to_state`: bit-identical
/// for every state whose clock had not yet passed the displayed deadline
/// (the grace window is not on the wire) and whose `result` is `None` (the
/// result travels in `Phase`, not in the board).
///
/// # Errors
/// A snapshot that describes an impossible board.
pub fn from_state(board: &BoardState) -> Result<State, InvalidBoard> {
    if board.seats.len() != SEATS
        || board
            .seats
            .iter()
            .enumerate()
            .any(|(i, s)| usize::from(s.seat) != i)
    {
        return Err(InvalidBoard::SeatsNotFour);
    }
    if usize::from(board.seat) >= SEATS {
        return Err(InvalidBoard::ToMoveOutOfRange);
    }
    let mut tiles = [None; TILES];
    for p in &board.pieces {
        let t = Tile::new(p.x, p.y).ok_or(InvalidBoard::PieceOffBoard)?;
        if usize::from(p.owner) >= SEATS {
            return Err(InvalidBoard::OwnerOutOfRange);
        }
        let slot = &mut tiles[t.index()];
        if slot.is_some() {
            return Err(InvalidBoard::TwoPiecesOnOneTile);
        }
        *slot = Some(Piece {
            id: p.id,
            owner: p.owner,
            kind: p.kind,
        });
    }
    let seats = std::array::from_fn(|i| {
        let s = &board.seats[i];
        Seat {
            present: s.present,
            alive: s.alive,
            garrison: s.garrison,
            own_turns: s.own_turns,
            timeouts: s.timeouts,
            captured: s.captured.clone(),
        }
    });
    Ok(State {
        board: tiles,
        seats,
        to_move: board.seat,
        turn: board.turn,
        quiet: board.quiet,
        stalls: board.stalls,
        clock: TurnClock::from_display_left_ms(board.left_ms),
        last: board.last,
        result: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::ActionKind;

    const K: Kind = Kind::King;
    const Q: Kind = Kind::Queen;
    const R: Kind = Kind::Rook;
    const B: Kind = Kind::Bishop;
    const N: Kind = Kind::Knight;
    const H: Kind = Kind::Hero;
    const J: Kind = Kind::Joker;
    const P: Kind = Kind::Pawn;

    /// The global table of section 1.3, pinned literally: for each setup
    /// index, the kind and the tile of seats 0, 1, 2, 3.
    const SETUP_TABLE: [(Kind, [(u8, u8); 4]); 16] = [
        (K, [(0, 0), (9, 0), (9, 9), (0, 9)]),
        (Q, [(1, 0), (9, 1), (8, 9), (0, 8)]),
        (H, [(0, 1), (8, 0), (9, 8), (1, 9)]),
        (J, [(1, 1), (8, 1), (8, 8), (1, 8)]),
        (R, [(2, 0), (9, 2), (7, 9), (0, 7)]),
        (B, [(2, 1), (8, 2), (7, 8), (1, 7)]),
        (B, [(2, 2), (7, 2), (7, 7), (2, 7)]),
        (N, [(1, 2), (7, 1), (8, 7), (2, 8)]),
        (R, [(0, 2), (7, 0), (9, 7), (2, 9)]),
        (P, [(3, 0), (9, 3), (6, 9), (0, 6)]),
        (P, [(3, 1), (8, 3), (6, 8), (1, 6)]),
        (P, [(3, 2), (7, 3), (6, 7), (2, 6)]),
        (P, [(3, 3), (6, 3), (6, 6), (3, 6)]),
        (P, [(2, 3), (6, 2), (7, 6), (3, 7)]),
        (P, [(1, 3), (6, 1), (8, 6), (3, 8)]),
        (P, [(0, 3), (6, 0), (9, 6), (3, 9)]),
    ];

    fn full() -> State {
        setup([true; 4], [Formation::DEFAULT; 4])
    }

    fn in_cross(t: Tile) -> bool {
        (4..=5).contains(&t.x) || (4..=5).contains(&t.y)
    }

    #[test]
    fn setup_counts() {
        let state = full();
        assert_eq!(state.board.iter().flatten().count(), 64);
        for seat in 0..4 {
            let pieces: Vec<_> = state.pieces_of(seat).collect();
            assert_eq!(pieces.len(), 16, "seat {seat}");
            for (t, p) in &pieces {
                assert!(in_home_block(seat, *t), "seat {seat} piece {p:?} at {t:?}");
                assert!(!in_cross(*t));
                assert_eq!(p.id / PIECES_PER_SEAT, seat);
            }
            let ids: std::collections::BTreeSet<u8> = pieces.iter().map(|(_, p)| p.id).collect();
            assert_eq!(ids.len(), 16, "ids unique within seat {seat}");
        }
        let cross_occupied = Tile::all()
            .filter(|t| in_cross(*t) && state.piece(*t).is_some())
            .count();
        assert_eq!(cross_occupied, 0, "the neutral cross starts empty");
        assert_eq!(Tile::all().filter(|t| in_cross(*t)).count(), 36);
        assert_eq!(state.to_move, 0);
        assert_eq!(state.turn, 1);
        assert_eq!(state.seats[0].own_turns, 1);
        assert_eq!(state.seats[1].own_turns, 0);
        assert_eq!(state.clock, TurnClock::new());
        assert_eq!(state.result, None);
    }

    #[test]
    fn setup_is_four_rotations() {
        let state = full();
        let tile_of = |seat: u8, index: u8| {
            state
                .pieces_of(seat)
                .find(|(_, p)| p.id == seat * PIECES_PER_SEAT + index)
                .map(|(t, _)| t)
                .expect("piece present")
        };
        for index in 0..PIECES_PER_SEAT {
            for seat in 0..4u8 {
                let next = (seat + 1) % 4;
                assert_eq!(
                    tile_of(next, index),
                    rot(tile_of(seat, index)),
                    "index {index}: seat {next} is rot(seat {seat})"
                );
            }
            assert_eq!(tile_of(0, index), rot(tile_of(3, index)));
        }
    }

    #[test]
    fn setup_matches_the_tables() {
        let state = full();
        let mut seen = 0;
        for (index, (kind, tiles)) in SETUP_TABLE.iter().enumerate() {
            for (seat, &(x, y)) in tiles.iter().enumerate() {
                let t = Tile::at(x, y);
                let p = state.piece(t).unwrap_or_else(|| panic!("empty {t:?}"));
                assert_eq!(p.kind, *kind, "seat {seat} index {index} at {t:?}");
                assert_eq!(usize::from(p.owner), seat);
                assert_eq!(usize::from(p.id), seat * 16 + index);
                seen += 1;
            }
        }
        assert_eq!(seen, 64);
        // The local table too, through the transform.
        for (index, &(u, v)) in SETUP_LOCAL.iter().enumerate() {
            for seat in 0..4u8 {
                let t = to_global(seat, u, v);
                assert_eq!(
                    (t.x, t.y),
                    SETUP_TABLE[index].1[seat as usize],
                    "seat {seat} index {index}"
                );
            }
        }
    }

    #[test]
    fn frames_round_trip() {
        for seat in 0..4u8 {
            for u in 0..SIDE {
                for v in 0..SIDE {
                    let t = to_global(seat, u, v);
                    assert_eq!(to_local(seat, t), (u, v), "seat {seat} ({u},{v})");
                    assert_eq!(
                        to_global((seat + 1) % 4, u, v),
                        rot(t),
                        "seat {seat} ({u},{v}): next seat is rot"
                    );
                }
            }
        }
        // Every global tile has exactly one local pre-image per seat.
        for seat in 0..4u8 {
            let mut hit = [false; TILES];
            for u in 0..SIDE {
                for v in 0..SIDE {
                    hit[to_global(seat, u, v).index()] = true;
                }
            }
            assert!(hit.iter().all(|h| *h), "seat {seat} covers the board");
        }
    }

    #[test]
    fn frame_vectors_pinned() {
        // (corner, f, l, f+l, f-l, -f+l) per seat, section 1.2.
        let table = [
            ((0, 0), (1, 0), (0, 1), (1, 1), (1, -1), (-1, 1)),
            ((9, 0), (0, 1), (-1, 0), (-1, 1), (1, 1), (-1, -1)),
            ((9, 9), (-1, 0), (0, -1), (-1, -1), (-1, 1), (1, -1)),
            ((0, 9), (0, -1), (1, 0), (1, -1), (-1, -1), (1, 1)),
        ];
        for (seat, row) in table.iter().enumerate() {
            let seat = u8::try_from(seat).unwrap();
            let fr = frame(seat);
            let d = |(dx, dy): (i8, i8)| Dir::new(dx, dy);
            assert_eq!((fr.corner.x, fr.corner.y), row.0, "seat {seat} corner");
            assert_eq!(fr.f, d(row.1), "seat {seat} f");
            assert_eq!(fr.l, d(row.2), "seat {seat} l");
            assert_eq!(front_left(seat), d(row.3), "seat {seat} f+l");
            assert_eq!(fr.f.sub(fr.l), d(row.4), "seat {seat} f-l");
            assert_eq!(fr.f.neg().add(fr.l), d(row.5), "seat {seat} -f+l");
            assert_eq!(Dir::in_frame(1, 1, fr), d(row.3));
            assert_eq!(Dir::in_frame(1, -1, fr), d(row.4));
            assert_eq!(Dir::in_frame(-1, 1, fr), d(row.5));
            // Rotations, never reflections: l is f turned 90 degrees CCW.
            assert_eq!((fr.l.dx, fr.l.dy), (-fr.f.dy, fr.f.dx), "seat {seat} handedness");
            // Forward points at the next seat's corner.
            let next = frame((seat + 1) % 4).corner;
            assert_eq!(
                fr.corner.offset(Dir::new(fr.f.dx * 9, fr.f.dy * 9)),
                Some(next),
                "seat {seat} forward reaches the next corner"
            );
        }
    }

    #[test]
    fn front_left_on_board_from_every_home_tile() {
        for seat in 0..4u8 {
            for u in 0..=HOME_MAX {
                for v in 0..=HOME_MAX {
                    let t = to_global(seat, u, v);
                    let fl = t.offset(front_left(seat));
                    assert!(fl.is_some(), "seat {seat} ({u},{v})");
                    assert_eq!(to_local(seat, fl.unwrap()), (u + 1, v + 1));
                }
            }
        }
    }

    #[test]
    fn front_left_on_board_from_every_start_mirror() {
        // Section 1.3's pinned list: (seat, mirror tile, its front-left).
        let pinned = [
            (0, (8, 1), (9, 2)),
            (0, (1, 8), (2, 9)),
            (0, (8, 8), (9, 9)),
            (1, (1, 1), (0, 2)),
            (1, (8, 8), (7, 9)),
            (1, (1, 8), (0, 9)),
            (2, (1, 8), (0, 7)),
            (2, (8, 1), (7, 0)),
            (2, (1, 1), (0, 0)),
            (3, (8, 8), (9, 7)),
            (3, (1, 1), (2, 0)),
            (3, (8, 1), (9, 0)),
        ];
        for seat in 0..4u8 {
            let start = to_global(seat, 1, 1);
            for m in mirrors(start) {
                let fl = m.offset(front_left(seat));
                assert!(fl.is_some(), "seat {seat} mirror {m:?}");
                let fl = fl.unwrap();
                assert!(
                    pinned.contains(&(seat, (m.x, m.y), (fl.x, fl.y))),
                    "seat {seat} mirror {m:?} -> {fl:?} is not in the doc's list"
                );
            }
            // And from the start tile itself: the seat's own second bishop.
            let fl = start.offset(front_left(seat)).unwrap();
            assert_eq!((fl.x, fl.y), SETUP_TABLE[6].1[seat as usize]);
        }
        assert_eq!(pinned.len(), 12);
    }

    #[test]
    fn promotion_line_is_a_formula() {
        for seat in 0..4u8 {
            for t in Tile::all() {
                let (u, v) = to_local(seat, t);
                assert_eq!(
                    on_far_edge(seat, t),
                    u == 9 || v == 9,
                    "seat {seat} tile {t:?} local ({u},{v})"
                );
            }
            assert_eq!(Tile::all().filter(|t| on_far_edge(seat, *t)).count(), 19);
        }
    }

    #[test]
    fn mirrors_are_involutions() {
        for t in Tile::all() {
            for (i, m) in [mirror_row, mirror_col, mirror_centre].iter().enumerate() {
                assert_eq!(m(m(t)), t, "mirror {i} of {t:?}");
                assert_ne!(m(t), t, "mirror {i} of {t:?} is a fixed point");
            }
            let ms = mirrors(t);
            assert_eq!(ms, [mirror_row(t), mirror_col(t), mirror_centre(t)]);
            assert_ne!(ms[0], ms[1]);
            assert_ne!(ms[1], ms[2]);
            assert_ne!(ms[0], ms[2]);
        }
        assert_eq!(mirrors(Tile::at(1, 1)), [Tile::at(8, 1), Tile::at(1, 8), Tile::at(8, 8)]);
        assert_eq!(rot(rot(rot(rot(Tile::at(3, 7))))), Tile::at(3, 7));
    }

    #[test]
    fn bishops_start_on_opposite_colours() {
        let state = full();
        for seat in 0..4u8 {
            let colours: Vec<u8> = state
                .pieces_of(seat)
                .filter(|(_, p)| p.kind == Kind::Bishop)
                .map(|(t, _)| t.colour())
                .collect();
            assert_eq!(colours.len(), 2, "seat {seat}");
            assert_ne!(colours[0], colours[1], "seat {seat}");
        }
        // The doc's own parities.
        assert_eq!(Tile::at(2, 1).colour(), 1);
        assert_eq!(Tile::at(2, 2).colour(), 0);
        assert_eq!(Tile::at(8, 2).colour(), 0);
        assert_eq!(Tile::at(7, 2).colour(), 1);
    }

    #[test]
    fn formation_validator() {
        assert_eq!(Formation::DEFAULT.validate(), Ok(()));
        assert_eq!(Formation::default(), Formation::DEFAULT);
        let f = |legend: [Kind; 4], epic: [Kind; 5]| Formation { legend, epic };
        let default_epic = Formation::DEFAULT.epic;
        // Any legend permutation.
        assert_eq!(f([J, H, Q, K], default_epic).validate(), Ok(()));
        assert_eq!(f([Q, K, J, H], default_epic).validate(), Ok(()));
        // Bishops on (2,0) and (2,2): both even.
        assert_eq!(
            f([K, Q, H, J], [B, R, B, R, N]).validate(),
            Err(FormationError::BishopsOnOneColour)
        );
        // Bishops on (2,1) and (1,2): both odd.
        assert_eq!(
            f([K, Q, H, J], [R, B, R, B, N]).validate(),
            Err(FormationError::BishopsOnOneColour)
        );
        // Bishops on (2,0) and (0,2): both even.
        assert_eq!(
            f([K, Q, H, J], [B, R, R, N, B]).validate(),
            Err(FormationError::BishopsOnOneColour)
        );
        // Bishops on (2,0) even and (1,2) odd: accepted (the doc's
        // "[B,R,R,B,N]-style" example is in fact an opposite-colour pair).
        assert_eq!(f([K, Q, H, J], [B, R, R, B, N]).validate(), Ok(()));
        // The knight on the ring elbow.
        assert_eq!(f([K, Q, H, J], [R, B, N, B, R]).validate(), Ok(()));
        // Wrong multisets.
        assert_eq!(
            f([K, K, H, J], default_epic).validate(),
            Err(FormationError::LegendNotAPermutation)
        );
        assert_eq!(
            f([K, Q, H, R], default_epic).validate(),
            Err(FormationError::LegendNotAPermutation)
        );
        assert_eq!(
            f([K, Q, H, J], [R, R, R, B, N]).validate(),
            Err(FormationError::EpicNotAPermutation)
        );
        assert_eq!(
            f([K, Q, H, J], [R, B, B, N, N]).validate(),
            Err(FormationError::EpicNotAPermutation)
        );
        assert_eq!(
            f([K, Q, H, J], [R, B, B, N, Q]).validate(),
            Err(FormationError::EpicNotAPermutation)
        );
        // Legend is checked before epic, and the reasons read.
        assert_eq!(
            f([K, K, H, J], [R, R, R, B, N]).validate(),
            Err(FormationError::LegendNotAPermutation)
        );
        for e in [
            FormationError::LegendNotAPermutation,
            FormationError::EpicNotAPermutation,
            FormationError::BishopsOnOneColour,
        ] {
            assert!(!e.reason().is_empty());
            assert_eq!(e.to_string(), e.reason());
        }
        // kind_at covers the three classes.
        assert_eq!(Formation::DEFAULT.kind_at(0), K);
        assert_eq!(Formation::DEFAULT.kind_at(4), R);
        assert_eq!(Formation::DEFAULT.kind_at(8), R);
        assert_eq!(Formation::DEFAULT.kind_at(9), P);
        assert_eq!(Formation::DEFAULT.kind_at(15), P);
    }

    #[test]
    fn seat_by_join() {
        assert_eq!(SEAT_BY_JOIN, [0, 2, 1, 3]);
        let a = frame(SEAT_BY_JOIN[0]).corner;
        let b = frame(SEAT_BY_JOIN[1]).corner;
        assert_eq!(mirror_centre(a), b, "two players sit diagonally");
        let mut sorted = SEAT_BY_JOIN;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3]);
        // A two-player setup through the table: seats 0 and 2 alive.
        let mut present = [false; 4];
        for &seat in &SEAT_BY_JOIN[..2] {
            present[seat as usize] = true;
        }
        let state = setup(present, [Formation::DEFAULT; 4]);
        assert_eq!(state.alive_count(), 2);
        assert!(state.seats[0].alive && state.seats[2].alive);
        assert!(state.seats[1].garrison && state.seats[3].garrison);
        assert!(!state.seats[1].present);
        assert_eq!(state.board.iter().flatten().count(), 64);
    }

    #[test]
    fn to_state_from_state_round_trip() {
        let mut state = setup([true, false, true, true], [Formation::DEFAULT; 4]);
        state.seats[0].captured = vec![Kind::Pawn, Kind::Rook, Kind::King];
        state.seats[2].captured = vec![Kind::Joker];
        state.seats[3].timeouts = 2;
        state.seats[2].own_turns = 7;
        state.turn = 23;
        state.quiet = 5;
        state.stalls = 1;
        state.to_move = 2;
        state.clock.tick(6_600);
        state.last = Some(LastAction {
            seat: 3,
            kind: ActionKind::JokerTeleport,
            fx: 1,
            fy: 8,
            tx: 8,
            ty: 8,
            captured: Some(Kind::Joker),
            promoted: false,
            eliminated: Some(1),
        });
        // Move a few pieces so the board is not the setup.
        let pawn = state.piece(Tile::at(3, 0));
        state.set(Tile::at(3, 0), None);
        state.set(Tile::at(5, 5), pawn);
        state.set(Tile::at(9, 9), None);

        let wire = to_state(&state);
        assert_eq!(wire.pieces.len(), 63);
        assert_eq!(wire.seats.len(), 4);
        assert_eq!(wire.left_ms, TurnClock::new().display_left_ms() - 6_600);
        let back = from_state(&wire).expect("valid board");
        assert_eq!(back, state);

        // Through JSON as well, which is the path the client takes.
        let json = serde_json::to_string(&wire).unwrap();
        let wire2: BoardState = serde_json::from_str(&json).unwrap();
        assert_eq!(from_state(&wire2).unwrap(), state);

        // A fresh setup round-trips too.
        let fresh = full();
        assert_eq!(from_state(&to_state(&fresh)).unwrap(), fresh);
    }

    #[test]
    fn from_state_refuses_impossible_boards() {
        let good = to_state(&full());
        let mut off = good.clone();
        off.pieces[0].x = 10;
        assert_eq!(from_state(&off), Err(InvalidBoard::PieceOffBoard));
        let mut dup = good.clone();
        dup.pieces[1].x = dup.pieces[0].x;
        dup.pieces[1].y = dup.pieces[0].y;
        assert_eq!(from_state(&dup), Err(InvalidBoard::TwoPiecesOnOneTile));
        let mut owner = good.clone();
        owner.pieces[0].owner = 4;
        assert_eq!(from_state(&owner), Err(InvalidBoard::OwnerOutOfRange));
        let mut seats = good.clone();
        seats.seats.pop();
        assert_eq!(from_state(&seats), Err(InvalidBoard::SeatsNotFour));
        let mut order = good.clone();
        order.seats.swap(0, 1);
        assert_eq!(from_state(&order), Err(InvalidBoard::SeatsNotFour));
        let mut mover = good;
        mover.seat = 4;
        assert_eq!(from_state(&mover), Err(InvalidBoard::ToMoveOutOfRange));
        assert_eq!(
            InvalidBoard::TwoPiecesOnOneTile.to_string(),
            "two pieces share a tile"
        );
    }

    #[test]
    fn tiles_and_dirs() {
        assert_eq!(Tile::new(9, 9), Some(Tile::at(9, 9)));
        assert_eq!(Tile::new(10, 0), None);
        assert_eq!(Tile::new(0, 10), None);
        assert_eq!(Tile::at(3, 7).index(), 73);
        assert_eq!(Tile::from_index(73), Some(Tile::at(3, 7)));
        assert_eq!(Tile::from_index(100), None);
        assert_eq!(Tile::all().count(), 100);
        assert_eq!(Tile::at(0, 0).offset(Dir::new(-1, 0)), None);
        assert_eq!(Tile::at(9, 9).offset(Dir::new(0, 1)), None);
        assert_eq!(Tile::at(4, 4).offset(Dir::new(2, -1)), Some(Tile::at(6, 3)));
        assert_eq!(ALL8.len(), 8);
        assert_eq!(KNIGHT8.len(), 8);
        for d in ALL8 {
            assert!(ORTHO4.contains(&d) ^ DIAG4.contains(&d));
        }
        let state = full();
        assert_eq!(state.next_alive_after(3), Some(0));
        assert_eq!(state.next_alive_after(0), Some(1));
        assert_eq!(state.sole_survivor(), None);
        let two = setup([true, false, true, false], [Formation::DEFAULT; 4]);
        assert_eq!(two.next_alive_after(0), Some(2));
        assert_eq!(two.next_alive_after(2), Some(0));
        assert!(two.has_pawn(1), "garrison pawns count as pawns of seat 1");
        let none = setup([false; 4], [Formation::DEFAULT; 4]);
        assert_eq!(none.next_alive_after(0), None);
        assert_eq!(none.to_move, 0);
        assert_eq!(none.seats[0].own_turns, 0);
    }
}
