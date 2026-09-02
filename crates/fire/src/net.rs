//! Fire protocol 1 adapter over the shared client WebSocket lifecycle.

use std::collections::VecDeque;
use std::time::Duration;

use ember_client_net::{
    ClientConnection, ClientDiagnostics, ConnectionProgress, HookError, InnerFrameCodec, Keepalive,
    LegacyHandshake, LegacyJsonTags, TransportConfig, TransportStatus, WireFrame,
};
use fire_core::proto::{C2S, CLIENT_PING_SECS, S2C};

/// Fire's frozen JSON text-frame codec hook.
#[derive(Clone, Copy, Debug, Default)]
pub struct FireCodec;

impl InnerFrameCodec for FireCodec {
    type Outbound = C2S;
    type Inbound = S2C;

    fn encode_inner(&self, message: &Self::Outbound) -> Result<WireFrame, HookError> {
        serde_json::to_string(message)
            .map(WireFrame::Text)
            .map_err(|error| HookError::Encode(error.to_string()))
    }

    fn decode_inner(&self, frame: &WireFrame) -> Result<Self::Inbound, HookError> {
        let WireFrame::Text(text) = frame else {
            return Err(HookError::WrongFrameKind);
        };
        serde_json::from_str(text).map_err(|error| HookError::Decode(error.to_string()))
    }
}

/// Compatibility lifecycle status retained for callers and frozen tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// The platform socket has not opened yet.
    Connecting,
    /// The platform socket is open, independent of lobby progress.
    Open,
    /// The socket closed with an actionable reason.
    Closed(String),
}

fn keepalive(codec: FireCodec) -> Result<Keepalive, String> {
    codec
        .encode_inner(&C2S::Ping { nonce: 1 })
        .map(|frame| Keepalive {
            interval: Duration::from_secs(CLIENT_PING_SECS),
            frame,
        })
        .map_err(|error| error.to_string())
}

const fn config() -> TransportConfig {
    TransportConfig {
        max_frame_bytes: fire_core::proto::MAX_FRAME_BYTES,
        inbox_capacity: 256,
        outbox_capacity: 256,
        keepalive: None,
    }
}

/// Fire client channel preserving its exact protocol messages over shared plumbing.
pub struct Net {
    connection: ClientConnection<LegacyHandshake>,
    codec: FireCodec,
}

impl Net {
    /// Connects a compatibility channel whose caller sends the legacy hello.
    ///
    /// # Errors
    ///
    /// Returns an immediate transport startup failure.
    pub fn connect(url: &str) -> Result<Self, String> {
        let codec = FireCodec;
        // A manual caller has not guaranteed Hello yet, so no keepalive may race it.
        let handshake = LegacyHandshake::manual(LegacyJsonTags::fire_v1(), None);
        let connection = ClientConnection::connect(url, config(), handshake)?;
        Ok(Self { connection, codec })
    }

    /// Connects and drives Fire's hello, Welcome gate, and create/join exchange.
    ///
    /// # Errors
    ///
    /// Returns an immediate transport or frozen-message encoding failure.
    pub fn connect_session(
        url: &str,
        handle: &str,
        lobby: &str,
        password: Option<String>,
        create: bool,
    ) -> Result<Self, String> {
        let codec = FireCodec;
        let hello = codec
            .encode_inner(&C2S::Hello {
                proto: fire_core::proto::PROTO_VERSION,
                handle: handle.to_string(),
            })
            .map_err(|error| error.to_string())?;
        let selection = if create {
            C2S::CreateLobby {
                name: lobby.to_string(),
                password,
            }
        } else {
            C2S::JoinLobby {
                name: lobby.to_string(),
                password,
            }
        };
        let selection = codec
            .encode_inner(&selection)
            .map_err(|error| error.to_string())?;
        let handshake = LegacyHandshake::automatic(
            hello,
            selection,
            LegacyJsonTags::fire_v1(),
            Some(keepalive(codec)?),
        );
        let connection = ClientConnection::connect(url, config(), handshake)?;
        Ok(Self { connection, codec })
    }

    /// Encodes and queues one exact Fire client message.
    pub fn send(&self, message: &C2S) {
        match self.codec.encode_inner(message) {
            Ok(frame) => {
                if let Err(error) = self.connection.send(frame) {
                    tracing::warn!(?error, "fire net: outbound frame was not queued");
                }
            }
            Err(error) => tracing::warn!(%error, "fire net: outbound frame was not encoded"),
        }
    }

    /// Decodes every currently available Fire server message in order.
    pub fn drain(&mut self, output: &mut VecDeque<S2C>) {
        let mut frames = VecDeque::new();
        self.connection.drain(&mut frames);
        while let Some(frame) = frames.pop_front() {
            match self.codec.decode_inner(&frame) {
                Ok(message) => output.push_back(message),
                Err(error) => tracing::warn!(%error, "fire net: undecodable server frame"),
            }
        }
    }

    /// Returns the compatibility transport status.
    #[must_use]
    pub fn status(&self) -> Status {
        match self.connection.transport_status() {
            TransportStatus::Connecting => Status::Connecting,
            TransportStatus::Open => Status::Open,
            TransportStatus::Closed(reason) => Status::Closed(reason.detail),
        }
    }

    /// Returns shared handshake progress for reconnect-friendly UI state.
    #[must_use]
    pub fn progress(&self) -> ConnectionProgress {
        self.connection.progress()
    }

    /// Returns bounded connection diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> ClientDiagnostics {
        self.connection.diagnostics()
    }
}

/// A queue of messages received but not yet consumed by the game loop.
#[derive(Default)]
pub struct Inbox(pub VecDeque<S2C>);

impl Inbox {
    /// Pumps the shared transport into this game-loop queue.
    pub fn pump(&mut self, net: &mut Net) {
        net.drain(&mut self.0);
    }

    /// Removes the oldest server message.
    pub fn pop(&mut self) -> Option<S2C> {
        self.0.pop_front()
    }
}

/// Sends Fire's frozen direct-server hello for compatibility callers.
pub fn hello(net: &Net, handle: &str) {
    net.send(&C2S::Hello {
        proto: fire_core::proto::PROTO_VERSION,
        handle: handle.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_codec_preserves_the_frozen_hello_shape() {
        let frame = FireCodec
            .encode_inner(&C2S::Hello {
                proto: 1,
                handle: "driver".to_string(),
            })
            .unwrap();
        assert_eq!(
            frame,
            WireFrame::Text(r#"{"t":"hello","proto":1,"handle":"driver"}"#.to_string())
        );
    }

    #[test]
    fn fire_codec_rejects_binary_without_reinterpreting_it() {
        assert!(matches!(
            FireCodec.decode_inner(&WireFrame::Binary(vec![1, 2, 3])),
            Err(HookError::WrongFrameKind)
        ));
    }
}
