//! Compatibility re-exports for the kernels-owned rendered-tile record vocabulary.

pub use ember_julibrot_kernels::{
    CanonicalChartCellKey, DescriptorAbiError, DescriptorCostLedger, DescriptorSamplePair,
    DescriptorTexel, ExactF32, ExactF64, PoseMapKey, RenderControlChange, SliceIdentity,
    SourceScreenRect as SourcePixelRect, TileContentKey, TileInvalidation, TilePoseHeader,
    TileQuality, TileRenderKey, TileResidency, TileRung, TransitionPresentation,
    select_same_surface_owner, tile_invalidation, transition_presentation, validate_pose_header,
};
