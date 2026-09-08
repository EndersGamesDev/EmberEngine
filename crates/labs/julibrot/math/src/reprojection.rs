//! Binary64 source reconstruction and target projection for retained rendered samples.

use thiserror::Error;

use crate::{
    EscapeGridRecord, Plane, Pose, PoseMap, RELIEF_NEAR_FRACTION, ViewControls, construct_plane,
};

/// Binary64 self-reprojection has nine decimal pixel/depth digits of rounding headroom.
const SOURCE_ROUND_TRIP_EPSILON: f64 = 1.0e-9;
/// Julia's canonical zero orthogonal components differ from the binary32 image of `cos(pi/2)` by
/// less than one f32 epsilon, while a missing unit basis remains one unit away.
const TARGET_PLANE_COMPONENT_TOLERANCE: f32 = f32::EPSILON;

impl Plane {
    /// Expands one two-dimensional chart coordinate through this plane's rounded basis.
    #[must_use]
    pub fn local_point(self, coordinate: [f64; 2]) -> [f64; 4] {
        core::array::from_fn(|axis| {
            f64::from(self.basis_u[axis])
                .mul_add(coordinate[0], f64::from(self.basis_v[axis]) * coordinate[1])
        })
    }
}

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
    /// Source-anchor-relative point retained before finite-mirror addition can lose deep detail.
    pub source_local_four: [f64; 4],
    /// Source scale used to express `source_local_four` in the requested zoom frame.
    pub source_zoom_log2: f64,
    /// Palette-independent value information that supplies target height.
    pub value: RetainedValueSample,
}

impl ReconstructedSample {
    /// Reconstructs one source sample and verifies its pixel and linear-depth receipt.
    ///
    /// Separate bounds let a descriptor declare the independently measured coordinate and depth
    /// errors introduced by its `f32` lanes, while the existing free function retains its stricter
    /// binary64 self-check.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for invalid inputs or a source projection outside either bound.
    pub fn from_source_receipt(
        pose: &Pose,
        source_pixel: [f64; 2],
        depth: SourceDepthRecord,
        value: RetainedValueSample,
        pixel_tolerance: f64,
        depth_tolerance: f64,
    ) -> Result<Self, ReprojectionError> {
        if !pixel_tolerance.is_finite()
            || pixel_tolerance < 0.0
            || !depth_tolerance.is_finite()
            || depth_tolerance < 0.0
        {
            return Err(ReprojectionError::InvalidSource);
        }
        reconstruct_source_sample_with_tolerance(
            pose,
            source_pixel,
            depth,
            value,
            pixel_tolerance,
            depth_tolerance,
        )
    }

    /// Maps this source-local point into a requested exact-anchor frame and projects it.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for invalid placement or target inputs, an error above one pixel,
    /// or a target projection pole.
    pub fn project_from_anchor(
        self,
        target: &Pose,
        source_to_request_anchor_px: [f64; 2],
        placement_error_px: f64,
    ) -> Result<ProjectedSample, ReprojectionError> {
        project_reconstructed_sample_from_anchor(
            target,
            self,
            source_to_request_anchor_px,
            placement_error_px,
        )
    }
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

impl ProjectedSample {
    /// Smallest admitted positive denominator or observer distance in the shared projection.
    pub const POLE_EPSILON: f64 = 1.0e-4;
    /// Descriptor placement is admitted only when its independent and split error is at most 1 px.
    pub const PLACEMENT_ERROR_LIMIT_PX: f64 = 1.0;

    /// Projects one point already expressed relative to the requested plane origin.
    ///
    /// This is the shared ordered five-dimensional chain used by retained-sample projection and
    /// the whole-grid planner. Keeping plane-point construction outside this entry preserves the
    /// planner's existing binary64 basis algebra while giving both paths one camera, translation,
    /// perspective, observer, depth, and viewport implementation.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for an invalid pose, an edge-on map, a non-finite input, or a
    /// perspective pole.
    pub fn from_local_point(
        pose: &Pose,
        local_four: [f64; 4],
        value: RetainedValueSample,
    ) -> Result<Self, ReprojectionError> {
        project_local_point(pose, local_four, value)
    }
}

/// Typed refusal from source reconstruction or target projection.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ReprojectionError {
    /// The retained record, pose, or derived point was invalid or non-finite.
    #[error("the retained source sample is invalid or non-finite")]
    InvalidSource,
    /// The requested pose's rounded plane does not match its object controls.
    #[error("the requested target pose has an invalid sampled plane")]
    InvalidTarget,
    /// The source sample did not reproduce its stored pixel and linear depth.
    #[error("the retained source sample failed its self-round-trip receipt")]
    SourceRoundTrip,
    /// The target is edge-on or the sample reaches a projection pole.
    #[error("the target projection has no finite visible result")]
    ProjectionPole,
    /// The exact-anchor split cannot certify the descriptor placement below one pixel.
    #[error("the descriptor exact-anchor placement exceeds its one-pixel error ceiling")]
    UncertifiedPlacement,
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
    let source_local_four = pose.plane.local_point([a_f, b_f]);
    let projected = ProjectedSample::from_local_point(pose, source_local_four, value)?;
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
    ReconstructedSample::from_source_receipt(
        pose,
        source_pixel,
        depth,
        value,
        SOURCE_ROUND_TRIP_EPSILON,
        SOURCE_ROUND_TRIP_EPSILON,
    )
}

