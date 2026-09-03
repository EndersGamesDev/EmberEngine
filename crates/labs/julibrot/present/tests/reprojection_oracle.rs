use ember_julibrot_kernels::{EscapeParams, KernelSample, RefinementLevel, escape_shallow_point};
use ember_julibrot_math::{
    EscapeGridRecord, ObjectAngles, Pose, PoseMap, PrecisionMode, ViewControls, construct_plane,
    pixel_scale, screen_to_plane,
};
use ember_julibrot_present::{
    CLASSIC_PALETTE, PaletteId, SampleClass, SceneFrame, SubmissionKind, SubmissionMeasurement,
    WARP_MAX_ERROR_PX, Warp, WarpKind, WarpValidation, apply_homography, height_for_record,
    project_scene_point, project_scene_vertex, shade_lit_escape_record,
};

const EXTENT: [u32; 2] = [96, 54];
const BASE_ORIGIN: [f64; 4] = [0.0, 0.0, -0.75, 0.1];
const ESCAPE: EscapeParams = EscapeParams::new(128);
const LIGHT: f32 = 0.7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Expected {
    Agree,
    Clear,
    /// Redrawn because the measured corpus put the one image homography beyond its ceiling.
    ///
    /// The retained records still describe the destination, so the fixture additionally proves
    /// that redrawing them under the destination pose needs no new sampling.
    Relief,
    Either,
}

fn pose_at(
    extent: [u32; 2],
    object: ObjectAngles,
    view: ViewControls,
    origin: [f64; 4],
    zoom_log2: f64,
    displacement: [f64; 2],
) -> Pose {
    let plane = construct_plane(object).expect("fixture object constructs a plane");
    let map = screen_to_plane(
        &object,
        &view,
        zoom_log2,
        extent[0],
        extent[1],
        f64::from(extent[0]) / f64::from(extent[1]),
    )
    .map_or(PoseMap::EdgeOn, PoseMap::Mapped);
    Pose {
        epoch: 1,
        orbit_generation: 1,
        plane,
        object,
        plane_origin: origin,
        zoom_log2,
        view,
        grid_width: extent[0],
        grid_height: extent[1],
        map,
        centre_from_reference_px: displacement,
    }
}

fn pose(
    object: ObjectAngles,
    view: ViewControls,
    origin: [f64; 4],
    zoom_log2: f64,
    displacement: [f64; 2],
) -> Pose {
    pose_at(EXTENT, object, view, origin, zoom_log2, displacement)
}

const fn frame(pose: &Pose) -> SceneFrame {
    SceneFrame {
        scene_id: 7,
        pose: *pose,
        palette: PaletteId::Classic,
        iteration_cap: ESCAPE.max_iter,
        level: RefinementLevel::Final,
        extent: [pose.grid_width, pose.grid_height],
        texture_index: 1,
        centre_revision: 1,
        plane_origin_f64: pose.plane_origin,
        precision_mode: PrecisionMode::PictureFast.as_str(),
        measurement: SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id: 7,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            precision_mode: PrecisionMode::PictureFast.as_str(),
            wall_ms: 1.0,
            fence_wait_ms: 0.5,
            polls: 1,
        },
    }
}

fn rows(plan: [[f32; 4]; 3]) -> [f64; 9] {
    core::array::from_fn(|index| f64::from(plan[index / 3][index % 3]))
}

fn map_plane_offset(pose: &Pose, screen: [f64; 2]) -> Option<[f64; 2]> {
    let PoseMap::Mapped(map) = pose.map else {
        return None;
    };
    let denominator = map.rows[6].mul_add(screen[0], map.rows[7].mul_add(screen[1], map.rows[8]));
    if !denominator.is_finite() || denominator <= 0.0 {
        return None;
    }
    let mapped = [
        map.rows[0].mul_add(screen[0], map.rows[1].mul_add(screen[1], map.rows[2])) / denominator,
        map.rows[3].mul_add(screen[0], map.rows[4].mul_add(screen[1], map.rows[5])) / denominator,
    ];
    mapped
        .iter()
        .all(|value| value.is_finite())
        .then_some(mapped)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "the oracle intentionally rounds the same finite chart coordinates to kernel f32"
)]
fn sample_pose(pose: &Pose, screen: [f64; 2]) -> Option<KernelSample> {
    let Some(offset) = map_plane_offset(pose, screen) else {
        return Some(KernelSample {
            record: EscapeGridRecord {
                smooth_iter: -1.0,
                escaped: 0.0,
                rebase_count: 0.0,
                status: 2.0,
            },
            escape_index: None,
        });
    };
    let scale = pixel_scale(pose.zoom_log2, pose.grid_width).ok()?;
    let coordinate = [
        pose.centre_from_reference_px[0] + offset[0],
        pose.centre_from_reference_px[1] + offset[1],
    ];
    let point = core::array::from_fn(|axis| {
        scale.mul_add(
            f64::from(pose.plane.basis_u[axis]).mul_add(
                coordinate[0],
                f64::from(pose.plane.basis_v[axis]) * coordinate[1],
            ),
            pose.plane_origin[axis],
        ) as f32
    });
    escape_shallow_point(point, ESCAPE).ok()
}

