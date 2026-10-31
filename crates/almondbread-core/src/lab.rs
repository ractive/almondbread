//! The parameter lab (F10, kb: plans/f10-parameter-lab): `z ← zᵖ + c` with three complex knobs,
//! the start `z₀`, the parameter `c` and the exponent `p` (2swap, "Mandelbrot's Evil Twin"). A
//! picture fixes two of them and lets the pixel be the third:
//!
//! - [`Plane::C`]: pixel = c, a Mandelbrot-like set (z₀ ≠ 0 and p ≠ 2 allowed);
//! - [`Plane::Z0`]: pixel = z₀, a Julia-like set;
//! - [`Plane::P`]: pixel = p, the "X-set" (z₀ and c fixed, the exponent varies per pixel).
//!
//! `p` may be non-integer or complex: `zᵖ = exp(p · Log z)` with the principal branch of the
//! logarithm (`arg z ∈ (−π, π]`), so pictures with a non-integer p have a seam along the negative
//! real axis of z (where the branch jumps); that is the formula, not a bug. An integer p of 1 ... 16
//! (outside the p plane) takes repeated multiplication instead, the same value without the branch
//! and faster. `0ᵖ` is 0 for `Re p > 0` and infinite (escaped) otherwise.
//!
//! Shallow by design: direct iteration (no perturbation: non-integer and complex powers do not
//! reduce, kb: formulas/others-feasibility), float32 on the GPU. Zoom stops at [`min_pixel`].
//! This module is the platform-free part: the values, the planes' framings and limits, the
//! random and reset values, and an f64 reference of the iteration (the GPU kernel's tests, the
//! orbit overlay).

use rayon::prelude::*;

use crate::oracle::{ITER_INTERIOR, IterationField};
use crate::viewport::{SetBox, ViewLimits, Viewport};

/// Which knob the pixel varies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Plane {
    /// pixel = c: Mandelbrot-like.
    #[default]
    C,
    /// pixel = z₀: Julia-like.
    Z0,
    /// pixel = p: the X-set.
    P,
}

impl Plane {
    pub const ALL: [Plane; 3] = [Plane::C, Plane::Z0, Plane::P];

    pub fn name(self) -> &'static str {
        match self {
            Plane::C => "c",
            Plane::Z0 => "z0",
            Plane::P => "p",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "c" => Some(Plane::C),
            "z0" | "z₀" | "z" => Some(Plane::Z0),
            "p" | "x" => Some(Plane::P),
            _ => None,
        }
    }

    /// The knob's name in labels ("c", "z₀", "p").
    pub fn symbol(self) -> &'static str {
        match self {
            Plane::C => "c",
            Plane::Z0 => "z₀",
            Plane::P => "p",
        }
    }

    /// The plane's picture in words (panel titles).
    pub fn title(self) -> &'static str {
        match self {
            Plane::C => "c plane",
            Plane::Z0 => "z₀ plane",
            Plane::P => "p plane",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Plane::C => 0,
            Plane::Z0 => 1,
            Plane::P => 2,
        }
    }

    /// The fixed framing of the plane's panel and home view: centre and the span (re, im) a view
    /// shows at least (chosen by looking at the planes for the default and random values).
    pub fn frame(self) -> ((f64, f64), (f64, f64)) {
        match self {
            // The Mandelbrot set spans re −2 .. 0.47; higher powers and other starts stay
            // within |c| ≤ 2 mostly.
            Plane::C => ((-0.5, 0.0), (4.2, 3.0)),
            // Julia-like sets of a c in the set lie in |z| ≤ 2.
            Plane::Z0 => ((0.0, 0.0), (4.2, 3.0)),
            // The X-set: p from about −1 to 6 (Re p < 1 hardly escapes, large Re p is a disc).
            Plane::P => ((2.5, 0.0), (8.4, 6.0)),
        }
    }

    /// The box the view centre stays in (kb: decisions/controls "View limits"): the framing
    /// with a margin, so a view never shows only empty plane.
    pub fn bounds(self) -> SetBox {
        let ((cx, cy), (sx, sy)) = self.frame();
        SetBox::new(cx - 0.6 * sx, cx + 0.6 * sx, cy - 0.6 * sy, cy + 0.6 * sy)
    }

    /// The ranges the sliders cover (and values are clamped to): re and im.
    pub fn range(self) -> ((f64, f64), (f64, f64)) {
        match self {
            Plane::C => ((-2.5, 2.5), (-2.5, 2.5)),
            Plane::Z0 => ((-2.5, 2.5), (-2.5, 2.5)),
            Plane::P => ((-4.0, 10.0), (-4.0, 4.0)),
        }
    }
}

