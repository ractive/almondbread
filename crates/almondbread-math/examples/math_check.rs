//! Rust twin of `swift run -c release almondbread-math-check`, same workloads and output:
//!
//!   cargo run --release -p almondbread-math --example math_check -- bench [bits...]
//!   cargo run --release -p almondbread-math --example math_check -- ops
//!   cargo run --release -p almondbread-math --example math_check -- nucleus
//!
//! `bench` prints reference-orbit iterations per second per precision (single thread).

use almondbread_math::*;
use std::hint::black_box;
use std::time::Instant;

/// A C with all mantissa bits populated, inside the main cardioid (no escape).
fn bench_point(bits: usize) -> BigComplex {
    let mut re = BigFloat::parse("-0.50000000000000000000123456789012345678901234567890123456789", bits).unwrap();
    let im = BigFloat::parse("0.500000000000000000000987654321098765432109876543210987654321", bits).unwrap();
    re += BigFloat::from_f64(0.123456789, bits).times_power_of_two(-(bits.max(80) as i64 - 8));
    BigComplex::new(re, im)
}

/// Iterations per second: calibrate to ~`MIN_SECONDS`, then the best of `BEST_OF` runs
/// (env vars; defaults 0.4 s and 1 = the Swift tool's method). Best-of-N is robust to a
/// loaded machine.
fn bench_orbit(bits: usize, variant: Variant, outputs: bool) -> f64 {
    let min_t: f64 = std::env::var("MIN_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.4);
    let best_of: usize = std::env::var("BEST_OF").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let comp = ReferenceOrbitComputer::new(&bench_point(bits), bits).with_variant(variant);
    let run = |iters: usize| -> f64 {
        let mut pts = vec![[0f32; 2]; if outputs { iters + 1 } else { 0 }];
        let mut ext = vec![FloatExpComplex::ZERO; if outputs { iters + 1 } else { 0 }];
        let t0 = Instant::now();
        let r = with_no_control(4096, |c| {
            comp.compute(
                iters,
                2.0,
                if outputs { Some(&mut pts) } else { None },
                if outputs { Some(&mut ext) } else { None },
                c,
            )
        });
        let t = t0.elapsed().as_secs_f64();
        assert!(!r.escaped && r.iterations() == iters, "benchmark orbit stopped early");
        t
    };
    let mut iters = 1000usize;
    loop {
        let t = run(iters);
        if t > min_t || iters > 50_000_000 {
            let mut best = t;
            for _ in 1..best_of {
                best = best.min(run(iters));
            }
            return iters as f64 / best;
        }
        iters *= 4;
    }
}

fn run_bench(sizes: &[usize]) {
    println!("Reference orbit, iterations per second (release, single thread)");
    println!(
        "{:>6} {:>12} {:>12} {:>12} {:>12}",
        "bits", "sqr+out", "mul+out", "sqr", "mul"
    );
    for &bits in sizes {
        let a = bench_orbit(bits, Variant::Squarings, true);
        let b = bench_orbit(bits, Variant::Multiplications, true);
        let c = bench_orbit(bits, Variant::Squarings, false);
        let d = bench_orbit(bits, Variant::Multiplications, false);
        println!(
            "{bits:>6} {:>12} {:>12} {:>12} {:>12}",
            a as u64, b as u64, c as u64, d as u64
        );
    }
}

fn secs(body: impl FnOnce()) -> f64 {
    let t0 = Instant::now();
    body();
    t0.elapsed().as_secs_f64()
}

fn run_ops() {
    for bits in [128usize, 512, 4096] {
        let p = bench_point(bits);
        let (a, b) = (p.re, p.im);
        let mut x = a.clone();
        let n = 200_000 / (bits / 128).max(1);
        let tm = secs(|| {
            for _ in 0..n {
                x = black_box(&a) * black_box(&b);
            }
        });
        let ta = secs(|| {
            for _ in 0..n {
                x = black_box(&a) + black_box(&b);
            }
        });
        let td = secs(|| {
            for _ in 0..n / 10 {
                x = black_box(&a) / black_box(&b);
            }
        });
        let tf = secs(|| {
            for _ in 0..n / 100 {
                black_box(x.to_string());
            }
        });
        let s = a.to_string();
        let tp = secs(|| {
            for _ in 0..n / 100 {
                x = BigFloat::parse(black_box(&s), bits).unwrap();
            }
        });
        black_box(&x);
        println!(
            "{bits} bits: mul {} ns, add {} ns, div {} ns, format {} us, parse {} us (allocating operators)",
            (tm / n as f64 * 1e9) as u64,
            (ta / n as f64 * 1e9) as u64,
            (td / (n / 10) as f64 * 1e9) as u64,
            (tf / (n / 100) as f64 * 1e6) as u64,
            (tp / (n / 100) as f64 * 1e6) as u64
        );
    }
}

fn run_nucleus() {
    let bits = 192;
    let center = BigComplex::parse(
        "-0.743643887037158704752191506114774",
        "0.131825904205311970493132056385139",
        bits,
    )
    .unwrap();
    for radius in ["1e-10", "1e-20", "1e-25", "1e-32"] {
        let r = BigFloat::parse(radius, 64).unwrap();
        let t0 = Instant::now();
        let c = select_reference(&center, &r, 100_000, Some(bits), PeriodMethod::Ball, &mut || false);
        let t = t0.elapsed().as_secs_f64();
        let opt = |v: Option<usize>| v.map_or("-".to_string(), |p| p.to_string());
        println!(
            "radius {radius}: nucleus={} period={} detected={} steps={} size={} in {:.3} s",
            c.is_nucleus,
            opt(c.period),
            opt(c.detected_period),
            c.newton_steps,
            c.minibrot_size.map_or("-".into(), |s| s.to_string()),
            t
        );
        if c.is_nucleus {
            println!(
                "  re {}\n  im {}",
                c.point.re.to_decimal_string(Some(45)),
                c.point.im.to_decimal_string(Some(45))
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str).unwrap_or("bench") {
        "bench" => {
            let sizes: Vec<usize> = if args.len() > 1 {
                args[1..].iter().map(|s| s.parse().unwrap()).collect()
            } else {
                vec![64, 128, 192, 256, 384, 512, 768, 1024, 2048, 4096, 8192, 16384]
            };
            run_bench(&sizes)
        }
        "ops" => run_ops(),
        "nucleus" => run_nucleus(),
        _ => println!("usage: math_check [bench [bits...]|ops|nucleus]"),
    }
}
