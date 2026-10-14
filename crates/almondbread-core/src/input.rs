//! Platform-neutral navigation: held keys, the zoom target, pointer and gesture deltas,
//! integrated per display frame from timestamps (kb: decisions/controls). Every UI forwards
//! its decoded events here and calls [`Navigator::advance`] once per frame; nothing about
//! motion lives in the platform layer.
//!
//! Coordinates are view points (not pixels) from the view centre, y up.

use crate::viewport::Viewport;

/// Keys with a navigation meaning. The platform layer decodes its key events into these
/// (WASD and the arrows to the four directions, Space to `Zoom`, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavKey {
    Up,
    Down,
    Left,
    Right,
    /// Space: continuous zoom toward the zoom target (with Shift: out).
    Zoom,
    /// Q: zoom speed x0.8.
    SlowerZoom,
    /// E: zoom speed x1.25.
    FasterZoom,
    /// + / =: one step in (x1.5) around the centre.
    StepIn,
    /// - / _: one step out.
    StepOut,
    /// R: back to the home view fitted to the window.
    Reset,
}

/// Decoded input events (what the platform view forwards).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    KeyDown(NavKey),
    KeyUp(NavKey),
    /// Shift state (Shift+Space zooms out).
    Shift(bool),
    /// Focus lost: key-ups may never arrive, so stop everything.
    FocusLost,
    /// The view's size in points (for the pan speed, the zoom target and Reset).
    Resize {
        width: f64,
        height: f64,
    },
    /// Pointer position (zoom target of Space); `None` when it left the view.
    Pointer(Option<(f64, f64)>),
    /// Content follows a drag / trackpad scroll / one-finger pan by (dx, dy) points.
    Pan {
        dx: f64,
        dy: f64,
    },
    /// A mouse wheel notch or precise zoom scroll: zoom by 1.15^delta around (x, y).
    WheelZoom {
        delta: f64,
        x: f64,
        y: f64,
    },
    /// Trackpad pinch (macOS magnify): zoom by 1 + magnification around (x, y).
    Magnify {
        magnification: f64,
        x: f64,
        y: f64,
    },
    /// Touch pinch: scale ratio since the last event, and the pinch centre's movement.
    Pinch {
        ratio: f64,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    },
    /// Double click / tap: zoom x2 (or out x0.5) around (x, y).
    DoubleTap {
        x: f64,
        y: f64,
        out: bool,
    },
}

/// Pan speed of the direction keys, in view widths per second.
pub const PAN_SPEED: f64 = 0.4;
/// Zoom speed range, e-folds of magnification per second.
pub const ZOOM_SPEED_RANGE: (f64, f64) = (0.1, 8.0);
/// Input younger than this counts as an ongoing gesture (the renderer shows previews).
pub const IDLE_DELAY: f64 = 0.15;

/// Held keys and gesture state. Owned by the renderer host; see the module docs.
#[derive(Clone, Debug)]
pub struct Navigator {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    zoom: bool,
    shift: bool,
    pointer: Option<(f64, f64)>,
    view_size: (f64, f64),
    /// Continuous zoom speed (Space), e-folds per second; Q / E change it.
    zoom_speed: f64,
    /// When the zoom speed last changed (for a HUD highlight).
    pub zoom_speed_changed: f64,
    last_tick: Option<f64>,
    /// Time of the last input that moved the view.
    pub last_interaction: f64,
}

impl Default for Navigator {
    fn default() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            zoom: false,
            shift: false,
            pointer: None,
            view_size: (0.0, 0.0),
            zoom_speed: 1.6,
            zoom_speed_changed: f64::NEG_INFINITY,
            last_tick: None,
            last_interaction: f64::NEG_INFINITY,
        }
    }
}

impl Navigator {
    pub fn zoom_speed(&self) -> f64 {
        self.zoom_speed
    }

    pub fn set_zoom_speed(&mut self, v: f64, now: f64) {
        self.zoom_speed = v.clamp(ZOOM_SPEED_RANGE.0, ZOOM_SPEED_RANGE.1);
        self.zoom_speed_changed = now;
    }