fn pixel_screen(extent: [u32; 2], column: u32, row: u32) -> [f64; 2] {
    [
        0.5_f64.mul_add(-f64::from(extent[0]), f64::from(column) + 0.5),
        0.5_f64.mul_add(-f64::from(extent[1]), f64::from(row) + 0.5),
    ]
}

fn render_retained(pose: &Pose) -> Vec<KernelSample> {
    (0..pose.grid_height)
        .flat_map(|row| {
            (0..pose.grid_width).map(move |column| {
                sample_pose(
                    pose,
                    pixel_screen([pose.grid_width, pose.grid_height], column, row),
                )
                .expect("retained pixel samples")
            })
        })
        .collect()
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "bounds checks prove the rounded nearest-pixel indices fit the render extent"
)]
fn nearest_retained(
    image: &[KernelSample],
    extent: [u32; 2],
    source: [f64; 2],
) -> Option<(KernelSample, [f64; 2])> {
    let column = (f64::from(extent[0]).mul_add(0.5, source[0]) - 0.5).round();
    let row = (f64::from(extent[1]).mul_add(0.5, source[1]) - 0.5).round();
    if column < 0.0 || row < 0.0 || column >= f64::from(extent[0]) || row >= f64::from(extent[1]) {
        return None;
    }
    let index = row as usize * extent[0] as usize + column as usize;
    let sample = image.get(index).copied()?;
    Some((sample, pixel_screen(extent, column as u32, row as u32)))
}

const fn record(sample: KernelSample) -> [f32; 4] {
    [
        sample.record.smooth_iter,
        sample.record.escaped,
        sample.record.rebase_count,
        sample.record.status,
    ]
}

fn same_terminal_and_index(left: KernelSample, right: KernelSample) -> bool {
    left.record.escaped.to_bits() == right.record.escaped.to_bits()
        && left.record.status.to_bits() == right.record.status.to_bits()
        && left.escape_index == right.escape_index
}

fn colours_within_one_code(left: KernelSample, right: KernelSample) -> bool {
    let left = shade_lit_escape_record(record(left), CLASSIC_PALETTE, LIGHT).rgba;
    let right = shade_lit_escape_record(record(right), CLASSIC_PALETTE, LIGHT).rgba;
    left.into_iter()
        .zip(right)
        .all(|(left, right)| (left - right).abs() <= 1.0 / 255.0 + f32::EPSILON)
}

fn invert_homography(matrix: [f64; 9]) -> Option<[f64; 9]> {
    let determinant = matrix[2].mul_add(
        matrix[3].mul_add(matrix[7], -matrix[4] * matrix[6]),
        matrix[0].mul_add(
            matrix[4].mul_add(matrix[8], -matrix[5] * matrix[7]),
            -matrix[1] * matrix[3].mul_add(matrix[8], -matrix[5] * matrix[6]),
        ),
    );
    if !determinant.is_finite() || determinant.abs() <= 1.0e-12 {
        return None;
    }
    let inverse = [
        matrix[4].mul_add(matrix[8], -matrix[5] * matrix[7]) / determinant,
        matrix[2].mul_add(matrix[7], -matrix[1] * matrix[8]) / determinant,
        matrix[1].mul_add(matrix[5], -matrix[2] * matrix[4]) / determinant,
        matrix[5].mul_add(matrix[6], -matrix[3] * matrix[8]) / determinant,
        matrix[0].mul_add(matrix[8], -matrix[2] * matrix[6]) / determinant,
        matrix[2].mul_add(matrix[3], -matrix[0] * matrix[5]) / determinant,
        matrix[3].mul_add(matrix[7], -matrix[4] * matrix[6]) / determinant,
        matrix[1].mul_add(matrix[6], -matrix[0] * matrix[7]) / determinant,
        matrix[0].mul_add(matrix[4], -matrix[1] * matrix[3]) / determinant,
    ];
    inverse
        .iter()
        .all(|value| value.is_finite())
        .then_some(inverse)
}

