use ember_julibrot_math::{Plane, Pose, ViewMode, warp_matrix};

use crate::{
    SceneFrame, WarpKind, WarpPlan, apply_homography, pack_homography_rows, solve_homography,
};

const CHART_CORNERS: [[f64; 2]; 4] =
    [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];
const HEIGHT_SAMPLES: [f64; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];
const CHART_STEPS: u32 = 9;
const PERSPECTIVE_POLE: f64 = 8.0;
const POLE_EPSILON: f64 = 1.0e-4;

/// Pure CPU reprojection planner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Warp;

impl Warp {
    /// Builds the exact flat or four-anchor tumbled inverse-sampling plan.
    ///
    /// A pose mismatch, incompatible arithmetic, or projection pole returns an honest clear-only
    /// plan. This function never allocates a GPU resource, submits work, or mutates the frame.
    #[must_use]
    pub fn reproject(last_frame: &SceneFrame, from_pose: &Pose, to_pose: &Pose) -> WarpPlan {
        if last_frame.pose != *from_pose {
            return clear_only();
        }
        let Ok(flat) = warp_matrix(from_pose, to_pose) else {
            return clear_only();
        };
        let chart_residual = chart_residual(from_pose, to_pose);
        if !chart_residual.is_finite() {
            return clear_only();
        }
        if from_pose.view == ViewMode::Flat && to_pose.view == ViewMode::Flat {
            return pack_homography_rows(flat.forward).map_or_else(clear_only, |rows| WarpPlan {
                rows,
                source_valid: true,
                kind: WarpKind::FlatExact,
                chart_residual,
                approx_max_error_px: None,
                approx_p95_error_px: None,
            });
        }
        tumbled_plan(from_pose, to_pose, flat.forward, chart_residual).unwrap_or_else(clear_only)
    }
}

fn clear_only() -> WarpPlan {
    WarpPlan {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        source_valid: false,
        kind: WarpKind::ClearOnly,
        chart_residual: 0.0,
        approx_max_error_px: None,
        approx_p95_error_px: None,
    }
}

fn tumbled_plan(
    from_pose: &Pose,
    to_pose: &Pose,
    flat_forward: [f64; 9],
    chart_residual: f64,
) -> Option<WarpPlan> {
    let destination = CHART_CORNERS.map(|chart| project_presented(to_pose, chart, 0.0));
    let source = CHART_CORNERS.map(|chart| {
        let source_chart = apply_homography(flat_forward, chart)?;
        project_presented(from_pose, source_chart, 0.0)
    });
    let destination = transpose_options(destination)?;
    let source = transpose_options(source)?;
    let matrix = solve_homography(destination, source)?;
    let rows = pack_homography_rows(matrix)?;
    let mut errors = sampled_tumbled_errors(from_pose, to_pose, flat_forward, matrix)?;
    errors.sort_by(f64::total_cmp);
    let maximum = errors.last().copied()?;
    let percentile_index = errors.len().saturating_mul(95).div_ceil(100).saturating_sub(1);
    let percentile = *errors.get(percentile_index)?;
    Some(WarpPlan {
        rows,
        source_valid: true,
        kind: WarpKind::TumbledHomography,
        chart_residual,
        approx_max_error_px: Some(maximum),
        approx_p95_error_px: Some(percentile),
    })
}

fn transpose_options(values: [Option<[f64; 2]>; 4]) -> Option<[[f64; 2]; 4]> {
    let mut output = [[0.0; 2]; 4];
    for (target, value) in output.iter_mut().zip(values) {
        *target = value?;
    }
    Some(output)
}

fn sampled_tumbled_errors(
    from_pose: &Pose,
    to_pose: &Pose,
    flat_forward: [f64; 9],
    approximate: [f64; 9],
) -> Option<Vec<f64>> {
    let sample_count = usize::try_from(CHART_STEPS * CHART_STEPS).ok()? * HEIGHT_SAMPLES.len();
    let mut errors = Vec::new();
    errors.try_reserve_exact(sample_count).ok()?;
    for row in 0..CHART_STEPS {
        for column in 0..CHART_STEPS {
            let chart = [
                -1.0 + 2.0 * f64::from(column) / f64::from(CHART_STEPS - 1),
                -1.0 + 2.0 * f64::from(row) / f64::from(CHART_STEPS - 1),
            ];
            let source_chart = apply_homography(flat_forward, chart)?;
            for height in HEIGHT_SAMPLES {
                let destination_ndc = project_presented(to_pose, chart, height)?;
                let expected_source = project_presented(from_pose, source_chart, height)?;
                let approximate_source = apply_homography(approximate, destination_ndc)?;
                let pixel_error = ((approximate_source[0] - expected_source[0])
                    * f64::from(from_pose.grid_width)
                    * 0.5)
                    .hypot(
                        (approximate_source[1] - expected_source[1])
                            * f64::from(from_pose.grid_height)
                            * 0.5,
                    );
                if !pixel_error.is_finite() {
                    return None;
                }
                errors.push(pixel_error);
            }
        }
    }
    Some(errors)
}

