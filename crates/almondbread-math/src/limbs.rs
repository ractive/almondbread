//! Unsigned multi-limb arithmetic on caller-owned slices.
//!
//! Limbs are `u64`, little-endian (limb 0 is least significant). Nothing here allocates,
//! so the reference-orbit loop runs without touching the heap. Products use `u128` and
//! column-wise (Comba / product scanning) accumulation with two independent 128-bit
//! accumulators (low halves and high halves), so the add chains do not serialise through
//! one carry (kb: platform/mpfr-gmp-on-apple-silicon). Every routine is `#[inline(always)]`:
//! when the caller passes `[u64; N]` storage the lengths are compile-time constants and
//! LLVM unrolls the loops (the fixed-size kernels).

/// Karatsuba takes over from the Comba basecase at this many limbs (full products only;
/// the orbit uses short Comba products up to [`SHORT_PRODUCT_MAX_LIMBS`]).
pub const KARATSUBA_MUL_THRESHOLD: usize = 40;
pub const KARATSUBA_SQR_THRESHOLD: usize = 48;

/// Below this many limbs products are always full (the short product saves nothing).
pub const SHORT_PRODUCT_MIN_LIMBS: usize = 3;
/// Up to this many limbs the short Comba product is used (above it the full Karatsuba).
pub const SHORT_PRODUCT_MAX_LIMBS: usize = 400;

// ---------------------------------------------------------------------------------------
// Linear operations

// The carry chains are unrolled by four: inside the unrolled body LLVM keeps the carry in
// the flags (`adds`/`adcs`), while a one-limb loop materialises it in a register every
// iteration (measured 2.5x faster at 32 limbs on M1 Pro).

/// One unrolled step of a carry chain over four limbs.
macro_rules! chain4 {
    ($op:ident, $r:expr, $a:expr, $b:expr, $c:ident) => {{
        let (s0, c0) = $a[0].$op($b[0], $c);
        let (s1, c1) = $a[1].$op($b[1], c0);
        let (s2, c2) = $a[2].$op($b[2], c1);
        let (s3, c3) = $a[3].$op($b[3], c2);
        *$r = [s0, s1, s2, s3];
        $c = c3;
    }};
}

/// r = a + b (equal lengths, r.len() limbs); returns the carry.
#[inline(always)]
pub fn add_chain(r: &mut [u64], a: &[u64], b: &[u64]) -> bool {
    let n = r.len();
    let (r4, rr) = r.as_chunks_mut::<4>();
    let (a4, ar) = a[..n].as_chunks::<4>();
    let (b4, br) = b[..n].as_chunks::<4>();
    let mut c = false;
    for ((r, a), b) in r4.iter_mut().zip(a4).zip(b4) {
        chain4!(carrying_add, r, a, b, c);
    }
    for ((r, a), b) in rr.iter_mut().zip(ar).zip(br) {
        (*r, c) = a.carrying_add(*b, c);
    }
    c
}

/// r = a - b - borrow_in (equal lengths); returns the borrow.
#[inline(always)]
pub fn sub_chain(r: &mut [u64], a: &[u64], b: &[u64], borrow_in: bool) -> bool {
    let n = r.len();
    let (r4, rr) = r.as_chunks_mut::<4>();
    let (a4, ar) = a[..n].as_chunks::<4>();
    let (b4, br) = b[..n].as_chunks::<4>();
    let mut c = borrow_in;
    for ((r, a), b) in r4.iter_mut().zip(a4).zip(b4) {
        chain4!(borrowing_sub, r, a, b, c);
    }
    for ((r, a), b) in rr.iter_mut().zip(ar).zip(br) {
        (*r, c) = a.borrowing_sub(*b, c);
    }
    c
}

/// r += b (in place, r.len() limbs); returns the carry.
#[inline(always)]
pub fn add_assign_chain(r: &mut [u64], b: &[u64]) -> bool {
    let n = r.len();
    let (r4, rr) = r.as_chunks_mut::<4>();
    let (b4, br) = b[..n].as_chunks::<4>();
    let mut c = false;
    for (r, b) in r4.iter_mut().zip(b4) {
        let a = *r;
        chain4!(carrying_add, r, a, b, c);
    }
    for (r, b) in rr.iter_mut().zip(br) {
        (*r, c) = r.carrying_add(*b, c);
    }
    c
}

