//! Where the lifted scene mesh actually lands on the render surface.
//!
//! Screen-aligned sampling chooses chart points by inverting the scene map at height zero, so at
//! `height_scale = 0` every mesh vertex is its own screen pixel and the mesh tiles the frame
//! exactly. Lifting a vertex into the fifth coordinate moves it: the five-to-four perspective
//! divides by `d₅ − h`, so a vertex whose record sits below the chart is pulled toward the frame
//! centre and one above it is pushed out. The frame's own boundary vertices are therefore no
//! longer on the frame boundary, and the mesh stops covering the surface it was sampled for.
//!
//! This module mirrors the scene shader's vertex chain in binary64 and reports, for one pose, how
//! far the boundary moves, how much overscan the sampling extent would need to undo the movement,
//! and what share of the surface the mesh cannot reach. It reads no records: the bound is taken
//! over the whole height range a record can occupy, `record_height ∈ [−2,2]` scaled by the height
//! control, which makes it a property of the pose alone.

#![allow(
    clippy::cast_precision_loss,
    reason = "lattice and edge counts are small integers chosen in this module"
)]
#![allow(
    clippy::suboptimal_flops,
    reason = "the chain mirrors the shader's written operation order rather than a faster one"
)]

use crate::{MathError, ObjectAngles, Plane, ViewControls, construct_plane, screen::camera_matrix};

/// Points sampled along each of the four frame edges when tracing the lifted boundary.
const BOUNDARY_SAMPLES_PER_EDGE: usize = 64;

/// Side of the fixed square lattice the uncovered share is counted on.
const COVERAGE_LATTICE_SIDE: usize = 65;

/// Guard the scene shader applies to both perspective denominators before dividing.
const DENOMINATOR_EPSILON: f64 = 1.0e-4;

/// Largest sampling-map apron the application will spend at a fixed record count.
pub const MAX_APRON_SCALE: f64 = 2.0;

/// Where one pose's lifted mesh lands relative to the frame it was sampled for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneFootprint {
    /// Smallest radial scale the frame boundary reaches under the height range; `1.0` covers.
    pub boundary_scale: f64,
    /// Screen-space overscan applied to the sampling extent, clamped to [`MAX_APRON_SCALE`].
    pub apron_scale: f64,
    /// Unclamped screen-space overscan needed to cover the complete frame.
    pub requested_apron_scale: f64,
    /// Share of the render surface the mesh cannot reach after applying `apron_scale`, in `[0,1]`.
    pub uncovered_fraction: f64,
}

impl SceneFootprint {
    /// The flat-chart answer: the mesh is its own frame and nothing is uncovered.
    pub const COVERED: Self = Self {
        boundary_scale: 1.0,
        apron_scale: 1.0,
        requested_apron_scale: 1.0,
        uncovered_fraction: 0.0,
    };
}

