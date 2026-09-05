//! Binary64 source reconstruction and target projection for retained rendered samples.

use thiserror::Error;

use crate::{EscapeGridRecord, Pose, PoseMap, RELIEF_NEAR_FRACTION, ViewControls};

const POLE_EPSILON: f64 = 1.0e-4;
const SOURCE_ROUND_TRIP_EPSILON: f64 = 1.0e-9;

/// Palette-independent value information needed to rebuild one retained sample's lift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedValueSample {
    /// Normalized escape height in the scene record domain `[-2,2]`.
    pub record_height: f64,
}

/// Reusable source-depth and source-local coordinate record stored beside one value record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceDepthRecord {
    /// Source-local first plane coordinate after the accepted source map and scale.
    pub a_f: f64,
    /// Source-local second plane coordinate after the accepted source map and scale.
    pub b_f: f64,
    /// Positive linear source-view distance after both perspective divides and observer rotation.
    pub zeta_f: f64,
    /// Whether source visibility selected a surface sample at this pixel.
    pub valid: bool,
}

/// One reconstructed point on the canonical four-dimensional slice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReconstructedSample {
    /// Absolute finite mirror of the canonical slice point.
    pub ambient_four: [f64; 4],
    /// Palette-independent value information that supplies target height.
    pub value: RetainedValueSample,
}

/// Complete binary64 target projection of one reconstructed retained sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectedSample {
    /// Centred target pixel coordinate.
    pub screen: [f64; 2],
    /// Positive linear distance from the target observer before finite depth mapping.
    pub linear_depth: f64,
    /// Target `Depth24Plus` value before attachment quantization.
    pub raster_depth: f64,
}

/// Typed refusal from source reconstruction or target projection.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ReprojectionError {
    /// The retained record, pose, or derived point was invalid or non-finite.
    #[error("the retained source sample is invalid or non-finite")]
    InvalidSource,
    /// The source sample did not reproduce its stored pixel and linear depth.
    #[error("the retained source sample failed its self-round-trip receipt")]
    SourceRoundTrip,
    /// The target is edge-on or the sample reaches a projection pole.
    #[error("the target projection has no finite visible result")]
    ProjectionPole,
}

/// Builds a flat-chart `S1` fixture record for one visible source sample.
///
/// A lifted source needs the post-visibility plane coordinates selected by rasterization; those
/// cannot be recovered by inverting the neutral-height source map and must come from the rendered
/// descriptor instead.
///
/// # Errors
///
/// Returns a typed refusal for a lifted pose, invalid map, non-finite input, or projection pole.
pub fn source_depth_record(
    pose: &Pose,
    source_pixel: [f64; 2],
    value: RetainedValueSample,
) -> Result<SourceDepthRecord, ReprojectionError> {
    if pose.view.height_scale.to_bits() != 0.0_f64.to_bits() {
        return Err(ReprojectionError::InvalidSource);
    }
    let PoseMap::Mapped(map) = pose.map else {
        return Err(ReprojectionError::InvalidSource);
    };
    let homogeneous = apply_homogeneous(map.rows, source_pixel);
    if !homogeneous.into_iter().all(f64::is_finite) || homogeneous[2] <= 0.0 {
        return Err(ReprojectionError::InvalidSource);
    }
    let chart_scale = 4.0 * map.apron_scale / f64::from(pose.grid_width);
    let a_f = chart_scale * homogeneous[0] / homogeneous[2];
    let b_f = chart_scale * homogeneous[1] / homogeneous[2];
    let ambient_four = absolute_plane_point(pose, [a_f, b_f]);
    let projected = project_ambient_point(pose, ambient_four, value)?;
    Ok(SourceDepthRecord {
        a_f,
        b_f,
        zeta_f: projected.linear_depth,
        valid: true,
    })
}

