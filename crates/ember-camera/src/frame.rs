use crate::{Basis, CameraError, Fixed, Orientation, Turn, View, rebuild_basis};

/// One fixed CORDIC angle per nonzero 64-bit binary turn step.
///
/// Entry `i` is `atan(2^-i)` rounded to the nearest `u64` turn. The extra 32 fraction bits are
/// retained through the complete vectoring and rounded once, ties to even, into [`Turn`].
const CORDIC_ATAN_TURNS: [u64; 63] = [
    0x2000_0000_0000_0000,
    0x12e4_051d_9df3_0866,
    0x09fb_385b_5ee3_9e8e,
    0x0511_11d4_1ddd_9a1b,
    0x028b_0d43_0e58_9aed,
    0x0145_d7e1_5904_6278,
    0x00a2_f61e_5c28_262a,
    0x0051_7c55_11d4_42af,
    0x0028_be53_46d0_c337,
    0x0014_5f2e_bb30_ab38,
    0x000a_2f98_0091_ba7b,
    0x0005_17cc_14a8_0cb7,
    0x0002_8be6_0cdf_ec62,
    0x0001_45f3_06c1_72f2,
    0x0000_a2f9_836a_e911,
    0x0000_517c_c1b6_ba7c,
    0x0000_28be_60db_85fc,
    0x0000_145f_306d_c816,
    0x0000_0a2f_9836_e4ae,
    0x0000_0517_cc1b_726b,
    0x0000_028b_e60d_b938,
    0x0000_0145_f306_dc9c,
    0x0000_00a2_f983_6e4e,
    0x0000_0051_7cc1_b727,
    0x0000_0028_be60_db94,
    0x0000_0014_5f30_6dca,
    0x0000_000a_2f98_36e5,
    0x0000_0005_17cc_1b72,
    0x0000_0002_8be6_0db9,
    0x0000_0001_45f3_06dd,
    0x0000_0000_a2f9_836e,
    0x0000_0000_517c_c1b7,
    0x0000_0000_28be_60dc,
    0x0000_0000_145f_306e,
    0x0000_0000_0a2f_9837,
    0x0000_0000_0517_cc1b,
    0x0000_0000_028b_e60e,
    0x0000_0000_0145_f307,
    0x0000_0000_00a2_f983,
    0x0000_0000_0051_7cc2,
    0x0000_0000_0028_be61,
    0x0000_0000_0014_5f30,
    0x0000_0000_000a_2f98,
    0x0000_0000_0005_17cc,
    0x0000_0000_0002_8be6,
    0x0000_0000_0001_45f3,
    0x0000_0000_0000_a2fa,
    0x0000_0000_0000_517d,
    0x0000_0000_0000_28be,
    0x0000_0000_0000_145f,
    0x0000_0000_0000_0a30,
    0x0000_0000_0000_0518,
    0x0000_0000_0000_028c,
    0x0000_0000_0000_0146,
    0x0000_0000_0000_00a3,
    0x0000_0000_0000_0051,
    0x0000_0000_0000_0029,
    0x0000_0000_0000_0014,
    0x0000_0000_0000_000a,
    0x0000_0000_0000_0005,
    0x0000_0000_0000_0003,
    0x0000_0000_0000_0001,
    0x0000_0000_0000_0001,
];

/// Exact binary64 bits of the 63-step CORDIC inverse gain.
const CORDIC_INVERSE_GAIN_BITS: u64 = 0x3fe3_6e9d_b508_6bcc;

const QUARTER_TURN: Turn = Turn::from_bits(1_u32 << (u32::BITS - 2));
const HALF_TURN: Turn = Turn::from_bits(1_u32 << (u32::BITS - 1));
const HALF_WIDE_TURN: u64 = 1_u64 << (u64::BITS - 1);
/// Internal vectoring width, independent of the exact centre's consumer-selected width.
const FRAME_EXTRACTION_LIMBS: usize = 3;

/// Dot-product and determinant tolerance for a complete binary64 frame.
///
/// `2e-10` covers accumulated binary64 products at ordinary camera dimensions while remaining
/// below one `Turn` step in the rebuilt presentation.
const FRAME_ORTHONORMAL_TOLERANCE: f64 = 2.0e-10;
/// Complete-frame comparison tolerance after deterministic basis reconstruction.
///
/// `2e-8` covers two turn quanta plus the fixed polynomial basis error without accepting a
/// malformed axis.
const FRAME_REBUILD_TOLERANCE: f64 = 2.0e-8;
const PLANE_RELATION_LIMBS: usize = 3;
const PLANE_RELATION_TOLERANCE: f64 = 8.0e-8;

