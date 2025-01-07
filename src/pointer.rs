use anyhow::{Result, anyhow};
use mouse_keyboard_input::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, Button, VirtualDevice};
use serde::Deserialize;

const SCROLL_STEP: i32 = 10;

#[derive(Debug)]
pub enum PointerMessage {
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    Move(i32, i32),
    Press(PointerButton),
    Release(PointerButton),
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum PointerButton {
    #[serde(rename = "PointerLeft")]
    Left,

    #[serde(rename = "PointerMiddle")]
    Middle,

    #[serde(rename = "PointerRight")]
    Right,
}
impl Into<Button> for PointerButton {
    fn into(self) -> Button {
        match self {
            Self::Left => BTN_LEFT,
            Self::Middle => BTN_MIDDLE,
            Self::Right => BTN_RIGHT,
        }
    }
}

pub struct Pointer {
    device: VirtualDevice,
}
impl Pointer {
    pub fn new() -> Self {
        let device = VirtualDevice::default().unwrap();
        Self { device }
    }

    pub fn handle(&mut self, message: PointerMessage) -> Result<()> {
        match message {
            PointerMessage::ScrollUp => self.scroll_up(),
            PointerMessage::ScrollDown => self.scroll_down(),
            PointerMessage::ScrollLeft => self.scroll_left(),
            PointerMessage::ScrollRight => self.scroll_right(),
            PointerMessage::Move(x, y) => self.translate(x, y),
            PointerMessage::Press(btn) => self.press(btn),
            PointerMessage::Release(btn) => self.release(btn),
        }
    }

    fn scroll_up(&mut self) -> Result<()> {
        self.device
            .scroll_y(SCROLL_STEP)
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn scroll_down(&mut self) -> Result<()> {
        self.device
            .scroll_y(-SCROLL_STEP)
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn scroll_left(&mut self) -> Result<()> {
        self.device
            .scroll_x(-SCROLL_STEP)
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn scroll_right(&mut self) -> Result<()> {
        self.device
            .scroll_x(SCROLL_STEP)
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn translate(&mut self, x: i32, y: i32) -> Result<()> {
        self.device
            .smooth_move_mouse(x, y)
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn press(&mut self, button: PointerButton) -> Result<()> {
        self.device
            .press(button.into())
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn release(&mut self, button: PointerButton) -> Result<()> {
        self.device
            .release(button.into())
            .map_err(|err| anyhow!(err.to_string()))
    }
}
