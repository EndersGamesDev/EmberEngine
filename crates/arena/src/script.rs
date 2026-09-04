//! Scripted input (`EMBER_SCRIPT`): the client drives itself, hands off the
//! operator's machine.
//!
//! The capture harness used to press the operator's own keyboard and move
//! their own cursor: it clicked a window to focus it (which also asked the
//! game to grab the pointer), then `keybd_event`/`mouse_event` went into
//! whatever had focus. That takes the machine away from whoever is sitting
//! at it, and their stray mouse motion turned our camera, because winit
//! delivers raw `DeviceEvent::MouseMotion` whether the window is focused or
//! not. So the driving moved in here.
//!
//! `EMBER_SCRIPT` carries a timeline. While it is set the client reads *no*
//! device at all — no key, no mouse button, no mouse motion, no gamepad —
//! and the window never asks for the cursor (`run_online` passes
//! `capture_mouse: false`). Read once at startup, native only, like
//! `EMBER_WEAPON`, `EMBER_CAM` and `EMBER_ROUNDS`.
//!
//! # The grammar
//!
//! Steps are separated by `;` or newlines; `#` starts a comment. A step is a
//! few words and, last, an optional duration in seconds. A step with no
//! duration lasts exactly one frame, which is all an edge-latched intent
//! (jump, melee) needs.
//!
//! | word | meaning |
//! |---|---|
//! | `wait` | hold nothing |
//! | `walk DIRS` | hold a movement direction; `DIRS` is any of `w` `a` `s` `d` together, e.g. `wa` |
//! | `sprint [DIRS]` | hold sprint, optionally with a direction |
//! | `crouch [DIRS]` | hold crouch, optionally with a direction |
//! | `aim DEG` | face this heading: 0 looks down +X, 90 down +Z |
//! | `turn DEG` | turn by this much from wherever the view is |
//! | `look DEG` | set the elevation, + up (clamped by the sim's pitch limit) |
//! | `fire` | hold the trigger |
//! | `ads` | hold aim-down-sights |
//! | `shield` | hold the shield up |
//! | `reload` `jump` `melee` | the latched intents |
//!
//! Words combine inside one step, so the two captures that needed a held
//! modifier keep working: `ads fire 0.12` fires through the scope, and
//! `sprint w 2` is the run the old harness held a key for.
//!
//! ```text
//! wait 1.5; walk w 2; sprint w 2; crouch w 2; aim 90; fire 0.12; wait 0.5
//! ```
//!
//! Each piece is here because a committed capture needed it: `wait` for the
//! settle before a photograph, the three movement words for the third-person
//! and bonk sets, `aim`/`turn`/`look` because a scripted client has no other
//! way to point (the old harness could not turn at all), `fire` for the shot
//! the status line proves, `ads` for the scope pair, `shield` for the
//! scutum, and the three latched words for the reload, jump and melee taps.
//!
//! Time is the client's own frame clock: a step is held for its duration in
//! seconds of `dt`, so a script runs the same at any frame rate.

/// A yaw change a step asks for, applied once on the frame the step begins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Turn {
    /// An absolute heading, in radians.
    To(f32),
    /// A change from the current heading, in radians.
    By(f32),
}

/// One intent a step can hold down, one bit each in [`Held`].
///
/// A bitmask rather than a field per intent, for the same reason
/// `ember_engine::PadState` numbers its buttons that way: eight booleans in
/// a struct is eight chances to read the wrong one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Hold {
    Sprint = 0,
    Crouch = 1,
    Shield = 2,
    Fire = 3,
    Ads = 4,
    Reload = 5,
    Jump = 6,
    Melee = 7,
}

impl Hold {
    /// The bit this intent occupies.
    const fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

/// What a step holds down for its whole duration.
///
/// The latched intents (`Jump`, `Melee`, `Reload`) are held like the rest:
/// the client latches them on their rising edge exactly as it does a key, so
/// a one-frame step is one press.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    /// Forward on the camera axis, `-1..=1`.
    pub fwd: f32,
    /// Right on the camera axis, `-1..=1`.
    pub right: f32,
    /// One bit per [`Hold`].
    holds: u8,
}

impl Held {
    /// Whether this intent is held.
    #[must_use]
    pub const fn down(&self, h: Hold) -> bool {
        self.holds & h.mask() != 0
    }

    /// Hold it.
    const fn set(&mut self, h: Hold) {
        self.holds |= h.mask();
    }
}

/// One parsed step: what it holds, the angles it sets when it begins, and
/// how long it lasts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub held: Held,
    pub yaw: Option<Turn>,
    /// Absolute elevation in radians; the caller clamps it to the sim's limit.
    pub pitch: Option<f32>,
    pub secs: f32,
}

