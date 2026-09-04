use ember_julibrot_kernels::EscapeGrid;
use ember_julibrot_math::{Plane, Pose, plane_chart_relation};

use crate::{
    DropReason, PaletteId, PresentError, RefinementLevel, SceneFrame, SubmissionMeasurement,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PendingScene {
    pub scene_id: u64,
    pub pose: Pose,
    pub palette: PaletteId,
    pub iteration_cap: u32,
    pub level: RefinementLevel,
    pub extent: [u32; 2],
    pub grid: EscapeGrid,
    pub texture_index: u32,
    pub centre_revision: u32,
    pub plane_origin_f64: [f64; 4],
    pub precision_mode: &'static str,
    pub drop_reason: Option<DropReason>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneCompletion {
    Promoted(SceneFrame),
    /// Completed scene whose texture stays reusable because the accepted retained source is better.
    KeptBest(SceneFrame),
    Dropped {
        pending: PendingScene,
        reason: DropReason,
        measurement: SubmissionMeasurement,
    },
}

/// Pure refresh-loop exposure state: only a completed scene may clear it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExposureLatch {
    due: bool,
}

impl ExposureLatch {
    pub const fn observe_warp(&mut self, exposed: bool) {
        self.due |= exposed;
    }

    pub const fn scene_completed(&mut self) {
        self.due = false;
    }

    pub const fn due(self) -> bool {
        self.due
    }
}

/// Pure two-index ledger; GPU resources mirror these identities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneLedger {
    retained: Option<SceneFrame>,
    retained_grid: Option<EscapeGrid>,
    pending: Option<PendingScene>,
}

impl SceneLedger {
    pub fn available_texture_index(&self) -> Result<u32, PresentError> {
        if let Some(pending) = &self.pending {
            return Err(PresentError::SceneBusy {
                scene_id: pending.scene_id,
            });
        }
        Ok(self
            .retained
            .as_ref()
            .map_or(0, |frame| 1 - frame.texture_index))
    }

    pub fn begin(
        &mut self,
        build: impl FnOnce(u32) -> Result<PendingScene, PresentError>,
    ) -> Result<u32, PresentError> {
        let texture_index = self.available_texture_index()?;
        let incoming = build(texture_index)?;
        if incoming.texture_index != texture_index {
            return Err(PresentError::Device {
                operation: "construct pending scene texture index",
            });
        }
        self.pending = Some(incoming);
        Ok(texture_index)
    }

    #[cfg(test)]
    pub fn complete(&mut self, measurement: SubmissionMeasurement) -> Option<SceneCompletion> {
        self.complete_preserving_accepted_best(measurement, false)
    }

    pub fn complete_preserving_accepted_best(
        &mut self,
        measurement: SubmissionMeasurement,
        preserve_accepted_best: bool,
    ) -> Option<SceneCompletion> {
        let pending = self.pending.take()?;
        if let Some(reason) = pending.drop_reason {
            return Some(SceneCompletion::Dropped {
                pending,
                reason,
                measurement,
            });
        }
        let frame = SceneFrame {
            scene_id: pending.scene_id,
            pose: pending.pose,
            palette: pending.palette,
            iteration_cap: pending.iteration_cap,
            level: pending.level,
            extent: pending.extent,
            texture_index: pending.texture_index,
            centre_revision: pending.centre_revision,
            plane_origin_f64: pending.plane_origin_f64,
            precision_mode: pending.precision_mode,
            measurement,
        };
        if preserve_accepted_best
            && self
                .retained
                .as_ref()
                .is_some_and(|retained| retained_is_better(retained, &frame))
        {
            return Some(SceneCompletion::KeptBest(frame));
        }
        self.retained_grid = Some(pending.grid);
        self.retained = Some(frame.clone());
        Some(SceneCompletion::Promoted(frame))
    }

    pub const fn cancel_pending(&mut self) -> Option<PendingScene> {
        self.pending.take()
    }

    pub fn invalidate_incompatible(
        &mut self,
        iteration_cap: u32,
        plane_origin_f64: [f64; 4],
        plane: Plane,
        precision_mode: &'static str,
    ) -> bool {
        let retained_invalid = self.retained.as_ref().is_some_and(|frame| {
            frame.iteration_cap != iteration_cap
                || !origins_share_slice(&frame.pose, plane_origin_f64)
                || plane_chart_relation(frame.pose.plane, plane).is_none()
                || frame.precision_mode != precision_mode
        });
        if retained_invalid {
            self.retained = None;
            self.retained_grid = None;
        }
        if let Some(pending) = &mut self.pending
            && (pending.iteration_cap != iteration_cap
                || !origins_share_slice(&pending.pose, plane_origin_f64)
                || plane_chart_relation(pending.pose.plane, plane).is_none()
                || pending.precision_mode != precision_mode)
        {
            pending.drop_reason = Some(DropReason::IncompatibleMain);
        }
        retained_invalid
    }

