//! The `Selection` machine (design 4.4): pure, over `kings_core::targets`.
//!
//! Idle; click an own piece to select it (while Waiting, only an own Legend
//! or Epic piece); click a legal target to emit a `Move` stamped with the
//! current turn (or, while Waiting, a second own piece of the same class to
//! emit the `SetFormation` with the two tiles swapped); anything else
//! clears. `pending` is set by an emission and blocks every click until the
//! caller sees the next `State` or `Rejected` and calls `settle`.
//!
//! Nothing here is applied to a board: hotseat applies the emitted move
//! locally, the online game sends it and waits for the echo.

use kings_core::board::{EPIC_END, LEGEND_END, SETUP_LOCAL, State, Tile, to_global, to_local};
use kings_core::proto::{Formation, Kind, Phase};
use kings_core::{Target, targets};

/// The three character classes of design section 2, by home ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// The corner 2x2: king, queen, hero, joker.
    Legend,
    /// Tier 1: two rooks, two bishops, a knight.
    Epic,
    /// Tier 2: the seven pawns.
    Common,
}

/// The setup index of a tile in `seat`'s home block, or `None` outside it.
#[must_use]
pub fn setup_index(seat: u8, tile: Tile) -> Option<usize> {
    let local = to_local(seat, tile);
    SETUP_LOCAL.iter().position(|&uv| uv == local)
}

/// The class of a tile in `seat`'s home block, or `None` outside it.
#[must_use]
pub fn class_of(seat: u8, tile: Tile) -> Option<Class> {
    let index = setup_index(seat, tile)?;
    Some(if index < LEGEND_END {
        Class::Legend
    } else if index < EPIC_END {
        Class::Epic
    } else {
        Class::Common
    })
}

/// The formation `seat` currently shows on its home block, read off the
/// board. A Legend or Epic tile that is unexpectedly empty falls back to the
/// default kind for that tile.
#[must_use]
pub fn formation_on_board(state: &State, seat: u8) -> Formation {
    let kind_at = |index: usize| {
        let (u, v) = SETUP_LOCAL[index];
        state
            .piece(to_global(seat, u, v))
            .map_or(Formation::DEFAULT.kind_at(index), |p| p.kind)
    };
    Formation {
        legend: std::array::from_fn(kind_at),
        epic: std::array::from_fn(|i| kind_at(LEGEND_END + i)),
    }
}

/// The formation of `seat` with the kinds on `a` and `b` swapped. Both
/// tiles must be Legend or Epic tiles of the same class; the caller checks.
#[must_use]
pub fn swapped_formation(state: &State, seat: u8, a: Tile, b: Tile) -> Formation {
    let mut formation = formation_on_board(state, seat);
    let (Some(ia), Some(ib)) = (setup_index(seat, a), setup_index(seat, b)) else {
        return formation;
    };
    if ia >= EPIC_END || ib >= EPIC_END {
        return formation;
    }
    let mut kinds: [Kind; EPIC_END] = std::array::from_fn(|i| formation.kind_at(i));
    kinds.swap(ia, ib);
    formation.legend.copy_from_slice(&kinds[..LEGEND_END]);
    formation.epic.copy_from_slice(&kinds[LEGEND_END..EPIC_END]);
    formation
}

/// What is selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    /// Nothing.
    #[default]
    Idle,
    /// A piece on `from`, with its targets shown.
    Selected {
        /// The selected piece's tile.
        from: Tile,
    },
}

/// What a click produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiOut {
    /// A move to apply locally or send as `C2S::Move`.
    Move {
        /// The turn it was computed against.
        turn: u32,
        /// From.
        from: Tile,
        /// To.
        to: Tile,
    },
    /// A within-class card swap to send as `C2S::SetFormation`.
    SetFormation(Formation),
}

/// The selection machine.
#[derive(Clone, Debug, Default)]
pub struct Ui {
    /// Current selection.
    pub sel: Selection,
    /// Targets of the selected piece (empty while Waiting).
    pub targets: Vec<Target>,
    /// An emission is in flight; clicks are ignored until `settle`.
    pub pending: bool,
}

