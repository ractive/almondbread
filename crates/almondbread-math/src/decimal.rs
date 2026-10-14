//! Exact decimal parsing and correctly rounded decimal formatting for [`BigFloat`]
//! (coordinates and bookmarks such as "-0.7436438870371587047...").

use crate::bigfloat::BigFloat;
use crate::bignat::{self, Nat};
use crate::raw;
use core::cmp::Ordering;

impl BigFloat {
    /// Parses `[+-]digits[.digits][(e|E)[+-]digits]` (surrounding whitespace allowed) and
    /// rounds the exact decimal value to nearest-even at `precision_bits`. `None` if
    /// malformed.
    pub fn parse(text: &str, precision_bits: usize) -> Option<BigFloat> {
        let bytes = text.trim_matches([' ', '\t', '\n', '\r']).as_bytes();
        let mut i = 0;
        let mut negative = false;
        if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
            negative = bytes[i] == b'-';
            i += 1;
        }
        let mut digits: Vec<u8> = Vec::new();
        let mut fraction_digits: i64 = 0;
        let mut saw_digit = false;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            digits.push(bytes[i] - b'0');
            saw_digit = true;
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                digits.push(bytes[i] - b'0');
                fraction_digits += 1;
                saw_digit = true;
                i += 1;
            }
        }
        if !saw_digit {
            return None;
        }
        let mut exp10: i64 = 0;
        if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
            i += 1;
            let mut exp_neg = false;
            if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
                exp_neg = bytes[i] == b'-';
                i += 1;
            }
            let mut saw_exp = false;
            let mut e: i64 = 0;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                if e < 100_000_000 {
                    e = e * 10 + (bytes[i] - b'0') as i64;
                }
                saw_exp = true;
                i += 1;
            }
            if !saw_exp {
                return None;
            }
            exp10 = if exp_neg { -e } else { e };
        }
        if i != bytes.len() {
            return None;
        }
        // Strip leading zeros (they do not change the value) and trailing zeros (fold
        // into the exponent).
        let mut start = 0;
        while start < digits.len() - 1 && digits[start] == 0 {
            start += 1;
        }
        let mut end = digits.len();
        let mut dec = exp10 - fraction_digits;
        while end - start > 1 && digits[end - 1] == 0 {
            end -= 1;
            dec += 1;
        }
        let d = bignat::from_decimal_digits(&digits[start..end]);
        Some(Self::from_decimal_parts(&d, dec, negative, precision_bits))
    }

    /// value = ±D · 10^exponent10, correctly rounded.
    pub(crate) fn from_decimal_parts(d: &[u64], exponent10: i64, negative: bool, precision_bits: usize) -> BigFloat {
        let n = Self::limb_count_for_bits(precision_bits);
        if d.is_empty() {
            return Self::zero_limbs(n);
        }
        if exponent10 >= 0 {
            let v = bignat::mul(d, &bignat::pow10(exponent10 as usize));
            return Self::from_parts(negative, &v, 0, precision_bits);
        }
        // D / 10^k: scale D by 2^s so the quotient has at least 64n + 2 bits, then round
        // with the remainder as sticky bit.
        let t = bignat::pow10((-exponent10) as usize);
        let s = (64 * n as i64 + 2 + bignat::bit_length(&t) as i64 - bignat::bit_length(d) as i64 + 1).max(0);
        let (q, r) = bignat::div_mod(&bignat::shl(d, s as usize), &t);
        let mut v = Self::zero_limbs(n);
        let bl = raw::round_wide(&q, !r.is_empty(), &mut v.limbs);
        v.negative = negative;
        v.exponent = bl - s;
        v
    }

    /// Significant decimal digits that guarantee a round trip at this precision.
    pub fn round_trip_digits(&self) -> usize {
        // ceil(p * log10(2)) + 1
        (self.precision_bits() * 30103).div_ceil(100_000) + 1
    }

    /// Decimal string with `significant_digits` significant digits (default: round-trip),
    /// rounded half-to-even, trailing zeros removed. Positional notation for decimal
    /// exponents -7 < x < 21, otherwise scientific (`1.25e-300`).
    pub fn to_decimal_string(&self, significant_digits: Option<usize>) -> String {
        if self.is_zero() {
            return "0".into();
        }
        let digits_wanted = significant_digits.unwrap_or_else(|| self.round_trip_digits()).max(1) as i64;
        let (_, sig, e2) = self.exact_parts();
        let m = bignat::trim(sig.to_vec());
        // Estimate floor(log10 |v|).
        let mut x10 = (self.log2_magnitude() * core::f64::consts::LOG10_2).floor() as i64;
        let upper = bignat::pow10(digits_wanted as usize);
        let lower = bignat::pow10(digits_wanted as usize - 1);
        let n_digits: Nat = loop {
            let k = digits_wanted - 1 - x10;
            let mut num = m.clone();
            let mut den: Nat = vec![1];
            if k >= 0 {
                num = bignat::mul(&num, &bignat::pow10(k as usize));
            } else {
                den = bignat::pow10((-k) as usize);
            }
            if e2 >= 0 {
                num = bignat::shl(&num, e2 as usize);
            } else {
                den = bignat::shl(&den, (-e2) as usize);
            }
            let (mut q, r) = bignat::div_mod(&num, &den);
            // Fix x10 = floor(log10 |v|) on the truncated quotient, then round half to even.
            if bignat::compare(&q, &upper) != Ordering::Less {
                x10 += 1;
                continue;
            }
            if bignat::compare(&q, &lower) == Ordering::Less {
                x10 -= 1;
                continue;
            }
            let twice = bignat::shl(&r, 1);
            let c = bignat::compare(&twice, &den);
            if c == Ordering::Greater || (c == Ordering::Equal && q.first().copied().unwrap_or(0) & 1 == 1) {
                q = bignat::add(&q, &[1]);
            }
            if bignat::compare(&q, &upper) == Ordering::Equal {
                q = lower.clone();
                x10 += 1;
            }
            break q;
        };
        let mut s = bignat::to_decimal(&n_digits).into_bytes();
        while s.len() > 1 && s.last() == Some(&b'0') {
            s.pop();
        }
        let s = String::from_utf8(s).expect("ascii digits");
        let mut out = String::new();
        if self.is_negative() {
            out.push('-');
        }
        if x10 > -7 && x10 < 21 {
            if x10 >= 0 {
                let int_count = (x10 + 1) as usize;
                if s.len() <= int_count {
                    out.push_str(&s);
                    out.extend(std::iter::repeat_n('0', int_count - s.len()));
                } else {
                    out.push_str(&s[..int_count]);
                    out.push('.');
                    out.push_str(&s[int_count..]);
                }
            } else {
                out.push_str("0.");
                out.extend(std::iter::repeat_n('0', (-x10 - 1) as usize));
                out.push_str(&s);
            }
        } else {
            out.push_str(&s[..1]);
            if s.len() > 1 {
                out.push('.');
                out.push_str(&s[1..]);
            }
            if x10 < 0 {
                out.push_str(&format!("e-{}", -x10));
            } else {
                out.push_str(&format!("e+{x10}"));
            }
        }
        out
    }
}
