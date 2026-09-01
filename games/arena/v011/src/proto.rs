//! Online protocol: JSON text frames over WebSocket. JSON (rather than the
//! arena's binary postcard) because the lobby browser on the web page speaks
//! it natively from JavaScript, and traffic is small (~30 Hz states).
//!
//! v2: the match is the drop-in arena shooter. A lobby IS a running game —
//! creating one starts it with the host inside; joiners drop straight in.

use serde::{Deserialize, Serialize};

/// v9: the off-hand shield, which reflects. `shield` is `#[serde(default)]`
/// on both the input and the player state, so both directions decode — and
/// decoding is not the test. Ask instead what an old peer DOES.
///
/// A pre-v9 client against a v9 server can never raise a shield, and — the
/// part that decides this — its own perfectly-aimed shot can now come back
/// and kill it, sent by an opponent whose shield it does not render, with no
/// visible cause anywhere on screen. A v9 client against a pre-v9 server has
/// the mirror problem: the server drops the field, so holding Q raises a
/// shield that is drawn, believed, and does nothing, while the player stands
/// still in the open trusting it.
///
/// That is the same shape as the v8 pitch bump rather than the jump one.
/// Jump was additive — an old peer played a smaller game and every existing
/// interaction still resolved the same way. Pitch gave the hit volume a
/// finite top, and the shield gives a hit a second possible outcome: a shot
/// that used to land now legitimately comes back. `serde(default)` protects
/// the wire format, not the meaning of a shot, so this bumps.
///
/// v10: `PState.vy`, the vertical speed at the tick being acked. Position
/// alone cannot restart a second-order integrator. Without it a client
/// rebases the jump replay on the server's y paired with its OWN present
/// velocity, re-integrates gravity across a window forward prediction has
/// already covered, and collapses its predicted arc from 1.687 m to 1.393 m,
/// landing ~200 ms early and writing 29-160 cm of camera correction into the
/// eye thirty times a second.
///
/// Additive in the sense the jump bump was: no existing interaction resolves
/// differently and hit resolution is untouched. It bumps anyway, because the
/// failure mode of NOT bumping is silent. A cached client that reads a field
/// its server never sends defaults it to 0.0 and seeds every replay with
/// zero vertical velocity - worse than the bug being fixed, and invisible.
/// Being told to reload is the better error.
///
/// The general rule, since the same reconciler is one line from repeating
/// this on any future velocity-carrying state: reconciliation must seed
/// EVERY integrator input from the authoritative snapshot, not just the
/// position that snapshot happens to carry.
/// v11: `PState.ack_age_ticks`, plus `Input.jump` becomes a PRESS rather
/// than a held key.
///
/// The age lets a client place the state on its own clock. `ack` said WHICH
/// command the server last had, never how long it had been integrating it,
/// so the replay window was guessed from the send cadence and was 0-50 ms
/// wrong - up to 46 cm of vertical position at take-off speed, alternating
/// sign through a jump because vy does.
///
/// The press is a meaning change, which is what makes this a bump rather
/// than an additive field. The server re-applies the last input every tick,
/// so a held `jump: true` re-launched the player on every grounded tick:
/// land on a crate with Space down and you launched again from 1.5 m, apex
/// 3.19 m, clearing the 2.4 m containers that are supposed to be hard cover.
/// The client now latches the rising edge (which also stops a sub-50 ms tap
/// falling between two sends) and the server consumes it after one tick.
/// Note what that is and is not: a contract about what the flag means, not an
/// enforcement. A client that sets it in every packet still gets one launch
/// per packet - so does this repo's own wsbot unless its jump mode pulses.
pub const PROTO_VERSION: u16 = 11;
/// Maximum sanitized player-handle length.
pub const MAX_HANDLE_LEN: usize = 20;
/// Maximum sanitized lobby-name length.
pub const MAX_LOBBY_LEN: usize = 24;
/// Maximum sanitized lobby-password length.
pub const MAX_PASSWORD_LEN: usize = 40;
/// State broadcast every Nth sim tick (60 Hz sim -> 30 Hz state).
pub const STATE_EVERY_TICKS: u64 = 2;
/// Clients ping at least this often; the server drops peers silent > 30 s.
pub const CLIENT_PING_SECS: u64 = 5;

