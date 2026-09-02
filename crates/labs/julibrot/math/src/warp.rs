use crate::{MathError, Plane, Pose, WarpMatrix};

const MINIMUM_DETERMINANT: f64 = 1.0 / 1_099_511_627_776.0;

/// Builds inverse-sampling `to`-to-`from` and its explicit affine inverse.
///
/// # Errors
///
/// Returns an error for invalid poses, non-finite coefficients, or a small determinant.
pub fn warp_matrix(from: &Pose, to: &Pose) -> Result<WarpMatrix, MathError> {
    validate_pose(from)?;
    validate_pose(to)?;
    let scale_ratio = (from.zoom_log2 - to.zoom_log2).exp2()
        * f64::from(from.grid_width)
        / f64::from(to.grid_width);
    if !scale_ratio.is_finite() || scale_ratio == 0.0 {
        return Err(MathError::DegenerateWarp);
    }
    let basis_change = basis_overlap(from.plane, to.plane);
    let m00 = scale_ratio * basis_change[0];
    let m01 = scale_ratio * basis_change[1];
    let m10 = scale_ratio * basis_change[2];
    let m11 = scale_ratio * basis_change[3];
    let from_width = f64::from(from.grid_width);
    let from_height = f64::from(from.grid_height);
    let to_width = f64::from(to.grid_width);
    let to_height = f64::from(to.grid_height);
    let a00 = m00 * to_width / from_width;
    let a01 = m01 * to_height / from_width;
    let a10 = m10 * to_width / from_height;
    let a11 = m11 * to_height / from_height;
    let b0 = 2.0
        * (m00.mul_add(
            to.centre_from_reference_px[0],
            m01 * to.centre_from_reference_px[1],
        ) - from.centre_from_reference_px[0])
        / from_width;
    let b1 = 2.0
        * (m10.mul_add(
            to.centre_from_reference_px[0],
            m11 * to.centre_from_reference_px[1],
        ) - from.centre_from_reference_px[1])
        / from_height;
    let determinant = a00.mul_add(a11, -(a01 * a10));
    if !determinant.is_finite() || determinant.abs() <= MINIMUM_DETERMINANT {
        return Err(MathError::DegenerateWarp);
    }
    let forward = [a00, a01, b0, a10, a11, b1, 0.0, 0.0, 1.0];
    if !forward.iter().all(|coefficient| coefficient.is_finite()) {
        return Err(MathError::NonFinite);
    }
    let inverse_a00 = a11 / determinant;
    let inverse_a01 = -a01 / determinant;
    let inverse_a10 = -a10 / determinant;
    let inverse_a11 = a00 / determinant;
    let inverse_b0 = -(inverse_a00.mul_add(b0, inverse_a01 * b1));
    let inverse_b1 = -(inverse_a10.mul_add(b0, inverse_a11 * b1));
    let inverse = [
        inverse_a00,
        inverse_a01,
        inverse_b0,
        inverse_a10,
        inverse_a11,
        inverse_b1,
        0.0,
        0.0,
        1.0,
    ];
    if !inverse.iter().all(|coefficient| coefficient.is_finite()) {
        return Err(MathError::NonFinite);
    }
    Ok(WarpMatrix { forward, inverse })
}

#[must_use]
pub fn warp_identity_error(matrix: WarpMatrix) -> f64 {
    let product = multiply_3x3(matrix.inverse, matrix.forward);
    product
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let expected = if index % 4 == 0 { 1.0 } else { 0.0 };
            (value - expected).abs()
        })
        .fold(0.0, f64::max)
}

fn validate_pose(pose: &Pose) -> Result<(), MathError> {
    if pose.grid_width == 0 || pose.grid_height == 0 {
        return Err(MathError::InvalidExtent);
    }
    let scalar_values = [
        pose.plane_theta_1,
        pose.plane_theta_2,
        pose.zoom_log2,
        pose.view_theta_1,
        pose.centre_from_reference_px[0],
        pose.centre_from_reference_px[1],
    ];
    if !scalar_values.iter().all(|value| value.is_finite())
        || !pose
            .plane
            .basis_u
            .into_iter()
            .chain(pose.plane.basis_v)
            .all(f32::is_finite)
    {
        return Err(MathError::NonFinite);
    }
    Ok(())
}

fn basis_overlap(from: Plane, to: Plane) -> [f64; 4] {
    [
        dot(from.basis_u, to.basis_u),
        dot(from.basis_u, to.basis_v),
        dot(from.basis_v, to.basis_u),
        dot(from.basis_v, to.basis_v),
    ]
}

fn dot(left: [f32; 4], right: [f32; 4]) -> f64 {
    left.into_iter()
        .zip(right)
        .fold(0.0, |sum, (a, b)| f64::from(a).mul_add(f64::from(b), sum))
}

fn multiply_3x3(left: [f64; 9], right: [f64; 9]) -> [f64; 9] {
    core::array::from_fn(|index| {
        let row = index / 3;
        let column = index % 3;
        (0..3).fold(0.0, |sum, inner| {
            left[row * 3 + inner].mul_add(right[inner * 3 + column], sum)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{warp_identity_error, warp_matrix};
    use crate::{MathError, Plane, Pose, ViewMode};

    fn pose(zoom_log2: f64, displacement: [f64; 2]) -> Pose {
        Pose {
            epoch: 1,
            orbit_generation: 2,
            plane: Plane {
                basis_u: [0.8, 0.0, 0.6, 0.0],
                basis_v: [0.0, 0.6, 0.0, 0.8],
            },
            plane_theta_1: 0.643_501_108_793_284_4,
            plane_theta_2: 0.927_295_218_001_612_3,
            zoom_log2,
            view_theta_1: 0.0,
            grid_width: 1920,
            grid_height: 1080,
            view: ViewMode::Flat,
            centre_from_reference_px: displacement,
        }
    }

    #[test]
    fn inverse_times_forward_is_identity_at_all_required_depths() -> Result<(), MathError> {
        for zoom_log2 in [0.0, 10.0, 20.0, 40.0, 80.0, 100.0] {
            let from = pose(zoom_log2, [13.25, -7.5]);
            let mut to = pose(zoom_log2 + 0.75, [-4.0, 9.125]);
            to.grid_width = 1536;
            to.grid_height = 864;
            let matrix = warp_matrix(&from, &to)?;
            assert!(warp_identity_error(matrix) <= 1.0e-9);
        }
        Ok(())
    }

    #[test]
    fn matching_chart_reduces_to_displacement_translation() -> Result<(), MathError> {
        let from = pose(40.0, [2.0, 3.0]);
        let to = pose(40.0, [7.0, -1.0]);
        let matrix = warp_matrix(&from, &to)?;
        assert!((matrix.forward[2] - 10.0 / 1920.0).abs() <= 1.0e-9);
        assert!((matrix.forward[5] + 8.0 / 1080.0).abs() <= 1.0e-9);
        Ok(())
    }
}
