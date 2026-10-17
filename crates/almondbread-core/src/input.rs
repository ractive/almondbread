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

use crate::autopilot::{self, Autopilot};
use crate::cinematic::Cinematic;
use crate::cruise::{self, Cruise, Level};
use crate::flyto::{self, Flight, FlyPath};
use crate::journey::{self, Choice, Journey};
use crate::seek::{SeekPhase, Seeker};
use crate::viewport::{Scale, Viewport, wrap_angle};

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
    /// Q: the pedal out (kb: decisions/controls "Driving levels"): from rest the cruise
    /// starts zooming out, while cruising one rung slower in / faster out; the autopilot's or
    /// a journey's speed one rung down (to zero: cruise takes over). While Space is held: the
    /// Space zoom speed x0.8; during a fly-to: its speed.
    SlowerZoom,
    /// E: the pedal in: as Q the other way (Space held: x1.25).
    FasterZoom,
    /// + / =: one step in (x1.5) at the zoom target.
    StepIn,
    /// - / _: one step out.
    StepOut,
    /// R: back to the home view fitted to the window.
    Reset,
    /// Z: autopilot on / off (kb: decisions/controls, autopilot).
    Autopilot,
    /// Esc: stops the autopilot (eased); not handled while it is off.
    Stop,
    /// `,` / `.` (held): turn the image counter-clockwise / clockwise (view rotation).
    RotateCcw,
    RotateCw,
    /// `/`: turn back upright (animated).
    Straighten,
    /// J: journey on (a random seed) / resume after manual input / off; at a journey stop:
    /// on as planned.
    Journey,
    /// ⇧J: as J, but a new journey starts in the other mode than the setting (continuous /
    /// stop at each find; kb: decisions/controls "Journey with stops").
    JourneyAlt,

    /// X: at a journey stop, somewhere else (the next leg crosses over); not handled otherwise.
    Elsewhere,
    /// N: seek the lowest-period minibrot in or near the view (the next smaller one from a
    /// minibrot's framing) and fly there (kb: decisions/controls "Seek minibrot").
    SeekMinibrot,
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
    /// Trackpad two-finger scroll (`momentum`: the OS's momentum phase after the fingers
    /// lifted): pans like `Pan`, but sets the speed while the autopilot is on.
    Scroll {
        dx: f64,
        dy: f64,
        momentum: bool,
    },
    /// A single click / tap on the canvas: pauses or resumes the autopilot (unused while it
    /// is off).
    Click {
        x: f64,
        y: f64,
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
    /// Trackpad rotation (macOS rotate gesture, iOS rotation recogniser): the image turns
    /// counter-clockwise by `radians` about (x, y) (pointer mode; keyboard mode: the
    /// crosshair), 1:1 with the fingers.
    Rotate {
        radians: f64,
        x: f64,
        y: f64,
    },
}

impl InputEvent {
    /// Every number in the event is finite (NaN or inf from a platform bug would reach the
    /// BigFloat centre and panic, or a size of usize::MAX; kb: techniques/hang-prevention).
    pub fn is_finite(&self) -> bool {
        let f = |v: &[f64]| v.iter().all(|x| x.is_finite());
        match *self {
            Self::KeyDown(_) | Self::KeyUp(_) | Self::Shift(_) | Self::FocusLost | Self::GestureEnd => true,
            Self::Resize { width, height } => f(&[width, height]),
            Self::Insets {
                top,
                left,
                bottom,
                right,
            } => f(&[top, left, bottom, right]),
            Self::Pointer(p) => p.is_none_or(|(x, y)| f(&[x, y])),
            Self::Pan { dx, dy } | Self::Scroll { dx, dy, .. } => f(&[dx, dy]),
            Self::Click { x, y } | Self::DoubleTap { x, y, .. } => f(&[x, y]),
            Self::WheelZoom { delta, x, y } | Self::WheelNotch { delta, x, y } => f(&[delta, x, y]),
            Self::Magnify { magnification, x, y } => f(&[magnification, x, y]),
            Self::Pinch { ratio, x, y, dx, dy } => f(&[ratio, x, y, dx, dy]),
            Self::ZoomBy { factor } => factor.is_finite(),
            Self::Gamepad {
                lx, ly, rx, ry, lt, rt, ..
            } => f(&[lx, ly, rx, ry, lt, rt]),
            Self::Rotate { radians, x, y } => f(&[radians, x, y]),
        }
    }
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
/// Turn rate of the rotate keys (radians per second, 45°/s) and their spring frequencies.
pub const ROTATE_SPEED: f64 = std::f64::consts::FRAC_PI_4;
pub const ROTATE_OMEGA: (f64, f64) = (20.0, 14.0);
/// Time constant (s) of the animated turn back upright.
pub const STRAIGHTEN_TAU: f64 = 0.12;
const ROTATE_REST: f64 = 1e-4;
/// Pointer travel (points) after a journey leg took the wheel, and how long (s) a deflection
/// past the steering dead zone must last, before it counts as the user steering (it then hands
/// over to cruise): the pointer passing over the canvas on its way to a button is not steering.
/// Raised 2026-10-17 (user: "moving the mouse just a bit throws you into cruise"): 60 points
/// of travel, a deflection past [`STEER_HANDOVER`] of the steering radius, held 0.6 s. In the
/// autopilot the pointer no longer hands over: it is a direction wish ([`WISH_TRAVEL`]).
pub const STEER_TRAVEL: f64 = 60.0;
pub const STEER_ENGAGE: f64 = 0.6;
/// Share of the steering radius the pointer must be off-centre to count as steering against
/// the autopilot (well outside the steering dead zone: a nudge of the mouse is not steering).
pub const STEER_HANDOVER: f64 = 0.35;
/// Pointer travel (points) since the autopilot took the wheel before the pointer's direction
/// counts as a wish (kb: decisions/controls "Pointer wish"): a pointer resting off-centre when
/// Z was pressed is not a wish.
pub const WISH_TRAVEL: f64 = 24.0;
/// A released cruise slower than this (e-folds/s) no longer holds a journey's next leg back.
pub const LEG_CARRY_REST: f64 = 0.15;
/// A lane-assist nudge lights the cockpit's lane icon this long (s).
pub const ASSIST_FLASH: f64 = 0.6;
/// Below these the velocities snap to rest (points/s, e-folds/s).
const PAN_REST: f64 = 0.5;
const ZOOM_REST: f64 = 1e-3;

/// What the target marker shows (kb: decisions/controls "Target marker"): where the autopilot
/// or a journey leg is heading (a complex point), a short name for a known kind of target, and
/// the pointer wish's direction (unit, view points, y up).
#[derive(Clone, Debug, PartialEq)]
pub struct AimMarker {
    pub center: Option<crate::viewport::Center>,
    pub label: Option<String>,
    pub wish: Option<(f64, f64)>,
}

/// Two-finger scroll set to zoom: e-folds per scrolled point (100 points: about ×2.7).
const SCROLL_ZOOM_PER_POINT: f64 = 0.01;
/// Lane assist (kb: decisions/controls "Driving levels"): a survey older than this (s) is not
/// acted on, and the nudge's heading reaches at most this share of the steering radius.
const ASSIST_SURVEY_AGE: f64 = 0.5;
const ASSIST_REACH: f64 = 0.6;

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
    /// Continuous zoom steered by the pointer (kb: decisions/controls, autopilot).
    autopilot: Autopilot,
    /// When the pointer last moved over the canvas (an idle pointer stops steering).
    pointer_moved: f64,
    /// When manual input last moved the view (holds the autopilot's target lock off).
    manual_at: f64,
    /// Fly to coordinates (kb: decisions/controls, fly to) and its speed (e-folds/s).
    flight: Option<Flight>,
    fly_speed: f64,
    /// When the fly speed last changed (HUD highlight).
    pub fly_speed_changed: f64,
    /// Rotate keys held, the turn rate (radians/s, + turns the camera counter-clockwise, the
    /// image clockwise), and an animated turn still to apply (Straighten).
    rot_ccw: bool,
    rot_cw: bool,
    rot_rate: Spring,
    rot_step_left: f64,
    /// Cinematic effects: the spin while the autopilot, a journey or a flight zooms.
    cinematic: Cinematic,
    /// The journey: planned flights from landmark to landmark (kb: decisions/controls).
    journey: Journey,
    /// Seek minibrot (N): its search thread and what the HUD says.
    seek: Seeker,
    /// The setting "Journey: stop at each find" (J starts that mode, ⇧J the other).
    journey_stops: bool,
    /// Cruise: the pedal-driven zoom of level C (kb: decisions/controls "Driving levels").
    cruise: Cruise,
    /// A held pedal (direction +1 E / −1 Q) and when it steps again.
    pedal_hold: Option<(f64, f64)>,
    /// Pointer travel (points) since the autopilot or a journey leg took the wheel, and since
    /// when the pointer has deflected past the dead zone: sustained steering hands over to
    /// cruise.
    wheel_travel: f64,
    deflected_since: Option<f64>,
    /// Lane assist (setting, default off; active in cruise only) and when it last nudged.
    assist: bool,
    /// Two-finger scrolling zooms (vertical) instead of panning (setting "Two-finger scroll").
    scroll_zooms: bool,
    assist_nudged: f64,
    /// The compass's eased turn back to north (kb: decisions/controls "Compass").
    north: Option<crate::compass::NorthTurn>,
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
            autopilot: Autopilot::default(),
            pointer_moved: f64::NEG_INFINITY,
            manual_at: f64::NEG_INFINITY,
            flight: None,
            fly_speed: flyto::SPEED_DEFAULT,
            fly_speed_changed: f64::NEG_INFINITY,
            rot_ccw: false,
            rot_cw: false,
            rot_rate: Spring::default(),
            rot_step_left: 0.0,
            cinematic: Cinematic::default(),
            journey: Journey::default(),
            seek: Seeker::default(),
            journey_stops: false,
            cruise: Cruise::default(),
            pedal_hold: None,
            wheel_travel: 0.0,
            deflected_since: None,
            assist: false,
            scroll_zooms: false,
            assist_nudged: f64::NEG_INFINITY,
            north: None,
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

