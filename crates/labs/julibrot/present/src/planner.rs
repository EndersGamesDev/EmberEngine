use ember_julibrot_math::{Plane, Pose, PoseMap, PrecisionMode, ViewControls, warp_matrix};

use crate::homography::solve_homogeneous;
use crate::{
    SceneFrame, WarpKind, WarpPlan, WarpValidation, apply_homography, pack_homography_rows,
};

const HEIGHT_SAMPLES: [f64; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];
const SCREEN_STEPS: u32 = 9;
const POLE_EPSILON: f64 = 1.0e-4;
const MAX_CHART_RESIDUAL_PX: f64 = 0.5;

/// Maximum measured reprojection error a displayed warp may move a feature.
pub const WARP_MAX_ERROR_PX: f64 = 1.0;

/// Pure CPU reprojection planner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Warp;

impl Warp {
    /// Builds the one four-screen-corner inverse-sampling plan.
    ///
    /// A pose mismatch or incompatible arithmetic returns an honest clear-only plan. A screen
    /// corner beyond a horizon remains a homogeneous anchor and does not clear the whole picture.
    #[must_use]
    pub fn reproject(
        last_frame: &SceneFrame,
        from_pose: &Pose,
        to_pose: &Pose,
        _precision_mode: PrecisionMode,
        _validation: WarpValidation,
    ) -> WarpPlan {
        if matches!(to_pose.map, PoseMap::EdgeOn) {
            return edge_on();
        }
        if last_frame.pose != *from_pose {
            return clear_only(true);
        }
        if !object_samples_match(from_pose, to_pose) {
            return clear_only(true);
        }
        let Ok(flat) = warp_matrix(from_pose, to_pose) else {
            return clear_only(true);
        };
        let chart_residual = chart_residual(from_pose, to_pose);
        if !chart_residual.is_finite() || chart_residual > MAX_CHART_RESIDUAL_PX {
            return clear_only(true);
        }
        anchor_plan(
            last_frame,
            from_pose,
            to_pose,
            flat.forward,
            chart_residual,
        )
        .map_or_else(|| clear_only(true), enforce_error_ceiling)
    }
}

fn enforce_error_ceiling(mut plan: WarpPlan) -> WarpPlan {
    if plan
        .approx_max_error_px
        .is_some_and(|error| error <= WARP_MAX_ERROR_PX)
    {
        return plan;
    }
    plan.source_scene_id = None;
    plan.source_texture_index = None;
    plan.source_valid = false;
    plan.exposed = true;
    plan.kind = WarpKind::ClearOnly;
    plan
}

fn object_samples_match(from: &Pose, to: &Pose) -> bool {
    from.object
        .as_array()
        .into_iter()
        .zip(to.object.as_array())
        .all(|(from, to)| (from - to).abs() <= f64::from(f32::EPSILON))
}

const fn clear_only(exposed: bool) -> WarpPlan {
    WarpPlan {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        source_scene_id: None,
        source_texture_index: None,
        source_valid: false,
        edge_on: false,
        exposed,
        kind: WarpKind::ClearOnly,
        chart_residual: 0.0,
        approx_max_error_px: None,
        approx_p95_error_px: None,
    }
}

const fn edge_on() -> WarpPlan {
    WarpPlan {
        edge_on: true,
        exposed: false,
        ..clear_only(false)
    }
}

fn anchor_plan(
    last_frame: &SceneFrame,
    from_pose: &Pose,
    to_pose: &Pose,
    flat_forward: [f64; 9],
    chart_residual: f64,
) -> Option<WarpPlan> {
    let source = screen_corners(from_pose).map(|[x, y]| [x, y, 1.0]);
    let destination = screen_corners(from_pose).map(|corner| homogeneous(flat_forward, corner));
    let inverse_sampling = solve_homogeneous(destination, source)?;
    let rows = pack_homography_rows(inverse_sampling)?;
    let displayed_sampling = core::array::from_fn(|index| f64::from(rows[index / 3][index % 3]));
    let metrics = sampled_errors(from_pose, to_pose, displayed_sampling).and_then(|mut errors| {
        if errors.is_empty() {
            return None;
        }
            errors.sort_by(f64::total_cmp);
            let maximum = errors.last().copied()?;
            let percentile_index = errors
                .len()
                .saturating_mul(95)
                .div_ceil(100)
                .saturating_sub(1);
            Some((maximum, *errors.get(percentile_index)?))
        });
    Some(WarpPlan {
        rows,
        source_scene_id: Some(last_frame.scene_id),
        source_texture_index: Some(last_frame.texture_index),
        source_valid: true,
        edge_on: false,
        exposed: warp_exposes_source(inverse_sampling, from_pose, to_pose),
        kind: WarpKind::AnchorHomography,
        chart_residual,
        approx_max_error_px: metrics.map(|value| value.0),
        approx_p95_error_px: metrics.map(|value| value.1),
    })
}

