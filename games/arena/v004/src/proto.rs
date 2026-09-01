//! Online protocol: JSON text frames over WebSocket. JSON (rather than the
//! arena's binary postcard) because the lobby browser on the web page speaks
//! it natively from JavaScript, and traffic is small (~30 Hz states).
//!
//! v2: the match is the drop-in arena shooter. A lobby IS a running game —
//! creating one starts it with the host inside; joiners drop straight in.

use serde::{Deserialize, Serialize};

/// Frozen Arena v4 proto version contract value.
pub const PROTO_VERSION: u16 = 4;
/// Frozen Arena v4 max handle len contract value.
pub const MAX_HANDLE_LEN: usize = 20;
/// Frozen Arena v4 max lobby len contract value.
pub const MAX_LOBBY_LEN: usize = 24;
/// Frozen Arena v4 max password len contract value.
pub const MAX_PASSWORD_LEN: usize = 40;
/// State broadcast every Nth sim tick (60 Hz sim -> 30 Hz state).
/// Frozen Arena v4 state every ticks contract value.
pub const STATE_EVERY_TICKS: u64 = 2;
/// Clients ping at least this often; the server drops peers silent > 30 s.
/// Frozen Arena v4 client ping secs contract value.
pub const CLIENT_PING_SECS: u64 = 5;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
/// Frozen Arena v4 LobbyInfo wire record.
pub struct LobbyInfo {
    /// Frozen Arena v4 name field.
    pub name: String,
    /// Frozen Arena v4 host field.
    pub host: String,
    /// Frozen Arena v4 has password field.
    pub has_password: bool,
    /// Frozen Arena v4 players field.
    pub players: u8,
    /// Frozen Arena v4 cap field.
    pub cap: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
/// Frozen Arena v4 PlayerMeta wire record.
pub struct PlayerMeta {
    /// Frozen Arena v4 id field.
    pub id: u8,
    /// Frozen Arena v4 handle field.
    pub handle: String,
    /// Frozen Arena v4 color field.
    pub color: [f32; 3],
}

/// Per-player state inside a State broadcast.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
/// Frozen Arena v4 PState wire record.
pub struct PState {
    /// Frozen Arena v4 id field.
    pub id: u8,
    /// Frozen Arena v4 x field.
    pub x: f32,
    /// Frozen Arena v4 z field.
    pub z: f32,
    /// Aim direction (normalized).
    /// Frozen Arena v4 ax field.
    pub ax: f32,
    /// Frozen Arena v4 az field.
    pub az: f32,
    /// Frozen Arena v4 hp field.
    pub hp: u8,
    /// Frozen Arena v4 score field.
    pub score: u32,
    /// Frozen Arena v4 alive field.
    pub alive: bool,
    /// Frozen Arena v4 crouch field.
    pub crouch: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
/// Frozen Arena v4 BState wire record.
pub struct BState {
    /// Frozen Arena v4 x field.
    pub x: f32,
    /// Frozen Arena v4 z field.
    pub z: f32,
    /// Frozen Arena v4 vx field.
    pub vx: f32,
    /// Frozen Arena v4 vz field.
    pub vz: f32,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
/// Frozen Arena v4 C2S message family.
pub enum C2S {
    /// Must be the first message on a connection.
    Hello {
        /// Frozen Arena v4 proto field.
        proto: u16,
        /// Frozen Arena v4 handle field.
        handle: String,
    },
    /// Frozen Arena v4 ListLobbies message.
    ListLobbies,
    CreateLobby {
        /// Frozen Arena v4 name field.
        name: String,
        /// Frozen Arena v4 password field.
        password: Option<String>,
    },
    JoinLobby {
        /// Frozen Arena v4 name field.
        name: String,
        /// Frozen Arena v4 password field.
        password: Option<String>,
    },
    /// Frozen Arena v4 LeaveLobby message.
    LeaveLobby,
    /// Held intents: movement, aim, trigger, stance. Doubles as the
    /// keepalive.
    Input {
        /// Frozen Arena v4 mx field.
        mx: f32,
        /// Frozen Arena v4 my field.
        my: f32,
        /// Frozen Arena v4 ax field.
        ax: f32,
        /// Frozen Arena v4 az field.
        az: f32,
        /// Frozen Arena v4 fire field.
        fire: bool,
        #[serde(default)]
        /// Frozen Arena v4 sprint field.
        sprint: bool,
        #[serde(default)]
        /// Frozen Arena v4 crouch field.
        crouch: bool,
    },
    Ping {
        /// Frozen Arena v4 nonce field.
        nonce: u32,
    },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
/// Frozen Arena v4 S2C message family.
pub enum S2C {
    Welcome {
        /// Frozen Arena v4 proto field.
        proto: u16,
        /// Frozen Arena v4 motd field.
        motd: String,
    },
    /// Recoverable failures (wrong password, name taken, ...). The
    /// connection stays open.
    Error {
        /// Frozen Arena v4 message field.
        message: String,
    },
    LobbyList {
        /// Frozen Arena v4 lobbies field.
        lobbies: Vec<LobbyInfo>,
    },
    /// You are in a game (created or joined). `players` includes yourself;
    /// generate the arena locally from `seed`.
    GameJoined {
        /// Frozen Arena v4 id field.
        id: u8,
        /// Frozen Arena v4 seed field.
        seed: u64,
        /// Frozen Arena v4 arena half field.
        arena_half: f32,
        /// Frozen Arena v4 players field.
        players: Vec<PlayerMeta>,
    },
    PlayerJoined {
        /// Frozen Arena v4 meta field.
        meta: PlayerMeta,
    },
    PlayerLeft {
        /// Frozen Arena v4 id field.
        id: u8,
    },
    State {
        /// Frozen Arena v4 tick field.
        tick: u64,
        /// Frozen Arena v4 players field.
        players: Vec<PState>,
        /// Frozen Arena v4 bullets field.
        bullets: Vec<BState>,
    },
    Kill {
        /// Frozen Arena v4 killer field.
        killer: u8,
        /// Frozen Arena v4 victim field.
        victim: u8,
    },
    Pong {
        /// Frozen Arena v4 nonce field.
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
