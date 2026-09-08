use crate::frame::orientation_for_segment;
use crate::{
    Basis, CameraError, Exponent, Fixed, MAX_EXPONENT_QUANTA, MIN_EXPONENT_QUANTA, Orientation,
    Scale, Screen, View, rebuild_basis, scale_for,
};

/// Largest magnitude accepted at the floating-point pixel input boundary.
///
/// A signed 31-bit render coordinate covers every `u32` screen about its centre while leaving one
/// sign bit for exact differences and midpoints.
pub const MAX_SCREEN_COORDINATE_PIXELS: i64 = 2_147_483_647;

/// Maximum relative box-edge slack introduced by one exponent quantum.
///
/// This is `2^(1/1024) - 1`: selecting the deepest containing exponent can leave an edge at most
/// this fraction inward from the continuous, unquantised fit.
pub const EXPONENT_QUANTUM_EDGE_TOLERANCE: f64 = 0.000_677_130_693_066_407_8;

/// Converts one centred render-grid pixel into its exact N-dimensional plane point.
///
/// Pixel origin is the canvas centre, x points right, and y points up. Binary64 inputs are decoded
/// by their bits, then all screen scaling, basis weighting, and centre additions use [`Fixed`].
/// The basis and scale are rebuilt from the view so callers cannot provide a second source of truth.
///
/// # Errors
///
/// Returns a typed refusal for invalid geometry, an out-of-range pixel, or checked arithmetic
/// overflow.
pub fn click<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    screen: Screen,
    screen_px: [f64; 2],
) -> Result<[Fixed<LIMBS>; N], CameraError> {
    let screen_px = fixed_screen_point(screen_px)?;
    click_fixed(view, screen, &screen_px)
}

/// Translates a view opposite a drag displacement in centred render-grid pixels.
///
/// A positive horizontal delta drags the picture right and therefore moves the stored centre left
/// along `u`; a positive vertical delta follows the same rule upward along `v`. The view changes
/// only after every checked coordinate update succeeds.
///
/// # Errors
///
/// Returns a typed refusal for invalid geometry, input, scale, basis, or arithmetic overflow.
pub fn pan<const N: usize, const LIMBS: usize>(
    view: &mut View<N, LIMBS>,
    screen: Screen,
    horizontal_delta: f64,
    vertical_delta: f64,
) -> Result<(), CameraError> {
    let pixels = fixed_screen_point([horizontal_delta, vertical_delta])?;
    let offset = screen_offset(view, screen, &pixels)?;
    let mut centre = view.centre;
    for (coordinate, movement) in centre.iter_mut().zip(offset) {
        *coordinate = coordinate.sub(&movement)?;
    }
    view.centre = centre;
    Ok(())
}

/// Changes the integer exponent while preserving the point beneath a pixel anchor.
///
/// The old and new anchor offsets are both rebuilt from exact state. Their difference is applied
/// only after all coordinates succeed, so a refused edit leaves the view byte-identical.
///
/// # Errors
///
/// Returns a typed refusal for an exponent outside the named range, invalid geometry or input, or
/// checked arithmetic overflow.
pub fn zoom_about<const N: usize, const LIMBS: usize>(
    view: &mut View<N, LIMBS>,
    screen: Screen,
    anchor_px: [f64; 2],
    delta_quanta: i32,
) -> Result<(), CameraError> {
    let anchor = fixed_screen_point(anchor_px)?;
    let old_offset = screen_offset(view, screen, &anchor)?;
    let mut next = *view;
    next.exponent = view.exponent.checked_add(delta_quanta)?;
    let new_offset = screen_offset(&next, screen, &anchor)?;
    for ((output, old), new) in next.centre.iter_mut().zip(old_offset).zip(new_offset) {
        let movement = old.sub(&new)?;
        *output = output.add(&movement)?;
    }
    *view = next;
    Ok(())
}

/// Adds integer plane-angle deltas while preserving the point beneath a pixel anchor.
///
/// The full basis is rebuilt before and after the angular change; only its binary64 component bits
/// cross into fixed precision. No floating-point arithmetic is used to update the centre.
///
/// # Errors
///
/// Returns a typed refusal for invalid geometry or input, or checked arithmetic overflow.
pub fn rotate_about<const N: usize, const LIMBS: usize>(
    view: &mut View<N, LIMBS>,
    screen: Screen,
    anchor_px: [f64; 2],
    angle_deltas: &Orientation<N>,
) -> Result<(), CameraError> {
    let anchor = fixed_screen_point(anchor_px)?;
    let old_offset = screen_offset(view, screen, &anchor)?;
    let mut next = *view;
    next.orientation = view.orientation.with_deltas(angle_deltas);
    let new_offset = screen_offset(&next, screen, &anchor)?;
    for ((output, old), new) in next.centre.iter_mut().zip(old_offset).zip(new_offset) {
        let movement = old.sub(&new)?;
        *output = output.add(&movement)?;
    }
    *view = next;
    Ok(())
}

