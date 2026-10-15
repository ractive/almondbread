//! The `Explorer`: the Rust renderer hosted in the app's view (kb: platform/render-architecture,
//! "Hosting in the app"; platform/rust-swift-ffi).
//!
//! Swift creates a `CAMetalLayer`, hands its pointer over once, and then makes control-plane
//! calls only: `resize` on layout changes, `input` with decoded events, `frame` once per
//! display refresh (from the display link, which it pauses while `frame` returns false),
//! `settings`/`set_settings` when the inspector changes something, `stats` at ~10 Hz for the
//! HUD. Device, queue, buffers, pipelines, drawables, scheduling and motion all live in Rust.
//! Nothing per pixel crosses; a frame is one call.
//!
//! Threading: Swift calls from the main thread. The state sits behind a mutex anyway, so a
//! call from another thread is serialised, never a data race.

use std::ffi::c_void;
use std::sync::{Arc, Mutex, MutexGuard};

use almondbread_core::image::write_png_bgra;
use almondbread_core::input::{self, EdgePan as CoreEdgePan, InputMode as CoreInputMode, NavKey as CoreNavKey};
use almondbread_core::settings::{RenderSettings, Supersampling};
use almondbread_core::viewport::{Center, Scale, Viewport};
use almondbread_render::metal::{MetalBackend, MetalLayerSurface, PresentationLog, media_time};
use almondbread_render::{GpuError, Host, Renderer};

use crate::{CoreError, invalid};

fn gpu(e: GpuError) -> CoreError {
    CoreError::Gpu { message: e.0 }
}

/// Keys with a navigation meaning; Swift decodes its key events into these
/// (kb: decisions/controls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NavKey {
    /// W / up arrow
    Up,
    /// S / down arrow
    Down,
    /// A / left arrow
    Left,
    /// D / right arrow
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
    /// R: home view fitted to the window.
    Reset,
    /// Z: autopilot on / off.
    Autopilot,
    /// Esc: stops the autopilot, a flight or a journey (not handled while nothing flies: the
    /// platform keeps the key).
    Stop,
    /// `,` / `.` (held): turn the image counter-clockwise / clockwise.
    RotateCcw,
    RotateCw,
    /// `/`: turn back upright.
    Straighten,
    /// J: journey on (random seed) / resume after manual input / off.
    Journey,
}

impl From<NavKey> for CoreNavKey {
    fn from(k: NavKey) -> Self {
        match k {
            NavKey::Up => Self::Up,
            NavKey::Down => Self::Down,
            NavKey::Left => Self::Left,
            NavKey::Right => Self::Right,
            NavKey::Zoom => Self::Zoom,
            NavKey::SlowerZoom => Self::SlowerZoom,
            NavKey::FasterZoom => Self::FasterZoom,
            NavKey::StepIn => Self::StepIn,
            NavKey::StepOut => Self::StepOut,
            NavKey::Reset => Self::Reset,
            NavKey::Autopilot => Self::Autopilot,
            NavKey::Stop => Self::Stop,
            NavKey::RotateCcw => Self::RotateCcw,
            NavKey::RotateCw => Self::RotateCw,
            NavKey::Straighten => Self::Straighten,
            NavKey::Journey => Self::Journey,
        }
    }
}

/// Which input was used last (kb: decisions/controls, revision): pointer mode shows the
/// system crosshair cursor over the canvas, keyboard mode hides it and draws the overlay
/// crosshair at `crosshair_x()`, `crosshair_y()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum InputMode {
    Pointer,
    Keyboard,
}

impl From<CoreInputMode> for InputMode {
    fn from(m: CoreInputMode) -> Self {
        match m {
            CoreInputMode::Pointer => Self::Pointer,
            CoreInputMode::Keyboard => Self::Keyboard,
        }
    }
}

/// Edge panning setting (pointer mode): off, while a continuous zoom runs, or always.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EdgePan {
    Off,
    WhileZooming,
    Always,
}

impl From<EdgePan> for CoreEdgePan {
    fn from(e: EdgePan) -> Self {
        match e {
            EdgePan::Off => Self::Off,
            EdgePan::WhileZooming => Self::WhileZooming,
            EdgePan::Always => Self::Always,
        }
    }
}

impl From<CoreEdgePan> for EdgePan {
    fn from(e: CoreEdgePan) -> Self {
        match e {
            CoreEdgePan::Off => Self::Off,
            CoreEdgePan::WhileZooming => Self::WhileZooming,
            CoreEdgePan::Always => Self::Always,
        }
    }
}

/// Decoded platform events. Positions are view points from the view centre, y up.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum InputEvent {
    KeyDown {
        key: NavKey,
    },
    KeyUp {
        key: NavKey,
    },
    /// Shift state (Shift+Space zooms out).
    Shift {
        down: bool,
    },
    /// Key window / first responder lost: key-ups may never arrive, so stop everything.
    FocusLost,
    /// Parts of the view covered by window chrome (safe-area insets), in points.
    Insets {
        top: f64,
        left: f64,
        bottom: f64,
        right: f64,
    },
    /// Pointer position (mouse, trackpad, iPad pointer): the zoom target in pointer mode;
    /// travel switches from keyboard to pointer mode.
    Pointer {
        x: f64,
        y: f64,
    },
    /// The pointer left the canvas (or is over the HUD): zooms go to the centre, edge
    /// panning stops.
    PointerExited,
    /// Content follows a drag / one-finger pan.
    Pan {
        dx: f64,
        dy: f64,
    },
    /// Trackpad two-finger scroll (content follows; `momentum`: the OS's momentum phase). Pans
    /// like `Pan`; while the autopilot is on it sets the autopilot's speed instead.
    Scroll {
        dx: f64,
        dy: f64,
        momentum: bool,
    },
    /// A single click / tap on the canvas: pauses or resumes the autopilot.
    Click {
        x: f64,
        y: f64,
    },
    /// Mouse wheel notch or zoom scroll: zoom by 1.15^delta at the zoom target ((x, y) in
    /// pointer mode).
    WheelZoom {
        delta: f64,
        x: f64,
        y: f64,
    },
    /// Trackpad pinch: zoom by 1 + magnification at the zoom target ((x, y) in pointer mode).
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
    /// Double click / tap: zoom x2 (out: x0.5) at (x, y); switches to pointer mode.
    DoubleTap {
        x: f64,
        y: f64,
        out: bool,
    },
    /// Zoom by `factor` at the zoom target (menu commands), animated.
    ZoomBy {
        factor: f64,
    },
    /// A mouse wheel notch (not a precise scroll): zoom by 1.15^delta at the zoom target,
    /// animated ((x, y) in pointer mode).
    WheelNotch {
        delta: f64,
        x: f64,
        y: f64,
    },
    /// The drag, touch pan, pinch or trackpad magnify that drove the view ended: it flings on
    /// if it was still moving. Not for trackpad scrolling (the OS sends momentum scrolls).
    GestureEnd,
    /// Game controller state, sent when it changes: sticks −1...1 (y up), triggers 0...1,
    /// `boost` (a face button) speeds motion up. Dead zones and curves are Rust's.
    Gamepad {
        lx: f64,
        ly: f64,
        rx: f64,
        ry: f64,
        lt: f64,
        rt: f64,
        boost: bool,
    },
    /// Trackpad / touch rotation: the image turns counter-clockwise by `radians` about (x, y)
    /// (pointer mode; keyboard mode: the crosshair), 1:1.
    Rotate {
        radians: f64,
        x: f64,
        y: f64,
    },
}

