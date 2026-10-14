//! BigFloat: arbitrary-precision binary floating point (kb: decisions/arbitrary-precision).
//!
//! A sign, an i64 exponent and an n-limb u64 mantissa, value
//!     (-1)^negative * mantissa / 2^(64n) * 2^exponent,  top mantissa bit set,
//! so |value| is in [2^(exponent-1), 2^exponent). Precision is fixed per value at creation,
//! in whole limbs. Every operation rounds to nearest, ties to even (correctly rounded)
//! except [`BigFloat::sqrt`] (within 1 ulp). The exponent is a full i64: derivatives of size
//! 1e100000 and radii of 1e-100000 are fine, which fixed point could not do.
//!
//! The operators allocate their result; the hot loops (reference orbit, period detection,
//! Newton) run on preallocated [`Raw`] registers instead. The result of a binary operation
//! has the larger of the two precisions.

use crate::bignat;
use crate::floatexp::{DoubleExp, FloatExp, log2_unit};
use crate::limbs;
use crate::raw::{self, Raw, View};
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct BigFloat {
    pub(crate) limbs: Vec<u64>,
    pub(crate) negative: bool,
    pub(crate) exponent: i64,
}

/// Scratch buffer for one operation: on the stack up to a size, else on the heap.
pub(crate) fn with_scratch<R>(n: usize, body: impl FnOnce(&mut [u64]) -> R) -> R {
    let len = raw::raw_scratch_limbs(n);
    if len <= 256 {
        let mut s = [0u64; 256];
        body(&mut s[..len])
    } else {
        let mut s = vec![0u64; len];
        body(&mut s)
    }
}

impl BigFloat {
    /// Number of limbs for a precision in bits (at least one).
    pub const fn limb_count_for_bits(bits: usize) -> usize {
        let n = bits.div_ceil(64);
        if n == 0 { 1 } else { n }
    }

    /// Zero at the given precision.
    pub fn zero(precision_bits: usize) -> Self {
        Self::zero_limbs(Self::limb_count_for_bits(precision_bits))
    }

    pub(crate) fn zero_limbs(n: usize) -> Self {
        BigFloat {
            limbs: vec![0; n],
            negative: false,
            exponent: 0,
        }
    }

    /// Exact conversion from f64 (any precision holds all 53 bits). Panics if not finite.
    pub fn from_f64(value: f64, precision_bits: usize) -> Self {
        let mut r = Self::zero(precision_bits);
        r.with_raw_mut(|raw| raw::set_f64(raw, value));
        r
    }

    /// Conversion from i64 (exact from 64 bits of precision).
    pub fn from_i64(value: i64, precision_bits: usize) -> Self {
        Self::from_parts(value < 0, &[value.unsigned_abs()], 0, precision_bits)
    }

    /// value = (-1)^negative * S * 2^exponent, where S is the unsigned integer with the
    /// given little-endian limbs; rounded to nearest-even at `precision_bits`.
    pub fn from_parts(negative: bool, significand: &[u64], exponent: i64, precision_bits: usize) -> Self {
        let n = Self::limb_count_for_bits(precision_bits);
        let mut r = Self::zero_limbs(n);
        let s = &significand[..significand.iter().rposition(|&x| x != 0).map_or(0, |i| i + 1)];
        if s.is_empty() {
            return r;
        }
        let bl = raw::round_wide(s, false, &mut r.limbs);
        r.negative = negative;
        r.exponent = bl + exponent;
        r
    }

    pub fn limb_count(&self) -> usize {
        self.limbs.len()
    }

    pub fn precision_bits(&self) -> usize {
        self.limbs.len() * 64
    }

    pub fn is_zero(&self) -> bool {
        self.limbs[self.limbs.len() - 1] == 0
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }

    /// Binary exponent: |value| is in [2^(exponent-1), 2^exponent). 0 for zero.
    pub fn exponent(&self) -> i64 {
        self.exponent
    }

    /// The normalised mantissa limbs, least significant first.
    pub fn significand_limbs(&self) -> &[u64] {
        &self.limbs
    }

    /// The same value at another precision (exact when widening, rounded when narrowing).
    pub fn with_precision(&self, bits: usize) -> BigFloat {
        self.with_limb_count(Self::limb_count_for_bits(bits)).into_owned()
    }

