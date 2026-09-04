//! Exact integer credit accounting and producer-side admission shaping.

use crate::{ChannelError, ErrorCode, ORBIT_BUDGET_US_PER_SECOND, WorkerMode};

const MICROS_PER_SECOND: u64 = 1_000_000;

/// Result of charging one measured worker computation to the owner budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditCharge {
    /// Remaining owner credit after the charge.
    pub credit_us: u32,
    /// Measured work beyond the available budget.
    pub overfeed_us: u32,
}

/// Owner-side microsecond token bucket with a fixed one-second capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditAccount {
    credit_us: u32,
    last_update_us: Option<u64>,
}

impl Default for CreditAccount {
    fn default() -> Self {
        Self::new()
    }
}

impl CreditAccount {
    /// Creates a full bucket whose clock epoch begins at its first charge.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            credit_us: ORBIT_BUDGET_US_PER_SECOND,
            last_update_us: None,
        }
    }

    /// Returns the currently recorded credit without projecting elapsed time.
    #[must_use]
    pub const fn credit_us(self) -> u32 {
        self.credit_us
    }

    /// Refills at the fixed policy rate and charges measured producer work.
    ///
    /// # Errors
    ///
    /// Returns `TimingOverflow` if the supplied monotonic time moves backwards.
    pub fn charge(
        &mut self,
        owner_now_us: u64,
        compute_us: u32,
    ) -> Result<CreditCharge, ChannelError> {
        let elapsed = match self.last_update_us {
            Some(previous) => owner_now_us
                .checked_sub(previous)
                .ok_or_else(timing_refusal)?,
            None => 0,
        };
        let refilled = refill(self.credit_us, elapsed);
        let charge = CreditCharge {
            credit_us: refilled.saturating_sub(compute_us),
            overfeed_us: compute_us.saturating_sub(refilled),
        };
        self.credit_us = charge.credit_us;
        self.last_update_us = Some(owner_now_us);
        Ok(charge)
    }
}

/// Producer admission decision after projecting the most recent returned credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Work may begin with the stated pre-charge credit.
    Ready {
        /// Projected credit immediately before admission.
        credit_us: u32,
        /// True only for the first unpriced computation after startup or resize.
        warm_up: bool,
    },
    /// Work must remain pending for at least this many microseconds.
    Delay {
        /// Exact ceiling wait implied by the fixed refill rate.
        wait_us: u64,
    },
    /// The warm-up returned zero elapsed time, so another price cannot be invented.
    TimingUnavailable,
}

/// Producer-side projection and bounded measured-cost admission shaper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProducerShaper {
    local_credit_us: u32,
    local_update_us: Option<u64>,
    estimate_us: u32,
    warm_up_pending: bool,
    awaiting_warm_up_return: bool,
}

impl Default for ProducerShaper {
    fn default() -> Self {
        Self::new()
    }
}

