//! The editor's document IS the sim's `Level`.
//!
//! Loading turns a seeded arena into editable objects; exporting turns
//! those objects back into a `Level` the sim could be handed. The round
//! trip is the point: a level that survives load-then-export unchanged is
//! a level the game would play exactly as the editor drew it.
//!
//! Two conversions are lossy on purpose, and both refuse rather than
//! silently discard:
//!
//! * **Yaw.** `Obstacle` is an AABB on XZ, so only a quarter turn is
//!   representable (by swapping extents). The editor snaps object yaw live
//!   — see [`snap_yaw`] — so by the time anything is exported the rotation
//!   on screen is already the rotation that can be stored.
//! * **Height off the floor.** `Obstacle` has a top but no bottom; every
//!   box stands on the ground. An object dragged up the Y axis cannot be
//!   represented, so exporting one is an error naming the object rather
//!   than a box that quietly sinks to the floor.

use glam::Vec3;
use pong_core::shooter::{Level, Obstacle};

use crate::palette::{Class, Kind};
use crate::Obj;

/// A quarter turn; the only rotation an AABB can carry.
pub const YAW_STEP: f32 = std::f32::consts::FRAC_PI_2;
/// How far off the floor a box may sit before export refuses it. Generous
/// enough to absorb float drift from a scale drag, far below a deliberate
/// lift.
pub const FLOOR_TOLERANCE: f32 = 1e-3;

/// Snap a yaw to the nearest quarter turn. Applied live while rotating an
/// object, so the editor never shows a rotation it cannot store.
pub fn snap_yaw(yaw: f32) -> f32 {
    (yaw / YAW_STEP).round() * YAW_STEP
}

/// Does this object's footprint swap X and Z? A quarter or three-quarter
/// turn exchanges the extents; a half turn leaves an AABB unchanged.
fn extents_swapped(yaw: f32) -> bool {
    let quarters = (yaw / YAW_STEP).round() as i32;
    quarters.rem_euclid(2) == 1
}

/// The AABB an object occupies, with its rotation folded into the extents.
fn footprint(o: &Obj) -> ([f32; 2], [f32; 2]) {
    let (mut hx, mut hz) = (o.scale.x * 0.5, o.scale.z * 0.5);
    if extents_swapped(o.yaw) {
        std::mem::swap(&mut hx, &mut hz);
    }
    (
        [o.pos.x - hx, o.pos.z - hz],
        [o.pos.x + hx, o.pos.z + hz],
    )
}

/// Why an export was refused, with enough detail to fix it.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    /// A box was dragged off the floor. `Obstacle` cannot express it.
    FloatingObject { index: usize, base_y: f32 },
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::FloatingObject { index, base_y } => write!(
                f,
                "object {index} floats {base_y:.2} above the floor; the sim's \
                 obstacles have a top but no bottom, so drop it to the ground \
                 or delete it"
            ),
        }
    }
}

/// Turn the edited objects into a level the sim could run.
///
/// Objects become obstacles whose height is simply their `scale.y` — the
/// palette put a sensible number there and a resize changes it, so what
/// was dragged is what ships. Spawns become points; their yaw is dropped,
/// because `Sim::add_player` hardcodes the initial aim and a facing the
/// game ignores would be a promise the format cannot keep.
pub fn to_level(objects: &[Obj], arena_half: f32) -> Result<Level, ExportError> {
    let mut obstacles = Vec::new();
    let mut spawns = Vec::new();
    for (index, o) in objects.iter().enumerate() {
        match o.class {
            Class::Object => {
                let base_y = o.pos.y - o.scale.y * 0.5;
                if base_y.abs() > FLOOR_TOLERANCE {
                    return Err(ExportError::FloatingObject { index, base_y });
                }
                let (min, max) = footprint(o);
                obstacles.push(Obstacle {
                    min,
                    max,
                    h: o.scale.y,
                });
            }
            Class::Spawn => spawns.push([o.pos.x, o.pos.z]),
        }
    }
    Ok(Level {
        arena_half,
        obstacles,
        spawns,
    })
}

