//! What hotseat and online play share (design 4.4 and 4.6).
//!
//! Mesh ids in registration order, the seat colours, tile and camera maths,
//! `scene()`, the HUD thread-local the page polls, the `UiCmd` queue the
//! page pushes into, and the keyboard cursor.

use std::cell::RefCell;
use std::f32::consts::FRAC_PI_2;

use ember_engine::glam::Vec3;
use ember_engine::{Camera, Frame, InputState, Instance, KeyCode, MeshData};
use kings_core::board::{
    SIDE, State, TILES, Tile, frame, front_left, in_home_block, to_global, to_local,
};
use kings_core::proto::{
    ActionKind, BoardState, EndReason, Kind, LastAction, Phase, PieceState, PlayerMeta, SeatState,
};
use kings_core::{Seat, Target, TargetKind, TurnClock};
use serde::Serialize;

use crate::meshes;

// ---- meshes ---------------------------------------------------------------

/// Mesh ids, assigned in registration order.
///
/// `EngineConfig.meshes` entries take ids `1..=N` and id 0 is the engine's
/// built-in cube, so the order of `build_meshes` IS this struct; the two
/// change together and a test pins them.
#[derive(Clone, Copy, Debug)]
pub struct Meshes {
    /// The board tile.
    pub tile: u32,
    /// The legal-target disc.
    pub disc: u32,
    /// The selection and cursor ring.
    pub ring: u32,
    /// King.
    pub king: u32,
    /// Queen.
    pub queen: u32,
    /// Rook.
    pub rook: u32,
    /// Bishop.
    pub bishop: u32,
    /// Knight.
    pub knight: u32,
    /// Pawn.
    pub pawn: u32,
    /// Joker.
    pub joker: u32,
    /// Dormant hero.
    pub hero: u32,
    /// Awake hero.
    pub hero_awake: u32,
}

impl Meshes {
    /// The mesh of a piece kind.
    #[must_use]
    pub const fn of(&self, kind: Kind) -> u32 {
        match kind {
            Kind::King => self.king,
            Kind::Queen => self.queen,
            Kind::Rook => self.rook,
            Kind::Bishop => self.bishop,
            Kind::Knight => self.knight,
            Kind::Pawn => self.pawn,
            Kind::Joker => self.joker,
            Kind::Hero => self.hero,
            Kind::HeroAwake => self.hero_awake,
        }
    }
}

/// Every mesh the client registers, and their ids.
#[must_use]
pub fn build_meshes() -> (Vec<MeshData>, Meshes) {
    let list = vec![
        // 1 tile, 2 disc, 3 ring
        meshes::tile(),
        meshes::disc(),
        meshes::ring(),
        // 4..=12 the nine silhouettes, in `Kind` order
        meshes::king(),
        meshes::queen(),
        meshes::rook(),
        meshes::bishop(),
        meshes::knight(),
        meshes::pawn(),
        meshes::joker(),
        meshes::hero(),
        meshes::hero_awake(),
    ];
    let ids = Meshes {
        tile: 1,
        disc: 2,
        ring: 3,
        king: 4,
        queen: 5,
        rook: 6,
        bishop: 7,
        knight: 8,
        pawn: 9,
        joker: 10,
        hero: 11,
        hero_awake: 12,
    };
    (list, ids)
}

// ---- colours --------------------------------------------------------------

/// The page's four seat colours as their sRGB components over 255.
///
/// `#e6b93f` gold, `#e2495c` crimson, `#4ea6f2` azure, `#3fd08c` emerald.
/// The renderer applies no transfer curve, so they are used as they are,
/// the same convention as `fire::game::livery`; that reads a little
/// brighter than the page's swatches under the scene light, which is the
/// intent for a small piece on a dark board.
pub const SEAT_COLOURS: [Vec3; 4] = [
    Vec3::new(0.902, 0.725, 0.247),
    Vec3::new(0.886, 0.286, 0.361),
    Vec3::new(0.306, 0.651, 0.949),
    Vec3::new(0.247, 0.816, 0.549),
];
const TILE_LIGHT: Vec3 = Vec3::new(0.44, 0.42, 0.50);
const TILE_DARK: Vec3 = Vec3::new(0.30, 0.28, 0.36);
/// How far a corner block's tiles lean toward the seat colour.
const BLOCK_TINT: f32 = 0.15;
/// Garrison pieces are the seat colour dimmed, as the page greys them.
const GARRISON_DIM: f32 = 0.55;
const SEL_COLOUR: Vec3 = Vec3::new(0.96, 0.92, 0.83);
const CURSOR_COLOUR: Vec3 = Vec3::new(0.62, 0.60, 0.70);
const LAST_COLOUR: Vec3 = Vec3::new(0.55, 0.53, 0.60);

/// The colour of a seat; any seat number is folded into the four.
#[must_use]
pub fn seat_colour(seat: u8) -> Vec3 {
    SEAT_COLOURS[usize::from(seat) % SEAT_COLOURS.len()]
}

