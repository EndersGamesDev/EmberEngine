//! Online arena shooter client: renders the server-authoritative match and
//! aims with relative mouse deltas under pointer lock (yaw plus a pitch the
//! server now honours), carrying the cyberpunk sidearm in one hand and a
//! shield in the other.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;

use arena_core::proto::{BState, C2S, PROTO_VERSION, PState, PlayerMeta, S2C, STATE_EVERY_TICKS};
use arena_core::shooter::{
    Cover, Decor, EYE_CROUCH, EYE_STAND, FIXED_DT, Level, MAX_HP, MAX_PITCH, MELEE_COOLDOWN,
    Obstacle, Projectile, RESERVE_INFINITE, SIDEARM, WEAPON_COUNT, move_circle, stance_speed,
    step_vertical, weapon_name, weapon_stats,
};
use ember_engine::glam::{Quat, Vec2, Vec3};
use ember_engine::{
    Camera, EmberGame, Feedback, Frame, InputState, Instance, KeyCode, MouseButton, PadButton,
};
use serde::Deserialize;

use crate::feel::{self, Climb, Cue, GLOW_BLUE, Shake, weapon_feel};
use crate::props::{LOOT_SPENT_TINT, Prop, Props, tex};
use crate::sound::{Audio, BUDGET, Sfx, prioritize};

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
fn classify(name: &str) -> Slot {
    match name {
        "w_vityaz" => Slot::Weapon(2),
        "w_ak47" => Slot::Weapon(3),
        "w_m4" => Slot::Weapon(4),
        _ if name.starts_with("w_revolver_") => Slot::Weapon(5),
        "w_sniper" => Slot::Weapon(6),
        "w_rpg7" => Slot::Weapon(7),
        "w_rpg7_rocket" => Slot::Rocket,
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
/// None when unset or malformed (and always None on the web).
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
}

impl OnlineConfig {
    fn opening_msgs(&self) -> Result<Vec<C2S>, String> {
        let action = match self.action.as_str() {
            "create" => C2S::CreateLobby {
                name: self.lobby.clone(),
                password: self.password.clone().filter(|p| !p.is_empty()),
                map: self.map.clone(),
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
    part(
        Vec3::new(0.76, 0.09, 0.0),
        Vec3::new(0.16, 0.13, 0.13),
        BRONZE,
    );
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

/// One short-lived cube: sparks on a hit, shards and smoke from a blast,
/// the launch smoke behind a rocket. `life` is the ttl it started with, so
/// a size or colour can fade against it.
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

// These flags represent independent input, connection, animation, and UI state transitions.
#[allow(clippy::struct_excessive_bools)]
pub struct ShooterGame {
    chan: net::NetChan,
    my_id: Option<u8>,
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
            decor: Vec::new(),
            env_base: 0,
            rig_character: None,
            anim: HashMap::new(),
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
        }
    }

    /// Apply one event cue: raise the shake, queue the rumble, queue the
    /// sound. The camera's part of an event is a timer set by the caller.
    fn cue(&mut self, c: Cue, sfx: &mut Vec<(Sfx, f32)>) {
        self.shake.hit(c.shake);
        if let Some(r) = c.rumble {
            self.feedback.rumble(r.strong, r.weak, r.ms);
        }
        if let Some(s) = c.sfx {
            sfx.push(s);
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

    /// Spark burst on a body that was hit.
    fn sparks(&mut self, x: f32, y: f32, z: f32) {
        for k in 0_u8..6 {
            let a = f32::from(k) * std::f32::consts::TAU / 6.0;
            self.fx.push(Fx {
                pos: Vec3::new(x, y, z),
                vel: Vec3::new(a.cos() * 2.2, 1.8, a.sin() * 2.2),
                ttl: 0.3,
                life: 0.3,
                size: 0.09,
                color: Vec3::new(1.0, 0.62, 0.2),
                gravity: 9.0,
            });
        }
    }

    /// A rocket went off: one flash cube, twelve shards under gravity,
    /// eight smoke cubes rising. Directions are a fixed fan, not random:
    /// there is no RNG on the client and a burst does not need one.
    fn blast_fx(&mut self, at: Vec3) {
        self.fx.push(Fx {
            pos: at,
            vel: Vec3::ZERO,
            ttl: 0.08,
            life: 0.08,
            size: 2.2,
            color: Vec3::new(1.0, 0.85, 0.5),
            gravity: 0.0,
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

    fn handle_of(&self, id: u8) -> String {
        self.metas
            .get(&id)
            .map_or_else(|| format!("player {id}"), |m| m.handle.clone())
    }

    /// Full Tab-overlay scoreboard: frags and deaths, sorted.
    fn scoreboard_text(&self) -> String {
        let mut rows: Vec<&PState> = self.latest.values().collect();
        rows.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
        let mut s = format!("{:<20} {:>6} {:>7}\n", "PLAYER", "FRAGS", "DEATHS");
        s.push_str(&"─".repeat(35));
        s.push('\n');
        for p in rows {
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
        let pad = if self.pad_status_shown == "none" {
            String::new()
        } else {
            format!("   gamepad: {}", self.pad_status_shown)
        };
        format!(
            "{hp}  {gun}   {list}   ({} in arena){pad}",
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
        let mut sfx: Vec<(Sfx, f32)> = Vec::new();
        let mut drained: Vec<S2C> = Vec::new();
        while let Some(msg) = self.chan.poll() {
            drained.push(msg);
        }
        let suppress_sfx = drained.len() > 6;
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
                } => {
                    self.my_id = Some(id);
                    self.arena_half = arena_half;
                    // The same level the server built its lobby from, so
                    // prediction and authority resolve against identical
                    // cover; the seed is only what an unknown name falls
                    // back to.
                    let level = Level::named(&map, seed);
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
                    self.pops.clear();
                    self.history.clear();
                    self.reload_started = None;
                    self.was_alive = false; // first State snaps the prediction
                    for m in players {
                        self.metas.insert(m.id, m);
                    }
                    status_event = Some(
                        "in the arena — click to capture mouse · WASD move · Shift sprint · C crouch · Q shield (reflects!) · click fire".into(),
                    );
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
                } => {
                    self.last_tick = tick;
                    self.pads_active = pads;
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
                        // Remote shots (mine are cued from ammo below).
                        for (&owner, &(n, pos, weapon)) in &curr {
                            if Some(owner) != self.my_id
                                && n > prev_counts.get(&owner).copied().unwrap_or(0)
                            {
                                let d = (Vec2::new(pos[0], pos[1]) - self.pred_pos).length();
                                let f = weapon_feel(weapon);
                                let vol = (f.volume * 0.9 * (1.0 - d / 40.0)).clamp(0.05, f.volume);
                                sfx.push((f.sound, vol));
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
                                sfx.push((f.sound, f.volume));
                                self.feedback
                                    .rumble(f.rumble.strong, f.rumble.weak, f.rumble.ms);
                                self.shake.hit(f.launch_shake);
                                // Recoil and muzzle flash hang off the same
                                // authoritative signal as the audio: a round
                                // that the SERVER agrees left the weapon.
                                self.shot_started = Some(self.time);
                                self.shots = self.shots.wrapping_add(1);
                                if weapon_stats(new_me.weapon).kind == Projectile::Rocket {
                                    self.launch_smoke = true;
                                }
                            }
                            let changed = new_me.weapon != me.weapon && me.alive && new_me.alive;
                            if changed && new_me.weapon == SIDEARM {
                                // A looted gun ran dry: the sidearm is back.
                                self.holster_started = Some(self.time);
                                self.cue(feel::holster(), &mut sfx);
                            } else if changed && self.time - self.last_pop_at > 0.5 {
                                // A grant with no pop before it: a pad.
                                sfx.push((Sfx::Upgrade, 0.55));
                                status_event =
                                    Some(format!("⬆ picked up: {}", loadout_of(new_me.weapon)));
                            }
                            if new_me.reloading && !me.reloading {
                                self.cue(feel::reload_start(), &mut sfx);
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
                                    sfx.push((Sfx::Respawn, 0.4));
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
                        sfx.push((Sfx::Hit, 0.12));
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
                    } else if let Some(v) = self.latest.get(&victim).copied() {
                        // Visual confirmation on the body: a damage flash
                        // and a spark burst, for everyone watching.
                        self.flash.insert(victim, 0.18);
                        self.sparks(v.x, v.y + 1.1, v.z);
                    }
                }
                S2C::Blast { x, y, z, owner: _ } => {
                    let at = Vec3::new(x, y, z);
                    let eye = Vec3::new(self.pred_pos.x, self.pred_y + self.eye_h, self.pred_pos.y);
                    let d = (at - eye).length();
                    self.cue(feel::blast(d), &mut sfx);
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
                        self.cue(feel::pop(false), &mut sfx);
                    }
                }
                // The client never asks for a listing; a page's lobby
                // browser does, in plain JS. Logged so a stray one is seen.
                S2C::LobbyList { lobbies } => {
                    tracing::debug!(
                        maps = ?lobbies.iter().map(|l| l.map.as_str()).collect::<Vec<_>>(),
                        "unsolicited lobby list"
                    );
                }
                S2C::Pong { .. } => {}
            }
        }
        if self.chan.is_dead() && !self.lost {
            self.lost = true;
            set_status("connection lost — reload to play again");
        }
        // Play the queued cues under a per-frame budget, the important ones
        // first, so a crowded frame drops a footfall and never the boom.
        if !suppress_sfx && let Some(audio) = self.audio.as_ref() {
            prioritize(&mut sfx);
            for (s, v) in sfx.into_iter().take(BUDGET) {
                audio.play(s, v);
            }
        }

        // ---- the pad, merged with the keys: either device at any moment ----
        let pad = input.pad();
        if pad.is_some() && self.pad_status_shown != input.pad_status() {
            self.pad_status_shown = input.pad_status();
            status_event = Some(format!("gamepad: {}", self.pad_status_shown));
        }
        let pad_down = |b: PadButton| pad.is_some_and(|p| p.down(b));
        let stick_l = pad.map_or([0.0, 0.0], |p| p.left);
        let stick_r = pad.map_or([0.0, 0.0], |p| p.right);

        // ---- ADS (RMB or LT): tighter FOV, damped sensitivity ----
        let aiming = input.mouse_down(MouseButton::Right) || pad.is_some_and(|p| p.lt > 0.5);
        let zoom_target = if aiming { 1.0 } else { 0.0 };
        self.zoom += (zoom_target - self.zoom) * (1.0 - (-dt * 14.0).exp());

        // ---- first-person look: mouse deltas and the right stick -> yaw/pitch ----
        let sens = LOOK_SENS * (1.0 - 0.45 * self.zoom);
        let (mdx, mdy) = input.mouse_delta();
        let stick_sens = if aiming { 0.55 } else { 1.0 };
        self.yaw += mdx * sens + stick_r[0] * 2.8 * dt * stick_sens;
        self.pitch = (self.pitch - mdy * sens + stick_r[1] * 2.0 * dt * stick_sens)
            .clamp(-MAX_PITCH, MAX_PITCH);
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
        let sprint =
            input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight) || self.sprint_latch;
        let crouch = input.down(KeyCode::KeyC) || pad_down(PadButton::East);
        // Held, like every other intent: there is no local toggle state that
        // a dropped input packet could leave disagreeing with the server.
        let shield = input.down(KeyCode::KeyQ) || pad_down(PadButton::LB);
        self.shield_raise +=
            ((if shield { 1.0 } else { 0.0 }) - self.shield_raise) * (1.0 - (-dt * 16.0).exp());
        let target_eye = if crouch { EYE_CROUCH } else { EYE_STAND };
        self.eye_h += (target_eye - self.eye_h) * (1.0 - (-dt * 12.0).exp());

        // The left stick is already dead-zoned and curved by the platform.
        let mut mv = forward2 * (input.axis(KeyCode::KeyS, KeyCode::KeyW) + stick_l[1])
            + right2 * (input.axis(KeyCode::KeyA, KeyCode::KeyD) + stick_l[0]);
        if mv.length_squared() > 1.0 {
            mv = mv.normalize();
        }
        let moving = mv.length_squared() > 0.01;
        let fire = input.mouse_down(MouseButton::Left) || pad.is_some_and(|p| p.rt > 0.5);
        // The dry trigger: once per press, on an empty magazine that is not
        // being reloaded. Nothing goes to the server for it; the sim would
        // refuse the round anyway.
        if fire
            && !self.prev_fire
            && self
                .my_id
                .and_then(|id| self.latest.get(&id))
                .is_some_and(|p| p.alive && p.ammo == 0 && !p.reloading)
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
        let space = input.down(KeyCode::Space) || pad_down(PadButton::South);
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
        let e_down = input.down(KeyCode::KeyE) || pad_down(PadButton::RB);
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
                reload: input.down(KeyCode::KeyR) || pad_down(PadButton::West),
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
            let bonk = stepped
                .bonked
                .filter(|&k| self.prev_bonked != Some(k) && self.pred_vy > 0.0)
                .filter(|&k| self.obstacles.get(k).is_some_and(|o| o.kind == Cover::Loot))
                .and_then(|k| self.loot_slot(k));
            self.prev_bonked = stepped.bonked;
            if let Some(i) = bonk {
                let mut cues = Vec::new();
                if self.loot_active.get(i).copied().unwrap_or(true) {
                    self.loot_bump[i] = Some(self.time);
                    self.dip_started = Some(self.time);
                    self.cue(feel::bonk(), &mut cues);
                } else {
                    self.cue(feel::bonk_dead(), &mut cues);
                }
                if let Some(audio) = self.audio.as_ref() {
                    for (s, v) in cues {
                        audio.play(s, v);
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
        self.since_score_ui += dt;
        let tab = input.down(KeyCode::Tab) || pad_down(PadButton::Start);
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
        self.fx.retain_mut(|f| {
            f.vel.y -= f.gravity * dt;
            f.pos += f.vel * dt;
            f.ttl -= dt;
            f.ttl > 0.0
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
        let me_latest = self.my_id.and_then(|id| self.latest.get(&id)).copied();
        let my_weapon = shown_weapon(me_latest.map_or(SIDEARM, |p| p.weapon));
        let my_feel = weapon_feel(my_weapon);
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
            fov_y_deg: my_feel.fov(self.zoom),
        });

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
            let color = self
                .metas
                .get(&id)
                .map_or(Vec3::splat(0.6), |m| Vec3::from_array(m.color));
            let aim = Vec2::new(p.ax, p.az);
            let (body_h, head_y, hand_y, pip_y) = if p.crouch {
                (0.75, 0.95, 0.62, 1.5)
            } else {
                (1.1, 1.35, 0.85, 2.0)
            };
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
            // Jointed rig when parts loaded; textured/plain boxes else.
            if let Some(rc) = &self.rig_character {
                let prev = self.prev_pos.insert(id, pos).unwrap_or(pos);
                let vel = if dt > 0.0 {
                    (pos - prev) / dt
                } else {
                    Vec2::ZERO
                };
                let slot = self.anim.entry(id).or_insert((0.0, 0.0, 0.0));
                ember_engine::puppet::advance_anim(slot, vel, dt);
                let crouch = self.crouch_ease.entry(id).or_insert(0.0);
                let target = if p.crouch { 1.0 } else { 0.0 };
                *crouch += (target - *crouch) * (1.0 - (-10.0 * dt).exp());
                // Bodies face where the player AIMS (shooter convention).
                let aim_yaw = aim.x.atan2(aim.y);
                let pose =
                    ember_engine::rig::walk_pose(slot.1, slot.2, *crouch, self.time, &rc.dims);
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
            let hand =
                Vec3::new(pos.x, feet_y + hand_y, pos.y) + Vec3::new(aim.x, 0.0, aim.y) * 0.55;
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
            for h in 0..p.hp {
                inst(
                    &mut frame,
                    Vec3::new(pos.x - 0.3 + f32::from(h) * 0.3, feet_y + pip_y, pos.y),
                    Vec3::splat(0.16),
                    Vec3::new(0.3, 0.9, 0.4),
                );
            }
        }

        // Sparks, shards and smoke: opaque cubes, the only particle the
        // scene pass can draw. Shards shrink out; smoke swells and dims.
        for f in &self.fx {
            let k = (f.ttl / f.life).clamp(0.0, 1.0);
            let (edge, color) = if f.gravity > 0.0 {
                (f.size * k, f.color)
            } else {
                (f.size * (1.4 - 0.4 * k), f.color * (0.4 + 0.6 * k))
            };
            frame
                .instances
                .push(Instance::new(f.pos, Vec3::splat(edge), color).with_yaw(f.ttl * 12.0));
        }

        // Bullets: tracers along the server's real 3D path, extrapolation
        // bounded to ~2 state intervals so stalls don't fly them through
        // walls. A round is drawn as a streak stretched along its flight
        // direction with a hotter head, which reads as something moving
        // fast rather than as a floating cube; the rod's length, thickness
        // and colour are the shooter's weapon's, from `BState.weapon`. A
        // rocket is the rocket mesh flown along the path with an exhaust
        // rod behind it.
        let age = self.bullets_age.min(0.12);
        for b in &self.bullets {
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
            let rocket = weapon_stats(b.weapon).kind == Projectile::Rocket;
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
            if rocket {
                match self.assets.as_ref().filter(|a| !a.rocket.is_empty()) {
                    Some(a) => {
                        push_parts(&mut frame, &a.rocket, at, rot, row.accent, Action::REST);
                    }
                    None => frame.instances.push(
                        Instance::new(at, Vec3::new(0.5, 0.16, 0.16), Vec3::new(0.35, 0.4, 0.3))
                            .with_rot(rot),
                    ),
                }
            } else {
                frame.instances.push(
                    Instance::new(
                        at,
                        Vec3::new(row.head, row.head * 0.68, row.head * 0.68),
                        Vec3::new(1.0, 0.95, 0.75),
                    )
                    .with_rot(rot),
                );
            }
        }

        // ---- crosshair markers: hit (white X) and kill (red X, larger) ----
        if self.hitmarker_t > 0.0 || self.kill_t > 0.0 {
            let up = Vec3::Y;
            let center = eye + look * 1.2;
            let (off, edge, col) = if self.kill_t > 0.0 {
                (0.045, 0.016, Vec3::new(1.0, 0.15, 0.1))
            } else {
                (
                    0.028 * self.hitmarker_scale,
                    0.009 * self.hitmarker_scale,
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
            let muzzle = if let Some(a) = &self.assets {
                push_weapon(&mut frame, a, my_weapon, base, rot, action, loaded);
                push_parts(&mut frame, &a.arms, base, rot, accent, action);
                base + rot * a.muzzle_of(my_weapon)
            } else {
                push_gun(&mut frame, base, forward2, accent);
                base + look * 0.95
            };
            // The rocket's launch smoke: six grey cubes drifting back from
            // the muzzle, spawned on the frame the shot was confirmed.
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
                    });
                }
            }
            // The sniper's scope: four opaque black bars half a metre from
            // the eye framing the narrow field, once the zoom is mostly in.
            // Opaque because that is the one kind of cube there is; the
            // scene pass has no blending.
            if my_feel.ads_fov < 30.0 && self.zoom > 0.6 {
                let half_h = 0.5 * (my_feel.ads_fov.to_radians() * 0.5).tan();
                let half_w = half_h * input.aspect();
                let up = Vec3::Y;
                let c = eye + look * 0.5;
                let thick = 0.012;
                for (dx, dy, sx, sy) in [
                    (0.0, half_h, half_w * 2.4, thick),
                    (0.0, -half_h, half_w * 2.4, thick),
                    (half_w, 0.0, thick, half_h * 2.4),
                    (-half_w, 0.0, thick, half_h * 2.4),
                ] {
                    frame.instances.push(
                        Instance::new(
                            c + right3 * dx + up * dy,
                            Vec3::new(sx, sy, thick),
                            Vec3::splat(0.01),
                        )
                        .with_rot(Quat::from_rotation_arc(Vec3::Z, -look)),
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

            let flashing = self
                .shot_started
                .is_some_and(|t0| self.time - t0 < my_feel.flash_ms);
            if flashing {
                inst(
                    &mut frame,
                    muzzle,
                    Vec3::splat(my_feel.flash),
                    Vec3::new(1.0, 0.9, 0.5),
                );
            }
            // Aim dot floating on the sight line (occluded by walls,
            // which reads like a laser sight).
            inst(&mut frame, eye + look * 4.0, Vec3::splat(0.05), accent);

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
                            Err(tungstenite::Error::Io(e))
                                if e.kind() == std::io::ErrorKind::WouldBlock
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
            let (out_tx, out_rx) = mpsc::channel::<C2S>();
            let (_in_tx, in_rx) = mpsc::channel::<S2C>();
            (
                Self {
                    out_tx,
                    in_rx,
                    dead: Arc::new(AtomicBool::new(false)),
                },
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
}