/// Reconstructs `U_F(s_F,d_F,r_F)` and verifies the retained source receipt in binary64.
///
/// # Errors
///
/// Returns a typed refusal for an invalid record or when source pixel/depth reproduction exceeds
/// `1e-9`.
pub fn reconstruct_source_sample(
    pose: &Pose,
    source_pixel: [f64; 2],
    depth: SourceDepthRecord,
    value: RetainedValueSample,
) -> Result<ReconstructedSample, ReprojectionError> {
    if !depth.valid
        || !source_pixel.into_iter().all(f64::is_finite)
        || ![depth.a_f, depth.b_f, depth.zeta_f, value.record_height]
            .into_iter()
            .all(f64::is_finite)
        || depth.zeta_f <= 0.0
    {
        return Err(ReprojectionError::InvalidSource);
    }
    let sample = ReconstructedSample {
        ambient_four: absolute_plane_point(pose, [depth.a_f, depth.b_f]),
        value,
    };
    let projected = project_ambient_point(pose, sample.ambient_four, value)?;
    let pixel_error =
        (projected.screen[0] - source_pixel[0]).hypot(projected.screen[1] - source_pixel[1]);
    let depth_error = (projected.linear_depth - depth.zeta_f).abs();
    if pixel_error > SOURCE_ROUND_TRIP_EPSILON || depth_error > SOURCE_ROUND_TRIP_EPSILON {
        return Err(ReprojectionError::SourceRoundTrip);
    }
    Ok(sample)
}

/// Evaluates the complete target chain `Pi_T` in binary64.
///
/// # Errors
///
/// Returns a typed refusal for an invalid pose, an edge-on map, or a perspective pole.
pub fn project_reconstructed_sample(
    target: &Pose,
    sample: ReconstructedSample,
) -> Result<ProjectedSample, ReprojectionError> {
    project_ambient_point(target, sample.ambient_four, sample.value)
}

/// Converts one existing escape record into the value-height input used by reprojection.
///
/// # Errors
///
/// Returns a typed refusal for zero cap or a non-finite record.
pub fn retained_value_sample(
    record: EscapeGridRecord,
    iteration_cap: u32,
) -> Result<RetainedValueSample, ReprojectionError> {
    if iteration_cap == 0
        || ![
            record.smooth_iter,
            record.escaped,
            record.rebase_count,
            record.status,
        ]
        .into_iter()
        .all(f32::is_finite)
    {
        return Err(ReprojectionError::InvalidSource);
    }
    let record_height = if matches!(record.status.to_bits(), 0x3f80_0000 | 0x4000_0000) {
        0.0
    } else if record.escaped == 0.0 {
        -2.0
    } else {
        (f64::from(record.smooth_iter) / f64::from(iteration_cap))
            .clamp(0.0, 1.0)
            .mul_add(4.0, -2.0)
    };
    Ok(RetainedValueSample { record_height })
}

fn absolute_plane_point(pose: &Pose, coordinate: [f64; 2]) -> [f64; 4] {
    core::array::from_fn(|axis| {
        f64::from(pose.plane.basis_u[axis]).mul_add(
            coordinate[0],
            f64::from(pose.plane.basis_v[axis]).mul_add(coordinate[1], pose.plane_origin[axis]),
        )
    })
}