/// The target disc colour by kind, the page's `--tgt-*` swatches.
#[must_use]
pub const fn target_colour(kind: TargetKind) -> Vec3 {
    match kind {
        TargetKind::Move => Vec3::new(0.31, 0.84, 0.42),
        TargetKind::Capture => Vec3::new(1.0, 0.36, 0.36),
        TargetKind::Teleport => Vec3::new(0.69, 0.52, 1.0),
        TargetKind::Place => Vec3::new(1.0, 0.71, 0.28),
        TargetKind::Swap | TargetKind::Wake => Vec3::new(0.31, 0.89, 0.89),
    }
}

// ---- geometry -------------------------------------------------------------

/// World units between tile centres.
pub const TILE_PITCH: f32 = 1.0;
const HALF_BOARD: f32 = 4.5;

/// The world position of a tile's centre, on the board's top surface: the
/// board is centred at the origin, `x` runs east along `+X`, `y` runs north
/// along `-Z`, `Y` is up.
#[must_use]
pub fn tile_pos(t: Tile) -> Vec3 {
    Vec3::new(
        (f32::from(t.x) - HALF_BOARD) * TILE_PITCH,
        0.0,
        (HALF_BOARD - f32::from(t.y)) * TILE_PITCH,
    )
}

/// The yaw that turns a mesh's `+X` (its forward) to a seat's forward.
#[must_use]
pub fn seat_yaw(seat: u8) -> f32 {
    f32::from(seat % 4) * FRAC_PI_2
}

// ---- camera ---------------------------------------------------------------

const CAM_UP: f32 = 12.0;
const CAM_OUT: f32 = 11.0;
const CAM_FOV: f32 = 50.0;
/// How long the hotseat camera takes to swing to the next seat's corner.
pub const CAM_LERP_SECS: f32 = 0.4;

/// A seat's camera: eye on the seat's corner diagonal, `CAM_UP` up and
/// `CAM_OUT` out from the centre, looking at the centre.
#[must_use]
pub fn seat_camera(seat: u8) -> Camera {
    let corner = tile_pos(frame(seat.min(3)).corner);
    let dir = Vec3::new(corner.x, 0.0, corner.z).normalize_or_zero();
    Camera {
        eye: dir * CAM_OUT + Vec3::Y * CAM_UP,
        target: Vec3::ZERO,
        fov_y_deg: CAM_FOV,
    }
}

/// The camera that follows a seat, swinging to a new seat's corner over
/// `CAM_LERP_SECS`.
#[derive(Clone, Copy, Debug)]
pub struct SeatCamera {
    seat: u8,
    from: Vec3,
    to: Vec3,
    t: f32,
}

impl SeatCamera {
    /// A camera already at `seat`'s corner.
    #[must_use]
    pub fn new(seat: u8) -> Self {
        let eye = seat_camera(seat).eye;
        Self {
            seat,
            from: eye,
            to: eye,
            t: 1.0,
        }
    }

    /// The seat the camera is at, or swinging to.
    #[must_use]
    pub const fn seat(&self) -> u8 {
        self.seat
    }

    /// Start swinging to `seat`'s corner; a no-op for the current seat.
    pub fn retarget(&mut self, seat: u8) {
        if seat != self.seat {
            self.from = self.eye();
            self.to = seat_camera(seat).eye;
            self.t = 0.0;
            self.seat = seat;
        }
    }

    fn eye(&self) -> Vec3 {
        let k = self.t * self.t * (3.0 - 2.0 * self.t);
        self.from.lerp(self.to, k)
    }

    /// Advance the swing and hand back this frame's camera.
    pub fn tick(&mut self, dt: f32) -> Camera {
        self.t = (self.t + dt / CAM_LERP_SECS).min(1.0);
        Camera {
            eye: self.eye(),
            target: Vec3::ZERO,
            fov_y_deg: CAM_FOV,
        }
    }
}

// ---- HUD ------------------------------------------------------------------

/// Which game is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Hotseat.
    Local,
    /// A server game.
    Online,
}

/// Where the online client is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    /// Connected, not in a lobby yet.
    Browsing,
    /// In a lobby.
    Lobby,
}

/// One legal target, as the page reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HudTarget {
    /// Column.
    pub x: u8,
    /// Row.
    pub y: u8,
    /// What moving there does.
    pub k: TargetKind,
}

