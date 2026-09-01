//! Canonical game-neutral JSON protocol before a connection joins one lobby.
//!
//! This module is additive beside the retained cube protocol. Once joined, the
//! host passes exact text or binary payloads to the selected version codec and
//! does not parse, wrap, default, compress, or translate them here.

#![deny(missing_docs)]
// Protocol-qualified names stay explicit at the host's wire boundary.
#![allow(clippy::module_name_repetitions)]

use std::fmt;

use serde::{Deserialize, Serialize};

/// The first canonical outer-protocol version.
pub const OUTER_VERSION: u16 = 1;

/// Every outer decoder retained by this build, in ascending order.
pub const SUPPORTED_OUTER_VERSIONS: [u16; 1] = [OUTER_VERSION];

/// Maximum charged UTF-8 byte length of one outer JSON text frame.
pub const MAX_OUTER_FRAME_BYTES: usize = 64 * 1024;

/// Permanent bootstrap request; its JSON shape is never reinterpreted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Hello {
    /// Exact outer decoder requested by the peer.
    pub outer_version: u16,
    /// Peer display handle used by host admission metadata.
    pub handle: String,
}

/// Successful selection of one retained outer decoder.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Welcome {
    /// Exact decoder selected for this connection.
    pub outer_version: u16,
    /// Every outer decoder retained by the current host.
    pub supported_outer_versions: Vec<u16>,
}

/// Ungated request for the complete hosted lobby projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ListLobbies;

/// One complete game-version-scoped lobby tuple.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LobbyEntry {
    /// Permanent lowercase ASCII game slug.
    pub game_id: String,
    /// Exact frozen game contract version.
    pub game_version: u32,
    /// Human lobby name scoped by the exact game and version.
    pub lobby_name: String,
    /// Whether admission requires a password, without disclosing its value.
    pub password_protected: bool,
    /// Number of currently admitted peers.
    pub occupancy: u16,
    /// Maximum admitted peers for this lobby.
    pub capacity: u16,
    /// Small version-owned status projection containing no inner state.
    pub status: LobbyStatus,
}

/// Small version-owned status safe for ungated outer listing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LobbyStatus {
    /// Stable compact status code such as `waiting` or `racing`.
    pub code: String,
    /// Optional short display detail.
    pub detail: Option<String>,
}

/// Complete ungated lobby-list response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Lobbies {
    /// Full tuples across all hosted games and versions.
    pub entries: Vec<LobbyEntry>,
}

/// Request to create one lobby under an exact hosted key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreateLobby {
    /// Permanent lowercase ASCII game slug.
    pub game_id: String,
    /// Exact frozen game contract version.
    pub game_version: u32,
    /// Human lobby name scoped by the exact game and version.
    pub lobby_name: String,
    /// Optional password supplied only to host admission.
    pub password: Option<String>,
}

/// Request to join one lobby under an exact hosted key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct JoinLobby {
    /// Permanent lowercase ASCII game slug.
    pub game_id: String,
    /// Exact frozen game contract version.
    pub game_version: u32,
    /// Human lobby name scoped by the exact game and version.
    pub lobby_name: String,
    /// Optional password supplied only to host admission.
    pub password: Option<String>,
}

/// Successful exact admission and the boundary after which payloads are inner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Joined {
    /// Permanent lowercase ASCII game slug.
    pub game_id: String,
    /// Exact frozen game contract version.
    pub game_version: u32,
    /// Human lobby name scoped by the exact game and version.
    pub lobby_name: String,
}

/// Structured refusal for a known game whose exact version is not hosted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionNotHosted {
    /// Requested permanent game slug.
    pub requested_game: String,
    /// Requested frozen contract version.
    pub requested_version: u32,
    /// Sorted versions currently hosted for the requested game.
    pub hosted_versions_for_game: Vec<u32>,
}

/// Structured refusal for a game absent from the live registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameNotHosted {
    /// Requested permanent game slug.
    pub requested_game: String,
    /// Sorted permanent game slugs currently hosted.
    pub hosted_games: Vec<String>,
}

