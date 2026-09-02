//! Online arena shooter client: renders the server-authoritative match and
//! aims with relative mouse deltas under pointer lock (yaw plus a pitch the
//! server now honours), carrying the cyberpunk sidearm in one hand and a
//! shield in the other.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;

use arena_core::proto::{BState, C2S, PROTO_VERSION, PState, PlayerMeta, S2C, STATE_EVERY_TICKS};
use arena_core::shooter::{
    Decor, EYE_CROUCH, EYE_STAND, FIXED_DT, Level, MAX_HP, MAX_PITCH, MELEE_COOLDOWN, Obstacle,
    RELOAD_SECS, move_circle, stance_speed, step_vertical, weapon_name, weapon_stats,
};
use ember_engine::glam::{Quat, Vec2, Vec3};
use ember_engine::{Camera, EmberGame, Frame, InputState, Instance, KeyCode, MouseButton};
use serde::Deserialize;

use crate::props::{Prop, Props, tex};
use crate::sound::{Audio, Sfx};

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

/// The Blender-authored viewmodel: pistol parts + hands/arms parts.
#[derive(Clone, Default)]
pub struct Assets {
    pub gun: Vec<Part>,
    pub arms: Vec<Part>,
    /// Model-space muzzle tip, from the sidecar; where the flash sits.
    pub muzzle: Vec3,
}

