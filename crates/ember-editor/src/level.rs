//! The editor's document IS the sim's `Level`.
//!
//! Loading turns a level into editable objects; exporting turns those
//! objects back into a `Level` the sim can be handed. The round trip is the
//! point: a level that survives load-then-export unchanged is a level the
//! game would play exactly as the editor drew it.
//!
//! One conversion is lossy on purpose, and it refuses to be by snapping
//! rather than by silently discarding:
//!
//! * **Yaw.** `Obstacle` is an AABB on XZ, so only a quarter turn is
//!   representable (by swapping extents). The editor snaps object yaw live
//!   — see [`snap_yaw`] — so by the time anything is exported the rotation
//!   on screen is already the rotation that can be stored.
//!
//! Height off the floor used to be the other one: `Obstacle` had a top but
//! no bottom, and an object dragged up the Y axis was refused by name.
//! Since v13 a box carries `base`, so a lift is simply exported — a tunnel
//! roof IS a box dragged up the Y axis — and `from_level` puts it back at
//! that height.
//!
//! What a box IS (`Cover`) comes from the palette entry that placed it. An
//! `Obj` remembers its entry only by colour, so export maps the colour back
//! to the palette, and a loaded box is coloured by its kind so the round
//! trip holds for every kind the palette has. Kinds the palette lacks
//! (ammo, sandbag, rubble, plinth) load with the colour of the entry nearest
//! in height and export as that entry's kind; their geometry survives, their
//! kind does not, and that is recorded here rather than fixed because the
//! palette is what the user places from and a hidden ninth entry helps
//! nobody.

use glam::Vec3;
use pong_core::shooter::{Level, Obstacle};

use crate::Obj;
use crate::palette::{Class, Kind, PALETTE};

/// A quarter turn; the only rotation an AABB can carry.
pub const YAW_STEP: f32 = std::f32::consts::FRAC_PI_2;

/// Snap a yaw to the nearest quarter turn. Applied live while rotating an
/// object, so the editor never shows a rotation it cannot store.
#[must_use]
pub fn snap_yaw(yaw: f32) -> f32 {
    (yaw / YAW_STEP).round() * YAW_STEP
}

/// Does this object's footprint swap X and Z? A quarter or three-quarter
/// turn exchanges the extents; a half turn leaves an AABB unchanged.
fn extents_swapped(yaw: f32) -> bool {
    let quarter_parity = (yaw / YAW_STEP).round().rem_euclid(2.0);
    (quarter_parity - 1.0).abs() < f32::EPSILON
}

/// The AABB an object occupies, with its rotation folded into the extents.
fn footprint(o: &Obj) -> ([f32; 2], [f32; 2]) {
    let (mut hx, mut hz) = (o.scale.x * 0.5, o.scale.z * 0.5);
    if extents_swapped(o.yaw) {
        std::mem::swap(&mut hx, &mut hz);
    }
    ([o.pos.x - hx, o.pos.z - hz], [o.pos.x + hx, o.pos.z + hz])
}

/// The palette entry an object was placed from: the one whose colour it
/// carries, or — for an object whose colour matches nothing, which no
/// placement produces — the cover entry nearest its height.
fn kind_of(o: &Obj) -> &'static Kind {
    PALETTE
        .iter()
        .find(|k| k.class == Class::Object && k.color == o.color)
        .unwrap_or_else(|| kind_for_height(o.scale.y))
}

/// The palette entry a box of this kind loads as, or the entry nearest in
/// height when the palette has no entry for the kind.
fn kind_for_obstacle(o: &Obstacle) -> &'static Kind {
    PALETTE
        .iter()
        .find(|k| k.class == Class::Object && k.cover == o.kind)
        .unwrap_or_else(|| kind_for_height(o.h - o.base))
}

/// The cover entry whose height is closest to `h`.
fn kind_for_height(h: f32) -> &'static Kind {
    PALETTE
        .iter()
        .filter(|k| k.class == Class::Object)
        .min_by(|a, b| (a.scale.y - h).abs().total_cmp(&(b.scale.y - h).abs()))
        .expect("the palette has at least one cover entry")
}

