use ember_julibrot_math::{
    Plane, Pose, PoseMap, PrecisionMode, RELIEF_NEAR_FRACTION, ViewControls, plane_chart_relation,
    warp_matrix,
};

use crate::homography::solve_homogeneous;
use crate::{
    SceneFrame, WarpKind, WarpPlan, WarpValidation, apply_homography, pack_homography_rows,
};

const HEIGHT_SAMPLES: [f64; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];
const SCREEN_STEPS: u32 = 9;
const POLE_EPSILON: f64 = 1.0e-4;
const MAX_CHART_RESIDUAL_PX: f64 = 0.5;
const REDRAW_NEUTRAL_EPSILON: f64 = 1.0e-12;

/// Maximum measured reprojection error a displayed warp may move a feature.
pub const WARP_MAX_ERROR_PX: f64 = 1.0;

/// Half a retained texel, the reach of the retained image beyond its outermost sample centre.
///
/// The retained scene's outermost samples sit exactly on the half-extent and own the half pixel
/// past it, so a destination landing inside that footprint is still covered. The tolerance is not
/// cosmetic: a pose reprojected onto itself composes to the identity only as closely as the f32
/// plane basis it was built from allows, which at a tilted pose puts the frame's own border some
/// tens of microns of a pixel outside itself. Without the footprint that reads as a disocclusion,
/// and the exposure it latches restarts the refinement ladder for as long as the pose is held.
const RETAINED_TEXEL_REACH_PX: f64 = 0.5;

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
        if renders_same_picture(from_pose, to_pose) {
            return exact_self(last_frame);
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
        anchor_plan(last_frame, from_pose, to_pose, flat.forward, chart_residual).map_or_else(
            || clear_only(true),
            |plan| enforce_error_ceiling(plan, from_pose, to_pose),
        )
    }
}

/// Selects retained-record redraw when the image homography exceeds the displayed-error ceiling.
///
/// A measured over-ceiling plan keeps its source identity only in the geometrically exact redraw
/// family. Every other over-ceiling or unmeasurable plan clears and waits for a sampled scene.
///
/// A plan only reaches here once the retained records have been shown to describe the destination:
/// the object samples match and the plane-chart residual is inside its limit. What remains is
/// whether the one image homography can carry the picture, and the corpus answers that in pixels.
/// A measured error beyond the ceiling means the destination differs from the retained image by a
/// displacement that depends on each pixel's own escape height — the escape height enters the
/// projection on the fifth ambient axis, so `height_scale`, the fifth-axis distance, the four
/// camera factors that turn that axis into the chart and the fifth translation all move a lifted
/// record by an amount no image map is able to express. That case is `ReliefRedraw`: the retained
/// escape records still describe the destination exactly, so redrawing them under the new pose
/// reprojects the motion with no new sampling at all.
///
/// An unmeasurable corpus is a different matter. It means a perspective pole fell on the sampled
/// relief, where the projection has no finite answer to redraw towards, so the plan stays an
/// honest `ClearOnly`.
fn enforce_error_ceiling(mut plan: WarpPlan, from_pose: &Pose, to_pose: &Pose) -> WarpPlan {
    match plan.approx_max_error_px {
        Some(error) if error <= WARP_MAX_ERROR_PX => return plan,
        Some(_) if exact_relief_redraw_family(from_pose, to_pose) => {
            plan.exposed = true;
            plan.kind = WarpKind::ReliefRedraw;
            return plan;
        }
        Some(_) | None => {}
    }
    plan.source_scene_id = None;
    plan.source_texture_index = None;
    plan.source_valid = false;
    plan.exposed = true;
    plan.kind = WarpKind::ClearOnly;
    plan
}

