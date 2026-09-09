use crate::basis::radian_sin_cos;
use crate::fixed::solve_two_axis_gram;
use crate::types::ObserverProjection;
use crate::{
    CameraError, Fixed, Observer, Screen, TWO_STAGE_FRAME_PLANES, TwoStageProjection, View,
    rebuild_basis, scale_for,
};

/// Binary64 encoding of the nominal symmetric projection range.
const PROJECT_RANGE_BITS: u64 = 0x41e0_0000_0000_0000;

/// Nominal symmetric pixel range in which projection keeps every intermediate screen-sized.
///
/// Two billion pixels exceeds any `u32` render-grid radius while ensuring dot-product accumulation
/// remains far from binary64 overflow. A projection readout may extend through
/// [`PROJECT_READOUT_LIMIT_PIXELS`] so its last rounding step remains valid input to
/// [`crate::click`].
pub const PROJECT_RANGE_PIXELS: f64 = f64::from_bits(PROJECT_RANGE_BITS);

/// Binary64 readout budget for reconstructing an arbitrary image-plane point.
///
/// Projection necessarily rounds its exact fixed rational quotient to a binary64 pixel coordinate.
///
/// Nearest-even conversion is within half a representable step; one complete step is retained as
/// the public reconstruction budget and does not grow with the exponent or orientation.
pub const PROJECT_READOUT_ULPS: u64 = 1;

/// Orientation-independent absolute projection-error budget in pixels.
///
/// This is one binary64 step immediately above [`PROJECT_RANGE_PIXELS`], the largest spacing in the
/// readout envelope. The exact fixed rational quotient rounds by at most half of its local step;
/// the complete step leaves a conservative uniform bound at the binade boundary.
pub const PROJECT_PIXEL_TOLERANCE_PIXELS: f64 = 4.768_371_582_031_25e-7;

/// Outer projection-readout and exact-decoder range in centred pixels.
///
/// The nominal range plus [`PROJECT_READOUT_ULPS`] upward binary64 step ensures that
/// `click(project(point))` remains accepted when a boundary readout rounds just outward.
pub const PROJECT_READOUT_LIMIT_PIXELS: f64 =
    f64::from_bits(PROJECT_RANGE_BITS + PROJECT_READOUT_ULPS);

/// Denominator magnitude treated as a perspective ray parallel to the base plane.
const PERSPECTIVE_RAY_EPSILON: f64 = 1.0e-12;

/// Absolute pixel tolerance for a presentation projection followed by its inverse.
///
/// Both operations use the same documented binary64 transform. One billionth of a pixel covers
/// the final multiply, divide, and ray-intersection rounding at ordinary render extents.
pub const PERSPECTIVE_ROUND_TRIP_TOLERANCE_PIXELS: f64 = 1.0e-9;

const fn multiply_then_add(left: f64, right: f64, addend: f64) -> f64 {
    // Exact navigation ends before observer projection. Its named binary64 tolerance includes
    // these two core operations, so fused evaluation is not part of the boundary contract.
    left * right + addend
}

/// Projects a flat base-plane point through the observer into centred render-grid pixels.
///
/// For the simple observer, the input pixel is first scaled to the four-unit base plane and
/// translation is added. The documented forward rotation is `R = P(pitch) * Y(yaw)`. If
/// `(x, y, z) = R * ([4 * px / width, 4 * py / width, 0] + translation)`, the perspective result
/// is `width / 4 * [d * x / (d - z), d * y / (d - z)]`. The complete two-stage observer applies
/// its five-dimensional rotation, translation, five-to-four divide, and the same final transform.
///
/// `None` means the observer is invalid, has fewer than three dimensions, the input is non-finite,
/// or the point lies on or behind a perspective plane.
#[must_use]
pub fn project_perspective<const N: usize>(
    observer: &Observer<N>,
    screen: Screen,
    base_point_px: [f64; 2],
) -> Option<[f64; 2]> {
    if N < 3 || !base_point_px.iter().all(|component| component.is_finite()) {
        return None;
    }
    match observer.projection {
        ObserverProjection::Simple => project_simple_perspective(observer, screen, base_point_px),
        ObserverProjection::TwoStage => {
            let projection = observer.two_stage?;
            let forward = two_stage_forward_homography(&projection, screen)?;
            map_projective(forward, base_point_px)
        }
    }
}