/// r -= b (in place); returns the borrow.
#[inline(always)]
pub fn sub_assign_chain(r: &mut [u64], b: &[u64]) -> bool {
    let n = r.len();
    let (r4, rr) = r.as_chunks_mut::<4>();
    let (b4, br) = b[..n].as_chunks::<4>();
    let mut c = false;
    for (r, b) in r4.iter_mut().zip(b4) {
        let a = *r;
        chain4!(borrowing_sub, r, a, b, c);
    }
    for (r, b) in rr.iter_mut().zip(br) {
        (*r, c) = r.borrowing_sub(*b, c);
    }
    c
}

/// r = a + b over n limbs; returns the carry (0 or 1).
#[inline(always)]
pub fn add_n(r: &mut [u64], a: &[u64], b: &[u64], n: usize) -> u64 {
    add_chain(&mut r[..n], a, b) as u64
}

/// r += b over n limbs (in place); returns the carry.
#[inline(always)]
pub fn add_assign_n(r: &mut [u64], b: &[u64], n: usize) -> u64 {
    add_assign_chain(&mut r[..n], b) as u64
}

/// r -= b over n limbs (in place); returns the borrow.
#[inline(always)]
pub fn sub_assign_n(r: &mut [u64], b: &[u64], n: usize) -> u64 {
    sub_assign_chain(&mut r[..n], b) as u64
}

/// Adds the single limb `v` into r; returns the carry out.
#[inline(always)]
pub fn add_limb(r: &mut [u64], v: u64) -> u64 {
    let mut carry = v;
    for x in r.iter_mut() {
        if carry == 0 {
            break;
        }
        let (s, o) = x.overflowing_add(carry);
        *x = s;
        carry = o as u64;
    }
    carry
}

/// Subtracts the single limb `v` from r; returns the borrow out.
#[inline(always)]
pub fn sub_limb(r: &mut [u64], v: u64) -> u64 {
    let mut borrow = v;
    for x in r.iter_mut() {
        if borrow == 0 {
            break;
        }
        let (s, o) = x.overflowing_sub(borrow);
        *x = s;
        borrow = o as u64;
    }
    borrow
}

/// Compares a and b (n limbs each).
#[inline(always)]
pub fn compare(a: &[u64], b: &[u64], n: usize) -> core::cmp::Ordering {
    for i in (0..n).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    core::cmp::Ordering::Equal
}

#[inline(always)]
pub fn is_zero(a: &[u64]) -> bool {
    a.iter().all(|&x| x == 0)
}

/// r <<= bits (0 < bits < 64) in place; returns the bits shifted out of the top limb.
#[inline(always)]
pub fn shl_in_place(r: &mut [u64], bits: u32) -> u64 {
    let n = r.len();
    let back = 64 - bits;
    let out = r[n - 1] >> back;
    for i in (1..n).rev() {
        r[i] = (r[i] << bits) | (r[i - 1] >> back);
    }
    r[0] <<= bits;
    out
}

/// r >>= bits (0 < bits < 64) in place; returns the bits shifted out, left-aligned.
#[inline(always)]
pub fn shr_in_place(r: &mut [u64], bits: u32) -> u64 {
    let n = r.len();
    let back = 64 - bits;
    let out = r[0] << back;
    for i in 0..n - 1 {
        r[i] = (r[i] >> bits) | (r[i + 1] << back);
    }
    r[n - 1] >>= bits;
    out
}

/// Number of significant bits (0 if zero).
#[inline(always)]
pub fn bit_length(a: &[u64]) -> usize {
    for i in (0..a.len()).rev() {
        if a[i] != 0 {
            return i * 64 + (64 - a[i].leading_zeros() as usize);
        }
    }
    0
}

/// True if any bit below bit position `pos` is set.
#[inline(always)]
pub fn any_bit_below(a: &[u64], pos: usize) -> bool {
    if pos == 0 {
        return false;
    }
    let full = (pos >> 6).min(a.len());
    if a[..full].iter().any(|&x| x != 0) {
        return true;
    }
    let rem = pos & 63;
    if rem != 0 && full < a.len() {
        return a[full] & ((1u64 << rem) - 1) != 0;
    }
    false
}

/// Bit `pos` (false beyond the end).
#[inline(always)]
pub fn bit(a: &[u64], pos: usize) -> bool {
    let l = pos >> 6;
    l < a.len() && (a[l] >> (pos & 63)) & 1 != 0
}

