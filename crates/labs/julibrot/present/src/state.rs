use ember_julibrot_math::Pose;

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
    pub texture_index: u32,
    pub centre_revision: u32,
    pub plane_origin_f64: [f64; 4],
    pub drop_reason: Option<DropReason>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneCompletion {
    Promoted(SceneFrame),
    Dropped {
        pending: PendingScene,
        reason: DropReason,
        measurement: SubmissionMeasurement,
    },
}

/// Pure two-index ledger; GPU resources mirror these identities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneLedger {
    retained: Option<SceneFrame>,
    pending: Option<PendingScene>,
}

impl SceneLedger {
    pub const fn available_texture_index(&self) -> Result<u32, PresentError> {
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

    pub fn begin(&mut self, mut incoming: PendingScene) -> Result<u32, PresentError> {
        let texture_index = self.available_texture_index()?;
        incoming.texture_index = texture_index;
        self.pending = Some(incoming);
        Ok(texture_index)
    }

    pub fn complete(&mut self, measurement: SubmissionMeasurement) -> Option<SceneCompletion> {
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
            measurement,
        };
        self.retained = Some(frame.clone());
        Some(SceneCompletion::Promoted(frame))
    }

    pub const fn cancel_pending(&mut self) -> Option<PendingScene> {
        self.pending.take()
    }

    #[allow(clippy::float_cmp)]
    pub fn invalidate_incompatible(
        &mut self,
        iteration_cap: u32,
        plane_origin_f64: [f64; 4],
    ) -> bool {
        let retained_invalid = self.retained.as_ref().is_some_and(|frame| {
            frame.iteration_cap != iteration_cap || frame.plane_origin_f64 != plane_origin_f64
        });
        if retained_invalid {
            self.retained = None;
        }
        if let Some(pending) = &mut self.pending
            && (pending.iteration_cap != iteration_cap
                || pending.plane_origin_f64 != plane_origin_f64)
        {
            pending.drop_reason = Some(DropReason::IncompatibleMain);
        }
        retained_invalid
    }

    pub fn apply_reference_shift(
        &mut self,
        accepted_pose: &Pose,
        new_generation: u32,
        new_revision: u32,
        shift_px: [f64; 2],
    ) {
        if let Some(frame) = &mut self.retained
            && frame.centre_revision != new_revision
        {
            rebase_pose(&mut frame.pose, accepted_pose, shift_px);
            frame.pose.orbit_generation = new_generation;
            frame.centre_revision = new_revision;
        }
        if let Some(pending) = &mut self.pending
            && pending.centre_revision != new_revision
        {
            rebase_pose(&mut pending.pose, accepted_pose, shift_px);
            pending.pose.orbit_generation = new_generation;
            pending.centre_revision = new_revision;
        }
    }

    pub fn mark_replaced(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.drop_reason = Some(DropReason::ReplacedMain);
        }
    }

    pub const fn retained(&self) -> Option<&SceneFrame> {
        self.retained.as_ref()
    }

    pub const fn pending(&self) -> Option<&PendingScene> {
        self.pending.as_ref()
    }
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
    use ember_julibrot_math::{Plane, ViewMode};

    use super::*;
    use crate::{SampleClass, SubmissionKind, SubmissionMeasurement};

    const ORIGIN: [f64; 4] = [0.0; 4];

    fn pose(generation: u32) -> Pose {
        Pose {
            epoch: 1,
            orbit_generation: generation,
            plane: Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            plane_theta_1: 0.0,
            plane_theta_2: 0.0,
            zoom_log2: 20.0,
            view_theta_1: 0.0,
            grid_width: 800,
            grid_height: 600,
            view: ViewMode::Flat,
            centre_from_reference_px: [11.0, -3.0],
        }
    }

    fn measurement(scene_id: u64) -> SubmissionMeasurement {
        SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id: scene_id,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            wall_ms: 2.0,
            fence_wait_ms: 1.0,
            polls: 2,
        }
    }

    fn begin(ledger: &mut SceneLedger, scene_id: u64, generation: u32) -> u32 {
        ledger
            .begin(PendingScene {
                scene_id,
                pose: pose(generation),
                palette: PaletteId::Classic,
                iteration_cap: 64,
                level: RefinementLevel::Preview,
                extent: [800, 600],
                texture_index: 0,
                centre_revision: generation,
                plane_origin_f64: ORIGIN,
                drop_reason: None,
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
            ledger.begin(PendingScene {
                scene_id: 2,
                pose: pose(1),
                palette: PaletteId::Classic,
                iteration_cap: 64,
                level: RefinementLevel::Preview,
                extent: [800, 600],
                texture_index: 0,
                centre_revision: 1,
                plane_origin_f64: ORIGIN,
                drop_reason: None,
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
        let accepted = pose(2);
        ledger.apply_reference_shift(&accepted, 2, 2, [4.0, -8.0]);
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
        ledger.apply_reference_shift(&accepted, 2, 2, [4.0, -8.0]);
        assert_eq!(
            ledger
                .retained()
                .map(|frame| frame.pose.centre_from_reference_px),
            Some([7.0, 5.0])
        );
    }

    #[test]
    fn generation_alone_survives_but_cap_or_origin_drops() {
        let mut ledger = SceneLedger::default();
        begin(&mut ledger, 1, 1);
        ledger.apply_reference_shift(&pose(2), 2, 2, [0.0; 2]);
        assert!(matches!(
            ledger.complete(measurement(1)),
            Some(SceneCompletion::Promoted(_))
        ));
        assert!(!ledger.invalidate_incompatible(64, ORIGIN));
        assert!(ledger.invalidate_incompatible(128, ORIGIN));

        begin(&mut ledger, 2, 2);
        ledger.invalidate_incompatible(64, [0.0, 0.0, 1.0, 0.0]);
        assert!(matches!(
            ledger.complete(measurement(2)),
            Some(SceneCompletion::Dropped {
                reason: DropReason::IncompatibleMain,
                ..
            })
        ));
    }
}
