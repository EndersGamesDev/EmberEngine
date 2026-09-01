//! Frozen Pong protocol 1 simulation, wire codec, and hosted-session adapter.

#![deny(missing_docs)]
// Preserve the era simulation's exact f32 operation order.
#![allow(clippy::suboptimal_flops)]

pub mod hosted;
pub mod proto;
pub mod sim;
