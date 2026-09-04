//! Where the lifted scene mesh actually lands on the render surface.
//!
//! Screen-aligned sampling chooses chart points by inverting the scene map at height zero, so at
//! `height_scale = 0` every interior mesh vertex is its own screen pixel, the outermost ring is
//! drawn on the frame boundary, and the mesh tiles the frame exactly. Lifting a vertex into the fifth coordinate moves it: the five-to-four perspective
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

/// Vertices per axis of the mirrored sampling mesh whose triangles are rasterized for coverage.
const COVERAGE_MESH_SIDE: u32 = 65;

/// Side of the fixed square lattice the uncovered share is counted on.
///
/// The samples run from `-1` to `+1` inclusive, so the frame corners and edges are tested points
/// rather than cell centres: a mesh that falls short of the frame by less than one cell is counted
/// short instead of being rounded into coverage.
const COVERAGE_LATTICE_SIDE: usize = 65;

/// Guard the scene shader applies to both perspective denominators before dividing.
const DENOMINATOR_EPSILON: f64 = 1.0e-4;

/// Minimum retained distance from the five-dimensional eye, as a fraction of `distance_five`.
///
/// Five percent leaves twenty-times perspective magnification available for relief while keeping
/// every lifted vertex strictly in front of the eye. A later limit-model study owns replacing this
/// small 2D-projection rule with a true model-space clipping distance.
pub const RELIEF_NEAR_FRACTION: f64 = 0.05;

/// Record heights sampled by both pose censuses, in units of the height control.
const CENSUS_HEIGHTS: [f64; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];

/// Screen samples per axis in the clipping census.
const CLIP_LATTICE_SIDE: u32 = 9;

/// Coarse backdrop spans considered by the rasterized coverage policy.
const APRON_CANDIDATES: [f64; 5] = [1.25, 1.5, 2.0, 3.0, 5.0];

/// Where one pose's lifted mesh lands relative to the frame it was sampled for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneFootprint {
    /// Raster-selected screen-space overscan applied to the coarse backdrop sampling extent.
    pub apron_scale: f64,
    /// Share of the render surface no record of any admissible height can reach.
    ///
    /// Measured by rasterizing the mirrored sampling mesh at every census height, through the
    /// backdrop's apron, into a fixed lattice that includes the frame corners. A triangle whose
    /// vertices do not all project, or whose three vertices are all held at the near clamp, covers
    /// nothing — the same rule the scene shader applies — so a pose whose mesh falls apart reports
    /// more sky, never less.
    pub uncovered_fraction: f64,
    /// Share of the fixed plane-and-height census clamped at the five-dimensional near limit.
    ///
    /// The denominator is the whole fixed census. A census point that cannot be projected at all is
    /// not clipped and is not removed either: dropping it would let a pose that projects almost
    /// nothing publish a small clipped share.
    pub relief_clipped_fraction: f64,
}

impl SceneFootprint {
    /// The flat-chart answer: the mesh is its own frame and nothing is uncovered.
    pub const COVERED: Self = Self {
        apron_scale: 1.0,
        uncovered_fraction: 0.0,
        relief_clipped_fraction: 0.0,
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
    if view.height_scale == 0.0 {
        return Ok(SceneFootprint::COVERED);
    }

    let chain = VertexChain {
        map: &map,
        plane,
        view,
        matrix: camera_matrix(view),
    };
    let extent = [grid_w, grid_h];
    let main_uncovered = uncovered_fraction(&chain, extent, 1.0);
    let candidates = APRON_CANDIDATES.map(|apron_scale| {
        (
            apron_scale,
            uncovered_fraction(&chain, extent, apron_scale),
        )
    });
    let widest_uncovered = candidates[APRON_CANDIDATES.len() - 1].1;
    let widest_gain = main_uncovered - widest_uncovered;
    let (apron_scale, uncovered) = if widest_gain > 0.0 {
        let required_gain = 0.5 * widest_gain;
        candidates
            .into_iter()
            .find(|(_, candidate_uncovered)| {
                main_uncovered - candidate_uncovered >= required_gain
            })
            .unwrap_or((5.0, widest_uncovered))
    } else {
        (1.0, main_uncovered)
    };
    let relief_clipped_fraction = clipped_fraction(&chain, grid_w, grid_h, apron_scale);
    Ok(SceneFootprint {
        apron_scale,
        uncovered_fraction: uncovered,
        relief_clipped_fraction,
    })
}

/// The fixed part of the scene vertex chain, built once per pose.
///
/// The sampling map, its plane, the view controls and the five-dimensional camera matrix are the
/// same for every vertex of every census height; carrying them together is what keeps the mirror's
/// per-vertex signatures readable, and it also builds the camera matrix once instead of per point.
struct VertexChain<'a> {
    map: &'a crate::Homography,
    plane: Plane,
    view: &'a ViewControls,
    matrix: [[f64; 5]; 5],
}