/// Whether the retained grid is a proved exact record source for a relief redraw.
///
/// A pure height or fifth-distance change keeps every sampled ambient four-point fixed, so the
/// retained records are the destination records. More generally, with neutral five-dimensional
/// rotation and translation, equal constructed plane spans let an in-plane basis change express
/// the retained record at the same ambient point before its destination lift; each constant-height
/// layer remains one fixed plane that the later perspectives and observer map projectively. Equal
/// non-neutral cameras do not suffice because mixing the height axis generally destroys that
/// fixed-plane relation.
fn exact_relief_redraw_family(from: &Pose, to: &Pose) -> bool {
    if [from.grid_width, from.grid_height] != [to.grid_width, to.grid_height] {
        return false;
    }
    (neutral_five_camera(from.view)
        && neutral_five_camera(to.view)
        && same_sampling_lattice(from, to))
        || pure_height_or_fifth_distance(from, to)
}

fn neutral_five_camera(view: ViewControls) -> bool {
    view.camera
        .into_iter()
        .chain(view.camera_translation)
        .all(|value| value.abs() <= REDRAW_NEUTRAL_EPSILON)
}

fn pure_height_or_fifth_distance(from: &Pose, to: &Pose) -> bool {
    arrays_close(from.view.camera, to.view.camera)
        && arrays_close(from.view.camera_translation, to.view.camera_translation)
        && close(from.view.camera_yaw, to.view.camera_yaw)
        && close(from.view.camera_pitch, to.view.camera_pitch)
        && close(from.view.distance_four, to.view.distance_four)
        && close(from.zoom_log2, to.zoom_log2)
        && arrays_close(from.plane_origin, to.plane_origin)
        && arrays_close(from.centre_from_reference_px, to.centre_from_reference_px)
}

fn same_sampling_lattice(from: &Pose, to: &Pose) -> bool {
    close(from.zoom_log2, to.zoom_log2)
        && arrays_close(from.plane_origin, to.plane_origin)
        && arrays_close(from.centre_from_reference_px, to.centre_from_reference_px)
}

fn arrays_close<const N: usize>(from: [f64; N], to: [f64; N]) -> bool {
    from.into_iter().zip(to).all(|(from, to)| close(from, to))
}

fn close(from: f64, to: f64) -> bool {
    (from - to).abs() <= REDRAW_NEUTRAL_EPSILON
}

fn object_samples_match(from: &Pose, to: &Pose) -> bool {
    plane_chart_relation(from.plane, to.plane).is_some()
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

/// Whether two poses put the same picture on the surface.
///
/// Every field that decides a pixel is compared: the slice and its origin, the object angles, the
/// zoom, the view controls, the sampled extent, the screen map, and the displacement from the
/// accepted reference. `epoch` and `orbit_generation` are not among them. They are publication
/// bookkeeping — the epoch advances on every HOT write, so a whole-`Pose` equality is false on the
/// refresh after the one that captured it and stays false for as long as the view is held, which
/// makes it useless as a test of what is on screen.
#[must_use]
#[allow(
    clippy::float_cmp,
    reason = "these coordinates are copied between poses, never recomputed, so identity is exact"
)]
pub fn renders_same_picture(first: &Pose, second: &Pose) -> bool {
    first.plane == second.plane
        && first.object == second.object
        && first.plane_origin == second.plane_origin
        && first.zoom_log2 == second.zoom_log2
        && first.view == second.view
        && first.grid_width == second.grid_width
        && first.grid_height == second.grid_height
        && first.map == second.map
        && first.centre_from_reference_px == second.centre_from_reference_px
}

