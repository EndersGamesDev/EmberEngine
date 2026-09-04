//! What a round looks like in flight (arena v20, the second pass): the
//! projectile of every bullet weapon as a body of revolution at its real
//! calibre, wearing a copper-jacket strip, and one tapered streak that
//! trails it. The rocket (weapon 7) keeps its GLB and is not here.
//!
//! The first cut of v20 drew a round as box rods, so every shot was a
//! square-section stick. These meshes replace what the frame draws for a
//! `feel::Tracer`; the tracer's kinematics (where the head is, how long the
//! core and the tail are, the linger and the fade) are untouched.
//!
//! Every mesh points along +X (nose at +X), is centred on its length so an
//! instance's position is the round's centre, and is in metres at real
//! size: a 9x19 is 0.0155 long. The frame scales it by `ROUND_SCALE`.
//! Textured parts are pushed with `Vec3::ONE`, never a tint (the picture
//! carries the colour; a tint on top double-tints).
//!
//! The streak is two meshes so that it is one taper: a frustum (`CORE`)
//! for the bright core right behind the round and a cone (`STREAK`) for
//! the dim tail behind that, the frustum's narrow end the cone's base. A
//! rod with nothing behind it (the core alone, early in a flight or on a
//! short segment) is the cone, so a streak always ends in a point.
//!
//! The third pass adds the hole a round leaves (`DISC`): a thin closed
//! disc the frame lays on the face a round hit in place of the square
//! plate `feel::Mark` was, sized by the calibre of the round that made
//! it. The flash star is not a mesh of its own: it is the streak cone five
//! times over, radiating from the muzzle (`online::push_flash`).

use std::f32::consts::TAU;

use ember_engine::{MeshData, MeshVertex, TextureData};

use crate::feel::{TRACER_CORE_LEN, TRACER_TAIL_LEN};

/// One calibre with a mesh of its own. The discriminant is the mesh's
/// offset from the base `run_online` hands to `ShooterGame::set_rounds`,
/// so the order here IS the registration order and `round_meshes` builds
/// from `ALL` to keep them from drifting apart. The streak comes after the
/// last round, at `STREAK_OFFSET`, the core after it, the disc last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Round {
    /// 9x19 mm full metal jacket, round nose: the sidearm and the Vityaz.
    Nine,
    /// 7.62x39 mm full metal jacket, boat tail, cannelure: the AK-47.
    Ak,
    /// 5.56x45 mm, boat tail, spitzer: the M4.
    M4,
    /// .454 Casull jacketed soft point, flat nose, cannelure: the revolver.
    Casull,
    /// .338 Lapua Magnum, long boat tail, long secant ogive: the sniper.
    Lapua,
}

impl Round {
    /// Every round, in registration order.
    pub const ALL: [Self; 5] = [Self::Nine, Self::Ak, Self::M4, Self::Casull, Self::Lapua];

    /// Offset from the registered base.
    #[must_use]
    pub const fn offset(self) -> u32 {
        self as u32
    }

    /// The real (diameter, length) of the projectile, millimetres.
    #[must_use]
    pub const fn calibre_mm(self) -> (f32, f32) {
        match self {
            Self::Nine => (9.0, 15.5),
            Self::Ak => (7.9, 26.5),
            Self::M4 => (5.7, 23.0),
            Self::Casull => (11.5, 19.0),
            Self::Lapua => (8.6, 41.0),
        }
    }

    /// Real length, metres.
    #[must_use]
    pub const fn length(self) -> f32 {
        self.calibre_mm().1 * MM
    }

    /// The radius of the heel, the rim of the base, millimetres: on a boat
    /// tail well under the body's. The profile reads it from here, so the
    /// streak that meets the round at its tail sizes itself from the same
    /// number (`STREAK_LEAD`) and never stands proud of the heel.
    #[must_use]
    pub const fn heel_mm(self) -> f32 {
        match self {
            Self::Nine => 4.2,
            Self::Ak => 3.3,
            Self::M4 => 2.3,
            Self::Casull => 5.4,
            Self::Lapua => 3.4,
        }
    }

    /// The heel's radius, metres.
    #[must_use]
    pub const fn heel_radius(self) -> f32 {
        self.heel_mm() * MM
    }
}

/// Millimetres to metres.
const MM: f32 = 0.001;

/// The streak mesh's offset from the registered base: after the last round.
pub const STREAK_OFFSET: u32 = Round::Lapua.offset() + 1;

