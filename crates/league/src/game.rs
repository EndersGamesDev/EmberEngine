//! The local match: the authoritative sim right here in the executable,
//! one human seat, the rest bots. It is the practice mode the page offers
//! without a server, and it is what `league-app` runs with no arguments.
//!
//! Input mapping lives here as a shared free function, because the online
//! game reads the very same controls and must produce the very same
//! commands; the only difference is who runs the world.

use ember_engine::{EmberGame, Frame, InputState, KeyCode, MouseButton};
use league_core::ai;
use league_core::proto::{Cmd, Phase, S2C};
use league_core::sim::Match;

use crate::scene::{self, camera_for, ground_point, project};
use crate::world::{feed_line, FxLite, World};

/// Commands queued by the page (`cmd_json`); both game modes drain this.
pub mod uiq {
    use std::sync::Mutex;

    static QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());

    /// The page calls this between frames; wasm is single-threaded and the
    /// engine loop runs on rAF, so it never races `update`.
    pub fn push(json: String) {
        if let Ok(mut q) = QUEUE.lock() {
            if q.len() < 256 {
                q.push(json);
            }
        }
    }

    pub fn drain() -> Vec<String> {
        QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
    }
}

/// The shared control map: cursor and keys in, `Cmd`s out. `my_alive`
/// gates orders that only a living champion can give.
#[must_use]
pub fn read_input(
    input: &InputState,
    prev: &mut Prev,
    world: &World,
    aspect: f32,
    my_alive: bool,
) -> Vec<Cmd> {
    let mut out = Vec::new();
    let camera = camera_for(world.cam);
    let cursor = input.cursor_ndc();

    // right button: the MOBA cursor — attack what is under it, else walk
    let rmb = input.mouse_down(MouseButton::Right);
    let lmb = input.mouse_down(MouseButton::Left);
    let rmb_edge = rmb && !prev.rmb;
    let lmb_edge = lmb && !prev.rmb_was_left;
    prev.rmb = rmb;
    prev.rmb_was_left = lmb;

    if my_alive && (rmb_edge || lmb_edge) {
        if let Some(ndc) = cursor {
            if let Some(target) = pick_enemy(&camera, aspect, world, ndc) {
                out.push(Cmd::Attack { target });
            } else if let Some((x, z)) = ground_point(&camera, aspect, ndc) {
                out.push(Cmd::Move { x, z });
            }
        }
    }

    // Q W E R / D F / 1-6, all casts aimed at the cursor point
    if my_alive {
        if let Some(ndc) = cursor {
            let aim = ground_point(&camera, aspect, ndc);
            for (idx, key) in [
                (0usize, KeyCode::KeyQ),
                (1, KeyCode::KeyW),
                (2, KeyCode::KeyE),
                (3, KeyCode::KeyR),
            ] {
                let down = input.down(key);
                if down && !prev.abil[idx] {
                    if let Some((x, z)) = aim {
                        out.push(Cmd::Cast { slot: idx as u8, x, z });
                    }
                }
                prev.abil[idx] = down;
            }
            for (idx, key) in [(0usize, KeyCode::KeyD), (1, KeyCode::KeyF)] {
                let down = input.down(key);
                if down && !prev.spell[idx] {
                    if let Some((x, z)) = aim {
                        out.push(Cmd::Spell { slot: idx as u8, x, z });
                    }
                }
                prev.spell[idx] = down;
            }
        }
        for (idx, key) in [
            (0usize, KeyCode::Digit1),
            (1, KeyCode::Digit2),
            (2, KeyCode::Digit3),
            (3, KeyCode::Digit4),
            (4, KeyCode::Digit5),
            (5, KeyCode::Digit6),
        ] {
            let down = input.down(key);
            if down && !prev.item[idx] {
                out.push(Cmd::UseItem { slot: idx as u8 });
            }
            prev.item[idx] = down;
        }
    }
    out
}