impl InputEvent {
    /// The core event, or None if a coordinate is not finite (a NaN would poison the view).
    fn to_core(self) -> Option<input::InputEvent> {
        use input::InputEvent as E;
        let ok = |v: &[f64]| v.iter().all(|x| x.is_finite());
        Some(match self {
            Self::KeyDown { key } => E::KeyDown(key.into()),
            Self::KeyUp { key } => E::KeyUp(key.into()),
            Self::Shift { down } => E::Shift(down),
            Self::FocusLost => E::FocusLost,
            Self::Insets {
                top,
                left,
                bottom,
                right,
            } if ok(&[top, left, bottom, right]) => E::Insets {
                top,
                left,
                bottom,
                right,
            },
            Self::Pointer { x, y } if ok(&[x, y]) => E::Pointer(Some((x, y))),
            Self::PointerExited => E::Pointer(None),
            Self::Pan { dx, dy } if ok(&[dx, dy]) => E::Pan { dx, dy },
            Self::Scroll { dx, dy, momentum } if ok(&[dx, dy]) => E::Scroll { dx, dy, momentum },
            Self::Click { x, y } if ok(&[x, y]) => E::Click { x, y },
            Self::WheelZoom { delta, x, y } if ok(&[delta, x, y]) => E::WheelZoom { delta, x, y },
            Self::Magnify { magnification, x, y } if ok(&[magnification, x, y]) && magnification > -1.0 => {
                E::Magnify { magnification, x, y }
            }
            Self::Pinch { ratio, x, y, dx, dy } if ok(&[ratio, x, y, dx, dy]) && ratio > 0.0 => {
                E::Pinch { ratio, x, y, dx, dy }
            }
            Self::DoubleTap { x, y, out } if ok(&[x, y]) => E::DoubleTap { x, y, out },
            Self::ZoomBy { factor } if ok(&[factor]) && factor > 0.0 => E::ZoomBy { factor },
            Self::WheelNotch { delta, x, y } if ok(&[delta, x, y]) => E::WheelNotch { delta, x, y },
            Self::GestureEnd => E::GestureEnd,
            Self::Rotate { radians, x, y } if ok(&[radians, x, y]) => E::Rotate { radians, x, y },
            // Non-finite axes read as centred (handled in the navigator).
            Self::Gamepad {
                lx,
                ly,
                rx,
                ry,
                lt,
                rt,
                boost,
            } => E::Gamepad {
                lx,
                ly,
                rx,
                ry,
                lt,
                rt,
                boost,
            },
            _ => return None,
        })
    }
}

/// What an input event did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct InputResult {
    /// The event was ours: swallow it (no system beep).
    pub handled: bool,
    /// The view needs frames: resume the display link.
    pub redraw: bool,
    /// The input mode after the event (pointer moves below the switch threshold keep
    /// keyboard mode: the platform keeps the cursor hidden).
    pub mode: InputMode,
}

/// Bits of `Explorer::frame`'s result: keep the display link running (when clear, pause it
/// until an `input` asks for frames).
pub const FRAME_ANIMATING: u8 = 1;
/// Keyboard mode: hide the cursor, draw the overlay crosshair at
/// (`crosshair_x()`, `crosshair_y()`).
pub const FRAME_KEYBOARD: u8 = 2;
/// The autopilot is on (iOS: a finger steers instead of panning).
pub const FRAME_AUTOPILOT: u8 = 4;

/// Why the autopilot flies slower than its set speed (kb: decisions/controls, autopilot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AutopilotHint {
    None,
    /// Previews cannot stay sharp at this speed.
    Rendering,
    /// Waiting for a deep reference.
    Reference,
    /// Heading into the interior.
    Interior,
    /// Heading into featureless exterior.
    Featureless,
    /// The depth limit.
    Limit,
    /// Nothing ahead and no target: zooming out until there is.
    Rescue,
}

impl From<almondbread_core::autopilot::Hint> for AutopilotHint {
    fn from(h: almondbread_core::autopilot::Hint) -> Self {
        use almondbread_core::autopilot::Hint as H;
        match h {
            H::None => Self::None,
            H::Rendering => Self::Rendering,
            H::Reference => Self::Reference,
            H::Interior => Self::Interior,
            H::Featureless => Self::Featureless,
            H::Limit => Self::Limit,
            H::Rescue => Self::Rescue,
        }
    }
}

/// A flight to coordinates for the HUD (kb: decisions/controls, fly to).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FlightStatus {
    pub active: bool,
    /// 0...1 along the path.
    pub progress: f64,
    /// Set speed, e-folds per second at the zoom ends (Q / E, scroll).
    pub speed: f64,
    /// Renderer back-off 0...1 and why (rendering, reference).
    pub throttle: f64,
    pub hint: AutopilotHint,
    /// Esc: slowing to a stop.
    pub stopping: bool,
    /// The target's zoom (log10, relative to home).
    pub target_zoom_log10: f64,
}

/// A journey for the HUD and the control center (kb: decisions/controls, "Journey").
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct JourneyStatus {
    pub active: bool,
    /// Manual input took over: J, the button or a click resumes.
    pub suspended: bool,
    /// The seed ("Journey #4711"); the daily journey's is the date (yyyymmdd).
    pub seed: u64,
    pub daily: bool,
    /// Legs flown (the current one included).
    pub leg: u32,
    /// "flying", "resting", "planning".
    pub phase: String,
    /// The current target: kind ("minibrot", "embedded Julia", "spiral", ...), its label
    /// ("period-34 minibrot", "spiral M(36,2)") and depth (log10 of the magnification).
    pub kind: String,
    pub label: String,
    pub target_zoom_log10: f64,
    /// Set speed, e-folds per second at the zoom ends.
    pub speed: f64,
}

/// The autopilot for the HUD.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AutopilotStatus {
    pub active: bool,
    pub paused: bool,
    /// Set speed and current (eased, throttled) zoom rate, e-folds per second.
    pub speed: f64,
    pub rate: f64,
    /// 0...1: how the renderer keeps up, and how uniform the heading is.
    pub throttle: f64,
    pub safety: f64,
    pub hint: AutopilotHint,
    /// Target lock on, and what it aims at ("towards spiral M(36,2)"; "" while steered).
    pub lock: bool,
    pub lock_label: String,
}

/// Everything that affects the image except the viewport (the inspector's model).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ExplorerSettings {
    pub auto_iterations: bool,
    pub manual_iterations: u64,
    pub max_iterations_cap: u64,
    /// Stop pixels whose orbit derivative shows an attracting cycle.
    pub interior_detection: bool,
    pub palette_id: String,
    /// Palette cycles per iteration.
    pub color_density: f64,
    pub color_offset: f64,
    pub log_coloring: bool,
    pub smooth_coloring: bool,
    /// Interior colour, linear RGB.
    pub interior_red: f32,
    pub interior_green: f32,
    pub interior_blue: f32,
    pub shading: f64,
    /// Degrees.
    pub light_angle: f64,
    pub outline: f64,
    pub outline_width: f64,
    /// Outline (boundary emphasis) colour, linear RGB.
    pub edge_red: f32,
    pub edge_green: f32,
    pub edge_blue: f32,
    /// Highlight strength 0...1 and Blinn-Phong exponent.
    pub specular: f64,
    pub shininess: f64,
    /// Elevation of the light (0 grazing, larger from the front).
    pub light_height: f64,
    /// Glow along the boundary: strength 0...1, linear RGB, width in pixels.
    pub glow: f64,
    pub glow_red: f32,
    pub glow_green: f32,
    pub glow_blue: f32,
    pub glow_width: f64,
    /// Filter the palette by the colour gradient (palette anti-aliasing).
    pub palette_filter: bool,
    /// Colour cycling while idle, palette cycles per second (0: off).
    pub cycle_speed: f64,
    /// Palette phase shift per decade of zoom (0: off).
    pub depth_tint: f64,
    /// Anti-aliasing: samples per axis, 1 to 4 (the view accumulates the square while idle).
    pub supersampling: u8,
    pub interactive_preview: bool,
    /// Interior colouring (kb: techniques/coloring-methods, "Interior colouring as built").
    pub interior_mode: InteriorMode,
    /// Palette cycles of the interior modes.
    pub interior_density: f64,
    /// Angle mode: stripes per turn.
    pub interior_stripes: f64,
    /// Dense regions fade to a calm dark colour with thin filaments (off: the old grey).
    pub dense_calm: bool,
    /// Cinematic effects (kb: decisions/controls "Cinematic effects"): master switch, view
    /// spin (degrees per e-fold of automatic zoom), palette hue drift (degrees per e-fold of
    /// depth), phase drift (cycles per e-fold of depth), cycling while flying (cycles/s).
    pub cinematic: bool,
    pub cinematic_spin: f64,
    pub cinematic_hue_drift: f64,
    pub cinematic_phase_drift: f64,
    pub cinematic_fly_cycle: f64,
}