    /// The view's size in points.
    pub fn view_size(&self) -> (f64, f64) {
        self.view_size
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

    pub fn autopilot(&self) -> &Autopilot {
        &self.autopilot
    }

    /// The renderer reports feedback and surveys here; settings (assist) too.
    pub fn autopilot_mut(&mut self) -> &mut Autopilot {
        &mut self.autopilot
    }

    /// The autopilot, a journey or a flight runs: the renderer reports how it keeps up.
    pub fn wants_feedback(&self) -> bool {
        self.autopilot.is_active() || self.flight.is_some() || self.journey.wants_frames() || self.cruise.is_engaged()
    }

    /// Automatic motion (autopilot, journey, flight) is under way: colours may cycle.
    /// The journey's palette phase now (cycles; kb: decisions/controls "Journey"): it moves
    /// on at each cross-over, eased over the leg's flight. The renderer adds it to the palette
    /// offset only with the cinematic effects on.
    pub fn journey_palette_phase(&self) -> f64 {
        let t = match &self.flight {
            Some(f) if self.journey.is_running() => f.progress(),
            _ => 1.0,
        };
        self.journey.palette_phase(t)
    }

    pub fn is_flying(&self) -> bool {
        self.autopilot.is_moving() || self.flight.is_some() || self.cruise.is_moving()
    }

    // MARK: Driving levels (kb: decisions/controls "Driving levels")

    /// The driving level: C while the cruise is engaged (its soft landing included), A while
    /// the autopilot is on, J while a journey runs (at a stop too) or a fly-to flight flies,
    /// else M.
    pub fn level(&self) -> Level {
        if self.cruise.is_engaged() {
            Level::Cruise
        } else if self.autopilot.is_active() {
            Level::Autopilot
        } else if self.journey.is_running() || self.flight.is_some() {
            Level::Journey
        } else {
            Level::Manual
        }
    }

    pub fn cruise(&self) -> &Cruise {
        &self.cruise
    }

    /// What the target marker shows (kb: decisions/controls "Target marker"): while the
    /// autopilot runs, its lock target or survey steering's boundary point and the pointer
    /// wish; while a journey flies a leg, the leg's target. None in manual and cruise.
    pub fn aim_marker(&self) -> Option<AimMarker> {
        if self.autopilot.is_active() {
            let ap = &self.autopilot;
            let (center, label) = match (ap.is_survey_steering(), ap.lock().target(), ap.survey_pick()) {
                (false, Some(t), _) => (Some(t.center.clone()), t.kind.marker_label(t.period)),
                (true, _, Some(c)) => (Some(c.clone()), None),
                _ => (None, None),
            };
            return Some(AimMarker {
                center,
                label,
                wish: ap.wish().map(|w| w.dir),
            });
        }
        if self.journey_flying() {
            let t = self.journey.current()?;
            return Some(AimMarker {
                center: Some(t.center.clone()),
                label: t.kind.marker_label(t.period),
                wish: None,
            });
        }
        None
    }

    /// What the HUD cockpit shows.
    pub fn cockpit(&self, now: f64) -> Cockpit {
        let level = self.level();
        let (set, rate) = match level {
            Level::Manual => (0.0, self.zoom_rate.v),
            Level::Cruise => match (self.cruise.landing(), &self.flight) {
                (Some(_), Some(f)) => (self.cruise.set_speed(), -f.speed.v * flyto::RHO),
                _ => (self.cruise.set_speed(), self.cruise.rate()),
            },
            Level::Autopilot => (self.autopilot.speed(), self.autopilot.rate()),
            Level::Journey if self.journey.at_stop() => (0.0, 0.0),
            Level::Journey => {
                let s = self.flight_speed();
                (s, if self.flight.is_some() { s } else { 0.0 })
            }
        };
        Cockpit {
            level,
            set,
            rate,
            boost: if level == Level::Cruise {
                self.cruise.boost()
            } else {
                1.0
            },
            landing: self.cruise.landing().is_some(),
            assist: self.assist,
            nudging: self.assist && now - self.assist_nudged < ASSIST_FLASH,
        }
    }

    /// Lane assist (setting "Assist", key L; kb: decisions/controls "Driving levels").
    pub fn assist(&self) -> bool {
        self.assist
    }

    pub fn set_assist(&mut self, on: bool) {
        self.assist = on;
    }

    /// Two-finger scroll: zoom (vertical only, about the pointer; horizontal ignored) instead of
    /// pan (setting "Two-finger scroll: Pan / Zoom", default pan). Click-and-drag still pans.
    pub fn set_scroll_zooms(&mut self, on: bool) {
        self.scroll_zooms = on;
    }

    /// Lane assist's nudge of the cruise's heading target (view points from the centre), asked
    /// each cruise frame while the user does not steer. Phase 1: the hook, no nudge. Phase 2
    /// wires the GPU survey's steering away from featureless regions (the void-steer work in
    /// `autopilot`): a gentle nudge towards the nearest boundary only when the region ahead is
    /// becoming featureless; the user's steering always wins (this is not asked then).
    fn assist_nudge(&self, _viewport: &Viewport, radius: f64, now: f64) -> Option<(f64, f64)> {
        let s = self.autopilot.survey()?;
        if now - s.time > ASSIST_SURVEY_AGE {
            return None;
        }
        // Straight ahead is the centre (the user does not steer when this is asked).
        let a = s.assess((0.0, 0.0))?;
        if a.danger() <= 0.5 && a.interior <= 0.5 {
            return None;
        }
        let p = s.boundary_point((0.0, 0.0), None)?;
        let d = p.0.hypot(p.1);
        if !(d.is_finite() && d >= 1.0) {
            return None;
        }
        // A gentle heading towards the boundary point (at most ASSIST_REACH of the radius).
        let k = (ASSIST_REACH * radius).min(d) / d;
        Some((p.0 * k, p.1 * k))
    }

    /// The renderer surveys frames for the autopilot, and for lane assist while cruising.
    pub fn wants_survey(&self) -> bool {
        self.autopilot.is_active() || (self.assist && self.cruise.is_engaged())
    }

    /// A manual zoom (wheel, trackpad, pinch, double tap, menu) while cruising takes the zoom
    /// over: the cruise stops dead and the level drops to manual (the user's request: zooming by
    /// hand should not fight a set speed).
    fn manual_zoom_ends_cruise(&mut self) {
        if self.cruise.is_engaged() {
            self.cruise.halt();
        }
    }

    /// Depth below the home view fitted to the window, e-folds (negative: wider than home).
    fn depth(&self, viewport: &Viewport) -> f64 {
        let home = Viewport::fitting(self.view_size.0, self.view_size.1);
        home.scale.ln() - viewport.scale.ln()
    }

    /// The autopilot runs, or a journey flies a leg: sustained user steering takes the wheel.
    fn automatic_steerable(&self) -> bool {
        (self.autopilot.is_active() && !self.autopilot.is_paused()) || self.journey_flying()
    }

    fn journey_flying(&self) -> bool {
        self.journey.is_running() && !self.journey.at_stop() && self.flight.is_some() && self.cruise.landing().is_none()
    }

    /// Steering events: W A S D / arrows, a drag or touch pan, the gamepad's left stick.
    fn is_steering(event: &InputEvent) -> bool {
        match *event {
            InputEvent::KeyDown(k) => k.is_direction(),
            InputEvent::Pan { .. } => true,
            InputEvent::Gamepad { lx, ly, .. } => lx.hypot(ly) > STICK_DEAD_ZONE,
            _ => false,
        }
    }

    /// The autopilot or a journey's leg took the wheel now: pointer steering is measured anew.
    fn reset_wheel(&mut self) {
        self.wheel_travel = 0.0;
        self.deflected_since = None;
    }

    /// The user takes the wheel from the autopilot or a journey's flight: the cruise engages
    /// with the current zoom rate and `set` (default: the autopilot's set speed, or the
    /// flight's current zoom rate); the journey is suspended (J resumes), the autopilot off
    /// and it does not take the wheel back by itself. Returns whether it did.
    fn take_wheel(&mut self, set: Option<f64>, now: f64) -> bool {
        if self.autopilot.is_active() {
            let rate = self.autopilot.rate();
            let set = set.unwrap_or(if rate >= 0.0 { self.autopilot.speed() } else { rate });
            let heading = self.autopilot.heading();
            self.autopilot.halt();
            self.cruise.engage(set, rate, heading, now);
            true
        } else if self.journey_flying() {
            self.journey.suspend();
            let z = self.take_flight_motion().unwrap_or(0.0);
            // The cruise goes on at the journey's speed, not at the leg's paced rate (long legs
            // and dull stretches fly up to 9× faster: a hand-over during a cross-over set the
            // cruise to ×55/s and more); the rate eases there from the flight's.
            let set = set.unwrap_or(z.signum() * z.abs().min(self.journey.speed()));
            self.cruise.engage(set, z, (0.0, 0.0), now);
            true
        } else {
            false
        }
    }

    /// A released cruise still glides out (after J resumed or chose a journey's next leg).
    fn cruise_gliding(&self) -> bool {
        !self.cruise.is_engaged() && self.cruise.landing().is_none() && self.cruise.is_moving()
    }

    /// The next leg waits while a released cruise still zooms faster than [`LEG_CARRY_REST`]
    /// the other way than the leg starts (a leg that starts the same way takes its rate over).
    fn leg_waits_for_cruise(&self, viewport: &Viewport) -> bool {
        let rate = self.cruise.rate();
        if rate.abs() <= LEG_CARRY_REST {
            return false;
        }
        let Some(t) = self.journey.next_target() else {
            return false;
        };
        let target = t.viewport(self.min_side());
        let path = FlyPath::new(viewport, &target, self.view_size.0);
        rate.signum() != flyto_start_sign(&path)
    }

    /// The pointer steers (pointer mode, over the canvas, moved lately and since the wheel was
    /// taken, deflected past the dead zone).
    fn pointer_deflected(&self, now: f64) -> bool {
        if self.mode != InputMode::Pointer
            || now - self.pointer_moved >= autopilot::STEER_IDLE
            || self.wheel_travel < STEER_TRAVEL
        {
            return false;
        }
        let Some(p) = self.pointer_on_canvas() else {
            return false;
        };
        let r = self.steer_radius();
        p.0.hypot(p.1) >= STEER_HANDOVER * r && self.autopilot.steer_target(Some(p), r) != (0.0, 0.0)
    }

    fn steer_radius(&self) -> f64 {
        let (x0, x1, y0, y1) = self.canvas();
        0.5 * (x1 - x0).min(y1 - y0)
    }

    /// A pedal press (`cross`) or a held pedal's repeat: E (+1) / Q (−1) on the ladder of the
    /// level driving: the cruise from rest or in C, the autopilot's speed in A, the journey's
    /// in J; down to zero there, the cruise takes over (braking to rest).
    fn pedal(&mut self, dir: f64, cross: bool, viewport: &Viewport, now: f64) {
        self.note_interaction(now);
        if self.cruise.landing().is_some() {
            // Towards zero while landing: an eased stop where it is.
            if dir > 0.0 {
                self.cruise.stop(now);
                self.stop_flight();
            }
            return;
        }
        let journey_drives = self.journey.wants_frames();
        if self.cruise.is_engaged() || !(self.autopilot.is_active() || journey_drives) {
            let out_ok = self.depth(viewport) > 0.01;
            self.cruise.pedal(dir, cross, out_ok, now);
            return;
        }
        // Beyond the ladder's top rung (journeys fly up to 16 e-folds/s) E goes on ×1.25.
        let step = |v: f64| {
            if dir > 0.0 && v >= cruise::max_speed() * 0.99 {
                v * autopilot::SPEED_STEP
            } else {
                cruise::pedal(v, dir, cross)
            }
        };
        if self.autopilot.is_active() {
            let next = step(self.autopilot.speed());
            if next <= 0.0 {
                self.take_wheel(Some(0.0), now);
            } else {
                self.autopilot.set_speed(next, now);
            }
            return;
        }
        let next = step(self.journey.speed());
        if next <= 0.0 {
            if !self.take_wheel(Some(0.0), now) {
                self.journey.suspend();
            }
        } else {
            self.journey.set_speed(next, now);
        }
    }

    pub fn cinematic(&self) -> Cinematic {
        self.cinematic
    }

    /// The renderer passes the settings' effects every frame.
    pub fn set_cinematic(&mut self, c: Cinematic) {
        self.cinematic = c;
    }

    pub fn journey(&self) -> &Journey {
        &self.journey
    }

    pub fn journey_mut(&mut self) -> &mut Journey {
        &mut self.journey
    }

    /// The setting "Journey: stop at each find" (J starts that mode, ⇧J the other).
    pub fn journey_stops(&self) -> bool {
        self.journey_stops
    }

    pub fn set_journey_stops(&mut self, on: bool) {
        self.journey_stops = on;
    }

    /// Starts a journey with `seed` from `viewport` in the set mode (the autopilot and any
    /// flight stop).
    pub fn start_journey(&mut self, seed: u64, daily: bool, viewport: &Viewport, now: f64) {
        self.start_journey_mode(seed, daily, self.journey_stops, viewport, now);
    }

    /// Starts a journey with `seed`, stopping at each find or not.
    pub fn start_journey_mode(&mut self, seed: u64, daily: bool, stops: bool, viewport: &Viewport, now: f64) {
        let was = self.is_animating();
        self.stop_motion();
        let span = viewport.scale * self.min_side();
        self.journey.start(seed, daily, &viewport.center, span);
        self.journey.set_stops(stops);
        self.note_interaction(now);
        if !was {
            self.last_tick = Some(now);
        }
    }

    /// Stops a journey (its flight eases to a stop).
    pub fn stop_journey(&mut self) {
        if self.journey.is_active() {
            self.journey.stop();
            self.stop_flight();
        }
    }

    /// Resumes a journey that manual input took over.
    pub fn resume_journey(&mut self, viewport: &Viewport, now: f64) {
        if self.journey.is_suspended() {
            let span = viewport.scale * self.min_side();
            self.autopilot.halt();
            // The cruise brakes out on its spring; the next leg starts once it carries on the
            // cruise's direction or the cruise is (nearly) at rest (no dead stop).
            self.cruise.release();
            self.journey.resume(&viewport.center, span);
            self.note_interaction(now);
            self.last_tick.get_or_insert(now);
        }
    }

    /// A choice at a journey stop (J, Z, X; kb: decisions/controls "Journey with stops"):
    /// the next leg is planned from the landmark, or from the view if the user moved far away.
    /// Returns whether the journey was at a stop.
    pub fn journey_choose(&mut self, choice: Choice, viewport: &Viewport, now: f64) -> bool {
        let span = viewport.scale * self.min_side();
        let was = self.is_animating();
        let chose = self.journey.choose(choice, &viewport.center, span);
        if chose {
            self.cruise.release();
            self.note_interaction(now);
            if !was {
                self.last_tick = Some(now);
            }
        }
        chose
    }

    /// The view's shorter side in points (spans of places and journey targets).
    fn min_side(&self) -> f64 {
        let s = self.view_size.0.min(self.view_size.1);
        if s.is_finite() && s > 0.0 { s } else { 800.0 }
    }

    /// The journey's next target view once planned (the renderer prefetches its reference
    /// while the journey rests).
    pub fn prefetch_target(&self) -> Option<Viewport> {
        if !self.journey.is_running() || self.flight.is_some() || self.journey.choice().is_some() {
            return None;
        }
        self.journey.next_target().map(|t| t.viewport(self.min_side()))
    }

    /// Starts an animated flight from `viewport` to `target` (any depth); motion in progress
    /// stops. Any navigation input cancels it, Esc eases it to a stop. With the spin effect
    /// on, the view turns with the zoom on the way (the target's angle is then where the
    /// spin ends); otherwise it turns to the target's angle the short way round.
    pub fn fly_to(&mut self, target: &Viewport, viewport: &Viewport, now: f64) {
        self.seek.clear();
        let was = self.is_animating();
        self.stop_motion();
        self.journey.stop();
        self.begin_flight(target, viewport);
        self.note_interaction(now);
        if !was {
            self.last_tick = Some(now);
        }
    }

    fn begin_flight(&mut self, target: &Viewport, viewport: &Viewport) {
        let path = FlyPath::new(viewport, target, self.view_size.0);
        self.flight = Some(Flight::turning(path, viewport.angle, self.cinematic.spin_per_efold()));
    }

    /// The speed the flight runs at: the journey's during a journey, the cruise's at its soft
    /// landing on the home view, else the fly-to speed.
    pub fn flight_speed(&self) -> f64 {
        if let Some(s) = self.cruise.landing() {
            s
        } else if self.journey.is_running() {
            self.journey
                .leg_speed_at(self.flight.as_ref().map_or(0.0, |f| f.progress()))
        } else {
            self.fly_speed
        }
    }

    /// Q / E, wheel and scroll during a flight or journey.
    fn scale_flight_speed(&mut self, f: f64, now: f64) {
        if self.cruise.landing().is_some() {
            self.cruise.scale_speed(f, now);
        } else if self.journey.is_running() {
            self.journey.scale_speed(f, now);
        } else {
            self.set_fly_speed(self.fly_speed * f, now);
        }
    }

    /// The view's rotation rate (radians/s) right now.
    pub fn rotation_rate(&self) -> f64 {
        self.rot_rate.v
    }

    /// The view was set directly (go to coordinates, reset): a flight ends where it is.
    pub fn cancel_flight(&mut self) {
        self.flight = None;
        self.seek.clear();
    }

    pub fn flight(&self) -> Option<&Flight> {
        self.flight.as_ref()
    }

    /// A finished minibrot search: fly to the minibrot found (the view's angle kept).
    fn poll_seek(&mut self, viewport: &Viewport, now: f64) {
        if let Some(m) = self.seek.poll(now) {
            let target = m.viewport(self.min_side()).with_angle(viewport.angle);
            self.fly_to(&target, viewport, now);
            self.seek.flying(&m, target.zoom_log10(), now);
        }
    }

    /// Navigation ends a minibrot seek (its search, or the label of its flight): any key but N
    /// (Q / E and scrolling only while it searches: during the flight they set its speed), a
    /// click, or input that takes over a flight.
    fn ends_seek(&self, event: &InputEvent) -> bool {
        let searching = self.seek.is_searching();
        match *event {
            InputEvent::KeyDown(NavKey::SeekMinibrot) => false,
            InputEvent::KeyDown(NavKey::SlowerZoom | NavKey::FasterZoom) => searching,
            InputEvent::KeyDown(_) => true,
            InputEvent::Click { .. } | InputEvent::Scroll { .. } | InputEvent::WheelNotch { .. } => searching,
            ref e => Self::cancels_flight(e),
        }
    }

    /// Seek minibrot's state for the HUD: phase, period, the target's zoom (log10).
    pub fn seek_status(&self, now: f64) -> (SeekPhase, usize, f64) {
        self.seek.status(now)
    }

    pub fn seek_mut(&mut self) -> &mut Seeker {
        &mut self.seek
    }

    /// The flight's target, for an early reference.
    pub fn fly_target(&self) -> Option<&Viewport> {
        self.flight.as_ref().map(|f| f.path.target())
    }

    pub fn fly_speed(&self) -> f64 {
        self.fly_speed
    }

    pub fn set_fly_speed(&mut self, v: f64, now: f64) {
        if v.is_finite() {
            self.fly_speed = v.clamp(flyto::SPEED_RANGE.0, flyto::SPEED_RANGE.1);
            self.fly_speed_changed = now;
        }
    }

    /// Esc during a flight: slow to a stop on the path. Returns whether one was flying.
    fn stop_flight(&mut self) -> bool {
        match &mut self.flight {
            Some(f) => {
                f.stopping = true;
                true
            }
            None => false,
        }
    }

    /// Manual input takes over: the flight ends and its velocity carries on in the pan and
    /// zoom springs, which glide out (no jolt).
    fn hand_over_flight(&mut self) {
        if let Some(z) = self.take_flight_motion() {
            self.zoom_rate = Spring { v: z, a: 0.0 };
        }
    }

    /// Ends the flight: its pan and turn carry on in the springs (they glide out); returns its
    /// zoom rate (e-folds/s, + in) for whoever drives the zoom on.
    fn take_flight_motion(&mut self) -> Option<f64> {
        let f = self.flight.take()?;
        if self.cruise.landing().is_some() {
            self.cruise.end_landing();
        }
        let h = 1e-3;
        let a = f.viewport();
        let mut fb = f.clone();
        fb.s = (f.s + f.speed.v * h).min(f.path.length());
        let b = fb.viewport();
        let (dx, dy) = a.offset_of(&b.center);
        self.pan = [Spring { v: -dx / h, a: 0.0 }, Spring { v: -dy / h, a: 0.0 }];
        self.rot_rate = Spring {
            v: wrap_angle(b.angle - a.angle) / h,
            a: 0.0,
        };
        Some((a.scale.ln() - b.scale.ln()) / h)
    }

    /// Input that takes over from a flight.
    fn cancels_flight(event: &InputEvent) -> bool {
        match *event {
            InputEvent::KeyDown(k) => matches!(
                k,
                NavKey::Up
                    | NavKey::Down
                    | NavKey::Left
                    | NavKey::Right
                    | NavKey::Zoom
                    | NavKey::StepIn
                    | NavKey::StepOut
                    | NavKey::Autopilot
                    | NavKey::RotateCcw
                    | NavKey::RotateCw
                    | NavKey::Straighten
            ),
            InputEvent::Pan { .. }
            | InputEvent::Rotate { .. }
            | InputEvent::WheelZoom { .. }
            | InputEvent::Magnify { .. }
            | InputEvent::Pinch { .. }
            | InputEvent::DoubleTap { .. }
            | InputEvent::ZoomBy { .. } => true,
            InputEvent::Gamepad { lx, ly, ry, lt, rt, .. } => {
                lx.hypot(ly) > STICK_DEAD_ZONE
                    || ry.abs() > STICK_DEAD_ZONE
                    || lt > TRIGGER_DEAD_ZONE
                    || rt > TRIGGER_DEAD_ZONE
            }
            _ => false,
        }
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
        self.up || self.down || self.left || self.right || self.zoom || self.rot_ccw || self.rot_cw
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
            || self.autopilot.is_moving()
            || self.cruise.is_moving()
            || self.pedal_hold.is_some()
            || self.flight.is_some()
            || !self.rot_rate.at_rest()
            || self.rot_step_left != 0.0
            || self.journey.wants_frames()
            || self.north.is_some()
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
        self.rot_ccw = false;
        self.rot_cw = false;
        self.pad = GamepadState::default();
        self.pedal_hold = None;
    }

    /// Stops all motion at once (Reset, a new flight).
    /// A jump to a given view (pasted coordinates, a link, a place): everything that moves
    /// stops dead and a journey ends, so the new view is not carried off by the old motion (the
    /// user's request). Settings (speeds, lane assist) stay.
    pub fn jumped(&mut self) {
        self.stop_motion();
        self.journey.stop();
    }

    fn stop_motion(&mut self) {
        self.autopilot.halt();
        self.cruise.halt();
        self.flight = None;
        self.pan = [Spring::default(); 2];
        self.zoom_rate = Spring::default();
        self.stop_flings();
        self.step_left = 0.0;
        self.step_at = None;
        self.rot_rate = Spring::default();
        self.rot_step_left = 0.0;
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
        if !event.is_finite() || !now.is_finite() {
            return false;
        }
        let event = match event {
            InputEvent::Resize { width, height } => InputEvent::Resize {
                width: width.max(0.0),
                height: height.max(0.0),
            },
            e => e,
        };
        let was_animating = self.is_animating();
        let seeking = self.seek.is_searching();
        if self.ends_seek(&event) {
            self.seek.clear();
        }
        // Esc while seeking was handled (it cancelled the search).
        let handled = self.apply(event, viewport, now) || (seeking && event == InputEvent::KeyDown(NavKey::Stop));
        if !was_animating && self.is_animating() {
            // Motion starts now: the first frame integrates from this event, not from the
            // last frame before the display link paused (that was up to 0.1 s of jump).
            self.last_tick = Some(now);
        }
        handled
    }

    fn apply(&mut self, event: InputEvent, viewport: &mut Viewport, now: f64) -> bool {
        // At a journey stop J / ⇧J, Z and X choose the next leg (kb: decisions/controls
        // "Journey with stops"); everything else works as without a journey.
        if self.journey.at_stop() {
            let choice = match event {
                InputEvent::KeyDown(NavKey::Journey | NavKey::JourneyAlt) => Some(Choice::Next),
                InputEvent::KeyDown(NavKey::Autopilot) => Some(Choice::Deeper),
                InputEvent::KeyDown(NavKey::Elsewhere) => Some(Choice::Elsewhere),
                _ => None,
            };
            if let Some(c) = choice {
                return self.journey_choose(c, viewport, now);
            }
        }
        // User steering takes the wheel from the autopilot or a journey's flight: cruise goes on
        // at the current speed (kb: decisions/controls "Driving levels").
        if Self::is_steering(&event) && self.automatic_steerable() {
            self.take_wheel(None, now);
        }
        // Manual input takes over from a journey (eased: the flight's motion glides out); J,
        // the button or a click resumes it. Not at a stop: looking around there is allowed.
        if self.journey.is_running()
            && !self.journey.at_stop()
            && Self::cancels_flight(&event)
            && !matches!(event, InputEvent::KeyDown(NavKey::Journey | NavKey::JourneyAlt))
        {
            self.journey.suspend();
        }
        if self.flight.is_some() && Self::cancels_flight(&event) {
            self.hand_over_flight();
        }
        // Turning by hand ends the compass's turn to north.
        if matches!(
            event,
            InputEvent::Rotate { .. } | InputEvent::KeyDown(NavKey::RotateCcw | NavKey::RotateCw | NavKey::Straighten)
        ) {
            self.north = None;
        }
        if Self::cancels_flight(&event) && event != InputEvent::KeyDown(NavKey::Autopilot) {
            self.manual_at = now;
        }
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
                // A flight continues from where it is, planned for the new width (the
                // zoom-out autopilot to the new home view).
                if let Some(f) = self.flight.take() {
                    let mut target = f.path.target().clone();
                    if self.cruise.landing().is_some() {
                        target = Viewport::fitting(width, height).with_angle(target.angle);
                    }
                    let mut next = Flight::new(FlyPath::new(viewport, &target, width));
                    next.soft_landing = f.soft_landing;
                    // The turn continues from here to where it would have ended.
                    next.angle0 = viewport.angle;
                    next.angle1 = viewport.angle + (f.angle1 - f.angle0) * (1.0 - f.progress());
                    next.land_angle = f.land_angle;
                    next.speed = f.speed;
                    next.stopping = f.stopping;
                    self.flight = Some(next);
                }
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
                self.pointer(p, now);
                true
            }
            InputEvent::Scroll { dx, dy, momentum } => {
                if self.flight.is_some() || self.journey.wants_frames() {
                    if !momentum {
                        self.scale_flight_speed((dy * autopilot::SCROLL_PER_POINT).exp(), now);
                    }
                    true
                } else if self.autopilot.is_active() {
                    // Scrolling sets the speed; the OS's momentum after the fingers lift does not.
                    if !momentum {
                        self.autopilot
                            .scale_speed((dy * autopilot::SCROLL_PER_POINT).exp(), now);
                    }
                    true
                } else if self.scroll_zooms {
                    // Fingers up zoom in, like the wheel; the momentum after lifting coasts on.
                    if dy != 0.0 && dy.is_finite() {
                        self.manual_zoom_ends_cruise();
                        let (tx, ty) = self.zoom_target();
                        self.zoom_at(viewport, (dy * SCROLL_ZOOM_PER_POINT).exp(), tx, ty, now);
                    }
                    true
                } else {
                    self.apply(InputEvent::Pan { dx, dy }, viewport, now)
                }
            }
            InputEvent::Click { .. } if self.journey.is_active() && !self.journey.at_stop() => {
                // A click pauses a journey (its flight glides out) and resumes it.
                if self.journey.is_running() {
                    self.journey.suspend();
                    self.hand_over_flight();
                } else {
                    self.resume_journey(viewport, now);
                }
                true
            }
            InputEvent::Click { .. } => self.autopilot.click(now),
            InputEvent::Rotate { radians, x, y } => {
                if radians.is_finite() {
                    self.note_interaction(now);
                    let (tx, ty) = self.gesture_target(x, y);
                    // The image follows the fingers: counter-clockwise fingers, the camera turns clockwise.
                    viewport.rotate_by(-radians, tx, ty);
                }
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
                self.manual_zoom_ends_cruise();
                let (tx, ty) = self.gesture_target(x, y);
                self.zoom_at(viewport, 1.15f64.powf(delta), tx, ty, now);
                true
            }
            InputEvent::WheelNotch { delta, .. } if self.flight.is_some() || self.journey.wants_frames() => {
                self.scale_flight_speed(autopilot::NOTCH_FACTOR.powf(delta), now);
                true
            }
            InputEvent::WheelNotch { delta, .. } if self.autopilot.is_active() => {
                self.autopilot.scale_speed(autopilot::NOTCH_FACTOR.powf(delta), now);
                true
            }
            InputEvent::WheelNotch { delta, x, y } => {
                self.manual_zoom_ends_cruise();
                let (tx, ty) = self.gesture_target(x, y);
                self.step_zoom(1.15f64.powf(delta), tx, ty, now);
                true
            }
            InputEvent::Magnify { magnification, x, y } => {
                self.manual_zoom_ends_cruise();
                self.stop_flings();
                let (tx, ty) = self.gesture_target(x, y);
                self.zoom_at(viewport, 1.0 + magnification, tx, ty, now);
                self.sample_gesture(now, 0.0, 0.0, (1.0 + magnification).ln(), (tx, ty));
                true
            }
            InputEvent::Pinch { ratio, x, y, dx, dy } => {
                self.manual_zoom_ends_cruise();
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
                self.manual_zoom_ends_cruise();
                // Its first click toggled the autopilot's pause: undone.
                self.autopilot.double_click(now);
                self.enter_pointer_mode();
                self.step_zoom(if out { 0.5 } else { 2.0 }, x, y, now);
                true
            }
            InputEvent::ZoomBy { factor } => {
                self.manual_zoom_ends_cruise();
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

    fn pointer(&mut self, p: Option<(f64, f64)>, now: f64) {
        if p.is_some() && p != self.pointer {
            self.pointer_moved = now;
        }
        if let (Some(a), Some(b)) = (self.pointer, p) {
            self.wheel_travel += (b.0 - a.0).hypot(b.1 - a.1);
        }
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
            NavKey::RotateCcw => self.rot_ccw = down,
            NavKey::RotateCw => self.rot_cw = down,
            NavKey::Straighten if down => {
                self.rot_rate = Spring::default();
                self.rot_step_left = -viewport.angle;
            }
            NavKey::Journey | NavKey::JourneyAlt if down => {
                if self.journey.is_suspended() {
                    self.resume_journey(viewport, now);
                } else if self.journey.is_active() {
                    self.stop_journey();
                } else {
                    let seed = random_seed(now);
                    let stops = self.journey_stops != (k == NavKey::JourneyAlt);
                    self.start_journey_mode(seed, false, stops, viewport, now);
                }
            }

            // X means something only at a journey stop (handled before).
            NavKey::Elsewhere => return false,
            NavKey::SeekMinibrot if down => {
                self.seek.start(viewport, self.min_side(), now);
                self.poll_seek(viewport, now);
            }
            // Q / E: the Space speed while Space is held, a fly-to's speed, else the pedals
            // (kb: decisions/controls "Driving levels").
            NavKey::SlowerZoom | NavKey::FasterZoom => {
                let dir = if k == NavKey::FasterZoom { 1.0 } else { -1.0 };
                if !down {
                    if self.pedal_hold.is_some_and(|(d, _)| d == dir) {
                        self.pedal_hold = None;
                    }
                } else if self.zoom {
                    let f = if dir > 0.0 { 1.25 } else { 0.8 };
                    self.set_zoom_speed(self.zoom_speed * f, now);
                } else if self.flight.is_some() && !self.journey.is_running() && self.cruise.landing().is_none() {
                    let f = if dir > 0.0 {
                        autopilot::SPEED_STEP
                    } else {
                        1.0 / autopilot::SPEED_STEP
                    };
                    self.scale_flight_speed(f, now);
                } else {
                    self.pedal(dir, true, viewport, now);
                    self.pedal_hold = Some((dir, now + cruise::HOLD_DELAY));
                }
            }
            NavKey::Autopilot if down => {
                self.journey.stop();
                if self.cruise.is_engaged() {
                    // C → A: the cruise brakes out on its springs while the autopilot eases in,
                    // at the cruise's speed when it went forward.
                    let s = self.cruise.set_speed();
                    if self.cruise.landing().is_some() {
                        self.hand_over_flight();
                    }
                    self.cruise.release();
                    if s > 0.0 {
                        self.autopilot.set_speed(s, now);
                    }
                    if !self.autopilot.is_active() {
                        self.autopilot.toggle();
                    }
                } else {
                    self.autopilot.toggle();
                }
                self.reset_wheel();
            }
            NavKey::Stop if down && self.cruise.is_engaged() => {
                // Esc in cruise: brake smoothly to rest (a landing: an eased stop where it is).
                self.cruise.stop(now);
                if self.cruise.landing().is_some() {
                    self.stop_flight();
                }
            }
            NavKey::Stop => {
                return if down {
                    let j = self.journey.is_active();
                    self.journey.stop();
                    let a = self.autopilot.stop();
                    let f = self.stop_flight();
                    a || f || j
                } else {
                    false
                };
            }
            NavKey::StepIn | NavKey::StepOut if down => {
                let (tx, ty) = self.zoom_target();
                let f = if k == NavKey::StepIn { 1.5 } else { 1.0 / 1.5 };
                self.step_zoom(f, tx, ty, now);
            }
            NavKey::Reset if down => {
                self.stop_motion();
                self.journey.stop();
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
        // The autopilot and the cruise steer with the pointer instead.
        if !active || self.mode != InputMode::Pointer || self.autopilot.is_active() || self.cruise.is_engaged() {
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
            // NaN (a bad timestamp) counts as no time (clamp keeps NaN).
            Some(t) => (now - t).clamp(0.0, 0.1).max(0.0),
            None => 1.0 / 60.0,
        };
        self.last_tick = Some(now);
        self.poll_seek(viewport, now);
        let gliding = self.is_gliding();
        // The zoom follows the crosshair while it glides: the target is where it is drawn.
        self.glide(dt);
        // A held pedal keeps stepping (repeats stop at the detent; at most 16 per frame).
        if let Some((dir, mut next)) = self.pedal_hold {
            let mut k = 0;
            while now >= next && k < 16 {
                self.pedal(dir, false, viewport, now);
                next += cruise::HOLD_REPEAT;
                k += 1;
            }
            if now >= next {
                next = now + cruise::HOLD_REPEAT;
            }
            self.pedal_hold = Some((dir, next));
        }
        // Sustained pointer steering takes the wheel from a journey's flight. In the autopilot the
        // pointer is a direction wish instead (kb: decisions/controls "Pointer wish"); W A S D,
        // a drag, the stick and the pedal to zero still hand over there.
        if self.journey_flying() && self.pointer_deflected(now) {
            let since = *self.deflected_since.get_or_insert(now);
            if now - since >= STEER_ENGAGE {
                self.take_wheel(None, now);
                self.deflected_since = None;
            }
        } else {
            self.deflected_since = None;
        }
        // The journey starts its next leg when one is due (after a cruise's glide, below).
        let cruise_glide = self.cruise_gliding();
        if self.journey.is_running()
            && !(cruise_glide && self.leg_waits_for_cruise(viewport))
            && let Some(journey::Action::Fly(t)) = self.journey.tick(now, self.flight.is_some())
        {
            let mut target = t.viewport(self.min_side());
            target.angle = viewport.angle;
            let speed = self.flight.as_ref().map(|f| f.speed);
            let carry = if cruise_glide { self.cruise.rate() } else { 0.0 };
            self.cruise.halt();
            self.reset_wheel();
            let from = viewport.clone();
            self.begin_flight(&target, &from);
            if let Some(f) = &mut self.flight {
                if let Some(s) = speed {
                    f.speed = s;
                } else if carry != 0.0 && carry.signum() == flyto_start_sign(&f.path) {
                    // The cruise's zoom carries on into the leg (its path starts the same way).
                    f.speed = Spring {
                        v: carry.abs() / flyto::RHO,
                        a: 0.0,
                    };
                }
            }
            // Long legs (cross-overs from deep dives) fly faster.
            let efolds = self
                .flight
                .as_ref()
                .map_or(0.0, |f| f.path.length() * crate::flyto::RHO);
            self.journey.set_leg_efolds(efolds);
        }
        // A flight sets the view from its path parameter (exact, nothing integrates).
        if self.flight.is_some() {
            let speed = self.flight_speed();
            let f = self.flight.as_mut().expect("flight");
            self.last_interaction = now;
            self.autopilot.update_throttle(dt);
            let done = f.step(dt, speed, self.autopilot.throttle());
            *viewport = f.viewport();
            let stopped = f.stopping;
            self.step_north(dt, viewport);
            if done {
                self.flight = None;
                self.seek.landed(now, stopped);
                if self.cruise.landing().is_some() {
                    // The cruise's soft landing rests on the home view (or stopped on the way).
                    self.cruise.end_landing();
                } else if stopped {
                    self.journey.stop();
                }
            }
            return true;
        }
        if self.journey.wants_frames() && !self.cruise.is_moving() {
            // Resting at a target (it refines) or waiting for the plan (a released cruise
            // glides out first, below).
            self.step_north(dt, viewport);
            return true;
        }
        if !self.is_moving() {
            // A minibrot search wants frames (to be polled), not motion.
            return gliding || self.seek.is_searching();
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
        // Autopilot: steered by the pointer while it is on the canvas, moving, in pointer mode.
        let steer_pointer = match self.mode {
            InputMode::Pointer if now - self.pointer_moved < autopilot::STEER_IDLE => self.pointer_on_canvas(),
            _ => None,
        };
        let (x0, x1, y0, y1) = self.canvas();
        let radius = 0.5 * (x1 - x0).min(y1 - y0);
        // Manual motion (pans, drags, flings, held keys) holds the target lock off.
        let manual = now - self.manual_at < autopilot::MANUAL_HOLD
            || tx != 0.0
            || ty != 0.0
            || self.fling != (0.0, 0.0)
            || self.driven();
        // The autopilot does not steer by the pointer; its direction is a wish for the targets.
        let wish_pointer = match self.mode {
            InputMode::Pointer if self.wheel_travel >= WISH_TRAVEL => self.pointer_on_canvas(),
            _ => None,
        };
        self.autopilot.set_wish(wish_pointer, radius, now);
        let target = self.autopilot.aim(viewport, self.min_side(), radius, None, manual, now);
        let ap = self.autopilot.step(dt, target, now);
        // Cruise: steered by the pointer only (and lane assist's nudge while the user does not
        // steer); the depth boost in reverse.
        let cruise_pointer = if self.cruise.is_engaged() { steer_pointer } else { None };
        let mut steer = self.autopilot.steer_target(cruise_pointer, radius);
        if self.cruise.is_engaged()
            && self.assist
            && steer == (0.0, 0.0)
            && !manual
            && let Some(n) = self.assist_nudge(viewport, radius, now)
        {
            steer = n;
            self.assist_nudged = now;
        }
        let depth = self.depth(viewport);
        let cm = self.cruise.step(
            dt,
            steer,
            depth,
            self.autopilot.throttle(),
            self.autopilot.feedback().at_limit,
        );
        let v = &mut self.pan;
        if tx == 0.0 && ty == 0.0 && v[0].v.hypot(v[1].v) < PAN_REST && v[0].a.hypot(v[1].a) < PAN_REST * PAN_OMEGA.1 {
            *v = [Spring::default(); 2];
        }
        let z = &mut self.zoom_rate;
        if tz == 0.0 && z.v.abs() < ZOOM_REST && z.a.abs() < ZOOM_REST * ZOOM_OMEGA.1 {
            *z = Spring::default();
        }
        // Rotate keys: the camera turns opposite to the image.
        let tr = ROTATE_SPEED * (f64::from(u8::from(self.rot_cw)) - f64::from(u8::from(self.rot_ccw)));
        let romega = if tr.abs() > self.rot_rate.v.abs() {
            ROTATE_OMEGA.0
        } else {
            ROTATE_OMEGA.1
        };
        let mut turn = self.rot_rate.follow(tr, romega, dt);
        let r = &mut self.rot_rate;
        if tr == 0.0 && r.v.abs() < ROTATE_REST && r.a.abs() < ROTATE_REST * ROTATE_OMEGA.1 {
            *r = Spring::default();
        }
        if self.rot_step_left != 0.0 {
            let mut part = self.rot_step_left * (1.0 - (-dt / STRAIGHTEN_TAU).exp());
            if (self.rot_step_left - part).abs() < 1e-5 {
                part = self.rot_step_left;
            }
            self.rot_step_left -= part;
            turn += part;
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
        dx += ap.pan.0 + cm.pan.0;
        dy += ap.pan.1 + cm.pan.1;
        if dx != 0.0 || dy != 0.0 {
            // Pan and zoom run together: pan at the step's midpoint scale, so a frame's
            // order of operations does not show as a frame-rate dependence.
            let mid = (-0.5 * (ln_zoom + ap.ln_zoom + cm.ln_zoom)).exp();
            viewport.pan_by_points(dx * mid, dy * mid);
        }
        if ln_zoom != 0.0 {
            let (x, y) = self.zoom_target();
            viewport.zoom_by(ln_zoom.exp(), x, y);
            ln_zoom = 0.0;
        }
        if ap.ln_zoom != 0.0 {
            viewport.zoom_by(ap.ln_zoom.exp(), ap.at.0, ap.at.1);
            // Cinematic spin: the view turns with the autopilot's zoom, about its dive point.
            let spin = self.cinematic.spin_per_efold();
            if spin != 0.0 {
                viewport.rotate_by(spin * ap.ln_zoom, ap.at.0, ap.at.1);
            }
        }
        if cm.ln_zoom != 0.0 {
            viewport.zoom_by(cm.ln_zoom.exp(), cm.at.0, cm.at.1);
        }
        self.cruise_landing(viewport);
        if turn != 0.0 {
            let (x, y) = self.zoom_target();
            viewport.rotate_by(turn, x, y);
            if self.rot_step_left == 0.0 && viewport.angle.abs() < 1e-12 && !self.rot_ccw && !self.rot_cw {
                viewport.angle = 0.0;
            }
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
        // Last, so it absorbs this frame's spin and ends at exactly north.
        self.step_north(dt, viewport);
        true
    }

    /// Hands a reverse cruise near home to a soft-landing flight onto the home view fitted to
    /// the window (kb: decisions/controls "Driving levels"): when the path home is no longer
    /// than [`cruise::LAND_LEAD`] seconds of the current rate (its brake curve then matches the
    /// rate), or, braking, when the glide would reach home; always once the depth is gone.
    fn cruise_landing(&mut self, viewport: &Viewport) {
        if !self.cruise.is_engaged() || self.cruise.landing().is_some() || self.cruise.rate() >= 0.0 {
            return;
        }
        let r = -self.cruise.rate();
        let depth = self.depth(viewport);
        let lead = if self.cruise.set_speed() < 0.0 {
            cruise::LAND_LEAD * r
        } else {
            2.0 / autopilot::RATE_OMEGA.1 * r
        };
        if depth > lead + 1.0 {
            return;
        }
        let home = Viewport::fitting(self.view_size.0, self.view_size.1);
        let path = FlyPath::new(viewport, &home, self.view_size.0);
        if depth > 0.0 && path.length() * flyto::RHO > lead.max(0.02) {
            return;
        }
        self.begin_flight(&home, viewport);
        if let Some(f) = &mut self.flight {
            f.soft_landing = true;
            f.speed = Spring {
                v: r / flyto::RHO,
                a: 0.0,
            };
            if self.cruise.set_speed() == 0.0 {
                // Braking: it rests on home only if the glide would have got there.
                f.stopping = false;
            }
        }
        self.cruise.begin_landing(r);
    }

    // MARK: Compass (kb: decisions/controls "Compass"; geometry in `crate::compass`)

    /// Starts the compass's eased turn back to north (a click on it), ~0.4 s. During the
    /// autopilot, a flight or a journey the view straightens and their spin goes on from north.
    /// A turn already under way goes on (a double click does not restart it).
    pub fn turn_north(&mut self, now: f64) {
        if self.north.is_some() {
            return;
        }
        let was_animating = self.is_animating();
        self.rot_step_left = 0.0;
        self.rot_rate = Spring::default();
        self.north = Some(crate::compass::NorthTurn::default());
        self.note_interaction(now);
        if !was_animating {
            self.last_tick = Some(now);
        }
    }

    /// The compass's turn to north is under way.
    pub fn is_turning_north(&self) -> bool {
        self.north.is_some()
    }

    /// Turns the view by `delta` radians about its centre (dragging the compass): a flight in
    /// progress turns along (it keeps flying and spinning from the new angle). Ends a turn to
    /// north.
    pub fn turn_view(&mut self, delta: f64, viewport: &mut Viewport, now: f64) {
        self.north = None;
        self.rotate_about_centre(delta, viewport);
        self.note_interaction(now);
    }

    fn rotate_about_centre(&mut self, delta: f64, viewport: &mut Viewport) {
        if !delta.is_finite() || delta == 0.0 {
            return;
        }
        viewport.angle = wrap_angle(viewport.angle + delta);
        if let Some(f) = &mut self.flight {
            f.shift_angle(delta);
        }
    }

    fn step_north(&mut self, dt: f64, viewport: &mut Viewport) {
        let Some(mut turn) = self.north else { return };
        let a = viewport.angle;
        let (b, done) = turn.step(a, dt);
        self.rotate_about_centre(b - a, viewport);
        if done {
            // Exactly north (a + (0 − a) is 0, but wrapping may round).
            viewport.angle = 0.0;
            self.north = None;
        } else {
            self.north = Some(turn);
        }
    }

    /// Insets of the canvas from the view edges (top, left, bottom, right; view points): the
    /// title bar or toolbar overlapping the view (overlays keep clear of them).
    pub fn insets(&self) -> (f64, f64, f64, f64) {
        self.insets
    }
}

/// The direction a flight path starts in: +1 zooming in, −1 zooming out (0: no zoom).
fn flyto_start_sign(path: &FlyPath) -> f64 {
    let h = path.length().min(1e-3);
    if h <= 0.0 {
        return 0.0;
    }
    let d = path.state(0.0).1 - path.state(h).1;
    if d > 0.0 {
        1.0
    } else if d < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// What the HUD cockpit shows (kb: decisions/controls "Driving levels").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cockpit {
    pub level: Level,
    /// The level's set speed (signed e-folds per second, + in) and the zoom rate now.
    pub set: f64,
    pub rate: f64,
    /// The reverse cruise's depth boost (1: none).
    pub boost: f64,
    /// The cruise's soft landing on the home view flies.
    pub landing: bool,
    /// Lane assist is on, and nudged just now.
    pub assist: bool,
    pub nudging: bool,
}

/// A journey seed for J: 1 ... 999 999 from the clock (short enough to read out and share).
pub fn random_seed(now: f64) -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let mut r = journey::Rng::new(t ^ now.to_bits());
    1 + r.next_u64() % 999_999
}

/// The span (shorter side) of `viewport` in a view of `view` points.
pub fn span_of(viewport: &Viewport, view: (f64, f64)) -> Scale {
    viewport.scale * view.0.min(view.1).max(1.0)
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

    /// Lane assist: nothing while the centre shows detail; a gentle heading towards the
    /// boundary point once the region ahead is uniform; nothing on a stale survey.
    #[test]
    fn lane_assist_nudges_only_when_the_nothing_is_ahead() {
        use crate::autopilot::{Survey, SurveyCell};
        let (mut n, v) = setup();
        let survey = |centre_edges: u32, time: f64| {
            let mut cells = vec![
                SurveyCell {
                    samples: 100,
                    interior: 0,
                    pairs: 200,
                    edges: centre_edges,
                    ..Default::default()
                };
                160
            ];
            // One detailed cell off to the upper right, its peak at (300, 200).
            cells[2 * 16 + 13] = SurveyCell {
                samples: 100,
                interior: 50,
                pairs: 200,
                edges: 100,
                peak: (300.0, 200.0),
                peak_n: 5000.0,
            };
            Survey {
                cols: 16,
                rows: 10,
                cells,
                view: (1000.0, 600.0),
                texels_across: 400.0,
                texels_down: 240.0,
                time,
                viewport: None,
            }
        };
        let r = 300.0;
        n.autopilot_mut().set_survey(survey(100, 1.0));
        assert_eq!(n.assist_nudge(&v, r, 1.1), None);
        n.autopilot_mut().set_survey(survey(0, 1.0));
        let h = n.assist_nudge(&v, r, 1.1).expect("nudge");
        assert!(h.0 > 0.0 && h.1 > 0.0 && dist(h) <= ASSIST_REACH * r + 1e-9, "{h:?}");
        assert!((h.1 / h.0 - 200.0 / 300.0).abs() < 1e-9);
        assert_eq!(n.assist_nudge(&v, r, 1.0 + 2.0 * ASSIST_SURVEY_AGE), None);
    }

    /// Setting "Two-finger scroll: Zoom": up zooms in, down out, sideways nothing; default pans.
    #[test]
    fn two_finger_scroll_zooms_when_set() {
        let (mut n, mut v) = setup();
        let s0 = v.scale.to_f64();
        let c0 = v.center.clone();
        n.handle(
            InputEvent::Scroll {
                dx: 40.0,
                dy: 0.0,
                momentum: false,
            },
            &mut v,
            0.0,
        );
        assert_ne!(v.center, c0, "default: scrolling pans");
        n.set_scroll_zooms(true);
        let (mut n2, mut v2) = setup();
        n2.set_scroll_zooms(true);
        n2.handle(
            InputEvent::Scroll {
                dx: 40.0,
                dy: 0.0,
                momentum: false,
            },
            &mut v2,
            0.0,
        );
        assert!((v2.scale.to_f64() - s0).abs() < 1e-15, "sideways does nothing");
        n2.handle(
            InputEvent::Scroll {
                dx: 0.0,
                dy: 100.0,
                momentum: false,
            },
            &mut v2,
            0.0,
        );
        let s1 = v2.scale.to_f64();
        assert!((s0 / s1 - 1f64.exp()).abs() < 1e-9, "{}", s0 / s1);
        n2.handle(
            InputEvent::Scroll {
                dx: 0.0,
                dy: -100.0,
                momentum: false,
            },
            &mut v2,
            0.0,
        );
        assert!((v2.scale.to_f64() / s0 - 1.0).abs() < 1e-9);
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
                out.push(((v.center.re() - re0) / 1e-3, v.scale.to_f64().ln()));
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
        assert!((v.scale.to_f64() - expect).abs() < 1e-15);
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
        assert!(v.scale.to_f64() < 1e-4);
        let (dr, di) = v.point_at_offset(p.0, p.1).delta_from(&target);
        assert!(
            dr.hypot(di) / v.scale.to_f64() < 1e-6,
            "drift {} points",
            dr.hypot(di) / v.scale.to_f64()
        );

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
        assert!(dr.hypot(di) / v.scale.to_f64() < 1e-6);
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
        let frac = (1e-3 / v.scale.to_f64()).ln() / 2f64.ln();
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
        assert!(
            (v.scale.to_f64() - 1e-3 / 2.0 / 1.15).abs() < 1e-12,
            "{}",
            v.scale.to_f64()
        );
    }

    #[test]
    fn drag_release_flings_and_decays() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        // A drag at 600 pt/s, 120 Hz events, released while moving.
        let mut t = 0.0;
        for _ in 0..cruise::LADDER.len() + 2 {
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
        assert!(dr.hypot(di) / v.scale.to_f64() < 1e-6);
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

    // Autopilot (kb: decisions/controls, autopilot). The setup's canvas is 1000 x 600 points:
    // steering radius 300, dead zone 30.

    /// Screen offset (points) of the complex point `c` in the view.
    fn screen_of(v: &Viewport, c: &crate::viewport::Center) -> (f64, f64) {
        c.delta_in(&v.center, v.scale)
    }

    /// The setup with the target lock off: the pure joystick.
    fn joystick() -> (Navigator, Viewport) {
        let (mut n, v) = setup();
        n.autopilot_mut().set_lock(false);
        (n, v)
    }

    /// The setup with the target lock searching on the calling thread (reproducible).
    fn locked() -> (Navigator, Viewport) {
        let (mut n, v) = setup();
        n.autopilot_mut().lock_mut().synchronous = true;
        (n, v)
    }

    /// The span across the view's shorter side (the setup's canvas is 1000 x 600).
    fn span_of(v: &Viewport) -> crate::viewport::Scale {
        v.scale * 600.0
    }

    /// The CPU probe of the view: (band crossings per width, interior share).
    fn probe_view(v: &Viewport) -> (f64, f64) {
        let st = journey::ProbeStats::of(&journey::probe(&v.center, span_of(v), 0));
        (st.detail, st.interior)
    }

    #[test]
    fn autopilot_lock_aims_at_boundary_targets_and_retargets_ahead() {
        let (mut n, mut v) = locked();
        n.handle(InputEvent::Pointer(None), &mut v, 0.0);
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
        let mut t = 0.0;
        let mut last_heading = (0.0, 0.0);
        let mut worst_step: f64 = 0.0;
        let mut labels: Vec<String> = Vec::new();
        for _ in 0..(20 * 60) {
            t = run(&mut n, &mut v, t, t + 1.0 / 60.0, 60.0);
            let h = n.autopilot().heading();
            worst_step = worst_step.max(dist((h.0 - last_heading.0, h.1 - last_heading.1)));
            last_heading = h;
            if let Some(lt) = n.autopilot().lock().target() {
                if labels.last() != Some(&lt.label) {
                    labels.push(lt.label.clone());
                }
                // Never far past a target's end: the next one was ready before.
                assert!(
                    span_of(&v).ratio(lt.end_span).ln() > -1.5,
                    "{} passed by {:.2} e-folds at {t:.2}",
                    lt.label,
                    -span_of(&v).ratio(lt.end_span).ln()
                );
            }
        }
        let a = n.autopilot();
        assert!(a.lock().switches >= 3, "retargeted: {labels:?}");
        assert!(!a.is_rescuing());
        // Steering eased: no jumps of the heading between frames (points at 60 Hz).
        assert!(worst_step < 20.0, "heading jumped {worst_step:.1} points in a frame");
        let depth = (1e-3 / v.scale.to_f64()).ln();
        assert!(depth > 14.0, "{depth} e-folds");
        let (detail, interior) = probe_view(&v);
        assert!(
            detail >= 3.0 && interior < 0.6,
            "ended in a uniform view: {detail} {interior} ({labels:?})"
        );
    }

    #[test]
    fn autopilot_lock_is_frame_rate_independent() {
        let path = |hz: f64| {
            let (mut n, mut v) = locked();
            n.handle(InputEvent::Pointer(None), &mut v, 0.0);
            n.advance(&mut v, 0.0);
            n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
            run(&mut n, &mut v, 0.0, 8.0, hz);
            let label = n.autopilot().lock().target().map(|t| t.label.clone());
            (v, label)
        };
        let (a, la) = path(30.0);
        for hz in [60.0, 120.0] {
            let (b, lb) = path(hz);
            assert_eq!(la, lb, "same target at {hz} Hz");
            // The lock's aim (and the zoom's slowdown while it turns towards a far target) is set
            // per frame, so 30, 60 and 120 Hz agree to first order: a small share of the view.
            let dz = (a.scale.ln() - b.scale.ln()).abs();
            assert!(dz < 0.02, "{hz} Hz: zoom {dz} e-folds apart");
            let d = b.center.distance_in(&a.center, span_of(&a));
            assert!(d < 0.01, "{hz} Hz: {d} of the view apart");
        }
    }

    #[test]
    fn autopilot_eases_in_cruises_and_glides_to_a_stop() {
        let (mut n, mut v) = joystick();
        n.advance(&mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
        n.handle(InputEvent::KeyUp(NavKey::Autopilot), &mut v, 0.0);
        assert!(n.autopilot().is_active() && n.is_moving());
        let speed = autopilot::SPEED_DEFAULT;
        // Eased start: far from the set speed after 0.3 s.
        let t = run(&mut n, &mut v, 0.0, 0.3, 60.0);
        let r = n.autopilot().rate();
        assert!(r > 0.1 * speed && r < 0.6 * speed, "{r}");
        let t = run(&mut n, &mut v, t, 4.0, 60.0);
        assert!((n.autopilot().rate() - speed).abs() < 1e-3 * speed);
        // Zoomed by the spring's exact integral (critically damped from rest: −2/ω of a second).
        let expect = ease_in_distance(speed, autopilot::RATE_OMEGA.0, 4.0);
        assert!(((1e-3 / v.scale.to_f64()).ln() - expect).abs() < 1e-9);
        // Esc: an eased stop; Esc again is not ours (the platform keeps it).
        assert!(n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t));
        assert!(!n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t));
        let ln0 = v.scale.ln();
        let mut last = n.autopilot().rate();
        let mut t = t;
        let mut frames = 0;
        while n.advance(&mut v, t + 1.0 / 60.0) {
            t += 1.0 / 60.0;
            frames += 1;
            let r = n.autopilot().rate();
            assert!(r <= last + 1e-12 && r >= 0.0, "not a monotonic glide: {r} after {last}");
            last = r;
            assert!(frames < 600, "never stopped");
        }
        // It glides on 2/ω seconds' worth of the rate, and is at rest within ~2 s.
        let glide = ln0 - v.scale.ln();
        let expect = 2.0 / autopilot::RATE_OMEGA.1 * speed;
        assert!((glide - expect).abs() < 0.01 * expect, "glide {glide} vs {expect}");
        assert!((frames as f64) / 60.0 < 2.5, "{frames} frames");
        assert_eq!(n.autopilot().rate(), 0.0);
    }

    #[test]
    fn autopilot_dead_zone_and_no_pointer_fly_straight() {
        for pointer in [Some((20.0, -15.0)), None] {
            let (mut n, mut v) = joystick();
            n.handle(InputEvent::Pointer(pointer), &mut v, 0.0);
            let c0 = v.center.clone();
            n.advance(&mut v, 0.0);
            n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
            run(&mut n, &mut v, 0.0, 2.0, 60.0);
            assert!(v.scale.to_f64() < 0.3e-3, "zoomed in");
            // Zooming about the centre: the centre's coordinate is exact.
            assert_eq!(v.center.delta_from(&c0), (0.0, 0.0), "{pointer:?}");
            assert_eq!(n.autopilot().heading(), (0.0, 0.0));
        }
        // Keyboard mode: straight too (W A S D pan as usual).
        let (mut n, mut v) = joystick();
        n.handle(InputEvent::Pointer(Some((250.0, 0.0))), &mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Up), &mut v, 0.0);
        n.handle(InputEvent::KeyUp(NavKey::Up), &mut v, 0.0);
        n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
        run(&mut n, &mut v, 0.0, 1.0, 60.0);
        assert_eq!(n.autopilot().heading_target(), (0.0, 0.0));
    }

    #[test]
    fn fly_to_lands_exactly_speed_keys_and_cancel() {
        let target = crate::flyto::view_from_text("-0.743643887037151", "0.13182590420533", "1e-20").unwrap();
        // Lands exactly, Q / E change the flight's speed (not Space's).
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        n.fly_to(&target, &v.clone(), 0.0);
        assert!(n.is_moving() && n.fly_target().is_some());
        n.handle(InputEvent::KeyDown(NavKey::FasterZoom), &mut v, 0.0);
        assert!((n.fly_speed() - crate::flyto::SPEED_DEFAULT * autopilot::SPEED_STEP).abs() < 1e-12);
        assert!((n.zoom_speed() - 1.6).abs() < 1e-12);
        let mut t = 0.0;
        while n.advance(&mut v, t + 1.0 / 60.0) {
            t += 1.0 / 60.0;
            assert!(t < 60.0, "never landed");
        }
        assert!(n.flight().is_none());
        assert_eq!(v, target);
        // Esc: eased stop on the path, then idle.
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        n.fly_to(&target, &v.clone(), 0.0);
        let t = run(&mut n, &mut v, 0.0, 2.0, 60.0);
        assert!(n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t));
        let s0 = v.scale.ln();
        let t = run(&mut n, &mut v, t, t + 3.5, 60.0);
        assert!(n.flight().is_none() && !n.advance(&mut v, t + 0.01));
        assert!(v.scale.ln() < s0 && v.scale.to_f64() > 1e-19, "glided, stopped short");
        // A pan takes over: the flight's motion glides out in the springs, no jump.
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        n.fly_to(&target, &v.clone(), 0.0);
        let t = run(&mut n, &mut v, 0.0, 3.0, 60.0);
        let rate = n.velocity().2;
        let before = v.clone();
        n.handle(InputEvent::Pan { dx: 1.0, dy: 0.0 }, &mut v, t);
        assert!(n.flight().is_none());
        assert!((n.velocity().2 - rate).abs() < 1e-12 || rate == 0.0);
        n.advance(&mut v, t + 1.0 / 60.0);
        let step = before.scale.ln() - v.scale.ln();
        assert!(step > 0.0 && step < 0.1, "continues smoothly: {step}");
        run(&mut n, &mut v, t + 1.0 / 60.0, t + 2.0, 60.0);
        assert!(!n.is_moving());
    }

    #[test]
    fn rotate_keys_ease_and_are_frame_rate_independent() {
        let mut angles = Vec::new();
        for hz in [30.0, 60.0, 120.0] {
            let (mut n, mut v) = setup();
            n.advance(&mut v, 0.0);
            n.handle(InputEvent::KeyDown(NavKey::RotateCw), &mut v, 0.0);
            let t = run(&mut n, &mut v, 0.0, 1.0, hz);
            n.handle(InputEvent::KeyUp(NavKey::RotateCw), &mut v, t);
            run(&mut n, &mut v, t, t + 1.0, hz);
            assert!(!n.is_moving());
            angles.push(v.angle);
        }
        // About 45 degrees per second minus the ease-in plus the glide.
        assert!((angles[0] - ROTATE_SPEED).abs() < 0.1, "{angles:?}");
        for a in &angles {
            assert!((a - angles[0]).abs() < 1e-6, "{angles:?}");
        }
        // Straighten turns back to exactly 0.
        let (mut n, mut v) = setup();
        v.angle = 1.0;
        n.handle(InputEvent::KeyDown(NavKey::Straighten), &mut v, 0.0);
        run(&mut n, &mut v, 0.0, 1.5, 60.0);
        assert_eq!(v.angle, 0.0);
    }

    #[test]
    fn rotate_gesture_keeps_the_point_under_the_fingers() {
        let (mut n, mut v) = setup();
        n.handle(InputEvent::Pointer(Some((120.0, -80.0))), &mut v, 0.0);
        let p = v.point_at_offset(120.0, -80.0);
        n.handle(
            InputEvent::Rotate {
                radians: 0.4,
                x: 120.0,
                y: -80.0,
            },
            &mut v,
            0.0,
        );
        assert!((v.angle + 0.4).abs() < 1e-12);
        let (x, y) = v.offset_of(&p);
        assert!((x - 120.0).abs() < 1e-6 && (y + 80.0).abs() < 1e-6);
        // Panning a rotated view moves the content along the screen axes.
        let q = v.point_at_offset(0.0, 0.0);
        n.handle(InputEvent::Pan { dx: 10.0, dy: 0.0 }, &mut v, 0.1);
        let (x, y) = v.offset_of(&q);
        assert!((x - 10.0).abs() < 1e-6 && y.abs() < 1e-6, "{x} {y}");
    }

    #[test]
    fn flights_turn_and_land_exactly() {
        let target = crate::flyto::view_from_text("-0.743643887037151", "0.13182590420533", "1e-20")
            .unwrap()
            .with_angle(-2.5);
        // Without the spin: the short way round to the target's angle, landing exactly.
        let (mut n, mut v) = setup();
        v.angle = 2.5;
        n.advance(&mut v, 0.0);
        n.fly_to(&target, &v.clone(), 0.0);
        let mut t = 0.0;
        let mut max_step: f64 = 0.0;
        let mut prev = v.angle;
        while n.advance(&mut v, t + 1.0 / 60.0) {
            t += 1.0 / 60.0;
            max_step = max_step.max(wrap_angle(v.angle - prev).abs());
            prev = v.angle;
            assert!(t < 60.0);
        }
        assert_eq!(v, target);
        assert!(max_step < 0.05, "turned the short way, smoothly: {max_step}");
        // With the spin: the view turns with the zoom and lands on the spun angle.
        let (mut n, mut v) = setup();
        n.set_cinematic(Cinematic {
            enabled: true,
            ..Cinematic::default()
        });
        n.advance(&mut v, 0.0);
        let plain = target.clone().with_angle(0.0);
        n.fly_to(&plain, &v.clone(), 0.0);
        let f = n.flight().unwrap().clone();
        let e = f.path.length() * crate::flyto::RHO;
        assert!((f.angle1 - 4f64.to_radians() * e).abs() < 1e-9);
        let mut t = 0.0;
        while n.advance(&mut v, t + 1.0 / 60.0) {
            t += 1.0 / 60.0;
        }
        assert_eq!(v.angle, wrap_angle(f.angle1));
        assert_eq!(&v.center, &plain.center);
    }

    #[test]
    fn autopilot_spins_with_the_zoom_only_when_enabled() {
        for enabled in [false, true] {
            let (mut n, mut v) = joystick();
            n.set_cinematic(Cinematic {
                enabled,
                ..Cinematic::default()
            });
            n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
            let s0 = v.scale.ln();
            run(&mut n, &mut v, 0.0, 3.0, 60.0);
            let efolds = s0 - v.scale.ln();
            assert!(efolds > 1.0);
            if enabled {
                assert!(
                    (v.angle - 4f64.to_radians() * efolds).abs() < 1e-6,
                    "{} vs {}",
                    v.angle,
                    efolds
                );
            } else {
                assert_eq!(v.angle, 0.0);
            }
        }
    }

    #[test]
    fn journey_flies_legs_and_manual_input_takes_over() {
        let mut n = Navigator::default();
        let mut v = Viewport::fitting(1000.0, 600.0);
        n.handle(
            InputEvent::Resize {
                width: 1000.0,
                height: 600.0,
            },
            &mut v,
            0.0,
        );
        n.journey_mut().synchronous = true;
        n.start_journey(4711, false, &v.clone(), 0.0);
        let mut t = 0.0;
        while t < 60.0 && n.journey().leg() < 3 {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        assert!(n.journey().leg() >= 3, "flew legs: {}", n.journey().leg());
        assert!(v.zoom_log10() > 1.0, "went deeper: {}", v.zoom_log10());
        // A pan takes over; J resumes.
        n.handle(InputEvent::Pan { dx: 5.0, dy: 0.0 }, &mut v, t);
        assert!(n.journey().is_suspended() && n.flight().is_none());
        n.handle(InputEvent::KeyDown(NavKey::Journey), &mut v, t + 0.5);
        assert!(n.journey().is_running());
        // Esc stops it.
        n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t + 1.0);
        assert!(!n.journey().is_active());
    }

    // MARK: Driving levels (kb: decisions/controls "Driving levels")

    /// A key pressed and released at `t`.
    fn tap(n: &mut Navigator, v: &mut Viewport, k: NavKey, t: f64) -> bool {
        let handled = n.handle(InputEvent::KeyDown(k), v, t);
        n.handle(InputEvent::KeyUp(k), v, t);
        handled
    }

    /// The setup's navigator at `scale` near the seahorse valley's spiral.
    fn deep(scale: &str) -> (Navigator, Viewport) {
        let (mut n, _) = setup();
        let mut v = crate::flyto::view_from_text("-0.743643887037151", "0.13182590420533", scale).unwrap();
        n.advance(&mut v, 0.0);
        (n, v)
    }

    #[test]
    fn cruise_pedals_step_the_ladder_with_a_detent_and_hold() {
        let (mut n, mut v) = deep("1e-30");
        assert_eq!(n.level(), Level::Manual);
        // From rest E starts cruising in; each press is one rung.
        assert!(tap(&mut n, &mut v, NavKey::FasterZoom, 0.0));
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), cruise::rung(0));
        tap(&mut n, &mut v, NavKey::FasterZoom, 0.1);
        assert_eq!(n.cruise().set_speed(), cruise::rung(1));
        // Q twice: back to zero (still C while it brakes), then at rest: M.
        tap(&mut n, &mut v, NavKey::SlowerZoom, 0.2);
        tap(&mut n, &mut v, NavKey::SlowerZoom, 0.3);
        assert_eq!(n.cruise().set_speed(), 0.0);
        assert_eq!(n.level(), Level::Cruise);
        let t = run(&mut n, &mut v, 0.3, 3.5, 60.0);
        assert_eq!(n.level(), Level::Manual, "{:?}", n.cruise());
        assert!(!n.is_moving());
        // Holding Q from rest: reverse, one rung more after 0.4 s and every 0.25 s.
        n.handle(InputEvent::KeyDown(NavKey::SlowerZoom), &mut v, t);
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(n.cruise().set_speed(), -cruise::rung(4), "held 1 s: four repeats (0.4 s, then every 0.15 s)");
        n.handle(InputEvent::KeyUp(NavKey::SlowerZoom), &mut v, t);
        // Holding E brakes rung by rung and stops at the detent: never across zero.
        n.handle(InputEvent::KeyDown(NavKey::FasterZoom), &mut v, t);
        let t = run(&mut n, &mut v, t, t + 3.0, 60.0);
        assert_eq!(n.cruise().set_speed(), 0.0);
        n.handle(InputEvent::KeyUp(NavKey::FasterZoom), &mut v, t);
        // A new press crosses: forward.
        tap(&mut n, &mut v, NavKey::FasterZoom, t);
        assert_eq!(n.cruise().set_speed(), cruise::rung(0));
        // Space held: Q / E set the Space speed and leave the cruise alone.
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, t);
        tap(&mut n, &mut v, NavKey::FasterZoom, t);
        assert!((n.zoom_speed() - 2.0).abs() < 1e-12);
        assert_eq!(n.cruise().set_speed(), cruise::rung(0));
        n.handle(InputEvent::KeyUp(NavKey::Zoom), &mut v, t);
        // R stops everything dead.
        tap(&mut n, &mut v, NavKey::Reset, t);
        assert_eq!(n.level(), Level::Manual);
        assert!(!n.is_moving());
        // A manual zoom (wheel, trackpad) while cruising takes over: back to manual at once
        // (E: at home Q has nowhere to go).
        tap(&mut n, &mut v, NavKey::FasterZoom, t);
        assert_eq!(n.level(), Level::Cruise);
        n.handle(
            InputEvent::WheelZoom {
                delta: -1.0,
                x: 0.0,
                y: 0.0,
            },
            &mut v,
            t,
        );
        assert_eq!(n.level(), Level::Manual);
        assert_eq!(n.cruise().set_speed(), 0.0);
        // A jump (pasted coordinates, a link, a place) stops everything dead.
        tap(&mut n, &mut v, NavKey::FasterZoom, t);
        assert_eq!(n.level(), Level::Cruise);
        n.jumped();
        assert_eq!(n.level(), Level::Manual);
        assert!(!n.is_moving());
        // At the whole set already, Q does not reverse.
        tap(&mut n, &mut v, NavKey::SlowerZoom, t);
        assert_eq!(n.level(), Level::Manual);
    }

