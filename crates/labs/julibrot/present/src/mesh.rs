use thiserror::Error;

use crate::palette::DEBUG_TINT;
use crate::{PaletteRecord, shade_escape_record};

/// Refusal from checked scene-grid construction.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum MeshError {
    /// Width, height, or iteration cap was zero, or a pixel was outside the extent.
    #[error("the scene grid extent, pixel, or iteration cap is invalid")]
    InvalidInput,
    /// The exact index count cannot be represented by the renderer's u32 contract.
    #[error("the scene index count overflowed u32")]
    IndexCountOverflow,
    /// Reserving the exact index payload failed.
    #[error("the scene index allocation failed")]
    Allocation,
    /// A control or derived coefficient was not finite, or a control left its range.
    #[error("a VIEW control is not finite or is outside its range")]
    NonFiniteRotation,
}

/// CPU mirror of the height and debug decision made by the scene shader.
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
pub fn scene_index_count(extent: [u32; 2]) -> Result<u32, MeshError> {
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
pub fn scene_indices(extent: [u32; 2]) -> Result<Vec<u32>, MeshError> {
    let [width, height] = extent;
    let count = scene_index_count(extent)?;
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

/// Returns one mesh vertex's screen offset from the frame centre along one axis.
///
/// A grid of `count` samples covers `count` pixels, so the mesh spanning their centres stops half a
/// pixel short of the frame at both ends. The rasterizer's fill rule hides that at the low end and
/// exposes it at the high end, leaving the last column and row painted with the scene pass's clear
/// rather than from a record. The outermost samples are therefore placed on the frame boundary, so
/// the mesh tiles the frame it was sampled for; interior samples keep their pixel centre exactly.
///
/// This is the drawn rule, not the sampling rule: which plane point a record carries is still the
/// pixel centre that `ember_julibrot_math` chose, and the fragment stage still resolves every pixel
/// to its own record.
#[must_use]
pub fn grid_screen(index: u32, count: u32) -> f64 {
    let centre = 0.5 * f64::from(count);
    if count <= 1 {
        return 0.0;
    }
    if index == 0 {
        return -centre;
    }
    if index + 1 == count {
        return centre;
    }
    f64::from(index) + 0.5 - centre
}

/// Computes display-normalized `(q_u,q_v)` at one bottom-row-first mesh vertex.
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
    let span = f64::from(width);
    Ok([
        (4.0 * grid_screen(column, width) / span) as f32,
        (4.0 * grid_screen(row, height) / span) as f32,
    ])
}

/// Applies the exact record height and honest-debug decision to an escape record.
///
/// The returned height is the record's own `H` in `[-2,2]`; presentation maps that domain from the
/// chart floor zero through the former positive peak, without changing this classification.
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
    let status = record[3];
    let debug_tint = palette.rgba == DEBUG_TINT && palette.contract_violation;
    let height = if debug_tint || status == 1.0 || status == 2.0 {
        0.0
    } else if record[1] == 0.0 {
        -2.0
    } else {
        (record[0] / max_iter as f32)
            .clamp(0.0, 1.0)
            .mul_add(4.0, -2.0)
    };
    Ok(HeightSample {
        height,
        debug_tint,
        contract_violation: palette.contract_violation,
    })
}

/// Returns five `[cos a,sin a,cos b,sin b]` lanes for the ten ambient camera factors.
///
/// # Errors
///
/// Returns an error when an angle or any narrowed coefficient is not finite.
pub fn camera_rotation_pairs(camera: [f64; 10]) -> Result<[[f32; 4]; 5], MeshError> {
    Ok([
        pair_rotation(camera[0], camera[1])?,
        pair_rotation(camera[2], camera[3])?,
        pair_rotation(camera[4], camera[5])?,
        pair_rotation(camera[6], camera[7])?,
        pair_rotation(camera[8], camera[9])?,
    ])
}

/// Returns two padded lanes for the five-dimensional camera translation.
///
/// # Errors
///
/// Returns an error when a coordinate or its narrowed value is not finite.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the HOT contract deliberately narrows validated camera controls once"
)]
pub fn camera_translation(translation: [f64; 5]) -> Result<[[f32; 4]; 2], MeshError> {
    if !translation.into_iter().all(f64::is_finite) {
        return Err(MeshError::NonFiniteRotation);
    }
    let lanes = [
        [
            translation[0] as f32,
            translation[1] as f32,
            translation[2] as f32,
            translation[3] as f32,
        ],
        [translation[4] as f32, 0.0, 0.0, 0.0],
    ];
    lanes
        .into_iter()
        .flatten()
        .all(f32::is_finite)
        .then_some(lanes)
        .ok_or(MeshError::NonFiniteRotation)
}

