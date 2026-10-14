//! C `printf` `%g`, so the Rust tools print exactly what the Swift tools print with
//! `String(format:)` and their outputs diff 1:1.

/// `%.{precision}g`.
pub fn g(value: f64, precision: usize) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let p = precision.max(1);
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    // The exponent after rounding to p significant digits decides the style.
    let sci = format!("{:.*e}", p - 1, value);
    let (mantissa, exp) = sci.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent");
    if x < -4 || x >= p as i32 {
        let m = strip_zeros(mantissa);
        let sign = if x < 0 { '-' } else { '+' };
        format!("{m}e{sign}{:02}", x.abs())
    } else {
        let decimals = (p as i32 - 1 - x).max(0) as usize;
        strip_zeros(&format!("{value:.decimals$}")).to_string()
    }
}

fn strip_zeros(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::g;

    #[test]
    fn matches_printf() {
        assert_eq!(g(0.0, 3), "0");
        assert_eq!(g(1.0, 3), "1");
        assert_eq!(g(123456.0, 3), "1.23e+05");
        assert_eq!(g(999.6, 3), "1e+03");
        assert_eq!(g(0.0001234, 3), "0.000123");
        assert_eq!(g(0.00001234, 3), "1.23e-05");
        assert_eq!(g(2.4e-7, 3), "2.4e-07");
        assert_eq!(g(83.0, 3), "83");
        assert_eq!(g(6300.0, 3), "6.3e+03");
        assert_eq!(g(-0.7500016386523336, 17), "-0.7500016386523336");
        assert_eq!(g(1.5, 4), "1.5");
        assert_eq!(g(1e100, 3), "1e+100");
    }
}
