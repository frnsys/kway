use std::{ffi::CString, fs::File, io::Write, path::PathBuf};

use xkbcommon::xkb;

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

/// Get a default glyph/character for a key.
pub fn default_glyph(key: &evdev::Key) -> String {
    let keymap = default_keymap();

    // Always an offset of 8;
    // see <https://xkbcommon.org/doc/current/keymap-text-format-v1.html>
    let scan_code = key.code() + 8;
    let code = xkb::Keycode::from(scan_code);
    keymap.key_get_utf8(code)
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
