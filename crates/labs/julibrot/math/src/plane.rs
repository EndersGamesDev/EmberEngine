use crate::{Axis4, MathError, Plane, PlaneAngles};

/// The one seed pair the lab rotates: the Mandelbrot axes `(e₃,e₄)`.
pub const SEED_AXES: [Axis4; 2] = [Axis4::E3, Axis4::E4];

/// Constructs the sampled plane from the one seed and two independent angles.
///
/// There is no preset argument because a preset is a row of control values, not a choice of axes:
/// `θ₁=θ₂=−π/2` carries the seed to exactly `(e₁,e₂)` and `+π/2` to the reversed pair, so every
/// named plane and every hybrid between them is an angle position of this one construction.
///
/// # Errors
///
/// Returns an error for non-finite input or a failed rounding postcondition.
#[allow(clippy::cast_possible_truncation)]
pub fn construct_plane(angles: PlaneAngles) -> Result<Plane, MathError> {
    if !angles.theta_1.is_finite() || !angles.theta_2.is_finite() {
        return Err(MathError::NonFinite);
    }
    let basis_u = rotate_axis(SEED_AXES[0], angles);
    let basis_v = rotate_axis(SEED_AXES[1], angles);
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

fn rotate_axis(axis: Axis4, angles: PlaneAngles) -> [f64; 4] {
    let mut value = [0.0; 4];
    value[axis.index()] = 1.0;
    let (sin_2, cos_2) = angles.theta_2.sin_cos();
    let e2 = cos_2.mul_add(value[1], -sin_2 * value[3]);
    let e4 = sin_2.mul_add(value[1], cos_2 * value[3]);
    value[1] = e2;
    value[3] = e4;
    let (sin_1, cos_1) = angles.theta_1.sin_cos();
    let e1 = cos_1.mul_add(value[0], -sin_1 * value[2]);
    let e3 = sin_1.mul_add(value[0], cos_1 * value[2]);
    value[0] = e1;
    value[2] = e3;
    value
}

fn dot_f32(left: [f32; 4], right: [f32; 4]) -> f32 {
    left.into_iter()
        .zip(right)
        .fold(0.0, |sum, (a, b)| a.mul_add(b, sum))
}

#[cfg(test)]
mod tests {
    use super::construct_plane;
    use crate::{MathError, PlaneAngles};

    fn angles(theta_1: f64, theta_2: f64) -> PlaneAngles {
        PlaneAngles { theta_1, theta_2 }
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
