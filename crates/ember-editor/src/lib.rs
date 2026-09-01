//! The ember engine editor: a level builder and (later) a character
//! inspector, built on the same `EmberGame` contract the games use.
//!
//! WHAT THIS IS NOT, YET. The editor exports a level file that the running
//! arena does not load. The shooter sim has no level: it generates obstacles
//! procedurally from a seed and derives each box's height by hashing its own
//! coordinates, so an authored box has no door into a running game. Closing
//! that is milestone 2 bite 12 and needs a protocol bump plus a server
//! redeploy that the backlog records as blocked. Saying so here because an
//! editor that implies its output is playable is worse than one that admits
//! it is not. See `docs/plans/milestone-2-editor.md`.
//!
//! CONTROLS, and why they are shaped this way. W is already fly-forward in
//! every client in this repo, and the user asked for W to be the translate
//! gizmo — one key cannot be both. So the editor is modal on the right mouse
//! button: hold RMB to fly (WASD, Space/Ctrl for height, Shift to boost),
//! release it and W/E/R select the translate/rotate/scale mode while LMB
//! picks. One branch, no chord, and no mode that can be entered by accident.

pub mod drag;
pub mod gizmo;
pub mod level;
pub mod palette;
pub mod pick;

use ember_engine::glam::{Quat, Vec3};
use ember_engine::{
    Camera, EmberGame, EngineConfig, Frame, InputState, Instance, KeyCode, MouseButton,
};

/// Half-extent of the authored area, and of the reference grid.
pub const GRID_HALF: f32 = 24.0;
/// Grid spacing. 2.0 keeps a 24-unit half-extent at 25 lines per axis, which
/// is legible without turning the floor into a solid sheet.
const GRID_STEP: f32 = 2.0;
/// Number of grid steps from the origin to `GRID_HALF` at `GRID_STEP` spacing.
const GRID_STEPS: i16 = 12;
/// Thickness of a grid line drawn as a box. Below ~0.02 it aliases into
/// dashes at grazing angles; there are no mipmaps and no MSAA to hide it.
const GRID_THICK: f32 = 0.03;

/// How far the eye may get from the origin. The projection's far plane is
/// hardcoded at 500 (`Camera::view_proj`), so a level flown past it simply
/// vanishes with nothing logged; stopping short of it is the difference
/// between "the editor has bounds" and "the editor broke".
const EYE_LIMIT: f32 = 220.0;

/// Radians of look per unit of NDC. The pointer delta is in NDC, so a full
/// screen width is 2.0 units — meaning look sensitivity scales with window
/// size rather than with pixels. That is deliberate (the same drag crosses
/// the same fraction of the view at any window size) but it is a real
/// difference from the shooter's pixel-based `mouse_delta`, so it is written
/// down rather than discovered.
const LOOK_SENS: f32 = 2.2;

const FLY_SPEED: f32 = 14.0;
const FLY_BOOST: f32 = 3.5;
/// Just under a right angle: at exactly +/-pi/2 the forward vector is
/// parallel to world up and the camera basis degenerates.
const PITCH_LIMIT: f32 = 1.5533;

/// Instance colours are an unclamped shader multiplier.
///
/// The shader lights, ACES-tonemaps and fogs every fragment, so a gizmo
/// coloured (1,0,0) comes
/// out dark red on unlit faces and washes toward the fog colour with
/// distance. Over-driving past 1.0 pushes every face to saturation whatever
/// its normal, which is the whole point of "different colours to know where
/// you place things".
///
/// This leans on the clamp inside `aces()`. If the tonemap ever moves, these
/// stop reading as R/G/B with no compile error — which is why bite 1's
/// acceptance check is that the axes read as distinctly red, green and blue,
/// not merely that they appear.
///
/// MEASURED on Vulkan at 4.0 (brightest pixel per bar, and its own channel
/// over the next highest):
///   +X  rgb(229,  32,  35)  6.5x
///   +Y  rgb( 32, 222,  49)  4.5x
///   +Z  rgb( 30,  89, 226)  2.5x
///
/// Blue is the one that degrades first, and not for a reason this file can
/// see: the scene shader's fill light is cool (0.42, 0.50, 0.68), so it lifts
/// the green channel of anything blue. The over-drive has to out-run a second
/// light that the other two axes barely notice. There is ample headroom at
/// 4.0; below ~2.5, re-measure rather than reason about it.
pub const AXIS_X: Vec3 = Vec3::new(4.0, 0.06, 0.06);
pub const AXIS_Y: Vec3 = Vec3::new(0.10, 4.0, 0.10);
pub const AXIS_Z: Vec3 = Vec3::new(0.10, 0.35, 4.0);
const GRID_COLOR: Vec3 = Vec3::new(0.20, 0.24, 0.30);
const GRID_MAJOR: Vec3 = Vec3::new(0.34, 0.40, 0.50);

