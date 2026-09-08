use crate::basis::rotate_rows;
use crate::{Basis, CameraError, Fixed, Orientation, Turn, View, rebuild_basis};

/// Squared residual below which two directions cannot define another frame axis.
///
/// The threshold is far below one turn quantum at screen scale while staying above binary64 noise
/// left by repeated Gram-Schmidt subtraction.
const FRAME_DEGENERACY_NORM_SQUARED: f64 = 1.0e-24;

/// Fixed iteration count for the dependency-free Newton square root.
const SQRT_NEWTON_STEPS: usize = 64;

const ATAN_SERIES_DENOMINATORS: [f64; 15] = [
    3.0, 5.0, 7.0, 9.0, 11.0, 13.0, 15.0, 17.0, 19.0, 21.0, 23.0, 25.0, 27.0, 29.0, 31.0,
];

/// Reconstructs an orientation whose first axis follows an exact segment.
///
/// # Errors
///
/// Returns an error when the segment cannot define an image-plane frame or arithmetic fails.
pub fn orientation_for_segment<const N: usize, const LIMBS: usize>(
    view: &View<N, LIMBS>,
    segment: &[Fixed<LIMBS>; N],
) -> Result<Orientation<N>, CameraError> {
    let previous = rebuild_basis(&view.orientation)?;
    let mut direction = [0.0; N];
    for (output, coordinate) in direction.iter_mut().zip(segment) {
        *output = coordinate.to_f64()?;
    }
    let mut target = [[0.0; N]; N];
    target[0] = normalized(direction)?;
    let previous_frame = joined_frame(previous);
    for target_index in 1..N {
        let preferred = if target_index == 1 { 1 } else { target_index };
        let preferred_residual = residual(previous_frame[preferred], &target, target_index);
        if norm_squared(&preferred_residual) > FRAME_DEGENERACY_NORM_SQUARED {
            target[target_index] = normalized(preferred_residual)?;
            continue;
        }
        let mut best = [0.0; N];
        let mut best_norm = 0.0;
        for candidate in previous_frame {
            let candidate_residual = residual(candidate, &target, target_index);
            let candidate_norm = norm_squared(&candidate_residual);
            if candidate_norm > best_norm {
                best = candidate_residual;
                best_norm = candidate_norm;
            }
        }
        if best_norm <= FRAME_DEGENERACY_NORM_SQUARED {
            return Err(CameraError::DegenerateFrame);
        }
        target[target_index] = normalized(best)?;
    }
    orientation_from_frame(&target)
}

fn joined_frame<const N: usize>(basis: Basis<N>) -> [[f64; N]; N] {
    let mut frame = basis.remaining;
    frame[0] = basis.u;
    frame[1] = basis.v;
    frame
}

fn residual<const N: usize>(
    candidate: [f64; N],
    target: &[[f64; N]; N],
    completed: usize,
) -> [f64; N] {
    let mut output = candidate;
    for axis in target.iter().take(completed) {
        let projection = dot(&output, axis);
        for (component, axis_component) in output.iter_mut().zip(axis) {
            *component -= projection * axis_component;
        }
    }
    output
}

fn normalized<const N: usize>(mut vector: [f64; N]) -> Result<[f64; N], CameraError> {
    let maximum = vector
        .iter()
        .map(|component| component.abs())
        .fold(0.0_f64, f64::max);
    if !maximum.is_finite() || maximum == 0.0 {
        return Err(CameraError::DegenerateFrame);
    }
    for component in &mut vector {
        *component /= maximum;
    }
    let length = square_root(norm_squared(&vector));
    if !length.is_finite() || length == 0.0 {
        return Err(CameraError::DegenerateFrame);
    }
    for component in &mut vector {
        *component /= length;
    }
    Ok(vector)
}

fn norm_squared<const N: usize>(vector: &[f64; N]) -> f64 {
    dot(vector, vector)
}

fn dot<const N: usize>(left: &[f64; N], right: &[f64; N]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left_component, right_component)| left_component * right_component)
        .sum()
}

