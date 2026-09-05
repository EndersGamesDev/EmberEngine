#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "a raster oracle narrows binary64 device coordinates to pixel indices throughout"
)]
#![allow(
    clippy::suboptimal_flops,
    reason = "the mirror keeps the shader's own operation order; a fused multiply-add is a different rounding"
)]
#![allow(
    clippy::float_cmp,
    reason = "record status and the escape sentinels are exact lane values, not measured quantities"
)]
#![allow(
    clippy::neg_cmp_op_on_partial_ord,
    reason = "the depth test mirrors LessEqual, which a non-comparable depth fails"
)]
#![allow(
    clippy::too_many_lines,
    clippy::option_if_let_else,
    clippy::missing_const_for_fn,
    clippy::print_stdout,
    reason = "one rasterizer written as one pass, reporting its measurements"
)]
//! A native render of this row's scene pass, measured against the browser's own readback.
//!
//! The lane that produced this file predicted a browser outcome from a census of refused SAMPLES
//! and was wrong, because a refused sample was already painted the exterior colour the pass clears
//! to. The instrument that was missing is this one: the records, the mesh, the depth buffer and
//! the fragment shading, run to a frame that the browser's own column-transition and
//! background-fraction statistics can be computed on. A prediction about the picture has to come
//! from a picture.

use ember_julibrot_kernels::{EscapeParams, escape_shallow_point};
use ember_julibrot_math::{
    ObjectAngles, Pose, PoseMap, ViewControls, construct_plane, pixel_scale, screen_to_plane,
};
use ember_julibrot_present::{CLASSIC_PALETTE, exterior_zero, grid_screen, shade_escape_record};

const EXTENT: [u32; 2] = [960, 540];
const CAP: u32 = 512;
const ESCAPE: EscapeParams = EscapeParams::new(CAP);
const OBJECT_ANGLE: f64 = -0.163_226_878_883_618_53;
const PLANE_ORIGIN: [f64; 4] = [-0.629, 0.0, -0.083, 0.016];
const CAMERA: [f64; 10] = [
    0.387_171_329_215_743,
    0.945_997_639_356_507,
    -1.185_339_915_598_97,
    -0.756_473_212_467_682,
    -0.236_634_784_429_762,
    -1.770_158_147_141_63,
    2.011_666_416_834_24,
    0.504_134_975_524_275,
    -0.665_501_487_561_046,
    0.374_175_368_514_795,
];

fn steep_view(height_scale: f64) -> ViewControls {
    ViewControls {
        camera: CAMERA,
        camera_translation: [-0.04, 0.258, 0.0, 0.0, 0.0],
        camera_yaw: 0.0,
        camera_pitch: 0.0,
        height_scale,
        distance_five: 8.0,
        distance_four: 8.0,
    }
}

fn pose_with(view: ViewControls) -> Pose {
    let object = ObjectAngles {
        rho_13: OBJECT_ANGLE,
        rho_24: OBJECT_ANGLE,
        ..ObjectAngles::IDENTITY
    };
    let plane = construct_plane(object).expect("plane");
    let map = screen_to_plane(
        &object,
        &view,
        0.0,
        EXTENT[0],
        EXTENT[1],
        f64::from(EXTENT[0]) / f64::from(EXTENT[1]),
    )
    .map_or(PoseMap::EdgeOn, PoseMap::Mapped);
    Pose {
        epoch: 1,
        orbit_generation: 1,
        plane,
        object,
        plane_origin: PLANE_ORIGIN,
        zoom_log2: 0.0,
        view,
        grid_width: EXTENT[0],
        grid_height: EXTENT[1],
        map,
        centre_from_reference_px: [0.0, 0.0],
    }
}

fn pixel_screen(column: u32, row: u32) -> [f64; 2] {
    [
        f64::from(column) + 0.5 - 0.5 * f64::from(EXTENT[0]),
        f64::from(row) + 0.5 - 0.5 * f64::from(EXTENT[1]),
    ]
}

fn draw_screen(column: u32, row: u32) -> [f64; 2] {
    [grid_screen(column, EXTENT[0]), grid_screen(row, EXTENT[1])]
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
    mapped.iter().all(|v| v.is_finite()).then_some(mapped)
}