/// Everything the page draws, polled once per animation frame as JSON
/// (design 4.6). Coordinates are absolute board tiles; the page rotates for
/// display only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[allow(clippy::struct_excessive_bools)] // the page's contract, one flag per fact
pub struct HudState {
    /// `local` or `online`.
    pub mode: Mode,
    /// The socket is open (always true in hotseat).
    pub connected: bool,
    /// `browsing` or `lobby`.
    pub screen: Screen,
    /// `waiting`, `playing` or `finished`.
    pub phase: Phase,
    /// The winning seat once Finished; `None` for a draw.
    pub winner: Option<u8>,
    /// Why the game ended.
    pub end: Option<EndReason>,
    /// The local seat; `None` in hotseat (the page then shows seat 0
    /// bottom-left).
    pub me: Option<u8>,
    /// Lobby id of the creator.
    pub creator: u8,
    /// This client is the creator.
    pub is_creator: bool,
    /// The creator may start now.
    pub can_start: bool,
    /// Global turn number.
    pub turn: u32,
    /// Seat to move.
    pub seat: u8,
    /// Displayed clock, counted down between server messages.
    pub left_ms: u32,
    /// The local seat is to move.
    pub my_turn: bool,
    /// Completed turns without progress.
    pub quiet: u32,
    /// Consecutive forced passes.
    pub stalls: u8,
    /// The lobby's members.
    pub roster: Vec<PlayerMeta>,
    /// The four seats.
    pub seats: Vec<SeatState>,
    /// Every piece on the board.
    pub pieces: Vec<PieceState>,
    /// Each seat's joker capture tile, `None` when off the board or when the
    /// seat has no joker.
    pub joker_fl: [Option<[u8; 2]>; 4],
    /// The keyboard cursor.
    pub cursor: [u8; 2],
    /// The selected tile.
    pub sel: Option<[u8; 2]>,
    /// The selected piece's legal targets.
    pub targets: Vec<HudTarget>,
    /// The last applied action.
    pub last: Option<LastAction>,
    /// A move or formation is in flight.
    pub pending: bool,
    /// The last refusal or connection message.
    pub notice: Option<String>,
}

impl HudState {
    /// An empty HUD for a mode.
    #[must_use]
    pub const fn new(mode: Mode) -> Self {
        Self {
            mode,
            connected: true,
            screen: Screen::Lobby,
            phase: Phase::Waiting,
            winner: None,
            end: None,
            me: None,
            creator: 0,
            is_creator: false,
            can_start: false,
            turn: 0,
            seat: 0,
            left_ms: 0,
            my_turn: false,
            quiet: 0,
            stalls: 0,
            roster: Vec::new(),
            seats: Vec::new(),
            pieces: Vec::new(),
            joker_fl: [None; 4],
            cursor: [0, 0],
            sel: None,
            targets: Vec::new(),
            last: None,
            pending: false,
            notice: None,
        }
    }
}

thread_local! {
    static HUD: RefCell<Option<HudState>> = const { RefCell::new(None) };
    static CMDS: RefCell<Vec<UiCmd>> = const { RefCell::new(Vec::new()) };
}

/// The HUD published last; an empty hotseat HUD before any game started.
#[must_use]
pub fn hud() -> HudState {
    HUD.with(|h| {
        h.borrow()
            .clone()
            .unwrap_or_else(|| HudState::new(Mode::Local))
    })
}

/// Publish this frame's HUD. Both play modes go through here so the page
/// reads one shape regardless of which one is running.
pub fn set_hud(h: HudState) {
    HUD.with(|c| *c.borrow_mut() = Some(h));
}

/// The HUD as the page reads it.
#[must_use]
pub fn state_json() -> String {
    serde_json::to_string(&hud()).unwrap_or_else(|_| "{}".into())
}

/// What the page (or the keyboard) asks the game to do, queued for the next
/// update so a JS call between frames never races the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCmd {
    /// A click on the absolute tile `(x, y)`.
    Click(u8, u8),
    /// The creator's Start button.
    Start,
    /// Esc, or a click beside the board.
    Clear,
}

/// Queue a command for the next update.
pub fn push_cmd(cmd: UiCmd) {
    CMDS.with(|c| c.borrow_mut().push(cmd));
}