fn stable_across_sampling_envelope(
    pose: &Pose,
    target: [f64; 2],
    sampled_target: [f64; 2],
    maximum: f64,
) -> Option<KernelSample> {
    let centre = sample_pose(pose, target)?;
    for step in 0..=4 {
        let fraction = f64::from(step) * 0.25;
        let point = [
            (sampled_target[0] - target[0]).mul_add(fraction, target[0]),
            (sampled_target[1] - target[1]).mul_add(fraction, target[1]),
        ];
        for offset in [
            [0.0, 0.0],
            [maximum, 0.0],
            [-maximum, 0.0],
            [0.0, maximum],
            [0.0, -maximum],
        ] {
            let nearby = sample_pose(pose, [point[0] + offset[0], point[1] + offset[1]])?;
            if !same_terminal_and_index(centre, nearby) || !colours_within_one_code(centre, nearby)
            {
                return None;
            }
        }
    }
    Some(centre)
}

fn oracle_points(extent: [u32; 2]) -> Vec<[f64; 2]> {
    let mut points = Vec::with_capacity(181);
    for row in 0..9 {
        for column in 0..13 {
            points.push([
                ((f64::from(column) + 0.37) / 13.0 - 0.5) * f64::from(extent[0]) * 0.8,
                ((f64::from(row) + 0.61) / 9.0 - 0.5) * f64::from(extent[1]) * 0.8,
            ]);
        }
    }
    let mut state = 0x7a11_ce55_u32;
    for _ in 0..64 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let x = f64::from(state) / f64::from(u32::MAX) - 0.5;
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let y = f64::from(state) / f64::from(u32::MAX) - 0.5;
        points.push([
            x * f64::from(extent[0]) * 0.8,
            y * f64::from(extent[1]) * 0.8,
        ]);
    }
    points
}

fn compare_accepted(
    name: &str,
    from: &Pose,
    to: &Pose,
    plan_rows: [[f32; 4]; 3],
    maximum: f64,
    height: f64,
) -> u32 {
    let inverse = rows(plan_rows);
    let forward = invert_homography(inverse).expect("accepted warp is invertible");
    let retained = render_retained(from);
    let mut compared = 0_u32;
    for target in oracle_points([to.grid_width, to.grid_height]) {
        let source = apply_homography(inverse, target)
            .unwrap_or_else(|| panic!("{name}: accepted map had an off-corpus pole"));
        let destination_relief = project_scene_point(to, target, height)
            .unwrap_or_else(|| panic!("{name}: destination had an off-corpus pole"));
        let expected_source = project_scene_point(from, source, height)
            .unwrap_or_else(|| panic!("{name}: retained scene had an off-corpus pole"));
        let warped_source = apply_homography(inverse, destination_relief)
            .unwrap_or_else(|| panic!("{name}: warp had an off-corpus pole"));
        let error =
            (warped_source[0] - expected_source[0]).hypot(warped_source[1] - expected_source[1]);
        assert!(error <= maximum + 1.0e-6, "{name}: {error} > {maximum}");

        let Some((retained, retained_pixel)) =
            nearest_retained(&retained, [from.grid_width, from.grid_height], source)
        else {
            continue;
        };
        let sampled_target = apply_homography(forward, retained_pixel)
            .unwrap_or_else(|| panic!("{name}: retained pixel mapped through a pole"));
        let Some(fresh) = stable_across_sampling_envelope(to, target, sampled_target, maximum)
        else {
            continue;
        };
        assert!(
            same_terminal_and_index(retained, fresh),
            "{name}: terminal/index mismatch"
        );
        assert!(
            colours_within_one_code(retained, fresh),
            "{name}: colour mismatch"
        );
        compared = compared.saturating_add(1);
    }
    assert!(
        compared >= 8,
        "{name}: only {compared} stable independent samples"
    );
    compared
}

/// Proves the retained escape records already describe the destination pose.
///
/// A relief redraw carries no new sampling: the oracle independently rasterizes both the retained
/// record mesh under the destination view and a freshly sampled destination mesh. It reproduces
/// the GPU's perspective-correct grid-coordinate interpolation rather than consulting the image
/// homography whose failure selected this path. Points where the two finite grids choose adjacent
/// records are counted as resampling uncertainty; every certified point keeps its terminal class,
/// escape index, and colour to one code.
#[derive(Clone, Copy)]
struct RedrawVertex {
    screen: [f64; 2],
    reciprocal_w: f64,
    grid: [f64; 2],
}

fn cross(left: [f64; 2], right: [f64; 2]) -> f64 {
    left[0].mul_add(right[1], -left[1] * right[0])
}

