//! CPU truth and numeric interfaces for the Julibrot lab.

mod big;
mod drift;
mod footprint;
mod morph;
mod navigation;
mod orbit;
mod perturb;
mod plane;
mod scale;
mod screen;
mod types;
mod warp;

pub use big::{
    BigCentre, BigScalar, DETERMINISTIC_CENTRE_BITS, EncodedBigScalar, PICTURE_FAST_EDIT_BUDGET,
    centre_precision_for, decode_big_scalar, encode_big_scalar,
};
pub use drift::{navigation_drift_f32, navigation_drift_f64};
pub use footprint::{SceneFootprint, scene_footprint};
pub use morph::{
    MORPH_EXTRA_BITS, lerp_centre, lerp_f64, lerp_object_angles, lerp_origin, lerp_plane_angles,
    lerp_view, morph_precision_bits, round_centre,
};
pub use orbit::{ReferenceOrbitBuilder, escape_f32};
pub use perturb::{perturb_scaled_f64, perturb_scaled_f64_with_envelope};
pub use plane::{SEED_AXES, construct_plane, rotation_orthonormality_4};
pub use scale::{
    centre_displacement_px, centre_from_reference_px, mirror_centre, pixel_scale, precision_for,
    reference_shift_px, scale_split, scaled_pixel_offset, scaled_pixel_scale, shallow_pixel_scale,
    split_centre, split_scalar,
};
pub use screen::{navigation_delta, plane_to_screen, rotation_orthonormality_5, screen_to_plane};
pub use types::{
    Axis4, CentreF64, CentreSplit, ComputedOrbit, EscapeGridRecord, EscapeParams, EscapeSample,
    Homography, MathError, NavigationDelta, ObjectAngles, OrbitStep, PerturbSample,
    PerturbationEnvelope, Plane, PlaneAngles, Pose, PoseMap, PrecisionMode, PrecisionPlan,
    ReferenceOrbitRecord, ReferencePass, ReferenceVerification, ScaleSplit, ScaledPixelScale,
    ViewControls, WarpMatrix,
};
pub use warp::{warp_identity_error, warp_matrix};
