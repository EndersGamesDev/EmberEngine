//! The game client: connects to an ember-server, sends movement intents,
//! renders every player as a colored cube in the arena. Falls back to a
//! local offline arena if the server is unreachable.
//!
//!     game [SERVER_ADDR] [NAME]

mod character;
mod net;
mod props;
mod world;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use character::{push_character, walk_speed_to_phase_delta};

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{
    Camera, EmberGame, EngineConfig, Frame, InputState, Instance, KeyCode, MeshData, MeshVertex,
    TextureData,
};
use ember_net::{ClientMsg, ARENA_HALF, MOVE_SPEED};

use net::NetClient;
use world::World;

enum Session {
    Online(NetClient),
    /// Server unreachable: local-only arena so the game still runs.
    Offline { pos: Vec2 },
}

// Registered mesh ids (0 is the engine's built-in cube).
const MESH_FLOOR: u32 = 1;
const MESH_WALL: u32 = 2;
/// First character part mesh (head); torso and limb follow (see character.rs).
const MESH_CHAR: u32 = 3;
/// First mesh id after the fixed set; GLB monument parts land here.
const MESH_MONUMENT: u32 = 6;

/// Load the articulated part character: five GLBs in assets/models/parts/
/// (head, torso, arm, leg, boot — arm/leg/boot shared by both sides).
/// All five must load; otherwise the caller falls back.
fn load_part_character(
    first_mesh: u32,
) -> (Vec<MeshData>, Option<character::PartCharacter>) {
    // (file stem, target world height).
    const PARTS: [(&str, f32); 5] = [
        ("part-head", 0.34),
        ("part-torso", 0.68),
        ("part-arm", 0.66),
        ("part-leg", 0.55),
        ("part-boot", 0.19),
    ];
    let mut meshes = Vec::new();
    let mut infos = Vec::new();
    for (i, (stem, target_h)) in PARTS.iter().enumerate() {
        let candidates = [
            format!("{}/../../assets/models/parts/{stem}.glb", env!("CARGO_MANIFEST_DIR")),
            format!("assets/models/parts/{stem}.glb"),
        ];
        let mut loaded = None;
        for path in candidates {
            if let Ok(bytes) = std::fs::read(&path) {
                match ember_engine::assets::load_glb(&bytes) {
                    Ok(parts) => {
                        // Merge multi-part GLBs into one mesh.
                        let mut merged = MeshData::default();
                        for p in parts {
                            merged.vertices.extend(p.mesh.vertices);
                        }
                        loaded = Some(merged);
                        break;
                    }
                    Err(e) => tracing::error!(path, error = %e, "part GLB load failed"),
                }
            }
        }
        let Some(mesh) = loaded else {
            tracing::info!(stem, "part GLB missing; articulated character unavailable");
            return (Vec::new(), None);
        };
        let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &mesh.vertices {
            for a in 0..3 {
                min[a] = min[a].min(v.pos[a]);
                max[a] = max[a].max(v.pos[a]);
            }
        }
        let h = (max[1] - min[1]).max(1e-3);
        infos.push(character::MeshPart {
            mesh: first_mesh + i as u32,
            scale: target_h / h,
            center: [
                (min[0] + max[0]) * 0.5,
                (min[1] + max[1]) * 0.5,
                (min[2] + max[2]) * 0.5,
            ],
        });
        meshes.push(mesh);
    }
    let mut it = infos.into_iter();
    let pc = character::PartCharacter {
        head: it.next().unwrap(),
        torso: it.next().unwrap(),
        arm: it.next().unwrap(),
        leg: it.next().unwrap(),
        boot: it.next().unwrap(),
    };
    tracing::info!("articulated part character loaded (5 parts)");
    (meshes, Some(pc))
}