/// One lobby row in the deployed Arena browser response.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LobbyInfo {
    /// Sanitized lobby name.
    pub name: String,
    /// Display handle of the first member.
    pub host: String,
    /// Whether joining requires a password.
    pub has_password: bool,
    /// Current admitted player count.
    pub players: u8,
    /// Maximum admitted player count.
    pub cap: u8,
}

/// Stable public metadata for one admitted Arena player.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerMeta {
    /// In-lobby player identifier.
    pub id: u8,
    /// Sanitized display handle.
    pub handle: String,
    /// Stable color derived from the player identifier.
    pub color: [f32; 3],
}

/// Per-player state inside a State broadcast.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PState {
    /// In-lobby player identifier.
    pub id: u8,
    /// Authoritative horizontal X coordinate.
    pub x: f32,
    /// Authoritative horizontal Z coordinate.
    pub z: f32,
    /// Feet height: 0 on the floor, a crate top when standing on cover.
    /// Defaulted, so a pre-jump server simply reports everyone grounded.
    #[serde(default)]
    pub y: f32,
    /// Vertical speed at the tick this state acks. The client's jump replay
    /// has to restart gravity from the server's velocity, not from its own;
    /// defaulted, so a pre-v10 server simply reports everyone at rest.
    #[serde(default)]
    pub vy: f32,
    /// HORIZONTAL aim direction (normalized).
    pub ax: f32,
    /// Horizontal aim direction Z component.
    pub az: f32,
    /// Aim elevation in radians, positive = up. Sent so remote players'
    /// weapons tilt with where they are actually looking; defaulted, so a
    /// pre-pitch server simply reports everyone aiming level.
    #[serde(default)]
    pub pitch: f32,
    /// Remaining hit points.
    pub hp: u8,
    /// Authoritative kill score.
    pub score: u32,
    /// Whether the player is active rather than awaiting respawn.
    pub alive: bool,
    /// Whether the player uses the crouched stance.
    pub crouch: bool,
    /// Off-hand shield raised. Sent because a shield you cannot see is a
    /// mechanic that kills you for no visible reason; defaulted, so a
    /// pre-shield server simply reports everyone unshielded.
    #[serde(default)]
    pub shield: bool,
    /// Weapon level (1 pistol, 2 rapid, 3 heavy).
    #[serde(default)]
    pub weapon: u8,
    /// Rounds remaining in the current magazine.
    #[serde(default)]
    pub ammo: u8,
    /// True while this player's reload is in progress.
    #[serde(default)]
    pub reloading: bool,
    /// Authoritative death count for the scoreboard.
    #[serde(default)]
    pub deaths: u32,
    /// Sequence number of this player's last applied Input — their own
    /// client rebases its movement prediction on it.
    #[serde(default)]
    pub ack: u32,
    /// How many ticks the server has been applying that acked command. With
    /// it (and the round trip it lets the client solve for) the replay can
    /// start at the instant this state describes instead of at a guess.
    #[serde(default)]
    pub ack_age_ticks: u16,
}