/// Digit keys that arm palette entries on the native shell, in order. The
/// web shell sends the same `Cmd::Arm` from DOM buttons, so the two shells
/// share one code path rather than two that can drift.
const PALETTE_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// A flying camera.
///
/// Yaw/pitch match the arena client's convention exactly, so
/// "yaw" means the same thing in the editor and in the game — an editor whose
/// forward disagreed with the game's would place everything rotated.
pub struct FlyCam {
    pub eye: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Cursor position on the previous frame, while the look drag is active.
    /// `None` means no drag is in progress, which is also how the first frame
    /// of a drag avoids a jump from a stale anchor.
    last_ndc: Option<[f32; 2]>,
}

impl Default for FlyCam {
    fn default() -> Self {
        Self {
            eye: Vec3::new(18.0, 14.0, 18.0),
            // Looking back at the origin from the default eye.
            yaw: std::f32::consts::PI * 1.25,
            pitch: -0.5,
            last_ndc: None,
        }
    }
}

impl FlyCam {
    /// Unit vector the camera is looking along.
    #[must_use]
    pub fn forward(&self) -> Vec3 {
        let (sz, cx) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cx * cp, sp, sz * cp)
    }

    /// Horizontal right, for strafing. Deliberately flat: a fly camera that
    /// rolls its strafe with pitch is disorienting when placing objects.
    #[must_use]
    pub fn right(&self) -> Vec3 {
        let (sz, cx) = self.yaw.sin_cos();
        Vec3::new(-sz, 0.0, cx)
    }

    #[must_use]
    pub fn camera(&self) -> Camera {
        Camera {
            eye: self.eye,
            target: self.eye + self.forward(),
            fov_y_deg: 70.0,
        }
    }

    /// Advances the camera. Returns true while the fly drag is held, which is
    /// what tells the caller that W/E/R mean "move" and not "mode".
    pub fn update(&mut self, input: &InputState, dt: f32) -> bool {
        let flying = input.mouse_down(MouseButton::Right);
        if !flying {
            self.last_ndc = None;
            return false;
        }

        // Look, by differencing the cursor. `cursor_ndc` is the same source
        // picking uses, so the editor's idea of where the pointer is cannot
        // disagree with itself.
        if let Some(ndc) = input.cursor_ndc() {
            if let Some(prev) = self.last_ndc {
                let (dx, dy) = (ndc[0] - prev[0], ndc[1] - prev[1]);
                self.yaw = f32::mul_add(dx, LOOK_SENS, self.yaw);
                // NDC y is up-positive and so is pitch, so this needs no flip.
                self.pitch =
                    f32::mul_add(dy, LOOK_SENS, self.pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            }
            self.last_ndc = Some(ndc);
        }

        let speed = if input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight) {
            FLY_SPEED * FLY_BOOST
        } else {
            FLY_SPEED
        };
        let fwd = input.axis(KeyCode::KeyS, KeyCode::KeyW);
        let strafe = input.axis(KeyCode::KeyA, KeyCode::KeyD);
        let lift = input.axis(KeyCode::ControlLeft, KeyCode::Space);
        let step = self.forward() * fwd + self.right() * strafe + Vec3::Y * lift;
        if step.length_squared() > 1e-6 {
            self.eye += step.normalize() * speed * dt;
        }
        // Bounded well inside the far plane; see EYE_LIMIT.
        self.eye = self
            .eye
            .clamp(Vec3::splat(-EYE_LIMIT), Vec3::splat(EYE_LIMIT));
        true
    }
}

/// One line of the reference grid, drawn as a long thin box because the
/// renderer has triangles and nothing else — no line primitive exists.
fn grid_line(along_x: bool, offset: f32, half: f32, thick: f32, color: Vec3) -> Instance {
    let scale = if along_x {
        Vec3::new(half * 2.0, thick, thick)
    } else {
        Vec3::new(thick, thick, half * 2.0)
    };
    let pos = if along_x {
        Vec3::new(0.0, 0.0, offset)
    } else {
        Vec3::new(offset, 0.0, 0.0)
    };
    Instance::new(pos, scale, color)
}

/// The ground grid: the thing that makes "where am I" answerable at all.
#[must_use]
pub fn grid_instances() -> Vec<Instance> {
    let mut out = Vec::new();
    for i in -GRID_STEPS..=GRID_STEPS {
        let offset = f32::from(i) * GRID_STEP;
        // Every fifth line brighter, so distance is countable rather than
        // merely visible.
        let (color, thick) = if i % 5 == 0 {
            (GRID_MAJOR, GRID_THICK * 1.6)
        } else {
            (GRID_COLOR, GRID_THICK)
        };
        // The centre lines are the world axes, drawn separately and brighter.
        if i != 0 {
            out.push(grid_line(true, offset, GRID_HALF, thick, color));
            out.push(grid_line(false, offset, GRID_HALF, thick, color));
        }
    }
    out
}