/// What the client should do this frame. The angles are `Some` only on the
/// first frame of the step that set them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    pub held: Held,
    pub yaw: Option<Turn>,
    pub pitch: Option<f32>,
}

/// The words a step can be built from, for the error message.
const VERBS: &str =
    "wait, walk, sprint, crouch, aim, turn, look, fire, ads, shield, reload, jump, melee";

/// Whether a token is one of the words above, so it is never mistaken for a
/// direction (`ads` is spelled out of `a`, `d` and `s`).
fn is_verb(t: &str) -> bool {
    matches!(
        t,
        "wait"
            | "walk"
            | "sprint"
            | "crouch"
            | "aim"
            | "turn"
            | "look"
            | "fire"
            | "ads"
            | "shield"
            | "reload"
            | "jump"
            | "melee"
    )
}

/// `(forward, right)` for a direction token, or `None` when the token is not
/// one — which is how `crouch 2` (crouch still) is told from `crouch w 2`.
fn dirs(t: &str) -> Option<(f32, f32)> {
    if t.is_empty() || is_verb(t) {
        return None;
    }
    let (mut f, mut r) = (0.0f32, 0.0f32);
    for c in t.chars() {
        match c {
            'w' => f += 1.0,
            's' => f -= 1.0,
            'd' => r += 1.0,
            'a' => r -= 1.0,
            _ => return None,
        }
    }
    Some((f.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0)))
}

/// A finite `f32` from a token, or `None`.
fn number(t: &str) -> Option<f32> {
    t.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// One step's words. `n` is its 1-based place in the script, for errors.
fn parse_step(src: &str, n: usize) -> Result<Step, String> {
    let bad = |m: &str| format!("script step {n} (`{src}`): {m}");
    let toks: Vec<&str> = src.split_whitespace().collect();
    let mut step = Step::default();
    let mut secs: Option<f32> = None;
    let mut i = 0;
    while let Some(&t) = toks.get(i) {
        i += 1;
        match t {
            "wait" => {}
            "walk" | "sprint" | "crouch" => {
                if t == "sprint" {
                    step.held.set(Hold::Sprint);
                } else if t == "crouch" {
                    step.held.set(Hold::Crouch);
                }
                if let Some((f, r)) = toks.get(i).copied().and_then(dirs) {
                    step.held.fwd = f;
                    step.held.right = r;
                    i += 1;
                } else if t == "walk" {
                    return Err(bad(
                        "`walk` needs a direction built from w/a/s/d, as in `walk w` or `walk wa`",
                    ));
                }
            }
            "aim" | "turn" | "look" => {
                let a = toks
                    .get(i)
                    .copied()
                    .ok_or_else(|| bad(&format!("`{t}` needs an angle in degrees")))?;
                let deg = number(a)
                    .ok_or_else(|| bad(&format!("`{t}` needs an angle in degrees, got `{a}`")))?;
                i += 1;
                match t {
                    "aim" => step.yaw = Some(Turn::To(deg.to_radians())),
                    "turn" => step.yaw = Some(Turn::By(deg.to_radians())),
                    _ => step.pitch = Some(deg.to_radians()),
                }
            }
            "fire" => step.held.set(Hold::Fire),
            "ads" => step.held.set(Hold::Ads),
            "shield" => step.held.set(Hold::Shield),
            "reload" => step.held.set(Hold::Reload),
            "jump" => step.held.set(Hold::Jump),
            "melee" => step.held.set(Hold::Melee),
            other => {
                let Some(v) = number(other) else { continue };
                if v < 0.0 {
                    return Err(bad(&format!(
                        "a duration cannot be negative, got `{other}`"
                    )));
                }
                if secs.is_some() {
                    return Err(bad(&format!("two durations (`{other}` is the second)")));
                }
                secs = Some(v);
            }
        }
    }
    step.secs = secs.unwrap_or(0.0);
    Ok(step)
}

/// A parsed script and where it has got to.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    steps: Vec<Step>,
    /// The step being held, or past the end when the script is spent.
    at: usize,
    /// Seconds of frame clock already spent on it.
    held: f32,
    /// Whether it has been emitted at least once — so a zero-duration step
    /// still gets its one frame.
    started: bool,
}

