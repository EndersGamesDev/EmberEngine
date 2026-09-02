//! Online protocol: JSON text frames over WebSocket.
//!
//! JSON (rather than the
//! arena's binary postcard) because the lobby browser on the web page speaks
//! it natively from JavaScript, and traffic is small (~30 Hz states).
//!
//! v2: the match is the drop-in arena shooter. A lobby IS a running game —
//! creating one starts it with the host inside; joiners drop straight in.

use serde::{Deserialize, Serialize};

/// Frozen Arena wire-protocol version.
pub const PROTO_VERSION: u16 = 6;
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
/// One lobby row in the Arena browser response.
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
/// Stable public metadata for one admitted Arena player.
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
    /// Aim direction (normalized).
    pub ax: f32,
    /// Horizontal aim direction Z component.
    pub az: f32,
    /// Remaining hit points.
    pub hp: u8,
    /// Authoritative kill score.
    pub score: u32,
    /// Whether the player is active rather than awaiting respawn.
    pub alive: bool,
    /// Whether the player uses the crouched stance.
    pub crouch: bool,
    /// Sequence number of this player's last applied Input — their own
    /// client rebases its movement prediction on it.
    #[serde(default)]
    pub ack: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
/// One authoritative bullet in a state broadcast.
pub struct BState {
    /// Horizontal X coordinate.
    pub x: f32,
    /// Horizontal Z coordinate.
    pub z: f32,
    /// Horizontal X velocity.
    pub vx: f32,
    /// Horizontal Z velocity.
    pub vz: f32,
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
    /// Requests the ungated Arena lobby list.
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
        /// Horizontal movement intent X component.
        mx: f32,
        /// Horizontal movement intent Z component.
        my: f32,
        /// Horizontal aim direction X component.
        ax: f32,
        /// Horizontal aim direction Z component.
        az: f32,
        /// Whether the trigger is held.
        fire: bool,
        /// Whether sprint is held.
        #[serde(default)]
        sprint: bool,
        /// Whether crouch is held.
        #[serde(default)]
        crouch: bool,
    },
    /// Application-level latency probe.
    Ping {
        /// Client-selected value echoed by the server.
        nonce: u32,
    },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    /// Confirms the server protocol and supplies its greeting.
    Welcome {
        /// Arena protocol version spoken by the server.
        proto: u16,
        /// Server greeting.
        motd: String,
    },
    /// Recoverable failures (wrong password, name taken, ...). The
    /// connection stays open.
    Error {
        /// Human-readable refusal or failure.
        message: String,
    },
    /// Current joinable Arena lobbies.
    LobbyList {
        /// Lobby rows visible to this client.
        lobbies: Vec<LobbyInfo>,
    },
    /// You are in a game (created or joined). `players` includes yourself;
    /// generate the arena locally from `seed`.
    GameJoined {
        /// Assigned in-lobby player identifier.
        id: u8,
        /// Deterministic arena-generation seed.
        seed: u64,
        /// Arena boundary half-extent.
        arena_half: f32,
        /// Complete admitted-player roster, including this client.
        players: Vec<PlayerMeta>,
    },
    /// Announces a newly admitted player.
    PlayerJoined {
        /// Public metadata for the new player.
        meta: PlayerMeta,
    },
    /// Announces a departing player.
    PlayerLeft {
        /// In-lobby identifier of the departing player.
        id: u8,
    },
    /// Authoritative periodic simulation snapshot.
    State {
        /// Server simulation tick represented by this snapshot.
        tick: u64,
        /// Authoritative player states.
        players: Vec<PState>,
        /// Authoritative active bullets.
        bullets: Vec<BState>,
    },
    /// Announces a lethal hit.
    Kill {
        /// In-lobby identifier credited with the kill.
        killer: u8,
        /// In-lobby identifier of the victim.
        victim: u8,
    },
    /// Echoes an application-level latency probe.
    Pong {
        /// Client-selected value from the corresponding ping.
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

/// Removes controls and surrounding whitespace, then caps the character count.
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
            fire: true,
            sprint: true,
            crouch: false,
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
