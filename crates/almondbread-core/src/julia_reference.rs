//! The two reference orbits of a deep Julia view (F4, kb: plans/f4-deep-julia, formulas/julia,
//! techniques/rebasing).
//!
//! A Julia pixel starts at `z_0 = centre + delta` with `k` fixed, so its delta to the view
//! centre's orbit starts at `delta` (no `+c` term afterwards). Zhuoran's rebasing needs a
//! reference through 0, which the centre orbit is not; the critical orbit (the orbit of 0 under
//! `z^2 + k`, i.e. the Mandelbrot orbit of `c = k`) is. So a pixel iterates against the centre
//! orbit until its first rebase (`|Z_m + z| < |z|`, or the centre orbit ends) and from then on
//! against the critical orbit, rebasing to its index 0 exactly as a Mandelbrot pixel does
//! (step-1 gate: plans/f4-deep-julia "Step 1").
//!
//! **Layout.** The kernels read one orbit buffer; a Julia reference concatenates the two:
//! `[critical orbit 0 ..= crit_last][zeros][centre orbit from center_start]`. Rebasing keeps
//! its target 0 (the critical orbit's start, as in Mandelbrot); fresh pixels start at
//! `center_start` with `n = 0`. The zeros make every BLA block that would straddle the two
//! orbits invalid (a zero `Z` has radius 0), and `center_start` is a multiple of
//! [`SEGMENT_ALIGN`] so the centre orbit's blocks keep their alignment (lookups at
//! `m = 1 (mod 2^l)`). The kernels end each orbit at its own last index.
//!
//! The critical orbit depends on `k` alone: it is cached per `k` (and precision), so panning
//! and zooming a Julia view recompute only the centre orbit.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use almondbread_math::{
    BigComplex, DoubleExp, DoubleExpComplex, FloatExpComplex, JuliaK, ReferenceOrbitComputer, Threads,
};

/// The centre orbit starts at a multiple of this (>= the BLA's lookup interval; blocks of the
/// centre orbit up to this length line up with the table's levels).
pub const SEGMENT_ALIGN: usize = 4096;

/// Where the two orbits lie in a Julia reference's points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JuliaLayout {
    /// Last index of the critical orbit (it starts at 0).
    pub crit_last: usize,
    /// First index of the centre orbit (its Z_0 is the view centre).
    pub center_start: usize,
    /// The critical orbit escaped (its end is an exact rebase point).
    pub crit_escaped: bool,
    /// Bits the reference carries beyond the view's because pixels pass the critical point
    /// within their own spread ([`near_critical_bits`]; F9). 0 for f64 references.
    pub near_critical_bits: usize,
}

impl JuliaLayout {
    /// Iterations the centre orbit holds, given the concatenated length.
    pub fn center_iterations(&self, len: usize) -> usize {
        len - 1 - self.center_start
    }
}

/// The index where the centre orbit starts after a critical orbit of `crit_len` points
/// (at least one zero in between).
pub fn center_start(crit_len: usize) -> usize {
    (crit_len + 1).div_ceil(SEGMENT_ALIGN) * SEGMENT_ALIGN
}

/// Concatenates the critical and the centre orbit (`extended` may be empty for both: f64
/// references).
pub fn assemble<T: Copy>(crit: &[T], center: &[T], zero: T) -> Vec<T> {
    let start = center_start(crit.len());
    let mut out = Vec::with_capacity(start + center.len());
    out.extend_from_slice(crit);
    out.resize(start, zero);
    out.extend_from_slice(center);
    out
}

