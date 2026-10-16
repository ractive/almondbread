//! π in the Mandelbrot set, a "maths moment" (kb: techniques/pi-in-the-mandelbrot-set).
//!
//! Boll (1991): the escape count N of `c = -3/4 + iε` (the neck) satisfies `N·ε → π`, and
//! that of `c = 1/4 + ε` (the cusp) `N·√ε → π`, with the escape test `|z_n| > 2`, `z_0 = 0`,
//! N the first n ≥ 1 that passes it (Klebanoff 2001 proves the cusp; Brockmoeller, Scherz,
//! Srkalovic 2025 the neck, with `N = π/ε + O(1)`).
//!
//! The counts are computed in coordinates centred on the parabolic fixed point, where the
//! tiny ε is kept exactly instead of being rounded into `c`:
//!
//! - neck: `z = -1/2 + w`, `w' = w² - w + iε`, `w_0 = 1/2`;
//! - cusp: `z = 1/2 + w`, `w' = w² + w + ε`, `w_0 = -1/2` (real).
//!
//! In the plain `z² + c` form, `c = 0.25 + ε` is not even representable below ε ≈ 3e-17, and
//! at the neck the slow passage amplifies the rounding of `c` (Mark VandeWettering's 2011
//! program gives a wrong count at ε = 1e-9). The shifted form runs with a compensated update
//! (the rounding error of each small step is carried along), checked against double-double
//! and a 256-bit BigFloat orbit (the kb page has the table). Counts are long (N ≈ π/ε at the
//! neck), so they run on a background thread with cancellation and progress.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::viewport::{Center, Viewport};
use almondbread_math::BigFloat;

/// The two points where π shows up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PiPoint {
    /// `c = -3/4 + iε`: `N·ε → π`.
    Neck,
    /// `c = 1/4 + ε`: `N·√ε → π`.
    Cusp,
}

impl PiPoint {
    /// The parabolic parameter itself.
    pub fn base(self) -> (f64, f64) {
        match self {
            PiPoint::Neck => (-0.75, 0.0),
            PiPoint::Cusp => (0.25, 0.0),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PiPoint::Neck => "neck",
            PiPoint::Cusp => "cusp",
        }
    }

    /// The point at distance `eps` along the line the theorem uses.
    pub fn point(self, eps: f64) -> (f64, f64) {
        match self {
            PiPoint::Neck => (-0.75, eps),
            PiPoint::Cusp => (0.25 + eps, 0.0),
        }
    }

    /// `N·ε` (neck) or `N·√ε` (cusp).
    pub fn product(self, n: u64, eps: f64) -> f64 {
        match self {
            PiPoint::Neck => n as f64 * eps,
            PiPoint::Cusp => n as f64 * eps.sqrt(),
        }
    }

    /// The count the limit predicts, π/ε or π/√ε.
    pub fn expected_count(self, eps: f64) -> f64 {
        match self {
            PiPoint::Neck => std::f64::consts::PI / eps,
            PiPoint::Cusp => std::f64::consts::PI / eps.sqrt(),
        }
    }

