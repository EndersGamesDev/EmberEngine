//! Hotseat: four human seats on one keyboard, the default formations, the
//! engine's own `TurnClock` run by this client without grace, and a camera
//! that swings to the seat to move (design 1.4 and 4.4).
//!
//! `me` is `None`: whoever is to move acts, and the page lays the board out
//! for seat 0. The keyboard cursor moves in seat 0's frame for the same
//! reason: the page's grid, where the cursor is drawn, is seat 0's.

use ember_engine::{EmberGame, Frame, InputState};
use kings_core::board::{State, Tile, to_state};
use kings_core::proto::{Formation, Phase};
use kings_core::{apply, setup, timeout};

use crate::game::{
    self, Cursor, CursorCmd, CursorKeys, HudState, Meshes, Mode, SeatCamera, UiCmd, View,
    fill_board, set_hud,
};
use crate::ui::{Ui, UiOut};

/// The frame the keyboard cursor moves in: the page renders seat 0
/// bottom-left in hotseat.
const CURSOR_SEAT: u8 = 0;

/// The hotseat game.
pub struct Hotseat {
    /// The game state, applied locally.
    pub state: State,
    ui: Ui,
    cursor: Cursor,
    cam: SeatCamera,
    meshes: Meshes,
    notice: Option<String>,
    /// Fractional milliseconds not yet fed to the clock.
    ms_acc: f32,
}

impl Hotseat {
    /// A fresh four-seat game with the default formations.
    #[must_use]
    pub fn new(meshes: Meshes) -> Self {
        let state = setup([true; 4], [Formation::DEFAULT; 4]);
        Self {
            cam: SeatCamera::new(state.to_move),
            state,
            ui: Ui::default(),
            cursor: Cursor::new(CURSOR_SEAT),
            meshes,
            notice: None,
            ms_acc: 0.0,
        }
    }

    /// Playing until the engine has a result, then Finished.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        if self.state.result.is_some() {
            Phase::Finished
        } else {
            Phase::Playing
        }
    }

    /// The seat the camera is at or swinging to.
    #[must_use]
    pub const fn camera_seat(&self) -> u8 {
        self.cam.seat()
    }

    /// Point the camera at the seat to move.
    pub fn follow(&mut self) {
        self.cam.retarget(self.state.to_move);
    }

    /// Advance the client clock by `ms`. There is no grace in hotseat: the
    /// turn times out the moment the displayed clock reads 0.
    pub fn advance(&mut self, ms: u32) {
        if self.state.result.is_some() {
            return;
        }
        self.state.clock.tick(ms);
        if self.state.clock.display_left_ms() == 0 {
            timeout(&mut self.state);
            self.ui.clear();
            self.follow();
        }
    }

    /// A click on `tile` by the seat to move.
    pub fn click(&mut self, tile: Tile) {
        let phase = self.phase();
        match self.ui.click(&self.state, None, phase, tile) {
            Some(UiOut::Move { from, to, .. }) => {
                match apply(&mut self.state, from, to) {
                    Ok(_) => self.notice = None,
                    Err(e) => self.notice = Some(e.reason().to_string()),
                }
                // The echo is immediate: the board above IS the answer.
                self.ui.settle();
                self.follow();
            }
            // Formations are frozen once a game runs, and hotseat is always
            // running; the machine never emits this in Playing.
            Some(UiOut::SetFormation(_)) => self.ui.settle(),
            None => {}
        }
    }

    /// Drop the selection.
    pub fn clear(&mut self) {
        self.ui.clear();
    }

    fn view(&self) -> View {
        View {
            sel: self.ui.selected(),
            targets: self.ui.targets.clone(),
            cursor: Some(self.cursor.tile),
        }
    }

    fn publish(&self) {
        let wire = to_state(&self.state);
        let mut hud = HudState::new(Mode::Local);
        hud.phase = self.phase();
        if let Some(outcome) = self.state.result {
            hud.winner = outcome.winner;
            hud.end = Some(outcome.end);
        }
        fill_board(&mut hud, &wire, &self.state, &self.view());
        hud.left_ms = wire.left_ms;
        hud.notice.clone_from(&self.notice);
        set_hud(hud);
    }
}

impl EmberGame for Hotseat {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let dt = dt.clamp(0.0, 0.1);
        self.ms_acc += dt * 1000.0;
        let whole = self.ms_acc.floor();
        self.ms_acc -= whole;
        // Non-negative and far below u32::MAX: at most 100 per frame.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = whole as u32;
        self.advance(ms);