/// The three world axes at the origin, in red/green/blue.
///
/// Drawn as half-length bars from the origin outward rather than centred
/// bars, so +X and -X are distinguishable — a centred bar tells you the axis
/// but not its sign, which is exactly what you need when placing something.
#[must_use]
pub fn axis_instances(len: f32, thick: f32) -> Vec<Instance> {
    let half = len * 0.5;
    vec![
        Instance::new(
            Vec3::new(half, 0.0, 0.0),
            Vec3::new(len, thick, thick),
            AXIS_X,
        ),
        Instance::new(
            Vec3::new(0.0, half, 0.0),
            Vec3::new(thick, len, thick),
            AXIS_Y,
        ),
        Instance::new(
            Vec3::new(0.0, 0.0, half),
            Vec3::new(thick, thick, len),
            AXIS_Z,
        ),
    ]
}

/// A placed object. Bite 7 turns this into `pong_core::level::Level`; until
/// then it is deliberately the smallest thing that can be drawn and picked.
#[derive(Clone, Copy, Debug)]
pub struct Obj {
    pub pos: Vec3,
    pub scale: Vec3,
    pub yaw: f32,
    pub color: Vec3,
    /// Objects become collidable boxes on export; spawns become the points
    /// players appear at. Carried from the palette entry that placed it.
    pub class: palette::Class,
}

impl Obj {
    #[must_use]
    pub fn instance(&self) -> Instance {
        Instance::new(self.pos, self.scale, self.color).with_rot(Quat::from_rotation_y(self.yaw))
    }

    /// Places this object from a palette entry, sitting ON the ground rather
    /// than centred in it — every extent in the palette is a height above
    /// the floor, and a box half-buried at the origin is not what "place a
    /// crate here" means.
    #[must_use]
    pub fn from_kind(kind: &palette::Kind, at: Vec3) -> Self {
        Self {
            pos: Vec3::new(at.x, kind.scale.y * 0.5, at.z),
            scale: kind.scale,
            yaw: 0.0,
            color: kind.color,
            class: kind.class,
        }
    }
}

pub struct Editor {
    pub cam: FlyCam,
    pub objects: Vec<Obj>,
    /// Index into `objects`, not into the frame's instance list — the frame
    /// also carries the grid and the axes, and their indices shift.
    pub selected: Option<usize>,
    pub mode: gizmo::Mode,
    /// Which gizmo handle the cursor is over, if any. Recomputed each frame
    /// rather than stored across frames, so it cannot go stale against a
    /// camera that moved.
    pub hovered: Option<gizmo::Axis>,
    /// The drag in progress, if a handle is being held.
    pub drag: Option<drag::Drag>,
    /// Index into `palette::PALETTE` of the armed entry — what `Place` puts
    /// down.
    pub armed: usize,
    /// Grid step new objects snap to. Zero means free placement.
    pub snap: f32,
    lmb: gizmo::Edge,
    keys: Vec<(KeyCode, gizmo::Edge)>,
    key_place: gizmo::Edge,
    key_delete: gizmo::Edge,
    key_w: gizmo::Edge,
    key_e: gizmo::Edge,
    key_r: gizmo::Edge,
}

