//! Reference selection: period detection, Newton's method for minibrot nuclei, size
//! estimates and the "best reference for this view" policy (kb: techniques/reference-selection).
//!
//! The high-precision loops run on preallocated registers (no allocation per iteration);
//! radii and derivatives that need a few digits but huge range use [`DoubleExp`].

use crate::bigfloat::{BigFloat, raw_double_exp, with_scratch};
use crate::complex::BigComplex;
use crate::floatexp::{DoubleExp, DoubleExpComplex};
use crate::orbit::load;
use crate::raw::{self, Raw};

type Reg = Raw<Vec<u64>>;

fn reg(n: usize) -> Reg {
    Raw {
        m: vec![0; n],
        neg: false,
        exp: 0,
        zero: true,
    }
}

/// |x + iy| in extended range.
fn complex_magnitude(x: &Reg, y: &Reg) -> DoubleExp {
    DoubleExpComplex::new(raw_double_exp(x.view()), raw_double_exp(y.view())).magnitude()
}

fn complex_value(x: &Reg, y: &Reg) -> DoubleExpComplex {
    DoubleExpComplex::new(raw_double_exp(x.view()), raw_double_exp(y.view()))
}

/// (x, y) <- (x + iy)^2 + (cx + i cy), multiplication form (relative accuracy per part).
#[allow(clippy::too_many_arguments)]
fn complex_square_add(x: &mut Reg, y: &mut Reg, cx: &Reg, cy: &Reg, t: &mut [Reg; 4], s: &mut [u64]) {
    let [t0, t1, t2, t3] = t;
    raw::add(t0, x.view(), y.view(), false, s);
    raw::add(t1, x.view(), y.view(), true, s);
    raw::mul(t2, t0.view(), t1.view(), s);
    raw::mul(t3, x.view(), y.view(), s);
    if !t3.zero {
        t3.exp += 1;
    }
    raw::add(x, t2.view(), cx.view(), false, s);
    raw::add(y, t3.view(), cy.view(), false, s);
}

fn temps(n: usize) -> [Reg; 4] {
    [reg(n), reg(n), reg(n), reg(n)]
}

// ---------------------------------------------------------------------------------------
// Period detection

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PeriodMethod {
    /// Ball arithmetic: R_{n+1} = 2|z_n| R_n + R_n^2 + r, first n with |z_n| < R_n. A
    /// rigorous enclosure of the disc's image (ignoring rounding): never misses the lowest
    /// period but can report a slightly lower one in skewed places.
    #[default]
    Ball,
    /// First-order ball: R_n = |dz_n/dc| r. Cheaper bound, closer to KF's default.
    BallFirstOrder,
    /// Box (Munafo): first n at which the image of the square's four corners winds
    /// around 0. Four orbits per iteration.
    Box,
}

/// Lowest period of a minibrot (mu-atom) found inside the disc or box of radius `radius`
/// around `center`, or `None` if none is found up to `max_period` (or the region escapes,
/// or `is_cancelled` returns true; polled every `check_interval` iterations). Runs at the
/// center's precision.
pub fn find_period(
    center: &BigComplex,
    radius: &BigFloat,
    max_period: usize,
    method: PeriodMethod,
    check_interval: usize,
    is_cancelled: &mut dyn FnMut() -> bool,
) -> Option<usize> {
    find_period_from(center, radius, 1, max_period, method, check_interval, is_cancelled)
}

/// [`find_period`] ignoring periods below `min_period` (ball methods only; the box method
/// ignores it): the next atom touching the disc after a known one, e.g. to step from a
/// minibrot into the structures around it (deep location search).
pub fn find_period_from(
    center: &BigComplex,
    radius: &BigFloat,
    min_period: usize,
    max_period: usize,
    method: PeriodMethod,
    check_interval: usize,
    is_cancelled: &mut dyn FnMut() -> bool,
) -> Option<usize> {
    let ci = check_interval.max(1);
    match method {
        PeriodMethod::Ball | PeriodMethod::BallFirstOrder => ball_period(
            center,
            radius.double_exp().abs(),
            min_period,
            max_period,
            method == PeriodMethod::BallFirstOrder,
            ci,
            is_cancelled,
        ),
        PeriodMethod::Box => box_period(center, radius, max_period, ci, is_cancelled),
    }
}

