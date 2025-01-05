use serde::Deserialize;

use crate::kbd::KeyType;

use super::keymap::default_glyph;

#[derive(Debug, Deserialize)]
pub struct Layout {
    pub left: Vec<Layer>,
    pub right: Vec<Layer>,
}
impl Layout {
    pub fn height(&self) -> i32 {
        // TODO
        100
    }
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
    pub up: Option<KeyAction>,

    #[serde(default, rename = "e")]
    pub right: Option<KeyAction>,

    #[serde(default, rename = "w")]
    pub left: Option<KeyAction>,

    #[serde(default, rename = "s")]
    pub down: Option<KeyAction>,

    #[serde(default)]
    width: Option<u8>,

    #[serde(default)]
    label: Option<String>,
}
impl Default for KeyDef {
    fn default() -> Self {
        Self {
            key: evdev::Key::KEY_A,
            up: Some(KeyAction::Shift),
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
    pub fn glyph(&self) -> String {
        self.label
            .clone()
            .unwrap_or_else(|| default_glyph(&self.key))
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

#[derive(Debug, Clone, Deserialize)]
pub enum KeyAction {
    Key(evdev::Key),
    Ctrl,
    Shift,
    Super,

    // Drag cursor
    Arrow,
}
