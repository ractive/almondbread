//! The fractal formula `z <- f(z) + c` (F2, kb: plans/f2-pluggable-formulas,
//! synthesis/other-fractal-types §6).
//!
//! An enum, not a trait object: every hot loop matches it once, outside the loop, so the
//! Mandelbrot loops stay the monomorphic code they were before F2 (bit-identical orbits, fields
//! and timings). Each formula is a set of hooks (the step, its derivative `f'(z)` and `f''(z)`,
//! the factored perturbation step) that the generic code paths call; Mandelbrot keeps its own
//! specialised code everywhere and never goes through the generic hooks.
//!
//! `Multibrot(p)` is the generic polynomial path `z^p + c` (kb: formulas/multibrot). In F2 it is
//! test-only: `Multibrot(2)` written through the generic code must equal Mandelbrot in the
//! orbit, the oracle and on the GPU, which exercises every hook before F3 makes powers 3..8
//! user-selectable.

use std::sync::{Arc, Mutex};

use crate::complex::BigComplex;
use crate::floatexp::DoubleExpComplex;

/// Largest Multibrot power the generic paths accept (KF supports 2..10; kb: formulas/multibrot).
pub const MULTIBROT_MAX_POWER: u32 = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Formula {
    /// `z^2 + c`: the specialised fast path (formula 0 in the kernels).
    #[default]
    Mandelbrot,
    /// `z^p + c`, `2 <= p <= MULTIBROT_MAX_POWER`, through the generic hooks (kernel formula 1).
    /// F2: exercised by tests only (p = 2 must equal Mandelbrot).
    Multibrot(u32),
    /// The Julia set of `z^2 + k` (F4, kb: plans/f4-deep-julia, formulas/julia; kernel formula
    /// 2): the pixel is the start `z_0`, `k` is fixed. Its orbits are Mandelbrot orbits with
    /// `c = k` (the step hooks are Mandelbrot's); the view-centre orbit starts at the centre
    /// ([`crate::ReferenceOrbitComputer::with_start`]) and the critical orbit (from 0) is the
    /// Mandelbrot orbit of `k`.
    Julia(JuliaK),
}

/// The parameter `k` of a Julia set, in high precision, as a small `Copy` handle: values are
/// interned (equal values, whatever their precision, share one handle), so [`Formula`] stays
/// `Copy + Eq + Hash` and everything keyed by the formula (references, tables, pipelines, the
/// scheduler's job key) tells two Julia sets apart. A session interns a handful (each
/// "open this Julia set" adds at most one).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct JuliaK(u32);

static JULIA_KS: Mutex<Vec<Arc<BigComplex>>> = Mutex::new(Vec::new());

impl JuliaK {
    /// The handle of `k` (an existing one if the value was interned before).
    pub fn new(k: &BigComplex) -> Self {
        let mut ks = JULIA_KS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = ks.iter().position(|v| v.re == k.re && v.im == k.im) {
            return Self(i as u32);
        }
        ks.push(Arc::new(k.clone()));
        Self((ks.len() - 1) as u32)
    }

    /// `k` at the precision it was given with (exact: a larger precision only pads zeros).
    pub fn value(self) -> Arc<BigComplex> {
        let ks = JULIA_KS.lock().unwrap_or_else(|e| e.into_inner());
        ks[self.0 as usize].clone()
    }

    /// `k` rounded to f64.
    pub fn approx(self) -> (f64, f64) {
        let v = self.value().to_f64();
        (v[0], v[1])
    }
}

impl core::fmt::Debug for JuliaK {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (re, im) = self.approx();
        write!(f, "JuliaK({re:e}, {im:e})")
    }
}

type C64 = (f64, f64);