#[allow(clippy::too_many_arguments)]
fn ball_period(
    center: &BigComplex,
    r: DoubleExp,
    min_period: usize,
    max_period: usize,
    first_order: bool,
    check_interval: usize,
    is_cancelled: &mut dyn FnMut() -> bool,
) -> Option<usize> {
    let n = BigFloat::limb_count_for_bits(center.precision_bits());
    let (mut x, mut y, mut cx, mut cy) = (reg(n), reg(n), reg(n), reg(n));
    let mut t = temps(n);
    load(&mut cx, &center.re);
    load(&mut cy, &center.im);
    let two = DoubleExp::from_f64(2.0);
    let escape = DoubleExp::from_f64(1e10);
    with_scratch(n, |s| {
        let mut radius_acc = DoubleExp::ZERO; // R_n
        let mut dz = DoubleExpComplex::ZERO; // dz_n/dc
        let mut z_abs = DoubleExp::ZERO; // |z_{n-1}|
        for p in 1..=max_period.max(1) {
            if first_order {
                // dz_p = 2 z_{p-1} dz_{p-1} + 1
                dz = (complex_value(&x, &y) * dz).times_two() + DoubleExpComplex::ONE;
            } else {
                radius_acc = two * z_abs * radius_acc + radius_acc * radius_acc + r;
            }
            complex_square_add(&mut x, &mut y, &cx, &cy, &mut t, s);
            z_abs = complex_magnitude(&x, &y);
            let bound = if first_order { dz.magnitude() * r } else { radius_acc };
            if z_abs < bound && p >= min_period {
                return Some(p);
            }
            if escape < z_abs - bound {
                return None; // the whole disc escaped
            }
            if !first_order && escape < radius_acc {
                return None;
            }
            if p % check_interval == 0 && is_cancelled() {
                return None;
            }
        }
        None
    })
}

fn box_period(
    center: &BigComplex,
    half_size: &BigFloat,
    max_period: usize,
    check_interval: usize,
    is_cancelled: &mut dyn FnMut() -> bool,
) -> Option<usize> {
    let n = BigFloat::limb_count_for_bits(center.precision_bits());
    let h = half_size.abs();
    // Corners in counter-clockwise order.
    let corners = [
        BigComplex::new(&center.re - &h, &center.im - &h),
        BigComplex::new(&center.re + &h, &center.im - &h),
        BigComplex::new(&center.re + &h, &center.im + &h),
        BigComplex::new(&center.re - &h, &center.im + &h),
    ];
    let mut zx: [Reg; 4] = core::array::from_fn(|_| reg(n));
    let mut zy: [Reg; 4] = core::array::from_fn(|_| reg(n));
    let mut cx: [Reg; 4] = core::array::from_fn(|_| reg(n));
    let mut cy: [Reg; 4] = core::array::from_fn(|_| reg(n));
    for k in 0..4 {
        load(&mut cx[k], &corners[k].re);
        load(&mut cy[k], &corners[k].im);
    }
    let mut t = temps(n);
    let (mut p1, mut p2) = (reg(n), reg(n));
    with_scratch(n, |s| {
        for p in 1..=max_period.max(1) {
            let mut escaped = false;
            for k in 0..4 {
                complex_square_add(&mut zx[k], &mut zy[k], &cx[k], &cy[k], &mut t, s);
                if zx[k].exp > 20 || zy[k].exp > 20 {
                    escaped = true;
                }
            }
            if escaped {
                return None;
            }
            // Crossing-number test of the origin against the quadrilateral z0 z1 z2 z3.
            let mut inside = false;
            for a in 0..4 {
                let b = (a + 1) & 3;
                let a_up = !zy[a].zero && !zy[a].neg;
                let b_up = !zy[b].zero && !zy[b].neg;
                if a_up == b_up {
                    continue;
                }
                // The edge crosses the positive real axis iff cross(a, b) has the sign of b.y.
                raw::mul(&mut p1, zx[a].view(), zy[b].view(), s);
                raw::mul(&mut p2, zy[a].view(), zx[b].view(), s);
                let c = raw::compare(p1.view(), p2.view());
                if c.is_ne() && c.is_gt() == b_up {
                    inside = !inside;
                }
            }
            if inside {
                return Some(p);
            }
            if p % check_interval == 0 && is_cancelled() {
                return None;
            }
        }
        None
    })
}

