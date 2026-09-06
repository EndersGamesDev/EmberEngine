//! Small, typed bridge from authoritative match state to the Killshot HUD.

use arena_core::proto::PState;
use arena_core::shooter::{MAX_HP, RESERVE_INFINITE, weapon_stats};
use serde::Serialize;

/// Short taps survive the send cadence; paused/dead input needs a fresh release.
#[derive(Default)]
pub struct SlotInput {
    previous: u8,
    pending: u8,
    blocked: bool,
}

impl SlotInput {
    pub const fn update(&mut self, held: u8, enabled: bool) {
        if !enabled {
            self.pending = 0;
            self.blocked = true;
        } else if held == 0 {
            self.blocked = false;
        } else if !self.blocked && held != self.previous && held <= 9 {
            self.pending = held;
        }
        self.previous = held;
    }

    pub fn take(&mut self) -> u8 {
        std::mem::take(&mut self.pending)
    }
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    visible: bool,
    alive: bool,
    hp: u8,
    max_hp: u8,
    weapon: u8,
    magazine: u8,
    reserve: Option<u8>,
    reload_remaining: f32,
    reload_duration: f32,
    slots: [u8; 9],
    selected_slot: u8,
}

/// Extrapolate presentation at most one normal input interval. A lost connection
/// must not pretend the server completed a reload and awarded ammunition.
pub fn remaining(player: &PState, age: f32) -> f32 {
    if !player.alive || !player.reloading || !player.reload_remaining.is_finite() {
        return 0.0;
    }
    let age = if age.is_finite() {
        age.clamp(0.0, 0.05)
    } else {
        0.0
    };
    // Keep the indicator visible until authority clears reloading, even if
    // the estimated visual clock has reached zero before the next packet.
    (player.reload_remaining - age).max(0.001)
}

impl Snapshot {
    pub fn from_player(player: Option<&PState>, visible: bool, age: f32) -> Self {
        Self {
            visible,
            alive: player.is_some_and(|p| p.alive),
            hp: player.map_or(0, |p| p.hp.min(MAX_HP)),
            max_hp: MAX_HP,
            weapon: player.map_or(1, |p| p.weapon),
            magazine: player.map_or(0, |p| p.ammo),
            reserve: player.and_then(|p| (p.reserve != RESERVE_INFINITE).then_some(p.reserve)),
            reload_remaining: player.map_or(0.0, |p| remaining(p, age)),
            reload_duration: player.map_or(0.0, |p| weapon_stats(p.weapon).reload),
            slots: player.map_or([0; 9], |p| p.inventory.map(|slot| slot.weapon)),
            selected_slot: player.map_or(0, |p| p.weapon),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn publish(&self) {
        use wasm_bindgen::{JsCast as _, JsValue};
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(callback) =
            js_sys::Reflect::get(&window, &JsValue::from_str("emberUpdateKillshotHud"))
        else {
            return;
        };
        let Some(callback) = callback.dyn_ref::<js_sys::Function>() else {
            return;
        };
        if let Ok(json) = serde_json::to_string(self) {
            drop(callback.call1(&window, &JsValue::from_str(&json)));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::unused_self)] // Same call shape as the browser-only DOM bridge.
    pub const fn publish(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> PState {
        let mut value = serde_json::json!({
            "id":1,"x":0,"z":0,"ax":1,"az":0,"hp":5,"score":0,
            "alive":true,"crouch":false,"weapon":3,"ammo":9,"reserve":30,
            "reloading":true,"reload_remaining":1.2,
            "inventory":[]
        });
        let mut inventory = vec![serde_json::json!({"weapon":0,"ammo":0,"reserve":0}); 9];
        inventory[0] = serde_json::json!({"weapon":1,"ammo":6,"reserve":255});
        inventory[2] = serde_json::json!({"weapon":3,"ammo":9,"reserve":30});
        value["inventory"] = serde_json::json!(inventory);
        serde_json::from_value(value).expect("valid state")
    }

    #[test]
    fn slot_taps_survive_send_window_without_repeating_or_pause_leaking() {
        let mut input = SlotInput::default();
        input.update(3, true);
        input.update(0, true);
        assert_eq!(input.take(), 3);
        assert_eq!(input.take(), 0);
        input.update(4, true);
        assert_eq!(input.take(), 4);
        input.update(4, true);
        assert_eq!(input.take(), 0);
        input.update(5, false);
        input.update(5, true);
        assert_eq!(input.take(), 0);
        input.update(0, true);
        input.update(5, true);
        assert_eq!(input.take(), 5);
    }

    #[test]
    fn countdown_is_bounded_and_never_awards_ammo() {
        let p = player();
        assert!((remaining(&p, 20.0) - 1.15).abs() < 0.0001);
        assert_eq!(remaining(&p, f32::NAN), 1.2);
        let hud = Snapshot::from_player(Some(&p), true, 20.0);
        assert_eq!(hud.magazine, 9);
        assert_eq!(hud.slots, [1, 0, 3, 0, 0, 0, 0, 0, 0]);
        assert_eq!(hud.selected_slot, 3);
        assert_eq!(hud.reserve, Some(30));
    }

    #[test]
    fn death_and_absent_state_have_no_reload_and_infinite_is_null() {
        let mut p = player();
        p.alive = false;
        p.hp = 0;
        p.reserve = RESERVE_INFINITE;
        let hud = Snapshot::from_player(Some(&p), true, 0.0);
        assert!(!hud.alive);
        assert_eq!(hud.reload_remaining, 0.0);
        assert_eq!(hud.reserve, None);
        let absent = Snapshot::from_player(None, false, 0.0);
        assert!(!absent.visible);
        assert_eq!(absent.slots, [0; 9]);
    }
}