/// r = bits [shift, shift + 64 r.len()) of a (zero beyond the end).
#[inline(always)]
pub fn extract(r: &mut [u64], a: &[u64], shift: usize) {
    let ls = shift >> 6;
    let bs = (shift & 63) as u32;
    let n = a.len();
    for (i, out) in r.iter_mut().enumerate() {
        let j = i + ls;
        let lo = if j < n { a[j] } else { 0 };
        *out = if bs == 0 {
            lo
        } else {
            let hi = if j + 1 < n { a[j + 1] } else { 0 };
            (lo >> bs) | (hi << (64 - bs))
        };
    }
}

// ---------------------------------------------------------------------------------------
// Products by one limb

/// r = a * m over n limbs; returns the high limb.
#[inline(always)]
pub fn mul_1(r: &mut [u64], a: &[u64], n: usize, m: u64) -> u64 {
    let (r, a) = (&mut r[..n], &a[..n]);
    let mut carry: u64 = 0;
    for i in 0..n {
        let p = (a[i] as u128) * (m as u128) + carry as u128;
        r[i] = p as u64;
        carry = (p >> 64) as u64;
    }
    carry
}

/// r -= a * m over n limbs; returns the borrow limb.
#[inline(always)]
pub fn sub_mul_1(r: &mut [u64], a: &[u64], n: usize, m: u64) -> u64 {
    let (r, a) = (&mut r[..n], &a[..n]);
    let mut carry: u64 = 0;
    for i in 0..n {
        let p = (a[i] as u128) * (m as u128) + carry as u128;
        let (s, o) = r[i].overflowing_sub(p as u64);
        r[i] = s;
        carry = ((p >> 64) as u64) + o as u64;
    }
    carry
}

// ---------------------------------------------------------------------------------------
// Basecase products (Comba)
//
// Column k sums a_i * b_(k-i). `k0` > 0 skips the low columns (short product): limbs below
// k0 are then not written and limbs from k0 up lack the carries of the skipped columns
// (see `round_short_product` in raw.rs).

/// r[k0..2n] = high columns of a * b.
#[inline(always)]
pub fn mul_columns(r: &mut [u64], a: &[u64], b: &[u64], n: usize, k0: usize) {
    let (r, a, b) = (&mut r[..2 * n], &a[..n], &b[..n]);
    let mut cin: u128 = 0;
    let top = 2 * n - 1;
    for k in k0..top {
        let lo_i = if k < n { 0 } else { k - n + 1 };
        let hi_i = if k < n { k } else { n - 1 };
        let mut lo = cin;
        let mut hi: u128 = 0;
        for (x, y) in a[lo_i..=hi_i].iter().zip(b[k - hi_i..=k - lo_i].iter().rev()) {
            let p = (*x as u128) * (*y as u128);
            lo += p as u64 as u128;
            hi += p >> 64;
        }
        r[k] = lo as u64;
        cin = (lo >> 64) + hi;
    }
    r[top] = cin as u64;
}

/// r[k0..2n] = high columns of a * a.
#[inline(always)]
pub fn sqr_columns(r: &mut [u64], a: &[u64], n: usize, k0: usize) {
    let (r, a) = (&mut r[..2 * n], &a[..n]);
    let mut cin: u128 = 0;
    let top = 2 * n - 1;
    for k in k0..top {
        let lo_i = if k < n { 0 } else { k - n + 1 };
        // Off-diagonal pairs i < j with i + j = k.
        let pairs = (k + 1 - 2 * lo_i) / 2; // count of i in [lo_i, ..) with i < k - i
        let mut lo: u128 = 0;
        let mut hi: u128 = 0;
        let hi_j = k - lo_i;
        for (x, y) in a[lo_i..lo_i + pairs]
            .iter()
            .zip(a[hi_j + 1 - pairs..=hi_j].iter().rev())
        {
            let p = (*x as u128) * (*y as u128);
            lo += p as u64 as u128;
            hi += p >> 64;
        }
        // Double the off-diagonal sums (each < 2^127 for the column counts used here).
        lo <<= 1;
        hi <<= 1;
        if k.is_multiple_of(2) {
            let d = a[k / 2];
            let p = (d as u128) * (d as u128);
            lo += p as u64 as u128;
            hi += p >> 64;
        }
        lo += cin;
        r[k] = lo as u64;
        cin = (lo >> 64) + hi;
    }
    r[top] = cin as u64;
}

