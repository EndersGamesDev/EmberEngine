//! The Four Kings wire protocol: JSON over WebSocket.
//!
//! Its own `PROTO_VERSION`, independent of `pong_core` and `fire_core`, for
//! the reason `fire_core::proto` gives: the join gate is exact equality, and
//! a board-game message must never be able to make the arena or the racer
//! list-only. Same house style: `#[serde(tag = "t", rename_all =
//! "snake_case")]`, JSON text frames, `#[serde(default)]` on anything added
//! after v1, and for every defaulted field a comment saying what an old peer
//! DOES when it is absent.
//!
//! The Rust below is section 4.5 of `docs/kings-design.md` as written there,
//! plus the derives the tests need (`Eq` everywhere the fields allow it,
//! `PartialEq` on the message enums), documentation on every item, and the
//! two helpers copied from `fire-core` at the bottom.

use serde::{Deserialize, Serialize};

/// Kings' own protocol version.
pub const PROTO_VERSION: u16 = 1;

/// Longest player handle kept after `sanitize`.
pub const MAX_HANDLE_LEN: usize = 20;
/// Longest lobby name kept after `sanitize`.
pub const MAX_LOBBY_LEN: usize = 24;
/// Longest lobby password kept after `sanitize`.
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
/// The server drops a peer that has been silent longer than this.
pub const CLIENT_TIMEOUT_SECS: u64 = 30;
/// A single frame larger than this is a protocol violation, not a big board.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

/// A piece kind. The kind is the whole of a piece's rule state: a dormant
/// and an awakened hero are two kinds, a promoted pawn is a queen.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Steps one tile in any of the eight directions.
    King,
    /// Slides in any of the eight directions.
    Queen,
    /// Slides orthogonally.
    Rook,
    /// Slides diagonally.
    Bishop,
    /// Jumps in the eight knight offsets.
    Knight,
    /// Marches one tile forward or left; captures on three diagonals.
    Pawn,
    /// Teleports and is placed; captures only on its owner's front-left.
    Joker,
    /// Dormant: cannot move or capture; its only moves are the swap onto
    /// an own pawn and, with no pawns left, awakening in place.
    Hero,
    /// Awakened: moves and captures as rook and knight combined.
    HeroAwake,
}

/// Where a lobby is in its life.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// In the lobby; the board shows the setup for the seats held so far.
    Waiting,
    /// A game is running; nobody may join.
    Playing,
    /// The result is being shown; the lobby returns to Waiting after
    /// `RESULTS_SECS`.
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
    /// The joker moved to one of its three mirror tiles.
    JokerTeleport,
    /// The joker was placed on a free tile on one of its owner's fifth turns.
    JokerPlace,
    /// The dormant hero took an own pawn's place and awakened.
    HeroSwap,
    /// Hero awakened in place (no pawn left to swap with).
    HeroWake,
    /// A forced pass: the seat had no legal move. Never a timeout.
    Pass,
    /// A pass the server made because the clock ran out.
    Timeout,
}

/// One row of the lobby browser.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LobbyInfo {
    /// Lobby name, sanitized.
    pub name: String,
    /// Handle of the creator.
    pub host: String,
    /// Whether joining needs a password.
    pub has_password: bool,
    /// Humans seated right now.
    pub players: u8,
    /// Seats in total (`MAX_PLAYERS`).
    pub cap: u8,
    /// True once the game has started; joining is refused until it resets.
    /// An old peer that never learned the field lists the lobby as joinable
    /// and has its join refused with the reason, which is the same outcome
    /// one message later.
    #[serde(default)]
    pub playing: bool,
}

/// One member of a lobby, as seen in `Roster`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerMeta {
    /// Lobby-local id, stable while the member stays.
    pub id: u8,
    /// Handle, sanitized.
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
    /// Kinds on the corner 2x2, a permutation of King, Queen, Hero, Joker.
    pub legend: [Kind; 4],
    /// Kinds on tier 1, a permutation of Rook, Rook, Bishop, Bishop, Knight
    /// with the bishops on opposite colours.
    pub epic: [Kind; 5],
}

/// One piece in a board broadcast.
///
/// Flat scalars: the rules crate's `Piece` can be refactored without that
/// being a protocol question.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceState {
    /// `seat * 16 + setup index`; stable for the whole game.
    pub id: u8,
    /// Owning seat.
    pub owner: u8,
    /// Current kind (promotion and awakening change it, never the id).
    pub kind: Kind,
    /// Column, 0 at the west edge.
    pub x: u8,
    /// Row, 0 at the south edge.
    pub y: u8,
}