/// Atom-domain period: the n >= 1 at which |z_n| reaches its last new minimum before
/// escape or `max_iterations` (Peitgen & Richter; kb: techniques/reference-selection).
pub fn atom_domain_period(c: &BigComplex, max_iterations: usize) -> usize {
    let n = BigFloat::limb_count_for_bits(c.precision_bits());
    let (mut x, mut y, mut cx, mut cy) = (reg(n), reg(n), reg(n), reg(n));
    let mut t = temps(n);
    load(&mut cx, &c.re);
    load(&mut cy, &c.im);
    let mut best = DoubleExp::from_f64(1e300);
    let mut period = 1;
    let two = DoubleExp::from_f64(2.0);
    with_scratch(n, |s| {
        for i in 1..=max_iterations.max(1) {
            complex_square_add(&mut x, &mut y, &cx, &cy, &mut t, s);
            let a = complex_magnitude(&x, &y);
            if a < best {
                best = a;
                period = i;
            }
            if two < a {
                break;
            }
        }
    });
    period
}

// ---------------------------------------------------------------------------------------
// Newton's method for nuclei

#[derive(Clone, Debug)]
pub struct NucleusResult {
    pub nucleus: BigComplex,
    pub period: usize,
    /// Newton steps taken.
    pub steps: usize,
    /// log2 |last Newton step| (-inf if the last step was exactly zero).
    pub last_step_log2: f64,
}

/// Options for [`find_nucleus`].
#[derive(Clone, Debug)]
pub struct NewtonOptions {
    /// Working precision (default: the start point's).
    pub precision_bits: Option<usize>,
    pub max_steps: usize,
    /// Give up if c moves farther than this from the start.
    pub max_distance: Option<BigFloat>,
}

impl Default for NewtonOptions {
    fn default() -> Self {
        NewtonOptions {
            precision_bits: None,
            max_steps: 64,
            max_distance: None,
        }
    }
}