/// Returns `[cos yaw,sin yaw,cos pitch,sin pitch]` for the observer orientation.
///
/// # Errors
///
/// Returns an error when either angle or any narrowed coefficient is not finite.
pub fn camera_rotation(yaw: f64, pitch: f64) -> Result<[f32; 4], MeshError> {
    pair_rotation(yaw, pitch)
}

/// Returns the `[h,d₅,d₄,0]` scale lane for one refresh.
///
/// # Errors
///
/// Returns an error for a non-finite value, a negative height, or a non-positive distance.
#[allow(clippy::cast_possible_truncation)]
pub fn view_scale(
    height_scale: f64,
    distance_five: f64,
    distance_four: f64,
) -> Result<[f32; 4], MeshError> {
    let values = [height_scale, distance_five, distance_four];
    if !values.iter().all(|value| value.is_finite())
        || height_scale < 0.0
        || distance_five <= 0.0
        || distance_four <= 0.0
    {
        return Err(MeshError::NonFiniteRotation);
    }
    let lane = [
        height_scale as f32,
        distance_five as f32,
        distance_four as f32,
        0.0,
    ];
    (lane[1] > 0.0 && lane[2] > 0.0 && lane.iter().all(|value| value.is_finite()))
        .then_some(lane)
        .ok_or(MeshError::NonFiniteRotation)
}