/// How the inside of the set is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum InteriorMode {
    Flat,
    Bubbles,
    Period,
    Angle,
    Distance,
}

impl From<almondbread_core::InteriorMode> for InteriorMode {
    fn from(m: almondbread_core::InteriorMode) -> Self {
        use almondbread_core::InteriorMode as M;
        match m {
            M::Flat => Self::Flat,
            M::Bubbles => Self::Bubbles,
            M::Period => Self::Period,
            M::Angle => Self::Angle,
            M::Distance => Self::Distance,
        }
    }
}

impl From<InteriorMode> for almondbread_core::InteriorMode {
    fn from(m: InteriorMode) -> Self {
        match m {
            InteriorMode::Flat => Self::Flat,
            InteriorMode::Bubbles => Self::Bubbles,
            InteriorMode::Period => Self::Period,
            InteriorMode::Angle => Self::Angle,
            InteriorMode::Distance => Self::Distance,
        }
    }
}

/// Corner of the mini-map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MapCorner {
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

/// Overlays drawn by the renderer over the fractal (kb: decisions/controls, "Orbit overlay
/// and mini-map"); never part of the image, the history or exports.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OverlaySettings {
    /// The orbit of the point under the pointer (keyboard mode: the crosshair).
    pub orbit: bool,
    /// Iterates drawn, 1 ... 1000.
    pub orbit_length: u32,
    /// The mini-map (whole set with the view marked; clicks jump there).
    pub map: bool,
    /// Nested maps at decades of zoom, 1 ... 4.
    pub map_levels: u32,
    /// Width of a map panel in points (80 ... 400).
    pub map_size: f64,
    pub map_corner: MapCorner,
}

impl From<&almondbread_core::settings::OverlaySettings> for OverlaySettings {
    fn from(s: &almondbread_core::settings::OverlaySettings) -> Self {
        use almondbread_core::settings::Corner;
        Self {
            orbit: s.orbit,
            orbit_length: s.orbit_length as u32,
            map: s.map,
            map_levels: s.map_levels as u32,
            map_size: s.map_size,
            map_corner: match s.map_corner {
                Corner::BottomLeft => MapCorner::BottomLeft,
                Corner::BottomRight => MapCorner::BottomRight,
                Corner::TopLeft => MapCorner::TopLeft,
                Corner::TopRight => MapCorner::TopRight,
            },
        }
    }
}

impl OverlaySettings {
    fn to_core(&self) -> almondbread_core::settings::OverlaySettings {
        use almondbread_core::settings::Corner;
        almondbread_core::settings::OverlaySettings {
            orbit: self.orbit,
            orbit_length: self.orbit_length as usize,
            map: self.map,
            map_levels: self.map_levels as usize,
            map_size: self.map_size,
            map_corner: match self.map_corner {
                MapCorner::BottomLeft => Corner::BottomLeft,
                MapCorner::BottomRight => Corner::BottomRight,
                MapCorner::TopLeft => Corner::TopLeft,
                MapCorner::TopRight => Corner::TopRight,
            },
        }
        .sanitized()
    }
}

/// The π meter's state, for the control center (the overlay draws the panel itself; kb:
/// techniques/pi-in-the-mandelbrot-set).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PiMeterInfo {
    pub enabled: bool,
    /// "neck" or "cusp" when the view is centred on one; empty otherwise.
    pub point: String,
    /// ε of the view centre.
    pub eps: f64,
    /// The latest finished count there (0: none yet), its ε and N·ε (N·√ε).
    pub count: u64,
    pub count_eps: f64,
    pub product: f64,
    /// Digits of the product that agree with π.
    pub digits: u32,
    pub counting: bool,
    /// 0 ... 1 of the running count.
    pub progress: f64,
    /// The centre is closer than the meter counts (more than 1e9 iterations).
    pub too_deep: bool,
}

/// The probed point's orbit, for the UI (the overlay draws the same in the view).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OrbitInfo {
    /// "escapes at n 37", "period 3  |λ| 0.412  arg +0.250", "analysing ...", "" when off.
    pub text: String,
    /// Iterates drawn and |z_n| of the last.
    pub iterates: u32,
    pub last_abs: f64,
    /// 0 unless an attracting cycle was found.
    pub period: u64,
    /// 0 unless it escaped.
    pub escaped_at: u64,
    /// What the last mini-map click did (empty if none recently).
    pub map_note: String,
}

impl From<&RenderSettings> for ExplorerSettings {
    fn from(s: &RenderSettings) -> Self {
        Self {
            auto_iterations: s.auto_iterations,
            manual_iterations: s.manual_iterations as u64,
            max_iterations_cap: s.max_iterations_cap as u64,
            interior_detection: s.interior_detection,
            palette_id: s.palette_id.clone(),
            color_density: s.color_density,
            color_offset: s.color_offset,
            log_coloring: s.log_coloring,
            smooth_coloring: s.smooth_coloring,
            interior_red: s.interior_color[0],
            interior_green: s.interior_color[1],
            interior_blue: s.interior_color[2],
            shading: s.shading,
            light_angle: s.light_angle,
            outline: s.outline,
            outline_width: s.outline_width,
            edge_red: s.edge_color[0],
            edge_green: s.edge_color[1],
            edge_blue: s.edge_color[2],
            specular: s.specular,
            shininess: s.shininess,
            light_height: s.light_height,
            glow: s.glow,
            glow_red: s.glow_color[0],
            glow_green: s.glow_color[1],
            glow_blue: s.glow_color[2],
            glow_width: s.glow_width,
            palette_filter: s.palette_filter,
            cycle_speed: s.cycle_speed,
            depth_tint: s.depth_tint,
            supersampling: s.supersampling.factor() as u8,
            interactive_preview: s.interactive_preview,
            interior_mode: s.interior_mode.into(),
            interior_density: s.interior_density,
            interior_stripes: s.interior_stripes,
            dense_calm: s.dense_calm,
            cinematic: s.cinematic.enabled,
            cinematic_spin: s.cinematic.spin,
            cinematic_hue_drift: s.cinematic.hue_drift,
            cinematic_phase_drift: s.cinematic.phase_drift,
            cinematic_fly_cycle: s.cinematic.fly_cycle,
        }
    }
}

/// Upper bound on iteration limits accepted over the FFI.
const MAX_ITERATIONS: u64 = 1 << 31;