/// Frames two exact space points in a deterministically rebuilt image plane.
///
/// The centre is their fixed-point midpoint, dropping one lowest bit toward negative infinity when
/// the sum is odd. The new `u` follows the segment, the previous `v` is made orthonormal to it, and
/// the deepest exponent whose width and height still contain both basis separations is selected.
///
/// # Errors
///
/// Returns a typed refusal for coincident points, an unavailable frame, or checked arithmetic
/// overflow. A refusal leaves the view unchanged.
pub fn frame_points<const N: usize, const LIMBS: usize>(
    view: &mut View<N, LIMBS>,
    point_one: &[Fixed<LIMBS>; N],
    point_two: &[Fixed<LIMBS>; N],
    screen: Screen,
) -> Result<(), CameraError> {
    let mut segment = [Fixed::ZERO; N];
    for ((output, second), first) in segment.iter_mut().zip(point_two).zip(point_one) {
        *output = second.sub(first)?;
    }
    if segment.iter().all(Fixed::is_zero) {
        return Err(CameraError::EmptySelection);
    }
    let orientation = orientation_for_segment(view, &segment)?;
    let basis = rebuild_basis(&orientation)?;
    let horizontal_span = axis_separation(&segment, &basis.u)?.abs_checked()?;
    let vertical_span = axis_separation(&segment, &basis.v)?.abs_checked()?;
    let exponent = deepest_fitting_exponent(screen, &horizontal_span, &vertical_span)?;
    let mut centre = [Fixed::ZERO; N];
    for ((output, first), second) in centre.iter_mut().zip(point_one).zip(point_two) {
        *output = first.midpoint_floor(second)?;
    }
    *view = View::new(centre, exponent, orientation);
    Ok(())
}

/// Centres and quantises a render-grid selection box into the current image plane.
///
/// The selected box is fitted without cropping: the limiting width or height chooses the deepest
/// exponent whose quantised scale still contains both edges. Screen midpoints use fixed arithmetic,
/// so odd lowest bits are dropped toward negative infinity.
///
/// # Errors
///
/// Returns a typed refusal for a zero-area box, invalid input, or checked arithmetic overflow. A
/// refusal leaves the view unchanged.
pub fn select_box<const N: usize, const LIMBS: usize>(
    view: &mut View<N, LIMBS>,
    first_corner_px: [f64; 2],
    second_corner_px: [f64; 2],
    screen: Screen,
) -> Result<(), CameraError> {
    let first_corner = fixed_screen_point(first_corner_px)?;
    let second_corner = fixed_screen_point(second_corner_px)?;
    let width_px = second_corner[0].sub(&first_corner[0])?.abs_checked()?;
    let height_px = second_corner[1].sub(&first_corner[1])?.abs_checked()?;
    if width_px.is_zero() || height_px.is_zero() {
        return Err(CameraError::EmptySelection);
    }
    let centre_px = [
        first_corner[0].midpoint_floor(&second_corner[0])?,
        first_corner[1].midpoint_floor(&second_corner[1])?,
    ];
    let centre = click_fixed(view, screen, &centre_px)?;
    let old_scale = scale_for::<LIMBS>(view.exponent, screen)?.units_per_pixel();
    let horizontal_span = old_scale.mul(&width_px)?;
    let vertical_span = old_scale.mul(&height_px)?;
    let exponent = deepest_fitting_exponent(screen, &horizontal_span, &vertical_span)?;
    let mut next = *view;
    next.centre = centre;
    next.exponent = exponent;
    *view = next;
    Ok(())
}

fn fixed_screen_point<const LIMBS: usize>(
    screen_px: [f64; 2],
) -> Result<[Fixed<LIMBS>; 2], CameraError> {
    let limit = Fixed::from_i64(MAX_SCREEN_COORDINATE_PIXELS)?;
    let mut converted = [Fixed::ZERO; 2];
    for (output, input) in converted.iter_mut().zip(screen_px) {
        *output = Fixed::from_f64(input)?;
        if output.abs_checked()? > limit {
            return Err(CameraError::ScreenCoordinateOutOfRange);
        }
    }
    Ok(converted)
}

