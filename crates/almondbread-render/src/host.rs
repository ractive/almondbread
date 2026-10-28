//! Hosting the renderer in an interactive view, platform-neutral (kb:
//! platform/render-architecture, "Hosting in the app"). A platform UI owns a [`Host`] and
//! makes three kinds of calls: `resize` on layout changes, `input` with decoded events, and
//! one `frame` per display refresh, which returns whether the display link should keep
//! running. Motion policy lives in [`almondbread_core::input::Navigator`], scheduling in
//! [`Renderer`]; this layer adds the view size, the first fit of the home view, and the
//! measurements of interaction tests (frame pacing, input-to-photon latency, draw CPU).
//!
//! Time: `now` arguments are seconds on one monotonic clock shared with the platform's
//! presentation timestamps (on Apple: `CACurrentMediaTime`, the display link's clock).

use std::collections::HashMap;

use almondbread_core::cfmt::g;
use almondbread_core::input::InputEvent;
use almondbread_core::journal::Step;
use almondbread_core::tour::{Snapshot, StepPlan, Tour, TourPlayer};
use almondbread_core::viewport::Viewport;

use crate::backend::GpuBackend;
use crate::overlay::Overlay;
use crate::scheduler::{FrameOutcome, FrameStats, FrameTarget, Renderer};

/// What an input event did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputOutcome {
    /// The event was ours (the platform swallows it: no system beep).
    pub handled: bool,
    /// The view needs frames now (start the display link).
    pub redraw: bool,
}

/// Frame pacing and latency of one measured interval (interaction tests). Times in ms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InteractionReport {
    /// Frames that reached the screen.
    pub presented: usize,
    pub fps: f64,
    pub interval_p50: f64,
    pub interval_p95: f64,
    pub interval_max: f64,
    /// Oldest unshown input to `presentedTime`.
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub input_to_draw_p50: f64,
    pub draw_to_present_p50: f64,
    /// Time spent inside `frame` (without the FFI call itself).
    pub draw_cpu_p50: f64,
    pub draw_cpu_p95: f64,
    pub draw_cpu_mean: f64,
    pub iterate_gpu_per_frame: f64,
    pub colorize_gpu_per_frame: f64,
    pub preview_pct: f64,
    /// Mean resolution of the preview frames (texels per pixel; 1: full resolution).
    pub preview_scale: f64,
    /// Refreshes that encoded nothing (frames in flight, or no drawable without blocking).
    pub skipped: usize,
    pub skipped_in_flight: usize,
    pub skipped_no_target: usize,
    /// `frame` calls and frames encoded.
    pub draws: usize,
    pub encoded: usize,
    /// Switches between the Buddhabrot look and the escape-time colouring.
    pub look_switches: usize,
}

struct Measurement {
    start: f64,
    draw_cpu: Vec<f64>,
    /// When each draw started (the worst-frames line, `ALMONDBREAD_FRAME_LOG=1`).
    draw_at: Vec<f64>,
    pending_input: Option<f64>,
    input_to_draw: Vec<f64>,
    /// Presentation tag -> (draw start, input the frame shows).
    frames: HashMap<u64, (f64, Option<f64>)>,
    presented: Vec<(u64, f64)>,
}

/// The renderer plus the view it draws into.
/// Committed command buffers the GPU may have queued before a refresh is skipped: normal
/// frames keep under ten (2 frames, 3 chunks, map levels, a survey); Metal blocks at 64.
pub const GPU_QUEUE_LIMIT: usize = 24;

pub struct Host<B: GpuBackend> {
    /// Refreshes skipped because the GPU queue was full ([`GPU_QUEUE_LIMIT`]).
    pub gpu_queue_skips: u64,
    /// Orbit overlay and mini-map (kb: decisions/controls, "Orbit overlay and mini-map").
    pub overlay: Overlay<B>,
    overlay_tick: u32,
    pub renderer: Renderer<B>,
    width: usize,
    height: usize,
    points_per_pixel: f64,
    view_points: (f64, f64),
    /// The backing scale the platform asked for (the drawable may be smaller: `max_texels`).
    requested_scale: f64,
    /// Output texels the memory budget allows (kb: platform/ios-device "Memory"): a larger view
    /// renders at a lower scale and the layer stretches it.
    max_texels: Option<usize>,
    fitted: bool,
    measure: Option<Measurement>,
    /// The guided tour being played (kb: decisions/controls "Guided tours").
    pub tour: Option<TourPlayer>,
    /// F4 (kb: plans/f4-deep-julia): the view and formula a Julia view was opened from (the
    /// way back); None when not in a Julia view opened here.
    pub julia_return: Option<(Viewport, almondbread_core::Formula)>,
    /// The Julia set the overlay's way back was made for.
    overlay_way_back_k: Option<almondbread_core::formula::JuliaK>,
    /// The last click was a ⌘-click that opened a Julia set: the rest of its click stream (⌘
    /// double clicks) does not zoom the new view.
    command_click_opened: bool,
}

impl<B: GpuBackend + Clone> Host<B> {
    pub fn new(renderer: Renderer<B>) -> Self {
        Self {
            renderer,
            width: 0,
            height: 0,
            points_per_pixel: 0.0,
            view_points: (0.0, 0.0),
            requested_scale: 1.0,
            max_texels: None,
            fitted: false,
            measure: None,
            gpu_queue_skips: 0,
            overlay: Overlay::default(),
            overlay_tick: 0,
            tour: None,
            julia_return: None,
            overlay_way_back_k: None,
            command_click_opened: false,
        }
    }

