use ember_net::outer::{
    ClientMessage, CreateLobby, Hello, JoinLobby, ListLobbies, ServerMessage,
};

use crate::{Keepalive, WireFrame};

/// Game-neutral progress through transport bootstrap and lobby admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandshakeProgress {
    /// The transport is open and the initial hello has been emitted.
    AwaitingWelcome,
    /// A welcome arrived and the requested selection has been emitted.
    Selecting,
    /// The connection may browse or retry a refused lobby selection.
    Browsing,
    /// One exact lobby has admitted the connection.
    Joined,
}

/// Outbound effects and diagnostics produced by one handshake event.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HandshakeUpdate {
    /// Exact frames to enqueue in order.
    pub outbound: Vec<WireFrame>,
    /// Optional human-readable protocol diagnostic.
    pub diagnostic: Option<String>,
}

/// Pluggable initial exchange driven by the shared connection lifecycle.
pub trait HandshakeProvider {
    /// Called exactly once when the transport starts; queued output is emitted on open.
    fn opened(&mut self) -> HandshakeUpdate;

    /// Observes one inbound frame before the same frame reaches the game.
    fn received(&mut self, frame: &WireFrame) -> HandshakeUpdate;

    /// Returns the current admission progress.
    fn progress(&self) -> HandshakeProgress;

    /// Returns an inactivity keepalive, when this protocol defines one.
    fn keepalive(&self) -> Option<Keepalive>;
}

/// JSON tag names used by a frozen pre-canonical lobby handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyJsonTags {
    /// Field carrying the adjacent enum tag.
    pub field: String,
    /// Tag acknowledging the legacy hello.
    pub welcome: String,
    /// Tag confirming lobby admission.
    pub joined: String,
    /// Tag carrying a retryable refusal.
    pub rejected: String,
}

impl LegacyJsonTags {
    /// Fire protocol 1 tag vocabulary.
    #[must_use]
    pub fn fire_v1() -> Self {
        Self {
            field: "t".to_string(),
            welcome: "welcome".to_string(),
            joined: "joined".to_string(),
            rejected: "rejected".to_string(),
        }
    }
}

enum LegacyMode {
    Automatic {
        hello: WireFrame,
        selection: WireFrame,
    },
    Manual,
}

/// Provider for existing direct game-server hello-then-select protocols.
pub struct LegacyHandshake {
    mode: LegacyMode,
    tags: LegacyJsonTags,
    progress: HandshakeProgress,
    selection_sent: bool,
    keepalive: Option<Keepalive>,
}

impl LegacyHandshake {
    /// Constructs an automatic legacy exchange that waits for welcome.
    #[must_use]
    pub const fn automatic(
        hello: WireFrame,
        selection: WireFrame,
        tags: LegacyJsonTags,
        keepalive: Option<Keepalive>,
    ) -> Self {
        Self {
            mode: LegacyMode::Automatic { hello, selection },
            tags,
            progress: HandshakeProgress::AwaitingWelcome,
            selection_sent: false,
            keepalive,
        }
    }

    /// Constructs a compatibility channel whose caller owns legacy messages.
    #[must_use]
    pub const fn manual(tags: LegacyJsonTags, keepalive: Option<Keepalive>) -> Self {
        Self {
            mode: LegacyMode::Manual,
            tags,
            progress: HandshakeProgress::Browsing,
            selection_sent: false,
            keepalive,
        }
    }

    fn tag(&self, frame: &WireFrame) -> Option<(String, serde_json::Value)> {
        let WireFrame::Text(text) = frame else {
            return None;
        };
        let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
        let tag = value.get(&self.tags.field)?.as_str()?.to_string();
        Some((tag, value))
    }
}

impl HandshakeProvider for LegacyHandshake {
    fn opened(&mut self) -> HandshakeUpdate {
        match &self.mode {
            LegacyMode::Automatic { hello, .. } => HandshakeUpdate {
                outbound: vec![hello.clone()],
                diagnostic: None,
            },
            LegacyMode::Manual => HandshakeUpdate::default(),
        }
    }