/// Measures the lifted mesh footprint for one pose.
///
/// The result is independent of `zoom_log2`: zoom enters the picture only through
/// [`crate::pixel_scale`], which converts plane offsets into complex coordinates for the kernels.
/// The scene map and the vertex chain are built from the plane, the view controls and the grid
/// width alone, so the inward pull is a screen-space quantity and the same at every zoom.
///
/// # Errors
///
/// Returns an error for invalid controls, a zero extent, or a pose whose map is degenerate.
pub fn scene_footprint(
    object: &ObjectAngles,
    view: &ViewControls,
    grid_w: u32,
    grid_h: u32,
) -> Result<SceneFootprint, MathError> {
    if !object.is_valid() || !view.is_valid() {
        return Err(MathError::InvalidViewControls);
    }
    if grid_w == 0 || grid_h == 0 {
        return Err(MathError::InvalidExtent);
    }
    let aspect = f64::from(grid_w) / f64::from(grid_h);
    let map = crate::screen_to_plane(object, view, 0.0, grid_w, grid_h, aspect)?;
    let plane = construct_plane(*object)?;

    // `record_height` spans [-2,2]; the height control scales it. The fifth coordinate is affine
    // in the record height and `d₅/(d₅ − h)` is monotone in it, so the extremes of the range bound
    // every intermediate lift and two boundary traces are enough.
    let amplitude = 2.0 * view.height_scale;
    let unscaled_boundaries = [-amplitude, amplitude]
        .map(|height| lifted_boundary(&map, plane, view, grid_w, grid_h, aspect, height, 1.0));
    let unscaled_traces: Vec<&Vec<[f64; 2]>> = unscaled_boundaries.iter().flatten().collect();
    if unscaled_traces.is_empty() {
        // Every boundary vertex fell past a perspective pole. The shader clips those triangles and
        // paints sky there, which is the geometry answering rather than a coverage shortfall.
        return Ok(SceneFootprint {
            boundary_scale: 1.0,
            apron_scale: 1.0,
            requested_apron_scale: 1.0,
            uncovered_fraction: 1.0,
        });
    }
    let mut boundary_scale = 1.0_f64;
    for point in unscaled_traces.iter().flat_map(|ring| ring.iter()) {
        let radius = point[0].abs().max(point[1].abs());
        boundary_scale = boundary_scale.min(radius);
    }
    if !boundary_scale.is_finite() || boundary_scale <= 0.0 {
        return Err(MathError::DegenerateViewMap);
    }
    let requested_apron_scale = (1.0 / boundary_scale).max(1.0);
    let apron_scale = requested_apron_scale.min(MAX_APRON_SCALE);
    let applied_boundaries = [-amplitude, amplitude].map(|height| {
        lifted_boundary(
            &map,
            plane,
            view,
            grid_w,
            grid_h,
            aspect,
            height,
            apron_scale,
        )
    });
    let applied_traces: Vec<&Vec<[f64; 2]>> = applied_boundaries.iter().flatten().collect();
    let mut covered = 0_usize;
    let mut total = 0_usize;
    for row in 0..COVERAGE_LATTICE_SIDE {
        for column in 0..COVERAGE_LATTICE_SIDE {
            let point = lattice_point(column, row);
            total += 1;
            if applied_traces.iter().all(|ring| encloses(ring, point)) {
                covered += 1;
            }
        }
    }
    let uncovered = 1.0 - covered as f64 / total as f64;
    Ok(SceneFootprint {
        boundary_scale,
        apron_scale,
        requested_apron_scale,
        uncovered_fraction: uncovered,
    })
}

/// Normalized device coordinate of one lattice cell centre across the frame.
fn lattice_point(column: usize, row: usize) -> [f64; 2] {
    let side = COVERAGE_LATTICE_SIDE as f64;
    [
        2.0 * ((column as f64 + 0.5) / side) - 1.0,
        2.0 * ((row as f64 + 0.5) / side) - 1.0,
    ]
}

/// Traces the frame boundary through the scene chain at one fixed lift, in device coordinates.
///
/// Returns `None` when any boundary vertex fails a perspective guard, which is the same condition
/// under which the shader clips the vertex and leaves the sky behind it.
fn lifted_boundary(
    map: &crate::Homography,
    plane: Plane,
    view: &ViewControls,
    grid_w: u32,
    grid_h: u32,
    aspect: f64,
    height: f64,
    apron_scale: f64,
) -> Option<Vec<[f64; 2]>> {
    let half_w = f64::from(grid_w) * 0.5;
    let half_h = f64::from(grid_h) * 0.5;
    let steps = BOUNDARY_SAMPLES_PER_EDGE;
    let mut ring = Vec::with_capacity(4 * steps);
    for edge in 0..4 {
        for step in 0..steps {
            let t = step as f64 / steps as f64;
            let screen = match edge {
                0 => [-half_w + 2.0 * half_w * t, -half_h],
                1 => [half_w, -half_h + 2.0 * half_h * t],
                2 => [half_w - 2.0 * half_w * t, half_h],
                _ => [-half_w, half_h - 2.0 * half_h * t],
            };
            ring.push(project_vertex(
                map,
                plane,
                view,
                grid_w,
                aspect,
                screen,
                height,
                apron_scale,
            )?);
        }
    }
    Some(ring)
}

