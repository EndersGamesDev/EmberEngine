use bytemuck::{Pod, Zeroable};
use ember_julibrot_math::Homography;
use thiserror::Error;

use crate::{PaletteRecord, pack_homography_rows};

/// Number of payload bytes in one HOT ring slot.
pub const HOT_PAYLOAD_BYTES: u32 = 176;

/// Number of bytes in the regional scene payload.
pub const SCENE_PAYLOAD_BYTES: u32 = 128;

/// Number of dynamic-offset slots in the HOT ring.
pub const HOT_RING_SLOTS: u32 = 3;

/// Exact GPU HOT payload consumed by scene and warp shaders.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct HotUniform {
    /// Cosine and sine of the observer yaw, then of its pitch.
    pub camera: [f32; 4],
    /// Height amplitude, both perspective distances, and one reserved zero.
    pub view_scale: [f32; 4],
    /// Cosine and sine for both standing VIEW rotations.
    pub view_rotation: [f32; 4],
    /// First padded row of the inverse-sampling homography.
    pub homography_row_0: [f32; 4],
    /// Second padded row of the inverse-sampling homography.
    pub homography_row_1: [f32; 4],
    /// Third padded row of the inverse-sampling homography.
    pub homography_row_2: [f32; 4],
    /// First padded row of the current screen-to-plane map.
    pub screen_to_plane_row_0: [f32; 4],
    /// Second padded row of the current screen-to-plane map.
    pub screen_to_plane_row_1: [f32; 4],
    /// Third padded row of the current screen-to-plane map.
    pub screen_to_plane_row_2: [f32; 4],
    /// Honest clear and disocclusion colour.
    pub clear_rgba: [f32; 4],
    /// Epoch low/high words, source validity, and one reserved zero.
    pub flags: [u32; 4],
}

/// Exact regional MAIN payload consumed by a scene shader.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct SceneUniform {
    /// Width, height, refinement discriminant, and iteration cap.
    pub grid: [u32; 4],
    /// Span-directory index, logical length, and zero padding.
    pub span: [u32; 4],
    /// First padded row of the map used to sample this grid.
    pub screen_to_plane_row_0: [f32; 4],
    /// Second padded row of the map used to sample this grid.
    pub screen_to_plane_row_1: [f32; 4],
    /// Third padded row of the map used to sample this grid.
    pub screen_to_plane_row_2: [f32; 4],
    /// Palette period, phase, colour mix, and value.
    pub palette_map: [f32; 4],
    /// Exact interior colour.
    pub interior_rgba: [f32; 4],
    /// Exact clear colour.
    pub clear_rgba: [f32; 4],
}

/// Refusal from checked presentation-data construction.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PresentDataError {
    /// A dynamic-uniform alignment was zero.
    #[error("dynamic-uniform alignment is zero")]
    ZeroAlignment,
    /// Checked byte arithmetic exceeded `u32`.
    #[error("presentation byte arithmetic overflowed")]
    ArithmeticOverflow,
    /// A slot stride could overlap a 176-byte payload.
    #[error("HOT slot stride {0} is invalid")]
    InvalidStride(u32),
    /// A grid extent or active prefix was invalid.
    #[error("grid {width}x{height} does not fit logical length {logical_len}")]
    InvalidGrid {
        /// Grid width in pixels.
        width: u32,
        /// Grid height in pixels.
        height: u32,
        /// Addressable span records.
        logical_len: u32,
    },
    /// A screen map coefficient cannot be represented by the GPU payload.
    #[error("the screen-to-plane map cannot be represented as finite f32 rows")]
    InvalidMap,
}

/// Computes the dynamic-uniform stride for one 176-byte payload.
///
/// # Errors
///
/// Returns an error for zero alignment or checked byte-arithmetic overflow.
pub fn hot_stride(alignment: u32) -> Result<u32, PresentDataError> {
    if alignment == 0 {
        return Err(PresentDataError::ZeroAlignment);
    }
    HOT_PAYLOAD_BYTES
        .div_ceil(alignment)
        .checked_mul(alignment)
        .ok_or(PresentDataError::ArithmeticOverflow)
}

/// Computes the exact allocation size of the three-slot HOT ring.
///
/// # Errors
///
/// Returns an error when the stride is invalid or the ring size overflows.
pub fn hot_ring_bytes(alignment: u32) -> Result<u32, PresentDataError> {
    hot_stride(alignment)?
        .checked_mul(HOT_RING_SLOTS)
        .ok_or(PresentDataError::ArithmeticOverflow)
}

