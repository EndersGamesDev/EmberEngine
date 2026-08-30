//! The transform gizmo, the selection cage, and the edge detector that makes
//! W/E/R behave like mode switches instead of like held keys.
//!
//! Everything here is instanced boxes, because the renderer has triangles and
//! nothing else — no line primitive, no blending, no second pass. That single
//! fact decides most of the design below:
//!
//! * A selected object cannot be outlined by an inverted hull, because
//!   `cull_mode` is `None`: a scaled-up shell renders its front faces and
//!   simply covers the object it was meant to outline.
//! * It cannot be tinted by a translucent overlay either, because the blend
//!   state is `REPLACE`.
//! * The gizmo shares the world's depth buffer with `depth_compare: Less`, so
//!   it IS occluded by geometry and cannot be ghosted on top.
//!
//! So selection reads as a colour boost plus a wireframe cage built from the
//! box's twelve edges, and the gizmo is nudged toward the camera by an
//! epsilon so it does not z-fight the object it is attached to.

use ember_engine::glam::{Quat, Vec3};
use ember_engine::Instance;

use crate::pick;
use crate::{AXIS_X, AXIS_Y, AXIS_Z};

/// What a drag on a handle does. The user asked for W/E/R; these are the
/// modes those keys select, and they only apply while the fly drag is NOT
/// held — see the modality note in the crate docs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn dir(self) -> Vec3 {
        match self {
            Axis::X => Vec3::X,
            Axis::Y => Vec3::Y,
            Axis::Z => Vec3::Z,
        }
    }

    pub fn color(self) -> Vec3 {
        match self {
            Axis::X => AXIS_X,
            Axis::Y => AXIS_Y,
            Axis::Z => AXIS_Z,
        }
    }

    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];
}

/// Thickness of a gizmo shaft, as a fraction of its length.
const SHAFT_THICK: f32 = 0.055;
/// Head size, as a fraction of shaft length. Scale mode uses a cube and
/// translate a longer block, so the mode is readable without any text.
const HEAD_FRAC: f32 = 0.22;
/// How much fatter the invisible pick volume is than the drawn handle. A
/// handle thin enough to look right is far too thin to hit with a mouse.
const PICK_FATTEN: f32 = 3.5;
/// Fraction of the distance to the eye that one handle spans. Keeping the
/// gizmo a constant size ON SCREEN is what makes it usable at any zoom; a
/// world-sized gizmo vanishes when you fly out and swallows the level when
/// you fly in.
const HANDLE_SCREEN_FRAC: f32 = 0.13;
/// Shortest and longest a handle may get, so it stays sane at the extremes.
const HANDLE_MIN: f32 = 0.6;
const HANDLE_MAX: f32 = 12.0;

/// Handle length for a gizmo at `centre` seen from `eye`.
pub fn handle_len(centre: Vec3, eye: Vec3) -> f32 {
    ((centre - eye).length() * HANDLE_SCREEN_FRAC).clamp(HANDLE_MIN, HANDLE_MAX)
}

/// The six drawn gizmo instances: three shafts, then three heads, each triple
/// ordered X, Y, Z so `[0..3]` is red, green, blue.
///
/// `eye` is used only to nudge the gizmo a hair toward the camera. Without
/// that, a gizmo centred inside its own object z-fights it across the whole
/// shaft, which reads as flicker rather than as a handle.
pub fn gizmo_instances(centre: Vec3, eye: Vec3, mode: Mode) -> Vec<Instance> {
    let len = handle_len(centre, eye);
    let thick = len * SHAFT_THICK;
    let head = len * HEAD_FRAC;
    // Toward the eye by a hair. Proportional to handle length so it holds up
    // at every distance rather than being tuned for one.
    let nudge = (eye - centre).normalize_or_zero() * (thick * 1.5);
    let base = centre + nudge;

    let mut out = Vec::with_capacity(6);
    for a in Axis::ALL {
        let d = a.dir();
        // Shafts run from the centre outward, so the gizmo shows the POSITIVE
        // direction of each axis — which is the thing you need to know when
        // deciding which way a drag will move something.
        out.push(
            Instance::new(
                base + d * (len * 0.5),
                Vec3::ONE * thick + d * (len - thick),
                a.color(),
            )
            .with_rot(Quat::IDENTITY),
        );
    }
    for a in Axis::ALL {
        let d = a.dir();
        let head_scale = match mode {
            // A long block reads as an arrow; a cube reads as a grab handle.
            Mode::Translate => Vec3::ONE * (head * 0.55) + d * (head - head * 0.55),
            Mode::Scale => Vec3::ONE * (head * 0.8),
            // Flattened ACROSS its axis: a disc-ish tab suggesting rotation
            // about that axis, which is the closest a box gets to a ring.
            Mode::Rotate => Vec3::ONE * head - d * (head * 0.72),
        };
        out.push(Instance::new(base + d * len, head_scale, a.color()));
    }
    out
}

