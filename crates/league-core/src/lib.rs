// League maths and wire conversions round constants through f32 the way the
// other cores do; the sim is server-authoritative, so none of it is a
// prediction hazard.
#![allow(clippy::suboptimal_flops)]
#![allow(clippy::imprecise_flops)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
#![allow(clippy::needless_range_loop, clippy::module_name_repetitions)]

//! Ultimate League — the shared simulation and wire format.
//!
//! One-lane, top-down MOBA: 1v1 and 3v3, five champions, minion waves, two
//! side courts and two cores. This crate is pure: no threads, no time, no IO.
//! [`sim::Match`] steps a fixed 60 Hz tick from its own tick counter, and the
//! only randomness is the deterministic `hash(tick, who, salt)` roll — bots
//! included — so the sim is a function of its commands.
//!
//! Layering: `league-server` runs this for real; the client renders snapshots
//! and never simulates abilities. That is the sole reason beams, hooks and
//! movement may use f32 transcendentals freely: there is exactly one peer
//! doing physics.

pub mod ai;
pub mod data;
pub mod kits;
pub mod proto;
pub mod rng;
pub mod sim;

pub use sim::Match;

/// Simulation tick rate, as the other cores state it.
pub const TICK_HZ: u32 = 60;
/// Seconds per tick.
pub const DT: f32 = 1.0 / TICK_HZ as f32;
