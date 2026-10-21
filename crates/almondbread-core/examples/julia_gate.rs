//! F4 step 1, the gate (kb: plans/f4-deep-julia "Design sketch"): does critical-orbit rebasing
//! give oracle-matching deep Julia pictures with float32 deltas?
//!
//! For five `k` and four depths it renders a small view three ways and compares them:
//! - **oracle**: every pixel iterated directly in high precision (`BigComplex`, the view's
//!   bits + 128) until its own spacing at iteration n (`|(f^n)'(z0)| texel`) reaches 2^-24, then
//!   directly in f64; independent of perturbation and rebasing. A second oracle on a grid shifted
//!   by (0.013, 0.007) texels measures chaos (how much a picture changes when nothing is wrong).
//! - **f32 rebased**: what the GPU kernel does, emulated in float32 on the CPU: the pixel's delta
//!   to the view-centre orbit `Z` (high precision, rounded to float32 / floatexp with a float32
//!   mantissa), `z' = 2Zz + z^2` (no `+c`), plain float32 above 2^-40 and floatexp below
//!   (float32 mantissa, int exponent), and Zhuoran's rebase `z <- Z_m + z` onto the *critical
//!   orbit* (the orbit of 0 under `z^2 + k`, which is the Mandelbrot reference orbit of `k`) when
//!   `|Z_m + z| < |z|` or the reference ends; after the first rebase the pixel stays on the
//!   critical orbit, rebasing to its index 0 like a Mandelbrot pixel.
//! - **f32 no rebase** (control): the same, but the only switch is at the centre orbit's end.
//!
//! Locations: an N-th preimage of the critical point 0 (backward iteration from 0 with seeded
//! random branches, N such that the preimage's Fatou piece is about a quarter of the view), the
//! centre offset from it by (0.23 W, 0.17 H) texels, so the centre orbit does not pass through 0
//! but the pixels around the preimage do (where rebasing has to work).
//!
//! `cargo run --release -p almondbread-core --example julia_gate -- [--size 200x125]
//!  [--depths 20,100,300,1000] [--cases card,rabbit,i,siegel,outside] [--png dir]`

use std::time::Instant;

use almondbread_core::oracle::{self, ITER_INTERIOR, IterationField};
use almondbread_math::{BigComplex, BigFloat, FloatExpComplex, ReferenceOrbitComputer};
use rayon::prelude::*;

const BAILOUT: f64 = 65536.0;

struct Case {
    name: &'static str,
    re: &'static str,
    im: &'static str,
    about: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "card",
        re: "-0.0619",
        im: "0.4848",
        about: "main cardioid (|mu| = 0.8, theta = 2 rad)",
    },
    Case {
        name: "rabbit",
        re: "-0.1225611668766536",
        im: "0.7448617666197442",
        about: "period-3 bulb nucleus (Douady rabbit)",
    },
    Case {
        name: "i",
        re: "0",
        im: "1",
        about: "Misiurewicz point i (dendrite)",
    },
    Case {
        name: "siegel",
        re: "-0.39054087021840008",
        im: "-0.58678790734696872",
        about: "Siegel disc, golden-mean rotation (cardioid boundary)",
    },
    Case {
        name: "outside",
        re: "-0.8",
        im: "0.156",
        about: "outside the set (Cantor dust)",
    },
];

// ---------------------------------------------------------------------------------------------
// Locations.

fn csqrt(w: &BigComplex) -> BigComplex {
    let bits = w.precision_bits();
    let r = w.magnitude_squared().sqrt();
    if r.is_zero() {
        return BigComplex::zero(bits);
    }
    let half = |x: BigFloat| x.times_power_of_two(-1);
    if !w.re.is_negative() {
        let re = half(&r + &w.re).sqrt();
        let im = &w.im / &re.times_power_of_two(1);
        BigComplex::new(re, im)
    } else {
        let mut im = half(&r - &w.re).sqrt();
        if w.im.is_negative() {
            im.negate();
        }
        let re = &w.im / &im.times_power_of_two(1);
        BigComplex::new(re, im)
    }
}