/// Load the AI-generated full-body character (assets/models/character.glb),
/// returning its meshes plus bounds for feet/height normalization.
fn load_mesh_character(first_mesh: u32) -> (Vec<MeshData>, Option<character::MeshCharacter>) {
    let candidates = [
        format!("{}/../../assets/models/character.glb", env!("CARGO_MANIFEST_DIR")),
        "assets/models/character.glb".to_string(),
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            match ember_engine::assets::load_glb(&bytes) {
                Ok(parts) => {
                    let meshes: Vec<MeshData> = parts.into_iter().map(|p| p.mesh).collect();
                    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
                    for m in &meshes {
                        for v in &m.vertices {
                            min_y = min_y.min(v.pos[1]);
                            max_y = max_y.max(v.pos[1]);
                        }
                    }
                    if max_y <= min_y {
                        tracing::error!(path, "character GLB has no vertical extent");
                        return (Vec::new(), None);
                    }
                    tracing::info!(path, parts = meshes.len(), height = max_y - min_y, "character GLB loaded");
                    let mc = character::MeshCharacter {
                        first_mesh,
                        parts: meshes.len() as u32,
                        feet_y: min_y,
                        height: max_y - min_y,
                        yaw_offset: std::f32::consts::PI, // tuned: concept faces camera
                    };
                    return (meshes, Some(mc));
                }
                Err(e) => tracing::error!(path, error = %e, "character GLB load failed"),
            }
        }
    }
    tracing::info!("no character GLB; using the blocky humanoid");
    (Vec::new(), None)
}

/// Load the AI-generated helmet monument GLB (assets/models/helmet.glb),
/// returning its part meshes; empty when absent (arena renders without it).
fn load_monument() -> Vec<MeshData> {
    let candidates = [
        format!("{}/../../assets/models/helmet.glb", env!("CARGO_MANIFEST_DIR")),
        "assets/models/helmet.glb".to_string(),
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            match ember_engine::assets::load_glb(&bytes) {
                Ok(parts) => {
                    tracing::info!(path, parts = parts.len(), "monument GLB loaded");
                    return parts.into_iter().map(|p| p.mesh).collect();
                }
                Err(e) => tracing::error!(path, error = %e, "monument GLB load failed"),
            }
        }
    }
    tracing::warn!("no monument GLB; arena renders without it");
    Vec::new()
}

/// Tries assets/textures/<name> relative to the workspace, then the cwd.
/// Missing or broken files degrade to untextured rendering, never a crash.
fn load_texture(name: &str) -> Option<TextureData> {
    let candidates = [
        format!("{}/../../assets/textures/{name}", env!("CARGO_MANIFEST_DIR")),
        format!("assets/textures/{name}"),
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            match TextureData::from_png_bytes(&bytes) {
                Ok(t) => {
                    tracing::info!(path, w = t.width, h = t.height, "texture loaded");
                    return Some(t);
                }
                Err(e) => tracing::error!(path, error = %e, "texture decode failed"),
            }
        }
    }
    tracing::warn!(name, "texture missing; rendering untextured");
    None
}

/// Axis-aligned unit box, every face UV-tiled `tiles` times.
fn box_mesh(tiles: f32, texture: Option<TextureData>) -> MeshData {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let mut vertices = Vec::with_capacity(36);
    for (n, u, v) in faces {
        let n3 = Vec3::from(n);
        let u3 = Vec3::from(u);
        let v3 = Vec3::from(v);
        let center = n3 * 0.5;
        let corners = [
            center - u3 * 0.5 - v3 * 0.5,
            center + u3 * 0.5 - v3 * 0.5,
            center + u3 * 0.5 + v3 * 0.5,
            center - u3 * 0.5 + v3 * 0.5,
        ];
        for idx in [0usize, 1, 2, 0, 2, 3] {
            vertices.push(MeshVertex {
                pos: corners[idx].to_array(),
                normal: n,
                uv: [uvs[idx][0] * tiles, uvs[idx][1] * tiles],
            });
        }
    }
    MeshData { vertices, texture }
}

/// Flat unit plane at y = 0 facing +Y, UVs tiled `tiles` times.
fn plane_mesh(tiles: f32, texture: Option<TextureData>) -> MeshData {
    let corners = [
        Vec3::new(-0.5, 0.0, -0.5),
        Vec3::new(0.5, 0.0, -0.5),
        Vec3::new(0.5, 0.0, 0.5),
        Vec3::new(-0.5, 0.0, 0.5),
    ];
    let uvs = [[0.0, 0.0], [tiles, 0.0], [tiles, tiles], [0.0, tiles]];
    let mut vertices = Vec::with_capacity(6);
    for idx in [0usize, 1, 2, 0, 2, 3] {
        vertices.push(MeshVertex {
            pos: corners[idx].to_array(),
            normal: [0.0, 1.0, 0.0],
            uv: uvs[idx],
        });
    }
    MeshData { vertices, texture }
}

