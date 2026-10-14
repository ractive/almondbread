//! Complex numbers over [`BigFloat`], for viewports, references and Newton steps.

use crate::bigfloat::BigFloat;
use crate::floatexp::{DoubleExp, DoubleExpComplex, FloatExpComplex};
use crate::raw::{self, View};
use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Clone, Debug, PartialEq)]
pub struct BigComplex {
    pub re: BigFloat,
    pub im: BigFloat,
}

impl BigComplex {
    pub fn new(re: BigFloat, im: BigFloat) -> Self {
        BigComplex { re, im }
    }

    pub fn from_f64(re: f64, im: f64, precision_bits: usize) -> Self {
        BigComplex {
            re: BigFloat::from_f64(re, precision_bits),
            im: BigFloat::from_f64(im, precision_bits),
        }
    }

    /// Parses decimal strings for both parts (see [`BigFloat::parse`]).
    pub fn parse(re: &str, im: &str, precision_bits: usize) -> Option<Self> {
        Some(BigComplex {
            re: BigFloat::parse(re, precision_bits)?,
            im: BigFloat::parse(im, precision_bits)?,
        })
    }

    pub fn zero(precision_bits: usize) -> Self {
        BigComplex {
            re: BigFloat::zero(precision_bits),
            im: BigFloat::zero(precision_bits),
        }
    }

    pub fn precision_bits(&self) -> usize {
        self.re.precision_bits().max(self.im.precision_bits())
    }

    pub fn with_precision(&self, bits: usize) -> Self {
        BigComplex {
            re: self.re.with_precision(bits),
            im: self.im.with_precision(bits),
        }
    }

    /// (x + iy)^2 = (x+y)(x-y) + 2xy i: two real multiplications.
    pub fn square(&self) -> Self {
        BigComplex {
            re: (&self.re + &self.im) * (&self.re - &self.im),
            im: (&self.re * &self.im).times_power_of_two(1),
        }
    }

    pub fn times_power_of_two(&self, k: i64) -> Self {
        BigComplex {
            re: self.re.times_power_of_two(k),
            im: self.im.times_power_of_two(k),
        }
    }

    pub fn scaled(&self, s: &BigFloat) -> Self {
        BigComplex {
            re: &self.re * s,
            im: &self.im * s,
        }
    }

    /// |z|^2 (correctly rounded parts, one rounding for the sum).
    pub fn magnitude_squared(&self) -> BigFloat {
        self.re.square() + self.im.square()
    }

    /// |z| in extended range (for comparisons against radii).
    pub fn magnitude_double_exp(&self) -> DoubleExp {
        DoubleExpComplex::new(self.re.double_exp(), self.im.double_exp()).magnitude()
    }

    /// Nearest f64 pair.
    pub fn to_f64(&self) -> [f64; 2] {
        [self.re.to_f64(), self.im.to_f64()]
    }

    /// f32 pair (for the GPU's float path).
    pub fn to_f32(&self) -> [f32; 2] {
        [self.re.to_f64() as f32, self.im.to_f64() as f32]
    }

    /// Shared-exponent extended-range form (see [`FloatExpComplex`]).
    pub fn float_exp_complex(&self) -> FloatExpComplex {
        let n = self.re.limb_count().max(self.im.limb_count());
        let a = self.re.with_limb_count(n);
        let b = self.im.with_limb_count(n);
        make_float_exp_complex(a.view(), b.view())
    }
}

/// Shared-exponent f32 form of x + iy (see [`FloatExpComplex`]).
#[inline(always)]
pub(crate) fn make_float_exp_complex(x: View<'_>, y: View<'_>) -> FloatExpComplex {
    if x.zero && y.zero {
        return FloatExpComplex::ZERO;
    }
    let e = if x.zero {
        y.exp
    } else if y.zero {
        x.exp
    } else {
        x.exp.max(y.exp)
    };
    // Mantissas relative to 2^e, rounded once to f32.
    let part = |v: View<'_>| -> f64 {
        if v.zero {
            return 0.0;
        }
        let d = v.exp - e;
        let m = if d < -200 {
            0.0
        } else {
            raw::jammed_top_limb(v.m) as f64 * raw::TWO_M64 * raw::pow2(d)
        };
        if v.neg { -m } else { m }
    };
    let mut fx = part(x) as f32;
    let mut fy = part(y) as f32;
    let mut ee = e;
    if fx.abs().max(fy.abs()) >= 1.0 {
        fx *= 0.5;
        fy *= 0.5;
        ee += 1;
    }
    FloatExpComplex {
        mantissa: [fx, fy],
        exponent: ee.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
    }
}

impl Add for &BigComplex {
    type Output = BigComplex;
    fn add(self, b: &BigComplex) -> BigComplex {
        BigComplex {
            re: &self.re + &b.re,
            im: &self.im + &b.im,
        }
    }
}

impl Sub for &BigComplex {
    type Output = BigComplex;
    fn sub(self, b: &BigComplex) -> BigComplex {
        BigComplex {
            re: &self.re - &b.re,
            im: &self.im - &b.im,
        }
    }
}

impl Neg for &BigComplex {
    type Output = BigComplex;
    fn neg(self) -> BigComplex {
        BigComplex {
            re: -&self.re,
            im: -&self.im,
        }
    }
}

impl Mul for &BigComplex {
    type Output = BigComplex;
    /// Four correctly rounded products, (ac - bd) and (ad + bc): each part stays accurate
    /// relative to its own size.
    fn mul(self, b: &BigComplex) -> BigComplex {
        BigComplex {
            re: &self.re * &b.re - &self.im * &b.im,
            im: &self.re * &b.im + &self.im * &b.re,
        }
    }
}

impl Div for &BigComplex {
    type Output = BigComplex;
    fn div(self, b: &BigComplex) -> BigComplex {
        let den = b.re.square() + b.im.square();
        BigComplex {
            re: (&self.re * &b.re + &self.im * &b.im) / &den,
            im: (&self.im * &b.re - &self.re * &b.im) / &den,
        }
    }
}

macro_rules! owned_ops {
    ($($tr:ident $m:ident),*) => {$(
        impl $tr for BigComplex {
            type Output = BigComplex;
            fn $m(self, b: BigComplex) -> BigComplex { (&self).$m(&b) }
        }
        impl $tr<&BigComplex> for BigComplex {
            type Output = BigComplex;
            fn $m(self, b: &BigComplex) -> BigComplex { (&self).$m(b) }
        }
    )*};
}
owned_ops!(Add add, Sub sub, Mul mul, Div div);

impl fmt::Display for BigComplex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.re, self.im)
    }
}
