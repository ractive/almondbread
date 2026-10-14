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
    pending_input: Option<f64>,
    input_to_draw: Vec<f64>,
    /// Presentation tag -> (draw start, input the frame shows).
    frames: HashMap<u64, (f64, Option<f64>)>,
    presented: Vec<(u64, f64)>,
}

/// The renderer plus the view it draws into.
pub struct Host<B: GpuBackend> {
    pub renderer: Renderer<B>,
    width: usize,
    height: usize,
    points_per_pixel: f64,
    view_points: (f64, f64),
    fitted: bool,
    measure: Option<Measurement>,
}

impl<B: GpuBackend> Host<B> {
    pub fn new(renderer: Renderer<B>) -> Self {
        Self {
            renderer,
            width: 0,
            height: 0,
            points_per_pixel: 0.0,
            view_points: (0.0, 0.0),
            fitted: false,
            measure: None,
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
        let (w, h) = (width.max(0.0), height.max(0.0));
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
        if self.renderer.navigator.last_interaction != interaction
            && let Some(m) = &mut self.measure
        {
            m.pending_input.get_or_insert(now);
        }
        let moved = self.renderer.viewport != before;
        // A mode change needs a frame too: the platform updates cursor and crosshair from
        // the frame's overlay state; edge panning may start on a pointer move.
        let nav = &self.renderer.navigator;
        let redraw = handled
            && (moved
                || nav.is_animating()
                || nav.mode() != mode
                || matches!(event, InputEvent::KeyDown(_) | InputEvent::Resize { .. }));
        InputOutcome { handled, redraw }
    }

    /// The home view fitted to the view.
    pub fn reset_viewport(&mut self) {
        let (w, h) = self.view_points;
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
        let (w, h, ppp) = (self.width, self.height, self.points_per_pixel);
        let outcome = self
            .renderer
            .render_frame_gated(now, w, h, ppp, target_ready, &mut |final_frame| {
                acquire(final_frame).map(|(target, t)| {
                    tag = t;
                    target
                })
            });
        if let Some(m) = &mut self.measure {
            m.draw_cpu.push(clock() - start);
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
            skipped: fs.skipped,
            skipped_in_flight: fs.skipped_in_flight,
            skipped_no_target: fs.skipped_no_target,
            draws: m.draw_cpu.len(),
            encoded: fs.frames,
        }
    }

    /// Centre and scale for the clipboard, full f64 precision.
    pub fn coordinates_text(&self) -> String {
        let v = &self.renderer.viewport;
        format!(
            "re {}\nim {}\nscale {} (zoom {}×)",
            g(v.center.re(), 17),
            g(v.center.im(), 17),
            g(v.scale, 6),
            g(v.zoom(), 6)
        )
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
