//! Where we look: the centre of the view and the size of one view point, at any depth
//! (M3b, kb: decisions/arbitrary-precision, techniques/extended-range-floats).
//!
//! The centre is an arbitrary-precision complex ([`Center`], a `BigComplex` whose precision
//! grows with the zoom); the scale is an extended-range positive number ([`Scale`], an f64
//! mantissa with an i64 exponent), so views of 1e-1000 per point are as ordinary as 1e-3.
//! Callers read f64 views of both where they are in range (`re()`, `Scale::to_f64`), and ask for
//! offsets between centres *in units of a scale* ([`Center::delta_in`]), which stay small and
//! exact enough in f64 at any depth. Motion applies exact offsets: a pan of `dx` points adds
//! `dx * scale` to the centre in its own precision, so no position is ever lost to rounding.

use core::cmp::Ordering;
use core::fmt;
use core::ops::{Div, Mul};

use almondbread_math::{BigComplex, BigFloat, DoubleExp};

/// `2^k` as f64 for `k` in the normal range, 0 or inf outside.
fn pow2(k: i64) -> f64 {
    if k < -1074 {
        0.0
    } else if k < -1022 {
        // Subnormal: two exact steps.
        pow2(k + 64) * pow2(-64)
    } else if k > 1023 {
        f64::INFINITY
    } else {
        f64::from_bits(((k + 1023) as u64) << 52)
    }
}

/// A positive extended-range number: `mant * 2^exp` with `mant` in [1, 2). Arithmetic with f64
/// factors rounds exactly like f64 arithmetic on the value would in f64's normal range (only
/// the power of two is carried separately), so shallow views compute bit-identical texel scales.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    mant: f64,
    exp: i64,
}

impl Scale {
    /// From a positive finite f64 (non-positive or non-finite values give 1, the caller checks).
    pub fn from_f64(v: f64) -> Self {
        if !(v.is_finite() && v > 0.0) {
            return Self { mant: 1.0, exp: 0 };
        }
        Self::normalised(v, 0)
    }

    /// `2^k`.
    pub fn pow2(k: i64) -> Self {
        Self { mant: 1.0, exp: k }
    }

    /// `2^l` for any finite `l` (beyond f64's range too).
    pub fn from_log2(l: f64) -> Self {
        if !l.is_finite() {
            return Self { mant: 1.0, exp: 0 };
        }
        let k = l.floor();
        Self::normalised((l - k).exp2(), k as i64)
    }

    fn normalised(m: f64, e: i64) -> Self {
        if !(m.is_finite() && m > 0.0) {
            return Self { mant: 1.0, exp: e };
        }
        let bits = m.to_bits();
        let be = ((bits >> 52) & 0x7ff) as i64;
        if be == 0 {
            // Subnormal mantissa (never from normal use): renormalise through a multiply.
            return Self::normalised(m * 18446744073709551616.0, e - 64);
        }
        Self {
            mant: f64::from_bits((bits & ((1u64 << 52) - 1)) | (1023u64 << 52)),
            exp: e + be - 1023,
        }
    }

    /// The value as f64 (0 below f64's range, inf above).
    pub fn to_f64(self) -> f64 {
        if self.exp < -1022 {
            return self.mant * pow2(self.exp + 60) * pow2(-60);
        }
        if self.exp > 1023 {
            return f64::INFINITY;
        }
        self.mant * pow2(self.exp)
    }

    /// Mantissa in [1, 2) and binary exponent.
    pub fn parts(self) -> (f64, i64) {
        (self.mant, self.exp)
    }

    pub fn log2(self) -> f64 {
        self.exp as f64 + self.mant.log2()
    }

    pub fn log10(self) -> f64 {
        self.log2() * core::f64::consts::LOG10_2
    }

    pub fn ln(self) -> f64 {
        self.log2() * core::f64::consts::LN_2
    }

    /// `self / other` as f64 (scale ratios are moderate).
    pub fn ratio(self, other: Scale) -> f64 {
        self.mant / other.mant * pow2(self.exp - other.exp)
    }