    /// Reverse from any depth in gear 10 (×5/s): the boost builds up over seconds, the zoom out
    /// never turns back in, home within about a minute, a soft landing exactly on home.
    #[test]
    fn cruise_reverse_lands_softly_on_home_from_any_depth() {
        let home = Viewport::fitting(W, 600.0);
        for (scale, limit) in [("1e-10", 25.0), ("1e-50", 40.0), ("1e-300", 55.0), ("1e-3000", 85.0)] {
            let (mut n, mut v) = deep(scale);
            let l0 = v.scale.ln();
            for _ in 0..10 {
                tap(&mut n, &mut v, NavKey::SlowerZoom, 0.0);
            }
            let hz = 30.0;
            let mut t = 0.0;
            let mut last = l0;
            let mut rates = Vec::new();
            let mut landed_at = None;
            while n.level() != Level::Manual {
                t += 1.0 / hz;
                n.advance(&mut v, t);
                let l = v.scale.ln();
                assert!(l >= last - 1e-9, "{scale}: never zooms in ({last} -> {l} at {t})");
                rates.push((t, (l - last) * hz));
                last = l;
                if landed_at.is_none() && n.cockpit(t).landing {
                    landed_at = Some(t);
                }
                assert!(t < 200.0, "{scale}: never home");
            }
            eprintln!(
                "reverse cruise from {scale}: home in {t:.1} s (landing from {:.1} s), peak {:.0} e-folds/s",
                landed_at.unwrap_or(f64::NAN),
                rates.iter().map(|r| r.1).fold(0.0, f64::max)
            );
            assert_eq!(v, home, "{scale}: rests exactly on the home view");
            assert!(t < limit, "{scale}: {t} s");
            // Gentle start: less than 1.5 e-folds in the first second.
            assert!(
                rates.iter().filter(|r| r.0 <= 1.0).map(|r| r.1 / hz).sum::<f64>() < 1.5,
                "{scale}"
            );
            // The soft landing: the last second slower and slower.
            let tail: Vec<f64> = rates.iter().filter(|r| r.0 > t - 1.0).map(|r| r.1).collect();
            assert!(
                tail.windows(2).all(|w| w[1] <= w[0] + 1e-9),
                "{scale}: eases out {tail:?}"
            );
            assert!(!n.advance(&mut v, t + 0.1), "{scale}: at rest");
        }
    }

