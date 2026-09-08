//! Fixed-step warden state and animation timing. World collision lives in Dungeon.
use crate::STEP;
use glam::Vec2;

pub const WAKE_DURATION: f32 = 1.65;
pub const KNIFE_WINDUP: f32 = 0.50;
pub const KNIFE_CONTACT: f32 = 0.63;
pub const KNIFE_FOLLOW: f32 = 0.78;
pub const KNIFE_DURATION: f32 = 1.42;
pub const STAGGER_DURATION: f32 = 0.42;
pub const HIT_DURATION: f32 = 0.24;
pub const FLINCH_DURATION: f32 = 0.22;
pub const WALK_SPEED: f32 = 1.25;
const TURN_SPEED: f32 = 3.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WardenPhase {
    #[default]
    Sleeping,
    Waking,
    Hunting,
    Attacking,
    Staggered,
    Dead,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Warden {
    pub phase: WardenPhase,
    /// Seconds in the current phase, excluding pause and global hitstop.
    pub elapsed: f32,
    /// Same convention as the player: zero faces -Z; PI faces the cell at +Z.
    pub yaw: f32,
    /// Radians advanced by actual distance travelled (one stride per 1.2 m).
    pub walk_phase: f32,
    /// Smoothly plant the feet when pursuit stops or commits to a knife windup.
    pub walk_blend: f32,
    pub attack_event: u32,
    /// Successful knife contacts against the player, not received sword hits.
    pub hit_event: u32,
    pub hit_left: f32,
    /// A received sword hit can flinch without interrupting a light-hit attack.
    pub flinch_left: f32,
    contact_done: bool,
    death_stand: f32,
    death_knife: f32,
    interrupted_attack: Option<(f32, f32)>,
    flinch_origin: f32,
}

impl Default for Warden {
    fn default() -> Self {
        Self {
            phase: WardenPhase::Sleeping,
            elapsed: 0.0,
            yaw: std::f32::consts::PI,
            walk_phase: 0.0,
            walk_blend: 0.0,
            attack_event: 0,
            hit_event: 0,
            hit_left: 0.0,
            flinch_left: 0.0,
            contact_done: false,
            death_stand: 0.0,
            death_knife: 0.0,
            interrupted_attack: None,
            flinch_origin: 0.0,
        }
    }
}

fn ease(start: f32, end: f32, time: f32) -> f32 {
    let t = ((time - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Warden {
    pub fn stand_amount(&self) -> f32 {
        match self.phase {
            WardenPhase::Sleeping => 0.0,
            WardenPhase::Waking => ease(0.12, 1.25, self.elapsed),
            // Keep a fatal hit during the rise from snapping the corpse upright.
            WardenPhase::Dead => self.death_stand,
            _ => 1.0,
        }
    }

    pub fn knife_draw(&self) -> f32 {
        match self.phase {
            WardenPhase::Sleeping => 0.0,
            WardenPhase::Waking => ease(0.80, 1.62, self.elapsed),
            WardenPhase::Dead => self.death_knife,
            _ => 1.0,
        }
    }

    /// Authored attack time and its contribution to the current pose. An
    /// interruption samples this before changing phase, so it never resumes a
    /// cancelled attack clock or adds another knife contact.
    pub fn attack_pose(&self) -> Option<(f32, f32)> {
        match self.phase {
            WardenPhase::Attacking => Some((self.elapsed, 1.0)),
            WardenPhase::Staggered => self.interrupted_attack.map(|(time, weight)| {
                (
                    time,
                    weight * (1.0 - ease(0.0, STAGGER_DURATION, self.elapsed)),
                )
            }),
            WardenPhase::Dead => self.interrupted_attack,
            _ => None,
        }
    }

    /// Start a new flinch from the current displacement, then settle to zero.
    /// This is continuous even when a fatal hit interrupts an earlier flinch.
    pub fn flinch_amount(&self) -> f32 {
        if self.flinch_left <= 0.0 {
            return 0.0;
        }
        let age = (FLINCH_DURATION - self.flinch_left).clamp(0.0, FLINCH_DURATION);
        (self.flinch_origin + (1.0 - self.flinch_origin) * ease(0.0, 0.045, age))
            * (1.0 - ease(0.045, FLINCH_DURATION, age))
    }

    pub fn label(&self) -> &'static str {
        match self.phase {
            WardenPhase::Sleeping => "Asleep",
            WardenPhase::Waking => "Waking · drawing knife",
            WardenPhase::Hunting => "Hunting",
            WardenPhase::Attacking if self.elapsed < KNIFE_WINDUP => "Knife · windup",
            WardenPhase::Attacking if self.elapsed < KNIFE_FOLLOW => "Knife · strike",
            WardenPhase::Attacking => "Knife · recovery",
            WardenPhase::Staggered => "Staggered",
            WardenPhase::Dead => "Fallen",
        }
    }

    pub fn forward(&self) -> Vec2 {
        Vec2::new(self.yaw.sin(), -self.yaw.cos())
    }

    pub fn threaten(&mut self) {
        if self.phase == WardenPhase::Sleeping {
            self.enter(WardenPhase::Waking);
        }
    }

    /// Preserve the current stance as the origin of the collapse animation.
    pub fn die(&mut self) {
        if self.phase != WardenPhase::Dead {
            let attack = self.attack_pose();
            self.death_stand = self.stand_amount();
            self.death_knife = self.knife_draw();
            self.enter(WardenPhase::Dead);
            self.interrupted_attack = attack;
        }
    }

    pub fn on_sword_hit(&mut self, heavy: bool, killed: bool) {
        if self.phase == WardenPhase::Dead {
            return;
        }
        self.flinch_origin = self.flinch_amount();
        self.flinch_left = FLINCH_DURATION;
        if killed {
            self.die();
        } else if self.phase == WardenPhase::Sleeping {
            self.threaten();
        } else if heavy && self.phase != WardenPhase::Waking {
            let attack = self.attack_pose();
            self.enter(WardenPhase::Staggered);
            self.interrupted_attack = attack;
        }
    }

    fn enter(&mut self, phase: WardenPhase) {
        self.phase = phase;
        self.elapsed = 0.0;
        self.contact_done = false;
        self.interrupted_attack = None;
    }

    pub(crate) fn begin_attack(&mut self) {
        self.enter(WardenPhase::Attacking);
        self.attack_event = self.attack_event.wrapping_add(1);
    }

    pub(crate) fn turn_toward(&mut self, direction: Vec2) {
        if direction.length_squared() < 0.00001 {
            return;
        }
        let target = direction.x.atan2(-direction.y);
        let delta = (target - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.yaw = (self.yaw + delta.clamp(-TURN_SPEED * STEP, TURN_SPEED * STEP))
            .rem_euclid(std::f32::consts::TAU);
    }

    pub(crate) fn walked(&mut self, distance: f32) {
        if distance > 0.00001 {
            self.walk_blend = (self.walk_blend + STEP * 10.0).min(1.0);
        }
        self.walk_phase = (self.walk_phase + distance * std::f32::consts::TAU / 1.2)
            .rem_euclid(std::f32::consts::TAU);
    }

    pub(crate) fn hit_player(&mut self) {
        self.hit_event = self.hit_event.wrapping_add(1);
        self.hit_left = HIT_DURATION;
    }

    /// Advance only on an unfrozen simulation tick. True requests exactly one
    /// world contact sample; a miss is consumed just like a successful contact.
    pub(crate) fn advance(&mut self) -> bool {
        self.elapsed += STEP;
        self.walk_blend = (self.walk_blend - STEP * 5.0).max(0.0);
        self.hit_left = (self.hit_left - STEP).max(0.0);
        self.flinch_left = (self.flinch_left - STEP).max(0.0);
        let contact = self.phase == WardenPhase::Attacking
            && !self.contact_done
            && self.elapsed + 0.00001 >= KNIFE_CONTACT;
        if contact {
            self.contact_done = true;
        }
        match self.phase {
            WardenPhase::Waking if self.elapsed + 0.00001 >= WAKE_DURATION => {
                self.enter(WardenPhase::Hunting);
            }
            WardenPhase::Attacking if self.elapsed + 0.00001 >= KNIFE_DURATION => {
                self.enter(WardenPhase::Hunting);
            }
            WardenPhase::Staggered if self.elapsed + 0.00001 >= STAGGER_DURATION => {
                self.enter(WardenPhase::Hunting);
            }
            _ => {}
        }
        contact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_attack_pose_recovers_without_contact_or_stale_next_attack() {
        for time in [KNIFE_WINDUP, KNIFE_CONTACT] {
            let mut warden = Warden::default();
            warden.begin_attack();
            warden.elapsed = time;
            warden.on_sword_hit(true, false);
            assert_eq!(warden.phase, WardenPhase::Staggered);
            assert_eq!(warden.attack_pose(), Some((time, 1.0)));
            assert_eq!(warden.flinch_amount(), 0.0);
            let mut previous_weight = 1.0;
            for _ in 0..26 {
                assert!(!warden.advance(), "cancelled knife must not sample contact");
                if let Some((sample, weight)) = warden.attack_pose() {
                    assert_eq!(sample, time);
                    assert!(weight <= previous_weight);
                    previous_weight = weight;
                }
            }
            assert_eq!(warden.phase, WardenPhase::Hunting);
            assert_eq!(warden.attack_pose(), None);
            assert_eq!(warden.interrupted_attack, None);
            warden.begin_attack();
            assert_eq!(warden.attack_pose(), Some((0.0, 1.0)));
            assert_eq!(warden.attack_event, 2);
            for _ in 0..37 {
                assert!(!warden.advance());
            }
            assert!(warden.advance());
            assert!(!warden.advance());
        }
    }

    #[test]
    fn fatal_hit_retains_attack_or_partially_recovered_pose_and_flinch() {
        for time in [KNIFE_WINDUP, KNIFE_CONTACT] {
            for stagger_first in [false, true] {
                let mut warden = Warden::default();
                warden.begin_attack();
                warden.elapsed = time;
                if stagger_first {
                    warden.on_sword_hit(true, false);
                    for _ in 0..8 {
                        assert!(!warden.advance());
                    }
                }
                let pose = warden.attack_pose();
                let flinch = warden.flinch_amount();
                warden.on_sword_hit(false, true);
                assert_eq!(warden.phase, WardenPhase::Dead);
                assert_eq!(warden.elapsed, 0.0);
                assert_eq!(warden.attack_pose(), pose);
                assert_eq!(warden.flinch_amount(), flinch);
                for _ in 0..120 {
                    assert!(!warden.advance());
                    assert_eq!(warden.attack_pose(), pose);
                }
            }
        }
    }

    #[test]
    fn flinch_rises_smoothly_and_repeated_hit_keeps_its_current_displacement() {
        let mut warden = Warden::default();
        warden.phase = WardenPhase::Hunting;
        warden.on_sword_hit(false, false);
        assert_eq!(warden.flinch_amount(), 0.0);
        warden.advance();
        let first = warden.flinch_amount();
        assert!(first > 0.0 && first < 0.5);
        warden.on_sword_hit(false, false);
        assert_eq!(warden.flinch_amount(), first);
        for _ in 0..14 {
            warden.advance();
        }
        assert_eq!(warden.flinch_amount(), 0.0);
    }

    #[test]
    fn feet_settle_when_the_warden_stops_without_restarting_the_stride() {
        let mut warden = Warden::default();
        warden.phase = WardenPhase::Hunting;
        for _ in 0..20 {
            warden.advance();
            warden.walked(WALK_SPEED * STEP);
        }
        assert_eq!(warden.walk_blend, 1.0);
        let phase = warden.walk_phase;
        warden.begin_attack();
        for _ in 0..13 {
            warden.advance();
        }
        assert_eq!(warden.walk_blend, 0.0);
        assert_eq!(warden.walk_phase, phase);
        assert!(warden.elapsed < KNIFE_WINDUP);
    }

    #[test]
    fn sword_hits_preserve_gradual_waking_and_fatal_pose() {
        let mut warden = Warden::default();
        warden.on_sword_hit(true, false);
        assert_eq!(warden.phase, WardenPhase::Waking);
        assert_eq!(warden.stand_amount(), 0.0);
        assert_eq!(warden.knife_draw(), 0.0);
        for _ in 0..42 {
            assert!(!warden.advance());
        }
        let elapsed = warden.elapsed;
        let stand = warden.stand_amount();
        assert!(stand > 0.0 && stand < 1.0);
        warden.on_sword_hit(true, false);
        assert_eq!(warden.phase, WardenPhase::Waking);
        assert_eq!(warden.elapsed, elapsed);
        assert_eq!(warden.stand_amount(), stand);
        let drawn = warden.knife_draw();
        warden.on_sword_hit(false, true);
        assert_eq!(warden.phase, WardenPhase::Dead);
        assert_eq!(warden.elapsed, 0.0);
        assert_eq!(warden.stand_amount(), stand);
        assert_eq!(warden.knife_draw(), drawn);
        for _ in 0..120 {
            assert!(!warden.advance());
        }
        assert!(warden.elapsed > 1.99);
        assert_eq!(warden.phase, WardenPhase::Dead);
    }

    #[test]
    fn wake_and_stagger_complete_at_fixed_tick_boundaries() {
        let mut warden = Warden::default();
        warden.threaten();
        for _ in 0..98 {
            warden.advance();
            assert_eq!(warden.phase, WardenPhase::Waking);
        }
        warden.advance();
        assert_eq!(warden.phase, WardenPhase::Hunting);
        assert_eq!(warden.elapsed, 0.0);
        assert_eq!(warden.stand_amount(), 1.0);
        assert_eq!(warden.knife_draw(), 1.0);
        warden.begin_attack();
        warden.on_sword_hit(true, false);
        for _ in 0..25 {
            assert!(!warden.advance());
            assert_eq!(warden.phase, WardenPhase::Staggered);
        }
        assert!(!warden.advance());
        assert_eq!(warden.phase, WardenPhase::Hunting);
        assert_eq!(warden.hit_event, 0);
    }

    #[test]
    fn turning_is_bounded_and_uses_player_yaw_convention() {
        let mut warden = Warden::default();
        let before = warden.yaw;
        warden.turn_toward(Vec2::X);
        assert!((warden.yaw - before).abs() <= TURN_SPEED * STEP + 0.00001);
        for _ in 0..60 {
            warden.turn_toward(Vec2::X);
        }
        assert!(warden.forward().distance(Vec2::X) < 0.0001);
        for _ in 0..60 {
            warden.turn_toward(-Vec2::Y);
        }
        assert!(warden.forward().distance(-Vec2::Y) < 0.0001);
    }
}
