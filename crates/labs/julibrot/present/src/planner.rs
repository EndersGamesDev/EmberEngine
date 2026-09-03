use ember_julibrot_math::{Plane, Pose, PrecisionMode, warp_matrix};

use crate::{
    SceneFrame, WarpKind, WarpPlan, WarpValidation, apply_homography, pack_homography_rows,
    solve_homography,
};

const CHART_CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];
const HEIGHT_SAMPLES: [f64; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];
const CHART_STEPS: u32 = 9;
const POLE_EPSILON: f64 = 1.0e-4;

/// Pure CPU reprojection planner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Warp;

impl Warp {
    /// Builds the one four-anchor inverse-sampling plan.
    ///
    /// A pose mismatch, incompatible arithmetic, or projection pole returns an honest clear-only
    /// plan. This function never allocates a GPU resource, submits work, or mutates the frame.
    #[must_use]
    pub fn reproject(
        last_frame: &SceneFrame,
        from_pose: &Pose,
        to_pose: &Pose,
        precision_mode: PrecisionMode,
        validation: WarpValidation,
    ) -> WarpPlan {
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
        let Some((mut plan, approximate)) =
            anchor_plan(from_pose, to_pose, flat.forward, chart_residual)
        else {
            return clear_only();
        };
        if validation.samples_corpus(precision_mode) {
            let Some((maximum, percentile)) =
                sampled_error_summary(from_pose, to_pose, flat.forward, approximate)
            else {
                return clear_only();
            };
            plan.approx_max_error_px = Some(maximum);
            plan.approx_p95_error_px = Some(percentile);
        }
        plan
    }
}

