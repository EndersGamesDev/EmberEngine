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

const HALF_TURN: Turn = Turn::from_bits(1_u32 << (u32::BITS - 1));

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
    let mut coefficients = projected_coefficients(&previous_basis.v, &partial_basis)?;
    if coefficients.iter().skip(1).all(Fixed::is_zero) {
        coefficients = 'fallback: {
            for candidate_index in 2..N {
                let candidate = previous_basis
                    .vector(candidate_index)
                    .ok_or(CameraError::DegenerateFrame)?;
                let projected = projected_coefficients(candidate, &partial_basis)?;
                if projected.iter().skip(1).any(|value| !value.is_zero()) {
                    break 'fallback projected;
                }
            }
            projected_coefficients(&previous_basis.u, &partial_basis)?
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
) -> Result<[Fixed<LIMBS>; N], CameraError> {
    let mut fixed_candidate = [Fixed::ZERO; N];
    for (output, component) in fixed_candidate.iter_mut().zip(candidate) {
        *output = Fixed::from_f64(*component)?;
    }
    let mut coefficients = [Fixed::ZERO; N];
    for (axis_index, coefficient) in coefficients.iter_mut().enumerate().skip(1) {
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
    use super::orientation_for_segment;
    use crate::{CameraError, Exponent, Fixed, Orientation, View, rebuild_basis};

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
}
