//! The π meter (kb: techniques/pi-in-the-mandelbrot-set, decisions/controls "π meter"): when
//! the view is centred on the neck `-3/4 + iε` or the cusp `1/4 + ε`, a panel shows the
//! escape count N of the centre, the product N·ε (N·√ε) with the digits that already agree
//! with π lit, and a log-log plot of the error |N·ε − π| against ε that fills in as the view
//! zooms along the line. The counts (up to [`MAX_COUNT`] iterations, about 13 s) run on a
//! background thread ([`almondbread_core::pi_moment::escape_count`]); the frame only reads
//! their result, so the meter costs the frame a scene rebuild when its text changes. The
//! image's iteration limit is left alone (raising it near a parabolic point costs seconds
//! to minutes of GPU per frame and shows nothing new: the kb page has the measurements).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use almondbread_core::cfmt::g;
use almondbread_core::pi_moment::{
    Count, MAX_COUNT, PI_TEXT, PiPoint, PiTarget, escape_count, matching_prefix, product_text, target,
};
use almondbread_core::viewport::{Center, Viewport};

use crate::overlay::{Scene, rgba};

/// One finished count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub point: PiPoint,
    pub eps: f64,
    pub n: u64,
    pub seconds: f64,
}

impl Measured {
    pub fn product(&self) -> f64 {
        self.point.product(self.n, self.eps)
    }

    /// The product as shown (decimals to where the error can reach).
    pub fn text(&self) -> String {
        let scale = match self.point {
            PiPoint::Neck => self.eps,
            PiPoint::Cusp => self.eps.sqrt(),
        };
        product_text(self.product(), scale)
    }

    /// Digits of the shown product that agree with π.
    pub fn digits(&self) -> usize {
        matching_prefix(&self.text()).1
    }
}

struct Job {
    target: PiTarget,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
    /// The count and its seconds.
    handle: JoinHandle<(Count, f64)>,
}

/// What the meter shows, for the UI (control center).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PiMeterState {
    /// The view is centred on the neck or the cusp ("neck", "cusp"; empty: neither).
    pub point: String,
    /// ε of the view centre.
    pub eps: f64,
    /// The latest finished count at that point (0: none yet).
    pub count: u64,
    pub count_eps: f64,
    pub product: f64,
    pub digits: u32,
    /// A count is running; its progress 0 ... 1.
    pub counting: bool,
    pub progress: f64,
    /// ε is below the meter's limit.
    pub too_deep: bool,
}

/// The meter: settings, the counting thread, the measured history.
#[derive(Default)]
pub struct PiMeter {
    pub enabled: bool,
    /// Right and bottom margins of the panel beyond the standard 12 points, in view points
    /// (iPhone portrait keeps its buttons clear).
    pub inset: [f64; 2],
    target: Option<PiTarget>,
    /// The parabolic point's offset from the view centre in view points (x right, y up).
    base_offset: Option<[f64; 2]>,
    /// The last count started (point, ε).
    attempted: Option<(PiPoint, f64)>,
    job: Option<Job>,
    last: Option<Measured>,
    /// Finished counts per point (neck, cusp), for the plot.
    history: [Vec<Measured>; 2],
    flight: Option<PiFlight>,
}

/// The guided flight: an exponential zoom about the parabolic point, ε = ε_0·e^(-rate·t),
/// each frame's view framed like the place (the point stays put on screen). The fly-to path
/// would not do: between two views this close it pans most of the way first.
#[derive(Clone, Debug)]
struct PiFlight {
    point: PiPoint,
    eps: f64,
    end_eps: f64,
    last_time: Option<f64>,
    /// The view set last frame: any other view means the user took over.
    last_view: Option<Viewport>,
}

fn slot(p: PiPoint) -> usize {
    match p {
        PiPoint::Neck => 0,
        PiPoint::Cusp => 1,
    }
}

/// Plot ranges: log10 ε from -1 to the meter's limit, log10 error from 0 to -10.
fn x_range(p: PiPoint) -> (f64, f64) {
    (-1.0, p.min_eps().log10().floor())
}
const Y_RANGE: (f64, f64) = (0.0, -10.0);

