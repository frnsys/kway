use gdk4::glib::{
    Value,
    value::{FromValue, GenericValueTypeChecker},
};

/// Minimum distance to trigger a swipe.
/// If this is too low, then taps may be
/// interpreted as swipes.
const SWIPE_MIN_DISTANCE: f64 = 3.;

/// Swipe angle must be w/in this number of degrees
/// to trigger a directional swipe.
const SWIPE_ANGLE_TOLERANCE: f64 = 25.;

/// Minimum a swipe must increment to trigger repeat presses.
const SWIPE_MIN_INCREMENT: f64 = 5.;

pub fn did_swipe(dx: f64, dy: f64) -> (bool, Option<Direction>) {
    let did_swipe = dx.abs() >= SWIPE_MIN_DISTANCE || dy.abs() >= SWIPE_MIN_DISTANCE;
    if did_swipe {
        (true, direction(dx, dy))
    } else {
        (false, None)
    }
}

pub fn did_swipe_increment(
    (x, y): (f64, f64),
    (last_x, last_y): (f64, f64),
) -> (bool, Option<Direction>) {
    let dist = distance(last_x, last_y, x, y);
    let did_swipe = dist >= SWIPE_MIN_INCREMENT;
    if did_swipe {
        let dx = x - last_x;
        let dy = y - last_y;
        (true, direction(dx, dy))
    } else {
        (false, None)
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
    pub fn as_value(&self) -> u8 {
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