/// Serialize a level for a human to commit. Pretty-printed because the
/// round trip today is "export, commit the JSON, redeploy" and a diff
/// nobody can read is a bad artifact.
pub fn to_json(objects: &[Obj], arena_half: f32) -> Result<String, ExportError> {
    let level = to_level(objects, arena_half)?;
    Ok(serde_json::to_string_pretty(&level).expect("Level is plain data"))
}

/// Build editable objects from a level, so the editor opens on a real
/// arena rather than an empty grid.
///
/// Colours come from the palette entry a box's height would have placed,
/// which keeps a loaded arena looking like an authored one.
pub fn from_level(level: &Level) -> Vec<Obj> {
    let mut out = Vec::with_capacity(level.obstacles.len() + level.spawns.len());
    for o in &level.obstacles {
        let sx = o.max[0] - o.min[0];
        let sz = o.max[1] - o.min[1];
        out.push(Obj {
            pos: Vec3::new(
                (o.min[0] + o.max[0]) * 0.5,
                o.h * 0.5,
                (o.min[1] + o.max[1]) * 0.5,
            ),
            scale: Vec3::new(sx, o.h, sz),
            yaw: 0.0,
            color: colour_for_height(o.h),
            class: Class::Object,
        });
    }
    let spawn_kind = crate::palette::PALETTE
        .iter()
        .find(|k| k.class == Class::Spawn);
    for s in &level.spawns {
        let kind: &Kind = match spawn_kind {
            Some(k) => k,
            None => break,
        };
        out.push(Obj::from_kind(kind, Vec3::new(s[0], 0.0, s[1])));
    }
    out
}

