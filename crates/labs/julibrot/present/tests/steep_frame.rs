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
    BigCentre, Homography, ObjectAngles, Pose, PoseMap, ViewControls, centre_from_reference_px,
    construct_plane, pixel_scale, plane_to_screen, screen_to_plane,
};
use ember_julibrot_present::{
    CLASSIC_PALETTE, SceneUniform, exterior_zero, grid_screen, shade_escape_record,
};

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

/// The 2026-09-06 row: the same pose and the same mesh, two record fields apart.
///
/// The row is the accepted steep row with exactly two fields changed, `zoom_log2` and the centre.
/// Neither reaches [`scene_vertex`]: the screen map is zoom-free by
/// `screen_to_plane`'s own contract, and the centre enters only where a pixel is turned into a
/// four-dimensional point. So the mesh this pose draws is bit-identical to the steep row's mesh
/// and every difference in the picture is a difference in the records that mesh is lifted and
/// painted by.
const ZOOM_LOG2: f64 = 1.259_194_831_013_92;
const ZOOM_CENTRE: [f64; 4] = [
    -0.446_045_388_951_547_5,
    -0.036_447_606_537_045_21,
    1.027_888_715_651_299_5,
    -0.205_307_539_517_433_4,
];
const CENTRE_BITS: u32 = 1024;

fn zoom_pose(height_scale: f64) -> Pose {
    let mut pose = pose_with(steep_view(height_scale));
    let centre = BigCentre::from_f64(ZOOM_CENTRE, CENTRE_BITS).expect("row centre");
    let reference = BigCentre::from_f64(PLANE_ORIGIN, CENTRE_BITS).expect("row origin");
    pose.zoom_log2 = ZOOM_LOG2;
    pose.centre_from_reference_px =
        centre_from_reference_px(&centre, &reference, &pose.plane, ZOOM_LOG2, EXTENT[0])
            .expect("the row's centre lies in the row's own plane");
    pose
}

/// Reproduces the browser's screen-centred zoom against the retained frame's reference.
///
/// # Panics
///
/// Panics only if the measured relief zoom row loses its finite screen-centre map, which the saved
/// browser pose cannot arrange.
fn screen_centred_zoom_destination(from: &Pose, zoom_delta: f64) -> Pose {
    let anchor = map_plane_offset(from, [0.0; 2])
        .expect("the measured relief zoom row contains the screen centre");
    let displacement_scale = zoom_delta.exp2() - 1.0;
    let mut to = *from;
    to.zoom_log2 += zoom_delta;
    to.centre_from_reference_px = anchor.map(|value| value * displacement_scale);
    to
}

/// Carries the saved row's absolute sampling centre through the same screen-centred zoom.
fn screen_centred_sampling_pose(
    from: &Pose,
    destination: &Pose,
    zoom_delta: f64,
) -> Pose {
    let zoom_scale = zoom_delta.exp2();
    let mut to = *from;
    to.zoom_log2 += zoom_delta;
    to.centre_from_reference_px = core::array::from_fn(|axis| {
        zoom_scale.mul_add(
            from.centre_from_reference_px[axis],
            destination.centre_from_reference_px[axis],
        )
    });
    to
}

fn multiply_3x3(left: [f64; 9], right: [f64; 9]) -> [f64; 9] {
    core::array::from_fn(|index| {
        let row = index / 3;
        let column = index % 3;
        (0..3).fold(0.0, |sum, inner| {
            left[row * 3 + inner].mul_add(right[inner * 3 + column], sum)
        })
    })
}

fn invert_3x3(matrix: [f64; 9]) -> Option<[f64; 9]> {
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

/// Builds the source-lattice map whose output is expressed in destination chart pixels.
///
/// # Panics
///
/// Panics only if either accepted measured pose loses its map or scale, or if their plane relation
/// is singular, none of which the fixed zoom pair can arrange.
fn relief_redraw_pose(from: &Pose, to: &Pose) -> Pose {
    let PoseMap::Mapped(from_map) = from.map else {
        panic!("the retained measured relief zoom pose has a finite map");
    };
    let PoseMap::Mapped(to_map) = to.map else {
        panic!("the destination measured relief zoom pose has a finite map");
    };
    let from_scale = pixel_scale(from.zoom_log2, from.grid_width)
        .expect("the retained measured relief zoom pose has a finite scale");
    let to_scale = pixel_scale(to.zoom_log2, to.grid_width)
        .expect("the destination measured relief zoom pose has a finite scale");
    let scale = from_scale / to_scale;
    let origin_delta: [f64; 4] =
        core::array::from_fn(|axis| from.plane_origin[axis] - to.plane_origin[axis]);
    let basis = [to.plane.basis_u, to.plane.basis_v];
    let source_basis = [from.plane.basis_u, from.plane.basis_v];
    let relation: [[f64; 2]; 2] = core::array::from_fn(|to_axis| {
        core::array::from_fn(|from_axis| {
            source_basis[from_axis]
                .into_iter()
                .zip(basis[to_axis])
                .fold(0.0, |sum, (source, destination)| {
                    f64::from(source).mul_add(f64::from(destination), sum)
                })
                * scale
        })
    });
    let shift: [f64; 2] = core::array::from_fn(|axis| {
        let origin = origin_delta
            .into_iter()
            .zip(basis[axis])
            .fold(0.0, |sum, (value, destination)| {
                value.mul_add(f64::from(destination), sum)
            })
            / to_scale;
        relation[axis][0].mul_add(
            from.centre_from_reference_px[0],
            relation[axis][1].mul_add(
                from.centre_from_reference_px[1],
                origin - to.centre_from_reference_px[axis],
            ),
        )
    });
    let source_to_destination = [
        relation[0][0],
        relation[0][1],
        shift[0],
        relation[1][0],
        relation[1][1],
        shift[1],
        0.0,
        0.0,
        1.0,
    ];
    let rows = multiply_3x3(source_to_destination, from_map.rows);
    let inverse = invert_3x3(rows).expect("the relief redraw chart map is invertible");
    let mut redraw = *to;
    redraw.grid_width = from.grid_width;
    redraw.grid_height = from.grid_height;
    redraw.map = PoseMap::Mapped(Homography {
        rows,
        inverse,
        condition_number: from_map.condition_number,
        apron_scale: to_map.apron_scale * f64::from(from.grid_width)
            / f64::from(to.grid_width),
    });
    redraw
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

/// Where a never-escaped record is placed on the chart's own height axis.
///
/// `InteriorAtFloor` is the shipped law, mirrored below. `InteriorAtPeak` is the alternative the
/// rulings name — the interior continuous with the deepest escapes rather than opposite them —
/// carried here only so this oracle can measure what it would change in one frame.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mapping {
    InteriorAtFloor,
    InteriorAtPeak,
}

fn record_height_under(record: [f32; 4], mapping: Mapping) -> f64 {
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
            return match mapping {
                Mapping::InteriorAtFloor => -2.0,
                Mapping::InteriorAtPeak => 2.0,
            };
        }
        return 0.0;
    }
    if !record[0].is_finite() {
        return 0.0;
    }
    4.0 * f64::from(record[0] / CAP.max(1) as f32).clamp(0.0, 1.0) - 2.0
}

fn record_height(record: [f32; 4]) -> f64 {
    record_height_under(record, Mapping::InteriorAtFloor)
}

/// Which gate of `scene_vertex` decided this vertex, in the shader's own order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Reason {
    /// Placed by the projection and drawn.
    Drawn,
    /// The flat path: no height amplitude, so the vertex stays where the chart puts it.
    Flat,
    /// A horizon record: the plane reaches no point at this pixel.
    Horizon,
    /// The screen map's denominator was not positive here.
    Map,
    /// Past the five-dimensional near limit at `0.05 * d5`.
    NearLimit,
    /// At or past the five-dimensional pole itself.
    FivePole,
    /// At or past the four-dimensional pole.
    FourPole,
    /// Behind the observer's own near plane.
    Observer,
}