/// Per-corner bookkeeping.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SeatState {
    /// Corner index, 0 = SW, counter-clockwise.
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

/// The last applied action, for narration.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastAction {
    /// The seat that acted (or passed, or timed out).
    pub seat: u8,
    /// What the action was.
    pub kind: ActionKind,
    /// From/to tiles; for a pass or timeout all four are 0 and the page
    /// ignores them.
    pub fx: u8,
    /// From row.
    pub fy: u8,
    /// To column.
    pub tx: u8,
    /// To row.
    pub ty: u8,
    /// The foreign piece taken, if any.
    pub captured: Option<Kind>,
    /// The mover was a pawn that became a queen.
    pub promoted: bool,
    /// A seat eliminated by this action (king captured, third timeout,
    /// disconnect).
    pub eliminated: Option<u8>,
}

/// The whole board.
///
/// Sent in full on every change: at 64 pieces it is a few kilobytes, and a
/// full snapshot cannot get out of step the way a stream of deltas can (the
/// racer's one-shot events already showed what a missed message costs).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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
    /// Every piece on the board, in tile-index order.
    pub pieces: Vec<PieceState>,
    /// Always four entries, indexed by seat.
    pub seats: Vec<SeatState>,
    /// The last action, for narration. An old peer that never learned the
    /// field simply narrates nothing; the board it shows is the same.
    #[serde(default)]
    pub last: Option<LastAction>,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum C2S {
    /// Must be the first message on a connection.
    Hello {
        /// The client's `PROTO_VERSION`; the gate is exact equality.
        proto: u16,
        /// Display name, sanitized by the server.
        handle: String,
    },
    /// Ask for the lobby list; ungated, so a browser may send `proto: 0`.
    ListLobbies,
    /// Create a lobby and sit at seat 0 as its creator.
    CreateLobby {
        /// Lobby name.
        name: String,
        /// Optional password.
        password: Option<String>,
    },
    /// Join an existing Waiting lobby.
    JoinLobby {
        /// Lobby name.
        name: String,
        /// Password, if the lobby has one.
        password: Option<String>,
    },
    /// Leave the current lobby (a mid-game leave is an elimination).
    LeaveLobby,
    /// Waiting only. Validated as a within-class permutation with the two
    /// bishops on opposite colours; the Waiting board is rebuilt and
    /// broadcast. Invalid: `Rejected`, formation unchanged.
    SetFormation {
        /// The requested formation.
        formation: Formation,
    },
    /// Creator only, Waiting only, at least `MIN_PLAYERS` seated. Anyone
    /// else gets `Rejected` with the reason.
    Start,
    /// The one action shape. Ordinary moves, the joker's step, teleport
    /// (a mirror tile), placement (any empty tile on own turns 5, 10, ...)
    /// and front-left capture, the hero's swap (`to` = own pawn) and its
    /// awakening in place (`to == from`, only with no pawns). `turn` must
    /// equal the current `BoardState::turn`.
    Move {
        /// The turn this move was computed against.
        turn: u32,
        /// From column.
        fx: u8,
        /// From row.
        fy: u8,
        /// To column.
        tx: u8,
        /// To row.
        ty: u8,
    },
    /// Liveness. There is no input stream, so this is the keepalive.
    Ping {
        /// Echoed back in `Pong`.
        nonce: u32,
    },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    /// Reply to a valid Hello. The identity fields follow `docs/hosts.md`
    /// and are informational: a peer that ignores them plays the same game.
    Welcome {
        /// The server's `PROTO_VERSION`.
        proto: u16,
        /// Host name, `""` when the server was started without one.
        #[serde(default)]
        host: String,
        /// `r<N>` of the build, `""` for an unstamped dev build.
        #[serde(default)]
        version: String,
        /// Short commit hash of the build, `""` when unstamped.
        #[serde(default)]
        commit: String,
        /// Connected players, informational.
        #[serde(default)]
        players: u32,
        /// Open lobbies, informational.
        #[serde(default)]
        lobbies: u32,
    },
    /// A refused Hello, create, join, formation, start or move. The
    /// connection stays open; a refused move leaves the board as it was.
    Rejected {
        /// What the player reads.
        reason: String,
    },
    /// The lobby list.
    Lobbies {
        /// One row per lobby.
        lobbies: Vec<LobbyInfo>,
    },
    /// Reply to a successful create or join; followed by `Roster`, `State`
    /// and `Phase` so the joiner sees the table immediately.
    Joined {
        /// The lobby's name.
        lobby: String,
        /// The joiner's lobby-local id.
        id: u8,
    },
    /// The full roster with seats, on every join, leave, re-seat or creator
    /// handover. The client finds itself by `id`.
    Roster {
        /// Id of the creator.
        creator: u8,
        /// Every member with its seat.
        roster: Vec<PlayerMeta>,
    },
    /// The spec's start notification: to the creator only, whenever the
    /// lobby holds at least `MIN_PLAYERS` and on every roster change while
    /// it does.
    CanStart {
        /// Humans seated.
        players: u8,
    },
    /// `winner` is the seat that won; `None` outside Finished or for a draw.
    /// `end` says why a Finished game ended.
    Phase {
        /// The lobby's phase.
        phase: Phase,
        /// Winning seat. An old peer that never learned the field shows
        /// Finished without a winner, a poorer banner over the same game.
        #[serde(default)]
        winner: Option<u8>,
        /// Why a Finished game ended. Absent for an old peer: the same
        /// degraded banner as `winner`.
        #[serde(default)]
        end: Option<EndReason>,
    },
    /// The full board, on every change and on join.
    State {
        /// The snapshot.
        board: BoardState,
    },
    /// Once a second while Playing: whose turn, and how long is left.
    Clock {
        /// Current turn number.
        turn: u32,
        /// Seat to move.
        seat: u8,
        /// Milliseconds left on the displayed clock.
        left_ms: u32,
    },
    /// Reply to `Ping`.
    Pong {
        /// The client's nonce.
        nonce: u32,
    },
}

