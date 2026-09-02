//! CPU truth and numeric interfaces for the Julibrot lab.

mod big;
mod plane;
mod scale;
mod types;

pub use big::{BigCentre, BigScalar, EncodedBigScalar, decode_big_scalar, encode_big_scalar};
pub use plane::{construct_plane, construct_plane_from_spec, preset_spec};
pub use scale::{
    centre_displacement_px, mirror_centre, precision_for, scale_split, scaled_pixel_offset,
    scaled_pixel_scale, shallow_pixel_scale, split_centre, split_scalar,
};
pub use types::{
    Axis4, CentreF64, CentreSplit, ComputedOrbit, EscapeGridRecord, EscapeParams, EscapeSample,
    MathError, OrbitStep, PerturbSample, Plane, PlaneAngles, PlanePreset, PlaneSpec, Pose,
    PrecisionPlan, ReferenceOrbitRecord, ScaleSplit, ScaledPixelScale, ViewMode, WarpMatrix,
};
