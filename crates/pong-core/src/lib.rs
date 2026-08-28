//! Shared pong core: the pure deterministic simulation and the online
//! wire protocol. No engine, no platform — both the wgpu client and the
//! headless matchmaking server build on this.

pub mod proto;
pub mod shooter;
pub mod sim;
