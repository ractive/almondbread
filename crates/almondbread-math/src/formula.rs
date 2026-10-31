//! The fractal formula `z <- f(z) + c` (F2, kb: plans/f2-pluggable-formulas,
//! synthesis/other-fractal-types §6).
//!
//! An enum, not a trait object: every hot loop matches it once, outside the loop, so the
//! Mandelbrot loops stay the monomorphic code they were before F2 (bit-identical orbits, fields
//! and timings). Each formula is a set of hooks (the step, its derivative `f'(z)` and `f''(z)`,
//! the factored perturbation step) that the generic code paths call; Mandelbrot keeps its own
//! specialised code everywhere and never goes through the generic hooks.
//!
//! `Multibrot(p)` is the generic polynomial path `z^p + c` (kb: formulas/multibrot). In F2 it was
//! test-only: `Multibrot(2)` written through the generic code must equal Mandelbrot in the
//! orbit, the oracle and on the GPU. F3 (kb: plans/f3-multibrot) makes powers 3..8
//! user-selectable; `Multibrot(2)` stays a test formula.

use std::sync::{Arc, Mutex};

use crate::complex::BigComplex;
use crate::floatexp::{DoubleExp, DoubleExpComplex};
use crate::linear::{Linear, LinearX};

/// Largest Multibrot power the generic paths accept (KF supports 2..10; kb: formulas/multibrot).
pub const MULTIBROT_MAX_POWER: u32 = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Formula {
    /// `z^2 + c`: the specialised fast path (formula 0 in the kernels).
    #[default]
    Mandelbrot,
    /// `z^p + c`, `2 <= p <= MULTIBROT_MAX_POWER`, through the generic hooks (kernel formula 1).
    /// Users pick p = 3..8 (F3); p = 2 is the tests' check that the generic path is Mandelbrot.
    Multibrot(u32),
    /// The Julia set of `z^2 + k` (F4, kb: plans/f4-deep-julia, formulas/julia; kernel formula
    /// 2): the pixel is the start `z_0`, `k` is fixed. Its orbits are Mandelbrot orbits with
    /// `c = k` (the step hooks are Mandelbrot's); the view-centre orbit starts at the centre
    /// ([`crate::ReferenceOrbitComputer::with_start`]) and the critical orbit (from 0) is the
    /// Mandelbrot orbit of `k`.
    Julia(JuliaK),
    /// The Mandelbar set (Tricorn) `conj(z)^2 + c` (F5, kb: plans/f5-mandelbar,
    /// formulas/mandelbar; kernel formula 3): the first non-conformal formula. Its step is
    /// Mandelbrot's followed by a conjugation (before `+c`); its derivative is a real 2x2
    /// Jacobian ([`Formula::jacobian`], [`Linear`]), not a complex multiplier.
    Mandelbar,
    /// The Burning Ship (F6, kb: plans/f6-burning-ship, formulas/burning-ship; kernel formula 4)
    /// the way up it is conventionally shown: `z <- (|x| - i|y|)^2 + c`, i.e. `x' = x^2 - y^2 +
    /// cx`, `y' = cy - 2|x y|`. That is the literature's `(|x| + i|y|)^2 + c` with the imaginary
    /// axis flipped (our `c` and `z` are the conjugates of the literature's), so the ships stand
    /// upright with the imaginary axis pointing up, as the other formulas are shown. Its
    /// perturbation step needs `diffabs` ([`diffabs`]); its Jacobian is `2z` or Mandelbar's
    /// reflection `δ ↦ conj(2 z δ)` by the sign of `x y` ([`Formula::jacobian`]).
    BurningShip,
    /// The Celtic set (the Burning Ship family's variants, kb: plans/f6b-burning-ship-variants,
    /// formulas/burning-ship "Variants"; kernel formula 5): Kalles Fraktaler's "Celtic"
    /// `w = z^2, z <- |Re w| + i Im w + c`, i.e. `x' = |x^2 - y^2| + cx`, `y' = 2xy + cy`. It
    /// commutes with conjugation (symmetric in the real axis), so the ship's flip changes
    /// nothing: our coordinates are KF's. Folds the real part.
    Celtic,
    /// The Buffalo (kernel formula 6): KF's "Buffalo" `w = z^2, z <- |Re w| + i|Im w| + c` with
    /// the imaginary axis flipped as the ship's: `x' = |x^2 - y^2| + cx`, `y' = cy - 2|xy|`.
    /// Folds both parts.
    Buffalo,
    /// The perpendicular Burning Ship (kernel formula 7): KF's "Perpendicular Burning Ship"
    /// `z <- (x - i|y|)^2 + c` with the imaginary axis flipped as the ship's: `x' = x^2 - y^2 +
    /// cx`, `y' = 2x|y| + cy`, i.e. `(x + i|y|)^2 + c`. Folds at the real axis (the sign of y).
    Perpendicular,
}