/// Quantises a complete floating-point frame into the canonical integer orientation.
///
/// Each supplied basis bit pattern enters an internal fixed precision once, independent of any
/// consumer's centre width. Successive fixed-point CORDIC vectorings then recover the row-major
/// Givens factors without a platform transcendental call. This constructor is for
/// application-boundary frame conventions; exact navigation continues to rebuild its basis from
/// the returned integer record.
///
/// # Errors
///
/// Returns a typed refusal for a frame smaller than two dimensions, a non-finite component, a
/// degenerate axis, or checked fixed-point arithmetic failure.
pub fn orientation_from_frame<const N: usize>(
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
    validate_frame(frame)?;
    let mut angles = [[Turn::ZERO; N]; N];
    for (first, candidate) in frame.iter().enumerate().take(N.saturating_sub(1)) {
        let partial = Orientation::new(angles)?;
        let basis = rebuild_basis(&partial)?;
        let coefficients =
            projected_coefficients::<N, FRAME_EXTRACTION_LIMBS>(candidate, &basis, first)?;
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
    let orientation = Orientation::new(angles)?;
    let rebuilt = rebuild_basis(&orientation)?;
    for (actual, expected) in rebuilt_frame(&rebuilt).iter().zip(frame) {
        if actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| (*actual - *expected).abs() > FRAME_REBUILD_TOLERANCE)
        {
            return Err(CameraError::DegenerateFrame);
        }
    }
    Ok(orientation)
}