/// A snapshot gap above this (at 60 Hz ≈ 16 ms cadence) is a lag spike.
const SNAPSHOT_STALE_AFTER: Duration = Duration::from_millis(300);
/// Round-trip times above this get flagged.
const HIGH_RTT_MS: u32 = 250;

struct Game {
    session: Session,
    world: World,
    last_dir: [f32; 2],
    last_input_sent: Instant,
    reported_disconnect: bool,
    frames: u64,
    last_status_log: Instant,
    /// Most recent measured round-trip time (from keepalive Ping/Pong).
    last_rtt_ms: Option<u32>,
    /// Set while the snapshot stream is stale (lag spike in progress).
    stale_since: Option<Instant>,
    /// Per-player (facing_yaw, walk_phase, swing_amplitude) animation state.
    anim: HashMap<ember_net::PlayerId, (f32, f32, f32)>,
    /// Animation state for the offline local player.
    offline_anim: (f32, f32, f32),
    /// Static cover props for the current arena layout.
    layouts: Option<props::Layouts>,
    /// Number of monument GLB part meshes registered after MESH_MONUMENT.
    monument_parts: u32,
    /// AI-generated full-body player mesh; None = blocky humanoid fallback.
    mesh_character: Option<character::MeshCharacter>,
    /// Articulated five-part AI character (preferred when present).
    part_character: Option<character::PartCharacter>,
}

impl Game {
    fn arena_frame(&self, camera: Camera) -> Frame {
        let mut frame = Frame { camera, instances: Vec::new() };
        let half = self.world.arena_half;
        let span = half * 2.0 + 2.0;
        // Ground slab, top surface at y = 0 (dark base under the floor).
        frame.instances.push(Instance::new(Vec3::new(0.0, -0.5, 0.0), Vec3::new(span, 1.0, span), Vec3::new(0.16, 0.17, 0.20)));
        // Textured basalt floor overlay.
        frame.instances.push(
            Instance::new(Vec3::new(0.0, 0.005, 0.0), Vec3::new(span, 1.0, span), Vec3::ONE)
                .with_mesh(MESH_FLOOR),
        );
        // Perimeter walls (textured), overlapping at the corners.
        for &s in &[-1.0f32, 1.0] {
            frame.instances.push(
                Instance::new(Vec3::new(0.0, 1.25, (half + 1.0) * s), Vec3::new(span + 1.2, 2.5, 0.6), Vec3::ONE)
                    .with_mesh(MESH_WALL),
            );
            frame.instances.push(
                Instance::new(Vec3::new((half + 1.0) * s, 1.25, 0.0), Vec3::new(span + 1.2, 2.5, 0.6), Vec3::ONE)
                    .with_mesh(MESH_WALL)
                    .with_yaw(std::f32::consts::FRAC_PI_2),
            );
        }
        // Arena corner markers.
        for &sx in &[-1.0f32, 1.0] {
            for &sz in &[-1.0f32, 1.0] {
                frame.instances.push(Instance::new(Vec3::new(half * sx, 0.75, half * sz), Vec3::new(0.5, 1.5, 0.5), Vec3::new(0.35, 0.37, 0.42)));
            }
        }
        // Cover props (stone kinds use the wall mesh, metal kinds the torso mesh).
        if let Some(layouts) = &self.layouts {
            props::push_props(&mut frame, props::pick(layouts), MESH_WALL, MESH_CHAR + 1);
        }
        // AI-generated helmet monument at the arena center.
        for part in 0..self.monument_parts {
            frame.instances.push(
                Instance::new(Vec3::new(0.0, 2.2, 0.0), Vec3::splat(3.0), Vec3::new(0.82, 0.84, 0.88))
                    .with_mesh(MESH_MONUMENT + part),
            );
        }
        frame
    }

