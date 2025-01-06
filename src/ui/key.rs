use std::{
    sync::{Arc, OnceLock, RwLock},
    time::Duration,
};

use arc_swap::ArcSwap;
use gdk4::glib::{
    Properties, Type, Value,
    subclass::Signal,
    value::{FromValue, GenericValueTypeChecker},
};
use gtk::{glib, prelude::*, subclass::prelude::*};
use relm4::gtk;
use tracing::debug;

#[derive(Debug, Default, Properties)]
#[properties(wrapper_type = KeyButton)]
pub struct ButtonInner {
    #[property(get, set)]
    primary_content: Arc<RwLock<Option<String>>>,
    #[property(get, set)]
    secondary_content: Arc<RwLock<Option<String>>>,
}

/// Minimum velocity to trigger a swipe.
const SWIPE_MIN_VELOCITY: f64 = 100.;

/// Swipe angle must be w/in this number of degrees
/// to trigger a directional swipe.
const SWIPE_ANGLE_TOLERANCE: f64 = 15.;

/// The velocity of an incremental swipe
/// to fire a drag swipe action.
const DRAG_TRIGGER_VELOCITY: f64 = 50.;

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

enum KeyState {
    Idle,
    Pressed,
    Swiping,
}

impl KeyState {
    fn can_press(&self) -> bool {
        matches!(self, KeyState::Idle)
    }

    fn can_swipe(&self) -> bool {
        matches!(self, KeyState::Idle)
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
        obj.connect_secondary_content_notify(|obj| {
            obj.update_view();
        });

        let state = Arc::new(ArcSwap::from_pointee(KeyState::Idle));

        let gesture = gtk::GestureSwipe::new();

        let obj_cb = obj.clone();
        let state_cb = Arc::clone(&state);
        gesture.connect_swipe(move |gesture, vel_x, vel_y| {
            if vel_x.abs() < SWIPE_MIN_VELOCITY && vel_y.abs() < SWIPE_MIN_VELOCITY {
                return;
            } else if state_cb.load().can_swipe() {
                state_cb.store(Arc::new(KeyState::Swiping));
                debug!("[Swipe] velocity={:?},{:?}", vel_x, vel_y);

                if let Some(dir) = direction(vel_x, vel_y) {
                    debug!("[Swipe] direction={:?}", dir);
                    obj_cb.emit_by_name::<()>("swipe-pressed", &[&dir.to_value()]);
                }
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });

        gesture.connect_update(move |gesture, _| {
            if let Some((vel_x, vel_y)) = gesture.velocity() {
                if let Some(dir) = direction(vel_x, vel_y) {
                    debug!("[Drag] direction={:?}", dir);
                    // TODO
                    // obj_cb.emit_by_name::<()>("swipe-pressed", &[&dir.to_value()]);
                }
            }
        });

        let state_cb = Arc::clone(&state);
        gesture.connect_sequence_state_changed(move |_gesture, _, state| {
            if state == gtk::EventSequenceState::Claimed {
                state_cb.store(Arc::new(KeyState::Idle));
            }
        });
        obj.add_controller(gesture);

        let gesture = gtk::GestureClick::new();
        let weak_ref = self.downgrade();
        let state_cb = Arc::clone(&state);
        gesture.connect_pressed(move |_gesture, _, _, _| {
            let weak_ref = weak_ref.clone();
            let state_cb = state_cb.clone();
            glib::timeout_add_once(Duration::from_millis(60), move || {
                if state_cb.load().can_press() {
                    debug!("[Tap] pressed");
                    let obj = weak_ref.upgrade().unwrap();
                    state_cb.store(Arc::new(KeyState::Pressed));
                    obj.obj().emit_by_name::<()>("tap-pressed", &[]);
                } else {
                    debug!("[Tap] swipe locked");
                }
            });
        });

        let obj_cb = obj.clone();
        let state_cb = Arc::clone(&state);
        gesture.connect_released(move |_gesture, _, _, _| {
            debug!("[Tap] released");
            state_cb.store(Arc::new(KeyState::Idle));
            obj_cb.emit_by_name::<()>("released", &[]);
        });
        obj.add_controller(gesture);

        let gesture = gtk::GestureDrag::new();
        gesture.connect_drag_begin(move |_gesture, _x, _y| {
            debug!("[Drag] begin");
        });
        gesture.connect_drag_update(move |_gesture, _x, _y| {
            debug!("[Drag] update");
        });
        gesture.connect_drag_end(move |_gesture, _x, _y| {
            debug!("[Drag] end");
            // println!("offset: {:?}", gesture.offset());
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
                Signal::builder("swipe-released").build(),
                Signal::builder("tap-pressed").build(),
                Signal::builder("tap-released").build(),
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
        let secondary_content = self.secondary_content();

        let layout = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .build();

        if let Some(primary_content) = primary_content {
            if primary_content.len() > 0 {
                let primary_content = gtk::Label::new(Some(primary_content.as_str()));
                layout.append(&primary_content);
            }
        }

        if let Some(secondary_content) = secondary_content {
            if secondary_content.len() > 0 {
                let secondary_content = gtk::Label::new(Some(secondary_content.as_str()));
                layout.append(&secondary_content);
            }
        }

        // Remove existing content.
        if let Some(child) = self.first_child() {
            child.unparent();
        }

        layout.set_parent(&*self);
    }
}

impl Default for KeyButton {
    fn default() -> Self {
        glib::Object::new()
    }
}

#[derive(Debug)]
pub enum Direction {
    Up,
    Left,
    Right,
    Down,
}
impl Direction {
    fn to_value(&self) -> u8 {
        match self {
            Self::Up => 0,
            Self::Left => 1,
            Self::Right => 2,
            Self::Down => 3,
        }
    }
}

unsafe impl FromValue<'_> for Direction {
    type Checker = GenericValueTypeChecker<u8>;
    unsafe fn from_value(value: &Value) -> Self {
        let value = value.get::<u8>().unwrap();
        match value {
            0 => Self::Up,
            1 => Self::Left,
            2 => Self::Right,
            3 => Self::Down,
            _ => panic!("Unknown enum variant"),
        }
    }
}

fn direction(x: f64, y: f64) -> Option<Direction> {
    let rad = y.atan2(x);
    let deg = rad * (180.0 / std::f64::consts::PI);
    debug!("[Swipe] angle={:?}", deg);
    if (-90. - deg).abs() <= SWIPE_ANGLE_TOLERANCE {
        Some(Direction::Up)
    } else if deg.abs() <= SWIPE_ANGLE_TOLERANCE {
        Some(Direction::Right)
    } else if (180. - deg).abs() <= SWIPE_ANGLE_TOLERANCE {
        Some(Direction::Left)
    } else if (90. - deg).abs() <= SWIPE_ANGLE_TOLERANCE {
        Some(Direction::Down)
    } else {
        None
    }
}