/// The exact plan for a retained scene whose pose is the one being displayed.
///
/// Sampling a scene at the pose it was rendered at is the identity: every destination pixel reads
/// the source pixel it came from. There is no reprojection to approximate here and therefore none
/// to measure, so the sampled corpus must not be consulted. It can fail to be measurable for
/// reasons that say nothing about this plan — a relief lattice sample behind a perspective pole,
/// a screen sample beyond the horizon — and an unmeasurable corpus refuses. Refusing the identity
/// is not an honest clear, and unlike every other refusal it cannot recover: the clear counts as
/// exposure, exposure restarts the refinement ladder, and the ladder can only deliver the same
/// scene at the same pose to be refused again. That loop is what a held relief pose with a horizon
/// inside its frame used to sit in, showing the clear colour with a completed Final in hand.
const fn exact_self(last_frame: &SceneFrame) -> WarpPlan {
    WarpPlan {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        source_scene_id: Some(last_frame.scene_id),
        source_texture_index: Some(last_frame.texture_index),
        source_valid: true,
        edge_on: false,
        exposed: false,
        kind: WarpKind::AnchorHomography,
        chart_residual: 0.0,
        approx_max_error_px: Some(0.0),
        approx_p95_error_px: Some(0.0),
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
    let half_width = f64::from(from.grid_width).mul_add(0.5, RETAINED_TEXEL_REACH_PX);
    let half_height = f64::from(from.grid_height).mul_add(0.5, RETAINED_TEXEL_REACH_PX);
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

/// Whether one screen point lies in front of that pose's screen-map horizon.
///
/// Beyond the horizon the scene pass shows the exterior sky, so the point carries no reprojection
/// error to measure. This is not the perspective pole further down the projection chain: a pole on
/// the sampled relief surface is unbounded and still refuses the plan.
fn in_front_of_horizon(pose: &Pose, screen: [f64; 2]) -> bool {
    let PoseMap::Mapped(map) = pose.map else {
        return false;
    };
    let weight = homogeneous(map.rows, screen)[2];
    weight.is_finite() && weight > 0.0
}

/// Measures the displayed sampling against the full forward chain over the screen-and-relief
/// lattice.
///
/// A sample beyond either pose's horizon is skipped rather than refusing the whole corpus: the
/// exterior sky is what the scene pass leaves there and the warp pass carries it across, so there
/// is no reprojection error to measure. Refusing instead cleared the surface of every pose whose
/// horizon crossed the frame, a settled pose warping onto itself included. Everything else that
/// cannot be projected — a perspective pole on the sampled surface — still refuses, as does a
/// non-finite error, which is broken arithmetic rather than geometry. `None` means the corpus
/// could not be measured at all.
fn sampled_errors(from_pose: &Pose, to_pose: &Pose, approximate: [f64; 9]) -> Option<Vec<f64>> {
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
            if !in_front_of_horizon(to_pose, target_screen)
                || !in_front_of_horizon(from_pose, source_screen)
            {
                continue;
            }
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
/// `record_height` is the escape record's normalized height in `[-2,2]`. The five-dimensional
/// near pole is clamped; `None` means the later four-dimensional or observer projection is behind
/// its pole and the exterior sky remains visible.
#[must_use]
pub fn project_scene_point(pose: &Pose, screen: [f64; 2], record_height: f64) -> Option<[f64; 2]> {
    project_scene_vertex(pose, screen, record_height).map(|projected| projected.0)
}

/// Mirrors one scene vertex and returns its screen point with its clip-space `w`.
///
/// The second value lets CPU raster oracles reproduce perspective-correct interpolation of the
/// grid coordinate. `None` means the vertex lies behind a later perspective pole.
#[must_use]
pub fn project_scene_vertex(
    pose: &Pose,
    screen: [f64; 2],
    record_height: f64,
) -> Option<([f64; 2], f64)> {
    project_scene_vertex_with_shortcut(pose, screen, record_height, true)
}

#[cfg(test)]
fn project_scene_point_with_shortcut(
    pose: &Pose,
    screen: [f64; 2],
    record_height: f64,
    flat_shortcut: bool,
) -> Option<[f64; 2]> {
    project_scene_vertex_with_shortcut(pose, screen, record_height, flat_shortcut)
        .map(|projected| projected.0)
}

#[allow(
    clippy::float_cmp,
    reason = "height zero selects the scene shader's exact identity branch"
)]
fn project_scene_vertex_with_shortcut(
    pose: &Pose,
    screen: [f64; 2],
    record_height: f64,
    flat_shortcut: bool,
) -> Option<([f64; 2], f64)> {
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
    let height = pose.view.height_scale * (record_height + 2.0) * 0.5;
    if flat_shortcut && height == 0.0 && map.apron_scale.to_bits() == 1.0_f64.to_bits() {
        return Some((screen, 1.0));
    }
    let chart_scale = 4.0 * map.apron_scale / f64::from(pose.grid_width);
    let chart_coordinate = [chart_scale * mapped[0], chart_scale * mapped[1]];
    let rotated = ambient_point(pose.plane, chart_coordinate, height, &pose.view);
    let distance_five = pose.view.distance_five;
    let distance_four = pose.view.distance_four;
    let denominator_five = (distance_five - rotated[4]).max(RELIEF_NEAR_FRACTION * distance_five);
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
        .then_some((projected, clip_w))
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

    fn set_object(pose: &mut Pose, object: ObjectAngles) {
        pose.object = object;
        pose.plane = construct_plane(object).expect("fixture object constructs a plane");
        pose.map = PoseMap::Mapped(map(
            pose.object,
            pose.view,
            [pose.grid_width, pose.grid_height],
        ));
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
        let mut large_object = large.object;
        large_object.rho_13 += 0.3;
        set_object(&mut large, large_object);
        assert_eq!(
            reproject(&frame(&from), &from, &large).kind,
            WarpKind::ClearOnly
        );

        let mut rounded = from;
        let mut rounded_object = rounded.object;
        rounded_object.rho_13 += 1.0e-9;
        set_object(&mut rounded, rounded_object);
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
    fn plane_stabilizer_object_turns_are_exact_flat_warps() {
        let retained_object = ObjectAngles::IDENTITY;
        let retained_plane = construct_plane(retained_object).expect("identity plane constructs");
        let from = object_pose(
            retained_object,
            retained_plane,
            ViewControls::MANDELBROT_FLAT,
            [0.0; 2],
        );

        for requested_object in [
            ObjectAngles {
                rho_34: 0.3,
                ..retained_object
            },
            ObjectAngles {
                rho_12: 0.5,
                ..retained_object
            },
        ] {
            let to = object_pose(
                requested_object,
                construct_plane(requested_object).expect("stabilizer plane constructs"),
                ViewControls::MANDELBROT_FLAT,
                [0.0; 2],
            );
            let plan = reproject(&frame(&from), &from, &to);
            assert_eq!(plan.kind, WarpKind::AnchorHomography);
            assert_eq!(plan.approx_max_error_px, Some(0.0));
            assert!(plan.source_valid);
        }
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
    fn floor_shortcut_matches_the_full_forward_chain() {
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
                        let record_height = if view.height_scale.to_bits() == 0.0_f64.to_bits() {
                            0.0
                        } else {
                            -2.0
                        };
                        let shortcut = project_scene_point(&posed, screen, record_height)
                            .expect("the floor shortcut projects");
                        let full =
                            project_scene_point_with_shortcut(&posed, screen, record_height, false)
                                .expect("the full floor chain projects");
                        assert_eq!(shortcut, screen);
                        let error = (shortcut[0] - full[0]).hypot(shortcut[1] - full[1]);
                        assert!(error <= 1.0e-9, "full-chain error was {error} px");
                    }
                }
            }
        }
    }

    #[test]
    fn zero_height_amplitude_is_bit_identical_for_every_record_height() {
        let posed = pose(ViewControls::NEUTRAL, [0.0; 2]);
        for record_height in HEIGHT_SAMPLES {
            for screen in screen_corners(&posed).into_iter().chain([[0.0; 2]]) {
                assert_eq!(
                    project_scene_point(&posed, screen, record_height),
                    Some(screen)
                );
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
            apron_scale: 1.0,
        });
        let PoseMap::Mapped(to_map) = to.map else {
            panic!("fixture must be mapped");
        };
        let weights = screen_corners(&from).map(|corner| homogeneous(to_map.rows, corner)[2]);
        assert!(weights.iter().any(|weight| *weight <= 0.0));
        assert!(weights.iter().any(|weight| *weight > 0.0));
        // A corner beyond the horizon stays a homogeneous anchor. The plan is solved through it
        // and judged on the lattice in front of the horizon, because beyond it there is nothing to
        // reproject: the scene pass leaves the exterior sky and the warp pass carries it across.
        // Clearing the picture instead would leave every pose whose horizon crosses the frame
        // permanently unpainted.
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::AnchorHomography);
        assert!(plan.source_valid);
        assert!(
            plan.approx_max_error_px
                .is_some_and(|error| error <= WARP_MAX_ERROR_PX)
        );
        let measured = sampled_errors(&from, &to, unpack_rows(plan.rows))
            .expect("the corpus measures what lies in front of the horizon");
        assert_ne!(measured.len(), 0);
    }

    #[test]
    fn a_settled_near_edge_on_pose_shows_its_scene_instead_of_the_clear_colour() {
        // The near-edge-on Mandelbrot object rotation the browser proof settles on. A third of
        // this frame is beyond the map's horizon, and the pose is still an ordinary mapped one,
        // not the refused edge-on state.
        let object = ObjectAngles {
            rho_13: 1.5,
            ..ObjectAngles::IDENTITY
        };
        let plane = construct_plane(object).expect("the tilted fixture plane is orthonormal");
        let mut settled = object_pose(object, plane, ViewControls::MANDELBROT_FLAT, [0.0; 2]);
        set_extent(&mut settled, [960, 540]);
        let PoseMap::Mapped(settled_map) = settled.map else {
            panic!("fixture must be mapped");
        };
        let weights: Vec<f64> = (0..SCREEN_STEPS)
            .flat_map(|row| (0..SCREEN_STEPS).map(move |column| (row, column)))
            .map(|(row, column)| {
                let screen = [
                    (f64::from(column) / f64::from(SCREEN_STEPS - 1) - 0.5)
                        * f64::from(settled.grid_width),
                    (f64::from(row) / f64::from(SCREEN_STEPS - 1) - 0.5)
                        * f64::from(settled.grid_height),
                ];
                homogeneous(settled_map.rows, screen)[2]
            })
            .collect();
        assert!(weights.iter().any(|weight| *weight <= 0.0));
        assert!(weights.iter().any(|weight| *weight > 0.0));

        // A ladder that has settled reprojects the retained scene onto its own pose. That plan is
        // the exact identity, so the completed scene is what the surface shows: every pixel is
        // mesh or sky, none of it the clear colour, and no exposure is latched.
        let plan = reproject(&frame(&settled), &settled, &settled);
        assert_eq!(plan.kind, WarpKind::AnchorHomography);
        assert!(plan.source_valid);
        assert!(!plan.exposed);
        assert!(plan.approx_max_error_px.is_some_and(|error| error < 1.0e-9));

        // The measured corpus is thinned by the horizon rather than abandoned at it: samples do
        // survive, and the plan is judged on them.
        let steps = usize::try_from(SCREEN_STEPS).expect("the step count fits a machine word");
        let whole_lattice = steps * steps * HEIGHT_SAMPLES.len();
        let measured = sampled_errors(&settled, &settled, unpack_rows(plan.rows))
            .expect("the settled corpus measures the samples the horizon leaves");
        assert_ne!(measured.len(), 0);
        assert!(measured.len() < whole_lattice);

        // The retained texel's reach is load-bearing here: this pose composed onto itself is the
        // identity only as closely as its f32 plane basis allows, so the frame's own border lands
        // a fraction of a pixel outside the retained image. Measured without that reach it reads
        // as a disocclusion, and the exposure it latches restarts the ladder for as long as the
        // pose is held.
        let flat = warp_matrix(&settled, &settled).expect("a pose composes onto itself");
        let drift = screen_corners(&settled)
            .into_iter()
            .filter_map(|corner| {
                apply_homography(flat.forward, corner)
                    .map(|moved| (moved[0] - corner[0]).hypot(moved[1] - corner[1]))
            })
            .fold(0.0_f64, f64::max);
        assert!(drift > 0.0);
        assert!(drift < RETAINED_TEXEL_REACH_PX);
    }

    #[test]
    fn tumbled_cross_term_above_the_homography_ceiling_clears() {
        let from = pose(relief(0.6), [0.0; 2]);
        let mut to = pose(relief(0.8), [8.0, -4.0]);
        to.zoom_log2 += 0.25;
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::ClearOnly);
        assert!(!plan.source_valid);
        assert!(plan.exposed);
        assert_eq!(plan.source_scene_id, None);
        assert_eq!(plan.source_texture_index, None);
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
            assert!(plan.approx_max_error_px.is_none_or(f64::is_finite));
        }
    }

    /// The height control is the one observer degree of freedom the image homography cannot hold.
    ///
    /// The same plan is proved exact on the record floor and beyond the ceiling above it, which is
    /// what makes the refusal geometry rather than arithmetic: the destination differs from the
    /// retained image by a displacement each pixel takes from its height above that floor.
    #[test]
    fn a_pure_height_change_selects_a_relief_redraw_and_is_flat_exact() {
        let mut lifted = ViewControls::NEUTRAL;
        lifted.camera_yaw = 0.3;
        let from = pose(lifted, [0.0; 2]);
        let mut raised = lifted;
        raised.height_scale = 1.0;
        let to = pose(raised, [0.0; 2]);

        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::ReliefRedraw);
        assert!(plan.source_valid);
        assert!(plan.exposed);
        assert_eq!(plan.source_scene_id, Some(3));
        let maximum = plan
            .approx_max_error_px
            .expect("a relief redraw still publishes the measured maximum");
        assert!(
            maximum > WARP_MAX_ERROR_PX,
            "a relief redraw is measured over the ceiling, not unmeasurable: {maximum}"
        );

        let displayed = unpack_rows(plan.rows);
        let mut floor_error = 0.0_f64;
        let mut lifted_error = 0.0_f64;
        for corner in screen_corners(&to).into_iter().chain([[0.0; 2]]) {
            for (height, worst) in [(-2.0, &mut floor_error), (1.0, &mut lifted_error)] {
                let source =
                    apply_homography(displayed, corner).expect("displayed map has no pole");
                let destination =
                    project_scene_point(&to, corner, height).expect("destination projects");
                let expected =
                    project_scene_point(&from, source, height).expect("retained pose projects");
                let approximate =
                    apply_homography(displayed, destination).expect("displayed map has no pole");
                *worst =
                    worst.max((approximate[0] - expected[0]).hypot(approximate[1] - expected[1]));
            }
        }
        assert!(
            floor_error <= 1.0e-6,
            "the same map carries every floor record exactly: {floor_error}"
        );
        assert!(
            lifted_error > WARP_MAX_ERROR_PX,
            "the record height alone breaks it: {lifted_error}"
        );
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "the requested relief corpus publishes its freshly measured pixel table"
    )]
    fn floor_anchored_relief_corpus_pins_the_published_measurements() {
        fn published_pose(height_scale: f64, distance_five: f64) -> Pose {
            let mut posed = pose(
                ViewControls {
                    height_scale,
                    distance_five,
                    ..ViewControls::NEUTRAL
                },
                [0.0; 2],
            );
            set_extent(&mut posed, [960, 540]);
            posed
        }

        fn maximum(from: &Pose, to: &Pose) -> f64 {
            reproject(&frame(from), from, to)
                .approx_max_error_px
                .expect("the published relief fixture is measurable")
        }

        let flat = published_pose(0.0, 8.0);
        let step = published_pose(0.005, 8.0);
        let one = published_pose(1.0, 8.0);
        let half = published_pose(0.5, 8.0);
        let nearer = published_pose(1.0, 6.0);
        let measured = [
            maximum(&flat, &step),
            maximum(&flat, &one),
            maximum(&one, &half),
            maximum(&one, &nearer),
        ];
        let expected = [0.689, 183.58, 104.90, 91.79];
        let tolerances = [0.0005, 0.005, 0.005, 0.005];
        for ((actual, expected), tolerance) in measured.into_iter().zip(expected).zip(tolerances) {
            assert!(
                (actual - expected).abs() < tolerance,
                "measured {actual} px, expected published rounding {expected} px"
            );
        }
        eprintln!(
            "floor relief corpus | 0 -> 0.005 = {:.6} px | 0 -> 1 = {:.6} px | 1 -> 0.5 = {:.6} px | d5 8 -> 6 h1 = {:.6} px",
            measured[0], measured[1], measured[2], measured[3]
        );
    }

    #[test]
    fn exact_relief_redraw_family_follows_the_fixed_plane_proof() {
        let neutral = pose(ViewControls::NEUTRAL, [0.0; 2]);
        let mut neutral_observer = neutral;
        neutral_observer.view.camera_yaw = 0.4;
        neutral_observer.view.camera_pitch = -0.2;
        neutral_observer.view.distance_four = 6.0;
        assert!(exact_relief_redraw_family(&neutral, &neutral_observer));

        let mut resampled = neutral_observer;
        resampled.zoom_log2 = 0.25;
        resampled.centre_from_reference_px = [3.0, -2.0];
        assert!(!exact_relief_redraw_family(&neutral, &resampled));

        let tumbled = pose(relief(0.4), [0.0; 2]);
        let mut pure_height = tumbled;
        pure_height.view.height_scale = 0.5;
        assert!(exact_relief_redraw_family(&tumbled, &pure_height));

        let mut pure_fifth_distance = tumbled;
        pure_fifth_distance.view.distance_five = 6.0;
        assert!(exact_relief_redraw_family(&tumbled, &pure_fifth_distance));

        let mut cross_term = pure_height;
        cross_term.view.camera_yaw += 0.1;
        assert!(!exact_relief_redraw_family(&tumbled, &cross_term));

        let mut resized = pure_height;
        resized.grid_width /= 2;
        resized.grid_height /= 2;
        assert!(!exact_relief_redraw_family(&tumbled, &resized));
    }

    #[test]
    fn the_five_dimensional_pole_is_clamped_but_resampling_still_clears() {
        // The bounded near plane turns the former five-dimensional pole into a closed finite
        // surface. Moving the sampling lattice still requires fresh records, so this finite
        // over-ceiling plan clears rather than mislabelling stale records as an exact redraw.
        let view = ViewControls {
            height_scale: 1.0,
            distance_five: 1.0,
            ..ViewControls::NEUTRAL
        };
        let from = pose(view, [0.0; 2]);
        let mut to = pose(view, [3.0, -2.0]);
        to.zoom_log2 += 0.125;
        assert_ne!(from, to);
        let plan = reproject(&frame(&from), &from, &to);
        assert_eq!(plan.kind, WarpKind::ClearOnly);
        assert!(!plan.source_valid);
        assert!(plan.approx_max_error_px.is_some_and(f64::is_finite));
    }

    /// The owner's broken row, taken from the page's own Copy row JSON.
    fn owner_relief_row() -> ViewControls {
        ViewControls {
            height_scale: 1.327,
            distance_five: 8.0,
            distance_four: 8.0,
            camera_yaw: 0.0,
            camera_pitch: 0.0,
            camera: {
                let mut camera = [0.0; 10];
                camera[1] = -core::f64::consts::FRAC_PI_2;
                camera[4] = -core::f64::consts::FRAC_PI_2;
                camera
            },
            camera_translation: [0.0; 5],
        }
    }

    #[test]
    fn the_owner_relief_row_presents_its_completed_scene_and_stops_asking_for_another() {
        let object = ObjectAngles {
            rho_13: 1.174_251_648_646_25,
            rho_23: 1.612_768_861_833_65,
            ..ObjectAngles::IDENTITY
        };
        let view = owner_relief_row();
        let plane = construct_plane(object).expect("the row's plane is orthonormal");
        let mut settled = object_pose(object, plane, view, [0.0; 2]);
        settled.zoom_log2 = 0.0;
        settled.plane_origin = [0.0; 4];
        set_extent(&mut settled, [960, 540]);
        let PoseMap::Mapped(map) = settled.map else {
            panic!("the row is an ordinary mapped pose, not the edge-on state");
        };
        // The two conditions that used to combine into the refusal: a horizon crosses the frame,
        // and the relief lattice reaches past a perspective pole so the corpus cannot be measured.
        assert!(map.condition_number > 1.0);
        let horizon: Vec<f64> = screen_corners(&settled)
            .into_iter()
            .map(|corner| homogeneous(map.rows, corner)[2])
            .collect();
        assert!(horizon.iter().any(|weight| *weight <= 0.0));
        assert!(horizon.iter().any(|weight| *weight > 0.0));
        assert_ne!(view.height_scale, 0.0);

        // One Final completes at this pose, and every later refresh reprojects it onto itself.
        // That plan is the exact identity: the surface is the scene, mesh where the map is finite
        // and exterior sky beyond the horizon, and never the clear colour.
        let mut retained = frame(&settled);
        for refresh in 0..16 {
            // Publication bookkeeping advances on every refresh while the view is held. It must
            // not decide what is on screen: whole-pose equality would be false from the second
            // refresh onward, which is exactly how this row stayed clear with a Final in hand.
            let mut current = settled;
            current.epoch = settled.epoch + u64::from(refresh) + 1;
            current.orbit_generation = settled.orbit_generation + refresh;
            retained.pose.epoch = settled.epoch;
            assert!(renders_same_picture(&retained.pose, &current));
            let plan = reproject(&retained, &retained.pose.clone(), &current);
            assert_eq!(plan.kind, WarpKind::AnchorHomography, "refresh {refresh}");
            assert!(plan.source_valid, "refresh {refresh}");
            assert_eq!(plan.source_scene_id, Some(retained.scene_id));
            assert_eq!(plan.source_texture_index, Some(retained.texture_index));
            assert_eq!(plan.approx_max_error_px, Some(0.0), "refresh {refresh}");
            // Nothing here asks the ladder for another scene, so it idles instead of restarting.
            assert!(!plan.exposed, "refresh {refresh}");
        }
        // A pose that is not this picture is still measured, and at this row still refused.
        let mut moved = settled;
        moved.centre_from_reference_px = [9.0, -6.0];
        assert!(!renders_same_picture(&settled, &moved));
        assert_eq!(
            reproject(&frame(&settled), &settled, &moved).kind,
            WarpKind::ClearOnly
        );
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
            let destination = project_scene_point_with_shortcut(&to, target, -2.0, false)
                .expect("translated full chain projects");
            let expected = project_scene_point_with_shortcut(&from, source_flat, -2.0, false)
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
        let with_translation =
            project_scene_point(&to, [200.0, -100.0], 1.0).expect("translated relief projects");
        let without_translation =
            project_scene_point(&from, [200.0, -100.0], 1.0).expect("untranslated relief projects");
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
        let full =
            sampled_errors(&from, &to, approximate).expect("the full validation corpus projects");
        assert_eq!(full.len(), 9 * 9 * HEIGHT_SAMPLES.len());
        assert_eq!(
            ordinary.approx_max_error_px,
            full.iter().copied().reduce(f64::max)
        );

        let measured = Warp::reproject(
            &frame(&from),
            &from,
            &to,
            PrecisionMode::PictureFast,
            WarpValidation::Measure,
        );
        assert_eq!(
            measured.approx_max_error_px,
            full.into_iter().reduce(f64::max)
        );
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "the timed corpus probe reports the measured per-plan evidence"
    )]
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
    fn non_neutral_relief_sweep_measures_its_envelope_and_refuses_over_ceiling_rows() {
        let mut observed_max = 0.0_f64;
        let mut observed_p95 = 0.0_f64;
        let mut cleared = 0_u32;
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
                assert_ne!(plan.kind, WarpKind::ReliefRedraw);
                if plan.kind == WarpKind::ClearOnly {
                    cleared = cleared.saturating_add(1);
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
            cleared > 0,
            "the measured non-neutral relief envelope never refused an over-ceiling row"
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
