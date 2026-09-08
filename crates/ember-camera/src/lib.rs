//! Exact camera state and navigation for an image plane in N-dimensional space.

#![no_std]

#[cfg(test)]
extern crate std;

mod basis;
mod fixed;
mod frame;
mod navigation;
mod projection;
mod scale;
mod types;

pub use basis::rebuild_basis;
pub use fixed::{CameraError, FIXED_INTEGER_BITS, Fixed};
pub use navigation::{
    MAX_SCREEN_COORDINATE_PIXELS, click, frame_points, pan, rotate_about, select_box, zoom_about,
};
pub use projection::{
    PROJECT_RANGE_PIXELS, PROJECT_READOUT_ULPS, invert_perspective, project, reference_displacement,
};
pub use scale::{Scale, scale_for};
pub use types::{
    Basis, EXPONENT_QUANTA_PER_OCTAVE, Exponent, MAX_EXPONENT_QUANTA, MIN_EXPONENT_QUANTA,
    Observer, Orientation, Screen, Turn, View, ViewBytes,
};