fn barycentric(point: [f64; 2], triangle: [RedrawVertex; 3]) -> Option<[f64; 3]> {
    let [a, b, c] = triangle.map(|vertex| vertex.screen);
    let denominator = cross([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
    if !denominator.is_finite() || denominator.abs() <= 1.0e-12 {
        return None;
    }
    let weights = [
        cross(
            [b[0] - point[0], b[1] - point[1]],
            [c[0] - point[0], c[1] - point[1]],
        ) / denominator,
        cross(
            [c[0] - point[0], c[1] - point[1]],
            [a[0] - point[0], a[1] - point[1]],
        ) / denominator,
        cross(
            [a[0] - point[0], a[1] - point[1]],
            [b[0] - point[0], b[1] - point[1]],
        ) / denominator,
    ];
    weights
        .iter()
        .all(|weight| *weight >= -1.0e-9)
        .then_some(weights)
}

fn redraw_vertices(from: &Pose, to: &Pose, retained: &[KernelSample]) -> Vec<Option<RedrawVertex>> {
    let mut redraw_pose = *to;
    redraw_pose.plane = from.plane;
    redraw_pose.object = from.object;
    redraw_pose.map = from.map;
    redraw_pose.grid_width = from.grid_width;
    redraw_pose.grid_height = from.grid_height;
    retained
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let index = u32::try_from(index).expect("fixture grid index fits u32");
            let column = index % from.grid_width;
            let row = index / from.grid_width;
            let height = height_for_record(record(*sample), ESCAPE.max_iter, CLASSIC_PALETTE)
                .expect("fixture record has a height")
                .height;
            project_scene_vertex(
                &redraw_pose,
                pixel_screen([from.grid_width, from.grid_height], column, row),
                f64::from(height),
            )
            .map(|(screen, clip_w)| RedrawVertex {
                screen,
                reciprocal_w: clip_w.recip(),
                grid: [f64::from(column), f64::from(row)],
            })
        })
        .collect()
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "bounds checks prove the rounded oracle grid coordinate fits the fixture"
)]
fn sample_redraw_mesh(
    target: [f64; 2],
    extent: [u32; 2],
    records: &[KernelSample],
    vertices: &[Option<RedrawVertex>],
) -> Option<KernelSample> {
    let width = usize::try_from(extent[0]).expect("fixture width fits usize");
    for row in 0..extent[1].saturating_sub(1) {
        for column in 0..extent[0].saturating_sub(1) {
            let a = usize::try_from(row * extent[0] + column).expect("fixture index fits usize");
            let b = a + 1;
            let c = a + width;
            let d = c + 1;
            for indices in [[a, b, c], [b, d, c]] {
                let [Some(a), Some(b), Some(c)] = indices.map(|index| vertices[index]) else {
                    continue;
                };
                let triangle = [a, b, c];
                let Some(weights) = barycentric(target, triangle) else {
                    continue;
                };
                let reciprocal_w = weights
                    .into_iter()
                    .zip(triangle)
                    .fold(0.0, |sum, (weight, vertex)| {
                        weight.mul_add(vertex.reciprocal_w, sum)
                    });
                if !reciprocal_w.is_finite() || reciprocal_w <= 0.0 {
                    continue;
                }
                let grid: [f64; 2] = core::array::from_fn(|axis| {
                    weights
                        .into_iter()
                        .zip(triangle)
                        .fold(0.0, |sum, (weight, vertex)| {
                            (weight * vertex.reciprocal_w).mul_add(vertex.grid[axis], sum)
                        })
                        / reciprocal_w
                });
                let sample_column = grid[0].round();
                let sample_row = grid[1].round();
                if sample_column < 0.0
                    || sample_row < 0.0
                    || sample_column >= f64::from(extent[0])
                    || sample_row >= f64::from(extent[1])
                {
                    continue;
                }
                let index = sample_row as usize * width + sample_column as usize;
                return records.get(index).copied();
            }
        }
    }
    None
}

fn compare_redraw(name: &str, from: &Pose, to: &Pose) -> (u32, u32, u32) {
    let retained = render_retained(from);
    let fresh = render_retained(to);
    let retained_vertices = redraw_vertices(from, to, &retained);
    let fresh_vertices = redraw_vertices(to, to, &fresh);
    let mut compared = 0_u32;
    let mut disoccluded = 0_u32;
    let mut uncertain = 0_u32;
    for target in oracle_points([to.grid_width, to.grid_height]) {
        let redrawn = sample_redraw_mesh(
            target,
            [from.grid_width, from.grid_height],
            &retained,
            &retained_vertices,
        );
        let freshly_drawn = sample_redraw_mesh(target, [to.grid_width, to.grid_height], &fresh, &fresh_vertices);
        let (Some(redrawn), Some(freshly_drawn)) = (redrawn, freshly_drawn) else {
            assert!(
                redrawn.is_none(),
                "{name}: redraw showed stale content where the fresh mesh is sky"
            );
            disoccluded = disoccluded.saturating_add(1);
            continue;
        };
        if same_terminal_and_index(redrawn, freshly_drawn)
            && colours_within_one_code(redrawn, freshly_drawn)
        {
            compared = compared.saturating_add(1);
        } else {
            uncertain = uncertain.saturating_add(1);
        }
    }
    assert!(
        compared >= 8,
        "{name}: only {compared} stable independent redraw samples"
    );
    (compared, disoccluded, uncertain)
}