    /// The places-catalogue entry at this point (where the guided flight starts).
    pub fn place_id(self) -> &'static str {
        match self {
            PiPoint::Neck => "pi-at-the-neck",
            PiPoint::Cusp => "pi-at-the-cusp",
        }
    }

    pub fn named(name: &str) -> Option<PiPoint> {
        match name {
            "neck" => Some(PiPoint::Neck),
            "cusp" => Some(PiPoint::Cusp),
            _ => None,
        }
    }

    /// Where the guided flight ends: a count of a few seconds (π/ε = 3e8 at the neck, π/√ε
    /// = 3e8 at the cusp).
    pub fn flight_eps(self) -> f64 {
        match self {
            PiPoint::Neck => 1e-8,
            PiPoint::Cusp => 1e-16,
        }
    }

    /// The view centred at `eps` on the line, framed like the place: the parabolic point at
    /// a quarter (neck) or a third (cusp) of the shorter side `short_side` (view points) from
    /// the centre, so a flight between two such views zooms in about it.
    pub fn view_at(self, eps: f64, short_side: f64) -> Viewport {
        let ratio = match self {
            PiPoint::Neck => 0.25,
            PiPoint::Cusp => 1.0 / 3.0,
        };
        let (bre, bim) = self.base();
        let base = Center::new(bre, bim).with_min_precision(192);
        let center = match self {
            PiPoint::Neck => base.offset(0.0, eps),
            PiPoint::Cusp => base.offset(eps, 0.0),
        };
        let mut v = Viewport::with_center(
            center,
            crate::viewport::Scale::from_f64(eps / ratio / short_side.max(1.0)),
        );
        v.ensure_precision();
        v
    }

    /// The scenic framing of the guided flight: the valley the pinch point opens into
    /// (seahorse valley above the neck, elephant valley right of the cusp), the pinch point
    /// itself in view, `zoom` ≥ 1 times magnified about it (the flight's gentle zoom).
    /// `short_side`: the view's shorter side in points. Chosen by looking at renders
    /// (kb: techniques/pi-in-the-mandelbrot-set "In the app").
    pub fn scenic_view(self, zoom: f64, short_side: f64) -> Viewport {
        // Span of the shorter side, and the pinch point's offset from the centre in spans.
        let (span, off) = match self {
            PiPoint::Neck => (0.45, 0.29),
            PiPoint::Cusp => (0.15, 0.30),
        };
        let span = span / zoom.max(1.0);
        let (dx, dy) = match self {
            PiPoint::Neck => (0.0, off * span),
            PiPoint::Cusp => (off * span, 0.0),
        };
        let (bre, bim) = self.base();
        let c = Center::new(bre, bim).with_min_precision(64).offset(dx, dy);
        let mut v = Viewport::with_center(c, crate::viewport::Scale::from_f64(span / short_side.max(1.0)));
        v.ensure_precision();
        v
    }

    /// How fast ε falls for a growth of the expected count: ln ε per e-fold of N (the guided
    /// flight slides at the zoom speed in e-folds of N, so both points take the same time).
    pub fn eps_per_count_efold(self) -> f64 {
        match self {
            PiPoint::Neck => 1.0,
            PiPoint::Cusp => 2.0,
        }
    }

    /// The smallest ε the meter counts: the count stays below [`MAX_COUNT`] iterations.
    pub fn min_eps(self) -> f64 {
        match self {
            PiPoint::Neck => std::f64::consts::PI / MAX_COUNT as f64,
            PiPoint::Cusp => (std::f64::consts::PI / MAX_COUNT as f64).powi(2),
        }
    }
}

/// Longest count the meter runs: about 10 s on one core of an M1 Pro in double-double
/// (neck ε ≈ 3e-9, cusp ε ≈ 1e-17; kb: techniques/pi-in-the-mandelbrot-set).
pub const MAX_COUNT: u64 = 1_000_000_000;

/// The view is "at" one of the points: its centre lies on the line of approach, within 20 %
/// of ε sideways, and ε ≤ 0.1. `eps` is the centre's distance along the line (exact even at
/// depth: the difference is taken in the centre's precision).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PiTarget {
    pub point: PiPoint,
    pub eps: f64,
}

/// Largest ε the meter shows (at ε = 0.1 the product is still 3.3 and 2.5).
pub const MAX_EPS: f64 = 0.1;

/// Which point (if any) the view is centred near.
pub fn target(view: &Viewport) -> Option<PiTarget> {
    target_of(&view.center)
}

pub fn target_of(center: &Center) -> Option<PiTarget> {
    for point in [PiPoint::Neck, PiPoint::Cusp] {
        let (bre, bim) = point.base();
        let (dx, dy) = center.delta_from(&Center::new(bre, bim));
        let (along, across) = match point {
            PiPoint::Neck => (dy.abs(), dx.abs()),
            PiPoint::Cusp => (dx, dy.abs()),
        };
        if along > 0.0 && along <= MAX_EPS && across <= 0.2 * along {
            return Some(PiTarget { point, eps: along });
        }
    }
    None
}

/// Result of a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// `|z_n| > 2` first at this n.
    Escaped(u64),
    /// Not escaped within the limit.
    Limit,
    Cancelled,
}

// MARK: double-double

/// An unevaluated sum hi + lo with |lo| ≤ ulp(hi)/2 (Dekker/Knuth; about 106 bits).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dd {
    hi: f64,
    lo: f64,
}

#[inline(always)]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

#[inline(always)]
fn quick_two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    (s, b - (s - a))
}

impl Dd {
    const fn new(hi: f64) -> Self {
        Dd { hi, lo: 0.0 }
    }

    #[inline(always)]
    fn add(self, b: Dd) -> Dd {
        let (s, e) = two_sum(self.hi, b.hi);
        let (t, f) = two_sum(self.lo, b.lo);
        let (s, e) = quick_two_sum(s, e + t);
        let (hi, lo) = quick_two_sum(s, e + f);
        Dd { hi, lo }
    }

    #[inline(always)]
    fn neg(self) -> Dd {
        Dd {
            hi: -self.hi,
            lo: -self.lo,
        }
    }

    #[inline(always)]
    fn mul(self, b: Dd) -> Dd {
        let p = self.hi * b.hi;
        let e = self.hi.mul_add(b.hi, -p);
        let e = e + (self.hi * b.lo + self.lo * b.hi);
        let (hi, lo) = quick_two_sum(p, e);
        Dd { hi, lo }
    }

    #[inline(always)]
    fn twice(self) -> Dd {
        Dd {
            hi: 2.0 * self.hi,
            lo: 2.0 * self.lo,
        }
    }
}

