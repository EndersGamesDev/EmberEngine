use crate::{Basis, CameraError, Fixed, Orientation, Turn, View, rebuild_basis};

/// One fixed CORDIC angle per representable binary turn bit.
///
/// Entry `i` is `atan(2^-i)` rounded to the nearest `u32` turn. 31 iterations exhaust the nonzero
/// rounded micro-rotation table; accumulated table rounding is covered by `ALIGNMENT_TOLERANCE`.
const CORDIC_ATAN_TURNS: [u32; 31] = [
    0x2000_0000,
    0x12e4_051e,
    0x09fb_385b,
    0x0511_11d4,
    0x028b_0d43,
    0x0145_d7e1,
    0x00a2_f61e,
    0x0051_7c55,
    0x0028_be53,
    0x0014_5f2f,
    0x000a_2f98,
    0x0005_17cc,
    0x0002_8be6,
    0x0001_45f3,
    0x0000_a2fa,
    0x0000_517d,
    0x0000_28be,
    0x0000_145f,
    0x0000_0a30,
    0x0000_0518,
    0x0000_028c,
    0x0000_0146,
    0x0000_00a3,
    0x0000_0051,
    0x0000_0029,
    0x0000_0014,
    0x0000_000a,
    0x0000_0005,
    0x0000_0003,
    0x0000_0001,
    0x0000_0001,
];

/// Exact binary64 bits of the 31-step CORDIC inverse gain.
const CORDIC_INVERSE_GAIN_BITS: u64 = 0x3fe3_6e9d_b508_6bcc;

const QUARTER_TURN: Turn = Turn::from_bits(1_u32 << (u32::BITS - 2));
const HALF_TURN: Turn = Turn::from_bits(1_u32 << (u32::BITS - 1));

/// Quantises a complete floating-point frame into the canonical integer orientation.
///
/// Each supplied basis bit pattern enters fixed precision once. Successive fixed-point CORDIC
/// vectorings then recover the row-major Givens factors without a platform transcendental call.
/// This constructor is for application-boundary frame conventions; exact navigation continues to
/// rebuild its basis from the returned integer record.
///
/// # Errors
///
/// Returns a typed refusal for a frame smaller than two dimensions, a non-finite component, a
/// degenerate axis, or checked fixed-point arithmetic failure.
pub fn orientation_from_frame<const N: usize, const LIMBS: usize>(
    frame: &[[f64; N]; N],
) -> Result<Orientation<N>, CameraError> {
    if N < 2 {
        return Err(CameraError::DimensionTooSmall);
    }
    if !frame
        .iter()
        .flatten()
        .all(|component| component.is_finite())
    {
        return Err(CameraError::NonFinite);
    }
    let mut angles = [[Turn::ZERO; N]; N];
    for (first, candidate) in frame.iter().enumerate().take(N.saturating_sub(1)) {
        let partial = Orientation::new(angles)?;
        let basis = rebuild_basis(&partial)?;
        let coefficients = projected_coefficients::<N, LIMBS>(candidate, &basis, first)?;
        let mut radius = coefficients[first];
        for (second, component) in coefficients.iter().enumerate().skip(first + 1) {
            let (next_radius, angle) = vectoring_turn(&radius, component)?;
            angles[first][second] = angle;
            radius = next_radius;
        }
        if radius.is_zero() {
            return Err(CameraError::DegenerateFrame);
        }
    }
    Orientation::new(angles)
}

