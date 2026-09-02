use crate::{Axis4, MathError, Plane, PlaneAngles, PlanePreset, PlaneSpec};

pub fn preset_spec(preset: PlanePreset) -> Result<PlaneSpec, MathError> {
    let spec = match preset {
        PlanePreset::Mandelbrot => PlaneSpec {
            axis_a: Axis4::E3,
            axis_b: Axis4::E4,
            plane_origin: [0.0; 4],
        },
        PlanePreset::Julia { c0 } => PlaneSpec {
            axis_a: Axis4::E1,
            axis_b: Axis4::E2,
            plane_origin: [0.0, 0.0, c0[0], c0[1]],
        },
    };
    if spec.plane_origin.iter().all(|component| component.is_finite()) {
        Ok(spec)
    } else {
        Err(MathError::NonFinite)
    }
}

pub fn construct_plane(preset: PlanePreset, angles: PlaneAngles) -> Result<Plane, MathError> {
    construct_plane_from_spec(preset_spec(preset)?, angles)
}

pub fn construct_plane_from_spec(
    spec: PlaneSpec,
    angles: PlaneAngles,
) -> Result<Plane, MathError> {
    if spec.axis_a == spec.axis_b {
        return Err(MathError::InvalidPlaneSeed);
    }
    if !angles.theta_1.is_finite()
        || !angles.theta_2.is_finite()
        || !spec
            .plane_origin
            .iter()
            .all(|component| component.is_finite())
    {
        return Err(MathError::NonFinite);
    }
    let basis_u = rotate_axis(spec.axis_a, angles);
    let basis_v = rotate_axis(spec.axis_b, angles);
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
    use super::{construct_plane, preset_spec};
    use crate::{Axis4, MathError, PlaneAngles, PlanePreset};

    #[test]
    fn presets_are_exact_at_identity() -> Result<(), MathError> {
        let identity = PlaneAngles {
            theta_1: 0.0,
            theta_2: 0.0,
        };
        let mandelbrot = construct_plane(PlanePreset::Mandelbrot, identity)?;
        assert_eq!(mandelbrot.basis_u, [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(mandelbrot.basis_v, [0.0, 0.0, 0.0, 1.0]);
        let julia = construct_plane(PlanePreset::Julia { c0: [-0.8, 0.156] }, identity)?;
        assert_eq!(julia.basis_u, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(julia.basis_v, [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(
            preset_spec(PlanePreset::Julia { c0: [-0.8, 0.156] })?.plane_origin,
            [0.0, 0.0, -0.8, 0.156]
        );
        assert_eq!(
            preset_spec(PlanePreset::Mandelbrot)?.axis_a,
            Axis4::E3
        );
        Ok(())
    }

    #[test]
    fn quarter_turn_maps_mandelbrot_to_reversed_julia() -> Result<(), MathError> {
        let plane = construct_plane(
            PlanePreset::Mandelbrot,
            PlaneAngles {
                theta_1: core::f64::consts::FRAC_PI_2,
                theta_2: core::f64::consts::FRAC_PI_2,
            },
        )?;
        assert!((plane.basis_u[0] + 1.0).abs() <= f32::EPSILON);
        assert!(plane.basis_u[2].abs() <= f32::EPSILON);
        assert!((plane.basis_v[1] + 1.0).abs() <= f32::EPSILON);
        assert!(plane.basis_v[3].abs() <= f32::EPSILON);
        Ok(())
    }

    #[test]
    fn interior_rotation_is_a_hybrid_plane() -> Result<(), MathError> {
        let plane = construct_plane(
            PlanePreset::Mandelbrot,
            PlaneAngles {
                theta_1: 0.4,
                theta_2: 0.7,
            },
        )?;
        assert!(plane.basis_u[..2].iter().any(|component| *component != 0.0));
        assert!(plane.basis_u[2..].iter().any(|component| *component != 0.0));
        assert!(plane.basis_v[..2].iter().any(|component| *component != 0.0));
        assert!(plane.basis_v[2..].iter().any(|component| *component != 0.0));
        Ok(())
    }
}
