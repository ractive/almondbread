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
pub struct Host<B: GpuBackend> {
    /// Orbit overlay and mini-map (kb: decisions/controls, "Orbit overlay and mini-map").
    pub overlay: Overlay<B>,
    overlay_tick: u32,
    pub renderer: Renderer<B>,
    width: usize,
    height: usize,
    points_per_pixel: f64,
    view_points: (f64, f64),
    fitted: bool,
    measure: Option<Measurement>,
}

impl<B: GpuBackend + Clone> Host<B> {
    pub fn new(renderer: Renderer<B>) -> Self {
        Self {
            renderer,
            width: 0,
            height: 0,
            points_per_pixel: 0.0,
            view_points: (0.0, 0.0),
            fitted: false,
            measure: None,
            overlay: Overlay::default(),
            overlay_tick: 0,
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
        let before = self.renderer.viewport.clone();
        let interaction = self.renderer.navigator.last_interaction;
        let mode = self.renderer.navigator.mode();
        let handled = self.renderer.handle_input(event, now);
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
        // The orbit overlay follows the pointer: every pointer move needs a frame then.
        let orbit = self.overlay.settings.orbit && matches!(event, InputEvent::Pointer(_));
        let redraw = handled
            && (moved
                || orbit
                || nav.is_animating()
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
        self.overlay.click(&self.renderer, px, py, ppp, fly)
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
        crate::help::hit(&self.overlay.help, px, py, 1.0 / ppp).map(|b| self.help_badge_points(&b))
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

    /// Every help badge of the last scene, in view points from the centre, y up (window tests).
    pub fn help_badges(&self) -> Vec<(crate::help::HelpTopic, (f64, f64), f64)> {
        self.overlay.help.iter().map(|b| self.help_badge_points(b)).collect()
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
                self.renderer.viewport = v;
                self.renderer.note_interaction(now);
            }
        }
        let turning = self.renderer.navigator.is_turning_north();
        self.overlay.compass.update(self.renderer.viewport.angle, turning, now);
        if self.overlay.active()
            || self.overlay.compass.shown(now)
            || self.renderer.overlay.is_some()
            || self.renderer.progress.current().alpha > 0.0
        {
            let (w, h, ppp) = (self.width, self.height, self.points_per_pixel);
            self.overlay.prepare(&mut self.renderer, (w, h), ppp, now);
        } else {
            // Nothing drawn: no badge to press.
            self.overlay.help.clear();
        }
    }

    /// Overlay work after the frame: about 1.5 ms of orbit analysis, and the maps' renderers
    /// while the view is not in a gesture (they never take the gesture frame budget).
    fn overlay_after(&mut self, now: f64, outcome: &mut FrameOutcome) {
        outcome.animating |= self.overlay.cosmic.animating(now)
            || self.overlay.compass.animating(now)
            || self.renderer.progress.animating();
        if !self.overlay.settings.any() {
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
        }
        outcome.animating |= self.overlay.busy();
    }

    /// The home view fitted to the view.
    pub fn reset_viewport(&mut self) {
        let (w, h) = self.view_points;
        self.renderer.navigator.cancel_flight();
        self.renderer.viewport = Viewport::fitting(w, h);
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
        if let Some(m) = &mut self.measure
            && self.renderer.navigator.is_moving()
        {
            // Held keys move the view in this frame: input "arrives" now.
            m.pending_input.get_or_insert(start);
        }
        let mut tag = 0;
        self.overlay_before(now);
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
        }
    }

    /// Centre and scale for the clipboard, at the precision the view needs (M3b: arbitrary;
    /// pasting it back gives the same view to a fraction of a point).
    pub fn coordinates_text(&self) -> String {
        let v = &self.renderer.viewport;
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
