use ember_julibrot_math::{Plane, Pose, ViewMode, warp_matrix};

use crate::{
    SceneFrame, WarpKind, WarpPlan, apply_homography, pack_homography_rows, solve_homography,
};

const CHART_CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];
const HEIGHT_SAMPLES: [f64; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];
const CHART_STEPS: u32 = 9;
const PERSPECTIVE_POLE: f64 = 8.0;
const POLE_EPSILON: f64 = 1.0e-4;
const YAW_COSINE: f64 = 0.939_692_620_8;
const YAW_SINE: f64 = 0.342_020_143_3;
const PITCH_COSINE: f64 = 0.965_925_826_3;
const PITCH_SINE: f64 = 0.258_819_045_1;

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
    let percentile_index = errors
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
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
/// `rotation` is `[cos θ, sin θ, cos φθ, sin φθ]` for the standing VIEW rotation `R12(θ)R35(φθ)`.
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

fn project_presented(pose: &Pose, chart: [f64; 2], height: f64) -> Option<[f64; 2]> {
    if pose.grid_width == 0 || pose.grid_height == 0 {
        return None;
    }
    let display_coordinate = [
        2.0 * chart[0],
        2.0 * f64::from(pose.grid_height) / f64::from(pose.grid_width) * chart[1],
    ];
    let theta_two = f64::midpoint(1.0, 5.0_f64.sqrt()) * pose.view_theta_1;
    let (sine_one, cosine_one) = pose.view_theta_1.sin_cos();
    let (sine_two, cosine_two) = theta_two.sin_cos();
    let rotation = [cosine_one, sine_one, cosine_two, sine_two];
    let rotated = display_point(display_coordinate, height, rotation);
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
    use ember_julibrot_math::{
        MathError, Plane, PlaneAngles, PlanePreset, ViewMode, construct_plane,
    };

    use super::*;
    use crate::{PaletteId, SampleClass, SubmissionKind, SubmissionMeasurement};

    const SWEEP_ANGLES: u32 = 256;
    const JULIA_SEED: [f64; 2] = [-0.8, 0.156];

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
    fn uploaded_flat_rows_stay_within_quarter_pixel_at_all_required_depths() {
        for zoom_log2 in [0.0, 10.0, 20.0, 40.0, 80.0, 100.0] {
            let mut from = pose(ViewMode::Flat, 0.0, [1_003.25, -507.5]);
            from.zoom_log2 = zoom_log2;
            let mut to = pose(ViewMode::Flat, 0.0, [-811.125, 401.75]);
            to.zoom_log2 = zoom_log2 + 0.2;
            let exact = warp_matrix(&from, &to).expect("required warp fixture is finite");
            let plan = Warp::reproject(&frame(from), &from, &to);
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
        let mut to = pose(ViewMode::Tumbled, 0.602, [2.0, -1.0]);
        to.zoom_log2 += 0.025;
        let plan = Warp::reproject(&frame(from), &from, &to);
        assert_eq!(plan.kind, WarpKind::TumbledHomography);
        let maximum = plan
            .approx_max_error_px
            .expect("the complete corpus reports a maximum");
        assert!(maximum <= 2.0, "maximum error was {maximum} pixels");
        assert!(plan.approx_p95_error_px.is_some_and(f64::is_finite));
    }

    fn tumbled_pose(plane: Plane, theta: f64, displacement: [f64; 2]) -> Pose {
        let mut posed = pose(ViewMode::Tumbled, theta, displacement);
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

    fn sweep_plan(plane: Plane, step: u32, motion: Motion) -> WarpPlan {
        let theta = core::f64::consts::TAU * f64::from(step) / f64::from(SWEEP_ANGLES);
        let displacement = [3.5, -2.25];
        let from = tumbled_pose(plane, theta, displacement);
        let panned = [
            displacement[0] + motion.pan_px[0],
            displacement[1] + motion.pan_px[1],
        ];
        let mut to = tumbled_pose(plane, theta + motion.view_radians, panned);
        to.zoom_log2 += motion.zoom_log2;
        Warp::reproject(&frame(from), &from, &to)
    }

    /// Returns the clear-only count and the worst sampled error over one full turn of VIEW angle.
    fn sweep(plane: Plane, motion: Motion) -> (u32, f64) {
        let mut clear_only = 0_u32;
        let mut maximum = 0.0_f64;
        for step in 0..SWEEP_ANGLES {
            let plan = sweep_plan(plane, step, motion);
            if plan.kind == WarpKind::ClearOnly {
                clear_only += 1;
                continue;
            }
            assert_eq!(plan.kind, WarpKind::TumbledHomography);
            assert!(plan.source_valid);
            let error = plan
                .approx_max_error_px
                .expect("a tumbled plan reports its sampled maximum");
            let percentile = plan
                .approx_p95_error_px
                .expect("a tumbled plan reports its sampled percentile");
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
        Ok([
            (
                "mandelbrot",
                construct_plane(PlanePreset::Mandelbrot, identity)?,
            ),
            (
                "julia",
                construct_plane(PlanePreset::Julia { c0: JULIA_SEED }, identity)?,
            ),
            ("hybrid", construct_plane(PlanePreset::Mandelbrot, hybrid)?),
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
        for (name, plane) in named_planes()? {
            let (clear_only, maximum) = sweep(plane, ENVELOPE);
            assert_eq!(
                clear_only, 0,
                "{name} plane fell back to clear-only at {clear_only} of {SWEEP_ANGLES} angles"
            );
            assert!(
                maximum <= 3.5,
                "{name} plane sampled {maximum} pixels over the full acceptance envelope"
            );
            let (clear_only, maximum) = sweep(plane, ROTATION_ONLY);
            assert_eq!(clear_only, 0, "{name} plane cleared without a zoom step");
            assert!(
                maximum <= 1.0,
                "{name} plane sampled {maximum} pixels for rotation and pan alone"
            );
        }
        Ok(())
    }

    #[test]
    fn the_tumbled_plan_ignores_which_ambient_axes_a_preset_names() -> Result<(), MathError> {
        let [(_, mandelbrot), (_, julia), (_, hybrid)] = named_planes()?;
        for step in 0..SWEEP_ANGLES {
            let reference = sweep_plan(mandelbrot, step, ENVELOPE);
            assert_eq!(sweep_plan(julia, step, ENVELOPE), reference);
            let tilted = sweep_plan(hybrid, step, ENVELOPE);
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
        Ok(())
    }
}