/// Iterations between cancellation checks and progress updates.
const SLICE: u64 = 1 << 20;

/// Runs `step` (true: escaped) up to `limit` times in slices, with progress and cancellation.
#[inline(always)]
fn run(limit: u64, cancel: &AtomicBool, progress: &AtomicU64, mut step: impl FnMut(u64) -> bool) -> Count {
    let mut n: u64 = 0;
    loop {
        let end = (n + SLICE).min(limit);
        while n < end {
            n += 1;
            if step(n) {
                progress.store(n, Ordering::Relaxed);
                return Count::Escaped(n);
            }
        }
        progress.store(n, Ordering::Relaxed);
        if n >= limit {
            return Count::Limit;
        }
        if cancel.load(Ordering::Relaxed) {
            return Count::Cancelled;
        }
    }
}

/// The escape count of the point at `eps`: the meter's count. Shifted coordinates with a
/// compensated update: each step adds a small increment (`w² + ε`, or `w² + iε` after the
/// neck's sign flip) to w, and the rounding error of that addition, which would otherwise
/// pile up over the π/ε steps of the slow passage, is carried in a second f64 (TwoSum).
/// Matches the double-double and 256-bit counts (kb page). `progress` receives the
/// iterations done every 2^20; `cancel` stops it.
pub fn escape_count(point: PiPoint, eps: f64, limit: u64, cancel: &AtomicBool, progress: &AtomicU64) -> Count {
    escape_count_traced(point, eps, limit, cancel, progress, None)
}

/// [`escape_count`] that also feeds every `trail.stride`-th iterate (and the last) to the
/// orbit trail, which publishes its picture a few times per second. The check per
/// iteration is one comparison; the samples (at most [`TRAIL_SAMPLES`]) cost the rest.
pub fn escape_count_traced(
    point: PiPoint,
    eps: f64,
    limit: u64,
    cancel: &AtomicBool,
    progress: &AtomicU64,
    mut trail: Option<&mut Trail>,
) -> Count {
    let stride = trail.as_ref().map_or(u64::MAX, |t| t.stride);
    let mut next = 1u64;
    let count = match point {
        PiPoint::Neck => {
            // w = a + ib; w' = -w + (a² - b²) + i(2ab + ε); |z|² = (a - 1/2)² + b².
            let (mut ah, mut al, mut bh, mut bl) = (0.5f64, 0.0f64, 0.0f64, 0.0f64);
            run(limit, cancel, progress, |n| {
                let ia = (ah * ah - bh * bh) + 2.0 * (ah * al - bh * bl);
                let ib = (2.0 * ah * bh + eps) + 2.0 * (ah * bl + al * bh);
                let (s, e) = two_sum(-ah, ia);
                (ah, al) = quick_two_sum(s, e - al);
                let (s, e) = two_sum(-bh, ib);
                (bh, bl) = quick_two_sum(s, e - bl);
                let x = ah - 0.5;
                let out = x * x + bh * bh > 4.0;
                if n >= next || out {
                    next = next.saturating_add(stride);
                    if let Some(t) = trail.as_deref_mut() {
                        t.sample(n, ah, bh);
                    }
                }
                out
            })
        }
        PiPoint::Cusp => {
            let (mut wh, mut wl) = (-0.5f64, 0.0f64);
            run(limit, cancel, progress, |n| {
                let inc = (wh * wh + eps) + 2.0 * wh * wl;
                let (s, e) = two_sum(wh, inc);
                (wh, wl) = quick_two_sum(s, e + wl);
                let out = (wh + 0.5).abs() > 2.0;
                if n >= next || out {
                    next = next.saturating_add(stride);
                    if let Some(t) = trail.as_deref_mut() {
                        t.sample(n, wh, 0.0);
                    }
                }
                out
            })
        }
    };
    if let Some(t) = trail {
        t.publish(!matches!(count, Count::Cancelled));
    }
    count
}

/// The count in double-double shifted coordinates (about 106 bits; the accuracy check of
/// [`escape_count`], about 30 ns per iteration on an M1 Pro).
pub fn escape_count_dd(point: PiPoint, eps: f64, limit: u64) -> Option<u64> {
    let e = Dd::new(eps);
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    let count = match point {
        PiPoint::Neck => {
            let (mut a, mut b) = (Dd::new(0.5), Dd::new(0.0));
            run(limit, &cancel, &progress, |_| {
                let na = a.mul(a).add(b.mul(b).neg()).add(a.neg());
                let nb = a.mul(b).twice().add(b.neg()).add(e);
                a = na;
                b = nb;
                let x = a.hi - 0.5;
                x * x + b.hi * b.hi > 4.0
            })
        }
        PiPoint::Cusp => {
            let mut w = Dd::new(-0.5);
            run(limit, &cancel, &progress, |_| {
                w = w.mul(w).add(w).add(e);
                (w.hi + 0.5).abs() > 2.0
            })
        }
    };
    match count {
        Count::Escaped(n) => Some(n),
        _ => None,
    }
}