/// One authoritative bullet in a state broadcast.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct BState {
    /// Horizontal X coordinate.
    pub x: f32,
    /// Horizontal Z coordinate.
    pub z: f32,
    /// Horizontal X velocity.
    pub vx: f32,
    /// Horizontal Z velocity.
    pub vz: f32,
    /// Height above the floor and its rate of change. Sent so the client
    /// can draw a tracer along the bullet's REAL path; it used to guess the
    /// vertical part locally from its own aim, which is why the tracer and
    /// the shot disagreed. Defaulted, so a pre-pitch server reads as flat.
    #[serde(default)]
    pub y: f32,
    /// Vertical velocity, paired with `y` above; defaulted for the same reason.
    #[serde(default)]
    pub vy: f32,
    /// Firing player — clients use it for shot audio cues.
    #[serde(default)]
    pub owner: u8,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum C2S {
    /// Must be the first message on a connection.
    Hello {
        /// Arena protocol version spoken by the client.
        proto: u16,
        /// Requested display handle.
        handle: String,
    },
    /// Requests the ungated legacy Arena lobby list.
    ListLobbies,
    /// Requests creation of a named Arena lobby.
    CreateLobby {
        /// Requested lobby name.
        name: String,
        /// Optional lobby password.
        password: Option<String>,
    },
    /// Requests admission to an existing Arena lobby.
    JoinLobby {
        /// Requested lobby name.
        name: String,
        /// Optional lobby password.
        password: Option<String>,
    },
    /// Leaves the current Arena lobby.
    LeaveLobby,
    /// Held intents: movement, aim, trigger, stance. Doubles as the
    /// keepalive.
    Input {
        /// Client-assigned sequence number, echoed back as PState.ack.
        #[serde(default)]
        seq: u32,
        /// The sim tick this client is currently rendering remote players
        /// at — the server rewinds hit tests to it (lag compensation).
        #[serde(default)]
        view_tick: u64,
        /// World-space movement intent on the X axis.
        mx: f32,
        /// World-space movement intent on the Z axis.
        my: f32,
        /// Horizontal aim X component.
        ax: f32,
        /// Horizontal aim Z component.
        az: f32,
        /// Aim elevation in radians, positive = up. A SCALAR beside the
        /// horizontal aim, deliberately not a third component of it — the
        /// sim keeps `ax`/`az` unit-length and reads elevation separately.
        /// Defaulted, so an older client simply always fires level.
        #[serde(default)]
        pitch: f32,
        /// Held trigger intent.
        fire: bool,
        /// Held sprint intent.
        #[serde(default)]
        sprint: bool,
        /// Held crouch intent.
        #[serde(default)]
        crouch: bool,
        /// Held reload intent.
        #[serde(default)]
        reload: bool,
        /// A Space PRESS, not the held key: true means "the player pressed
        /// jump since my last input", and the sim consumes it on one tick.
        /// Held-key semantics re-launched the player on every grounded tick,
        /// because the server keeps applying the last input it received.
        /// Defaulted so an older client simply never jumps.
        #[serde(default)]
        jump: bool,
        /// Q, held. Defaulted so an older client simply never raises one —
        /// which is exactly why the version gate above had to move too.
        #[serde(default)]
        shield: bool,
    },
    /// Application keepalive request.
    Ping {
        /// Client nonce echoed in the reply.
        nonce: u32,
    },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    /// Confirms the legacy hello.
    Welcome {
        /// Live Arena protocol version.
        proto: u16,
        /// Deployed message of the day.
        motd: String,
    },
    /// Recoverable failures (wrong password, name taken, ...). The
    /// connection stays open.
    Error {
        /// Human-readable recoverable failure.
        message: String,
    },
    /// Returns visible Arena lobbies using the deployed tag.
    LobbyList {
        /// Visible, non-full lobby rows.
        lobbies: Vec<LobbyInfo>,
    },
    /// You are in a game (created or joined). `players` includes yourself;
    /// generate the arena locally from `seed`.
    GameJoined {
        /// Assigned in-lobby player identifier.
        id: u8,
        /// Seed used by clients and server to regenerate the arena.
        seed: u64,
        /// Arena boundary half-extent.
        arena_half: f32,
        /// Full roster including the newly admitted player.
        players: Vec<PlayerMeta>,
    },
    /// Announces a newly admitted player to existing members.
    PlayerJoined {
        /// Public metadata for the new member.
        meta: PlayerMeta,
    },
    /// Announces a departed player.
    PlayerLeft {
        /// Departed in-lobby player identifier.
        id: u8,
    },
    /// Periodic authoritative simulation snapshot.
    State {
        /// Authoritative simulation tick.
        tick: u64,
        /// Authoritative player states in simulation order.
        players: Vec<PState>,
        /// Authoritative bullets in simulation order.
        bullets: Vec<BState>,
        /// Weapon-pad availability, index-aligned with the seeded pad
        /// positions every client derives locally.
        #[serde(default)]
        pads: Vec<bool>,
    },
    /// Announces one authoritative kill event.
    Kill {
        /// Credited killer identifier.
        killer: u8,
        /// Victim identifier.
        victim: u8,
    },
    /// Echoes an application keepalive nonce.
    Pong {
        /// Client nonce from the corresponding ping.
        nonce: u32,
    },
}