impl PiMeter {
    /// Follows the view: collects a finished count, cancels one the view left, starts one for
    /// the centre's ε. Cheap (no counting on this thread).
    pub fn update(&mut self, view: &Viewport) {
        if !self.enabled {
            self.stop();
            self.target = None;
            return;
        }
        self.target = target(view);
        self.base_offset = self.target.map(|t| {
            let (bre, bim) = t.point.base();
            let (dx, dy) = Center::new(bre, bim).delta_in(&view.center, view.scale);
            let (x, y) = view.unrotate(dx, dy);
            [x, y]
        });
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished())
            && let Some(job) = self.job.take()
            && let Ok((Count::Escaped(n), seconds)) = job.handle.join()
        {
            self.record(Measured {
                point: job.target.point,
                eps: job.target.eps,
                n,
                seconds,
            });
        }
        // A running count stays while the view goes deeper (its result is still a point on
        // the plot); it is dropped when the view left the point or zoomed out more than a
        // decade past it (deep counts are the long ones).
        if let Some(j) = &self.job {
            let keep = self
                .target
                .is_some_and(|t| t.point == j.target.point && j.target.eps * 10.0 >= t.eps);
            if !keep {
                self.stop();
            }
        }
        let Some(t) = self.target else { return };
        if self.job.is_some() || t.eps < t.point.min_eps() {
            return;
        }
        // Counted (or tried) already: a count that ends without escaping is not retried.
        let same = |p: PiPoint, e: f64| p == t.point && (e / t.eps - 1.0).abs() < 1e-9;
        if self.attempted.is_some_and(|(p, e)| same(p, e)) || self.last.is_some_and(|m| same(m.point, m.eps)) {
            return;
        }
        self.attempted = Some((t.point, t.eps));
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let (c, p) = (cancel.clone(), progress.clone());
        let handle = std::thread::Builder::new()
            .name("pi-meter".into())
            .spawn(move || {
                let start = Instant::now();
                // (2x: N ≈ π/ε + O(1) reaches MAX_COUNT at the limit itself.)
                let count = escape_count(t.point, t.eps, 2 * MAX_COUNT, &c, &p);
                (count, start.elapsed().as_secs_f64())
            })
            .expect("pi meter thread");
        self.job = Some(Job {
            target: t,
            cancel,
            progress,
            handle,
        });
    }

    fn stop(&mut self) {
        if let Some(j) = self.job.take() {
            // The thread sees the flag within 2^20 iterations (about 10 ms) and ends; the
            // count was not made, so the same view counts again when it comes back.
            j.cancel.store(true, Ordering::Relaxed);
            self.attempted = None;
        }
    }

    fn record(&mut self, m: Measured) {
        self.last = Some(m);
        let h = &mut self.history[slot(m.point)];
        // One point per 0.05 decade (the newest wins).
        h.retain(|o| (o.eps.log10() - m.eps.log10()).abs() > 0.05);
        h.push(m);
        h.sort_by(|a, b| b.eps.total_cmp(&a.eps));
        if h.len() > 200 {
            h.remove(0);
        }
    }

    /// A count is running (frames keep coming so its progress shows) or the flight is on.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.flight.is_some()
    }

    /// Starts the guided flight from the view `start_eps` on the line down to `end_eps`.
    pub fn start_flight(&mut self, point: PiPoint, start_eps: f64, end_eps: f64) {
        self.enabled = true;
        self.flight = Some(PiFlight {
            point,
            eps: start_eps,
            end_eps,
            last_time: None,
            last_view: None,
        });
    }

    pub fn stop_flight(&mut self) {
        self.flight = None;
    }

    pub fn flying(&self) -> bool {
        self.flight.is_some()
    }

    /// The flight's view for this refresh: `current` is the renderer's view (a different one
    /// than set last frame ends the flight: the user moved), `short_side` the view's shorter
    /// side in points, `rate` e-folds per second (the zoom speed: Q / E change it). None:
    /// no flight (or it just ended).
    pub fn flight_view(&mut self, current: &Viewport, short_side: f64, now: f64, rate: f64) -> Option<Viewport> {
        let f = self.flight.as_mut()?;
        if f.last_view.as_ref().is_some_and(|v| v != current) || !self.enabled {
            self.flight = None;
            return None;
        }
        let dt = f.last_time.map_or(0.0, |t| (now - t).clamp(0.0, 0.1));
        f.last_time = Some(now);
        // Ease out over the last e-fold so the flight lands rather than stops.
        let left = (f.eps / f.end_eps).ln().max(0.0);
        let r = rate.max(0.05) * left.min(1.0).max(0.08);
        f.eps = (f.eps * (-r * dt).exp()).max(f.end_eps);
        let v = f.point.view_at(f.eps, short_side).with_angle(current.angle);
        if f.eps <= f.end_eps * (1.0 + 1e-9) {
            self.flight = None;
        } else {
            f.last_view = Some(v.clone());
        }
        Some(v)
    }

    /// Waits for the running count (headless stills).
    pub fn wait(&mut self, view: &Viewport) {
        while let Some(j) = &self.job {
            if j.handle.is_finished() {
                self.update(view);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        self.update(view);
    }

    /// Records a finished count as if the meter had made it (headless stills fill the plot
    /// with the counts along the line).
    pub fn add_measurement(&mut self, m: Measured) {
        self.record(m);
    }

    pub fn state(&self) -> PiMeterState {
        let Some(t) = self.target.filter(|_| self.enabled) else {
            return PiMeterState::default();
        };
        let last = self.last.filter(|m| m.point == t.point);
        let (counting, progress) = match &self.job {
            Some(j) => (
                true,
                j.progress.load(Ordering::Relaxed) as f64 / j.target.point.expected_count(j.target.eps),
            ),
            None => (false, 0.0),
        };
        PiMeterState {
            point: t.point.name().into(),
            eps: t.eps,
            count: last.map_or(0, |m| m.n),
            count_eps: last.map_or(0.0, |m| m.eps),
            product: last.map_or(0.0, |m| m.product()),
            digits: last.map_or(0, |m| m.digits() as u32),
            counting,
            progress: progress.clamp(0.0, 1.0),
            too_deep: t.eps < t.point.min_eps(),
        }
    }

    /// Draws the panel into `scene` (output pixels `w` x `h`, `px` pixels per point) and
    /// returns the scene key part (the scene is rebuilt when it changes).
    pub fn draw(&self, scene: &mut Scene, w: f64, h: f64, px: f64) -> String {
        if !self.enabled {
            return String::new();
        }
        let p = px as f32;
        let right = (w - (12.0 + self.inset[0]) * px) as f32;
        let bottom = (h - (12.0 + self.inset[1]) * px) as f32;
        let pad = 12.0 * p;
        let fill = rgba(0.02, 0.02, 0.03, 0.8);
        let border = rgba(1.0, 1.0, 1.0, 0.22);
        let white = |a: f32| rgba(1.0, 1.0, 1.0, a);
        let gold = rgba(1.0, 0.82, 0.32, 1.0);
        let Some(t) = self.target else {
            // Not at a point: how to get there.
            let lines = [
                "π meter: centre the view on the neck c = −3/4 + iε".to_string(),
                "or the cusp c = 1/4 + ε (Places: π at the neck, π at the cusp)".to_string(),
            ];
            let size = 12.0 * p;
            let wmax = lines.iter().map(|l| crate::font::layout(l, size).1).fold(0.0, f32::max);
            let bw = wmax + 2.0 * pad;
            let bh = 2.0 * pad + size * (1.0 + 1.5);
            let r = [right - bw, bottom - bh, right, bottom];
            scene.rect(r, 8.0 * p, fill, border, p);
            for (k, l) in lines.iter().enumerate() {
                scene.text([r[0] + pad, r[1] + pad + k as f32 * 1.5 * size], size, l, white(0.85));
            }
            return format!("pi hint {w}x{h} {px} {:?}", self.inset);
        };
        let point = t.point;
        let last = self.last.filter(|m| m.point == point);
        let state = self.state();
        // ---- On the canvas: the line of approach from the parabolic point to the centre,
        // where the count is made (the image itself is mostly unresolved this close).
        if let Some([bx, by]) = self.base_offset {
            let (cx, cy) = (w / 2.0, h / 2.0);
            let base = [(cx + bx * px) as f32, (cy - by * px) as f32];
            let centre = [cx as f32, cy as f32];
            let dash = white(0.55);
            let shadow = rgba(0.0, 0.0, 0.0, 0.5);
            if let Some((a, b)) = crate::overlay::clip_segment(base, centre, [0.0, 0.0, w as f32, h as f32]) {
                let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                let period = 9.0 * p;
                let n = (len / period).ceil().min(400.0) as usize;
                for k in 0..n {
                    let t0 = k as f32 / n as f32;
                    let t1 = (k as f32 + 0.55) / n as f32;
                    let at = |t: f32| [f64::from(a[0] + (b[0] - a[0]) * t), f64::from(a[1] + (b[1] - a[1]) * t)];
                    scene.segment(at(t0), at(t1), 1.6 * p, shadow, shadow);
                    scene.segment(at(t0), at(t1), 0.7 * p, dash, dash);
                }
            }
            let on_screen = |q: [f32; 2]| q[0] >= 0.0 && q[1] >= 0.0 && q[0] <= w as f32 && q[1] <= h as f32;
            if on_screen(base) {
                let bq = [f64::from(base[0]), f64::from(base[1])];
                scene.ring(bq, 5.0 * p, 2.6 * p, shadow);
                scene.ring(bq, 5.0 * p, 1.2 * p, white(0.9));
                let lab = match t.point {
                    PiPoint::Neck => "−3/4",
                    PiPoint::Cusp => "1/4",
                };
                scene.label([base[0] + 9.0 * p, base[1] + 6.0 * p], 11.0 * p, lab, white(0.85));
            }
            let cq = [cx, cy];
            scene.ring(cq, 6.5 * p, 2.8 * p, shadow);
            scene.ring(cq, 6.5 * p, 1.4 * p, gold);
            let sym_c = match t.point {
                PiPoint::Neck => "−3/4 + iε",
                PiPoint::Cusp => "1/4 + ε",
            };
            scene.label([centre[0] + 11.0 * p, centre[1] - 16.0 * p], 11.0 * p, sym_c, gold);
        }
        // ---- Panel.
        let bw = 316.0 * p;
        let plot_h = 104.0 * p;
        let s_title = 11.0 * p;
        let s_big = 22.0 * p;
        let s_info = 11.5 * p;
        let s_tick = 9.5 * p;
        let y_title = 0.0;
        let y_value = y_title + 1.9 * s_title;
        let y_pi = y_value + 1.3 * s_big;
        let y_info = y_pi + s_big + 0.95 * s_info;
        let y_status = y_info + 1.55 * s_info;
        let y_plot = y_status + 2.1 * s_info;
        let bh = 2.0 * pad + y_plot + plot_h + 1.8 * s_tick;
        let r = [right - bw, bottom - bh, right, bottom];
        scene.rect(r, 8.0 * p, fill, border, p);
        let x0 = r[0] + pad;
        let top = r[1] + pad;
        let (sym, line) = match point {
            PiPoint::Neck => ("N·ε", "π at the neck   c = −3/4 + iε"),
            PiPoint::Cusp => ("N·√ε", "π at the cusp   c = 1/4 + ε"),
        };
        scene.text([x0, top + y_title], s_title, line, white(0.62));
        // ---- The product against π, digits aligned (tabular figures).
        let lab_v = format!("{sym} =");
        let lab_w = crate::font::layout(&lab_v, s_big).1;
        let col = x0 + lab_w + 0.3 * s_big;
        scene.text([x0, top + y_value], s_big, &lab_v, white(0.75));
        let pi_w = crate::font::layout("π =", s_big).1;
        scene.text([x0 + lab_w - pi_w, top + y_pi], s_big, "π =", white(0.45));
        let mut key = format!("pi {w}x{h} {px} {:?} {} {} ", self.inset, point.name(), g(t.eps, 4));
        match last {
            Some(m) => {
                let text = m.text();
                let (chars, _) = matching_prefix(&text);
                let (good, rest) = text.split_at(text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i));
                let gw = scene.text([col, top + y_value], s_big, good, gold);
                scene.text([col + gw, top + y_value], s_big, rest, white(0.38));
                let pi_len = text.chars().count() + 2;
                let pi_shown: String = PI_TEXT.chars().take(pi_len).collect();
                let pw = scene.text([col, top + y_pi], s_big, &pi_shown, white(0.45));
                scene.text([col + pw, top + y_pi], s_big, "…", white(0.3));
                key.push_str(&format!("{} {} ", m.n, m.eps));
            }
            None => {
                scene.text([col, top + y_value], s_big, "…", white(0.5));
                let pi_shown: String = PI_TEXT.chars().take(10).collect();
                scene.text([col, top + y_pi], s_big, &pi_shown, white(0.45));
            }
        }
        // ---- N and ε, then the status.
        let info = match last {
            Some(m) => format!("N = {}   ε = {}", group(m.n), g(m.eps, 3)),
            None => format!("ε = {}", g(t.eps, 3)),
        };
        scene.text([x0, top + y_info], s_info, &info, white(0.8));
        let status = if state.too_deep {
            format!(
                "past the depth limit: ε < {}, N > 1e{}",
                g(point.min_eps(), 2),
                (MAX_COUNT as f64).log10().round()
            )
        } else if let Some(j) = &self.job {
            let pct = (100.0 * state.progress).floor();
            key.push_str(&format!("job {} {pct} ", j.target.eps));
            format!("counting ε = {} …  {pct:.0} %", g(j.target.eps, 3))
        } else if let Some(m) = last {
            let d = m.digits();
            format!(
                "{d} digit{} of π   ({:.2} s, compensated f64)",
                if d == 1 { "" } else { "s" },
                m.seconds
            )
        } else {
            String::new()
        };
        let status_color = if self.job.is_none() && last.is_some() && !state.too_deep {
            gold
        } else {
            white(0.7)
        };
        scene.text([x0, top + y_status], s_info, &status, status_color);
        if self.job.is_some() && !state.too_deep {
            // Progress bar under the status line.
            let by = top + y_status + 1.15 * s_info;
            let bx1 = r[2] - pad;
            scene.rect([x0, by, bx1, by + 2.0 * p], p, white(0.15), [0.0; 4], 0.0);
            let fx = x0 + (bx1 - x0) * state.progress as f32;
            scene.rect([x0, by, fx.max(x0 + 2.0 * p), by + 2.0 * p], p, gold, [0.0; 4], 0.0);
        }
        // ---- Convergence plot: log10 error against log10 ε (ε falls to the right).
        let pr = [x0 + 36.0 * p, top + y_plot, r[2] - pad, top + y_plot + plot_h];
        let (xa, xb) = x_range(point);
        let (ya, yb) = Y_RANGE;
        let to = |le: f64, lerr: f64| -> [f64; 2] {
            let fx = ((le - xa) / (xb - xa)).clamp(-0.02, 1.02);
            let fy = ((lerr - ya) / (yb - ya)).clamp(-0.02, 1.02);
            [
                f64::from(pr[0]) + fx * f64::from(pr[2] - pr[0]),
                f64::from(pr[1]) + fy * f64::from(pr[3] - pr[1]),
            ]
        };
        let axis = white(0.35);
        let hw = 0.5 * p;
        scene.segment(
            [f64::from(pr[0]), f64::from(pr[1])],
            [f64::from(pr[0]), f64::from(pr[3])],
            hw,
            axis,
            axis,
        );
        scene.segment(
            [f64::from(pr[0]), f64::from(pr[3])],
            [f64::from(pr[2]), f64::from(pr[3])],
            hw,
            axis,
            axis,
        );
        // Ticks and labels: ε every few decades, the error at 1, 1e-5, 1e-10.
        let step = if xb < -12.0 { 4 } else { 2 };
        let mut d = xa.round() as i64;
        while d as f64 >= xb {
            if (d as f64) <= xa && (d.unsigned_abs() as i64 - 1) % step == 0 {
                let q = to(d as f64, yb);
                scene.segment(q, [q[0], q[1] + 3.0 * px], hw, axis, axis);
                let lab = format!("1e{d}");
                let lw = crate::font::layout(&lab, s_tick).1;
                scene.text(
                    [(q[0] as f32 - 0.5 * lw).min(r[2] - 4.0 * p - lw), q[1] as f32 + 4.5 * p],
                    s_tick,
                    &lab,
                    white(0.5),
                );
            }
            d -= 1;
        }
        for (e, lab) in [(0.0, "1"), (-5.0, "1e-5"), (-10.0, "1e-10")] {
            let q = to(xa, e);
            scene.segment([q[0] - 3.0 * px, q[1]], q, hw, axis, axis);
            let lw = crate::font::layout(lab, s_tick).1;
            scene.text(
                [q[0] as f32 - 5.0 * p - lw, q[1] as f32 - 0.36 * s_tick],
                s_tick,
                lab,
                white(0.5),
            );
        }
        let (err_lab, guide_lab) = match point {
            PiPoint::Neck => ("|N·ε − π|", "~ε"),
            PiPoint::Cusp => ("|N·√ε − π|", "~√ε"),
        };
        scene.text([pr[0] + 12.0 * p, pr[1] - 0.2 * s_tick], s_tick, err_lab, white(0.55));
        let el = "ε → 0";
        let elw = crate::font::layout(el, s_tick).1;
        scene.text([pr[2] - elw, pr[1] - 0.2 * s_tick], s_tick, el, white(0.45));
        // The expected trend through the measured offsets (N − π/ε ≈ 1 at the neck,
        // N − π/√ε ≈ −2 at the cusp; measured 2026-10-15): dashed.
        let (c, slope) = match point {
            PiPoint::Neck => (0.9f64.log10(), 1.0),
            PiPoint::Cusp => (1.9f64.log10(), 0.5),
        };
        let guide = rgba(0.55, 0.85, 1.0, 0.45);
        let n_dash = 26;
        for k in 0..n_dash {
            if k % 2 == 1 {
                continue;
            }
            let a = xa + (xb - xa) * k as f64 / n_dash as f64;
            let b = xa + (xb - xa) * (k + 1) as f64 / n_dash as f64;
            scene.segment(to(a, c + slope * a), to(b, c + slope * b), 0.6 * p, guide, guide);
        }
        let gx = xa + 0.8 * (xb - xa);
        let gq = to(gx, c + slope * gx);
        scene.text([gq[0] as f32, gq[1] as f32 - 1.9 * s_tick], s_tick, guide_lab, guide);
        // Measured points.
        let hist = &self.history[slot(point)];
        let pts: Vec<[f64; 2]> = hist
            .iter()
            .map(|m| {
                to(
                    m.eps.log10(),
                    (m.product() - std::f64::consts::PI).abs().max(1e-16).log10(),
                )
            })
            .collect();
        let line = rgba(1.0, 0.82, 0.32, 0.55);
        for k in 1..pts.len() {
            scene.segment(pts[k - 1], pts[k], 0.7 * p, line, line);
        }
        for (k, q) in pts.iter().enumerate() {
            let current = last.is_some_and(|m| m.eps == hist[k].eps);
            if current {
                scene.disc(*q, 4.2 * p, rgba(0.0, 0.0, 0.0, 0.6));
                scene.disc(*q, 3.2 * p, gold);
            } else {
                scene.disc(*q, 2.0 * p, white(0.8));
            }
        }
        // Where the view is now (a tick on the ε axis).
        let now = to(t.eps.log10().max(xb), yb);
        scene.segment([now[0], now[1] - 5.0 * px], now, 0.8 * p, gold, gold);
        key.push_str(&format!(
            "h{} now {:.1} base {:?} ",
            hist.len(),
            now[0],
            self.base_offset.map(|b| [(b[0] * px).round(), (b[1] * px).round()])
        ));
        key
    }
}