/// Solves z_p(c) = 0 by Newton's method from `start`. Each step iterates
/// z <- z^2 + c, dz <- 2 z dz + 1 for p steps and sets c <- c - z/dz. Converges
/// quadratically; stops when the step is within 16 bits of the working precision or stops
/// shrinking near it. `None` if it does not converge in `max_steps`, if dz vanishes, if c
/// moves farther than `max_distance`, or if cancelled (polled once per step).
pub fn find_nucleus(
    start: &BigComplex,
    period: usize,
    options: &NewtonOptions,
    is_cancelled: &mut dyn FnMut() -> bool,
) -> Option<NucleusResult> {
    assert!(period >= 1, "period must be >= 1");
    let bits = BigFloat::limb_count_for_bits(options.precision_bits.unwrap_or(start.precision_bits())) * 64;
    let n = bits / 64;
    let mut c = start.with_precision(bits);
    let (mut zx, mut zy, mut dx, mut dy) = (reg(n), reg(n), reg(n), reg(n));
    let (mut cx, mut cy) = (reg(n), reg(n));
    let mut t = temps(n);
    let (mut u0, mut u1, mut one) = (reg(n), reg(n), reg(n));
    raw::set_f64(&mut one, 1.0);
    let mut scratch = vec![0u64; raw::raw_scratch_limbs(n)];
    let s = &mut scratch[..];
    let mut previous_step_log2 = f64::INFINITY;
    for step in 1..=options.max_steps {
        if is_cancelled() {
            return None;
        }
        load(&mut cx, &c.re);
        load(&mut cy, &c.im);
        for r in [&mut zx, &mut zy, &mut dx, &mut dy] {
            raw::set_zero(r);
        }
        for _ in 0..period {
            // dz <- 2 z dz + 1 (uses the old z)
            {
                let [t0, t1, _, _] = &mut t;
                raw::mul(t0, zx.view(), dx.view(), s);
                raw::mul(t1, zy.view(), dy.view(), s);
                raw::add(&mut u0, t0.view(), t1.view(), true, s);
                raw::mul(t0, zx.view(), dy.view(), s);
                raw::mul(t1, zy.view(), dx.view(), s);
                raw::add(&mut u1, t0.view(), t1.view(), false, s);
            }
            if !u0.zero {
                u0.exp += 1;
            }
            if !u1.zero {
                u1.exp += 1;
            }
            raw::add(&mut dx, u0.view(), one.view(), false, s);
            raw::copy(&mut dy, u1.view());
            // z <- z^2 + c
            complex_square_add(&mut zx, &mut zy, &cx, &cy, &mut t, s);
        }
        if dx.zero && dy.zero {
            return None;
        }
        let z = BigComplex::new(BigFloat::from_raw(&zx), BigFloat::from_raw(&zy));
        let dz = BigComplex::new(BigFloat::from_raw(&dx), BigFloat::from_raw(&dy));
        let delta = &z / &dz;
        c = &c - &delta;
        let step_log2 = if delta.re.is_zero() && delta.im.is_zero() {
            f64::NEG_INFINITY
        } else {
            delta.re.log2_magnitude().max(delta.im.log2_magnitude())
        };
        if let Some(md) = &options.max_distance {
            let off = &c - start;
            if off.re.log2_magnitude().max(off.im.log2_magnitude()) > md.log2_magnitude() {
                return None;
            }
        }
        let c_log2 = c.re.log2_magnitude().max(c.im.log2_magnitude()).max(-(bits as f64));
        let floor = c_log2 - bits as f64 + 16.0;
        if step_log2 <= floor || (step_log2 < c_log2 - bits as f64 / 2.0 && step_log2 > previous_step_log2 - 1.0) {
            return Some(NucleusResult {
                nucleus: c,
                period,
                steps: step,
                last_step_log2: step_log2,
            });
        }
        previous_step_log2 = step_log2;
    }
    None
}

/// Size estimate of the minibrot with nucleus c and period p (relative to the main
/// cardioid's size 1): |1 / (b l^2)| with l = prod 2 z_i, b = 1 + sum 1/l_i over
/// i = 1 .. p-1 (mathr's estimate; kb: techniques/reference-selection).
pub fn minibrot_size(nucleus: &BigComplex, period: usize) -> DoubleExp {
    minibrot_size_complex(nucleus, period).magnitude()
}

/// Complex size estimate: its magnitude is the minibrot's scale relative to the whole set,
/// its argument the minibrot's rotation (a point w of the set maps to c + size * w).
pub fn minibrot_size_complex(nucleus: &BigComplex, period: usize) -> DoubleExpComplex {
    let n = BigFloat::limb_count_for_bits(nucleus.precision_bits());
    let (mut x, mut y, mut cx, mut cy) = (reg(n), reg(n), reg(n), reg(n));
    let mut t = temps(n);
    load(&mut cx, &nucleus.re);
    load(&mut cy, &nucleus.im);
    let mut l = DoubleExpComplex::ONE;
    let mut b = DoubleExpComplex::ONE;
    let degenerate = with_scratch(n, |s| {
        for _ in 1..period {
            complex_square_add(&mut x, &mut y, &cx, &cy, &mut t, s);
            l = (complex_value(&x, &y) * l).times_two();
            if l.re.is_zero() && l.im.is_zero() {
                return true;
            }
            b = b + DoubleExpComplex::ONE / l;
        }
        false
    });
    if degenerate {
        return DoubleExpComplex::ZERO;
    }
    DoubleExpComplex::ONE / (b * l * l)
}