/// The same count in plain f64 in the shifted coordinates (accuracy comparisons).
pub fn escape_count_f64(point: PiPoint, eps: f64, limit: u64) -> Option<u64> {
    match point {
        PiPoint::Neck => {
            let (mut a, mut b) = (0.5f64, 0.0f64);
            for n in 1..=limit {
                let na = a * a - b * b - a;
                b = 2.0 * a * b - b + eps;
                a = na;
                if (a - 0.5) * (a - 0.5) + b * b > 4.0 {
                    return Some(n);
                }
            }
            None
        }
        PiPoint::Cusp => {
            let mut w = -0.5f64;
            for n in 1..=limit {
                w = w * w + w + eps;
                if (w + 0.5).abs() > 2.0 {
                    return Some(n);
                }
            }
            None
        }
    }
}

/// The count in the textbook form `z' = z² + c`, f64, c rounded to f64 (how most programs do
/// it; accuracy comparisons).
pub fn escape_count_naive(point: PiPoint, eps: f64, limit: u64) -> Option<u64> {
    let (cr, ci) = point.point(eps);
    let (mut x, mut y) = (0.0f64, 0.0f64);
    for n in 1..=limit {
        let nx = x * x - y * y + cr;
        y = 2.0 * x * y + ci;
        x = nx;
        if x * x + y * y > 4.0 {
            return Some(n);
        }
    }
    None
}

/// The count in `bits`-bit BigFloat in the textbook form with c exact (the reference; slow,
/// about a microsecond per iteration).
pub fn escape_count_big(point: PiPoint, eps: f64, bits: usize, limit: u64) -> Option<u64> {
    let (bre, bim) = point.base();
    let e = BigFloat::from_f64(eps, bits);
    let (cr, ci) = match point {
        PiPoint::Neck => (BigFloat::from_f64(bre, bits), e),
        PiPoint::Cusp => (&BigFloat::from_f64(bre, bits) + &e, BigFloat::from_f64(bim, bits)),
    };
    let four = BigFloat::from_f64(4.0, bits);
    let (mut x, mut y) = (BigFloat::zero(bits), BigFloat::zero(bits));
    for n in 1..=limit {
        let x2 = x.square();
        let y2 = y.square();
        if n > 1 && (&x2 + &y2).compare(&four) == std::cmp::Ordering::Greater {
            return Some(n - 1);
        }
        let xy = x.mul_ref(&y);
        x = &(&x2 - &y2) + &cr;
        y = &xy.times_power_of_two(1) + &ci;
    }
    let r2 = (&x.square() + &y.square()).compare(&four);
    (r2 == std::cmp::Ordering::Greater).then_some(limit)
}

// MARK: The orbit trail

/// Most iterates a count feeds to its trail (every k-th, k = N / this; a density picture of a
/// few hundred thousand pixels needs no more).
pub const TRAIL_SAMPLES: f64 = 4.0e6;

/// Half-width of the neck's orbit plot in units of √ε (the gate between the fixed point and
/// the 2-cycle, at distance √ε, with the incoming and outgoing arms).
pub const NECK_SPAN: f64 = 4.0;
/// The cusp's orbit plot: √ε·n from 0 to this (π is the escape), and (z − 1/2)/√ε up to
/// ± [`CUSP_Y`].
pub const CUSP_X: f64 = 1.1 * std::f64::consts::PI;
pub const CUSP_Y: f64 = 6.0;

/// Where iterate n (shifted coordinates, w = z ∓ 1/2) falls on the orbit plot, x and y in
/// [-1, 1] (y up): the neck in the z-plane around −1/2 at ±[`NECK_SPAN`]·√ε; the cusp's real
/// orbit against n, `(z_n − 1/2)/√ε` over `√ε·n` (a tangent curve: `√ε·tan(√ε n − π/2)`,
/// kb: techniques/pi-in-the-mandelbrot-set "The orbit").
#[inline]
pub fn orbit_plot(point: PiPoint, eps: f64, n: u64, a: f64, b: f64) -> [f64; 2] {
    let s = eps.sqrt();
    match point {
        PiPoint::Neck => [a / (NECK_SPAN * s), b / (NECK_SPAN * s)],
        PiPoint::Cusp => [2.0 * s * n as f64 / CUSP_X - 1.0, a / (CUSP_Y * s)],
    }
}

