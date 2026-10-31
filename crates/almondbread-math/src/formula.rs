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

use crate::bigfloat::BigFloat;
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
    /// F9, the turn (kb: plans/f9-turn): `z^2 + c` on a plane of the 4-dimensional space of
    /// start `z_0` and parameter `c`, turned by an angle `t` between the Mandelbrot plane (the
    /// pixel is `c`, `z_0` fixed) and the Julia plane (the pixel is `z_0`, `c` fixed). A pixel
    /// at view coordinate `p` is `c = c* + cos t (p - P)`, `z_0 = z* + sin t (p - P)` with the
    /// pivot `(c*, z*)` and its view coordinate `P = cos t c* + sin t z*` ([`TurnPlane`]). The
    /// ends are other formulas where they can be: `t = 0` with `z* = 0` is
    /// [`Formula::Mandelbrot`], `t = 90°` is [`Formula::Julia`] of `k = c*`. Its orbits are
    /// Mandelbrot orbits (the step hooks are Mandelbrot's); the view-centre orbit starts at its
    /// `z_0` and the rebase target is the critical orbit (from 0) of the same `c` (kernel
    /// formula 9).
    Turn(TurnPlane),
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

/// The plane of a [`Formula::Turn`] view (F9, kb: plans/f9-turn): the turn angle in hundredths
/// of a degree (`0 ..= 9000`) and a pivot `(c*, z*)` on the plane in high precision, interned by
/// value like [`JuliaK`] (so the formula stays `Copy + Eq + Hash`). Turning keeps the view
/// centre's point of the 4-space: the new plane's pivot is that point, so a gesture of the turn
/// slider interns one pivot.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TurnPlane {
    pivot: u32,
    centidegrees: u16,
}

/// Interned turn pivots `(c*, z*)`.
static TURN_PIVOTS: Mutex<Vec<Arc<(BigComplex, BigComplex)>>> = Mutex::new(Vec::new());

/// The largest turn angle in hundredths of a degree (90°: the Julia plane).
pub const TURN_MAX_CENTIDEGREES: u16 = 9000;

/// `(cos t, sin t)` of a turn angle in hundredths of a degree: exact at 0 and 90°, else f64's
/// (every path that maps pixels of a turned view calls this, so they agree bit for bit).
pub fn turn_cos_sin(centidegrees: u16) -> (f64, f64) {
    match centidegrees {
        0 => (1.0, 0.0),
        TURN_MAX_CENTIDEGREES => (0.0, 1.0),
        t => {
            let (s, c) = (f64::from(t) * (std::f64::consts::PI / 18000.0)).sin_cos();
            (c, s)
        }
    }
}

impl TurnPlane {
    /// The plane at `centidegrees` (clamped to `0 ..= 9000`) through the pivot `(c, z0)`.
    pub fn new(c: &BigComplex, z0: &BigComplex, centidegrees: u16) -> Self {
        let mut ps = TURN_PIVOTS.lock().unwrap_or_else(|e| e.into_inner());
        let i = match ps
            .iter()
            .position(|v| v.0.re == c.re && v.0.im == c.im && v.1.re == z0.re && v.1.im == z0.im)
        {
            Some(i) => i,
            None => {
                ps.push(Arc::new((c.clone(), z0.clone())));
                ps.len() - 1
            }
        };
        Self {
            pivot: i as u32,
            centidegrees: centidegrees.min(TURN_MAX_CENTIDEGREES),
        }
    }

    /// The pivot `(c*, z*)` at the precision it was given with.
    pub fn pivot(self) -> Arc<(BigComplex, BigComplex)> {
        let ps = TURN_PIVOTS.lock().unwrap_or_else(|e| e.into_inner());
        ps[self.pivot as usize].clone()
    }

    pub fn centidegrees(self) -> u16 {
        self.centidegrees
    }

    pub fn degrees(self) -> f64 {
        f64::from(self.centidegrees) / 100.0
    }

    /// `(cos t, sin t)`, [`turn_cos_sin`].
    pub fn cos_sin(self) -> (f64, f64) {
        turn_cos_sin(self.centidegrees)
    }

    /// The precision [`TurnPlane::pivot_view`] is computed at: the pivot's own plus 64 bits
    /// (the products with the 53-bit cosine and sine are exact), whatever the caller's, so `P`
    /// is one fixed value and [`TurnPlane::point`] at `p = P` gives the pivot exactly.
    pub fn pivot_bits(self) -> usize {
        let pv = self.pivot();
        pv.0.precision_bits().max(pv.1.precision_bits()) + 64
    }

    /// The pivot's view coordinate `P = cos t c* + sin t z*` (at [`TurnPlane::pivot_bits`]).
    pub fn pivot_view(self) -> BigComplex {
        let bits = self.pivot_bits();
        let pv = self.pivot();
        let (cos, sin) = self.cos_sin();
        let (cb, sb) = (BigFloat::from_f64(cos, bits), BigFloat::from_f64(sin, bits));
        let c = pv.0.with_precision(bits);
        let z = pv.1.with_precision(bits);
        &c.scaled(&cb) + &z.scaled(&sb)
    }

    /// The point `(c, z_0)` of the 4-space at view coordinate `p`, at least at `bits`:
    /// `c = c* + cos t (p - P)`, `z_0 = z* + sin t (p - P)` (the pivot itself at `p = P`).
    pub fn point(self, p: &BigComplex, bits: usize) -> (BigComplex, BigComplex) {
        let bits = bits.max(self.pivot_bits());
        let pv = self.pivot();
        let (cos, sin) = self.cos_sin();
        let w = &p.with_precision(bits) - &self.pivot_view();
        let c = &pv.0.with_precision(bits) + &w.scaled(&BigFloat::from_f64(cos, bits));
        let z = &pv.1.with_precision(bits) + &w.scaled(&BigFloat::from_f64(sin, bits));
        (c, z)
    }