    /// Any motion key held.
    pub fn is_moving(&self) -> bool {
        self.up || self.down || self.left || self.right || self.zoom
    }

    /// Number of held motion keys.
    pub fn held_count(&self) -> usize {
        [self.up, self.down, self.left, self.right, self.zoom]
            .iter()
            .filter(|&&b| b)
            .count()
    }

    /// Input younger than [`IDLE_DELAY`]: a gesture is going on.
    pub fn is_interacting(&self, now: f64) -> bool {
        now - self.last_interaction < IDLE_DELAY
    }

    /// Records input that moves the view now (the scheduler then shows gesture frames).
    pub fn note_interaction(&mut self, now: f64) {
        self.last_interaction = now;
    }

    pub fn release_all(&mut self) {
        self.up = false;
        self.down = false;
        self.left = false;
        self.right = false;
        self.zoom = false;
        self.shift = false;
    }

    /// Applies one event. Returns true if the event was ours (the platform swallows it).
    pub fn handle(&mut self, event: InputEvent, viewport: &mut Viewport, now: f64) -> bool {
        match event {
            InputEvent::KeyDown(k) => self.key(k, true, viewport, now),
            InputEvent::KeyUp(k) => self.key(k, false, viewport, now),
            InputEvent::Shift(s) => {
                self.shift = s;
                false
            }
            InputEvent::FocusLost => {
                self.release_all();
                true
            }
            InputEvent::Resize { width, height } => {
                self.view_size = (width, height);
                true
            }
            InputEvent::Pointer(p) => {
                self.pointer = p;
                true
            }
            InputEvent::Pan { dx, dy } => {
                self.note_interaction(now);
                viewport.pan_by_points(dx, dy);
                true
            }
            InputEvent::WheelZoom { delta, x, y } => {
                self.zoom_at(viewport, 1.15f64.powf(delta), x, y, now);
                true
            }
            InputEvent::Magnify { magnification, x, y } => {
                self.zoom_at(viewport, 1.0 + magnification, x, y, now);
                true
            }
            InputEvent::Pinch { ratio, x, y, dx, dy } => {
                self.note_interaction(now);
                // Two-finger drag pans while pinching.
                viewport.pan_by_points(dx, dy);
                viewport.zoom_by(ratio, x, y);
                true
            }
            InputEvent::DoubleTap { x, y, out } => {
                self.zoom_at(viewport, if out { 0.5 } else { 2.0 }, x, y, now);
                true
            }
        }
    }

    fn zoom_at(&mut self, viewport: &mut Viewport, factor: f64, x: f64, y: f64, now: f64) {
        self.note_interaction(now);
        viewport.zoom_by(factor, x, y);
    }

    fn key(&mut self, k: NavKey, down: bool, viewport: &mut Viewport, now: f64) -> bool {
        match k {
            NavKey::Up => self.up = down,
            NavKey::Down => self.down = down,
            NavKey::Left => self.left = down,
            NavKey::Right => self.right = down,
            NavKey::Zoom => self.zoom = down,
            NavKey::SlowerZoom if down => self.set_zoom_speed(self.zoom_speed * 0.8, now),
            NavKey::FasterZoom if down => self.set_zoom_speed(self.zoom_speed * 1.25, now),
            NavKey::StepIn if down => self.zoom_at(viewport, 1.5, 0.0, 0.0, now),
            NavKey::StepOut if down => self.zoom_at(viewport, 1.0 / 1.5, 0.0, 0.0, now),
            NavKey::Reset if down => {
                *viewport = Viewport::fitting(self.view_size.0, self.view_size.1);
            }
            _ => {}
        }
        true
    }

    /// The zoom target: the pointer while it is inside the view, else the centre.
    pub fn zoom_target(&self) -> (f64, f64) {
        match self.pointer {
            Some((x, y)) if x.abs() <= self.view_size.0 / 2.0 && y.abs() <= self.view_size.1 / 2.0 => (x, y),
            _ => (0.0, 0.0),
        }
    }