/// The iterate in the "turning" coordinate, on the unit circle up to O(√ε): neck
/// `1 + iε/w²`, cusp `−(√ε + iw)²/(ε + w²)`. The orbit goes round it once at a constant
/// angle per step (2ε at the neck, 2√ε at the cusp), starting and escaping at 1: one turn is
/// 2π, so N ≈ π/ε (π/√ε). Derivation on the kb page.
#[inline]
pub fn turn_plot(point: PiPoint, eps: f64, a: f64, b: f64) -> [f64; 2] {
    match point {
        PiPoint::Neck => {
            let (x, y) = (a * a - b * b, 2.0 * a * b);
            let m = (x * x + y * y).max(1e-300);
            [1.0 + eps * y / m, eps * x / m]
        }
        PiPoint::Cusp => {
            let d = eps + a * a;
            [-(eps - a * a) / d, -2.0 * eps.sqrt() * a / d]
        }
    }
}

/// The marked points of the orbit plot (plot coordinates): the neck's fixed point
/// `−1/2 + iε/2` and its 2-cycle `−1/2 ± √ε·e^(−iπ/4)` (the orbit passes the gate between
/// them); none on the cusp's graph.
pub fn orbit_marks(point: PiPoint, eps: f64) -> Vec<[f64; 2]> {
    match point {
        PiPoint::Neck => {
            let r = std::f64::consts::FRAC_1_SQRT_2 / NECK_SPAN;
            vec![[0.0, 0.5 * eps.sqrt() / NECK_SPAN], [r, -r], [-r, r]]
        }
        PiPoint::Cusp => Vec::new(),
    }
}

/// Pixel layout of the trail picture: the orbit plot on the left, the turn circle (square)
/// on the right, `gap` apart, both vertically centred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrailLayout {
    pub orbit: [usize; 2],
    pub turn: usize,
    pub gap: usize,
}

impl TrailLayout {
    pub fn size(&self) -> (usize, usize) {
        (self.orbit[0] + self.gap + self.turn, self.orbit[1].max(self.turn))
    }

    /// Top-left pixel of the orbit plot and the turn plot.
    pub fn origins(&self) -> ([usize; 2], [usize; 2]) {
        let h = self.size().1;
        (
            [0, (h - self.orbit[1]) / 2],
            [self.orbit[0] + self.gap, (h - self.turn) / 2],
        )
    }

    /// Pixel position (fractional, y down) of plot coordinates on plot 0 (orbit) or 1 (turn);
    /// the turn plot shows the unit circle at 0.82 of its half-size.
    pub fn to_pixel(&self, plot: usize, p: [f64; 2]) -> [f64; 2] {
        let (o, t) = self.origins();
        if plot == 0 {
            let (w, h) = (self.orbit[0] as f64, self.orbit[1] as f64);
            [
                o[0] as f64 + (p[0] * 0.5 + 0.5) * w,
                o[1] as f64 + (0.5 - p[1] * 0.5) * h,
            ]
        } else {
            let s = self.turn as f64;
            let k = 0.82 * 0.5;
            [t[0] as f64 + (0.5 + p[0] * k) * s, t[1] as f64 + (0.5 - p[1] * k) * s]
        }
    }
}

/// A published picture of the trail: sRGB-encoded BGRA8 (the overlay adds it to the image,
/// so black is transparent), the newest iterate on both plots, how far the count got.
#[derive(Clone, Debug, PartialEq)]
pub struct TrailImage {
    /// Unique over all trails (the overlay uploads a picture when this changes).
    pub serial: u64,
    pub point: PiPoint,
    pub eps: f64,
    pub layout: TrailLayout,
    pub bgra: Vec<u8>,
    /// The newest iterate in plot coordinates (orbit plot, turn plot).
    pub head: Option<([f64; 2], [f64; 2])>,
    /// Iterations done and iterates drawn.
    pub n: u64,
    pub samples: u64,
    /// The count ended (escaped or reached its limit).
    pub done: bool,
}

/// The newest picture of a running trail, shared with the frame.
pub type TrailSlot = std::sync::Arc<std::sync::Mutex<Option<TrailImage>>>;

static TRAIL_SERIAL: AtomicU64 = AtomicU64::new(1);

/// The orbit's density on both plots, accumulated on the counting thread: per pixel the
/// number of iterates and the sum of their progress n/N (the colour); a tone-mapped picture
/// with a soft glow is published to `slot` every [`Trail::PUBLISH_EVERY`] seconds.
pub struct Trail {
    point: PiPoint,
    eps: f64,
    layout: TrailLayout,
    /// Every stride-th iterate is drawn.
    pub stride: u64,
    count: Vec<f32>,
    tsum: Vec<f32>,
    inv_expected: f64,
    head: Option<([f64; 2], [f64; 2])>,
    n: u64,
    samples: u64,
    slot: TrailSlot,
    last_publish: std::time::Instant,
}

impl Trail {
    pub const PUBLISH_EVERY: f64 = 0.12;