/// The sidecar `tools/v15/build_viewmodel.py` writes beside the GLB:
/// per-part pivots and the muzzle tip, in engine space.
#[derive(Deserialize, Default)]
struct ViewmodelRig {
    #[serde(default)]
    pivots: HashMap<String, [f32; 3]>,
    #[serde(default)]
    muzzle: Option<[f32; 3]>,
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
            let mut assets = Assets {
                muzzle: rig.muzzle.map_or(LEGACY_MUZZLE, Vec3::from_array),
                ..Assets::default()
            };
            for p in parts {
                let pivot = rig.pivots.get(&p.name).copied().map(Vec3::from_array);
                // A moving part without a pivot would spin about the model
                // origin, which is the hold point on the grip: worse than
                // not moving. So no pivot, no animation.
                let anim = match (p.name.as_str(), pivot) {
                    (n, Some(_)) if n.starts_with("cylinder") => PartAnim::Cylinder,
                    ("hammer", Some(_)) => PartAnim::Hammer,
                    ("trigger", Some(_)) => PartAnim::Trigger,
                    _ => PartAnim::Fixed,
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
                if p.name.starts_with("arm") || p.name.starts_with("hand") {
                    assets.arms.push(part);
                } else {
                    assets.gun.push(part);
                }
            }
            tracing::info!(
                gun_parts = assets.gun.len(),
                arm_parts = assets.arms.len(),
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

/// Weapon-level accent color (the glow strip on the pistol).
const fn weapon_accent(level: u8) -> Vec3 {
    match level {
        3 => Vec3::new(1.0, 0.25, 0.20),
        2 => Vec3::new(1.0, 0.55, 0.15),
        _ => GLOW_BLUE,
    }
}

/// Draws a part list at one transform. `rot` is a full rotation rather than
/// a yaw so a weapon can tilt with its owner's aim elevation.
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

/// Rotation for a weapon held at `yaw` and looking up/down by `pitch`.
/// The model convention is +X forward / +Y up, and +Z is the right-hand
/// axis that lifts +X toward +Y, so elevation is a Z rotation applied in
/// model space before the world yaw.
fn weapon_rot(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_z(pitch)
}

/// The first-person melee: a butt-strike with the rifle. Keyframes of
/// (seconds since the press, forward, left, up, yaw, pitch) in the weapon's
/// own frame - metres and radians - linear between keys, nothing before the
/// first or after the last. The last key sits on `MELEE_COOLDOWN`, so the
/// weapon is back in the hold by the time the next swing can start. Local
/// only: the protocol carries no melee state for remote players.
const STRIKE: [(f32, f32, f32, f32, f32, f32); 5] = [
    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
    // Wind up: back and up, the muzzle swung across to the left.
    (0.14, -0.10, 0.06, 0.06, 0.85, 0.20),
    // The strike: forward and across, the muzzle dropping.
    (0.30, 0.24, 0.18, -0.05, 0.30, -0.25),
    (0.48, 0.08, 0.10, -0.02, 0.15, -0.08),
    (MELEE_COOLDOWN, 0.0, 0.0, 0.0, 0.0, 0.0),
];

/// Where the strike has the weapon `t` seconds after the press: an offset
/// (forward, left, up) and a (yaw, pitch) added to the hold. `None` when no
/// strike is in progress.
fn strike_pose(since: f32) -> Option<(Vec3, f32, f32)> {
    if !(0.0..MELEE_COOLDOWN).contains(&since) {
        return None;
    }
    let idx = STRIKE.iter().rposition(|key| key.0 <= since)?;
    let from = STRIKE[idx];
    let to = STRIKE[(idx + 1).min(STRIKE.len() - 1)];
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
}

impl OnlineConfig {
    fn opening_msgs(&self) -> Result<Vec<C2S>, String> {
        let action = match self.action.as_str() {
            "create" => C2S::CreateLobby {
                name: self.lobby.clone(),
                password: self.password.clone().filter(|p| !p.is_empty()),
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
const GLOW_BLUE: Vec3 = Vec3::new(0.20, 0.65, 1.00);

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
    /// Per-victim white damage-flash time remaining.
    flash: HashMap<u8, f32>,
    /// Impact spark particles: (position, velocity, ttl).
    particles: Vec<(Vec3, Vec3, f32)>,
}

impl ShooterGame {
    pub fn connect(cfg: &OnlineConfig, assets: Option<Assets>) -> Result<Self, String> {
        let opening_messages = cfg.opening_msgs()?;
        let chan = net::NetChan::connect(&cfg.url, &opening_messages)?;
        set_status("connecting…");
        Ok(Self {
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
            audio: Audio::new(),
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
            flash: HashMap::new(),
            particles: Vec::new(),
        })
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
        let gun = me
            .map(|p| {
                if p.reloading {
                    format!("{} ⟳", weapon_name(p.weapon))
                } else {
                    format!(
                        "{} {}/{}",
                        weapon_name(p.weapon),
                        p.ammo,
                        weapon_stats(p.weapon).mag
                    )
                }
            })
            .unwrap_or_default();
        format!("{hp}  {gun}   {list}   ({} in arena)", self.latest.len())
    }
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
                } => {
                    self.last_tick = tick;
                    self.pads_active = pads;
                    // ---- audio cues from state diffs (before overwrite) ----
                    {
                        // Bullet counts per owner; position tracks the
                        // NEWEST bullet (drives distance falloff).
                        let mut prev_counts: HashMap<u8, usize> = HashMap::new();
                        for b in &self.bullets {
                            *prev_counts.entry(b.owner).or_insert(0) += 1;
                        }
                        let mut curr: HashMap<u8, (usize, [f32; 2])> = HashMap::new();
                        for b in &bullets {
                            let e = curr.entry(b.owner).or_insert((0, [b.x, b.z]));
                            e.0 += 1;
                            e.1 = [b.x, b.z];
                        }
                        // Remote shots (mine are cued from ammo below).
                        for (&owner, &(n, pos)) in &curr {
                            if Some(owner) != self.my_id
                                && n > prev_counts.get(&owner).copied().unwrap_or(0)
                            {
                                let d = (Vec2::new(pos[0], pos[1]) - self.pred_pos).length();
                                sfx.push((Sfx::Shot, (0.45 * (1.0 - d / 40.0)).clamp(0.05, 0.45)));
                            }
                        }
                        // My own transitions, from authoritative state.
                        if let (Some(me), Some(new_me)) = (
                            self.my_id.and_then(|id| self.latest.get(&id)),
                            self.my_id
                                .and_then(|id| players.iter().find(|p| p.id == id)),
                        ) {
                            // A falling ammo count is not by itself a shot:
                            // a pad pickup reassigns the whole magazine
                            // (Rapid 12 -> Heavy 6) and a respawn resets it
                            // (12 -> 8) in the same state that flips alive.
                            // Requiring the weapon to be unchanged and the
                            // player alive on BOTH sides rejects those two
                            // without losing a real shot, since firing
                            // needs alive and the weapon only changes on
                            // pickup or respawn.
                            if new_me.ammo < me.ammo
                                && new_me.weapon == me.weapon
                                && me.alive
                                && new_me.alive
                            {
                                sfx.push((Sfx::Shot, 0.5)); // exact own-shot cue
                                // Recoil and muzzle flash hang off the same
                                // authoritative signal as the audio: a round
                                // that the SERVER agrees left the weapon.
                                self.shot_started = Some(self.time);
                                self.shots = self.shots.wrapping_add(1);
                            }
                            if new_me.hp < me.hp && new_me.alive {
                                sfx.push((Sfx::Hurt, 0.6));
                            }
                            if new_me.weapon > me.weapon {
                                sfx.push((Sfx::Upgrade, 0.55));
                                status_event = Some(format!(
                                    "⬆ weapon upgraded: {}!",
                                    weapon_name(new_me.weapon)
                                ));
                            }
                            if new_me.reloading && !me.reloading {
                                sfx.push((Sfx::Reload, 0.45));
                                self.reload_started = Some(self.time);
                            } else if !new_me.reloading {
                                self.reload_started = None;
                            }
                            // Hitmarker only when plausibly MINE: one of my
                            // bullets vanished AND an enemy lost hp.
                            let my_id = new_me.id;
                            let my_gone = curr.get(&my_id).map_or(0, |e| e.0)
                                < prev_counts.get(&my_id).copied().unwrap_or(0);
                            let hurt: Vec<(u8, f32, f32)> = players
                                .iter()
                                .filter(|p| {
                                    p.id != my_id
                                        && self.latest.get(&p.id).is_some_and(|old| p.hp < old.hp)
                                })
                                .map(|p| (p.id, p.x, p.z))
                                .collect();
                            if my_gone && !hurt.is_empty() {
                                sfx.push((Sfx::Hit, 0.35));
                                // Visual confirmation: crosshair marker plus
                                // a damage flash and spark burst on the victim.
                                self.hitmarker_t = 0.14;
                                for &(id, x, z) in &hurt {
                                    self.flash.insert(id, 0.18);
                                    for k in 0_u8..6 {
                                        let a = f32::from(k) * std::f32::consts::TAU / 6.0;
                                        self.particles.push((
                                            Vec3::new(x, 1.1, z),
                                            Vec3::new(a.cos() * 2.2, 1.8, a.sin() * 2.2),
                                            0.3,
                                        ));
                                    }
                                }
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
                                    y = stepped.0;
                                    vy = stepped.1;
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
                    if Some(killer) == self.my_id {
                        sfx.push((Sfx::Kill, 0.5));
                        // Big red confirm marker: the elimination register.
                        self.kill_t = 0.55;
                    } else if Some(victim) == self.my_id {
                        sfx.push((Sfx::Death, 0.55));
                    } else {
                        sfx.push((Sfx::Hit, 0.12));
                    }
                    let line = if Some(victim) == self.my_id {
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
                S2C::Pong { .. } | S2C::LobbyList { .. } => {}
            }
        }
        if self.chan.is_dead() && !self.lost {
            self.lost = true;
            set_status("connection lost — reload to play again");
        }
        // Play the queued cues under a per-frame budget.
        if !suppress_sfx && let Some(audio) = self.audio.as_ref() {
            for (s, v) in sfx.into_iter().take(6) {
                audio.play(s, v);
            }
        }

        // ---- ADS zoom (RMB): tighter FOV, damped sensitivity ----
        let aiming = input.mouse_down(MouseButton::Right);
        let zoom_target = if aiming { 1.0 } else { 0.0 };
        self.zoom += (zoom_target - self.zoom) * (1.0 - (-dt * 14.0).exp());

        // ---- first-person look: raw mouse deltas -> yaw/pitch ----
        let sens = LOOK_SENS * (1.0 - 0.45 * self.zoom);
        let (mdx, mdy) = input.mouse_delta();
        self.yaw += mdx * sens;
        self.pitch = (self.pitch - mdy * sens).clamp(-MAX_PITCH, MAX_PITCH);
        let (fz, fx) = self.yaw.sin_cos();
        self.aim = Vec2::new(fx, fz);
        let forward2 = self.aim;
        // Camera right (from look_at basis: cross(forward, up)).
        let right2 = Vec2::new(-fz, fx);

        // ---- stance + camera-relative movement ----
        let sprint = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
        let crouch = input.down(KeyCode::KeyC);
        // Held, like every other intent: there is no local toggle state that
        // a dropped input packet could leave disagreeing with the server.
        let shield = input.down(KeyCode::KeyQ);
        self.shield_raise +=
            ((if shield { 1.0 } else { 0.0 }) - self.shield_raise) * (1.0 - (-dt * 16.0).exp());
        let target_eye = if crouch { EYE_CROUCH } else { EYE_STAND };
        self.eye_h += (target_eye - self.eye_h) * (1.0 - (-dt * 12.0).exp());

        let mut mv = forward2 * input.axis(KeyCode::KeyS, KeyCode::KeyW)
            + right2 * input.axis(KeyCode::KeyA, KeyCode::KeyD);
        if mv.length_squared() > 1.0 {
            mv = mv.normalize();
        }
        let moving = mv.length_squared() > 0.01;
        let fire = input.mouse_down(MouseButton::Left);
        // A jump is a press, latched until the next send frame: sampling the
        // held key at 20 Hz dropped taps shorter than 50 ms outright, and a
        // held key re-launched the player off every surface they touched.
        let space = input.down(KeyCode::Space);
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
        let e_down = input.down(KeyCode::KeyE);
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
                reload: input.down(KeyCode::KeyR),
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
            let (y, vy, _grounded) = step_vertical(
                p,
                self.pred_y,
                self.pred_vy,
                self.pred_jump,
                dt,
                &self.obstacles,
            );
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
        let tab = input.down(KeyCode::Tab);
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

        // Shot-feel timers and impact particles.
        self.hitmarker_t = (self.hitmarker_t - dt).max(0.0);
        self.kill_t = (self.kill_t - dt).max(0.0);
        self.flash.retain(|_, t| {
            *t -= dt;
            *t > 0.0
        });
        self.particles.retain_mut(|(p, v, ttl)| {
            v.y -= 9.0 * dt;
            *p += *v * dt;
            *ttl -= dt;
            *ttl > 0.0
        });

        // ---- first-person camera at my PREDICTED position ----
        let my_pos = self.own_render;
        let (ps, pc) = self.pitch.sin_cos();
        let look = Vec3::new(fx * pc, ps, fz * pc);
        // The eye rides the predicted feet height, so jumping and standing
        // on a crate raise the view.
        let eye = Vec3::new(my_pos.x, self.render_y_own + self.eye_h + bob, my_pos.y);
        let camera = debug_camera().unwrap_or(Camera {
            eye,
            target: eye + look,
            fov_y_deg: 70.0 - 26.0 * self.zoom,
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
                push_shield(
                    &mut frame,
                    // Reaching as far forward as the gun hand does (0.55),
                    // and for the same reason: the body box is 1.0 across,
                    // so anything held closer than 0.5 is held INSIDE the
                    // torso and the plate's face never shows.
                    Vec3::new(pos.x, feet_y + hand_y + 0.20, pos.y)
                        + Vec3::new(aim.x, 0.0, aim.y) * 0.52
                        + Vec3::new(left.x, 0.0, left.y) * 0.34,
                    Quat::from_rotation_y(-aim.y.atan2(aim.x)),
                    SHIELD_PLATE,
                    fc * 0.85,
                );
            }
            if let Some(a) = &self.assets {
                let yaw = -aim.y.atan2(aim.x);
                // Remote weapons tilt with the owner's real aim elevation,
                // so a player shooting down off a container looks like it.
                push_parts(
                    &mut frame,
                    &a.gun,
                    hand,
                    weapon_rot(yaw, p.pitch),
                    accent,
                    Action::REST,
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

        // Impact sparks: short-lived glowing shards on confirmed hits.
        for (p, _, ttl) in &self.particles {
            frame.instances.push(
                Instance::new(
                    *p,
                    Vec3::splat(0.09 * (ttl / 0.3)),
                    Vec3::new(1.0, 0.62, 0.2),
                )
                .with_yaw(*ttl * 12.0),
            );
        }

        // Bullets: tracers along the server's real 3D path, extrapolation
        // bounded to ~2 state intervals so stalls don't fly them through
        // walls. A round is drawn as a streak stretched along its flight
        // direction with a hotter head, which reads as something moving
        // fast rather than as a floating cube.
        let age = self.bullets_age.min(0.12);
        for b in &self.bullets {
            let p = Vec3::new(b.x + b.vx * age, b.y + b.vy * age, b.z + b.vz * age);
            let v = Vec3::new(b.vx, b.vy, b.vz);
            let speed = v.length();
            if speed < 1e-3 {
                continue;
            }
            let dir = v / speed;
            // Scale is applied before rotation, so a box long in X becomes
            // a rod pointing along the flight direction.
            let rot = Quat::from_rotation_arc(Vec3::X, dir);
            // The trail is clamped so it cannot reach back through the
            // camera. Your own round is only ~0.77 from the eye on the
            // first state that carries it, so a fixed 0.68 tail would end
            // up inside the 0.1 near plane — and the scene pass does not
            // cull backfaces, so it would paint a solid block across the
            // middle of the screen, right over the crosshair.
            let back = 0.68f32.min(((p - eye).dot(dir) - 0.35).max(0.0));
            if back > 0.02 {
                frame.instances.push(
                    Instance::new(
                        p - dir * (back * 0.5),
                        Vec3::new(back, 0.075, 0.075),
                        GLOW_BLUE * 0.55,
                    )
                    .with_rot(rot),
                );
            }
            frame.instances.push(
                Instance::new(p, Vec3::new(0.22, 0.15, 0.15), Vec3::new(1.0, 0.95, 0.75))
                    .with_rot(rot),
            );
        }

        // ---- crosshair markers: hit (white X) and kill (red X, larger) ----
        if self.hitmarker_t > 0.0 || self.kill_t > 0.0 {
            let f3 = Vec3::new(fx, 0.0, fz);
            let right3 = Vec3::new(-fz, 0.0, fx);
            let up = Vec3::Y;
            let center = eye + Vec3::new(fx * pc, ps, fz * pc) * 1.2;
            let (off, size, col) = if self.kill_t > 0.0 {
                (0.045, 0.016, Vec3::new(1.0, 0.15, 0.1))
            } else {
                (0.028, 0.009, Vec3::new(1.0, 1.0, 1.0))
            };
            let _ = f3;
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                frame.instances.push(Instance::new(
                    center + right3 * (off * sx) + up * (off * sy),
                    Vec3::splat(size),
                    col,
                ));
            }
        }

        // ---- viewmodel: the sidearm in hand, plus muzzle flash ----
        if me_alive {
            let me_latest = self.my_id.and_then(|id| self.latest.get(&id));
            let my_weapon = me_latest.map_or(1, |p| p.weapon);
            let reloading = me_latest.is_some_and(|p| p.reloading);
            let accent = weapon_accent(my_weapon);
            let right3 = Vec3::new(-fz, 0.0, fx);

            // Reload animation: the gun dips and rolls out of the way.
            let reload_dip = if reloading {
                let t0 = self.reload_started.unwrap_or(self.time);
                let progress = ((self.time - t0) / RELOAD_SECS).clamp(0.0, 1.0);
                (progress * std::f32::consts::PI).sin() * 0.24
            } else {
                0.0
            };
            // Recoil across the weapon's own cooldown: a fast rise and a
            // slower settle, so a rapid weapon never fully recovers between
            // rounds and a heavy one does. Driven by the server-confirmed
            // shot rather than the trigger — holding fire on an empty
            // magazine, or during a reload, must not kick.
            let cooldown = weapon_stats(my_weapon).cooldown;
            let recoil = self.shot_started.map_or(0.0, |t0| {
                let k = ((self.time - t0) / cooldown).clamp(0.0, 1.0);
                if k < 0.16 {
                    k / 0.16
                } else {
                    let settle = (1.0 - k) / 0.84;
                    settle * settle
                }
            });
            // ADS rides the FULL look vector rather than its horizontal
            // part. That is what puts the sights on the shot line when
            // pitched; the old pose used horizontal forward plus a
            // `pitch * 0.10` nudge, so the gun and the bullet disagreed.
            // Offsets tuned for the v16 rifle, whose hold point is the top of
            // the pistol grip at the trigger: a bullpup, so the stock runs
            // 0.45 back from there toward the shoulder and the hold sits
            // further out than the revolver's did, or the receiver fills
            // the corner of the screen.
            let base = eye
                + look * (0.60 + 0.08 * self.zoom - 0.06 * recoil)
                + right3 * (0.20 * (1.0 - self.zoom) + 0.012)
                + Vec3::Y
                    * (-0.24 + 0.07 * self.zoom + bob * 0.4 * (1.0 - self.zoom) - reload_dip
                        + 0.03 * recoil);
            let yaw = -forward2.y.atan2(forward2.x);
            // Tilts with aim elevation, plus a muzzle-up kick per shot.
            let rot = weapon_rot(yaw, self.pitch + 0.16 * recoil);
            // The melee strike moves the whole hold, in the weapon's frame.
            let (base, rot) = match self
                .melee_started
                .and_then(|t0| strike_pose(self.time - t0))
            {
                Some((offset, yaw_add, pitch_add)) => (
                    base + look * offset.x - right3 * offset.y + Vec3::Y * offset.z,
                    weapon_rot(yaw + yaw_add, self.pitch + 0.16 * recoil + pitch_add),
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
                push_parts(&mut frame, &a.gun, base, rot, accent, action);
                push_parts(&mut frame, &a.arms, base, rot, accent, action);
                base + rot * a.muzzle
            } else {
                push_gun(&mut frame, base, forward2, accent);
                base + look * 0.95
            };
            // Muzzle flash on a round the server agrees left the weapon.
            // What stood here fired on `time % cooldown` while the trigger
            // was held — a free-running clock with no relationship to
            // whether a bullet was ever spawned or ammo remained.
            let flashing = self.shot_started.is_some_and(|t0| self.time - t0 < 0.045);
            if flashing {
                inst(
                    &mut frame,
                    muzzle,
                    Vec3::splat(0.14),
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
                push_shield(
                    &mut frame,
                    center,
                    weapon_rot(yaw, self.pitch),
                    SHIELD_PLATE * lerp(0.9, 1.0),
                    GUNMETAL * 1.6 + accent * 0.10,
                );
            }
        }

        frame
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
mod strike_tests {
    use super::*;

    #[test]
    fn the_strike_starts_and_ends_at_the_hold() {
        assert!(strike_pose(-0.01).is_none(), "nothing before the press");
        assert!(
            strike_pose(MELEE_COOLDOWN).is_none(),
            "nothing once the cooldown is over"
        );
        let (o, yaw, pitch) = strike_pose(0.0).unwrap();
        assert!(
            o.length() < 1e-6 && yaw.abs() < 1e-6 && pitch.abs() < 1e-6,
            "starts at the hold"
        );
        let (o, yaw, pitch) = strike_pose(MELEE_COOLDOWN - 1e-4).unwrap();
        assert!(
            o.length() < 1e-2 && yaw.abs() < 1e-2 && pitch.abs() < 1e-2,
            "back at the hold: {o} {yaw} {pitch}"
        );
    }

    #[test]
    fn the_strike_winds_up_back_then_thrusts_forward() {
        let (wind, ..) = strike_pose(0.14).unwrap();
        let (hit, ..) = strike_pose(0.30).unwrap();
        assert!(wind.x < 0.0, "winds up backward: {wind}");
        assert!(hit.x > 0.2, "strikes forward: {hit}");
        // Continuous at 60 Hz: no frame jumps more than a few centimetres.
        let mut prev = strike_pose(0.0).unwrap().0;
        let mut t = 1.0 / 60.0;
        while t < MELEE_COOLDOWN {
            let cur = strike_pose(t).unwrap().0;
            assert!(
                (cur - prev).length() < 0.05,
                "jump of {} at {t}",
                (cur - prev).length()
            );
            prev = cur;
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn the_strike_keys_are_in_order_and_end_on_the_cooldown() {
        for w in STRIKE.windows(2) {
            assert!(w[1].0 > w[0].0, "keys in time order");
        }
        assert!((STRIKE[STRIKE.len() - 1].0 - MELEE_COOLDOWN).abs() < 1e-6);
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