/// Stable machine-readable outer protocol error codes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OuterErrorCode {
    /// The UTF-8 text is not one canonical outer JSON message.
    MalformedMessage,
    /// The frame exceeded the outer byte limit before JSON decoding.
    MessageTooLarge,
    /// The message is not legal in the connection's current outer state.
    UnexpectedMessage,
    /// A second hello was received after decoder selection.
    RepeatedHello,
    /// A non-outer payload arrived before exact lobby admission.
    PayloadBeforeJoin,
    /// The permanent hello requested no decoder retained by this build.
    UnsupportedOuterVersion,
    /// A selector or handle violates stable outer syntax constraints.
    InvalidRequest,
    /// The outer host cannot safely continue the request.
    InternalError,
}

/// Stable outer error response with diagnostic detail.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OuterError {
    /// Stable machine-readable classification.
    pub code: OuterErrorCode,
    /// Human-readable detail that clients must not parse for behavior.
    pub message: String,
}

/// Canonical client-to-host messages while the connection is not joined.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Permanent bootstrap request.
    Hello(Hello),
    /// Ungated complete lobby-list request.
    ListLobbies(ListLobbies),
    /// Exact lobby-creation request.
    CreateLobby(CreateLobby),
    /// Exact lobby-join request.
    JoinLobby(JoinLobby),
}

/// Canonical host-to-client messages while the connection is not joined.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Successful retained-decoder selection.
    Welcome(Welcome),
    /// Complete ungated lobby projection.
    Lobbies(Lobbies),
    /// Successful exact lobby admission.
    Joined(Joined),
    /// Known-game exact-version refusal.
    VersionNotHosted(VersionNotHosted),
    /// Unknown-game refusal.
    GameNotHosted(GameNotHosted),
    /// Stable outer protocol error.
    Error(OuterError),
}

/// A failure at the size-capped outer JSON codec boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OuterCodecError {
    /// Input exceeded the limit before JSON parsing.
    FrameTooLarge {
        /// Charged input bytes.
        actual: usize,
        /// Stable maximum input bytes.
        maximum: usize,
    },
    /// Input is not one canonical outer client message.
    MalformedJson {
        /// Parser detail for diagnostics only.
        detail: String,
    },
    /// The permanent hello requested no retained decoder.
    UnsupportedOuterVersion {
        /// Exact requested decoder version.
        requested: u16,
        /// Retained decoder versions.
        supported: Vec<u16>,
    },
    /// A server message could not be encoded.
    EncodeFailed {
        /// Encoder detail for diagnostics only.
        detail: String,
    },
}

impl OuterCodecError {
    /// Projects this codec failure into a stable outer response.
    #[must_use]
    pub fn outer_error(&self) -> OuterError {
        match self {
            Self::FrameTooLarge { .. } => OuterError {
                code: OuterErrorCode::MessageTooLarge,
                message: "outer frame exceeds the byte limit".to_string(),
            },
            Self::MalformedJson { .. } => OuterError {
                code: OuterErrorCode::MalformedMessage,
                message: "malformed outer JSON message".to_string(),
            },
            Self::UnsupportedOuterVersion { .. } => OuterError {
                code: OuterErrorCode::UnsupportedOuterVersion,
                message: "requested outer version is not supported".to_string(),
            },
            Self::EncodeFailed { .. } => OuterError {
                code: OuterErrorCode::InternalError,
                message: "outer response encoding failed".to_string(),
            },
        }
    }

    /// Returns whether further outer interpretation would be ambiguous.
    #[must_use]
    pub const fn closes_connection(&self) -> bool {
        match self {
            Self::FrameTooLarge { .. }
            | Self::MalformedJson { .. }
            | Self::UnsupportedOuterVersion { .. }
            | Self::EncodeFailed { .. } => true,
        }
    }
}

