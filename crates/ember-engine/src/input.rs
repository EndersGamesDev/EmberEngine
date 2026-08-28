use std::collections::HashSet;

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Which physical keys/buttons are currently held, plus pointer state.
/// Snapshot-style: the game polls it every frame instead of chasing events.
pub struct InputState {
    pressed: HashSet<KeyCode>,
    mouse: HashSet<MouseButton>,
    /// Cursor in normalized device coordinates (-1..1, +y up), if the
    /// cursor is over the window.
    cursor_ndc: Option<[f32; 2]>,
    /// Current viewport aspect ratio — what the game needs (together with
    /// its own camera) to unproject the cursor into the world.
    aspect: f32,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            pressed: HashSet::new(),
            mouse: HashSet::new(),
            cursor_ndc: None,
            aspect: 16.0 / 9.0,
        }
    }
}

impl InputState {
    pub fn down(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    /// 2D axis helper: (negative key, positive key) -> -1.0 / 0.0 / 1.0.
    pub fn axis(&self, neg: KeyCode, pos: KeyCode) -> f32 {
        (self.down(pos) as i32 - self.down(neg) as i32) as f32
    }

    pub fn mouse_down(&self, button: MouseButton) -> bool {
        self.mouse.contains(&button)
    }

    pub fn cursor_ndc(&self) -> Option<[f32; 2]> {
        self.cursor_ndc
    }

    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    pub(crate) fn press(&mut self, key: KeyCode) {
        self.pressed.insert(key);
    }

    pub(crate) fn release(&mut self, key: KeyCode) {
        self.pressed.remove(&key);
    }

    pub(crate) fn mouse_press(&mut self, button: MouseButton) {
        self.mouse.insert(button);
    }

    pub(crate) fn mouse_release(&mut self, button: MouseButton) {
        self.mouse.remove(&button);
    }

    pub(crate) fn set_cursor_ndc(&mut self, ndc: Option<[f32; 2]>) {
        self.cursor_ndc = ndc;
    }

    pub(crate) fn set_aspect(&mut self, aspect: f32) {
        if aspect.is_finite() && aspect > 0.0 {
            self.aspect = aspect;
        }
    }

    /// Called on focus loss so keys don't stick when alt-tabbing.
    pub(crate) fn clear(&mut self) {
        self.pressed.clear();
        self.mouse.clear();
    }
}
