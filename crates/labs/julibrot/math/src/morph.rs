use crate::big::rounded_astro_precision;
use crate::{BigCentre, BigScalar, MathError, PlaneAngles, ViewControls};

/// Bits the morph itself needs beyond the deeper of the two endpoints it runs between.
///
/// A slider that resolves a few thousand steps needs only a handful of bits below the endpoints to
/// keep consecutive steps distinguishable; sixty-four is one Astro-float word, so asking for it
/// costs one allocation step and buys far more headroom than any control can spend.
pub const MORPH_EXTRA_BITS: u32 = 64;

/// Returns the working precision a morph between two centres runs at.
///
/// # Errors
///
/// Returns an error when the deeper endpoint plus the morph's own bits overflows.
pub fn morph_precision_bits(from: &BigCentre, to: &BigCentre) -> Result<u32, MathError> {
    from.precision_bits
        .max(to.precision_bits)
        .checked_add(MORPH_EXTRA_BITS)
        .ok_or(MathError::CounterOverflow)
}

/// Interpolates one finite scalar into another at `t`.
///
/// The form is `(1−t)a + tb` rather than `a + t(b−a)` because only the first reproduces both ends
/// exactly: at `t=1` the second leaves `a + (b−a)`, which is `b` only when that difference happens
/// to be representable.
///
/// # Errors
///
/// Returns an error for a non-finite endpoint, a `t` outside `[0,1]`, or a non-finite result.
pub fn lerp_f64(from: f64, to: f64, t: f64) -> Result<f64, MathError> {
    validate_fraction(t)?;
    if !from.is_finite() || !to.is_finite() {
        return Err(MathError::NonFinite);
    }
    let value = t.mul_add(to, (1.0 - t) * from);
    value
        .is_finite()
        .then_some(value)
        .ok_or(MathError::NonFinite)
}

/// Interpolates every VIEW control linearly on its own value.
///
/// Both VIEW angles and both camera angles interpolate on the number the slider shows, without
/// shortest-arc rewrapping: the sliders span `[−π,π]`, and a rewrap would send the picture the
/// opposite way round the circle from the way the handle travels.
///
/// # Errors
///
/// Returns an error for a non-finite control, a `t` outside `[0,1]`, a non-finite result, or a row
/// the view controls themselves reject.
pub fn lerp_view(from: ViewControls, to: ViewControls, t: f64) -> Result<ViewControls, MathError> {
    if !from.is_valid() || !to.is_valid() {
        return Err(MathError::InvalidViewControls);
    }
    let view = ViewControls {
        theta_1: lerp_f64(from.theta_1, to.theta_1, t)?,
        theta_2: lerp_f64(from.theta_2, to.theta_2, t)?,
        camera_yaw: lerp_f64(from.camera_yaw, to.camera_yaw, t)?,
        camera_pitch: lerp_f64(from.camera_pitch, to.camera_pitch, t)?,
        height_scale: lerp_f64(from.height_scale, to.height_scale, t)?,
        distance_five: lerp_f64(from.distance_five, to.distance_five, t)?,
        distance_four: lerp_f64(from.distance_four, to.distance_four, t)?,
    };
    view.is_valid()
        .then_some(view)
        .ok_or(MathError::InvalidViewControls)
}

/// Interpolates both plane angles linearly, on the same argument as the VIEW angles.
///
/// # Errors
///
/// Returns an error for a non-finite angle, a `t` outside `[0,1]`, or a non-finite result.
pub fn lerp_plane_angles(
    from: PlaneAngles,
    to: PlaneAngles,
    t: f64,
) -> Result<PlaneAngles, MathError> {
    Ok(PlaneAngles {
        theta_1: lerp_f64(from.theta_1, to.theta_1, t)?,
        theta_2: lerp_f64(from.theta_2, to.theta_2, t)?,
    })
}

/// Interpolates the four plane-origin coordinates.
///
/// # Errors
///
/// Returns an error for a non-finite coordinate, a `t` outside `[0,1]`, or a non-finite result.
pub fn lerp_origin(from: [f64; 4], to: [f64; 4], t: f64) -> Result<[f64; 4], MathError> {
    let mut origin = [0.0; 4];
    for ((coordinate, first), second) in origin.iter_mut().zip(from).zip(to) {
        *coordinate = lerp_f64(first, second, t)?;
    }
    Ok(origin)
}

