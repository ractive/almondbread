//! Allocation-free binary floating point on caller-owned limb storage.
//!
//! A [`Raw`] is a header (sign, exponent, zero flag) plus n limbs of storage `S`
//! (`[u64; N]` in the specialised hot loops, `Vec<u64>` or slices elsewhere). The value is
//!     (-1)^neg * M / 2^(64n) * 2^exp,    M normalised: top bit of limb n-1 set,
//! so |value| lies in [2^(exp-1), 2^exp). Zero has `zero == true` (limbs kept zero too).
//! Every operation rounds to nearest, ties to even, exactly as if computed with unbounded
//! precision and then rounded to n limbs (products use the full 2n-limb product or a short
//! product whose rounding decision is certain; sums use one guard limb plus a jammed
//! sticky bit, which suffices because a subtraction with alignment distance >= 2 bits
//! cancels at most one leading bit).
//!
//! Outputs never alias inputs (the borrow checker enforces it).

use crate::limbs;

/// Limb storage of a register.
pub trait Storage: AsRef<[u64]> + AsMut<[u64]> {
    fn zeroed(n: usize) -> Self;
}

impl<const N: usize> Storage for [u64; N] {
    #[inline(always)]
    fn zeroed(n: usize) -> Self {
        debug_assert_eq!(n, N);
        [0; N]
    }
}

impl Storage for Vec<u64> {
    fn zeroed(n: usize) -> Self {
        vec![0; n]
    }
}

#[derive(Clone, Debug)]
pub struct Raw<S> {
    pub m: S,
    pub neg: bool,
    pub exp: i64,
    pub zero: bool,
}

impl<S: Storage> Raw<S> {
    #[inline(always)]
    pub fn new_zero(n: usize) -> Self {
        Raw {
            m: S::zeroed(n),
            neg: false,
            exp: 0,
            zero: true,
        }
    }
}

/// Read-only parts of a value: (limbs, neg, exp, zero).
#[derive(Clone, Copy)]
pub struct View<'a> {
    pub m: &'a [u64],
    pub neg: bool,
    pub exp: i64,
    pub zero: bool,
}

impl<S: AsRef<[u64]>> Raw<S> {
    #[inline(always)]
    pub fn view(&self) -> View<'_> {
        View {
            m: self.m.as_ref(),
            neg: self.neg,
            exp: self.exp,
            zero: self.zero,
        }
    }
}

/// Scratch limbs needed by the operations at n limbs.
pub const fn raw_scratch_limbs(n: usize) -> usize {
    3 * (n + 1) + 2 * n + limbs::product_scratch_limbs(n) + 8
}

#[inline(always)]
pub fn set_zero<S: AsMut<[u64]>>(r: &mut Raw<S>) {
    r.m.as_mut().fill(0);
    r.neg = false;
    r.exp = 0;
    r.zero = true;
}

#[inline(always)]
pub fn copy<S: AsMut<[u64]>>(r: &mut Raw<S>, a: View<'_>) {
    r.m.as_mut().copy_from_slice(a.m);
    r.neg = a.neg;
    r.exp = a.exp;
    r.zero = a.zero;
}

/// Adds one ulp to the mantissa; true on carry-out (mantissa was all ones; it is then
/// reset to 2^(64n-1) and the caller bumps the exponent).
#[inline(always)]
pub fn mantissa_increment(m: &mut [u64]) -> bool {
    if limbs::add_limb(m, 1) != 0 {
        let n = m.len();
        m[n - 1] = 1 << 63;
        return true;
    }
    false
}

// ---------------------------------------------------------------------------------------
// Multiplication

/// Rounds a full 2n-limb product p of two normalised mantissas (p >= 2^(128n-2)) into
/// `out`. Returns the exponent adjustment (0 or -1, +1 on carry-out).
#[inline(always)]
pub fn round_product(p: &[u64], n: usize, out: &mut [u64]) -> i64 {
    let (p, out) = (&p[..2 * n], &mut out[..n]);
    let mut adjust = 0;
    let round_bit;
    let rest_non_zero;
    if p[2 * n - 1] >> 63 == 0 {
        for i in (0..n).rev() {
            out[i] = (p[n + i] << 1) | (p[n + i - 1] >> 63);
        }
        round_bit = (p[n - 1] >> 62) & 1 != 0;
        rest_non_zero = p[n - 1] & ((1u64 << 62) - 1) != 0;
        adjust = -1;
    } else {
        out.copy_from_slice(&p[n..2 * n]);
        round_bit = p[n - 1] >> 63 != 0;
        rest_non_zero = p[n - 1] << 1 != 0;
    }
    if round_bit {
        // Round up unless exactly half-way and the mantissa is even.
        let up = (out[0] & 1 != 0) || rest_non_zero || !limbs::is_zero(&p[..n - 1]);
        if up && mantissa_increment(out) {
            adjust += 1;
        }
    }
    adjust
}