#[allow(
    clippy::print_stderr,
    reason = "the oracle emits the requested per-fixture verdict table"
)]
fn assert_fixture(name: &str, from: &Pose, to: &Pose, height: f64, expected: Expected) {
    let plan = Warp::reproject(
        &frame(from),
        from,
        to,
        PrecisionMode::PictureFast,
        WarpValidation::Ordinary,
    );
    if plan.kind == WarpKind::ReliefRedraw {
        assert!(
            matches!(
                expected,
                Expected::Relief | Expected::Clear | Expected::Either
            ),
            "{name}: unexpectedly selected a relief redraw"
        );
        assert!(plan.source_valid, "{name}: relief redraw lost its record source");
        assert_eq!(plan.source_scene_id, Some(7), "{name}");
        assert_eq!(plan.source_texture_index, Some(1), "{name}");
        let maximum = plan
            .approx_max_error_px
            .expect("a relief redraw is measured, never unmeasurable");
        assert!(maximum > WARP_MAX_ERROR_PX, "{name}: {maximum}");
        let (compared, disoccluded, uncertain) = compare_redraw(name, from, to);
        eprintln!(
            "oracle fixture | {name} | relief redraw | samples={compared} | uncertain={uncertain} | disoccluded={disoccluded} | homography={maximum:.3} px"
        );
        return;
    }
    if plan.kind == WarpKind::ClearOnly {
        assert!(
            matches!(expected, Expected::Clear | Expected::Either),
            "{name}: unexpectedly cleared"
        );
        assert!(!plan.source_valid, "{name}: clear plan retained a source");
        eprintln!("oracle fixture | {name} | cleared");
        return;
    }
    assert!(
        matches!(expected, Expected::Agree | Expected::Either),
        "{name}: unexpectedly displayed"
    );
    assert!(plan.source_valid, "{name}");
    assert_eq!(plan.source_scene_id, Some(7), "{name}");
    assert_eq!(plan.source_texture_index, Some(1), "{name}");
    let maximum = plan
        .approx_max_error_px
        .expect("every displayable plan publishes its measured maximum");
    assert!(maximum <= WARP_MAX_ERROR_PX, "{name}: {maximum}");
    let compared = compare_accepted(name, from, to, plan.rows, maximum, height);
    eprintln!("oracle fixture | {name} | agree | samples={compared} | bound={maximum:.6}");
}

fn flat() -> Pose {
    pose(
        ObjectAngles::JULIA,
        ViewControls::NEUTRAL,
        BASE_ORIGIN,
        0.0,
        [0.0; 2],
    )
}

const fn relief() -> ViewControls {
    let mut camera = [0.0; 10];
    camera[8] = 0.25;
    ViewControls {
        camera,
        camera_yaw: 0.12,
        camera_pitch: -0.09,
        height_scale: 1.0,
        ..ViewControls::NEUTRAL
    }
}

