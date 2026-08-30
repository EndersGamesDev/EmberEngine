//! The shape palette, the two asset classes, and the command queue that
//! makes the native and the web editor one program rather than two.
//!
//! WHY A QUEUE. `ember_engine::run` MOVES the game and never returns on
//! native, so nothing outside `update()` holds a handle to the editor. The
//! web sidebar is page DOM (egui is not compiled on wasm), so it can only
//! reach Rust through a `#[wasm_bindgen]` free function. A mailbox drained
//! at the top of `update()` is the one shape that serves both: digit keys
//! push into it on native, DOM buttons push into it on the web, and the
//! editor never learns which shell it is running under.
//!
//! `thread_local!` rather than a `Mutex` or a `static mut`: both targets are
//! single-threaded here — wasm has no threads, and winit's event loop owns
//! the native one — so this needs no locking, no `unsafe`, and no `Send`
//! bound on `EmberGame`, which the trait does not have and should not grow.
//!
//! ON "GEOMETRIC SHAPES". The user asked for a sidebar of them, and the
//! engine has exactly ONE mesh: the built-in unit cube. So the palette is
//! box PRESETS — different proportions and colours — not different
//! primitives. That is an honest gap, recorded in the milestone's deferred
//! list rather than papered over: a cylinder or a wedge needs a real mesh in
//! the engine, and a palette of one shape you can actually place beats a
//! palette of four that do not exist.

use std::cell::RefCell;
use std::collections::VecDeque;

use ember_engine::glam::Vec3;
use ember_engine::Instance;

use crate::gizmo::Mode;

/// What a placed thing IS, which decides both how it draws and what it
/// becomes on export. Objects become collidable boxes; spawns become the
/// points players appear at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Object,
    Spawn,
}

/// One entry in the palette.
pub struct Kind {
    pub name: &'static str,
    pub class: Class,
    /// Extents in world units. Heights match the sim's two cover classes so
    /// the editor authors for the game that exists: crates are jumpable,
    /// containers are hard cover.
    pub scale: Vec3,
    pub color: Vec3,
}

/// Over-driven so a spawn marker reads as an annotation rather than as
/// scenery — the same trick and the same reason as the world axes.
const SPAWN_COLOR: Vec3 = Vec3::new(0.10, 2.6, 2.9);

pub const PALETTE: &[Kind] = &[
    Kind {
        name: "crate",
        class: Class::Object,
        // Under the sim's CRATE_MAX_H, so it is jumpable cover.
        scale: Vec3::new(2.2, 1.2, 2.2),
        color: Vec3::new(0.55, 0.42, 0.30),
    },
    Kind {
        name: "container",
        class: Class::Object,
        // Above CONTAINER_MIN_H: hard cover you cannot climb.
        scale: Vec3::new(3.0, 2.6, 6.0),
        color: Vec3::new(0.42, 0.45, 0.50),
    },
    Kind {
        name: "pillar",
        class: Class::Object,
        scale: Vec3::new(1.0, 5.0, 1.0),
        color: Vec3::new(0.38, 0.38, 0.42),
    },
    Kind {
        name: "plate",
        class: Class::Object,
        // Low enough to shoot over from a standing muzzle at 1.45.
        scale: Vec3::new(5.0, 0.4, 5.0),
        color: Vec3::new(0.30, 0.34, 0.38),
    },
    Kind {
        name: "wall",
        class: Class::Object,
        scale: Vec3::new(8.0, 3.0, 0.6),
        color: Vec3::new(0.34, 0.36, 0.40),
    },
    Kind {
        name: "spawn",
        class: Class::Spawn,
        // Roughly a standing player, so a spawn shows the space it needs.
        scale: Vec3::new(0.8, 1.8, 0.8),
        color: SPAWN_COLOR,
    },
];

