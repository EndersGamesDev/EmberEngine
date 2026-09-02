//! GPU-kernel contracts and CPU mirrors for the Julibrot lab.

mod dialect;
mod error;
mod perturb;
mod records;
mod refinement;
mod shallow;

pub use dialect::{OUTPUT_PAGE_SIDE, perturbation_kernel, shallow_kernel};
pub use ember_julibrot_math::{
    CentreSplit, EscapeGridRecord, EscapeParams, Plane, ReferenceOrbitRecord, ScaleSplit,
};
pub use error::KernelError;
pub use perturb::{perturb_scaled_offset, perturb_scaled_pixel};
pub use records::{
    EscapeGrid, GridExtent, KernelMode, PerturbUniform, ReferenceOrbitInput, RefinementLevel,
    ShallowUniform,
};
pub use refinement::{DispatchFacts, LevelSpec, RefinementPlan, dispatch_facts, plan_refinement};
pub use shallow::{KernelSample, escape_shallow_pixel, escape_shallow_point};
