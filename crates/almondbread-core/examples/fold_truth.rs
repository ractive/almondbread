//! Ground truth for deep views of any formula by direct BigFloat iteration of every pixel
//! (kb: formulas/burning-ship "Little ships"): no perturbation, no float32, no BLA.
//!
//! ```text
//! cargo run --release -p almondbread-core --example fold_truth -- --formula burningship \
//!     --center re,im --scale s --size 32x20 --limit 200000 [--extra 256] [--shift 0.001] \
//!     [--out build/x/truth]
//! ```
//!
//! Per pixel it iterates three times: at the view's precision (`needed_bits` + 64), at
//! `--extra` bits more, and at the view's precision with the pixel centre moved `--shift`
//! pixels right. Prints how many escape counts agree between the precisions (rounding) and
//! between the grids (sensitivity to c at this sampling: chaos), the share that never
//! escapes within `--limit`, and the escape histogram by powers of two. Writes `<out>.iter`
//! (the oracle encoding: escaped n/s, -1 not escaped) for `almondbread verify --fields`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use almondbread_core::formula::Formula;
use almondbread_core::oracle::{ITER_INTERIOR, IterationField};
use almondbread_core::viewport::{Center, Scale};
use almondbread_math::{BigComplex, BigFloat, ReferenceOrbitComputer, with_no_control};
use rayon::prelude::*;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let f = Formula::parse(arg(&args, "--formula").unwrap_or("burningship")).expect("--formula");
    let (re, im) = arg(&args, "--center").expect("--center re,im").split_once(',').expect("re,im");
    let center = Center::parse(re, im).expect("center");
    let texel = Scale::parse(arg(&args, "--scale").expect("--scale")).expect("scale");
    let (w, h) = arg(&args, "--size")
        .unwrap_or("32x20")
        .split_once('x')
        .map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap()))
        .expect("--size WxH");
    let limit: usize = arg(&args, "--limit").unwrap_or("200000").parse().unwrap();
    let extra: usize = arg(&args, "--extra").unwrap_or("256").parse().unwrap();
    let shift: f64 = arg(&args, "--shift").unwrap_or("0.001").parse().unwrap();
    let bailout = f.bailout();
    let bits = almondbread_core::reference::needed_bits(&center, texel, w.max(h), 64) + 64;
    let (tm, te) = texel.parts();
    let done = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    let run = |x: f64, y: f64, b: usize| -> (f32, f32) {
        let dx = (x + 0.5 - w as f64 / 2.0) * tm;
        let dy = -(y + 0.5 - h as f64 / 2.0) * tm;
        let c = BigComplex::new(
            center.big().re.with_precision(b) + BigFloat::from_f64(dx, 64).times_power_of_two(te),
            center.big().im.with_precision(b) + BigFloat::from_f64(dy, 64).times_power_of_two(te),
        );
        let comp = ReferenceOrbitComputer::new(&c, b).with_formula(f);
        let o = with_no_control(1 << 20, |ctl| comp.compute(limit, bailout, None, None, ctl));
        if o.escaped {
            let r2 = o.final_z.magnitude_squared().to_f64();
            (o.iterations() as f32, f.smooth_offset(r2) as f32)
        } else {
            (ITER_INTERIOR, 0.0)
        }
    };
    let res: Vec<[(f32, f32); 3]> = (0..w * h)
        .into_par_iter()
        .map(|k| {
            let (x, y) = ((k % w) as f64, (k / w) as f64);
            let r = [run(x, y, bits), run(x, y, bits + extra), run(x + shift, y, bits)];
            let d = done.fetch_add(1, Ordering::Relaxed) + 1;
            if d % (w * h / 10).max(1) == 0 {
                eprintln!("{d}/{} pixels, {:.0} s", w * h, t0.elapsed().as_secs_f64());
            }
            r
        })
        .collect();
    let n = w * h;
    let same_prec = res.iter().filter(|r| r[0].0 == r[1].0).count();
    let same_shift = res.iter().filter(|r| r[0].0 == r[2].0).count();
    let bounded = res.iter().filter(|r| r[0].0 < 0.0).count();
    let mut hist: BTreeMap<u32, usize> = BTreeMap::new();
    for r in &res {
        if r[0].0 >= 0.0 {
            *hist.entry((r[0].0.max(1.0)).log2() as u32).or_default() += 1;
        }
    }
    println!(
        "{f:?} {} {} {} per pixel, {w}x{h}, {bits} bits, limit {limit}: same n at +{extra} bits {same_prec}/{n}, \
         at a {shift} px shift {same_shift}/{n}; not escaped {bounded} ({:.1} %); {:.0} s",
        center.re(),
        center.im(),
        texel.to_f64(),
        100.0 * bounded as f64 / n as f64,
        t0.elapsed().as_secs_f64()
    );
    let line: Vec<String> = hist.iter().map(|(k, v)| format!("2^{k}: {v}")).collect();
    println!("escape n histogram: {}", line.join(", "));
    if let Some(out) = arg(&args, "--out") {
        let texels: Vec<[f32; 2]> = res.iter().map(|r| [r[0].0, r[0].1]).collect();
        IterationField::from_texels(w, h, &texels).write(format!("{out}.iter")).expect("write");
    }
}