/// Windows `ERROR_IO_PENDING` / `WSA_IO_PENDING`.
///
/// A socket with `SO_RCVTIMEO` set does not always report a timed-out read as
/// `WSAETIMEDOUT`. When the read times out part-way through an overlapped
/// operation Windows returns 997 instead, and Rust has no `ErrorKind` for it:
/// `kind()` is `Uncategorized`, so it matches neither `WouldBlock` nor
/// `TimedOut`.
const WINDOWS_IO_PENDING: i32 = 997;

/// True when a read error means "nothing to read yet", not "this connection is
/// finished".
///
/// Copied from `fire-core`, where the doc comment records the three
/// unrelated-looking symptoms a wrong answer produced. Every read loop in
/// these games is a short-timeout poll, and the loops treat anything else as
/// fatal, so a misclassified transient error kills the reader permanently.
#[must_use]
pub fn is_transient_read(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted
    ) || e.raw_os_error() == Some(WINDOWS_IO_PENDING)
}

/// Names and handles arrive from the network.
///
/// Strip control characters and cap the length, or a peer can write terminal
/// escapes into the server's log and the other players' lobby lists.
#[must_use]
pub fn sanitize(s: &str, max: usize) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).take(max).collect();
    if cleaned.trim().is_empty() {
        String::new()
    } else {
        cleaned
    }
}

