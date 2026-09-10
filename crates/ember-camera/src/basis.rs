use crate::{Basis, CameraError, Orientation, Turn};

const QUADRANT_BITS: u32 = 30;
const QUADRANT_MASK: u32 = (1_u32 << QUADRANT_BITS) - 1;
const QUADRANT_RADIANS_PER_BIT: f64 = core::f64::consts::FRAC_PI_2 / 1_073_741_824.0;

const SINE_COEFFICIENTS: [f64; 11] = [
    1.0 / 51_090_942_171_709_440_000.0, // x^21 / 21! term.
    -1.0 / 121_645_100_408_832_000.0,   // -x^19 / 19! term.
    1.0 / 355_687_428_096_000.0,        // x^17 / 17! term.
    -1.0 / 1_307_674_368_000.0,         // -x^15 / 15! term.
    1.0 / 6_227_020_800.0,              // x^13 / 13! term.
    -1.0 / 39_916_800.0,                // -x^11 / 11! term.
    1.0 / 362_880.0,                    // x^9 / 9! term.
    -1.0 / 5_040.0,                     // -x^7 / 7! term.
    1.0 / 120.0,                        // x^5 / 5! term.
    -1.0 / 6.0,                         // -x^3 / 3! term.
    1.0,                                // x / 1! term.
];

const COSINE_COEFFICIENTS: [f64; 11] = [
    1.0 / 2_432_902_008_176_640_000.0, // x^20 / 20! term.
    -1.0 / 6_402_373_705_728_000.0,    // -x^18 / 18! term.
    1.0 / 20_922_789_888_000.0,        // x^16 / 16! term.
    -1.0 / 87_178_291_200.0,           // -x^14 / 14! term.
    1.0 / 479_001_600.0,               // x^12 / 12! term.
    -1.0 / 3_628_800.0,                // -x^10 / 10! term.
    1.0 / 40_320.0,                    // x^8 / 8! term.
    -1.0 / 720.0,                      // -x^6 / 6! term.
    1.0 / 24.0,                        // x^4 / 4! term.
    -1.0 / 2.0,                        // -x^2 / 2! term.
    1.0,                               // 1 / 0! term.
];

/// Rebuilds the complete frame from integer turns in canonical product order.
///
/// The polynomial sine and cosine use coefficient arrays ordered from highest term to constant;
/// Horner evaluation follows that array order, fixing the operation sequence without a platform
/// math-library call. Rebuilding the same orientation therefore returns the same binary64 bits;
/// no frame is accumulated from a previous rebuild.
///
/// # Errors
///
/// Returns [`CameraError::DimensionTooSmall`] when `N` cannot contain an image plane.
pub fn rebuild_basis<const N: usize>(
    orientation: &Orientation<N>,
) -> Result<Basis<N>, CameraError> {
    if N < 2 {
        return Err(CameraError::DimensionTooSmall);
    }
    let mut frame = [[0.0; N]; N];
    for (index, vector) in frame.iter_mut().enumerate() {
        vector[index] = 1.0;
    }
    for (first, row) in orientation.angles().iter().enumerate() {
        for (second, angle) in row.iter().copied().enumerate().skip(first + 1) {
            rotate_rows(&mut frame, first, second, angle);
        }
    }
    let mut remaining = [[0.0; N]; N];
    for (index, vector) in frame.iter().copied().enumerate().skip(2) {
        remaining[index] = vector;
    }
    Ok(Basis {
        u: frame[0],
        v: frame[1],
        remaining,
    })
}

fn rotate_rows<const N: usize>(
    frame: &mut [[f64; N]; N],
    first: usize,
    second: usize,
    angle: Turn,
) {
    let (sine, cosine) = turn_sin_cos(angle);
    let left = frame[first];
    let right = frame[second];
    let mut rotated_left = [0.0; N];
    let mut rotated_right = [0.0; N];
    for (((left_output, right_output), left_input), right_input) in rotated_left
        .iter_mut()
        .zip(&mut rotated_right)
        .zip(left)
        .zip(right)
    {
        *left_output = cosine * left_input + sine * right_input;
        *right_output = cosine * right_input - sine * left_input;
    }
    frame[first] = rotated_left;
    frame[second] = rotated_right;
}