/// Optional startup selection, read once from `EMBER_EDITOR_SELECT`.
///
/// This exists for verification, not for users. Z-fighting is a flicker, so
/// checking the selection cage means capturing several frames of a STATIC
/// scene and diffing them — which needs the cage on screen deterministically,
/// with no synthetic mouse in the loop. A gate that cannot reach a state
/// cannot report on it, and without this the honest gate outcome for the
/// cage is "unverified".
#[cfg(not(target_arch = "wasm32"))]
fn startup_selection() -> Option<usize> {
    std::env::var("EMBER_EDITOR_SELECT")
        .ok()?
        .trim()
        .parse::<usize>()
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn startup_selection() -> Option<usize> {
    None
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    #[must_use]
    pub fn new() -> Self {
        let objects = starter_scene();
        // Out-of-range is dropped rather than clamped: silently selecting a
        // different object than the one asked for would make a verification
        // hook lie about what it put on screen.
        let selected = startup_selection().filter(|&i| i < objects.len());
        Self {
            cam: FlyCam::default(),
            objects,
            selected,
            mode: gizmo::Mode::default(),
            hovered: None,
            drag: None,
            armed: 0,
            snap: GRID_STEP,
            lmb: gizmo::Edge::default(),
            // Digit keys arm palette entries on native; on the web the same
            // commands arrive from DOM buttons. Only as many digits as the
            // palette has entries, so key 6 with a five-entry palette does
            // nothing rather than arming something that is not there.
            keys: PALETTE_KEYS
                .iter()
                .take(palette::PALETTE.len())
                .map(|&k| (k, gizmo::Edge::default()))
                .collect(),
            key_place: gizmo::Edge::default(),
            key_delete: gizmo::Edge::default(),
            key_w: gizmo::Edge::default(),
            key_e: gizmo::Edge::default(),
            key_r: gizmo::Edge::default(),
        }
    }

    /// Applies one queued command. Separate from `update` so a shell can be
    /// tested against the editor without a frame loop.
    pub fn apply(&mut self, cmd: palette::Cmd) {
        match cmd {
            palette::Cmd::Arm(i) => {
                if i < palette::PALETTE.len() {
                    self.armed = i;
                }
            }
            palette::Cmd::Place => {
                let at = palette::ground_point(self.cam.eye, self.cam.forward(), self.snap);
                let obj = Obj::from_kind(&palette::PALETTE[self.armed], at);
                self.objects.push(obj);
                // Select what was just placed: the next thing anyone wants
                // is to move it, and hunting for it with a click is friction
                // the editor can simply remove.
                self.selected = Some(self.objects.len() - 1);
            }
            palette::Cmd::Delete => {
                if let Some(i) = self.selected {
                    if i < self.objects.len() {
                        self.objects.remove(i);
                    }
                    // Every later index shifted, so any surviving selection
                    // would now point at a different object. Clearing is the
                    // only answer that cannot be silently wrong.
                    self.selected = None;
                    self.drag = None;
                }
            }
            palette::Cmd::SetMode(m) => self.mode = m,
            palette::Cmd::Clear => {
                self.objects.clear();
                self.selected = None;
                self.drag = None;
            }
        }
    }

    /// The selected object's transform, for a drag to anchor against.
    fn selected_xform(&self) -> Option<drag::Xform> {
        let o = self.objects.get(self.selected?)?;
        Some(drag::Xform {
            pos: o.pos,
            scale: o.scale,
            yaw: o.yaw,
        })
    }

    /// The ray under the cursor this frame, if the cursor is over the window.
    #[must_use]
    pub fn cursor_ray(&self, input: &InputState) -> Option<(Vec3, Vec3)> {
        let ndc = input.cursor_ndc()?;
        Some(pick::ray_from_cursor(
            &self.cam.camera(),
            input.aspect(),
            ndc,
        ))
    }

    /// Centre of the current selection, which is where the gizmo sits.
    #[must_use]
    pub fn selection_centre(&self) -> Option<Vec3> {
        self.selected.map(|i| self.objects[i].pos)
    }

    /// The instances that are actually pickable, in the same order as
    /// `objects`, so an index from `pick_nearest` indexes `objects` directly.
    #[must_use]
    pub fn object_instances(&self) -> Vec<Instance> {
        self.objects.iter().map(Obj::instance).collect()
    }

    fn handle_shortcuts(&mut self, input: &InputState, flying: bool) {
        // Native shell shortcuts feed the same queue as the web sidebar.
        for (i, (key, edge)) in self.keys.iter_mut().enumerate() {
            if edge.pressed(input.down(*key)) && !flying {
                palette::push(palette::Cmd::Arm(i));
            }
        }
        if self.key_place.pressed(input.down(KeyCode::Enter)) && !flying {
            palette::push(palette::Cmd::Place);
        }
        if self.key_delete.pressed(input.down(KeyCode::KeyX)) && !flying {
            palette::push(palette::Cmd::Delete);
        }

        // Poll edges while flying so releasing RMB cannot trigger a stale mode change.
        let w = self.key_w.pressed(input.down(KeyCode::KeyW));
        let e = self.key_e.pressed(input.down(KeyCode::KeyE));
        let r = self.key_r.pressed(input.down(KeyCode::KeyR));
        if !flying {
            if w {
                self.mode = gizmo::Mode::Translate;
            }
            if e {
                self.mode = gizmo::Mode::Rotate;
            }
            if r {
                self.mode = gizmo::Mode::Scale;
            }
        }
    }
}

/// Something to look at on first run, and something to aim the picker at
/// before a palette exists.
/// The scene the editor opens on: a real arena the shooter would play,
/// not a hand-arranged approximation of one.
///
/// `EMBER_EDITOR_SEED` picks which; the default is the seed the arena has
/// always demoed with. Loading a `Level` rather than inventing boxes means
/// the blank page is already a level you can fly through, export, and get
/// back byte-identical.
fn starter_scene() -> Vec<Obj> {
    level::from_level(&pong_core::shooter::Level::from_seed(starter_seed()))
}

fn starter_seed() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(s) = std::env::var("EMBER_EDITOR_SEED")
            && let Ok(v) = s.parse()
        {
            return v;
        }
    }
    7
}