fn warp_exposes_source(inverse_sampling: [f64; 9], from: &Pose, to: &Pose) -> bool {
    let half_width = f64::from(from.grid_width) * 0.5;
    let half_height = f64::from(from.grid_height) * 0.5;
    (0..SCREEN_STEPS)
        .flat_map(|row| (0..SCREEN_STEPS).map(move |column| (row, column)))
        .any(|(row, column)| {
            let target = [
                (f64::from(column) / f64::from(SCREEN_STEPS - 1) - 0.5) * f64::from(to.grid_width),
                (f64::from(row) / f64::from(SCREEN_STEPS - 1) - 0.5) * f64::from(to.grid_height),
            ];
            apply_homography(inverse_sampling, target).is_none_or(|source| {
                source[0] < -half_width
                    || source[0] > half_width
                    || source[1] < -half_height
                    || source[1] > half_height
            })
        })
}

fn screen_corners(pose: &Pose) -> [[f64; 2]; 4] {
    let half_width = f64::from(pose.grid_width) * 0.5;
    let half_height = f64::from(pose.grid_height) * 0.5;
    [
        [-half_width, -half_height],
        [half_width, -half_height],
        [-half_width, half_height],
        [half_width, half_height],
    ]
}

const fn homogeneous(matrix: [f64; 9], point: [f64; 2]) -> [f64; 3] {
    let [x, y] = point;
    [
        matrix[0].mul_add(x, matrix[1].mul_add(y, matrix[2])),
        matrix[3].mul_add(x, matrix[4].mul_add(y, matrix[5])),
        matrix[6].mul_add(x, matrix[7].mul_add(y, matrix[8])),
    ]
}