impl ProducerShaper {
    /// Creates one full producer projection with one labelled unpriced admission.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            local_credit_us: ORBIT_BUDGET_US_PER_SECOND,
            local_update_us: None,
            estimate_us: 0,
            warm_up_pending: true,
            awaiting_warm_up_return: false,
        }
    }

    /// Resets the pricing epoch after the four buffers are resized.
    pub const fn reset_for_resize(&mut self) {
        *self = Self::new();
    }

    /// Reconciles the local projection with an owner CREDIT header.
    ///
    /// # Errors
    ///
    /// Returns `TimingOverflow` if producer time moves backwards.
    pub fn observe_return(
        &mut self,
        producer_now_us: u64,
        returned_credit_us: u32,
        compute_us: u32,
    ) -> Result<(), ChannelError> {
        if let Some(previous) = self.local_update_us {
            producer_now_us
                .checked_sub(previous)
                .ok_or_else(timing_refusal)?;
        }
        self.local_credit_us = returned_credit_us.min(ORBIT_BUDGET_US_PER_SECOND);
        self.local_update_us = Some(producer_now_us);
        self.fold_estimate(compute_us);
        self.awaiting_warm_up_return = false;
        Ok(())
    }

    /// Admits, delays, or honestly refuses to price the next computation.
    ///
    /// # Errors
    ///
    /// Returns `TimingOverflow` if producer time moves backwards.
    pub fn admit(&mut self, producer_now_us: u64) -> Result<Admission, ChannelError> {
        self.project(producer_now_us)?;
        if self.warm_up_pending {
            self.warm_up_pending = false;
            self.awaiting_warm_up_return = true;
            return Ok(Admission::Ready {
                credit_us: self.local_credit_us,
                warm_up: true,
            });
        }
        if self.awaiting_warm_up_return {
            return Ok(Admission::Delay { wait_us: 0 });
        }
        if self.estimate_us == 0 {
            return Ok(Admission::TimingUnavailable);
        }
        let price_us = self.admission_price_us();
        if self.local_credit_us < price_us {
            let deficit = u64::from(price_us - self.local_credit_us);
            let numerator = deficit
                .checked_mul(MICROS_PER_SECOND)
                .ok_or_else(timing_refusal)?;
            let divisor = u64::from(ORBIT_BUDGET_US_PER_SECOND);
            return Ok(Admission::Delay {
                wait_us: numerator.div_ceil(divisor),
            });
        }
        let credit_us = self.local_credit_us;
        self.local_credit_us -= price_us;
        Ok(Admission::Ready {
            credit_us,
            warm_up: false,
        })
    }

    /// Returns the decayed measured cost that prices the next computation.
    #[must_use]
    pub const fn estimate_us(self) -> u32 {
        self.estimate_us
    }

    /// Returns the bounded balance one admission asks of the local projection.
    ///
    /// The bucket's capacity is one second of budget, so a price above it is a
    /// balance the projection can never reach; charging the bounded price keeps
    /// the implied wait at one second at most, and the owner still charges the
    /// full measured cost and reports the excess as `overfeed_us`.
    #[must_use]
    pub const fn admission_price_us(self) -> u32 {
        if self.estimate_us < ORBIT_BUDGET_US_PER_SECOND {
            self.estimate_us
        } else {
            ORBIT_BUDGET_US_PER_SECOND
        }
    }

    /// Folds one measured cost into the admission estimate.
    ///
    /// A nonzero measurement at or above the estimate replaces it at once, so
    /// the next request is priced at a cost the producer has just proved it can
    /// incur; a cheaper nonzero measurement halves the remaining gap, so one
    /// expensive orbit stops pricing every later cheap one after a bounded
    /// number of returns. A measured zero prices nothing and changes nothing.
    const fn fold_estimate(&mut self, compute_us: u32) {
        if compute_us == 0 {
            return;
        }
        if compute_us >= self.estimate_us {
            self.estimate_us = compute_us;
            return;
        }
        self.estimate_us = compute_us + (self.estimate_us - compute_us) / 2;
    }

    fn project(&mut self, producer_now_us: u64) -> Result<(), ChannelError> {
        let elapsed = match self.local_update_us {
            Some(previous) => producer_now_us
                .checked_sub(previous)
                .ok_or_else(timing_refusal)?,
            None => 0,
        };
        self.local_credit_us = refill(self.local_credit_us, elapsed);
        self.local_update_us = Some(producer_now_us);
        Ok(())
    }
}

