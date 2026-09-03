//! CPU truth and numeric interfaces for the Julibrot lab.

mod big;
mod drift;
mod morph;
mod navigation;
mod orbit;
mod perturb;
mod plane;
mod scale;
mod types;
mod warp;

pub use big::{BigCentre, BigScalar, EncodedBigScalar, decode_big_scalar, encode_big_scalar};
pub use drift::{navigation_drift_f32, navigation_drift_f64};
pub use morph::{
    MORPH_EXTRA_BITS, lerp_centre, lerp_f64, lerp_origin, lerp_plane_angles, lerp_view,
    morph_precision_bits, round_centre,
};
pub use orbit::{ReferenceOrbitBuilder, escape_f32};
pub use perturb::{perturb_scaled_f64, perturb_scaled_f64_with_envelope};
pub use plane::{SEED_AXES, construct_plane};
pub use scale::{
    centre_displacement_px, centre_from_reference_px, mirror_centre, pixel_scale, precision_for,
    reference_shift_px, scale_split, scaled_pixel_offset, scaled_pixel_scale, shallow_pixel_scale,
    split_centre, split_scalar,
};
pub use types::{
    Axis4, CentreF64, CentreSplit, ComputedOrbit, EscapeGridRecord, EscapeParams, EscapeSample,
    MathError, NavigationDelta, OrbitStep, PerturbSample, PerturbationEnvelope, Plane, PlaneAngles,
    Pose, PrecisionMode, PrecisionPlan, ReferenceOrbitRecord, ScaleSplit, ScaledPixelScale,
    ViewControls, WarpMatrix,
};
pub use warp::{warp_identity_error, warp_matrix};