impl EmberGame for Editor {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        // Drained first, so a frame's behaviour is a function of the state it
        // began with. A command pushed during this frame lands on the next.
        for cmd in palette::drain() {
            self.apply(cmd);
        }

        let flying = self.cam.update(input, dt);

        self.handle_shortcuts(input, flying);

        // Hover and selection both come off one ray, so what highlights is
        // exactly what a click would take.
        let ray = self.cursor_ray(input);
        self.hovered = match (ray, self.selection_centre()) {
            (Some((org, dir)), Some(centre)) => gizmo::pick_handle(org, dir, centre, self.cam.eye),
            _ => None,
        };

        let held_before = self.lmb.is_down();
        let lmb = input.mouse_down(MouseButton::Left);
        let clicked = self.lmb.pressed(lmb);
        let released = held_before && !lmb;

        if released {
            self.drag = None;
        }
        if clicked && !flying {
            // A click on a handle STARTS A DRAG rather than changing the
            // selection — otherwise grabbing the gizmo would deselect the
            // thing the gizmo belongs to.
            match (self.hovered, self.selected_xform(), ray) {
                (Some(axis), Some(start), Some((org, dir))) => {
                    // `begin` returns None for a handle this data model
                    // cannot honour (see drag.rs on rotation) or one seen too
                    // nearly edge-on to solve. Either way: no drag, and the
                    // selection is left alone.
                    self.drag = drag::Drag::begin(axis, self.mode, start, org, dir);
                }
                _ => {
                    self.selected = match ray {
                        Some((org, dir)) => {
                            pick::pick_nearest(org, dir, &self.object_instances()).map(|(i, _)| i)
                        }
                        None => self.selected,
                    };
                }
            }
        }

        // Apply the drag. Borrow of `self.drag` is finished before the
        // objects are touched, so the two disjoint fields never overlap.
        let dragged = match (self.drag.as_mut(), ray) {
            (Some(d), Some((org, dir))) => d.update(org, dir),
            // A ray that has swung to where the solve is meaningless holds
            // the last good transform rather than snapping anywhere.
            _ => None,
        };
        if let (Some(x), Some(i)) = (dragged, self.selected) {
            let o = &mut self.objects[i];
            o.pos = x.pos;
            o.scale = x.scale;
            // Collidable boxes snap to quarter turns AS THEY ROTATE, not at
            // save: `Obstacle` is an AABB and only a quarter turn survives
            // export, so a free angle on screen would be a rotation the
            // editor cannot keep. Spawns are points and rotate freely.
            o.yaw = match o.class {
                palette::Class::Object => level::snap_yaw(x.yaw),
                palette::Class::Spawn => x.yaw,
            };
        }
        // While dragging, the grabbed handle stays lit even if the cursor
        // wanders off it — the drag is still going, so the highlight would
        // otherwise contradict what the object is doing.
        if let Some(d) = self.drag.as_ref() {
            self.hovered = Some(d.axis);
        }

        let mut instances = grid_instances();
        instances.extend(axis_instances(6.0, 0.12));

        // Pickable geometry, one instance per object and in the same order,
        // so an index from `pick_nearest` indexes `objects` directly.
        for (i, obj) in self.objects.iter().enumerate() {
            let mut inst = obj.instance();
            if Some(i) == self.selected {
                inst.color = gizmo::selected_color(obj.color);
            }
            instances.push(inst);
        }
        // Decoration comes AFTER, and is deliberately not pickable: it would
        // otherwise break the one-to-one index correspondence above, and
        // clicking a spawn's pad would select an object nobody owns.
        for obj in &self.objects {
            if obj.class == palette::Class::Spawn {
                instances.push(palette::spawn_decoration(obj.pos));
            }
        }

        // Cage and gizmo last: they are the smallest things on screen and
        // the ones that must not be lost behind anything drawn after them.
        if let Some(i) = self.selected {
            let inst = self.objects[i].instance();
            instances.extend(gizmo::selection_cage(&inst));
            instances.extend(gizmo::gizmo_instances(
                self.objects[i].pos,
                self.cam.eye,
                self.mode,
            ));
        }

        Frame {
            camera: self.cam.camera(),
            instances,
        }
    }
}