fn click_fixed<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    screen: Screen,
    screen_px: &[Fixed<LIMBS>; 2],
) -> Result<[Fixed<LIMBS>; N], CameraError> {
    let offset = screen_offset(view, screen, screen_px)?;
    let mut point = view.centre;
    for (coordinate, movement) in point.iter_mut().zip(offset) {
        *coordinate = coordinate.add(&movement)?;
    }
    Ok(point)
}

fn screen_offset<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    screen: Screen,
    screen_px: &[Fixed<LIMBS>; 2],
) -> Result<[Fixed<LIMBS>; N], CameraError> {
    let basis = rebuild_basis(&view.orientation)?;
    let scale = scale_for::<LIMBS>(view.exponent, screen)?;
    offset_from_parts(&basis, scale, screen_px)
}

fn offset_from_parts<const N: usize, const LIMBS: usize>(
    basis: &Basis<N>,
    scale: Scale<LIMBS>,
    screen_px: &[Fixed<LIMBS>; 2],
) -> Result<[Fixed<LIMBS>; N], CameraError> {
    let units_per_pixel = scale.units_per_pixel();
    let horizontal = units_per_pixel.mul(&screen_px[0])?;
    let vertical = units_per_pixel.mul(&screen_px[1])?;
    let mut offset = [Fixed::ZERO; N];
    for ((output, horizontal_basis), vertical_basis) in offset.iter_mut().zip(basis.u).zip(basis.v)
    {
        let horizontal_component = horizontal.mul(&Fixed::from_f64(horizontal_basis)?)?;
        let vertical_component = vertical.mul(&Fixed::from_f64(vertical_basis)?)?;
        *output = horizontal_component.add(&vertical_component)?;
    }
    Ok(offset)
}

fn axis_separation<const N: usize, const LIMBS: usize>(
    segment: &[Fixed<LIMBS>; N],
    axis: &[f64; N],
) -> Result<Fixed<LIMBS>, CameraError> {
    let mut separation = Fixed::ZERO;
    for (coordinate, component) in segment.iter().zip(axis) {
        let weighted = coordinate.mul(&Fixed::from_f64(*component)?)?;
        separation = separation.add(&weighted)?;
    }
    Ok(separation)
}