impl fmt::Display for OuterCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameTooLarge { actual, maximum } => {
                write!(formatter, "outer frame has {actual} bytes; limit is {maximum}")
            }
            Self::MalformedJson { detail } => write!(formatter, "malformed outer JSON: {detail}"),
            Self::UnsupportedOuterVersion {
                requested,
                supported,
            } => write!(
                formatter,
                "unsupported outer version {requested}; supported versions: {supported:?}"
            ),
            Self::EncodeFailed { detail } => {
                write!(formatter, "outer response encoding failed: {detail}")
            }
        }
    }
}

impl std::error::Error for OuterCodecError {}

/// Decodes one size-capped canonical client JSON frame with version selection.
///
/// # Errors
///
/// Returns a stable size, syntax, shape, or unsupported-version failure.
pub fn decode_client_frame(frame: &[u8]) -> Result<ClientMessage, OuterCodecError> {
    if frame.len() > MAX_OUTER_FRAME_BYTES {
        return Err(OuterCodecError::FrameTooLarge {
            actual: frame.len(),
            maximum: MAX_OUTER_FRAME_BYTES,
        });
    }
    let message = serde_json::from_slice::<ClientMessage>(frame).map_err(|error| {
        OuterCodecError::MalformedJson {
            detail: error.to_string(),
        }
    })?;
    if let ClientMessage::Hello(hello) = &message
        && !SUPPORTED_OUTER_VERSIONS.contains(&hello.outer_version)
    {
        return Err(OuterCodecError::UnsupportedOuterVersion {
            requested: hello.outer_version,
            supported: SUPPORTED_OUTER_VERSIONS.to_vec(),
        });
    }
    Ok(message)
}

/// Encodes one canonical server message and enforces the same outer byte cap.
///
/// # Errors
///
/// Returns an encoding or encoded-size failure without emitting a partial frame.
pub fn encode_server_frame(message: &ServerMessage) -> Result<Vec<u8>, OuterCodecError> {
    let frame = serde_json::to_vec(message).map_err(|error| OuterCodecError::EncodeFailed {
        detail: error.to_string(),
    })?;
    if frame.len() > MAX_OUTER_FRAME_BYTES {
        return Err(OuterCodecError::FrameTooLarge {
            actual: frame.len(),
            maximum: MAX_OUTER_FRAME_BYTES,
        });
    }
    Ok(frame)
}

/// One exact post-join WebSocket data payload, opaque to the outer protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InnerPayload {
    /// Exact UTF-8 WebSocket text payload.
    Text(String),
    /// Exact WebSocket binary payload.
    Binary(Vec<u8>),
}

/// Total input alphabet for the canonical connection state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateInput {
    /// A decoded outer message received before admission.
    Outer(ClientMessage),
    /// An exact data payload not decoded by the outer layer.
    Inner(InnerPayload),
    /// A legal WebSocket ping, pong, or other non-close control frame.
    Control,
    /// A WebSocket close or underlying transport closure.
    TransportClosed,
}

/// Canonical connection lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// No permanent hello has selected an outer decoder.
    AwaitHello,
    /// A decoder is selected and lobby operations are legal.
    Browsing {
        /// Selected retained outer decoder.
        outer_version: u16,
    },
    /// Exact lobby admission succeeded and every data payload is inner.
    Joined {
        /// Exact admitted game slug.
        game_id: String,
        /// Exact admitted frozen contract version.
        game_version: u32,
        /// Exact admitted lobby name.
        lobby_name: String,
    },
    /// The connection accepts no further data messages.
    Closed,
}

/// Host action produced by one total connection-state transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateAction {
    /// Select and acknowledge the requested outer decoder.
    AcceptHello(Hello),
    /// Produce a complete ungated lobby projection.
    ListLobbies,
    /// Attempt exact lobby creation while remaining in browsing until admitted.
    CreateLobby(CreateLobby),
    /// Attempt exact lobby join while remaining in browsing until admitted.
    JoinLobby(JoinLobby),
    /// Dispatch the exact payload to the selected version codec.
    DispatchInner(InnerPayload),
    /// Handle a legal WebSocket control frame outside game code.
    HandleControl,
    /// Send a stable outer error and optionally close afterward.
    Reject {
        /// Stable error response.
        error: OuterError,
        /// Whether the host closes after safely sending the error.
        close_after_error: bool,
    },
    /// Complete transport cleanup and remain closed.
    Close,
    /// Ignore input because cleanup has already completed.
    Ignore,
}

