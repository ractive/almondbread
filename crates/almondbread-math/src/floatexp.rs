//! Extended-range types for the GPU and for low-precision bookkeeping
//! (kb: techniques/extended-range-floats).
//!
//! Convention: value = mantissa * 2^exponent with 0.5 <= |mantissa| < 1, and zero as
//! mantissa 0, exponent 0. GPU code must test the mantissa for zero rather than compare
//! exponents.

use crate::raw::pow2;
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// f32 mantissa + i32 exponent (8 bytes; MSL `struct { float m; int e; }`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FloatExp {
    pub mantissa: f32,
    pub exponent: i32,
}

impl FloatExp {
    pub const ZERO: FloatExp = FloatExp {
        mantissa: 0.0,
        exponent: 0,
    };

    /// Nearest f64 of the value (0 or inf outside f64's range).
    pub fn to_f64(self) -> f64 {
        self.mantissa as f64 * pow2(self.exponent as i64)
    }
}

/// A complex value with one shared exponent: (m[0] + i m[1]) * 2^exponent, where the larger
/// component's mantissa magnitude is in [0.5, 1). The smaller component is flushed to zero
/// when it is more than ~2^-149 below the larger, far below f32 precision relative to it.
/// Layout identical to MSL `struct { float2 m; int e; }`: 16-byte stride, 8-byte alignment.
/// This is the extended-range reference-orbit form (kb: techniques/extended-range-floats).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FloatExpComplex {
    pub mantissa: [f32; 2],
    pub exponent: i32,
}

const _: () = assert!(core::mem::size_of::<FloatExpComplex>() == 16);
const _: () = assert!(core::mem::align_of::<FloatExpComplex>() == 8);
const _: () = assert!(core::mem::size_of::<FloatExp>() == 8);

impl FloatExpComplex {
    pub const ZERO: FloatExpComplex = FloatExpComplex {
        mantissa: [0.0, 0.0],
        exponent: 0,
    };

    /// Nearest f64 pair (flushes to 0 outside f64's range).
    pub fn to_f64(self) -> [f64; 2] {
        let s = pow2(self.exponent as i64);
        [self.mantissa[0] as f64 * s, self.mantissa[1] as f64 * s]
    }
}

/// A complex value whose components carry their own exponents (F6, kb: plans/f6-burning-ship
/// "As built"): `(m[0] 2^e[0], m[1] 2^e[1])`, each mantissa magnitude in [0.5, 1) or 0 (with
/// exponent 0). Unlike [`FloatExpComplex`] the smaller component is never flushed: the Burning
/// Ship's fold needs the sign and size of `X Y` when one part is far below the other (a reference
/// along the needle, `Y ~ 1e-100`). Same 16 bytes as [`FloatExpComplex`]; MSL reads it from the
/// same buffer as `{float2 m; int e; int e2}` (the second exponent in the padding word).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FloatExpSplit {
    pub mantissa: [f32; 2],
    pub exponent: [i32; 2],
}

const _: () = assert!(core::mem::size_of::<FloatExpSplit>() == 16);

impl FloatExpSplit {
    pub const ZERO: FloatExpSplit = FloatExpSplit {
        mantissa: [0.0, 0.0],
        exponent: [0, 0],
    };

    /// From extended-range components (each rounded to f32 once).
    pub fn from_double_exp(z: DoubleExpComplex) -> Self {
        let part = |d: DoubleExp| -> (f32, i32) {
            if d.is_zero() {
                return (0.0, 0);
            }
            let mut m = d.mantissa as f32;
            let mut e = d.exponent;
            if m.abs() >= 1.0 {
                m *= 0.5;
                e += 1;
            }
            (m, e.clamp(i32::MIN as i64 / 2, i32::MAX as i64 / 2) as i32)
        };
        let (a, ea) = part(z.re);
        let (b, eb) = part(z.im);
        FloatExpSplit {
            mantissa: [a, b],
            exponent: [ea, eb],
        }
    }

    pub fn to_double_exp(self) -> DoubleExpComplex {
        let part = |m: f32, e: i32| DoubleExp::from_f64(f64::from(m)).times_power_of_two(i64::from(e));
        DoubleExpComplex::new(
            part(self.mantissa[0], self.exponent[0]),
            part(self.mantissa[1], self.exponent[1]),
        )
    }
}