/// Stable per-player color, by in-lobby id.
pub fn color_for(id: u8) -> [f32; 3] {
    const PALETTE: [[f32; 3]; 8] = [
        [0.25, 0.55, 0.95], // blue
        [0.92, 0.32, 0.28], // red
        [0.30, 0.80, 0.40], // green
        [0.95, 0.75, 0.20], // yellow
        [0.70, 0.40, 0.90], // purple
        [0.95, 0.50, 0.20], // orange
        [0.25, 0.80, 0.80], // teal
        [0.90, 0.45, 0.70], // pink
    ];
    PALETTE[id as usize % PALETTE.len()]
}

/// Removes control characters and surrounding whitespace, then caps character count.
pub fn sanitize_text(s: &str, max: usize) -> String {
    // Strip controls, trim, THEN cap — surrounding whitespace must not
    // consume the length budget.
    s.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(max)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip() {
        let s = serde_json::to_string(&C2S::Input {
            seq: 7,
            view_tick: 120,
            mx: 1.0,
            my: 0.0,
            ax: 0.5,
            az: -0.5,
            pitch: -0.3,
            fire: true,
            sprint: true,
            crouch: false,
            reload: false,
            jump: true,
            shield: true,
        })
        .unwrap();
        assert!(s.contains("\"t\":\"input\""));
        let back: C2S = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            C2S::Input {
                fire: true,
                shield: true,
                ..
            }
        ));

        let s = serde_json::to_string(&S2C::GameJoined {
            id: 2,
            seed: 987654321,
            arena_half: 24.0,
            players: vec![PlayerMeta {
                id: 2,
                handle: "ender".into(),
                color: color_for(2),
            }],
        })
        .unwrap();
        let back: S2C = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            S2C::GameJoined {
                id: 2,
                seed: 987654321,
                ..
            }
        ));
    }

    #[test]
    fn the_shield_survives_the_codec_in_both_directions() {
        // The player state carries it, so remote shields can be drawn.
        let p = PState {
            id: 3,
            x: 1.0,
            z: -2.0,
            y: 0.5,
            vy: -1.5,
            ax: 0.0,
            az: 1.0,
            pitch: 0.2,
            hp: 2,
            score: 4,
            alive: true,
            crouch: false,
            shield: true,
            weapon: 2,
            ammo: 7,
            reloading: false,
            deaths: 1,
            ack: 42,
            ack_age_ticks: 7,
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"shield\":true"), "{s}");
        let back: PState = serde_json::from_str(&s).unwrap();
        assert!(back.shield);
        assert_eq!(back.ack_age_ticks, 7, "the ack age has to survive the codec");

        // And both fields decode from a frame that predates them: this is
        // what `serde(default)` buys, and it is the whole of what it buys —
        // the version gate is what stops an old peer from PLAYING against
        // this, for the reasons written at PROTO_VERSION.
        let old_state = r#"{"id":1,"x":0.0,"z":0.0,"ax":1.0,"az":0.0,
            "hp":3,"score":0,"alive":true,"crouch":false}"#;
        let p: PState = serde_json::from_str(old_state).unwrap();
        assert!(!p.shield, "an absent shield reads as lowered");
        let old_input = r#"{"t":"input","mx":0.0,"my":0.0,"ax":1.0,"az":0.0,"fire":false}"#;
        let back: C2S = serde_json::from_str(old_input).unwrap();
        assert!(matches!(back, C2S::Input { shield: false, .. }));
    }

    #[test]
    fn sanitizers() {
        assert_eq!(sanitize_text("  hi\u{7}there  ", 5), "hithe");
        assert_eq!(sanitize_text("\u{7}\u{8}", 5), "");
    }
}
