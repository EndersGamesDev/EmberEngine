//! Physical keyboard controls shared by practice and online play.
//!
//! The page saves the flat JSON map and renders labels from `options_json`.
//! Rust reads the engine's physical keys directly; the page must not translate
//! those key presses into duplicate gameplay commands. Only `shop` is handled
//! by the page. Escape and platform shortcuts are deliberately not bindable.

use std::sync::Mutex;

use ember_engine::KeyCode;
use serde::Deserialize;

const ACTIONS: [&str; 15] = [
    "q",
    "w",
    "e",
    "r",
    "d",
    "f",
    "item1",
    "item2",
    "item3",
    "item4",
    "item5",
    "item6",
    "stop",
    "shop",
    "attackMove",
];

struct KeyOption {
    code: &'static str,
    label: &'static str,
    key: KeyCode,
}

macro_rules! key {
    ($key:ident, $label:literal) => {
        KeyOption {
            code: stringify!($key),
            label: $label,
            key: KeyCode::$key,
        }
    };
}

// Every entry is supported by winit's native and KeyboardEvent.code mappings.
// Modifier keys are reserved for ranking abilities; navigation keys with page
// activation semantics (Enter/Tab) and function keys remain platform controls.
const OPTIONS: &[KeyOption] = &[
    key!(KeyA, "A"),
    key!(KeyB, "B"),
    key!(KeyC, "C"),
    key!(KeyD, "D"),
    key!(KeyE, "E"),
    key!(KeyF, "F"),
    key!(KeyG, "G"),
    key!(KeyH, "H"),
    key!(KeyI, "I"),
    key!(KeyJ, "J"),
    key!(KeyK, "K"),
    key!(KeyL, "L"),
    key!(KeyM, "M"),
    key!(KeyN, "N"),
    key!(KeyO, "O"),
    key!(KeyP, "P"),
    key!(KeyQ, "Q"),
    key!(KeyR, "R"),
    key!(KeyS, "S"),
    key!(KeyT, "T"),
    key!(KeyU, "U"),
    key!(KeyV, "V"),
    key!(KeyW, "W"),
    key!(KeyX, "X"),
    key!(KeyY, "Y"),
    key!(KeyZ, "Z"),
    key!(Digit0, "0"),
    key!(Digit1, "1"),
    key!(Digit2, "2"),
    key!(Digit3, "3"),
    key!(Digit4, "4"),
    key!(Digit5, "5"),
    key!(Digit6, "6"),
    key!(Digit7, "7"),
    key!(Digit8, "8"),
    key!(Digit9, "9"),
    key!(Numpad0, "Num 0"),
    key!(Numpad1, "Num 1"),
    key!(Numpad2, "Num 2"),
    key!(Numpad3, "Num 3"),
    key!(Numpad4, "Num 4"),
    key!(Numpad5, "Num 5"),
    key!(Numpad6, "Num 6"),
    key!(Numpad7, "Num 7"),
    key!(Numpad8, "Num 8"),
    key!(Numpad9, "Num 9"),
    key!(NumpadAdd, "Num +"),
    key!(NumpadSubtract, "Num -"),
    key!(NumpadMultiply, "Num *"),
    key!(NumpadDivide, "Num /"),
    key!(NumpadDecimal, "Num ."),
    key!(NumpadEqual, "Num ="),
    key!(Backquote, "`"),
    key!(Minus, "-"),
    key!(Equal, "="),
    key!(BracketLeft, "["),
    key!(BracketRight, "]"),
    key!(Backslash, "\\"),
    key!(Semicolon, ";"),
    key!(Quote, "'"),
    key!(Comma, ","),
    key!(Period, "."),
    key!(Slash, "/"),
    key!(IntlBackslash, "Intl \\"),
    key!(ArrowLeft, "Left"),
    key!(ArrowRight, "Right"),
    key!(ArrowUp, "Up"),
    key!(ArrowDown, "Down"),
    key!(Space, "Space"),
    key!(Home, "Home"),
    key!(End, "End"),
    key!(PageUp, "Page Up"),
    key!(PageDown, "Page Down"),
    key!(Insert, "Insert"),
    key!(Delete, "Delete"),
];

