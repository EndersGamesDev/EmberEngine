// Preserve deterministic racing simulation arithmetic and its established rounding.
#![allow(clippy::suboptimal_flops)]
// Course-sampling tests intentionally advance through production f32 coordinates.
#![allow(clippy::while_float)]

//! Shared racing simulation and wire protocol for Fire Racer.
//!
//! This crate freezes Fire protocol 1 behind the evergreen hosted-game interface.

pub mod ai;
pub mod car;
pub mod castle;
pub mod proto;
pub mod sim;
pub mod track;
