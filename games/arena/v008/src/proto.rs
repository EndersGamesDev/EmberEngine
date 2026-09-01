//! Online protocol: JSON text frames over WebSocket.
//!
//! JSON (rather than the
//! arena's binary postcard) because the lobby browser on the web page speaks
//! it natively from JavaScript, and traffic is small (~30 Hz states).
//!
//! v2: the match is the drop-in arena shooter. A lobby IS a running game —
//! creating one starts it with the host inside; joiners drop straight in.

use serde::{Deserialize, Serialize};

/// v8: shots carry elevation. The added fields are all `#[serde(default)]`
/// and so would decode across the boundary — but decoding is not the test.
/// A pre-v8 client sends no pitch and therefore fires permanently level
/// against a server whose hit volume now has a finite top, and a v8 client
/// against a pre-v8 server reads every bullet height as a defaulted 0.0 and
/// draws every tracer buried in the floor. Both directions are broken in
/// ways a version gate exists precisely to prevent, so this bumps.
pub const PROTO_VERSION: u16 = 8;
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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
// The wire-format booleans are independent protocol fields and cannot be consolidated compatibly.
#[allow(clippy::struct_excessive_bools)]
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
    /// Vertical velocity paired with `y`.
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
        /// Space. Defaulted so an older client simply never jumps.
        #[serde(default)]
        jump: bool,
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
        /// Client nonce echoed in the reply.
        nonce: u32,
    },
}

/// Stable per-player color, by in-lobby id.
#[must_use]
pub const fn color_for(id: u8) -> [f32; 3] {
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
#[must_use]
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
        })
        .unwrap();
        assert!(s.contains("\"t\":\"input\""));
        let back: C2S = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, C2S::Input { fire: true, .. }));

        let s = serde_json::to_string(&S2C::GameJoined {
            id: 2,
            seed: 987_654_321,
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
                seed: 987_654_321,
                ..
            }
        ));
    }

    #[test]
    fn sanitizers() {
        assert_eq!(sanitize_text("  hi\u{7}there  ", 5), "hithe");
        assert_eq!(sanitize_text("\u{7}\u{8}", 5), "");
    }
}
