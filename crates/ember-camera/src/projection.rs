use crate::basis::turn_sin_cos;
use crate::{CameraError, Fixed, Observer, Screen, Turn, View, rebuild_basis, scale_for};

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
/// Projection necessarily rounds to a binary64 pixel coordinate. Three representable steps at
/// that coordinate, scaled by the view's pixel scale, cover the exhaustive all-plane orientation
/// sweep at every exponent quantum; the uncertainty does not grow with the exponent.
pub const PROJECT_READOUT_ULPS: u64 = 3;

/// Largest measured pixel error in the exhaustive rotated projection sweep.
///
/// This is three binary64 steps immediately below [`PROJECT_RANGE_PIXELS`]. It replaces the
/// narrower provisional bound that the maximum-coordinate cases disproved.
pub const PROJECT_PIXEL_TOLERANCE_PIXELS: f64 = 7.152_557_373_046_875e-7;

/// Outer projection-readout and exact-decoder range in centred pixels.
///
/// The nominal range plus [`PROJECT_READOUT_ULPS`] upward binary64 steps ensures that
/// `click(project(point))` remains accepted when a boundary readout rounds just outward.
pub const PROJECT_READOUT_LIMIT_PIXELS: f64 =
    f64::from_bits(PROJECT_RANGE_BITS + PROJECT_READOUT_ULPS);

/// Denominator magnitude treated as a perspective ray parallel to the base plane.
const PERSPECTIVE_RAY_EPSILON: f64 = 1.0e-12;

/// Projects an N-dimensional point into centred render-grid pixels.
///
/// The function performs N exact big subtractions first. Each result is then converted once to a
/// screen-scale binary64 displacement. Two displacement dots are solved through the rebuilt
/// basis's two-by-two Gram matrix, compensating its bounded polynomial approximation error before
/// the named range check. Orthogonal components are discarded, so `click(project(point))` is an
/// inverse only for points on the image plane.
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
/// This per-frame renderer quantity performs N exact subtractions, truncates only at the binary64
/// screen boundary, and solves its two basis dots through the rebuilt frame's Gram matrix.
///
/// # Errors
///
/// Returns a typed refusal for invalid geometry, an out-of-range result, or checked arithmetic
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
    if projected
        .iter()
        .all(|component| component.is_finite() && component.abs() <= PROJECT_READOUT_LIMIT_PIXELS)
    {
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
    if N < 3
        || !observer.yaw.is_finite()
        || !observer.pitch.is_finite()
        || !observer.perspective.is_finite()
        || observer.perspective <= 0.0
        || !observer
            .translation
            .iter()
            .all(|component| component.is_finite())
        || !screen_px.iter().all(|component| component.is_finite())
    {
        return None;
    }
    let units_per_pixel = 4.0 / f64::from(screen.width());
    let horizontal = screen_px[0] * units_per_pixel;
    let vertical = screen_px[1] * units_per_pixel;
    let (yaw_sine, yaw_cosine) = turn_sin_cos(Turn::from_radians(observer.yaw).ok()?);
    let (pitch_sine, pitch_cosine) = turn_sin_cos(Turn::from_radians(observer.pitch).ok()?);
    let yaw_horizontal = yaw_cosine * horizontal - yaw_sine * observer.perspective;
    let yaw_depth = -yaw_sine * horizontal - yaw_cosine * observer.perspective;
    let pitched_vertical = pitch_cosine * vertical - pitch_sine * yaw_depth;
    let pitched_depth = pitch_sine * vertical + pitch_cosine * yaw_depth;
    if !pitched_depth.is_finite() || pitched_depth >= -PERSPECTIVE_RAY_EPSILON {
        return None;
    }
    let origin_depth = observer.translation[2] + observer.perspective;
    let distance = -origin_depth / pitched_depth;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let base_horizontal = observer.translation[0] + distance * yaw_horizontal;
    let base_vertical = observer.translation[1] + distance * pitched_vertical;
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

fn project_delta<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    screen: Screen,
    delta: &[Fixed<LIMBS>; N],
) -> Result<[f64; 2], CameraError> {
    let basis = rebuild_basis(&view.orientation)?;
    let scale = scale_for::<LIMBS>(view.exponent, screen)?
        .units_per_pixel()
        .to_f64()?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(CameraError::Overflow);
    }
    let mut horizontal_dot = 0.0;
    let mut vertical_dot = 0.0;
    let mut horizontal_norm_squared = 0.0;
    let mut cross_dot = 0.0;
    let mut vertical_norm_squared = 0.0;
    for ((coordinate, horizontal_basis), vertical_basis) in delta.iter().zip(basis.u).zip(basis.v) {
        let screen_coordinate = coordinate.to_f64()? / scale;
        horizontal_dot += screen_coordinate * horizontal_basis;
        vertical_dot += screen_coordinate * vertical_basis;
        horizontal_norm_squared += horizontal_basis * horizontal_basis;
        cross_dot += horizontal_basis * vertical_basis;
        vertical_norm_squared += vertical_basis * vertical_basis;
    }
    let cross_squared = cross_dot * cross_dot;
    let determinant = horizontal_norm_squared * vertical_norm_squared - cross_squared;
    if !determinant.is_finite() || determinant <= 0.0 {
        return Err(CameraError::DegenerateFrame);
    }
    let horizontal =
        (horizontal_dot * vertical_norm_squared - vertical_dot * cross_dot) / determinant;
    let vertical =
        (vertical_dot * horizontal_norm_squared - horizontal_dot * cross_dot) / determinant;
    Ok([horizontal, vertical])
}

#[cfg(test)]
mod tests {
    use super::{
        PROJECT_PIXEL_TOLERANCE_PIXELS, PROJECT_RANGE_PIXELS, PROJECT_READOUT_LIMIT_PIXELS,
        PROJECT_READOUT_ULPS, invert_perspective, project, reference_displacement,
    };
    use crate::{
        CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, MAX_SCREEN_COORDINATE_PIXELS,
        Observer, Orientation, Screen, Turn, View, click, pan, rebuild_basis, scale_for,
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
}