/// An N-th preimage of 0 whose piece is about `target_log2` (log2 of its size), and N.
fn preimage_of_zero(k: &BigComplex, bits: usize, target_log2: f64, seed: u64) -> (BigComplex, usize, f64) {
    let mut w = BigComplex::zero(bits);
    let mut log_d = 0.0f64; // log2 |(f^N)'(z*)|
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut n = 0usize;
    // The piece of 0 is ~0.25 across; its N-th preimage ~0.25 / |(f^N)'|.
    while -2.0 - log_d > target_log2 && n < 400_000 {
        let mut z = csqrt(&(&w - k));
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        if (rng >> 33) & 1 == 1 {
            z = BigComplex::new(-z.re, -z.im);
        }
        let zf = z.to_f64();
        log_d += (2.0 * zf[0].hypot(zf[1])).log2();
        w = z;
        n += 1;
    }
    (w, n, log_d)
}

// ---------------------------------------------------------------------------------------------
// The oracle: direct high precision, then f64.

struct Oracle<'a> {
    k: &'a BigComplex,
    kf: (f64, f64),
    bits: usize,
    texel_log2: f64,
    cycle: Vec<(f64, f64)>,
    max_iter: usize,
}

impl Oracle<'_> {
    fn pixel(&self, z0: BigComplex) -> (f32, f32) {
        let b2 = BAILOUT * BAILOUT;
        let mut z = z0;
        let (mut dr, mut di, mut dl) = (1.0f64, 0.0f64, 0.0f64); // derivative = d 2^dl
        let mut n = 0usize;
        let near_cycle = |p: (f64, f64)| self.cycle.iter().any(|c| (p.0 - c.0).hypot(p.1 - c.1) < 1e-9);
        // High precision while the pixel's own spacing is below 2^-24 (at most 40 x the depth's
        // bits: Siegel-disc pixels never expand).
        let cap = 40 * self.bits + 1000;
        let mut zf = z.to_f64();
        loop {
            let r2 = zf[0] * zf[0] + zf[1] * zf[1];
            if r2 > b2 {
                return (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32);
            }
            if n >= self.max_iter || near_cycle((zf[0], zf[1])) {
                return (ITER_INTERIOR, 0.0);
            }
            if dl + (dr.hypot(di)).log2() + self.texel_log2 > -24.0 || n > cap {
                break;
            }
            // d <- 2 z d
            let (a, b) = (2.0 * (zf[0] * dr - zf[1] * di), 2.0 * (zf[0] * di + zf[1] * dr));
            let m = a.abs().max(b.abs());
            if m > 0.0 {
                let e = m.log2().floor();
                dr = a * (-e).exp2();
                di = b * (-e).exp2();
                dl += e;
            } else {
                dl = -1e9;
            }
            z = &z.square() + self.k;
            n += 1;
            zf = z.to_f64();
        }
        let (mut x, mut y) = (zf[0], zf[1]);
        let (kr, ki) = self.kf;
        while n < self.max_iter {
            let t = x * x - y * y + kr;
            y = 2.0 * x * y + ki;
            x = t;
            n += 1;
            let r2 = x * x + y * y;
            if r2 > b2 {
                return (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32);
            }
            if n & 15 == 0 && near_cycle((x, y)) {
                return (ITER_INTERIOR, 0.0);
            }
        }
        (ITER_INTERIOR, 0.0)
    }
}

fn pixel_start(center: &BigComplex, texel: &BigFloat, x: usize, y: usize, w: usize, h: usize, shift: (f64, f64)) -> BigComplex {
    let ox = x as f64 + 0.5 - w as f64 / 2.0 + shift.0;
    let oy = -(y as f64 + 0.5 - h as f64 / 2.0) + shift.1;
    let bits = center.precision_bits();
    BigComplex::new(
        &center.re + &(texel * &BigFloat::from_f64(ox, bits)),
        &center.im + &(texel * &BigFloat::from_f64(oy, bits)),
    )
}

