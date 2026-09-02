//! CPU truth and numeric interfaces for the Julibrot lab.

mod big;
mod drift;
mod orbit;
mod perturb;
mod plane;
mod scale;
mod types;
mod warp;

pub use big::{BigCentre, BigScalar, EncodedBigScalar, decode_big_scalar, encode_big_scalar};
pub use drift::{navigation_drift_f32, navigation_drift_f64};
pub use orbit::{ReferenceOrbitBuilder, escape_f32, smooth_iteration_f64};
pub use perturb::{perturb_scaled_f64, perturb_scaled_f64_with_envelope};
pub use plane::{construct_plane, construct_plane_from_spec, preset_spec};
pub use scale::{
    centre_displacement_px, centre_from_reference_px, mirror_centre, precision_for,
    reference_shift_px, scale_split, scaled_pixel_offset, scaled_pixel_scale, shallow_pixel_scale,
    split_centre, split_scalar,
};
pub use types::{
    Axis4, CentreF64, CentreSplit, ComputedOrbit, EscapeGridRecord, EscapeParams, EscapeSample,
    MathError, OrbitStep, PerturbSample, PerturbationEnvelope, Plane, PlaneAngles, PlanePreset,
    PlaneSpec, Pose, PrecisionPlan, ReferenceOrbitRecord, ScaleSplit, ScaledPixelScale, ViewMode,
    WarpMatrix,
};
pub use warp::{warp_identity_error, warp_matrix};