    /// Best available character: articulated parts > single mesh > boxes.
    #[allow(clippy::too_many_arguments)]
    fn push_best_character(
        &self,
        frame: &mut Frame,
        pos: Vec2,
        yaw: f32,
        color: [f32; 3],
        is_me: bool,
        phase: f32,
        amp: f32,
    ) {
        if let Some(pc) = &self.part_character {
            let scale = if is_me { 1.1 } else { 1.0 };
            ember_engine::puppet::push_character_parts(
                frame, pos, yaw, color, scale, phase, amp, false, pc,
            );
        } else if let Some(mc) = &self.mesh_character {
            character::push_character_mesh(frame, pos, yaw, color, is_me, phase, mc);
        } else {
            push_character(frame, pos, yaw, color, is_me, phase, MESH_CHAR);
        }
    }

    /// Advance one animation slot from a velocity: smoothed facing (shortest
    /// arc), walk phase, and an eased swing amplitude for soft starts/stops.
    fn advance_anim(slot: &mut (f32, f32, f32), vel: Vec2, dt: f32) -> (f32, f32, f32) {
        let speed = vel.length();
        let moving = speed > 0.05;
        if moving {
            let target = vel.x.atan2(vel.y);
            let mut diff = target - slot.0;
            while diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            }
            while diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            slot.0 += diff * (1.0 - (-12.0 * dt).exp());
            slot.1 += walk_speed_to_phase_delta(speed, dt);
        }
        let amp_target = if moving { 1.0 } else { 0.0 };
        slot.2 += (amp_target - slot.2) * (1.0 - (-9.0 * dt).exp());
        *slot
    }
}

impl EmberGame for Game {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        self.frames += 1;

        // WASD / arrows on the XZ plane. Screen-up (W) is -Z: away from the
        // default camera, which sits on +Z looking at the origin.
        let mut dir = Vec2::new(
            input.axis(KeyCode::KeyA, KeyCode::KeyD)
                + input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight),
            input.axis(KeyCode::KeyW, KeyCode::KeyS)
                + input.axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
        );
        if dir.length_squared() > 1.0 {
            dir = dir.normalize();
        }
        let dir = [dir.x, dir.y];

        match &mut self.session {
            Session::Online(net) => {
                for msg in net.rx.try_iter() {
                    if let ember_net::ServerMsg::Pong { nonce } = msg {
                        // Keepalive pings carry a send timestamp as nonce.
                        let rtt = net.elapsed_ms().saturating_sub(nonce);
                        self.last_rtt_ms = Some(rtt);
                        if rtt > HIGH_RTT_MS {
                            tracing::warn!(rtt_ms = rtt, "network lag: high round-trip time");
                        }
                    } else {
                        self.world.handle(msg);
                    }
                }

                // Staleness: the server streams snapshots at 60 Hz, so a
                // long gap means the link or the server is stalling.
                if !net.is_dead() {
                    if let Some(age) = self.world.snapshot_age() {
                        if age > SNAPSHOT_STALE_AFTER {
                            if self.stale_since.is_none() {
                                self.stale_since = Some(Instant::now());
                                tracing::warn!(
                                    age_ms = age.as_millis() as u64,
                                    "snapshot stream stale: no server state received"
                                );
                            }
                        } else if let Some(since) = self.stale_since.take() {
                            tracing::info!(
                                outage_ms = since.elapsed().as_millis() as u64,
                                "snapshot stream recovered"
                            );
                        }
                    }
                }
                // Re-send on change, plus a periodic keepalive well under the
                // server's timeout.
                if dir != self.last_dir
                    || self.last_input_sent.elapsed() > Duration::from_millis(300)
                {
                    self.last_dir = dir;
                    self.last_input_sent = Instant::now();
                    let _ = net.send(&ClientMsg::Input { move_dir: dir });
                }
                if net.is_dead() && !self.reported_disconnect {
                    self.reported_disconnect = true;
                    tracing::error!("disconnected from server; world is frozen (restart to reconnect)");
                }
                self.world.advance(dt);

                if self.last_status_log.elapsed() > Duration::from_secs(5) {
                    self.last_status_log = Instant::now();
                    tracing::info!(
                        players = self.world.player_count(),
                        server_tick = self.world.last_tick,
                        rtt_ms = self.last_rtt_ms,
                        "online"
                    );
                }

                let mut frame = self.arena_frame(Camera::default());
                let mut seen: Vec<(ember_net::PlayerId, Vec2, Vec2, [f32; 3], bool)> =
                    self.world.render_players().collect();
                for (id, pos, vel, color, is_me) in seen.drain(..) {
                    let slot = self.anim.entry(id).or_insert((0.0, 0.0, 0.0));
                    let (yaw, phase, amp) = Self::advance_anim(slot, vel, dt);
                    self.push_best_character(&mut frame, pos, yaw, color, is_me, phase, amp);
                }
                frame
            }
            Session::Offline { pos } => {
                let vel = Vec2::from_array(dir) * MOVE_SPEED;
                *pos += vel * dt;
                *pos = pos.clamp(Vec2::splat(-ARENA_HALF), Vec2::splat(ARENA_HALF));
                let me = *pos;
                let mut frame = self.arena_frame(Camera::default());
                let mut slot = self.offline_anim;
                let (yaw, phase, amp) = Self::advance_anim(&mut slot, vel, dt);
                self.offline_anim = slot;
                self.push_best_character(&mut frame, me, yaw, [0.9, 0.9, 0.9], true, phase, amp);
                frame
            }
        }
    }
}

