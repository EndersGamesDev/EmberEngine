//! The Fire Racer wire protocol: JSON over WebSocket.
//!
//! **Why this is not in `pong-core`.** That crate is shared between the arena
//! client's prediction and the authoritative arena server, and the arena's
//! join gate is exact `PROTO_VERSION` equality — bumping it makes every frozen
//! hub build list-only until the server is redeployed in the same window.
//! Racing messages have no business being able to trigger that. Fire carries
//! its own version, and the two games can never break each other.
//!
//! Transport and shape follow the house style set by `pong-core::proto`:
//! `#[serde(tag = "t", rename_all = "snake_case")]`, JSON text frames, and
//! `#[serde(default)]` on anything added after v1.
//!
//! On `#[serde(default)]`: it makes a new field *decode* against an old peer.
//! Decoding is not working. For every defaulted field below, the comment says
//! what an old peer actually DOES when the field is absent — and if the answer
//! is "plays a different game", the version gets bumped instead.

use serde::{Deserialize, Serialize};

/// Fire's own protocol version. Independent of `pong_core::proto::PROTO_VERSION`.
pub const PROTO_VERSION: u16 = 1;

pub const MAX_HANDLE_LEN: usize = 20;
pub const MAX_LOBBY_LEN: usize = 24;
pub const MAX_PASSWORD_LEN: usize = 40;
/// Cars per race, matching the grid the circuit is laid out for.
pub const MAX_PLAYERS: u8 = 8;
/// Simulation ticks between state broadcasts: 60 Hz sim, 30 Hz on the wire.
pub const STATE_EVERY_TICKS: u64 = 2;
/// Clients ping at least this often.
pub const CLIENT_PING_SECS: u64 = 5;
/// The server drops a peer that has been silent longer than this.
pub const CLIENT_TIMEOUT_SECS: u64 = 30;
/// A single frame larger than this is a protocol violation, not a big update.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LobbyInfo {
    pub name: String,
    pub host: String,
    pub has_password: bool,
    pub players: u8,
    pub cap: u8,
    /// True once the race has started; a spectator can still join but will
    /// be placed at the back.
    #[serde(default)]
    pub racing: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerMeta {
    pub id: u8,
    pub handle: String,
    /// Grid slot, which also picks the livery — so every client paints the
    /// same car the same colour without another round trip.
    pub slot: u8,
}

/// One car in a state broadcast.
///
/// Flat scalars on purpose: the sim's `Car` holds `glam::Vec2`, and keeping
/// the wire shape separate means the internal struct can be refactored without
/// that being a protocol question.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CarState {
    pub id: u8,
    pub x: f32,
    pub z: f32,
    /// Heading, radians. Sent separately from velocity because a drifting car
    /// is precisely one whose nose and velocity disagree — deriving heading
    /// from velocity would render every slide straight.
    pub yaw: f32,
    pub vx: f32,
    pub vz: f32,
    pub lap: u32,
    /// Distance covered along the racing line. The ordering key, monotonic
    /// across laps, so standings never tie at the start line.
    pub progress: f32,
    pub boost: u8,
    #[serde(default)]
    pub boosting: bool,
    /// 0..1 drift intensity, for tyre smoke on remote cars.
    #[serde(default)]
    pub drift: f32,
    /// The client's last input sequence number this state includes. The
    /// client replays everything after it; without this there is nothing to
    /// reconcile against and prediction cannot converge.
    #[serde(default)]
    pub ack: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Waiting,
    Countdown,
    Racing,
    Finished,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum C2S {
    /// Must be the first message on a connection.
    Hello { proto: u16, handle: String },
    ListLobbies,
    CreateLobby { name: String, password: Option<String> },
    JoinLobby { name: String, password: Option<String> },
    LeaveLobby,
    /// Vote to start. The race begins when every player in the lobby is
    /// ready, or when the lobby is full.
    Ready { ready: bool },
    /// Held driver intents. Doubles as the liveness keepalive.
    Input {
        /// Client-assigned, monotonic. Echoed back as `CarState::ack` so the
        /// client knows how much of its prediction the server has consumed.
        seq: u32,
        throttle: f32,
        steer: f32,
        handbrake: bool,
        /// A boost PRESS, not the held key: "the player pressed boost since
        /// my last input", consumed by the sim on one tick. The repo already
        /// paid for this once with jump — a held flag re-triggers on every
        /// packet the server receives, which here would drain all three
        /// charges in three frames. Defaulted, so an older client simply
        /// never boosts, which is a worse race but a legal one.
        #[serde(default)]
        boost: bool,
    },
    Ping { nonce: u32 },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    /// Reply to a valid Hello.
    Welcome { proto: u16 },
    /// A refused Hello, join or create. The connection stays open so the
    /// client can show the reason and try something else.
    Rejected { reason: String },
    Lobbies { lobbies: Vec<LobbyInfo> },
    /// Reply to a successful create or join. `roster` includes the joiner.
    Joined { lobby: String, id: u8, slot: u8, laps: u32, roster: Vec<PlayerMeta> },
    PlayerJoined { meta: PlayerMeta },
    PlayerLeft { id: u8 },
    /// Race lifecycle. `countdown` is seconds remaining, 0 outside Countdown.
    Phase { phase: Phase, countdown: f32 },
    State { tick: u64, cars: Vec<CarState> },
    /// Finishing order, by player id.
    Results { order: Vec<u8> },
    Pong { nonce: u32 },
}

