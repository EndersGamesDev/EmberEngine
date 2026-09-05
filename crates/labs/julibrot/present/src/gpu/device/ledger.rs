use super::{EXPOSURE_FACT_STEPS, Pose, PoseMap, WarpKind};
use crate::{LatticePair, identity_warp_rows};

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

pub(super) const fn clear_warp_plan(edge_on: bool, exposed: bool) -> crate::WarpPlan {
    crate::WarpPlan {
        rows: identity_warp_rows(),
        lattice: None,
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
    // A hold shows the last completed picture unchanged while the next scene renders, which is the
    // covering map of the pair between the picture's delivered extent and the lattice it is
    // presented on. A hold whose pair cannot be named is refused: the clear plan asserts no
    // geometry, while a picture placed at the wrong scale asserts geometry that does not exist.
    let Some(lattice) = LatticePair::new(frame.extent, destination_extent) else {
        return plan;
    };
    let Some(rows) = lattice.covering_rows() else {
        return plan;
    };
    plan.rows = rows;
    plan.lattice = Some(lattice);
    plan.source_scene_id = Some(frame.scene_id);
    plan.source_texture_index = Some(frame.texture_index);
    plan.source_valid = true;
    plan.exposed = false;
    plan.kind = WarpKind::HoldStale;
    plan
}

/// Why a plan was refused for the lattice pair it named.
///
/// Published so a captured frame's facts row says which of the three ways a plan can fail to
/// state its geometry actually happened, rather than only that the surface went clear.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LatticeRefusal {
    /// A plan claiming a source that named no lattice pair at all.
    Unstated,
    /// A plan whose named destination is not the lattice this payload publishes.
    WrongDestination,
    /// A plan claiming full coverage whose destination leaves the source picture.
    OutsideSource,
}

impl LatticeRefusal {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Unstated => "plan named no lattice pair",
            Self::WrongDestination => "plan named another destination lattice",
            Self::OutsideSource => "plan maps the destination outside the source",
        }
    }
}

/// Refuses a plan that does not state where it puts the picture on this destination lattice.
///
/// Three claims are checked, and each of them is a way a frame can reach the surface at a scale
/// the geometry does not have. A plan that samples a source names both extents, or it is asserting
/// that they are equal. The destination it names is the lattice this HOT payload publishes, or the
/// rows are read against a lattice they were not written for. And a plan that does not declare
/// exposure claims its source covers the destination, which is a question the pair can answer on
/// the CPU: the four destination corners and the centre map inside the source picture, or they do
/// not.
///
/// A refused plan becomes an honest clear. Clearing is a loss — it restarts the refinement ladder
/// — but it is the loss the exposure machinery is built to recover from, and it asserts nothing
/// about geometry. A picture at the wrong scale asserts geometry that does not exist, and under
/// the rendering rule a moving frame may be very inaccurate but never wrong.
///
/// Exposure is the declared exception and not a hole: a plan that declares it is telling the
/// exposure machinery that part of the destination has no source, which is measured separately in
/// pixels by `warp_exposed_fraction` and answered by a completed scene. A relief redraw declares
/// exposure and does not sample the source texture at all; it draws the retained records as a
/// mesh, so its rows are not read by the warp fragment.
pub(super) fn enforce_lattice(
    plan: crate::WarpPlan,
    destination_extent: [u32; 2],
) -> (crate::WarpPlan, Option<LatticeRefusal>) {
    if !plan.source_valid {
        return (plan, None);
    }
    let refusal = match plan.lattice {
        None => Some(LatticeRefusal::Unstated),
        Some(lattice) if lattice.destination() != destination_extent => {
            Some(LatticeRefusal::WrongDestination)
        }
        Some(lattice) if !plan.exposed && !lattice.covers_destination(plan.rows) => {
            Some(LatticeRefusal::OutsideSource)
        }
        Some(_) => None,
    };
    refusal.map_or((plan, None), |refusal| {
        (clear_warp_plan(plan.edge_on, true), Some(refusal))
    })
}
