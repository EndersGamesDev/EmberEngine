use thiserror::Error;

use crate::palette::DEBUG_TINT;
use crate::{PaletteRecord, shade_escape_record};

/// Refusal from checked tumbled-grid construction.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum MeshError {
    /// Width, height, or iteration cap was zero, or a pixel was outside the extent.
    #[error("the tumbled grid extent, pixel, or iteration cap is invalid")]
    InvalidInput,
    /// The exact index count cannot be represented by the renderer's u32 contract.
    #[error("the tumbled index count overflowed u32")]
    IndexCountOverflow,
    /// Reserving the exact index payload failed.
    #[error("the tumbled index allocation failed")]
    Allocation,
    /// A time or derived VIEW coefficient was not finite.
    #[error("the VIEW rotation is not finite")]
    NonFiniteRotation,
}

/// CPU mirror of the height and debug decision made by the tumbled shaders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeightSample {
    /// Display height in the fifth coordinate.
    pub height: f32,
    /// Whether the shader must use the fixed debug tint.
    pub debug_tint: bool,
    /// Whether the record violated the four-channel contract.
    pub contract_violation: bool,
}

/// Returns the exact u32 index count `6 * (width - 1) * (height - 1)`.
///
/// # Errors
///
/// Returns an error for a zero extent or when the count exceeds u32.
pub fn tumbled_index_count(extent: [u32; 2]) -> Result<u32, MeshError> {
    let [width, height] = extent;
    if width == 0 || height == 0 {
        return Err(MeshError::InvalidInput);
    }
    width
        .saturating_sub(1)
        .checked_mul(height.saturating_sub(1))
        .and_then(|cells| cells.checked_mul(6))
        .ok_or(MeshError::IndexCountOverflow)
}

