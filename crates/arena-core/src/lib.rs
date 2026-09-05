// Preserve deterministic simulation expression ordering and established rounding.
#![allow(clippy::suboptimal_flops)]
// Existing square-root formulas are part of the deterministic simulation trajectory.
#![allow(clippy::imprecise_flops)]

//! Shared Arena core: the pure deterministic simulation and the online
//! wire protocol. No engine, no platform — both the wgpu client and the
//! headless matchmaking server build on this.

pub mod freight_yard;
pub mod harbor;
pub mod proto;
pub mod shooter;
pub mod sim;