fn square_root(value: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    let mut estimate = if value < 1.0 { 1.0 } else { value };
    for _ in 0..SQRT_NEWTON_STEPS {
        estimate = estimate.midpoint(value / estimate);
    }
    estimate
}

fn orientation_from_frame<const N: usize>(
    target: &[[f64; N]; N],
) -> Result<Orientation<N>, CameraError> {
    let mut current = [[0.0; N]; N];
    for (index, vector) in current.iter_mut().enumerate() {
        vector[index] = 1.0;
    }
    let mut angles = [[Turn::ZERO; N]; N];
    for first in 0..N.saturating_sub(1) {
        let mut coefficients = [0.0; N];
        for (second, coefficient) in coefficients.iter_mut().enumerate().skip(first) {
            *coefficient = dot(&target[first], &current[second]);
        }
        let coefficient_norm = square_root(
            coefficients[first..]
                .iter()
                .map(|coefficient| coefficient * coefficient)
                .sum(),
        );
        if coefficient_norm <= FRAME_DEGENERACY_NORM_SQUARED {
            return Err(CameraError::DegenerateFrame);
        }
        for coefficient in &mut coefficients[first..] {
            *coefficient /= coefficient_norm;
        }
        for (second, angle) in angles[first].iter_mut().enumerate().skip(first + 2).rev() {
            let prefix_norm = square_root(
                coefficients[first..second]
                    .iter()
                    .map(|coefficient| coefficient * coefficient)
                    .sum(),
            );
            let tail = coefficients[second];
            *angle = Turn::from_radians(atan2(tail, prefix_norm))?;
            if prefix_norm <= FRAME_DEGENERACY_NORM_SQUARED {
                coefficients[first] = 1.0;
                for coefficient in &mut coefficients[first + 1..second] {
                    *coefficient = 0.0;
                }
            } else {
                let total_norm = square_root(prefix_norm * prefix_norm + tail * tail);
                let factor = total_norm / prefix_norm;
                for coefficient in &mut coefficients[first..second] {
                    *coefficient *= factor;
                }
            }
            coefficients[second] = 0.0;
        }
        angles[first][first + 1] =
            Turn::from_radians(atan2(coefficients[first + 1], coefficients[first]))?;
        for (second, angle) in angles[first].iter().copied().enumerate().skip(first + 1) {
            rotate_rows(&mut current, first, second, angle);
        }
    }
    Orientation::new(angles)
}

fn atan2(vertical: f64, horizontal: f64) -> f64 {
    if horizontal == 0.0 {
        return if vertical > 0.0 {
            core::f64::consts::FRAC_PI_2
        } else if vertical < 0.0 {
            -core::f64::consts::FRAC_PI_2
        } else {
            0.0
        };
    }
    let unsigned = if horizontal > 0.0 {
        atan_positive(vertical.abs() / horizontal)
    } else {
        core::f64::consts::PI - atan_positive(vertical.abs() / -horizontal)
    };
    if vertical < 0.0 { -unsigned } else { unsigned }
}

fn atan_positive(value: f64) -> f64 {
    if value > 1.0 {
        return core::f64::consts::FRAC_PI_2 - atan_positive(1.0 / value);
    }
    if value > core::f64::consts::SQRT_2 - 1.0 {
        return core::f64::consts::FRAC_PI_4 + atan_series((value - 1.0) / (value + 1.0));
    }
    atan_series(value)
}

fn atan_series(value: f64) -> f64 {
    let squared = value * value;
    let mut power = value;
    let mut sum = value;
    let mut subtract = true;
    for denominator in ATAN_SERIES_DENOMINATORS {
        power *= squared;
        let term = power / denominator;
        if subtract {
            sum -= term;
        } else {
            sum += term;
        }
        subtract = !subtract;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::orientation_for_segment;
    use crate::{CameraError, Exponent, Fixed, Orientation, View, rebuild_basis};

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
            .map(|(left, right)| left * right)
            .sum();
        assert!(orthogonality.abs() <= ALIGNMENT_TOLERANCE);
        Ok(())
    }
}
