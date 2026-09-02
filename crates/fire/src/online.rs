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

use std::time::Duration;

use ember_client_net::{
    AcknowledgementMode, CorrectionMode, PredictionHooks, Reconciler, RemoteEntityHooks,
    RemoteSnapshotBuffer, ReplayContext,
};
use ember_engine::glam::Vec2;
use fire_core::car::{Car, CarInput, DT, OFFROAD_FACTOR};
use fire_core::castle;
use fire_core::proto::{C2S, CarState, LobbyInfo, Phase, PlayerMeta, S2C};
use fire_core::sim::{Race, RaceState};

/// Inputs kept for replay. At 60 Hz this is two seconds — far more round trip
/// than any playable connection, and bounded so a silent server cannot make
/// the client grow without limit.
const MAX_HISTORY: usize = 120;

#[derive(Clone)]
struct FireAuthoritative {
    tick: u64,
    car: CarState,
}

const fn apply_car_state(car: &mut Car, state: &CarState) {
    car.pos = Vec2::new(state.x, state.z);
    car.vel = Vec2::new(state.vx, state.vz);
    car.yaw = state.yaw;
    car.boost_charges = state.boost;
    car.boost_left = if state.boosting {
        car.boost_left.max(DT)
    } else {
        0.0
    };
    car.drift = state.drift;
}

struct FirePredictionHooks<'a> {
    track: &'a fire_core::track::Track,
}

impl PredictionHooks for FirePredictionHooks<'_> {
    type Input = CarInput;
    type AuthoritativeState = FireAuthoritative;
    type PredictedState = Car;

    fn acknowledgement(&self, authoritative: &Self::AuthoritativeState) -> u32 {
        authoritative.car.ack
    }

    fn server_timestamp(&self, authoritative: &Self::AuthoritativeState) -> u64 {
        authoritative.tick
    }

    fn acknowledgement_mode(&self) -> AcknowledgementMode {
        AcknowledgementMode::Through
    }

    fn apply_authoritative(
        &self,
        predicted: &mut Self::PredictedState,
        authoritative: &Self::AuthoritativeState,
    ) {
        apply_car_state(predicted, &authoritative.car);
    }

    fn replay_one_slice(
        &self,
        predicted: &mut Self::PredictedState,
        input: &Self::Input,
        _context: ReplayContext,
        _authoritative: &Self::AuthoritativeState,
    ) {
        let grip = if self.track.off_track(predicted.pos) {
            OFFROAD_FACTOR
        } else {
            1.0
        };
        predicted.step(input, grip, DT);
    }

    fn snap_or_smooth(
        &self,
        _before: &Self::PredictedState,
        _after: &Self::PredictedState,
        _authoritative: &Self::AuthoritativeState,
    ) -> CorrectionMode {
        CorrectionMode::Snap
    }
}

struct FireRemoteHooks;

impl RemoteEntityHooks for FireRemoteHooks {
    type Snapshot = CarState;
    type RenderState = CarState;

    fn interpolate_remote(
        &self,
        from: &Self::Snapshot,
        to: &Self::Snapshot,
        numerator: u64,
        denominator: u64,
    ) -> Self::RenderState {
        let numerator = u16::try_from(numerator).unwrap_or(u16::MAX);
        let denominator = u16::try_from(denominator).unwrap_or(u16::MAX).max(1);
        let alpha = f32::from(numerator) / f32::from(denominator);
        let mut state = *to;
        state.x = from.x + (to.x - from.x) * alpha;
        state.z = from.z + (to.z - from.z) * alpha;
        state.yaw = from.yaw + (to.yaw - from.yaw) * alpha;
        state.vx = from.vx + (to.vx - from.vx) * alpha;
        state.vz = from.vz + (to.vz - from.vz) * alpha;
        state.drift = from.drift + (to.drift - from.drift) * alpha;
        state.progress = from.progress + (to.progress - from.progress) * alpha;
        state
    }

    fn dead_reckon_remote(&self, latest: &Self::Snapshot, elapsed: u64) -> Self::RenderState {
        let elapsed = u16::try_from(elapsed).unwrap_or(u16::MAX);
        let mut state = *latest;
        state.x += state.vx * DT * f32::from(elapsed);
        state.z += state.vz * DT * f32::from(elapsed);
        state
    }