#[inline(always)]
fn acc(lo: &mut u128, hi: &mut u128, x: u64, y: u64) {
    let p = (x as u128) * (y as u128);
    *lo += p as u64 as u128;
    *hi += p >> 64;
}

/// Two-column Comba: columns k and k+1 together share the a[i] loads and run two
/// independent accumulators (measured 5-20 % faster than one column at 9...256 limbs).
/// Same output as [`mul_columns`].
#[inline(always)]
pub fn mul_columns2(r: &mut [u64], a: &[u64], b: &[u64], n: usize, k0: usize) {
    let (r, a, b) = (&mut r[..2 * n], &a[..n], &b[..n]);
    let mut cin: u128 = 0;
    let top = 2 * n - 1;
    let mut k = k0;
    while k + 1 < top {
        let (mut lo0, mut hi0, mut lo1, mut hi1) = (cin, 0u128, 0u128, 0u128);
        if k + 1 >= n {
            acc(&mut lo0, &mut hi0, a[k + 1 - n], b[n - 1]);
        }
        let i_lo = (k + 2).saturating_sub(n);
        let i_hi = k.min(n - 1);
        if i_lo <= i_hi {
            for (x, w) in a[i_lo..=i_hi].iter().zip(b[k - i_hi..=k + 1 - i_lo].windows(2).rev()) {
                acc(&mut lo0, &mut hi0, *x, w[0]);
                acc(&mut lo1, &mut hi1, *x, w[1]);
            }
        }
        if k + 1 < n {
            acc(&mut lo1, &mut hi1, a[k + 1], b[0]);
        }
        r[k] = lo0 as u64;
        lo1 += (lo0 >> 64) + hi0;
        r[k + 1] = lo1 as u64;
        cin = (lo1 >> 64) + hi1;
        k += 2;
    }
    if k < top {
        let lo_i = if k < n { 0 } else { k - n + 1 };
        let hi_i = if k < n { k } else { n - 1 };
        let (mut lo, mut hi) = (cin, 0u128);
        for (x, y) in a[lo_i..=hi_i].iter().zip(b[k - hi_i..=k - lo_i].iter().rev()) {
            acc(&mut lo, &mut hi, *x, *y);
        }
        r[k] = lo as u64;
        cin = (lo >> 64) + hi;
    }
    r[top] = cin as u64;
}

/// Two-column Comba squaring; same output as [`sqr_columns`].
#[inline(always)]
pub fn sqr_columns2(r: &mut [u64], a: &[u64], n: usize, k0: usize) {
    let (r, a) = (&mut r[..2 * n], &a[..n]);
    let mut cin: u128 = 0;
    let top = 2 * n - 1;
    let mut k = k0;
    while k + 1 < top {
        let (mut lo0, mut hi0, mut lo1, mut hi1) = (0u128, 0u128, 0u128, 0u128);
        // col k pairs: i in [max(0,k-n+1), (k-1)/2]; col k+1: i in [max(0,k-n+2), k/2]
        let lo_k = (k + 1).saturating_sub(n);
        let lo_k1 = (k + 2).saturating_sub(n);
        let hi_k = if k >= 1 { (k - 1) / 2 } else { 0 };
        let has_k = k >= 1 && lo_k <= hi_k;
        if has_k && lo_k < lo_k1 {
            acc(&mut lo0, &mut hi0, a[lo_k], a[k - lo_k]);
        }
        let c_lo = lo_k1;
        let c_hi = hi_k; // common range [c_lo, c_hi] when k >= 1
        if k >= 1 && c_lo <= c_hi {
            for (x, w) in a[c_lo..=c_hi].iter().zip(a[k - c_hi..=k + 1 - c_lo].windows(2).rev()) {
                acc(&mut lo0, &mut hi0, *x, w[0]);
                acc(&mut lo1, &mut hi1, *x, w[1]);
            }
        }
        if k.is_multiple_of(2) && k / 2 >= lo_k1 && k / 2 + 1 < n {
            acc(&mut lo1, &mut hi1, a[k / 2], a[k / 2 + 1]);
        }
        lo0 <<= 1;
        hi0 <<= 1;
        lo1 <<= 1;
        hi1 <<= 1;
        if k.is_multiple_of(2) {
            acc(&mut lo0, &mut hi0, a[k / 2], a[k / 2]);
        } else {
            acc(&mut lo1, &mut hi1, a[k.div_ceil(2)], a[k.div_ceil(2)]);
        }
        lo0 += cin;
        r[k] = lo0 as u64;
        lo1 += (lo0 >> 64) + hi0;
        r[k + 1] = lo1 as u64;
        cin = (lo1 >> 64) + hi1;
        k += 2;
    }
    if k < top {
        // last column 2n-2: only the diagonal a[n-1]^2
        let (mut lo, mut hi) = (cin, 0u128);
        acc(&mut lo, &mut hi, a[n - 1], a[n - 1]);
        r[k] = lo as u64;
        cin = (lo >> 64) + hi;
    }
    r[top] = cin as u64;
}

