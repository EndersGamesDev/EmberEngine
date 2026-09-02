//! Frozen Arena protocol and gameplay contract, version 6.

#![deny(missing_docs)]

mod adapter;

pub use adapter::{ArenaCodec, ArenaFactory, game_key};
pub mod proto;
pub mod shooter;

/// Identifier used by the hosted-contract fixture gate.
pub const FIXTURE_SUITE_ID: &str = "arena-v6-hosted-contract";
