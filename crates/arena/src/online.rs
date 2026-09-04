//! Online arena shooter client: renders the server-authoritative match and
//! aims with relative mouse deltas under pointer lock (yaw plus a pitch the
//! server now honours), carrying the cyberpunk sidearm in one hand and a
//! shield in the other.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;

use arena_core::proto::{BState, C2S, PROTO_VERSION, PState, PlayerMeta, S2C, STATE_EVERY_TICKS};
use arena_core::shooter::{
    Cover, Decor, EYE_CROUCH, EYE_STAND, FFA_FRAG_LIMIT, FIXED_DT, GameMode, HILL_CONTESTED,
    HILL_FREE, HILL_LIMIT, Hill, Level, MAX_HP, MAX_PITCH, MELEE_COOLDOWN, Obstacle, Projectile,
    RESERVE_INFINITE, SHOT_BODY, SHOT_SHIELD, SIDEARM, TDM_FRAG_LIMIT, WEAPON_COUNT, move_circle,
    stance_speed, step_vertical, weapon_name, weapon_stats,
};
use ember_engine::glam::{Mat3, Quat, Vec2, Vec3};
use ember_engine::{
    Camera, EmberGame, Feedback, Frame, InputState, Instance, KeyCode, MouseButton, PadButton,
};
use serde::Deserialize;

use crate::feel::{self, Climb, Cue, GLOW_BLUE, Mark, Play, Puff, Rod, Shake, Tracer, weapon_feel};
use crate::props::{LOOT_SPENT_TINT, Prop, Props, tex};
use crate::rounds::{self, Round, Rounds};
use crate::script;
use crate::sound::{Audio, BUDGET, Dist, Sfx};

/// What a part does when the weapon fires (v15). Decided by the part's
/// node name, which is the asset's contract with this file: `cylinder*`
/// spins one chamber per shot, `hammer` cocks and falls, `trigger` pulls,
/// everything else rides rigidly with the frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartAnim {
    Fixed,
    Cylinder,
    Hammer,
    Trigger,
}

/// One colored piece of a loaded GLB model.
#[derive(Clone)]
pub struct Part {
    pub mesh: u32,
    pub color: Vec3,
    pub is_strip: bool,
    pub anim: PartAnim,
    /// Model-space point the part rotates about, from the sidecar.
    /// `load_glb` bakes every node's transform into its vertices and
    /// discards the hierarchy, so a pivot cannot be recovered from the GLB
    /// (docs/asset-pipeline.md, "Pivots do not survive import").
    pub pivot: Vec3,
}

/// One slot per weapon id (`1..=WEAPON_COUNT`); slot 0 is never read.
const WEAPON_SLOTS: usize = WEAPON_COUNT as usize + 1;

/// The Blender-authored viewmodel: every weapon by id, the hands that hold
/// them, and what the off-hand and the melee key bring out.
#[derive(Clone, Default)]
pub struct Assets {
    /// Part lists by weapon id. An empty list is a weapon whose node is not
    /// in the GLB (the M4 today, any future gap): `weapon_parts` draws the
    /// sidearm for it, so a missing node shows as the wrong rifle, never as
    /// an empty hand.
    pub weapons: [Vec<Part>; WEAPON_SLOTS],
    /// Model-space muzzle tip per weapon id, from the sidecar; where the
    /// flash sits. Filled with the sidearm's for any weapon the sidecar
    /// does not name.
    pub muzzles: [Vec3; WEAPON_SLOTS],
    /// The RPG-7's rocket, in the launcher's own frame, so it draws in the
    /// tube at the launcher's transform and flies on its own as the
    /// projectile.
    pub rocket: Vec<Part>,
    pub arms: Vec<Part>,
    /// The scutum, drawn where the box plate used to be. Its model origin
    /// is the handle behind the boss, so the centres `push_shield` was
    /// given carry over unchanged.
    pub shield: Vec<Part>,
    /// The Murasama: blade along +X, tip forward, origin at the grip.
    pub sword: Vec<Part>,
    /// The fist and forearm that hold the sword, in the sword's own frame.
    /// Viewmodel-only, like `arms`.
    pub fist: Vec<Part>,
}

impl Assets {
    /// The parts that draw weapon `id`, and whether they are its own. An id
    /// whose list is empty falls back to the sidearm's, so the caller draws
    /// the fallback with the missing weapon's accent and the strip still
    /// says which gun it stands for.
    #[must_use]
    pub fn weapon_parts(&self, id: u8) -> (&[Part], bool) {
        let own = &self.weapons[slot(id)];
        if own.is_empty() {
            (&self.weapons[SIDEARM as usize], false)
        } else {
            (own, true)
        }
    }

    /// The muzzle tip for weapon `id`, in the frame of whatever
    /// `weapon_parts` returns for it.
    #[must_use]
    pub fn muzzle_of(&self, id: u8) -> Vec3 {
        let (_, own) = self.weapon_parts(id);
        if own {
            self.muzzles[slot(id)]
        } else {
            self.muzzles[SIDEARM as usize]
        }
    }
}

/// Table index for a weapon id: ids off the table share the sidearm's
/// slot, exactly as `weapon_stats` answers them.
const fn slot(id: u8) -> usize {
    if id >= 1 && id <= WEAPON_COUNT {
        id as usize
    } else {
        SIDEARM as usize
    }
}

/// Which list a viewmodel node belongs in.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum Slot {
    Weapon(u8),
    Rocket,
    Arms,
    Shield,
    Sword,
    Fist,
}

/// Sort a GLB node by name. The `w_*` weapon nodes are matched exactly
/// first (the revolver's five parts by prefix, since its moving parts carry
/// their own suffix), then the sidearm's `rifle`, then the three v17 nodes,
/// then the older prefix rule, which still sends anything called
/// `arm*`/`hand*` to the viewmodel-only list; anything unnamed rides the
/// sidearm, as it always did. `hand_sword` keeps that prefix on purpose: an
/// older client that has only the prefix rule hides it rather than welding
/// it to a remote player's rifle.
///
/// Every weapon also gets a `w_<weapon>_<part>` prefix rule, because the
/// build's `--split` fallback (`tools/v18/build_weapons.py`) ships a
/// multi-material gun as one node per material named that way when the
/// atlas bake fails; without the rule those parts would ride the sidearm.
/// The rocket's exact name is matched before the `w_rpg7_` prefix so it
/// keeps its own list.
fn classify(name: &str) -> Slot {
    match name {
        "w_vityaz" => Slot::Weapon(2),
        "w_ak47" => Slot::Weapon(3),
        "w_m4" => Slot::Weapon(4),
        _ if name.starts_with("w_revolver_") => Slot::Weapon(5),
        "w_sniper" => Slot::Weapon(6),
        "w_rpg7" => Slot::Weapon(7),
        "w_rpg7_rocket" => Slot::Rocket,
        _ if name.starts_with("w_vityaz_") => Slot::Weapon(2),
        _ if name.starts_with("w_ak47_") => Slot::Weapon(3),
        _ if name.starts_with("w_m4_") => Slot::Weapon(4),
        _ if name.starts_with("w_sniper_") => Slot::Weapon(6),
        _ if name.starts_with("w_rpg7_") => Slot::Weapon(7),
        "rifle" => Slot::Weapon(SIDEARM),
        "shield" => Slot::Shield,
        "sword" => Slot::Sword,
        "hand_sword" => Slot::Fist,
        _ if name.starts_with("arm") || name.starts_with("hand") => Slot::Arms,
        _ => Slot::Weapon(SIDEARM),
    }
}

/// What a named part does when its weapon fires. The v18 names carry the
/// weapon as a prefix and the role as a suffix; the bare v15 names are kept
/// so an older sidecar still animates.
fn anim_of(name: &str) -> PartAnim {
    if name.ends_with("_cylinder") || name.starts_with("cylinder") {
        PartAnim::Cylinder
    } else if name.ends_with("_hammer") || name == "hammer" {
        PartAnim::Hammer
    } else if name.ends_with("_trigger") || name == "trigger" {
        PartAnim::Trigger
    } else {
        PartAnim::Fixed
    }
}

/// The sidecar `tools/v18/build_weapons.py` writes beside the GLB:
/// per-part pivots by full node name, the sidearm's muzzle tip, and one
/// muzzle per weapon node, all in engine space. `muzzles` defaults so the
/// v17 sidecar still loads (every weapon then flashes at the sidearm's tip).
#[derive(Deserialize, Default)]
struct ViewmodelRig {
    #[serde(default)]
    pivots: HashMap<String, [f32; 3]>,
    #[serde(default)]
    muzzle: Option<[f32; 3]>,
    #[serde(default)]
    muzzles: HashMap<String, [f32; 3]>,
}

const VIEWMODEL_GLB: &[u8] = include_bytes!("../assets/viewmodel.glb");
const VIEWMODEL_RIG: &str = include_str!("../assets/viewmodel-rig.json");

/// How far along the muzzle axis the flash sat on the old box pistol; the
/// fallback when the sidecar names no muzzle.
const LEGACY_MUZZLE: Vec3 = Vec3::new(0.95, 0.0, 0.0);

/// The middle of the rocket along the bore, in the launcher's frame. The
/// asset build report (`tools/v18/build_weapons.py`, the `w_rpg7_rocket`
/// box line) seats the rocket at x -0.266..0.538: 0.43 m of it inside the
/// tube behind the muzzle at x 0.166 and the warhead 0.37 m out of it, so
/// its centre is 0.136 ahead of the grip. An in-flight rocket is drawn
/// with this point on the server's path, at the bore height the sidecar
/// gives the launcher's muzzle.
const ROCKET_CENTRE_X: f32 = 0.136;

/// Load the GLB into engine meshes + part lists. Falls back to the classic
/// cube pistol when the asset is missing/broken.
pub fn load_assets() -> (Vec<ember_engine::MeshData>, Option<Assets>) {
    match ember_engine::assets::load_glb(VIEWMODEL_GLB) {
        Ok(parts) => {
            let rig: ViewmodelRig = serde_json::from_str(VIEWMODEL_RIG).unwrap_or_else(|e| {
                tracing::warn!("viewmodel sidecar unusable ({e}); parts ride rigidly");
                ViewmodelRig::default()
            });
            let mut meshes = Vec::new();
            let sidearm_muzzle = rig.muzzle.map_or(LEGACY_MUZZLE, Vec3::from_array);
            let mut assets = Assets {
                muzzles: [sidearm_muzzle; WEAPON_SLOTS],
                ..Assets::default()
            };
            // A weapon's muzzle is keyed by its node name; the revolver's
            // is on its frame part. A weapon the sidecar does not name
            // keeps the sidearm's tip, which is at least on the gun.
            for (name, m) in &rig.muzzles {
                if let Slot::Weapon(id) = classify(name) {
                    assets.muzzles[slot(id)] = Vec3::from_array(*m);
                }
            }
            for p in parts {
                let pivot = rig.pivots.get(&p.name).copied().map(Vec3::from_array);
                // A moving part without a pivot would spin about the model
                // origin, which is the hold point on the grip: worse than
                // not moving. So no pivot, no animation.
                let anim = if pivot.is_some() {
                    anim_of(&p.name)
                } else {
                    PartAnim::Fixed
                };
                let part = Part {
                    mesh: u32::try_from(meshes.len()).expect("viewmodel mesh count fits in u32")
                        + 1, // 0 is the built-in cube
                    color: Vec3::from_array(p.color),
                    is_strip: p.name == "strip",
                    anim,
                    pivot: pivot.unwrap_or(Vec3::ZERO),
                };
                meshes.push(p.mesh);
                match classify(&p.name) {
                    Slot::Shield => assets.shield.push(part),
                    Slot::Sword => assets.sword.push(part),
                    Slot::Fist => assets.fist.push(part),
                    Slot::Arms => assets.arms.push(part),
                    Slot::Rocket => assets.rocket.push(part),
                    Slot::Weapon(id) => assets.weapons[slot(id)].push(part),
                }
            }
            let missing: Vec<&str> = (1..=WEAPON_COUNT)
                .filter(|&id| assets.weapons[slot(id)].is_empty())
                .map(weapon_name)
                .collect();
            tracing::info!(
                weapon_parts = ?assets.weapons.iter().map(Vec::len).collect::<Vec<_>>(),
                rocket_parts = assets.rocket.len(),
                arm_parts = assets.arms.len(),
                shield_parts = assets.shield.len(),
                sword_parts = assets.sword.len(),
                fist_parts = assets.fist.len(),
                fallback_to_sidearm = ?missing,
                "viewmodel glb loaded"
            );
            (meshes, Some(assets))
        }
        Err(e) => {
            tracing::warn!("viewmodel glb unusable ({e}); using cube fallback");
            (Vec::new(), None)
        }
    }
}

/// The box-body player's armour picture, embedded so the wasm build ships it
/// (arena v8). The basalt floor and wall that shipped beside it went with
/// arena v13: the floor is cobble and the boundary the city wall, both in
/// `props`, and two unused 2 MB pictures in the bundle were the whole cost.
const TEX_ARMOR: &[u8] = include_bytes!("../../../assets/textures/player_armor.png");

/// Textured environment meshes, registered after the viewmodel GLB parts:
/// `env_base` + 0 = armor box (the box-body player fallback).
pub fn env_meshes() -> Vec<ember_engine::MeshData> {
    vec![ember_engine::MeshData::textured_box(
        1.0,
        tex(TEX_ARMOR, "player_armor"),
    )]
}

/// Articulated character part meshes (decimated GLBs, ~0.1MB each) — the
/// wasm build ships them embedded.
const PART_GLBS: [(&[u8], f32); 5] = [
    (
        include_bytes!("../../../assets/models/parts/part-head.glb"),
        0.34,
    ),
    (
        include_bytes!("../../../assets/models/parts/part-torso.glb"),
        0.68,
    ),
    (
        include_bytes!("../../../assets/models/parts/part-arm.glb"),
        0.66,
    ),
    (
        include_bytes!("../../../assets/models/parts/part-leg.glb"),
        0.55,
    ),
    (
        include_bytes!("../../../assets/models/parts/part-boot.glb"),
        0.19,
    ),
];

/// Fixed camera from `EMBER_CAM` ("ex,ey,ez,tx,ty,tz"): an overview of the
/// arena for reviewing level and character work in screenshots. Parsed
/// once; None when unset or malformed (and always None on the web).
///
/// The fourth of this native debug set is `EMBER_SCRIPT` (see
/// [`crate::script`]), which drives the client instead of a person: while it
/// is set the client reads no key, no mouse and no pad, and never grabs the
/// cursor, so a capture leaves the operator's machine alone.
fn debug_camera() -> Option<Camera> {
    static CAM: std::sync::OnceLock<Option<Camera>> = std::sync::OnceLock::new();
    *CAM.get_or_init(|| {
        let v: Vec<f32> = std::env::var("EMBER_CAM")
            .ok()?
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        (v.len() == 6).then(|| Camera {
            eye: Vec3::new(v[0], v[1], v[2]),
            target: Vec3::new(v[3], v[4], v[5]),
            fov_y_deg: 65.0,
        })
    })
}

/// The weapon id every player is DRAWN with, from `EMBER_WEAPON` ("1".."7"):
/// a review aid for the capture harness (`tools/v18/capture.ps1`), since the
/// only way to a looted gun in play is a random roll. Cosmetic like
/// `EMBER_CAM`: the sim's weapon, ammo and cooldown are untouched, so a
/// shot fired under it still leaves the sidearm's magazine. Parsed once;
/// None when unset or malformed (and always None on the web). The third
/// of the set is `EMBER_ROUNDS=1` (`debug_rounds`), which hangs every v20
/// shape (the five rounds, a streak, two holes, a flash star) half a metre
/// in front of the eye so they can be judged in one frame: a round crosses
/// the view in one frame at its real speed, so no capture of play shows
/// one.
fn debug_weapon() -> Option<u8> {
    static WEAPON: std::sync::OnceLock<Option<u8>> = std::sync::OnceLock::new();
    *WEAPON.get_or_init(|| {
        std::env::var("EMBER_WEAPON")
            .ok()?
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|w| (1..=WEAPON_COUNT).contains(w))
    })
}

/// The weapon id to draw for a player who holds `id`: the debug override
/// when one is set, else the real one.
fn shown_weapon(id: u8) -> u8 {
    debug_weapon().unwrap_or(id)
}

/// Whether `EMBER_ROUNDS` is "1": the round showcase (`push_showcase`) is
/// drawn every frame the local player is alive and not through the scope
/// (the scope mask would black out all of it but the circle). Read once,
/// native only, like `EMBER_WEAPON`.
#[cfg(not(target_arch = "wasm32"))]
fn debug_rounds() -> bool {
    static ROUNDS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ROUNDS.get_or_init(|| std::env::var("EMBER_ROUNDS").is_ok_and(|v| v.trim() == "1"))
}

/// The artist-made SWAT operator, split one mesh per rig joint by
/// `tools/swat_split.py` and embedded so the wasm build ships it too.
const SWAT_GLB: &[u8] = include_bytes!("../../../assets/models/swat-parts.glb");
const SWAT_RIG: &str = include_str!("../../../assets/models/swat-rig.json");

/// Build the player character, registered starting at `first_mesh`: the
/// SWAT operator when it loads, else the five AI-generated parts.
pub fn part_meshes(
    first_mesh: u32,
) -> (
    Vec<ember_engine::MeshData>,
    Option<ember_engine::rig::RigCharacter>,
) {
    match ember_engine::rig::skinned_from_glb(SWAT_GLB, SWAT_RIG, first_mesh) {
        Ok((meshes, rc)) => {
            tracing::info!("swat operator loaded ({} parts)", rc.parts.len());
            return (meshes, Some(rc));
        }
        Err(e) => tracing::warn!("swat operator unusable ({e}); AI-generated parts"),
    }
    let mut meshes = Vec::new();
    let mut sources = Vec::new();
    for (i, (bytes, _target_h)) in PART_GLBS.iter().enumerate() {
        let mesh_offset = u32::try_from(i).expect("character part count fits in u32");
        match ember_engine::rig::source_from_glb_bytes(bytes, first_mesh + mesh_offset) {
            Ok((mesh, mut src)) => {
                // The embedded v1 parts are single-view concepts facing the
                // camera; the rig flips them to its +Z forward.
                src.flipped = true;
                meshes.push(mesh);
                sources.push(src);
            }
            Err(e) => {
                tracing::warn!("character part {i} unusable ({e}); box bodies");
                return (Vec::new(), None);
            }
        }
    }
    let rc = ember_engine::rig::veteran_rig(&ember_engine::rig::VeteranSources {
        head: Some(sources[0]),
        torso: Some(sources[1]),
        arm: Some(sources[2]),
        leg: Some(sources[3]),
        boot: Some(sources[4]),
        ..Default::default()
    });
    tracing::info!("jointed rig character assembled ({} parts)", rc.parts.len());
    (meshes, Some(rc))
}

/// The accent colour of a weapon id: the strip on the sidearm, and what a
/// fallback part list is tinted with so a missing node still says which gun
/// it stands for.
const fn weapon_accent(id: u8) -> Vec3 {
    weapon_feel(id).accent
}

/// The native crosshair: how far ahead of the eye it sits, how long each
/// of its two bars is and how thick, metres at hip field of view. The
/// same 1.2 m as the hit markers, so a wall closer than that occludes
/// both the same way. At 1.2 m under a 70 degree field a metre is 0.60
/// screen heights, so 16 mm is ten pixels on a 1080-line screen (six on
/// 600, twenty on 2160) and 3 mm is two (one on 600, four on 2160): a
/// fine "+" at every size. The thickness must be over one pixel: the
/// screen's centre is a pixel boundary on any even height, and a bar
/// thinner than a pixel there straddles two pixel centres without
/// covering either, so the first cut's 1.5 mm (0.66 px at 600 lines,
/// 0.96 at 1080) drew nothing at all in the first capture. The web page
/// has no use for it: its `div#crosshair` is the crosshair there.
#[cfg(not(target_arch = "wasm32"))]
const CROSSHAIR_DIST: f32 = 1.2;
#[cfg(not(target_arch = "wasm32"))]
const CROSSHAIR_LEN: f32 = 0.016;
#[cfg(not(target_arch = "wasm32"))]
const CROSSHAIR_THICK: f32 = 0.003;
#[cfg(not(target_arch = "wasm32"))]
const CROSSHAIR_COLOR: Vec3 = Vec3::ONE;

/// Push the two white hairline bars of a "+" at the centre of the view:
/// one along the camera's right, one along its up, both scaled by
/// `view_scale` like the hit markers so the cross keeps its size on
/// screen as the field narrows. The up is the camera's own (the right
/// crossed with the look), the same basis the hit markers use, so neither
/// overlay foreshortens as the aim pitches. Scale applies before rotation:
/// a cube long in X is turned onto the right, one long in Y onto the up.
#[cfg(not(target_arch = "wasm32"))]
fn push_crosshair(frame: &mut Frame, eye: Vec3, look: Vec3, right: Vec3, view_scale: f32) {
    let up = right.cross(look).normalize();
    let center = eye + look * CROSSHAIR_DIST;
    let len = CROSSHAIR_LEN * view_scale;
    let thick = CROSSHAIR_THICK * view_scale;
    frame.instances.push(
        Instance::new(center, Vec3::new(len, thick, thick), CROSSHAIR_COLOR)
            .with_rot(Quat::from_rotation_arc(Vec3::X, right)),
    );
    frame.instances.push(
        Instance::new(center, Vec3::new(thick, len, thick), CROSSHAIR_COLOR)
            .with_rot(Quat::from_rotation_arc(Vec3::Y, up)),
    );
}

/// The mechanical state of the weapon, for the parts that move.
///
/// `cycle` is progress through the current shot's cooldown, 0 at the
/// server-confirmed shot and 1 once settled; `shots` counts confirmed shots
/// so the cylinder's index accumulates instead of snapping back. A remote
/// player, whose shots this client does not time, is drawn at rest.
#[derive(Clone, Copy)]
struct Action {
    cycle: f32,
    shots: u32,
}

impl Action {
    const REST: Self = Self {
        cycle: 1.0,
        shots: 0,
    };

    /// Rotation of a part about its pivot, in model space.
    ///
    /// Model axes: +X forward, +Y up, +Z right. The cylinder turns about the
    /// barrel; the hammer and trigger swing about the side-to-side axis, the
    /// same one `weapon_rot` pitches the whole gun about.
    fn local_rot(self, anim: PartAnim) -> Quat {
        const CHAMBER: f32 = std::f32::consts::TAU / 6.0;
        let c = self.cycle.clamp(0.0, 1.0);
        match anim {
            PartAnim::Fixed => Quat::IDENTITY,
            // One chamber per shot, advanced over the first 60% of the
            // cooldown with an ease-out, indexed by shots so it never runs
            // backwards between rounds.
            PartAnim::Cylinder => {
                let k = (c / 0.6).min(1.0);
                let ease = 1.0 - (1.0 - k) * (1.0 - k);
                let chamber = u8::try_from(self.shots % 6).expect("a value mod 6 fits in a u8");
                let turns = f32::from(chamber) + ease - 1.0;
                Quat::from_rotation_x(-CHAMBER * turns)
            }
            // Double action: the spur travels back through the first 55%
            // of the cooldown and drops for the round that follows.
            PartAnim::Hammer => {
                let cocked = if c < 0.55 { c / 0.55 } else { (1.0 - c) / 0.45 };
                Quat::from_rotation_z(0.6 * cocked)
            }
            // Pulled at the shot, released over the first quarter.
            PartAnim::Trigger => {
                let pulled = 1.0 - (c / 0.25).min(1.0);
                Quat::from_rotation_z(-0.45 * pulled)
            }
        }
    }
}

fn push_parts(
    frame: &mut Frame,
    parts: &[Part],
    pos: Vec3,
    rot: Quat,
    accent: Vec3,
    action: Action,
) {
    for p in parts {
        let color = if p.is_strip { accent } else { p.color };
        let local = action.local_rot(p.anim);
        // Rotating a part about its pivot: v' = local * (v - pivot) + pivot,
        // so the instance rotates by local and shifts by (pivot - local *
        // pivot), all before the weapon's own placement.
        let shift = if p.anim == PartAnim::Fixed {
            Vec3::ZERO
        } else {
            p.pivot - local * p.pivot
        };
        frame.instances.push(
            Instance::new(pos + rot * shift, Vec3::ONE, color)
                .with_rot(rot * local)
                .with_mesh(p.mesh),
        );
    }
}

/// Draw weapon `id` at one transform: its own parts, or the sidearm's in
/// its accent when it has none, plus the rocket riding the RPG's tube while
/// `loaded` (a round in the magazine and no reload in progress: the tube is
/// visibly empty between shots, first and third person both).
fn push_weapon(
    frame: &mut Frame,
    assets: &Assets,
    id: u8,
    pos: Vec3,
    rot: Quat,
    action: Action,
    loaded: bool,
) {
    let (parts, own) = assets.weapon_parts(id);
    push_parts(frame, parts, pos, rot, weapon_accent(id), action);
    if own && weapon_stats(id).kind == Projectile::Rocket && loaded {
        push_parts(
            frame,
            &assets.rocket,
            pos,
            rot,
            weapon_accent(id),
            Action::REST,
        );
    }
}

/// Rotation for a weapon held at `yaw` and looking up/down by `pitch`.
/// The model convention is +X forward / +Y up, and +Z is the right-hand
/// axis that lifts +X toward +Y, so elevation is a Z rotation applied in
/// model space before the world yaw.
fn weapon_rot(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_z(pitch)
}

/// A melee keyframe: seconds since the press, then an offset (forward,
/// left, up) in metres and a (yaw, pitch, roll) in radians, all in the
/// weapon's own frame. Linear between keys, nothing before the first or
/// after the last, and the last always sits on `MELEE_COOLDOWN` so the
/// hold is back before the next swing can start. Local only: the protocol
/// carries no melee state for remote players.
type MeleeKey = (f32, f32, f32, f32, f32, f32, f32);