/// Turn the edited objects into a level the sim could run.
///
/// Objects become obstacles: the box's bottom and top are simply where its
/// `scale.y` extent sits around `pos.y` — the palette put a sensible number
/// there, a resize changes it, a lift raises both, and what was dragged is
/// what ships. Spawns become points; their yaw is dropped, because
/// `Sim::add_player` hardcodes the initial aim and a facing the game
/// ignores would be a promise the format cannot keep.
///
/// Pads and decor are not authored here yet, so an exported level carries
/// none; the sim plays it without pads, exactly as its author left it.
#[must_use]
pub fn to_level(objects: &[Obj], arena_half: f32) -> Level {
    let mut obstacles = Vec::new();
    let mut spawns = Vec::new();
    for o in objects {
        match o.class {
            Class::Object => {
                let (min, max) = footprint(o);
                let half = o.scale.y * 0.5;
                obstacles.push(Obstacle::boxed(
                    kind_of(o).cover,
                    min,
                    max,
                    o.pos.y - half,
                    o.pos.y + half,
                ));
            }
            Class::Spawn => spawns.push([o.pos.x, o.pos.z]),
        }
    }
    Level {
        arena_half,
        obstacles,
        spawns,
        pads: Vec::new(),
        decor: Vec::new(),
    }
}

/// Serialize a level for a human to commit. Pretty-printed because the
/// round trip today is "export, commit the JSON, redeploy" and a diff
/// nobody can read is a bad artifact.
///
/// # Panics
///
/// Panics only if `serde_json` unexpectedly fails to serialize `Level`'s
/// plain data representation.
#[must_use]
pub fn to_json(objects: &[Obj], arena_half: f32) -> String {
    serde_json::to_string_pretty(&to_level(objects, arena_half)).expect("Level is plain data")
}