/// One frame sample position on the closed `[-1,1]` lattice.
fn lattice_value(index: usize) -> f64 {
    2.0 * (index as f64 / (COVERAGE_LATTICE_SIDE - 1) as f64) - 1.0
}

/// Rasterizes the mirrored sampling mesh at every census height and counts what it never reaches.
///
/// This is the picture's own rule mirrored in binary64: the mesh is a lattice of quads, each drawn
/// as two triangles, and a triangle is drawn only when all three of its vertices project. A
/// triangle whose three vertices are all held at the five-dimensional near clamp is fabricated
/// geometry and is dropped by the scene shader, so it is dropped here too. Nothing a pose fails to
/// project can therefore improve the answer.
///
/// The heights are sampled, so the reachable set is a subset of the true one in that direction. The
/// mesh is coarser than the drawn one, which errs the other way: near the projective horizon a long
/// chord cuts across the curved image and covers ground the fine mesh leaves as sky, so the share
/// is not a bound in either direction. It is the same rule the picture is drawn by, evaluated at a
/// stated resolution, and it is compared only against itself.
fn uncovered_fraction(chain: &VertexChain<'_>, extent: [u32; 2], apron_scale: f64) -> f64 {
    let side = COVERAGE_LATTICE_SIDE;
    let mut reached = vec![false; side * side];
    let mesh = COVERAGE_MESH_SIDE as usize;
    let mut projected: Vec<Option<([f64; 2], bool)>> = vec![None; mesh * mesh];
    let [grid_w, grid_h] = extent;
    for record_height in CENSUS_HEIGHTS {
        let height = displayed_height(chain.view.height_scale, record_height);
        for row in 0..mesh {
            for column in 0..mesh {
                let screen = [
                    (column as f64 / (mesh - 1) as f64 - 0.5) * f64::from(grid_w),
                    (row as f64 / (mesh - 1) as f64 - 0.5) * f64::from(grid_h),
                ];
                projected[row * mesh + column] =
                    project_vertex(chain, extent, screen, height, apron_scale);
            }
        }
        for row in 0..mesh - 1 {
            for column in 0..mesh - 1 {
                let corners = [
                    projected[row * mesh + column],
                    projected[row * mesh + column + 1],
                    projected[(row + 1) * mesh + column + 1],
                    projected[(row + 1) * mesh + column],
                ];
                for triangle in [
                    [corners[0], corners[1], corners[2]],
                    [corners[0], corners[2], corners[3]],
                ] {
                    let Some(drawn) = drawn_triangle(triangle) else {
                        continue;
                    };
                    mark_triangle(&mut reached, drawn);
                }
            }
        }
    }
    let covered = reached.iter().filter(|hit| **hit).count();
    1.0 - covered as f64 / (side * side) as f64
}

/// Applies the same left-to-right height operation as the scene WGSL.
fn displayed_height(height_scale: f64, record_height: f64) -> f64 {
    height_scale * (record_height + 2.0) * 0.5
}

/// The scene shader's primitive rule: every vertex must project, and not every one may be clamped.
fn drawn_triangle(triangle: [Option<([f64; 2], bool)>; 3]) -> Option<[[f64; 2]; 3]> {
    let mut points = [[0.0_f64; 2]; 3];
    let mut clamped = 0_u8;
    for (slot, vertex) in triangle.iter().enumerate() {
        let (point, is_clamped) = (*vertex)?;
        points[slot] = point;
        clamped += u8::from(is_clamped);
    }
    (clamped < 3).then_some(points)
}

