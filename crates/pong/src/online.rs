//! Online arena shooter client: renders the server-authoritative match,
//! aims with the mouse (cursor unprojected onto the ground plane), and
//! carries the cyberpunk sidearm — rebuilt out of cubes: gunmetal slide,
//! bronze barrel and trigger guard, glowing blue circuit strip.

use std::collections::{HashMap, VecDeque};

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{Camera, EmberGame, Frame, InputState, Instance, KeyCode, MouseButton};
use pong_core::proto::{BState, PState, PlayerMeta, C2S, PROTO_VERSION, S2C, STATE_EVERY_TICKS};
use pong_core::shooter::{
    generate_arena, generate_pads, move_circle, stance_speed, weapon_name, weapon_stats, Obstacle,
    MAX_HP, RELOAD_SECS,
};
use serde::Deserialize;

use crate::sound::{Audio, Sfx};

/// One colored piece of a loaded GLB model.
#[derive(Clone)]
pub(crate) struct Part {
    pub mesh: u32,
    pub color: Vec3,
    pub is_strip: bool,
}

/// The Blender-authored viewmodel: pistol parts + hands/arms parts.
#[derive(Clone, Default)]
pub(crate) struct Assets {
    pub gun: Vec<Part>,
    pub arms: Vec<Part>,
}

const VIEWMODEL_GLB: &[u8] = include_bytes!("../assets/viewmodel.glb");

