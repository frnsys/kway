mod key;

use std::sync::Arc;

use arc_swap::ArcSwapOption;
use gdk4::{
    glib::value::FromValue,
    prelude::{Cast, ObjectExt},
};
use gtk::prelude::{ApplicationExt, BoxExt, GtkWindowExt, ToggleButtonExt, WidgetExt};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::{
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent,
    gtk::{self, prelude::GtkApplicationExt},
};
use tracing::debug;

use crate::{
    kbd::{self, KeyDef, KeyMessage, KeyType, Keyboard, SwipeAction},
    ptr::Pointer,
};

use key::{Direction, KeyButton};

pub struct UIModel {
    /// We use two windows, one for each half of the keyboard.
    /// This lets input in the area between the two halves pass through.
    window: (gtk::Window, gtk::Window),
    keyboard: Keyboard,
    left: Vec<gtk::Box>,
    right: Vec<gtk::Box>,
}

#[derive(Debug)]
pub enum UIMessage {
    Keyboard(KeyMessage),
    LayoutChanged,
    AppQuit,
}
impl From<KeyMessage> for UIMessage {
    fn from(value: KeyMessage) -> Self {
        Self::Keyboard(value)
    }
}

impl SimpleComponent for UIModel {
    type Init = (Keyboard, Pointer);

    type Input = UIMessage;
    type Output = ();
    type Widgets = ();

    /// Note: A hacky approach is needed to have
    /// a multi-window app with `relm4`.
    ///
    /// We pretend like we'll only have one window.
    type Root = gtk::Window;

    fn init_root() -> Self::Root {
        gtk::Window::builder().build()
    }

    fn init(
        handle: Self::Init,
        window: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // The dummy window is pushed to the back
        // and made invisible.
        window.init_layer_shell();
        window.set_layer(Layer::Background);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_opacity(0.0);

        // Then we initialize our actual two windows.
        let (mut left, mut right) = (
            gtk::Window::builder().build(),
            gtk::Window::builder().build(),
        );
        setup_window(&mut left, true);
        setup_window(&mut right, false);

        let (keyboard, pointer) = handle;

        let left_halves: Vec<_> = keyboard
            .left_layers()
            .map(|layer| layer.render(sender.clone()))
            .collect();
        let right_halves: Vec<_> = keyboard
            .right_layers()
            .map(|layer| layer.render(sender.clone()))
            .collect();

        let model = UIModel {
            keyboard,
            window: (left, right),
            left: left_halves,
            right: right_halves,
        };
        model.render_keyboard();

        // Then we manually add our two windows
        // to the application.
        let app = relm4::main_application();
        app.add_window(&model.window.0);
        app.add_window(&model.window.1);
        model.window.0.present();
        model.window.1.present();

        // Close the dummy window.
        window.close();

        ComponentParts { model, widgets: () }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            UIMessage::Keyboard(msg) => {
                self.keyboard.handle(msg);
            }
            UIMessage::LayoutChanged => {
                self.render_keyboard();
            }
            UIMessage::AppQuit => {
                self.keyboard.destroy();
                relm4::main_application().quit();
            }
        }
    }
}

/// Setup the window for a half of the keyboard.
fn setup_window(window: &mut gtk::Window, is_left: bool) {
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    // window.set_opacity(0.1);

    let anchors = [
        (Edge::Left, is_left),
        (Edge::Right, !is_left),
        (Edge::Top, false),
        (Edge::Bottom, true),
    ];
    for (anchor, state) in anchors {
        window.set_anchor(anchor, state);
    }
}

impl UIModel {
    fn render_keyboard(&self) {
        let (left, right) = self.keyboard.layer;
        self.window.0.set_child(Some(&self.left[left]));
        self.window.1.set_child(Some(&self.right[right]));
    }
}

impl kbd::KeyDef {
    fn render(&self, size: i32, sender: &ComponentSender<UIModel>) -> gtk::Widget {
        let key = self.clone();
        let scan_code = key.key.code();
        let width = (key.width() * f32::from(size as u16)).round() as i32;

        match key.key_type() {
            KeyType::Mod => {
                let toggle = gtk::ToggleButton::builder()
                    .label(key.glyph())
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
                    .label("TODO")
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
                button.set_primary_content(key.glyph());

                button.set_width_request(width);
                button.set_height_request(size);

                let sender_cb = sender.clone();
                button.connect("tap-pressed", true, move |_| {
                    sender_cb.input(KeyMessage::ButtonPress(scan_code).into());
                    None
                });

                let state = Arc::new(ArcSwapOption::from(None));

                let key_cb = key.clone();
                let sender_cb = sender.clone();
                let state_cb = state.clone();
                button.connect("swipe-pressed", true, move |args| {
                    let dir: Direction = unsafe { Direction::from_value(&args[1]) };
                    let action = match dir {
                        Direction::Up => &key_cb.up,
                        Direction::Right => &key_cb.right,
                        Direction::Left => &key_cb.left,
                        Direction::Down => &key_cb.down,
                    };
                    if let Some(action) = action {
                        debug!("[Swipe] Pressed: {:?} -> {:?}", dir, action);
                        state_cb.store(Some(Arc::new(dir)));
                        handle_swipe_action_press(&key_cb, action, dir, &sender_cb);
                    }
                    None
                });

                let key_cb = key.clone();
                let sender_cb = sender.clone();
                let state_cb = state.clone();
                button.connect("released", true, move |_| {
                    if let Some(dir) = state_cb.swap(None).take() {
                        let action = match *dir {
                            Direction::Up => &key_cb.up,
                            Direction::Right => &key_cb.right,
                            Direction::Left => &key_cb.left,
                            Direction::Down => &key_cb.down,
                        };
                        if let Some(action) = action {
                            debug!("[Swipe] Released: {:?} -> {:?}", dir, action);
                            handle_swipe_action_release(&key_cb, action, *dir, &sender_cb);
                        }
                    } else {
                        sender_cb.input(KeyMessage::ButtonRelease(scan_code).into());
                    }
                    None
                });

                button.upcast()
            }
        }
    }
}