    pub fn apply_reference_shift(
        &mut self,
        new_generation: u32,
        new_revision: u32,
        shift_px: [f64; 2],
    ) {
        if let Some(frame) = &mut self.retained
            && frame.centre_revision != new_revision
        {
            let sampled_pose = frame.pose;
            rebase_pose(&mut frame.pose, &sampled_pose, shift_px);
            frame.pose.orbit_generation = new_generation;
            frame.centre_revision = new_revision;
        }
        if let Some(pending) = &mut self.pending
            && pending.centre_revision != new_revision
        {
            let sampled_pose = pending.pose;
            rebase_pose(&mut pending.pose, &sampled_pose, shift_px);
            pending.pose.orbit_generation = new_generation;
            pending.centre_revision = new_revision;
        }
    }

    pub const fn mark_replaced(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.drop_reason = Some(DropReason::ReplacedMain);
        }
    }

    pub const fn retained(&self) -> Option<&SceneFrame> {
        self.retained.as_ref()
    }

    pub const fn retained_grid(&self) -> Option<&EscapeGrid> {
        self.retained_grid.as_ref()
    }

    pub fn forget_retained_grid(&mut self, grid: &EscapeGrid) -> bool {
        let matches = self
            .retained_grid
            .as_ref()
            .is_some_and(|retained| retained.span == grid.span);
        if matches {
            self.retained = None;
            self.retained_grid = None;
        }
        matches
    }

    pub fn forget_retained_records(&mut self, grid: &EscapeGrid) -> bool {
        let matches = self
            .retained_grid
            .as_ref()
            .is_some_and(|retained| retained.span == grid.span);
        if matches {
            self.retained_grid = None;
        }
        matches
    }

    #[cfg(test)]
    pub const fn pending(&self) -> Option<&PendingScene> {
        self.pending.as_ref()
    }
}

const fn level_rank(level: RefinementLevel) -> u32 {
    match level {
        RefinementLevel::Preview => 0,
        RefinementLevel::Interactive => 1,
        RefinementLevel::Final => 2,
    }
}

fn retained_is_better(retained: &SceneFrame, candidate: &SceneFrame) -> bool {
    let retained_rank = level_rank(retained.level);
    let candidate_rank = level_rank(candidate.level);
    retained_rank > candidate_rank
        || (retained_rank == candidate_rank
            && candidate.level != RefinementLevel::Final
            && u64::from(retained.extent[0]) * u64::from(retained.extent[1])
                > u64::from(candidate.extent[0]) * u64::from(candidate.extent[1]))
}

fn origins_share_slice(pose: &Pose, origin: [f64; 4]) -> bool {
    let delta = core::array::from_fn(|axis| origin[axis] - pose.plane_origin[axis]);
    let projection = [
        dot_f64(pose.plane.basis_u, delta),
        dot_f64(pose.plane.basis_v, delta),
    ];
    let residual: [f64; 4] = core::array::from_fn(|axis| {
        delta[axis]
            - f64::from(pose.plane.basis_u[axis]).mul_add(
                projection[0],
                f64::from(pose.plane.basis_v[axis]) * projection[1],
            )
    });
    let pixels_per_chart = 0.25 * f64::from(pose.grid_width) * pose.zoom_log2.exp2();
    residual
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
        * pixels_per_chart
        <= 0.5
}

fn dot_f64(left: [f32; 4], right: [f64; 4]) -> f64 {
    left.into_iter()
        .zip(right)
        .fold(0.0, |sum, (a, b)| f64::from(a).mul_add(b, sum))
}

fn rebase_pose(pose: &mut Pose, accepted_pose: &Pose, shift_px: [f64; 2]) {
    let ratio = (pose.zoom_log2 - accepted_pose.zoom_log2).exp2() * f64::from(pose.grid_width)
        / f64::from(accepted_pose.grid_width);
    let overlap = [
        dot(pose.plane.basis_u, accepted_pose.plane.basis_u),
        dot(pose.plane.basis_u, accepted_pose.plane.basis_v),
        dot(pose.plane.basis_v, accepted_pose.plane.basis_u),
        dot(pose.plane.basis_v, accepted_pose.plane.basis_v),
    ];
    pose.centre_from_reference_px[0] = ratio.mul_add(
        -overlap[0].mul_add(shift_px[0], overlap[1] * shift_px[1]),
        pose.centre_from_reference_px[0],
    );
    pose.centre_from_reference_px[1] = ratio.mul_add(
        -overlap[2].mul_add(shift_px[0], overlap[3] * shift_px[1]),
        pose.centre_from_reference_px[1],
    );
}

