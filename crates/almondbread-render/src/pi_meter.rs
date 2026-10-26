//! The π meter (kb: techniques/pi-in-the-mandelbrot-set, decisions/controls "π meter"): the
//! escape count N of a point on the neck's line `-3/4 + iε` or the cusp's `1/4 + ε`, the
//! product N·ε (N·√ε) with the digits that already agree with π lit, a log-log plot of the
//! error |N·ε − π| against ε, and the counted point's orbit as a glowing trail.
//!
//! The counted point is the view's centre when the view is centred on a line, or else the
//! **probe**: a marker the guided flight slides down the line towards the pinch point while
//! the view stays on the valley it opens into (seahorse valley above the neck, elephant
//! valley right of the cusp). The image there is honest black (the gap at height ε is ~ε²
//! wide), so the probe slides on a log ruler, one tick per decade, its true position as text.
//!
//! The counts (up to [`MAX_COUNT`] iterations, about 13 s) run on a background thread
//! ([`almondbread_core::pi_moment::escape_count_traced`]), which also accumulates the orbit's
//! density into the trail picture and publishes it a few times per second; the frame only
//! reads results and uploads a new picture when there is one. The image's iteration limit is
//! left alone (raising it near a parabolic point costs seconds to minutes of GPU per frame
//! and shows nothing new: the kb page has the measurements).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use almondbread_core::cfmt::g;
use almondbread_core::pi_moment::{
    CUSP_X, Count, MAX_COUNT, MAX_EPS, PI_TEXT, PiPoint, PiTarget, Trail, TrailImage, TrailLayout, TrailSlot,
    escape_count_traced, matching_prefix, orbit_marks, product_text, target,
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
    /// The newest trail picture of this count.
    slot: TrailSlot,
    /// The count and its seconds.
    handle: JoinHandle<(Count, f64)>,
}

/// What the meter shows, for the UI (control center).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PiMeterState {
    /// The view is centred on the neck or the cusp, or the probe is there ("neck", "cusp";
    /// empty: neither).
    pub point: String,
    /// ε of the counted point (view centre or probe).
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

/// Space other overlays take, in output pixels, which the meter keeps clear of: the height
/// used at the bottom right and the top right, the width used at the bottom left, the height
/// of platform controls along the top (iPad round buttons), and rects it must not cover (the
/// cosmic scale, the cockpit, a bottom-left map row, a guided tour's card).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reserve {
    pub bottom_right: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub top: f32,
    /// The safe area's right and bottom insets (pixels: the notch and the home indicator of a
    /// phone), added to the panel's margins.
    pub edge: [f32; 2],
    pub keep_out: [Option<[f32; 4]>; 6],
}

impl Reserve {
    /// Adds a rect (pixels: x0, y0, x1, y1) to keep clear of (the first six count).
    pub fn keep_clear(&mut self, r: [f32; 4]) {
        if r.iter().all(|v| v.is_finite())
            && r[2] > r[0]
            && r[3] > r[1]
            && let Some(slot) = self.keep_out.iter_mut().find(|s| s.is_none())
        {
            *slot = Some(r);
        }
    }

    fn rects(&self) -> impl Iterator<Item = [f32; 4]> + '_ {
        self.keep_out.iter().flatten().copied()
    }
}

/// The smallest drawing scale of the meter's panel on a narrow canvas (an iPhone, an iPad in
/// portrait with a tour card).
const MIN_SCALE: f32 = 0.75;

/// Where the panel goes: its right and bottom edges (pixels) and its drawing scale.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Place {
    right: f32,
    bottom: f32,
    k: f32,
}