fn project_ambient_point(
    pose: &Pose,
    ambient_four: [f64; 4],
    value: RetainedValueSample,
) -> Result<ProjectedSample, ReprojectionError> {
    if pose.grid_width == 0
        || pose.grid_height == 0
        || !pose.view.is_valid()
        || !ambient_four.into_iter().all(f64::is_finite)
        || !value.record_height.is_finite()
        || matches!(pose.map, PoseMap::EdgeOn)
    {
        return Err(ReprojectionError::ProjectionPole);
    }
    let local_four: [f64; 4] =
        core::array::from_fn(|axis| ambient_four[axis] - pose.plane_origin[axis]);
    let height = pose.view.height_scale * (value.record_height + 2.0) * 0.5;
    let mut ambient = [
        local_four[0],
        local_four[1],
        local_four[2],
        local_four[3],
        height,
    ];
    apply_camera_rotation(&mut ambient, &pose.view);
    for (coordinate, translation) in ambient.iter_mut().zip(pose.view.camera_translation) {
        *coordinate += translation;
    }

    let distance_five = pose.view.distance_five;
    let distance_four = pose.view.distance_four;
    let unclamped_five = distance_five - ambient[4];
    let denominator_five = unclamped_five.max(RELIEF_NEAR_FRACTION * distance_five);
    if denominator_five <= POLE_EPSILON || unclamped_five < RELIEF_NEAR_FRACTION * distance_five {
        return Err(ReprojectionError::ProjectionPole);
    }
    let scale_five = distance_five / denominator_five;
    let projected_four = [
        ambient[0] * scale_five,
        ambient[1] * scale_five,
        ambient[2] * scale_five,
        ambient[3] * scale_five,
    ];
    let denominator_four = distance_four - projected_four[3];
    if denominator_four <= POLE_EPSILON {
        return Err(ReprojectionError::ProjectionPole);
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
    let linear_depth = -view[2];
    if !linear_depth.is_finite() || linear_depth <= POLE_EPSILON {
        return Err(ReprojectionError::ProjectionPole);
    }
    let aspect = f64::from(pose.grid_width) / f64::from(pose.grid_height);
    let perspective_scale = aspect * distance_four * 0.5;
    let ndc = [
        perspective_scale * view[0] / aspect / linear_depth,
        perspective_scale * view[1] / linear_depth,
    ];
    let screen = [
        ndc[0] * f64::from(pose.grid_width) * 0.5,
        ndc[1] * f64::from(pose.grid_height) * 0.5,
    ];
    let camera_near = 0.1;
    let camera_far = 4.0 * distance_four;
    let clip_depth = (camera_far / (camera_near - camera_far)).mul_add(
        view[2],
        camera_far * camera_near / (camera_near - camera_far),
    );
    let raster_depth = clip_depth / linear_depth;
    if !screen.into_iter().chain([raster_depth]).all(f64::is_finite) {
        return Err(ReprojectionError::ProjectionPole);
    }
    Ok(ProjectedSample {
        screen,
        linear_depth,
        raster_depth,
    })
}

fn apply_camera_rotation(value: &mut [f64; 5], view: &ViewControls) {
    for factor in (0..ViewControls::CAMERA_PLANES.len()).rev() {
        let (first, second) = ViewControls::CAMERA_PLANES[factor];
        let (sine, cosine) = view.camera[factor].sin_cos();
        let a = cosine.mul_add(value[first], -sine * value[second]);
        let b = sine.mul_add(value[first], cosine * value[second]);
        value[first] = a;
        value[second] = b;
    }
}

const fn apply_homogeneous(matrix: [f64; 9], point: [f64; 2]) -> [f64; 3] {
    [
        matrix[0].mul_add(point[0], matrix[1].mul_add(point[1], matrix[2])),
        matrix[3].mul_add(point[0], matrix[4].mul_add(point[1], matrix[5])),
        matrix[6].mul_add(point[0], matrix[7].mul_add(point[1], matrix[8])),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObjectAngles, construct_plane, screen_to_plane};

    fn pose_for(object: ObjectAngles, view: ViewControls) -> Pose {
        let extent = [960, 540];
        let map = screen_to_plane(
            &object,
            &view,
            0.0,
            extent[0],
            extent[1],
            f64::from(extent[0]) / f64::from(extent[1]),
        )
        .expect("fixture map is finite");
        Pose {
            epoch: 1,
            orbit_generation: 7,
            plane: construct_plane(object).expect("fixture plane constructs"),
            object,
            plane_origin: [0.25, -0.5, 0.0, 0.0],
            zoom_log2: 0.0,
            view,
            grid_width: extent[0],
            grid_height: extent[1],
            map: PoseMap::Mapped(map),
            centre_from_reference_px: [0.0; 2],
        }
    }

    fn pose(view: ViewControls) -> Pose {
        pose_for(ObjectAngles::JULIA, view)
    }

    fn lifted_pose() -> Pose {
        let object = ObjectAngles::JULIA;
        let mut camera = [0.0; 10];
        camera[1] = 0.02;
        camera[6] = 0.03;
        camera[9] = -0.02;
        pose_for(
            object,
            ViewControls {
                camera,
                camera_translation: [0.01, -0.01, 0.0, 0.0, -0.01],
                camera_yaw: 0.04,
                camera_pitch: -0.03,
                height_scale: 0.3,
                distance_five: 8.0,
                distance_four: 8.0,
            },
        )
    }

    fn lifted_record(
        pose: &Pose,
        pixel: [f64; 2],
        value: RetainedValueSample,
    ) -> SourceDepthRecord {
        const STEP: f64 = 1.0e-6;

        let PoseMap::Mapped(map) = pose.map else {
            panic!("lifted fixture has a finite source map");
        };
        let homogeneous = apply_homogeneous(map.rows, pixel);
        let chart_scale = 4.0 * map.apron_scale / f64::from(pose.grid_width);
        let mut coordinate = [
            chart_scale * homogeneous[0] / homogeneous[2],
            chart_scale * homogeneous[1] / homogeneous[2],
        ];
        for _ in 0..12 {
            let projected = project_ambient_point(
                pose,
                absolute_plane_point(pose, coordinate),
                value,
            )
            .expect("lifted fixture remains visible");
            let error = [
                projected.screen[0] - pixel[0],
                projected.screen[1] - pixel[1],
            ];
            if error[0].hypot(error[1]) <= 1.0e-12 {
                break;
            }
            let shifted_a = project_ambient_point(
                pose,
                absolute_plane_point(pose, [coordinate[0] + STEP, coordinate[1]]),
                value,
            )
            .expect("first finite-difference point remains visible");
            let shifted_b = project_ambient_point(
                pose,
                absolute_plane_point(pose, [coordinate[0], coordinate[1] + STEP]),
                value,
            )
            .expect("second finite-difference point remains visible");
            let jacobian = [
                [
                    (shifted_a.screen[0] - projected.screen[0]) / STEP,
                    (shifted_b.screen[0] - projected.screen[0]) / STEP,
                ],
                [
                    (shifted_a.screen[1] - projected.screen[1]) / STEP,
                    (shifted_b.screen[1] - projected.screen[1]) / STEP,
                ],
            ];
            let determinant = jacobian[0][1]
                .mul_add(-jacobian[1][0], jacobian[0][0] * jacobian[1][1]);
            assert!(determinant.abs() > 1.0e-12);
            let delta = [
                jacobian[0][1].mul_add(-error[1], error[0] * jacobian[1][1]) / determinant,
                error[0].mul_add(-jacobian[1][0], jacobian[0][0] * error[1]) / determinant,
            ];
            coordinate[0] -= delta[0];
            coordinate[1] -= delta[1];
        }
        let projected = project_ambient_point(
            pose,
            absolute_plane_point(pose, coordinate),
            value,
        )
        .expect("solved lifted fixture remains visible");
        assert!(
            (projected.screen[0] - pixel[0]).hypot(projected.screen[1] - pixel[1])
                <= SOURCE_ROUND_TRIP_EPSILON
        );
        SourceDepthRecord {
            a_f: coordinate[0],
            b_f: coordinate[1],
            zeta_f: projected.linear_depth,
            valid: true,
        }
    }

    fn reproject_lifted_pixel(
        source: &Pose,
        target: &Pose,
        pixel: [f64; 2],
        value: RetainedValueSample,
    ) -> [f64; 2] {
        let depth = lifted_record(source, pixel, value);
        let sample = reconstruct_source_sample(source, pixel, depth, value)
            .expect("GPU-shaped source receipt round-trips");
        project_reconstructed_sample(target, sample)
            .expect("lifted sample projects to target")
            .screen
    }

    #[test]
    fn retained_sample_reprojects_to_its_own_pixel_within_binary64_bound() {
        let source = pose(ViewControls::NEUTRAL);
        let pixel = [137.5, -81.5];
        let value = RetainedValueSample {
            record_height: -2.0,
        };
        let depth = source_depth_record(&source, pixel, value).expect("source sample is visible");
        let reconstructed = reconstruct_source_sample(&source, pixel, depth, value)
            .expect("source receipt round-trips");
        let projected = project_reconstructed_sample(&source, reconstructed)
            .expect("self projection stays visible");
        assert!((projected.screen[0] - pixel[0]).abs() <= SOURCE_ROUND_TRIP_EPSILON);
        assert!((projected.screen[1] - pixel[1]).abs() <= SOURCE_ROUND_TRIP_EPSILON);
        assert!((projected.linear_depth - depth.zeta_f).abs() <= SOURCE_ROUND_TRIP_EPSILON);
    }

    #[test]
    fn gpu_shaped_lifted_record_round_trips_a_noncanonical_pose_and_checks_depth() {
        let source = lifted_pose();
        let pixel = [37.5, -21.5];
        let value = RetainedValueSample {
            record_height: -1.0,
        };
        assert_eq!(
            source_depth_record(&source, pixel, value),
            Err(ReprojectionError::InvalidSource),
            "the flat-chart helper cannot invent post-visibility lifted coordinates"
        );
        let depth = lifted_record(&source, pixel, value);
        let reconstructed = reconstruct_source_sample(&source, pixel, depth, value)
            .expect("external lifted receipt round-trips");
        let projected = project_reconstructed_sample(&source, reconstructed)
            .expect("lifted self projection stays visible");
        assert!((projected.screen[0] - pixel[0]).abs() <= SOURCE_ROUND_TRIP_EPSILON);
        assert!((projected.screen[1] - pixel[1]).abs() <= SOURCE_ROUND_TRIP_EPSILON);

        let wrong_depth = SourceDepthRecord {
            zeta_f: depth.zeta_f + 1.0e-6,
            ..depth
        };
        assert_eq!(
            reconstruct_source_sample(&source, pixel, wrong_depth, value),
            Err(ReprojectionError::SourceRoundTrip)
        );
    }

    #[test]
    fn edge_on_invalid_and_pole_records_are_refused() {
        let source = pose(ViewControls::NEUTRAL);
        let pixel = [0.5, 0.5];
        let value = RetainedValueSample {
            record_height: -2.0,
        };
        let mut edge = source;
        edge.map = PoseMap::EdgeOn;
        assert_eq!(
            source_depth_record(&edge, pixel, value),
            Err(ReprojectionError::InvalidSource)
        );

        let depth = source_depth_record(&source, pixel, value).expect("source sample is visible");
        let invalid = SourceDepthRecord {
            valid: false,
            ..depth
        };
        assert_eq!(
            reconstruct_source_sample(&source, pixel, invalid, value),
            Err(ReprojectionError::InvalidSource)
        );

        let mut pole = source;
        pole.view.distance_four = 1.0e-5;
        assert_eq!(
            project_reconstructed_sample(
                &pole,
                ReconstructedSample {
                    ambient_four: source.plane_origin,
                    value,
                }
            ),
            Err(ReprojectionError::ProjectionPole)
        );
    }

    #[test]
    fn one_pixel_tile_interpolation_stays_inside_the_admission_bound() {
        let source = pose(ViewControls::NEUTRAL);
        let mut target_view = ViewControls::NEUTRAL;
        target_view.camera_yaw = 0.000_005;
        target_view.camera_pitch = -0.000_002_5;
        let target = pose(target_view);
        let value = RetainedValueSample {
            record_height: -2.0,
        };
        let corners = [[10.0, 20.0], [11.0, 20.0], [10.0, 21.0], [11.0, 21.0]];
        let projected = corners.map(|pixel| {
            let depth = source_depth_record(&source, pixel, value).expect("corner is visible");
            let sample = reconstruct_source_sample(&source, pixel, depth, value)
                .expect("corner round-trips");
            project_reconstructed_sample(&target, sample)
                .expect("corner projects into target")
                .screen
        });
        let centre = [10.5, 20.5];
        let centre_depth = source_depth_record(&source, centre, value).expect("centre is visible");
        let direct = project_reconstructed_sample(
            &target,
            reconstruct_source_sample(&source, centre, centre_depth, value)
                .expect("centre round-trips"),
        )
        .expect("centre projects")
        .screen;
        let interpolated = [
            0.25 * (projected[0][0] + projected[1][0] + projected[2][0] + projected[3][0]),
            0.25 * (projected[0][1] + projected[1][1] + projected[2][1] + projected[3][1]),
        ];
        let error = (direct[0] - interpolated[0]).hypot(direct[1] - interpolated[1]);
        assert!(
            error <= SOURCE_ROUND_TRIP_EPSILON,
            "near-affine one-pixel interpolation error {error}"
        );
    }

    #[test]
    fn curved_one_pixel_tile_stays_inside_the_admission_bound() {
        let source = lifted_pose();
        let mut target_view = source.view;
        target_view.camera[1] += 0.18;
        target_view.camera[6] -= 0.14;
        target_view.camera_yaw += 0.11;
        target_view.camera_pitch -= 0.07;
        target_view.height_scale = 1.1;
        target_view.distance_five = 5.5;
        let target = pose_for(source.object, target_view);
        let value = RetainedValueSample {
            record_height: -1.0,
        };
        let corners = [
            [300.0, 180.0],
            [301.0, 180.0],
            [300.0, 181.0],
            [301.0, 181.0],
        ];
        let projected = corners.map(|pixel| reproject_lifted_pixel(&source, &target, pixel, value));
        let direct = reproject_lifted_pixel(&source, &target, [300.5, 180.5], value);
        let interpolated = [
            0.25 * (projected[0][0] + projected[1][0] + projected[2][0] + projected[3][0]),
            0.25 * (projected[0][1] + projected[1][1] + projected[2][1] + projected[3][1]),
        ];
        let error = (direct[0] - interpolated[0]).hypot(direct[1] - interpolated[1]);
        assert!(error > SOURCE_ROUND_TRIP_EPSILON, "curved error {error}");
        assert!(error <= 1.0, "curved one-pixel interpolation error {error}");
    }
}
