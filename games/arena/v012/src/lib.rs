//! Frozen Arena protocol and gameplay contract, version 12.

#![deny(missing_docs)]

mod adapter;

pub use adapter::{
    ArenaCodec, ArenaFactory, ArenaLegacyAction, ArenaLegacyDecoder, ArenaLegacyIngressFactory,
    LegacyLobbyEntry, game_key,
};
pub mod proto;
pub mod shooter;

/// Identifier used by the hosted-contract fixture gate.
pub const FIXTURE_SUITE_ID: &str = "arena-v12-hosted-contract";