fn oracle_field(o: &Oracle<'_>, center: &BigComplex, texel: &BigFloat, w: usize, h: usize, shift: (f64, f64)) -> IterationField {
    let px: Vec<(f32, f32)> = (0..w * h)
        .into_par_iter()
        .map(|i| o.pixel(pixel_start(center, texel, i % w, i / w, w, h, shift)))
        .collect();
    IterationField {
        width: w,
        height: h,
        n: px.iter().map(|p| p.0).collect(),
        s: px.iter().map(|p| p.1).collect(),
    }
}

// ---------------------------------------------------------------------------------------------
// The float32 emulation (the kernel's FE: float32 mantissa, max-abs in [0.5, 1), int exponent).

const FE_ZERO: i32 = -(1 << 28);

#[derive(Clone, Copy, Debug)]
struct Fe {
    m: [f32; 2],
    e: i32,
}

fn pow2f(k: i32) -> f32 {
    if k < -126 { 0.0 } else { f32::from_bits(((k.min(127) + 127) as u32) << 23) }
}

fn scale2(m: [f32; 2], k: i32) -> [f32; 2] {
    let h = k >> 1;
    let (a, b) = (pow2f(h), pow2f(k - h));
    [m[0] * a * b, m[1] * a * b]
}

fn exp_of(a: f32) -> i32 {
    ((a.to_bits() >> 23) & 0xff) as i32 - 126
}

fn fe_norm(m: [f32; 2], e: i32) -> Fe {
    let a = m[0].abs().max(m[1].abs());
    if !(a > 0.0) || !a.is_normal() {
        if a > 0.0 {
            // subnormal: renormalise through f64
            let k = (a as f64).log2().floor() as i32 + 1;
            let f = (2.0f64).powi(-k);
            return Fe { m: [(m[0] as f64 * f) as f32, (m[1] as f64 * f) as f32], e: e + k };
        }
        return Fe { m: [0.0, 0.0], e: FE_ZERO };
    }
    let k = exp_of(a);
    Fe { m: scale2(m, -k), e: e + k }
}

fn fe_add(a: Fe, b: Fe) -> Fe {
    let (a, b) = if a.e < b.e { (b, a) } else { (a, b) };
    let d = a.e - b.e;
    if d > 40 {
        return a;
    }
    let s = scale2(b.m, -d);
    fe_norm([a.m[0] + s[0], a.m[1] + s[1]], a.e)
}

fn cmulf(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]]
}

fn fe_mul(a: Fe, b: Fe) -> Fe {
    if a.e == FE_ZERO || b.e == FE_ZERO {
        return Fe { m: [0.0, 0.0], e: FE_ZERO };
    }
    fe_norm(cmulf(a.m, b.m), a.e + b.e)
}

fn fe_less(a: Fe, b: Fe) -> bool {
    let na = a.m[0] * a.m[0] + a.m[1] * a.m[1];
    let nb = b.m[0] * b.m[0] + b.m[1] * b.m[1];
    if !(nb > 0.0) {
        return false;
    }
    if !(na > 0.0) {
        return true;
    }
    let d = a.e - b.e;
    if d > 2 {
        return false;
    }
    if d < -2 {
        return true;
    }
    na * pow2f(2 * d) < nb
}

fn fe_float(a: Fe) -> [f32; 2] {
    scale2(a.m, a.e.max(-300))
}

fn fe_of(x: FloatExpComplex) -> Fe {
    if x.mantissa == [0.0, 0.0] {
        Fe { m: [0.0, 0.0], e: FE_ZERO }
    } else {
        fe_norm(x.mantissa, x.exponent)
    }
}