    /// The view is `width` x `height` points at `pixels_per_point`. Returns the drawable
    /// size in pixels. The first non-empty size replaces the default home view with one
    /// fitted to the view.
    pub fn resize(&mut self, width: f64, height: f64, pixels_per_point: f64, now: f64) -> (usize, usize) {
        let s = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
            pixels_per_point
        } else {
            1.0
        };
        // NaN gives 0 (max), +inf or absurd sizes a 16k-pixel cap: an unbounded size meant
        // usize::MAX texels and tile loops (kb: techniques/hang-prevention).
        let cap = 16384.0 / s;
        let (w, h) = (width.max(0.0).min(cap), height.max(0.0).min(cap));
        self.requested_scale = s;
        // Within the memory budget: the same view at fewer pixels.
        let s = match self
            .max_texels
            .and_then(|m| crate::memory::fit_texels((w * s).round() as usize, (h * s).round() as usize, m))
        {
            Some((_, f)) => s * f,
            None => s,
        };
        self.width = (w * s).round() as usize;
        self.height = (h * s).round() as usize;
        self.points_per_pixel = 1.0 / s;
        self.view_points = (w, h);
        self.renderer
            .handle_input(InputEvent::Resize { width: w, height: h }, now);
        if !self.fitted && self.width > 0 && self.height > 0 {
            self.fitted = true;
            if self.renderer.viewport == Viewport::home() {
                self.renderer.viewport = Viewport::fitting(w, h);
            }
        }
        (self.width, self.height)
    }

    /// Caps the output texels (the memory budget; None: no cap). Returns the new drawable size
    /// when the view had to change size.
    pub fn set_max_texels(&mut self, max_texels: Option<usize>, now: f64) -> Option<(usize, usize)> {
        if max_texels == self.max_texels {
            return None;
        }
        self.max_texels = max_texels;
        let before = (self.width, self.height);
        if before.0 == 0 || before.1 == 0 {
            return None;
        }
        let (w, h) = self.view_points;
        let after = self.resize(w, h, self.requested_scale, now);
        (after != before).then_some(after)
    }

    /// Drawable size in pixels.
    pub fn pixel_size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// View size in points.
    pub fn view_points(&self) -> (f64, f64) {
        self.view_points
    }

    /// Forwards a decoded event to the navigator.
    pub fn input(&mut self, event: InputEvent, now: f64) -> InputOutcome {
        // A finger going down: where it is decides what it may do (kb: decisions/controls
        // "Touch", "Rest or swipe"); the platform does not hit-test the overlays Rust draws.
        let event = match event {
            InputEvent::FingerDown { x, y, .. } => InputEvent::FingerDown {
                x,
                y,
                spot: self.touch_spot(x, y, now),
            },
            e => e,
        };
        // ⌘-click (kb: decisions/controls "⌘-click: open a point's Julia set"): the clicked
        // point's Julia set, where the formula offers the Julia panel; the navigator never sees
        // that click (no autopilot pause), and the further clicks of its stream do not zoom.
        match event {
            InputEvent::Click { x, y, command } => {
                self.command_click_opened = command && self.open_julia_at(x, y);
                if self.command_click_opened {
                    return InputOutcome {
                        handled: true,
                        redraw: true,
                    };
                }
            }
            InputEvent::DoubleTap { command: true, .. } if self.command_click_opened => {
                return InputOutcome {
                    handled: true,
                    redraw: false,
                };
            }
            _ => {}
        }
        let before = self.renderer.viewport.clone();
        let interaction = self.renderer.navigator.last_interaction;
        let mode = self.renderer.navigator.mode();
        // Seek and the home view follow the current formula (F3).
        self.renderer.navigator.set_formula(self.renderer.settings.formula);
        let handled = self.renderer.handle_input(event, now);
        // Focus lost (kb: decisions/controls "Held input"): a compass press lets go without its
        // click (no turn to north), as the navigator lets go of keys, pedals and fingers.
        if event == InputEvent::FocusLost {
            self.overlay.compass.release();
        }
        // Esc ends the π meter's guided flight too (other input ends it by moving the view).
        if matches!(event, InputEvent::KeyDown(almondbread_core::input::NavKey::Stop)) && self.overlay.pi.flying() {
            self.overlay.pi.stop_flight();
        }
        if self.renderer.navigator.last_interaction != interaction
            && let Some(m) = &mut self.measure
        {
            m.pending_input.get_or_insert(now);
        }
        let moved = self.renderer.viewport != before;
        // A mode change needs a frame too: the platform updates cursor and crosshair from
        // the frame's overlay state; edge panning may start on a pointer move.
        let nav = &self.renderer.navigator;
        // The orbit overlay and the Julia panel follow the pointer (or a touch's probe): every
        // move needs a frame.
        let orbit = (self.overlay.settings.orbit || self.overlay.settings.julia)
            && matches!(event, InputEvent::Pointer(_) | InputEvent::Probe(_));
        let redraw = handled
            && (moved
                || orbit
                || nav.wants_frames()
                || nav.mode() != mode
                || matches!(event, InputEvent::KeyDown(_) | InputEvent::Resize { .. }));
        InputOutcome { handled, redraw }
    }

    /// A click (or tap) at (x, y) view points from the centre, y up: on a mini-map panel it
    /// starts a jump there (resolved on a thread, applied by a later `frame`; `fly`: Option
    /// or Alt was held) and returns true (the platform then ignores the click otherwise).
    pub fn map_click(&mut self, x: f64, y: f64, fly: bool) -> bool {
        if self.points_per_pixel <= 0.0 {
            return false;
        }
        let ppp = self.points_per_pixel;
        let px = self.width as f64 / 2.0 + x / ppp;
        let py = self.height as f64 / 2.0 - y / ppp;
        let hit = self.overlay.click(&self.renderer, px, py, ppp, fly);
        // A click on the Mandelbrot panel of a Julia view: back to the view it shows (as ⇧U).
        if std::mem::take(&mut self.overlay.go_back) {
            self.back_from_julia();
        }
        // A click on the Julia panel: its set as the main view, whole (F4).
        if let Some(k) = self.overlay.open_julia.take() {
            let formula = almondbread_core::Formula::Julia(almondbread_core::formula::JuliaK::new(k.big()));
            let (w, h) = self.view_points;
            self.open_julia(formula, almondbread_core::julia::home_view(w, h));
        }
        hit
    }

    /// ⌘-click (kb: decisions/controls "⌘-click: open a point's Julia set"): opens the Julia set
    /// of the point at (x, y) view points from the centre, y up, whole, as a click on the Julia
    /// panel does: k is that point's c exactly (the view's centre precision), the view is kept
    /// as the way back. False (nothing done) where the formula has no Julia panel (a Multibrot or
    /// Mandelbar view, a Julia view) or before the first resize.
    pub fn open_julia_at(&mut self, x: f64, y: f64) -> bool {
        let f = self.renderer.settings.formula;
        if f.is_julia() || !almondbread_core::FormulaCapabilities::of(f).julia_panel {
            return false;
        }
        let (w, h) = self.view_points;
        if !(x.is_finite() && y.is_finite() && w > 0.0 && h > 0.0) {
            return false;
        }
        let k = self.renderer.viewport.point_at_offset(x, y);
        let formula = almondbread_core::Formula::Julia(almondbread_core::formula::JuliaK::new(k.big()));
        self.open_julia(formula, almondbread_core::julia::home_view(w, h));
        true
    }

    /// Shows `formula` (a Julia set) at `view` (kb: plans/f4-deep-julia), remembering the view
    /// it came from when that was not a Julia view (the way back). A jump: flights stop.
    pub fn open_julia(&mut self, formula: almondbread_core::Formula, mut view: Viewport) {
        let r = &mut self.renderer;
        if !r.settings.formula.is_julia() {
            self.julia_return = Some((r.viewport.clone(), r.settings.formula));
            // The Mandelbrot panel starts from this view's reference (a deep twin's is costly).
            self.overlay.way_back_reference = r.reference().filter(|x| x.formula == r.settings.formula).cloned();
        }
        // Every entry shows the way back, whatever the Julia panel's setting (kb:
        // decisions/controls "Mandelbrot panel").
        self.overlay.mandelbrot_panel = true;
        r.navigator.cancel_flight();
        r.navigator.jumped();
        view.ensure_precision();
        r.settings.formula = formula;
        r.viewport = view;
    }

    /// The Julia twin (F4): the Julia set of the view's centre at the same framing. False in a
    /// Julia view (its twin is the way back) or for a formula without the Julia panel's `z^2+k`.
    pub fn julia_twin(&mut self) -> bool {
        let f = self.renderer.settings.formula;
        if f.is_julia() || !almondbread_core::FormulaCapabilities::of(f).julia_panel {
            return false;
        }
        let (formula, view) = almondbread_core::julia::twin(&self.renderer.viewport);
        self.open_julia(formula, view);
        true
    }

    /// Back to the view a Julia view was opened from (F4); without one (a Julia view from a
    /// link), the whole Mandelbrot set. False when not in a Julia view.
    pub fn back_from_julia(&mut self) -> bool {
        if !self.renderer.settings.formula.is_julia() {
            return false;
        }
        // The way back is the journal's previous entry (it was recorded on the way in): an
        // ordinary step back, so ⇧⌘Z / ⌘Y returns to the Julia set; the view is the way back's,
        // bit for bit (kb: decisions/controls "Travel journal").
        if let Some((view, formula)) = self.julia_return.clone() {
            let r = &self.renderer;
            let (w, _) = self.view_points;
            let back = Step::Back;
            let same = r
                .navigator
                .journal()
                .peek(back, &r.viewport, r.settings.formula, w)
                .is_some_and(|e| {
                    almondbread_core::journal::same_place(
                        e,
                        &almondbread_core::journal::Entry::new(view.clone(), formula),
                        w,
                    )
                });
            if same && self.journal_step(back, self.renderer.navigator.clock()) {
                self.overlay.way_back_reference = None;
                self.julia_return = None;
                let r = &mut self.renderer;
                r.settings.formula = formula;
                r.navigator.set_formula(formula);
                r.viewport = view;
                return true;
            }
        }
        self.leave_julia()
    }

    /// Leaves a Julia view for the view it was opened from (the whole Mandelbrot set without
    /// one), a plain jump the journal records like any other: for callers that go on somewhere
    /// else at once (a pasted view, the formula picker). False when not in a Julia view.
    pub fn leave_julia(&mut self) -> bool {
        if !self.renderer.settings.formula.is_julia() {
            return false;
        }
        let (w, h) = self.view_points;
        let (view, formula) = self
            .julia_return
            .take()
            .unwrap_or_else(|| (Viewport::fitting(w, h), almondbread_core::Formula::Mandelbrot));
        self.overlay.way_back_reference = None;
        let r = &mut self.renderer;
        r.navigator.cancel_flight();
        r.navigator.jumped();
        r.settings.formula = formula;
        r.viewport = view;
        true
    }

    /// U in a Julia view: the Mandelbrot panel on or off (kb: decisions/controls "Mandelbrot
    /// panel"). False outside a Julia view (there U is the Julia panel's setting).
    pub fn toggle_mandelbrot_panel(&mut self) -> bool {
        if !self.renderer.settings.formula.is_julia() {
            return false;
        }
        self.overlay.mandelbrot_panel = !self.overlay.mandelbrot_panel;
        true
    }

    /// The Mandelbrot panel is on (in a Julia view; it shows from the next frame).
    pub fn mandelbrot_panel_on(&self) -> bool {
        self.renderer.settings.formula.is_julia() && self.overlay.mandelbrot_panel
    }

    /// The way back the Mandelbrot panel shows in a Julia view (the view it was opened from, or
    /// the whole Mandelbrot set as `back_from_julia` goes there), None outside one. Kept while
    /// nothing changed (the panel's key).
    fn update_way_back(&mut self) {
        let f = self.renderer.settings.formula;
        let Some(k) = f.julia_k() else {
            self.overlay.way_back = None;
            self.overlay.way_back_reference = None;
            return;
        };
        let (w, h) = self.view_points;
        let (view, formula) = self
            .julia_return
            .clone()
            .unwrap_or_else(|| (Viewport::fitting(w, h), almondbread_core::Formula::Mandelbrot));
        let same = self.overlay.way_back.as_ref().is_some_and(|b| {
            b.view == view && b.formula == formula && b.view_points == (w, h) && self.overlay_way_back_k == Some(k)
        });
        if !same {
            self.overlay_way_back_k = Some(k);
            self.overlay.way_back = Some(crate::mandelbrot_panel::WayBack {
                view,
                formula,
                view_points: (w, h),
                k: almondbread_core::viewport::Center::from_big((*k.value()).clone()),
            });
        }
    }

    /// The help badge (kb: decisions/controls "Help") under a press at (x, y) view points from
    /// the centre, y up: its topic, centre (same coordinates) and radius in points. The platform
    /// then shows the topic's popover there and gives the press to nothing else.
    pub fn help_hit(&self, x: f64, y: f64) -> Option<(crate::help::HelpTopic, (f64, f64), f64)> {
        let ppp = self.points_per_pixel;
        if ppp <= 0.0 {
            return None;
        }
        let px = self.width as f64 / 2.0 + x / ppp;
        let py = self.height as f64 / 2.0 - y / ppp;
        // Touch screens take a fingertip around the badge (kb: decisions/controls "Touch").
        let hit = if self.overlay.cockpit.touch {
            crate::help::hit_touch(&self.overlay.help, px, py, 1.0 / ppp)
        } else {
            crate::help::hit(&self.overlay.help, px, py, 1.0 / ppp)
        };
        hit.map(|b| self.help_badge_points(&b))
    }

    /// What a finger held still at (x, y) view points from the centre, y up, does (kb:
    /// decisions/controls "Touch"): the overlays say where it rests (a control, a mini-map,
    /// the picture), the navigator what that means in the current level.
    pub fn touch_hold(&self, x: f64, y: f64, now: f64) -> almondbread_core::TouchHold {
        self.renderer.navigator.touch_hold(self.touch_spot(x, y, now))
    }

    /// Where a finger at (x, y) view points from the centre, y up, rests: a control (help
    /// badge, cockpit), the compass, a mini-map or the picture (as drawn in the last frame).
    pub fn touch_spot(&self, x: f64, y: f64, now: f64) -> almondbread_core::TouchSpot {
        use almondbread_core::TouchSpot;
        let ppp = self.points_per_pixel;
        if ppp <= 0.0 {
            TouchSpot::Canvas
        } else if self.help_hit(x, y).is_some() || self.cockpit_hit(x, y).is_some() {
            TouchSpot::Control
        } else if self.overlay.compass.hits(x, y, now) {
            TouchSpot::Compass
        } else if self.overlay.map_hit(
            self.width as f64 / 2.0 + x / ppp,
            self.height as f64 / 2.0 - y / ppp,
            ppp,
        ) {
            TouchSpot::Map
        } else {
            TouchSpot::Canvas
        }
    }

    /// The cockpit control under a press at (x, y) view points from the centre, y up.
    pub fn cockpit_hit(&self, x: f64, y: f64) -> Option<&'static str> {
        let ppp = self.points_per_pixel;
        if ppp <= 0.0 {
            return None;
        }
        let px = self.width as f64 / 2.0 + x / ppp;
        let py = self.height as f64 / 2.0 - y / ppp;
        self.overlay.cockpit.hit(px, py)
    }

    /// Every cockpit control of the last scene: id, centre and half sizes of its hit cell, in
    /// view points from the centre, y up (UI tests).
    pub fn cockpit_controls(&self) -> Vec<CockpitControl> {
        let ppp = self.points_per_pixel;
        self.overlay
            .cockpit
            .controls()
            .into_iter()
            .map(|(id, x, y, hw, hh)| {
                (
                    id,
                    (
                        (x - self.width as f64 / 2.0) * ppp,
                        (self.height as f64 / 2.0 - y) * ppp,
                    ),
                    (hw * ppp, hh * ppp),
                )
            })
            .collect()
    }

    /// Every help badge of the last scene, in view points from the centre, y up (window tests).
    pub fn help_badges(&self) -> Vec<(crate::help::HelpTopic, (f64, f64), f64)> {
        self.overlay.help.iter().map(|b| self.help_badge_points(b)).collect()
    }

    /// The Rust panels of the last scene (id, x, y, width, height) in view points from the top
    /// left (UI tests).
    pub fn overlay_panels(&self) -> Vec<(&'static str, [f64; 4])> {
        let ppp = self.points_per_pixel;
        let f = |v: f32| f64::from(v) * ppp;
        self.overlay
            .panels
            .iter()
            .map(|(id, r)| (*id, [f(r[0]), f(r[1]), f(r[2] - r[0]), f(r[3] - r[1])]))
            .collect()
    }

    fn help_badge_points(&self, b: &crate::help::HelpBadge) -> (crate::help::HelpTopic, (f64, f64), f64) {
        let ppp = self.points_per_pixel;
        let x = (f64::from(b.centre[0]) - self.width as f64 / 2.0) * ppp;
        let y = (self.height as f64 / 2.0 - f64::from(b.centre[1])) * ppp;
        (b.topic, (x, y), f64::from(b.radius) * ppp)
    }

    /// A press at (x, y) view points from the centre, y up: on the visible compass it starts
    /// following the pointer (a drag turns the view, a click turns it to north on release) and
    /// returns true (the platform then gives the press, its drag and its release to the
    /// compass, and no double-click zoom; kb: decisions/controls "Compass").
    pub fn compass_press(&mut self, x: f64, y: f64, now: f64) -> bool {
        self.overlay.compass.press(x, y, now)
    }

    /// The pointer moved to (x, y) during a compass press: the needle follows it.
    pub fn compass_drag(&mut self, x: f64, y: f64, now: f64) {
        let turn = self.overlay.compass.drag_to(x, y);
        if turn != 0.0 {
            let r = &mut self.renderer;
            r.navigator.turn_view(turn, &mut r.viewport, now);
        }
    }

    /// The compass press ended: a click turns the view to north (eased).
    pub fn compass_release(&mut self, now: f64) {
        if self.overlay.compass.release() {
            self.renderer.navigator.turn_north(now);
        }
    }

    /// Overlay work before the renderer's frame: a resolved map click is applied (a jump
    /// keeping the zoom), the probe and maps follow the view, the scene is rebuilt if it
    /// changed. Cheap when the overlays are off.
    fn overlay_before(&mut self, now: f64) {
        self.overlay.poll_click(now);
        if let Some(j) = self.overlay.pending_jump.take() {
            let mut v =
                Viewport::with_center(j.center, self.renderer.viewport.scale).with_angle(self.renderer.viewport.angle);
            v.ensure_precision();
            if j.fly {
                // Option-click: the animated fly-to path (out, across, in) to the same zoom.
                let start = self.renderer.viewport.clone();
                self.renderer.navigator.fly_to(&v, &start, now);
            } else {
                self.renderer.navigator.note_travel();
                self.renderer.viewport = v;
                self.renderer.note_interaction(now);
            }
        }
        let turning = self.renderer.navigator.is_turning_north();
        self.overlay.compass.update(self.renderer.viewport.angle, turning, now);
        let aim = self.renderer.navigator.aim_marker();
        self.overlay.marker.update(aim.as_ref(), &self.renderer.viewport, now);
        // A step's goal counts from where a flight the tour started (or any fly-to) lands.
        if let Some(t) = &mut self.tour
            && self.renderer.navigator.flight().is_some()
        {
            t.set_goal_from(self.renderer.viewport.clone());
        }
        // A tour sets the orbit probe's point (fixed or moving) and its marker.
        match &self.tour {
            Some(t) => {
                self.overlay.tour.probe = t.probe_at(now);
                self.overlay.tour.marker = t.marker().cloned();
                self.overlay.tour.labels = t.labels().to_vec();
                self.overlay.tour.circle = t.circle().cloned();
            }
            None => self.overlay.tour = Default::default(),
        }
        self.update_way_back();
        if self.overlay.active()
            || self.overlay.compass.shown(now)
            || self.overlay.marker.visible()
            || self.overlay.tour.shown()
            || self.renderer.overlay.is_some()
            || self.renderer.progress.current().alpha > 0.0
        {
            let (w, h, ppp) = (self.width, self.height, self.points_per_pixel);
            self.overlay.prepare(&mut self.renderer, (w, h), ppp, now);
        } else {
            // Nothing drawn: no badge to press.
            self.overlay.help.clear();
            self.overlay.panels.clear();
        }
    }

    /// Overlay work after the frame: about 1.5 ms of orbit analysis, and the maps' renderers
    /// while the view is not in a gesture (they never take the gesture frame budget).
    fn overlay_after(&mut self, now: f64, outcome: &mut FrameOutcome) {
        outcome.animating |= self.overlay.cosmic.animating(now)
            || self.overlay.compass.animating(now)
            || self.overlay.marker.animating(now)
            || self.tour.as_ref().is_some_and(|t| t.animating(now))
            || self.renderer.progress.animating();
        if !self.overlay.settings.any() && !self.overlay.back_shown() {
            // The π meter alone: its count runs on its own thread; frames show its progress.
            outcome.animating |= self.overlay.busy();
            return;
        }
        self.overlay.step_probe(0.0015);
        // During a gesture (or the autopilot, which never stops moving) only every 8th refresh,
        // so a level still gets drawn; idle, every refresh.
        self.overlay_tick = self.overlay_tick.wrapping_add(1);
        if !self.renderer.navigator.is_interacting(now) || self.overlay_tick.is_multiple_of(8) {
            let settings = self.renderer.settings.clone();
            self.overlay.step_maps(&settings, now);
            self.overlay.mandelbrot.step(&settings, now);
        }
        outcome.animating |= self.overlay.busy();
    }

    // MARK: Guided tours (kb: decisions/controls "Guided tours")

    /// The settings a tour changes, as they are now.
    pub fn tour_snapshot(&self) -> Snapshot {
        Snapshot {
            settings: self.renderer.settings.clone(),
            overlay: self.overlay.settings.clone(),
            cosmic: self.overlay.cosmic.settings.enabled,
            pi_meter: self.overlay.pi.enabled,
            compass: self.overlay.compass.visibility.always,
        }
    }

    fn apply_tour_snapshot(&mut self, s: &Snapshot) {
        // What the renderer learned about iterations stays.
        let learned = self.renderer.settings.learned_iterations;
        self.renderer.settings = s.settings.clone();
        self.renderer.settings.learned_iterations = learned;
        self.overlay.settings = s.overlay.clone().sanitized();
        self.overlay.cosmic.settings.enabled = s.cosmic;
        self.overlay.pi.enabled = s.pi_meter;
        self.overlay.compass.visibility.always = s.compass;
    }

    fn apply_tour_plan(&mut self, plan: StepPlan, now: f64) {
        // A π hunt runs only during its own step.
        if self.overlay.pi.flying() {
            self.overlay.pi.stop_flight();
        }
        self.apply_tour_snapshot(&plan.snapshot);
        if let Some(point) = plan.pi_hunt {
            let (w, h) = self.view_points;
            self.overlay
                .start_pi_flight(&mut self.renderer, point, w.min(h).max(1.0), now);
        }
        if let Some((v, jump)) = plan.view {
            let (w, h) = self.view_points;
            let target = v.viewport(w.min(h).max(1.0));
            // `move: jump`, or a flight longer than ~5 s (from deep down a tour's first step
            // flew for minutes): a jump.
            let r = &mut self.renderer;
            let start = r.viewport.clone();
            if jump {
                r.navigator.jumped();
            }
            if jump || !r.navigator.travel_to(&target, &start, now) {
                r.viewport = target;
                r.note_interaction(now);
            }
        }
    }

    /// Starts `tour` at its first step. Like a jump it stops other motion (autopilot, cruise,
    /// flight, journey, the π hunt); the current settings and view are saved for the end.
    pub fn start_tour(&mut self, tour: Tour, now: f64) {
        // A tour started during another ("Next tour" on the last step): the first one's saved
        // settings and view are the user's, and the end goes back to them; the view stays where
        // it is, so the next tour flies on from here (kb: decisions/controls "Finding the tours").
        let mut saved_view = None;
        if let Some(old) = self.tour.take() {
            let (s, v) = old.finish();
            self.apply_tour_snapshot(&s);
            self.overlay.tour = Default::default();
            saved_view = Some(v);
        }
        self.renderer.navigator.jumped();
        if self.overlay.pi.flying() {
            self.overlay.pi.stop_flight();
        }
        let view = saved_view.unwrap_or_else(|| self.renderer.viewport.clone());
        // The tour's fractal (F2; the user's comes back with the saved settings at the end).
        let formula = tour.formula;
        let (player, plan) = TourPlayer::start(tour, self.tour_snapshot(), view, now);
        self.renderer.settings.formula = formula;
        self.tour = Some(player);
        self.apply_tour_plan(plan, now);
        self.tour_goal_from_here();
    }

    /// Goes to step `index` of the running tour (clamped). False without a tour.
    pub fn tour_go(&mut self, index: usize, now: f64) -> bool {
        let Some(t) = &mut self.tour else { return false };
        let plan = t.go(index, now);
        self.apply_tour_plan(plan, now);
        self.tour_goal_from_here();
        true
    }

    /// The step's goal counts from the view as it is now.
    fn tour_goal_from_here(&mut self) {
        let v = self.renderer.viewport.clone();
        if let Some(t) = &mut self.tour {
            t.set_goal_from(v);
        }
    }

    /// The running step's goal (`goal:`) is done: false without a tour, a goal, or while a
    /// flight is under way (the goal counts from where it lands).
    pub fn tour_goal_met(&self) -> bool {
        self.renderer.navigator.flight().is_none()
            && self.tour.as_ref().is_some_and(|t| t.goal_met(&self.renderer.viewport))
    }

    /// Ends the tour: the saved settings come back and the view flies back to where it was
    /// (jumps when the flight would take over ~5 s). False without a tour.
    pub fn end_tour(&mut self, now: f64) -> bool {
        let Some(t) = self.tour.take() else { return false };
        if self.overlay.pi.flying() {
            self.overlay.pi.stop_flight();
        }
        let (saved, view) = t.finish();
        self.apply_tour_snapshot(&saved);
        self.overlay.tour = Default::default();
        let r = &mut self.renderer;
        let start = r.viewport.clone();
        r.navigator.jumped();
        // Back by air when that takes at most ~5 s, else a jump.
        if !r.navigator.travel_to(&view, &start, now) {
            r.viewport = view;
            r.note_interaction(now);
        }
        true
    }

    /// Clamps the view into the view limits of the current formula and size (kb:
    /// decisions/controls "View limits"): a view set from outside the navigator, at once; the
    /// navigator's own rubber band is left to spring back. True if the view changed.
    pub fn keep_in_limits(&mut self) -> bool {
        let r = &mut self.renderer;
        r.navigator.set_formula(r.settings.formula);
        r.navigator.keep_in_limits(&mut r.viewport)
    }

    /// The home view fitted to the view (a Julia set's whole view in a Julia view, F4).
    pub fn reset_viewport(&mut self) {
        let (w, h) = self.view_points;
        self.renderer.navigator.cancel_flight();
        self.renderer.navigator.note_travel();
        self.renderer.viewport = Viewport::fitting_for(self.renderer.settings.formula, w, h);
    }

    /// The travel journal (kb: decisions/controls "Travel journal"): back (⌘Z) or forward (⇧⌘Z,
    /// ⌘Y) to the entry, flying when that takes at most `TRAVEL_FLY_MAX_SECONDS`, else jumping;
    /// into another fractal a jump (into a Julia set from a Mandelbrot view, that view becomes its
    /// way back, as `open_julia` keeps it). False when there is nothing to go to. The settings'
    /// formula may change: the platform pulls the settings.
    pub fn journal_step(&mut self, step: almondbread_core::journal::Step, now: f64) -> bool {
        let r = &mut self.renderer;
        r.navigator.set_formula(r.settings.formula);
        let before = (r.viewport.clone(), r.settings.formula);
        let reference = r.reference().filter(|x| x.formula == before.1).cloned();
        let Some(changed) = r
            .navigator
            .journal_go(step, &mut r.viewport, &mut r.settings.formula, now)
        else {
            return false;
        };
        r.note_interaction(now);
        if changed {
            let f = self.renderer.settings.formula;
            if f.is_julia() {
                if !before.1.is_julia() {
                    self.julia_return = Some(before);
                    self.overlay.way_back_reference = reference;
                }
                self.overlay.mandelbrot_panel = true;
            } else {
                self.julia_return = None;
                self.overlay.way_back_reference = None;
            }
        }
        true
    }

    /// The journal can step from the current view (menu items and buttons).
    pub fn journal_can(&self, step: almondbread_core::journal::Step) -> bool {
        let r = &self.renderer;
        r.navigator.journal_can(step, &r.viewport, r.settings.formula)
    }

    /// One display refresh. `now` is the refresh's timestamp, `clock` reads the same clock
    /// (for draw CPU and latency), `target_ready` says whether a target can be had without
    /// blocking (else the refresh is skipped before encoding), `acquire` returns the target
    /// and a presentation tag (see [`Host::on_presented`]) or None.
    pub fn frame(
        &mut self,
        now: f64,
        clock: &dyn Fn() -> f64,
        target_ready: bool,
        acquire: &mut dyn FnMut(bool) -> Option<(FrameTarget<B>, u64)>,
    ) -> FrameOutcome {
        let start = clock();
        // (The navigator's formula, F3: a change from the UI reaches it before any input.)
        self.renderer.navigator.set_formula(self.renderer.settings.formula);
        // A GPU far behind (an iPad simulator iterating 300 000 steps a pixel queued over 64
        // command buffers and `commandBuffer()` blocked the main thread for up to 25 s): skip
        // this refresh entirely until it catches up (kb: techniques/hang-prevention "GPU queue").
        if self.renderer.backend().submissions_in_flight() >= GPU_QUEUE_LIMIT {
            self.gpu_queue_skips += 1;
            return FrameOutcome {
                animating: true,
                presented: false,
            };
        }
        if let Some(m) = &mut self.measure
            && self.renderer.navigator.is_moving()
        {
            // Held keys move the view in this frame: input "arrives" now.
            m.pending_input.get_or_insert(start);
        }
        let mut tag = 0;
        self.overlay_before(now);
        // Views set from outside (restored, pasted, places, tours, map jumps, a formula or
        // window change) come into the view limits (kb: decisions/controls "View limits").
        self.keep_in_limits();
        let (w, h, ppp) = (self.width, self.height, self.points_per_pixel);
        let mut outcome = self
            .renderer
            .render_frame_gated(now, w, h, ppp, target_ready, &mut |final_frame| {
                acquire(final_frame).map(|(target, t)| {
                    tag = t;
                    target
                })
            });
        self.overlay_after(now, &mut outcome);
        if let Some(m) = &mut self.measure {
            m.draw_cpu.push(clock() - start);
            m.draw_at.push(start);
            if outcome.presented {
                let input = m.pending_input.take();
                if let Some(i) = input {
                    m.input_to_draw.push(start - i);
                }
                m.frames.insert(tag, (start, input));
            }
        }
        outcome
    }

    /// A drawable with this tag reached the screen at `time` (measurements only).
    pub fn on_presented(&mut self, tag: u64, time: f64) {
        if let Some(m) = &mut self.measure {
            m.presented.push((tag, time));
        }
    }

    pub fn is_measuring(&self) -> bool {
        self.measure.is_some()
    }

    /// Starts measuring frame pacing and latency.
    pub fn begin_measurement(&mut self, now: f64) {
        self.renderer.frame_stats = Some(FrameStats::default());
        self.measure = Some(Measurement {
            start: now,
            draw_cpu: Vec::new(),
            draw_at: Vec::new(),
            pending_input: None,
            input_to_draw: Vec::new(),
            frames: HashMap::new(),
            presented: Vec::new(),
        });
    }

    /// Ends the measurement started by [`Host::begin_measurement`]; frames presented after
    /// this are not counted.
    pub fn end_measurement(&mut self, now: f64) -> InteractionReport {
        let fs = self.renderer.frame_stats.take().unwrap_or_default();
        let Some(m) = self.measure.take() else {
            return InteractionReport::default();
        };
        let mut shown: Vec<f64> = Vec::new();
        let mut latency = Vec::new();
        let mut draw_to_present = Vec::new();
        for &(tag, t) in &m.presented {
            let Some(&(draw, input)) = m.frames.get(&tag) else {
                continue;
            };
            shown.push(t);
            draw_to_present.push(t - draw);
            if let Some(i) = input {
                latency.push(t - i);
            }
        }
        shown.sort_by(f64::total_cmp);
        let intervals: Vec<f64> = shown.windows(2).map(|w| w[1] - w[0]).collect();
        if std::env::var("ALMONDBREAD_FRAME_LOG").is_ok_and(|v| v == "1") {
            // Where the hitches are: the longest gaps between presented frames and the slowest
            // draws, with their time since the measurement started (interaction tests).
            let t0 = shown.first().copied().unwrap_or(m.start).min(m.start);
            let mut gaps: Vec<(f64, f64)> = shown.windows(2).map(|w| (w[1] - w[0], w[0] - t0)).collect();
            gaps.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut draws: Vec<(f64, f64)> = m.draw_cpu.iter().zip(&m.draw_at).map(|(&c, &a)| (c, a - t0)).collect();
            draws.sort_by(|a, b| b.0.total_cmp(&a.0));
            let fmt = |v: &[(f64, f64)]| {
                v.iter()
                    .take(5)
                    .map(|(d, t)| format!("{:.1}@{:.2}s", d * 1000.0, t))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            println!(
                "frame-log: longest gaps ms@t {} | slowest draws ms@t {}",
                fmt(&gaps),
                fmt(&draws)
            );
        }
        let n = fs.frames.max(1) as f64;
        let duration = (now - m.start).max(1e-9);
        InteractionReport {
            presented: shown.len(),
            fps: shown.len() as f64 / duration,
            interval_p50: pct(&intervals, 0.5),
            interval_p95: pct(&intervals, 0.95),
            interval_max: pct(&intervals, 1.0),
            latency_p50: pct(&latency, 0.5),
            latency_p95: pct(&latency, 0.95),
            input_to_draw_p50: pct(&m.input_to_draw, 0.5),
            draw_to_present_p50: pct(&draw_to_present, 0.5),
            draw_cpu_p50: pct(&m.draw_cpu, 0.5),
            draw_cpu_p95: pct(&m.draw_cpu, 0.95),
            draw_cpu_mean: 1000.0 * m.draw_cpu.iter().sum::<f64>() / m.draw_cpu.len().max(1) as f64,
            iterate_gpu_per_frame: fs.chunk_gpu * 1000.0 / n,
            colorize_gpu_per_frame: fs.colorize_gpu * 1000.0 / n,
            preview_pct: 100.0 * fs.preview_frames as f64 / n,
            preview_scale: fs.preview_scale_sum / fs.preview_frames.max(1) as f64,
            skipped: fs.skipped,
            skipped_in_flight: fs.skipped_in_flight,
            skipped_no_target: fs.skipped_no_target,
            draws: m.draw_cpu.len(),
            encoded: fs.frames,
            look_switches: fs.look_switches,
        }
    }

    /// Centre and scale for the clipboard, at the precision the view needs (M3b: arbitrary;
    /// pasting it back gives the same view to a fraction of a point).
    pub fn coordinates_text(&self) -> String {
        coordinates_text_for(&self.renderer.viewport, self.renderer.settings.formula)
    }
}

