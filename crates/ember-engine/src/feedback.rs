//! What a game hands back to the platform after each update: things the
//! player should feel rather than see.
//!
//! The one-way layering holds here exactly as it does for drawing. The game
//! returns plain data, `app.rs` (the platform layer) turns it into motor
//! commands, and the renderer never learns any of it exists. Today the only
//! channel is gamepad rumble; the struct is a bag so a later channel (an
//! audio cue routed through the platform, say) is a field, not a new trait
//! method.

/// One rumble request: motor magnitudes in `0..=1` and a duration.
///
/// Requests made in one frame, and requests that arrive while an earlier one
/// is still playing, are merged by the platform: each motor takes the `max`
/// of the magnitudes asked of it, and the merged rumble ends at the latest
/// end among them. There is one end, not one per motor, so a request holds
/// its magnitude in the merge until that end passes, however short it asked
/// for. A 30 ms hitmarker tick that lands inside a 400 ms death rumble
/// therefore never cancels or shortens the death, and it raises the weak
/// motor for the rest of the 400, not for 30. The trade is deliberate: a
/// per-motor end would need a re-play in the middle of a rumble on both
/// platforms, and a hitmarker held inside a death rumble is not felt as
/// wrong, where a death rumble cut short by a hitmarker is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rumble {
    /// The heavy, low-frequency motor (the left one on an Xbox-style pad).
    pub strong: f32,
    /// The light, high-frequency motor (the right one on an Xbox-style pad).
    pub weak: f32,
    /// How long the request lasts, in milliseconds. The browser's actuator
    /// rejects an effect longer than 5000 ms, so the page's shim clamps the
    /// merged duration there; native plays the full length.
    pub ms: u16,
}

/// Everything a game wants felt this frame.
///
/// Returned by [`EmberGame::feedback`](crate::EmberGame::feedback) and
/// consumed by the platform; an empty value costs nothing, which is what the
/// default trait method returns for every game that has not asked for
/// haptics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feedback {
    /// Rumble requests in the order they were made. Order does not matter to
    /// the platform, which merges them; it is kept so a test can read back
    /// exactly what the game asked for.
    pub rumbles: Vec<Rumble>,
}

impl Feedback {
    /// Queue one rumble. Magnitudes are clamped to `0..=1` here, once, so the
    /// platform can trust every value it merges and a game cannot push a
    /// motor past full by adding two requests together.
    pub fn rumble(&mut self, strong: f32, weak: f32, ms: u16) {
        self.rumbles.push(Rumble {
            strong: clamp_unit(strong),
            weak: clamp_unit(weak),
            ms,
        });
    }
}

/// Clamp to `0..=1`, treating NaN as silence: a NaN magnitude would otherwise
/// win every `max` in the merge and pin a motor on.
const fn clamp_unit(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rumble_clamps_magnitudes_and_keeps_order() {
        let mut fb = Feedback::default();
        fb.rumble(1.5, -0.2, 40);
        fb.rumble(f32::NAN, 0.5, 300);
        assert_eq!(
            fb.rumbles,
            vec![
                Rumble {
                    strong: 1.0,
                    weak: 0.0,
                    ms: 40
                },
                Rumble {
                    strong: 0.0,
                    weak: 0.5,
                    ms: 300
                },
            ]
        );
    }
}