    fn snap_or_smooth_remote(
        &self,
        _from: &Self::Snapshot,
        _to: &Self::Snapshot,
    ) -> CorrectionMode {
        CorrectionMode::Snap
    }
}

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
    reconciler: Reconciler<CarInput>,
    /// Per-slot authoritative samples; empty means the server has never
    /// described that car and its grid pose must not be extrapolated.
    remote_snapshots: [RemoteSnapshotBuffer<CarState>; 8],
    remote_time: [u64; 8],
}

impl Default for Online {
    fn default() -> Self {
        Self::new()
    }
}

impl Online {
    #[must_use]
    pub fn new() -> Self {
        Self {
            race: Race::new(
                castle::track(),
                usize::from(fire_core::proto::MAX_PLAYERS),
                3,
            ),
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
            reconciler: Reconciler::new(MAX_HISTORY),
            remote_snapshots: std::array::from_fn(|_| RemoteSnapshotBuffer::new(3)),
            remote_time: [0; 8],
        }
    }

    #[must_use]
    pub fn my_car(&self) -> Option<&Car> {
        let s = usize::from(self.my_slot?);
        self.race.racers.get(s).map(|r| &r.car)
    }

    /// Stamp an input, remember it for replay, and hand back the message to
    /// send. The caller owns the socket.
    pub fn make_input(&mut self, input: CarInput) -> C2S {
        let sequence = self.reconciler.record(input, Duration::ZERO);
        C2S::Input {
            seq: sequence,
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
            let is_me = self.my_slot.is_some_and(|slot| usize::from(slot) == i);
            if is_me {
                let grip = self.grip_at(self.race.racers[i].car.pos);
                self.race.racers[i].car.step(&input, grip, dt);
            } else if !self.remote_snapshots[i].is_empty() {
                // Straight-line extrapolation. Guessing at a remote driver's
                // steering would look worse than a slightly stale heading.
                self.remote_time[i] = self.remote_time[i].wrapping_add(1);
                if let Some(state) =
                    self.remote_snapshots[i].sample_at(&FireRemoteHooks, self.remote_time[i])
                {
                    apply_car_state(&mut self.race.racers[i].car, &state);
                }
            }
        }
    }

