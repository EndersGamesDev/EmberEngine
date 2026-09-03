use crate::screen::{invert_3x3, multiply_3x3};
use crate::{MathError, Pose, PoseMap, WarpMatrix, plane_chart_relation};

/// Builds the composed source-to-destination screen homography and its inverse-sampling inverse.
///
/// # Errors
///
/// Returns an error for invalid poses, non-finite coefficients, or a small determinant.
pub fn warp_matrix(from: &Pose, to: &Pose) -> Result<WarpMatrix, MathError> {
    validate_pose(from)?;
    validate_pose(to)?;
    let (PoseMap::Mapped(from_map), PoseMap::Mapped(to_map)) = (from.map, to.map) else {
        return Err(MathError::DegenerateWarp);
    };
    let scale_ratio = (to.zoom_log2 - from.zoom_log2).exp2() * f64::from(to.grid_width)
        / f64::from(from.grid_width);
    if !scale_ratio.is_finite() || scale_ratio == 0.0 {
        return Err(MathError::DegenerateWarp);
    }
    let basis_change = plane_chart_relation(from.plane, to.plane)
        .ok_or(MathError::DegenerateWarp)?
        .chart_map;
    let m00 = scale_ratio * basis_change[0];
    let m01 = scale_ratio * basis_change[1];
    let m10 = scale_ratio * basis_change[2];
    let m11 = scale_ratio * basis_change[3];
    let b0 = m00.mul_add(
        from.centre_from_reference_px[0],
        m01 * from.centre_from_reference_px[1],
    ) - to.centre_from_reference_px[0]
        + origin_offset_px(from, to, to.plane.basis_u)?;
    let b1 = m10.mul_add(
        from.centre_from_reference_px[0],
        m11 * from.centre_from_reference_px[1],
    ) - to.centre_from_reference_px[1]
        + origin_offset_px(from, to, to.plane.basis_v)?;
    let plane_map = [m00, m01, b0, m10, m11, b1, 0.0, 0.0, 1.0];
    let forward = multiply_3x3(to_map.inverse, multiply_3x3(plane_map, from_map.rows));
    if !forward.iter().all(|coefficient| coefficient.is_finite()) {
        return Err(MathError::NonFinite);
    }
    let inverse = invert_3x3(forward).ok_or(MathError::DegenerateWarp)?;
    Ok(WarpMatrix { forward, inverse })
}

fn origin_offset_px(from: &Pose, to: &Pose, axis: [f32; 4]) -> Result<f64, MathError> {
    let target_pixels_per_chart = 0.25 * f64::from(to.grid_width) * to.zoom_log2.exp2();
    let offset = from
        .plane_origin
        .into_iter()
        .zip(to.plane_origin)
        .zip(axis)
        .fold(0.0, |sum, ((from, to), component)| {
            (from - to).mul_add(f64::from(component), sum)
        });
    let pixels = offset * target_pixels_per_chart;
    pixels
        .is_finite()
        .then_some(pixels)
        .ok_or(MathError::DegenerateWarp)
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
    if !pose.view.is_valid() {
        return Err(MathError::InvalidViewControls);
    }
    if !pose.object.is_valid() {
        return Err(MathError::NonFinite);
    }
    let map = match pose.map {
        PoseMap::Mapped(map) => map,
        PoseMap::EdgeOn => return Ok(()),
    };
    let scalar_values = [
        pose.zoom_log2,
        pose.centre_from_reference_px[0],
        pose.centre_from_reference_px[1],
        map.condition_number,
        map.apron_scale,
    ];
    if !scalar_values.iter().all(|value| value.is_finite())
        || !(1.0..=2.0).contains(&map.apron_scale)
        || !map
            .rows
            .iter()
            .chain(&map.inverse)
            .all(|value| value.is_finite())
        || !pose.plane_origin.into_iter().all(f64::is_finite)
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

#[cfg(test)]
mod tests {
    use super::{warp_identity_error, warp_matrix};
    use crate::{Homography, MathError, ObjectAngles, Plane, Pose, PoseMap, ViewControls};

    fn pose(zoom_log2: f64, displacement: [f64; 2]) -> Pose {
        Pose {
            epoch: 1,
            orbit_generation: 2,
            plane: Plane {
                basis_u: [0.8, 0.0, 0.6, 0.0],
                basis_v: [0.0, 0.6, 0.0, 0.8],
            },
            object: ObjectAngles {
                rho_13: 0.643_501_108_793_284_4,
                rho_24: 0.927_295_218_001_612_3,
                ..ObjectAngles::IDENTITY
            },
            plane_origin: [0.0; 4],
            zoom_log2,
            view: ViewControls::NEUTRAL,
            grid_width: 1920,
            grid_height: 1080,
            map: PoseMap::Mapped(Homography::IDENTITY),
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
        let exact_plane = Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        };
        let mut from = pose(40.0, [2.0, 3.0]);
        from.plane = exact_plane;
        let mut to = pose(40.0, [7.0, -1.0]);
        to.plane = exact_plane;
        let matrix = warp_matrix(&from, &to)?;
        assert!((matrix.forward[2] + 5.0).abs() <= 1.0e-9);
        assert!((matrix.forward[5] - 4.0).abs() <= 1.0e-9);
        Ok(())
    }

    #[test]
    fn in_plane_origin_translation_is_an_exact_screen_pan() -> Result<(), MathError> {
        let exact_plane = Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        };
        let mut from = pose(0.0, [0.0; 2]);
        from.plane = exact_plane;
        let mut to = from;
        to.plane_origin = [0.5, -0.25, 0.0, 0.0];
        let matrix = warp_matrix(&from, &to)?;
        assert!((matrix.forward[2] + 240.0).abs() <= 1.0e-12);
        assert!((matrix.forward[5] - 120.0).abs() <= 1.0e-12);
        assert!(warp_identity_error(matrix) <= 1.0e-12);
        Ok(())
    }
}