impl Ui {
    /// The selected tile, if any.
    #[must_use]
    pub const fn selected(&self) -> Option<Tile> {
        match self.sel {
            Selection::Idle => None,
            Selection::Selected { from } => Some(from),
        }
    }

    /// Drop the selection (Esc, a click elsewhere).
    pub fn clear(&mut self) {
        self.sel = Selection::Idle;
        self.targets.clear();
    }

    /// The next `State` or `Rejected` arrived: the emission is answered.
    pub fn settle(&mut self) {
        self.pending = false;
        self.clear();
    }

    fn select(&mut self, state: &State, phase: Phase, tile: Tile) {
        self.sel = Selection::Selected { from: tile };
        self.targets = if phase == Phase::Playing {
            targets(state, tile)
        } else {
            Vec::new()
        };
    }

    fn emit(&mut self, out: UiOut) -> Option<UiOut> {
        self.pending = true;
        self.clear();
        Some(out)
    }

    /// One click on `tile` by `me` (or, with `me == None`, by the seat to
    /// move: the hotseat convention).
    pub fn click(&mut self, state: &State, me: Option<u8>, phase: Phase, tile: Tile) -> Option<UiOut> {
        if self.pending {
            return None;
        }
        if phase == Phase::Finished {
            self.clear();
            return None;
        }
        let actor = me.unwrap_or(state.to_move);
        let own = |t: Tile| state.piece(t).is_some_and(|p| p.owner == actor);
        let selectable = |t: Tile| {
            own(t)
                && (phase != Phase::Waiting
                    || matches!(class_of(actor, t), Some(Class::Legend | Class::Epic)))
        };
        let Selection::Selected { from } = self.sel else {
            if selectable(tile) {
                self.select(state, phase, tile);
            } else {
                self.clear();
            }
            return None;
        };
        if phase == Phase::Playing && self.targets.iter().any(|t| t.tile() == tile) {
            return self.emit(UiOut::Move {
                turn: state.turn,
                from,
                to: tile,
            });
        }
        if phase == Phase::Waiting {
            let same_class = match (class_of(actor, from), class_of(actor, tile)) {
                (Some(a), Some(b)) => a == b && a != Class::Common,
                _ => false,
            };
            if tile != from && own(tile) && same_class {
                let formation = swapped_formation(state, actor, from, tile);
                return self.emit(UiOut::SetFormation(formation));
            }
            self.clear();
            return None;
        }
        if tile != from && selectable(tile) {
            // Another own piece: switch to it rather than making the player
            // click twice.
            self.select(state, phase, tile);
            return None;
        }
        self.clear();
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kings_core::setup;

    fn full() -> State {
        setup([true; 4], [Formation::DEFAULT; 4])
    }

    fn t(x: u8, y: u8) -> Tile {
        Tile::at(x, y)
    }

    #[test]
    fn classes_follow_the_rings() {
        assert_eq!(class_of(0, t(0, 0)), Some(Class::Legend));
        assert_eq!(class_of(0, t(1, 1)), Some(Class::Legend));
        assert_eq!(class_of(0, t(2, 0)), Some(Class::Epic));
        assert_eq!(class_of(0, t(0, 2)), Some(Class::Epic));
        assert_eq!(class_of(0, t(3, 3)), Some(Class::Common));
        assert_eq!(class_of(0, t(4, 4)), None);
        // Seat 2's joker tile (8,8) is a Legend tile in seat 2's frame only.
        assert_eq!(class_of(2, t(8, 8)), Some(Class::Legend));
        assert_eq!(class_of(0, t(8, 8)), None);
    }

    #[test]
    fn select_own_piece_shows_its_targets() {
        let state = full();
        let mut ui = Ui::default();
        assert_eq!(ui.click(&state, Some(0), Phase::Playing, t(3, 0)), None);
        assert_eq!(ui.selected(), Some(t(3, 0)));
        let tiles = |ui: &Ui| ui.targets.iter().map(|x| (x.x, x.y)).collect::<Vec<_>>();
        // Its left neighbour (3,1) holds an own pawn, so only forward.
        assert_eq!(tiles(&ui), vec![(4, 0)]);
        // The elbow pawn has both outward axes free.
        ui.click(&state, Some(0), Phase::Playing, t(3, 3));
        assert_eq!(ui.selected(), Some(t(3, 3)));
        assert_eq!(tiles(&ui), vec![(4, 3), (3, 4)]);
        // The hotseat convention: `me == None` acts for the seat to move.
        let mut ui = Ui::default();
        ui.click(&state, None, Phase::Playing, t(1, 2));
        assert_eq!(ui.selected(), Some(t(1, 2)), "seat 0's knight");
        assert!(!ui.pending);
    }

    #[test]
    fn a_target_emits_a_move_stamped_with_the_current_turn() {
        let mut state = full();
        state.turn = 7;
        let mut ui = Ui::default();
        ui.click(&state, Some(0), Phase::Playing, t(3, 0));
        let out = ui.click(&state, Some(0), Phase::Playing, t(4, 0));
        assert_eq!(
            out,
            Some(UiOut::Move {
                turn: 7,
                from: t(3, 0),
                to: t(4, 0),
            })
        );
        assert!(ui.pending);
        assert_eq!(ui.selected(), None);
        assert!(ui.targets.is_empty());
    }

    #[test]
    fn a_foreign_piece_clears() {
        let state = full();
        let mut ui = Ui::default();
        // Seat 1's king is foreign to seat 0: nothing selects.
        assert_eq!(ui.click(&state, Some(0), Phase::Playing, t(9, 0)), None);
        assert_eq!(ui.selected(), None);
        // From Selected, a foreign piece that is not a target clears too.
        ui.click(&state, Some(0), Phase::Playing, t(3, 0));
        assert_eq!(ui.click(&state, Some(0), Phase::Playing, t(9, 9)), None);
        assert_eq!(ui.selected(), None);
        assert!(!ui.pending);
        // And an empty non-target tile.
        ui.click(&state, Some(0), Phase::Playing, t(3, 0));
        assert_eq!(ui.click(&state, Some(0), Phase::Playing, t(7, 7)), None);
        assert_eq!(ui.selected(), None);
    }

    #[test]
    fn another_own_piece_switches_the_selection() {
        let state = full();
        let mut ui = Ui::default();
        ui.click(&state, Some(0), Phase::Playing, t(3, 0));
        ui.click(&state, Some(0), Phase::Playing, t(1, 2));
        assert_eq!(ui.selected(), Some(t(1, 2)));
        // Clicking the selected piece again toggles it off.
        ui.click(&state, Some(0), Phase::Playing, t(1, 2));
        assert_eq!(ui.selected(), None);
    }

    /// A dormant hero with no pawns left has its own tile as a target, so
    /// the self-click is a move, not a toggle.
    #[test]
    fn the_wake_in_place_is_reachable() {
        let mut state = full();
        for tile in Tile::all() {
            if state.piece(tile).is_some_and(|p| p.owner == 0 && p.kind == Kind::Pawn) {
                state.set(tile, None);
            }
        }
        let mut ui = Ui::default();
        ui.click(&state, Some(0), Phase::Playing, t(0, 1));
        let out = ui.click(&state, Some(0), Phase::Playing, t(0, 1));
        assert_eq!(
            out,
            Some(UiOut::Move {
                turn: 1,
                from: t(0, 1),
                to: t(0, 1),
            })
        );
    }

    #[test]
    fn waiting_same_class_emits_the_swapped_formation() {
        let state = full();
        let mut ui = Ui::default();
        ui.click(&state, Some(0), Phase::Waiting, t(0, 0));
        assert_eq!(ui.selected(), Some(t(0, 0)));
        assert!(ui.targets.is_empty(), "no move highlights while waiting");
        let out = ui.click(&state, Some(0), Phase::Waiting, t(1, 1));
        assert_eq!(
            out,
            Some(UiOut::SetFormation(Formation {
                legend: [Kind::Joker, Kind::Queen, Kind::Hero, Kind::King],
                epic: Formation::DEFAULT.epic,
            }))
        );
        assert!(ui.pending);
        ui.settle();
        // Epic too, for seat 2 in its own frame: (7,9) rook with (8,7) knight.
        let out2 = {
            ui.click(&state, Some(2), Phase::Waiting, t(7, 9));
            ui.click(&state, Some(2), Phase::Waiting, t(8, 7))
        };
        assert_eq!(
            out2,
            Some(UiOut::SetFormation(Formation {
                legend: Formation::DEFAULT.legend,
                epic: [
                    Kind::Knight,
                    Kind::Bishop,
                    Kind::Bishop,
                    Kind::Rook,
                    Kind::Rook
                ],
            }))
        );
    }

    #[test]
    fn waiting_refuses_cross_class_and_commons() {
        let state = full();
        let mut ui = Ui::default();
        ui.click(&state, Some(0), Phase::Waiting, t(0, 0));
        assert_eq!(ui.click(&state, Some(0), Phase::Waiting, t(2, 0)), None);
        assert_eq!(ui.selected(), None, "king with rook: cleared");
        assert!(!ui.pending);
        // A pawn cannot be selected while waiting.
        ui.click(&state, Some(0), Phase::Waiting, t(3, 0));
        assert_eq!(ui.selected(), None);
        // A foreign legend cannot be selected, and clears a selection.
        ui.click(&state, Some(0), Phase::Waiting, t(1, 1));
        assert_eq!(ui.click(&state, Some(0), Phase::Waiting, t(8, 8)), None);
        assert_eq!(ui.selected(), None);
        // The same tile twice is not a swap.
        ui.click(&state, Some(0), Phase::Waiting, t(1, 1));
        assert_eq!(ui.click(&state, Some(0), Phase::Waiting, t(1, 1)), None);
        assert_eq!(ui.selected(), None);
    }

    #[test]
    fn pending_blocks_clicks_until_the_echo() {
        let state = full();
        let mut ui = Ui::default();
        ui.click(&state, Some(0), Phase::Playing, t(3, 0));
        assert!(ui.click(&state, Some(0), Phase::Playing, t(4, 0)).is_some());
        assert!(ui.pending);
        assert_eq!(ui.click(&state, Some(0), Phase::Playing, t(3, 1)), None);
        assert_eq!(ui.selected(), None, "ignored, not selected");
        ui.settle();
        assert!(!ui.pending);
        ui.click(&state, Some(0), Phase::Playing, t(3, 1));
        assert_eq!(ui.selected(), Some(t(3, 1)));
    }

    #[test]
    fn finished_ignores_everything() {
        let state = full();
        let mut ui = Ui::default();
        ui.click(&state, Some(0), Phase::Playing, t(3, 0));
        assert_eq!(ui.click(&state, Some(0), Phase::Finished, t(4, 0)), None);
        assert_eq!(ui.selected(), None);
    }

    #[test]
    fn the_formation_is_read_off_the_board() {
        let custom = Formation {
            legend: [Kind::Joker, Kind::Queen, Kind::Hero, Kind::King],
            epic: [
                Kind::Bishop,
                Kind::Rook,
                Kind::Rook,
                Kind::Bishop,
                Kind::Knight,
            ],
        };
        let state = setup([true; 4], [custom, Formation::DEFAULT, custom, Formation::DEFAULT]);
        assert_eq!(formation_on_board(&state, 0), custom);
        assert_eq!(formation_on_board(&state, 1), Formation::DEFAULT);
        assert_eq!(formation_on_board(&state, 2), custom);
        // Swapping the two rooks of seat 0 (local (2,1) and (2,2)) changes
        // nothing; swapping a bishop and a rook does.
        let a = to_global(0, 2, 1);
        let b = to_global(0, 2, 2);
        assert_eq!(swapped_formation(&state, 0, a, b), custom);
        let c = to_global(0, 2, 0);
        let swapped = swapped_formation(&state, 0, c, a);
        assert_eq!(swapped.epic[0], Kind::Rook);
        assert_eq!(swapped.epic[1], Kind::Bishop);
        assert_eq!(swapped.legend, custom.legend);
    }
}