impl kbd::Layer {
    fn render(&self, sender: ComponentSender<UIModel>) -> gtk::Box {
        let key_size = 48;

        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        for row in self.rows() {
            let row_container = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .build();

            row.iter().for_each(|key| {
                let button = key.render(key_size, &sender);
                row_container.append(&button);
            });

            container.append(&row_container);
        }

        // Swiping/dragging leads to weird velocity/offset values
        // if the swipe/drag ends outside of the gtk window.
        // Having some margin helps protect against this.
        container.set_margin_all(32);
        container.set_align(gtk::Align::Center);
        container.set_expand(true);
        container
    }
}

const ALT: u16 = 56;
const CTRL: u16 = 29;
const SHIFT: u16 = 42;
const META: u16 = 125;

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

fn handle_swipe_action_press(
    key_def: &KeyDef,
    action: &SwipeAction,
    dir: Direction,
    sender: &ComponentSender<UIModel>,
) {
    let scan_code = key_def.key.code();

    match action {
        SwipeAction::Key(key) => {
            send_key(key.code(), sender);
        }
        SwipeAction::Shift => {
            send_mod_key(SHIFT, scan_code, sender);
        }
        SwipeAction::Ctrl => {
            send_mod_key(CTRL, scan_code, sender);
        }
        SwipeAction::Alt => {
            send_mod_key(ALT, scan_code, sender);
        }
        SwipeAction::Meta => {
            send_mod_key(META, scan_code, sender);
        }
        SwipeAction::Layer(side, idx) => {
            sender.input(KeyMessage::Layer(*side, *idx).into());
            sender.input(UIMessage::LayoutChanged);
        }
        SwipeAction::Arrow => {
            let key = match dir {
                Direction::Up => evdev::Key::KEY_UP,
                Direction::Right => evdev::Key::KEY_RIGHT,
                Direction::Left => evdev::Key::KEY_LEFT,
                Direction::Down => evdev::Key::KEY_DOWN,
            };
            send_key(key.code(), sender);
        }
        SwipeAction::Select => {
            let key = match dir {
                Direction::Up => evdev::Key::KEY_UP,
                Direction::Right => evdev::Key::KEY_RIGHT,
                Direction::Left => evdev::Key::KEY_LEFT,
                Direction::Down => evdev::Key::KEY_DOWN,
            };
            send_mod_key(SHIFT, key.code(), sender);
        }
        SwipeAction::Delete => {
            let key = match dir {
                Direction::Up => evdev::Key::KEY_UP,
                Direction::Right => evdev::Key::KEY_RIGHT,
                Direction::Left => evdev::Key::KEY_LEFT,
                Direction::Down => evdev::Key::KEY_DOWN,
            };
            send_mod_key(SHIFT, key.code(), sender);
        }
        SwipeAction::Scroll => {
            let key = match dir {
                Direction::Up => evdev::Key::KEY_SCROLLUP,
                Direction::Right => evdev::Key::KEY_RIGHT,
                Direction::Left => evdev::Key::KEY_LEFT,
                Direction::Down => evdev::Key::KEY_SCROLLDOWN,
            };
            send_key(key.code(), sender);
        }
    }
}

fn handle_swipe_action_release(
    _key_def: &KeyDef,
    action: &SwipeAction,
    _dir: Direction,
    sender: &ComponentSender<UIModel>,
) {
    match action {
        // Swipe-releasing is only relevant for the layer swipe action.
        SwipeAction::Layer(side, _) => {
            sender.input(KeyMessage::Layer(*side, 0).into());
            sender.input(UIMessage::LayoutChanged);
        }

        // TODO the downside with this approach, which doesn't
        // require any specific input field API, is that it will
        // still delete one character on an empty selection;
        // i.e. normal backspace behavior.
        SwipeAction::Delete => {
            send_key(evdev::Key::KEY_BACKSPACE.code(), sender);
        }
        _ => (),
    }
}
