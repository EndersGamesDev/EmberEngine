//! Online play: lobby state, client-side prediction, and reconciliation.
//!
//! **Why predict at all.** CLAUDE.md's rule is that the arena's bullets are
//! stepped server-side only, which is what makes `f32` transcendentals safe
//! there. Cars call `sin`/`cos` every tick, so a predicted car and the
//! server's can drift apart in the last bits. That is survivable *here* and
//! not there, and the difference is what the number decides: a bullet's
//! trajectory decides a kill and must be arbitrated once, whereas a car's
//! position is corrected against the server thirty times a second and never
//! decides anything on its own. Without prediction, steering would lag by a
//! full round trip, which at any real latency is unplayable.
//!
//! **How.** Every input is stamped with a sequence number and kept. A state
//! broadcast carries the last sequence the server consumed; on arrival the
//! local car is snapped to the authoritative state and every input after that
//! sequence is re-applied. The player sees their own steering immediately and
//! still ends up where the server says.
//!
//! Remote cars are dead-reckoned from their last snapshot rather than
//! predicted: their inputs are unknown, and integrating the velocity the
//! server sent is both cheap and right for the 33 ms between broadcasts.

use std::collections::VecDeque;

use ember_engine::glam::Vec2;
use fire_core::car::{Car, CarInput, DT, OFFROAD_FACTOR};
use fire_core::castle;
use fire_core::proto::{CarState, LobbyInfo, Phase, PlayerMeta, C2S, S2C};
use fire_core::sim::{Race, RaceState};

/// Inputs kept for replay. At 60 Hz this is two seconds — far more round trip
/// than any playable connection, and bounded so a silent server cannot make
/// the client grow without limit.
const MAX_HISTORY: usize = 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Connected, listing lobbies, not yet in one.
    Browsing,
    /// In a lobby; the race may or may not have started.
    InLobby,
}

pub struct Online {
    /// Local mirror of the race, used for rendering. Authoritative only for
    /// the local car between snapshots.
    pub race: Race,
    pub screen: Screen,
    pub my_slot: Option<u8>,
    pub phase: Phase,
    pub countdown: f32,
    pub lobby_name: Option<String>,
    pub roster: Vec<PlayerMeta>,
    pub lobbies: Vec<LobbyInfo>,
    /// Last thing the server refused, for the page to show.
    pub notice: Option<String>,
    pub results: Option<Vec<u8>>,
    /// Set by `Welcome`. `Hello` must be the first message on a connection and
    /// `Welcome` is its acknowledgement, so anything version-gated —
    /// create and join both are — has to wait for it. Firing them off straight
    /// after `Hello` is speculative: if the server has not applied the `Hello`
    /// yet, the connection's protocol is still 0 and the join is refused with
    /// "this build speaks fire protocol v0".
    pub welcomed: bool,
    seq: u32,
    history: VecDeque<(u32, CarInput)>,
    /// Slots the server has actually told us about. A car we have never had a
    /// snapshot for must not be dead-reckoned from its grid pose.
    known: [bool; 8],
}

impl Default for Online {
    fn default() -> Self {
        Self::new()
    }
}

impl Online {
    pub fn new() -> Self {
        Self {
            race: Race::new(castle::track(), fire_core::proto::MAX_PLAYERS as usize, 3),
            screen: Screen::Browsing,
            my_slot: None,
            phase: Phase::Waiting,
            countdown: 0.0,
            lobby_name: None,
            roster: Vec::new(),
            lobbies: Vec::new(),
            notice: None,
            results: None,
            welcomed: false,
            seq: 0,
            history: VecDeque::new(),
            known: [false; 8],
        }
    }

    pub fn my_car(&self) -> Option<&Car> {
        let s = self.my_slot? as usize;
        self.race.racers.get(s).map(|r| &r.car)
    }

    /// Stamp an input, remember it for replay, and hand back the message to
    /// send. The caller owns the socket.
    pub fn make_input(&mut self, input: CarInput) -> C2S {
        self.seq = self.seq.wrapping_add(1);
        self.history.push_back((self.seq, input));
        while self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        C2S::Input {
            seq: self.seq,
            throttle: input.throttle,
            steer: input.steer,
            handbrake: input.handbrake,
            boost: input.boost,
        }
    }

    fn grip_at(&self, p: Vec2) -> f32 {
        if self.race.track.off_track(p) {
            OFFROAD_FACTOR
        } else {
            1.0
        }
    }