        for cmd in game::drain_cmds() {
            match cmd {
                UiCmd::Click(x, y) => {
                    if let Some(t) = Tile::new(x, y) {
                        self.click(t);
                    }
                }
                UiCmd::Clear => self.clear(),
                // There is no lobby in hotseat: the game is always started.
                UiCmd::Start => {}
            }
        }
        match self.cursor.step(CursorKeys::read(input), CURSOR_SEAT) {
            Some(CursorCmd::Click(t)) => self.click(t),
            Some(CursorCmd::Clear) => self.clear(),
            None => {}
        }

        self.follow();
        let camera = self.cam.tick(dt);
        self.publish();
        game::scene(&self.state, &self.meshes, &self.view(), camera)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kings_core::proto::{ActionKind, TURN_MS};

    fn fresh() -> Hotseat {
        let (_, ids) = game::build_meshes();
        Hotseat::new(ids)
    }

    #[test]
    fn four_human_seats_with_no_garrisons() {
        let h = fresh();
        assert_eq!(h.state.board.iter().flatten().count(), 64);
        for seat in &h.state.seats {
            assert!(seat.present && seat.alive && !seat.garrison);
        }
        assert_eq!(h.state.to_move, 0);
        assert_eq!(h.phase(), Phase::Playing);
        assert_eq!(h.camera_seat(), 0);
        let hud = {
            h.publish();
            game::hud()
        };
        assert_eq!(hud.mode, Mode::Local);
        assert_eq!(hud.me, None);
        assert_eq!(hud.left_ms, TURN_MS);
        assert_eq!(hud.pieces.len(), 64);
        assert_eq!(hud.phase, Phase::Playing);
        assert!(!hud.my_turn);
    }

    #[test]
    fn the_client_clock_times_out_at_the_displayed_zero() {
        let mut h = fresh();
        h.advance(TURN_MS - 1);
        assert_eq!(h.state.to_move, 0, "one millisecond left");
        assert_eq!(h.state.seats[0].timeouts, 0);
        h.advance(1);
        assert_eq!(h.state.to_move, 1, "no grace in hotseat");
        assert_eq!(h.state.seats[0].timeouts, 1);
        assert_eq!(h.state.last.map(|l| l.kind), Some(ActionKind::Timeout));
        assert_eq!(h.state.turn, 2);
        assert_eq!(h.state.clock.display_left_ms(), TURN_MS, "reset for seat 1");
        // The camera follows the seat to move.
        assert_eq!(h.camera_seat(), 1);
        for expect in [2, 3, 0] {
            h.advance(TURN_MS);
            assert_eq!(h.state.to_move, expect);
            assert_eq!(h.camera_seat(), expect);
        }
    }

    #[test]
    fn a_click_pair_applies_a_move_and_an_illegal_pair_does_not() {
        let mut h = fresh();
        h.click(Tile::at(3, 0));
        assert_eq!(h.ui.selected(), Some(Tile::at(3, 0)));
        h.click(Tile::at(4, 0));
        assert_eq!(h.state.piece(Tile::at(4, 0)).map(|p| p.owner), Some(0));
        assert_eq!(h.state.piece(Tile::at(3, 0)), None);
        assert_eq!(h.state.to_move, 1);
        assert!(!h.ui.pending, "the local echo settles the machine at once");
        assert_eq!(h.notice, None);
        assert_eq!(h.camera_seat(), 1);
        // Seat 1's turn: a seat-0 piece is foreign now and cannot be picked.
        h.click(Tile::at(4, 0));
        assert_eq!(h.ui.selected(), None);
        // Seat 1's knight (7,1) to a non-target clears without a move.
        h.click(Tile::at(7, 1));
        h.click(Tile::at(4, 4));
        assert_eq!(h.ui.selected(), None);
        assert_eq!(h.state.to_move, 1);
        assert_eq!(h.state.piece(Tile::at(7, 1)).map(|p| p.owner), Some(1));
    }

    #[test]
    fn the_game_runs_without_input() {
        let mut h = fresh();
        let input = InputState::default();
        // Four seats, 15 s each, at 60 Hz: through a whole round of timeouts.
        for _ in 0..(60 * 61) {
            let frame = h.update(&input, 1.0 / 60.0);
            assert!(frame.camera.eye.is_finite());
            assert_eq!(frame.instances.len(), 100 + 64 + 1, "tiles, pieces, cursor");
        }
        assert!(h.state.turn >= 5, "turn {}", h.state.turn);
        for seat in &h.state.seats {
            assert_eq!(seat.timeouts, 1);
        }
        let hud = game::hud();
        assert_eq!(hud.cursor, [3, 3]);
        assert!(hud.left_ms <= TURN_MS);
    }
}
