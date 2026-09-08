use crate::basis::turn_sin_cos;
use crate::{CameraError, Fixed, Observer, Screen, Turn, View, rebuild_basis, scale_for};

/// Symmetric pixel range in which projection keeps every intermediate screen-sized.
///
/// Two billion pixels exceeds any `u32` render-grid radius while ensuring dot-product accumulation
/// remains far from binary64 overflow. [`project`] returns `None` outside this range.
pub const PROJECT_RANGE_PIXELS: f64 = 2_147_483_648.0;

/// Binary64 readout budget for reconstructing an arbitrary image-plane point.
///
/// Projection necessarily rounds to a binary64 pixel coordinate. One representable step at that
/// coordinate, scaled by the view's pixel scale, is the unavoidable plane-space uncertainty; it
/// does not grow with the exponent.
pub const PROJECT_READOUT_ULPS: u64 = 1;

/// Denominator magnitude treated as a perspective ray parallel to the base plane.
const PERSPECTIVE_RAY_EPSILON: f64 = 1.0e-12;

/// Projects an N-dimensional point into centred render-grid pixels.
///
/// The function performs N exact big subtractions first. Each result is then converted once to a
/// screen-scale binary64 displacement, followed by two small dot products against the rebuilt
/// basis. Points outside the named screen range return `None`. Orthogonal components are discarded,
/// so `click(project(point))` is an inverse only for points on the image plane.
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
        .all(|component| component.is_finite() && component.abs() <= PROJECT_RANGE_PIXELS)
    {
        Ok(Some(projected))
    } else {
        Ok(None)
    }
}

/// Returns the current centre's render-grid displacement from a reference centre.
///
/// This per-frame renderer quantity performs N exact subtractions, truncates only at the binary64
/// screen boundary, and then performs the two small basis dot products.
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
        .all(|component| component.is_finite() && component.abs() <= PROJECT_RANGE_PIXELS)
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
    let mut horizontal = 0.0;
    let mut vertical = 0.0;
    for ((coordinate, horizontal_basis), vertical_basis) in delta.iter().zip(basis.u).zip(basis.v) {
        let screen_coordinate = coordinate.to_f64()? / scale;
        horizontal += screen_coordinate * horizontal_basis;
        vertical += screen_coordinate * vertical_basis;
    }
    Ok([horizontal, vertical])
}

#[cfg(test)]
mod tests {
    use super::{PROJECT_READOUT_ULPS, invert_perspective, project, reference_displacement};
    use crate::{
        CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, Observer, Orientation, Screen,
        View, click, pan, scale_for,
    };

    /// Projection noise budget, below one thousandth of a render pixel.
    const PIXEL_ROUND_TRIP_TOLERANCE: f64 = 3.0e-7;

    /// Converts the one-ulp binary64 pixel budget to plane units at the current scale.
    const fn readout_plane_tolerance(pixel: f64, units_per_pixel: f64) -> f64 {
        let magnitude_bits = pixel.to_bits() & (u64::MAX >> 1);
        let magnitude = f64::from_bits(magnitude_bits);
        let next = f64::from_bits(magnitude_bits + PROJECT_READOUT_ULPS);
        (next - magnitude) * units_per_pixel
    }

    #[test]
    fn project_and_click_round_trip_plane_pixels_at_every_depth() -> Result<(), CameraError> {
        let golden_screen = Screen::new(4, 4)?;
        let golden_view = View::new([Fixed::<2>::ZERO; 2], Exponent::ZERO, Orientation::IDENTITY);
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
        for quanta in crate::MIN_EXPONENT_QUANTA..=crate::MAX_EXPONENT_QUANTA {
            let view = View::new(
                [Fixed::<8>::ZERO; 5],
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

            let arbitrary_pixel = [173.25, -91.5];
            let arbitrary_point = click(&view, screen, arbitrary_pixel)?;
            let arbitrary_projection = project(&view, screen, &arbitrary_point)?
                .ok_or(CameraError::ScreenCoordinateOutOfRange)?;
            assert!(
                (arbitrary_projection[0] - arbitrary_pixel[0]).abs() <= PIXEL_ROUND_TRIP_TOLERANCE
            );
            assert!(
                (arbitrary_projection[1] - arbitrary_pixel[1]).abs() <= PIXEL_ROUND_TRIP_TOLERANCE
            );
            let arbitrary_rebuilt = click(&view, screen, arbitrary_projection)?;
            let units_per_pixel = scale_for::<8>(view.exponent, screen)?
                .units_per_pixel()
                .to_f64()?;
            let horizontal_error = arbitrary_rebuilt[0]
                .sub(&arbitrary_point[0])?
                .abs_checked()?
                .to_f64()?;
            let vertical_error = arbitrary_rebuilt[1]
                .sub(&arbitrary_point[1])?
                .abs_checked()?
                .to_f64()?;
            assert!(
                horizontal_error
                    <= readout_plane_tolerance(arbitrary_projection[0], units_per_pixel)
            );
            assert!(
                vertical_error <= readout_plane_tolerance(arbitrary_projection[1], units_per_pixel)
            );
            assert_eq!(&arbitrary_rebuilt[2..], &arbitrary_point[2..]);
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
        assert!((displacement[0] + 13.0).abs() <= PIXEL_ROUND_TRIP_TOLERANCE);
        assert!((displacement[1] - 9.0).abs() <= PIXEL_ROUND_TRIP_TOLERANCE);
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
