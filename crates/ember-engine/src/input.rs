use std::collections::HashSet;

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Radius of the stick's dead zone, as a fraction of full deflection. Below
/// it a stick reports exactly zero, so a worn pad that rests slightly off
/// centre does not creep the player.
pub const STICK_DEAD_ZONE: f32 = 0.18;
/// Exponent of the response curve applied to the stick magnitude after the
/// dead zone. Above 1 so the first half of the travel is fine control and the
/// last quarter is the sprint; 1.8 was chosen by feel, not derived.
pub const STICK_CURVE: f32 = 1.8;

/// A gamepad button, numbered by the W3C Gamepad API "standard" mapping.
///
/// One numbering so both platforms produce one bitmask: `PadState::buttons`
/// bit `b as u16` is that button. The names are `XInput`'s (South is A, East
/// is B, West is X, North is Y); the numbers are the browser's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PadButton {
    South = 0,
    East = 1,
    West = 2,
    North = 3,
    /// The left bumper.
    LB = 4,
    /// The right bumper.
    RB = 5,
    /// The left trigger past its click threshold; `PadState::lt` has the analogue value.
    LT = 6,
    /// The right trigger past its click threshold; `PadState::rt` has the analogue value.
    RT = 7,
    Back = 8,
    Start = 9,
    /// The left stick pressed in.
    L3 = 10,
    /// The right stick pressed in.
    R3 = 11,
    Up = 12,
    Down = 13,
    Left = 14,
    Right = 15,
}

impl PadButton {
    /// Every button, in bit order, so a table over the buttons can be checked
    /// for completeness by a test.
    pub const ALL: [Self; 16] = [
        Self::South,
        Self::East,
        Self::West,
        Self::North,
        Self::LB,
        Self::RB,
        Self::LT,
        Self::RT,
        Self::Back,
        Self::Start,
        Self::L3,
        Self::R3,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
    ];

    /// The bit this button occupies in `PadState::buttons`.
    #[must_use]
    pub const fn mask(self) -> u16 {
        1 << (self as u16)
    }
}

/// A snapshot of the first connected gamepad, taken by the platform once per
/// frame before the game's update.
///
/// Sticks are already dead-zoned and curved by [`PadState::stick`], with Y
/// up-positive on both platforms (the browser reports down-positive and the
/// platform negates it), so a game reads `left` straight into its movement
/// and must not apply the curve a second time. Triggers are the raw analogue
/// `0..=1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    /// The left stick, `[x, y]`, each in `-1..=1`, curved.
    pub left: [f32; 2],
    /// The right stick, `[x, y]`, each in `-1..=1`, curved.
    pub right: [f32; 2],
    /// The left trigger, `0..=1`.
    pub lt: f32,
    /// The right trigger, `0..=1`.
    pub rt: f32,
    /// One bit per [`PadButton`], by its standard-mapping index.
    pub buttons: u16,
}

impl PadState {
    /// Whether a button is held this frame.
    #[must_use]
    pub const fn down(&self, b: PadButton) -> bool {
        self.buttons & b.mask() != 0
    }

    /// Apply the radial dead zone and the response curve to a raw stick.
    ///
    /// Radial, not per axis, so a diagonal push does not snap to the axes at
    /// the dead zone's edge: the magnitude is remapped from
    /// `STICK_DEAD_ZONE..=1` onto `0..=1` and raised to `STICK_CURVE`, and
    /// the direction is kept exactly. A raw magnitude above 1 (some pads
    /// overshoot on the diagonals) is treated as 1, so the output never
    /// leaves the unit disc.
    #[must_use]
    pub fn stick(raw: [f32; 2]) -> [f32; 2] {
        let m = raw[0].hypot(raw[1]);
        if m.is_nan() || m <= STICK_DEAD_ZONE {
            // A NaN axis reads as centred, never as a push.
            return [0.0, 0.0];
        }
        let m_out = ((m.min(1.0) - STICK_DEAD_ZONE) / (1.0 - STICK_DEAD_ZONE)).powf(STICK_CURVE);
        let k = m_out / m;
        [raw[0] * k, raw[1] * k]
    }
}

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
    /// The first connected gamepad this frame, if any.
    pad: Option<PadState>,
    /// What the platform found when it probed for pads and rumble; see
    /// [`InputState::pad_status`].
    pad_status: &'static str,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            pressed: HashSet::new(),
            mouse: HashSet::new(),
            cursor_ndc: None,
            aspect: 16.0 / 9.0,
            mouse_delta: (0.0, 0.0),
            pad: None,
            pad_status: "none",
        }
    }
}