/// Builds `[a,b,c,b,d,c]` for every grid cell in bottom-row-first order.
///
/// # Errors
///
/// Returns a typed extent, count, or allocation failure before writing any index.
pub fn tumbled_indices(extent: [u32; 2]) -> Result<Vec<u32>, MeshError> {
    let [width, height] = extent;
    let count = tumbled_index_count(extent)?;
    width
        .checked_mul(height)
        .ok_or(MeshError::IndexCountOverflow)?;
    let capacity = usize::try_from(count).map_err(|_| MeshError::IndexCountOverflow)?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(capacity)
        .map_err(|_| MeshError::Allocation)?;
    for row in 0..height.saturating_sub(1) {
        for column in 0..width.saturating_sub(1) {
            let a = row * width + column;
            let b = a + 1;
            let c = a + width;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    debug_assert_eq!(indices.len(), capacity);
    Ok(indices)
}

/// Computes display-normalized `(q_u,q_v)` at one bottom-row-first pixel centre.
///
/// # Errors
///
/// Returns an error for a zero extent or a pixel outside the extent.
#[allow(clippy::cast_possible_truncation)]
pub fn display_coordinate(extent: [u32; 2], pixel: [u32; 2]) -> Result<[f32; 2], MeshError> {
    let [width, height] = extent;
    let [column, row] = pixel;
    if width == 0 || height == 0 || column >= width || row >= height {
        return Err(MeshError::InvalidInput);
    }
    let width = f64::from(width);
    let height = f64::from(height);
    Ok([
        (4.0 * ((f64::from(column) + 0.5) / width - 0.5)) as f32,
        (4.0 * height.mul_add(-0.5, f64::from(row) + 0.5) / width) as f32,
    ])
}

/// Applies the exact tumbled height and honest-debug decision to an escape record.
///
/// # Errors
///
/// Returns an error when the delivered iteration cap is zero.
#[allow(clippy::cast_precision_loss, clippy::float_cmp)]
pub fn height_for_record(
    record: [f32; 4],
    max_iter: u32,
    selected: PaletteRecord,
) -> Result<HeightSample, MeshError> {
    if max_iter == 0 {
        return Err(MeshError::InvalidInput);
    }
    let palette = shade_escape_record(record, selected);
    let glitch = record[3] == 1.0;
    let debug_tint = palette.rgba == DEBUG_TINT && (palette.contract_violation || glitch);
    let height = if debug_tint {
        0.0
    } else if record[1] == 0.0 {
        -2.0
    } else {
        (record[0] / max_iter as f32).clamp(0.0, 1.0).mul_add(4.0, -2.0)
    };
    Ok(HeightSample {
        height,
        debug_tint,
        contract_violation: palette.contract_violation,
    })
}

/// Returns `[cos(theta),sin(theta),cos(phi*theta),sin(phi*theta)]` for `theta=0.4t`.
///
/// # Errors
///
/// Returns an error when time or any narrowed coefficient is not finite.
#[allow(clippy::cast_possible_truncation)]
pub fn view_rotation(time_seconds: f64) -> Result<[f32; 4], MeshError> {
    if !time_seconds.is_finite() {
        return Err(MeshError::NonFiniteRotation);
    }
    let theta_one = 0.4 * time_seconds;
    let theta_two = f64::midpoint(1.0, 5.0_f64.sqrt()) * theta_one;
    let (sine_one, cosine_one) = theta_one.sin_cos();
    let (sine_two, cosine_two) = theta_two.sin_cos();
    let coefficients = [
        cosine_one as f32,
        sine_one as f32,
        cosine_two as f32,
        sine_two as f32,
    ];
    coefficients
        .iter()
        .all(|value| value.is_finite())
        .then_some(coefficients)
        .ok_or(MeshError::NonFiniteRotation)
}

#[cfg(test)]
mod tests {
    use ember_lab_heap::{FrameUniform, mode_a_endpoint};

    use super::*;
    use crate::CLASSIC_PALETTE;

    #[test]
    fn mesh_counts_and_cell_order_are_exact() -> Result<(), MeshError> {
        assert_eq!(tumbled_index_count([3, 2])?, 12);
        assert_eq!(
            tumbled_indices([3, 2])?,
            [0, 1, 3, 1, 4, 3, 1, 2, 4, 2, 5, 4]
        );
        assert_eq!(tumbled_index_count([1, 1])?, 0);
        assert_eq!(
            tumbled_index_count([u32::MAX, u32::MAX]),
            Err(MeshError::IndexCountOverflow)
        );
        Ok(())
    }

    #[test]
    fn coordinates_are_pixel_centred_square_and_bottom_first() -> Result<(), MeshError> {
        assert_eq!(display_coordinate([2, 2], [0, 0])?, [-1.0, -1.0]);
        assert_eq!(display_coordinate([2, 2], [1, 1])?, [1.0, 1.0]);
        assert_eq!(display_coordinate([4, 2], [0, 0])?, [-1.5, -0.5]);
        assert_eq!(display_coordinate([4, 2], [0, 1])?, [-1.5, 0.5]);
        Ok(())
    }

    #[test]
    fn height_keeps_interior_glitch_and_escape_semantics_distinct() -> Result<(), MeshError> {
        assert_eq!(
            height_for_record([-1.0, 0.0, 4.0, 0.0], 64, CLASSIC_PALETTE)?.height,
            -2.0
        );
        let glitch = height_for_record([20.0, 1.0, 4.0, 1.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(glitch.height, 0.0);
        assert!(glitch.debug_tint);
        assert!(!glitch.contract_violation);
        let escaped = height_for_record([32.0, 1.0, 0.0, 0.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(escaped.height, 0.0);
        assert!(!escaped.debug_tint);
        Ok(())
    }

    #[test]
    fn view_coefficients_feed_the_exported_heap_algebra_oracle() -> Result<(), MeshError> {
        let rotation = view_rotation(0.0)?;
        let frame = FrameUniform {
            rotation,
            projection_spacing: [8.0, 8.0, 0.0, 1.0e-4],
            render: [0.0; 4],
            axes_four: [0.0; 4],
            axis_fifth_range: [0.0; 4],
            basis_four: [[0.0; 4]; 5],
            basis_fifth: [[0.0; 4]; 2],
        };
        let endpoint = mode_a_endpoint([1.0, 2.0, 3.0, 4.0, 0.0], [0; 5], &frame);
        assert_eq!(endpoint.point, [2.0, 4.0, 6.0]);
        assert_eq!(endpoint.fifth, 0.0);
        assert!(endpoint.valid);
        let pole = mode_a_endpoint([0.0, 0.0, 0.0, 0.0, 8.0], [0; 5], &frame);
        assert!(!pole.valid);
        Ok(())
    }
}
