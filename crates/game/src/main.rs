//! The game client: connects to an ember-server, sends movement intents,
//! renders every player as a colored cube in the arena. Falls back to a
//! local offline arena if the server is unreachable.
//!
//!     game [SERVER_ADDR] [NAME]

mod net;
mod world;

use std::time::{Duration, Instant};

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
const MESH_PLAYER: u32 = 3;

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
        frame
    }

    fn push_player(frame: &mut Frame, pos: Vec2, color: [f32; 3], is_me: bool) {
        let scale = if is_me { 1.15 } else { 1.0 };
        frame.instances.push(
            Instance::new(Vec3::new(pos.x, scale * 0.5, pos.y), Vec3::splat(scale), Vec3::from_array(color))
                .with_mesh(MESH_PLAYER),
        );
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
                for (pos, color, is_me) in self.world.render_players() {
                    Self::push_player(&mut frame, pos, color, is_me);
                }
                frame
            }
            Session::Offline { pos } => {
                *pos += Vec2::from_array(dir) * MOVE_SPEED * dt;
                *pos = pos.clamp(Vec2::splat(-ARENA_HALF), Vec2::splat(ARENA_HALF));
                let me = *pos;
                let mut frame = self.arena_frame(Camera::default());
                Self::push_player(&mut frame, me, [0.9, 0.9, 0.9], true);
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
                Session::Offline { pos: Vec2::ZERO },
                World::new(ARENA_HALF),
                "ember — OFFLINE".to_string(),
            )
        }
    };

    ember_engine::run(
        EngineConfig {
            title,
            meshes: vec![
                plane_mesh(12.0, load_texture("floor_basalt.png")),
                box_mesh(4.0, load_texture("wall_basalt.png")),
                box_mesh(1.0, load_texture("player_armor.png")),
            ],
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
        },
    );
}
