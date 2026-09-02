//! GPU-kernel contracts and CPU mirrors for the Julibrot lab.

mod dialect;
mod error;
mod records;
mod shallow;

pub use dialect::{OUTPUT_PAGE_SIDE, perturbation_kernel, shallow_kernel};
pub use ember_julibrot_math::{
    CentreSplit, EscapeGridRecord, EscapeParams, Plane, ReferenceOrbitRecord, ScaleSplit,
};
pub use error::KernelError;
pub use records::{
    EscapeGrid, GridExtent, KernelMode, PerturbUniform, ReferenceOrbitInput, RefinementLevel,
    ShallowUniform,
};
pub use shallow::{KernelSample, escape_shallow_pixel, escape_shallow_point};
