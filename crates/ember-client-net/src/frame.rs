use std::time::Duration;

/// One exact WebSocket data frame owned by a game codec or handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireFrame {
    /// An exact UTF-8 text payload.
    Text(String),
    /// An exact binary payload.
    Binary(Vec<u8>),
}

impl WireFrame {
    /// Returns the charged payload byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Binary(bytes) => bytes.len(),
        }
    }

    /// Returns whether the payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An application-level keepalive emitted only after outbound inactivity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Keepalive {
    /// Minimum time without another outbound data frame.
    pub interval: Duration,
    /// Exact game- or protocol-owned frame to emit.
    pub frame: WireFrame,
}
