//! Platform-neutral navigation: held keys, the zoom target, pointer and keyboard modes, the
//! keyboard crosshair's glide, edge panning, pointer and gesture deltas, integrated per
//! display frame from timestamps (kb: decisions/controls, revision "pointer and keyboard
//! modes"). Every UI forwards its decoded events here and calls [`Navigator::advance`] once
//! per frame; nothing about motion lives in the platform layer. The platform only presents
//! what [`Navigator::mode`] and [`Navigator::crosshair`] say (cursor shape, overlay).
//!
//! Coordinates are view points (not pixels) from the view centre, y up.
//!
//! **Zoom target.** Every zoom (held Space, pinch, scroll and wheel zoom, double-click, the
//! `+` / `−` keys) zooms towards one point, and the complex coordinate under it stays fixed
//! on screen. In pointer mode it is the pointer (or the event's own position for gestures);
//! in keyboard mode the overlay crosshair, which glides to the view centre on a critically
//! damped spring.

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
    /// + / =: one step in (x1.5) at the zoom target.
    StepIn,
    /// - / _: one step out.
    StepOut,
    /// R: back to the home view fitted to the window.
    Reset,
}

impl NavKey {
    /// W A S D / arrows: the keys that switch to keyboard mode.
    pub fn is_direction(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::Left | Self::Right)
    }
}

/// Which input was used last; decides the zoom target and how it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Mouse or trackpad pointer: zoom target = pointer, shown as the system crosshair cursor.
    #[default]
    Pointer,
    /// W A S D / arrows: zoom target = an overlay crosshair gliding to the view centre; the
    /// system cursor is hidden.
    Keyboard,
}

/// When the view pans towards an edge the pointer is near (pointer mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EdgePan {
    Off,
    /// Only while a continuous zoom (Space) runs.
    #[default]
    WhileZooming,
    Always,
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
    /// Parts of the view covered by window chrome (title bar, toolbar, notch), in points.
    /// The canvas is the rest: the pointer counts as over the canvas only there, and edge
    /// panning measures its band from these edges.
    Insets {
        top: f64,
        left: f64,
        bottom: f64,
        right: f64,
    },
    /// Pointer position (mouse, trackpad, iPad pointer); `None` when it left the canvas.
    Pointer(Option<(f64, f64)>),
    /// Content follows a drag / trackpad scroll / one-finger pan by (dx, dy) points.
    Pan {
        dx: f64,
        dy: f64,
    },
    /// A mouse wheel notch or precise zoom scroll: zoom by 1.15^delta at the zoom target
    /// ((x, y), the pointer, in pointer mode).
    WheelZoom {
        delta: f64,
        x: f64,
        y: f64,
    },
    /// Trackpad pinch (macOS magnify): zoom by 1 + magnification at the zoom target.
    Magnify {
        magnification: f64,
        x: f64,
        y: f64,
    },
    /// Touch pinch: scale ratio since the last event, and the pinch centre's movement. The
    /// pinch centre is the zoom target (touch is pointer use).
    Pinch {
        ratio: f64,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    },
    /// Double click / tap: zoom x2 (or out x0.5) at (x, y); a click is pointer use.
    DoubleTap {
        x: f64,
        y: f64,
        out: bool,
    },
    /// Zoom by `factor` at the zoom target (menu commands).
    ZoomBy {
        factor: f64,
    },
}

/// Pan speed of the direction keys, in view widths per second.
pub const PAN_SPEED: f64 = 0.4;
/// Zoom speed range, e-folds of magnification per second.
pub const ZOOM_SPEED_RANGE: (f64, f64) = (0.1, 8.0);
/// Input younger than this counts as an ongoing gesture (the renderer shows previews).
pub const IDLE_DELAY: f64 = 0.15;
/// Pointer travel (points, from where it was when keyboard mode began) that switches back
/// to pointer mode; smaller moves (a nudge of the desk) do not.
pub const POINTER_MODE_THRESHOLD: f64 = 6.0;
/// Natural angular frequency (1/s) of the crosshair's critically damped glide. From rest the
/// offset is `x0 (1 + ωt) e^(−ωt)`: 10 % left after 156 ms, 1 % after 265 ms, never past
/// the centre.
pub const GLIDE_OMEGA: f64 = 25.0;
/// Below this offset (points) and speed (points/s) the glide snaps onto the centre.
const GLIDE_SNAP: (f64, f64) = (0.02, 0.5);
/// Edge band width, as a fraction of the canvas's smaller dimension.
pub const EDGE_BAND: f64 = 0.08;