/// An f64 orbit of `z^2 + k` from `z0`: at least two points (as the Mandelbrot f64
/// reference), `escaped`. Same arithmetic as [`crate::reference::Reference::compute_f64`].
pub fn orbit_f64(
    k: (f64, f64),
    z0: (f64, f64),
    max_iter: usize,
    bailout: f64,
    cancel: &AtomicBool,
) -> Option<(Vec<[f32; 2]>, bool)> {
    let b2 = bailout * bailout;
    let (mut x, mut y) = z0;
    let mut pts = Vec::with_capacity(max_iter.min(1 << 18) + 1);
    pts.push([x as f32, y as f32]);
    let mut escaped = x * x + y * y > b2;
    let mut done = 0usize;
    while done < max_iter && !escaped {
        if done & 0xffff == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let nx = x * x - y * y + k.0;
        y = 2.0 * x * y + k.1;
        x = nx;
        pts.push([x as f32, y as f32]);
        escaped = x * x + y * y > b2;
        done += 1;
    }
    if pts.len() < 2 {
        pts.push(pts[0]);
    }
    Some((pts, escaped))
}

/// A cached critical orbit in high precision.
pub struct CriticalOrbit {
    pub k: JuliaK,
    pub bits: usize,
    /// Iterations it was computed for (it may be shorter: escaped).
    pub max_iter: usize,
    pub points: Vec<[f32; 2]>,
    pub extended: Vec<FloatExpComplex>,
    pub escaped: bool,
}

/// The last few critical orbits (a session works on one or two `k` at a time).
static CRITICAL: Mutex<Vec<Arc<CriticalOrbit>>> = Mutex::new(Vec::new());
const CRITICAL_KEPT: usize = 2;

/// The critical orbit of `k` with at least `bits` bits and `max_iter` iterations (or
/// escaped), from the cache or computed (on three threads where that pays; `cancel` polled
/// every 4096 iterations, None when cancelled).
pub fn critical_orbit(
    k: JuliaK,
    bits: usize,
    max_iter: usize,
    bailout: f64,
    cancel: &AtomicBool,
) -> Option<Arc<CriticalOrbit>> {
    {
        let cache = CRITICAL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = cache
            .iter()
            .find(|c| c.k == k && c.bits >= bits && (c.escaped || c.max_iter >= max_iter))
        {
            return Some(c.clone());
        }
    }
    let comp = ReferenceOrbitComputer::new(&k.value(), bits).with_threads(Threads::Automatic);
    let (mut points, mut extended, escaped) = staged(&comp, max_iter, bailout, cancel)?;
    if points.len() < 2 {
        let c = k.value().to_f32();
        points.push(c);
        extended.push(k.value().float_exp_complex());
    }
    let orbit = Arc::new(CriticalOrbit {
        k,
        bits: comp.precision_bits(),
        max_iter,
        points,
        extended,
        escaped,
    });
    let mut cache = CRITICAL.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|c| c.k != k);
    cache.insert(0, orbit.clone());
    cache.truncate(CRITICAL_KEPT);
    Some(orbit)
}

/// The centre orbit of a Julia view in high precision: `z_0 = center`, `c = k`.
pub fn center_orbit(
    k: JuliaK,
    center: &BigComplex,
    bits: usize,
    max_iter: usize,
    bailout: f64,
    cancel: &AtomicBool,
) -> Option<(Vec<[f32; 2]>, Vec<FloatExpComplex>, bool)> {
    let comp = ReferenceOrbitComputer::new(&k.value(), bits)
        .with_start(center)
        .with_threads(Threads::Automatic);
    let (mut points, mut extended, escaped) = staged(&comp, max_iter, bailout, cancel)?;
    if points.len() < 2 {
        points.push(points[0]);
        extended.push(extended[0]);
    }
    Some((points, extended, escaped))
}