/// Take every queued command, in order.
#[must_use]
pub fn drain_cmds() -> Vec<UiCmd> {
    CMDS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Copy the board half of the HUD from a wire snapshot, the state it
/// decodes to, and the view.
pub fn fill_board(hud: &mut HudState, wire: &BoardState, state: &State, view: &View) {
    hud.turn = wire.turn;
    hud.seat = wire.seat;
    hud.quiet = wire.quiet;
    hud.stalls = wire.stalls;
    hud.seats.clone_from(&wire.seats);
    hud.pieces.clone_from(&wire.pieces);
    hud.last = wire.last;
    hud.joker_fl = joker_capture_tiles(state);
    // Both games always show the cursor; a hidden one reports the corner.
    hud.cursor = view.cursor.map_or([0, 0], |t| [t.x, t.y]);
    hud.sel = view.sel.map(|t| [t.x, t.y]);
    hud.targets = view
        .targets
        .iter()
        .map(|t| HudTarget {
            x: t.x,
            y: t.y,
            k: t.kind,
        })
        .collect();
}

/// Each seat's joker capture tile: its joker's front-left in the owner's
/// frame, `None` off the board or without a joker.
#[must_use]
pub fn joker_capture_tiles(state: &State) -> [Option<[u8; 2]>; 4] {
    let mut out = [None; 4];
    for (seat, slot) in (0..4u8).zip(out.iter_mut()) {
        *slot = state
            .pieces_of(seat)
            .find(|(_, p)| p.kind == Kind::Joker)
            .and_then(|(t, _)| t.offset(front_left(seat)))
            .map(|t| [t.x, t.y]);
    }
    out
}

/// A state with an empty board and four absent seats: what the online
/// client shows before its first `State`.
#[must_use]
pub fn empty_state() -> State {
    State {
        board: [None; TILES],
        seats: std::array::from_fn(|_| Seat::default()),
        to_move: 0,
        turn: 1,
        quiet: 0,
        stalls: 0,
        clock: TurnClock::new(),
        last: None,
        result: None,
    }
}

// ---- keyboard cursor ------------------------------------------------------

/// The keys the cursor reads, sampled once per frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // one flag per key
pub struct CursorKeys {
    /// Right arrow or D: `+u`, forward.
    pub right: bool,
    /// Left arrow or A: `-u`.
    pub left: bool,
    /// Up arrow or W: `+v`, left.
    pub up: bool,
    /// Down arrow or S: `-v`.
    pub down: bool,
    /// Enter or Space: click at the cursor.
    pub click: bool,
    /// Esc: clear.
    pub clear: bool,
}

impl CursorKeys {
    /// Sample the engine's input.
    #[must_use]
    pub fn read(input: &InputState) -> Self {
        let held = |a: KeyCode, b: KeyCode| input.down(a) || input.down(b);
        Self {
            right: held(KeyCode::ArrowRight, KeyCode::KeyD),
            left: held(KeyCode::ArrowLeft, KeyCode::KeyA),
            up: held(KeyCode::ArrowUp, KeyCode::KeyW),
            down: held(KeyCode::ArrowDown, KeyCode::KeyS),
            click: held(KeyCode::Enter, KeyCode::Space),
            clear: input.down(KeyCode::Escape),
        }
    }
}

/// What a frame of keyboard input asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorCmd {
    /// Enter or Space: a click on the cursor's tile.
    Click(Tile),
    /// Esc.
    Clear,
}

/// The tile `du` forward and `dv` left of `tile` in `seat`'s frame, clamped
/// to the board.
#[must_use]
pub fn shifted(tile: Tile, seat: u8, du: i8, dv: i8) -> Tile {
    let seat = seat.min(3);
    let (u, v) = to_local(seat, tile);
    let clamp = |c: u8, d: i8| {
        let n = (i16::from(c) + i16::from(d)).clamp(0, i16::from(SIDE) - 1);
        // Clamped into 0..=9.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = n as u8;
        n
    };
    to_global(seat, clamp(u, du), clamp(v, dv))
}

/// The keyboard cursor, moving in the LOCAL seat's frame.
///
/// Arrows and WASD move it one tile: Right is `+u`, forward; Up is `+v`,
/// left, which is how the page lays the board out with the local corner
/// bottom-left. Rising-edge latched, so a held key moves once.
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    /// Where the cursor is.
    pub tile: Tile,
    was: CursorKeys,
}

impl Cursor {
    /// A cursor on `seat`'s home elbow, local `(3, 3)`.
    #[must_use]
    pub fn new(seat: u8) -> Self {
        Self {
            tile: to_global(seat.min(3), 3, 3),
            was: CursorKeys::default(),
        }
    }

    /// Feed one frame of keys; the cursor moves in `seat`'s frame.
    pub fn step(&mut self, keys: CursorKeys, seat: u8) -> Option<CursorCmd> {
        let edge = |now: bool, was: bool| now && !was;
        let du =
            i8::from(edge(keys.right, self.was.right)) - i8::from(edge(keys.left, self.was.left));
        let dv = i8::from(edge(keys.up, self.was.up)) - i8::from(edge(keys.down, self.was.down));
        if du != 0 || dv != 0 {
            self.tile = shifted(self.tile, seat, du, dv);
        }
        let cmd = if edge(keys.click, self.was.click) {
            Some(CursorCmd::Click(self.tile))
        } else if edge(keys.clear, self.was.clear) {
            Some(CursorCmd::Clear)
        } else {
            None
        };
        self.was = keys;
        cmd
    }
}

// ---- the scene ------------------------------------------------------------

/// The view-only part of a frame: what is selected, where the cursor is.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// The selected tile.
    pub sel: Option<Tile>,
    /// Its legal targets.
    pub targets: Vec<Target>,
    /// The keyboard cursor; `None` hides the ring.
    pub cursor: Option<Tile>,
}

fn block_of(t: Tile) -> Option<u8> {
    (0..4u8).find(|&s| in_home_block(s, t))
}

fn push_tiles(frame: &mut Frame, meshes: &Meshes) {
    for t in Tile::all() {
        let base = if t.colour() == 0 {
            TILE_DARK
        } else {
            TILE_LIGHT
        };
        let colour = block_of(t).map_or(base, |s| base.lerp(seat_colour(s), BLOCK_TINT));
        frame
            .instances
            .push(Instance::new(tile_pos(t), Vec3::ONE, colour).with_mesh(meshes.tile));
    }
}

