//! The online game: pump the socket, keep the last snapshot as the world,
//! send the player's commands, render. Lobby browsing is not here — the
//! page opens its own short-lived socket to list lobbies, then calls
//! `start_online` with the one the player picked.

use ember_engine::{EmberGame, Frame, InputState};
use league_core::proto::{C2S, Cmd, Phase, S2C};

use crate::game::{read_input, uiq, Prev};
use crate::net::{Inbox, Net, Status};
use crate::world::{feed_line, FxLite, World};

/// What the page hands to `start_online`.
#[derive(Debug, Clone)]
pub struct Config {
    pub ws: String,
    pub handle: String,
    pub lobby: String,
    pub password: Option<String>,
    pub create: bool,
    pub mode: u8,
}

impl Config {
    /// Parsed by hand: the shape is strings and flags, and this keeps the
    /// page's JSON contract visible in one place.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not JSON or has no WebSocket URL.
    pub fn from_json(s: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let get = |key: &str| v.get(key).and_then(serde_json::Value::as_str).unwrap_or("");
        let ws = get("ws").to_string();
        if ws.is_empty() {
            return Err("config has no ws url".into());
        }
        let password = v
            .get("password")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.is_empty());
        let handle = get("handle");
        let lobby = get("lobby");
        let mode = v.get("mode").and_then(serde_json::Value::as_u64).unwrap_or(3);
        Ok(Self {
            ws,
            handle: if handle.is_empty() {
                "player".into()
            } else {
                handle.to_string()
            },
            lobby: if lobby.is_empty() {
                "lane".into()
            } else {
                lobby.to_string()
            },
            password,
            create: v
                .get("create")
                .is_some_and(|value| value.as_bool() == Some(true)),
            mode: u8::try_from(mode).unwrap_or(3).min(3),
        })
    }
}

pub struct OnlineGame {
    net: Net,
    inbox: Inbox,
    cfg: Config,
    pub world: World,
    prev: Prev,
    entered: bool,
    welcomed: bool,
    lost: Option<String>,
}

impl OnlineGame {
    /// Connect the online game to its configured server.
    ///
    /// # Errors
    ///
    /// Returns an error if the networking backend cannot start the connection.
    pub fn connect(cfg: Config) -> Result<Self, String> {
        let net = Net::connect(&cfg.ws, &cfg.handle)?;
        let mut world = World::new(cfg.mode);
        world.phase = Phase::Select;
        Ok(Self {
            net,
            inbox: Inbox::default(),
            cfg,
            world,
            prev: Prev::default(),
            entered: false,
            welcomed: false,
            lost: None,
        })
    }

