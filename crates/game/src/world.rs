//! Client-side view of the shared world, built from server messages.
//! Positions are interpolated between snapshots so 60 Hz network state
//! renders smoothly at any frame rate.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use ember_net::{PlayerId, PlayerMeta, PlayerState, ServerMsg, TICK_HZ};
use glam::Vec2;

struct Entry {
    meta: PlayerMeta,
    from: Vec2,
    to: Vec2,
    /// 0..1 interpolation progress from `from` to `to`.
    t: f32,
}

impl Entry {
    fn render_pos(&self) -> Vec2 {
        self.from.lerp(self.to, self.t.clamp(0.0, 1.0))
    }
}

pub struct World {
    pub my_id: Option<PlayerId>,
    pub arena_half: f32,
    players: HashMap<PlayerId, Entry>,
    pub last_tick: u64,
    /// When the most recent snapshot arrived; drives staleness detection.
    last_snapshot_at: Option<Instant>,
}

impl World {
    pub fn new(arena_half: f32) -> Self {
        Self {
            my_id: None,
            arena_half,
            players: HashMap::new(),
            last_tick: 0,
            last_snapshot_at: None,
        }
    }

    /// Time since the last snapshot arrived, once at least one has.
    pub fn snapshot_age(&self) -> Option<Duration> {
        self.last_snapshot_at.map(|t| t.elapsed())
    }

    pub fn add_meta(&mut self, meta: PlayerMeta) {
        let spawn = Vec2::from_array(meta.pos);
        self.players.entry(meta.id).or_insert(Entry {
            meta,
            from: spawn,
            to: spawn,
            t: 1.0,
        });
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn handle(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::PlayerJoined { meta } => {
                tracing::info!("{} joined ({:?})", meta.name, meta.id);
                self.add_meta(meta);
            }
            ServerMsg::PlayerLeft { id } => {
                if let Some(e) = self.players.remove(&id) {
                    tracing::info!("{} left ({id:?})", e.meta.name);
                }
            }
            ServerMsg::Snapshot { tick, players } => {
                // Ignore reordered/stale snapshots (can't happen over one TCP
                // stream, but cheap to guard).
                if tick < self.last_tick {
                    return;
                }
                self.last_tick = tick;
                self.last_snapshot_at = Some(Instant::now());
                for ps in players {
                    self.apply_state(ps);
                }
            }
            ServerMsg::Pong { .. } | ServerMsg::Welcome { .. } | ServerMsg::Reject { .. } => {}
        }
    }

    fn apply_state(&mut self, ps: PlayerState) {
        let target = Vec2::from_array(ps.pos);
        match self.players.get_mut(&ps.id) {
            Some(e) => {
                e.from = e.render_pos();
                e.to = target;
                e.t = 0.0;
            }
            None => {
                // Snapshot mentioned someone we have no meta for (shouldn't
                // happen, but stay resilient): show a gray placeholder.
                tracing::debug!("snapshot for unknown {:?}", ps.id);
                self.players.insert(
                    ps.id,
                    Entry {
                        meta: PlayerMeta {
                            id: ps.id,
                            name: "?".into(),
                            color: [0.6, 0.6, 0.6],
                            pos: ps.pos,
                        },
                        from: target,
                        to: target,
                        t: 1.0,
                    },
                );
            }
        }
    }

    /// Advance interpolation. One snapshot interval = full from->to blend.
    pub fn advance(&mut self, dt: f32) {
        let step = dt * TICK_HZ as f32;
        for e in self.players.values_mut() {
            e.t = (e.t + step).min(1.0);
        }
    }

    /// (position, color, is_me) for every player, for rendering.
    pub fn render_players(&self) -> impl Iterator<Item = (Vec2, [f32; 3], bool)> + '_ {
        self.players.values().map(move |e| {
            (
                e.render_pos(),
                e.meta.color,
                Some(e.meta.id) == self.my_id,
            )
        })
    }
}
