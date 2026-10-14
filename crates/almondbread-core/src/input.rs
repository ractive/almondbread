//! Platform-neutral navigation: held keys, the zoom target, pointer and keyboard modes, the
//! keyboard crosshair's glide, edge panning, pointer and gesture deltas, integrated per
//! display frame from timestamps (kb: decisions/controls, revision "pointer and keyboard
//! modes"). Every UI forwards its decoded events here and calls [`Navigator::advance`] once
//! per frame; nothing about motion lives in the platform layer. The platform only presents
//! what [`Navigator::mode`] and [`Navigator::crosshair`] say (cursor shape, overlay).
//!
//! Coordinates are view points (not pixels) from the view centre, y up.
//!
//! **Motion feel (FL).** Held keys, edge panning and the gamepad set a *target* velocity
//! (pan in points per second, zoom in e-folds per second); the actual velocity follows it
//! on a critically damped spring (faster when speeding up than when slowing down), so
//! motion eases in and glides to a stop instead of starting and stopping hard. Each frame
//! integrates the spring in closed form, including the exact distance travelled, so motion
//! is identical at 30, 60 and 120 Hz. Direct manipulation stays 1:1 (drag, trackpad scroll,
//! pinch); a drag or a touch pan or pinch released while moving flings on with exponential
//! decay (the macOS trackpad's own momentum events arrive as ordinary pans). Discrete zoom
//! steps (wheel notch, double click, + / −, menu) are animated with a short exponential
//! ease-out at the zoom target. kb: decisions/controls (motion feel).
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
    /// Zoom by `factor` at the zoom target (menu commands), animated.
    ZoomBy {
        factor: f64,
    },
    /// A mouse wheel notch (a discrete step, not a precise scroll): zoom by 1.15^delta at
    /// (x, y) (pointer mode) or the crosshair, animated over ~0.1 s.
    WheelNotch {
        delta: f64,
        x: f64,
        y: f64,
    },
    /// The finger or button that drove `Pan` / `Pinch` / `Magnify` lifted: if the gesture
    /// was still moving, the view flings on and decays. Not sent for trackpad scrolling,
    /// whose momentum the OS delivers as further `Pan` events.
    GestureEnd,
    /// Game controller state (absolute, sent when it changes): left stick pans, right stick
    /// y and the triggers zoom (right in, left out), `boost` speeds both up. Raw values
    /// (sticks −1...1, y up; triggers 0...1); dead zones and response curves are applied
    /// here. Deflection switches to keyboard mode (the crosshair is the zoom target).
    Gamepad {
        lx: f64,
        ly: f64,
        rx: f64,
        ry: f64,
        lt: f64,
        rt: f64,
        boost: bool,
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
/// Spring frequencies (1/s) of the pan velocity towards its target: speeding up (from rest,
/// 50 % of the key speed after 56 ms, 90 % after 130 ms) and slowing down (10 % left after
/// 216 ms; a released key glides on 2/ω = 0.11 s worth of its speed, 4.4 % of the view
/// width at the key speed).
pub const PAN_OMEGA: (f64, f64) = (30.0, 18.0);
/// The same for the continuous zoom rate (Space, triggers): a released Space zooms on by
/// 2/ω · rate (0.18 e-folds, ×1.19, at the default 1.6 e-folds per second).
pub const ZOOM_OMEGA: (f64, f64) = (25.0, 18.0);
/// Decay time (s) of a pan fling after a drag or touch pan is released while moving, its
/// minimum start speed and the speed it stops at (points per second).
pub const FLING_TAU: f64 = 0.33;
const FLING_START: f64 = 60.0;
const FLING_STOP: f64 = 6.0;
/// The same for a zoom fling after a pinch (e-folds per second).
pub const ZOOM_FLING_TAU: f64 = 0.25;
const ZOOM_FLING_START: f64 = 0.4;
const ZOOM_FLING_STOP: f64 = 0.02;
/// Gesture samples older than this at release do not count towards the fling velocity, and
/// a release this long after the last movement does not fling at all.
const FLING_WINDOW: f64 = 0.1;
const FLING_STALE: f64 = 0.06;
/// Time constant (s) of animated zoom steps (wheel notch, double click, + / −, menu): 63 %
/// of the step after 60 ms, 99 % after 0.28 s.
pub const STEP_TAU: f64 = 0.06;
/// Gamepad: radial stick dead zone, trigger dead zone, and the boost button's speed factor.
pub const STICK_DEAD_ZONE: f64 = 0.15;
pub const TRIGGER_DEAD_ZONE: f64 = 0.06;
pub const BOOST: f64 = 2.5;
/// Below these the velocities snap to rest (points/s, e-folds/s).
const PAN_REST: f64 = 0.5;
const ZOOM_REST: f64 = 1e-3;

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
    /// Pan velocity (points/s, content motion) and its rate of change, following the target
    /// of keys, edge panning and the left stick.
    pan: [Spring; 2],
    /// Continuous zoom rate (e-folds/s) following Space and the gamepad.
    zoom_rate: Spring,
    /// Pan fling velocity (points/s), decaying with [`FLING_TAU`].
    fling: (f64, f64),
    /// Zoom fling rate (e-folds/s) and the point it zooms at.
    zoom_fling: f64,
    zoom_fling_at: (f64, f64),
    /// Recent direct-manipulation samples (time, dx, dy, ln zoom, x, y) for fling velocity.
    gesture: Vec<(f64, f64, f64, f64, f64, f64)>,
    /// Animated zoom steps still to apply (e-folds) and where.
    step_left: f64,
    step_at: Option<(f64, f64)>,
    pad: GamepadState,
}

