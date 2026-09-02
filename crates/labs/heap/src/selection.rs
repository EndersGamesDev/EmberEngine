//! Generation state for latest-selection-wins asynchronous work.

/// An infallible selection epoch whose newest value invalidates older work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionEpoch<T> {
    generation: u64,
    latest: T,
}

impl<T: Copy> SelectionEpoch<T> {
    /// Starts at generation zero with the supplied initial value.
    pub const fn new(latest: T) -> Self {
        Self {
            generation: 0,
            latest,
        }
    }

    /// Records a new selection and returns its generation.
    pub const fn select(&mut self, latest: T) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.latest = latest;
        self.generation
    }

    /// Invalidates in-flight work without replacing the requested selection.
    pub const fn invalidate(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    /// Returns the current generation.
    #[cfg(target_arch = "wasm32")]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns whether work from `generation` may still publish.
    pub const fn is_current(self, generation: u64) -> bool {
        self.generation == generation
    }

    /// Returns the latest requested selection.
    #[cfg(test)]
    pub const fn latest(self) -> T {
        self.latest
    }
}

#[cfg(test)]
mod tests {
    use super::SelectionEpoch;

    #[test]
    fn a_selection_at_every_yield_discards_stale_work_and_latest_wins() {
        const YIELD_POINTS: usize = 8;
        for arrival in 0..YIELD_POINTS {
            let mut state = SelectionEpoch::new("initial");
            let stale_generation = state.select("in flight");
            let mut stale_discarded = false;
            for point in 0..YIELD_POINTS {
                if point == arrival {
                    state.select("latest");
                }
                if !state.is_current(stale_generation) {
                    stale_discarded = true;
                    break;
                }
            }
            assert!(
                stale_discarded,
                "arrival at yield {arrival} was not discarded"
            );
            assert_eq!(state.latest(), "latest");
        }
    }

    #[test]
    fn invalidation_is_infallible_and_preserves_the_requested_selection() {
        let mut state = SelectionEpoch::new(3_u32);
        let selected = state.select(7);
        assert!(state.is_current(selected));
        let invalidated = state.invalidate();
        assert!(!state.is_current(selected));
        assert!(state.is_current(invalidated));
        assert_eq!(state.latest(), 7);
    }
}
