//! GPU-kernel contracts and CPU mirrors for the Julibrot lab.

mod dialect;
mod records;

pub use dialect::{OUTPUT_PAGE_SIDE, perturbation_kernel, shallow_kernel};
pub use ember_julibrot_math::{
    CentreSplit, EscapeGridRecord, EscapeParams, Plane, ReferenceOrbitRecord, ScaleSplit,
};
pub use records::{
    EscapeGrid, GridExtent, KernelMode, PerturbUniform, ReferenceOrbitInput, RefinementLevel,
    ShallowUniform,
};
