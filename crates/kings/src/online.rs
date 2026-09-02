//! Online play: pure client state over the server's messages (design 4.4).
//!
//! There is no prediction. The server is the sole clock and the sole
//! validator; the client keeps the last `State` it was sent, decodes it with
//! `kings_core::board::from_state` for highlights, counts the clock down
//! between `Clock` messages and never past what it was told, and shows
//! `Rejected` reasons as a notice. `apply` and `tick` are pure so the whole
//! thing is unit-tested against hand-built messages.

use kings_core::board::{State, from_state};
use kings_core::proto::{BoardState, EndReason, LobbyInfo, MIN_PLAYERS, Phase, PlayerMeta, S2C};

use crate::game::Screen;

/// The client's view of the server.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // one flag per fact the page shows
pub struct Online {
    /// Set by `Welcome`. `Hello` must be the first message on a connection
    /// and `Welcome` is its acknowledgement, so anything version-gated,
    /// create and join both are, has to wait for it.
    pub welcomed: bool,
    /// Browsing until `Joined`.
    pub screen: Screen,
    /// Our lobby-local id, from `Joined`.
    pub my_id: Option<u8>,
    /// Our seat, from the `Roster` entry with our id.
    pub my_seat: Option<u8>,
    /// The creator's id, from `Roster`.
    pub creator: Option<u8>,
    /// We are the creator.
    pub is_creator: bool,
    /// The creator may start now.
    pub can_start: bool,
    /// The lobby's phase.
    pub phase: Phase,
    /// The winner once Finished.
    pub winner: Option<u8>,
    /// Why the game ended once Finished.
    pub end: Option<EndReason>,
    /// The last board, decoded.
    pub state: Option<State>,
    /// The last board as sent.
    pub board: Option<BoardState>,
    /// The lobby's members with seats.
    pub roster: Vec<PlayerMeta>,
    /// The last lobby list.
    pub lobbies: Vec<LobbyInfo>,
    /// The last thing the server refused, for the page to show.
    pub notice: Option<String>,
    /// A move or formation is in flight.
    pub pending: bool,
    /// The displayed clock: resynced by `State` and `Clock`, counted down by
    /// `tick`, never below 0.
    pub left_ms: u32,
    /// Sub-millisecond remainder of the countdown.
    left_frac: f32,
}

impl Default for Online {
    fn default() -> Self {
        Self::new()
    }
}