impl ExplorerSettings {
    /// Applies onto `s` (keeping what the renderer learned), validating every value.
    fn apply(&self, s: &mut RenderSettings) -> Result<(), CoreError> {
        let finite = [
            self.color_density,
            self.color_offset,
            self.shading,
            self.light_angle,
            self.outline,
            self.outline_width,
            self.specular,
            self.shininess,
            self.light_height,
            self.glow,
            self.glow_width,
            self.cycle_speed,
            self.depth_tint,
        ]
        .iter()
        .chain(&[
            self.interior_red as f64,
            self.interior_green as f64,
            self.interior_blue as f64,
            self.edge_red as f64,
            self.edge_green as f64,
            self.edge_blue as f64,
            self.glow_red as f64,
            self.glow_green as f64,
            self.glow_blue as f64,
        ])
        .all(|v| v.is_finite());
        if !finite {
            return Err(invalid("settings must be finite"));
        }
        if self.manual_iterations == 0
            || self.manual_iterations > MAX_ITERATIONS
            || self.max_iterations_cap == 0
            || self.max_iterations_cap > MAX_ITERATIONS
        {
            return Err(invalid(format!("iteration limits must be in 1...{MAX_ITERATIONS}")));
        }
        let ss = Supersampling::from_factor(self.supersampling as usize)
            .ok_or_else(|| invalid("supersampling must be 1 to 4"))?;
        s.auto_iterations = self.auto_iterations;
        s.manual_iterations = self.manual_iterations as usize;
        s.max_iterations_cap = self.max_iterations_cap as usize;
        s.interior_detection = self.interior_detection;
        s.palette_id = self.palette_id.clone();
        s.color_density = self.color_density.max(0.0);
        s.color_offset = self.color_offset;
        s.log_coloring = self.log_coloring;
        s.smooth_coloring = self.smooth_coloring;
        s.interior_color = [self.interior_red, self.interior_green, self.interior_blue];
        s.shading = self.shading.clamp(0.0, 1.0);
        s.light_angle = self.light_angle;
        s.outline = self.outline.clamp(0.0, 1.0);
        s.outline_width = self.outline_width.max(0.0);
        let unit = |v: f32| v.clamp(0.0, 1.0);
        s.edge_color = [unit(self.edge_red), unit(self.edge_green), unit(self.edge_blue)];
        s.specular = self.specular.clamp(0.0, 1.0);
        s.shininess = self.shininess.clamp(1.0, 256.0);
        s.light_height = self.light_height.clamp(0.05, 10.0);
        s.glow = self.glow.clamp(0.0, 1.0);
        s.glow_color = [unit(self.glow_red), unit(self.glow_green), unit(self.glow_blue)];
        s.glow_width = self.glow_width.clamp(0.5, 100.0);
        s.palette_filter = self.palette_filter;
        s.cycle_speed = self.cycle_speed.clamp(-2.0, 2.0);
        s.depth_tint = self.depth_tint.clamp(-1.0, 1.0);
        s.supersampling = ss;
        s.interactive_preview = self.interactive_preview;
        s.interior_mode = self.interior_mode.into();
        s.interior_density = if self.interior_density.is_finite() {
            self.interior_density.clamp(0.0, 10.0)
        } else {
            0.25
        };
        s.interior_stripes = if self.interior_stripes.is_finite() {
            self.interior_stripes.clamp(0.0, 64.0)
        } else {
            8.0
        };
        s.dense_calm = self.dense_calm;
        s.cinematic = almondbread_core::cinematic::Cinematic {
            enabled: self.cinematic,
            spin: self.cinematic_spin,
            hue_drift: self.cinematic_hue_drift,
            phase_drift: self.cinematic_phase_drift,
            fly_cycle: self.cinematic_fly_cycle,
        }
        .sanitized();
        Ok(())
    }
}

/// A view: centre (decimal strings with the digits the view needs: arbitrary precision since
/// M3b) and scale.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ViewportInfo {
    pub re: String,
    pub im: String,
    /// Complex units per view point (0 beyond f64's range: use `scale_text`).
    pub scale: f64,
    /// The scale as text at any depth ("1.234e-1000").
    pub scale_text: String,
    /// Magnification relative to the home view (inf beyond f64: use `zoom_log10`).
    pub zoom: f64,
    pub zoom_log10: f64,
    /// Smallest scale (0 beyond f64's range).
    pub min_scale: f64,
    /// Zoom stops here.
    pub at_precision_limit: bool,
    /// View rotation, degrees (the camera's counter-clockwise turn; 0 upright).
    pub angle_degrees: f64,
}

impl From<&Viewport> for ViewportInfo {
    fn from(v: &Viewport) -> Self {
        let (re, im) = v
            .center
            .to_strings_digits(almondbread_render::host::coordinate_digits(v));
        Self {
            re,
            im,
            scale: v.scale.to_f64(),
            scale_text: v.scale.to_text(6),
            zoom: v.zoom(),
            zoom_log10: v.zoom_log10(),
            min_scale: v.min_scale().to_f64(),
            at_precision_limit: v.at_precision_limit(),
            angle_degrees: v.angle.to_degrees(),
        }
    }
}

/// HUD and inspector values, read at ~10 Hz (never per input event).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ExplorerStats {
    /// The view the shown frame belongs to.
    pub shown: ViewportInfo,
    pub max_iterations: u64,
    /// GPU seconds of all iterate chunks of the shown view so far.
    pub gpu_time: f64,
    pub chunks: u64,
    pub render_width: u32,
    pub render_height: u32,
    pub is_rendering: bool,
    pub is_preview: bool,
    pub reference_time: f64,
    pub reference_length: u64,
    /// Period of the reference when it is a minibrot nucleus (0: the view centre).
    pub reference_period: u64,
    /// Precision of the reference in bits (53: the f64 orbit of shallow views).
    pub reference_bits: u32,
    /// A better reference is being computed in the background.
    pub reference_pending: bool,
    /// The deep (extended-range) kernel renders the view.
    pub deep: bool,
    /// Continuous zoom speed, e-folds per second (Q / E change it).
    pub zoom_speed: f64,
    pub autopilot: AutopilotStatus,
    pub flight: FlightStatus,
    pub journey: JourneyStatus,
    pub held_keys: u32,
    /// The current view's refined frame is presented and nothing is left to iterate.
    pub settled: bool,
    /// Frames the GPU failed to draw.
    pub failed_frames: u64,
}

/// An offscreen render written to a PNG.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OffscreenInfo {
    pub width: u32,
    pub height: u32,
    pub iterate_gpu_seconds: f64,
    pub chunks: u64,
    pub wall_seconds: f64,
}

/// See [`almondbread_render::InteractionReport`]. Times in ms.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct InteractionReport {
    pub presented: u64,
    pub fps: f64,
    pub interval_p50: f64,
    pub interval_p95: f64,
    pub interval_max: f64,
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub input_to_draw_p50: f64,
    pub draw_to_present_p50: f64,
    pub draw_cpu_p50: f64,
    pub draw_cpu_p95: f64,
    pub draw_cpu_mean: f64,
    pub iterate_gpu_per_frame: f64,
    pub colorize_gpu_per_frame: f64,
    pub preview_pct: f64,
    pub preview_scale: f64,
    pub skipped: u64,
    pub skipped_in_flight: u64,
    pub skipped_no_target: u64,
    pub draws: u64,
    pub encoded: u64,
}

impl From<almondbread_render::InteractionReport> for InteractionReport {
    fn from(r: almondbread_render::InteractionReport) -> Self {
        Self {
            presented: r.presented as u64,
            fps: r.fps,
            interval_p50: r.interval_p50,
            interval_p95: r.interval_p95,
            interval_max: r.interval_max,
            latency_p50: r.latency_p50,
            latency_p95: r.latency_p95,
            input_to_draw_p50: r.input_to_draw_p50,
            draw_to_present_p50: r.draw_to_present_p50,
            draw_cpu_p50: r.draw_cpu_p50,
            draw_cpu_p95: r.draw_cpu_p95,
            draw_cpu_mean: r.draw_cpu_mean,
            iterate_gpu_per_frame: r.iterate_gpu_per_frame,
            colorize_gpu_per_frame: r.colorize_gpu_per_frame,
            preview_pct: r.preview_pct,
            preview_scale: r.preview_scale,
            skipped: r.skipped as u64,
            skipped_in_flight: r.skipped_in_flight as u64,
            skipped_no_target: r.skipped_no_target as u64,
            draws: r.draws as u64,
            encoded: r.encoded as u64,
        }
    }
}

/// The last refined, full-resolution frame and the last input (interaction tests).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FinalFrame {
    /// When it was committed (`CACurrentMediaTime` clock); 0 if none yet.
    pub time: f64,
    /// "WxH" of its level.
    pub size: String,
    /// Time of the last input that moved the view.
    pub last_input: f64,
}

/// The shown frame against a fresh render.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FrameMismatch {
    pub pixels: u64,
    pub percent: f64,
    pub width: u32,
    pub height: u32,
}