/// Projects an N-dimensional point into centred render-grid pixels.
///
/// The function performs N exact big subtractions first. Rebuilt binary64 basis bits then enter
/// fixed precision exactly; both displacement dots and all three Gram terms are evaluated there.
/// The resulting rational Gram solve is rounded directly to binary64, without floating-point dot
/// accumulation or division. Orthogonal components are discarded, so `click(project(point))` is
/// an inverse only for points on the image plane.
///
/// # Errors
///
/// Returns a typed refusal for invalid geometry or checked fixed-point arithmetic failure.
pub fn project<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    screen: Screen,
    point: &[Fixed<LIMBS>; N],
) -> Result<Option<[f64; 2]>, CameraError> {
    let mut delta = [Fixed::ZERO; N];
    for ((output, coordinate), centre) in delta.iter_mut().zip(point).zip(view.centre) {
        *output = coordinate.sub(&centre)?;
    }
    let projected = project_delta(view, screen, &delta)?;
    if projected
        .iter()
        .all(|component| component.is_finite() && component.abs() <= PROJECT_READOUT_LIMIT_PIXELS)
    {
        Ok(Some(projected))
    } else {
        Ok(None)
    }
}

/// Returns the current centre's render-grid displacement from a reference centre.
///
/// This per-frame renderer quantity performs N exact subtractions and a fixed-point Gram solve,
/// then rounds only each final rational pixel quotient to binary64. Unlike a pointer coordinate,
/// the displacement from a stale reference may span more than the complete screen-input range at
/// deep zoom; only a non-finite final readout is refused.
///
/// # Errors
///
/// Returns a typed refusal for invalid geometry, a non-finite result, or checked arithmetic
/// failure.
pub fn reference_displacement<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    reference_centre: &[Fixed<LIMBS>; N],
    screen: Screen,
) -> Result<[f64; 2], CameraError> {
    let mut delta = [Fixed::ZERO; N];
    for ((output, centre), reference) in delta.iter_mut().zip(view.centre).zip(reference_centre) {
        *output = centre.sub(reference)?;
    }
    let projected = project_delta(view, screen, &delta)?;
    if projected.iter().all(|component| component.is_finite()) {
        Ok(projected)
    } else {
        Err(CameraError::ScreenCoordinateOutOfRange)
    }
}

/// Inverts a tilted observer's screen ray onto the flat base plane.
///
/// Screen coordinates are centred render-grid pixels with positive y upward. They are first mapped
/// to the exponent-zero four-unit view width. Yaw rotates about the rebuilt `v` axis and pitch about
/// `u`; translation is applied in view units before the perspective divide. The ray is intersected
/// with base height zero, never with relief. A surface snap is therefore a presentation convenience:
/// it must produce a base-plane point and then use [`crate::click`].
///
/// `None` means the observer is invalid, has fewer than three dimensions, or the ray is parallel to
/// or points away from the base plane.
#[must_use]
pub fn invert_perspective<const N: usize>(
    observer: &Observer<N>,
    screen: Screen,
    screen_px: [f64; 2],
) -> Option<[f64; 2]> {
    if N < 3 || !screen_px.iter().all(|component| component.is_finite()) {
        return None;
    }
    match observer.projection {
        ObserverProjection::Simple => invert_simple_perspective(observer, screen, screen_px),
        ObserverProjection::TwoStage => {
            let projection = observer.two_stage?;
            invert_two_stage_perspective(&projection, screen, screen_px)
        }
    }
}

