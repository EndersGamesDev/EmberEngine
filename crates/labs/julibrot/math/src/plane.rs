use crate::{Axis4, MathError, ObjectAngles, Plane};

/// The one seed pair the lab rotates: the Mandelbrot axes `(e₃,e₄)`.
pub const SEED_AXES: [Axis4; 2] = [Axis4::E3, Axis4::E4];

/// Constructs the sampled plane by applying the six-factor object rotation to the one seed.
///
/// # Errors
///
/// Returns an error for non-finite input or a failed rounding postcondition.
#[allow(clippy::cast_possible_truncation)]
pub fn construct_plane(angles: impl Into<ObjectAngles>) -> Result<Plane, MathError> {
    let angles = angles.into();
    if !angles.is_valid() {
        return Err(MathError::NonFinite);
    }
    let basis_u = rotate_axis(SEED_AXES[0], &angles);
    let basis_v = rotate_axis(SEED_AXES[1], &angles);
    let plane = Plane {
        basis_u: basis_u.map(|component| component as f32),
        basis_v: basis_v.map(|component| component as f32),
    };
    let norm_u = dot_f32(plane.basis_u, plane.basis_u);
    let norm_v = dot_f32(plane.basis_v, plane.basis_v);
    let orthogonality = dot_f32(plane.basis_u, plane.basis_v);
    let tolerance = 8.0 * f32::EPSILON;
    if (norm_u - 1.0).abs() > tolerance
        || (norm_v - 1.0).abs() > tolerance
        || orthogonality.abs() > tolerance
    {
        return Err(MathError::PlaneRoundingBound);
    }
    Ok(plane)
}

fn rotate_axis(axis: Axis4, angles: &ObjectAngles) -> [f64; 4] {
    let mut value = [0.0; 4];
    value[axis.index()] = 1.0;
    for (first, second, angle) in [
        (2, 3, angles.rho_34),
        (1, 3, angles.rho_24),
        (1, 2, angles.rho_23),
        (0, 3, angles.rho_14),
        (0, 2, angles.rho_13),
        (0, 1, angles.rho_12),
    ] {
        rotate_pair(&mut value, first, second, angle);
    }
    value
}

fn rotate_pair<const N: usize>(value: &mut [f64; N], first: usize, second: usize, angle: f64) {
    let (sine, cosine) = angle.sin_cos();
    let a = cosine.mul_add(value[first], -sine * value[second]);
    let b = sine.mul_add(value[first], cosine * value[second]);
    value[first] = a;
    value[second] = b;
}

fn object_rotation_matrix(angles: &ObjectAngles) -> [[f64; 4]; 4] {
    let columns: [[f64; 4]; 4] = core::array::from_fn(|column| {
        rotate_axis([Axis4::E1, Axis4::E2, Axis4::E3, Axis4::E4][column], angles)
    });
    core::array::from_fn(|row| core::array::from_fn(|column| columns[column][row]))
}

#[must_use]
pub fn rotation_orthonormality_4(angles: &ObjectAngles) -> f64 {
    orthonormality_error(object_rotation_matrix(angles))
}

fn orthonormality_error<const N: usize>(matrix: [[f64; N]; N]) -> f64 {
    (0..N)
        .flat_map(|row| (0..N).map(move |column| (row, column)))
        .map(|(row, column)| {
            let product = (0..N).fold(0.0, |sum, inner| {
                matrix[inner][row].mul_add(matrix[inner][column], sum)
            });
            let expected = if row == column { 1.0 } else { 0.0 };
            (product - expected).abs()
        })
        .fold(0.0, f64::max)
}

fn dot_f32(left: [f32; 4], right: [f32; 4]) -> f32 {
    left.into_iter()
        .zip(right)
        .fold(0.0, |sum, (a, b)| a.mul_add(b, sum))
}

#[cfg(test)]
mod tests {
    use super::construct_plane;
    use crate::{MathError, ObjectAngles, PlaneAngles};

    fn angles(theta_1: f64, theta_2: f64) -> PlaneAngles {
        PlaneAngles { theta_1, theta_2 }
    }

    #[test]
    fn six_angles_reproduce_the_legacy_two_angle_plane_exactly() -> Result<(), MathError> {
        for legacy in [angles(0.0, 0.0), angles(-0.7, 1.1), angles(0.4, -2.0)] {
            assert_eq!(
                construct_plane(legacy)?,
                construct_plane(ObjectAngles::from(legacy))?
            );
        }
        Ok(())
    }

    #[test]
    fn the_seed_is_exact_at_identity() -> Result<(), MathError> {
        let plane = construct_plane(angles(0.0, 0.0))?;
        assert_eq!(plane.basis_u, [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(plane.basis_v, [0.0, 0.0, 0.0, 1.0]);
        Ok(())
    }

    #[test]
    fn a_negative_quarter_turn_is_the_julia_pair() -> Result<(), MathError> {
        // The unit components are exact; the orthogonal ones carry only the binary32 image of
        // cos(pi/2) = 6.1e-17, which is the bound the reversed pair has always been pinned at.
        let quarter = core::f64::consts::FRAC_PI_2;
        let plane = construct_plane(angles(-quarter, -quarter))?;
        assert_eq!(plane.basis_u[0], 1.0);
        assert_eq!(plane.basis_u[1], 0.0);
        assert!(plane.basis_u[2].abs() <= f32::EPSILON);
        assert_eq!(plane.basis_u[3], 0.0);
        assert_eq!(plane.basis_v[1], 1.0);
        assert_eq!(plane.basis_v[0], 0.0);
        assert!(plane.basis_v[3].abs() <= f32::EPSILON);
        assert_eq!(plane.basis_v[2], 0.0);
        Ok(())
    }

    #[test]
    fn a_positive_quarter_turn_is_the_reversed_pair() -> Result<(), MathError> {
        let quarter = core::f64::consts::FRAC_PI_2;
        let plane = construct_plane(angles(quarter, quarter))?;
        assert!((plane.basis_u[0] + 1.0).abs() <= f32::EPSILON);
        assert!(plane.basis_u[2].abs() <= f32::EPSILON);
        assert!((plane.basis_v[1] + 1.0).abs() <= f32::EPSILON);
        assert!(plane.basis_v[3].abs() <= f32::EPSILON);
        Ok(())
    }

    #[test]
    fn interior_rotation_is_a_hybrid_plane() -> Result<(), MathError> {
        let plane = construct_plane(angles(0.4, 0.7))?;
        assert!(plane.basis_u[..2].iter().any(|component| *component != 0.0));
        assert!(plane.basis_u[2..].iter().any(|component| *component != 0.0));
        assert!(plane.basis_v[..2].iter().any(|component| *component != 0.0));
        assert!(plane.basis_v[2..].iter().any(|component| *component != 0.0));
        Ok(())
    }

    #[test]
    fn non_finite_angles_are_refused() {
        assert_eq!(
            construct_plane(angles(f64::NAN, 0.0)),
            Err(MathError::NonFinite)
        );
        assert_eq!(
            construct_plane(angles(0.0, f64::INFINITY)),
            Err(MathError::NonFinite)
        );
    }
}