/// Largest limb count whose short products are fully unrolled. Above it the operands no
/// longer fit in registers and the looped kernel wins (measured: unrolled 16-limb products
/// spill).
pub const UNROLL_MAX_LIMBS: usize = 8;

/// Short product a*b at limbs n-2..2n. With constant `n <= UNROLL_MAX_LIMBS` (fixed-size
/// storage) the column loops unroll completely: the "fixed-limb kernels".
#[inline(always)]
pub fn mul_short(r: &mut [u64], a: &[u64], b: &[u64], n: usize) {
    if n <= UNROLL_MAX_LIMBS {
        mul_columns(r, a, b, n, n - 2)
    } else {
        mul_short_loop(r, a, b, n)
    }
}

#[inline(always)]
pub fn sqr_short(r: &mut [u64], a: &[u64], n: usize) {
    if n <= UNROLL_MAX_LIMBS {
        sqr_columns(r, a, n, n - 2)
    } else {
        sqr_short_loop(r, a, n)
    }
}

#[inline(never)]
fn mul_short_loop(r: &mut [u64], a: &[u64], b: &[u64], n: usize) {
    mul_columns2(r, a, b, n, n - 2)
}

#[inline(never)]
fn sqr_short_loop(r: &mut [u64], a: &[u64], n: usize) {
    sqr_columns2(r, a, n, n - 2)
}

// ---------------------------------------------------------------------------------------
// Karatsuba

/// Scratch limbs needed by [`mul`]/[`sqr`] for n-limb operands (beyond the result).
pub const fn product_scratch_limbs(n: usize) -> usize {
    let t = if KARATSUBA_MUL_THRESHOLD < KARATSUBA_SQR_THRESHOLD {
        KARATSUBA_MUL_THRESHOLD
    } else {
        KARATSUBA_SQR_THRESHOLD
    };
    if n < t {
        return 0;
    }
    let l = n - n / 2;
    4 * (l + 1) + product_scratch_limbs(l + 1)
}

/// r[0..2n] = a * b; `scratch` needs `product_scratch_limbs(n)`.
#[inline]
pub fn mul(r: &mut [u64], a: &[u64], b: &[u64], n: usize, scratch: &mut [u64]) {
    if n < KARATSUBA_MUL_THRESHOLD {
        mul_columns2(r, a, b, n, 0)
    } else {
        mul_karatsuba(r, a, b, n, scratch)
    }
}

/// r[0..2n] = a * a; `scratch` needs `product_scratch_limbs(n)`.
#[inline]
pub fn sqr(r: &mut [u64], a: &[u64], n: usize, scratch: &mut [u64]) {
    if n < KARATSUBA_SQR_THRESHOLD {
        sqr_columns2(r, a, n, 0)
    } else {
        sqr_karatsuba(r, a, n, scratch)
    }
}

/// a = a0 + a1 B^h with a0 of h limbs and a1 of l = n - h limbs (l = h or h + 1).
/// Writes a0 + a1 into s[0..=l].
fn karatsuba_half_sum(s: &mut [u64], a: &[u64], h: usize, l: usize) {
    let c = add_n(s, &a[..h], &a[h..2 * h], h);
    let mut top = c;
    if l > h {
        let (v, o) = a[2 * h].overflowing_add(c);
        s[h] = v;
        top = o as u64;
    }
    s[l] = top;
}

