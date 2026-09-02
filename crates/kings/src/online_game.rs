//! The online mode's `EmberGame`: pump the socket, keep the lobby state,
//! drain the page's commands through the selection machine, render.
//!
//! Lobby browsing is not here. The page opens its own short-lived socket to
//! list lobbies, as the arena and fire pages do, and then calls
//! `start_online` with the one the player picked.

use ember_engine::{EmberGame, Frame, InputState};
use kings_core::board::Tile;
use kings_core::proto::C2S;

use crate::game::{
    self, Cursor, CursorCmd, CursorKeys, HudState, Meshes, Mode, SeatCamera, UiCmd, View,
    fill_board, set_hud,
};
use crate::net::{Inbox, Net, Status};
use crate::online::Online;
use crate::ui::{Ui, UiOut};

/// What the page hands to `start_online`.
#[derive(Debug, Clone)]
pub struct Config {
    /// The server's WebSocket URL.
    pub ws: String,
    /// Display name.
    pub handle: String,
    /// Lobby name.
    pub lobby: String,
    /// Lobby password, if any.
    pub password: Option<String>,
    /// Create the lobby rather than join an existing one.
    pub create: bool,
}

impl Config {
    /// Parsed by hand rather than with serde: the shape is three strings and
    /// two flags, and this keeps the page's JSON contract visible in one
    /// place instead of spread across derive attributes.
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
        Ok(Self {
            ws,
            handle: if handle.is_empty() {
                "player".into()
            } else {
                handle.to_string()
            },
            lobby: if lobby.is_empty() {
                "court".into()
            } else {
                lobby.to_string()
            },
            password,
            create: v
                .get("create")
                .is_some_and(|value| value.as_bool() == Some(true)),
        })
    }
}

/// The online game.
///
/// Hello and the keepalive are not here: the socket sends both on its own
/// clock (`crate::net`), so a web tab that gets no animation frames keeps
/// its seat. This loop only reads what arrived and sends what the player
/// did.
pub struct OnlineGame {
    net: Net,
    inbox: Inbox,
    online: Online,
    ui: Ui,
    cursor: Cursor,
    cam: SeatCamera,
    meshes: Meshes,
    cfg: Config,
    /// One-shot milestone: the create/join has been sent.
    entered: bool,
    /// Why the socket closed, once it has.
    lost: Option<String>,
}

impl OnlineGame {
    /// Connect the online game to its configured server. The socket greets
    /// the server with the configured handle as soon as it opens.
    ///
    /// # Errors
    ///
    /// Returns an error if the networking backend cannot start the connection.
    pub fn connect(cfg: Config, meshes: Meshes) -> Result<Self, String> {
        let net = Net::connect(&cfg.ws, &cfg.handle)?;
        Ok(Self {
            net,
            inbox: Inbox::default(),
            online: Online::new(),
            ui: Ui::default(),
            cursor: Cursor::new(0),
            cam: SeatCamera::new(0),
            meshes,
            cfg,
            entered: false,
            lost: None,
        })
    }

    /// The frame the page lays the board out in: our seat, or seat 0 until
    /// the roster says.
    fn local_seat(&self) -> u8 {
        self.online.my_seat.unwrap_or(0)
    }

    fn send_ui(&mut self, out: UiOut) {
        let msg = match out {
            UiOut::Move { turn, from, to } => C2S::Move {
                turn,
                fx: from.x,
                fy: from.y,
                tx: to.x,
                ty: to.y,
            },
            UiOut::SetFormation(formation) => C2S::SetFormation { formation },
        };
        self.net.send(&msg);
        self.online.pending = true;
    }

    fn click(&mut self, tile: Tile) {
        let out = {
            // No seat yet (the roster has not arrived) means no click: the
            // machine's `None` is the hotseat convention, acting for the
            // seat to move, which is somebody else's piece here.
            let (Some(state), Some(me)) = (self.online.state.as_ref(), self.online.my_seat) else {
                return;
            };
            self.ui.click(state, Some(me), self.online.phase, tile)
        };
        if let Some(out) = out {
            self.send_ui(out);
        }
    }

    fn view(&self) -> View {
        View {
            sel: self.ui.selected(),
            targets: self.ui.targets.clone(),
            cursor: Some(self.cursor.tile),
        }
    }