/// Where the rifle goes while the sword is out: down and to the right,
/// clear of the frame. It has to leave, not just dip - the operator's right
/// fist is on the sword, and a rifle still in shot shows that hand twice.
const LOWER: [MeleeKey; 5] = [
    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
    (0.14, -0.10, -0.30, -0.55, -0.30, -0.70, 0.0),
    (0.36, -0.14, -0.34, -0.62, -0.34, -0.75, 0.0),
    (0.55, -0.10, -0.30, -0.55, -0.30, -0.70, 0.0),
    (MELEE_COOLDOWN, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
];

/// The cut: the Murasama swings in from the lower right already raised,
/// travels diagonally across to the lower left with the edge leading (the
/// roll turns the edge into the direction of travel), follows through, and
/// drops back out of frame. Positive `left` is toward the left of the
/// screen, so the sign of that column is the direction of the cut.
const SLASH: [MeleeKey; 6] = [
    (0.0, 0.50, -0.34, -0.44, -0.70, 0.80, 0.0),
    // Raised over the right shoulder: hilt low right, blade up and out of
    // the frame, the way a sword is actually carried into a cut.
    (0.14, 0.54, -0.26, -0.10, -0.55, 0.70, -0.10),
    // The cut. The yaw is what makes this readable: a blade pointed away
    // from the eye shows 1 cm of edge and draws as a red line, so the cut
    // brings it round to 66 degrees off the look, broadside across the
    // upper left, where its 4 cm of width faces the camera. The hilt
    // barely moves - a cut pivots at the shoulder.
    (0.30, 0.56, -0.08, -0.10, 1.15, 0.25, -0.45),
    // Follow-through: further left and dropping.
    (0.44, 0.52, 0.16, -0.26, 1.55, -0.30, -0.55),
    // Recovering, dropping out of frame.
    (0.64, 0.46, -0.16, -0.42, -0.20, 0.50, -0.20),
    (MELEE_COOLDOWN, 0.50, -0.34, -0.52, -0.70, 0.80, 0.0),
];

/// Read a keyframe table `since` seconds after the press: the offset and
/// the (yaw, pitch, roll) to add to the hold. `None` outside the swing.
fn melee_pose(keys: &[MeleeKey], since: f32) -> Option<(Vec3, f32, f32, f32)> {
    if !(0.0..MELEE_COOLDOWN).contains(&since) {
        return None;
    }
    let idx = keys.iter().rposition(|key| key.0 <= since)?;
    let from = keys[idx];
    let to = keys[(idx + 1).min(keys.len() - 1)];
    let span = to.0 - from.0;
    let blend = if span > 0.0 {
        ((since - from.0) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let lerp = |x: f32, y: f32| x + (y - x) * blend;
    Some((
        Vec3::new(lerp(from.1, to.1), lerp(from.2, to.2), lerp(from.3, to.3)),
        lerp(from.4, to.4),
        lerp(from.5, to.5),
        lerp(from.6, to.6),
    ))
}

/// Mouse-look sensitivity, radians per pixel.
const LOOK_SENS: f32 = 0.0026;

#[derive(Deserialize, Clone, Debug)]
pub struct OnlineConfig {
    pub url: String,
    /// "create" or "join"
    pub action: String,
    pub lobby: String,
    #[serde(default)]
    pub password: Option<String>,
    pub handle: String,
    /// The level a `create` asks for, by name (`MAP_FREIGHT_YARD` or
    /// `MAP_TRENCH_CITY`); empty is the server's default. Ignored on a
    /// `join`, where the lobby already has one.
    #[serde(default)]
    pub map: String,
    /// The `GameMode` a `create` asks for, by name (`GameMode::name`);
    /// empty is free for all. Ignored on a `join`, like `map`.
    #[serde(default)]
    pub mode: String,
}

impl OnlineConfig {
    fn opening_msgs(&self) -> Result<Vec<C2S>, String> {
        let action = match self.action.as_str() {
            "create" => C2S::CreateLobby {
                name: self.lobby.clone(),
                password: self.password.clone().filter(|p| !p.is_empty()),
                map: self.map.clone(),
                mode: self.mode.clone(),
            },
            "join" => C2S::JoinLobby {
                name: self.lobby.clone(),
                password: self.password.clone().filter(|p| !p.is_empty()),
            },
            other => return Err(format!("unknown action \"{other}\"")),
        };
        Ok(vec![
            C2S::Hello {
                proto: PROTO_VERSION,
                handle: self.handle.clone(),
            },
            action,
        ])
    }
}

/// Tab scoreboard: a monospace overlay on the web page (safe text-only
/// rendering), nothing on native (the status log already carries scores).
#[cfg(target_arch = "wasm32")]
fn set_scoreboard(text: Option<&str>) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("scoreboard"))
    {
        match text {
            Some(t) => {
                el.set_text_content(Some(t));
                drop(el.remove_attribute("hidden"));
            }
            None => {
                drop(el.set_attribute("hidden", ""));
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn set_scoreboard(_text: Option<&str>) {}

/// Show progress where the player can see it: the page's #status element on
/// the web, the log on native.
fn set_status(text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("status"))
        {
            el.set_text_content(Some(text));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    tracing::info!(status = %text);
}

// ---- the sidearm, in cubes (palette from the concept art) ----

const GUNMETAL: Vec3 = Vec3::new(0.16, 0.17, 0.20);
const GUNMETAL_DARK: Vec3 = Vec3::new(0.11, 0.11, 0.13);
const BRONZE: Vec3 = Vec3::new(0.46, 0.32, 0.21);

/// The off-hand shield plate: thin along the direction it faces, taller than
/// it is wide. Model space is +X forward, matching the pistol, so the same
/// yaw that points a weapon downrange turns this plate's face there too.
const SHIELD_PLATE: Vec3 = Vec3::new(0.09, 0.54, 0.46);

/// Draw a raised shield: a plate facing `yaw`, with a brighter boss just
/// proud of its face so it reads as an object rather than a flat rectangle.
/// Opaque, because the scene pass is `BlendState::REPLACE` and there is no
/// transparent anything to be had — a shield you can see through is not an
/// option here, it is a different renderer.
fn push_shield(frame: &mut Frame, center: Vec3, rot: Quat, plate: Vec3, color: Vec3) {
    frame
        .instances
        .push(Instance::new(center, plate, color).with_rot(rot));
    // The boss sits forward along the plate's own +X, so it moves with the
    // rotation instead of needing its own trigonometry.
    frame.instances.push(
        Instance::new(
            center + rot * Vec3::new(plate.x * 0.6, 0.0, 0.0),
            Vec3::new(plate.x * 0.7, plate.y * 0.30, plate.z * 0.30),
            color * 0.45 + GLOW_BLUE * 0.35,
        )
        .with_rot(rot),
    );
}

/// The cube-fallback pistol's barrel-tip box, in the gun's own frame:
/// where its centre sits and how big it is. `push_gun` draws it and
/// `BOX_MUZZLE` reads its front face, so a flash hung on the fallback gun
/// is on the end of the barrel that is actually on screen.
const BOX_TIP_AT: Vec3 = Vec3::new(0.76, 0.09, 0.0);
const BOX_TIP_SIZE: Vec3 = Vec3::new(0.16, 0.13, 0.13);

/// The tip of the cube pistol's barrel: the front face of that box. Not
/// `LEGACY_MUZZLE`, which is 0.11 m past the end of it — that number is
/// where the old first-person flash hung, and nobody sees the fallback
/// gun's own tip from behind their own eye.
const BOX_MUZZLE: Vec3 = Vec3::new(BOX_TIP_AT.x + BOX_TIP_SIZE.x * 0.5, BOX_TIP_AT.y, 0.0);

/// Cube-fallback pistol: held at `hand`, pointing along `aim`.
/// Local space is +X forward; each part is rotated by the shared yaw.
fn push_gun(frame: &mut Frame, hand: Vec3, aim: Vec2, accent: Vec3) {
    let yaw = -aim.y.atan2(aim.x);
    let rot = |p: Vec3| -> Vec3 {
        let (s, c) = yaw.sin_cos();
        Vec3::new(p.x * c + p.z * s, p.y, -p.x * s + p.z * c)
    };
    let mut part = |offset: Vec3, scale: Vec3, color: Vec3| {
        frame
            .instances
            .push(Instance::new(hand + rot(offset), scale, color).with_yaw(yaw));
    };
    // Slide (dark gunmetal), barrel tip (bronze), glow strip (blue),
    // trigger guard (bronze), grip (near-black).
    part(
        Vec3::new(0.34, 0.12, 0.0),
        Vec3::new(0.74, 0.17, 0.15),
        GUNMETAL,
    );
    part(BOX_TIP_AT, BOX_TIP_SIZE, BRONZE);
    part(
        Vec3::new(0.32, 0.03, 0.0),
        Vec3::new(0.58, 0.045, 0.17),
        accent,
    );
    part(
        Vec3::new(0.18, -0.06, 0.0),
        Vec3::new(0.14, 0.06, 0.11),
        BRONZE,
    );
    part(
        Vec3::new(0.02, -0.14, 0.0),
        Vec3::new(0.15, 0.26, 0.13),
        GUNMETAL_DARK,
    );
}

#[derive(Clone, Copy, Default)]
struct PSnap {
    x: f32,
    z: f32,
    /// Feet height, so remote players stand on crates and jump visibly.
    y: f32,
}

/// One sent input command, kept until the server acks it — the base of
/// client-side movement prediction.
struct Cmd {
    seq: u32,
    mv: [f32; 2],
    speed: f32,
    jump: bool,
    sent_at: f32,
}

/// One short-lived opaque ball (`rounds::puff`, drawn at `feel::PUFF_BALL`
/// of `size`): sparks on a hit, shards and smoke from a blast, the launch
/// smoke behind a rocket. `life` is the ttl it started with, so a size or
/// colour can fade against it.
#[derive(Clone, Copy)]
struct Fx {
    pos: Vec3,
    vel: Vec3,
    ttl: f32,
    life: f32,
    size: f32,
    color: Vec3,
    /// Downward acceleration; 0 for smoke.
    gravity: f32,
    /// The time it comes alive (`self.time` plus `feel::Puff::delay` at the
    /// spawn): until then it does not move, does not age and is not drawn.
    ///
    /// Absolute, not a per-frame countdown, so that it is the same clock
    /// `Flash::until` is read against. A countdown was a frame out on the
    /// remote path, where a shot is spawned in the inbox drain and so had
    /// that frame's own `dt` taken off it before the flash's clock had
    /// started: the smoke arrived one frame before the star went out and
    /// sat on it.
    born: f32,
}

impl Fx {
    /// A spawned `Puff` as a frame particle: its delay is relative to the
    /// spawn, this is the wall time it becomes visible.
    fn spawn(p: Puff, now: f32) -> Self {
        Self {
            pos: p.pos,
            vel: p.vel,
            ttl: p.ttl,
            life: p.ttl,
            size: p.size,
            color: p.color,
            gravity: p.gravity,
            born: now + p.delay,
        }
    }
}

/// A remote muzzle flash (v20): a star at a shooter's muzzle from the
/// `Shot` event, along the round's direction, gone at `until`. Remote
/// flashes used to wait for a state that happened to carry the round; at
/// real speeds most never did.
#[derive(Clone, Copy)]
struct Flash {
    pos: Vec3,
    /// The bore: the shot's direction, which the star's forward cone
    /// follows and its four petals stand square to.
    dir: Vec3,
    size: f32,
    until: f32,
}

/// The muzzle flash colour: a hot yellow-white, the same for every gun.
const FLASH_COLOR: Vec3 = Vec3::new(1.0, 0.9, 0.5);
/// The star's cones as fractions of the weapon's `flash` size: every
/// cone's base radius, the four petals' length, the forward cone's length.
const FLASH_BASE: f32 = 0.22;
const FLASH_PETAL: f32 = 1.0;
const FLASH_FORWARD: f32 = 1.6;

/// How far past the smoke's own outer edge (`feel::plume_reach()`, the
/// spawn ring plus one ball) a shot's petal tips must stand.
///
/// Not why the star was lost: the plume and the star no longer overlap at
/// all, since every plume puff now waits out its weapon's flash. The star
/// was buried because four opaque cubes of edge 0.10 m sat on the muzzle
/// for a quarter second while it lived 35 to 60 ms — no petal in the table
/// was ever shorter than the ball. What this earns is the frame the
/// handover happens on: a star narrower than the smoke that replaces it
/// reads as a small light swallowed by a big ball, and it is the margin
/// that keeps a future fatter plume from taking the star back. The plume
/// is now a ring with the bore clear through it, so the star also stands
/// in the hole rather than behind the smoke; the floor is unchanged
/// because `plume_reach` is (`feel::PLUME_RING`).
const FLASH_CLEAR: f32 = 1.4;

/// The size of the star at a shot from `row`: the weapon's own `flash`,
/// raised if that is too small to put the petal tips (`FLASH_PETAL` of the
/// size) `FLASH_CLEAR` times the plume's reach out. The whole star is
/// scaled rather than the petals alone, so it keeps its shape.
///
/// The floor is 0.126 m and exactly one row in the table is under it: the
/// Vityaz (id 2, flash 0.10), which is drawn at 0.126. Every other gun,
/// the sidearm's 0.14 included, is already clear and is untouched. The
/// point of taking the floor from `feel::plume_reach` rather than from a
/// bare number is that a fatter plume widens every star that would
/// otherwise be lost in it. `push_showcase` passes its own size and is not
/// touched: nothing smokes in the showcase.
fn shot_flash(row: &feel::WeaponFeel) -> f32 {
    row.flash
        .max(feel::plume_reach() * FLASH_CLEAR / FLASH_PETAL)
}

/// Push a muzzle flash as a star of five streak cones radiating from the
/// muzzle: four petals in the plane square to the bore (up, down, left and
/// right of it), each with its base ring at the muzzle and its point
/// `size` away, and one longer cone forward along the bore, its point
/// `size * FLASH_FORWARD` out. Nothing back along the bore: that would
/// point into the gun. What stood here was one opaque cube of edge `size`,
/// a block on the end of the barrel in every capture; the star is opaque
/// too (the scene pass has no blending), but five thin spikes read as a
/// flash where a cube reads as a box. The plane's basis is the bore
/// crossed with the world up, or with the world Z when the bore is near
/// vertical and that cross would vanish; which way the petals turn round
/// the bore does not show, the four being symmetric. Scale before
/// rotation: the cone is long in +X, turned onto each spike's direction.
fn push_flash(frame: &mut Frame, rs: Rounds, muzzle: Vec3, dir: Vec3, size: f32) {
    let bore = if dir.length_squared() < 1e-6 {
        Vec3::X
    } else {
        dir.normalize()
    };
    let seed = if bore.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let across = bore.cross(seed).normalize();
    let up = across.cross(bore).normalize();
    let base = size * FLASH_BASE;
    let mut cone = |d: Vec3, len: f32| {
        frame.instances.push(
            Instance::new(muzzle, Vec3::new(len, base, base), FLASH_COLOR)
                .with_rot(Quat::from_rotation_arc(Vec3::X, d))
                .with_mesh(rs.streak()),
        );
    };
    for d in [up, -up, across, -across] {
        cone(d, size * FLASH_PETAL);
    }
    cone(bore, size * FLASH_FORWARD);
}

/// A round as it is drawn: which profile, and how many times its real
/// size. In play the scale is `ROUND_SCALE`; the showcase shrinks it with
/// the field of view so its rows keep their size on screen. The streak is
/// measured from the same pair, so the two never disagree about how big
/// the round they belong to is.
#[derive(Clone, Copy)]
struct Drawn {
    round: Round,
    scale: f32,
}

/// Push a round in flight: its mesh at `pos`, nose along `dir`, at its
/// drawn scale, untinted (the jacket picture is the colour). A round is a
/// body of revolution, so `from_rotation_arc` has no roll to get wrong on
/// it.
fn push_round(frame: &mut Frame, rs: Rounds, drawn: Drawn, pos: Vec3, dir: Vec3) {
    frame.instances.push(
        Instance::new(pos, Vec3::splat(drawn.scale), Vec3::ONE)
            .with_rot(Quat::from_rotation_arc(Vec3::X, dir))
            .with_mesh(rs.mesh(drawn.round)),
    );
}

/// Push the streak behind a round whose head is at `head` flying along
/// `dir`: the tracer's rods (`Tracer::rods`, the core from the head back
/// and the tail behind it) as one taper. The core is a frustum whose base
/// (its fat end) sits `STREAK_INSET` inside the round's tail, so the
/// bullet leads it and the two base discs never share a plane, and whose
/// back end is where the core rod ends; the tail is a cone whose base is
/// the frustum's narrow end (`CORE_NECK` of the base) and whose point
/// trails. A core with no tail behind it is the cone, so a streak always
/// ends in a point. The base radius is the round's drawn heel times
/// `STREAK_LEAD` times `fade`, so the streak thins out through the
/// linger. Scale is applied before rotation, so a shape long in +X is
/// turned onto MINUS the flight direction. The streak is measured from
/// the round's own drawn size (`Drawn`), so it thickens and thins with it.
fn push_streak(
    frame: &mut Frame,
    rs: Rounds,
    drawn: Drawn,
    head: Vec3,
    dir: Vec3,
    rods: &[Rod],
    fade: f32,
) {
    let Drawn { round, scale } = drawn;
    let half_len = round.length() * 0.5 * scale;
    let r = round.heel_radius() * scale * rounds::STREAK_LEAD * fade;
    let back = Quat::from_rotation_arc(Vec3::X, -dir);
    for (i, rod) in rods.iter().enumerate() {
        let rear = rod.center - dir * (rod.len * 0.5);
        // The core's front is inside the round; the tail's is where the
        // core ends, at the frustum's narrow end.
        let (front, radius) = if i == 0 {
            let to_base = half_len * (1.0 - rounds::STREAK_INSET);
            (head - dir * to_base, r)
        } else {
            (rod.center + dir * (rod.len * 0.5), r * rounds::CORE_NECK)
        };
        let len = (front - rear).dot(dir);
        if len <= 1e-4 {
            // The head has barely left the round's own length: the round
            // covers it.
            continue;
        }
        // A rod with another behind it wears the frustum; the last one,
        // the cone.
        let last = i + 1 == rods.len();
        let mesh = if last { rs.streak() } else { rs.core() };
        frame.instances.push(
            Instance::new(front, Vec3::new(len, radius, radius), rod.color)
                .with_rot(back)
                .with_mesh(mesh),
        );
    }
}

/// The showcase's layout: how far ahead of the eye it hangs, the gap
/// between rounds in the top row, the drop from one row to the next, the
/// streak's length in the second row and the flash star's size in the
/// third. Every one of them is a length at the hip; the offsets and the
/// mesh scales are multiplied by the tangent ratio of the field to the
/// hip's, at the same fixed distance, so the layout keeps its size on
/// screen as the field narrows down the sights.
#[cfg(not(target_arch = "wasm32"))]
const SHOWCASE_DIST: f32 = 0.5;
#[cfg(not(target_arch = "wasm32"))]
const SHOWCASE_GAP: f32 = 0.02;
#[cfg(not(target_arch = "wasm32"))]
const SHOWCASE_DROP: f32 = 0.06;
#[cfg(not(target_arch = "wasm32"))]
const SHOWCASE_STREAK: f32 = 0.4;
#[cfg(not(target_arch = "wasm32"))]
const SHOWCASE_FLASH: f32 = 0.08;

/// How much the showcase shrinks at `fov_deg`: the tangent of its half
/// angle over the hip's, which is the factor that holds a thing's share
/// of the view when the field narrows and the depth does not change.
#[cfg(not(target_arch = "wasm32"))]
fn showcase_scale(fov_deg: f32) -> f32 {
    let half = |deg: f32| (deg * 0.5).to_radians().tan();
    half(fov_deg.clamp(1.0, 179.0)) / half(feel::HIP_FOV)
}

/// The round showcase (`EMBER_ROUNDS=1`, native only): every v20 shape
/// hung in the eye's own frame so the operator can judge each in one
/// frame, since a round crosses the view in one frame at its real speed
/// and no capture of play has shown one. `SHOWCASE_DIST` ahead of the
/// eye, in the basis the scope mask uses (the camera's right, its up,
/// its look), three rows: the five rounds side by side along the right,
/// noses along the right so the profile is seen side-on, nose to tail
/// with `SHOWCASE_GAP` between them (a fixed 0.11 m pitch would overlap
/// them: the Lapua alone is 0.205 m at `ROUND_SCALE`, so the row is laid
/// by length instead), each at `ROUND_SCALE` in its jacket;
/// `SHOWCASE_DROP` below them the Lapua with a streak `SHOWCASE_STREAK`
/// long behind it exactly as the tracer loop draws one (the core frustum
/// inset in the tail, the tail cone on its neck, the sniper's tracer
/// colour, no fade); and a row below that (the second row is too wide to
/// share; a 4:3 window shows 0.93 m across at this distance) two holes
/// facing the eye (the AK's 24 mm, then the rocket's 0.5 m blast mark
/// scaled to a fifth, 0.1 m, so it fits the row: its shape is the same
/// disc) and one flash star at `SHOWCASE_FLASH`, its bore half toward
/// the eye's right and half down the look so both the petals and the
/// forward cone show. Occludes the crosshair and the gun; it is a review
/// aid, not a view to play in.
///
/// The metres above are the hip's. Every offset and every mesh scale
/// here is multiplied by `k`, the tangent ratio of `fov_deg` to
/// `HIP_FOV` (NOT the crosshair's linear `fov_now / HIP_FOV`, which is
/// close enough for a two-pixel bar and is not close enough for a
/// 0.7 m row: at 44 degrees the tangent has fallen to 0.58 of the hip's
/// where the ratio of the angles is 0.63), at the same fixed
/// `SHOWCASE_DIST`. A lateral offset over its depth divided by the
/// field's tangent is exactly the fraction of the half-view a thing
/// covers, so scaling the offsets and the sizes by that ratio at a fixed
/// depth holds every row where it sits on screen; scaling the distance
/// too would be a similarity about the eye, which holds the ANGLE and so
/// lets the rows swell across a narrowed view. Unscaled, row one ran off
/// a 4:3 window at the narrowest sight (44 degrees). Drawn only while the
/// local player is alive and never through the scope, whose mask would
/// black out all but the circle anyway; see `debug_rounds`.
#[cfg(not(target_arch = "wasm32"))]
fn push_showcase(
    frame: &mut Frame,
    rs: Rounds,
    eye: Vec3,
    look: Vec3,
    right3: Vec3,
    now: f32,
    fov_deg: f32,
) {
    let up = right3.cross(look).normalize();
    let right = look.cross(up);
    // One factor on every offset and every mesh, the depth left alone:
    // at the hip it is 1 and every number below is the metres it reads as.
    let k = showcase_scale(fov_deg);
    let round_scale = rounds::ROUND_SCALE * k;
    let centre = eye + look * SHOWCASE_DIST;
    let gap = SHOWCASE_GAP * k;
    let drop = SHOWCASE_DROP * k;
    // Row one: the rounds, nose to tail along the right, centred.
    let total = Round::ALL
        .iter()
        .map(|r| r.length() * round_scale + gap)
        .sum::<f32>()
        - gap;
    let mut x = -total * 0.5;
    for r in Round::ALL {
        let len = r.length() * round_scale;
        let drawn = Drawn {
            round: r,
            scale: round_scale,
        };
        push_round(frame, rs, drawn, centre + right * (x + len * 0.5), right);
        x += len + gap;
    }
    // Row two: the Lapua flying along the right with its streak behind
    // it, the core `TRACER_CORE_LEN` of `TRACER_TAIL_LEN` of the length
    // (the proportion at which the two are one straight taper), the
    // core rod measured from the head as `Tracer::rods` measures it.
    let round = Round::Lapua;
    let head = centre - up * drop + right * (0.30 * k);
    let inset = round.length() * 0.5 * round_scale * (1.0 - rounds::STREAK_INSET);
    let streak = SHOWCASE_STREAK * k;
    let core = inset + streak * (feel::TRACER_CORE_LEN / feel::TRACER_TAIL_LEN);
    let tail = inset + streak - core;
    let color = weapon_feel(feel::SCOPED_WEAPON).tracer;
    let rods = [
        Rod {
            center: head - right * (core * 0.5),
            len: core,
            color,
        },
        Rod {
            center: head - right * (core + tail * 0.5),
            len: tail,
            color: color * feel::TRACER_TAIL_DIM,
        },
    ];
    let drawn = Drawn {
        round,
        scale: round_scale,
    };
    push_streak(frame, rs, drawn, head, right, &rods, 1.0);
    push_round(frame, rs, drawn, head, right);
    // Row three: the holes, facing the eye, and the star.
    let row = centre - up * (drop * 2.0 + 0.04 * k);
    let hole = |weapon: u8, at: Vec3, shrink: f32| {
        let m = Mark {
            pos: at,
            normal: -look,
            weapon,
            born: now,
        };
        let (pos, scale, rot) = m.placement();
        Instance::new(
            pos,
            Vec3::new(scale.x * k, scale.y * shrink * k, scale.z * shrink * k),
            feel::MARK_COLOR,
        )
        .with_rot(rot)
        .with_mesh(rs.disc())
    };
    frame.instances.push(hole(3, row - right * (0.30 * k), 1.0));
    frame
        .instances
        .push(hole(7, row - right * (0.15 * k), 0.1 / feel::ROCKET_MARK));
    let bore = (right + look).normalize();
    push_flash(
        frame,
        rs,
        row + right * (0.10 * k),
        bore,
        SHOWCASE_FLASH * k,
    );
}

/// A brass casing out of my own gun (v20): falls under gravity from the
/// ejection port, stops where it lands, and is gone at `CASING_SECS`.
#[derive(Clone, Copy)]
struct Casing {
    pos: Vec3,
    vel: Vec3,
    /// The height it lands on: my feet, which is the floor or the box I
    /// stand on.
    land_y: f32,
    born: f32,
}

/// My own shot's segment, held from the event until the render pass has
/// the viewmodel's muzzle to start the streak from (v20). The sim's origin
/// is the eye height a hand ahead of the eye; the muzzle reads better, and
/// only the render pass knows where it is this frame.
#[derive(Clone, Copy)]
struct PendingShot {
    from: Vec3,
    to: Vec3,
    weapon: u8,
}

/// How long the end point of a body or shield hit is remembered as a point
/// a later segment may start from. A pierce's second segment arrives in
/// the same tick; a reflection's return segment ends a few ticks later.
const CONTINUATION_SECS: f32 = 1.5;

/// How close a segment's start has to be to a remembered end point to be
/// its continuation rather than a new shot.
const CONTINUATION_EPS: f32 = 1e-3;

/// How far down a remote round's line its flash and plume are drawn when
/// there is no drawn gun to hang them on (metres): the shooter has left
/// the state, or is not alive in it, so nothing of that body is on screen
/// and the only honest anchor is the round's own line. When the body IS
/// drawn, `ShooterGame::drawn_muzzle` gives the tip of the weapon actually
/// on screen and this is not used: the sim fires from `EYE_STAND` while
/// the weapon is drawn at the hand, so this number put every remote flash
/// about 0.6 m above the gun that fired it (captured).
const REMOTE_MUZZLE: f32 = 0.75;

/// How far ahead of a remote player's body centre the gun hand reaches,
/// metres. The body box is 1.0 across, so anything held closer than 0.5 is
/// held inside the torso and never shows.
const HAND_REACH: f32 = 0.55;

/// How far a drawn muzzle may stand from the point the sim fired from
/// before it is disowned (metres). A gun on the shooter's own body is
/// about 1.3 m from it at most — 0.6 down from the eye to the hand and
/// the length of the longest weapon forward — so this passes every real
/// pose and refuses a body drawn somewhere else entirely, which is what a
/// missing or stale interpolation snapshot looks like.
const MUZZLE_SANITY: f32 = 2.5;

/// The heights a remote body is drawn at, above its own feet: the body
/// box's full height, the head, the gun hand and the hp pips. Crouching
/// pulls all four down. One table, read by the render pass and by
/// `ShooterGame::drawn_muzzle`, so a flash cannot land somewhere the gun
/// is not.
const fn body_heights(crouch: bool) -> (f32, f32, f32, f32) {
    if crouch {
        (0.75, 0.95, 0.62, 1.5)
    } else {
        (1.1, 1.35, 0.85, 2.0)
    }
}

/// Where a remote player's gun hand is drawn: `hand_y` above its feet and
/// `HAND_REACH` forward along its aim, which is where the body faces.
fn hand_at(pos: Vec2, feet_y: f32, aim: Vec2, hand_y: f32) -> Vec3 {
    Vec3::new(pos.x, feet_y + hand_y, pos.y) + Vec3::new(aim.x, 0.0, aim.y) * HAND_REACH
}

/// Where the listener is and which way is right, for the spatial cues: my
/// eye at the top of the frame, before the look moves it, which is a frame
/// of lag on a pan and nothing on a delay.
#[derive(Clone, Copy)]
struct Ear {
    at: Vec3,
    right: Vec3,
}

/// A weapon rising out of a bonked block: which block, which gun, when.
#[derive(Clone, Copy)]
struct Pop {
    slot: usize,
    weapon: u8,
    started: f32,
}

/// How long a pop takes to rise and vanish, and a bump to play out.
const POP_SECS: f32 = 0.5;
const BUMP_SECS: f32 = 0.25;

/// How long after a predicted bonk the same block stays silent. Longer
/// than the slice of history a reconciliation replays (0.3 s per command),
/// which is how far a rewind can put the head back under the block, and
/// shorter than a block's own 18 s cooldown, so the only bonk it can eat
/// is the dead click of a bunny-hop straight back into a block that has
/// already paid.
const BONK_DEBOUNCE: f32 = 0.5;

/// Whether a bonk at `now` is a new one for a block last bonked at `last`.
fn bonk_is_new(last: Option<f32>, now: f32) -> bool {
    last.is_none_or(|t| now - t >= BONK_DEBOUNCE)
}

/// When a body last sounded a footstep: one clock per remote id and one
/// for me. Two fields of one idea, kept together so the game's own
/// constructor does not grow a line per boot.
#[derive(Default)]
struct StepClocks {
    body: HashMap<u8, f32>,
    own: Option<f32>,
}

// These flags represent independent input, connection, animation, and UI state transitions.
#[allow(clippy::struct_excessive_bools)]
pub struct ShooterGame {
    chan: net::NetChan,
    my_id: Option<u8>,
    /// The rules this lobby plays, from `GameJoined.mode`; free for all
    /// until joined. Decides the status line, the scoreboard's shape,
    /// whether bodies wear team colours and whether the hill is drawn.
    mode: GameMode,
    // ---- v19: the match, from `State` ----
    /// Frag totals per team, `[0, 0]` outside team deathmatch.
    team_score: [u32; 2],
    /// `State.hill`: `HILL_FREE`, `HILL_CONTESTED` or the king's id.
    hill_holder: u8,
    /// Seconds left in the pause after a round, 0 while a round runs.
    round_pause: f32,
    /// The level's hill, kept from `GameJoined` so it can be drawn; only
    /// drawn in `GameMode::Hill`.
    hill: Option<Hill>,
    /// The announcement the last `RoundOver` made, shown beside the pause
    /// countdown until the next round starts, so a player who missed the
    /// one-frame status event still learns who won.
    round_line: Option<String>,
    arena_half: f32,
    obstacles: Vec<Obstacle>,
    metas: HashMap<u8, PlayerMeta>,
    from: HashMap<u8, PSnap>,
    to: HashMap<u8, PSnap>,
    t: f32,
    latest: HashMap<u8, PState>,
    bullets: Vec<BState>,
    bullets_age: f32,
    /// Client-side movement prediction: instant response locally, rebased
    /// on the server's authoritative position + unacked-command replay.
    pred_pos: Vec2,
    /// Predicted feet height and vertical speed (jump physics).
    pred_y: f32,
    pred_vy: f32,
    own_render: Vec2,
    /// Smoothed eye height. `pred_y` is the simulation state; this is the
    /// only thing the camera is allowed to read, so a reconciliation nudge
    /// arrives as a glide rather than as a jolt.
    render_y_own: f32,
    /// Space last frame, and a press held until the next send frame. Sampling
    /// the held key at 20 Hz lost any tap shorter than the send interval.
    prev_space: bool,
    /// Rising-edge latch for the melee key, and whether a swing happened
    /// anywhere in the current send window. Same shape as the jump pair
    /// beside it, for the same reason: at a 20 Hz send rate, sampling a
    /// held key drops taps shorter than 50 ms outright.
    prev_e: bool,
    melee_pending: bool,
    jump_pending: bool,
    /// The same press the server will get, held until prediction spends it.
    /// Predicting on the raw frame edge instead let the local view and the
    /// server disagree about whether a press near a landing happened at all.
    /// Note what this is NOT: jump buffering. The spend test is "vy rose",
    /// and a landing raises vy to zero, so a press made in the air is eaten
    /// by the touchdown rather than carried across it - which matches the
    /// server, since it consumes the flag after one tick and buffers nothing.
    pred_jump: bool,
    /// When the last state was drained, and a smoothed estimate of the gap
    /// between states. Interpolating on the nominal 33.3 ms froze remotes on
    /// every long gap and ran them fast after every short one.
    last_state_at: f32,
    state_interval: f32,
    was_alive: bool,
    history: VecDeque<Cmd>,
    next_seq: u32,
    last_tick: u64,
    audio: Option<Audio>,
    assets: Option<Assets>,
    pads_pos: Vec<[f32; 2]>,
    pads_active: Vec<bool>,
    /// ADS: 0 = hip, 1 = fully zoomed (RMB).
    zoom: f32,
    /// Eased 0..1 raise of MY own shield. Driven by local input rather than
    /// by the acked state, like the bob and the zoom: it is cosmetic, and
    /// the sim is still the only thing that reflects a round.
    shield_raise: f32,
    /// Local time when my current reload started (drives the viewmodel dip).
    reload_started: Option<f32>,
    /// Local time of my last AUTHORITATIVE shot — the server-confirmed
    /// ammo decrement, not the trigger being held. Drives recoil, the
    /// slide cycle and the muzzle flash.
    shot_started: Option<f32>,
    /// When the melee key went down; drives the first-person strike. The
    /// press is what starts it, not a server verdict: the swing is the
    /// player's own motion, the kill is the server's.
    melee_started: Option<f32>,
    /// Confirmed shots this session; indexes the revolver's cylinder so it
    /// advances one chamber per round instead of snapping back.
    shots: u32,
    since_score_ui: f32,
    score_shown: bool,
    aim: Vec2,
    yaw: f32,
    pitch: f32,
    eye_h: f32,
    bob_t: f32,
    time: f32,
    since_input: f32,
    since_ping: f32,
    since_status: f32,
    lost: bool,
    /// The v13 prop set (cover by kind, city, sky, ground) and where it got
    /// registered; None = plain coloured boxes and no city.
    props: Option<Props>,
    /// The round meshes, the streak and the core (v20) and where they got
    /// registered. `run_online` always sets it; None (a frame built by a
    /// test that did not ask) draws no tracer at all, since the box rods
    /// the rounds replaced are the look the operator sent back.
    rounds: Option<Rounds>,
    /// The level's decor list, from `GameJoined`: client-only, but listed
    /// by the level so every client draws the same city.
    decor: Vec<Decor>,
    /// First mesh id of `env_meshes()` (armor); 0 = untextured.
    env_base: u32,
    /// Jointed player character; None = textured/plain boxes.
    rig_character: Option<ember_engine::rig::RigCharacter>,
    /// Per-player (yaw, `walk_phase`, amplitude) + previous render position.
    anim: HashMap<u8, (f32, f32, f32)>,
    prev_pos: HashMap<u8, Vec2>,
    /// The cues the last frame put out, in the order they were played.
    heard: Vec<Play>,
    /// When each body last sounded a step, and when I last sounded mine.
    /// The legs plant far faster than boots do (`feel::WALK_GAP` says how
    /// much), so the cue needs a clock the phase cannot give it.
    steps: StepClocks,
    /// Per-player eased crouch amount (0..1) so the pose sinks smoothly.
    crouch_ease: HashMap<u8, f32>,
    // ---- shot-feel feedback (v9.1) ----
    /// Crosshair hitmarker time remaining.
    hitmarker_t: f32,
    /// Kill-confirm marker time remaining.
    kill_t: f32,
    /// Hitmarker scale: 1, or 1.5 for a head hit.
    hitmarker_scale: f32,
    /// Per-victim white damage-flash time remaining.
    flash: HashMap<u8, f32>,
    /// Sparks, shards and smoke.
    fx: Vec<Fx>,
    // ---- v18: loot blocks ----
    /// Obstacle index of every `Cover::Loot` box, in obstacle order: the
    /// index space `State.loot` and `S2C::Loot.block` are aligned with.
    loot_index: Vec<usize>,
    /// Armed (true) or spent, per block, from the last `State`.
    loot_active: Vec<bool>,
    /// When each block's bump started, if one is playing.
    loot_bump: Vec<Option<f32>>,
    /// Weapons rising out of blocks.
    pops: Vec<Pop>,
    /// The box my head hit last frame, so a bonk is an edge, not a level.
    prev_bonked: Option<usize>,
    /// When each block was last bonked by the prediction, per loot slot.
    /// The edge above is not enough on its own: a reconciliation can
    /// rewind the arc to below the block with the head still rising, the
    /// edge clears on the frame between, and the forward prediction clamps
    /// on the same block a second time.
    last_bonk_at: Vec<Option<f32>>,
    /// When the bonk camera dip started.
    dip_started: Option<f32>,
    /// When my last `S2C::Loot` arrived: a weapon change right after it is
    /// the pop, not a pad pickup.
    last_pop_at: f32,
    // ---- v18: the feel pass ----
    /// Rumble requests since the platform last took them.
    feedback: Feedback,
    climb: Climb,
    shake: Shake,
    /// When the holster drop started (a looted gun ran dry).
    holster_started: Option<f32>,
    /// A rocket left this frame: the render pass spawns the launch smoke at
    /// the muzzle, which only it knows the position of.
    launch_smoke: bool,
    /// Fire last frame, for the dry-trigger edge.
    prev_fire: bool,
    /// L3 last frame, and the sprint it latched.
    prev_l3: bool,
    sprint_latch: bool,
    /// Whether the pad status has been shown on the status line yet.
    pad_status_shown: &'static str,
    // ---- v20: the realism pass ----
    /// Streaks from `S2C::Shot`, drawn until each has flown and faded.
    tracers: Vec<Tracer>,
    /// Impact marks in age order, capped at `feel::MARK_CAP`.
    marks: VecDeque<Mark>,
    /// Remote muzzle flashes from `Shot`.
    flashes: Vec<Flash>,
    /// My own casings in the air and on the ground.
    casings: Vec<Casing>,
    /// My own shots waiting for the render pass to anchor their streaks at
    /// the viewmodel's muzzle.
    pending_shots: Vec<PendingShot>,
    /// Where a round was reflected or pierced through, and when: the
    /// point the next segment of that round starts from. A segment that
    /// starts here is the same round going on, which draws a streak and
    /// an impact but no second flash, plume or gunshot.
    continuations: Vec<(Vec3, f32)>,
    /// A shot of mine was confirmed this frame: the render pass spawns the
    /// plume and the casing at the muzzle, which only it knows.
    own_plume: bool,
    /// My own (yaw, `walk_phase`, amplitude), advanced by the same
    /// `puppet::advance_anim` every remote body's legs are advanced by, off
    /// my predicted movement. My body is the camera and is never posed, but
    /// my footsteps have to land on the same clock a listener next to me
    /// hears from my legs, and the camera bob is a time-based sine that
    /// would put my cadence somewhere else entirely.
    own_anim: (f32, f32, f32),
    /// The scripted-input timeline (`EMBER_SCRIPT`), when this client drives
    /// itself instead of a person. `Some` for the whole run once it is set,
    /// including after the timeline is spent: a scripted client stays
    /// hands-off forever, so the operator's keyboard and mouse are never
    /// read and the cursor is never grabbed. See `crate::script`.
    script: Option<script::Timeline>,
    /// Whether the "script starts"/"script is spent" lines have been logged;
    /// the harness waits on the first (a script's clock starts on its first
    /// frame, not when the window appears) and reads the second to know the
    /// timeline finished rather than timing the run.
    script_began: bool,
    script_spent: bool,
}

impl ShooterGame {
    pub fn connect(cfg: &OnlineConfig, assets: Option<Assets>) -> Result<Self, String> {
        let opening_messages = cfg.opening_msgs()?;
        let chan = net::NetChan::connect(&cfg.url, &opening_messages)?;
        set_status("connecting…");
        Ok(Self::with_chan(chan, assets, Audio::new()))
    }

    /// The game over an already-open channel; `connect` and the tests both
    /// build one here so the field list lives in one place.
    fn with_chan(chan: net::NetChan, assets: Option<Assets>, audio: Option<Audio>) -> Self {
        Self {
            chan,
            my_id: None,
            mode: GameMode::Ffa,
            team_score: [0, 0],
            hill_holder: HILL_FREE,
            round_pause: 0.0,
            hill: None,
            round_line: None,
            arena_half: 24.0,
            obstacles: Vec::new(),
            metas: HashMap::new(),
            from: HashMap::new(),
            to: HashMap::new(),
            t: 1.0,
            latest: HashMap::new(),
            bullets: Vec::new(),
            bullets_age: 0.0,
            pred_pos: Vec2::ZERO,
            pred_y: 0.0,
            pred_vy: 0.0,
            own_render: Vec2::ZERO,
            render_y_own: 0.0,
            prev_space: false,
            prev_e: false,
            melee_pending: false,
            jump_pending: false,
            pred_jump: false,
            last_state_at: 0.0,
            state_interval: 1.0 / 30.0,
            was_alive: false,
            history: VecDeque::new(),
            next_seq: 1,
            last_tick: 0,
            audio,
            assets,
            pads_pos: Vec::new(),
            pads_active: Vec::new(),
            zoom: 0.0,
            shield_raise: 0.0,
            reload_started: None,
            shot_started: None,
            melee_started: None,
            shots: 0,
            since_score_ui: 1.0,
            score_shown: false,
            aim: Vec2::new(1.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            eye_h: EYE_STAND,
            bob_t: 0.0,
            time: 0.0,
            since_input: 0.0,
            since_ping: 0.0,
            since_status: 0.0,
            lost: false,
            props: None,
            rounds: None,
            decor: Vec::new(),
            env_base: 0,
            rig_character: None,
            anim: HashMap::new(),
            heard: Vec::new(),
            steps: StepClocks::default(),
            prev_pos: HashMap::new(),
            crouch_ease: HashMap::new(),
            hitmarker_t: 0.0,
            kill_t: 0.0,
            hitmarker_scale: 1.0,
            flash: HashMap::new(),
            fx: Vec::new(),
            loot_index: Vec::new(),
            loot_active: Vec::new(),
            loot_bump: Vec::new(),
            pops: Vec::new(),
            prev_bonked: None,
            last_bonk_at: Vec::new(),
            dip_started: None,
            last_pop_at: -10.0,
            feedback: Feedback::default(),
            climb: Climb::default(),
            shake: Shake::default(),
            holster_started: None,
            launch_smoke: false,
            prev_fire: false,
            prev_l3: false,
            sprint_latch: false,
            pad_status_shown: "none",
            tracers: Vec::new(),
            marks: VecDeque::new(),
            flashes: Vec::new(),
            casings: Vec::new(),
            pending_shots: Vec::new(),
            continuations: Vec::new(),
            own_plume: false,
            own_anim: (0.0, 0.0, 0.0),
            script: script::from_env(),
            script_began: false,
            script_spent: false,
        }
    }

    /// Apply one event cue: raise the shake, queue the rumble, queue the
    /// sound. The camera's part of an event is a timer set by the caller.
    fn cue(&mut self, c: Cue, sfx: &mut Vec<Play>) {
        self.shake.hit(c.shake);
        if let Some(r) = c.rumble {
            self.feedback.rumble(r.strong, r.weak, r.ms);
        }
        if let Some(s) = c.sfx {
            sfx.push(Play::from_cue(s));
        }
    }

    /// `cue`, with the sound placed at `source` in the world: panned to its
    /// bearing from the ear and late by its distance (v20).
    fn cue_at(&mut self, c: Cue, source: Vec3, ear: Ear, sfx: &mut Vec<Play>) {
        self.shake.hit(c.shake);
        if let Some(r) = c.rumble {
            self.feedback.rumble(r.strong, r.weak, r.ms);
        }
        if let Some((s, v)) = c.sfx {
            sfx.push(Play::spatial(s, v, source, ear.at, ear.right));
        }
    }

    /// Spawn a burst of particles.
    fn puffs(&mut self, puffs: Vec<Puff>) {
        let now = self.time;
        self.fx.extend(puffs.into_iter().map(|p| Fx::spawn(p, now)));
    }

    /// One `S2C::Shot`: a round's segment ended. The streak, the muzzle
    /// flash and plume, the gunshot, the crack, the impact and its mark and
    /// sound, all from this one event (v20).
    ///
    /// A segment that starts where a shield reflected a round or a body was
    /// pierced is the same round going on: it draws its streak and its
    /// impact, but the muzzle it never left gets no flash, no plume and no
    /// gunshot. A rocket's segment draws no streak (the rocket is a mesh
    /// flown from the state) and no impact (the `Blast` beside it is the
    /// impact); it leaves its mark.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn on_shot(
        &mut self,
        owner: u8,
        weapon: u8,
        from: Vec3,
        to: Vec3,
        hit: u8,
        cover: u8,
        normal: [i8; 3],
        ear: Ear,
        sfx: &mut Vec<Play>,
    ) {
        let now = self.time;
        self.continuations
            .retain(|(_, t0)| now - t0 < CONTINUATION_SECS);
        let continuation = self
            .continuations
            .iter()
            .any(|(p, _)| (*p - from).length() < CONTINUATION_EPS);
        let mine = Some(owner) == self.my_id;
        let traces = feel::traces(weapon);
        let stats = weapon_stats(weapon);
        let row = weapon_feel(weapon);
        let dir = (to - from).normalize_or_zero();
        if traces && !continuation {
            if mine {
                self.pending_shots.push(PendingShot { from, to, weapon });
            } else {
                // The remote shot, seen and heard from its muzzle: the
                // flash, the plume, and the gunshot at the distance's
                // variant, late by the distance and panned to it.
                //
                // The sim fires from the shooter's eye (`EYE_STAND` a hand
                // ahead of it) while the client draws that shooter's weapon
                // at the hand, so the event's own origin is about 0.6 m
                // above and behind the drawn barrel: hanging the light
                // there put it in mid air with wall showing between it and
                // the gun. `drawn_muzzle` is the barrel that is on screen.
                // Nothing about the round moves: `from`, `to`, the head,
                // the hit and the impact are the server's, and the muzzle
                // only says where the streak is drawn from.
                //
                // A drawn gun further than `MUZZLE_SANITY` from where the
                // sim fired is not this shot's gun — a body drawn off a
                // snapshot we have not got, or an id whose interpolation
                // is stale — and the round's own line is the honest
                // anchor again.
                let muzzle = self
                    .drawn_muzzle(owner)
                    .filter(|m| (*m - from).length() < MUZZLE_SANITY)
                    .unwrap_or_else(|| from + dir * REMOTE_MUZZLE);
                self.tracers.push(Tracer {
                    from,
                    muzzle,
                    to,
                    weapon,
                    born: now,
                });
                self.flashes.push(Flash {
                    pos: muzzle,
                    dir,
                    size: shot_flash(&row),
                    until: now + row.flash_ms,
                });
                self.puffs(feel::plume(muzzle, dir, weapon));
                let d = (from - ear.at).length();
                sfx.push(Play::spatial(
                    feel::shot_sfx(weapon, Dist::at(d)),
                    feel::remote_shot_volume(&row, d),
                    from,
                    ear.at,
                    ear.right,
                ));
            }
        } else if traces {
            // A continuation left no muzzle: it starts where a shield
            // reflected it or a body let it through, and that point is the
            // server's exactly.
            self.tracers.push(Tracer {
                from,
                muzzle: from,
                to,
                weapon,
                born: now,
            });
        }
        // The crack of a round passing my head: mine never do, and it is
        // never late, because it arrives with the round.
        if !mine && let Some(vol) = feel::crack(from, to, ear.at, stats.speed_max) {
            sfx.push(Play::centre(Sfx::Crack, vol));
        }
        let d = (to - ear.at).length();
        let n = feel::mark_normal(normal);
        if let Some(material) = feel::impact_material(hit, cover) {
            feel::add_mark(
                &mut self.marks,
                Mark {
                    pos: to,
                    normal: n,
                    weapon,
                    born: now,
                },
            );
            if traces {
                self.puffs(material.burst(to, n));
                sfx.push(Play::spatial(
                    material.sfx(),
                    material.volume() * feel::falloff(d),
                    to,
                    ear.at,
                    ear.right,
                ));
                if material == feel::Material::Metal && feel::ricochets(to.to_array()) {
                    self.puffs(feel::ricochet_sparks(to, n));
                    sfx.push(Play::spatial(
                        Sfx::Ricochet,
                        feel::RICOCHET_VOLUME * feel::falloff(d),
                        to,
                        ear.at,
                        ear.right,
                    ));
                }
            }
        } else if hit == SHOT_BODY && traces {
            self.puffs(feel::body_sparks(to));
            sfx.push(Play::spatial(
                Sfx::ImpactBody,
                0.4 * feel::falloff(d),
                to,
                ear.at,
                ear.right,
            ));
        } else if hit == SHOT_SHIELD {
            // Off the plate: metal sparks and the ring of it, for the
            // holder and for everyone watching.
            self.puffs(feel::Material::Metal.burst(to, -dir));
            sfx.push(Play::spatial(
                Sfx::ImpactMetal,
                feel::Material::Metal.volume() * feel::falloff(d),
                to,
                ear.at,
                ear.right,
            ));
        }
        if hit == SHOT_BODY || hit == SHOT_SHIELD {
            self.continuations.push((to, now));
        }
    }

    /// The loot slot (index into `loot_index`) of an obstacle, if it is a
    /// block.
    fn loot_slot(&self, obstacle: usize) -> Option<usize> {
        self.loot_index.iter().position(|&k| k == obstacle)
    }

    /// Where a block's centre is, for distance and for the pop.
    fn block_centre(&self, slot_idx: usize) -> Option<Vec3> {
        let o = self.obstacles.get(*self.loot_index.get(slot_idx)?)?;
        Some(Vec3::new(
            f32::midpoint(o.min[0], o.max[0]),
            f32::midpoint(o.base, o.h),
            f32::midpoint(o.min[1], o.max[1]),
        ))
    }

    /// A rocket went off: one flash ball, twelve shards under gravity,
    /// eight balls of smoke rising. Directions are a fixed fan, not random:
    /// there is no RNG on the client and a burst does not need one. A blast
    /// is all born at once — the delay is the muzzle plume's alone.
    fn blast_fx(&mut self, at: Vec3) {
        let now = self.time;
        self.fx.push(Fx {
            pos: at,
            vel: Vec3::ZERO,
            ttl: 0.08,
            life: 0.08,
            size: 2.2,
            color: Vec3::new(1.0, 0.85, 0.5),
            gravity: 0.0,
            born: now,
        });
        for k in 0_u8..12 {
            let a = f32::from(k) * std::f32::consts::TAU / 12.0;
            let up = 0.25 + 0.35 * f32::from(k % 3);
            self.fx.push(Fx {
                pos: at,
                vel: Vec3::new(a.cos(), up, a.sin()).normalize() * 9.0,
                ttl: 0.35,
                life: 0.35,
                size: 0.18,
                color: Vec3::new(1.0, 0.45, 0.12),
                gravity: 9.0,
                born: now,
            });
        }
        for k in 0_u8..8 {
            let a = f32::from(k) * std::f32::consts::TAU / 8.0 + 0.4;
            self.fx.push(Fx {
                pos: at + Vec3::new(a.cos() * 0.6, 0.3, a.sin() * 0.6),
                vel: Vec3::new(a.cos() * 0.8, 2.2, a.sin() * 0.8),
                ttl: 0.6,
                life: 0.6,
                size: 0.7,
                color: Vec3::new(0.30, 0.28, 0.26),
                gravity: 0.0,
                born: now,
            });
        }
    }

    /// Where `env_meshes()` got registered (set by `run_online` after load).
    pub const fn set_env_base(&mut self, base: u32) {
        self.env_base = base;
    }

    /// Where `props::prop_meshes()` got registered, with their measured
    /// sizes (set by `run_online` after load).
    pub const fn set_props(&mut self, base: u32, fits: &crate::props::PropFits) {
        self.props = Some(Props { base, fits: *fits });
    }

    /// Where `rounds::round_meshes()` got registered (set by `run_online`
    /// after load).
    pub const fn set_rounds(&mut self, base: u32) {
        self.rounds = Some(Rounds { base });
    }

    /// Install the jointed character (set by `run_online` after load).
    pub fn set_parts(&mut self, rc: Option<ember_engine::rig::RigCharacter>) {
        self.rig_character = rc;
    }

    fn render_pos(&self, id: u8) -> Vec2 {
        let a = self.t.clamp(0.0, 1.0);
        let f = self.from.get(&id).copied().unwrap_or_default();
        let to = self.to.get(&id).copied().unwrap_or_default();
        Vec2::new(f.x + (to.x - f.x) * a, f.z + (to.z - f.z) * a)
    }

    /// Interpolated feet height, so a remote player's jump is visible.
    fn render_y(&self, id: u8) -> f32 {
        let a = self.t.clamp(0.0, 1.0);
        let f = self.from.get(&id).copied().unwrap_or_default();
        let to = self.to.get(&id).copied().unwrap_or_default();
        f.y + (to.y - f.y) * a
    }

    /// The tip of the gun player `id` is drawn holding, in the world, or
    /// `None` when no gun of theirs is on screen (no state for them, or
    /// they are not alive in it — the render pass skips both, and a flash
    /// on a body nobody can see is worse than one on the round's line).
    ///
    /// Every number here is the render pass's own: the interpolated feet
    /// and position it draws the body at, `body_heights` for the hand, and
    /// `weapon_rot` with the model-space muzzle the sidecar gives the
    /// weapon `shown_weapon` picks. So the flash, the smoke and the start
    /// of the streak land on the barrel that is drawn, crouching or
    /// standing, however steeply the shooter is aiming. Without the
    /// viewmodel loaded the body carries the cube pistol, whose barrel tip
    /// is `BOX_MUZZLE` and which is yawed only, exactly as `push_gun`
    /// draws it.
    fn drawn_muzzle(&self, id: u8) -> Option<Vec3> {
        let p = self.latest.get(&id).filter(|p| p.alive)?;
        let aim = Vec2::new(p.ax, p.az);
        let (_, _, hand_y, _) = body_heights(p.crouch);
        let hand = hand_at(self.render_pos(id), self.render_y(id), aim, hand_y);
        let yaw = -aim.y.atan2(aim.x);
        Some(
            hand + self.assets.as_ref().map_or_else(
                || Quat::from_rotation_y(yaw) * BOX_MUZZLE,
                |a| weapon_rot(yaw, p.pitch) * a.muzzle_of(shown_weapon(p.weapon)),
            ),
        )
    }

    fn handle_of(&self, id: u8) -> String {
        self.metas
            .get(&id)
            .map_or_else(|| format!("player {id}"), |m| m.handle.clone())
    }

    /// The colour a player's body, ring, pips and hill are drawn in: the
    /// team's in team deathmatch, so a teammate is told from an enemy at a
    /// glance and never by remembering eight id colours; the id colour
    /// from `PlayerMeta` everywhere else. A player the state names but
    /// the metas do not is grey, as before.
    fn player_color(&self, id: u8) -> Vec3 {
        if self.mode == GameMode::Tdm
            && let Some(p) = self.latest.get(&id)
        {
            return feel::team_color(p.team);
        }
        self.metas
            .get(&id)
            .map_or(Vec3::splat(0.6), |m| Vec3::from_array(m.color))
    }

    /// My team, 0 when I am not in the state yet.
    fn my_team(&self) -> u8 {
        self.my_id
            .and_then(|id| self.latest.get(&id))
            .map_or(0, |p| p.team)
    }

    /// What `S2C::RoundOver` says: the team by name, the king by handle,
    /// the free-for-all winner by handle with the frags that ended it
    /// (from the message's own scores, since the state that follows may
    /// already be the reset).
    fn round_over_line(&self, winner: u8, team: bool, scores: &[(u8, u32)]) -> String {
        if team {
            return format!("{} wins the round", feel::team_name(winner));
        }
        let name = self.handle_of(winner);
        if self.mode == GameMode::Hill {
            return format!("{name} is king of the hill");
        }
        let frags = scores
            .iter()
            .find(|(id, _)| *id == winner)
            .map_or(0, |(_, s)| *s);
        format!("{name} wins the round ({frags} frags)")
    }

    /// The match segment of the status line: the pause countdown while a
    /// round is over (with the winner's line still beside it), else the
    /// mode's own score against its limit. In team deathmatch my team is
    /// named first; the status element is plain text, so the colour the
    /// plan asks for is the page's to add and the order is what this
    /// line can do.
    fn mode_line(&self, me: Option<&PState>) -> String {
        if self.round_pause > 0.0 {
            let secs = self.round_pause.ceil();
            let wait = format!("next round in {secs:.0} s");
            return match &self.round_line {
                Some(line) => format!("{line} · {wait}"),
                None => wait,
            };
        }
        let score = me.map_or(0, |p| p.score);
        match self.mode {
            GameMode::Ffa => format!("frags {score} / {FFA_FRAG_LIMIT}"),
            GameMode::Tdm => {
                let mine = self.my_team();
                let theirs = 1 - (mine & 1);
                format!(
                    "{} {} · {} {} / {TDM_FRAG_LIMIT}",
                    feel::team_name(mine),
                    self.team_score[usize::from(mine & 1)],
                    feel::team_name(theirs),
                    self.team_score[usize::from(theirs)]
                )
            }
            GameMode::Hill => {
                let king = match self.hill_holder {
                    HILL_FREE => "hill free".to_string(),
                    HILL_CONTESTED => "contested".to_string(),
                    id => format!("king: {}", self.handle_of(id)),
                };
                format!("hill {score} / {HILL_LIMIT} · {king}")
            }
        }
    }

    /// Full Tab-overlay scoreboard, shaped by the mode: frags and deaths
    /// sorted by score in free for all; the same under a header per team,
    /// blue first, in team deathmatch; hill points as SCORE in king of
    /// the hill, where the score is not the frags (the wire carries only
    /// the mode's score per player, so the frag column waits on a
    /// `PState.frags`).
    fn scoreboard_text(&self) -> String {
        let mut rows: Vec<&PState> = self.latest.values().collect();
        let by_score = |a: &PState, b: &PState| b.score.cmp(&a.score).then(a.id.cmp(&b.id));
        if self.mode == GameMode::Tdm {
            rows.sort_by(|a, b| a.team.cmp(&b.team).then(by_score(a, b)));
        } else {
            rows.sort_by(|a, b| by_score(a, b));
        }
        let column = if self.mode == GameMode::Hill {
            "SCORE"
        } else {
            "FRAGS"
        };
        let mut s = format!("{:<20} {column:>6} {:>7}\n", "PLAYER", "DEATHS");
        s.push_str(&"─".repeat(35));
        s.push('\n');
        let mut header: Option<u8> = None;
        for p in rows {
            if self.mode == GameMode::Tdm && header != Some(p.team) {
                header = Some(p.team);
                writeln!(
                    s,
                    "{} {}",
                    feel::team_name(p.team),
                    self.team_score[usize::from(p.team & 1)]
                )
                .expect("writing to a String cannot fail");
            }
            let me = if Some(p.id) == self.my_id {
                "▶ "
            } else {
                "  "
            };
            let state = if p.alive { "" } else { " ☠" };
            // Char-truncated so 20-char/unicode handles can't break columns.
            let name: String = self.handle_of(p.id).chars().take(16).collect();
            writeln!(s, "{me}{name:<16} {:>6} {:>7}{state}", p.score, p.deaths)
                .expect("writing to a String cannot fail");
        }
        s
    }

    fn scoreboard(&self) -> String {
        let mut rows: Vec<&PState> = self.latest.values().collect();
        rows.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
        let list = rows
            .iter()
            .take(4)
            .map(|p| {
                let me = if Some(p.id) == self.my_id { "▶" } else { "" };
                format!("{me}{} {}", self.handle_of(p.id), p.score)
            })
            .collect::<Vec<_>>()
            .join("  ·  ");
        let me = self.my_id.and_then(|id| self.latest.get(&id));
        let hp = me
            .map(|p| {
                if p.alive {
                    "♥".repeat(p.hp as usize) + &"♡".repeat(MAX_HP.saturating_sub(p.hp) as usize)
                } else {
                    "respawning…".into()
                }
            })
            .unwrap_or_default();
        let gun = me.map(gun_line).unwrap_or_default();
        let mode = self.mode_line(me);
        let pad = if self.pad_status_shown == "none" {
            String::new()
        } else {
            format!("   gamepad: {}", self.pad_status_shown)
        };
        format!(
            "{hp}  {gun}   {mode}   {list}   ({} in arena){pad}",
            self.latest.len()
        )
    }
}

/// The HUD's weapon line: `AK-47 24/30 · 30`, `Sidearm 7/8 · ∞`, or the
/// reload glyph in place of the count while the magazine is out.
fn gun_line(p: &PState) -> String {
    let reserve = if p.reserve == RESERVE_INFINITE {
        "∞".to_string()
    } else {
        p.reserve.to_string()
    };
    if p.reloading {
        format!("{} ⟳ · {reserve}", weapon_name(p.weapon))
    } else {
        format!(
            "{} {}/{} · {reserve}",
            weapon_name(p.weapon),
            p.ammo,
            weapon_stats(p.weapon).mag
        )
    }
}

/// What a block hands out, for the status line: `AK-47 (30+30)`.
fn loadout_of(weapon: u8) -> String {
    let s = weapon_stats(weapon);
    format!("{} ({}+{})", s.name, s.mag, s.reserve)
}

impl EmberGame for ShooterGame {
    // Keeping the event, prediction, and render phases together preserves their state-update order.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        self.time += dt;
        self.since_input += dt;
        self.since_ping += dt;
        self.since_status += dt;
        self.bullets_age += dt;

        let mut status_event: Option<String> = None;
        // Queue sounds and play at the end under a budget: a backlogged
        // burst (hidden tab catching up) must not blast every buffered cue.
        let mut sfx: Vec<Play> = Vec::new();
        // Footsteps started this frame, against `feel::STEP_CAP`: my own
        // first (it is one cue and it is mine), then as many of the lobby's
        // as the cap allows. A step dropped here costs nothing a player can
        // act on; the next one is a fraction of a second behind it.
        let mut steps_queued = 0usize;
        // The listener for every spatial cue this frame: my eye and my
        // right, from last frame's look.
        let ear = {
            let (sz, cx) = self.yaw.sin_cos();
            Ear {
                at: Vec3::new(self.pred_pos.x, self.pred_y + self.eye_h, self.pred_pos.y),
                right: Vec3::new(-sz, 0.0, cx),
            }
        };
        let mut drained: Vec<S2C> = Vec::new();
        while let Some(msg) = self.chan.poll() {
            drained.push(msg);
        }
        // A backlog is measured in states, not messages: one tick fans out
        // into up to eight `Hit`, eight `Kill`, a `Blast`, a `Loot` and the
        // `State` that carries them, which is a crowded moment and not a
        // stall, and it is exactly the frame the blast, the death and the
        // hurt cues matter. More than three states in one frame is a
        // hidden tab catching up; anything less is thinned by
        // `prioritize` and the budget below.
        let states_drained = drained
            .iter()
            .filter(|m| matches!(m, S2C::State { .. }))
            .count();
        let suppress_sfx = states_drained > 3;
        // The arrival-gap estimate must see at most one sample per frame:
        // self.time advances once per frame, so every extra state drained in
        // the same frame reads as a 0 ms gap and would collapse the estimate.
        let mut sampled_gap = false;
        for msg in drained {
            match msg {
                S2C::Welcome { .. } => set_status("connected…"),
                S2C::GameJoined {
                    id,
                    seed,
                    arena_half,
                    players,
                    map,
                    mode,
                } => {
                    self.my_id = Some(id);
                    self.mode = GameMode::from_name(&mode).unwrap_or_default();
                    self.arena_half = arena_half;
                    // The same level the server built its lobby from, so
                    // prediction and authority resolve against identical
                    // cover; the seed is only what an unknown name falls
                    // back to.
                    let level = Level::named(&map, seed);
                    self.hill = level.hill;
                    self.team_score = [0, 0];
                    self.hill_holder = HILL_FREE;
                    self.round_pause = 0.0;
                    self.round_line = None;
                    self.obstacles = level.obstacles;
                    self.pads_pos = level.pads;
                    self.decor = level.decor;
                    self.pads_active = vec![true; self.pads_pos.len()];
                    // The blocks, in obstacle order: the index space the
                    // server's `State.loot` and `Loot.block` speak in.
                    self.loot_index = self
                        .obstacles
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| o.kind == Cover::Loot)
                        .map(|(k, _)| k)
                        .collect();
                    self.loot_active = vec![true; self.loot_index.len()];
                    self.loot_bump = vec![None; self.loot_index.len()];
                    self.last_bonk_at = vec![None; self.loot_index.len()];
                    self.pops.clear();
                    self.history.clear();
                    self.reload_started = None;
                    self.was_alive = false; // first State snaps the prediction
                    for m in players {
                        self.metas.insert(m.id, m);
                    }
                    // A scripted client has no player to instruct, and the
                    // line must not tell a capture's reader to click: this
                    // client ignores clicks and never takes the cursor.
                    status_event = Some(if self.script.is_some() {
                        "in the arena — driven by EMBER_SCRIPT · keyboard, mouse and pad ignored"
                            .into()
                    } else {
                        "in the arena — click to capture mouse · WASD move · Shift sprint · C crouch · Q shield (reflects!) · click fire".to_string()
                    });
                }
                S2C::PlayerJoined { meta } => {
                    status_event = Some(format!("{} joined the arena", meta.handle));
                    self.metas.insert(meta.id, meta);
                }
                S2C::PlayerLeft { id } => {
                    status_event = Some(format!("{} left", self.handle_of(id)));
                    self.metas.remove(&id);
                    self.from.remove(&id);
                    self.to.remove(&id);
                    self.latest.remove(&id);
                }
                S2C::State {
                    tick,
                    players,
                    bullets,
                    pads,
                    loot,
                    team_score,
                    hill,
                    round_pause,
                } => {
                    self.last_tick = tick;
                    self.pads_active = pads;
                    // The round restarting: the pause ran out in this
                    // state. Everyone is respawned with the sidearm and
                    // every score is zero, none of which is a holster, a
                    // pickup or a death, so the cues below that read a
                    // weapon change stay quiet for it.
                    let restarted = self.round_pause > 0.0 && round_pause <= 0.0;
                    if restarted {
                        self.round_line = None;
                    }
                    self.team_score = team_score;
                    self.hill_holder = hill;
                    self.round_pause = round_pause;
                    // A block the server never mentions stays armed: the
                    // list is index-aligned, and a short one is a server
                    // that predates a block, not a spent block.
                    for (i, armed) in loot.into_iter().enumerate() {
                        if let Some(slot) = self.loot_active.get_mut(i) {
                            *slot = armed;
                        }
                    }
                    // ---- cues from state diffs (before overwrite) ----
                    {
                        // Bullet counts per owner; position and weapon track
                        // the NEWEST bullet (distance falloff, and which
                        // gun's cue a remote shot plays).
                        let mut prev_counts: HashMap<u8, usize> = HashMap::new();
                        for b in &self.bullets {
                            *prev_counts.entry(b.owner).or_insert(0) += 1;
                        }
                        let mut curr: HashMap<u8, (usize, [f32; 2], u8)> = HashMap::new();
                        for b in &bullets {
                            let e = curr.entry(b.owner).or_insert((0, [b.x, b.z], b.weapon));
                            e.0 += 1;
                            e.1 = [b.x, b.z];
                            e.2 = b.weapon;
                        }
                        // Remote rocket launches (mine are cued from ammo
                        // below). A bullet's shot is cued from its `Shot`
                        // event since v20; a rocket's event arrives when
                        // it detonates, seconds after the launch, so the
                        // launch is still read off the state that first
                        // carries the rocket, where it is at the muzzle.
                        for (&owner, &(n, pos, weapon)) in &curr {
                            if Some(owner) != self.my_id
                                && !feel::traces(weapon)
                                && n > prev_counts.get(&owner).copied().unwrap_or(0)
                            {
                                let at = Vec3::new(pos[0], ear.at.y, pos[1]);
                                let d = (at - ear.at).length();
                                let f = weapon_feel(weapon);
                                sfx.push(Play::spatial(
                                    feel::shot_sfx(weapon, Dist::at(d)),
                                    feel::remote_shot_volume(&f, d),
                                    at,
                                    ear.at,
                                    ear.right,
                                ));
                            }
                        }
                        // My own transitions, from authoritative state.
                        // Copied out: the cues below borrow `self` mutably.
                        if let (Some(me), Some(new_me)) = (
                            self.my_id.and_then(|id| self.latest.get(&id)).copied(),
                            self.my_id
                                .and_then(|id| players.iter().find(|p| p.id == id))
                                .copied(),
                        ) {
                            // A falling ammo count is not by itself a shot:
                            // a loot grant reassigns the whole magazine and
                            // a respawn resets it in the same state that
                            // flips alive. Requiring the weapon unchanged
                            // and the player alive on BOTH sides rejects
                            // those without losing a real shot, since
                            // firing needs alive and the weapon only changes
                            // on a grant, a dry gun or a respawn.
                            if new_me.ammo < me.ammo
                                && new_me.weapon == me.weapon
                                && me.alive
                                && new_me.alive
                            {
                                let f = weapon_feel(new_me.weapon);
                                // A fast gun can fire twice between states;
                                // every confirmed round climbs, one cues.
                                for _ in new_me.ammo..me.ammo {
                                    self.climb.shot(&f);
                                }
                                // My own shot: the near variant, centred,
                                // now.
                                sfx.push(Play::centre(
                                    feel::shot_sfx(new_me.weapon, Dist::Near),
                                    f.volume,
                                ));
                                self.feedback
                                    .rumble(f.rumble.strong, f.rumble.weak, f.rumble.ms);
                                self.shake.hit(f.launch_shake);
                                // Recoil and muzzle flash hang off the same
                                // authoritative signal as the audio: a round
                                // that the SERVER agrees left the weapon.
                                self.shot_started = Some(self.time);
                                self.shots = self.shots.wrapping_add(1);
                                self.own_plume = true;
                                if weapon_stats(new_me.weapon).kind == Projectile::Rocket {
                                    self.launch_smoke = true;
                                }
                            }
                            let changed = new_me.weapon != me.weapon
                                && me.alive
                                && new_me.alive
                                && !restarted;
                            if changed && new_me.weapon == SIDEARM {
                                // A looted gun ran dry: the sidearm is back.
                                self.holster_started = Some(self.time);
                                self.cue(feel::holster(), &mut sfx);
                            } else if changed && self.time - self.last_pop_at > 0.5 {
                                // A grant with no pop before it: a pad.
                                sfx.push(Play::centre(Sfx::Upgrade, 0.55));
                                status_event =
                                    Some(format!("⬆ picked up: {}", loadout_of(new_me.weapon)));
                            }
                            if new_me.reloading && !me.reloading {
                                self.cue(feel::reload_start(new_me.weapon), &mut sfx);
                                self.reload_started = Some(self.time);
                            } else if !new_me.reloading {
                                if me.reloading && new_me.alive {
                                    self.cue(feel::reload_end(), &mut sfx);
                                }
                                self.reload_started = None;
                            }
                        }
                    }
                    // Compute current render positions from the OLD from/to
                    // pair BEFORE replacing anything, then interpolate from
                    // there toward the new state. Snap (no slide) for a
                    // first sighting or a teleport-sized jump (respawn).
                    let mut new_from = HashMap::with_capacity(players.len());
                    for p in &players {
                        let snap = self.to.get(&p.id).map_or(
                            PSnap {
                                x: p.x,
                                z: p.z,
                                y: p.y,
                            },
                            |prev_to| {
                                let cur = self.render_pos(p.id);
                                let (dx, dz) = (p.x - prev_to.x, p.z - prev_to.z);
                                if dx * dx + dz * dz > 6.0 * 6.0 {
                                    // respawn teleport
                                    PSnap {
                                        x: p.x,
                                        z: p.z,
                                        y: p.y,
                                    }
                                } else {
                                    PSnap {
                                        x: cur.x,
                                        z: cur.y,
                                        y: self.render_y(p.id),
                                    }
                                }
                            },
                        );
                        new_from.insert(p.id, snap);
                    }
                    self.from = new_from;
                    self.to = players
                        .iter()
                        .map(|p| {
                            (
                                p.id,
                                PSnap {
                                    x: p.x,
                                    z: p.z,
                                    y: p.y,
                                },
                            )
                        })
                        .collect();
                    self.latest = players.into_iter().map(|p| (p.id, p)).collect();
                    // States do not arrive on the nominal 33.3 ms grid - the
                    // server flushes between reads and the tunnel adds jitter,
                    // so measured gaps run 21-53 ms. Track what is actually
                    // happening instead of assuming.
                    if self.last_state_at > 0.0 && !sampled_gap {
                        let gap = (self.time - self.last_state_at).clamp(0.008, 0.200);
                        self.state_interval += (gap - self.state_interval) * 0.15;
                    }
                    sampled_gap = true;
                    self.last_state_at = self.time;
                    self.t = 0.0;
                    // Bullet heights now arrive from the server with the rest
                    // of the bullet. What stood here was a local guess —
                    // matching bullets across states by predicted position
                    // and seeding new ones near me with my own aim pitch —
                    // which drew MY tracers along my look ray while the
                    // authoritative shot went somewhere else entirely.
                    self.bullets = bullets;
                    self.bullets_age = 0.0;

                    // Reconcile my prediction: rebase on the authoritative
                    // position and replay every not-yet-acked command.
                    if let Some(my) = self.my_id.and_then(|id| self.latest.get(&id)) {
                        let server = Vec2::new(my.x, my.z);
                        let newly_alive = my.alive && !self.was_alive;
                        self.was_alive = my.alive;
                        if my.alive {
                            // KEEP the acked command: the instant this state
                            // describes sits inside its window, at
                            // sent_at + however long the server has been
                            // applying it, and its tail after that point is
                            // ours to replay. Deriving the cursor from a
                            // command we then dropped made it available on
                            // only two states in three, so the replay window
                            // flipped between two lengths at 30 Hz.
                            while self.history.front().is_some_and(|c| c.seq < my.ack) {
                                self.history.pop_front();
                            }
                            let state_at = self
                                .history
                                .front()
                                .filter(|c| c.seq == my.ack)
                                .map(|c| c.sent_at + f32::from(my.ack_age_ticks) * FIXED_DT);
                            let mut p = [server.x, server.y];
                            // BOTH halves of the vertical state come from the
                            // server. Seeding vy from our own prediction pairs
                            // the server's PAST height with our PRESENT speed
                            // and re-integrates gravity across a window the
                            // forward prediction has already covered.
                            let (mut y, mut vy) = (my.y, my.vy);
                            let mut it = self.history.iter().peekable();
                            while let Some(c) = it.next() {
                                let end = it.peek().map_or(self.time, |n| n.sent_at);
                                // Replay only what the server has not seen
                                // yet: the slice of this command after the
                                // instant the state describes.
                                let start = state_at.map_or(c.sent_at, |s| c.sent_at.max(s));
                                let dur = (end - start).clamp(0.0, 0.3);
                                // A press launches on one step, exactly as the
                                // server consumes it on one tick - and the
                                // acked command's press is already IN the
                                // state we are rebasing on, so replaying it
                                // would launch the same jump twice.
                                let mut press = c.jump && c.seq != my.ack;
                                // An input event must never be deleted by
                                // arithmetic: if the slice trims to nothing,
                                // still give the press one tick to happen in.
                                let dur = if press && dur < FIXED_DT {
                                    FIXED_DT
                                } else {
                                    dur
                                };
                                // Replay at the server's tick length. Horizontal
                                // motion is exact under time-splitting; gravity
                                // is not - one 50 ms step lands 2 cm from three
                                // 16.7 ms ones, and the error compounds.
                                let mut left = dur;
                                while left > 1e-6 {
                                    let step = left.min(FIXED_DT);
                                    p = move_circle(p, y, c.mv, c.speed, step, &self.obstacles);
                                    let stepped =
                                        step_vertical(p, y, vy, press, step, &self.obstacles);
                                    press = false;
                                    y = stepped.y;
                                    vy = stepped.vy;
                                    left -= step;
                                }
                            }
                            let rebased = Vec2::new(p[0], p[1]);
                            self.pred_y = y;
                            self.pred_vy = vy;
                            if newly_alive || rebased.distance(self.pred_pos) > 4.0 {
                                // Respawn / teleport: snap everything.
                                self.pred_pos = server;
                                self.own_render = server;
                                self.render_y_own = my.y;
                                self.history.clear();
                                if newly_alive {
                                    sfx.push(Play::centre(Sfx::Respawn, 0.4));
                                }
                            } else {
                                self.pred_pos = rebased;
                            }
                        }
                    }
                }
                S2C::Kill { killer, victim } => {
                    if Some(victim) == self.my_id {
                        // A self-kill is a death first: the shake and the
                        // long rumble, not the kill marker.
                        self.cue(feel::death(), &mut sfx);
                    } else if Some(killer) == self.my_id {
                        self.cue(feel::kill(), &mut sfx);
                        // Big red confirm marker: the elimination register.
                        self.kill_t = 0.55;
                    } else {
                        sfx.push(Play::centre(Sfx::Hit, 0.12));
                    }
                    let line = if killer == victim && Some(victim) == self.my_id {
                        "☠ you blew yourself up".to_string()
                    } else if Some(victim) == self.my_id {
                        format!("☠ you were fragged by {}", self.handle_of(killer))
                    } else if Some(killer) == self.my_id {
                        format!("✚ you fragged {}", self.handle_of(victim))
                    } else {
                        format!(
                            "{} fragged {}",
                            self.handle_of(killer),
                            self.handle_of(victim)
                        )
                    };
                    status_event = Some(line);
                }
                S2C::Error { message } => {
                    if self.my_id.is_none() {
                        // Failed to even get into a game (bad password, name
                        // taken, ...): dead end, tell the player.
                        self.lost = true;
                        set_status(&format!("server error: {message}"));
                    } else {
                        // In-game errors are informational, keep playing.
                        status_event = Some(format!("server: {message}"));
                    }
                }
                // Authoritative hits, from `Sim.hits`. What stood here was
                // a guess ("one of my bullets vanished and an enemy lost
                // hp") that fired falsely on a reflected round and would
                // have missed the second body of a pierce.
                S2C::Hit {
                    shooter,
                    victim,
                    dmg: _,
                    head,
                } => {
                    if Some(shooter) == self.my_id && Some(victim) != self.my_id {
                        self.cue(feel::hit(head), &mut sfx);
                        self.hitmarker_t = 0.14;
                        self.hitmarker_scale = if head { 1.5 } else { 1.0 };
                    }
                    if Some(victim) == self.my_id {
                        self.cue(feel::hurt(), &mut sfx);
                    } else {
                        // Visual confirmation on the body: a damage flash
                        // for everyone watching. The sparks moved to the
                        // `Shot` event (v20), which knows the exact point
                        // the round met the body.
                        self.flash.insert(victim, 0.18);
                    }
                }
                S2C::Blast { x, y, z, owner: _ } => {
                    let at = Vec3::new(x, y, z);
                    let d = (at - ear.at).length();
                    self.cue_at(feel::blast(d), at, ear, &mut sfx);
                    self.blast_fx(at);
                }
                S2C::Loot {
                    player,
                    block,
                    weapon,
                } => {
                    let slot_idx = usize::from(block);
                    if let Some(armed) = self.loot_active.get_mut(slot_idx) {
                        *armed = false;
                    }
                    // The bump for everyone who did not predict it (a remote
                    // bonk, or my own if the prediction missed it).
                    if let Some(bump) = self.loot_bump.get_mut(slot_idx)
                        && bump.is_none_or(|t0| self.time - t0 >= BUMP_SECS)
                    {
                        *bump = Some(self.time);
                    }
                    self.pops.push(Pop {
                        slot: slot_idx,
                        weapon,
                        started: self.time,
                    });
                    if Some(player) == self.my_id {
                        self.last_pop_at = self.time;
                        self.cue(feel::pop(true), &mut sfx);
                        status_event = Some(format!("? popped: {}", loadout_of(weapon)));
                    } else if let Some(c) = self.block_centre(slot_idx)
                        && (c - Vec3::new(self.pred_pos.x, self.pred_y, self.pred_pos.y)).length()
                            < feel::POP_EARSHOT
                    {
                        self.cue_at(feel::pop(false), c, ear, &mut sfx);
                    }
                }
                // The client never asks for a listing; a page's lobby
                // browser does, in plain JS. Logged so a stray one is seen.
                S2C::LobbyList { lobbies } => {
                    tracing::debug!(
                        maps = ?lobbies.iter().map(|l| l.map.as_str()).collect::<Vec<_>>(),
                        modes = ?lobbies.iter().map(|l| l.mode.as_str()).collect::<Vec<_>>(),
                        "unsolicited lobby list"
                    );
                }
                // A round ended. The line names the winner in the mode's
                // own words; the pause that follows arrives in `State`
                // and keeps the line on screen beside the countdown.
                S2C::RoundOver {
                    winner,
                    team,
                    scores,
                } => {
                    let won = if team {
                        self.my_id
                            .and_then(|id| self.latest.get(&id))
                            .is_some_and(|p| p.team == winner)
                    } else {
                        Some(winner) == self.my_id
                    };
                    self.cue(feel::round_over(won), &mut sfx);
                    let line = self.round_over_line(winner, team, &scores);
                    status_event = Some(line.clone());
                    self.round_line = Some(line);
                }
                // `Shot`: a round's segment ended (v20). The streak, the
                // flash and plume, the gunshot, the crack, the impact and
                // its mark all come from here. `victim` is not read: the
                // damage flash on the body arrives as `Hit`.
                S2C::Shot {
                    owner,
                    weapon,
                    x0,
                    y0,
                    z0,
                    x1,
                    y1,
                    z1,
                    hit,
                    cover,
                    victim: _,
                    normal,
                } => {
                    self.on_shot(
                        owner,
                        weapon,
                        Vec3::new(x0, y0, z0),
                        Vec3::new(x1, y1, z1),
                        hit,
                        cover,
                        normal,
                        ear,
                        &mut sfx,
                    );
                }
                S2C::Pong { .. } => {}
            }
        }
        if self.chan.is_dead() && !self.lost {
            self.lost = true;
            set_status("connection lost — reload to play again");
        }

        // ---- the script, when one drives this client (`EMBER_SCRIPT`) ----
        // A scripted client is hands-off: every read of `input` below is
        // behind this tick, so no key, no mouse button, no mouse motion and
        // no pad reaches the game. That is the whole point — the operator
        // keeps their machine while a capture runs, and their stray mouse
        // cannot turn our camera (winit delivers raw mouse motion whether
        // the window is focused or not). Once spent, the tick is neutral
        // forever; the client keeps drawing frames and still touches
        // nothing.
        let tick = self.script.as_mut().map(|s| s.advance(dt));
        let scripted = tick.is_some();
        // Two lines the harness waits on: the first scripted frame (which is
        // well after the window appears — the GPU context is built in
        // between — so it, not the window, is when a shot list's clock
        // starts), and the frame the timeline runs out.
        if scripted && !self.script_began {
            self.script_began = true;
            tracing::info!("EMBER_SCRIPT starts");
        }
        // …and one line per step boundary, on the client's OWN clock. The
        // harness's wall clock and this timeline drift apart, because a
        // frame longer than the engine's `dt` clamp loses script time for
        // good, so a shot can be timed off a step instead of off seconds.
        if let Some(n) = tick.as_ref().and_then(|t| t.began) {
            tracing::info!(step = n, "EMBER_SCRIPT step begins");
        }
        if !self.script_spent && self.script.as_ref().is_some_and(script::Timeline::is_done) {
            self.script_spent = true;
            tracing::info!("EMBER_SCRIPT is spent; the client stays up and stays hands-off");
        }

        // ---- the pad, merged with the keys: either device at any moment ----
        let pad = if scripted { None } else { input.pad() };
        if pad.is_some() && self.pad_status_shown != input.pad_status() {
            self.pad_status_shown = input.pad_status();
            status_event = Some(format!("gamepad: {}", self.pad_status_shown));
        }
        let pad_down = |b: PadButton| pad.is_some_and(|p| p.down(b));
        let stick_l = pad.map_or([0.0, 0.0], |p| p.left);
        let stick_r = pad.map_or([0.0, 0.0], |p| p.right);

        // ---- ADS (RMB or LT): tighter FOV, the look slowed to match ----
        let aiming = tick.as_ref().map_or_else(
            || input.mouse_down(MouseButton::Right) || pad.is_some_and(|p| p.lt > 0.5),
            |t| t.held.down(script::Hold::Ads),
        );
        let zoom_target = if aiming { 1.0 } else { 0.0 };
        self.zoom += (zoom_target - self.zoom) * (1.0 - (-dt * 14.0).exp());
        // The gun I am drawn with decides the field of view, and the field
        // of view decides the sensitivity, so it is read here, before the
        // look, and reused by the frame below (nothing between rewrites
        // `latest`). The frame's recoil kick is what the wire never sees.
        let me_latest = self.my_id.and_then(|id| self.latest.get(&id)).copied();
        let my_weapon = shown_weapon(me_latest.map_or(SIDEARM, |p| p.weapon));
        let my_feel = weapon_feel(my_weapon);
        let fov_now = my_feel.fov(self.zoom);
        // A narrower view turns slower in the same ratio (a 20x scope turns
        // twenty times slower), so a target crossing the screen costs the
        // same hand travel at every zoom. Mouse and stick alike.
        let look_scale = feel::look_scale(fov_now);

        // ---- first-person look: mouse deltas and the right stick -> yaw/pitch ----
        let sens = LOOK_SENS * look_scale;
        // The device's raw motion, or nothing at all while a script drives:
        // this is the read that used to hand the operator's mouse our camera.
        let (mdx, mdy) = if scripted {
            (0.0, 0.0)
        } else {
            input.mouse_delta()
        };
        self.yaw += mdx * sens + stick_r[0] * 2.8 * dt * look_scale;
        self.pitch = (self.pitch - mdy * sens + stick_r[1] * 2.0 * dt * look_scale)
            .clamp(-MAX_PITCH, MAX_PITCH);
        // A script's `aim`/`turn`/`look` lands once, on the frame its step
        // begins, and the heading then holds: the timeline sets the angle,
        // it does not sweep to it.
        if let Some(t) = tick.as_ref() {
            match t.yaw {
                Some(script::Turn::To(y)) => self.yaw = y,
                Some(script::Turn::By(d)) => self.yaw += d,
                None => {}
            }
            if let Some(p) = t.pitch {
                self.pitch = p.clamp(-MAX_PITCH, MAX_PITCH);
            }
        }
        let (fz, fx) = self.yaw.sin_cos();
        self.aim = Vec2::new(fx, fz);
        let forward2 = self.aim;
        // Camera right (from look_at basis: cross(forward, up)).
        let right2 = Vec2::new(-fz, fx);

        // ---- stance + camera-relative movement ----
        // L3 latches sprint until the left stick drops under half: a stick
        // cannot hold a modifier the way a key does.
        let l3 = pad_down(PadButton::L3);
        if l3 && !self.prev_l3 {
            self.sprint_latch = true;
        }
        self.prev_l3 = l3;
        if stick_l[0].hypot(stick_l[1]) < 0.5 {
            self.sprint_latch = false;
        }
        let sprint = tick.as_ref().map_or_else(
            || {
                input.down(KeyCode::ShiftLeft)
                    || input.down(KeyCode::ShiftRight)
                    || self.sprint_latch
            },
            |t| t.held.down(script::Hold::Sprint),
        );
        let crouch = tick.as_ref().map_or_else(
            || input.down(KeyCode::KeyC) || pad_down(PadButton::East),
            |t| t.held.down(script::Hold::Crouch),
        );
        // Held, like every other intent: there is no local toggle state that
        // a dropped input packet could leave disagreeing with the server.
        let shield = tick.as_ref().map_or_else(
            || input.down(KeyCode::KeyQ) || pad_down(PadButton::LB),
            |t| t.held.down(script::Hold::Shield),
        );
        self.shield_raise +=
            ((if shield { 1.0 } else { 0.0 }) - self.shield_raise) * (1.0 - (-dt * 16.0).exp());
        let target_eye = if crouch { EYE_CROUCH } else { EYE_STAND };
        self.eye_h += (target_eye - self.eye_h) * (1.0 - (-dt * 12.0).exp());

        // The left stick is already dead-zoned and curved by the platform.
        let (ax_fwd, ax_right) = tick.as_ref().map_or_else(
            || {
                (
                    input.axis(KeyCode::KeyS, KeyCode::KeyW) + stick_l[1],
                    input.axis(KeyCode::KeyA, KeyCode::KeyD) + stick_l[0],
                )
            },
            |t| (t.held.fwd, t.held.right),
        );
        let mut mv = forward2 * ax_fwd + right2 * ax_right;
        if mv.length_squared() > 1.0 {
            mv = mv.normalize();
        }
        let moving = mv.length_squared() > 0.01;
        let fire = tick.as_ref().map_or_else(
            || input.mouse_down(MouseButton::Left) || pad.is_some_and(|p| p.rt > 0.5),
            |t| t.held.down(script::Hold::Fire),
        );
        // The dry trigger: once per press, while the magazine is out for a
        // reload. An empty magazine that is not reloading cannot be seen
        // from here, because the sim starts the reload on the tick the last
        // round leaves, so the reload is the only state a pull can find
        // nothing in. Nothing goes to the server for it; the sim refuses
        // the round anyway.
        if fire
            && !self.prev_fire
            && self
                .my_id
                .and_then(|id| self.latest.get(&id))
                .is_some_and(|p| p.alive && p.reloading)
        {
            let c = feel::empty_trigger();
            self.shake.hit(c.shake);
            if let Some(r) = c.rumble {
                self.feedback.rumble(r.strong, r.weak, r.ms);
            }
            if let (Some(audio), Some((s, v))) = (self.audio.as_ref(), c.sfx) {
                audio.play(s, v);
            }
        }
        self.prev_fire = fire;
        // A jump is a press, latched until the next send frame: sampling the
        // held key at 20 Hz dropped taps shorter than 50 ms outright, and a
        // held key re-launched the player off every surface they touched.
        // The pad's South button is the same latch.
        let space = tick.as_ref().map_or_else(
            || input.down(KeyCode::Space) || pad_down(PadButton::South),
            |t| t.held.down(script::Hold::Jump),
        );
        let jump = space && !self.prev_space;
        self.prev_space = space;
        self.jump_pending |= jump;
        self.pred_jump |= jump;
        // Melee, latched the same way. Note what is deliberately absent: there
        // is no `pred_melee`. Movement is predicted because it is ours to
        // predict, but a kill is not - the sim resolves melee server-side for
        // exactly the reason it resolves bullets there, and a client that
        // guessed a kill would have to un-kill someone when the server
        // disagreed. The swing is sent; the outcome comes back.
        let e_down = tick.as_ref().map_or_else(
            || input.down(KeyCode::KeyE) || pad_down(PadButton::RB),
            |t| t.held.down(script::Hold::Melee),
        );
        let melee = e_down && !self.prev_e;
        self.prev_e = e_down;
        self.melee_pending |= melee;
        if melee
            && self
                .melee_started
                .is_none_or(|t0| self.time - t0 >= MELEE_COOLDOWN)
        {
            self.melee_started = Some(self.time);
        }

        // Walk bob (cosmetic, client-side only).
        if moving {
            self.bob_t += dt
                * if sprint {
                    11.0
                } else if crouch {
                    5.5
                } else {
                    8.0
                };
        }
        let bob = if moving {
            (self.bob_t).sin() * 0.035
        } else {
            0.0
        };

        let me_alive = self
            .my_id
            .and_then(|id| self.latest.get(&id))
            .is_some_and(|p| p.alive);
        // Prediction reads the same rule the server applies, shield included
        // — a raised shield cancels sprint, and predicting otherwise would
        // rubber-band anyone who raised one while running.
        let speed = stance_speed(sprint, crouch, shield);

        // Send intents at a fixed cadence (also the keepalive), remembering
        // each command until the server acks it.
        if self.my_id.is_some() && !self.lost && self.since_input >= 0.05 {
            self.since_input = 0.0;
            let seq = self.next_seq;
            self.next_seq = self.next_seq.wrapping_add(1);
            // Whether a press happened anywhere in this send window, not
            // whether the key happens to be down on this exact frame.
            let jump_press = if me_alive {
                std::mem::take(&mut self.jump_pending)
            } else {
                false
            };
            if me_alive {
                self.history.push_back(Cmd {
                    seq,
                    mv: [mv.x, mv.y],
                    speed,
                    jump: jump_press,
                    sent_at: self.time,
                });
                if self.history.len() > 64 {
                    self.history.pop_front();
                }
            }
            // The sim tick our remote-player rendering currently shows —
            // the server rewinds our hit tests to it (lag compensation).
            let view_tick = self.last_tick.saturating_sub(
                ((1.0 - self.t.clamp(0.0, 1.0)) * STATE_EVERY_TICKS as f32).round() as u64,
            );
            self.chan.send(&C2S::Input {
                seq,
                view_tick,
                mx: mv.x,
                my: mv.y,
                ax: self.aim.x,
                az: self.aim.y,
                // The elevation the sim fires along. Previously dropped
                // here, which is why the shot ignored where you looked.
                pitch: self.pitch,
                fire,
                sprint,
                crouch,
                reload: tick.as_ref().map_or_else(
                    || input.down(KeyCode::KeyR) || pad_down(PadButton::West),
                    |t| t.held.down(script::Hold::Reload),
                ),
                jump: jump_press,
                // Sent raw: the trigger gate lives in the sim, so `fire` is
                // reported honestly even while Q is down and the server is
                // the only thing that decides a round did not leave.
                shield,
                // Cleared on send whether or not anyone was in reach: the
                // server owns the cooldown, so a swing at air still costs a
                // swing and the client never has to model that.
                melee: if me_alive {
                    std::mem::take(&mut self.melee_pending)
                } else {
                    false
                },
                // Held, like the shield: the sim tightens the cone of the
                // round fired this tick by it.
                ads: aiming,
            });
        }

        // Predict my own movement locally — instant response; the State
        // handler above rebases this on the server's authority.
        if me_alive {
            let was = self.pred_pos;
            let p = move_circle(
                self.pred_pos.to_array(),
                self.pred_y,
                [mv.x, mv.y],
                speed,
                dt,
                &self.obstacles,
            );
            self.pred_pos = Vec2::new(p[0], p[1]);
            let stepped = step_vertical(
                p,
                self.pred_y,
                self.pred_vy,
                self.pred_jump,
                dt,
                &self.obstacles,
            );
            let (y, vy) = (stepped.y, stepped.vy);
            // The predicted bump: the frame the clamp first names a block
            // while I was rising is the bonk, felt now; the server's answer
            // (the pop, or nothing because someone was 30 ms earlier)
            // follows one round trip later. Requiring the pre-step vy > 0
            // is the same belt-and-braces the sim applies.
            // The edge alone fires twice on most bonks: the reconciliation
            // replay rewinds the arc to below the block, the edge clears
            // on the frame between, and the clamp names the block again.
            // So a bonk is also once per block per `BONK_DEBOUNCE`.
            let bonk = stepped
                .bonked
                .filter(|&k| self.prev_bonked != Some(k) && self.pred_vy > 0.0)
                .filter(|&k| self.obstacles.get(k).is_some_and(|o| o.kind == Cover::Loot))
                .and_then(|k| self.loot_slot(k))
                .filter(|&i| bonk_is_new(self.last_bonk_at.get(i).copied().flatten(), self.time));
            self.prev_bonked = stepped.bonked;
            if let Some(i) = bonk {
                if let Some(last) = self.last_bonk_at.get_mut(i) {
                    *last = Some(self.time);
                }
                let mut cues = Vec::new();
                if self.loot_active.get(i).copied().unwrap_or(true) {
                    self.loot_bump[i] = Some(self.time);
                    self.dip_started = Some(self.time);
                    self.cue(feel::bonk(), &mut cues);
                } else {
                    self.cue(feel::bonk_dead(), &mut cues);
                }
                if let Some(audio) = self.audio.as_ref() {
                    for p in cues {
                        audio.play(p.sfx, p.vol);
                    }
                }
            }
            // A launch is the only thing that can raise vy, so that is the
            // press being spent - exactly the one shot the server gets.
            if vy > self.pred_vy {
                self.pred_jump = false;
            }
            self.pred_y = y;
            self.pred_vy = vy;
            // My own boots. The speed is what I actually covered, not what
            // I asked for, so walking into a wall is silent; the phase is
            // the leg cycle, so my cadence is the one a listener beside me
            // hears off my legs, and it rises when I sprint because the
            // cycle is per metre. Crouch, the air and death are the pure
            // rule's, in `feel::footstep`, so both peers apply them.
            let moved = if dt > 0.0 {
                (self.pred_pos - was) / dt
            } else {
                Vec2::ZERO
            };
            let prev_phase = self.own_anim.1;
            ember_engine::puppet::advance_anim(&mut self.own_anim, moved, dt);
            let mine = feel::Stepper {
                who: self.my_id.unwrap_or(0),
                alive: true,
                crouch,
                vy,
                speed: moved.length(),
                prev_phase,
                phase: self.own_anim.1,
                since_last: self.steps.own.map_or(f32::INFINITY, |t| self.time - t),
            };
            if let Some((s, v)) = feel::footstep(&mine, 0.0, true) {
                sfx.push(Play::centre(s, v));
                self.steps.own = Some(self.time);
                steps_queued += 1;
            }
        }
        // Tight smoothing absorbs reconciliation nudges without adding lag.
        let k = 1.0 - (-dt * 25.0).exp();
        self.own_render += (self.pred_pos - self.own_render) * k;
        // The eye height gets exactly what x and z already get. `pred_y`
        // stays the simulation state - the smoothed value must never feed
        // back into step_vertical, or the physics chases its own tail.
        self.render_y_own += (self.pred_y - self.render_y_own) * k;
        if self.since_ping > 4.0 {
            self.since_ping = 0.0;
            self.chan.send(&C2S::Ping { nonce: 1 });
        }
        if let Some(line) = status_event {
            self.since_status = 0.0;
            set_status(&format!("{line}   |   {}", self.scoreboard()));
        } else if self.since_status > 1.0 && self.my_id.is_some() && !self.lost {
            self.since_status = 0.0;
            set_status(&self.scoreboard());
        }

        // ---- Tab scoreboard overlay ----
        // Forced on through the pause after a round: the round's result
        // is the one moment the whole table matters, and nobody should
        // have to find Tab to see it.
        self.since_score_ui += dt;
        // Gated like every other device read. The harness raises the client
        // windows over the operator's work, so a click of theirs can focus
        // one; from that moment their Tab (alt-tab, a shell completion)
        // would flip the scoreboard overlay into our pictures. It cannot
        // move the player, so this corrupts a capture rather than stealing
        // the machine — but "the client reads no device" has to be true
        // without an asterisk, or the next reader will not believe the rest.
        let tab = tick.as_ref().map_or_else(
            || input.down(KeyCode::Tab) || pad_down(PadButton::Start),
            |_| false,
        ) || self.round_pause > 0.0;
        if tab && self.my_id.is_some() {
            if self.since_score_ui > 0.25 || !self.score_shown {
                self.since_score_ui = 0.0;
                set_scoreboard(Some(&self.scoreboard_text()));
                self.score_shown = true;
            }
        } else if self.score_shown {
            set_scoreboard(None);
            self.score_shown = false;
        }

        // Pace interpolation off the measured gap, with a 15% margin so a
        // slightly late state finds us still short of the target rather than
        // parked on it. Reaching 1.0 early is a dead stop on screen, and the
        // clamp used to do that for 15% of wall-clock time.
        self.t = (self.t + dt / (self.state_interval * 1.15).max(1e-3)).min(1.0);

        // Shot-feel timers, particles, the climb and the shake.
        self.hitmarker_t = (self.hitmarker_t - dt).max(0.0);
        self.kill_t = (self.kill_t - dt).max(0.0);
        self.flash.retain(|_, t| {
            *t -= dt;
            *t > 0.0
        });
        // The v20 leftovers: streaks that have faded, flashes that are
        // over, marks past twenty seconds, casings past their time. A
        // casing falls until it lands and then lies there. `now` is read
        // before the particles so that a held-back plume and the star it
        // waits for are tested against the one clock.
        let now = self.time;
        self.fx.retain_mut(|f| {
            // Held back (the muzzle plume, waiting out its flash): not
            // born yet, so it neither moves nor ages nor is drawn.
            if f.born > now {
                return true;
            }
            f.vel.y -= f.gravity * dt;
            f.pos += f.vel * dt;
            f.ttl -= dt;
            f.ttl > 0.0
        });
        self.tracers.retain(|t| t.alive(now));
        self.flashes.retain(|f| f.until > now);
        feel::expire_marks(&mut self.marks, now);
        self.casings.retain_mut(|c| {
            if c.pos.y > c.land_y {
                c.vel.y -= feel::CASING_GRAVITY * dt;
                c.pos += c.vel * dt;
                if c.pos.y <= c.land_y {
                    c.pos.y = c.land_y;
                    c.vel = Vec3::ZERO;
                }
            }
            now - c.born < feel::CASING_SECS
        });
        self.pops
            .retain(|p| self.time - p.started < POP_SECS && p.slot < self.loot_index.len());
        self.climb.decay(dt);
        self.shake.decay(dt);

        // ---- first-person camera at my PREDICTED position ----
        // The camera reads the player's own yaw and pitch plus the recoil
        // kick, the full-auto climb and the shake. All of it is cosmetic:
        // the Input sent above carried `self.pitch` and `self.aim`, so the
        // server's aim never moves with the kick.
        let cooldown = weapon_stats(my_weapon).cooldown;
        let recoil = self
            .shot_started
            .map_or(0.0, |t0| my_feel.recoil((self.time - t0) / cooldown));
        let kick_side = feel::yaw_side(self.shots);
        let cam_yaw = self.yaw + my_feel.yaw_alt * kick_side * recoil;
        let cam_pitch = (self.pitch + my_feel.kick_cam * recoil + self.climb.value)
            .clamp(-MAX_PITCH - 0.1, MAX_PITCH + 0.1);
        let (cz, cx) = cam_yaw.sin_cos();
        let (ps, pc) = cam_pitch.sin_cos();
        let right3 = Vec3::new(-cz, 0.0, cx);
        let (shake_eye, shake_look) = self.shake.offsets(self.time, right3);
        let look = (Vec3::new(cx * pc, ps, cz * pc) + shake_look).normalize();
        // The bonk dip: the head just hit a box.
        let dip = self.dip_started.map_or(0.0, |t0| {
            let k = (self.time - t0) / 0.08;
            if k < 1.0 {
                0.06 * (k * std::f32::consts::PI).sin()
            } else {
                0.0
            }
        });
        // The eye rides the predicted feet height, so jumping and standing
        // on a crate raise the view.
        let my_pos = self.own_render;
        let eye = Vec3::new(
            my_pos.x,
            self.render_y_own + self.eye_h + bob - dip,
            my_pos.y,
        ) + shake_eye;
        let camera = debug_camera().unwrap_or_else(|| Camera {
            eye,
            target: eye + look,
            fov_y_deg: fov_now,
        });
        // Screen-space furniture (the hit markers, the native crosshair) is
        // a world-space cube a fixed distance ahead, so it grows as the view
        // narrows; scaled by the narrowing it keeps its size on screen, and
        // through the 20x scope it stays a marker rather than a wall.
        let view_scale = fov_now / feel::HIP_FOV;
        // The scope view: the sniper mostly zoomed. The held gun is not
        // drawn (at 0.6 m it would fill a 3.5 degree view) and the tube's
        // mask stands in front of the eye instead.
        let scoped = feel::scoped(my_weapon, self.zoom);

        // ---- build the scene ----
        let mut frame = Frame {
            camera,
            instances: Vec::with_capacity(160),
            // Golden hour over the city: a warm haze the sky cylinder reads
            // bright through, instead of the pre-v13 navy that turned the
            // panorama into night at 60 m. Post-tonemap light, per `Fog`.
            fog: ember_engine::Fog {
                color: [0.62, 0.50, 0.40],
                density: 0.006,
            },
        };
        let half = self.arena_half;
        let inst = |frame: &mut Frame, p: Vec3, s: Vec3, c: Vec3| {
            frame.instances.push(Instance::new(p, s, c));
        };

        // The city: sky cylinder and the far ground first, then the arena
        // floor and its boundary wall. env_base > 0: the armour picture for
        // box-body players; the props carry every other picture.
        let env = self.env_base;
        let props = self.props;
        if let Some(pr) = &props {
            pr.push_sky_and_ground(&mut frame);
        }
        // The floor slab: what the arena stands on, and what closes the gap
        // between the cobble plane and the far ground.
        inst(
            &mut frame,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(half * 2.0 + 2.0, 1.0, half * 2.0 + 2.0),
            Vec3::new(0.12, 0.13, 0.17),
        );
        if let Some(pr) = &props {
            frame.instances.push(
                Instance::new(
                    Vec3::new(0.0, 0.004, 0.0),
                    Vec3::new(half * 2.0 + 2.0, 1.0, half * 2.0 + 2.0),
                    Vec3::ONE,
                )
                .with_mesh(pr.mesh(Prop::Floor)),
            );
        }
        for (px, pz, sx, sz) in [
            (half + 0.45, 0.0, 0.9, half * 2.0 + 2.7),
            (-half - 0.45, 0.0, 0.9, half * 2.0 + 2.7),
            (0.0, half + 0.45, half * 2.0 + 2.7, 0.9),
            (0.0, -half - 0.45, half * 2.0 + 2.7, 0.9),
        ] {
            if let Some(pr) = &props {
                // The balustrade picture, one tile per wall height; the
                // fit turns the east and west walls so the long faces
                // carry it whichever way the wall runs.
                pr.push_fitted(
                    &mut frame,
                    Prop::CityWall,
                    Vec3::new(px, 1.75, pz),
                    Vec3::new(sx, 3.5, sz),
                    Vec3::ONE,
                );
            } else {
                inst(
                    &mut frame,
                    Vec3::new(px, 1.75, pz),
                    Vec3::new(sx, 3.5, sz),
                    Vec3::new(0.26, 0.28, 0.34),
                );
            }
        }
        // The listed decor: statue, cathedral, the façade ring, lamps and
        // wrecks, each scaled to the height the level gives it.
        if let Some(pr) = &props {
            for d in &self.decor {
                pr.push_decor(&mut frame, d);
            }
        }
        // Weapon-upgrade pads: base slab always, a spinning pickup while
        // active (positions are seeded, availability comes from State).
        for (i, pad) in self.pads_pos.iter().enumerate() {
            let active = self.pads_active.get(i).copied().unwrap_or(false);
            inst(
                &mut frame,
                Vec3::new(pad[0], 0.06, pad[1]),
                Vec3::new(1.9, 0.12, 1.9),
                if active {
                    Vec3::new(0.16, 0.30, 0.42)
                } else {
                    Vec3::new(0.15, 0.16, 0.20)
                },
            );
            if active {
                let hover = 1.0 + (self.time * 2.0).sin() * 0.15;
                frame.instances.push(
                    Instance::new(
                        Vec3::new(pad[0], hover, pad[1]),
                        Vec3::splat(0.5),
                        Vec3::new(0.55, 0.85, 1.0),
                    )
                    .with_yaw(self.time * 2.2),
                );
            }
        }

        // The hill, in king of the hill only: four thin bars along its
        // footprint and a marker cube high over its centre, white while
        // free, the king's colour while held, pulsing orange while
        // contested, so the state of the hill is read from anywhere on
        // the map without a line of text.
        if self.mode == GameMode::Hill
            && let Some(h) = &self.hill
        {
            let holder_color =
                (self.hill_holder < HILL_CONTESTED).then(|| self.player_color(self.hill_holder));
            let color = feel::hill_color(self.hill_holder, holder_color, self.time);
            for (centre, size) in feel::hill_bars(h) {
                inst(&mut frame, centre, size, color);
            }
            let (centre, size) = feel::hill_marker(h);
            inst(&mut frame, centre, size, color);
        }

        // Cover, drawn by kind: the same boxes prediction resolves against,
        // so what you see is what stops you. A raised box (a tunnel roof)
        // is drawn from its base, not from the floor.
        for o in &self.obstacles {
            if o.kind == Cover::Loot {
                // Drawn below with their own state.
                continue;
            }
            if let Some(pr) = &props {
                pr.push_obstacle(&mut frame, o);
            } else {
                let height = o.h - o.base;
                let pos = Vec3::new(
                    f32::midpoint(o.min[0], o.max[0]),
                    o.base + height * 0.5,
                    f32::midpoint(o.min[1], o.max[1]),
                );
                let size = Vec3::new(o.max[0] - o.min[0], height, o.max[1] - o.min[1]);
                inst(&mut frame, pos, size, Vec3::new(0.30, 0.33, 0.40));
            }
        }

        // The `?` blocks: armed ones turn slowly and bob a little, a bonked
        // one jumps a quarter metre and settles, a spent one is the same
        // mesh tinted dark and still. The box the sim resolves against
        // never moves; the bump is drawn, not simulated.
        for (i, &k) in self.loot_index.iter().enumerate() {
            let Some(o) = self.obstacles.get(k) else {
                continue;
            };
            let armed = self.loot_active.get(i).copied().unwrap_or(true);
            let bump = self.loot_bump.get(i).copied().flatten().map_or(0.0, |t0| {
                let t = self.time - t0;
                if t < BUMP_SECS {
                    0.25 * (t * std::f32::consts::PI / BUMP_SECS).sin()
                } else {
                    0.0
                }
            });
            let (yaw, bob_y, color) = if armed {
                (self.time * 0.6, (self.time * 1.5).sin() * 0.02, Vec3::ONE)
            } else {
                (0.0, 0.0, Vec3::splat(LOOT_SPENT_TINT))
            };
            if let Some(pr) = &props {
                pr.push_loot(&mut frame, o, bump + bob_y, yaw, color);
            } else {
                let extent = Vec3::new(o.max[0] - o.min[0], o.h - o.base, o.max[1] - o.min[1]);
                inst(
                    &mut frame,
                    Vec3::new(
                        f32::midpoint(o.min[0], o.max[0]),
                        f32::midpoint(o.base, o.h) + bump + bob_y,
                        f32::midpoint(o.min[1], o.max[1]),
                    ),
                    extent,
                    color * Vec3::new(0.85, 0.65, 0.2),
                );
            }
        }
        // The pop: the granted gun rises out of the block's top, spinning
        // two turns, and is gone half a second later.
        for p in &self.pops {
            let Some(o) = self
                .loot_index
                .get(p.slot)
                .and_then(|&k| self.obstacles.get(k))
            else {
                continue;
            };
            let t = ((self.time - p.started) / POP_SECS).clamp(0.0, 1.0);
            let pos = Vec3::new(
                f32::midpoint(o.min[0], o.max[0]),
                o.h + 0.6 * t,
                f32::midpoint(o.min[1], o.max[1]),
            );
            let rot = Quat::from_rotation_y(t * 2.0 * std::f32::consts::TAU);
            if let Some(a) = &self.assets {
                push_weapon(&mut frame, a, p.weapon, pos, rot, Action::REST, true);
            } else {
                let fwd = rot * Vec3::X;
                push_gun(
                    &mut frame,
                    pos,
                    Vec2::new(fwd.x, fwd.z),
                    weapon_accent(p.weapon),
                );
            }
        }

        // Other players (my own body is the camera).
        for (&id, p) in &self.latest {
            if !p.alive || Some(id) == self.my_id {
                continue;
            }
            let pos = self.render_pos(id);
            let feet_y = self.render_y(id);
            let color = self.player_color(id);
            let aim = Vec2::new(p.ax, p.az);
            let (body_h, head_y, hand_y, pip_y) = body_heights(p.crouch);
            // Hitbox truth ring: a flat plate exactly the server's hit
            // circle footprint (radius PLAYER_R) — what you aim at is real.
            let flash = self.flash.get(&id).copied().unwrap_or(0.0) / 0.18;
            frame.instances.push(Instance::new(
                Vec3::new(pos.x, feet_y + 0.02, pos.y),
                Vec3::new(1.2, 0.04, 1.2),
                color * (0.45 + flash * 0.5),
            ));
            // Damage flash: victim blinks toward white.
            let fc = Vec3::new(
                color.x + (1.0 - color.x) * flash,
                color.y + (1.0 - color.y) * flash,
                color.z + (1.0 - color.z) * flash,
            );
            // The gait, ahead of the pose: the walk phase the legs are
            // posed from is also the clock this body's footsteps land on,
            // so a step is heard exactly when a boot reaches the floor and
            // a sprinter's arrive faster because the cycle is per metre.
            // Advanced whether or not the rig is loaded, so a lobby drawn
            // as boxes still has feet.
            let prev = self.prev_pos.insert(id, pos).unwrap_or(pos);
            let vel = if dt > 0.0 {
                (pos - prev) / dt
            } else {
                Vec2::ZERO
            };
            let slot = self.anim.entry(id).or_insert((0.0, 0.0, 0.0));
            let prev_phase = slot.1;
            ember_engine::puppet::advance_anim(slot, vel, dt);
            let (walk_phase, walk_amp) = (slot.1, slot.2);
            if steps_queued < feel::STEP_CAP {
                let at = Vec3::new(pos.x, feet_y, pos.y);
                let last = self.steps.body.get(&id).copied();
                let stepper = feel::Stepper {
                    who: id,
                    alive: p.alive,
                    crouch: p.crouch,
                    vy: p.vy,
                    speed: vel.length(),
                    prev_phase,
                    phase: walk_phase,
                    since_last: last.map_or(f32::INFINITY, |t| self.time - t),
                };
                if let Some((s, v)) = feel::footstep(&stepper, (at - ear.at).length(), false) {
                    sfx.push(Play::spatial(s, v, at, ear.at, ear.right));
                    self.steps.body.insert(id, self.time);
                    steps_queued += 1;
                }
            }
            // Jointed rig when parts loaded; textured/plain boxes else.
            if let Some(rc) = &self.rig_character {
                let crouch = self.crouch_ease.entry(id).or_insert(0.0);
                let target = if p.crouch { 1.0 } else { 0.0 };
                *crouch += (target - *crouch) * (1.0 - (-10.0 * dt).exp());
                // Bodies face where the player AIMS (shooter convention).
                let aim_yaw = aim.x.atan2(aim.y);
                let pose = ember_engine::rig::walk_pose(
                    walk_phase, walk_amp, *crouch, self.time, &rc.dims,
                );
                ember_engine::rig::push_rig(
                    &mut frame,
                    &rc.parts,
                    &rc.skel,
                    &pose,
                    pos,
                    feet_y,
                    aim_yaw,
                    [fc.x, fc.y, fc.z],
                    0.95,
                );
            } else if env > 0 {
                frame.instances.push(
                    Instance::new(
                        Vec3::new(pos.x, feet_y + body_h * 0.5, pos.y),
                        Vec3::new(1.0, body_h, 1.0),
                        color,
                    )
                    .with_mesh(env),
                );
                frame.instances.push(
                    Instance::new(
                        Vec3::new(pos.x, feet_y + head_y, pos.y),
                        Vec3::splat(0.55),
                        color * 0.7,
                    )
                    .with_mesh(env),
                );
            } else {
                inst(
                    &mut frame,
                    Vec3::new(pos.x, feet_y + body_h * 0.5, pos.y),
                    Vec3::new(1.0, body_h, 1.0),
                    color,
                );
                inst(
                    &mut frame,
                    Vec3::new(pos.x, feet_y + head_y, pos.y),
                    Vec3::splat(0.55),
                    color * 0.7,
                );
            }
            // hand_y and pip_y are heights above the FEET, so both need this
            // player's own feet height added. Without it someone standing on
            // a crate carried their gun down at floor level.
            let hand = hand_at(pos, feet_y, aim, hand_y);
            let accent = weapon_accent(p.weapon);
            // The off-hand shield, on the side the pistol is not. The plate
            // is yawed only: a shield is carried upright whatever its owner
            // is looking at, and tilting it with pitch would swing its face
            // off the arc the sim actually protects.
            if p.shield {
                // Perpendicular to the aim in world XZ, matching the camera
                // right the first-person pose uses — so both views put the
                // shield on the same side of the body.
                let left = Vec2::new(aim.y, -aim.x);
                // Reaching as far forward as the gun hand does (0.55), and
                // for the same reason: the body box is 1.0 across, so
                // anything held closer than 0.5 is held INSIDE the torso
                // and the plate's face never shows.
                let center = Vec3::new(pos.x, feet_y + hand_y + 0.20, pos.y)
                    + Vec3::new(aim.x, 0.0, aim.y) * 0.52
                    + Vec3::new(left.x, 0.0, left.y) * 0.34;
                let rot = Quat::from_rotation_y(-aim.y.atan2(aim.x));
                // The scutum's origin is its handle, behind the boss, so it
                // hangs on the same centre the box plate was given.
                match self.assets.as_ref().filter(|a| !a.shield.is_empty()) {
                    Some(a) => {
                        push_parts(&mut frame, &a.shield, center, rot, fc, Action::REST);
                    }
                    None => push_shield(&mut frame, center, rot, SHIELD_PLATE, fc * 0.85),
                }
            }
            if let Some(a) = &self.assets {
                let yaw = -aim.y.atan2(aim.x);
                // Remote weapons are drawn by id and tilt with the owner's
                // real aim elevation, so a player shooting down off a
                // container looks like it; the rocket rides the tube only
                // while there is one to fire.
                push_weapon(
                    &mut frame,
                    a,
                    shown_weapon(p.weapon),
                    hand,
                    weapon_rot(yaw, p.pitch),
                    Action::REST,
                    p.ammo > 0 && !p.reloading,
                );
            } else {
                push_gun(&mut frame, hand, aim, accent);
            }
            // Hp pips: green, or the team's colour in team deathmatch so
            // the pips over a head say whose head it is before the body
            // reads.
            let pip = if self.mode == GameMode::Tdm {
                color
            } else {
                Vec3::new(0.3, 0.9, 0.4)
            };
            for h in 0..p.hp {
                inst(
                    &mut frame,
                    Vec3::new(pos.x - 0.3 + f32::from(h) * 0.3, feet_y + pip_y, pos.y),
                    Vec3::splat(0.16),
                    pip,
                );
            }
        }

        // Impact marks, tracers, particles and remote flashes (v20) all
        // wear the round meshes; without them registered (a frame built by
        // a test that did not ask) none is drawn, since the cubes they
        // replaced are the look the operator sent back.
        if let Some(rs) = self.rounds {
            // Sparks, shards and smoke: opaque balls (`rounds::puff`) at
            // `feel::PUFF_BALL` of the size that used to be a cube's edge,
            // which is the only particle the scene pass can draw. Shards
            // shrink out; smoke swells and dims. No yaw any more: a ball
            // turned about its centre is the same ball, so the spin the
            // cubes needed to look less like boxes is work no pixel could
            // show. A puff still waiting out a flash is not drawn.
            for f in self.fx.iter().filter(|f| f.born <= self.time) {
                let k = (f.ttl / f.life).clamp(0.0, 1.0);
                let (edge, dim) = feel::puff_draw(f.gravity, k);
                frame.instances.push(
                    Instance::new(
                        f.pos,
                        Vec3::splat(f.size * edge * feel::PUFF_BALL),
                        f.color * dim,
                    )
                    .with_mesh(rs.puff()),
                );
            }
            // Impact marks: near-black holes on the faces rounds hit, each
            // the width its round makes (`Mark::diameter`), drawn after the
            // cover so the depth test lays them on it; the disc is sunk
            // into the face with 1 mm proud (`Mark::placement`), so it
            // never fights the face for the pixel and never reads as a
            // puck stuck on the wall at a grazing angle.
            for m in &self.marks {
                let (pos, scale, rot) = m.placement();
                frame.instances.push(
                    Instance::new(pos, scale, feel::MARK_COLOR)
                        .with_rot(rot)
                        .with_mesh(rs.disc()),
                );
            }
            // Tracers from shot events: the round itself at the head,
            // replayed along the segment at the weapon's speed and drawn
            // at `ROUND_SCALE` times its calibre, with a bright streak
            // behind it and a dimmer one behind that (`push_streak`), both
            // thinning out over the last 120 ms. Through the linger only
            // the streak remains.
            for t in &self.tracers {
                let Some(round) = rounds::round_for(t.weapon) else {
                    continue;
                };
                let dir = t.dir();
                let head = t.head(self.time);
                let rods = t.rods(self.time);
                let drawn = Drawn {
                    round,
                    scale: rounds::ROUND_SCALE,
                };
                // The streak runs back to the muzzle the client drew; the
                // round itself flies along the server's segment. The two
                // directions differ by the width of a shooter's body over
                // the first metre or so and by nothing after that.
                push_streak(
                    &mut frame,
                    rs,
                    drawn,
                    head,
                    t.streak_dir(self.time),
                    &rods,
                    t.fade(self.time),
                );
                if t.flying(self.time) {
                    push_round(&mut frame, rs, drawn, head, dir);
                }
            }
            // Remote muzzle flashes, from the same events: a star along
            // the shot.
            for f in &self.flashes {
                push_flash(&mut frame, rs, f.pos, f.dir, f.size);
            }
        }
        // My casings, tumbling while they fly and still once they land.
        for c in &self.casings {
            let age = self.time - c.born;
            let rot = if c.vel == Vec3::ZERO {
                Quat::from_rotation_y(c.born * 7.0)
            } else {
                Quat::from_rotation_y(age * 25.0) * Quat::from_rotation_x(age * 18.0)
            };
            frame
                .instances
                .push(Instance::new(c.pos, feel::CASING_SIZE, feel::CASING_COLOR).with_rot(rot));
        }

        // The rocket, from the state: the mesh flown along the server's
        // real 3D path with an exhaust rod behind it, extrapolation bounded
        // to ~2 state intervals so stalls don't fly it through walls. A
        // bullet in a state is skipped: since v20 it is drawn as a round
        // and a streak from its `Shot` event, and drawing it here as well
        // would show the same round twice. So what reaches the body of the
        // loop is a rocket, the one projectile kind that does not trace.
        let age = self.bullets_age.min(0.12);
        for b in &self.bullets {
            if feel::traces(b.weapon) {
                continue;
            }
            let at = Vec3::new(b.x + b.vx * age, b.y + b.vy * age, b.z + b.vz * age);
            let vel = Vec3::new(b.vx, b.vy, b.vz);
            let speed = vel.length();
            if speed < 1e-3 {
                continue;
            }
            let dir = vel / speed;
            // Scale is applied before rotation, so a box long in X becomes
            // a rod pointing along the flight direction.
            let rot = Quat::from_rotation_arc(Vec3::X, dir);
            let row = weapon_feel(b.weapon);
            // The trail is clamped so it cannot reach back through the
            // camera. Your own round is only ~0.77 from the eye on the
            // first state that carries it, so a fixed tail would end up
            // inside the 0.1 near plane — and the scene pass does not cull
            // backfaces, so it would paint a solid block across the middle
            // of the screen, right over the crosshair.
            let back = row.tracer_len.min(((at - eye).dot(dir) - 0.35).max(0.0));
            if back > 0.02 {
                frame.instances.push(
                    Instance::new(
                        at - dir * (back * 0.5),
                        Vec3::new(back, row.tracer_thick, row.tracer_thick),
                        row.tracer,
                    )
                    .with_rot(rot),
                );
            }
            // The rocket is a body, not a rod: `from_rotation_arc` rolls
            // it with its heading, so the same mesh reads differently on
            // every bearing. The held launcher's own convention (a yaw,
            // then an elevation) is roll-stable.
            let rocket_rot = weapon_rot(-dir.z.atan2(dir.x), dir.y.asin());
            match self.assets.as_ref().filter(|a| !a.rocket.is_empty()) {
                Some(a) => {
                    // The mesh lives in the launcher's frame, so drawing it
                    // at the bullet position would put the launcher's
                    // origin (the grip) on the server's path and the bore a
                    // hand's width off it. Shift it so the rocket's own
                    // centre, on the bore line, sits on the path.
                    let bore = a.muzzles[slot(7)];
                    let centre = Vec3::new(ROCKET_CENTRE_X, bore.y, bore.z);
                    push_parts(
                        &mut frame,
                        &a.rocket,
                        at - rocket_rot * centre,
                        rocket_rot,
                        row.accent,
                        Action::REST,
                    );
                }
                None => frame.instances.push(
                    Instance::new(at, Vec3::new(0.5, 0.16, 0.16), Vec3::new(0.35, 0.4, 0.3))
                        .with_rot(rocket_rot),
                ),
            }
        }

        // ---- crosshair markers: hit (white X) and kill (red X, larger) ----
        // In the camera's own basis (the right, and the right crossed
        // with the look), the same as the native crosshair, so the X keeps
        // its shape on a pitched aim instead of foreshortening against a
        // "+" that does not.
        if self.hitmarker_t > 0.0 || self.kill_t > 0.0 {
            let up = right3.cross(look).normalize();
            let center = eye + look * 1.2;
            let (off, edge, col) = if self.kill_t > 0.0 {
                (
                    0.045 * view_scale,
                    0.016 * view_scale,
                    Vec3::new(1.0, 0.15, 0.1),
                )
            } else {
                (
                    0.028 * self.hitmarker_scale * view_scale,
                    0.009 * self.hitmarker_scale * view_scale,
                    Vec3::new(1.0, 1.0, 1.0),
                )
            };
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                frame.instances.push(Instance::new(
                    center + right3 * (off * sx) + up * (off * sy),
                    Vec3::splat(edge),
                    col,
                ));
            }
        }

        // ---- viewmodel: the held gun by id, plus muzzle flash ----
        if me_alive {
            let reloading = me_latest.is_some_and(|p| p.reloading);
            let loaded = me_latest.is_some_and(|p| p.ammo > 0 && !p.reloading);
            let accent = weapon_accent(my_weapon);

            // Reload animation: the gun dips and rolls out of the way, over
            // this weapon's own reload time.
            let reload_dip = if reloading {
                let t0 = self.reload_started.unwrap_or(self.time);
                let progress = ((self.time - t0) / weapon_stats(my_weapon).reload).clamp(0.0, 1.0);
                (progress * std::f32::consts::PI).sin() * 0.24
            } else {
                0.0
            };
            // The holster: a looted gun ran dry, the model drops 0.3 m and
            // the sidearm comes back up over 0.35 s.
            let holster_drop = self.holster_started.map_or(0.0, |t0| {
                let k = (self.time - t0) / 0.35;
                if k < 1.0 {
                    0.3 * (k * std::f32::consts::PI).sin()
                } else {
                    0.0
                }
            });
            // Recoil across the weapon's own cooldown, shaped per weapon by
            // the feel table (`recoil` above, the same curve the camera
            // reads): the model kicks far more than the view and slides
            // back by `push`. Driven by the server-confirmed shot rather
            // than the trigger — holding fire on an empty magazine, or
            // during a reload, must not kick.
            // ADS rides the FULL look vector rather than its horizontal
            // part. That is what puts the sights on the shot line when
            // pitched; the old pose used horizontal forward plus a
            // `pitch * 0.10` nudge, so the gun and the bullet disagreed.
            // Offsets tuned for the v16 rifle, whose hold point is the top of
            // the pistol grip at the trigger: every weapon is fitted to that
            // same hold by the build, so the one set of offsets serves all.
            let base = eye
                + look * (0.60 + 0.08 * self.zoom - 0.06 * recoil - my_feel.push * recoil)
                + right3 * (0.20 * (1.0 - self.zoom) + 0.012)
                + Vec3::Y
                    * (-0.24 + 0.07 * self.zoom + bob * 0.4 * (1.0 - self.zoom)
                        - reload_dip
                        - holster_drop
                        + 0.03 * recoil);
            let yaw = -forward2.y.atan2(forward2.x);
            // Tilts with aim elevation, plus the muzzle-up kick and the
            // alternating sideways kick per shot.
            let kick_yaw = yaw + my_feel.yaw_alt * kick_side * recoil;
            let kick_pitch = self.pitch + my_feel.kick_model * recoil;
            let rot = weapon_rot(kick_yaw, kick_pitch);
            // The melee drops the rifle out of the frame, in the weapon's
            // own frame, and the sword comes out in its place.
            let melee_since = self.melee_started.map(|t0| self.time - t0);
            let (base, rot) = match melee_since.and_then(|since| melee_pose(&LOWER, since)) {
                Some((offset, yaw_add, pitch_add, _roll)) => (
                    base + look * offset.x - right3 * offset.y + Vec3::Y * offset.z,
                    weapon_rot(kick_yaw + yaw_add, kick_pitch + pitch_add),
                ),
                None => (base, rot),
            };
            // The moving parts read the same confirmed shot the recoil
            // does, so a dry trigger moves nothing.
            let action = Action {
                cycle: self
                    .shot_started
                    .map_or(1.0, |t0| ((self.time - t0) / cooldown).clamp(0.0, 1.0)),
                shots: self.shots,
            };
            let muzzle = if scoped {
                // Nothing of the gun is drawn through the scope; the flash
                // below reads `scoped` too, and the muzzle only anchors the
                // rocket's smoke, which is not a sniper's.
                base + look * 0.95
            } else if let Some(a) = &self.assets {
                push_weapon(&mut frame, a, my_weapon, base, rot, action, loaded);
                push_parts(&mut frame, &a.arms, base, rot, accent, action);
                base + rot * a.muzzle_of(my_weapon)
            } else {
                push_gun(&mut frame, base, forward2, accent);
                base + look * 0.95
            };
            // My own streaks start at this muzzle, not at the sim's launch
            // point: the segment's end is the server's, its start is where
            // the gun is drawn. Unlike a remote shot, the whole line is
            // moved rather than the streak alone — the viewmodel's muzzle
            // travels with the view, and a round of mine drawn on the
            // server's line would swim beside my own barrel as I turn.
            for p in std::mem::take(&mut self.pending_shots) {
                self.tracers.push(Tracer {
                    from: muzzle,
                    muzzle,
                    to: p.to,
                    weapon: p.weapon,
                    born: self.time,
                });
            }
            // The plume and the casing of a confirmed shot of mine, at the
            // muzzle. The casing is thrown right and up out of the port
            // and lands on whatever my feet are on; its tink is queued now,
            // late by the fall, and only off the floor (a crate top is not
            // cobbles).
            if std::mem::take(&mut self.own_plume) {
                self.puffs(feel::plume(muzzle, look, my_weapon));
                if feel::traces(my_weapon) {
                    let (pos, vel) = feel::casing_eject(muzzle, right3, look);
                    let land_y = self.pred_y;
                    self.casings.push(Casing {
                        pos,
                        vel,
                        land_y,
                        born: self.time,
                    });
                    if land_y < 0.05 {
                        sfx.push(Play {
                            sfx: Sfx::Casing,
                            vol: feel::CASING_VOLUME,
                            pan: 0.0,
                            delay: feel::fall_secs(pos.y - land_y, vel.y),
                        });
                    }
                }
            }
            // The rocket's launch smoke: six grey balls drifting back from
            // the muzzle, spawned on the frame the shot was confirmed and
            // held back by the launch flash like the plume, so the star
            // has the first frames of the launch to itself.
            if std::mem::take(&mut self.launch_smoke) {
                for k in 0_u8..6 {
                    let a = f32::from(k) * std::f32::consts::TAU / 6.0;
                    self.fx.push(Fx {
                        pos: muzzle + right3 * (a.cos() * 0.12) + Vec3::Y * (a.sin() * 0.12),
                        vel: -look * 1.6
                            + right3 * (a.cos() * 0.5)
                            + Vec3::Y * (0.6 + a.sin() * 0.4),
                        ttl: 0.4,
                        life: 0.4,
                        size: 0.22,
                        color: Vec3::new(0.45, 0.43, 0.40),
                        gravity: 0.0,
                        born: now + feel::plume_delay_of(my_weapon),
                    });
                }
            }
            // The sniper's scope: an opaque near-black mask 0.30 m from the
            // eye with a round hole, and a reticle across the hole. Opaque
            // because that is the one kind of cube there is; the scene pass
            // has no blending, so the tube is 24 overlapping slabs round a
            // 24-gon, and the world outside the hole is simply hidden
            // behind them. The geometry is `feel::scope_mask`, sized from
            // the current field of view so the hole is the same fraction of
            // the screen while the zoom is still easing in, and closed at
            // every aspect ratio without reading the window's.
            if scoped {
                // The mask's basis, built outright: `from_rotation_arc`
                // picks a roll of its own for a pitched look, and the frame
                // then turned on its axis as the aim rose. Up is what is
                // perpendicular to both the horizontal right and the look,
                // and the right is re-derived from those two so the basis
                // stays orthonormal when the shake tilts the look off the
                // yaw plane.
                let up = right3.cross(look).normalize();
                let mask_right = look.cross(up);
                let plane = |v: Vec2| mask_right * v.x + up * v.y;
                let c = eye + look * feel::SCOPE_DIST;
                let (a, slabs) = feel::scope_mask(fov_now);
                for s in slabs {
                    let t3 = plane(s.tangent);
                    let n3 = plane(s.normal);
                    // Scale before rotation: x along the tangent, y along
                    // the outward normal, z along the look; the third
                    // column is the cross product so the basis is
                    // right-handed whichever way the tangent runs.
                    let rot = Quat::from_mat3(&Mat3::from_cols(t3, n3, t3.cross(n3)));
                    frame.instances.push(
                        Instance::new(
                            c + plane(s.center),
                            Vec3::new(2.0 * s.half_len, 2.0 * s.half_thick, 0.01),
                            feel::SCOPE_BLACK,
                        )
                        .with_rot(rot),
                    );
                }
                // The reticle, a hair nearer than the slabs so its tips do
                // not fight them for the pixels at the polygon's edge.
                let rot = Quat::from_mat3(&Mat3::from_cols(mask_right, up, -look));
                for size in feel::scope_reticle(a) {
                    frame.instances.push(
                        Instance::new(c - look * 0.01, size, feel::SCOPE_BLACK).with_rot(rot),
                    );
                }
            }
            // Muzzle flash on a round the server agrees left the weapon.
            // What stood here fired on `time % cooldown` while the trigger
            // was held — a free-running clock with no relationship to
            // whether a bullet was ever spawned or ammo remained.
            // The Murasama, in the operator's own fist, on its own hold: the
            // eye rather than the rifle's base, so the reload dip and the
            // recoil do not ride along with a weapon that is not firing.
            if let Some(a) = self.assets.as_ref().filter(|a| !a.sword.is_empty())
                && let Some(since) = melee_since
                && let Some((offset, yaw_add, pitch_add, roll)) = melee_pose(&SLASH, since)
            {
                let sword_base = eye + look * offset.x - right3 * offset.y + Vec3::Y * offset.z;
                // Roll last, in the weapon's own frame: it turns the edge
                // into the direction of the cut.
                let sword_rot =
                    weapon_rot(yaw + yaw_add, self.pitch + pitch_add) * Quat::from_rotation_x(roll);
                push_parts(
                    &mut frame,
                    &a.sword,
                    sword_base,
                    sword_rot,
                    accent,
                    Action::REST,
                );
                push_parts(
                    &mut frame,
                    &a.fist,
                    sword_base,
                    sword_rot,
                    accent,
                    Action::REST,
                );
            }

            // The star along the look, which is where the plume goes and
            // where the round the server confirms will have gone.
            let flashing = self
                .shot_started
                .is_some_and(|t0| self.time - t0 < my_feel.flash_ms);
            if flashing
                && !scoped
                && let Some(rs) = self.rounds
            {
                push_flash(&mut frame, rs, muzzle, look, shot_flash(&my_feel));
            }
            // The crosshair. What stood here was an aim dot: an opaque
            // cube in the weapon's accent 4 m down the sight line, which
            // on the web sat as a coloured square under the page's own "+"
            // (`div#crosshair`) and down the sights was a block. The page
            // is the crosshair on the web; the native client, which has no
            // page, gets a hairline here. Not through the scope, which has
            // its own reticle.
            #[cfg(not(target_arch = "wasm32"))]
            if !scoped {
                push_crosshair(&mut frame, eye, look, right3, view_scale);
            }
            // The round showcase, a review aid: see `push_showcase`.
            #[cfg(not(target_arch = "wasm32"))]
            if debug_rounds()
                && !scoped
                && let Some(rs) = self.rounds
            {
                push_showcase(&mut frame, rs, eye, look, right3, self.time, fov_now);
            }

            // ---- the off-hand shield, in the hand the pistol is not ----
            // Drawn last so it is the newest instance, though the scene pass
            // is depth-tested and the order does not decide what wins.
            //
            // Lowered it hangs below the frame; raised it swings up and in,
            // taking a real bite out of the left of the view. That cost is
            // the point: the sim charges you your trigger and your sprint
            // for this, and it should be as obvious on screen as it is in
            // the rules. Distance from the eye is well past the 0.1 near
            // plane at both ends of the swing.
            if self.shield_raise > 0.01 {
                let k = self.shield_raise;
                let lerp = |lo: f32, hi: f32| lo + (hi - lo) * k;
                let center = eye + look * lerp(0.62, 0.74) - right3 * lerp(0.36, 0.26)
                    + Vec3::Y * (lerp(-0.66, -0.09) + bob * 0.4);
                let rot = weapon_rot(yaw, self.pitch);
                // The scutum rides the same swing the plate did; what the
                // box got from growing (lerp(0.9, 1.0)) the mesh gets from
                // the swing alone, since parts are drawn at unit scale.
                match self.assets.as_ref().filter(|a| !a.shield.is_empty()) {
                    Some(a) => push_parts(
                        &mut frame,
                        &a.shield,
                        center,
                        rot,
                        GUNMETAL * 1.6 + accent * 0.10,
                        Action::REST,
                    ),
                    None => push_shield(
                        &mut frame,
                        center,
                        rot,
                        SHIELD_PLATE * lerp(0.9, 1.0),
                        GUNMETAL * 1.6 + accent * 0.10,
                    ),
                }
            }
        }
        // A shot of mine that arrived while I was dead (the round outlived
        // me) starts where the sim says; there is no gun to start it from.
        for p in std::mem::take(&mut self.pending_shots) {
            self.tracers.push(Tracer {
                from: p.from,
                muzzle: p.from,
                to: p.to,
                weapon: p.weapon,
                born: self.time,
            });
        }
        // The plume of a shot confirmed on a frame with no viewmodel (dead
        // by the time the state arrived) has no muzzle: dropped.
        self.own_plume = false;

        // Play the queued cues under a per-frame budget, the important ones
        // first, so a crowded frame drops a footfall and never the boom.
        // After the render pass, because the casing's tink is queued there.
        if !suppress_sfx {
            feel::prioritize_plays(&mut sfx);
            let heard: Vec<Play> = sfx.into_iter().take(BUDGET).collect();
            if let Some(audio) = self.audio.as_ref() {
                for p in &heard {
                    audio.play_spatial(p.sfx, p.vol, p.pan, p.delay);
                }
            }
            // What this frame put out, for the tests: a client built by
            // `with_chan` has no audio device, so without this the only way
            // to check a cue is to trust the code that queued it.
            self.heard = heard;
        }

        frame
    }

    /// Everything the pad should feel since the last frame. The platform
    /// merges the requests per channel by max, so a hitmarker tick queued
    /// beside a death rumble never cancels it.
    fn feedback(&mut self) -> Feedback {
        std::mem::take(&mut self.feedback)
    }
}

// ---- platform-split WebSocket channel ----

#[cfg(not(target_arch = "wasm32"))]
mod net {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::Duration;

    use arena_core::proto::{C2S, S2C};
    use tungstenite::Message;
    use tungstenite::stream::MaybeTlsStream;

    pub struct NetChan {
        out_tx: Sender<C2S>,
        in_rx: Receiver<S2C>,
        dead: Arc<AtomicBool>,
    }

    impl NetChan {
        pub fn connect(url: &str, initial: &[C2S]) -> Result<Self, String> {
            // rustls needs an explicitly installed crypto provider (both
            // backends are compiled into the tree). Err = already installed.
            drop(rustls::crypto::ring::default_provider().install_default());
            let (mut ws, _) = tungstenite::connect(url).map_err(|e| format!("connect: {e}"))?;
            match ws.get_ref() {
                MaybeTlsStream::Plain(s) => {
                    drop(s.set_read_timeout(Some(Duration::from_millis(20))));
                }
                MaybeTlsStream::Rustls(s) => {
                    drop(
                        s.get_ref()
                            .set_read_timeout(Some(Duration::from_millis(20))),
                    );
                }
                _ => {}
            }
            for msg in initial {
                let text = serde_json::to_string(msg).map_err(|e| e.to_string())?;
                ws.send(Message::text(text))
                    .map_err(|e| format!("send: {e}"))?;
            }

            let (out_tx, out_rx) = mpsc::channel::<C2S>();
            let (in_tx, in_rx) = mpsc::channel::<S2C>();
            let dead = Arc::new(AtomicBool::new(false));
            {
                let dead = Arc::clone(&dead);
                std::thread::spawn(move || {
                    loop {
                        loop {
                            match out_rx.try_recv() {
                                Ok(msg) => {
                                    let Ok(text) = serde_json::to_string(&msg) else {
                                        continue;
                                    };
                                    if ws.send(Message::text(text)).is_err() {
                                        dead.store(true, Ordering::Relaxed);
                                        return;
                                    }
                                }
                                Err(mpsc::TryRecvError::Empty) => break,
                                Err(mpsc::TryRecvError::Disconnected) => {
                                    drop(ws.close(None));
                                    return;
                                }
                            }
                        }
                        match ws.read() {
                            Ok(Message::Text(t)) => {
                                if let Ok(msg) = serde_json::from_str::<S2C>(t.as_str())
                                    && in_tx.send(msg).is_err()
                                {
                                    return;
                                }
                            }
                            Ok(Message::Close(_)) => {
                                dead.store(true, Ordering::Relaxed);
                                return;
                            }
                            Ok(_) => {}
                            // os error 997 is Windows ERROR_IO_PENDING: the
                            // 20 ms read timeout above firing inside an
                            // overlapped read. Rust maps it to neither
                            // WouldBlock nor TimedOut, so without the raw
                            // check the arm below declared the channel dead
                            // on an ordinary quiet 20 ms and the client sat
                            // there showing "connection lost" while the
                            // server logged a plain disconnect. Measured on
                            // 2026-09-04: a capture's alpha dropped 7.5 s in
                            // with nothing in either log. Every other read
                            // loop in this workspace already carries this
                            // predicate (`arena-server/src/lib.rs`,
                            // `arena-server/examples/wsbot.rs`,
                            // `ember-client-net::transport`,
                            // `fire_core::proto::is_transient_read`); this
                            // one was the exception, so it is inlined here
                            // rather than depending on any of them.
                            Err(tungstenite::Error::Io(e))
                                if e.raw_os_error() == Some(997)
                                    || e.kind() == std::io::ErrorKind::WouldBlock
                                    || e.kind() == std::io::ErrorKind::TimedOut => {}
                            Err(_) => {
                                dead.store(true, Ordering::Relaxed);
                                return;
                            }
                        }
                    }
                });
            }
            Ok(Self {
                out_tx,
                in_rx,
                dead,
            })
        }

        pub fn send(&self, msg: &C2S) {
            drop(self.out_tx.send(clone_c2s(msg)));
        }

        pub fn poll(&self) -> Option<S2C> {
            self.in_rx.try_recv().ok()
        }

        pub fn is_dead(&self) -> bool {
            self.dead.load(Ordering::Relaxed)
        }
    }

    /// C2S is small; re-serialize instead of deriving Clone on the
    /// protocol type.
    fn clone_c2s(msg: &C2S) -> C2S {
        serde_json::from_str(&serde_json::to_string(msg).unwrap()).unwrap()
    }

    #[cfg(test)]
    impl NetChan {
        /// A channel with no socket behind it: what the game sends lands in
        /// the returned receiver and nothing ever arrives. Lets a test run
        /// `update` and read the exact `Input` frames it would have put on
        /// the wire.
        pub fn detached() -> (Self, Receiver<C2S>) {
            let (chan, _inbox, wire) = Self::detached_duplex();
            (chan, wire)
        }

        /// `detached`, plus the sender a test feeds server messages
        /// through: what a state does to the prediction is only observable
        /// by handing the game one.
        pub fn detached_duplex() -> (Self, Sender<S2C>, Receiver<C2S>) {
            let (out_tx, out_rx) = mpsc::channel::<C2S>();
            let (in_tx, in_rx) = mpsc::channel::<S2C>();
            (
                Self {
                    out_tx,
                    in_rx,
                    dead: Arc::new(AtomicBool::new(false)),
                },
                in_tx,
                out_rx,
            )
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod net {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use arena_core::proto::{C2S, CLIENT_PING_SECS, S2C};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    pub struct NetChan {
        ws: web_sys::WebSocket,
        inbox: Rc<RefCell<VecDeque<S2C>>>,
        open: Rc<Cell<bool>>,
        dead: Rc<Cell<bool>>,
        /// Messages queued until the socket opens.
        pending: Rc<RefCell<Vec<String>>>,
        _callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
        _on_msg: Closure<dyn FnMut(web_sys::MessageEvent)>,
        /// Handle of the keepalive timer, so it dies with the channel.
        keepalive_id: Option<i32>,
        _keepalive: Option<Closure<dyn FnMut()>>,
    }

    impl NetChan {
        pub fn connect(url: &str, initial: &[C2S]) -> Result<Self, String> {
            let ws = web_sys::WebSocket::new(url).map_err(|_| format!("bad url: {url}"))?;
            let inbox = Rc::new(RefCell::new(VecDeque::new()));
            let open = Rc::new(Cell::new(false));
            let dead = Rc::new(Cell::new(false));
            let pending: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(
                initial
                    .iter()
                    .map(|m| serde_json::to_string(m).unwrap())
                    .collect(),
            ));

            let on_msg = {
                let inbox = Rc::clone(&inbox);
                Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                    if let Some(text) = e.data().as_string() {
                        if let Ok(msg) = serde_json::from_str::<S2C>(&text) {
                            inbox.borrow_mut().push_back(msg);
                        }
                    }
                })
            };
            ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));

            let mut callbacks = Vec::new();
            {
                let open = Rc::clone(&open);
                let pending = Rc::clone(&pending);
                let ws2 = ws.clone();
                let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                    open.set(true);
                    for text in pending.borrow_mut().drain(..) {
                        drop(ws2.send_with_str(&text));
                    }
                });
                ws.set_onopen(Some(cb.as_ref().unchecked_ref()));
                callbacks.push(cb);
            }
            for setter in ["error", "close"] {
                let dead = Rc::clone(&dead);
                let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                    dead.set(true);
                });
                match setter {
                    "error" => ws.set_onerror(Some(cb.as_ref().unchecked_ref())),
                    _ => ws.set_onclose(Some(cb.as_ref().unchecked_ref())),
                }
                callbacks.push(cb);
            }

            // The keepalive runs on a timer, NOT on the frame loop. The game
            // pings from its update step, and a hidden browser tab gets no
            // requestAnimationFrame at all - so a backgrounded player went
            // completely silent and the server dropped them, closing their
            // lobby with them. Timers keep running when frames stop.
            let ping = serde_json::to_string(&C2S::Ping { nonce: 1 }).unwrap_or_default();
            let mut keepalive = None;
            let mut keepalive_id = None;
            if let Some(win) = web_sys::window() {
                let ws2 = ws.clone();
                let open2 = Rc::clone(&open);
                let dead2 = Rc::clone(&dead);
                let cb = Closure::<dyn FnMut()>::new(move || {
                    if open2.get() && !dead2.get() && ws2.send_with_str(&ping).is_err() {
                        dead2.set(true);
                    }
                });
                keepalive_id = win
                    .set_interval_with_callback_and_timeout_and_arguments_0(
                        cb.as_ref().unchecked_ref(),
                        (CLIENT_PING_SECS as i32) * 1000,
                    )
                    .ok();
                keepalive = Some(cb);
            }

            Ok(Self {
                ws,
                inbox,
                open,
                dead,
                pending,
                _callbacks: callbacks,
                _on_msg: on_msg,
                keepalive_id,
                _keepalive: keepalive,
            })
        }

        pub fn send(&mut self, msg: &C2S) {
            let Ok(text) = serde_json::to_string(msg) else {
                return;
            };
            if self.open.get() {
                if self.ws.send_with_str(&text).is_err() {
                    self.dead.set(true);
                }
            } else if !self.dead.get() {
                self.pending.borrow_mut().push(text);
            }
        }

        pub fn poll(&mut self) -> Option<S2C> {
            self.inbox.borrow_mut().pop_front()
        }

        pub fn is_dead(&self) -> bool {
            self.dead.get()
        }
    }

    impl Drop for NetChan {
        fn drop(&mut self) {
            if let (Some(win), Some(id)) = (web_sys::window(), self.keepalive_id) {
                win.clear_interval_with_handle(id);
            }
        }
    }
}

