//! Bounded voice events. Audio and subtitle playback consume (life, id) pairs;
//! reading a snapshot never consumes it, so skipped presentation frames are safe.
pub const RECENT_VOICE_EVENTS: usize = 8;
pub const MOVEMENT_COOLDOWN: f32 = 12.0;
const MOVEMENT_COOLDOWN_TICKS: u64 = 720;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceKind {
    Movement,
    GateUnlocked,
    SwordClaimed,
    WardenDeath,
}

impl VoiceKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Movement => "movement",
            Self::GateUnlocked => "key",
            Self::SwordClaimed => "sword",
            Self::WardenDeath => "death",
        }
    }

    fn once_bit(self) -> u8 {
        match self {
            Self::Movement => 0,
            Self::GateUnlocked => 1,
            Self::SwordClaimed => 2,
            Self::WardenDeath => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceEvent {
    pub id: u32,
    pub kind: VoiceKind,
    pub time: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dialogue {
    /// Increments when the prisoner dies and Dungeon resets. Combine with id.
    pub life: u32,
    /// Latest emitted id in this life, including events no longer in the ring.
    pub sequence: u32,
    recent: [Option<VoiceEvent>; RECENT_VOICE_EVENTS],
    clock: u64,
    last_movement: Option<u64>,
    spoken: u8,
}

impl Dialogue {
    pub fn new(life: u32) -> Self {
        Self {
            life,
            ..Self::default()
        }
    }

    /// Chronological, non-consuming recent events for a presentation snapshot.
    pub fn events(&self) -> impl Iterator<Item = &VoiceEvent> {
        self.recent.iter().flatten()
    }

    pub fn dead(&self) -> bool {
        self.spoken & VoiceKind::WardenDeath.once_bit() != 0
    }

    pub(crate) fn advance(&mut self) {
        self.clock += 1;
    }

    pub(crate) fn emit(&mut self, kind: VoiceKind, time: f32) -> bool {
        if self.dead() || self.spoken & kind.once_bit() != 0 {
            return false;
        }
        if kind == VoiceKind::Movement {
            if self
                .last_movement
                .is_some_and(|last| self.clock - last < MOVEMENT_COOLDOWN_TICKS)
            {
                return false;
            }
            self.last_movement = Some(self.clock);
        } else {
            self.spoken |= kind.once_bit();
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.recent.rotate_left(1);
        self.recent[RECENT_VOICE_EVENTS - 1] = Some(VoiceEvent {
            id: self.sequence,
            kind,
            time,
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::STEP;

    #[test]
    fn movement_cooldown_expires_at_exactly_720_unfrozen_ticks() {
        let mut dialogue = Dialogue::default();
        assert!(dialogue.emit(VoiceKind::Movement, 0.0));
        for tick in 1..720 {
            dialogue.advance();
            assert!(!dialogue.emit(VoiceKind::Movement, tick as f32 * STEP));
        }
        dialogue.advance();
        assert!(dialogue.emit(VoiceKind::Movement, MOVEMENT_COOLDOWN));
        assert_eq!(dialogue.sequence, 2);
    }

    #[test]
    fn recent_events_are_bounded_ordered_and_survive_skipped_reads() {
        let mut dialogue = Dialogue::new(7);
        for index in 0..12 {
            assert!(dialogue.emit(VoiceKind::Movement, index as f32 * MOVEMENT_COOLDOWN));
            for _ in 0..MOVEMENT_COOLDOWN_TICKS {
                dialogue.advance();
            }
        }
        let first: Vec<_> = dialogue.events().copied().collect();
        assert_eq!(first.len(), RECENT_VOICE_EVENTS);
        assert_eq!(first.first().unwrap().id, 5);
        assert_eq!(first.last().unwrap().id, 12);
        assert!(first
            .windows(2)
            .all(|p| p[0].id < p[1].id && p[0].time < p[1].time));
        assert_eq!(dialogue.events().copied().collect::<Vec<_>>(), first);
        assert_eq!(dialogue.life, 7);
    }

    #[test]
    fn story_lines_are_once_per_life_and_death_is_terminal() {
        let mut dialogue = Dialogue::default();
        for kind in [
            VoiceKind::GateUnlocked,
            VoiceKind::SwordClaimed,
            VoiceKind::WardenDeath,
        ] {
            assert!(dialogue.emit(kind, 0.0));
            assert!(!dialogue.emit(kind, 0.0));
        }
        assert!(dialogue.dead());
        for _ in 0..1440 {
            dialogue.advance();
        }
        for kind in [
            VoiceKind::Movement,
            VoiceKind::GateUnlocked,
            VoiceKind::SwordClaimed,
            VoiceKind::WardenDeath,
        ] {
            assert!(!dialogue.emit(kind, 24.0));
        }
        assert_eq!(dialogue.sequence, 3);
        let mut next = Dialogue::new(dialogue.life + 1);
        assert_eq!(next.sequence, 0);
        assert_eq!(next.events().count(), 0);
        assert!(!next.dead());
        assert!(next.emit(VoiceKind::GateUnlocked, 0.0));
        assert_eq!(next.sequence, 1);
    }
}
