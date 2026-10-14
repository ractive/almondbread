//! Arbitrary-size natural numbers as trimmed little-endian `Vec<u64>` (no leading zero
//! limbs; zero is empty). Off the hot path only: decimal parsing/formatting and division.

use crate::limbs;
use core::cmp::Ordering;

pub type Nat = Vec<u64>;

pub fn trim(mut a: Nat) -> Nat {
    while a.last() == Some(&0) {
        a.pop();
    }
    a
}

pub fn bit_length(a: &[u64]) -> usize {
    limbs::bit_length(a)
}

pub fn compare(a: &[u64], b: &[u64]) -> Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    limbs::compare(a, b, a.len())
}

/// a * m + add
pub fn mul_small(a: &[u64], m: u64, add: u64) -> Nat {
    let mut r = Vec::with_capacity(a.len() + 1);
    let mut carry = add;
    for &x in a {
        let p = (x as u128) * (m as u128) + carry as u128;
        r.push(p as u64);
        carry = (p >> 64) as u64;
    }
    r.push(carry);
    trim(r)
}

/// (a / d, a % d) for a single limb d > 0.
pub fn div_small(a: &[u64], d: u64) -> (Nat, u64) {
    let mut q = vec![0u64; a.len()];
    let mut rem: u64 = 0;
    for i in (0..a.len()).rev() {
        let num = ((rem as u128) << 64) | a[i] as u128;
        q[i] = (num / d as u128) as u64;
        rem = (num % d as u128) as u64;
    }
    (trim(q), rem)
}

pub fn add(a: &[u64], b: &[u64]) -> Nat {
    let (x, y) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut r = Vec::with_capacity(x.len() + 1);
    let mut carry = 0u64;
    for i in 0..x.len() {
        let yi = if i < y.len() { y[i] } else { 0 };
        let s = x[i] as u128 + yi as u128 + carry as u128;
        r.push(s as u64);
        carry = (s >> 64) as u64;
    }
    r.push(carry);
    trim(r)
}

pub fn mul(a: &[u64], b: &[u64]) -> Nat {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    // Pad both to the same length and use the limb kernels (Karatsuba when large).
    let n = a.len().max(b.len());
    let mut x = a.to_vec();
    x.resize(n, 0);
    let mut y = b.to_vec();
    y.resize(n, 0);
    let mut r = vec![0u64; 2 * n];
    let mut s = vec![0u64; limbs::product_scratch_limbs(n) + 1];
    limbs::mul(&mut r, &x, &y, n, &mut s);
    trim(r)
}

pub fn shl(a: &[u64], bits: usize) -> Nat {
    if a.is_empty() || bits == 0 {
        return a.to_vec();
    }
    let (ls, bs) = (bits / 64, (bits % 64) as u32);
    let mut r = vec![0u64; ls];
    r.extend_from_slice(a);
    r.push(0);
    if bs > 0 {
        limbs::shl_in_place(&mut r[ls..], bs);
    }
    trim(r)
}

pub fn pow10(k: usize) -> Nat {
    let mut result: Nat = vec![1];
    let mut base: Nat = vec![10];
    let mut e = k;
    while e > 0 {
        if e & 1 != 0 {
            result = mul(&result, &base);
        }
        e >>= 1;
        if e > 0 {
            base = mul(&base, &base);
        }
    }
    result
}

/// Knuth algorithm D: (u / v, u % v), v != 0.
pub fn div_mod(u: &[u64], v: &[u64]) -> (Nat, Nat) {
    let v = trim(v.to_vec());
    let u = trim(u.to_vec());
    assert!(!v.is_empty(), "division by zero");
    if compare(&u, &v) == Ordering::Less {
        return (Vec::new(), u);
    }
    if v.len() == 1 {
        let (q, r) = div_small(&u, v[0]);
        return (q, if r == 0 { Vec::new() } else { vec![r] });
    }
    let n = v.len();
    let m = u.len() - n;
    let s = v[n - 1].leading_zeros();
    // Normalise so the divisor's top bit is set.
    let mut vn = v;
    let mut un = u;
    un.push(0);
    if s > 0 {
        limbs::shl_in_place(&mut vn, s);
        limbs::shl_in_place(&mut un, s);
    }
    let mut q = vec![0u64; m + 1];
    let (v_top, v_next) = (vn[n - 1], vn[n - 2]);
    for j in (0..=m).rev() {
        let (mut qhat, mut rhat, mut rhat_overflow);
        if un[j + n] >= v_top {
            qhat = u64::MAX;
            let (r, o) = un[j + n - 1].overflowing_add(v_top);
            rhat = r;
            rhat_overflow = o || un[j + n] > v_top;
        } else {
            let num = ((un[j + n] as u128) << 64) | un[j + n - 1] as u128;
            qhat = (num / v_top as u128) as u64;
            rhat = (num % v_top as u128) as u64;
            rhat_overflow = false;
        }
        while !rhat_overflow {
            let p = (qhat as u128) * (v_next as u128);
            let (ph, pl) = ((p >> 64) as u64, p as u64);
            if ph > rhat || (ph == rhat && pl > un[j + n - 2]) {
                qhat -= 1;
                let (r, o) = rhat.overflowing_add(v_top);
                rhat = r;
                rhat_overflow = o;
            } else {
                break;
            }
        }
        // un[j..=j+n] -= qhat * vn
        let b = limbs::sub_mul_1(&mut un[j..j + n], &vn, n, qhat);
        let (t, o) = un[j + n].overflowing_sub(b);
        un[j + n] = t;
        if o {
            qhat -= 1;
            let c = limbs::add_assign_n(&mut un[j..j + n], &vn, n);
            un[j + n] = un[j + n].wrapping_add(c);
        }
        q[j] = qhat;
    }
    let mut r = un[..n].to_vec();
    if s > 0 {
        limbs::shr_in_place(&mut r, s);
    }
    (trim(q), trim(r))
}

/// Decimal digits (no sign, "0" for zero).
pub fn to_decimal(a: &[u64]) -> String {
    if a.is_empty() {
        return "0".into();
    }
    const BASE: u64 = 10_000_000_000_000_000_000; // 10^19
    let mut chunks = Vec::new();
    let mut x = a.to_vec();
    while !x.is_empty() {
        let (q, r) = div_small(&x, BASE);
        chunks.push(r);
        x = q;
    }
    let mut s = chunks.last().unwrap().to_string();
    for c in chunks.iter().rev().skip(1) {
        s.push_str(&format!("{c:019}"));
    }
    s
}

/// Parses a run of decimal digit values (0...9).
pub fn from_decimal_digits(digits: &[u8]) -> Nat {
    let mut r: Nat = Vec::new();
    for chunk in digits.chunks(19) {
        let mut v = 0u64;
        let mut p = 1u64;
        for &d in chunk {
            v = v * 10 + d as u64;
            p *= 10;
        }
        r = mul_small(&r, p, v);
    }
    trim(r)
}
