//! Extreme-value smoke tests (kb: techniques/hang-prevention): absurd inputs return quickly
//! with something sane. A few calls each; milliseconds in all.

use crate::BigFloat;
use crate::floatexp::{exponent_significand, log2_unit};
use crate::orbit::recommended_precision_bits_with;

#[test]
fn extreme_parse_rejects_absurd_exponents_quickly() {
    let t0 = std::time::Instant::now();
    // 10^999999999 exactly took minutes (a hang when such text was pasted into the app).
    assert!(BigFloat::parse("1e-999999999", 128).is_none());
    assert!(BigFloat::parse("1e999999999", 128).is_none());
    assert!(BigFloat::parse("0.0001e-99999", 128).is_none());
    // Zero keeps any exponent; the deepest views still parse.
    assert!(BigFloat::parse("0e-999999999", 128).unwrap().is_zero());
    let deep = BigFloat::parse("2.5e-3010", 128).unwrap();
    assert!((deep.log2_magnitude() / std::f64::consts::LOG2_10 + 3009.6).abs() < 0.1);
    assert!(BigFloat::parse("1e100000", 64).is_some());
    assert!(t0.elapsed().as_secs_f64() < 0.5, "{:?}", t0.elapsed());
}

#[test]
fn extreme_precision_bits_are_bounded() {
    for s in [f64::NEG_INFINITY, f64::INFINITY, f64::NAN, -1e300, 1e300, -1e18] {
        for c in [f64::NAN, f64::INFINITY, 0.0, -1.0, 1e300] {
            let b = recommended_precision_bits_with(s, 0, c, usize::MAX / 2);
            assert!((64..=1 << 20).contains(&b), "{s} {c}: {b}");
            assert_eq!(b % 64, 0);
        }
    }
}

#[test]
fn extreme_float_decomposition_terminates() {
    // Zero used to recurse forever through the subnormal branch.
    assert_eq!(exponent_significand(0.0), (0, 0.0));
    assert_eq!(exponent_significand(-0.0), (0, 0.0));
    let (e, s) = exponent_significand(f64::from_bits(1)); // smallest subnormal
    assert_eq!((e, s), (-1074, 1.0));
    for x in [0.5, 1.0, f64::NAN, f64::INFINITY, 0.0, -1.0] {
        let _ = log2_unit(x);
    }
}