fn sample_pose(pose: &Pose, screen: [f64; 2]) -> [f32; 4] {
    let Some(offset) = map_plane_offset(pose, screen) else {
        return [-1.0, 0.0, 0.0, 2.0];
    };
    let Ok(scale) = pixel_scale(pose.zoom_log2, pose.grid_width) else {
        return [-1.0, 0.0, 0.0, 3.0];
    };
    let coordinate = [
        pose.centre_from_reference_px[0] + offset[0],
        pose.centre_from_reference_px[1] + offset[1],
    ];
    let point: [f32; 4] = core::array::from_fn(|axis| {
        scale.mul_add(
            f64::from(pose.plane.basis_u[axis]).mul_add(
                coordinate[0],
                f64::from(pose.plane.basis_v[axis]) * coordinate[1],
            ),
            pose.plane_origin[axis],
        ) as f32
    });
    match escape_shallow_point(point, ESCAPE) {
        Ok(sample) => [
            sample.record.smooth_iter,
            sample.record.escaped,
            sample.record.rebase_count,
            sample.record.status,
        ],
        Err(_) => [-1.0, 0.0, 0.0, 3.0],
    }
}

fn record_height(record: [f32; 4]) -> f64 {
    let malformed = !(record[1] == 0.0 || record[1] == 1.0)
        || !(record[3] == 0.0 || record[3] == 1.0 || record[3] == 2.0 || record[3] == 3.0)
        || !record[2].is_finite()
        || record[2] < 0.0
        || record[2] != record[2].floor();
    if malformed || record[3] == 1.0 || record[3] == 2.0 {
        return 0.0;
    }
    if record[1] == 0.0 {
        if record[0] == -1.0 {
            return -2.0;
        }
        return 0.0;
    }
    if !record[0].is_finite() {
        return 0.0;
    }
    4.0 * f64::from(record[0] / CAP.max(1) as f32).clamp(0.0, 1.0) - 2.0
}