/// Missing actions retain defaults, so `{}` is the reset operation.
/// A derived struct also rejects duplicate JSON action names.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    #[serde(default, deserialize_with = "present_code")]
    q: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    w: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    e: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    r: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    d: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    f: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    item1: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    item2: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    item3: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    item4: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    item5: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    item6: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    stop: Option<String>,
    #[serde(default, deserialize_with = "present_code")]
    shop: Option<String>,
    #[serde(default, rename = "attackMove", deserialize_with = "present_code")]
    attack_move: Option<String>,
}

fn present_code<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Controls {
    pub keys: [KeyCode; 15],
    pub enabled: bool,
    pub revision: u64,
}

impl Controls {
    pub const DEFAULT: Self = Self {
        keys: [
            KeyCode::KeyQ,
            KeyCode::KeyW,
            KeyCode::KeyE,
            KeyCode::KeyR,
            KeyCode::KeyD,
            KeyCode::KeyF,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::KeyS,
            KeyCode::KeyB,
            KeyCode::KeyA,
        ],
        enabled: true,
        revision: 0,
    };

    pub(crate) fn set_json(&mut self, json: &str) -> Result<(), String> {
        if !json.trim_start().starts_with('{') {
            return Err("Bindings must be a flat JSON object of actions and key codes".into());
        }
        let p: Patch = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let values = [
            p.q,
            p.w,
            p.e,
            p.r,
            p.d,
            p.f,
            p.item1,
            p.item2,
            p.item3,
            p.item4,
            p.item5,
            p.item6,
            p.stop,
            p.shop,
            p.attack_move,
        ];
        let mut keys = Self::DEFAULT.keys;
        for (i, code) in values.iter().enumerate() {
            if let Some(code) = code {
                keys[i] = OPTIONS
                    .iter()
                    .find(|o| o.code == code)
                    .ok_or_else(|| format!("Unsupported key code {code:?} for {}", ACTIONS[i]))?
                    .key;
            }
        }
        for i in 0..keys.len() {
            if let Some(j) = keys[..i].iter().position(|key| *key == keys[i]) {
                return Err(format!(
                    "{} and {} use the same key ({})",
                    ACTIONS[j],
                    ACTIONS[i],
                    code_for(keys[i])
                ));
            }
        }
        // Validate the whole replacement before changing any live controls.
        if self.keys != keys {
            self.keys = keys;
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    pub const fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn json(&self) -> String {
        let map: serde_json::Map<_, _> = ACTIONS
            .iter()
            .zip(self.keys)
            .map(|(action, key)| ((*action).to_owned(), serde_json::Value::from(code_for(key))))
            .collect();
        serde_json::Value::Object(map).to_string()
    }

    pub const fn permit(&self) -> Permit {
        Permit {
            enabled: self.enabled,
            revision: self.revision,
        }
    }
}

fn code_for(key: KeyCode) -> &'static str {
    OPTIONS.iter().find(|o| o.key == key).map_or("", |o| o.code)
}

/// Commands queued before a pause or rebind must not replay afterward.
#[derive(Clone, Copy)]
pub(crate) struct Permit {
    enabled: bool,
    revision: u64,
}

impl Permit {
    pub const fn allows(self, controls: &Controls) -> bool {
        self.enabled && controls.enabled && self.revision == controls.revision
    }
}

static CONTROLS: Mutex<Controls> = Mutex::new(Controls::DEFAULT);

pub(crate) fn snapshot() -> Controls {
    *CONTROLS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Replace the physical bindings with a flat map merged over defaults.
///
/// # Errors
///
/// Rejects invalid JSON, unknown actions, unsupported codes, or keys shared
/// between actions. A rejected map leaves the current bindings unchanged.
pub fn set_json(json: &str) -> Result<(), String> {
    CONTROLS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .set_json(json)
}

/// The complete action-to-`KeyboardEvent.code` map, including the DOM shop key.
#[must_use]
pub fn json() -> String {
    snapshot().json()
}

/// Supported physical keys as an array of `{code, label}` objects.
#[must_use]
pub fn options_json() -> String {
    serde_json::Value::Array(
        OPTIONS
            .iter()
            .map(|o| serde_json::json!({ "code": o.code, "label": o.label }))
            .collect(),
    )
    .to_string()
}

/// Pause gameplay input while the page has a menu or text field focused.
///
/// Draft, start, and shop visibility messages remain available; gameplay
/// commands, keyboard casts, attacks, and movement are suppressed together.
pub fn set_input_enabled(enabled: bool) {
    CONTROLS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .set_enabled(enabled);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_options_round_trip_with_distinct_physical_keys() {
        let mut controls = Controls::DEFAULT;
        controls.set_json(&controls.json()).unwrap();
        assert_eq!(controls, Controls::DEFAULT);
        let values: serde_json::Value = serde_json::from_str(&controls.json()).unwrap();
        assert_eq!(values["q"], "KeyQ");
        assert_eq!(values["item6"], "Digit6");
        assert_eq!(values["stop"], "KeyS");
        assert_eq!(values["shop"], "KeyB");
        assert_eq!(values["attackMove"], "KeyA");
        assert_eq!(values.as_object().unwrap().len(), 15);
        for (i, option) in OPTIONS.iter().enumerate() {
            assert!(
                !OPTIONS[..i]
                    .iter()
                    .any(|o| o.code == option.code || o.key == option.key)
            );
        }
        let options: serde_json::Value = serde_json::from_str(&options_json()).unwrap();
        assert!(
            options
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["code"] == "Numpad1" && o["label"] == "Num 1")
        );
    }

    #[test]
    fn partial_maps_start_from_defaults_and_shop_can_move() {
        let mut controls = Controls::DEFAULT;
        controls
            .set_json(r#"{"q":"KeyB","shop":"KeyZ","attackMove":"KeyX"}"#)
            .unwrap();
        assert_eq!(controls.keys[0], KeyCode::KeyB);
        assert_eq!(controls.keys[13], KeyCode::KeyZ);
        assert_eq!(controls.keys[14], KeyCode::KeyX);
        controls.set_json(r#"{"q":"KeyW","w":"KeyQ"}"#).unwrap();
        assert_eq!(controls.keys[0], KeyCode::KeyW);
        assert_eq!(controls.keys[1], KeyCode::KeyQ);
        assert_eq!(controls.keys[13], KeyCode::KeyB);
        controls.set_json("{}").unwrap();
        assert_eq!(controls.keys, Controls::DEFAULT.keys);
    }

    #[test]
    fn invalid_maps_are_atomic_and_conflicts_include_shop() {
        let mut controls = Controls::DEFAULT;
        controls.set_json(r#"{"q":"KeyZ"}"#).unwrap();
        let before = controls;
        for bad in [
            r#"{"q":"KeyW"}"#,
            r#"{"q":"KeyB"}"#,
            r#"{"q":"KeyA"}"#,
            r#"{"attackMove":"KeyQ"}"#,
            r#"{"q":"KeyA","q":"KeyZ"}"#,
            r#"{"unknown":"KeyX"}"#,
            r#"{"q":"Escape"}"#,
            r#"{"q":"ControlLeft"}"#,
            r#"{"q":"F11"}"#,
            r#"{"q":"q"}"#,
            r#"{"q":12}"#,
            r#"{"q":null}"#,
            "[]",
            r#"["KeyQ","KeyW","KeyE","KeyR","KeyD","KeyF","Digit1","Digit2","Digit3","Digit4","Digit5","Digit6","KeyS","KeyB"]"#,
            "not json",
        ] {
            assert!(controls.set_json(bad).is_err(), "accepted {bad}");
            assert_eq!(controls, before, "changed bindings after {bad}");
        }
    }

    #[test]
    fn queued_gameplay_cannot_cross_pause_or_binding_changes() {
        let mut controls = Controls::DEFAULT;
        let active = controls.permit();
        assert!(active.allows(&controls));
        controls.set_enabled(false);
        let paused = controls.permit();
        assert!(!active.allows(&controls));
        assert!(!paused.allows(&controls));
        controls.set_enabled(true);
        assert!(!active.allows(&controls));
        assert!(!paused.allows(&controls));
        let resumed = controls.permit();
        assert!(resumed.allows(&controls));
        controls.set_json(r#"{"q":"KeyZ"}"#).unwrap();
        assert!(!resumed.allows(&controls));
    }
}