/// The three knobs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabValues {
    pub z0: (f64, f64),
    pub c: (f64, f64),
    pub p: (f64, f64),
}

impl Default for LabValues {
    /// Reset: z₀ = 0, c = −0.75, p = 2 (the Mandelbrot set's neck, where the three planes are
    /// the familiar pictures).
    fn default() -> Self {
        Self {
            z0: (0.0, 0.0),
            c: (-0.75, 0.0),
            p: (2.0, 0.0),
        }
    }
}

impl LabValues {
    pub fn get(&self, plane: Plane) -> (f64, f64) {
        match plane {
            Plane::C => self.c,
            Plane::Z0 => self.z0,
            Plane::P => self.p,
        }
    }

    /// Sets one knob, clamped into its range (non-finite parts are ignored).
    pub fn set(&mut self, plane: Plane, v: (f64, f64)) {
        let ((r0, r1), (i0, i1)) = plane.range();
        let old = self.get(plane);
        let re = if v.0.is_finite() { v.0.clamp(r0, r1) } else { old.0 };
        let im = if v.1.is_finite() { v.1.clamp(i0, i1) } else { old.1 };
        match plane {
            Plane::C => self.c = (re, im),
            Plane::Z0 => self.z0 = (re, im),
            Plane::P => self.p = (re, im),
        }
    }

    /// The same values with every knob in its range.
    pub fn sanitized(self) -> Self {
        let mut v = Self::default();
        for plane in Plane::ALL {
            v.set(plane, self.get(plane));
        }
        v
    }
}

/// Escape radius: `|z| > 256`. Smaller than the Mandelbrot kernels' 2¹⁶ because `zᵖ` of a
/// large z overflows float32 for large `Re p` (|z|ᵖ with |z| = 2¹⁶ and p = 10 is 2¹⁶⁰); 256 keeps
/// one more step finite up to `Re p ≈ 16` and is ample for the smooth count.
pub const BAILOUT: f64 = 256.0;

/// The automatic iteration limit at a pixel size of 10⁻³ (about the home views) and its growth
/// per decade of zoom past that, up to [`MAX_AUTO_ITERATIONS`]. Shallow pictures need few: at
/// the home views the boundary's slow pixels escape within a few hundred steps, and interior
/// pixels that no cycle check catches cost the full limit each.
pub const BASE_ITERATIONS: usize = 300;
pub const ITERATIONS_PER_DECADE: f64 = 150.0;
pub const MAX_AUTO_ITERATIONS: usize = 2000;

/// An orbit that comes back within this distance of the value it had at the last power-of-two
/// step is periodic: interior (Brent's cycle detection; it ends interior samples early).
pub const CYCLE_EPS: f64 = 1e-6;

/// The smallest `Re p` the smooth count's base takes (see [`smooth_offset`]).
pub const MIN_SMOOTH_BASE: f64 = 1.5;

/// Iterations of the lab for the settings and the pixel size (plane units): a manual limit when
/// automatic iterations are off, else [`BASE_ITERATIONS`] plus [`ITERATIONS_PER_DECADE`] per
/// decade of pixel size below 10⁻³.
pub fn iterations(auto: bool, manual: usize, pixel: f64) -> usize {
    if auto {
        let decades = if pixel > 0.0 {
            (1e-3 / pixel).log10().max(0.0)
        } else {
            0.0
        };
        (BASE_ITERATIONS + (ITERATIONS_PER_DECADE * decades) as usize).min(MAX_AUTO_ITERATIONS)
    } else {
        manual.clamp(16, 100_000)
    }
}

