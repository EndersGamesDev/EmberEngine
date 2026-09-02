use crate::{FenceRefusal, SampleClass, SubmissionKind, SubmissionMeasurement};

/// One bounded asynchronous fence's CPU timing and poll ledger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FenceLedger {
    kind: SubmissionKind,
    id: u64,
    source_scene_id: Option<u64>,
    sample_class: SampleClass,
    started_ms: f64,
    first_poll_ms: Option<f64>,
    polls: u32,
    deadline_ms: f64,
    max_polls: u32,
}

/// Result of one cooperative observation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FenceDecision {
    /// Callback has not fired and the budget remains.
    Pending,
    /// Callback succeeded; the contained measurement is complete.
    Complete(SubmissionMeasurement),
    /// Callback or a bounded policy refused the fence.
    Refused {
        reason: FenceRefusal,
        polls: u32,
        wall_ms: f64,
    },
}

impl FenceLedger {
    pub(crate) fn new(
        kind: SubmissionKind,
        id: u64,
        source_scene_id: Option<u64>,
        sample_class: SampleClass,
        started_ms: f64,
        deadline_ms: f64,
        max_polls: u32,
    ) -> Self {
        Self {
            kind,
            id,
            source_scene_id,
            sample_class,
            started_ms,
            first_poll_ms: None,
            polls: 0,
            deadline_ms,
            max_polls,
        }
    }

    pub(crate) const fn id(self) -> u64 {
        self.id
    }

    pub(crate) const fn polls(self) -> u32 {
        self.polls
    }

    pub(crate) fn observe(
        &mut self,
        now_ms: f64,
        callback: Option<Result<(), ()>>,
    ) -> FenceDecision {
        let wall_ms = nonnegative_elapsed(self.started_ms, now_ms);
        if self.first_poll_ms.is_none() {
            self.first_poll_ms = Some(now_ms);
        }
        self.polls = self.polls.saturating_add(1);
        if matches!(callback, Some(Err(()))) || !now_ms.is_finite() {
            return FenceDecision::Refused {
                reason: FenceRefusal::Device,
                polls: self.polls,
                wall_ms,
            };
        }
        if callback == Some(Ok(())) {
            return FenceDecision::Complete(SubmissionMeasurement {
                kind: self.kind,
                id: self.id,
                source_scene_id: self.source_scene_id,
                sample_class: self.sample_class,
                wall_ms,
                fence_wait_ms: self
                    .first_poll_ms
                    .map_or(0.0, |first| nonnegative_elapsed(first, now_ms)),
                polls: self.polls,
            });
        }
        if wall_ms >= self.deadline_ms {
            return FenceDecision::Refused {
                reason: FenceRefusal::Deadline,
                polls: self.polls,
                wall_ms,
            };
        }
        if self.polls >= self.max_polls {
            return FenceDecision::Refused {
                reason: FenceRefusal::PollLimit,
                polls: self.polls,
                wall_ms,
            };
        }
        FenceDecision::Pending
    }
}

fn nonnegative_elapsed(started_ms: f64, now_ms: f64) -> f64 {
    if started_ms.is_finite() && now_ms.is_finite() {
        (now_ms - started_ms).max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger(deadline_ms: f64, max_polls: u32) -> FenceLedger {
        FenceLedger::new(
            SubmissionKind::Warp,
            7,
            Some(3),
            SampleClass::Measured,
            100.0,
            deadline_ms,
            max_polls,
        )
    }

    #[test]
    fn callback_completion_counts_every_observation_and_separates_wait() {
        let mut fence = ledger(30_000.0, 4_096);
        assert_eq!(fence.observe(102.0, None), FenceDecision::Pending);
        let FenceDecision::Complete(measurement) = fence.observe(105.5, Some(Ok(()))) else {
            panic!("successful callback must complete");
        };
        assert_eq!(measurement.id, 7);
        assert_eq!(measurement.polls, 2);
        assert_eq!(measurement.wall_ms, 5.5);
        assert_eq!(measurement.fence_wait_ms, 3.5);
    }

    #[test]
    fn deadline_poll_limit_and_mapping_error_are_distinct() {
        let mut deadline = ledger(10.0, 100);
        assert!(matches!(
            deadline.observe(110.0, None),
            FenceDecision::Refused {
                reason: FenceRefusal::Deadline,
                ..
            }
        ));
        let mut poll_limit = ledger(1_000.0, 2);
        assert_eq!(poll_limit.observe(101.0, None), FenceDecision::Pending);
        assert!(matches!(
            poll_limit.observe(102.0, None),
            FenceDecision::Refused {
                reason: FenceRefusal::PollLimit,
                polls: 2,
                ..
            }
        ));
        let mut device = ledger(1_000.0, 2);
        assert!(matches!(
            device.observe(101.0, Some(Err(()))),
            FenceDecision::Refused {
                reason: FenceRefusal::Device,
                ..
            }
        ));
    }
}