    fn apply(&mut self, msg: S2C) {
        match msg {
            S2C::Welcome { .. } => self.welcomed = true,
            S2C::Rejected { reason } => self.world.notice = Some(reason),
            S2C::Joined {
                lobby,
                id,
                mode,
                roster,
            } => {
                self.world.my_slot = id;
                self.world.mode = mode;
                self.world.roster = roster;
                self.world.notice = None;
                self.world.cam = (if id < mode { -40.0 } else { 40.0 }, 0.0);
                tracing::info!(lobby, id, "joined lobby");
            }
            S2C::Roster { roster } => self.world.roster = roster,
            S2C::PlayerJoined { slot } => {
                if let Some(i) = self
                    .world
                    .roster
                    .iter()
                    .position(|r| r.slot == slot.slot)
                {
                    self.world.roster[i] = slot;
                }
            }
            S2C::PlayerLeft { slot } => {
                if let Some(i) = self.world.roster.iter().position(|r| r.slot == slot) {
                    self.world.roster[i].connected = false;
                    self.world.roster[i].bot = true;
                }
            }
            S2C::Phase { phase, left } => {
                self.world.phase = phase;
                self.world.left = left;
            }
            S2C::State {
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
            } => {
                self.world.tick = tick;
                self.world.secs = secs;
                self.world.set_units(&units);
                self.world.champs = champs;
                self.world.buffs = buffs;
                self.world.kills = kills;
                self.world.boon = boon;
                self.world.boon_left = boon_left;
                self.world.court_respawn = court_respawn;
                self.world.phase = Phase::Live;
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
            S2C::Result { winner, .. } => {
                self.world.winner = winner;
                self.world.phase = Phase::Over;
            }
            S2C::Lobbies { .. } | S2C::Pong { .. } => {}
        }
    }

    fn drain_ui(&mut self) {
        for json in uiq::drain() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
                continue;
            };
            if let Some(p) = v.get("pick") {
                let mut runes = [u8::MAX; 3];
                if let Some(a) = p.get("runes").and_then(serde_json::Value::as_array) {
                    for (i, r) in a.iter().take(3).enumerate() {
                        runes[i] = r.as_u64().unwrap_or(0) as u8;
                    }
                }
                let msg = C2S::Pick {
                    champ: p.get("champ").and_then(serde_json::Value::as_u64).unwrap_or(0) as u8,
                    d: p.get("d").and_then(serde_json::Value::as_u64).unwrap_or(0) as u8,
                    f: p.get("f").and_then(serde_json::Value::as_u64).unwrap_or(1) as u8,
                    runes,
                };
                self.net.send(&msg);
            }
            if v.get("start").and_then(serde_json::Value::as_bool) == Some(true) {
                self.net.send(&C2S::StartMatch);
            }
            if let Some(open) = v.get("shop").and_then(serde_json::Value::as_bool) {
                self.world.shop_open = open;
            }
            if let Some(item) = v.get("buy").and_then(serde_json::Value::as_u64) {
                self.send_cmd(Cmd::Buy { item: item as u16 });
            }
            if let Some(s) = v.get("use").and_then(serde_json::Value::as_u64) {
                self.send_cmd(Cmd::UseItem { slot: s as u8 });
            }
            if let Some(s) = v.get("rank").and_then(serde_json::Value::as_u64) {
                self.send_cmd(Cmd::Rank { slot: s as u8 });
            }
        }
    }

    fn send_cmd(&mut self, cmd: Cmd) {
        if self.world.phase == Phase::Live {
            self.net.send(&C2S::Cmd(cmd));
        }
    }
}

impl EmberGame for OnlineGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let dt = dt.clamp(0.0, 0.1);
        let status = self.net.status();
        if let Status::Closed(why) = &status
            && self.lost.is_none()
        {
            self.lost = Some(format!("connection lost: {why}"));
        }
        self.inbox.pump(&mut self.net);
        while let Some(m) = self.inbox.pop() {
            self.apply(m);
        }
        // Wait for Welcome before create/join: both are version-gated, and
        // the gate reads a protocol number that the socket's Hello sets.
        if !self.entered && self.welcomed {
            self.entered = true;
            let msg = if self.cfg.create {
                C2S::CreateLobby {
                    name: self.cfg.lobby.clone(),
                    password: self.cfg.password.clone(),
                    mode: self.cfg.mode,
                }
            } else {
                C2S::JoinLobby {
                    name: self.cfg.lobby.clone(),
                    password: self.cfg.password.clone(),
                }
            };
            self.net.send(&msg);
        }
        self.world.connected = status == Status::Open;
        self.world.notice = self.lost.clone().or_else(|| self.world.notice.take());

        self.drain_ui();
        if self.world.phase == Phase::Live {
            let my_alive = self
                .world
                .champs
                .iter()
                .any(|c| c.slot == self.world.my_slot && c.alive);
            for cmd in read_input(input, &mut self.prev, &self.world, input.aspect(), my_alive) {
                self.send_cmd(cmd);
            }
        }
        self.world.tick_clocks(dt);
        self.world.follow(dt);
        crate::hud::set(&self.world.state_json());

        let camera = crate::scene::camera_for(self.world.cam);
        crate::scene::scene(
            &self.world.units,
            &self.world.zones,
            &self.world.fx,
            self.world.secs,
            camera,
        )
    }
}