/// A cockpit control in view points from the centre, y up: id, centre, half sizes.
pub type CockpitControl = (&'static str, (f64, f64), (f64, f64));

/// The clipboard text of a view: centre (full precision), scale, zoom and, when turned, angle.
pub fn coordinates_text_of(v: &Viewport) -> String {
    let (re, im) = v.center.to_strings_digits(coordinate_digits(v));
    let mut text = format!(
        "re {re}\nim {im}\nscale {} (zoom {})",
        v.scale.to_text(6),
        zoom_text(v.zoom_log10())
    );
    // A rotated view (kb: decisions/controls "View rotation"): degrees, counter-clockwise
    // turn of the camera. Unrotated views omit the line (older builds read the text too).
    if v.angle != 0.0 {
        text.push_str(&format!(
            "\nangle {}",
            almondbread_core::cfmt::g(v.angle.to_degrees(), 10)
        ));
    }
    text
}

/// [`coordinates_text_of`] with the fractal (F3): a `formula multibrotP` line for a Multibrot
/// set, F4's `julia re im` line for a Julia set (its k, to the centre's digits: k places the set
/// to them as well), nothing for Mandelbrot (whose text stays as older builds wrote and read it;
/// pasting text without either line means Mandelbrot, `flyto::parse_formula`).
pub fn coordinates_text_for(v: &Viewport, formula: almondbread_core::formula::Formula) -> String {
    let mut text = coordinates_text_of(v);
    if let Some(k) = formula.julia_k() {
        let d = coordinate_digits(v) + 2;
        let kv = k.value();
        text.push_str(&format!(
            "\njulia {} {}",
            kv.re.to_decimal_string(Some(d)),
            kv.im.to_decimal_string(Some(d))
        ));
    } else if !formula.is_mandelbrot() {
        text.push_str(&format!("\nformula {}", formula.name()));
    }
    text
}

/// Significant digits that place the centre of `v` to about 1/1000 of a point.
pub fn coordinate_digits(v: &Viewport) -> usize {
    let mag = v
        .center
        .re()
        .abs()
        .max(v.center.im().abs())
        .max(1e-300)
        .log10()
        .ceil()
        .max(0.0);
    ((mag - v.scale.log10()).ceil() as usize + 4).max(17)
}

/// "1.5e+05×" below 1e15, "10^123.4×" beyond.
pub fn zoom_text(log10: f64) -> String {
    if log10 < 15.0 {
        format!("{}×", g(10f64.powf(log10), 6))
    } else {
        format!("10^{log10:.1}×")
    }
}

/// Percentile in ms, picked like the Swift interaction test (`sorted[(n-1)·p]`); NaN if empty.
fn pct(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p) as usize] * 1000.0
}

