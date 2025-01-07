mod key;

use std::{process::Command, sync::Arc};

use arc_swap::ArcSwapOption;
use gdk4::{
    glib::value::FromValue,
    prelude::{Cast, ObjectExt},
};
use gtk::prelude::{ApplicationExt, BoxExt, GtkWindowExt, ToggleButtonExt, WidgetExt};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::{
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent,
    gtk::{
        self,
        prelude::{ButtonExt, GtkApplicationExt},
    },
};
use tracing::debug;

use crate::{
    kbd::{self, BasicKey, KeyDef, KeyMessage, KeyType, Keyboard, Modifier, SwipeAction},
    ptr::{Pointer, PointerMessage},
};

use key::{Direction, KeyButton};

pub struct UIModel {
    /// We use two windows, one for each half of the keyboard.
    /// This lets input in the area between the two halves pass through.
    window: (gtk::Window, gtk::Window),

    trigger: gtk::Window,
    keyboard: Keyboard,
    pointer: Pointer,
    left: Vec<gtk::Box>,
    right: Vec<gtk::Box>,
}

#[derive(Debug)]
pub enum UIMessage {
    /// Pass message to the keyboard.
    Keyboard(KeyMessage),

    /// Pass message to the pointer.
    Pointer(PointerMessage),

    /// Execute a command.
    Command(String, Vec<String>),

    /// Update displayed layouts.
    UpdateLayout,

    /// Hide the keyboard.
    HideKeyboard,

    /// Show the keyboard.
    ShowKeyboard,

    /// Quit the application.
    Quit,
}
impl From<KeyMessage> for UIMessage {
    fn from(value: KeyMessage) -> Self {
        Self::Keyboard(value)
    }
}
impl From<PointerMessage> for UIMessage {
    fn from(value: PointerMessage) -> Self {
        Self::Pointer(value)
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
        // The main window hosts the button
        // to show the keyboard.
        let trigger = gtk::Button::builder()
            .label("")
            .width_request(8)
            .height_request(8)
            .css_classes(["trigger"])
            .build();
        let sender_cb = sender.clone();
        trigger.connect_clicked(move |_| {
            sender_cb.input(UIMessage::ShowKeyboard);
        });
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_anchor(Edge::Right, true);
        window.set_anchor(Edge::Bottom, true);
        window.set_child(Some(&trigger));
        window.set_visible(false);

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
            pointer,
            keyboard,
            trigger: window,
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
        model.window.0.set_visible(false);
        model.window.1.set_visible(false);

        ComponentParts { model, widgets: () }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            UIMessage::Keyboard(msg) => {
                self.keyboard.handle(msg);
            }
            UIMessage::Pointer(msg) => {
                self.pointer.handle(msg).unwrap();
            }
            UIMessage::Command(cmd, args) => {
                Command::new(cmd)
                    .args(args)
                    .spawn()
                    .expect("Command failed to start");
            }
            UIMessage::UpdateLayout => {
                self.render_keyboard();
            }
            UIMessage::HideKeyboard => {
                self.hide_keyboard();
            }
            UIMessage::ShowKeyboard => {
                self.show_keyboard();
            }
            UIMessage::Quit => {
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

    fn show_keyboard(&self) {
        self.trigger.set_visible(false);
        self.window.0.set_visible(true);
        self.window.1.set_visible(true);
    }

    fn hide_keyboard(&self) {
        self.trigger.set_visible(true);
        self.window.0.set_visible(false);
        self.window.1.set_visible(false);
    }
}

impl kbd::KeyDef {
    fn render(&self, size: i32, sender: &ComponentSender<UIModel>) -> gtk::Widget {
        match self {
            KeyDef::Basic(key) => key.render(size, sender),
            KeyDef::Command { label, cmd, args } => {
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
                let button = KeyButton::default();
                button.set_primary_content(key.glyph());
                button.set_width_request(size);
                button.set_height_request(size);

                let key = key.clone();
                let sender_cb = sender.clone();
                button.connect("tap-pressed", true, move |_| {
                    sender_cb.input(PointerMessage::Press(key).into());
                    None
                });

                let key = key.clone();
                let sender_cb = sender.clone();
                button.connect("released", true, move |_| {
                    sender_cb.input(PointerMessage::Release(key).into());
                    None
                });

                button.upcast()
            }
            KeyDef::Pointer => {
                let glyph = "※";
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

impl kbd::BasicKey {
    fn render(&self, size: i32, sender: &ComponentSender<UIModel>) -> gtk::Widget {
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
                let modifiers = key.modifiers.clone();
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
        SwipeAction::HideKeyboard => {
            sender.input(UIMessage::HideKeyboard);
        }
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
        _ => (),
    }
}