    fn received(&mut self, frame: &WireFrame) -> HandshakeUpdate {
        let Some((tag, value)) = self.tag(frame) else {
            return HandshakeUpdate::default();
        };
        if tag == self.tags.welcome {
            let LegacyMode::Automatic { selection, .. } = &self.mode else {
                return HandshakeUpdate::default();
            };
            if !self.selection_sent {
                self.selection_sent = true;
                self.progress = HandshakeProgress::Selecting;
                return HandshakeUpdate {
                    outbound: vec![selection.clone()],
                    diagnostic: None,
                };
            }
        } else if tag == self.tags.joined {
            self.progress = HandshakeProgress::Joined;
        } else if tag == self.tags.rejected {
            self.progress = HandshakeProgress::Browsing;
            let detail = value
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("legacy server refused the request");
            return HandshakeUpdate {
                outbound: Vec::new(),
                diagnostic: Some(detail.to_string()),
            };
        }
        HandshakeUpdate::default()
    }

    fn progress(&self) -> HandshakeProgress {
        self.progress.clone()
    }

    fn keepalive(&self) -> Option<Keepalive> {
        self.keepalive.clone()
    }
}

/// Canonical action taken after the outer welcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSelection {
    /// Request the ungated all-game lobby list.
    Browse,
    /// Create a lobby under an exact game and version.
    Create(CreateLobby),
    /// Join a lobby under an exact game and version.
    Join(JoinLobby),
}

/// Provider for the canonical outer hello, list, and exact selection exchange.
pub struct CanonicalHandshake {
    hello: Hello,
    selection: CanonicalSelection,
    progress: HandshakeProgress,
    selection_sent: bool,
}

impl CanonicalHandshake {
    /// Constructs a canonical exchange for one handle and follow-up action.
    #[must_use]
    pub const fn new(hello: Hello, selection: CanonicalSelection) -> Self {
        Self {
            hello,
            selection,
            progress: HandshakeProgress::AwaitingWelcome,
            selection_sent: false,
        }
    }

    fn encode(message: &ClientMessage) -> Result<WireFrame, String> {
        serde_json::to_string(message)
            .map(WireFrame::Text)
            .map_err(|error| format!("canonical handshake encode failed: {error}"))
    }

    fn selection_message(&self) -> ClientMessage {
        match &self.selection {
            CanonicalSelection::Browse => ClientMessage::ListLobbies(ListLobbies),
            CanonicalSelection::Create(request) => ClientMessage::CreateLobby(request.clone()),
            CanonicalSelection::Join(request) => ClientMessage::JoinLobby(request.clone()),
        }
    }
}

impl HandshakeProvider for CanonicalHandshake {
    fn opened(&mut self) -> HandshakeUpdate {
        match Self::encode(&ClientMessage::Hello(self.hello.clone())) {
            Ok(frame) => HandshakeUpdate {
                outbound: vec![frame],
                diagnostic: None,
            },
            Err(detail) => HandshakeUpdate {
                outbound: Vec::new(),
                diagnostic: Some(detail),
            },
        }
    }

    fn received(&mut self, frame: &WireFrame) -> HandshakeUpdate {
        let WireFrame::Text(text) = frame else {
            return HandshakeUpdate::default();
        };
        let Ok(message) = serde_json::from_str::<ServerMessage>(text) else {
            return HandshakeUpdate::default();
        };
        match message {
            ServerMessage::Welcome(_) if !self.selection_sent => {
                self.selection_sent = true;
                self.progress = HandshakeProgress::Selecting;
                match Self::encode(&self.selection_message()) {
                    Ok(selection) => HandshakeUpdate {
                        outbound: vec![selection],
                        diagnostic: None,
                    },
                    Err(detail) => HandshakeUpdate {
                        outbound: Vec::new(),
                        diagnostic: Some(detail),
                    },
                }
            }
            ServerMessage::Lobbies(_) => {
                self.progress = HandshakeProgress::Browsing;
                HandshakeUpdate::default()
            }
            ServerMessage::Joined(_) => {
                self.progress = HandshakeProgress::Joined;
                HandshakeUpdate::default()
            }
            ServerMessage::VersionNotHosted(refusal) => {
                self.progress = HandshakeProgress::Browsing;
                HandshakeUpdate {
                    outbound: Vec::new(),
                    diagnostic: Some(format!(
                        "{}/{} is not hosted; available versions: {:?}",
                        refusal.requested_game,
                        refusal.requested_version,
                        refusal.hosted_versions_for_game
                    )),
                }
            }
            ServerMessage::GameNotHosted(refusal) => {
                self.progress = HandshakeProgress::Browsing;
                HandshakeUpdate {
                    outbound: Vec::new(),
                    diagnostic: Some(format!(
                        "{} is not hosted; available games: {:?}",
                        refusal.requested_game, refusal.hosted_games
                    )),
                }
            }
            ServerMessage::Error(error) => HandshakeUpdate {
                outbound: Vec::new(),
                diagnostic: Some(error.message),
            },
            ServerMessage::Welcome(_) => HandshakeUpdate {
                outbound: Vec::new(),
                diagnostic: Some("canonical server repeated welcome".to_string()),
            },
        }
    }