#[derive(Clone, Copy)]
struct Vertex {
    x: f64,
    y: f64,
    depth: f64,
    reciprocal_w: f64,
    grid: [f64; 2],
    world: [f64; 3],
    valid: bool,
    clamped: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Rule {
    Base,
    Fixed,
}

fn ambient_camera(mut point: [f64; 5], view: &ViewControls) -> [f64; 5] {
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

/// Line-for-line mirror of `scene_vertex`, with the base and fixed placement rules selectable.
fn scene_vertex(pose: &Pose, column: u32, row: u32, record: [f32; 4], rule: Rule) -> Vertex {
    let screen = draw_screen(column, row);
    let flat = Vertex {
        x: screen[0],
        y: screen[1],
        depth: 0.0,
        reciprocal_w: 1.0,
        grid: [f64::from(column), f64::from(row)],
        world: [0.0; 3],
        valid: true,
        clamped: false,
    };
    let invalid = Vertex {
        valid: false,
        ..flat
    };
    let PoseMap::Mapped(map) = pose.map else {
        return flat;
    };
    if pose.view.height_scale == 0.0 {
        return flat;
    }
    if record[3] == 2.0 {
        return if rule == Rule::Base { flat } else { invalid };
    }
    let denominator = map.rows[6].mul_add(screen[0], map.rows[7].mul_add(screen[1], map.rows[8]));
    if !denominator.is_finite() || denominator <= 0.0 {
        return if rule == Rule::Base { flat } else { invalid };
    }
    let offset = [
        map.rows[0].mul_add(screen[0], map.rows[1].mul_add(screen[1], map.rows[2])) / denominator,
        map.rows[3].mul_add(screen[0], map.rows[4].mul_add(screen[1], map.rows[5])) / denominator,
    ];
    let chart_scale = 4.0 * map.apron_scale / f64::from(pose.grid_width);
    let display: [f64; 4] = core::array::from_fn(|axis| {
        chart_scale
            * f64::from(pose.plane.basis_u[axis])
                .mul_add(offset[0], f64::from(pose.plane.basis_v[axis]) * offset[1])
    });
    let height = pose.view.height_scale * (record_height(record) + 2.0) * 0.5;
    let ambient = ambient_camera(
        [display[0], display[1], display[2], display[3], height],
        &pose.view,
    );
    let d5 = pose.view.distance_five;
    let d4 = pose.view.distance_four;
    let raw_five = d5 - ambient[4];
    let clamped = raw_five < 0.05 * d5;
    let denominator_five = if rule == Rule::Base {
        raw_five.max(0.05 * d5)
    } else {
        if clamped {
            return invalid;
        }
        raw_five
    };
    if denominator_five <= 1.0e-4 {
        return invalid;
    }
    let scale_five = d5 / denominator_five;
    let projected_four: [f64; 4] = core::array::from_fn(|axis| ambient[axis] * scale_five);
    let denominator_four = d4 - projected_four[3];
    if denominator_four <= 1.0e-4 {
        return invalid;
    }
    let scale_four = d4 / denominator_four;
    let world = [
        projected_four[0] * scale_four,
        projected_four[1] * scale_four,
        projected_four[2] * scale_four,
    ];
    let near = 0.1;
    let far = 4.0 * d4;
    let (yaw_sine, yaw_cosine) = pose.view.camera_yaw.sin_cos();
    let (pitch_sine, pitch_cosine) = pose.view.camera_pitch.sin_cos();
    let yawed = [
        yaw_cosine.mul_add(world[0], yaw_sine * world[2]),
        world[1],
        (-yaw_sine).mul_add(world[0], yaw_cosine * world[2]),
    ];
    let view_point = [
        yawed[0],
        pitch_cosine.mul_add(yawed[1], -pitch_sine * yawed[2]),
        pitch_sine.mul_add(yawed[1], pitch_cosine * yawed[2]) - d4,
    ];
    if -view_point[2] <= 1.0e-4 {
        return invalid;
    }
    let clip_depth = (far / (near - far)) * view_point[2] + far * near / (near - far);
    let aspect = f64::from(pose.grid_width) / f64::from(pose.grid_height);
    let perspective_scale = aspect * d4 * 0.5;
    let clip_w = -view_point[2];
    Vertex {
        x: perspective_scale * view_point[0] / aspect / clip_w * 0.5 * f64::from(pose.grid_width),
        y: perspective_scale * view_point[1] / clip_w * 0.5 * f64::from(pose.grid_height),
        depth: clip_depth / clip_w,
        reciprocal_w: clip_w.recip(),
        grid: [f64::from(column), f64::from(row)],
        world,
        valid: true,
        clamped,
    }
}

fn srgb(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8
}

fn shade(record: [f32; 4], light: f64) -> [f64; 3] {
    if record[3] == 2.0 {
        let exterior = exterior_zero(CLASSIC_PALETTE);
        return [
            f64::from(exterior[0]),
            f64::from(exterior[1]),
            f64::from(exterior[2]),
        ];
    }
    let base = shade_escape_record(record, CLASSIC_PALETTE).rgba;
    [
        f64::from(base[0]) * light,
        f64::from(base[1]) * light,
        f64::from(base[2]) * light,
    ]
}

fn render(pose: &Pose, records: &[[f32; 4]], rule: Rule) -> (Vec<[u8; 3]>, Vec<bool>) {
    let [width, height] = EXTENT;
    let vertices: Vec<Vertex> = (0..height)
        .flat_map(|row| {
            (0..width).map(move |column| {
                let record = records[(row * width + column) as usize];
                scene_vertex(pose, column, row, record, rule)
            })
        })
        .collect();
    let clear = exterior_zero(CLASSIC_PALETTE);
    let clear_rgb = [
        srgb(f64::from(clear[0])),
        srgb(f64::from(clear[1])),
        srgb(f64::from(clear[2])),
    ];
    let pixels = (width as usize) * (height as usize);
    let mut colour = vec![clear_rgb; pixels];
    let mut depth = vec![f64::INFINITY; pixels];
    let mut covered = vec![false; pixels];
    let half_w = 0.5 * f64::from(width);
    let half_h = 0.5 * f64::from(height);
    let light_direction = {
        let raw = [0.4_f64, 0.7, 0.6];
        let norm = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt();
        [raw[0] / norm, raw[1] / norm, raw[2] / norm]
    };
    for row in 0..height - 1 {
        for column in 0..width - 1 {
            let a = (row * width + column) as usize;
            let b = a + 1;
            let c = a + width as usize;
            let d = c + 1;
            for indices in [[a, b, c], [b, d, c]] {
                let tri = indices.map(|index| vertices[index]);
                if !tri.iter().all(|vertex| vertex.valid) {
                    continue;
                }
                if rule == Rule::Base && tri.iter().all(|vertex| vertex.clamped) {
                    continue;
                }
                // Geometric normal in world space, standing in for the shader's screen derivatives.
                let edge_one: [f64; 3] =
                    core::array::from_fn(|axis| tri[1].world[axis] - tri[0].world[axis]);
                let edge_two: [f64; 3] =
                    core::array::from_fn(|axis| tri[2].world[axis] - tri[0].world[axis]);
                let cross = [
                    edge_one[1] * edge_two[2] - edge_one[2] * edge_two[1],
                    edge_one[2] * edge_two[0] - edge_one[0] * edge_two[2],
                    edge_one[0] * edge_two[1] - edge_one[1] * edge_two[0],
                ];
                let norm = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
                let light = if norm > 1.0e-12 {
                    let normal = [cross[0] / norm, cross[1] / norm, cross[2] / norm];
                    let dot = normal[0] * light_direction[0]
                        + normal[1] * light_direction[1]
                        + normal[2] * light_direction[2];
                    0.24_f64.mul_add(dot.abs(), 0.58)
                } else {
                    0.24_f64.mul_add(1.0, 0.58)
                };
                let xs = [tri[0].x, tri[1].x, tri[2].x];
                let ys = [tri[0].y, tri[1].y, tri[2].y];
                let min_x = xs.iter().fold(f64::INFINITY, |m, v| m.min(*v));
                let max_x = xs.iter().fold(f64::NEG_INFINITY, |m, v| m.max(*v));
                let min_y = ys.iter().fold(f64::INFINITY, |m, v| m.min(*v));
                let max_y = ys.iter().fold(f64::NEG_INFINITY, |m, v| m.max(*v));
                if !(min_x.is_finite()
                    && max_x.is_finite()
                    && min_y.is_finite()
                    && max_y.is_finite())
                {
                    continue;
                }
                let first_column = ((min_x + half_w - 0.5).floor().max(0.0)) as i64;
                let last_column =
                    ((max_x + half_w - 0.5).ceil().min(f64::from(width) - 1.0)) as i64;
                let first_row = ((half_h - max_y - 0.5).floor().max(0.0)) as i64;
                let last_row = ((half_h - min_y - 0.5).ceil().min(f64::from(height) - 1.0)) as i64;
                if last_column < first_column || last_row < first_row {
                    continue;
                }
                let area = (tri[1].x - tri[0].x) * (tri[2].y - tri[0].y)
                    - (tri[1].y - tri[0].y) * (tri[2].x - tri[0].x);
                if !area.is_finite() || area.abs() <= 1.0e-12 {
                    continue;
                }
                for py in first_row..=last_row {
                    for px in first_column..=last_column {
                        let point = [px as f64 + 0.5 - half_w, half_h - (py as f64 + 0.5)];
                        let weight_zero = ((tri[1].x - point[0]) * (tri[2].y - point[1])
                            - (tri[1].y - point[1]) * (tri[2].x - point[0]))
                            / area;
                        let weight_one = ((tri[2].x - point[0]) * (tri[0].y - point[1])
                            - (tri[2].y - point[1]) * (tri[0].x - point[0]))
                            / area;
                        let weight_two = 1.0 - weight_zero - weight_one;
                        if weight_zero < 0.0 || weight_one < 0.0 || weight_two < 0.0 {
                            continue;
                        }
                        let weights = [weight_zero, weight_one, weight_two];
                        let fragment_depth =
                            weights.iter().zip(tri).fold(0.0, |sum, (weight, vertex)| {
                                weight.mul_add(vertex.depth, sum)
                            });
                        let index = py as usize * width as usize + px as usize;
                        if !(fragment_depth <= depth[index]) {
                            continue;
                        }
                        let reciprocal =
                            weights.iter().zip(tri).fold(0.0, |sum, (weight, vertex)| {
                                weight.mul_add(vertex.reciprocal_w, sum)
                            });
                        if !reciprocal.is_finite() || reciprocal <= 0.0 {
                            continue;
                        }
                        let grid: [f64; 2] = core::array::from_fn(|axis| {
                            weights.iter().zip(tri).fold(0.0, |sum, (weight, vertex)| {
                                (weight * vertex.reciprocal_w).mul_add(vertex.grid[axis], sum)
                            }) / reciprocal
                        });
                        let sample_column =
                            (grid[0] + 0.5).floor().clamp(0.0, f64::from(width) - 1.0) as u32;
                        let sample_row =
                            (grid[1] + 0.5).floor().clamp(0.0, f64::from(height) - 1.0) as u32;
                        let record = records[(sample_row * width + sample_column) as usize];
                        let linear = shade(record, light);
                        depth[index] = fragment_depth;
                        covered[index] = true;
                        colour[index] = [srgb(linear[0]), srgb(linear[1]), srgb(linear[2])];
                    }
                }
            }
        }
    }
    (colour, covered)
}

/// The browser's own classifier, over the canvas readback of the served frame.
const fn background(pixel: [u8; 3]) -> bool {
    pixel[0] > 200 && pixel[1] > 100 && pixel[1] < 190 && pixel[2] > 90 && pixel[2] < 170
}

/// The statistics the browser proof reports for one frame.
struct Statistics {
    /// Sampled columns of x 300..660 whose background classification flips six or more times.
    busy_columns: u32,
    /// Mean flips per sampled column.
    mean_transitions: f64,
    /// Background share of the four canvas bands the proof reports.
    bands: [f64; 4],
    /// Share of each band the mesh covered at all.
    covered: [f64; 4],
}

const BANDS: [(usize, usize); 4] = [(0, 90), (90, 270), (270, 430), (430, 540)];

fn statistics(image: &[[u8; 3]], covered: &[bool]) -> Statistics {
    let width = EXTENT[0] as usize;
    let mut busy_columns = 0;
    let mut transitions_total = 0_u32;
    let mut columns = 0_u32;
    for x in (300..660).step_by(4) {
        columns += 1;
        let mut transitions = 0;
        let mut previous = background(image[180 * width + x]);
        for y in 181..430 {
            let current = background(image[y * width + x]);
            if current != previous {
                transitions += 1;
                previous = current;
            }
        }
        transitions_total += transitions;
        busy_columns += u32::from(transitions >= 6);
    }
    let share = |low: usize, high: usize, test: &dyn Fn(usize) -> bool| {
        let mut hits = 0_u64;
        for y in low..high {
            for x in 0..width {
                hits += u64::from(test(y * width + x));
            }
        }
        hits as f64 / ((high - low) * width) as f64
    };
    Statistics {
        busy_columns,
        mean_transitions: f64::from(transitions_total) / f64::from(columns),
        bands: core::array::from_fn(|slot| {
            share(BANDS[slot].0, BANDS[slot].1, &|index| {
                background(image[index])
            })
        }),
        covered: core::array::from_fn(|slot| {
            share(BANDS[slot].0, BANDS[slot].1, &|index| covered[index])
        }),
    }
}

fn steep_records(pose: &Pose) -> Vec<[f32; 4]> {
    (0..EXTENT[1])
        .flat_map(|row| {
            (0..EXTENT[0]).map(move |column| sample_pose(pose, pixel_screen(column, row)))
        })
        .collect()
}

/// This render is the frame the browser read back, to the precision either can claim.
///
/// The browser measured, on the served build of this lane's head, 38 of 90 sampled columns with
/// six or more background flips, a mean of 5.4, and background shares 0.999 / 0.749 / 0.796 /
/// 0.882 over the four bands; on the previous build 39, 5.3, and 0.999 / 0.749 / 0.797 / 0.910.
/// It also reported exactly two background colours, (255,129,129) and (221,112,112), whose ratio
/// 0.867 is the fragment lighting term: the first is the pass clear and an unlit horizon fragment,
/// the second a lit exterior fragment of the drawn mesh. This oracle reproduces all of that.
#[test]
fn the_native_render_reproduces_the_browser_readback_of_this_row() {
    let pose = pose_with(steep_view(3.565));
    let records = steep_records(&pose);
    let horizon = records.iter().filter(|record| record[3] == 2.0).count();
    assert_eq!(horizon, 17_556, "the row's horizon record count");
    let (image, covered) = render(&pose, &records, Rule::Fixed);
    let measured = statistics(&image, &covered);
    assert!(
        (39..=43).contains(&measured.busy_columns),
        "busy columns {} against the browser's 38",
        measured.busy_columns
    );
    assert!((measured.mean_transitions - 5.4).abs() <= 0.3);
    for (slot, browser) in [0.999, 0.749, 0.796, 0.882].into_iter().enumerate() {
        assert!(
            (measured.bands[slot] - browser).abs() <= 0.03,
            "band {slot} share {} against the browser's {browser}",
            measured.bands[slot]
        );
    }
    // The background is drawn mesh, not sky: the middle band is covered almost everywhere.
    assert!(measured.covered[1] > 0.99, "{}", measured.covered[1]);
    let clear = image[0];
    assert_eq!(
        clear,
        [255, 129, 129],
        "the pass clear is the exterior colour"
    );
}

/// Refusing the two kinds of vertex changes one part of the frame in a hundred, and only below it.
///
/// This is why the browser proof saw an unchanged picture: a refused sample was already painted
/// the exterior colour, and the pass clears to that same colour, so the change is a lighting
/// change over the ground below the plane's horizon rather than a change of shape.
#[test]
fn the_refusals_change_only_the_lower_third_of_this_row_s_frame() {
    let pose = pose_with(steep_view(3.565));
    let records = steep_records(&pose);
    let (base, base_cover) = render(&pose, &records, Rule::Base);
    let (fixed, fixed_cover) = render(&pose, &records, Rule::Fixed);
    let width = EXTENT[0] as usize;
    let mut differing = 0_u64;
    let mut first_row = usize::MAX;
    for y in 0..EXTENT[1] as usize {
        for x in 0..width {
            if base[y * width + x] != fixed[y * width + x] {
                differing += 1;
                first_row = first_row.min(y);
            }
        }
    }
    let total = u64::from(EXTENT[0]) * u64::from(EXTENT[1]);
    assert!(
        (4_000..7_000).contains(&differing),
        "the refusals changed {differing} of {total} pixels"
    );
    assert!(
        first_row >= 350,
        "the first changed row is {first_row}, above the plane's horizon"
    );
    let base_statistics = statistics(&base, &base_cover);
    let fixed_statistics = statistics(&fixed, &fixed_cover);
    assert!(
        (base_statistics.covered[3] - fixed_statistics.covered[3]) > 0.3,
        "the refusals must clear a large share of the lowest band"
    );
    assert!(
        (base_statistics.bands[3] - fixed_statistics.bands[3]).abs() < 0.05,
        "and must barely move its background classification, which is why the proof saw nothing"
    );
}

/// The curtain is the height control, continuous from the flat picture that has none.
///
/// At zero height the frame is the two-dimensional chart: fully covered, no column flipping six
/// times. The flips appear as the amplitude rises and are monotone in it up to the row's own
/// 3.565, so the streaks are the relief this row asks for and not a defect of the pass.
#[test]
fn the_curtain_grows_with_the_height_control_from_a_flat_frame_that_has_none() {
    let sampling = pose_with(steep_view(3.565));
    let records = steep_records(&sampling);
    let mut previous = 0;
    for (height_scale, floor, ceiling) in [
        (0.0_f64, 0_u32, 0_u32),
        (0.2, 0, 4),
        (1.0, 20, 40),
        (3.565, 35, 50),
    ] {
        let pose = pose_with(steep_view(height_scale));
        let (image, covered) = render(&pose, &records, Rule::Fixed);
        let measured = statistics(&image, &covered);
        assert!(
            (floor..=ceiling).contains(&measured.busy_columns),
            "height {height_scale} gave {} busy columns",
            measured.busy_columns
        );
        if height_scale == 0.0 {
            assert!(
                measured.covered.iter().all(|share| *share > 0.999),
                "a flat chart covers the frame it was sampled for"
            );
        } else {
            assert!(measured.busy_columns >= previous, "height {height_scale}");
        }
        previous = measured.busy_columns;
    }
}
