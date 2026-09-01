//! Closed product registrations joined to the authoritative hosted manifest.

// Crate visibility documents the sibling-module host boundary behind this private module.
#![allow(clippy::redundant_pub_crate)]

use std::sync::Arc;

use ember_game_arena_v12::{ArenaCodec, ArenaFactory, ArenaLegacyIngressFactory, game_key};
use ember_game_fire_v1::hosted::{self, FireCodec, FireFactory};
use ember_game_fire_v1::legacy::LegacyFireIngressFactory;
use ember_legacy::{MonotonicDuration, VersionLimits};

use crate::{RegistryBuilder, RegistryError, RegistryRegistration};

pub(crate) fn register(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    builder.register(
        RegistryRegistration::new(
            game_key(),
            arena_limits(),
            Arc::new(ArenaCodec::new()),
            Arc::new(ArenaFactory::new()),
        )
        .with_legacy_ingress(Arc::new(ArenaLegacyIngressFactory)),
    )?;
    builder.register(
        RegistryRegistration::new(
            hosted::game_key(),
            fire_limits(),
            Arc::new(FireCodec),
            Arc::new(FireFactory),
        )
        .with_legacy_ingress(Arc::new(LegacyFireIngressFactory)),
    )
}

// These nonzero profiles are startup-safe placeholders until the estate measurements named by
// `games/hosted.toml` replace them.
const fn arena_limits() -> VersionLimits {
    VersionLimits {
        max_lobbies: 64,
        max_players_per_lobby: 8,
        max_frame_bytes: 64 * 1_024,
        max_messages_per_second: 120,
        max_outbound_queue_bytes: 256 * 1_024,
        max_outbound_bytes_per_second: 4 * 1_024 * 1_024,
        max_step_duration: MonotonicDuration::from_micros(20_000),
    }
}

const fn fire_limits() -> VersionLimits {
    VersionLimits {
        max_lobbies: 64,
        max_players_per_lobby: 8,
        max_frame_bytes: 64 * 1_024,
        max_messages_per_second: 120,
        max_outbound_queue_bytes: 256 * 1_024,
        max_outbound_bytes_per_second: 4 * 1_024 * 1_024,
        max_step_duration: MonotonicDuration::from_micros(20_000),
    }
}