/// Bits a Julia or turned reference needs beyond the view's where its pixels pass the critical
/// point 0 within their own spread (F9's gate found it, kb: plans/f9-turn "Near the critical
/// point"). A Julia step has no `+c`: neighbouring pixels `Δ` apart at `z ≈ 0` are `2|z| Δ +
/// Δ^2` apart after it, so near 0 their differences shrink to `Δ^2` and the reference must
/// resolve that, not the texel (a view centred on 0 needs twice the bits: the picture there is
/// the one near `k` at the squared scale). A turned plane adds `cos t texel` per step, which
/// bounds the loss.
///
/// Along the centre orbit `Z_m` (`orbit`), with the plane derivative `D_0 = sin t`, `D_{m+1} =
/// 2 Z_m D_m + cos t`: the view's image at step m has radius `I_m = radius |D_m|`; where it
/// reaches 0 (`|Z_m| < 4 I_m`) neighbours end up `rho texel` apart with `rho = max(2 max(|Z_m| -
/// I_m, 0) |D_m|, |D_m|^2 texel, cos t)`, and `-log2 rho` bits are missing. Scanned until the
/// image is large (`I_m >= 1`: the pixels spread over the whole set). Capped at the bits of a
/// view at 0 (`-log2(radius)`) plus 16. 0 for Mandelbrot-like planes (`cos t ≈ 1`).
pub fn near_critical_bits(
    orbit: impl Iterator<Item = DoubleExpComplex>,
    cos: f64,
    sin: f64,
    radius: DoubleExp,
    texel: DoubleExp,
) -> usize {
    if cos >= 0.5 || radius.is_zero() || texel.is_zero() {
        return 0;
    }
    let cap = (-radius.log2_magnitude()).max(0.0) + 16.0;
    let (c, two) = (DoubleExp::from_f64(cos), DoubleExp::from_f64(2.0));
    let one = DoubleExp::from_f64(1.0);
    let mut d = DoubleExpComplex::new(DoubleExp::from_f64(sin), DoubleExp::ZERO);
    let mut extra = 0.0f64;
    for (m, z) in orbit.enumerate() {
        let dn = d.magnitude();
        let image = radius * dn;
        if image >= one || m > 1 << 20 {
            break;
        }
        let zn = z.magnitude();
        if zn < image * DoubleExp::from_f64(4.0) {
            let gap = if zn > image { zn - image } else { DoubleExp::ZERO };
            let mut rho = two * gap * dn;
            let sq = dn * dn * texel;
            if sq > rho {
                rho = sq;
            }
            if c > rho {
                rho = c;
            }
            if !rho.is_zero() && rho < one {
                extra = extra.max(-rho.log2_magnitude());
            } else if rho.is_zero() {
                extra = cap;
            }
        }
        d = z.times_two() * d + DoubleExpComplex::new(c, DoubleExp::ZERO);
    }
    extra.min(cap).ceil() as usize
}

/// [`near_critical_bits`] of a floatexp orbit.
pub fn near_critical_bits_x(
    orbit: &[FloatExpComplex],
    cos: f64,
    sin: f64,
    radius: DoubleExp,
    texel: DoubleExp,
) -> usize {
    let x = |z: &FloatExpComplex| {
        DoubleExpComplex::new(
            DoubleExp::from_f64(f64::from(z.mantissa[0])).times_power_of_two(i64::from(z.exponent)),
            DoubleExp::from_f64(f64::from(z.mantissa[1])).times_power_of_two(i64::from(z.exponent)),
        )
    };
    near_critical_bits(orbit.iter().map(x), cos, sin, radius, texel)
}

/// F9 (kb: plans/f9-turn): the two orbits of a turned reference with parameter `c` and start
/// `z0`, in high precision: the critical orbit (from 0) and the centre orbit (from `z0`; the
/// critical one itself when `z0 = 0`), each `(points, extended, escaped)`. Not cached: `c`
/// changes with every reference point (unlike a Julia set's `k`). The two run side by side.
#[allow(clippy::type_complexity)]
pub fn turn_orbits(
    c: &BigComplex,
    z0: &BigComplex,
    bits: usize,
    max_iter: usize,
    bailout: f64,
    cancel: &AtomicBool,
) -> Option<(
    (Vec<[f32; 2]>, Vec<FloatExpComplex>, bool),
    (Vec<[f32; 2]>, Vec<FloatExpComplex>, bool),
)> {
    let pad = |(mut p, mut e, esc): (Vec<[f32; 2]>, Vec<FloatExpComplex>, bool)| {
        if p.len() < 2 {
            p.push(p[0]);
            e.push(e[0]);
        }
        (p, e, esc)
    };
    let crit_comp = ReferenceOrbitComputer::new(c, bits).with_threads(Threads::Automatic);
    if z0.re.is_zero() && z0.im.is_zero() {
        let crit = pad(staged(&crit_comp, max_iter, bailout, cancel)?);
        return Some((crit.clone(), crit));
    }
    let center_comp = ReferenceOrbitComputer::new(c, bits)
        .with_start(z0)
        .with_threads(Threads::Automatic);
    let (crit, center) = std::thread::scope(|s| {
        let h = s.spawn(|| staged(&crit_comp, max_iter, bailout, cancel));
        let center = staged(&center_comp, max_iter, bailout, cancel);
        (h.join().ok().flatten(), center)
    });
    Some((pad(crit?), pad(center?)))
}

