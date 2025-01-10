use std::path::Path;

use serde::{Deserialize, Deserializer};

use crate::pointer::PointerButton;

/// A `Layout` has two [`Side`]s,
/// each of which consists of one or more [`Layer`]s.
#[derive(Debug, Deserialize)]
pub struct Layout {
    pub left: Vec<Layer>,
    pub right: Vec<Layer>,
}

impl Layout {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let layout: String = fs_err::read_to_string(path).expect("Layout file not found");
        Self::from_str(&layout)
    }

    fn from_str(s: &str) -> Self {
        let mut layout: Layout = serde_yaml::from_str(s).expect("Layout is invalid");
        let mouse_layer = include_str!("../assets/mouse-layer.yml");
        let mouse_layer: Layer = serde_yaml::from_str(mouse_layer).expect("Mouse layer is invalid");
        layout.left.push(mouse_layer);
        layout
    }
}
impl Default for Layout {
    fn default() -> Self {
        let default = include_str!("../assets/layout.yml");
        Self::from_str(default)
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

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Modifier {
    Alt,
    Ctrl,
    Shift,
    Meta,
}
impl Modifier {
    pub fn code(&self) -> u16 {
        match self {
            Self::Alt => 56,
            Self::Ctrl => 29,
            Self::Shift => 42,
            Self::Meta => 125,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Command {
    pub cmd: String,

    #[serde(default)]
    pub args: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum KeyDef {
    /// A basic key, with optional swipe actions.
    Basic(BasicKey),

    /// Execute an arbitrary command.
    Command(Command),

    /// Send a mouse/pointer button.
    PointerButton(PointerButton),

    /// Control the mouse/pointer.
    #[serde(deserialize_with = "pointer")]
    Pointer,
}

// Hack to deserialize an untagged unit variant by name.
// <https://github.com/serde-rs/serde/issues/1158#issuecomment-365362959>
fn pointer<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    enum Helper {
        #[serde(rename = "Pointer")]
        Pointer,
    }
    Helper::deserialize(deserializer)?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct BasicKey {
    pub key: evdev::Key,

    #[serde(default, rename = "mods")]
    pub modifiers: Vec<Modifier>,

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
    pub label: Option<String>,
}
impl Default for BasicKey {
    fn default() -> Self {
        Self {
            key: evdev::Key::KEY_A,
            modifiers: Vec::default(),
            up: None,
            right: None,
            left: None,
            down: None,
            width: None,
            label: None,
        }
    }
}
impl BasicKey {
    pub fn width(&self) -> f32 {
        self.width.unwrap_or(1) as f32
    }
}

impl From<evdev::Key> for KeyDef {
    fn from(value: evdev::Key) -> Self {
        KeyDef::Basic(BasicKey {
            key: value,
            ..Default::default()
        })
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

    /// Fire a modified key press.
    ModKey(evdev::Key, Vec<Modifier>),

    /// Switch layer
    Layer(Side, usize),

    /// Fire the pressed key with a modifier.
    Modified(Modifier),

    /// Drag cursor in the swipe direction.
    Arrow,

    /// Mouse scroll in the swipe direction.
    Scroll,

    /// Select text in the swipe direction.
    /// This is only meaningful for left/right swipes.
    Select,

    /// Delete text in the swipe direction.
    /// This is only meaningful for left/right swipes.
    Delete,

    /// Run a system command.
    Command(Command),

    /// Hide the keyboard.
    HideKeyboard,
}
