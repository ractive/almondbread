//! Oracle tests, ported from Tests/AlmondbreadMathTests (OracleTests.swift, LimbTests.swift).
//!
//! Fixtures (tests/fixtures, copied from Tests/AlmondbreadMathTests/Fixtures) come from
//! oracle/generate_fixtures.py: exact Python integer arithmetic with explicit
//! round-to-nearest-even, and mpmath for nuclei. Expected values are bit-exact.

use almondbread_math::*;
use serde_json::Value;

fn fixture(name: &str) -> Vec<Value> {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str::<Value>(&text)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

fn limbs_from_hex(hex: &str) -> Vec<u64> {
    let b = hex.as_bytes();
    let mut out = Vec::new();
    let mut end = b.len();
    while end > 0 {
        let start = end.saturating_sub(16);
        out.push(u64::from_str_radix(std::str::from_utf8(&b[start..end]).unwrap(), 16).unwrap());
        end = start;
    }
    out
}

fn hex(limbs: &[u64]) -> String {
    let mut s = String::new();
    for (i, l) in limbs.iter().rev().enumerate() {
        if i == 0 {
            s.push_str(&format!("{l:x}"));
        } else {
            s.push_str(&format!("{l:016x}"));
        }
    }
    s
}

fn as_usize(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}

/// Decodes [negative, hex, exponent] (or null = zero).
fn value(v: &Value, prec: usize) -> BigFloat {
    match v.as_array() {
        None => BigFloat::zero(prec),
        Some(a) => BigFloat::from_parts(
            a[0].as_bool().unwrap(),
            &limbs_from_hex(a[1].as_str().unwrap()),
            a[2].as_i64().unwrap(),
            prec,
        ),
    }
}

#[track_caller]
fn assert_bits(v: &BigFloat, expected: &Value, prec: usize, what: &str) {
    assert_eq!(v.precision_bits(), prec, "precision of {what}");
    match expected.as_array() {
        None => assert!(v.is_zero(), "{what}: expected 0, got {v}"),
        Some(a) => {
            let (neg, sig, e) = v.exact_parts();
            let ok = !v.is_zero()
                && neg == a[0].as_bool().unwrap()
                && hex(sig) == a[1].as_str().unwrap()
                && e == a[2].as_i64().unwrap();
            assert!(ok, "{what}: got {neg} {} e{e}, expected {expected}", hex(sig));
        }
    }
}

#[test]
fn arithmetic_against_oracle() {
    let cases = fixture("arith");
    assert!(cases.len() > 1000);
    for (i, c) in cases.iter().enumerate() {
        let prec = as_usize(&c["prec"]);
        let a = value(&c["a"], prec);
        let b = value(&c["b"], prec);
        assert_bits(&(&a + &b), &c["add"], prec, &format!("case {i} add"));
        assert_bits(&(&a - &b), &c["sub"], prec, &format!("case {i} sub"));
        assert_bits(&(&a * &b), &c["mul"], prec, &format!("case {i} mul"));
        assert_bits(&a.square(), &c["sqr"], prec, &format!("case {i} sqr"));
        assert_bits(&(&a / &b), &c["div"], prec, &format!("case {i} div"));
        let small = c["small"].as_i64().unwrap();
        assert_bits(&a.mul_int(small), &c["mulSmall"], prec, &format!("case {i} mulSmall"));
        let mut x = a.clone();
        x += &b;
        assert_bits(&x, &c["add"], prec, &format!("case {i} +="));
        let mut y = a.clone();
        y *= &b;
        assert_bits(&y, &c["mul"], prec, &format!("case {i} *="));
        if let Some(bits) = c["doubleBits"].as_str() {
            assert_eq!(
                a.to_f64().to_bits(),
                u64::from_str_radix(bits, 16).unwrap(),
                "case {i} double"
            );
        }
    }
}

#[test]
fn parse_against_oracle() {
    for c in fixture("parse") {
        let prec = as_usize(&c["prec"]);
        let text = c["text"].as_str().unwrap();
        let v = BigFloat::parse(text, prec).unwrap_or_else(|| panic!("parse {text}"));
        assert_bits(&v, &c["value"], prec, &format!("parse {text} @{prec}"));
    }
}

#[test]
fn format_against_oracle() {
    for c in fixture("format") {
        let prec = as_usize(&c["prec"]);
        let v = value(&c["value"], prec);
        let digits = c["digits"].as_u64().map(|d| d as usize);
        assert_eq!(v.to_decimal_string(digits), c["text"].as_str().unwrap());
    }
}

#[test]
fn decimal_round_trip() {
    for c in fixture("arith").iter().take(400) {
        let prec = as_usize(&c["prec"]);
        for key in ["a", "mul", "div"] {
            let v = value(&c[key], prec);
            let s = v.to_string();
            let back = BigFloat::parse(&s, prec).unwrap();
            assert!(back == v && back.exact_parts().1 == v.exact_parts().1, "round trip {s}");
        }
    }
}

#[test]
fn malformed_strings() {
    for s in ["", "-", ".", "1e", "1.2.3", "abc", "1e+", "--1", "1 2"] {
        assert!(BigFloat::parse(s, 128).is_none(), "{s:?}");
    }
    assert_eq!(BigFloat::parse(" -0.5e1 ", 64).unwrap().to_f64(), -5.0);
    assert_eq!(BigFloat::parse(".25", 64).unwrap().to_f64(), 0.25);
}

#[test]
fn div_and_sqrt() {
    let a = BigFloat::parse("2", 512).unwrap();
    let r = a.sqrt();
    assert!((&r * &r - &a).log2_magnitude() < -505.0);
    let third = BigFloat::from_i64(1, 256) / BigFloat::from_i64(3, 256);
    assert_eq!(third.to_decimal_string(Some(30)), "0.333333333333333333333333333333");
    assert_eq!(
        BigFloat::parse("1e-1000", 4096).unwrap().to_decimal_string(Some(5)),
        "1e-1000"
    );
}

#[test]
fn float_exp_conversions() {
    let v = BigFloat::parse("-3.0e-1000", 256).unwrap();
    let fe = v.float_exp();
    assert!(fe.mantissa < 0.0);
    assert_eq!(fe.exponent as f64, v.log2_magnitude().floor() + 1.0);
    assert!(fe.mantissa.abs() >= 0.5 && fe.mantissa.abs() < 1.0);
    assert!((v.double_exp().log2_magnitude() - v.log2_magnitude()).abs() < 1e-9);
    assert_eq!(
        BigFloat::from_f64(0.75, 64).float_exp(),
        FloatExp {
            mantissa: 0.75,
            exponent: 0
        }
    );
    assert_eq!(BigFloat::from_f64(-6.0, 64).double_exp(), DoubleExp::new(-0.75, 3));
    let z = BigComplex::new(
        BigFloat::parse("1e-400", 192).unwrap(),
        BigFloat::parse("-2e-400", 192).unwrap(),
    );
    let fc = z.float_exp_complex();
    assert!(fc.mantissa[0].abs().max(fc.mantissa[1].abs()) >= 0.5);
    assert!(((fc.mantissa[0] / fc.mantissa[1]) as f64 + 0.5).abs() < 1e-6);
    assert_eq!(BigFloat::from_f64(1.5, 64).to_f64(), 1.5);
    assert_eq!(BigFloat::from_f64(-5e-324, 64).to_f64(), -5e-324);
}

// ---------------------------------------------------------------------------------------
// Orbits

#[test]
fn reference_orbit_bit_exact() {
    for c in fixture("orbit") {
        let prec = as_usize(&c["prec"]);
        let iters = as_usize(&c["iterations"]);
        let (re, im) = (c["re"].as_str().unwrap(), c["im"].as_str().unwrap());
        let center = BigComplex::parse(re, im, prec).unwrap();
        for (variant, key) in [
            (Variant::Squarings, "squarings"),
            (Variant::Multiplications, "multiplications"),
        ] {
            let comp = ReferenceOrbitComputer::new(&center, prec).with_variant(variant);
            let (pts, ext, r) = comp.compute_vec(iters, 2.0);
            assert_eq!(
                r.iterations(),
                as_usize(&c[format!("{key}Count")]),
                "{re} {key} iterations"
            );
            let z = c[key].as_array().unwrap();
            assert_bits(&r.final_z.re, &z[0], prec, &format!("{re} @{prec} {key} re"));
            assert_bits(&r.final_z.im, &z[1], prec, &format!("{re} @{prec} {key} im"));
            // Float outputs agree with the full-precision final point.
            let last = pts[r.count - 1];
            let (fre, fim) = (r.final_z.re.to_f64(), r.final_z.im.to_f64());
            assert!((last[0] - fre as f32).abs() <= last[0].abs() * 1e-6 + 1e-30);
            assert!((last[1] - fim as f32).abs() <= last[1].abs() * 1e-6 + 1e-30);
            let e = ext[r.count - 1].to_f64();
            assert!((e[0] - fre).abs() <= e[0].abs() * 1e-6 + 1e-300);
            assert!((e[1] - fim).abs() <= e[0].abs().max(e[1].abs()) * 1e-6 + 1e-300);
            let high = c["high"].as_array().unwrap();
            let hre = BigFloat::parse(high[0].as_str().unwrap(), prec + 256).unwrap();
            let him = BigFloat::parse(high[1].as_str().unwrap(), prec + 256).unwrap();
            let err = (&r.final_z.re - &hre)
                .log2_magnitude()
                .max((&r.final_z.im - &him).log2_magnitude());
            println!(
                "orbit {re} @{prec} bits {key}: {} iters, log2|error vs high| = {err}",
                r.iterations()
            );
        }
    }
}

#[test]
fn orbit_buffers_and_cancellation() {
    let c = BigComplex::from_f64(-0.1, 0.1, 256);
    let comp = ReferenceOrbitComputer::new(&c, 256);
    let mut polls = 0;
    let mut progress_calls = Vec::new();
    let mut pts = vec![[0.0f32; 2]; 100_001];
    let mut is_cancelled = || {
        polls += 1;
        polls >= 3
    };
    let mut progress = |d: usize| progress_calls.push(d);
    let mut control = Control {
        check_interval: 1000,
        is_cancelled: &mut is_cancelled,
        progress: &mut progress,
    };
    let r = comp.compute(100_000, 2.0, Some(&mut pts), None, &mut control);
    assert!(r.cancelled);
    assert_eq!(r.iterations(), 3000);
    assert_eq!(progress_calls, vec![1000, 2000, 3000]);
    let (mut x, mut y) = (0.0f64, 0.0f64);
    for p in pts.iter().skip(1).take(20) {
        (x, y) = (x * x - y * y - 0.1, 2.0 * x * y + 0.1);
        assert!((p[0] - x as f32).abs() < 1e-6);
        assert!((p[1] - y as f32).abs() < 1e-6);
    }
}

#[test]
fn escape_stops_orbit() {
    let c = BigComplex::from_f64(0.5, 0.5, 128);
    let (pts, ext, r) = ReferenceOrbitComputer::new(&c, 128).compute_vec(1000, 2.0);
    assert!(r.escaped);
    assert_eq!(pts.len(), r.count);
    assert_eq!(ext.len(), r.count);
    let last = pts.last().unwrap();
    assert!(((last[0] as f64).powi(2) + (last[1] as f64).powi(2)).sqrt() > 2.0);
    assert!(r.iterations() < 10);
}

#[test]
fn precision_rule() {
    // 1e-100 spacing, 2048 px: 333 + 11 + 2 + 48 -> 448 bits (7 limbs).
    assert_eq!(
        recommended_precision_bits(-100.0 * std::f64::consts::LOG2_10, 2048),
        448
    );
    assert_eq!(recommended_precision_bits(-10.0, 1000), 128);
}

// ---------------------------------------------------------------------------------------
// Nuclei and reference selection

fn never() -> impl FnMut() -> bool {
    || false
}

#[test]
fn nuclei_against_mpmath() {
    let cases = fixture("nuclei");
    assert!(!cases.is_empty());
    for c in cases {
        let prec = as_usize(&c["prec"]);
        let p = as_usize(&c["period"]);
        let start = BigComplex::parse(c["re"].as_str().unwrap(), c["im"].as_str().unwrap(), prec).unwrap();
        let radius = BigFloat::parse(c["radius"].as_str().unwrap(), 64).unwrap();
        let found = find_period(&start, &radius, 200_000, PeriodMethod::Ball, 4096, &mut never());
        assert_eq!(
            found,
            c["ballPeriod"].as_u64().map(|v| v as usize),
            "ball period at {}",
            c["re"]
        );
        let nr = find_nucleus(&start, p, &NewtonOptions::default(), &mut never())
            .unwrap_or_else(|| panic!("newton {}", c["re"]));
        let ere = BigFloat::parse(c["nucleusRe"].as_str().unwrap(), prec + 128).unwrap();
        let eim = BigFloat::parse(c["nucleusIm"].as_str().unwrap(), prec + 128).unwrap();
        let err = (&nr.nucleus.re - &ere)
            .log2_magnitude()
            .max((&nr.nucleus.im - &eim).log2_magnitude());
        println!(
            "nucleus p={p} @{prec} bits: {} Newton steps, log2|error| = {err}",
            nr.steps
        );
        assert!(err < -(prec as f64) + 24.0, "nucleus accuracy p={p}");
        let (pts, ext, orbit) = ReferenceOrbitComputer::new(&nr.nucleus, prec).compute_vec(p, 2.0);
        assert_eq!(orbit.iterations(), p);
        let z_log2 = orbit.final_z.re.log2_magnitude().max(orbit.final_z.im.log2_magnitude());
        assert!(z_log2 < -(prec as f64) + 2.0 * p as f64 + 24.0, "Z_p ~ 0 for p={p}");
        if !orbit.final_z.re.is_zero() && z_log2 < -200.0 {
            assert_eq!(pts[p], [0.0, 0.0]);
            assert!((ext[p].exponent as f64 - (z_log2.floor() + 1.0)).abs() <= 1.0);
        }
    }
}

#[test]
fn known_real_nuclei() {
    let opts = NewtonOptions::default();
    let p3 = find_nucleus(&BigComplex::from_f64(-1.75, 0.0, 128), 3, &opts, &mut never()).unwrap();
    assert!(
        p3.nucleus
            .re
            .to_decimal_string(Some(17))
            .starts_with("-1.754877666246692")
    );
    assert!(p3.nucleus.im.is_zero() || p3.nucleus.im.log2_magnitude() < -120.0);
    let p4 = find_nucleus(&BigComplex::from_f64(-1.94, 0.0, 128), 4, &opts, &mut never()).unwrap();
    assert!(
        p4.nucleus
            .re
            .to_decimal_string(Some(17))
            .starts_with("-1.940799806529484")
    );
    let (_, _, r) = ReferenceOrbitComputer::new(&p4.nucleus, 128).compute_vec(4, 2.0);
    assert!(r.final_z.re.log2_magnitude().max(r.final_z.im.log2_magnitude()) < -110.0);
}

#[test]
fn minibrot_size_estimate() {
    let s2 = minibrot_size(&BigComplex::from_f64(-1.0, 0.0, 128), 2).to_f64();
    assert!((s2 - 0.5).abs() < 1e-12);
    let p3 = find_nucleus(
        &BigComplex::from_f64(-1.75, 0.0, 128),
        3,
        &NewtonOptions::default(),
        &mut never(),
    )
    .unwrap();
    let size = minibrot_size(&p3.nucleus, 3).to_f64();
    println!("period-3 minibrot size estimate {size}");
    assert!(size > 0.01 && size < 0.03);
}

#[test]
fn box_and_first_order_periods() {
    let c = BigComplex::from_f64(-1.75, 0.001, 128);
    let r = BigFloat::from_f64(0.01, 64);
    for m in [PeriodMethod::Box, PeriodMethod::BallFirstOrder, PeriodMethod::Ball] {
        assert_eq!(find_period(&c, &r, 1000, m, 4096, &mut never()), Some(3), "{m:?}");
    }
}

#[test]
fn atom_domain() {
    // Inside the period-3 bulb near its nucleus the last new minimum of |z_n| repeats every 3.
    let c = BigComplex::from_f64(-1.7548, 0.0, 128);
    assert_eq!(atom_domain_period(&c, 30) % 3, 0);
}

#[test]
fn reference_selector() {
    let view = BigComplex::from_f64(-1.752, 0.0005, 128);
    let choice = select_reference(
        &view,
        &BigFloat::from_f64(0.01, 64),
        1000,
        None,
        PeriodMethod::Ball,
        &mut never(),
    );
    assert!(choice.is_nucleus);
    assert_eq!(choice.period, Some(3));
    let out = BigComplex::from_f64(0.5, 0.5, 128);
    let c2 = select_reference(
        &out,
        &BigFloat::from_f64(1e-6, 64),
        1000,
        None,
        PeriodMethod::Ball,
        &mut never(),
    );
    assert!(!c2.is_nucleus);
    assert_eq!(c2.point, out);
}
