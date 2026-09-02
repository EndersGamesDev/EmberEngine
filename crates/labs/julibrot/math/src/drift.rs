const DELTA_THETA: f64 = 1.0e-3;
const GOLDEN_RATIO: f64 = 1.618_033_988_749_895;
const MATRIX_SIDE: usize = 5;

#[must_use]
pub fn navigation_drift_f64(steps: u32) -> f64 {
    let increment = rotation_f64(DELTA_THETA);
    let mut accumulated = identity_f64();
    for _ in 0..steps {
        accumulated = multiply_f64(accumulated, increment);
    }
    orthonormality_error_f64(accumulated)
}

#[must_use]
pub fn navigation_drift_f32(steps: u32) -> f64 {
    let increment = rotation_f32(DELTA_THETA as f32);
    let mut accumulated = identity_f32();
    for step in 1..=steps {
        accumulated = multiply_f32(accumulated, increment);
        if step % 64 == 0 {
            accumulated = gram_schmidt_f32(accumulated);
        }
    }
    orthonormality_error_f32(accumulated)
}

fn rotation_f64(theta: f64) -> [[f64; MATRIX_SIDE]; MATRIX_SIDE] {
    let mut matrix = identity_f64();
    let (sin_1, cos_1) = theta.sin_cos();
    let (sin_2, cos_2) = (GOLDEN_RATIO * theta).sin_cos();
    matrix[0][0] = cos_1;
    matrix[0][1] = -sin_1;
    matrix[1][0] = sin_1;
    matrix[1][1] = cos_1;
    matrix[2][2] = cos_2;
    matrix[2][4] = -sin_2;
    matrix[4][2] = sin_2;
    matrix[4][4] = cos_2;
    matrix
}

fn rotation_f32(theta: f32) -> [[f32; MATRIX_SIDE]; MATRIX_SIDE] {
    let mut matrix = identity_f32();
    let (sin_1, cos_1) = theta.sin_cos();
    let (sin_2, cos_2) = ((GOLDEN_RATIO as f32) * theta).sin_cos();
    matrix[0][0] = cos_1;
    matrix[0][1] = -sin_1;
    matrix[1][0] = sin_1;
    matrix[1][1] = cos_1;
    matrix[2][2] = cos_2;
    matrix[2][4] = -sin_2;
    matrix[4][2] = sin_2;
    matrix[4][4] = cos_2;
    matrix
}

fn identity_f64() -> [[f64; MATRIX_SIDE]; MATRIX_SIDE] {
    core::array::from_fn(|row| {
        core::array::from_fn(|column| if row == column { 1.0 } else { 0.0 })
    })
}

fn identity_f32() -> [[f32; MATRIX_SIDE]; MATRIX_SIDE] {
    core::array::from_fn(|row| {
        core::array::from_fn(|column| if row == column { 1.0 } else { 0.0 })
    })
}

fn multiply_f64(
    left: [[f64; MATRIX_SIDE]; MATRIX_SIDE],
    right: [[f64; MATRIX_SIDE]; MATRIX_SIDE],
) -> [[f64; MATRIX_SIDE]; MATRIX_SIDE] {
    core::array::from_fn(|row| {
        core::array::from_fn(|column| {
            (0..MATRIX_SIDE).fold(0.0, |sum, inner| {
                left[row][inner].mul_add(right[inner][column], sum)
            })
        })
    })
}

fn multiply_f32(
    left: [[f32; MATRIX_SIDE]; MATRIX_SIDE],
    right: [[f32; MATRIX_SIDE]; MATRIX_SIDE],
) -> [[f32; MATRIX_SIDE]; MATRIX_SIDE] {
    core::array::from_fn(|row| {
        core::array::from_fn(|column| {
            (0..MATRIX_SIDE).fold(0.0, |sum, inner| {
                left[row][inner].mul_add(right[inner][column], sum)
            })
        })
    })
}

fn gram_schmidt_f32(mut matrix: [[f32; MATRIX_SIDE]; MATRIX_SIDE]) -> [[f32; 5]; 5] {
    for column in 0..MATRIX_SIDE {
        for previous in 0..column {
            let projection = (0..MATRIX_SIDE).fold(0.0, |sum, row| {
                matrix[row][column].mul_add(matrix[row][previous], sum)
            });
            for row in &mut matrix {
                row[column] -= projection * row[previous];
            }
        }
        let norm = (0..MATRIX_SIDE)
            .fold(0.0, |sum, row| matrix[row][column].mul_add(matrix[row][column], sum))
            .sqrt();
        for row in &mut matrix {
            row[column] /= norm;
        }
    }
    matrix
}

fn orthonormality_error_f64(matrix: [[f64; MATRIX_SIDE]; MATRIX_SIDE]) -> f64 {
    let squared_error = (0..MATRIX_SIDE).fold(0.0, |outer_sum, row| {
        outer_sum
            + (0..MATRIX_SIDE).fold(0.0, |inner_sum, column| {
                let dot = (0..MATRIX_SIDE).fold(0.0, |sum, axis| {
                    matrix[axis][row].mul_add(matrix[axis][column], sum)
                });
                let residual = dot - if row == column { 1.0 } else { 0.0 };
                residual.mul_add(residual, inner_sum)
            })
    });
    squared_error.sqrt()
}

fn orthonormality_error_f32(matrix: [[f32; MATRIX_SIDE]; MATRIX_SIDE]) -> f64 {
    let squared_error = (0..MATRIX_SIDE).fold(0.0_f64, |outer_sum, row| {
        outer_sum
            + (0..MATRIX_SIDE).fold(0.0, |inner_sum, column| {
                let dot = (0..MATRIX_SIDE).fold(0.0_f64, |sum, axis| {
                    f64::from(matrix[axis][row])
                        .mul_add(f64::from(matrix[axis][column]), sum)
                });
                let residual = dot - if row == column { 1.0 } else { 0.0 };
                residual.mul_add(residual, inner_sum)
            })
    });
    squared_error.sqrt()
}

#[cfg(test)]
mod tests {
    use super::{navigation_drift_f32, navigation_drift_f64};

    #[test]
    fn composed_navigation_rotation_meets_the_bound() {
        for steps in [10_000, 100_000] {
            assert!(navigation_drift_f64(steps) <= 1.0e-5);
            assert!(navigation_drift_f32(steps) <= 1.0e-5);
        }
    }
}
