use bytemuck::{Pod, Zeroable};
use thiserror::Error;

use crate::PaletteRecord;

/// Number of payload bytes in one HOT ring slot.
pub const HOT_PAYLOAD_BYTES: u32 = 128;

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
    /// A slot stride could overlap a 128-byte payload.
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
}

/// Computes the dynamic-uniform stride for one 128-byte payload.
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
        Ok(Self {
            grid: [width, height, level, max_iter],
            span: [directory_index, active_len, 0, 0],
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
        assert_eq!(size_of::<HotUniform>(), 128);
        assert_eq!(align_of::<HotUniform>(), 16);
        assert_eq!(offset_of!(HotUniform, camera), 0);
        assert_eq!(offset_of!(HotUniform, view_scale), 16);
        assert_eq!(offset_of!(HotUniform, view_rotation), 32);
        assert_eq!(offset_of!(HotUniform, homography_row_0), 48);
        assert_eq!(offset_of!(HotUniform, homography_row_1), 64);
        assert_eq!(offset_of!(HotUniform, homography_row_2), 80);
        assert_eq!(offset_of!(HotUniform, clear_rgba), 96);
        assert_eq!(offset_of!(HotUniform, flags), 112);
        assert_eq!(size_of::<SceneUniform>(), 80);
        assert_eq!(align_of::<SceneUniform>(), 16);
        assert_eq!(offset_of!(SceneUniform, grid), 0);
        assert_eq!(offset_of!(SceneUniform, span), 16);
        assert_eq!(offset_of!(SceneUniform, palette_map), 32);
        assert_eq!(offset_of!(SceneUniform, interior_rgba), 48);
        assert_eq!(offset_of!(SceneUniform, clear_rgba), 64);
    }

    #[test]
    fn ring_stride_and_slots_are_checked() {
        assert_eq!(hot_stride(256), Ok(256));
        assert_eq!(hot_ring_bytes(256), Ok(768));
        assert_eq!(hot_stride(48), Ok(144));
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
        let uniform = SceneUniform::new([3, 2], 1, 64, 7, 12, CLASSIC_PALETTE)
            .expect("six active records fit twelve-record capacity");
        assert_eq!(uniform.grid, [3, 2, 1, 64]);
        assert_eq!(uniform.span, [7, 6, 0, 0]);
        assert_eq!(
            SceneUniform::new([4, 4], 0, 64, 7, 12, CLASSIC_PALETTE),
            Err(PresentDataError::InvalidGrid {
                width: 4,
                height: 4,
                logical_len: 12,
            })
        );
    }
}
