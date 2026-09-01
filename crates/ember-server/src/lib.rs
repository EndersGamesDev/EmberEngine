//! Game-neutral host for every compiled and explicitly manifested game version.
//!
//! The original server header's authority shape is preserved: all game state lives on ONE
//! simulation-owning hub thread. Network I/O threads never touch version state; they translate
//! WebSocket frames into events and drain one bounded outbound channel per connection. The hub is
//! therefore the single writer of truth while version codecs and sessions remain transport-neutral.

#![deny(missing_docs)]

mod capabilities;
mod connection;
#[cfg(any(test, feature = "demo"))]
mod fixture;
mod registry;
mod runtime;

pub use registry::{Registry, RegistryBuilder, RegistryError, RegistryRegistration};
pub use runtime::{DrainHandle, Host, HostConfig, HostError, OccupancySnapshot};