/// Which handle the ray hits, if any. Nearest wins, so overlapping handles
/// near the origin of the gizmo resolve to the one actually in front.
///
/// The pick volume is a fattened box along the whole shaft rather than the
/// drawn geometry: handles are drawn thin because thin looks right, and a
/// pick test against what is drawn would demand pixel-accurate aiming.
pub fn pick_handle(origin: Vec3, dir: Vec3, centre: Vec3, eye: Vec3) -> Option<Axis> {
    let len = handle_len(centre, eye);
    let thick = len * SHAFT_THICK * PICK_FATTEN;
    let nudge = (eye - centre).normalize_or_zero() * (len * SHAFT_THICK * 1.5);
    let base = centre + nudge;

    let mut best: Option<(Axis, f32)> = None;
    for a in Axis::ALL {
        let d = a.dir();
        // Covers shaft and head together: the head sits at the far end, so
        // extending the box a little past `len` catches it too.
        let half = (Vec3::ONE * thick + d * (len * 1.25 - thick)) * 0.5;
        let centre_of = base + d * (len * 0.625);
        let Some(t) = pick::ray_box(origin, dir, centre_of, Quat::IDENTITY, half) else {
            continue;
        };
        if best.is_none_or(|(_, bt)| t < bt) {
            best = Some((a, t));
        }
    }
    best.map(|(a, _)| a)
}

/// Colour a selected object is drawn with. A boost rather than a fixed
/// highlight colour, so a selected object still reads as itself — which
/// matters when several are similar and only one is selected.
pub fn selected_color(base: Vec3) -> Vec3 {
    base * 1.6 + Vec3::splat(0.25)
}

/// The twelve edges of an instance's box, as thin boxes: a wireframe cage.
///
/// This is the only outline this renderer can express. It is drawn slightly
/// larger than the object so the cage is not co-planar with the faces it
/// surrounds, which would z-fight along every edge.
pub fn selection_cage(inst: &Instance) -> Vec<Instance> {
    let half = inst.scale.abs() * 0.5;
    // Thickness scaled off the object, so a cage on a tiny prop is not a
    // slab and a cage on a container is not invisible.
    let thick = (half.min_element() * 0.09).clamp(0.015, 0.09);
    let half = half + Vec3::splat(thick * 0.5);
    let color = Vec3::new(3.4, 3.0, 0.6); // over-driven amber, same reason as the axes

    let mut out = Vec::with_capacity(12);
    // Four edges parallel to each axis, at the four sign combinations of the
    // other two.
    for (axis, (u, v)) in [
        (Vec3::X, (Vec3::Y, Vec3::Z)),
        (Vec3::Y, (Vec3::X, Vec3::Z)),
        (Vec3::Z, (Vec3::X, Vec3::Y)),
    ] {
        let span = axis * (half.dot(axis) * 2.0) + (Vec3::ONE - axis) * thick;
        for su in [-1.0f32, 1.0] {
            for sv in [-1.0f32, 1.0] {
                let offset = u * (half.dot(u) * su) + v * (half.dot(v) * sv);
                out.push(
                    Instance::new(inst.position + inst.rot * offset, span, color)
                        .with_rot(inst.rot),
                );
            }
        }
    }
    out
}

/// Rising-edge detector for the handful of inputs the editor treats as
/// presses rather than as holds.
///
/// The engine offers only "is it down now" — there is no `just_pressed` — and
/// adding one to `InputState` would mean settling its semantics against the
/// two-consumer latch `docs/input-latch.md` owns. The editor is the only
/// consumer, so it tracks its own.
///
/// One subtlety worth the struct existing at all: the platform layer clears
/// every held key on `Focused(false)` without telling the game. A naive
/// detector would then see the next real poll as a fresh press. Because this
/// only ever reports a LOW to HIGH transition, a clear looks like a release,
/// which is exactly right.
#[derive(Default)]
pub struct Edge {
    prev: bool,
}

impl Edge {
    /// True on the frame `now` first becomes true.
    pub fn pressed(&mut self, now: bool) -> bool {
        let fired = now && !self.prev;
        self.prev = now;
        fired
    }

    /// True on the frame `now` first becomes false.
    pub fn released(&mut self, now: bool) -> bool {
        let fired = !now && self.prev;
        self.prev = now;
        fired
    }

    pub fn is_down(&self) -> bool {
        self.prev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_fires_once_per_press() {
        let mut e = Edge::default();
        // down, held, held, up, up, down again
        let seq = [true, true, true, false, false, true];
        let fired: Vec<bool> = seq.iter().map(|&d| e.pressed(d)).collect();
        assert_eq!(fired, vec![true, false, false, false, false, true]);
    }

    #[test]
    fn a_focus_loss_clear_does_not_look_like_a_press() {
        // The platform layer clears held keys on Focused(false) and the game
        // is not told. The next poll must not read as a new press.
        let mut e = Edge::default();
        assert!(e.pressed(true), "initial press");
        assert!(!e.pressed(true), "still held");
        // Focus lost: the engine clears, so the game sees `false`.
        assert!(!e.pressed(false), "a clear is a release, not a press");
        // Focus regained with the key genuinely still held down.
        assert!(e.pressed(true), "a real re-press after the clear");
    }

    #[test]
    fn released_is_the_mirror_of_pressed() {
        let mut e = Edge::default();
        assert!(!e.released(true));
        assert!(e.released(false));
        assert!(!e.released(false));
    }

    #[test]
    fn the_gizmo_is_six_instances_ordered_rgb() {
        let g = gizmo_instances(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0), Mode::Translate);
        assert_eq!(g.len(), 6, "three shafts and three heads");
        for triple in [0, 3] {
            assert_eq!(g[triple].color, AXIS_X);
            assert_eq!(g[triple + 1].color, AXIS_Y);
            assert_eq!(g[triple + 2].color, AXIS_Z);
        }
    }