    pub(crate) fn with_limb_count(&self, n: usize) -> Cow<'_, BigFloat> {
        let m = self.limb_count();
        if n == m {
            return Cow::Borrowed(self);
        }
        let mut r = Self::zero_limbs(n);
        if self.is_zero() {
            return Cow::Owned(r);
        }
        r.negative = self.negative;
        if n > m {
            r.limbs[n - m..].copy_from_slice(&self.limbs);
            r.exponent = self.exponent;
        } else {
            let adj = raw::round_mantissa(&self.limbs, &mut r.limbs);
            r.exponent = self.exponent + adj;
        }
        Cow::Owned(r)
    }

    // ---- raw access

    pub(crate) fn view(&self) -> View<'_> {
        View {
            m: &self.limbs,
            neg: self.negative,
            exp: self.exponent,
            zero: self.is_zero(),
        }
    }

    pub(crate) fn with_raw_mut<R>(&mut self, body: impl FnOnce(&mut Raw<&mut [u64]>) -> R) -> R {
        let zero = self.is_zero();
        let mut raw = Raw {
            m: &mut self.limbs[..],
            neg: self.negative,
            exp: self.exponent,
            zero,
        };
        let res = body(&mut raw);
        let (neg, exp, zero) = (raw.neg, raw.exp, raw.zero);
        if zero {
            self.limbs.fill(0);
            self.negative = false;
            self.exponent = 0;
        } else {
            self.negative = neg;
            self.exponent = exp;
        }
        res
    }

    /// Copies a register out as a BigFloat.
    pub(crate) fn from_raw<S: AsRef<[u64]>>(r: &Raw<S>) -> BigFloat {
        let m = r.m.as_ref();
        if r.zero {
            return Self::zero_limbs(m.len());
        }
        BigFloat {
            limbs: m.to_vec(),
            negative: r.neg,
            exponent: r.exp,
        }
    }

    fn binary(
        a: &BigFloat,
        b: &BigFloat,
        op: impl FnOnce(&mut Raw<&mut [u64]>, View<'_>, View<'_>, &mut [u64]),
    ) -> BigFloat {
        let n = a.limb_count().max(b.limb_count());
        let pa = a.with_limb_count(n);
        let pb = b.with_limb_count(n);
        let mut r = Self::zero_limbs(n);
        with_scratch(n, |s| r.with_raw_mut(|rr| op(rr, pa.view(), pb.view(), s)));
        r
    }

    // ---- arithmetic

    pub fn add_ref(&self, b: &BigFloat) -> BigFloat {
        Self::binary(self, b, |r, x, y, s| raw::add(r, x, y, false, s))
    }

    pub fn sub_ref(&self, b: &BigFloat) -> BigFloat {
        Self::binary(self, b, |r, x, y, s| raw::add(r, x, y, true, s))
    }

    pub fn mul_ref(&self, b: &BigFloat) -> BigFloat {
        #[allow(clippy::redundant_closure)] // the closure is needed for the higher-ranked lifetime
        Self::binary(self, b, |r, x, y, s| raw::mul(r, x, y, s))
    }

    /// self * self (dedicated squaring, about half the limb products of a multiply).
    pub fn square(&self) -> BigFloat {
        let n = self.limb_count();
        let mut r = Self::zero_limbs(n);
        with_scratch(n, |s| r.with_raw_mut(|rr| raw::sqr(rr, self.view(), s)));
        r
    }

    pub fn negate(&mut self) {
        if !self.is_zero() {
            self.negative = !self.negative;
        }
    }

    pub fn abs(&self) -> BigFloat {
        let mut r = self.clone();
        r.negative = false;
        r
    }

    /// self * 2^k (exact).
    pub fn times_power_of_two(&self, k: i64) -> BigFloat {
        let mut r = self.clone();
        if !r.is_zero() {
            r.exponent += k;
        }
        r
    }

    /// self * m for a machine integer m (correctly rounded).
    pub fn mul_int(&self, m: i64) -> BigFloat {
        let n = self.limb_count();
        if self.is_zero() || m == 0 {
            return Self::zero_limbs(n);
        }
        let mut wide = vec![0u64; n + 1];
        wide[n] = limbs::mul_1(&mut wide, &self.limbs, n, m.unsigned_abs());
        let mut r = Self::zero_limbs(n);
        let bl = raw::round_wide(&wide, false, &mut r.limbs);
        r.negative = self.negative != (m < 0);
        r.exponent = self.exponent - 64 * n as i64 + bl;
        r
    }

    /// Correctly rounded quotient. Panics on division by zero.
    pub fn div_ref(&self, b: &BigFloat) -> BigFloat {
        assert!(!b.is_zero(), "BigFloat division by zero");
        let n = self.limb_count().max(b.limb_count());
        if self.is_zero() {
            return Self::zero_limbs(n);
        }
        let pa = self.with_limb_count(n);
        let pb = b.with_limb_count(n);
        // q = floor(ma * 2^(64(n+1)) / mb) has 64n+1 or 64n+2 bits; the remainder is sticky.
        let mut u = vec![0u64; n + 1];
        u.extend_from_slice(&pa.limbs);
        let (q, rem) = bignat::div_mod(&u, &pb.limbs);
        let mut r = Self::zero_limbs(n);
        let bl = raw::round_wide(&q, !rem.is_empty(), &mut r.limbs);
        r.negative = pa.negative != pb.negative;
        r.exponent = pa.exponent - pb.exponent - 64 * (n as i64 + 1) + bl;
        r
    }

    /// Square root (Newton, within 1 ulp). Panics on negative input.
    pub fn sqrt(&self) -> BigFloat {
        assert!(!self.negative || self.is_zero(), "square root of a negative BigFloat");
        if self.is_zero() {
            return self.clone();
        }
        let n = self.limb_count();
        let work = n + 1;
        let x = self.with_limb_count(work).into_owned();
        // Seed from f64 on an even exponent so the seed's exponent is exact.
        let e = self.exponent - (self.exponent & 1); // x = m * 2^e with m in [0.25, 1)
        let mant = raw::double_mantissa(self.view()) * if self.exponent & 1 == 0 { 1.0 } else { 2.0 };
        let mut y = BigFloat::from_f64(mant.sqrt(), work * 64).times_power_of_two(e / 2);
        // Each Newton step doubles the correct bits (~50 from the seed).
        let mut bits = 50;
        while bits < work * 64 + 8 {
            y = (&y + &(&x / &y)).times_power_of_two(-1);
            bits *= 2;
        }
        y = (&y + &(&x / &y)).times_power_of_two(-1);
        y.with_limb_count(n).into_owned()
    }

    // ---- conversions

    /// Nearest f64 (ties to even; may double-round in the subnormal range).
    pub fn to_f64(&self) -> f64 {
        raw::to_f64(self.view())
    }

    /// log2 |value| (-inf for zero), accurate to ~1e-15 relative.
    pub fn log2_magnitude(&self) -> f64 {
        if self.is_zero() {
            return f64::NEG_INFINITY;
        }
        self.exponent as f64 + log2_unit(raw::double_mantissa(self.view()))
    }

    /// f32 mantissa in [0.5, 1) and i32 exponent (exponent clamped to the i32 range).
    pub fn float_exp(&self) -> FloatExp {
        if self.is_zero() {
            return FloatExp::ZERO;
        }
        let mut mant = raw::jammed_top_limb(&self.limbs) as f32 * (raw::TWO_M64 as f32);
        let mut e = self.exponent;
        if mant == 1.0 {
            mant = 0.5;
            e += 1;
        }
        FloatExp {
            mantissa: if self.negative { -mant } else { mant },
            exponent: e.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        }
    }

    /// f64 mantissa in [0.5, 1) and i64 exponent.
    pub fn double_exp(&self) -> DoubleExp {
        raw_double_exp(self.view())
    }

    /// Exact value as (negative, integer significand S, binary exponent E): value = ±S·2^E.
    pub fn exact_parts(&self) -> (bool, &[u64], i64) {
        (
            self.negative,
            &self.limbs,
            self.exponent - 64 * self.limb_count() as i64,
        )
    }

    /// Compares by value (precision ignored).
    pub fn compare(&self, b: &BigFloat) -> Ordering {
        let n = self.limb_count().max(b.limb_count());
        let pa = self.with_limb_count(n);
        let pb = b.with_limb_count(n);
        raw::compare(pa.view(), pb.view())
    }
}

