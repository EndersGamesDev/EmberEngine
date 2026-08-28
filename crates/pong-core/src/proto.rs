//! Online protocol: JSON text frames over WebSocket. JSON (rather than the
//! arena's binary postcard) because the lobby browser on the web page speaks
//! it natively from JavaScript, and pong traffic is tiny (~30 Hz, <200 B).

use serde::{Deserialize, Serialize};

pub const PROTO_VERSION: u16 = 1;
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
    /// Held paddle intent, -1..1. Doubles as the in-match keepalive.
    Input { axis: f32 },
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
    /// You created a lobby and are waiting for an opponent.
    LobbyCreated { name: String },
    /// A match begins. role 0 = near/blue paddle (+z), 1 = far/red (-z).
    MatchStart { role: u8, opponent: String },
    State {
        tick: u64,
        ball: [f32; 2],
        paddles: [f32; 2],
        scores: [u32; 2],
        serving: bool,
    },
    /// A point (or the game) was decided. Scores are post-event.
    MatchEvent { scorer: u8, won: bool, scores: [u32; 2] },
    /// Opponent disconnected/left; you are back to waiting in the lobby.
    OpponentLeft,
    Pong { nonce: u32 },
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

/// Movement intents from the network are untrusted.
pub fn sanitize_axis(axis: f32) -> f32 {
    if axis.is_finite() {
        axis.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip() {
        let msgs = vec![
            serde_json::to_string(&C2S::Hello { proto: PROTO_VERSION, handle: "ender".into() })
                .unwrap(),
            serde_json::to_string(&C2S::CreateLobby {
                name: "duel".into(),
                password: Some("hunter2".into()),
            })
            .unwrap(),
        ];
        assert!(msgs[0].contains("\"t\":\"hello\""));
        let back: C2S = serde_json::from_str(&msgs[1]).unwrap();
        match back {
            C2S::CreateLobby { name, password } => {
                assert_eq!(name, "duel");
                assert_eq!(password.as_deref(), Some("hunter2"));
            }
            other => panic!("wrong variant: {other:?}"),
        }
        let s = serde_json::to_string(&S2C::State {
            tick: 42,
            ball: [1.0, -2.0],
            paddles: [0.5, -0.5],
            scores: [3, 4],
            serving: false,
        })
        .unwrap();
        let back: S2C = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, S2C::State { tick: 42, .. }));
    }

    #[test]
    fn sanitizers() {
        assert_eq!(sanitize_text("  hi\u{7}there  ", 5), "hithe");
        assert_eq!(sanitize_axis(f32::NAN), 0.0);
        assert_eq!(sanitize_axis(5.0), 1.0);
    }
}