/// |d| = significand * 2^exponent with significand in [1, 2) (also for subnormals).
/// `d` must be finite and non-zero.
pub(crate) fn exponent_significand(d: f64) -> (i64, f64) {
    if d == 0.0 {
        return (0, 0.0); // the subnormal branch below would recurse forever
    }
    let bits = d.abs().to_bits();
    let be = (bits >> 52) as i64;
    if be == 0 {
        let (e, s) = exponent_significand(d.abs() * 18446744073709551616.0);
        return (e - 64, s);
    }
    (be - 1023, f64::from_bits((bits & ((1u64 << 52) - 1)) | (1023u64 << 52)))
}

/// log2 for x in [0.5, 1] by the series x = (1 + t) / (1 - t), ln x = 2 (t + t^3/3 + ...).
/// The same evaluation as the Swift library, so decisions based on it agree bit for bit.
pub(crate) fn log2_unit(x: f64) -> f64 {
    let t = (x - 1.0) / (x + 1.0);
    let t2 = t * t;
    let mut term = t;
    let mut sum = 0.0;
    // Odd k = 1, 3, ..., 59 as an integer loop (bounded by construction; same divisors).
    for k in (1..60u32).step_by(2) {
        sum += term / f64::from(k);
        term *= t2;
        if term.abs() < 1e-20 {
            break;
        }
    }
    2.0 * sum / core::f64::consts::LN_2
}

/// f64 mantissa + i64 exponent with the operations the period detector and size estimates
/// need (radii of 1e-100000, derivatives of 1e+100000).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DoubleExp {
    pub mantissa: f64,
    pub exponent: i64,
}

impl DoubleExp {
    pub const ZERO: DoubleExp = DoubleExp {
        mantissa: 0.0,
        exponent: 0,
    };
    pub const ONE: DoubleExp = DoubleExp {
        mantissa: 0.5,
        exponent: 1,
    };

    pub const fn new(mantissa: f64, exponent: i64) -> Self {
        DoubleExp { mantissa, exponent }
    }

    /// From an f64, normalised. A non-finite value is carried as it is (mantissa ±inf or NaN,
    /// exponent 0), the form an overflow in `*`, `/` or `times_power_of_two` already gives, so
    /// it propagates like f64 and callers' finite checks see it. This used to panic: in the app
    /// a panic here only ever dropped the frame and fired again on the next one (2026-10-26:
    /// the orbit probe of a point 10^336 view widths away, every refresh; kb:
    /// techniques/hang-prevention).
    pub fn from_f64(d: f64) -> Self {
        if !d.is_finite() {
            return DoubleExp {
                mantissa: d,
                exponent: 0,
            };
        }
        if d == 0.0 {
            return Self::ZERO;
        }
        let (e, s) = exponent_significand(d);
        let m = s * 0.5;
        DoubleExp {
            mantissa: if d < 0.0 { -m } else { m },
            exponent: e + 1,
        }
    }

    #[inline]
    pub(crate) fn normalised(m: f64, e: i64) -> Self {
        Self::normalised_wide(m, e as i128)
    }

    /// `m * 2^e` normalised, the exponent computed without overflow: past i64's range the
    /// value saturates to +-inf (mantissa, as an f64 overflow would) or to zero. An orbit
    /// that escapes far (Newton from a start outside the set, bailout f64::MAX) squares its
    /// exponent each iteration; i64 exponents used to wrap there (a debug-build panic).
    #[inline]
    fn normalised_wide(m: f64, e: i128) -> Self {
        if m == 0.0 {
            return Self::ZERO;
        }
        if !m.is_finite() {
            return DoubleExp {
                mantissa: m,
                exponent: e.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            };
        }
        let (k, s) = exponent_significand(m);
        let mm = s * 0.5;
        let e = e + k as i128 + 1;
        if e > i64::MAX as i128 {
            return DoubleExp {
                mantissa: m * f64::INFINITY,
                exponent: 0,
            };
        }
        if e < i64::MIN as i128 {
            return Self::ZERO;
        }
        DoubleExp {
            mantissa: if m < 0.0 { -mm } else { mm },
            exponent: e as i64,
        }
    }

    pub fn is_zero(self) -> bool {
        self.mantissa == 0.0
    }

