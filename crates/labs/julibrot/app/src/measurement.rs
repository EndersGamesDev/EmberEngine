//! Bounded timer probing, adaptive repetition, summaries, and second-frame policy.

use thiserror::Error;

/// Named untimed warm-ups before each adaptive sample series.
pub const ADAPTIVE_WARM_UPS: u32 = 3;
/// Timed observations in one admitted series.
pub const ADAPTIVE_SAMPLES: usize = 15;
/// Minimum timer quanta crossed by one batch.
pub const TARGET_TIMER_QUANTA: u32 = 32;
/// Maximum repeated submissions in one batch.
pub const MAX_ADAPTIVE_REPEATS: u32 = 4_096;
/// Maximum target duration of one batch.
pub const MAX_BATCH_MS: f64 = 250.0;
/// Maximum active wall of one suite.
pub const SUITE_DEADLINE_MS: f64 = 30_000.0;
/// Maximum performance timer reads in the resolution probe.
pub const TIMER_READ_LIMIT: u32 = 4_000_000;
/// Positive timer transitions requested by the resolution probe.
pub const TIMER_TRANSITION_TARGET: u32 = 32;
/// Active timer-probe wall.
pub const TIMER_PROBE_DEADLINE_MS: f64 = 500.0;
/// Second-frame threshold selecting continuous or on-demand work.
pub const CONTINUOUS_FRAME_THRESHOLD_MS: f64 = 100.0;

/// Bounded measurement-plan refusal.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum MeasurementError {
    /// A timer or observed duration was non-finite, negative, or zero where positive was required.
    #[error("measurement input is not a finite positive duration")]
    InvalidDuration,
    /// Thirty-two timer quanta exceed the fixed batch target cap.
    #[error("32 timer quanta require {target_ms} ms, above the 250 ms batch cap")]
    TimerTooCoarse {
        /// Requested target duration.
        target_ms: f64,
    },
    /// Maximum repetition could not cross the target duration.
    #[error("4096 repeats did not cross the {target_ms} ms adaptive target")]
    RepeatLimit {
        /// Requested target duration.
        target_ms: f64,
    },
    /// A summary did not contain exactly 15 finite observations.
    #[error("measurement summary requires exactly 15 finite observations")]
    InvalidSampleCount,
}

/// Honest outcome of the bounded timer-resolution probe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerProbeFacts {
    /// Smallest observed positive transition, or unavailable.
    pub quantum_ms: Option<f64>,
    /// Number of reads performed after the initial read.
    pub reads: u32,
    /// Number of positive transitions observed.
    pub positive_transitions: u32,
    /// Number of equal consecutive readings.
    pub zero_transitions: u32,
    /// Observed active probe wall.
    pub wall_ms: f64,
}

/// Executes the bounded timer probe against an injected monotonic source.
#[must_use]
pub fn probe_timer(mut now_ms: impl FnMut() -> f64) -> TimerProbeFacts {
    let started = now_ms();
    let mut previous = started;
    let mut reads = 0;
    let mut positive_transitions = 0;
    let mut zero_transitions = 0;
    let mut quantum_ms = None::<f64>;
    while reads < TIMER_READ_LIMIT && positive_transitions < TIMER_TRANSITION_TARGET {
        let current = now_ms();
        reads += 1;
        let delta = current - previous;
        if delta > 0.0 && delta.is_finite() {
            positive_transitions += 1;
            quantum_ms = Some(quantum_ms.map_or(delta, |minimum| minimum.min(delta)));
        } else if delta == 0.0 {
            zero_transitions += 1;
        }
        previous = current;
        if current - started >= TIMER_PROBE_DEADLINE_MS {
            break;
        }
    }
    TimerProbeFacts {
        quantum_ms,
        reads,
        positive_transitions,
        zero_transitions,
        wall_ms: (previous - started).max(0.0),
    }
}

/// Exact adaptive repetition arithmetic shared by scene and warp suites.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptivePlan {
    /// Smallest positive timer transition.
    pub timer_quantum_ms: f64,
    /// Required batch duration, exactly 32 timer quanta.
    pub target_ms: f64,
}

impl AdaptivePlan {
    /// Creates a plan from a measured timer quantum.
    ///
    /// # Errors
    ///
    /// Returns a typed non-finite or 250 ms target refusal.
    pub fn new(timer_quantum_ms: f64) -> Result<Self, MeasurementError> {
        if !timer_quantum_ms.is_finite() || timer_quantum_ms <= 0.0 {
            return Err(MeasurementError::InvalidDuration);
        }
        let target_ms = timer_quantum_ms * f64::from(TARGET_TIMER_QUANTA);
        if target_ms > MAX_BATCH_MS {
            return Err(MeasurementError::TimerTooCoarse { target_ms });
        }
        Ok(Self {
            timer_quantum_ms,
            target_ms,
        })
    }

    /// Chooses the next repeat count after a batch below target.
    ///
    /// # Errors
    ///
    /// Returns a typed duration or 4,096-repeat refusal.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn next_repeats(
        self,
        current_repeats: u32,
        elapsed_ms: f64,
    ) -> Result<u32, MeasurementError> {
        if current_repeats == 0 || !elapsed_ms.is_finite() || elapsed_ms <= 0.0 {
            return Err(MeasurementError::InvalidDuration);
        }
        if elapsed_ms >= self.target_ms {
            return Ok(current_repeats);
        }
        if current_repeats >= MAX_ADAPTIVE_REPEATS {
            return Err(MeasurementError::RepeatLimit {
                target_ms: self.target_ms,
            });
        }
        let scaled = (f64::from(current_repeats) * self.target_ms / elapsed_ms).ceil();
        let scaled = scaled.min(f64::from(MAX_ADAPTIVE_REPEATS)) as u32;
        Ok(scaled.max(current_repeats + 1))
    }
}

