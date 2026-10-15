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
fn run(limit: u64, cancel: &AtomicBool, progress: &AtomicU64, mut step: impl FnMut() -> bool) -> Count {
    let mut n: u64 = 0;
    loop {
        let end = (n + SLICE).min(limit);
        while n < end {
            n += 1;
            if step() {
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
    match point {
        PiPoint::Neck => {
            // w = a + ib; w' = -w + (a² - b²) + i(2ab + ε); |z|² = (a - 1/2)² + b².
            let (mut ah, mut al, mut bh, mut bl) = (0.5f64, 0.0f64, 0.0f64, 0.0f64);
            run(limit, cancel, progress, || {
                let ia = (ah * ah - bh * bh) + 2.0 * (ah * al - bh * bl);
                let ib = (2.0 * ah * bh + eps) + 2.0 * (ah * bl + al * bh);
                let (s, e) = two_sum(-ah, ia);
                (ah, al) = quick_two_sum(s, e - al);
                let (s, e) = two_sum(-bh, ib);
                (bh, bl) = quick_two_sum(s, e - bl);
                let x = ah - 0.5;
                x * x + bh * bh > 4.0
            })
        }
        PiPoint::Cusp => {
            let (mut wh, mut wl) = (-0.5f64, 0.0f64);
            run(limit, cancel, progress, || {
                let inc = (wh * wh + eps) + 2.0 * wh * wl;
                let (s, e) = two_sum(wh, inc);
                (wh, wl) = quick_two_sum(s, e + wl);
                (wh + 0.5).abs() > 2.0
            })
        }
    }
}

/// The count in double-double shifted coordinates (about 106 bits; the accuracy check of
/// [`escape_count`], about 30 ns per iteration on an M1 Pro).
pub fn escape_count_dd(point: PiPoint, eps: f64, limit: u64) -> Option<u64> {
    let e = Dd::new(eps);
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    let count = match point {
        PiPoint::Neck => {
            let (mut a, mut b) = (Dd::new(0.5), Dd::new(0.0));
            run(limit, &cancel, &progress, || {
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
            run(limit, &cancel, &progress, || {
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
    fn guided_flight_stays_on_the_line() {
        use crate::input::{InputEvent, Navigator};
        let (w, h) = (1280.0, 800.0);
        for (p, eps0) in [(PiPoint::Neck, 0.02), (PiPoint::Cusp, 2e-4)] {
            let start = p.view_at(eps0, h);
            let target_view = p.view_at(p.flight_eps(), h);
            let mut v = start.clone();
            let mut nav = Navigator::default();
            nav.handle(InputEvent::Resize { width: w, height: h }, &mut v, 0.0);
            nav.fly_to(&target_view, &start, 0.0);
            let mut t = 0.0;
            let mut frames = 0;
            let mut far = 0.0f64;
            while nav.is_flying() && t < 300.0 {
                t += 1.0 / 60.0;
                nav.advance(&mut v, t);
                frames += 1;
                let tg = target(&v).unwrap_or_else(|| panic!("{} left the line at t {t}", p.name()));
                assert_eq!(tg.point, p);
                // The parabolic point stays near the frame (the zoom is about it).
                let (bre, bim) = p.base();
                let (dx, dy) = Center::new(bre, bim).delta_in(&v.center, v.scale);
                far = far.max(dx.hypot(dy) / h);
            }
            let tg = target(&v).unwrap();
            assert!((tg.eps / p.flight_eps() - 1.0).abs() < 1e-6, "{} {}", p.name(), tg.eps);
            assert!(frames > 300, "{frames}");
            assert!(far < 3.0, "{} base point {far} view heights away", p.name());
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