struct Ref {
    pts: Vec<[f32; 2]>,
    ext: Vec<FloatExpComplex>,
}

const PLAIN_EXP: i32 = -40;
const TINY2: f32 = 1.0 / (1u128 << 100) as f32;

/// One pixel, as the deep kernel would do it: delta z0 = d (floatexp), centre orbit first.
fn emulate(center: &Ref, crit: &Ref, d: Fe, max_iter: usize, rebase: bool) -> (f32, f32, u32) {
    let b2 = (BAILOUT * BAILOUT) as f32;
    let mut r: &Ref = center;
    let mut on_crit = false;
    let mut last = r.pts.len() - 1;
    let (mut m, mut n) = (0usize, 0usize);
    let mut rebases = 0u32;
    // plain (e == None) or floatexp
    let mut w = [0f32; 2];
    let mut x = d;
    let mut plain = d.e >= PLAIN_EXP;
    if plain {
        w = fe_float(d);
    }
    let escaped = |n: usize, r2: f32| (n as f32, (1.0 - (0.5 * (r2 as f64).ln()).log2()) as f32);
    loop {
        if plain {
            let zr = r.pts[m];
            let z = [zr[0] + w[0], zr[1] + w[1]];
            let r2 = z[0] * z[0] + z[1] * z[1];
            if r2 > b2 {
                return (escaped(n, r2).0, escaped(n, r2).1, rebases);
            }
            let mut w2 = w[0] * w[0] + w[1] * w[1];
            let mut zz = zr;
            if (rebase && r2 < w2) || m == last {
                w = z;
                r = crit;
                on_crit = true;
                last = r.pts.len() - 1;
                m = 0;
                zz = [0.0, 0.0];
                w2 = r2;
                rebases += 1;
            }
            if w2 < TINY2 {
                x = fe_norm(w, 0);
                plain = false;
                continue;
            }
            if n >= max_iter {
                return (ITER_INTERIOR, 0.0, rebases);
            }
            let s = [2.0 * zz[0] + w[0], 2.0 * zz[1] + w[1]];
            w = cmulf(s, w);
            m += 1;
            n += 1;
        } else {
            // floatexp: full steps with the reference's own exponent (the kernel's scaled step
            // where |Z| >= 2^-32 rounds the same way to float32 mantissas).
            let zx = fe_of(r.ext[m]);
            let zf = fe_add(zx, x);
            let mut zr = zx;
            if m == last && zf.e > 0 {
                let p = fe_float(zf);
                let r2 = p[0] * p[0] + p[1] * p[1];
                if r2 > b2 {
                    return (escaped(n, r2).0, escaped(n, r2).1, rebases);
                }
            }
            let at_start = m == 0 && (on_crit || !rebase);
            if !at_start && ((rebase && fe_less(zf, x)) || m == last) {
                x = zf;
                r = crit;
                on_crit = true;
                last = r.pts.len() - 1;
                m = 0;
                zr = Fe { m: [0.0, 0.0], e: FE_ZERO };
                rebases += 1;
            }
            if x.e >= PLAIN_EXP {
                w = fe_float(x);
                plain = true;
                continue;
            }
            if n >= max_iter {
                return (ITER_INTERIOR, 0.0, rebases);
            }
            let two_z = if zr.e == FE_ZERO { zr } else { Fe { m: zr.m, e: zr.e + 1 } };
            x = fe_mul(fe_add(two_z, x), x);
            m += 1;
            n += 1;
            if x.e >= PLAIN_EXP {
                w = fe_float(x);
                plain = true;
            }
        }
    }
}