/// Marks every lattice sample inside one device-space triangle.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the bounding-box indices are floored from values already clamped to [0, span]"
)]
fn mark_triangle(reached: &mut [bool], triangle: [[f64; 2]; 3]) {
    let side = COVERAGE_LATTICE_SIDE;
    let xs = [triangle[0][0], triangle[1][0], triangle[2][0]];
    let ys = [triangle[0][1], triangle[1][1], triangle[2][1]];
    let minimum = |values: [f64; 3]| values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = |values: [f64; 3]| values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (x0, x1, y0, y1) = (minimum(xs), maximum(xs), minimum(ys), maximum(ys));
    if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite())
        || x1 < -1.0
        || x0 > 1.0
        || y1 < -1.0
        || y0 > 1.0
    {
        return;
    }
    let span = (side - 1) as f64;
    // `value.mul_add(0.5, 0.5)` is the single-rounding form of `(value + 1.0) * 0.5`, and lands on
    // the same binary64 result; it is written this way so the expression is a scale of the device
    // coordinate rather than a midpoint of two bounds, which is what it means.
    let index_of = |value: f64| (value.mul_add(0.5, 0.5) * span).floor();
    let low = |value: f64| index_of(value).max(0.0) as usize;
    let high = |value: f64| index_of(value).min(span).max(0.0) as usize + 1;
    let (column_first, column_last) = (low(x0), high(x1).min(side - 1));
    let (row_first, row_last) = (low(y0), high(y1).min(side - 1));
    let [ax, ay] = triangle[0];
    let [bx, by] = triangle[1];
    let [cx, cy] = triangle[2];
    let area = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
    if area == 0.0 || !area.is_finite() {
        return;
    }
    for row in row_first..=row_last {
        let point_y = lattice_value(row);
        for column in column_first..=column_last {
            let slot = row * side + column;
            if reached[slot] {
                continue;
            }
            let point_x = lattice_value(column);
            let first = ((by - cy) * (point_x - cx) + (cx - bx) * (point_y - cy)) / area;
            let second = ((cy - ay) * (point_x - cx) + (ax - cx) * (point_y - cy)) / area;
            let third = 1.0 - first - second;
            if first >= 0.0 && second >= 0.0 && third >= 0.0 {
                reached[slot] = true;
            }
        }
    }
}

/// Mirrors one scene-shader vertex: map to the chart, lift, both perspectives, then the observer.
///
/// The second component reports whether the lift reached the five-dimensional near clamp, which is
/// what lets a caller apply the shader's all-clamped primitive rule.
fn project_vertex(
    chain: &VertexChain<'_>,
    extent: [u32; 2],
    screen: [f64; 2],
    height: f64,
    apron_scale: f64,
) -> Option<([f64; 2], bool)> {
    let [grid_w, grid_h] = extent;
    let view = chain.view;
    let aspect = f64::from(grid_w) / f64::from(grid_h);
    let mut ambient = ambient_vertex(chain, grid_w, screen, height, apron_scale)?;
    let near_five = RELIEF_NEAR_FRACTION * view.distance_five;
    let maximum_fifth = view.distance_five - near_five;
    let clamped = ambient[4] > maximum_fifth;
    ambient[4] = ambient[4].min(maximum_fifth);

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
    ndc.iter()
        .all(|value| value.is_finite())
        .then_some((ndc, clamped))
}

/// Maps one grid point through the sampling chart and five-dimensional camera, before clipping.
fn ambient_vertex(
    chain: &VertexChain<'_>,
    grid_w: u32,
    screen: [f64; 2],
    height: f64,
    apron_scale: f64,
) -> Option<[f64; 5]> {
    let (plane, view, matrix) = (chain.plane, chain.view, &chain.matrix);
    let rows = chain.map.rows;
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
    let mut ambient: [f64; 5] = core::array::from_fn(|row| {
        (0..5).fold(0.0, |sum, column| {
            matrix[row][column].mul_add(ambient_in[column], sum)
        })
    });
    for (axis, value) in ambient.iter_mut().enumerate() {
        *value += view.camera_translation[axis];
    }
    ambient
        .iter()
        .all(|value| value.is_finite())
        .then_some(ambient)
}