fn main() {
    // Tracing pipeline (RUST_LOG-compatible); idempotent with run()'s init.
    ember_engine::init_diagnostics();

    let mut args = std::env::args().skip(1);
    let addr = args
        .next()
        .unwrap_or_else(|| format!("127.0.0.1:{}", ember_net::DEFAULT_PORT));
    let name = args
        .next()
        .or_else(|| std::env::var("USERNAME").ok())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "player".into());

    let (session, world, title) = match NetClient::connect(&addr, &name) {
        Ok((net, welcome)) => {
            tracing::info!(
                "connected to {addr} as {:?} \"{name}\" ({} online, {} Hz)",
                welcome.id,
                welcome.roster.len(),
                welcome.tick_hz
            );
            let mut world = World::new(welcome.arena_half);
            world.my_id = Some(welcome.id);
            for meta in welcome.roster {
                world.add_meta(meta);
            }
            (
                Session::Online(net),
                world,
                format!("ember — {name} @ {addr}"),
            )
        }
        Err(e) => {
            tracing::warn!("could not reach server {addr} ({e}); running OFFLINE");
            (
                // Spawn clear of the center monument.
                Session::Offline { pos: Vec2::new(6.0, 6.0) },
                World::new(ARENA_HALF),
                "ember — OFFLINE".to_string(),
            )
        }
    };

    let monument = load_monument();
    let monument_parts = monument.len() as u32;
    let (char_meshes, mesh_character) =
        load_mesh_character(MESH_MONUMENT + monument_parts);
    let (part_meshes, part_character) = load_part_character(
        MESH_MONUMENT + monument_parts + char_meshes.len() as u32,
    );
    let mut meshes = vec![
        plane_mesh(12.0, load_texture("floor_basalt.png")),
        box_mesh(4.0, load_texture("wall_basalt.png")),
        // Character parts (see character.rs): head, torso, limb.
        box_mesh(1.0, load_texture("char_head.png").or_else(|| load_texture("player_armor.png"))),
        box_mesh(1.0, load_texture("char_torso.png").or_else(|| load_texture("player_armor.png"))),
        box_mesh(1.0, load_texture("char_limb.png").or_else(|| load_texture("player_armor.png"))),
    ];
    meshes.extend(monument);
    meshes.extend(char_meshes);
    meshes.extend(part_meshes);

    ember_engine::run(
        EngineConfig {
            title,
            meshes,
            ..Default::default()
        },
        Game {
            session,
            world,
            last_dir: [0.0, 0.0],
            last_input_sent: Instant::now(),
            reported_disconnect: false,
            frames: 0,
            last_status_log: Instant::now(),
            last_rtt_ms: None,
            stale_since: None,
            anim: HashMap::new(),
            offline_anim: (0.0, 0.0, 0.0),
            layouts: props::load_layouts(),
            monument_parts,
            mesh_character,
            part_character,
        },
    );
}
