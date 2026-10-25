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
//! steps (wheel notch, + / −, menu) are animated with a short exponential ease-out at the
//! zoom target; a double click / tap is a gentler ×2 glide (minimum-jerk, [`TAP_ZOOM_SECONDS`])
//! that further clicks extend. kb: decisions/controls (motion feel).
//!
//! **Zoom target.** Every zoom (held Space, pinch, scroll and wheel zoom, double-click, the
//! `+` / `−` keys) zooms towards one point, and the complex coordinate under it stays fixed
//! on screen. In pointer mode it is the pointer (or the event's own position for gestures);
//! in keyboard mode the overlay crosshair, which glides to the view centre on a critically
//! damped spring.
//!
//! **View limits** (kb: decisions/controls "View limits", [`ViewLimits`]). Every zoom out
//! eases into the zoom-out limit ([`ViewLimits::zoom_factor`]: no bounce, a continuing pinch or
//! wheel just stops there). Every centre move (pans, flings, keys, zoom about an off-centre
//! point, the autopilot's and cruise's steering) goes through a rubber band past the centre
//! bound ([`ViewLimits::rubber_move`]); once the hand is off, the centre springs back onto
//! the bound. Flights, journeys and places fly to targets clamped into the limits.

use crate::autopilot::{self, Autopilot};
use crate::cinematic::Cinematic;
use crate::cruise::{self, Cruise, Level};
use crate::flyto::{self, Flight, FlyPath};
use crate::journey::{self, Choice, Journey};
use crate::seek::{SeekPhase, Seeker};
use crate::viewport::{RUBBER_BAND, RUBBER_RETURN_TAU, Scale, ViewLimits, Viewport, wrap_angle};

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
    /// is off). `command`: ⌘ was held (Mac; iPad with a keyboard): where the formula offers the
    /// Julia panel the host opens the clicked point's Julia set instead and the navigator never
    /// sees the click (kb: decisions/controls "⌘-click: open a point's Julia set").
    Click {
        x: f64,
        y: f64,
        command: bool,
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
    /// Double click / tap: zoom x2 (or out x0.5) at (x, y); a click is pointer use. `command`:
    /// ⌘ was held (the further clicks of a ⌘-click that opened a Julia set do not zoom it).
    DoubleTap {
        x: f64,
        y: f64,
        out: bool,
        command: bool,
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
    /// A two-finger twist on a touch screen (the iOS rotation recogniser), `radians` since the
    /// last event: like `Rotate`, but only once the gesture has turned [`TWIST_DEAD_BAND`]
    /// (then on from there, no jump). A pinch always twists a little; a child's pinches turned
    /// the view and brought up the compass (QA 2026-10-18).
    TouchRotate {
        radians: f64,
        x: f64,
        y: f64,
    },
    /// The twist ended (fingers lifted): the next one starts at the dead band again.
    TouchRotateEnd,
    /// Every finger is off the screen (iOS; kb: decisions/controls "Touch", device round). A
    /// finger left down after a pinch or twist neither pans nor steers until then: lifting one
    /// finger of a pinch first used to pan the view with the other.
    TouchesLifted,
    /// Touch (kb: decisions/controls "Touch", "Rest or swipe"): the first finger went down at
    /// (x, y) on `spot` (the platform fills in where it is; Swift sends the canvas and the host
    /// hit-tests). While A, C or a journey's flight flies ([`Navigator::finger_steers`]) Rust
    /// classifies the finger itself from its moves and their times: one that rests (moves less
    /// than [`FINGER_SLOP`] in its first [`FINGER_REST`] s) steers until it lifts, one that
    /// swipes pans the picture; the platform's own pan events for it are ignored. Elsewhere the
    /// finger is not Rust's and the pan recogniser's `Pan` moves the view as before.
    FingerDown {
        x: f64,
        y: f64,
        spot: TouchSpot,
    },
    /// That finger moved to (x, y) (view points from the centre, y up; every move, from the
    /// first one, so nothing hides under the platform's pan start distance).
    FingerMove {
        x: f64,
        y: f64,
    },
    /// That finger lifted; `cancelled`: a second finger came down or the system took the touch
    /// (no fling, no wish left behind).
    FingerUp {
        cancelled: bool,
    },
    /// The touch throttle lever crossed one detent up (`up`: E, towards "in") or down (Q): the
    /// pedals' path, one rung per detent. Within one drag the speed stops at zero (the keys'
    /// detent: a held key never crosses zero); lifting and dragging again crosses.
    Throttle {
        up: bool,
    },
    /// The finger left the throttle lever (the gear stays).
    ThrottleEnd,
    /// The touch throttle is a position lever (kb: decisions/controls "Touch"): its knob was
    /// put on signed gear `gear` (1–26 in, −1…−26 out, 0 the middle: stop). The level driving
    /// takes that rung as its set speed (the cruise from rest or in C, the autopilot's or the
    /// journey's speed in A and J, a fly-to's speed); 0 or reverse in A or J hands over to a
    /// cruise at that speed, as the pedals do towards zero. No detent at zero: the lever's
    /// position is the speed.
    SetGear {
        gear: i32,
    },
    /// The orbit probe (touch has no hover): the orbit overlay's point c at (x, y) view points,
    /// fixed on screen like a resting mouse pointer; `None` gives it back to the zoom target.
    Probe(Option<(f64, f64)>),
    /// Seek minibrot (N) about (x, y) instead of the view centre (long-press menu).
    SeekAt {
        x: f64,
        y: f64,
    },
    /// Fly here (long-press menu): the fly-to path to (x, y) centred, [`FLY_HERE_ZOOM`]× deeper.
    FlyHere {
        x: f64,
        y: f64,
    },
}

/// Where a finger rests (the platform's hit tests; kb: decisions/controls "Touch").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchSpot {
    /// A control Rust draws (cockpit, help badge, compass).
    Control,
    /// A mini-map panel.
    Map,
    /// The compass rose (its drag turns the view: never a steering or panning finger).
    Compass,
    /// The picture.
    Canvas,
}

/// What a finger held still does ([`Navigator::touch_hold`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchHold {
    Nothing,
    /// Fly to the mini-map's point (the Mac's ⌥-click).
    FlyToMap,
    /// The long-press menu at the finger.
    Menu,
}

impl InputEvent {
    /// Every number in the event is finite (NaN or inf from a platform bug would reach the
    /// BigFloat centre and panic, or a size of usize::MAX; kb: techniques/hang-prevention).
    pub fn is_finite(&self) -> bool {
        let f = |v: &[f64]| v.iter().all(|x| x.is_finite());
        match *self {
            Self::KeyDown(_)
            | Self::KeyUp(_)
            | Self::Shift(_)
            | Self::FocusLost
            | Self::GestureEnd
            | Self::FingerUp { .. }
            | Self::TouchRotateEnd
            | Self::TouchesLifted
            | Self::Throttle { .. }
            | Self::SetGear { .. }
            | Self::ThrottleEnd => true,
            Self::Probe(p) => p.is_none_or(|(x, y)| f(&[x, y])),
            Self::FingerDown { x, y, .. }
            | Self::FingerMove { x, y }
            | Self::SeekAt { x, y }
            | Self::FlyHere { x, y } => f(&[x, y]),
            Self::Resize { width, height } => f(&[width, height]),
            Self::Insets {
                top,
                left,
                bottom,
                right,
            } => f(&[top, left, bottom, right]),
            Self::Pointer(p) => p.is_none_or(|(x, y)| f(&[x, y])),
            Self::Pan { dx, dy } | Self::Scroll { dx, dy, .. } => f(&[dx, dy]),
            Self::Click { x, y, .. } | Self::DoubleTap { x, y, .. } => f(&[x, y]),
            Self::WheelZoom { delta, x, y } | Self::WheelNotch { delta, x, y } => f(&[delta, x, y]),
            Self::Magnify { magnification, x, y } => f(&[magnification, x, y]),
            Self::Pinch { ratio, x, y, dx, dy } => f(&[ratio, x, y, dx, dy]),
            Self::ZoomBy { factor } => factor.is_finite(),
            Self::Gamepad {
                lx, ly, rx, ry, lt, rt, ..
            } => f(&[lx, ly, rx, ry, lt, rt]),
            Self::Rotate { radians, x, y } | Self::TouchRotate { radians, x, y } => f(&[radians, x, y]),
        }
    }
}

/// How far a touch twist must turn before the view turns (radians, 15°): below it a pinch's
/// incidental twist does nothing. Mac trackpad rotation (`Rotate`) stays 1:1.
pub const TWIST_DEAD_BAND: f64 = 15.0 * std::f64::consts::PI / 180.0;

/// Rest or swipe (kb: decisions/controls "Touch", "Rest or swipe"): while A, C or a journey's
/// flight flies, a finger that stays within this many points of where it went down for its
/// first [`FINGER_REST`] seconds rests, and steers (A: the wish; C: like the Mac's pointer; a
/// journey's flight: takes the wheel once off the dead zone, then the cruise steers); one that
/// gets farther sooner swipes and pans the picture. 10 points is about UIKit's own pan start
/// distance (a finger's tremor stays well inside it); the swipe's pan then catches up the
/// whole way from the touch-down point, so the picture is under the finger.
pub const FINGER_SLOP: f64 = 10.0;
/// See [`FINGER_SLOP`]: a finger still within it after this long (s) rests. Long enough for a
/// deliberate swipe to leave the slop (a flick crosses 10 points in under 0.1 s), short
/// enough that a finger put down to steer acts almost at once (round 2 steered after 0.2 s).
pub const FINGER_REST: f64 = 0.25;

/// What a finger the navigator classifies is ([`InputEvent::FingerDown`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FingerKind {
    /// Down for less than [`FINGER_REST`] and within [`FINGER_SLOP`]: not decided yet.
    Pending,
    /// It rested: it steers until it lifts, wherever it slides (a joystick).
    Steer,
    /// It swiped: it pans the picture until it lifts.
    Pan,
    /// Lifted, cancelled, rested off the picture, or left after a pinch: nothing more until
    /// every finger is off (the platform's last pan events for it are still swallowed).
    Done,
}

/// The finger the navigator classifies (kb: decisions/controls "Touch", "Rest or swipe").
#[derive(Clone, Copy, Debug)]
struct Finger {
    /// When it went down, where, and where it is now (view points from the centre, y up).
    down: f64,
    start: (f64, f64),
    last: (f64, f64),
    kind: FingerKind,
    /// It went down on the picture (a rest steers; on a mini-map or a control it does nothing).
    on_canvas: bool,
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
/// Time constant (s) of animated zoom steps (wheel notch, + / −, menu): 63 % of the step
/// after 60 ms, 99 % after 0.28 s.
pub const STEP_TAU: f64 = 0.06;
/// Duration (s) of a double click / tap zoom step: a ×2 on a minimum-jerk curve in log scale
/// (eases in and out, no jump on the first frame). Its tails are slow (below 10 % of the peak
/// rate for the first and last 13 %), so the visible motion lasts ~0.25 s, like the system's
/// own double-tap zooms; the peak rate is 1.875 · ln 2 / 0.35 = 3.7 e-folds/s. A further
/// click while one runs adds ×2 to the target and the curve goes on from the current rate
/// (kb: decisions/controls "Animated zoom steps").
pub const TAP_ZOOM_SECONDS: f64 = 0.35;
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
/// "Fly here" (touch long-press menu) zooms in by this factor while centring the point.
pub const FLY_HERE_ZOOM: f64 = 8.0;
/// A released cruise slower than this (e-folds/s) no longer holds a journey's next leg back.
pub const LEG_CARRY_REST: f64 = 0.15;
/// A lane-assist nudge lights the cockpit's lane icon this long (s).
pub const ASSIST_FLASH: f64 = 0.6;
/// Below these the velocities snap to rest (points/s, e-folds/s).
const PAN_REST: f64 = 0.5;
const ZOOM_REST: f64 = 1e-3;

/// A double click / tap zoom glide (e-folds of zoom-in, positive): a quintic in the time
/// since `start` from the given offset, rate and acceleration to rest at `end` after
/// [`TAP_ZOOM_SECONDS`] (minimum jerk from rest; C² across chained clicks).
#[derive(Clone, Copy, Debug)]
struct TapZoom {
    start: f64,
    c: [f64; 6],
    end: f64,
    /// Offset the frames applied so far.
    applied: f64,
    at: (f64, f64),
}

impl TapZoom {
    fn new(start: f64, p0: f64, v0: f64, a0: f64, end: f64, at: (f64, f64)) -> Self {
        let t = TAP_ZOOM_SECONDS;
        let d = end - p0;
        let c = [
            p0,
            v0,
            a0 / 2.0,
            (20.0 * d - 12.0 * v0 * t - 3.0 * a0 * t * t) / (2.0 * t.powi(3)),
            (-30.0 * d + 16.0 * v0 * t + 3.0 * a0 * t * t) / (2.0 * t.powi(4)),
            (12.0 * d - 6.0 * v0 * t - a0 * t * t) / (2.0 * t.powi(5)),
        ];
        Self {
            start,
            c,
            end,
            applied: 0.0,
            at,
        }
    }

    /// Offset, rate and acceleration `e` seconds after the start (held at the end afterwards).
    fn eval(&self, e: f64) -> (f64, f64, f64) {
        if e >= TAP_ZOOM_SECONDS {
            return (self.end, 0.0, 0.0);
        }
        let e = e.max(0.0);
        let c = &self.c;
        let p = c[0] + e * (c[1] + e * (c[2] + e * (c[3] + e * (c[4] + e * c[5]))));
        let v = c[1] + e * (2.0 * c[2] + e * (3.0 * c[3] + e * (4.0 * c[4] + e * 5.0 * c[5])));
        let a = 2.0 * c[2] + e * (6.0 * c[3] + e * (12.0 * c[4] + e * 20.0 * c[5]));
        (p, v, a)
    }
}

/// What the target marker shows (kb: decisions/controls "Target marker"): where the autopilot
/// or a journey leg is heading (a complex point), a short name for a known kind of target, and
/// the pointer wish's direction (unit, view points, y up).
#[derive(Clone, Debug, PartialEq)]
pub struct AimMarker {
    pub center: Option<crate::viewport::Center>,
    pub label: Option<String>,
    pub wish: Option<(f64, f64)>,
}

/// Two-finger scroll set to zoom: e-folds per scrolled point, the rate of ⌘ + scroll
/// (`WheelZoom`: 1.15^(ΔY / 40), so ln 1.15 / 40; 100 points: about ×1.4).
const SCROLL_ZOOM_PER_POINT: f64 = 0.003_494_049;
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
    /// The double click / tap zoom glide in flight.
    tap_zoom: Option<TapZoom>,
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
    /// Touch (kb: decisions/controls "Touch"): the steering finger while it is down, the wish
    /// it left in A after it lifted, the direction of a throttle drag that braked to zero (it
    /// stops there until the finger lifts), and the orbit probe placed by touch.
    touch: Option<(f64, f64)>,
    /// The steering finger's last point after it lifted in A, and when it lifted.
    touch_wish: Option<((f64, f64), f64)>,
    /// A pinch or twist happened since the last `TouchesLifted`: the finger left on the
    /// screen neither pans nor steers.
    pinched: bool,
    /// The finger Rust classifies (rest or swipe) from its `FingerDown` until every finger is
    /// off (`TouchesLifted`); None when the finger is not Rust's (the pan recogniser pans).
    finger: Option<Finger>,
    throttle_rest: Option<f64>,
    /// The current touch twist: turned so far while below the dead band, or None once it
    /// turns the view (until `TouchRotateEnd`).
    twist: Option<f64>,
    probe: Option<(f64, f64)>,
    /// The navigator's own motion left the centre past its bound (the rubber band): it springs
    /// back once the hand is off. A centre out of bounds without it came from a jump and is
    /// clamped ([`Navigator::keep_in_limits`]).
    rubber: bool,
    /// When direct manipulation (drag, scroll, pinch, magnify, twist) last moved the view: the
    /// rubber band holds while the hand is on.
    hand_at: f64,
    /// When the spring back last tended the band (or the band was set): a band left alone
    /// longer than [`RUBBER_STUCK`] with the hand off is clamped ([`Navigator::keep_in_limits`]).
    spring_at: f64,
    /// The largest scale the current flight may show (its start's, if that was beyond the limit).
    flight_cap: Scale,
}

