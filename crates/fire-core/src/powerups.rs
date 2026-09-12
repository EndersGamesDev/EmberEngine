//! Tactical items. All collection and combat run in `Race`, never prediction.

use glam::Vec2;

pub const NITRO: u8 = 1;
pub const SHIELD: u8 = 2;
pub const PULSE: u8 = 3;
pub const OIL: u8 = 4;
pub const GRIP: u8 = 5;
pub const PICKUP_RADIUS: f32 = 2.25;
pub const PICKUP_RESPAWN: f32 = 5.0;
pub const OIL_RADIUS: f32 = 2.7;
pub const PULSE_SPEED: f32 = 66.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pickup {
    pub id: u16,
    pub pos: Vec2,
    pub respawn_left: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    pub owner: u8,
    pub target: u8,
    pub pos: Vec2,
    pub life_left: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oil {
    pub owner: u8,
    pub pos: Vec2,
    pub life_left: f32,
}

/// A tick-indexed integer draw: replayable and independent of rendering rate.
/// Trailing positions get more speed/recovery; leaders can still defend.
#[must_use]
pub fn draw(tick: u64, box_id: u16, driver: usize, place: usize, count: usize) -> u8 {
    let mut seed = tick
        .wrapping_add(u64::from(box_id).wrapping_mul(0x9e37_79b9))
        .wrapping_add(u64::try_from(driver).unwrap_or(0).wrapping_mul(0x85eb_ca6b));
    seed ^= seed >> 16;
    seed = seed.wrapping_mul(0x7feb_352d);
    seed ^= seed >> 15;
    let roll = seed % 10;
    if place >= count / 2 && count > 1 {
        match roll {
            0..=3 => NITRO,
            4..=5 => PULSE,
            6..=7 => GRIP,
            8 => SHIELD,
            _ => OIL,
        }
    } else {
        match roll {
            0..=1 => NITRO,
            2..=4 => SHIELD,
            5 => PULSE,
            6..=7 => OIL,
            _ => GRIP,
        }
    }
}

#[must_use]
pub const fn name(item: u8) -> &'static str {
    match item {
        NITRO => "NITRO",
        SHIELD => "SHIELD",
        PULSE => "PULSE",
        OIL => "OIL",
        GRIP => "GRIP",
        _ => "EMPTY",
    }
}