/// Opaque checked selector for one HOT dynamic-offset slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotSlot {
    index: u32,
    dynamic_offset: u32,
    epoch: u64,
}

impl HotSlot {
    /// Selects `refresh_id mod 3` using a previously validated stride.
    ///
    /// # Errors
    ///
    /// Returns an error for a short or misaligned stride or checked offset overflow.
    pub fn for_refresh(
        refresh_id: u64,
        slot_stride: u32,
        epoch: u64,
    ) -> Result<Self, PresentDataError> {
        if slot_stride < HOT_PAYLOAD_BYTES || !slot_stride.is_multiple_of(16) {
            return Err(PresentDataError::InvalidStride(slot_stride));
        }
        let index = u32::try_from(refresh_id % u64::from(HOT_RING_SLOTS))
            .map_err(|_| PresentDataError::ArithmeticOverflow)?;
        let dynamic_offset = index
            .checked_mul(slot_stride)
            .ok_or(PresentDataError::ArithmeticOverflow)?;
        Ok(Self {
            index,
            dynamic_offset,
            epoch,
        })
    }

    /// Returns the ring index, always zero through two.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Returns the byte offset supplied as the dynamic uniform offset.
    #[must_use]
    pub const fn dynamic_offset(self) -> u32 {
        self.dynamic_offset
    }

    /// Returns the owner epoch captured for observation attribution.
    #[must_use]
    pub const fn epoch(self) -> u64 {
        self.epoch
    }
}

