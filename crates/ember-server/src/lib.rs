//! Game-neutral host for every compiled and explicitly manifested game version.
//!
//! The original server header's authority shape is preserved: all game state lives on ONE
//! simulation-owning hub thread. Network I/O threads never touch version state; they translate
//! WebSocket frames into events and drain one bounded outbound channel per connection. The hub is
//! therefore the single writer of truth while version codecs and sessions remain transport-neutral.

#![deny(missing_docs)]
// Public host and registry qualifiers remain clear when imported beside version-owned types.
#![allow(clippy::module_name_repetitions)]

mod capabilities;
mod connection;
#[cfg(any(test, feature = "demo"))]
mod fixture;
mod registry;
mod runtime;

pub use registry::{
    Registry, RegistryBuilder, RegistryError, RegistryRegistration, SelectionError,
};
pub use runtime::{DrainHandle, Host, HostConfig, HostError, OccupancySnapshot};

/// Builds the feature-gated fixture registry for end-to-end host demonstrations.
///
/// # Errors
///
/// Returns a fixture registration or injected-manifest validation failure.
#[cfg(feature = "demo")]
pub fn demo_registry() -> Result<Registry, RegistryError> {
    let mut builder = RegistryBuilder::new();
    fixture::register(&mut builder)?;
    builder.build_from_source(fixture::MANIFEST)
}
