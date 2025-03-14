mod glyphs;
mod key;
mod keyboard;
mod swipe;

use std::process::Command;

use gdk4::glib::{self, object::ObjectExt};
use gtk::prelude::{ApplicationExt, GtkWindowExt, WidgetExt};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::{
    ComponentParts, ComponentSender, SimpleComponent,
    gtk::{self, prelude::GtkApplicationExt},
};

use crate::{
    keyboard::{KeyMessage, Keyboard},
    layout::TriggerKey,
    pointer::{Pointer, PointerMessage},
};

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
        let (keyboard, pointer) = handle;

        // The main window hosts the button
        // to show the keyboard.
        let trigger = setup_trigger_key(keyboard.trigger_key(), sender.clone());
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
                #[allow(clippy::zombie_processes)]
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

fn setup_trigger_key(trigger_key: &TriggerKey, sender: ComponentSender<UIModel>) -> gtk::Widget {
    let trigger = trigger_key.as_key();
    let trigger = trigger.render(8, &sender);
    trigger.set_css_classes(&["trigger"]);

    let sender_cb = sender.clone();
    trigger.connect("tap-pressed", false, move |args| {
        let obj = args[0].get::<glib::Object>().expect("Failed to get object");
        obj.stop_signal_emission_by_name("tap-pressed");
        sender_cb.input(UIMessage::ShowKeyboard);
        None
    });
    trigger
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