/// Windows `ERROR_IO_PENDING` / `WSA_IO_PENDING`.
///
/// A socket with `SO_RCVTIMEO` set does not always report a timed-out read as
/// `WSAETIMEDOUT`. When the read times out part-way through an overlapped
/// operation Windows returns 997 instead, and Rust has no `ErrorKind` for it —
/// `kind()` is `Uncategorized`, so it matches neither `WouldBlock` nor
/// `TimedOut`.
const WINDOWS_IO_PENDING: i32 = 997;

/// True when a read error means "nothing to read yet", not "this connection is
/// finished".
///
/// Every read loop in this game is a short-timeout poll: read for a few
/// milliseconds, do other work, read again. Getting this predicate wrong is
/// not a cosmetic error — the loops treat anything else as fatal and exit, so
/// a misclassified transient error kills the reader permanently and the peer
/// goes silently deaf for the rest of its life. That is exactly what 997 did:
/// roughly one connection in ten on Windows stopped receiving, which showed up
/// as a joining player never appearing, a race that never went green, and
/// remote cars frozen at the grid — three unrelated-looking symptoms, one
/// cause.
#[must_use]
pub fn is_transient_read(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted
    ) || e.raw_os_error() == Some(WINDOWS_IO_PENDING)
}

/// Names and handles arrive from the network. Strip control characters and
/// cap the length, or a peer can write terminal escapes into the server's log
/// and the other players' lobby lists.
#[must_use]
pub fn sanitize(s: &str, max: usize) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).take(max).collect();
    if cleaned.trim().is_empty() {
        String::new()
    } else {
        cleaned
    }
}

