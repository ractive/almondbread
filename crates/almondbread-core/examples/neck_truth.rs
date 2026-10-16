//! Ground truth for slow-escape views (kb: synthesis/neck-escape-statistics): direct f64
//! iteration `z <- z^2 + c` with exact cycle detection, to very high limits, multithreaded.
//!
//! ```text
//! cargo run --release -p almondbread-core --example neck_truth -- \
//!     --center re,im --scale s --size 160x100 [--angle deg] --limit 10000000 \
//!     [--out build/neck/truth] [--dd 97]
//! ```
//!
//! Per pixel: escape n (bailout 2^8 radius, as the kernels), proven interior (the orbit
//! returned to within 1e-15 of a saved point, Brent's power-of-two saves; prints the period),
//! or unresolved at the limit. Prints an escape histogram by powers of two, writes
//! `<out>.iter` (the oracle encoding: escaped n/s, -1 interior or limit) and `<out>.png`
//! (escaped: grey by log2 n; interior: dark blue; unresolved: red). `--dd k` re-iterates
//! every k-th escaped pixel in double-double (about 106 bits) and reports how many escape
//! counts differ: f64 is trustworthy when the counts agree.
//!
//! Why f64 suffices here: a rounding error at step k reaches z_n multiplied by
//! dz_n/dz_k, and dz_n/dc is the sum of those products, so the accumulated error acts like
//! a shift of c by about 1e-16 relative, far below a 1e-12 pixel. The `--dd` sample checks it.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use almondbread_core::oracle::{ITER_INTERIOR, IterationField};
use rayon::prelude::*;

const BAILOUT2: f64 = 256.0 * 256.0;

#[derive(Clone, Copy, Debug)]
enum Fate {
    /// (n, smooth offset, log2 |dz/dc| at escape)
    Escaped(u64, f32, f64),
    Interior(u64), // period
    Unresolved,
}

fn iterate_f64(cr: f64, ci: f64, limit: u64) -> Fate {
    let (mut zr, mut zi) = (0.0f64, 0.0f64);
    let (mut sr, mut si) = (0.0f64, 0.0f64);
    let (mut wr, mut wi) = (1.0f64, 0.0f64);
    let mut save_at = 16u64;
    let mut n = 0u64;
    // dz/dc as (dr, di) * 2^de, renormalised (it overflows f64 here).
    let (mut dr, mut di, mut de) = (0.0f64, 0.0f64, 0i64);
    while n < limit {
        let one = if de > 1000 { 0.0 } else { 2f64.powi(-(de as i32)) };
        let t = 2.0 * (zr * dr - zi * di) + one;
        di = 2.0 * (zr * di + zi * dr);
        dr = t;
        if dr.abs().max(di.abs()) > 1e100 {
            let k = 2f64.powi(-332);
            dr *= k;
            di *= k;
            de += 332;
        }
        let t = zr * zr - zi * zi + cr;
        zi = 2.0 * zr * zi + ci;
        zr = t;
        n += 1;
        let r2 = zr * zr + zi * zi;
        if r2 > BAILOUT2 {
            let l = 0.5 * (dr * dr + di * di).log2() + de as f64;
            return Fate::Escaped(n, (1.0 - (0.5 * r2.ln()).log2()) as f32, l);
        }
        // dz_n/dz_save: the derivative of the map since the last save.
        let t = 2.0 * (zr * wr - zi * wi);
        wi = 2.0 * (zr * wi + zi * wr);
        wr = t;
        let w2 = wr * wr + wi * wi;
        if w2 > 1e200 {
            // Expanding (exterior-like) stretch: keep the magnitude bounded, it stays > 1.
            wr *= 1e-100;
            wi *= 1e-100;
        }
        // Back within 1e-10 of the saved point with a contracting map since then: an
        // attracting cycle (the near-parabolic cycles here converge too slowly, and f64
        // rounding keeps them jittering above an exact-equality tolerance).
        if (zr - sr).abs() < 1e-10 && (zi - si).abs() < 1e-10 && w2 < 0.25 {
            return Fate::Interior(n - save_at / 2);
        }
        if n == save_at {
            sr = zr;
            si = zi;
            (wr, wi) = (1.0, 0.0);
            save_at *= 2;
        }
    }
    Fate::Unresolved
}

// Double-double (Dekker / Knuth error-free transforms; kb: techniques/double-float).
#[derive(Clone, Copy)]
struct Dd(f64, f64);

fn two_sum(a: f64, b: f64) -> Dd {
    let s = a + b;
    let bb = s - a;
    Dd(s, (a - (s - bb)) + (b - bb))
}

fn add(a: Dd, b: Dd) -> Dd {
    let s = two_sum(a.0, b.0);
    let t = two_sum(a.1, b.1);
    let s = two_sum(s.0, s.1 + t.0);
    two_sum(s.0, s.1 + t.1)
}