    pub fn new(point: PiPoint, eps: f64, layout: TrailLayout, slot: TrailSlot) -> Self {
        let (w, h) = layout.size();
        let expected = point.expected_count(eps);
        let mut stride = (expected / TRAIL_SAMPLES).ceil().max(1.0) as u64;
        // The neck's even and odd iterates lie on opposite arms: an odd stride keeps both.
        if point == PiPoint::Neck && stride > 1 && stride.is_multiple_of(2) {
            stride += 1;
        }
        Trail {
            point,
            eps,
            layout,
            stride,
            count: vec![0.0; w * h],
            tsum: vec![0.0; w * h],
            inv_expected: 1.0 / expected,
            head: None,
            n: 0,
            samples: 0,
            slot,
            last_publish: std::time::Instant::now(),
        }
    }

    #[inline]
    fn add(&mut self, plot: usize, p: [f64; 2], t: f32) {
        // Each plot keeps to its own box (the neck's arms run far out along the axes).
        let lim = if plot == 0 { 1.0 } else { 1.0 / 0.82 };
        if !(p[0].abs() < lim && p[1].abs() < lim) {
            return;
        }
        let q = self.layout.to_pixel(plot, p);
        let (w, h) = self.layout.size();
        if q[0] >= 0.0 && q[1] >= 0.0 && q[0] < w as f64 && q[1] < h as f64 {
            let i = q[1] as usize * w + q[0] as usize;
            self.count[i] += 1.0;
            self.tsum[i] += t;
        }
    }

    /// Iterate n at w = a + ib (shifted coordinates).
    #[inline]
    pub fn sample(&mut self, n: u64, a: f64, b: f64) {
        let t = (n as f64 * self.inv_expected).min(1.0) as f32;
        let o = orbit_plot(self.point, self.eps, n, a, b);
        let c = turn_plot(self.point, self.eps, a, b);
        self.add(0, o, t);
        self.add(1, c, t);
        self.head = Some((o, c));
        self.n = n;
        self.samples += 1;
        if self.samples.is_multiple_of(4096) && self.last_publish.elapsed().as_secs_f64() >= Self::PUBLISH_EVERY {
            self.publish(false);
        }
    }

    /// Tone-maps the density into the shared picture.
    pub fn publish(&mut self, done: bool) {
        self.last_publish = std::time::Instant::now();
        let bgra = self.render();
        let img = TrailImage {
            serial: TRAIL_SERIAL.fetch_add(1, Ordering::Relaxed),
            point: self.point,
            eps: self.eps,
            layout: self.layout,
            bgra,
            head: self.head,
            n: self.n,
            samples: self.samples,
            done,
        };
        if let Ok(mut s) = self.slot.lock() {
            *s = Some(img);
        }
    }

    /// The picture: brightness log(1 + count) per plot (each plot normalised to its own
    /// densest pixel, never fully dark where an iterate fell), colour by mean progress
    /// (early blue, the crawl gold, the escape rose), plus a blurred copy as glow.
    pub fn render(&self) -> Vec<u8> {
        let (w, h) = self.layout.size();
        let split = self.layout.orbit[0] + self.layout.gap / 2;
        let mut max = [0.0f32; 2];
        for y in 0..h {
            for x in 0..w {
                let k = usize::from(x >= split);
                max[k] = max[k].max(self.count[y * w + x]);
            }
        }
        let lmax = [(1.0 + max[0]).ln().max(1e-6), (1.0 + max[1]).ln().max(1e-6)];
        let mut lin = vec![[0.0f32; 3]; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let c = self.count[i];
                if c <= 0.0 {
                    continue;
                }
                let k = usize::from(x >= split);
                let v = 0.18 + 0.82 * ((1.0 + c).ln() / lmax[k]).powf(0.8);
                let col = trail_color(self.tsum[i] / c);
                lin[i] = [col[0] * v, col[1] * v, col[2] * v];
            }
        }
        let r = (w.min(h) / 60).max(2);
        let glow = box_blur(&box_blur(&lin, w, h, r), w, h, r);
        let mut out = vec![0u8; w * h * 4];
        for i in 0..w * h {
            let mut px = [0.0f32; 3];
            for (k, p) in px.iter_mut().enumerate() {
                *p = (lin[i][k] + 2.2 * glow[i][k]).min(1.0);
            }
            out[i * 4] = srgb8(px[2]);
            out[i * 4 + 1] = srgb8(px[1]);
            out[i * 4 + 2] = srgb8(px[0]);
            out[i * 4 + 3] = 255;
        }
        out
    }
}