    fn publish(&self, connected: bool) {
        let o = &self.online;
        let mut hud = HudState::new(Mode::Online);
        hud.connected = connected;
        hud.screen = o.screen;
        hud.phase = o.phase;
        hud.winner = o.winner;
        hud.end = o.end;
        hud.me = o.my_seat;
        hud.creator = o.creator.unwrap_or(0);
        hud.is_creator = o.is_creator;
        hud.can_start = o.can_start;
        hud.my_turn = o.my_turn();
        hud.left_ms = o.left_ms;
        hud.roster.clone_from(&o.roster);
        hud.pending = o.pending;
        hud.notice = self.lost.clone().or_else(|| o.notice.clone());
        if let (Some(board), Some(state)) = (&o.board, &o.state) {
            fill_board(&mut hud, board, state, &self.view());
        }
        set_hud(hud);
    }
}

impl EmberGame for OnlineGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let dt = dt.clamp(0.0, 0.1);

        let status = self.net.status();
        if let Status::Closed(why) = &status
            && self.lost.is_none()
        {
            tracing::warn!("kings: connection lost: {why}");
            self.lost = Some(format!("connection lost: {why}"));
        }

        self.inbox.pump(&mut self.net);
        while let Some(m) = self.inbox.pop() {
            if matches!(
                m,
                kings_core::proto::S2C::State { .. } | kings_core::proto::S2C::Rejected { .. }
            ) {
                self.ui.settle();
            }
            self.online.apply(m);
        }

        // Wait for Welcome before create/join. Both are version-gated, and
        // the gate reads a protocol number that the socket's Hello sets.
        if !self.entered && self.online.welcomed {
            self.entered = true;
            let msg = if self.cfg.create {
                C2S::CreateLobby {
                    name: self.cfg.lobby.clone(),
                    password: self.cfg.password.clone(),
                }
            } else {
                C2S::JoinLobby {
                    name: self.cfg.lobby.clone(),
                    password: self.cfg.password.clone(),
                }
            };
            self.net.send(&msg);
        }

        self.online.tick(dt);

        let seat = self.local_seat();
        for cmd in game::drain_cmds() {
            match cmd {
                UiCmd::Click(x, y) => {
                    if let Some(t) = Tile::new(x, y) {
                        self.click(t);
                    }
                }
                UiCmd::Start => self.net.send(&C2S::Start),
                UiCmd::Clear => self.ui.clear(),
            }
        }
        match self.cursor.step(CursorKeys::read(input), seat) {
            Some(CursorCmd::Click(t)) => self.click(t),
            Some(CursorCmd::Clear) => self.ui.clear(),
            None => {}
        }

        self.cam.retarget(seat);
        let camera = self.cam.tick(dt);
        self.publish(status == Status::Open);
        let empty = game::empty_state();
        let state = self.online.state.as_ref().unwrap_or(&empty);
        game::scene(state, &self.meshes, &self.view(), camera)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_the_pages_json() {
        let c = Config::from_json(
            r#"{"ws":"ws://x:1","handle":"ender","lobby":"court","password":"p","create":true}"#,
        )
        .unwrap();
        assert_eq!(c.ws, "ws://x:1");
        assert_eq!(c.handle, "ender");
        assert_eq!(c.lobby, "court");
        assert_eq!(c.password.as_deref(), Some("p"));
        assert!(c.create);
    }

    #[test]
    fn config_fills_in_sensible_defaults() {
        let c = Config::from_json(r#"{"ws":"ws://x:1"}"#).unwrap();
        assert_eq!(c.handle, "player");
        assert_eq!(c.lobby, "court");
        assert!(
            c.password.is_none(),
            "an absent password must not become Some(\"\")"
        );
        assert!(!c.create);
    }

    /// An empty password string from a form field is "no password", not a
    /// password that happens to be empty; otherwise every join to an open
    /// lobby carries a credential the server then compares against None.
    #[test]
    fn an_empty_password_field_means_no_password() {
        let c = Config::from_json(r#"{"ws":"ws://x:1","password":""}"#).unwrap();
        assert!(c.password.is_none());
    }

    #[test]
    fn a_config_without_a_url_is_refused() {
        assert!(Config::from_json(r#"{"handle":"x"}"#).is_err());
        assert!(Config::from_json("not json").is_err());
    }
}