/// A velocity following a target on a critically damped spring.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    /// Current value (a velocity).
    pub v: f64,
    /// Its rate of change.
    pub a: f64,
}

impl Spring {
    /// Moves `v` towards `target` over `dt` seconds with frequency `omega` (exact closed
    /// form) and returns the integral of `v` over the step (the distance travelled).
    pub fn follow(&mut self, target: f64, omega: f64, dt: f64) -> f64 {
        let x0 = self.v - target;
        let e = (-omega * dt).exp();
        let c = self.a + omega * x0;
        let x1 = (x0 + c * dt) * e;
        let a1 = (self.a - omega * c * dt) * e;
        let integral = target * dt + x0 * (1.0 - e) / omega + c * (1.0 - e - omega * dt * e) / (omega * omega);
        self.v = target + x1;
        self.a = a1;
        integral
    }

    fn at_rest(&self) -> bool {
        self.v == 0.0 && self.a == 0.0
    }
}

/// Gamepad axes after dead zones and response curves.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GamepadState {
    /// Pan direction and strength (length <= 1), x right, y up (camera motion).
    pan: (f64, f64),
    /// Zoom strength −1...1 (in > 0).
    zoom: f64,
    boost: bool,
}

/// Response curve of a deflection past its dead zone: fine control near the centre, full
/// speed at the stop (`0.25 m + 0.75 m³` of the rescaled magnitude).
pub fn response_curve(m: f64, dead_zone: f64) -> f64 {
    let m = ((m.abs() - dead_zone) / (1.0 - dead_zone)).clamp(0.0, 1.0);
    0.25 * m + 0.75 * m * m * m
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
            pan: [Spring::default(); 2],
            zoom_rate: Spring::default(),
            fling: (0.0, 0.0),
            zoom_fling: 0.0,
            zoom_fling_at: (0.0, 0.0),
            gesture: Vec::new(),
            step_left: 0.0,
            step_at: None,
            pad: GamepadState::default(),
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

    /// Something drives motion: held keys, edge panning or a deflected gamepad.
    fn driven(&self) -> bool {
        self.keys_held() || self.edge_vector() != (0.0, 0.0) || self.pad.pan != (0.0, 0.0) || self.pad.zoom != 0.0
    }

    /// Something moves the view each frame: a drive (keys, edge panning, gamepad), or motion
    /// still easing out (velocity glide, fling, animated zoom step).
    pub fn is_moving(&self) -> bool {
        self.driven()
            || !self.pan[0].at_rest()
            || !self.pan[1].at_rest()
            || !self.zoom_rate.at_rest()
            || self.fling != (0.0, 0.0)
            || self.zoom_fling != 0.0
            || self.step_left != 0.0
    }

    /// Current pan velocity (points/s) and zoom rate (e-folds/s), springs plus flings.
    pub fn velocity(&self) -> (f64, f64, f64) {
        (
            self.pan[0].v + self.fling.0,
            self.pan[1].v + self.fling.1,
            self.zoom_rate.v + self.zoom_fling,
        )
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
        self.pad = GamepadState::default();
    }

    /// Stops all motion at once (Reset; a new drag or pinch grabbing the view).
    fn stop_motion(&mut self) {
        self.pan = [Spring::default(); 2];
        self.zoom_rate = Spring::default();
        self.stop_flings();
        self.step_left = 0.0;
        self.step_at = None;
    }

    fn stop_flings(&mut self) {
        self.fling = (0.0, 0.0);
        self.zoom_fling = 0.0;
    }

    /// Records a direct-manipulation sample for the fling velocity at release.
    fn sample_gesture(&mut self, now: f64, dx: f64, dy: f64, ln_zoom: f64, at: (f64, f64)) {
        self.gesture.retain(|s| now - s.0 <= FLING_WINDOW);
        self.gesture.push((now, dx, dy, ln_zoom, at.0, at.1));
    }

    /// The gesture was released: fling with its recent velocity if it was still moving.
    fn release_gesture(&mut self, now: f64) {
        let samples = std::mem::take(&mut self.gesture);
        let (Some(first), Some(last)) = (samples.first(), samples.last()) else {
            return;
        };
        if now - last.0 > FLING_STALE || samples.len() < 2 {
            return;
        }
        // Velocity over the window: the first sample's movement happened before its time.
        let span = (last.0 - first.0).max(1e-3);
        let (mut dx, mut dy, mut dz) = (0.0, 0.0, 0.0);
        for s in &samples[1..] {
            dx += s.1;
            dy += s.2;
            dz += s.3;
        }
        let (vx, vy, vz) = (dx / span, dy / span, dz / span);
        if vx.hypot(vy) >= FLING_START {
            self.fling = (vx, vy);
        }
        if vz.abs() >= ZOOM_FLING_START {
            self.zoom_fling = vz;
            self.zoom_fling_at = (last.4, last.5);
        }
    }

    /// Queues an animated zoom step of `factor` at (x, y).
    fn step_zoom(&mut self, factor: f64, x: f64, y: f64, now: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        self.note_interaction(now);
        self.step_left += factor.ln();
        self.step_at = Some((x, y));
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
                // The hand has the view: flings stop; motion is 1:1.
                self.stop_flings();
                self.note_interaction(now);
                viewport.pan_by_points(dx, dy);
                self.sample_gesture(now, dx, dy, 0.0, (0.0, 0.0));
                true
            }
            InputEvent::WheelZoom { delta, x, y } => {
                let (tx, ty) = self.gesture_target(x, y);
                self.zoom_at(viewport, 1.15f64.powf(delta), tx, ty, now);
                true
            }
            InputEvent::WheelNotch { delta, x, y } => {
                let (tx, ty) = self.gesture_target(x, y);
                self.step_zoom(1.15f64.powf(delta), tx, ty, now);
                true
            }
            InputEvent::Magnify { magnification, x, y } => {
                self.stop_flings();
                let (tx, ty) = self.gesture_target(x, y);
                self.zoom_at(viewport, 1.0 + magnification, tx, ty, now);
                self.sample_gesture(now, 0.0, 0.0, (1.0 + magnification).ln(), (tx, ty));
                true
            }
            InputEvent::Pinch { ratio, x, y, dx, dy } => {
                self.enter_pointer_mode();
                self.stop_flings();
                self.note_interaction(now);
                // Two-finger drag pans while pinching.
                viewport.pan_by_points(dx, dy);
                viewport.zoom_by(ratio, x, y);
                self.sample_gesture(now, dx, dy, ratio.ln(), (x, y));
                true
            }
            InputEvent::GestureEnd => {
                self.release_gesture(now);
                true
            }
            InputEvent::DoubleTap { x, y, out } => {
                self.enter_pointer_mode();
                self.step_zoom(if out { 0.5 } else { 2.0 }, x, y, now);
                true
            }
            InputEvent::ZoomBy { factor } => {
                let (tx, ty) = self.zoom_target();
                self.step_zoom(factor, tx, ty, now);
                true
            }
            InputEvent::Gamepad {
                lx,
                ly,
                rx: _,
                ry,
                lt,
                rt,
                boost,
            } => {
                self.gamepad(lx, ly, ry, lt, rt, boost);
                true
            }
        }
    }

    /// Dead zones and response curves; deflection enters keyboard mode.
    fn gamepad(&mut self, lx: f64, ly: f64, ry: f64, lt: f64, rt: f64, boost: bool) {
        let finite = |v: f64| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
        let (lx, ly, ry) = (finite(lx), finite(ly), finite(ry));
        let (lt, rt) = (finite(lt).max(0.0), finite(rt).max(0.0));
        let m = lx.hypot(ly);
        let pan = if m > STICK_DEAD_ZONE {
            let s = response_curve(m.min(1.0), STICK_DEAD_ZONE) / m;
            (lx * s, ly * s)
        } else {
            (0.0, 0.0)
        };
        let stick_zoom = if ry.abs() > STICK_DEAD_ZONE {
            ry.signum() * response_curve(ry, STICK_DEAD_ZONE)
        } else {
            0.0
        };
        let zoom = (response_curve(rt, TRIGGER_DEAD_ZONE) - response_curve(lt, TRIGGER_DEAD_ZONE) + stick_zoom)
            .clamp(-1.0, 1.0);
        let next = GamepadState { pan, zoom, boost };
        if (next.pan != (0.0, 0.0) || next.zoom != 0.0) && self.pad.pan == (0.0, 0.0) && self.pad.zoom == 0.0 {
            self.enter_keyboard_mode();
        }
        self.pad = next;
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
            // A key takes over from a fling.
            self.fling = (0.0, 0.0);
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
                self.step_zoom(f, tx, ty, now);
            }
            NavKey::Reset if down => {
                self.stop_motion();
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

    /// Target velocities of the drives: pan in points per second (content motion, the camera
    /// moves the other way) and the zoom rate in e-folds per second, from held keys, edge
    /// panning and the gamepad. The actual velocity follows it (see [`Spring`]).
    pub fn target_velocity(&self) -> (f64, f64, f64) {
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
        // The stick moves the camera like the keys.
        let boost = if self.pad.boost { BOOST } else { 1.0 };
        x -= self.pad.pan.0 * boost;
        y -= self.pad.pan.1 * boost;
        // Keys plus stick never beyond the boosted key speed.
        let len = x.hypot(y);
        let cap = if self.pad.boost { BOOST } else { 1.0 };
        if len > cap {
            x *= cap / len;
            y *= cap / len;
        }
        let v = PAN_SPEED * self.view_size.0;
        let mut z = if self.zoom {
            if self.shift { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        z = (z + self.pad.zoom * boost).clamp(-cap, cap);
        (x * v, y * v, z * self.zoom_speed)
    }

    /// Pan in view points and the zoom factor for a frame of `dt` seconds at the target
    /// velocity (no easing; tests and tools that want the drives' raw speed).
    pub fn step(&self, dt: f64) -> (f64, f64, f64) {
        let (vx, vy, z) = self.target_velocity();
        (vx * dt, vy * dt, (z * dt).exp())
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

    /// Integrates the drives (keys, edge panning, gamepad) through the velocity springs,
    /// flings, animated zoom steps and the crosshair glide over the time since the last frame
    /// (at most 0.1 s, 1/60 s on the first) and moves `viewport`. Returns true while frames
    /// are needed (the host keeps its display link running).
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
        self.note_interaction(now);
        let (tx, ty, tz) = self.target_velocity();
        // Speeding up or slowing down (by magnitude): different spring frequencies.
        let speed = self.pan[0].v.hypot(self.pan[1].v);
        let omega = if tx.hypot(ty) > speed { PAN_OMEGA.0 } else { PAN_OMEGA.1 };
        let mut dx = self.pan[0].follow(tx, omega, dt);
        let mut dy = self.pan[1].follow(ty, omega, dt);
        let zomega = if tz.abs() > self.zoom_rate.v.abs() {
            ZOOM_OMEGA.0
        } else {
            ZOOM_OMEGA.1
        };
        let mut ln_zoom = self.zoom_rate.follow(tz, zomega, dt);
        let v = &mut self.pan;
        if tx == 0.0 && ty == 0.0 && v[0].v.hypot(v[1].v) < PAN_REST && v[0].a.hypot(v[1].a) < PAN_REST * PAN_OMEGA.1 {
            *v = [Spring::default(); 2];
        }
        let z = &mut self.zoom_rate;
        if tz == 0.0 && z.v.abs() < ZOOM_REST && z.a.abs() < ZOOM_REST * ZOOM_OMEGA.1 {
            *z = Spring::default();
        }

        // Flings: v(t) = v0 e^(−t/τ), distance v0 τ (1 − e^(−dt/τ)).
        if self.fling != (0.0, 0.0) {
            let e = (-dt / FLING_TAU).exp();
            dx += self.fling.0 * FLING_TAU * (1.0 - e);
            dy += self.fling.1 * FLING_TAU * (1.0 - e);
            self.fling = (self.fling.0 * e, self.fling.1 * e);
            if self.fling.0.hypot(self.fling.1) < FLING_STOP {
                self.fling = (0.0, 0.0);
            }
        }
        if dx != 0.0 || dy != 0.0 {
            // Pan and zoom run together: pan at the step's midpoint scale, so a frame's
            // order of operations does not show as a frame-rate dependence.
            let mid = (-0.5 * ln_zoom).exp();
            viewport.pan_by_points(dx * mid, dy * mid);
        }
        if ln_zoom != 0.0 {
            let (x, y) = self.zoom_target();
            viewport.zoom_by(ln_zoom.exp(), x, y);
            ln_zoom = 0.0;
        }
        if self.zoom_fling != 0.0 {
            let e = (-dt / ZOOM_FLING_TAU).exp();
            ln_zoom += self.zoom_fling * ZOOM_FLING_TAU * (1.0 - e);
            self.zoom_fling *= e;
            if self.zoom_fling.abs() < ZOOM_FLING_STOP {
                self.zoom_fling = 0.0;
            }
            let (x, y) = self.gesture_target(self.zoom_fling_at.0, self.zoom_fling_at.1);
            viewport.zoom_by(ln_zoom.exp(), x, y);
        }
        // Animated zoom steps: exponential ease-out of what is left.
        if self.step_left != 0.0 {
            let mut part = self.step_left * (1.0 - (-dt / STEP_TAU).exp());
            if (self.step_left - part).abs() < 1e-4 {
                part = self.step_left;
            }
            self.step_left -= part;
            let (x, y) = self.step_at.unwrap_or((0.0, 0.0));
            viewport.zoom_by(part.exp(), x, y);
            if self.step_left == 0.0 {
                self.step_at = None;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 1000.0;

    fn setup() -> (Navigator, Viewport) {
        let mut n = Navigator::default();
        let mut v = Viewport::new(-0.5, 0.25, 1e-3);
        n.handle(
            InputEvent::Resize {
                width: W,
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

    /// Runs frames at `hz` from `t0` to `t1`; returns the time reached.
    fn run(n: &mut Navigator, v: &mut Viewport, t0: f64, t1: f64, hz: f64) -> f64 {
        let steps = ((t1 - t0) * hz).round() as usize;
        let mut t = t0;
        for i in 1..=steps {
            t = t0 + i as f64 / hz;
            n.advance(v, t);
        }
        t
    }

    /// Distance a critically damped velocity covers from rest towards `target` in `t`.
    fn ease_in_distance(target: f64, omega: f64, t: f64) -> f64 {
        let mut s = Spring::default();
        s.follow(target, omega, t)
    }

    #[test]
    fn spring_integral_is_exact() {
        // Many small steps equal one big step, for distance and state.
        let mut a = Spring { v: 3.0, a: -20.0 };
        let mut b = a;
        let big = a.follow(10.0, 25.0, 0.2);
        let mut small = 0.0;
        for _ in 0..1000 {
            small += b.follow(10.0, 25.0, 0.0002);
        }
        assert!((big - small).abs() < 1e-9, "{big} vs {small}");
        assert!((a.v - b.v).abs() < 1e-9 && (a.a - b.a).abs() < 1e-7);
        // Numerical integration of the closed-form velocity agrees with the integral.
        let mut c = Spring::default();
        let d = c.follow(1.0, 30.0, 0.05);
        let x0 = -1.0f64;
        let numeric: f64 = (0..10000)
            .map(|i| {
                let t = (i as f64 + 0.5) * 0.05 / 10000.0;
                1.0 + x0 * (1.0 + 30.0 * t) * (-30.0 * t).exp()
            })
            .sum::<f64>()
            * 0.05
            / 10000.0;
        assert!((d - numeric).abs() < 1e-9);
    }

    #[test]
    fn keys_ease_in_and_glide_to_a_stop() {
        let (mut n, mut v) = setup();
        let re0 = v.center.re();
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 0.0);
        let target = PAN_SPEED * W;
        // Half the key speed after ~56 ms, 90 % after ~130 ms: no hard start.
        let t = run(&mut n, &mut v, 0.0, 0.05, 120.0);
        let vx = -n.velocity().0;
        assert!(vx > 0.3 * target && vx < 0.6 * target, "{vx} at {t}");
        let t = run(&mut n, &mut v, t, 1.0, 120.0);
        assert!((-n.velocity().0 - target).abs() < 1e-6 * target);
        // Distance so far: the eased start loses 2/ω of a second.
        let moved = (v.center.re() - re0) / 1e-3;
        let expect = ease_in_distance(target, PAN_OMEGA.0, 1.0);
        assert!((moved - expect).abs() < 1e-6, "{moved} vs {expect}");
        assert!((expect - target * (1.0 - 2.0 / PAN_OMEGA.0)).abs() < 1e-3 * target);
        // Released: glides 2/ω_decel worth of speed further, then rests.
        n.handle(InputEvent::KeyUp(NavKey::Right), &mut v, t);
        let before = v.center.re();
        let mut t = t;
        let mut frames = 0;
        while n.advance(&mut v, t + 1.0 / 120.0) {
            t += 1.0 / 120.0;
            frames += 1;
            assert!(frames < 240, "never settled");
        }
        let glide = (v.center.re() - before) / 1e-3;
        assert!(
            (glide - target * 2.0 / PAN_OMEGA.1).abs() < 0.01 * target,
            "glide {glide}"
        );
        // ... monotonic (critically damped: no backwards drift) and settled within ~0.6 s.
        assert!(t - 1.0 < 0.7, "settled after {} s", t - 1.0);
        assert_eq!(n.velocity(), (0.0, 0.0, 0.0));
        assert!(n.is_interacting(t + 0.1) && !n.is_interacting(t + 0.2));
    }

    #[test]
    fn motion_is_frame_rate_independent() {
        let path = |hz: f64| {
            let (mut n, mut v) = setup();
            let re0 = v.center.re();
            n.advance(&mut v, 0.0);
            n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 0.0);
            n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, 0.0);
            let mut out = Vec::new();
            let mut t = 0.0;
            for k in 1..=8 {
                if k == 4 {
                    n.handle(InputEvent::KeyUp(NavKey::Right), &mut v, t);
                    n.handle(InputEvent::KeyUp(NavKey::Zoom), &mut v, t);
                }
                t = run(&mut n, &mut v, t, k as f64 * 0.1, hz);
                out.push(((v.center.re() - re0) / 1e-3, v.scale.ln()));
            }
            out
        };
        let (a, b, c) = (path(30.0), path(60.0), path(120.0));
        for i in 0..a.len() {
            for o in [b[i], c[i]] {
                // Pan alone is exact; coupled with a zoom, second order in the frame time.
                assert!(
                    (a[i].0 - o.0).abs() < 2e-4 * a[i].0.abs(),
                    "pan {i}: {} vs {}",
                    a[i].0,
                    o.0
                );
                assert!((a[i].1 - o.1).abs() < 1e-9, "zoom {i}");
            }
        }
    }

    #[test]
    fn motion_from_idle_integrates_from_the_key_press() {
        // The display link paused long ago; the first frame after a key press moves the view
        // by the eased distance since the press, not by the 0.1 s cap.
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        assert!(!n.advance(&mut v, 0.01));
        let re0 = v.center.re();
        n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 5.0);
        n.advance(&mut v, 5.008);
        let moved = (v.center.re() - re0) / 1e-3;
        let expect = ease_in_distance(PAN_SPEED * W, PAN_OMEGA.0, 0.008);
        assert!((moved - expect).abs() < 1e-9, "{moved} vs {expect}");
    }

    #[test]
    fn space_zooms_toward_pointer_and_speed_keys() {
        let (mut n, mut v) = setup();
        n.handle(InputEvent::Pointer(Some((100.0, 50.0))), &mut v, 0.0);
        let target = v.point_at_offset(100.0, 50.0);
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, 0.0);
        n.advance(&mut v, 0.05);
        let expect = 1e-3 / ease_in_distance(1.6, ZOOM_OMEGA.0, 0.05).exp();
        assert!((v.scale - expect).abs() < 1e-15);
        let (dr, di) = v.point_at_offset(100.0, 50.0).delta_from(&target);
        assert!(dr.abs() < 1e-15 && di.abs() < 1e-15);
        n.handle(InputEvent::KeyDown(NavKey::FasterZoom), &mut v, 0.1);
        assert!((n.zoom_speed() - 2.0).abs() < 1e-12);
        n.handle(InputEvent::FocusLost, &mut v, 0.2);
        assert_eq!(n.held_count(), 0);
        // Pointer outside the view: zoom around the centre.
        n.handle(InputEvent::Pointer(Some((900.0, 0.0))), &mut v, 0.2);
        assert_eq!(n.zoom_target(), (0.0, 0.0));
        // The zoom glides out after release, then stops.
        let t = run(&mut n, &mut v, 0.2, 1.2, 60.0);
        assert!(!n.is_moving(), "still moving at {t}");
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
        // (The tapped key's pan has glided out by now too.)
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
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
        // Pointer mode: every zoom input keeps the point under the pointer, including the
        // eased start and stop and the animated steps.
        let (mut n, mut v) = setup();
        let p = (-321.0, 187.0);
        n.handle(InputEvent::Pointer(Some(p)), &mut v, 0.0);
        let target = v.point_at_offset(p.0, p.1);
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, 0.0);
        let mut t = run(&mut n, &mut v, 0.0, 2.0, 60.0);
        n.handle(InputEvent::KeyUp(NavKey::Zoom), &mut v, t);
        for e in [
            InputEvent::KeyDown(NavKey::StepIn),
            InputEvent::WheelZoom {
                delta: 2.0,
                x: p.0,
                y: p.1,
            },
            InputEvent::WheelNotch {
                delta: 1.0,
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
        t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(!n.is_moving());
        assert!(v.scale < 1e-4);
        let (dr, di) = v.point_at_offset(p.0, p.1).delta_from(&target);
        assert!(dr.hypot(di) / v.scale < 1e-6, "drift {} points", dr.hypot(di) / v.scale);

        // Keyboard mode after the glide: the centre is the target, for a trackpad pinch too
        // (its event position is wherever the hidden pointer is).
        n.handle(InputEvent::KeyDown(NavKey::Down), &mut v, t);
        n.handle(InputEvent::KeyUp(NavKey::Down), &mut v, t);
        t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(!n.is_gliding() && !n.is_moving());
        let centre = v.point_at_offset(0.0, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, t);
        t = run(&mut n, &mut v, t, t + 1.0, 60.0);
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
    fn zoom_steps_are_animated_and_exact() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        n.handle(
            InputEvent::DoubleTap {
                x: 50.0,
                y: -20.0,
                out: false,
            },
            &mut v,
            0.0,
        );
        // Not a jump: after one 120 Hz frame about 13 % of the step (1 − e^(−1/120/τ)).
        n.advance(&mut v, 1.0 / 120.0);
        let frac = (1e-3 / v.scale).ln() / 2f64.ln();
        assert!((frac - (1.0 - (-1.0 / 120.0 / STEP_TAU).exp())).abs() < 1e-9, "{frac}");
        // Two notches in quick succession add up exactly.
        n.handle(
            InputEvent::WheelNotch {
                delta: 1.0,
                x: 50.0,
                y: -20.0,
            },
            &mut v,
            0.01,
        );
        run(&mut n, &mut v, 1.0 / 120.0, 1.0, 120.0);
        assert!(!n.is_moving());
        assert!((v.scale - 1e-3 / 2.0 / 1.15).abs() < 1e-12, "{}", v.scale);
    }

    #[test]
    fn drag_release_flings_and_decays() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        // A drag at 600 pt/s, 120 Hz events, released while moving.
        let mut t = 0.0;
        for _ in 0..12 {
            t += 1.0 / 120.0;
            n.handle(InputEvent::Pan { dx: 5.0, dy: 0.0 }, &mut v, t);
        }
        let re = v.center.re();
        n.handle(InputEvent::GestureEnd, &mut v, t + 0.005);
        assert!((n.velocity().0 - 600.0).abs() < 1.0, "{:?}", n.velocity());
        let end = run(&mut n, &mut v, t, t + 3.0, 120.0);
        assert!(!n.is_moving(), "fling still running at {end}");
        // Coasts v0 τ (minus the tail below the stop speed), content moving right.
        let coast = (re - v.center.re()) / 1e-3;
        assert!(
            (coast - 600.0 * FLING_TAU).abs() < FLING_STOP * FLING_TAU + 1.0,
            "coast {coast}"
        );

        // Released after holding still: no fling. A trackpad scroll (no GestureEnd): none.
        n.handle(InputEvent::Pan { dx: 5.0, dy: 0.0 }, &mut v, 10.0);
        n.handle(InputEvent::Pan { dx: 5.0, dy: 0.0 }, &mut v, 10.008);
        n.handle(InputEvent::GestureEnd, &mut v, 10.2);
        assert_eq!(n.velocity(), (0.0, 0.0, 0.0));
        // A new touch stops a fling.
        n.handle(InputEvent::Pan { dx: 5.0, dy: 0.0 }, &mut v, 11.0);
        n.handle(InputEvent::Pan { dx: 5.0, dy: 0.0 }, &mut v, 11.008);
        n.handle(InputEvent::GestureEnd, &mut v, 11.01);
        assert!(n.velocity().0 > 100.0);
        n.handle(InputEvent::Pan { dx: 0.0, dy: 0.0 }, &mut v, 11.05);
        assert_eq!(n.velocity(), (0.0, 0.0, 0.0));
    }

    #[test]
    fn pinch_release_flings_the_zoom_at_the_pinch_centre() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        let c = (120.0, -40.0);
        let fixed = v.point_at_offset(c.0, c.1);
        let mut t = 0.0;
        for _ in 0..10 {
            t += 1.0 / 60.0;
            n.handle(
                InputEvent::Pinch {
                    ratio: 1.05,
                    x: c.0,
                    y: c.1,
                    dx: 0.0,
                    dy: 0.0,
                },
                &mut v,
                t,
            );
        }
        n.handle(InputEvent::GestureEnd, &mut v, t);
        assert!((n.velocity().2 - 1.05f64.ln() * 60.0).abs() < 1e-6);
        run(&mut n, &mut v, t, t + 2.0, 60.0);
        assert!(!n.is_moving());
        let (dr, di) = v.point_at_offset(c.0, c.1).delta_from(&fixed);
        assert!(dr.hypot(di) / v.scale < 1e-6);
    }

    #[test]
    fn gamepad_dead_zones_curves_and_boost() {
        assert_eq!(response_curve(0.1, STICK_DEAD_ZONE), 0.0);
        assert_eq!(response_curve(1.0, STICK_DEAD_ZONE), 1.0);
        // Fine control near the centre: half deflection is well under half speed.
        let half = response_curve(STICK_DEAD_ZONE + 0.5 * (1.0 - STICK_DEAD_ZONE), STICK_DEAD_ZONE);
        assert!((half - (0.125 + 0.75 * 0.125)).abs() < 1e-12);
        let (mut n, mut v) = setup();
        let pad = |lx: f64, ly: f64, ry: f64, lt: f64, rt: f64, boost: bool| InputEvent::Gamepad {
            lx,
            ly,
            rx: 0.0,
            ry,
            lt,
            rt,
            boost,
        };
        // Drift inside the dead zone: nothing moves, the mode stays.
        n.handle(pad(0.1, -0.08, 0.05, 0.03, 0.0, false), &mut v, 0.0);
        assert!(!n.is_moving());
        assert_eq!(n.mode(), InputMode::Pointer);
        // Full right stick: the camera moves right at the key speed (content left), keyboard
        // mode; right trigger zooms in at the zoom speed.
        n.handle(pad(1.0, 0.0, 0.0, 0.0, 1.0, false), &mut v, 0.0);
        assert_eq!(n.mode(), InputMode::Keyboard);
        let (vx, vy, z) = n.target_velocity();
        assert!((vx + PAN_SPEED * W).abs() < 1e-9 && vy == 0.0 && (z - 1.6).abs() < 1e-12);
        // Boost; the left trigger against the right cancels.
        n.handle(pad(0.0, 1.0, 0.0, 1.0, 1.0, true), &mut v, 0.0);
        let (vx, vy, z) = n.target_velocity();
        assert!(vx.abs() < 1e-9 && (vy + BOOST * PAN_SPEED * W).abs() < 1e-9 && z == 0.0);
        // Right stick up zooms in.
        n.handle(pad(0.0, 0.0, 1.0, 0.0, 0.0, false), &mut v, 0.0);
        assert!((n.target_velocity().2 - 1.6).abs() < 1e-12);
        // Released (or disconnected: zero state): the motion eases out and stops.
        n.handle(pad(0.0, 0.0, 0.0, 0.0, 0.0, false), &mut v, 0.0);
        n.advance(&mut v, 0.0);
        run(&mut n, &mut v, 0.0, 1.0, 60.0);
        assert!(!n.is_moving());
    }

    #[test]
    fn reset_stops_everything() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 0.0);
        run(&mut n, &mut v, 0.0, 0.5, 60.0);
        n.handle(InputEvent::KeyUp(NavKey::Right), &mut v, 0.5);
        n.handle(InputEvent::KeyDown(NavKey::Reset), &mut v, 0.5);
        assert!(!n.is_moving());
        assert_eq!(v, Viewport::fitting(W, 600.0));
    }

    #[test]
    fn edge_pan_ramps_from_the_band_border_to_key_speed() {
        let (mut n, mut v) = setup();
        let band = EDGE_BAND * 600.0;
        let speed_at = |n: &mut Navigator, v: &mut Viewport, x: f64, y: f64| {
            n.handle(InputEvent::Pointer(Some((x, y))), v, 0.0);
            let (dx, dy, _) = n.step(1.0);
            (dx / (PAN_SPEED * W), dy / (PAN_SPEED * W))
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