    /// The value as f64 (0 or inf outside f64's range).
    pub fn to_f64(self) -> f64 {
        if self.exponent > 1100 {
            return if self.mantissa < 0.0 {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        if self.exponent < -1100 {
            return 0.0;
        }
        if self.exponent >= -1000 {
            self.mantissa * pow2(self.exponent)
        } else {
            self.mantissa * pow2(self.exponent + 200) * pow2(-200)
        }
    }

    /// log2 |value| (-inf for zero).
    pub fn log2_magnitude(self) -> f64 {
        if self.is_zero() {
            return f64::NEG_INFINITY;
        }
        self.exponent as f64 + log2_unit(self.mantissa.abs())
    }

    pub fn abs(self) -> Self {
        DoubleExp {
            mantissa: self.mantissa.abs(),
            exponent: self.exponent,
        }
    }

    /// Panics on a negative value.
    pub fn sqrt(self) -> Self {
        assert!(self.mantissa >= 0.0, "square root of a negative DoubleExp");
        if self.is_zero() {
            return Self::ZERO;
        }
        if self.exponent & 1 == 0 {
            Self::normalised(self.mantissa.sqrt(), self.exponent / 2)
        } else {
            Self::normalised((self.mantissa * 2.0).sqrt(), (self.exponent - 1) / 2)
        }
    }

    /// Multiplies by 2^k (saturating to inf or zero past i64's exponent range).
    pub fn times_power_of_two(self, k: i64) -> Self {
        if self.is_zero() {
            self
        } else if let Some(exponent) = self.exponent.checked_add(k) {
            DoubleExp {
                mantissa: self.mantissa,
                exponent,
            }
        } else {
            Self::normalised_wide(self.mantissa, self.exponent as i128 + k as i128)
        }
    }

    fn sign(self) -> i32 {
        if self.mantissa < 0.0 {
            -1
        } else if self.mantissa > 0.0 {
            1
        } else {
            0
        }
    }
}

impl Mul for DoubleExp {
    type Output = DoubleExp;
    fn mul(self, b: DoubleExp) -> DoubleExp {
        DoubleExp::normalised_wide(self.mantissa * b.mantissa, self.exponent as i128 + b.exponent as i128)
    }
}

impl Div for DoubleExp {
    type Output = DoubleExp;
    fn div(self, b: DoubleExp) -> DoubleExp {
        assert!(!b.is_zero(), "DoubleExp division by zero");
        DoubleExp::normalised_wide(self.mantissa / b.mantissa, self.exponent as i128 - b.exponent as i128)
    }
}

impl Add for DoubleExp {
    type Output = DoubleExp;
    fn add(self, b: DoubleExp) -> DoubleExp {
        if self.is_zero() {
            return b;
        }
        if b.is_zero() {
            return self;
        }
        let (x, y) = if self.exponent >= b.exponent {
            (self, b)
        } else {
            (b, self)
        };
        let d = x.exponent.saturating_sub(y.exponent);
        if d > 60 {
            return x;
        }
        DoubleExp::normalised(x.mantissa + y.mantissa * pow2(-d), x.exponent)
    }
}

impl Neg for DoubleExp {
    type Output = DoubleExp;
    fn neg(self) -> DoubleExp {
        DoubleExp {
            mantissa: -self.mantissa,
            exponent: self.exponent,
        }
    }
}

impl Sub for DoubleExp {
    type Output = DoubleExp;
    fn sub(self, b: DoubleExp) -> DoubleExp {
        self + (-b)
    }
}

impl PartialOrd for DoubleExp {
    fn partial_cmp(&self, b: &DoubleExp) -> Option<Ordering> {
        let (sa, sb) = (self.sign(), b.sign());
        if sa != sb {
            return Some(sa.cmp(&sb));
        }
        if sa == 0 {
            return Some(Ordering::Equal);
        }
        if self.exponent != b.exponent {
            let c = self.exponent.cmp(&b.exponent);
            return Some(if sa > 0 { c } else { c.reverse() });
        }
        self.mantissa.partial_cmp(&b.mantissa)
    }
}

impl fmt::Display for DoubleExp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        use core::f64::consts::LOG10_2;
        let l10 = self.log2_magnitude() * LOG10_2;
        let e10 = l10.floor();
        let m = 2f64.powf((l10 - e10) / LOG10_2);
        let sign = if self.mantissa < 0.0 { "-" } else { "" };
        write!(f, "{sign}{m}e{}", e10 as i64)
    }
}

