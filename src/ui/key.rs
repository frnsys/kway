//! Defines a single key (button) in the UI
//! and implements its interactions:
//!
//! - tap/click
//! - hold (tap-hold)
//! - swipe (single direction, swipe-and-release)
//! - swipe-and-hold (single direction)
//! - drag (potentially back-and-forth)

use std::{
    sync::{Arc, OnceLock, RwLock},
    time::Duration,
};

use arc_swap::ArcSwap;
use gdk4::glib::{Properties, Type, subclass::Signal};
use gtk::{glib, prelude::*, subclass::prelude::*};
use relm4::gtk;
use tracing::debug;

use super::swipe::{did_swipe, did_swipe_increment};

#[derive(Debug, Default, Properties)]
#[properties(wrapper_type = KeyButton)]
pub struct ButtonInner {
    #[property(get, set)]
    primary_content: Arc<RwLock<Option<String>>>,
}

/// How long a key must be pressed in a non-swipe
/// to trigger hold-and-repeat.
const HOLD_TERM: u64 = 500;

#[glib::object_subclass]
impl ObjectSubclass for ButtonInner {
    const NAME: &'static str = "KeyButton";
    type Type = KeyButton;
    type ParentType = gtk::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_layout_manager_type::<gtk::BinLayout>();
        class.set_css_name("button");
        class.set_accessible_role(gtk::AccessibleRole::Button);
    }
}

#[derive(Clone, Copy)]
enum KeyState {
    Idle,
    Unclaimed,
    Pressed,
    Swiping { x: f64, y: f64 },
}

impl KeyState {
    fn can_press(&self) -> bool {
        matches!(self, KeyState::Unclaimed)
    }

    fn can_swipe(&self) -> bool {
        matches!(self, KeyState::Unclaimed)
    }

    fn last_swipe_offset(&self) -> Option<(f64, f64)> {
        match self {
            KeyState::Swiping { x, y } => Some((*x, *y)),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct ActionState {
    state: Arc<ArcSwap<KeyState>>,
    last_position: Arc<ArcSwap<(f64, f64)>>,
}
impl Default for ActionState {
    fn default() -> Self {
        Self {
            state: Arc::new(ArcSwap::from_pointee(KeyState::Idle)),
            last_position: Arc::new(ArcSwap::from_pointee((0., 0.))),
        }
    }
}
impl ActionState {
    fn set(&self, state: KeyState) {
        self.state.store(Arc::new(state));
    }

    fn can_press(&self) -> bool {
        self.state.load().can_press()
    }

    fn can_swipe(&self) -> bool {
        self.state.load().can_swipe()
    }

    fn set_pos(&self, pos: (f64, f64)) {
        self.last_position.store(Arc::new(pos));
    }

    fn last_pos(&self) -> (f64, f64) {
        **self.last_position.load()
    }

    fn last_swipe_offset(&self) -> Option<(f64, f64)> {
        self.state.load().last_swipe_offset()
    }

    fn reset(&self) {
        self.set(KeyState::Idle);
        self.set_pos((0., 0.));
    }
}

#[glib::derived_properties]
impl ObjectImpl for ButtonInner {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();

        obj.connect_primary_content_notify(|obj| {
            obj.update_view();
        });

        let action_state = ActionState::default();

        let gesture = gtk::GestureDrag::new();
        let weak_ref = self.downgrade();
        let state = action_state.clone();
        gesture.connect_drag_begin(move |_gesture, _x, _y| {
            debug!("[Interaction Start]");
            state.set(KeyState::Unclaimed);

            let weak_ref = weak_ref.clone();
            let state = state.clone();
            glib::timeout_add_once(Duration::from_millis(HOLD_TERM), move || {
                if state.can_press() {
                    debug!("  [Hold]");
                    state.set(KeyState::Pressed);
                    let obj = weak_ref.upgrade().unwrap();
                    obj.obj().emit_by_name::<()>("tap-pressed", &[]);
                }
            });
        });

        let obj_cb = obj.clone();
        let state = action_state.clone();
        gesture.connect_drag_update(move |gesture, _x, _y| {
            if let Some((x, y)) = gesture.offset() {
                // Calculate relative movement since the last update.
                let (last_x, last_y) = state.last_pos();
                let delta_x = x - last_x;
                let delta_y = last_y - y;
                obj_cb.emit_by_name::<()>("freemove", &[&delta_x, &delta_y, &x, &y]);
                state.set_pos((x, y));

                // Check if we started a swipe.
                let (did_swipe, dir) = did_swipe(x, y);
                if did_swipe && state.can_swipe() {
                    state.set(KeyState::Swiping { x, y });
                    debug!("[Swipe] offset={:?},{:?}", x, y);

                    if let Some(dir) = dir {
                        obj_cb.emit_by_name::<()>("swipe-pressed", &[&dir.to_value()]);
                    } else {
                        debug!("  [Swipe] no direction");
                    }

                // Otherwise check if we're incrementing a swipe (swipe-hold).
                } else if let Some(last) = state.last_swipe_offset() {
                    if let (true, dir) = did_swipe_increment((x, y), last) {
                        state.set(KeyState::Swiping { x, y });
                        if let Some(dir) = dir {
                            obj_cb.emit_by_name::<()>("swipe-repeated", &[&dir.to_value()]);
                        }
                    }
                }
            }
        });

        let obj_cb = obj.clone();
        let state = action_state.clone();
        gesture.connect_drag_end(move |_gesture, _x, _y| {
            // If this hasn't yet been claimed as a swipe or a hold
            // then treat it as a tap.
            if state.can_press() {
                debug!("  [Tap]");
                state.set(KeyState::Pressed);
                obj_cb.emit_by_name::<()>("tap-pressed", &[]);
            }

            debug!("  [Release]");
            state.reset();
            obj_cb.emit_by_name::<()>("released", &[]);
        });
        obj.add_controller(gesture);
    }

    fn signals() -> &'static [Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                Signal::builder("swipe-pressed")
                    .param_types([Type::U8])
                    .build(),
                Signal::builder("swipe-repeated")
                    .param_types([Type::U8])
                    .build(),
                Signal::builder("tap-pressed").build(),
                Signal::builder("released").build(),
                Signal::builder("freemove")
                    .param_types([Type::F64, Type::F64, Type::F64, Type::F64])
                    .build(),
            ]
        })
    }

    fn dispose(&self) {
        if let Some(child) = self.obj().first_child() {
            child.unparent();
        }
    }
}

impl WidgetImpl for ButtonInner {}

glib::wrapper! {
    pub struct KeyButton(ObjectSubclass<ButtonInner>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl KeyButton {
    pub fn update_view(&self) {
        let primary_content = self.primary_content();

        let layout = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .build();

        if let Some(primary_content) = primary_content {
            if !primary_content.is_empty() {
                let primary_content = gtk::Label::new(Some(primary_content.as_str()));
                layout.append(&primary_content);
            }
        }

        // Remove existing content.
        if let Some(child) = self.first_child() {
            child.unparent();
        }

        layout.set_parent(self);
    }
}
impl Default for KeyButton {
    fn default() -> Self {
        glib::Object::new()
    }
}