/// Rounds a short product (columns >= n-2 only) into `out` when the rounding decision is
/// certain, returning the exponent adjustment; `None` means "compute the full product".
/// The skipped columns add less than n units to limb n-1, so the decision is certain when
/// the bits of limb n-1 below the round bit stay below the round bit after adding n, and
/// no tie is possible: either those bits are non-zero or the round bit is clear (then the
/// exact value is below half an ulp whatever the skipped columns hold; this keeps products
/// of short mantissas, such as an orbit of exact small numbers like c = i, on the short
/// path). Fails with probability ~n/2^62 on full mantissas; the result is then identical
/// to the full product's.
#[inline(always)]
pub fn round_short_product(p: &[u64], n: usize, out: &mut [u64]) -> Option<i64> {
    let (p, out) = (&p[..2 * n], &mut out[..n]);
    let l = p[n - 1];
    let top_set = p[2 * n - 1] >> 63 != 0;
    let rb_pos = if top_set { 63 } else { 62 };
    let mask = (1u64 << rb_pos) - 1;
    let rest = l & mask;
    let round_bit = (l >> rb_pos) & 1 != 0;
    if (rest == 0 && round_bit) || rest.wrapping_add(n as u64).wrapping_add(1) > mask {
        return None;
    }
    let mut adjust = 0;
    if top_set {
        out.copy_from_slice(&p[n..2 * n]);
    } else {
        for i in (0..n).rev() {
            out[i] = (p[n + i] << 1) | (p[n + i - 1] >> 63);
        }
        adjust = -1;
    }
    if round_bit && mantissa_increment(out) {
        adjust += 1;
    }
    Some(adjust)
}

/// r = a * b.
#[inline(always)]
pub fn mul<R: AsMut<[u64]>>(r: &mut Raw<R>, a: View<'_>, b: View<'_>, scratch: &mut [u64]) {
    if a.zero || b.zero {
        set_zero(r);
        return;
    }
    let n = a.m.len();
    let neg = a.neg != b.neg;
    let e = a.exp.wrapping_add(b.exp);
    let rm = r.m.as_mut();
    if (limbs::SHORT_PRODUCT_MIN_LIMBS..=limbs::SHORT_PRODUCT_MAX_LIMBS).contains(&n) {
        limbs::mul_short(scratch, a.m, b.m, n);
        if let Some(adj) = round_short_product(scratch, n, rm) {
            r.neg = neg;
            r.exp = e + adj;
            r.zero = false;
            return;
        }
    }
    let (p, rest) = scratch.split_at_mut(2 * n);
    limbs::mul(p, a.m, b.m, n, rest);
    let adj = round_product(p, n, rm);
    r.neg = neg;
    r.exp = e + adj;
    r.zero = false;
}

/// r = a * a.
#[inline(always)]
pub fn sqr<R: AsMut<[u64]>>(r: &mut Raw<R>, a: View<'_>, scratch: &mut [u64]) {
    if a.zero {
        set_zero(r);
        return;
    }
    let n = a.m.len();
    let e = a.exp.wrapping_mul(2);
    let rm = r.m.as_mut();
    if (limbs::SHORT_PRODUCT_MIN_LIMBS..=limbs::SHORT_PRODUCT_MAX_LIMBS).contains(&n) {
        limbs::sqr_short(scratch, a.m, n);
        if let Some(adj) = round_short_product(scratch, n, rm) {
            r.neg = false;
            r.exp = e + adj;
            r.zero = false;
            return;
        }
    }
    let (p, rest) = scratch.split_at_mut(2 * n);
    limbs::sqr(p, a.m, n, rest);
    let adj = round_product(p, n, rm);
    r.neg = false;
    r.exp = e + adj;
    r.zero = false;
}

// ---------------------------------------------------------------------------------------
// Addition