fn invert_simple_perspective<const N: usize>(
    observer: &Observer<N>,
    screen: Screen,
    screen_px: [f64; 2],
) -> Option<[f64; 2]> {
    let units_per_pixel = 4.0 / f64::from(screen.width());
    let horizontal = screen_px[0] * units_per_pixel;
    let vertical = screen_px[1] * units_per_pixel;
    let (yaw_sine, yaw_cosine) = radian_sin_cos(observer.yaw);
    let (pitch_sine, pitch_cosine) = radian_sin_cos(observer.pitch);
    let pole = inverse_observer_rotation(
        [0.0, 0.0, observer.perspective],
        yaw_sine,
        yaw_cosine,
        pitch_sine,
        pitch_cosine,
    );
    let origin = [
        pole[0] - observer.translation[0],
        pole[1] - observer.translation[1],
        pole[2] - observer.translation[2],
    ];
    let ray = inverse_observer_rotation(
        [horizontal, vertical, -observer.perspective],
        yaw_sine,
        yaw_cosine,
        pitch_sine,
        pitch_cosine,
    );
    if !ray[2].is_finite() || ray[2].abs() <= PERSPECTIVE_RAY_EPSILON {
        return None;
    }
    let distance = -origin[2] / ray[2];
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let base_horizontal = multiply_then_add(distance, ray[0], origin[0]);
    let base_vertical = multiply_then_add(distance, ray[1], origin[1]);
    let pixels_per_unit = f64::from(screen.width()) / 4.0;
    let result = [
        base_horizontal * pixels_per_unit,
        base_vertical * pixels_per_unit,
    ];
    result
        .iter()
        .all(|component| component.is_finite())
        .then_some(result)
}