    /// Advance the local view by exactly one simulation tick: predict the
    /// local car forward with the player's current input, and dead-reckon
    /// everyone else.
    ///
    /// One tick, not one frame. Reconciliation replays history at `DT`, and
    /// the server steps at `DT`; predicting forward at the render rate would
    /// mean the forward and replay paths never agree and the car would creep
    /// away from the server between snapshots. The caller drives this from a
    /// `FixedStep`.
    pub fn predict_tick(&mut self, input: CarInput) {
        let dt = DT;
        if self.phase != Phase::Racing {
            return;
        }
        for i in 0..self.race.racers.len() {
            let is_me = self.my_slot == Some(i as u8);
            if is_me {
                let grip = self.grip_at(self.race.racers[i].car.pos);
                self.race.racers[i].car.step(&input, grip, dt);
            } else if self.known[i] {
                // Straight-line extrapolation. Guessing at a remote driver's
                // steering would look worse than a slightly stale heading.
                let v = self.race.racers[i].car.vel;
                self.race.racers[i].car.pos += v * dt;
            }
        }
    }

    /// Apply one server message.
    pub fn apply(&mut self, msg: S2C) {
        match msg {
            S2C::Welcome { .. } => self.welcomed = true,
            S2C::Rejected { reason } => self.notice = Some(reason),
            S2C::Lobbies { lobbies } => self.lobbies = lobbies,

            S2C::Joined { lobby, slot, laps, roster, .. } => {
                self.race = Race::new(castle::track(), fire_core::proto::MAX_PLAYERS as usize, laps);
                self.screen = Screen::InLobby;
                self.lobby_name = Some(lobby);
                self.my_slot = Some(slot);
                self.roster = roster;
                self.results = None;
                self.notice = None;
                self.history.clear();
                self.known = [false; 8];
            }

            S2C::PlayerJoined { meta } => {
                self.roster.retain(|p| p.id != meta.id);
                self.roster.push(meta);
            }
            S2C::PlayerLeft { id } => self.roster.retain(|p| p.id != id),

            S2C::Phase { phase, countdown } => {
                self.phase = phase;
                self.countdown = countdown;
                self.race.state = match phase {
                    Phase::Waiting => RaceState::Waiting,
                    Phase::Countdown => RaceState::Countdown,
                    Phase::Racing => RaceState::Racing,
                    Phase::Finished => RaceState::Finished,
                };
                if phase == Phase::Waiting {
                    self.results = None;
                    self.history.clear();
                }
            }

            S2C::Results { order } => self.results = Some(order),
            S2C::Pong { .. } => {}

            S2C::State { cars, .. } => self.apply_state(&cars),
        }
    }