impl Drop for PiMeter {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 314159266 → "314 159 266".
fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flight_zooms_about_the_point_and_lands() {
        let h = 800.0;
        let p = PiPoint::Neck;
        let mut m = PiMeter::default();
        let mut v = p.view_at(0.02, h);
        m.start_flight(p, 0.02, p.flight_eps());
        let (mut t, mut prev) = (0.0, 0.02);
        while m.flying() && t < 60.0 {
            t += 1.0 / 60.0;
            let Some(nv) = m.flight_view(&v, h, t, 1.6) else { break };
            v = nv;
            let tg = target(&v).unwrap();
            assert!(tg.eps <= prev * (1.0 + 1e-12), "{t}: ε went up");
            prev = tg.eps;
            // The parabolic point stays a quarter of the height below the centre.
            let (dx, dy) = Center::new(-0.75, 0.0).delta_in(&v.center, v.scale);
            assert!(dx.abs() < 1e-6 && (dy / h + 0.25).abs() < 1e-6, "{t}: {dx} {dy}");
        }
        assert!(!m.flying());
        assert!((prev / p.flight_eps() - 1.0).abs() < 1e-9, "{prev}");
        // 14.5 e-folds at 1.6 per second, plus the eased landing.
        assert!(t > 9.0 && t < 14.0, "{t}");
        // A view moved by the user ends it.
        m.start_flight(p, 0.02, p.flight_eps());
        let v0 = p.view_at(0.02, h);
        let v1 = m.flight_view(&v0, h, 0.0, 1.6).unwrap();
        let mut moved = v1.clone();
        moved.pan_by_points(5.0, 0.0);
        assert!(m.flight_view(&moved, h, 0.1, 1.6).is_none());
        assert!(!m.flying());
    }

