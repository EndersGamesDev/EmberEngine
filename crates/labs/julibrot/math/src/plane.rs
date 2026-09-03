use crate::{Axis4, MathError, ObjectAngles, Plane};

/// The one seed pair the lab rotates: the Mandelbrot axes `(e₃,e₄)`.
pub const SEED_AXES: [Axis4; 2] = [Axis4::E3, Axis4::E4];

const PLANE_SPAN_TOLERANCE: f32 = 8.0 * f32::EPSILON;

/// Exact chart-coordinate relation between two once-rounded bases of one ambient plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneChartRelation {
    /// Row-major map from retained chart coordinates to requested chart coordinates.
    pub chart_map: [f64; 4],
    /// Signed rotation of the requested basis relative to the retained basis.
    pub basis_angle: f64,
    /// Whether the requested basis reverses the retained plane orientation.
    pub reflected: bool,
}

/// Constructs the sampled plane by applying the six-factor object rotation to the one seed.
///
/// # Errors
///
/// Returns an error for non-finite input or a failed rounding postcondition.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the plane contract deliberately rounds once from f64 to shader-compatible f32"
)]
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

/// Compares the sampled planes selected by two object-angle records.
///
/// The returned matrix is exactly orthogonal even though the compared bases have each undergone
/// their one required f32 rounding pass.
///
/// # Errors
///
/// Returns an error when either angle record cannot construct a valid plane.
pub fn object_plane_relation(
    retained: ObjectAngles,
    requested: ObjectAngles,
) -> Result<Option<PlaneChartRelation>, MathError> {
    Ok(plane_chart_relation(
        construct_plane(retained)?,
        construct_plane(requested)?,
    ))
}

/// Returns the exact chart-coordinate relation when two rounded bases span the same plane.
#[must_use]
pub fn plane_chart_relation(retained: Plane, requested: Plane) -> Option<PlaneChartRelation> {
    let requested_u = requested.basis_u.map(f64::from);
    let requested_v = requested.basis_v.map(f64::from);
    let retained_u = retained.basis_u.map(f64::from);
    let retained_v = retained.basis_v.map(f64::from);
    let overlap = [
        dot_f64(retained_u, requested_u),
        dot_f64(retained_u, requested_v),
        dot_f64(retained_v, requested_u),
        dot_f64(retained_v, requested_v),
    ];
    let residual_u = span_residual(
        requested_u,
        retained_u,
        retained_v,
        [overlap[0], overlap[2]],
    );
    let residual_v = span_residual(
        requested_v,
        retained_u,
        retained_v,
        [overlap[1], overlap[3]],
    );
    if residual_u > f64::from(PLANE_SPAN_TOLERANCE) || residual_v > f64::from(PLANE_SPAN_TOLERANCE)
    {
        return None;
    }
    let basis_angle = overlap[2].atan2(overlap[0]);
    let (sine, cosine) = basis_angle.sin_cos();
    let reflected = overlap[0].mul_add(overlap[3], -overlap[1] * overlap[2]) < 0.0;
    let basis_map = if reflected {
        [cosine, sine, sine, -cosine]
    } else {
        [cosine, -sine, sine, cosine]
    };
    Some(PlaneChartRelation {
        chart_map: [basis_map[0], basis_map[2], basis_map[1], basis_map[3]],
        basis_angle,
        reflected,
    })
}

fn span_residual(
    vector: [f64; 4],
    basis_u: [f64; 4],
    basis_v: [f64; 4],
    coordinates: [f64; 2],
) -> f64 {
    vector
        .into_iter()
        .zip(basis_u)
        .zip(basis_v)
        .map(|((value, u), v)| value - coordinates[0].mul_add(u, coordinates[1] * v))
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
}

fn dot_f64(left: [f64; 4], right: [f64; 4]) -> f64 {
    left.into_iter()
        .zip(right)
        .fold(0.0, |sum, (a, b)| a.mul_add(b, sum))
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
    use super::{construct_plane, object_plane_relation};
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
    fn identity_o34_is_an_exact_in_plane_rotation() -> Result<(), MathError> {
        let requested = ObjectAngles {
            rho_34: 0.3,
            ..ObjectAngles::IDENTITY
        };
        let relation = object_plane_relation(ObjectAngles::IDENTITY, requested)?
            .expect("o34 preserves the identity seed plane");
        let (sine, cosine) = 0.3_f64.sin_cos();
        assert!((relation.basis_angle - 0.3).abs() <= f64::from(f32::EPSILON));
        assert!(!relation.reflected);
        for (actual, expected) in relation
            .chart_map
            .into_iter()
            .zip([cosine, sine, -sine, cosine])
        {
            assert!((actual - expected).abs() <= f64::from(f32::EPSILON));
        }
        Ok(())
    }

    #[test]
    fn identity_o12_is_inert_on_the_seed_plane() -> Result<(), MathError> {
        let requested = ObjectAngles {
            rho_12: 0.5,
            ..ObjectAngles::IDENTITY
        };
        let relation = object_plane_relation(ObjectAngles::IDENTITY, requested)?
            .expect("o12 preserves the identity seed plane");
        assert_eq!(relation.chart_map, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(relation.basis_angle, 0.0);
        assert!(!relation.reflected);
        Ok(())
    }

    #[test]
    fn rightmost_o34_rotates_inside_a_general_object_plane() -> Result<(), MathError> {
        let retained = ObjectAngles {
            rho_12: 0.2,
            rho_13: -0.4,
            rho_14: 0.1,
            rho_23: 0.25,
            rho_24: -0.3,
            rho_34: 0.15,
        };
        let requested = ObjectAngles {
            rho_34: retained.rho_34 + 0.3,
            ..retained
        };
        let relation = object_plane_relation(retained, requested)?
            .expect("the rightmost factor acts within the seed plane before the shared rotation");
        assert!((relation.basis_angle - 0.3).abs() <= 2.0 * f64::from(f32::EPSILON));
        assert!(!relation.reflected);
        Ok(())
    }

    #[test]
    fn identity_o13_tilts_out_of_the_seed_plane() -> Result<(), MathError> {
        let requested = ObjectAngles {
            rho_13: 0.3,
            ..ObjectAngles::IDENTITY
        };
        assert_eq!(
            object_plane_relation(ObjectAngles::IDENTITY, requested)?,
            None
        );
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