fn pick_enemy(
    camera: &ember_engine::Camera,
    aspect: f32,
    world: &World,
    ndc: [f32; 2],
) -> Option<u32> {
    let me = world.my_team();
    let mut best: Option<(u32, f32)> = None;
    for u in &world.units {
        if u.dead || u.t == me || u.k == 3 {
            continue; // holograms are not in the target list
        }
        let (sx, sy) = project(camera, aspect, u.x, 1.0, u.z);
        let rx = match u.k {
            0 => 0.055,
            1 | 2 => 0.030,
            4 | 5 | 6 | 7 => 0.075,
            _ => 0.03,
        };
        let d2 = ((sx - ndc[0]) * rx).powi(2) + ((sy - ndc[1]) / rx).powi(2);
        if d2 <= 1.0 && best.is_none_or(|(_, bd)| d2 < bd) {
            best = Some((u.id, d2));
        }
    }
    best.map(|(i, _)| i)
}

/// Last-frame held keys, for edge detection. `InputState` is a held set;
/// league wants presses.
#[derive(Default)]
pub struct Prev {
    pub rmb: bool,
    pub rmb_was_left: bool,
    pub abil: [bool; 4],
    pub spell: [bool; 2],
    pub item: [bool; 6],
}

/// The practice match. One human (slot 0), the rest bots, a duel by
/// default and a squad when the page asks for `mode: 3`.
pub struct LocalGame {
    m: Match,
    acc: f32,
    pub world: World,
    prev: Prev,
    human: u8,
}

impl LocalGame {
    /// A fresh local lobby: mode is the team size; the human is slot 0 and
    /// starts picking immediately (the page shows the draft screen).
    #[must_use]
    pub fn new(mode: u8, human_handle: &str, seed: u64) -> Self {
        let mut m = Match::new(mode, seed);
        m.join(human_handle);
        let mut world = World::new(mode);
        world.my_slot = 0;
        world.roster = m.roster.clone();
        world.phase = Phase::Select;
        world.left = league_core::data::SELECT_SECS;
        world.connected = true;
        Self {
            m,
            acc: 0.0,
            world,
            prev: Prev::default(),
            human: 0,
        }
    }