/// `sanitize` for a handle, with a fallback so nobody is nameless.
#[must_use]
pub fn sanitize_handle(s: &str) -> String {
    let h = sanitize(s, MAX_HANDLE_LEN);
    if h.is_empty() {
        "player".to_string()
    } else {
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{setup, to_state};

    fn roundtrip<T: Serialize + serde::de::DeserializeOwned + std::fmt::Debug>(v: &T) -> T {
        let s = serde_json::to_string(v).expect("encode");
        assert!(s.len() < MAX_FRAME_BYTES, "frame too large");
        serde_json::from_str(&s).expect("decode")
    }

    fn sample_board() -> BoardState {
        let mut state = setup([true, false, true, true], [Formation::DEFAULT; 4]);
        state.seats[0].captured = vec![Kind::Pawn, Kind::Rook];
        state.last = Some(LastAction {
            seat: 0,
            kind: ActionKind::Move,
            fx: 3,
            fy: 0,
            tx: 4,
            ty: 0,
            captured: None,
            promoted: false,
            eliminated: None,
        });
        to_state(&state)
    }

    /// The tag discipline is what lets both ends match on `t`. If the casing
    /// or the tag key drifts, every message becomes an unknown variant.
    #[test]
    fn the_wire_shape_is_the_house_style() {
        let s = serde_json::to_string(&C2S::ListLobbies).unwrap();
        assert_eq!(s, r#"{"t":"list_lobbies"}"#);
        let s = serde_json::to_string(&S2C::Pong { nonce: 7 }).unwrap();
        assert_eq!(s, r#"{"t":"pong","nonce":7}"#);
        let s = serde_json::to_string(&S2C::Phase {
            phase: Phase::Finished,
            winner: Some(2),
            end: Some(EndReason::NoProgress),
        })
        .unwrap();
        assert_eq!(
            s,
            r#"{"t":"phase","phase":"finished","winner":2,"end":"no_progress"}"#
        );
        let s = serde_json::to_string(&C2S::Move {
            turn: 3,
            fx: 3,
            fy: 0,
            tx: 4,
            ty: 0,
        })
        .unwrap();
        assert_eq!(s, r#"{"t":"move","turn":3,"fx":3,"fy":0,"tx":4,"ty":0}"#);
        let s = serde_json::to_string(&Kind::HeroAwake).unwrap();
        assert_eq!(s, r#""hero_awake""#);
        let s = serde_json::to_string(&ActionKind::JokerTeleport).unwrap();
        assert_eq!(s, r#""joker_teleport""#);
    }

    /// The struct keys the page reads raw off the socket (a `LobbyInfo` row
    /// in the lobby browser, the board's pieces, seats and last action),
    /// pinned in the ENCODE direction: a field rename here would pass every
    /// round-trip test and break the page.
    #[test]
    fn the_struct_keys_are_pinned() {
        let s = serde_json::to_string(&LobbyInfo {
            name: "court".into(),
            host: "ada".into(),
            has_password: true,
            players: 2,
            cap: 4,
            playing: false,
        })
        .unwrap();
        assert_eq!(
            s,
            r#"{"name":"court","host":"ada","has_password":true,"players":2,"cap":4,"playing":false}"#
        );
        let s = serde_json::to_string(&PlayerMeta {
            id: 1,
            handle: "bob".into(),
            seat: 2,
        })
        .unwrap();
        assert_eq!(s, r#"{"id":1,"handle":"bob","seat":2}"#);
        let s = serde_json::to_string(&PieceState {
            id: 3,
            owner: 0,
            kind: Kind::Joker,
            x: 1,
            y: 1,
        })
        .unwrap();
        assert_eq!(s, r#"{"id":3,"owner":0,"kind":"joker","x":1,"y":1}"#);
        let s = serde_json::to_string(&SeatState {
            seat: 0,
            present: true,
            alive: true,
            garrison: false,
            own_turns: 3,
            timeouts: 0,
            captured: vec![Kind::Pawn, Kind::Rook],
        })
        .unwrap();
        assert_eq!(
            s,
            r#"{"seat":0,"present":true,"alive":true,"garrison":false,"own_turns":3,"timeouts":0,"captured":["pawn","rook"]}"#
        );
        let s = serde_json::to_string(&LastAction {
            seat: 0,
            kind: ActionKind::Move,
            fx: 3,
            fy: 0,
            tx: 3,
            ty: 1,
            captured: None,
            promoted: false,
            eliminated: None,
        })
        .unwrap();
        assert_eq!(
            s,
            r#"{"seat":0,"kind":"move","fx":3,"fy":0,"tx":3,"ty":1,"captured":null,"promoted":false,"eliminated":null}"#
        );
        let s = serde_json::to_string(&Formation::DEFAULT).unwrap();
        assert_eq!(
            s,
            r#"{"legend":["king","queen","hero","joker"],"epic":["rook","bishop","bishop","knight","rook"]}"#
        );
        // The board's own keys, with an empty piece and seat list so the
        // literal stays short; the nested shapes are pinned above.
        let s = serde_json::to_string(&BoardState {
            turn: 1,
            seat: 0,
            left_ms: 15_000,
            quiet: 0,
            stalls: 0,
            pieces: Vec::new(),
            seats: Vec::new(),
            last: None,
        })
        .unwrap();
        assert_eq!(
            s,
            r#"{"turn":1,"seat":0,"left_ms":15000,"quiet":0,"stalls":0,"pieces":[],"seats":[],"last":null}"#
        );
        let s = serde_json::to_string(&S2C::State {
            board: BoardState {
                turn: 1,
                seat: 0,
                left_ms: 15_000,
                quiet: 0,
                stalls: 0,
                pieces: Vec::new(),
                seats: Vec::new(),
                last: None,
            },
        })
        .unwrap();
        assert!(s.starts_with(r#"{"t":"state","board":{"turn":1,"#), "{s}");
        let s = serde_json::to_string(&S2C::Clock {
            turn: 4,
            seat: 2,
            left_ms: 8_400,
        })
        .unwrap();
        assert_eq!(s, r#"{"t":"clock","turn":4,"seat":2,"left_ms":8400}"#);
        let s = serde_json::to_string(&C2S::SetFormation {
            formation: Formation::DEFAULT,
        })
        .unwrap();
        assert!(s.starts_with(r#"{"t":"set_formation","formation":{"legend":"#), "{s}");
    }

    #[test]
    fn roundtrip_every_c2s_variant() {
        let all = [
            C2S::Hello {
                proto: PROTO_VERSION,
                handle: "ada".into(),
            },
            C2S::ListLobbies,
            C2S::CreateLobby {
                name: "court".into(),
                password: Some("pw".into()),
            },
            C2S::JoinLobby {
                name: "court".into(),
                password: None,
            },
            C2S::LeaveLobby,
            C2S::SetFormation {
                formation: Formation::DEFAULT,
            },
            C2S::Start,
            C2S::Move {
                turn: 9,
                fx: 1,
                fy: 2,
                tx: 2,
                ty: 4,
            },
            C2S::Ping { nonce: 5 },
        ];
        for msg in &all {
            assert_eq!(&roundtrip(msg), msg, "{msg:?}");
        }
    }

    #[test]
    fn roundtrip_every_s2c_variant() {
        let all = [
            S2C::Welcome {
                proto: PROTO_VERSION,
                host: "ember".into(),
                version: "r100".into(),
                commit: "abc1234".into(),
                players: 3,
                lobbies: 1,
            },
            S2C::Rejected {
                reason: "not your turn".into(),
            },
            S2C::Lobbies {
                lobbies: vec![LobbyInfo {
                    name: "court".into(),
                    host: "ada".into(),
                    has_password: true,
                    players: 2,
                    cap: MAX_PLAYERS,
                    playing: true,
                }],
            },
            S2C::Joined {
                lobby: "court".into(),
                id: 1,
            },
            S2C::Roster {
                creator: 0,
                roster: vec![
                    PlayerMeta {
                        id: 0,
                        handle: "ada".into(),
                        seat: 0,
                    },
                    PlayerMeta {
                        id: 1,
                        handle: "bob".into(),
                        seat: 2,
                    },
                ],
            },
            S2C::CanStart { players: 2 },
            S2C::Phase {
                phase: Phase::Finished,
                winner: None,
                end: Some(EndReason::Stalemate),
            },
            S2C::State {
                board: sample_board(),
            },
            S2C::Clock {
                turn: 4,
                seat: 2,
                left_ms: 8_400,
            },
            S2C::Pong { nonce: 5 },
        ];
        for msg in &all {
            assert_eq!(&roundtrip(msg), msg, "{msg:?}");
        }
    }

    /// Every `#[serde(default)]` field must actually decode when absent,
    /// with the absent behaviour its comment documents.
    #[test]
    fn defaulted_fields_decode_from_an_older_peer() {
        // A server that never learned `playing` lists a lobby as joinable.
        let old = r#"{"name":"court","host":"ada","has_password":false,"players":1,"cap":4}"#;
        let info: LobbyInfo = serde_json::from_str(old).expect("old lobby row");
        assert!(!info.playing);

        // A server that sends only `proto` in Welcome is unstamped and empty.
        let old = r#"{"t":"welcome","proto":1}"#;
        match serde_json::from_str::<S2C>(old).expect("old welcome") {
            S2C::Welcome {
                proto,
                host,
                version,
                commit,
                players,
                lobbies,
            } => {
                assert_eq!(proto, 1);
                assert!(host.is_empty() && version.is_empty() && commit.is_empty());
                assert_eq!((players, lobbies), (0, 0));
            }
            other => panic!("wrong variant: {other:?}"),
        }

        // A Phase with no winner and no end reason: Finished, poorer banner.
        let old = r#"{"t":"phase","phase":"finished"}"#;
        match serde_json::from_str::<S2C>(old).expect("old phase") {
            S2C::Phase { phase, winner, end } => {
                assert_eq!(phase, Phase::Finished);
                assert_eq!(winner, None);
                assert_eq!(end, None);
            }
            other => panic!("wrong variant: {other:?}"),
        }

        // A board with no `last`: the same board, nothing to narrate.
        let old =
            r#"{"turn":1,"seat":0,"left_ms":15000,"quiet":0,"stalls":0,"pieces":[],"seats":[]}"#;
        let board: BoardState = serde_json::from_str(old).expect("old board");
        assert_eq!(board.last, None);
    }

    /// The join gate is exact equality, so a mismatched peer must be told
    /// what it is speaking rather than silently playing a different game.
    #[test]
    fn version_mismatch_is_rejectable() {
        let hello = C2S::Hello {
            proto: PROTO_VERSION + 1,
            handle: "x".into(),
        };
        match roundtrip(&hello) {
            C2S::Hello { proto, .. } => assert_ne!(proto, PROTO_VERSION),
            other => panic!("wrong variant: {other:?}"),
        }
        let r = S2C::Rejected {
            reason: format!("this build speaks kings protocol v{PROTO_VERSION}"),
        };
        assert!(matches!(roundtrip(&r), S2C::Rejected { .. }));
    }

    /// 64 pieces and four captured lists: the biggest frame the game sends,
    /// measured at its worst: every piece the longest kind name, every
    /// captured list full of the longest kind name, every counter at its
    /// widest, and a `last` with every option set.
    #[test]
    fn a_full_board_frame_stays_far_under_max_frame_bytes() {
        let mut state = setup([true; 4], [Formation::DEFAULT; 4]);
        assert_eq!(state.board.iter().flatten().count(), 64);
        let longest = [
            Kind::King,
            Kind::Queen,
            Kind::Rook,
            Kind::Bishop,
            Kind::Knight,
            Kind::Pawn,
            Kind::Joker,
            Kind::Hero,
            Kind::HeroAwake,
        ]
        .into_iter()
        .max_by_key(|k| serde_json::to_string(k).unwrap().len())
        .unwrap();
        assert_eq!(longest, Kind::HeroAwake);
        for slot in state.board.iter_mut().flatten() {
            slot.kind = longest;
        }
        for seat in &mut state.seats {
            // A seat can take at most the other 48 pieces.
            seat.captured = vec![longest; 48];
            seat.own_turns = u32::MAX;
            seat.timeouts = u8::MAX;
        }
        state.turn = u32::MAX;
        state.quiet = u32::MAX;
        state.stalls = u8::MAX;
        state.last = Some(LastAction {
            seat: 3,
            kind: ActionKind::JokerTeleport,
            fx: 9,
            fy: 9,
            tx: 9,
            ty: 9,
            captured: Some(longest),
            promoted: true,
            eliminated: Some(3),
        });
        let frame = serde_json::to_string(&S2C::State {
            board: to_state(&state),
        })
        .expect("encode");
        assert!(
            frame.len() < MAX_FRAME_BYTES / 4,
            "worst-case board is {} bytes",
            frame.len()
        );
    }

    #[test]
    fn sanitize_and_handle_limits() {
        assert_eq!(sanitize_handle("\u{1b}[31mred"), "[31mred");
        assert_eq!(sanitize_handle("   "), "player");
        assert_eq!(sanitize_handle(""), "player");
        assert_eq!(sanitize_handle(&"x".repeat(100)).len(), MAX_HANDLE_LEN);
        assert_eq!(sanitize("a\nb", MAX_LOBBY_LEN), "ab");
        assert_eq!(
            sanitize(&"p".repeat(100), MAX_PASSWORD_LEN).len(),
            MAX_PASSWORD_LEN
        );
    }

    /// The three symptoms this predicate caused when it was wrong are in
    /// `fire-core`'s doc comment; this is the test that keeps the copy right.
    #[test]
    fn a_timed_out_read_is_not_a_dead_connection() {
        use std::io::{Error, ErrorKind};
        for kind in [
            ErrorKind::WouldBlock,
            ErrorKind::TimedOut,
            ErrorKind::Interrupted,
        ] {
            assert!(
                is_transient_read(&Error::new(kind, "poll")),
                "{kind:?} must be transient"
            );
        }
        let pending = Error::from_raw_os_error(WINDOWS_IO_PENDING);
        assert!(
            is_transient_read(&pending),
            "os error 997 must be transient"
        );
        for kind in [
            ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted,
            ErrorKind::BrokenPipe,
            ErrorKind::UnexpectedEof,
        ] {
            assert!(
                !is_transient_read(&Error::new(kind, "gone")),
                "{kind:?} must be fatal"
            );
        }
    }
}