fn dot(left: [f32; 4], right: [f32; 4]) -> f64 {
    left.into_iter()
        .zip(right)
        .fold(0.0, |sum, (a, b)| f64::from(a).mul_add(f64::from(b), sum))
}

#[cfg(test)]
mod tests {
    use ember_julibrot_math::{
        Homography, ObjectAngles, Plane, PoseMap, PrecisionMode, ViewControls, construct_plane,
    };

    use super::*;
    use crate::{SampleClass, SubmissionKind, SubmissionMeasurement};

    const ORIGIN: [f64; 4] = [0.0; 4];
    const MODE: &str = PrecisionMode::Deterministic.as_str();

    fn grid(extent: [u32; 2], level: RefinementLevel) -> EscapeGrid {
        let mut arena = ember_lab_heap::SpanArena::new(2_048, 1, 1_024, 4_096, 16)
            .expect("scene-ledger fixture arena is valid");
        let span = arena
            .allocate_span(extent[0] * extent[1], 64)
            .expect("scene-ledger fixture grid fits");
        EscapeGrid {
            span,
            width: extent[0],
            height: extent[1],
            level,
        }
    }

    fn pose(generation: u32) -> Pose {
        Pose {
            epoch: 1,
            orbit_generation: generation,
            plane: Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            object: ObjectAngles::JULIA,
            plane_origin: ORIGIN,
            zoom_log2: 20.0,
            view: ViewControls::NEUTRAL,
            grid_width: 800,
            grid_height: 600,
            map: PoseMap::Mapped(Homography::IDENTITY),
            centre_from_reference_px: [11.0, -3.0],
        }
    }