/// Extended-range value of a register.
#[inline(always)]
pub(crate) fn raw_double_exp(a: View<'_>) -> DoubleExp {
    if a.zero {
        return DoubleExp::ZERO;
    }
    let mut mant = raw::double_mantissa(a);
    let mut e = a.exp;
    if mant == 1.0 {
        mant = 0.5;
        e += 1;
    }
    DoubleExp::new(if a.neg { -mant } else { mant }, e)
}

impl PartialEq for BigFloat {
    /// Value equality (precision is ignored).
    fn eq(&self, other: &Self) -> bool {
        self.compare(other) == Ordering::Equal
    }
}

impl PartialOrd for BigFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.compare(other))
    }
}

impl fmt::Display for BigFloat {
    /// Round-trip decimal form at this precision.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_decimal_string(f.precision()))
    }
}

macro_rules! binop {
    ($tr:ident, $m:ident, $imp:ident, $atr:ident, $am:ident) => {
        impl<B: Borrow<BigFloat>> $tr<B> for &BigFloat {
            type Output = BigFloat;
            fn $m(self, b: B) -> BigFloat {
                self.$imp(b.borrow())
            }
        }
        impl<B: Borrow<BigFloat>> $tr<B> for BigFloat {
            type Output = BigFloat;
            fn $m(self, b: B) -> BigFloat {
                self.$imp(b.borrow())
            }
        }
        impl<B: Borrow<BigFloat>> $atr<B> for BigFloat {
            fn $am(&mut self, b: B) {
                *self = self.$imp(b.borrow());
            }
        }
    };
}

binop!(Add, add, add_ref, AddAssign, add_assign);
binop!(Sub, sub, sub_ref, SubAssign, sub_assign);
binop!(Mul, mul, mul_ref, MulAssign, mul_assign);
binop!(Div, div, div_ref, DivAssign, div_assign);

impl Neg for BigFloat {
    type Output = BigFloat;
    fn neg(mut self) -> BigFloat {
        self.negate();
        self
    }
}

impl Neg for &BigFloat {
    type Output = BigFloat;
    fn neg(self) -> BigFloat {
        -self.clone()
    }
}