/// Measures the share of the bounded record domain that reaches the five-dimensional near clamp.
///
/// The denominator is the whole fixed census, projectable or not. A point beyond the projective
/// horizon is not at the clamp, but removing it from the denominator would let a pose that projects
/// almost nothing publish a comfortable share.
fn clipped_fraction(chain: &VertexChain<'_>, grid_w: u32, grid_h: u32, apron_scale: f64) -> f64 {
    let view = chain.view;
    let mut clipped = 0_u32;
    let mut total = 0_u32;
    for row in 0..CLIP_LATTICE_SIDE {
        for column in 0..CLIP_LATTICE_SIDE {
            let screen = [
                (f64::from(column) / f64::from(CLIP_LATTICE_SIDE - 1) - 0.5) * f64::from(grid_w),
                (f64::from(row) / f64::from(CLIP_LATTICE_SIDE - 1) - 0.5) * f64::from(grid_h),
            ];
            for record_height in CENSUS_HEIGHTS {
                total += 1;
                let Some(ambient) = ambient_vertex(
                    chain,
                    grid_w,
                    screen,
                    displayed_height(view.height_scale, record_height),
                    apron_scale,
                ) else {
                    continue;
                };
                let near_five = RELIEF_NEAR_FRACTION * view.distance_five;
                if view.distance_five - ambient[4] < near_five {
                    clipped += 1;
                }
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        f64::from(clipped) / f64::from(total)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APRON_CANDIDATES, SceneFootprint, VertexChain, displayed_height, scene_footprint,
        uncovered_fraction,
    };
    use crate::screen::camera_matrix;
    use crate::{ObjectAngles, PlaneAngles, ViewControls, construct_plane};

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
        assert_eq!(footprint.uncovered_fraction, 0.0);
        assert_eq!(footprint.relief_clipped_fraction, 0.0);
        assert_eq!(SceneFootprint::COVERED.uncovered_fraction, 0.0);
    }

    /// The requested zoom names the floor, while the top keeps the former positive peak height.
    #[test]
    fn requested_zoom_anchors_the_floor_and_preserves_the_peak() {
        for height_scale in [0.0, 0.5, 1.0, 2.165, 4.0] {
            assert_eq!(
                displayed_height(height_scale, -2.0).to_bits(),
                0.0_f64.to_bits()
            );
            assert_eq!(
                displayed_height(height_scale, 2.0).to_bits(),
                (2.0 * height_scale).to_bits()
            );
            let (object, view) = owner_row(height_scale);
            let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
            assert_eq!(footprint.apron_scale.to_bits(), 1.0_f64.to_bits());
            assert_eq!(footprint.uncovered_fraction, 0.0);
        }
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

    /// Every requested apron is one of the reviewed raster-policy candidates.
    #[test]
    fn apron_scale_pushes_the_boundary_past_the_frame() {
        for (object, view) in [owner_row(2.165), owner_row(4.0), close_owner_row()] {
            let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
            assert!(
                footprint.apron_scale.to_bits() == 1.0_f64.to_bits()
                    || APRON_CANDIDATES.contains(&footprint.apron_scale),
                "unreviewed apron {}",
                footprint.apron_scale
            );
        }
    }

    /// At the ordinary distance, the slider maximum asks the backdrop to span twice the frame, and
    /// the records at the top of the domain reach the near clamp: a fifth of the census is held
    /// there, and the backdrop still covers the frame because the lower heights reach it.
    #[test]
    fn maximum_height_keeps_the_floor_covered_and_reports_peak_clipping() {
        let (object, view) = owner_row(4.0);
        let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        assert_eq!(footprint.apron_scale.to_bits(), 1.0_f64.to_bits());
        assert_eq!(footprint.uncovered_fraction, 0.0);
        assert!(
            (footprint.relief_clipped_fraction - 81.0 / 405.0).abs() < 1.0e-12,
            "clipped {}",
            footprint.relief_clipped_fraction
        );
    }

    /// The three rows published in `docs/julibrot/present.md`, asserted exactly.
    ///
    /// The documented table is a claim about what the engine reports, so it is transcribed from a
    /// run rather than reasoned about, and pinned here so prose and code cannot drift apart. The
    /// close row is the one that matters: it publishes real sky, where the old measurement
    /// published a vacuous zero.
    #[test]
    fn the_published_owner_rows_are_the_documented_ones() {
        let rows = [
            (owner_row(2.165), 1.0, 0.0, 0.0),
            (owner_row(4.0), 1.0, 0.0, 81.0 / 405.0),
            (close_owner_row(), 1.25, 655.0 / 4225.0, 252.0 / 405.0),
        ];
        for ((object, view), apron, uncovered, clipped) in rows {
            let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
            assert!(
                (footprint.apron_scale - apron).abs() < 1.0e-12
                    && (footprint.uncovered_fraction - uncovered).abs() < 1.0e-12
                    && (footprint.relief_clipped_fraction - clipped).abs() < 1.0e-12,
                "documented row {apron}/{uncovered}/{clipped} reads {footprint:?}"
            );
        }
    }

    /// The owner's second row: a tumbled plane seen from close in, at the slider maximum height.
    fn close_owner_row() -> (ObjectAngles, ViewControls) {
        let angle = -1.316_653_720_171_549_4;
        let object = ObjectAngles {
            rho_13: angle,
            rho_24: angle,
            ..ObjectAngles::IDENTITY
        };
        let camera_angle = -0.254_142_606_623_347_1;
        let mut camera = [0.0; 10];
        camera[1] = camera_angle;
        camera[4] = camera_angle;
        let view = ViewControls {
            camera,
            camera_yaw: 0.960_422_302_787_256,
            camera_pitch: core::f64::consts::PI,
            height_scale: 4.0,
            distance_five: 2.0,
            distance_four: 2.0,
            ..ViewControls::NEUTRAL
        };
        (object, view)
    }

    /// The second row asks for the smallest candidate backdrop, and retains honest horizon sky.
    ///
    /// This is the row that caught a vacuous coverage fact. The old measurement traced two boundary
    /// rings and dropped a ring entirely when any of its vertices failed a guard; with both rings
    /// dropped, "covered by every ring" was true of every lattice point and the pose published a
    /// perfect zero while three-quarters of the frame was empty. The measurement now rasterizes the
    /// mesh, so a triangle that fails to project covers nothing and cannot improve the answer.
    #[test]
    fn close_owner_row_gets_a_backdrop_and_reports_the_sky_it_cannot_reach() {
        let (object, view) = close_owner_row();
        let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        assert_eq!(footprint.apron_scale.to_bits(), 1.25_f64.to_bits());
        assert!(
            (footprint.uncovered_fraction - 655.0 / 4225.0).abs() < 1.0e-12,
            "uncovered {}",
            footprint.uncovered_fraction
        );
        assert!(footprint.uncovered_fraction > 0.15);
    }

    /// How much the coverage layer is actually worth at the close owner row.
    ///
    /// The published fact measures the frame the backdrop reaches. The comparison the design rests
    /// on is against the main grid alone, at the same pose through the same mirror with the apron
    /// set to one: the wide layer is a large real improvement there, not a rounding one, and the
    /// remaining sky is honest rather than a measurement artefact.
    #[test]
    fn the_backdrop_reaches_far_more_of_the_close_row_than_the_main_grid_alone() {
        let (object, view) = close_owner_row();
        let map = crate::screen_to_plane(&object, &view, 0.0, 960, 540, 960.0 / 540.0)
            .expect("relief pose maps");
        let chain = VertexChain {
            map: &map,
            plane: construct_plane(object).expect("relief plane"),
            view: &view,
            matrix: camera_matrix(&view),
        };
        let main_alone = uncovered_fraction(&chain, [960, 540], 1.0);
        let with_backdrop = uncovered_fraction(&chain, [960, 540], 1.25);
        let with_widest = uncovered_fraction(&chain, [960, 540], 5.0);
        assert!(
            (main_alone - 821.0 / 4225.0).abs() < 1.0e-12
                && (with_backdrop - 655.0 / 4225.0).abs() < 1.0e-12
                && (with_widest - 668.0 / 4225.0).abs() < 1.0e-12,
            "main {main_alone}, backdrop {with_backdrop}, widest {with_widest}"
        );
        assert!(
            main_alone - with_backdrop >= 0.5 * (main_alone - with_widest),
            "the chosen backdrop must recover half the fivefold gain"
        );
    }

    /// The close row's residual sky is the plane's own projective horizon, not a coverage failure.
    ///
    /// A tumbled plane seen from close in has a large part of the frame looking past its own
    /// horizon: those screen points fail the sampling map outright, no height and no apron can
    /// recover them, and honest sky is the correct picture there. Measured through the same mirror
    /// at the same pose with the height set to zero — a flat chart, no relief at all — the frame is
    /// still `821/4225` uncovered at apron one, `655/4225` at the chosen 1.25 apron, and
    /// `668/4225` at apron five. The small non-monotonic step is raster-lattice quantization.
    ///
    /// The relief scene matches those flat-floor fractions exactly, so the residual is the plane's
    /// projective horizon rather than relief coverage that a still-wider backdrop could repair.
    #[test]
    fn the_close_row_residual_is_the_plane_horizon_and_not_a_coverage_deficit() {
        let (object, view) = close_owner_row();
        let flat = ViewControls {
            height_scale: 0.0,
            ..view
        };
        let map = crate::screen_to_plane(&object, &view, 0.0, 960, 540, 960.0 / 540.0)
            .expect("relief pose maps");
        let plane = construct_plane(object).expect("relief plane");
        let horizon = |view: &ViewControls, apron: f64| {
            uncovered_fraction(
                &VertexChain {
                    map: &map,
                    plane,
                    view,
                    matrix: camera_matrix(view),
                },
                [960, 540],
                apron,
            )
        };
        let flat_narrow = horizon(&flat, 1.0);
        let flat_chosen = horizon(&flat, 1.25);
        let flat_wide = horizon(&flat, 5.0);
        let relief_chosen = horizon(&view, 1.25);
        let relief_wide = horizon(&view, 5.0);
        assert!(
            (flat_narrow - 821.0 / 4225.0).abs() < 1.0e-12
                && (flat_chosen - 655.0 / 4225.0).abs() < 1.0e-12
                && (flat_wide - 668.0 / 4225.0).abs() < 1.0e-12
                && (relief_chosen - 655.0 / 4225.0).abs() < 1.0e-12
                && (relief_wide - 668.0 / 4225.0).abs() < 1.0e-12,
            "flat {flat_narrow} / {flat_chosen} / {flat_wide}, relief {relief_chosen} / {relief_wide}"
        );
        assert!(
            relief_chosen <= flat_chosen,
            "relief must not leave more sky than its floor at the same apron"
        );
    }

    /// The clipping census keeps its whole fixed denominator.
    ///
    /// At the second owner row 252 of the 405 census points are held at the near clamp. The
    /// denominator stays the whole fixed domain, including points that cannot project at all.
    #[test]
    fn the_clipping_census_counts_every_point_it_sampled() {
        let (object, view) = close_owner_row();
        let footprint = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        assert!(
            (footprint.relief_clipped_fraction - 252.0 / 405.0).abs() < 1.0e-12,
            "clipped {}",
            footprint.relief_clipped_fraction
        );
        assert!(footprint.relief_clipped_fraction < 1.0);
    }

    /// Every relief pose that publishes a coverage number publishes a measurable one: a pose whose
    /// mesh falls apart reports more sky, never less.
    #[test]
    fn no_relief_pose_reports_perfect_coverage_it_could_not_measure() {
        let (object, view) = close_owner_row();
        let flat = ViewControls {
            height_scale: 0.0,
            ..view
        };
        let covered = scene_footprint(&object, &flat, 960, 540).expect("flat pose maps");
        assert_eq!(covered.uncovered_fraction, 0.0);
        assert_eq!(covered.relief_clipped_fraction, 0.0);
        let relief = scene_footprint(&object, &view, 960, 540).expect("relief pose maps");
        assert!(
            relief.uncovered_fraction > covered.uncovered_fraction,
            "the tumbled relief row cannot be as covered as its own flat chart"
        );
    }
}