fn intersects(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// From a start (right and bottom edge), a place for a panel `size` (pixels at scale 1) clear
/// of `keep`: a rect reaching in from the left shrinks the panel (down to [`MIN_SCALE`]), any
/// other lifts it above that rect; None when it runs out of room (`left` / `top` limits).
fn resolve(right: f32, bottom: f32, size: [f32; 2], left: f32, top: f32, keep: &[[f32; 4]], gap: f32) -> Option<Place> {
    let [bw, bh] = size;
    if bw.is_nan() || bh.is_nan() || bw <= 0.0 || bh <= 0.0 || right - left < bw * MIN_SCALE {
        return None;
    }
    let mut k = ((right - left) / bw).min(1.0);
    let mut bottom = bottom;
    // Each round clears one rect; a few rects, so a few rounds (bounded: kb: techniques/hang-prevention).
    for _ in 0..12 {
        if bottom - bh * k < top {
            k = (bottom - top) / bh;
            if k.is_nan() || k < MIN_SCALE {
                return None;
            }
        }
        let r = [right - bw * k, bottom - bh * k, right, bottom];
        let Some(o) = keep.iter().find(|o| intersects(r, **o)) else {
            return Some(Place { right, bottom, k });
        };
        let shrunk = (right - o[2] - gap) / bw;
        if o[0] < r[0] && shrunk >= MIN_SCALE {
            k = k.min(shrunk);
        } else {
            bottom = o[1] - gap;
        }
    }
    None
}

/// The trail inset's plots in view points: the orbit plot (w, h), the turn circle, the gap.
const TRAIL_ORBIT: [f64; 2] = [220.0, 200.0];
const TRAIL_TURN: f64 = 168.0;
const TRAIL_GAP: f64 = 22.0;
/// Width of the trail inset in points (the meter's panel is 316).
const INSET_W: f64 = 434.0;

/// The meter: settings, the counting thread, the measured history, the probe and its flight.
#[derive(Default)]
pub struct PiMeter {
    pub enabled: bool,
    /// Right and bottom margins of the panel beyond the standard 12 points, in view points
    /// (iPhone portrait keeps its buttons clear).
    pub inset: [f64; 2],
    /// Height of platform controls along the top (view points; the iPad's round buttons):
    /// the panel and the trail inset stay below it when they move up.
    pub inset_top: f64,
    /// The panel and the trail inset as last drawn (pixels), for the UI tests.
    drawn: std::cell::Cell<[Option<[f32; 4]>; 2]>,
    target: Option<PiTarget>,
    /// The probe (counted instead of the view centre): set by the guided flight, kept while
    /// its pinch point stays in view (it wins over a view centred on the line).
    probe: Option<PiTarget>,
    /// The parabolic point's offset from the view centre in view points (x right, y up).
    base_offset: Option<[f64; 2]>,
    /// The line of approach on screen (unit vector, points, x right, y up) and the view's
    /// complex units per point.
    line_dir: [f64; 2],
    scale_pt: f64,
    /// View size in points, pixels per point, and the trail picture's layout at that scale.
    view_pts: [f64; 2],
    px: f64,
    trail_layout: Option<TrailLayout>,
    /// The last count started (point, ε).
    attempted: Option<(PiPoint, f64)>,
    job: Option<Job>,
    last: Option<Measured>,
    /// The newest trail picture (of the running count, else of the last).
    trail: Option<TrailImage>,
    /// Finished counts per point (neck, cusp), for the plot.
    history: [Vec<Measured>; 2],
    flight: Option<PiFlight>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The navigator's fly-to eases the view to the valley.
    Approach,
    /// The probe slides down the line (ε falls at the zoom speed in e-folds of the expected
    /// count), the view zooms gently about the pinch point.
    Slide,
    /// The probe has arrived: until its count is done, then a short hold.
    Hold,
}

/// The guided flight (kb: decisions/controls "π meter"): the view eases to the valley, the
/// probe slides in on the log ruler while the digits fill and the trail grows, a hold at the
/// end. The view never goes into the black at the pinch point.
#[derive(Clone, Debug)]
struct PiFlight {
    point: PiPoint,
    phase: Phase,
    start_eps: f64,
    eps: f64,
    end_eps: f64,
    last_time: Option<f64>,
    hold_since: Option<f64>,
    /// The view set last frame of the slide: any other view means the user took over.
    last_view: Option<Viewport>,
}

/// Largest magnification of the gentle zoom over the slide (the valley stays in view;
/// chosen by looking at renders: 1.4 at the neck, whose valley narrows to a wedge, 2 at the
/// cusp).
const FLIGHT_ZOOM: f64 = 2.0;

fn flight_zoom_total(point: PiPoint) -> f64 {
    match point {
        PiPoint::Neck => 1.4,
        PiPoint::Cusp => FLIGHT_ZOOM,
    }
}
/// Seconds the flight holds after the last count.
const FLIGHT_HOLD: f64 = 2.5;

/// The flight's gentle zoom with the probe at `eps` on a slide from `start` (1 at the start,
/// [`FLIGHT_ZOOM`] at the end, geometric in between).
pub fn flight_zoom(point: PiPoint, start: f64, eps: f64) -> f64 {
    let s = ((start / eps).ln() / (start / point.flight_eps()).ln()).clamp(0.0, 1.0);
    flight_zoom_total(point).powf(s)
}

/// The view of the flight with the probe at `eps` (headless stills of its moments).
pub fn flight_still_view(point: PiPoint, eps: f64, short_side: f64) -> Viewport {
    point.scenic_view(flight_zoom(point, MAX_EPS, eps), short_side)
}

/// Height of the meter's panel in pixels (`p` pixels per point; the layout in `draw`).
fn panel_height(p: f32) -> f32 {
    let (s_title, s_big, s_info, s_tick) = (11.0 * p, 22.0 * p, 11.5 * p, 9.5 * p);
    let y_plot = 1.9 * s_title + 1.3 * s_big + s_big + 0.95 * s_info + 1.55 * s_info + 2.1 * s_info;
    2.0 * 12.0 * p + y_plot + 104.0 * p + 1.8 * s_tick
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

/// The parabolic point's offset from the view centre in points (x right, y up).
fn base_offset(point: PiPoint, view: &Viewport) -> [f64; 2] {
    let (bre, bim) = point.base();
    let (dx, dy) = Center::new(bre, bim).delta_in(&view.center, view.scale);
    let (x, y) = view.unrotate(dx, dy);
    [x, y]
}

/// Distance from `a` along the unit vector `d` to the edge of rectangle `r` (x0, y0, x1, y1);
/// 0 when `a` is outside.
fn ray_to_edge(a: [f64; 2], d: [f64; 2], r: [f64; 4]) -> f64 {
    if a[0] < r[0] || a[1] < r[1] || a[0] > r[2] || a[1] > r[3] {
        return 0.0;
    }
    let mut t = f64::INFINITY;
    for k in 0..2 {
        if d[k] > 1e-12 {
            t = t.min((r[k + 2] - a[k]) / d[k]);
        } else if d[k] < -1e-12 {
            t = t.min((r[k] - a[k]) / d[k]);
        }
    }
    t.max(0.0)
}

impl PiMeter {
    /// The output size in pixels and pixels per point (call before [`Self::update`] when it
    /// may have changed): the probe's visibility and the trail picture's resolution follow.
    pub fn set_layout(&mut self, width: usize, height: usize, px: f64) {
        // +inf too: the trail picture would be usize::MAX pixels wide.
        if !px.is_finite() || px <= 0.0 {
            return;
        }
        self.px = px;
        self.view_pts = [width as f64 / px, height as f64 / px];
        let r = |v: f64| (v * px).round().max(8.0) as usize;
        self.trail_layout = Some(TrailLayout {
            orbit: [r(TRAIL_ORBIT[0]), r(TRAIL_ORBIT[1])],
            turn: r(TRAIL_TURN),
            gap: r(TRAIL_GAP),
        });
    }

    /// Follows the view: collects a finished count and the newest trail picture, cancels a
    /// count the view left, starts one for the probe or the centre. Cheap (no counting on
    /// this thread).
    pub fn update(&mut self, view: &Viewport) {
        if !self.enabled {
            self.stop();
            self.target = None;
            self.probe = None;
            self.flight = None;
            self.trail = None;
            return;
        }
        let centred = target(view);
        if self.flight.is_none() {
            // The probe counts while its pinch point is in view (the valley views are centred
            // on the line too: the probe wins there); once the pinch point left the view, the
            // probe is put away and a view centred on a line counts its centre.
            if let Some(p) = self.probe {
                let [x, y] = base_offset(p.point, view);
                // (Negated <=: a pinch point beyond f64 view units is NaN in a turned view.)
                if self.view_pts[0] > 0.0 && !(x.abs() <= 0.5 * self.view_pts[0] && y.abs() <= 0.5 * self.view_pts[1]) {
                    self.probe = None;
                }
            }
        }
        self.target = self.probe.or(centred);
        self.base_offset = self.target.map(|t| base_offset(t.point, view));
        if let Some(t) = self.target {
            let (lx, ly) = match t.point {
                PiPoint::Neck => (0.0, 1.0),
                PiPoint::Cusp => (1.0, 0.0),
            };
            let (x, y) = view.unrotate(lx, ly);
            self.line_dir = [x, y];
            self.scale_pt = view.scale.to_f64();
        }
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished())
            && let Some(job) = self.job.take()
        {
            if let Ok(mut s) = job.slot.lock()
                && let Some(img) = s.take()
            {
                self.trail = Some(img);
            }
            if let Ok((Count::Escaped(n), seconds)) = job.handle.join() {
                self.record(Measured {
                    point: job.target.point,
                    eps: job.target.eps,
                    n,
                    seconds,
                });
            }
        }
        if let Some(j) = &self.job
            && let Ok(mut s) = j.slot.lock()
            && let Some(img) = s.take()
        {
            self.trail = Some(img);
        }
        if self
            .trail
            .as_ref()
            .is_some_and(|i| self.target.is_none_or(|t| t.point != i.point))
        {
            self.trail = None;
        }
        // A running count stays while the target goes deeper (its result is still a point on
        // the plot); it is dropped when the target left the point or went out more than a
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
        let slot: TrailSlot = Arc::new(Mutex::new(None));
        let (c, p, s) = (cancel.clone(), progress.clone(), slot.clone());
        let layout = self.trail_layout;
        let handle = std::thread::Builder::new()
            .name("pi-meter".into())
            .spawn(move || {
                let start = Instant::now();
                let mut trail = layout.map(|l| Trail::new(t.point, t.eps, l, s));
                // (2x: N ≈ π/ε + O(1) reaches MAX_COUNT at the limit itself.)
                let count = escape_count_traced(t.point, t.eps, 2 * MAX_COUNT, &c, &p, trail.as_mut());
                (count, start.elapsed().as_secs_f64())
            })
            .expect("pi meter thread");
        self.job = Some(Job {
            target: t,
            cancel,
            progress,
            slot,
            handle,
        });
    }

    fn stop(&mut self) {
        if let Some(j) = self.job.take() {
            // The thread sees the flag within 2^20 iterations (about 10 ms) and ends; the
            // count was not made, so the same point counts again when it comes back.
            j.cancel.store(true, Ordering::Relaxed);
            self.attempted = None;
        }
    }

    fn record(&mut self, m: Measured) {
        self.last = Some(m);
        let h = &mut self.history[slot(m.point)];
        // One point per 0.1-decade bin (the newest wins; a bin, not a distance, or a slow zoom
        // would keep replacing its last point and leave the plot empty).
        let bin = |e: f64| (e.log10() * 10.0).round() as i64;
        h.retain(|o| bin(o.eps) != bin(m.eps));
        h.push(m);
        h.sort_by(|a, b| b.eps.total_cmp(&a.eps));
        if h.len() > 200 {
            h.remove(0);
        }
    }

    /// A count is running (frames keep coming so its progress and trail show) or the flight
    /// is on.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.flight.is_some()
    }

    /// Starts the guided flight: the probe appears at `start_eps` on the line (the navigator
    /// is easing the view to [`PiPoint::scenic_view`] meanwhile), then slides to `end_eps`.
    pub fn start_flight(&mut self, point: PiPoint, start_eps: f64, end_eps: f64) {
        self.enabled = true;
        self.probe = Some(PiTarget { point, eps: start_eps });
        self.flight = Some(PiFlight {
            point,
            phase: Phase::Approach,
            start_eps,
            eps: start_eps,
            end_eps,
            last_time: None,
            hold_since: None,
            last_view: None,
        });
    }

    /// Ends the flight; the probe stays where it is.
    pub fn stop_flight(&mut self) {
        self.flight = None;
    }

    pub fn flying(&self) -> bool {
        self.flight.is_some()
    }

    /// Puts the probe at ε on the line of `point` (headless stills; the flight sets it).
    pub fn set_probe(&mut self, point: PiPoint, eps: f64) {
        self.probe = Some(PiTarget { point, eps });
    }

    /// The probe, if any.
    pub fn probe(&self) -> Option<PiTarget> {
        self.probe
    }

    /// The flight's view for this refresh. `current` is the renderer's view, `navigating`
    /// whether the navigator's fly-to still runs (the approach), `short_side` the view's
    /// shorter side in points, `rate` e-folds per second (the zoom speed: Q / E change it).
    /// None: the view is left alone (approach, hold, no flight). During the slide a view
    /// other than the one set last frame ends the flight (the user moved); the probe stays.
    pub fn flight_view(
        &mut self,
        current: &Viewport,
        navigating: bool,
        short_side: f64,
        now: f64,
        rate: f64,
    ) -> Option<Viewport> {
        if !self.enabled {
            self.flight = None;
            return None;
        }
        let mut f = self.flight.take()?;
        let out = match f.phase {
            Phase::Approach => {
                if !navigating {
                    f.phase = Phase::Slide;
                    f.last_time = Some(now);
                }
                None
            }
            Phase::Slide => {
                if f.last_view.as_ref().is_some_and(|v| v != current) {
                    return None;
                }
                let dt = f.last_time.map_or(0.0, |t| (now - t).clamp(0.0, 0.1));
                f.last_time = Some(now);
                // ε falls at `rate` e-folds of the expected count per second, eased over the
                // last e-fold so the probe lands rather than stops.
                let k = f.point.eps_per_count_efold();
                let left = (f.eps / f.end_eps).ln().max(0.0) / k;
                let r = rate.max(0.05) * left.clamp(0.08, 1.0);
                f.eps = (f.eps * (-r * k * dt).exp()).max(f.end_eps);
                let v = f
                    .point
                    .scenic_view(flight_zoom(f.point, f.start_eps, f.eps), short_side)
                    .with_angle(current.angle);
                self.probe = Some(PiTarget {
                    point: f.point,
                    eps: f.eps,
                });
                if f.eps <= f.end_eps * (1.0 + 1e-9) {
                    f.phase = Phase::Hold;
                    f.last_view = None;
                } else {
                    f.last_view = Some(v.clone());
                }
                Some(v)
            }
            Phase::Hold => {
                let end = f.end_eps;
                let counted = self.job.is_none()
                    && self
                        .attempted
                        .is_some_and(|(p, e)| p == f.point && (e / end - 1.0).abs() < 1e-9);
                if counted {
                    let since = *f.hold_since.get_or_insert(now);
                    if now - since >= FLIGHT_HOLD {
                        return None;
                    }
                }
                None
            }
        };
        self.flight = Some(f);
        out
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

    /// The panel and the trail inset as last drawn (pixels; None: not drawn).
    pub fn drawn(&self) -> [Option<[f32; 4]>; 2] {
        self.drawn.get()
    }

    /// The newest trail picture (the overlay uploads it when its serial changes).
    pub fn trail_picture(&self) -> Option<&TrailImage> {
        self.trail.as_ref().filter(|_| self.enabled)
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

    /// Draws the panels into `scene` (output pixels `w` x `h`, `px` pixels per point), clear
    /// of `reserve`, and returns the scene key part (the scene is rebuilt when it changes).
    pub fn draw(
        &self,
        scene: &mut Scene,
        help: &mut Vec<crate::help::HelpBadge>,
        w: f64,
        h: f64,
        px: f64,
        reserve: Reserve,
    ) -> String {
        self.drawn.set([None, None]);
        if !self.enabled {
            return String::new();
        }
        let p = px as f32;
        let start = [
            (w - (12.0 + self.inset[0]) * px) as f32 - reserve.edge[0],
            (h - (12.0 + self.inset[1]) * px) as f32 - reserve.bottom_right - reserve.edge[1],
        ];
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
            let bw = wmax + 2.0 * pad + crate::help::reserve(p);
            let bh = 2.0 * pad + size * (1.0 + 1.5);
            let pl = self
                .place(w as f32, p, start, [bw, bh], reserve, false)
                .unwrap_or_else(|| Self::corner(p, start, [bw, bh]));
            let r = [pl.right - bw, pl.bottom - bh, pl.right, pl.bottom];
            self.drawn.set([Some(r), None]);
            scene.rect(r, 8.0 * p, fill, border, p);
            for (k, l) in lines.iter().enumerate() {
                scene.text([r[0] + pad, r[1] + pad + k as f32 * 1.5 * size], size, l, white(0.85));
            }
            let mut key = format!("pi hint {w}x{h} {px} {:?} {reserve:?}", self.inset);
            let c = crate::help::top_right(r, p);
            crate::help::draw(scene, help, crate::help::HelpTopic::PiMeter, c, p, &mut key);
            return key;
        };
        let point = t.point;
        let last = self.last.filter(|m| m.point == point);
        let state = self.state();
        let mut canvas_key = String::new();
        // The panel stays off the probe's pinch point and the start of its ruler (the picture's
        // subject during a hunt), where it has the choice.
        let mut room = reserve;
        if self.probe.is_some()
            && let Some([bx, by]) = self.base_offset
        {
            let a = [w / 2.0 + bx * px, h / 2.0 - by * px];
            let b = [
                a[0] + self.line_dir[0] * 160.0 * px,
                a[1] - self.line_dir[1] * 160.0 * px,
            ];
            let m = 24.0 * px;
            room.keep_clear([
                (a[0].min(b[0]) - m) as f32,
                (a[1].min(b[1]) - m) as f32,
                (a[0].max(b[0]) + m) as f32,
                (a[1].max(b[1]) + m) as f32,
            ]);
        }
        // (The pinch point is a wish: without room beside it, the panel may cover it, never the
        // card, the cockpit or the other panels.)
        let size = [316.0 * p, panel_height(p)];
        let pl = self
            .place(w as f32, p, start, size, room, true)
            .or_else(|| self.place(w as f32, p, start, size, reserve, true))
            .unwrap_or_else(|| Self::corner(p, start, size));
        let (right, bottom, k) = (pl.right, pl.bottom, pl.k);
        // ---- On the canvas: the probe on its log ruler, or (view centred on the line) the
        // line of approach from the parabolic point to the centre, where the count is made
        // (the image itself is mostly unresolved this close).
        if self.probe.is_some() {
            // The ruler stops short of the panels.
            let panel = [right - 316.0 * p * k, bottom - panel_height(p) * k, right, bottom];
            let mut keep_clear = vec![panel];
            keep_clear.extend(self.inset_rect(panel, p, w as f32, reserve));
            // (And the tour card: the ruler's caption ran under it.)
            keep_clear.extend(reserve.rects());
            canvas_key = self.draw_probe(scene, w, h, px, t, &keep_clear);
        } else if let Some([bx, by]) = self.base_offset {
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
        // ---- Panel, drawn at scale k (a narrow canvas).
        let trail_panel = [right - 316.0 * p * k, bottom - panel_height(p) * k, right, bottom];
        let p = p * k;
        let pad = pad * k;
        let tick = f64::from(p);
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
        debug_assert!((bh - panel_height(p)).abs() < 1e-3);
        let r = [right - bw, bottom - bh, right, bottom];
        scene.rect(r, 8.0 * p, fill, border, p);
        crate::help::draw(
            scene,
            help,
            crate::help::HelpTopic::PiMeter,
            crate::help::top_right(r, p),
            p,
            &mut canvas_key,
        );
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
        // Integer decades from xa down to xb, at most 64 (kb: techniques/hang-prevention).
        let (d_hi, d_lo) = (xa.round() as i64, xb.ceil() as i64);
        for d in (d_lo.max(d_hi.saturating_sub(64))..=d_hi).rev() {
            if (d as f64) <= xa && (d.unsigned_abs() as i64 - 1) % step == 0 {
                let q = to(d as f64, yb);
                scene.segment(q, [q[0], q[1] + 3.0 * tick], hw, axis, axis);
                let lab = format!("1e{d}");
                let lw = crate::font::layout(&lab, s_tick).1;
                scene.text(
                    [(q[0] as f32 - 0.5 * lw).min(r[2] - 4.0 * p - lw), q[1] as f32 + 4.5 * p],
                    s_tick,
                    &lab,
                    white(0.5),
                );
            }
        }
        for (e, lab) in [(0.0, "1"), (-5.0, "1e-5"), (-10.0, "1e-10")] {
            let q = to(xa, e);
            scene.segment([q[0] - 3.0 * tick, q[1]], q, hw, axis, axis);
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
        scene.segment([now[0], now[1] - 5.0 * tick], now, 0.8 * p, gold, gold);
        key.push_str(&format!(
            "h{} now {:.1} base {:?} ",
            hist.len(),
            now[0],
            self.base_offset.map(|b| [(b[0] * px).round(), (b[1] * px).round()])
        ));
        key.push_str(&canvas_key);
        // ---- The orbit trail inset (above the panel, or left of it when the view is low).
        key.push_str(&format!("k {k:.3} "));
        self.drawn.set([
            Some(trail_panel),
            self.inset_rect(trail_panel, px as f32, w as f32, reserve),
        ]);
        key.push_str(&self.draw_trail(scene, trail_panel, px, w as f32, point, reserve));
        key
    }

    /// The probe on its log ruler: the line of approach from the pinch point, one tick per
    /// decade of ε (two at the cusp) from 0.1 down to the flight's end, the probe glowing at
    /// its ε with its true position as text (on the image it would be far below a pixel from
    /// the pinch point: the gap there is ~ε² wide).
    fn draw_probe(&self, scene: &mut Scene, w: f64, h: f64, px: f64, t: PiTarget, keep_clear: &[[f32; 4]]) -> String {
        let Some([bx, by]) = self.base_offset else {
            return String::new();
        };
        let p = px as f32;
        let base = [w / 2.0 + bx * px, h / 2.0 - by * px];
        let d = [self.line_dir[0], -self.line_dir[1]];
        let spp = self.scale_pt / px;
        let m = 30.0 * px;
        let room = ray_to_edge(base, d, [m, m, w - m, h - m]);
        if spp.is_nan() || spp <= 0.0 || room < 80.0 * px {
            return "probe off ".into();
        }
        // Under a panel (an iPhone in portrait leaves the meter no other place): no ruler
        // through its text; the panel shows the count.
        let (bx0, by0) = (base[0] as f32, base[1] as f32);
        if keep_clear
            .iter()
            .any(|r| bx0 > r[0] && bx0 < r[2] && by0 > r[1] && by0 < r[3])
        {
            return "probe covered ".into();
        }
        // Purely logarithmic: ε = 0.1 (where the flight starts) at the far end, one more
        // decade past the flight's end at the pinch point.
        let top_dec = MAX_EPS.log10().round();
        let mut len = (0.8 * room).min(560.0 * px);
        // Shorter while the ruler and its caption would run under a panel.
        let hits = |len: f64| {
            let a = [base[0] as f32, base[1] as f32];
            let e = len + 60.0 * px;
            let b = [(base[0] + d[0] * e) as f32, (base[1] + d[1] * e) as f32];
            let m = (10.0 * px) as f32;
            keep_clear
                .iter()
                .any(|r| crate::overlay::clip_segment(a, b, [r[0] - m, r[1] - m, r[2] + m, r[3] + m]).is_some())
        };
        while len > 160.0 * px && hits(len) {
            len *= 0.92;
        }
        let bottom_dec = t.point.flight_eps().log10().round() - 1.0;
        let pos = |e: f64| len * ((e.log10() - bottom_dec) / (top_dec - bottom_dec)).clamp(0.0, 1.05);
        let at = |s: f64| [base[0] + d[0] * s, base[1] + d[1] * s];
        let perp = [-d[1], d[0]];
        let shadow = rgba(0.0, 0.0, 0.0, 0.55);
        let white = |a: f32| rgba(1.0, 1.0, 1.0, a);
        let gold = rgba(1.0, 0.82, 0.32, 1.0);
        scene.segment(base, at(len), 1.8 * p, shadow, shadow);
        scene.segment(base, at(len), 0.8 * p, white(0.6), white(0.6));
        // Ticks and their decades.
        let s_tick = 9.5 * p;
        let step = match t.point {
            PiPoint::Neck => 1,
            PiPoint::Cusp => 2,
        };
        // Integer decades above bottom_dec, at most 64 (kb: techniques/hang-prevention).
        let (dec_hi, dec_lo) = (top_dec as i64, (bottom_dec.floor() as i64).saturating_add(1));
        for dec in (dec_lo.max(dec_hi.saturating_sub(64))..=dec_hi).rev() {
            if (top_dec as i64 - dec) % step == 0 {
                let q = at(pos(10f64.powi(dec as i32)));
                let a = [q[0] - perp[0] * 4.0 * px, q[1] - perp[1] * 4.0 * px];
                let b = [q[0] + perp[0] * 4.0 * px, q[1] + perp[1] * 4.0 * px];
                scene.segment(a, b, 1.6 * p, shadow, shadow);
                scene.segment(a, b, 0.7 * p, white(0.8), white(0.8));
                let lab = format!("1e{dec}");
                let lw = f64::from(crate::font::layout(&lab, s_tick).1);
                let c = [q[0] + perp[0] * 9.0 * px, q[1] + perp[1] * 9.0 * px];
                // Right of a vertical ruler, under a horizontal one.
                let o = if perp[0].abs() > perp[1].abs() {
                    [
                        c[0] + if perp[0] < 0.0 { -lw } else { 0.0 },
                        c[1] - 0.36 * f64::from(s_tick),
                    ]
                } else {
                    [
                        c[0] - 0.5 * lw,
                        c[1] + if perp[1] < 0.0 { -f64::from(s_tick) } else { 0.0 },
                    ]
                };
                scene.label([o[0] as f32, o[1] as f32], s_tick, &lab, white(0.75));
            }
        }
        let cap = "ε, log scale";
        let cw = f64::from(crate::font::layout(cap, s_tick).1);
        let e = at(len + 10.0 * px);
        let co = if perp[0].abs() > perp[1].abs() {
            [e[0] - 0.5 * cw, e[1] - f64::from(s_tick)]
        } else {
            [e[0], e[1] - 0.36 * f64::from(s_tick)]
        };
        scene.label([co[0] as f32, co[1] as f32], s_tick, cap, white(0.7));
        // The pinch point.
        scene.ring(base, 5.0 * p, 2.6 * p, shadow);
        scene.ring(base, 5.0 * p, 1.2 * p, white(0.9));
        let lab = match t.point {
            PiPoint::Neck => "−3/4",
            PiPoint::Cusp => "1/4",
        };
        let lw = crate::font::layout(lab, 11.0 * p).1;
        let lo = match t.point {
            PiPoint::Neck => [base[0] as f32 - lw - 9.0 * p, base[1] as f32 + 4.0 * p],
            PiPoint::Cusp => [base[0] as f32 - lw - 9.0 * p, base[1] as f32 - 5.0 * p],
        };
        scene.label(lo, 11.0 * p, lab, white(0.85));
        // The probe: a glow, a ring, its true position.
        let q = at(pos(t.eps));
        for (r, a) in [(18.0, 0.08), (11.0, 0.16), (6.5, 0.4)] {
            scene.disc(q, r * p, rgba(1.0, 0.78, 0.3, a));
        }
        scene.ring(q, 7.5 * p, 1.3 * p, white(0.95));
        scene.disc(q, 2.6 * p, white(1.0));
        let e = format!("{:.2e}", t.eps);
        let text = match t.point {
            PiPoint::Neck => format!("probe  c = −3/4 + {e}·i"),
            PiPoint::Cusp => format!("probe  c = 1/4 + {e}"),
        };
        let size = 12.5 * p;
        let tw = f64::from(crate::font::layout(&text, size).1);
        let o = if perp[0].abs() > perp[1].abs() {
            // Vertical ruler: the text on the other side of the ticks.
            let side = if perp[0] > 0.0 { -1.0 } else { 1.0 };
            let x = q[0] + side * 16.0 * px + if side < 0.0 { -tw } else { 0.0 };
            [x, q[1] - 0.36 * f64::from(size)]
        } else {
            let side = if perp[1] > 0.0 { -1.0 } else { 1.0 };
            [
                q[0] - 0.5 * tw,
                q[1] + side * 18.0 * px - if side < 0.0 { f64::from(size) } else { 0.0 },
            ]
        };
        let o = [o[0].clamp(4.0 * px, (w - tw - 4.0 * px).max(4.0 * px)), o[1]];
        scene.label([o[0] as f32, o[1] as f32], size, &text, gold);
        format!("probe {:.1} {:.1} {top_dec} {e} ", q[0], q[1])
    }

    /// The orbit trail inset: the orbit plot (the neck in the z-plane at the gate, the cusp's
    /// real orbit against n) and the turn circle, the glowing density picture over both, the
    /// newest iterate while the count runs. Placed above the meter's panel `panel`, or left
    /// of it when the view is too low, or not at all.
    /// Where the trail inset goes next to the meter's panel `panel` (pixels): above it, or left
    /// of it when the view is too low, or nowhere.
    /// Above: right-aligned with the panel, moved right to stay on a canvas `w` pixels wide.
    /// Never over a rect of `reserve` (a tour card, the cockpit): hidden when there is no room
    /// (an iPhone: the inset is wider than the screen).
    fn inset_rect(&self, panel: [f32; 4], p: f32, w: f32, reserve: Reserve) -> Option<[f32; 4]> {
        let l = self.trail_layout?;
        let lh = l.size().1 as f32;
        let iw = INSET_W as f32 * p;
        let ih = 2.0 * 12.0 * p + 1.9 * 11.0 * p + lh + 6.0 * p + 2.0 * 1.45 * 9.5 * p;
        let gap = 8.0 * p;
        let margin = 12.0 * p;
        let top = margin + reserve.top_right.max(reserve.top);
        let free = |r: [f32; 4], left: f32| {
            r[0] >= left && r[2] <= w - margin && r[1] >= top && !reserve.rects().any(|o| intersects(r, o))
        };
        let x0 = (panel[2] - iw).max(margin);
        let above = [x0, panel[1] - gap - ih, x0 + iw, panel[1] - gap];
        let beside = [panel[0] - gap - iw, panel[3] - ih, panel[0] - gap, panel[3]];
        if free(above, margin) {
            Some(above)
        } else if free(beside, reserve.bottom_left + margin) {
            Some(beside)
        } else {
            None
        }
    }

    /// The panel's place: the bottom-right corner (`start`: right and bottom edge, pixels) or
    /// left of a rect there, shrunk or lifted clear of the rects in `reserve` (kb:
    /// decisions/controls "π meter on narrow canvases"). Prefers the full size, then room for
    /// the trail inset (`trail`), then the place nearest the corner.
    fn place(&self, w: f32, p: f32, start: [f32; 2], size: [f32; 2], reserve: Reserve, trail: bool) -> Option<Place> {
        let keep: Vec<[f32; 4]> = reserve.rects().collect();
        let gap = 8.0 * p;
        let left = 12.0 * p;
        let top = 12.0 * p + reserve.top.max(reserve.top_right);
        let mut starts = vec![start[0]];
        starts.extend(keep.iter().map(|o| o[0] - gap).filter(|r| *r < start[0] && *r > left));
        let mut best: Option<(Place, (i32, bool, f32))> = None;
        for right in starts {
            let Some(pl) = resolve(right, start[1], size, left, top, &keep, gap) else {
                continue;
            };
            let panel = [
                pl.right - size[0] * pl.k,
                pl.bottom - size[1] * pl.k,
                pl.right,
                pl.bottom,
            ];
            let with_trail = trail && self.inset_rect(panel, p, w, reserve).is_some();
            let near = -((start[0] - pl.right) + (start[1] - pl.bottom));
            let score = ((pl.k * 20.0).round() as i32, with_trail, near);
            if best.as_ref().is_none_or(|(_, s)| score > *s) {
                best = Some((pl, score));
            }
        }
        best.map(|b| b.0)
    }

    /// No room anywhere: the corner, as small as allowed.
    fn corner(p: f32, start: [f32; 2], size: [f32; 2]) -> Place {
        let k = (start[0] - 12.0 * p) / size[0];
        let k = if k.is_finite() { k.clamp(MIN_SCALE, 1.0) } else { 1.0 };
        Place {
            right: start[0],
            bottom: start[1],
            k,
        }
    }

    fn draw_trail(
        &self,
        scene: &mut Scene,
        panel: [f32; 4],
        px: f64,
        w: f32,
        point: PiPoint,
        reserve: Reserve,
    ) -> String {
        let Some(l) = self.trail_layout else {
            return String::new();
        };
        let p = px as f32;
        let pad = 12.0 * p;
        let s_title = 11.0 * p;
        let s_cap = 9.5 * p;
        let (lw, lh) = (l.size().0 as f32, l.size().1 as f32);
        let iw = INSET_W as f32 * p;
        let Some(r) = self.inset_rect(panel, p, w, reserve) else {
            return "trail hidden ".into();
        };
        let white = |a: f32| rgba(1.0, 1.0, 1.0, a);
        let gold = rgba(1.0, 0.82, 0.32, 1.0);
        scene.rect(r, 8.0 * p, rgba(0.01, 0.01, 0.02, 0.86), white(0.22), p);
        let img = self.trail.as_ref().filter(|i| i.point == point);
        let x0 = r[0] + 0.5 * (iw - lw);
        let top = r[1] + pad;
        scene.text(
            [r[0] + pad, top],
            s_title,
            "orbit of the probe c,  n = 0 … N",
            white(0.62),
        );
        if let Some(i) = img {
            let t = if i.done {
                format!("N = {}", group(i.n))
            } else {
                format!("n = {}", group(i.n))
            };
            let tw = crate::font::layout(&t, s_title).1;
            scene.text(
                [r[2] - pad - tw, top],
                s_title,
                &t,
                if i.done { gold } else { white(0.85) },
            );
        }
        let o = [f64::from(x0), f64::from(top + 1.9 * s_title)];
        let at = |plot: usize, q: [f64; 2]| {
            let v = l.to_pixel(plot, q);
            [o[0] + v[0], o[1] + v[1]]
        };
        let (oo, to) = l.origins();
        let ob = [
            o[0] + oo[0] as f64,
            o[1] + oo[1] as f64,
            o[0] + (oo[0] + l.orbit[0]) as f64,
            o[1] + (oo[1] + l.orbit[1]) as f64,
        ];
        let tb = [
            o[0] + to[0] as f64,
            o[1] + to[1] as f64,
            o[0] + (to[0] + l.turn) as f64,
            o[1] + (to[1] + l.turn) as f64,
        ];
        let f32r = |b: [f64; 4]| [b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32];
        scene.rect(f32r(ob), 4.0 * p, rgba(0.0, 0.0, 0.0, 0.5), [0.0; 4], 0.0);
        let guide = white(0.16);
        let hw = 0.5 * p;
        let eps = img.map_or(self.target.map_or(1e-4, |t| t.eps), |i| i.eps);
        match point {
            PiPoint::Neck => {
                let c = at(0, [0.0, 0.0]);
                scene.segment([ob[0], c[1]], [ob[2], c[1]], hw, guide, guide);
                scene.segment([c[0], ob[1]], [c[0], ob[3]], hw, guide, guide);
            }
            PiPoint::Cusp => {
                let c = at(0, [-1.0, 0.0]);
                scene.segment([ob[0], c[1]], [ob[2], c[1]], hw, guide, guide);
                // √ε·n = π/2 and π (the escape): dashed verticals, labelled at the bottom.
                for (v, lab) in [(0.5, "π/2"), (1.0, "π")] {
                    let x = at(0, [2.0 * v * std::f64::consts::PI / CUSP_X - 1.0, 0.0])[0];
                    let n = 14;
                    for k in (0..n).step_by(2) {
                        let y0 = ob[1] + (ob[3] - ob[1]) * k as f64 / n as f64;
                        let y1 = ob[1] + (ob[3] - ob[1]) * (k + 1) as f64 / n as f64;
                        scene.segment([x, y0], [x, y1], hw, guide, guide);
                    }
                    let tw = crate::font::layout(lab, s_cap).1;
                    scene.text(
                        [x as f32 - 0.5 * tw, ob[3] as f32 - 1.3 * s_cap],
                        s_cap,
                        lab,
                        white(0.6),
                    );
                }
            }
        }
        // The turn circle.
        let tc = at(1, [0.0, 0.0]);
        let tr = 0.82 * 0.5 * l.turn as f64;
        scene.ring(tc, tr as f32, 0.8 * p, guide);
        let start = at(1, [1.0, 0.0]);
        let _ = tb;
        // The glowing density picture.
        if let Some(i) = img {
            scene.glow_image(
                [o[0] as f32, o[1] as f32, o[0] as f32 + lw, o[1] as f32 + lh],
                i.layout.size(),
                1.0,
            );
        }
        scene.disc(start, 2.4 * p, white(0.7));
        // The neck's fixed point and 2-cycle: the orbit passes the gate between them.
        let marks = orbit_marks(point, eps);
        for (k, m) in marks.iter().enumerate() {
            let q = at(0, *m);
            scene.ring(q, 3.2 * p, 1.0 * p, white(0.8));
            // Labels in the quadrants the arms leave free (upper right, lower left).
            let (lab, right) = match k {
                0 => ("−1/2", true),
                2 => ("2-cycle", false),
                _ => continue,
            };
            let tw = crate::font::layout(lab, s_cap).1;
            let o = if right {
                [q[0] as f32 + 6.0 * p, q[1] as f32 - 6.0 * p - s_cap]
            } else {
                [q[0] as f32 - 6.0 * p - tw, q[1] as f32 - 6.0 * p - s_cap]
            };
            scene.label(o, s_cap, lab, white(0.75));
        }
        // The newest iterate while the count runs.
        if let Some(i) = img
            && !i.done
            && let Some((ho, ht)) = i.head
        {
            for (plot, h) in [(0, ho), (1, ht)] {
                if h[0].abs() <= 1.0 && h[1].abs() <= 1.0 / 0.82 {
                    let q = at(plot, h);
                    scene.disc(q, 7.0 * p, rgba(1.0, 0.85, 0.4, 0.3));
                    scene.disc(q, 2.6 * p, white(1.0));
                }
            }
        }
        // Captions.
        let (c1, c2, c3, c4) = match point {
            PiPoint::Neck => (
                "z near −1/2, width 8√ε".to_string(),
                format!("√ε = {:.2e}", eps.sqrt()),
                "one turn: start to escape".to_string(),
                "2ε per step: N ≈ π/ε".to_string(),
            ),
            PiPoint::Cusp => (
                "(z − 1/2)/√ε against √ε·n".to_string(),
                "escape at √ε·n ≈ π".to_string(),
                "one turn: start to escape".to_string(),
                "2√ε per step: N ≈ π/√ε".to_string(),
            ),
        };
        let y = ob[3].max(tb[3]) as f32 + 6.0 * p;
        let centred = |scene: &mut Scene, cx: f64, y: f32, t: &str, c: [f32; 4]| {
            let tw = crate::font::layout(t, s_cap).1;
            scene.text([cx as f32 - 0.5 * tw, y], s_cap, t, c);
        };
        let ocx = 0.5 * (ob[0] + ob[2]);
        let tcx = 0.5 * (tb[0] + tb[2]);
        centred(scene, ocx, y, &c1, white(0.62));
        centred(scene, ocx, y + 1.45 * s_cap, &c2, white(0.5));
        centred(scene, tcx, y, &c3, white(0.62));
        centred(scene, tcx, y + 1.45 * s_cap, &c4, gold);
        format!(
            "trail {r:?} {:?} {:?} {:?} ",
            img.map(|i| i.serial),
            img.and_then(|i| i.head)
                .map(|h| [(h.0[0] * 300.0).round(), (h.0[1] * 300.0).round()]),
            img.map(|i| i.done)
        )
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

    /// Runs the flight as the app does: each refresh moves it on, then the meter follows the
    /// view; `navigating` refreshes of approach first. Returns the views and the probe's ε.
    fn fly(m: &mut PiMeter, p: PiPoint, end: f64, h: f64) -> (Vec<Viewport>, Vec<f64>, f64) {
        let mut v = p.scenic_view(1.0, h);
        m.set_layout(1600, 1000, 1.0);
        m.start_flight(p, 0.1, end);
        let (mut t, mut views, mut eps) = (0.0, Vec::new(), Vec::new());
        while m.flying() && t < 120.0 {
            t += 1.0 / 60.0;
            if let Some(nv) = m.flight_view(&v, t < 0.5, h, t, 1.6) {
                v = nv;
            }
            m.update(&v);
            views.push(v.clone());
            if let Some(pr) = m.probe() {
                eps.push(pr.eps);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        (views, eps, t)
    }

    #[test]
    fn flight_slides_the_probe_and_keeps_the_valley() {
        let h = 1000.0;
        for p in [PiPoint::Neck, PiPoint::Cusp] {
            let end = match p {
                PiPoint::Neck => 1e-6,
                PiPoint::Cusp => 1e-12,
            };
            let mut m = PiMeter::default();
            m.enabled = true;
            let (views, eps, t) = fly(&mut m, p, end, h);
            assert!(!m.flying(), "{} did not land in {t} s", p.name());
            // The probe only goes in, and lands at the end.
            assert!(eps.windows(2).all(|w| w[1] <= w[0] * (1.0 + 1e-12)), "{}", p.name());
            assert!((eps.last().unwrap() / end - 1.0).abs() < 1e-9);
            // Every view shows the pinch point, never zoomed in more than FLIGHT_ZOOM.
            let first = p.scenic_view(1.0, h);
            for v in &views {
                let [x, y] = base_offset(p, v);
                assert!(x.abs() < 800.0 && y.abs() < 500.0, "{} {x} {y}", p.name());
                let z = first.scale.ratio(v.scale);
                assert!((1.0 - 1e-9..=FLIGHT_ZOOM * (1.0 + 1e-9)).contains(&z), "{z}");
            }
            // Counts were made along the way, the last at the end; the trail came with them.
            assert!(m.history[slot(p)].len() >= 5, "{}", m.history[slot(p)].len());
            assert!((m.last.unwrap().eps / end - 1.0).abs() < 1e-9);
            let img = m.trail_picture().expect("trail");
            assert!(img.done && img.samples > 1000);
            assert!(img.bgra.chunks(4).any(|c| c[0] > 100), "dark trail");
            // The slide takes about 10 s at both points (in e-folds of N), then the hold waits
            // for the last count (slow in a debug build) and 2.5 s.
            assert!(t > 8.0 && t < 100.0, "{} {t}", p.name());
            // The probe stays after the flight (the panel keeps counting it).
            assert_eq!(m.state().point, p.name());
        }
    }

    #[test]
    fn a_move_ends_the_slide_and_esc_the_flight() {
        let h = 1000.0;
        let p = PiPoint::Neck;
        let mut m = PiMeter::default();
        m.enabled = true;
        m.start_flight(p, 0.1, 1e-8);
        let v0 = p.scenic_view(1.0, h);
        assert!(
            m.flight_view(&v0, true, h, 0.0, 1.6).is_none(),
            "approach leaves the view"
        );
        assert!(m.flight_view(&v0, false, h, 0.1, 1.6).is_none());
        let v1 = m.flight_view(&v0, false, h, 0.2, 1.6).expect("slide");
        let mut moved = v1.clone();
        moved.pan_by_points(5.0, 0.0);
        assert!(m.flight_view(&moved, false, h, 0.3, 1.6).is_none());
        assert!(!m.flying());
        assert!(m.probe().is_some(), "the probe stays");
        m.start_flight(p, 0.1, 1e-8);
        m.stop_flight();
        assert!(!m.flying());
    }

    #[test]
    fn ruler_and_ray() {
        assert_eq!(ray_to_edge([10.0, 50.0], [0.0, -1.0], [0.0, 0.0, 100.0, 100.0]), 50.0);
        assert_eq!(ray_to_edge([10.0, 50.0], [1.0, 0.0], [0.0, 0.0, 100.0, 100.0]), 90.0);
        assert_eq!(ray_to_edge([-1.0, 50.0], [1.0, 0.0], [0.0, 0.0, 100.0, 100.0]), 0.0);
    }

    /// The panel and its trail inset on narrow canvases (kb: decisions/controls "π meter on
    /// narrow canvases"): on the canvas, clear of the tour card and the cockpit, shrunk at most
    /// to MIN_SCALE; the inset hidden where it has no room.
    #[test]
    fn panel_fits_narrow_canvases_beside_the_tour_card() {
        // (name, canvas points, pixels per point, insets right / bottom / top, card, cockpit),
        // rects in points (x0, y0, x1, y1), as the iOS layout puts them.
        type Case = (
            &'static str,
            [f32; 2],
            f32,
            [f32; 3],
            Option<[f32; 4]>,
            Option<[f32; 4]>,
            bool,
        );
        let cases: [Case; 7] = [
            (
                "ipad mini portrait, tour",
                [744.0, 1133.0],
                2.0,
                [60.0, 0.0, 56.0],
                Some([332.0, 845.0, 672.0, 1065.0]),
                Some([219.0, 1077.0, 525.0, 1121.0]),
                true,
            ),
            (
                "ipad mini portrait",
                [744.0, 1133.0],
                2.0,
                [60.0, 0.0, 56.0],
                None,
                Some([219.0, 1077.0, 525.0, 1121.0]),
                true,
            ),
            (
                "ipad mini landscape, tour",
                [1133.0, 744.0],
                2.0,
                [60.0, 0.0, 56.0],
                Some([721.0, 456.0, 1061.0, 676.0]),
                Some([413.0, 688.0, 719.0, 732.0]),
                true,
            ),
            (
                "iphone portrait, tour",
                [390.0, 844.0],
                3.0,
                [56.0, 0.0, 0.0],
                Some([12.0, 12.0, 378.0, 260.0]),
                Some([40.0, 700.0, 290.0, 810.0]),
                false,
            ),
            (
                "iphone landscape, tour",
                [844.0, 390.0],
                3.0,
                [56.0, 56.0, 0.0],
                Some([12.0, 12.0, 492.0, 212.0]),
                Some([254.0, 313.0, 560.0, 357.0]),
                false,
            ),
            (
                "mac, tour",
                [1280.0, 820.0],
                2.0,
                [0.0, 0.0, 0.0],
                Some([928.0, 588.0, 1268.0, 808.0]),
                Some([441.0, 772.0, 839.0, 808.0]),
                true,
            ),
            (
                "mac",
                [1280.0, 820.0],
                2.0,
                [0.0, 0.0, 0.0],
                None,
                Some([441.0, 772.0, 839.0, 808.0]),
                true,
            ),
        ];
        for (name, [wp, hp], px, [ir, ib, it], card, cockpit, trail) in cases {
            let mut m = PiMeter::default();
            m.set_layout((wp * px) as usize, (hp * px) as usize, f64::from(px));
            let mut reserve = Reserve {
                top: it * px,
                ..Reserve::default()
            };
            let to_px = |r: [f32; 4]| r.map(|v| v * px);
            for r in card.iter().chain(cockpit.iter()) {
                reserve.keep_clear(to_px(*r));
            }
            let (w, h) = (wp * px, hp * px);
            let start = [w - (12.0 + ir) * px, h - (12.0 + ib) * px];
            let size = [316.0 * px, panel_height(px)];
            let pl = m.place(w, px, start, size, reserve, true).expect("a place");
            let panel = [
                pl.right - size[0] * pl.k,
                pl.bottom - size[1] * pl.k,
                pl.right,
                pl.bottom,
            ];
            assert!(pl.k >= MIN_SCALE && pl.k <= 1.0, "{name}: {pl:?}");
            assert!(
                panel[0] >= 12.0 * px - 0.5 && panel[1] >= 12.0 * px && panel[2] <= w && panel[3] <= h,
                "{name}: {panel:?}"
            );
            for r in reserve.rects() {
                assert!(!intersects(panel, r), "{name}: panel {panel:?} over {r:?}");
            }
            let inset = m.inset_rect(panel, px, w, reserve);
            assert_eq!(inset.is_some(), trail, "{name}: {inset:?}");
            if let Some(i) = inset {
                assert!(i[0] >= 0.0 && i[2] <= w && i[1] >= 0.0, "{name}: {i:?}");
                for r in reserve.rects() {
                    assert!(!intersects(i, r), "{name}: inset {i:?} over {r:?}");
                }
            }
            // The Mac without a tour keeps its corner, full size.
            if name == "mac" {
                assert_eq!(
                    pl,
                    Place {
                        right: start[0],
                        bottom: start[1],
                        k: 1.0
                    }
                );
            }
        }
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
        m.set_layout(1600, 1000, 1.0);
        let view = Viewport::new(-0.75, 1e-4, 1e-6);
        m.update(&view);
        assert!(m.busy());
        m.wait(&view);
        let s = m.state();
        assert_eq!((s.point.as_str(), s.count, s.digits), ("neck", 31417, 4));
        assert!(m.trail_picture().is_some_and(|i| i.done && i.n == 31417));
        let mut scene = Scene::default();
        let key = m.draw(&mut scene, &mut Vec::new(), 1600.0, 1000.0, 1.0, Reserve::default());
        assert!(key.contains("31417") && key.contains("trail ["), "{key}");
        assert!(scene.prims.len() > 50);
        // A probe in the scenic view: the ruler and the probe are drawn.
        m.set_probe(PiPoint::Neck, 1e-5);
        let v = PiPoint::Neck.scenic_view(1.0, 1000.0);
        m.update(&v);
        let mut scene = Scene::default();
        let key = m.draw(&mut scene, &mut Vec::new(), 1600.0, 1000.0, 1.0, Reserve::default());
        assert!(key.contains("probe ") && !key.contains("probe off"), "{key}");
        m.wait(&v);
        // What a frame pays when the panels change: update, build, bin at Retina size.
        m.set_layout(2880, 1800, 2.0);
        let t0 = Instant::now();
        for _ in 0..100 {
            m.update(&v);
            let mut s = Scene::default();
            m.draw(&mut s, &mut Vec::new(), 2880.0, 1800.0, 2.0, Reserve::default());
            std::hint::black_box(s.bin(2880, 1800));
        }
        let per = t0.elapsed().as_secs_f64() / 100.0;
        assert!(per < 0.004, "{per}");
        // Every character of the panels is in the font.
        for t in [
            "N·ε",
            "N·√ε",
            "|N·√ε − π|",
            "~√ε",
            "ε → 0",
            "π at the neck   c = −3/4 + iε",
            "…",
            "orbit of the probe c,  n = 0 … N",
            "one turn: start to escape",
            "2√ε per step: N ≈ π/√ε",
            "π/2",
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
        assert!(
            m.draw(&mut scene, &mut Vec::new(), 1600.0, 1000.0, 1.0, Reserve::default())
                .is_empty()
        );
        assert!(scene.prims.is_empty());
    }
}
