//! Canonical math adapter and cooperatively bounded reference-orbit work.

use core::num::NonZeroU32;

use ember_julibrot_math::{
    ComputedOrbit, EscapeParams, MathError, OrbitStep, PrecisionPlan, ReferenceOrbitBuilder,
};

use crate::{ChannelError, ErrorCode, OrbitRequest};

/// Maximum reference iterations evaluated between browser-task yields.
pub const ORBIT_CHUNK_MAX_ITERATIONS: u32 = 64;
/// Maximum measured compute wall targeted between browser-task yields.
pub const ORBIT_CHUNK_MAX_US: u32 = 2_000;

const PRECISION_POLICY_DIGITS: u32 = 300;
const LOG10_2: f64 = core::f64::consts::LOG10_2;

/// Stable detail values carried with [`ErrorCode::MathFailure`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MathFailureCode {
    /// A numeric input or result was not finite.
    NonFinite = 1,
    /// A grid extent was zero.
    InvalidExtent = 2,
    /// Maximum iteration was zero.
    InvalidMaxIter = 3,
    /// The fixed bailout was not used.
    InvalidBailout = 4,
    /// A VIEW control was invalid.
    InvalidViewControls = 5,
    /// Plane rounding exceeded its bound.
    PlaneRoundingBound = 6,
    /// Centre bytes were not canonical math input.
    InvalidCentreEncoding = 7,
    /// Bignum coordinates disagreed on precision.
    PrecisionMismatch = 8,
    /// A scale exponent overflowed.
    ScaleExponentOverflow = 9,
    /// A warp matrix was degenerate.
    DegenerateWarp = 10,
    /// An orbit length was unrepresentable.
    OrbitTooLong = 11,
    /// The orbit builder reached inconsistent state.
    InvalidOrbitState = 12,
    /// The reference orbit was empty.
    EmptyReferenceOrbit = 13,
    /// The transported precision plan was inconsistent.
    InvalidPrecisionPlan = 14,
    /// A checked counter overflowed.
    CounterOverflow = 15,
    /// A duration overflowed.
    DurationOverflow = 16,
    /// The 300-digit policy was exhausted.
    PrecisionExhausted = 17,
    /// Astro-float refused an operation.
    BigFloat = 18,
}

/// Monotonic microsecond clock used by native and browser compute loops.
pub trait MonotonicClock {
    /// Returns the current monotonic time in integer microseconds.
    fn now_us(&self) -> u64;
}

/// Result of one cooperatively bounded reference-orbit chunk.
#[derive(Clone, Debug, PartialEq)]
pub enum OrbitTaskPoll {
    /// More work remains and the caller must yield one browser task.
    Pending {
        /// Records held by the math builder so far.
        stored: u32,
        /// Iterations evaluated in this chunk.
        chunk_iterations: u32,
        /// Measured compute wall accumulated across chunks.
        compute_us: u32,
    },
    /// A policy-valid working orbit is ready, with deferred or completed verification facts.
    Complete {
        /// Math-owned reusable linear-memory records.
        orbit: ComputedOrbit,
        /// Measured compute wall accumulated across chunks.
        compute_us: u32,
    },
    /// A newer generation made this partial result stale.
    Cancelled {
        /// Generation whose work was discarded.
        generation: u32,
        /// Measured compute wall accumulated before cancellation.
        compute_us: u32,
    },
}

/// One decoded, validated, cooperatively stepped reference-orbit computation.
#[derive(Debug)]
pub struct ReferenceOrbitTask {
    generation: u32,
    builder: ReferenceOrbitBuilder,
    compute_us: u64,
}