/// Build editable objects from a level, so the editor opens on a real
/// arena rather than an empty grid.
///
/// A raised box is placed with its centre halfway between its bottom and
/// its top and its extent the distance between them, so a roof loads
/// hanging where the sim has it. Colours come from the palette entry of the
/// box's kind, which keeps a loaded arena looking like an authored one and
/// is what lets the kind survive the round trip.
#[must_use]
pub fn from_level(level: &Level) -> Vec<Obj> {
    let mut out = Vec::with_capacity(level.obstacles.len() + level.spawns.len());
    for o in &level.obstacles {
        let sx = o.max[0] - o.min[0];
        let sz = o.max[1] - o.min[1];
        out.push(Obj {
            pos: Vec3::new(
                f32::midpoint(o.min[0], o.max[0]),
                f32::midpoint(o.base, o.h),
                f32::midpoint(o.min[1], o.max[1]),
            ),
            scale: Vec3::new(sx, o.h - o.base, sz),
            yaw: 0.0,
            color: kind_for_obstacle(o).color,
            class: Class::Object,
        });
    }
    let spawn_kind = PALETTE.iter().find(|k| k.class == Class::Spawn);
    for s in &level.spawns {
        let kind: &Kind = match spawn_kind {
            Some(k) => k,
            None => break,
        };
        out.push(Obj::from_kind(kind, Vec3::new(s[0], 0.0, s[1])));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pong_core::shooter::{ARENA_HALF, Cover};

    fn obj(pos: Vec3, scale: Vec3, yaw: f32, class: Class) -> Obj {
        Obj {
            pos,
            scale,
            yaw,
            color: Vec3::ONE,
            class,
        }
    }

    /// Geometry and kind of two obstacle lists agree, in order.
    fn assert_same_boxes(back: &[Obstacle], want: &[Obstacle], what: &str) {
        assert_eq!(back.len(), want.len(), "{what}: obstacle count");
        for (a, b) in back.iter().zip(want) {
            for i in 0..2 {
                assert!((a.min[i] - b.min[i]).abs() < 1e-4, "{what}: min");
                assert!((a.max[i] - b.max[i]).abs() < 1e-4, "{what}: max");
            }
            assert!(
                (a.h - b.h).abs() < 1e-6,
                "{what}: height {} vs {}",
                a.h,
                b.h
            );
            assert!(
                (a.base - b.base).abs() < 1e-6,
                "{what}: base {} vs {}",
                a.base,
                b.base
            );
        }
    }

    #[test]
    fn a_seeded_arena_survives_load_then_export() {
        // The round trip that matters: what the editor opens on must be
        // what it would write back, or every load silently edits the level.
        for seed in [0u64, 1, 7, 20_260_830] {
            let level = Level::from_seed(seed);
            let objects = from_level(&level);
            let back = to_level(&objects, level.arena_half);
            assert_eq!(back.arena_half, level.arena_half, "seed {seed}");
            assert_same_boxes(&back.obstacles, &level.obstacles, &format!("seed {seed}"));
            for (a, b) in back.obstacles.iter().zip(&level.obstacles) {
                assert_eq!(
                    a.kind, b.kind,
                    "seed {seed}: a crate must come back a crate"
                );
            }
            assert_eq!(back.spawns.len(), level.spawns.len(), "seed {seed}: spawns");
            for (a, b) in back.spawns.iter().zip(&level.spawns) {
                assert!((a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn trench_city_survives_load_then_export_where_the_palette_can_name_it() {
        // The authored arena has raised boxes and eight kinds. Geometry -
        // footprint, bottom, top - must survive for every box; the kind
        // survives for every kind the palette has an entry for, and the
        // module docs say which do not.
        let level = Level::trench_city();
        let back = to_level(&from_level(&level), level.arena_half);
        assert_same_boxes(&back.obstacles, &level.obstacles, "trench city");
        for (a, b) in back.obstacles.iter().zip(&level.obstacles) {
            if PALETTE.iter().any(|k| k.cover == b.kind) {
                assert_eq!(a.kind, b.kind, "{b:?} changed kind through the editor");
            }
        }
        assert!(
            level
                .obstacles
                .iter()
                .any(|o| o.kind == Cover::Roof && o.base > 0.0),
            "the level under test has a raised roof, or this proves nothing"
        );
        assert_eq!(back.spawns, level.spawns);
    }

    #[test]
    fn height_is_whatever_the_box_was_resized_to() {
        // No palette lookup on export: the preset's job was to put a
        // sensible number in scale.y, and a resize must survive.
        let objects = vec![obj(
            Vec3::new(2.0, 1.75, -3.0),
            Vec3::new(4.0, 3.5, 2.0),
            0.0,
            Class::Object,
        )];
        let level = to_level(&objects, ARENA_HALF);
        assert_eq!(level.obstacles[0].h, 3.5);
        assert_eq!(level.obstacles[0].base, 0.0);
        assert_eq!(level.obstacles[0].min, [0.0, -4.0]);
        assert_eq!(level.obstacles[0].max, [4.0, -2.0]);
    }

    #[test]
    fn a_quarter_turn_swaps_the_footprint_and_a_half_turn_does_not() {
        let long = Vec3::new(8.0, 3.0, 2.0);
        let at = Vec3::new(0.0, 1.5, 0.0);
        let square = |o: &Obstacle| (o.max[0] - o.min[0], o.max[1] - o.min[1]);

        let none = to_level(&[obj(at, long, 0.0, Class::Object)], ARENA_HALF);
        assert_eq!(square(&none.obstacles[0]), (8.0, 2.0));

        let quarter = to_level(&[obj(at, long, YAW_STEP, Class::Object)], ARENA_HALF);
        assert_eq!(
            square(&quarter.obstacles[0]),
            (2.0, 8.0),
            "a quarter turn swaps extents"
        );

        let half = to_level(&[obj(at, long, 2.0 * YAW_STEP, Class::Object)], ARENA_HALF);
        assert_eq!(
            square(&half.obstacles[0]),
            (8.0, 2.0),
            "a half turn is a no-op on an AABB"
        );

        let three = to_level(&[obj(at, long, 3.0 * YAW_STEP, Class::Object)], ARENA_HALF);
        assert_eq!(square(&three.obstacles[0]), (2.0, 8.0));
    }

    #[test]
    fn yaw_snaps_to_quarter_turns_including_negatives() {
        assert_eq!(snap_yaw(0.0), 0.0);
        assert!(
            (snap_yaw(0.2) - 0.0).abs() < 1e-6,
            "small turns round back to square"
        );
        assert!((snap_yaw(1.4) - YAW_STEP).abs() < 1e-6);
        assert!(
            (snap_yaw(-1.4) + YAW_STEP).abs() < 1e-6,
            "negatives snap symmetrically"
        );
        assert!(2.0f32.mul_add(-YAW_STEP, snap_yaw(3.0)).abs() < 1e-6);
    }

    #[test]
    fn a_lifted_box_exports_its_base_and_loads_back_at_the_same_height() {
        // The translate gizmo has a Y handle and nothing stops it. This
        // used to be refused by name because a box had no bottom; it is now
        // exactly how a roof is authored, so the lift must ship and must
        // come back.
        let roof = PALETTE.iter().find(|k| k.name == "roof").unwrap();
        let objects = vec![
            obj(
                Vec3::new(0.0, 0.6, 0.0),
                Vec3::splat(1.2),
                0.0,
                Class::Object,
            ),
            Obj {
                pos: Vec3::new(4.0, 2.7, 0.0),
                scale: Vec3::new(6.0, 0.4, 3.0),
                yaw: 0.0,
                color: roof.color,
                class: Class::Object,
            },
        ];
        let level = to_level(&objects, ARENA_HALF);
        assert_eq!(
            level.obstacles[0].base, 0.0,
            "the floor box stays on the floor"
        );
        let lifted = level.obstacles[1];
        assert!((lifted.base - 2.5).abs() < 1e-5, "base {}", lifted.base);
        assert!((lifted.h - 2.9).abs() < 1e-5, "top {}", lifted.h);
        assert_eq!(lifted.kind, Cover::Roof, "the roof entry exports as a roof");
        assert_eq!(lifted.min, [1.0, -1.5]);
        assert_eq!(lifted.max, [7.0, 1.5]);

        // And it plays as a roof: the sim walks under it and stands on it.
        let obs = &level.obstacles;
        assert_eq!(
            pong_core::shooter::support_height([4.0, 0.0], 0.6, 0.0, obs),
            0.0,
            "on the floor under the roof you are on the floor"
        );
        assert!(
            (pong_core::shooter::support_height([4.0, 0.0], 0.6, 2.9, obs) - 2.9).abs() < 1e-5,
            "on top of it you are on it"
        );

        // Loaded back, the object hangs where it was dragged.
        let back = from_level(&level);
        assert!(
            (back[1].pos.y - 2.7).abs() < 1e-5,
            "loaded at y {}",
            back[1].pos.y
        );
        assert!((back[1].scale.y - 0.4).abs() < 1e-5);
        assert_eq!(back[1].color, roof.color, "loaded with the roof's colour");
        let again = to_level(&back, ARENA_HALF);
        assert!((again.obstacles[1].base - 2.5).abs() < 1e-5);
        assert!((again.obstacles[1].h - 2.9).abs() < 1e-5);
        assert_eq!(again.obstacles[1].kind, Cover::Roof);
    }

    #[test]
    fn every_palette_entry_exports_as_the_cover_it_names() {
        // Placed from the palette (its colour, its extents, hung at its
        // base), each cover entry must come out as its own kind - and the
        // roof at its base, which Obj::from_kind does not yet apply.
        for k in PALETTE.iter().filter(|k| k.class == Class::Object) {
            let o = Obj {
                pos: Vec3::new(0.0, f32::mul_add(k.scale.y, 0.5, k.base), 0.0),
                scale: k.scale,
                yaw: 0.0,
                color: k.color,
                class: k.class,
            };
            let level = to_level(&[o], ARENA_HALF);
            let b = level.obstacles[0];
            assert_eq!(b.kind, k.cover, "{} exported as {:?}", k.name, b.kind);
            assert!(
                (b.base - k.base).abs() < 1e-5,
                "{}: base {}",
                k.name,
                b.base
            );
            assert!(
                (b.h - (k.base + k.scale.y)).abs() < 1e-5,
                "{}: top {}",
                k.name,
                b.h
            );
        }
    }

    #[test]
    fn spawns_export_as_points_and_drop_their_yaw() {
        // Sim::add_player hardcodes the initial aim, so a spawn facing
        // would be a promise the format cannot keep.
        let objects = vec![obj(
            Vec3::new(3.0, 0.9, -4.0),
            Vec3::new(0.8, 1.8, 0.8),
            1.1,
            Class::Spawn,
        )];
        let level = to_level(&objects, ARENA_HALF);
        assert_eq!(
            level.obstacles,
            Vec::<Obstacle>::new(),
            "a spawn is not cover"
        );
        assert_eq!(level.spawns, vec![[3.0, -4.0]]);
    }

    #[test]
    fn a_spawn_high_off_the_floor_is_still_a_point() {
        // A spawn is a point, and its own Y is not part of what gets
        // exported.
        let objects = vec![obj(
            Vec3::new(0.0, 12.0, 0.0),
            Vec3::new(0.8, 1.8, 0.8),
            0.0,
            Class::Spawn,
        )];
        let level = to_level(&objects, ARENA_HALF);
        assert_eq!(level.spawns, vec![[0.0, 0.0]]);
        assert_eq!(level.obstacles, Vec::<Obstacle>::new());
    }

    #[test]
    fn the_json_is_readable_and_parses_back() {
        let objects = from_level(&Level::from_seed(3));
        let json = to_json(&objects, ARENA_HALF);
        assert!(json.contains('\n'), "pretty-printed for a human diff");
        let parsed: Level = serde_json::from_str(&json).expect("round-trips through serde");
        assert_eq!(parsed.obstacles.len(), Level::from_seed(3).obstacles.len());
    }
}