    /// The page's draft/start/buy commands, applied to the local sim.
    fn drain_ui(&mut self) {
        for json in uiq::drain() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
                continue;
            };
            if let Some(p) = v.get("pick") {
                let champ = p.get("champ").and_then(serde_json::Value::as_u64).unwrap_or(0) as u8;
                let d = p.get("d").and_then(serde_json::Value::as_u64).unwrap_or(0) as u8;
                let f = p.get("f").and_then(serde_json::Value::as_u64).unwrap_or(1) as u8;
                let runes: [u8; 3] = match p.get("runes").and_then(serde_json::Value::as_array) {
                    Some(a) if a.len() == 3 => std::array::from_fn(|i| a[i].as_u64().unwrap_or(0) as u8),
                    _ => [u8::MAX; 3],
                };
                self.m.set_pick(self.human, champ, d, f, runes);
                self.world.roster = self.m.roster.clone();
            }
            if v.get("start").and_then(serde_json::Value::as_bool) == Some(true) {
                self.m.start();
                self.world.phase = Phase::Live;
            }
            if let Some(b) = self.human_cmd(&v) {
                self.m.command(self.human, b);
            }
            if let Some(open) = v.get("shop").and_then(serde_json::Value::as_bool) {
                self.world.shop_open = open;
            }
        }
    }

    /// buy / use / rank, the commands that do not need a cursor.
    fn human_cmd(&self, v: &serde_json::Value) -> Option<Cmd> {
        if let Some(item) = v.get("buy").and_then(serde_json::Value::as_u64) {
            return Some(Cmd::Buy { item: item as u16 });
        }
        if let Some(s) = v.get("use").and_then(serde_json::Value::as_u64) {
            return Some(Cmd::UseItem { slot: s as u8 });
        }
        if let Some(s) = v.get("rank").and_then(serde_json::Value::as_u64) {
            return Some(Cmd::Rank { slot: s as u8 });
        }
        None
    }

    fn step_once(&mut self) {
        if self.m.phase == Phase::Live {
            let mut acts: Vec<(u8, Cmd)> = Vec::new();
            for slot in 0..2 * self.m.team_size {
                if slot == self.human {
                    continue;
                }
                acts.extend(ai::think(&self.m, slot).map(|c| (slot, c)));
            }
            for (slot, c) in acts {
                self.m.command(slot, c);
            }
        }
        let before = self.m.phase;
        self.m.step();
        if self.m.phase == Phase::Over && self.m.left <= 0.0 {
            // the practice loop: straight back to the draft
            let mode = self.m.team_size;
            let handle = self.m.roster[usize::from(self.human)].handle.clone();
            self.m = Match::new(mode, self.m.seed.wrapping_add(1));
            self.m.join(&handle);
        }
        if before != self.m.phase {
            self.world.left = self.m.left;
        }
    }

    fn rebuild_world(&mut self) {
        let S2C::State {
            tick,
            secs,
            units,
            champs,
            buffs,
            kills,
            boon,
            boon_left,
            court_respawn,
            fx,
            log,
        } = self.m.snapshot()
        else {
            return;
        };
        self.world.tick = tick;
        self.world.secs = secs;
        self.world.set_units(&units);
        self.world.champs = champs;
        self.world.buffs = buffs;
        self.world.kills = kills;
        self.world.boon = boon;
        self.world.boon_left = boon_left;
        self.world.court_respawn = court_respawn;
        self.world.phase = self.m.phase;
        self.world.left = self.m.left;
        self.world.winner = self.m.winner;
        self.world.zones = self
            .m
            .zones
            .iter()
            .map(|z| {
                let k = match z.zk {
                    league_core::sim::ZoneKind::Tornado => 0,
                    league_core::sim::ZoneKind::Trap => 1,
                    league_core::sim::ZoneKind::Stasis => 2,
                    league_core::sim::ZoneKind::Shroud => 3,
                };
                (k, z.x, z.z, z.r, (self.m.tick as f32 * 0.35) % std::f32::consts::TAU)
            })
            .collect();
        for f in fx {
            self.world.push_fx(FxLite {
                k: f.k,
                x: f.x,
                z: f.z,
                x2: f.x2,
                z2: f.z2,
                v: f.v,
                life: 0.0,
                left: 0.0,
            });
        }
        for ev in log {
            if let Some(text) = feed_line(&self.world, &ev) {
                if self.world.feed.len() > 8 {
                    self.world.feed.remove(0);
                }
                self.world.feed.push(crate::world::FeedLine { text, left: 7.0 });
            }
        }
    }
}

impl EmberGame for LocalGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let dt = dt.clamp(0.0, 0.1);
        self.drain_ui();
        let my_alive = self
            .m
            .champ_by_slot(self.human)
            .is_some_and(|i| !self.m.units[i].dead);
        for cmd in read_input(input, &mut self.prev, &self.world, input.aspect(), my_alive) {
            self.m.command(self.human, cmd);
        }
        // rebuild the view from the sim before stepping, so the click you
        // just made is judged on the frame you saw (one frame of lag,
        // which is what an authoritative server gives anyway)
        self.rebuild_world();
        self.acc += dt;
        let mut steps = 0;
        while self.acc >= league_core::DT && steps < 8 {
            self.step_once();
            self.acc -= league_core::DT;
            steps += 1;
        }
        self.world.tick_clocks(dt);
        self.world.follow(dt);
        crate::hud::set(&self.world.state_json());
        scene::scene(
            &self.world.units,
            &self.world.zones,
            &self.world.fx,
            self.world.secs,
            camera_for(self.world.cam),
        )
    }
}
