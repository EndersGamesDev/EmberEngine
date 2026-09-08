//! Exact camera state and navigation for an image plane in N-dimensional space.

#![no_std]

mod basis;
mod fixed;
mod scale;
mod types;

pub use basis::rebuild_basis;
pub use fixed::{CameraError, FIXED_INTEGER_BITS, Fixed};
pub use scale::{Scale, scale_for};
pub use types::{
    Basis, EXPONENT_QUANTA_PER_OCTAVE, Exponent, MAX_EXPONENT_QUANTA, MIN_EXPONENT_QUANTA,
    Observer, Orientation, Screen, Turn, View, ViewBytes,
};
