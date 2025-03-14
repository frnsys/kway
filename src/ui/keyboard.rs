use std::sync::Arc;

use arc_swap::ArcSwapOption;
use gdk4::{
    glib::value::FromValue,
    prelude::{Cast, ObjectExt},
};
use relm4::{
    ComponentSender, RelmWidgetExt,
    gtk::{
        self,
        prelude::{BoxExt, GestureDragExt, ToggleButtonExt, WidgetExt},
    },
};
use tracing::debug;

use crate::{
    keyboard::{KeyMessage, KeyType},
    layout::{BasicKey, Command, KeyDef, Layer, Modifier, SwipeAction},
    pointer::PointerMessage,
};

use super::{UIMessage, UIModel, key::KeyButton, swipe::Direction};

const KEY_SPACING: i32 = 2;
const KEY_SIZE: i32 = 42;
const KB_PADDING: i32 = 24;

impl BasicKey {
    fn dir_action(&self, dir: Direction) -> &Option<SwipeAction> {
        match dir {
            Direction::Up => &self.up,
            Direction::Right => &self.right,
            Direction::Left => &self.left,
            Direction::Down => &self.down,
        }
    }
}

impl KeyDef {
    fn render(&self, size: i32, sender: &ComponentSender<UIModel>) -> gtk::Widget {
        match self {
            KeyDef::Basic(key) => key.render(size, sender),
            KeyDef::Command(Command { label, cmd, args }) => {
                let button = KeyButton::default();
                button.set_primary_content(label.as_str());
                button.set_width_request(size);
                button.set_height_request(size);

                let cmd = cmd.clone();
                let args = args.clone();
                let sender_cb = sender.clone();
                button.connect("released", true, move |_| {
                    sender_cb.input(UIMessage::Command(cmd.clone(), args.clone()));
                    None
                });

                button.upcast()
            }
            KeyDef::PointerButton(key) => {
                let key = *key;
                let button = KeyButton::default();
                button.set_primary_content(key.glyph());
                button.set_width_request(size);
                button.set_height_request(size);

                let sender_cb = sender.clone();
                button.connect("tap-pressed", true, move |_| {
                    sender_cb.input(PointerMessage::Press(key).into());
                    None
                });

                let sender_cb = sender.clone();
                button.connect("released", true, move |_| {
                    sender_cb.input(PointerMessage::Release(key).into());
                    None
                });

                button.upcast()
            }
            KeyDef::Pointer => {
                let glyph = "✱";
                let button = KeyButton::default();
                button.set_primary_content(glyph);
                button.set_width_request(size);
                button.set_height_request(size);

                // We scale the pointer movement exponentially
                // based on distance from the drag start, such that
                // closer movements are finer and larger movements
                // cover more ground.
                let base_scale = 2.;
                let offset_exp = 1. / 3.;
                let sender_cb = sender.clone();
                button.connect("freemove", true, move |args| {
                    let dx = args[1].get::<f64>().unwrap();
                    let dy = args[2].get::<f64>().unwrap();
                    let ox = args[3].get::<f64>().unwrap().abs().powf(offset_exp);
                    let oy = args[4].get::<f64>().unwrap().abs().powf(offset_exp);
                    let dx = (dx * ox * base_scale).round() as i32;
                    let dy = (dy * oy * base_scale).round() as i32;
                    sender_cb.input(PointerMessage::Move(dx, dy).into());
                    sender_cb.input(KeyMessage::MouseLayer(true).into());
                    sender_cb.input(UIMessage::UpdateLayout);
                    None
                });

                let sender_cb = sender.clone();
                button.connect("released", true, move |_| {
                    sender_cb.input(KeyMessage::MouseLayer(false).into());
                    sender_cb.input(UIMessage::UpdateLayout);
                    None
                });

                button.upcast()
            }
        }
    }
}

