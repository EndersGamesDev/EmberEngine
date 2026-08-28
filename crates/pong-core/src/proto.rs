//! Online protocol: JSON text frames over WebSocket. JSON (rather than the
//! arena's binary postcard) because the lobby browser on the web page speaks
//! it natively from JavaScript, and traffic is small (~30 Hz states).
//!
//! v2: the match is the drop-in arena shooter. A lobby IS a running game —
//! creating one starts it with the host inside; joiners drop straight in.

use serde::{Deserialize, Serialize};

pub const PROTO_VERSION: u16 = 2;
pub const MAX_HANDLE_LEN: usize = 20;
pub const MAX_LOBBY_LEN: usize = 24;
pub const MAX_PASSWORD_LEN: usize = 40;
/// State broadcast every Nth sim tick (60 Hz sim -> 30 Hz state).
pub const STATE_EVERY_TICKS: u64 = 2;
/// Clients ping at least this often; the server drops peers silent > 30 s.
pub const CLIENT_PING_SECS: u64 = 5;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LobbyInfo {
    pub name: String,
    pub host: String,
    pub has_password: bool,
    pub players: u8,
    pub cap: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerMeta {
    pub id: u8,
    pub handle: String,
    pub color: [f32; 3],
}

/// Per-player state inside a State broadcast.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PState {
    pub id: u8,
    pub x: f32,
    pub z: f32,
    /// Aim direction (normalized).
    pub ax: f32,
    pub az: f32,
    pub hp: u8,
    pub score: u32,
    pub alive: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct BState {
    pub x: f32,
    pub z: f32,
    pub vx: f32,
    pub vz: f32,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum C2S {
    /// Must be the first message on a connection.
    Hello { proto: u16, handle: String },
    ListLobbies,
    CreateLobby { name: String, password: Option<String> },
    JoinLobby { name: String, password: Option<String> },
    LeaveLobby,
    /// Held intents: movement, aim, trigger. Doubles as the keepalive.
    Input { mx: f32, my: f32, ax: f32, az: f32, fire: bool },
    Ping { nonce: u32 },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    Welcome { proto: u16, motd: String },
    /// Recoverable failures (wrong password, name taken, ...). The
    /// connection stays open.
    Error { message: String },
    LobbyList { lobbies: Vec<LobbyInfo> },
    /// You are in a game (created or joined). `players` includes yourself;
    /// generate the arena locally from `seed`.
    GameJoined {
        id: u8,
        seed: u64,
        arena_half: f32,
        players: Vec<PlayerMeta>,
    },
    PlayerJoined { meta: PlayerMeta },
    PlayerLeft { id: u8 },
    State {
        tick: u64,
        players: Vec<PState>,
        bullets: Vec<BState>,
    },
    Kill { killer: u8, victim: u8 },
    Pong { nonce: u32 },
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
        })
        .unwrap();
        assert!(s.contains("\"t\":\"input\""));
        let back: C2S = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, C2S::Input { fire: true, .. }));

        let s = serde_json::to_string(&S2C::GameJoined {
            id: 2,
            seed: 987654321,
            arena_half: 24.0,
            players: vec![PlayerMeta { id: 2, handle: "ender".into(), color: color_for(2) }],
        })
        .unwrap();
        let back: S2C = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, S2C::GameJoined { id: 2, seed: 987654321, .. }));
    }

    #[test]
    fn sanitizers() {
        assert_eq!(sanitize_text("  hi\u{7}there  ", 5), "hithe");
        assert_eq!(sanitize_text("\u{7}\u{8}", 5), "");
    }
}