impl ReferenceOrbitTask {
    /// Decodes a canonical request and starts its policy-selected reference builder.
    ///
    /// The clock includes centre decoding and builder initialization in `compute_us`.
    ///
    /// # Errors
    ///
    /// Returns a stable math or timing refusal for invalid centre, precision, or clock input.
    pub fn start<C: MonotonicClock>(
        request: &OrbitRequest,
        clock: &C,
    ) -> Result<Self, ChannelError> {
        let started = clock.now_us();
        let centre = request.centre().decode_math(request.precision_bits())?;
        let plan = precision_plan(request)?;
        let builder = ReferenceOrbitBuilder::new_with_policy(
            &centre,
            plan,
            EscapeParams::new(request.max_iter()),
            request.precision_mode(),
            request.reference_pass(),
        )
        .map_err(|error| math_error(&error))?;
        let compute_us = elapsed(started, clock.now_us())?;
        checked_compute_us(compute_us)?;
        Ok(Self {
            generation: request.generation(),
            builder,
            compute_us,
        })
    }

    /// Advances through at most 64 records or 2,000 measured microseconds.
    ///
    /// A caller yields one browser task after `Pending` and supplies the latest generation on the
    /// next call; a mismatch cancels without publishing partial records.
    ///
    /// # Errors
    ///
    /// Returns a stable math or timing refusal if computation or the monotonic clock fails.
    pub fn poll<C: MonotonicClock>(
        &mut self,
        latest_generation: u32,
        clock: &C,
    ) -> Result<OrbitTaskPoll, ChannelError> {
        if latest_generation != self.generation {
            return Ok(OrbitTaskPoll::Cancelled {
                generation: self.generation,
                compute_us: checked_compute_us(self.compute_us)?,
            });
        }
        let started = clock.now_us();
        let one = NonZeroU32::MIN;
        let mut last_stored = 0;
        for chunk_iterations in 1..=ORBIT_CHUNK_MAX_ITERATIONS {
            let step = self.builder.step(one).map_err(|error| math_error(&error))?;
            let chunk_us = elapsed(started, clock.now_us())?;
            match step {
                OrbitStep::Complete(orbit) => {
                    self.add_compute_us(chunk_us)?;
                    return Ok(OrbitTaskPoll::Complete {
                        orbit,
                        compute_us: checked_compute_us(self.compute_us)?,
                    });
                }
                OrbitStep::Pending { stored } if chunk_us >= u64::from(ORBIT_CHUNK_MAX_US) => {
                    self.add_compute_us(chunk_us)?;
                    return Ok(OrbitTaskPoll::Pending {
                        stored,
                        chunk_iterations,
                        compute_us: checked_compute_us(self.compute_us)?,
                    });
                }
                OrbitStep::Pending { stored } => last_stored = stored,
            }
        }
        let chunk_us = elapsed(started, clock.now_us())?;
        self.add_compute_us(chunk_us)?;
        Ok(OrbitTaskPoll::Pending {
            stored: last_stored,
            chunk_iterations: ORBIT_CHUNK_MAX_ITERATIONS,
            compute_us: checked_compute_us(self.compute_us)?,
        })
    }

    /// Returns the generation fixed at task construction.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    fn add_compute_us(&mut self, elapsed_us: u64) -> Result<(), ChannelError> {
        self.compute_us = self
            .compute_us
            .checked_add(elapsed_us)
            .ok_or_else(timing_overflow)?;
        checked_compute_us(self.compute_us).map(|_| ())
    }
}

fn precision_plan(request: &OrbitRequest) -> Result<PrecisionPlan, ChannelError> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let working_digits = (f64::from(request.precision_bits()) * LOG10_2).floor() as u32;
    let minimum_floor = request.depth_digits().saturating_add(8).max(1);
    if working_digits < minimum_floor || working_digits == 0 {
        return Err(math_error(&MathError::InvalidPrecisionPlan));
    }
    Ok(PrecisionPlan {
        floor_digits: minimum_floor,
        working_digits,
        requested_bits: request.precision_bits(),
        policy_digits: PRECISION_POLICY_DIGITS,
    })
}