fn chart_residual(from: &Pose, to: &Pose) -> f64 {
    let ratio = (from.zoom_log2 - to.zoom_log2).exp2() * f64::from(from.grid_width)
        / f64::from(to.grid_width);
    CHART_CORNERS
        .into_iter()
        .map(|chart| {
            let coordinate = [
                to.centre_from_reference_px[0]
                    + 0.5 * f64::from(to.grid_width) * chart[0],
                to.centre_from_reference_px[1]
                    + 0.5 * f64::from(to.grid_height) * chart[1],
            ];
            let vector = plane_point(to.plane, coordinate).map(|value| ratio * value);
            let projection = plane_projection(from.plane, vector);
            vector
                .into_iter()
                .zip(projection)
                .map(|(value, projected)| (value - projected).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .fold(0.0, f64::max)
}

fn plane_point(plane: Plane, coordinate: [f64; 2]) -> [f64; 4] {
    std::array::from_fn(|axis| {
        f64::from(plane.basis_u[axis])
            .mul_add(coordinate[0], f64::from(plane.basis_v[axis]) * coordinate[1])
    })
}

fn plane_projection(plane: Plane, point: [f64; 4]) -> [f64; 4] {
    let u = dot4(plane.basis_u, point);
    let v = dot4(plane.basis_v, point);
    std::array::from_fn(|axis| {
        f64::from(plane.basis_u[axis]).mul_add(u, f64::from(plane.basis_v[axis]) * v)
    })
}

fn dot4(basis: [f32; 4], point: [f64; 4]) -> f64 {
    basis
        .into_iter()
        .zip(point)
        .fold(0.0, |sum, (component, value)| {
            f64::from(component).mul_add(value, sum)
        })
}

fn project_presented(pose: &Pose, chart: [f64; 2], height: f64) -> Option<[f64; 2]> {
    if pose.grid_width == 0 || pose.grid_height == 0 {
        return None;
    }
    let plane = plane_point(pose.plane, chart);
    let theta_two = f64::midpoint(1.0, 5.0_f64.sqrt()) * pose.view_theta_1;
    let (sine_one, cosine_one) = pose.view_theta_1.sin_cos();
    let (sine_two, cosine_two) = theta_two.sin_cos();
    let rotated = [
        plane[0].mul_add(cosine_one, -plane[1] * sine_one),
        plane[0].mul_add(sine_one, plane[1] * cosine_one),
        plane[2].mul_add(cosine_two, -height * sine_two),
        plane[3],
        plane[2].mul_add(sine_two, height * cosine_two),
    ];
    let denominator_five = PERSPECTIVE_POLE - rotated[4];
    if denominator_five <= POLE_EPSILON {
        return None;
    }
    let scale_five = PERSPECTIVE_POLE / denominator_five;
    let projected_four = [
        rotated[0] * scale_five,
        rotated[1] * scale_five,
        rotated[2] * scale_five,
        rotated[3] * scale_five,
    ];
    let denominator_four = PERSPECTIVE_POLE - projected_four[3];
    if denominator_four <= POLE_EPSILON {
        return None;
    }
    let scale_four = PERSPECTIVE_POLE / denominator_four;
    let world = [
        projected_four[0] * scale_four,
        projected_four[1] * scale_four,
        projected_four[2] * scale_four,
    ];
    const YAW_COSINE: f64 = 0.939_692_620_8;
    const YAW_SINE: f64 = 0.342_020_143_3;
    const PITCH_COSINE: f64 = 0.965_925_826_3;
    const PITCH_SINE: f64 = 0.258_819_045_1;
    let yawed = [
        YAW_COSINE.mul_add(world[0], YAW_SINE * world[2]),
        world[1],
        (-YAW_SINE).mul_add(world[0], YAW_COSINE * world[2]),
    ];
    let view = [
        yawed[0],
        PITCH_COSINE.mul_add(yawed[1], -PITCH_SINE * yawed[2]),
        PITCH_SINE.mul_add(yawed[1], PITCH_COSINE * yawed[2]) - 9.0,
    ];
    let clip_w = -view[2];
    if !clip_w.is_finite() || clip_w <= POLE_EPSILON {
        return None;
    }
    let aspect = f64::from(pose.grid_width) / f64::from(pose.grid_height);
    let ndc = [1.72 * view[0] / aspect / clip_w, 1.72 * view[1] / clip_w];
    ndc.iter().all(|value| value.is_finite()).then_some(ndc)
}

#[cfg(test)]
mod tests {
    use ember_julibrot_kernels::RefinementLevel;
    use ember_julibrot_math::{Plane, ViewMode};

    use super::*;
    use crate::{PaletteId, SampleClass, SubmissionKind, SubmissionMeasurement};

    fn pose(view: ViewMode, theta: f64, displacement: [f64; 2]) -> Pose {
        Pose {
            epoch: 1,
            orbit_generation: 4,
            plane: Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            plane_theta_1: 0.0,
            plane_theta_2: 0.0,
            zoom_log2: 40.0,
            view_theta_1: theta,
            grid_width: 1920,
            grid_height: 1080,
            view,
            centre_from_reference_px: displacement,
        }
    }

    fn frame(pose: Pose) -> SceneFrame {
        SceneFrame {
            scene_id: 3,
            pose,
            palette: PaletteId::Classic,
            iteration_cap: 256,
            level: RefinementLevel::Interactive,
            extent: [1920, 1080],
            texture_index: 0,
            centre_revision: 4,
            plane_origin_f64: [0.0; 4],
            measurement: SubmissionMeasurement {
                kind: SubmissionKind::Scene,
                id: 3,
                source_scene_id: None,
                sample_class: SampleClass::Measured,
                wall_ms: 1.0,
                fence_wait_ms: 0.5,
                polls: 1,
            },
        }
    }

    #[test]
    fn flat_plan_delegates_pan_translation_to_math() {
        let from = pose(ViewMode::Flat, 0.0, [2.0, 3.0]);
        let to = pose(ViewMode::Flat, 0.0, [7.0, -1.0]);
        let plan = Warp::reproject(&frame(from), &from, &to);
        assert_eq!(plan.kind, WarpKind::FlatExact);
        assert!(plan.source_valid);
        assert!((f64::from(plan.rows[0][2]) - 10.0 / 1920.0).abs() < 1.0e-7);
        assert!((f64::from(plan.rows[1][2]) + 8.0 / 1080.0).abs() < 1.0e-7);
    }

    #[test]
    fn pose_mismatch_or_invalid_extent_is_clear_only() {
        let from = pose(ViewMode::Flat, 0.0, [0.0; 2]);
        let mut mismatched = from;
        mismatched.epoch = 2;
        assert_eq!(
            Warp::reproject(&frame(from), &mismatched, &from).kind,
            WarpKind::ClearOnly
        );
        let mut invalid = from;
        invalid.grid_width = 0;
        assert_eq!(
            Warp::reproject(&frame(invalid), &invalid, &from).kind,
            WarpKind::ClearOnly
        );
    }

    #[test]
    fn tumbled_identity_has_neutral_anchors_and_zero_sample_error() {
        let current = pose(ViewMode::Tumbled, 0.6, [0.0; 2]);
        let plan = Warp::reproject(&frame(current), &current, &current);
        assert_eq!(plan.kind, WarpKind::TumbledHomography);
        assert!(plan.source_valid);
        assert!(plan.approx_max_error_px.is_some_and(|error| error < 1.0e-9));
        assert!(plan.approx_p95_error_px.is_some_and(|error| error < 1.0e-9));
    }

    #[test]
    fn tumbled_small_motion_reports_full_error_corpus() {
        let from = pose(ViewMode::Tumbled, 0.6, [0.0; 2]);
        let mut to = pose(ViewMode::Tumbled, 0.61, [2.0, -1.0]);
        to.zoom_log2 += 0.1;
        let plan = Warp::reproject(&frame(from), &from, &to);
        assert_eq!(plan.kind, WarpKind::TumbledHomography);
        assert!(plan.approx_max_error_px.is_some_and(f64::is_finite));
        assert!(plan.approx_p95_error_px.is_some_and(f64::is_finite));
    }
}
