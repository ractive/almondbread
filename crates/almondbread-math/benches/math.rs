//! Criterion benchmarks: reference-orbit iterations/s per precision and operator
//! microbenchmarks. `cargo bench -p almondbread-math [-- <filter>]`.
//! For the table in kb synthesis/benchmarks use the `math_check` example (same workload and
//! output format as the former Swift `almondbread-math-check bench`).

use almondbread_math::*;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;

/// A C with all mantissa bits populated inside the main cardioid (as the former Swift almondbread-math-check).
fn bench_point(bits: usize) -> BigComplex {
    let mut re = BigFloat::parse("-0.50000000000000000000123456789012345678901234567890123456789", bits).unwrap();
    let im = BigFloat::parse("0.500000000000000000000987654321098765432109876543210987654321", bits).unwrap();
    re += BigFloat::from_f64(0.123456789, bits).times_power_of_two(-(bits.max(80) as i64 - 8));
    BigComplex::new(re, im)
}

fn orbit(c: &mut Criterion) {
    let mut g = c.benchmark_group("orbit");
    g.sample_size(10)
        .measurement_time(Duration::from_secs(2))
        .warm_up_time(Duration::from_millis(300));
    for bits in [64usize, 128, 256, 512, 1024, 2048, 4096, 16384] {
        let iters = (20_000_000 / (bits * bits / 4096).max(1)).clamp(200, 200_000);
        let comp = ReferenceOrbitComputer::new(&bench_point(bits), bits);
        let mut pts = vec![[0f32; 2]; iters + 1];
        let mut ext = vec![FloatExpComplex::ZERO; iters + 1];
        g.throughput(Throughput::Elements(iters as u64));
        g.bench_with_input(BenchmarkId::new("auto+out", bits), &bits, |b, _| {
            b.iter(|| {
                with_no_control(4096, |ctl| {
                    comp.compute(iters, 2.0, Some(&mut pts), Some(&mut ext), ctl)
                })
                .count
            })
        });
    }
    g.finish();
}

fn ops(c: &mut Criterion) {
    let mut g = c.benchmark_group("ops");
    g.sample_size(20)
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_millis(200));
    for bits in [128usize, 512, 4096] {
        let p = bench_point(bits);
        let (a, b) = (p.re, p.im);
        let text = a.to_string();
        g.bench_function(BenchmarkId::new("mul", bits), |bn| {
            bn.iter(|| black_box(&a) * black_box(&b))
        });
        g.bench_function(BenchmarkId::new("square", bits), |bn| {
            bn.iter(|| black_box(&a).square())
        });
        g.bench_function(BenchmarkId::new("add", bits), |bn| {
            bn.iter(|| black_box(&a) + black_box(&b))
        });
        g.bench_function(BenchmarkId::new("div", bits), |bn| {
            bn.iter(|| black_box(&a) / black_box(&b))
        });
        g.bench_function(BenchmarkId::new("format", bits), |bn| {
            bn.iter(|| black_box(&a).to_string())
        });
        g.bench_function(BenchmarkId::new("parse", bits), |bn| {
            bn.iter(|| BigFloat::parse(black_box(&text), bits))
        });
    }
    g.finish();
}

fn kernels(c: &mut Criterion) {
    let mut g = c.benchmark_group("kernels");
    g.sample_size(20)
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_millis(200));
    for n in [4usize, 8, 16, 32, 64, 256] {
        let a: Vec<u64> = (0..n as u64)
            .map(|i| i.wrapping_mul(0x9e3779b97f4a7c15) | 1 << 63)
            .collect();
        let b: Vec<u64> = (0..n as u64)
            .map(|i| i.wrapping_mul(0xbf58476d1ce4e5b9) | 1 << 63)
            .collect();
        let mut r = vec![0u64; 2 * n];
        g.bench_function(BenchmarkId::new("mul_short", n), |bn| {
            bn.iter(|| limbs::mul_short(black_box(&mut r), black_box(&a), black_box(&b), n))
        });
        g.bench_function(BenchmarkId::new("sqr_short", n), |bn| {
            bn.iter(|| limbs::sqr_short(black_box(&mut r), black_box(&a), n))
        });
        let mut s = vec![0u64; raw::raw_scratch_limbs(n)];
        let ra = raw::Raw {
            m: a.clone(),
            neg: false,
            exp: 3,
            zero: false,
        };
        let rb = raw::Raw {
            m: b.clone(),
            neg: true,
            exp: 2,
            zero: false,
        };
        let mut rr = raw::Raw {
            m: vec![0u64; n],
            neg: false,
            exp: 0,
            zero: true,
        };
        g.bench_function(BenchmarkId::new("raw_sub", n), |bn| {
            bn.iter(|| raw::add(black_box(&mut rr), ra.view(), rb.view(), false, black_box(&mut s)))
        });
    }
    g.finish();
}

criterion_group!(benches, orbit, ops, kernels);
criterion_main!(benches);