/// Page-visible worker accounting with a pinned 64-byte C layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WorkerFacts {
    /// Facts revision, incremented on each observable channel transition.
    pub epoch: u64,
    /// Latest generation installed by the owner.
    pub last_applied_generation: u32,
    /// Latest generation whose orbit buffer was credited.
    pub last_ack_generation: u32,
    /// Completed or cancelled orbit buffers waiting on main.
    pub orbit_queue_depth: u32,
    /// Pending shutdown acknowledgements.
    pub shutdown_queue_depth: u32,
    /// Remaining owner orbit budget.
    pub credit_us: u32,
    /// Most recently credited worker compute wall.
    pub last_compute_us: u32,
    /// Most recent work beyond available owner budget.
    pub last_overfeed_us: u32,
    /// Number of applied orbit responses.
    pub applied_count: u32,
    /// Number of stale orbit responses.
    pub stale_count: u32,
    /// Number of credited cancelled computations.
    pub cancelled_count: u32,
    /// Initial allocation plus reconciled max-iteration resizes.
    pub allocation_events: u32,
    /// Request-pool buffers currently owned by main.
    pub request_buffers_owned_main: u32,
    /// Orbit-pool buffers queued or leased on main.
    pub orbit_buffers_owned_main: u32,
    /// [`WorkerMode`] discriminant.
    pub mode: u32,
}

impl WorkerFacts {
    /// Creates the startup snapshot after the four transport buffers are allocated.
    #[must_use]
    pub const fn new(mode: WorkerMode) -> Self {
        Self {
            epoch: 0,
            last_applied_generation: 0,
            last_ack_generation: 0,
            orbit_queue_depth: 0,
            shutdown_queue_depth: 0,
            credit_us: ORBIT_BUDGET_US_PER_SECOND,
            last_compute_us: 0,
            last_overfeed_us: 0,
            applied_count: 0,
            stale_count: 0,
            cancelled_count: 0,
            allocation_events: 1,
            request_buffers_owned_main: 2,
            orbit_buffers_owned_main: 0,
            mode: mode as u32,
        }
    }
}

fn refill(previous: u32, elapsed_us: u64) -> u32 {
    if elapsed_us >= 4 * MICROS_PER_SECOND {
        return ORBIT_BUDGET_US_PER_SECOND;
    }
    let earned = elapsed_us * u64::from(ORBIT_BUDGET_US_PER_SECOND) / MICROS_PER_SECOND;
    let bounded = u64::from(previous)
        .saturating_add(earned)
        .min(u64::from(ORBIT_BUDGET_US_PER_SECOND));
    u32::try_from(bounded).unwrap_or(ORBIT_BUDGET_US_PER_SECOND)
}

