//! GPU-kernel contracts and CPU mirrors for the Julibrot lab.

/// Extra decimal digits used by the deterministic reference-orbit verification pass.
pub const DETERMINISTIC_VERIFICATION_DIGITS: u32 = 16;

mod conformance;
mod dialect;
mod error;
mod gpu;
mod perturb;
mod records;
mod refinement;
mod shallow;
mod tile_job;

pub use conformance::{
    ConformanceResult, ConformanceVerdict, PERTURB_SMOOTH_TOLERANCE, SHALLOW_SMOOTH_TOLERANCE,
    VISIBLE_REPLAY_CARDS, VisibleReplayCard, evaluate_perturbation_conformance,
    evaluate_shallow_conformance, record_is_well_formed,
};
pub use dialect::{KERNEL_UNIFORM_BYTES, OUTPUT_PAGE_SIDE, perturbation_kernel, shallow_kernel};
pub use ember_julibrot_math::{
    CentreSplit, EscapeGridRecord, EscapeParams, Plane, ReferenceOrbitRecord, ScaleSplit,
};
pub use error::KernelError;
pub use gpu::JulibrotKernels;
pub use perturb::{perturb_scaled_offset, perturb_scaled_pixel};
pub use records::{
    EscapeGrid, GLITCH_NUMERIC_FAILURE, GLITCH_REFERENCE_EXHAUSTED, GridExtent, KernelMode,
    PerturbUniform, ReferenceOrbitInput, RefinementLevel, SampleStatus, ShallowUniform,
};
pub use refinement::{
    DispatchFacts, LevelSpec, RefinementPlan, dispatch_facts, next_refinement_level,
    plan_refinement,
};
pub use shallow::{KernelSample, escape_shallow_pixel, escape_shallow_point};
pub use tile_job::{
    ContentIdentity, CoverageClass, DEFAULT_TILE_APRON, DEFAULT_TILE_CORE_SIDE,
    DEFAULT_TILE_LOGICAL_BYTES, DEFAULT_TILE_SAMPLE_BYTES, DEFAULT_TILE_SIDE, DemandKey,
    MainIdentity, PairedOutputCompletion, PairedOutputSpanPlan, PublishedTileOutputs,
    ReferenceIdentity, ReferenceLease, ReferenceLeaseSet, ResidentTileCost, ResidentTileProfile,
    SourceScreenRect, StableJobId, TILE_HEADER_BYTES, TILE_SAMPLE_RECORD_BYTES, TileDemandQueue,
    TileGeometry, TileJob, TileJobError, TileOutput, TileQuality, TileResidency,
    resident_tile_cost,
};