#[cfg(test)]
mod tests {
    use super::pct;

    #[test]
    fn percentiles_match_the_swift_test() {
        let v = [0.004, 0.001, 0.003, 0.002];
        assert_eq!(pct(&v, 0.5), 2.0);
        assert_eq!(pct(&v, 1.0), 4.0);
        assert!(pct(&[], 0.5).is_nan());
    }
}

/// ⌘-click (kb: decisions/controls "⌘-click: open a point's Julia set") through a real host on
/// Metal: the clicked point's Julia set opens with k bit for bit that point's c, the way back is
/// the view as it was, and plain clicks, ⌥ double clicks and other formulas are unchanged.
#[cfg(all(test, target_vendor = "apple"))]
mod command_click_tests {
    use super::*;
    use crate::metal::MetalBackend;
    use almondbread_core::input::NavKey;
    use almondbread_core::viewport::{Center, Scale};
    use almondbread_core::{Formula, julia};

    const W: f64 = 400.0;
    const H: f64 = 250.0;

    fn host() -> Host<MetalBackend> {
        let backend = MetalBackend::new().expect("metal");
        let mut host = Host::new(Renderer::new(backend).expect("renderer"));
        host.resize(W, H, 2.0, 0.0);
        host
    }

    /// The view point (from the centre, y up) of pixel (px, py)'s centre on the 2x drawable.
    fn pixel_point(px: f64, py: f64) -> (f64, f64) {
        ((px + 0.5) / 2.0 - W / 2.0, H / 2.0 - (py + 0.5) / 2.0)
    }

