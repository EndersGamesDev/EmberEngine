use crate::{Homography, MathError, ObjectAngles, Plane, ViewControls, construct_plane};

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
    object: &ObjectAngles,
    view: &ViewControls,
    zoom_log2: f64,
    grid_w: u32,
    grid_h: u32,
    aspect: f64,
) -> Result<Homography, MathError> {
    if !object.is_valid() || !view.is_valid() {
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

    let plane = construct_plane(*object)?;
    if is_canonical_flat_pair(*object, *view) {
        return Ok(Homography::IDENTITY);
    }
    let forward = forward_homography(plane, view, grid_w)?;
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

fn is_canonical_flat_pair(object: ObjectAngles, view: ViewControls) -> bool {
    view.camera_yaw == 0.0
        && view.camera_pitch == 0.0
        && view
            .camera_translation
            .iter()
            .all(|value| value.to_bits() << 1 == 0)
        && ((object == ObjectAngles::IDENTITY
            && view.camera == ViewControls::MANDELBROT_FLAT.camera)
            || (object == ObjectAngles::JULIA && view.camera == ViewControls::NEUTRAL.camera))
}

fn forward_homography(
    plane: Plane,
    view: &ViewControls,
    grid_w: u32,
) -> Result<[f64; 9], MathError> {
    let camera = camera_matrix(view);
    let chart_scale = 4.0 / f64::from(grid_w);
    let transformed_basis: [[f64; 5]; 2] = [plane.basis_u, plane.basis_v].map(|basis| {
        let ambient = [
            f64::from(basis[0]),
            f64::from(basis[1]),
            f64::from(basis[2]),
            f64::from(basis[3]),
            0.0,
        ];
        core::array::from_fn(|row| {
            camera[row]
                .into_iter()
                .zip(ambient)
                .fold(0.0, |sum, (coefficient, value)| {
                    coefficient.mul_add(value, sum)
                })
        })
    });
    let q: [[f64; 3]; 5] = core::array::from_fn(|axis| {
        [
            chart_scale * transformed_basis[0][axis],
            chart_scale * transformed_basis[1][axis],
            view.camera_translation[axis],
        ]
    });
    let distance_five = view.distance_five;
    let distance_four = view.distance_four;
    let perspective_product = distance_four * distance_five;
    let numerator = [
        scale_row(q[0], perspective_product),
        scale_row(q[1], perspective_product),
        scale_row(q[2], perspective_product),
    ];
    let denominator_four = add_rows(
        add_rows(
            [0.0, 0.0, perspective_product],
            scale_row(q[4], -distance_four),
        ),
        scale_row(q[3], -distance_five),
    );
    let (yaw_sine, yaw_cosine) = view.camera_yaw.sin_cos();
    let (pitch_sine, pitch_cosine) = view.camera_pitch.sin_cos();
    let yawed_x = add_rows(
        scale_row(numerator[0], yaw_cosine),
        scale_row(numerator[2], yaw_sine),
    );
    let yawed_z = add_rows(
        scale_row(numerator[0], -yaw_sine),
        scale_row(numerator[2], yaw_cosine),
    );
    let view_y = add_rows(
        scale_row(numerator[1], pitch_cosine),
        scale_row(yawed_z, -pitch_sine),
    );
    let clip_w = add_rows(
        add_rows(
            scale_row(denominator_four, distance_four),
            scale_row(numerator[1], -pitch_sine),
        ),
        scale_row(yawed_z, -pitch_cosine),
    );
    let viewport_scale = f64::from(grid_w) * distance_four * 0.25;
    let x = scale_row(yawed_x, viewport_scale);
    let y = scale_row(view_y, viewport_scale);
    let normalizer = clip_w[2];
    if !normalizer.is_finite() || normalizer.abs() < PIVOT_EPSILON {
        return Err(MathError::DegenerateViewMap);
    }
    let forward = [
        x[0] / normalizer,
        x[1] / normalizer,
        x[2] / normalizer,
        y[0] / normalizer,
        y[1] / normalizer,
        y[2] / normalizer,
        clip_w[0] / normalizer,
        clip_w[1] / normalizer,
        1.0,
    ];
    forward
        .iter()
        .all(|value| value.is_finite())
        .then_some(forward)
        .ok_or(MathError::NonFinite)
}

const fn scale_row(row: [f64; 3], scale: f64) -> [f64; 3] {
    [row[0] * scale, row[1] * scale, row[2] * scale]
}

const fn add_rows(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

pub fn camera_matrix(view: &ViewControls) -> [[f64; 5]; 5] {
    let columns: [[f64; 5]; 5] = core::array::from_fn(|column| {
        let mut value = [0.0; 5];
        value[column] = 1.0;
        for factor in (0..ViewControls::CAMERA_PLANES.len()).rev() {
            let (first, second) = ViewControls::CAMERA_PLANES[factor];
            rotate_pair(&mut value, first, second, view.camera[factor]);
        }
        value
    });
    core::array::from_fn(|row| core::array::from_fn(|column| columns[column][row]))
}

fn rotate_pair(value: &mut [f64; 5], first: usize, second: usize, angle: f64) {
    let (sine, cosine) = angle.sin_cos();
    let a = cosine.mul_add(value[first], -sine * value[second]);
    let b = sine.mul_add(value[first], cosine * value[second]);
    value[first] = a;
    value[second] = b;
}

#[must_use]
pub fn rotation_orthonormality_5(view: &ViewControls) -> f64 {
    let matrix = camera_matrix(view);
    (0..5)
        .flat_map(|row| (0..5).map(move |column| (row, column)))
        .map(|(row, column)| {
            let product = (0..5).fold(0.0, |sum, inner| {
                matrix[inner][row].mul_add(matrix[inner][column], sum)
            });
            let expected = if row == column { 1.0 } else { 0.0 };
            (product - expected).abs()
        })
        .fold(0.0, f64::max)
}

/// Converts page target, selection, and scale inputs into local plane-offset pixels.
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

/// Projects a plane-offset point back to centred screen pixels through the forward homography.
///
/// # Errors
///
/// Returns an error for a non-finite point, a point on the projective horizon, or an evaluation
/// whose conservative binary64 error exceeds one quarter pixel.
pub fn plane_to_screen(
    screen_to_plane: &Homography,
    plane_offset_px: [f64; 2],
) -> Result<[f64; 2], MathError> {
    if !plane_offset_px.into_iter().all(f64::is_finite) {
        return Err(MathError::NonFinite);
    }
    map_guarded(screen_to_plane.inverse, plane_offset_px)
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

    fn map(
        object: ObjectAngles,
        view: ViewControls,
        extent: [u32; 2],
    ) -> Result<Homography, MathError> {
        screen_to_plane(
            &object,
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
                    let mandelbrot = ViewControls {
                        distance_five: distance,
                        distance_four: distance,
                        ..ViewControls::MANDELBROT_FLAT
                    };
                    let julia = ViewControls {
                        distance_five: distance,
                        distance_four: distance,
                        ..ViewControls::NEUTRAL
                    };
                    for (object, view) in [
                        (ObjectAngles::IDENTITY, mandelbrot),
                        (ObjectAngles::JULIA, julia),
                    ] {
                        let homography = screen_to_plane(
                            &object,
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
        }
        Ok(())
    }

    #[test]
    fn canonical_shortcut_matches_the_forward_chain_to_binary64_tolerance() -> Result<(), MathError>
    {
        for (object, view) in [
            (ObjectAngles::IDENTITY, ViewControls::MANDELBROT_FLAT),
            (ObjectAngles::JULIA, ViewControls::NEUTRAL),
        ] {
            let forward = forward_homography(construct_plane(object)?, &view, 960)?;
            for (index, value) in forward.into_iter().enumerate() {
                let expected = if index % 4 == 0 { 1.0 } else { 0.0 };
                assert!((value - expected).abs() <= 1.0e-12);
            }
        }
        Ok(())
    }

    #[test]
    fn mandelbrot_camera_quarter_turns_face_the_seed_with_the_shipped_orientation() {
        let matrix = camera_matrix(&ViewControls::MANDELBROT_FLAT);
        assert!((matrix[0][2] - 1.0).abs() <= f64::EPSILON);
        assert!((matrix[1][3] - 1.0).abs() <= f64::EPSILON);
        assert!(matrix[2][2].abs() <= f64::EPSILON);
        assert!(matrix[3][3].abs() <= f64::EPSILON);
    }

    /// The screen map is the neutral-height chart map, so the height control cannot appear in it.
    ///
    /// This is the whole reason a height change reprojects nothing on its own: the image
    /// homography every reprojection is fitted from is built here, and it is bit-identical across
    /// height amplitudes. The escape height reaches the picture only through the scene pass, which
    /// lifts each vertex by that vertex's own record, so a height change is a deformation of the
    /// surface rather than a motion of the observer.
    #[test]
    fn the_screen_map_is_independent_of_the_height_amplitude() -> Result<(), MathError> {
        let mut tumbled_camera = [0.0; 10];
        tumbled_camera[0] = 0.37;
        tumbled_camera[8] = 0.31;
        let base = ViewControls {
            camera: tumbled_camera,
            camera_translation: [0.2, -0.1, 0.3, 0.05, -0.2],
            camera_yaw: 0.349,
            camera_pitch: 0.262,
            height_scale: 0.0,
            distance_five: 6.0,
            distance_four: 8.0,
        };
        let flat = map(ObjectAngles::JULIA, base, [1920, 1080])?;
        for height_scale in [0.5, 1.0, 2.5, 4.0] {
            let lifted = map(
                ObjectAngles::JULIA,
                ViewControls {
                    height_scale,
                    ..base
                },
                [1920, 1080],
            )?;
            assert_eq!(
                lifted.rows.map(f64::to_bits),
                flat.rows.map(f64::to_bits),
                "height {height_scale} moved the screen map"
            );
            assert_eq!(
                lifted.inverse.map(f64::to_bits),
                flat.inverse.map(f64::to_bits),
                "height {height_scale} moved the inverse map"
            );
        }
        Ok(())
    }

    #[test]
    fn forward_after_inverse_is_identity_on_the_screen_lattice() -> Result<(), MathError> {
        let mut tumbled_camera = [0.0; 10];
        tumbled_camera[0] = 0.37;
        tumbled_camera[8] = 0.31;
        let mut second_camera = ViewControls::MANDELBROT_FLAT.camera;
        second_camera[6] = -0.2;
        let fixtures = [
            (ObjectAngles::JULIA, ViewControls::NEUTRAL),
            (
                ObjectAngles::JULIA,
                ViewControls {
                    camera: tumbled_camera,
                    camera_translation: [0.2, -0.1, 0.3, 0.05, -0.2],
                    camera_yaw: 0.349,
                    camera_pitch: 0.262,
                    height_scale: 1.0,
                    distance_five: 6.0,
                    distance_four: 8.0,
                },
            ),
            (
                ObjectAngles {
                    rho_14: 0.2,
                    rho_23: -0.15,
                    ..ObjectAngles::IDENTITY
                },
                ViewControls {
                    camera: second_camera,
                    camera_translation: [-0.3, 0.2, 0.1, -0.05, 0.15],
                    camera_yaw: -0.41,
                    camera_pitch: 0.23,
                    height_scale: 0.5,
                    distance_five: 2.0,
                    distance_four: 64.0,
                },
            ),
        ];
        for extent in [[1024, 1024], [1920, 1080]] {
            for (object, view) in fixtures {
                let homography = map(object, view, extent)?;
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
        assert_eq!(
            map(ObjectAngles::IDENTITY, ViewControls::NEUTRAL, [960, 540]),
            Err(MathError::DegenerateViewMap)
        );
    }

    #[test]
    fn zoom_does_not_enter_pixel_unit_rows() -> Result<(), MathError> {
        let view = ViewControls::MANDELBROT_FLAT;
        let first = screen_to_plane(&ObjectAngles::IDENTITY, &view, 0.0, 960, 540, 16.0 / 9.0)?;
        let deep = screen_to_plane(&ObjectAngles::IDENTITY, &view, 100.0, 960, 540, 16.0 / 9.0)?;
        assert_eq!(first, deep);
        Ok(())
    }

    #[test]
    fn inverse_retains_negative_denominators_beyond_the_horizon() -> Result<(), MathError> {
        let mut camera = [0.0; 10];
        camera[6] = 1.4;
        let view = ViewControls {
            camera,
            distance_five: 0.5,
            ..ViewControls::NEUTRAL
        };
        let homography = map(ObjectAngles::JULIA, view, [960, 540])?;
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

        let mut camera = ViewControls::MANDELBROT_FLAT.camera;
        camera[0] = 0.2;
        let homography = map(
            ObjectAngles::IDENTITY,
            ViewControls {
                camera,
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

    #[test]
    fn plane_projection_is_the_inverse_navigation_uses() -> Result<(), MathError> {
        let object = ObjectAngles::IDENTITY;
        let view = ViewControls {
            camera: [
                0.07, -1.1, 0.03, -0.11, -1.2, 0.05, 0.09, -0.04, 0.13, -0.08,
            ],
            camera_translation: [0.2, -0.1, 0.3, -0.2, 0.15],
            camera_yaw: 0.17,
            camera_pitch: -0.12,
            ..ViewControls::MANDELBROT_FLAT
        };
        let homography = map(object, view, [960, 540])?;
        for screen in [[0.0, 0.0], [137.0, -64.0], [-311.0, 201.0]] {
            let plane = navigation_delta(&homography, [0.0; 2], 0.0, screen)?.anchor_canvas_px;
            let projected = plane_to_screen(&homography, plane)?;
            assert!((projected[0] - screen[0]).abs() < 1.0e-9);
            assert!((projected[1] - screen[1]).abs() < 1.0e-9);
        }
        Ok(())
    }

    #[test]
    fn object_and_camera_rotations_remain_orthonormal_for_random_angles() {
        let mut state = 0x4d59_5df4_d0f3_3173_u64;
        let mut next_angle = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let word = u32::try_from(state >> 32).expect("shifted random word fits u32");
            let unit = f64::from(word) / f64::from(u32::MAX);
            core::f64::consts::TAU.mul_add(unit, -core::f64::consts::PI)
        };
        for _ in 0..256 {
            let values: [f64; 6] = core::array::from_fn(|_| next_angle());
            let object = ObjectAngles {
                rho_12: values[0],
                rho_13: values[1],
                rho_14: values[2],
                rho_23: values[3],
                rho_24: values[4],
                rho_34: values[5],
            };
            let view = ViewControls {
                camera: core::array::from_fn(|_| next_angle()),
                ..ViewControls::NEUTRAL
            };
            assert!(crate::rotation_orthonormality_4(&object) <= 1.0e-12);
            assert!(rotation_orthonormality_5(&view) <= 1.0e-12);
        }
    }

    #[test]
    fn the_julia_to_mandelbrot_object_morph_meets_edge_on_once() {
        let refusals = (0..=256)
            .filter(|step| {
                let t = f64::from(*step) / 256.0;
                let object =
                    crate::lerp_object_angles(ObjectAngles::JULIA, ObjectAngles::IDENTITY, t)
                        .expect("finite object morph");
                map(object, ViewControls::NEUTRAL, [960, 540]).is_err()
            })
            .count();
        assert_eq!(refusals, 1);
    }
}