fn project_simple_perspective<const N: usize>(
    observer: &Observer<N>,
    screen: Screen,
    base_point_px: [f64; 2],
) -> Option<[f64; 2]> {
    let chart_scale = 4.0 / f64::from(screen.width());
    let translated = [
        (base_point_px[0] * chart_scale) + observer.translation[0],
        (base_point_px[1] * chart_scale) + observer.translation[1],
        observer.translation[2],
    ];
    let (yaw_sine, yaw_cosine) = radian_sin_cos(observer.yaw);
    let (pitch_sine, pitch_cosine) = radian_sin_cos(observer.pitch);
    let yawed_horizontal = multiply_then_add(yaw_cosine, translated[0], yaw_sine * translated[2]);
    let yawed_depth = multiply_then_add(-yaw_sine, translated[0], yaw_cosine * translated[2]);
    let viewed_vertical = multiply_then_add(pitch_cosine, translated[1], -pitch_sine * yawed_depth);
    let viewed_depth = multiply_then_add(pitch_sine, translated[1], pitch_cosine * yawed_depth);
    let denominator = observer.perspective - viewed_depth;
    if !denominator.is_finite() || denominator <= PERSPECTIVE_RAY_EPSILON {
        return None;
    }
    let pixel_scale = f64::from(screen.width()) * 0.25;
    let perspective_scale = (pixel_scale * observer.perspective) / denominator;
    let result = [
        perspective_scale * yawed_horizontal,
        perspective_scale * viewed_vertical,
    ];
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

fn inverse_observer_rotation(
    vector: [f64; 3],
    yaw_sine: f64,
    yaw_cosine: f64,
    pitch_sine: f64,
    pitch_cosine: f64,
) -> [f64; 3] {
    let vertical = multiply_then_add(pitch_cosine, vector[1], pitch_sine * vector[2]);
    let yaw_depth = multiply_then_add(-pitch_sine, vector[1], pitch_cosine * vector[2]);
    [
        multiply_then_add(yaw_cosine, vector[0], -yaw_sine * yaw_depth),
        vertical,
        multiply_then_add(yaw_sine, vector[0], yaw_cosine * yaw_depth),
    ]
}

fn invert_two_stage_perspective(
    projection: &TwoStageProjection,
    screen: Screen,
    screen_px: [f64; 2],
) -> Option<[f64; 2]> {
    let forward = two_stage_forward_homography(projection, screen)?;
    map_projective(invert_projective(forward)?, screen_px)
}

fn two_stage_forward_homography(
    projection: &TwoStageProjection,
    screen: Screen,
) -> Option<[f64; 9]> {
    let camera = presentation_matrix(projection.frame_angles);
    let chart_scale = 4.0 / f64::from(screen.width());
    let transformed_basis: [[f64; 5]; 2] = projection.image_plane.map(|basis| {
        core::array::from_fn(|row| {
            camera[row]
                .into_iter()
                .zip(basis)
                .fold(0.0, |sum, (coefficient, value)| {
                    multiply_then_add(coefficient, value, sum)
                })
        })
    });
    let q: [[f64; 3]; 5] = core::array::from_fn(|axis| {
        [
            chart_scale * transformed_basis[0][axis],
            chart_scale * transformed_basis[1][axis],
            projection.translation[axis],
        ]
    });
    let distance_five = projection.distance_five;
    let distance_four = projection.distance_four;
    let perspective_product = distance_four * distance_five;
    let numerator = [
        scale_homogeneous_row(q[0], perspective_product),
        scale_homogeneous_row(q[1], perspective_product),
        scale_homogeneous_row(q[2], perspective_product),
    ];
    let denominator_four = add_homogeneous_rows(
        add_homogeneous_rows(
            [0.0, 0.0, perspective_product],
            scale_homogeneous_row(q[4], -distance_four),
        ),
        scale_homogeneous_row(q[3], -distance_five),
    );
    let (yaw_sine, yaw_cosine) = radian_sin_cos(projection.yaw);
    let (pitch_sine, pitch_cosine) = radian_sin_cos(projection.pitch);
    let yawed_x = add_homogeneous_rows(
        scale_homogeneous_row(numerator[0], yaw_cosine),
        scale_homogeneous_row(numerator[2], yaw_sine),
    );
    let yawed_z = add_homogeneous_rows(
        scale_homogeneous_row(numerator[0], -yaw_sine),
        scale_homogeneous_row(numerator[2], yaw_cosine),
    );
    let view_y = add_homogeneous_rows(
        scale_homogeneous_row(numerator[1], pitch_cosine),
        scale_homogeneous_row(yawed_z, -pitch_sine),
    );
    let clip_w = add_homogeneous_rows(
        add_homogeneous_rows(
            scale_homogeneous_row(denominator_four, distance_four),
            scale_homogeneous_row(numerator[1], -pitch_sine),
        ),
        scale_homogeneous_row(yawed_z, -pitch_cosine),
    );
    let viewport_scale = f64::from(screen.width()) * distance_four * 0.25;
    let x = scale_homogeneous_row(yawed_x, viewport_scale);
    let y = scale_homogeneous_row(view_y, viewport_scale);
    let normalizer = clip_w[2];
    if !normalizer.is_finite() || normalizer.abs() < PERSPECTIVE_RAY_EPSILON {
        return None;
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
}

fn presentation_matrix(angles: [f64; 10]) -> [[f64; 5]; 5] {
    let columns: [[f64; 5]; 5] = core::array::from_fn(|column| {
        let mut value = [0.0; 5];
        value[column] = 1.0;
        for (factor, (first, second)) in TWO_STAGE_FRAME_PLANES.into_iter().enumerate().rev() {
            rotate_pair(&mut value, first, second, angles[factor]);
        }
        value
    });
    core::array::from_fn(|row| core::array::from_fn(|column| columns[column][row]))
}

fn rotate_pair(value: &mut [f64; 5], first: usize, second: usize, angle: f64) {
    let (sine, cosine) = radian_sin_cos(angle);
    let first_value = multiply_then_add(cosine, value[first], -sine * value[second]);
    let second_value = multiply_then_add(sine, value[first], cosine * value[second]);
    value[first] = first_value;
    value[second] = second_value;
}

const fn scale_homogeneous_row(row: [f64; 3], scale: f64) -> [f64; 3] {
    [row[0] * scale, row[1] * scale, row[2] * scale]
}

const fn add_homogeneous_rows(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn invert_projective(matrix: [f64; 9]) -> Option<[f64; 9]> {
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
        if !pivot_magnitude.is_finite() || pivot_magnitude < PERSPECTIVE_RAY_EPSILON {
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
                    augmented[row][column] = multiply_then_add(
                        -factor,
                        augmented[pivot_column][column],
                        augmented[row][column],
                    );
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

fn map_projective(matrix: [f64; 9], point: [f64; 2]) -> Option<[f64; 2]> {
    let homogeneous = [
        multiply_then_add(
            matrix[0],
            point[0],
            multiply_then_add(matrix[1], point[1], matrix[2]),
        ),
        multiply_then_add(
            matrix[3],
            point[0],
            multiply_then_add(matrix[4], point[1], matrix[5]),
        ),
        multiply_then_add(
            matrix[6],
            point[0],
            multiply_then_add(matrix[7], point[1], matrix[8]),
        ),
    ];
    if !homogeneous[2].is_finite() || homogeneous[2] <= PERSPECTIVE_RAY_EPSILON {
        return None;
    }
    let result = [
        homogeneous[0] / homogeneous[2],
        homogeneous[1] / homogeneous[2],
    ];
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

fn project_delta<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    screen: Screen,
    delta: &[Fixed<LIMBS>; N],
) -> Result<[f64; 2], CameraError> {
    let basis = rebuild_basis(&view.orientation)?;
    let scale = scale_for::<LIMBS>(view.exponent, screen)?.units_per_pixel();
    let mut horizontal_basis = [Fixed::ZERO; N];
    let mut vertical_basis = [Fixed::ZERO; N];
    for (((horizontal, vertical), horizontal_bits), vertical_bits) in horizontal_basis
        .iter_mut()
        .zip(&mut vertical_basis)
        .zip(basis.u)
        .zip(basis.v)
    {
        *horizontal = Fixed::from_binary64_bits(horizontal_bits.to_bits())?;
        *vertical = Fixed::from_binary64_bits(vertical_bits.to_bits())?;
    }
    solve_two_axis_gram(delta, &horizontal_basis, &vertical_basis, &scale)
}

#[cfg(test)]
mod tests {
    use super::{
        PERSPECTIVE_ROUND_TRIP_TOLERANCE_PIXELS, PROJECT_PIXEL_TOLERANCE_PIXELS,
        PROJECT_RANGE_PIXELS, PROJECT_READOUT_LIMIT_PIXELS, PROJECT_READOUT_ULPS,
        invert_perspective, project, project_perspective, reference_displacement,
    };
    use crate::{
        CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, MAX_SCREEN_COORDINATE_PIXELS,
        Observer, Orientation, Screen, Turn, TwoStageProjection, View, click, pan, rebuild_basis,
        scale_for,
    };

    /// Width exercised by projection invariants because it is the first consumer's fixed budget.
    const ROUND_TRIP_LIMBS: usize = 8;

    /// Dimension exercised by projection invariants because it activates all first-consumer planes.
    const ROUND_TRIP_DIMENSIONS: usize = 5;

    /// Compact width used only to keep the pinned transport golden readable.
    const GOLDEN_LIMBS: usize = 2;

    type RoundTripView = View<ROUND_TRIP_DIMENSIONS, ROUND_TRIP_LIMBS>;

    /// Converts the named binary64-step budget to plane units at the current scale.
    const fn readout_plane_tolerance(pixel: f64, units_per_pixel: f64) -> f64 {
        let magnitude_bits = pixel.to_bits() & (u64::MAX >> 1);
        let magnitude = f64::from_bits(magnitude_bits);
        let next = f64::from_bits(magnitude_bits + PROJECT_READOUT_ULPS);
        (next - magnitude) * units_per_pixel
    }

    fn all_plane_orientation() -> Result<Orientation<ROUND_TRIP_DIMENSIONS>, CameraError> {
        let mut angles = [[Turn::ZERO; ROUND_TRIP_DIMENSIONS]; ROUND_TRIP_DIMENSIONS];
        angles[0][1] = Turn::from_bits(0x0123_4567);
        angles[0][2] = Turn::from_bits(0x1234_5678);
        angles[0][3] = Turn::from_bits(0x2345_6789);
        angles[0][4] = Turn::from_bits(0x3456_789a);
        angles[1][2] = Turn::from_bits(0x4567_89ab);
        angles[1][3] = Turn::from_bits(0x5678_9abc);
        angles[1][4] = Turn::from_bits(0x6789_abcd);
        angles[2][3] = Turn::from_bits(0x789a_bcde);
        angles[2][4] = Turn::from_bits(0x89ab_cdef);
        angles[3][4] = Turn::from_bits(0x9abc_def0);
        Orientation::new(angles)
    }

    fn review_orientation() -> Result<Orientation<ROUND_TRIP_DIMENSIONS>, CameraError> {
        let mut angles = [[Turn::ZERO; ROUND_TRIP_DIMENSIONS]; ROUND_TRIP_DIMENSIONS];
        angles[0][3] = Turn::from_bits(0x2b60_35ee);
        angles[0][4] = Turn::from_bits(0x6b17_5866);
        angles[1][3] = Turn::from_bits(0xc56a_58ce);
        angles[1][4] = Turn::from_bits(0xb5cc_ebbf);
        Orientation::new(angles)
    }

    fn assert_readout_round_trip(
        view: &RoundTripView,
        screen: Screen,
        pixel: [f64; 2],
    ) -> Result<(), CameraError> {
        let point = click(view, screen, pixel)?;
        let projected =
            project(view, screen, &point)?.ok_or(CameraError::ScreenCoordinateOutOfRange)?;
        for (actual, expected) in projected.iter().zip(pixel) {
            let error = (*actual - expected).abs();
            assert!(error <= PROJECT_PIXEL_TOLERANCE_PIXELS);
            assert!(error <= readout_plane_tolerance(*actual, 1.0));
        }

        let rebuilt = click(view, screen, projected)?;
        let units_per_pixel = scale_for::<ROUND_TRIP_LIMBS>(view.exponent, screen)?
            .units_per_pixel()
            .to_f64()?;
        let horizontal_tolerance = readout_plane_tolerance(projected[0], units_per_pixel);
        let vertical_tolerance = readout_plane_tolerance(projected[1], units_per_pixel);
        let basis = rebuild_basis(&view.orientation)?;
        for (((actual, expected), horizontal_basis), vertical_basis) in
            rebuilt.iter().zip(point).zip(basis.u).zip(basis.v)
        {
            let error = actual.sub(&expected)?.abs_checked()?.to_f64()?;
            let tolerance = horizontal_tolerance * horizontal_basis.abs()
                + vertical_tolerance * vertical_basis.abs();
            assert!(error <= tolerance);
        }
        Ok(())
    }

    #[test]
    fn project_and_click_round_trip_plane_pixels_at_every_depth() -> Result<(), CameraError> {
        let golden_screen = Screen::new(4, 4)?;
        let golden_view = View::new(
            [Fixed::<GOLDEN_LIMBS>::ZERO; 2],
            Exponent::ZERO,
            Orientation::IDENTITY,
        );
        let golden_point = click(&golden_view, golden_screen, [1.0, -1.0])?;
        assert_eq!(
            golden_point.map(|coordinate| coordinate.to_le_bytes()),
            [[[0; 8], [1, 0, 0, 0, 0, 0, 0, 0]], [[0; 8], [255; 8]],]
        );
        let golden_projection = project(&golden_view, golden_screen, &golden_point)?
            .ok_or(CameraError::ScreenCoordinateOutOfRange)?;
        assert_eq!(
            golden_projection.map(f64::to_bits),
            [0x3ff0_0000_0000_0000, 0xbff0_0000_0000_0000]
        );

        let screen = Screen::new(1_024, 512)?;
        let rotated_orientation = all_plane_orientation()?;
        let review_frame = review_orientation()?;
        let maximum_pixel =
            Fixed::<ROUND_TRIP_LIMBS>::from_i64(MAX_SCREEN_COORDINATE_PIXELS)?.to_f64()?;
        assert_eq!(maximum_pixel.to_bits(), PROJECT_RANGE_PIXELS.to_bits());
        assert!(PROJECT_READOUT_LIMIT_PIXELS > maximum_pixel);
        for quanta in crate::MIN_EXPONENT_QUANTA..=crate::MAX_EXPONENT_QUANTA {
            let view = View::new(
                [Fixed::<ROUND_TRIP_LIMBS>::ZERO; ROUND_TRIP_DIMENSIONS],
                Exponent::new(quanta)?,
                Orientation::IDENTITY,
            );
            let lattice_pixel = [128.0, -64.0];
            let lattice_point = click(&view, screen, lattice_pixel)?;
            let lattice_projection = project(&view, screen, &lattice_point)?
                .ok_or(CameraError::ScreenCoordinateOutOfRange)?;
            assert_eq!(
                [
                    lattice_projection[0].to_bits(),
                    lattice_projection[1].to_bits()
                ],
                [lattice_pixel[0].to_bits(), lattice_pixel[1].to_bits()]
            );
            assert_eq!(click(&view, screen, lattice_projection)?, lattice_point);

            assert_readout_round_trip(&view, screen, [173.25, -91.5])?;

            let rotated_view = View::new(
                [Fixed::<ROUND_TRIP_LIMBS>::ZERO; ROUND_TRIP_DIMENSIONS],
                Exponent::new(quanta)?,
                rotated_orientation,
            );
            assert_readout_round_trip(&rotated_view, screen, [2_000_000_000.0, -1_000_000_000.0])?;
            assert_readout_round_trip(&rotated_view, screen, [maximum_pixel, -maximum_pixel])?;

            if quanta == 0 {
                let review_view = View::new(
                    [Fixed::<ROUND_TRIP_LIMBS>::ZERO; ROUND_TRIP_DIMENSIONS],
                    Exponent::ZERO,
                    review_frame,
                );
                assert_readout_round_trip(
                    &review_view,
                    screen,
                    [PROJECT_RANGE_PIXELS, -PROJECT_RANGE_PIXELS],
                )?;
            }
        }
        Ok(())
    }

    #[test]
    fn reference_displacement_tracks_pan_pixels() -> Result<(), CameraError> {
        let screen = Screen::new(960, 540)?;
        let mut view = View::new(
            [Fixed::<8>::ZERO; 5],
            Exponent::new(40 * EXPONENT_QUANTA_PER_OCTAVE)?,
            Orientation::IDENTITY,
        );
        let reference = view.centre;
        pan(&mut view, screen, 13.0, -9.0)?;
        let displacement = reference_displacement(&view, &reference, screen)?;
        assert!((displacement[0] + 13.0).abs() <= PROJECT_PIXEL_TOLERANCE_PIXELS);
        assert!((displacement[1] - 9.0).abs() <= PROJECT_PIXEL_TOLERANCE_PIXELS);
        Ok(())
    }

    #[test]
    fn reference_displacement_is_not_a_pointer_coordinate() -> Result<(), CameraError> {
        let screen = Screen::new(960, 540)?;
        let reference = [Fixed::<8>::ZERO; 5];
        let mut centre = reference;
        centre[0] = Fixed::from_i64(1)?;
        let view = View::new(
            centre,
            Exponent::new(24 * EXPONENT_QUANTA_PER_OCTAVE)?,
            Orientation::IDENTITY,
        );
        let displacement = reference_displacement(&view, &reference, screen)?;
        assert_eq!(displacement, [4_026_531_840.0, 0.0]);
        assert!(displacement[0] > PROJECT_READOUT_LIMIT_PIXELS);
        Ok(())
    }

    #[test]
    fn projection_refuses_points_outside_the_named_screen_range() -> Result<(), CameraError> {
        let screen = Screen::new(4, 4)?;
        let view = View::new([Fixed::<8>::ZERO; 5], Exponent::ZERO, Orientation::IDENTITY);
        let point = [
            Fixed::from_i64(i64::MAX)?,
            Fixed::ZERO,
            Fixed::ZERO,
            Fixed::ZERO,
            Fixed::ZERO,
        ];
        assert_eq!(project(&view, screen, &point)?, None);
        Ok(())
    }

    #[test]
    fn zero_tilt_perspective_inversion_is_identity() -> Result<(), CameraError> {
        let screen = Screen::new(1_920, 1_080)?;
        let observer = Observer::new(0.0, 0.0, [0.0; 5], 8.0)?;
        let input = [317.25, -144.5];
        let output =
            invert_perspective(&observer, screen, input).ok_or(CameraError::InvalidPerspective)?;
        assert!((output[0] - input[0]).abs() <= f64::EPSILON * input[0].abs());
        assert!((output[1] - input[1]).abs() <= f64::EPSILON * input[1].abs());
        Ok(())
    }

    #[test]
    fn model_translation_is_inverted_before_the_base_plane_intersection() -> Result<(), CameraError>
    {
        let screen = Screen::new(1_024, 512)?;
        let observer = Observer::new(0.0, 0.0, [0.2, 0.0, 0.0, 0.0], 8.0)?;
        let output = invert_perspective(&observer, screen, [137.0, 0.0])
            .ok_or(CameraError::InvalidPerspective)?;
        assert!((output[0] - 85.8).abs() <= 2.0 * f64::EPSILON * 85.8);
        assert_eq!(output[1], 0.0);
        Ok(())
    }

    #[test]
    fn simple_observer_round_trips_its_documented_transform() -> Result<(), CameraError> {
        let screen = Screen::new(1_024, 576)?;
        for observer in [
            Observer::new(0.17, -0.12, [0.2, -0.1, 0.3, 0.0], 7.0)?,
            Observer::new(-0.31, 0.23, [-0.15, 0.09, -0.2, 0.0], 11.0)?,
        ] {
            for base_point in [[0.0; 2], [91.5, -37.25], [-311.0, 123.0]] {
                let screen_point = project_perspective(&observer, screen, base_point)
                    .ok_or(CameraError::InvalidPerspective)?;
                let restored = invert_perspective(&observer, screen, screen_point)
                    .ok_or(CameraError::InvalidPerspective)?;
                assert!(
                    (restored[0] - base_point[0]).abs() <= PERSPECTIVE_ROUND_TRIP_TOLERANCE_PIXELS
                );
                assert!(
                    (restored[1] - base_point[1]).abs() <= PERSPECTIVE_ROUND_TRIP_TOLERANCE_PIXELS
                );
            }
        }
        Ok(())
    }

    #[test]
    fn simple_observer_rejects_unmodelled_higher_translation() {
        assert_eq!(
            Observer::new(0.0, 0.0, [0.0, 0.0, 0.0, 0.1], 8.0),
            Err(CameraError::InvalidPerspective)
        );
    }

    #[test]
    fn two_stage_observer_inverts_its_complete_projection() -> Result<(), CameraError> {
        let screen = Screen::new(1_024, 576)?;
        let projection = TwoStageProjection {
            image_plane: [[0.0, 0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0, 0.0]],
            frame_angles: [0.13, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -0.21, 0.0],
            translation: [0.2, -0.1, 0.3, -0.2, 0.15],
            yaw: 0.17,
            pitch: -0.12,
            distance_five: 7.0,
            distance_four: 8.0,
        };
        let observer = Observer::two_stage(projection)?;
        let plane_pixel = [91.5, -37.25];
        let screen_pixel = project_perspective(&observer, screen, plane_pixel)
            .ok_or(CameraError::InvalidPerspective)?;
        let restored = invert_perspective(&observer, screen, screen_pixel)
            .ok_or(CameraError::InvalidPerspective)?;
        assert!((restored[0] - plane_pixel[0]).abs() <= PERSPECTIVE_ROUND_TRIP_TOLERANCE_PIXELS);
        assert!((restored[1] - plane_pixel[1]).abs() <= PERSPECTIVE_ROUND_TRIP_TOLERANCE_PIXELS);
        Ok(())
    }
}