/// r = a + b, or a - b when `subtract`.
#[inline(always)]
pub fn add<R: AsMut<[u64]>>(r: &mut Raw<R>, a: View<'_>, b: View<'_>, subtract: bool, scratch: &mut [u64]) {
    let b_neg = b.neg != subtract;
    if b.zero {
        copy(r, a);
        return;
    }
    if a.zero {
        copy(r, b);
        r.neg = b_neg;
        return;
    }
    let n = a.m.len();
    let effective_sub = a.neg != b_neg;
    // x = the operand of larger magnitude.
    let swap = a.exp < b.exp || (effective_sub && a.exp == b.exp && limbs::compare(a.m, b.m, n).is_lt());
    let (x, y, x_neg) = if swap { (b, a, b_neg) } else { (a, b, a.neg) };
    let d = (x.exp - y.exp) as u64;
    if d < 64 && add_aligned_fast(r, x, y, x_neg, effective_sub, d as u32, scratch) {
        return;
    }
    let w = n + 1;
    // Frame of w limbs: A = x << 64 (zero guard limb), B = (y << 64) >> d with a jammed
    // sticky bit; R = A +- B.
    let (big_r, rest) = scratch.split_at_mut(w);
    let (big_b, rest) = rest.split_at_mut(w);
    let (big_r, big_b) = (&mut big_r[..w], &mut big_b[..w]);
    let (xm, ym) = (&x.m[..n], &y.m[..n]);
    if d < 64 {
        // Fast path (operands of similar size, the common case): no bits are lost.
        if d == 0 {
            big_b[0] = 0;
            big_b[1..].copy_from_slice(ym);
        } else {
            let bs = d as u32;
            let back = 64 - bs;
            big_b[0] = ym[0] << back;
            for i in 1..n {
                big_b[i] = (ym[i - 1] >> bs) | (ym[i] << back);
            }
            big_b[n] = ym[n - 1] >> bs;
        }
    } else {
        let f = &mut rest[..w];
        f[0] = 0;
        f[1..].copy_from_slice(ym);
        if d >= 64 * w as u64 {
            big_b.fill(0);
            big_b[0] = 1;
        } else {
            limbs::extract(big_b, f, d as usize);
            if limbs::any_bit_below(f, d as usize) {
                big_b[0] |= 1;
            }
        }
    }
    let mut e = x.exp;
    if !effective_sub {
        big_r[0] = big_b[0];
        if limbs::add_chain(&mut big_r[1..], xm, &big_b[1..]) {
            let lost = big_r[0] & 1;
            limbs::shr_in_place(big_r, 1);
            big_r[w - 1] |= 1 << 63;
            big_r[0] |= lost;
            e += 1;
        }
    } else {
        big_r[0] = 0u64.wrapping_sub(big_b[0]);
        limbs::sub_chain(&mut big_r[1..], xm, &big_b[1..], big_b[0] != 0);
        // Normalise.
        if big_r[w - 1] >> 63 == 0 {
            let Some(top) = big_r.iter().rposition(|&v| v != 0) else {
                set_zero(r);
                return;
            };
            let limb_shift = w - 1 - top;
            let bit_shift = big_r[top].leading_zeros();
            if limb_shift > 0 {
                big_r.copy_within(0..w - limb_shift, limb_shift);
                big_r[..limb_shift].fill(0);
            }
            if bit_shift > 0 {
                limbs::shl_in_place(big_r, bit_shift);
            }
            e -= (limb_shift * 64) as i64 + bit_shift as i64;
        }
    }
    // Round the guard limb away.
    let g = big_r[0];
    let rm = &mut r.m.as_mut()[..n];
    rm.copy_from_slice(&big_r[1..]);
    if g >> 63 != 0 && ((g << 1) != 0 || (rm[0] & 1) != 0) && mantissa_increment(rm) {
        e += 1;
    }
    r.neg = x_neg;
    r.exp = e;
    r.zero = false;
}

/// The common case of [`add`]: alignment distance below one limb. Same frame arithmetic as
/// the general path (guard limb + sticky), but the shift is fused into the add/sub loop and
/// the result is written straight into `r`: two passes instead of four. Returns false
/// (nothing written) only for a cancellation of a whole limb or more, which the general
/// path normalises.
#[inline(always)]
fn add_aligned_fast<R: AsMut<[u64]>>(
    r: &mut Raw<R>,
    x: View<'_>,
    y: View<'_>,
    x_neg: bool,
    effective_sub: bool,
    bs: u32,
    scratch: &mut [u64],
) -> bool {
    let n = x.m.len();
    let (xm, ym) = (&x.m[..n], &y.m[..n]);
    // B = limbs 1..=n of (y << 64) >> bs; `(v << 1) << (63 - bs)` is v << (64 - bs) without
    // the overflowing shift by 64 when bs == 0. No carry chain: this pass vectorises.
    let up = |v: u64| (v << 1) << (63 - bs);
    let g0 = up(ym[0]);
    let b = &mut scratch[..n];
    for i in 0..n - 1 {
        b[i] = (ym[i] >> bs) | up(ym[i + 1]);
    }
    b[n - 1] = ym[n - 1] >> bs;
    let rm = &mut r.m.as_mut()[..n];
    let mut e = x.exp;
    let mut g;
    if !effective_sub {
        g = g0;
        if limbs::add_chain(rm, xm, b) {
            let lost = g & 1;
            g = (g >> 1) | (rm[0] << 63) | lost;
            limbs::shr_in_place(rm, 1);
            rm[n - 1] |= 1 << 63;
            e += 1;
        }
    } else {
        g = 0u64.wrapping_sub(g0);
        limbs::sub_chain(rm, xm, b, g0 != 0);
        let top = rm[n - 1];
        if top >> 63 == 0 {
            if top == 0 {
                // Whole-limb cancellation: the general path redoes it (r is output only).
                return false;
            }
            let sh = top.leading_zeros();
            limbs::shl_in_place(rm, sh);
            rm[0] |= g >> (64 - sh);
            g <<= sh;
            e -= sh as i64;
        }
    }
    // Round the guard limb away.
    if g >> 63 != 0 && ((g << 1) != 0 || (rm[0] & 1) != 0) && mantissa_increment(rm) {
        e += 1;
    }
    r.neg = x_neg;
    r.exp = e;
    r.zero = false;
    true
}