/// Reconstructs an orientation whose first axis follows an exact segment.
///
/// # Errors
///
/// Returns an error when the segment cannot define an image-plane frame or arithmetic fails.
pub fn orientation_for_segment<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    segment: &[Fixed<LIMBS>; N],
) -> Result<Orientation<N>, CameraError> {
    if N < 2 {
        return Err(CameraError::DimensionTooSmall);
    }
    let mut angles = [[Turn::ZERO; N]; N];
    let mut radius = segment[0];
    for (second, component) in segment.iter().enumerate().skip(1) {
        let (next_radius, angle) = vectoring_turn(&radius, component)?;
        angles[0][second] = angle;
        radius = next_radius;
    }
    if radius.is_zero() {
        return Err(CameraError::DegenerateFrame);
    }
    if N == 2 {
        return Orientation::new(angles);
    }

    let partial_orientation = Orientation::new(angles)?;
    let previous_basis = rebuild_basis(&view.orientation)?;
    let partial_basis = rebuild_basis(&partial_orientation)?;
    let mut coefficients = projected_coefficients(&previous_basis.v, &partial_basis, 1)?;
    if coefficients.iter().skip(1).all(Fixed::is_zero) {
        coefficients = 'fallback: {
            for candidate_index in 2..N {
                let candidate = previous_basis
                    .vector(candidate_index)
                    .ok_or(CameraError::DegenerateFrame)?;
                let projected = projected_coefficients(candidate, &partial_basis, 1)?;
                if projected.iter().skip(1).any(|value| !value.is_zero()) {
                    break 'fallback projected;
                }
            }
            projected_coefficients(&previous_basis.u, &partial_basis, 1)?
        };
    }
    if coefficients.iter().skip(1).all(Fixed::is_zero) {
        return Err(CameraError::DegenerateFrame);
    }
    radius = coefficients[1];
    for (second, component) in coefficients.iter().enumerate().skip(2) {
        let (next_radius, angle) = vectoring_turn(&radius, component)?;
        angles[1][second] = angle;
        radius = next_radius;
    }
    Orientation::new(angles)
}

fn projected_coefficients<const N: usize, const LIMBS: usize>(
    candidate: &[f64; N],
    basis: &Basis<N>,
    first_axis: usize,
) -> Result<[Fixed<LIMBS>; N], CameraError> {
    let mut fixed_candidate = [Fixed::ZERO; N];
    for (output, component) in fixed_candidate.iter_mut().zip(candidate) {
        *output = Fixed::from_f64(*component)?;
    }
    let mut coefficients = [Fixed::ZERO; N];
    for (axis_index, coefficient) in coefficients.iter_mut().enumerate().skip(first_axis) {
        let axis = basis
            .vector(axis_index)
            .ok_or(CameraError::DegenerateFrame)?;
        for (candidate_component, axis_component) in fixed_candidate.iter().zip(axis) {
            let fixed_axis_component = Fixed::from_f64(*axis_component)?;
            *coefficient = coefficient.add(&candidate_component.mul(&fixed_axis_component)?)?;
        }
    }
    Ok(coefficients)
}

fn vectoring_turn<const LIMBS: usize>(
    horizontal: &Fixed<LIMBS>,
    vertical: &Fixed<LIMBS>,
) -> Result<(Fixed<LIMBS>, Turn), CameraError> {
    if vertical.is_zero() {
        return if horizontal.is_negative() {
            Ok((horizontal.neg()?, HALF_TURN))
        } else {
            Ok((*horizontal, Turn::ZERO))
        };
    }
    if horizontal.is_zero() {
        let radius = vertical.abs_checked()?;
        let angle = if vertical.is_negative() {
            QUARTER_TURN.inverse()
        } else {
            QUARTER_TURN
        };
        return Ok((radius, angle));
    }
    let mut horizontal_work = *horizontal;
    let mut vertical_work = *vertical;
    let mut angle = if horizontal_work.is_negative() {
        horizontal_work = horizontal_work.neg()?;
        vertical_work = vertical_work.neg()?;
        HALF_TURN
    } else {
        Turn::ZERO
    };
    for (shift, angle_bits) in CORDIC_ATAN_TURNS.iter().copied().enumerate() {
        let horizontal_shift = horizontal_work.shift_right(shift)?;
        let vertical_shift = vertical_work.shift_right(shift)?;
        let step = Turn::from_bits(angle_bits);
        if vertical_work.is_negative() {
            horizontal_work = horizontal_work.sub(&vertical_shift)?;
            vertical_work = vertical_work.add(&horizontal_shift)?;
            angle = angle.wrapping_add(step.inverse());
        } else {
            horizontal_work = horizontal_work.add(&vertical_shift)?;
            vertical_work = vertical_work.sub(&horizontal_shift)?;
            angle = angle.wrapping_add(step);
        }
    }
    let inverse_gain = Fixed::from_binary64_bits(CORDIC_INVERSE_GAIN_BITS)?;
    Ok((horizontal_work.mul(&inverse_gain)?, angle))
}