#[must_use]
pub fn sanitize_handle(s: &str) -> String {
    let h = sanitize(s, MAX_HANDLE_LEN);
    if h.is_empty() {
        "driver".to_string()
    } else {
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + serde::de::DeserializeOwned + std::fmt::Debug>(v: &T) -> T {
        let s = serde_json::to_string(v).expect("encode");
        assert!(s.len() < MAX_FRAME_BYTES, "frame too large");
        serde_json::from_str(&s).expect("decode")
    }

    #[test]
    fn messages_round_trip() {
        let c = C2S::Input { seq: 42, throttle: 1.0, steer: -0.5, handbrake: true, boost: true };
        assert!(matches!(roundtrip(&c), C2S::Input { seq: 42, handbrake: true, .. }));

        let s = S2C::State {
            tick: 900,
            cars: vec![CarState {
                id: 3, x: 1.5, z: -2.5, yaw: 0.75, vx: 12.0, vz: -3.0,
                lap: 2, progress: 1840.0, boost: 1, boosting: true, drift: 0.4, ack: 41,
            }],
        };
        match roundtrip(&s) {
            S2C::State { tick, cars } => {
                assert_eq!(tick, 900);
                assert_eq!(cars[0].id, 3);
                assert_eq!(cars[0].ack, 41);
                assert!(cars[0].boosting);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    /// The tag discipline is what lets both ends match on `t`. If the casing
    /// or the tag key drifts, every message becomes an unknown variant.
    #[test]
    fn the_wire_shape_is_the_house_style() {
        let s = serde_json::to_string(&C2S::ListLobbies).unwrap();
        assert_eq!(s, r#"{"t":"list_lobbies"}"#);
        let s = serde_json::to_string(&S2C::Pong { nonce: 7 }).unwrap();
        assert_eq!(s, r#"{"t":"pong","nonce":7}"#);
        let s = serde_json::to_string(&S2C::Phase { phase: Phase::Countdown, countdown: 1.5 }).unwrap();
        assert_eq!(s, r#"{"t":"phase","phase":"countdown","countdown":1.5}"#);
    }

    /// Every `#[serde(default)]` field must actually decode when absent —
    /// that is the whole reason it is there.
    #[test]
    fn defaulted_fields_decode_from_an_older_peer() {
        // An older client that never learned about boost.
        let old = r#"{"t":"input","seq":5,"throttle":1.0,"steer":0.0,"handbrake":false}"#;
        match serde_json::from_str::<C2S>(old).expect("old input must decode") {
            C2S::Input { seq, boost, .. } => {
                assert_eq!(seq, 5);
                assert!(!boost, "an absent boost flag must mean 'did not press'");
            }
            other => panic!("wrong variant: {other:?}"),
        }
        // An older server that sends no ack, drift or boosting.
        let old = r#"{"t":"state","tick":1,"cars":[{"id":0,"x":0,"z":0,"yaw":0,"vx":0,"vz":0,
                     "lap":0,"progress":0,"boost":3}]}"#;
        match serde_json::from_str::<S2C>(old).expect("old state must decode") {
            S2C::State { cars, .. } => {
                assert_eq!(cars[0].ack, 0);
                assert_eq!(cars[0].drift, 0.0);
                assert!(!cars[0].boosting);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    /// The three symptoms this predicate caused when it was wrong are all in
    /// its doc comment; this is the test that keeps it right.
    #[test]
    fn a_timed_out_read_is_not_a_dead_connection() {
        use std::io::{Error, ErrorKind};
        for kind in [ErrorKind::WouldBlock, ErrorKind::TimedOut, ErrorKind::Interrupted] {
            assert!(is_transient_read(&Error::new(kind, "poll")), "{kind:?} must be transient");
        }
        // The one that actually bit: Windows returns 997 for a read that timed
        // out inside an overlapped operation. The whole reason it slipped
        // through is that Rust gives it no useful `ErrorKind` — so assert
        // exactly that, which is what makes the raw-os-error check necessary.
        let pending = Error::from_raw_os_error(WINDOWS_IO_PENDING);
        assert!(
            !matches!(
                pending.kind(),
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
            ),
            "997 now has a sensible ErrorKind; the raw-os-error check can go"
        );
        assert!(is_transient_read(&pending), "os error 997 must be transient");
    }

    #[test]
    fn a_real_failure_is_still_fatal() {
        use std::io::{Error, ErrorKind};
        for kind in [
            ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted,
            ErrorKind::BrokenPipe,
            ErrorKind::UnexpectedEof,
        ] {
            assert!(!is_transient_read(&Error::new(kind, "gone")), "{kind:?} must be fatal");
        }
    }

    #[test]
    fn hostile_names_are_defanged() {
        assert_eq!(sanitize_handle("\u{1b}[31mred"), "[31mred");
        assert_eq!(sanitize_handle("   "), "driver");
        assert_eq!(sanitize_handle(""), "driver");
        assert_eq!(sanitize_handle(&"x".repeat(100)).len(), MAX_HANDLE_LEN);
        assert_eq!(sanitize("a\nb", MAX_LOBBY_LEN), "ab");
    }

    /// The join gate is exact equality, so a mismatched peer must be told
    /// what it is speaking rather than silently playing a different game.
    #[test]
    fn a_version_mismatch_is_rejectable() {
        let hello = C2S::Hello { proto: PROTO_VERSION + 1, handle: "x".into() };
        match roundtrip(&hello) {
            C2S::Hello { proto, .. } => assert_ne!(proto, PROTO_VERSION),
            other => panic!("wrong variant: {other:?}"),
        }
        let r = S2C::Rejected { reason: format!("this build speaks fire protocol v{PROTO_VERSION}") };
        assert!(matches!(roundtrip(&r), S2C::Rejected { .. }));
    }
}