    fn click(host: &mut Host<MetalBackend>, (x, y): (f64, f64), command: bool, now: f64) -> InputOutcome {
        host.input(InputEvent::Click { x, y, command }, now)
    }

    /// Shallow, turned, and turned at 1e-30 per point.
    fn views() -> Vec<Viewport> {
        let deep = Center::parse(
            "-0.743643887037158704752191506114774",
            "0.131825904205311970493132056385139",
        )
        .expect("centre");
        vec![
            Viewport::new(-0.7453, 0.1127, 6e-5),
            Viewport::new(-0.7453, 0.1127, 6e-5).with_angle(0.7),
            Viewport::with_center(deep, Scale::from_f64(1e-30)).with_angle(-1.1),
        ]
    }

    #[test]
    fn command_click_opens_exactly_the_clicked_point() {
        for (i, view) in views().into_iter().enumerate() {
            let mut h = host();
            h.renderer.viewport = view.clone();
            let p = pixel_point(517.0, 123.0);
            // The point's c computed here, independently of the host: the centre plus the
            // turned offset times the scale, at the centre's precision.
            let (rx, ry) = view.rotate(p.0, p.1);
            let expected = view.center.offset_scaled(rx, ry, view.scale);
            let o = click(&mut h, p, true, 1.0);
            assert!(o.handled && o.redraw, "view {i}");
            let k = h.renderer.settings.formula.julia_k().expect("a Julia view");
            let kv = k.value();
            assert_eq!(kv.re, expected.big().re, "view {i}: re bit for bit");
            assert_eq!(kv.im, expected.big().im, "view {i}: im bit for bit");
            assert_eq!(kv.re.precision_bits(), view.center.precision_bits(), "view {i}");
            // Not rounded to f64 at depth: k sits at the clicked offset to 1e-9 point.
            let (dx, dy) = Center::from_big((*kv).clone()).delta_in(&view.center, view.scale);
            assert!(
                (dx - rx).abs() < 1e-9 && (dy - ry).abs() < 1e-9,
                "view {i}: ({dx}, {dy})"
            );
            // The whole Julia set, and the way back is the view as it was (nothing moved
            // first).
            assert_eq!(h.renderer.viewport, julia::home_view(W, H), "view {i}");
            assert_eq!(h.julia_return, Some((view.clone(), Formula::Mandelbrot)), "view {i}");
            assert!(h.mandelbrot_panel_on(), "view {i}: the way back shows");
            assert!(h.back_from_julia());
            assert_eq!(h.renderer.viewport, view, "view {i}: back bit for bit");
            assert_eq!(h.renderer.settings.formula, Formula::Mandelbrot);
        }
    }