/// While direct manipulation arrived this recently (s), the hand is on the view: the rubber
/// band holds (trackpads send no event when the fingers rest).
const RUBBER_HOLD: f64 = 0.1;
/// A fling's decay time (s) once it has carried the centre past its bound.
const FLING_BAND_TAU: f64 = 0.05;
/// The spring back snaps onto the bound within this many view points.
const RUBBER_SNAP: f64 = 0.25;
/// A rubber band the spring back has not tended for this long (s) with the hand off is stuck:
/// clamped onto the bound ([`Navigator::keep_in_limits`]).
const RUBBER_STUCK: f64 = 0.5;

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
            tap_zoom: None,
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
            touch: None,
            touch_wish: None,
            pinched: false,
            finger: None,
            throttle_rest: None,
            twist: Some(0.0),
            probe: None,
            rubber: false,
            hand_at: f64::NEG_INFINITY,
            spring_at: f64::NEG_INFINITY,
            flight_cap: Scale::from_f64(crate::viewport::MAX_SCALE),
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

    /// A resting finger steers here (kb: decisions/controls "Touch", "Rest or swipe"): the
    /// autopilot, the cruise or a journey's flight flies. There one rule holds: *a finger that
    /// rests goes there, a finger that swipes moves the picture*, decided by Rust from the raw
    /// finger ([`InputEvent::FingerDown`], [`FINGER_SLOP`], [`FINGER_REST`]). Elsewhere (at
    /// rest, at a journey stop, a fly-to the user started) a drag pans and a held finger opens
    /// the menu. History: the device round made a drag pan in C (the finger had steered like the
    /// Mac's resting pointer, so a drag seemed to do nothing) and kept A's finger a wish (a line
    /// from the centre to the finger, which did not follow a swipe: "random"); the user wanted
    /// both, told apart by rest versus swipe.
    pub fn finger_steers(&self) -> bool {
        self.autopilot.is_active() || self.cruise.is_engaged() || self.journey_flying()
    }

    /// What a finger held still on the canvas does (kb: decisions/controls "Touch"): on a
    /// control nothing more (its tap acts), on a mini-map it flies there (the Mac's ⌥-click),
    /// in A, C and a journey's flight nothing for the platform (Rust classifies the finger:
    /// a rest steers; no menu while the point under it moves), else (the view at rest, or a
    /// fly-to the user started) the long-press menu. Nothing either for the finger left down
    /// after a pinch.
    pub fn touch_hold(&self, spot: TouchSpot) -> TouchHold {
        match spot {
            TouchSpot::Control | TouchSpot::Compass => TouchHold::Nothing,
            TouchSpot::Map => TouchHold::FlyToMap,
            TouchSpot::Canvas if self.pinched || self.finger_steers() => TouchHold::Nothing,
            TouchSpot::Canvas => TouchHold::Menu,
        }
    }

    /// The finger went down (kb: decisions/controls "Touch", "Rest or swipe"): Rust's to
    /// classify while a resting finger steers ([`Self::finger_steers`]), not on the compass,
    /// not after a pinch.
    fn finger_down(&mut self, x: f64, y: f64, spot: TouchSpot, now: f64) {
        self.finger = None;
        if self.pinched || spot == TouchSpot::Compass || !self.finger_steers() {
            return;
        }
        self.finger = Some(Finger {
            down: now,
            start: (x, y),
            last: (x, y),
            kind: FingerKind::Pending,
            on_canvas: spot == TouchSpot::Canvas,
        });
    }

    /// A pending finger that has stayed within [`FINGER_SLOP`] for [`FINGER_REST`] rests: on
    /// the picture it steers from where it is, elsewhere it is done.
    fn finger_rest_check(&mut self, now: f64) {
        let Some(f) = self.finger.as_mut() else { return };
        if f.kind != FingerKind::Pending || now - f.down < FINGER_REST {
            return;
        }
        if f.on_canvas {
            f.kind = FingerKind::Steer;
            let p = f.last;
            self.finger_steer(p, now);
        } else {
            f.kind = FingerKind::Done;
        }
    }

    /// A steering finger at `p`: the cruise steers towards it and in A it is the wish (as the
    /// round-1 `TouchSteer` did: not the pointer, so neither the zoom target nor the orbit
    /// probe). A journey's flight: off the dead zone it takes the wheel (a touch is deliberate:
    /// at once, where the Mac's pointer needs 60 points and 0.6 s), then the cruise steers.
    fn finger_steer(&mut self, p: (f64, f64), now: f64) {
        self.touch = Some(p);
        self.touch_wish = None;
        if self.journey_flying() && self.autopilot.steer_target(Some(p), self.steer_radius()) != (0.0, 0.0) {
            self.take_wheel(None, now);
        }
    }

    fn finger_move(&mut self, x: f64, y: f64, viewport: &mut Viewport, now: f64) -> bool {
        self.finger_rest_check(now);
        let Some(f) = self.finger.as_mut() else {
            return false;
        };
        let from = match f.kind {
            FingerKind::Pending if (x - f.start.0).hypot(y - f.start.1) >= FINGER_SLOP => {
                // A swipe: the picture catches up from where the finger went down.
                f.kind = FingerKind::Pan;
                f.start
            }
            _ => f.last,
        };
        f.last = (x, y);
        let kind = f.kind;
        match kind {
            FingerKind::Steer => self.finger_steer((x, y), now),
            FingerKind::Pan => self.finger_pan(x - from.0, y - from.1, viewport, now),
            FingerKind::Pending | FingerKind::Done => {}
        }
        true
    }

    /// A swiping finger moves the picture 1:1. In C and a journey's flight as a drag does (a
    /// journey hands over to the cruise). In A it pans without handing over (a Mac drag still
    /// does): the target lock lets go while manual input is younger than
    /// [`autopilot::MANUAL_HOLD`] (the heading eases to straight ahead, nothing pulls back) and
    /// then picks up anew from the moved view.
    fn finger_pan(&mut self, dx: f64, dy: f64, viewport: &mut Viewport, now: f64) {
        if self.autopilot.is_active() {
            self.manual_at = now;
            self.stop_flings();
            self.note_interaction(now);
            self.hand_at = now;
            self.pan_view(viewport, dx, dy);
            self.sample_gesture(now, dx, dy, 0.0, (0.0, 0.0));
        } else {
            self.apply(InputEvent::Pan { dx, dy }, viewport, now);
        }
    }

    fn finger_up(&mut self, cancelled: bool, now: f64) -> bool {
        if !cancelled {
            self.finger_rest_check(now);
        }
        let Some(f) = self.finger.as_mut() else {
            return false;
        };
        let kind = std::mem::replace(&mut f.kind, FingerKind::Done);
        match kind {
            FingerKind::Steer => {
                // A touch has no resting pointer: in A the wish is held until it settled, then
                // let go (`advance`), so the wish line fades.
                if self.autopilot.is_active() && !cancelled {
                    self.touch_wish = self.touch.map(|p| (p, now));
                }
                self.touch = None;
            }
            FingerKind::Pan if cancelled => self.gesture.clear(),
            FingerKind::Pan => self.release_gesture(now),
            FingerKind::Pending | FingerKind::Done => {}
        }
        true
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
            // A fly-to (no journey): its own speed.
            Level::Journey if !self.journey.is_running() => (
                self.fly_speed,
                self.flight.as_ref().map_or(0.0, |f| f.speed.v * flyto::RHO),
            ),
            // The journey's set speed and the leg's speed flown (paced along the leg, eased,
            // throttled by the renderer back-off).
            Level::Journey => (
                self.journey.speed(),
                self.flight.as_ref().map_or(0.0, |f| f.speed.v * flyto::RHO),
            ),
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
        let home = self.home();
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
        self.touch_wish = None;
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

    /// One detent of the touch throttle (kb: decisions/controls "Touch"): what an E / Q press
    /// does (a fly-to's speed during a fly-to, else the pedal), except that a drag which brought
    /// the level's speed to zero stops there: the lever crosses into the other direction only
    /// in a new drag, like a new key press.
    fn throttle(&mut self, dir: f64, viewport: &Viewport, now: f64) {
        self.note_interaction(now);
        if self.flight.is_some() && !self.journey.is_running() && self.cruise.landing().is_none() {
            let f = if dir > 0.0 {
                autopilot::SPEED_STEP
            } else {
                1.0 / autopilot::SPEED_STEP
            };
            self.scale_flight_speed(f, now);
            return;
        }
        let before = self.cockpit(now).set;
        let cross = self.throttle_rest != Some(dir);
        self.pedal(dir, cross, viewport, now);
        let after = self.cockpit(now).set;
        if after == 0.0 && before != 0.0 {
            self.throttle_rest = Some(dir);
        } else if after != 0.0 {
            self.throttle_rest = None;
        }
    }

    /// The touch throttle's knob put on signed gear `gear` (kb: decisions/controls "Touch"):
    /// that rung becomes the set speed of the level driving. See [`InputEvent::SetGear`].
    fn set_gear(&mut self, gear: i32, viewport: &Viewport, now: f64) {
        self.note_interaction(now);
        self.throttle_rest = None;
        let v = cruise::gear_speed(gear);
        if self.cruise.landing().is_some() {
            // Landing on the home view: the middle or forward is an eased stop where it is.
            if v >= 0.0 {
                self.cruise.stop(now);
                self.stop_flight();
            }
            return;
        }
        if self.flight.is_some() && !self.journey.is_running() {
            // A fly-to: its speed (unsigned; the path decides the direction); the middle stops it.
            if v == 0.0 {
                self.stop_flight();
            } else {
                self.set_fly_speed(v.abs(), now);
            }
            return;
        }
        let journey_drives = self.journey.wants_frames();
        if self.cruise.is_engaged() || !(self.autopilot.is_active() || journey_drives) {
            let out_ok = self.depth(viewport) > 0.01;
            if v == 0.0 {
                self.cruise.stop(now);
            } else {
                self.cruise.set_to(v, out_ok, now);
            }
            return;
        }
        if v > 0.0 {
            if self.autopilot.is_active() {
                self.autopilot.set_speed(v, now);
            } else {
                self.journey.set_speed(v, now);
            }
        } else if !self.take_wheel(Some(v), now) {
            self.journey.suspend();
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
        // The target within the limits; on the way a long flight may zoom out past the limit
        // by its path: capped (kb: decisions/controls "View limits").
        let l = self.limits();
        let mut target = target.clone();
        l.clamp(&mut target);
        self.flight_cap = l.max_scale.max(viewport.scale);
        let path = FlyPath::new(viewport, &target, self.view_size.0);
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
            InputEvent::Click { .. }
            | InputEvent::Scroll { .. }
            | InputEvent::WheelNotch { .. }
            | InputEvent::Throttle { .. }
            | InputEvent::SetGear { .. } => searching,
            InputEvent::SeekAt { .. } => false,
            ref e => Self::cancels_flight(e),
        }
    }

    /// Seek minibrot's state for the HUD: phase, period, the target's zoom (log10).
    pub fn seek_status(&self, now: f64) -> (SeekPhase, usize, f64) {
        self.seek.status(now)
    }

    /// The fractal the navigator is in (F3, kept in step with the settings by the host): seek
    /// searches its set and the home view (Reset, the autopilot's way out) is its whole set.
    pub fn set_formula(&mut self, formula: crate::formula::Formula) {
        self.seek.formula = formula;
    }

    /// The home view fitted to the window (the current formula's whole set).
    fn home(&self) -> Viewport {
        Viewport::fitting_for(self.seek.formula, self.view_size.0, self.view_size.1)
    }

    // MARK: View limits (kb: decisions/controls "View limits")

    /// Where navigation may take the view: the current formula's home at the current size.
    pub fn limits(&self) -> ViewLimits {
        ViewLimits::new(self.seek.formula, self.view_size.0, self.view_size.1)
    }

    /// The centre is past its bound on the rubber band (springing back or held by the hand).
    pub fn is_stretched(&self) -> bool {
        self.rubber
    }

    /// Clamps a view set from outside the navigator (a jump, a restored or pasted view, a
    /// formula or window change) into the limits at once; the navigator's own rubber band is
    /// left to spring back. Hosts call it before each frame. True if the view changed.
    pub fn keep_in_limits(&mut self, viewport: &mut Viewport) -> bool {
        // Not before the view has a size (a view restored before the first layout).
        if !(self.view_size.0 > 0.0 && self.view_size.1 > 0.0) {
            return false;
        }
        let l = self.limits();
        let mut changed = false;
        let cap = if self.flight.is_some() {
            l.max_scale.max(self.flight_cap)
        } else {
            l.max_scale
        };
        if viewport.scale > cap {
            viewport.scale = cap;
            viewport.ensure_precision();
            changed = true;
        }
        if l.past(&viewport.center, viewport.scale) {
            if !self.rubber {
                changed |= l.clamp(viewport);
            } else {
                // The navigator's own band: never further out than the band, whatever moved
                // the centre there; onto the bound if the spring back stopped tending it.
                changed |= l.clamp_stretch(viewport, self.band());
                if let Some(now) = self.last_tick
                    && !self.hand_on(now)
                    && now - self.spring_at > RUBBER_STUCK
                {
                    changed |= l.clamp(viewport);
                    self.rubber = false;
                }
            }
        }
        changed
    }

    /// The most the rubber band stretches past the centre bound (view points).
    fn band(&self) -> f64 {
        RUBBER_BAND * self.min_side()
    }

    /// The centre's move left it past its bound: the band is set (it springs back), capped at
    /// its most.
    fn stretched(&mut self, viewport: &mut Viewport) {
        let l = self.limits();
        if !l.past(&viewport.center, viewport.scale) {
            return;
        }
        if !self.rubber {
            self.rubber = true;
            self.spring_at = self.last_tick.unwrap_or(f64::NEG_INFINITY);
        }
        l.clamp_stretch(viewport, self.band());
    }

    /// The hand is on the view: direct manipulation just moved it, or a finger pans.
    fn hand_on(&self, now: f64) -> bool {
        now - self.hand_at < RUBBER_HOLD || self.finger.is_some_and(|f| f.kind == FingerKind::Pan)
    }

    /// Moves the centre by (x, y) points along the complex axes through the rubber band.
    fn move_center(&mut self, viewport: &mut Viewport, x: f64, y: f64) {
        if !(x.is_finite() && y.is_finite()) || (x == 0.0 && y == 0.0) {
            return;
        }
        let l = self.limits();
        let past = l.edge_distances(&viewport.center, viewport.scale);
        let band = self.band();
        let mx = ViewLimits::rubber_move(past[0], x, band);
        let my = ViewLimits::rubber_move(past[1], y, band);
        viewport.center = viewport.center.offset_scaled(mx, my, viewport.scale);
        self.stretched(viewport);
    }

    /// Moves the content by (dx, dy) view points ([`Viewport::pan_by_points`]) through the
    /// rubber band.
    fn pan_view(&mut self, viewport: &mut Viewport, dx: f64, dy: f64) {
        let (x, y) = viewport.rotate(dx, dy);
        self.move_center(viewport, -x, -y);
    }

    /// Zooms by `factor` at (x, y) ([`Viewport::zoom_by`]) within the limits: a zoom out eases
    /// into the zoom-out limit, and the centre's move (a zoom about an off-centre point moves
    /// it) goes through the rubber band (near the bound the point under the pointer then
    /// slides).
    fn zoom_view(&mut self, viewport: &mut Viewport, factor: f64, x: f64, y: f64) {
        let l = self.limits();
        let f = l.zoom_factor(viewport.scale, factor);
        if !(f.is_finite() && f > 0.0) || f == 1.0 {
            return;
        }
        let before = viewport.center.clone();
        let s0 = viewport.scale;
        viewport.zoom_by(f, x, y);
        // The centre moved by at most |(x, y)|·|s0 − s1| complex units: far inside the box that
        // cannot reach the bound.
        let reach = x.hypot(y) * (s0.to_f64() - viewport.scale.to_f64()).abs() + 1e-9;
        let (re, im) = (before.re() - l.center.0, before.im() - l.center.1);
        let room = crate::viewport::CENTER_LIMIT - re.abs().max(im.abs());
        if room > reach && !self.rubber {
            return;
        }
        let (mx, my) = viewport.center.delta_in(&before, viewport.scale);
        viewport.center = before.with_min_precision(viewport.needed_center_bits());
        self.move_center(viewport, mx, my);
        // A zoom in while stretched shows the stretch larger: capped at the band (the centre
        // slides towards the bound).
        self.stretched(viewport);
    }

    /// Turns by `delta` about (x, y) ([`Viewport::rotate_by`]); a turn about an off-centre
    /// point moves the centre, through the rubber band like any other move (it springs back).
    fn rotate_view(&mut self, viewport: &mut Viewport, delta: f64, x: f64, y: f64) {
        let before = viewport.center.clone();
        viewport.rotate_by(delta, x, y);
        let (mx, my) = viewport.center.delta_in(&before, viewport.scale);
        if !(mx.is_finite() && my.is_finite()) {
            self.stretched(viewport);
            return;
        }
        viewport.center = before;
        self.move_center(viewport, mx, my);
    }

    /// The spring back onto the centre bound once the hand is off: the excess decays with
    /// [`RUBBER_RETURN_TAU`] (exact for any frame time) and snaps onto the bound at the end.
    fn spring_back(&mut self, viewport: &mut Viewport, dt: f64, now: f64) {
        if !self.rubber {
            return;
        }
        self.spring_at = now;
        let l = self.limits();
        if !l.past(&viewport.center, viewport.scale) {
            self.rubber = false;
            return;
        }
        if self.hand_on(now) {
            return;
        }
        let past = l.edge_distances(&viewport.center, viewport.scale);
        let excess = |(hi, lo): (f64, f64)| {
            if hi > 0.0 {
                hi
            } else if lo > 0.0 {
                -lo
            } else {
                0.0
            }
        };
        let (ex, ey) = (excess(past[0]), excess(past[1]));
        let keep = (-dt / RUBBER_RETURN_TAU).exp();
        if ex.hypot(ey) * keep < RUBBER_SNAP || !(ex.is_finite() && ey.is_finite()) {
            l.clamp(viewport);
            self.rubber = false;
        } else {
            viewport.center = viewport
                .center
                .offset_scaled(-ex * (1.0 - keep), -ey * (1.0 - keep), viewport.scale);
        }
        self.note_interaction(now);
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

    /// The platform should keep asking for frames: motion, a glide, or a minibrot search to be
    /// polled. An event that leaves only a search running (the long-press "Seek Minibrot Here")
    /// asked for none, so the search was never polled until the next touch (QA 2026-10-18).
    pub fn wants_frames(&self) -> bool {
        self.is_animating() || self.seek.is_searching()
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
            || self.tap_zoom.is_some()
            || self.autopilot.is_moving()
            || self.cruise.is_moving()
            || self.pedal_hold.is_some()
            || self.flight.is_some()
            || !self.rot_rate.at_rest()
            || self.rot_step_left != 0.0
            || self.journey.wants_frames()
            || self.north.is_some()
            || self.rubber
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
        // A pending finger needs a frame to be found resting.
        self.is_moving() || self.is_gliding() || self.finger.is_some_and(|f| f.kind == FingerKind::Pending)
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

    /// Every held input lets go as if released, because its release may never arrive
    /// (`InputEvent::FocusLost`: the app or window lost focus, the view left its window, the app
    /// went to the background; kb: decisions/controls "Held input"): keys, Shift, the pedals
    /// (the cruise keeps its gear, as after a key-up), the gamepad, a mouse drag or finger (no
    /// fling: the motion it made glides out on its springs only), the touch throttle and twist.
    /// Motion already under way eases out, so the view settles and the display link pauses.
    pub fn release_all(&mut self) {
        self.release_holds();
        self.shift = false;
        self.pinched = false;
        self.finger = None;
        self.touch = None;
        self.gesture.clear();
        self.throttle_rest = None;
        self.twist = Some(0.0);
    }

    /// The holds that drive motion on their own each frame (held keys, a held pedal, a
    /// deflected gamepad) let go.
    fn release_holds(&mut self) {
        self.up = false;
        self.down = false;
        self.left = false;
        self.right = false;
        self.zoom = false;
        self.rot_ccw = false;
        self.rot_cw = false;
        self.pad = GamepadState::default();
        self.pedal_hold = None;
    }

    /// The wish a lifted finger left in A has done its work: the autopilot's settled wish points
    /// its way (the lock re-targeted towards it once), or it was held twice the settle time
    /// (a wish that never settled: a lift inside the dead zone, A paused).
    fn touch_wish_settled(&self, p: (f64, f64), lifted: f64, now: f64) -> bool {
        let r = p.0.hypot(p.1);
        let agrees = self
            .autopilot
            .wish()
            .is_some_and(|w| r > 0.0 && (p.0 * w.dir.0 + p.1 * w.dir.1) / r >= autopilot::WISH_TURN.cos());
        agrees || now - lifted >= 2.0 * autopilot::WISH_SETTLE
    }

    /// A jump to a given view (pasted coordinates, a link, a place): everything that moves
    /// stops dead and a journey ends, so the new view is not carried off by the old motion (the
    /// user's request). Held keys, a held pedal and the gamepad let go too (a held E would
    /// otherwise start a new cruise at once; kb: decisions/controls "Held input"): a key still
    /// down moves again on its next press. Settings (speeds, lane assist) stay.
    pub fn jumped(&mut self) {
        self.stop_motion();
        self.rubber = false;
        self.journey.stop();
        self.release_holds();
    }

    /// Stops all motion at once (Reset, a new flight).
    fn stop_motion(&mut self) {
        self.autopilot.halt();
        self.cruise.halt();
        self.flight = None;
        self.pan = [Spring::default(); 2];
        self.zoom_rate = Spring::default();
        self.stop_flings();
        self.step_left = 0.0;
        self.step_at = None;
        self.tap_zoom = None;
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

    /// Adds a double click / tap zoom of `factor` at (x, y): a glide of [`TAP_ZOOM_SECONDS`]
    /// to the target. One already in flight is extended: its target moves by `factor` and the
    /// new curve starts from the current offset, rate and acceleration (no restart, no
    /// stutter); the zoom point is the latest click's.
    fn tap_zoom(&mut self, factor: f64, x: f64, y: f64, now: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        self.note_interaction(now);
        // State of the running glide at `now`, relative to what frames already applied.
        let (p, v, a, end) = match self.tap_zoom {
            Some(z) => {
                let (p, v, a) = z.eval(now - z.start);
                (p - z.applied, v, a, z.end - z.applied)
            }
            None => (0.0, 0.0, 0.0, 0.0),
        };
        self.tap_zoom = Some(TapZoom::new(now, p, v, a, end + factor.ln(), (x, y)));
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
        // Touch (kb: decisions/controls "Touch", device round): once a pinch or twist began, the
        // finger left down when the other lifts neither pans nor steers until every finger is
        // off the screen (its pan would also measure a fling).
        match event {
            InputEvent::Pinch { .. } | InputEvent::TouchRotate { .. } => {
                self.pinched = true;
                self.touch = None;
                if let Some(f) = self.finger.as_mut() {
                    f.kind = FingerKind::Done;
                }
            }
            InputEvent::Pan { .. } | InputEvent::FingerMove { .. } if self.pinched => return false,
            // Rest or swipe (kb: decisions/controls "Touch"): a finger Rust classifies pans from
            // its own moves; the pan recogniser's events for it are swallowed (its end too, but
            // a pinch's end still measures the pinch's fling).
            InputEvent::Pan { .. } if self.finger.is_some() => return true,
            InputEvent::GestureEnd if self.finger.is_some() && !self.pinched => return true,
            InputEvent::TouchesLifted => {
                self.finger_up(true, now);
                self.finger = None;
                self.pinched = false;
                return true;
            }
            _ => {}
        }
        let event = match event {
            InputEvent::Resize { width, height } => InputEvent::Resize {
                width: width.max(0.0),
                height: height.max(0.0),
            },
            // A touch twist turns the view only past the dead band, then on from there.
            InputEvent::TouchRotate { radians, x, y } => match self.twist {
                None => InputEvent::Rotate { radians, x, y },
                Some(sum) => {
                    let sum = sum + radians;
                    if sum.abs() < TWIST_DEAD_BAND {
                        self.twist = Some(sum);
                        return true;
                    }
                    self.twist = None;
                    InputEvent::Rotate {
                        radians: sum - TWIST_DEAD_BAND.copysign(sum),
                        x,
                        y,
                    }
                }
            },
            InputEvent::TouchRotateEnd => {
                self.twist = Some(0.0);
                return true;
            }
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
                        target = Viewport::fitting_for(self.seek.formula, width, height).with_angle(target.angle);
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
                    // The same direction as ⌘ + scroll and the wheel (`WheelZoom`: the OS's ΔY = −dy,
                    // natural scrolling already applied); the momentum after lifting coasts on.
                    if dy != 0.0 && dy.is_finite() {
                        self.manual_zoom_ends_cruise();
                        let (tx, ty) = self.zoom_target();
                        self.zoom_at(viewport, (-dy * SCROLL_ZOOM_PER_POINT).exp(), tx, ty, now);
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
                    self.hand_at = now;
                    let (tx, ty) = self.gesture_target(x, y);
                    // The image follows the fingers: counter-clockwise fingers, the camera turns clockwise.
                    self.rotate_view(viewport, -radians, tx, ty);
                }
                true
            }
            // Mapped to `Rotate` (or nothing) by `handle`'s dead band; `TouchesLifted` taken there.
            InputEvent::TouchRotate { .. } | InputEvent::TouchRotateEnd | InputEvent::TouchesLifted => false,
            InputEvent::Pan { dx, dy } => {
                // The hand has the view: flings stop; motion is 1:1.
                self.stop_flings();
                self.note_interaction(now);
                self.hand_at = now;
                self.pan_view(viewport, dx, dy);
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
                self.hand_at = now;
                // Two-finger drag pans while pinching.
                self.pan_view(viewport, dx, dy);
                self.zoom_view(viewport, ratio, x, y);
                self.sample_gesture(now, dx, dy, ratio.ln(), (x, y));
                true
            }
            InputEvent::GestureEnd => {
                self.hand_at = f64::NEG_INFINITY;
                self.release_gesture(now);
                true
            }
            InputEvent::FingerDown { x, y, spot } => {
                self.finger_down(x, y, spot, now);
                self.finger.is_some()
            }
            InputEvent::FingerMove { x, y } => self.finger_move(x, y, viewport, now),
            InputEvent::FingerUp { cancelled } => self.finger_up(cancelled, now),
            InputEvent::Throttle { up } => {
                self.throttle(if up { 1.0 } else { -1.0 }, viewport, now);
                true
            }
            InputEvent::ThrottleEnd => {
                self.throttle_rest = None;
                true
            }
            InputEvent::SetGear { gear } => {
                self.set_gear(gear, viewport, now);
                true
            }
            InputEvent::Probe(p) => {
                self.probe = p;
                true
            }
            InputEvent::SeekAt { x, y } => {
                let at =
                    Viewport::with_center(viewport.point_at_offset(x, y), viewport.scale).with_angle(viewport.angle);
                self.seek.start(&at, self.min_side(), now);
                self.poll_seek(viewport, now);
                true
            }
            InputEvent::FlyHere { x, y } => {
                let mut target = Viewport::with_center(viewport.point_at_offset(x, y), viewport.scale / FLY_HERE_ZOOM)
                    .with_angle(viewport.angle);
                target.ensure_precision();
                self.fly_to(&target, viewport, now);
                true
            }
            InputEvent::DoubleTap { x, y, out, .. } => {
                self.manual_zoom_ends_cruise();
                // Its first click toggled the autopilot's pause: undone.
                self.autopilot.double_click(now);
                self.enter_pointer_mode();
                self.tap_zoom(if out { 0.5 } else { 2.0 }, x, y, now);
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
        self.hand_at = now;
        self.zoom_view(viewport, factor, x, y);
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
                *viewport = self.home();
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

    /// The orbit overlay's point c (view points from the centre, y up): the probe a touch
    /// placed (kb: decisions/controls "Touch"), else the zoom target.
    pub fn probe_point(&self) -> (f64, f64) {
        self.probe.unwrap_or_else(|| self.zoom_target())
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
        self.finger_rest_check(now);
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
            let cap = self.limits().max_scale.max(self.flight_cap);
            let f = self.flight.as_mut().expect("flight");
            self.last_interaction = now;
            self.autopilot.update_throttle(dt);
            let done = f.step(dt, speed, self.autopilot.throttle());
            *viewport = f.viewport();
            if viewport.scale > cap {
                viewport.scale = cap;
            }
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
            self.spring_back(viewport, dt, now);
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
        // A steering finger (touch) steers while it is down, without the idle timeout.
        let steer_pointer = match self.mode {
            _ if self.touch.is_some() => self.touch,
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
        // A touch is deliberate: its wish needs no travel, and stays after the finger lifted.
        if let Some((p, lifted)) = self.touch_wish
            && self.touch_wish_settled(p, lifted, now)
        {
            self.touch_wish = None;
        }
        let wish_pointer = match self.mode {
            _ if self.touch.is_some() || self.touch_wish.is_some() => self.touch.or(self.touch_wish.map(|w| w.0)),
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
            // Past the centre bound the band brakes a fling hard (then it springs back).
            let tau = if self.rubber { FLING_BAND_TAU } else { FLING_TAU };
            let e = (-dt / tau).exp();
            dx += self.fling.0 * tau * (1.0 - e);
            dy += self.fling.1 * tau * (1.0 - e);
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
            self.pan_view(viewport, dx * mid, dy * mid);
        }
        if ln_zoom != 0.0 {
            let (x, y) = self.zoom_target();
            self.zoom_view(viewport, ln_zoom.exp(), x, y);
            ln_zoom = 0.0;
        }
        if ap.ln_zoom != 0.0 {
            self.zoom_view(viewport, ap.ln_zoom.exp(), ap.at.0, ap.at.1);
            // Cinematic spin: the view turns with the autopilot's zoom, about its dive point.
            let spin = self.cinematic.spin_per_efold();
            if spin != 0.0 {
                self.rotate_view(viewport, spin * ap.ln_zoom, ap.at.0, ap.at.1);
            }
        }
        if cm.ln_zoom != 0.0 {
            self.zoom_view(viewport, cm.ln_zoom.exp(), cm.at.0, cm.at.1);
        }
        self.cruise_landing(viewport);
        if turn != 0.0 {
            let (x, y) = self.zoom_target();
            self.rotate_view(viewport, turn, x, y);
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
            self.zoom_view(viewport, ln_zoom.exp(), x, y);
        }
        // Animated zoom steps: exponential ease-out of what is left.
        if self.step_left != 0.0 {
            let mut part = self.step_left * (1.0 - (-dt / STEP_TAU).exp());
            if (self.step_left - part).abs() < 1e-4 {
                part = self.step_left;
            }
            self.step_left -= part;
            let (x, y) = self.step_at.unwrap_or((0.0, 0.0));
            self.zoom_view(viewport, part.exp(), x, y);
            if self.step_left == 0.0 {
                self.step_at = None;
            }
        }
        // The double click / tap glide: by the clock (exact duration whatever the frame rate).
        if let Some(z) = self.tap_zoom.as_mut() {
            let e = if now.is_finite() { now - z.start } else { 0.0 };
            let (p, _, _) = z.eval(e);
            let part = p - z.applied;
            z.applied = p;
            let (x, y) = z.at;
            if e >= TAP_ZOOM_SECONDS {
                self.tap_zoom = None;
            }
            if part != 0.0 {
                self.zoom_view(viewport, part.exp(), x, y);
            }
        }
        // Past the centre bound with the hand off: back onto it.
        self.spring_back(viewport, dt, now);
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
        let home = self.home();
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

    /// Setting "Two-finger scroll: Zoom": the OS's ΔY > 0 (dy < 0) zooms in like ⌘ + scroll, the
    /// other way out, sideways nothing; default pans.
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
                dy: -100.0,
                momentum: false,
            },
            &mut v2,
            0.0,
        );
        let s1 = v2.scale.to_f64();
        assert!((s0 / s1 - 1.15f64.powf(2.5)).abs() < 1e-6, "{}", s0 / s1);
        n2.handle(
            InputEvent::Scroll {
                dx: 0.0,
                dy: 100.0,
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
                command: false,
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
        // A wheel notch worth ×2.
        n.handle(
            InputEvent::WheelNotch {
                delta: 2f64.ln() / 1.15f64.ln(),
                x: 50.0,
                y: -20.0,
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

    /// Scale and the screen drift (points) of the complex point that was under (x, y).
    fn tap_drift(v: &Viewport, c: &crate::viewport::Center, x: f64, y: f64) -> f64 {
        let (dr, di) = v.point_at_offset(x, y).delta_from(c);
        dr.hypot(di) / v.scale.to_f64()
    }

    fn double_tap(n: &mut Navigator, v: &mut Viewport, x: f64, y: f64, out: bool, t: f64) {
        assert!(n.handle(
            InputEvent::DoubleTap {
                x,
                y,
                out,
                command: false
            },
            v,
            t
        ));
    }

    #[test]
    fn double_tap_zoom_eases_in_and_out() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        let c = v.point_at_offset(50.0, -20.0);
        double_tap(&mut n, &mut v, 50.0, -20.0, false, 0.0);
        // No jump: one 120 Hz frame moves a tiny share of the ×2 (minimum jerk: 10 s³ ≈ 0.02 %).
        n.advance(&mut v, 1.0 / 120.0);
        let frac = (1e-3 / v.scale.to_f64()).ln() / 2f64.ln();
        assert!(frac > 0.0 && frac < 1e-3, "{frac}");
        // Half way at half the time (the curve is symmetric), the zoom point holds throughout.
        let mut t = 1.0 / 120.0;
        let mut last = v.scale.to_f64();
        let mut half = None;
        while n.is_moving() {
            t += 1.0 / 120.0;
            n.advance(&mut v, t);
            let s = v.scale.to_f64();
            assert!(s <= last, "monotonic");
            last = s;
            if half.is_none() && s <= 1e-3 / 2f64.sqrt() {
                half = Some(t);
            }
            assert!(tap_drift(&v, &c, 50.0, -20.0) < 1e-6);
            assert!(t < 1.0);
        }
        let half = half.unwrap();
        assert!((half - TAP_ZOOM_SECONDS / 2.0).abs() < 0.01, "{half}");
        assert!((t - TAP_ZOOM_SECONDS).abs() < 0.01, "{t}");
        assert!((v.scale.to_f64() - 5e-4).abs() < 1e-15, "{}", v.scale.to_f64());
        // Option / right double click: ×0.5 back.
        double_tap(&mut n, &mut v, 50.0, -20.0, true, t);
        run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!((v.scale.to_f64() - 1e-3).abs() < 1e-15);
    }

    #[test]
    fn chained_double_taps_accumulate_smoothly() {
        // Clicks every `gap` seconds (clicks 2, 3 and 4 of a click stream zoom): ×8 in all,
        // the scale never turns back, and the zoom rate never jumps between frames.
        for gap in [0.08, 0.15, 0.2, 0.3, 0.5] {
            for hz in [60.0, 120.0] {
                let (mut n, mut v) = setup();
                n.advance(&mut v, 0.0);
                let mut t = 0.0;
                let mut clicks = vec![0.0, gap, 2.0 * gap];
                let mut last = v.scale.to_f64();
                let mut last_rate: Option<f64> = None;
                let mut max_rate: f64 = 0.0;
                let mut max_rate_jump: f64 = 0.0;
                for _ in 0..(3.0 * hz) as usize {
                    if clicks.first().is_some_and(|&c| t >= c) {
                        clicks.remove(0);
                        double_tap(&mut n, &mut v, 30.0, 40.0, false, t);
                    }
                    t += 1.0 / hz;
                    n.advance(&mut v, t);
                    let s = v.scale.to_f64();
                    assert!(s <= last * (1.0 + 1e-12), "gap {gap}: turned back at {t}");
                    let rate = (last / s).ln() * hz;
                    if let Some(r) = last_rate {
                        max_rate_jump = max_rate_jump.max((rate - r).abs());
                    }
                    max_rate = max_rate.max(rate);
                    last_rate = Some(rate);
                    last = s;
                }
                assert!(!n.is_moving());
                assert!(
                    (1e-3 / v.scale.to_f64() - 8.0).abs() < 1e-9,
                    "gap {gap}: ×{}",
                    1e-3 / v.scale.to_f64()
                );
                // Smooth: no frame-to-frame jump in the rate, only bounded acceleration (a
                // single ×2 glide peaks at 5.8 · ln 2 / T² = 33 e-folds/s², 9 /s of its peak rate; measured 6.6-9.2 /s for these streams at both rates).
                let accel = max_rate_jump * hz / max_rate;
                assert!(
                    accel < 12.0,
                    "gap {gap} {hz} Hz: rate jump {max_rate_jump} of {max_rate}"
                );
            }
        }
        // A click after the glide ended starts a new one from rest.
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        double_tap(&mut n, &mut v, 0.0, 0.0, false, 0.0);
        let t = run(&mut n, &mut v, 0.0, 1.0, 60.0);
        double_tap(&mut n, &mut v, 0.0, 0.0, false, t);
        n.advance(&mut v, t + 1.0 / 60.0);
        assert!((5e-4 / v.scale.to_f64()).ln() < 2e-3 * 2f64.ln());
        run(&mut n, &mut v, t + 1.0 / 60.0, t + 1.0, 60.0);
        assert!((v.scale.to_f64() - 2.5e-4).abs() < 1e-15);
    }

    #[test]
    fn click_streams_keep_autopilot_pause() {
        // Autopilot on: a click pauses; the double click (its second click) undoes that; the
        // third and fourth clicks of the stream zoom on and never toggle the pause.
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        assert!(n.autopilot().is_active() && !n.autopilot().is_paused());
        assert!(n.handle(
            InputEvent::Click {
                x: 5.0,
                y: 5.0,
                command: false
            },
            &mut v,
            1.0
        ));
        assert!(n.autopilot().is_paused());
        for (k, t) in [1.2, 1.4, 1.6].into_iter().enumerate() {
            double_tap(&mut n, &mut v, 5.0, 5.0, false, t);
            assert!(!n.autopilot().is_paused(), "click {}", k + 2);
        }
        // Paused before a stream: its first click resumes, the second undoes that, the rest
        // leave it paused.
        assert!(n.handle(
            InputEvent::Click {
                x: 5.0,
                y: 5.0,
                command: false
            },
            &mut v,
            3.0
        ));
        assert!(n.autopilot().is_paused());
        assert!(n.handle(
            InputEvent::Click {
                x: 5.0,
                y: 5.0,
                command: false
            },
            &mut v,
            5.0
        ));
        assert!(!n.autopilot().is_paused());
        for t in [5.2, 5.4, 5.6, 5.8] {
            double_tap(&mut n, &mut v, 5.0, 5.0, false, t);
            assert!(n.autopilot().is_paused());
        }
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
        // Holding Q from rest: reverse, one rung more after 0.4 s and every 0.15 s.
        n.handle(InputEvent::KeyDown(NavKey::SlowerZoom), &mut v, t);
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(
            n.cruise().set_speed(),
            -cruise::rung(4),
            "held 1 s: four repeats (0.4 s, then every 0.15 s)"
        );
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
            // Gear 10 (×5/s out): far enough out within 4.8 s for the depth boost to matter.
            for _ in 0..10 {
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
                // The steered centre within 3 points (of 600) after about 9 e-folds out: the
                // boost depends on the depth the zoom changes (first order in the 2 ms substeps).
                let d = (a[i].0 - o.0).hypot(a[i].1 - o.1);
                assert!(d < 3.0, "pan {i}: {:?} vs {:?}", a[i], o);
            }
        }
        assert!(a[9].2 > 8.0, "zoomed out: {:?}", a[9]);
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
        // 11 presses from 1.4 e-folds/s: ×60/s on the 26-gear ladder (6 on the old 12 rungs).
        for (presses, max_acc) in [(0, 16.0), (11, 24.0)] {
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
                // On to gear 4 (×1.5/s out, the old first rung): the lowest gears crawl.
                for _ in 0..3 {
                    tap(&mut n, &mut v, NavKey::SlowerZoom, t);
                }
                assert_eq!(n.cruise().set_speed(), -cruise::rung(3));
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
        // The cockpit: the journey's set speed, and the leg's speed flown.
        let c = n.cockpit(t);
        assert_eq!(c.set, n.journey().speed());
        assert!(c.rate > 0.0 && c.rate == n.flight().unwrap().speed.v * flyto::RHO);
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
        for _ in 0..cruise::LADDER.len() + 2 {
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
        for _ in 0..8 {
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
        assert!(!n.handle(
            InputEvent::Click {
                x: 0.0,
                y: 0.0,
                command: false
            },
            &mut v,
            0.0
        ));
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
        assert!(n.handle(
            InputEvent::Click {
                x: 5.0,
                y: 5.0,
                command: false
            },
            &mut v,
            2.0
        ));
        assert!(n.autopilot().is_paused());
        let t = run(&mut n, &mut v, 2.0, 5.0, 60.0);
        assert!(!n.is_moving() && !n.advance(&mut v, t + 0.01));
        assert!(n.handle(
            InputEvent::Click {
                x: 5.0,
                y: 5.0,
                command: false
            },
            &mut v,
            t
        ));
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
            n.handle(
                InputEvent::Click {
                    x: 0.0,
                    y: 0.0,
                    command: false,
                },
                &mut v,
                t + 1.1,
            );
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

    // MARK: Touch (kb: decisions/controls "Touch")

    /// The throttle lever steps the pedal ladder like E / Q (the same speeds as the keys), and
    /// within one drag stops at zero; a new drag crosses into reverse.
    #[test]
    fn throttle_steps_the_gears_with_a_detent_at_zero_per_drag() {
        let (mut n, mut v) = deep("1e-30");
        let (mut keys, mut kv) = deep("1e-30");
        let up = InputEvent::Throttle { up: true };
        let down = InputEvent::Throttle { up: false };
        for _ in 0..3 {
            assert!(n.handle(up, &mut v, 0.0));
            tap(&mut keys, &mut kv, NavKey::FasterZoom, 0.0);
        }
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), cruise::rung(2));
        assert_eq!(n.cruise().set_speed(), keys.cruise().set_speed(), "the keys' rungs");
        // Down through the gears in the same drag: it stops at zero, however far it goes.
        for _ in 0..6 {
            n.handle(down, &mut v, 0.0);
        }
        assert_eq!(n.cruise().set_speed(), 0.0, "stopped at the detent");
        n.handle(InputEvent::ThrottleEnd, &mut v, 0.0);
        // A new drag crosses into reverse.
        n.handle(down, &mut v, 0.1);
        assert_eq!(n.cruise().set_speed(), -cruise::rung(0));
        n.handle(down, &mut v, 0.1);
        assert_eq!(n.cruise().set_speed(), -cruise::rung(1));
        // Back up in the same drag: it stops at zero again, the other way.
        for _ in 0..4 {
            n.handle(up, &mut v, 0.1);
        }
        assert_eq!(n.cruise().set_speed(), 0.0);
        n.handle(InputEvent::ThrottleEnd, &mut v, 0.1);
        n.handle(up, &mut v, 0.2);
        assert_eq!(n.cruise().set_speed(), cruise::rung(0));
        // In A it sets the autopilot's speed on the same ladder; down to zero hands over to a
        // braking cruise, as Q does.
        let (mut n, mut v) = joystick();
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        let s0 = n.autopilot().speed();
        n.handle(up, &mut v, 0.0);
        assert_eq!(n.level(), Level::Autopilot);
        assert_eq!(n.autopilot().speed(), cruise::pedal(s0, 1.0, true));
        for _ in 0..40 {
            n.handle(down, &mut v, 0.0);
        }
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), 0.0, "no reverse within the drag");
    }

    /// The position lever (kb: decisions/controls "Touch"): `SetGear` puts the level driving
    /// on that rung, with no detent at zero; the cockpit reports the gear back, so the knob
    /// follows changes made elsewhere (keys, the autopilot, a fly-to).
    #[test]
    fn set_gear_puts_the_level_on_that_rung() {
        let gear = |n: &Navigator| cruise::signed_gear(n.cockpit(0.0).set);
        let set = |g: i32| InputEvent::SetGear { gear: g };
        // From rest: cruise in that gear; straight across zero into reverse; 0 brakes to M.
        let (mut n, mut v) = deep("1e-30");
        assert!(n.handle(set(5), &mut v, 0.0));
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), cruise::rung(4));
        assert_eq!(gear(&n), 5);
        n.handle(set(-3), &mut v, 0.1);
        assert_eq!(n.cruise().set_speed(), -cruise::rung(2), "no detent at zero");
        assert_eq!(gear(&n), -3);
        // The keys move the gear the knob shows.
        tap(&mut n, &mut v, NavKey::SlowerZoom, 0.2);
        assert_eq!(gear(&n), -4);
        n.handle(set(0), &mut v, 0.3);
        assert_eq!(n.cruise().set_speed(), 0.0);
        assert_eq!(gear(&n), 0);
        let t = run(&mut n, &mut v, 0.3, 4.0, 60.0);
        assert_eq!(n.level(), Level::Manual, "braked to rest");
        // Out of range: the top gear.
        n.handle(set(99), &mut v, t);
        assert_eq!(gear(&n), 26);
        // At home there is nothing to zoom out to.
        let (mut n, _) = setup();
        let mut v = Viewport::fitting(W, 600.0);
        n.advance(&mut v, 0.0);
        n.handle(set(-2), &mut v, 0.0);
        assert_eq!(n.level(), Level::Manual);
        assert_eq!(gear(&n), 0);
        // In A: the autopilot's speed (its range clamps the top gears); the knob shows the
        // autopilot's own speed changes; zero or reverse hands over to the cruise.
        let (mut n, mut v) = joystick();
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        assert_eq!(gear(&n), 7, "the default ×2.46/s");
        // E from the off-ladder default goes to its rung (×2.5, still gear 7), then on.
        tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        assert_eq!(gear(&n), 8);
        n.handle(set(3), &mut v, 0.0);
        assert_eq!(n.level(), Level::Autopilot);
        assert_eq!(n.autopilot().speed(), cruise::rung(2));
        assert_eq!(gear(&n), 3);
        n.handle(set(26), &mut v, 0.0);
        assert_eq!(n.autopilot().speed(), autopilot::SPEED_RANGE.1);
        assert_eq!(gear(&n), 20, "the autopilot's top speed ×55/s");
        n.handle(set(-2), &mut v, 0.0);
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), -cruise::rung(1));
        // A fly-to: its speed; the middle stops it.
        let target = crate::flyto::view_from_text("-0.743643887037151", "0.13182590420533", "1e-20").unwrap();
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        n.fly_to(&target, &v.clone(), 0.0);
        assert_eq!(
            gear(&n),
            cruise::signed_gear(crate::flyto::SPEED_DEFAULT),
            "the fly-to's speed"
        );
        n.handle(set(10), &mut v, 0.0);
        assert_eq!(n.fly_speed(), cruise::rung(9));
        assert_eq!(gear(&n), 10);
        n.handle(set(0), &mut v, 0.1);
        assert!(n.flight().is_some_and(|f| f.stopping));
    }

    /// A finger put down on the picture at (x, y) at `t` and held still past [`FINGER_REST`]
    /// (frames at 60 Hz): it rests. Returns the time reached.
    fn rest_finger(n: &mut Navigator, v: &mut Viewport, x: f64, y: f64, t: f64) -> f64 {
        n.handle(
            InputEvent::FingerDown {
                x,
                y,
                spot: TouchSpot::Canvas,
            },
            v,
            t,
        );
        run(n, v, t, t + FINGER_REST + 0.05, 60.0)
    }

    /// The finger, already down at `from`, slides by `by` over `secs` (a move and a frame every
    /// 1/60 s), the platform's pan recogniser sending its own `Pan` alongside as it does. Returns
    /// the time reached.
    fn slide_finger(n: &mut Navigator, v: &mut Viewport, from: (f64, f64), by: (f64, f64), t: f64, secs: f64) -> f64 {
        let steps = (secs * 60.0).round().max(1.0) as usize;
        let mut tt = t;
        for i in 1..=steps {
            tt = t + i as f64 / 60.0;
            let f = i as f64 / steps as f64;
            n.handle(
                InputEvent::FingerMove {
                    x: from.0 + by.0 * f,
                    y: from.1 + by.1 * f,
                },
                v,
                tt,
            );
            n.handle(
                InputEvent::Pan {
                    dx: by.0 / steps as f64,
                    dy: by.1 / steps as f64,
                },
                v,
                tt,
            );
            n.advance(v, tt);
        }
        tt
    }

    /// The finger lifts and the platform reports every finger off.
    fn lift_finger(n: &mut Navigator, v: &mut Viewport, t: f64) {
        n.handle(InputEvent::FingerUp { cancelled: false }, v, t);
        n.handle(InputEvent::GestureEnd, v, t);
        n.handle(InputEvent::TouchesLifted, v, t);
    }

    /// Points the view centre moved by since `before` (y up), in the current scale.
    fn moved_points(before: &Viewport, v: &Viewport) -> (f64, f64) {
        let s = v.scale.to_f64();
        (
            (v.center.re() - before.center.re()) / s,
            (v.center.im() - before.center.im()) / s,
        )
    }

    /// Rest or swipe in C (kb: decisions/controls "Touch", "Rest or swipe"): a resting finger
    /// steers like the Mac's pointer while it is down (past the pointer's idle timeout), and
    /// keeps steering when it then slides (a joystick: the picture is not dragged); a swipe pans
    /// 1:1 from where the finger went down (the first 10 points included), the pan
    /// recogniser's own events are ignored, the release flings, the gear stays.
    #[test]
    fn a_resting_finger_steers_and_a_swipe_pans_in_c() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        for _ in 0..3 {
            tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        }
        let t = run(&mut n, &mut v, 0.0, 1.0, 60.0);
        let set = n.cruise().set_speed();
        assert!(n.finger_steers(), "C");
        let t = rest_finger(&mut n, &mut v, 200.0, 0.0, t);
        assert_eq!(n.touch, Some((200.0, 0.0)), "rested: steers");
        let t = run(&mut n, &mut v, t, t + 4.0, 60.0);
        assert!(n.cruise().heading().0 > 50.0, "steering: {:?}", n.cruise().heading());
        assert_eq!(n.zoom_target(), (0.0, 0.0), "a steering finger is not the pointer");
        // It slides down fast: still steering, now downwards; a pan would have moved the centre
        // up by 200 points.
        let before = v.clone();
        let t = slide_finger(&mut n, &mut v, (200.0, 0.0), (0.0, -200.0), t, 0.2);
        assert!(
            moved_points(&before, &v).1 < 30.0,
            "not dragged: {:?}",
            moved_points(&before, &v)
        );
        assert_eq!(n.touch, Some((200.0, -200.0)));
        let t = run(&mut n, &mut v, t, t + 3.0, 60.0);
        assert!(
            n.cruise().heading().1 < -50.0,
            "steers down: {:?}",
            n.cruise().heading()
        );
        lift_finger(&mut n, &mut v, t);
        assert_eq!(n.touch, None);
        let t = run(&mut n, &mut v, t, t + 2.0, 60.0);
        assert!(
            n.cruise().heading().0.hypot(n.cruise().heading().1) < 1.0,
            "straight on: {:?}",
            n.cruise().heading()
        );
        assert_eq!(n.level(), Level::Cruise);
        // A swipe: nothing within the slop, then the picture catches up with the finger.
        let before = v.clone();
        n.handle(
            InputEvent::FingerDown {
                x: 0.0,
                y: 0.0,
                spot: TouchSpot::Canvas,
            },
            &mut v,
            t,
        );
        n.handle(InputEvent::FingerMove { x: 6.0, y: 0.0 }, &mut v, t + 0.02);
        assert_eq!(v, before, "within the slop: undecided");
        n.handle(InputEvent::FingerMove { x: 40.0, y: 0.0 }, &mut v, t + 0.05);
        let m = moved_points(&before, &v);
        assert!(
            (m.0 + 40.0).abs() < 1e-6 && m.1.abs() < 1e-6,
            "1:1 from the touch-down: {m:?}"
        );
        assert!(n.handle(InputEvent::Pan { dx: 30.0, dy: 0.0 }, &mut v, t + 0.05));
        assert_eq!(moved_points(&before, &v), m, "the recogniser's pan is ignored");
        let mut tt = t + 0.05;
        for i in 1..=20 {
            tt = t + 0.05 + i as f64 / 60.0;
            n.handle(
                InputEvent::FingerMove {
                    x: 40.0 + 4.0 * i as f64,
                    y: 0.0,
                },
                &mut v,
                tt,
            );
            n.advance(&mut v, tt);
        }
        assert_eq!(n.touch, None, "a swipe never steers");
        n.handle(InputEvent::FingerUp { cancelled: false }, &mut v, tt + 0.005);
        assert!(n.velocity().0 > 100.0, "flings: {:?}", n.velocity());
        n.handle(InputEvent::GestureEnd, &mut v, tt + 0.006);
        n.handle(InputEvent::TouchesLifted, &mut v, tt + 0.006);
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), set, "the gear stays");
    }

    /// Rest or swipe in A: a resting finger is the wish (line from the centre to the finger)
    /// while it is down; after the lift it is let go once it settled, so the wish line goes
    /// (device round). A tap (lifted before it rested) and a rest near the centre leave no
    /// wish; Z off and on brings none back. It never hands over.
    #[test]
    fn a_resting_finger_wishes_in_a() {
        let (mut n, mut v) = joystick();
        n.advance(&mut v, 0.0);
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        let t = run(&mut n, &mut v, 0.0, 1.0, 60.0);
        let t = rest_finger(&mut n, &mut v, 250.0, 30.0, t);
        let t = run(&mut n, &mut v, t, t + 0.5, 60.0);
        let w = n.autopilot().wish().expect("a wish to the right");
        assert!(w.dir.0 > 0.99, "{:?}", w.dir);
        assert!(n.aim_marker().is_some_and(|m| m.wish == Some(w.dir)), "the wish line");
        lift_finger(&mut n, &mut v, t);
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(n.level(), Level::Autopilot, "a touch never hands over in A");
        assert!(n.autopilot().wish().is_none(), "let go after the lift");
        assert!(n.aim_marker().is_some_and(|m| m.wish.is_none()), "no wish line left");
        // A tap: lifted after 0.1 s, before it rested.
        n.handle(
            InputEvent::FingerDown {
                x: 0.0,
                y: 250.0,
                spot: TouchSpot::Canvas,
            },
            &mut v,
            t,
        );
        let t = run(&mut n, &mut v, t, t + 0.1, 60.0);
        lift_finger(&mut n, &mut v, t);
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(n.autopilot().wish().is_none(), "a tap is no wish");
        // Rested near the centre: no wish.
        let t = rest_finger(&mut n, &mut v, 10.0, 0.0, t);
        lift_finger(&mut n, &mut v, t);
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(n.autopilot().wish().is_none());
        // A rest lifted before a frame found it resting still counts (rested by the lift).
        n.handle(
            InputEvent::FingerDown {
                x: 0.0,
                y: -250.0,
                spot: TouchSpot::Canvas,
            },
            &mut v,
            t,
        );
        lift_finger(&mut n, &mut v, t + 0.3);
        let mut settled = None;
        let mut tt = t + 0.3;
        for _ in 0..90 {
            tt += 1.0 / 60.0;
            n.advance(&mut v, tt);
            if let Some(w) = n.autopilot().wish() {
                settled.get_or_insert(w);
            }
        }
        let f = settled.expect("its wish settled");
        assert!(f.dir.1 < -0.99 && f.epoch > w.epoch, "{f:?}");
        assert!(n.autopilot().wish().is_none(), "and was let go");
        let t = tt;
        // Z off and on: a kept wish does not come back.
        let t = rest_finger(&mut n, &mut v, -250.0, 0.0, t);
        lift_finger(&mut n, &mut v, t);
        tap(&mut n, &mut v, NavKey::Autopilot, t);
        tap(&mut n, &mut v, NavKey::Autopilot, t);
        run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(n.autopilot().wish().is_none());
    }

    /// A swipe in A moves the picture 1:1 and the autopilot stays on (a Mac drag hands over):
    /// no wish, no line; the target lock lets go while the hand moves the view and picks up a
    /// target again from the moved view, so it does not pull the picture back.
    #[test]
    fn a_swipe_in_a_pans_and_the_autopilot_re_aims() {
        let (mut n, mut v) = locked();
        n.handle(InputEvent::Pointer(None), &mut v, 0.0);
        n.advance(&mut v, 0.0);
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        let t = run(&mut n, &mut v, 0.0, 3.0, 60.0);
        let old = n.autopilot().lock().target().expect("a lock target").center.clone();
        let p0 = screen_of(&v, &old);
        let before = v.clone();
        n.handle(
            InputEvent::FingerDown {
                x: -100.0,
                y: 0.0,
                spot: TouchSpot::Canvas,
            },
            &mut v,
            t,
        );
        n.handle(InputEvent::FingerMove { x: -60.0, y: 0.0 }, &mut v, t + 0.04);
        let m = moved_points(&before, &v);
        assert!((m.0 + 40.0).abs() < 1e-6, "panned at once: {m:?}");
        let t = slide_finger(&mut n, &mut v, (-60.0, 0.0), (200.0, 0.0), t + 0.04, 0.5);
        n.handle(InputEvent::FingerUp { cancelled: false }, &mut v, t);
        n.handle(InputEvent::GestureEnd, &mut v, t);
        n.handle(InputEvent::TouchesLifted, &mut v, t);
        assert_eq!(n.level(), Level::Autopilot, "a swipe does not hand over in A");
        assert!(n.autopilot().is_active() && !n.autopilot().is_paused());
        assert!(n.autopilot().wish().is_none(), "a swipe is no wish");
        assert!(n.aim_marker().is_some_and(|m| m.wish.is_none()), "no wish line");
        assert!(
            n.autopilot().lock().target().is_none(),
            "the lock let go while the hand moved"
        );
        let p1 = screen_of(&v, &old);
        assert!(
            p1.0 - p0.0 > 150.0,
            "the picture moved with the finger: {p0:?} → {p1:?}"
        );
        let t = run(&mut n, &mut v, t, t + 3.0, 60.0);
        assert_eq!(n.level(), Level::Autopilot);
        assert!(n.autopilot().lock().target().is_some(), "a target from the moved view");
        assert!(!n.autopilot().is_rescuing());
        let _ = t;
    }

    /// The rest-or-swipe decision: a slow drift that stays within [`FINGER_SLOP`] rests (steers
    /// from where it is then); leaving the slop before [`FINGER_REST`] swipes; a finger whose
    /// first move after the rest time leaves the slop had rested (no frame in between).
    #[test]
    fn a_slow_drift_within_the_slop_rests() {
        let cruise = || {
            let (mut n, mut v) = setup();
            n.advance(&mut v, 0.0);
            tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
            let t = run(&mut n, &mut v, 0.0, 0.5, 60.0);
            (n, v, t)
        };
        let down = |n: &mut Navigator, v: &mut Viewport, t: f64| {
            n.handle(
                InputEvent::FingerDown {
                    x: 100.0,
                    y: 0.0,
                    spot: TouchSpot::Canvas,
                },
                v,
                t,
            );
        };
        // 0.5 points a frame for 14 frames: 7 points in 0.23 s, never panned.
        let (mut n, mut v, t) = cruise();
        down(&mut n, &mut v, t);
        let mut tt = t;
        for i in 1..=14 {
            tt = t + i as f64 / 60.0;
            let at = v.clone();
            n.handle(
                InputEvent::FingerMove {
                    x: 100.0 + 0.5 * i as f64,
                    y: 0.0,
                },
                &mut v,
                tt,
            );
            assert_eq!(v, at, "no pan within the slop");
            n.advance(&mut v, tt);
        }
        assert_eq!(n.touch, None, "not decided before the rest time");
        run(&mut n, &mut v, tt, t + FINGER_REST + 0.02, 60.0);
        assert_eq!(n.touch, Some((107.0, 0.0)), "rested: steers from where it is");
        // 12 points in 0.15 s: a swipe.
        let (mut n, mut v, t) = cruise();
        down(&mut n, &mut v, t);
        let before = v.clone();
        let t = slide_finger(&mut n, &mut v, (100.0, 0.0), (12.0, 0.0), t, 0.15);
        assert_eq!(n.touch, None);
        assert!(
            moved_points(&before, &v).0 < -8.0,
            "panned: {:?}",
            moved_points(&before, &v)
        );
        run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(n.touch, None, "a swipe stays a swipe when it stops");
        // Still at 0.3 s (no frame in between), then 50 points at once: it had rested.
        let (mut n, mut v, t) = cruise();
        down(&mut n, &mut v, t);
        let at = v.clone();
        n.handle(InputEvent::FingerMove { x: 150.0, y: 0.0 }, &mut v, t + 0.3);
        assert_eq!(v, at, "not panned");
        assert_eq!(n.touch, Some((150.0, 0.0)), "steers");
    }

    /// Where a resting finger steers (A, C, a journey's flight) the platform's long press does
    /// not begin on the picture (Rust classifies the finger); in M a held finger opens the menu,
    /// and there the finger is not Rust's: the pan recogniser pans as before. On a mini-map a
    /// hold flies there, on a control or the compass nothing. A fly-to the user started is not
    /// steering: the menu.
    #[test]
    fn a_held_finger_steers_in_flight_and_opens_the_menu_in_m() {
        let (mut n, mut v) = joystick();
        n.advance(&mut v, 0.0);
        assert!(!n.finger_steers());
        assert_eq!(n.touch_hold(TouchSpot::Canvas), TouchHold::Menu);
        assert_eq!(n.touch_hold(TouchSpot::Map), TouchHold::FlyToMap);
        assert_eq!(n.touch_hold(TouchSpot::Control), TouchHold::Nothing);
        assert_eq!(n.touch_hold(TouchSpot::Compass), TouchHold::Nothing);
        // M: not Rust's finger; the recogniser's drag pans and flings.
        let down = InputEvent::FingerDown {
            x: 100.0,
            y: 0.0,
            spot: TouchSpot::Canvas,
        };
        assert!(!n.handle(down, &mut v, 0.0));
        let t = run(&mut n, &mut v, 0.0, 0.5, 60.0);
        assert!(!n.handle(InputEvent::FingerMove { x: 140.0, y: 0.0 }, &mut v, t));
        assert_eq!(n.touch, None, "no steering in M");
        let before = v.clone();
        for i in 1..=10 {
            n.handle(InputEvent::Pan { dx: 5.0, dy: 0.0 }, &mut v, t + i as f64 / 60.0);
        }
        assert!((moved_points(&before, &v).0 + 50.0).abs() < 1e-6, "the drag pans");
        n.handle(InputEvent::FingerUp { cancelled: false }, &mut v, t + 0.17);
        n.handle(InputEvent::GestureEnd, &mut v, t + 0.17);
        assert!(n.velocity().0 > 100.0, "and flings: {:?}", n.velocity());
        n.handle(InputEvent::TouchesLifted, &mut v, t + 0.17);
        tap(&mut n, &mut v, NavKey::Autopilot, 1.0);
        run(&mut n, &mut v, 1.0, 1.5, 60.0);
        assert!(n.finger_steers(), "A");
        assert_eq!(n.touch_hold(TouchSpot::Canvas), TouchHold::Nothing, "A: Rust's finger");
        assert_eq!(n.touch_hold(TouchSpot::Map), TouchHold::FlyToMap);
        assert_eq!(n.touch_hold(TouchSpot::Control), TouchHold::Nothing);
        tap(&mut n, &mut v, NavKey::Autopilot, 1.5);
        assert!(!n.finger_steers(), "A off");
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        assert_eq!(n.level(), Level::Cruise);
        assert!(n.finger_steers(), "C");
        assert_eq!(n.touch_hold(TouchSpot::Canvas), TouchHold::Nothing, "C: no menu");
        assert_eq!(n.touch_hold(TouchSpot::Map), TouchHold::FlyToMap, "C");
        // C: a finger on the compass is never Rust's (its drag turns the view); on a mini-map
        // it is (a swipe pans) but a rest steers nothing (the platform's hold flies there).
        let t = run(&mut n, &mut v, 0.0, 0.5, 60.0);
        let compass = InputEvent::FingerDown {
            x: 400.0,
            y: 200.0,
            spot: TouchSpot::Compass,
        };
        assert!(!n.handle(compass, &mut v, t));
        assert!(n.finger.is_none());
        n.handle(InputEvent::TouchesLifted, &mut v, t);
        let map = InputEvent::FingerDown {
            x: 400.0,
            y: -200.0,
            spot: TouchSpot::Map,
        };
        assert!(n.handle(map, &mut v, t));
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(n.touch, None, "a rest on a mini-map does not steer");
        lift_finger(&mut n, &mut v, t);
        // A fly-to the user started (Fly Here): not steering, the menu, not Rust's finger.
        let (mut n, mut v) = setup();
        n.handle(InputEvent::FlyHere { x: 100.0, y: 0.0 }, &mut v, 0.0);
        assert!(n.flight().is_some());
        assert!(!n.finger_steers(), "fly-to");
        assert_eq!(n.touch_hold(TouchSpot::Canvas), TouchHold::Menu, "fly-to");
        assert!(!n.handle(down, &mut v, 0.0));
    }

    /// A journey's flight: a resting finger inside the steering dead zone lets the leg fly on;
    /// off it, it takes the wheel at once (the cruise at the journey's speed, the journey
    /// suspended) and the cruise steers towards it. A swipe takes the wheel and pans.
    #[test]
    fn a_resting_finger_takes_the_wheel_from_a_journey_flight_and_steers() {
        let flying = || {
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
            while t < 30.0 && !n.journey_flying() {
                t += 1.0 / 60.0;
                n.advance(&mut v, t);
            }
            assert!(n.journey_flying(), "a leg flies");
            assert!(n.finger_steers());
            (n, v, t)
        };
        let (mut n, mut v, t) = flying();
        let t = rest_finger(&mut n, &mut v, 20.0, 0.0, t);
        assert!(n.journey_flying(), "inside the dead zone the leg flies on");
        lift_finger(&mut n, &mut v, t);
        let t = rest_finger(&mut n, &mut v, 250.0, 0.0, t);
        assert_eq!(n.level(), Level::Cruise, "off the dead zone it took the wheel");
        assert!(n.journey().is_suspended());
        let t = run(&mut n, &mut v, t, t + 2.0, 60.0);
        assert!(
            n.cruise().heading().0 > 30.0,
            "the cruise steers: {:?}",
            n.cruise().heading()
        );
        lift_finger(&mut n, &mut v, t);
        // A swipe.
        let (mut n, mut v, t) = flying();
        let before = v.clone();
        n.handle(
            InputEvent::FingerDown {
                x: 0.0,
                y: 0.0,
                spot: TouchSpot::Canvas,
            },
            &mut v,
            t,
        );
        n.handle(InputEvent::FingerMove { x: 40.0, y: 0.0 }, &mut v, t + 0.05);
        assert_eq!(n.level(), Level::Cruise, "the swipe took the wheel");
        assert!(n.journey().is_suspended());
        assert!((moved_points(&before, &v).0 + 40.0).abs() < 1.0, "1:1 under the finger");
        assert_eq!(n.touch, None);
    }

    /// A second finger ends the rest-or-swipe finger (the platform reports it cancelled):
    /// pending, it never rests or swipes; steering, it stops; after a pinch the finger left down
    /// does nothing, its moves and the pan recogniser's are ignored, until every finger is off;
    /// then a fresh finger works.
    #[test]
    fn a_second_finger_ends_the_resting_or_swiping_finger() {
        let pinch = |n: &mut Navigator, v: &mut Viewport, t: f64| {
            n.handle(
                InputEvent::Pinch {
                    ratio: 1.02,
                    x: 30.0,
                    y: 10.0,
                    dx: 0.0,
                    dy: 0.0,
                },
                v,
                t,
            );
        };
        let (mut n, mut v) = joystick();
        n.advance(&mut v, 0.0);
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        let t = run(&mut n, &mut v, 0.0, 0.5, 60.0);
        n.handle(
            InputEvent::FingerDown {
                x: 250.0,
                y: 0.0,
                spot: TouchSpot::Canvas,
            },
            &mut v,
            t,
        );
        n.handle(InputEvent::FingerUp { cancelled: true }, &mut v, t + 0.1);
        for i in 0..10 {
            pinch(&mut n, &mut v, t + 0.1 + i as f64 / 120.0);
        }
        let t = run(&mut n, &mut v, t + 0.2, t + 1.0, 60.0);
        assert_eq!(n.touch, None, "never rested");
        assert!(n.autopilot().wish().is_none());
        // One finger lifted, the other moves on: ignored, the recogniser's pan too.
        let at = v.center.clone();
        assert!(!n.handle(InputEvent::FingerMove { x: 300.0, y: 0.0 }, &mut v, t));
        assert!(!n.handle(InputEvent::Pan { dx: 50.0, dy: 0.0 }, &mut v, t));
        assert_eq!(v.center, at);
        n.handle(InputEvent::TouchesLifted, &mut v, t);
        let t = rest_finger(&mut n, &mut v, 250.0, 0.0, t);
        let t = run(&mut n, &mut v, t, t + 0.5, 60.0);
        assert!(n.autopilot().wish().is_some(), "a fresh finger wishes");
        // A steering finger, then a second one: the steering stops, no wish is left.
        n.handle(InputEvent::FingerUp { cancelled: true }, &mut v, t);
        pinch(&mut n, &mut v, t);
        assert_eq!(n.touch, None);
        assert!(n.touch_wish.is_none(), "no wish left behind");
        n.handle(InputEvent::TouchesLifted, &mut v, t);
        // A pinch arriving before the platform's cancel: the finger is done too.
        let t = rest_finger(&mut n, &mut v, 250.0, 0.0, t);
        pinch(&mut n, &mut v, t);
        assert_eq!(n.touch, None);
        assert!(!n.handle(InputEvent::FingerMove { x: 260.0, y: 0.0 }, &mut v, t));
        assert_eq!(n.touch, None);
    }

    /// A journey's flight: a drag (the Mac's; touch: `a_resting_finger_takes_the_wheel_...`)
    /// pans: it takes the wheel at once (the cruise goes on at the journey's speed) and suspends
    /// the journey; the next drag pans the cruise the same way (one meaning).
    #[test]
    fn a_drag_takes_the_wheel_from_a_journey_flight_and_pans() {
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
        while t < 30.0 && !n.journey_flying() {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        assert!(n.journey_flying(), "a leg flies");
        assert_eq!(n.touch_hold(TouchSpot::Canvas), TouchHold::Nothing);
        let before = v.clone();
        n.handle(InputEvent::Pan { dx: 40.0, dy: 0.0 }, &mut v, t);
        assert_eq!(n.level(), Level::Cruise, "the drag took the wheel");
        assert!(n.journey().is_suspended());
        let moved = (v.center.re() - before.center.re()) / v.scale.to_f64();
        assert!((moved + 40.0).abs() < 1.0, "1:1 under the finger: {moved}");
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(n.level(), Level::Cruise);
        let before = v.clone();
        n.handle(InputEvent::Pan { dx: 0.0, dy: -30.0 }, &mut v, t);
        let moved = (v.center.im() - before.center.im()) / v.scale.to_f64();
        assert!((moved - 30.0).abs() < 1.0, "pans the cruise: {moved}");
        assert_eq!(n.level(), Level::Cruise);
    }

    /// The cruise and a drag (the pan recogniser's, as on the Mac; device round: "panning
    /// doesn't work, you can't steer"): the drag moves the picture 1:1 while the cruise zooms on
    /// at its gear; the release flings.
    #[test]
    fn a_drag_pans_the_cruise_and_it_zooms_on() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        for _ in 0..3 {
            tap(&mut n, &mut v, NavKey::FasterZoom, 0.0);
        }
        let t = run(&mut n, &mut v, 0.0, 1.0, 60.0);
        let set = n.cruise().set_speed();
        let (re0, s0) = (v.center.re(), v.scale.to_f64());
        let mut t = t;
        for _ in 0..30 {
            t += 1.0 / 60.0;
            n.handle(InputEvent::Pan { dx: 4.0, dy: 0.0 }, &mut v, t);
            n.advance(&mut v, t);
        }
        // 120 points to the right in 0.5 s: the content moved with the finger.
        assert!(v.center.re() < re0 - 60.0 * v.scale.to_f64(), "panned");
        assert!(v.scale.to_f64() < s0 * 0.95, "zoomed on while panning");
        assert_eq!(n.level(), Level::Cruise);
        assert_eq!(n.cruise().set_speed(), set, "the gear stays");
        n.handle(InputEvent::GestureEnd, &mut v, t + 0.005);
        assert!(n.velocity().0 > 100.0, "flings: {:?}", n.velocity());
    }

    /// A pinch whose fingers lift one after the other (device round: "after the zoom there's
    /// also some panning"): the finger left down neither pans nor steers nor flings, whichever
    /// lifted first (the platform sends the same events), until every finger is off; then a
    /// fresh touch pans. A pinch's release after holding still does not fling.
    #[test]
    fn a_finger_left_after_a_pinch_does_not_pan() {
        let (mut n, mut v) = setup();
        n.advance(&mut v, 0.0);
        let mut t = 0.0;
        let pinch = |n: &mut Navigator, v: &mut Viewport, t: f64| {
            n.handle(
                InputEvent::Pinch {
                    ratio: 1.05,
                    x: 30.0,
                    y: 10.0,
                    dx: 2.0,
                    dy: 1.0,
                },
                v,
                t,
            );
        };
        for _ in 0..20 {
            t += 1.0 / 120.0;
            pinch(&mut n, &mut v, t);
        }
        // Held still, then one finger lifts (the platform ends the pinch there).
        t += 0.2;
        n.handle(InputEvent::GestureEnd, &mut v, t);
        assert_eq!(n.velocity(), (0.0, 0.0, 0.0), "no fling from a stale pinch");
        assert_eq!(n.touch_hold(TouchSpot::Canvas), TouchHold::Nothing, "no menu for it");
        let at = v.clone();
        // The other finger moves on and lifts while moving.
        for _ in 0..20 {
            t += 1.0 / 120.0;
            assert!(!n.handle(InputEvent::Pan { dx: 6.0, dy: -3.0 }, &mut v, t));
        }
        n.handle(InputEvent::GestureEnd, &mut v, t + 0.005);
        n.handle(InputEvent::TouchesLifted, &mut v, t + 0.005);
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(v, at, "the remaining finger did not pan or fling");
        assert_eq!(n.velocity(), (0.0, 0.0, 0.0));
        // A fresh touch pans.
        assert!(n.handle(InputEvent::Pan { dx: 6.0, dy: 0.0 }, &mut v, t + 0.1));
        assert_ne!(v, at);
        assert_eq!(n.touch_hold(TouchSpot::Canvas), TouchHold::Menu);

        // In A: a finger left after a pinch sets no wish; a twist counts as a pinch too.
        let (mut n, mut v) = joystick();
        n.advance(&mut v, 0.0);
        tap(&mut n, &mut v, NavKey::Autopilot, 0.0);
        let t = run(&mut n, &mut v, 0.0, 0.5, 60.0);
        n.handle(
            InputEvent::TouchRotate {
                radians: 0.01,
                x: 0.0,
                y: 0.0,
            },
            &mut v,
            t,
        );
        n.handle(InputEvent::TouchRotateEnd, &mut v, t);
        assert!(!n.handle(InputEvent::FingerMove { x: 250.0, y: 0.0 }, &mut v, t));
        let t = run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(n.autopilot().wish().is_none(), "no wish from the remaining finger");
        n.handle(InputEvent::FingerUp { cancelled: false }, &mut v, t);
        n.handle(InputEvent::TouchesLifted, &mut v, t);
        let t = rest_finger(&mut n, &mut v, 250.0, 0.0, t);
        run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(n.autopilot().wish().is_some(), "a fresh finger wishes");
        // Lost focus (the app left the screen with fingers down) clears it too.
        let (mut n, mut v) = setup();
        pinch(&mut n, &mut v, 0.0);
        n.handle(InputEvent::FocusLost, &mut v, 0.1);
        assert!(n.handle(InputEvent::Pan { dx: 6.0, dy: 0.0 }, &mut v, 0.2));
    }

    /// The orbit probe a touch placed replaces the zoom target for the orbit only.
    #[test]
    fn a_touch_probe_moves_the_orbit_point_only() {
        let (mut n, mut v) = setup();
        assert_eq!(n.probe_point(), (0.0, 0.0));
        n.handle(InputEvent::Probe(Some((120.0, -40.0))), &mut v, 0.0);
        assert_eq!(n.probe_point(), (120.0, -40.0));
        assert_eq!(n.zoom_target(), (0.0, 0.0));
        n.handle(InputEvent::Probe(None), &mut v, 0.0);
        assert_eq!(n.probe_point(), (0.0, 0.0));
        assert!(!InputEvent::Probe(Some((f64::NAN, 0.0))).is_finite());
    }

    /// Long-press "Fly here": the fly-to path to the point, centred and FLY_HERE_ZOOM× deeper,
    /// landing exactly. "Seek minibrot here": the search about the point, not the centre.
    #[test]
    fn fly_here_and_seek_here() {
        let (mut n, mut v) = setup();
        let target = v.point_at_offset(150.0, -60.0);
        let scale = v.scale / FLY_HERE_ZOOM;
        assert!(n.handle(InputEvent::FlyHere { x: 150.0, y: -60.0 }, &mut v, 0.0));
        assert_eq!(n.level(), Level::Journey, "a fly-to");
        let mut t = 0.0;
        while n.flight().is_some() && t < 30.0 {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        assert!(n.flight().is_none(), "landed");
        assert_eq!(v.scale, scale);
        assert!(v.center.distance_in(&target, v.scale) < 1e-6);
        // Seek about a point far outside the set finds nothing; at the centre (seahorse valley,
        // span 1e-6: the period-117 minibrot) it does.
        let mut v = Viewport::new(-0.743643887037151, 0.13182590420533, 1e-6 / 600.0);
        let mut n = Navigator::default();
        n.handle(
            InputEvent::Resize {
                width: 1000.0,
                height: 600.0,
            },
            &mut v,
            0.0,
        );
        n.seek_mut().synchronous = true;
        assert!(n.handle(InputEvent::SeekAt { x: 2.5e9, y: 0.0 }, &mut v, 0.0));
        assert_eq!(n.seek_status(0.0).0, SeekPhase::NotFound);
        assert!(n.flight().is_none());
        assert!(n.handle(InputEvent::SeekAt { x: 0.0, y: 0.0 }, &mut v, 5.0));
        assert_eq!(n.seek_status(5.0).0, SeekPhase::Flying);
    }

    /// A touch twist turns the view only past the 15° dead band, then on from there without a
    /// jump; each gesture starts at the dead band again. The Mac's `Rotate` stays 1:1.
    #[test]
    fn a_touch_twist_turns_only_past_the_dead_band() {
        let (mut n, mut v) = setup();
        let deg = std::f64::consts::PI / 180.0;
        let twist = |n: &mut Navigator, v: &mut Viewport, d: f64| {
            n.handle(
                InputEvent::TouchRotate {
                    radians: d * deg,
                    x: 0.0,
                    y: 0.0,
                },
                v,
                0.0,
            )
        };
        // A pinch's incidental twist, back and forth: nothing.
        for d in [4.0, 5.0, -3.0, 6.0, -8.0, 9.0] {
            assert!(twist(&mut n, &mut v, d));
            assert_eq!(v.angle, 0.0, "turned below the dead band");
        }
        // 13° so far; 5° more crosses 15° and turns by the 3° past it (the image turns
        // clockwise for counter-clockwise fingers, as `Rotate`).
        twist(&mut n, &mut v, 5.0);
        assert!((v.angle.abs() - 3.0 * deg).abs() < 1e-9, "angle {}", v.angle / deg);
        // From there 1:1.
        twist(&mut n, &mut v, 10.0);
        assert!((v.angle.abs() - 13.0 * deg).abs() < 1e-9);
        // A new gesture starts at the dead band again.
        n.handle(InputEvent::TouchRotateEnd, &mut v, 0.0);
        let a = v.angle;
        twist(&mut n, &mut v, -14.0);
        assert_eq!(v.angle, a);
        // The trackpad's rotation is not damped.
        n.handle(
            InputEvent::Rotate {
                radians: 2.0 * deg,
                x: 0.0,
                y: 0.0,
            },
            &mut v,
            0.0,
        );
        assert!(((v.angle - a).abs() - 2.0 * deg).abs() < 1e-9);
    }

    /// The iPad simulator's flaky "small twist" UI test (kb: decisions/controls "Touch", QA pass
    /// "twist dead band"): XCTest's `rotate(0.15)` is an 18.4° finger jump, and UIKit hands it
    /// over as ONE `.ended` rotation of 0.21472 rad (12.3°) or 0.42879 rad (24.6°) from the same
    /// synthesised events. The dead band is not bypassed in either case: a single event that
    /// crosses it turns by the excess only (24.6° − 15° = the 9.57° the test saw), one below it
    /// turns nothing, and a fresh navigator starts every twist at the band.
    #[test]
    fn a_single_twist_event_crossing_the_dead_band_turns_by_the_excess() {
        for (radians, turned) in [
            (0.21472, 0.0),
            (0.42879, 0.42879 - TWIST_DEAD_BAND),
            (-0.42879, 0.42879 - TWIST_DEAD_BAND),
        ] {
            let (mut n, mut v) = setup();
            // As FractalView sends it: the `.ended` rotation (clockwise-positive, negated), then the end.
            assert!(n.handle(
                InputEvent::TouchRotate {
                    radians: -radians,
                    x: 0.0,
                    y: 0.0
                },
                &mut v,
                0.0
            ));
            n.handle(InputEvent::TouchRotateEnd, &mut v, 0.0);
            assert!(
                (v.angle.abs() - turned).abs() < 1e-12,
                "{radians}: angle {}",
                v.angle.to_degrees()
            );
            // The end re-arms the band: the same twist again turns by the same excess only.
            let a = v.angle.abs();
            n.handle(
                InputEvent::TouchRotate {
                    radians: -radians,
                    x: 0.0,
                    y: 0.0,
                },
                &mut v,
                0.0,
            );
            assert!((v.angle.abs() - a - turned).abs() < 1e-12);
        }
    }

    /// A search started from rest (the long-press "Seek Minibrot Here") asks for frames, so the
    /// platform polls it: nothing else moves while it searches (QA 2026-10-18: it stayed
    /// "searching" until the next touch).
    #[test]
    fn a_search_from_rest_wants_frames() {
        let (mut n, mut v) = setup();
        assert!(!n.wants_frames());
        assert!(n.handle(InputEvent::SeekAt { x: 40.0, y: 10.0 }, &mut v, 0.0));
        assert_eq!(n.seek_status(0.0).0, SeekPhase::Searching);
        assert!(!n.is_animating(), "a search is not motion");
        assert!(n.wants_frames(), "but it needs frames to be polled");
    }

    // Held input whose release never arrives (kb: decisions/controls "Held input"): a key-up
    // the inactive app never gets, a mouse-up after the window lost key, a touch cancelled
    // with the app going to the background. `FocusLost` lets go of everything as a release
    // would, and the view settles.

    /// Each held key (W A S D, Space, the turn keys): after `FocusLost` the view goes exactly
    /// where a key-up at that moment takes it, without a fling, and settles.
    #[test]
    fn focus_lost_releases_each_held_key_and_settles() {
        let keys = [
            NavKey::Up,
            NavKey::Down,
            NavKey::Left,
            NavKey::Right,
            NavKey::Zoom,
            NavKey::RotateCcw,
            NavKey::RotateCw,
        ];
        for k in keys {
            let (mut n, mut v) = setup();
            let (mut twin, mut tv) = setup();
            for (m, vv) in [(&mut n, &mut v), (&mut twin, &mut tv)] {
                m.handle(InputEvent::KeyDown(k), vv, 0.0);
                run(m, vv, 0.0, 0.5, 60.0);
            }
            assert!(n.is_moving(), "{k:?} moves while held");
            n.handle(InputEvent::FocusLost, &mut v, 0.5);
            twin.handle(InputEvent::KeyUp(k), &mut tv, 0.5);
            assert_eq!(n.fling, (0.0, 0.0), "{k:?}: no fling");
            assert_eq!(n.held_count(), 0);
            run(&mut n, &mut v, 0.5, 4.0, 60.0);
            run(&mut twin, &mut tv, 0.5, 4.0, 60.0);
            assert_eq!(v, tv, "{k:?}: FocusLost acts as the key-up");
            assert!(!n.is_moving() && !n.is_animating(), "{k:?}: settled");
        }
    }

    /// A held pedal (E from rest starts a cruise and climbs a rung every 0.15 s): after
    /// `FocusLost` it climbs no further and the cruise keeps the gear it had, as after a key-up;
    /// Esc then stops it and the view settles. The same for Q held in a cruise.
    #[test]
    fn focus_lost_releases_a_held_pedal() {
        for (k, set0) in [(NavKey::FasterZoom, None), (NavKey::SlowerZoom, Some(10))] {
            let (mut n, mut v) = setup();
            let (mut twin, mut tv) = setup();
            for (m, vv) in [(&mut n, &mut v), (&mut twin, &mut tv)] {
                if let Some(g) = set0 {
                    m.handle(InputEvent::SetGear { gear: g }, vv, 0.0);
                }
                m.handle(InputEvent::KeyDown(k), vv, 0.0);
                run(m, vv, 0.0, 1.0, 60.0);
            }
            n.handle(InputEvent::FocusLost, &mut v, 1.0);
            twin.handle(InputEvent::KeyUp(k), &mut tv, 1.0);
            assert!(n.pedal_hold.is_none(), "{k:?}");
            let set = n.cruise().set_speed();
            assert_eq!(n.level(), Level::Cruise, "{k:?}");
            run(&mut n, &mut v, 1.0, 3.0, 60.0);
            run(&mut twin, &mut tv, 1.0, 3.0, 60.0);
            assert_eq!(n.cruise().set_speed(), set, "{k:?}: the gear stays");
            assert_eq!(v, tv, "{k:?}: FocusLost acts as the key-up");
            n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, 3.0);
            n.handle(InputEvent::KeyUp(NavKey::Stop), &mut v, 3.0);
            run(&mut n, &mut v, 3.0, 8.0, 60.0);
            assert!(!n.is_moving() && !n.is_animating(), "{k:?}: settled after Esc");
        }
    }

    /// A mouse drag whose mouse-up never came: no fling at `FocusLost`, none either from a
    /// late `GestureEnd`, and the view stays where the drag left it.
    #[test]
    fn focus_lost_ends_a_drag_without_a_fling() {
        let (mut n, mut v) = setup();
        let mut t = 0.0;
        for _ in 0..12 {
            t += 1.0 / 60.0;
            n.handle(InputEvent::Pan { dx: 20.0, dy: 0.0 }, &mut v, t);
            n.advance(&mut v, t);
        }
        n.handle(InputEvent::FocusLost, &mut v, t);
        n.handle(InputEvent::GestureEnd, &mut v, t + 0.001);
        assert_eq!(n.fling, (0.0, 0.0));
        let before = v.clone();
        run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert_eq!(v, before);
        assert!(!n.is_moving());
    }

    /// A deflected gamepad stick and trigger: `FocusLost` centres them (the motion eases out).
    #[test]
    fn focus_lost_centres_the_gamepad() {
        let (mut n, mut v) = setup();
        let pad = InputEvent::Gamepad {
            lx: 0.9,
            ly: 0.0,
            rx: 0.0,
            ry: 0.0,
            lt: 0.0,
            rt: 0.8,
            boost: false,
        };
        n.handle(pad, &mut v, 0.0);
        run(&mut n, &mut v, 0.0, 0.5, 60.0);
        assert!(n.is_moving());
        n.handle(InputEvent::FocusLost, &mut v, 0.5);
        run(&mut n, &mut v, 0.5, 4.0, 60.0);
        assert!(!n.is_moving() && !n.is_animating());
    }

    /// A steering finger in the cruise and a touch throttle drag that braked to zero, cancelled
    /// by the app leaving the screen: the finger steers no more, the throttle's rest is gone,
    /// the cruise keeps its gear; Esc and the view settles.
    #[test]
    fn focus_lost_lets_go_of_a_finger() {
        let (mut n, mut v) = setup();
        n.handle(InputEvent::SetGear { gear: 5 }, &mut v, 0.0);
        let t = rest_finger(&mut n, &mut v, 200.0, 0.0, 0.0);
        assert!(n.touch.is_some() && n.finger.is_some(), "the finger steers");
        let set = n.cruise().set_speed();
        n.handle(InputEvent::FocusLost, &mut v, t);
        assert!(n.touch.is_none() && n.finger.is_none() && n.throttle_rest.is_none());
        assert_eq!(n.cruise().set_speed(), set);
        // The touch's later cancellation changes nothing.
        n.handle(InputEvent::FingerUp { cancelled: true }, &mut v, t + 0.01);
        n.handle(InputEvent::TouchesLifted, &mut v, t + 0.01);
        n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t + 0.1);
        run(&mut n, &mut v, t + 0.1, t + 6.0, 60.0);
        assert!(!n.is_moving() && !n.is_animating(), "settled");
    }

    /// A jump (pasted coordinates, a link, a place) with E or W still held: nothing moves on,
    /// no cruise starts from the held pedal (it stepped again after 0.4 s before), and the
    /// key-ups arriving later change nothing.
    #[test]
    fn a_jump_lets_go_of_held_keys_and_pedals() {
        for k in [NavKey::FasterZoom, NavKey::Up, NavKey::Zoom] {
            let (mut n, mut v) = setup();
            n.handle(InputEvent::KeyDown(k), &mut v, 0.0);
            run(&mut n, &mut v, 0.0, 0.2, 60.0);
            n.jumped();
            let jumped_to = v.clone();
            assert!(!n.is_moving(), "{k:?}");
            run(&mut n, &mut v, 0.2, 2.0, 60.0);
            assert_eq!(n.level(), Level::Manual, "{k:?}");
            assert!(!n.is_moving() && v == jumped_to, "{k:?}: still");
            n.handle(InputEvent::KeyUp(k), &mut v, 2.0);
            assert!(!n.is_moving(), "{k:?}");
        }
    }

    // MARK: View limits (kb: decisions/controls "View limits")

    use crate::viewport::{CENTER_LIMIT, Center};

    /// The home view (1000 x 600) and its navigator.
    fn at_home() -> (Navigator, Viewport) {
        let (n, _) = setup();
        (n, Viewport::fitting(W, 600.0))
    }

    /// Zoom (e-folds left to the limit) of a view.
    fn to_limit(n: &Navigator, v: &Viewport) -> f64 {
        n.limits().max_scale.ln() - v.scale.ln()
    }

    /// Every zoom-out path stops at the limit, eased in (the per-step zoom never grows
    /// near the limit, nothing passes it), and zooming in works at once from there.
    #[test]
    fn every_zoom_out_stops_at_the_limit() {
        type Drive = fn(&mut Navigator, &mut Viewport, f64) -> f64;
        let drives: [(&str, Drive); 7] = [
            ("wheel zoom", |n, v, t| {
                for i in 0..200 {
                    n.handle(
                        InputEvent::WheelZoom {
                            delta: -1.0,
                            x: 120.0,
                            y: -40.0,
                        },
                        v,
                        t + i as f64 / 120.0,
                    );
                }
                t + 200.0 / 120.0
            }),
            ("magnify", |n, v, t| {
                for i in 0..200 {
                    n.handle(
                        InputEvent::Magnify {
                            magnification: -0.1,
                            x: 0.0,
                            y: 0.0,
                        },
                        v,
                        t + i as f64 / 120.0,
                    );
                }
                n.handle(InputEvent::GestureEnd, v, t + 200.0 / 120.0);
                run(n, v, t + 200.0 / 120.0, t + 4.0, 60.0)
            }),
            ("pinch", |n, v, t| {
                for i in 0..200 {
                    let e = InputEvent::Pinch {
                        ratio: 0.9,
                        x: -50.0,
                        y: 30.0,
                        dx: 0.0,
                        dy: 0.0,
                    };
                    n.handle(e, v, t + i as f64 / 120.0);
                }
                n.handle(InputEvent::GestureEnd, v, t + 200.0 / 120.0);
                run(n, v, t + 200.0 / 120.0, t + 4.0, 60.0)
            }),
            ("wheel notches", |n, v, t| {
                for i in 0..30 {
                    n.handle(
                        InputEvent::WheelNotch {
                            delta: -1.0,
                            x: 0.0,
                            y: 0.0,
                        },
                        v,
                        t + i as f64 * 0.05,
                    );
                    run(n, v, t + i as f64 * 0.05, t + (i + 1) as f64 * 0.05, 120.0);
                }
                run(n, v, t + 1.5, t + 3.0, 60.0)
            }),
            ("step-out key", |n, v, t| {
                for i in 0..10 {
                    tap(n, v, NavKey::StepOut, t + i as f64 * 0.1);
                }
                run(n, v, t + 1.0, t + 3.0, 60.0)
            }),
            ("double tap out", |n, v, t| {
                for i in 0..6 {
                    double_tap(n, v, 200.0, 100.0, true, t + i as f64 * 0.4);
                    run(n, v, t + i as f64 * 0.4, t + (i + 1) as f64 * 0.4, 60.0);
                }
                run(n, v, t + 2.4, t + 3.0, 60.0)
            }),
            ("held ⇧ Space", |n, v, t| {
                n.handle(InputEvent::Shift(true), v, t);
                n.handle(InputEvent::KeyDown(NavKey::Zoom), v, t);
                let mut last = f64::INFINITY;
                let mut s = t;
                for _ in 0..600 {
                    s += 1.0 / 120.0;
                    n.advance(v, s);
                    let left = to_limit(n, v);
                    // Eased in: frame steps only shrink once the softness band is reached.
                    assert!(left >= 0.0 && left <= last + 1e-12, "{left} after {last}");
                    last = left;
                }
                n.handle(InputEvent::KeyUp(NavKey::Zoom), v, s);
                run(n, v, s, s + 2.0, 60.0)
            }),
        ];
        for (name, drive) in drives {
            let (mut n, mut v) = at_home();
            let t = drive(&mut n, &mut v, 0.0);
            let max = n.limits().max_scale;
            assert!(v.scale <= max, "{name}: {} past {}", v.scale, max);
            let left = to_limit(&n, &v);
            assert!((0.0..0.02).contains(&left), "{name}: {left} e-folds from the limit");
            assert!(!n.limits().outside(&v.center), "{name}: centre out");
            // Zooming in again works at once and exactly.
            let s = v.scale;
            n.handle(
                InputEvent::WheelZoom {
                    delta: 1.0,
                    x: 0.0,
                    y: 0.0,
                },
                &mut v,
                t + 1.0,
            );
            assert!(
                (s.ratio(v.scale) - 1.15).abs() < 1e-9,
                "{name}: zoom in ×{}",
                s.ratio(v.scale)
            );
        }
    }

    /// The soft stop is the same at any frame rate and any step split.
    #[test]
    fn the_zoom_limit_is_frame_rate_independent() {
        let l = ViewLimits::new(crate::formula::Formula::Mandelbrot, W, 600.0);
        let start = l.max_scale / 1.5;
        let mut one = start;
        one = one / l.zoom_factor(one, 0.25);
        let mut many = start;
        for _ in 0..64 {
            many = many / l.zoom_factor(many, 0.25f64.powf(1.0 / 64.0));
        }
        assert!((one.ratio(many) - 1.0).abs() < 1e-12, "{}", one.ratio(many));
        assert!(one < l.max_scale && one.ratio(l.max_scale) > 0.9);
        // Outside the softness band a zoom out is exactly the plain one.
        let far = l.max_scale / 10.0;
        assert_eq!(l.zoom_factor(far, 0.5), 0.5);
        // At or past the limit: nothing; zooming in: untouched.
        assert_eq!(l.zoom_factor(l.max_scale, 0.5), 1.0);
        assert_eq!(l.zoom_factor(l.max_scale * 2.0, 0.5), 1.0);
        assert_eq!(l.zoom_factor(l.max_scale, 2.0), 2.0);
    }

    /// A drag past the centre bound is resisted (never more than the band past it) and
    /// springs back onto the bound once released; a fling into it the same.
    #[test]
    fn a_drag_past_the_bound_is_resisted_and_springs_back() {
        let (mut n, mut v) = at_home();
        // At the zoom limit, dragging the content right (the centre left) by 20 views.
        v.scale = n.limits().max_scale;
        let lo = n.limits().center.0 - CENTER_LIMIT;
        let band = RUBBER_BAND * 600.0;
        let mut t = 0.0;
        let mut worst: f64 = 0.0;
        for _ in 0..2000 {
            t += 1.0 / 120.0;
            n.handle(InputEvent::Pan { dx: 10.0, dy: 0.0 }, &mut v, t);
            worst = worst.max((lo - v.center.re()) / v.scale.to_f64());
        }
        assert!(worst > 0.5 * band && worst < band, "{worst} points past (band {band})");
        assert!(n.is_stretched());
        // Held (no GestureEnd yet, events still arriving): no spring back within the hold.
        let held = v.center.re();
        n.advance(&mut v, t + 0.05);
        assert_eq!(v.center.re(), held);
        // Released after holding still (no fling): back onto the bound, smoothly (monotonic),
        // within a second.
        n.handle(InputEvent::GestureEnd, &mut v, t + 0.08);
        let mut last = v.center.re();
        let mut s = t + 0.08;
        while n.is_moving() && s < t + 3.0 {
            s += 1.0 / 60.0;
            n.advance(&mut v, s);
            assert!(v.center.re() >= last, "monotonic");
            last = v.center.re();
        }
        assert!(s < t + 1.1, "settled after {} s", s - t);
        assert!(!n.is_stretched() && !n.limits().outside(&v.center));
        assert!((v.center.re() - lo).abs() < 1e-9, "on the bound: {}", v.center.re());
        // Moving back in from past the bound is not resisted.
        let mut w = v.clone();
        w.center = w.center.offset(-0.5, 0.0);
        n.rubber = true;
        let before = w.center.re();
        n.handle(InputEvent::Pan { dx: -10.0, dy: 0.0 }, &mut w, s + 1.0);
        assert!(((w.center.re() - before) / w.scale.to_f64() - 10.0).abs() < 1e-6);

        // A fast fling into the bound stops there and comes back.
        let (mut n, mut v) = at_home();
        v.scale = n.limits().max_scale;
        let mut t = 0.0;
        for _ in 0..10 {
            t += 1.0 / 120.0;
            n.handle(InputEvent::Pan { dx: 0.0, dy: -40.0 }, &mut v, t);
        }
        n.handle(InputEvent::GestureEnd, &mut v, t + 0.005);
        let mut s = t;
        let hi = n.limits().center.1 + CENTER_LIMIT;
        let mut worst: f64 = 0.0;
        while n.is_moving() && s < t + 5.0 {
            s += 1.0 / 120.0;
            n.advance(&mut v, s);
            worst = worst.max((v.center.im() - hi) / v.scale.to_f64());
        }
        assert!(worst < band, "{worst}");
        assert!(!n.limits().outside(&v.center) && !n.is_stretched());
    }

    /// Held keys push against the bound softly (a few points past it, not the band) and the
    /// view returns when they let go; zooming about a point far off does not drag the centre
    /// out either.
    #[test]
    fn keys_and_zooms_keep_to_the_bound() {
        let (mut n, mut v) = at_home();
        v.scale = n.limits().max_scale;
        let hi = n.limits().center.0 + CENTER_LIMIT;
        n.handle(InputEvent::KeyDown(NavKey::Right), &mut v, 0.0);
        let mut worst: f64 = 0.0;
        let mut t = 0.0;
        for _ in 0..1200 {
            t += 1.0 / 120.0;
            n.advance(&mut v, t);
            worst = worst.max((v.center.re() - hi) / v.scale.to_f64());
        }
        assert!(worst > 0.0 && worst < RUBBER_BAND * 600.0, "{worst}");
        n.handle(InputEvent::KeyUp(NavKey::Right), &mut v, t);
        run(&mut n, &mut v, t, t + 3.0, 60.0);
        assert!(!n.is_moving() && !n.limits().outside(&v.center), "{}", v.center.re());
        // Zooming in at the far right edge of the view, over and over: the centre follows the
        // pointer only up to the band (in complex units: a deeper zoom shows the same stretch
        // larger), and comes back.
        let (mut n, mut v) = at_home();
        v.center = Center::new(hi - 0.1, 0.0);
        let reach = RUBBER_BAND * 600.0 * v.scale.to_f64();
        for i in 0..100 {
            n.handle(
                InputEvent::WheelZoom {
                    delta: 1.0,
                    x: 499.0,
                    y: 0.0,
                },
                &mut v,
                i as f64 / 120.0,
            );
            let past = v.center.re() - hi;
            assert!(past < reach, "{i}: {past}");
        }
        run(&mut n, &mut v, 1.0, 3.0, 60.0);
        assert!(!n.limits().outside(&v.center));
    }

    /// Flights, journeys, places and tours fly to targets clamped into the limits; a view set
    /// from outside (restored, pasted) is clamped at once by `keep_in_limits`, a rubber band
    /// of the navigator's own is not.
    #[test]
    fn targets_and_outside_views_are_clamped() {
        let (mut n, mut v) = at_home();
        let target = Viewport::new(8.0, -6.0, 0.05);
        n.fly_to(&target, &v.clone(), 0.0);
        let mut t = 0.0;
        while n.flight().is_some() && t < 30.0 {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
            assert!(v.scale <= n.limits().max_scale, "{t}: {}", v.scale);
        }
        let l = n.limits();
        assert!(v.scale == l.max_scale, "{}", v.scale);
        assert!((v.center.re() - (l.center.0 + CENTER_LIMIT)).abs() < 1e-9);
        assert!((v.center.im() - (l.center.1 - CENTER_LIMIT)).abs() < 1e-9);

        // A restored view far out: clamped at once (centre onto the box, scale to the limit).
        let (mut n, _) = at_home();
        let mut v = Viewport::new(-40.0, 3.0, 0.04);
        n.jumped();
        assert!(n.keep_in_limits(&mut v));
        assert_eq!(v.scale, n.limits().max_scale);
        assert!((v.center.re() - (l.center.0 - CENTER_LIMIT)).abs() < 1e-9);
        assert!((v.center.im() - (l.center.1 + CENTER_LIMIT)).abs() < 1e-9);
        assert!(!n.keep_in_limits(&mut v), "inside: unchanged");
        // Deep and far out (beyond f64 in view points): still onto the box.
        let mut deep = Viewport::with_center(Center::new(1e6, 0.1), Scale::pow2(-3000));
        assert!(n.keep_in_limits(&mut deep));
        assert!(!n.limits().outside(&deep.center) && deep.center.im() == 0.1);
        // A view inside is untouched bit for bit.
        let mut inside = Viewport::new(-0.743, 0.131, 1e-9).with_angle(0.4);
        let same = inside.clone();
        assert!(!n.keep_in_limits(&mut inside) && inside == same);

        // Burning Ship: its own home (centre −0.45 + 0.62i) is the box's middle.
        n.set_formula(crate::formula::Formula::BurningShip);
        let l = n.limits();
        let home = Viewport::fitting_for(crate::formula::Formula::BurningShip, W, 600.0);
        assert_eq!(l.center, (home.center.re(), home.center.im()));
        assert_eq!(l.max_scale, home.scale / crate::viewport::ZOOM_OUT_LIMIT);
        let mut v = Viewport::new(-0.45, 3.5, 1.0);
        n.keep_in_limits(&mut v);
        assert!((v.center.im() - (0.62 + CENTER_LIMIT)).abs() < 1e-9 && v.scale == l.max_scale);
    }

    // MARK: View limits, user report 2026-10-25 (kb: decisions/controls "View limits")

    /// The user's window (a full-screen 16" MacBook Pro: 1728 x 1081 points) at home.
    fn user_window() -> (Navigator, Viewport) {
        let mut n = Navigator::default();
        let mut v = Viewport::fitting(1728.0, 1081.0);
        n.handle(
            InputEvent::Resize {
                width: 1728.0,
                height: 1081.0,
            },
            &mut v,
            0.0,
        );
        (n, v)
    }

    /// How far the centre is past its box, in complex units (negative: inside).
    fn past_bound(n: &Navigator, v: &Viewport) -> f64 {
        let l = n.limits();
        let re = (v.center.re() - l.center.0).abs() - CENTER_LIMIT;
        let im = (v.center.im() - l.center.1).abs() - CENTER_LIMIT;
        re.max(im)
    }

    /// One display refresh as the host runs it: views set from outside clamped, then motion.
    fn host_frame(n: &mut Navigator, v: &mut Viewport, t: f64) {
        n.keep_in_limits(v);
        n.advance(v, t);
    }

    /// The user's report (2026-10-25, "I can keep panning", HUD at the zoom limit 0.78×, the
    /// centre 2.59 past the im bound): Space held to zoom out at ×4.95/s (and faster after
    /// E), or out and then in, with every kind of pan at once (keys, drag, trackpad scroll and
    /// its momentum, flings, the stick, a pinch or wheel zoom or twist about an off-centre
    /// point, the autopilot with keys): the centre is never further past its bound than the
    /// band (RUBBER_BAND of the shorter side at the current scale; at most RUBBER_BAND of
    /// the shorter side at the zoom limit in complex units), and it is back inside soon after
    /// the hand lets go.
    #[test]
    fn space_zoom_with_panning_keeps_to_the_limits() {
        type Drive = fn(&mut Navigator, &mut Viewport, f64, usize);
        let drives: [(&str, Drive); 12] = [
            ("arrow key", |n, v, t, i| {
                if i == 0 {
                    n.handle(InputEvent::KeyDown(NavKey::Right), v, t);
                }
            }),
            ("two keys", |n, v, t, i| {
                if i == 0 {
                    n.handle(InputEvent::KeyDown(NavKey::Right), v, t);
                    n.handle(InputEvent::KeyDown(NavKey::Down), v, t);
                }
            }),
            ("drag", |n, v, t, _| {
                n.handle(InputEvent::Pan { dx: -15.0, dy: -15.0 }, v, t);
            }),
            ("scroll", |n, v, t, _| {
                let e = InputEvent::Scroll {
                    dx: -15.0,
                    dy: -15.0,
                    momentum: false,
                };
                n.handle(e, v, t);
            }),
            ("scroll momentum", |n, v, t, i| {
                let e = InputEvent::Scroll {
                    dx: -15.0,
                    dy: -15.0,
                    momentum: i > 60,
                };
                n.handle(e, v, t);
            }),
            ("flings", |n, v, t, i| {
                if i % 20 < 5 {
                    n.handle(InputEvent::Pan { dx: -40.0, dy: -40.0 }, v, t);
                } else if i % 20 == 5 {
                    n.handle(InputEvent::GestureEnd, v, t);
                }
            }),
            ("stick", |n, v, t, i| {
                if i == 0 {
                    let e = InputEvent::Gamepad {
                        lx: 1.0,
                        ly: -1.0,
                        rx: 0.0,
                        ry: 0.0,
                        lt: 0.0,
                        rt: 0.0,
                        boost: true,
                    };
                    n.handle(e, v, t);
                }
            }),
            ("pinch off centre", |n, v, t, _| {
                let e = InputEvent::Pinch {
                    ratio: 1.01,
                    x: 800.0,
                    y: 500.0,
                    dx: -10.0,
                    dy: -10.0,
                };
                n.handle(e, v, t);
            }),
            ("wheel in off centre", |n, v, t, _| {
                let e = InputEvent::WheelZoom {
                    delta: 0.2,
                    x: 850.0,
                    y: -520.0,
                };
                n.handle(e, v, t);
            }),
            ("twist off centre", |n, v, t, _| {
                let e = InputEvent::Rotate {
                    radians: 0.05,
                    x: 850.0,
                    y: 520.0,
                };
                n.handle(e, v, t);
            }),
            ("drag and twist", |n, v, t, _| {
                n.handle(InputEvent::Pan { dx: -15.0, dy: -15.0 }, v, t);
                let e = InputEvent::Rotate {
                    radians: 0.03,
                    x: -850.0,
                    y: 520.0,
                };
                n.handle(e, v, t);
            }),
            ("autopilot and key", |n, v, t, i| {
                if i == 0 {
                    tap(n, v, NavKey::Autopilot, t);
                    n.handle(InputEvent::KeyDown(NavKey::Right), v, t);
                }
            }),
        ];
        let min_side = 1081.0;
        for zoom in ["space out", "space out faster", "space out then in"] {
            for (name, drive) in drives {
                let (mut n, mut v) = user_window();
                let max_band = RUBBER_BAND * min_side * n.limits().max_scale.to_f64();
                let mut t = 0.0;
                n.handle(InputEvent::Shift(true), &mut v, t);
                n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, t);
                if zoom == "space out faster" {
                    tap(&mut n, &mut v, NavKey::FasterZoom, t);
                    tap(&mut n, &mut v, NavKey::FasterZoom, t);
                }
                let mut worst: f64 = 0.0;
                for i in 0..1200 {
                    t += 1.0 / 120.0;
                    if zoom == "space out then in" && i == 600 {
                        n.handle(InputEvent::Shift(false), &mut v, t);
                    }
                    drive(&mut n, &mut v, t, i);
                    host_frame(&mut n, &mut v, t);
                    let past = past_bound(&n, &v);
                    let band = RUBBER_BAND * min_side * v.scale.to_f64();
                    assert!(
                        past <= band * (1.0 + 1e-6) + 1e-12,
                        "{zoom}, {name}, {i}: {past} past, band {band}"
                    );
                    assert!(past <= max_band * (1.0 + 1e-6), "{zoom}, {name}, {i}: {past}");
                    assert!(v.scale <= n.limits().max_scale, "{zoom}, {name}, {i}: {}", v.scale);
                    worst = worst.max(past);
                }
                // The user's state: 2.59 past the bound at the zoom limit; the band now allows
                // well under one unit there.
                assert!(worst < 0.8, "{zoom}, {name}: {worst}");
                // The hand lets go: back inside within a second and a half.
                n.handle(InputEvent::KeyUp(NavKey::Zoom), &mut v, t);
                n.handle(InputEvent::Shift(false), &mut v, t);
                n.handle(InputEvent::FocusLost, &mut v, t);
                n.handle(InputEvent::GestureEnd, &mut v, t);
                for _ in 0..180 {
                    t += 1.0 / 120.0;
                    host_frame(&mut n, &mut v, t);
                }
                assert!(
                    past_bound(&n, &v) <= 1e-9,
                    "{zoom}, {name}: {} past",
                    past_bound(&n, &v)
                );
                assert!(!n.is_stretched(), "{zoom}, {name}: stretched");
            }
        }
    }

    /// Zooming out with Space at the default ×4.95/s slows visibly into the limit (the user:
    /// "zooming out does not slow down"): from 90 % of the held rate down to a quarter takes
    /// at least half a second, and it never passes the limit.
    #[test]
    fn space_zoom_out_slows_visibly_into_the_limit() {
        let (mut n, mut v) = user_window();
        let max = n.limits().max_scale;
        // From 20 times deeper than the limit (3 e-folds: Space reaches its full rate first).
        v.scale = max / 20.0;
        let mut t = 0.0;
        n.handle(InputEvent::Shift(true), &mut v, t);
        n.handle(InputEvent::KeyDown(NavKey::Zoom), &mut v, t);
        let full = n.zoom_speed();
        let dt = 1.0 / 120.0;
        let (mut slowing, mut slow) = (None, None);
        let mut was_full = false;
        for _ in 0..1200 {
            let s0 = v.scale;
            t += dt;
            host_frame(&mut n, &mut v, t);
            let rate = v.scale.ratio(s0).ln() / dt;
            assert!(v.scale <= max);
            if rate > 0.95 * full {
                was_full = true;
            }
            if was_full && slowing.is_none() && rate < 0.9 * full {
                slowing = Some(t);
            }
            if was_full && slow.is_none() && rate < 0.25 * full {
                slow = Some(t);
            }
        }
        let (a, b) = (slowing.expect("slows"), slow.expect("stops"));
        assert!(b - a >= 0.5, "slows from 90 % to 25 % in {} s", b - a);
        assert!(to_limit(&n, &v) < 0.02, "{}", to_limit(&n, &v));
    }

    /// The safety net: whatever left the centre past its bound with the band set (a jump while
    /// stretched, a band that never springs back), `keep_in_limits` caps it at the band at
    /// once, and clamps it onto the bound once the spring back has not run for a while with
    /// the hand off.
    #[test]
    fn a_stuck_rubber_band_is_still_kept_in_limits() {
        let (mut n, mut v) = user_window();
        n.advance(&mut v, 10.0);
        v.scale = n.limits().max_scale;
        let l = n.limits();
        v.center = Center::new(l.center.0 + 9.0, l.center.1 - 7.0);
        n.rubber = true;
        n.keep_in_limits(&mut v);
        let band = RUBBER_BAND * 1081.0 * v.scale.to_f64();
        let past = past_bound(&n, &v);
        assert!(past <= band * (1.0 + 1e-9), "{past} past, band {band}");
        // The spring back never runs (no frames advance the motion): clamped all the same.
        n.last_tick = Some(11.0);
        n.keep_in_limits(&mut v);
        assert!(
            past_bound(&n, &v) <= 1e-9 && !n.is_stretched(),
            "{}",
            past_bound(&n, &v)
        );
    }

    /// At the zoom limit, stretched to the band at the worst corner of the box, some of the set
    /// is still on screen (any window shape): the band never takes the set out of view.
    #[test]
    fn the_set_stays_in_view_at_the_band() {
        let escapes = |re: f64, im: f64| {
            let (mut x, mut y) = (0.0f64, 0.0f64);
            for _ in 0..200 {
                (x, y) = (x * x - y * y + re, 2.0 * x * y + im);
                if x * x + y * y > 4.0 {
                    return true;
                }
            }
            false
        };
        for (w, h) in [(1728.0, 1081.0), (1000.0, 1000.0), (400.0, 860.0), (2560.0, 600.0)] {
            let mut n = Navigator::default();
            let mut v = Viewport::fitting(w, h);
            n.handle(InputEvent::Resize { width: w, height: h }, &mut v, 0.0);
            let l = n.limits();
            let s = l.max_scale.to_f64();
            let band = RUBBER_BAND * w.min(h) * s;
            for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                let c = (
                    l.center.0 + sx * (CENTER_LIMIT + band),
                    l.center.1 + sy * (CENTER_LIMIT + band),
                );
                let mut inside = 0;
                for i in 0..=400 {
                    for j in 0..=400 {
                        let re = c.0 + (i as f64 / 400.0 - 0.5) * w * s;
                        let im = c.1 + (j as f64 / 400.0 - 0.5) * h * s;
                        inside += usize::from(!escapes(re, im));
                    }
                }
                assert!(inside > 0, "{w}x{h} corner ({sx}, {sy}): none of the set in view");
            }
        }
    }
}
