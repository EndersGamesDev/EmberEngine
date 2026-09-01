use std::collections::VecDeque;
use std::time::Duration;

use crate::{AcknowledgementMode, CorrectionMode, PredictionHooks, ReplayContext};

/// Wrapping monotonic allocator for frozen `u32` input sequence fields.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SequenceAllocator {
    last: u32,
}

impl SequenceAllocator {
    /// Starts an allocator whose first sequence is one.
    #[must_use]
    pub const fn new() -> Self {
        Self { last: 0 }
    }

    /// Starts after a known wire sequence, including near wraparound.
    #[must_use]
    pub const fn after(last: u32) -> Self {
        Self { last }
    }

    /// Allocates the next wrapping wire sequence.
    pub const fn allocate(&mut self) -> u32 {
        self.last = self.last.wrapping_add(1);
        self.last
    }

    /// Returns the most recently allocated sequence.
    #[must_use]
    pub const fn last(&self) -> u32 {
        self.last
    }
}

/// One timestamped input retained for authoritative replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequencedInput<I> {
    /// Client-owned wrapping wire sequence.
    pub sequence: u32,
    /// Client monotonic emission time.
    pub sent_at: Duration,
    /// Game-owned input payload.
    pub input: I,
}

/// Capacity-bounded, sequence-ordered unacknowledged input history.
#[derive(Clone, Debug)]
pub struct InputHistory<I> {
    capacity: usize,
    entries: VecDeque<SequencedInput<I>>,
}

impl<I> InputHistory<I> {
    /// Constructs an empty history; a zero request is normalized to one.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::with_capacity(capacity.max(1)),
        }
    }

    /// Appends one input and evicts only the oldest entries over capacity.
    pub fn push(&mut self, input: SequencedInput<I>) {
        self.entries.push_back(input);
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    /// Trims the authoritative prefix using wrapping serial-number order.
    pub fn trim_acknowledged(&mut self, acknowledgement: u32, mode: AcknowledgementMode) {
        while self.entries.front().is_some_and(|entry| {
            let distance = acknowledgement.wrapping_sub(entry.sequence);
            let at_or_before = distance < (1_u32 << 31);
            at_or_before
                && (mode == AcknowledgementMode::Through || entry.sequence != acknowledgement)
        }) {
            self.entries.pop_front();
        }
    }

    /// Removes every retained input without changing capacity.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the number of retained inputs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no inputs remain.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterates in original allocation order.
    pub fn iter(&self) -> impl Iterator<Item = &SequencedInput<I>> {
        self.entries.iter()
    }
}

/// Summary of one authoritative rebase and replay pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reconciliation {
    /// Authoritative sequence used to trim the cursor.
    pub acknowledgement: u32,
    /// Opaque timestamp extracted by the game hook.
    pub server_timestamp: u64,
    /// Number of retained inputs replayed.
    pub replayed_inputs: usize,
    /// Player-visible correction policy selected by the game.
    pub correction: CorrectionMode,
}

/// Shared sequence, bounded-history, acknowledgement, and replay orchestrator.
#[derive(Clone, Debug)]
pub struct Reconciler<I> {
    sequences: SequenceAllocator,
    history: InputHistory<I>,
}

impl<I> Reconciler<I> {
    /// Constructs an empty reconciler with a bounded history.
    #[must_use]
    pub fn new(history_capacity: usize) -> Self {
        Self {
            sequences: SequenceAllocator::new(),
            history: InputHistory::new(history_capacity),
        }
    }

    /// Allocates, timestamps, and retains one input, returning its sequence.
    pub fn record(&mut self, input: I, sent_at: Duration) -> u32 {
        let sequence = self.sequences.allocate();
        self.history.push(SequencedInput {
            sequence,
            sent_at,
            input,
        });
        sequence
    }