/// The core frustum's offset: after the streak.
pub const CORE_OFFSET: u32 = STREAK_OFFSET + 1;

/// The hole disc's offset: after the core.
pub const DISC_OFFSET: u32 = CORE_OFFSET + 1;

/// How many meshes `round_meshes` builds: the rounds, the streak, the
/// core, the disc.
pub const MESH_COUNT: usize = DISC_OFFSET as usize + 1;

/// A round is drawn at this many times its real size. At real size a 9 mm
/// is under a pixel past about 3 m, so it would be a fleck at the muzzle
/// and nothing after; at five times it reads as a bullet at the muzzle and
/// inside about 10 m, and as a bright point beyond, which is what the eye
/// expects of a round it can see at all. The streak scales with it, so the
/// two never come apart.
pub const ROUND_SCALE: f32 = 5.0;

/// The streak's radius where it meets the round, as a fraction of the
/// round's drawn HEEL radius (`Round::heel_radius`, the rim of the base,
/// which is where the streak meets it): under one, so the bullet's own
/// silhouette leads the streak instead of vanishing inside it, and on a
/// boat tail the streak sits inside the heel rather than as a lip round it.
pub const STREAK_LEAD: f32 = 0.9;
const _: () = assert!(STREAK_LEAD > 0.0 && STREAK_LEAD < 1.0);

/// How far the streak's base sits inside the round, as a fraction of the
/// round's half-length. The round's own base disc and the streak's base
/// disc are both opaque; in one plane they would z-fight (copper against
/// the tracer colour, seen from behind an own round), so the streak starts
/// this far up the body, where every profile is already wider than the
/// heel and hides the disc.
pub const STREAK_INSET: f32 = 0.1;

/// The core frustum's narrow end as a fraction of its base, which is also
/// the tail cone's base as a fraction of the core's: the ratio at which
/// the two make one straight taper from the round to the tail's end when
/// both are at full length (`feel::TRACER_CORE_LEN` of core, the rest of
/// `feel::TRACER_TAIL_LEN` of tail). Without it the core came to a point
/// 2.5 m behind the round and the tail began there at full width: two
/// arrowheads in a row, the neck plain in the first capture.
pub const CORE_NECK: f32 = (TRACER_TAIL_LEN - TRACER_CORE_LEN) / TRACER_TAIL_LEN;
const _: () = assert!(CORE_NECK > 0.5 && CORE_NECK < 1.0);

/// Sides of a round's revolution: a 12-gon reads as round at the sizes a
/// round is ever drawn at, and keeps every mesh at a few hundred vertices.
pub const SIDES: u32 = 12;

/// Sides of the streak's cone and the core's frustum.
pub const STREAK_SIDES: u32 = 8;

/// The copper jacket, as the 8-bit sRGB bytes the picture is stored in
/// (the upload is `Rgba8UnormSrgb`, so these decode to about 0.61, 0.22,
/// 0.06 linear). Reasoned, not seen: a saturated copper a step brighter
/// than the classic `#B87333`, so the sRGB decode does not sink to black
/// at 5x on a small dark shape, and redder than the spec's pale hint so
/// the warm fog does not turn it pink. Nobody has judged it on screen yet;
/// the capture step does, and this is the one place to change it.
const COPPER: [f32; 3] = [205.0, 130.0, 70.0];

/// The strip's size: `u` runs around the round (8 texels, the jacket is
/// the same all round), `v` along it from the base (64 texels, one per
/// band boundary a profile can have).
const TEX_W: u32 = 8;
const TEX_H: u32 = 64;

/// The round that a weapon id fires, or `None` for the rocket, which is a
/// mesh of its own. An id off the table fires the sidearm's round, as
/// `weapon_stats` and `weapon_feel` read such an id as the sidearm.
#[must_use]
pub const fn round_for(weapon: u8) -> Option<Round> {
    match weapon {
        3 => Some(Round::Ak),
        4 => Some(Round::M4),
        5 => Some(Round::Casull),
        6 => Some(Round::Lapua),
        7 => None,
        _ => Some(Round::Nine),
    }
}

/// Where the round meshes landed in the engine's mesh table.
#[derive(Clone, Copy, Debug)]
pub struct Rounds {
    pub base: u32,
}

impl Rounds {
    #[must_use]
    pub const fn mesh(self, r: Round) -> u32 {
        self.base + r.offset()
    }