impl InputState {
    /// An input snapshot built by hand, as if the platform had just filled
    /// it: keys held, buttons held, raw mouse motion since the last frame,
    /// and a pad.
    ///
    /// The setters are `pub(crate)` because only the platform may fill this
    /// from a device; this exists so a game's own tests can present one —
    /// notably to prove that a client which is *not* meant to read the
    /// device ignores a snapshot that has everything held down.
    #[must_use]
    pub fn from_parts(
        keys: &[KeyCode],
        buttons: &[MouseButton],
        mouse_delta: (f32, f32),
        pad: Option<PadState>,
    ) -> Self {
        Self {
            pressed: keys.iter().copied().collect(),
            mouse: buttons.iter().copied().collect(),
            mouse_delta,
            pad,
            ..Self::default()
        }
    }

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

    /// The first connected gamepad, or `None` when there is none, when the
    /// platform has no pad support, or when the window lost focus (a held
    /// trigger must not keep firing into an alt-tabbed game any more than a
    /// held key does).
    #[must_use]
    pub const fn pad(&self) -> Option<PadState> {
        self.pad
    }

    /// What the platform found: `none` until a pad has been seen,
    /// `input-only` for a pad without force feedback (or, on the web, a page
    /// without the rumble shim), `input+rumble` when both work. A page shows
    /// it beside its renderer probe so a player without rumble can tell a
    /// missing feature from a broken one.
    #[must_use]
    pub const fn pad_status(&self) -> &'static str {
        self.pad_status
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

    pub(crate) const fn set_pad(&mut self, pad: Option<PadState>) {
        self.pad = pad;
    }

    pub(crate) const fn set_pad_status(&mut self, status: &'static str) {
        self.pad_status = status;
    }

    /// Called on focus loss so keys don't stick when alt-tabbing. The pad
    /// goes with them: it is re-read on the next frame the platform polls it,
    /// which is how a held trigger stops mattering while the game is behind
    /// another window.
    pub(crate) fn clear(&mut self) {
        self.pressed.clear();
        self.mouse.clear();
        self.pad = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_axis_curve_is_dead_below_threshold_and_monotonic() {
        // Dead zone: anything inside the radius is exactly zero, on the axes
        // and on the diagonal alike (the zone is radial).
        assert_eq!(PadState::stick([0.0, 0.0]), [0.0, 0.0]);
        assert_eq!(PadState::stick([0.17, 0.0]), [0.0, 0.0]);
        assert_eq!(PadState::stick([-0.1, 0.1]), [0.0, 0.0]);
        assert_eq!(PadState::stick([f32::NAN, 0.3]), [0.0, 0.0]);

        // Monotonic in the magnitude, zero at the threshold, one at full
        // deflection, and never above one.
        let mut last = 0.0f32;
        for i in 0..=1000u16 {
            let m = f32::from(i) / 1000.0;
            let out = PadState::stick([m, 0.0]);
            assert_eq!(out[1], 0.0);
            assert!(
                out[0] >= last,
                "not monotonic at m={m}: {} < {last}",
                out[0]
            );
            assert!(out[0] <= 1.0);
            last = out[0];
        }
        assert!((last - 1.0).abs() < 1e-6, "full deflection reads {last}");
        assert!(PadState::stick([STICK_DEAD_ZONE + 1e-4, 0.0])[0] < 1e-3);

        // The direction is kept: a diagonal push comes out on the diagonal.
        let d = PadState::stick([0.6, -0.6]);
        assert!((d[0] + d[1]).abs() < 1e-6);
        assert!(d[0] > 0.0 && d[1] < 0.0);

        // Overshoot past the unit circle is clamped, not amplified.
        let o = PadState::stick([1.0, 1.0]);
        assert!(o[0].hypot(o[1]) <= 1.0 + 1e-6);

        // The curve bends the middle down: half travel reads well under half.
        let half = PadState::stick([0.59, 0.0])[0];
        assert!(half < 0.4, "half travel reads {half}");
    }

    #[test]
    fn pad_buttons_are_one_bit_each_in_standard_order() {
        for (i, b) in PadButton::ALL.iter().enumerate() {
            assert_eq!(*b as usize, i);
            assert_eq!(b.mask(), 1 << i);
        }
        let st = PadState {
            buttons: PadButton::South.mask() | PadButton::RT.mask(),
            ..PadState::default()
        };
        assert!(st.down(PadButton::South));
        assert!(st.down(PadButton::RT));
        assert!(!st.down(PadButton::East));
    }

    #[test]
    fn losing_focus_drops_the_pad_with_the_keys() {
        let mut input = InputState::default();
        input.press(KeyCode::KeyW);
        input.set_pad(Some(PadState {
            rt: 1.0,
            ..PadState::default()
        }));
        input.clear();
        assert!(!input.down(KeyCode::KeyW));
        assert_eq!(input.pad(), None);
    }
}