// ---------------------------------------------------------------------------------------
// Newton's method for Misiurewicz points

/// A Misiurewicz point found by [`find_misiurewicz`].
#[derive(Clone, Debug)]
pub struct MisiurewiczResult {
    pub point: BigComplex,
    pub preperiod: usize,
    pub period: usize,
    /// Newton steps taken.
    pub steps: usize,
}

/// Solves z_{k+p}(c) = z_k(c) (preperiod k, period p) by Newton's method from `start`: the
/// tips, branch points and spiral centres of the set (kb: techniques/reference-selection,
/// Misiurewicz points). Plain Newton on f = z_{k+p} - z_k converges to the nearest root, which
/// may have a lower preperiod when the start is poor: start within the root's basin (e.g.
/// from a published 10+ digit approximation). Same stopping rule as [`find_nucleus`]; `None`
/// if it does not converge in `max_steps`, the derivative vanishes or c leaves
/// `max_distance`. Slow (BigComplex arithmetic): for catalogues, not per frame.
pub fn find_misiurewicz(
    start: &BigComplex,
    preperiod: usize,
    period: usize,
    options: &NewtonOptions,
) -> Option<MisiurewiczResult> {
    assert!(period >= 1, "period must be >= 1");
    let bits = BigFloat::limb_count_for_bits(options.precision_bits.unwrap_or(start.precision_bits())) * 64;
    let mut c = start.with_precision(bits);
    let one = BigComplex::from_f64(1.0, 0.0, bits);
    let mut previous_step_log2 = f64::INFINITY;
    for step in 1..=options.max_steps {
        // z_i and dz_i/dc for i = 0 ..= k + p.
        let mut zs = Vec::with_capacity(preperiod + period + 1);
        let mut dzs = Vec::with_capacity(preperiod + period + 1);
        let (mut z, mut dz) = (BigComplex::zero(bits), BigComplex::zero(bits));
        zs.push(z.clone());
        dzs.push(dz.clone());
        for _ in 1..=preperiod + period {
            // dz <- 2 z dz + 1 (old z), z <- z^2 + c
            dz = &(&z * &dz).times_power_of_two(1) + &one;
            z = &z.square() + &c;
            zs.push(z.clone());
            dzs.push(dz.clone());
        }
        // g = f / prod_{i<k} (z_{i+p} - z_i) with f = z_{k+p} - z_k (lower preperiods divided
        // out); Newton step g/g' = 1 / (f'/f - sum_i (dz_{i+p} - dz_i) / (z_{i+p} - z_i)).
        let f = &zs[preperiod + period] - &zs[preperiod];
        let df = &dzs[preperiod + period] - &dzs[preperiod];
        if f.re.is_zero() && f.im.is_zero() {
            return Some(MisiurewiczResult {
                point: c,
                preperiod,
                period,
                steps: step,
            });
        }
        let mut ratio = &df / &f;
        for i in 0..preperiod {
            let h = &zs[i + period] - &zs[i];
            if h.re.is_zero() && h.im.is_zero() {
                return None;
            }
            let dh = &dzs[i + period] - &dzs[i];
            ratio = &ratio - &(&dh / &h);
        }
        if ratio.re.is_zero() && ratio.im.is_zero() {
            return None;
        }
        let delta = &one / &ratio;
        c = &c - &delta;
        let step_log2 = if delta.re.is_zero() && delta.im.is_zero() {
            f64::NEG_INFINITY
        } else {
            delta.re.log2_magnitude().max(delta.im.log2_magnitude())
        };
        if let Some(md) = &options.max_distance {
            let off = &c - start;
            if off.re.log2_magnitude().max(off.im.log2_magnitude()) > md.log2_magnitude() {
                return None;
            }
        }
        let c_log2 = c.re.log2_magnitude().max(c.im.log2_magnitude()).max(-(bits as f64));
        let floor = c_log2 - bits as f64 + 16.0;
        if step_log2 <= floor || (step_log2 < c_log2 - bits as f64 / 2.0 && step_log2 > previous_step_log2 - 1.0) {
            return Some(MisiurewiczResult {
                point: c,
                preperiod,
                period,
                steps: step,
            });
        }
        previous_step_log2 = step_log2;
    }
    None
}