fn sampled_errors(
    from_pose: &Pose,
    to_pose: &Pose,
    approximate: [f64; 9],
) -> Option<Vec<f64>> {
    let screen_sample_count = usize::try_from(SCREEN_STEPS).ok()?;
    let sample_count = screen_sample_count * screen_sample_count * HEIGHT_SAMPLES.len();
    let mut errors = Vec::new();
    errors.try_reserve_exact(sample_count).ok()?;
    for row in 0..SCREEN_STEPS {
        for column in 0..SCREEN_STEPS {
            let target_screen = [
                (f64::from(column) / f64::from(SCREEN_STEPS - 1) - 0.5)
                    * f64::from(to_pose.grid_width),
                (f64::from(row) / f64::from(SCREEN_STEPS - 1) - 0.5)
                    * f64::from(to_pose.grid_height),
            ];
            let source_screen = apply_homography(approximate, target_screen)?;
            for height in HEIGHT_SAMPLES {
                let destination_relief = project_scene_point(to_pose, target_screen, height)?;
                let expected_source = project_scene_point(from_pose, source_screen, height)?;
                let approximate_source = apply_homography(approximate, destination_relief)?;
                let pixel_error = (approximate_source[0] - expected_source[0])
                    .hypot(approximate_source[1] - expected_source[1]);
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
    let source_pixels_per_chart = 0.25 * f64::from(from.grid_width) * from.zoom_log2.exp2();
    let PoseMap::Mapped(to_map) = to.map else {
        return f64::INFINITY;
    };
    screen_corners(to)
        .into_iter()
        .filter_map(|screen| apply_homography(to_map.rows, screen))
        .map(|offset| {
            let coordinate = [
                to.centre_from_reference_px[0] + offset[0],
                to.centre_from_reference_px[1] + offset[1],
            ];
            let mut vector = plane_point(to.plane, coordinate).map(|value| ratio * value);
            for (axis, value) in vector.iter_mut().enumerate() {
                *value = (to.plane_origin[axis] - from.plane_origin[axis])
                    .mul_add(source_pixels_per_chart, *value);
            }
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

fn ambient_point(plane: Plane, coordinate: [f64; 2], height: f64, view: &ViewControls) -> [f64; 5] {
    let chart = plane_point(plane, coordinate);
    let mut point = [chart[0], chart[1], chart[2], chart[3], height];
    for factor in (0..ViewControls::CAMERA_PLANES.len()).rev() {
        let (first, second) = ViewControls::CAMERA_PLANES[factor];
        let (sine, cosine) = view.camera[factor].sin_cos();
        let a = cosine.mul_add(point[first], -sine * point[second]);
        let b = sine.mul_add(point[first], cosine * point[second]);
        point[first] = a;
        point[second] = b;
    }
    for (coordinate, translation) in point.iter_mut().zip(view.camera_translation) {
        *coordinate += translation;
    }
    point
}

fn plane_point(plane: Plane, coordinate: [f64; 2]) -> [f64; 4] {
    std::array::from_fn(|axis| {
        f64::from(plane.basis_u[axis]).mul_add(
            coordinate[0],
            f64::from(plane.basis_v[axis]) * coordinate[1],
        )
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

/// Mirrors the generated scene WGSL from one grid-screen point through its plane point and relief.
///
/// `record_height` is the escape record's normalized height in `[-2,2]`. `None` means that the
/// projected vertex lies behind one of the perspective poles and the exterior sky remains visible.
#[must_use]
#[allow(
    clippy::float_cmp,
    reason = "height zero is a semantic branch whose exact identity is part of the shader contract"
)]
pub fn project_scene_point(pose: &Pose, screen: [f64; 2], record_height: f64) -> Option<[f64; 2]> {
    project_scene_point_with_shortcut(pose, screen, record_height, true)
}

fn project_scene_point_with_shortcut(
    pose: &Pose,
    screen: [f64; 2],
    record_height: f64,
    flat_shortcut: bool,
) -> Option<[f64; 2]> {
    if pose.grid_width == 0 || pose.grid_height == 0 || !pose.view.is_valid() {
        return None;
    }
    let PoseMap::Mapped(map) = pose.map else {
        return None;
    };
    let mapped_homogeneous = homogeneous(map.rows, screen);
    if mapped_homogeneous[2] <= 0.0 || !mapped_homogeneous[2].is_finite() {
        return None;
    }
    let mapped = [
        mapped_homogeneous[0] / mapped_homogeneous[2],
        mapped_homogeneous[1] / mapped_homogeneous[2],
    ];
    let height = pose.view.height_scale * record_height;
    if flat_shortcut && height == 0.0 {
        return Some(screen);
    }
    let chart_coordinate = [
        4.0 * mapped[0] / f64::from(pose.grid_width),
        4.0 * mapped[1] / f64::from(pose.grid_width),
    ];
    let rotated = ambient_point(pose.plane, chart_coordinate, height, &pose.view);
    let distance_five = pose.view.distance_five;
    let distance_four = pose.view.distance_four;
    let denominator_five = distance_five - rotated[4];
    if denominator_five <= POLE_EPSILON {
        return None;
    }
    let scale_five = distance_five / denominator_five;
    let projected_four = [
        rotated[0] * scale_five,
        rotated[1] * scale_five,
        rotated[2] * scale_five,
        rotated[3] * scale_five,
    ];
    let denominator_four = distance_four - projected_four[3];
    if denominator_four <= POLE_EPSILON {
        return None;
    }
    let scale_four = distance_four / denominator_four;
    let world = [
        projected_four[0] * scale_four,
        projected_four[1] * scale_four,
        projected_four[2] * scale_four,
    ];
    let (yaw_sine, yaw_cosine) = pose.view.camera_yaw.sin_cos();
    let (pitch_sine, pitch_cosine) = pose.view.camera_pitch.sin_cos();
    let yawed = [
        yaw_cosine.mul_add(world[0], yaw_sine * world[2]),
        world[1],
        (-yaw_sine).mul_add(world[0], yaw_cosine * world[2]),
    ];
    let view = [
        yawed[0],
        pitch_cosine.mul_add(yawed[1], -pitch_sine * yawed[2]),
        pitch_sine.mul_add(yawed[1], pitch_cosine * yawed[2]) - distance_four,
    ];
    let clip_w = -view[2];
    if !clip_w.is_finite() || clip_w <= POLE_EPSILON {
        return None;
    }
    let aspect = f64::from(pose.grid_width) / f64::from(pose.grid_height);
    let perspective_scale = aspect * distance_four * 0.5;
    let ndc = [
        perspective_scale * view[0] / aspect / clip_w,
        perspective_scale * view[1] / clip_w,
    ];
    let projected = [
        ndc[0] * f64::from(pose.grid_width) * 0.5,
        ndc[1] * f64::from(pose.grid_height) * 0.5,
    ];
    projected
        .iter()
        .all(|value| value.is_finite())
        .then_some(projected)
}

#[cfg(test)]
mod tests {
    use ember_julibrot_kernels::RefinementLevel;
    use ember_julibrot_math::{
        Homography, ObjectAngles, Plane, PlaneAngles, PoseMap, ViewControls, construct_plane,
        screen_to_plane,
    };

    use super::*;
    use crate::{PaletteId, SampleClass, SubmissionKind, SubmissionMeasurement};

    const SWEEP_ANGLES: u32 = 256;
    const RELIEF_YAW: f64 = 0.349;
    const RELIEF_PITCH: f64 = 0.262;

    fn relief(theta: f64) -> ViewControls {
        let mut camera = [0.0; 10];
        camera[0] = theta;
        camera[8] = f64::midpoint(1.0, 5.0_f64.sqrt()) * theta;
        ViewControls {
            camera,
            camera_yaw: RELIEF_YAW,
            camera_pitch: RELIEF_PITCH,
            height_scale: 1.0,
            ..ViewControls::NEUTRAL
        }
    }

    fn faced_relief(object: ObjectAngles, theta: f64) -> ViewControls {
        let mut view = relief(theta);
        view.camera[1] = -core::f64::consts::FRAC_PI_2 - object.rho_13;
        view.camera[4] = -core::f64::consts::FRAC_PI_2 - object.rho_24;
        view
    }

    fn map(object: ObjectAngles, view: ViewControls, extent: [u32; 2]) -> Homography {
        screen_to_plane(
            &object,
            &view,
            40.0,
            extent[0],
            extent[1],
            f64::from(extent[0]) / f64::from(extent[1]),
        )
        .expect("fixture map is invertible")
    }

    fn pose(view: ViewControls, displacement: [f64; 2]) -> Pose {
        let extent = [1920, 1080];
        let object = ObjectAngles::JULIA;
        Pose {
            epoch: 1,
            orbit_generation: 4,
            plane: Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            object,
            plane_origin: [0.0; 4],
            zoom_log2: 40.0,
            view,
            grid_width: extent[0],
            grid_height: extent[1],
            map: PoseMap::Mapped(map(object, view, extent)),
            centre_from_reference_px: displacement,
        }
    }

    fn object_pose(
        object: ObjectAngles,
        plane: Plane,
        view: ViewControls,
        displacement: [f64; 2],
    ) -> Pose {
        let mut posed = pose(ViewControls::NEUTRAL, displacement);
        posed.object = object;
        posed.plane = plane;
        posed.view = view;
        posed.map = PoseMap::Mapped(map(object, view, [posed.grid_width, posed.grid_height]));
        posed
    }

    fn set_extent(pose: &mut Pose, extent: [u32; 2]) {
        pose.grid_width = extent[0];
        pose.grid_height = extent[1];
        pose.map = PoseMap::Mapped(map(pose.object, pose.view, extent));
    }

    fn frame(pose: &Pose) -> SceneFrame {
        SceneFrame {
            scene_id: 3,
            pose: *pose,
            palette: PaletteId::Classic,
            iteration_cap: 256,
            level: RefinementLevel::Interactive,
            extent: [pose.grid_width, pose.grid_height],
            texture_index: 0,
            centre_revision: 4,
            plane_origin_f64: [0.0; 4],
            precision_mode: PrecisionMode::PictureFast.as_str(),
            measurement: SubmissionMeasurement {
                kind: SubmissionKind::Scene,
                id: 3,
                source_scene_id: None,
                sample_class: SampleClass::Measured,
                precision_mode: PrecisionMode::PictureFast.as_str(),
                wall_ms: 1.0,
                fence_wait_ms: 0.5,
                polls: 1,
            },
        }
    }

    fn reproject(last_frame: &SceneFrame, from_pose: &Pose, to_pose: &Pose) -> WarpPlan {
        Warp::reproject(
            last_frame,
            from_pose,
            to_pose,
            PrecisionMode::PictureFast,
            WarpValidation::Ordinary,
        )
    }

    fn unpack_rows(rows: [[f32; 4]; 3]) -> [f64; 9] {
        core::array::from_fn(|index| f64::from(rows[index / 3][index % 3]))
    }

    #[test]
    fn neutral_plan_reproduces_inverse_sampling_translation() {
        let from = pose(ViewControls::NEUTRAL, [2.0, 3.0]);
        let to = pose(ViewControls::NEUTRAL, [7.0, -1.0]);
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::AnchorHomography);
        assert!(plan.source_valid);
        assert!((f64::from(plan.rows[0][2]) - 5.0).abs() < 1.0e-7);
        assert!((f64::from(plan.rows[1][2]) + 4.0).abs() < 1.0e-7);
    }

    #[test]
    fn slice_change_clears_above_the_plane_rounding_floor() {
        let from = pose(ViewControls::NEUTRAL, [0.0; 2]);
        let mut large = from;
        large.object.rho_12 += 0.3;
        assert_eq!(
            reproject(&frame(&from), &from, &large).kind,
            WarpKind::ClearOnly
        );

        let mut rounded = from;
        rounded.object.rho_12 += 1.0e-9;
        assert_eq!(
            reproject(&frame(&from), &from, &rounded).kind,
            WarpKind::AnchorHomography
        );

        let mut panned = from;
        panned.centre_from_reference_px = [8.0, -5.0];
        assert_eq!(
            reproject(&frame(&from), &from, &panned).kind,
            WarpKind::AnchorHomography
        );
    }

    #[test]
    fn neutral_plan_is_math_inverse_at_all_distances() {
        for distance in [2.0, 8.0, 64.0] {
            let view = ViewControls {
                distance_five: distance,
                distance_four: distance,
                ..ViewControls::NEUTRAL
            };
            let from = pose(view, [13.25, -7.5]);
            let mut to = pose(view, [-4.0, 9.125]);
            to.zoom_log2 += 0.75;
            let exact = warp_matrix(&from, &to).expect("neutral fixture is finite");
            let plan = reproject(&frame(&from), &from, &to);
            let scale = exact.inverse[8];
            for (value, wanted) in unpack_rows(plan.rows).into_iter().zip(exact.inverse) {
                assert!((value - wanted / scale).abs() <= 1.0e-5);
            }
            assert!(plan.approx_max_error_px.is_some_and(|error| error < 1.0e-9));
        }
    }

    #[test]
    fn height_zero_shortcut_matches_the_full_forward_chain() {
        let mut moved_camera = [0.0; 10];
        moved_camera[0] = 0.6;
        moved_camera[8] = -0.3;
        let views = [
            ViewControls::NEUTRAL,
            ViewControls {
                camera: moved_camera,
                camera_translation: [0.2, -0.1, 0.3, 0.05, -0.2],
                camera_yaw: RELIEF_YAW,
                camera_pitch: RELIEF_PITCH,
                height_scale: 1.0,
                distance_five: 6.0,
                distance_four: 9.0,
                ..ViewControls::NEUTRAL
            },
        ];
        for extent in [[1920, 1080], [1024, 1024]] {
            for view in views {
                let mut posed = pose(view, [0.0; 2]);
                set_extent(&mut posed, extent);
                for row in 0..SCREEN_STEPS {
                    for column in 0..SCREEN_STEPS {
                        let screen = [
                            (f64::from(column) / f64::from(SCREEN_STEPS - 1) - 0.5)
                                * f64::from(extent[0]),
                            (f64::from(row) / f64::from(SCREEN_STEPS - 1) - 0.5)
                                * f64::from(extent[1]),
                        ];
                        let shortcut = project_scene_point(&posed, screen, 0.0)
                            .expect("the flat shortcut projects");
                        let full = project_scene_point_with_shortcut(&posed, screen, 0.0, false)
                            .expect("the full flat chain projects");
                        assert_eq!(shortcut, screen);
                        let error = (shortcut[0] - full[0]).hypot(shortcut[1] - full[1]);
                        assert!(error <= 1.0e-9, "full-chain error was {error} px");
                    }
                }
            }
        }
    }

    #[test]
    fn uploaded_inverse_rows_stay_within_quarter_pixel() {
        for zoom_log2 in [0.0, 10.0, 20.0, 40.0, 80.0, 100.0] {
            let mut from = pose(ViewControls::NEUTRAL, [1_003.25, -507.5]);
            from.zoom_log2 = zoom_log2;
            let mut to = pose(ViewControls::NEUTRAL, [-811.125, 401.75]);
            to.zoom_log2 = zoom_log2 + 0.2;
            let exact = warp_matrix(&from, &to).expect("required warp fixture is finite");
            let plan = reproject(&frame(&from), &from, &to);
            let rounded = unpack_rows(plan.rows);
            for screen in screen_corners(&to).into_iter().chain([[0.0; 2]]) {
                let expected = apply_homography(exact.inverse, screen)
                    .expect("exact fixture has no projective pole");
                let actual = apply_homography(rounded, screen)
                    .expect("rounded fixture has no projective pole");
                let error_px = (actual[0] - expected[0]).hypot(actual[1] - expected[1]);
                assert!(error_px <= 0.25, "zoom {zoom_log2} error was {error_px} px");
            }
        }
    }

    #[test]
    fn pose_mismatch_or_invalid_extent_is_clear_only() {
        let from = pose(ViewControls::NEUTRAL, [0.0; 2]);
        let mut mismatched = from;
        mismatched.epoch = 2;
        assert_eq!(
            reproject(&frame(&from), &mismatched, &from).kind,
            WarpKind::ClearOnly
        );
        let mut invalid = from;
        invalid.grid_width = 0;
        assert_eq!(
            reproject(&frame(&invalid), &invalid, &from).kind,
            WarpKind::ClearOnly
        );
        let mut edge_on = from;
        edge_on.map = PoseMap::EdgeOn;
        let plan = reproject(&frame(&from), &from, &edge_on);
        assert!(plan.edge_on);
        assert!(!plan.exposed);
    }

    #[test]
    fn relief_identity_has_zero_sample_error() {
        let current = pose(relief(0.6), [0.0; 2]);
        let plan = reproject(&frame(&current), &current, &current);
        assert_eq!(plan.kind, WarpKind::AnchorHomography);
        assert!(plan.source_valid);
        assert!(plan.approx_max_error_px.is_some_and(|error| error < 1.0e-9));
    }

    #[test]
    fn a_horizon_inside_the_destination_frame_refuses_the_plan() {
        let from = pose(ViewControls::NEUTRAL, [0.0; 2]);
        let mut to = from;
        to.map = PoseMap::Mapped(Homography {
            rows: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.002, 0.0, 1.0],
            inverse: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -0.002, 0.0, 1.0],
            condition_number: 1.004,
        });
        let PoseMap::Mapped(to_map) = to.map else {
            panic!("fixture must be mapped");
        };
        let weights = screen_corners(&from).map(|corner| homogeneous(to_map.inverse, corner)[2]);
        assert!(weights.iter().any(|weight| *weight < 0.0));
        assert!(weights.iter().any(|weight| *weight > 0.0));
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::ClearOnly);
        assert!(!plan.source_valid);
    }

    #[test]
    fn relief_plan_above_the_published_ceiling_is_refused() {
        let from = pose(relief(0.6), [0.0; 2]);
        let mut to = pose(relief(0.8), [8.0, -4.0]);
        to.zoom_log2 += 0.25;
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::ClearOnly);
        assert!(!plan.source_valid);
        let maximum = plan
            .approx_max_error_px
            .expect("the sampled corpus reports a maximum");
        assert!(
            maximum > WARP_MAX_ERROR_PX,
            "maximum error was {maximum} pixels"
        );
        assert!(
            plan.approx_p95_error_px
                .is_some_and(|error| error <= maximum)
        );
    }

    fn counterexample_view(
        distance_five: f64,
        distance_four: f64,
        camera: [f64; 3],
        observer: [f64; 2],
        translation: [f64; 5],
    ) -> ViewControls {
        let mut factors = [0.0; 10];
        factors[0] = camera[0];
        factors[6] = camera[1];
        factors[8] = camera[2];
        ViewControls {
            camera: factors,
            camera_translation: translation,
            camera_yaw: observer[0],
            camera_pitch: observer[1],
            height_scale: 1.0,
            distance_five,
            distance_four,
        }
    }

    #[test]
    fn reviewers_pole_counterexamples_are_fail_closed() {
        let cases = [
            (
                counterexample_view(
                    1.682,
                    3.947,
                    [0.6371, -1.3451, -0.7003],
                    [0.2566, -0.4173],
                    [0.6332, 0.6895, -0.7479, -0.2055, -0.4357],
                ),
                counterexample_view(
                    1.682,
                    3.947,
                    [0.6524, -1.3451, -0.7088],
                    [0.2566, -0.4173],
                    [0.6516, 0.6744, -0.7417, -0.1976, -0.4433],
                ),
                [-0.64, -5.663],
                39.965,
            ),
            (
                counterexample_view(
                    4.235,
                    5.195,
                    [0.2422, -1.3395, 0.9025],
                    [-0.1128, 0.1702],
                    [0.9049, -0.671, -0.8312, -0.2096, 0.9151],
                ),
                counterexample_view(
                    4.235,
                    5.195,
                    [0.2432, -1.3395, 0.8874],
                    [-0.1128, 0.1702],
                    [0.9111, -0.6721, -0.8317, -0.2196, 0.9234],
                ),
                [-0.72, 1.583],
                39.9503,
            ),
        ];
        for (from_view, to_view, displacement, zoom_log2) in cases {
            let from = pose(from_view, [0.0; 2]);
            let mut to = pose(to_view, displacement);
            to.zoom_log2 = zoom_log2;
            let plan = reproject(&frame(&from), &from, &to);
            assert_eq!(plan.kind, WarpKind::ClearOnly);
            assert!(!plan.source_valid);
        }
    }

    #[test]
    fn a_perspective_pole_on_the_sampled_surface_is_unbounded() {
        let view = ViewControls {
            height_scale: 1.0,
            distance_five: 1.0,
            ..ViewControls::NEUTRAL
        };
        let current = pose(view, [0.0; 2]);
        let plan = reproject(&frame(&current), &current, &current);
        assert_eq!(plan.kind, WarpKind::ClearOnly);
        assert!(!plan.source_valid);
        assert_eq!(plan.approx_max_error_px, None);
    }

    #[test]
    fn flat_plan_is_exact_and_remains_displayable() {
        let from = pose(ViewControls::NEUTRAL, [3.0, -2.0]);
        let mut to = pose(ViewControls::NEUTRAL, [11.0, 7.0]);
        to.zoom_log2 += 0.5;
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::AnchorHomography);
        assert!(plan.source_valid);
        assert!(plan.approx_max_error_px.is_some_and(|error| error < 1.0e-9));
    }

    #[test]
    fn pure_camera_translation_is_an_exact_flat_warp() {
        let base_view = ViewControls {
            camera_yaw: 0.17,
            camera_pitch: -0.11,
            height_scale: 1.0,
            ..relief(0.2)
        };
        let from = pose(base_view, [0.0; 2]);
        let translated_view = ViewControls {
            camera_translation: [0.2, -0.1, 0.3, -0.05, 0.1],
            ..base_view
        };
        let to = pose(translated_view, [0.0; 2]);
        let inverse = warp_matrix(&from, &to)
            .expect("translated flat warp is finite")
            .inverse;
        for target in [[-321.0, 117.0], [0.0, 0.0], [287.0, -91.0]] {
            let source_flat = apply_homography(inverse, target).expect("flat source is finite");
            let destination = project_scene_point_with_shortcut(&to, target, 0.0, false)
                .expect("translated full chain projects");
            let expected = project_scene_point_with_shortcut(&from, source_flat, 0.0, false)
                .expect("source full chain projects");
            let actual = apply_homography(inverse, destination).expect("warp projects");
            let error = (actual[0] - expected[0]).hypot(actual[1] - expected[1]);
            assert!(error <= 1.0e-9, "translated flat error was {error} px");
        }
    }

    #[test]
    fn relief_translation_is_measured_through_the_affine_camera() {
        let from = pose(relief(0.2), [0.0; 2]);
        let mut translated = relief(0.2);
        translated.camera_translation = [1.0e-4, -2.0e-4, 1.5e-4, 0.0, 1.0e-4];
        let to = pose(translated, [0.0; 2]);
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::AnchorHomography);
        let maximum = plan.approx_max_error_px.expect("translation is measured");
        assert!(maximum > 0.0 && maximum <= WARP_MAX_ERROR_PX);
        let with_translation = project_scene_point(&to, [200.0, -100.0], 1.0)
            .expect("translated relief projects");
        let without_translation = project_scene_point(&from, [200.0, -100.0], 1.0)
            .expect("untranslated relief projects");
        assert_ne!(with_translation, without_translation);
    }

    #[test]
    fn plane_origin_translation_distinguishes_pan_from_slice_change() {
        let from = pose(ViewControls::NEUTRAL, [0.0; 2]);
        let mut in_plane = from;
        in_plane.plane_origin = [1.0e-12, -2.0e-12, 0.0, 0.0];
        assert_eq!(
            reproject(&frame(&from), &from, &in_plane).kind,
            WarpKind::AnchorHomography
        );

        let mut out_of_plane = from;
        out_of_plane.plane_origin = [0.0, 0.0, 1.0e-9, 0.0];
        assert_eq!(
            reproject(&frame(&from), &from, &out_of_plane).kind,
            WarpKind::ClearOnly
        );
    }

    #[test]
    fn every_validation_mode_measures_the_full_corpus() {
        let from = pose(ViewControls::NEUTRAL, [0.0; 2]);
        let mut to = pose(ViewControls::NEUTRAL, [5.0, -3.0]);
        to.zoom_log2 += 0.1;
        let ordinary = reproject(&frame(&from), &from, &to);
        let approximate = unpack_rows(ordinary.rows);
        let full = sampled_errors(&from, &to, approximate)
            .expect("the full validation corpus projects");
        assert_eq!(full.len(), 9 * 9 * HEIGHT_SAMPLES.len());
        assert_eq!(ordinary.approx_max_error_px, full.iter().copied().reduce(f64::max));

        let measured = Warp::reproject(
            &frame(&from),
            &from,
            &to,
            PrecisionMode::PictureFast,
            WarpValidation::Measure,
        );
        assert_eq!(measured.approx_max_error_px, full.into_iter().reduce(f64::max));
    }

    #[test]
    fn full_corpus_cost_probe_reports_every_planned_sample() {
        let from = pose(relief(0.2), [0.0; 2]);
        let mut moved = relief(0.2);
        moved.camera[8] += 1.0e-5;
        let to = pose(moved, [0.0; 2]);
        let started = std::time::Instant::now();
        let plans = 1_536_u32;
        for _ in 0..plans {
            let plan = std::hint::black_box(reproject(&frame(&from), &from, &to));
            assert_eq!(plan.kind, WarpKind::AnchorHomography);
            assert!(plan.approx_max_error_px.is_some());
        }
        let elapsed = started.elapsed();
        eprintln!(
            "full warp corpus: {plans} plans in {:.6}s, {:.6}ms/plan",
            elapsed.as_secs_f64(),
            elapsed.as_secs_f64() * 1_000.0 / f64::from(plans)
        );
    }

    fn named_objects() -> [(ObjectAngles, Plane); 3] {
        let quarter = core::f64::consts::FRAC_PI_2;
        let angles = [
            PlaneAngles {
                theta_1: 0.0,
                theta_2: 0.0,
            },
            PlaneAngles {
                theta_1: -quarter,
                theta_2: -quarter,
            },
            PlaneAngles {
                theta_1: core::f64::consts::FRAC_PI_4,
                theta_2: core::f64::consts::FRAC_PI_4,
            },
        ];
        angles.map(|legacy| {
            let object = ObjectAngles::from(legacy);
            (
                object,
                construct_plane(object).expect("validated object constructs a plane"),
            )
        })
    }

    #[test]
    fn every_plane_keeps_a_full_screen_plan_through_a_view_turn() {
        let mut observed_max = 0.0_f64;
        let mut observed_p95 = 0.0_f64;
        let mut refused = 0_u32;
        for (object, plane) in named_objects() {
            for step in 0..SWEEP_ANGLES {
                let theta = -0.6 + 1.2 * f64::from(step) / f64::from(SWEEP_ANGLES - 1);
                let from_view = faced_relief(object, theta);
                let to_view = faced_relief(object, theta + 0.002);
                let from = object_pose(object, plane, from_view, [3.5, -2.25]);
                let mut to = object_pose(object, plane, to_view, [5.5, -1.25]);
                to.zoom_log2 += 0.025;
                let plan = Warp::reproject(
                    &frame(&from),
                    &from,
                    &to,
                    PrecisionMode::PictureFast,
                    WarpValidation::Measure,
                );
                if plan.kind == WarpKind::ClearOnly {
                    refused = refused.saturating_add(1);
                }
                let error = plan
                    .approx_max_error_px
                    .expect("the swept plan reports a sampled maximum");
                let p95 = plan
                    .approx_p95_error_px
                    .expect("the swept plan reports a sampled percentile");
                assert!(p95 <= error);
                observed_max = observed_max.max(error);
                observed_p95 = observed_p95.max(p95);
            }
        }
        assert!(
            observed_max <= 47.0,
            "swept maximum was {observed_max} pixels"
        );
        assert!(
            observed_p95 <= 32.0,
            "swept p95 maximum was {observed_p95} pixels"
        );
        assert!(
            refused > 0,
            "the measured relief envelope never reached the ceiling"
        );
    }

    fn triangle_contains(point: [f64; 2], triangle: [[f64; 2]; 3]) -> bool {
        let cross = |a: [f64; 2], b: [f64; 2]| a[0].mul_add(b[1], -a[1] * b[0]);
        let edges = [
            cross(
                [
                    triangle[1][0] - triangle[0][0],
                    triangle[1][1] - triangle[0][1],
                ],
                [point[0] - triangle[0][0], point[1] - triangle[0][1]],
            ),
            cross(
                [
                    triangle[2][0] - triangle[1][0],
                    triangle[2][1] - triangle[1][1],
                ],
                [point[0] - triangle[1][0], point[1] - triangle[1][1]],
            ),
            cross(
                [
                    triangle[0][0] - triangle[2][0],
                    triangle[0][1] - triangle[2][1],
                ],
                [point[0] - triangle[2][0], point[1] - triangle[2][1]],
            ),
        ];
        edges.iter().all(|value| *value >= -1.0e-9) || edges.iter().all(|value| *value <= 1.0e-9)
    }

    fn mesh_covers(pose: &Pose, target: [f64; 2]) -> bool {
        let vertex = |column: u32, row: u32| {
            project_scene_point(
                pose,
                [
                    0.5_f64.mul_add(-f64::from(pose.grid_width), f64::from(column) + 0.5),
                    0.5_f64.mul_add(-f64::from(pose.grid_height), f64::from(row) + 0.5),
                ],
                2.0,
            )
        };
        (0..pose.grid_height.saturating_sub(1)).any(|row| {
            (0..pose.grid_width.saturating_sub(1)).any(|column| {
                let vertices = [
                    vertex(column, row),
                    vertex(column + 1, row),
                    vertex(column, row + 1),
                    vertex(column + 1, row + 1),
                ];
                [[0, 1, 2], [1, 3, 2]].into_iter().any(|indices| {
                    match indices.map(|index| vertices[index]) {
                        [Some(a), Some(b), Some(c)] => triangle_contains(target, [a, b, c]),
                        _ => false,
                    }
                })
            })
        })
    }

    #[test]
    fn completed_scene_surface_is_mesh_or_exterior_sky_over_pose_lattice() {
        let extent = [17, 9];
        let mut near_edge_camera = [0.0; 10];
        near_edge_camera[6] = 1.2;
        let views = [
            ViewControls::NEUTRAL,
            ViewControls {
                height_scale: 2.0,
                ..relief(0.6)
            },
            ViewControls {
                camera: near_edge_camera,
                camera_translation: [0.3, -0.2, 0.1, 0.0, 0.2],
                height_scale: 2.0,
                distance_five: 2.0,
                ..ViewControls::NEUTRAL
            },
        ];
        let poses = views.map(|view| {
            let mut posed = pose(view, [0.0; 2]);
            set_extent(&mut posed, extent);
            posed
        });
        let mut edge_on = poses[0];
        edge_on.map = PoseMap::EdgeOn;
        let sky = crate::exterior_zero(crate::CLASSIC_PALETTE);
        let scene_load = crate::gpu::scene_load_color(crate::CLASSIC_PALETTE);
        let expected_sky = wgpu::Color {
            r: f64::from(sky[0]),
            g: f64::from(sky[1]),
            b: f64::from(sky[2]),
            a: f64::from(sky[3]),
        };
        let clear = crate::CLASSIC_PALETTE.clear_rgba;
        let clear = wgpu::Color {
            r: f64::from(clear[0]),
            g: f64::from(clear[1]),
            b: f64::from(clear[2]),
            a: f64::from(clear[3]),
        };
        assert_eq!(scene_load, expected_sky);
        assert_ne!(scene_load, clear);
        let mut saw_mesh = false;
        let mut saw_sky = false;
        for posed in poses.into_iter().chain([edge_on]) {
            for row in 0..extent[1] {
                for column in 0..extent[0] {
                    let target = [
                        0.5_f64.mul_add(-f64::from(extent[0]), f64::from(column) + 0.5),
                        0.5_f64.mul_add(-f64::from(extent[1]), f64::from(row) + 0.5),
                    ];
                    if mesh_covers(&posed, target) {
                        saw_mesh = true;
                    } else {
                        saw_sky = true;
                        assert_eq!(scene_load, expected_sky, "pixel {column},{row} was not sky");
                        assert_ne!(scene_load, clear, "pixel {column},{row} used clear colour");
                    }
                }
            }
        }
        assert!(saw_mesh);
        assert!(saw_sky);
    }
}
