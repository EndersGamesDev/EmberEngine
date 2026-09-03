//! Per-edit, per-level timing records assembled from existing application boundaries.

use std::collections::VecDeque;

use ember_julibrot_kernels::RefinementLevel;
use ember_julibrot_present::SubmissionMeasurement;
use serde::Serialize;

/// Maximum number of per-level observations retained for the facts overlay.
pub const LEVEL_TIMING_CAPACITY: usize = 64;

/// Stable JSON name for a progressive-refinement level.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TimingLevel {
    /// Coarse feedback level.
    Preview,
    /// Half-extent intermediate level.
    Interactive,
    /// Full delivered extent and cap.
    Final,
}

impl From<RefinementLevel> for TimingLevel {
    fn from(level: RefinementLevel) -> Self {
        match level {
            RefinementLevel::Preview => Self::Preview,
            RefinementLevel::Interactive => Self::Interactive,
            RefinementLevel::Final => Self::Final,
        }
    }
}

/// One level's measurements, grouped by the edit that requested it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct LevelTimingRecord {
    /// Centre revision identifying the edit.
    pub edit: u32,
    /// Progressive-refinement level.
    pub level: TimingLevel,
    /// Kernel-only GPU wall, unavailable without another fence.
    pub dispatch_us: Option<u64>,
    /// Existing scene-fence wall.
    pub scene_us: Option<u64>,
    /// First existing warp-fence wall that sampled this scene.
    pub warp_us: Option<u64>,
    /// Worker-measured reference computation wall for this edit.
    pub worker_reference_us: Option<u64>,
    /// Credit-shaper wait wall, unavailable in the current worker response.
    pub credit_wait_us: Option<u64>,
    /// True when the submitted scene was dropped or refused before promotion.
    pub discarded: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrackedLevel {
    scene_id: u64,
    record: LevelTimingRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkerTiming {
    edit: u32,
    reference_us: u64,
    credit_wait_us: Option<u64>,
}

/// Bounded application ledger populated only from measured completion records.
#[derive(Debug, Default)]
pub struct LevelTimingLedger {
    levels: VecDeque<TrackedLevel>,
    workers: VecDeque<WorkerTiming>,
}

impl LevelTimingLedger {
    /// Records the accepted worker measurement for one edit.
    pub fn record_worker(&mut self, edit: u32, reference_us: u32, credit_wait_us: Option<u64>) {
        if let Some(previous) = self.workers.iter_mut().find(|item| item.edit == edit) {
            previous.reference_us = u64::from(reference_us);
            previous.credit_wait_us = credit_wait_us;
            return;
        }
        if self.workers.len() == LEVEL_TIMING_CAPACITY {
            self.workers.pop_front();
        }
        self.workers.push_back(WorkerTiming {
            edit,
            reference_us: u64::from(reference_us),
            credit_wait_us,
        });
    }

    /// Starts one record when app successfully submits its scene fence.
    pub fn begin_scene(&mut self, edit: u32, scene_id: u64, level: RefinementLevel) {
        if self.levels.len() == LEVEL_TIMING_CAPACITY {
            self.levels.pop_front();
        }
        let worker = self.workers.iter().rev().find(|item| item.edit == edit);
        self.levels.push_back(TrackedLevel {
            scene_id,
            record: LevelTimingRecord {
                edit,
                level: level.into(),
                dispatch_us: None,
                scene_us: None,
                warp_us: None,
                worker_reference_us: worker.map(|item| item.reference_us),
                credit_wait_us: worker.and_then(|item| item.credit_wait_us),
                discarded: false,
            },
        });
    }

    /// Attaches the existing scene-fence wall to its submitted level.
    pub fn complete_scene(&mut self, scene_id: u64, measurement: SubmissionMeasurement) {
        if let Some(level) = self.level_mut(scene_id) {
            level.record.scene_us = milliseconds_to_microseconds(measurement.wall_ms);
        }
    }

    /// Marks a scene as discarded and preserves any completed fence wall it carried.
    pub fn drop_scene(&mut self, scene_id: u64, measurement: Option<SubmissionMeasurement>) {
        if let Some(level) = self.level_mut(scene_id) {
            level.record.discarded = true;
            level.record.scene_us =
                measurement.and_then(|value| milliseconds_to_microseconds(value.wall_ms));
        }
    }

    /// Attaches the first warp wall whose source identity names this scene.
    pub fn complete_warp(&mut self, measurement: SubmissionMeasurement) {
        let Some(scene_id) = measurement.source_scene_id else {
            return;
        };
        if let Some(level) = self.level_mut(scene_id)
            && level.record.warp_us.is_none()
        {
            level.record.warp_us = milliseconds_to_microseconds(measurement.wall_ms);
        }
    }

    /// Returns the bounded ring in oldest-to-newest JSON order.
    #[must_use]
    pub fn records(&self) -> Vec<LevelTimingRecord> {
        self.levels.iter().map(|item| item.record).collect()
    }

    fn level_mut(&mut self, scene_id: u64) -> Option<&mut TrackedLevel> {
        self.levels
            .iter_mut()
            .rev()
            .find(|item| item.scene_id == scene_id)
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn milliseconds_to_microseconds(milliseconds: f64) -> Option<u64> {
    (milliseconds.is_finite() && milliseconds >= 0.0)
        .then(|| (milliseconds * 1_000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use ember_julibrot_present::{SampleClass, SubmissionKind};

    use super::*;

    fn measurement(
        kind: SubmissionKind,
        id: u64,
        source_scene_id: Option<u64>,
        wall_ms: f64,
    ) -> SubmissionMeasurement {
        SubmissionMeasurement {
            kind,
            id,
            source_scene_id,
            sample_class: SampleClass::Measured,
            wall_ms,
            fence_wait_ms: wall_ms,
            polls: 1,
        }
    }

    #[test]
    fn ring_preserves_measured_boundaries_and_evicts_oldest() {
        let mut ledger = LevelTimingLedger::default();
        let capacity = u32::try_from(LEVEL_TIMING_CAPACITY).expect("capacity fits u32");
        for edit in 0..=capacity {
            let scene_id = u64::from(edit) + 1;
            ledger.record_worker(edit, edit + 10, None);
            ledger.begin_scene(edit, scene_id, RefinementLevel::Final);
            ledger.complete_scene(
                scene_id,
                measurement(SubmissionKind::Scene, scene_id, None, 2.25),
            );
            ledger.complete_warp(measurement(
                SubmissionKind::Warp,
                scene_id + 100,
                Some(scene_id),
                0.75,
            ));
        }
        let records = ledger.records();
        assert_eq!(records.len(), LEVEL_TIMING_CAPACITY);
        assert_eq!(records[0].edit, 1);
        assert_eq!(records.last().map(|item| item.scene_us), Some(Some(2_250)));
        assert_eq!(records.last().map(|item| item.warp_us), Some(Some(750)));
        assert_eq!(records.last().map(|item| item.dispatch_us), Some(None));
        assert_eq!(records.last().map(|item| item.credit_wait_us), Some(None));
    }
}
