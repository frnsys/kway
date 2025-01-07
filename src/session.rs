use std::{ffi::CString, fs::File, io::Write, os::fd::AsFd, path::PathBuf};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
    protocol::{
        wl_keyboard::{self},
        wl_registry,
        wl_seat::{self, WlSeat},
    },
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{self, ZwpInputMethodV2},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

use xkbcommon::xkb;

#[derive(Debug, Default)]
pub struct SessionState {
    pub keyboard_manager: Option<ZwpVirtualKeyboardManagerV1>,
    pub keyboard: Option<ZwpVirtualKeyboardV1>,
    pub input_manager: Option<ZwpInputMethodManagerV2>,
    pub input: Option<ZwpInputMethodV2>,
    pub input_serial: u32,
    pub seat: Option<WlSeat>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for SessionState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<SessionState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "zwp_virtual_keyboard_manager_v1" => {
                    let keyboard =
                        registry.bind::<ZwpVirtualKeyboardManagerV1, _, _>(name, version, &qh, ());
                    state.keyboard_manager = Some(keyboard);
                }
                "zwp_input_method_manager_v2" => {
                    let input =
                        registry.bind::<ZwpInputMethodManagerV2, _, _>(name, version, &qh, ());
                    state.input_manager = Some(input);
                }
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ());
                    state.seat = Some(seat);
                }
                _ => {}
            }
        }
    }
}

macro_rules! impl_dispatch {
    ($ty:ty) => {
        impl Dispatch<$ty, ()> for SessionState {
            fn event(
                _state: &mut Self,
                _proxy: &$ty,
                _event: <$ty as Proxy>::Event,
                _data: &(),
                _conn: &Connection,
                _qhandle: &QueueHandle<Self>,
            ) {
            }
        }
    };
}
impl_dispatch!(ZwpVirtualKeyboardManagerV1);
impl_dispatch!(ZwpVirtualKeyboardV1);
impl_dispatch!(ZwpInputMethodManagerV2);

impl Dispatch<WlSeat, ()> for SessionState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<SessionState>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                if let Some(keyboard_manager) = &state.keyboard_manager {
                    let keyboard = keyboard_manager.create_virtual_keyboard(seat, qh, ());

                    let (file, len) = get_keymap_as_file();
                    keyboard.keymap(wl_keyboard::KeymapFormat::XkbV1.into(), file.as_fd(), len);
                    state.keyboard = Some(keyboard);
                }

                if let Some(input_manager) = &state.input_manager {
                    let input = input_manager.get_input_method(seat, qh, ());
                    state.input = Some(input);
                }
            }
        }
    }
}

// TODO: Seems to be inconsistent; browsers I tried
// only sometimes send the activate event.
impl Dispatch<ZwpInputMethodV2, ()> for SessionState {
    fn event(
        state: &mut Self,
        _: &ZwpInputMethodV2,
        event: <ZwpInputMethodV2 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<SessionState>,
    ) {
        println!("RECEIVED INPUT EVENT: {:?}", event);
        match event {
            zwp_input_method_v2::Event::Activate => {
                println!("ACTIVATE");
                state.input_serial = 0;
            }
            zwp_input_method_v2::Event::Done => {
                state.input_serial = state.input_serial.wrapping_add(1);
            }
            _ => {}
        }
    }
}

// NOTE: This assumes US layout.
fn default_keymap() -> xkb::State {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = xkb::Keymap::new_from_names(
        &context,
        "",
        "",
        "us",
        "",
        None,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .expect("xkbcommon keymap panicked!");
    xkb::State::new(&keymap)
}

pub fn get_keymap_as_file() -> (File, u32) {
    let xkb_state = default_keymap();
    let keymap = xkb_state
        .get_keymap()
        .get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
    let keymap = CString::new(keymap).expect("Keymap should not contain interior nul bytes");
    let keymap = keymap.as_bytes_with_nul();
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let mut file = tempfile::tempfile_in(dir).expect("File could not be created!");
    file.write_all(keymap).unwrap();
    file.flush().unwrap();
    (file, keymap.len() as u32)
}