/// Everything a shell can ask the editor to do. Deliberately small and
/// serialisable in shape: the web side sends these as JSON, tagged, so
/// `{"t":"arm","i":2}` and `{"t":"place"}` are the whole vocabulary.
#[derive(Clone, Copy, PartialEq, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Cmd {
    /// Arm a palette entry, by index.
    Arm { i: usize },
    /// Place the armed entry on the ground under the camera's aim.
    Place,
    /// Delete the current selection.
    Delete,
    SetMode { mode: Mode },
    /// Drop every placed object. Guarded in the UI, not here.
    Clear,
}

thread_local! {
    static MAILBOX: RefCell<VecDeque<Cmd>> = const { RefCell::new(VecDeque::new()) };
}

/// Queue a command from outside the frame loop.
pub fn push(cmd: Cmd) {
    MAILBOX.with(|m| m.borrow_mut().push_back(cmd));
}

/// Take everything queued so far. Called once at the top of `update()`, so a
/// command pushed DURING a frame lands on the next one — which keeps a
/// frame's behaviour a function of the state it started with.
pub fn drain() -> Vec<Cmd> {
    MAILBOX.with(|m| m.borrow_mut().drain(..).collect())
}

/// The palette as JSON for a shell to render: index, name and class, so a
/// page can build its sidebar from the real entries rather than a copy.
/// Colours stay out — a DOM button is styled by CSS, not by the world
/// colour the box will be drawn in.
pub fn palette_json() -> String {
    let entries: Vec<String> = PALETTE
        .iter()
        .enumerate()
        .map(|(i, k)| {
            format!(
                r#"{{"index":{i},"name":"{}","class":"{}"}}"#,
                k.name,
                match k.class {
                    Class::Object => "object",
                    Class::Spawn => "spawn",
                }
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

thread_local! {
    /// The document as JSON, refreshed whenever it CHANGES rather than
    /// every frame.
    ///
    /// The mailbox above only goes one way. `ember_engine::run` moves the
    /// game and never returns, so a shell has no handle to ask anything —
    /// which is fine for commands going in, and useless for a Save button
    /// that needs a value back. Keeping a snapshot here means the shell
    /// reads the latest document synchronously, with no round trip and no
    /// "not ready yet" state to handle. It costs one serialise per edit,
    /// which at this scale is nothing.
    static SNAPSHOT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Publish the document. Called by the editor after anything that changes
/// it — a place, a delete, the end of a drag.
pub fn set_snapshot(json: String) {
    SNAPSHOT.with(|s| *s.borrow_mut() = Some(json));
}

/// The latest document, or `None` before the first frame has published one.
pub fn snapshot() -> Option<String> {
    SNAPSHOT.with(|s| s.borrow().clone())
}

/// Empties the queue without acting on it. For tests, and for a shell that
/// is tearing down.
pub fn clear_queue() {
    SNAPSHOT.with(|s| *s.borrow_mut() = None);
    MAILBOX.with(|m| m.borrow_mut().clear());
}

/// Where a placed object should land: the ground plane under the camera's
/// aim, snapped to the grid.
///
/// Falls back to a point in front of the eye when the camera is level or
/// looking up, where the ray meets `y = 0` behind the viewer or not at all.
/// Placing something behind you because you happened to look at the horizon
/// is worse than placing it a fixed distance ahead.
pub fn ground_point(eye: Vec3, forward: Vec3, snap: f32) -> Vec3 {
    const FALLBACK_DIST: f32 = 14.0;
    let p = if forward.y < -1e-3 {
        let t = -eye.y / forward.y;
        if t > 0.0 && t < 400.0 {
            eye + forward * t
        } else {
            eye + forward * FALLBACK_DIST
        }
    } else {
        eye + forward * FALLBACK_DIST
    };
    snap_to(Vec3::new(p.x, 0.0, p.z), snap)
}

/// Snaps x and z to a grid, leaving y alone.
pub fn snap_to(p: Vec3, snap: f32) -> Vec3 {
    if snap <= 0.0 {
        return p;
    }
    Vec3::new((p.x / snap).round() * snap, p.y, (p.z / snap).round() * snap)
}

/// Extra, NON-PICKABLE geometry marking a spawn point: a flat pad under the
/// post, so a spawn reads as a place rather than as a small crate.
///
/// Kept out of the pickable list on purpose. Picking indexes objects one to
/// one, and decoration that could be clicked would break that correspondence
/// — you would select a pad and move a spawn, or worse, an index nobody owns.
pub fn spawn_decoration(pos: Vec3) -> Instance {
    Instance::new(
        Vec3::new(pos.x, 0.02, pos.z),
        Vec3::new(2.4, 0.04, 2.4),
        SPAWN_COLOR * 0.5,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_web_shells_command_vocabulary_parses() {
        // These strings are what web/engine/index.html actually sends. The
        // page is not compiled or type-checked against Rust, so this is the
        // only thing standing between a renamed variant and a sidebar
        // button that silently stops working.
        let cases: [(&str, Cmd); 5] = [
            (r#"{"t":"arm","i":2}"#, Cmd::Arm { i: 2 }),
            (r#"{"t":"place"}"#, Cmd::Place),
            (r#"{"t":"delete"}"#, Cmd::Delete),
            (
                r#"{"t":"set_mode","mode":"rotate"}"#,
                Cmd::SetMode {
                    mode: crate::gizmo::Mode::Rotate,
                },
            ),
            (r#"{"t":"clear"}"#, Cmd::Clear),
        ];
        for (json, want) in cases {
            let got: Cmd = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            assert_eq!(got, want, "{json}");
        }
        // Every mode the sidebar offers.
        for m in ["translate", "rotate", "scale"] {
            let json = format!(r#"{{"t":"set_mode","mode":"{m}"}}"#);
            serde_json::from_str::<Cmd>(&json).unwrap_or_else(|e| panic!("{json}: {e}"));
        }
    }

    #[test]
    fn the_palette_json_describes_every_entry() {
        // The page builds its sidebar from this rather than duplicating the
        // entries, so it must list all of them, in order, with the class
        // the button's tooltip depends on.
        let json = palette_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let arr = parsed.as_array().expect("an array");
        assert_eq!(arr.len(), PALETTE.len());
        for (i, (entry, kind)) in arr.iter().zip(PALETTE).enumerate() {
            assert_eq!(entry["index"], i);
            assert_eq!(entry["name"], kind.name);
            let class = if kind.class == Class::Spawn { "spawn" } else { "object" };
            assert_eq!(entry["class"], class);
        }
    }

    #[test]
    fn the_snapshot_is_what_was_last_published() {
        clear_queue();
        assert_eq!(snapshot(), None, "nothing published yet");
        set_snapshot("{\"a\":1}".into());
        assert_eq!(snapshot().as_deref(), Some("{\"a\":1}"));
        set_snapshot("{\"a\":2}".into());
        assert_eq!(snapshot().as_deref(), Some("{\"a\":2}"), "latest wins");
        clear_queue();
        assert_eq!(snapshot(), None, "teardown clears it");
    }

    #[test]
    fn the_queue_is_fifo_and_empties_on_drain() {
        clear_queue();
        push(Cmd::Arm { i: 2 });
        push(Cmd::Place);
        push(Cmd::Delete);
        assert_eq!(drain(), vec![Cmd::Arm { i: 2 }, Cmd::Place, Cmd::Delete]);
        assert!(drain().is_empty(), "a drain must empty the queue");
    }

    #[test]
    fn a_command_pushed_after_a_drain_waits_for_the_next_one() {
        // The property that keeps a frame's behaviour a function of the state
        // it began with: a sidebar click mid-frame must not half-apply.
        clear_queue();
        push(Cmd::Place);
        let first = drain();
        push(Cmd::Delete);
        assert_eq!(first, vec![Cmd::Place]);
        assert_eq!(drain(), vec![Cmd::Delete]);
    }

    #[test]
    fn the_palette_covers_both_asset_classes() {
        // The user asked for "a asset class objects and then characters to
        // place spawn points" — both must exist or the ask is half-built.
        assert!(PALETTE.iter().any(|k| k.class == Class::Object));
        assert!(PALETTE.iter().any(|k| k.class == Class::Spawn));
    }

    #[test]
    fn cover_presets_straddle_the_sims_two_cover_classes() {
        // An editor whose "crate" is not jumpable and whose "container" is
        // would author levels that play the opposite of how they read.
        let crate_k = PALETTE.iter().find(|k| k.name == "crate").unwrap();
        let container = PALETTE.iter().find(|k| k.name == "container").unwrap();
        assert!(
            crate_k.scale.y <= 1.5,
            "a crate must stay under the sim's jumpable ceiling"
        );
        assert!(
            container.scale.y >= 2.4,
            "a container must be tall enough to be hard cover"
        );
    }

    #[test]
    fn every_preset_has_positive_extents() {
        // A zero or negative extent is an unpickable degenerate slab, and the
        // palette is the one place it could ship as data rather than as a bug.
        for k in PALETTE {
            assert!(
                k.scale.min_element() > 0.0,
                "{} has a non-positive extent {:?}",
                k.name,
                k.scale
            );
        }
    }

    #[test]
    fn ground_point_lands_where_the_camera_looks() {
        // Eye 10 up, looking down at 45 degrees: the ray meets the ground 10
        // ahead.
        let eye = Vec3::new(0.0, 10.0, 0.0);
        let fwd = Vec3::new(0.0, -1.0, -1.0).normalize();
        let p = ground_point(eye, fwd, 0.0);
        assert!((p.y).abs() < 1e-6, "must land ON the ground");
        assert!((p.z + 10.0).abs() < 1e-3, "expected z = -10, got {}", p.z);
    }

    #[test]
    fn looking_up_places_ahead_rather_than_behind() {
        // The ray meets y = 0 BEHIND the viewer here. Solving it blindly puts
        // the object over your shoulder, which reads as the click doing
        // nothing at all.
        let eye = Vec3::new(0.0, 2.0, 0.0);
        let fwd = Vec3::new(0.0, 0.5, -1.0).normalize();
        let p = ground_point(eye, fwd, 0.0);
        assert!(p.z < 0.0, "placed at z = {} — behind the camera", p.z);
    }

    #[test]
    fn a_nearly_level_view_does_not_place_at_infinity() {
        // Grazing the horizon makes t enormous; without the bound the object
        // lands kilometres away, past the 500-unit far plane, invisible.
        let eye = Vec3::new(0.0, 1.5, 0.0);
        let fwd = Vec3::new(0.0, -0.0005, -1.0).normalize();
        let p = ground_point(eye, fwd, 0.0);
        assert!(
            p.length() < 100.0,
            "placed {} units away — past the far plane",
            p.length()
        );
    }

    #[test]
    fn snapping_is_to_the_nearest_and_leaves_height_alone() {
        let p = snap_to(Vec3::new(2.6, 5.0, -3.4), 2.0);
        assert_eq!(p.x, 2.0);
        assert_eq!(p.z, -4.0);
        assert_eq!(p.y, 5.0, "snapping is a ground-plane concern");
        // A zero step must be a no-op rather than a division.
        let q = snap_to(Vec3::new(1.234, 0.0, 5.678), 0.0);
        assert_eq!(q.x, 1.234);
    }

    #[test]
    fn spawn_decoration_lies_flat_on_the_ground() {
        let d = spawn_decoration(Vec3::new(4.0, 9.0, -2.0));
        assert!(d.position.y < 0.1, "the pad must sit on the floor");
        assert_eq!(d.position.x, 4.0);
        assert_eq!(d.position.z, -2.0);
        assert!(d.scale.y < d.scale.x, "a pad is flat");
    }
}