#[allow(clippy::cast_possible_truncation)]
fn pair_rotation(first: f64, second: f64) -> Result<[f32; 4], MeshError> {
    if !first.is_finite() || !second.is_finite() {
        return Err(MeshError::NonFiniteRotation);
    }
    let (sine_one, cosine_one) = first.sin_cos();
    let (sine_two, cosine_two) = second.sin_cos();
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
    use super::*;
    use crate::CLASSIC_PALETTE;

    #[test]
    fn mesh_counts_and_cell_order_are_exact() -> Result<(), MeshError> {
        assert_eq!(scene_index_count([3, 2])?, 12);
        assert_eq!(scene_indices([3, 2])?, [0, 1, 3, 1, 4, 3, 1, 2, 4, 2, 5, 4]);
        assert_eq!(scene_index_count([1, 1])?, 0);
        assert_eq!(
            scene_index_count([u32::MAX, u32::MAX]),
            Err(MeshError::IndexCountOverflow)
        );
        Ok(())
    }

    #[test]
    fn the_outer_ring_is_the_frame_and_interiors_keep_their_centre() -> Result<(), MeshError> {
        // Every sample of a two-wide axis is an outer one, so both land on the frame boundary.
        assert_eq!(display_coordinate([2, 2], [0, 0])?, [-2.0, -2.0]);
        assert_eq!(display_coordinate([2, 2], [1, 1])?, [2.0, 2.0]);
        // The display box is square in q_u, so a 4-by-2 frame is +/-2 across and +/-1 tall.
        assert_eq!(display_coordinate([4, 2], [0, 0])?, [-2.0, -1.0]);
        assert_eq!(display_coordinate([4, 2], [3, 1])?, [2.0, 1.0]);
        // Interior samples are untouched: column 1 of 4 keeps its pixel centre at screen -0.5.
        assert_eq!(display_coordinate([4, 4], [1, 2])?, [-0.5, 0.5]);
        assert_eq!(grid_screen(1, 3), 0.0);
        assert_eq!(grid_screen(0, 1), 0.0);
        Ok(())
    }

    /// The whole point of the outer ring: the drawn mesh has to cover the frame, and every pixel
    /// has to resolve to its own record. A pixel-centred mesh leaves the last column and row
    /// uncovered, and the scene pass paints those with its clear -- a colour no record carries.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the asserted grid coordinate is a rounded index inside the fixture extent"
    )]
    #[test]
    fn the_mesh_covers_every_pixel_and_each_reads_its_own_record() -> Result<(), MeshError> {
        let extent = [4_u32, 3_u32];
        let [width, height] = extent;
        // Vertex positions in pixel units, mirroring the shader's `grid_screen` placement.
        let vertex = |index: u32| -> [f64; 2] {
            [
                0.5_f64.mul_add(f64::from(width), grid_screen(index % width, width)),
                0.5_f64.mul_add(f64::from(height), grid_screen(index / width, height)),
            ]
        };
        let indices = scene_indices(extent)?;
        for row in 0..height {
            for column in 0..width {
                let centre = [f64::from(column) + 0.5, f64::from(row) + 0.5];
                let mut resolved = None;
                for triangle in indices.as_chunks::<3>().0 {
                    let corners = [
                        vertex(triangle[0]),
                        vertex(triangle[1]),
                        vertex(triangle[2]),
                    ];
                    let Some(weights) = barycentric(centre, corners) else {
                        continue;
                    };
                    let grid: [f64; 2] = core::array::from_fn(|axis| {
                        weights.iter().zip(triangle.iter().copied()).fold(
                            0.0,
                            |sum, (weight, index)| {
                                let coordinate = if axis == 0 {
                                    index % width
                                } else {
                                    index / width
                                };
                                weight.mul_add(f64::from(coordinate), sum)
                            },
                        )
                    });
                    resolved = Some(grid.map(|value| value.round() as u32));
                    break;
                }
                assert_eq!(
                    resolved,
                    Some([column, row]),
                    "pixel ({column},{row}) must be covered and must read its own record"
                );
            }
        }
        Ok(())
    }

    /// Returns the barycentric weights of `point` in `triangle`, or `None` when it is outside.
    fn barycentric(point: [f64; 2], triangle: [[f64; 2]; 3]) -> Option<[f64; 3]> {
        let [a, b, c] = triangle;
        let cross = |u: [f64; 2], v: [f64; 2]| u[0].mul_add(v[1], -(u[1] * v[0]));
        let edge = |from: [f64; 2], to: [f64; 2]| [to[0] - from[0], to[1] - from[1]];
        let denominator = cross(edge(a, b), edge(a, c));
        if denominator == 0.0 {
            return None;
        }
        let weights = [
            cross(edge(point, b), edge(point, c)) / denominator,
            cross(edge(point, c), edge(point, a)) / denominator,
            cross(edge(point, a), edge(point, b)) / denominator,
        ];
        weights
            .iter()
            .all(|weight| *weight >= -1.0e-12)
            .then_some(weights)
    }

    #[test]
    fn height_keeps_interior_glitch_and_escape_semantics_distinct() -> Result<(), MeshError> {
        assert_eq!(
            height_for_record([-1.0, 0.0, 4.0, 0.0], 64, CLASSIC_PALETTE)?.height,
            -2.0
        );
        let glitch = height_for_record([20.0, 1.0, 4.0, 1.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(glitch.height, 0.0);
        assert!(!glitch.debug_tint);
        assert!(!glitch.contract_violation);
        let escaped = height_for_record([32.0, 1.0, 0.0, 0.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(escaped.height, 0.0);
        assert!(!escaped.debug_tint);
        let horizon = height_for_record([0.0, 0.0, 0.0, 2.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(horizon.height, 0.0);
        let uncertain = height_for_record([32.0, 1.0, 0.0, 3.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(uncertain.height, 0.0);
        let uncertain_interior = height_for_record([-1.0, 0.0, 0.0, 3.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(uncertain_interior.height, -2.0);
        // A beyond-bailout escape carries a negative smooth count. It is an ordinary exterior
        // sample: the clamp puts it on the floor, and the shader's `record_height` agrees.
        let beyond_bailout = height_for_record([-1.112_397, 1.0, 0.0, 0.0], 64, CLASSIC_PALETTE)?;
        assert_eq!(beyond_bailout.height, -2.0);
        assert!(!beyond_bailout.debug_tint);
        assert!(!beyond_bailout.contract_violation);
        Ok(())
    }

    #[test]
    fn controls_are_checked_before_they_reach_a_lane() -> Result<(), MeshError> {
        assert_eq!(camera_rotation_pairs([0.0; 10])?, [[1.0, 0.0, 1.0, 0.0]; 5]);
        assert_eq!(camera_rotation(0.0, 0.0)?, [1.0, 0.0, 1.0, 0.0]);
        assert_eq!(view_scale(0.0, 8.0, 8.0)?, [0.0, 8.0, 8.0, 0.0]);
        assert_eq!(view_scale(1.0, 2.0, 64.0)?, [1.0, 2.0, 64.0, 0.0]);
        assert_eq!(
            view_scale(-0.001, 8.0, 8.0),
            Err(MeshError::NonFiniteRotation)
        );
        assert_eq!(view_scale(1.0, 0.0, 8.0), Err(MeshError::NonFiniteRotation));
        assert_eq!(
            camera_rotation_pairs([f64::NAN; 10]),
            Err(MeshError::NonFiniteRotation)
        );
        assert_eq!(
            camera_rotation(0.0, f64::INFINITY),
            Err(MeshError::NonFiniteRotation)
        );
        Ok(())
    }
}