    /// Clears retained inputs while preserving sequence monotonicity.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Returns the number of retained inputs.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Returns whether no inputs remain to replay.
    #[must_use]
    pub fn history_is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Applies authority, trims history, and visits each surviving input once.
    pub fn reconcile<H>(
        &mut self,
        hooks: &H,
        predicted: &mut H::PredictedState,
        authoritative: &H::AuthoritativeState,
        replay_until: Duration,
    ) -> Reconciliation
    where
        H: PredictionHooks<Input = I>,
    {
        let acknowledgement = hooks.acknowledgement(authoritative);
        let server_timestamp = hooks.server_timestamp(authoritative);
        self.history
            .trim_acknowledged(acknowledgement, hooks.acknowledgement_mode());
        let before = predicted.clone();
        hooks.apply_authoritative(predicted, authoritative);
        let replayed_inputs = self.history.len();
        for (index, entry) in self.history.entries.iter().enumerate() {
            let next_sent_at = self
                .history
                .entries
                .get(index + 1)
                .map(|next| next.sent_at);
            hooks.replay_one_slice(
                predicted,
                &entry.input,
                ReplayContext {
                    sequence: entry.sequence,
                    sent_at: entry.sent_at,
                    next_sent_at,
                    replay_until,
                    acknowledgement,
                    server_timestamp,
                },
                authoritative,
            );
        }
        Reconciliation {
            acknowledgement,
            server_timestamp,
            replayed_inputs,
            correction: hooks.snap_or_smooth(&before, predicted, authoritative),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CorrectionMode, PredictionHooks};

    #[derive(Clone)]
    struct FakeAuthoritative {
        value: i32,
        ack: u32,
        timestamp: u64,
    }

    struct FakeHooks;

    impl PredictionHooks for FakeHooks {
        type Input = i32;
        type AuthoritativeState = FakeAuthoritative;
        type PredictedState = i32;

        fn acknowledgement(&self, authoritative: &Self::AuthoritativeState) -> u32 {
            authoritative.ack
        }

        fn server_timestamp(&self, authoritative: &Self::AuthoritativeState) -> u64 {
            authoritative.timestamp
        }

        fn acknowledgement_mode(&self) -> AcknowledgementMode {
            AcknowledgementMode::Through
        }

        fn apply_authoritative(
            &self,
            predicted: &mut Self::PredictedState,
            authoritative: &Self::AuthoritativeState,
        ) {
            *predicted = authoritative.value;
        }

        fn replay_one_slice(
            &self,
            predicted: &mut Self::PredictedState,
            input: &Self::Input,
            _context: ReplayContext,
            _authoritative: &Self::AuthoritativeState,
        ) {
            *predicted += input;
        }

        fn snap_or_smooth(
            &self,
            before: &Self::PredictedState,
            after: &Self::PredictedState,
            _authoritative: &Self::AuthoritativeState,
        ) -> CorrectionMode {
            if before.abs_diff(*after) > 20 {
                CorrectionMode::Snap
            } else {
                CorrectionMode::Smooth
            }
        }
    }

    #[test]
    fn sequence_wrap_and_ack_trim_share_serial_order() {
        let mut allocator = SequenceAllocator::after(u32::MAX - 1);
        assert_eq!(allocator.allocate(), u32::MAX);
        assert_eq!(allocator.allocate(), 0);
        assert_eq!(allocator.allocate(), 1);

        let mut history = InputHistory::new(4);
        for sequence in [u32::MAX, 0, 1] {
            history.push(SequencedInput {
                sequence,
                sent_at: Duration::ZERO,
                input: sequence,
            });
        }
        history.trim_acknowledged(0, AcknowledgementMode::Through);
        assert_eq!(history.iter().map(|entry| entry.sequence).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn history_evicts_the_oldest_input_at_capacity() {
        let mut history = InputHistory::new(2);
        for sequence in 1..=3 {
            history.push(SequencedInput {
                sequence,
                sent_at: Duration::from_millis(u64::from(sequence)),
                input: sequence,
            });
        }
        assert_eq!(history.iter().map(|entry| entry.sequence).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn before_mode_retains_the_acknowledged_command_for_partial_replay() {
        let mut history = InputHistory::new(4);
        for sequence in 6..=8 {
            history.push(SequencedInput {
                sequence,
                sent_at: Duration::ZERO,
                input: sequence,
            });
        }
        history.trim_acknowledged(7, AcknowledgementMode::Before);
        assert_eq!(
            history
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![7, 8]
        );
    }

    #[test]
    fn fake_game_rebases_trims_and_replays_in_order() {
        let mut reconciler = Reconciler::new(8);
        reconciler.record(2, Duration::from_millis(10));
        reconciler.record(3, Duration::from_millis(20));
        reconciler.record(5, Duration::from_millis(30));
        let authoritative = FakeAuthoritative {
            value: 100,
            ack: 2,
            timestamp: 77,
        };
        let mut predicted = 10;
        let result = reconciler.reconcile(
            &FakeHooks,
            &mut predicted,
            &authoritative,
            Duration::from_millis(40),
        );
        assert_eq!(predicted, 108);
        assert_eq!(result.replayed_inputs, 2);
        assert_eq!(result.server_timestamp, 77);
        assert_eq!(result.correction, CorrectionMode::Snap);
    }
}