impl Online {
    /// Nothing heard yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            welcomed: false,
            screen: Screen::Browsing,
            my_id: None,
            my_seat: None,
            creator: None,
            is_creator: false,
            can_start: false,
            phase: Phase::Waiting,
            winner: None,
            end: None,
            state: None,
            board: None,
            roster: Vec::new(),
            lobbies: Vec::new(),
            notice: None,
            pending: false,
            left_ms: 0,
            left_frac: 0.0,
        }
    }

    /// The local seat is to move.
    #[must_use]
    pub fn my_turn(&self) -> bool {
        self.phase == Phase::Playing
            && self.my_seat.is_some()
            && self.board.as_ref().map(|b| b.seat) == self.my_seat
    }

    /// Apply one server message.
    pub fn apply(&mut self, msg: S2C) {
        match msg {
            S2C::Welcome { .. } => self.welcomed = true,
            S2C::Rejected { reason } => {
                self.notice = Some(reason);
                self.pending = false;
            }
            S2C::Lobbies { lobbies } => self.lobbies = lobbies,
            S2C::Joined { id, .. } => {
                self.screen = Screen::Lobby;
                self.my_id = Some(id);
                self.notice = None;
                self.pending = false;
                self.winner = None;
                self.end = None;
            }
            S2C::Roster { creator, roster } => {
                self.my_seat = roster
                    .iter()
                    .find(|p| Some(p.id) == self.my_id)
                    .map(|p| p.seat);
                self.creator = Some(creator);
                self.is_creator = self.my_id.is_some() && self.my_id == Some(creator);
                self.can_start = self.is_creator && roster.len() >= usize::from(MIN_PLAYERS);
                self.roster = roster;
            }
            S2C::CanStart { .. } => self.can_start = true,
            S2C::Phase { phase, winner, end } => {
                self.phase = phase;
                if phase == Phase::Finished {
                    self.winner = winner;
                    self.end = end;
                } else {
                    self.winner = None;
                    self.end = None;
                }
                if phase != Phase::Playing {
                    self.pending = false;
                }
            }
            S2C::State { board } => {
                match from_state(&board) {
                    Ok(state) => self.state = Some(state),
                    Err(e) => self.notice = Some(format!("bad board from the server: {e}")),
                }
                self.left_ms = board.left_ms;
                self.left_frac = 0.0;
                self.board = Some(board);
                self.pending = false;
            }
            S2C::Clock {
                turn,
                seat,
                left_ms,
            } => {
                self.left_ms = left_ms;
                self.left_frac = 0.0;
                if let Some(board) = self.board.as_mut()
                    && board.turn == turn
                    && board.seat == seat
                {
                    board.left_ms = left_ms;
                }
            }
            S2C::Pong { .. } => {}
        }
    }

    /// Count the displayed clock down by `dt` seconds while Playing; it
    /// stops at 0 and waits for the server's word.
    pub fn tick(&mut self, dt: f32) {
        if self.phase != Phase::Playing {
            return;
        }
        self.left_frac += dt.max(0.0) * 1000.0;
        let whole = self.left_frac.floor();
        self.left_frac -= whole;
        // Non-negative, and a frame is far below u32::MAX milliseconds.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = whole as u32;
        self.left_ms = self.left_ms.saturating_sub(whole);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kings_core::board::{setup, to_state};
    use kings_core::proto::{Formation, Kind, TURN_MS};

    fn roster(creator: u8, ids: &[(u8, u8)]) -> S2C {
        S2C::Roster {
            creator,
            roster: ids
                .iter()
                .map(|&(id, seat)| PlayerMeta {
                    id,
                    handle: format!("p{id}"),
                    seat,
                })
                .collect(),
        }
    }

    fn board() -> BoardState {
        to_state(&setup([true, false, true, false], [Formation::DEFAULT; 4]))
    }

    #[test]
    fn joined_then_roster_sets_me_and_the_creator() {
        let mut o = Online::new();
        assert_eq!(o.screen, Screen::Browsing);
        o.apply(S2C::Welcome {
            proto: 1,
            host: String::new(),
            version: String::new(),
            commit: String::new(),
            players: 0,
            lobbies: 0,
        });
        assert!(o.welcomed);
        o.apply(S2C::Joined {
            lobby: "court".into(),
            id: 1,
        });
        assert_eq!(o.screen, Screen::Lobby);
        assert_eq!(o.my_id, Some(1));
        assert_eq!(o.my_seat, None, "no roster yet");
        o.apply(roster(0, &[(0, 0), (1, 2)]));
        assert_eq!(o.my_seat, Some(2));
        assert_eq!(o.creator, Some(0));
        assert!(!o.is_creator);
        assert!(!o.can_start, "only the creator may start");
        assert_eq!(o.roster.len(), 2);
        // The creator's side of the same lobby.
        let mut c = Online::new();
        c.apply(S2C::Joined {
            lobby: "court".into(),
            id: 0,
        });
        c.apply(roster(0, &[(0, 0)]));
        assert!(c.is_creator);
        assert!(!c.can_start, "alone");
        c.apply(roster(0, &[(0, 0), (1, 2)]));
        assert!(c.can_start, "two seated");
    }

    #[test]
    fn a_roster_handover_updates_the_creator() {
        let mut o = Online::new();
        o.apply(S2C::Joined {
            lobby: "court".into(),
            id: 1,
        });
        o.apply(roster(0, &[(0, 0), (1, 2)]));
        assert!(!o.is_creator);
        // The creator left: we are moved to seat 0 and become the creator.
        o.apply(roster(1, &[(1, 0)]));
        assert!(o.is_creator);
        assert_eq!(o.creator, Some(1));
        assert_eq!(o.my_seat, Some(0));
        assert!(!o.can_start, "alone again");
        o.apply(S2C::CanStart { players: 2 });
        assert!(o.can_start);
    }

    #[test]
    fn a_state_replaces_the_board_and_resyncs_the_clock() {
        let mut o = Online::new();
        assert!(o.state.is_none());
        let mut b = board();
        b.left_ms = 9_000;
        o.apply(S2C::State { board: b.clone() });
        assert_eq!(o.board.as_ref(), Some(&b));
        let s = o.state.as_ref().expect("decoded");
        assert_eq!(s.board.iter().flatten().count(), 64);
        assert!(s.seats[1].garrison);
        assert_eq!(o.left_ms, 9_000);
        // A second state is a replacement, not a merge.
        let mut b2 = board();
        b2.pieces.retain(|p| p.owner != 3);
        b2.turn = 12;
        o.apply(S2C::State { board: b2 });
        assert_eq!(o.state.as_ref().unwrap().board.iter().flatten().count(), 48);
        assert_eq!(o.state.as_ref().unwrap().turn, 12);
        assert_eq!(o.left_ms, TURN_MS);
        // An impossible board is refused with a notice, the old one kept.
        let mut bad = board();
        bad.seats.pop();
        o.apply(S2C::State { board: bad });
        assert_eq!(o.state.as_ref().unwrap().turn, 12);
        assert!(o.notice.as_deref().unwrap().starts_with("bad board"));
    }

    #[test]
    fn the_clock_resyncs_and_counts_down_never_below_zero() {
        let mut o = Online::new();
        o.apply(S2C::State { board: board() });
        o.apply(S2C::Phase {
            phase: Phase::Playing,
            winner: None,
            end: None,
        });
        o.apply(S2C::Clock {
            turn: 1,
            seat: 0,
            left_ms: 2_500,
        });
        assert_eq!(o.left_ms, 2_500);
        assert_eq!(o.board.as_ref().unwrap().left_ms, 2_500);
        o.tick(0.5);
        assert_eq!(o.left_ms, 2_000);
        for _ in 0..100 {
            o.tick(1.0 / 60.0);
        }
        // 100 frames at 16.67 ms is 1666 ms; fractions carry over.
        assert!((330..=340).contains(&o.left_ms), "{}", o.left_ms);
        o.tick(10.0);
        assert_eq!(o.left_ms, 0, "never negative");
        o.tick(1.0);
        assert_eq!(o.left_ms, 0);
        o.apply(S2C::Clock {
            turn: 1,
            seat: 0,
            left_ms: 800,
        });
        assert_eq!(o.left_ms, 800, "resynced");
        // A clock for another turn does not touch the board snapshot.
        o.apply(S2C::Clock {
            turn: 2,
            seat: 2,
            left_ms: 15_000,
        });
        assert_eq!(o.board.as_ref().unwrap().left_ms, 800);
        assert_eq!(o.left_ms, 15_000);
        // Not counting while waiting.
        o.apply(S2C::Phase {
            phase: Phase::Waiting,
            winner: None,
            end: None,
        });
        o.tick(5.0);
        assert_eq!(o.left_ms, 15_000);
    }

    #[test]
    fn rejected_surfaces_a_notice_and_clears_pending() {
        let mut o = Online::new();
        o.pending = true;
        o.apply(S2C::Rejected {
            reason: "not your turn".into(),
        });
        assert_eq!(o.notice.as_deref(), Some("not your turn"));
        assert!(!o.pending);
        // A State echo clears pending too, and joining clears the notice.
        o.pending = true;
        o.apply(S2C::State { board: board() });
        assert!(!o.pending);
        o.apply(S2C::Joined {
            lobby: "court".into(),
            id: 0,
        });
        assert!(o.notice.is_none());
    }

    #[test]
    fn phase_finished_stores_winner_and_end() {
        let mut o = Online::new();
        o.apply(S2C::Phase {
            phase: Phase::Playing,
            winner: None,
            end: None,
        });
        assert_eq!(o.phase, Phase::Playing);
        o.apply(S2C::Phase {
            phase: Phase::Finished,
            winner: Some(2),
            end: Some(EndReason::LastKing),
        });
        assert_eq!(o.phase, Phase::Finished);
        assert_eq!(o.winner, Some(2));
        assert_eq!(o.end, Some(EndReason::LastKing));
        // A draw.
        o.apply(S2C::Phase {
            phase: Phase::Finished,
            winner: None,
            end: Some(EndReason::Stalemate),
        });
        assert_eq!(o.winner, None);
        assert_eq!(o.end, Some(EndReason::Stalemate));
        // Back to Waiting clears the result.
        o.apply(S2C::Phase {
            phase: Phase::Waiting,
            winner: None,
            end: None,
        });
        assert_eq!(o.winner, None);
        assert_eq!(o.end, None);
    }

    #[test]
    fn my_turn_follows_the_board() {
        let mut o = Online::new();
        o.apply(S2C::Joined {
            lobby: "court".into(),
            id: 1,
        });
        o.apply(roster(0, &[(0, 0), (1, 2)]));
        o.apply(S2C::State { board: board() });
        assert!(!o.my_turn(), "waiting");
        o.apply(S2C::Phase {
            phase: Phase::Playing,
            winner: None,
            end: None,
        });
        assert!(!o.my_turn(), "seat 0 moves first");
        let mut b = board();
        b.seat = 2;
        o.apply(S2C::State { board: b });
        assert!(o.my_turn());
        let mut o2 = Online::new();
        o2.apply(S2C::Phase {
            phase: Phase::Playing,
            winner: None,
            end: None,
        });
        o2.apply(S2C::State { board: board() });
        assert!(!o2.my_turn(), "no seat, never my turn");
    }

    #[test]
    fn lobbies_and_pong_are_bookkeeping() {
        let mut o = Online::new();
        o.apply(S2C::Lobbies {
            lobbies: vec![LobbyInfo {
                name: "court".into(),
                host: "ada".into(),
                has_password: false,
                players: 1,
                cap: 4,
                playing: false,
            }],
        });
        assert_eq!(o.lobbies.len(), 1);
        let before = o.clone();
        o.apply(S2C::Pong { nonce: 3 });
        assert_eq!(o.lobbies, before.lobbies);
        assert_eq!(o.screen, before.screen);
        // The captured lists come through the wire snapshot.
        let mut b = board();
        b.seats[0].captured = vec![Kind::Pawn];
        o.apply(S2C::State { board: b });
        assert_eq!(
            o.state.as_ref().unwrap().seats[0].captured,
            vec![Kind::Pawn]
        );
    }
}
