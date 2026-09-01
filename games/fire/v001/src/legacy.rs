//! Legacy-query ingress for the deployed Fire protocol 1 page and bundle.

use ember_legacy::{DecodedInput, EncodedEvent, GameKey, InnerCodecError, InnerFrame};

use crate::hosted::GAME_ID;
use crate::proto::{
    self, C2S, LobbyInfo, MAX_LOBBY_LEN, MAX_PASSWORD_LEN, PROTO_VERSION, S2C,
};

/// Closed query-selector value accepted for the deployed Fire client.
pub const LEGACY_SELECTOR: &str = "fire";

/// Canonical lobby selection synthesized from a selector-free legacy request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyLobbyRequest {
    /// Exact game/version key derived from the preceding Fire hello.
    pub selection: GameKey,
    /// Fire-sanitized lobby name.
    pub lobby_name: String,
    /// Password using the deployed create or join normalization rule.
    pub password: Option<String>,
}

/// One Fire lobby projected from canonical host state into the deployed list schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyLobby {
    /// Lobby name scoped to Fire protocol 1.
    pub name: String,
    /// Display handle of the first admitted member.
    pub host: String,
    /// Whether joining requires a password.
    pub has_password: bool,
    /// Number of admitted human drivers.
    pub players: u8,
    /// Maximum admitted human drivers.
    pub cap: u8,
    /// Whether the race has left the waiting state.
    pub racing: bool,
}

/// Host action produced by one decoded legacy Fire frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyFireAction {
    /// Record a Fire hello and send the bundled client its protocol-1 welcome.
    Hello {
        /// Exact canonical key synthesized from the hello's independent Fire version.
        selection: GameKey,
        /// Fire-sanitized handle to use for later admission.
        handle: String,
        /// Exact legacy `welcome` event.
        welcome: EncodedEvent,
    },
    /// Return only Fire lobbies through [`project_lobbies`].
    ListLobbies,
    /// Create a lobby under the exact synthesized selection.
    CreateLobby(LegacyLobbyRequest),
    /// Join a lobby under the exact synthesized selection.
    JoinLobby(LegacyLobbyRequest),
    /// Detach from the current lobby while retaining legacy browsing state.
    LeaveLobby,
    /// Dispatch an exact post-join protocol-1 payload to [`crate::hosted::FireSession`].
    SessionInput(DecodedInput),
    /// Send an exact legacy refusal while leaving the peer in browsing.
    Reply(EncodedEvent),
}

/// Stateful decoder for the `legacy_game=fire` ingress selected by the URL.
#[derive(Clone, Debug, Default)]
pub struct LegacyFireAdapter {
    requested_proto: u16,
    handle: Option<String>,
}

impl LegacyFireAdapter {
    /// Decodes one deployed-client text frame and projects selector-free requests.
    ///
    /// # Errors
    ///
    /// Returns a codec error for binary, oversized, or malformed protocol-1 frames.
    pub fn decode(&mut self, frame: &InnerFrame) -> Result<LegacyFireAction, InnerCodecError> {
        let InnerFrame::Text(text) = frame else {
            return Err(InnerCodecError::WrongFrameKind);
        };
        if text.len() > proto::MAX_FRAME_BYTES {
            return Err(InnerCodecError::InvalidFrame(format!(
                "fire protocol 1 frame is {} bytes; maximum is {}",
                text.len(),
                proto::MAX_FRAME_BYTES
            )));
        }
        let message = serde_json::from_str::<C2S>(text)
            .map_err(|error| InnerCodecError::DecodeFailed(error.to_string()))?;
        match message {
            C2S::Hello {
                proto: version,
                handle,
            } => {
                self.requested_proto = version;
                let handle = proto::sanitize_handle(&handle);
                self.handle = Some(handle.clone());
                Ok(LegacyFireAction::Hello {
                    selection: selection(version),
                    handle,
                    welcome: encode(&S2C::Welcome {
                        proto: PROTO_VERSION,
                    })?,
                })
            }
            C2S::ListLobbies => Ok(LegacyFireAction::ListLobbies),
            C2S::CreateLobby { name, password } => {
                if let Some(refusal) = self.version_refusal()? {
                    return Ok(LegacyFireAction::Reply(refusal));
                }
                let lobby_name = proto::sanitize(&name, MAX_LOBBY_LEN);
                if lobby_name.is_empty() {
                    return Ok(LegacyFireAction::Reply(rejected("lobby needs a name")?));
                }
                let password = password
                    .map(|value| proto::sanitize(&value, MAX_PASSWORD_LEN))
                    .filter(|value| !value.is_empty());
                Ok(LegacyFireAction::CreateLobby(LegacyLobbyRequest {
                    selection: self.selection(),
                    lobby_name,
                    password,
                }))
            }
            C2S::JoinLobby { name, password } => {
                if let Some(refusal) = self.version_refusal()? {
                    return Ok(LegacyFireAction::Reply(refusal));
                }
                Ok(LegacyFireAction::JoinLobby(LegacyLobbyRequest {
                    selection: self.selection(),
                    lobby_name: proto::sanitize(&name, MAX_LOBBY_LEN),
                    password,
                }))
            }
            C2S::LeaveLobby => Ok(LegacyFireAction::LeaveLobby),
            C2S::Ready { .. } | C2S::Input { .. } | C2S::Ping { .. } => {
                Ok(LegacyFireAction::SessionInput(DecodedInput {
                    payload: text.as_bytes().to_vec(),
                }))
            }
        }
    }

    /// Returns the most recently synthesized Fire selection.
    #[must_use]
    pub fn selection(&self) -> GameKey {
        selection(self.requested_proto)
    }

    /// Returns the Fire-sanitized handle from the most recent hello.
    #[must_use]
    pub fn handle(&self) -> Option<&str> {
        self.handle.as_deref()
    }

    fn version_refusal(&self) -> Result<Option<EncodedEvent>, InnerCodecError> {
        if self.requested_proto == PROTO_VERSION {
            return Ok(None);
        }
        rejected(&format!(
            "this build speaks fire protocol v{}, the live game is v{PROTO_VERSION}",
            self.requested_proto
        ))
        .map(Some)
    }
}

/// Encodes the deployed Fire `lobbies` response tag and field layout.
///
/// # Errors
///
/// Returns an encoding error if a supplied value cannot be represented as JSON.
pub fn project_lobbies(lobbies: &[LegacyLobby]) -> Result<EncodedEvent, InnerCodecError> {
    let projected = lobbies
        .iter()
        .map(|lobby| LobbyInfo {
            name: lobby.name.clone(),
            host: lobby.host.clone(),
            has_password: lobby.has_password,
            players: lobby.players,
            cap: lobby.cap,
            racing: lobby.racing,
        })
        .collect();
    encode(&S2C::Lobbies {
        lobbies: projected,
    })
}

/// Encodes a browsing-safe refusal using Fire's deployed `rejected` variant.
///
/// # Errors
///
/// Returns an encoding error if the refusal cannot be represented as JSON.
pub fn rejected(reason: &str) -> Result<EncodedEvent, InnerCodecError> {
    encode(&S2C::Rejected {
        reason: reason.to_string(),
    })
}

fn selection(proto: u16) -> GameKey {
    GameKey {
        game_id: GAME_ID.to_string(),
        game_version: u32::from(proto),
    }
}

fn encode(message: &S2C) -> Result<EncodedEvent, InnerCodecError> {
    serde_json::to_vec(message)
        .map(|payload| EncodedEvent { payload })
        .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))
}