    /// [`TurnPlane::point`] in f64 (overlays, shallow CPU renders).
    pub fn point_f64(self, p: (f64, f64)) -> ((f64, f64), (f64, f64)) {
        let pv = self.pivot();
        let (cos, sin) = self.cos_sin();
        let (c, z) = (pv.0.to_f64(), pv.1.to_f64());
        let pp = (cos * c[0] + sin * z[0], cos * c[1] + sin * z[1]);
        let w = (p.0 - pp.0, p.1 - pp.1);
        (
            (c[0] + cos * w.0, c[1] + cos * w.1),
            (z[0] + sin * w.0, z[1] + sin * w.1),
        )
    }
}

impl core::fmt::Debug for TurnPlane {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let pv = self.pivot();
        let (c, z) = (pv.0.to_f64(), pv.1.to_f64());
        write!(
            f,
            "TurnPlane({}°, c* = ({:e}, {:e}), z* = ({:e}, {:e}))",
            self.degrees(),
            c[0],
            c[1],
            z[0],
            z[1]
        )
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
            Self::Mandelbrot | Self::Julia(_) | Self::Mandelbar | Self::BurningShip | Self::Turn(_) => 2,
            Self::Multibrot(p) => p,
        }
    }

    /// Whether the step is complex-analytic (its derivative one complex multiplier). False for
    /// Mandelbar (F5) and the Burning Ship family (F6): their derivatives, BLA coefficients and
    /// Newton steps are real-linear maps ([`Formula::jacobian`]); the kernels keep a second
    /// complex part per derivative (kb: plans/f5-mandelbar "As built").
    pub fn is_conformal(self) -> bool {
        !matches!(self, Self::Mandelbar | Self::BurningShip)
    }

    /// Whether the step folds the plane (`|x|`, `|y|`; F6, the Burning Ship family): its
    /// perturbation step needs the reference's `X Y` in extended range, its BLA blocks stop at
    /// the fold lines (kb: plans/f6-burning-ship "As built").
    pub fn folds(self) -> bool {
        matches!(self, Self::BurningShip)
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
            // (9, not the next free code: clear of the codes other formula work takes.)
            Self::Turn(_) => 9,
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
            Self::Turn(_) => "turn".into(),
        }
    }

    /// The turned plane (F9; None for the other formulas).
    pub fn turn(self) -> Option<TurnPlane> {
        match self {
            Self::Turn(t) => Some(t),
            _ => None,
        }
    }

    /// Parses a name; `julia` needs its `k` and `turn` its plane: neither is parsed here.
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
            Self::Mandelbrot | Self::Julia(_) | Self::Mandelbar | Self::BurningShip | Self::Turn(_) => 65536.0,
            Self::Multibrot(p) => 2f64.powi((60 / p.max(1)).min(16) as i32),
        }
    }

    /// The smooth iteration offset of an escaped orbit with `|z|^2 = r2`: `1 - log_p(ln|z|)`
    /// (continuous across escape counts: `ln|z|` grows by the factor p per step; Mandelbrot's
    /// `1 - log2(ln|z|)` exactly as before).
    pub fn smooth_offset(self, r2: f64) -> f64 {
        match self {
            Self::Mandelbrot | Self::Julia(_) | Self::Mandelbar | Self::BurningShip | Self::Turn(_) => {
                1.0 - (0.5 * r2.ln()).log2()
            }
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
            Self::Mandelbrot | Self::Julia(_) | Self::Turn(_) => cmul(z, z),
            Self::Multibrot(p) => Self::cpow(z, p),
            // conj(z)^2 = conj(z^2).
            Self::Mandelbar => {
                let w = cmul(z, z);
                (w.0, -w.1)
            }
            // (|x| - i|y|)^2 = x^2 - y^2 - 2i|xy|.
            Self::BurningShip => (z.0 * z.0 - z.1 * z.1, -2.0 * (z.0 * z.1).abs()),
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
            _ => LinearX::conformal(self.prime_x(z)),
        }
    }

    /// The perturbation step `z' = f(Z + z) - f(Z) + c` (F5: every formula; the conformal ones
    /// are `z S + c` with [`Formula::delta_factor`], Mandelbar `conj((2Z + z) z) + c`).
    #[inline]
    pub fn delta_step(self, big_z: C64, z: C64, c: C64) -> C64 {
        self.delta_step_scaled(big_z, z, z, c, big_z.0 * big_z.1)
    }

    /// The rescaled perturbation step (the deep loops' `z = w 2^e`, as the kernels' `fStep`):
    /// `w' = (f(Z + sw) - f(Z)) / s + dd` with `sw = s w`, `dd = c / s`. `xy_s` is `X Y / s`
    /// (exact in extended range: it can overflow to inf, which reads as "no fold"), read by the
    /// Burning Ship only (F6): its imaginary part is `-2 diffabs(X Y, d) / s + dd.y` with the
    /// perturbation `d = X y + x Y + x y`, i.e. `-2 diffabs(xy_s, X w.y + w.x Y + sw.x w.y)`.
    #[inline]
    pub fn delta_step_scaled(self, big_z: C64, sw: C64, w: C64, dd: C64, xy_s: f64) -> C64 {
        if self == Self::BurningShip {
            let re = (2.0 * big_z.0 + sw.0) * w.0 - (2.0 * big_z.1 + sw.1) * w.1 + dd.0;
            let d = big_z.0 * w.1 + w.0 * big_z.1 + sw.0 * w.1;
            return (re, -2.0 * diffabs(xy_s, d) + dd.1);
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
        assert_eq!(Formula::parse("celtic"), None);
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