/// `p` as a small positive integer (1 ... 16), if it is one: repeated multiplication.
pub fn integer_power(p: (f64, f64)) -> Option<u32> {
    (p.1 == 0.0 && p.0.fract() == 0.0 && (1.0..=16.0).contains(&p.0)).then_some(p.0 as u32)
}

fn cmul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

/// `zᵖ`, principal branch (`int`: p as a small integer, [`integer_power`]).
pub fn cpow(z: (f64, f64), p: (f64, f64), int: Option<u32>) -> (f64, f64) {
    if let Some(k) = int {
        let mut r = z;
        for _ in 1..k {
            r = cmul(r, z);
        }
        return r;
    }
    let r2 = z.0 * z.0 + z.1 * z.1;
    if r2 == 0.0 {
        return if p.0 > 0.0 { (0.0, 0.0) } else { (f64::INFINITY, 0.0) };
    }
    // Log z = (ln|z|, arg z); zᵖ = exp(p Log z).
    let l = (0.5 * r2.ln(), z.1.atan2(z.0));
    let w = cmul(p, l);
    let m = w.0.exp();
    (m * w.1.cos(), m * w.1.sin())
}

/// The smooth offset of an escaped `z` (`|z|` past the bailout): `s = (1 − log_k(ln|z|)) · w`
/// with the base `k = max(Re p, 1.5)`, `ln|z|` capped at 80 and the weight
/// `w = clamp(2 (Re p − 1), 0, 1)`, so `n + s` is continuous across the escape steps for
/// `Re p ≥ 1.5` (the Mandelbrot kernels' `1 − log2(ln|z|)` is the case k = 2). Why `Re p` and not
/// `|p|`: near infinity `ln|zᵖ| = Re p · ln|z| − Im p · arg z`, and `arg z` stays within ±π, so
/// the escape is double-exponential with base `Re p`. Towards `Re p = 1` it is not (no smooth
/// count exists), and the offset fades to the plain count instead of jumping (a seam across the p
/// plane otherwise). Non-finite results (overflow) are 0.
pub fn smooth_offset(abs_z: f64, p: (f64, f64)) -> f64 {
    let k = p.0.max(MIN_SMOOTH_BASE);
    // (Capped at 80, as the float32 kernel: past that its |z| has overflowed.)
    let lnz = if abs_z.is_finite() { abs_z.ln().min(80.0) } else { 80.0 };
    let s = (1.0 - lnz.log2() / k.log2()) * (2.0 * (p.0 - 1.0)).clamp(0.0, 1.0);
    if s.is_finite() { s } else { 0.0 }
}

/// The sample of one pixel value: the escape `(n, s)` as the kernels store it, or
/// ([`ITER_INTERIOR`], 0) when it does not escape within `max_iter` (or the orbit cycles).
pub fn escape(v: &LabValues, plane: Plane, pixel: (f64, f64), max_iter: usize) -> (f32, f32) {
    let mut vals = *v;
    match plane {
        Plane::C => vals.c = pixel,
        Plane::Z0 => vals.z0 = pixel,
        Plane::P => vals.p = pixel,
    }
    let int = if plane == Plane::P { None } else { integer_power(vals.p) };
    let b2 = BAILOUT * BAILOUT;
    let mut z = vals.z0;
    let mut r2 = z.0 * z.0 + z.1 * z.1;
    let mut n = 0usize;
    let mut saved = z;
    let mut next_save = 1usize;
    let eps2 = CYCLE_EPS * CYCLE_EPS;
    // (NaN compares false: an orbit that turned NaN counts as escaped.)
    while n < max_iter && r2 <= b2 {
        let w = cpow(z, vals.p, int);
        z = (w.0 + vals.c.0, w.1 + vals.c.1);
        n += 1;
        r2 = z.0 * z.0 + z.1 * z.1;
        let (dx, dy) = (z.0 - saved.0, z.1 - saved.1);
        if dx * dx + dy * dy < eps2 {
            return (ITER_INTERIOR, 1.0);
        }
        if n == next_save {
            saved = z;
            next_save *= 2;
        }
    }
    if r2 <= b2 {
        return (ITER_INTERIOR, 0.0);
    }
    (n as f32, smooth_offset(r2.sqrt(), vals.p) as f32)
}