fn push_pieces(frame: &mut Frame, state: &State, meshes: &Meshes) {
    for (i, slot) in state.board.iter().enumerate() {
        let (Some(p), Some(t)) = (slot, Tile::from_index(i)) else {
            continue;
        };
        let mut colour = seat_colour(p.owner);
        if state
            .seats
            .get(usize::from(p.owner))
            .is_some_and(|s| s.garrison)
        {
            colour *= GARRISON_DIM;
        }
        let pos = tile_pos(t);
        let scale = if p.kind == Kind::HeroAwake {
            frame
                .instances
                .push(Instance::new(pos, Vec3::new(0.8, 1.0, 0.8), colour).with_mesh(meshes.ring));
            1.2
        } else {
            1.0
        };
        frame.instances.push(
            Instance::new(pos, Vec3::splat(scale), colour)
                .with_yaw(seat_yaw(p.owner))
                .with_mesh(meshes.of(p.kind)),
        );
    }
}

fn push_marks(frame: &mut Frame, state: &State, meshes: &Meshes, view: &View) {
    if let Some(last) = state.last
        && !matches!(last.kind, ActionKind::Pass | ActionKind::Timeout)
    {
        for (x, y, s) in [(last.fx, last.fy, 0.6), (last.tx, last.ty, 0.85)] {
            if let Some(t) = Tile::new(x, y) {
                frame.instances.push(
                    Instance::new(tile_pos(t), Vec3::new(s, 1.0, s), LAST_COLOUR)
                        .with_mesh(meshes.disc),
                );
            }
        }
    }
    for target in &view.targets {
        if let Some(t) = Tile::new(target.x, target.y) {
            frame.instances.push(
                Instance::new(tile_pos(t), Vec3::ONE, target_colour(target.kind))
                    .with_mesh(meshes.disc),
            );
        }
    }
    if let Some(t) = view.sel {
        frame
            .instances
            .push(Instance::new(tile_pos(t), Vec3::ONE, SEL_COLOUR).with_mesh(meshes.ring));
    }
    if let Some(t) = view.cursor {
        frame.instances.push(
            Instance::new(tile_pos(t), Vec3::new(0.88, 1.0, 0.88), CURSOR_COLOUR)
                .with_mesh(meshes.ring),
        );
    }
}

