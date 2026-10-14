//! Property tests: every BigFloat operation must equal the exact rational result rounded to
//! nearest-even at the working precision (num-bigint / num-rational as the oracle).

use almondbread_math::{BigComplex, BigFloat, ReferenceOrbitComputer, Variant};
use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use proptest::prelude::*;

fn to_rational(v: &BigFloat) -> BigRational {
    let (neg, sig, e) = v.exact_parts();
    let mut digits = Vec::new();
    for l in sig {
        digits.push(*l as u32);
        digits.push((*l >> 32) as u32);
    }
    let s = BigInt::from_biguint(if neg { Sign::Minus } else { Sign::Plus }, BigUint::new(digits));
    if e >= 0 {
        BigRational::from_integer(s << e as usize)
    } else {
        BigRational::new(s, BigInt::one() << (-e) as usize)
    }
}

/// Rounds r to `prec` bits, ties to even: (negative, S with exactly prec bits, E).
fn round(r: &BigRational, prec: usize) -> Option<(bool, BigUint, i64)> {
    if r.is_zero() {
        return None;
    }
    let neg = r.is_negative();
    let a = r.abs();
    let (num, den) = (a.numer().magnitude().clone(), a.denom().magnitude().clone());
    // e such that 2^(prec-1) <= a / 2^e < 2^prec.
    let mut e = num.bits() as i64 - den.bits() as i64 - prec as i64;
    loop {
        let (n2, d2) = if e >= 0 {
            (num.clone(), &den << e as usize)
        } else {
            (&num << (-e) as usize, den.clone())
        };
        let (q, rem) = n2.div_rem(&d2);
        let bits = q.bits() as usize;
        if bits > prec {
            e += 1;
            continue;
        }
        if bits < prec {
            e -= 1;
            continue;
        }
        let twice: BigUint = &rem << 1usize;
        let up = twice > d2 || (twice == d2 && q.bit(0));
        let mut q = q;
        if up {
            q += 1u32;
        }
        if q.bits() as usize > prec {
            q >>= 1usize;
            e += 1;
        }
        return Some((neg, q, e));
    }
}

fn parts(v: &BigFloat) -> Option<(bool, BigUint, i64)> {
    if v.is_zero() {
        return None;
    }
    let (neg, sig, e) = v.exact_parts();
    let mut digits = Vec::new();
    for l in sig {
        digits.push(*l as u32);
        digits.push((*l >> 32) as u32);
    }
    Some((neg, BigUint::new(digits), e))
}

fn arb_value(limbs: usize) -> impl Strategy<Value = (bool, Vec<u64>, i64)> {
    (any::<bool>(), prop::collection::vec(any::<u64>(), limbs), -400i64..400)
}

fn make(prec_limbs: usize, (neg, mut m, e): (bool, Vec<u64>, i64)) -> BigFloat {
    let n = m.len();
    m[n - 1] |= 1 << 63;
    BigFloat::from_parts(neg, &m, e, prec_limbs * 64)
}