    fn progress(&self) -> HandshakeProgress {
        self.progress.clone()
    }

    fn keepalive(&self) -> Option<Keepalive> {
        None
    }
}

#[cfg(test)]
mod tests {
    use ember_net::outer::{Joined, Lobbies, OUTER_VERSION, Welcome};

    use super::*;

    fn text<T: serde::Serialize>(value: &T) -> WireFrame {
        WireFrame::Text(serde_json::to_string(value).unwrap())
    }

    #[test]
    fn legacy_waits_for_welcome_before_selection() {
        let mut handshake = LegacyHandshake::automatic(
            WireFrame::Text(r#"{"t":"hello"}"#.to_string()),
            WireFrame::Text(r#"{"t":"join_lobby"}"#.to_string()),
            LegacyJsonTags::fire_v1(),
            None,
        );
        assert_eq!(handshake.opened().outbound.len(), 1);
        assert_eq!(
            handshake
                .received(&WireFrame::Text(r#"{"t":"state"}"#.to_string()))
                .outbound,
            Vec::<WireFrame>::new()
        );
        let selection =
            handshake.received(&WireFrame::Text(r#"{"t":"welcome","proto":1}"#.to_string()));
        assert_eq!(selection.outbound.len(), 1);
        assert_eq!(handshake.progress(), HandshakeProgress::Selecting);
        assert_eq!(
            handshake
                .received(&WireFrame::Text(r#"{"t":"welcome","proto":1}"#.to_string()))
                .outbound,
            Vec::<WireFrame>::new()
        );
        handshake.received(&WireFrame::Text(r#"{"t":"joined"}"#.to_string()));
        assert_eq!(handshake.progress(), HandshakeProgress::Joined);
    }

    #[test]
    fn canonical_hello_list_and_join_are_welcome_gated() {
        let hello = Hello {
            outer_version: OUTER_VERSION,
            handle: "fake".to_string(),
        };
        let mut browse = CanonicalHandshake::new(hello.clone(), CanonicalSelection::Browse);
        assert!(matches!(
            serde_json::from_str::<ClientMessage>(match &browse.opened().outbound[0] {
                WireFrame::Text(frame) => frame,
                WireFrame::Binary(_) => "",
            }),
            Ok(ClientMessage::Hello(_))
        ));
        let welcome = ServerMessage::Welcome(Welcome {
            outer_version: OUTER_VERSION,
            supported_outer_versions: vec![OUTER_VERSION],
        });
        let update = browse.received(&text(&welcome));
        assert!(matches!(
            serde_json::from_str::<ClientMessage>(match &update.outbound[0] {
                WireFrame::Text(frame) => frame,
                WireFrame::Binary(_) => "",
            }),
            Ok(ClientMessage::ListLobbies(_))
        ));
        browse.received(&text(&ServerMessage::Lobbies(Lobbies { entries: Vec::new() })));
        assert_eq!(browse.progress(), HandshakeProgress::Browsing);

        let mut join = CanonicalHandshake::new(
            hello,
            CanonicalSelection::Join(JoinLobby {
                game_id: "fake".to_string(),
                game_version: 1,
                lobby_name: "room".to_string(),
                password: None,
            }),
        );
        join.opened();
        let update = join.received(&text(&welcome));
        assert!(matches!(
            serde_json::from_str::<ClientMessage>(match &update.outbound[0] {
                WireFrame::Text(frame) => frame,
                WireFrame::Binary(_) => "",
            }),
            Ok(ClientMessage::JoinLobby(_))
        ));
        join.received(&text(&ServerMessage::Joined(Joined {
            game_id: "fake".to_string(),
            game_version: 1,
            lobby_name: "room".to_string(),
        })));
        assert_eq!(join.progress(), HandshakeProgress::Joined);
    }
}