    pub fn double_exp(self) -> DoubleExp {
        DoubleExp::new(self.mant * 0.5, self.exp + 1)
    }

    /// Exact BigFloat of `x * self` for |x| < f64::MAX / 2. Beyond (a pan by 1e308 points),
    /// the product saturates at +-f64::MAX instead of overflowing to inf; NaN is 0 (kb:
    /// techniques/hang-prevention).
    pub fn big_times(self, x: f64, precision_bits: usize) -> BigFloat {
        let p = x * self.mant;
        let p = if p.is_nan() { 0.0 } else { p.clamp(-f64::MAX, f64::MAX) };
        BigFloat::from_f64(p, precision_bits.max(64)).times_power_of_two(self.exp)
    }

    pub fn min(self, b: Scale) -> Scale {
        if b < self { b } else { self }
    }

    pub fn max(self, b: Scale) -> Scale {
        if b > self { b } else { self }
    }

    /// Parses a positive decimal (`3.2e-1000`).
    pub fn parse(text: &str) -> Option<Self> {
        let b = BigFloat::parse(text, 128)?;
        if b.is_zero() || b.is_negative() {
            return None;
        }
        let d = b.double_exp();
        Some(Self::normalised(d.mantissa, d.exponent))
    }

    /// `%g`-like text with `digits` significant digits at any exponent (`1.2346e-1000`).
    pub fn to_text(self, digits: usize) -> String {
        let l10 = self.log10();
        if (-300.0..300.0).contains(&l10) {
            return crate::cfmt::g(self.to_f64(), digits);
        }
        let mut e10 = l10.floor();
        let mut m = 10f64.powf(l10 - e10);
        let r = 10f64.powi(digits.max(1) as i32 - 1);
        m = (m * r).round() / r;
        if m >= 10.0 {
            m /= 10.0;
            e10 += 1.0;
        }
        let ms = crate::cfmt::g(m, digits);
        format!("{ms}e{}{:02}", if e10 < 0.0 { '-' } else { '+' }, e10.abs() as i64)
    }
}

impl PartialOrd for Scale {
    fn partial_cmp(&self, b: &Scale) -> Option<Ordering> {
        Some(self.exp.cmp(&b.exp).then(self.mant.total_cmp(&b.mant)))
    }
}

impl Mul<f64> for Scale {
    type Output = Scale;
    fn mul(self, f: f64) -> Scale {
        Scale::normalised(self.mant * f, self.exp)
    }
}

impl Div<f64> for Scale {
    type Output = Scale;
    fn div(self, f: f64) -> Scale {
        Scale::normalised(self.mant / f, self.exp)
    }
}

impl fmt::Display for Scale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text(f.precision().unwrap_or(6)))
    }
}

/// Centre of a view in the complex plane: an arbitrary-precision complex. Its precision is
/// raised as the view zooms in ([`Viewport::ensure_precision`]); offsets are added exactly up
/// to that precision.
#[derive(Clone, Debug, PartialEq)]
pub struct Center {
    c: BigComplex,
}

/// Smallest centre precision (whole 64-bit limbs).
pub const MIN_CENTER_BITS: usize = 64;
/// Largest centre precision: a little past the deepest scale ([`MIN_SCALE_LOG2`]) plus the
/// guard bits, whole limbs. Neither the view nor a pasted coordinate takes more.
pub const MAX_CENTER_BITS: usize = ((-MIN_SCALE_LOG2 + 1024 + CENTER_GUARD_BITS) as usize).div_ceil(64) * 64;
/// Most digits a coordinate may have ([`Center::parse`]): about 6x the ~3 350 digits that
/// [`MAX_CENTER_BITS`] holds, so any coordinate another program writes at our depths is
/// accepted (then rounded), and parsing stays around a millisecond.
pub const MAX_COORDINATE_DIGITS: usize = 20_000;