    #[test]
    fn groups_digits() {
        assert_eq!(group(314159266), "314 159 266");
        assert_eq!(group(33), "33");
        assert_eq!(group(3143), "3 143");
    }

    #[test]
    fn meter_counts_on_a_thread_and_draws() {
        let mut m = PiMeter::default();
        m.enabled = true;
        let view = Viewport::new(-0.75, 1e-4, 1e-6);
        m.update(&view);
        assert!(m.busy());
        m.wait(&view);
        let s = m.state();
        assert_eq!((s.point.as_str(), s.count, s.digits), ("neck", 31417, 4));
        let mut scene = Scene::default();
        let key = m.draw(&mut scene, 1600.0, 1000.0, 1.0);
        assert!(key.contains("31417"), "{key}");
        assert!(scene.prims.len() > 50);
        // What a frame pays when the panel changes: update, build, bin at Retina size.
        let t0 = Instant::now();
        for _ in 0..100 {
            m.update(&view);
            let mut s = Scene::default();
            m.draw(&mut s, 2880.0, 1800.0, 2.0);
            std::hint::black_box(s.bin(2880, 1800));
        }
        let per = t0.elapsed().as_secs_f64() / 100.0;
        assert!(per < 0.004, "{per}");
        // Every character of the panel is in the font.
        for t in [
            "N·ε",
            "N·√ε",
            "|N·√ε − π|",
            "~√ε",
            "ε → 0",
            "π at the neck   c = −3/4 + iε",
            "…",
        ] {
            for ch in t.chars().filter(|c| !c.is_whitespace()) {
                assert!(crate::font::glyph(ch) != crate::font::glyph('?') || ch == '?', "{ch}");
            }
        }
        // Off: nothing drawn, the thread stopped.
        m.enabled = false;
        m.update(&view);
        assert!(!m.busy());
        let mut scene = Scene::default();
        assert!(m.draw(&mut scene, 1600.0, 1000.0, 1.0).is_empty());
        assert!(scene.prims.is_empty());
    }
}
