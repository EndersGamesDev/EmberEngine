//! Pixel presentation records and pure planners for the Julibrot lab.

#![deny(missing_docs)]

mod contract;
mod fence;
mod gpu;
mod homography;
mod mesh;
mod palette;
mod planner;
mod shader;
mod state;
mod tile;
mod uniform;
mod warp_shader;

pub use contract::{
    DropReason, FenceRefusal, FramePartition, FrameReceipt, FrameState, PresentBackdrop,
    PresentConfig, PresentError, PresentEvent, PresentEvents, PresentFacts, PresentHot, PresentMain,
    PresentStatus, SampleClass, SceneFrame, SubmissionKind, SubmissionMeasurement, WarpKind,
    WarpPlan, WarpValidation,
};
pub use ember_julibrot_kernels::RefinementLevel;
pub use ember_julibrot_math::{ObjectAngles, Pose, PoseMap, ViewControls};
pub use gpu::Presenter;
pub use homography::{
    apply_homography, inverse_identity_error, pack_homography_rows, solve_homography,
};
pub use mesh::{
    HeightSample, MeshError, camera_rotation, camera_rotation_pairs, camera_translation,
    display_coordinate, grid_screen, height_for_record, scene_index_count, scene_indices,
    view_scale,
};
pub use palette::{
    CLASSIC_PALETTE, DEBUG_TINT, EMBER_PALETTE, GLITCH_DIAGNOSTIC, ICE_PALETTE, PaletteId,
    PaletteOutcome, PaletteRecord, exterior_zero, palette, shade_escape_record,
    shade_lit_escape_record,
};
pub use planner::{
    WARP_MAX_ERROR_PX, Warp, project_scene_point, project_scene_vertex, renders_same_picture,
};
pub use shader::{glitch_count_shader, scene_shader};
pub use tile::{
    CanonicalChartCellKey, DescriptorAbiError, DescriptorCostLedger, DescriptorSamplePair,
    DescriptorTexel, ExactF32, ExactF64, PoseMapKey, RenderControlChange, SliceIdentity,
    SourcePixelRect, TileContentKey, TileInvalidation, TilePoseHeader, TileQuality, TileRenderKey,
    TileResidency, TileRung, TransitionPresentation, select_same_surface_owner, tile_invalidation,
    transition_presentation, validate_pose_header,
};
pub use uniform::{
    HOT_PAYLOAD_BYTES, HOT_RING_SLOTS, HotSlot, HotUniform, PresentDataError, SCENE_PAYLOAD_BYTES,
    SceneUniform, hot_ring_bytes, hot_stride,
};
pub use warp_shader::warp_shader;
