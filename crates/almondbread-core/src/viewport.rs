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

    /// Exact BigFloat of `x * self` (x finite).
    pub fn big_times(self, x: f64, precision_bits: usize) -> BigFloat {
        BigFloat::from_f64(x * self.mant, precision_bits.max(64)).times_power_of_two(self.exp)
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
    /// precision for every digit given.
    pub fn parse(re: &str, im: &str) -> Option<Self> {
        let digits = |s: &str| s.bytes().filter(u8::is_ascii_digit).count();
        let bits = ((digits(re).max(digits(im)) as f64 * 3.33) as usize + 16).max(MIN_CENTER_BITS);
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

/// Centre and size of one view point (not one pixel: independent of the display scale).
#[derive(Clone, Debug, PartialEq)]
pub struct Viewport {
    pub center: Center,
    /// Complex units per view point.
    pub scale: Scale,
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

impl Viewport {
    pub fn new(re: f64, im: f64, scale: f64) -> Self {
        let mut v = Self {
            center: Center::new(re, im),
            scale: Scale::from_f64(scale),
        };
        v.ensure_precision();
        v
    }

    pub fn with_center(center: Center, scale: Scale) -> Self {
        let mut v = Self { center, scale };
        v.ensure_precision();
        v
    }

    pub fn home() -> Self {
        Self::new(-0.75, 0.0, HOME_SCALE)
    }

    /// Centre precision needed for this scale (bits, whole limbs).
    pub fn needed_center_bits(&self) -> usize {
        let bits = (-self.scale.log2()).ceil() as i64 + CENTER_GUARD_BITS;
        (bits.max(MIN_CENTER_BITS as i64) as usize).div_ceil(64) * 64
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

    /// Complex coordinate of a point given in view points from the centre (y up).
    pub fn point_at_offset(&self, dx: f64, dy: f64) -> Center {
        self.center.offset_scaled(dx, dy, self.scale)
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
        let bits = (-self.scale.log2()).ceil() as i64 + CENTER_GUARD_BITS;
        let bits = (bits.max(MIN_CENTER_BITS as i64) as usize).div_ceil(64) * 64;
        self.center = p.with_min_precision(bits).offset_scaled(-dx, -dy, self.scale);
    }

    /// Moves the content by (dx, dy) view points (the camera moves the other way).
    pub fn pan_by_points(&mut self, dx: f64, dy: f64) {
        self.center = self.center.offset_scaled(-dx, -dy, self.scale);
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
}