impl Timeline {
    /// Parse a script.
    ///
    /// # Errors
    ///
    /// Returns a message naming the step and the word when a step contains a
    /// word that is not in the grammar, an angle that is not a number, a
    /// negative duration, or two durations. Nothing is skipped silently: a
    /// script that does not parse whole does not parse at all.
    pub fn parse(src: &str) -> Result<Self, String> {
        let mut steps = Vec::new();
        for raw in src.split([';', '\n']) {
            let line = raw.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            steps.push(parse_step(line, steps.len() + 1)?);
        }
        Ok(Self {
            steps,
            at: 0,
            held: 0.0,
            started: false,
        })
    }

    /// The parsed steps, in order.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Whether every step has been held for its duration.
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.at >= self.steps.len()
    }

    /// Advance by one frame of the client's own clock and return what to do.
    ///
    /// Past the end this returns a neutral tick forever: the client keeps
    /// running so a capture can still take frames, but it holds nothing and
    /// it still reads no device. It never grabs the cursor on the way out —
    /// the grab was refused once, at startup, and is never re-asked.
    pub fn advance(&mut self, dt: f32) -> Tick {
        while self.started && self.steps.get(self.at).is_some_and(|s| self.held >= s.secs) {
            self.at += 1;
            self.held = 0.0;
            self.started = false;
        }
        let Some(step) = self.steps.get(self.at).copied() else {
            return Tick::default();
        };
        let first = !self.started;
        self.started = true;
        self.held += dt;
        Tick {
            held: step.held,
            yaw: if first { step.yaw } else { None },
            pitch: if first { step.pitch } else { None },
        }
    }
}

/// `EMBER_SCRIPT`, read once. Native only: there is no operator to protect
/// on the web and no environment to read it from.
#[cfg(not(target_arch = "wasm32"))]
fn env_script() -> Option<&'static str> {
    static SRC: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    SRC.get_or_init(|| {
        std::env::var("EMBER_SCRIPT")
            .ok()
            .filter(|s| !s.trim().is_empty())
    })
    .as_deref()
}

#[cfg(target_arch = "wasm32")]
const fn env_script() -> Option<&'static str> {
    None
}

/// Whether this process is script-driven. The window's mouse grab is decided
/// by this, in `run_online`, before the game exists — a scripted client must
/// never take the pointer, not even for the frame before its first step.
#[must_use]
pub fn scripted() -> bool {
    env_script().is_some()
}