fn object_angle(mut object: ObjectAngles, index: usize, delta: f64) -> ObjectAngles {
    match index {
        0 => object.rho_12 += delta,
        1 => object.rho_13 += delta,
        2 => object.rho_14 += delta,
        3 => object.rho_23 += delta,
        4 => object.rho_24 += delta,
        5 => object.rho_34 += delta,
        _ => panic!("object factor index {index}"),
    }
    object
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one independent oracle keeps every required reprojection degree of freedom visible"
)]
fn retained_warp_matches_independent_fresh_scenes() {
    let base = flat();

    let mut pan = base;
    pan.centre_from_reference_px = [0.25, -0.2];
    assert_fixture("pure pan", &base, &pan, 0.0, Expected::Agree);

    let mut zoom = base;
    zoom.zoom_log2 = 0.05;
    assert_fixture("pure zoom", &base, &zoom, 0.0, Expected::Agree);

    for index in 0..6 {
        let tiny = pose(
            object_angle(ObjectAngles::JULIA, index, 1.0e-9),
            ViewControls::NEUTRAL,
            BASE_ORIGIN,
            0.0,
            [0.0; 2],
        );
        assert_fixture(
            &format!("object {index} tiny"),
            &base,
            &tiny,
            0.0,
            Expected::Agree,
        );
        let changed = pose(
            object_angle(ObjectAngles::JULIA, index, 0.3),
            ViewControls::NEUTRAL,
            BASE_ORIGIN,
            0.0,
            [0.0; 2],
        );
        assert_fixture(
            &format!("object {index} changed"),
            &base,
            &changed,
            0.0,
            Expected::Clear,
        );
    }

    for index in 0..10 {
        let mut flat_view = ViewControls::NEUTRAL;
        flat_view.camera[index] = 0.2;
        let turned = pose(ObjectAngles::JULIA, flat_view, BASE_ORIGIN, 0.0, [0.0; 2]);
        assert_fixture(
            &format!("camera {index} h0"),
            &base,
            &turned,
            0.0,
            Expected::Agree,
        );

        let from = pose(ObjectAngles::JULIA, relief(), BASE_ORIGIN, 0.0, [0.0; 2]);
        let mut relief_view = relief();
        relief_view.camera[index] += 0.2;
        let to = pose(ObjectAngles::JULIA, relief_view, BASE_ORIGIN, 0.0, [0.0; 2]);
        assert_fixture(
            &format!("camera {index} h1"),
            &from,
            &to,
            1.0,
            Expected::Either,
        );
    }

    let mut observer = ViewControls::NEUTRAL;
    observer.camera_yaw = 0.2;
    observer.camera_pitch = -0.15;
    let observer = pose(ObjectAngles::JULIA, observer, BASE_ORIGIN, 0.0, [0.0; 2]);
    assert_fixture("yaw pitch h0", &base, &observer, 0.0, Expected::Agree);

    let relief_from = pose(ObjectAngles::JULIA, relief(), BASE_ORIGIN, 0.0, [0.0; 2]);
    let mut observer_relief = relief();
    observer_relief.camera_yaw += 0.2;
    observer_relief.camera_pitch -= 0.15;
    let observer_relief = pose(
        ObjectAngles::JULIA,
        observer_relief,
        BASE_ORIGIN,
        0.0,
        [0.0; 2],
    );
    assert_fixture(
        "yaw pitch h1",
        &relief_from,
        &observer_relief,
        1.0,
        Expected::Either,
    );

    for (name, field) in [("distance five", 5_u8), ("distance four", 4_u8)] {
        let mut flat_view = ViewControls::NEUTRAL;
        if field == 5 {
            flat_view.distance_five = 6.5;
        } else {
            flat_view.distance_four = 6.5;
        }
        let flat_to = pose(ObjectAngles::JULIA, flat_view, BASE_ORIGIN, 0.0, [0.0; 2]);
        assert_fixture(&format!("{name} h0"), &base, &flat_to, 0.0, Expected::Agree);
        let mut relief_view = relief();
        if field == 5 {
            relief_view.distance_five = 6.5;
        } else {
            relief_view.distance_four = 6.5;
        }
        let relief_to = pose(ObjectAngles::JULIA, relief_view, BASE_ORIGIN, 0.0, [0.0; 2]);
        assert_fixture(
            &format!("{name} h1"),
            &relief_from,
            &relief_to,
            1.0,
            Expected::Either,
        );
    }

    let mut height_view = relief();
    height_view.height_scale = 1.4;
    let height_to = pose(ObjectAngles::JULIA, height_view, BASE_ORIGIN, 0.0, [0.0; 2]);
    assert_fixture(
        "height scale",
        &relief_from,
        &height_to,
        1.0,
        Expected::Relief,
    );

    let mut translated_view = relief();
    translated_view.camera_translation = [1.0e-4, -2.0e-4, 1.5e-4, 0.0, 1.0e-4];
    let translated = pose(
        ObjectAngles::JULIA,
        translated_view,
        BASE_ORIGIN,
        0.0,
        [0.0; 2],
    );
    assert_fixture(
        "camera translation h1",
        &relief_from,
        &translated,
        1.0,
        Expected::Agree,
    );

    let in_plane = pose(
        ObjectAngles::JULIA,
        ViewControls::NEUTRAL,
        [0.01, -0.005, BASE_ORIGIN[2], BASE_ORIGIN[3]],
        0.0,
        [0.0; 2],
    );
    assert_fixture("in-plane origin", &base, &in_plane, 0.0, Expected::Agree);
    let out_of_plane = pose(
        ObjectAngles::JULIA,
        ViewControls::NEUTRAL,
        [0.0, 0.0, BASE_ORIGIN[2] + 0.03, BASE_ORIGIN[3]],
        0.0,
        [0.0; 2],
    );
    assert_fixture(
        "out-of-plane origin",
        &base,
        &out_of_plane,
        0.0,
        Expected::Clear,
    );

    let resized = pose_at(
        [80, 60],
        ObjectAngles::JULIA,
        ViewControls::NEUTRAL,
        BASE_ORIGIN,
        0.0,
        [0.0; 2],
    );
    assert_fixture("extent aspect", &base, &resized, 0.0, Expected::Agree);

    let mut edge_on = base;
    edge_on.map = PoseMap::EdgeOn;
    assert_fixture("edge on", &base, &edge_on, 0.0, Expected::Clear);

    let pole_view = ViewControls {
        height_scale: 1.0,
        distance_five: 1.0,
        ..ViewControls::NEUTRAL
    };
    // The pole must be carried to a pose that is not the one it was rendered at: a scene sampled
    // at its own pose is the identity and is displayed without ever building a corpus.
    let pole = pose(ObjectAngles::JULIA, pole_view, BASE_ORIGIN, 0.0, [0.0; 2]);
    let pole_moved = pose(
        ObjectAngles::JULIA,
        pole_view,
        BASE_ORIGIN,
        0.0,
        [6.0, -4.0],
    );
    assert_ne!(pole, pole_moved);
    assert_fixture(
        "pole inside frame",
        &pole,
        &pole_moved,
        1.0,
        Expected::Clear,
    );

    observer_bars();

    let mut cross_view = relief();
    cross_view.camera[0] += 0.08;
    cross_view.camera[6] -= 0.05;
    cross_view.camera_translation = [0.01, -0.005, 0.008, 0.0, 0.006];
    cross_view.camera_yaw += 0.04;
    cross_view.distance_four = 7.8;
    let cross = pose(
        ObjectAngles::JULIA,
        cross_view,
        [0.002, -0.001, BASE_ORIGIN[2], BASE_ORIGIN[3]],
        0.03,
        [0.2, -0.15],
    );
    assert_fixture("cross terms", &relief_from, &cross, 1.0, Expected::Either);
}

