//! The shared browser loader: what a page knows while its game arrives.
//!
//! A live page used to learn which server it would join only after its game
//! bundle had compiled and initialised, because the protocol number it filters
//! hosts by came from the bundle's own export. On the page carrying the
//! largest bundle that is about a minute of a chip reading "server …" with
//! nothing said about how far the download had got.
//!
//! This crate owns the rules of that load and nothing else. It holds no
//! sockets, opens no requests and touches no DOM: the tracked
//! `web/loader.ts.j2` performs the four pieces of browser input and output —
//! discovery through the emitted root `hosts.js`, the counting fetch, the
//! compile, and the call into the game's own wasm-bindgen glue — and drives
//! this machine with the clock as it goes. The machine decides what is legal,
//! what the numbers are, and what the page is
//! told, which is why all of it is tested here natively with no browser.
//!
//! Layering: this is platform code. It depends on no game crate and on no
//! engine crate, and it is the same bundle for every page, built once and
//! shipped once, so no game carries loading code of its own.

pub mod event;
pub mod phase;
pub mod progress;
pub mod text;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use event::{Event, Status};
pub use phase::{Machine, Phase, STALL_MS};
pub use progress::Progress;

/// Every loader type supplied to the phase 0b renderer.
#[must_use]
pub const fn boundary_descriptions() -> &'static [&'static ember_boundary::Description] {
    ember_boundary::boundary_descriptions![Phase, Status, Event]
}