fn first_quadrant_sin_cos(angle: f64) -> (f64, f64) {
    let squared = angle * angle;
    let mut sine_polynomial = 0.0;
    for coefficient in SINE_COEFFICIENTS {
        sine_polynomial = sine_polynomial * squared + coefficient;
    }
    let sine = angle * sine_polynomial;
    let mut cosine = 0.0;
    for coefficient in COSINE_COEFFICIENTS {
        cosine = cosine * squared + coefficient;
    }
    (sine, cosine)
}

pub fn turn_sin_cos(turn: Turn) -> (f64, f64) {
    let quadrant = turn.bits() >> QUADRANT_BITS;
    let offset = turn.bits() & QUADRANT_MASK;
    let angle = f64::from(offset) * QUADRANT_RADIANS_PER_BIT;
    let (sine, cosine) = first_quadrant_sin_cos(angle);
    match quadrant {
        0 => (sine, cosine),
        1 => (cosine, -sine),
        2 => (-sine, -cosine),
        _ => (-cosine, sine),
    }
}

/// Evaluates a binary64 angle without relying on a platform math library.
///
/// Quadrant reduction keeps the fixed Taylor polynomials on `[0, pi / 2]`. This is a
/// presentation-only boundary utility: its approximation and the separate core arithmetic in the
/// observer transform are covered by that transform's named binary64 pixel tolerance.
pub fn radian_sin_cos(radians: f64) -> (f64, f64) {
    let remainder = radians % core::f64::consts::TAU;
    let reduced = if remainder < 0.0 {
        remainder + core::f64::consts::TAU
    } else {
        remainder
    };
    if reduced <= core::f64::consts::FRAC_PI_2 {
        first_quadrant_sin_cos(reduced)
    } else if reduced <= core::f64::consts::PI {
        let (sine, cosine) = first_quadrant_sin_cos(core::f64::consts::PI - reduced);
        (sine, -cosine)
    } else if reduced <= 3.0 * core::f64::consts::FRAC_PI_2 {
        let (sine, cosine) = first_quadrant_sin_cos(reduced - core::f64::consts::PI);
        (-sine, -cosine)
    } else {
        let (sine, cosine) = first_quadrant_sin_cos(core::f64::consts::TAU - reduced);
        (-sine, cosine)
    }
}

#[cfg(test)]
mod tests {
    use super::rebuild_basis;
    use crate::{CameraError, Orientation, Turn};

    const ORTHONORMAL_TOLERANCE: f64 = 2.0e-12;

    fn dot<const N: usize>(left: &[f64; N], right: &[f64; N]) -> f64 {
        left.iter()
            .zip(right)
            .map(|(left_component, right_component)| left_component * right_component)
            .sum()
    }

    #[test]
    fn rebuild_is_bit_deterministic_and_orthonormal() -> Result<(), CameraError> {
        let identity = rebuild_basis(&Orientation::<5>::IDENTITY)?;
        assert_eq!(
            identity.u.map(f64::to_bits),
            [0x3ff0_0000_0000_0000, 0, 0, 0, 0]
        );
        assert_eq!(
            identity.v.map(f64::to_bits),
            [0, 0x3ff0_0000_0000_0000, 0, 0, 0]
        );
        assert_eq!(
            identity.remaining.map(|row| row.map(f64::to_bits)),
            [
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0x3ff0_0000_0000_0000, 0, 0],
                [0, 0, 0, 0x3ff0_0000_0000_0000, 0],
                [0, 0, 0, 0, 0x3ff0_0000_0000_0000],
            ]
        );
        let mut angles = [[Turn::ZERO; 5]; 5];
        angles[0][1] = Turn::from_bits(0x1234_5678);
        angles[0][4] = Turn::from_bits(0x9abc_def0);
        angles[2][3] = Turn::from_bits(0x2468_ace0);
        let orientation = Orientation::new(angles)?;
        let first = rebuild_basis(&orientation)?;
        let second = rebuild_basis(&orientation)?;
        assert_eq!(first, second);
        let vectors = [
            first.u,
            first.v,
            first.remaining[2],
            first.remaining[3],
            first.remaining[4],
        ];
        for (row_index, row) in vectors.iter().enumerate() {
            for (column_index, column) in vectors.iter().enumerate() {
                let expected = f64::from(u8::from(row_index == column_index));
                assert!((dot(row, column) - expected).abs() <= ORTHONORMAL_TOLERANCE);
            }
        }
        Ok(())
    }
}
