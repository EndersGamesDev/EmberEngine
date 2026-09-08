//! Fixed-step sword rhythm, buffered strikes and presentation feedback.
//!
//! Attack inputs are press edges supplied by the client. Rhythm uses integer
//! ticks (18 quick / 54 delayed), independently of motion frozen by hitstop.
use crate::STEP;
use glam::Vec3;

const QUICK_TICKS: u64 = 18;
const DELAYED_TICKS: u64 = 54;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrikeKind {
    Cut,
    Backhand,
    Finisher,
    Overhead,
    Rising,
}

impl StrikeKind {
    pub fn duration(self) -> f32 {
        match self {
            Self::Cut => 0.78,
            Self::Backhand => 0.74,
            Self::Finisher => 1.05,
            Self::Overhead => 1.12,
            Self::Rising => 1.0,
        }
    }
    pub fn windup_time(self) -> f32 {
        match self {
            Self::Cut => 0.24,
            Self::Backhand => 0.22,
            Self::Finisher => 0.40,
            Self::Overhead => 0.50,
            Self::Rising => 0.37,
        }
    }
    pub fn contact_time(self) -> f32 {
        match self {
            Self::Cut => 0.30,
            Self::Backhand => 0.28,
            Self::Finisher => 0.48,
            Self::Overhead => 0.58,
            Self::Rising => 0.45,
        }
    }
    pub fn follow_end(self) -> f32 {
        match self {
            Self::Cut => 0.42,
            Self::Backhand => 0.39,
            Self::Finisher => 0.64,
            Self::Overhead => 0.74,
            Self::Rising => 0.62,
        }
    }
    pub fn damage(self) -> f32 {
        match self {
            Self::Cut => 28.0,
            Self::Backhand => 30.0,
            Self::Finisher => 55.0,
            Self::Overhead => 62.0,
            Self::Rising => 48.0,
        }
    }
    pub fn stamina_cost(self) -> f32 {
        match self {
            Self::Cut | Self::Backhand => 20.0,
            Self::Finisher => 32.0,
            Self::Overhead => 34.0,
            Self::Rising => 28.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Backhand => "Backhand",
            Self::Finisher => "Finisher",
            Self::Overhead => "Overhead",
            Self::Rising => "Rising cut",
        }
    }
    pub fn heavy(self) -> bool {
        matches!(self, Self::Finisher | Self::Overhead | Self::Rising)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub kind: StrikeKind,
    pub elapsed: f32,
    /// The contact sample was evaluated, whether it hit an object or missed.
    pub contact_done: bool,
}

impl Strike {
    pub fn new(kind: StrikeKind) -> Self {
        Self {
            kind,
            elapsed: 0.0,
            contact_done: false,
        }
    }
    pub fn remaining(self) -> f32 {
        (self.kind.duration() - self.elapsed).max(0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactKind {
    Warden,
    Chain,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Rhythm {
    #[default]
    Ready,
    One,
    QuickPair,
    Closed,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Combat {
    pub active: Option<Strike>,
    pub swing_event: u32,
    pub impact_event: u32,
    pub impact_left: f32,
    pub impact_point: Vec3,
    pub impact_kind: Option<ImpactKind>,
    pub impact_strike: Option<StrikeKind>,
    pub impact_strength: f32,
    pub hitstop_left: f32,
    queue: [Option<StrikeKind>; 2],
    rhythm: Rhythm,
    clock: u64,
    last_press: Option<u64>,
    selection: Option<StrikeKind>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CombatTick {
    pub contact: Option<StrikeKind>,
    pub frozen: bool,
}

impl Combat {
    pub fn impact_duration(&self) -> f32 {
        if self.impact_kind == Some(ImpactKind::Chain)
            || self.impact_strike.is_some_and(StrikeKind::heavy)
        {
            0.32
        } else {
            0.22
        }
    }

    pub fn queued_count(&self) -> usize {
        self.queue.iter().flatten().count()
    }
    pub fn queued(&self) -> [Option<StrikeKind>; 2] {
        self.queue
    }
    pub fn label(&self) -> &'static str {
        match self.selection {
            None => "Ready",
            Some(StrikeKind::Cut) => "Single cut",
            Some(StrikeKind::Backhand) => "Quick pair",
            Some(StrikeKind::Finisher) => "Combo I · Wolf's Fang",
            Some(StrikeKind::Overhead) => "Combo II · Gravebreaker",
            Some(StrikeKind::Rising) => "Combo III · Rising Wolf",
        }
    }
    pub fn rhythm_age(&self) -> Option<f32> {
        self.last_press
            .map(|tick| (self.clock - tick) as f32 * STEP)
    }
    pub fn rhythm_left(&self) -> f32 {
        if matches!(self.rhythm, Rhythm::One | Rhythm::QuickPair) {
            self.last_press.map_or(0.0, |tick| {
                DELAYED_TICKS.saturating_sub(self.clock - tick) as f32 * STEP
            })
        } else {
            0.0
        }
    }
    pub fn quick_left(&self) -> f32 {
        if matches!(self.rhythm, Rhythm::One | Rhythm::QuickPair) {
            self.last_press.map_or(0.0, |tick| {
                QUICK_TICKS.saturating_sub(self.clock - tick) as f32 * STEP
            })
        } else {
            0.0
        }
    }
    pub fn remaining(&self) -> f32 {
        self.active.map_or(0.0, Strike::remaining)
    }
    pub fn finished(&self) -> bool {
        self.active.is_none()
            && self.queued_count() == 0
            && self.hitstop_left == 0.0
            && self.impact_left == 0.0
    }
    /// Stop future strikes after the escape chain breaks, retaining this strike's recovery.
    pub fn clear_queue(&mut self) {
        self.queue = [None; 2];
        self.rhythm = Rhythm::Closed;
        self.last_press = None;
    }
    /// Pickups own both arms; cancel combat without replaying or rewinding event ids.
    pub fn cancel(&mut self) {
        self.active = None;
        self.clear_queue();
        self.rhythm = Rhythm::Ready;
        self.selection = None;
        self.hitstop_left = 0.0;
        self.impact_left = 0.0;
        self.impact_kind = None;
        self.impact_strike = None;
        self.impact_strength = 0.0;
    }
    fn start(&mut self, kind: StrikeKind, stamina: &mut f32) -> bool {
        if !stamina.is_finite() || *stamina < kind.stamina_cost() {
            self.clear_queue();
            self.rhythm = Rhythm::Ready;
            self.selection = None;
            return false;
        }
        *stamina -= kind.stamina_cost();
        self.active = Some(Strike::new(kind));
        self.swing_event = self.swing_event.wrapping_add(1);
        true
    }
    fn press(&mut self, stamina: &mut f32) {
        let gap = self.last_press.map_or(u64::MAX, |tick| self.clock - tick);
        let (kind, next) = match self.rhythm {
            Rhythm::One if gap <= QUICK_TICKS => (StrikeKind::Backhand, Rhythm::QuickPair),
            Rhythm::One if gap <= DELAYED_TICKS => (StrikeKind::Overhead, Rhythm::Closed),
            Rhythm::QuickPair if gap <= QUICK_TICKS => (StrikeKind::Finisher, Rhythm::Closed),
            Rhythm::QuickPair if gap <= DELAYED_TICKS => (StrikeKind::Rising, Rhythm::Closed),
            _ if self.active.is_none() && self.queued_count() == 0 => {
                (StrikeKind::Cut, Rhythm::One)
            }
            _ => return,
        };
        if self.active.is_none() {
            if !self.start(kind, stamina) {
                return;
            }
        } else if let Some(slot) = self.queue.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(kind);
        } else {
            return;
        }
        self.last_press = Some(self.clock);
        self.rhythm = next;
        self.selection = Some(kind);
    }
    /// One simulation tick. Edges still enter the rhythm buffer during hitstop.
    pub(crate) fn tick(&mut self, attack: bool, stamina: &mut f32) -> CombatTick {
        self.clock += 1;
        if self
            .last_press
            .is_some_and(|tick| self.clock - tick > DELAYED_TICKS)
            && matches!(self.rhythm, Rhythm::One | Rhythm::QuickPair)
        {
            self.rhythm = if self.active.is_none() && self.queued_count() == 0 {
                self.selection = None;
                Rhythm::Ready
            } else {
                Rhythm::Closed
            };
            self.last_press = None;
        }
        self.impact_left = (self.impact_left - STEP).max(0.0);
        if attack {
            self.press(stamina);
        }
        if self.hitstop_left > 0.0 {
            self.hitstop_left = (self.hitstop_left - STEP).max(0.0);
            if self.hitstop_left < 0.000001 {
                self.hitstop_left = 0.0;
            }
            return CombatTick {
                frozen: true,
                ..CombatTick::default()
            };
        }
        let mut contact = None;
        if let Some(mut strike) = self.active {
            strike.elapsed = (strike.elapsed + STEP).min(strike.kind.duration());
            if !strike.contact_done && strike.elapsed >= strike.kind.contact_time() {
                strike.contact_done = true;
                contact = Some(strike.kind);
            }
            if strike.elapsed >= strike.kind.duration() {
                self.active = None;
                let next = self.queue[0].take();
                self.queue[0] = self.queue[1].take();
                if let Some(kind) = next {
                    self.start(kind, stamina);
                }
            } else {
                self.active = Some(strike);
            }
        }
        CombatTick {
            contact,
            frozen: false,
        }
    }
    pub(crate) fn impact(&mut self, strike: StrikeKind, kind: ImpactKind, point: Vec3) {
        let heavy = strike.heavy() || kind == ImpactKind::Chain;
        self.impact_event = self.impact_event.wrapping_add(1);
        self.impact_left = if heavy { 0.32 } else { 0.22 };
        self.impact_point = point;
        self.impact_kind = Some(kind);
        self.impact_strike = Some(strike);
        self.impact_strength = if heavy { 0.85 } else { 0.45 };
        self.hitstop_left = if heavy { 0.10 } else { 0.06 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(presses: &[usize], spam: Option<std::ops::Range<usize>>) -> Vec<StrikeKind> {
        let mut combat = Combat::default();
        let mut stamina = 500.0;
        let mut swings = Vec::new();
        for tick in 0..360 {
            let event = combat.swing_event;
            combat.tick(
                presses.contains(&tick) || spam.as_ref().is_some_and(|r| r.contains(&tick)),
                &mut stamina,
            );
            assert!(combat.queued_count() <= 2);
            if combat.swing_event != event {
                swings.push(combat.active.unwrap().kind);
            }
        }
        assert!(combat.finished());
        swings
    }

    #[test]
    fn exact_click_patterns_and_no_automatic_followup() {
        use StrikeKind::*;
        assert_eq!(pattern(&[0], None), [Cut]);
        assert_eq!(pattern(&[0, 6, 12], None), [Cut, Backhand, Finisher]);
        assert_eq!(pattern(&[0, 24], None), [Cut, Overhead]);
        assert_eq!(pattern(&[0, 6, 30], None), [Cut, Backhand, Rising]);
    }

    #[test]
    fn rhythm_boundaries_are_inclusive_integer_ticks() {
        use StrikeKind::*;
        assert_eq!(pattern(&[0, 18, 36], None), [Cut, Backhand, Finisher]);
        assert_eq!(pattern(&[0, 19], None), [Cut, Overhead]);
        assert_eq!(pattern(&[0, 54], None), [Cut, Overhead]);
        assert_eq!(pattern(&[0, 55], None), [Cut, Cut]);
        assert_eq!(pattern(&[0, 6, 24], None), [Cut, Backhand, Finisher]);
        assert_eq!(pattern(&[0, 6, 25], None), [Cut, Backhand, Rising]);
        assert_eq!(pattern(&[0, 6, 60], None), [Cut, Backhand, Rising]);
        assert_eq!(pattern(&[0, 6, 61], None), [Cut, Backhand]);
    }

    #[test]
    fn full_buffer_accepts_third_press_and_discards_finisher_spam() {
        use StrikeKind::*;
        let mut combat = Combat::default();
        let mut stamina = 100.0;
        for _ in 0..3 {
            combat.tick(true, &mut stamina);
        }
        assert_eq!(combat.queued(), [Some(Backhand), Some(Finisher)]);
        assert_eq!(stamina, 80.0, "queued strikes charged before starting");
        assert_eq!(pattern(&[0, 1, 2], Some(3..145)), [Cut, Backhand, Finisher]);
        assert_eq!(pattern(&[0, 24], Some(25..110)), [Cut, Overhead]);
    }

    #[test]
    fn expired_quick_pair_does_not_queue_a_ghost_new_chain() {
        use StrikeKind::*;
        assert_eq!(
            pattern(&[0, 6, 61, 62, 63, 110], None),
            [Cut, Backhand, Cut]
        );
    }

    #[test]
    fn every_strike_has_one_contact_after_windup_even_on_a_miss() {
        for kind in [
            StrikeKind::Cut,
            StrikeKind::Backhand,
            StrikeKind::Finisher,
            StrikeKind::Overhead,
            StrikeKind::Rising,
        ] {
            assert!(kind.windup_time() < kind.contact_time());
            assert!(kind.contact_time() < kind.follow_end());
            assert!(kind.follow_end() < kind.duration());
            let mut combat = Combat {
                active: Some(Strike::new(kind)),
                ..Combat::default()
            };
            let mut stamina = 100.0;
            let mut contacts = 0;
            for tick in 1..=90 {
                let sample = combat.tick(false, &mut stamina);
                if sample.contact.is_some() {
                    contacts += 1;
                    assert!(tick as f32 * STEP + 0.00001 >= kind.contact_time());
                    assert!((tick - 1) as f32 * STEP < kind.contact_time() + 0.00001);
                }
                assert!(!sample.frozen);
            }
            assert_eq!(contacts, 1);
            assert_eq!(combat.impact_event, 0);
            assert_eq!(combat.hitstop_left, 0.0);
        }
    }

    #[test]
    fn insufficient_stamina_clears_all_future_strikes() {
        let mut combat = Combat::default();
        let mut stamina = 39.0;
        for _ in 0..3 {
            combat.tick(true, &mut stamina);
        }
        assert_eq!(combat.queued_count(), 2);
        for _ in 0..180 {
            combat.tick(false, &mut stamina);
        }
        assert_eq!(combat.swing_event, 1);
        assert_eq!(stamina, 19.0);
        assert!(combat.finished());
        stamina = 100.0;
        combat.tick(false, &mut stamina);
        assert!(
            combat.active.is_none(),
            "stamina recovery revived a cleared queue"
        );
        combat.tick(true, &mut stamina);
        assert_eq!(combat.active.unwrap().kind, StrikeKind::Cut);
    }

    #[test]
    fn hitstop_freezes_motion_but_buffers_edges_on_the_live_rhythm_clock() {
        let mut combat = Combat::default();
        let mut stamina = 100.0;
        combat.tick(true, &mut stamina);
        for _ in 0..16 {
            combat.tick(false, &mut stamina);
        }
        combat.impact(StrikeKind::Cut, ImpactKind::Warden, Vec3::X);
        let frozen = combat.active;
        assert!(combat.tick(false, &mut stamina).frozen);
        assert!(combat.tick(true, &mut stamina).frozen); // exactly 18 ticks after first press
        assert_eq!(combat.active, frozen);
        assert_eq!(combat.queued(), [Some(StrikeKind::Backhand), None]);
        assert!(combat.tick(true, &mut stamina).frozen);
        assert_eq!(combat.queued_count(), 2);
        assert_eq!(combat.impact_event, 1);
    }

    #[test]
    fn rejected_start_does_not_charge_stamina_or_queue_a_later_attack() {
        let mut combat = Combat::default();
        let mut stamina = 19.0;
        combat.tick(true, &mut stamina);
        assert_eq!(stamina, 19.0);
        assert_eq!(combat.swing_event, 0);
        assert_eq!(combat.queued_count(), 0);
        assert!(combat.active.is_none());
        stamina = 100.0;
        for _ in 0..120 {
            combat.tick(false, &mut stamina);
        }
        assert_eq!(combat.swing_event, 0);
        assert!(combat.finished());
        combat.tick(true, &mut stamina);
        assert_eq!(combat.active.unwrap().kind, StrikeKind::Cut);
        assert_eq!(stamina, 80.0);
    }
}