fn elapsed(started: u64, finished: u64) -> Result<u64, ChannelError> {
    finished.checked_sub(started).ok_or_else(timing_overflow)
}

fn checked_compute_us(value: u64) -> Result<u32, ChannelError> {
    u32::try_from(value).map_err(|_| timing_overflow())
}

const fn timing_overflow() -> ChannelError {
    ChannelError::new(ErrorCode::TimingOverflow, 0, u32::MAX, 0)
}

/// Maps every math refusal to a stable worker wire detail.
#[doc(hidden)]
pub const fn math_error(error: &MathError) -> ChannelError {
    let detail = match error {
        MathError::NonFinite => MathFailureCode::NonFinite,
        MathError::InvalidExtent => MathFailureCode::InvalidExtent,
        MathError::InvalidMaxIter => MathFailureCode::InvalidMaxIter,
        MathError::InvalidBailout => MathFailureCode::InvalidBailout,
        MathError::InvalidViewControls => MathFailureCode::InvalidViewControls,
        MathError::PlaneRoundingBound => MathFailureCode::PlaneRoundingBound,
        MathError::InvalidCentreEncoding => MathFailureCode::InvalidCentreEncoding,
        MathError::PrecisionMismatch => MathFailureCode::PrecisionMismatch,
        MathError::ScaleExponentOverflow => MathFailureCode::ScaleExponentOverflow,
        MathError::DegenerateWarp => MathFailureCode::DegenerateWarp,
        MathError::OrbitTooLong => MathFailureCode::OrbitTooLong,
        MathError::InvalidOrbitState => MathFailureCode::InvalidOrbitState,
        MathError::EmptyReferenceOrbit => MathFailureCode::EmptyReferenceOrbit,
        MathError::InvalidPrecisionPlan => MathFailureCode::InvalidPrecisionPlan,
        MathError::CounterOverflow => MathFailureCode::CounterOverflow,
        MathError::DurationOverflow => MathFailureCode::DurationOverflow,
        MathError::PrecisionExhausted { .. } => MathFailureCode::PrecisionExhausted,
        MathError::BigFloat => MathFailureCode::BigFloat,
    };
    ChannelError::new(ErrorCode::MathFailure, detail as u32, 0, 0)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::time::Instant;

    use ember_julibrot_math::{
        BigCentre, ComputedOrbit, EscapeParams, PrecisionMode, ReferenceOrbitRecord, ReferencePass,
        perturb_scaled_f64, precision_for,
    };

    use super::{
        MathFailureCode, MonotonicClock, ORBIT_CHUNK_MAX_ITERATIONS, OrbitTaskPoll,
        ReferenceOrbitTask,
    };
    use crate::wire::{OrbitVerificationFacts, Pool, WireBuffer};
    use crate::{Admission, EncodedCentre, OrbitReason, OrbitRequest, ProducerShaper};

    struct StepClock {
        now: Cell<u64>,
        step: u64,
    }

    impl StepClock {
        const fn new(step: u64) -> Self {
            Self {
                now: Cell::new(0),
                step,
            }
        }
    }

    impl MonotonicClock for StepClock {
        fn now_us(&self) -> u64 {
            let now = self.now.get();
            self.now.set(now + self.step);
            now
        }
    }

    struct WallClock {
        started: Instant,
    }

    impl WallClock {
        fn new() -> Self {
            Self {
                started: Instant::now(),
            }
        }
    }

    impl MonotonicClock for WallClock {
        fn now_us(&self) -> u64 {
            u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX)
        }
    }

    fn request(centre: &BigCentre, max_iter: u32) -> OrbitRequest {
        OrbitRequest::new(
            7,
            EncodedCentre::encode_math(centre, 19).unwrap(),
            0,
            128,
            max_iter,
            PrecisionMode::Deterministic,
            OrbitReason::INITIAL,
        )
        .unwrap()
    }

    fn measured_orbit(max_iter: u32) -> (ComputedOrbit, u32, u32) {
        let plan = precision_for(100.0, 960, max_iter).expect("measurement precision plan");
        let centre =
            BigCentre::from_f64([0.0; 4], plan.requested_bits).expect("measurement centre");
        let request = OrbitRequest::new(
            1,
            EncodedCentre::encode_math(&centre, 1).expect("measurement encoding"),
            31,
            plan.requested_bits,
            max_iter,
            OrbitReason::INITIAL,
        )
        .expect("measurement request")
        .with_precision_policy(PrecisionMode::PictureFast, ReferencePass::Preview);
        let clock = WallClock::new();
        let mut task = ReferenceOrbitTask::start(&request, &clock).expect("measurement task");
        loop {
            match task.poll(1, &clock).expect("measurement poll") {
                OrbitTaskPoll::Pending { .. } => {}
                OrbitTaskPoll::Complete { orbit, compute_us } => {
                    return (orbit, compute_us, plan.working_digits);
                }
                OrbitTaskPoll::Cancelled { .. } => panic!("current measurement was cancelled"),
            }
        }
    }

    #[test]
    #[ignore = "native reference performance measurement; run explicitly on the worker pod"]
    #[allow(
        clippy::print_stderr,
        reason = "this is the explicit performance report"
    )]
    fn measures_first_orbit_transfer_and_admission_at_worker_caps() {
        const SAMPLE_COUNT: usize = 7;
        const PACK_REPEATS: u32 = 256;
        for max_iter in [512, 4_096] {
            let mut compute_samples = Vec::with_capacity(SAMPLE_COUNT);
            let mut retained = None;
            let mut working_digits = 0;
            for _ in 0..SAMPLE_COUNT {
                let (orbit, compute_us, digits) = measured_orbit(max_iter);
                assert_eq!(orbit.length, max_iter);
                compute_samples.push(compute_us);
                retained = Some(orbit);
                working_digits = digits;
            }
            compute_samples.sort_unstable();
            let compute_us = compute_samples[SAMPLE_COUNT / 2];
            let orbit = retained.expect("at least one measurement orbit");
            let mut buffer =
                WireBuffer::new(Pool::Orbit, 0, max_iter).expect("measurement transfer buffer");
            let pack_started = Instant::now();
            let facts = OrbitVerificationFacts::from_orbit(&orbit);
            for _ in 0..PACK_REPEATS {
                buffer
                    .write_orbit(
                        1,
                        orbit.precision_bits,
                        compute_us,
                        0,
                        &orbit.records,
                        facts,
                    )
                    .expect("measurement transfer packing");
                std::hint::black_box(buffer.as_bytes());
            }
            let pack_mean_ns = pack_started.elapsed().as_nanos() / u128::from(PACK_REPEATS);
            let payload_bytes = orbit.records.len() * size_of::<ReferenceOrbitRecord>();
            let mut shaper = ProducerShaper::new();
            assert!(matches!(
                shaper.admit(0).expect("warm-up admission"),
                Admission::Ready { warm_up: true, .. }
            ));
            shaper
                .observe_return(0, 0, compute_us)
                .expect("measured price return");
            let wait_us = match shaper.admit(0).expect("priced admission") {
                Admission::Delay { wait_us } => wait_us,
                other => panic!("depleted bucket must delay measured work: {other:?}"),
            };
            eprintln!(
                "reference_measurement record_bytes={} cap={max_iter} working_digits={working_digits} first_orbit_us={compute_us} payload_bytes={payload_bytes} pack_mean_ns={pack_mean_ns} admission_price_us={} depleted_wait_us={wait_us}",
                size_of::<ReferenceOrbitRecord>(),
                shaper.admission_price_us(),
            );
        }
    }

    #[test]
    fn canonical_adapter_round_trips_all_four_dyadics() {
        let centre = BigCentre::from_f64([0.0, -0.0, -0.75, 0.125], 128).unwrap();
        let encoded = EncodedCentre::encode_math(&centre, 41).unwrap();
        assert_eq!(encoded.revision, 41);
        for mode in PrecisionMode::ALL {
            let decoded = encoded.decode_math(128).unwrap();
            for (actual, expected) in decoded
                .to_f64_mirror()
                .into_iter()
                .zip(centre.to_f64_mirror())
            {
                let tolerance = expected.abs().mul_add(f64::EPSILON, f64::from_bits(1));
                assert!((actual - expected).abs() <= tolerance);
            }
            if mode.requires_bit_identity() {
                // Deterministic-only contract: all four dyadic records round-trip identically.
                assert_eq!(decoded, centre);
            }
        }
        assert_eq!(encoded.coordinates[0].limb_count, 0);
        assert_eq!(encoded.coordinates[1].sign, 0);
    }

    #[test]
    fn cooperative_builder_pins_record_zero_length_and_scaled_fixture() {
        let centre = BigCentre::from_f64([0.0, 0.0, 2.0, 0.0], 128).unwrap();
        let clock = StepClock::new(1);
        let mut task = ReferenceOrbitTask::start(&request(&centre, 64), &clock).unwrap();
        let OrbitTaskPoll::Complete { orbit, compute_us } = task.poll(7, &clock).unwrap() else {
            panic!("four-entry orbit must finish in its first chunk");
        };
        assert_eq!(orbit.length, 4);
        assert_eq!(orbit.escape_index, Some(3));
        assert_eq!(orbit.records[0].re_hi, 0.0);
        assert!(compute_us > 0);
        let sample =
            perturb_scaled_f64(&orbit.records, [0.0; 4], -900, EscapeParams::new(64)).unwrap();
        assert!(sample.escaped);
        assert_eq!(sample.escape_index, Some(3));
    }

    #[test]
    fn chunk_stops_at_iteration_or_clock_bound_and_cancels_at_next_yield() {
        let centre = BigCentre::from_f64([0.0; 4], 128).unwrap();
        let clock = StepClock::new(1);
        let mut task = ReferenceOrbitTask::start(&request(&centre, 128), &clock).unwrap();
        let OrbitTaskPoll::Pending {
            stored,
            chunk_iterations,
            ..
        } = task.poll(7, &clock).unwrap()
        else {
            panic!("non-escaping orbit must yield");
        };
        assert_eq!(stored, ORBIT_CHUNK_MAX_ITERATIONS);
        assert_eq!(chunk_iterations, ORBIT_CHUNK_MAX_ITERATIONS);
        assert!(matches!(
            task.poll(8, &clock).unwrap(),
            OrbitTaskPoll::Cancelled { generation: 7, .. }
        ));

        let slow_clock = StepClock::new(1_000);
        let mut slow = ReferenceOrbitTask::start(&request(&centre, 128), &slow_clock).unwrap();
        let OrbitTaskPoll::Pending {
            chunk_iterations, ..
        } = slow.poll(7, &slow_clock).unwrap()
        else {
            panic!("clock-bound orbit must yield");
        };
        assert_eq!(chunk_iterations, 2);
    }

    #[test]
    fn insufficient_transported_precision_is_a_stable_math_refusal() {
        let centre = BigCentre::from_f64([0.0; 4], 128).unwrap();
        let encoded = EncodedCentre::encode_math(&centre, 1).unwrap();
        let request = OrbitRequest::new(
            1,
            encoded,
            100,
            64,
            64,
            PrecisionMode::Deterministic,
            OrbitReason::INITIAL,
        )
        .unwrap();
        let error = ReferenceOrbitTask::start(&request, &StepClock::new(1)).unwrap_err();
        assert_eq!(error.code, crate::ErrorCode::MathFailure);
        assert_eq!(error.detail, MathFailureCode::InvalidPrecisionPlan as u32);
    }
}