impl BasicKey {
    pub fn render(&self, size: i32, sender: &ComponentSender<UIModel>) -> gtk::Widget {
        let key = self.clone();
        let glyph = key.glyph();
        let scan_code = key.key.code();
        let width = (key.width() * f32::from(size as u16)).round() as i32;

        match KeyType::from(key.key) {
            KeyType::Mod => {
                let toggle = gtk::ToggleButton::builder()
                    .label(glyph)
                    .width_request(width)
                    .height_request(size)
                    .build();

                let button_sender = sender.clone();
                toggle.connect_toggled(move |btn| {
                    if btn.is_active() {
                        button_sender.input(KeyMessage::ModPress(scan_code).into());
                    } else {
                        button_sender.input(KeyMessage::ModRelease(scan_code).into());
                    }
                });

                toggle.upcast()
            }
            KeyType::Lock => {
                let toggle = gtk::ToggleButton::builder()
                    .label(glyph)
                    .width_request(width)
                    .height_request(size)
                    .build();

                let button_sender = sender.clone();
                toggle.connect_toggled(move |btn| {
                    if btn.is_active() {
                        button_sender.input(KeyMessage::LockPress(scan_code).into());
                    } else {
                        button_sender.input(KeyMessage::LockRelease(scan_code).into());
                    }
                });

                toggle.upcast()
            }
            KeyType::Normal => {
                let button = KeyButton::default();
                button.set_primary_content(glyph);
                button.set_width_request(width);
                button.set_height_request(size);

                let sender_cb = sender.clone();
                let modifiers = key.modifiers.clone();
                button.connect("tap-pressed", true, move |_| {
                    for modifier in &modifiers {
                        sender_cb.input(KeyMessage::ModPress(modifier.code()).into());
                    }
                    sender_cb.input(KeyMessage::ButtonPress(scan_code).into());
                    None
                });

                let state = Arc::new(ArcSwapOption::from(None));

                let key_cb = key.clone();
                let state_cb = state.clone();
                let sender_cb = sender.clone();
                button.connect("swipe-pressed", true, move |args| {
                    let dir: Direction = unsafe { Direction::from_value(&args[1]) };
                    let action = key_cb.dir_action(dir);
                    if let Some(action) = action {
                        debug!("  [Swipe] Pressed: {:?} -> {:?}", dir, action);
                        state_cb.store(Some(Arc::new(dir)));
                        handle_swipe_action_press(&key_cb, action, dir, &sender_cb);
                    }
                    None
                });

                let key_cb = key.clone();
                let state_cb = state.clone();
                let sender_cb = sender.clone();
                button.connect("swipe-repeated", true, move |args| {
                    let dir: Direction = unsafe { Direction::from_value(&args[1]) };
                    let action = key_cb.dir_action(dir);
                    if let Some(action) = action {
                        debug!("  [Swipe] Repeated: {:?} -> {:?}", dir, action);
                        state_cb.store(Some(Arc::new(dir)));
                        handle_swipe_action_repeat(&key_cb, action, dir, &sender_cb);
                    }
                    None
                });

                let key_cb = key.clone();
                let sender_cb = sender.clone();
                let state_cb = state.clone();
                let modifiers = key.modifiers.clone();
                button.connect("released", true, move |_| {
                    if let Some(dir) = state_cb.swap(None) {
                        let action = key_cb.dir_action(*dir);
                        if let Some(action) = action {
                            debug!("  [Swipe] Released: {:?} -> {:?}", dir, action);
                            handle_swipe_action_release(&key_cb, action, *dir, &sender_cb);
                        }
                    } else {
                        sender_cb.input(KeyMessage::ButtonRelease(scan_code).into());
                        for modifier in &modifiers {
                            sender_cb.input(KeyMessage::ModRelease(modifier.code()).into());
                        }
                    }
                    None
                });

                button.upcast()
            }
        }
    }
}

impl Layer {
    pub fn render(&self, sender: ComponentSender<UIModel>) -> gtk::Overlay {
        let overlay = gtk::Overlay::new();

        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        for row in self.rows() {
            let row_container = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .build();

            row.iter().for_each(|key| {
                let button = key.render(KEY_SIZE, &sender);
                button.set_margin_all(KEY_SPACING);
                row_container.append(&button);
            });

            container.append(&row_container);
        }

        // Add a invisible swipe area on the left half of each keyboard half.
        let drag_handle = gtk::Box::new(gtk::Orientation::Vertical, 0);
        drag_handle.add_css_class("drag-handle");
        drag_handle.set_size_request(KB_PADDING, -1);
        drag_handle.set_halign(gtk::Align::Start);
        drag_handle.set_valign(gtk::Align::Fill);
        overlay.add_overlay(&drag_handle);

        // Attach a drag handler to detect the swipes/drags.
        let drag = gtk::GestureDrag::new();
        let sender_cb = sender.clone();
        drag.connect_drag_update(move |_, _, y| {
            let change = if y > 0. { -1 } else { 1 };
            sender_cb.input(UIMessage::FadeKeyboard(change));
        });
        drag_handle.add_controller(drag);

        // Swiping/dragging leads to weird velocity/offset values
        // if the swipe/drag ends outside of the gtk window.
        // Having some margin helps protect against this.
        container.set_margin_all(KB_PADDING);
        container.set_align(gtk::Align::Center);
        container.set_expand(true);

        overlay.set_child(Some(&container));
        overlay
    }
}

