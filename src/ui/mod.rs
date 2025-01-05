mod key;

use gdk4::{glib::value::FromValue, prelude::ObjectExt};
use gtk::prelude::{ApplicationExt, BoxExt, GtkWindowExt, ToggleButtonExt, WidgetExt};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::{
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent,
    gtk::{self, prelude::GtkApplicationExt},
};

use crate::{
    kbd::{self, KeyAction, KeyMessage, KeyType, Keyboard, Layout},
    ptr::Pointer,
};

use key::{Direction, KeyButton};

// TODO swiping/dragging leads to weird velocity/offset values
// if the swipe/drag ends outside of the gtk window. not sure how to handle this.

pub struct UIModel {
    /// We use two windows, one for each half of the keyboard.
    /// This lets input in the area between the two halves pass through.
    window: (gtk::Window, gtk::Window),
    sender: ComponentSender<Self>,
    keyboard: Keyboard,
}

#[derive(Debug)]
pub enum UIMessage {
    Keyboard(KeyMessage),
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

        let model = UIModel {
            keyboard,
            window: (left, right),
            sender,
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
        let (left, right) = self.keyboard.layout.render(160, self.sender.clone());
        self.window.0.set_child(Some(&left));
        self.window.1.set_child(Some(&right));
    }
}

impl kbd::Layer {
    fn render(&self, geometry_unit: i32, sender: ComponentSender<UIModel>) -> gtk::Box {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        for row in self.rows() {
            let row_container = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .build();

            row.iter().for_each(|key| {
                let key = key.clone();
                let scan_code = key.key.code();
                let width = (key.width() * f32::from(geometry_unit as u16)).round() as i32;

                match key.key_type() {
                    KeyType::Mod => {
                        let toggle = gtk::ToggleButton::builder()
                            .label(key.glyph())
                            .width_request(width)
                            .height_request(geometry_unit)
                            .build();

                        let button_sender = sender.clone();
                        toggle.connect_toggled(move |btn| {
                            if btn.is_active() {
                                button_sender.input(KeyMessage::ModPress(scan_code).into());
                            } else {
                                button_sender.input(KeyMessage::ModRelease(scan_code).into());
                            }
                        });

                        row_container.append(&toggle);
                    }
                    KeyType::Lock => {
                        let toggle = gtk::ToggleButton::builder()
                            .label("TODO")
                            .width_request(width)
                            .height_request(geometry_unit)
                            .build();

                        let button_sender = sender.clone();
                        toggle.connect_toggled(move |btn| {
                            if btn.is_active() {
                                button_sender.input(KeyMessage::LockPress(scan_code).into());
                            } else {
                                button_sender.input(KeyMessage::LockRelease(scan_code).into());
                            }
                        });

                        row_container.append(&toggle);
                    }
                    KeyType::Normal => {
                        let button = KeyButton::default();
                        button.set_primary_content(key.glyph());
                        button.set_secondary_content(key.glyph());

                        button.set_width_request(width);
                        button.set_height_request(geometry_unit);

                        let press_sender = sender.clone();
                        button.connect("pressed", true, move |_| {
                            press_sender.input(KeyMessage::ButtonPress(scan_code).into());
                            None
                        });

                        let release_sender = sender.clone();
                        button.connect("released", true, move |_| {
                            release_sender.input(KeyMessage::ButtonRelease(scan_code).into());
                            None
                        });

                        let swipe_sender = sender.clone();
                        button.connect("swiped", true, move |args| {
                            let dir: Direction = unsafe { Direction::from_value(&args[1]) };
                            let action = match dir {
                                Direction::Up => &key.up,
                                Direction::Right => &key.right,
                                Direction::Left => &key.left,
                                Direction::Down => &key.down,
                            };
                            if let Some(action) = action {
                                match action {
                                    KeyAction::Key(key) => {
                                        swipe_sender
                                            .input(KeyMessage::ButtonPress(key.code()).into());
                                        swipe_sender
                                            .input(KeyMessage::ButtonRelease(key.code()).into());
                                    }
                                    // TODO
                                    _ => (),
                                }
                            }
                            None
                        });

                        row_container.append(&button);
                    }
                }
            });

            container.append(&row_container);
        }

        container
    }
}

impl Layout {
    fn render(&self, height: i32, sender: ComponentSender<UIModel>) -> (gtk::Box, gtk::Box) {
        let geometry_unit = height / self.height();
        let Self { left, right } = self;

        // TODO get current layer
        let left = left[0].render(geometry_unit, sender.clone());
        left.set_margin_all(15);
        left.set_align(gtk::Align::Center);
        left.set_expand(true);

        // TODO get current layer
        let right = right[0].render(geometry_unit, sender);
        right.set_margin_all(15);
        right.set_align(gtk::Align::Center);
        right.set_expand(true);

        (left, right)
    }
}