/// Held keys, modes and gesture state. Owned by the renderer host; see the module docs.
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
    /// top, left, bottom, right
    insets: (f64, f64, f64, f64),
    /// Continuous zoom speed (Space), e-folds per second; Q / E change it.
    zoom_speed: f64,
    /// When the zoom speed last changed (for a HUD highlight).
    pub zoom_speed_changed: f64,
    last_tick: Option<f64>,
    /// Time of the last input that moved the view.
    pub last_interaction: f64,
    mode: InputMode,
    /// Where the pointer was when keyboard mode began (or first seen after); travel from
    /// here past [`POINTER_MODE_THRESHOLD`] switches to pointer mode.
    anchor: Option<(f64, f64)>,
    /// Keyboard crosshair position and velocity (points, points/s).
    crosshair: (f64, f64),
    crosshair_velocity: (f64, f64),
    edge_pan: EdgePan,
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
            insets: (0.0, 0.0, 0.0, 0.0),
            zoom_speed: 1.6,
            zoom_speed_changed: f64::NEG_INFINITY,
            last_tick: None,
            last_interaction: f64::NEG_INFINITY,
            mode: InputMode::Pointer,
            anchor: None,
            crosshair: (0.0, 0.0),
            crosshair_velocity: (0.0, 0.0),
            edge_pan: EdgePan::WhileZooming,
        }
    }
}

/// One step of a critically damped spring towards 0, exact for any `dt` (so the glide is
/// the same at every frame rate): with c = v + ωx, x(t) = (x + ct)e^(−ωt) and
/// v(t) = (v − ωct)e^(−ωt).
pub fn spring_step(x: f64, v: f64, omega: f64, dt: f64) -> (f64, f64) {
    let e = (-omega * dt).exp();
    let c = v + omega * x;
    ((x + c * dt) * e, (v - omega * c * dt) * e)
}