impl Center {
    /// From f64 coordinates (exact).
    pub fn new(re: f64, im: f64) -> Self {
        Self {
            c: BigComplex::from_f64(re, im, MIN_CENTER_BITS),
        }
    }

    pub fn from_big(c: BigComplex) -> Self {
        let bits = c.precision_bits().max(MIN_CENTER_BITS);
        Self {
            c: c.with_precision(bits),
        }
    }

    /// Parses decimal coordinates (the format coordinates cross the FFI in) with enough
    /// precision for every digit given, up to [`MAX_CENTER_BITS`] (the deepest view uses no
    /// more). `None` for more than [`MAX_COORDINATE_DIGITS`] digits: parsing is quadratic in
    /// the digit count (a pasted million digits took 1.6 s and left a 52 000-limb centre;
    /// kb: techniques/hang-prevention).
    pub fn parse(re: &str, im: &str) -> Option<Self> {
        let digits = |s: &str| s.bytes().filter(u8::is_ascii_digit).count();
        let n = digits(re).max(digits(im));
        if n > MAX_COORDINATE_DIGITS {
            return None;
        }
        let bits = ((n as f64 * 3.33) as usize + 16).clamp(MIN_CENTER_BITS, MAX_CENTER_BITS);
        let c = BigComplex::parse(re.trim(), im.trim(), bits)?;
        Some(Self { c })
    }

    pub fn big(&self) -> &BigComplex {
        &self.c
    }

    pub fn precision_bits(&self) -> usize {
        self.c.precision_bits()
    }

    /// The same point with at least `bits` of precision (widening is exact).
    pub fn with_min_precision(&self, bits: usize) -> Self {
        if self.c.precision_bits() >= bits {
            return self.clone();
        }
        Self {
            c: self.c.with_precision(bits),
        }
    }

    /// Real part, rounded to f64.
    pub fn re(&self) -> f64 {
        self.c.re.to_f64()
    }

    /// Imaginary part, rounded to f64.
    pub fn im(&self) -> f64 {
        self.c.im.to_f64()
    }

    /// Decimal strings that round-trip exactly at the centre's precision.
    pub fn to_strings(&self) -> (String, String) {
        (self.c.re.to_decimal_string(None), self.c.im.to_decimal_string(None))
    }

    /// Decimal strings with `digits` significant digits.
    pub fn to_strings_digits(&self, digits: usize) -> (String, String) {
        (
            self.c.re.to_decimal_string(Some(digits)),
            self.c.im.to_decimal_string(Some(digits)),
        )
    }

    /// `self + (dre, dim)`, exact up to the centre's precision.
    pub fn offset(&self, dre: f64, dim: f64) -> Self {
        let bits = self.precision_bits();
        Self {
            c: BigComplex::new(
                &self.c.re + BigFloat::from_f64(dre, bits),
                &self.c.im + BigFloat::from_f64(dim, bits),
            ),
        }
    }

    /// `self + (dx, dy) * unit`, exact up to the centre's precision (the product `dx * unit` is
    /// rounded once, like an f64 product).
    pub fn offset_scaled(&self, dx: f64, dy: f64, unit: Scale) -> Self {
        let bits = self.precision_bits();
        Self {
            c: BigComplex::new(
                &self.c.re + unit.big_times(dx, bits),
                &self.c.im + unit.big_times(dy, bits),
            ),
        }
    }

    /// `self - other` in complex units as f64 (0 below f64's range: use [`Center::delta_in`]
    /// for deep views).
    pub fn delta_from(&self, other: &Center) -> (f64, f64) {
        ((&self.c.re - &other.c.re).to_f64(), (&self.c.im - &other.c.im).to_f64())
    }

    /// `(self - other) / unit` as f64: offsets in pixels or texels at any depth.
    pub fn delta_in(&self, other: &Center, unit: Scale) -> (f64, f64) {
        let u = unit.double_exp();
        let f = |d: BigFloat| (d.double_exp() / u).to_f64();
        (f(&self.c.re - &other.c.re), f(&self.c.im - &other.c.im))
    }

