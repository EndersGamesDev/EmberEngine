use crate::{Homography, MathError, ViewControls};

const PIVOT_EPSILON: f64 = 1.0e-12;
const MAP_ERROR_LIMIT_PX: f64 = 0.25;

/// Builds the neutral-height inverse view from centred screen pixels to plane-offset pixels.
///
/// `aspect` must be the binary64 quotient `grid_w / grid_h`. The returned rows map screen to
/// plane, the inverse rows map plane to screen, and the condition number uses the infinity norm.
///
/// # Errors
///
/// Returns an error for invalid controls, zoom, extent, aspect, or a forward map whose partial
/// pivot falls below `1e-12`.
pub fn screen_to_plane(
    view: &ViewControls,
    zoom_log2: f64,
    grid_w: u32,
    grid_h: u32,
    aspect: f64,
) -> Result<Homography, MathError> {
    if !view.is_valid() {
        return Err(MathError::InvalidViewControls);
    }
    if grid_w == 0 || grid_h == 0 {
        return Err(MathError::InvalidExtent);
    }
    if !zoom_log2.is_finite() || !aspect.is_finite() || aspect <= 0.0 {
        return Err(MathError::NonFinite);
    }
    if aspect.to_bits() != (f64::from(grid_w) / f64::from(grid_h)).to_bits() {
        return Err(MathError::DegenerateViewMap);
    }

    if view.theta_1 == 0.0
        && view.theta_2 == 0.0
        && view.camera_yaw == 0.0
        && view.camera_pitch == 0.0
    {
        return Ok(Homography::IDENTITY);
    }

    let (view_sine, view_cosine) = view.theta_1.sin_cos();
    let (yaw_sine, yaw_cosine) = view.camera_yaw.sin_cos();
    let (pitch_sine, pitch_cosine) = view.camera_pitch.sin_cos();
    let inverse_camera_distance = 1.0 / view.distance_four;
    let denominator_scale = 4.0 * inverse_camera_distance / f64::from(grid_w);

    // These are the generated scene WGSL camera equations at h=0, with the common homogeneous
    // factor d4 removed. The five- and four-dimensional perspective denominators are one on this
    // two-flat; theta_2 and d5 therefore cancel before this matrix is formed.
    let forward = [
        yaw_cosine * view_cosine,
        -yaw_cosine * view_sine,
        0.0,
        pitch_sine.mul_add(yaw_sine * view_cosine, pitch_cosine * view_sine),
        (-pitch_sine).mul_add(yaw_sine * view_sine, pitch_cosine * view_cosine),
        0.0,
        denominator_scale * pitch_cosine.mul_add(yaw_sine * view_cosine, -pitch_sine * view_sine),
        denominator_scale
            * (-pitch_cosine).mul_add(yaw_sine * view_sine, -pitch_sine * view_cosine),
        1.0,
    ];
    if !forward.iter().all(|value| value.is_finite()) {
        return Err(MathError::NonFinite);
    }
    let rows = invert_3x3(forward).ok_or(MathError::DegenerateViewMap)?;
    if rows[8] <= 0.0 || !rows.iter().all(|value| value.is_finite()) {
        return Err(MathError::DegenerateViewMap);
    }
    let condition_number = norm_infinity(forward) * norm_infinity(rows);
    if !condition_number.is_finite() {
        return Err(MathError::DegenerateViewMap);
    }
    Ok(Homography {
        rows,
        inverse: forward,
        condition_number,
    })
}

/// Converts DOM drag and wheel-anchor inputs into local plane-offset pixels.
///
/// # Errors
///
/// Returns an error for non-finite input, a point on or beyond the horizon, or a homogeneous
/// evaluation whose conservative binary64 error exceeds one quarter pixel.
pub fn navigation_delta(
    screen_to_plane: &Homography,
    drag_delta_px_down: [f64; 2],
    zoom_delta_log2: f64,
    anchor_px_up: [f64; 2],
) -> Result<crate::NavigationDelta, MathError> {
    if !zoom_delta_log2.is_finite()
        || !drag_delta_px_down
            .into_iter()
            .chain(anchor_px_up)
            .all(f64::is_finite)
    {
        return Err(MathError::NonFinite);
    }
    let anchor = map_guarded(screen_to_plane.rows, anchor_px_up)?;
    let origin = map_guarded(screen_to_plane.rows, [0.0; 2])?;
    let drag = map_guarded(
        screen_to_plane.rows,
        [drag_delta_px_down[0], -drag_delta_px_down[1]],
    )?;
    let pan_canvas_px = [drag[0] - origin[0], drag[1] - origin[1]];
    if !pan_canvas_px.iter().all(|value| value.is_finite()) {
        return Err(MathError::NonFinite);
    }
    Ok(crate::NavigationDelta {
        pan_canvas_px,
        zoom_delta_log2,
        anchor_canvas_px: anchor,
    })
}

pub fn multiply_3x3(left: [f64; 9], right: [f64; 9]) -> [f64; 9] {
    core::array::from_fn(|index| {
        let row = index / 3;
        let column = index % 3;
        (0..3).fold(0.0, |sum, inner| {
            left[row * 3 + inner].mul_add(right[inner * 3 + column], sum)
        })
    })
}