/// A host-side lifecycle operation attempted from the wrong state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateMutationError {
    /// Admission completion was attempted outside browsing.
    NotBrowsing,
}

/// The total canonical outer connection state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateMachine {
    state: ConnectionState,
}

impl StateMachine {
    /// Constructs a connection awaiting its permanent hello.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ConnectionState::AwaitHello,
        }
    }

    /// Returns the current canonical lifecycle state.
    #[must_use]
    pub const fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// Applies one input using a total, explicit state transition.
    pub fn transition(&mut self, input: StateInput) -> StateAction {
        match (&self.state, input) {
            (ConnectionState::Closed, _) => StateAction::Ignore,
            (_, StateInput::TransportClosed) => {
                self.state = ConnectionState::Closed;
                StateAction::Close
            }
            (_, StateInput::Control) => StateAction::HandleControl,
            (ConnectionState::AwaitHello, StateInput::Outer(ClientMessage::Hello(hello))) => {
                self.state = ConnectionState::Browsing {
                    outer_version: hello.outer_version,
                };
                StateAction::AcceptHello(hello)
            }
            (ConnectionState::AwaitHello, StateInput::Outer(_)) => StateAction::Reject {
                error: OuterError {
                    code: OuterErrorCode::UnexpectedMessage,
                    message: "hello must be the first outer message".to_string(),
                },
                close_after_error: true,
            },
            (ConnectionState::AwaitHello, StateInput::Inner(_)) => StateAction::Reject {
                error: OuterError {
                    code: OuterErrorCode::PayloadBeforeJoin,
                    message: "inner payload received before lobby admission".to_string(),
                },
                close_after_error: true,
            },
            (ConnectionState::Browsing { .. }, StateInput::Outer(ClientMessage::Hello(_))) => {
                StateAction::Reject {
                    error: OuterError {
                        code: OuterErrorCode::RepeatedHello,
                        message: "hello may be sent only once".to_string(),
                    },
                    close_after_error: true,
                }
            }
            (
                ConnectionState::Browsing { .. },
                StateInput::Outer(ClientMessage::ListLobbies(_)),
            ) => StateAction::ListLobbies,
            (
                ConnectionState::Browsing { .. },
                StateInput::Outer(ClientMessage::CreateLobby(request)),
            ) => StateAction::CreateLobby(request),
            (
                ConnectionState::Browsing { .. },
                StateInput::Outer(ClientMessage::JoinLobby(request)),
            ) => StateAction::JoinLobby(request),
            (ConnectionState::Browsing { .. }, StateInput::Inner(_)) => StateAction::Reject {
                error: OuterError {
                    code: OuterErrorCode::PayloadBeforeJoin,
                    message: "inner payload received before lobby admission".to_string(),
                },
                close_after_error: true,
            },
            (ConnectionState::Joined { .. }, StateInput::Inner(payload)) => {
                StateAction::DispatchInner(payload)
            }
            (ConnectionState::Joined { .. }, StateInput::Outer(_)) => StateAction::Reject {
                error: OuterError {
                    code: OuterErrorCode::UnexpectedMessage,
                    message: "outer message received after lobby admission".to_string(),
                },
                close_after_error: true,
            },
        }
    }

    /// Atomically records successful exact lobby admission.
    ///
    /// # Errors
    ///
    /// Returns an error unless the connection is currently browsing.
    pub fn mark_joined(&mut self, joined: Joined) -> Result<(), StateMutationError> {
        if !matches!(self.state, ConnectionState::Browsing { .. }) {
            return Err(StateMutationError::NotBrowsing);
        }
        self.state = ConnectionState::Joined {
            game_id: joined.game_id,
            game_version: joined.game_version,
            lobby_name: joined.lobby_name,
        };
        Ok(())
    }

    /// Closes the lifecycle idempotently.
    pub fn close(&mut self) {
        self.state = ConnectionState::Closed;
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}
