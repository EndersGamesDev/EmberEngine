//! Rules and wire protocol for Four Kings, the four-corner chess variant.
//!
//! Deliberately its own crate, with no dependency on `pong-core` or
//! `fire-core`. Both of those carry a join gate that is exact
//! `PROTO_VERSION` equality, so a type shared with either would put a live
//! game one careless bump away from list-only. The two helpers this crate
//! has in common with `fire-core` (`sanitize`, `is_transient_read`) are
//! copied, not imported; lifting them into a shared crate is a backlog line.
//!
//! The crate is shared by the authoritative server and the client, for
//! highlights and for applying the server's echo, never for prediction. It
//! is deterministic by construction: no floats, no RNG, no clock of its own
//! (the server or the hotseat client feeds `TurnClock` with elapsed
//! milliseconds).
//!
//! The rules of record are `docs/kings-design.md`; section numbers in the
//! doc comments below refer to it.

#![deny(missing_docs)]

pub mod board;
pub mod clock;
pub mod proto;
pub mod rules;

pub use board::{Dir, Frame, Outcome, Piece, Seat, State, Tile, setup};
pub use clock::TurnClock;
pub use proto::{ActionKind, EndReason, Formation, Kind, LastAction};
pub use rules::{Illegal, Target, TargetKind, apply, disconnect, targets, timeout};