#[cfg(test)]
mod tests {
    use super::{QUARTER_TURN, orientation_for_segment, orientation_from_frame};
    use crate::{CameraError, Exponent, Fixed, Orientation, Turn, View, rebuild_basis};

    /// Two Turn quanta plus deterministic CORDIC and basis-polynomial error.
    const ALIGNMENT_TOLERANCE: f64 = 2.0e-8;

    #[test]
    fn segment_orientation_aligns_u_and_keeps_v_orthogonal() -> Result<(), CameraError> {
        let view = View::new([Fixed::<8>::ZERO; 5], Exponent::ZERO, Orientation::IDENTITY);
        let segment = [
            Fixed::from_i64(2)?,
            Fixed::from_i64(-3)?,
            Fixed::from_i64(5)?,
            Fixed::from_i64(1)?,
            Fixed::from_i64(-2)?,
        ];
        let orientation = orientation_for_segment(&view, &segment)?;
        let basis = rebuild_basis(&orientation)?;
        let segment_f64 = [2.0, -3.0, 5.0, 1.0, -2.0];
        let length = (43.0_f64).sqrt();
        for (actual, expected) in basis.u.iter().zip(segment_f64) {
            assert!((*actual - expected / length).abs() <= ALIGNMENT_TOLERANCE);
        }
        let orthogonality: f64 = basis
            .u
            .iter()
            .zip(basis.v)
            .map(|(horizontal, vertical)| horizontal * vertical)
            .sum();
        assert!(orthogonality.abs() <= ALIGNMENT_TOLERANCE);
        Ok(())
    }

    #[test]
    fn frame_constructor_preserves_exact_coordinate_axes() -> Result<(), CameraError> {
        let frame = [
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
        ];
        let orientation = orientation_from_frame::<4, 8>(&frame)?;
        assert_eq!(orientation.angle(0, 1), Some(Turn::ZERO));
        assert_eq!(orientation.angle(0, 2), Some(QUARTER_TURN));
        assert_eq!(orientation.angle(0, 3), Some(Turn::ZERO));
        assert_eq!(orientation.angle(1, 2), Some(Turn::ZERO));
        assert_eq!(orientation.angle(1, 3), Some(QUARTER_TURN));
        assert_eq!(orientation.angle(2, 3), Some(Turn::ZERO));
        let basis = rebuild_basis(&orientation)?;
        assert_eq!(basis.u, frame[0]);
        assert_eq!(basis.v, frame[1]);
        assert_eq!(basis.remaining[2], frame[2]);
        assert_eq!(basis.remaining[3], frame[3]);
        Ok(())
    }

    #[test]
    fn frame_constructor_rebuilds_a_general_reordered_frame() -> Result<(), CameraError> {
        let mut angles = [[Turn::ZERO; 4]; 4];
        angles[0][1] = Turn::from_bits(0x0826_135f);
        angles[0][2] = Turn::from_bits(0xefb3_d942);
        angles[0][3] = Turn::from_bits(0x0413_09b0);
        angles[1][2] = Turn::from_bits(0x0a2f_9837);
        angles[1][3] = Turn::from_bits(0xf3c6_e2f1);
        angles[2][3] = Turn::from_bits(0x061c_8e87);
        let source = rebuild_basis(&Orientation::new(angles)?)?;
        let frame = [
            source.remaining[2],
            source.remaining[3],
            source.u.map(|component| -component),
            source.v.map(|component| -component),
        ];
        let rebuilt = rebuild_basis(&orientation_from_frame::<4, 8>(&frame)?)?;
        let actual = [
            rebuilt.u,
            rebuilt.v,
            rebuilt.remaining[2],
            rebuilt.remaining[3],
        ];
        for (actual_axis, expected_axis) in actual.iter().zip(frame) {
            for (actual, expected) in actual_axis.iter().zip(expected_axis) {
                assert!((*actual - expected).abs() <= ALIGNMENT_TOLERANCE);
            }
        }
        Ok(())
    }
}