/// The timeline for this process, if `EMBER_SCRIPT` is set.
///
/// A script that does not parse becomes an *empty* timeline, not `None`: the
/// client stays hands-off and simply does nothing, because the alternative —
/// falling back to reading the device — is exactly the bug this exists to
/// prevent. The error is logged, loudly, so a typo is not silent.
#[must_use]
pub fn from_env() -> Option<Timeline> {
    let src = env_script()?;
    let t = Timeline::parse(src).unwrap_or_else(|e| {
        tracing::error!(
            error = %e,
            "EMBER_SCRIPT did not parse; this client will do nothing (and still reads no device)"
        );
        Timeline::default()
    });
    // The harness reads the client's log; the step count is how it tells a
    // script that ran from one that never parsed.
    tracing::info!(
        steps = t.steps().len(),
        "EMBER_SCRIPT drives this client: no keyboard, no mouse, no pad, no cursor grab"
    );
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::{Held, Hold, Tick, Timeline, Turn};

    /// Close enough for an angle in radians converted from whole degrees.
    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn a_script_parses_to_the_steps_it_says() {
        let t = Timeline::parse(
            "wait 1.5; walk w 2; sprint w 2; crouch w 2; aim 90; fire 0.12; wait 0.5",
        )
        .expect("the documented example parses");
        let s = t.steps();
        assert_eq!(s.len(), 7);

        assert_eq!(s[0].held, Held::default());
        assert!(near(s[0].secs, 1.5));

        assert!(near(s[1].held.fwd, 1.0) && near(s[1].held.right, 0.0));
        assert!(!s[1].held.down(Hold::Sprint) && !s[1].held.down(Hold::Crouch));
        assert!(near(s[1].secs, 2.0));

        assert!(s[2].held.down(Hold::Sprint) && near(s[2].held.fwd, 1.0));
        assert!(s[3].held.down(Hold::Crouch) && near(s[3].held.fwd, 1.0));

        // An angle step sets a heading and lasts one frame.
        assert!(matches!(s[4].yaw, Some(Turn::To(y)) if near(y, std::f32::consts::FRAC_PI_2)));
        assert!(near(s[4].secs, 0.0));

        assert!(s[5].held.down(Hold::Fire) && near(s[5].secs, 0.12));
        assert_eq!(s[6].held, Held::default());
    }

    #[test]
    fn the_rest_of_the_grammar_parses() {
        let t = Timeline::parse(
            "walk wa 1 # strafing forward-left\nturn -45\nlook 12\nads fire 0.2; shield 1; crouch 0.5; jump; melee; reload",
        )
        .expect("every word parses");
        let s = t.steps();
        assert_eq!(s.len(), 9);
        assert!(near(s[0].held.fwd, 1.0) && near(s[0].held.right, -1.0));
        assert!(matches!(s[1].yaw, Some(Turn::By(y)) if near(y, (-45f32).to_radians())));
        assert!(s[2].pitch.is_some_and(|p| near(p, 12f32.to_radians())));
        // One step can hold two things: the scope capture fires while zoomed.
        assert!(s[3].held.down(Hold::Ads) && s[3].held.down(Hold::Fire) && near(s[3].secs, 0.2));
        assert!(s[4].held.down(Hold::Shield));
        // `crouch` with no direction crouches on the spot.
        assert!(
            s[5].held.down(Hold::Crouch) && near(s[5].held.fwd, 0.0) && near(s[5].held.right, 0.0)
        );
        assert!(
            s[6].held.down(Hold::Jump)
                && s[7].held.down(Hold::Melee)
                && s[8].held.down(Hold::Reload)
        );
    }

    #[test]
    fn a_bad_step_is_an_error_that_names_it_not_a_silent_skip() {
        // An unknown word: the whole script is refused, and the message
        // carries the step, its text, the word and the vocabulary.
        let e = Timeline::parse("walk w 1; sprnt w 2").expect_err("a typo is refused");
        assert!(e.contains("step 2"), "{e}");
        assert!(e.contains("sprnt"), "{e}");
        assert!(e.contains("wait, walk, sprint"), "{e}");

        // Not a skip: nothing of the script survives.
        assert!(Timeline::parse("walk w 1; nonsense").is_err());

        for (src, want) in [
            ("walk 2", "needs a direction"),
            ("aim", "needs an angle"),
            ("aim left", "needs an angle"),
            ("wait -1", "negative"),
            ("wait 1 2", "two durations"),
        ] {
            let e = Timeline::parse(src).map_or_else(|e| e, |_| String::from("<parsed>"));
            assert!(
                e.contains(want),
                "`{src}` should complain about {want}: {e}"
            );
        }
    }

    #[test]
    fn the_timeline_holds_each_step_for_its_duration_on_the_frame_clock() {
        // Two frame rates, one script: the same wall time in each state.
        for dt in [1.0 / 60.0, 1.0 / 15.0] {
            let mut t = Timeline::parse("walk w 0.5; crouch a 0.25").expect("parses");
            let (mut walked, mut crouched) = (0.0f32, 0.0f32);
            for _ in 0..200 {
                let tick = t.advance(dt);
                if tick.held.down(Hold::Crouch) {
                    crouched += dt;
                } else if tick.held.fwd > 0.5 {
                    walked += dt;
                }
            }
            assert!((walked - 0.5).abs() <= dt, "walked {walked} at dt={dt}");
            assert!(
                (crouched - 0.25).abs() <= dt,
                "crouched {crouched} at dt={dt}"
            );
            assert!(t.is_done());
            // Spent, and still hands-off: neutral forever after.
            assert_eq!(t.advance(dt), Tick::default());
        }
    }

    #[test]
    fn a_crouch_step_really_holds_crouch_and_an_angle_lands_once() {
        let mut t = Timeline::parse("aim 90; crouch w 0.1").expect("parses");
        let first = t.advance(1.0 / 60.0);
        assert!(matches!(first.yaw, Some(Turn::To(y)) if near(y, std::f32::consts::FRAC_PI_2)));
        assert!(!first.held.down(Hold::Crouch));
        let second = t.advance(1.0 / 60.0);
        // The heading is set once, not re-set every frame it is held.
        assert!(second.yaw.is_none());
        assert!(
            second.held.down(Hold::Crouch),
            "the crouch step sets crouch"
        );
        assert!(near(second.held.fwd, 1.0));
    }

    #[test]
    fn a_zero_duration_step_still_gets_one_frame() {
        // `jump` is edge-latched by the client, so it must be held for at
        // least one frame or the press never happens.
        let mut t = Timeline::parse("jump; wait 1").expect("parses");
        assert!(t.advance(0.5).held.down(Hold::Jump));
        assert!(!t.advance(0.5).held.down(Hold::Jump));
    }

    #[test]
    fn an_empty_or_comment_only_script_is_a_spent_timeline() {
        let t = Timeline::parse("  # nothing here \n\n ;; ").expect("parses");
        assert_eq!(t.steps(), []);
        assert!(t.is_done());
    }
}