#[cfg(test)]
mod melee_tests {
    use super::*;

    fn keys_are_a_swing(keys: &[MeleeKey]) {
        for pair in keys.windows(2) {
            assert!(pair[1].0 > pair[0].0, "keys in time order");
        }
        assert!(
            (keys[keys.len() - 1].0 - MELEE_COOLDOWN).abs() < 1e-6,
            "the last key sits on the cooldown"
        );
    }

    fn is_continuous(keys: &[MeleeKey], limit: f32) {
        let mut prev = melee_pose(keys, 0.0).unwrap().0;
        let mut t = 1.0 / 60.0;
        while t < MELEE_COOLDOWN {
            let cur = melee_pose(keys, t).unwrap().0;
            let step = (cur - prev).length();
            assert!(step < limit, "jump of {step} at {t}");
            prev = cur;
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn neither_table_plays_outside_the_cooldown() {
        for keys in [&LOWER[..], &SLASH[..]] {
            assert!(
                melee_pose(keys, -0.01).is_none(),
                "nothing before the press"
            );
            assert!(
                melee_pose(keys, MELEE_COOLDOWN).is_none(),
                "nothing once the cooldown is over"
            );
            keys_are_a_swing(keys);
        }
    }

    #[test]
    fn the_rifle_leaves_the_frame_and_comes_back_to_the_hold() {
        let (start, yaw, pitch, _) = melee_pose(&LOWER, 0.0).unwrap();
        assert!(
            start.length() < 1e-6 && yaw.abs() < 1e-6 && pitch.abs() < 1e-6,
            "starts at the hold"
        );
        let (down, ..) = melee_pose(&LOWER, 0.2).unwrap();
        assert!(down.z < -0.4, "well below the hold mid-swing: {down}");
        assert!(down.y < -0.2, "and out to the right: {down}");
        let (back, yaw, pitch, _) = melee_pose(&LOWER, MELEE_COOLDOWN - 1e-4).unwrap();
        assert!(
            back.length() < 1e-2 && yaw.abs() < 1e-2 && pitch.abs() < 1e-2,
            "back at the hold: {back} {yaw} {pitch}"
        );
        is_continuous(&LOWER, 0.08);
    }

    #[test]
    fn the_cut_travels_from_the_right_to_the_left() {
        let (raised, raised_yaw, raised_pitch, _) = melee_pose(&SLASH, 0.14).unwrap();
        let (cut, cut_yaw, cut_pitch, _) = melee_pose(&SLASH, 0.30).unwrap();
        let (through, through_yaw, through_pitch, _) = melee_pose(&SLASH, 0.44).unwrap();
        // The blade is what travels. A positive yaw points it left of the
        // crosshair and a positive pitch above it, so the cut starts high
        // on the right and finishes low on the left.
        assert!(raised_yaw < 0.0, "blade starts right: {raised_yaw}");
        assert!(cut_yaw > 0.2, "sweeps through the middle: {cut_yaw}");
        assert!(through_yaw > cut_yaw, "and on to the left: {through_yaw}");
        assert!(
            raised_pitch > 0.5,
            "starts above the crosshair: {raised_pitch}"
        );
        assert!(
            cut_pitch < raised_pitch,
            "descends into the cut: {cut_pitch}"
        );
        assert!(
            through_pitch < 0.0 && through_pitch < cut_pitch,
            "and finishes below the crosshair: {through_pitch}"
        );
        // The hilt follows, much less far: a cut pivots at the shoulder.
        assert!(
            raised.y < cut.y && cut.y < through.y,
            "the hilt drifts left"
        );
        assert!(
            through.y - raised.y < 0.6,
            "the hilt travels less than the blade: {raised} to {through}"
        );
        assert!(
            raised.z > through.z,
            "and settles lower: {raised} {through}"
        );
        is_continuous(&SLASH, 0.08);
    }

    #[test]
    fn the_edge_turns_over_into_the_cut() {
        let (.., roll_before) = melee_pose(&SLASH, 0.0).unwrap();
        let (.., roll_cut) = melee_pose(&SLASH, 0.30).unwrap();
        let (.., roll_through) = melee_pose(&SLASH, 0.44).unwrap();
        let (.., roll_end) = melee_pose(&SLASH, MELEE_COOLDOWN - 1e-4).unwrap();
        assert!(roll_before.abs() < 1e-6, "starts unrolled");
        assert!(roll_cut < -0.4, "rolled into the cut: {roll_cut}");
        assert!(roll_through <= roll_cut, "and holds through it");
        // The roll stays modest on purpose: it turns about the blade, so
        // the fist and its cuff orbit with it, and a big roll throws the
        // arm across the screen.
        assert!(roll_through > -0.9, "not a barrel roll: {roll_through}");
        assert!(roll_end.abs() < 1e-2, "back to level: {roll_end}");
    }

    #[test]
    fn the_viewmodel_nodes_are_sorted_by_name() {
        // The fifteen names `verify_glb` pins, each to its slot.
        assert_eq!(classify("hand_sword"), Slot::Fist);
        assert_eq!(classify("hands"), Slot::Arms);
        assert_eq!(classify("rifle"), Slot::Weapon(1));
        assert_eq!(classify("shield"), Slot::Shield);
        assert_eq!(classify("sword"), Slot::Sword);
        assert_eq!(classify("w_ak47"), Slot::Weapon(3));
        assert_eq!(classify("w_revolver_cylinder"), Slot::Weapon(5));
        assert_eq!(classify("w_revolver_frame"), Slot::Weapon(5));
        assert_eq!(classify("w_revolver_hammer"), Slot::Weapon(5));
        assert_eq!(classify("w_revolver_receiver"), Slot::Weapon(5));
        assert_eq!(classify("w_revolver_trigger"), Slot::Weapon(5));
        assert_eq!(classify("w_rpg7"), Slot::Weapon(7));
        assert_eq!(classify("w_rpg7_rocket"), Slot::Rocket);
        assert_eq!(classify("w_sniper"), Slot::Weapon(6));
        assert_eq!(classify("w_vityaz"), Slot::Weapon(2));
        // The M4's node, when it exists, and the older rules.
        assert_eq!(classify("w_m4"), Slot::Weapon(4));
        // The `--split` fallback's per-material nodes, by prefix; the
        // rocket keeps its own list under the `w_rpg7_` prefix.
        assert_eq!(classify("w_vityaz_Glass"), Slot::Weapon(2));
        assert_eq!(classify("w_vityaz_receiver"), Slot::Weapon(2));
        assert_eq!(classify("w_sniper_mag"), Slot::Weapon(6));
        assert_eq!(classify("w_sniper_rifle.001"), Slot::Weapon(6));
        assert_eq!(classify("w_ak47_AK"), Slot::Weapon(3));
        assert_eq!(classify("w_m4_body"), Slot::Weapon(4));
        assert_eq!(classify("w_rpg7_RPG7"), Slot::Weapon(7));
        assert_eq!(classify("w_rpg7_rocket"), Slot::Rocket);
        assert_eq!(classify("arm_sword"), Slot::Arms);
        assert_eq!(classify("cylinder"), Slot::Weapon(1));
        // The revolver's moving parts are told by their suffix, the v15
        // bare names still by theirs.
        assert_eq!(anim_of("w_revolver_cylinder"), PartAnim::Cylinder);
        assert_eq!(anim_of("w_revolver_hammer"), PartAnim::Hammer);
        assert_eq!(anim_of("w_revolver_trigger"), PartAnim::Trigger);
        assert_eq!(anim_of("w_revolver_frame"), PartAnim::Fixed);
        assert_eq!(anim_of("cylinder_a"), PartAnim::Cylinder);
        assert_eq!(anim_of("hammer"), PartAnim::Hammer);
        assert_eq!(anim_of("w_ak47"), PartAnim::Fixed);
    }

    /// Ids off the table share the sidearm's slot, never index past it.
    #[test]
    fn every_weapon_id_has_a_slot() {
        for id in 0..=255u8 {
            let s = slot(id);
            assert!((1..WEAPON_SLOTS).contains(&s), "id {id} -> slot {s}");
        }
        assert_eq!(slot(0), SIDEARM as usize);
        assert_eq!(slot(WEAPON_COUNT + 1), SIDEARM as usize);
        assert_eq!(slot(7), 7);
    }

    /// The real GLB: every table weapon resolves to a non-empty part list,
    /// its own or the sidearm's. Loosened on purpose while the GLB is
    /// being rebuilt: which ids draw their own mesh is the integrator's
    /// assertion once `verify_glb` has passed on the shipped file.
    #[test]
    fn every_table_weapon_has_a_node_or_a_fallback() {
        let (meshes, assets) = load_assets();
        let assets = assets.expect("the embedded viewmodel loads");
        assert!(!meshes.is_empty());
        assert!(
            !assets.weapons[SIDEARM as usize].is_empty(),
            "the sidearm is the fallback and must itself exist"
        );
        for id in 1..=WEAPON_COUNT {
            let (parts, own) = assets.weapon_parts(id);
            assert!(!parts.is_empty(), "{}: no parts at all", weapon_name(id));
            for p in parts {
                assert!(p.mesh >= 1 && (p.mesh as usize) <= meshes.len());
            }
            let m = assets.muzzle_of(id);
            assert!(
                m.is_finite() && m.length() > 0.05,
                "{}: muzzle {m}",
                weapon_name(id)
            );
            // The shipped GLB carries a node for every weapon but the M4 (id 4,
            // still inside its RAR5 archive), so only the M4 may take the fallback;
            // an id that unexpectedly falls back means a node was renamed or lost.
            assert_eq!(
                own,
                id != 4,
                "{}: expected own mesh = {}, got {own}",
                weapon_name(id),
                id != 4
            );
            if !own {
                assert_eq!(
                    parts.as_ptr(),
                    assets.weapons[SIDEARM as usize].as_ptr(),
                    "{}: the fallback is the sidearm's list",
                    weapon_name(id)
                );
            }
        }
        // An unknown id reads as the sidearm, like `weapon_stats`.
        let (parts, _) = assets.weapon_parts(200);
        assert_eq!(parts.as_ptr(), assets.weapons[SIDEARM as usize].as_ptr());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod wire_tests {
    use super::*;
    use arena_core::proto::color_for;

    const fn me(id: u8) -> PState {
        PState {
            id,
            x: 0.0,
            z: 0.0,
            y: 0.0,
            vy: 0.0,
            ax: 1.0,
            az: 0.0,
            pitch: 0.0,
            hp: MAX_HP,
            score: 0,
            alive: true,
            crouch: false,
            shield: false,
            weapon: 3,
            ammo: 20,
            reserve: 30,
            reloading: false,
            deaths: 0,
            ack: 0,
            ack_age_ticks: 0,
            team: 0,
        }
    }

    /// The recoil kick and the climb move the camera and the model, never
    /// the pitch the client sends: during a burst every `Input` on the wire
    /// carries `self.pitch`, while the frame's camera looks higher.
    #[test]
    fn recoil_never_reaches_the_wire() {
        let (chan, wire) = net::NetChan::detached();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.latest.insert(2, me(2));
        game.was_alive = true;
        game.pitch = 0.3;
        game.time = 5.0;
        // A confirmed AK shot an instant ago, with a climb from the burst.
        game.shot_started = Some(game.time);
        game.climb.value = 0.05;
        game.shots = 1;
        let input = InputState::default();
        let mut inputs_seen = 0;
        let mut kicked_frames = 0;
        for _ in 0..12 {
            let frame = game.update(&input, 0.01);
            let look = (frame.camera.target - frame.camera.eye).normalize();
            let cam_pitch = look.y.asin();
            if (cam_pitch - game.pitch).abs() > 1e-3 {
                kicked_frames += 1;
            }
            while let Ok(msg) = wire.try_recv() {
                if let C2S::Input { pitch, ax, az, .. } = msg {
                    inputs_seen += 1;
                    assert_eq!(pitch, game.pitch, "the wire carries the player's own pitch");
                    assert_eq!((ax, az), (game.aim.x, game.aim.y));
                }
            }
        }
        assert!(inputs_seen >= 2, "inputs were sent: {inputs_seen}");
        // `EMBER_CAM` pins the camera for screenshots; the kick is then not
        // observable, and the wire half of the claim is what matters.
        if debug_camera().is_none() {
            assert!(
                kicked_frames >= 6,
                "the camera kicked: {kicked_frames} frames"
            );
        }
        // Nothing on the wire moved the player's own aim either.
        assert_eq!(game.pitch, 0.3);
    }

    /// A device saying everything at once, for the two tests below.
    ///
    /// Every intent `update` reads from a device is held here, and each one
    /// is what makes the matching gate in `update` load-bearing: W (move),
    /// Shift (sprint), R (reload), Tab (the scoreboard), Q (shield), Space
    /// (jump), E (melee), both mouse buttons (fire and ADS), 400 x 120 px of
    /// raw mouse motion (the look — this is the read that used to hand the
    /// operator's mouse our camera), and a pad on full trigger with both
    /// sticks pushed. Deliberately absent: C, because the scripted half
    /// asserts that crouch came from the script, and a device C would make
    /// that assertion pass with the gate deleted.
    fn a_busy_device() -> InputState {
        InputState::from_parts(
            &[
                KeyCode::KeyW,
                KeyCode::ShiftLeft,
                KeyCode::KeyR,
                KeyCode::Tab,
                KeyCode::KeyQ,
                KeyCode::Space,
                KeyCode::KeyE,
            ],
            &[MouseButton::Left, MouseButton::Right],
            (400.0, 120.0),
            Some(ember_engine::PadState {
                rt: 1.0,
                left: [1.0, 1.0],
                right: [1.0, 0.0],
                ..ember_engine::PadState::default()
            }),
        )
    }

    /// Run a client for 12 frames against [`a_busy_device`], scripted or
    /// not, and return it with every `Input` that reached the wire.
    fn run_against_device(src: Option<&str>) -> (ShooterGame, Vec<C2S>) {
        let device = a_busy_device();
        let (chan, wire) = net::NetChan::detached();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.latest.insert(2, me(2));
        game.was_alive = true;
        game.script = src.map(|s| script::Timeline::parse(s).expect("the script parses"));
        for _ in 0..12 {
            game.update(&device, 0.02);
        }
        let mut sent = Vec::new();
        while let Ok(msg) = wire.try_recv() {
            if matches!(msg, C2S::Input { .. }) {
                sent.push(msg);
            }
        }
        (game, sent)
    }

    /// While a script drives the client, the device is not read at all.
    ///
    /// [`a_busy_device`] is fed to a scripted client, which must turn only
    /// where its script says and send only what its script holds. Every
    /// assertion here pins one gate in `update`, and the companion test
    /// below proves none of them is vacuous. Delete a gate and this fails:
    /// the mouse or the right stick turns the view, the keys walk it
    /// forward, the trigger, reload, shield, jump, melee or scope come back,
    /// or Tab flips the scoreboard into the picture.
    #[test]
    fn a_script_drives_the_client_and_the_device_is_ignored() {
        // Face +Z, then crouch-strafe left for the rest of the run.
        let (game, sent) = run_against_device(Some("aim 90; crouch a 5"));
        assert!(!sent.is_empty(), "the scripted client still sends input");
        let yaw = game.yaw;
        assert!(
            (yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-5,
            "yaw {yaw}: the script set the heading, and neither the mouse nor the right stick moved it"
        );
        assert!(
            game.pitch.abs() < 1e-6,
            "pitch {}: 120 px of device motion did not tilt the view",
            game.pitch
        );
        assert!(
            game.zoom < 1e-3,
            "zoom {}: the held right button did not raise the scope",
            game.zoom
        );
        assert!(
            !game.score_shown,
            "the held Tab did not put the scoreboard in the picture"
        );
        for m in &sent {
            let C2S::Input {
                mx,
                my,
                fire,
                crouch,
                sprint,
                reload,
                jump,
                shield,
                melee,
                ads,
                ..
            } = *m
            else {
                unreachable!("filtered to Input")
            };
            // Facing +Z, left of that is +X: the script's strafe, not W and
            // not the left stick.
            assert!(mx > 0.9 && my.abs() < 0.1, "moved ({mx}, {my}), not left");
            assert!(!fire, "the held trigger and the pad's did not fire");
            assert!(!sprint, "the held Shift did not sprint");
            assert!(!reload, "the held R did not reload");
            assert!(!shield, "the held Q did not raise the shield");
            assert!(!jump, "the held Space did not jump");
            assert!(!melee, "the held E did not swing");
            assert!(!ads, "the held right button did not go to the sim");
            assert!(crouch, "the script's crouch is what reached the wire");
        }
    }

    /// The same device with no script: proof the assertions above are not
    /// vacuous — every one of them flips.
    #[test]
    fn without_a_script_that_same_device_does_all_of_it() {
        let (game, sent) = run_against_device(None);
        assert!(
            game.yaw.abs() > 1.0,
            "unscripted, the mouse turns the view: {}",
            game.yaw
        );
        assert!(
            game.pitch.abs() > 0.01,
            "unscripted, the mouse tilts it: {}",
            game.pitch
        );
        assert!(game.zoom > 0.1, "unscripted, the right button scopes");
        assert!(game.score_shown, "unscripted, Tab shows the scoreboard");
        let held = |f: fn(&C2S) -> bool| sent.iter().any(f);
        // Jump and melee are edge-latched, so they land in one packet, not
        // all of them; the rest are held every frame.
        assert!(held(|m| matches!(m, C2S::Input { jump: true, .. })), "jump");
        assert!(
            held(|m| matches!(m, C2S::Input { melee: true, .. })),
            "melee"
        );
        let C2S::Input {
            mx,
            my,
            fire,
            crouch,
            sprint,
            reload,
            shield,
            ads,
            ..
        } = *sent.last().expect("the unscripted client sends input")
        else {
            unreachable!("filtered to Input")
        };
        // W and the left stick walk at full speed along the mouse's heading.
        assert!(
            (mx.hypot(my) - 1.0).abs() < 0.05,
            "unscripted, W walks: ({mx}, {my})"
        );
        assert!(fire && sprint && reload, "unscripted: fire/sprint/reload");
        assert!(shield && ads, "unscripted: shield/ads");
        assert!(!crouch, "unscripted, nothing crouches");
    }

    /// The scope view is a frame-level fact: the sniper mostly zoomed draws
    /// the 24 near-black slabs of the mask 0.30 m ahead of the eye at the
    /// scope's own field of view; any other gun, or the sniper still
    /// easing in, draws none of them and keeps the hip field's blend.
    #[test]
    fn the_scope_mask_is_drawn_only_for_the_sniper_mostly_zoomed() {
        let frame_for = |weapon: u8, zoom: f32| {
            let (chan, _wire) = net::NetChan::detached();
            let mut game = ShooterGame::with_chan(chan, None, None);
            game.my_id = Some(2);
            let mut p = me(2);
            p.weapon = weapon;
            game.latest.insert(2, p);
            game.was_alive = true;
            game.zoom = zoom;
            game.time = 5.0;
            // A short frame: the eased zoom moves by well under 0.02.
            game.update(&InputState::default(), 0.001)
        };
        let slabs_in = |frame: &Frame| {
            frame
                .instances
                .iter()
                .filter(|i| i.color == feel::SCOPE_BLACK)
                .filter(|i| {
                    debug_camera().is_some()
                        || ((i.position - frame.camera.eye)
                            .dot((frame.camera.target - frame.camera.eye).normalize())
                            - feel::SCOPE_DIST)
                            .abs()
                            < 0.02
                })
                .count()
        };
        let sniper = frame_for(feel::SCOPED_WEAPON, 1.0);
        // The mask's 24 slabs and the reticle's two bars, nothing else
        // that colour at that distance.
        assert_eq!(slabs_in(&sniper), feel::SCOPE_SIDES + 2);
        if debug_camera().is_none() {
            // The zoom eased back a little over the frame (1.4 percent at
            // this dt), so the field is the scope's, not exactly 3.5.
            let fov = sniper.camera.fov_y_deg;
            assert!(fov < 5.0 && fov > 3.4, "scope field {fov}");
        }
        assert_eq!(
            slabs_in(&frame_for(feel::SCOPED_WEAPON, 0.5)),
            0,
            "easing in"
        );
        assert_eq!(slabs_in(&frame_for(3, 1.0)), 0, "an AK has no scope");
        assert_eq!(slabs_in(&frame_for(SIDEARM, 0.0)), 0, "hip");
    }

    /// A predicted bonk is felt once per jump. A reconciliation can rewind
    /// the arc to below the block with the head still rising, and the
    /// forward prediction then clamps on the block a second time; the
    /// per-slot debounce keeps that second clamp silent, and a jump made
    /// after the window bonks again.
    #[test]
    fn a_rewound_jump_does_not_bonk_twice() {
        use arena_core::shooter::BODY_H_STAND;

        let (chan, inbox, _wire) = net::NetChan::detached_duplex();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.latest.insert(2, me(2));
        game.was_alive = true;
        // One armed block hung 0.6 m above a standing head, over the spawn.
        let base = BODY_H_STAND + 0.6;
        game.obstacles = vec![Obstacle {
            min: [-0.5, -0.5],
            max: [0.5, 0.5],
            h: base + 1.0,
            base,
            kind: Cover::Loot,
        }];
        game.loot_index = vec![0];
        game.loot_active = vec![true];
        game.loot_bump = vec![None];
        game.last_bonk_at = vec![None];
        let input = InputState::default();
        let dt = 1.0 / 60.0;
        let bonk_rumble = feel::bonk().rumble.unwrap();
        let clamp_y = base - BODY_H_STAND;
        // Run `n` frames; how many bonks were felt and how high the feet got.
        let run = |game: &mut ShooterGame, n: usize| {
            let mut bonks = 0;
            let mut top = f32::MIN;
            for _ in 0..n {
                game.update(&input, dt);
                top = top.max(game.pred_y);
                bonks += game
                    .feedback()
                    .rumbles
                    .iter()
                    .filter(|r| **r == bonk_rumble)
                    .count();
            }
            (bonks, top)
        };
        game.pred_jump = true;
        let (bonks, top) = run(&mut game, 20);
        assert!(top >= clamp_y - 1e-3, "the jump reached the block: {top}");
        assert_eq!(bonks, 1, "the jump bonked once");
        assert!(game.last_bonk_at[0].is_some(), "the bonk was recorded");
        // The server's state rewinds the arc to 0.3 m up and still rising,
        // with nothing left to replay over it, so the prediction climbs
        // into the block a second time.
        game.history.clear();
        let mut rewound = me(2);
        rewound.y = 0.3;
        rewound.vy = 5.0;
        inbox
            .send(S2C::State {
                tick: 1,
                players: vec![rewound],
                bullets: Vec::new(),
                pads: Vec::new(),
                loot: vec![true],
                team_score: [0, 0],
                hill: arena_core::shooter::HILL_FREE,
                round_pause: 0.0,
            })
            .unwrap();
        let (bonks, top) = run(&mut game, 20);
        assert!(
            top >= clamp_y - 1e-3,
            "the rewound arc reached the block: {top}"
        );
        assert_eq!(bonks, 0, "the rewound arc did not bonk again");
        // Once the window has passed, a fresh jump from the floor bonks.
        run(&mut game, 30);
        assert!(game.pred_y <= 1e-3, "landed: {}", game.pred_y);
        game.time += BONK_DEBOUNCE;
        game.pred_jump = true;
        let (bonks, _) = run(&mut game, 20);
        assert_eq!(bonks, 1, "a later jump bonks again");
    }

    /// One `S2C::Shot` on the wire is one streak in the frame (v20): a
    /// remote round's starts at the shooter's muzzle with a flash and a
    /// plume there and leaves a mark on the face it hit; my own starts at
    /// the viewmodel's muzzle, not at the sim's origin, and flashes
    /// nothing (the viewmodel's own flash does that); a segment that goes
    /// on from a pierced body is a streak and nothing else; a rocket's
    /// segment is no streak at all, only its mark.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn a_shot_event_produces_a_tracer() {
        use arena_core::shooter::{SHOT_COVER, SHOT_FLOOR};

        let (chan, inbox, _wire) = net::NetChan::detached_duplex();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.latest.insert(2, me(2));
        let mut other = me(3);
        other.x = 10.0;
        // Facing the way it shoots, and drawn where it stands: the flash
        // hangs on the gun those two place, so the interpolation snaps
        // have to say the same thing the state does.
        other.ax = 0.0;
        other.az = 1.0;
        game.latest.insert(3, other);
        let snap = PSnap {
            x: 10.0,
            z: 0.0,
            y: 0.0,
        };
        game.from.insert(3, snap);
        game.to.insert(3, snap);
        game.was_alive = true;
        game.time = 5.0;
        game.set_rounds(500);
        let shot = |owner: u8,
                    weapon: u8,
                    from: [f32; 3],
                    to: [f32; 3],
                    hit: u8,
                    cover: u8,
                    normal: [i8; 3]| {
            S2C::Shot {
                owner,
                weapon,
                x0: from[0],
                y0: from[1],
                z0: from[2],
                x1: to[0],
                y1: to[1],
                z1: to[2],
                hit,
                cover,
                victim: 255,
                normal,
            }
        };
        let input = InputState::default();
        let flash_colour = FLASH_COLOR;
        // A remote AK round from 10 m to my right, north into a container.
        inbox
            .send(shot(
                3,
                3,
                [10.0, 1.45, 0.0],
                [10.0, 1.45, 30.0],
                SHOT_COVER,
                Cover::Container.index(),
                [0, 0, -1],
            ))
            .unwrap();
        game.update(&input, 0.001);
        assert_eq!(game.tracers.len(), 1);
        let t = game.tracers[0];
        assert_eq!(t.from, Vec3::new(10.0, 1.45, 0.0));
        assert_eq!(t.to, Vec3::new(10.0, 1.45, 30.0));
        assert_eq!((t.weapon, t.born), (3, game.time));
        // The frame it arrived on the head has not left the muzzle; a
        // millisecond later the AK's 715 m/s has it 0.7 m out, and the
        // streak behind it (the round's own drawn length shorter) is the
        // core alone.
        let frame = game.update(&input, 0.001);
        let ak = weapon_feel(3).tracer;
        let rods: Vec<&Instance> = frame.instances.iter().filter(|i| i.color == ak).collect();
        assert_eq!(rods.len(), 1, "a millisecond in, the core streak alone");
        assert!(
            rods[0].scale.x > 0.5 && rods[0].scale.x < 1.0,
            "{}",
            rods[0].scale
        );
        // The remote flash: a star of five streak cones with their bases
        // at the drawn muzzle — the tip of the gun this body is holding,
        // not the sim's eye-height launch point — four square to the shot
        // and one along it, none back along it.
        let muzzle = game.drawn_muzzle(3).expect("the shooter's gun is drawn");
        assert!(
            (muzzle - t.from).length() > 0.5 && muzzle.y < t.from.y - 0.4,
            "the drawn gun is well below and ahead of the sim's origin: {muzzle}"
        );
        let star: Vec<&Instance> = frame
            .instances
            .iter()
            .filter(|i| (i.position - muzzle).length() < 1e-4 && i.color == flash_colour)
            .collect();
        assert_eq!(star.len(), 5, "the remote flash star at the drawn muzzle");
        let ak_flash = weapon_feel(3).flash;
        let (mut across, mut along) = (0, 0);
        for c in &star {
            assert_eq!(c.mesh, 500 + rounds::STREAK_OFFSET, "a streak cone");
            assert!((c.scale.y - ak_flash * FLASH_BASE).abs() < 1e-6);
            let d = (c.rot * Vec3::X).dot(Vec3::Z);
            if d.abs() < 1e-5 {
                across += 1;
                assert!((c.scale.x - ak_flash * FLASH_PETAL).abs() < 1e-6);
            } else {
                assert!(d > 0.999, "forward, never back: {d}");
                along += 1;
                assert!((c.scale.x - ak_flash * FLASH_FORWARD).abs() < 1e-6);
            }
        }
        assert_eq!((across, along), (4, 1));
        assert!(
            !frame
                .instances
                .iter()
                .any(|i| i.mesh == 0 && i.color == flash_colour),
            "no flash cube"
        );
        assert_eq!(game.marks.len(), 1);
        assert_eq!(game.marks[0].pos, t.to);
        assert_eq!(game.marks[0].normal, -Vec3::Z);
        assert_eq!(game.marks[0].weapon, 3, "the mark knows what made it");
        // The mark is the AK's hole: the disc, 23.7 mm across, sunk into
        // the container's south face with a millimetre of it proud, thick
        // along the normal; not a square.
        let holes: Vec<&Instance> = frame
            .instances
            .iter()
            .filter(|i| i.color == feel::MARK_COLOR)
            .collect();
        assert_eq!(holes.len(), 1, "the mark is drawn");
        let h = holes[0];
        assert_eq!(h.mesh, 500 + rounds::DISC_OFFSET, "a disc, not a cube");
        assert!((h.scale.y * 2.0 - 0.0237).abs() < 1e-4, "{}", h.scale);
        assert_eq!(h.scale.y, h.scale.z);
        assert_eq!(h.scale.x, feel::MARK_THICK);
        assert!(
            (h.position - (t.to + Vec3::Z * (feel::MARK_THICK - feel::MARK_LIFT))).length() < 1e-6,
            "sunk into the face, not standing off it: {}",
            h.position
        );
        assert!(
            (h.rot * Vec3::X + Vec3::Z).length() < 1e-5,
            "thick along the normal"
        );
        assert!(game.fx.len() >= 12, "plume and sparks: {}", game.fx.len());
        assert!(game.continuations.is_empty(), "cover ends a round");
        // My own round into a body 20 m ahead: the streak starts at the
        // viewmodel's muzzle, a hand's reach ahead of the eye and below
        // it, not at the sim's launch point.
        let launch = Vec3::new(0.2, 1.45, 0.0);
        inbox
            .send(shot(
                2,
                3,
                launch.to_array(),
                [20.0, 1.45, 0.0],
                SHOT_BODY,
                255,
                [0, 0, 0],
            ))
            .unwrap();
        game.update(&input, 0.001);
        assert_eq!(game.tracers.len(), 2);
        let own = game.tracers[1];
        assert!(
            (own.from - launch).length() > 0.3 && own.from.x > 0.5 && own.from.y < 1.45,
            "anchored at the muzzle: {}",
            own.from
        );
        assert_eq!(own.to, Vec3::new(20.0, 1.45, 0.0));
        assert_eq!(game.flashes.len(), 1, "no second flash for my own shot");
        assert_eq!(game.marks.len(), 1, "a body takes no mark");
        assert_eq!(
            game.continuations.len(),
            1,
            "the body is a point to go on from"
        );
        // The same round going on through that body to the floor: a
        // streak from the body, no flash, no plume, a mark on the floor.
        let fx_before = game.fx.len();
        inbox
            .send(shot(
                2,
                3,
                [20.0, 1.45, 0.0],
                [40.0, 0.0, 0.0],
                SHOT_FLOOR,
                255,
                [0, 1, 0],
            ))
            .unwrap();
        game.update(&input, 0.001);
        assert_eq!(game.tracers.len(), 3);
        assert_eq!(game.tracers[2].from, Vec3::new(20.0, 1.45, 0.0));
        assert_eq!(game.flashes.len(), 1);
        assert_eq!(game.marks.len(), 2);
        assert_eq!(game.marks[1].normal, Vec3::Y);
        assert_eq!(game.marks[1].weapon, 3);
        assert_eq!(
            game.fx.len() - fx_before,
            6,
            "the floor's dust and nothing else"
        );
        // A rocket's segment: no streak (the mesh flies from the state),
        // its mark on the floor.
        inbox
            .send(shot(
                3,
                7,
                [10.0, 1.45, 0.0],
                [10.0, 0.0, 12.0],
                SHOT_FLOOR,
                255,
                [0, 1, 0],
            ))
            .unwrap();
        game.update(&input, 0.001);
        assert_eq!(game.tracers.len(), 3);
        assert_eq!(game.marks.len(), 3);
        assert_eq!(game.marks[2].weapon, 7, "the rocket's blast mark");
        assert!((game.marks[2].diameter() - feel::ROCKET_MARK).abs() < 1e-6);
    }

    /// Plant the shooter 10 m to the right of the watcher, facing and
    /// firing north, in the pose the case under test needs.
    fn place_shooter(game: &mut ShooterGame, crouch: bool, pitch: f32) {
        let mut p = me(3);
        p.x = 10.0;
        p.ax = 0.0;
        p.az = 1.0;
        p.crouch = crouch;
        p.pitch = pitch;
        game.latest.insert(3, p);
        let snap = PSnap {
            x: 10.0,
            z: 0.0,
            y: 0.0,
        };
        game.from.insert(3, snap);
        game.to.insert(3, snap);
    }
    /// Put one `Shot` from that shooter on the wire: the server's own
    /// geometry, from the eye it fires from to wherever the round ended.
    fn fire_shot(inbox: &std::sync::mpsc::Sender<S2C>, launch: Vec3, to: Vec3) {
        inbox
            .send(S2C::Shot {
                owner: 3,
                weapon: 3,
                x0: launch.x,
                y0: launch.y,
                z0: launch.z,
                x1: to.x,
                y1: to.y,
                z1: to.z,
                hit: arena_core::shooter::SHOT_COVER,
                cover: Cover::Container.index(),
                victim: 255,
                normal: [0, 0, -1],
            })
            .unwrap();
    }
    /// The tip of the barrel the remote body actually draws, read out of
    /// the frame rather than recomputed from the numbers under test.
    fn drawn_tip(frame: &Frame) -> Vec3 {
        let tip = frame
            .instances
            .iter()
            .find(|i| i.scale == BOX_TIP_SIZE && i.position.x > 5.0)
            .expect("the remote body's barrel tip");
        tip.position + tip.rot * Vec3::X * (BOX_TIP_SIZE.x * 0.5)
    }
    /// You hear a stranger walk, you hear them run louder, and you do not
    /// hear them crouch. The whole chain, not the rule alone: a body on the
    /// wire, the walk cycle its legs are posed from, the earshot and the
    /// cadence floor, and the cue that came out of the frame.
    ///
    /// The listener stands still five metres away. The walker is moved by
    /// hand between frames at exactly the stance speed for the gait under
    /// test, which is what the client measures a remote body's speed from.
    #[test]
    fn a_stranger_is_heard_walking_and_running_but_never_crouching() {
        /// How far the stranger walks from the listener, metres.
        const RING: f32 = 5.0;

        use arena_core::shooter::stance_speed;

        let heard_over = |sprint: bool, crouch: bool| -> Vec<Sfx> {
            let (chan, _inbox, _wire) = net::NetChan::detached_duplex();
            let mut game = ShooterGame::with_chan(chan, None, None);
            game.my_id = Some(2);
            game.latest.insert(2, me(2));
            game.was_alive = true;
            game.set_rounds(500);
            let input = InputState::default();
            let speed = stance_speed(sprint, crouch, false);
            let dt = 1.0 / 60.0;
            let mut out = Vec::new();
            // Two seconds of moving at a constant five metres, walked round
            // the listener rather than past them: a sprinter walked in a
            // straight line simply leaves earshot sooner and is heard less,
            // which says nothing about the cadence under test.
            let mut a = 0.0f32;
            for _ in 0..120 {
                a += speed * dt / RING;
                let (sn, cs) = a.sin_cos();
                let (x, z) = (RING * cs, RING * sn);
                let mut p = me(3);
                p.x = x;
                p.z = z;
                p.crouch = crouch;
                game.latest.insert(3, p);
                let snap = PSnap { x, z, y: 0.0 };
                game.from.insert(3, snap);
                game.to.insert(3, snap);
                game.update(&input, dt);
                out.extend(game.heard.iter().map(|p| p.sfx));
            }
            out
        };

        let walking: Vec<Sfx> = heard_over(false, false)
            .into_iter()
            .filter(|s| feel::is_step(*s))
            .collect();
        let running: Vec<Sfx> = heard_over(true, false)
            .into_iter()
            .filter(|s| feel::is_step(*s))
            .collect();
        let crouching: Vec<Sfx> = heard_over(false, true)
            .into_iter()
            .filter(|s| feel::is_step(*s))
            .collect();

        assert!(
            (4..=8).contains(&walking.len()),
            "two seconds of walking is about six steps, not {}",
            walking.len()
        );
        assert!(
            running.len() > walking.len(),
            "a runner is heard more often: {} against {}",
            running.len(),
            walking.len()
        );
        assert!(
            crouching.is_empty(),
            "a crouching body is silent, but {} steps came out",
            crouching.len()
        );
        assert!(
            walking.iter().all(|s| feel::is_walk_step(*s)),
            "a walk plays the walk cue"
        );
        assert!(
            running.iter().all(|s| !feel::is_walk_step(*s)),
            "and a sprint plays the other one"
        );
    }

    /// A remote shot's light and smoke belong on the gun that is drawn.
    ///
    /// The sim fires from the shooter's eye (`EYE_STAND`, a hand ahead of
    /// it) while the client draws that shooter's weapon at the hand, so
    /// hanging the flash on the event's own origin put it about 0.6 m
    /// above the barrel with wall showing in between: every remote shot
    /// anyone saw, captured. The round is not re-aimed to fix it. The
    /// head, the segment's end and the mark stay the server's, and only
    /// the flash, the smoke and the streak's start move onto the barrel.
    ///
    /// Crouching drops it, a near-vertical aim swings it up with the
    /// weapon's own elevation, and a shooter whose body is not drawn at
    /// all falls back to the round's line.
    #[test]
    fn a_remote_flash_sits_on_the_gun_that_is_drawn() {
        let (chan, inbox, _wire) = net::NetChan::detached_duplex();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.latest.insert(2, me(2));
        game.was_alive = true;
        game.time = 5.0;
        game.set_rounds(500);
        let input = InputState::default();
        // The shooter: 10 m to my right, facing and firing north.
        let launch = Vec3::new(10.0, EYE_STAND, 0.0);
        let end = Vec3::new(10.0, EYE_STAND, 30.0);
        // ---- standing ----
        place_shooter(&mut game, false, 0.0);
        fire_shot(&inbox, launch, end);
        let frame = game.update(&input, 0.001);
        let flash = game.flashes[0].pos;
        let standing = flash;
        assert!(
            (flash - drawn_tip(&frame)).length() < 0.03,
            "the flash is on the drawn barrel: {flash} against {}",
            drawn_tip(&frame)
        );
        assert!(
            (flash - launch).length() > 0.5,
            "and not on the event's origin: {flash}"
        );
        assert!(
            flash.y < launch.y - 0.4,
            "the gun is well below the eye that fired: {flash}"
        );
        // The smoke is on the same barrel, not on the event's origin.
        let plume: Vec<&Fx> = game.fx.iter().filter(|f| f.born > game.time).collect();
        assert_eq!(plume.len(), 4, "the plume is held back and is four balls");
        assert!(
            plume
                .iter()
                .all(|f| (f.pos - flash).length() < feel::PLUME_LEAD + feel::plume_reach() + 1e-3),
            "the smoke rings the drawn muzzle"
        );
        // The round itself is untouched: its head, and so its drawn body,
        // is on the server's segment, and the segment's end is exactly
        // what the server sent.
        let t = game.tracers[0];
        assert_eq!((t.from, t.to), (launch, end));
        assert_eq!(t.muzzle, flash, "the streak starts where the light is");
        let head = t.head(game.time);
        let seg = (t.to - t.from).normalize();
        let off = head - t.from;
        assert!(
            (off - seg * off.dot(seg)).length() < 1e-4,
            "the round flies on the server's line: {head}"
        );
        assert_eq!(game.marks[0].pos, end, "and it hit where it hit");

        // ---- crouching: the hand drops, the flash with it ----
        game.flashes.clear();
        game.fx.clear();
        place_shooter(&mut game, true, 0.0);
        fire_shot(&inbox, launch, end);
        let frame = game.update(&input, 0.001);
        let crouched = game.flashes[0].pos;
        assert!(
            (crouched - drawn_tip(&frame)).length() < 0.03,
            "still on the drawn barrel when crouched: {crouched}"
        );
        let (_, _, stand_y, _) = body_heights(false);
        let (_, _, crouch_y, _) = body_heights(true);
        assert!(
            ((standing.y - crouched.y) - (stand_y - crouch_y)).abs() < 1e-4,
            "it dropped by exactly the hand: {} against {}",
            standing.y - crouched.y,
            stand_y - crouch_y
        );

        // ---- a near-vertical aim ----
        // The fallback cube pistol is drawn yaw-only, so the light on it
        // is too: on the barrel that is on screen, which is the whole
        // rule. The aim's yaw and its elevation stay two things — folding
        // them into one 3D direction would swing the body as well as the
        // gun.
        game.flashes.clear();
        game.fx.clear();
        place_shooter(&mut game, false, 1.5);
        fire_shot(&inbox, launch, Vec3::new(10.0, 30.0, 2.0));
        let frame = game.update(&input, 0.001);
        let steep = game.flashes[0].pos;
        assert!(
            (steep - drawn_tip(&frame)).length() < 0.03,
            "on the barrel however steep the aim: {steep}"
        );
        // With the real viewmodel the weapon tilts with its owner's
        // elevation (`weapon_rot`), and the muzzle climbs with it.
        let (_, assets) = load_assets();
        game.assets = Some(assets.expect("the embedded viewmodel loads"));
        place_shooter(&mut game, false, 0.0);
        let level = game.drawn_muzzle(3).expect("the gun is drawn");
        place_shooter(&mut game, false, 1.5);
        let raised = game.drawn_muzzle(3).expect("the gun is drawn");
        assert!(
            raised.y > level.y + 0.2 && raised.z < level.z,
            "the barrel came up and drew in: {level} to {raised}"
        );
        game.assets = None;

        // ---- no body drawn: back to the round's own line ----
        game.flashes.clear();
        game.fx.clear();
        game.latest.remove(&3);
        fire_shot(&inbox, launch, end);
        game.update(&input, 0.001);
        let orphan = game.flashes[0].pos;
        assert!(
            (orphan - (launch + Vec3::Z * REMOTE_MUZZLE)).length() < 1e-4,
            "nothing of that shooter is on screen, so the line it is: {orphan}"
        );
    }

    /// A shot reads as light first and smoke second (v20, the fourth
    /// pass). The plume used to be born with the star, four opaque cubes
    /// of edge 0.10 m over a muzzle for a quarter second while the star
    /// lived 35 to 60 ms, so from six metres a shot was a lump of grey
    /// boxes and no star at all. Now every plume puff is held back by its
    /// weapon's own flash life and starts `feel::PLUME_LEAD` down the
    /// bore: at the shot there is a star and no smoke, and when the flash
    /// is out the smoke is there and the star is gone. And no weapon's
    /// petals are shorter than the ball of smoke that follows them.
    #[test]
    fn the_star_shows_before_the_smoke() {
        use arena_core::shooter::{SHOT_COVER, WEAPON_COUNT};

        let (chan, inbox, _wire) = net::NetChan::detached_duplex();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.latest.insert(2, me(2));
        let mut other = me(3);
        other.x = 10.0;
        other.ax = 0.0;
        other.az = 1.0;
        game.latest.insert(3, other);
        let snap = PSnap {
            x: 10.0,
            z: 0.0,
            y: 0.0,
        };
        game.from.insert(3, snap);
        game.to.insert(3, snap);
        game.was_alive = true;
        game.time = 5.0;
        game.set_rounds(500);
        let input = InputState::default();
        // A remote AK shot 10 m to my right, north into a container.
        inbox
            .send(S2C::Shot {
                owner: 3,
                weapon: 3,
                x0: 10.0,
                y0: 1.45,
                z0: 0.0,
                x1: 10.0,
                y1: 1.45,
                z1: 30.0,
                hit: SHOT_COVER,
                cover: Cover::Container.index(),
                victim: 255,
                normal: [0, 0, -1],
            })
            .unwrap();
        let muzzle = game.drawn_muzzle(3).expect("the shooter's gun is drawn");
        // The star's five cones, and the balls of smoke at that muzzle:
        // the impact's dust is 30 m up the line and is not smoke here.
        let star = |f: &Frame| {
            f.instances
                .iter()
                .filter(|i| i.color == FLASH_COLOR)
                .count()
        };
        let smoke = |f: &Frame| {
            f.instances
                .iter()
                .filter(|i| {
                    i.mesh == 500 + rounds::PUFF_OFFSET && (i.position - muzzle).length() < 1.0
                })
                .count()
        };
        let frame = game.update(&input, 0.001);
        assert_eq!(star(&frame), 5, "the star is drawn at the shot");
        assert_eq!(smoke(&frame), 0, "and nothing is over it");
        assert_eq!(game.fx.iter().filter(|f| f.born > game.time).count(), 4);
        // Now walk the flash out at the frame rate the client actually
        // runs, 60 Hz, not in one step the length of the flash: the delay
        // used to be a per-frame countdown started in the inbox drain,
        // which is BEFORE the particle retain and after `self.time` has
        // moved, so a remote plume lost the shot frame's own dt and was
        // drawn one frame early — on top of the star. On no frame may both
        // be up.
        let mut smoked = 0;
        for _ in 0..8 {
            let frame = game.update(&input, 1.0 / 60.0);
            let (s, k) = (star(&frame), smoke(&frame));
            assert!(
                s == 0 || k == 0,
                "t={}: {s} star cones and {k} balls of smoke on one frame",
                game.time
            );
            smoked += usize::from(k > 0);
        }
        // And the smoke did arrive: the AK's flash is 45 ms, so it is out
        // inside three frames and the four balls have the muzzle after it.
        assert!(smoked >= 5, "the plume arrives as the light goes");
        assert!(game.fx.iter().all(|f| f.born <= game.time));
        // Every weapon's star stands out past the smoke that succeeds it,
        // by the margin `FLASH_CLEAR` promises — a bound the raw table did
        // not already meet, so dropping the floor fails this.
        for id in 1..=WEAPON_COUNT {
            let petal = shot_flash(&weapon_feel(id)) * FLASH_PETAL;
            assert!(
                petal >= feel::plume_reach() * FLASH_CLEAR - 1e-6,
                "id {id}: a petal of {petal} against a plume reaching {}",
                feel::plume_reach()
            );
        }
        // The Vityaz's 0.10 is the one flash in the table under that floor
        // and is drawn bigger than the table says; the sidearm's 0.14 is
        // already clear and is drawn exactly as the table has it.
        assert!(shot_flash(&weapon_feel(2)) > weapon_feel(2).flash);
        let sidearm = weapon_feel(1);
        assert!((shot_flash(&sidearm) - sidearm.flash).abs() < 1e-6);
    }

    /// With the round meshes registered a tracer is the round itself at
    /// the head, at `ROUND_SCALE` times its calibre and with no tint, and
    /// one streak behind it: a cone alone while only the core exists, its
    /// base inside the round's tail and its point trailing; a frustum core
    /// and a cone tail once the tail exists, the tail's base the frustum's
    /// narrow end. Once the head lands the round goes and the streaks stay
    /// through the linger, thinning. The aim dot is gone: nothing stands
    /// 4 m down the look; on native the crosshair is two white hairlines
    /// 1.2 m ahead.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn a_tracer_is_the_round_with_a_streak_behind_it() {
        use arena_core::shooter::SHOT_COVER;

        let (chan, inbox, _wire) = net::NetChan::detached_duplex();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.latest.insert(2, me(2));
        let mut other = me(3);
        other.x = 10.0;
        game.latest.insert(3, other);
        game.was_alive = true;
        game.time = 5.0;
        game.set_rounds(500);
        let round_id = 500 + rounds::Round::Ak.offset();
        let streak_id = 500 + rounds::STREAK_OFFSET;
        let core_id = 500 + rounds::CORE_OFFSET;
        let input = InputState::default();
        // A remote AK round from 10 m to my right, north into a container.
        inbox
            .send(S2C::Shot {
                owner: 3,
                weapon: 3,
                x0: 10.0,
                y0: 1.45,
                z0: 0.0,
                x1: 10.0,
                y1: 1.45,
                z1: 30.0,
                hit: SHOT_COVER,
                cover: Cover::Container.index(),
                victim: 255,
                normal: [0, 0, -1],
            })
            .unwrap();
        game.update(&input, 0.001);
        let t = game.tracers[0];
        let ak = weapon_feel(3).tracer;
        let half_len = rounds::Round::Ak.length() * 0.5 * rounds::ROUND_SCALE;
        // The streak's base: the lead fraction of the drawn heel, a tenth
        // of the half-length inside the round's tail.
        let lead = rounds::Round::Ak.heel_radius() * rounds::ROUND_SCALE * rounds::STREAK_LEAD;
        let inset = half_len * (1.0 - rounds::STREAK_INSET);
        assert!(lead < rounds::Round::Ak.heel_radius() * rounds::ROUND_SCALE);
        // Two milliseconds in the head is 1.43 m out: the round is at it,
        // nose north, at five times its size, untinted.
        let frame = game.update(&input, 0.002);
        let now = game.time;
        let drawn: Vec<&Instance> = frame
            .instances
            .iter()
            .filter(|i| i.mesh == round_id)
            .collect();
        assert_eq!(drawn.len(), 1, "one round in flight");
        let r = drawn[0];
        assert!((r.position - t.head(now)).length() < 1e-4, "{}", r.position);
        assert_eq!(r.scale, Vec3::splat(rounds::ROUND_SCALE));
        assert_eq!(r.color, Vec3::ONE, "a textured part carries no tint");
        assert!(
            (r.rot * Vec3::X - Vec3::Z).length() < 1e-5,
            "nose along the flight"
        );
        // The core streak alone this early, as the cone (nothing behind
        // it): the AK's colour, its base radius the lead fraction of the
        // round's drawn heel, its base inside the round's tail, its back
        // where the core rod ends (so it is the rod less the inset), its
        // point trailing south. The remote flash star wears the same cone
        // in the flash colour; it is not a streak.
        let streaks: Vec<&Instance> = frame
            .instances
            .iter()
            .filter(|i| i.mesh == streak_id && i.color != FLASH_COLOR)
            .collect();
        assert_eq!(streaks.len(), 1, "the core alone");
        assert!(
            !frame.instances.iter().any(|i| i.mesh == core_id),
            "no frustum without a tail"
        );
        let s = streaks[0];
        let rods = t.rods(now);
        assert_eq!(s.color, ak);
        assert!(
            (s.scale.x - (rods[0].len - inset)).abs() < 1e-5,
            "{}",
            s.scale
        );
        assert!((s.scale.y - lead).abs() < 1e-6 && (s.scale.z - lead).abs() < 1e-6);
        assert!(
            (s.rot * Vec3::X + Vec3::Z).length() < 1e-5,
            "the point trails"
        );
        let base = t.head(now) - Vec3::Z * inset;
        assert!(
            (s.position - base).length() < 1e-4,
            "the base inside the round's tail: {} vs {base}",
            s.position
        );
        let core_back = t.head(now) - Vec3::Z * rods[0].len;
        assert!(
            (s.position - Vec3::Z * s.scale.x - core_back).length() < 1e-4,
            "the point where the core rod ends"
        );
        assert!(
            !frame.instances.iter().any(|i| i.mesh == 0 && i.color == ak),
            "no box rods with the meshes registered"
        );
        // Sixty milliseconds on the AK's 30 m (42 ms) are flown: the round
        // is gone, both streaks stay, thinned by the fade: the core as the
        // frustum, the tail as the cone with its base the frustum's narrow
        // end, exactly where the core ends.
        let frame = game.update(&input, 0.06);
        let now = game.time;
        assert!(!t.flying(now) && t.alive(now), "in the linger");
        assert!(
            !frame.instances.iter().any(|i| i.mesh == round_id),
            "no round in the linger"
        );
        let cores: Vec<&Instance> = frame
            .instances
            .iter()
            .filter(|i| i.mesh == core_id)
            .collect();
        let tails: Vec<&Instance> = frame
            .instances
            .iter()
            .filter(|i| i.mesh == streak_id && i.color != FLASH_COLOR)
            .collect();
        assert_eq!((cores.len(), tails.len()), (1, 1), "a core and a tail");
        let (core, tail) = (cores[0], tails[0]);
        let fade = t.fade(now);
        assert!(fade > 0.5 && fade < 1.0, "fading: {fade}");
        assert!((core.scale.y - lead * fade).abs() < 1e-6, "{}", core.scale);
        assert!(
            (tail.scale.y - lead * fade * rounds::CORE_NECK).abs() < 1e-6,
            "{}",
            tail.scale
        );
        assert_eq!(core.color, ak);
        assert_eq!(tail.color, ak * feel::TRACER_TAIL_DIM);
        let rods = t.rods(now);
        assert!((core.scale.x - (rods[0].len - inset)).abs() < 1e-5);
        assert!((tail.scale.x - rods[1].len).abs() < 1e-5);
        let core_end = core.position - Vec3::Z * core.scale.x;
        assert!(
            (tail.position - core_end).length() < 1e-4,
            "the tail starts where the core ends"
        );
        for s in [core, tail] {
            assert!((s.rot * Vec3::X + Vec3::Z).length() < 1e-5, "trailing");
        }
        if debug_camera().is_none() {
            // The aim dot is gone: no cube 4 m down the look.
            let look = (frame.camera.target - frame.camera.eye).normalize();
            let dot = frame.camera.eye + look * 4.0;
            assert!(
                !frame
                    .instances
                    .iter()
                    .any(|i| i.mesh == 0 && (i.position - dot).length() < 1e-3),
                "the aim dot"
            );
            // The native crosshair: two white hairlines 1.2 m ahead, one
            // along the right and one along the up, never thicker than a
            // hairline.
            #[cfg(not(target_arch = "wasm32"))]
            {
                let at = frame.camera.eye + look * CROSSHAIR_DIST;
                let hair: Vec<&Instance> = frame
                    .instances
                    .iter()
                    .filter(|i| i.mesh == 0 && i.color == CROSSHAIR_COLOR)
                    .filter(|i| (i.position - at).length() < 1e-4)
                    .collect();
                assert_eq!(hair.len(), 2, "two bars");
                for h in &hair {
                    assert!(h.scale.max_element() <= CROSSHAIR_LEN + 1e-6, "{}", h.scale);
                    assert!(h.scale.min_element() <= CROSSHAIR_THICK + 1e-6);
                }
                let along: Vec<Vec3> = hair
                    .iter()
                    .map(|h| {
                        let axis = if h.scale.x > h.scale.y {
                            Vec3::X
                        } else {
                            Vec3::Y
                        };
                        h.rot * axis
                    })
                    .collect();
                assert!(along[0].dot(look).abs() < 1e-4 && along[1].dot(look).abs() < 1e-4);
                assert!(along[0].dot(along[1]).abs() < 1e-4, "a cross");
            }
        }
    }

    /// Rumble requests accumulate through a frame and the platform takes
    /// them all at once; the next take is empty.
    #[test]
    fn feedback_is_handed_over_once() {
        let (chan, _wire) = net::NetChan::detached();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.feedback.rumble(0.5, 0.2, 40);
        game.feedback.rumble(0.1, 0.9, 300);
        let fb = game.feedback();
        assert_eq!(fb.rumbles.len(), 2);
        assert_eq!(game.feedback(), Feedback::default());
    }

    fn meta(id: u8, handle: &str) -> PlayerMeta {
        PlayerMeta {
            id,
            handle: handle.into(),
            color: color_for(id),
        }
    }

    /// In team deathmatch a remote body and its pips wear the team's
    /// colour and the id colour from `PlayerMeta` appears nowhere; in free
    /// for all the id colour is back and the pips are green.
    #[test]
    fn team_colours_replace_id_colours_in_tdm() {
        let frame_for = |mode: GameMode| {
            let (chan, _wire) = net::NetChan::detached();
            let mut game = ShooterGame::with_chan(chan, None, None);
            game.my_id = Some(2);
            game.mode = mode;
            game.latest.insert(2, me(2));
            let mut other = me(3);
            other.x = 4.0;
            other.team = 1;
            other.hp = 3;
            game.latest.insert(3, other);
            // Player 3's id colour is the palette's green.
            game.metas.insert(3, meta(3, "green"));
            game.was_alive = true;
            game.time = 5.0;
            game.update(&InputState::default(), 0.001)
        };
        let green = Vec3::from_array(color_for(3));
        let red = feel::team_color(1);
        let pip_green = Vec3::new(0.3, 0.9, 0.4);
        let count = |f: &Frame, c: Vec3| f.instances.iter().filter(|i| i.color == c).count();
        let tdm = frame_for(GameMode::Tdm);
        assert!(
            count(&tdm, red) >= 4,
            "the body and three pips in red: {}",
            count(&tdm, red)
        );
        assert_eq!(count(&tdm, green), 0, "no id colour in a team game");
        assert_eq!(count(&tdm, pip_green), 0, "pips take the team colour");
        let ffa = frame_for(GameMode::Ffa);
        assert!(count(&ffa, green) >= 1, "the id colour outside a team game");
        assert_eq!(count(&ffa, red), 0);
        assert!(
            count(&ffa, pip_green) >= 3,
            "green pips outside a team game"
        );
    }

    /// The status line's match segment per mode, my team first in team
    /// deathmatch, the king by handle in king of the hill, and the pause
    /// countdown with the winner's line beside it; and the scoreboard's
    /// shape: a team header per side and a SCORE column on the hill.
    #[test]
    fn the_status_line_names_the_mode() {
        let (chan, _wire) = net::NetChan::detached();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        let mut mine = me(2);
        mine.score = 5;
        mine.team = 1;
        game.latest.insert(2, mine);
        game.latest.insert(3, me(3));
        game.metas.insert(3, meta(3, "kestrel"));
        let line = |g: &ShooterGame| g.mode_line(g.latest.get(&2));
        game.mode = GameMode::Ffa;
        assert_eq!(line(&game), "frags 5 / 20");
        assert!(
            game.scoreboard().contains("frags 5 / 20"),
            "on the status line"
        );
        game.mode = GameMode::Tdm;
        game.team_score = [12, 9];
        assert_eq!(line(&game), "RED 9 · BLUE 12 / 30", "my team first");
        game.latest.get_mut(&2).unwrap().team = 0;
        assert_eq!(line(&game), "BLUE 12 · RED 9 / 30");
        game.mode = GameMode::Hill;
        game.latest.get_mut(&2).unwrap().score = 23;
        game.hill_holder = HILL_FREE;
        assert_eq!(line(&game), "hill 23 / 60 · hill free");
        game.hill_holder = HILL_CONTESTED;
        assert_eq!(line(&game), "hill 23 / 60 · contested");
        game.hill_holder = 3;
        assert_eq!(line(&game), "hill 23 / 60 · king: kestrel");
        game.hill_holder = 2;
        assert_eq!(line(&game), "hill 23 / 60 · king: player 2");
        // The pause replaces it with the countdown, rounded up so the
        // line never says 0 while the round is still paused.
        game.round_pause = 6.2;
        assert_eq!(line(&game), "next round in 7 s");
        game.round_line = Some("kestrel is king of the hill".into());
        assert_eq!(
            line(&game),
            "kestrel is king of the hill · next round in 7 s"
        );
        game.round_pause = 0.0;
        // The hill's scoreboard scores hill points, not frags.
        let board = game.scoreboard_text();
        assert!(
            board.contains("SCORE") && !board.contains("FRAGS"),
            "{board}"
        );
        // The team scoreboard: blue's header, blue's rows, red's header,
        // red's rows.
        game.mode = GameMode::Tdm;
        game.latest.get_mut(&3).unwrap().team = 1;
        let board = game.scoreboard_text();
        assert!(board.contains("FRAGS"), "{board}");
        let blue = board.find("BLUE 12").expect("blue header");
        let mine = board.find("▶ player 2").expect("my row");
        let red = board.find("RED 9").expect("red header");
        let theirs = board.find("  kestrel").expect("their row");
        assert!(blue < mine && mine < red && red < theirs, "{board}");
    }

    /// The hill's four bars and its marker are drawn in king of the hill
    /// only, white while free, in the king's own colour while held (mine
    /// when it is me), pulsing orange while contested.
    #[test]
    fn the_hill_bars_take_the_holders_colour() {
        let dock = Hill {
            min: [-4.0, -2.0],
            max: [4.0, 2.0],
            top: 1.2,
        };
        let frame_for = |mode: GameMode, holder: u8| {
            let (chan, _wire) = net::NetChan::detached();
            let mut game = ShooterGame::with_chan(chan, None, None);
            game.my_id = Some(2);
            game.mode = mode;
            game.hill = Some(dock);
            game.hill_holder = holder;
            game.latest.insert(2, me(2));
            game.latest.insert(3, me(3));
            game.metas.insert(2, meta(2, "me"));
            game.metas.insert(3, meta(3, "them"));
            game.was_alive = true;
            game.time = 5.0;
            let frame = game.update(&InputState::default(), 0.001);
            (frame, game.time)
        };
        let bar_y = dock.top + feel::HILL_BAR_LIFT;
        let marker = Vec3::new(0.0, dock.top + feel::HILL_MARKER_RISE, 0.0);
        let hill_colours = |f: &Frame| -> Vec<Vec3> {
            f.instances
                .iter()
                .filter(|i| {
                    (i.position.y - bar_y).abs() < 1e-5 || (i.position - marker).length() < 1e-5
                })
                .map(|i| i.color)
                .collect()
        };
        let (free, _) = frame_for(GameMode::Hill, HILL_FREE);
        let colours = hill_colours(&free);
        assert_eq!(colours.len(), 5, "four bars and a marker");
        assert!(colours.iter().all(|c| *c == feel::HILL_FREE_COLOR));
        let (held, _) = frame_for(GameMode::Hill, 3);
        let king = Vec3::from_array(color_for(3));
        let colours = hill_colours(&held);
        assert_eq!(colours.len(), 5);
        assert!(colours.iter().all(|c| *c == king), "the king's colour");
        let (mine, _) = frame_for(GameMode::Hill, 2);
        let me_colour = Vec3::from_array(color_for(2));
        assert!(hill_colours(&mine).iter().all(|c| *c == me_colour));
        let (contested, t) = frame_for(GameMode::Hill, HILL_CONTESTED);
        let expected = feel::hill_color(HILL_CONTESTED, None, t);
        let colours = hill_colours(&contested);
        assert_eq!(colours.len(), 5);
        assert!(colours.iter().all(|c| (*c - expected).length() < 1e-6));
        let (ffa, _) = frame_for(GameMode::Ffa, 3);
        assert!(hill_colours(&ffa).is_empty(), "no hill outside the mode");
    }

    /// One `RoundOver` on the wire is one announcement: one rumble, one
    /// line, and the line stays through the pause (with the scoreboard
    /// forced on) until the state that restarts the round clears it.
    #[test]
    fn round_over_is_announced_once() {
        let (chan, inbox, _wire) = net::NetChan::detached_duplex();
        let mut game = ShooterGame::with_chan(chan, None, None);
        game.my_id = Some(2);
        game.mode = GameMode::Tdm;
        game.latest.insert(2, me(2));
        let mut other = me(3);
        other.team = 1;
        game.latest.insert(3, other);
        game.was_alive = true;
        inbox
            .send(S2C::RoundOver {
                winner: 0,
                team: true,
                scores: vec![(2, 7), (3, 4)],
            })
            .unwrap();
        let rumble = feel::round_over(true).rumble.unwrap();
        let input = InputState::default();
        let mut felt = 0;
        for _ in 0..30 {
            game.update(&input, 1.0 / 60.0);
            felt += game
                .feedback()
                .rumbles
                .iter()
                .filter(|r| **r == rumble)
                .count();
        }
        assert_eq!(felt, 1, "announced once");
        assert_eq!(game.round_line.as_deref(), Some("BLUE wins the round"));
        let state = |pause: f32| S2C::State {
            tick: 1,
            players: vec![me(2), other],
            bullets: Vec::new(),
            pads: Vec::new(),
            loot: Vec::new(),
            team_score: [30, 12],
            hill: HILL_FREE,
            round_pause: pause,
        };
        inbox.send(state(9.5)).unwrap();
        game.update(&input, 1.0 / 60.0);
        assert_eq!(game.round_pause, 9.5);
        assert_eq!(game.team_score, [30, 12]);
        assert!(
            game.score_shown,
            "the scoreboard is forced on through the pause"
        );
        assert!(
            game.round_line.is_some(),
            "the line stays through the pause"
        );
        assert!(
            game.scoreboard()
                .contains("BLUE wins the round · next round in 10 s")
        );
        inbox.send(state(0.0)).unwrap();
        game.update(&input, 1.0 / 60.0);
        assert!(!game.score_shown, "the restart releases the scoreboard");
        assert_eq!(game.round_line, None, "the restart clears the line");
        assert_eq!(game.feedback(), Feedback::default(), "nothing else rumbled");
        // The other two modes' lines.
        game.metas.insert(3, meta(3, "kestrel"));
        game.mode = GameMode::Ffa;
        assert_eq!(
            game.round_over_line(3, false, &[(3, 20), (2, 11)]),
            "kestrel wins the round (20 frags)"
        );
        game.mode = GameMode::Hill;
        assert_eq!(
            game.round_over_line(3, false, &[(3, 60)]),
            "kestrel is king of the hill"
        );
        assert_eq!(game.round_over_line(1, true, &[]), "RED wins the round");
    }
}

#[cfg(test)]
mod viewmodel_tests {
    use super::*;

    fn deg(q: Quat, axis: Vec3) -> f32 {
        let (a, angle) = q.to_axis_angle();
        if a.dot(axis) < 0.0 { -angle } else { angle }.to_degrees()
    }

    #[test]
    fn a_fixed_part_never_moves() {
        for c in [0.0, 0.3, 1.0] {
            let a = Action { cycle: c, shots: 4 };
            assert_eq!(a.local_rot(PartAnim::Fixed), Quat::IDENTITY);
        }
    }

    #[test]
    fn the_cylinder_advances_one_chamber_per_shot_and_never_runs_back() {
        // Settled after shot n it sits n chambers on; the next shot starts
        // exactly where the previous one settled and adds one more. Compared
        // as rotations, not as angles: past 180 degrees an axis-angle reading
        // flips and would call a full turn a reversal.
        let settled = |n: u32| {
            Action {
                cycle: 1.0,
                shots: n,
            }
            .local_rot(PartAnim::Cylinder)
        };
        let start = |n: u32| {
            Action {
                cycle: 0.0,
                shots: n,
            }
            .local_rot(PartAnim::Cylinder)
        };
        for n in 1..12 {
            assert!(
                start(n + 1).angle_between(settled(n)) < 1e-3,
                "shot {n}: the next round starts where the last settled"
            );
            let step = settled(n + 1).angle_between(settled(n)).to_degrees();
            assert!(
                (step - 60.0).abs() < 1e-2,
                "shot {n}: one chamber is 60 degrees, got {step}"
            );
        }
        // Monotonic through the cooldown: every step turns the same way.
        let mut prev = start(2);
        for i in 1..=20u8 {
            let now = Action {
                cycle: f32::from(i) / 20.0,
                shots: 2,
            }
            .local_rot(PartAnim::Cylinder);
            let rel = prev.inverse() * now;
            let (axis, angle) = rel.to_axis_angle();
            if angle > 1e-5 {
                assert!(axis.x < 0.0, "cylinder ran backwards at sample {i}");
                assert!(angle.to_degrees() < 15.0, "cylinder jumped at sample {i}");
            }
            prev = now;
        }
    }

    #[test]
    fn the_hammer_cocks_then_falls_and_rests_forward() {
        let at = |c: f32| {
            deg(
                Action { cycle: c, shots: 1 }.local_rot(PartAnim::Hammer),
                Vec3::Z,
            )
        };
        assert!(at(0.0).abs() < 1e-4, "forward at the shot");
        assert!(at(0.55) > at(0.2) && at(0.2) > 0.0, "travels back");
        assert!(at(1.0).abs() < 1e-4, "rests forward");
    }

    #[test]
    fn the_trigger_is_pulled_at_the_shot_and_released_after() {
        let at = |c: f32| {
            deg(
                Action { cycle: c, shots: 1 }.local_rot(PartAnim::Trigger),
                Vec3::Z,
            )
        };
        assert!(at(0.0) < -20.0, "pulled back at the shot: {}", at(0.0));
        assert!(at(0.5).abs() < 1e-4 && at(1.0).abs() < 1e-4, "released");
    }

    #[test]
    fn rotating_about_a_pivot_leaves_the_pivot_where_it_was() {
        // The instance maths in push_parts: pos + rot * (pivot - local * pivot),
        // rotated by rot * local, must map the pivot point onto itself.
        let pivot = Vec3::new(0.29, 0.02, 0.0);
        let local = Action {
            cycle: 0.2,
            shots: 3,
        }
        .local_rot(PartAnim::Cylinder);
        let rot = weapon_rot(0.7, -0.2);
        let pos = Vec3::new(3.0, 1.4, -2.0);
        let shift = pivot - local * pivot;
        let placed = pos + rot * shift + (rot * local) * pivot;
        let expected = pos + rot * pivot;
        assert!(
            (placed - expected).length() < 1e-5,
            "{placed} vs {expected}"
        );
    }
    /// The `EMBER_ROUNDS` showcase draws every v20 shape, and draws all
    /// of it inside the view: the five rounds, the Lapua's two-rod
    /// streak with its round at the head, the AK's hole and the rocket's
    /// shrunk one, and the five cones of a flash star. Its offsets and
    /// its mesh scales are multiplied by `showcase_scale`, so every row
    /// keeps the same share of the view down the sights as at the hip;
    /// unscaled, row one ran off a 4:3 window at the narrowest sight
    /// (44 of 70 degrees), which is what the frustum half of this test
    /// pins. Nothing here can judge the copper, the taper or the star's
    /// proportions: that wants a capture.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn the_showcase_hangs_every_shape_in_front_of_the_eye_at_any_field() {
        let eye = Vec3::new(3.0, 1.45, -2.0);
        let look = Vec3::new(0.6, -0.2, 0.8).normalize();
        // The camera's right as the frame builds it, then its own up.
        let right3 = look.cross(Vec3::Y).normalize();
        let up = right3.cross(look).normalize();
        let rs = rounds::Rounds { base: 500 };
        // The hip, and the narrowest sight any weapon has (44 degrees).
        for fov in [feel::HIP_FOV, 44.0_f32] {
            let view_scale = showcase_scale(fov);
            let mut frame = Frame::default();
            push_showcase(&mut frame, rs, eye, look, right3, 5.0, fov);
            let n = |mesh: u32| frame.instances.iter().filter(|i| i.mesh == mesh).count();
            assert_eq!(
                frame.instances.len(),
                15,
                "5 rounds + 2 streak rods + the flying round + 2 holes + 5 flash cones"
            );
            for r in rounds::Round::ALL {
                let want = if r == rounds::Round::Lapua { 2 } else { 1 };
                assert_eq!(n(500 + r.offset()), want, "{r:?}");
            }
            // The streak is a core frustum with a cone tail behind it;
            // the star is five cones, so the streak mesh carries six.
            assert_eq!(n(500 + rounds::CORE_OFFSET), 1, "the core frustum");
            assert_eq!(n(500 + rounds::STREAK_OFFSET), 6, "the tail and the star");
            assert_eq!(n(500 + rounds::DISC_OFFSET), 2, "two holes");
            // A textured round is pushed untinted or the jacket is
            // double-tinted; the holes and the star wear their colours.
            for i in frame.instances.iter().filter(|i| {
                rounds::Round::ALL
                    .iter()
                    .any(|r| i.mesh == 500 + r.offset())
            }) {
                assert_eq!(i.color, Vec3::ONE, "the jacket is the colour");
            }
            // The rocket's blast mark is drawn at 0.1 m for the row, the
            // AK's hole at its own 24 mm, both times the view scale.
            let mut holes: Vec<f32> = frame
                .instances
                .iter()
                .filter(|i| i.mesh == 500 + rounds::DISC_OFFSET)
                .map(|i| i.scale.y * 2.0)
                .collect();
            holes.sort_by(f32::total_cmp);
            assert!((holes[0] - 0.0237 * view_scale).abs() < 1e-4, "{holes:?}");
            assert!((holes[1] - 0.1 * view_scale).abs() < 1e-4, "{holes:?}");
            // Everything sits in front of the eye, past the 0.1 m near
            // plane, and inside the field. A 4:3 window is the narrowest
            // the arena runs, so the half-height at a depth is
            // tan(fov/2) * depth and the half-width 4/3 of that.
            let half = (fov * 0.5).to_radians().tan();
            let lateral = half * SHOWCASE_DIST * 4.0 / 3.0;
            for i in &frame.instances {
                let to = i.position - eye;
                let d = to.dot(look);
                assert!(d > 0.1, "inside the near plane: {i:?}");
                assert!(to.dot(up).abs() < half * d, "off the top or bottom: {i:?}");
                assert!(to.dot(right3).abs() < lateral, "off the side: {i:?}");
            }
            // The instance origins are not the widest points: check the
            // two that are, row one's outer nose and the Lapua's nose in
            // row two. Both are lengths times the same view scale as the
            // window they sit in, so what fits at the hip fits down the
            // sights — which is the whole point of scaling the layout.
            let scale = rounds::ROUND_SCALE * view_scale;
            let row_one = (rounds::Round::ALL
                .iter()
                .map(|r| r.length() * scale + SHOWCASE_GAP * view_scale)
                .sum::<f32>()
                - SHOWCASE_GAP * view_scale)
                * 0.5;
            assert!(
                row_one < lateral,
                "row one runs off: {row_one} vs {lateral}"
            );
            let row_two = 0.30 * view_scale + rounds::Round::Lapua.length() * scale * 0.5;
            assert!(
                row_two < lateral,
                "row two runs off: {row_two} vs {lateral}"
            );
        }
    }
}