    #[must_use]
    pub const fn streak(self) -> u32 {
        self.base + STREAK_OFFSET
    }

    #[must_use]
    pub const fn core(self) -> u32 {
        self.base + CORE_OFFSET
    }

    #[must_use]
    pub const fn disc(self) -> u32 {
        self.base + DISC_OFFSET
    }
}

/// Build every mesh, in registration order: `Round::ALL`, then the streak,
/// then the core, then the disc.
#[must_use]
pub fn round_meshes() -> Vec<MeshData> {
    let mut meshes: Vec<MeshData> = Round::ALL.into_iter().map(round_mesh).collect();
    meshes.push(streak_mesh());
    meshes.push(core_mesh());
    meshes.push(disc_mesh());
    debug_assert_eq!(meshes.len(), MESH_COUNT);
    meshes
}

/// One point of a profile: `x` along the axis from the base, `r` the
/// radius there, both in the caller's units. A `corner` point is a crease
/// (the base's rim, a meplat's edge, a cannelure's walls): the surface on
/// either side keeps its own normal there. Elsewhere the two sides share
/// one, so an ogive lit at 12 sides reads as one smooth curve rather than
/// as bands.
#[derive(Clone, Copy, Debug)]
struct Pt {
    x: f32,
    r: f32,
    corner: bool,
}

const fn p(x: f32, r: f32) -> Pt {
    Pt {
        x,
        r,
        corner: false,
    }
}

const fn c(x: f32, r: f32) -> Pt {
    Pt { x, r, corner: true }
}

/// A band of the jacket that is darker than the rest: `(x0, x1, factor)`
/// in millimetres from the base.
type Band = (f32, f32, f32);

/// How dark the base is: the exposed lead at the heel.
const BASE_DIM: f32 = 0.72;
/// How far the darker base reaches up the round, millimetres.
const BASE_LEN: f32 = 0.9;
/// The cannelure's groove.
const CANNELURE_DIM: f32 = 0.55;
/// The Lapua's tip band.
const TIP_DIM: f32 = 0.5;

/// The profile of a round in millimetres, base at `x = 0`, and the darker
/// bands on its jacket besides the base every round has. The second point
/// is the heel's rim, at `Round::heel_mm`.
fn profile(r: Round) -> (Vec<Pt>, Vec<Band>) {
    let heel = r.heel_mm();
    match r {
        // A round nose: a small flat heel, a straight body, then an
        // elliptical nose to the point.
        Round::Nine => (
            vec![
                c(0.0, 0.0),
                c(0.0, heel),
                c(0.4, 4.5),
                p(7.0, 4.5),
                p(9.2, 4.35),
                p(11.25, 3.9),
                p(13.0, 3.18),
                p(14.36, 2.25),
                p(15.21, 1.16),
                p(15.5, 0.0),
            ],
            vec![],
        ),
        // A short boat tail, a cannelure a quarter of the way up, a
        // spitzer that ends in a small flat meplat.
        Round::Ak => (
            vec![
                c(0.0, 0.0),
                c(0.0, heel),
                c(2.5, 3.95),
                c(6.0, 3.95),
                c(6.3, 3.6),
                c(7.3, 3.6),
                c(7.6, 3.95),
                p(14.0, 3.95),
                p(18.0, 3.6),
                p(21.5, 2.8),
                p(24.5, 1.6),
                c(26.5, 0.6),
                c(26.5, 0.0),
            ],
            vec![(6.0, 7.6, CANNELURE_DIM)],
        ),
        // A boat tail and a spitzer to a fine point.
        Round::M4 => (
            vec![
                c(0.0, 0.0),
                c(0.0, heel),
                c(3.0, 2.85),
                p(11.0, 2.85),
                p(15.0, 2.6),
                p(18.5, 1.9),
                p(21.5, 0.9),
                p(23.0, 0.0),
            ],
            vec![],
        ),
        // No boat tail: a heel chamfer, a straight body with a cannelure,
        // and a truncated cone to a wide flat meplat.
        Round::Casull => (
            vec![
                c(0.0, 0.0),
                c(0.0, heel),
                c(0.5, 5.75),
                c(7.0, 5.75),
                c(7.3, 5.3),
                c(8.5, 5.3),
                c(8.8, 5.75),
                c(12.5, 5.75),
                c(19.0, 3.4),
                c(19.0, 0.0),
            ],
            vec![(7.0, 8.8, CANNELURE_DIM)],
        ),
        // A long boat tail, a body, and a long secant ogive to a point;
        // the last five millimetres of the jacket are the darker tip.
        Round::Lapua => (
            vec![
                c(0.0, 0.0),
                c(0.0, heel),
                c(5.5, 4.3),
                p(18.0, 4.3),
                p(24.0, 4.0),
                p(29.5, 3.3),
                p(34.0, 2.3),
                p(38.0, 1.1),
                p(41.0, 0.0),
            ],
            vec![(36.0, 41.0, TIP_DIM)],
        ),
    }
}