/// Smooth 0 → 1 ramp (zero slope at both ends).
fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Navigator {
    pub fn zoom_speed(&self) -> f64 {
        self.zoom_speed
    }

    pub fn set_zoom_speed(&mut self, v: f64, now: f64) {
        self.zoom_speed = v.clamp(ZOOM_SPEED_RANGE.0, ZOOM_SPEED_RANGE.1);
        self.zoom_speed_changed = now;
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn edge_pan(&self) -> EdgePan {
        self.edge_pan
    }

    pub fn set_edge_pan(&mut self, e: EdgePan) {
        self.edge_pan = e;
    }

    /// The overlay crosshair (keyboard mode only), view points from the centre, y up.
    pub fn crosshair(&self) -> Option<(f64, f64)> {
        (self.mode == InputMode::Keyboard).then_some(self.crosshair)
    }

    /// The crosshair is still gliding to the centre.
    pub fn is_gliding(&self) -> bool {
        self.mode == InputMode::Keyboard && (self.crosshair != (0.0, 0.0) || self.crosshair_velocity != (0.0, 0.0))
    }

    fn keys_held(&self) -> bool {
        self.up || self.down || self.left || self.right || self.zoom
    }

    /// Something moves the view each frame: held keys or edge panning.
    pub fn is_moving(&self) -> bool {
        self.keys_held() || self.edge_vector() != (0.0, 0.0)
    }

    /// Frames are needed: the view moves or the crosshair glides.
    pub fn is_animating(&self) -> bool {
        self.is_moving() || self.is_gliding()
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

    /// The canvas (view minus insets): x0, x1, y0, y1 in view points from the centre.
    fn canvas(&self) -> (f64, f64, f64, f64) {
        let (w, h) = self.view_size;
        let (top, left, bottom, right) = self.insets;
        (-w / 2.0 + left, w / 2.0 - right, -h / 2.0 + bottom, h / 2.0 - top)
    }

    /// The pointer, if it is over the canvas.
    fn pointer_on_canvas(&self) -> Option<(f64, f64)> {
        let (x0, x1, y0, y1) = self.canvas();
        self.pointer.filter(|&(x, y)| x >= x0 && x <= x1 && y >= y0 && y <= y1)
    }

    fn enter_keyboard_mode(&mut self) {
        if self.mode == InputMode::Keyboard {
            return;
        }
        // The crosshair starts where the zoom target was and glides to the centre.
        self.crosshair = self.zoom_target();
        self.crosshair_velocity = (0.0, 0.0);
        self.mode = InputMode::Keyboard;
        self.anchor = self.pointer;
    }

    fn enter_pointer_mode(&mut self) {
        self.mode = InputMode::Pointer;
        self.anchor = None;
        self.crosshair = (0.0, 0.0);
        self.crosshair_velocity = (0.0, 0.0);
    }

    /// Applies one event. Returns true if the event was ours (the platform swallows it).
    pub fn handle(&mut self, event: InputEvent, viewport: &mut Viewport, now: f64) -> bool {
        let was_animating = self.is_animating();
        let handled = self.apply(event, viewport, now);
        if !was_animating && self.is_animating() {
            // Motion starts now: the first frame integrates from this event, not from the
            // last frame before the display link paused (that was up to 0.1 s of jump).
            self.last_tick = Some(now);
        }
        handled
    }

    fn apply(&mut self, event: InputEvent, viewport: &mut Viewport, now: f64) -> bool {
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
            InputEvent::Insets {
                top,
                left,
                bottom,
                right,
            } => {
                let c = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
                self.insets = (c(top), c(left), c(bottom), c(right));
                true
            }
            InputEvent::Pointer(p) => {
                self.pointer(p);
                true
            }
            InputEvent::Pan { dx, dy } => {
                self.note_interaction(now);
                viewport.pan_by_points(dx, dy);
                true
            }
            InputEvent::WheelZoom { delta, x, y } => {
                let (tx, ty) = self.gesture_target(x, y);
                self.zoom_at(viewport, 1.15f64.powf(delta), tx, ty, now);
                true
            }
            InputEvent::Magnify { magnification, x, y } => {
                let (tx, ty) = self.gesture_target(x, y);
                self.zoom_at(viewport, 1.0 + magnification, tx, ty, now);
                true
            }
            InputEvent::Pinch { ratio, x, y, dx, dy } => {
                self.enter_pointer_mode();
                self.note_interaction(now);
                // Two-finger drag pans while pinching.
                viewport.pan_by_points(dx, dy);
                viewport.zoom_by(ratio, x, y);
                true
            }
            InputEvent::DoubleTap { x, y, out } => {
                self.enter_pointer_mode();
                self.zoom_at(viewport, if out { 0.5 } else { 2.0 }, x, y, now);
                true
            }
            InputEvent::ZoomBy { factor } => {
                let (tx, ty) = self.zoom_target();
                self.zoom_at(viewport, factor, tx, ty, now);
                true
            }
        }
    }

    fn pointer(&mut self, p: Option<(f64, f64)>) {
        match p {
            Some(pos) if self.mode == InputMode::Keyboard => match self.anchor {
                Some(a) if ((pos.0 - a.0).powi(2) + (pos.1 - a.1).powi(2)).sqrt() > POINTER_MODE_THRESHOLD => {
                    self.enter_pointer_mode();
                }
                Some(_) => {}
                None => self.anchor = Some(pos),
            },
            Some(_) => {}
            // Re-entering the canvas measures travel from where it comes back in.
            None => self.anchor = None,
        }
        self.pointer = p;
    }

    /// Target of a zoom gesture reported at (x, y): the event position in pointer mode, the
    /// crosshair in keyboard mode (a trackpad pinch does not move the pointer).
    fn gesture_target(&self, x: f64, y: f64) -> (f64, f64) {
        match self.mode {
            InputMode::Pointer => (x, y),
            InputMode::Keyboard => self.crosshair,
        }
    }

    fn zoom_at(&mut self, viewport: &mut Viewport, factor: f64, x: f64, y: f64, now: f64) {
        self.note_interaction(now);
        viewport.zoom_by(factor, x, y);
    }

    fn key(&mut self, k: NavKey, down: bool, viewport: &mut Viewport, now: f64) -> bool {
        if down && k.is_direction() {
            self.enter_keyboard_mode();
        }
        match k {
            NavKey::Up => self.up = down,
            NavKey::Down => self.down = down,
            NavKey::Left => self.left = down,
            NavKey::Right => self.right = down,
            NavKey::Zoom => self.zoom = down,
            NavKey::SlowerZoom if down => self.set_zoom_speed(self.zoom_speed * 0.8, now),
            NavKey::FasterZoom if down => self.set_zoom_speed(self.zoom_speed * 1.25, now),
            NavKey::StepIn | NavKey::StepOut if down => {
                let (tx, ty) = self.zoom_target();
                let f = if k == NavKey::StepIn { 1.5 } else { 1.0 / 1.5 };
                self.zoom_at(viewport, f, tx, ty, now);
            }
            NavKey::Reset if down => {
                *viewport = Viewport::fitting(self.view_size.0, self.view_size.1);
            }
            _ => {}
        }
        true
    }

    /// The zoom target: in keyboard mode the crosshair; in pointer mode the pointer while it
    /// is over the canvas, else the centre.
    pub fn zoom_target(&self) -> (f64, f64) {
        match self.mode {
            InputMode::Keyboard => self.crosshair,
            InputMode::Pointer => self.pointer_on_canvas().unwrap_or((0.0, 0.0)),
        }
    }

    /// Edge-pan direction and strength (each component in −1...1, length at most 1; +x: pan
    /// towards the right edge). Zero unless pointer mode, the setting allows it now, and
    /// the pointer is inside the band along an edge of the canvas.
    pub fn edge_vector(&self) -> (f64, f64) {
        let active = match self.edge_pan {
            EdgePan::Off => false,
            EdgePan::WhileZooming => self.zoom,
            EdgePan::Always => true,
        };
        if !active || self.mode != InputMode::Pointer {
            return (0.0, 0.0);
        }
        let Some((x, y)) = self.pointer_on_canvas() else {
            return (0.0, 0.0);
        };
        let (x0, x1, y0, y1) = self.canvas();
        let band = EDGE_BAND * (x1 - x0).min(y1 - y0);
        if band <= 0.0 {
            return (0.0, 0.0);
        }
        // 0 at the band's inner border, 1 at the edge.
        let ramp = |d: f64| smoothstep(1.0 - d / band);
        let ex = ramp(x1 - x) - ramp(x - x0);
        let ey = ramp(y1 - y) - ramp(y - y0);
        let len = (ex * ex + ey * ey).sqrt();
        // Diagonal in corners, never faster than the keys.
        if len > 1.0 { (ex / len, ey / len) } else { (ex, ey) }
    }

    /// Pan in view points (content follows the camera) and the zoom factor for a frame of
    /// `dt` seconds, from the held keys and edge panning.
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
        if len > 0.0 {
            x /= len;
            y /= len;
        }
        // Panning towards an edge moves the content away from it.
        let (ex, ey) = self.edge_vector();
        x -= ex;
        y -= ey;
        let v = PAN_SPEED * self.view_size.0 * dt;
        let z = if self.zoom {
            ((if self.shift { -1.0 } else { 1.0 }) * self.zoom_speed * dt).exp()
        } else {
            1.0
        };
        (x * v, y * v, z)
    }

    /// Advances the crosshair's glide by `dt` seconds.
    fn glide(&mut self, dt: f64) {
        if !self.is_gliding() {
            return;
        }
        let (x, vx) = spring_step(self.crosshair.0, self.crosshair_velocity.0, GLIDE_OMEGA, dt);
        let (y, vy) = spring_step(self.crosshair.1, self.crosshair_velocity.1, GLIDE_OMEGA, dt);
        if x.hypot(y) < GLIDE_SNAP.0 && vx.hypot(vy) < GLIDE_SNAP.1 {
            self.crosshair = (0.0, 0.0);
            self.crosshair_velocity = (0.0, 0.0);
        } else {
            self.crosshair = (x, y);
            self.crosshair_velocity = (vx, vy);
        }
    }

    /// Integrates held keys, edge panning and the crosshair glide over the time since the
    /// last frame (at most 0.1 s, 1/60 s on the first) and moves `viewport`. Returns true
    /// while frames are needed (the host keeps its display link running).
    pub fn advance(&mut self, viewport: &mut Viewport, now: f64) -> bool {
        let dt = match self.last_tick {
            // A frame may report a time before the event that started the motion.
            Some(t) => (now - t).clamp(0.0, 0.1),
            None => 1.0 / 60.0,
        };
        self.last_tick = Some(now);
        let gliding = self.is_gliding();
        // The zoom follows the crosshair while it glides: the target is where it is drawn.
        self.glide(dt);
        if !self.is_moving() {
            return gliding;
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

    fn setup() -> (Navigator, Viewport) {
        let mut n = Navigator::default();
        let mut v = Viewport::new(-0.5, 0.25, 1e-3);
        n.handle(
            InputEvent::Resize {
                width: 1000.0,
                height: 600.0,
            },
            &mut v,
            0.0,
        );
        (n, v)
    }

    fn dist(a: (f64, f64)) -> f64 {
        a.0.hypot(a.1)
    }

    #[test]
    fn keys_integrate_over_time() {
        let (mut n, mut v) = setup();
        let re0 = v.center.re();
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 0.0);
        assert!(n.advance(&mut v, 0.5));
        // 0.4 view widths per second, at most 0.1 s per frame
        assert!((v.center.re() - re0 - 0.4 * 1000.0 * 0.1 * 1e-3).abs() < 1e-12);
        n.handle(InputEvent::KeyUp(NavKey::Right), &mut v, 0.5);
        assert!(!n.advance(&mut v, 0.51));
        assert!(n.is_interacting(0.6));
        assert!(!n.is_interacting(0.7));
    }

    #[test]
    fn motion_from_idle_integrates_from_the_key_press() {
        // The display link paused long ago; the first frame after a key press moves the view
        // by the time since the press, not by the 0.1 s cap.
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        assert!(!n.advance(&mut v, 0.01));
        let re0 = v.center.re();
        n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 5.0);
        n.advance(&mut v, 5.008);
        let moved = (v.center.re() - re0) / (PAN_SPEED * 1000.0 * 1e-3);
        assert!((moved - 0.008).abs() < 1e-9, "{moved} s of motion");
    }

    #[test]
    fn space_zooms_toward_pointer_and_speed_keys() {
        let (mut n, mut v) = setup();
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

    #[test]
    fn modes_switch_by_last_input() {
        let (mut n, mut v) = setup();
        assert_eq!(n.mode(), InputMode::Pointer);
        n.handle(InputEvent::Pointer(Some((200.0, -100.0))), &mut v, 0.0);
        // Q / E, Space, +/- and Shift do not switch.
        for k in [NavKey::SlowerZoom, NavKey::FasterZoom, NavKey::Zoom, NavKey::StepIn] {
            n.handle(InputEvent::KeyDown(k), &mut v, 0.0);
            n.handle(InputEvent::KeyUp(k), &mut v, 0.0);
            assert_eq!(n.mode(), InputMode::Pointer, "{k:?}");
        }
        assert_eq!(n.crosshair(), None);
        // Every direction key does; the crosshair starts at the pointer.
        for k in [NavKey::Up, NavKey::Down, NavKey::Left, NavKey::Right] {
            let mut m = n.clone();
            m.handle(InputEvent::KeyDown(k), &mut v, 0.0);
            assert_eq!(m.mode(), InputMode::Keyboard, "{k:?}");
            assert_eq!(m.crosshair(), Some((200.0, -100.0)));
        }
        n.handle(InputEvent::KeyDown(NavKey::Left), &mut v, 0.0);
        n.handle(InputEvent::KeyUp(NavKey::Left), &mut v, 0.0);
        // A nudge below the threshold keeps keyboard mode ...
        n.handle(InputEvent::Pointer(Some((203.0, -97.0))), &mut v, 0.1);
        assert_eq!(n.mode(), InputMode::Keyboard);
        // ... more travel switches back; the zoom target is the pointer again.
        n.handle(InputEvent::Pointer(Some((210.0, -100.0))), &mut v, 0.2);
        assert_eq!(n.mode(), InputMode::Pointer);
        assert_eq!(n.zoom_target(), (210.0, -100.0));
        // Leaving the canvas and coming back in elsewhere is not travel by itself.
        n.handle(InputEvent::KeyDown(NavKey::Up), &mut v, 0.3);
        n.handle(InputEvent::Pointer(None), &mut v, 0.3);
        n.handle(InputEvent::Pointer(Some((-400.0, 0.0))), &mut v, 0.4);
        assert_eq!(n.mode(), InputMode::Keyboard);
        // A double-click is pointer use.
        n.handle(
            InputEvent::DoubleTap {
                x: 10.0,
                y: 10.0,
                out: false,
            },
            &mut v,
            0.5,
        );
        assert_eq!(n.mode(), InputMode::Pointer);
    }

    #[test]
    fn glide_converges_without_overshoot() {
        let (mut n, mut v) = setup();
        n.handle(InputEvent::Pointer(Some((400.0, -250.0))), &mut v, 0.0);
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Up), &mut v, 0.0);
        n.handle(InputEvent::KeyUp(NavKey::Up), &mut v, 0.0);
        let mut last = n.crosshair().unwrap();
        let mut t = 0.0;
        let mut at_1pct = None;
        while n.is_gliding() {
            t += 1.0 / 60.0;
            assert!(n.advance(&mut v, t), "frames while gliding");
            let c = n.crosshair().unwrap();
            // Monotonic, same side of the centre on both axes: no overshoot.
            assert!(c.0 >= 0.0 && c.1 <= 0.0, "overshoot at {t}: {c:?}");
            assert!(dist(c) <= dist(last));
            if at_1pct.is_none() && dist(c) < 0.01 * dist((400.0, -250.0)) {
                at_1pct = Some(t);
            }
            last = c;
            assert!(t < 2.0, "glide never settled");
        }
        assert_eq!(n.crosshair(), Some((0.0, 0.0)));
        let t1 = at_1pct.unwrap();
        assert!((0.2..=0.3).contains(&t1), "1 % after {t1} s");
        assert!(!n.advance(&mut v, t + 0.1), "idle after the glide");
    }

    #[test]
    fn glide_is_frame_rate_independent() {
        // Positions at the same instants agree at 30, 60 and 120 Hz.
        let sample = |hz: f64| {
            let (mut n, mut v) = setup();
            n.handle(InputEvent::Pointer(Some((300.0, 120.0))), &mut v, 0.0);
            n.advance(&mut v, 0.0);
            n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 0.0);
            n.handle(InputEvent::KeyUp(NavKey::Right), &mut v, 0.0);
            let steps = (0.3 * hz).round() as usize;
            let mut out = Vec::new();
            for i in 1..=steps {
                n.advance(&mut v, i as f64 / hz);
                if i % (hz / 10.0).round() as usize == 0 {
                    out.push(n.crosshair().unwrap());
                }
            }
            out
        };
        let (a, b, c) = (sample(30.0), sample(60.0), sample(120.0));
        assert_eq!(a.len(), 3);
        for i in 0..a.len() {
            for other in [b[i], c[i]] {
                assert!((a[i].0 - other.0).abs() < 1e-9 && (a[i].1 - other.1).abs() < 1e-9);
            }
        }
        // And it matches the closed form x0 (1 + ωt) e^(−ωt).
        let t = 0.1;
        let expect = 300.0 * (1.0 + GLIDE_OMEGA * t) * (-GLIDE_OMEGA * t).exp();
        assert!((a[0].0 - expect).abs() < 1e-9);
    }

    #[test]
    fn zoom_keeps_the_target_fixed() {
        // Pointer mode: every zoom input keeps the point under the pointer.
        let (mut n, mut v) = setup();
        let p = (-321.0, 187.0);
        n.handle(InputEvent::Pointer(Some(p)), &mut v, 0.0);
        let target = v.point_at_offset(p.0, p.1);
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, 0.0);
        let mut t = 0.0;
        for _ in 0..120 {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        n.handle(InputEvent::KeyUp(NavKey::Zoom), &mut v, t);
        for e in [
            InputEvent::KeyDown(NavKey::StepIn),
            InputEvent::WheelZoom {
                delta: 2.0,
                x: p.0,
                y: p.1,
            },
            InputEvent::Magnify {
                magnification: 0.3,
                x: p.0,
                y: p.1,
            },
            InputEvent::ZoomBy { factor: 0.5 },
        ] {
            n.handle(e, &mut v, t);
        }
        assert!(v.scale < 1e-4);
        let (dr, di) = v.point_at_offset(p.0, p.1).delta_from(&target);
        assert!(dr.hypot(di) / v.scale < 1e-6, "drift {} points", dr.hypot(di) / v.scale);

        // Keyboard mode after the glide: the centre is the target, for a trackpad pinch too
        // (its event position is wherever the hidden pointer is).
        n.handle(InputEvent::KeyDown(NavKey::Down), &mut v, t);
        n.handle(InputEvent::KeyUp(NavKey::Down), &mut v, t);
        while n.is_gliding() {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        let centre = v.point_at_offset(0.0, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, t);
        for _ in 0..60 {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        n.handle(
            InputEvent::Magnify {
                magnification: 0.5,
                x: p.0,
                y: p.1,
            },
            &mut v,
            t,
        );
        let (dr, di) = v.point_at_offset(0.0, 0.0).delta_from(&centre);
        assert!(dr.hypot(di) / v.scale < 1e-6);
    }

    #[test]
    fn edge_pan_ramps_from_the_band_border_to_key_speed() {
        let (mut n, mut v) = setup();
        let band = EDGE_BAND * 600.0;
        let speed_at = |n: &mut Navigator, v: &mut Viewport, x: f64, y: f64| {
            n.handle(InputEvent::Pointer(Some((x, y))), v, 0.0);
            let (dx, dy, _) = n.step(1.0);
            (dx / (PAN_SPEED * 1000.0), dy / (PAN_SPEED * 1000.0))
        };
        // Not zooming: no edge pan by default.
        assert_eq!(speed_at(&mut n, &mut v, 499.0, 0.0), (0.0, 0.0));
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, 0.0);
        // Inner border 0, edge = key speed, monotonic in between; content moves away from
        // the edge.
        assert_eq!(speed_at(&mut n, &mut v, 500.0 - band, 0.0), (0.0, 0.0));
        assert!((speed_at(&mut n, &mut v, 500.0, 0.0).0 + 1.0).abs() < 1e-12);
        let mut last = 0.0;
        for i in 1..=20 {
            let s = -speed_at(&mut n, &mut v, 500.0 - band + band * i as f64 / 20.0, 0.0).0;
            assert!(s > last && s <= 1.0);
            last = s;
        }
        // Halfway: smoothstep(0.5) = 0.5.
        assert!((speed_at(&mut n, &mut v, 500.0 - band / 2.0, 0.0).0 + 0.5).abs() < 1e-12);
        // Corners pan diagonally at most at key speed; the top edge moves content down.
        let (cx, cy) = speed_at(&mut n, &mut v, -500.0, 300.0);
        assert!((cx - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
        assert!((cy + std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
        // Insets move the edges in: a pointer under the title bar is off the canvas.
        n.handle(
            InputEvent::Insets {
                top: 50.0,
                left: 0.0,
                bottom: 0.0,
                right: 0.0,
            },
            &mut v,
            0.0,
        );
        assert_eq!(speed_at(&mut n, &mut v, 0.0, 280.0), (0.0, 0.0));
        assert_eq!(n.zoom_target(), (0.0, 0.0));
        assert!((speed_at(&mut n, &mut v, 0.0, 250.0).1 + 1.0).abs() < 1e-12);
        // Pointer gone (left the window), keyboard mode, or the setting off: no pan.
        assert!(n.handle(InputEvent::Pointer(None), &mut v, 0.0));
        assert_eq!(n.step(1.0).0, 0.0);
        n.handle(InputEvent::Pointer(Some((499.0, 0.0))), &mut v, 0.0);
        n.set_edge_pan(EdgePan::Off);
        assert_eq!(n.edge_vector(), (0.0, 0.0));
        n.set_edge_pan(EdgePan::Always);
        n.handle(InputEvent::KeyUp(NavKey::Zoom), &mut v, 0.0);
        assert!(n.edge_vector().0 > 0.9);
        assert!(n.is_moving());
        n.handle(InputEvent::KeyDown(NavKey::Left), &mut v, 0.0);
        n.handle(InputEvent::KeyUp(NavKey::Left), &mut v, 0.0);
        assert_eq!(n.edge_vector(), (0.0, 0.0));
    }

    #[test]
    fn edge_pan_moves_the_view_while_zooming() {
        let (mut n, mut v) = setup();
        n.handle(InputEvent::Pointer(Some((-499.0, 0.0))), &mut v, 0.0);
        n.advance(&mut v, 0.0);
        let re0 = v.center.re();
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, 0.0);
        let mut t = 0.0;
        for _ in 0..30 {
            t += 1.0 / 60.0;
            assert!(n.advance(&mut v, t));
        }
        assert!(v.center.re() < re0, "pans towards the left edge");
    }
}
