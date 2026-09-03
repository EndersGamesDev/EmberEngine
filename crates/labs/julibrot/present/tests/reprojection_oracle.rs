use ember_julibrot_kernels::RefinementLevel;
use ember_julibrot_math::{
    ObjectAngles, Pose, PoseMap, PrecisionMode, ViewControls, construct_plane, screen_to_plane,
};
use ember_julibrot_present::{
    PaletteId, SampleClass, SceneFrame, SubmissionKind, SubmissionMeasurement, WARP_MAX_ERROR_PX,
    Warp, WarpKind, WarpValidation, apply_homography, project_scene_point,
};

const EXTENT: [u32; 2] = [960, 540];

fn pose(
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
        plane_origin: origin,
        zoom_log2,
        view,
        grid_width: EXTENT[0],
        grid_height: EXTENT[1],
        map,
        centre_from_reference_px: displacement,
    }
}

fn frame(pose: Pose) -> SceneFrame {
    SceneFrame {
        scene_id: 7,
        pose,
        palette: PaletteId::Classic,
        iteration_cap: 512,
        level: RefinementLevel::Final,
        extent: EXTENT,
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

fn inside(point: [f64; 2]) -> bool {
    point[0].abs() <= f64::from(EXTENT[0]) * 0.5
        && point[1].abs() <= f64::from(EXTENT[1]) * 0.5
}

fn assert_agrees(name: &str, from: Pose, to: Pose, height: f64, must_clear: bool) {
    let plan = Warp::reproject(
        &frame(from),
        &from,
        &to,
        PrecisionMode::PictureFast,
        WarpValidation::Final,
    );
    if must_clear {
        assert_eq!(plan.kind, WarpKind::ClearOnly, "{name}");
        return;
    }
    assert_eq!(plan.kind, WarpKind::AnchorHomography, "{name}");
    assert!(plan.source_valid, "{name}");
    assert_eq!(plan.source_scene_id, Some(7), "{name}");
    assert_eq!(plan.source_texture_index, Some(1), "{name}");
    let maximum = plan
        .approx_max_error_px
        .expect("every displayable plan publishes its measured maximum");
    assert!(maximum <= WARP_MAX_ERROR_PX, "{name}: {maximum}");
    let inverse_sampling = rows(plan.rows);
    let mut comparisons = 0_u32;
    for row in 0..9 {
        for column in 0..9 {
            let target = [
                (f64::from(column) / 8.0 - 0.5) * f64::from(EXTENT[0]),
                (f64::from(row) / 8.0 - 0.5) * f64::from(EXTENT[1]),
            ];
            let Some(source_flat) = apply_homography(inverse_sampling, target) else {
                continue;
            };
            let Some(fresh_destination) = project_scene_point(&to, target, height) else {
                continue;
            };
            let Some(fresh_source) = project_scene_point(&from, source_flat, height) else {
                continue;
            };
            let Some(warped_source) = apply_homography(inverse_sampling, fresh_destination) else {
                continue;
            };
            if !inside(fresh_source) || !inside(warped_source) {
                continue;
            }
            let error = (fresh_source[0] - warped_source[0])
                .hypot(fresh_source[1] - warped_source[1]);
            assert!(error <= maximum + 1.0e-3, "{name}: {error} > {maximum}");
            comparisons = comparisons.saturating_add(1);
        }
    }
    assert!(comparisons > 0, "{name} had no shared source points");
}

fn relief() -> ViewControls {
    let mut camera = [0.0; 10];
    camera[8] = 0.3;
    ViewControls {
        camera,
        camera_yaw: 0.2,
        camera_pitch: -0.15,
        height_scale: 1.0,
        ..ViewControls::NEUTRAL
    }
}

#[test]
fn retained_warp_matches_fresh_scene_over_the_navigation_corpus() {
    let flat = pose(
        ObjectAngles::JULIA,
        ViewControls::NEUTRAL,
        [0.0; 4],
        0.0,
        [0.0; 2],
    );

    let mut pan = flat;
    pan.centre_from_reference_px = [5.0, -3.0];
    assert_agrees("pure pan", flat, pan, 0.0, false);

    let mut zoom = flat;
    zoom.zoom_log2 = 0.1;
    assert_agrees("pure zoom", flat, zoom, 0.0, false);

    let mut rotated_flat_view = ViewControls::NEUTRAL;
    rotated_flat_view.camera[0] = 0.1;
    let rotated_flat = pose(
        ObjectAngles::JULIA,
        rotated_flat_view,
        [0.0; 4],
        0.0,
        [0.0; 2],
    );
    assert_agrees("view rotation h=0", flat, rotated_flat, 0.0, false);

    let relief_from = pose(
        ObjectAngles::JULIA,
        relief(),
        [0.0; 4],
        0.0,
        [0.0; 2],
    );
    let mut relief_to_view = relief();
    relief_to_view.camera[8] += 1.0e-4;
    let relief_to = pose(
        ObjectAngles::JULIA,
        relief_to_view,
        [0.0; 4],
        0.0,
        [0.0; 2],
    );
    assert_agrees("view rotation h=1", relief_from, relief_to, 1.0, false);

    let mut observer = ViewControls::NEUTRAL;
    observer.camera_yaw = 1.0e-4;
    observer.camera_pitch = -1.0e-4;
    let observer_pose = pose(
        ObjectAngles::JULIA,
        observer,
        [0.0; 4],
        0.0,
        [0.0; 2],
    );
    assert_agrees("camera yaw pitch", flat, observer_pose, 0.0, false);

    let mut tiny_object = ObjectAngles::JULIA;
    tiny_object.rho_12 += 1.0e-9;
    let tiny_object_pose = pose(
        tiny_object,
        ViewControls::NEUTRAL,
        [0.0; 4],
        0.0,
        [0.0; 2],
    );
    assert_agrees("object 1e-9", flat, tiny_object_pose, 0.0, false);

    let mut changed_object = ObjectAngles::JULIA;
    changed_object.rho_12 += 0.3;
    let changed_object_pose = pose(
        changed_object,
        ViewControls::NEUTRAL,
        [0.0; 4],
        0.0,
        [0.0; 2],
    );
    assert_agrees("object 0.3", flat, changed_object_pose, 0.0, true);

    let in_plane = pose(
        ObjectAngles::JULIA,
        ViewControls::NEUTRAL,
        [0.001, -0.0005, 0.0, 0.0],
        0.0,
        [0.0; 2],
    );
    assert_agrees("in-plane origin", flat, in_plane, 0.0, false);
    let out_of_plane = pose(
        ObjectAngles::JULIA,
        ViewControls::NEUTRAL,
        [0.0, 0.0, 0.01, 0.0],
        0.0,
        [0.0; 2],
    );
    assert_agrees("out-of-plane origin", flat, out_of_plane, 0.0, true);

    let translated = pose(
        ObjectAngles::JULIA,
        ViewControls {
            camera_translation: [0.1, -0.05, 0.02, 0.0, 0.03],
            ..ViewControls::NEUTRAL
        },
        [0.0; 4],
        0.0,
        [0.0; 2],
    );
    assert_agrees("camera translation", flat, translated, 0.0, false);

    let mut cross_view = relief();
    cross_view.camera_translation = [1.0e-4, -2.0e-4, 0.0, 0.0, 1.0e-4];
    cross_view.camera_yaw += 1.0e-4;
    let cross = pose(
        ObjectAngles::JULIA,
        cross_view,
        [1.0e-5, -1.0e-5, 0.0, 0.0],
        0.001,
        [0.1, -0.1],
    );
    assert_agrees("cross terms", relief_from, cross, 1.0, false);
}