/// One round: its profile revolved, in metres, centred on its length.
fn round_mesh(r: Round) -> MeshData {
    let (pts, bands) = profile(r);
    let (_, len_mm) = r.calibre_mm();
    let half = len_mm * 0.5;
    let metres: Vec<Pt> = pts
        .iter()
        .map(|q| Pt {
            x: (q.x - half) * MM,
            r: q.r * MM,
            corner: q.corner,
        })
        .collect();
    revolve(&metres, SIDES, Some(jacket(len_mm, &bands)))
}

/// The streak: a cone from radius 1 at `x = 0` to a point at `x = 1`,
/// its base closed (the scene pass has no backface culling, so an open
/// cone shows its inside, lit inside out). Untextured: it wears the
/// weapon's tracer colour.
fn streak_mesh() -> MeshData {
    revolve(&[c(0.0, 0.0), c(0.0, 1.0), c(1.0, 0.0)], STREAK_SIDES, None)
}

/// The core: a frustum from radius 1 at `x = 0` to `CORE_NECK` at
/// `x = 1`, closed at both ends, so the tail cone can take over at its
/// narrow end without a step. Untextured, like the streak.
fn core_mesh() -> MeshData {
    revolve(
        &[c(0.0, 0.0), c(0.0, 1.0), c(1.0, CORE_NECK), c(1.0, 0.0)],
        STREAK_SIDES,
        None,
    )
}

/// The hole: a disc of radius 1 in its YZ plane, `x` from 0 to 1, closed
/// on both faces, so a scale of `(thick, r, r)` with +X turned onto a
/// face's outward normal lays a hole `r` in radius and `thick` deep on the
/// face with its back at the instance's position. Twelve sides like a
/// round, so a hole the size of a round is as round as the round.
/// Untextured: it wears `feel::MARK_COLOR`.
fn disc_mesh() -> MeshData {
    revolve(
        &[c(0.0, 0.0), c(0.0, 1.0), c(1.0, 1.0), c(1.0, 0.0)],
        SIDES,
        None,
    )
}

/// The jacket strip for a round `len_mm` long: copper, the base darker
/// over its first `BASE_LEN`, and each band darker by its factor. Row `j`
/// is the jacket at `v = (j + 0.5) / TEX_H` of the length from the base,
/// which is where the revolve's `v` puts it.
fn jacket(len_mm: f32, bands: &[Band]) -> TextureData {
    let mut rgba8 = Vec::with_capacity((TEX_W * TEX_H * 4) as usize);
    for j in 0..TEX_H {
        // Exact: row counts are tiny.
        #[allow(clippy::cast_precision_loss)]
        let x = (j as f32 + 0.5) / TEX_H as f32 * len_mm;
        let mut dim = if x < BASE_LEN { BASE_DIM } else { 1.0 };
        for &(x0, x1, f) in bands {
            if x >= x0 && x < x1 {
                dim = dim.min(f);
            }
        }
        for _ in 0..TEX_W {
            for ch in COPPER {
                // Truncation is the intent: a byte of colour.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                rgba8.push((ch * dim).round().clamp(0.0, 255.0) as u8);
            }
            rgba8.push(255);
        }
    }
    TextureData {
        width: TEX_W,
        height: TEX_H,
        rgba8,
    }
}

/// The outward unit normal, in the `(x, r)` plane, of the surface between
/// two profile points: the profile's tangent turned a quarter outward. A
/// cylinder wall gets `(0, 1)`, a flat base `(-1, 0)`, a meplat `(1, 0)`.
fn wall_normal(a: Pt, b: Pt) -> [f32; 2] {
    let (dx, dr) = (b.x - a.x, b.r - a.r);
    let len = dx.hypot(dr).max(1e-9);
    [-dr / len, dx / len]
}