    /// Pan in view points (content follows the camera) and the zoom factor for a frame of
    /// `dt` seconds, from the held keys.
    pub fn step(&self, dt: f64) -> (f64, f64, f64) {
        let mut x = 0.0f64;
        let mut y = 0.0f64;
        if self.left {
            x += 1.0;
        }
        if self.right {
            x -= 1.0;
        }
        if self.up {
            y -= 1.0;
        }
        if self.down {
            y += 1.0;
        }
        let len = (x * x + y * y).sqrt();
        // Diagonals are not faster.
        let v = if len > 0.0 {
            PAN_SPEED * self.view_size.0 * dt / len
        } else {
            0.0
        };
        let z = if self.zoom {
            ((if self.shift { -1.0 } else { 1.0 }) * self.zoom_speed * dt).exp()
        } else {
            1.0
        };
        (x * v, y * v, z)
    }

    /// Integrates held keys over the time since the last frame (at most 0.1 s, 1/60 s on the
    /// first) and moves `viewport`. Returns true while keys keep the view moving (the host
    /// keeps its display link running).
    pub fn advance(&mut self, viewport: &mut Viewport, now: f64) -> bool {
        let dt = match self.last_tick {
            Some(t) => (now - t).min(0.1),
            None => 1.0 / 60.0,
        };
        self.last_tick = Some(now);
        if !self.is_moving() {
            return false;
        }
        let (dx, dy, z) = self.step(dt);
        self.note_interaction(now);
        if dx != 0.0 || dy != 0.0 {
            viewport.pan_by_points(dx, dy);
        }
        if z != 1.0 {
            let (tx, ty) = self.zoom_target();
            viewport.zoom_by(z, tx, ty);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_integrate_over_time() {
        let mut n = Navigator::default();
        let mut v = Viewport::new(0.0, 0.0, 1e-3);
        n.handle(
            InputEvent::Resize {
                width: 1000.0,
                height: 600.0,
            },
            &mut v,
            0.0,
        );
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 0.0);
        assert!(n.advance(&mut v, 0.5));
        // 0.4 view widths per second, at most 0.1 s per frame
        assert!((v.center.re() - 0.4 * 1000.0 * 0.1 * 1e-3).abs() < 1e-12);
        n.handle(InputEvent::KeyUp(NavKey::Right), &mut v, 0.5);
        assert!(!n.advance(&mut v, 0.51));
        assert!(n.is_interacting(0.6));
        assert!(!n.is_interacting(0.7));
    }

    #[test]
    fn space_zooms_toward_pointer_and_speed_keys() {
        let mut n = Navigator::default();
        let mut v = Viewport::new(0.0, 0.0, 1e-3);
        n.handle(
            InputEvent::Resize {
                width: 1000.0,
                height: 600.0,
            },
            &mut v,
            0.0,
        );
        n.handle(InputEvent::Pointer(Some((100.0, 50.0))), &mut v, 0.0);
        let target = v.point_at_offset(100.0, 50.0);
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, 0.0);
        n.advance(&mut v, 0.05);
        assert!((v.scale - 1e-3 / (1.6f64 * 0.05).exp()).abs() < 1e-15);
        let (dr, di) = v.point_at_offset(100.0, 50.0).delta_from(&target);
        assert!(dr.abs() < 1e-15 && di.abs() < 1e-15);
        n.handle(InputEvent::KeyDown(NavKey::FasterZoom), &mut v, 0.1);
        assert!((n.zoom_speed() - 2.0).abs() < 1e-12);
        n.handle(InputEvent::FocusLost, &mut v, 0.2);
        assert!(!n.is_moving());
        // pointer outside the view: zoom around the centre
        n.handle(InputEvent::Pointer(Some((900.0, 0.0))), &mut v, 0.2);
        assert_eq!(n.zoom_target(), (0.0, 0.0));
    }
}