pub fn invert_3x3(matrix: [f64; 9]) -> Option<[f64; 9]> {
    if !matrix.iter().all(|value| value.is_finite()) {
        return None;
    }
    let mut augmented = [[0.0; 6]; 3];
    let mut row = 0;
    while row < 3 {
        let mut column = 0;
        while column < 3 {
            augmented[row][column] = matrix[row * 3 + column];
            column += 1;
        }
        augmented[row][row + 3] = 1.0;
        row += 1;
    }

    let mut pivot_column = 0;
    while pivot_column < 3 {
        let mut pivot_row = pivot_column;
        let mut pivot_magnitude = augmented[pivot_row][pivot_column].abs();
        let mut candidate = pivot_column + 1;
        while candidate < 3 {
            let magnitude = augmented[candidate][pivot_column].abs();
            if magnitude > pivot_magnitude {
                pivot_row = candidate;
                pivot_magnitude = magnitude;
            }
            candidate += 1;
        }
        if !pivot_magnitude.is_finite() || pivot_magnitude < PIVOT_EPSILON {
            return None;
        }
        augmented.swap(pivot_column, pivot_row);
        let pivot = augmented[pivot_column][pivot_column];
        let mut column = 0;
        while column < 6 {
            augmented[pivot_column][column] /= pivot;
            column += 1;
        }
        row = 0;
        while row < 3 {
            if row != pivot_column {
                let factor = augmented[row][pivot_column];
                column = 0;
                while column < 6 {
                    augmented[row][column] =
                        (-factor).mul_add(augmented[pivot_column][column], augmented[row][column]);
                    column += 1;
                }
            }
            row += 1;
        }
        pivot_column += 1;
    }
    let inverse = core::array::from_fn(|index| augmented[index / 3][index % 3 + 3]);
    inverse
        .iter()
        .all(|value| value.is_finite())
        .then_some(inverse)
}

#[cfg(test)]
fn map_projective(matrix: [f64; 9], point: [f64; 2]) -> Option<[f64; 2]> {
    let homogeneous = homogeneous(matrix, point);
    let denominator = homogeneous[2];
    if !denominator.is_finite() || denominator == 0.0 {
        return None;
    }
    let mapped = [homogeneous[0] / denominator, homogeneous[1] / denominator];
    mapped
        .iter()
        .all(|value| value.is_finite())
        .then_some(mapped)
}

const fn homogeneous(matrix: [f64; 9], point: [f64; 2]) -> [f64; 3] {
    let [x, y] = point;
    [
        matrix[0].mul_add(x, matrix[1].mul_add(y, matrix[2])),
        matrix[3].mul_add(x, matrix[4].mul_add(y, matrix[5])),
        matrix[6].mul_add(x, matrix[7].mul_add(y, matrix[8])),
    ]
}

fn map_guarded(matrix: [f64; 9], point: [f64; 2]) -> Result<[f64; 2], MathError> {
    let result = homogeneous(matrix, point);
    let denominator = result[2];
    if denominator <= 0.0 || !denominator.is_finite() {
        return Err(MathError::DegenerateViewMap);
    }
    let unit_roundoff = f64::EPSILON * 0.5;
    let gamma_five = 5.0 * unit_roundoff / 5.0_f64.mul_add(-unit_roundoff, 1.0);
    let scales = [
        matrix[0]
            .abs()
            .mul_add(point[0].abs(), matrix[1].abs() * point[1].abs())
            + matrix[2].abs(),
        matrix[3]
            .abs()
            .mul_add(point[0].abs(), matrix[4].abs() * point[1].abs())
            + matrix[5].abs(),
        matrix[6]
            .abs()
            .mul_add(point[0].abs(), matrix[7].abs() * point[1].abs())
            + matrix[8].abs(),
    ];
    let errors = scales.map(|scale| gamma_five * scale);
    if denominator <= errors[2] {
        return Err(MathError::DegenerateViewMap);
    }
    let mapped = [result[0] / denominator, result[1] / denominator];
    let safe_denominator = denominator - errors[2];
    let quotient_errors = [
        mapped[0].abs().mul_add(errors[2], errors[0]) / safe_denominator,
        mapped[1].abs().mul_add(errors[2], errors[1]) / safe_denominator,
    ];
    if !mapped.iter().all(|value| value.is_finite())
        || quotient_errors[0].hypot(quotient_errors[1]) > MAP_ERROR_LIMIT_PX
    {
        return Err(MathError::DegenerateViewMap);
    }
    Ok(mapped)
}