    /// Apply one server message.
    pub fn apply(&mut self, msg: S2C) {
        match msg {
            S2C::Welcome { .. } => self.welcomed = true,
            S2C::Rejected { reason } => self.notice = Some(reason),
            S2C::Lobbies { lobbies } => self.lobbies = lobbies,

            S2C::Joined {
                lobby,
                slot,
                laps,
                roster,
                ..
            } => {
                let race = Race::new(
                    castle::track(),
                    usize::from(fire_core::proto::MAX_PLAYERS),
                    laps,
                );
                if usize::from(slot) >= race.racers.len() {
                    self.notice = Some(format!("server assigned invalid player slot {slot}"));
                    return;
                }
                self.race = race;
                self.screen = Screen::InLobby;
                self.lobby_name = Some(lobby);
                self.my_slot = Some(slot);
                self.roster = roster;
                self.results = None;
                self.notice = None;
                self.reconciler.clear_history();
                for snapshots in &mut self.remote_snapshots {
                    snapshots.clear();
                }
                self.remote_time = [0; 8];
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
                    self.reconciler.clear_history();
                }
            }

            S2C::Results { order } => self.results = Some(order),
            S2C::Pong { .. } => {}

            S2C::State { tick, cars } => self.apply_state(tick, &cars),
        }
    }

    fn apply_state(&mut self, tick: u64, cars: &[CarState]) {
        for c in cars {
            let i = usize::from(c.id);
            if i >= self.race.racers.len() {
                continue;
            }
            self.race.racers[i].lap.lap = c.lap;
            self.race.racers[i].lap.progress = c.progress;

            // Reconcile the local car: drop everything the server has already
            // consumed, then re-apply the rest on top of its authoritative
            // state. Without this the car snaps back a round trip's worth of
            // steering every time a packet lands.
            if self.my_slot == Some(c.id) {
                let authoritative = FireAuthoritative { tick, car: *c };
                let hooks = FirePredictionHooks {
                    track: &self.race.track,
                };
                self.reconciler.reconcile(
                    &hooks,
                    &mut self.race.racers[i].car,
                    &authoritative,
                    Duration::ZERO,
                );
            } else if i < self.remote_snapshots.len() {
                self.remote_snapshots[i].push(tick, *c);
                self.remote_time[i] = tick;
                apply_car_state(&mut self.race.racers[i].car, c);
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
                id: slot,
                x,
                z,
                yaw: 0.0,
                vx: 0.0,
                vz: 0.0,
                lap: 0,
                progress: 0.0,
                boost: 3,
                boosting: false,
                drift: 0.0,
                ack,
            }],
        }
    }

    fn joined(slot: u8) -> S2C {
        S2C::Joined {
            lobby: "t".into(),
            id: slot,
            slot,
            laps: 3,
            roster: vec![PlayerMeta {
                id: slot,
                handle: "me".into(),
                slot,
            }],
        }
    }

    #[test]
    fn joining_sets_up_the_local_race() {
        let mut o = Online::new();
        o.apply(joined(2));
        assert_eq!(o.screen, Screen::InLobby);
        assert_eq!(o.my_slot, Some(2));
        assert_eq!(o.race.racers.len(), usize::from(MAX_PLAYERS));
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
        o.apply(S2C::Phase {
            phase: Phase::Racing,
            countdown: 0.0,
        });

        let throttle = CarInput {
            throttle: 1.0,
            steer: 0.0,
            handbrake: false,
            boost: false,
        };
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
        assert_eq!(o.reconciler.history_len(), 10);
        o.apply(state_for(0, 0.0, 0.0, 7));
        assert_eq!(
            o.reconciler.history_len(),
            3,
            "acked inputs were kept and will be replayed forever"
        );
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
        assert!(
            o.reconciler.history_len() <= MAX_HISTORY,
            "history grew to {}",
            o.reconciler.history_len()
        );
    }

    /// Remote cars are dead-reckoned, but only once the server has actually
    /// described them — otherwise the whole grid slides off from its start
    /// pose before the first snapshot arrives.
    #[test]
    fn unknown_remote_cars_are_not_dead_reckoned() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::Phase {
            phase: Phase::Racing,
            countdown: 0.0,
        });
        let before: Vec<Vec2> = o.race.racers.iter().map(|r| r.car.pos).collect();
        o.predict_tick(CarInput::default());
        for i in 1..o.race.racers.len() {
            assert_eq!(
                o.race.racers[i].car.pos, before[i],
                "car {i} moved before we heard about it"
            );
        }
    }

    #[test]
    fn known_remote_cars_coast_between_snapshots() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::Phase {
            phase: Phase::Racing,
            countdown: 0.0,
        });
        o.apply(S2C::State {
            tick: 1,
            cars: vec![CarState {
                id: 1,
                x: 0.0,
                z: 0.0,
                yaw: 0.0,
                vx: 10.0,
                vz: 0.0,
                lap: 0,
                progress: 0.0,
                boost: 3,
                boosting: false,
                drift: 0.0,
                ack: 0,
            }],
        });
        o.predict_tick(CarInput::default());
        let p = o.race.racers[1].car.pos;
        assert!(
            (p.x - 10.0 * DT).abs() < 1e-4,
            "remote car did not coast: {p}"
        );
    }

    #[test]
    fn nothing_moves_outside_the_racing_phase() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::Phase {
            phase: Phase::Countdown,
            countdown: 3.0,
        });
        let before = o.my_car().unwrap().pos;
        for _ in 0..120 {
            o.predict_tick(CarInput {
                throttle: 1.0,
                steer: 0.0,
                handbrake: false,
                boost: true,
            });
        }
        assert_eq!(o.my_car().unwrap().pos, before, "a car jumped the start");
    }

    #[test]
    fn a_rejection_surfaces_as_a_notice() {
        let mut o = Online::new();
        o.apply(S2C::Rejected {
            reason: "wrong password".into(),
        });
        assert_eq!(o.notice.as_deref(), Some("wrong password"));
        // Joining clears it.
        o.apply(joined(1));
        assert!(o.notice.is_none());
    }

    #[test]
    fn the_roster_tracks_arrivals_and_departures() {
        let mut o = Online::new();
        o.apply(joined(0));
        o.apply(S2C::PlayerJoined {
            meta: PlayerMeta {
                id: 3,
                handle: "b".into(),
                slot: 3,
            },
        });
        assert_eq!(o.roster.len(), 2);
        // A duplicate announcement must not double-list anyone.
        o.apply(S2C::PlayerJoined {
            meta: PlayerMeta {
                id: 3,
                handle: "b".into(),
                slot: 3,
            },
        });
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

    #[test]
    fn an_out_of_range_join_slot_is_refused() {
        let mut online = Online::new();
        online.apply(joined(200));
        assert_eq!(online.screen, Screen::Browsing);
        assert_eq!(online.my_slot, None);
        assert_eq!(
            online.notice.as_deref(),
            Some("server assigned invalid player slot 200")
        );
    }
}