/// The palette colour of whichever cover class this height belongs to, so
/// a loaded crate looks like a placed crate.
fn colour_for_height(h: f32) -> Vec3 {
    crate::palette::PALETTE
        .iter()
        .filter(|k| k.class == Class::Object)
        .min_by(|a, b| {
            (a.scale.y - h)
                .abs()
                .total_cmp(&(b.scale.y - h).abs())
        })
        .map(|k| k.color)
        .unwrap_or(Vec3::new(0.42, 0.45, 0.50))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pong_core::shooter::ARENA_HALF;

    fn obj(pos: Vec3, scale: Vec3, yaw: f32, class: Class) -> Obj {
        Obj {
            pos,
            scale,
            yaw,
            color: Vec3::ONE,
            class,
        }
    }

    #[test]
    fn a_seeded_arena_survives_load_then_export() {
        // The round trip that matters: what the editor opens on must be
        // what it would write back, or every load silently edits the level.
        for seed in [0u64, 1, 7, 20260830] {
            let level = Level::from_seed(seed);
            let objects = from_level(&level);
            let back = to_level(&objects, level.arena_half).expect("seeded arena exports");
            assert_eq!(back.arena_half, level.arena_half, "seed {seed}");
            assert_eq!(
                back.obstacles.len(),
                level.obstacles.len(),
                "seed {seed}: obstacle count"
            );
            for (a, b) in back.obstacles.iter().zip(&level.obstacles) {
                for i in 0..2 {
                    assert!((a.min[i] - b.min[i]).abs() < 1e-4, "seed {seed}: min");
                    assert!((a.max[i] - b.max[i]).abs() < 1e-4, "seed {seed}: max");
                }
                assert!((a.h - b.h).abs() < 1e-6, "seed {seed}: height {} vs {}", a.h, b.h);
            }
            assert_eq!(back.spawns.len(), level.spawns.len(), "seed {seed}: spawns");
            for (a, b) in back.spawns.iter().zip(&level.spawns) {
                assert!((a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4);
            }
        }
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
        let level = to_level(&objects, ARENA_HALF).unwrap();
        assert_eq!(level.obstacles[0].h, 3.5);
        assert_eq!(level.obstacles[0].min, [0.0, -4.0]);
        assert_eq!(level.obstacles[0].max, [4.0, -2.0]);
    }

    #[test]
    fn a_quarter_turn_swaps_the_footprint_and_a_half_turn_does_not() {
        let long = Vec3::new(8.0, 3.0, 2.0);
        let at = Vec3::new(0.0, 1.5, 0.0);
        let square = |o: &Obstacle| (o.max[0] - o.min[0], o.max[1] - o.min[1]);

        let none = to_level(&[obj(at, long, 0.0, Class::Object)], ARENA_HALF).unwrap();
        assert_eq!(square(&none.obstacles[0]), (8.0, 2.0));

        let quarter = to_level(&[obj(at, long, YAW_STEP, Class::Object)], ARENA_HALF).unwrap();
        assert_eq!(square(&quarter.obstacles[0]), (2.0, 8.0), "a quarter turn swaps extents");

        let half = to_level(&[obj(at, long, 2.0 * YAW_STEP, Class::Object)], ARENA_HALF).unwrap();
        assert_eq!(square(&half.obstacles[0]), (8.0, 2.0), "a half turn is a no-op on an AABB");

        let three = to_level(&[obj(at, long, 3.0 * YAW_STEP, Class::Object)], ARENA_HALF).unwrap();
        assert_eq!(square(&three.obstacles[0]), (2.0, 8.0));
    }

    #[test]
    fn yaw_snaps_to_quarter_turns_including_negatives() {
        assert_eq!(snap_yaw(0.0), 0.0);
        assert!((snap_yaw(0.2) - 0.0).abs() < 1e-6, "small turns round back to square");
        assert!((snap_yaw(1.4) - YAW_STEP).abs() < 1e-6);
        assert!((snap_yaw(-1.4) + YAW_STEP).abs() < 1e-6, "negatives snap symmetrically");
        assert!((snap_yaw(3.0) - 2.0 * YAW_STEP).abs() < 1e-6);
    }

    #[test]
    fn a_floating_box_is_refused_by_name_rather_than_dropped() {
        // The translate gizmo has a Y handle and nothing stops it, so this
        // is reachable by ordinary use. Silently flattening it would ship a
        // level that is not the one on screen.
        let objects = vec![
            obj(Vec3::new(0.0, 0.6, 0.0), Vec3::splat(1.2), 0.0, Class::Object),
            obj(Vec3::new(4.0, 5.0, 0.0), Vec3::splat(1.2), 0.0, Class::Object),
        ];
        match to_level(&objects, ARENA_HALF) {
            Err(ExportError::FloatingObject { index, base_y }) => {
                assert_eq!(index, 1, "names the offending object");
                assert!((base_y - 4.4).abs() < 1e-4, "reports how far up: {base_y}");
                let msg = ExportError::FloatingObject { index, base_y }.to_string();
                assert!(msg.contains("object 1") && msg.contains("floor"), "{msg}");
            }
            other => panic!("expected a refusal, got {other:?}"),
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
        let level = to_level(&objects, ARENA_HALF).unwrap();
        assert!(level.obstacles.is_empty(), "a spawn is not cover");
        assert_eq!(level.spawns, vec![[3.0, -4.0]]);
    }

    #[test]
    fn a_spawn_high_off_the_floor_is_allowed() {
        // Only obstacles are floor-bound; a spawn is a point, and its own
        // Y is not part of what gets exported.
        let objects = vec![obj(
            Vec3::new(0.0, 12.0, 0.0),
            Vec3::new(0.8, 1.8, 0.8),
            0.0,
            Class::Spawn,
        )];
        assert!(to_level(&objects, ARENA_HALF).is_ok());
    }

    #[test]
    fn the_json_is_readable_and_parses_back() {
        let objects = from_level(&Level::from_seed(3));
        let json = to_json(&objects, ARENA_HALF).unwrap();
        assert!(json.contains('\n'), "pretty-printed for a human diff");
        let parsed: Level = serde_json::from_str(&json).expect("round-trips through serde");
        assert_eq!(parsed.obstacles.len(), Level::from_seed(3).obstacles.len());
    }
}
