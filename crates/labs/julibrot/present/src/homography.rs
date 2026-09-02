const PIVOT_EPSILON: f64 = 1.0e-12;

/// Solves the projective map from four destination anchors to four source anchors.
#[must_use]
pub fn solve_homography(destination: [[f64; 2]; 4], source: [[f64; 2]; 4]) -> Option<[f64; 9]> {
    if destination
        .iter()
        .chain(source.iter())
        .flatten()
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let mut augmented = [[0.0; 9]; 8];
    for (point, ([x, y], [u, v])) in destination.into_iter().zip(source).enumerate() {
        augmented[point * 2] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        augmented[point * 2 + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    let mut column = 0;
    while column < 8 {
        let pivot = (column..8).max_by(|left, right| {
            augmented[*left][column]
                .abs()
                .total_cmp(&augmented[*right][column].abs())
        })?;
        if augmented[pivot][column].abs() < PIVOT_EPSILON {
            return None;
        }
        augmented.swap(column, pivot);
        let divisor = augmented[column][column];
        for value in &mut augmented[column][column..] {
            *value /= divisor;
        }
        let pivot_row = augmented[column];
        let mut row = 0;
        while row < 8 {
            if row != column {
                let factor = augmented[row][column];
                for (value, pivot_value) in augmented[row][column..]
                    .iter_mut()
                    .zip(&pivot_row[column..])
                {
                    *value -= factor * pivot_value;
                }
            }
            row += 1;
        }
        column += 1;
    }
    let mut result = [0.0; 9];
    for (value, row) in result[..8].iter_mut().zip(augmented) {
        *value = row[8];
    }
    result[8] = 1.0;
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

/// Applies a row-major homography and rejects a pole or non-finite result.
#[must_use]
pub fn apply_homography(matrix: [f64; 9], point: [f64; 2]) -> Option<[f64; 2]> {
    let [x, y] = point;
    let denominator = matrix[6].mul_add(x, matrix[7].mul_add(y, matrix[8]));
    if !denominator.is_finite() || denominator.abs() <= PIVOT_EPSILON {
        return None;
    }
    let mapped = [
        matrix[0].mul_add(x, matrix[1].mul_add(y, matrix[2])) / denominator,
        matrix[3].mul_add(x, matrix[4].mul_add(y, matrix[5])) / denominator,
    ];
    mapped
        .iter()
        .all(|value| value.is_finite())
        .then_some(mapped)
}

/// Returns `max(abs(inverse * forward - identity))` for the f64 warp oracle.
#[must_use]
pub fn inverse_identity_error(forward: [f64; 9], inverse: [f64; 9]) -> f64 {
    let mut maximum = 0.0_f64;
    let mut row = 0;
    while row < 3 {
        let mut column = 0;
        while column < 3 {
            let mut value = 0.0;
            let mut inner = 0;
            while inner < 3 {
                value = inverse[row * 3 + inner].mul_add(forward[inner * 3 + column], value);
                inner += 1;
            }
            let expected = if row == column { 1.0 } else { 0.0 };
            maximum = maximum.max((value - expected).abs());
            column += 1;
        }
        row += 1;
    }
    maximum
}

/// Rounds a finite row-major f64 homography into three padded f32 rows.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn pack_homography_rows(forward: [f64; 9]) -> Option<[[f32; 4]; 3]> {
    let mut rows = [[0.0; 4]; 3];
    let (source_rows, remainder) = forward.as_chunks::<3>();
    debug_assert_eq!(remainder, []);
    for (destination, source) in rows.iter_mut().zip(source_rows) {
        for (packed, value) in destination[..3].iter_mut().zip(source) {
            *packed = *value as f32;
            if !packed.is_finite() {
                return None;
            }
        }
    }
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

    #[test]
    fn solver_recovers_an_affine_pan_and_zoom() {
        let source = CORNERS.map(|[x, y]| [0.5_f64.mul_add(x, 0.25), 2.0_f64.mul_add(y, -0.125)]);
        let matrix = solve_homography(CORNERS, source).expect("independent corners");
        for (destination, expected) in CORNERS.into_iter().zip(source) {
            let actual = apply_homography(matrix, destination).expect("finite affine result");
            assert!((actual[0] - expected[0]).abs() < 1.0e-12);
            assert!((actual[1] - expected[1]).abs() < 1.0e-12);
        }
        assert_eq!(
            pack_homography_rows(matrix).map(|rows| rows[2][3]),
            Some(0.0)
        );
    }

    #[test]
    fn solver_refuses_degenerate_or_non_finite_anchors() {
        assert!(solve_homography([[0.0, 0.0]; 4], CORNERS).is_none());
        let mut invalid = CORNERS;
        invalid[2][0] = f64::NAN;
        assert!(solve_homography(invalid, CORNERS).is_none());
    }

    #[test]
    fn inverse_error_matches_the_shared_oracle_metric() {
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        assert_eq!(inverse_identity_error(identity, identity), 0.0);
        let perturbed = [1.0, 0.0, 1.0e-10, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        assert_eq!(inverse_identity_error(perturbed, identity), 1.0e-10);
    }
}