impl SceneUniform {
    /// Packs one checked scene record without sampling an inactive span suffix.
    ///
    /// # Errors
    ///
    /// Returns an error when the extent is empty, overflows, or exceeds the span length.
    pub fn new(
        extent: [u32; 2],
        level: u32,
        max_iter: u32,
        directory_index: u32,
        logical_len: u32,
        screen_to_plane: &Homography,
        selected: PaletteRecord,
    ) -> Result<Self, PresentDataError> {
        let [width, height] = extent;
        let active_len = width
            .checked_mul(height)
            .filter(|length| *length > 0 && *length <= logical_len)
            .ok_or(PresentDataError::InvalidGrid {
                width,
                height,
                logical_len,
            })?;
        let rows =
            pack_homography_rows(screen_to_plane.rows).ok_or(PresentDataError::InvalidMap)?;
        Ok(Self {
            grid: [width, height, level, max_iter],
            span: [directory_index, active_len, 0, 0],
            screen_to_plane_row_0: rows[0],
            screen_to_plane_row_1: rows[1],
            screen_to_plane_row_2: rows[2],
            palette_map: selected.map,
            interior_rgba: selected.interior_rgba,
            clear_rgba: selected.clear_rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, offset_of, size_of};

    use super::*;
    use crate::CLASSIC_PALETTE;

    #[test]
    fn gpu_layouts_match_the_exact_byte_contract() {
        assert_eq!(size_of::<HotUniform>(), 176);
        assert_eq!(align_of::<HotUniform>(), 16);
        assert_eq!(offset_of!(HotUniform, camera), 0);
        assert_eq!(offset_of!(HotUniform, view_scale), 16);
        assert_eq!(offset_of!(HotUniform, view_rotation), 32);
        assert_eq!(offset_of!(HotUniform, homography_row_0), 48);
        assert_eq!(offset_of!(HotUniform, homography_row_1), 64);
        assert_eq!(offset_of!(HotUniform, homography_row_2), 80);
        assert_eq!(offset_of!(HotUniform, screen_to_plane_row_0), 96);
        assert_eq!(offset_of!(HotUniform, screen_to_plane_row_1), 112);
        assert_eq!(offset_of!(HotUniform, screen_to_plane_row_2), 128);
        assert_eq!(offset_of!(HotUniform, clear_rgba), 144);
        assert_eq!(offset_of!(HotUniform, flags), 160);
        assert_eq!(size_of::<SceneUniform>(), 128);
        assert_eq!(align_of::<SceneUniform>(), 16);
        assert_eq!(offset_of!(SceneUniform, grid), 0);
        assert_eq!(offset_of!(SceneUniform, span), 16);
        assert_eq!(offset_of!(SceneUniform, screen_to_plane_row_0), 32);
        assert_eq!(offset_of!(SceneUniform, screen_to_plane_row_1), 48);
        assert_eq!(offset_of!(SceneUniform, screen_to_plane_row_2), 64);
        assert_eq!(offset_of!(SceneUniform, palette_map), 80);
        assert_eq!(offset_of!(SceneUniform, interior_rgba), 96);
        assert_eq!(offset_of!(SceneUniform, clear_rgba), 112);
    }

    #[test]
    fn the_packed_lanes_carry_the_numbers_the_shader_reads() {
        // This asserts the bytes a shader samples, not a Rust re-statement of the algebra. The
        // browser found d4 reframing a height-zero chart that both the WGSL and the CPU mirror say
        // it cannot touch, and a mirror test cannot see that class of divergence at all.
        use crate::{camera_rotation, view_rotation, view_scale};
        let uniform = HotUniform {
            camera: camera_rotation(0.0, 0.0).expect("neutral observer"),
            view_scale: view_scale(0.0, 8.0, 8.0).expect("neutral distances"),
            view_rotation: view_rotation(0.0, 0.0).expect("neutral VIEW"),
            homography_row_0: [1.0, 0.0, 0.0, 0.0],
            homography_row_1: [0.0, 1.0, 0.0, 0.0],
            homography_row_2: [0.0, 0.0, 1.0, 0.0],
            clear_rgba: [0.0; 4],
            flags: [7, 0, 1, 0],
        };
        let bytes = bytemuck::bytes_of(&uniform);
        assert_eq!(bytes.len(), 128);
        let lane = |offset: usize| -> [f32; 4] {
            core::array::from_fn(|index| {
                let start = offset + index * 4;
                f32::from_le_bytes([
                    bytes[start],
                    bytes[start + 1],
                    bytes[start + 2],
                    bytes[start + 3],
                ])
            })
        };
        // Byte 0 is the observer the vertex reads as camera.x/.y (yaw) and .z/.w (pitch).
        assert_eq!(lane(0), [1.0, 0.0, 1.0, 0.0]);
        // Byte 16 is [h, d5, d4, reserved]: the vertex reads .x as the height amplitude, .y as the
        // five-to-four pole, and .z as both the four-to-three pole and the observer distance.
        assert_eq!(lane(16), [0.0, 8.0, 8.0, 0.0]);
        assert_eq!(lane(32), [1.0, 0.0, 1.0, 0.0]);
        // The retired view discriminant is a reserved zero.
        assert_eq!(&bytes[124..128], &0_u32.to_le_bytes());
        let moved = HotUniform {
            view_scale: view_scale(1.5, 2.0, 40.0).expect("moved distances"),
            ..uniform
        };
        assert_eq!(lane_of(&moved, 16), [1.5, 2.0, 40.0, 0.0]);
    }

    fn lane_of(uniform: &HotUniform, offset: usize) -> [f32; 4] {
        let bytes = bytemuck::bytes_of(uniform);
        core::array::from_fn(|index| {
            let start = offset + index * 4;
            f32::from_le_bytes([
                bytes[start],
                bytes[start + 1],
                bytes[start + 2],
                bytes[start + 3],
            ])
        })
    }

    #[test]
    fn ring_stride_and_slots_are_checked() {
        assert_eq!(hot_stride(256), Ok(256));
        assert_eq!(hot_ring_bytes(256), Ok(768));
        assert_eq!(hot_stride(48), Ok(192));
        assert_eq!(hot_stride(0), Err(PresentDataError::ZeroAlignment));
        let slot = HotSlot::for_refresh(8, 256, 19).expect("valid slot");
        assert_eq!(
            (slot.index(), slot.dynamic_offset(), slot.epoch()),
            (2, 512, 19)
        );
        assert_eq!(
            HotSlot::for_refresh(0, 112, 0),
            Err(PresentDataError::InvalidStride(112))
        );
    }

    #[test]
    fn scene_uniform_keeps_final_capacity_and_rejects_bad_prefixes() {
        let uniform =
            SceneUniform::new([3, 2], 1, 64, 7, 12, &Homography::IDENTITY, CLASSIC_PALETTE)
                .expect("six active records fit twelve-record capacity");
        assert_eq!(uniform.grid, [3, 2, 1, 64]);
        assert_eq!(uniform.span, [7, 6, 0, 0]);
        assert_eq!(
            SceneUniform::new([4, 4], 0, 64, 7, 12, &Homography::IDENTITY, CLASSIC_PALETTE,),
            Err(PresentDataError::InvalidGrid {
                width: 4,
                height: 4,
                logical_len: 12,
            })
        );
    }
}