    /// `|self - other| / unit` (inf if the offset is beyond f64 range in those units).
    pub fn distance_in(&self, other: &Center, unit: Scale) -> f64 {
        let (x, y) = self.delta_in(other, unit);
        x.hypot(y)
    }
}

/// Centre, size of one view point (not one pixel: independent of the display scale) and the
/// view's rotation.
///
/// **Rotation** (cinematic effects, kb: decisions/controls "View rotation"): a view point at
/// offset (dx, dy) from the centre (y up) is the complex point
/// `center + scale · e^(i·angle) · (dx + i·dy)`, so a positive angle turns the camera
/// counter-clockwise and the image clockwise. Every pixel-to-c mapping goes through
/// [`Viewport::rotate`] (and the GPU kernel's `rot`); angle 0 keeps the unrotated arithmetic
/// bit for bit.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewport {
    pub center: Center,
    /// Complex units per view point.
    pub scale: Scale,
    /// Radians in (-π, π]; 0 for an unrotated view.
    pub angle: f64,
}

/// `a` wrapped into (-π, π] (0 stays exactly 0; non-finite gives 0).
pub fn wrap_angle(a: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    if !a.is_finite() {
        return 0.0;
    }
    if a > -PI && a <= PI {
        return a;
    }
    let w = a.rem_euclid(TAU);
    if w > PI { w - TAU } else { w }
}

/// Scale of the home view.
pub const HOME_SCALE: f64 = 3.0 / 800.0;
/// Largest scale (most zoomed out).
pub const MAX_SCALE: f64 = 0.05;
/// Deepest scale, log2 (about 1e-3010 per point): reference orbits there take 10 000+ bits;
/// a practical bound, not a numerical one (kb: synthesis/benchmarks M3b).
pub const MIN_SCALE_LOG2: i64 = -10_000;
/// Guard bits of the centre beyond the point spacing (view sizes up to 2^16 points, and
/// sub-point offsets kept exactly over long pans).
pub const CENTER_GUARD_BITS: i64 = 96;

/// Centre bits for `scale`, whole limbs. Capped a little past the deepest scale: a pasted
/// "1e-999999999" asked for billions of bits before the caller clamped the scale, and the
/// precision raise hung the UI (kb: techniques/hang-prevention).
fn center_bits_for(scale: Scale) -> usize {
    let bits = ((-scale.log2()).ceil() as i64).saturating_add(CENTER_GUARD_BITS);
    (bits.clamp(MIN_CENTER_BITS as i64, MAX_CENTER_BITS as i64) as usize).div_ceil(64) * 64
}

impl Viewport {
    pub fn new(re: f64, im: f64, scale: f64) -> Self {
        let mut v = Self {
            center: Center::new(re, im),
            scale: Scale::from_f64(scale),
            angle: 0.0,
        };
        v.ensure_precision();
        v
    }

    pub fn with_center(center: Center, scale: Scale) -> Self {
        let mut v = Self {
            center,
            scale,
            angle: 0.0,
        };
        v.ensure_precision();
        v
    }

    /// The same view turned to `angle` radians (wrapped).
    pub fn with_angle(mut self, angle: f64) -> Self {
        self.angle = wrap_angle(angle);
        self
    }

    /// `(cos, sin)` of the angle; exactly (1, 0) unrotated.
    pub fn rotation(&self) -> (f64, f64) {
        if self.angle == 0.0 {
            (1.0, 0.0)
        } else {
            let (s, c) = self.angle.sin_cos();
            (c, s)
        }
    }

    /// A view-point offset (screen axes, y up) as a complex offset in units of the scale: the
    /// offset turned by the angle. Exact identity when unrotated.
    pub fn rotate(&self, dx: f64, dy: f64) -> (f64, f64) {
        if self.angle == 0.0 {
            return (dx, dy);
        }
        let (c, s) = self.rotation();
        (dx * c - dy * s, dx * s + dy * c)
    }