/// Mirrors one scene-shader vertex: map to the chart, lift, both perspectives, then the observer.
fn project_vertex(
    map: &crate::Homography,
    plane: Plane,
    view: &ViewControls,
    grid_w: u32,
    aspect: f64,
    screen: [f64; 2],
    height: f64,
    apron_scale: f64,
) -> Option<[f64; 2]> {
    let rows = map.rows;
    let homogeneous: [f64; 3] = core::array::from_fn(|row| {
        rows[3 * row] * screen[0] + rows[3 * row + 1] * screen[1] + rows[3 * row + 2]
    });
    if !homogeneous.iter().all(|value| value.is_finite()) || homogeneous[2] <= 0.0 {
        return None;
    }
    let offset = [
        homogeneous[0] / homogeneous[2],
        homogeneous[1] / homogeneous[2],
    ];
    let chart_scale = 4.0 * apron_scale / f64::from(grid_w);
    let display: [f64; 4] = core::array::from_fn(|axis| {
        chart_scale
            * offset[0].mul_add(
                f64::from(plane.basis_u[axis]),
                offset[1] * f64::from(plane.basis_v[axis]),
            )
    });

    let ambient_in = [display[0], display[1], display[2], display[3], height];
    let matrix = camera_matrix(view);
    let mut ambient: [f64; 5] = core::array::from_fn(|row| {
        (0..5).fold(0.0, |sum, column| {
            matrix[row][column].mul_add(ambient_in[column], sum)
        })
    });
    for (axis, value) in ambient.iter_mut().enumerate() {
        *value += view.camera_translation[axis];
    }

    let denominator_five = view.distance_five - ambient[4];
    if denominator_five <= DENOMINATOR_EPSILON {
        return None;
    }
    let scale_five = view.distance_five / denominator_five;
    let projected = [
        ambient[0] * scale_five,
        ambient[1] * scale_five,
        ambient[2] * scale_five,
        ambient[3] * scale_five,
    ];
    let denominator_four = view.distance_four - projected[3];
    if denominator_four <= DENOMINATOR_EPSILON {
        return None;
    }
    let scale_four = view.distance_four / denominator_four;
    let world = [
        projected[0] * scale_four,
        projected[1] * scale_four,
        projected[2] * scale_four,
    ];

    let (yaw_sine, yaw_cosine) = view.camera_yaw.sin_cos();
    let (pitch_sine, pitch_cosine) = view.camera_pitch.sin_cos();
    let yawed = [
        yaw_cosine.mul_add(world[0], yaw_sine * world[2]),
        world[1],
        (-yaw_sine).mul_add(world[0], yaw_cosine * world[2]),
    ];
    let view_point = [
        yawed[0],
        pitch_cosine.mul_add(yawed[1], -(pitch_sine * yawed[2])),
        pitch_sine.mul_add(yawed[1], pitch_cosine * yawed[2]) - view.distance_four,
    ];
    if -view_point[2] <= DENOMINATOR_EPSILON {
        return None;
    }
    let perspective_scale = aspect * view.distance_four * 0.5;
    let clip_w = -view_point[2];
    let ndc = [
        perspective_scale * view_point[0] / aspect / clip_w,
        perspective_scale * view_point[1] / clip_w,
    ];
    ndc.iter().all(|value| value.is_finite()).then_some(ndc)
}

