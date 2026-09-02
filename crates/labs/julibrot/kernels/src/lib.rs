//! GPU-kernel contracts and CPU mirrors for the Julibrot lab.

mod conformance;
mod dialect;
mod error;
mod gpu;
mod perturb;
mod records;
mod refinement;
mod shallow;

pub use conformance::{
    ConformanceResult, ConformanceVerdict, PERTURB_SMOOTH_TOLERANCE,
    SHALLOW_SMOOTH_TOLERANCE, VISIBLE_REPLAY_CARDS, VisibleReplayCard,
    evaluate_perturbation_conformance, evaluate_shallow_conformance, record_is_well_formed,
};
pub use dialect::{OUTPUT_PAGE_SIDE, perturbation_kernel, shallow_kernel};
pub use ember_julibrot_math::{
    CentreSplit, EscapeGridRecord, EscapeParams, Plane, ReferenceOrbitRecord, ScaleSplit,
};
pub use error::KernelError;
pub use gpu::JulibrotKernels;
pub use perturb::{perturb_scaled_offset, perturb_scaled_pixel};
pub use records::{
    EscapeGrid, GridExtent, KernelMode, PerturbUniform, ReferenceOrbitInput, RefinementLevel,
    ShallowUniform,
};
pub use refinement::{DispatchFacts, LevelSpec, RefinementPlan, dispatch_facts, plan_refinement};
pub use shallow::{KernelSample, escape_shallow_pixel, escape_shallow_point};