    /// The inverse of [`Viewport::rotate`]: a complex offset (units of the scale) in screen axes.
    pub fn unrotate(&self, x: f64, y: f64) -> (f64, f64) {
        if self.angle == 0.0 {
            return (x, y);
        }
        let (c, s) = self.rotation();
        (x * c + y * s, -x * s + y * c)
    }

    /// [`Viewport::unrotate`] of a pair.
    pub fn unrotate_vec(&self, v: (f64, f64)) -> (f64, f64) {
        self.unrotate(v.0, v.1)
    }

    /// Where `c` is on screen: view points from the centre, y up (at any depth).
    pub fn offset_of(&self, c: &Center) -> (f64, f64) {
        let (x, y) = c.delta_in(&self.center, self.scale);
        self.unrotate(x, y)
    }

    /// Turns the view by `delta` radians about the view point (x, y), which stays fixed.
    pub fn rotate_by(&mut self, delta: f64, x: f64, y: f64) {
        if !delta.is_finite() || delta == 0.0 {
            return;
        }
        let p = self.point_at_offset(x, y);
        self.angle = wrap_angle(self.angle + delta);
        let (rx, ry) = self.rotate(x, y);
        self.center = p.offset_scaled(-rx, -ry, self.scale);
    }

    pub fn home() -> Self {
        Self::new(-0.75, 0.0, HOME_SCALE)
    }

    /// Centre precision needed for this scale (bits, whole limbs).
    pub fn needed_center_bits(&self) -> usize {
        center_bits_for(self.scale)
    }

    /// Raises the centre's precision to what the scale needs (never lowers it).
    pub fn ensure_precision(&mut self) {
        let bits = self.needed_center_bits();
        if self.center.precision_bits() < bits {
            self.center = self.center.with_min_precision(bits);
        }
    }

    /// Magnification relative to the home view (inf beyond f64: use `zoom_log10`).
    pub fn zoom(&self) -> f64 {
        Scale::from_f64(HOME_SCALE).ratio(self.scale)
    }

    /// log10 of the magnification.
    pub fn zoom_log10(&self) -> f64 {
        HOME_SCALE.log10() - self.scale.log10()
    }

    /// The deepest scale zooming reaches.
    pub fn min_scale(&self) -> Scale {
        Scale::pow2(MIN_SCALE_LOG2)
    }

    /// True when zooming in further is blocked by `min_scale`.
    pub fn at_precision_limit(&self) -> bool {
        self.scale <= self.min_scale() * 1.001
    }

    /// The whole set fitted into a view of `width` x `height` points.
    pub fn fitting(width: f64, height: f64) -> Self {
        if width <= 0.0 || height <= 0.0 {
            return Self::home();
        }
        Self::new(-0.75, 0.0, (3.2 / width).max(2.6 / height))
    }

    /// The home view of a formula's whole set (F3): [`Viewport::fitting`] for Mandelbrot; a
    /// Multibrot set is centred on 0 and spans about 2.9 in both directions; a Julia set (F4) gets
    /// [`crate::julia::home_view`]; the Mandelbar set (F5) is centred on -1/2.
    pub fn fitting_for(formula: crate::formula::Formula, width: f64, height: f64) -> Self {
        if formula.is_julia() {
            return crate::julia::home_view(width, height);
        }
        if formula.is_mandelbrot() || width <= 0.0 || height <= 0.0 {
            return Self::fitting(width, height);
        }
        if formula == crate::formula::Formula::Mandelbar {
            // F5: the Tricorn spans re -2 .. 1 (its antenna tips -2 and 1 ± i sqrt 3) and
            // im ±sqrt 3.
            return Self::new(-0.5, 0.0, (3.4 / width).max(3.75 / height));
        }
        if formula == crate::formula::Formula::BurningShip {
            // F6: the ship spans re -2 .. 1.15 and im -0.45 .. 1.75 (ours: the imaginary axis
            // flipped against the literature, the ships upright; kb: formulas/burning-ship).
            return Self::new(-0.45, 0.62, (3.6 / width).max(2.6 / height));
        }
        Self::new(0.0, 0.0, (2.9 / width).max(2.9 / height))
    }