/// Reports whether two exact orientations select the same unoriented image plane.
///
/// Each deterministically rebuilt basis enters internal fixed precision once. The comparison then
/// uses the Plucker coordinates of the two image axes, accepting either common sign so a reflected
/// chart still names the same plane. The tolerance covers two turn quanta and the documented basis
/// rebuild bound; it affects only the plane-identity decision and never enters a view update.
///
/// # Errors
///
/// Returns a typed refusal when the dimension cannot contain an image plane or fixed arithmetic
/// overflows.
pub fn same_image_plane<const N: usize>(
    first: &Orientation<N>,
    second: &Orientation<N>,
) -> Result<bool, CameraError> {
    let first_basis = rebuild_basis(first)?;
    let second_basis = rebuild_basis(second)?;
    let first_u = fixed_axis(&first_basis.u)?;
    let first_v = fixed_axis(&first_basis.v)?;
    let second_u = fixed_axis(&second_basis.u)?;
    let second_v = fixed_axis(&second_basis.v)?;
    let tolerance = Fixed::from_f64(PLANE_RELATION_TOLERANCE)?;

    let mut pivot = None;
    let mut pivot_magnitude = Fixed::ZERO;
    for (first_axis, _) in first_u.iter().enumerate() {
        for (second_axis, _) in first_u.iter().enumerate().skip(first_axis + 1) {
            let coordinate = wedge_coordinate(&first_u, &first_v, first_axis, second_axis)?;
            let magnitude = coordinate.abs_checked()?;
            if magnitude > pivot_magnitude {
                pivot = Some((first_axis, second_axis, coordinate));
                pivot_magnitude = magnitude;
            }
        }
    }
    let Some((first_axis, second_axis, first_pivot)) = pivot else {
        return Err(CameraError::DimensionTooSmall);
    };
    let second_pivot = wedge_coordinate(&second_u, &second_v, first_axis, second_axis)?;
    if second_pivot.abs_checked()? <= tolerance {
        return Ok(false);
    }
    let reflected = first_pivot.is_negative() != second_pivot.is_negative();
    for (first_axis, _) in first_u.iter().enumerate() {
        for (second_axis, _) in first_u.iter().enumerate().skip(first_axis + 1) {
            let first_coordinate = wedge_coordinate(&first_u, &first_v, first_axis, second_axis)?;
            let second_coordinate =
                wedge_coordinate(&second_u, &second_v, first_axis, second_axis)?;
            let difference = if reflected {
                first_coordinate.add(&second_coordinate)?
            } else {
                first_coordinate.sub(&second_coordinate)?
            };
            if difference.abs_checked()? > tolerance {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn fixed_axis<const N: usize>(
    axis: &[f64; N],
) -> Result<[Fixed<PLANE_RELATION_LIMBS>; N], CameraError> {
    let mut fixed = [Fixed::ZERO; N];
    for (output, component) in fixed.iter_mut().zip(axis) {
        *output = Fixed::from_f64(*component)?;
    }
    Ok(fixed)
}

fn wedge_coordinate<const N: usize>(
    u: &[Fixed<PLANE_RELATION_LIMBS>; N],
    v: &[Fixed<PLANE_RELATION_LIMBS>; N],
    first: usize,
    second: usize,
) -> Result<Fixed<PLANE_RELATION_LIMBS>, CameraError> {
    u[first].mul(&v[second])?.sub(&u[second].mul(&v[first])?)
}

fn validate_frame<const N: usize>(frame: &[[f64; N]; N]) -> Result<(), CameraError> {
    for (row_index, row) in frame.iter().enumerate() {
        let norm = dot(row, row);
        if (norm - 1.0).abs() > FRAME_ORTHONORMAL_TOLERANCE {
            return Err(CameraError::DegenerateFrame);
        }
        for previous in &frame[..row_index] {
            if dot(row, previous).abs() > FRAME_ORTHONORMAL_TOLERANCE {
                return Err(CameraError::DegenerateFrame);
            }
        }
    }
    if (determinant(frame) - 1.0).abs() > FRAME_ORTHONORMAL_TOLERANCE {
        return Err(CameraError::DegenerateFrame);
    }
    Ok(())
}

fn dot<const N: usize>(left: &[f64; N], right: &[f64; N]) -> f64 {
    // The fixed 2e-10 frame tolerance covers separately rounded binary64 products and sums by
    // many orders of magnitude for the unit camera frames accepted here, so fusion is immaterial.
    left.iter()
        .zip(right)
        .fold(0.0, |sum, (left, right)| (*left * *right) + sum)
}

fn determinant<const N: usize>(frame: &[[f64; N]; N]) -> f64 {
    let mut matrix = *frame;
    let mut result = 1.0;
    let mut column = 0;
    while column < N {
        let Some((pivot_row, _)) = matrix
            .iter()
            .enumerate()
            .skip(column)
            .max_by(|(_, left), (_, right)| left[column].abs().total_cmp(&right[column].abs()))
        else {
            return 0.0;
        };
        if matrix[pivot_row][column].abs() <= FRAME_ORTHONORMAL_TOLERANCE {
            return 0.0;
        }
        if pivot_row != column {
            matrix.swap(pivot_row, column);
            result = -result;
        }
        let pivot = matrix[column][column];
        result *= pivot;
        let pivot_row = matrix[column];
        for row in matrix.iter_mut().skip(column + 1) {
            let factor = row[column] / pivot;
            for (entry, pivot_entry) in row
                .iter_mut()
                .skip(column + 1)
                .zip(pivot_row.iter().skip(column + 1))
            {
                // This elimination only classifies a frame against the same 2e-10 tolerance;
                // separate core multiplication and addition cannot affect that classification.
                *entry -= factor * *pivot_entry;
            }
        }
        column += 1;
    }
    result
}

fn rebuilt_frame<const N: usize>(basis: &Basis<N>) -> [[f64; N]; N] {
    core::array::from_fn(|index| basis.vector(index).copied().unwrap_or([0.0; N]))
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
        HALF_WIDE_TURN
    } else {
        0
    };
    for (shift, angle_bits) in CORDIC_ATAN_TURNS.iter().copied().enumerate() {
        let horizontal_shift = horizontal_work.shift_right(shift)?;
        let vertical_shift = vertical_work.shift_right(shift)?;
        if vertical_work.is_negative() {
            horizontal_work = horizontal_work.sub(&vertical_shift)?;
            vertical_work = vertical_work.add(&horizontal_shift)?;
            angle = angle.wrapping_sub(angle_bits);
        } else {
            horizontal_work = horizontal_work.add(&vertical_shift)?;
            vertical_work = vertical_work.sub(&horizontal_shift)?;
            angle = angle.wrapping_add(angle_bits);
        }
    }
    let inverse_gain = Fixed::from_binary64_bits(CORDIC_INVERSE_GAIN_BITS)?;
    Ok((horizontal_work.mul(&inverse_gain)?, turn_from_wide(angle)))
}

fn turn_from_wide(angle: u64) -> Turn {
    let retained = angle >> u32::BITS;
    let discarded = angle & u64::from(u32::MAX);
    let halfway = 1_u64 << (u32::BITS - 1);
    let increment = discarded > halfway || (discarded == halfway && retained & 1 != 0);
    let rounded = retained.wrapping_add(u64::from(increment)) & u64::from(u32::MAX);
    let [first, second, third, fourth, _, _, _, _] = rounded.to_le_bytes();
    Turn::from_bits(u32::from_le_bytes([first, second, third, fourth]))
}

#[cfg(test)]
mod tests {
    use super::{QUARTER_TURN, orientation_for_segment, orientation_from_frame, same_image_plane};
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
        let orientation = orientation_from_frame::<4>(&frame)?;
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
        let rebuilt = rebuild_basis(&orientation_from_frame::<4>(&frame)?)?;
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

    fn assert_first_turn_recovers_exactly(bits: u32) -> Result<(), CameraError> {
        let mut angles = [[Turn::ZERO; 4]; 4];
        angles[0][1] = Turn::from_bits(bits);
        let expected = Orientation::new(angles)?;
        let basis = rebuild_basis(&expected)?;
        let frame = [basis.u, basis.v, basis.remaining[2], basis.remaining[3]];
        assert_eq!(
            orientation_from_frame::<4>(&frame)?,
            expected,
            "turn bits {bits:#010x}"
        );
        Ok(())
    }

    #[test]
    fn frame_constructor_recovers_representative_turns_exactly() -> Result<(), CameraError> {
        for bits in [
            0,
            1,
            0x0826_135f,
            0x1fff_ffff,
            0x2000_0000,
            0x2000_0001,
            0x3fff_ffff,
            0x4000_0000,
            0x4000_0001,
            0x7fff_ffff,
            0x8000_0000,
            0x8000_0001,
            0xbfff_ffff,
            0xc000_0000,
            0xc000_0001,
            0xdfff_ffff,
            0xe000_0000,
            0xe000_0001,
            0xffff_ffff,
        ] {
            assert_first_turn_recovers_exactly(bits)?;
        }
        Ok(())
    }

    #[test]
    fn frame_constructor_recovers_randomised_turns_exactly() -> Result<(), CameraError> {
        let mut bits = 0x5eed_cafe_u32;
        for _ in 0..4_096 {
            bits = bits.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
            assert_first_turn_recovers_exactly(bits)?;
        }
        Ok(())
    }

    fn assert_malformed_frame_is_refused(frame: [[f64; 2]; 2]) {
        assert_eq!(
            orientation_from_frame::<2>(&frame),
            Err(CameraError::DegenerateFrame)
        );
    }

    #[test]
    fn frame_constructor_refuses_a_zero_final_axis() {
        assert_malformed_frame_is_refused([[1.0, 0.0], [0.0, 0.0]]);
    }

    #[test]
    fn frame_constructor_refuses_a_duplicated_axis() {
        assert_malformed_frame_is_refused([[1.0, 0.0], [1.0, 0.0]]);
    }

    #[test]
    fn frame_constructor_refuses_a_scaled_axis() {
        assert_malformed_frame_is_refused([[2.0, 0.0], [0.0, 1.0]]);
    }

    #[test]
    fn frame_constructor_refuses_a_reflected_frame() {
        assert_malformed_frame_is_refused([[1.0, 0.0], [0.0, -1.0]]);
    }

    #[test]
    fn exact_orientation_classifies_image_plane_changes() -> Result<(), CameraError> {
        let identity = Orientation::<4>::IDENTITY;
        let mut in_plane_angles = [[Turn::ZERO; 4]; 4];
        in_plane_angles[0][1] = Turn::from_bits(0x1357_9bdf);
        let in_plane = Orientation::new(in_plane_angles)?;
        assert!(same_image_plane(&identity, &in_plane)?);

        let mut complement_angles = [[Turn::ZERO; 4]; 4];
        complement_angles[2][3] = Turn::from_bits(0x2468_ace0);
        let complement = Orientation::new(complement_angles)?;
        assert!(same_image_plane(&identity, &complement)?);

        let mut tilted_angles = [[Turn::ZERO; 4]; 4];
        tilted_angles[0][2] = Turn::from_bits(0x1020_3040);
        let tilted = Orientation::new(tilted_angles)?;
        assert!(!same_image_plane(&identity, &tilted)?);
        Ok(())
    }
}
