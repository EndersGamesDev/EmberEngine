//! Shared, game-neutral client networking scaffolding.
//!
//! Transport lifecycle and prediction bookkeeping live here. Games retain
//! ownership of wire payloads, simulation meaning, and presentation policy.

#![deny(missing_docs)]
