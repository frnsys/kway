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
}

/// Minimum distance to trigger a swipe.
const SWIPE_MIN_DISTANCE: f64 = 5.;

/// Swipe angle must be w/in this number of degrees
/// to trigger a directional swipe.
const SWIPE_ANGLE_TOLERANCE: f64 = 15.;

/// Minimum a swipe must increment to trigger repeat presses.
const SWIPE_MIN_INCREMENT: f64 = 5.;

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

#[glib::derived_properties]
impl ObjectImpl for ButtonInner {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();

        obj.connect_primary_content_notify(|obj| {
            obj.update_view();
        });

        let state = Arc::new(ArcSwap::from_pointee(KeyState::Idle));
        let last_position = Arc::new(ArcSwap::from_pointee((0f64, 0f64)));

        let gesture = gtk::GestureDrag::new();
        let weak_ref = self.downgrade();
        let state_cb = Arc::clone(&state);
        gesture.connect_drag_begin(move |_gesture, _x, _y| {
            state_cb.store(Arc::new(KeyState::Unclaimed));

            let weak_ref = weak_ref.clone();
            let state_cb = state_cb.clone();
            glib::timeout_add_once(Duration::from_millis(HOLD_TERM), move || {
                if state_cb.load().can_press() {
                    debug!("[Hold]");
                    let obj = weak_ref.upgrade().unwrap();
                    state_cb.store(Arc::new(KeyState::Pressed));
                    obj.obj().emit_by_name::<()>("tap-pressed", &[]);
                }
            });
        });

        let obj_cb = obj.clone();
        let state_cb = Arc::clone(&state);
        let last_pos_cb = last_position.clone();
        gesture.connect_drag_update(move |gesture, _x, _y| {
            if let Some((x, y)) = gesture.offset() {
                let (last_x, last_y) = **last_pos_cb.load();
                let delta_x = x - last_x;
                let delta_y = last_y - y;
                obj_cb.emit_by_name::<()>("freemove", &[&delta_x, &delta_y, &x, &y]);
                last_pos_cb.store(Arc::new((x, y)));

                if (x.abs() >= SWIPE_MIN_DISTANCE || y.abs() >= SWIPE_MIN_DISTANCE)
                    && state_cb.load().can_swipe()
                {
                    state_cb.store(Arc::new(KeyState::Swiping { x, y }));
                    debug!("[Swipe] offset={:?},{:?}", x, y);

                    if let Some(dir) = direction(x, y) {
                        debug!("[Swipe] direction={:?}", dir);
                        obj_cb.emit_by_name::<()>("swipe-pressed", &[&dir.to_value()]);
                    }
                } else if let Some((last_x, last_y)) = state_cb.load().last_swipe_offset() {
                    let dist = distance(last_x, last_y, x, y);
                    let delta_x = x - last_x;
                    let delta_y = y - last_y;
                    if dist >= SWIPE_MIN_INCREMENT {
                        debug!(
                            "[Swipe INCREMENT] offset={:?},{:?} dist={:?}",
                            delta_x, delta_y, dist
                        );
                        state_cb.store(Arc::new(KeyState::Swiping { x, y }));
                        if let Some(dir) = direction(delta_x, delta_y) {
                            debug!("[Swipe] direction={:?}", dir);
                            obj_cb.emit_by_name::<()>("swipe-pressed", &[&dir.to_value()]);
                        }
                    }
                }
            }
        });

        let obj_cb = obj.clone();
        let state_cb = Arc::clone(&state);
        let last_pos_cb = last_position.clone();
        gesture.connect_drag_end(move |_gesture, _x, _y| {
            // If this hasn't yet been claimed as a swipe or a hold
            // then treat it as a tap.
            if state_cb.load().can_press() {
                debug!("[Tap]");
                state_cb.store(Arc::new(KeyState::Pressed));
                obj_cb.emit_by_name::<()>("tap-pressed", &[]);
            }

            debug!("[Release]");
            state_cb.store(Arc::new(KeyState::Idle));
            last_pos_cb.store(Arc::new((0., 0.)));
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
            if primary_content.len() > 0 {
                let primary_content = gtk::Label::new(Some(primary_content.as_str()));
                layout.append(&primary_content);
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

#[derive(Debug, Clone, Copy)]
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

fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    (dx.powi(2) + dy.powi(2)).sqrt()
}