    fn measurement(scene_id: u64) -> SubmissionMeasurement {
        SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id: scene_id,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            precision_mode: MODE,
            wall_ms: 2.0,
            fence_wait_ms: 1.0,
            polls: 2,
        }
    }

    fn begin(ledger: &mut SceneLedger, scene_id: u64, generation: u32) -> u32 {
        ledger
            .begin(|texture_index| {
                Ok(PendingScene {
                    scene_id,
                    pose: pose(generation),
                    palette: PaletteId::Classic,
                    iteration_cap: 64,
                    level: RefinementLevel::Preview,
                    extent: [800, 600],
                    grid: grid([800, 600], RefinementLevel::Preview),
                    texture_index,
                    centre_revision: generation,
                    plane_origin_f64: ORIGIN,
                    precision_mode: MODE,
                    drop_reason: None,
                })
            })
            .expect("test scene is valid")
    }

    #[test]
    fn exactly_two_indices_alternate_and_a_third_scene_is_refused() {
        let mut ledger = SceneLedger::default();
        assert_eq!(ledger.available_texture_index(), Ok(0));
        assert!(ledger.pending().is_none());
        assert_eq!(begin(&mut ledger, 1, 1), 0);
        assert_eq!(
            ledger.begin(|texture_index| {
                Ok(PendingScene {
                    scene_id: 2,
                    pose: pose(1),
                    palette: PaletteId::Classic,
                    iteration_cap: 64,
                    level: RefinementLevel::Preview,
                    extent: [800, 600],
                    grid: grid([800, 600], RefinementLevel::Preview),
                    texture_index,
                    centre_revision: 1,
                    plane_origin_f64: ORIGIN,
                    precision_mode: MODE,
                    drop_reason: None,
                })
            }),
            Err(PresentError::SceneBusy { scene_id: 1 })
        );
        assert!(matches!(
            ledger.complete(measurement(1)),
            Some(SceneCompletion::Promoted(_))
        ));
        assert_eq!(begin(&mut ledger, 2, 1), 1);
        assert!(matches!(
            ledger.complete(measurement(2)),
            Some(SceneCompletion::Promoted(_))
        ));
        assert_eq!(begin(&mut ledger, 3, 1), 0);
    }

    #[test]
    fn freeing_the_promoted_record_grid_forgets_its_retained_scene() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        assert!(matches!(
            ledger.complete(measurement(1)),
            Some(SceneCompletion::Promoted(_))
        ));
        let retained = ledger
            .retained_grid()
            .expect("promoted scene retains its record grid")
            .clone();
        let unrelated = grid([64, 36], RefinementLevel::Preview);
        assert!(!ledger.forget_retained_grid(&unrelated));
        assert_eq!(ledger.retained().map(|frame| frame.scene_id), Some(1));
        assert!(ledger.forget_retained_grid(&retained));
        assert!(ledger.retained().is_none());
        assert!(ledger.retained_grid().is_none());
    }

    #[test]
    fn overwriting_retained_records_keeps_the_completed_image() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 72, 1);
        let sampled = match ledger.complete(measurement(72)) {
            Some(SceneCompletion::Promoted(frame)) => frame,
            completion => panic!("expected promoted scene, got {completion:?}"),
        };
        let retained = ledger
            .retained_grid()
            .expect("the promoted scene keeps its records")
            .clone();

        assert!(ledger.forget_retained_records(&retained));
        assert_eq!(
            ledger.retained().map(|frame| frame.scene_id),
            Some(sampled.scene_id)
        );
        assert!(ledger.retained_grid().is_none());
        assert!(!ledger.forget_retained_records(&retained));
    }

    #[test]
    fn ledger_builds_pending_scene_from_its_returned_texture_index() {
        let mut ledger = SceneLedger::default();
        assert_eq!(begin(&mut ledger, 1, 1), 0);
        assert_eq!(
            ledger.pending().map(|pending| pending.texture_index),
            Some(0)
        );
        assert!(matches!(
            ledger.complete(measurement(1)),
            Some(SceneCompletion::Promoted(_))
        ));
        assert_eq!(begin(&mut ledger, 2, 1), 1);
        assert_eq!(
            ledger.pending().map(|pending| pending.texture_index),
            Some(1)
        );
    }

    #[test]
    fn replaced_main_marks_only_the_pending_scene() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        ledger.complete(measurement(1));
        begin(&mut ledger, 2, 1);
        ledger.mark_replaced();
        assert_eq!(ledger.retained().map(|frame| frame.scene_id), Some(1));
        assert!(matches!(
            ledger.complete(measurement(2)),
            Some(SceneCompletion::Dropped {
                reason: DropReason::ReplacedMain,
                ..
            })
        ));
        assert_eq!(ledger.retained().map(|frame| frame.scene_id), Some(1));
    }

    #[test]
    fn reference_shift_rebases_retained_and_pending_once() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        ledger.complete(measurement(1));
        begin(&mut ledger, 2, 1);
        ledger.apply_reference_shift(2, 2, [4.0, -8.0]);
        assert_eq!(
            ledger
                .retained()
                .map(|frame| frame.pose.centre_from_reference_px),
            Some([7.0, 5.0])
        );
        assert_eq!(
            ledger
                .pending()
                .map(|pending| pending.pose.centre_from_reference_px),
            Some([7.0, 5.0])
        );
        ledger.apply_reference_shift(2, 2, [4.0, -8.0]);
        assert_eq!(
            ledger
                .retained()
                .map(|frame| frame.pose.centre_from_reference_px),
            Some([7.0, 5.0])
        );
    }

    #[test]
    fn reference_shift_uses_the_retained_sample_pose_not_a_newer_hot_pose() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        ledger.complete(measurement(1));
        let sampled = ledger.retained().expect("fixture retains a scene").pose;
        let mut newer_hot = sampled;
        newer_hot.epoch += 1;
        newer_hot.zoom_log2 += 2.0;
        newer_hot.grid_width *= 2;

        let mut expected = sampled;
        rebase_pose(&mut expected, &sampled, [4.0, -8.0]);
        let mut wrong = sampled;
        rebase_pose(&mut wrong, &newer_hot, [4.0, -8.0]);
        assert_ne!(
            expected.centre_from_reference_px,
            wrong.centre_from_reference_px
        );

        ledger.apply_reference_shift(2, 2, [4.0, -8.0]);
        assert_eq!(
            ledger
                .retained()
                .map(|frame| frame.pose.centre_from_reference_px),
            Some(expected.centre_from_reference_px)
        );
    }

    #[test]
    fn generation_alone_survives_but_cap_origin_or_mode_drops() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        ledger.apply_reference_shift(2, 2, [0.0; 2]);
        assert!(matches!(
            ledger.complete(measurement(1)),
            Some(SceneCompletion::Promoted(_))
        ));
        let plane = pose(2).plane;
        assert!(!ledger.invalidate_incompatible(64, ORIGIN, plane, MODE));
        assert!(ledger.invalidate_incompatible(128, ORIGIN, plane, MODE));

        begin(&mut ledger, 2, 2);
        ledger.invalidate_incompatible(64, [0.0, 0.0, 1.0, 0.0], plane, MODE);
        assert!(matches!(
            ledger.complete(measurement(2)),
            Some(SceneCompletion::Dropped {
                reason: DropReason::IncompatibleMain,
                ..
            })
        ));

        begin(&mut ledger, 3, 3);
        ledger.complete(measurement(3));
        assert!(ledger.invalidate_incompatible(
            64,
            ORIGIN,
            plane,
            PrecisionMode::PictureFast.as_str()
        ));
    }

    #[test]
    fn plane_span_invalidation_accepts_stabilizers_and_refuses_tilts() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        ledger.complete(measurement(1));
        let retained = ObjectAngles::JULIA;

        let rounded = ObjectAngles {
            rho_13: retained.rho_13 + 1.0e-9,
            ..retained
        };
        assert!(!ledger.invalidate_incompatible(
            64,
            ORIGIN,
            construct_plane(rounded).expect("rounded tilt constructs"),
            MODE
        ));

        let in_plane = ObjectAngles {
            rho_34: 0.3,
            ..retained
        };
        assert!(!ledger.invalidate_incompatible(
            64,
            ORIGIN,
            construct_plane(in_plane).expect("in-plane turn constructs"),
            MODE
        ));

        let tilted = ObjectAngles {
            rho_13: retained.rho_13 + 0.3,
            ..retained
        };
        assert!(ledger.invalidate_incompatible(
            64,
            ORIGIN,
            construct_plane(tilted).expect("tilt constructs"),
            MODE
        ));
    }

    #[test]
    fn exposed_warp_keeps_the_frame_loop_due_until_the_next_scene_completes() {
        let mut latch = ExposureLatch::default();
        latch.observe_warp(true);
        assert!(latch.due());
        latch.observe_warp(false);
        assert!(latch.due());
        latch.scene_completed();
        assert!(!latch.due());
    }

    #[test]
    fn accepted_exposed_final_survives_a_completed_preview_until_the_new_final_arrives() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        let SceneCompletion::Promoted(mut first) = ledger
            .complete(measurement(1))
            .expect("first scene completes")
        else {
            panic!("first scene must promote");
        };
        first.level = RefinementLevel::Final;
        first.extent = [1_920, 1_080];
        ledger.retained = Some(first);

        begin(&mut ledger, 2, 1);
        assert!(matches!(
            ledger.complete_preserving_accepted_best(measurement(2), true),
            Some(SceneCompletion::KeptBest(SceneFrame {
                scene_id: 2,
                level: RefinementLevel::Preview,
                ..
            }))
        ));
        assert_eq!(ledger.retained().map(|frame| frame.scene_id), Some(1));
        assert_eq!(ledger.available_texture_index(), Ok(1));

        ledger
            .begin(|texture_index| {
                let mut final_pose = pose(1);
                final_pose.grid_width = 1_920;
                final_pose.grid_height = 1_080;
                Ok(PendingScene {
                    scene_id: 3,
                    pose: final_pose,
                    palette: PaletteId::Classic,
                    iteration_cap: 64,
                    level: RefinementLevel::Final,
                    extent: [1_920, 1_080],
                    grid: grid([1_920, 1_080], RefinementLevel::Final),
                    texture_index,
                    centre_revision: 1,
                    plane_origin_f64: ORIGIN,
                    precision_mode: MODE,
                    drop_reason: None,
                })
            })
            .expect("new Final uses the non-retained texture");
        assert!(matches!(
            ledger.complete_preserving_accepted_best(measurement(3), true),
            Some(SceneCompletion::Promoted(SceneFrame { scene_id: 3, .. }))
        ));
        assert_eq!(ledger.retained().map(|frame| frame.scene_id), Some(3));
    }

    #[test]
    fn refused_best_warp_allows_a_preview_to_become_the_source() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        ledger.complete(measurement(1));
        if let Some(retained) = &mut ledger.retained {
            retained.level = RefinementLevel::Final;
        }
        begin(&mut ledger, 2, 1);
        assert!(matches!(
            ledger.complete_preserving_accepted_best(measurement(2), false),
            Some(SceneCompletion::Promoted(SceneFrame { scene_id: 2, .. }))
        ));
        assert_eq!(ledger.retained().map(|frame| frame.scene_id), Some(2));
    }
}
