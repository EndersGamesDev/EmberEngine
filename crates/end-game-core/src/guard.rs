//! Fixed-step held sword guard and contact feedback.
use crate::STEP;
use glam::Vec3;

pub const RAISE_TIME: f32 = 0.18;
pub const LOWER_TIME: f32 = 0.12;
pub const IMPACT_TIME: f32 = 0.24;
pub const BREAK_TIME: f32 = 0.90;
pub const BLOCK_COST: f32 = 28.0;
pub const PARRY_WINDOW: f32 = 0.12;
pub const PARRY_STAMINA_COST: f32 = 22.0;
pub const PARRY_COOLDOWN: f32 = 0.50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardContact {
    Open,
    Blocked,
    Broken,
    Parried,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Guard {
    pub amount: f32,
    pub impact_left: f32,
    pub broken_left: f32,
    pub parry_window_left: f32,
    pub parry_cooldown: f32,
    pub block_event: u32,
    pub break_event: u32,
    pub parry_event: u32,
    pub impact_point: Vec3,
}

impl Guard {
    pub fn ready(&self) -> bool {
        self.amount >= 1.0 - 0.00001 && self.broken_left == 0.0
    }

    pub fn tick(&mut self, held: bool, available: bool) {
        self.impact_left = (self.impact_left - STEP).max(0.0);
        self.broken_left = (self.broken_left - STEP).max(0.0);
        self.parry_window_left = (self.parry_window_left - STEP).max(0.0);
        self.parry_cooldown = (self.parry_cooldown - STEP).max(0.0);
        let raising = held && available && self.broken_left == 0.0 && self.parry_cooldown == 0.0;
        self.amount = (self.amount
            + if raising {
                STEP / RAISE_TIME
            } else {
                -STEP / LOWER_TIME
            })
        .clamp(0.0, 1.0);
    }

    /// Facing, range and obstruction are decided by the dungeon first.
    pub fn receive(&mut self, stamina: &mut f32, point: Vec3) -> GuardContact {
        self.receive_cost(stamina, point, BLOCK_COST)
    }

    /// Castle weapons retain the same guard timing with an authored stamina cost.
    pub fn receive_cost(&mut self, stamina: &mut f32, point: Vec3, cost: f32) -> GuardContact {
        if !self.ready() {
            return GuardContact::Open;
        }
        self.impact_point = point;
        self.impact_left = IMPACT_TIME;
        if *stamina >= cost {
            *stamina -= cost;
            self.block_event = self.block_event.wrapping_add(1);
            GuardContact::Blocked
        } else {
            *stamina = 0.0;
            self.broken_left = BREAK_TIME;
            self.break_event = self.break_event.wrapping_add(1);
            GuardContact::Broken
        }
    }

    /// Parry: attempt to deflect an incoming attack during the parry window.
    /// Returns Parried if successful, Blocked if the parry failed but guard is up.
    pub fn try_parry(&mut self, stamina: &mut f32, point: Vec3, parry_cost: f32) -> GuardContact {
        if self.parry_cooldown > 0.0 {
            return if self.ready() {
                self.receive_cost(stamina, point, parry_cost)
            } else {
                GuardContact::Open
            };
        }
        if self.parry_window_left > 0.0 && *stamina >= parry_cost {
            *stamina -= parry_cost;
            self.parry_window_left = 0.0;
            self.parry_cooldown = PARRY_COOLDOWN;
            self.parry_event = self.parry_event.wrapping_add(1);
            self.impact_point = point;
            self.impact_left = IMPACT_TIME;
            GuardContact::Parried
        } else if self.ready() {
            self.receive_cost(stamina, point, parry_cost)
        } else {
            GuardContact::Open
        }
    }
}