    #[test]
    fn each_shaft_is_longest_along_its_own_axis() {
        let g = gizmo_instances(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0), Mode::Translate);
        assert!(g[0].scale.x > g[0].scale.y && g[0].scale.x > g[0].scale.z);
        assert!(g[1].scale.y > g[1].scale.x && g[1].scale.y > g[1].scale.z);
        assert!(g[2].scale.z > g[2].scale.x && g[2].scale.z > g[2].scale.y);
    }

    #[test]
    fn the_gizmo_holds_its_screen_size() {
        // The property that makes it usable at any zoom: twice as far away,
        // twice as big in world units, so it covers the same screen area.
        let near = handle_len(Vec3::ZERO, Vec3::new(0.0, 0.0, 20.0));
        let far = handle_len(Vec3::ZERO, Vec3::new(0.0, 0.0, 40.0));
        assert!(
            (far / near - 2.0).abs() < 1e-4,
            "handle length must track distance: {near} then {far}"
        );
        // ...but not without bound, at either end.
        assert_eq!(handle_len(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.1)), HANDLE_MIN);
        assert_eq!(handle_len(Vec3::ZERO, Vec3::splat(9000.0)), HANDLE_MAX);
    }

    #[test]
    fn a_ray_down_an_axis_picks_that_axis() {
        let centre = Vec3::ZERO;
        let eye = Vec3::new(0.0, 40.0, 0.0); // above, looking down
        let len = handle_len(centre, eye);
        // Aim at a point partway along each shaft, from a long way off along
        // a direction that only meets that one handle.
        let cases = [
            (Axis::X, Vec3::X, Vec3::Z),
            (Axis::Z, Vec3::Z, Vec3::X),
        ];
        for (want, along, from) in cases {
            let target = centre + along * (len * 0.6);
            let org = target + from * 50.0;
            let dir = (target - org).normalize();
            assert_eq!(
                pick_handle(org, dir, centre, eye),
                Some(want),
                "a ray straight at the {want:?} shaft picked something else"
            );
        }
    }

    #[test]
    fn a_ray_into_empty_space_picks_no_handle() {
        let centre = Vec3::ZERO;
        let eye = Vec3::new(0.0, 0.0, 20.0);
        let org = Vec3::new(0.0, 0.0, 20.0);
        // Straight up, missing every shaft.
        assert!(pick_handle(org, Vec3::Y, centre, eye).is_none());
    }

    #[test]
    fn the_cage_has_twelve_edges_and_surrounds_the_box() {
        let inst = Instance::new(Vec3::new(3.0, 1.0, -2.0), Vec3::new(2.0, 4.0, 6.0), Vec3::ONE);
        let cage = selection_cage(&inst);
        assert_eq!(cage.len(), 12, "a box has twelve edges");
        for e in &cage {
            // Every edge sits on the surface, not inside: its offset from the
            // centre must reach at least one half-extent.
            let d = (e.position - inst.position).abs();
            let half = inst.scale.abs() * 0.5;
            assert!(
                d.x >= half.x - 1e-3 || d.y >= half.y - 1e-3 || d.z >= half.z - 1e-3,
                "cage edge at {:?} is inside the box",
                e.position
            );
        }
    }

    #[test]
    fn the_cage_follows_a_rotated_box() {
        // The cage is drawn in the object's frame, so a rotated object must
        // get a rotated cage — otherwise it reads as a bug in the rotation.
        let rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let inst = Instance::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 8.0), Vec3::ONE).with_rot(rot);
        let cage = selection_cage(&inst);
        assert!(cage.iter().all(|e| e.rot == rot));
        // A corner of a 2x2x8 box rotated 45 degrees about Y reaches further
        // in x than the unrotated half-extent of 1.0.
        assert!(cage.iter().any(|e| e.position.x.abs() > 1.5));
    }

    #[test]
    fn selection_brightens_without_discarding_the_objects_colour() {
        // Two differently-coloured objects must still look different when
        // selected; a fixed highlight colour would make them identical.
        let a = selected_color(Vec3::new(0.4, 0.2, 0.1));
        let b = selected_color(Vec3::new(0.1, 0.2, 0.4));
        assert!(a.x > 0.4 && b.z > 0.4, "selection must brighten");
        assert!(a != b, "selection must not flatten two colours into one");
    }
}