/// The Mandelbrot preset row's own plane origin, where the canonical flat pair is exact.
const PRESET_ORIGIN: [f64; 4] = [0.0; 4];

/// A camera factor turns the flat picture, and the warp turns it to the same place.
///
/// At height zero the scene pass draws every record at its own grid position and the picture is
/// decided entirely by which chart point each pixel samples — that is the screen map, and the
/// camera rotation is in it. So a camera factor off the preset row must *resample*: the same
/// screen pixel lands on a different point of the same slice, and the picture foreshortens.
///
/// This is worth pinning because the failure it rules out is easy to misread on the page. A
/// reprojected rectangle becomes a tilted quadrilateral with exposed corners, and when the next
/// scene fills those corners the frame boundary stops being tilted. That reads as "the picture
/// rotated and then snapped back upright" while the picture itself never moved: measured on the
/// deployed page at 960 by 540, q₁₂ = 0.8 puts the warp and the settled scene 0.063 px apart in
/// the interior centroid, against 49 px between the settled scene and the untouched preset row.
///
/// The identity picture belongs to the preset row alone. The canonical short-circuit keys on the
/// object and camera pair, never on the height, so it holds at q₁₂ = 0 and is gone at q₁₂ = 0.8.
#[test]
fn a_camera_factor_turns_the_flat_picture_off_the_preset_row() {
    let preset = pose(
        ObjectAngles::IDENTITY,
        ViewControls::MANDELBROT_FLAT,
        PRESET_ORIGIN,
        0.0,
        [0.0; 2],
    );
    let PoseMap::Mapped(preset_map) = preset.map else {
        panic!("the preset row is a mapped pose");
    };
    assert_eq!(
        preset_map.condition_number, 1.0,
        "the canonical flat pair is the exact identity map"
    );

    let mut turned_view = ViewControls::MANDELBROT_FLAT;
    turned_view.camera[0] = 0.8;
    let turned = pose(
        ObjectAngles::IDENTITY,
        turned_view,
        PRESET_ORIGIN,
        0.0,
        [0.0; 2],
    );
    let PoseMap::Mapped(turned_map) = turned.map else {
        panic!("a turned camera is still a mapped pose");
    };
    assert!(
        turned_map.condition_number > 1.5,
        "q12 = 0.8 leaves the canonical pair and anisotropy enters the map: {}",
        turned_map.condition_number
    );

    let points = oracle_points([turned.grid_width, turned.grid_height]);
    let mut resampled = 0_u32;
    let mut compared = 0_u32;
    for screen in &points {
        let Some(before) = sample_pose(&preset, *screen) else {
            continue;
        };
        let Some(after) = sample_pose(&turned, *screen) else {
            continue;
        };
        compared = compared.saturating_add(1);
        if !same_terminal_and_index(before, after) || !colours_within_one_code(before, after) {
            resampled = resampled.saturating_add(1);
        }
    }
    assert!(compared >= 64, "only {compared} comparable pixels");
    assert!(
        resampled * 4 >= compared,
        "a turned camera must resample the slice, not reproduce it: {resampled} of {compared}"
    );

    let plan = Warp::reproject(
        &frame(&preset),
        &preset,
        &turned,
        PrecisionMode::PictureFast,
        WarpValidation::Ordinary,
    );
    assert!(plan.source_valid, "the turned flat warp is accepted");
    assert!(plan.exposed, "the turned rectangle exposes surface corners");
    assert_eq!(plan.source_scene_id, Some(7));
    let inverse = rows(plan.rows);
    let retained = render_retained(&preset);
    let mut exposed = 0_u32;
    for row in 0..17 {
        for column in 0..17 {
            let target = [
                ((f64::from(column) + 0.5) / 17.0 - 0.5) * f64::from(turned.grid_width),
                ((f64::from(row) + 0.5) / 17.0 - 0.5) * f64::from(turned.grid_height),
            ];
            if map_plane_offset(&turned, target).is_none() {
                continue;
            }
            let Some(source) = apply_homography(inverse, target) else {
                continue;
            };
            if nearest_retained(&retained, [preset.grid_width, preset.grid_height], source)
                .is_none()
            {
                exposed = exposed.saturating_add(1);
            }
        }
    }
    assert!(
        exposed > 0,
        "the accepted source honestly leaves clear corners"
    );

    assert_fixture(
        "camera q12 off the preset row",
        &preset,
        &turned,
        0.0,
        Expected::Agree,
    );
}

