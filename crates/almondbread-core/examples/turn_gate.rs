//! F9, the gate (kb: plans/f9-turn "Gate"): do perturbation with critical-orbit rebasing and
//! the BLA hold on a turned plane of the (c, z_0) space, at depth?
//!
//! A turned view is the plane through a pivot `(c*, z*)` at angle `t`: the pixel at view offset
//! `dp` from the centre has `c = C + cos t dp`, `z_0 = Z_0 + sin t dp`, with `(C, Z_0)` the
//! centre's point. Each view is rendered four ways and compared:
//! - **direct**: every pixel iterated directly in high precision (`BigComplex`, twice the view's
//!   bits plus 128) from its own `(c, z_0)` until its spacing `|dz_n/dp| texel` reaches 2^-24 (or 4 x
//!   the bits + 2000 steps: interior orbits contract), then in f64; no perturbation, no
//!   rebasing. A second direct field on a grid shifted by (0.013, 0.007) texels measures chaos.
//! - **oracle**: the CPU oracle (`oracle::cpu_render_deep_with`: f64 extended-range
//!   perturbation against the centre orbit from `Z_0`, `z' = 2Zz + z^2 + dc` from `z = sin t
//!   dp`, Zhuoran's rebase onto the critical orbit of `C` when `|Z_m + z| < |z|` or an orbit
//!   ends). What `almondbread verify` compares the GPU with.
//! - **bla**: the same model with the deep BLA table of the reference (Mandelbrot's single
//!   steps, `B = 1`, applied to `dz` and `dc`) taken at every index where a block is valid.
//!
//! Locations: `rabbit-pre`, `i-pre`: the pivot is `c*` (the Douady rabbit, the Misiurewicz point
//! i) with `z*` an N-th preimage of 0 under `z^2 + c*` (backward iteration with seeded random
//! branches), N such that the piece of 0 pulled back along the plane (`0.25 / |sin t A_N +
//! cos t B_N|`, `A = dz_N/dz_0`, `B = dz_N/dc`) is about a quarter of the view at the depth;
//! `i-zero`: the pivot is `(i, 0)` (a Mandelbrot view of the Misiurewicz point i, turned), scale
//! the depth. The view centre is offset from the pivot by (0.23 W, 0.17 H) texels, so the centre
//! orbit does not pass through 0 but the pixels near the pivot do (where rebasing must work).
//!
//! `cargo run --release -p almondbread-core --example turn_gate -- [--size 128x80]
//!  [--depths 30,100,300] [--angles 0,30,60,90] [--cases rabbit-pre,i-pre,i-zero] [--png dir]
//!  [--locations 1]`

use std::time::Instant;

use almondbread_core::bla::{BLA_EPSILON, BlaTableX};
use almondbread_core::formula::Formula;
use almondbread_core::oracle::{self, ITER_INTERIOR, IterationField, OracleReference};
use almondbread_core::viewport::{Center, Scale};
use almondbread_math::{BigComplex, BigFloat, DoubleExp, DoubleExpComplex, TurnPlane};
use rayon::prelude::*;

const BAILOUT: f64 = 65536.0;

struct Case {
    name: &'static str,
    re: &'static str,
    im: &'static str,
    /// z* = a preimage of 0 (else 0).
    preimage: bool,
}

const CASES: &[Case] = &[
    Case {
        name: "rabbit-pre",
        re: "-0.1225611668766536",
        im: "0.7448617666197442",
        preimage: true,
    },
    Case {
        name: "i-pre",
        re: "0",
        im: "1",
        preimage: true,
    },
    Case {
        name: "i-zero",
        re: "0",
        im: "1",
        preimage: false,
    },
];

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

/// log2 |sin t A_N + cos t B_N| along an orbit given by its points `orbit[0] = z_0`, ...,
/// `orbit[N-1]` (f64 with a running exponent; the points come from the high-precision chain, so
/// no f64 orbit drifts off the preimage).
fn plane_derivative_log2(orbit: impl Iterator<Item = (f64, f64)>, cos: f64, sin: f64) -> f64 {
    let (mut d, mut dl) = ((sin, 0.0), 0.0f64);
    for z in orbit {
        // d <- 2 z d + cos 2^-dl
        let a = (2.0 * (z.0 * d.0 - z.1 * d.1), 2.0 * (z.0 * d.1 + z.1 * d.0));
        d = (a.0 + cos * (-dl).exp2(), a.1);
        let m = d.0.abs().max(d.1.abs());
        if m > 0.0 {
            let e = m.log2().floor();
            d = (d.0 * (-e).exp2(), d.1 * (-e).exp2());
            dl += e;
        }
    }
    dl + d.0.hypot(d.1).log2()
}