    #[test]
    fn cruise_is_frame_rate_independent() {
        let path = |hz: f64| {
            let (mut n, mut v) = deep("1e-40");
            let c0 = v.center.clone();
            let s0 = v.scale;
            n.handle(InputEvent::Pointer(Some((150.0, 80.0))), &mut v, 0.0);
            // Gear 6 (×2/s out).
            for _ in 0..6 {
                tap(&mut n, &mut v, NavKey::SlowerZoom, 0.0);
            }
            let mut out = Vec::new();
            let mut t = 0.0;
            for k in 1..=10 {
                if k == 4 {
                    n.handle(InputEvent::Pointer(Some((-120.0, -60.0))), &mut v, t);
                }
                if k == 8 {
                    n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t);
                }
                t = run(&mut n, &mut v, t, k as f64 * 0.6, hz);
                let (x, y) = v.center.delta_in(&c0, v.scale);
                out.push((x, y, v.scale.ln() - s0.ln()));
            }
            out
        };
        let (a, b, c) = (path(30.0), path(60.0), path(120.0));
        for i in 0..a.len() {
            for o in [b[i], c[i]] {
                assert!(
                    (a[i].2 - o.2).abs() < 2e-3 * a[i].2.abs().max(1.0),
                    "zoom {i}: {a:?} vs {o:?}"
                );
                // The steered centre within 3 points (of 600) after up to 26 e-folds out: the
                // boost depends on the depth the zoom changes (first order in the 2 ms substeps).
                let d = (a[i].0 - o.0).hypot(a[i].1 - o.1);
                assert!(d < 3.0, "pan {i}: {:?} vs {:?}", a[i], o);
            }
        }
        assert!(a[9].2 > 5.0, "zoomed out: {:?}", a[9]);
    }

    #[test]
    fn cruise_esc_brakes_on_a_smooth_curve() {
        let (mut n, mut v) = deep("1e-30");
        for _ in 0..4 {
            tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        }
        let speed = cruise::rung(3);
        let t = run(&mut n, &mut v, 0.0, 4.0, 60.0);
        assert!((n.cruise().rate() - speed).abs() < 1e-3 * speed);
        assert!(n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t));
        let l0 = v.scale.ln();
        let mut last = n.cruise().rate();
        let mut t = t;
        let mut frames = 0;
        while n.level() == Level::Cruise {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
            let r = n.cruise().rate();
            assert!(r <= last + 1e-12 && r >= 0.0, "monotonic braking: {r} after {last}");
            last = r;
            frames += 1;
            assert!(frames < 300, "never at rest");
        }
        let glide = l0 - v.scale.ln();
        let expect = 2.0 / autopilot::RATE_OMEGA.1 * speed;
        assert!((glide - expect).abs() < 0.02 * expect, "glide {glide} vs {expect}");
        assert!(!n.is_moving());
    }

    #[test]
    fn cruise_steers_the_pointed_content_to_the_centre() {
        let (mut n, mut v) = setup();
        n.handle(InputEvent::Pointer(Some((200.0, 0.0))), &mut v, 0.0);
        let c = v.point_at_offset(200.0, 0.0);
        let ahead = v.point_at_offset(0.0, 0.0);
        n.advance(&mut v, 0.0);
        for _ in 0..3 {
            tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        }
        let mut last = 200.0;
        let mut t = 0.0;
        for _ in 0..5 {
            t = run(&mut n, &mut v, t, t + 0.5, 60.0);
            let (x, y) = screen_of(&v, &c);
            assert!(x < last && y.abs() < 1e-6, "{x} {y} at {t}");
            last = x;
        }
        assert!(last < 120.0, "{last}");
        assert!(screen_of(&v, &ahead).0 < -10.0);
        // An idle pointer stops steering after STEER_IDLE seconds: never pulled anywhere.
        run(&mut n, &mut v, t, autopilot::STEER_IDLE + 1.5, 60.0);
        assert!(dist(n.cruise().heading()) < 1.0, "{:?}", n.cruise().heading());
        assert_eq!(n.level(), Level::Cruise);
    }

    /// Any user steering in A drops to cruise at the current speed; the autopilot does not take
    /// the wheel back; Z re-engages; pulling the speed to zero drops to cruise too.
    #[test]
    fn autopilot_hands_over_to_cruise_and_never_takes_the_wheel_back() {
        let (mut n, mut v) = joystick();
        // The pointer resting off-centre when Z is pressed does not count.
        n.handle(InputEvent::Pointer(Some((250.0, 0.0))), &mut v, 0.0);
        n.advance(&mut v, 0.0);
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        let t = run(&mut n, &mut v, 0.0, 2.0, 60.0);
        assert_eq!(n.level(), Level::Autopilot);
        let rate = n.autopilot().rate();
        // A small nudge of the mouse is not steering.
        n.handle(InputEvent::Pointer(Some((255.0, 5.0))), &mut v, t);
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(n.level(), Level::Autopilot, "a nudge keeps the autopilot");
        // Moving it far and holding it there never hands over: it is a direction wish.
        n.handle(InputEvent::Pointer(Some((180.0, 10.0))), &mut v, t);
        n.handle(InputEvent::Pointer(Some((250.0, 20.0))), &mut v, t);
        let t = run(&mut n, &mut v, t, t + STEER_ENGAGE + 3.0, 60.0);
        assert_eq!(n.level(), Level::Autopilot, "the pointer does not hand over in A");
        let w = n.autopilot().wish().expect("a wish to the right");
        assert!(w.dir.0 > 0.99, "{:?}", w.dir);
        // The target marker shows in A (with the wish line), not in C.
        assert!(n.aim_marker().is_some_and(|m| m.wish == Some(w.dir)));
        // The stick hands over (as W A S D and a drag below).
        n.handle(
            InputEvent::Gamepad {
                lx: 0.8,
                ly: 0.0,
                rx: 0.0,
                ry: 0.0,
                lt: 0.0,
                rt: 0.0,
                boost: false,
            },
            &mut v,
            t,
        );
        assert_eq!(n.level(), Level::Cruise);
        assert!(n.aim_marker().is_none());
        n.handle(
            InputEvent::Gamepad {
                lx: 0.0,
                ly: 0.0,
                rx: 0.0,
                ry: 0.0,
                lt: 0.0,
                rt: 0.0,
                boost: false,
            },
            &mut v,
            t,
        );
        assert!(!n.autopilot().is_active());
        assert_eq!(n.cruise().set_speed(), autopilot::SPEED_DEFAULT);
        assert!(
            (n.cruise().rate() - rate).abs() < 0.05,
            "kept its speed: {} vs {rate}",
            n.cruise().rate()
        );
        // Idle for long: still cruise (no re-lock).
        let t = run(&mut n, &mut v, t, t + 6.0, 60.0);
        assert_eq!(n.level(), Level::Cruise);
        assert!(!n.autopilot().is_active());
        // Z again: A, at the cruise's speed.
        tap(&mut n, &mut v, NavKey::Autopilot, t);
        assert_eq!(n.level(), Level::Autopilot);
        // W A S D: cruise at once.
        n.handle(InputEvent::KeyDown(NavKey::Up), &mut v, t);
        n.handle(InputEvent::KeyUp(NavKey::Up), &mut v, t);
        assert_eq!(n.level(), Level::Cruise);
        // A drag too.
        let t = run(&mut n, &mut v, t, t + 0.5, 60.0);
        tap(&mut n, &mut v, NavKey::Autopilot, t);
        n.handle(InputEvent::Pan { dx: 4.0, dy: 0.0 }, &mut v, t);
        assert_eq!(n.level(), Level::Cruise);
        // In A, Q / E step the autopilot's speed on the ladder; down to zero: cruise, braking.
        let t = run(&mut n, &mut v, t, t + 0.5, 60.0);
        tap(&mut n, &mut v, NavKey::Autopilot, t);
        assert_eq!(n.level(), Level::Autopilot);
        let s = n.autopilot().speed();
        tap(&mut n, &mut v, NavKey::FasterZoom, t);
        assert!(n.autopilot().speed() > s);
        for _ in 0..cruise::LADDER.len() + 2 {
            if n.level() == Level::Cruise {
                break;
            }
            tap(&mut n, &mut v, NavKey::SlowerZoom, t);
        }
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), 0.0, "the detent: not reversing");
        run(&mut n, &mut v, t, t + 4.0, 60.0);
        assert_eq!(n.level(), Level::Manual);
    }

    /// Journey #4711's zoom rate per frame (e-folds/s, + in) and whether a leg flew, over
    /// `secs` at `hz`, after `presses` of E.
    fn journey_rates(presses: usize, secs: f64, hz: f64) -> Vec<(f64, bool)> {
        let (mut n, mut v) = journey_nav(false);
        n.start_journey(4711, false, &v.clone(), 0.0);
        let mut t = 0.0;
        for _ in 0..presses {
            t = run(&mut n, &mut v, t, t + 0.25, hz);
            tap(&mut n, &mut v, NavKey::FasterZoom, t);
        }
        let mut z = v.scale.ln();
        let mut out = Vec::new();
        for _ in 0..(secs * hz) as usize {
            t += 1.0 / hz;
            n.advance(&mut v, t);
            let z1 = v.scale.ln();
            out.push(((z - z1) * hz, n.flight().is_some()));
            z = z1;
        }
        out
    }

    /// A journey's legs at the default and a raised speed (kb: decisions/controls "Journey",
    /// smooth legs): no brake-and-accelerate pulse while a leg zooms one way, bounded zoom
    /// acceleration (cross-overs brake into their turn), and every leg comes to rest instead
    /// of stopping dead. Before: 38 e-folds/s² at the default speed (a dull stretch up to the
    /// target braked from 10 e-folds/s in 0.3 s; a one-sample gap in a dull stretch braked
    /// 10.7 → 8.6 → 10.7 e-folds/s), 460 at ×35/s (a turn from −28 to +23 in 0.15 s).
    #[test]
    fn journey_legs_have_no_pulses_or_dead_stops() {
        for (presses, max_acc) in [(0, 16.0), (6, 24.0)] {
            let r = journey_rates(presses, 75.0, 60.0);
            let k = 6;
            let mut legs = 0;
            let mut worst = 0.0f64;
            let mut i = 0;
            while i < r.len() {
                if !r[i].1 {
                    i += 1;
                    continue;
                }
                let j = (i..r.len()).find(|&j| !r[j].1).unwrap_or(r.len());
                let leg: Vec<f64> = r[i..j].iter().map(|x| x.0).collect();
                i = j;
                if j == r.len() {
                    break; // cut off by the end of the run
                }
                legs += 1;
                let peak = leg.iter().fold(0.0f64, |m, x| m.max(x.abs()));
                // At rest on landing: the last 0.1 s slow.
                let last = leg[leg.len().saturating_sub(k)..]
                    .iter()
                    .fold(0.0f64, |m, x| m.max(x.abs()));
                assert!(last < 0.2 * peak, "E×{presses} leg {legs}: lands at {last} of {peak}");
                // Zoom acceleration over 0.1 s.
                for w in leg.windows(k + 1) {
                    worst = worst.max((w[k] - w[0]).abs() * 60.0 / k as f64);
                }
                // No pulse: while zooming one way, the rate does not drop by more than 15 % of
                // its peak so far and then rise again by 10 %.
                let (mut hi, mut lo, mut sign) = (0.0f64, f64::INFINITY, 0.0);
                for &x in &leg {
                    let sg = if x > 0.05 {
                        1.0
                    } else if x < -0.05 {
                        -1.0
                    } else {
                        0.0
                    };
                    if sg != sign {
                        (hi, lo, sign) = (0.0, f64::INFINITY, sg);
                    }
                    let a = x.abs();
                    if sg == 0.0 {
                        continue;
                    }
                    if a >= hi && lo == f64::INFINITY {
                        hi = a;
                    } else {
                        lo = lo.min(a);
                        assert!(
                            !(lo < 0.85 * hi && a > lo + 0.1 * hi),
                            "E×{presses} leg {legs}: pulse {hi} → {lo} → {a}"
                        );
                    }
                }
            }
            assert!(legs >= 4, "E×{presses}: {legs} legs");
            assert!(worst < max_acc, "E×{presses}: zoom acceleration {worst} e-folds/s²");
        }
    }

    /// J after the user took the wheel: the released cruise glides out and the next leg takes
    /// its zoom over (or waits for it), no dead stop (before: the cruise halted at once).
    #[test]
    fn journey_resumes_from_cruise_without_a_dead_stop() {
        // Cruising on in the leg's direction, then cruising out (Q into reverse).
        for reverse in [false, true] {
            let (mut n, mut v) = journey_nav(false);
            n.start_journey(4711, false, &v.clone(), 0.0);
            let mut t = 0.0;
            while !(n.flight().is_some() && n.flight().unwrap().progress() > 0.5) {
                t = run(&mut n, &mut v, t, t + 1.0 / 60.0, 60.0);
                assert!(t < 60.0, "no leg");
            }
            n.handle(InputEvent::KeyDown(NavKey::Left), &mut v, t);
            n.handle(InputEvent::KeyUp(NavKey::Left), &mut v, t);
            assert_eq!(n.level(), Level::Cruise);
            // The cruise goes on at the journey speed, not at the leg's paced rate.
            assert!(n.cruise().set_speed().abs() <= n.journey().speed() + 1e-9);
            if reverse {
                for _ in 0..cruise::LADDER.len() + 2 {
                    if n.cruise().set_speed() < 0.0 {
                        break;
                    }
                    tap(&mut n, &mut v, NavKey::SlowerZoom, t);
                }
                assert!(n.cruise().set_speed() < 0.0);
                t = run(&mut n, &mut v, t, t + 3.0, 60.0);
            } else {
                t = run(&mut n, &mut v, t, t + 1.0, 60.0);
            }
            let rate = n.cruise().rate();
            assert!(rate.abs() > 0.3, "cruising {rate}");
            tap(&mut n, &mut v, NavKey::Journey, t);
            assert_eq!(n.level(), Level::Journey);
            let mut z = v.scale.ln();
            let mut prev = rate;
            let mut flew = false;
            for _ in 0..(6 * 60) {
                t += 1.0 / 60.0;
                n.advance(&mut v, t);
                let z1 = v.scale.ln();
                let r = (z - z1) * 60.0;
                z = z1;
                assert!((r - prev).abs() < 0.25, "reverse {reverse}: jolt {prev} → {r} at {t}");
                prev = r;
                flew |= n.flight().is_some();
            }
            assert!(flew, "reverse {reverse}: the next leg flew");
        }
    }

    #[test]
    fn steering_or_pedal_to_zero_drops_a_journey_to_cruise() {
        let (mut n, mut v) = journey_nav(false);
        n.start_journey(4711, false, &v.clone(), 0.0);
        let mut t = 0.0;
        let flying = |n: &mut Navigator, v: &mut Viewport, t: &mut f64| {
            let t0 = *t;
            while !(n.journey().is_running() && n.flight().is_some() && n.flight().unwrap().progress() > 0.3) {
                *t += 1.0 / 60.0;
                n.advance(v, *t);
                assert!(*t < t0 + 60.0, "no leg");
            }
        };
        flying(&mut n, &mut v, &mut t);
        assert_eq!(n.level(), Level::Journey);
        n.handle(InputEvent::KeyDown(NavKey::Left), &mut v, t);
        n.handle(InputEvent::KeyUp(NavKey::Left), &mut v, t);
        assert_eq!(n.level(), Level::Cruise);
        assert!(n.journey().is_suspended() && n.flight().is_none());
        // J resumes the journey.
        tap(&mut n, &mut v, NavKey::Journey, t);
        assert_eq!(n.level(), Level::Journey);
        flying(&mut n, &mut v, &mut t);
        // Q steps the journey's speed down; to zero: cruise, braking to rest.
        let s = n.journey().speed();
        tap(&mut n, &mut v, NavKey::SlowerZoom, t);
        assert!(n.journey().speed() < s && n.level() == Level::Journey);
        for _ in 0..8 {
            if n.level() == Level::Cruise {
                break;
            }
            tap(&mut n, &mut v, NavKey::SlowerZoom, t);
        }
        assert_eq!(n.level(), Level::Cruise);
        assert!(n.journey().is_suspended());
        assert_eq!(n.cruise().set_speed(), 0.0);
    }

    #[test]
    fn autopilot_recovers_after_cruising_into_a_featureless_region() {
        // The user cruises into the main cardioid's interior on purpose, then hands over to A.
        let (mut n, mut v) = locked();
        let inside = crate::viewport::Center::new(-0.3, 0.1);
        n.handle(InputEvent::Pointer(Some(v.offset_of(&inside))), &mut v, 0.0);
        n.advance(&mut v, 0.0);
        for _ in 0..3 {
            tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        }
        let mut t = 0.0;
        for _ in 0..(10 * 30) {
            let (x, y) = v.offset_of(&inside);
            n.handle(
                InputEvent::Pointer(Some((x.clamp(-490.0, 490.0), y.clamp(-290.0, 290.0)))),
                &mut v,
                t,
            );
            t = run(&mut n, &mut v, t, t + 1.0 / 30.0, 60.0);
        }
        let (detail, interior) = probe_view(&v);
        assert!(detail < 1.0 && interior > 0.99, "cruised into it: {detail} {interior}");
        assert_eq!(n.level(), Level::Cruise);
        let deepest = v.scale.ln();
        n.handle(InputEvent::Pointer(None), &mut v, t);
        tap(&mut n, &mut v, NavKey::Autopilot, t);
        assert_eq!(n.level(), Level::Autopilot);
        let mut rescued = false;
        for _ in 0..(40 * 60) {
            t = run(&mut n, &mut v, t, t + 1.0 / 60.0, 60.0);
            rescued |= n.autopilot().is_rescuing();
            if rescued && n.autopilot().lock().target().is_some() {
                break;
            }
        }
        assert!(rescued, "it backed out");
        assert!(n.autopilot().lock().target().is_some(), "found a target");
        assert!(v.scale.ln() > deepest + 2.0);
        let t = run(&mut n, &mut v, t, t + 10.0, 60.0);
        assert!(
            n.autopilot().rate() > 0.5 * autopilot::SPEED_DEFAULT,
            "{} at {t}",
            n.autopilot().rate()
        );
        let (detail, interior) = probe_view(&v);
        assert!(detail >= 3.0 && interior < 0.6, "{detail} {interior}");
    }

    #[test]
    fn autopilot_speed_keys_scroll_and_click() {
        let (mut n, mut v) = joystick();
        // Off: a scroll pans, a click is not ours.
        let c0 = v.center.clone();
        n.handle(
            InputEvent::Scroll {
                dx: 10.0,
                dy: 0.0,
                momentum: false,
            },
            &mut v,
            0.0,
        );
        assert_ne!(v.center, c0);
        assert!(!n.handle(InputEvent::Click { x: 0.0, y: 0.0 }, &mut v, 0.0));
        // On: E / Q step its speed on the ladder; scrolling sets it and does not pan.
        tap(&mut n, &mut v, NavKey::Autopilot, 0.1);
        let s0 = n.autopilot().speed();
        tap(&mut n, &mut v, NavKey::FasterZoom, 0.1);
        assert_eq!(n.autopilot().speed(), cruise::pedal(s0, 1.0, true));
        assert!((n.zoom_speed() - 1.6).abs() < 1e-12);
        tap(&mut n, &mut v, NavKey::SlowerZoom, 0.1);
        let s0 = n.autopilot().speed();
        let c1 = v.center.clone();
        n.handle(
            InputEvent::Scroll {
                dx: 0.0,
                dy: 100.0,
                momentum: false,
            },
            &mut v,
            0.2,
        );
        assert_eq!(v.center, c1);
        assert!((n.autopilot().speed() - s0 * (100.0 * autopilot::SCROLL_PER_POINT).exp()).abs() < 1e-12);
        let s1 = n.autopilot().speed();
        n.handle(
            InputEvent::Scroll {
                dx: 0.0,
                dy: 100.0,
                momentum: true,
            },
            &mut v,
            0.2,
        );
        assert_eq!(n.autopilot().speed(), s1, "momentum scrolls are ignored");
        n.handle(
            InputEvent::WheelNotch {
                delta: -2.0,
                x: 0.0,
                y: 0.0,
            },
            &mut v,
            0.3,
        );
        assert!((n.autopilot().speed() - s1 / autopilot::NOTCH_FACTOR.powi(2)).abs() < 1e-12);
        // A click pauses (eased to rest: the display link may stop), another resumes.
        n.advance(&mut v, 0.3);
        run(&mut n, &mut v, 0.3, 2.0, 60.0);
        assert!(n.handle(InputEvent::Click { x: 5.0, y: 5.0 }, &mut v, 2.0));
        assert!(n.autopilot().is_paused());
        let t = run(&mut n, &mut v, 2.0, 5.0, 60.0);
        assert!(!n.is_moving() && !n.advance(&mut v, t + 0.01));
        assert!(n.handle(InputEvent::Click { x: 5.0, y: 5.0 }, &mut v, t));
        assert!(n.is_moving());
        // Reset stops it dead.
        n.handle(InputEvent::KeyDown(NavKey::Reset), &mut v, t);
        assert!(!n.autopilot().is_active() && !n.is_moving());
    }

    fn journey_nav(stops: bool) -> (Navigator, Viewport) {
        let mut n = Navigator::default();
        let mut v = Viewport::fitting(1000.0, 600.0);
        n.handle(
            InputEvent::Resize {
                width: 1000.0,
                height: 600.0,
            },
            &mut v,
            0.0,
        );
        n.journey_mut().synchronous = true;
        n.set_journey_stops(stops);
        (n, v)
    }

    /// Runs frames at `hz` until the journey is at a stop with nothing pending; returns the time.
    fn until_stop(n: &mut Navigator, v: &mut Viewport, mut t: f64, hz: f64) -> f64 {
        let t0 = t;
        while !(n.journey().at_stop() && n.journey().choice().is_none()) {
            t += 1.0 / hz;
            n.advance(v, t);
            assert!(t < t0 + 120.0, "never stopped");
        }
        t
    }

    /// The journey in "stop at each find" mode: it rests at each landmark (no frames needed),
    /// J flies the continuous journey's next target, X crosses over, Z goes deeper, looking
    /// around does not pause it, Esc ends it where it is (kb: decisions/controls "Journey with
    /// stops").
    #[test]
    fn journey_stops_at_each_find_and_takes_choices() {
        // The continuous journey's first targets, for comparison.
        let (mut c, mut cv) = journey_nav(false);
        c.handle(InputEvent::KeyDown(NavKey::JourneyAlt), &mut cv, 0.0);
        assert!(c.journey().stops(), "⇧J starts the other mode");
        c.stop_journey();
        c.start_journey(4711, false, &cv.clone(), 0.0);
        let mut t = 0.0;
        let mut cont = Vec::new();
        while cont.len() < 2 {
            t += 1.0 / 60.0;
            c.advance(&mut cv, t);
            if let Some(cur) = c.journey().current()
                && cont.last() != Some(cur)
            {
                cont.push(cur.clone());
            }
            assert!(t < 120.0);
        }

        let mut lands = Vec::new();
        for hz in [30.0, 60.0, 120.0] {
            let (mut n, mut v) = journey_nav(true);
            n.start_journey(4711, false, &v.clone(), 0.0);
            assert!(n.journey().stops());
            let t = until_stop(&mut n, &mut v, 0.0, hz);
            let first = n.journey().current().unwrap().clone();
            assert_eq!(first, cont[0]);
            assert_eq!(
                v,
                first.viewport(600.0).with_angle(v.angle),
                "{hz} Hz: at rest on the landmark"
            );
            assert!(
                !n.is_moving() && !n.advance(&mut v, t + 0.5),
                "{hz} Hz: no frames needed at a stop"
            );
            assert!(n.journey().is_running() && !n.journey().is_planning());
            // Looking around (pan, zoom step, click) keeps the stop.
            n.handle(InputEvent::Pan { dx: 20.0, dy: 5.0 }, &mut v, t + 1.0);
            n.handle(
                InputEvent::WheelNotch {
                    delta: 1.0,
                    x: 0.0,
                    y: 0.0,
                },
                &mut v,
                t + 1.0,
            );
            n.handle(InputEvent::Click { x: 0.0, y: 0.0 }, &mut v, t + 1.1);
            let t = run(&mut n, &mut v, t + 1.0, t + 2.0, hz);
            assert!(
                n.journey().at_stop() && !n.journey().is_suspended(),
                "{hz} Hz: still at the stop"
            );
            // J: as planned, the continuous journey's second target.
            assert!(n.handle(InputEvent::KeyDown(NavKey::Journey), &mut v, t));
            let t = until_stop(&mut n, &mut v, t, hz);
            assert_eq!(
                n.journey().current().unwrap(),
                &cont[1],
                "{hz} Hz: J = the planned next target"
            );
            assert_eq!(n.journey().leg(), 2);
            lands.push((v.clone(), n.journey().current().unwrap().clone()));

            if hz != 60.0 {
                continue;
            }
            // X: somewhere else, a cross-over to another region.
            let here = n.journey().current().unwrap().clone();
            assert!(n.handle(InputEvent::KeyDown(NavKey::Elsewhere), &mut v, t));
            let t = until_stop(&mut n, &mut v, t, hz);
            let x = n.journey().current().unwrap().clone();
            assert!(
                matches!(x.kind, journey::TargetKind::Crossing | journey::TargetKind::Surface),
                "X crosses over: {}",
                x.label
            );
            assert_ne!(region_of(&x), region_of(&here), "{} -> {}", here.label, x.label);
            // Z: deeper here (or the plan as it was, with a note).
            let t = until_stop(&mut n, &mut v, t, hz);
            assert!(n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, t));
            assert!(!n.autopilot().is_active(), "Z at a stop is not the autopilot");
            let t = until_stop(&mut n, &mut v, t, hz);
            let z = n.journey().current().unwrap().clone();
            let gain = x.span.ratio(z.span).ln();
            assert!(
                gain >= journey::DEEPER_MIN_GAIN || n.journey().note() == Some(journey::NOTHING_DEEPER),
                "Z goes deeper: {} e-folds ({})",
                gain,
                z.label
            );
            assert!(
                z.center.distance_in(&x.center, x.span) < 1.0,
                "Z stays here: {}",
                z.label
            );
            // Moved far away (zoomed out a long way): J plans from the view instead.
            v.zoom_by(1e-4, 0.0, 0.0);
            assert!(n.handle(InputEvent::KeyDown(NavKey::Journey), &mut v, t));
            let span = v.scale * 600.0;
            let (pc, ps) = n.journey().planned_from().unwrap();
            assert!(pc == &v.center && *ps == span, "planned from the view");
            let t = until_stop(&mut n, &mut v, t, hz);
            // Esc ends the journey where it is.
            let here = v.clone();
            assert!(n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t));
            assert!(!n.journey().is_active() && !n.advance(&mut v, t + 0.1) && v == here);
            // X is not ours when no journey stops.
            assert!(!n.handle(InputEvent::KeyDown(NavKey::Elsewhere), &mut v, t + 0.2));
        }
        for l in &lands {
            assert_eq!(l.1, lands[0].1, "30/60/120 Hz: the same landmarks");
            assert_eq!(l.0, lands[0].0, "30/60/120 Hz: exactly on them");
        }
    }

    fn region_of(t: &journey::Target) -> journey::RegionId {
        journey::region_of(t.center.re(), t.center.im()).0
    }
}