/// Crossing-count test for a point against the traced boundary ring.
fn encloses(ring: &[[f64; 2]], point: [f64; 2]) -> bool {
    let mut inside = false;
    for (index, &a) in ring.iter().enumerate() {
        let b = ring[(index + 1) % ring.len()];
        if (a[1] > point[1]) != (b[1] > point[1]) {
            let crossing = (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0];
            if point[0] < crossing {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::{SceneFootprint, scene_footprint};
    use crate::{ObjectAngles, PlaneAngles, ViewControls};

    const OWNER_THETA: f64 = -core::f64::consts::FRAC_PI_2;

    /// The owner's relief row: the Julia plane at `o₁₃ = o₂₄ = −π/2` with the identity camera.
    fn owner_row(height_scale: f64) -> (ObjectAngles, ViewControls) {
        let object = ObjectAngles::from(PlaneAngles {
            theta_1: OWNER_THETA,
            theta_2: OWNER_THETA,
        });
        let view = ViewControls {
            height_scale,
            ..ViewControls::NEUTRAL
        };
        (object, view)
    }

    #[test]
    fn flat_chart_covers_its_own_frame() {
        let (object, view) = owner_row(0.0);
        let footprint = scene_footprint(&object, &view, 960, 540).expect("flat pose maps");
        assert_eq!(footprint.apron_scale.to_bits(), 1.0_f64.to_bits());
        assert_eq!(footprint.requested_apron_scale.to_bits(), 1.0_f64.to_bits());
        assert!((footprint.boundary_scale - 1.0).abs() < 1.0e-9);
        assert_eq!(footprint.uncovered_fraction, 0.0);
        assert_eq!(SceneFootprint::COVERED.uncovered_fraction, 0.0);
    }

    /// On the Julia plane the chart basis is `(e₁,e₂)`, so the fourth display coordinate is zero,
    /// the four-to-three divide is exactly one, and the lift is a pure radial scale by
    /// `d₅/(d₅ − h)`. The most contracting record height is `−2·height_scale`.
    #[test]
    fn owner_row_pull_is_the_closed_form_scale() {
        let (object, view) = owner_row(2.165);
        let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        let expected = view.distance_five / (view.distance_five + 2.0 * view.height_scale);
        assert!(
            (footprint.boundary_scale - expected).abs() < 1.0e-6,
            "boundary {} expected {expected}",
            footprint.boundary_scale
        );
        assert!((footprint.requested_apron_scale - 1.0 / expected).abs() < 1.0e-6);
        assert!((footprint.apron_scale - 1.0 / expected).abs() < 1.0e-6);
        assert_eq!(footprint.uncovered_fraction, 0.0);
    }

    /// Zoom never enters the scene map or the vertex chain, only the kernels' pixel scale, so the
    /// owner's two rows differ in content and not in footprint.
    #[test]
    fn footprint_is_zoom_invariant() {
        let (object, view) = owner_row(2.165);
        let aspect = 960.0 / 540.0;
        let flat = crate::screen_to_plane(&object, &view, 0.0, 960, 540, aspect).expect("zoom 0");
        let deep = crate::screen_to_plane(&object, &view, -1.001_417_717_032_54, 960, 540, aspect)
            .expect("zoom -1.0014");
        assert_eq!(flat.rows, deep.rows);
        let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        assert_eq!(footprint.uncovered_fraction, 0.0);
    }

    /// The apron bound is what the sampling extent must be multiplied by: scaling the traced frame
    /// by `apron_scale` puts the lifted boundary outside the frame it must cover.
    #[test]
    fn apron_scale_pushes_the_boundary_past_the_frame() {
        for height_scale in [0.5, 1.0, 2.165, 4.0] {
            let (object, view) = owner_row(height_scale);
            let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
            assert!(
                footprint.apron_scale >= 1.0,
                "height {height_scale} apron {}",
                footprint.apron_scale
            );
            assert!(
                (footprint.requested_apron_scale * footprint.boundary_scale - 1.0).abs() < 1.0e-9,
                "height {height_scale} apron and boundary must be reciprocal"
            );
        }
    }

    /// The slider maximum doubles the sampled extent, which is four times the records.
    #[test]
    fn maximum_height_needs_a_doubled_extent() {
        let (object, view) = owner_row(4.0);
        let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        assert!(
            (footprint.apron_scale - 2.0).abs() < 1.0e-6,
            "apron {}",
            footprint.apron_scale
        );
        assert!((footprint.requested_apron_scale - 2.0).abs() < 1.0e-6);
        assert_eq!(footprint.uncovered_fraction, 0.0);
    }

    #[test]
    fn an_apron_beyond_the_budget_is_clamped_and_its_shortfall_stays_visible() {
        let (object, view) = owner_row(8.0);
        let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        assert!((footprint.requested_apron_scale - 3.0).abs() < 1.0e-6);
        assert_eq!(footprint.apron_scale, super::MAX_APRON_SCALE);
        assert!(footprint.uncovered_fraction > 0.5);
    }
}
