//! The game client: connects to an ember-server, sends movement intents,
//! renders every player as a colored cube in the arena. Falls back to a
//! local offline arena if the server is unreachable.
//!
//!     game [SERVER_ADDR] [NAME]

mod net;
mod world;

use std::time::{Duration, Instant};

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{Camera, EmberGame, EngineConfig, Frame, InputState, Instance, KeyCode};
use ember_net::{ClientMsg, ARENA_HALF, MOVE_SPEED};

use net::NetClient;
use world::World;

enum Session {
    Online(NetClient),
    /// Server unreachable: local-only arena so the game still runs.
    Offline { pos: Vec2 },
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
        // Ground slab, top surface at y = 0.
        frame.instances.push(Instance {
            position: Vec3::new(0.0, -0.5, 0.0),
            scale: Vec3::new(half * 2.0 + 2.0, 1.0, half * 2.0 + 2.0),
            color: Vec3::new(0.16, 0.17, 0.20),
        });
        // Arena corner markers.
        for &sx in &[-1.0f32, 1.0] {
            for &sz in &[-1.0f32, 1.0] {
                frame.instances.push(Instance {
                    position: Vec3::new(half * sx, 0.75, half * sz),
                    scale: Vec3::new(0.5, 1.5, 0.5),
                    color: Vec3::new(0.35, 0.37, 0.42),
                });
            }
        }
        frame
    }

    fn push_player(frame: &mut Frame, pos: Vec2, color: [f32; 3], is_me: bool) {
        let scale = if is_me { 1.15 } else { 1.0 };
        frame.instances.push(Instance {
            position: Vec3::new(pos.x, scale * 0.5, pos.y),
            scale: Vec3::splat(scale),
            color: Vec3::from_array(color),
        });
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
        EngineConfig { title },
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