const fn clear_only() -> WarpPlan {
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

fn anchor_plan(
    from_pose: &Pose,
    to_pose: &Pose,
    flat_forward: [f64; 9],
    chart_residual: f64,
) -> Option<(WarpPlan, [f64; 9])> {
    let destination = CHART_CORNERS.map(|chart| project_presented(to_pose, chart, 0.0));
    let source = CHART_CORNERS.map(|chart| {
        let source_chart = apply_homography(flat_forward, chart)?;
        project_presented(from_pose, source_chart, 0.0)
    });
    let destination = transpose_options(destination)?;
    let source = transpose_options(source)?;
    let matrix = solve_homography(destination, source)?;
    let rows = pack_homography_rows(matrix)?;
    Some((
        WarpPlan {
            rows,
            source_valid: true,
            kind: WarpKind::AnchorHomography,
            chart_residual,
            approx_max_error_px: None,
            approx_p95_error_px: None,
        },
        matrix,
    ))
}

fn sampled_error_summary(
    from_pose: &Pose,
    to_pose: &Pose,
    flat_forward: [f64; 9],
    approximate: [f64; 9],
) -> Option<(f64, f64)> {
    let mut errors = sampled_errors(from_pose, to_pose, flat_forward, approximate)?;
    errors.sort_by(f64::total_cmp);
    let maximum = errors.last().copied()?;
    let percentile_index = errors
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    let percentile = *errors.get(percentile_index)?;
    Some((maximum, percentile))
}

fn transpose_options(values: [Option<[f64; 2]>; 4]) -> Option<[[f64; 2]; 4]> {
    let mut output = [[0.0; 2]; 4];
    for (target, value) in output.iter_mut().zip(values) {
        *target = value?;
    }
    Some(output)
}

fn sampled_errors(
    from_pose: &Pose,
    to_pose: &Pose,
    flat_forward: [f64; 9],
    approximate: [f64; 9],
) -> Option<Vec<f64>> {
    #[cfg(test)]
    SAMPLED_CORPUS_RUNS.with(|runs| runs.set(runs.get().saturating_add(1)));
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

#[cfg(test)]
thread_local! {
    static SAMPLED_CORPUS_RUNS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn chart_residual(from: &Pose, to: &Pose) -> f64 {
    let ratio = (from.zoom_log2 - to.zoom_log2).exp2() * f64::from(from.grid_width)
        / f64::from(to.grid_width);
    CHART_CORNERS
        .into_iter()
        .map(|chart| {
            let coordinate = [
                (0.5 * f64::from(to.grid_width)).mul_add(chart[0], to.centre_from_reference_px[0]),
                (0.5 * f64::from(to.grid_height)).mul_add(chart[1], to.centre_from_reference_px[1]),
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

/// Builds the five-dimensional VIEW-rotated point in the chart's own display frame.
///
/// The chart is an isometric two-plane, so its two spanning directions are display axes one and
/// two and escape height is display axis five; the plane's ambient orientation names which fractal
/// axes are sampled and never places the geometry. Embedding the ambient components directly would
/// make the picture depend on which axes a preset happens to name and would annihilate world `x`
/// and `y` for every plane missing `span(e1,e2)` — the Mandelbrot seed `(e3,e4)` among them. The
/// display frame has no such plane.
///
/// `rotation` is `[cos θᵥ₁, sin θᵥ₁, cos θᵥ₂, sin θᵥ₂]` for the VIEW rotation `R12(θᵥ₁)R35(θᵥ₂)`.
fn display_point(coordinate: [f64; 2], height: f64, rotation: [f64; 4]) -> [f64; 5] {
    let [cosine_one, sine_one, cosine_two, sine_two] = rotation;
    [
        coordinate[0].mul_add(cosine_one, -coordinate[1] * sine_one),
        coordinate[0].mul_add(sine_one, coordinate[1] * cosine_one),
        -height * sine_two,
        0.0,
        height * cosine_two,
    ]
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

/// Projects one chart point at one record height through the pose's own VIEW controls.
///
/// `record_height` is the escape record's own `H`; the pose's height control scales it, so a pose
/// at `height_scale = 0` projects every sample to the height-zero chart no matter what `H` is.
///
/// The observer's perspective scale is `aspect·d₄/2`, which is the whole reason the height-zero
/// image is exact: at world `z = 0` the divide is by `d₄`, the two cancel, and NDC reduces to
/// `(x/2, aspect·y/2)` for every `d₄` and every extent.
fn project_presented(pose: &Pose, chart: [f64; 2], record_height: f64) -> Option<[f64; 2]> {
    if pose.grid_width == 0 || pose.grid_height == 0 || !pose.view.is_valid() {
        return None;
    }
    let display_coordinate = [
        2.0 * chart[0],
        2.0 * f64::from(pose.grid_height) / f64::from(pose.grid_width) * chart[1],
    ];
    let (sine_one, cosine_one) = pose.view.theta_1.sin_cos();
    let (sine_two, cosine_two) = pose.view.theta_2.sin_cos();
    let rotation = [cosine_one, sine_one, cosine_two, sine_two];
    let height = pose.view.height_scale * record_height;
    let rotated = display_point(display_coordinate, height, rotation);
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
    ndc.iter().all(|value| value.is_finite()).then_some(ndc)
}

#[cfg(test)]
mod tests {
    use ember_julibrot_kernels::RefinementLevel;
    use ember_julibrot_math::{
        MathError, Plane, PlaneAngles, PrecisionMode, ViewControls, construct_plane,
    };

    use super::*;
    use crate::{PaletteId, SampleClass, SubmissionKind, SubmissionMeasurement};

    const SWEEP_ANGLES: u32 = 256;
    const RELIEF_YAW: f64 = 0.349;
    const RELIEF_PITCH: f64 = 0.262;
    const CONFORMANCE_MODE: PrecisionMode = PrecisionMode::Deterministic;

    /// One relief row parameterised by a single swept angle.
    ///
    /// The second VIEW angle is the golden-ratio multiple of the first only so that this sweep
    /// covers the geometry the retired clock covered; nothing in the lab derives it that way.
    fn relief(theta: f64) -> ViewControls {
        ViewControls {
            theta_1: theta,
            theta_2: f64::midpoint(1.0, 5.0_f64.sqrt()) * theta,
            camera_yaw: RELIEF_YAW,
            camera_pitch: RELIEF_PITCH,
            height_scale: 1.0,
            ..ViewControls::NEUTRAL
        }
    }

    fn pose(view: ViewControls, displacement: [f64; 2]) -> Pose {
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
            view,
            grid_width: 1920,
            grid_height: 1080,
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
            precision_mode: CONFORMANCE_MODE.as_str(),
            measurement: SubmissionMeasurement {
                kind: SubmissionKind::Scene,
                id: 3,
                source_scene_id: None,
                sample_class: SampleClass::Measured,
                precision_mode: CONFORMANCE_MODE.as_str(),
                wall_ms: 1.0,
                fence_wait_ms: 0.5,
                polls: 1,
            },
        }
    }

    fn validated_plan(from: Pose, to: &Pose, mode: PrecisionMode) -> WarpPlan {
        Warp::reproject(
            &frame(from),
            &from,
            to,
            mode,
            WarpValidation::Measure,
        )
    }

    #[test]
    fn the_neutral_plan_reproduces_the_exact_pan_translation() {
        let from = pose(ViewControls::NEUTRAL, [2.0, 3.0]);
        let to = pose(ViewControls::NEUTRAL, [7.0, -1.0]);
        for mode in PrecisionMode::ALL {
            let plan = validated_plan(from, &to, mode);
            assert_eq!(plan.kind, WarpKind::AnchorHomography);
            assert!(plan.source_valid);
            assert!((f64::from(plan.rows[0][2]) - 10.0 / 1920.0).abs() < 1.0e-7);
            assert!((f64::from(plan.rows[1][2]) + 8.0 / 1080.0).abs() < 1.0e-7);
        }
    }

    #[test]
    fn the_neutral_plan_is_math_s_exact_matrix_and_reports_zero_error() {
        // Retiring the exact plan retired a code path, not a capability: at height zero and zero
        // camera angles the four anchors are the chart corners under the exact plane-chart
        // homography, so the solve reproduces it and the sampled corpus has nothing to report.
        for distance in [2.0, 8.0, 64.0] {
            let view = ViewControls {
                distance_five: distance,
                distance_four: distance,
                ..ViewControls::NEUTRAL
            };
            let from = pose(view, [13.25, -7.5]);
            let mut to = pose(view, [-4.0, 9.125]);
            to.zoom_log2 += 0.75;
            let exact = warp_matrix(&from, &to).expect("the neutral fixture is finite");
            for mode in PrecisionMode::ALL {
                let plan = validated_plan(from, &to, mode);
                assert_eq!(plan.kind, WarpKind::AnchorHomography);
                let solved = [
                    f64::from(plan.rows[0][0]),
                    f64::from(plan.rows[0][1]),
                    f64::from(plan.rows[0][2]),
                    f64::from(plan.rows[1][0]),
                    f64::from(plan.rows[1][1]),
                    f64::from(plan.rows[1][2]),
                    f64::from(plan.rows[2][0]),
                    f64::from(plan.rows[2][1]),
                    f64::from(plan.rows[2][2]),
                ];
                for (index, (value, wanted)) in solved.into_iter().zip(exact.forward).enumerate() {
                    assert!(
                        (value - wanted).abs() <= 1.0e-6,
                        "d={distance} coefficient {index}: {value} left the exact {wanted}"
                    );
                }
                assert!(plan.approx_max_error_px.is_some_and(|error| error < 1.0e-9));
                assert!(plan.approx_p95_error_px.is_some_and(|error| error < 1.0e-9));
            }
        }
    }

    #[test]
    fn height_zero_projects_to_the_exact_flat_chart() {
        // This is why the fullscreen pass could be deleted rather than kept beside the mesh.
        for extent in [[1920_u32, 1080_u32], [1024, 1024]] {
            for distance in [2.0, 8.0, 64.0] {
                let mut posed = pose(
                    ViewControls {
                        distance_five: distance,
                        distance_four: distance,
                        ..ViewControls::NEUTRAL
                    },
                    [0.0; 2],
                );
                posed.grid_width = extent[0];
                posed.grid_height = extent[1];
                let aspect = f64::from(extent[0]) / f64::from(extent[1]);
                for chart in CHART_CORNERS.into_iter().chain([[0.0, 0.0], [0.25, -0.5]]) {
                    // Every record height is flattened by the zero height control, so even an
                    // interior sample at H = -2 lands on the chart.
                    for record_height in HEIGHT_SAMPLES {
                        let ndc = project_presented(&posed, chart, record_height)
                            .expect("the neutral projection has no pole");
                        let display = [
                            2.0 * chart[0],
                            2.0 * f64::from(extent[1]) / f64::from(extent[0]) * chart[1],
                        ];
                        let expected = [display[0] * 0.5, display[1] * aspect * 0.5];
                        assert!(
                            (ndc[0] - expected[0]).abs() <= 1.0e-12
                                && (ndc[1] - expected[1]).abs() <= 1.0e-12,
                            "{extent:?} d={distance} chart {chart:?}: {ndc:?} != {expected:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn uploaded_rows_stay_within_quarter_pixel_at_all_required_depths() {
        for zoom_log2 in [0.0, 10.0, 20.0, 40.0, 80.0, 100.0] {
            let mut from = pose(ViewControls::NEUTRAL, [1_003.25, -507.5]);
            from.zoom_log2 = zoom_log2;
            let mut to = pose(ViewControls::NEUTRAL, [-811.125, 401.75]);
            to.zoom_log2 = zoom_log2 + 0.2;
            let exact = warp_matrix(&from, &to).expect("required warp fixture is finite");
            let plan = Warp::reproject(
                &frame(from),
                &from,
                &to,
                PrecisionMode::PictureFast,
                WarpValidation::Ordinary,
            );
            let rounded = [
                f64::from(plan.rows[0][0]),
                f64::from(plan.rows[0][1]),
                f64::from(plan.rows[0][2]),
                f64::from(plan.rows[1][0]),
                f64::from(plan.rows[1][1]),
                f64::from(plan.rows[1][2]),
                f64::from(plan.rows[2][0]),
                f64::from(plan.rows[2][1]),
                f64::from(plan.rows[2][2]),
            ];
            for chart in CHART_CORNERS.into_iter().chain([[0.0, 0.0]]) {
                let expected = apply_homography(exact.forward, chart)
                    .expect("exact fixture has no projective pole");
                let actual = apply_homography(rounded, chart)
                    .expect("rounded fixture has no projective pole");
                let error_px = ((actual[0] - expected[0]) * 0.5 * f64::from(from.grid_width))
                    .hypot((actual[1] - expected[1]) * 0.5 * f64::from(from.grid_height));
                assert!(
                    error_px <= 0.25,
                    "zoom {zoom_log2} rounded warp error was {error_px} px"
                );
            }
        }
    }

    #[test]
    fn pose_mismatch_or_invalid_extent_is_clear_only() {
        let from = pose(ViewControls::NEUTRAL, [0.0; 2]);
        let mut mismatched = from;
        mismatched.epoch = 2;
        assert_eq!(
            Warp::reproject(
                &frame(from),
                &mismatched,
                &from,
                CONFORMANCE_MODE,
                WarpValidation::Ordinary,
            )
            .kind,
            WarpKind::ClearOnly
        );
        let mut invalid = from;
        invalid.grid_width = 0;
        assert_eq!(
            Warp::reproject(
                &frame(invalid),
                &invalid,
                &from,
                CONFORMANCE_MODE,
                WarpValidation::Ordinary,
            )
            .kind,
            WarpKind::ClearOnly
        );
    }

    #[test]
    fn relief_identity_has_neutral_anchors_and_zero_sample_error() {
        let current = pose(relief(0.6), [0.0; 2]);
        for mode in PrecisionMode::ALL {
            let plan = validated_plan(current, &current, mode);
            assert_eq!(plan.kind, WarpKind::AnchorHomography);
            assert!(plan.source_valid);
            assert!(plan.approx_max_error_px.is_some_and(|error| error < 1.0e-9));
            assert!(plan.approx_p95_error_px.is_some_and(|error| error < 1.0e-9));
        }
    }

    #[test]
    fn relief_small_motion_reports_full_error_corpus() {
        let from = pose(relief(0.6), [0.0; 2]);
        let mut to = pose(relief(0.602), [2.0, -1.0]);
        to.zoom_log2 += 0.025;
        for mode in PrecisionMode::ALL {
            let plan = validated_plan(from, &to, mode);
            assert_eq!(plan.kind, WarpKind::AnchorHomography);
            let maximum = plan
                .approx_max_error_px
                .expect("the complete corpus reports a maximum");
            // Re-measured under the control-driven observer: the approximation is unchanged in
            // chart terms, but the height-zero framing is now the chart map instead of the retired
            // mount's 2*1.72/(9*aspect), so the same error covers about 4.65 times as many pixels.
            assert!(maximum <= 8.0, "maximum error was {maximum} pixels");
            assert!(plan.approx_p95_error_px.is_some_and(f64::is_finite));
        }
    }

    fn relief_pose(plane: Plane, theta: f64, displacement: [f64; 2]) -> Pose {
        let mut posed = pose(relief(theta), displacement);
        posed.plane = plane;
        posed
    }

    /// One retained-to-current motion, expressed in the section 2.6 acceptance envelope.
    #[derive(Clone, Copy)]
    struct Motion {
        view_radians: f64,
        zoom_log2: f64,
        pan_px: [f64; 2],
    }

    const ENVELOPE: Motion = Motion {
        view_radians: 0.002,
        zoom_log2: 0.025,
        pan_px: [2.0, 1.0],
    };

    const ROTATION_ONLY: Motion = Motion {
        view_radians: 0.002,
        zoom_log2: 0.0,
        pan_px: [2.0, 1.0],
    };

    fn sweep_plan(plane: Plane, step: u32, motion: Motion, mode: PrecisionMode) -> WarpPlan {
        let theta = core::f64::consts::TAU * f64::from(step) / f64::from(SWEEP_ANGLES);
        let displacement = [3.5, -2.25];
        let from = relief_pose(plane, theta, displacement);
        let panned = [
            displacement[0] + motion.pan_px[0],
            displacement[1] + motion.pan_px[1],
        ];
        let mut to = relief_pose(plane, theta + motion.view_radians, panned);
        to.zoom_log2 += motion.zoom_log2;
        validated_plan(from, &to, mode)
    }

    /// Returns the clear-only count and the worst sampled error over one full turn of VIEW angle.
    fn sweep(plane: Plane, motion: Motion, mode: PrecisionMode) -> (u32, f64) {
        let mut clear_only = 0_u32;
        let mut maximum = 0.0_f64;
        for step in 0..SWEEP_ANGLES {
            let plan = sweep_plan(plane, step, motion, mode);
            if plan.kind == WarpKind::ClearOnly {
                clear_only += 1;
                continue;
            }
            assert_eq!(plan.kind, WarpKind::AnchorHomography);
            assert!(plan.source_valid);
            let error = plan
                .approx_max_error_px
                .expect("an anchor plan reports its sampled maximum");
            let percentile = plan
                .approx_p95_error_px
                .expect("an anchor plan reports its sampled percentile");
            assert!(percentile <= error);
            maximum = maximum.max(error);
        }
        (clear_only, maximum)
    }

    fn named_planes() -> Result<[(&'static str, Plane); 3], MathError> {
        let identity = PlaneAngles {
            theta_1: 0.0,
            theta_2: 0.0,
        };
        let hybrid = PlaneAngles {
            theta_1: core::f64::consts::FRAC_PI_4,
            theta_2: core::f64::consts::FRAC_PI_4,
        };
        let quarter = core::f64::consts::FRAC_PI_2;
        Ok([
            ("mandelbrot", construct_plane(identity)?),
            (
                "julia",
                construct_plane(PlaneAngles {
                    theta_1: -quarter,
                    theta_2: -quarter,
                })?,
            ),
            ("hybrid", construct_plane(hybrid)?),
        ])
    }

    #[test]
    fn the_ambient_embedding_annihilates_mandelbrot_display_axes() -> Result<(), MathError> {
        let [(_, mandelbrot), ..] = named_planes()?;
        let ambient = plane_point(mandelbrot, [1.5, -0.75]);
        assert_eq!([ambient[0], ambient[1]], [0.0, 0.0]);
        assert_ne!([ambient[2], ambient[3]], [0.0, 0.0]);
        for step in 0..SWEEP_ANGLES {
            let theta = core::f64::consts::TAU * f64::from(step) / f64::from(SWEEP_ANGLES);
            let (sine, cosine) = theta.sin_cos();
            let old_first = ambient[0].mul_add(cosine, -ambient[1] * sine);
            let old_second = ambient[0].mul_add(sine, ambient[1] * cosine);
            assert_eq!([old_first, old_second], [0.0, 0.0]);
        }
        let display = display_point([1.5, -0.75], 0.0, [1.0, 0.0, 1.0, 0.0]);
        assert_eq!([display[0], display[1]], [1.5, -0.75]);
        Ok(())
    }

    #[test]
    fn every_named_plane_warps_at_every_swept_view_angle() -> Result<(), MathError> {
        for mode in PrecisionMode::ALL {
            for (name, plane) in named_planes()? {
                let (clear_only, maximum) = sweep(plane, ENVELOPE, mode);
                assert_eq!(
                    clear_only, 0,
                    "{name} plane fell back to clear-only at {clear_only} of {SWEEP_ANGLES} angles"
                );
                assert!(
                    maximum <= 16.0,
                    "{name} plane sampled {maximum} pixels over the full acceptance envelope"
                );
                let (clear_only, maximum) = sweep(plane, ROTATION_ONLY, mode);
                assert_eq!(clear_only, 0, "{name} plane cleared without a zoom step");
                assert!(
                    maximum <= 4.0,
                    "{name} plane sampled {maximum} pixels for rotation and pan alone"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn the_plan_ignores_which_ambient_axes_the_plane_names() -> Result<(), MathError> {
        let [(_, mandelbrot), (_, julia), (_, hybrid)] = named_planes()?;
        for mode in PrecisionMode::ALL {
            for step in 0..SWEEP_ANGLES {
                let reference = sweep_plan(mandelbrot, step, ENVELOPE, mode);
                let julia_plan = sweep_plan(julia, step, ENVELOPE, mode);
                // Deliberate operation-word identity belongs only to deterministic conformance.
                if mode.requires_bit_identity() {
                    assert_eq!(julia_plan.rows, reference.rows);
                    assert_eq!(
                        julia_plan.approx_max_error_px,
                        reference.approx_max_error_px
                    );
                }
                assert_eq!(julia_plan.kind, reference.kind);
                assert!(julia_plan.chart_residual <= 1.0e-12);
                assert!(reference.chart_residual <= 1.0e-12);
                let tilted = sweep_plan(hybrid, step, ENVELOPE, mode);
                assert_eq!(tilted.kind, reference.kind);
                for (row, expected) in tilted.rows.into_iter().zip(reference.rows) {
                    for (value, wanted) in row.into_iter().zip(expected) {
                        assert!(
                            (value - wanted).abs() <= 1.0e-6,
                            "hybrid row entry {value} left the binary32 basis tolerance of {wanted}"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn picture_fast_refreshes_skip_the_corpus_until_measure() {
        SAMPLED_CORPUS_RUNS.with(|runs| runs.set(0));
        let from = pose(relief(0.6), [0.0; 2]);
        let mut to = pose(relief(0.602), [2.0, -1.0]);
        to.zoom_log2 += 0.025;
        for _ in 0..12 {
            let plan = Warp::reproject(
                &frame(from),
                &from,
                &to,
                PrecisionMode::PictureFast,
                WarpValidation::Ordinary,
            );
            assert_eq!(plan.approx_max_error_px, None);
            assert_eq!(plan.approx_p95_error_px, None);
        }
        SAMPLED_CORPUS_RUNS.with(|runs| assert_eq!(runs.get(), 0));
        let measured = Warp::reproject(
            &frame(from),
            &from,
            &to,
            PrecisionMode::PictureFast,
            WarpValidation::Measure,
        );
        assert!(measured.approx_max_error_px.is_some());
        SAMPLED_CORPUS_RUNS.with(|runs| assert_eq!(runs.get(), 1));
    }
}
