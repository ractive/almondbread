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
            Self::Mandelbrot => 2,
            Self::Multibrot(p) => p,
        }
    }

    pub fn is_mandelbrot(self) -> bool {
        self == Self::Mandelbrot
    }

    /// The kernels' `kFormula` function constant (0: Mandelbrot, compiled to the pre-F2 code).
    pub fn kernel_code(self) -> u32 {
        match self {
            Self::Mandelbrot => 0,
            Self::Multibrot(_) => 1,
        }
    }

    /// Stable name (CLI `--formula`, FFI, persisted settings): `mandelbrot`, `multibrot3`, ...
    pub fn name(self) -> String {
        match self {
            Self::Mandelbrot => "mandelbrot".into(),
            Self::Multibrot(p) => format!("multibrot{p}"),
        }
    }

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
            Self::Mandelbrot => cmul(z, z),
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
}