/// The orbit `z₀, z₁, ...` of the values, up to `len` points or the escape (the orbit overlay).
pub fn orbit(v: &LabValues, len: usize) -> Vec<(f64, f64)> {
    let int = integer_power(v.p);
    let mut z = v.z0;
    let mut out = Vec::with_capacity(len.min(4096));
    out.push(z);
    while out.len() < len {
        let w = cpow(z, v.p, int);
        z = (w.0 + v.c.0, w.1 + v.c.1);
        if !(z.0.is_finite() && z.1.is_finite()) {
            break;
        }
        out.push(z);
        if z.0 * z.0 + z.1 * z.1 > BAILOUT * BAILOUT {
            break;
        }
    }
    out
}

/// The plane's value at the centre of pixel (x, y) (y down) of a `w` × `h` picture of `view`
/// with `ppp` view points per pixel.
pub fn pixel_value(view: &Viewport, x: f64, y: f64, w: usize, h: usize, ppp: f64) -> (f64, f64) {
    let dx = (x + 0.5 - w as f64 / 2.0) * ppp;
    let dy = (h as f64 / 2.0 - y - 0.5) * ppp;
    let (rx, ry) = view.rotate(dx, dy);
    let s = view.scale.to_f64();
    (view.center.re() + rx * s, view.center.im() + ry * s)
}

/// A `w` × `h` field of `view` in `plane` (`ppp` view points per pixel) in f64, rows in parallel:
/// what the GPU kernel `labField` computes in float32.
pub fn cpu_field(
    v: &LabValues,
    plane: Plane,
    view: &Viewport,
    w: usize,
    h: usize,
    ppp: f64,
    max_iter: usize,
) -> IterationField {
    let mut n = vec![0f32; w * h];
    let mut s = vec![0f32; w * h];
    n.par_chunks_mut(w.max(1))
        .zip(s.par_chunks_mut(w.max(1)))
        .enumerate()
        .for_each(|(y, (nrow, srow))| {
            for x in 0..w {
                let (a, b) = escape(v, plane, pixel_value(view, x as f64, y as f64, w, h, ppp), max_iter);
                nrow[x] = a;
                srow[x] = b;
            }
        });
    IterationField {
        width: w,
        height: h,
        n,
        s,
    }
}

/// The plane's whole framing for a view of `w` × `h` points (the lab's home view, and the
/// panels' fixed framing at their pixel size).
pub fn home_view(plane: Plane, w: f64, h: f64) -> Viewport {
    let ((cx, cy), (sx, sy)) = plane.frame();
    if !(w > 0.0 && h > 0.0) {
        return Viewport::new(cx, cy, sx / 1000.0);
    }
    Viewport::new(cx, cy, (sx / w).max(sy / h))
}

/// The view limits of the plane (kb: decisions/controls "View limits"): the centre in
/// [`Plane::bounds`], zoomed out at most to twice the home view.
pub fn limits(plane: Plane, w: f64, h: f64) -> ViewLimits {
    let home = home_view(plane, w, h);
    ViewLimits {
        bounds: plane.bounds(),
        max_scale: home.scale * 2.0,
    }
}

/// The precision limit (zoom stops here): the smallest pixel size (plane units) for a view
/// centred at `center`. The GPU iterates in float32, whose spacing near `x` is about
/// `2⁻²³ |x|`: at about one step per pixel neighbouring pixels round to the same value and the
/// picture turns into blocks; a few steps per pixel already shift pixels by a good part of a
/// pixel. [`LIMIT_ULPS`] float32 steps (of the larger of `|centre|` and 1) per pixel is where the
/// float32 field still differs from the f64 reference less than the reference differs from itself
/// shifted by a quarter pixel, at boundary points of every plane (measured 2026-10-31, M1 Pro;
/// kb: plans/f10-parameter-lab "Zoom limit").
pub fn min_pixel(center: (f64, f64)) -> f64 {
    LIMIT_ULPS * f64::from(f32::EPSILON) * center.0.abs().max(center.1.abs()).max(1.0)
}