/// Compares |a| and |b|.
#[inline(always)]
pub fn compare_magnitude(a: View<'_>, b: View<'_>) -> core::cmp::Ordering {
    use core::cmp::Ordering::*;
    if a.zero {
        return if b.zero { Equal } else { Less };
    }
    if b.zero {
        return Greater;
    }
    if a.exp != b.exp {
        return a.exp.cmp(&b.exp);
    }
    limbs::compare(a.m, b.m, a.m.len())
}

/// Compares a and b.
#[inline(always)]
pub fn compare(a: View<'_>, b: View<'_>) -> core::cmp::Ordering {
    if a.zero && b.zero {
        return core::cmp::Ordering::Equal;
    }
    let sign = |v: View<'_>| {
        if v.zero {
            0
        } else if v.neg {
            -1
        } else {
            1
        }
    };
    let (sa, sb) = (sign(a), sign(b));
    if sa != sb {
        return sa.cmp(&sb);
    }
    let c = compare_magnitude(a, b);
    if sa < 0 { c.reverse() } else { c }
}

// ---------------------------------------------------------------------------------------
// Rounding from a wide integer

/// Rounds the integer W = w (plus a sticky bit below it when `sticky_in`) to a normalised
/// n-limb mantissa in `out`. Returns the bit length of the rounded result: W*2^u rounds
/// to out/2^(64n) * 2^(result + u). W must be non-zero.
pub fn round_wide(w: &[u64], sticky_in: bool, out: &mut [u64]) -> i64 {
    let n = out.len();
    let mut bl = limbs::bit_length(w) as i64;
    assert!(bl > 0, "round_wide of zero");
    let shift = bl - 64 * n as i64;
    if shift <= 0 {
        // Exact (the sticky bit lies below a zero round bit, so it never rounds up).
        let s = (-shift) as usize;
        let (ls, bs) = (s >> 6, (s & 63) as u32);
        out.fill(0);
        for (i, &v) in w.iter().enumerate() {
            if i + ls >= n {
                break;
            }
            out[i + ls] = v;
        }
        if bs > 0 {
            limbs::shl_in_place(out, bs);
        }
        return bl;
    }
    let shift = shift as usize;
    limbs::extract(out, w, shift);
    if limbs::bit(w, shift - 1) {
        let up = (out[0] & 1 != 0) || sticky_in || limbs::any_bit_below(w, shift - 1);
        if up && mantissa_increment(out) {
            bl += 1;
        }
    }
    bl
}

/// Rounds a normalised mantissa `src` to `out.len()` limbs (fewer); returns the exponent
/// adjustment (0 or 1).
pub fn round_mantissa(src: &[u64], out: &mut [u64]) -> i64 {
    round_wide(src, false, out) - 64 * src.len() as i64
}

// ---------------------------------------------------------------------------------------
// Conversions of a single value

/// 2^e as f64 (0 below the subnormal range, inf above the normal range).
#[inline(always)]
pub fn pow2(e: i64) -> f64 {
    if e > 1023 {
        f64::INFINITY
    } else if e >= -1022 {
        f64::from_bits(((e + 1023) as u64) << 52)
    } else if e >= -1074 {
        f64::from_bits(1u64 << (e + 1074))
    } else {
        0.0
    }
}

