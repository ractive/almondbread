//! Periods of the hyperbolic components a view touches, for the interior probe's Component
//! Newton (kb: techniques/interior-detection "Component Newton").
//!
//! Near the edge of a component the orbit converges to its attracting cycle very slowly
//! (`|λ|` close to 1), so neither the iteration limit nor a scan for the cycle along the orbit
//! finds it in time. Newton on `F(z) = f_c^p(z) − z` from `z = 0` does, in a handful of passes,
//! once the period `p` is known: `0` is a point of the cycle at the component's nucleus, and the
//! cycle point it continues to stays in Newton's basin across the component (measured
//! 2026-10-17: every interior pixel of a period-22 and a period-64 view, 5 passes on average).
//! This module finds `p` per view on the CPU: at a grid of sample points it takes the
//! atom-domain candidates (the indices of new minima of `|z_n|`) and keeps the smallest one for
//! which Newton from 0 converges to an attracting cycle. The GPU then proves each pixel on its
//! own (the probe's Kantorovich bound); a period here is only a hint, so this needs no rigour.
//!
//! Deep views perturb against the renderer's reference orbit (`z = Z_m + δ`, rebasing as the
//! kernels do) in [`DoubleExpComplex`]; shallow ones iterate `c` directly in f64. The search is
//! bounded by a step budget (kb: techniques/hang-prevention).

use std::ops::{Add, Div, Mul};

use crate::math::{DoubleExp, DoubleExpComplex, FloatExpComplex, Formula};

/// Periods handed to the kernel at most (`ComponentParams::periods`).
pub const MAX_PERIODS: usize = 4;
/// Newton passes per period try (CPU; the kernel's `SEED_PASSES` is its own).
const NEWTON_PASSES: usize = 12;
/// Converged: the Newton step is below 2^-30 of the cycle point.
const CONVERGED_LOG2: f64 = -30.0;

/// A complex number type the search runs in.
pub trait Cx: Copy + Add<Output = Self> + Mul<Output = Self> + Div<Output = Self> {
    const ZERO: Self;
    const ONE: Self;
    fn minus(self, o: Self) -> Self;
    fn twice(self) -> Self;
    /// `k z` (F2: the generic derivative `p z^(p-1)`).
    fn scaled(self, k: f64) -> Self;
    /// log2 |z| (-inf for 0).
    fn log2_abs(self) -> f64;
}

/// f64 complex (shallow views: `c` iterated directly).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C64(pub f64, pub f64);

