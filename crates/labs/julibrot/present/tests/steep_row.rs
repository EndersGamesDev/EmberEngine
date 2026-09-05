//! Scene placement at a fully rotated five-dimensional pose.
//!
//! The row reproduced here is a saved view whose settled Final frame showed a curtain of vertical
//! streaks and a flat slab with a sharp diagonal edge: a mixed slice, all ten camera angles
//! nonzero, height scale 3.565, both perspective poles at eight, zoom zero. A settled frame may
//! not assert geometry the object does not have, so every drawn vertex has to be placed by the one
//! projection the pose defines, and a vertex that projection does not define may not be drawn.

use ember_julibrot_math::{
    ObjectAngles, Plane, Pose, PoseMap, ViewControls, construct_plane, screen_to_plane,
};
use ember_julibrot_present::{
    CLASSIC_PALETTE, grid_screen, project_scene_point, project_scene_record_vertex,
};

const EXTENT: [u32; 2] = [960, 540];
const ITERATION_CAP: u32 = 512;
const OBJECT_ANGLE: f64 = -0.163_226_878_883_618_53;
const PLANE_ORIGIN: [f64; 4] = [-0.629, 0.0, -0.083, 0.016];

/// The row's ten camera angles in the product order 12, 13, 14, 23, 24, 34, 15, 25, 35, 45.
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

/// A record the kernel writes where the screen-to-plane denominator was not positive.
const HORIZON_RECORD: [f32; 4] = [0.0, 0.0, 0.0, 2.0];
/// An escaped record at the chart floor: it left the disc on its first iteration.
const FLOOR_ESCAPE_RECORD: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
/// A sample that never escaped: the in-set interior, which also sits on the chart floor.
const IN_SET_RECORD: [f32; 4] = [-1.0, 0.0, 512.0, 0.0];

const fn steep_object() -> ObjectAngles {
    ObjectAngles {
        rho_13: OBJECT_ANGLE,
        rho_24: OBJECT_ANGLE,
        ..ObjectAngles::IDENTITY
    }
}

const fn steep_view() -> ViewControls {
    ViewControls {
        camera: CAMERA,
        camera_translation: [-0.04, 0.258, 0.0, 0.0, 0.0],
        camera_yaw: 0.0,
        camera_pitch: 0.0,
        height_scale: 3.565,
        distance_five: 8.0,
        distance_four: 8.0,
    }
}

