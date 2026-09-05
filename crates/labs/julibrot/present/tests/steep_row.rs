//! The scene placement invariant at a fully rotated five-dimensional pose.
//!
//! The row reproduced here is a saved view whose settled Final frame shows a curtain of vertical
//! streaks and a flat slab: a mixed slice, all ten camera angles nonzero, height scale 3.565, both
//! perspective poles at eight. A settled frame may not assert geometry the object does not have,
//! so every drawn vertex has to be placed by the one projection the pose defines.

use ember_julibrot_math::{
    ObjectAngles, Pose, PoseMap, ViewControls, construct_plane, screen_to_plane,
};
use ember_julibrot_present::{grid_screen, project_scene_point};

const EXTENT: [u32; 2] = [960, 540];
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

fn steep_object() -> ObjectAngles {
    ObjectAngles {
        rho_13: OBJECT_ANGLE,
        rho_24: OBJECT_ANGLE,
        ..ObjectAngles::IDENTITY
    }
}

fn steep_view() -> ViewControls {
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
    [
        grid_screen(column, EXTENT[0]),
        grid_screen(row, EXTENT[1]),
    ]
}

/// Two records one iteration apart on the set boundary have all but identical relief height.
///
/// An in-set sample's height is the chart floor `-2`, and so is an escaped sample that left the
/// disc on its first iteration. Their vertices are therefore the same point of the same surface up
/// to a millionth of the height domain, and no projection of that surface can separate them by
/// more than a fraction of a cell.
#[test]
fn an_in_set_vertex_shares_its_neighbour_s_projection_at_the_steep_row() {
    let pose = pose_with(steep_view());
    let screen = sample_screen(EXTENT[0] / 2, EXTENT[1] / 2);
    let interior = project_scene_point(&pose, screen, -2.0).expect("the interior vertex projects");
    let escaped =
        project_scene_point(&pose, screen, -2.0 + 1.0e-6).expect("the escaped vertex projects");
    let separation = (interior[0] - escaped[0]).hypot(interior[1] - escaped[1]);
    assert!(
        separation <= 1.0,
        "an in-set vertex and its escaped neighbour of the same height are {separation} px apart at the steep row; interior {interior:?} escaped {escaped:?}"
    );
}