/// One observer bar: its name, the control it moves, and its class at height zero and lifted.
type ObserverCase = (
    &'static str,
    fn(ViewControls) -> ViewControls,
    Expected,
    Expected,
);

/// One row for every observer bar, at height zero and again lifted.
///
/// This is the page's own manual-mode table written as fixtures: move one observer control from a
/// settled scene and record what the surface is allowed to do. Yaw, pitch and the four-space
/// distance are observer motions the one image homography carries exactly at any height. The
/// fifth-space distance and the height amplitude are not motions of the observer at all — they
/// change where a record of a given escape height sits — so each retained pixel moves by its own
/// amount and the homography is refused, honestly, in favour of a relief redraw.
///
/// At height zero the fifth-space distance and the four-space distance are inert by construction:
/// every record projects at its chart position, so the map is the identity and the retained image
/// is displayed unchanged.
fn observer_bars() {
    let flat_view = ViewControls::NEUTRAL;
    let lifted_view = ViewControls {
        height_scale: 1.0,
        ..ViewControls::NEUTRAL
    };
    let flat_from = pose(ObjectAngles::JULIA, flat_view, BASE_ORIGIN, 0.0, [0.0; 2]);
    let lifted_from = pose(ObjectAngles::JULIA, lifted_view, BASE_ORIGIN, 0.0, [0.0; 2]);

    let cases: [ObserverCase; 4] = [
        (
            "yaw",
            |mut view| {
                view.camera_yaw += 0.3;
                view
            },
            Expected::Agree,
            Expected::Agree,
        ),
        (
            "pitch",
            |mut view| {
                view.camera_pitch += 0.3;
                view
            },
            Expected::Agree,
            Expected::Agree,
        ),
        (
            "distance five",
            |mut view| {
                view.distance_five = 6.0;
                view
            },
            Expected::Agree,
            Expected::Relief,
        ),
        (
            "distance four",
            |mut view| {
                view.distance_four = 6.0;
                view
            },
            Expected::Agree,
            Expected::Agree,
        ),
    ];
    for (name, moved, at_zero, at_one) in cases {
        let flat_to = pose(
            ObjectAngles::JULIA,
            moved(flat_view),
            BASE_ORIGIN,
            0.0,
            [0.0; 2],
        );
        assert_fixture(
            &format!("observer {name} h0"),
            &flat_from,
            &flat_to,
            0.0,
            at_zero,
        );
        let lifted_to = pose(
            ObjectAngles::JULIA,
            moved(lifted_view),
            BASE_ORIGIN,
            0.0,
            [0.0; 2],
        );
        assert_fixture(
            &format!("observer {name} h1"),
            &lifted_from,
            &lifted_to,
            1.0,
            at_one,
        );
    }

    assert_fixture(
        "observer height 0 to 1",
        &flat_from,
        &lifted_from,
        1.0,
        Expected::Relief,
    );
    let half = pose(
        ObjectAngles::JULIA,
        ViewControls {
            height_scale: 0.5,
            ..ViewControls::NEUTRAL
        },
        BASE_ORIGIN,
        0.0,
        [0.0; 2],
    );
    assert_fixture(
        "observer height 1 to half",
        &lifted_from,
        &half,
        1.0,
        Expected::Relief,
    );
}