    #[test]
    fn plain_click_is_unchanged_and_command_click_stops_the_autopilot() {
        let mut h = host();
        let view = views()[1].clone();
        h.renderer.viewport = view.clone();
        h.input(InputEvent::KeyDown(NavKey::Autopilot), 0.0);
        h.input(InputEvent::KeyUp(NavKey::Autopilot), 0.0);
        assert!(h.renderer.navigator.autopilot().is_active());
        // A plain click pauses the autopilot; the view and the formula stay.
        let p = pixel_point(100.0, 400.0);
        click(&mut h, p, false, 1.0);
        assert!(h.renderer.navigator.autopilot().is_paused());
        assert_eq!(h.renderer.settings.formula, Formula::Mandelbrot);
        assert_eq!(h.renderer.viewport, view);
        click(&mut h, p, false, 2.0);
        assert!(!h.renderer.navigator.autopilot().is_paused());
        // A ⌘-click is no pause: it opens the Julia set, a jump that stops the autopilot.
        click(&mut h, p, true, 3.0);
        assert!(h.renderer.settings.formula.is_julia());
        assert!(!h.renderer.navigator.autopilot().is_active());
    }

    /// The zoom (old scale over new) after a double click at `p` and a second of glide.
    fn double_click(h: &mut Host<MetalBackend>, p: (f64, f64), out: bool, command: bool, now: f64) -> f64 {
        let before = h.renderer.viewport.scale;
        h.input(
            InputEvent::DoubleTap {
                x: p.0,
                y: p.1,
                out,
                command,
            },
            now,
        );
        let mut v = h.renderer.viewport.clone();
        h.renderer.navigator.advance(&mut v, now + 1.0 / 60.0);
        h.renderer.navigator.advance(&mut v, now + 1.0);
        h.renderer.viewport = v;
        before.ratio(h.renderer.viewport.scale)
    }