#[inline(always)]
fn cmul(a: C64, b: C64) -> C64 {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

impl Formula {
    /// `Multibrot(p)` for a supported power (2 included: the generic path), else None.
    pub fn multibrot(power: u32) -> Option<Self> {
        (2..=MULTIBROT_MAX_POWER)
            .contains(&power)
            .then_some(Self::Multibrot(power))
    }

    /// The degree of the polynomial `f`.
    pub fn power(self) -> u32 {
        match self {
            Self::Mandelbrot | Self::Julia(_) => 2,
            Self::Multibrot(p) => p,
        }
    }

    /// The Julia set's `k` (None for the parameter-plane formulas).
    pub fn julia_k(self) -> Option<JuliaK> {
        match self {
            Self::Julia(k) => Some(k),
            _ => None,
        }
    }

    pub fn is_julia(self) -> bool {
        matches!(self, Self::Julia(_))
    }

    pub fn is_mandelbrot(self) -> bool {
        self == Self::Mandelbrot
    }

    /// The kernels' `kFormula` function constant (0: Mandelbrot, compiled to the pre-F2 code).
    pub fn kernel_code(self) -> u32 {
        match self {
            Self::Mandelbrot => 0,
            Self::Multibrot(_) => 1,
            Self::Julia(_) => 2,
        }
    }

    /// Stable name (CLI `--formula`, FFI, persisted settings): `mandelbrot`, `multibrot3`, ...;
    /// `julia` (its `k` travels separately).
    pub fn name(self) -> String {
        match self {
            Self::Mandelbrot => "mandelbrot".into(),
            Self::Multibrot(p) => format!("multibrot{p}"),
            Self::Julia(_) => "julia".into(),
        }
    }

    /// Parses a name; `julia` needs its `k` and is not parsed here.
    pub fn parse(v: &str) -> Option<Self> {
        if v == "mandelbrot" {
            return Some(Self::Mandelbrot);
        }
        v.strip_prefix("multibrot")?.parse().ok().and_then(Self::multibrot)
    }

    // -----------------------------------------------------------------------------------
    // Generic hooks in f64 (CPU references, oracle, overlay, BLA). Mandelbrot callers keep
    // their own inline code; these are for the other formulas (and tests).

    /// `z^k` for k >= 1 by repeated multiplication (k is small).
    #[inline]
    fn cpow(z: C64, k: u32) -> C64 {
        let mut w = z;
        for _ in 1..k {
            w = cmul(w, z);
        }
        w
    }

    /// `f(z) + c`.
    #[inline]
    pub fn step(self, z: C64, c: C64) -> C64 {
        let w = match self {
            Self::Mandelbrot | Self::Julia(_) => cmul(z, z),
            Self::Multibrot(p) => Self::cpow(z, p),
        };
        (w.0 + c.0, w.1 + c.1)
    }

    /// `f'(z)` (`2z` for Mandelbrot): the multiplier of `dz/dc`, `dz/dz1` and the BLA's `A`.
    #[inline]
    pub fn prime(self, z: C64) -> C64 {
        let p = self.power();
        let w = if p == 2 { z } else { Self::cpow(z, p - 1) };
        (f64::from(p) * w.0, f64::from(p) * w.1)
    }

    /// `f''(z)` (2 for Mandelbrot).
    #[inline]
    pub fn second(self, z: C64) -> C64 {
        let p = self.power();
        let k = f64::from(p * (p - 1));
        if p == 2 {
            return (k, 0.0);
        }
        let w = if p == 3 { z } else { Self::cpow(z, p - 2) };
        (k * w.0, k * w.1)
    }

    /// The factor `S` of the perturbation step `z' = z S + c` with `W = Z + z`
    /// (`S = sum_{i<p} W^i Z^{p-1-i}`, Horner; `2Z + z` for p = 2; kb: formulas/multibrot).
    #[inline]
    pub fn delta_factor(self, big_z: C64, w: C64) -> C64 {
        let p = self.power();
        let (mut s, mut zk) = ((1.0, 0.0), (1.0, 0.0));
        for _ in 1..p {
            zk = cmul(zk, big_z);
            let t = cmul(s, w);
            s = (t.0 + zk.0, t.1 + zk.1);
        }
        s
    }

    // Extended range (deep oracle, Newton, BLA tables of big references).

    fn cpow_x(z: DoubleExpComplex, k: u32) -> DoubleExpComplex {
        let mut w = z;
        for _ in 1..k {
            w = w * z;
        }
        w
    }

    /// `f(z) + c` in extended range.
    pub fn step_x(self, z: DoubleExpComplex, c: DoubleExpComplex) -> DoubleExpComplex {
        Self::cpow_x(z, self.power()) + c
    }

    /// [`Formula::prime`] in extended range.
    pub fn prime_x(self, z: DoubleExpComplex) -> DoubleExpComplex {
        let p = self.power();
        let w = if p == 2 { z } else { Self::cpow_x(z, p - 1) };
        let k = crate::floatexp::DoubleExp::from_f64(f64::from(p));
        DoubleExpComplex::new(k * w.re, k * w.im)
    }

    /// [`Formula::delta_factor`] in extended range.
    pub fn delta_factor_x(self, big_z: DoubleExpComplex, w: DoubleExpComplex) -> DoubleExpComplex {
        let (mut s, mut zk) = (DoubleExpComplex::ONE, DoubleExpComplex::ONE);
        for _ in 1..self.power() {
            zk = zk * big_z;
            s = s * w + zk;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for f in [Formula::Mandelbrot, Formula::Multibrot(2), Formula::Multibrot(8)] {
            assert_eq!(Formula::parse(&f.name()), Some(f));
        }
        assert_eq!(Formula::parse("multibrot1"), None);
        assert_eq!(Formula::parse("multibrot9"), None);
        assert_eq!(Formula::parse("burningship"), None);
        assert_eq!(Formula::default(), Formula::Mandelbrot);
    }

    #[test]
    fn power_two_hooks_are_mandelbrot() {
        let m = Formula::Multibrot(2);
        let (z, c) = ((0.3, -0.7), (-0.1, 0.65));
        let s = m.step(z, c);
        assert!((s.0 - (z.0 * z.0 - z.1 * z.1 + c.0)).abs() < 1e-15);
        assert!((s.1 - (2.0 * z.0 * z.1 + c.1)).abs() < 1e-15);
        assert_eq!(m.prime(z), (2.0 * z.0, 2.0 * z.1));
        assert_eq!(m.second(z), (2.0, 0.0));
        let (big, d) = ((0.25, 0.5), (1e-3, -2e-3));
        let f = m.delta_factor(big, (big.0 + d.0, big.1 + d.1));
        assert!((f.0 - (2.0 * big.0 + d.0)).abs() < 1e-15 && (f.1 - (2.0 * big.1 + d.1)).abs() < 1e-15);
    }

    #[test]
    fn delta_factor_is_the_perturbation_of_the_power() {
        for p in 2..=MULTIBROT_MAX_POWER {
            let f = Formula::Multibrot(p);
            let (big, d) = ((0.4, -0.3), (1e-4, 3e-5));
            let w = (big.0 + d.0, big.1 + d.1);
            let s = f.delta_factor(big, w);
            let exact = {
                let a = f.step(w, (0.0, 0.0));
                let b = f.step(big, (0.0, 0.0));
                (a.0 - b.0, a.1 - b.1)
            };
            let got = cmul(d, s);
            assert!(((got.0 - exact.0).hypot(got.1 - exact.1)) < 1e-15, "p = {p}");
        }
    }

    /// F4: `k` is interned by value (precision aside) and the Julia formula steps like
    /// Mandelbrot with `c = k`.
    #[test]
    fn julia_ks_are_interned_by_value() {
        // (Exact binary values: "0.1" rounds differently at 128 and 1024 bits, two values.)
        let a = JuliaK::new(&BigComplex::parse("-0.75", "0.125", 128).unwrap());
        let b = JuliaK::new(&BigComplex::parse("-0.75", "0.125", 1024).unwrap());
        let c = JuliaK::new(&BigComplex::parse("-0.75", "0.1250001", 128).unwrap());
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.approx(), (-0.75, 0.125));
        let j = Formula::Julia(a);
        assert_eq!((j.power(), j.kernel_code(), j.name()), (2, 2, "julia".to_string()));
        assert_eq!(j.julia_k(), Some(a));
        assert!(!j.is_mandelbrot() && j.is_julia());
        assert_eq!(Formula::parse("julia"), None);
        assert_eq!(
            j.step((0.3, 0.2), (-0.75, 0.1)),
            Formula::Mandelbrot.step((0.3, 0.2), (-0.75, 0.1))
        );
    }
}