// ---------------------------------------------------------------------------------------
// Reference selection policy

#[derive(Clone, Debug)]
pub struct ReferenceChoice {
    /// The reference point C, at the requested precision.
    pub point: BigComplex,
    /// The nucleus period when `is_nucleus` (the orbit then only needs `period`
    /// iterations and Z_period = 0); `None` for the view centre.
    pub period: Option<usize>,
    pub is_nucleus: bool,
    /// Minibrot size estimate for a nucleus.
    pub minibrot_size: Option<DoubleExp>,
    pub newton_steps: usize,
    /// The period the detector reported, also when Newton failed or its nucleus lay
    /// outside the view (then `is_nucleus` is false and `point` is the centre).
    pub detected_period: Option<usize>,
}

/// Picks the reference for a view: the nucleus of the lowest-period minibrot found in the
/// disc of radius `radius` around `center` (use the view's half-diagonal) if Newton
/// converges to a point inside that disc, else the centre itself. Run it off the main
/// thread; cost is period detection (up to `max_iterations` complex squarings) plus Newton
/// (~10 steps of period-p iterations with derivative).
pub fn select_reference(
    center: &BigComplex,
    radius: &BigFloat,
    max_iterations: usize,
    precision_bits: Option<usize>,
    method: PeriodMethod,
    is_cancelled: &mut dyn FnMut() -> bool,
) -> ReferenceChoice {
    let bits = BigFloat::limb_count_for_bits(precision_bits.unwrap_or(center.precision_bits())) * 64;
    let c0 = center.with_precision(bits);
    let mut fallback = ReferenceChoice {
        point: c0.clone(),
        period: None,
        is_nucleus: false,
        minibrot_size: None,
        newton_steps: 0,
        detected_period: None,
    };
    let Some(p) = find_period(&c0, radius, max_iterations, method, 4096, is_cancelled) else {
        return fallback;
    };
    fallback.detected_period = Some(p);
    let options = NewtonOptions {
        precision_bits: Some(bits),
        max_steps: 64,
        max_distance: Some(radius.times_power_of_two(2)),
    };
    let Some(nr) = find_nucleus(&c0, p, &options, is_cancelled) else {
        return fallback;
    };
    let dist = (&nr.nucleus - &c0).magnitude_double_exp();
    if radius.double_exp().abs() < dist {
        return fallback;
    }
    let size = minibrot_size(&nr.nucleus, p);
    ReferenceChoice {
        point: nr.nucleus,
        period: Some(p),
        is_nucleus: true,
        minibrot_size: Some(size),
        newton_steps: nr.steps,
        detected_period: Some(p),
    }
}

#[cfg(test)]
mod misiurewicz_tests {
    use super::*;

    #[test]
    fn misiurewicz_points() {
        let opts = NewtonOptions {
            precision_bits: Some(256),
            ..NewtonOptions::default()
        };
        // M_{3,1} on the main antenna: the real root of c^3 + 2c^2 + 2c + 2 = 0 (-1.5436890126920764).
        let r = find_misiurewicz(&BigComplex::from_f64(-1.54, 0.0, 64), 3, 1, &opts).unwrap();
        let x = r.point.re.to_f64();
        assert!((x + 1.543_689_012_692_076_4).abs() < 1e-15, "{x}");
        assert!((x * x * x + 2.0 * x * x + 2.0 * x + 2.0).abs() < 1e-12);
        // c = i (preperiod 2, period 2).
        let r = find_misiurewicz(&BigComplex::from_f64(0.01, 0.99, 64), 2, 2, &opts).unwrap();
        assert!(r.point.re.log2_magnitude() < -200.0);
        assert!((r.point.im.to_f64() - 1.0).abs() < 1e-30);
    }
}
