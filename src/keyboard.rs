use tracing::debug;
use wayland_client::{Connection, EventQueue, protocol::wl_keyboard::KeyState};

use crate::{
    layout::{Layer, Layout, Side},
    session::SessionState,
};

pub enum KeyType {
    Mod,
    Lock,
    Normal,
}
impl From<evdev::Key> for KeyType {
    fn from(value: evdev::Key) -> Self {
        match value {
            evdev::Key::KEY_LEFTCTRL
            | evdev::Key::KEY_RIGHTCTRL
            | evdev::Key::KEY_LEFTMETA
            | evdev::Key::KEY_RIGHTMETA
            | evdev::Key::KEY_LEFTSHIFT
            | evdev::Key::KEY_RIGHTSHIFT
            | evdev::Key::KEY_LEFTALT
            | evdev::Key::KEY_RIGHTALT => KeyType::Mod,
            evdev::Key::KEY_CAPSLOCK | evdev::Key::KEY_NUMLOCK | evdev::Key::KEY_SCROLLLOCK => {
                KeyType::Lock
            }
            _ => KeyType::Normal,
        }
    }
}

#[derive(Debug)]
pub enum KeyMessage {
    ButtonPress(u16),
    ButtonRelease(u16),
    ModPress(u16),
    ModRelease(u16),
    LockPress(u16),
    LockRelease(u16),
    Layer(Side, usize),
    MouseLayer(bool),
}

pub struct Keyboard {
    session_state: SessionState,
    event_queue: EventQueue<SessionState>,
    layout: Layout,
    pub layer: (usize, usize),
    modifiers: u32,
    locks: u32,
}
impl Keyboard {
    pub fn new(layout: Layout) -> Self {
        let conn = Connection::connect_to_env().unwrap();
        let display = conn.display();

        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let _registry = display.get_registry(&qh, ());

        let mut state = SessionState::default();

        // 1. Bind seat and virtual keyboard/input method manager
        // 2. Create virtual keyboard/input method by seat and manager
        event_queue.roundtrip(&mut state).unwrap();
        event_queue.roundtrip(&mut state).unwrap();

        Self {
            session_state: state,
            event_queue,
            modifiers: 0,
            locks: 0,

            layout,
            layer: (0, 0),
        }
    }

    pub fn handle(&mut self, msg: KeyMessage) {
        match msg {
            KeyMessage::ButtonPress(scan_code) => {
                self.press_key(evdev::Key::new(scan_code));
            }
            KeyMessage::ButtonRelease(scan_code) => {
                self.release_key(evdev::Key::new(scan_code));
            }
            KeyMessage::ModPress(scan_code) => {
                self.append_mod(evdev::Key::new(scan_code));
            }
            KeyMessage::ModRelease(scan_code) => {
                self.remove_mod(evdev::Key::new(scan_code));
            }
            KeyMessage::LockPress(scan_code) => {
                self.append_lock(evdev::Key::new(scan_code));
            }
            KeyMessage::LockRelease(scan_code) => {
                self.remove_lock(evdev::Key::new(scan_code));
            }
            KeyMessage::MouseLayer(enable) => {
                // The mouse layer is added as the last layer.
                if enable {
                    self.layer.0 = self.layout.left.len() - 1;
                } else {
                    self.layer.0 = 0;
                }
            }
            KeyMessage::Layer(side, idx) => {
                debug!("  [Layer] Switched: {:?} -> {:?}", side, idx);
                match side {
                    Side::Left => self.layer.0 = idx,
                    Side::Right => self.layer.1 = idx,
                }
            }
        }
    }

    fn press_key(&mut self, key: evdev::Key) {
        if let Some(keyboard) = &self.session_state.keyboard {
            debug!("  [Key] Pressed: {:?}", key);
            keyboard.key(0, key.code().into(), KeyState::Pressed.into());
            self.event_queue.roundtrip(&mut self.session_state).unwrap();
        }
    }

    fn release_key(&mut self, key: evdev::Key) {
        if let Some(keyboard) = &self.session_state.keyboard {
            debug!("  [Key] Released: {:?}", key);
            keyboard.key(0, key.code().into(), KeyState::Released.into());
            self.event_queue.roundtrip(&mut self.session_state).unwrap();
        }
    }

    fn append_mod(&mut self, key: evdev::Key) {
        debug!("  [Mod] Appended: {:?}", key);
        let mod_code = Self::map_mod_key(key);
        self.modifiers |= mod_code;

        self.update_state();
    }

    fn remove_mod(&mut self, key: evdev::Key) {
        debug!("  [Mod] Removed: {:?}", key);
        let mod_code = Self::map_mod_key(key);
        self.modifiers &= !mod_code;

        self.update_state();
    }

    fn append_lock(&mut self, key: evdev::Key) {
        debug!("  [Lock] Appended: {:?}", key);
        let lock_code = Self::map_lock_key(key);
        self.locks |= lock_code;

        self.update_state();
    }

    fn remove_lock(&mut self, key: evdev::Key) {
        debug!("  [Lock] Removed: {:?}", key);
        let lock_code = Self::map_lock_key(key);
        self.locks &= !lock_code;

        self.update_state();
    }

    pub fn destroy(&mut self) {
        if let Some(keyboard) = &self.session_state.keyboard {
            keyboard.destroy();
            self.event_queue.roundtrip(&mut self.session_state).unwrap();
        }
    }

    fn update_state(&mut self) {
        if let Some(keyboard) = &self.session_state.keyboard {
            keyboard.modifiers(self.modifiers, 0, self.locks, 0);
            self.event_queue.roundtrip(&mut self.session_state).unwrap();
        }
    }

    fn map_mod_key(key: evdev::Key) -> u32 {
        match key {
            evdev::Key::KEY_LEFTCTRL | evdev::Key::KEY_RIGHTCTRL => 4,
            evdev::Key::KEY_LEFTMETA | evdev::Key::KEY_RIGHTMETA => 4,
            evdev::Key::KEY_LEFTSHIFT | evdev::Key::KEY_RIGHTSHIFT => 1,
            evdev::Key::KEY_LEFTALT | evdev::Key::KEY_RIGHTALT => 8,
            _ => 0,
        }
    }

    fn map_lock_key(key: evdev::Key) -> u32 {
        match key {
            evdev::Key::KEY_CAPSLOCK => 2,
            evdev::Key::KEY_NUMLOCK => 256,
            evdev::Key::KEY_SCROLLLOCK => 32768,
            _ => 0,
        }
    }

    pub fn left_layers(&self) -> impl Iterator<Item = &Layer> {
        self.layout.left.iter()
    }

    pub fn right_layers(&self) -> impl Iterator<Item = &Layer> {
        self.layout.right.iter()
    }
}