/// Median and nearest-rank p95 of exactly 15 normalized observations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleSummary {
    /// Middle sorted observation.
    pub median_ms: f64,
    /// Nearest-rank 95th percentile at `ceil(0.95n)`.
    pub p95_ms: f64,
}

impl SampleSummary {
    /// Summarizes exactly the admitted sample count.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal unless all 15 observations are finite and nonnegative.
    pub fn new(samples: &[f64]) -> Result<Self, MeasurementError> {
        if samples.len() != ADAPTIVE_SAMPLES
            || !samples
                .iter()
                .all(|sample| sample.is_finite() && *sample >= 0.0)
        {
            return Err(MeasurementError::InvalidSampleCount);
        }
        let mut ordered = samples.to_vec();
        ordered.sort_by(f64::total_cmp);
        let p95_index = (ordered.len() * 95).div_ceil(100) - 1;
        Ok(Self {
            median_ms: ordered[ordered.len() / 2],
            p95_ms: ordered[p95_index],
        })
    }
}

/// Animation/refinement behavior selected only by the second completed frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FramePolicy {
    /// No second completed frame exists yet.
    Undecided,
    /// Work proceeds only for input or an explicit frame request.
    SingleFrameOnDemand,
    /// Continuous animation and refinement are admitted.
    Continuous,
}

/// Label returned for the first warm-up and second decision observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameObservation {
    /// First completed observation is excluded and labelled warm-up.
    WarmUp,
    /// Second completed observation selected the returned policy.
    Decision(FramePolicy),
    /// A later observation leaves the existing decision unchanged.
    Recorded(FramePolicy),
}

/// Resettable two-frame selector for pipeline, texture, and view changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramePolicyTracker {
    completed: u32,
    policy: FramePolicy,
}

impl FramePolicyTracker {
    /// Creates an undecided tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            completed: 0,
            policy: FramePolicy::Undecided,
        }
    }

    /// Clears prior observations after a warm-up-causing change.
    pub const fn reset(&mut self) {
        self.completed = 0;
        self.policy = FramePolicy::Undecided;
    }

    /// Records one completed frame; the first is warm-up and only the second decides.
    ///
    /// # Errors
    ///
    /// Returns a duration refusal for negative or non-finite wall time.
    pub fn record(&mut self, wall_ms: f64) -> Result<FrameObservation, MeasurementError> {
        if !wall_ms.is_finite() || wall_ms < 0.0 {
            return Err(MeasurementError::InvalidDuration);
        }
        self.completed = self.completed.saturating_add(1);
        match self.completed {
            1 => Ok(FrameObservation::WarmUp),
            2 => {
                self.policy = if wall_ms > CONTINUOUS_FRAME_THRESHOLD_MS {
                    FramePolicy::SingleFrameOnDemand
                } else {
                    FramePolicy::Continuous
                };
                Ok(FrameObservation::Decision(self.policy))
            }
            _ => Ok(FrameObservation::Recorded(self.policy)),
        }
    }

    /// Returns the current decision.
    #[must_use]
    pub const fn policy(self) -> FramePolicy {
        self.policy
    }
}

impl Default for FramePolicyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_probe_stops_at_32_positive_transitions_and_keeps_the_minimum() {
        let mut time = 0.0;
        let mut read = 0_u32;
        let facts = probe_timer(|| {
            read += 1;
            if !read.is_multiple_of(3) {
                time += 0.25;
            }
            time
        });
        assert_eq!(facts.positive_transitions, TIMER_TRANSITION_TARGET);
        assert_eq!(facts.quantum_ms, Some(0.25));
        assert!(facts.reads <= TIMER_READ_LIMIT);
    }

    #[test]
    fn adaptive_repeats_scale_up_and_refuse_the_hard_limit() {
        let plan = AdaptivePlan::new(0.5).expect("16 ms target is admitted");
        assert_eq!(plan.target_ms, 16.0);
        assert_eq!(plan.next_repeats(1, 4.0), Ok(4));
        assert_eq!(plan.next_repeats(4, 16.0), Ok(4));
        assert_eq!(
            plan.next_repeats(MAX_ADAPTIVE_REPEATS, 1.0),
            Err(MeasurementError::RepeatLimit { target_ms: 16.0 })
        );
    }

    #[test]
    fn summary_uses_middle_and_nearest_rank_p95() {
        let samples = [
            14.0, 0.0, 7.0, 1.0, 8.0, 2.0, 9.0, 3.0, 10.0, 4.0, 11.0, 5.0, 12.0, 6.0, 13.0,
        ];
        assert_eq!(
            SampleSummary::new(&samples),
            Ok(SampleSummary {
                median_ms: 7.0,
                p95_ms: 14.0
            })
        );
    }

    #[test]
    fn first_frame_is_warmup_and_only_second_frame_decides() {
        let mut tracker = FramePolicyTracker::new();
        assert_eq!(tracker.record(900.0), Ok(FrameObservation::WarmUp));
        assert_eq!(tracker.policy(), FramePolicy::Undecided);
        assert_eq!(
            tracker.record(100.01),
            Ok(FrameObservation::Decision(FramePolicy::SingleFrameOnDemand))
        );
        tracker.reset();
        assert_eq!(tracker.record(400.0), Ok(FrameObservation::WarmUp));
        assert_eq!(
            tracker.record(100.0),
            Ok(FrameObservation::Decision(FramePolicy::Continuous))
        );
    }
}
