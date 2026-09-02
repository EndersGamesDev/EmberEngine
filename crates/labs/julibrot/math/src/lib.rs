//! CPU truth and numeric interfaces for the Julibrot lab.

mod big;
mod types;

pub use big::{BigCentre, BigScalar, EncodedBigScalar, decode_big_scalar, encode_big_scalar};
pub use types::{
    Axis4, CentreF64, CentreSplit, ComputedOrbit, EscapeGridRecord, EscapeParams, EscapeSample,
    MathError, OrbitStep, PerturbSample, Plane, PlaneAngles, PlanePreset, PlaneSpec, Pose,
    PrecisionPlan, ReferenceOrbitRecord, ScaleSplit, ScaledPixelScale, ViewMode, WarpMatrix,
};