fn deepest_fitting_exponent<const LIMBS: usize>(
    screen: Screen,
    horizontal_span: &Fixed<LIMBS>,
    vertical_span: &Fixed<LIMBS>,
) -> Result<Exponent, CameraError> {
    let mut lower = MIN_EXPONENT_QUANTA;
    let mut upper = MAX_EXPONENT_QUANTA;
    let mut best = None;
    while lower <= upper {
        let middle = lower.midpoint(upper);
        let candidate = Exponent::new(middle)?;
        let scale = scale_for::<LIMBS>(candidate, screen)?.units_per_pixel();
        let available_width = scale.mul_small(i64::from(screen.width()))?;
        let available_height = scale.mul_small(i64::from(screen.height()))?;
        if available_width >= *horizontal_span && available_height >= *vertical_span {
            best = Some(candidate);
            lower = middle + 1;
        } else {
            upper = middle - 1;
        }
    }
    best.ok_or(CameraError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::{click, frame_points, pan, rotate_about, select_box, zoom_about};
    use crate::{
        CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, MAX_EXPONENT_QUANTA,
        MIN_EXPONENT_QUANTA, Orientation, Screen, Turn, View, project,
    };

    /// Projection noise budget, below one thousandth of a render pixel.
    const PIXEL_TOLERANCE: f64 = 3.0e-7;

    fn view() -> View<5, 8> {
        View::new([Fixed::ZERO; 5], Exponent::ZERO, Orientation::IDENTITY)
    }

    #[test]
    fn pan_and_its_inverse_restore_every_bit() -> Result<(), CameraError> {
        let screen = Screen::new(1_024, 512)?;
        let mut camera = view();
        let original = camera;
        pan(&mut camera, screen, 17.25, -9.5)?;
        pan(&mut camera, screen, -17.25, 9.5)?;
        assert_eq!(camera, original);
        Ok(())
    }

    #[test]
    fn extreme_integer_edit_round_trip_matches_direct_zoom() -> Result<(), CameraError> {
        let screen = Screen::new(1_920, 1_080)?;
        let anchor = [173.25, -91.5];
        let mut angles = [[Turn::ZERO; 5]; 5];
        angles[0][4] = Turn::from_bits(0x1020_3040);
        angles[2][3] = Turn::from_bits(0x5060_7080);
        let rotation = Orientation::new(angles)?;
        let mut direct = view();
        zoom_about(&mut direct, screen, anchor, MAX_EXPONENT_QUANTA)?;
        let mut journey = view();
        zoom_about(&mut journey, screen, anchor, MAX_EXPONENT_QUANTA)?;
        rotate_about(&mut journey, screen, anchor, &rotation)?;
        zoom_about(
            &mut journey,
            screen,
            anchor,
            MIN_EXPONENT_QUANTA - MAX_EXPONENT_QUANTA,
        )?;
        rotate_about(&mut journey, screen, anchor, &rotation.inverse())?;
        zoom_about(
            &mut journey,
            screen,
            anchor,
            MAX_EXPONENT_QUANTA - MIN_EXPONENT_QUANTA,
        )?;
        assert_eq!(journey, direct);
        let projected = project(&journey, screen, &click(&direct, screen, anchor)?)?
            .ok_or(CameraError::ScreenCoordinateOutOfRange)?;
        assert!((projected[0] - anchor[0]).abs() <= PIXEL_TOLERANCE);
        assert!((projected[1] - anchor[1]).abs() <= PIXEL_TOLERANCE);
        Ok(())
    }

    #[test]
    fn fractional_round_trip_restores_every_bit() -> Result<(), CameraError> {
        let screen = Screen::new(960, 540)?;
        let anchor = [117.5, -33.25];
        let fractional = 37 * EXPONENT_QUANTA_PER_OCTAVE + 511;
        let mut direct = view();
        zoom_about(&mut direct, screen, anchor, fractional)?;
        let mut journey = view();
        zoom_about(&mut journey, screen, anchor, fractional)?;
        zoom_about(&mut journey, screen, anchor, -fractional)?;
        zoom_about(&mut journey, screen, anchor, fractional)?;
        assert_eq!(journey.exponent, direct.exponent);
        assert_eq!(journey.orientation, direct.orientation);
        assert_eq!(journey.centre, direct.centre);
        Ok(())
    }

    #[test]
    fn refused_exponent_edit_leaves_the_view_unchanged() -> Result<(), CameraError> {
        let screen = Screen::new(960, 540)?;
        let mut camera = View::new(
            [Fixed::<8>::ZERO; 5],
            Exponent::new(MAX_EXPONENT_QUANTA)?,
            Orientation::IDENTITY,
        );
        let original = camera;
        assert_eq!(
            zoom_about(&mut camera, screen, [17.0, -9.0], 1),
            Err(CameraError::ExponentOutOfRange)
        );
        assert_eq!(camera, original);
        Ok(())
    }

    #[test]
    fn frame_points_is_reproducible() -> Result<(), CameraError> {
        let screen = Screen::new(960, 540)?;
        let point_one = [
            Fixed::from_i64(-2)?,
            Fixed::from_i64(1)?,
            Fixed::from_i64(0)?,
            Fixed::from_i64(3)?,
            Fixed::from_i64(-1)?,
        ];
        let point_two = [
            Fixed::from_i64(4)?,
            Fixed::from_i64(-2)?,
            Fixed::from_i64(5)?,
            Fixed::from_i64(3)?,
            Fixed::from_i64(2)?,
        ];
        let mut first = view();
        let mut second = first;
        frame_points(&mut first, &point_one, &point_two, screen)?;
        frame_points(&mut second, &point_one, &point_two, screen)?;
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn select_box_centres_and_contains_every_edge() -> Result<(), CameraError> {
        let screen = Screen::new(1_000, 500)?;
        let mut camera = view();
        let selected_left = click(&camera, screen, [-250.0, -100.0])?;
        let selected_right = click(&camera, screen, [250.0, 100.0])?;
        select_box(&mut camera, [-250.0, -100.0], [250.0, 100.0], screen)?;
        assert_eq!(camera.centre, [Fixed::ZERO; 5]);
        let projected_left = project(&camera, screen, &selected_left)?
            .ok_or(CameraError::ScreenCoordinateOutOfRange)?;
        let projected_right = project(&camera, screen, &selected_right)?
            .ok_or(CameraError::ScreenCoordinateOutOfRange)?;
        let half_width = f64::from(screen.width()) / 2.0;
        let edge_tolerance = half_width * super::EXPONENT_QUANTUM_EDGE_TOLERANCE + PIXEL_TOLERANCE;
        assert!(projected_left[0] >= -half_width - PIXEL_TOLERANCE);
        assert!(projected_right[0] <= half_width + PIXEL_TOLERANCE);
        assert!((projected_left[0].abs() - half_width).abs() <= edge_tolerance);
        assert!((projected_right[0].abs() - half_width).abs() <= edge_tolerance);
        Ok(())
    }
}