fn mid_normal(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let (x, r) = (a[0] + b[0], a[1] + b[1]);
    let len = x.hypot(r);
    if len < 1e-6 { a } else { [x / len, r / len] }
}

/// Revolve a profile about +X with `sides` steps, as a triangle list with
/// `u` around and `v` along the length from the profile's first point.
/// Normals are smooth around the axis and, along the profile, shared
/// across a point unless it is a corner (see `Pt`). A point on the axis
/// closes its side of the surface with a fan rather than a degenerate
/// quad, so a nose or a flat base costs one triangle per side.
fn revolve(pts: &[Pt], sides: u32, texture: Option<TextureData>) -> MeshData {
    let sides = sides.max(3);
    let n = pts.len();
    assert!(n >= 2, "a profile needs two points");
    let x_min = pts.iter().map(|q| q.x).fold(f32::INFINITY, f32::min);
    let x_max = pts.iter().map(|q| q.x).fold(f32::NEG_INFINITY, f32::max);
    let span = (x_max - x_min).max(1e-9);
    let walls: Vec<[f32; 2]> = pts.windows(2).map(|w| wall_normal(w[0], w[1])).collect();
    // The normal each wall uses at its start point and at its end point.
    let at_start = |i: usize| {
        if i == 0 || pts[i].corner {
            walls[i]
        } else {
            mid_normal(walls[i - 1], walls[i])
        }
    };
    let at_end = |i: usize| {
        if i + 1 == n - 1 || pts[i + 1].corner {
            walls[i]
        } else {
            mid_normal(walls[i], walls[i + 1])
        }
    };
    // Exact: side counts are tiny.
    #[allow(clippy::cast_precision_loss)]
    let ring = |k: u32| -> (f32, f32, f32) {
        let f = k as f32 / sides as f32;
        let (s, c) = (f * TAU).sin_cos();
        (c, s, f)
    };
    let vertex = |q: Pt, nrm: [f32; 2], k: u32| -> MeshVertex {
        let (c, s, u) = ring(k);
        MeshVertex {
            pos: [q.x, q.r * c, q.r * s],
            normal: [nrm[0], nrm[1] * c, nrm[1] * s],
            uv: [u, (q.x - x_min) / span],
        }
    };
    let mut vertices = Vec::with_capacity((n - 1) * sides as usize * 6);
    for i in 0..n - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let (na, nb) = (at_start(i), at_end(i));
        for k in 0..sides {
            let a0 = vertex(a, na, k);
            let a1 = vertex(a, na, k + 1);
            let b0 = vertex(b, nb, k);
            let b1 = vertex(b, nb, k + 1);
            match (a.r < 1e-9, b.r < 1e-9) {
                (true, true) => {}
                (true, false) => vertices.extend([a0, b1, b0]),
                (false, true) => vertices.extend([a0, a1, b0]),
                (false, false) => vertices.extend([a0, a1, b1, a0, b1, b0]),
            }
        }
    }
    MeshData { vertices, texture }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::Bounds;
    use arena_core::shooter::WEAPON_COUNT;
    use ember_engine::glam::Vec3;

    fn radius_of(v: &MeshVertex) -> f32 {
        v.pos[1].hypot(v.pos[2])
    }

    /// Real diameter, metres.
    fn diameter(r: Round) -> f32 {
        r.calibre_mm().0 * MM
    }

    /// Real radius, metres.
    fn radius(r: Round) -> f32 {
        diameter(r) * 0.5
    }

    /// Every round's mesh is exactly its stated calibre and length, centred
    /// on its length, with its nose at +X, a few hundred unit-normal
    /// vertices and an 8-bit RGBA strip.
    #[test]
    fn every_round_is_its_real_size_and_points_forward() {
        let meshes = round_meshes();
        assert_eq!(
            meshes.len(),
            Round::ALL.len() + 3,
            "the rounds, the streak, the core and the disc"
        );
        assert_eq!(meshes.len(), MESH_COUNT);
        for r in Round::ALL {
            let m = &meshes[r.offset() as usize];
            let b = Bounds::of(m);
            let s = b.size();
            assert!((s.x - r.length()).abs() < 1e-4, "{r:?}: length {}", s.x);
            assert!((s.y - diameter(r)).abs() < 1e-4, "{r:?}: height {}", s.y);
            assert!((s.z - diameter(r)).abs() < 1e-4, "{r:?}: width {}", s.z);
            assert!(b.center().length() < 1e-5, "{r:?}: centre {}", b.center());
            // The heel table is the profile's rim: the widest vertex in the
            // base plane is exactly `heel_radius`, and it is never wider
            // than the body.
            let rim = m
                .vertices
                .iter()
                .filter(|v| (v.pos[0] - b.min.x).abs() < 1e-7)
                .map(radius_of)
                .fold(0.0, f32::max);
            assert!((rim - r.heel_radius()).abs() < 1e-7, "{r:?}: heel {rim}");
            assert!(
                r.heel_radius() <= radius(r),
                "{r:?}: the heel is the narrow end"
            );
            // The streak's base, `STREAK_LEAD` of the heel and `STREAK_INSET`
            // of the half-length up the body, is inside the round: every
            // ring between the base plane and that plane is wider than the
            // streak is, so the streak's disc is hidden and the streak
            // leaves the heel narrower than the heel. A future profile that
            // necks in above the base would fail here.
            let inset_x = b.min.x + r.length() * 0.5 * STREAK_INSET;
            let streak_r = r.heel_radius() * STREAK_LEAD;
            let narrowest = m
                .vertices
                .iter()
                .filter(|v| v.pos[0] <= inset_x + 1e-7 && radius_of(v) > 1e-7)
                .map(radius_of)
                .fold(f32::INFINITY, f32::min);
            assert!(
                narrowest > streak_r,
                "{r:?}: the body is {narrowest} at the streak's base of {streak_r}"
            );
            // The nose is at +X: a point, or a meplat narrower than the
            // body with the axis point at its centre; the base at -X has
            // a rim.
            for v in &m.vertices {
                if (v.pos[0] - b.max.x).abs() < 1e-7 {
                    assert!(
                        radius_of(v) < 0.65 * radius(r),
                        "{r:?}: the nose is narrower than the body"
                    );
                }
                let n = Vec3::from(v.normal);
                assert!((n.length() - 1.0).abs() < 1e-4, "{r:?}: normal {n}");
                assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0 && v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
            }
            let on_axis_at = |x: f32| {
                m.vertices
                    .iter()
                    .any(|v| (v.pos[0] - x).abs() < 1e-7 && radius_of(v) < 1e-7)
            };
            assert!(on_axis_at(b.max.x), "{r:?}: the nose closes on the axis");
            assert!(on_axis_at(b.min.x), "{r:?}: the base closes on the axis");
            assert!(
                m.vertices
                    .iter()
                    .any(|v| (v.pos[0] - b.min.x).abs() < 1e-7 && radius_of(v) > 1e-4),
                "{r:?}: the base has a rim"
            );
            assert_eq!(m.vertices.len() % 3, 0, "{r:?}: not a triangle list");
            assert!(
                m.vertices.len() < 1000,
                "{r:?}: {} vertices",
                m.vertices.len()
            );
            let t = m.texture.as_ref().expect("a jacket");
            assert_eq!((t.width, t.height), (TEX_W, TEX_H));
            assert_eq!(t.rgba8.len(), (t.width * t.height * 4) as usize);
            assert!(t.rgba8.chunks(4).all(|px| px[3] == 255), "{r:?}: opaque");
        }
        // The nose tip's normal points forward, the base's back.
        let nine = &meshes[Round::Nine.offset() as usize];
        let b = Bounds::of(nine);
        for v in &nine.vertices {
            if (v.pos[0] - b.max.x).abs() < 1e-7 {
                assert!(v.normal[0] > 0.0, "nose normal {:?}", v.normal);
            }
            if (v.pos[0] - b.min.x).abs() < 1e-7 && radius_of(v) < 1e-7 {
                assert!(v.normal[0] < -0.99, "base normal {:?}", v.normal);
            }
        }
    }

    /// The jacket: copper along the body, darker at the base, darker in
    /// the cannelure where the profile has one, darker at the Lapua's tip.
    #[test]
    fn the_jacket_is_copper_with_its_bands() {
        let row = |t: &TextureData, j: u32| {
            let i = (j * TEX_W * 4) as usize;
            [t.rgba8[i], t.rgba8[i + 1], t.rgba8[i + 2]]
        };
        let luma = |px: [u8; 3]| u32::from(px[0]) + u32::from(px[1]) + u32::from(px[2]);
        let meshes = round_meshes();
        let tex = |r: Round| meshes[r.offset() as usize].texture.clone().unwrap();
        // Row 32 is mid-body on every round: plain copper, red over green
        // over blue.
        for r in Round::ALL {
            let t = tex(r);
            let body = row(&t, 32);
            assert!(body[0] > body[1] && body[1] > body[2], "{r:?}: {body:?}");
            assert!(luma(row(&t, 0)) < luma(body), "{r:?}: the base is darker");
            // Every column of a row is the same: the jacket is uniform
            // round the axis.
            for j in [0, 32, 63] {
                let i = (j * TEX_W * 4) as usize;
                let first = &t.rgba8[i..i + 4];
                for k in 1..TEX_W as usize {
                    assert_eq!(&t.rgba8[i + k * 4..i + k * 4 + 4], first);
                }
            }
        }
        // The AK's cannelure sits between 6 and 7.6 mm of 26.5: row 16 is
        // 6.8 mm. The Casull's between 7 and 8.8 of 19: row 26 is 7.9 mm.
        assert!(luma(row(&tex(Round::Ak), 16)) < luma(row(&tex(Round::Ak), 32)));
        assert!(luma(row(&tex(Round::Casull), 26)) < luma(row(&tex(Round::Casull), 40)));
        // The Lapua's tip band is the last five millimetres of 41: row 62.
        assert!(luma(row(&tex(Round::Lapua), 62)) < luma(row(&tex(Round::Lapua), 32)));
        // The Nine and the M4 have no band: the body is uniform copper
        // from just past the base to the nose.
        for r in [Round::Nine, Round::M4] {
            let t = tex(r);
            for j in 4..TEX_H {
                assert_eq!(row(&t, j), row(&t, 32), "{r:?}: row {j}");
            }
        }
    }

    /// The streak is a closed cone: radius 1 at `x = 0`, a point at
    /// `x = 1`, eight sides, nothing else.
    #[test]
    fn the_streak_tapers_from_one_to_a_point() {
        let meshes = round_meshes();
        let s = &meshes[STREAK_OFFSET as usize];
        assert!(s.texture.is_none(), "the streak wears the tracer colour");
        // Eight wall triangles and eight base triangles.
        assert_eq!(s.vertices.len(), 16 * 3);
        for v in &s.vertices {
            let r = radius_of(v);
            if v.pos[0].abs() < 1e-7 {
                assert!(r < 1e-6 || (r - 1.0).abs() < 1e-6, "base ring {r}");
            } else {
                assert!((v.pos[0] - 1.0).abs() < 1e-7, "x {}", v.pos[0]);
                assert!(r < 1e-7, "the point has no radius: {r}");
            }
            let n = Vec3::from(v.normal);
            assert!((n.length() - 1.0).abs() < 1e-4);
        }
        let b = Bounds::of(s);
        assert!((b.min.x).abs() < 1e-7 && (b.max.x - 1.0).abs() < 1e-7);
        assert!((b.size().y - 2.0).abs() < 1e-6 && (b.size().z - 2.0).abs() < 1e-6);
    }

    /// The core is a frustum: radius 1 at `x = 0`, `CORE_NECK` at `x = 1`,
    /// closed at both ends, eight sides; and `CORE_NECK` is the ratio that
    /// makes the core and the tail one straight taper at full length.
    #[test]
    fn the_core_is_a_frustum_that_meets_the_tail() {
        let meshes = round_meshes();
        let c = &meshes[CORE_OFFSET as usize];
        assert!(c.texture.is_none(), "the core wears the tracer colour");
        // Eight base triangles, eight wall quads (two triangles each),
        // eight neck triangles.
        assert_eq!(c.vertices.len(), (8 + 16 + 8) * 3);
        for v in &c.vertices {
            let r = radius_of(v);
            if v.pos[0].abs() < 1e-7 {
                assert!(r < 1e-6 || (r - 1.0).abs() < 1e-6, "base ring {r}");
            } else {
                assert!((v.pos[0] - 1.0).abs() < 1e-7, "x {}", v.pos[0]);
                assert!(r < 1e-6 || (r - CORE_NECK).abs() < 1e-6, "neck ring {r}");
            }
            let n = Vec3::from(v.normal);
            assert!((n.length() - 1.0).abs() < 1e-4);
        }
        let b = Bounds::of(c);
        assert!((b.min.x).abs() < 1e-7 && (b.max.x - 1.0).abs() < 1e-7);
        assert!((b.size().y - 2.0).abs() < 1e-6 && (b.size().z - 2.0).abs() < 1e-6);
        // One taper: the core's slope over its length equals the tail's
        // over the rest of the tail length.
        let core_slope = (1.0 - CORE_NECK) / TRACER_CORE_LEN;
        let tail_slope = CORE_NECK / (TRACER_TAIL_LEN - TRACER_CORE_LEN);
        assert!(
            (core_slope - tail_slope).abs() < 1e-6,
            "{core_slope} vs {tail_slope}"
        );
    }

    /// The hole disc: radius 1 in the YZ plane, `x` from 0 to 1, closed on
    /// both faces (a face missing would show the disc's inside through it,
    /// there being no backface culling), twelve sides, untextured, and its
    /// back face at `x = 0` so an instance placed 1 mm off a wall keeps
    /// that 1 mm.
    #[test]
    fn the_disc_is_a_closed_twelve_sided_slab() {
        let meshes = round_meshes();
        let d = &meshes[DISC_OFFSET as usize];
        assert!(d.texture.is_none(), "the disc wears the mark colour");
        // Twelve triangles per face, twelve wall quads.
        assert_eq!(d.vertices.len(), (12 + 24 + 12) * 3);
        let (mut back, mut front) = (0, 0);
        for v in &d.vertices {
            let r = radius_of(v);
            assert!(r < 1e-6 || (r - 1.0).abs() < 1e-6, "ring {r}");
            if v.pos[0].abs() < 1e-7 {
                back += 1;
            } else {
                assert!((v.pos[0] - 1.0).abs() < 1e-7, "x {}", v.pos[0]);
                front += 1;
            }
            let n = Vec3::from(v.normal);
            assert!((n.length() - 1.0).abs() < 1e-4);
            // A face's normal is along the axis, the wall's is radial.
            if r < 1e-6 {
                assert!(n.x.abs() > 0.99, "face normal {n}");
            }
        }
        // The back face's fan and the wall's back ring, and the same at
        // the front: the slab is closed at both ends.
        assert_eq!(back, 12 * 3 + 24 * 3 / 2);
        assert_eq!(front, 12 * 3 + 24 * 3 / 2);
        let b = Bounds::of(d);
        assert!((b.min.x).abs() < 1e-7 && (b.max.x - 1.0).abs() < 1e-7);
        assert!((b.size().y - 2.0).abs() < 1e-6 && (b.size().z - 2.0).abs() < 1e-6);
    }

    /// The weapon table: the two 9 mm guns share a round, the rifles and
    /// the revolver have their own, the sniper its Lapua, the rocket none;
    /// an id off the table fires the sidearm's.
    #[test]
    fn round_for_maps_every_weapon() {
        assert_eq!(round_for(1), Some(Round::Nine));
        assert_eq!(round_for(2), Some(Round::Nine));
        assert_eq!(round_for(3), Some(Round::Ak));
        assert_eq!(round_for(4), Some(Round::M4));
        assert_eq!(round_for(5), Some(Round::Casull));
        assert_eq!(round_for(6), Some(Round::Lapua));
        assert_eq!(round_for(7), None);
        assert_eq!(round_for(0), Some(Round::Nine));
        assert_eq!(round_for(200), Some(Round::Nine));
        // Every weapon that traces has a round; the one that does not is
        // the rocket.
        for id in 1..=WEAPON_COUNT {
            assert_eq!(round_for(id).is_some(), crate::feel::traces(id), "id {id}");
        }
        // The registration order is the enum's, the streak, the core and
        // the disc after it.
        let rs = Rounds { base: 40 };
        for (k, r) in (0u32..).zip(Round::ALL) {
            assert_eq!(rs.mesh(r), 40 + k);
        }
        assert_eq!(rs.streak(), 45);
        assert_eq!(rs.core(), 46);
        assert_eq!(rs.disc(), 47);
        assert_eq!(STREAK_OFFSET, 5, "the streak follows the last round");
        assert_eq!(CORE_OFFSET, 6, "the core follows the streak");
        assert_eq!(DISC_OFFSET, 7, "the disc follows the core");
        assert_eq!(round_meshes().len(), 8);
        // Real sizes, in metres.
        assert!((Round::Nine.length() - 0.0155).abs() < 1e-7);
        assert!((diameter(Round::Lapua) - 0.0086).abs() < 1e-7);
        assert!((radius(Round::Casull) - 0.00575).abs() < 1e-7);
    }
}