/// An N-th preimage of 0 whose pull-back along the plane is about `2^target_log2`, and N.
fn preimage_of_zero(
    c: &BigComplex,
    bits: usize,
    target_log2: f64,
    cos: f64,
    sin: f64,
    seed: u64,
) -> (BigComplex, usize) {
    let mut w = BigComplex::zero(bits);
    let mut chain: Vec<BigComplex> = Vec::new();
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut log_a = 0.0f64;
    // Backward until |A_N| alone is well past the target (B_N grows like A_N), then pick the
    // depth along the chain where the plane's derivative fits.
    while -2.0 - log_a > target_log2 - 8.0 && chain.len() < 400_000 {
        let mut z = csqrt(&(&w - c));
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        if (rng >> 33) & 1 == 1 {
            z = BigComplex::new(-z.re, -z.im);
        }
        let zf = z.to_f64();
        log_a += (2.0 * zf[0].hypot(zf[1])).log2();
        chain.push(z.clone());
        w = z;
    }
    // chain[k] is a (k+1)-th preimage; walk down until the plane derivative reaches the target.
    let pts: Vec<(f64, f64)> = chain
        .iter()
        .map(|z| {
            let f = z.to_f64();
            (f[0], f[1])
        })
        .collect();
    let mut best = (chain.len() - 1, f64::INFINITY);
    for k in (0..chain.len()).step_by(4) {
        // Forward from chain[k]: chain[k], chain[k-1], ..., chain[0] (then 0).
        let l = -2.0 - plane_derivative_log2(pts[..=k].iter().rev().copied(), cos, sin);
        let err = (l - target_log2).abs();
        if err < best.1 {
            best = (k, err);
        }
    }
    (chain[best.0].clone(), best.0 + 1)
}

struct Direct {
    bits: usize,
    texel_log2: f64,
    cos: f64,
    sin: f64,
    max_iter: usize,
}

impl Direct {
    fn pixel(&self, c: &BigComplex, z0: BigComplex) -> (f32, f32) {
        let b2 = BAILOUT * BAILOUT;
        let mut z = z0;
        let (mut dr, mut di, mut dl) = (self.sin, 0.0f64, 0.0f64); // dz/dp = d 2^dl
        let mut n = 0usize;
        let cap = 4 * self.bits + 2000;
        let mut zf = z.to_f64();
        loop {
            let r2 = zf[0] * zf[0] + zf[1] * zf[1];
            if r2 > b2 {
                return (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32);
            }
            if n >= self.max_iter {
                return (ITER_INTERIOR, 0.0);
            }
            let spacing = dl + dr.hypot(di).log2() + self.texel_log2;
            if spacing > -24.0 || n > cap {
                break;
            }
            let a = 2.0 * (zf[0] * dr - zf[1] * di);
            let b = 2.0 * (zf[0] * di + zf[1] * dr);
            let a = a + self.cos * (-dl).exp2();
            let m = a.abs().max(b.abs());
            if m > 0.0 {
                let e = m.log2().floor();
                dr = a * (-e).exp2();
                di = b * (-e).exp2();
                dl += e;
            }
            z = &z.square() + c;
            n += 1;
            zf = z.to_f64();
        }
        let cf = c.to_f64();
        let (mut x, mut y) = (zf[0], zf[1]);
        while n < self.max_iter {
            let t = x * x - y * y + cf[0];
            y = 2.0 * x * y + cf[1];
            x = t;
            n += 1;
            let r2 = x * x + y * y;
            if r2 > b2 {
                return (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32);
            }
        }
        (ITER_INTERIOR, 0.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn field(
        &self,
        c0: &BigComplex,
        z0: &BigComplex,
        texel: &BigFloat,
        w: usize,
        h: usize,
        shift: (f64, f64),
    ) -> IterationField {
        // (Twice the view's bits: a pixel at the critical point 0 is z_0^2 + c one step later,
        // which needs the squared scale.)
        let bits = 2 * self.bits + 128;
        let (cos, sin) = (BigFloat::from_f64(self.cos, bits), BigFloat::from_f64(self.sin, bits));
        let px: Vec<(f32, f32)> = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let ox = (i % w) as f64 + 0.5 - w as f64 / 2.0 + shift.0;
                let oy = -((i / w) as f64 + 0.5 - h as f64 / 2.0) + shift.1;
                let d = BigComplex::new(
                    texel * &BigFloat::from_f64(ox, bits),
                    texel * &BigFloat::from_f64(oy, bits),
                );
                let c = c0 + &d.scaled(&cos);
                let z = z0 + &d.scaled(&sin);
                self.pixel(&c, z)
            })
            .collect();
        IterationField {
            width: w,
            height: h,
            n: px.iter().map(|p| p.0).collect(),
            s: px.iter().map(|p| p.1).collect(),
        }
    }
}

