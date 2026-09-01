//! Frozen Arena protocol and gameplay contract, version 2.

mod adapter;

pub use adapter::{ArenaCodec, ArenaFactory, game_key};
/// Frozen Arena v2 wire protocol.
pub mod proto;
/// Frozen Arena v2 deterministic shooter simulation.
pub mod shooter;

/// Identifier used by the hosted-contract fixture gate.
pub const FIXTURE_SUITE_ID: &str = "arena-v2-hosted-contract";