/// Runs `comp` to `max_iter` (or escape) with storage that grows stage by stage (an orbit
/// usually escapes long before a high limit: no reservation of the whole request).
pub(crate) fn staged(
    comp: &ReferenceOrbitComputer,
    max_iter: usize,
    bailout: f64,
    cancel: &AtomicBool,
) -> Option<(Vec<[f32; 2]>, Vec<FloatExpComplex>, bool)> {
    let mut points: Vec<[f32; 2]> = Vec::new();
    let mut extended: Vec<FloatExpComplex> = Vec::new();
    let mut is_cancelled = || cancel.load(Ordering::Relaxed);
    let mut ignore = |_: usize| {};
    let mut ctl = almondbread_math::Control {
        check_interval: 4096,
        is_cancelled: &mut is_cancelled,
        progress: &mut ignore,
    };
    let mut stage = max_iter.min(1 << 18);
    let mut prev: Option<almondbread_math::OrbitResult> = None;
    loop {
        points.resize(stage + 1, [0.0; 2]);
        extended.resize(stage + 1, FloatExpComplex::ZERO);
        let r = match &prev {
            None => comp.compute(stage, bailout, Some(&mut points), Some(&mut extended), &mut ctl),
            Some(p) => comp.resume(p, stage, bailout, Some(&mut points), Some(&mut extended), &mut ctl),
        };
        if r.cancelled {
            return None;
        }
        if r.escaped || stage >= max_iter {
            points.truncate(r.count);
            extended.truncate(r.count);
            return Some((points, extended, r.escaped));
        }
        prev = Some(r);
        stage = stage.saturating_mul(2).min(max_iter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_aligns_the_centre_orbit() {
        assert_eq!(center_start(1), SEGMENT_ALIGN);
        assert_eq!(center_start(SEGMENT_ALIGN - 1), SEGMENT_ALIGN);
        assert_eq!(center_start(SEGMENT_ALIGN), 2 * SEGMENT_ALIGN);
        let v = assemble(&[1, 2, 3], &[7, 8], 0);
        assert_eq!(v.len(), SEGMENT_ALIGN + 2);
        assert_eq!(&v[..4], &[1, 2, 3, 0]);
        assert_eq!(&v[SEGMENT_ALIGN..], &[7, 8]);
    }

    #[test]
    fn critical_orbits_are_cached_per_k() {
        let k = JuliaK::new(&BigComplex::parse("-0.12256", "0.74486", 256).unwrap());
        let never = AtomicBool::new(false);
        let a = critical_orbit(k, 256, 1000, 65536.0, &never).unwrap();
        let b = critical_orbit(k, 192, 500, 65536.0, &never).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        let c = critical_orbit(k, 512, 1000, 65536.0, &never).unwrap();
        assert!(!Arc::ptr_eq(&a, &c));
        assert_eq!(a.points.len(), 1001);
        // The critical orbit is the Mandelbrot orbit of k: Z_1 = k.
        assert!((f64::from(a.points[1][0]) + 0.12256).abs() < 1e-6);
        let (f, esc) = orbit_f64(k.approx(), (0.0, 0.0), 1000, 65536.0, &never).unwrap();
        assert!(!esc);
        for (p, q) in f.iter().zip(&a.points).take(40) {
            assert!((p[0] - q[0]).abs() < 1e-5 && (p[1] - q[1]).abs() < 1e-5);
        }
    }
}