/// Adds z1 = sum - z0 - z2 into r at limb offset h (shared tail of mul and sqr).
fn karatsuba_combine(r: &mut [u64], z1: &mut [u64], n: usize, h: usize, l: usize) {
    let w = 2 * l + 2;
    // z1 -= z0 (2h limbs), z1 -= z2 (2l limbs); the result is a0*b1 + a1*b0 >= 0.
    let bw = sub_assign_n(z1, &r[..2 * h], 2 * h);
    sub_limb(&mut z1[2 * h..w], bw);
    let bw = sub_assign_n(z1, &r[2 * h..2 * h + 2 * l], 2 * l);
    sub_limb(&mut z1[2 * l..w], bw);
    // r[h..] += z1; z1 < 2^(64(2l+1)) and r has 2n - h >= 2l + 1 limbs above h.
    let len = w.min(2 * n - h);
    let c = add_assign_n(&mut r[h..h + len], z1, len);
    add_limb(&mut r[h + len..2 * n], c);
}

fn mul_karatsuba(r: &mut [u64], a: &[u64], b: &[u64], n: usize, scratch: &mut [u64]) {
    let h = n / 2;
    let l = n - h;
    let (sa, rest) = scratch.split_at_mut(l + 1);
    let (sb, rest) = rest.split_at_mut(l + 1);
    let (z1, rest) = rest.split_at_mut(2 * (l + 1));
    {
        let (r0, r1) = r.split_at_mut(2 * h);
        mul(r0, &a[..h], &b[..h], h, rest);
        mul(r1, &a[h..n], &b[h..n], l, rest);
    }
    karatsuba_half_sum(sa, a, h, l);
    karatsuba_half_sum(sb, b, h, l);
    mul(z1, sa, sb, l + 1, rest);
    karatsuba_combine(r, z1, n, h, l);
}

fn sqr_karatsuba(r: &mut [u64], a: &[u64], n: usize, scratch: &mut [u64]) {
    let h = n / 2;
    let l = n - h;
    let (sa, rest) = scratch.split_at_mut(2 * (l + 1));
    let (z1, rest) = rest.split_at_mut(2 * (l + 1));
    {
        let (r0, r1) = r.split_at_mut(2 * h);
        sqr(r0, &a[..h], h, rest);
        sqr(r1, &a[h..n], l, rest);
    }
    karatsuba_half_sum(sa, a, h, l);
    sqr(z1, &sa[..l + 1], l + 1, rest);
    karatsuba_combine(r, z1, n, h, l);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rand_limbs(seed: &mut u64, n: usize) -> Vec<u64> {
        (0..n)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 7;
                *seed ^= *seed << 17;
                *seed
            })
            .collect()
    }

    #[test]
    fn karatsuba_matches_basecase() {
        let mut seed = 0x1234_5678_9abc_def1u64;
        for n in [40, 41, 47, 48, 63, 64, 65, 96, 127, 128, 200, 256, 257] {
            let mut a = rand_limbs(&mut seed, n);
            let mut b = rand_limbs(&mut seed, n);
            if n % 2 == 0 {
                a[n - 1] = u64::MAX;
                b[0] = u64::MAX;
            }
            let mut scratch = vec![0u64; product_scratch_limbs(n) + 1];
            let mut r1 = vec![0u64; 2 * n];
            let mut r2 = r1.clone();
            let mut r3 = r1.clone();
            let mut r4 = r1.clone();
            mul_columns(&mut r1, &a, &b, n, 0);
            mul_karatsuba(&mut r2, &a, &b, n, &mut scratch);
            sqr_columns(&mut r3, &a, n, 0);
            sqr_karatsuba(&mut r4, &a, n, &mut scratch);
            assert_eq!(r1, r2, "mul n={n}");
            assert_eq!(r3, r4, "sqr n={n}");
        }
    }

    #[test]
    fn square_basecase_matches_mul() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for n in 1..=70 {
            let mut a = rand_limbs(&mut seed, n);
            if n > 2 {
                a[1] = u64::MAX;
                a[n - 1] = u64::MAX;
            }
            let mut r1 = vec![0u64; 2 * n];
            let mut r2 = r1.clone();
            mul_columns(&mut r1, &a, &a, n, 0);
            sqr_columns(&mut r2, &a, n, 0);
            assert_eq!(r1, r2, "n={n}");
            let mut r3 = vec![0u64; 2 * n];
            sqr_columns2(&mut r3, &a, n, 0);
            assert_eq!(r1, r3, "2col sqr n={n}");
            mul_columns2(&mut r3, &a, &a, n, 0);
            assert_eq!(r1, r3, "2col mul n={n}");
        }
    }
}