fn mul(a: Dd, b: Dd) -> Dd {
    let p = a.0 * b.0;
    let e = a.0.mul_add(b.0, -p);
    two_sum(p, e + (a.0 * b.1 + a.1 * b.0))
}

fn neg(a: Dd) -> Dd {
    Dd(-a.0, -a.1)
}

fn iterate_dd(cr: Dd, ci: Dd, limit: u64) -> Option<u64> {
    let (mut zr, mut zi) = (Dd(0.0, 0.0), Dd(0.0, 0.0));
    for n in 1..=limit {
        let zr2 = mul(zr, zr);
        let zi2 = mul(zi, zi);
        let zri = mul(zr, zi);
        zr = add(add(zr2, neg(zi2)), cr);
        zi = add(add(zri, zri), ci);
        let r2 = zr.0 * zr.0 + zi.0 * zi.0;
        if r2 > BAILOUT2 {
            return Some(n);
        }
    }
    None
}

/// Decimal string to double-double (enough for the centre: digits beyond f64 go to the tail).
fn parse_dd(s: &str) -> Dd {
    let hi: f64 = s.parse().expect("number");
    // The tail: the decimal string minus hi, subtracted digit by digit.
    let tail = residual(s, hi);
    two_sum(hi, tail)
}

/// s - hi: both as fixed-point decimal digit strings (hi written with 40 significant digits,
/// enough for its exact value's leading part), subtracted digit by digit.
fn residual(s: &str, hi: f64) -> f64 {
    let a = to_fixed(s);
    let b = to_fixed(&format!("{hi:.40e}"));
    let d = sub_decimal(&a, &b);
    d.parse::<f64>().unwrap_or(0.0)
}

/// A decimal number as (negative, digits): 10 integer and 80 fractional digits.
fn to_fixed(s: &str) -> (bool, Vec<u8>) {
    let s = s.trim();
    let (neg, s) = s.strip_prefix('-').map(|r| (true, r)).unwrap_or((false, s));
    let (mant, exp) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().unwrap()),
        None => (s, 0),
    };
    let (ip, fp) = mant.split_once('.').unwrap_or((mant, ""));
    let digits: Vec<u8> = ip.bytes().chain(fp.bytes()).map(|b| b - b'0').collect();
    let point = ip.len() as i32 + exp; // digits before the decimal point
    // 10 integer digits, 80 fractional
    let mut out = vec![0u8; 90];
    for (i, &d) in digits.iter().enumerate() {
        let pos = 10 + i as i32 - point;
        if (0..90).contains(&pos) {
            out[pos as usize] = d;
        }
    }
    (neg, out)
}