/// A shared-exponent float32 point (the GPU's floatexp reference) of an extended-range one.
fn float_exp(z: DoubleExpComplex) -> almondbread_math::FloatExpComplex {
    if z.re.is_zero() && z.im.is_zero() {
        return almondbread_math::FloatExpComplex::ZERO;
    }
    let e = match (z.re.is_zero(), z.im.is_zero()) {
        (true, _) => z.im.exponent,
        (_, true) => z.re.exponent,
        _ => z.re.exponent.max(z.im.exponent),
    };
    let part = |d: DoubleExp| {
        if d.is_zero() {
            0.0
        } else {
            d.times_power_of_two(-e).to_f64() as f32
        }
    };
    almondbread_math::FloatExpComplex {
        mantissa: [part(z.re), part(z.im)],
        exponent: e as i32,
    }
}

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
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (w, h) = arg("--size")
        .map(|s| {
            let (a, b) = s.split_once('x').unwrap();
            (a.parse().unwrap(), b.parse().unwrap())
        })
        .unwrap_or((128usize, 80usize));
    let list = |name: &str, default: &str| -> Vec<u32> {
        arg(name)
            .unwrap_or_else(|| default.into())
            .split(',')
            .map(|d| d.parse().unwrap())
            .collect()
    };
    let depths = list("--depths", "30,100,300");
    let angles = list("--angles", "0,30,60,90");
    let cases: Vec<&Case> = match arg("--cases") {
        Some(l) => l
            .split(',')
            .map(|n| CASES.iter().find(|c| c.name == n).expect("case"))
            .collect(),
        None => CASES.iter().collect(),
    };
    let png_dir = arg("--png");
    let locations = arg("--locations").is_some();
    if !locations {
        println!(
            "case        angle depth  N     iters | shift: class% esc%  med|dmu| | oracle: class% esc%  med|dmu| | bla: class% esc%  med|dmu| skip% | rebases/px | t direct/model s"
        );
    }
    for case in cases {
        for &depth in &depths {
            for &angle in &angles {
                let centideg = (angle * 100) as u16;
                let (cos, sin) = almondbread_math::turn_cos_sin(centideg);
                let texel_log2 = -(depth as f64) * std::f64::consts::LOG2_10;
                let bits = ((-texel_log2) as usize + 160).div_ceil(64) * 64;
                let c_star = Center::parse(case.re, case.im).unwrap().big().with_precision(bits);
                let view_log2 = texel_log2 + (w as f64).log2();
                let (z_star, npre) = if case.preimage {
                    preimage_of_zero(&c_star, bits, view_log2 - 2.0, cos, sin, 7 + depth as u64)
                } else {
                    (BigComplex::zero(bits), 0)
                };
                let plane = TurnPlane::new(&c_star, &z_star, centideg);
                let texel = BigFloat::from_f64(1.0, bits).times_power_of_two(texel_log2.floor() as i64)
                    * BigFloat::from_f64((texel_log2 - texel_log2.floor()).exp2(), bits);
                let off = (0.23 * w as f64, 0.17 * h as f64);
                let pv = plane.pivot_view();
                let p = BigComplex::new(
                    &pv.re + &(&texel * &BigFloat::from_f64(off.0, bits)),
                    &pv.im + &(&texel * &BigFloat::from_f64(off.1, bits)),
                );
                let center = Center::from_big(p.clone());
                if locations {
                    // The view as `almondbread verify` options: the centre's point (c, z_0) and
                    // the angle (`--center c --z0 z0 --turn t --scale texel`).
                    let digits = 2 * depth as usize + 30;
                    let (c, z) = plane.point(&p, 2 * bits + 128);
                    let s = |x: &BigFloat| x.to_decimal_string(Some(digits));
                    println!(
                        "{} {} {} --center {},{} --z0 {},{} --turn {} --scale {} --iterations {}",
                        case.name,
                        angle,
                        depth,
                        s(&c.re),
                        s(&c.im),
                        s(&z.re),
                        s(&z.im),
                        angle,
                        texel.to_decimal_string(Some(17)),
                        (4 * bits).max(3000) + 2 * npre
                    );
                    continue;
                }
                let mi = (4 * bits).max(3000) + 2 * npre;
                let formula = Formula::Turn(plane);
                let (c0, z0) = plane.point(&p, 2 * bits + 128);
                let direct = Direct {
                    bits,
                    texel_log2,
                    cos,
                    sin,
                    max_iter: mi,
                };
                let t0 = Instant::now();
                let truth = direct.field(&c0, &z0, &texel, w, h, (0.0, 0.0));
                let shifted = direct.field(&c0, &z0, &texel, w, h, (0.013, 0.007));
                let td = t0.elapsed().as_secs_f64();
                let t1 = Instant::now();
                let scale = Scale::from_log2(texel_log2);
                let reference = OracleReference::compute_for(formula, &center, scale, w.max(h), mi, BAILOUT);
                let model = oracle::cpu_render_deep_with(&reference, scale, w, h, mi, BAILOUT, (0.0, 0.0), 0.0);
                // The deep BLA table of the same reference (from its floatexp points, as the
                // renderer builds it).
                let pts: Vec<almondbread_math::FloatExpComplex> =
                    reference.extended.iter().map(|z| float_exp(*z)).collect();
                let table = BlaTableX::build_for(&pts, BLA_EPSILON, formula);
                let (tm, te) = scale.parts();
                let px: Vec<oracle::TurnPixel> = (0..w * h)
                    .into_par_iter()
                    .map(|i| {
                        let ox = (i % w) as f64 + 0.5 - w as f64 / 2.0;
                        let oy = -((i / w) as f64 + 0.5 - h as f64 / 2.0);
                        let d = DoubleExpComplex::new(
                            DoubleExp::from_f64(ox * tm).times_power_of_two(te),
                            DoubleExp::from_f64(oy * tm).times_power_of_two(te),
                        );
                        let (d0, dc) = oracle::turn_split(d, cos, sin);
                        oracle::turn_pixel(&reference, d0, dc, mi, BAILOUT * BAILOUT, Some(&table))
                    })
                    .collect();
                let bla = IterationField {
                    width: w,
                    height: h,
                    n: px.iter().map(|p| p.n).collect(),
                    s: px.iter().map(|p| p.s).collect(),
                };
                let skipped: usize = px.iter().map(|p| p.skipped).sum();
                let iterated: f64 = px
                    .iter()
                    .map(|p| if p.n >= 0.0 { f64::from(p.n) } else { mi as f64 })
                    .sum();
                let rebases: u64 = px.iter().map(|p| u64::from(p.rebases)).sum();
                let tm_s = t1.elapsed().as_secs_f64();
                let (rs, _) = oracle::compare(&shifted, &truth);
                let (ro, _) = oracle::compare(&model, &truth);
                let (rb, _) = oracle::compare(&bla, &truth);
                let class =
                    |r: &oracle::VerifyReport| 100.0 * (r.false_interior + r.false_exterior) as f64 / r.pixels as f64;
                println!(
                    "{:11} {:5} 1e-{:<4} {:5} {:6} | {:6.2} {:6.2} {:9.2e} | {:6.2} {:6.2} {:9.2e} | {:6.2} {:6.2} {:9.2e} {:5.1} | {:6.2} | {:.1}/{:.1}",
                    case.name,
                    angle,
                    depth,
                    npre,
                    mi,
                    class(&rs),
                    rs.escape_differs_pct(),
                    rs.median_error,
                    class(&ro),
                    ro.escape_differs_pct(),
                    ro.median_error,
                    class(&rb),
                    rb.escape_differs_pct(),
                    rb.median_error,
                    100.0 * skipped as f64 / iterated.max(1.0),
                    rebases as f64 / (w * h) as f64,
                    td,
                    tm_s
                );
                if let Some(dir) = &png_dir {
                    let stem = format!("{dir}/{}-{}deg-1e{}", case.name, angle, depth);
                    png(&truth, &format!("{stem}-direct.png"));
                    png(&model, &format!("{stem}-oracle.png"));
                    png(&bla, &format!("{stem}-bla.png"));
                }
            }
        }
    }
}
