mod keymap;
mod layout;
mod session;

use layout::Side;
pub use layout::{Layer, Layout, SwipeAction};
use session::SessionState;
use tracing::debug;
use wayland_client::{Connection, EventQueue, protocol::wl_keyboard::KeyState};

pub enum KeyType {
    Mod,
    Lock,
    Normal,
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
}

pub struct Keyboard {
    session_state: SessionState,
    event_queue: EventQueue<SessionState>,
    layout: Layout,
    layer: (usize, usize),
    modifiers: u32,
    locks: u32,
}
impl Keyboard {
    pub fn new() -> Self {
        let conn = Connection::connect_to_env().unwrap();
        let display = conn.display();

        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let _registry = display.get_registry(&qh, ());

        let mut state = SessionState {
            keyboard_manager: None,
            keyboard: None,
            seat: None,
        };

        //bind seat and virtual keyboard manager
        event_queue.roundtrip(&mut state).unwrap();
        //create virtual keyboard by seat and manager
        event_queue.roundtrip(&mut state).unwrap();

        Self {
            session_state: state,
            event_queue,
            modifiers: 0,
            locks: 0,

            layout: Layout::default(),
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
            KeyMessage::Layer(side, idx) => match side {
                Side::Left => self.layer.0 = idx,
                Side::Right => self.layer.1 = idx,
            },
        }
    }

    fn press_key(&mut self, key: evdev::Key) {
        if let Some(keyboard) = &self.session_state.keyboard {
            debug!("Key Pressed: {:?}", key);
            keyboard.key(0, key.code().into(), KeyState::Pressed.into());
            self.event_queue.roundtrip(&mut self.session_state).unwrap();
        }
    }

    fn release_key(&mut self, key: evdev::Key) {
        if let Some(keyboard) = &self.session_state.keyboard {
            debug!("Key Released: {:?}", key);
            keyboard.key(0, key.code().into(), KeyState::Released.into());
            self.event_queue.roundtrip(&mut self.session_state).unwrap();
        }
    }

    fn append_mod(&mut self, key: evdev::Key) {
        debug!("Mod Appended: {:?}", key);
        let mod_code = Self::map_mod_key(key);
        self.modifiers |= mod_code;

        self.update_state();
    }

    fn remove_mod(&mut self, key: evdev::Key) {
        debug!("Mod Removed: {:?}", key);
        let mod_code = Self::map_mod_key(key);
        self.modifiers &= !mod_code;

        self.update_state();
    }

    fn append_lock(&mut self, key: evdev::Key) {
        debug!("Lock Appended: {:?}", key);
        let lock_code = Self::map_lock_key(key);
        self.locks |= lock_code;

        self.update_state();
    }

    fn remove_lock(&mut self, key: evdev::Key) {
        debug!("Lock Removed: {:?}", key);
        let lock_code = Self::map_lock_key(key);
        self.locks &= !lock_code;

        self.update_state();
    }

    pub fn destroy(&mut self) {
        if let Some(keyboard) = &self.session_state.keyboard {
            debug!("Destroying Virtual Keyboard.");
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

    pub fn active_layers(&self) -> (&Layer, &Layer) {
        (
            &self.layout.left[self.layer.0],
            &self.layout.right[self.layer.1],
        )
    }
}