/// Trail colour (linear RGB) by progress t = n/N: deep blue, cyan, gold, rose.
pub fn trail_color(t: f32) -> [f32; 3] {
    const STOPS: [[f32; 3]; 4] = [
        [0.10, 0.25, 1.0],
        [0.15, 0.75, 1.0],
        [1.0, 0.72, 0.18],
        [1.0, 0.22, 0.30],
    ];
    let x = t.clamp(0.0, 1.0) * 3.0;
    let k = (x as usize).min(2);
    let f = x - k as f32;
    let (a, b) = (STOPS[k], STOPS[k + 1]);
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

fn srgb8(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

/// Separable box blur of radius r (running sums, edges clamped to zero).
fn box_blur(src: &[[f32; 3]], w: usize, h: usize, r: usize) -> Vec<[f32; 3]> {
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut tmp = vec![[0.0f32; 3]; w * h];
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let mut acc = [0.0f32; 3];
        for x in 0..(r + 1).min(w) {
            for k in 0..3 {
                acc[k] += row[x][k];
            }
        }
        for x in 0..w {
            for k in 0..3 {
                tmp[y * w + x][k] = acc[k] * norm;
            }
            if x + r + 1 < w {
                for k in 0..3 {
                    acc[k] += row[x + r + 1][k];
                }
            }
            if x >= r {
                for k in 0..3 {
                    acc[k] -= row[x - r][k];
                }
            }
        }
    }
    let mut out = vec![[0.0f32; 3]; w * h];
    for x in 0..w {
        let mut acc = [0.0f32; 3];
        for y in 0..(r + 1).min(h) {
            for k in 0..3 {
                acc[k] += tmp[y * w + x][k];
            }
        }
        for y in 0..h {
            for k in 0..3 {
                out[y * w + x][k] = acc[k] * norm;
            }
            if y + r + 1 < h {
                for k in 0..3 {
                    acc[k] += tmp[(y + r + 1) * w + x][k];
                }
            }
            if y >= r {
                for k in 0..3 {
                    acc[k] -= tmp[(y - r) * w + x][k];
                }
            }
        }
    }
    out
}

/// π to 20 decimals.
pub const PI_TEXT: &str = "3.14159265358979323846";

/// Leading characters of `text` (a decimal like "3.14157") that agree with π, and how many of
/// them are digits ("3.1416" → (5, 4)).
pub fn matching_prefix(text: &str) -> (usize, usize) {
    let mut chars = 0;
    let mut digits = 0;
    for (a, b) in text.chars().zip(PI_TEXT.chars()) {
        if a != b {
            break;
        }
        chars += 1;
        if a.is_ascii_digit() {
            digits += 1;
        }
    }
    (chars, digits)
}

/// The product as shown: enough decimals for the digits that can agree (about
/// -log10(error) + 2), at most 12.
pub fn product_text(value: f64, error_scale: f64) -> String {
    let decimals = if error_scale > 0.0 && error_scale.is_finite() {
        ((-error_scale.log10()).ceil() as i64 + 2).clamp(3, 12) as usize
    } else {
        6
    };
    format!("{value:.decimals$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(point: PiPoint, eps: f64) -> u64 {
        match escape_count(point, eps, MAX_COUNT, &AtomicBool::new(false), &AtomicU64::new(0)) {
            Count::Escaped(n) => n,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn boll_tables() {
        // Boll's counts as quoted by Klebanoff (2001), Fractals 9(4) p. 394.
        for (k, n) in [(1, 33), (2, 315), (3, 3143), (4, 31417), (5, 314160)] {
            assert_eq!(count(PiPoint::Neck, 10f64.powi(-k)), n, "neck 1e-{k}");
        }
        for (k, n) in [(1, 8), (2, 30), (3, 97), (4, 312), (5, 991), (6, 3140), (8, 31414)] {
            assert_eq!(count(PiPoint::Cusp, 10f64.powi(-k)), n, "cusp 1e-{k}");
        }
    }

    #[test]
    fn shifted_forms_match_the_reference() {
        for eps in [3e-2, 1e-3, 7.7e-4] {
            let big = escape_count_big(PiPoint::Neck, eps, 192, 1 << 20);
            assert_eq!(Some(count(PiPoint::Neck, eps)), big, "neck {eps}");
        }
        for eps in [1e-4, 2.5e-7] {
            let big = escape_count_big(PiPoint::Cusp, eps, 192, 1 << 20);
            assert_eq!(Some(count(PiPoint::Cusp, eps)), big, "cusp {eps}");
        }
    }

    #[test]
    fn targets_and_digits() {
        let t = target(&Viewport::new(-0.75, 1e-5, 1e-7)).unwrap();
        assert_eq!(t.point, PiPoint::Neck);
        assert!((t.eps - 1e-5).abs() < 1e-20);
        let t = target(&Viewport::new(0.25 + 1e-6, 0.0, 1e-8)).unwrap();
        assert_eq!(t.point, PiPoint::Cusp);
        assert!(target(&Viewport::new(-0.5, 0.0, 1e-3)).is_none());
        assert!(target(&Viewport::new(0.25 - 1e-6, 0.0, 1e-8)).is_none());
        assert_eq!(matching_prefix("3.1416"), (5, 4));
        assert_eq!(matching_prefix("3.14157"), (6, 5));
        assert_eq!(matching_prefix("2.9"), (0, 0));
        assert_eq!(product_text(std::f64::consts::PI, 1e-5), "3.1415927");
    }

    #[test]
    fn flight_views_frame_the_point_alike() {
        let h = 800.0;
        for (p, ratio) in [(PiPoint::Neck, 0.25), (PiPoint::Cusp, 1.0 / 3.0)] {
            for eps in [0.02, 1e-5, p.flight_eps()] {
                let v = p.view_at(eps, h);
                let t = target(&v).unwrap();
                assert_eq!(t.point, p);
                assert!((t.eps / eps - 1.0).abs() < 1e-9, "{} {eps} {}", p.name(), t.eps);
                let (bre, bim) = p.base();
                let (dx, dy) = Center::new(bre, bim).delta_in(&v.center, v.scale);
                assert!((dx.hypot(dy) / h - ratio).abs() < 1e-6, "{} {eps}", p.name());
            }
        }
    }

    #[test]
    fn trail_turns_once_and_does_not_change_the_count() {
        // The turning coordinate stays on the unit circle (up to O(√ε)) and goes round once:
        // its angle, unwrapped, grows by 2π over the orbit (kb page "The orbit").
        for (point, eps) in [(PiPoint::Neck, 1e-5), (PiPoint::Cusp, 1e-10)] {
            let (mut a, mut b) = match point {
                PiPoint::Neck => (0.5f64, 0.0f64),
                PiPoint::Cusp => (-0.5, 0.0),
            };
            let (mut total, mut prev) = (0.0f64, 0.0f64);
            let mut worst = 0.0f64;
            for n in 1..u64::MAX {
                match point {
                    PiPoint::Neck => {
                        let na = a * a - b * b - a;
                        b = 2.0 * a * b - b + eps;
                        a = na;
                    }
                    PiPoint::Cusp => a = a * a + a + eps,
                }
                let out = match point {
                    PiPoint::Neck => (a - 0.5) * (a - 0.5) + b * b > 4.0,
                    PiPoint::Cusp => (a + 0.5).abs() > 2.0,
                };
                if out {
                    break;
                }
                let q = turn_plot(point, eps, a, b);
                let ang = q[1].atan2(q[0]);
                if n > 1 {
                    let mut d = ang - prev;
                    d -= std::f64::consts::TAU * (d / std::f64::consts::TAU).round();
                    total += d;
                }
                prev = ang;
                // Far from the gate the circle is only approximate; inside it, close.
                if n as f64 > 0.05 * point.expected_count(eps) && (n as f64) < 0.95 * point.expected_count(eps) {
                    worst = worst.max((q[0].hypot(q[1]) - 1.0).abs());
                }
            }
            assert!(
                (total.abs() / std::f64::consts::TAU - 1.0).abs() < 0.05,
                "{} {total}",
                point.name()
            );
            assert!(worst < 0.05, "{} {worst}", point.name());
        }
        // Tracing changes nothing about the count, and the picture shows something.
        for (point, eps) in [(PiPoint::Neck, 3e-6), (PiPoint::Cusp, 1e-11)] {
            let slot: TrailSlot = std::sync::Arc::new(std::sync::Mutex::new(None));
            let layout = TrailLayout {
                orbit: [120, 100],
                turn: 80,
                gap: 10,
            };
            let mut trail = Trail::new(point, eps, layout, slot.clone());
            let (c, p) = (AtomicBool::new(false), AtomicU64::new(0));
            let traced = escape_count_traced(point, eps, MAX_COUNT, &c, &p, Some(&mut trail));
            assert_eq!(traced, Count::Escaped(count(point, eps)));
            let img = slot.lock().unwrap().take().expect("published");
            assert!(img.done && img.n == count(point, eps));
            let lit = img.bgra.chunks(4).filter(|px| px[0].max(px[1]).max(px[2]) > 40).count();
            assert!(lit > 500, "{} {lit}", point.name());
        }
    }

    #[test]
    fn cancellation_and_limit() {
        let p = AtomicU64::new(0);
        assert_eq!(
            escape_count(PiPoint::Neck, 1e-9, 1000, &AtomicBool::new(false), &p),
            Count::Limit
        );
        assert_eq!(
            escape_count(PiPoint::Neck, 1e-9, MAX_COUNT, &AtomicBool::new(true), &p),
            Count::Cancelled
        );
        assert_eq!(p.load(Ordering::Relaxed), SLICE);
        // The compensated count agrees with double-double where f64 in the textbook form is
        // off by thousands (cusp 1e-14).
        for (point, eps) in [(PiPoint::Cusp, 1e-14), (PiPoint::Neck, 3.3e-7)] {
            assert_eq!(Some(count(point, eps)), escape_count_dd(point, eps, MAX_COUNT), "{eps}");
        }
    }
}
