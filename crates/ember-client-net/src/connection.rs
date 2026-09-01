use std::collections::VecDeque;

use crate::{
    ConnectionDiagnostics, HandshakeProgress, HandshakeProvider, HandshakeUpdate, SendError,
    TransportConfig, TransportStatus, WebSocketTransport, WireFrame,
};

/// Combined transport and handshake diagnostic snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDiagnostics {
    /// Current game-neutral connection progress.
    pub progress: ConnectionProgress,
    /// Platform transport counters and newest socket failure.
    pub transport: ConnectionDiagnostics,
    /// Recent bounded handshake and enqueue diagnostics.
    pub handshake: Vec<String>,
}

/// Reconnect-oriented progress across transport and pluggable handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionProgress {
    /// The platform WebSocket is not open yet.
    Connecting,
    /// The open transport is awaiting its welcome.
    AwaitingWelcome,
    /// The welcome arrived and lobby selection is pending.
    Selecting,
    /// The connection can browse or retry selection.
    Browsing,
    /// The connection was admitted to one exact lobby.
    Joined,
    /// The transport closed with a structured reason.
    Closed(crate::ConnectionClose),
}

/// Shared lifecycle that drives one pluggable game or canonical handshake.
pub struct ClientConnection<H> {
    transport: WebSocketTransport,
    handshake: H,
    diagnostics: VecDeque<String>,
}

impl<H: HandshakeProvider> ClientConnection<H> {
    /// Starts a WebSocket and installs the provider's keepalive when needed.
    ///
    /// # Errors
    ///
    /// Returns an immediate transport startup failure.
    pub fn connect(url: &str, mut config: TransportConfig, handshake: H) -> Result<Self, String> {
        if config.keepalive.is_none() {
            config.keepalive = handshake.keepalive();
        }
        let transport = WebSocketTransport::connect(url, config)?;
        let mut connection = Self {
            transport,
            handshake,
            diagnostics: VecDeque::new(),
        };
        let update = connection.handshake.opened();
        connection.apply_update(update);
        Ok(connection)
    }

    /// Enqueues an exact post-handshake frame.
    ///
    /// # Errors
    ///
    /// Returns a bounded-outbox or closed-connection failure.
    pub fn send(&self, frame: WireFrame) -> Result<(), SendError> {
        self.transport.send(frame)
    }

    fn record(&mut self, detail: String) {
        self.diagnostics.push_back(detail);
        while self.diagnostics.len() > 16 {
            self.diagnostics.pop_front();
        }
    }

    fn apply_update(&mut self, update: HandshakeUpdate) {
        if let Some(detail) = update.diagnostic {
            self.record(detail);
        }
        for frame in update.outbound {
            if let Err(error) = self.transport.send(frame) {
                self.record(format!("handshake frame could not be queued: {error:?}"));
            }
        }
    }

    /// Pumps transport and handshake state, then forwards every raw game frame.
    pub fn drain(&mut self, output: &mut VecDeque<WireFrame>) {
        let mut incoming = VecDeque::new();
        self.transport.drain(&mut incoming);
        while let Some(frame) = incoming.pop_front() {
            let update = self.handshake.received(&frame);
            self.apply_update(update);
            output.push_back(frame);
        }
    }

    /// Returns combined transport and handshake progress.
    #[must_use]
    pub fn progress(&self) -> ConnectionProgress {
        match self.transport.status() {
            TransportStatus::Connecting => ConnectionProgress::Connecting,
            TransportStatus::Closed(reason) => ConnectionProgress::Closed(reason),
            TransportStatus::Open => match self.handshake.progress() {
                HandshakeProgress::AwaitingWelcome => ConnectionProgress::AwaitingWelcome,
                HandshakeProgress::Selecting => ConnectionProgress::Selecting,
                HandshakeProgress::Browsing => ConnectionProgress::Browsing,
                HandshakeProgress::Joined => ConnectionProgress::Joined,
            },
        }
    }

    /// Returns the underlying WebSocket lifecycle for compatibility adapters.
    #[must_use]
    pub fn transport_status(&self) -> TransportStatus {
        self.transport.status()
    }

    /// Returns counters, progress, and recent bounded diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> ClientDiagnostics {
        ClientDiagnostics {
            progress: self.progress(),
            transport: self.transport.diagnostics(),
            handshake: self.diagnostics.iter().cloned().collect(),
        }
    }
}
