use serde::Deserialize;

use crate::kbd::KeyType;

use super::glyphs::default_glyph;

/// A `Layout` has two [`Side`]s,
/// each of which consists of one or more [`Layer`]s.
#[derive(Debug, Deserialize)]
pub struct Layout {
    pub left: Vec<Layer>,
    pub right: Vec<Layer>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum Side {
    Left,
    Right,
}

impl Default for Layout {
    fn default() -> Self {
        let default = include_str!("../../assets/layout.yml");
        serde_yaml::from_str(default).expect("Default layout is valid")
    }
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Layer {
    layout: Vec<Vec<KeyDef>>,
}
impl Layer {
    pub fn rows(&self) -> impl Iterator<Item = &Vec<KeyDef>> {
        self.layout.iter()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyDef {
    pub key: evdev::Key,

    #[serde(default, rename = "n")]
    pub up: Option<SwipeAction>,

    #[serde(default, rename = "e")]
    pub right: Option<SwipeAction>,

    #[serde(default, rename = "w")]
    pub left: Option<SwipeAction>,

    #[serde(default, rename = "s")]
    pub down: Option<SwipeAction>,

    #[serde(default)]
    width: Option<u8>,

    #[serde(default)]
    label: Option<String>,
}
impl Default for KeyDef {
    fn default() -> Self {
        Self {
            key: evdev::Key::KEY_A,
            up: None,
            right: None,
            left: None,
            down: None,
            width: None,
            label: None,
        }
    }
}

impl From<evdev::Key> for KeyDef {
    fn from(value: evdev::Key) -> Self {
        KeyDef {
            key: value,
            ..Default::default()
        }
    }
}
impl KeyDef {
    /// A string representing this key.
    pub fn glyph(&self) -> String {
        self.label
            .clone()
            .unwrap_or_else(|| default_glyph(&self.key).to_string())
    }

    pub fn width(&self) -> f32 {
        self.width.unwrap_or(1) as f32
    }

    pub fn key_type(&self) -> KeyType {
        if self.is_mod_key() {
            KeyType::Mod
        } else if self.is_lock_key() {
            KeyType::Lock
        } else {
            KeyType::Normal
        }
    }

    fn is_mod_key(&self) -> bool {
        matches!(
            self.key,
            evdev::Key::KEY_LEFTCTRL
                | evdev::Key::KEY_RIGHTCTRL
                | evdev::Key::KEY_LEFTMETA
                | evdev::Key::KEY_RIGHTMETA
                | evdev::Key::KEY_LEFTSHIFT
                | evdev::Key::KEY_RIGHTSHIFT
                | evdev::Key::KEY_LEFTALT
                | evdev::Key::KEY_RIGHTALT
        )
    }

    fn is_lock_key(&self) -> bool {
        matches!(
            self.key,
            evdev::Key::KEY_CAPSLOCK | evdev::Key::KEY_NUMLOCK | evdev::Key::KEY_SCROLLLOCK
        )
    }
}

/// Swipe actions are triggered by directional swipes on keys.
///
/// There are mode of swipe actions:
/// - Fire: a one-shot; immediate press-and-release
/// - Hold: swipe up and hold to fire press, release to fire release.
/// - Drag: swipe and hold, and each incremental move fires press-and-release.
///
/// Each action has an assumed mode:
/// - Layer -> Hold
/// - Arrow -> Drag
/// - Scroll -> Drag
/// - Everything else -> Fire
#[derive(Debug, Clone, Deserialize)]
pub enum SwipeAction {
    /// Fire a regular key press.
    Key(evdev::Key),

    /// Switch layer
    Layer(Side, usize),

    /// Fire the pressed key with Alt.
    Alt,

    /// Fire the pressed key with Ctrl.
    Ctrl,

    /// Fire the pressed key with Shift.
    Shift,

    /// Fire the pressed key with Meta/Super.
    Meta,

    /// Drag cursor in the swipe direction.
    Arrow,

    /// Mouse scroll in the swipe direction.
    /// This is only meaningful for up/down swipes.
    /// Left/right swipes will instead send left/right arrows.
    Scroll,

    /// Select text in the swipe direction.
    /// This is only meaningful for left/right swipes.
    Select,

    /// Delete text in the swipe direction.
    /// This is only meaningful for left/right swipes.
    Delete,

    /// Hide the keyboard.
    HideKeyboard,
}