/// The top limb with a jammed sticky bit if any lower limb is non-zero: converting this
/// 64-bit value to f64 or f32 rounds the same as the full mantissa would.
#[inline(always)]
pub fn jammed_top_limb(m: &[u64]) -> u64 {
    let n = m.len();
    let mut t = m[n - 1];
    if n > 1 && !limbs::is_zero(&m[..n - 1]) {
        t |= 1;
    }
    t
}

/// Mantissa in [0.5, 1] as f64 (may round up to exactly 1.0).
#[inline(always)]
pub fn double_mantissa(a: View<'_>) -> f64 {
    jammed_top_limb(a.m) as f64 * TWO_M64
}

pub const TWO_M64: f64 = 1.0 / 18446744073709551616.0;
const TWO_M200: f64 = 6.223015277861142e-61; // 2^-200

/// Nearest f64 (ties-to-even except in the subnormal range, where it may double-round).
#[inline(always)]
pub fn to_f64(a: View<'_>) -> f64 {
    if a.zero {
        return 0.0;
    }
    let mut mant = double_mantissa(a);
    let mut e = a.exp;
    if mant == 1.0 {
        mant = 0.5;
        e += 1;
    }
    let v = if e >= -1000 {
        mant * pow2(e)
    } else {
        (mant * pow2(e + 200)) * TWO_M200
    };
    if a.neg { -v } else { v }
}

/// Quick f64 approximation for hot loops (truncates the mantissa to its top limb).
#[inline(always)]
pub fn to_f64_fast(a: View<'_>) -> f64 {
    if a.zero {
        return 0.0;
    }
    let top = a.m[a.m.len() - 1] as f64 * TWO_M64;
    let v = if a.exp >= -1000 {
        top * pow2(a.exp)
    } else {
        top * pow2(a.exp + 200) * TWO_M200
    };
    if a.neg { -v } else { v }
}

/// Sets r = d exactly (any n >= 1 holds all 53 bits). Panics on a non-finite d.
pub fn set_f64<R: AsMut<[u64]>>(r: &mut Raw<R>, d: f64) {
    assert!(d.is_finite(), "BigFloat from a non-finite f64");
    let m = r.m.as_mut();
    m.fill(0);
    if d == 0.0 {
        r.neg = false;
        r.exp = 0;
        r.zero = true;
        return;
    }
    r.neg = d < 0.0;
    let bits = d.abs().to_bits();
    let be = (bits >> 52) as i64;
    let (sig, e2) = if be != 0 {
        ((bits & ((1 << 52) - 1)) | (1 << 52), be - 1023 - 52)
    } else {
        (bits, -1074)
    };
    let lz = sig.leading_zeros();
    let n = m.len();
    m[n - 1] = sig << lz;
    r.exp = 64 - lz as i64 + e2;
    r.zero = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The short-product rounding agrees with the full product's whenever it decides, also
    /// for sparse mantissas (exact small values, trailing zero limbs, runs of ones).
    #[test]
    fn short_product_rounding_matches_full() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut decided = 0;
        for n in 3..=24 {
            for case in 0..400 {
                let mut a = vec![0u64; n];
                let mut b = vec![0u64; n];
                for (k, (x, y)) in a.iter_mut().zip(b.iter_mut()).enumerate() {
                    let pick = |v: u64, sel: u64| match sel % 4 {
                        0 => 0,
                        1 => u64::MAX,
                        _ => v,
                    };
                    let keep = case % 3 == 0 || k + 2 >= n;
                    *x = if keep { pick(next(), next()) } else { 0 };
                    *y = if keep || case % 2 == 0 { pick(next(), next()) } else { 0 };
                }
                a[n - 1] |= 1 << 63;
                b[n - 1] |= 1 << 63;
                for sq in [false, true] {
                    let bb = if sq { &a } else { &b };
                    let mut pf = vec![0u64; 2 * n];
                    limbs::mul_columns(&mut pf, &a, bb, n, 0);
                    let mut want = vec![0u64; n];
                    let adj_want = round_product(&pf, n, &mut want);
                    let mut ps = vec![0u64; 2 * n];
                    if sq {
                        limbs::sqr_columns(&mut ps, &a, n, n - 2);
                    } else {
                        limbs::mul_columns(&mut ps, &a, bb, n, n - 2);
                    }
                    let mut got = vec![0u64; n];
                    if let Some(adj) = round_short_product(&ps, n, &mut got) {
                        decided += 1;
                        assert_eq!((adj, &got), (adj_want, &want), "n={n} case={case} sq={sq}");
                    }
                }
            }
        }
        assert!(decided > 10_000);
    }

    #[test]
    fn two_m200_is_exact() {
        assert_eq!(TWO_M200, pow2(-200));
        assert_eq!(TWO_M64, pow2(-64));
    }
}
