#![allow(
    clippy::suboptimal_flops,
    reason = "the binary32 mirror keeps the shader's own operation order, which a fused multiply-add changes"
)]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the vertex stage reads the same binary32 lanes this mirror narrows to"
)]
#![allow(
    clippy::print_stdout,
    reason = "the census tests report the counts their assertions bound"
)]
//! Scene placement at a fully rotated five-dimensional pose, read from the payloads present uploads.
//!
//! The row reproduced here is a saved view whose settled Final frame showed a curtain of vertical
//! streaks and a flat slab with a straight diagonal edge: a mixed slice, all ten camera angles
//! nonzero, height scale 3.565, both perspective limits at eight, zoom zero. A settled frame may
//! not assert geometry the object does not have, so every drawn vertex has to be placed by the one
//! projection the pose defines, and a vertex that projection does not define may not be drawn.
//!
//! The census below reads the binary32 `SceneUniform` and HOT lanes that present writes to the
//! GPU, not only the binary64 pose kept beside them, so a difference between the two is a
//! measurement rather than an assumption.

use ember_julibrot_math::{
    ObjectAngles, Pose, PoseMap, ViewControls, construct_plane, screen_to_plane,
};
use ember_julibrot_present::{
    CLASSIC_PALETTE, SceneUniform, camera_rotation, camera_rotation_pairs, camera_translation,
    grid_screen, project_scene_point, project_scene_record_vertex, project_scene_vertex,
    project_scene_vertex_exact, view_scale,
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

/// Builds the pose the way the app does.
///
/// `map_for` (`crates/labs/julibrot/app/src/state.rs:1576-1588`) takes the grid extent, computes
/// `aspect` as the binary64 quotient of its two axes, calls `screen_to_plane` with the same five
/// arguments, and turns `DegenerateViewMap` into `PoseMap::EdgeOn`. Nothing else in the app builds
/// a map, and the escape grid is sampled through the packed rows of this one.
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

/// The exact binary32 payloads present writes for this row.
///
/// `SceneUniform::new` is the constructor `submit_scene` uses
/// (`crates/labs/julibrot/present/src/gpu/device/scene/submit.rs:49-58`), and the HOT lanes are
/// the ones the HOT write fills (`crates/labs/julibrot/present/src/gpu/device/warp.rs:98-123`).
struct Uploaded {
    scene: SceneUniform,
    rotation_pairs: [[f32; 4]; 5],
    translation: [[f32; 4]; 2],
    observer: [f32; 4],
    scale: [f32; 4],
}

fn uploaded(pose: &Pose) -> Uploaded {
    let scene = SceneUniform::new(
        [pose.grid_width, pose.grid_height],
        3,
        ITERATION_CAP,
        0,
        pose.grid_width * pose.grid_height,
        pose.plane,
        pose.map,
        CLASSIC_PALETTE,
    )
    .expect("the row's map packs into the scene payload");
    Uploaded {
        scene,
        rotation_pairs: camera_rotation_pairs(pose.view.camera).expect("finite camera angles"),
        translation: camera_translation(pose.view.camera_translation).expect("finite translation"),
        observer: camera_rotation(pose.view.camera_yaw, pose.view.camera_pitch)
            .expect("finite observer"),
        scale: view_scale(
            pose.view.height_scale,
            pose.view.distance_five,
            pose.view.distance_four,
        )
        .expect("finite view scale"),
    }
}

/// The shader's own factor order: 45, 35, 25, 15, 34, 24, 23, 14, 13, 12, as `(i, j, lane, slot)`.
const ROTATION_ORDER: [(usize, usize, usize, usize); 10] = [
    (3, 4, 4, 2),
    (2, 4, 4, 0),
    (1, 4, 3, 2),
    (0, 4, 3, 0),
    (2, 3, 2, 2),
    (1, 3, 2, 0),
    (1, 2, 1, 2),
    (0, 3, 1, 0),
    (0, 2, 0, 2),
    (0, 1, 0, 0),
];

/// The fifth coordinate the vertex stage builds, in the binary32 the shader uses.
///
/// `None` is the shader's own horizon test: a non-positive packed map denominator.
fn uploaded_fifth(uploaded: &Uploaded, column: u32, row: u32, record_height: f32) -> Option<f32> {
    let scene = &uploaded.scene;
    let screen_x = grid_screen(column, scene.grid[0]) as f32;
    let screen_y = grid_screen(row, scene.grid[1]) as f32;
    let dot = |lane: [f32; 4]| lane[0] * screen_x + lane[1] * screen_y + lane[2];
    let homogeneous = [
        dot(scene.screen_to_plane_row_0),
        dot(scene.screen_to_plane_row_1),
        dot(scene.screen_to_plane_row_2),
    ];
    if !homogeneous.iter().all(|value| value.is_finite()) || homogeneous[2] <= 0.0 {
        return None;
    }
    let offset = [
        homogeneous[0] / homogeneous[2],
        homogeneous[1] / homogeneous[2],
    ];
    if !offset.iter().all(|value| value.is_finite()) {
        return None;
    }
    let chart_scale = 4.0 * scene.screen_to_plane_row_2[3] / scene.grid[0] as f32;
    let mut point: [f32; 5] = core::array::from_fn(|axis| {
        if axis == 4 {
            uploaded.scale[0] * (record_height + 2.0) * 0.5
        } else {
            chart_scale * (offset[0] * scene.basis_u[axis] + offset[1] * scene.basis_v[axis])
        }
    });
    for (first, second, lane, slot) in ROTATION_ORDER {
        let cosine = uploaded.rotation_pairs[lane][slot];
        let sine = uploaded.rotation_pairs[lane][slot + 1];
        let a = cosine * point[first] - sine * point[second];
        let b = sine * point[first] + cosine * point[second];
        point[first] = a;
        point[second] = b;
    }
    for axis in 0..4 {
        point[axis] += uploaded.translation[0][axis];
    }
    point[4] += uploaded.translation[1][0];
    Some(point[4])
}

/// The census the lane's numbers rest on, read from the uploaded binary32 payloads.
///
/// Every count here comes from `SceneUniform` and the HOT lanes and is compared against the same
/// census taken in binary64 through the pose, so a field that differed between the two would show
/// up as a differing count rather than as an assumption nobody checked.
#[test]
fn the_uploaded_payloads_carry_the_same_census_as_the_pose() {
    let pose = pose_with(steep_view());
    let uploaded = uploaded(&pose);
    assert_eq!(uploaded.scene.span[2], 0, "the row's map is not edge-on");
    assert_eq!(uploaded.scene.screen_to_plane_row_2[3], 1.0, "apron one");
    assert!((uploaded.scale[0] - 3.565).abs() < 1.0e-6);
    assert_eq!(uploaded.scale[1], 8.0);
    assert_eq!(uploaded.scale[2], 8.0);
    assert_eq!(uploaded.observer, [1.0, 0.0, 1.0, 0.0]);

    let limit = 0.95 * f64::from(uploaded.scale[1]);
    let mut uploaded_horizon = 0_u64;
    let mut uploaded_past = 0_u64;
    let mut pose_horizon = 0_u64;
    let mut pose_past = 0_u64;
    let mut disagreements = 0_u64;
    let mut total = 0_u64;
    for row in (0..EXTENT[1]).step_by(3) {
        for column in (0..EXTENT[0]).step_by(3) {
            total += 1;
            let screen = sample_screen(column, row);
            let fifth = uploaded_fifth(&uploaded, column, row, -2.0);
            let has_plane_point = map_denominator(&pose, screen) > 0.0;
            let uploaded_over = fifth.is_some_and(|fifth| f64::from(fifth) > limit);
            let pose_over = has_plane_point && project_scene_point(&pose, screen, -2.0).is_none();
            uploaded_horizon += u64::from(fifth.is_none());
            pose_horizon += u64::from(!has_plane_point);
            uploaded_past += u64::from(uploaded_over);
            pose_past += u64::from(pose_over);
            if fifth.is_none() == has_plane_point || uploaded_over != pose_over {
                disagreements += 1;
            }
        }
    }
    println!(
        "uploaded: horizon {uploaded_horizon} past-limit {uploaded_past} of {total}; pose: horizon {pose_horizon} past-limit {pose_past}; disagreements {disagreements}"
    );
    assert!(
        uploaded_horizon > 0 && uploaded_past > 0,
        "the uploaded payload must reach both refusals, or the lane's numbers describe nothing"
    );
    assert!(
        disagreements * 200 <= total,
        "the uploaded payload and the pose disagree about {disagreements} of {total} samples"
    );
}

/// A sample lifted past the near limit is refused, and the refusal is the limit's, not a shortcut.
///
/// The limit cuts at `0.05 * d₅`, so the band `(0.95 d₅, d₅)` is still in front of the
/// five-dimensional camera and is discarded along with everything past `d₅`. Screen point
/// (-480, -134.5) is grid sample (0, 135); at every record height below its lift is non-zero, so
/// no identity shortcut lies on this path.
#[test]
fn a_vertex_past_the_five_dimensional_near_limit_is_refused_at_the_steep_row() {
    let pose = pose_with(steep_view());
    let screen = sample_screen(0, 135);
    assert!(
        map_denominator(&pose, screen) > 0.0,
        "the sample has a plane point, so only the near limit can refuse it"
    );
    for record_height in [-1.0, 0.0, 1.0, 2.0] {
        assert_eq!(
            project_scene_point(&pose, screen, record_height),
            None,
            "a lifted vertex past the near limit may not be placed"
        );
    }
    assert_eq!(project_scene_vertex_exact(&pose, screen, -2.0), None);
}

/// The whole forward chain returns the screen point a zero-lift sample came from.
///
/// This is the claim the identity shortcut rests on, so it is measured through
/// `project_scene_vertex_exact`, which takes no shortcut. Where the near limit is not passed the
/// chain reproduces its own argument; where it is, it refuses.
#[test]
fn the_exact_chain_round_trips_every_zero_lift_sample_the_steep_row_draws() {
    let pose = pose_with(steep_view());
    let mut drawn = 0_u64;
    let mut worst = 0.0_f64;
    for row in (0..EXTENT[1]).step_by(7) {
        for column in (0..EXTENT[0]).step_by(7) {
            let screen = sample_screen(column, row);
            let Some((point, _)) = project_scene_vertex_exact(&pose, screen, -2.0) else {
                continue;
            };
            drawn += 1;
            worst = worst.max((point[0] - screen[0]).hypot(point[1] - screen[1]));
        }
    }
    assert!(drawn > 5_000, "only {drawn} samples reached the chain");
    assert!(
        worst <= 1.0e-6,
        "the exact chain moved a zero-lift sample {worst} px from its own screen point"
    );
}

/// The identity shortcut agrees with the chain it stands in for, wherever the stage runs that chain.
///
/// The vertex stage runs the forward chain per sample only when the height amplitude is nonzero;
/// at amplitude zero it takes its own whole-frame flat return, which is the shader's rule and not
/// a shortcut standing in for anything. So the agreement is asserted on the lifting poses, and the
/// flat poses are counted instead: the number printed is how many samples the chain would refuse
/// at the four-dimensional or observer limit while the flat chart draws them.
#[test]
fn the_zero_lift_shortcut_agrees_with_the_exact_chain_wherever_the_stage_runs_it() {
    let mut yaw_only = ViewControls::NEUTRAL;
    yaw_only.camera[0] = 0.6;
    yaw_only.height_scale = 3.565;
    let mut flattened = steep_view();
    flattened.height_scale = 0.0;
    let mut lifted_identity = ViewControls::NEUTRAL;
    lifted_identity.height_scale = 3.565;
    let table = [
        ("identity lifted", lifted_identity),
        ("yaw only", yaw_only),
        ("the steep row", steep_view()),
        ("identity flat", ViewControls::NEUTRAL),
        ("the steep row with no height", flattened),
    ];
    for (name, view) in table {
        let pose = pose_with(view);
        let lifting = view.height_scale != 0.0;
        let mut compared = 0_u64;
        let mut flat_only = 0_u64;
        for row in (0..EXTENT[1]).step_by(29) {
            for column in (0..EXTENT[0]).step_by(31) {
                let screen = sample_screen(column, row);
                let shortcut = project_scene_vertex(&pose, screen, -2.0);
                let exact = project_scene_vertex_exact(&pose, screen, -2.0);
                match (shortcut, exact) {
                    (Some((shortcut, _)), Some((exact, _))) => {
                        compared += 1;
                        let separation = (shortcut[0] - exact[0]).hypot(shortcut[1] - exact[1]);
                        assert!(
                            separation <= 1.0e-6,
                            "{name}: the shortcut and the chain differ by {separation} px"
                        );
                    }
                    (None, None) => {}
                    (Some(_), None) => {
                        flat_only += 1;
                        assert!(
                            !lifting,
                            "{name}: the shortcut placed a vertex the chain refuses"
                        );
                    }
                    (None, Some(_)) => {
                        panic!("{name}: the chain placed a vertex the shortcut refuses")
                    }
                }
            }
        }
        println!("{name}: compared {compared}, drawn flat but refused by the chain {flat_only}");
        assert!(compared > 0, "{name}: the pose drew nothing to compare");
    }
}

/// A horizon record has no plane point, so it has no vertex, so nothing is drawn for it.
///
/// Placing it at its flat screen point put a slab at the near depth over relief that is in front
/// of it. The scene pass clears to the exterior colour those samples carry, so refusing the vertex
/// changes nothing while the projection is the identity and removes the slab once it is not.
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

/// An in-set sample is placed by the chain like any other record, not by a flat rule of its own.
#[test]
fn an_in_set_vertex_is_placed_by_the_chain_like_any_other_record() {
    let pose = pose_with(steep_view());
    let mut compared = 0_u64;
    for row in (0..EXTENT[1]).step_by(29) {
        for column in (0..EXTENT[0]).step_by(31) {
            let screen = sample_screen(column, row);
            let interior = placement(&pose, screen, IN_SET_RECORD);
            let exact = project_scene_vertex_exact(&pose, screen, -2.0);
            assert_eq!(
                interior.is_some(),
                exact.is_some(),
                "an in-set vertex is drawn exactly when the chain places it"
            );
            let (Some((interior, _)), Some((exact, _))) = (interior, exact) else {
                continue;
            };
            compared += 1;
            let separation = (interior[0] - exact[0]).hypot(interior[1] - exact[1]);
            assert!(
                separation <= 1.0e-6,
                "an in-set vertex landed {separation} px from the chain's answer"
            );
            let escaped = placement(&pose, screen, FLOOR_ESCAPE_RECORD)
                .expect("the escaped floor neighbour shares the in-set height");
            assert!(
                (interior[0] - escaped.0[0]).hypot(interior[1] - escaped.0[1]) <= 1.0e-9,
                "two records of the same height were placed apart"
            );
        }
    }
    assert!(
        compared > 100,
        "only {compared} in-set vertices were placed"
    );
}

/// A flat chart keeps every sample where the map put it, and the chain agrees that it should.
#[test]
fn a_flat_chart_agrees_with_the_chain_that_every_sample_stays_put() {
    let mut view = steep_view();
    view.height_scale = 0.0;
    let pose = pose_with(view);
    let mut compared = 0_u64;
    for row in (0..EXTENT[1]).step_by(53) {
        for column in (0..EXTENT[0]).step_by(59) {
            let screen = sample_screen(column, row);
            for record in [FLOOR_ESCAPE_RECORD, IN_SET_RECORD, HORIZON_RECORD] {
                assert_eq!(placement(&pose, screen, record), Some((screen, 1.0)));
            }
            if let Some((exact, _)) = project_scene_vertex_exact(&pose, screen, -2.0) {
                compared += 1;
                let separation = (exact[0] - screen[0]).hypot(exact[1] - screen[1]);
                assert!(
                    separation <= 1.0e-6,
                    "the flat early return claims an identity the chain misses by {separation} px"
                );
            }
        }
    }
    assert!(
        compared > 0,
        "the flat chart placed nothing through the chain"
    );
}

/// The plane the pose defines is real: the fixture is not silently edge-on.
#[test]
fn the_steep_row_fixture_has_an_invertible_screen_to_plane_map() {
    let pose = pose_with(steep_view());
    let PoseMap::Mapped(map) = pose.map else {
        panic!("the steep row's map is invertible, so the fixture must not be edge-on")
    };
    assert_eq!(map.apron_scale, 1.0);
    assert!(pose.plane.basis_u.iter().all(|value| value.is_finite()));
    assert!(pose.plane.basis_v.iter().all(|value| value.is_finite()));
}