/// Complex DoubleExp with separate exponents (bookkeeping only, not a GPU type).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DoubleExpComplex {
    pub re: DoubleExp,
    pub im: DoubleExp,
}

impl DoubleExpComplex {
    pub const ZERO: DoubleExpComplex = DoubleExpComplex {
        re: DoubleExp::ZERO,
        im: DoubleExp::ZERO,
    };
    pub const ONE: DoubleExpComplex = DoubleExpComplex {
        re: DoubleExp::ONE,
        im: DoubleExp::ZERO,
    };

    pub fn new(re: DoubleExp, im: DoubleExp) -> Self {
        DoubleExpComplex { re, im }
    }

    pub fn magnitude_squared(self) -> DoubleExp {
        self.re * self.re + self.im * self.im
    }

    pub fn magnitude(self) -> DoubleExp {
        self.magnitude_squared().sqrt()
    }

    pub fn times_two(self) -> Self {
        DoubleExpComplex {
            re: self.re.times_power_of_two(1),
            im: self.im.times_power_of_two(1),
        }
    }

    /// The complex conjugate (F5: the non-conformal formulas, kb: formulas/mandelbar).
    pub fn conj(self) -> Self {
        DoubleExpComplex {
            re: self.re,
            im: -self.im,
        }
    }

    pub fn is_zero(self) -> bool {
        self.re.is_zero() && self.im.is_zero()
    }
}

impl Sub for DoubleExpComplex {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        DoubleExpComplex {
            re: self.re - b.re,
            im: self.im - b.im,
        }
    }
}

impl Add for DoubleExpComplex {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        DoubleExpComplex {
            re: self.re + b.re,
            im: self.im + b.im,
        }
    }
}

impl Mul for DoubleExpComplex {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        DoubleExpComplex {
            re: self.re * b.re - self.im * b.im,
            im: self.re * b.im + self.im * b.re,
        }
    }
}

impl Div for DoubleExpComplex {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        let den = b.re * b.re + b.im * b.im;
        DoubleExpComplex {
            re: (self.re * b.re + self.im * b.im) / den,
            im: (self.im * b.re - self.re * b.im) / den,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_exp_basics() {
        assert_eq!(DoubleExp::from_f64(-6.0), DoubleExp::new(-0.75, 3));
        assert_eq!(DoubleExp::ONE, DoubleExp::from_f64(1.0));
        let tiny = DoubleExp::from_f64(5e-324);
        assert_eq!(tiny, DoubleExp::new(0.5, -1073));
        let a = DoubleExp::new(0.5, -100_000);
        assert!((a * a).log2_magnitude() == -200_002.0);
        assert!(DoubleExp::from_f64(2.0) > DoubleExp::ONE);
        assert!(DoubleExp::from_f64(-2.0) < DoubleExp::from_f64(-1.0));
        assert_eq!(DoubleExp::from_f64(16.0).sqrt().to_f64(), 4.0);
        assert_eq!(DoubleExp::from_f64(8.0).sqrt().to_f64(), 8f64.sqrt());
    }

    /// Past i64's exponent range values saturate to inf or zero instead of wrapping (a
    /// debug-build overflow panic in Newton's derivative of a far-escaped orbit).
    #[test]
    fn double_exp_exponent_saturates() {
        let big = DoubleExp::new(0.75, i64::MAX - 1);
        let small = DoubleExp::new(-0.75, i64::MIN + 1);
        assert_eq!((big * big).mantissa, f64::INFINITY);
        assert_eq!((big * -big).mantissa, f64::NEG_INFINITY);
        assert!((small * small).is_zero());
        assert!((big / small).mantissa == f64::NEG_INFINITY);
        assert!((small / big).is_zero());
        assert_eq!(big.times_power_of_two(10).mantissa, f64::INFINITY);
        assert!(small.times_power_of_two(-10).is_zero());
        assert_eq!(big + small, big);
        assert_eq!(small + big, big);
        // In range: unchanged.
        assert_eq!(big.times_power_of_two(-5), DoubleExp::new(0.75, i64::MAX - 6));
        assert_eq!(
            DoubleExp::new(0.5, 1 << 62) * DoubleExp::new(0.5, 1 << 61),
            DoubleExp::new(0.5, (1 << 62) + (1 << 61) - 1)
        );
    }
}
