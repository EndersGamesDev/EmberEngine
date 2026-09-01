//! Closed product registrations joined to the authoritative hosted manifest.

// Crate visibility documents the sibling-module host boundary behind this private module.
#![allow(clippy::redundant_pub_crate)]

use std::sync::Arc;

use ember_game_fire_v1::hosted::{self, FireCodec, FireFactory};
use ember_game_fire_v1::legacy::LegacyFireIngressFactory;
use ember_game_pong_v1::hosted::{PongCodec, PongFactory};
use ember_game_what_is_this_v1::{WhatIsThisCodec, WhatIsThisFactory};
use ember_legacy::{MonotonicDuration, VersionLimits};

use crate::{RegistryBuilder, RegistryError, RegistryRegistration};

pub(crate) fn register(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    builder.register(RegistryRegistration::new(
        ember_game_arena_v2::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v2::ArenaCodec::new()),
        Arc::new(ember_game_arena_v2::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v3::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v3::ArenaCodec::new()),
        Arc::new(ember_game_arena_v3::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v4::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v4::ArenaCodec::new()),
        Arc::new(ember_game_arena_v4::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v5::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v5::ArenaCodec::new()),
        Arc::new(ember_game_arena_v5::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v6::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v6::ArenaCodec::new()),
        Arc::new(ember_game_arena_v6::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v7::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v7::ArenaCodec::new()),
        Arc::new(ember_game_arena_v7::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v8::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v8::ArenaCodec::new()),
        Arc::new(ember_game_arena_v8::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v9::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v9::ArenaCodec::new()),
        Arc::new(ember_game_arena_v9::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v10::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v10::ArenaCodec::new()),
        Arc::new(ember_game_arena_v10::ArenaFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_arena_v11::game_key(),
        placeholder_limits(),
        Arc::new(ember_game_arena_v11::ArenaCodec::new()),
        Arc::new(ember_game_arena_v11::ArenaFactory::new()),
    ))?;
    // The single "arena" legacy selector lives on v12: its decoder reads the era hello's
    // protocol number and synthesizes the exact per-version key, so frozen pages for
    // v7 through v11 dispatch through this one ingress.
    builder.register(
        RegistryRegistration::new(
            ember_game_arena_v12::game_key(),
            placeholder_limits(),
            Arc::new(ember_game_arena_v12::ArenaCodec::new()),
            Arc::new(ember_game_arena_v12::ArenaFactory::new()),
        )
        .with_legacy_ingress(Arc::new(ember_game_arena_v12::ArenaLegacyIngressFactory)),
    )?;
    builder.register(
        RegistryRegistration::new(
            hosted::game_key(),
            placeholder_limits(),
            Arc::new(FireCodec),
            Arc::new(FireFactory),
        )
        .with_legacy_ingress(Arc::new(LegacyFireIngressFactory)),
    )?;
    builder.register(RegistryRegistration::new(
        ember_game_pong_v1::hosted::game_key(),
        placeholder_limits(),
        Arc::new(PongCodec::new()),
        Arc::new(PongFactory::new()),
    ))?;
    builder.register(RegistryRegistration::new(
        ember_game_what_is_this_v1::game_key(),
        what_is_this_limits(),
        Arc::new(WhatIsThisCodec),
        Arc::new(WhatIsThisFactory),
    ))
}

// This nonzero profile is a startup-safe placeholder until the estate measurements named by
// `games/hosted.toml` replace it.
const fn placeholder_limits() -> VersionLimits {
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

const fn what_is_this_limits() -> VersionLimits {
    VersionLimits {
        max_lobbies: 32,
        max_players_per_lobby: 1,
        max_frame_bytes: 56 * 1_024,
        max_messages_per_second: 4,
        max_outbound_queue_bytes: 4 * 1_024,
        max_outbound_bytes_per_second: 16 * 1_024,
        max_step_duration: MonotonicDuration::from_micros(5_000),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn product_registry_contains_every_manifest_entry() {
        let mut builder = RegistryBuilder::new();
        register(&mut builder).expect("compiled product registrations must be unique");
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/hosted.toml");
        let registry = builder
            .load(&manifest)
            .expect("compiled registrations must match the hosted manifest");
        assert_eq!(
            registry.hosted_games(),
            ["arena", "fire", "pong", "what-is-this"].map(str::to_string)
        );
        assert_eq!(
            registry
                .exact_key("what-is-this", 1)
                .expect("what-is-this/1 must be registered"),
            ember_game_what_is_this_v1::game_key()
        );
    }
}
