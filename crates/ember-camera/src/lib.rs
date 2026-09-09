//! Exact camera state and navigation for an image plane in N-dimensional space.

#![no_std]

#[cfg(test)]
extern crate std;

pub(crate) mod basis;
mod fixed;
mod frame;
mod navigation;
mod projection;
mod scale;
pub(crate) mod types;

pub use basis::rebuild_basis;
pub use fixed::{CameraError, FIXED_INTEGER_BITS, Fixed};
pub use frame::{orientation_from_frame, same_image_plane};
pub use navigation::{
    EXPONENT_QUANTUM_EDGE_TOLERANCE, MAX_SCREEN_COORDINATE_PIXELS, click, frame_points, pan,
    rotate_about, select_box, zoom_about,
};
pub use projection::{
    PERSPECTIVE_ROUND_TRIP_TOLERANCE_PIXELS, PROJECT_PIXEL_TOLERANCE_PIXELS, PROJECT_RANGE_PIXELS,
    PROJECT_READOUT_LIMIT_PIXELS, PROJECT_READOUT_ULPS, invert_perspective, project,
    project_perspective, reference_displacement,
};
pub use scale::{Scale, scale_for};
pub use types::{
    Basis, EXPONENT_QUANTA_PER_OCTAVE, Exponent, MAX_EXPONENT_QUANTA, MIN_EXPONENT_QUANTA,
    Observer, Orientation, Screen, TWO_STAGE_FRAME_PLANES, Turn, TwoStageProjection, View,
    ViewBytes,
};

#[cfg(test)]
mod timing;
