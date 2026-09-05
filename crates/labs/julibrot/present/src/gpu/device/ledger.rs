use super::{EXPOSURE_FACT_STEPS, Pose, PoseMap, WarpKind};

pub(super) fn pose_is_finite(pose: &Pose) -> bool {
    pose.grid_width > 0
        && pose.grid_height > 0
        && pose.view.is_valid()
        && pose.object.is_valid()
        && [
            pose.zoom_log2,
            pose.centre_from_reference_px[0],
            pose.centre_from_reference_px[1],
        ]
        .into_iter()
        .chain(pose.plane_origin)
        .all(f64::is_finite)
        && pose
            .plane
            .basis_u
            .into_iter()
            .chain(pose.plane.basis_v)
            .all(f32::is_finite)
        && match pose.map {
            PoseMap::Mapped(map) => {
                map.rows
                    .into_iter()
                    .chain(map.inverse)
                    .chain([map.condition_number, map.apron_scale])
                    .all(f64::is_finite)
                    && map.apron_scale >= 1.0
            }
            PoseMap::EdgeOn => true,
        }
}

pub(super) fn select_warp_source(
    planned: Option<(u64, u32)>,
    retained: Option<&crate::SceneFrame>,
) -> Option<&crate::SceneFrame> {
    retained.filter(|frame| {
        select_warp_source_identity(planned, Some((frame.scene_id, frame.texture_index))).is_some()
    })
}

/// Measures the share of a fixed destination lattice that the actual warp shader paints clear.
/// Points behind either screen-map denominator are exterior sky rather than exposure.
pub(super) fn warp_exposed_fraction(
    plan: &crate::WarpPlan,
    to_pose: &Pose,
    retained: Option<&crate::SceneFrame>,
) -> Option<f64> {
    let retained = select_warp_source(
        plan.source_scene_id.zip(plan.source_texture_index),
        retained,
    )?;
    if plan.kind == WarpKind::ReliefRedraw {
        return None;
    }
    if !plan.exposed {
        return Some(0.0);
    }
    let PoseMap::Mapped(screen_to_plane) = to_pose.map else {
        return None;
    };
    let source_half_width = f64::from(retained.extent[0]) * 0.5;
    let source_half_height = f64::from(retained.extent[1]) * 0.5;
    let mut exposed = 0_u32;
    for row in 0..EXPOSURE_FACT_STEPS {
        for column in 0..EXPOSURE_FACT_STEPS {
            let target = [
                (f64::from(column) / f64::from(EXPOSURE_FACT_STEPS - 1) - 0.5)
                    * f64::from(to_pose.grid_width),
                (f64::from(row) / f64::from(EXPOSURE_FACT_STEPS - 1) - 0.5)
                    * f64::from(to_pose.grid_height),
            ];
            let plane_weight = screen_to_plane.rows[6].mul_add(
                target[0],
                screen_to_plane.rows[7].mul_add(target[1], screen_to_plane.rows[8]),
            );
            if !plane_weight.is_finite() || plane_weight <= 0.0 {
                continue;
            }
            let mapped = plan.rows.map(|plan_row| {
                f64::from(plan_row[0]).mul_add(
                    target[0],
                    f64::from(plan_row[1]).mul_add(target[1], f64::from(plan_row[2])),
                )
            });
            if !mapped.iter().all(|value| value.is_finite()) || mapped[2] <= 0.0 {
                continue;
            }
            let source_pixel = [mapped[0] / mapped[2], mapped[1] / mapped[2]];
            if source_pixel[0] < -source_half_width
                || source_pixel[0] > source_half_width
                || source_pixel[1] < -source_half_height
                || source_pixel[1] > source_half_height
            {
                exposed = exposed.saturating_add(1);
            }
        }
    }
    Some(f64::from(exposed) / f64::from(EXPOSURE_FACT_STEPS * EXPOSURE_FACT_STEPS))
}

pub(super) const fn select_warp_source_identity(
    planned: Option<(u64, u32)>,
    retained: Option<(u64, u32)>,
) -> Option<(u64, u32)> {
    match (planned, retained) {
        (Some(planned), Some(retained)) if planned.0 == retained.0 && planned.1 == retained.1 => {
            Some(retained)
        }
        _ => None,
    }
}

pub(super) const fn identity_rows() -> [[f32; 4]; 3] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]
}

/// Rows that hold a completed picture of `source_extent` on a destination lattice of another size.
///
/// A hold shows the last completed picture unchanged while the next scene renders, and the two
/// lattices cover the same field of view whatever their pixel counts: the chart span a pose
/// covers across its width is `4 / 2^zoom` however many pixels sample it, which is why a
/// reprojection between two extents scales by exactly this ratio (`planner.rs` lines 415 to 417).
/// The warp fragment normalises the mapped source pixel by the source texture's own dimensions,
/// and a scene texture is allocated at exactly the delivered extent of the scene drawn into it,
/// so the ratio between the two extents is the whole of the mapping. Equal extents give identity.
///
/// An unusable extent returns `None` so the caller refuses the hold rather than placing the
/// picture somewhere the geometry does not put it.
pub(super) fn hold_rows(
    source_extent: [u32; 2],
    destination_extent: [u32; 2],
) -> Option<[[f32; 4]; 3]> {
    if source_extent
        .into_iter()
        .chain(destination_extent)
        .any(|extent| extent == 0)
    {
        return None;
    }
    let scale = [
        f64::from(source_extent[0]) / f64::from(destination_extent[0]),
        f64::from(source_extent[1]) / f64::from(destination_extent[1]),
    ];
    let rows =
        crate::pack_homography_rows([scale[0], 0.0, 0.0, 0.0, scale[1], 0.0, 0.0, 0.0, 1.0])?;
    (rows[0][0] > 0.0 && rows[1][1] > 0.0).then_some(rows)
}

pub(super) const fn clear_warp_plan(edge_on: bool, exposed: bool) -> crate::WarpPlan {
    crate::WarpPlan {
        rows: identity_rows(),
        source_scene_id: None,
        source_texture_index: None,
        source_valid: false,
        edge_on,
        exposed,
        kind: WarpKind::ClearOnly,
        chart_residual: 0.0,
        approx_max_error_px: None,
        approx_p95_error_px: None,
    }
}

pub(super) fn apply_hold_policy(
    mut plan: crate::WarpPlan,
    retained: Option<&crate::SceneFrame>,
    hold_refused_warp: bool,
    destination_extent: [u32; 2],
) -> crate::WarpPlan {
    if !hold_refused_warp || plan.kind != WarpKind::ClearOnly {
        return plan;
    }
    let Some(frame) = retained else {
        return plan;
    };
    // A hold whose scale cannot be stated is refused: the clear plan asserts no geometry, while a
    // picture placed at the wrong scale asserts geometry that does not exist.
    let Some(rows) = hold_rows(frame.extent, destination_extent) else {
        return plan;
    };
    plan.rows = rows;
    plan.source_scene_id = Some(frame.scene_id);
    plan.source_texture_index = Some(frame.texture_index);
    plan.source_valid = true;
    plan.exposed = false;
    plan.kind = WarpKind::HoldStale;
    plan
}