impl Add for C64 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        C64(self.0 + o.0, self.1 + o.1)
    }
}
impl Mul for C64 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        C64(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
}
impl Div for C64 {
    type Output = Self;
    fn div(self, o: Self) -> Self {
        let d = o.0 * o.0 + o.1 * o.1;
        C64((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
}
impl Cx for C64 {
    const ZERO: Self = C64(0.0, 0.0);
    const ONE: Self = C64(1.0, 0.0);
    fn minus(self, o: Self) -> Self {
        C64(self.0 - o.0, self.1 - o.1)
    }
    fn twice(self) -> Self {
        C64(2.0 * self.0, 2.0 * self.1)
    }
    fn scaled(self, k: f64) -> Self {
        C64(k * self.0, k * self.1)
    }
    fn log2_abs(self) -> f64 {
        0.5 * (self.0 * self.0 + self.1 * self.1).log2()
    }
}

impl Cx for DoubleExpComplex {
    const ZERO: Self = DoubleExpComplex::ZERO;
    const ONE: Self = DoubleExpComplex::ONE;
    fn minus(self, o: Self) -> Self {
        self + DoubleExpComplex::new(-o.re, -o.im)
    }
    fn twice(self) -> Self {
        self.times_two()
    }
    fn scaled(self, k: f64) -> Self {
        let k = DoubleExp::from_f64(k);
        DoubleExpComplex::new(k * self.re, k * self.im)
    }
    fn log2_abs(self) -> f64 {
        let m = self.magnitude_squared();
        if m.is_zero() {
            f64::NEG_INFINITY
        } else {
            0.5 * m.log2_magnitude()
        }
    }
}

/// The orbit of one `c`: `refs` empty iterates `z <- z^2 + c` with `dc = c`; otherwise
/// `δ <- (2 Z_m + δ) δ + dc` against the reference `Z_0 = 0, Z_1, ...` with rebasing.
struct Orbit<'a, T> {
    formula: Formula,
    refs: &'a [T],
    dc: T,
    m: usize,
    delta: T,
}

impl<'a, T: Cx> Orbit<'a, T> {
    /// At `z` (an absolute value: the reference's `Z_0` is 0).
    fn at(formula: Formula, refs: &'a [T], dc: T, z: T) -> Self {
        Orbit {
            formula,
            refs,
            dc,
            m: 0,
            delta: z,
        }
    }

    /// The full z (rebases the delta when due).
    fn z(&mut self) -> T {
        if self.refs.is_empty() {
            return self.delta;
        }
        let z = self.refs[self.m] + self.delta;
        if self.m + 1 == self.refs.len() || (self.m != 0 && z.log2_abs() < self.delta.log2_abs()) {
            self.delta = z;
            self.m = 0;
        }
        z
    }

    fn step(&mut self) {
        if !self.formula.is_mandelbrot() {
            return self.step_generic();
        }
        if self.refs.is_empty() {
            self.delta = self.delta * self.delta + self.dc;
        } else {
            self.delta = (self.refs[self.m].twice() + self.delta) * self.delta + self.dc;
            self.m += 1;
        }
    }

    /// `z^p + c` (F2): directly, or `delta S + dc` with `S = sum W^i Z^(p-1-i)`, `W = Z + delta`.
    fn step_generic(&mut self) {
        let p = self.formula.power();
        if self.refs.is_empty() {
            self.delta = power(self.delta, p) + self.dc;
            return;
        }
        let big = self.refs[self.m];
        let w = big + self.delta;
        let (mut s, mut zk) = (T::ONE, T::ONE);
        for _ in 1..p {
            zk = zk * big;
            s = s * w + zk;
        }
        self.delta = self.delta * s + self.dc;
        self.m += 1;
    }
}

fn power<T: Cx>(z: T, p: u32) -> T {
    (1..p).fold(z, |w, _| w * z)
}

/// `f'(z)` (`2z` for Mandelbrot).
fn prime<T: Cx>(formula: Formula, z: T) -> T {
    match formula {
        Formula::Mandelbrot => z.twice(),
        f => power(z, f.power() - 1).scaled(f64::from(f.power())),
    }
}

/// A log2 magnitude above 1 (|z| > 2) or not a number.
fn beyond_two(l: f64) -> bool {
    l.is_nan() || l > 1.0
}

/// Newton on `f^p(z) - z` from `z = 0`: `|λ|` of the cycle it converges to (None: no
/// convergence, or an iterate escaped). Spends at most `NEWTON_PASSES * p` steps of `budget`.
fn cycle_multiplier<T: Cx>(formula: Formula, refs: &[T], dc: T, p: usize, budget: &mut usize) -> Option<f64> {
    let mut base = T::ZERO;
    for _ in 0..NEWTON_PASSES {
        if *budget < p {
            return None;
        }
        *budget -= p;
        let mut o = Orbit::at(formula, refs, dc, base);
        let mut lam = T::ONE;
        for _ in 0..p {
            let z = o.z();
            if beyond_two(z.log2_abs()) {
                return None;
            }
            lam = prime(formula, z) * lam;
            o.step();
        }
        let fz = o.z();
        let d = (fz.minus(base)) / (lam.minus(T::ONE));
        let dl = d.log2_abs();
        if beyond_two(dl) {
            return None;
        }
        base = base.minus(d);
        let bl = base.log2_abs();
        if dl == f64::NEG_INFINITY || (bl.is_finite() && dl <= bl + CONVERGED_LOG2) {
            return Some(lam.log2_abs().exp2());
        }
    }
    None
}

/// The period of an attracting cycle of one sample `c`: first the periods already found
/// (`known`, in order), then the atom-domain candidates of its orbit up to `max_period` (the
/// indices of new minima of `|z_n|`, smallest first). None: escaped, or nothing attracting
/// found within the budget.
pub fn sample_period<T: Cx>(
    refs: &[T],
    dc: T,
    known: &[usize],
    max_period: usize,
    budget: &mut usize,
) -> Option<usize> {
    sample_period_for(Formula::Mandelbrot, refs, dc, known, max_period, budget)
}

/// [`sample_period`] for any formula (F2; `refs` is an orbit of it).
pub fn sample_period_for<T: Cx>(
    formula: Formula,
    refs: &[T],
    dc: T,
    known: &[usize],
    max_period: usize,
    budget: &mut usize,
) -> Option<usize> {
    for &p in known {
        if cycle_multiplier(formula, refs, dc, p, budget).is_some_and(|l| l < 1.0) {
            return Some(p);
        }
    }
    let n_max = 2 * max_period;
    if *budget < n_max {
        return None;
    }
    *budget -= n_max;
    let mut o = Orbit::at(formula, refs, dc, T::ZERO);
    let mut best = f64::INFINITY;
    let mut candidates = Vec::new();
    o.step();
    for n in 1..=n_max {
        let l = o.z().log2_abs();
        if beyond_two(l) {
            return None;
        }
        if l < best {
            best = l;
            if n <= max_period && !known.contains(&n) {
                candidates.push(n);
            }
        }
        o.step();
    }
    candidates
        .into_iter()
        .find(|&p| cycle_multiplier(formula, refs, dc, p, budget).is_some_and(|l| l < 1.0))
}

/// Periods of attracting cycles at the sample points, with how many samples have each, most
/// first (ties: shorter first). Stops when `budget` steps are spent.
pub fn view_periods<T: Cx>(refs: &[T], samples: &[T], max_period: usize, budget: usize) -> Vec<(usize, usize)> {
    view_periods_for(Formula::Mandelbrot, refs, samples, max_period, budget)
}

/// [`view_periods`] for any formula (F2).
pub fn view_periods_for<T: Cx>(
    formula: Formula,
    refs: &[T],
    samples: &[T],
    max_period: usize,
    mut budget: usize,
) -> Vec<(usize, usize)> {
    let mut found: Vec<(usize, usize)> = Vec::new();
    for &dc in samples {
        if budget == 0 {
            break;
        }
        let known: Vec<usize> = found.iter().map(|f| f.0).collect();
        if let Some(p) = sample_period_for(formula, refs, dc, &known, max_period, &mut budget) {
            match found.iter_mut().find(|f| f.0 == p) {
                Some(f) => f.1 += 1,
                None => found.push((p, 1)),
            }
            found.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        }
    }
    found
}

/// The kernel's candidate list: at most [`MAX_PERIODS`] periods, most samples first, whose
/// Newton tries (`passes * p` probe steps each) fit `max_steps` together. Returns the periods
/// and the probe steps they need.
pub fn choose(found: &[(usize, usize)], passes: usize, max_steps: usize) -> (Vec<u32>, u32) {
    let mut out = Vec::new();
    let mut steps = 0usize;
    for &(p, _) in found {
        if out.len() == MAX_PERIODS {
            break;
        }
        let need = passes * p;
        if steps + need <= max_steps {
            steps += need;
            out.push(p as u32);
        }
    }
    (out, steps as u32)
}

/// A reference orbit's floatexp points as `DoubleExpComplex`.
pub fn from_floatexp(points: &[FloatExpComplex]) -> Vec<DoubleExpComplex> {
    points
        .iter()
        .map(|z| {
            let [re, im] = z.mantissa;
            let e = i64::from(z.exponent);
            DoubleExpComplex::new(
                DoubleExp::from_f64(f64::from(re)).times_power_of_two(e),
                DoubleExp::from_f64(f64::from(im)).times_power_of_two(e),
            )
        })
        .collect()
}

/// `DoubleExpComplex` from an f64 pair.
pub fn dx(re: f64, im: f64) -> DoubleExpComplex {
    DoubleExpComplex::new(DoubleExp::from_f64(re), DoubleExp::from_f64(im))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct(re: f64, im: f64) -> Option<usize> {
        let mut budget = 1 << 22;
        sample_period::<C64>(&[], C64(re, im), &[], 512, &mut budget)
    }

    #[test]
    fn finds_component_periods() {
        // Main cardioid, period-2 bulb, period-3 bulb (top).
        assert_eq!(direct(-0.1, 0.1), Some(1));
        assert_eq!(direct(-1.0, 0.2), Some(2));
        assert_eq!(direct(-0.1226, 0.7449), Some(3));
        // Exterior: escapes.
        assert_eq!(direct(1.0, 1.0), None);
        // Near the edge of a period-22 component (|λ| ~ 0.99; the -3/4 neck region): the orbit
        // takes thousands of periods to settle, Newton from 0 does not need them.
        let p = direct(-0.787_329_172_181_723_8, 0.137_594_319_554_082_24);
        assert_eq!(p, Some(22));
    }

    #[test]
    fn perturbed_matches_direct() {
        // Against a reference orbit at a nearby point: the same periods.
        let (cr, ci) = (-0.787_9, 0.136_2);
        let mut refs = vec![DoubleExpComplex::ZERO];
        let (mut zr, mut zi) = (0.0f64, 0.0f64);
        for _ in 0..4000 {
            let t = zr * zr - zi * zi + cr;
            zi = 2.0 * zr * zi + ci;
            zr = t;
            if zr * zr + zi * zi > 4.0 {
                break;
            }
            refs.push(dx(zr, zi));
        }
        for (re, im) in [(-0.787_329_172_181_723_8, 0.137_594_319_554_082_24), (-0.7878, 0.1362)] {
            let mut budget = 1 << 22;
            let pert = sample_period(&refs, dx(re - cr, im - ci), &[], 512, &mut budget);
            assert_eq!(pert, direct(re, im), "{re} {im}");
        }
    }

    #[test]
    fn view_periods_and_choice() {
        let samples: Vec<C64> = (0..9)
            .map(|k| C64(-0.7873 + 1e-4 * (k % 3) as f64, 0.1376 + 1e-4 * (k / 3) as f64))
            .collect();
        let found = view_periods(&[], &samples, 512, 1 << 22);
        assert_eq!(found.first().map(|f| f.0), Some(22), "{found:?}");
        let (periods, steps) = choose(&[(22, 9), (44, 1), (300, 1)], 10, 1000);
        assert_eq!(periods, vec![22, 44]);
        assert_eq!(steps, 660);
        // The budget bounds the work.
        let mut b = 100;
        assert_eq!(sample_period::<C64>(&[], samples[0], &[], 512, &mut b), None);
    }

    /// F2: the generic step at p = 2 finds Mandelbrot's periods, directly and perturbed; z^3 + c
    /// has its period-2 component around c = i.
    #[test]
    fn generic_formula_periods() {
        let g = Formula::Multibrot(2);
        for (re, im) in [(-0.1, 0.1), (-1.0, 0.2), (-0.1226, 0.7449), (1.0, 1.0)] {
            let mut budget = 1 << 22;
            assert_eq!(
                sample_period_for::<C64>(g, &[], C64(re, im), &[], 512, &mut budget),
                direct(re, im)
            );
        }
        let samples: Vec<C64> = (0..9)
            .map(|k| C64(-0.7873 + 1e-4 * (k % 3) as f64, 0.1376 + 1e-4 * (k / 3) as f64))
            .collect();
        assert_eq!(
            view_periods_for(g, &[], &samples, 512, 1 << 22),
            view_periods(&[], &samples, 512, 1 << 22)
        );
        let mut budget = 1 << 20;
        let cubic = sample_period_for::<C64>(Formula::Multibrot(3), &[], C64(0.02, 1.01), &[], 64, &mut budget);
        assert_eq!(cubic, Some(2));
    }
}