/// Interpolates the authoritative centre at the morph's working precision.
///
/// The arithmetic is the same `(1−t)a + tb` as the scalar case, evaluated in Astro-float so that a
/// pair of centres separated far below binary64 still moves step by step rather than snapping.
///
/// # Errors
///
/// Returns an error for a zero or unrepresentable precision, a `t` outside `[0,1]`, or Astro-float
/// arithmetic failure.
pub fn lerp_centre(
    from: &BigCentre,
    to: &BigCentre,
    t: f64,
    precision_bits: u32,
) -> Result<BigCentre, MathError> {
    validate_fraction(t)?;
    let precision_bits = rounded_astro_precision(precision_bits)?;
    let weight_to = BigScalar::from_f64(t, precision_bits)?;
    let weight_from = BigScalar::from_f64(1.0 - t, precision_bits)?;
    let mut coords = from.coords.clone();
    for ((coordinate, first), second) in coords.iter_mut().zip(&from.coords).zip(&to.coords) {
        *coordinate = first
            .mul(&weight_from, precision_bits)?
            .add(&second.mul(&weight_to, precision_bits)?, precision_bits)?;
    }
    Ok(BigCentre {
        coords,
        precision_bits,
    })
}

fn validate_fraction(t: f64) -> Result<(), MathError> {
    if !t.is_finite() || !(0.0..=1.0).contains(&t) {
        return Err(MathError::NonFinite);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_big_scalar, encode_big_scalar};

    const DEEP_BITS: u32 = 512;

    fn row_a() -> ViewControls {
        ViewControls {
            theta_1: -3.0,
            theta_2: 0.25,
            camera_yaw: -1.5,
            camera_pitch: 0.125,
            height_scale: 0.0,
            distance_five: 8.0,
            distance_four: 8.0,
        }
    }

    fn row_b() -> ViewControls {
        ViewControls {
            theta_1: 3.0,
            theta_2: -0.75,
            camera_yaw: 0.5,
            camera_pitch: -0.375,
            height_scale: 4.0,
            distance_five: 20.0,
            distance_four: 64.0,
        }
    }

    fn deep_pair() -> (BigCentre, BigCentre) {
        let from = BigCentre::from_f64([1.0 / 3.0, -0.1, 0.25, -2.5], DEEP_BITS)
            .expect("finite deep centre");
        let step = BigScalar::from_f64(2.0_f64.powi(-200), DEEP_BITS).expect("finite deep step");
        let mut to = from.clone();
        for coordinate in &mut to.coords {
            *coordinate = coordinate
                .add(&step, DEEP_BITS)
                .expect("deep centres add without overflow");
        }
        (from, to)
    }

    /// Both ends of the slider must be the rows the boxes hold, bit for bit.
    #[test]
    fn every_endpoint_reproduces_its_own_row_exactly() {
        assert_eq!(lerp_view(row_a(), row_b(), 0.0), Ok(row_a()));
        assert_eq!(lerp_view(row_a(), row_b(), 1.0), Ok(row_b()));
        let angles_a = PlaneAngles {
            theta_1: -2.5,
            theta_2: 0.75,
        };
        let angles_b = PlaneAngles {
            theta_1: 1.25,
            theta_2: -3.0,
        };
        assert_eq!(lerp_plane_angles(angles_a, angles_b, 0.0), Ok(angles_a));
        assert_eq!(lerp_plane_angles(angles_a, angles_b, 1.0), Ok(angles_b));
        let origin_a = [0.1, -0.2, 0.3, -0.4];
        let origin_b = [-0.8, 0.156, 1.0, -1.0];
        assert_eq!(lerp_origin(origin_a, origin_b, 0.0), Ok(origin_a));
        assert_eq!(lerp_origin(origin_a, origin_b, 1.0), Ok(origin_b));
        assert_eq!(lerp_f64(0.1, 0.7, 0.0), Ok(0.1));
        assert_eq!(lerp_f64(0.1, 0.7, 1.0), Ok(0.7));
    }

    /// An angle pair spanning most of the circle must cross zero, not wrap the short way.
    #[test]
    fn angles_interpolate_on_the_number_the_slider_shows() {
        let midpoint = lerp_view(row_a(), row_b(), 0.5).expect("valid interpolated row");
        assert!((midpoint.theta_1 - 0.0).abs() < 1.0e-15);
        assert!((midpoint.camera_yaw - (-0.5)).abs() < 1.0e-15);
        let quarter = lerp_f64(-3.0, 3.0, 0.25).expect("finite quarter");
        assert!((quarter - (-1.5)).abs() < 1.0e-15);
    }

    /// Linear on the exponent is a geometric zoom morph, so the midpoint of 10 and 30 is 20.
    #[test]
    fn the_zoom_exponent_interpolates_geometrically() {
        assert_eq!(lerp_f64(10.0, 30.0, 0.5), Ok(20.0));
        assert_eq!(lerp_f64(-2.0, 120.0, 0.5), Ok(59.0));
    }

    /// Every scalar of the row must land on its own midpoint at `t=0.5`.
    #[test]
    fn the_midpoint_row_is_the_midpoint_of_every_control() {
        let midpoint = lerp_view(row_a(), row_b(), 0.5).expect("valid interpolated row");
        let (first, second) = (row_a(), row_b());
        for (value, ends) in [
            (midpoint.theta_2, (first.theta_2, second.theta_2)),
            (
                midpoint.camera_pitch,
                (first.camera_pitch, second.camera_pitch),
            ),
            (
                midpoint.height_scale,
                (first.height_scale, second.height_scale),
            ),
            (
                midpoint.distance_five,
                (first.distance_five, second.distance_five),
            ),
            (
                midpoint.distance_four,
                (first.distance_four, second.distance_four),
            ),
        ] {
            assert!((value - f64::midpoint(ends.0, ends.1)).abs() < 1.0e-15);
        }
    }

    /// The working precision is the deeper endpoint plus the morph's own bits.
    #[test]
    fn the_morph_runs_deeper_than_either_endpoint() {
        let (from, to) = deep_pair();
        let bits = morph_precision_bits(&from, &to).expect("representable morph precision");
        assert_eq!(bits, DEEP_BITS + MORPH_EXTRA_BITS);
        let morphed = lerp_centre(&from, &to, 0.5, bits).expect("finite deep midpoint");
        assert_eq!(morphed.precision_bits, bits);
    }

    /// A deep centre pair must return its ends exactly and split the gap exactly in half.
    #[test]
    fn a_deep_centre_keeps_its_ends_and_halves_its_gap() {
        let (from, to) = deep_pair();
        let bits = morph_precision_bits(&from, &to).expect("representable morph precision");
        let start = lerp_centre(&from, &to, 0.0, bits).expect("finite deep start");
        let end = lerp_centre(&from, &to, 1.0, bits).expect("finite deep end");
        let middle = lerp_centre(&from, &to, 0.5, bits).expect("finite deep midpoint");
        for index in 0..4 {
            assert_eq!(
                start.coords[index]
                    .compare(&from.coords[index])
                    .expect("comparable deep coordinate"),
                0
            );
            assert_eq!(
                end.coords[index]
                    .compare(&to.coords[index])
                    .expect("comparable deep coordinate"),
                0
            );
            let below = middle.coords[index]
                .sub(&from.coords[index], bits)
                .expect("finite lower half");
            let above = to.coords[index]
                .sub(&middle.coords[index], bits)
                .expect("finite upper half");
            assert_eq!(
                below.compare(&above).expect("comparable deep halves"),
                0,
                "coordinate {index} is not the midpoint"
            );
            assert!(!below.is_zero(), "coordinate {index} collapsed to its end");
        }
    }

    /// A morphed centre must survive the encoding a saved view is stored in, exactly.
    #[test]
    fn a_morphed_centre_round_trips_through_its_canonical_encoding() {
        let (from, to) = deep_pair();
        let bits = morph_precision_bits(&from, &to).expect("representable morph precision");
        let morphed = lerp_centre(&from, &to, 0.375, bits).expect("finite deep interpolation");
        for coordinate in &morphed.coords {
            let encoded = encode_big_scalar(coordinate).expect("encodable deep coordinate");
            let decoded = decode_big_scalar(encoded.sign, encoded.exponent, &encoded.limbs, bits)
                .expect("decodable deep coordinate");
            assert_eq!(
                decoded
                    .compare(coordinate)
                    .expect("comparable deep coordinate"),
                0
            );
        }
    }

    /// A slider reading A to B promises nothing outside its own ends, and refuses to guess.
    #[test]
    fn a_fraction_outside_the_slider_or_a_non_finite_row_is_refused() {
        assert_eq!(lerp_f64(0.0, 1.0, -0.001), Err(MathError::NonFinite));
        assert_eq!(lerp_f64(0.0, 1.0, 1.001), Err(MathError::NonFinite));
        assert_eq!(lerp_f64(0.0, 1.0, f64::NAN), Err(MathError::NonFinite));
        assert_eq!(lerp_f64(f64::INFINITY, 1.0, 0.5), Err(MathError::NonFinite));
        let invalid = ViewControls {
            distance_four: 0.0,
            ..row_b()
        };
        assert_eq!(
            lerp_view(row_a(), invalid, 0.5),
            Err(MathError::InvalidViewControls)
        );
        let (from, to) = deep_pair();
        assert_eq!(
            lerp_centre(&from, &to, 2.0, 512).err(),
            Some(MathError::NonFinite)
        );
        assert_eq!(
            lerp_centre(&from, &to, 0.5, 0).err(),
            Some(MathError::CounterOverflow)
        );
    }
}