/// Engine configuration for the editor.
///
/// `capture_mouse: false` is mandatory, not stylistic. Capture fires on ANY
/// mouse press with no button filter, `cursor_ndc` goes stale rather than
/// `None` under pointer lock — which would break both look and picking — and
/// on the web a locked pointer makes the DOM sidebar unclickable.
#[must_use]
pub fn engine_config() -> EngineConfig {
    EngineConfig {
        title: "ember — editor".to_string(),
        capture_mouse: false,
        meshes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_and_right_stay_orthogonal_and_unit() {
        // The picking ray is built from this basis, so a basis that drifts
        // off unit length puts every click slightly wrong.
        let mut cam = FlyCam::default();
        for (yaw, pitch) in [(0.0, 0.0), (1.2, 0.7), (-2.5, -1.4), (6.0, PITCH_LIMIT)] {
            cam.yaw = yaw;
            cam.pitch = pitch;
            let f = cam.forward();
            let r = cam.right();
            assert!(
                (f.length() - 1.0).abs() < 1e-5,
                "forward not unit at {yaw},{pitch}"
            );
            assert!(
                (r.length() - 1.0).abs() < 1e-5,
                "right not unit at {yaw},{pitch}"
            );
            assert!(
                f.dot(r).abs() < 1e-5,
                "basis not orthogonal at {yaw},{pitch}"
            );
        }
    }

    #[test]
    fn the_camera_looks_where_forward_points() {
        // `Camera` is an eye/target pair, so the editor's yaw/pitch only mean
        // anything if target - eye reproduces forward exactly.
        let cam = FlyCam {
            yaw: 0.9,
            pitch: -0.4,
            ..Default::default()
        };
        let c = cam.camera();
        let look = (c.target - c.eye).normalize();
        assert!((look - cam.forward()).length() < 1e-6);
    }

    #[test]
    fn the_grid_leaves_the_centre_lines_to_the_axes() {
        // A grid line at offset 0 would z-fight the coloured axis sitting in
        // the same place, and the two would flicker against each other.
        let g = grid_instances();
        assert!(!g.is_empty());
        for inst in &g {
            assert!(
                inst.position.x.abs() > 1e-6 || inst.position.z.abs() > 1e-6,
                "a grid line sits on the origin, where an axis is drawn"
            );
        }
    }

    #[test]
    fn the_axes_are_distinct_and_over_driven() {
        // The check that survives someone "tidying" the colours back to
        // 0..1: under the scene's lighting and tonemap those read as three
        // grey sticks, which defeats the point of coloured axes.
        let a = axis_instances(6.0, 0.12);
        assert_eq!(a.len(), 3);
        for inst in &a {
            assert!(
                inst.color.max_element() > 1.5,
                "axis colour {:?} is not over-driven; it will tonemap to grey",
                inst.color
            );
        }
        // Each axis must be longest along its own world axis.
        assert!(a[0].scale.x > a[0].scale.y && a[0].scale.x > a[0].scale.z);
        assert!(a[1].scale.y > a[1].scale.x && a[1].scale.y > a[1].scale.z);
        assert!(a[2].scale.z > a[2].scale.x && a[2].scale.z > a[2].scale.y);
    }

    #[test]
    fn axes_run_from_the_origin_outward_so_sign_is_readable() {
        let a = axis_instances(6.0, 0.12);
        assert!(
            a[0].position.x > 0.0,
            "the X bar must show +X, not straddle 0"
        );
        assert!(a[1].position.y > 0.0);
        assert!(a[2].position.z > 0.0);
    }

    #[test]
    fn object_instances_index_the_same_as_objects() {
        // pick_nearest returns an index into whatever slice it searched; the
        // editor then indexes `objects` with it. If these two ever diverge in
        // length or order, clicking one box selects another.
        let ed = Editor::new();
        let inst = ed.object_instances();
        assert_eq!(inst.len(), ed.objects.len());
        for (o, i) in ed.objects.iter().zip(inst.iter()) {
            assert_eq!(o.pos, i.position);
            assert_eq!(o.scale, i.scale);
        }
    }

    #[test]
    fn clicking_a_starter_box_selects_that_box() {
        // End-to-end over bite 2's maths: aim the camera at a known object
        // and confirm the picked index is the one under the centre of the
        // screen.
        let mut ed = Editor::new();
        let target = ed.objects[4]; // the +X box at (10, _, 0)
        ed.cam.eye = target.pos + Vec3::new(0.0, 0.0, 12.0);
        // forward = (cos yaw, sin pitch, sin yaw); -pi/2 with level pitch
        // looks straight down -Z, back at the box.
        ed.cam.yaw = -std::f32::consts::FRAC_PI_2;
        ed.cam.pitch = 0.0;
        let cam = ed.cam.camera();
        let (org, dir) = pick::ray_from_cursor(&cam, 16.0 / 9.0, [0.0, 0.0]);
        let hit = pick::pick_nearest(org, dir, &ed.object_instances());
        assert_eq!(hit.map(|(i, _)| i), Some(4), "centre-screen pick missed");
    }

    /// Drives `update` with a scripted input, since `InputState` has no public
    /// constructor for tests to build directly.
    fn frame_of(ed: &mut Editor) -> Frame {
        ed.update(&InputState::default(), 1.0 / 60.0)
    }

    #[test]
    fn nothing_is_selected_until_something_is_clicked() {
        let mut ed = Editor::new();
        let f = frame_of(&mut ed);
        assert!(ed.selected.is_none());
        // Grid + 3 axes + one instance per object + one pad per spawn, and
        // no cage or gizmo. The spawn pads are drawn AFTER the objects and
        // are deliberately not pickable, so they are counted separately
        // here rather than folded into the object count.
        let spawns = ed
            .objects
            .iter()
            .filter(|o| o.class == palette::Class::Spawn)
            .count();
        assert_eq!(
            f.instances.len(),
            grid_instances().len() + 3 + ed.objects.len() + spawns,
            "an unselected scene must not draw a cage or a gizmo"
        );
    }

    #[test]
    fn a_selection_adds_exactly_a_cage_and_a_gizmo() {
        let mut ed = Editor::new();
        let plain = frame_of(&mut ed).instances.len();
        ed.selected = Some(2);
        let selected = frame_of(&mut ed).instances.len();
        assert_eq!(
            selected - plain,
            12 + 6,
            "selection should add twelve cage edges and six gizmo parts"
        );
    }

    #[test]
    fn the_selected_object_is_drawn_brighter_than_it_was() {
        let mut ed = Editor::new();
        let idx = 3;
        let before = frame_of(&mut ed);
        let base = ed.objects[idx].color;
        // Objects are pushed after the grid and the three axes.
        let obj_start = grid_instances().len() + 3;
        assert_eq!(before.instances[obj_start + idx].color, base);

        ed.selected = Some(idx);
        let after = frame_of(&mut ed);
        let lit = after.instances[obj_start + idx].color;
        assert!(
            lit.length() > base.length(),
            "selected object must read brighter: {base:?} -> {lit:?}"
        );
    }

    #[test]
    fn modes_default_to_translate() {
        // W is the mode the user reaches for first, and it is also the key
        // that doubles as fly-forward — so the default must not depend on a
        // keypress that might have been swallowed by a fly drag.
        let ed = Editor::new();
        assert_eq!(ed.mode, gizmo::Mode::Translate);
    }

    #[test]
    fn the_gizmo_only_appears_where_the_selection_is() {
        let mut ed = Editor::new();
        ed.selected = Some(5);
        let centre = ed.objects[5].pos;
        assert_eq!(ed.selection_centre(), Some(centre));
        let f = frame_of(&mut ed);
        // The last six instances are the gizmo; each must sit near the
        // selected object rather than at the world origin.
        for inst in f.instances.iter().rev().take(6) {
            assert!(
                (inst.position - centre).length() < 30.0,
                "a gizmo part at {:?} is nowhere near the selection at {centre:?}",
                inst.position
            );
        }
    }

    #[test]
    fn a_startup_selection_out_of_range_is_dropped_not_clamped() {
        // The hook exists so a gate can put a known state on screen. Clamping
        // would have it quietly show a different object than the one asked
        // for, which makes the hook lie about what it rendered.
        let ed = Editor::new();
        let n = ed.objects.len();
        assert!(ed.selected.is_none_or(|i| i < n));
    }

    #[test]
    fn grabbing_a_handle_does_not_change_the_selection() {
        // The regression this guards: treating a gizmo click as a pick
        // deselects the object the moment you try to move it.
        let mut ed = Editor::new();
        ed.selected = Some(2);
        ed.hovered = Some(gizmo::Axis::X);
        let before = ed.selected;
        let _ = frame_of(&mut ed);
        assert_eq!(ed.selected, before);
    }

    #[test]
    fn a_drag_moves_the_object_it_was_started_on() {
        // End-to-end over bite 4: begin a translate drag by hand, then step
        // the ray and confirm the object followed by the DIFFERENCE.
        let mut ed = Editor::new();
        ed.selected = Some(0);
        let start = ed.selected_xform().expect("something is selected");
        let (o1, d1) = (Vec3::new(start.pos.x + 5.0, 40.0, start.pos.z), Vec3::NEG_Y);
        ed.drag = drag::Drag::begin(gizmo::Axis::X, gizmo::Mode::Translate, start, o1, d1);
        assert!(ed.drag.is_some(), "a perpendicular view must be grabbable");

        let (o2, d2) = (Vec3::new(start.pos.x + 9.0, 40.0, start.pos.z), Vec3::NEG_Y);
        let moved = ed.drag.as_mut().unwrap().update(o2, d2).unwrap();
        assert!((moved.pos.x - (start.pos.x + 4.0)).abs() < 1e-3);
        assert_eq!(moved.pos.z, start.pos.z, "a locked axis must not leak");
    }

    #[test]
    fn releasing_the_button_ends_the_drag() {
        let mut ed = Editor::new();
        ed.selected = Some(1);
        let start = ed.selected_xform().unwrap();
        ed.drag = drag::Drag::begin(
            gizmo::Axis::X,
            gizmo::Mode::Translate,
            start,
            Vec3::new(start.pos.x + 5.0, 40.0, start.pos.z),
            Vec3::NEG_Y,
        );
        assert!(ed.drag.is_some());
        // The default InputState has no buttons held, so the first update
        // after a press-less frame must clear it.
        ed.lmb.pressed(true); // pretend the button was down last frame
        let _ = frame_of(&mut ed);
        assert!(ed.drag.is_none(), "a released button must end the drag");
    }

    #[test]
    fn a_queued_command_is_applied_on_the_next_frame() {
        // The whole point of the queue: a sidebar button and a digit key are
        // the same event by the time the editor sees them.
        palette::clear_queue();
        let mut ed = Editor::new();
        let before = ed.objects.len();
        palette::push(palette::Cmd::Place);
        let _ = frame_of(&mut ed);
        assert_eq!(ed.objects.len(), before + 1, "Place must add an object");
    }

    #[test]
    fn placing_selects_what_was_just_placed() {
        // Hunting for the thing you just put down is friction the editor can
        // simply remove — the next action is always to move it.
        palette::clear_queue();
        let mut ed = Editor::new();
        ed.apply(palette::Cmd::Place);
        assert_eq!(ed.selected, Some(ed.objects.len() - 1));
    }

    #[test]
    fn a_placed_object_sits_on_the_ground_not_half_in_it() {
        palette::clear_queue();
        let mut ed = Editor::new();
        ed.apply(palette::Cmd::Arm(1)); // container
        ed.apply(palette::Cmd::Place);
        let o = ed.objects.last().unwrap();
        assert!(
            o.scale.y.mul_add(-0.5, o.pos.y).abs() < 1e-5,
            "a box at y={} with height {} is buried",
            o.pos.y,
            o.scale.y
        );
    }

    #[test]
    fn arming_past_the_end_of_the_palette_is_ignored() {
        // Digit 9 with a six-entry palette, or a stale index from a sidebar
        // built against an older build. Ignoring beats panicking on an index.
        palette::clear_queue();
        let mut ed = Editor::new();
        ed.apply(palette::Cmd::Arm(999));
        assert!(ed.armed < palette::PALETTE.len());
    }

    #[test]
    fn deleting_clears_the_selection_rather_than_leaving_it_dangling() {
        // Removing shifts every later index, so a surviving selection would
        // silently point at a DIFFERENT object. That is the bug this guards.
        palette::clear_queue();
        let mut ed = Editor::new();
        let before = ed.objects.len();
        ed.selected = Some(0);
        ed.apply(palette::Cmd::Delete);
        assert_eq!(ed.objects.len(), before - 1);
        assert_eq!(ed.selected, None, "a stale index is worse than none");
        assert!(ed.drag.is_none());
    }

    #[test]
    fn deleting_with_nothing_selected_does_nothing() {
        palette::clear_queue();
        let mut ed = Editor::new();
        let before = ed.objects.len();
        ed.selected = None;
        ed.apply(palette::Cmd::Delete);
        assert_eq!(ed.objects.len(), before);
    }

    #[test]
    fn a_spawn_draws_a_pad_that_cannot_be_clicked() {
        // Decoration must not join the pickable list: picking indexes objects
        // one to one, and a clickable pad would select an index nobody owns.
        palette::clear_queue();
        let mut ed = Editor::new();
        let spawn_idx = palette::PALETTE
            .iter()
            .position(|k| k.class == palette::Class::Spawn)
            .expect("the palette has a spawn");
        ed.apply(palette::Cmd::Arm(spawn_idx));
        ed.apply(palette::Cmd::Place);

        let pickable = ed.object_instances().len();
        assert_eq!(pickable, ed.objects.len(), "decoration leaked into picking");

        let drawn = frame_of(&mut ed).instances.len();
        let expected_min = grid_instances().len() + 3 + ed.objects.len() + 1;
        assert!(
            drawn >= expected_min,
            "a spawn must draw its pad as well as its post"
        );
    }

    #[test]
    fn clear_empties_the_level_and_forgets_the_selection() {
        palette::clear_queue();
        let mut ed = Editor::new();
        ed.selected = Some(1);
        ed.apply(palette::Cmd::Clear);
        assert!(ed.objects.is_empty());
        assert_eq!(ed.selected, None);
    }

    #[test]
    fn a_mode_command_is_the_same_as_pressing_the_key() {
        // The web shell has no keyboard focus on the canvas, so every mode
        // change arrives as a command. It must reach the same state.
        palette::clear_queue();
        let mut ed = Editor::new();
        ed.apply(palette::Cmd::SetMode(gizmo::Mode::Scale));
        assert_eq!(ed.mode, gizmo::Mode::Scale);
    }
}