/// float32 steps per pixel at the zoom limit (see [`min_pixel`]).
pub const LIMIT_ULPS: f64 = 8.0;

/// SplitMix64: the Random button's stream (seeded, so tests and the CLI repeat).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [a, b).
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Random values in interesting ranges: p with `Re p` in [1.5, 5] and a small imaginary part
/// (|Im p| ≤ 0.25, zero half the time), z₀ in the disc |z₀| ≤ 0.3, and c near the boundary of
/// the set those two give in the c plane (outside it, escaping after 12 ... 200 steps: the
/// detailed band around the set). Tries up to 4000 points; else c = −0.75.
pub fn random(seed: u64) -> LabValues {
    let mut rng = Rng(seed ^ 0x6C61_6221_u64);
    let p_re = rng.range(1.5, 5.0);
    let p_im = if rng.next() & 1 == 0 {
        0.0
    } else {
        rng.range(-0.25, 0.25)
    };
    let r = 0.3 * rng.range(0.0, 1.0).sqrt();
    let a = rng.range(0.0, std::f64::consts::TAU);
    let mut v = LabValues {
        z0: (r * a.cos(), r * a.sin()),
        c: (-0.75, 0.0),
        p: (p_re, p_im),
    };
    let ((cx, cy), (sx, sy)) = Plane::C.frame();
    for _ in 0..4000 {
        let c = (cx + sx * rng.range(-0.45, 0.45), cy + sy * rng.range(-0.45, 0.45));
        let (n, _) = escape(&v, Plane::C, c, 400);
        if (12.0..=200.0).contains(&n) {
            v.c = c;
            break;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_two_is_the_mandelbrot_set() {
        let v = LabValues::default();
        // c plane at z₀ = 0, p = 2: the Mandelbrot set (cardioid interior, the period-2 bulb,
        // outside points escape as z² + c does).
        assert_eq!(escape(&v, Plane::C, (0.0, 0.0), 1000).0, ITER_INTERIOR);
        assert_eq!(escape(&v, Plane::C, (-1.0, 0.0), 1000).0, ITER_INTERIOR);
        let (n, s) = escape(&v, Plane::C, (0.5, 0.5), 1000);
        let (nj, sj) = crate::julia::escape((0.5, 0.5), (0.0, 0.0), 1000);
        // (Another bailout: the count differs by the steps between 256 and 65536.)
        assert!(n >= 3.0 && n <= nj && nj - n <= 2.0, "{n} {nj}");
        assert!(
            (f64::from(n + s) - f64::from(nj + sj)).abs() < 0.02,
            "{n}+{s} vs {nj}+{sj}"
        );
        // The general power at p = 2 + 0i agrees with the integer path where it matters.
        let g = cpow((0.3, -0.7), (2.0, 0.0), None);
        let i = cpow((0.3, -0.7), (2.0, 0.0), Some(2));
        assert!((g.0 - i.0).abs() < 1e-12 && (g.1 - i.1).abs() < 1e-12);
    }

    #[test]
    fn principal_branch_and_zero() {
        // (−1)^½ = i on the principal branch; just below the negative axis it is −i: the seam.
        let a = cpow((-1.0, 0.0), (0.5, 0.0), None);
        assert!((a.0).abs() < 1e-12 && (a.1 - 1.0).abs() < 1e-12);
        let b = cpow((-1.0, -1e-12), (0.5, 0.0), None);
        assert!((b.1 + 1.0).abs() < 1e-9);
        assert_eq!(cpow((0.0, 0.0), (2.5, 1.0), None), (0.0, 0.0));
        assert!(cpow((0.0, 0.0), (-1.0, 0.0), None).0.is_infinite());
        // z₀ = 0 with Re p ≤ 0 escapes at once (0ᵖ is infinite).
        let v = LabValues {
            p: (-0.5, 0.0),
            ..LabValues::default()
        };
        let (n, s) = escape(&v, Plane::C, (0.1, 0.0), 100);
        assert_eq!(n, 1.0);
        assert!(s.is_finite());
    }

    #[test]
    fn planes_meet_at_the_values() {
        // The same (z₀, c, p) gives the same sample in each plane.
        let v = LabValues {
            z0: (0.1, -0.05),
            c: (-0.12, 0.75),
            p: (2.3, 0.1),
        };
        let a = escape(&v, Plane::C, v.c, 500);
        let b = escape(&v, Plane::Z0, v.z0, 500);
        let c = escape(&v, Plane::P, v.p, 500);
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn random_values_are_in_range_and_near_the_boundary() {
        for seed in 0..20 {
            let v = random(seed);
            assert!((1.5..=5.0).contains(&v.p.0) && v.p.1.abs() <= 0.25, "{v:?}");
            assert!(v.z0.0.hypot(v.z0.1) <= 0.3 + 1e-12);
            let (n, _) = escape(&v, Plane::C, v.c, 400);
            assert!((12.0..=200.0).contains(&n), "seed {seed}: {v:?} n {n}");
            assert_eq!(v, random(seed));
            assert_eq!(v, v.sanitized());
        }
        assert_ne!(random(1), random(2));
    }

    #[test]
    fn values_clamp_into_their_ranges() {
        let mut v = LabValues::default();
        v.set(Plane::P, (50.0, f64::NAN));
        assert_eq!(v.p, (10.0, 0.0));
        v.set(Plane::C, (-9.0, 9.0));
        assert_eq!(v.c, (-2.5, 2.5));
        assert_eq!(Plane::parse("z₀"), Some(Plane::Z0));
        assert_eq!(Plane::parse("P"), Some(Plane::P));
        for p in Plane::ALL {
            assert_eq!(Plane::parse(p.name()), Some(p));
        }
    }

    #[test]
    fn smooth_count_is_continuous_across_steps() {
        // An escaping orbit at p = 3: n + s barely changes along a line of c (no band jumps).
        let v = LabValues {
            p: (3.0, 0.0),
            ..LabValues::default()
        };
        let mut last: Option<f64> = None;
        for i in 0..200 {
            let c = (0.6 + 0.001 * f64::from(i), 0.2);
            let (n, s) = escape(&v, Plane::C, c, 1000);
            let mu = f64::from(n) + f64::from(s);
            if let Some(m) = last {
                assert!((mu - m).abs() < 0.2, "{i}: {m} -> {mu}");
            }
            last = Some(mu);
        }
    }

    #[test]
    fn limits_and_framing() {
        let h = home_view(Plane::P, 1200.0, 800.0);
        assert!((h.center.re() - 2.5).abs() < 1e-12);
        let l = limits(Plane::P, 1200.0, 800.0);
        assert!(!l.outside(&h.center));
        assert!(min_pixel((0.0, 0.0)) < 1e-5 && min_pixel((100.0, 0.0)) > min_pixel((0.0, 0.0)));
        assert_eq!(iterations(true, 5000, 2e-3), BASE_ITERATIONS);
        assert_eq!(iterations(true, 5000, 1e-6), BASE_ITERATIONS + 450);
        assert_eq!(iterations(false, 5000, 1e-6), 5000);
        let f = cpu_field(
            &LabValues::default(),
            Plane::Z0,
            &home_view(Plane::Z0, 64.0, 40.0),
            64,
            40,
            1.0,
            200,
        );
        // z₀ plane at c = −0.75: the Julia set of the neck, symmetric under z ↦ −z.
        for y in 0..40 {
            for x in 0..64 {
                assert_eq!(f.n[y * 64 + x], f.n[(39 - y) * 64 + (63 - x)]);
            }
        }
    }
}
