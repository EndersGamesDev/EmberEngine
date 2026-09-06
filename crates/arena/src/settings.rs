//! Arena's keyboard/mouse preferences. The browser menu owns persistence and
//! publishes a revisioned snapshot; neither storage nor JSON is read per frame.
//! Physical key codes intentionally exclude OS/browser shortcut modifiers.

use ember_engine::{InputState, KeyCode, MouseButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Action {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Sprint,
    Crouch,
    Fire,
    Ads,
    Reload,
    Melee,
    Shield,
    Scoreboard,
    Slot1,
    Slot2,
    Slot3,
    Slot4,
    Slot5,
    Slot6,
    Slot7,
    Slot8,
    Slot9,
}

impl Action {
    #[cfg(any(target_arch = "wasm32", test))]
    const ALL: [Self; 22] = [
        Self::Forward,
        Self::Backward,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Sprint,
        Self::Crouch,
        Self::Fire,
        Self::Ads,
        Self::Reload,
        Self::Melee,
        Self::Shield,
        Self::Scoreboard,
        Self::Slot1,
        Self::Slot2,
        Self::Slot3,
        Self::Slot4,
        Self::Slot5,
        Self::Slot6,
        Self::Slot7,
        Self::Slot8,
        Self::Slot9,
    ];

    #[cfg(any(target_arch = "wasm32", test))]
    const fn name(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::Left => "left",
            Self::Right => "right",
            Self::Jump => "jump",
            Self::Sprint => "sprint",
            Self::Crouch => "crouch",
            Self::Fire => "fire",
            Self::Ads => "ads",
            Self::Reload => "reload",
            Self::Melee => "melee",
            Self::Shield => "shield",
            Self::Scoreboard => "scoreboard",
            Self::Slot1 => "slot1",
            Self::Slot2 => "slot2",
            Self::Slot3 => "slot3",
            Self::Slot4 => "slot4",
            Self::Slot5 => "slot5",
            Self::Slot6 => "slot6",
            Self::Slot7 => "slot7",
            Self::Slot8 => "slot8",
            Self::Slot9 => "slot9",
        }
    }

    pub fn down(self, settings: &Settings, input: &InputState) -> bool {
        !settings.paused
            && settings.bindings[self as usize]
                .iter()
                .flatten()
                .any(|binding| binding.down(input))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
}

impl Binding {
    fn down(self, input: &InputState) -> bool {
        match self {
            Self::Key(key) => input.down(key),
            Self::Mouse(button) => input.mouse_down(button),
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn parse(code: &str) -> Option<Self> {
        let key = match code {
            "KeyA" => KeyCode::KeyA,
            "KeyB" => KeyCode::KeyB,
            "KeyC" => KeyCode::KeyC,
            "KeyD" => KeyCode::KeyD,
            "KeyE" => KeyCode::KeyE,
            "KeyF" => KeyCode::KeyF,
            "KeyG" => KeyCode::KeyG,
            "KeyH" => KeyCode::KeyH,
            "KeyI" => KeyCode::KeyI,
            "KeyJ" => KeyCode::KeyJ,
            "KeyK" => KeyCode::KeyK,
            "KeyL" => KeyCode::KeyL,
            "KeyM" => KeyCode::KeyM,
            "KeyN" => KeyCode::KeyN,
            "KeyO" => KeyCode::KeyO,
            "KeyP" => KeyCode::KeyP,
            "KeyQ" => KeyCode::KeyQ,
            "KeyR" => KeyCode::KeyR,
            "KeyS" => KeyCode::KeyS,
            "KeyT" => KeyCode::KeyT,
            "KeyU" => KeyCode::KeyU,
            "KeyV" => KeyCode::KeyV,
            "KeyW" => KeyCode::KeyW,
            "KeyX" => KeyCode::KeyX,
            "KeyY" => KeyCode::KeyY,
            "KeyZ" => KeyCode::KeyZ,
            "Digit0" => KeyCode::Digit0,
            "Digit1" => KeyCode::Digit1,
            "Digit2" => KeyCode::Digit2,
            "Digit3" => KeyCode::Digit3,
            "Digit4" => KeyCode::Digit4,
            "Digit5" => KeyCode::Digit5,
            "Digit6" => KeyCode::Digit6,
            "Digit7" => KeyCode::Digit7,
            "Digit8" => KeyCode::Digit8,
            "Digit9" => KeyCode::Digit9,
            "Space" => KeyCode::Space,
            "Tab" => KeyCode::Tab,
            "ShiftLeft" => KeyCode::ShiftLeft,
            "ShiftRight" => KeyCode::ShiftRight,
            "ArrowUp" => KeyCode::ArrowUp,
            "ArrowDown" => KeyCode::ArrowDown,
            "ArrowLeft" => KeyCode::ArrowLeft,
            "ArrowRight" => KeyCode::ArrowRight,
            // Browser button numbering differs from winit's enum ordering.
            "Mouse0" => return Some(Self::Mouse(MouseButton::Left)),
            "Mouse1" => return Some(Self::Mouse(MouseButton::Middle)),
            "Mouse2" => return Some(Self::Mouse(MouseButton::Right)),
            _ => return None,
        };
        Some(Self::Key(key))
    }
}

type Bindings = [Option<Binding>; 2];

const fn key(code: KeyCode) -> Bindings {
    [Some(Binding::Key(code)), None]
}

const fn default_bindings() -> [Bindings; 22] {
    [
        key(KeyCode::KeyW),
        key(KeyCode::KeyS),
        key(KeyCode::KeyA),
        key(KeyCode::KeyD),
        key(KeyCode::Space),
        [
            Some(Binding::Key(KeyCode::ShiftLeft)),
            Some(Binding::Key(KeyCode::ShiftRight)),
        ],
        key(KeyCode::KeyC),
        [Some(Binding::Mouse(MouseButton::Left)), None],
        [Some(Binding::Mouse(MouseButton::Right)), None],
        key(KeyCode::KeyR),
        key(KeyCode::KeyE),
        key(KeyCode::KeyQ),
        key(KeyCode::Tab),
        key(KeyCode::Digit1),
        key(KeyCode::Digit2),
        key(KeyCode::Digit3),
        key(KeyCode::Digit4),
        key(KeyCode::Digit5),
        key(KeyCode::Digit6),
        key(KeyCode::Digit7),
        key(KeyCode::Digit8),
        key(KeyCode::Digit9),
    ]
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub sensitivity: f32,
    pub paused: bool,
    bindings: [Bindings; 22],
    #[cfg(target_arch = "wasm32")]
    revision: Option<f64>,
    #[cfg(target_arch = "wasm32")]
    browser: Option<BrowserSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 1.0,
            paused: false,
            bindings: default_bindings(),
            #[cfg(target_arch = "wasm32")]
            revision: None,
            #[cfg(target_arch = "wasm32")]
            browser: None,
        }
    }
}

impl Settings {
    /// A slot request, not a weapon ID. Simultaneous controls choose the lowest
    /// slot deterministically; the caller owns edge detection and inventory.
    pub fn selected_slot(&self, input: &InputState) -> u8 {
        [
            (Action::Slot1, 1),
            (Action::Slot2, 2),
            (Action::Slot3, 3),
            (Action::Slot4, 4),
            (Action::Slot5, 5),
            (Action::Slot6, 6),
            (Action::Slot7, 7),
            (Action::Slot8, 8),
            (Action::Slot9, 9),
        ]
        .into_iter()
        .find_map(|(action, slot)| action.down(self, input).then_some(slot))
        .unwrap_or(0)
    }

    pub fn axis(&self, negative: Action, positive: Action, input: &InputState) -> f32 {
        match (negative.down(self, input), positive.down(self, input)) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    }

    /// Native intentionally keeps defaults; its settings are not a browser UI.
    // Browser refresh mutates through JS calls; native deliberately keeps the
    // same interface as a no-op for default controls and headless tests.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        allow(
            clippy::unused_self,
            clippy::missing_const_for_fn,
            clippy::needless_pass_by_ref_mut
        )
    )]
    pub fn refresh(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            // Pausing remains fail-safe even if a menu forgets to bump revision.
            // These two scalar reads allocate no JSON or Rust strings.
            if self.browser.is_none() {
                self.browser = BrowserSettings::new();
            }
            let Some(browser) = &self.browser else { return };
            let config = js_sys::Reflect::get(&browser.window, &browser.settings_key)
                .unwrap_or(wasm_bindgen::JsValue::UNDEFINED);
            if !config.is_object() {
                // Frozen pages without the menu must not throw Reflect errors
                // (and allocate JS exceptions) on every animation frame.
                self.paused = false;
                return;
            }
            self.paused = js_sys::Reflect::get(&config, &browser.paused_key)
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let Some(revision) = js_sys::Reflect::get(&config, &browser.revision_key)
                .ok()
                .and_then(|value| value.as_f64())
                .filter(|value| valid_revision(*value))
            else {
                return;
            };
            if self.revision == Some(revision) {
                return;
            }
            if let Some(json) = js_sys::JSON::stringify(&config)
                .ok()
                .and_then(|value| value.as_string())
            {
                let next = Self::from_json(&json);
                self.sensitivity = next.sensitivity;
                self.bindings = next.bindings;
            }
            self.revision = Some(revision);
        }
    }

    /// Bad individual values fall back to defaults. A duplicate final table is
    /// rejected as a whole so one physical press never silently runs two actions.
    #[cfg(any(target_arch = "wasm32", test))]
    pub fn from_json(json: &str) -> Self {
        let mut settings = Self::default();
        let Ok(serde_json::Value::Object(config)) = serde_json::from_str(json) else {
            return settings;
        };
        settings.paused = config
            .get("paused")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if let Some(value) = config
            .get("sensitivity")
            .and_then(serde_json::Value::as_f64)
            && value.is_finite()
        {
            // Clamp before narrowing, so a huge finite JSON number stays safe.
            #[allow(clippy::cast_possible_truncation)]
            let sensitivity = value.clamp(0.1, 3.0) as f32;
            settings.sensitivity = sensitivity;
        }
        if let Some(serde_json::Value::Object(bindings)) = config.get("bindings") {
            for action in Action::ALL {
                if let Some(binding) = bindings.get(action.name()).and_then(parse_bindings) {
                    settings.bindings[action as usize] = binding;
                }
            }
            if has_duplicates(&settings.bindings) {
                settings.bindings = default_bindings();
            }
        }
        settings
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn parse_bindings(value: &serde_json::Value) -> Option<Bindings> {
    let values = value.as_array()?;
    if !(1..=2).contains(&values.len()) {
        return None;
    }
    let mut bindings = [None; 2];
    for (index, value) in values.iter().enumerate() {
        bindings[index] = Some(Binding::parse(value.as_str()?)?);
    }
    if bindings[0] == bindings[1] {
        return None;
    }
    Some(bindings)
}

#[cfg(any(target_arch = "wasm32", test))]
fn has_duplicates(table: &[Bindings; 22]) -> bool {
    for (index, binding) in table.iter().flatten().flatten().enumerate() {
        if table
            .iter()
            .flatten()
            .flatten()
            .skip(index + 1)
            .any(|other| other == binding)
        {
            return true;
        }
    }
    false
}

#[cfg(any(target_arch = "wasm32", test))]
fn valid_revision(value: f64) -> bool {
    value.is_finite() && (0.0..=9_007_199_254_740_991.0).contains(&value) && value.fract() == 0.0
}

// Cached property keys avoid re-encoding strings on every frame. Reflect keeps
// the deployment to its existing two files; inline_js needs extra snippet files.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
struct BrowserSettings {
    window: web_sys::Window,
    settings_key: wasm_bindgen::JsValue,
    paused_key: wasm_bindgen::JsValue,
    revision_key: wasm_bindgen::JsValue,
}

#[cfg(target_arch = "wasm32")]
impl BrowserSettings {
    fn new() -> Option<Self> {
        Some(Self {
            window: web_sys::window()?,
            settings_key: "__emberArenaSettings".into(),
            paused_key: "paused".into(),
            revision_key: "revision".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_current_controls_and_both_shifts() {
        let settings = Settings::default();
        let keys = [
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::Space,
            KeyCode::KeyC,
            KeyCode::KeyR,
            KeyCode::KeyE,
            KeyCode::KeyQ,
            KeyCode::Tab,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ];
        let input = InputState::from_parts(
            &keys,
            &[MouseButton::Left, MouseButton::Right],
            (0.0, 0.0),
            None,
        );
        for action in Action::ALL {
            assert_eq!(
                action.down(&settings, &input),
                action != Action::Sprint,
                "{action:?}"
            );
        }
        for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            assert!(Action::Sprint.down(
                &settings,
                &InputState::from_parts(&[shift], &[], (0.0, 0.0), None)
            ));
        }
        assert_eq!(
            settings.axis(Action::Backward, Action::Forward, &input),
            0.0
        );
        assert!(!has_duplicates(&settings.bindings));
    }

    #[test]
    fn remaps_swap_keys_and_map_browser_mouse_numbers_exactly() {
        let settings = Settings::from_json(
            r#"{"bindings":{"forward":["KeyS"],"backward":["KeyW"],"fire":["Mouse1"]}}"#,
        );
        let input =
            InputState::from_parts(&[KeyCode::KeyS], &[MouseButton::Middle], (0.0, 0.0), None);
        assert_eq!(
            settings.axis(Action::Backward, Action::Forward, &input),
            1.0
        );
        assert!(Action::Fire.down(&settings, &input));
        assert!(!Action::Ads.down(&settings, &input));
        assert_eq!(
            Binding::parse("Mouse0"),
            Some(Binding::Mouse(MouseButton::Left))
        );
        assert_eq!(
            Binding::parse("Mouse1"),
            Some(Binding::Mouse(MouseButton::Middle))
        );
        assert_eq!(
            Binding::parse("Mouse2"),
            Some(Binding::Mouse(MouseButton::Right))
        );
    }

    #[test]
    fn invalid_configs_and_conflicting_actions_cannot_replace_safe_defaults() {
        for json in [
            "null",
            "[]",
            "not json",
            r#"{"bindings":{"fire":["Mouse2"]}}"#,
            r#"{"bindings":{"jump":["Escape"]}}"#,
            r#"{"bindings":{"jump":["Space","Space"]}}"#,
            r#"{"bindings":{"jump":[]}}"#,
            r#"{"bindings":{"jump":["KeyJ","KeyK","KeyL"]}}"#,
        ] {
            assert_eq!(
                Settings::from_json(json).bindings,
                default_bindings(),
                "{json}"
            );
        }
        for code in [
            "Escape",
            "F1",
            "F11",
            "MetaLeft",
            "ControlLeft",
            "AltLeft",
            "Mouse3",
            "w",
        ] {
            assert_eq!(Binding::parse(code), None, "{code}");
        }
        assert!(Binding::parse("KeyF").is_some());
    }

    #[test]
    fn sensitivity_is_finite_and_bounded_and_pause_neutralizes_all_actions() {
        for (json, expected) in [
            (r#"{"sensitivity":-50}"#, 0.1),
            (r#"{"sensitivity":1e100}"#, 3.0),
            (r#"{"sensitivity":2.5}"#, 2.5),
            (r#"{"sensitivity":"NaN"}"#, 1.0),
            (r#"{"sensitivity":null}"#, 1.0),
        ] {
            assert_eq!(Settings::from_json(json).sensitivity, expected);
        }
        let paused = Settings::from_json(r#"{"paused":true}"#);
        let input = InputState::from_parts(
            &[KeyCode::KeyW, KeyCode::Space],
            &[MouseButton::Left],
            (70.0, 50.0),
            None,
        );
        for action in Action::ALL {
            assert!(!action.down(&paused, &input));
        }
        assert_eq!(paused.axis(Action::Backward, Action::Forward, &input), 0.0);
    }

    #[test]
    fn revision_requires_a_nonnegative_safe_js_integer() {
        for revision in [0.0, 1.0, 98765.0, 9_007_199_254_740_991.0] {
            assert!(valid_revision(revision));
        }
        for revision in [-1.0, 0.5, f64::NAN, f64::INFINITY, 9_007_199_254_740_992.0] {
            assert!(!valid_revision(revision));
        }
    }

    #[test]
    fn weapon_slots_have_distinct_defaults_respect_remaps_and_pause() {
        let settings = Settings::default();
        for (key, slot) in [
            (KeyCode::Digit1, 1),
            (KeyCode::Digit2, 2),
            (KeyCode::Digit3, 3),
            (KeyCode::Digit4, 4),
            (KeyCode::Digit5, 5),
            (KeyCode::Digit6, 6),
            (KeyCode::Digit7, 7),
            (KeyCode::Digit8, 8),
            (KeyCode::Digit9, 9),
        ] {
            let input = InputState::from_parts(&[key], &[], (0.0, 0.0), None);
            assert_eq!(settings.selected_slot(&input), slot);
            assert_eq!(
                Settings::from_json(r#"{"paused":true}"#).selected_slot(&input),
                0
            );
        }
        let remapped = Settings::from_json(r#"{"bindings":{"slot3":["KeyZ"]}}"#);
        assert_eq!(
            remapped.selected_slot(&InputState::from_parts(
                &[KeyCode::KeyZ],
                &[],
                (0.0, 0.0),
                None
            )),
            3
        );
        assert_eq!(
            remapped.selected_slot(&InputState::from_parts(
                &[KeyCode::Digit3],
                &[],
                (0.0, 0.0),
                None
            )),
            0
        );
        assert_eq!(
            settings.selected_slot(&InputState::from_parts(
                &[KeyCode::Digit9, KeyCode::Digit2],
                &[],
                (0.0, 0.0),
                None
            )),
            2
        );
        assert_eq!(
            Settings::from_json(r#"{"bindings":{"slot2":["Digit1"]}}"#).bindings,
            default_bindings()
        );
    }
}