    #[test]
    fn command_double_click_opens_once_and_does_not_zoom_the_julia_set() {
        let mut h = host();
        h.renderer.viewport = views()[0].clone();
        let p = pixel_point(300.0, 200.0);
        click(&mut h, p, true, 1.0);
        let k = h.renderer.settings.formula;
        assert!(k.is_julia());
        // The second click of the ⌘ stream: nothing.
        let r = double_click(&mut h, p, false, true, 1.2);
        assert_eq!(r, 1.0);
        assert_eq!(h.renderer.viewport, julia::home_view(W, H));
        assert_eq!(h.renderer.settings.formula, k);
        // In the Julia view a ⌘-click opens nothing new (no Julia panel there): a ⌘ double
        // click zooms it like a double click.
        click(&mut h, p, true, 3.0);
        assert_eq!(h.renderer.settings.formula, k);
        let r = double_click(&mut h, p, false, true, 3.2);
        assert!((r - 2.0).abs() < 1e-9, "zoomed ×{r}");
    }

    /// View limits (kb: decisions/controls "View limits"): a view restored or set from outside
    /// beyond them is clamped (not rejected) to the formula's own limits (`Host::frame` does
    /// this before every frame).
    #[test]
    fn views_set_from_outside_come_into_the_limits() {
        for (f, view) in [
            (Formula::Mandelbrot, Viewport::new(-30.0, 9.0, 0.05)),
            (Formula::BurningShip, Viewport::new(-0.45, -4.0, 0.2)),
        ] {
            let mut h = host();
            h.renderer.settings.formula = f;
            h.renderer.navigator.jumped();
            h.renderer.viewport = view.clone();
            assert!(h.keep_in_limits(), "{f:?}");
            let home = Viewport::fitting_for(f, W, H);
            let v = &h.renderer.viewport;
            assert_eq!(
                v.scale,
                home.scale / almondbread_core::viewport::ZOOM_OUT_LIMIT,
                "{f:?}"
            );
            // Onto the set's own box (its nearest point).
            let b = almondbread_core::viewport::set_box(f);
            let (re, im) = b.clamp(view.center.re(), view.center.im());
            assert!(
                (v.center.re() - re).abs() < 1e-9 && (v.center.im() - im).abs() < 1e-9,
                "{f:?}"
            );
            // Inside: unchanged.
            assert!(!h.keep_in_limits());
        }
    }

    #[test]
    fn option_double_click_and_other_formulas_are_unchanged() {
        // ⌥ double click (out) after a plain click: ×0.5, as before.
        let mut h = host();
        h.renderer.viewport = views()[0].clone();
        let p = pixel_point(300.0, 200.0);
        click(&mut h, p, false, 1.0);
        let r = double_click(&mut h, p, true, false, 1.2);
        assert!((r - 0.5).abs() < 1e-9, "zoomed ×{r}");
        assert_eq!(h.renderer.settings.formula, Formula::Mandelbrot);
        // Formulas without the Julia panel: a ⌘-click is a click, a ⌘ double click zooms.
        for f in [Formula::multibrot(5).expect("z^5"), Formula::Mandelbar] {
            let mut h = host();
            h.renderer.settings.formula = f;
            let view = Viewport::new(0.1, 0.2, 1e-4);
            h.renderer.viewport = view.clone();
            click(&mut h, p, true, 1.0);
            assert_eq!(h.renderer.settings.formula, f);
            assert_eq!(h.renderer.viewport, view);
            assert!(h.julia_return.is_none());
            let r = double_click(&mut h, p, false, true, 1.2);
            assert!((r - 2.0).abs() < 1e-9, "{f:?} zoomed ×{r}");
        }
    }
}

/// The travel journal through the host (kb: decisions/controls "Travel journal"): real frames
/// (`Host::frame` without a drawable), real flows: a place, seek minibrot, ⌘-click into a Julia
/// set and the way back, a tour step, a paused scroll. State-based: each waits for the motion to
/// end (bounded by wall clock), not for a number of frames.
#[cfg(all(test, target_vendor = "apple"))]
mod journal_tests {
    use super::*;
    use crate::metal::MetalBackend;
    use almondbread_core::Formula;
    use almondbread_core::input::{NavKey, ScrollPhase};
    use almondbread_core::journal::{Entry, Step, far, same_place};

    const W: f64 = 400.0;
    const H: f64 = 250.0;

    struct Rig {
        h: Host<MetalBackend>,
        now: f64,
    }

    impl Rig {
        fn new() -> Self {
            let backend = MetalBackend::new().expect("metal");
            let mut h = Host::new(Renderer::new(backend).expect("renderer"));
            h.resize(W, H, 1.0, 0.0);
            let mut r = Self { h, now: 0.0 };
            // The start view at rest past the dwell: the journal's first entry.
            r.frames(2.5);
            r
        }

