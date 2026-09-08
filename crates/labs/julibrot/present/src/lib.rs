//! Pixel presentation records and pure planners for the Julibrot lab.

#![deny(missing_docs)]

mod contract;
mod fence;
mod gpu;
mod homography;
mod lattice;
mod mesh;
mod palette;
mod planner;
mod shade_shader;
mod shader;
mod state;
mod tile;
mod uniform;
mod warp_shader;

pub use contract::{
    DropReason, FenceRefusal, FramePartition, FrameReceipt, FrameState,
    PRESENTATION_LEDGER_CAPACITY, PresentBackdrop, PresentConfig, PresentError, PresentEvent,
    PresentEvents, PresentFacts, PresentHot, PresentMain, PresentStatus, PresentationLedger,
    PresentationLedgerEntry, SampleClass, SceneFrame, SubmissionKind, SubmissionMeasurement,
    WarpKind, WarpPlan, WarpRefusalReason,
};
pub use ember_julibrot_kernels::RefinementLevel;
pub use ember_julibrot_math::{ObjectAngles, Pose, PoseMap, ViewControls};
pub use gpu::{FrameReadback, FrameReadbackRoute, Presenter, frame_readback_route};
pub use homography::{
    apply_homography, inverse_identity_error, pack_homography_rows, solve_homography,
};
pub use lattice::{
    COVERAGE_CHART_POINTS, LatticePair, SOURCE_TEXEL_REACH_PX, compose_homography,
    identity_warp_rows,
};
pub use mesh::{
    HeightSample, MeshError, camera_rotation, camera_rotation_pairs, camera_translation,
    display_coordinate, grid_screen, height_for_record, scene_index_count, scene_indices,
    view_scale,
};
pub use palette::{
    CLASSIC_PALETTE, CLEAR_VALUE, DEBUG_TINT, EMBER_PALETTE, EXPOSED_VALUE, GLITCH_DIAGNOSTIC,
    ICE_PALETTE, PaletteId, PaletteOutcome, PaletteRecord, SKY_VALUE, exterior_zero, palette,
    presentation_value, shade_escape_record, shade_lit_escape_record, shade_presentation_value,
};
pub use planner::{
    RELIEF_REDRAW_MAX_EXPOSED_FRACTION, WARP_MAX_ERROR_PX, Warp, project_scene_point,
    project_scene_record_vertex, project_scene_vertex, project_scene_vertex_exact,
    relief_redraw_source_covers_destination, relief_redraw_source_pose, renders_same_picture,
};
pub use shade_shader::shade_shader;
pub use shader::{glitch_count_shader, scene_shader};
pub use tile::{
    CanonicalChartCellKey, DerivedChartFootprint, DescriptorAbiError, DescriptorCostLedger,
    DescriptorFootprintSample, DescriptorSamplePair, DescriptorTexel, ExactF32, ExactF64,
    PoseMapKey, RenderControlChange, SliceChartTransform, SliceIdentity, SourcePixelRect,
    TileContentKey, TileInvalidation, TilePoseHeader, TileQuality, TileRenderKey, TileResidency,
    TileRung, TransitionPresentation, certify_same_slice, derive_chart_footprint,
    select_same_surface_owner, tile_invalidation, transition_presentation, validate_pose_header,
};
pub use uniform::{
    HOT_PAYLOAD_BYTES, HOT_RING_SLOTS, HotSlot, HotUniform, PresentDataError, SCENE_PAYLOAD_BYTES,
    SceneUniform, hot_ring_bytes, hot_stride,
};
pub use warp_shader::warp_shader;