/// Load the GLB into engine meshes + part lists. Falls back to the classic
/// cube pistol when the asset is missing/broken.
pub(crate) fn load_assets() -> (Vec<ember_engine::MeshData>, Option<Assets>) {
    match ember_engine::assets::load_glb(VIEWMODEL_GLB) {
        Ok(parts) => {
            let mut meshes = Vec::new();
            let mut assets = Assets::default();
            for p in parts {
                let part = Part {
                    mesh: meshes.len() as u32 + 1, // 0 is the built-in cube
                    color: Vec3::from_array(p.color),
                    is_strip: p.name == "strip",
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

/// Environment textures, embedded so the wasm build ships them (arena v8).
const TEX_FLOOR: &[u8] = include_bytes!("../../../assets/textures/floor_basalt.png");
const TEX_WALL: &[u8] = include_bytes!("../../../assets/textures/wall_basalt.png");
const TEX_ARMOR: &[u8] = include_bytes!("../../../assets/textures/player_armor.png");

fn tex(bytes: &[u8], name: &str) -> Option<ember_engine::TextureData> {
    match ember_engine::TextureData::from_png_bytes(bytes) {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!(name, "texture decode failed ({e}); untextured");
            None
        }
    }
}

/// Textured environment meshes, registered after the viewmodel GLB parts:
/// env_base + 0 = floor plane (12x tiles), + 1 = wall/cover box (4x),
/// + 2 = armor box (players, pads).
pub(crate) fn env_meshes() -> Vec<ember_engine::MeshData> {
    use ember_engine::MeshData;
    vec![
        MeshData::textured_plane(12.0, tex(TEX_FLOOR, "floor_basalt")),
        MeshData::textured_box(4.0, tex(TEX_WALL, "wall_basalt")),
        MeshData::textured_box(1.0, tex(TEX_ARMOR, "player_armor")),
    ]
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

/// Build the jointed rig character from the embedded GLBs, registered
/// starting at `first_mesh`. All five or none.
pub(crate) fn part_meshes(
    first_mesh: u32,
) -> (
    Vec<ember_engine::MeshData>,
    Option<ember_engine::rig::RigCharacter>,
) {
    let mut meshes = Vec::new();
    let mut sources = Vec::new();
    for (i, (bytes, _target_h)) in PART_GLBS.iter().enumerate() {
        match ember_engine::rig::source_from_glb_bytes(bytes, first_mesh + i as u32) {
            Ok((mesh, src)) => {
                meshes.push(mesh);
                sources.push(src);
            }
            Err(e) => {
                tracing::warn!("character part {i} unusable ({e}); box bodies");
                return (Vec::new(), None);
            }
        }
    }
    let rc = ember_engine::rig::veteran_rig(
        sources[0], sources[1], sources[2], sources[3], sources[4], None,
    );
    tracing::info!("jointed rig character assembled ({} parts)", rc.parts.len());
    (meshes, Some(rc))
}

/// Weapon-level accent color (the glow strip on the pistol).
fn weapon_accent(level: u8) -> Vec3 {
    match level {
        3 => Vec3::new(1.0, 0.25, 0.20),
        2 => Vec3::new(1.0, 0.55, 0.15),
        _ => GLOW_BLUE,
    }
}

fn push_parts(frame: &mut Frame, parts: &[Part], pos: Vec3, yaw: f32, accent: Vec3) {
    for p in parts {
        let color = if p.is_strip { accent } else { p.color };
        frame.instances.push(
            Instance::new(pos, Vec3::ONE, color)
                .with_yaw(yaw)
                .with_mesh(p.mesh),
        );
    }
}

/// Mouse-look sensitivity, radians per pixel.
const LOOK_SENS: f32 = 0.0026;
const EYE_STAND: f32 = 1.45;
const EYE_CROUCH: f32 = 0.85;

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
fn set_scoreboard(text: Option<&str>) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("scoreboard"))
        {
            match text {
                Some(t) => {
                    el.set_text_content(Some(t));
                    let _ = el.remove_attribute("hidden");
                }
                None => {
                    let _ = el.set_attribute("hidden", "");
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = text;
}

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

/// Deterministic cosmetic obstacle height (the sim is 2D; every client
/// derives the same height from the obstacle's own coordinates).
fn obstacle_height(o: &Obstacle) -> f32 {
    let h = (o.min[0] * 7.31 + o.max[1] * 3.17 + o.max[0] * 1.13).abs();
    1.7 + (h - h.floor()) * 1.6
}

#[derive(Clone, Copy, Default)]
struct PSnap {
    x: f32,
    z: f32,
}

/// One sent input command, kept until the server acks it — the base of
/// client-side movement prediction.
struct Cmd {
    seq: u32,
    mv: [f32; 2],
    speed: f32,
    sent_at: f32,
}

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
    own_render: Vec2,
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
    /// Local time when my current reload started (drives the viewmodel dip).
    reload_started: Option<f32>,
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
    /// First mesh id of env_meshes() (floor, wall, armor); 0 = untextured.
    env_base: u32,
    /// Jointed player character; None = textured/plain boxes.
    rig_character: Option<ember_engine::rig::RigCharacter>,
    /// Per-player (yaw, walk_phase, amplitude) + previous render position.
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
    /// Visual (height, vertical velocity) per bullet, parallel to `bullets`:
    /// MY bullets fly along my full aim ray (pitch included) visually while
    /// the authoritative 2D path stays the server's.
    bullet_vis: Vec<(f32, f32)>,
}

impl ShooterGame {
    pub fn connect(cfg: &OnlineConfig, assets: Option<Assets>) -> Result<Self, String> {
        let chan = net::NetChan::connect(&cfg.url, cfg.opening_msgs()?)?;
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
            own_render: Vec2::ZERO,
            was_alive: false,
            history: VecDeque::new(),
            next_seq: 1,
            last_tick: 0,
            audio: Audio::new(),
            assets,
            pads_pos: Vec::new(),
            pads_active: Vec::new(),
            zoom: 0.0,
            reload_started: None,
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
            env_base: 0,
            rig_character: None,
            anim: HashMap::new(),
            prev_pos: HashMap::new(),
            crouch_ease: HashMap::new(),
            hitmarker_t: 0.0,
            kill_t: 0.0,
            flash: HashMap::new(),
            particles: Vec::new(),
            bullet_vis: Vec::new(),
        })
    }

    /// Where env_meshes() got registered (set by run_online after load).
    pub fn set_env_base(&mut self, base: u32) {
        self.env_base = base;
    }

    /// Install the jointed character (set by run_online after load).
    pub fn set_parts(&mut self, rc: Option<ember_engine::rig::RigCharacter>) {
        self.rig_character = rc;
    }

    fn render_pos(&self, id: u8) -> Vec2 {
        let a = self.t.clamp(0.0, 1.0);
        let f = self.from.get(&id).copied().unwrap_or_default();
        let to = self.to.get(&id).copied().unwrap_or_default();
        Vec2::new(f.x + (to.x - f.x) * a, f.z + (to.z - f.z) * a)
    }

    fn handle_of(&self, id: u8) -> String {
        self.metas
            .get(&id)
            .map(|m| m.handle.clone())
            .unwrap_or_else(|| format!("player {id}"))
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
            s.push_str(&format!(
                "{me}{name:<16} {:>6} {:>7}{state}\n",
                p.score, p.deaths
            ));
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
        for msg in drained {
            match msg {
                S2C::Welcome { .. } => set_status("connected…"),
                S2C::GameJoined {
                    id,
                    seed,
                    arena_half,
                    players,
                } => {
                    self.my_id = Some(id);
                    self.arena_half = arena_half;
                    self.obstacles = generate_arena(seed);
                    self.pads_pos = generate_pads(seed);
                    self.pads_active = vec![true; self.pads_pos.len()];
                    self.history.clear();
                    self.reload_started = None;
                    self.was_alive = false; // first State snaps the prediction
                    for m in players {
                        self.metas.insert(m.id, m);
                    }
                    status_event = Some(
                        "in the arena — click to capture mouse · WASD move · Shift sprint · C crouch · click fire".into(),
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
                            if new_me.ammo < me.ammo {
                                sfx.push((Sfx::Shot, 0.5)); // exact own-shot cue
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
                            let my_gone = curr.get(&my_id).map(|e| e.0).unwrap_or(0)
                                < prev_counts.get(&my_id).copied().unwrap_or(0);
                            let hurt: Vec<(u8, f32, f32)> = players
                                .iter()
                                .filter(|p| {
                                    p.id != my_id
                                        && self
                                            .latest
                                            .get(&p.id)
                                            .map(|old| p.hp < old.hp)
                                            .unwrap_or(false)
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
                                    for k in 0..6 {
                                        let a = k as f32 * std::f32::consts::TAU / 6.0;
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
                        let snap = match self.to.get(&p.id) {
                            Some(prev_to) => {
                                let cur = self.render_pos(p.id);
                                let (dx, dz) = (p.x - prev_to.x, p.z - prev_to.z);
                                if dx * dx + dz * dz > 6.0 * 6.0 {
                                    PSnap { x: p.x, z: p.z } // respawn teleport
                                } else {
                                    PSnap { x: cur.x, z: cur.y }
                                }
                            }
                            None => PSnap { x: p.x, z: p.z },
                        };
                        new_from.insert(p.id, snap);
                    }
                    self.from = new_from;
                    self.to = players
                        .iter()
                        .map(|p| (p.id, PSnap { x: p.x, z: p.z }))
                        .collect();
                    self.latest = players.into_iter().map(|p| (p.id, p)).collect();
                    self.t = 0.0;
                    // Carry per-bullet visual heights across states (order is
                    // stable server-side: retain + append), matching by
                    // predicted position; new bullets near ME inherit my
                    // aim pitch so the tracer flies where the crosshair points.
                    let carry_age = self.bullets_age;
                    let mut used = vec![false; self.bullets.len()];
                    let mut new_vis = Vec::with_capacity(bullets.len());
                    for b in &bullets {
                        let mut vis = None;
                        for (i, ob) in self.bullets.iter().enumerate() {
                            if used[i] {
                                continue;
                            }
                            let px = ob.x + ob.vx * carry_age;
                            let pz = ob.z + ob.vz * carry_age;
                            let (dx, dz) = (b.x - px, b.z - pz);
                            if dx * dx + dz * dz < 2.25 {
                                used[i] = true;
                                let (y0, vy) =
                                    self.bullet_vis.get(i).copied().unwrap_or((1.25, 0.0));
                                vis = Some((y0 + vy * carry_age, vy));
                                break;
                            }
                        }
                        new_vis.push(vis.unwrap_or_else(|| {
                            let (dx, dz) = (b.x - self.own_render.x, b.z - self.own_render.y);
                            if dx * dx + dz * dz < 2.25 {
                                let h_speed = (b.vx * b.vx + b.vz * b.vz).sqrt();
                                (self.eye_h - 0.06, h_speed * self.pitch.tan())
                            } else {
                                (1.25, 0.0)
                            }
                        }));
                    }
                    self.bullet_vis = new_vis;
                    self.bullets = bullets;
                    self.bullets_age = 0.0;

                    // Reconcile my prediction: rebase on the authoritative
                    // position and replay every not-yet-acked command.
                    if let Some(my) = self.my_id.and_then(|id| self.latest.get(&id)) {
                        let server = Vec2::new(my.x, my.z);
                        let newly_alive = my.alive && !self.was_alive;
                        self.was_alive = my.alive;
                        if my.alive {
                            while self.history.front().is_some_and(|c| c.seq <= my.ack) {
                                self.history.pop_front();
                            }
                            let mut p = [server.x, server.y];
                            let mut it = self.history.iter().peekable();
                            while let Some(c) = it.next() {
                                let end = it.peek().map(|n| n.sent_at).unwrap_or(self.time);
                                let dur = (end - c.sent_at).clamp(0.0, 0.3);
                                p = move_circle(p, c.mv, c.speed, dur, &self.obstacles);
                            }
                            let rebased = Vec2::new(p[0], p[1]);
                            if newly_alive || rebased.distance(self.pred_pos) > 4.0 {
                                // Respawn / teleport: snap everything.
                                self.pred_pos = server;
                                self.own_render = server;
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
        if !suppress_sfx {
            if let Some(audio) = self.audio.as_ref() {
                for (s, v) in sfx.into_iter().take(6) {
                    audio.play(s, v);
                }
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
        self.pitch = (self.pitch - mdy * sens).clamp(-1.45, 1.45);
        let (fz, fx) = self.yaw.sin_cos();
        self.aim = Vec2::new(fx, fz);
        let forward2 = self.aim;
        // Camera right (from look_at basis: cross(forward, up)).
        let right2 = Vec2::new(-fz, fx);

        // ---- stance + camera-relative movement ----
        let sprint = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
        let crouch = input.down(KeyCode::KeyC);
        let target_eye = if crouch { EYE_CROUCH } else { EYE_STAND };
        self.eye_h += (target_eye - self.eye_h) * (1.0 - (-dt * 12.0).exp());

        let mut mv = forward2 * input.axis(KeyCode::KeyS, KeyCode::KeyW)
            + right2 * input.axis(KeyCode::KeyA, KeyCode::KeyD);
        if mv.length_squared() > 1.0 {
            mv = mv.normalize();
        }
        let moving = mv.length_squared() > 0.01;
        let fire = input.mouse_down(MouseButton::Left) || input.down(KeyCode::Space);

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
            .map(|p| p.alive)
            .unwrap_or(false);
        let speed = stance_speed(sprint, crouch);

        // Send intents at a fixed cadence (also the keepalive), remembering
        // each command until the server acks it.
        if self.my_id.is_some() && !self.lost && self.since_input >= 0.05 {
            self.since_input = 0.0;
            let seq = self.next_seq;
            self.next_seq = self.next_seq.wrapping_add(1);
            if me_alive {
                self.history.push_back(Cmd {
                    seq,
                    mv: [mv.x, mv.y],
                    speed,
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
                fire,
                sprint,
                crouch,
                reload: input.down(KeyCode::KeyR),
            });
        }

        // Predict my own movement locally — instant response; the State
        // handler above rebases this on the server's authority.
        if me_alive {
            let p = move_circle(
                self.pred_pos.to_array(),
                [mv.x, mv.y],
                speed,
                dt,
                &self.obstacles,
            );
            self.pred_pos = Vec2::new(p[0], p[1]);
        }
        // Tight smoothing absorbs reconciliation nudges without adding lag.
        let k = 1.0 - (-dt * 25.0).exp();
        self.own_render += (self.pred_pos - self.own_render) * k;
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

        self.t += dt * (60.0 / STATE_EVERY_TICKS as f32);

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
        let eye = Vec3::new(my_pos.x, self.eye_h + bob, my_pos.y);
        let camera = Camera {
            eye,
            target: eye + look,
            fov_y_deg: 70.0 - 26.0 * self.zoom,
        };

        // ---- build the scene ----
        let mut frame = Frame {
            camera,
            instances: Vec::with_capacity(96),
        };
        let half = self.arena_half;
        let inst = |frame: &mut Frame, p: Vec3, s: Vec3, c: Vec3| {
            frame.instances.push(Instance::new(p, s, c));
        };

        // Floor + enclosing walls (tall enough to feel like a room).
        // env_base > 0: textured basalt set (arena v8); else the classic flats.
        let env = self.env_base;
        inst(
            &mut frame,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(half * 2.0 + 2.0, 1.0, half * 2.0 + 2.0),
            Vec3::new(0.12, 0.13, 0.17),
        );
        if env > 0 {
            frame.instances.push(
                Instance::new(
                    Vec3::new(0.0, 0.004, 0.0),
                    Vec3::new(half * 2.0 + 2.0, 1.0, half * 2.0 + 2.0),
                    Vec3::ONE,
                )
                .with_mesh(env),
            );
        }
        for (px, pz, sx, sz) in [
            (half + 0.45, 0.0, 0.9, half * 2.0 + 2.7),
            (-half - 0.45, 0.0, 0.9, half * 2.0 + 2.7),
            (0.0, half + 0.45, half * 2.0 + 2.7, 0.9),
            (0.0, -half - 0.45, half * 2.0 + 2.7, 0.9),
        ] {
            if env > 0 {
                frame.instances.push(
                    Instance::new(
                        Vec3::new(px, 1.75, pz),
                        Vec3::new(sx, 3.5, sz),
                        Vec3::splat(0.95),
                    )
                    .with_mesh(env + 1),
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

        // Obstacles from the shared seed, with deterministic cosmetic
        // height variation (cover you can crouch behind, blocks you can't
        // see over).
        for o in &self.obstacles {
            let cx = (o.min[0] + o.max[0]) * 0.5;
            let cz = (o.min[1] + o.max[1]) * 0.5;
            let h = obstacle_height(o);
            let pos = Vec3::new(cx, h * 0.5, cz);
            let size = Vec3::new(o.max[0] - o.min[0], h, o.max[1] - o.min[1]);
            if env > 0 {
                frame
                    .instances
                    .push(Instance::new(pos, size, Vec3::splat(0.85)).with_mesh(env + 1));
            } else {
                inst(&mut frame, pos, size, Vec3::new(0.30, 0.33, 0.40));
            }
        }

        // Other players (my own body is the camera).
        for (&id, p) in &self.latest {
            if !p.alive || Some(id) == self.my_id {
                continue;
            }
            let pos = self.render_pos(id);
            let color = self
                .metas
                .get(&id)
                .map(|m| Vec3::from_array(m.color))
                .unwrap_or(Vec3::splat(0.6));
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
                Vec3::new(pos.x, 0.02, pos.y),
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
                    aim_yaw,
                    [fc.x, fc.y, fc.z],
                    0.95,
                );
            } else if env > 0 {
                frame.instances.push(
                    Instance::new(
                        Vec3::new(pos.x, body_h * 0.5, pos.y),
                        Vec3::new(1.0, body_h, 1.0),
                        color,
                    )
                    .with_mesh(env + 2),
                );
                frame.instances.push(
                    Instance::new(
                        Vec3::new(pos.x, head_y, pos.y),
                        Vec3::splat(0.55),
                        color * 0.7,
                    )
                    .with_mesh(env + 2),
                );
            } else {
                inst(
                    &mut frame,
                    Vec3::new(pos.x, body_h * 0.5, pos.y),
                    Vec3::new(1.0, body_h, 1.0),
                    color,
                );
                inst(
                    &mut frame,
                    Vec3::new(pos.x, head_y, pos.y),
                    Vec3::splat(0.55),
                    color * 0.7,
                );
            }
            let hand = Vec3::new(pos.x, hand_y, pos.y) + Vec3::new(aim.x, 0.0, aim.y) * 0.55;
            let accent = weapon_accent(p.weapon);
            if let Some(a) = &self.assets {
                let yaw = -aim.y.atan2(aim.x);
                push_parts(&mut frame, &a.gun, hand, yaw, accent);
            } else {
                push_gun(&mut frame, hand, aim, accent);
            }
            for h in 0..p.hp {
                inst(
                    &mut frame,
                    Vec3::new(pos.x - 0.3 + h as f32 * 0.3, pip_y, pos.y),
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

        // Bullets: glowing tracers near eye height, extrapolation bounded
        // to ~2 state intervals so stalls don't fly them through walls.
        let age = self.bullets_age.min(0.12);
        for (i, b) in self.bullets.iter().enumerate() {
            let (y0, vy) = self.bullet_vis.get(i).copied().unwrap_or((1.25, 0.0));
            let y = (y0 + vy * age).max(0.12);
            inst(
                &mut frame,
                Vec3::new(b.x + b.vx * age, y, b.z + b.vz * age),
                Vec3::splat(0.26),
                GLOW_BLUE,
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
            let my_weapon = me_latest.map(|p| p.weapon).unwrap_or(1);
            let reloading = me_latest.map(|p| p.reloading).unwrap_or(false);
            let accent = weapon_accent(my_weapon);
            let f3 = Vec3::new(fx, 0.0, fz);
            let right3 = Vec3::new(-fz, 0.0, fx);

            // Reload animation: the gun dips and rolls out of the way.
            let reload_dip = if reloading {
                let t0 = self.reload_started.unwrap_or(self.time);
                let progress = ((self.time - t0) / RELOAD_SECS).clamp(0.0, 1.0);
                (progress * std::f32::consts::PI).sin() * 0.24
            } else {
                0.0
            };
            // ADS pulls the gun to screen center and closer to the eye.
            let base = eye
                + f3 * (0.5 + 0.10 * self.zoom)
                + right3 * (0.24 * (1.0 - self.zoom) + 0.015)
                + Vec3::new(
                    0.0,
                    -0.30 + 0.075 * self.zoom + bob * 0.4 * (1.0 - self.zoom) + self.pitch * 0.10
                        - reload_dip,
                    0.0,
                );
            let yaw = -forward2.y.atan2(forward2.x);
            if let Some(a) = &self.assets {
                push_parts(&mut frame, &a.gun, base, yaw, accent);
                push_parts(&mut frame, &a.arms, base, yaw, accent);
            } else {
                push_gun(&mut frame, base, forward2, accent);
            }
            // Muzzle flash synced to the fire cooldown cadence.
            let cooldown = weapon_stats(my_weapon).cooldown;
            if fire && !reloading && (self.time % cooldown) < 0.06 {
                inst(
                    &mut frame,
                    base + f3 * 0.95 + Vec3::new(0.0, 0.1, 0.0),
                    Vec3::splat(0.14),
                    Vec3::new(1.0, 0.9, 0.5),
                );
            }
            // Aim dot floating on the sight line (occluded by walls,
            // which reads like a laser sight).
            inst(&mut frame, eye + look * 4.0, Vec3::splat(0.05), accent);
        }

        frame
    }
}

// ---- platform-split WebSocket channel ----

#[cfg(not(target_arch = "wasm32"))]
mod net {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::Arc;
    use std::time::Duration;

    use pong_core::proto::{C2S, S2C};
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::Message;

    pub struct NetChan {
        out_tx: Sender<C2S>,
        in_rx: Receiver<S2C>,
        dead: Arc<AtomicBool>,
    }

    impl NetChan {
        pub fn connect(url: &str, initial: Vec<C2S>) -> Result<NetChan, String> {
            // rustls needs an explicitly installed crypto provider (both
            // backends are compiled into the tree). Err = already installed.
            let _ = rustls::crypto::ring::default_provider().install_default();
            let (mut ws, _) = tungstenite::connect(url).map_err(|e| format!("connect: {e}"))?;
            match ws.get_ref() {
                MaybeTlsStream::Plain(s) => {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(20)));
                }
                MaybeTlsStream::Rustls(s) => {
                    let _ = s
                        .get_ref()
                        .set_read_timeout(Some(Duration::from_millis(20)));
                }
                _ => {}
            }
            for msg in &initial {
                let text = serde_json::to_string(msg).map_err(|e| e.to_string())?;
                ws.send(Message::text(text))
                    .map_err(|e| format!("send: {e}"))?;
            }

            let (out_tx, out_rx) = mpsc::channel::<C2S>();
            let (in_tx, in_rx) = mpsc::channel::<S2C>();
            let dead = Arc::new(AtomicBool::new(false));
            {
                let dead = Arc::clone(&dead);
                std::thread::spawn(move || loop {
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
                                let _ = ws.close(None);
                                return;
                            }
                        }
                    }
                    match ws.read() {
                        Ok(Message::Text(t)) => {
                            if let Ok(msg) = serde_json::from_str::<S2C>(t.as_str()) {
                                if in_tx.send(msg).is_err() {
                                    return;
                                }
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
                });
            }
            Ok(NetChan {
                out_tx,
                in_rx,
                dead,
            })
        }

        pub fn send(&mut self, msg: &C2S) {
            let _ = self.out_tx.send(clone_c2s(msg));
        }

        pub fn poll(&mut self) -> Option<S2C> {
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

    use pong_core::proto::{C2S, S2C};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    pub struct NetChan {
        ws: web_sys::WebSocket,
        inbox: Rc<RefCell<VecDeque<S2C>>>,
        open: Rc<Cell<bool>>,
        dead: Rc<Cell<bool>>,
        /// Messages queued until the socket opens.
        pending: Rc<RefCell<Vec<String>>>,
        _callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
        _on_msg: Closure<dyn FnMut(web_sys::MessageEvent)>,
    }

    impl NetChan {
        pub fn connect(url: &str, initial: Vec<C2S>) -> Result<NetChan, String> {
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
                        let _ = ws2.send_with_str(&text);
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

            Ok(NetChan {
                ws,
                inbox,
                open,
                dead,
                pending,
                _callbacks: callbacks,
                _on_msg: on_msg,
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
}
