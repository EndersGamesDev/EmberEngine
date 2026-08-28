use std::collections::HashSet;

use winit::keyboard::KeyCode;

/// Which physical keys are currently held. Snapshot-style: the game polls it
/// every frame instead of chasing events.
#[derive(Default)]
pub struct InputState {
    pressed: HashSet<KeyCode>,
}

impl InputState {
    pub fn down(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    /// 2D axis helper: (negative key, positive key) -> -1.0 / 0.0 / 1.0.
    pub fn axis(&self, neg: KeyCode, pos: KeyCode) -> f32 {
        (self.down(pos) as i32 - self.down(neg) as i32) as f32
    }

    pub(crate) fn press(&mut self, key: KeyCode) {
        self.pressed.insert(key);
    }

    pub(crate) fn release(&mut self, key: KeyCode) {
        self.pressed.remove(&key);
    }

    /// Called on focus loss so keys don't stick when alt-tabbing.
    pub(crate) fn clear(&mut self) {
        self.pressed.clear();
    }
}