        fn frame(&mut self) {
            self.now += 1.0 / 60.0;
            let now = self.now;
            self.h.frame(now, &|| now, false, &mut |_| None);
        }

        fn frames(&mut self, seconds: f64) {
            for _ in 0..(seconds * 60.0).ceil() as usize {
                self.frame();
            }
        }

        /// Frames until nothing moves and no search runs (at most `limit` s of wall clock; a
        /// minibrot search is a thread).
        fn settle(&mut self, limit: f64) {
            let start = std::time::Instant::now();
            loop {
                self.frame();
                let n = &self.h.renderer.navigator;
                if !n.is_moving() && !n.wants_frames() {
                    break;
                }
                assert!(start.elapsed().as_secs_f64() < limit, "did not settle");
                if n.seek_status(self.now).0 == almondbread_core::seek::SeekPhase::Searching {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
            self.frame();
        }

        fn count(&self) -> (usize, usize) {
            self.h.renderer.navigator.journal().position()
        }

        fn view(&self) -> Viewport {
            self.h.renderer.viewport.clone()
        }

        fn entry(&self, i: usize) -> Entry {
            self.h.renderer.navigator.journal().entries()[i].clone()
        }

        /// As the FFI's `go_to_place`: a jump to the place in its fractal.
        fn go_to_place(&mut self, id: &str) -> Viewport {
            let place = almondbread_core::places::place_named(id).expect("place");
            let mut v = place.viewport(W as usize, H as usize);
            v.ensure_precision();
            let r = &mut self.h.renderer;
            r.navigator.jumped();
            r.viewport = v;
            r.settings.formula = place.formula;
            self.frame();
            self.view()
        }

        fn step(&mut self, step: Step) -> bool {
            let ok = self.h.journal_step(step, self.now);
            self.settle(20.0);
            ok
        }

        fn scroll(&mut self, dy: f64, phase: ScrollPhase) {
            self.h.input(InputEvent::Scroll { dx: 0.0, dy, phase }, self.now);
        }
    }

    #[test]
    fn a_place_back_and_forward() {
        let mut r = Rig::new();
        let home = r.view();
        assert_eq!(r.count(), (0, 1), "the start view after its dwell");
        let place = r.go_to_place("seahorse-valley");
        r.frames(0.2);
        assert_eq!(r.count(), (1, 2));
        assert!(r.h.journal_can(Step::Back) && !r.h.journal_can(Step::Forward));
        assert!(r.step(Step::Back));
        assert_eq!(r.view(), home, "lands exactly on the entry");
        assert_eq!(r.count(), (0, 2), "going back records nothing");
        assert!(!r.h.journal_can(Step::Back) && r.h.journal_can(Step::Forward));
        assert!(r.step(Step::Forward));
        assert_eq!(r.view(), place);
        assert_eq!(r.count(), (1, 2));
        assert!(!r.step(Step::Forward), "nothing ahead");
    }

    #[test]
    fn seek_minibrot_records_its_landing() {
        let mut r = Rig::new();
        r.h.input(InputEvent::KeyDown(NavKey::SeekMinibrot), r.now);
        r.h.input(InputEvent::KeyUp(NavKey::SeekMinibrot), r.now);
        r.settle(30.0);
        let landed = r.view();
        assert!(landed.zoom_log10() > 0.5, "flew to a minibrot: {}", landed.zoom_log10());
        assert_eq!(r.count(), (1, 2));
        assert_eq!(r.entry(1).view, landed);
        // Back flies (a short path) or jumps, and lands on the start view.
        let home = r.entry(0).view;
        assert!(r.step(Step::Back));
        assert_eq!(r.view(), home);
        assert_eq!(r.count(), (0, 2));
    }

    #[test]
    fn command_click_julia_and_the_way_back_are_journal_steps() {
        let mut r = Rig::new();
        r.h.renderer.viewport = Viewport::new(-0.7453, 0.1127, 6e-5);
        r.h.renderer.navigator.note_travel();
        r.frames(0.1);
        let view = r.view();
        assert_eq!(r.count(), (1, 2));
        r.h.input(
            InputEvent::Click {
                x: 30.0,
                y: -20.0,
                command: true,
            },
            r.now,
        );
        let k = r.h.renderer.settings.formula;
        assert!(k.is_julia());
        r.frames(0.1);
        assert_eq!(r.count(), (2, 3));
        assert_eq!(r.entry(2).formula, k);
        // ⇧U / the Mandelbrot panel: a step back, bit for bit.
        assert!(r.h.back_from_julia());
        assert_eq!(r.view(), view);
        assert_eq!(r.h.renderer.settings.formula, Formula::Mandelbrot);
        r.frames(0.1);
        assert_eq!(r.count(), (1, 3), "no new entry: the journal stepped back");
        // Forward opens the same Julia set again, with this view as its way back.
        assert!(r.step(Step::Forward));
        assert_eq!(r.h.renderer.settings.formula, k);
        assert_eq!(r.h.julia_return, Some((view.clone(), Formula::Mandelbrot)));
        assert!(r.h.mandelbrot_panel_on());
        // ⌘Z from the Julia set goes the same way.
        assert!(r.step(Step::Back));
        assert_eq!(r.view(), view);
        assert_eq!(r.h.renderer.settings.formula, Formula::Mandelbrot);
        assert!(r.h.julia_return.is_none());
        assert_eq!(r.count(), (1, 3));
    }

    #[test]
    fn a_tour_step_is_an_entry() {
        let mut r = Rig::new();
        let tour = almondbread_core::tour::named("feigenbaum").expect("the Feigenbaum tour");
        let steps = tour.steps.len();
        r.h.start_tour(tour, r.now);
        r.settle(60.0);
        // (Its first step frames the whole set: the same place as the start view, merged.)
        let mut moved = 0;
        for i in 1..steps {
            let (_, before) = r.count();
            let from = Entry::new(r.view(), r.h.renderer.settings.formula);
            r.h.tour_go(i, r.now);
            r.settle(120.0);
            let here = Entry::new(r.view(), r.h.renderer.settings.formula);
            if !same_place(&here, &from, W) {
                moved += 1;
                assert_eq!(r.count(), (before, before + 1), "step {i}");
                assert_eq!(r.entry(before), here, "step {i}");
            }
        }
        assert!(moved >= 2, "the tour moved {moved} times");
        assert!(r.h.journal_can(Step::Back));
    }

    #[test]
    fn a_scroll_paused_with_the_fingers_on_records_nothing() {
        let mut r = Rig::new();
        // Zoomed in (at the home view the view limits hold a pan back).
        r.h.renderer.viewport = Viewport::new(-0.7453, 0.1127, 1e-6);
        r.h.renderer.navigator.note_travel();
        r.frames(0.1);
        assert_eq!(r.count(), (1, 2));
        for _ in 0..20 {
            r.scroll(60.0, ScrollPhase::Fingers);
            r.frame();
        }
        // The fingers rest 4 s on the trackpad (mid-gesture), then go on.
        r.frames(4.0);
        assert_eq!(r.count(), (1, 2), "a pause with the fingers on is no rest");
        for _ in 0..5 {
            r.scroll(60.0, ScrollPhase::Fingers);
            r.frame();
        }
        r.scroll(0.0, ScrollPhase::Lifted);
        r.settle(10.0);
        r.frames(2.5);
        // Lifted and at rest for the dwell: an entry if the scroll went far.
        let away = far(&Entry::new(r.view(), Formula::Mandelbrot), &r.entry(1), W);
        assert!(away, "the scroll panned 1500 points");
        assert_eq!(r.count(), (2, 3));
    }
}

#[cfg(all(test, target_vendor = "apple"))]
#[path = "host_tour_tests.rs"]
mod tour_tests;

#[cfg(all(test, target_vendor = "apple"))]
#[path = "host_map_tests.rs"]
mod map_tests;
