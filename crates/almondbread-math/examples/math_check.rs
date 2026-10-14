//! Orbit and operator benchmarks (the workloads and output of the former Swift
//! `almondbread-math-check`, deleted in R2b, so older records compare 1:1):
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
fn bench_orbit(bits: usize, variant: Variant, outputs: bool, threads: Threads) -> f64 {
    let min_t: f64 = std::env::var("MIN_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.4);
    let best_of: usize = std::env::var("BEST_OF").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let comp = ReferenceOrbitComputer::new(&bench_point(bits), bits)
        .with_variant(variant)
        .with_threads(threads);
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
    println!("Reference orbit, iterations per second (release; sqr3 = squarings on three threads)");
    println!(
        "{:>6} {:>12} {:>12} {:>12} {:>12} {:>12} {:>7}",
        "bits", "sqr+out", "mul+out", "sqr", "mul", "sqr3+out", "x"
    );
    let one = Threads::Single;
    for &bits in sizes {
        let a = bench_orbit(bits, Variant::Squarings, true, one);
        let b = bench_orbit(bits, Variant::Multiplications, true, one);
        let c = bench_orbit(bits, Variant::Squarings, false, one);
        let d = bench_orbit(bits, Variant::Multiplications, false, one);
        let e = bench_orbit(bits, Variant::Squarings, true, Threads::Three);
        println!(
            "{bits:>6} {:>12} {:>12} {:>12} {:>12} {:>12} {:>7.2}",
            a as u64,
            b as u64,
            c as u64,
            d as u64,
            e as u64,
            e / a.max(b)
        );
    }
}

/// Single-threaded (fastest form) against three threads, interleaved `ROUNDS` times (env,
/// default 7) so load from other processes hits both alike; prints best and median.
fn run_par(sizes: &[usize]) {
    let rounds: usize = std::env::var("ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(7);
    println!("Reference orbit with outputs, iterations per second: best / median of {rounds}");
    println!("{:>6} {:>21} {:>21} {:>13}", "bits", "1 thread", "3 threads", "x best/median");
    for &bits in sizes {
        let single_variant = if bits <= 64 * AUTOMATIC_MULTIPLICATION_LIMIT {
            Variant::Multiplications
        } else {
            Variant::Squarings
        };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for _ in 0..rounds {
            a.push(bench_orbit(bits, single_variant, true, Threads::Single));
            b.push(bench_orbit(bits, Variant::Squarings, true, Threads::Three));
        }
        let stats = |v: &mut Vec<f64>| {
            v.sort_by(|x, y| y.partial_cmp(x).unwrap());
            (v[0], v[v.len() / 2])
        };
        let ((ab, am), (bb, bm)) = (stats(&mut a), stats(&mut b));
        println!(
            "{bits:>6} {:>10} {:>10} {:>10} {:>10} {:>6.2} {:>6.2}",
            ab as u64,
            am as u64,
            bb as u64,
            bm as u64,
            bb / ab,
            bm / am
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
        "par" => {
            let sizes: Vec<usize> = if args.len() > 1 {
                args[1..].iter().map(|s| s.parse().unwrap()).collect()
            } else {
                vec![768, 1024, 1152, 1536, 2048, 3456, 4096, 6784, 10048]
            };
            run_par(&sizes)
        }
        "ops" => run_ops(),
        "nucleus" => run_nucleus(),
        _ => println!("usage: math_check [bench [bits...]|par [bits...]|ops|nucleus]"),
    }
}