const fn timing_refusal() -> ChannelError {
    ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::{Admission, CreditAccount, ProducerShaper, WorkerFacts};
    use crate::{ErrorCode, ORBIT_BUDGET_US_PER_SECOND, WorkerMode};

    #[test]
    fn facts_layout_is_exactly_sixty_four_bytes() {
        assert_eq!(size_of::<WorkerFacts>(), 64);
        assert_eq!(align_of::<WorkerFacts>(), 8);
        assert_eq!(std::mem::offset_of!(WorkerFacts, epoch), 0);
        assert_eq!(std::mem::offset_of!(WorkerFacts, mode), 60);
        assert_eq!(WorkerFacts::new(WorkerMode::SameThread).mode, 1);
    }

    #[test]
    fn owner_bucket_refills_clamps_depletes_and_reports_overfeed() {
        let mut account = CreditAccount::new();
        assert_eq!(account.charge(10, 100_000).unwrap().credit_us, 150_000);
        assert_eq!(account.charge(410_010, 200_000).unwrap().credit_us, 50_000);
        let overfed = account.charge(410_010, 75_000).unwrap();
        assert_eq!(overfed.credit_us, 0);
        assert_eq!(overfed.overfeed_us, 25_000);
        assert_eq!(account.charge(4_410_010, 0).unwrap().credit_us, 250_000);
        assert_eq!(account.credit_us(), ORBIT_BUDGET_US_PER_SECOND);
        assert_eq!(
            account.charge(4_410_009, 0).unwrap_err().code,
            ErrorCode::TimingOverflow
        );
    }

    #[test]
    fn producer_shapes_after_exactly_one_unpriced_warm_up() {
        let mut shaper = ProducerShaper::new();
        assert_eq!(
            shaper.admit(100).unwrap(),
            Admission::Ready {
                credit_us: 250_000,
                warm_up: true
            }
        );
        assert_eq!(shaper.admit(101).unwrap(), Admission::Delay { wait_us: 0 });
        shaper.observe_return(200, 10_000, 100_000).unwrap();
        assert_eq!(shaper.estimate_us(), 100_000);
        assert_eq!(
            shaper.admit(200).unwrap(),
            Admission::Delay { wait_us: 360_000 }
        );
        assert_eq!(
            shaper.admit(360_200).unwrap(),
            Admission::Ready {
                credit_us: 100_000,
                warm_up: false
            }
        );
        shaper.observe_return(600_000, 1, 0).unwrap();
        assert_eq!(shaper.estimate_us(), 100_000);
        shaper.reset_for_resize();
        assert!(matches!(
            shaper.admit(700_000).unwrap(),
            Admission::Ready { warm_up: true, .. }
        ));
    }

    #[test]
    fn an_estimate_beyond_capacity_prices_at_most_one_second_of_budget() {
        let mut shaper = ProducerShaper::new();
        shaper.admit(0).unwrap();
        shaper.observe_return(0, 0, 852_293).unwrap();
        assert_eq!(shaper.estimate_us(), 852_293);
        assert_eq!(shaper.admission_price_us(), ORBIT_BUDGET_US_PER_SECOND);
        assert_eq!(
            shaper.admit(0).unwrap(),
            Admission::Delay { wait_us: 1_000_000 }
        );
        assert_eq!(
            shaper.admit(999_999).unwrap(),
            Admission::Delay { wait_us: 4 }
        );
        assert_eq!(
            shaper.admit(1_000_003).unwrap(),
            Admission::Ready {
                credit_us: 250_000,
                warm_up: false
            }
        );
        assert_eq!(
            shaper.admit(1_000_003).unwrap(),
            Admission::Delay { wait_us: 1_000_000 }
        );
    }

    #[test]
    fn an_estimate_at_capacity_is_priced_without_a_wait() {
        let mut shaper = ProducerShaper::new();
        shaper.admit(0).unwrap();
        shaper
            .observe_return(
                0,
                ORBIT_BUDGET_US_PER_SECOND,
                ORBIT_BUDGET_US_PER_SECOND + 1,
            )
            .unwrap();
        assert_eq!(shaper.admission_price_us(), ORBIT_BUDGET_US_PER_SECOND);
        assert_eq!(
            shaper.admit(0).unwrap(),
            Admission::Ready {
                credit_us: 250_000,
                warm_up: false
            }
        );
    }

    #[test]
    fn the_estimate_rises_at_once_and_halves_toward_cheaper_measurements() {
        let mut shaper = ProducerShaper::new();
        shaper.admit(0).unwrap();
        shaper.observe_return(0, 250_000, 100_000).unwrap();
        assert_eq!(shaper.estimate_us(), 100_000);
        shaper.observe_return(1, 250_000, 200_000).unwrap();
        assert_eq!(shaper.estimate_us(), 200_000);
        shaper.observe_return(2, 250_000, 0).unwrap();
        assert_eq!(shaper.estimate_us(), 200_000);
        for expected in [
            100_500_u32,
            50_750,
            25_875,
            13_437,
            7_218,
            4_109,
            2_554,
            1_777,
            1_388,
            1_194,
            1_097,
            1_048,
            1_024,
            1_012,
            1_006,
            1_003,
            1_001,
            1_000,
            1_000,
        ] {
            shaper.observe_return(3, 250_000, 1_000).unwrap();
            assert_eq!(shaper.estimate_us(), expected);
        }
    }

    #[test]
    fn zero_timer_is_labelled_and_clock_regression_is_typed() {
        let mut shaper = ProducerShaper::new();
        shaper.admit(9).unwrap();
        shaper.observe_return(10, 250_000, 0).unwrap();
        assert_eq!(shaper.admit(10).unwrap(), Admission::TimingUnavailable);
        assert_eq!(shaper.admit(9).unwrap_err().code, ErrorCode::TimingOverflow);
    }
}
