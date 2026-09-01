//! The online mode's `EmberGame`: pump the socket, predict, render.
//!
//! Lobby *browsing* is not here. The page opens its own short-lived socket to
//! list lobbies — exactly as the arena page does — and then calls
//! `start_online` with the one the player picked. That keeps the engine loop
//! out of the menu business and means the browser works from a frozen page
//! even when this build's protocol has moved on.

use ember_engine::{EmberGame, Frame, InputState};
use fire_core::car::CarInput;
use fire_core::proto::{C2S, Phase};
use fire_core::sim::FixedStep;

use crate::game::{self, Chase, Hud, Meshes};
use crate::net::{Inbox, Net, Status};
use crate::online::Online;

/// What the page hands to `start_online`.
#[derive(Debug, Clone)]
pub struct Config {
    pub ws: String,
    pub handle: String,
    pub lobby: String,
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
                "driver".into()
            } else {
                handle.to_string()
            },
            lobby: if lobby.is_empty() {
                "castle".into()
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

pub struct OnlineGame {
    net: Net,
    inbox: Inbox,
    state: Online,
    ids: Meshes,
    chase: Chase,
    boost_was_down: bool,
    /// Auto-ready remains a Fire game action after shared lobby admission.
    readied: bool,
    connection_notice: bool,
    clock: FixedStep,
}

impl OnlineGame {
    /// Connect the online game to its configured server.
    ///
    /// # Errors
    ///
    /// Returns an error if the networking backend cannot start the connection.
    pub fn connect(cfg: &Config, ids: Meshes) -> Result<Self, String> {
        let net = Net::connect_session(
            &cfg.ws,
            &cfg.handle,
            &cfg.lobby,
            cfg.password.clone(),
            cfg.create,
        )?;
        let state = Online::new();
        let chase = Chase::new(&state.race.racers[0].car);
        Ok(Self {
            net,
            inbox: Inbox::default(),
            state,
            ids,
            chase,
            boost_was_down: false,
            readied: false,
            connection_notice: false,
            clock: FixedStep::default(),
        })
    }

    fn publish_hud(&self) {
        let me = usize::from(self.state.my_slot.unwrap_or(0));
        let racer = self.state.race.racers.get(me);
        let place = self
            .state
            .results
            .as_ref()
            .and_then(|o| o.iter().position(|&i| usize::from(i) == me))
            .map_or_else(
                || {
                    self.state
                        .race
                        .standings()
                        .iter()
                        .position(|&i| i == me)
                        .unwrap_or(0)
                        + 1
                },
                |position| position + 1,
            );
        game::set_hud(Hud {
            speed_kmh: racer.map_or(0.0, |r| r.car.speed() * 3.6),
            lap: racer.map_or(0, |r| r.lap.lap.min(self.state.race.laps_to_win)),
            laps_total: self.state.race.laps_to_win,
            place,
            // Cars on the grid, not humans in the roster. The server fills
            // every unclaimed slot with an AI, so a lobby of one still races
            // a field of eight — and "P8 of 1" is not a position.
            racers: self.state.race.racers.len(),
            boost_charges: racer.map_or(0, |r| r.car.boost_charges),
            boosting: racer.is_some_and(|r| r.car.boosting()),
            drifting: racer.is_some_and(|r| r.car.drift > 0.25),
            countdown: self.state.countdown,
            finished: self.state.results.is_some(),
        });
    }
}

impl EmberGame for OnlineGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        let dt = dt.clamp(0.0, 0.05);

        self.inbox.pump(&mut self.net);
        while let Some(m) = self.inbox.pop() {
            self.state.apply(m);
        }

        if !self.connection_notice && let Status::Closed(reason) = self.net.status() {
            self.connection_notice = true;
            self.state.notice = Some(format!("connection lost: {reason}"));
        }

        if !self.readied && self.state.my_slot.is_some() {
            self.readied = true;
            self.net.send(&C2S::Ready { ready: true });
        }

        let mine = game::read_input(input, &mut self.boost_was_down);
        let ticks = self.clock.ticks(dt);
        if self.state.my_slot.is_some() && self.state.phase == Phase::Racing {
            // One input message per simulated tick, so the sequence numbers
            // the server acks line up one-for-one with the history the client
            // replays. Sending once per frame while predicting several ticks
            // would leave the replay with inputs the server never saw.
            for _ in 0..ticks {
                let msg = self.state.make_input(mine);
                self.net.send(&msg);
                self.state.predict_tick(mine);
            }
        } else {
            // Still keep the connection alive while waiting on the grid.
            let idle = CarInput::default();
            let msg = self.state.make_input(idle);
            self.net.send(&msg);
        }

        let me = usize::from(self.state.my_slot.unwrap_or(0));
        let camera = self.chase.update(&self.state.race.racers[me].car, dt);
        self.publish_hud();
        game::scene(&self.state.race, &self.ids, me, camera)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_the_pages_json() {
        let c = Config::from_json(
            r#"{"ws":"ws://x:1","handle":"ender","lobby":"castle","password":"p","create":true}"#,
        )
        .unwrap();
        assert_eq!(c.ws, "ws://x:1");
        assert_eq!(c.handle, "ender");
        assert_eq!(c.lobby, "castle");
        assert_eq!(c.password.as_deref(), Some("p"));
        assert!(c.create);
    }

    #[test]
    fn config_fills_in_sensible_defaults() {
        let c = Config::from_json(r#"{"ws":"ws://x:1"}"#).unwrap();
        assert_eq!(c.handle, "driver");
        assert_eq!(c.lobby, "castle");
        assert!(
            c.password.is_none(),
            "an absent password must not become Some(\"\")"
        );
        assert!(!c.create);
    }

    /// An empty password string from a form field is "no password", not a
    /// password that happens to be empty — otherwise every join to an open
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
