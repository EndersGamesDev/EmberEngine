// Preserve the protocol's established floating-point operation ordering.
#![allow(clippy::suboptimal_flops)]

//! Shared multiplayer protocol: message types, wire framing, and the few
//! constants both sides must agree on.
//!
//! Transport is a plain TCP stream carrying length-prefixed frames
//! (u32 LE length, then a postcard-encoded message). TCP was chosen because
//! the deployment path runs through `WireGuard` userspace tunnels whose port
//! forwarding is TCP-only; the framing layer keeps the transport swappable.

use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Canonical JSON/WebSocket bootstrap and lobby protocol.
pub mod outer;

pub const PROTOCOL_VERSION: u16 = 2;
pub const DEFAULT_PORT: u16 = 7777;

/// Simulation and snapshot tick rate of the server.
pub const TICK_HZ: u32 = 60;
/// Players move on the XZ plane inside [-`ARENA_HALF`, `ARENA_HALF`]^2.
pub const ARENA_HALF: f32 = 20.0;
pub const MOVE_SPEED: f32 = 10.0;

/// Upper bound on a single frame; anything larger is a protocol violation.
pub const MAX_FRAME_BYTES: u32 = 64 * 1024;
pub const MAX_NAME_LEN: usize = 24;
/// Server drops a client that has been silent this long (the client sends
/// periodic input keepalives well below this).
pub const CLIENT_TIMEOUT_SECS: u64 = 10;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct PlayerId(pub u32);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerMeta {
    pub id: PlayerId,
    pub name: String,
    pub color: [f32; 3],
    /// Position at the time the meta was sent, so a newly-learned player
    /// renders in place instead of sliding in from the origin.
    pub pos: [f32; 2],
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PlayerState {
    pub id: PlayerId,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMsg {
    /// Must be the first message on a connection.
    Hello {
        protocol: u16,
        name: String,
    },
    /// Sets the held movement intent; applied until the next Input.
    /// Also serves as the liveness keepalive.
    Input {
        move_dir: [f32; 2],
    },
    Ping {
        nonce: u32,
    },
    Bye,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMsg {
    /// Reply to a valid Hello. `roster` includes the new player itself.
    Welcome {
        id: PlayerId,
        tick_hz: u32,
        arena_half: f32,
        roster: Vec<PlayerMeta>,
    },
    Reject {
        reason: String,
    },
    PlayerJoined {
        meta: PlayerMeta,
    },
    PlayerLeft {
        id: PlayerId,
    },
    Snapshot {
        tick: u64,
        players: Vec<PlayerState>,
    },
    Pong {
        nonce: u32,
    },
}

/// Writes one length-prefixed postcard message to `w`.
///
/// # Errors
///
/// Returns an error if serialization fails, the encoded message exceeds the
/// protocol frame limit, or the writer cannot accept or flush the frame.
pub fn write_msg<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let bytes =
        postcard::to_stdvec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if bytes.len() > MAX_FRAME_BYTES as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let frame_len = u32::try_from(bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "frame too large"))?;
    w.write_all(&frame_len.to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

/// Reads one length-prefixed postcard message from `r`.
///
/// # Errors
///
/// Returns an error if the frame header or body cannot be read, the declared
/// frame exceeds the protocol limit, or the message cannot be deserialized.
pub fn read_msg<R: Read, T: DeserializeOwned>(r: &mut R) -> io::Result<T> {
    let mut len_bytes = [0u8; 4];
    r.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes);
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    postcard::from_bytes(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Stable per-player color so every client renders the same world.
#[must_use]
pub const fn color_for(id: PlayerId) -> [f32; 3] {
    const PALETTE: [[f32; 3]; 8] = [
        [0.90, 0.30, 0.25], // red
        [0.25, 0.55, 0.95], // blue
        [0.30, 0.80, 0.40], // green
        [0.95, 0.75, 0.20], // yellow
        [0.70, 0.40, 0.90], // purple
        [0.95, 0.50, 0.20], // orange
        [0.25, 0.80, 0.80], // teal
        [0.90, 0.45, 0.70], // pink
    ];
    PALETTE[id.0 as usize % PALETTE.len()]
}

/// Movement intents from the network are untrusted: strip NaN/inf and cap
/// the magnitude so no client can move faster than anyone else.
#[must_use]
pub fn sanitize_dir(dir: [f32; 2]) -> [f32; 2] {
    let [x, y] = dir;
    if !x.is_finite() || !y.is_finite() {
        return [0.0, 0.0];
    }
    let len_sq = x * x + y * y;
    if len_sq > 1.0 {
        let len = len_sq.sqrt();
        [x / len, y / len]
    } else {
        [x, y]
    }
}

#[must_use]
pub fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_LEN)
        .collect();
    if cleaned.trim().is_empty() {
        "player".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_messages() {
        let msgs = vec![
            ServerMsg::Welcome {
                id: PlayerId(3),
                tick_hz: TICK_HZ,
                arena_half: ARENA_HALF,
                roster: vec![PlayerMeta {
                    id: PlayerId(3),
                    name: "ender".into(),
                    color: color_for(PlayerId(3)),
                    pos: [4.0, -1.0],
                }],
            },
            ServerMsg::Snapshot {
                tick: 12345,
                players: vec![PlayerState {
                    id: PlayerId(3),
                    pos: [1.5, -2.5],
                    vel: [0.0, 10.0],
                }],
            },
        ];
        let mut buf = Vec::new();
        for m in &msgs {
            write_msg(&mut buf, m).unwrap();
        }
        let mut cursor = std::io::Cursor::new(buf);
        for m in &msgs {
            let got: ServerMsg = read_msg(&mut cursor).unwrap();
            // Debug-compare is enough for a wire roundtrip check.
            assert_eq!(format!("{m:?}"), format!("{got:?}"));
        }
    }

    #[test]
    fn sanitize_rejects_bad_input() {
        assert_eq!(sanitize_dir([f32::NAN, 0.5]), [0.0, 0.0]);
        assert_eq!(sanitize_dir([f32::INFINITY, 0.0]), [0.0, 0.0]);
        let d = sanitize_dir([3.0, 4.0]);
        assert!((d[0] * d[0] + d[1] * d[1] - 1.0).abs() < 1e-5);
        assert_eq!(sanitize_dir([0.5, 0.5]), [0.5, 0.5]);
        assert_eq!(sanitize_name("\u{7}\u{8}"), "player");
        assert_eq!(sanitize_name("ender"), "ender");
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(MAX_FRAME_BYTES + 1).to_le_bytes());
        buf.extend_from_slice(&[0u8; 16]);
        let mut cursor = std::io::Cursor::new(buf);
        let res: io::Result<ClientMsg> = read_msg(&mut cursor);
        assert!(res.is_err());
    }
}