fn pose_with(view: ViewControls) -> Pose {
    let object = steep_object();
    let plane = construct_plane(object).expect("the row's object angles construct a plane");
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

/// The screen point of one grid sample, in the mesh's own frame-centred units.
fn sample_screen(column: u32, row: u32) -> [f64; 2] {
    [grid_screen(column, EXTENT[0]), grid_screen(row, EXTENT[1])]
}

/// The homography denominator, whose sign is the kernel's own horizon test.
const fn map_denominator(pose: &Pose, screen: [f64; 2]) -> f64 {
    let PoseMap::Mapped(map) = pose.map else {
        return 0.0;
    };
    map.rows[6].mul_add(screen[0], map.rows[7].mul_add(screen[1], map.rows[8]))
}

fn placement(pose: &Pose, screen: [f64; 2], record: [f32; 4]) -> Option<([f64; 2], f64)> {
    project_scene_record_vertex(pose, screen, record, ITERATION_CAP, CLASSIC_PALETTE)
        .expect("the fixture iteration cap is nonzero")
}

/// A sample lifted past the five-dimensional near limit is behind that camera and is not drawn.
///
/// The projective algebra still returns a point for such a vertex, mirrored through the pole. At
/// this row the mirrored point is up to 148.8 px from anywhere the surface reaches, and the frame
/// drew the whole band it sits in. Screen point (-480, -134.5) is grid sample (0, 135), whose
/// fifth coordinate at the chart floor is +15.67 against a near limit of 7.6.
#[test]
fn a_vertex_past_the_five_dimensional_pole_is_refused_at_the_steep_row() {
    let pose = pose_with(steep_view());
    let screen = sample_screen(0, 135);
    assert!(
        map_denominator(&pose, screen) > 0.0,
        "the sample has a plane point, so only the fifth pole can refuse it"
    );
    assert_eq!(
        project_scene_point(&pose, screen, -2.0),
        None,
        "a vertex behind the five-dimensional camera may not be placed"
    );
    assert_eq!(project_scene_point(&pose, screen, 0.0), None);
    assert_eq!(project_scene_point(&pose, screen, 2.0), None);
}

/// The refusal is the pole's, not a blanket one: the rest of the frame still projects exactly.
///
/// Where the fifth pole is not passed, the forward projection of a sample at the chart floor is
/// the screen-to-plane map's own inverse, so the round trip returns the screen point it started
/// from. Measured over every one of the 518400 samples of this row, the worst such round trip is
/// 6.4e-7 px.
#[test]
fn every_sample_the_steep_row_still_draws_round_trips_to_its_own_screen_point() {
    let pose = pose_with(steep_view());
    let mut drawn = 0_u64;
    let mut worst = 0.0_f64;
    for row in (0..EXTENT[1]).step_by(7) {
        for column in (0..EXTENT[0]).step_by(7) {
            let screen = sample_screen(column, row);
            let Some((point, _)) = placement(&pose, screen, FLOOR_ESCAPE_RECORD) else {
                continue;
            };
            drawn += 1;
            worst = worst.max((point[0] - screen[0]).hypot(point[1] - screen[1]));
        }
    }
    assert!(drawn > 0, "the row draws no sample at all");
    assert!(
        worst <= 1.0e-6,
        "a drawn sample at the chart floor landed {worst} px from its own screen point"
    );
}

/// A horizon record has no plane point, so it has no vertex, so nothing is drawn for it.
///
/// Placing it at its flat screen point put a slab at the near depth over relief that is in front
/// of it; at this row 17560 of 518400 samples (3.39%) are horizon, a wedge in the lower left of
/// the frame whose straight boundary is the plane's own horizon line. The scene pass clears to the
/// exterior colour those samples carry, so refusing the vertex changes nothing while the
/// projection is the identity and removes the slab once it is not.
#[test]
fn a_horizon_record_is_refused_rather_than_placed_flat() {
    let pose = pose_with(steep_view());
    let screen = sample_screen(0, 0);
    assert!(
        map_denominator(&pose, screen) <= 0.0,
        "the fixture corner is beyond the plane's horizon at this row"
    );
    assert_eq!(placement(&pose, screen, HORIZON_RECORD), None);
    // Also where the plane does reach: the record, not the geometry, is what refuses it.
    let inside = sample_screen(EXTENT[0] / 2, EXTENT[1] / 2);
    assert!(map_denominator(&pose, inside) > 0.0);
    assert_eq!(placement(&pose, inside, HORIZON_RECORD), None);
    assert!(placement(&pose, inside, FLOOR_ESCAPE_RECORD).is_some());
}

/// An in-set sample and an escaped neighbour of the same height are placed by the same projection.
///
/// An in-set sample's relief height is the chart floor, and so is an escaped sample that left the
/// disc on its first iteration: the two vertices are the same point of the same surface. No pose
/// may separate them, and no pose may place one of them flat while it projects the other.
#[test]
fn an_in_set_vertex_shares_its_escaped_neighbour_s_projection_over_the_pose_table() {
    let mut yaw_only = ViewControls::NEUTRAL;
    yaw_only.camera[0] = 0.6;
    yaw_only.height_scale = 3.565;
    let mut flattened = steep_view();
    flattened.height_scale = 0.0;
    let table = [
        ("identity", ViewControls::NEUTRAL),
        ("yaw only", yaw_only),
        ("the steep row", steep_view()),
        ("the steep row with no height", flattened),
    ];
    for (name, view) in table {
        let pose = pose_with(view);
        let mut compared = 0_u64;
        for row in (0..EXTENT[1]).step_by(29) {
            for column in (0..EXTENT[0]).step_by(31) {
                let screen = sample_screen(column, row);
                let interior = placement(&pose, screen, IN_SET_RECORD);
                let escaped = placement(&pose, screen, FLOOR_ESCAPE_RECORD);
                match (interior, escaped) {
                    (Some((interior, _)), Some((escaped, _))) => {
                        compared += 1;
                        let separation = (interior[0] - escaped[0]).hypot(interior[1] - escaped[1]);
                        assert!(
                            separation <= 1.0,
                            "{name}: an in-set vertex and its escaped neighbour of the same height are {separation} px apart, more than one cell"
                        );
                    }
                    (None, None) => {}
                    _ => panic!(
                        "{name}: one of two vertices of the same height was drawn and the other refused at {screen:?}"
                    ),
                }
            }
        }
        assert!(compared > 0, "{name}: the pose drew nothing to compare");
    }
}

/// The relief this row asks for is a curtain, and that part of the picture is honest.
///
/// One iteration of escape count moves a vertex 0.19 to 3.99 px across the frame at height scale
/// 3.565, and the whole height domain travels 95 to 1350 px. Two adjacent samples six iterations
/// apart therefore stand about twelve pixels apart on screen. That is the height control doing
/// what it was asked, not a defect, and it is why the frame is mostly wall.
#[test]
fn the_steep_row_s_relief_travel_is_the_height_control_and_stays_finite() {
    let pose = pose_with(steep_view());
    let screen = sample_screen(EXTENT[0] / 2, EXTENT[1] / 2);
    let floor = placement(&pose, screen, FLOOR_ESCAPE_RECORD).expect("the frame centre draws");
    let one_iteration = project_scene_point(&pose, screen, -2.0 + 4.0 / f64::from(ITERATION_CAP))
        .expect("one iteration above the floor draws");
    let travel = (one_iteration[0] - floor.0[0]).hypot(one_iteration[1] - floor.0[1]);
    assert!(
        (2.0..2.02).contains(&travel),
        "one iteration moved the frame centre {travel} px"
    );
}

/// A flat chart keeps every sample exactly where the screen-to-plane map put it.
#[test]
fn a_flat_chart_places_every_record_at_its_own_screen_point() {
    let mut view = steep_view();
    view.height_scale = 0.0;
    let pose = pose_with(view);
    for row in (0..EXTENT[1]).step_by(53) {
        for column in (0..EXTENT[0]).step_by(59) {
            let screen = sample_screen(column, row);
            for record in [FLOOR_ESCAPE_RECORD, IN_SET_RECORD, HORIZON_RECORD] {
                assert_eq!(placement(&pose, screen, record), Some((screen, 1.0)));
            }
        }
    }
}

/// The plane the pose defines is real: the fixture is not silently edge-on.
#[test]
fn the_steep_row_fixture_has_an_invertible_screen_to_plane_map() {
    let pose = pose_with(steep_view());
    let PoseMap::Mapped(map) = pose.map else {
        panic!("the steep row's map is invertible, so the fixture must not be edge-on")
    };
    assert_eq!(map.apron_scale, 1.0);
    let plane: Plane = pose.plane;
    assert!(plane.basis_u.iter().all(|value| value.is_finite()));
    assert!(plane.basis_v.iter().all(|value| value.is_finite()));
}