fn emulate_field(center: &Ref, crit: &Ref, texel: Fe, w: usize, h: usize, max_iter: usize, rebase: bool) -> (IterationField, u64) {
    let px: Vec<(f32, f32, u32)> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let ox = (i % w) as f32 + 0.5 - w as f32 / 2.0;
            let oy = -((i / w) as f32 + 0.5 - h as f32 / 2.0);
            let d = fe_mul(fe_norm([ox, oy], 0), texel);
            emulate(center, crit, d, max_iter, rebase)
        })
        .collect();
    let rebases = px.iter().map(|p| u64::from(p.2)).sum();
    (
        IterationField {
            width: w,
            height: h,
            n: px.iter().map(|p| p.0).collect(),
            s: px.iter().map(|p| p.1).collect(),
        },
        rebases,
    )
}

// ---------------------------------------------------------------------------------------------

fn png(field: &IterationField, path: &str) {
    let mut bgra = vec![0u8; field.n.len() * 4];
    for (i, (&n, &s)) in field.n.iter().zip(&field.s).enumerate() {
        let (r, g, b) = if n < 0.0 {
            (0.0, 0.0, 0.0)
        } else {
            let t = (n + s) as f64 * 0.35;
            (
                0.5 + 0.5 * (t).cos(),
                0.5 + 0.5 * (t + 2.1).cos(),
                0.5 + 0.5 * (t + 4.2).cos(),
            )
        };
        bgra[4 * i] = (b * 255.0) as u8;
        bgra[4 * i + 1] = (g * 255.0) as u8;
        bgra[4 * i + 2] = (r * 255.0) as u8;
        bgra[4 * i + 3] = 255;
    }
    almondbread_core::image::write_png_bgra(path, &bgra, field.width, field.height).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (w, h) = arg("--size")
        .map(|s| {
            let (a, b) = s.split_once('x').unwrap();
            (a.parse().unwrap(), b.parse().unwrap())
        })
        .unwrap_or((200usize, 125usize));
    let depths: Vec<u32> = arg("--depths")
        .unwrap_or_else(|| "20,100,300,1000".into())
        .split(',')
        .map(|d| d.parse().unwrap())
        .collect();
    let cases: Vec<&Case> = match arg("--cases") {
        Some(list) => list.split(',').map(|n| CASES.iter().find(|c| c.name == n).expect("case")).collect(),
        None => CASES.iter().collect(),
    };
    let png_dir = arg("--png");
    let max_iter: usize = arg("--iterations").map(|s| s.parse().unwrap()).unwrap_or(0);
    // `--locations 1`: print each view (for `almondbread verify --julia-k k --center c --scale
    // s`) instead of rendering.
    let locations = arg("--locations").is_some();

    if !locations {
        println!(
        "case     depth   N(pre)  iters  ref(c/k)    | oracle-shift: class%  esc%   med|dmu| | f32 rebased: class%  esc%   med|dmu|  rebases/px | no-rebase: class%  esc% | times o/e s"
        );
    }
    for case in cases {
        for &depth in &depths {
            let texel_log2 = -(depth as f64) * std::f64::consts::LOG2_10;
            let bits = ((-texel_log2) as usize + 160).div_ceil(64) * 64;
            // k as the app and the CLI read a decimal k (`Center::parse`: the binary value at the
            // precision its digits imply), so `almondbread verify --julia-k` sees the same set.
            let k = almondbread_core::viewport::Center::parse(case.re, case.im)
                .unwrap()
                .big()
                .with_precision(bits);
            let kf = (k.re.to_f64(), k.im.to_f64());
            let view_log2 = texel_log2 + (w as f64).log2();
            let (pre, npre, _) = preimage_of_zero(&k, bits, view_log2 - 2.0, 7 + depth as u64);
            let texel = BigFloat::from_f64(1.0, bits).times_power_of_two(texel_log2.floor() as i64)
                * BigFloat::from_f64((texel_log2 - texel_log2.floor()).exp2(), bits);
            let off = (0.23 * w as f64, 0.17 * h as f64);
            let center = BigComplex::new(
                &pre.re + &(&texel * &BigFloat::from_f64(off.0, bits)),
                &pre.im + &(&texel * &BigFloat::from_f64(off.1, bits)),
            );
            if locations {
                let digits = depth as usize + 25;
                println!(
                    "{} {} {},{} {},{} {} {}",
                    case.name,
                    depth,
                    case.re,
                    case.im,
                    center.re.to_decimal_string(Some(digits)),
                    center.im.to_decimal_string(Some(digits)),
                    texel.to_decimal_string(Some(17)),
                    (4 * bits).max(3000) + 2 * npre
                );
                continue;
            }
            // Iterations: the pixels separate from the centre after ~depth / log2|f'| steps.
            let mi = if max_iter > 0 { max_iter } else { (4 * bits).max(3000) + 2 * npre };
            let cycle: Vec<(f64, f64)> = almondbread_core::julia::attracting_cycle(kf)
                .map(|c| {
                    let mut p = c.point;
                    (0..c.period)
                        .map(|_| {
                            let q = p;
                            p = (p.0 * p.0 - p.1 * p.1 + kf.0, 2.0 * p.0 * p.1 + kf.1);
                            q
                        })
                        .collect()
                })
                .unwrap_or_default();
            let o = Oracle {
                k: &k,
                kf,
                bits,
                texel_log2,
                cycle,
                max_iter: mi,
            };
            let t0 = Instant::now();
            let truth = oracle_field(&o, &center, &texel, w, h, (0.0, 0.0));
            let shifted = oracle_field(&o, &center, &texel, w, h, (0.013, 0.007));
            let to = t0.elapsed().as_secs_f64();
            let t1 = Instant::now();
            let comp_c = ReferenceOrbitComputer::new(&k, bits).with_start(&center);
            let (cp, ce, _) = comp_c.compute_vec(mi, BAILOUT);
            let comp_k = ReferenceOrbitComputer::new(&k, bits);
            let (kp, ke, _) = comp_k.compute_vec(mi, BAILOUT);
            let cref = Ref { pts: cp, ext: ce };
            let kref = Ref { pts: kp, ext: ke };
            let tfe = texel.float_exp();
            let tx = fe_norm([tfe.mantissa, 0.0], tfe.exponent);
            let (emu, rebases) = emulate_field(&cref, &kref, tx, w, h, mi, true);
            let (naive, _) = emulate_field(&cref, &kref, tx, w, h, mi, false);
            let te = t1.elapsed().as_secs_f64();
            let (rs, _) = oracle::compare(&shifted, &truth);
            let (re, _) = oracle::compare(&emu, &truth);
            let (rn, _) = oracle::compare(&naive, &truth);
            let class = |r: &oracle::VerifyReport| 100.0 * (r.false_interior + r.false_exterior) as f64 / r.pixels as f64;
            println!(
                "{:8} 1e-{:<5} {:6} {:6} {:5}/{:<5} | {:6.2} {:6.2} {:9.2e} | {:6.2} {:6.2} {:9.2e} {:6.2} | {:6.2} {:6.2} | {:.1}/{:.1}",
                case.name,
                depth,
                npre,
                mi,
                cref.pts.len(),
                kref.pts.len(),
                class(&rs),
                rs.escape_differs_pct(),
                rs.median_error,
                class(&re),
                re.escape_differs_pct(),
                re.median_error,
                rebases as f64 / (w * h) as f64,
                class(&rn),
                rn.escape_differs_pct(),
                to,
                te
            );
            if let Some(dir) = &png_dir {
                png(&truth, &format!("{dir}/{}-1e{}-oracle.png", case.name, depth));
                png(&emu, &format!("{dir}/{}-1e{}-f32.png", case.name, depth));
                png(&naive, &format!("{dir}/{}-1e{}-norebase.png", case.name, depth));
            }
        }
        if !locations {
            println!("  ({}: k = {} + {}i)", case.about, case.re, case.im);
        }
    }
}