struct Inner {
    host: Host<MetalBackend>,
    surface: MetalLayerSurface,
    log: Option<PresentationLog>,
    /// The flight benchmark playing in this view (window test `--flight-bench=`).
    bench: Option<crate::flight_bench::FlightBench>,
}

// SAFETY: `Inner` holds Objective-C objects (Metal device, queue, buffers, textures, the
// CAMetalLayer), which objc2 does not mark Send. Metal objects are thread-safe; the layer is
// only touched under the `Explorer`'s mutex (by contract from the main thread), so no two
// threads ever use any of them at the same time.
unsafe impl Send for Inner {}

/// The interactive renderer for one view.
#[derive(uniffi::Object)]
pub struct Explorer {
    inner: Mutex<Inner>,
}

/// Logs every Rust panic (message, location, backtrace) to
/// `~/Library/Logs/Almondbread/panic.log` before the default hook runs. A panic inside an FFI
/// call becomes a Swift trap; an app started from Finder has no stderr, so without this the
/// cause is lost when a crash report is not written.
fn install_panic_log() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if let Some(home) = std::env::var_os("HOME") {
                let dir = std::path::Path::new(&home).join("Library/Logs/Almondbread");
                let _ = std::fs::create_dir_all(&dir);
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join("panic.log"))
                {
                    use std::io::Write;
                    let when = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    let bt = std::backtrace::Backtrace::force_capture();
                    let _ = writeln!(
                        f,
                        "=== {} (unix {when}) pid {} thread {:?}\n{info}\n{bt}",
                        env!("CARGO_PKG_VERSION"),
                        std::process::id(),
                        std::thread::current().name()
                    );
                }
            }
            previous(info);
        }));
    });
}

impl Explorer {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic inside a call (caught by UniFFI) must not brick the view.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The cosmic size scale's state (`crate::cosmic`).
    pub(crate) fn with_cosmic<R>(&self, f: impl FnOnce(&mut almondbread_render::cosmic::CosmicOverlay) -> R) -> R {
        f(&mut self.lock().host.overlay.cosmic)
    }
}