fn limb_counts() -> impl Strategy<Value = usize> {
    prop_oneof![4 => 1usize..=20, 1 => 21usize..=70]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2000, ..ProptestConfig::default() })]

    #[test]
    fn ops_are_correctly_rounded(
        (n, a, b, shift, small) in limb_counts().prop_flat_map(|n| (Just(n), arb_value(n), arb_value(n), 0i64..200, any::<i64>()))
    ) {
        let x = make(n, a);
        // Bring b near a in exponent half of the time to exercise cancellation.
        let mut y = make(n, b);
        if shift % 2 == 0 {
            y = y.times_power_of_two(x.exponent() - y.exponent() - shift / 50);
        }
        let prec = n * 64;
        let (rx, ry) = (to_rational(&x), to_rational(&y));
        prop_assert_eq!(parts(&(&x + &y)), round(&(&rx + &ry), prec), "add");
        prop_assert_eq!(parts(&(&x - &y)), round(&(&rx - &ry), prec), "sub");
        prop_assert_eq!(parts(&(&x * &y)), round(&(&rx * &ry), prec), "mul");
        prop_assert_eq!(parts(&x.square()), round(&(&rx * &rx), prec), "sqr");
        prop_assert_eq!(parts(&(&x / &y)), round(&(&rx / &ry), prec), "div");
        prop_assert_eq!(parts(&x.mul_int(small)), round(&(&rx * BigRational::from_integer(BigInt::from(small))), prec), "mul_int");
        prop_assert_eq!(x.compare(&y), rx.cmp(&ry), "compare");
    }

    #[test]
    fn near_cancellation_is_exact(n in 1usize..=12, a in any::<u64>(), delta in 1u64..1000, e in -50i64..50) {
        // x and y equal except in the last limb: the difference is exact.
        let mut m = vec![u64::MAX; n];
        m[0] = a;
        let x = BigFloat::from_parts(false, &m, e, n * 64);
        m[0] = a.wrapping_sub(delta);
        let y = BigFloat::from_parts(false, &m, e, n * 64);
        let d = &x - &y;
        prop_assert_eq!(parts(&d), round(&(to_rational(&x) - to_rational(&y)), n * 64));
    }

    #[test]
    fn parse_is_correctly_rounded(
        neg in any::<bool>(),
        int in "[0-9]{0,30}",
        frac in "[0-9]{0,60}",
        exp in -400i64..400,
        n in 1usize..=12,
    ) {
        prop_assume!(!(int.is_empty() && frac.is_empty()));
        let text = format!("{}{}.{}e{}", if neg { "-" } else { "" }, int, frac, exp);
        let v = BigFloat::parse(&text, n * 64).unwrap();
        let digits: BigInt = format!("{int}{frac}").trim_start_matches('0').parse().unwrap_or_default();
        let e10 = exp - frac.len() as i64;
        let ten = BigInt::from(10);
        let mut r = if e10 >= 0 {
            BigRational::from_integer(digits * num_traits::pow(ten, e10 as usize))
        } else {
            BigRational::new(digits, num_traits::pow(ten, (-e10) as usize))
        };
        if neg { r = -r; }
        prop_assert_eq!(parts(&v), round(&r, n * 64), "{}", text);
        // Formatting round-trips at this precision.
        let back = BigFloat::parse(&v.to_string(), n * 64).unwrap();
        prop_assert_eq!(parts(&back), parts(&v));
    }

    #[test]
    fn sqrt_within_one_ulp((n, a) in (1usize..=16).prop_flat_map(|n| (Just(n), arb_value(n)))) {
        let x = make(n, (false, a.1, a.2));
        let r = x.sqrt();
        // |r^2 - x| <= ~2 ulp(x) relative bound for a 1-ulp root.
        let err = (to_rational(&r) * to_rational(&r) - to_rational(&x)).abs();
        let ulp = BigRational::new(BigInt::one() << 3usize, BigInt::one()) * to_rational(&x)
            / BigRational::from_integer(BigInt::one() << (n * 64));
        prop_assert!(err <= ulp);
    }

    #[test]
    fn orbit_variants_match_bigfloat_iteration(
        n in 1usize..=20,
        re in -2.0f64..0.4,
        im in -1.2f64..1.2,
        iters in 1usize..60,
    ) {
        // The register loops must equal the same iteration written with BigFloat operators.
        let prec = n * 64;
        let c = BigComplex::from_f64(re, im, prec);
        for variant in [Variant::Squarings, Variant::Multiplications] {
            let (_, _, r) = ReferenceOrbitComputer::new(&c, prec).with_variant(variant).compute_vec(iters, 2.0);
            let (mut x, mut y) = (BigFloat::zero(prec), BigFloat::zero(prec));
            let mut k = 0;
            while k < iters {
                if variant == Variant::Squarings {
                    let (x2, y2) = (x.square(), y.square());
                    let s2 = (&x + &y).square();
                    let nrm = &x2 + &y2;
                    x = (&x2 - &y2) + &c.re;
                    y = (&s2 - &nrm) + &c.im;
                } else {
                    let p1 = (&x + &y) * (&x - &y);
                    let p2 = (&x * &y).times_power_of_two(1);
                    x = p1 + &c.re;
                    y = p2 + &c.im;
                }
                k += 1;
                let (xd, yd) = (x.to_f64(), y.to_f64());
                if xd * xd + yd * yd > 4.0 { break; }
            }
            prop_assert_eq!(r.iterations(), k);
            prop_assert_eq!(parts(&r.final_z.re), parts(&x));
            prop_assert_eq!(parts(&r.final_z.im), parts(&y));
        }
    }
}
