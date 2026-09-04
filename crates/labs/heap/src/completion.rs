//! Bounded completion-poll accounting shared by the wasm runtime and native tests.

pub const MAX_COMPLETION_POLLS: u32 = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PollCounter {
    polls: u32,
}

impl PollCounter {
    /// Starts a completion observation before its first poll.
    #[must_use]
    pub const fn new() -> Self {
        Self { polls: 0 }
    }

    /// Number of recorded device polls.
    #[must_use]
    pub const fn polls(self) -> u32 {
        self.polls
    }

    /// Records one poll without exceeding the fixed bound.
    ///
    /// # Errors
    ///
    /// Returns the terminal poll count when no further observation is admitted.
    pub const fn record(&mut self) -> Result<u32, u32> {
        if self.polls >= MAX_COMPLETION_POLLS {
            return Err(self.polls);
        }
        self.polls += 1;
        Ok(self.polls)
    }
}

impl Default for PollCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_counter_starts_at_zero_and_counts_every_poll() {
        let mut counter = PollCounter::new();
        assert_eq!(counter.polls(), 0);
        assert_eq!(counter.record(), Ok(1));
        assert_eq!(counter.record(), Ok(2));
        assert_eq!(counter.polls(), 2);
    }

    #[test]
    fn poll_counter_refuses_work_past_the_fixed_bound() {
        let mut counter = PollCounter::new();
        for expected in 1..=MAX_COMPLETION_POLLS {
            assert_eq!(counter.record(), Ok(expected));
        }
        assert_eq!(counter.record(), Err(MAX_COMPLETION_POLLS));
        assert_eq!(counter.polls(), MAX_COMPLETION_POLLS);
    }
}