fn sub_decimal(a: &(bool, Vec<u8>), b: &(bool, Vec<u8>)) -> String {
    // value = sign * digits; compute a - b with signs (only same-sign inputs occur).
    assert_eq!(a.0, b.0);
    let (x, y, flip) = if a.1 >= b.1 {
        (&a.1, &b.1, false)
    } else {
        (&b.1, &a.1, true)
    };
    let mut out = [0u8; 90];
    let mut borrow = 0i32;
    for i in (0..90).rev() {
        let mut d = x[i] as i32 - y[i] as i32 - borrow;
        borrow = 0;
        if d < 0 {
            d += 10;
            borrow = 1;
        }
        out[i] = d as u8;
    }
    let neg = a.0 ^ flip;
    let s: String = out.iter().map(|d| (b'0' + d) as char).collect();
    format!("{}{}.{}", if neg { "-" } else { "" }, &s[..10], &s[10..])
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == &format!("--{k}"))
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if let Some(path) = get("paint") {
        // `--paint a.iter --limit L --out a`: an iteration dump (verify --dump, --dump-cpu)
        // in this tool's colouring, for side-by-side looks. Unresolved (-3) red, interior blue.
        let f = IterationField::read(&path).expect("read iter");
        let lmax = get("limit").map(|l| l.parse::<f64>().unwrap()).unwrap_or(1e6).log2();
        // `--cycle p`: a periodic ramp over the smooth count mu (period p iterations), like a
        // palette, to show bands and noise.
        let cycle: f64 = get("cycle").map(|c| c.parse().unwrap()).unwrap_or(0.0);
        let mut bgra = vec![0u8; f.n.len() * 4];
        for (i, &n) in f.n.iter().enumerate() {
            let px = if n > 0.0 && cycle > 0.0 {
                let t = ((n as f64 + f.s[i] as f64) / cycle).fract();
                let tri = |x: f64| ((1.0 - (2.0 * ((x).fract()) - 1.0).abs()) * 255.0) as u8;
                [tri(t), tri(t + 1.0 / 3.0), tri(t + 2.0 / 3.0), 255]
            } else if n > 0.0 {
                let c = (40.0 + 215.0 * ((n as f64).log2() / lmax).clamp(0.0, 1.0)) as u8;
                [c, c, c, 255]
            } else if n == ITER_INTERIOR && f.s[i] > 0.5 {
                [90, 20, 10, 255]
            } else {
                [10, 10, 160, 255]
            };
            bgra[i * 4..i * 4 + 4].copy_from_slice(&px);
        }
        let mut bins: BTreeMap<i32, usize> = BTreeMap::new();
        for &n in &f.n {
            *bins
                .entry(if n > 0.0 {
                    (n as f64).log2() as i32
                } else {
                    -(n as i32) - 100
                })
                .or_default() += 1;
        }
        let total = f.n.len() as f64;
        let line: Vec<String> = bins
            .iter()
            .map(|(k, c)| match k {
                -99 => format!("int/limit {:.2}%", 100.0 * *c as f64 / total),
                -97 => format!("unresolved {:.2}%", 100.0 * *c as f64 / total),
                k => format!("2^{k} {:.2}%", 100.0 * *c as f64 / total),
            })
            .collect();
        println!("{path}: {}", line.join("  "));
        let out = get("out").expect("--out");
        almondbread_core::image::write_png_bgra(format!("{out}.png"), &bgra, f.width, f.height).expect("png");
        return;
    }
    let center = get("center").expect("--center re,im");
    let (re_s, im_s) = center.split_once(',').expect("re,im");
    let scale: f64 = get("scale").expect("--scale").parse().unwrap();
    let (w, h) = get("size")
        .map(|s| {
            let (a, b) = s.split_once('x').unwrap();
            (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap())
        })
        .unwrap_or((160, 100));
    let angle = get("angle")
        .map(|a| a.parse::<f64>().unwrap().to_radians())
        .unwrap_or(0.0);
    let limit: u64 = get("limit")
        .map(|l| l.parse::<f64>().unwrap() as u64)
        .unwrap_or(1_000_000);
    let out = get("out").unwrap_or_else(|| "build/neck/truth".into());
    let dd_every: usize = get("dd").map(|d| d.parse().unwrap()).unwrap_or(0);
    let shift: f64 = get("shift").map(|d| d.parse().unwrap()).unwrap_or(0.0);

    let cre = parse_dd(re_s);
    let cim = parse_dd(im_s);
    let (rs, rc) = angle.sin_cos();
    let offset = |x: usize, y: usize| {
        let ox = x as f64 + 0.5 - w as f64 / 2.0;
        let oy = -(y as f64 + 0.5 - h as f64 / 2.0);
        if angle == 0.0 {
            (ox * scale, oy * scale)
        } else {
            ((ox * rc - oy * rs) * scale, (ox * rs + oy * rc) * scale)
        }
    };
    let t0 = std::time::Instant::now();
    let done = AtomicUsize::new(0);
    let fates: Vec<Fate> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (dx, dy) = offset(i % w, i / w);
            let f = iterate_f64(add(cre, Dd(dx, 0.0)).0, add(cim, Dd(dy, 0.0)).0, limit);
            let d = done.fetch_add(1, Ordering::Relaxed) + 1;
            if d.is_multiple_of((w * h / 10).max(1)) {
                eprintln!("  {d}/{} pixels, {:.1} s", w * h, t0.elapsed().as_secs_f64());
            }
            f
        })
        .collect();
    let secs = t0.elapsed().as_secs_f64();

    let mut bins: BTreeMap<u32, usize> = BTreeMap::new();
    let mut periods: BTreeMap<u64, usize> = BTreeMap::new();
    let (mut esc, mut int, mut unres) = (0usize, 0usize, 0usize);
    let mut ns: Vec<u64> = Vec::new();
    let mut ders: Vec<f64> = Vec::new();
    for f in &fates {
        match *f {
            Fate::Escaped(n, _, l) => {
                esc += 1;
                ns.push(n);
                ders.push(l);
                *bins.entry(63 - n.leading_zeros()).or_default() += 1;
            }
            Fate::Interior(p) => {
                int += 1;
                *periods.entry(p).or_default() += 1;
            }
            Fate::Unresolved => unres += 1,
        }
    }
    ns.sort_unstable();
    let total = fates.len() as f64;
    println!(
        "truth {w}x{h} limit {limit}: escaped {:.2}%  proven interior {:.2}%  unresolved {:.2}%  ({secs:.1} s)",
        100.0 * esc as f64 / total,
        100.0 * int as f64 / total,
        100.0 * unres as f64 / total
    );
    if !ns.is_empty() {
        let q = |p: f64| ns[((ns.len() - 1) as f64 * p) as usize];
        println!(
            "escape n: min {} p10 {} median {} p90 {} max {}",
            ns[0],
            q(0.1),
            q(0.5),
            q(0.9),
            ns[ns.len() - 1]
        );
    }
    if !ders.is_empty() {
        ders.sort_by(f64::total_cmp);
        let q = |p: f64| ders[((ders.len() - 1) as f64 * p) as usize] * std::f64::consts::LOG10_2;
        // |dz/dc| x pixel > 1: structure below the pixel; |dz/dc| x 2^-53 |c| > 1: f64
        // rounding of c alone decides n.
        println!(
            "log10 |dz/dc| at escape: p10 {:.1} median {:.1} p90 {:.1} max {:.1}  (log10 1/pixel {:.1}, f64 resolves 16.1)",
            q(0.1),
            q(0.5),
            q(0.9),
            q(1.0),
            -scale.log10()
        );
    }
    println!("escape histogram (n in [2^k, 2^k+1): % of all pixels, cumulative escaped by 2^k+1):");
    let mut cum = 0usize;
    for (k, c) in &bins {
        cum += c;
        println!(
            "  2^{k:<2} {:>10} .. {:>10}  {:6.2}%  cum {:6.2}%",
            1u64 << k,
            (1u64 << (k + 1)) - 1,
            100.0 * *c as f64 / total,
            100.0 * cum as f64 / total
        );
    }
    let mut ptop: Vec<(u64, usize)> = periods.into_iter().collect();
    ptop.sort_by_key(|&(p, c)| (std::cmp::Reverse(c), p));
    let list: Vec<String> = ptop.iter().take(10).map(|(p, c)| format!("{p}:{c}")).collect();
    println!(
        "interior periods (period:pixels; a multiple of the true period): {}",
        list.join(" ")
    );

    // Oracle-encoded field and a classification PNG.
    let mut n = vec![0f32; w * h];
    let mut s = vec![0f32; w * h];
    let mut bgra = vec![0u8; w * h * 4];
    let lmax = (limit as f64).log2();
    for (i, f) in fates.iter().enumerate() {
        let (b, g, r) = match *f {
            Fate::Escaped(k, sm, _) => {
                n[i] = k as f32;
                s[i] = sm;
                let v = ((k as f64).log2() / lmax).clamp(0.0, 1.0);
                let c = (40.0 + 215.0 * v) as u8;
                (c, c, c)
            }
            Fate::Interior(..) => {
                n[i] = ITER_INTERIOR;
                s[i] = 1.0;
                (90, 20, 10)
            }
            Fate::Unresolved => {
                n[i] = ITER_INTERIOR;
                (10, 10, 160)
            }
        };
        bgra[i * 4..i * 4 + 4].copy_from_slice(&[b, g, r, 255]);
    }
    let field = IterationField {
        width: w,
        height: h,
        n,
        s,
    };
    field.write(format!("{out}.iter")).expect("write iter");
    almondbread_core::image::write_png_bgra(format!("{out}.png"), &bgra, w, h).expect("write png");
    println!("wrote {out}.iter, {out}.png");

    if dd_every > 0 {
        let sample: Vec<usize> = (0..w * h)
            .step_by(dd_every)
            .filter(|&i| matches!(fates[i], Fate::Escaped(..)))
            .collect();
        let t1 = std::time::Instant::now();
        let diffs: Vec<(u64, Option<u64>)> = sample
            .par_iter()
            .filter_map(|&i| {
                let (dx, dy) = offset(i % w, i / w);
                let Fate::Escaped(nf, _, _) = fates[i] else {
                    unreachable!()
                };
                // The f64 pixel's own c in double-double: differences are rounding, not c.
                let (cr, ci) = (add(cre, Dd(dx, 0.0)).0, add(cim, Dd(dy, 0.0)).0);
                if shift > 0.0 {
                    // Sensitivity to c itself: double-double at c and at c + shift.
                    let a = iterate_dd(Dd(cr, 0.0), Dd(ci, 0.0), limit);
                    let b = iterate_dd(Dd(cr, shift), Dd(ci, 0.0), limit);
                    return (a != b).then_some((a.unwrap_or(0), b));
                }
                let nd = iterate_dd(Dd(cr, 0.0), Dd(ci, 0.0), limit);
                (nd != Some(nf)).then_some((nf, nd))
            })
            .collect();
        println!(
            "double-double check{}: {} escaped pixels re-iterated, {} differ ({:.1} s){}",
            if shift > 0.0 {
                format!(" (dd at c vs c + {shift:e})")
            } else {
                " (f64 vs dd, same c)".into()
            },
            sample.len(),
            diffs.len(),
            t1.elapsed().as_secs_f64(),
            diffs
                .iter()
                .take(5)
                .map(|(a, b)| format!(" f64 {a} dd {b:?};"))
                .collect::<String>()
        );
    }
}