fn send_key(key: u16, sender: &ComponentSender<UIModel>) {
    sender.input(KeyMessage::ButtonPress(key).into());
    sender.input(KeyMessage::ButtonRelease(key).into());
}

fn send_mod_key(modifier: u16, key: u16, sender: &ComponentSender<UIModel>) {
    sender.input(KeyMessage::ModPress(modifier).into());
    sender.input(KeyMessage::ButtonPress(key).into());
    sender.input(KeyMessage::ButtonRelease(key).into());
    sender.input(KeyMessage::ModRelease(modifier).into());
}

fn send_mods_key(modifiers: Vec<u16>, key: u16, sender: &ComponentSender<UIModel>) {
    for modifier in &modifiers {
        sender.input(KeyMessage::ModPress(*modifier).into());
    }
    sender.input(KeyMessage::ButtonPress(key).into());
    sender.input(KeyMessage::ButtonRelease(key).into());
    for modifier in &modifiers {
        sender.input(KeyMessage::ModRelease(*modifier).into());
    }
}

fn handle_swipe_action_press(
    key_def: &BasicKey,
    action: &SwipeAction,
    dir: Direction,
    sender: &ComponentSender<UIModel>,
) {
    let scan_code = key_def.key.code();

    match action {
        SwipeAction::Key(key) => {
            send_key(key.code(), sender);
        }
        SwipeAction::ModKey(key, modifiers) => {
            let modifiers = modifiers.iter().map(Modifier::code).collect();
            send_mods_key(modifiers, key.code(), sender);
        }
        SwipeAction::Modified(modifier) => {
            send_mod_key(modifier.code(), scan_code, sender);
        }
        SwipeAction::Layer(side, idx) => {
            sender.input(KeyMessage::Layer(*side, *idx).into());
            sender.input(UIMessage::UpdateLayout);
        }
        SwipeAction::Arrow => {
            let key: evdev::Key = dir.into();
            send_key(key.code(), sender);
        }
        SwipeAction::Select => {
            let key: evdev::Key = dir.into();
            send_mod_key(Modifier::Shift.code(), key.code(), sender);
        }
        SwipeAction::Delete => {
            let key: evdev::Key = dir.into();
            send_mod_key(Modifier::Shift.code(), key.code(), sender);
        }
        SwipeAction::Scroll => {
            let msg = match dir {
                Direction::Up => PointerMessage::ScrollUp,
                Direction::Right => PointerMessage::ScrollRight,
                Direction::Left => PointerMessage::ScrollLeft,
                Direction::Down => PointerMessage::ScrollDown,
            };
            sender.input(msg.into());
        }
        SwipeAction::Command(Command { cmd, args, .. }) => {
            sender.input(UIMessage::Command(cmd.clone(), args.clone()));
        }
        SwipeAction::HideKeyboard => {
            // Trigger this on release,
            // otherwise the keyboard is hidden
            // before release is triggered, which
            // can cause state issues.
        }
    }
}

fn handle_swipe_action_repeat(
    key_def: &BasicKey,
    action: &SwipeAction,
    dir: Direction,
    sender: &ComponentSender<UIModel>,
) {
    match action {
        SwipeAction::Scroll | SwipeAction::Delete | SwipeAction::Select | SwipeAction::Arrow => {
            handle_swipe_action_press(key_def, action, dir, sender)
        }
        _ => {}
    }
}

fn handle_swipe_action_release(
    _key_def: &BasicKey,
    action: &SwipeAction,
    _dir: Direction,
    sender: &ComponentSender<UIModel>,
) {
    match action {
        // Swipe-releasing is only relevant for the layer swipe action.
        SwipeAction::Layer(side, _) => {
            sender.input(KeyMessage::Layer(*side, 0).into());
            sender.input(UIMessage::UpdateLayout);
        }

        // TODO the downside with this approach, which doesn't
        // require any specific input field API, is that it will
        // still delete one character on an empty selection;
        // i.e. normal backspace behavior.
        SwipeAction::Delete => {
            send_key(evdev::Key::KEY_BACKSPACE.code(), sender);
        }

        SwipeAction::HideKeyboard => {
            sender.input(UIMessage::HideKeyboard);
        }
        _ => (),
    }
}

impl From<Direction> for evdev::Key {
    fn from(dir: Direction) -> Self {
        match dir {
            Direction::Up => evdev::Key::KEY_UP,
            Direction::Right => evdev::Key::KEY_RIGHT,
            Direction::Left => evdev::Key::KEY_LEFT,
            Direction::Down => evdev::Key::KEY_DOWN,
        }
    }
}