fn norm_infinity(matrix: [f64; 9]) -> f64 {
    matrix
        .as_chunks::<3>()
        .0
        .iter()
        .map(|row| row.iter().map(|value| value.abs()).sum::<f64>())
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(view: ViewControls, extent: [u32; 2]) -> Result<Homography, MathError> {
        screen_to_plane(
            &view,
            40.0,
            extent[0],
            extent[1],
            f64::from(extent[0]) / f64::from(extent[1]),
        )
    }

    #[test]
    fn canonical_flat_view_is_exact_identity() -> Result<(), MathError> {
        for extent in [[960, 540], [1024, 1024]] {
            for zoom in [0.0, 40.0, 100.0] {
                for distance in [2.0, 8.0, 64.0] {
                    let view = ViewControls {
                        distance_five: distance,
                        distance_four: distance,
                        ..ViewControls::NEUTRAL
                    };
                    let homography = screen_to_plane(
                        &view,
                        zoom,
                        extent[0],
                        extent[1],
                        f64::from(extent[0]) / f64::from(extent[1]),
                    )?;
                    assert_eq!(homography, Homography::IDENTITY);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn forward_after_inverse_is_identity_on_the_screen_lattice() -> Result<(), MathError> {
        let fixtures = [
            ViewControls::NEUTRAL,
            ViewControls {
                theta_1: 0.37,
                theta_2: 0.61,
                camera_yaw: 0.349,
                camera_pitch: 0.262,
                height_scale: 1.0,
                distance_five: 6.0,
                distance_four: 8.0,
            },
            ViewControls {
                theta_1: -0.4,
                theta_2: 0.2,
                camera_yaw: -0.41,
                camera_pitch: 0.23,
                height_scale: 0.5,
                distance_five: 2.0,
                distance_four: 64.0,
            },
        ];
        for extent in [[1024, 1024], [1920, 1080]] {
            for view in fixtures {
                let homography = map(view, extent)?;
                assert!(homography.condition_number.is_finite());
                for row in 0..9 {
                    for column in 0..9 {
                        let screen = [
                            (f64::from(column) / 8.0 - 0.5) * f64::from(extent[0]),
                            (f64::from(row) / 8.0 - 0.5) * f64::from(extent[1]),
                        ];
                        let plane = map_projective(homography.rows, screen)
                            .ok_or(MathError::DegenerateViewMap)?;
                        let recovered = map_projective(homography.inverse, plane)
                            .ok_or(MathError::DegenerateViewMap)?;
                        assert!(
                            (recovered[0] - screen[0]).hypot(recovered[1] - screen[1]) <= 1.0e-9,
                            "{extent:?} {view:?} at {screen:?}: {recovered:?}"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn edge_on_camera_is_refused_at_the_declared_pivot() {
        let view = ViewControls {
            camera_yaw: core::f64::consts::FRAC_PI_2,
            ..ViewControls::NEUTRAL
        };
        assert_eq!(map(view, [960, 540]), Err(MathError::DegenerateViewMap));
    }

    #[test]
    fn zoom_does_not_enter_pixel_unit_rows() -> Result<(), MathError> {
        let view = ViewControls {
            theta_1: 0.2,
            camera_yaw: 0.3,
            camera_pitch: -0.15,
            ..ViewControls::NEUTRAL
        };
        let first = screen_to_plane(&view, 0.0, 960, 540, 16.0 / 9.0)?;
        let deep = screen_to_plane(&view, 100.0, 960, 540, 16.0 / 9.0)?;
        assert_eq!(first, deep);
        Ok(())
    }

    #[test]
    fn inverse_retains_negative_denominators_beyond_the_horizon() -> Result<(), MathError> {
        let view = ViewControls {
            camera_yaw: 1.4,
            distance_four: 0.5,
            ..ViewControls::NEUTRAL
        };
        let homography = map(view, [960, 540])?;
        let denominators = [-10_000.0, 10_000.0].map(|x| homogeneous(homography.rows, [x, 0.0])[2]);
        assert!(
            denominators
                .into_iter()
                .any(|denominator| denominator < 0.0)
        );
        Ok(())
    }

    #[test]
    fn navigation_maps_anchor_and_drag_without_changing_the_public_delta() -> Result<(), MathError>
    {
        let identity = navigation_delta(&Homography::IDENTITY, [3.5, -2.25], 0.2, [19.5, -7.25])?;
        assert_eq!(identity.pan_canvas_px, [3.5, 2.25]);
        assert_eq!(identity.anchor_canvas_px, [19.5, -7.25]);
        assert_eq!(identity.zoom_delta_log2, 0.2);

        let homography = map(
            ViewControls {
                theta_1: 0.2,
                camera_yaw: 0.3,
                camera_pitch: -0.15,
                ..ViewControls::NEUTRAL
            },
            [960, 540],
        )?;
        let delta = navigation_delta(&homography, [40.0, 12.0], 0.0, [100.0, 50.0])?;
        let mapped_drag =
            map_projective(homography.rows, [40.0, -12.0]).ok_or(MathError::DegenerateViewMap)?;
        let mapped_origin =
            map_projective(homography.rows, [0.0; 2]).ok_or(MathError::DegenerateViewMap)?;
        assert_eq!(
            delta.pan_canvas_px,
            [
                mapped_drag[0] - mapped_origin[0],
                mapped_drag[1] - mapped_origin[1]
            ]
        );
        assert_eq!(
            delta.anchor_canvas_px,
            map_projective(homography.rows, [100.0, 50.0]).ok_or(MathError::DegenerateViewMap)?
        );
        Ok(())
    }
}
