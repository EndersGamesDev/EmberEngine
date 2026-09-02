//! The per-turn clock, counted the same way by the server and the hotseat
//! client (section 1.5 of the design).
//!
//! The clock has no notion of time of its own: whoever owns it feeds elapsed
//! milliseconds into `tick`. That is what keeps the crate deterministic and
//! lets the server tests drive a turn with synthetic time.

use crate::proto::{GRACE_MS, TURN_MS};

/// Milliseconds a turn lasts before the timeout pass is applied.
pub const TURN_TOTAL_MS: u32 = TURN_MS + GRACE_MS;

/// Time left on the current turn, grace included.
///
/// `left_ms` counts down from `TURN_MS + GRACE_MS` to 0. The player sees
/// `display_left_ms`, which hides the grace: it reads `TURN_MS` right after
/// a reset and 0 from the moment the displayed deadline passes until the
/// timeout fires. A move is accepted while `expired` is false, so a move sent
/// at the displayed 0.0 still lands. Hotseat, which the design says runs
/// "without grace", times out on `display_left_ms() == 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TurnClock {
    /// Milliseconds until the timeout pass, grace included.
    pub left_ms: u32,
}

impl Default for TurnClock {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnClock {
    /// A clock at the start of a turn.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            left_ms: TURN_TOTAL_MS,
        }
    }

    /// A clock reconstructed from a displayed value, as a client does from
    /// `BoardState::left_ms` or `Clock::left_ms`. The grace window is added
    /// back, because the wire carries what the player sees.
    #[must_use]
    pub const fn from_display_left_ms(display_ms: u32) -> Self {
        let shown = if display_ms > TURN_MS {
            TURN_MS
        } else {
            display_ms
        };
        Self {
            left_ms: shown + GRACE_MS,
        }
    }

    /// Start a new turn.
    pub const fn reset(&mut self) {
        self.left_ms = TURN_TOTAL_MS;
    }

    /// Advance by `ms` of elapsed time. Returns true once `TURN_MS +
    /// GRACE_MS` has elapsed in total, i.e. the timeout pass is due; it keeps
    /// returning true until the next `reset`.
    pub const fn tick(&mut self, ms: u32) -> bool {
        self.left_ms = self.left_ms.saturating_sub(ms);
        self.expired()
    }

    /// True once the turn has run out, grace included.
    #[must_use]
    pub const fn expired(&self) -> bool {
        self.left_ms == 0
    }

    /// What the player sees: `TURN_MS` down to 0, never negative, never
    /// above `TURN_MS`.
    #[must_use]
    pub const fn display_left_ms(&self) -> u32 {
        let shown = self.left_ms.saturating_sub(GRACE_MS);
        if shown > TURN_MS { TURN_MS } else { shown }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_expires_at_turn_plus_grace() {
        let mut clock = TurnClock::new();
        assert!(!clock.tick(TURN_MS), "not at 15 000");
        assert!(!clock.tick(GRACE_MS - 1), "not at 15 299");
        assert!(clock.tick(1), "exactly at 15 300");
        assert!(clock.expired());
        assert!(clock.tick(1), "stays expired");
        clock.reset();
        assert!(!clock.expired());
        assert_eq!(clock.left_ms, TURN_TOTAL_MS);
        assert_eq!(TURN_TOTAL_MS, 15_300);

        // One tick of 15 300 also expires; one of 15 299 does not.
        let mut clock = TurnClock::new();
        assert!(clock.tick(15_300));
        let mut clock = TurnClock::new();
        assert!(!clock.tick(15_299));
    }

    #[test]
    fn display_never_exceeds_turn_ms_or_goes_negative() {
        let mut clock = TurnClock::new();
        assert_eq!(clock.display_left_ms(), TURN_MS);
        clock.tick(1_000);
        assert_eq!(clock.display_left_ms(), TURN_MS - 1_000);
        clock.tick(TURN_MS - 1_000);
        assert_eq!(clock.display_left_ms(), 0, "displayed deadline reached");
        assert!(!clock.expired(), "but the grace is still running");
        clock.tick(GRACE_MS);
        assert_eq!(clock.display_left_ms(), 0);
        clock.tick(u32::MAX);
        assert_eq!(clock.display_left_ms(), 0);
        assert_eq!(clock.left_ms, 0);

        let big = TurnClock {
            left_ms: TURN_TOTAL_MS * 3,
        };
        assert_eq!(big.display_left_ms(), TURN_MS);
        assert_eq!(
            TurnClock::from_display_left_ms(u32::MAX).display_left_ms(),
            TURN_MS
        );
        for shown in [0, 1, 299, 300, 8_400, TURN_MS] {
            assert_eq!(
                TurnClock::from_display_left_ms(shown).display_left_ms(),
                shown
            );
        }
    }
}
