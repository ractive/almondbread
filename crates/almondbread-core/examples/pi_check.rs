//! π in the Mandelbrot set: escape counts at the neck (c = -3/4 + iε) and the cusp
//! (c = 1/4 + ε) by the meter's compensated count, double-double, plain f64 in shifted
//! coordinates, the textbook f64 form, and a 256-bit BigFloat reference; the offset
//! N - π/ε (neck) or N - π/√ε (cusp) shows the order of the error (bounded offset: the
//! product's error is O(ε) and O(√ε)). (kb: techniques/pi-in-the-mandelbrot-set)
//!
//! cargo run --release -p almondbread-core --example pi_check [-- max_big max_dd max_count]

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Instant;

use almondbread_core::pi_moment::{
    Count, PiPoint, escape_count, escape_count_big, escape_count_dd, escape_count_f64, escape_count_naive,
};

fn arg(i: usize, default: f64) -> f64 {
    std::env::args().nth(i).and_then(|a| a.parse().ok()).unwrap_or(default)
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let v = f();
    (v, t.elapsed().as_secs_f64())
}

fn main() {
    let max_big = arg(1, 4e6);
    let max_dd = arg(2, 4e8);
    let max_count = arg(3, 4e9);
    for point in [PiPoint::Neck, PiPoint::Cusp] {
        println!(
            "{:5} {:>8} {:>11} {:>13} {:>8} {:>7} {:>11} {:>7} {:>11} {:>7} {:>11} {:>11} {:>7}",
            point.name(),
            "eps",
            "N",
            "product",
            "N-expect",
            "comp s",
            "dd",
            "dd s",
            "f64 shift",
            "f64 s",
            "f64 naive",
            "big 256",
            "big s"
        );
        let epss: Vec<f64> = match point {
            PiPoint::Neck => (1..=9).map(|k| 10f64.powi(-k)).chain([3.7e-6, 2.2e-8]).collect(),
            PiPoint::Cusp => (1..=19).map(|k| 10f64.powi(-k)).chain([3.7e-11]).collect(),
        };
        for eps in epss {
            let expected = point.expected_count(eps);
            if expected > max_count {
                continue;
            }
            let limit = (expected * 1.5) as u64 + 100;
            let (count, comp_s) =
                timed(|| escape_count(point, eps, limit, &AtomicBool::new(false), &AtomicU64::new(0)));
            let n = match count {
                Count::Escaped(n) => n,
                _ => 0,
            };
            let opt = |v: Option<u64>| v.map_or("-".to_string(), |n| n.to_string());
            let skip = || ("skip".to_string(), String::new());
            let (dd, dd_s) = if expected < max_dd {
                let (v, s) = timed(|| escape_count_dd(point, eps, limit));
                (opt(v), format!("{s:.2}"))
            } else {
                skip()
            };
            let (f, f_s) = {
                let (v, s) = timed(|| escape_count_f64(point, eps, limit));
                (opt(v), format!("{s:.2}"))
            };
            let naive = opt(escape_count_naive(point, eps, limit));
            let (big, big_s) = if expected < max_big {
                let (v, s) = timed(|| escape_count_big(point, eps, 256, limit));
                (opt(v), format!("{s:.2}"))
            } else {
                skip()
            };
            println!(
                "{:5} {:>8.1e} {:>11} {:>13.10} {:>8.3} {:>7.2} {:>11} {:>7} {:>11} {:>7} {:>11} {:>11} {:>7}",
                "",
                eps,
                n,
                point.product(n, eps),
                n as f64 - expected,
                comp_s,
                dd,
                dd_s,
                f,
                f_s,
                naive,
                big,
                big_s
            );
        }
    }
}