    fn apply_state(&mut self, cars: &[CarState]) {
        for c in cars {
            let i = c.id as usize;
            if i >= self.race.racers.len() {
                continue;
            }
            if i < self.known.len() {
                self.known[i] = true;
            }

            let r = &mut self.race.racers[i];
            r.car.pos = Vec2::new(c.x, c.z);
            r.car.vel = Vec2::new(c.vx, c.vz);
            r.car.yaw = c.yaw;
            r.car.boost_charges = c.boost;
            r.car.boost_left = if c.boosting { r.car.boost_left.max(DT) } else { 0.0 };
            r.car.drift = c.drift;
            r.lap.lap = c.lap;
            r.lap.progress = c.progress;

            // Reconcile the local car: drop everything the server has already
            // consumed, then re-apply the rest on top of its authoritative
            // state. Without this the car snaps back a round trip's worth of
            // steering every time a packet lands.
            if self.my_slot == Some(c.id) {
                while self.history.front().is_some_and(|(s, _)| *s <= c.ack) {
                    self.history.pop_front();
                }
                let replay: Vec<CarInput> = self.history.iter().map(|(_, i)| *i).collect();
                for input in replay {
                    let grip = self.grip_at(self.race.racers[i].car.pos);
                    self.race.racers[i].car.step(&input, grip, DT);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fire_core::proto::MAX_PLAYERS;

    fn state_for(slot: u8, x: f32, z: f32, ack: u32) -> S2C {
        S2C::State {
            tick: 100,
            cars: vec![CarState {
                id: slot, x, z, yaw: 0.0, vx: 0.0, vz: 0.0,
                lap: 0, progress: 0.0, boost: 3, boosting: false, drift: 0.0, ack,
            }],
        }
    }

    fn joined(slot: u8) -> S2C {
        S2C::Joined {
            lobby: "t".into(),
            id: slot,
            slot,
            laps: 3,
            roster: vec![PlayerMeta { id: slot, handle: "me".into(), slot }],
        }
    }

    #[test]
    fn joining_sets_up_the_local_race() {
        let mut o = Online::new();
        o.apply(joined(2));
        assert_eq!(o.screen, Screen::InLobby);
        assert_eq!(o.my_slot, Some(2));
        assert_eq!(o.race.racers.len(), MAX_PLAYERS as usize);
        assert_eq!(o.race.laps_to_win, 3);
    }

    #[test]
    fn a_snapshot_moves_the_car_where_the_server_says() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(state_for(0, 12.0, -34.0, 0));
        let c = o.my_car().unwrap();
        assert_eq!(c.pos, Vec2::new(12.0, -34.0));
    }

    /// The heart of it: inputs the server has not yet seen must survive a
    /// snapshot. Without replay the car rubber-bands backwards on every
    /// packet and the controls feel like treacle.
    #[test]
    fn unacked_inputs_are_replayed_on_top_of_the_server_state() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::Phase { phase: Phase::Racing, countdown: 0.0 });

        let throttle = CarInput { throttle: 1.0, steer: 0.0, handbrake: false, boost: false };
        // Five inputs sent, none acknowledged yet.
        for _ in 0..5 {
            o.make_input(throttle);
        }
        // Server reports the car at the origin, having consumed nothing.
        o.apply(state_for(0, 0.0, 0.0, 0));
        let replayed = o.my_car().unwrap().pos;
        assert!(
            replayed.length() > 0.0,
            "five unacked throttle inputs left the car exactly at the server position"
        );

        // Now the server acknowledges all five: there is nothing left to
        // replay, so the car should sit exactly where it was told.
        o.apply(state_for(0, 0.0, 0.0, 5));
        assert_eq!(
            o.my_car().unwrap().pos,
            Vec2::ZERO,
            "history was not cleared by the ack"
        );
    }

    #[test]
    fn acknowledged_inputs_are_dropped_from_the_history() {
        let mut o = Online::new();
        o.apply(joined(0));
        for _ in 0..10 {
            o.make_input(CarInput::default());
        }
        assert_eq!(o.history.len(), 10);
        o.apply(state_for(0, 0.0, 0.0, 7));
        assert_eq!(o.history.len(), 3, "acked inputs were kept and will be replayed forever");
    }

    /// A server that stops acknowledging must not make the client grow
    /// without bound.
    #[test]
    fn the_replay_history_is_bounded() {
        let mut o = Online::new();
        o.apply(joined(0));
        for _ in 0..10_000 {
            o.make_input(CarInput::default());
        }
        assert!(o.history.len() <= MAX_HISTORY, "history grew to {}", o.history.len());
    }

    /// Remote cars are dead-reckoned, but only once the server has actually
    /// described them — otherwise the whole grid slides off from its start
    /// pose before the first snapshot arrives.
    #[test]
    fn unknown_remote_cars_are_not_dead_reckoned() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::Phase { phase: Phase::Racing, countdown: 0.0 });
        let before: Vec<Vec2> = o.race.racers.iter().map(|r| r.car.pos).collect();
        o.predict_tick(CarInput::default());
        for i in 1..o.race.racers.len() {
            assert_eq!(o.race.racers[i].car.pos, before[i], "car {i} moved before we heard about it");
        }
    }

    #[test]
    fn known_remote_cars_coast_between_snapshots() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::Phase { phase: Phase::Racing, countdown: 0.0 });
        o.apply(S2C::State {
            tick: 1,
            cars: vec![CarState {
                id: 1, x: 0.0, z: 0.0, yaw: 0.0, vx: 10.0, vz: 0.0,
                lap: 0, progress: 0.0, boost: 3, boosting: false, drift: 0.0, ack: 0,
            }],
        });
        o.predict_tick(CarInput::default());
        let p = o.race.racers[1].car.pos;
        assert!((p.x - 10.0 * DT).abs() < 1e-4, "remote car did not coast: {p}");
    }

    #[test]
    fn nothing_moves_outside_the_racing_phase() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::Phase { phase: Phase::Countdown, countdown: 3.0 });
        let before = o.my_car().unwrap().pos;
        for _ in 0..120 {
            o.predict_tick(CarInput { throttle: 1.0, steer: 0.0, handbrake: false, boost: true });
        }
        assert_eq!(o.my_car().unwrap().pos, before, "a car jumped the start");
    }

    #[test]
    fn a_rejection_surfaces_as_a_notice() {
        let mut o = Online::new();
        o.apply(S2C::Rejected { reason: "wrong password".into() });
        assert_eq!(o.notice.as_deref(), Some("wrong password"));
        // Joining clears it.
        o.apply(joined(1));
        assert!(o.notice.is_none());
    }

    #[test]
    fn the_roster_tracks_arrivals_and_departures() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::PlayerJoined { meta: PlayerMeta { id: 3, handle: "b".into(), slot: 3 } });
        assert_eq!(o.roster.len(), 2);
        // A duplicate announcement must not double-list anyone.
        o.apply(S2C::PlayerJoined { meta: PlayerMeta { id: 3, handle: "b".into(), slot: 3 } });
        assert_eq!(o.roster.len(), 2);
        o.apply(S2C::PlayerLeft { id: 3 });
        assert_eq!(o.roster.len(), 1);
    }

    /// A malformed or hostile slot index must not panic the client.
    #[test]
    fn an_out_of_range_slot_is_ignored() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(state_for(200, 1.0, 1.0, 0));
        assert_eq!(o.my_car().unwrap().pos, o.race.racers[0].car.pos);
    }
}