#[uniffi::export]
impl Explorer {
    /// Creates the renderer for the `CAMetalLayer` at `layer` (its address, e.g.
    /// `Unmanaged.passUnretained(layer).toOpaque()`), which Rust retains and configures
    /// (device, BGRA8, not framebuffer-only). The caller guarantees `layer` points to a live
    /// `CAMetalLayer`. `scale` is the backing scale (pixels per point). Compiles the
    /// kernels: create one per window, not per view update.
    #[uniffi::constructor]
    pub fn new(layer: u64, scale: f64) -> Result<Arc<Self>, CoreError> {
        install_panic_log();
        if layer == 0 {
            return Err(invalid("layer must not be null"));
        }
        let backend = MetalBackend::new().map_err(gpu)?;
        // SAFETY: the caller guarantees a live CAMetalLayer at `layer` (documented above);
        // `from_raw` retains it.
        let surface = unsafe { MetalLayerSurface::from_raw(layer as usize as *mut c_void, &backend) }
            .ok_or_else(|| invalid("layer must not be null"))?;
        surface.set_contents_scale(scale);
        let renderer = Renderer::new(backend).map_err(gpu)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner {
                host: Host::new(renderer),
                surface,
                log: None,
                bench: None,
            }),
        }))
    }

    /// The view is `width` x `height` points at `scale` pixels per point (backing scale).
    /// Sets the layer's drawable size.
    pub fn resize(&self, width: f64, height: f64, scale: f64) {
        let mut g = self.lock();
        let (w, h) = g.host.resize(width, height, scale, media_time());
        g.surface.set_contents_scale(scale);
        if w > 0 && h > 0 {
            g.surface.set_drawable_size(w, h);
        }
    }

    /// One display refresh at `timestamp` (the display link's, `CACurrentMediaTime` clock;
    /// Rust uses the current time if that is later): integrates held keys, edge panning and
    /// the crosshair glide, schedules GPU work, presents. Returns whether the display link
    /// should keep running (bit 1: `FRAME_ANIMATING`; when clear, pause it until the next
    /// `input` asks for frames) and the input mode (bit 2: `FRAME_KEYBOARD`: show the
    /// crosshair at `crosshair_x()`, `crosshair_y()`). Bits rather than a record: a UniFFI
    /// record is serialised through a heap buffer on every call (measured M2.5: 11–18 µs
    /// per frame in the window instead of 2–3 µs).
    pub fn frame(&self, timestamp: f64) -> u8 {
        let mut g = self.lock();
        let Inner {
            host,
            surface,
            log,
            bench,
        } = &mut *g;
        // A panic must not cross the FFI (UniFFI turns it into a Swift trap: the app dies, as
        // in "selected Lagoon and almondbread died"). The panic hook has logged it to
        // ~/Library/Logs/Almondbread/panic.log; the renderer drops its job and goes on.
        let frame = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            MetalBackend::pool(|| {
                if let Some(log) = log {
                    let shown: Vec<(u64, f64)> = log.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default();
                    for (tag, t) in shown {
                        host.on_presented(tag, t);
                        if let Some(b) = bench.as_mut() {
                            b.presented(t);
                        }
                    }
                }
                // macOS reports the last vsync as the timestamp, up to a refresh in the past:
                // scheduling on it started refinement a refresh late (measured +15 ms to the final
                // frame), so the later of it and the current time is "now".
                let now = media_time().max(if timestamp.is_finite() { timestamp } else { 0.0 });
                if let Some(b) = bench.as_mut() {
                    b.before(host, surface, log, now);
                }
                let ready = surface.drawable_ready(now);
                let outcome = host.frame(now, &media_time, ready, &mut |final_frame| {
                    surface
                        .next_drawable(now, final_frame)
                        .map(|target| (target, surface.last_tag()))
                });
                match bench.as_mut() {
                    Some(b) if !b.done() => {
                        b.after(host, surface, log, now, outcome.presented);
                        true
                    }
                    _ => outcome.animating,
                }
            })
        }));
        let animating = frame.unwrap_or_else(|_| {
            host.renderer.recover_after_panic();
            true
        });
        let keyboard = host.renderer.navigator.mode() == CoreInputMode::Keyboard;
        let autopilot = host.renderer.navigator.autopilot().is_active();
        (u8::from(animating) * FRAME_ANIMATING)
            | (u8::from(keyboard) * FRAME_KEYBOARD)
            | (u8::from(autopilot) * FRAME_AUTOPILOT)
    }

    /// The keyboard crosshair's position as of the last `frame`, view points from the centre
    /// (y up); 0 in pointer mode. It glides to the centre; the zoom follows it.
    pub fn crosshair_x(&self) -> f64 {
        self.lock().host.renderer.navigator.crosshair().map_or(0.0, |c| c.0)
    }

    pub fn crosshair_y(&self) -> f64 {
        self.lock().host.renderer.navigator.crosshair().map_or(0.0, |c| c.1)
    }

    /// Edge panning (pointer mode): off, while zooming (default) or always.
    pub fn set_edge_pan(&self, mode: EdgePan) {
        self.lock().host.renderer.navigator.set_edge_pan(mode.into());
    }

    pub fn edge_pan(&self) -> EdgePan {
        self.lock().host.renderer.navigator.edge_pan().into()
    }

    /// Autopilot target lock: while the pointer rests the autopilot aims at boundary targets
    /// and backs out of voids (on by default; off: a pure joystick; kb: decisions/controls
    /// "Autopilot"). `ALMONDBREAD_AUTOPILOT_LOCK=0|1` overrides the setting (A/B tests).
    pub fn set_autopilot_lock(&self, on: bool) {
        let on = match std::env::var("ALMONDBREAD_AUTOPILOT_LOCK").ok().as_deref() {
            Some("0") => false,
            Some("1") => true,
            _ => on,
        };
        self.lock().host.renderer.navigator.autopilot_mut().set_lock(on);
    }

    /// Forwards a decoded event. `handled`: swallow it; `redraw`: resume the display link.
    pub fn input(&self, event: InputEvent) -> InputResult {
        let mut g = self.lock();
        let Some(e) = event.to_core() else {
            return InputResult {
                handled: false,
                redraw: false,
                mode: g.host.renderer.navigator.mode().into(),
            };
        };
        let host = &mut g.host;
        let o = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.input(e, media_time()))).unwrap_or(
            almondbread_render::host::InputOutcome {
                handled: false,
                redraw: true,
            },
        );
        InputResult {
            handled: o.handled,
            redraw: o.redraw,
            mode: g.host.renderer.navigator.mode().into(),
        }
    }

    /// The defaults (the default look, automatic iterations, flat interior, ...): what
    /// "Reset" in the control center goes back to.
    pub fn default_settings(&self) -> ExplorerSettings {
        ExplorerSettings::from(&RenderSettings::default())
    }

    pub fn settings(&self) -> ExplorerSettings {
        ExplorerSettings::from(&self.lock().host.renderer.settings)
    }

    /// Applies a look (palette and colouring, see `looks()`); returns the new settings for
    /// the inspector. Unknown ids change nothing.
    pub fn apply_look(&self, id: String) -> ExplorerSettings {
        let mut g = self.lock();
        if let Some(look) = almondbread_core::palette::Look::named(&id) {
            look.apply(&mut g.host.renderer.settings);
        }
        ExplorerSettings::from(&g.host.renderer.settings)
    }

    /// Replaces the settings (a frame follows: resume the display link).
    pub fn set_settings(&self, settings: ExplorerSettings) -> Result<(), CoreError> {
        let mut g = self.lock();
        let mut s = g.host.renderer.settings.clone();
        settings.apply(&mut s)?;
        g.host.renderer.settings = s;
        Ok(())
    }

    /// Applies a library palette with its recommended mapping (density, offset, log) and deep
    /// colour as interior and outline colour; the look's lighting stays. Returns the new
    /// settings. Unknown ids change nothing.
    pub fn apply_palette(&self, id: String) -> ExplorerSettings {
        let mut g = self.lock();
        let s = &mut g.host.renderer.settings;
        if let Some(p) = almondbread_core::palette_library::find(&id) {
            p.apply(s);
            s.palette_stops = None;
        } else if almondbread_core::palette::Palette::all().iter().any(|p| p.id == id) {
            s.palette_id = id;
            s.palette_stops = None;
        }
        ExplorerSettings::from(&*s)
    }

    /// Uses a palette given as stops (an imported or custom one, e.g. from a palette
    /// library) instead of the built-in `palette_id`; empty: back to the built-in one.
    pub fn set_custom_palette(&self, stops: Vec<crate::PaletteStop>) {
        let mut g = self.lock();
        g.host.renderer.settings.palette_stops = if stops.is_empty() {
            None
        } else {
            Some(
                stops
                    .iter()
                    .map(|s| (s.position.clamp(0.0, 1.0), s.rgb & 0xFF_FFFF))
                    .collect(),
            )
        };
    }

    pub fn overlay(&self) -> OverlaySettings {
        OverlaySettings::from(&self.lock().host.overlay.settings)
    }

    /// Switches the orbit overlay and mini-map (a frame follows: resume the display link).
    pub fn set_overlay(&self, settings: OverlaySettings) {
        self.lock().host.overlay.settings = settings.to_core();
    }

    /// A click or tap at (x, y) view points from the centre (y up): on a mini-map panel it
    /// jumps there keeping the zoom (snapping to a nearby minibrot when the map is much
    /// coarser than the view; `orbit_info().map_note` says what was done) and returns true,
    /// so the platform does not treat it as a drag. `fly`: Option/Alt held.
    pub fn map_click(&self, x: f64, y: f64, fly: bool) -> bool {
        if !(x.is_finite() && y.is_finite()) {
            return false;
        }
        self.lock().host.map_click(x, y, fly)
    }

    /// Switches the π meter (its panel, the line of approach and its counting thread);
    /// `inset_right` / `inset_bottom`: extra margins in points keeping platform buttons clear.
    /// A frame follows: resume the display link.
    pub fn set_pi_meter(&self, on: bool, inset_right: f64, inset_bottom: f64) {
        let mut g = self.lock();
        let m = &mut g.host.overlay.pi;
        m.enabled = on;
        m.inset = [inset_right.clamp(0.0, 2000.0), inset_bottom.clamp(0.0, 2000.0)];
    }

    pub fn pi_meter(&self) -> PiMeterInfo {
        let g = self.lock();
        let m = &g.host.overlay.pi;
        let s = m.state();
        PiMeterInfo {
            enabled: m.enabled,
            point: s.point,
            eps: s.eps,
            count: s.count,
            count_eps: s.count_eps,
            product: s.product,
            digits: s.digits,
            counting: s.counting,
            progress: s.progress,
            too_deep: s.too_deep,
        }
    }

    /// The guided π flight: jumps to the place at `point` ("neck" or "cusp"), switches the
    /// meter on and flies down the line of approach to ε = 1e-8 (neck) or 1e-16 (cusp), the
    /// parabolic point held at the same place on screen (an exponential zoom about it at the
    /// zoom speed: Q / E change it, Esc or any move stops). Returns the place's settings.
    /// Resume the display link after this.
    pub fn pi_flight(&self, point: String) -> Result<ExplorerSettings, CoreError> {
        use almondbread_core::pi_moment::PiPoint;
        let p = PiPoint::named(&point).ok_or_else(|| invalid(format!("unknown π point {point}")))?;
        let settings = self.go_to_place(p.place_id().into())?;
        let mut g = self.lock();
        g.host.renderer.navigator.cancel_flight();
        let start =
            almondbread_core::pi_moment::target(&g.host.renderer.viewport).map_or(p.flight_eps() * 1e6, |t| t.eps);
        g.host.overlay.pi.start_flight(p, start, p.flight_eps());
        g.host.renderer.note_interaction(media_time());
        Ok(settings)
    }

    /// Writes the shown image with its overlays as a PNG (drawable pixels; screenshots of
    /// background windows for documentation). False before the first frame or on error.
    pub fn capture_png(&self, path: String) -> bool {
        let mut g = self.lock();
        let Some((pixels, w, h)) = MetalBackend::pool(|| g.host.renderer.capture_shown()) else {
            return false;
        };
        write_png_bgra(&path, &pixels, w, h).is_ok()
    }

    pub fn orbit_info(&self) -> OrbitInfo {
        use almondbread_core::orbit::OrbitClass;
        let g = self.lock();
        let o = &g.host.overlay;
        let class = o.orbit_class();
        let note = o
            .note
            .as_ref()
            .filter(|(_, t)| media_time() - t < 4.0)
            .map(|(n, _)| n.clone())
            .unwrap_or_default();
        OrbitInfo {
            text: class.map(OrbitClass::text).unwrap_or_default(),
            iterates: o.orbit_points().view.len() as u32,
            last_abs: o.orbit_points().last_abs,
            period: match class {
                Some(OrbitClass::Interior { period, .. }) => *period as u64,
                _ => 0,
            },
            escaped_at: match class {
                Some(OrbitClass::Escaped { iteration }) => *iteration as u64,
                _ => 0,
            },
            map_note: note,
        }
    }

    pub fn stats(&self) -> ExplorerStats {
        let g = self.lock();
        let r = &g.host.renderer;
        let s = &r.stats;
        ExplorerStats {
            shown: ViewportInfo::from(s.viewport.as_ref().unwrap_or(&r.viewport)),
            max_iterations: if s.viewport.is_some() {
                s.max_iterations as u64
            } else {
                r.max_iterations() as u64
            },
            gpu_time: s.gpu_time,
            chunks: s.chunks as u64,
            render_width: s.render_size.0 as u32,
            render_height: s.render_size.1 as u32,
            is_rendering: s.is_rendering,
            is_preview: s.is_preview,
            reference_time: s.reference_time,
            reference_length: s.reference_length as u64,
            reference_period: s.reference_period.unwrap_or(0) as u64,
            reference_bits: s.reference_bits as u32,
            reference_pending: s.reference_pending,
            deep: s.deep,
            zoom_speed: r.navigator.zoom_speed(),
            flight: {
                let n = &r.navigator;
                let f = n.flight();
                let throttle = n.autopilot().throttle();
                FlightStatus {
                    active: f.is_some(),
                    progress: f.map_or(0.0, |f| f.progress()),
                    speed: n.fly_speed(),
                    throttle,
                    hint: if f.is_some() && throttle < 0.85 {
                        if s.reference_pending || r.waiting_reference() {
                            AutopilotHint::Reference
                        } else {
                            AutopilotHint::Rendering
                        }
                    } else {
                        AutopilotHint::None
                    },
                    stopping: f.is_some_and(|f| f.stopping),
                    target_zoom_log10: f.map_or(0.0, |f| f.path.target().zoom_log10()),
                }
            },
            autopilot: {
                let a = r.navigator.autopilot();
                AutopilotStatus {
                    active: a.is_active(),
                    paused: a.is_paused(),
                    speed: a.speed(),
                    rate: a.rate(),
                    throttle: a.throttle(),
                    safety: a.safety(),
                    hint: a.hint().into(),
                    lock: a.lock_enabled(),
                    lock_label: a.lock_label(),
                }
            },
            journey: {
                let j = r.navigator.journey();
                let t = j.current();
                JourneyStatus {
                    active: j.is_active(),
                    suspended: j.is_suspended(),
                    seed: j.seed(),
                    daily: j.is_daily(),
                    leg: j.leg(),
                    phase: if j.is_planning() {
                        "planning"
                    } else {
                        match j.phase() {
                            almondbread_core::journey::Phase::Flying => "flying",
                            almondbread_core::journey::Phase::Dwelling => "resting",
                            almondbread_core::journey::Phase::Planning => "planning",
                        }
                    }
                    .to_string(),
                    kind: t.map_or(String::new(), |t| t.kind.name().to_string()),
                    label: t.map_or(String::new(), |t| t.label.clone()),
                    target_zoom_log10: t.map_or(0.0, |t| t.zoom_log10()),
                    speed: j.speed(),
                }
            },
            held_keys: r.navigator.held_count() as u32,
            settled: r.is_settled(),
            failed_frames: r.failed_frames as u64,
        }
    }

    /// The current view (it may be ahead of the shown frame).
    pub fn viewport(&self) -> ViewportInfo {
        ViewportInfo::from(&self.lock().host.renderer.viewport)
    }

    /// Jumps to a view; `re`, `im` are decimal strings (any number of digits).
    pub fn set_viewport(&self, re: String, im: String, scale: f64) -> Result<(), CoreError> {
        if !(scale.is_finite() && scale > 0.0) {
            return Err(invalid("viewport must be finite with a positive scale"));
        }
        self.set_viewport_text(re, im, format!("{scale:e}"))
    }

    /// Jumps to a view given entirely as text: `scale` may be beyond f64 ("2.5e-1000").
    pub fn set_viewport_text(&self, re: String, im: String, scale: String) -> Result<(), CoreError> {
        let center = Center::parse(&re, &im).ok_or_else(|| CoreError::Malformed {
            text: format!("{re}, {im}"),
        })?;
        let scale = Scale::parse(&scale).ok_or_else(|| CoreError::Malformed { text: scale.clone() })?;
        if !center.re().is_finite() || !center.im().is_finite() {
            return Err(invalid("viewport must be finite"));
        }
        let mut v = Viewport::with_center(center, scale);
        v.scale = v
            .scale
            .max(v.min_scale())
            .min(Scale::from_f64(almondbread_core::viewport::MAX_SCALE));
        v.ensure_precision();
        let mut g = self.lock();
        g.host.renderer.navigator.cancel_flight();
        g.host.renderer.viewport = v;
        Ok(())
    }

    /// Parses coordinates as copied by `coordinates_text` (or `re im scale` lines) and jumps
    /// there. Returns false if the text is not a view.
    pub fn paste_coordinates(&self, text: String) -> bool {
        let mut re = None;
        let mut im = None;
        let mut scale = None;
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some("re"), Some(v)) => re = Some(v.to_string()),
                (Some("im"), Some(v)) => im = Some(v.to_string()),
                (Some("scale"), Some(v)) => scale = Some(v.to_string()),
                _ => {}
            }
        }
        let ok = match (re, im, scale) {
            (Some(re), Some(im), Some(scale)) => self.set_viewport_text(re, im, scale).is_ok(),
            _ => false,
        };
        // The optional `angle` line (degrees); without it the view comes upright.
        if ok {
            let a = almondbread_core::flyto::parse_angle(&text).unwrap_or(0.0);
            self.lock().host.renderer.viewport.angle = a;
        }
        ok
    }

    /// Flies to a view given as text (any depth): an animated zoom-out / pan / zoom-in path
    /// that lands exactly on it. Navigation input cancels, Esc eases to a stop, Q / E set the
    /// speed. Resume the display link after this.
    pub fn fly_to(&self, re: String, im: String, scale: String) -> Result<(), CoreError> {
        let target = almondbread_core::flyto::view_from_text(&re, &im, &scale).ok_or_else(|| CoreError::Malformed {
            text: format!("{re}, {im}, {scale}"),
        })?;
        let mut g = self.lock();
        let r = &mut g.host.renderer;
        let target = target.with_angle(r.viewport.angle);
        let start = r.viewport.clone();
        r.navigator.fly_to(&target, &start, media_time());
        Ok(())
    }

    /// Flies to coordinates as copied by `coordinates_text` (or "re im scale"). Returns false
    /// if the text is not a view.
    pub fn fly_to_coordinates(&self, text: String) -> bool {
        let Some((re, im, scale)) = almondbread_core::flyto::parse_coordinates(&text) else {
            return false;
        };
        let Some(target) = almondbread_core::flyto::view_from_text(&re, &im, &scale) else {
            return false;
        };
        let mut g = self.lock();
        let r = &mut g.host.renderer;
        // The text's angle; without one the view keeps its own.
        let target = target.with_angle(almondbread_core::flyto::parse_angle(&text).unwrap_or(r.viewport.angle));
        let start = r.viewport.clone();
        r.navigator.fly_to(&target, &start, media_time());
        true
    }

    /// Starts a journey with `seed` (kb: decisions/controls, "Journey"): planned legs from
    /// landmark to landmark, reproducible from the seed and the start view. Resume the
    /// display link after this.
    pub fn start_journey(&self, seed: u64) {
        let mut g = self.lock();
        let r = &mut g.host.renderer;
        let v = r.viewport.clone();
        r.navigator.start_journey(seed.max(1), false, &v, media_time());
    }

    /// The daily journey: the same for everyone on that date (seed yyyymmdd).
    pub fn start_daily_journey(&self, year: i32, month: u32, day: u32) {
        let seed = almondbread_core::journey::daily_seed(year, month, day);
        let mut g = self.lock();
        let r = &mut g.host.renderer;
        let v = r.viewport.clone();
        r.navigator.start_journey(seed, true, &v, media_time());
    }

    /// A random journey seed (1 ... 999 999).
    pub fn random_journey_seed(&self) -> u64 {
        almondbread_core::input::random_seed(media_time())
    }

    /// Stops a journey (its flight eases out).
    pub fn stop_journey(&self) {
        self.lock().host.renderer.navigator.stop_journey();
    }

    /// Resumes a journey that manual input took over.
    pub fn resume_journey(&self) {
        let mut g = self.lock();
        let r = &mut g.host.renderer;
        let v = r.viewport.clone();
        r.navigator.resume_journey(&v, media_time());
    }

    /// Turns the view to `degrees` (the camera's counter-clockwise turn) about the centre.
    pub fn set_angle(&self, degrees: f64) {
        if degrees.is_finite() {
            let mut g = self.lock();
            let v = &mut g.host.renderer.viewport;
            let d = almondbread_core::viewport::wrap_angle(degrees.to_radians()) - v.angle;
            v.rotate_by(d, 0.0, 0.0);
            g.host.renderer.note_interaction(media_time());
        }
    }

    /// The home view fitted to the window.
    pub fn reset_viewport(&self) {
        self.lock().host.reset_viewport();
    }

    /// Jumps to a catalogue place (`places()`, kb: synthesis/places): its centre, its span
    /// fitted to the view's shorter side, its look and iteration limit (automatic unless the
    /// place sets one). Returns the new settings for the inspector. A frame follows: resume
    /// the display link.
    pub fn go_to_place(&self, id: String) -> Result<ExplorerSettings, CoreError> {
        let place = almondbread_core::places::place_named(&id).ok_or_else(|| invalid(format!("unknown place {id}")))?;
        let mut g = self.lock();
        let (w, h) = g.host.view_points();
        let mut v = Viewport::with_center(place.center(), place.span_scale() / w.min(h).max(1.0));
        v.scale = v
            .scale
            .max(v.min_scale())
            .min(Scale::from_f64(almondbread_core::viewport::MAX_SCALE));
        v.ensure_precision();
        let renderer = &mut g.host.renderer;
        renderer.viewport = v;
        let s = &mut renderer.settings;
        if let Some(look) = almondbread_core::palette::Look::named(place.look) {
            look.apply(s);
        }
        match place.iterations {
            Some(n) => {
                s.auto_iterations = false;
                s.manual_iterations = n;
                s.max_iterations_cap = s.max_iterations_cap.max(n);
            }
            None => s.auto_iterations = true,
        }
        Ok(ExplorerSettings::from(&*s))
    }

    /// Flies to a catalogue place (the fly-to path), fitted to the view like `go_to_place`, and
    /// takes its look and iteration limit at once. Returns the new settings.
    pub fn fly_to_place(&self, id: String) -> Result<ExplorerSettings, CoreError> {
        let place = almondbread_core::places::place_named(&id).ok_or_else(|| invalid(format!("unknown place {id}")))?;
        let mut g = self.lock();
        let (w, h) = g.host.view_points();
        let mut v = Viewport::with_center(place.center(), place.span_scale() / w.min(h).max(1.0));
        v.scale = v
            .scale
            .max(v.min_scale())
            .min(Scale::from_f64(almondbread_core::viewport::MAX_SCALE));
        v.ensure_precision();
        let renderer = &mut g.host.renderer;
        let start = renderer.viewport.clone();
        renderer.navigator.fly_to(&v, &start, media_time());
        let s = &mut renderer.settings;
        if let Some(look) = almondbread_core::palette::Look::named(place.look) {
            look.apply(s);
        }
        match place.iterations {
            Some(n) => {
                s.auto_iterations = false;
                s.manual_iterations = n;
                s.max_iterations_cap = s.max_iterations_cap.max(n);
            }
            None => s.auto_iterations = true,
        }
        Ok(ExplorerSettings::from(&*s))
    }

    /// Centre and scale for the clipboard.
    pub fn coordinates_text(&self) -> String {
        self.lock().host.coordinates_text()
    }

    /// Renders the current view and settings offscreen at `width` x `height` pixels (the
    /// region the window shows) with a separate renderer on the same GPU, and writes a PNG.
    /// Uses the view's learned iteration limit and extension passes, so it looks like the
    /// refined window. Blocks; for self-tests and export.
    pub fn render_png(&self, path: String, width: u32, height: u32) -> Result<OffscreenInfo, CoreError> {
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(invalid("size must be in 1...16384"));
        }
        let (backend, settings, mut viewport, view_width) = {
            let g = self.lock();
            let r = &g.host.renderer;
            (
                r.backend().clone(),
                r.settings.clone(),
                r.viewport.clone(),
                g.host.view_points().0,
            )
        };
        if view_width > 0.0 {
            viewport.scale = viewport.scale * (view_width / width as f64);
        }
        MetalBackend::pool(|| {
            let mut r = Renderer::new(backend).map_err(gpu)?;
            // Like the view: unresolved pixels get the extension passes (offscreen renders
            // default to none, so verify diffs at one limit).
            r.config.offscreen_extension = r.config.extension_factor;
            r.settings = settings;
            r.viewport = viewport;
            let res = r
                .render_offscreen(width as usize, height as usize, false)
                .map_err(gpu)?;
            write_png_bgra(&path, &res.pixels, res.width, res.height).map_err(|e| invalid(format!("{path}: {e}")))?;
            Ok(OffscreenInfo {
                width,
                height,
                iterate_gpu_seconds: res.iterate_gpu_time,
                chunks: res.chunks as u64,
                wall_seconds: res.wall_time,
            })
        })
    }

    // Interaction tests

    /// Starts measuring frame pacing and latency (presentation times from the drawables).
    pub fn begin_measurement(&self) {
        let mut g = self.lock();
        let log = PresentationLog::default();
        g.surface.set_presentation_log(Some(log.clone()));
        g.log = Some(log);
        g.host.begin_measurement(media_time());
    }

    pub fn end_measurement(&self) -> InteractionReport {
        let mut g = self.lock();
        let Inner { host, surface, log, .. } = &mut *g;
        if let Some(l) = log.take() {
            let shown: Vec<(u64, f64)> = l.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
            for (tag, t) in shown {
                host.on_presented(tag, t);
            }
        }
        surface.set_presentation_log(None);
        host.end_measurement(media_time()).into()
    }

    /// Starts the flight benchmark (mode B, kb: synthesis/flight-benchmark): `script` is the
    /// flight's JSON, `hz` the display's maximum rate; `warmup` laps, then `laps` measured
    /// ones. Frames keep coming until it is done; `flight_bench_report` then returns the JSON.
    pub fn start_flight_bench(&self, script: String, hz: f64, laps: u32, warmup: u32) -> Result<(), CoreError> {
        let script =
            almondbread_core::flight::FlightScript::parse(&script).map_err(|e| CoreError::Malformed { text: e })?;
        if script.benchmark.is_none() {
            return Err(invalid("not a benchmark flight (no `benchmark` object)"));
        }
        self.lock().bench = Some(crate::flight_bench::FlightBench::new(
            script,
            hz,
            laps as usize,
            warmup as usize,
        ));
        Ok(())
    }

    /// The flight benchmark's JSON report once all laps are done (None before).
    pub fn flight_bench_report(&self) -> Option<String> {
        self.lock().bench.as_ref().and_then(|b| b.report.clone())
    }

    /// One line on the flight benchmark's progress ("" when none runs).
    pub fn flight_bench_progress(&self) -> String {
        self.lock().bench.as_ref().map(|b| b.progress()).unwrap_or_default()
    }

    pub fn final_frame(&self) -> FinalFrame {
        let g = self.lock();
        let r = &g.host.renderer;
        FinalFrame {
            time: r.last_final_frame.0,
            size: r.last_final_frame.1.clone(),
            last_input: r.navigator.last_interaction,
        }
    }

    /// Waits for the frames in flight; returns how many frames failed on the GPU so far.
    pub fn finish_frames(&self) -> u64 {
        self.lock().host.renderer.finish_frames() as u64
    }

    /// The shown frame against a fresh offscreen render of the same view (blocks for the
    /// render); None while a gesture or preview frame is shown.
    pub fn displayed_frame_mismatch(&self) -> Option<FrameMismatch> {
        let mut g = self.lock();
        MetalBackend::pool(|| g.host.renderer.displayed_frame_mismatch()).map(|m| FrameMismatch {
            pixels: m.pixels as u64,
            percent: m.percent,
            width: m.width as u32,
            height: m.height as u32,
        })
    }
}
