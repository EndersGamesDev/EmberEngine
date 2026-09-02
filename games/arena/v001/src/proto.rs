//! Arena protocol 1: JSON text frames over WebSocket.

use serde::{Deserialize, Serialize};

/// Frozen Arena inner-protocol version.
pub const PROTO_VERSION: u16 = 1;
/// Maximum accepted player-handle length in Unicode scalar values.
pub const MAX_HANDLE_LEN: usize = 20;
/// Maximum accepted lobby-name length in Unicode scalar values.
pub const MAX_LOBBY_LEN: usize = 24;
/// Maximum accepted lobby-password length in Unicode scalar values.
pub const MAX_PASSWORD_LEN: usize = 40;
/// State broadcast every second simulation tick (60 Hz simulation to 30 Hz state).
pub const STATE_EVERY_TICKS: u64 = 2;
/// Expected maximum interval between client pings.
pub const CLIENT_PING_SECS: u64 = 5;

/// One protocol-1 lobby-list entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LobbyInfo {
    /// Lobby name.
    pub name: String,
    /// Display handle of the waiting host.
    pub host: String,
    /// Whether the lobby requires a password.
    pub has_password: bool,
}

/// Client-to-server protocol-1 message.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum C2S {
    /// Must be the first message on an era connection.
    Hello {
        /// Requested protocol version.
        proto: u16,
        /// Requested player handle.
        handle: String,
    },
    /// Requests the waiting-lobby list.
    ListLobbies,
    /// Creates a waiting lobby.
    CreateLobby {
        /// Requested lobby name.
        name: String,
        /// Optional lobby password.
        password: Option<String>,
    },
    /// Joins a waiting lobby.
    JoinLobby {
        /// Requested lobby name.
        name: String,
        /// Optional lobby password.
        password: Option<String>,
    },
    /// Leaves the current lobby.
    LeaveLobby,
    /// Updates the held paddle intent, nominally in the range -1 through 1.
    Input {
        /// Horizontal paddle intent.
        axis: f32,
    },
    /// Requests an immediate pong response.
    Ping {
        /// Client-selected value echoed by the server.
        nonce: u32,
    },
}

/// Server-to-client protocol-1 message.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    /// Confirms the selected protocol.
    Welcome {
        /// Live protocol version.
        proto: u16,
        /// Server message of the day.
        motd: String,
    },
    /// Reports a recoverable failure while keeping the connection open.
    Error {
        /// Human-readable failure detail.
        message: String,
    },
    /// Returns waiting lobbies.
    LobbyList {
        /// Waiting lobby entries.
        lobbies: Vec<LobbyInfo>,
    },
    /// Confirms that the peer created a lobby and is waiting for an opponent.
    LobbyCreated {
        /// Created lobby name.
        name: String,
    },
    /// Announces a match and assigns the peer's paddle role.
    MatchStart {
        /// Zero for the near/blue paddle and one for the far/red paddle.
        role: u8,
        /// Opponent display handle.
        opponent: String,
    },
    /// Broadcasts one authoritative state checkpoint.
    State {
        /// Era server tick.
        tick: u64,
        /// Ball coordinates in `(x, z)` order.
        ball: [f32; 2],
        /// Near and far paddle x coordinates.
        paddles: [f32; 2],
        /// Near and far player scores.
        scores: [u32; 2],
        /// Whether the ball is waiting to serve.
        serving: bool,
    },
    /// Announces a point or match win.
    MatchEvent {
        /// Scoring player index.
        scorer: u8,
        /// Whether the point won the match.
        won: bool,
        /// Post-event scores.
        scores: [u32; 2],
    },
    /// Announces that the opponent left and the lobby is waiting again.
    OpponentLeft,
    /// Echoes a client ping nonce.
    Pong {
        /// Echoed client value.
        nonce: u32,
    },
}

/// Removes controls and surrounding whitespace, then caps the character count.
#[must_use]
pub fn sanitize_text(text: &str, max: usize) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(max)
        .collect()
}

/// Clamps a finite movement intent to the trusted range and maps non-finite values to zero.
#[must_use]
pub const fn sanitize_axis(axis: f32) -> f32 {
    if axis.is_finite() {
        axis.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // Sanitizer results are exact frozen values.
mod tests {
    use super::{C2S, PROTO_VERSION, S2C, sanitize_axis, sanitize_text};

    #[test]
    fn json_roundtrip() {
        let messages = [
            serde_json::to_string(&C2S::Hello {
                proto: PROTO_VERSION,
                handle: "ender".into(),
            })
            .unwrap(),
            serde_json::to_string(&C2S::CreateLobby {
                name: "duel".into(),
                password: Some("hunter2".into()),
            })
            .unwrap(),
        ];
        assert!(messages[0].contains("\"t\":\"hello\""));
        let back: C2S = serde_json::from_str(&messages[1]).unwrap();
        assert!(matches!(
            back,
            C2S::CreateLobby {
                name,
                password
            } if name == "duel" && password.as_deref() == Some("hunter2")
        ));
        let state = serde_json::to_string(&S2C::State {
            tick: 42,
            ball: [1.0, -2.0],
            paddles: [0.5, -0.5],
            scores: [3, 4],
            serving: false,
        })
        .unwrap();
        let back: S2C = serde_json::from_str(&state).unwrap();
        assert!(matches!(back, S2C::State { tick: 42, .. }));
    }

    #[test]
    fn sanitizers() {
        assert_eq!(sanitize_text("  hi\u{7}there  ", 5), "hithe");
        assert_eq!(sanitize_axis(f32::NAN), 0.0);
        assert_eq!(sanitize_axis(5.0), 1.0);
    }
}