    /// Complex coordinate of a point given in view points from the centre (y up).
    pub fn point_at_offset(&self, dx: f64, dy: f64) -> Center {
        let (x, y) = self.rotate(dx, dy);
        self.center.offset_scaled(x, y, self.scale)
    }

    /// Zooms by `factor` (> 1 zooms in) keeping the point at offset (dx, dy) fixed.
    pub fn zoom_by(&mut self, factor: f64, dx: f64, dy: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let p = self.point_at_offset(dx, dy);
        self.scale = (self.scale / factor)
            .max(self.min_scale())
            .min(Scale::from_f64(MAX_SCALE));
        let bits = center_bits_for(self.scale);
        let (rx, ry) = self.rotate(dx, dy);
        self.center = p.with_min_precision(bits).offset_scaled(-rx, -ry, self.scale);
    }

    /// Moves the content by (dx, dy) view points (the camera moves the other way).
    pub fn pan_by_points(&mut self, dx: f64, dy: f64) {
        let (x, y) = self.rotate(dx, dy);
        self.center = self.center.offset_scaled(-x, -y, self.scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_point_fixed() {
        let mut v = Viewport::new(-0.75, 0.1, 1e-3);
        let before = v.point_at_offset(120.0, -60.0);
        v.zoom_by(1.5, 120.0, -60.0);
        let after = v.point_at_offset(120.0, -60.0);
        let (dr, di) = after.delta_from(&before);
        assert!(dr.abs() < 1e-15 && di.abs() < 1e-15);
        assert!((v.scale.to_f64() - 1e-3 / 1.5).abs() < 1e-18);
    }

    #[test]
    fn zoom_clamps_at_limits() {
        let mut v = Viewport::new(-0.75, 0.0, 1e-15);
        v.zoom_by(1e-30, 0.0, 0.0);
        assert_eq!(v.scale.to_f64(), MAX_SCALE);
        let mut d = Viewport::with_center(Center::new(-0.75, 0.0), Scale::pow2(MIN_SCALE_LOG2 + 3));
        d.zoom_by(100.0, 0.0, 0.0);
        assert!(d.at_precision_limit());
    }

    #[test]
    fn scale_arithmetic_is_f64_exact_in_range() {
        let s = 3.0 / 800.0;
        let sc = Scale::from_f64(s);
        assert_eq!(sc.to_f64(), s);
        assert_eq!((sc / 1.15).to_f64(), s / 1.15);
        assert_eq!((sc * 0.37).to_f64(), s * 0.37);
        assert_eq!(Scale::pow2(-2000).log2(), -2000.0);
        assert_eq!(Scale::pow2(-2000).to_f64(), 0.0);
        assert!((Scale::pow2(-2000).ratio(Scale::pow2(-2003)) - 8.0).abs() < 1e-15);
        let p = Scale::parse("2.5e-1000").unwrap();
        assert!((p.log10() + 999.60206).abs() < 1e-4, "{}", p.log10());
        assert_eq!(p.to_text(3), "2.5e-1000");
        assert_eq!(Scale::from_f64(1.5e-5).to_text(3), "1.5e-05");
    }

    #[test]
    fn deep_zoom_keeps_points_exact() {
        // Zoom from 1e-3 to 1e-600 around an off-centre point: the point under the cursor stays
        // within a fraction of a point the whole way, which f64 loses at 1e-16.
        let mut v = Viewport::new(-0.743643887037151, 0.13182590420533, 1e-3);
        let target = v.point_at_offset(37.25, -11.5);
        for _ in 0..1380 {
            v.zoom_by(std::f64::consts::E, 37.25, -11.5);
        }
        assert!(v.scale.log10() < -598.0);
        let here = v.point_at_offset(37.25, -11.5);
        let (dx, dy) = here.delta_in(&target, v.scale);
        assert!(dx.abs() < 1e-3 && dy.abs() < 1e-3, "drift {dx} {dy} points");
        assert!(v.center.precision_bits() >= 2000);
        // Panning one point moves the centre by exactly one point.
        let before = v.center.clone();
        v.pan_by_points(1.0, 0.0);
        let (px, py) = v.center.delta_in(&before, v.scale);
        assert!((px + 1.0).abs() < 1e-12 && py.abs() < 1e-12);
    }

    #[test]
    fn rotation_round_trips_at_any_depth() {
        for &(scale, angle) in &[(1e-3, 0.7), (1e-300, -2.9), (0.0, 1.0e-3)] {
            let mut v = if scale > 0.0 {
                Viewport::new(-0.743, 0.131, scale)
            } else {
                Viewport::with_center(Center::new(-1.25, 0.1), Scale::pow2(-3330))
            };
            v.angle = angle;
            // Screen -> plane -> screen.
            for &(x, y) in &[(0.0, 0.0), (120.5, -33.25), (-640.0, 400.0)] {
                let c = v.point_at_offset(x, y);
                let (bx, by) = v.offset_of(&c);
                assert!((bx - x).abs() < 1e-9 && (by - y).abs() < 1e-9, "{x},{y} -> {bx},{by}");
            }
            // rotate / unrotate are inverse; lengths are kept.
            let (rx, ry) = v.rotate(3.0, 4.0);
            assert!((rx.hypot(ry) - 5.0).abs() < 1e-12);
            let (ux, uy) = v.unrotate(rx, ry);
            assert!((ux - 3.0).abs() < 1e-12 && (uy - 4.0).abs() < 1e-12);
            // A zoom keeps the point under the target fixed on screen.
            let p = v.point_at_offset(50.0, -20.0);
            v.zoom_by(3.7, 50.0, -20.0);
            let (sx, sy) = v.offset_of(&p);
            assert!((sx - 50.0).abs() < 1e-6 && (sy + 20.0).abs() < 1e-6, "{sx} {sy}");
            // A pan moves the content by exactly the screen offset.
            let q = v.point_at_offset(10.0, 10.0);
            v.pan_by_points(7.0, -3.0);
            let (qx, qy) = v.offset_of(&q);
            assert!((qx - 17.0).abs() < 1e-6 && (qy - 7.0).abs() < 1e-6, "{qx} {qy}");
            // Turning about a point keeps that point.
            let r = v.point_at_offset(-80.0, 60.0);
            v.rotate_by(1.3, -80.0, 60.0);
            let (rx2, ry2) = v.offset_of(&r);
            assert!((rx2 + 80.0).abs() < 1e-6 && (ry2 - 60.0).abs() < 1e-6, "{rx2} {ry2}");
        }
        // Unrotated mapping is exactly the plain one.
        let v = Viewport::new(-0.75, 0.1, 1e-3);
        assert_eq!(v.point_at_offset(1.5, -2.5), v.center.offset_scaled(1.5, -2.5, v.scale));
        assert_eq!(wrap_angle(4.0), 4.0 - std::f64::consts::TAU);
        assert_eq!(wrap_angle(std::f64::consts::PI), std::f64::consts::PI);
        assert_eq!(wrap_angle(0.0), 0.0);
    }

    #[test]
    fn coordinate_text_at_1e1000_is_cheap() {
        // The HUD formats the centre at ~10 Hz with the digits the depth needs.
        let mut v = Viewport::with_center(Center::new(-1.25, 0.1), Scale::pow2(-3330));
        v.pan_by_points(0.3, 0.7);
        let t = std::time::Instant::now();
        let (re, im) = v.center.to_strings_digits(1010);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("1010-digit centre text: {ms:.2} ms");
        assert!(re.len() > 1000 && im.len() > 1000);
        let back = Center::parse(&re, &im).unwrap();
        let (dx, dy) = back.delta_in(&v.center, v.scale);
        assert!(dx.abs() < 1e-3 && dy.abs() < 1e-3);
        assert!(ms < 50.0, "{ms} ms");
    }
}
