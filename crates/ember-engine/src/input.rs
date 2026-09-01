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
    /// Raw mouse movement accumulated since the last frame (mouse-look).
    mouse_delta: (f32, f32),
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            pressed: HashSet::new(),
            mouse: HashSet::new(),
            cursor_ndc: None,
            aspect: 16.0 / 9.0,
            mouse_delta: (0.0, 0.0),
        }
    }
}

impl InputState {
    #[must_use]
    pub fn down(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    /// 2D axis helper: (negative key, positive key) -> -1.0 / 0.0 / 1.0.
    #[must_use]
    pub fn axis(&self, neg: KeyCode, pos: KeyCode) -> f32 {
        match (self.down(neg), self.down(pos)) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    }

    #[must_use]
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        self.mouse.contains(&button)
    }

    #[must_use]
    pub const fn cursor_ndc(&self) -> Option<[f32; 2]> {
        self.cursor_ndc
    }

    #[must_use]
    pub const fn aspect(&self) -> f32 {
        self.aspect
    }

    /// Raw mouse movement (px) accumulated since the previous frame.
    /// Only meaningful while the cursor is captured (mouse-look).
    #[must_use]
    pub const fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    pub(crate) fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.mouse_delta.0 += dx;
        self.mouse_delta.1 += dy;
    }

    /// Called by the engine after each game update.
    pub(crate) const fn end_frame(&mut self) {
        self.mouse_delta = (0.0, 0.0);
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

    pub(crate) const fn set_cursor_ndc(&mut self, ndc: Option<[f32; 2]>) {
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