fn reconstruct_source_sample_with_tolerance(
    pose: &Pose,
    source_pixel: [f64; 2],
    depth: SourceDepthRecord,
    value: RetainedValueSample,
    pixel_tolerance: f64,
    depth_tolerance: f64,
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
    let source_local_four = pose.plane.local_point([depth.a_f, depth.b_f]);
    let sample = ReconstructedSample {
        ambient_four: absolute_plane_point(pose, [depth.a_f, depth.b_f]),
        source_local_four,
        source_zoom_log2: pose.zoom_log2,
        value,
    };
    let projected = ProjectedSample::from_local_point(pose, source_local_four, value)?;
    let pixel_error =
        (projected.screen[0] - source_pixel[0]).hypot(projected.screen[1] - source_pixel[1]);
    let depth_error = (projected.linear_depth - depth.zeta_f).abs();
    if pixel_error > pixel_tolerance || depth_error > depth_tolerance {
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

/// Maps a reconstructed source-local point into a requested exact-anchor frame and projects it.
///
/// `source_to_request_anchor_px` is the exact source anchor relative to the requested anchor in
/// requested-pose pixels. `placement_error_px` combines the independent f64 oracle bound with the
/// compensated residual's rounding bound.
///
/// # Errors
///
/// Returns a typed refusal for invalid placement or target inputs, an error above one pixel, or a
/// target projection pole.
fn project_reconstructed_sample_from_anchor(
    target: &Pose,
    sample: ReconstructedSample,
    source_to_request_anchor_px: [f64; 2],
    placement_error_px: f64,
) -> Result<ProjectedSample, ReprojectionError> {
    let expected_plane =
        construct_plane(target.object).map_err(|_| ReprojectionError::InvalidTarget)?;
    let plane_matches = target
        .plane
        .basis_u
        .iter()
        .chain(target.plane.basis_v.iter())
        .zip(
            expected_plane
                .basis_u
                .iter()
                .chain(expected_plane.basis_v.iter()),
        )
        .all(|(actual, expected)| {
            actual.is_finite() && (*actual - *expected).abs() <= TARGET_PLANE_COMPONENT_TOLERANCE
        });
    if !plane_matches {
        return Err(ReprojectionError::InvalidTarget);
    }
    if !source_to_request_anchor_px
        .into_iter()
        .chain(sample.source_local_four)
        .chain([sample.source_zoom_log2, placement_error_px])
        .all(f64::is_finite)
        || placement_error_px < 0.0
    {
        return Err(ReprojectionError::InvalidSource);
    }
    if placement_error_px > ProjectedSample::PLACEMENT_ERROR_LIMIT_PX {
        return Err(ReprojectionError::UncertifiedPlacement);
    }
    let scale_ratio = (target.zoom_log2 - sample.source_zoom_log2).exp2();
    if !scale_ratio.is_finite() || scale_ratio <= 0.0 || target.grid_width == 0 {
        return Err(ReprojectionError::ProjectionPole);
    }
    let chart_scale = 4.0 / f64::from(target.grid_width);
    let anchor_chart = source_to_request_anchor_px.map(|value| chart_scale * value);
    let anchor_local = target.plane.local_point(anchor_chart);
    let requested_local: [f64; 4] = core::array::from_fn(|axis| {
        scale_ratio.mul_add(sample.source_local_four[axis], anchor_local[axis])
    });
    ProjectedSample::from_local_point(target, requested_local, sample.value)
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
    let local_four = pose.plane.local_point(coordinate);
    core::array::from_fn(|axis| pose.plane_origin[axis] + local_four[axis])
}

fn project_ambient_point(
    pose: &Pose,
    ambient_four: [f64; 4],
    value: RetainedValueSample,
) -> Result<ProjectedSample, ReprojectionError> {
    if !ambient_four.into_iter().all(f64::is_finite) {
        return Err(ReprojectionError::ProjectionPole);
    }
    let local_four: [f64; 4] =
        core::array::from_fn(|axis| ambient_four[axis] - pose.plane_origin[axis]);
    project_local_point(pose, local_four, value)
}

fn project_local_point(
    pose: &Pose,
    local_four: [f64; 4],
    value: RetainedValueSample,
) -> Result<ProjectedSample, ReprojectionError> {
    if pose.grid_width == 0
        || pose.grid_height == 0
        || !pose.view.is_valid()
        || !local_four.into_iter().all(f64::is_finite)
        || !value.record_height.is_finite()
        || matches!(pose.map, PoseMap::EdgeOn)
    {
        return Err(ReprojectionError::ProjectionPole);
    }
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
    if denominator_five <= ProjectedSample::POLE_EPSILON
        || unclamped_five < RELIEF_NEAR_FRACTION * distance_five
    {
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
    if denominator_four <= ProjectedSample::POLE_EPSILON {
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
    if !linear_depth.is_finite() || linear_depth <= ProjectedSample::POLE_EPSILON {
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

    /// The solved binary64 fixture keeps its pixel receipt at the strict existing bound.
    const DECLARED_PIXEL_TOLERANCE: f64 = SOURCE_ROUND_TRIP_EPSILON;
    /// One rounded `f32` depth lane needs two millionths of linear-distance headroom.
    const DECLARED_DEPTH_TOLERANCE: f64 = 2.0e-6;

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
            let projected =
                project_ambient_point(pose, absolute_plane_point(pose, coordinate), value)
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
            let determinant =
                jacobian[0][1].mul_add(-jacobian[1][0], jacobian[0][0] * jacobian[1][1]);
            assert!(determinant.abs() > 1.0e-12);
            let delta = [
                jacobian[0][1].mul_add(-error[1], error[0] * jacobian[1][1]) / determinant,
                error[0].mul_add(-jacobian[1][0], jacobian[0][0] * error[1]) / determinant,
            ];
            coordinate[0] -= delta[0];
            coordinate[1] -= delta[1];
        }
        let projected = project_ambient_point(pose, absolute_plane_point(pose, coordinate), value)
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
        assert!(
            ReconstructedSample::from_source_receipt(
                &source,
                pixel,
                wrong_depth,
                value,
                DECLARED_PIXEL_TOLERANCE,
                DECLARED_DEPTH_TOLERANCE,
            )
            .is_ok()
        );
        assert_eq!(
            ReconstructedSample::from_source_receipt(
                &source,
                pixel,
                depth,
                value,
                -1.0,
                DECLARED_DEPTH_TOLERANCE,
            ),
            Err(ReprojectionError::InvalidSource)
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
                    source_local_four: [0.0; 4],
                    source_zoom_log2: source.zoom_log2,
                    value,
                }
            ),
            Err(ReprojectionError::ProjectionPole)
        );
    }

    #[test]
    fn exact_four_dimensional_pole_boundary_is_refused() {
        let source = pose(ViewControls::NEUTRAL);
        let value = RetainedValueSample {
            record_height: -2.0,
        };
        let sample = ReconstructedSample {
            ambient_four: source.plane_origin,
            source_local_four: [0.0; 4],
            source_zoom_log2: source.zoom_log2,
            value,
        };
        let mut boundary = source;
        boundary.view.distance_four = ProjectedSample::POLE_EPSILON;
        assert_eq!(
            project_reconstructed_sample(&boundary, sample),
            Err(ReprojectionError::ProjectionPole)
        );

        let mut above = boundary;
        above.view.distance_four = f64::from_bits(ProjectedSample::POLE_EPSILON.to_bits() + 1);
        assert!(project_reconstructed_sample(&above, sample).is_ok());
    }

    #[test]
    fn exact_anchor_projection_refuses_a_stale_target_plane() {
        let target = pose(ViewControls::NEUTRAL);
        let sample = ReconstructedSample {
            ambient_four: target.plane_origin,
            source_local_four: [0.0; 4],
            source_zoom_log2: target.zoom_log2,
            value: RetainedValueSample {
                record_height: -2.0,
            },
        };
        assert!(
            sample
                .project_from_anchor(&target, [0.25, -0.5], 0.0)
                .is_ok()
        );

        let mut stale = target;
        stale.plane = Plane {
            basis_u: [0.0; 4],
            basis_v: [0.0; 4],
        };
        assert_eq!(
            sample.project_from_anchor(&stale, [0.25, -0.5], 0.0),
            Err(ReprojectionError::InvalidTarget)
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
