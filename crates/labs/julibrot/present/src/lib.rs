//! Pixel presentation records and pure planners for the Julibrot lab.

#![deny(missing_docs)]

mod homography;
mod mesh;
mod palette;
mod shader;
mod uniform;

pub use homography::{
    apply_homography, inverse_identity_error, pack_homography_rows, solve_homography,
};
pub use mesh::{
    HeightSample, MeshError, display_coordinate, height_for_record, tumbled_index_count,
    tumbled_indices, view_rotation,
};
pub use palette::{
    CLASSIC_PALETTE, EMBER_PALETTE, ICE_PALETTE, PaletteId, PaletteOutcome, PaletteRecord, palette,
    shade_escape_record,
};
pub use shader::{ShaderSources, scene_shaders};
pub use uniform::{
    HOT_PAYLOAD_BYTES, HOT_RING_SLOTS, HotSlot, HotUniform, PresentDataError, SceneUniform,
    hot_ring_bytes, hot_stride,
};
