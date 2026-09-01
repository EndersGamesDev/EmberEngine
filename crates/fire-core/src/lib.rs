// Preserve deterministic racing simulation arithmetic and its established rounding.
#![allow(clippy::suboptimal_flops)]
// Course-sampling tests intentionally advance through production f32 coordinates.
#![allow(clippy::while_float)]

//! Shared racing simulation and wire protocol for Fire Racer.
//!
//! Deliberately a separate crate from `arena-core`: that one is shared between
//! the arena client's prediction and the authoritative arena server, and its
//! join gate is exact `PROTO_VERSION` equality. Racing types landing there
//! would put the live arena one careless bump away from list-only.

pub mod ai;
pub mod car;
pub mod castle;
pub mod proto;
pub mod sim;
pub mod track;
