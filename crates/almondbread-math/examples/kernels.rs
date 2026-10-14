//! Kernel microbenchmarks (best-of timing): ns per short square / short product / add.
use almondbread_math::{limbs, raw};
use std::hint::black_box;
use std::time::Instant;

#[inline(never)]
fn add_probe(r: &mut raw::Raw<Vec<u64>>, a: raw::View<'_>, b: raw::View<'_>, s: &mut [u64]) {
    raw::add(r, a, b, false, s)
}

fn best(reps: usize, mut f: impl FnMut()) -> f64 {
    let mut b = f64::MAX;
    for _ in 0..15 {
        let t0 = Instant::now();
        for _ in 0..reps {
            f();
        }
        b = b.min(t0.elapsed().as_secs_f64() / reps as f64);
    }
    b * 1e9
}

fn main() {
    for n in [4usize, 8, 12, 16, 24, 32, 48, 64, 128, 256] {
        let a: Vec<u64> = (0..n as u64)
            .map(|i| i.wrapping_mul(0x9e3779b97f4a7c15) | 1 << 63)
            .collect();
        let b: Vec<u64> = (0..n as u64)
            .map(|i| i.wrapping_mul(0xbf58476d1ce4e5b9) | 1 << 63)
            .collect();
        let mut r = vec![0u64; 2 * n];
        let reps = 2_000_000 / (n * n);
        let ts = best(reps, || limbs::sqr_short(black_box(&mut r), black_box(&a), n));
        let tm = best(reps, || {
            limbs::mul_short(black_box(&mut r), black_box(&a), black_box(&b), n)
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
        let tadd = best(reps * 4, || {
            add_probe(black_box(&mut rr), ra.view(), rb.view(), black_box(&mut s))
        });
        let trs = best(reps, || raw::sqr(black_box(&mut rr), ra.view(), black_box(&mut s)));
        println!(
            "n={n:>3}: sqr_short {ts:8.1} ns  mul_short {tm:8.1} ns  raw add {tadd:6.1} ns  raw sqr {trs:8.1} ns  ({:.2} ns/product sqr, {:.2} mul)",
            ts / ((n * n) as f64 / 4.0),
            tm / ((n * n) as f64 / 2.0)
        );
    }
}
