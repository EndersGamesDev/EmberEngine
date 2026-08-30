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

pub mod pick;

use ember_engine::glam::{Quat, Vec3};
use ember_engine::{Camera, EmberGame, EngineConfig, Frame, InputState, Instance, KeyCode, MouseButton};

/// Half-extent of the authored area, and of the reference grid.
pub const GRID_HALF: f32 = 24.0;
/// Grid spacing. 2.0 keeps a 24-unit half-extent at 25 lines per axis, which
/// is legible without turning the floor into a solid sheet.
const GRID_STEP: f32 = 2.0;
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

/// Instance colours are an unclamped multiplier into a shader that lights,
/// ACES-tonemaps and fogs every fragment, so a gizmo coloured (1,0,0) comes
/// out dark red on unlit faces and washes toward the fog colour with
/// distance. Over-driving past 1.0 pushes every face to saturation whatever
/// its normal, which is the whole point of "different colours to know where
/// you place things".
///
/// This leans on the clamp inside `aces()`. If the tonemap ever moves, these
/// stop reading as R/G/B with no compile error — which is why bite 1's
/// acceptance check is that the axes read as distinctly red, green and blue,
/// not merely that they appear.
pub const AXIS_X: Vec3 = Vec3::new(4.0, 0.06, 0.06);
pub const AXIS_Y: Vec3 = Vec3::new(0.10, 4.0, 0.10);
pub const AXIS_Z: Vec3 = Vec3::new(0.10, 0.35, 4.0);
const GRID_COLOR: Vec3 = Vec3::new(0.20, 0.24, 0.30);
const GRID_MAJOR: Vec3 = Vec3::new(0.34, 0.40, 0.50);

/// A flying camera. Yaw/pitch match the arena client's convention exactly, so
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
    pub fn forward(&self) -> Vec3 {
        let (sz, cx) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cx * cp, sp, sz * cp)
    }

    /// Horizontal right, for strafing. Deliberately flat: a fly camera that
    /// rolls its strafe with pitch is disorienting when placing objects.
    pub fn right(&self) -> Vec3 {
        let (sz, cx) = self.yaw.sin_cos();
        Vec3::new(-sz, 0.0, cx)
    }

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
                self.yaw += dx * LOOK_SENS;
                // NDC y is up-positive and so is pitch, so this needs no flip.
                self.pitch = (self.pitch + dy * LOOK_SENS).clamp(-PITCH_LIMIT, PITCH_LIMIT);
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
        self.eye = self.eye.clamp(Vec3::splat(-EYE_LIMIT), Vec3::splat(EYE_LIMIT));
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
pub fn grid_instances() -> Vec<Instance> {
    let mut out = Vec::new();
    let steps = (GRID_HALF / GRID_STEP) as i32;
    for i in -steps..=steps {
        let offset = i as f32 * GRID_STEP;
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
}

impl Obj {
    pub fn instance(&self) -> Instance {
        Instance::new(self.pos, self.scale, self.color).with_rot(Quat::from_rotation_y(self.yaw))
    }
}

pub struct Editor {
    pub cam: FlyCam,
    pub objects: Vec<Obj>,
    /// Index into `objects`, not into the frame's instance list — the frame
    /// also carries the grid and the axes, and their indices shift.
    pub selected: Option<usize>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            cam: FlyCam::default(),
            objects: starter_scene(),
            selected: None,
        }
    }

    /// The instances that are actually pickable, in the same order as
    /// `objects`, so an index from `pick_nearest` indexes `objects` directly.
    pub fn object_instances(&self) -> Vec<Instance> {
        self.objects.iter().map(|o| o.instance()).collect()
    }
}

/// Something to look at on first run, and something to aim the picker at
/// before a palette exists.
fn starter_scene() -> Vec<Obj> {
    let mut out = Vec::new();
    let grey = Vec3::new(0.42, 0.45, 0.50);
    let warm = Vec3::new(0.55, 0.42, 0.30);
    for (i, (x, z)) in [
        (-8.0, -8.0),
        (0.0, -10.0),
        (8.0, -8.0),
        (-10.0, 0.0),
        (10.0, 0.0),
        (-8.0, 8.0),
        (0.0, 10.0),
        (8.0, 8.0),
    ]
    .into_iter()
    .enumerate()
    {
        // Alternating crate and container heights, matching the two cover
        // classes the shooter already has, so the editor's default scene
        // looks like the game it authors for.
        let h = if i % 3 == 0 { 2.6 } else { 1.2 };
        out.push(Obj {
            pos: Vec3::new(x, h * 0.5, z),
            scale: Vec3::new(2.2, h, 2.2),
            yaw: 0.0,
            color: if i % 3 == 0 { warm } else { grey },
        });
    }
    out
}

impl EmberGame for Editor {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let _flying = self.cam.update(input, dt);

        let mut instances = grid_instances();
        instances.extend(axis_instances(6.0, 0.12));
        instances.extend(self.object_instances());

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
            assert!((f.length() - 1.0).abs() < 1e-5, "forward not unit at {yaw},{pitch}");
            assert!((r.length() - 1.0).abs() < 1e-5, "right not unit at {yaw},{pitch}");
            assert!(f.dot(r).abs() < 1e-5, "basis not orthogonal at {yaw},{pitch}");
        }
    }

    #[test]
    fn the_camera_looks_where_forward_points() {
        // `Camera` is an eye/target pair, so the editor's yaw/pitch only mean
        // anything if target - eye reproduces forward exactly.
        let mut cam = FlyCam::default();
        cam.yaw = 0.9;
        cam.pitch = -0.4;
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
        assert!(a[0].position.x > 0.0, "the X bar must show +X, not straddle 0");
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
}