/// `|a + d| - |a|` without cancellation (the Burning Ship's perturbed `|XY|`, kb:
/// formulas/burning-ship "Perturbed form"): `±d` while `a + d` keeps the sign of `a`, `±(2a + d)`
/// across the fold. `a` may be infinite (then no fold: `±d`).
#[inline(always)]
pub fn diffabs(a: f64, d: f64) -> f64 {
    if a >= 0.0 {
        if a + d >= 0.0 { d } else { -(2.0 * a + d) }
    } else if a + d > 0.0 {
        2.0 * a + d
    } else {
        -d
    }
}

/// [`diffabs`] in extended range.
pub fn diffabs_x(a: DoubleExp, d: DoubleExp) -> DoubleExp {
    let s = a + d;
    if a.mantissa >= 0.0 {
        if s.mantissa >= 0.0 {
            d
        } else {
            -(a.times_power_of_two(1) + d)
        }
    } else if s.mantissa > 0.0 {
        a.times_power_of_two(1) + d
    } else {
        -d
    }
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
            Self::Mandelbrot
            | Self::Julia(_)
            | Self::Mandelbar
            | Self::BurningShip
            | Self::Celtic
            | Self::Buffalo
            | Self::Perpendicular => 2,
            Self::Multibrot(p) => p,
        }
    }

    /// Whether the step is complex-analytic (its derivative one complex multiplier). False for
    /// Mandelbar (F5) and the Burning Ship family (F6): their derivatives, BLA coefficients and
    /// Newton steps are real-linear maps ([`Formula::jacobian`]); the kernels keep a second
    /// complex part per derivative (kb: plans/f5-mandelbar "As built").
    pub fn is_conformal(self) -> bool {
        !matches!(self, Self::Mandelbar) && !self.folds()
    }

    /// Whether the step folds the plane (`|x|`, `|y|`; F6, the Burning Ship family): its
    /// perturbation step needs the reference's `X Y` in extended range, its BLA blocks stop at
    /// the fold lines (kb: plans/f6-burning-ship "As built").
    pub fn folds(self) -> bool {
        matches!(
            self,
            Self::BurningShip | Self::Celtic | Self::Buffalo | Self::Perpendicular
        )
    }

    /// Whether the step folds the real part, `|x^2 - y^2|` (Celtic, Buffalo): the fold value
    /// `R = X^2 - Y^2` must come exact from the high-precision orbit (its rounding from rounded
    /// `X`, `Y` is absolute, `2^-53 |Z|^2`, far above the deltas of a deep view), so the
    /// references carry it per point (kb: plans/f6b-burning-ship-variants "As built").
    pub fn folds_re(self) -> bool {
        matches!(self, Self::Celtic | Self::Buffalo)
    }

    /// The Burning Ship family's variants (Celtic, Buffalo, Perpendicular; F6b): the ship's
    /// machinery with their own fold cases (kernel formulas 5-7).
    pub fn is_ship_variant(self) -> bool {
        matches!(self, Self::Celtic | Self::Buffalo | Self::Perpendicular)
    }

    /// The signs of a variant's Jacobian at `z` (one-sided on a fold line): the step's Jacobian
    /// is `diag(-1 if neg_re, -1 if neg_im)` times multiplication by `2z`. Celtic and Buffalo
    /// negate the real row where `|x| < |y|`, Buffalo the imaginary one where `x y >= 0` (as the
    /// ship), Perpendicular where `y < 0`.
    pub fn fold_signs(self, z: C64) -> (bool, bool) {
        let neg_re = self.folds_re() && z.0.abs() < z.1.abs();
        let neg_im = match self {
            Self::Buffalo => z.0 * z.1 >= 0.0,
            Self::Perpendicular => z.1 < 0.0,
            _ => false,
        };
        (neg_re, neg_im)
    }

    /// [`Formula::fold_signs`] in extended range.
    pub fn fold_signs_x(self, z: DoubleExpComplex) -> (bool, bool) {
        let neg_re = self.folds_re() && z.re.abs() < z.im.abs();
        let neg_im = match self {
            Self::Buffalo => (z.re.mantissa >= 0.0) == (z.im.mantissa >= 0.0) || z.re.is_zero() || z.im.is_zero(),
            Self::Perpendicular => z.im.mantissa < 0.0,
            _ => false,
        };
        (neg_re, neg_im)
    }

    /// A folding formula's fold values at the reference point `Z`, from `Z` itself (f64):
    /// `[X^2 - Y^2, Q]` with `Q = X Y` (the ship, Buffalo) or `Y` (Perpendicular). The first is
    /// only as good as `Z`'s rounding allows (absolute error `2^-53 |Z|^2`); the deep paths take
    /// it from the orbit ([`Formula::folds_re`]).
    #[inline]
    pub fn fold_values(self, z: C64) -> [f64; 2] {
        let q = if self == Self::Perpendicular { z.1 } else { z.0 * z.1 };
        [(z.0 - z.1) * (z.0 + z.1), q]
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
            Self::Mandelbar => 3,
            Self::BurningShip => 4,
            Self::Celtic => 5,
            Self::Buffalo => 6,
            Self::Perpendicular => 7,
        }
    }

    /// Stable name (CLI `--formula`, FFI, persisted settings): `mandelbrot`, `multibrot3`, ...;
    /// `julia` (its `k` travels separately).
    pub fn name(self) -> String {
        match self {
            Self::Mandelbrot => "mandelbrot".into(),
            Self::Multibrot(p) => format!("multibrot{p}"),
            Self::Julia(_) => "julia".into(),
            Self::Mandelbar => "mandelbar".into(),
            Self::BurningShip => "burningship".into(),
            Self::Celtic => "celtic".into(),
            Self::Buffalo => "buffalo".into(),
            Self::Perpendicular => "perpendicular".into(),
        }
    }

    /// Parses a name; `julia` needs its `k` and is not parsed here.
    pub fn parse(v: &str) -> Option<Self> {
        if v == "mandelbrot" {
            return Some(Self::Mandelbrot);
        }
        if v == "mandelbar" {
            return Some(Self::Mandelbar);
        }
        if v == "burningship" {
            return Some(Self::BurningShip);
        }
        match v {
            "celtic" => return Some(Self::Celtic),
            "buffalo" => return Some(Self::Buffalo),
            "perpendicular" => return Some(Self::Perpendicular),
            _ => {}
        }
        v.strip_prefix("multibrot")?.parse().ok().and_then(Self::multibrot)
    }

    /// The escape radius (F3). Mandelbrot's 2^16 keeps the smooth offset in a small range; for
    /// `z^p` the escaped value reaches `R^p` and its square `R^{2p}`, which must stay inside
    /// float32 (2^128) in the kernels, so high powers escape earlier: `R = 2^min(16, 60/p)`
    /// (p = 3: 2^16, 4: 2^15, 5: 2^12, 6: 2^10, 7: 2^8, 8: 2^7). Also keeps the per-step growth of
    /// the derivatives near escape (`p R^{p-1}`, at most 2^52) far below float32's range
    /// (kb: formulas/multibrot "As built").
    pub fn bailout(self) -> f64 {
        match self {
            Self::Mandelbrot
            | Self::Julia(_)
            | Self::Mandelbar
            | Self::BurningShip
            | Self::Celtic
            | Self::Buffalo
            | Self::Perpendicular => 65536.0,
            Self::Multibrot(p) => 2f64.powi((60 / p.max(1)).min(16) as i32),
        }
    }

    /// The smooth iteration offset of an escaped orbit with `|z|^2 = r2`: `1 - log_p(ln|z|)`
    /// (continuous across escape counts: `ln|z|` grows by the factor p per step; Mandelbrot's
    /// `1 - log2(ln|z|)` exactly as before).
    pub fn smooth_offset(self, r2: f64) -> f64 {
        match self {
            Self::Mandelbrot
            | Self::Julia(_)
            | Self::Mandelbar
            | Self::BurningShip
            | Self::Celtic
            | Self::Buffalo
            | Self::Perpendicular => 1.0 - (0.5 * r2.ln()).log2(),
            Self::Multibrot(p) => 1.0 - (0.5 * r2.ln()).log2() / f64::from(p).log2(),
        }
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
            // conj(z)^2 = conj(z^2).
            Self::Mandelbar => {
                let w = cmul(z, z);
                (w.0, -w.1)
            }
            // (|x| - i|y|)^2 = x^2 - y^2 - 2i|xy|.
            Self::BurningShip => (z.0 * z.0 - z.1 * z.1, -2.0 * (z.0 * z.1).abs()),
            // F6b: |Re z^2| + i Im z^2.
            Self::Celtic => ((z.0 * z.0 - z.1 * z.1).abs(), 2.0 * z.0 * z.1),
            // |Re z^2| - i|Im z^2|.
            Self::Buffalo => ((z.0 * z.0 - z.1 * z.1).abs(), -2.0 * (z.0 * z.1).abs()),
            // (x + i|y|)^2.
            Self::Perpendicular => (z.0 * z.0 - z.1 * z.1, 2.0 * z.0 * z.1.abs()),
        };
        (w.0 + c.0, w.1 + c.1)
    }

    /// The step's Jacobian at `z` as a real-linear map (F5): `f'(z)` for the conformal formulas,
    /// `δ ↦ conj(2 z δ)` (`p = 0, q = conj(2z)`) for Mandelbar. What the non-conformal paths
    /// (BLA matrices, DE and interior derivatives, Newton) compose.
    #[inline]
    pub fn jacobian(self, z: C64) -> Linear {
        match self {
            Self::Mandelbar => Linear {
                p: (0.0, 0.0),
                q: (2.0 * z.0, -2.0 * z.1),
            },
            // F6: y' = cy - 2 s (x y) with s = sign(x y) away from the folds: the matrix
            // [[2x, -2y], [-2sy, -2sx]], Mandelbar's reflection for s = +1 (also on a fold,
            // x y = 0, the one-sided derivative the kernels take), multiplication by 2z for
            // s = -1.
            Self::BurningShip if z.0 * z.1 >= 0.0 => Linear {
                p: (0.0, 0.0),
                q: (2.0 * z.0, -2.0 * z.1),
            },
            // F6b: diag(+-1, +-1) times 2z ([`Formula::fold_signs`]): 2z, Mandelbar's reflection,
            // -2z or the negated reflection.
            Self::Celtic | Self::Buffalo | Self::Perpendicular => {
                let (neg_re, neg_im) = self.fold_signs(z);
                let z2 = (2.0 * z.0, 2.0 * z.1);
                let l = if neg_re == neg_im {
                    Linear::conformal(z2)
                } else {
                    Linear {
                        p: (0.0, 0.0),
                        q: (z2.0, -z2.1),
                    }
                };
                if neg_re { l.scale(-1.0) } else { l }
            }
            _ => Linear::conformal(self.prime(z)),
        }
    }

    /// [`Formula::jacobian`] in extended range.
    pub fn jacobian_x(self, z: DoubleExpComplex) -> LinearX {
        match self {
            Self::Mandelbar => LinearX {
                p: DoubleExpComplex::ZERO,
                q: z.conj().times_two(),
            },
            Self::BurningShip
                if (z.re.mantissa >= 0.0) == (z.im.mantissa >= 0.0) || z.re.is_zero() || z.im.is_zero() =>
            {
                LinearX {
                    p: DoubleExpComplex::ZERO,
                    q: z.conj().times_two(),
                }
            }
            Self::Celtic | Self::Buffalo | Self::Perpendicular => {
                let (neg_re, neg_im) = self.fold_signs_x(z);
                let t = z.times_two();
                let z2 = if neg_re { DoubleExpComplex::new(-t.re, -t.im) } else { t };
                if neg_re == neg_im {
                    LinearX::conformal(z2)
                } else {
                    LinearX {
                        p: DoubleExpComplex::ZERO,
                        q: z2.conj(),
                    }
                }
            }
            _ => LinearX::conformal(self.prime_x(z)),
        }
    }

    /// The perturbation step `z' = f(Z + z) - f(Z) + c` (F5: every formula; the conformal ones
    /// are `z S + c` with [`Formula::delta_factor`], Mandelbar `conj((2Z + z) z) + c`).
    #[inline]
    pub fn delta_step(self, big_z: C64, z: C64, c: C64) -> C64 {
        self.delta_step_scaled(big_z, z, z, c, self.fold_values(big_z))
    }

    /// The rescaled perturbation step (the deep loops' `z = w 2^e`, as the kernels' `fStep`):
    /// `w' = (f(Z + sw) - f(Z)) / s + dd` with `sw = s w`, `dd = c / s`. `xy_s` is `X Y / s`
    /// (exact in extended range: it can overflow to inf, which reads as "no fold"), read by the
    /// Burning Ship only (F6): its imaginary part is `-2 diffabs(X Y, d) / s + dd.y` with the
    /// perturbation `d = X y + x Y + x y`, i.e. `-2 diffabs(xy_s, X w.y + w.x Y + sw.x w.y)`.
    /// `fold = [R / s, Q / s]`, [`Formula::fold_values`] over s: the ship reads `Q = X Y`
    /// (`xy_s` above). Its variants (F6b): Celtic and Buffalo fold `Re((2Z + z) z)` against
    /// `R = X^2 - Y^2`, Buffalo the imaginary part against `X Y` as the ship, Perpendicular
    /// against `Q = Y` (`x|Y + y| - X|Y| = x|Y| + (X + x) diffabs(Y, y)`).
    #[inline]
    pub fn delta_step_scaled(self, big_z: C64, sw: C64, w: C64, dd: C64, fold: [f64; 2]) -> C64 {
        if self == Self::BurningShip {
            let re = (2.0 * big_z.0 + sw.0) * w.0 - (2.0 * big_z.1 + sw.1) * w.1 + dd.0;
            let d = big_z.0 * w.1 + w.0 * big_z.1 + sw.0 * w.1;
            return (re, -2.0 * diffabs(fold[1], d) + dd.1);
        }
        if self.is_ship_variant() {
            let re = (2.0 * big_z.0 + sw.0) * w.0 - (2.0 * big_z.1 + sw.1) * w.1;
            let re = if self.folds_re() { diffabs(fold[0], re) } else { re };
            let im = match self {
                Self::Celtic => 2.0 * (big_z.0 * w.1 + w.0 * big_z.1 + sw.0 * w.1),
                Self::Buffalo => -2.0 * diffabs(fold[1], big_z.0 * w.1 + w.0 * big_z.1 + sw.0 * w.1),
                _ => 2.0 * (w.0 * big_z.1.abs() + (big_z.0 + sw.0) * diffabs(fold[1], w.1)),
            };
            return (re + dd.0, im + dd.1);
        }
        let s = self.delta_factor(big_z, (big_z.0 + sw.0, big_z.1 + sw.1));
        let t = (s.0 * w.0 - s.1 * w.1, s.0 * w.1 + s.1 * w.0);
        match self {
            Self::Mandelbar => (t.0 + dd.0, -t.1 + dd.1),
            _ => (t.0 + dd.0, t.1 + dd.1),
        }
    }

    /// [`Formula::delta_step`] in extended range.
    pub fn delta_step_x(self, big_z: DoubleExpComplex, z: DoubleExpComplex, c: DoubleExpComplex) -> DoubleExpComplex {
        let r = if self.folds_re() {
            (big_z.re - big_z.im) * (big_z.re + big_z.im)
        } else {
            DoubleExp::ZERO
        };
        self.delta_step_x_with(big_z, z, c, r)
    }

    /// [`Formula::delta_step_x`] with the fold value `R = X^2 - Y^2` given (exact, from the
    /// orbit: [`Formula::folds_re`]); the other formulas ignore it.
    pub fn delta_step_x_with(
        self,
        big_z: DoubleExpComplex,
        z: DoubleExpComplex,
        c: DoubleExpComplex,
        r: DoubleExp,
    ) -> DoubleExpComplex {
        if self.is_ship_variant() {
            // P = (2Z + z) z: Re P the perturbation of x^2 - y^2, Im P = 2 (X y + x Y + x y).
            let p = (big_z.times_two() + z) * z;
            let re = if self.folds_re() { diffabs_x(r, p.re) } else { p.re };
            let im = match self {
                Self::Celtic => p.im,
                Self::Buffalo => {
                    let d = big_z.re * z.im + z.re * big_z.im + z.re * z.im;
                    -diffabs_x(big_z.re * big_z.im, d).times_power_of_two(1)
                }
                _ => (z.re * big_z.im.abs() + (big_z.re + z.re) * diffabs_x(big_z.im, z.im)).times_power_of_two(1),
            };
            return DoubleExpComplex::new(re, im) + c;
        }
        if self == Self::BurningShip {
            // Re((2Z + z) z) and -2 diffabs(XY, Xy + xY + xy), each part its own exponent.
            let re = ((big_z.times_two() + z) * z).re;
            let d = big_z.re * z.im + z.re * big_z.im + z.re * z.im;
            let im = -diffabs_x(big_z.re * big_z.im, d).times_power_of_two(1);
            return DoubleExpComplex::new(re, im) + c;
        }
        let t = z * self.delta_factor_x(big_z, big_z + z);
        match self {
            Self::Mandelbar => t.conj() + c,
            _ => t + c,
        }
    }

    /// `f'(z)` (`2z` for Mandelbrot): the multiplier of `dz/dc`, `dz/dz1` and the BLA's `A`.
    /// For a non-conformal formula (Mandelbar) a conformal stand-in with the same singular values
    /// (`|f'| = 2|z|`), for bounds and magnitudes only; its derivatives are
    /// [`Formula::jacobian`].
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
        match self {
            Self::Mandelbar => (z * z).conj() + c,
            Self::BurningShip => {
                let re = z.re * z.re - z.im * z.im;
                let im = -(z.re * z.im).abs().times_power_of_two(1);
                DoubleExpComplex::new(re, im) + c
            }
            Self::Celtic | Self::Buffalo | Self::Perpendicular => {
                let re = z.re * z.re - z.im * z.im;
                let xy2 = (z.re * z.im).times_power_of_two(1);
                let (re, im) = match self {
                    Self::Celtic => (re.abs(), xy2),
                    Self::Buffalo => (re.abs(), -xy2.abs()),
                    _ => (re, (z.re * z.im.abs()).times_power_of_two(1)),
                };
                DoubleExpComplex::new(re, im) + c
            }
            _ => Self::cpow_x(z, self.power()) + c,
        }
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
        assert_eq!(Formula::parse("burningship"), Some(Formula::BurningShip));
        for f in [Formula::Celtic, Formula::Buffalo, Formula::Perpendicular] {
            assert_eq!(Formula::parse(&f.name()), Some(f));
            assert!(f.folds() && !f.is_conformal() && f.is_ship_variant());
        }
        assert!(Formula::Celtic.folds_re() && Formula::Buffalo.folds_re());
        assert!(!Formula::Perpendicular.folds_re() && !Formula::BurningShip.folds_re());
        assert_eq!(Formula::parse("celtic2"), None);
        assert_eq!(Formula::default(), Formula::Mandelbrot);
    }

    #[test]
    fn bailout_keeps_the_escaped_square_in_float32() {
        assert_eq!(Formula::Mandelbrot.bailout(), 65536.0);
        assert_eq!(Formula::Multibrot(2).bailout(), 65536.0);
        assert_eq!(Formula::Multibrot(3).bailout(), 65536.0);
        for p in 2..=MULTIBROT_MAX_POWER {
            let r = Formula::Multibrot(p).bailout();
            // |z_n| < R^p + 2 after the last step; its square fits float32 with room.
            assert!(2.0 * f64::from(p) * r.log2() <= 120.0, "p = {p}");
            assert!(r >= 128.0, "p = {p}");
        }
        // The smooth offset is continuous: one more step (ln|z| times p) is one less offset.
        for p in 2..=MULTIBROT_MAX_POWER {
            let f = Formula::Multibrot(p);
            let r2 = 1e5f64;
            let r2p = r2.powi(p as i32);
            assert!(
                (f.smooth_offset(r2) - f.smooth_offset(r2p) - 1.0).abs() < 1e-12,
                "p = {p}"
            );
        }
        let r2 = 3e9;
        assert_eq!(Formula::Mandelbrot.smooth_offset(r2), 1.0 - (0.5 * r2.ln()).log2());
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

    /// F6b: each variant's perturbation step (f64, rescaled, extended range) is the difference
    /// of two steps, also where `Z` and `Z + z` lie on different sides of a fold, and its
    /// Jacobian is the step's derivative away from the folds.
    #[test]
    fn ship_variants_perturb_and_differentiate() {
        let x = |v: f64| DoubleExp::from_f64(v);
        let xc = |z: C64| DoubleExpComplex::new(x(z.0), x(z.1));
        for f in [
            Formula::BurningShip,
            Formula::Celtic,
            Formula::Buffalo,
            Formula::Perpendicular,
        ] {
            let c = (-0.31, 0.47);
            let mut crossed = 0;
            for (big, d) in [
                ((0.4, -0.3), (1e-4, 3e-5)),
                ((0.3, 0.29995), (-2e-4, 1e-4)), // |x| = |y| crossed
                ((-0.5, 0.5), (1e-3, 1e-3)),     // on the diagonal
                ((0.2, 1e-5), (3e-5, -2e-5)),    // y = 0 crossed
                ((1e-5, -0.7), (-3e-5, 1e-4)),   // x = 0 crossed
                ((-1.1, 0.02), (0.01, -0.03)),
            ] {
                let w = (big.0 + d.0, big.1 + d.1);
                let exact = {
                    let a = f.step(w, c);
                    let b = f.step(big, (0.0, 0.0));
                    (a.0 - b.0, a.1 - b.1)
                };
                let got = f.delta_step(big, d, c);
                assert!(
                    (got.0 - exact.0).hypot(got.1 - exact.1) < 1e-15,
                    "{f:?} {big:?}: {got:?} {exact:?}"
                );
                // Rescaled by s = 2^-10.
                let s = 2f64.powi(-10);
                let fv = f.fold_values(big);
                let ws = f.delta_step_scaled(big, d, (d.0 / s, d.1 / s), (c.0 / s, c.1 / s), [fv[0] / s, fv[1] / s]);
                assert!((ws.0 * s - exact.0).hypot(ws.1 * s - exact.1) < 1e-14, "{f:?} scaled");
                let gx = f.delta_step_x(xc(big), xc(d), xc(c));
                assert!(
                    (gx.re.to_f64() - exact.0).hypot(gx.im.to_f64() - exact.1) < 1e-15,
                    "{f:?} x"
                );
                let gs = f.step_x(xc(w), xc(c));
                let sw = f.step(w, c);
                assert!((gs.re.to_f64() - sw.0).hypot(gs.im.to_f64() - sw.1) < 1e-15);
                if f.fold_signs(big) != f.fold_signs(w) || (f.folds() && (big.0 * big.1 > 0.0) != (w.0 * w.1 > 0.0)) {
                    crossed += 1;
                }
                // The Jacobian against central differences (far from the folds only).
                let (neg_re, neg_im) = f.fold_signs(big);
                let h = 1e-7;
                if big.0.abs().min(big.1.abs()).min((big.0.abs() - big.1.abs()).abs()) > 1e-3 {
                    let j = f.jacobian(big).matrix();
                    let jx = f.jacobian_x(xc(big)).to_f64().matrix();
                    for (k, e) in [(1.0, 0.0), (0.0, 1.0)].into_iter().enumerate() {
                        let a = f.step((big.0 + h * e.0, big.1 + h * e.1), c);
                        let b = f.step((big.0 - h * e.0, big.1 - h * e.1), c);
                        let col = ((a.0 - b.0) / (2.0 * h), (a.1 - b.1) / (2.0 * h));
                        assert!(
                            (j[0][k] - col.0).abs() < 1e-6 && (j[1][k] - col.1).abs() < 1e-6,
                            "{f:?} {big:?} {neg_re} {neg_im}"
                        );
                        assert!((jx[0][k] - j[0][k]).abs() < 1e-12 && (jx[1][k] - j[1][k]).abs() < 1e-12);
                    }
                }
            }
            assert!(crossed >= 2, "{f:?}: the cases cross a fold");
        }
        // Celtic is symmetric in the real axis; Buffalo and Perpendicular are not.
        let (c, cc) = ((-1.3, 0.2), (-1.3, -0.2));
        let mut z = [(0.0, 0.0); 2];
        for _ in 0..5 {
            z = [Formula::Celtic.step(z[0], c), Formula::Celtic.step(z[1], cc)];
        }
        assert!((z[0].0 - z[1].0).abs() < 1e-12 && (z[0].1 + z[1].1).abs() < 1e-12);
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