#[derive(Clone, Copy)]
struct Vertex {
    x: f64,
    y: f64,
    depth: f64,
    reciprocal_w: f64,
    grid: [f64; 2],
    chart: [f64; 2],
    world: [f64; 3],
    valid: bool,
    clamped: bool,
    reason: Reason,
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
fn scene_vertex(
    pose: &Pose,
    column: u32,
    row: u32,
    record: [f32; 4],
    rule: Rule,
    mapping: Mapping,
) -> Vertex {
    let screen = draw_screen(column, row);
    let flat = Vertex {
        x: screen[0],
        y: screen[1],
        depth: 0.0,
        reciprocal_w: 1.0,
        grid: [f64::from(column), f64::from(row)],
        chart: [f64::NAN; 2],
        world: [0.0; 3],
        valid: true,
        clamped: false,
        reason: Reason::Flat,
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
        return if rule == Rule::Base {
            Vertex {
                reason: Reason::Horizon,
                ..flat
            }
        } else {
            Vertex {
                reason: Reason::Horizon,
                ..invalid
            }
        };
    }
    let denominator = map.rows[6].mul_add(screen[0], map.rows[7].mul_add(screen[1], map.rows[8]));
    if !denominator.is_finite() || denominator <= 0.0 {
        return if rule == Rule::Base {
            Vertex {
                reason: Reason::Map,
                ..flat
            }
        } else {
            Vertex {
                reason: Reason::Map,
                ..invalid
            }
        };
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
    let height = pose.view.height_scale * (record_height_under(record, mapping) + 2.0) * 0.5;
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
            return Vertex {
                reason: Reason::NearLimit,
                ..invalid
            };
        }
        raw_five
    };
    if denominator_five <= 1.0e-4 {
        return Vertex {
            reason: Reason::FivePole,
            ..invalid
        };
    }
    let scale_five = d5 / denominator_five;
    let projected_four: [f64; 4] = core::array::from_fn(|axis| ambient[axis] * scale_five);
    let denominator_four = d4 - projected_four[3];
    if denominator_four <= 1.0e-4 {
        return Vertex {
            reason: Reason::FourPole,
            ..invalid
        };
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
        return Vertex {
            reason: Reason::Observer,
            ..invalid
        };
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
        chart: offset,
        world,
        valid: true,
        clamped,
        reason: Reason::Drawn,
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

/// What painted one pixel, as the pass itself decided it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cause {
    /// No primitive reached this pixel; the pass clear stands.
    Sky,
    /// A never-escaped record, placed by the height law at the chart floor.
    Interior,
    /// An escaped record whose lift is at most a twentieth of the cliff: the far exterior.
    ExteriorFloor,
    /// An escaped record lifted higher than that.
    LiftedEscape,
    /// A horizon record: the screen map reaches no plane point at that pixel.
    Horizon,
    /// A status-one or status-three record.
    Uncertain,
    /// A record the contract calls malformed; the pass answers with the debug tint.
    Malformed,
}

/// The lift, as a fraction of the full cliff, below which an escaped record is the exterior floor.
const FLOOR_FRACTION: f64 = 0.05;

fn cause_of(record: [f32; 4], floor_fraction: f64) -> Cause {
    if record[3] == 2.0 {
        return Cause::Horizon;
    }
    if record[3] == 1.0 || record[3] == 3.0 {
        return Cause::Uncertain;
    }
    if record[1] == 0.0 {
        return if record[0] == -1.0 {
            Cause::Interior
        } else {
            Cause::Malformed
        };
    }
    if record[1] != 1.0 {
        return Cause::Malformed;
    }
    if record_height(record) <= 4.0_f64.mul_add(floor_fraction, -2.0) {
        Cause::ExteriorFloor
    } else {
        Cause::LiftedEscape
    }
}

struct Frame {
    colour: Vec<[u8; 3]>,
    covered: Vec<bool>,
    cause: Vec<Cause>,
    record: Vec<Option<[f32; 4]>>,
    chart: Vec<Option<[f64; 2]>>,
    /// Pixels painted with a record belonging to no corner of the primitive that covered them.
    foreign: u64,
}

fn render(pose: &Pose, records: &[[f32; 4]], rule: Rule) -> (Vec<[u8; 3]>, Vec<bool>) {
    let frame = render_frame(pose, records, rule, Mapping::InteriorAtFloor);
    (frame.colour, frame.covered)
}

fn render_frame(pose: &Pose, records: &[[f32; 4]], rule: Rule, mapping: Mapping) -> Frame {
    let [width, height] = EXTENT;
    let vertices: Vec<Vertex> = (0..height)
        .flat_map(|row| {
            (0..width).map(move |column| {
                let record = records[(row * width + column) as usize];
                scene_vertex(pose, column, row, record, rule, mapping)
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
    let mut cause = vec![Cause::Sky; pixels];
    let mut sampled_record = vec![None; pixels];
    let mut sampled_chart = vec![None; pixels];
    let mut foreign = 0_u64;
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
                // The shader's fallback is a NORMAL, not a lighting term: it starts at
                // `vec3(0,0,1)` and replaces it only when the screen derivatives give a usable
                // cross product. Standing in a saturated term instead was wrong wherever the
                // fallback is taken, which at height zero is the whole frame — every vertex there
                // carries `world = vec3(0)`, so no triangle has a normal at all.
                let normal = if norm > 1.0e-12 {
                    [cross[0] / norm, cross[1] / norm, cross[2] / norm]
                } else {
                    [0.0, 0.0, 1.0]
                };
                let dot = normal[0] * light_direction[0]
                    + normal[1] * light_direction[1]
                    + normal[2] * light_direction[2];
                let light = 0.24_f64.mul_add(dot.abs(), 0.58);
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
                        let chart: [f64; 2] = core::array::from_fn(|axis| {
                            weights.iter().zip(tri).fold(0.0, |sum, (weight, vertex)| {
                                (weight * vertex.reciprocal_w).mul_add(vertex.chart[axis], sum)
                            }) / reciprocal
                        });
                        let sample_column =
                            (grid[0] + 0.5).floor().clamp(0.0, f64::from(width) - 1.0) as u32;
                        let sample_row =
                            (grid[1] + 0.5).floor().clamp(0.0, f64::from(height) - 1.0) as u32;
                        let record = records[(sample_row * width + sample_column) as usize];
                        // The fragment stage resolves a pixel to a sample by rounding the
                        // interpolated grid coordinate. The interpolant is a convex combination of
                        // the primitive's three grid coordinates, so the sample it lands on must be
                        // a corner of the primitive's own cell; anything else would paint a pixel
                        // with a record belonging to a different part of the object.
                        if sample_column < column
                            || sample_column > column + 1
                            || sample_row < row
                            || sample_row > row + 1
                        {
                            foreign += 1;
                        }
                        let linear = shade(record, light);
                        depth[index] = fragment_depth;
                        covered[index] = true;
                        cause[index] = cause_of(record, FLOOR_FRACTION);
                        sampled_record[index] = Some(record);
                        sampled_chart[index] = Some(chart);
                        colour[index] = [srgb(linear[0]), srgb(linear[1]), srgb(linear[2])];
                    }
                }
            }
        }
    }
    Frame {
        colour,
        covered,
        cause,
        record: sampled_record,
        chart: sampled_chart,
        foreign,
    }
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

fn render_relief_redraw(from: &Pose, to: &Pose, records: &[[f32; 4]]) -> Frame {
    render_frame(
        &relief_redraw_pose(from, to),
        records,
        Rule::Fixed,
        Mapping::InteriorAtFloor,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecordClass {
    Interior,
    Escape(i64),
    Horizon,
    Uncertain,
    Malformed,
}

fn record_class(cause: Cause, record: [f32; 4]) -> RecordClass {
    match cause {
        Cause::Interior => RecordClass::Interior,
        Cause::ExteriorFloor | Cause::LiftedEscape => {
            RecordClass::Escape(f64::from(record[0]).floor() as i64)
        }
        Cause::Horizon => RecordClass::Horizon,
        Cause::Uncertain => RecordClass::Uncertain,
        Cause::Malformed | Cause::Sky => RecordClass::Malformed,
    }
}

fn classes_agree(left: RecordClass, right: RecordClass) -> bool {
    match (left, right) {
        (RecordClass::Escape(left), RecordClass::Escape(right)) => left.abs_diff(right) <= 1,
        _ => left == right,
    }
}

fn broad_classes_agree(left: RecordClass, right: RecordClass) -> bool {
    matches!((left, right), (RecordClass::Escape(_), RecordClass::Escape(_))) || left == right
}

fn records_equal(left: [f32; 4], right: [f32; 4]) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| left.to_bits() == right.to_bits())
}

#[derive(Debug)]
struct ReliefRedrawMeasurement {
    zoom_delta: f64,
    agree: u64,
    hole: u64,
    occluded_wrong: u64,
    resolution_only: u64,
    agree_record_changed: u64,
    resolution_same_broad_class: u64,
    resolution_class_changed: u64,
    largest_connected_hole: u64,
    maximum_same_pixel_chart_separation_px: f64,
    source_texel_reach_px: f64,
    agree_fraction: f64,
    hole_fraction: f64,
    occluded_wrong_fraction: f64,
    resolution_only_fraction: f64,
    exposed_fraction: f64,
    requested_centre_truth_px: [f64; 2],
    requested_centre_redraw_px: [f64; 2],
    requested_centre_error_px: f64,
    requested_anchor_truth_px: [f64; 2],
    requested_anchor_redraw_px: [f64; 2],
    requested_anchor_error_px: f64,
}

fn largest_connected_region(mask: &[bool]) -> u64 {
    let [width, height] = EXTENT.map(|value| value as usize);
    let mut visited = vec![false; mask.len()];
    let mut largest = 0_u64;
    for seed in 0..mask.len() {
        if visited[seed] || !mask[seed] {
            continue;
        }
        let mut size = 0_u64;
        let mut pending = std::collections::VecDeque::from([seed]);
        visited[seed] = true;
        while let Some(index) = pending.pop_front() {
            size = size.saturating_add(1);
            let column = index % width;
            let row = index / width;
            let neighbours = [
                (column > 0).then_some(index - 1),
                (column + 1 < width).then_some(index + 1),
                (row > 0).then_some(index - width),
                (row + 1 < height).then_some(index + width),
            ];
            for neighbour in neighbours.into_iter().flatten() {
                if !visited[neighbour] && mask[neighbour] {
                    visited[neighbour] = true;
                    pending.push_back(neighbour);
                }
            }
        }
        largest = largest.max(size);
    }
    largest
}

fn presented_pixel(screen: [f64; 2]) -> [f64; 2] {
    [
        screen[0] + 0.5 * f64::from(EXTENT[0]),
        0.5 * f64::from(EXTENT[1]) - screen[1],
    ]
}

fn relief_redraw_position(from: &Pose, to: &Pose, target: [f64; 2]) -> [f64; 2] {
    let redraw = relief_redraw_pose(from, to);
    let PoseMap::Mapped(redraw_map) = redraw.map else {
        panic!("the relief redraw map is finite");
    };
    let PoseMap::Mapped(to_map) = to.map else {
        panic!("the destination map is finite");
    };
    let target_chart = map_plane_offset(to, target).expect("the requested point reaches the chart");
    let source_screen = plane_to_screen(&redraw_map, target_chart)
        .expect("the requested point reaches the source lattice");
    assert!(
        source_screen[0].abs() <= 0.5 * f64::from(from.grid_width) + 1.0e-9
            && source_screen[1].abs() <= 0.5 * f64::from(from.grid_height) + 1.0e-9,
        "the requested point {target:?} is outside the retained lattice at {source_screen:?}"
    );
    let redrawn_chart =
        map_plane_offset(&redraw, source_screen).expect("the redraw preserves the chart point");
    plane_to_screen(&to_map, redrawn_chart).expect("the redraw chart point projects to the surface")
}

fn placement(
    from: &Pose,
    to: &Pose,
    target: [f64; 2],
) -> ([f64; 2], [f64; 2], f64) {
    let truth = presented_pixel(target);
    let redraw = presented_pixel(relief_redraw_position(from, to, target));
    let error = (redraw[0] - truth[0]).hypot(redraw[1] - truth[1]);
    (truth, redraw, error)
}

fn measure_relief_redraw(
    from: &Pose,
    to: &Pose,
    redraw: &Frame,
    truth: &Frame,
    zoom_delta: f64,
) -> ReliefRedrawMeasurement {
    let total = u64::from(EXTENT[0]) * u64::from(EXTENT[1]);
    let source_step = zoom_delta.exp2();
    let source_texel_reach_px = core::f64::consts::FRAC_1_SQRT_2 * (source_step + 1.0);
    let mut agree = 0_u64;
    let mut hole = 0_u64;
    let mut occluded_wrong = 0_u64;
    let mut resolution_only = 0_u64;
    let mut agree_record_changed = 0_u64;
    let mut resolution_same_broad_class = 0_u64;
    let mut resolution_class_changed = 0_u64;
    let mut maximum_same_pixel_chart_separation_px = 0.0_f64;
    let mut hole_mask = vec![false; redraw.covered.len()];
    for index in 0..redraw.covered.len() {
        match (redraw.covered[index], truth.covered[index]) {
            (false, false) => agree = agree.saturating_add(1),
            (false, true) => {
                hole = hole.saturating_add(1);
                hole_mask[index] = true;
            }
            (true, false) => occluded_wrong = occluded_wrong.saturating_add(1),
            (true, true) => {
                let redrawn_chart = redraw.chart[index].expect("a redrawn fragment has a chart point");
                let true_chart = truth.chart[index].expect("a true fragment has a chart point");
                let separation = (redrawn_chart[0] - true_chart[0])
                    .hypot(redrawn_chart[1] - true_chart[1]);
                maximum_same_pixel_chart_separation_px =
                    maximum_same_pixel_chart_separation_px.max(separation);
                if separation > source_texel_reach_px + 1.0e-9 {
                    occluded_wrong = occluded_wrong.saturating_add(1);
                    continue;
                }
                let redrawn_record =
                    redraw.record[index].expect("a redrawn fragment resolves a record");
                let true_record = truth.record[index].expect("a true fragment resolves a record");
                let redrawn_class = record_class(redraw.cause[index], redrawn_record);
                let true_class = record_class(truth.cause[index], true_record);
                if classes_agree(redrawn_class, true_class) {
                    agree = agree.saturating_add(1);
                    agree_record_changed = agree_record_changed
                        .saturating_add(u64::from(!records_equal(redrawn_record, true_record)));
                } else {
                    resolution_only = resolution_only.saturating_add(1);
                    if broad_classes_agree(redrawn_class, true_class) {
                        resolution_same_broad_class =
                            resolution_same_broad_class.saturating_add(1);
                    } else {
                        resolution_class_changed = resolution_class_changed.saturating_add(1);
                    }
                }
            }
        }
    }
    assert_eq!(
        agree + hole + occluded_wrong + resolution_only,
        total,
        "the pixel classes partition the destination"
    );
    assert_eq!(
        resolution_same_broad_class + resolution_class_changed,
        resolution_only,
        "the resolution subclasses partition their class"
    );
    let centre = placement(from, to, [0.0; 2]);
    let anchor = placement(
        from,
        to,
        [
            -0.5 * f64::from(to.grid_width),
            0.5 * f64::from(to.grid_height),
        ],
    );
    ReliefRedrawMeasurement {
        zoom_delta,
        agree,
        hole,
        occluded_wrong,
        resolution_only,
        agree_record_changed,
        resolution_same_broad_class,
        resolution_class_changed,
        largest_connected_hole: largest_connected_region(&hole_mask),
        maximum_same_pixel_chart_separation_px,
        source_texel_reach_px,
        agree_fraction: agree as f64 / total as f64,
        hole_fraction: hole as f64 / total as f64,
        occluded_wrong_fraction: occluded_wrong as f64 / total as f64,
        resolution_only_fraction: resolution_only as f64 / total as f64,
        exposed_fraction: (hole + occluded_wrong) as f64 / total as f64,
        requested_centre_truth_px: centre.0,
        requested_centre_redraw_px: centre.1,
        requested_centre_error_px: centre.2,
        requested_anchor_truth_px: anchor.0,
        requested_anchor_redraw_px: anchor.1,
        requested_anchor_error_px: anchor.2,
    }
}

#[test]
fn measured_relief_zoom_redraw_reports_the_native_pixel_oracle() {
    let sampling_from = zoom_pose(3.565);
    let source_records = steep_records(&sampling_from);
    let mut from = sampling_from;
    from.centre_from_reference_px = [0.0; 2];
    let mut reports = Vec::new();
    for zoom_delta in [0.1, 0.5] {
        let to = screen_centred_zoom_destination(&from, zoom_delta);
        if zoom_delta == 0.1 {
            for (actual, pinned) in to
                .centre_from_reference_px
                .into_iter()
                .zip([16.654_733, -3.317_900])
            {
                assert!(
                    (actual - pinned).abs() < 1.0e-6,
                    "the 960 by 540 browser displacement changed: {actual} != {pinned}"
                );
            }
        }
        let sampling_to = screen_centred_sampling_pose(&sampling_from, &to, zoom_delta);
        assert_eq!(
            sample_pose(&sampling_from, [0.0; 2]),
            sample_pose(&sampling_to, [0.0; 2]),
            "screen-centred zoom {zoom_delta} must preserve the point under the centre"
        );
        let true_records = steep_records(&sampling_to);
        let redraw = render_relief_redraw(&from, &to, &source_records);
        let truth = render_frame(
            &to,
            &true_records,
            Rule::Fixed,
            Mapping::InteriorAtFloor,
        );
        reports.push(measure_relief_redraw(
            &from,
            &to,
            &redraw,
            &truth,
            zoom_delta,
        ));
    }
    panic!("native relief redraw measurements:\n{reports:#?}");
}

/// This render lands in the band the browser read back, which is a calibration, not a proof.
///
/// The browser measured, on the served build of this lane's head, 38 of 90 sampled columns with
/// six or more background flips, a mean of 5.4, and background shares 0.999 / 0.749 / 0.796 /
/// 0.882 over the four bands; on the previous build 39, 5.3, and 0.999 / 0.749 / 0.797 / 0.910.
/// It also reported exactly two background colours, (255,129,129) and (221,112,112), whose ratio
/// 0.867 is the fragment lighting term: the first is the pass clear and an unlit horizon fragment,
/// the second a lit exterior fragment of the drawn mesh.
///
/// The tolerances below admit BOTH of those readbacks, so this test says the oracle is the right
/// instrument for this frame and says nothing about which build produced it. It cannot: the two
/// builds' browser numbers differ by less than the oracle's own record source does. What
/// discriminates the builds is the A/B in
/// [`the_refusals_change_only_the_lower_third_of_this_row_s_frame`], which renders one record
/// array under both rules.
#[test]
fn the_native_render_lands_in_the_browser_s_calibration_band_for_this_row() {
    let pose = pose_with(steep_view(3.565));
    let records = steep_records(&pose);
    // Two horizon conventions, four samples apart. The records are sampled at pixel centres, which
    // is what the kernel does; the mesh places its outermost ring on the frame boundary instead, so
    // a vertex of that ring can fall on the other side of the plane's horizon line from the pixel
    // centre it carries. Both are named wherever either number is published.
    let horizon_records = records.iter().filter(|record| record[3] == 2.0).count();
    let horizon_vertices = (0..EXTENT[1])
        .flat_map(|row| (0..EXTENT[0]).map(move |column| draw_screen(column, row)))
        .filter(|screen| map_plane_offset(&pose, *screen).is_none())
        .count();
    assert_eq!(horizon_records, 17_556, "horizon records at pixel centres");
    assert_eq!(horizon_vertices, 17_560, "horizon vertices at draw points");

    let (image, covered) = render(&pose, &records, Rule::Fixed);
    let measured = statistics(&image, &covered);
    assert!(
        (36..=45).contains(&measured.busy_columns),
        "busy columns {} outside the band around the browser's 38 and 39",
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

/// Refusing the two kinds of vertex changes the lower third of this frame and nothing above it.
///
/// This is why the browser proof saw an unchanged picture: a refused sample was already painted
/// the exterior colour, and the pass clears to that same colour, so the change is a lighting
/// change over the ground below the plane's horizon rather than a change of shape.
///
/// The pixel count and the first changed row below are properties of THIS oracle's record field,
/// which comes from `escape_shallow_point` and not from the shallow WGSL kernel that fills the
/// delivered grid. They do not transfer to the frame: adding one iteration to every escaped record
/// of this same array, with the rule held fixed, changes 391,471 of 518,400 pixels from row 8 down,
/// so the set of vertices sitting past the near limit — and the area the refusal clears — moves
/// with the record field far more than with the rule. The browser's own readback of the two builds
/// puts the change at 11,440 pixels or more, with 47 of them above row 362. What transfers is what
/// the horizon geometry governs and the records do not: the location below the plane's horizon,
/// the direction, and the size of the band-3 background shift.
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
    assert_eq!(
        differing, 5_442,
        "the refusals changed {differing} of {total} pixels of this oracle's own record field"
    );
    assert_eq!(
        first_row, 362,
        "the first row this oracle's record field changes at"
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
/// At zero height the frame is the two-dimensional chart: fully covered, with no column flipping
/// six times. One column does at amplitude 0.2, thirty-one at 1.0 and forty-one at the row's own
/// 3.565, monotone throughout, so the streaks are the relief this row asks for and not a defect of
/// the pass. The counts are exact for this oracle's record field, which is what the doc paragraph
/// in `docs/julibrot/present.md` quotes.
#[test]
fn the_curtain_grows_with_the_height_control_from_a_flat_frame_that_has_none() {
    let sampling = pose_with(steep_view(3.565));
    let records = steep_records(&sampling);
    let mut previous = 0;
    for (height_scale, expected) in [(0.0_f64, 0_u32), (0.2, 1), (1.0, 31), (3.565, 41)] {
        let pose = pose_with(steep_view(height_scale));
        let (image, covered) = render(&pose, &records, Rule::Fixed);
        let measured = statistics(&image, &covered);
        assert_eq!(
            measured.busy_columns, expected,
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

/// The zoomed row's sampling map round-trips one pixel through the app's own two functions.
///
/// `sample_pose` composes three things the app composes: the screen map from `screen_to_plane`,
/// the pixel scale from `pixel_scale`, and the centre offset the app carries as
/// `centre_from_reference_px`. This walks one pixel forward to the four-dimensional point the
/// kernel is asked about and back through `plane_to_screen`, and states the two numbers the
/// browser published for this row, the scale mantissa and exponent, from the same `pixel_scale`
/// input.
///
/// What it can and cannot catch is worth being exact about. It inverts through the SAME centre it
/// added, so a wrong centre cancels and the round trip closes anyway: a one-pixel centre shift
/// survives this test. What it does catch is a wrong zoom, a wrong basis projection, and an
/// off-plane centre, none of which cancel. The centre itself is pinned by the flat-frame census in
/// [`the_oracle_reproduces_the_browser_s_census_of_the_flat_frame_and_the_upper_bands_of_the_row`],
/// which compares the records this centre selects against the served build's own counts.
#[test]
fn the_zoomed_row_s_sampling_map_round_trips_a_pixel_through_the_app_s_own_functions() {
    let pose = zoom_pose(3.565);
    let scale = pixel_scale(pose.zoom_log2, pose.grid_width).expect("row scale");
    let split = ember_julibrot_math::scaled_pixel_scale(pose.zoom_log2, pose.grid_width)
        .expect("row scale split");
    assert_eq!(
        split.exponent, -9,
        "the browser published scale_exponent -9"
    );
    assert!(
        (f64::from(split.mantissa) - 0.891_257_7).abs() <= 5.0e-8,
        "mantissa {} against the browser's 0.8912577",
        split.mantissa
    );

    let PoseMap::Mapped(map) = pose.map else {
        panic!("this row's pose is not edge-on");
    };
    let screen = pixel_screen(720, 200);
    let offset = map_plane_offset(&pose, screen).expect("a mapped pixel");
    let coordinate = [
        pose.centre_from_reference_px[0] + offset[0],
        pose.centre_from_reference_px[1] + offset[1],
    ];
    let point: [f64; 4] = core::array::from_fn(|axis| {
        scale.mul_add(
            f64::from(pose.plane.basis_u[axis]).mul_add(
                coordinate[0],
                f64::from(pose.plane.basis_v[axis]) * coordinate[1],
            ),
            pose.plane_origin[axis],
        )
    });
    // Back: project the four-dimensional point onto the plane basis, divide out the scale, remove
    // the centre, and ask the forward homography which pixel that plane offset belongs to.
    let delta: [f64; 4] = core::array::from_fn(|axis| point[axis] - pose.plane_origin[axis]);
    let recovered_coordinate = [
        delta
            .iter()
            .zip(pose.plane.basis_u)
            .fold(0.0, |sum, (value, basis)| {
                f64::from(basis).mul_add(*value, sum)
            })
            / scale,
        delta
            .iter()
            .zip(pose.plane.basis_v)
            .fold(0.0, |sum, (value, basis)| {
                f64::from(basis).mul_add(*value, sum)
            })
            / scale,
    ];
    let recovered_offset = [
        recovered_coordinate[0] - pose.centre_from_reference_px[0],
        recovered_coordinate[1] - pose.centre_from_reference_px[1],
    ];
    let recovered_screen = plane_to_screen(&map, recovered_offset).expect("a mapped plane offset");
    assert!(
        (recovered_screen[0] - screen[0]).hypot(recovered_screen[1] - screen[1]) <= 1.0e-6,
        "pixel (720,200) came back at {recovered_screen:?} from {screen:?}"
    );

    // The residual of the same displacement off the plane is at the plane's own rounding: the
    // basis is rounded once to binary32 by `construct_plane`'s contract, so a displacement of
    // magnitude one leaves about `f32::EPSILON` behind. The row's centre lies in the row's own
    // plane to that accuracy, which is why one basis projection recovers it.
    let residual: f64 = (0..4)
        .map(|axis| {
            let reconstructed = scale
                * f64::from(pose.plane.basis_u[axis]).mul_add(
                    recovered_coordinate[0],
                    f64::from(pose.plane.basis_v[axis]) * recovered_coordinate[1],
                );
            (delta[axis] - reconstructed).abs()
        })
        .fold(0.0, f64::max);
    assert!(
        residual <= 8.0 * f64::from(f32::EPSILON),
        "off-plane residual {residual}"
    );
}

const CAUSES: [Cause; 7] = [
    Cause::Sky,
    Cause::Interior,
    Cause::ExteriorFloor,
    Cause::LiftedEscape,
    Cause::Horizon,
    Cause::Uncertain,
    Cause::Malformed,
];

const CAUSE_NAMES: [&str; 7] = [
    "sky",
    "interior",
    "exteriorfloor",
    "lifted",
    "horizon",
    "uncertain",
    "malformed",
];

/// The four classes the browser census of the served frame was cut into.
///
/// The `dark` predicate is deliberately looser than the census's exact `(13,13,13)`: it admits any
/// pixel whose brightest channel is under a fifth, which is every colour this pass can emit for a
/// never-escaped record at any lighting term. The exact bytes the oracle does emit are reported
/// beside the counts, so the difference between the two is visible rather than assumed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Dark,
    PassClear,
    Exterior,
    Other,
}

const CLASS_NAMES: [&str; 4] = ["dark", "passclear", "exterior", "other"];

fn class_of(pixel: [u8; 3]) -> Class {
    if pixel[0] <= 51 && pixel[1] <= 51 && pixel[2] <= 51 {
        return Class::Dark;
    }
    if pixel == [255, 129, 129] {
        return Class::PassClear;
    }
    if background(pixel) {
        return Class::Exterior;
    }
    Class::Other
}

const CENSUS_BANDS: usize = 6;
const BAND_ROWS: usize = 90;

fn cause_bands(frame: &Frame) -> [[u64; 7]; CENSUS_BANDS] {
    let width = EXTENT[0] as usize;
    let mut table = [[0_u64; 7]; CENSUS_BANDS];
    for y in 0..EXTENT[1] as usize {
        for x in 0..width {
            let slot = CAUSES
                .iter()
                .position(|cause| *cause == frame.cause[y * width + x])
                .expect("every cause is named");
            table[y / BAND_ROWS][slot] += 1;
        }
    }
    table
}

fn class_bands(frame: &Frame) -> [[u64; 4]; CENSUS_BANDS] {
    let width = EXTENT[0] as usize;
    let mut table = [[0_u64; 4]; CENSUS_BANDS];
    for y in 0..EXTENT[1] as usize {
        for x in 0..width {
            let slot = match class_of(frame.colour[y * width + x]) {
                Class::Dark => 0,
                Class::PassClear => 1,
                Class::Exterior => 2,
                Class::Other => 3,
            };
            table[y / BAND_ROWS][slot] += 1;
        }
    }
    table
}

fn totals<const N: usize>(table: &[[u64; N]; CENSUS_BANDS]) -> [u64; N] {
    let mut sum = [0_u64; N];
    for band in table {
        for (slot, value) in band.iter().enumerate() {
            sum[slot] += value;
        }
    }
    sum
}

fn columns<T: core::fmt::Display>(values: impl IntoIterator<Item = T>) -> String {
    use core::fmt::Write as _;
    let mut line = String::new();
    for value in values {
        write!(line, "{value:>14}").expect("writing to a string cannot fail");
    }
    line
}

fn report_bands<const N: usize>(title: &str, names: [&str; N], table: &[[u64; N]; CENSUS_BANDS]) {
    let sum = totals(table);
    println!("{title}");
    println!("  band  {}", columns(names));
    for (index, band) in table.iter().enumerate() {
        println!("  {index:>4}  {}", columns(band));
    }
    println!("  all   {}", columns(sum));
}

fn top_colours(frame: &Frame, count: usize) -> Vec<([u8; 3], u64)> {
    let mut tally: std::collections::HashMap<[u8; 3], u64> = std::collections::HashMap::new();
    for pixel in &frame.colour {
        *tally.entry(*pixel).or_default() += 1;
    }
    let mut ordered: Vec<([u8; 3], u64)> = tally.into_iter().collect();
    ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ordered.truncate(count);
    ordered
}

const REASONS: [Reason; 8] = [
    Reason::Drawn,
    Reason::Flat,
    Reason::Horizon,
    Reason::Map,
    Reason::NearLimit,
    Reason::FivePole,
    Reason::FourPole,
    Reason::Observer,
];

const REASON_NAMES: [&str; 8] = [
    "drawn",
    "flat",
    "horizon",
    "map",
    "nearlimit",
    "fivepole",
    "fourpole",
    "observer",
];

/// Every gate of `scene_vertex` that decided a vertex, tallied over one grid.
fn vertex_reasons(pose: &Pose, records: &[[f32; 4]], mapping: Mapping) -> [u64; 8] {
    let [width, height] = EXTENT;
    let mut tally = [0_u64; 8];
    for row in 0..height {
        for column in 0..width {
            let record = records[(row * width + column) as usize];
            let vertex = scene_vertex(pose, column, row, record, Rule::Fixed, mapping);
            let slot = REASONS
                .iter()
                .position(|reason| *reason == vertex.reason)
                .expect("every gate is named");
            tally[slot] += 1;
        }
    }
    tally
}

fn census(name: &str, pose: &Pose, records: &[[f32; 4]], mapping: Mapping) -> Frame {
    let frame = render_frame(pose, records, Rule::Fixed, mapping);
    let base = render_frame(pose, records, Rule::Base, mapping);
    let width = EXTENT[0] as usize;
    let mut sky_from_near_limit = 0_u64;
    let mut sky_beyond = 0_u64;
    for index in 0..frame.covered.len() {
        if !frame.covered[index] {
            if base.covered[index] {
                sky_from_near_limit += 1;
            } else {
                sky_beyond += 1;
            }
        }
    }
    println!("== {name} ==");
    report_bands("cause per 90-row band", CAUSE_NAMES, &cause_bands(&frame));
    report_bands(
        "browser class per 90-row band",
        CLASS_NAMES,
        &class_bands(&frame),
    );
    println!(
        "  sky cleared by the near-limit refusal {sky_from_near_limit}, sky no primitive reaches at all {sky_beyond}"
    );
    let tally = vertex_reasons(pose, records, mapping);
    println!("  vertices of {}: {}", width * EXTENT[1] as usize, {
        use core::fmt::Write as _;
        let mut line = String::new();
        for (name, count) in REASON_NAMES.iter().zip(tally) {
            write!(line, "{name} {count} ").expect("writing to a string cannot fail");
        }
        line
    });
    println!("  top colours:");
    for (pixel, count) in top_colours(&frame, 6) {
        println!(
            "    ({:>3},{:>3},{:>3}) {count:>8}",
            pixel[0], pixel[1], pixel[2]
        );
    }
    frame
}

/// The zoomed row's frame, classified by cause at the three heights the browser census covers.
///
/// This is the instrument the evidence asked for: every pixel of the oracle's own frame carries
/// the reason the pass painted it, and the reasons are summed into the same six 90-row bands the
/// browser census was cut into. The assertions below fix only what the reading of the frame rests
/// on; the printed tables carry the rest and are quoted in `docs/julibrot/present.md`.
#[test]
fn the_zoomed_row_s_frame_is_classified_by_cause_at_the_three_census_heights() {
    let sampling = zoom_pose(3.565);
    let records = steep_records(&sampling);
    let mut interior_total = [0_u64; 3];
    for (slot, height_scale) in [3.565_f64, 1.0, 0.0].into_iter().enumerate() {
        let pose = zoom_pose(height_scale);
        let frame = census(
            &format!("zoom row, height {height_scale}"),
            &pose,
            &records,
            Mapping::InteriorAtFloor,
        );
        let bands = cause_bands(&frame);
        let sum = totals(&bands);
        interior_total[slot] = sum[1];
        assert_eq!(
            sum.iter().sum::<u64>(),
            u64::from(EXTENT[0]) * u64::from(EXTENT[1]),
            "every pixel carries exactly one cause"
        );
        // The record field is the same array at all three heights, so the number of pixels that
        // could show interior is fixed by the records; what moves is how many the mesh hides.
        assert!(sum[6] == 0, "no malformed record in this row: {}", sum[6]);
    }
    // The dark region is not a flat share of the frame: the height control moves it, which is what
    // a lifted surface does to what it hides and what it shows.
    assert!(
        interior_total[0] != interior_total[2],
        "the interior share moved between height 3.565 and height 0"
    );
}

/// The colour this pass emits for a never-escaped record, exactly, at this row's two lightings.
///
/// The census of the served frame reported 132,647 pixels of exactly `(13,13,13)` and read them as
/// the set interior. The interior colour of the Classic palette is `(0.005, 0.005, 0.008)` linear,
/// so whatever the lighting term does to it, the blue channel comes out strictly above the red and
/// green, which are equal. A neutral grey is therefore not a colour the scene pass can emit for an
/// interior record, and this test states the bytes it does emit instead.
#[test]
fn the_interior_colour_this_pass_emits_is_not_neutral_grey() {
    let flat = zoom_pose(0.0);
    let records = steep_records(&flat);
    let interior = records
        .iter()
        .copied()
        .find(|record| record[1] == 0.0 && record[0] == -1.0 && record[3] == 0.0)
        .expect("this row's slice has interior records");
    let frame = render_frame(&flat, &records, Rule::Fixed, Mapping::InteriorAtFloor);
    let painted: Vec<[u8; 3]> = frame
        .cause
        .iter()
        .zip(&frame.colour)
        .filter(|(cause, _)| **cause == Cause::Interior)
        .map(|(_, colour)| *colour)
        .collect();
    assert!(!painted.is_empty(), "the flat frame draws interior records");
    let first = painted[0];
    assert!(
        painted.iter().all(|pixel| *pixel == first),
        "a flat chart lights every interior fragment the same way"
    );
    assert_eq!(
        first[0], first[1],
        "the interior colour's red and green are equal"
    );
    assert!(
        first[2] > first[0],
        "and its blue is strictly higher: {first:?}"
    );
    assert_eq!(
        first,
        [12, 12, 17],
        "the flat frame's interior fragment, at the shader's own fallback normal"
    );
    println!("flat interior fragment {first:?} from record {interior:?}");
    let lifted = zoom_pose(3.565);
    let lifted_frame = render_frame(&lifted, &records, Rule::Fixed, Mapping::InteriorAtFloor);
    let lifted_painted: Vec<[u8; 3]> = lifted_frame
        .cause
        .iter()
        .zip(&lifted_frame.colour)
        .filter(|(cause, _)| **cause == Cause::Interior)
        .map(|(_, colour)| *colour)
        .collect();
    // Under the row's own height the interior is still one flat slab, but the triangles that
    // resolve to an interior record are not all in it: a cell straddling the set boundary carries
    // one floor vertex and one lifted vertex, and its fragments take that cliff's normal while
    // still reading the interior record. So the interior appears at several lighting terms, and
    // every one of them keeps red equal to green and blue strictly above both.
    let mut lighting_terms: Vec<[u8; 3]> = lifted_painted;
    lighting_terms.sort_unstable();
    lighting_terms.dedup();
    for pixel in &lighting_terms {
        assert_eq!(
            pixel[0], pixel[1],
            "interior red and green differ: {pixel:?}"
        );
        assert!(
            pixel[2] > pixel[0],
            "interior blue is not highest: {pixel:?}"
        );
        assert_ne!(*pixel, [13, 13, 13], "a neutral grey interior fragment");
    }
    println!(
        "lifted interior fragments over {} lighting terms: {lighting_terms:?}",
        lighting_terms.len()
    );
}

/// The boundary is a cliff on the screen and a small step on the chart, and this measures both.
///
/// The height law spans `[-2, 2]`, so a never-escaped record at the floor and a record surviving
/// to the iteration cap at the peak would stand `height_scale * 2` chart units apart. That is the
/// law's range and not this row's step: at this zoom the escaped neighbours of the interior are
/// the SHALLOWEST escapes, which the law also places near the floor. The test measures the actual
/// chart heights of the escaped side of every horizontal boundary pair and pins their quartiles.
///
/// The screen separation is a different quantity and is large: the fifth perspective divides by
/// `d5 - ambient.fifth`, so a small chart step near the near limit is magnified into tens or
/// hundreds of pixels. What the picture shows as a wall is that magnification acting on a step of
/// a fraction of a unit, not a full-range discontinuity.
#[test]
fn the_set_boundary_is_a_cliff_whose_screen_height_this_row_can_be_measured_in() {
    let pose = zoom_pose(3.565);
    let records = steep_records(&pose);
    let [width, height] = EXTENT;
    let interior = |record: [f32; 4]| record[3] == 0.0 && record[1] == 0.0 && record[0] == -1.0;
    let mut separations: Vec<f64> = Vec::new();
    let mut escaped_heights: Vec<f64> = Vec::new();
    let mut pairs = 0_u64;
    let mut refused = 0_u64;
    let mut example: Option<(u32, u32, f64, f64, f64)> = None;
    for row in 0..height {
        for column in 0..width - 1 {
            let left = records[(row * width + column) as usize];
            let right = records[(row * width + column + 1) as usize];
            if interior(left) == interior(right) {
                continue;
            }
            pairs += 1;
            escaped_heights.push(record_height(if interior(left) { right } else { left }));
            let (floor_column, floor_record, lifted_column, lifted_record) = if interior(left) {
                (column, left, column + 1, right)
            } else {
                (column + 1, right, column, left)
            };
            let floor_vertex = scene_vertex(
                &pose,
                floor_column,
                row,
                floor_record,
                Rule::Fixed,
                Mapping::InteriorAtFloor,
            );
            let lifted_vertex = scene_vertex(
                &pose,
                lifted_column,
                row,
                lifted_record,
                Rule::Fixed,
                Mapping::InteriorAtFloor,
            );
            if !lifted_vertex.valid {
                refused += 1;
                continue;
            }
            if !floor_vertex.valid {
                continue;
            }
            let separation =
                (lifted_vertex.x - floor_vertex.x).hypot(lifted_vertex.y - floor_vertex.y);
            separations.push(separation);
            if example.is_none() && separation > 40.0 {
                example = Some((
                    floor_column,
                    row,
                    separation,
                    record_height(floor_record),
                    record_height(lifted_record),
                ));
            }
        }
    }
    assert!(pairs > 0, "this row's slice has a set boundary");
    separations.sort_by(f64::total_cmp);
    let median = separations[separations.len() / 2];
    let largest = *separations.last().expect("a measured pair");
    let refused_share = refused as f64 / pairs as f64;
    println!(
        "boundary pairs {pairs}, near-limit refusals {refused} ({refused_share:.4}), screen separation median {median:.2} px, largest {largest:.2} px"
    );
    // The chart step across the same pairs. The height law's range is four units; this row uses a
    // small part of it at the boundary, because the escaped neighbours of the interior here are
    // the shallowest escapes and the law places those next to the floor as well.
    escaped_heights.sort_by(f64::total_cmp);
    let quantile = |fraction: f64| {
        escaped_heights[((escaped_heights.len() - 1) as f64 * fraction).round() as usize]
    };
    let above_midline = escaped_heights.iter().filter(|value| **value > 0.0).count();
    println!(
        "escaped-side chart height over {} pairs: min {:.3}, p25 {:.3}, median {:.3}, p75 {:.3}, max {:.3}; above the mid-line {above_midline}",
        escaped_heights.len(),
        escaped_heights[0],
        quantile(0.25),
        quantile(0.5),
        quantile(0.75),
        escaped_heights[escaped_heights.len() - 1]
    );
    assert!(
        quantile(0.5) < -1.5,
        "this row's boundary step is small, not the law's full range: median {}",
        quantile(0.5)
    );
    assert!(
        above_midline * 10 < escaped_heights.len(),
        "{above_midline} of {} escaped neighbours stand above the mid-line",
        escaped_heights.len()
    );
    if let Some((column, row, separation, floor_height, lifted_height)) = example {
        println!(
            "one measured pair at column {column} row {row}: record heights {floor_height:.3} and {lifted_height:.3}, {separation:.2} screen pixels apart"
        );
    }
    // A step of about a tenth of a unit on the chart, and tens of pixels on the screen: the gap
    // between the two is the fifth perspective's magnification, not the height law's range.
    assert!(
        median > 20.0,
        "the screen separation is the magnified step: median {median}"
    );
}

/// What the alternative height mapping would change in this frame, measured and not changed.
///
/// Placing a never-escaped record at the peak instead of the floor makes the interior continuous
/// with the deepest escapes: the surface then has no cliff at the set boundary at all, because the
/// records on both sides of it are the ones the law places highest. The shader keeps the shipped
/// law; this renders the same record field under both and prints the two censuses so the choice
/// can be argued from numbers.
#[test]
fn the_alternative_height_mapping_is_measured_here_and_not_adopted() {
    let pose = zoom_pose(3.565);
    let records = steep_records(&pose);
    let shipped = census(
        "zoom row, height 3.565, interior at the floor",
        &pose,
        &records,
        Mapping::InteriorAtFloor,
    );
    let alternative = census(
        "zoom row, height 3.565, interior at the peak",
        &pose,
        &records,
        Mapping::InteriorAtPeak,
    );
    let shipped_bands = totals(&cause_bands(&shipped));
    let alternative_bands = totals(&cause_bands(&alternative));
    println!(
        "interior pixels {} -> {}, sky {} -> {}",
        shipped_bands[1], alternative_bands[1], shipped_bands[0], alternative_bands[0]
    );
    let differing = shipped
        .colour
        .iter()
        .zip(&alternative.colour)
        .filter(|(left, right)| left != right)
        .count();
    println!(
        "the two mappings differ on {differing} of {} pixels",
        EXTENT[0] as usize * EXTENT[1] as usize
    );
    assert!(
        differing > 0,
        "the mapping is a real choice in this frame, not a no-op"
    );
}

/// The near limit cuts this frame by position on the chart, not by record height.
///
/// The five-dimensional camera is a general rotation, so the fifth coordinate it measures the near
/// limit against is a combination of the four chart coordinates and the lifted height. In this
/// row's camera the height enters that combination with a NEGATIVE weight: lifting a sample moves
/// it away from the near limit rather than into it, so no amount of height can push a sample past
/// the limit that its chart position had not already put there. The measurement below is the
/// consequence — the refused set is the same 79,350 vertices at height 1.0, at height 3.565, and
/// under the alternative height mapping, to within four vertices. It is therefore a region of the
/// CHART, near enough a half-plane; a projective map takes a line to a line; and the boundary of
/// the refusal reads as a straight edge across the frame rather than as anything the escape
/// records drew. That straight edge is the slab boundary a reader of this frame sees.
#[test]
fn the_near_limit_cuts_this_row_by_chart_position_rather_than_by_height() {
    let pose = zoom_pose(3.565);
    let flat_fifth = ambient_camera([0.0, 0.0, 0.0, 0.0, 0.0], &pose.view)[4];
    let raised_fifth = ambient_camera([0.0, 0.0, 0.0, 0.0, 1.0], &pose.view)[4];
    let height_coefficient = raised_fifth - flat_fifth;
    let chart_coefficients: [f64; 4] = core::array::from_fn(|axis| {
        let mut point = [0.0; 5];
        point[axis] = 1.0;
        ambient_camera(point, &pose.view)[4] - flat_fifth
    });
    println!(
        "fifth coordinate per unit of height {height_coefficient:.6}, per unit of chart axis {chart_coefficients:?}"
    );
    let records = steep_records(&pose);
    let refusals: Vec<u64> = [3.565_f64, 1.0]
        .into_iter()
        .map(|height_scale| {
            vertex_reasons(&zoom_pose(height_scale), &records, Mapping::InteriorAtFloor)[4]
        })
        .collect();
    let peak = vertex_reasons(&pose, &records, Mapping::InteriorAtPeak)[4];
    println!(
        "near-limit refusals: height 3.565 {}, height 1.0 {}, interior at the peak {peak}",
        refusals[0], refusals[1]
    );
    assert!(
        height_coefficient < 0.0,
        "lifting a sample must recede from the near limit, not approach it: {height_coefficient}"
    );
    assert!(
        chart_coefficients[0].abs() > 3.0 * height_coefficient.abs(),
        "the leading chart weight {} against the height's {height_coefficient}",
        chart_coefficients[0]
    );
    let spread = refusals[0].abs_diff(refusals[1]);
    assert!(
        spread <= 10,
        "a 3.5-fold height change moved the refused set by {spread} vertices"
    );
    assert_eq!(
        peak, refusals[0],
        "and the alternative height mapping moves it not at all"
    );
}

/// The two record-driven shares a page fact could publish for this row, measured.
///
/// `surface_uncovered_fraction` is a statement about the MESH at five census heights and reports
/// 0.042 here. Neither of the numbers below is that number, and neither is derivable from it: they
/// are counts over the record field and over the frame the record field produced.
#[test]
fn the_record_driven_shares_this_row_would_publish() {
    let pose = zoom_pose(3.565);
    let records = steep_records(&pose);
    let total = f64::from(EXTENT[0]) * f64::from(EXTENT[1]);
    let interior_records = records
        .iter()
        .filter(|record| record[3] == 0.0 && record[1] == 0.0 && record[0] == -1.0)
        .count();
    let horizon_records = records.iter().filter(|record| record[3] == 2.0).count();
    let frame = render_frame(&pose, &records, Rule::Fixed, Mapping::InteriorAtFloor);
    let bands = totals(&cause_bands(&frame));
    let refused = vertex_reasons(&pose, &records, Mapping::InteriorAtFloor)[4];
    println!(
        "interior records {interior_records} ({:.4} of the field), horizon records {horizon_records} ({:.4})",
        interior_records as f64 / total,
        horizon_records as f64 / total
    );
    println!(
        "dark share of the frame {:.4} ({} pixels), sky share {:.4} ({} pixels), refused share of the mesh {:.4} ({refused} vertices)",
        bands[1] as f64 / total,
        bands[1],
        bands[0] as f64 / total,
        bands[0],
        refused as f64 / total
    );
    assert!(
        (bands[1] as f64 / total) > 0.2,
        "a quarter of this frame is the set interior"
    );
}

/// Where this oracle agrees with the browser's census of the served frame, and where it does not.
///
/// The flat frame is the strongest calibration there is for this instrument: at height zero every
/// vertex takes the direct path, the mesh tiles the frame it was sampled for, and the picture is
/// the two-dimensional chart with nothing projected. The browser counted 108,732 pixels in its
/// dark class, 75,895 of them in the first 90-row band and 32,837 in the second and none below;
/// exactly 17,556 in the pass-clear class, in the bottom band only. This oracle's interior cause
/// reproduces all four numbers to within five pixels, which identifies the dark class as the set
/// interior of this slice beyond argument. The class split of the same frame agrees to four pixels
/// on the exterior class and one on the relief class, which is a second and independent check on
/// the fragment lighting term, since that term is what decides which side of the census's exterior
/// predicate a palette colour lands on.
///
/// Under the row's own height the agreement holds over the first four bands and breaks in the last
/// two, where this oracle clears 25,393 pixels the served frame paints. Those pixels are outside
/// what this instrument can speak for: it models the scene pass alone, and the served frame has a
/// backdrop grid drawn behind it at a wider apron. Any verdict this file supports is a verdict
/// about the upper four bands.
#[test]
fn the_oracle_reproduces_the_browser_s_census_of_the_flat_frame_and_the_upper_bands_of_the_row() {
    let records = steep_records(&zoom_pose(3.565));
    let flat = render_frame(
        &zoom_pose(0.0),
        &records,
        Rule::Fixed,
        Mapping::InteriorAtFloor,
    );
    let flat_causes = cause_bands(&flat);
    let flat_total = totals(&flat_causes);
    assert!(
        flat_total[1].abs_diff(108_732) <= 8,
        "flat interior {} against the browser's dark class 108,732",
        flat_total[1]
    );
    assert!(
        flat_causes[0][1].abs_diff(75_895) <= 8,
        "flat interior band 0 {}",
        flat_causes[0][1]
    );
    assert!(
        flat_causes[1][1].abs_diff(32_837) <= 8,
        "flat interior band 1 {}",
        flat_causes[1][1]
    );
    assert_eq!(
        flat_causes[2][1] + flat_causes[3][1] + flat_causes[4][1] + flat_causes[5][1],
        0,
        "the browser saw no dark pixel below row 180 in the flat frame"
    );
    let flat_classes = totals(&class_bands(&flat));
    assert_eq!(
        flat_classes[1], 17_556,
        "the flat frame's pass-clear class is the horizon record count exactly"
    );
    // The two halves of that total are pinned separately, because their split is what the
    // fragment lighting term decides. Standing a saturated term in for the shader's fallback
    // normal put 5,291 pixels on the wrong side of the census's exterior predicate while leaving
    // the sum untouched; with the shader's own fallback the two agree with the served build to
    // four pixels and to one.
    assert!(
        flat_classes[2].abs_diff(384_588) <= 8,
        "flat exterior {} against the browser's 384,588",
        flat_classes[2]
    );
    assert!(
        flat_classes[3].abs_diff(7_524) <= 8,
        "flat other {} against the browser's 7,524",
        flat_classes[3]
    );

    let lifted = render_frame(
        &zoom_pose(3.565),
        &records,
        Rule::Fixed,
        Mapping::InteriorAtFloor,
    );
    let lifted_classes = class_bands(&lifted);
    let upper = |slot: usize| -> u64 { (0..4).map(|band| lifted_classes[band][slot]).sum() };
    for (slot, browser, name) in [
        (0_usize, 128_743_u64, "dark"),
        (2, 188_433, "exterior"),
        (3, 28_424, "other"),
    ] {
        let measured = upper(slot);
        let apart = f64::from(u32::try_from(measured.abs_diff(browser)).expect("small difference"))
            / f64::from(u32::try_from(browser).expect("browser count fits"));
        println!(
            "upper four bands, {name}: oracle {measured}, browser {browser}, {apart:.4} apart"
        );
        assert!(
            apart <= 0.03,
            "{name} in the upper four bands is {apart} apart from the browser"
        );
    }
    let lower_clear: u64 = (4..6).map(|band| lifted_classes[band][1]).sum();
    println!(
        "lower two bands, pass clear: oracle {lower_clear}, browser 69,739 — pixels the oracle does not model"
    );
    assert!(
        lower_clear > 69_739,
        "the mirror clears more of the lower bands than the served frame, never less"
    );
}

/// The curtain at the right edge of this frame, measured the way the browser census measured it.
///
/// The census walked 120 columns at an eight-pixel stride, 270 samples down each at a two-pixel
/// stride, and counted how often the relief classification flipped. It found 29 columns with six
/// or more flips and a band at x 800..930 carrying 12 to 36 flips per column, everything left of
/// x 792 at most six. This reproduces that walk over the oracle's own frame and prints the cause
/// mix inside and outside that band, which is what says WHAT the curtain is made of.
#[test]
fn the_right_hand_curtain_is_lifted_escaped_records_and_the_census_walk_finds_it_here_too() {
    let pose = zoom_pose(3.565);
    let records = steep_records(&pose);
    let frame = render_frame(&pose, &records, Rule::Fixed, Mapping::InteriorAtFloor);
    let width = EXTENT[0] as usize;
    let relief = |index: usize| class_of(frame.colour[index]) == Class::Other;
    let mut busy = 0_u32;
    let mut band_low = u32::MAX;
    let mut band_high = 0_u32;
    let mut band_flips: Vec<u32> = Vec::new();
    for x in (0..width).step_by(8) {
        let mut flips = 0_u32;
        let mut previous = relief(x);
        for y in (2..EXTENT[1] as usize).step_by(2) {
            let current = relief(y * width + x);
            if current != previous {
                flips += 1;
                previous = current;
            }
        }
        if flips >= 6 {
            busy += 1;
            band_low = band_low.min(u32::try_from(x).expect("column fits"));
            band_high = band_high.max(u32::try_from(x).expect("column fits"));
        }
        if (800..=930).contains(&x) {
            band_flips.push(flips);
        }
    }
    let inside = |index: usize| (800..=930).contains(&(index % width));
    let mut inside_causes = [0_u64; 7];
    let mut outside_causes = [0_u64; 7];
    for (index, cause) in frame.cause.iter().enumerate() {
        let slot = CAUSES
            .iter()
            .position(|candidate| candidate == cause)
            .expect("every cause is named");
        if inside(index) {
            inside_causes[slot] += 1;
        } else {
            outside_causes[slot] += 1;
        }
    }
    println!(
        "columns with six or more relief flips: {busy} of 120, spanning x {band_low}..{band_high}; flips inside x 800..930 {band_flips:?}"
    );
    println!("cause inside x 800..930:  {}", columns(inside_causes));
    println!("cause outside x 800..930: {}", columns(outside_causes));
    let inside_lifted = inside_causes[3] as f64 / inside_causes.iter().sum::<u64>() as f64;
    let outside_lifted = outside_causes[3] as f64 / outside_causes.iter().sum::<u64>() as f64;
    println!("lifted share inside {inside_lifted:.4}, outside {outside_lifted:.4}");
    assert!(
        inside_lifted > 2.0 * outside_lifted,
        "the curtain band must be made of lifted escaped records: {inside_lifted} against {outside_lifted}"
    );
}

/// No pixel of this frame is painted with a record that is not its own surface point's.
///
/// This is the test the verdict on the row rests on. A settled frame may not assert geometry the
/// object does not have, and the way this pass could break that rule without any gate refusing
/// anything is to paint a pixel with the wrong record: the fragment stage resolves a pixel to a
/// sample by rounding the interpolated grid coordinate, and a primitive stretched across the frame
/// by the fifth perspective interpolates that coordinate over a long screen distance. The bound is
/// that the interpolant is a convex combination of the primitive's own three grid coordinates, so
/// the sample it rounds to is a corner of the primitive's own cell whatever the projection did to
/// the primitive's shape. The counter below is incremented wherever that fails, at every height
/// the census covers and under both height mappings, and is zero.
#[test]
fn no_pixel_of_this_row_is_painted_with_a_record_from_outside_its_own_cell() {
    let records = steep_records(&zoom_pose(3.565));
    for height_scale in [3.565_f64, 1.0, 0.0] {
        for mapping in [Mapping::InteriorAtFloor, Mapping::InteriorAtPeak] {
            for rule in [Rule::Fixed, Rule::Base] {
                let frame = render_frame(&zoom_pose(height_scale), &records, rule, mapping);
                assert_eq!(
                    frame.foreign, 0,
                    "height {height_scale} painted {} pixels from outside their own cell",
                    frame.foreign
                );
            }
        }
    }
}

/// The zoomed row uploads the accepted steep row's scene payload byte for byte.
///
/// The claim the whole lane rests on is that the two rows share a mesh, so that every difference
/// between their frames is a difference in the records. That is an argument from two contracts —
/// the screen map is zoom-free, and the centre enters only where a pixel becomes a point — and
/// this is the mechanical check of it: `SceneUniform::new` is present's own packer, and the bytes
/// it produces for the two poses are identical. The one lane that could carry a zoom is the apron
/// in `screen_to_plane_row_2.w`, which is one for both.
#[test]
fn the_zoomed_row_uploads_the_steep_row_s_scene_payload_unchanged() {
    let scene_for = |pose: &Pose| {
        SceneUniform::new(
            [pose.grid_width, pose.grid_height],
            3,
            CAP,
            0,
            pose.grid_width * pose.grid_height,
            pose.plane,
            pose.map,
            CLASSIC_PALETTE,
        )
        .expect("the row's map packs into the scene payload")
    };
    let steep = scene_for(&pose_with(steep_view(3.565)));
    let zoomed = scene_for(&zoom_pose(3.565));
    assert_eq!(steep.span[2], 0, "the row's map is not edge-on");
    assert_eq!(zoomed.screen_to_plane_row_2[3], 1.0, "apron one");
    assert_eq!(
        bytemuck::bytes_of(&steep),
        bytemuck::bytes_of(&zoomed),
        "the zoom and the centre reached the scene payload"
    );
    assert_ne!(
        pose_with(steep_view(3.565)).centre_from_reference_px,
        zoom_pose(3.565).centre_from_reference_px,
        "the two poses really do differ where the records are sampled"
    );
}