/// Build the whole frame (design 4.4).
///
/// 100 tiles, one instance per piece, a disc per legal target, a ring on
/// the selected tile and on the cursor, two dim discs on the last move.
/// Shared by hotseat and online play so the two cannot drift apart
/// visually. No text: the page draws the narrative.
#[must_use]
pub fn scene(state: &State, meshes: &Meshes, view: &View, camera: Camera) -> Frame {
    let mut frame = Frame {
        camera,
        instances: Vec::with_capacity(TILES + 64 + 40),
    };
    push_tiles(&mut frame, meshes);
    push_pieces(&mut frame, state, meshes);
    push_marks(&mut frame, state, meshes, view);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use kings_core::board::to_state;
    use kings_core::proto::Formation;
    use kings_core::setup;

    /// The mesh-id struct and the registration order are written out twice
    /// and must agree; if they drift, every piece draws as the wrong shape.
    #[test]
    fn mesh_ids_match_registration_order() {
        let (list, ids) = build_meshes();
        assert_eq!(list.len(), 12, "mesh count changed: update the id struct");
        let order = [
            ids.tile,
            ids.disc,
            ids.ring,
            ids.king,
            ids.queen,
            ids.rook,
            ids.bishop,
            ids.knight,
            ids.pawn,
            ids.joker,
            ids.hero,
            ids.hero_awake,
        ];
        for (i, id) in order.iter().enumerate() {
            let expected = u32::try_from(i).expect("fits") + 1;
            assert_eq!(*id, expected, "mesh id {id} is not at slot {expected}");
        }
        for kind in [
            Kind::King,
            Kind::Queen,
            Kind::Rook,
            Kind::Bishop,
            Kind::Knight,
            Kind::Pawn,
            Kind::Joker,
            Kind::Hero,
            Kind::HeroAwake,
        ] {
            let id = ids.of(kind);
            assert!((4..=12).contains(&id), "{kind:?} -> {id}");
        }
        for (i, mesh) in list.iter().enumerate() {
            assert!(!mesh.vertices.is_empty(), "mesh slot {} is empty", i + 1);
            assert_eq!(
                mesh.vertices.len() % 3,
                0,
                "slot {} not a triangle list",
                i + 1
            );
            for v in &mesh.vertices {
                let n = Vec3::from(v.normal);
                assert!((n.length() - 1.0).abs() < 1e-4, "slot {}: {n}", i + 1);
            }
        }
    }

    #[test]
    fn tiles_are_centred_with_unit_pitch_and_y_up() {
        let a = tile_pos(Tile::at(0, 0));
        let b = tile_pos(Tile::at(9, 9));
        assert_eq!(a + b, Vec3::ZERO, "the board is centred on the origin");
        assert!(a.y.abs() < 1e-6);
        let east = tile_pos(Tile::at(1, 0)) - a;
        let north = tile_pos(Tile::at(0, 1)) - a;
        assert_eq!(east, Vec3::new(TILE_PITCH, 0.0, 0.0));
        assert_eq!(north, Vec3::new(0.0, 0.0, -TILE_PITCH));
    }

    #[test]
    fn the_camera_sits_on_each_seats_corner_diagonal() {
        for seat in 0..4u8 {
            let cam = seat_camera(seat);
            let corner = tile_pos(frame(seat).corner);
            assert!((cam.eye.y - 12.0).abs() < 1e-5, "seat {seat} up");
            let out = Vec3::new(cam.eye.x, 0.0, cam.eye.z);
            assert!((out.length() - 11.0).abs() < 1e-4, "seat {seat} out");
            assert!(
                out.dot(corner) > 0.0,
                "seat {seat} eye is over its own corner"
            );
            assert!(
                (out.normalize().dot(corner.normalize()) - 1.0).abs() < 1e-5,
                "seat {seat} eye is on the corner diagonal"
            );
            assert_eq!(cam.target, Vec3::ZERO);
            assert!((cam.fov_y_deg - 50.0).abs() < 1e-5);
        }
        assert_ne!(seat_camera(0).eye, seat_camera(2).eye);
    }

    #[test]
    fn the_seat_camera_swings_over_the_lerp_time() {
        let mut cam = SeatCamera::new(0);
        let start = cam.tick(0.0).eye;
        cam.retarget(2);
        assert_eq!(cam.seat(), 2);
        let mid = cam.tick(CAM_LERP_SECS * 0.5).eye;
        assert!(mid.distance(start) > 0.5, "moved off the start");
        assert!(mid.distance(seat_camera(2).eye) > 0.5, "not there yet");
        let end = cam.tick(CAM_LERP_SECS).eye;
        assert!(end.distance(seat_camera(2).eye) < 1e-4, "arrived");
        // Retargeting to the same seat is a no-op.
        cam.retarget(2);
        assert!(cam.tick(0.0).eye.distance(end) < 1e-6);
    }

    #[test]
    fn the_scene_has_a_tile_per_square_and_a_piece_per_piece() {
        let state = setup([true, false, true, true], [Formation::DEFAULT; 4]);
        let (_, ids) = build_meshes();
        let view = View {
            sel: Some(Tile::at(3, 0)),
            targets: vec![Target {
                x: 4,
                y: 0,
                kind: TargetKind::Move,
            }],
            cursor: Some(Tile::at(3, 3)),
        };
        let frame = scene(&state, &ids, &view, seat_camera(0));
        let count = |mesh: u32| frame.instances.iter().filter(|i| i.mesh == mesh).count();
        assert_eq!(count(ids.tile), 100);
        assert_eq!(count(ids.disc), 1, "one target, no last move yet");
        assert_eq!(count(ids.ring), 2, "selection and cursor");
        assert_eq!(count(ids.pawn), 28);
        assert_eq!(count(ids.king), 4);
        assert_eq!(frame.instances.len(), 100 + 64 + 1 + 2);
        for i in &frame.instances {
            assert!(i.position.is_finite());
        }
        // A garrison king is dimmer than a seated one.
        let king_at = |t: Tile| {
            frame
                .instances
                .iter()
                .find(|i| i.mesh == ids.king && i.position.distance(tile_pos(t)) < 1e-4)
                .expect("king")
                .color
        };
        assert!(king_at(Tile::at(9, 0)).length() < king_at(Tile::at(0, 0)).length());
        // Corner tiles lean toward their seat colour; the cross does not.
        let tile_at = |t: Tile| {
            frame
                .instances
                .iter()
                .find(|i| i.mesh == ids.tile && i.position.distance(tile_pos(t)) < 1e-4)
                .expect("tile")
                .color
        };
        let corner = tile_at(Tile::at(0, 0));
        let cross = tile_at(Tile::at(4, 4));
        assert_ne!(corner, cross);
        assert!(
            (corner - TILE_DARK).dot(seat_colour(0) - TILE_DARK) > 0.0,
            "tinted toward gold"
        );
    }

    #[test]
    fn an_awake_hero_is_bigger_and_stands_on_a_ring() {
        let mut state = setup([true; 4], [Formation::DEFAULT; 4]);
        let hero = Tile::at(0, 1);
        let mut p = state.piece(hero).unwrap();
        p.kind = Kind::HeroAwake;
        state.set(hero, Some(p));
        state.last = Some(LastAction {
            seat: 0,
            kind: ActionKind::HeroSwap,
            fx: 0,
            fy: 1,
            tx: 0,
            ty: 1,
            captured: None,
            promoted: false,
            eliminated: None,
        });
        let (_, ids) = build_meshes();
        let frame = scene(&state, &ids, &View::default(), seat_camera(0));
        let big = frame
            .instances
            .iter()
            .find(|i| i.mesh == ids.hero_awake)
            .expect("awake hero drawn");
        assert!((big.scale.x - 1.2).abs() < 1e-6);
        assert_eq!(
            frame
                .instances
                .iter()
                .filter(|i| i.mesh == ids.ring)
                .count(),
            1
        );
        assert_eq!(
            frame
                .instances
                .iter()
                .filter(|i| i.mesh == ids.disc)
                .count(),
            2,
            "the last move's two discs"
        );
        // A pass draws no last-move discs.
        state.last = Some(LastAction {
            seat: 1,
            kind: ActionKind::Timeout,
            fx: 0,
            fy: 0,
            tx: 0,
            ty: 0,
            captured: None,
            promoted: false,
            eliminated: None,
        });
        let frame = scene(&state, &ids, &View::default(), seat_camera(0));
        assert_eq!(
            frame
                .instances
                .iter()
                .filter(|i| i.mesh == ids.disc)
                .count(),
            0
        );
    }

    /// Right is `+u` (forward) and Up is `+v` (left) in the local seat's
    /// frame, so that on the page's rotated grid the cursor moves the way
    /// the arrow points from every seat.
    #[test]
    fn the_cursor_moves_in_the_local_frame_on_rising_edges() {
        for seat in 0..4u8 {
            let mut cur = Cursor::new(seat);
            assert_eq!(to_local(seat, cur.tile), (3, 3));
            let right = CursorKeys {
                right: true,
                ..CursorKeys::default()
            };
            assert_eq!(cur.step(right, seat), None);
            assert_eq!(to_local(seat, cur.tile), (4, 3), "seat {seat} right is +u");
            // Held: no second step.
            cur.step(right, seat);
            assert_eq!(to_local(seat, cur.tile), (4, 3), "seat {seat} held key");
            cur.step(CursorKeys::default(), seat);
            let up = CursorKeys {
                up: true,
                ..CursorKeys::default()
            };
            cur.step(up, seat);
            assert_eq!(to_local(seat, cur.tile), (4, 4), "seat {seat} up is +v");
            cur.step(CursorKeys::default(), seat);
            // Clamped at the far edges and at the corner.
            for _ in 0..12 {
                cur.step(
                    CursorKeys {
                        left: true,
                        down: true,
                        ..CursorKeys::default()
                    },
                    seat,
                );
                cur.step(CursorKeys::default(), seat);
            }
            assert_eq!(cur.tile, frame(seat).corner, "seat {seat} corner");
            for _ in 0..12 {
                cur.step(right, seat);
                cur.step(CursorKeys::default(), seat);
            }
            assert_eq!(to_local(seat, cur.tile), (9, 0), "seat {seat} far edge");
            // Enter clicks where the cursor is; Esc clears; both once.
            let click = CursorKeys {
                click: true,
                ..CursorKeys::default()
            };
            assert_eq!(cur.step(click, seat), Some(CursorCmd::Click(cur.tile)));
            assert_eq!(cur.step(click, seat), None);
            cur.step(CursorKeys::default(), seat);
            let clear = CursorKeys {
                clear: true,
                ..CursorKeys::default()
            };
            assert_eq!(cur.step(clear, seat), Some(CursorCmd::Clear));
            assert_eq!(cur.step(clear, seat), None);
        }
        // The literal mapping for seat 1: forward is north, left is west.
        assert_eq!(shifted(Tile::at(9, 0), 1, 1, 0), Tile::at(9, 1));
        assert_eq!(shifted(Tile::at(9, 0), 1, 0, 1), Tile::at(8, 0));
    }

    #[test]
    fn the_command_queue_is_fifo_and_drains() {
        drop(drain_cmds());
        push_cmd(UiCmd::Click(1, 2));
        push_cmd(UiCmd::Start);
        push_cmd(UiCmd::Clear);
        assert_eq!(
            drain_cmds(),
            vec![UiCmd::Click(1, 2), UiCmd::Start, UiCmd::Clear]
        );
        assert_eq!(drain_cmds(), Vec::new());
    }

    #[test]
    fn joker_capture_tiles_follow_the_owners_frame() {
        let state = setup([true; 4], [Formation::DEFAULT; 4]);
        assert_eq!(
            joker_capture_tiles(&state),
            [Some([2, 2]), Some([7, 2]), Some([7, 7]), Some([2, 7])]
        );
        let mut state = state;
        // Seat 0's joker on its far edge has no capture; seat 2 loses its joker.
        let joker = state.piece(Tile::at(1, 1)).unwrap();
        state.set(Tile::at(1, 1), None);
        state.set(Tile::at(9, 3), Some(joker));
        state.set(Tile::at(8, 8), None);
        assert_eq!(
            joker_capture_tiles(&state),
            [None, Some([7, 2]), None, Some([2, 7])]
        );
    }

    /// The page reads these keys and values verbatim (design 4.6): pin the
    /// whole shape in the encode direction.
    #[test]
    fn state_json_has_exactly_the_pages_keys() {
        let state = setup([true, false, true, true], [Formation::DEFAULT; 4]);
        let wire = to_state(&state);
        let mut hud = HudState::new(Mode::Online);
        hud.screen = Screen::Lobby;
        hud.phase = Phase::Finished;
        hud.winner = Some(2);
        hud.end = Some(EndReason::NoProgress);
        hud.me = Some(0);
        hud.creator = 0;
        hud.is_creator = true;
        hud.roster = vec![PlayerMeta {
            id: 0,
            handle: "ada".into(),
            seat: 0,
        }];
        hud.left_ms = 8_400;
        hud.notice = Some("not your turn".into());
        let view = View {
            sel: Some(Tile::at(2, 3)),
            targets: vec![Target {
                x: 2,
                y: 4,
                kind: TargetKind::Move,
            }],
            cursor: Some(Tile::at(2, 3)),
        };
        fill_board(&mut hud, &wire, &state, &view);
        hud.last = Some(LastAction {
            seat: 0,
            kind: ActionKind::Move,
            fx: 3,
            fy: 0,
            tx: 3,
            ty: 1,
            captured: None,
            promoted: false,
            eliminated: None,
        });
        set_hud(hud.clone());
        let json = state_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = v.as_object().unwrap();
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = vec![
            "mode",
            "connected",
            "screen",
            "phase",
            "winner",
            "end",
            "me",
            "creator",
            "is_creator",
            "can_start",
            "turn",
            "seat",
            "left_ms",
            "my_turn",
            "quiet",
            "stalls",
            "roster",
            "seats",
            "pieces",
            "joker_fl",
            "cursor",
            "sel",
            "targets",
            "last",
            "pending",
            "notice",
        ];
        expected.sort_unstable();
        assert_eq!(keys, expected);
        // The value strings the page switches on.
        assert!(json.starts_with(r#"{"mode":"online","connected":true,"screen":"lobby","phase":"finished","winner":2,"end":"no_progress","me":0,"creator":0,"is_creator":true,"can_start":false,"turn":1,"seat":0,"left_ms":8400,"my_turn":false,"quiet":0,"stalls":0,"roster":[{"id":0,"handle":"ada","seat":0}],"seats":[{"seat":0,"present":true,"alive":true,"garrison":false,"own_turns":1,"timeouts":0,"captured":[]},"#), "{json}");
        assert!(
            json.contains(r#""pieces":[{"id":0,"owner":0,"kind":"king","x":0,"y":0},"#),
            "{json}"
        );
        assert!(json.contains(r#""joker_fl":[[2,2],[7,2],[7,7],[2,7]],"cursor":[2,3],"sel":[2,3],"targets":[{"x":2,"y":4,"k":"move"}],"last":{"seat":0,"kind":"move","fx":3,"fy":0,"tx":3,"ty":1,"captured":null,"promoted":false,"eliminated":null},"pending":false,"notice":"not your turn"}"#), "{json}");
        assert_eq!(obj["seats"].as_array().unwrap().len(), 4);
        assert_eq!(obj["pieces"].as_array().unwrap().len(), 64);
        // A null joker_fl entry and a null selection encode as null.
        let mut bare = HudState::new(Mode::Local);
        bare.joker_fl = [None, Some([1, 2]), None, None];
        let s = serde_json::to_string(&bare).unwrap();
        assert!(s.contains(r#""joker_fl":[null,[1,2],null,null]"#), "{s}");
        assert!(
            s.contains(r#""sel":null"#) && s.contains(r#""me":null"#),
            "{s}"
        );
        assert!(s.contains(r#""mode":"local""#), "{s}");
    }

    /// The value strings the page switches on, for every enum it reads.
    #[test]
    fn state_json_value_strings_are_the_pages() {
        assert_eq!(serde_json::to_string(&Mode::Local).unwrap(), r#""local""#);
        assert_eq!(
            serde_json::to_string(&Screen::Browsing).unwrap(),
            r#""browsing""#
        );
        assert_eq!(
            serde_json::to_string(&Phase::Waiting).unwrap(),
            r#""waiting""#
        );
        assert_eq!(
            serde_json::to_string(&Phase::Playing).unwrap(),
            r#""playing""#
        );
        for (k, s) in [
            (TargetKind::Capture, "capture"),
            (TargetKind::Teleport, "teleport"),
            (TargetKind::Place, "place"),
            (TargetKind::Swap, "swap"),
            (TargetKind::Wake, "wake"),
        ] {
            assert_eq!(serde_json::to_string(&k).unwrap(), format!("\"{s}\""));
        }
        for (k, s) in [
            (ActionKind::JokerTeleport, "joker_teleport"),
            (ActionKind::JokerPlace, "joker_place"),
            (ActionKind::HeroSwap, "hero_swap"),
            (ActionKind::HeroWake, "hero_wake"),
            (ActionKind::Pass, "pass"),
            (ActionKind::Timeout, "timeout"),
        ] {
            assert_eq!(serde_json::to_string(&k).unwrap(), format!("\"{s}\""));
        }
        for (k, s) in [
            (EndReason::LastKing, "last_king"),
            (EndReason::Stalemate, "stalemate"),
            (EndReason::TurnCap, "turn_cap"),
            (EndReason::Abandoned, "abandoned"),
        ] {
            assert_eq!(serde_json::to_string(&k).unwrap(), format!("\"{s}\""));
        }
        assert_eq!(
            serde_json::to_string(&Kind::HeroAwake).unwrap(),
            r#""hero_awake""#
        );
    }
}
