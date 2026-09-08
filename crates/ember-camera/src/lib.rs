//! Exact camera state and navigation for an image plane in N-dimensional space.

#![no_std]

mod fixed;

pub use fixed::{CameraError, FIXED_INTEGER_BITS, Fixed};
